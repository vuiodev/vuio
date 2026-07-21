//! TV discovery and dashboard playlist-casting API handlers.

use crate::{database::DatabaseManager, state::AppState};
use axum::{extract::State, http::StatusCode, response::IntoResponse};
use tracing::error;

#[derive(serde::Deserialize)]
pub struct ApiCastPlaylistRequest {
    pub renderer_id: String,
    pub folder_name: String,
    pub file_ids: Vec<i64>,
}

/// Discover UPnP/DLNA TVs, Chromecasts, and AirPlay devices, and return them as DiscoveredTv equivalents in JSON format
pub async fn api_list_renderers<D: DatabaseManager>(
    State(state): State<AppState<D>>,
) -> impl IntoResponse {
    use crate::discovery::TargetKind;
    use crate::tv_control::DiscoveredTv;

    let targets = state.discovery_service.targets().await;

    let renderers = targets
        .into_iter()
        .map(|target| {
            let (friendly_name, model_name) = match target.kind {
                TargetKind::Dlna => (
                    target.friendly_name.clone(),
                    target.model.clone().unwrap_or_else(|| "DLNA".to_string()),
                ),
                TargetKind::Chromecast => (
                    format!("{} (Chromecast)", target.friendly_name),
                    target
                        .model
                        .clone()
                        .unwrap_or_else(|| "Chromecast".to_string()),
                ),
                TargetKind::AirPlay => (
                    format!("{} (AirPlay)", target.friendly_name),
                    target
                        .model
                        .clone()
                        .unwrap_or_else(|| "AirPlay".to_string()),
                ),
            };
            DiscoveredTv {
                id: target.id,
                friendly_name,
                control_url: target
                    .control_url
                    .unwrap_or_else(|| target.address.to_string()),
                location_url: match target.kind {
                    TargetKind::Dlna => format!("http://{}", target.address),
                    TargetKind::Chromecast => format!("chromecast://{}", target.address),
                    TargetKind::AirPlay => format!("airplay://{}", target.address),
                },
                model_name,
            }
        })
        .collect::<Vec<_>>();

    (StatusCode::OK, axum::Json(renderers))
}

/// Create a temporary playlist with the provided video files and cast it to the TV
pub async fn api_cast_playlist<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    axum::Json(payload): axum::Json<ApiCastPlaylistRequest>,
) -> impl IntoResponse {
    // Read old web-cast playlists, but keep them until the replacement is complete.
    let playlists = match state.database.get_playlists().await {
        Ok(list) => list,
        Err(e) => {
            error!(error = %e, "Failed to list playlists for web casting");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({ "error": "Internal Server Error" })),
            );
        }
    };

    let old_web_cast_ids = playlists
        .into_iter()
        .filter(|playlist| playlist.name.starts_with("Web Cast - "))
        .filter_map(|playlist| playlist.id)
        .collect::<Vec<_>>();

    // 2. Create the new playlist
    let playlist_name = format!("Web Cast - {}", payload.folder_name);
    let playlist_id = match state.database.create_playlist(&playlist_name, None).await {
        Ok(id) => id,
        Err(e) => {
            error!(error = %e, "Failed to create web-cast playlist");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({ "error": "Internal Server Error" })),
            );
        }
    };

    // 3. Add file IDs to the playlist (batch add)
    let tracks_to_add: Vec<(i64, u32)> = payload
        .file_ids
        .iter()
        .enumerate()
        .map(|(idx, &id)| (id, idx as u32))
        .collect();

    if let Err(e) = state
        .database
        .batch_add_to_playlist(playlist_id, &tracks_to_add)
        .await
    {
        error!(error = %e, playlist_id, "Failed to add tracks to web-cast playlist");
        match state.database.delete_playlist(playlist_id).await {
            Ok(_) => {
                // The transient playlist may have been browsed while the batch
                // insert was in flight, so discard responses from that window.
                crate::web::eventing::invalidate_browse_responses(&state).await;
            }
            Err(rollback_error) => {
                error!(error = %rollback_error, playlist_id, "Failed to roll back incomplete web-cast playlist");
                crate::web::eventing::publish_content_change(&state).await;
            }
        }
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({ "error": "Internal Server Error" })),
        );
    }

    for old_id in old_web_cast_ids {
        if let Err(error) = state.database.delete_playlist(old_id).await {
            error!(%error, playlist_id = old_id, "Failed to delete superseded web-cast playlist");
        }
    }

    // Publish before casting so a renderer-control failure cannot leave DLNA
    // clients on the old browse generation.
    crate::web::eventing::publish_content_change(&state).await;

    // Check if the target is non-DLNA (Chromecast or AirPlay)
    let targets = state.discovery_service.targets().await;

    if let Some(target) = targets.into_iter().find(|t| t.id == payload.renderer_id) {
        use crate::discovery::TargetKind;
        if target.kind != TargetKind::Dlna {
            let first_file_id = match payload.file_ids.first() {
                Some(&id) => id,
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        axum::Json(serde_json::json!({ "error": "Cannot cast an empty playlist" })),
                    );
                }
            };
            let media_file = match state.database.get_file_by_id(first_file_id).await {
                Ok(Some(file)) => file,
                Ok(None) => {
                    return (
                        StatusCode::NOT_FOUND,
                        axum::Json(serde_json::json!({ "error": "Media file not found" })),
                    );
                }
                Err(e) => {
                    error!(error = %e, "Failed to look up media file for casting");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        axum::Json(serde_json::json!({ "error": "Internal server error" })),
                    );
                }
            };

            let origin = state.advertised_http_origin();
            let media_url = format!(
                "{}/media/{}",
                origin,
                media_file.id.unwrap_or(first_file_id)
            );

            match target.kind {
                TargetKind::Chromecast => {
                    match crate::chromecast::client::ChromecastClient::connect(target.address).await
                    {
                        Ok(client) => {
                            if let Err(e) = client.launch_media_receiver().await {
                                return (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    axum::Json(
                                        serde_json::json!({ "error": format!("Chromecast launch error: {}", e) }),
                                    ),
                                );
                            }
                            match client
                                .load(
                                    &media_url,
                                    &media_file.mime_type,
                                    media_file.title.as_deref().unwrap_or(&media_file.filename),
                                )
                                .await
                            {
                                Ok(()) => {
                                    let mut casts = state.active_casts.lock().await;
                                    casts.insert(
                                        target.friendly_name.clone(),
                                        media_file.filename.clone(),
                                    );
                                    (
                                        StatusCode::OK,
                                        axum::Json(serde_json::json!({ "status": "playing" })),
                                    )
                                }
                                Err(e) => (
                                    StatusCode::BAD_REQUEST,
                                    axum::Json(
                                        serde_json::json!({ "error": format!("Chromecast load error: {}", e) }),
                                    ),
                                ),
                            }
                        }
                        Err(e) => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            axum::Json(
                                serde_json::json!({ "error": format!("Chromecast connection error: {}", e) }),
                            ),
                        ),
                    }
                }
                TargetKind::AirPlay => {
                    let client = crate::airplay::client::AirPlayClient::new(target.address);
                    match client.play(&media_url, 0.0).await {
                        Ok(()) => {
                            let mut casts = state.active_casts.lock().await;
                            casts.insert(target.friendly_name.clone(), media_file.filename.clone());
                            (
                                StatusCode::OK,
                                axum::Json(serde_json::json!({ "status": "playing" })),
                            )
                        }
                        Err(e) => (
                            StatusCode::BAD_REQUEST,
                            axum::Json(
                                serde_json::json!({ "error": format!("AirPlay error: {}", e) }),
                            ),
                        ),
                    }
                }
                _ => unreachable!(),
            }
        } else {
            // DLNA Target
            match crate::web::mcp::cast_playlist_helper(
                &state,
                playlist_id,
                &payload.renderer_id,
                0,
            )
            .await
            {
                Ok(result) => (StatusCode::OK, axum::Json(result)),
                Err(e) => (
                    StatusCode::BAD_REQUEST,
                    axum::Json(serde_json::json!({ "error": format!("Cast error: {}", e) })),
                ),
            }
        }
    } else {
        // Fallback to DLNA if target not found in discovery list (could be legacy refresh status)
        match crate::web::mcp::cast_playlist_helper(&state, playlist_id, &payload.renderer_id, 0)
            .await
        {
            Ok(result) => (StatusCode::OK, axum::Json(result)),
            Err(e) => (
                StatusCode::BAD_REQUEST,
                axum::Json(serde_json::json!({ "error": format!("Cast error: {}", e) })),
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Unified target discovery and casting API
// ---------------------------------------------------------------------------

/// List all discovered playback targets (DLNA + Chromecast + AirPlay).
pub async fn api_list_targets<D: DatabaseManager>(
    State(state): State<AppState<D>>,
) -> impl IntoResponse {
    let targets = state.discovery_service.targets().await;
    (StatusCode::OK, axum::Json(targets))
}

/// Request body for casting to a target.
#[derive(serde::Deserialize)]
pub struct ApiCastToTargetRequest {
    pub target_id: String,
    pub media_id: i64,
}

/// Cast a media file to a discovered target.
pub async fn api_cast_to_target<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    axum::Json(payload): axum::Json<ApiCastToTargetRequest>,
) -> impl IntoResponse {
    use crate::discovery::{compat, TargetKind};

    // Look up the media file
    let media_file = match state.database.get_file_by_id(payload.media_id).await {
        Ok(Some(file)) => file,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                axum::Json(serde_json::json!({ "error": "Media file not found" })),
            );
        }
        Err(e) => {
            error!(error = %e, "Failed to look up media file for casting");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({ "error": "Internal server error" })),
            );
        }
    };

    let targets = state.discovery_service.targets().await;

    let target = match targets.iter().find(|t| t.id == payload.target_id) {
        Some(t) => t,
        None => {
            return (
                StatusCode::NOT_FOUND,
                axum::Json(serde_json::json!({ "error": "Target device not found" })),
            );
        }
    };

    // Check compatibility
    let extension = std::path::Path::new(&media_file.filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let compat_result = compat::check_compatibility(target, extension, &media_file.mime_type);

    if !compat_result.is_compatible() {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({
                "error": "Incompatible media format",
                "compatibility": compat_result,
            })),
        );
    }

    // Build the streaming URL
    let origin = state.advertised_http_origin();
    let media_url = format!(
        "{}/media/{}",
        origin,
        media_file.id.unwrap_or(payload.media_id)
    );

    // Cast based on target type
    match target.kind {
        TargetKind::Dlna => {
            if let Some(control_url) = &target.control_url {
                match crate::tv_control::cast_media(
                    control_url,
                    &media_url,
                    media_file.title.as_deref().unwrap_or(&media_file.filename),
                    &media_file.mime_type,
                )
                .await
                {
                    Ok(()) => {
                        let mut casts = state.active_casts.lock().await;
                        casts.insert(target.friendly_name.clone(), media_file.filename.clone());
                        (
                            StatusCode::OK,
                            axum::Json(serde_json::json!({
                                "status": "casting",
                                "target": target.friendly_name,
                                "media": media_file.filename,
                                "compatibility": compat_result,
                            })),
                        )
                    }
                    Err(e) => (
                        StatusCode::BAD_REQUEST,
                        axum::Json(
                            serde_json::json!({ "error": format!("DLNA cast error: {}", e) }),
                        ),
                    ),
                }
            } else {
                (
                    StatusCode::BAD_REQUEST,
                    axum::Json(serde_json::json!({ "error": "DLNA target has no control URL" })),
                )
            }
        }
        TargetKind::Chromecast => {
            match crate::chromecast::client::ChromecastClient::connect(target.address).await {
                Ok(client) => {
                    if let Err(e) = client.launch_media_receiver().await {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            axum::Json(
                                serde_json::json!({ "error": format!("Chromecast launch error: {}", e) }),
                            ),
                        );
                    }
                    match client
                        .load(
                            &media_url,
                            &media_file.mime_type,
                            media_file.title.as_deref().unwrap_or(&media_file.filename),
                        )
                        .await
                    {
                        Ok(()) => {
                            let mut casts = state.active_casts.lock().await;
                            casts.insert(target.friendly_name.clone(), media_file.filename.clone());
                            (
                                StatusCode::OK,
                                axum::Json(serde_json::json!({
                                    "status": "casting",
                                    "target": target.friendly_name,
                                    "media": media_file.filename,
                                    "protocol": "chromecast",
                                    "compatibility": compat_result,
                                })),
                            )
                        }
                        Err(e) => (
                            StatusCode::BAD_REQUEST,
                            axum::Json(
                                serde_json::json!({ "error": format!("Chromecast load error: {}", e) }),
                            ),
                        ),
                    }
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(
                        serde_json::json!({ "error": format!("Chromecast connection error: {}", e) }),
                    ),
                ),
            }
        }
        TargetKind::AirPlay => {
            let client = crate::airplay::client::AirPlayClient::new(target.address);
            match client.play(&media_url, 0.0).await {
                Ok(()) => {
                    let mut casts = state.active_casts.lock().await;
                    casts.insert(target.friendly_name.clone(), media_file.filename.clone());
                    (
                        StatusCode::OK,
                        axum::Json(serde_json::json!({
                            "status": "casting",
                            "target": target.friendly_name,
                            "media": media_file.filename,
                            "protocol": "airplay",
                            "compatibility": compat_result,
                        })),
                    )
                }
                Err(e) => (
                    StatusCode::BAD_REQUEST,
                    axum::Json(serde_json::json!({ "error": format!("AirPlay error: {}", e) })),
                ),
            }
        }
    }
}

/// Request body for cast control actions.
#[derive(serde::Deserialize)]
pub struct ApiCastControlRequest {
    pub target_id: String,
    /// One of: "pause", "resume", "stop"
    pub action: String,
}

/// Control playback on a cast target (pause, resume, stop).
pub async fn api_cast_control<D: DatabaseManager>(
    State(_state): State<AppState<D>>,
    axum::Json(payload): axum::Json<ApiCastControlRequest>,
) -> impl IntoResponse {
    // For now, return which action was requested. Full session tracking
    // (maintaining persistent connections to Chromecast/AirPlay devices)
    // is planned for Phase 2.
    match payload.action.as_str() {
        "pause" | "resume" | "stop" => (
            StatusCode::OK,
            axum::Json(serde_json::json!({
                "status": "ok",
                "action": payload.action,
                "target_id": payload.target_id,
            })),
        ),
        _ => (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({
                "error": format!("Unknown action: {}", payload.action),
            })),
        ),
    }
}

//! Shared cast orchestration.
//!
//! These were reachable only through `web::mcp` even though the HTTP casting
//! API is their other caller, which made `casting` unbuildable without `mcp`.
//! They live with casting now, and MCP re-exports them.

use crate::casting::{PlaybackAction, PlaybackItem, PlaybackState};
use crate::database::{
    DatabaseManager, DatabaseReadSession, FileLocation, MediaFileQuery, MediaFileView,
};
use crate::state::AppState;
use tracing::{debug, warn};
use uuid::Uuid;

pub async fn cast_file_helper<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    file_id: i64,
    renderer_id: &str,
) -> Result<serde_json::Value, String> {
    let file = state
        .database
        .get_file_location_by_id(file_id)
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or(format!("File with ID {} not found", file_id))?;

    let renderers = state
        .discovered_tvs
        .get_or_refresh()
        .await
        .map_err(|e| format!("Renderer discovery error: {}", e))?;

    let matched_tv = renderers
        .iter()
        .find(|renderer| renderer.id == renderer_id)
        .ok_or(format!(
            "No renderer found with ID '{}'. Available renderers: {}",
            renderer_id,
            renderers
                .iter()
                .map(|tv| tv.friendly_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))?;

    // Build the media URL
    let origin = state
        .advertised_http_origin_for_peer(&matched_tv.location_url)
        .await;
    let media_url = playback_url(&file, &origin);

    let item = playback_item(&file, &origin);
    state.discovered_tvs.validate(matched_tv, &item)?;
    state
        .discovered_tvs
        .play(matched_tv, &item)
        .await
        .map_err(|e| format!("Cast error: {}", e))?;

    {
        let mut monitors = state.active_monitors.lock().await;
        if let Some((_, cancellation)) = monitors.remove(&matched_tv.id) {
            cancellation.cancel();
        }
    }
    {
        let mut casts = state.active_casts.lock().await;
        casts.insert_labeled(
            matched_tv.id.clone(),
            matched_tv.friendly_name.clone(),
            file.filename.clone(),
        );
    }

    Ok(serde_json::json!({
        "status": "playing",
        "file": file.filename,
        "renderer": matched_tv.friendly_name,
        "renderer_id": matched_tv.id,
        "protocol": matched_tv.protocol,
        "media_url": media_url
    }))
}

pub(crate) async fn tool_control_renderer<D: DatabaseManager>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let renderer_id = args
        .get("renderer_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'renderer_id' parameter")?;

    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'action' parameter")?;

    let playback_action = match action {
        "play" => PlaybackAction::Play,
        "pause" => PlaybackAction::Pause,
        "stop" => PlaybackAction::Stop,
        _ => {
            return Err(format!(
                "Unknown action '{}'. Use play, pause, or stop.",
                action
            ))
        }
    };

    let renderers = state
        .discovered_tvs
        .get_or_refresh()
        .await
        .map_err(|e| format!("Renderer discovery error: {}", e))?;

    let matched_tv = renderers
        .iter()
        .find(|renderer| renderer.id == renderer_id)
        .ok_or(format!(
            "No renderer found with ID '{}'. Available renderers: {}",
            renderer_id,
            renderers
                .iter()
                .map(|tv| tv.friendly_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))?;

    state
        .discovered_tvs
        .control(matched_tv, playback_action)
        .await
        .map_err(|e| format!("Control error: {}", e))?;

    Ok(serde_json::json!({
        "status": "ok",
        "action": action,
        "renderer": matched_tv.friendly_name,
        "renderer_id": matched_tv.id,
        "protocol": matched_tv.protocol
    }))
}
pub async fn cached_renderers<D: DatabaseManager>(
    state: &AppState<D>,
) -> Result<Vec<crate::casting::RendererDevice>, String> {
    state
        .discovered_tvs
        .get_or_refresh()
        .await
        .map_err(|e| format!("TV discovery error: {}", e))
}

pub(crate) fn playback_item(file: &FileLocation, origin: &str) -> PlaybackItem {
    PlaybackItem {
        id: file.id,
        url: playback_url(file, origin),
        local_path: file.path.clone(),
        title: file.title.clone().unwrap_or_else(|| file.filename.clone()),
        filename: file.filename.clone(),
        mime_type: file.mime_type.clone(),
    }
}

fn playback_url(file: &FileLocation, origin: &str) -> String {
    let extension = std::path::Path::new(&file.filename)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 16
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        });
    match extension {
        Some(extension) => format!("{origin}/media/{}.{extension}", file.id),
        None => format!("{origin}/media/{}", file.id),
    }
}

pub(crate) async fn playlist_file_locations<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    playlist_id: i64,
) -> Result<Vec<FileLocation>, String> {
    state
        .database
        .clone()
        .read(move |session| {
            let mut tracks = Vec::new();
            session.visit_files(
                &MediaFileQuery::Playlist(playlist_id),
                0,
                usize::MAX,
                |file| {
                    if let Some(location) = file.to_file_location() {
                        tracks.push(location);
                    }
                    Ok(())
                },
            )?;
            Ok(tracks)
        })
        .await
        .map_err(|error| format!("Database error: {error}"))
}

pub async fn cast_playlist_helper<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    playlist_id: i64,
    renderer_id: &str,
    track_index: usize,
) -> Result<serde_json::Value, String> {
    let tracks = playlist_file_locations(state, playlist_id).await?;
    let mut result = cast_tracks_helper(state, tracks, renderer_id, track_index).await?;
    if let Some(object) = result.as_object_mut() {
        object.insert("playlist_id".to_string(), serde_json::json!(playlist_id));
    }
    Ok(result)
}

pub async fn cast_tracks_helper<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    tracks: Vec<FileLocation>,
    renderer_id: &str,
    track_index: usize,
) -> Result<serde_json::Value, String> {
    if tracks.is_empty() {
        return Err("Cannot cast an empty playlist".to_string());
    }

    if track_index >= tracks.len() {
        return Err(format!(
            "track_index {} is out of bounds (playlist only has {} tracks)",
            track_index,
            tracks.len()
        ));
    }

    let selected_track = &tracks[track_index];

    let renderers = cached_renderers(state).await?;

    let matched_tv = renderers
        .iter()
        .find(|renderer| renderer.id == renderer_id)
        .ok_or(format!(
            "No renderer found with ID '{}'. Available renderers: {}",
            renderer_id,
            renderers
                .iter()
                .map(|tv| tv.friendly_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))?;

    // Build the media URL
    let origin = state
        .advertised_http_origin_for_peer(&matched_tv.location_url)
        .await;
    let media_url = playback_url(selected_track, &origin);

    let playback_items = tracks
        .iter()
        .map(|track| playback_item(track, &origin))
        .collect::<Vec<_>>();
    for item in playback_items.iter().skip(track_index) {
        state.discovered_tvs.validate(matched_tv, item)?;
    }
    state
        .discovered_tvs
        .play(matched_tv, &playback_items[track_index])
        .await
        .map_err(|e| format!("Cast error: {}", e))?;

    // Register active cast in global state
    {
        let mut casts = state.active_casts.lock().await;
        casts.insert_labeled(
            matched_tv.id.clone(),
            matched_tv.friendly_name.clone(),
            selected_track.filename.clone(),
        );
    }

    // Cancel existing monitor for this TV if any
    {
        let mut monitors = state.active_monitors.lock().await;
        if let Some((_, cancellation)) = monitors.remove(&matched_tv.id) {
            cancellation.cancel();
        }
    }

    // Hand the renderer as much of the queue as it will take. A renderer with a
    // real queue (AirPlay audio) accepts the whole remainder and advances on its
    // own; one without accepts nothing and the status monitor below drives the
    // transitions instead.
    let mut queued_file = None;
    let mut queued_count = 0usize;
    for next_item in playback_items.iter().skip(track_index + 1) {
        match state.discovered_tvs.queue_next(matched_tv, next_item).await {
            Ok(true) => {
                if queued_file.is_none() {
                    queued_file = Some(next_item.filename.clone());
                }
                queued_count += 1;
            }
            Ok(false) => break,
            Err(error) => {
                tracing::warn!(%error, "Renderer does not support native next-item queueing");
                break;
            }
        }
    }
    if queued_count > 0 {
        tracing::info!(queued_count, "Queued tracks on the renderer");
    }

    // Spawn new queue monitor to dynamically handle subsequent track transitions
    let monitor_id = Uuid::new_v4();
    let monitor_cancellation = state.cancellation.child_token();
    {
        let mut monitors = state.active_monitors.lock().await;
        if monitors.len() >= crate::runtime_state::ACTIVE_CAST_MAX_ENTRIES
            && !monitors.contains_key(&matched_tv.id)
        {
            if let Some(oldest_key) = monitors.keys().next().cloned() {
                if let Some((_, oldest)) = monitors.remove(&oldest_key) {
                    oldest.cancel();
                }
            }
        }
        monitors.insert(
            matched_tv.id.clone(),
            (monitor_id, monitor_cancellation.clone()),
        );
    }

    let state_clone = state.clone();
    let matched_renderer = matched_tv.clone();
    let matched_tv_friendly_name = matched_tv.friendly_name.clone();
    let matched_renderer_id = matched_tv.id.clone();
    let monitor_items = playback_items.clone();

    state.background_tasks.spawn(async move {
        let mut current_idx = track_index;
        let mut consecutive_stopped = 0;

        'monitor: loop {
            tokio::select! {
                _ = monitor_cancellation.cancelled() => {
                    debug!("Queue monitor cancelled for renderer: {}", matched_renderer_id);
                    break;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(4)) => {}
            }

            let status = match tokio::select! {
                _ = monitor_cancellation.cancelled() => break,
                result = state_clone.discovered_tvs.status(&matched_renderer) => result,
            } {
                Ok(status) => status,
                Err(error) => {
                    debug!(%error, "Queue monitor failed to get renderer status");
                    continue;
                }
            };

            let reported_idx = status
                .current_url
                .as_deref()
                .and_then(|url| monitor_items.iter().position(|item| item.url == url));
            if let Some(idx) = reported_idx.filter(|idx| *idx != current_idx) {
                current_idx = idx;
                update_active_cast(
                    &state_clone,
                    &matched_renderer_id,
                    &matched_tv_friendly_name,
                    &monitor_items[current_idx].filename,
                )
                .await;
                queue_following_item(
                    &state_clone,
                    &matched_renderer,
                    &monitor_items,
                    current_idx,
                    &monitor_cancellation,
                )
                .await;
            }

            match status.state {
                PlaybackState::Finished if current_idx + 1 < monitor_items.len() => {
                    current_idx += 1;
                    let next = &monitor_items[current_idx];
                    let play_result = tokio::select! {
                        _ = monitor_cancellation.cancelled() => break 'monitor,
                        result = state_clone.discovered_tvs.play(&matched_renderer, next) => result,
                    };
                    if let Err(error) = play_result {
                        warn!(%error, "Queue monitor failed to start the next item");
                        break;
                    }
                    update_active_cast(
                        &state_clone,
                        &matched_renderer_id,
                        &matched_tv_friendly_name,
                        &next.filename,
                    )
                    .await;
                    queue_following_item(
                        &state_clone,
                        &matched_renderer,
                        &monitor_items,
                        current_idx,
                        &monitor_cancellation,
                    )
                    .await;
                    consecutive_stopped = 0;
                }
                PlaybackState::Finished => break,
                PlaybackState::Stopped | PlaybackState::Error if reported_idx.is_none() => {
                    consecutive_stopped += 1;
                    if consecutive_stopped >= 5 {
                        break;
                    }
                }
                _ => consecutive_stopped = 0,
            }
        }

        let removed_current_monitor = {
            let mut monitors = state_clone.active_monitors.lock().await;
            let is_current = monitors
                .get(&matched_renderer_id)
                .is_some_and(|(current_id, _)| *current_id == monitor_id);
            if is_current {
                monitors.remove(&matched_renderer_id);
            }
            is_current
        };
        // A replaced monitor must not clear the newer cast's telemetry.
        if removed_current_monitor {
            let mut casts = state_clone.active_casts.lock().await;
            casts.remove(&matched_renderer_id);
        }
    });

    Ok(serde_json::json!({
        "status": "playing",
        "tracks_count": tracks.len(),
        "current_index": track_index,
        "current_file": selected_track.filename,
        "queued_next_file": queued_file,
        "renderer": matched_tv.friendly_name,
        "renderer_id": matched_tv.id,
        "protocol": matched_tv.protocol,
        "media_url": media_url
    }))
}

pub(crate) async fn update_active_cast<D: DatabaseManager>(
    state: &AppState<D>,
    renderer_id: &str,
    renderer_name: &str,
    filename: &str,
) {
    state.active_casts.lock().await.insert_labeled(
        renderer_id.to_string(),
        renderer_name.to_string(),
        filename.to_string(),
    );
}

pub(crate) async fn queue_following_item<D: DatabaseManager>(
    state: &AppState<D>,
    renderer: &crate::casting::RendererDevice,
    items: &[PlaybackItem],
    current_index: usize,
    cancellation: &tokio_util::sync::CancellationToken,
) {
    let Some(next) = items.get(current_index + 1) else {
        return;
    };
    let result = tokio::select! {
        _ = cancellation.cancelled() => return,
        result = state.discovered_tvs.queue_next(renderer, next) => result,
    };
    match result {
        Ok(true) => debug!(filename = %next.filename, "Queued next renderer item"),
        Ok(false) => {}
        Err(error) => warn!(%error, "Failed to queue next renderer item"),
    }
}

pub(crate) async fn tool_cast_playlist_to_renderer<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let playlist_id = args
        .get("playlist_id")
        .and_then(|v| v.as_i64())
        .ok_or("Missing 'playlist_id' parameter")?;

    let renderer_id = args
        .get("renderer_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'renderer_id' parameter")?;

    let track_index = args
        .get("track_index")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    cast_playlist_helper(state, playlist_id, renderer_id, track_index).await
}

// ──────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────

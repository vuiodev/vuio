//! TV discovery and dashboard playlist-casting API handlers.

use crate::{database::PlaylistRepository, state::AppState};
use axum::{extract::State, http::StatusCode, response::IntoResponse};
use tracing::error;

#[derive(serde::Deserialize)]
pub struct ApiCastPlaylistRequest {
    pub renderer_id: String,
    pub folder_name: String,
    pub file_ids: Vec<i64>,
}

/// Discover UPnP/DLNA TVs and return their friendly names in JSON format
pub async fn api_list_renderers(State(state): State<AppState>) -> impl IntoResponse {
    match state.discovered_tvs.get_or_refresh().await {
        Ok(renderers) => (StatusCode::OK, axum::Json(renderers)),
        Err(e) => {
            error!(error = %e, "TV discovery request failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(Vec::<crate::tv_control::DiscoveredTv>::new()),
            )
        }
    }
}

/// Create a temporary playlist with the provided video files and cast it to the TV
pub async fn api_cast_playlist(
    State(state): State<AppState>,
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
            )
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
            )
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

    // Cast the playlist starting at index 0 using our shared helper.
    match crate::web::mcp::cast_playlist_helper(&state, playlist_id, &payload.renderer_id, 0).await {
        Ok(result) => (StatusCode::OK, axum::Json(result)),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({ "error": format!("Cast error: {}", e) })),
        ),
    }
}

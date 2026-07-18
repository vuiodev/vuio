//! TV discovery and dashboard playlist-casting API handlers.

use crate::{database::PlaylistRepository, state::AppState};
use axum::{extract::State, http::StatusCode, response::IntoResponse};
use tracing::error;

#[derive(serde::Deserialize)]
pub struct ApiCastPlaylistRequest {
    pub tv_name: String,
    pub folder_name: String,
    pub file_ids: Vec<i64>,
}

/// Discover UPnP/DLNA TVs and return their friendly names in JSON format
pub async fn api_list_tvs() -> impl IntoResponse {
    match crate::tv_control::discover_tvs().await {
        Ok(tvs) => {
            let names: Vec<String> = tvs.into_iter().map(|tv| tv.friendly_name).collect();
            (StatusCode::OK, axum::Json(names))
        }
        Err(e) => {
            error!(error = %e, "TV discovery request failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(vec!["Internal Server Error".to_string()]),
            )
        }
    }
}

/// Create a temporary playlist with the provided video files and cast it to the TV
pub async fn api_cast_playlist(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<ApiCastPlaylistRequest>,
) -> impl IntoResponse {
    // 1. List playlists to find and delete old "Web Cast - " playlists
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

    for pl in playlists {
        if pl.name.starts_with("Web Cast - ") {
            if let Some(id) = pl.id {
                let _ = state.database.delete_playlist(id).await;
            }
        }
    }

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
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({ "error": "Internal Server Error" })),
        );
    }

    // 4. Cast the playlist starting at index 0 using our shared helper
    match crate::web::mcp::cast_playlist_helper(&state, playlist_id, &payload.tv_name, 0).await {
        Ok(result) => (StatusCode::OK, axum::Json(result)),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({ "error": format!("Cast error: {}", e) })),
        ),
    }
}

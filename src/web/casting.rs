//! TV discovery and dashboard playlist-casting API handlers.

use crate::{database::PlaylistRepository, state::AppState};
use axum::{extract::State, http::StatusCode, response::IntoResponse};

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
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(vec![format!("Discovery error: {}", e)]),
        ),
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
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({ "error": format!("Database error: {}", e) })),
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
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(
                    serde_json::json!({ "error": format!("Failed to create playlist: {}", e) }),
                ),
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
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({ "error": format!("Failed to add tracks: {}", e) })),
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

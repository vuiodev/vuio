//! TV discovery and dashboard playlist-casting API handlers.

use crate::{
    database::{DatabaseManager, FileLocation, MediaFileView},
    state::AppState,
};
use axum::{extract::State, http::StatusCode, response::IntoResponse};
use std::{collections::HashSet, path::PathBuf};
use tracing::error;

#[derive(serde::Deserialize)]
pub struct ApiCastPlaylistRequest {
    pub renderer_id: String,
    pub folder_name: String,
    pub file_ids: Vec<i64>,
}

#[derive(serde::Deserialize)]
pub struct ApiCastRequest {
    pub renderer_id: String,
    pub source: ApiCastSource,
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApiCastSource {
    File { file_id: i64 },
    Folder { components: Vec<String> },
}

/// Discover UPnP/DLNA TVs and return their friendly names in JSON format
pub async fn api_list_renderers<D: DatabaseManager>(
    State(state): State<AppState<D>>,
) -> impl IntoResponse {
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

pub async fn api_cast<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    axum::Json(payload): axum::Json<ApiCastRequest>,
) -> impl IntoResponse {
    let result = match payload.source {
        ApiCastSource::File { file_id } => {
            let file = match state.database.get_file_location_by_id(file_id).await {
                Ok(Some(file)) if file.mime_type.starts_with("video/") => file,
                Ok(Some(_)) => {
                    return cast_error(StatusCode::BAD_REQUEST, "Only video files can be cast");
                }
                Ok(None) => return cast_error(StatusCode::NOT_FOUND, "Video file not found"),
                Err(error) => {
                    error!(%error, file_id, "Failed to load video for casting");
                    return cast_error(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error");
                }
            };
            crate::web::mcp::cast_file_helper(&state, file.id, &payload.renderer_id).await
        }
        ApiCastSource::Folder { components } => {
            let tracks = match resolve_video_folder(&state, &components).await {
                Ok(tracks) if !tracks.is_empty() => tracks,
                Ok(_) => {
                    return cast_error(
                        StatusCode::BAD_REQUEST,
                        "No video files found in this folder",
                    );
                }
                Err(message) => return cast_error(StatusCode::BAD_REQUEST, &message),
            };
            crate::web::mcp::cast_tracks_helper(&state, tracks, &payload.renderer_id, 0).await
        }
    };

    match result {
        Ok(value) => (StatusCode::OK, axum::Json(value)),
        Err(message) => cast_error(StatusCode::BAD_REQUEST, &message),
    }
}

fn cast_error(status: StatusCode, message: &str) -> (StatusCode, axum::Json<serde_json::Value>) {
    (status, axum::Json(serde_json::json!({ "error": message })))
}

async fn resolve_video_folder<D: DatabaseManager>(
    state: &AppState<D>,
    components: &[String],
) -> Result<Vec<FileLocation>, String> {
    if components.len() > 64
        || components.iter().any(|component| {
            component.is_empty()
                || component == "."
                || component == ".."
                || component.contains('/')
                || component.contains('\\')
        })
    {
        return Err("Invalid folder path".to_string());
    }

    let directories = state.media_directories.read().await.clone();
    let mut videos = Vec::new();
    let mut seen = HashSet::new();
    for directory in directories {
        let root = PathBuf::from(&directory.path);
        let folder = components
            .iter()
            .fold(root.clone(), |path, component| path.join(component));
        let files = state
            .database
            .get_files_with_path_prefix(folder.to_string_lossy().as_ref())
            .await
            .map_err(|error| format!("Database error: {error}"))?;
        for file in files {
            if !file.mime_type().starts_with("video/") {
                continue;
            }
            let Some(location) = file.to_file_location() else {
                continue;
            };
            if seen.insert(location.id) {
                let relative = file
                    .path
                    .strip_prefix(&root)
                    .unwrap_or(&file.path)
                    .to_string_lossy()
                    .into_owned();
                videos.push((relative, location));
            }
        }
    }
    videos.sort_by(|left, right| crate::natural_cmp(&left.0, &right.0));
    Ok(videos.into_iter().map(|(_, file)| file).collect())
}



async fn create_and_cast_playlist<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    renderer_id: &str,
    folder_name: &str,
    file_ids: &[i64],
) -> Result<serde_json::Value, String> {
    let playlist_name = format!("Web Cast - {folder_name}");
    let playlist_id = state
        .database
        .create_playlist(&playlist_name, None)
        .await
        .map_err(|error| format!("Failed to create cast playlist: {error}"))?;
    let tracks = file_ids
        .iter()
        .enumerate()
        .map(|(position, id)| (*id, position as u32))
        .collect::<Vec<_>>();
    if let Err(error) = state
        .database
        .batch_add_to_playlist(playlist_id, &tracks)
        .await
    {
        let _ = state.database.delete_playlist(playlist_id).await;
        return Err(format!("Failed to create cast playlist: {error}"));
    }
    crate::web::eventing::publish_content_change(state).await;
    match crate::web::mcp::cast_playlist_helper(state, playlist_id, renderer_id, 0).await {
        Ok(value) => Ok(value),
        Err(error) => {
            let _ = state.database.delete_playlist(playlist_id).await;
            crate::web::eventing::publish_content_change(state).await;
            Err(error)
        }
    }
}

/// Create a temporary playlist with the provided video files and cast it to the TV
pub async fn api_cast_playlist<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    axum::Json(payload): axum::Json<ApiCastPlaylistRequest>,
) -> impl IntoResponse {
    match create_and_cast_playlist(
        &state,
        &payload.renderer_id,
        &payload.folder_name,
        &payload.file_ids,
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

#[cfg(test)]
mod tests {

    #[test]
    fn episode_paths_use_natural_order() {
        let mut paths = [
            "Series/Episode 10.mkv",
            "Series/Episode 2.mkv",
            "Series/Episode 01.mkv",
        ];
        paths.sort_by(|left, right| crate::natural_cmp(left, right));
        assert_eq!(
            paths,
            [
                "Series/Episode 01.mkv",
                "Series/Episode 2.mkv",
                "Series/Episode 10.mkv"
            ]
        );
    }

    #[test]
    fn natural_order_is_case_insensitive() {
        assert_eq!(crate::natural_cmp("series/A.mkv", "Series/b.mkv"), std::cmp::Ordering::Less);
    }
}

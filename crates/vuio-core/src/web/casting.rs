//! TV discovery and dashboard playlist-casting API handlers.

pub(crate) mod helpers;
pub use helpers::{cast_file_helper, cast_playlist_helper, cast_tracks_helper};

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
pub struct ApiCastControlRequest {
    pub renderer_id: String,
    /// `play`, `pause` or `stop`.
    pub action: String,
}

#[derive(serde::Deserialize)]
pub struct ApiPairingStartRequest {
    pub renderer_id: String,
}

#[derive(serde::Deserialize)]
pub struct ApiPairingFinishRequest {
    pub renderer_id: String,
    pub challenge_id: String,
    pub pin: String,
}

#[derive(serde::Deserialize)]
pub struct ApiPairingForgetRequest {
    pub renderer_id: String,
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApiCastSource {
    File {
        file_id: i64,
    },
    Folder {
        components: Vec<String>,
        /// Restrict the queue to one media kind. Casting an audio folder should
        /// not drag in the videos sitting beside it.
        #[serde(default)]
        media: Option<String>,
    },
}

/// Discover supported playback devices and return their public details as JSON.
pub async fn api_list_renderers<D: DatabaseManager>(
    State(state): State<AppState<D>>,
) -> impl IntoResponse {
    match state.discovered_tvs.get_or_refresh().await {
        Ok(renderers) => (StatusCode::OK, axum::Json(renderers)),
        Err(e) => {
            error!(error = %e, "TV discovery request failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(Vec::<crate::casting::RendererDevice>::new()),
            )
        }
    }
}

async fn renderer_by_id<D: DatabaseManager>(
    state: &AppState<D>,
    renderer_id: &str,
) -> Result<crate::casting::RendererDevice, (StatusCode, axum::Json<serde_json::Value>)> {
    let renderers = state
        .discovered_tvs
        .get_or_refresh()
        .await
        .map_err(|error| {
            error!(%error, "Renderer lookup failed");
            cast_error(StatusCode::INTERNAL_SERVER_ERROR, "Device discovery failed")
        })?;
    renderers
        .into_iter()
        .find(|renderer| renderer.id == renderer_id)
        .ok_or_else(|| cast_error(StatusCode::NOT_FOUND, "Playback device not found"))
}

pub async fn api_pairing_start<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    axum::Json(payload): axum::Json<ApiPairingStartRequest>,
) -> impl IntoResponse {
    let renderer = match renderer_by_id(&state, &payload.renderer_id).await {
        Ok(renderer) => renderer,
        Err(response) => return response.into_response(),
    };
    match state.discovered_tvs.begin_pairing(&renderer).await {
        Ok(challenge) => (StatusCode::OK, axum::Json(serde_json::json!(challenge))).into_response(),
        Err(error) => {
            error!(%error, renderer_id = %renderer.id, "AirPlay pairing start failed");
            cast_error(StatusCode::BAD_REQUEST, &error.to_string()).into_response()
        }
    }
}

pub async fn api_pairing_finish<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    axum::Json(payload): axum::Json<ApiPairingFinishRequest>,
) -> impl IntoResponse {
    let renderer = match renderer_by_id(&state, &payload.renderer_id).await {
        Ok(renderer) => renderer,
        Err(response) => return response.into_response(),
    };
    match state
        .discovered_tvs
        .finish_pairing(renderer.protocol, &payload.challenge_id, payload.pin.trim())
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            axum::Json(serde_json::json!({ "paired": true })),
        )
            .into_response(),
        Err(error) => {
            error!(%error, renderer_id = %renderer.id, "AirPlay pairing finish failed");
            cast_error(StatusCode::BAD_REQUEST, &error.to_string()).into_response()
        }
    }
}

pub async fn api_pairing_forget<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    axum::Json(payload): axum::Json<ApiPairingForgetRequest>,
) -> impl IntoResponse {
    let renderer = match renderer_by_id(&state, &payload.renderer_id).await {
        Ok(renderer) => renderer,
        Err(response) => return response.into_response(),
    };
    match state.discovered_tvs.forget_pairing(&renderer).await {
        Ok(removed) => (
            StatusCode::OK,
            axum::Json(serde_json::json!({ "removed": removed })),
        )
            .into_response(),
        Err(error) => {
            error!(%error, renderer_id = %renderer.id, "AirPlay pairing removal failed");
            cast_error(StatusCode::BAD_REQUEST, &error.to_string()).into_response()
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
                Ok(Some(file)) if is_castable_mime(&file.mime_type) => file,
                Ok(Some(_)) => {
                    return cast_error(
                        StatusCode::BAD_REQUEST,
                        "Only video and audio files can be cast",
                    );
                }
                Ok(None) => return cast_error(StatusCode::NOT_FOUND, "Video file not found"),
                Err(error) => {
                    error!(%error, file_id, "Failed to load video for casting");
                    return cast_error(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error");
                }
            };
            cast_file_helper(&state, file.id, &payload.renderer_id).await
        }
        ApiCastSource::Folder { components, media } => {
            let tracks = match resolve_castable_folder(&state, &components, media.as_deref()).await
            {
                Ok(tracks) if !tracks.is_empty() => tracks,
                Ok(_) => {
                    return cast_error(
                        StatusCode::BAD_REQUEST,
                        "No castable media found in this folder",
                    );
                }
                Err(message) => return cast_error(StatusCode::BAD_REQUEST, &message),
            };
            cast_tracks_helper(&state, tracks, &payload.renderer_id, 0).await
        }
    };

    match result {
        Ok(value) => (StatusCode::OK, axum::Json(value)),
        Err(message) => cast_error(StatusCode::BAD_REQUEST, &message),
    }
}

/// Control what is already playing on a renderer.
///
/// Stopping matters for push protocols like AirPlay audio: the sender has to
/// tear the session down, otherwise the receiver keeps rendering.
pub async fn api_cast_control<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    axum::Json(payload): axum::Json<ApiCastControlRequest>,
) -> impl IntoResponse {
    let action = match payload.action.to_ascii_lowercase().as_str() {
        "play" => crate::casting::PlaybackAction::Play,
        "pause" => crate::casting::PlaybackAction::Pause,
        "stop" => crate::casting::PlaybackAction::Stop,
        other => {
            return cast_error(
                StatusCode::BAD_REQUEST,
                &format!("Unknown action '{other}'. Use play, pause or stop."),
            );
        }
    };

    let renderers = match state.discovered_tvs.get_or_refresh().await {
        Ok(renderers) => renderers,
        Err(error) => {
            return cast_error(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string());
        }
    };
    let Some(renderer) = renderers
        .iter()
        .find(|renderer| renderer.id == payload.renderer_id)
    else {
        return cast_error(StatusCode::NOT_FOUND, "No renderer found with that ID");
    };

    match state.discovered_tvs.control(renderer, action).await {
        Ok(()) => (
            StatusCode::OK,
            axum::Json(serde_json::json!({
                "status": "ok",
                "action": payload.action,
                "renderer": renderer.friendly_name,
            })),
        ),
        Err(error) => cast_error(StatusCode::BAD_REQUEST, &error.to_string()),
    }
}

fn cast_error(status: StatusCode, message: &str) -> (StatusCode, axum::Json<serde_json::Value>) {
    (status, axum::Json(serde_json::json!({ "error": message })))
}

async fn resolve_castable_folder<D: DatabaseManager>(
    state: &AppState<D>,
    components: &[String],
    media: Option<&str>,
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
            if !is_castable_mime(file.mime_type()) || !matches_media_kind(file.mime_type(), media) {
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

/// Whether a MIME type belongs to the requested media kind.
///
/// `None` accepts anything castable, which is what the folder API does when a
/// caller does not care.
pub(crate) fn matches_media_kind(mime: &str, media: Option<&str>) -> bool {
    match media {
        Some("audio") => mime.starts_with("audio/"),
        Some("video") => !mime.starts_with("audio/"),
        _ => true,
    }
}

/// Media the cast API will hand to a provider.
///
/// Audio is included because AirPlay receivers take an RTP audio stream even
/// when they cannot play video. Each provider still validates the item, so a
/// renderer that cannot take audio rejects it with its own message.
pub(crate) fn is_castable_mime(mime: &str) -> bool {
    let base = mime.split(';').next().unwrap_or(mime).trim();
    mime.starts_with("video/")
        || (base.starts_with("audio/") && base != "audio/radio")
        || matches!(
            base,
            "application/vnd.apple.mpegurl" | "application/x-mpegurl"
        )
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
    match cast_playlist_helper(state, playlist_id, renderer_id, 0).await {
        Ok(value) => Ok(value),
        Err(error) => {
            let _ = state.database.delete_playlist(playlist_id).await;
            crate::web::eventing::publish_content_change(state).await;
            Err(error)
        }
    }
}

/// Create a temporary playlist with the provided video files and cast it to the device.
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
        assert_eq!(
            crate::natural_cmp("series/A.mkv", "Series/b.mkv"),
            std::cmp::Ordering::Less
        );
    }
}

use crate::{
    database::playlist_formats::{PlaylistFileManager, PlaylistFormat},
    error::AppError,
    state::AppState,
};
use axum::{
    body::Body,
    extract::{Multipart, Path as AxumPath, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tracing::{error, info, warn};

/// Request to create a new playlist
#[derive(Deserialize)]
pub struct CreatePlaylistRequest {
    pub name: String,
    pub description: Option<String>,
}

/// Response after creating a playlist
#[derive(Serialize)]
pub struct CreatePlaylistResponse {
    pub id: i64,
    pub message: String,
}

/// Query parameters for playlist export
#[derive(Deserialize)]
pub struct ExportPlaylistQuery {
    pub format: Option<String>, // "m3u" or "pls"
}

/// Response for playlist operations
#[derive(Serialize)]
pub struct PlaylistOperationResponse {
    pub success: bool,
    pub message: String,
    pub playlist_id: Option<i64>,
}

/// Response for listing playlists
#[derive(Serialize)]
pub struct PlaylistInfo {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub track_count: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct ListPlaylistsResponse {
    pub playlists: Vec<PlaylistInfo>,
}

#[derive(Deserialize)]
pub struct ScanPlaylistsRequest {
    pub directory: String,
}

#[derive(Serialize)]
pub struct ScanPlaylistsResponse {
    pub imported_count: usize,
    pub playlist_ids: Vec<i64>,
    pub message: String,
}

/// Create a new empty playlist
pub async fn create_playlist(
    State(state): State<AppState>,
    Json(request): Json<CreatePlaylistRequest>,
) -> Result<Json<CreatePlaylistResponse>, AppError> {
    info!("Creating new playlist: {}", request.name);

    let playlist_id = state
        .database
        .create_playlist(&request.name, request.description.as_deref())
        .await
        .map_err(|e| {
            error!("Failed to create playlist: {}", e);
            AppError::Internal(e)
        })?;

    Ok(Json(CreatePlaylistResponse {
        id: playlist_id,
        message: format!("Playlist '{}' created successfully", request.name),
    }))
}

/// List all playlists
pub async fn list_playlists(
    State(state): State<AppState>,
) -> Result<Json<ListPlaylistsResponse>, AppError> {
    let playlists = state.database.get_playlists().await.map_err(|e| {
        error!("Failed to get playlists: {}", e);
        AppError::Internal(e)
    })?;

    let mut playlist_infos = Vec::new();
    for playlist in playlists {
        // Get track count for each playlist
        let track_count = match state
            .database
            .get_playlist_tracks(playlist.id.unwrap_or(0))
            .await
        {
            Ok(tracks) => tracks.len(),
            Err(_) => 0,
        };

        playlist_infos.push(PlaylistInfo {
            id: playlist.id.unwrap_or(0),
            name: playlist.name,
            description: playlist.description,
            track_count,
            created_at: format!("{:?}", playlist.created_at),
            updated_at: format!("{:?}", playlist.updated_at),
        });
    }

    Ok(Json(ListPlaylistsResponse {
        playlists: playlist_infos,
    }))
}

/// Import a playlist from an uploaded file
pub async fn import_playlist(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<PlaylistOperationResponse>, AppError> {
    let mut playlist_name: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;

    // Parse multipart form data
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        error!("Failed to read multipart field: {}", e);
        AppError::InvalidInput("Invalid multipart data".to_string())
    })? {
        let field_name = field.name().unwrap_or("").to_string();
        
        match field_name.as_str() {
            "name" => {
                playlist_name = Some(field.text().await.map_err(|e| {
                    error!("Failed to read playlist name: {}", e);
                    AppError::InvalidInput("Invalid playlist name".to_string())
                })?);
            }
            "file" => {
                filename = field.file_name().map(|s| s.to_string());
                file_data = Some(field.bytes().await.map_err(|e| {
                    error!("Failed to read file data: {}", e);
                    AppError::InvalidInput("Invalid file data".to_string())
                })?.to_vec());
            }
            _ => {
                warn!("Unknown multipart field: {}", field_name);
            }
        }
    }

    let file_data = file_data.ok_or_else(|| {
        AppError::InvalidInput("No file provided".to_string())
    })?;

    let filename = filename.ok_or_else(|| {
        AppError::InvalidInput("No filename provided".to_string())
    })?;

    // Write file to temporary location
    let temp_path = std::env::temp_dir().join(&filename);
    fs::write(&temp_path, file_data).await.map_err(|e| {
        error!("Failed to write temporary file: {}", e);
        AppError::Io(e)
    })?;

    // Import the playlist
    let result = state
        .database
        .import_playlist_file(&temp_path, playlist_name)
        .await;

    // Clean up temporary file
    let _ = fs::remove_file(&temp_path).await;

    match result {
        Ok(playlist_id) => {
            info!("Successfully imported playlist from file: {}", filename);
            Ok(Json(PlaylistOperationResponse {
                success: true,
                message: format!("Playlist imported successfully from {}", filename),
                playlist_id: Some(playlist_id),
            }))
        }
        Err(e) => {
            error!("Failed to import playlist from {}: {}", filename, e);
            Err(AppError::Internal(e))
        }
    }
}

/// Export a playlist to a file
pub async fn export_playlist(
    State(state): State<AppState>,
    AxumPath(playlist_id): AxumPath<i64>,
    Query(query): Query<ExportPlaylistQuery>,
) -> Result<Response, AppError> {
    let format = match query.format.as_deref() {
        Some("m3u") => PlaylistFormat::M3U,
        Some("pls") => PlaylistFormat::PLS,
        _ => PlaylistFormat::M3U, // Default to M3U
    };

    // Get playlist info
    let playlist = state
        .database
        .get_playlist(playlist_id)
        .await
        .map_err(|e| {
            error!("Failed to get playlist {}: {}", playlist_id, e);
            AppError::Internal(e)
        })?
        .ok_or_else(|| AppError::NotFound)?;

    // Generate output filename
    let filename = PlaylistFileManager::get_output_filename(&playlist.name, format);
    let temp_path = std::env::temp_dir().join(&filename);

    // Export the playlist
    state
        .database
        .export_playlist_file(playlist_id, &temp_path, format)
        .await
        .map_err(|e| {
            error!("Failed to export playlist {}: {}", playlist_id, e);
            AppError::Internal(e)
        })?;

    // Read the file content
    let file_content = fs::read(&temp_path).await.map_err(|e| {
        error!("Failed to read exported file: {}", e);
        AppError::Io(e)
    })?;

    // Clean up temporary file
    let _ = fs::remove_file(&temp_path).await;

    // Determine MIME type
    let mime_type = match format {
        PlaylistFormat::M3U => "audio/x-mpegurl",
        PlaylistFormat::PLS => "audio/x-scpls",
    };

    info!("Exported playlist '{}' as {}", playlist.name, filename);

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime_type),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        Body::from(file_content),
    )
        .into_response())
}

/// Delete a playlist
pub async fn delete_playlist(
    State(state): State<AppState>,
    AxumPath(playlist_id): AxumPath<i64>,
) -> Result<Json<PlaylistOperationResponse>, AppError> {
    let success = state
        .database
        .delete_playlist(playlist_id)
        .await
        .map_err(|e| {
            error!("Failed to delete playlist {}: {}", playlist_id, e);
            AppError::Internal(e)
        })?;

    if success {
        info!("Deleted playlist with ID: {}", playlist_id);
        Ok(Json(PlaylistOperationResponse {
            success: true,
            message: format!("Playlist {} deleted successfully", playlist_id),
            playlist_id: Some(playlist_id),
        }))
    } else {
        warn!("Playlist not found for deletion: {}", playlist_id);
        Err(AppError::NotFound)
    }
}

/// Scan a directory for playlist files and import them
pub async fn scan_and_import_playlists(
    State(state): State<AppState>,
    Json(request): Json<ScanPlaylistsRequest>,
) -> Result<Json<ScanPlaylistsResponse>, AppError> {
    let directory = PathBuf::from(&request.directory);

    if !directory.exists() || !directory.is_dir() {
        return Err(AppError::InvalidInput(format!(
            "Directory does not exist or is not a directory: {}",
            request.directory
        )));
    }

    let imported_ids = state
        .database
        .scan_and_import_playlists(&directory)
        .await
        .map_err(|e| {
            error!("Failed to scan and import playlists from {}: {}", request.directory, e);
            AppError::Internal(e)
        })?;

    info!(
        "Imported {} playlists from directory: {}",
        imported_ids.len(),
        request.directory
    );

    Ok(Json(ScanPlaylistsResponse {
        imported_count: imported_ids.len(),
        playlist_ids: imported_ids.clone(),
        message: format!(
            "Successfully imported {} playlists from {}",
            imported_ids.len(),
            request.directory
        ),
    }))
}
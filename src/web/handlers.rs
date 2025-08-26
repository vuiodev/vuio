use crate::{
    database::MediaDirectory,
    error::AppError,
    state::AppState,
    web::xml::{generate_browse_response, generate_description_xml, generate_scpd_xml},
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use tokio::io::AsyncSeekExt;
use tokio_util::io::ReaderStream;
use tracing::{debug, error, info, warn};

pub async fn root_handler() -> &'static str {
    "VuIO Media Server"
}

pub async fn description_handler(State(state): State<AppState>) -> impl IntoResponse {
    let xml = generate_description_xml(&state);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        xml,
    )
}

pub async fn content_directory_scpd() -> impl IntoResponse {
    let xml = generate_scpd_xml();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        xml,
    )
}

/// Extracts browse parameters from SOAP request
#[derive(Debug, Clone)]
struct BrowseParams {
    object_id: String,
    starting_index: u32,
    requested_count: u32,
}

fn parse_browse_params(body: &str) -> BrowseParams {
    let object_id = if let Some(start) = body.find("<ObjectID>") {
        if let Some(end) = body.find("</ObjectID>") {
            body[start + 10..end].to_string()
        } else {
            "0".to_string()
        }
    } else {
        "0".to_string()
    };
    
    let starting_index = if let Some(start) = body.find("<StartingIndex>") {
        if let Some(end) = body.find("</StartingIndex>") {
            body[start + 15..end].parse().unwrap_or(0)
        } else {
            0
        }
    } else {
        0
    };
    
    let requested_count = if let Some(start) = body.find("<RequestedCount>") {
        if let Some(end) = body.find("</RequestedCount>") {
            body[start + 16..end].parse().unwrap_or(0)
        } else {
            0
        }
    } else {
        0
    };
    
    BrowseParams {
        object_id,
        starting_index,
        requested_count,
    }
}

pub async fn content_directory_control(
    State(state): State<AppState>,
    body: String,
) -> Response {
    if body.contains("<u:Browse") {
        let params = parse_browse_params(&body);
        info!("Browse request - ObjectID: {}, StartingIndex: {}, RequestedCount: {}", 
              params.object_id, params.starting_index, params.requested_count);

        // Handle root browse request (ObjectID "0")
        if params.object_id == "0" {
            // For the root, we typically return the top-level containers (Video, Audio, Image).
            // The generate_browse_response function should be smart enough to create these
            // when given an object_id of "0" and empty lists of subdirectories and files.
            let response = generate_browse_response("0", &[], &[], &state);
            return (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                    (header::HeaderName::from_static("ext"), ""),
                ],
                response,
            )
                .into_response();
        }

        // Determine media type filter and path prefix from ObjectID
        let (media_type_filter, path_prefix_str) = if params.object_id.starts_with("video") {
            ("video/", params.object_id.strip_prefix("video").unwrap_or("").trim_start_matches('/'))
        } else if params.object_id.starts_with("audio") {
            // Handle music categorization within audio section
            let audio_path = params.object_id.strip_prefix("audio").unwrap_or("").trim_start_matches('/');
            
            // Check for music categorization paths
            if audio_path.is_empty() {
                // Root audio container - return categorization containers
                return handle_audio_root_browse(&params, &state).await;
            } else if audio_path.starts_with("artists") {
                return handle_artists_browse(&params, &state, audio_path).await;
            } else if audio_path.starts_with("albums") {
                return handle_albums_browse(&params, &state, audio_path).await;
            } else if audio_path.starts_with("genres") {
                return handle_genres_browse(&params, &state, audio_path).await;
            } else if audio_path.starts_with("years") {
                return handle_years_browse(&params, &state, audio_path).await;
            } else if audio_path.starts_with("playlists") {
                return handle_playlists_browse(&params, &state, audio_path).await;
            } else {
                // Traditional folder browsing within audio
                ("audio/", audio_path)
            }
        } else if params.object_id.starts_with("image") {
            ("image/", params.object_id.strip_prefix("image").unwrap_or("").trim_start_matches('/'))
        } else {
            // This case might happen for deeper browsing or custom object IDs.
            // Assume no specific type filter for the database query, and the object_id itself
            // represents the path relative to the media root.
            ("", params.object_id.as_str())
        };

        // Determine the base path for the media type.
        // For now, we assume all media is under one primary root.
        let media_root = state.config.get_primary_media_dir();
        let browse_path = if path_prefix_str.is_empty() {
            media_root.clone()
        } else {
            media_root.join(path_prefix_str)
        };
        
        // Query the database for the directory listing with timeout
        let query_future = state.database.get_directory_listing(&browse_path, media_type_filter);
        let timeout_duration = std::time::Duration::from_secs(30); // 30 second timeout
        
        match tokio::time::timeout(timeout_duration, query_future).await {
            Ok(Ok((subdirectories, files))) => {
                debug!("Database query completed: {} subdirs, {} files for path: {}", 
                       subdirectories.len(), files.len(), browse_path.display());
                       
                // Apply pagination if requested
                let starting_index = params.starting_index as usize;
                let requested_count = if params.requested_count == 0 { 
                    // If RequestedCount is 0, return all items (but limit to prevent hanging)
                    2000 
                } else { 
                    std::cmp::min(params.requested_count as usize, 2000) 
                };
                
                // Combine directories and files for pagination
                let mut all_items = Vec::new();
                for subdir in &subdirectories {
                    all_items.push((subdir.clone(), None));
                }
                for file in &files {
                    all_items.push((MediaDirectory { path: file.path.clone(), name: String::new() }, Some(file.clone())));
                }
                
                let total_matches = all_items.len();
                let end_index = std::cmp::min(starting_index + requested_count, total_matches);
                
                if starting_index >= total_matches {
                    // Starting index is beyond available items
                    let response = generate_browse_response(&params.object_id, &[], &[], &state);
                    return (
                        StatusCode::OK,
                        [
                            (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                            (header::HeaderName::from_static("ext"), ""),
                        ],
                        response,
                    )
                        .into_response();
                }
                
                // Extract paginated items
                let paginated_items = &all_items[starting_index..end_index];
                let mut paginated_subdirs = Vec::new();
                let mut paginated_files = Vec::new();
                
                for (item, file_opt) in paginated_items {
                    if let Some(file) = file_opt {
                        paginated_files.push(file.clone());
                    } else {
                        paginated_subdirs.push(item.clone());
                    }
                }
                
                debug!("Returning paginated results: {} subdirs, {} files (index {}-{} of {})",
                       paginated_subdirs.len(), paginated_files.len(), 
                       starting_index, end_index, total_matches);
                
                let response = generate_browse_response(&params.object_id, &paginated_subdirs, &paginated_files, &state);
                (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                        (header::HeaderName::from_static("ext"), ""),
                    ],
                    response,
                )
                    .into_response()
            },
            Ok(Err(e)) => {
                error!("Database error getting directory listing for {}: {}", params.object_id, e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing content".to_string(),
                )
                    .into_response()
            },
            Err(_timeout) => {
                error!("Database query timeout for object_id: {} (path: {})", params.object_id, browse_path.display());
                (
                    StatusCode::REQUEST_TIMEOUT,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Request timeout - directory too large".to_string(),
                )
                    .into_response()
            }
        }
    } else {
        (
            StatusCode::NOT_IMPLEMENTED,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Not implemented".to_string(),
        )
            .into_response()
    }
}

pub async fn serve_media(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let file_id = id.parse::<i64>().map_err(|_| AppError::NotFound)?;
    let file_info = state.database
        .get_file_by_id(file_id)
        .await
        .map_err(|_| AppError::NotFound)?
        .ok_or(AppError::NotFound)?;

    // Enforce read-only access to media files
    let mut file = tokio::fs::OpenOptions::new()
        .read(true)
        .write(false)
        .open(&file_info.path)
        .await
        .map_err(AppError::Io)?;
    let file_size = file_info.size;

    let mut response_builder = Response::builder()
        .header(header::CONTENT_TYPE, file_info.mime_type)
        .header(header::ACCEPT_RANGES, "bytes");

    let (start, end) = if let Some(range_header) = headers.get(header::RANGE) {
        let range_str = range_header.to_str().map_err(|_| AppError::InvalidRange)?;
        debug!("Received range request: {}", range_str);
        
        // Parse the range header manually to avoid enum variant issues
        parse_range_header(range_str, file_size)?
    } else {
        // No range requested, serve the whole file
        (0, file_size - 1)
    };

    let len = end - start + 1;

    let response_status = if len < file_size {
        response_builder = response_builder.header(
            header::CONTENT_RANGE,
            format!("bytes {}-{}/{}", start, end, file_size),
        );
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };

    response_builder = response_builder.header(header::CONTENT_LENGTH, len);

    file.seek(std::io::SeekFrom::Start(start)).await?;
    let stream = ReaderStream::with_capacity(file, 64 * 1024).take(len as usize);
    let body = Body::from_stream(stream);

    Ok(response_builder.status(response_status).body(body)?)
}

// Helper function to parse range header manually
fn parse_range_header(range_str: &str, file_size: u64) -> Result<(u64, u64), AppError> {
    // Remove "bytes=" prefix
    let range_part = range_str.strip_prefix("bytes=").ok_or(AppError::InvalidRange)?;
    
    // Split on comma to get individual ranges (we'll just handle the first one)
    let first_range = range_part.split(',').next().ok_or(AppError::InvalidRange)?;
    
    // Parse the range
    if let Some((start_str, end_str)) = first_range.split_once('-') {
        let start = if start_str.is_empty() {
            // Suffix range like "-500" (last 500 bytes)
            let suffix_len: u64 = end_str.parse().map_err(|_| AppError::InvalidRange)?;
            file_size.saturating_sub(suffix_len)
        } else {
            start_str.parse().map_err(|_| AppError::InvalidRange)?
        };
        
        let end = if end_str.is_empty() {
            // Range like "500-" (from 500 to end)
            file_size - 1
        } else {
            let parsed_end: u64 = end_str.parse().map_err(|_| AppError::InvalidRange)?;
            parsed_end.min(file_size - 1)
        };
        
        // Validate range
        if start > end || start >= file_size {
            return Err(AppError::InvalidRange);
        }
        
        Ok((start, end))
    } else {
        Err(AppError::InvalidRange)
    }
}

/// Handle UPnP eventing subscription requests for ContentDirectory service
pub async fn content_directory_subscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
    method: Method,
) -> impl IntoResponse {
    // Handle SUBSCRIBE method (which might come as GET or a custom method)
    if method == Method::GET || headers.get("CALLBACK").is_some() {
        // Handle subscription request
        if let Some(callback) = headers.get("CALLBACK") {
            let callback_url = callback.to_str().unwrap_or("");
            info!("UPnP subscription request from: {}", callback_url);
            
            // Generate a subscription ID (in a real implementation, this should be stored)
            let subscription_id = format!("uuid:{}", uuid::Uuid::new_v4());
            let timeout = "Second-1800"; // 30 minutes
            
            // Get current update ID
            let update_id = state.content_update_id.load(std::sync::atomic::Ordering::Relaxed);
            
            // Send initial event notification
            tokio::spawn(send_initial_event_notification(callback_url.to_string(), update_id));
            
            (
                StatusCode::OK,
                [
                    (header::HeaderName::from_static("sid"), subscription_id.as_str()),
                    (header::HeaderName::from_static("timeout"), timeout),
                    (header::CONTENT_LENGTH, "0"),
                ],
                "",
            ).into_response()
        } else {
            warn!("UPnP subscription request missing CALLBACK header");
            (
                StatusCode::BAD_REQUEST,
                [
                    (header::CONTENT_TYPE, "text/plain"),
                    (header::CONTENT_LENGTH, "0"),
                ],
                "",
            ).into_response()
        }
    } else if headers.get("SID").is_some() {
        // Handle unsubscription request (UNSUBSCRIBE method)
        let subscription_id = headers.get("SID").unwrap().to_str().unwrap_or("");
        info!("UPnP unsubscription request for: {}", subscription_id);
        
        (
            StatusCode::OK,
            [(header::CONTENT_LENGTH, "0")],
            "",
        ).into_response()
    } else {
        (
            StatusCode::METHOD_NOT_ALLOWED,
            [
                (header::CONTENT_TYPE, "text/plain"),
                (header::CONTENT_LENGTH, "0"),
            ],
            "",
        ).into_response()
    }
}

/// Send initial event notification to a subscribed client
async fn send_initial_event_notification(callback_url: String, update_id: u32) {
    let event_body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<e:propertyset xmlns:e="urn:schemas-upnp-org:event-1-0">
    <e:property>
        <SystemUpdateID>{}</SystemUpdateID>
    </e:property>
    <e:property>
        <ContainerUpdateIDs></ContainerUpdateIDs>
    </e:property>
</e:propertyset>"#,
        update_id
    );
    
    // Extract the actual URL from the callback (remove angle brackets if present)
    let url = callback_url.trim_start_matches('<').trim_end_matches('>');
    
    let client = reqwest::Client::new();
    match client
        .request(reqwest::Method::from_bytes(b"NOTIFY").unwrap(), url)
        .header("HOST", "")
        .header("CONTENT-TYPE", "text/xml; charset=\"utf-8\"")
        .header("NT", "upnp:event")
        .header("NTS", "upnp:propchange")
        .header("SID", "uuid:dummy") // In real implementation, use actual subscription ID
        .header("SEQ", "0")
        .body(event_body)
        .send()
        .await
    {
        Ok(response) => {
            debug!("Event notification sent successfully, status: {}", response.status());
        }
        Err(e) => {
            warn!("Failed to send event notification to {}: {}", url, e);
        }
    }
}

// Music categorization handlers

/// Handle browsing the root audio container with music categorization
async fn handle_audio_root_browse(
    params: &BrowseParams,
    state: &AppState,
) -> Response {
    use crate::web::xml::generate_browse_response;
    
    // Create virtual categorization containers
    let virtual_containers = vec![
        ("audio/artists", "Artists"),
        ("audio/albums", "Albums"), 
        ("audio/genres", "Genres"),
        ("audio/years", "Years"),
        ("audio/playlists", "Playlists"),
        ("audio/folders", "Folders"),
    ];
    
    // Convert to MediaDirectory for XML generation
    let subdirectories: Vec<crate::database::MediaDirectory> = virtual_containers
        .into_iter()
        .map(|(id, name)| crate::database::MediaDirectory {
            path: std::path::PathBuf::from(id),
            name: name.to_string(),
        })
        .collect();
    
    let response = generate_browse_response(&params.object_id, &subdirectories, &[], state);
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
            (header::HeaderName::from_static("ext"), ""),
        ],
        response,
    )
        .into_response()
}
    
/// Handle browsing artists
async fn handle_artists_browse(
    params: &BrowseParams,
    state: &AppState, 
    audio_path: &str,
) -> Response {
    use crate::web::xml::generate_browse_response;
    
    let path_parts: Vec<&str> = audio_path.split('/').filter(|s| !s.is_empty()).collect();
    
    if path_parts.len() == 1 {
        // List all artists
        match state.database.get_artists().await {
            Ok(artists) => {
                let subdirectories: Vec<crate::database::MediaDirectory> = artists
                    .into_iter()
                    .map(|artist| crate::database::MediaDirectory {
                        path: std::path::PathBuf::from(format!("audio/artists/{}", artist.name)),
                        name: format!("{} ({})", artist.name, artist.count),
                    })
                    .collect();
                    
                let response = generate_browse_response(&params.object_id, &subdirectories, &[], state);
                (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                        (header::HeaderName::from_static("ext"), ""),
                    ],
                    response,
                )
                    .into_response()
            }
            Err(e) => {
                error!("Error getting artists: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing artists".to_string(),
                )
                    .into_response()
            }
        }
    } else if path_parts.len() == 2 {
        // List tracks by specific artist
        let artist_name = path_parts[1];
        match state.database.get_music_by_artist(artist_name).await {
            Ok(files) => {
                let response = generate_browse_response(&params.object_id, &[], &files, state);
                (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                        (header::HeaderName::from_static("ext"), ""),
                    ],
                    response,
                )
                    .into_response()
            }
            Err(e) => {
                error!("Error getting music by artist {}: {}", artist_name, e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing artist tracks".to_string(),
                )
                    .into_response()
            }
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Invalid artist path".to_string(),
        )
            .into_response()
    }
}

/// Handle browsing albums
async fn handle_albums_browse(
    params: &BrowseParams,
    state: &AppState,
    audio_path: &str,
) -> Response {
    use crate::web::xml::generate_browse_response;
    
    let path_parts: Vec<&str> = audio_path.split('/').filter(|s| !s.is_empty()).collect();
    
    if path_parts.len() == 1 {
        // List all albums
        match state.database.get_albums(None).await {
            Ok(albums) => {
                let subdirectories: Vec<crate::database::MediaDirectory> = albums
                    .into_iter()
                    .map(|album| crate::database::MediaDirectory {
                        path: std::path::PathBuf::from(format!("audio/albums/{}", album.name)),
                        name: format!("{} ({})", album.name, album.count),
                    })
                    .collect();
                    
                let response = generate_browse_response(&params.object_id, &subdirectories, &[], state);
                (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                        (header::HeaderName::from_static("ext"), ""),
                    ],
                    response,
                )
                    .into_response()
            }
            Err(e) => {
                error!("Error getting albums: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing albums".to_string(),
                )
                    .into_response()
            }
        }
    } else if path_parts.len() == 2 {
        // List tracks by specific album
        let album_name = path_parts[1];
        match state.database.get_music_by_album(album_name, None).await {
            Ok(files) => {
                let response = generate_browse_response(&params.object_id, &[], &files, state);
                (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                        (header::HeaderName::from_static("ext"), ""),
                    ],
                    response,
                )
                    .into_response()
            }
            Err(e) => {
                error!("Error getting music by album {}: {}", album_name, e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing album tracks".to_string(),
                )
                    .into_response()
            }
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Invalid album path".to_string(),
        )
            .into_response()
    }
}

/// Handle browsing genres
async fn handle_genres_browse(
    params: &BrowseParams,
    state: &AppState,
    audio_path: &str,
) -> Response {
    use crate::web::xml::generate_browse_response;
    
    let path_parts: Vec<&str> = audio_path.split('/').filter(|s| !s.is_empty()).collect();
    
    if path_parts.len() == 1 {
        // List all genres
        match state.database.get_genres().await {
            Ok(genres) => {
                let subdirectories: Vec<crate::database::MediaDirectory> = genres
                    .into_iter()
                    .map(|genre| crate::database::MediaDirectory {
                        path: std::path::PathBuf::from(format!("audio/genres/{}", genre.name)),
                        name: format!("{} ({})", genre.name, genre.count),
                    })
                    .collect();
                    
                let response = generate_browse_response(&params.object_id, &subdirectories, &[], state);
                (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                        (header::HeaderName::from_static("ext"), ""),
                    ],
                    response,
                )
                    .into_response()
            }
            Err(e) => {
                error!("Error getting genres: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing genres".to_string(),
                )
                    .into_response()
            }
        }
    } else if path_parts.len() == 2 {
        // List tracks by specific genre
        let genre_name = path_parts[1];
        match state.database.get_music_by_genre(genre_name).await {
            Ok(files) => {
                let response = generate_browse_response(&params.object_id, &[], &files, state);
                (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                        (header::HeaderName::from_static("ext"), ""),
                    ],
                    response,
                )
                    .into_response()
            }
            Err(e) => {
                error!("Error getting music by genre {}: {}", genre_name, e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing genre tracks".to_string(),
                )
                    .into_response()
            }
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Invalid genre path".to_string(),
        )
            .into_response()
    }
}

/// Handle browsing years
async fn handle_years_browse(
    params: &BrowseParams,
    state: &AppState,
    audio_path: &str,
) -> Response {
    use crate::web::xml::generate_browse_response;
    
    let path_parts: Vec<&str> = audio_path.split('/').filter(|s| !s.is_empty()).collect();
    
    if path_parts.len() == 1 {
        // List all years
        match state.database.get_years().await {
            Ok(years) => {
                let subdirectories: Vec<crate::database::MediaDirectory> = years
                    .into_iter()
                    .map(|year| crate::database::MediaDirectory {
                        path: std::path::PathBuf::from(format!("audio/years/{}", year.name)),
                        name: format!("{} ({})", year.name, year.count),
                    })
                    .collect();
                    
                let response = generate_browse_response(&params.object_id, &subdirectories, &[], state);
                (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                        (header::HeaderName::from_static("ext"), ""),
                    ],
                    response,
                )
                    .into_response()
            }
            Err(e) => {
                error!("Error getting years: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing years".to_string(),
                )
                    .into_response()
            }
        }
    } else if path_parts.len() == 2 {
        // List tracks by specific year
        let year_str = path_parts[1];
        if let Ok(year) = year_str.parse::<u32>() {
            match state.database.get_music_by_year(year).await {
                Ok(files) => {
                    let response = generate_browse_response(&params.object_id, &[], &files, state);
                    (
                        StatusCode::OK,
                        [
                            (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                            (header::HeaderName::from_static("ext"), ""),
                        ],
                        response,
                    )
                        .into_response()
                }
                Err(e) => {
                    error!("Error getting music by year {}: {}", year, e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Error browsing year tracks".to_string(),
                    )
                        .into_response()
                }
            }
        } else {
            (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "Invalid year format".to_string(),
            )
                .into_response()
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Invalid year path".to_string(),
        )
            .into_response()
    }
}

/// Handle browsing playlists
async fn handle_playlists_browse(
    params: &BrowseParams,
    state: &AppState,
    audio_path: &str,
) -> Response {
    use crate::web::xml::generate_browse_response;
    
    let path_parts: Vec<&str> = audio_path.split('/').filter(|s| !s.is_empty()).collect();
    
    if path_parts.len() == 1 {
        // List all playlists
        match state.database.get_playlists().await {
            Ok(playlists) => {
                let subdirectories: Vec<crate::database::MediaDirectory> = playlists
                    .into_iter()
                    .map(|playlist| crate::database::MediaDirectory {
                        path: std::path::PathBuf::from(format!("audio/playlists/{}", playlist.id.unwrap_or(0))),
                        name: playlist.name,
                    })
                    .collect();
                    
                let response = generate_browse_response(&params.object_id, &subdirectories, &[], state);
                (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                        (header::HeaderName::from_static("ext"), ""),
                    ],
                    response,
                )
                    .into_response()
            }
            Err(e) => {
                error!("Error getting playlists: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing playlists".to_string(),
                )
                    .into_response()
            }
        }
    } else if path_parts.len() == 2 {
        // List tracks in specific playlist
        let playlist_id_str = path_parts[1];
        if let Ok(playlist_id) = playlist_id_str.parse::<i64>() {
            match state.database.get_playlist_tracks(playlist_id).await {
                Ok(files) => {
                    let response = generate_browse_response(&params.object_id, &[], &files, state);
                    (
                        StatusCode::OK,
                        [
                            (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                            (header::HeaderName::from_static("ext"), ""),
                        ],
                        response,
                    )
                        .into_response()
                }
                Err(e) => {
                    error!("Error getting playlist tracks for {}: {}", playlist_id, e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Error browsing playlist tracks".to_string(),
                    )
                        .into_response()
                }
            }
        } else {
            (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "Invalid playlist ID format".to_string(),
            )
                .into_response()
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Invalid playlist path".to_string(),
        )
            .into_response()
    }
}
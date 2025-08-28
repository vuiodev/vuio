use crate::{
    database::MediaDirectory,
    error::AppError,
    state::AppState,
    web::xml::{generate_description_xml, generate_scpd_xml},
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::io::AsyncSeekExt;
use tokio_util::io::ReaderStream;
use tracing::{debug, error, info, warn};

/// Atomic performance tracking for web handlers
pub struct WebHandlerMetrics {
    pub browse_requests: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub directory_listings: AtomicU64,
    pub file_serves: AtomicU64,
    pub errors: AtomicU64,
    pub total_response_time_ms: AtomicU64,
}

impl WebHandlerMetrics {
    pub fn new() -> Self {
        Self {
            browse_requests: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            directory_listings: AtomicU64::new(0),
            file_serves: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            total_response_time_ms: AtomicU64::new(0),
        }
    }
    
    pub fn record_browse_request(&self, response_time_ms: u64, cache_hit: bool) {
        self.browse_requests.fetch_add(1, Ordering::Relaxed);
        self.total_response_time_ms.fetch_add(response_time_ms, Ordering::Relaxed);
        if cache_hit {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.cache_misses.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    pub fn record_directory_listing(&self, response_time_ms: u64) {
        self.directory_listings.fetch_add(1, Ordering::Relaxed);
        self.total_response_time_ms.fetch_add(response_time_ms, Ordering::Relaxed);
    }
    
    pub fn record_file_serve(&self, response_time_ms: u64) {
        self.file_serves.fetch_add(1, Ordering::Relaxed);
        self.total_response_time_ms.fetch_add(response_time_ms, Ordering::Relaxed);
    }
    
    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn get_stats(&self) -> WebHandlerStats {
        let browse_requests = self.browse_requests.load(Ordering::Relaxed);
        let total_time = self.total_response_time_ms.load(Ordering::Relaxed);
        
        WebHandlerStats {
            browse_requests,
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            directory_listings: self.directory_listings.load(Ordering::Relaxed),
            file_serves: self.file_serves.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            average_response_time_ms: if browse_requests > 0 { total_time / browse_requests } else { 0 },
            cache_hit_rate: if browse_requests > 0 { 
                (self.cache_hits.load(Ordering::Relaxed) as f64 / browse_requests as f64) * 100.0 
            } else { 0.0 },
        }
    }
}

#[derive(Debug, Clone)]
pub struct WebHandlerStats {
    pub browse_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub directory_listings: u64,
    pub file_serves: u64,
    pub errors: u64,
    pub average_response_time_ms: u64,
    pub cache_hit_rate: f64,
}

// Web metrics will be stored in AppState for atomic access

pub async fn root_handler() -> &'static str {
    "VuIO Media Server"
}

pub async fn description_handler(State(state): State<AppState>) -> impl IntoResponse {
    let xml = generate_description_xml(&state).await;
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
    use quick_xml::events::Event;
    use quick_xml::Reader;
    
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);
    
    let mut object_id = "0".to_string();
    let mut starting_index = 0u32;
    let mut requested_count = 0u32;
    
    let mut buf = Vec::new();
    let mut current_element = String::new();
    
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                current_element = String::from_utf8_lossy(e.name().as_ref()).to_string();
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default();
                match current_element.as_str() {
                    "ObjectID" => {
                        object_id = text.trim().to_string();
                        if object_id.is_empty() {
                            object_id = "0".to_string();
                        }
                    }
                    "StartingIndex" => {
                        starting_index = text.trim().parse().unwrap_or_else(|e| {
                            warn!("Failed to parse StartingIndex '{}': {}", text, e);
                            0
                        });
                    }
                    "RequestedCount" => {
                        requested_count = text.trim().parse().unwrap_or_else(|e| {
                            warn!("Failed to parse RequestedCount '{}': {}", text, e);
                            0
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("Error parsing XML: {}, falling back to defaults", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }
    
    debug!("Parsed browse params - ObjectID: '{}', StartingIndex: {}, RequestedCount: {}", 
           object_id, starting_index, requested_count);
    
    BrowseParams {
        object_id,
        starting_index,
        requested_count,
    }
}

/// Content Directory Handler struct to encapsulate specialized browse handlers
pub struct ContentDirectoryHandler;

impl ContentDirectoryHandler {
    /// Handle video browse requests
    async fn handle_video_browse(
        params: &BrowseParams,
        state: &AppState,
        path_prefix_str: &str,
    ) -> Response {
        Self::handle_folder_browse(params, state, "video/", path_prefix_str).await
    }

    /// Handle music browse requests (folder-based, not categorized)
    async fn handle_music_browse(
        params: &BrowseParams,
        state: &AppState,
        path_prefix_str: &str,
    ) -> Response {
        Self::handle_folder_browse(params, state, "audio/", path_prefix_str).await
    }

    /// Handle image browse requests
    async fn handle_image_browse(
        params: &BrowseParams,
        state: &AppState,
        path_prefix_str: &str,
    ) -> Response {
        Self::handle_folder_browse(params, state, "image/", path_prefix_str).await
    }

    /// Handle generic folder-based browse requests with consistent path normalization
    /// Enhanced with atomic performance tracking and cache-friendly operations
    async fn handle_folder_browse(
        params: &BrowseParams,
        state: &AppState,
        media_type_filter: &str,
        path_prefix_str: &str,
    ) -> Response {
        use crate::web::xml::generate_browse_response;

        let start_time = Instant::now();
        let mut cache_hit = false;

        // Determine the base path for the media type
        let media_root = state.config.get_primary_media_dir();
        let browse_path = if path_prefix_str.is_empty() {
            media_root.clone()
        } else {
            media_root.join(path_prefix_str)
        };
        
        // Apply canonical path normalization to match how paths are stored in the database
        // Use the same normalization logic used during file scanning
        let canonical_browse_path = match state.filesystem_manager.get_canonical_path(&browse_path) {
            Ok(canonical) => std::path::PathBuf::from(canonical),
            Err(e) => {
                warn!("Failed to get canonical path for browse request '{}': {}, using basic normalization", browse_path.display(), e);
                state.web_metrics.record_error();
                state.filesystem_manager.normalize_path(&browse_path)
            }
        };
        
        // Query the ZeroCopy database for the directory listing with timeout and atomic operations
        let query_future = state.database.get_directory_listing(&canonical_browse_path, media_type_filter);
        let timeout_duration = std::time::Duration::from_secs(30); // 30 second timeout
        
        match tokio::time::timeout(timeout_duration, query_future).await {
            Ok(Ok((subdirectories, files))) => {
                cache_hit = !subdirectories.is_empty() || !files.is_empty(); // Assume cache hit if data found
                
                debug!("ZeroCopy browse request for '{}' -> canonical '{}' (filter: '{}') returned {} subdirs, {} files", 
                       browse_path.display(), canonical_browse_path.display(), media_type_filter, subdirectories.len(), files.len());
                       
                // Apply pagination if requested
                let starting_index = params.starting_index as usize;
                let requested_count = if params.requested_count == 0 { 
                    // If RequestedCount is 0, return all items (but limit to prevent hanging)
                    2000 
                } else { 
                    std::cmp::min(params.requested_count as usize, 2000) 
                };
                
                // Combine directories and files for pagination with atomic counting
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
                    // Starting index is beyond available items - record metrics and return empty
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_browse_request(response_time, cache_hit);
                    
                    let server_ip = state.get_server_ip();
                    let response = generate_browse_response(&params.object_id, &[], &[], state, &server_ip).await;
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
                
                // Extract paginated items with zero-copy operations
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
                
                debug!("ZeroCopy returning paginated results: {} subdirs, {} files (index {}-{} of {})",
                       paginated_subdirs.len(), paginated_files.len(), 
                       starting_index, end_index, total_matches);
                
                // Record atomic performance metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_browse_request(response_time, cache_hit);
                state.web_metrics.record_directory_listing(response_time);
                
                let server_ip = state.get_server_ip();
                let response = generate_browse_response(&params.object_id, &paginated_subdirs, &paginated_files, state, &server_ip).await;
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
                error!("ZeroCopy database error getting directory listing for {}: {}", params.object_id, e);
                
                // Record atomic error metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_error();
                state.web_metrics.record_browse_request(response_time, false);
                
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing content".to_string(),
                )
                    .into_response()
            },
            Err(_timeout) => {
                error!("ZeroCopy database query timeout for object_id: {} (path: {} -> canonical: {})", params.object_id, browse_path.display(), canonical_browse_path.display());
                
                // Record atomic timeout metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_error();
                state.web_metrics.record_browse_request(response_time, false);
                
                (
                    StatusCode::REQUEST_TIMEOUT,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Request timeout - directory too large".to_string(),
                )
                    .into_response()
            }
        }
    }

    /// Handle root browse request (ObjectID "0")
    async fn handle_root_browse(_params: &BrowseParams, state: &AppState) -> Response {
        use crate::web::xml::generate_browse_response;
        
        // For the root, we typically return the top-level containers (Video, Audio, Image).
        // The generate_browse_response function should be smart enough to create these
        // when given an object_id of "0" and empty lists of subdirectories and files.
        let server_ip = state.get_server_ip();
        let response = generate_browse_response("0", &[], &[], state, &server_ip).await;
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
            return ContentDirectoryHandler::handle_root_browse(&params, &state).await;
        }

        // Determine media type and delegate to specialized handlers
        if params.object_id.starts_with("video") {
            let path_prefix_str = params.object_id.strip_prefix("video").unwrap_or("").trim_start_matches('/');
            return ContentDirectoryHandler::handle_video_browse(&params, &state, path_prefix_str).await;
        } else if params.object_id.starts_with("audio") {
            // Handle music categorization within audio section
            let audio_path = params.object_id.strip_prefix("audio").unwrap_or("").trim_start_matches('/');
            
            // Check for music categorization paths
            if audio_path.is_empty() {
                // Root audio container - return categorization containers
                return handle_audio_root_browse(&params, &state).await;
            } else if audio_path.starts_with("artists") {
                return ContentDirectoryHandler::handle_artist_browse(&params, &state, audio_path).await;
            } else if audio_path.starts_with("albums") {
                return ContentDirectoryHandler::handle_album_browse(&params, &state, audio_path).await;
            } else if audio_path.starts_with("genres") {
                return handle_genres_browse(&params, &state, audio_path).await;
            } else if audio_path.starts_with("years") {
                return handle_years_browse(&params, &state, audio_path).await;
            } else if audio_path.starts_with("playlists") {
                return handle_playlists_browse(&params, &state, audio_path).await;
            } else {
                // Traditional folder browsing within audio
                return ContentDirectoryHandler::handle_music_browse(&params, &state, audio_path).await;
            }
        } else if params.object_id.starts_with("image") {
            let path_prefix_str = params.object_id.strip_prefix("image").unwrap_or("").trim_start_matches('/');
            return ContentDirectoryHandler::handle_image_browse(&params, &state, path_prefix_str).await;
        } else {
            // This case might happen for deeper browsing or custom object IDs.
            // Assume no specific type filter for the database query, and the object_id itself
            // represents the path relative to the media root.
            return ContentDirectoryHandler::handle_folder_browse(&params, &state, "", params.object_id.as_str()).await;
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
    let start_time = Instant::now();
    
    let file_id = id.parse::<i64>().map_err(|_| {
        state.web_metrics.record_error();
        AppError::NotFound
    })?;
    
    // Use ZeroCopy database with atomic cache lookup
    let file_info = state.database
        .get_file_by_id(file_id)
        .await
        .map_err(|e| {
            error!("ZeroCopy database error getting file by ID {}: {}", file_id, e);
            state.web_metrics.record_error();
            AppError::NotFound
        })?
        .ok_or_else(|| {
            debug!("ZeroCopy database: file ID {} not found", file_id);
            state.web_metrics.record_error();
            AppError::NotFound
        })?;

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

    // Record atomic performance metrics for file serving
    let response_time = start_time.elapsed().as_millis() as u64;
    state.web_metrics.record_file_serve(response_time);
    
    debug!("ZeroCopy served media file ID {} in {}ms", file_id, response_time);

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
    
    let server_ip = state.get_server_ip();
    let response = generate_browse_response(&params.object_id, &subdirectories, &[], state, &server_ip).await;
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
    
impl ContentDirectoryHandler {
    /// Handle artist browse requests with atomic performance tracking and ZeroCopy operations
    async fn handle_artist_browse(
        params: &BrowseParams,
        state: &AppState,
        audio_path: &str,
    ) -> Response {
        use crate::web::xml::generate_browse_response;
        
        let start_time = Instant::now();
        let path_parts: Vec<&str> = audio_path.split('/').filter(|s| !s.is_empty()).collect();
        
        if path_parts.len() == 1 {
            // List all artists using ZeroCopy atomic operations
            match state.database.get_artists().await {
                Ok(artists) => {
                    let has_data = !artists.is_empty();
                    let subdirectories: Vec<crate::database::MediaDirectory> = artists
                        .into_iter()
                        .map(|artist| crate::database::MediaDirectory {
                            path: std::path::PathBuf::from(format!("audio/artists/{}", artist.name)),
                            name: format!("{} ({})", artist.name, artist.count),
                        })
                        .collect();
                    
                    // Record atomic performance metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_browse_request(response_time, has_data);
                    
                    debug!("ZeroCopy retrieved {} artists in {}ms", subdirectories.len(), response_time);
                        
                    let server_ip = state.get_server_ip();
                    let response = generate_browse_response(&params.object_id, &subdirectories, &[], state, &server_ip).await;
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
                    error!("ZeroCopy error getting artists: {}", e);
                    
                    // Record atomic error metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_error();
                    state.web_metrics.record_browse_request(response_time, false);
                    
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Error browsing artists".to_string(),
                    )
                        .into_response()
                }
            }
        } else if path_parts.len() == 2 {
            // List tracks by specific artist using ZeroCopy atomic operations
            let artist_name = path_parts[1];
            match state.database.get_music_by_artist(artist_name).await {
                Ok(files) => {
                    // Record atomic performance metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_browse_request(response_time, !files.is_empty());
                    
                    debug!("ZeroCopy retrieved {} tracks for artist '{}' in {}ms", files.len(), artist_name, response_time);
                    
                    let server_ip = state.get_server_ip();
                    let response = generate_browse_response(&params.object_id, &[], &files, state, &server_ip).await;
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
                    error!("ZeroCopy error getting music by artist {}: {}", artist_name, e);
                    
                    // Record atomic error metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_error();
                    state.web_metrics.record_browse_request(response_time, false);
                    
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Error browsing artist tracks".to_string(),
                    )
                        .into_response()
                }
            }
        } else {
            // Record atomic error metrics for invalid path
            let response_time = start_time.elapsed().as_millis() as u64;
            state.web_metrics.record_error();
            state.web_metrics.record_browse_request(response_time, false);
            
            (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "Invalid artist path".to_string(),
            )
                .into_response()
        }
    }

    /// Handle album browse requests with atomic performance tracking and ZeroCopy operations
    async fn handle_album_browse(
        params: &BrowseParams,
        state: &AppState,
        audio_path: &str,
    ) -> Response {
        use crate::web::xml::generate_browse_response;
        
        let start_time = Instant::now();
        let path_parts: Vec<&str> = audio_path.split('/').filter(|s| !s.is_empty()).collect();
        
        if path_parts.len() == 1 {
            // List all albums using ZeroCopy atomic operations
            match state.database.get_albums(None).await {
                Ok(albums) => {
                    let has_data = !albums.is_empty();
                    let subdirectories: Vec<crate::database::MediaDirectory> = albums
                        .into_iter()
                        .map(|album| crate::database::MediaDirectory {
                            path: std::path::PathBuf::from(format!("audio/albums/{}", album.name)),
                            name: format!("{} ({})", album.name, album.count),
                        })
                        .collect();
                    
                    // Record atomic performance metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_browse_request(response_time, has_data);
                    
                    debug!("ZeroCopy retrieved {} albums in {}ms", subdirectories.len(), response_time);
                        
                    let server_ip = state.get_server_ip();
                    let response = generate_browse_response(&params.object_id, &subdirectories, &[], state, &server_ip).await;
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
                    error!("ZeroCopy error getting albums: {}", e);
                    
                    // Record atomic error metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_error();
                    state.web_metrics.record_browse_request(response_time, false);
                    
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Error browsing albums".to_string(),
                    )
                        .into_response()
                }
            }
        } else if path_parts.len() == 2 {
            // List tracks by specific album using ZeroCopy atomic operations
            let album_name = path_parts[1];
            match state.database.get_music_by_album(album_name, None).await {
                Ok(files) => {
                    // Record atomic performance metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_browse_request(response_time, !files.is_empty());
                    
                    debug!("ZeroCopy retrieved {} tracks for album '{}' in {}ms", files.len(), album_name, response_time);
                    
                    let server_ip = state.get_server_ip();
                    let response = generate_browse_response(&params.object_id, &[], &files, state, &server_ip).await;
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
                    error!("ZeroCopy error getting music by album {}: {}", album_name, e);
                    
                    // Record atomic error metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_error();
                    state.web_metrics.record_browse_request(response_time, false);
                    
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
}



/// Handle browsing genres with atomic performance tracking and ZeroCopy operations
async fn handle_genres_browse(
    params: &BrowseParams,
    state: &AppState,
    audio_path: &str,
) -> Response {
    use crate::web::xml::generate_browse_response;
    
    let start_time = Instant::now();
    let path_parts: Vec<&str> = audio_path.split('/').filter(|s| !s.is_empty()).collect();
    
    if path_parts.len() == 1 {
        // List all genres using ZeroCopy atomic operations
        match state.database.get_genres().await {
            Ok(genres) => {
                let has_data = !genres.is_empty();
                let subdirectories: Vec<crate::database::MediaDirectory> = genres
                    .into_iter()
                    .map(|genre| crate::database::MediaDirectory {
                        path: std::path::PathBuf::from(format!("audio/genres/{}", genre.name)),
                        name: format!("{} ({})", genre.name, genre.count),
                    })
                    .collect();
                
                // Record atomic performance metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_browse_request(response_time, has_data);
                
                debug!("ZeroCopy retrieved {} genres in {}ms", subdirectories.len(), response_time);
                    
                let server_ip = state.get_server_ip();
                let response = generate_browse_response(&params.object_id, &subdirectories, &[], state, &server_ip).await;
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
                error!("ZeroCopy error getting genres: {}", e);
                
                // Record atomic error metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_error();
                state.web_metrics.record_browse_request(response_time, false);
                
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing genres".to_string(),
                )
                    .into_response()
            }
        }
    } else if path_parts.len() == 2 {
        // List tracks by specific genre using ZeroCopy atomic operations
        let genre_name = path_parts[1];
        match state.database.get_music_by_genre(genre_name).await {
            Ok(files) => {
                // Record atomic performance metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_browse_request(response_time, !files.is_empty());
                
                debug!("ZeroCopy retrieved {} tracks for genre '{}' in {}ms", files.len(), genre_name, response_time);
                
                let server_ip = state.get_server_ip();
                let response = generate_browse_response(&params.object_id, &[], &files, state, &server_ip).await;
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
                error!("ZeroCopy error getting music by genre {}: {}", genre_name, e);
                
                // Record atomic error metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_error();
                state.web_metrics.record_browse_request(response_time, false);
                
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing genre tracks".to_string(),
                )
                    .into_response()
            }
        }
    } else {
        // Record atomic error metrics for invalid path
        let response_time = start_time.elapsed().as_millis() as u64;
        state.web_metrics.record_error();
        state.web_metrics.record_browse_request(response_time, false);
        
        (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Invalid genre path".to_string(),
        )
            .into_response()
    }
}

/// Handle browsing years with atomic performance tracking and ZeroCopy operations
async fn handle_years_browse(
    params: &BrowseParams,
    state: &AppState,
    audio_path: &str,
) -> Response {
    use crate::web::xml::generate_browse_response;
    
    let start_time = Instant::now();
    let path_parts: Vec<&str> = audio_path.split('/').filter(|s| !s.is_empty()).collect();
    
    if path_parts.len() == 1 {
        // List all years using ZeroCopy atomic operations
        match state.database.get_years().await {
            Ok(years) => {
                let has_data = !years.is_empty();
                let subdirectories: Vec<crate::database::MediaDirectory> = years
                    .into_iter()
                    .map(|year| crate::database::MediaDirectory {
                        path: std::path::PathBuf::from(format!("audio/years/{}", year.name)),
                        name: format!("{} ({})", year.name, year.count),
                    })
                    .collect();
                
                // Record atomic performance metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_browse_request(response_time, has_data);
                
                debug!("ZeroCopy retrieved {} years in {}ms", subdirectories.len(), response_time);
                    
                let server_ip = state.get_server_ip();
                let response = generate_browse_response(&params.object_id, &subdirectories, &[], state, &server_ip).await;
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
                error!("ZeroCopy error getting years: {}", e);
                
                // Record atomic error metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_error();
                state.web_metrics.record_browse_request(response_time, false);
                
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing years".to_string(),
                )
                    .into_response()
            }
        }
    } else if path_parts.len() == 2 {
        // List tracks by specific year using ZeroCopy atomic operations
        let year_str = path_parts[1];
        if let Ok(year) = year_str.parse::<u32>() {
            match state.database.get_music_by_year(year).await {
                Ok(files) => {
                    // Record atomic performance metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_browse_request(response_time, !files.is_empty());
                    
                    debug!("ZeroCopy retrieved {} tracks for year {} in {}ms", files.len(), year, response_time);
                    
                    let server_ip = state.get_server_ip();
                    let response = generate_browse_response(&params.object_id, &[], &files, state, &server_ip).await;
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
                    error!("ZeroCopy error getting music by year {}: {}", year, e);
                    
                    // Record atomic error metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_error();
                    state.web_metrics.record_browse_request(response_time, false);
                    
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Error browsing year tracks".to_string(),
                    )
                        .into_response()
                }
            }
        } else {
            // Record atomic error metrics for invalid year format
            let response_time = start_time.elapsed().as_millis() as u64;
            state.web_metrics.record_error();
            state.web_metrics.record_browse_request(response_time, false);
            
            (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "Invalid year format".to_string(),
            )
                .into_response()
        }
    } else {
        // Record atomic error metrics for invalid path
        let response_time = start_time.elapsed().as_millis() as u64;
        state.web_metrics.record_error();
        state.web_metrics.record_browse_request(response_time, false);
        
        (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Invalid year path".to_string(),
        )
            .into_response()
    }
}

/// Handle browsing playlists with atomic performance tracking and ZeroCopy operations
async fn handle_playlists_browse(
    params: &BrowseParams,
    state: &AppState,
    audio_path: &str,
) -> Response {
    use crate::web::xml::generate_browse_response;
    
    let start_time = Instant::now();
    let path_parts: Vec<&str> = audio_path.split('/').filter(|s| !s.is_empty()).collect();
    
    if path_parts.len() == 1 {
        // List all playlists using ZeroCopy atomic operations
        match state.database.get_playlists().await {
            Ok(playlists) => {
                let has_data = !playlists.is_empty();
                let subdirectories: Vec<crate::database::MediaDirectory> = playlists
                    .into_iter()
                    .map(|playlist| crate::database::MediaDirectory {
                        path: std::path::PathBuf::from(format!("audio/playlists/{}", playlist.id.unwrap_or(0))),
                        name: playlist.name,
                    })
                    .collect();
                
                // Record atomic performance metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_browse_request(response_time, has_data);
                
                debug!("ZeroCopy retrieved {} playlists in {}ms", subdirectories.len(), response_time);
                    
                let server_ip = state.get_server_ip();
                let response = generate_browse_response(&params.object_id, &subdirectories, &[], state, &server_ip).await;
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
                error!("ZeroCopy error getting playlists: {}", e);
                
                // Record atomic error metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_error();
                state.web_metrics.record_browse_request(response_time, false);
                
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing playlists".to_string(),
                )
                    .into_response()
            }
        }
    } else if path_parts.len() == 2 {
        // List tracks in specific playlist using ZeroCopy atomic operations
        let playlist_id_str = path_parts[1];
        if let Ok(playlist_id) = playlist_id_str.parse::<i64>() {
            match state.database.get_playlist_tracks(playlist_id).await {
                Ok(files) => {
                    // Record atomic performance metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_browse_request(response_time, !files.is_empty());
                    
                    debug!("ZeroCopy retrieved {} tracks for playlist {} in {}ms", files.len(), playlist_id, response_time);
                    
                    let server_ip = state.get_server_ip();
                    let response = generate_browse_response(&params.object_id, &[], &files, state, &server_ip).await;
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
                    error!("ZeroCopy error getting playlist tracks for {}: {}", playlist_id, e);
                    
                    // Record atomic error metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_error();
                    state.web_metrics.record_browse_request(response_time, false);
                    
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Error browsing playlist tracks".to_string(),
                    )
                        .into_response()
                }
            }
        } else {
            // Record atomic error metrics for invalid playlist ID
            let response_time = start_time.elapsed().as_millis() as u64;
            state.web_metrics.record_error();
            state.web_metrics.record_browse_request(response_time, false);
            
            (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "Invalid playlist ID format".to_string(),
            )
                .into_response()
        }
    } else {
        // Record atomic error metrics for invalid path
        let response_time = start_time.elapsed().as_millis() as u64;
        state.web_metrics.record_error();
        state.web_metrics.record_browse_request(response_time, false);
        
        (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Invalid playlist path".to_string(),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_browse_params_valid_xml() {
        let xml_body = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
    <s:Body>
        <u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
            <ObjectID>video/movies</ObjectID>
            <BrowseFlag>BrowseDirectChildren</BrowseFlag>
            <Filter>*</Filter>
            <StartingIndex>10</StartingIndex>
            <RequestedCount>25</RequestedCount>
            <SortCriteria></SortCriteria>
        </u:Browse>
    </s:Body>
</s:Envelope>"#;

        let params = parse_browse_params(xml_body);
        assert_eq!(params.object_id, "video/movies");
        assert_eq!(params.starting_index, 10);
        assert_eq!(params.requested_count, 25);
    }

    #[test]
    fn test_parse_browse_params_minimal_xml() {
        let xml_body = r#"<ObjectID>0</ObjectID><StartingIndex>0</StartingIndex><RequestedCount>0</RequestedCount>"#;

        let params = parse_browse_params(xml_body);
        assert_eq!(params.object_id, "0");
        assert_eq!(params.starting_index, 0);
        assert_eq!(params.requested_count, 0);
    }

    #[test]
    fn test_parse_browse_params_missing_elements() {
        let xml_body = r#"<ObjectID>audio/artists</ObjectID>"#;

        let params = parse_browse_params(xml_body);
        assert_eq!(params.object_id, "audio/artists");
        assert_eq!(params.starting_index, 0); // Default value
        assert_eq!(params.requested_count, 0); // Default value
    }

    #[test]
    fn test_parse_browse_params_invalid_numbers() {
        let xml_body = r#"<ObjectID>test</ObjectID><StartingIndex>invalid</StartingIndex><RequestedCount>not_a_number</RequestedCount>"#;

        let params = parse_browse_params(xml_body);
        assert_eq!(params.object_id, "test");
        assert_eq!(params.starting_index, 0); // Falls back to default
        assert_eq!(params.requested_count, 0); // Falls back to default
    }

    #[test]
    fn test_parse_browse_params_empty_xml() {
        let xml_body = "";

        let params = parse_browse_params(xml_body);
        assert_eq!(params.object_id, "0"); // Default value
        assert_eq!(params.starting_index, 0); // Default value
        assert_eq!(params.requested_count, 0); // Default value
    }

    #[test]
    fn test_parse_browse_params_malformed_xml() {
        let xml_body = r#"<ObjectID>test</ObjectID><StartingIndex>5<RequestedCount>10</RequestedCount>"#;

        let params = parse_browse_params(xml_body);
        // Should handle malformed XML gracefully and extract what it can
        assert_eq!(params.object_id, "test");
        // The parser should still work despite the malformed StartingIndex tag
    }

    #[test]
    fn test_parse_browse_params_with_whitespace() {
        let xml_body = r#"
        <ObjectID>  video/series  </ObjectID>
        <StartingIndex>  5  </StartingIndex>
        <RequestedCount>  15  </RequestedCount>
        "#;

        let params = parse_browse_params(xml_body);
        assert_eq!(params.object_id, "video/series"); // Should be trimmed
        assert_eq!(params.starting_index, 5);
        assert_eq!(params.requested_count, 15);
    }

    #[test]
    fn test_parse_browse_params_performance_comparison() {
        // This test demonstrates that the new XML parser handles complex XML correctly
        // while the old string-based approach would be fragile
        let complex_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" 
            s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
            <ObjectID>video/movies/action</ObjectID>
            <BrowseFlag>BrowseDirectChildren</BrowseFlag>
            <Filter>dc:title,dc:date,upnp:class,res@duration,res@size</Filter>
            <StartingIndex>100</StartingIndex>
            <RequestedCount>50</RequestedCount>
            <SortCriteria>+dc:title</SortCriteria>
        </u:Browse>
    </s:Body>
</s:Envelope>"#;

        let params = parse_browse_params(complex_xml);
        assert_eq!(params.object_id, "video/movies/action");
        assert_eq!(params.starting_index, 100);
        assert_eq!(params.requested_count, 50);
    }

}/// 
/// Get web handler performance metrics for monitoring
pub async fn get_web_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.web_metrics.get_stats();
    
    let metrics_json = serde_json::json!({
        "web_handler_metrics": {
            "browse_requests": stats.browse_requests,
            "cache_hits": stats.cache_hits,
            "cache_misses": stats.cache_misses,
            "cache_hit_rate_percent": stats.cache_hit_rate,
            "directory_listings": stats.directory_listings,
            "file_serves": stats.file_serves,
            "errors": stats.errors,
            "average_response_time_ms": stats.average_response_time_ms,
            "zerocopy_database": "active"
        }
    });
    
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        metrics_json.to_string(),
    )
}
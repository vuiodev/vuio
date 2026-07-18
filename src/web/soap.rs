//! UPnP device/service descriptions and SOAP control handlers.

use crate::{
    database::{DatabaseManager, MediaDirectory},
    state::AppState,
    web::xml::{generate_description_xml, generate_scpd_xml},
};
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use std::{path::PathBuf, sync::atomic::Ordering, time::Instant};
use tracing::{debug, error, info, warn};

pub async fn description_handler<D: DatabaseManager>(
    State(state): State<AppState<D>>,
) -> impl IntoResponse {
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

const MAX_BROWSE_ITEMS_PER_RESPONSE: usize = 2_000;

fn browse_page_limit(params: &BrowseParams) -> usize {
    if params.requested_count == 0 {
        MAX_BROWSE_ITEMS_PER_RESPONSE
    } else {
        (params.requested_count as usize).min(MAX_BROWSE_ITEMS_PER_RESPONSE)
    }
}

fn browse_page_bounds(params: &BrowseParams, total_matches: usize) -> std::ops::Range<usize> {
    let start = (params.starting_index as usize).min(total_matches);
    let end = start
        .saturating_add(browse_page_limit(params))
        .min(total_matches);
    start..end
}

fn soap_action(headers: &HeaderMap, body: &str) -> Result<String, Box<Response>> {
    let header_action = headers
        .get("soapaction")
        .and_then(|value| value.to_str().ok())
        .and_then(parse_soap_action_header);
    let body_action = match body_soap_action(body) {
        Ok(action) => action,
        Err(message) => return Err(Box::new(invalid_soap_request(message))),
    };

    if let Some(header_action) = header_action {
        if header_action != body_action {
            return Err(Box::new(invalid_soap_request(
                "SOAPAction header does not match the SOAP body",
            )));
        }
        Ok(header_action)
    } else {
        Ok(body_action)
    }
}

fn parse_soap_action_header(value: &str) -> Option<String> {
    let value = value.trim().trim_matches('"');
    let action = value
        .rsplit_once('#')
        .map(|(_, action)| action)
        .unwrap_or(value)
        .trim();
    (!action.is_empty()).then(|| action.to_string())
}

fn body_soap_action(body: &str) -> Result<String, &'static str> {
    use quick_xml::{events::Event, Reader};

    let mut reader = Reader::from_str(body);
    let mut buffer = Vec::new();
    let mut in_body = false;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) => {
                let qualified_name = element.name();
                let name = local_xml_name(qualified_name.as_ref());
                if in_body {
                    return Ok(name.to_string());
                }
                if name == "Body" {
                    in_body = true;
                }
            }
            Ok(Event::Empty(element)) if in_body => {
                return Ok(local_xml_name(element.name().as_ref()).to_string());
            }
            Ok(Event::End(element)) if local_xml_name(element.name().as_ref()) == "Body" => {
                break;
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err("Malformed SOAP XML"),
            _ => {}
        }
        buffer.clear();
    }
    Err("SOAP body has no action element")
}

fn xml_element_text(body: &str, expected_name: &str) -> Option<String> {
    use quick_xml::{events::Event, Reader};

    let mut reader = Reader::from_str(body);
    let mut buffer = Vec::new();
    let mut capture = false;
    loop {
        match reader.read_event_into(&mut buffer).ok()? {
            Event::Start(element) => {
                capture = local_xml_name(element.name().as_ref()) == expected_name;
            }
            Event::Text(text) if capture => {
                return reader
                    .decoder()
                    .decode(text.as_ref())
                    .ok()
                    .map(|value| value.into_owned());
            }
            Event::End(_) => capture = false,
            Event::Eof => return None,
            _ => {}
        }
        buffer.clear();
    }
}

fn local_xml_name(name: &[u8]) -> &str {
    let local = name
        .iter()
        .rposition(|byte| *byte == b':')
        .map(|position| &name[position + 1..])
        .unwrap_or(name);
    std::str::from_utf8(local).unwrap_or_default()
}

fn invalid_soap_request(message: &'static str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        message,
    )
        .into_response()
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
                let text = reader.decoder().decode(e.as_ref()).unwrap_or_default();
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

    debug!(
        "Parsed browse params - ObjectID: '{}', StartingIndex: {}, RequestedCount: {}",
        object_id, starting_index, requested_count
    );

    BrowseParams {
        object_id,
        starting_index,
        requested_count,
    }
}

/// Content Directory Handler struct to encapsulate specialized browse handlers
struct ContentDirectoryHandler;

impl ContentDirectoryHandler {
    /// Handle video browse requests
    async fn handle_video_browse<D: DatabaseManager + 'static>(
        params: &BrowseParams,
        state: &AppState<D>,
        path_prefix_str: &str,
    ) -> Response {
        Self::handle_folder_browse(params, state, "video/", path_prefix_str).await
    }

    /// Handle music browse requests (folder-based, not categorized)
    async fn handle_music_browse<D: DatabaseManager + 'static>(
        params: &BrowseParams,
        state: &AppState<D>,
        path_prefix_str: &str,
    ) -> Response {
        Self::handle_folder_browse(params, state, "audio/", path_prefix_str).await
    }

    /// Handle image browse requests
    async fn handle_image_browse<D: DatabaseManager + 'static>(
        params: &BrowseParams,
        state: &AppState<D>,
        path_prefix_str: &str,
    ) -> Response {
        Self::handle_folder_browse(params, state, "image/", path_prefix_str).await
    }

    /// Handle generic folder-based browse requests with consistent path normalization
    /// Enhanced with atomic performance tracking and cache-friendly operations
    async fn handle_folder_browse<D: DatabaseManager + 'static>(
        params: &BrowseParams,
        state: &AppState<D>,
        media_type_filter: &str,
        path_prefix_str: &str,
    ) -> Response {
        use crate::web::xml::generate_browse_response;

        let start_time = Instant::now();

        let client = crate::web::client::CURRENT_CLIENT
            .try_with(|c| *c)
            .unwrap_or(crate::web::client::DlnaClientProfile::Standard);

        let current_update_id = state.content_update_id.load(Ordering::SeqCst);
        let browse_epoch = state.browse_cache.lock().await.epoch();
        let cache_key = crate::state::SoapCacheKey {
            object_id: params.object_id.clone(),
            starting_index: params.starting_index,
            requested_count: params.requested_count,
            client_profile: client,
            content_update_id: current_update_id,
            browse_epoch,
        };

        // Cache lookup
        if state.content_update_id.load(Ordering::SeqCst) == current_update_id {
            let mut cache = state.browse_cache.lock().await;
            let needs_clear = cache
                .generation()
                .is_some_and(|generation| generation != current_update_id);
            if needs_clear {
                cache.clear();
            }
            if let Some(cached_xml) = cache.get(&cache_key) {
                let response_time = start_time.elapsed().as_micros() as u64;
                state.web_metrics.record_browse_request(response_time, true);
                state.web_metrics.record_directory_listing(response_time);
                debug!(
                    "Browse Cache Hit for Folder ObjectID: {} ({}ms)",
                    params.object_id, response_time
                );
                return (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                        (header::HeaderName::from_static("ext"), ""),
                    ],
                    cached_xml.clone(),
                )
                    .into_response();
            }
        }

        let cache_hit = false;

        let monitored_dirs = state.media_directories.read().await.clone();
        let unavailable_roots = state.unavailable_roots.read().await.clone();

        // Parse directory index prefix (e.g. "d0/movies" -> index 0, relative path "movies")
        let (dir_index_opt, relative_path) = parse_dir_index_prefix(path_prefix_str);

        let browse_path = match dir_index_opt {
            Some(idx) if idx < monitored_dirs.len() => {
                let base_path = PathBuf::from(&monitored_dirs[idx].path);
                if relative_path.is_empty() {
                    base_path
                } else {
                    base_path.join(relative_path)
                }
            }
            _ => {
                let media_root = state.current_config().get_primary_media_dir();
                if path_prefix_str.is_empty() {
                    media_root
                } else {
                    media_root.join(path_prefix_str)
                }
            }
        };

        // If there are multiple monitored directories and we are at the root, return virtual folders
        let (subdirectories, files) = if path_prefix_str.is_empty() && monitored_dirs.len() > 1 {
            let mut subdirs = Vec::new();
            for (idx, dir) in monitored_dirs.iter().enumerate() {
                let path = PathBuf::from(&dir.path);
                if !path.is_dir() || unavailable_roots.contains(&path) {
                    continue;
                }
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| dir.path.clone());
                subdirs.push(MediaDirectory {
                    path: PathBuf::from(format!("d{}", idx)),
                    name,
                });
            }
            (subdirs, Vec::<crate::database::MediaFile>::new())
        } else if !browse_path.is_dir()
            || unavailable_roots
                .iter()
                .any(|root| browse_path.starts_with(root))
        {
            // Configured/removable roots are hidden while unavailable. The watcher
            // recovery loop will rescan and republish them when they return.
            (Vec::new(), Vec::new())
        } else {
            // Apply canonical path normalization to match how paths are stored in the database
            let canonical_browse_path = match state
                .filesystem_manager
                .get_canonical_path(&browse_path)
            {
                Ok(canonical) => std::path::PathBuf::from(canonical),
                Err(e) => {
                    warn!("Failed to get canonical path for browse request '{}': {}, using basic normalization", browse_path.display(), e);
                    state.web_metrics.record_error();
                    state.filesystem_manager.normalize_path(&browse_path)
                }
            };

            let requested_count = browse_page_limit(params);
            let bookmarks = if matches!(
                client,
                crate::web::client::DlnaClientProfile::SamsungTv
                    | crate::web::client::DlnaClientProfile::SamsungTvQ
            ) {
                state.bookmarks.lock().await.snapshot()
            } else {
                std::collections::HashMap::new()
            };
            let context = crate::web::xml::BrowseRenderContext {
                client,
                server_ip: state.get_server_ip(),
                server_port: state.current_config().server.port,
                autoplay_enabled: state.current_config().media.autoplay_enabled,
                update_id: current_update_id,
                bookmarks,
            };
            let canonical_parent = canonical_browse_path.to_string_lossy().into_owned();
            let mime_family = media_type_filter.to_owned();
            let object_id = params.object_id.clone();
            let starting_index = params.starting_index as usize;
            let database = state.database.clone();
            let query = database.read(move |session| {
                crate::web::xml::generate_indexed_browse_response(
                    session,
                    &canonical_parent,
                    &mime_family,
                    &object_id,
                    starting_index,
                    requested_count,
                    context,
                )
            });

            let response =
                match tokio::time::timeout(std::time::Duration::from_secs(30), query).await {
                    Ok(Ok(response)) => response,
                    Ok(Err(error)) => {
                        error!("ReDB browse failed for {}: {}", params.object_id, error);
                        state.web_metrics.record_error();
                        return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
                            .into_response();
                    }
                    Err(_) => {
                        error!("Database query timed out for {}", params.object_id);
                        state.web_metrics.record_error();
                        return (
                            StatusCode::REQUEST_TIMEOUT,
                            "Request timeout - directory too large",
                        )
                            .into_response();
                    }
                };

            let response_time = start_time.elapsed().as_micros() as u64;
            state
                .web_metrics
                .record_browse_request(response_time, false);
            state.web_metrics.record_directory_listing(response_time);
            state
                .browse_cache
                .lock()
                .await
                .insert(cache_key, response.clone());
            return (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                    (header::HeaderName::from_static("ext"), ""),
                ],
                response,
            )
                .into_response();
        };

        debug!(
            "ReDB browse request for '{}' (filter: '{}') returned {} subdirs, {} files",
            browse_path.display(),
            media_type_filter,
            subdirectories.len(),
            files.len()
        );

        // This fallback is used only for virtual/unavailable roots. Persisted
        // directory and file listings return through the indexed visitor above.
        let total_matches = subdirectories.len();
        let page = browse_page_bounds(params, total_matches);
        let starting_index = page.start;
        let end_index = page.end;
        let paginated_subdirs = &subdirectories[page];

        debug!(
            "ReDB returning paginated results: {} subdirs, {} files (index {}-{} of {})",
            paginated_subdirs.len(),
            0,
            starting_index,
            end_index,
            total_matches
        );

        // Record atomic performance metrics
        let response_time = start_time.elapsed().as_micros() as u64;
        state
            .web_metrics
            .record_browse_request(response_time, cache_hit);
        state.web_metrics.record_directory_listing(response_time);

        let server_ip = state.get_server_ip();
        let response = generate_browse_response(
            &params.object_id,
            paginated_subdirs,
            &[],
            state,
            &server_ip,
            total_matches,
        )
        .await;

        // Cache insert
        {
            let mut cache = state.browse_cache.lock().await;
            let needs_clear = cache
                .generation()
                .is_some_and(|generation| generation != current_update_id);
            if needs_clear {
                cache.clear();
            }
            cache.insert(cache_key, response.clone().into());
        }

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

    /// Handle root browse request (ObjectID "0")
    async fn handle_root_browse<D: DatabaseManager>(
        params: &BrowseParams,
        state: &AppState<D>,
    ) -> Response {
        use crate::web::xml::generate_browse_response;

        let containers = [
            MediaDirectory {
                path: PathBuf::from("video"),
                name: "Video".to_string(),
            },
            MediaDirectory {
                path: PathBuf::from("audio"),
                name: "Music".to_string(),
            },
            MediaDirectory {
                path: PathBuf::from("image"),
                name: "Pictures".to_string(),
            },
            MediaDirectory {
                path: PathBuf::from("radio"),
                name: "Radio".to_string(),
            },
        ];
        let page = browse_page_bounds(params, containers.len());
        let server_ip = state.get_server_ip();
        let response = generate_browse_response(
            "0",
            &containers[page],
            &[],
            state,
            &server_ip,
            containers.len(),
        )
        .await;
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

    /// Handle radio browse request
    async fn handle_radio_browse<D: DatabaseManager + 'static>(
        params: &BrowseParams,
        state: &AppState<D>,
    ) -> Response {
        let context = crate::web::xml::BrowseRenderContext {
            client: crate::web::client::CURRENT_CLIENT
                .try_with(|client| *client)
                .unwrap_or(crate::web::client::DlnaClientProfile::Standard),
            server_ip: state.get_server_ip(),
            server_port: state.current_config().server.port,
            autoplay_enabled: state.current_config().media.autoplay_enabled,
            update_id: state.content_update_id.load(Ordering::SeqCst),
            bookmarks: state.bookmarks.lock().await.snapshot(),
        };
        let starting_index = params.starting_index as usize;
        let requested_count = browse_page_limit(params);
        let response = match state
            .database
            .clone()
            .read(move |session| {
                crate::web::xml::generate_indexed_items_response(
                    session,
                    crate::database::MediaFileQuery::Filtered {
                        after_id: None,
                        mime_family: Some("audio/radio".to_owned()),
                        text: None,
                    },
                    "radio",
                    starting_index,
                    requested_count,
                    context,
                )
            })
            .await
        {
            Ok(response) => response,
            Err(error) => {
                error!(%error, "Radio browse query failed");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };
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

pub async fn content_directory_control<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let client = crate::web::client::detect_client(&headers);
    crate::web::client::CURRENT_CLIENT.scope(client, async move {
        let action = match soap_action(&headers, &body) {
            Ok(action) => action,
            Err(response) => return *response,
        };
        if action == "Browse" {
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
                } else if audio_path.starts_with("folders") {
                    let folder_path = audio_path.strip_prefix("folders").unwrap_or("").trim_start_matches('/');
                    return ContentDirectoryHandler::handle_music_browse(&params, &state, folder_path).await;
                } else {
                    // Traditional folder browsing within audio
                    return ContentDirectoryHandler::handle_music_browse(&params, &state, audio_path).await;
                }
            } else if params.object_id.starts_with("image") {
                let path_prefix_str = params.object_id.strip_prefix("image").unwrap_or("").trim_start_matches('/');
                return ContentDirectoryHandler::handle_image_browse(&params, &state, path_prefix_str).await;
            } else if params.object_id.starts_with("radio") {
                return ContentDirectoryHandler::handle_radio_browse(&params, &state).await;
            } else {
                // This case might happen for deeper browsing or custom object IDs.
                // Assume no specific type filter for the database query, and the object_id itself
                // represents the path relative to the media root.
                return ContentDirectoryHandler::handle_folder_browse(&params, &state, "", params.object_id.as_str()).await;
            }
        } else if action == "GetSearchCapabilities" {
            let content = "<SearchCaps>dc:creator,dc:date,dc:title,upnp:album,upnp:actor,upnp:artist,upnp:class,upnp:genre,@refID</SearchCaps>";
            build_soap_response("GetSearchCapabilities", "urn:schemas-upnp-org:service:ContentDirectory:1", content)
        } else if action == "GetSortCapabilities" {
            let content = "<SortCaps>dc:title,dc:date,upnp:class,upnp:album,upnp:originalTrackNumber</SortCaps>";
            build_soap_response("GetSortCapabilities", "urn:schemas-upnp-org:service:ContentDirectory:1", content)
        } else if action == "GetSystemUpdateID" {
            let update_id = state.content_update_id.load(Ordering::SeqCst);
            let content = format!("<Id>{}</Id>", update_id);
            build_soap_response("GetSystemUpdateID", "urn:schemas-upnp-org:service:ContentDirectory:1", &content)
        } else if action == "X_GetFeatureList" {
            let content = r#"<FeatureList>&lt;?xml version="1.0" encoding="utf-8"?&gt;&lt;Features xmlns="urn:schemas-upnp-org:av:avs" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="urn:schemas-upnp-org:av:avs http://www.upnp.org/schemas/av/avs.xsd"&gt;&lt;Feature name="samsung.com_BASICVIEW" version="1"&gt;&lt;container id="1" type="object.item.audioItem"/&gt;&lt;container id="2" type="object.item.videoItem"/&gt;&lt;container id="3" type="object.item.imageItem"/&gt;&lt;/Feature&gt;&lt;/Features&gt;</FeatureList>"#;
            build_soap_response("X_GetFeatureList", "urn:schemas-upnp-org:service:ContentDirectory:1", content)
        } else if action == "X_SetBookmark" {
            let object_id = xml_element_text(&body, "ObjectID");
            let pos_second = xml_element_text(&body, "PosSecond");
            if let (Some(object_id), Some(pos_second)) = (object_id, pos_second) {
              if let (Ok(file_id), Ok(pos)) = (object_id.parse::<i64>(), pos_second.parse::<u32>()) {
                if state.database.get_file_location_by_id(file_id).await.ok().flatten().is_none() {
                    return (StatusCode::BAD_REQUEST, "Unknown media ID").into_response();
                }
                let mut bookmarks_guard = state.bookmarks.lock().await;
                bookmarks_guard.insert(file_id, pos);
                drop(bookmarks_guard);
                crate::web::eventing::invalidate_browse_responses(&state).await;
              }
            }
            build_soap_response("X_SetBookmark", "urn:schemas-upnp-org:service:ContentDirectory:1", "")
        } else {
            (
                StatusCode::NOT_IMPLEMENTED,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "Not implemented".to_string(),
            )
                .into_response()
        }
    }).await
}

// Music categorization handlers

/// Handle browsing the root audio container with music categorization
async fn handle_audio_root_browse<D: DatabaseManager>(
    params: &BrowseParams,
    state: &AppState<D>,
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

    let total_matches = subdirectories.len();
    let page = browse_page_bounds(params, total_matches);
    let server_ip = state.get_server_ip();
    let response = generate_browse_response(
        &params.object_id,
        &subdirectories[page],
        &[],
        state,
        &server_ip,
        total_matches,
    )
    .await;
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
    /// Handle artist browse requests with atomic performance tracking and ReDB operations
    async fn handle_artist_browse<D: DatabaseManager + 'static>(
        params: &BrowseParams,
        state: &AppState<D>,
        audio_path: &str,
    ) -> Response {
        let database = state.database.clone();
        handle_generic_category_browse(
            params,
            state,
            audio_path,
            "artists",
            move || async move { database.get_artists().await },
            |artist| crate::database::MediaDirectory {
                path: std::path::PathBuf::from(format!("audio/artists/{}", artist.name)),
                name: format!("{} ({})", artist.name, artist.count),
            },
        )
        .await
    }

    /// Handle album browse requests with atomic performance tracking and ReDB operations
    async fn handle_album_browse<D: DatabaseManager + 'static>(
        params: &BrowseParams,
        state: &AppState<D>,
        audio_path: &str,
    ) -> Response {
        let database = state.database.clone();
        handle_generic_category_browse(
            params,
            state,
            audio_path,
            "albums",
            move || async move { database.get_albums(None).await },
            |album| crate::database::MediaDirectory {
                path: std::path::PathBuf::from(format!("audio/albums/{}", album.name)),
                name: format!("{} ({})", album.name, album.count),
            },
        )
        .await
    }
}

/// Handle browsing genres with atomic performance tracking and ReDB operations
async fn handle_genres_browse<D: DatabaseManager + 'static>(
    params: &BrowseParams,
    state: &AppState<D>,
    audio_path: &str,
) -> Response {
    let database = state.database.clone();
    handle_generic_category_browse(
        params,
        state,
        audio_path,
        "genres",
        move || async move { database.get_genres().await },
        |genre| crate::database::MediaDirectory {
            path: std::path::PathBuf::from(format!("audio/genres/{}", genre.name)),
            name: format!("{} ({})", genre.name, genre.count),
        },
    )
    .await
}

/// Handle browsing years with atomic performance tracking and ReDB operations
async fn handle_years_browse<D: DatabaseManager + 'static>(
    params: &BrowseParams,
    state: &AppState<D>,
    audio_path: &str,
) -> Response {
    let database = state.database.clone();
    handle_generic_category_browse(
        params,
        state,
        audio_path,
        "years",
        move || async move { database.get_years().await },
        |year| crate::database::MediaDirectory {
            path: std::path::PathBuf::from(format!("audio/years/{}", year.name)),
            name: format!("{} ({})", year.name, year.count),
        },
    )
    .await
}

/// Handle browsing playlists with atomic performance tracking and ReDB operations
async fn handle_playlists_browse<D: DatabaseManager + 'static>(
    params: &BrowseParams,
    state: &AppState<D>,
    audio_path: &str,
) -> Response {
    let database = state.database.clone();
    handle_generic_category_browse(
        params,
        state,
        audio_path,
        "playlists",
        move || async move { database.get_playlists().await },
        |playlist| crate::database::MediaDirectory {
            path: std::path::PathBuf::from(format!("audio/playlists/{}", playlist.id.unwrap_or(0))),
            name: playlist.name,
        },
    )
    .await
}

/// Helper function to perform generic music category browsing
async fn handle_generic_category_browse<D, C, F, FFuture>(
    params: &BrowseParams,
    state: &AppState<D>,
    audio_path: &str,
    category_name: &str,
    list_categories_fn: F,
    map_category_fn: impl Fn(C) -> crate::database::MediaDirectory,
) -> Response
where
    D: DatabaseManager + 'static,
    F: FnOnce() -> FFuture,
    FFuture: std::future::Future<Output = Result<Vec<C>, anyhow::Error>>,
{
    use crate::web::xml::generate_browse_response;

    let start_time = Instant::now();

    let client = crate::web::client::CURRENT_CLIENT
        .try_with(|c| *c)
        .unwrap_or(crate::web::client::DlnaClientProfile::Standard);

    let current_update_id = state.content_update_id.load(Ordering::SeqCst);
    let browse_epoch = state.browse_cache.lock().await.epoch();
    let cache_key = crate::state::SoapCacheKey {
        object_id: params.object_id.clone(),
        starting_index: params.starting_index,
        requested_count: params.requested_count,
        client_profile: client,
        content_update_id: current_update_id,
        browse_epoch,
    };

    // Cache lookup
    {
        let mut cache = state.browse_cache.lock().await;
        let needs_clear = cache
            .generation()
            .is_some_and(|generation| generation != current_update_id);
        if needs_clear {
            cache.clear();
        }
        if let Some(cached_xml) = cache.get(&cache_key) {
            let response_time = start_time.elapsed().as_micros() as u64;
            state.web_metrics.record_browse_request(response_time, true);
            debug!(
                "Browse Cache Hit for Category ObjectID: {} ({}ms)",
                params.object_id, response_time
            );
            return (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                    (header::HeaderName::from_static("ext"), ""),
                ],
                cached_xml.clone(),
            )
                .into_response();
        }
    }

    // Find if we are browsing a category list (e.g. "artists") or filtering by a category value (e.g. "artists/AC/DC")
    let (is_category_list, key_str_opt) = if let Some(slash_idx) = audio_path.find('/') {
        let key_raw = &audio_path[slash_idx + 1..];
        let key_str = percent_encoding::percent_decode_str(key_raw)
            .decode_utf8_lossy()
            .into_owned();
        (false, Some(key_str))
    } else {
        (true, None)
    };

    if is_category_list {
        match list_categories_fn().await {
            Ok(categories) => {
                let has_data = !categories.is_empty();
                let subdirectories: Vec<crate::database::MediaDirectory> =
                    categories.into_iter().map(map_category_fn).collect();
                let total_matches = subdirectories.len();
                let page = browse_page_bounds(params, total_matches);

                let response_time = start_time.elapsed().as_micros() as u64;
                state
                    .web_metrics
                    .record_browse_request(response_time, has_data);

                debug!(
                    "ReDB retrieved {} {} in {}ms",
                    subdirectories.len(),
                    category_name,
                    response_time
                );

                let server_ip = state.get_server_ip();
                let response = generate_browse_response(
                    &params.object_id,
                    &subdirectories[page],
                    &[],
                    state,
                    &server_ip,
                    total_matches,
                )
                .await;

                // Cache insert
                if state.content_update_id.load(Ordering::SeqCst) == current_update_id {
                    let mut cache = state.browse_cache.lock().await;
                    let needs_clear = cache
                        .generation()
                        .is_some_and(|generation| generation != current_update_id);
                    if needs_clear {
                        cache.clear();
                    }
                    cache.insert(cache_key.clone(), response.clone().into());
                }

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
                error!("ReDB error getting {}: {}", category_name, e);

                let response_time = start_time.elapsed().as_micros() as u64;
                state.web_metrics.record_error();
                state
                    .web_metrics
                    .record_browse_request(response_time, false);

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Internal Server Error",
                )
                    .into_response()
            }
        }
    } else if let Some(key_str) = key_str_opt {
        let query = match category_name {
            "artists" => crate::database::MediaFileQuery::Artist(key_str.clone()),
            "albums" => crate::database::MediaFileQuery::Album {
                album: key_str.clone(),
                artist: None,
            },
            "genres" => crate::database::MediaFileQuery::Genre(key_str.clone()),
            "years" => match key_str.parse() {
                Ok(year) => crate::database::MediaFileQuery::Year(year),
                Err(_) => return (StatusCode::BAD_REQUEST, "Invalid year").into_response(),
            },
            "playlists" => match key_str.parse() {
                Ok(id) => crate::database::MediaFileQuery::Playlist(id),
                Err(_) => return (StatusCode::BAD_REQUEST, "Invalid playlist ID").into_response(),
            },
            _ => return (StatusCode::BAD_REQUEST, "Unknown category").into_response(),
        };
        let requested_count = browse_page_limit(params);
        let bookmarks = if matches!(
            client,
            crate::web::client::DlnaClientProfile::SamsungTv
                | crate::web::client::DlnaClientProfile::SamsungTvQ
        ) {
            state.bookmarks.lock().await.snapshot()
        } else {
            std::collections::HashMap::new()
        };
        let context = crate::web::xml::BrowseRenderContext {
            client,
            server_ip: state.get_server_ip(),
            server_port: state.current_config().server.port,
            autoplay_enabled: state.current_config().media.autoplay_enabled,
            update_id: current_update_id,
            bookmarks,
        };
        let object_id = params.object_id.clone();
        let starting_index = params.starting_index as usize;
        let database = state.database.clone();
        match database
            .read(move |session| {
                crate::web::xml::generate_indexed_items_response(
                    session,
                    query,
                    &object_id,
                    starting_index,
                    requested_count,
                    context,
                )
            })
            .await
        {
            Ok(response) => {
                let response_time = start_time.elapsed().as_micros() as u64;
                state.web_metrics.record_browse_request(response_time, true);

                debug!(
                    "ReDB retrieved {} tracks for {} '{}' in {}ms",
                    "zero-copy", category_name, key_str, response_time
                );

                // Cache insert
                if state.content_update_id.load(Ordering::SeqCst) == current_update_id {
                    let mut cache = state.browse_cache.lock().await;
                    let needs_clear = cache
                        .generation()
                        .is_some_and(|generation| generation != current_update_id);
                    if needs_clear {
                        cache.clear();
                    }
                    cache.insert(cache_key.clone(), response.clone());
                }

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
                error!(
                    "ReDB error getting music by {} {}: {}",
                    category_name, key_str, e
                );

                let response_time = start_time.elapsed().as_micros() as u64;
                state.web_metrics.record_error();
                state
                    .web_metrics
                    .record_browse_request(response_time, false);

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Internal Server Error",
                )
                    .into_response()
            }
        }
    } else {
        let response_time = start_time.elapsed().as_micros() as u64;
        state.web_metrics.record_error();
        state
            .web_metrics
            .record_browse_request(response_time, false);

        (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            format!("Invalid {} path", category_name),
        )
            .into_response()
    }
}

fn parse_dir_index_prefix(path_prefix_str: &str) -> (Option<usize>, &str) {
    if path_prefix_str.starts_with('d') {
        let chars = path_prefix_str.chars().skip(1);
        let mut num_str = String::new();
        for c in chars {
            if c.is_ascii_digit() {
                num_str.push(c);
            } else {
                break;
            }
        }
        if !num_str.is_empty() {
            if let Ok(idx) = num_str.parse::<usize>() {
                let prefix_len = 1 + num_str.len();
                let rem = if path_prefix_str.len() > prefix_len {
                    path_prefix_str[prefix_len..].trim_start_matches('/')
                } else {
                    ""
                };
                (Some(idx), rem)
            } else {
                (None, path_prefix_str)
            }
        } else {
            (None, path_prefix_str)
        }
    } else {
        (None, path_prefix_str)
    }
}

fn build_soap_response(action: &str, service_type: &str, content: &str) -> Response {
    let mut xml =
        String::with_capacity(300 + action.len() * 2 + service_type.len() + content.len());
    xml.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    xml.push_str("<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\n");
    xml.push_str("    <s:Body>\n");
    xml.push_str("        <u:");
    xml.push_str(action);
    xml.push_str("Response xmlns:u=\"");
    xml.push_str(service_type);
    xml.push_str("\">\n");
    xml.push_str("            ");
    xml.push_str(content);
    xml.push('\n');
    xml.push_str("        </u:");
    xml.push_str(action);
    xml.push_str("Response>\n");
    xml.push_str("    </s:Body>\n");
    xml.push_str("</s:Envelope>");

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
            (header::HeaderName::from_static("ext"), ""),
        ],
        xml,
    )
        .into_response()
}

pub async fn connection_manager_scpd() -> impl IntoResponse {
    let xml = crate::web::xml::generate_connection_manager_scpd();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        xml,
    )
}

pub async fn media_receiver_registrar_scpd() -> impl IntoResponse {
    let xml = crate::web::xml::generate_registrar_scpd();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        xml,
    )
}

pub async fn connection_manager_control<D: DatabaseManager>(
    State(_state): State<AppState<D>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let action = match soap_action(&headers, &body) {
        Ok(action) => action,
        Err(response) => return *response,
    };
    if action == "GetProtocolInfo" {
        let content = r#"<Source>http-get:*:video/x-msvideo:*,http-get:*:video/mp4:*,http-get:*:video/x-matroska:*,http-get:*:video/x-mkv:*,http-get:*:video/mpeg:*,http-get:*:video/divx:*,http-get:*:audio/mpeg:*,http-get:*:audio/x-flac:*,http-get:*:audio/wav:*,http-get:*:audio/mp4:*,http-get:*:image/jpeg:*,http-get:*:image/png:*,http-get:*:image/gif:*</Source><Sink></Sink>"#;
        build_soap_response(
            "GetProtocolInfo",
            "urn:schemas-upnp-org:service:ConnectionManager:1",
            content,
        )
    } else if action == "GetCurrentConnectionIDs" {
        let content = "<ConnectionIDs>0</ConnectionIDs>";
        build_soap_response(
            "GetCurrentConnectionIDs",
            "urn:schemas-upnp-org:service:ConnectionManager:1",
            content,
        )
    } else if action == "GetCurrentConnectionInfo" {
        let content = r#"<RcsID>-1</RcsID><AVTransportID>-1</AVTransportID><ProtocolInfo></ProtocolInfo><PeerConnectionManager></PeerConnectionManager><PeerConnectionID>-1</PeerConnectionID><Direction>Output</Direction><Status>Unknown</Status>"#;
        build_soap_response(
            "GetCurrentConnectionInfo",
            "urn:schemas-upnp-org:service:ConnectionManager:1",
            content,
        )
    } else {
        (
            StatusCode::NOT_IMPLEMENTED,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Not implemented".to_string(),
        )
            .into_response()
    }
}

pub async fn media_receiver_registrar_control<D: DatabaseManager>(
    State(_state): State<AppState<D>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let action = match soap_action(&headers, &body) {
        Ok(action) => action,
        Err(response) => return *response,
    };
    if action == "IsAuthorized" {
        let content = "<Result>1</Result>";
        build_soap_response(
            "IsAuthorized",
            "urn:microsoft.com:service:X_MS_MediaReceiverRegistrar:1",
            content,
        )
    } else if action == "RegisterDevice" {
        let content = "<RegistrationRespMsg></RegistrationRespMsg>";
        build_soap_response(
            "RegisterDevice",
            "urn:microsoft.com:service:X_MS_MediaReceiverRegistrar:1",
            content,
        )
    } else {
        (
            StatusCode::NOT_IMPLEMENTED,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Not implemented".to_string(),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soap_action_ignores_action_names_in_comments() {
        let headers = HeaderMap::new();
        let body = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><!-- <u:Browse/> --><u:GetSystemUpdateID xmlns:u="urn:test"/></s:Body></s:Envelope>"#;
        assert_eq!(soap_action(&headers, body).unwrap(), "GetSystemUpdateID");
    }

    #[test]
    fn soap_action_rejects_header_body_mismatch() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "soapaction",
            "\"urn:schemas-upnp-org:service:ContentDirectory:1#Browse\""
                .parse()
                .unwrap(),
        );
        let body = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><u:GetSystemUpdateID xmlns:u="urn:test"/></s:Body></s:Envelope>"#;
        assert!(soap_action(&headers, body).is_err());
    }

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
        let xml_body =
            r#"<ObjectID>test</ObjectID><StartingIndex>5<RequestedCount>10</RequestedCount>"#;

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

    #[test]
    fn test_parse_dir_index_prefix() {
        assert_eq!(parse_dir_index_prefix("d0"), (Some(0), ""));
        assert_eq!(parse_dir_index_prefix("d0/movies"), (Some(0), "movies"));
        assert_eq!(
            parse_dir_index_prefix("d12/movies/action"),
            (Some(12), "movies/action")
        );
        assert_eq!(parse_dir_index_prefix("d0/"), (Some(0), ""));
        assert_eq!(parse_dir_index_prefix("movies"), (None, "movies"));
        assert_eq!(parse_dir_index_prefix("d"), (None, "d"));
        assert_eq!(parse_dir_index_prefix("dx"), (None, "dx"));
        assert_eq!(parse_dir_index_prefix(""), (None, ""));
    }
}

use super::*;

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

/// Content Directory Handler struct to encapsulate specialized browse handlers
pub(super) struct ContentDirectoryHandler;

impl ContentDirectoryHandler {
    /// Handle video browse requests
    pub(super) async fn handle_video_browse<D: DatabaseManager + 'static>(
        params: &BrowseParams,
        state: &AppState<D>,
        path_prefix_str: &str,
    ) -> Response {
        Self::handle_folder_browse(params, state, "video/", path_prefix_str).await
    }

    /// Handle music browse requests (folder-based, not categorized)
    pub(super) async fn handle_music_browse<D: DatabaseManager + 'static>(
        params: &BrowseParams,
        state: &AppState<D>,
        path_prefix_str: &str,
    ) -> Response {
        Self::handle_folder_browse(params, state, "audio/", path_prefix_str).await
    }

    /// Handle image browse requests
    pub(super) async fn handle_image_browse<D: DatabaseManager + 'static>(
        params: &BrowseParams,
        state: &AppState<D>,
        path_prefix_str: &str,
    ) -> Response {
        Self::handle_folder_browse(params, state, "image/", path_prefix_str).await
    }

    /// Handle generic folder-based browse requests with consistent path normalization
    /// Enhanced with atomic performance tracking and cache-friendly operations
    pub(super) async fn handle_folder_browse<D: DatabaseManager + 'static>(
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
                server_port: state.http_binding.port(),
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
    pub(super) async fn handle_root_browse<D: DatabaseManager>(
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
    pub(super) async fn handle_radio_browse<D: DatabaseManager + 'static>(
        params: &BrowseParams,
        state: &AppState<D>,
    ) -> Response {
        let context = crate::web::xml::BrowseRenderContext {
            client: crate::web::client::CURRENT_CLIENT
                .try_with(|client| *client)
                .unwrap_or(crate::web::client::DlnaClientProfile::Standard),
            server_ip: state.get_server_ip(),
            server_port: state.http_binding.port(),
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
            let mut params = parse_browse_params(&body);
            info!("Browse request - ObjectID: {}, BrowseFlag: {:?}, StartingIndex: {}, RequestedCount: {}",
                  params.object_id, params.browse_flag, params.starting_index, params.requested_count);

            // Samsung BASICVIEW (X_GetFeatureList) advertises containers 1/2/3; some
            // firmwares (DCM10) use A/V/I. Rewrite to our real media roots so nested
            // container IDs stay on the video/audio/image paths.
            match params.object_id.as_str() {
                "1" | "A" => params.object_id = "audio".to_string(),
                "2" | "V" => params.object_id = "video".to_string(),
                "3" | "I" => params.object_id = "image".to_string(),
                _ => {}
            }

            // Samsung probes folders with BrowseMetadata and uses childCount to decide
            // whether the folder is empty. Always answer with a single container.
            if params.browse_flag == BrowseFlag::Metadata {
                return handle_browse_metadata(&params, &state).await;
            }

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
            let content = r#"<FeatureList>&lt;?xml version="1.0" encoding="utf-8"?&gt;&lt;Features xmlns="urn:schemas-upnp-org:av:avs" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="urn:schemas-upnp-org:av:avs http://www.upnp.org/schemas/av/avs.xsd"&gt;&lt;Feature name="samsung.com_BASICVIEW" version="1"&gt;&lt;container id="A" type="object.item.audioItem"/&gt;&lt;container id="V" type="object.item.videoItem"/&gt;&lt;container id="I" type="object.item.imageItem"/&gt;&lt;/Feature&gt;&lt;/Features&gt;</FeatureList>"#;
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

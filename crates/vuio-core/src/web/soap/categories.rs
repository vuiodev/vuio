use super::*;

impl ContentDirectoryHandler {
    /// Handle artist browse requests with atomic performance tracking and database operations
    pub(super) async fn handle_artist_browse<D: DatabaseManager + 'static>(
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

    /// Handle album browse requests with atomic performance tracking and database operations
    pub(super) async fn handle_album_browse<D: DatabaseManager + 'static>(
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

/// Handle browsing genres with atomic performance tracking and database operations
pub(super) async fn handle_genres_browse<D: DatabaseManager + 'static>(
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

/// Handle browsing years with atomic performance tracking and database operations
pub(super) async fn handle_years_browse<D: DatabaseManager + 'static>(
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

/// Handle browsing playlists with atomic performance tracking and database operations
pub(super) async fn handle_playlists_browse<D: DatabaseManager + 'static>(
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
pub(super) async fn handle_generic_category_browse<D, C, F, FFuture>(
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
                    "Retrieved {} {} in {}ms",
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
                error!("Database error getting {}: {}", category_name, e);

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
            server_port: state.http_binding.port(),
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
                    "Retrieved {} tracks for {} '{}' in {}ms",
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
                    "Database error getting music by {} {}: {}",
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

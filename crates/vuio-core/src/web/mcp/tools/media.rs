use super::super::*;
use crate::database::{MediaInfoOverlay, MusicCategoryFilter, MusicCategoryType};

// ──────────────────────────────────────────
// Library discovery
// ──────────────────────────────────────────

/// The configured media roots.
///
/// Without this an agent has to guess a path for `browse_folder`, which it
/// cannot do: the roots are wherever the operator put their library.
pub(crate) async fn tool_list_library_roots<D: DatabaseManager + 'static>(
    state: &AppState<D>,
) -> Result<serde_json::Value, String> {
    let directories = state.media_directories.read().await.clone();
    let unavailable = state.unavailable_roots.read().await.clone();

    let mut roots = Vec::with_capacity(directories.len());
    for directory in directories {
        let path = std::path::PathBuf::from(&directory.path);
        let canonical = canonical_for_browse(state, &path);
        let file_count = state
            .database
            .clone()
            .read({
                let canonical = canonical.to_string_lossy().into_owned();
                move |session| {
                    Ok(session
                        .visit_files(
                            &MediaFileQuery::Directory {
                                path: canonical,
                                mime_family: None,
                            },
                            0,
                            0,
                            |_| Ok(()),
                        )?
                        .matched)
                }
            })
            .await
            .unwrap_or(0);
        roots.push(serde_json::json!({
            "path": canonical.to_string_lossy(),
            "name": path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| directory.path.clone()),
            "file_count": file_count,
            "recursive": directory.recursive,
            "available": !unavailable.contains(&path),
        }));
    }

    Ok(serde_json::json!({ "roots": roots }))
}

// ──────────────────────────────────────────
// Search and listing
// ──────────────────────────────────────────

pub(crate) async fn tool_search_media<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'query' parameter")?
        .to_string();
    let limit = requested_limit(args, 50);
    let after_id = cursor_after_id(args)?;
    let mime_family = mime_family_for_category(args)?;
    let (matches_json, next_cursor) =
        query_media_page(state, after_id, mime_family, Some(query), limit).await?;

    Ok(serde_json::json!({
        "total_matches": matches_json.len(),
        "files": matches_json,
        "next_cursor": next_cursor
    }))
}

pub(crate) async fn tool_list_media<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let limit = requested_limit(args, 100);
    let after_id = cursor_after_id(args)?;
    let mime_family = mime_family_for_category(args)?;
    let (filtered_json, next_cursor) =
        query_media_page(state, after_id, mime_family, None, limit).await?;

    Ok(serde_json::json!({
        "total_files": filtered_json.len(),
        "files": filtered_json,
        "next_cursor": next_cursor
    }))
}

pub(crate) async fn tool_browse_folder<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let path_str = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'path' parameter")?;
    let mime_family = mime_family_for_category(args)?;
    let limit = args
        .get("limit")
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
        .unwrap_or(200)
        .clamp(1, 500);
    let offset = args
        .get("offset")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as usize;

    let canonical = canonical_media_path(state, path_str).await?;
    let origin = server_origin(state);
    let min_confidence = state.current_config().mediainfo.min_confidence;

    let query = MediaFileQuery::Directory {
        path: canonical.clone(),
        mime_family: mime_family.clone(),
    };
    let parent = std::path::Path::new(&canonical)
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned());

    let listing_path = canonical.clone();
    let (directories, files, total) = state
        .database
        .clone()
        .read(move |session| {
            // Folders and files are one ordered listing, so a single offset
            // walks the folders first and then continues into the files —
            // the same arithmetic `/api/browse` does, so the two agree on what
            // "page 2 of this folder" means.
            let directory_count = session
                .visit_direct_subdirectories(&listing_path, mime_family.as_deref(), 0, 0, |_| Ok(()))?
                .matched;
            let directory_limit = limit.min(directory_count.saturating_sub(offset));
            let file_offset = offset.saturating_sub(directory_count);
            let file_limit = limit.saturating_sub(directory_limit);

            let mut directories = Vec::new();
            session.visit_direct_subdirectories(
                &listing_path,
                mime_family.as_deref(),
                offset,
                directory_limit,
                |directory| {
                    directories.push(serde_json::json!({
                        "name": directory.name(),
                        "path": directory.path(),
                        "file_count": directory.file_count(),
                    }));
                    Ok(())
                },
            )?;

            let mut ids = Vec::with_capacity(file_limit);
            session.visit_files(&query, file_offset, file_limit, |file| {
                if let Some(id) = file.id().filter(|id| *id > 0) {
                    ids.push(id);
                }
                Ok(())
            })?;
            let overlays = session
                .mediainfo_overlays(&ids, min_confidence)
                .unwrap_or_default();

            let mut files = Vec::new();
            let summary = session.visit_files(&query, file_offset, file_limit, |file| {
                let overlay = file.id().and_then(|id| overlays.get(&id));
                files.push(media_file_view_to_json(&file, &origin, overlay));
                Ok(())
            })?;
            Ok((directories, files, directory_count + summary.matched))
        })
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(serde_json::json!({
        "path": canonical,
        "parent": parent,
        "directories": directories,
        "files": files,
        "total": total,
        "offset": offset
    }))
}

pub(crate) async fn tool_get_media_info<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let file_id = args
        .get("file_id")
        .and_then(|v| v.as_i64())
        .ok_or("Missing 'file_id' parameter")?;

    let origin = server_origin(state);
    let min_confidence = state.current_config().mediainfo.min_confidence;
    let file = state
        .database
        .clone()
        .read(move |session| {
            let overlays = session
                .mediainfo_overlays(&[file_id], min_confidence)
                .unwrap_or_default();
            let mut result = None;
            session.visit_files(&MediaFileQuery::Id(file_id), 0, 1, |file| {
                result = Some(media_file_view_to_json(
                    &file,
                    &origin,
                    overlays.get(&file_id),
                ));
                Ok(())
            })?;
            Ok(result)
        })
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or(format!("File with ID {} not found", file_id))?;

    Ok(file)
}

// ──────────────────────────────────────────
// Music facets
// ──────────────────────────────────────────

pub(crate) async fn tool_list_music_categories<D: DatabaseManager>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let kind = args
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'kind' parameter")?;
    let (category_type, child) = match kind {
        "artist" => (MusicCategoryType::Artist, Some(MusicCategoryType::Album)),
        "album" => (MusicCategoryType::Album, None),
        "album_artist" => (
            MusicCategoryType::AlbumArtist,
            Some(MusicCategoryType::Album),
        ),
        "genre" => (MusicCategoryType::Genre, Some(MusicCategoryType::Artist)),
        "year" => (MusicCategoryType::Year, None),
        other => {
            return Err(format!(
                "Unknown category kind '{other}'. Use artist, album, album_artist, genre or year."
            ))
        }
    };

    let filter = MusicCategoryFilter {
        artist: args
            .get("artist")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        genre: args
            .get("genre")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        ..MusicCategoryFilter::default()
    };

    let categories = state
        .database
        .get_music_categories(category_type, &filter, child)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    let categories: Vec<serde_json::Value> = categories
        .into_iter()
        .map(|category| {
            serde_json::json!({
                "name": category.name,
                "track_count": category.count,
                "child_count": category.child_count,
                "sample_file_id": category.sample_id,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "kind": kind,
        "total": categories.len(),
        "categories": categories
    }))
}

pub(crate) async fn tool_find_music<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let text = |key: &str| args.get(key).and_then(|v| v.as_str()).map(str::to_owned);
    let year = args
        .get("year")
        .and_then(|value| value.as_u64())
        .map(|value| value as u32);

    let query = MediaFileQuery::Music {
        artist: text("artist"),
        album_artist: text("album_artist"),
        album: text("album"),
        genre: text("genre"),
        year,
        // Internet radio is audio but not music, and it carries no tags to have
        // matched on in the first place.
        exclude_radio: true,
    };
    if matches!(
        &query,
        MediaFileQuery::Music {
            artist: None,
            album_artist: None,
            album: None,
            genre: None,
            year: None,
            ..
        }
    ) {
        return Err(
            "Give at least one of artist, album_artist, album, genre or year. \
             To list everything, use list_media."
                .to_string(),
        );
    }

    let limit = requested_limit(args, 200);
    let origin = server_origin(state);
    let files = state
        .database
        .clone()
        .read(move |session| {
            let mut page = Vec::new();
            session.visit_files(&query, 0, limit, |file| {
                page.push(media_file_view_to_json(&file, &origin, None));
                Ok(())
            })?;
            Ok(page)
        })
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(serde_json::json!({
        "total_matches": files.len(),
        "files": files,
        "next_cursor": serde_json::Value::Null
    }))
}

// ──────────────────────────────────────────
// Server and renderers
// ──────────────────────────────────────────

pub(crate) async fn tool_get_server_stats<D: DatabaseManager>(
    state: &AppState<D>,
) -> Result<serde_json::Value, String> {
    let stats = state
        .database
        .get_stats()
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(serde_json::json!({
        "server_name": state.current_config().server.name,
        "server_url": server_origin(state),
        "total_files": stats.total_files,
        "total_size_bytes": stats.total_size,
        "total_size_human": format_bytes(stats.total_size),
        "video_files": stats.video_files,
        "audio_files": stats.audio_files,
        "image_files": stats.image_files,
        "playlists": stats.playlists,
        "database_size_bytes": stats.database_size
    }))
}

#[cfg(feature = "casting")]
pub(crate) async fn tool_list_renderers<D: DatabaseManager>(
    state: &AppState<D>,
) -> Result<serde_json::Value, String> {
    let renderers = state
        .discovered_tvs
        .get_or_refresh()
        .await
        .map_err(|e| format!("Renderer discovery error: {}", e))?;

    let renderer_list: Vec<serde_json::Value> = renderers
        .iter()
        .map(|tv| {
            serde_json::json!({
                "id": tv.id,
                "friendly_name": tv.friendly_name,
                "model": tv.model_name,
                "location": tv.location_url,
                "protocol": tv.protocol,
                "pairing": tv.pairing,
                "capabilities": tv.capabilities
            })
        })
        .collect();

    Ok(serde_json::json!({
        "renderers_found": renderer_list.len(),
        "renderers": renderer_list
    }))
}

#[cfg(feature = "casting")]
pub(crate) async fn tool_cast_media_to_renderer<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let file_id = args
        .get("file_id")
        .and_then(|v| v.as_i64())
        .ok_or("Missing 'file_id' parameter")?;

    let renderer_id = args
        .get("renderer_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'renderer_id' parameter")?;

    crate::web::casting::cast_file_helper(state, file_id, renderer_id).await
}

// ──────────────────────────────────────────
// Shared helpers
// ──────────────────────────────────────────

/// The base URL a client should use to reach this server.
///
/// Every playable URL in a tool result is built from it, so an agent can hand
/// the user something they can open rather than a path on the server's disk.
pub(crate) fn server_origin<D: DatabaseManager>(state: &AppState<D>) -> String {
    format!(
        "http://{}:{}",
        state.get_server_ip(),
        state.http_binding.port()
    )
}

/// Resolve a caller-supplied folder path and prove it is inside a media root.
///
/// Canonicalized *before* the containment check, never after: `..` in the
/// request would otherwise walk out of a root that still looked like a prefix.
/// This is the same order [`crate::web::ui::browse_handler`] uses, and for the
/// same reason.
pub(crate) async fn canonical_media_path<D: DatabaseManager>(
    state: &AppState<D>,
    requested: &str,
) -> Result<String, String> {
    let requested = requested.trim();
    if requested.is_empty() {
        return Err("Path must not be empty. Use list_library_roots to find one.".to_string());
    }
    let canonical = canonical_for_browse(state, std::path::Path::new(requested));

    let roots = state.media_directories.read().await.clone();
    let canonical_roots: Vec<std::path::PathBuf> = roots
        .iter()
        .map(|directory| {
            canonical_for_browse(state, std::path::Path::new(&directory.path))
        })
        .collect();
    if !canonical_roots
        .iter()
        .any(|root| canonical == *root || canonical.starts_with(root))
    {
        return Err(format!(
            "'{requested}' is outside every configured media directory. \
             Use list_library_roots to see what is browsable."
        ));
    }
    Ok(canonical.to_string_lossy().into_owned())
}

fn canonical_for_browse<D: DatabaseManager>(
    state: &AppState<D>,
    path: &std::path::Path,
) -> std::path::PathBuf {
    match state.filesystem_manager.get_canonical_path(path) {
        Ok(canonical) => std::path::PathBuf::from(canonical),
        Err(_) => state.filesystem_manager.normalize_path(path),
    }
}

/// The MIME constraint behind a `category` argument.
///
/// The same five names `/api/media` accepts, so a category means one thing
/// across the whole server rather than one thing per caller.
fn mime_family_for_category(args: &serde_json::Value) -> Result<Option<String>, String> {
    match args.get("category").and_then(|v| v.as_str()).unwrap_or("all") {
        "all" | "" => Ok(None),
        "audio" => Ok(Some("audio/".to_string())),
        "video" => Ok(Some("video/".to_string())),
        "image" => Ok(Some("image/".to_string())),
        "radio" => Ok(Some("audio/radio".to_string())),
        other => Err(format!(
            "Unknown media category '{other}'. Use all, audio, video, image or radio."
        )),
    }
}

/// One media record, as an agent needs to see it.
///
/// Everything the index holds, plus the three URLs that make the record
/// actionable — without `stream_url` an agent can describe a file but cannot
/// give anyone a way to play it.
pub(crate) fn media_file_view_to_json(
    f: &impl MediaFileView,
    origin: &str,
    overlay: Option<&MediaInfoOverlay>,
) -> serde_json::Value {
    let id = f.id().unwrap_or_default();
    let mut value = serde_json::json!({
        "id": id,
        "filename": f.filename(),
        "path": f.path(),
        "mime_type": f.mime_type(),
        "size_bytes": f.size(),
        "size_human": format_bytes(f.size()),
        "duration_seconds": f.duration_secs(),
        "title": f.title(),
        "artist": f.artist(),
        "album": f.album(),
        "album_artist": f.album_artist(),
        "genre": f.genre(),
        "composer": f.composer(),
        "track_number": f.track_number(),
        "disc_number": f.disc_number(),
        "year": f.year(),
        "codec": f.codec(),
        "sample_rate": f.sample_rate(),
        "channels": f.channels(),
        "bits_per_sample": f.bits_per_sample(),
        "bit_rate": f.bit_rate(),
        "subtitle_available": f.subtitle_available(),
        "modified_at": f.modified_secs(),
        "stream_url": stream_url(f, origin),
        "cover_url": format!("{origin}/media/{id}/cover"),
        "subtitle_url": f
            .subtitle_available()
            .then(|| format!("{origin}/media/{id}/subtitle.vtt")),
    });

    // What an online provider said about the file, kept separate from what the
    // file says about itself. A caller that wants only the tags can ignore it.
    if let Some(overlay) = overlay {
        value["online_info"] = serde_json::json!({
            "title": overlay.title,
            "overview": overlay.overview,
            "genres": overlay.genres,
            "has_artwork": overlay.has_artwork,
        });
    }
    value
}

/// The playback URL for a record, with the extension renderers expect.
///
/// Mirrors [`crate::web::casting::helpers`]'s URL construction so a link an
/// agent hands a user is the same one a cast would have used.
fn stream_url(f: &impl MediaFileView, origin: &str) -> String {
    let id = f.id().unwrap_or_default();
    let extension = std::path::Path::new(f.filename())
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 16
                && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        });
    match extension {
        Some(extension) => format!("{origin}/media/{id}.{extension}"),
        None => format!("{origin}/media/{id}"),
    }
}

pub(crate) fn requested_limit(args: &serde_json::Value, default: usize) -> usize {
    args.get("limit")
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
        .unwrap_or(default)
        .clamp(1, 500)
}

pub(crate) fn cursor_after_id(args: &serde_json::Value) -> Result<Option<i64>, String> {
    let Some(cursor) = args.get("cursor") else {
        return Ok(None);
    };
    if cursor.is_null() {
        return Ok(None);
    }
    cursor
        .as_str()
        .ok_or("'cursor' must be a string")?
        .parse::<i64>()
        .map(Some)
        .map_err(|_| "Invalid media cursor".to_string())
}

pub(crate) async fn query_media_page<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    after_id: Option<i64>,
    mime_family: Option<String>,
    text: Option<String>,
    limit: usize,
) -> Result<(Vec<serde_json::Value>, Option<String>), String> {
    let query = MediaFileQuery::Filtered {
        after_id,
        mime_family,
        text,
    };
    let origin = server_origin(state);
    let min_confidence = state.current_config().mediainfo.min_confidence;
    let fetch_limit = limit.saturating_add(1);
    let mut files = state
        .database
        .clone()
        .read(move |session| {
            // Ids first, then rows: the session lends one row at a time and
            // cannot be queried while it is doing so, so the overlay lookup has
            // to happen between the two passes.
            let mut ids = Vec::with_capacity(fetch_limit);
            session.visit_files(&query, 0, fetch_limit, |file| {
                if let Some(id) = file.id().filter(|id| *id > 0) {
                    ids.push(id);
                }
                Ok(())
            })?;
            let overlays = session
                .mediainfo_overlays(&ids, min_confidence)
                .unwrap_or_default();

            let mut page = Vec::with_capacity(fetch_limit);
            session.visit_files(&query, 0, fetch_limit, |file| {
                let overlay = file.id().and_then(|id| overlays.get(&id));
                page.push(media_file_view_to_json(&file, &origin, overlay));
                Ok(())
            })?;
            Ok(page)
        })
        .await
        .map_err(|error| format!("Database error: {error}"))?;

    let has_more = files.len() > limit;
    if has_more {
        files.pop();
    }
    let next_cursor = has_more
        .then(|| files.last()?.get("id")?.as_i64().map(|id| id.to_string()))
        .flatten();
    Ok((files, next_cursor))
}

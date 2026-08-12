use super::super::*;

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
    let (matches_json, next_cursor) =
        query_media_page(state, after_id, None, Some(query), limit).await?;

    Ok(serde_json::json!({
        "total_matches": matches_json.len(),
        "files": matches_json,
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

    let category = args
        .get("category")
        .and_then(|v| v.as_str())
        .unwrap_or("all");

    let media_type_filter = match category {
        "audio" => Some("audio/".to_owned()),
        "video" => Some("video/".to_owned()),
        "image" => Some("image/".to_owned()),
        _ => None,
    };

    let browse_path = std::path::PathBuf::from(path_str);
    let canonical_path = state
        .filesystem_manager
        .get_canonical_path(&browse_path)
        .map_err(|error| format!("Invalid browse path: {error}"))?;
    let query = MediaFileQuery::Directory {
        path: canonical_path.clone(),
        mime_family: media_type_filter.clone(),
    };
    let (dir_list, file_list) = state
        .database
        .clone()
        .read(move |session| {
            let mut directories = Vec::new();
            session.visit_direct_subdirectories(
                &canonical_path,
                media_type_filter.as_deref(),
                0,
                usize::MAX,
                |directory| {
                    directories.push(serde_json::json!({
                        "name": directory.name(),
                        "path": directory.path(),
                    }));
                    Ok(())
                },
            )?;
            let mut files = Vec::new();
            session.visit_files(&query, 0, usize::MAX, |file| {
                files.push(media_file_view_to_json(&file));
                Ok(())
            })?;
            Ok((directories, files))
        })
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(serde_json::json!({
        "path": path_str,
        "directories": dir_list,
        "files": file_list
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

    let file = state
        .database
        .clone()
        .read(move |session| {
            let mut result = None;
            session.visit_files(&MediaFileQuery::Id(file_id), 0, 1, |file| {
                result = Some(media_file_view_to_json(&file));
                Ok(())
            })?;
            Ok(result)
        })
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or(format!("File with ID {} not found", file_id))?;

    Ok(file)
}

pub(crate) async fn tool_list_media<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let category = args
        .get("category")
        .and_then(|v| v.as_str())
        .unwrap_or("all");

    let limit = requested_limit(args, 100);
    let after_id = cursor_after_id(args)?;
    let mime_family = match category {
        "all" | "" => None,
        "audio" | "video" | "image" => Some(category.to_string()),
        _ => return Err(format!("Unknown media category '{category}'")),
    };
    let (filtered_json, next_cursor) =
        query_media_page(state, after_id, mime_family, None, limit).await?;

    Ok(serde_json::json!({
        "total_files": filtered_json.len(),
        "files": filtered_json,
        "next_cursor": next_cursor
    }))
}

pub(crate) async fn tool_get_server_stats<D: DatabaseManager>(
    state: &AppState<D>,
) -> Result<serde_json::Value, String> {
    let stats = state
        .database
        .get_stats()
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    let server_ip = state.get_server_ip();
    let port = state.http_binding.port();

    Ok(serde_json::json!({
        "server_name": state.current_config().server.name,
        "server_url": format!("http://{}:{}", server_ip, port),
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

pub(crate) fn media_file_view_to_json(f: &impl MediaFileView) -> serde_json::Value {
    serde_json::json!({
        "id": f.id(),
        "filename": f.filename(),
        "path": f.path(),
        "mime_type": f.mime_type(),
        "size_bytes": f.size(),
        "size_human": format_bytes(f.size()),
        "duration_seconds": f.duration_secs(),
        "title": f.title(),
        "artist": f.artist(),
        "album": f.album(),
        "genre": f.genre(),
        "track_number": f.track_number(),
        "year": f.year(),
        "album_artist": f.album_artist()
    })
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
    let fetch_limit = limit.saturating_add(1);
    let mut files = state
        .database
        .clone()
        .read(move |session| {
            let mut page = Vec::with_capacity(fetch_limit);
            session.visit_files(&query, 0, fetch_limit, |file| {
                page.push(media_file_view_to_json(&file));
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

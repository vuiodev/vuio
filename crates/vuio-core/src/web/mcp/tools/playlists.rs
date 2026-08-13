use super::super::*;
use super::{media_file_view_to_json, server_origin};

pub(crate) async fn tool_list_playlists<D: DatabaseManager>(
    state: &AppState<D>,
) -> Result<serde_json::Value, String> {
    let playlists = state
        .database
        .get_playlists()
        .await
        .map_err(|e| format!("Database error: {}", e))?;
    let counts = state
        .database
        .count_playlist_entries()
        .await
        .unwrap_or_default();

    let list: Vec<serde_json::Value> = playlists
        .into_iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "description": p.description,
                "track_count": p.id.and_then(|id| counts.get(&id)).copied().unwrap_or(0),
                "created_at": p.created_at.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
                "updated_at": p.updated_at.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
            })
        })
        .collect();

    Ok(serde_json::json!({
        "playlists": list
    }))
}

pub(crate) async fn tool_create_playlist<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'name' parameter")?;
    let description = args.get("description").and_then(|v| v.as_str());

    let id = state
        .database
        .create_playlist(name, description)
        .await
        .map_err(|e| format!("Database error: {}", e))?;
    crate::web::eventing::publish_content_change(state).await;

    Ok(serde_json::json!({
        "playlist_id": id,
        "status": "created",
        "name": name
    }))
}

pub(crate) async fn tool_delete_playlist<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = args
        .get("playlist_id")
        .and_then(|v| v.as_i64())
        .ok_or("Missing 'playlist_id' parameter")?;

    let deleted = state
        .database
        .delete_playlist(id)
        .await
        .map_err(|e| format!("Database error: {}", e))?;
    if deleted {
        crate::web::eventing::publish_content_change(state).await;
    }

    Ok(serde_json::json!({
        "playlist_id": id,
        "status": if deleted { "deleted" } else { "not_found" }
    }))
}

pub(crate) async fn tool_add_to_playlist<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let playlist_id = args
        .get("playlist_id")
        .and_then(|v| v.as_i64())
        .ok_or("Missing 'playlist_id' parameter")?;

    let ids_to_add = positioned_track_ids(args)?;

    let entry_ids = state
        .database
        .batch_add_to_playlist(playlist_id, &ids_to_add)
        .await
        .map_err(|e| format!("Database error: {}", e))?;
    if !entry_ids.is_empty() {
        crate::web::eventing::publish_content_change(state).await;
    }

    Ok(serde_json::json!({
        "playlist_id": playlist_id,
        "tracks_added": entry_ids.len(),
        "status": "success"
    }))
}

pub(crate) async fn tool_reorder_playlist<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let playlist_id = args
        .get("playlist_id")
        .and_then(|v| v.as_i64())
        .ok_or("Missing 'playlist_id' parameter")?;

    let positions = positioned_track_ids(args)?;
    if positions.is_empty() {
        return Err("'media_file_ids' must list at least one track".to_string());
    }

    state
        .database
        .reorder_playlist(playlist_id, &positions)
        .await
        .map_err(|e| format!("Database error: {}", e))?;
    crate::web::eventing::publish_content_change(state).await;

    Ok(serde_json::json!({
        "playlist_id": playlist_id,
        "tracks": positions.len(),
        "status": "reordered"
    }))
}

pub(crate) async fn tool_remove_from_playlist<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let playlist_id = args
        .get("playlist_id")
        .and_then(|v| v.as_i64())
        .ok_or("Missing 'playlist_id' parameter")?;

    let media_file_id = args
        .get("media_file_id")
        .and_then(|v| v.as_i64())
        .ok_or("Missing 'media_file_id' parameter")?;

    let removed = state
        .database
        .remove_from_playlist(playlist_id, media_file_id)
        .await
        .map_err(|e| format!("Database error: {}", e))?;
    if removed {
        crate::web::eventing::publish_content_change(state).await;
    }

    Ok(serde_json::json!({
        "playlist_id": playlist_id,
        "media_file_id": media_file_id,
        "status": if removed { "removed" } else { "not_found" }
    }))
}

pub(crate) async fn tool_get_playlist_tracks<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let playlist_id = args
        .get("playlist_id")
        .and_then(|v| v.as_i64())
        .ok_or("Missing 'playlist_id' parameter")?;

    let origin = server_origin(state);
    let list = state
        .database
        .clone()
        .read(move |session| {
            let mut tracks = Vec::new();
            session.visit_files(
                &MediaFileQuery::Playlist(playlist_id),
                0,
                usize::MAX,
                |file| {
                    tracks.push(media_file_view_to_json(&file, &origin, None));
                    Ok(())
                },
            )?;
            Ok(tracks)
        })
        .await
        .map_err(|e| format!("Database error: {e}"))?;

    Ok(serde_json::json!({
        "playlist_id": playlist_id,
        "tracks_count": list.len(),
        "tracks": list
    }))
}

/// Read `media_file_ids` as `(id, position)` pairs in the order given.
///
/// Shared by adding and reordering because both mean the same thing by the
/// array: the caller's order *is* the playback order.
fn positioned_track_ids(args: &serde_json::Value) -> Result<Vec<(i64, u32)>, String> {
    let media_file_ids = args
        .get("media_file_ids")
        .and_then(|v| v.as_array())
        .ok_or("Missing 'media_file_ids' parameter")?;

    media_file_ids
        .iter()
        .enumerate()
        .map(|(position, value)| {
            value
                .as_i64()
                .map(|id| (id, position as u32))
                .ok_or_else(|| "Invalid media_file_id, must be an integer".to_string())
        })
        .collect()
}

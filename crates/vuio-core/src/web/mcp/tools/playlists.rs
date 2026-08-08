use super::super::*;
use super::media_file_view_to_json;

pub(crate) async fn tool_list_playlists<D: DatabaseManager>(
    state: &AppState<D>,
) -> Result<serde_json::Value, String> {
    let playlists = state
        .database
        .get_playlists()
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    let list: Vec<serde_json::Value> = playlists
        .into_iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "description": p.description,
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

    let media_file_ids = args
        .get("media_file_ids")
        .and_then(|v| v.as_array())
        .ok_or("Missing 'media_file_ids' parameter")?;

    let mut ids_to_add = Vec::new();
    for (pos, val) in media_file_ids.iter().enumerate() {
        let id = val
            .as_i64()
            .ok_or("Invalid media_file_id, must be integer")?;
        ids_to_add.push((id, pos as u32));
    }

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
                    tracks.push(media_file_view_to_json(&file));
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

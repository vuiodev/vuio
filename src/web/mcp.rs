use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::state::AppState;
use crate::tv_control;

// ──────────────────────────────────────────
// JSON-RPC 2.0 types
// ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub id: Option<serde_json::Value>,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<serde_json::Value>, code: i64, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

// ──────────────────────────────────────────
// MCP tool definitions
// ──────────────────────────────────────────

fn get_tools_list() -> serde_json::Value {
    serde_json::json!({
        "tools": [
            {
                "name": "search_media",
                "description": "Search media files (video, audio, images) by keyword in filename or title. Returns matching files with their IDs, paths, types and metadata.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search keyword to match against filenames and titles"
                        }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "browse_folder",
                "description": "Browse files and subdirectories in a specific folder path. Returns directories and files at that location.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "The folder path to browse (relative to media root, or absolute path)"
                        },
                        "category": {
                            "type": "string",
                            "enum": ["all", "audio", "video", "image"],
                            "description": "Optional media type filter. Defaults to 'all'."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "get_media_info",
                "description": "Get detailed metadata for a specific media file by its numeric ID. Returns title, artist, album, duration, size, mime type and more.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_id": {
                            "type": "integer",
                            "description": "The numeric ID of the media file"
                        }
                    },
                    "required": ["file_id"]
                }
            },
            {
                "name": "get_server_stats",
                "description": "Get server statistics including total media file counts by type (video, audio, image), total library size, and database size.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "list_tvs",
                "description": "Discover UPnP/DLNA MediaRenderer devices (smart TVs, speakers, media players) on the local network. Returns their friendly names for use with cast_media_to_tv.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "cast_media_to_tv",
                "description": "Cast a media file to a smart TV or media renderer by name. First use search_media to find the file ID and list_tvs to find the TV name.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_id": {
                            "type": "integer",
                            "description": "The numeric ID of the media file to cast"
                        },
                        "tv_name": {
                            "type": "string",
                            "description": "The friendly name of the TV (as returned by list_tvs). Partial, case-insensitive match is supported."
                        }
                    },
                    "required": ["file_id", "tv_name"]
                }
            },
            {
                "name": "control_tv",
                "description": "Send a playback control command to a smart TV or media renderer. Use after casting media to control playback.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tv_name": {
                            "type": "string",
                            "description": "The friendly name of the TV"
                        },
                        "action": {
                            "type": "string",
                            "enum": ["play", "pause", "stop"],
                            "description": "The playback action to perform"
                        }
                    },
                    "required": ["tv_name", "action"]
                }
            },
            {
                "name": "list_media",
                "description": "List all media files indexed on the server, optionally filtered by category (video, audio, image). Returns a flat list containing IDs, filenames, titles, paths, size and mime type. Useful for getting an overview of what files exist.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "category": {
                            "type": "string",
                            "enum": ["all", "audio", "video", "image"],
                            "description": "Optional category filter. Defaults to 'all'."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of files to return (defaults to 100)"
                        }
                    }
                }
            }
        ]
    })
}

// ──────────────────────────────────────────
// SSE Handler — GET /sse
// ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SseQuery {
    #[serde(default)]
    pub client_id: Option<String>,
}

pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let client_id = Uuid::new_v4().to_string();
    let (tx, rx) = mpsc::channel::<String>(64);

    // Register this client
    {
        let mut clients = state.mcp_clients.lock().await;
        clients.insert(client_id.clone(), tx);
    }

    info!("MCP client connected: {}", client_id);

    // Build the SSE stream
    let client_id_for_cleanup = client_id.clone();
    let state_for_cleanup = state.clone();

    let endpoint_url = format!("/mcp/message?client_id={}", client_id);

    let initial_event = Event::default()
        .event("endpoint")
        .data(endpoint_url);

    let rx_stream = ReceiverStream::new(rx).map(|msg| {
        Ok(Event::default().event("message").data(msg))
    });

    let stream = futures_util::stream::once(async move {
        Ok::<_, Infallible>(initial_event)
    })
    .chain(rx_stream);

    // Spawn a cleanup task that fires when the stream is dropped
    tokio::spawn(async move {
        // Wait a bit to detect disconnection — this is cleaned up when
        // the channel sender is dropped by the client map removal
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
        let mut clients = state_for_cleanup.mcp_clients.lock().await;
        if clients.remove(&client_id_for_cleanup).is_some() {
            info!("MCP client disconnected (timeout): {}", client_id_for_cleanup);
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ──────────────────────────────────────────
// Message Handler — POST /mcp/message
// ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct MessageQuery {
    pub client_id: String,
}

pub async fn message_handler(
    State(state): State<AppState>,
    Query(query): Query<MessageQuery>,
    body: String,
) -> impl IntoResponse {
    let client_id = &query.client_id;

    // Parse JSON-RPC request
    let request: JsonRpcRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            let err_response = JsonRpcResponse::error(
                None,
                -32700,
                format!("Parse error: {}", e),
            );
            return (
                StatusCode::OK,
                axum::Json(err_response),
            ).into_response();
        }
    };

    debug!("MCP request from {}: method={}", client_id, request.method);

    // Handle the method
    let response = handle_method(&state, &request).await;

    // Send the response back through the SSE channel
    let response_json = serde_json::to_string(&response).unwrap_or_default();
    let mut sent_via_sse = false;
    {
        let clients = state.mcp_clients.lock().await;
        if let Some(tx) = clients.get(client_id) {
            if let Err(e) = tx.send(response_json).await {
                warn!("Failed to send MCP response to client {}: {}", client_id, e);
            } else {
                sent_via_sse = true;
            }
        } else {
            warn!("MCP client {} not found in client map", client_id);
        }
    }

    if sent_via_sse {
        (StatusCode::ACCEPTED, "").into_response()
    } else {
        (StatusCode::OK, axum::Json(response)).into_response()
    }
}

// ──────────────────────────────────────────
// Method dispatcher
// ──────────────────────────────────────────

async fn handle_method(state: &AppState, request: &JsonRpcRequest) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => handle_initialize(request),
        "initialized" => {
            // Notification — no response needed, but we return one for the HTTP body
            JsonRpcResponse::success(request.id.clone(), serde_json::json!({}))
        }
        "tools/list" => handle_tools_list(request),
        "tools/call" => handle_tools_call(state, request).await,
        "ping" => JsonRpcResponse::success(request.id.clone(), serde_json::json!({})),
        _ => JsonRpcResponse::error(
            request.id.clone(),
            -32601,
            format!("Method not found: {}", request.method),
        ),
    }
}

fn handle_initialize(request: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(
        request.id.clone(),
        serde_json::json!({
            "protocolVersion": "2025-03-26",
            "capabilities": {
                "tools": {
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": "vuio-media-server",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )
}

fn handle_tools_list(request: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(request.id.clone(), get_tools_list())
}

async fn handle_tools_call(state: &AppState, request: &JsonRpcRequest) -> JsonRpcResponse {
    let params = match &request.params {
        Some(p) => p,
        None => {
            return JsonRpcResponse::error(
                request.id.clone(),
                -32602,
                "Missing params".to_string(),
            );
        }
    };

    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));

    let result = match tool_name {
        "search_media" => tool_search_media(state, &arguments).await,
        "browse_folder" => tool_browse_folder(state, &arguments).await,
        "get_media_info" => tool_get_media_info(state, &arguments).await,
        "get_server_stats" => tool_get_server_stats(state).await,
        "list_tvs" => tool_list_tvs().await,
        "cast_media_to_tv" => tool_cast_media_to_tv(state, &arguments).await,
        "control_tv" => tool_control_tv(&arguments).await,
        "list_media" => tool_list_media(state, &arguments).await,
        _ => Err(format!("Unknown tool: {}", tool_name)),
    };

    match result {
        Ok(content) => JsonRpcResponse::success(
            request.id.clone(),
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&content).unwrap_or_default()
                }]
            }),
        ),
        Err(e) => JsonRpcResponse::success(
            request.id.clone(),
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": format!("Error: {}", e)
                }],
                "isError": true
            }),
        ),
    }
}

// ──────────────────────────────────────────
// Tool implementations
// ──────────────────────────────────────────

async fn tool_search_media(
    state: &AppState,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'query' parameter")?
        .to_lowercase();

    let all_files = state
        .database
        .collect_all_media_files()
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    let matches: Vec<serde_json::Value> = all_files
        .into_iter()
        .filter(|f| {
            f.filename.to_lowercase().contains(&query)
                || f.title
                    .as_ref()
                    .map(|t| t.to_lowercase().contains(&query))
                    .unwrap_or(false)
                || f.artist
                    .as_ref()
                    .map(|a| a.to_lowercase().contains(&query))
                    .unwrap_or(false)
                || f.album
                    .as_ref()
                    .map(|a| a.to_lowercase().contains(&query))
                    .unwrap_or(false)
        })
        .take(50) // Limit results
        .map(|f| media_file_to_json(&f))
        .collect();

    Ok(serde_json::json!({
        "total_matches": matches.len(),
        "files": matches
    }))
}

async fn tool_browse_folder(
    state: &AppState,
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
        "audio" => "audio",
        "video" => "video",
        "image" => "image",
        _ => "",
    };

    let browse_path = std::path::PathBuf::from(path_str);

    let (dirs, files) = state
        .database
        .get_directory_listing(&browse_path, media_type_filter)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    let dir_list: Vec<serde_json::Value> = dirs
        .iter()
        .map(|d| {
            serde_json::json!({
                "name": d.name,
                "path": d.path.to_string_lossy()
            })
        })
        .collect();

    let file_list: Vec<serde_json::Value> = files.iter().map(|f| media_file_to_json(f)).collect();

    Ok(serde_json::json!({
        "path": path_str,
        "directories": dir_list,
        "files": file_list
    }))
}

async fn tool_get_media_info(
    state: &AppState,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let file_id = args
        .get("file_id")
        .and_then(|v| v.as_i64())
        .ok_or("Missing 'file_id' parameter")?;

    let file = state
        .database
        .get_file_by_id(file_id)
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or(format!("File with ID {} not found", file_id))?;

    Ok(media_file_to_json(&file))
}

async fn tool_list_media(
    state: &AppState,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let category = args
        .get("category")
        .and_then(|v| v.as_str())
        .unwrap_or("all");

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(100) as usize;

    let all_files = state
        .database
        .collect_all_media_files()
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    let filtered: Vec<serde_json::Value> = all_files
        .into_iter()
        .filter(|f| {
            if category == "all" || category.is_empty() {
                true
            } else {
                f.mime_type.to_lowercase().starts_with(category)
            }
        })
        .take(limit)
        .map(|f| media_file_to_json(&f))
        .collect();

    Ok(serde_json::json!({
        "total_files": filtered.len(),
        "files": filtered
    }))
}

async fn tool_get_server_stats(state: &AppState) -> Result<serde_json::Value, String> {
    let stats = state
        .database
        .get_stats()
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    let server_ip = state.get_server_ip();
    let port = state.config.server.port;

    Ok(serde_json::json!({
        "server_name": state.config.server.name,
        "server_url": format!("http://{}:{}", server_ip, port),
        "total_files": stats.total_files,
        "total_size_bytes": stats.total_size,
        "total_size_human": format_size(stats.total_size),
        "video_files": stats.video_files,
        "audio_files": stats.audio_files,
        "image_files": stats.image_files,
        "playlists": stats.playlists,
        "database_size_bytes": stats.database_size
    }))
}

async fn tool_list_tvs() -> Result<serde_json::Value, String> {
    let tvs = tv_control::discover_tvs()
        .await
        .map_err(|e| format!("TV discovery error: {}", e))?;

    let tv_list: Vec<serde_json::Value> = tvs
        .iter()
        .map(|tv| {
            serde_json::json!({
                "friendly_name": tv.friendly_name,
                "model": tv.model_name,
                "location": tv.location_url
            })
        })
        .collect();

    Ok(serde_json::json!({
        "tvs_found": tv_list.len(),
        "tvs": tv_list
    }))
}

async fn tool_cast_media_to_tv(
    state: &AppState,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let file_id = args
        .get("file_id")
        .and_then(|v| v.as_i64())
        .ok_or("Missing 'file_id' parameter")?;

    let tv_name_query = args
        .get("tv_name")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'tv_name' parameter")?
        .to_lowercase();

    // Look up the media file
    let file = state
        .database
        .get_file_by_id(file_id)
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or(format!("File with ID {} not found", file_id))?;

    // Discover TVs and find a match
    let tvs = tv_control::discover_tvs()
        .await
        .map_err(|e| format!("TV discovery error: {}", e))?;

    let matched_tv = tvs
        .iter()
        .find(|tv| tv.friendly_name.to_lowercase().contains(&tv_name_query))
        .ok_or(format!(
            "No TV found matching '{}'. Available TVs: {}",
            tv_name_query,
            tvs.iter()
                .map(|tv| tv.friendly_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))?;

    // Build the media URL
    let server_ip = state.get_server_ip();
    let port = state.config.server.port;
    let media_url = format!(
        "http://{}:{}/media/{}",
        server_ip,
        port,
        file.id.unwrap_or(file_id)
    );

    let title = file
        .title
        .as_deref()
        .unwrap_or(&file.filename);

    // Cast
    tv_control::cast_media(&matched_tv.control_url, &media_url, title, &file.mime_type)
        .await
        .map_err(|e| format!("Cast error: {}", e))?;

    Ok(serde_json::json!({
        "status": "playing",
        "file": file.filename,
        "tv": matched_tv.friendly_name,
        "media_url": media_url
    }))
}

async fn tool_control_tv(args: &serde_json::Value) -> Result<serde_json::Value, String> {
    let tv_name_query = args
        .get("tv_name")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'tv_name' parameter")?
        .to_lowercase();

    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'action' parameter")?;

    let soap_action = match action {
        "play" => "Play",
        "pause" => "Pause",
        "stop" => "Stop",
        _ => return Err(format!("Unknown action '{}'. Use play, pause, or stop.", action)),
    };

    // Discover TVs and find a match
    let tvs = tv_control::discover_tvs()
        .await
        .map_err(|e| format!("TV discovery error: {}", e))?;

    let matched_tv = tvs
        .iter()
        .find(|tv| tv.friendly_name.to_lowercase().contains(&tv_name_query))
        .ok_or(format!(
            "No TV found matching '{}'. Available TVs: {}",
            tv_name_query,
            tvs.iter()
                .map(|tv| tv.friendly_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))?;

    tv_control::control_playback(&matched_tv.control_url, soap_action)
        .await
        .map_err(|e| format!("Control error: {}", e))?;

    Ok(serde_json::json!({
        "status": "ok",
        "action": action,
        "tv": matched_tv.friendly_name
    }))
}

// ──────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────

fn media_file_to_json(f: &crate::database::MediaFile) -> serde_json::Value {
    let duration_secs = f.duration.map(|d| d.as_secs());

    serde_json::json!({
        "id": f.id,
        "filename": f.filename,
        "path": f.path.to_string_lossy(),
        "mime_type": f.mime_type,
        "size_bytes": f.size,
        "size_human": format_size(f.size),
        "duration_seconds": duration_secs,
        "title": f.title,
        "artist": f.artist,
        "album": f.album,
        "genre": f.genre,
        "track_number": f.track_number,
        "year": f.year,
        "album_artist": f.album_artist
    })
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_response_success() {
        let resp = JsonRpcResponse::success(Some(serde_json::json!(1)), serde_json::json!({"ok": true}));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"ok\":true"));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_json_rpc_response_error() {
        let resp = JsonRpcResponse::error(Some(serde_json::json!(2)), -32601, "Method not found".to_string());
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("-32601"));
        assert!(json.contains("Method not found"));
        assert!(!json.contains("\"result\""));
    }

    #[test]
    fn test_initialize_response() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            id: Some(serde_json::json!(1)),
            params: None,
        };
        let resp = handle_initialize(&req);
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "2025-03-26");
        assert_eq!(result["serverInfo"]["name"], "vuio-media-server");
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn test_tools_list_contains_all_tools() {
        let tools = get_tools_list();
        let tool_names: Vec<&str> = tools["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();

        assert!(tool_names.contains(&"search_media"));
        assert!(tool_names.contains(&"browse_folder"));
        assert!(tool_names.contains(&"get_media_info"));
        assert!(tool_names.contains(&"get_server_stats"));
        assert!(tool_names.contains(&"list_tvs"));
        assert!(tool_names.contains(&"cast_media_to_tv"));
        assert!(tool_names.contains(&"control_tv"));
        assert!(tool_names.contains(&"list_media"));
        assert_eq!(tool_names.len(), 8);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1_048_576), "1.0 MB");
        assert_eq!(format_size(1_073_741_824), "1.0 GB");
        assert_eq!(format_size(1_099_511_627_776), "1.0 TB");
    }
}

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

use crate::{database::DatabaseManager, state::AppState};
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
            },
            {
                "name": "list_playlists",
                "description": "List all playlists currently stored on the server.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "create_playlist",
                "description": "Create a new media playlist.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The name of the new playlist"
                        },
                        "description": {
                            "type": "string",
                            "description": "Optional description for the playlist"
                        }
                    },
                    "required": ["name"]
                }
            },
            {
                "name": "delete_playlist",
                "description": "Delete a playlist by its ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "playlist_id": {
                            "type": "integer",
                            "description": "The numeric ID of the playlist to delete"
                        }
                    },
                    "required": ["playlist_id"]
                }
            },
            {
                "name": "add_to_playlist",
                "description": "Add one or more media files to a playlist in bulk.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "playlist_id": {
                            "type": "integer",
                            "description": "The numeric ID of the target playlist"
                        },
                        "media_file_ids": {
                            "type": "array",
                            "items": {
                                "type": "integer"
                            },
                            "description": "An array of media file IDs to add to the playlist"
                        }
                    },
                    "required": ["playlist_id", "media_file_ids"]
                }
            },
            {
                "name": "remove_from_playlist",
                "description": "Remove a specific media file from a playlist.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "playlist_id": {
                            "type": "integer",
                            "description": "The numeric ID of the playlist"
                        },
                        "media_file_id": {
                            "type": "integer",
                            "description": "The numeric ID of the media file to remove"
                        }
                    },
                    "required": ["playlist_id", "media_file_id"]
                }
            },
            {
                "name": "get_playlist_tracks",
                "description": "Get all media files/tracks in a specific playlist.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "playlist_id": {
                            "type": "integer",
                            "description": "The numeric ID of the playlist"
                        }
                    },
                    "required": ["playlist_id"]
                }
            },
            {
                "name": "cast_playlist_to_tv",
                "description": "Cast a playlist to a smart TV or media renderer by name. Starts playing the first item of the playlist (or the track specified by track_index).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "playlist_id": {
                            "type": "integer",
                            "description": "The numeric ID of the playlist to cast"
                        },
                        "tv_name": {
                            "type": "string",
                            "description": "The friendly name of the TV (partial, case-insensitive match supported)"
                        },
                        "track_index": {
                            "type": "integer",
                            "description": "Optional 0-based index of the track in the playlist to start playing from (defaults to 0)"
                        }
                    },
                    "required": ["playlist_id", "tv_name"]
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
        "list_playlists" => tool_list_playlists(state).await,
        "create_playlist" => tool_create_playlist(state, &arguments).await,
        "delete_playlist" => tool_delete_playlist(state, &arguments).await,
        "add_to_playlist" => tool_add_to_playlist(state, &arguments).await,
        "remove_from_playlist" => tool_remove_from_playlist(state, &arguments).await,
        "get_playlist_tracks" => tool_get_playlist_tracks(state, &arguments).await,
        "cast_playlist_to_tv" => tool_cast_playlist_to_tv(state, &arguments).await,
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

    let mut matches: Vec<_> = all_files
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
        .collect();

    // Sort files case-insensitively by filename to maintain natural ordering (e.g. S05E01 before S05E10)
    matches.sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));

    let matches_json: Vec<serde_json::Value> = matches
        .into_iter()
        .take(50) // Limit results
        .map(|f| media_file_to_json(&f))
        .collect();

    Ok(serde_json::json!({
        "total_matches": matches_json.len(),
        "files": matches_json
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

    let mut filtered: Vec<_> = all_files
        .into_iter()
        .filter(|f| {
            if category == "all" || category.is_empty() {
                true
            } else {
                f.mime_type.to_lowercase().starts_with(category)
            }
        })
        .collect();

    // Sort files case-insensitively by filename
    filtered.sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));

    let filtered_json: Vec<serde_json::Value> = filtered
        .into_iter()
        .take(limit)
        .map(|f| media_file_to_json(&f))
        .collect();

    Ok(serde_json::json!({
        "total_files": filtered_json.len(),
        "files": filtered_json
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
async fn tool_list_playlists(state: &AppState) -> Result<serde_json::Value, String> {
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

async fn tool_create_playlist(
    state: &AppState,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'name' parameter")?;
    let description = args
        .get("description")
        .and_then(|v| v.as_str());

    let id = state
        .database
        .create_playlist(name, description)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(serde_json::json!({
        "playlist_id": id,
        "status": "created",
        "name": name
    }))
}

async fn tool_delete_playlist(
    state: &AppState,
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

    Ok(serde_json::json!({
        "playlist_id": id,
        "status": if deleted { "deleted" } else { "not_found" }
    }))
}

async fn tool_add_to_playlist(
    state: &AppState,
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
        let id = val.as_i64().ok_or("Invalid media_file_id, must be integer")?;
        ids_to_add.push((id, pos as u32));
    }

    let entry_ids = state
        .database
        .batch_add_to_playlist(playlist_id, &ids_to_add)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(serde_json::json!({
        "playlist_id": playlist_id,
        "tracks_added": entry_ids.len(),
        "status": "success"
    }))
}

async fn tool_remove_from_playlist(
    state: &AppState,
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

    Ok(serde_json::json!({
        "playlist_id": playlist_id,
        "media_file_id": media_file_id,
        "status": if removed { "removed" } else { "not_found" }
    }))
}

async fn tool_get_playlist_tracks(
    state: &AppState,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let playlist_id = args
        .get("playlist_id")
        .and_then(|v| v.as_i64())
        .ok_or("Missing 'playlist_id' parameter")?;

    let tracks = state
        .database
        .get_playlist_tracks(playlist_id)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    let list: Vec<serde_json::Value> = tracks
        .iter()
        .map(|f| media_file_to_json(f))
        .collect();

    Ok(serde_json::json!({
        "playlist_id": playlist_id,
        "tracks_count": list.len(),
        "tracks": list
    }))
}

pub async fn discover_tvs_and_cache(state: &AppState) -> Result<Vec<tv_control::DiscoveredTv>, String> {
    let tvs = tv_control::discover_tvs()
        .await
        .map_err(|e| format!("TV discovery error: {}", e))?;
        
    let mut cache = state.discovered_tvs.lock().await;
    for tv in &tvs {
        if let Some(ip) = parse_ip_from_url(&tv.location_url) {
            cache.insert(ip, tv.friendly_name.clone());
        }
    }
    
    Ok(tvs)
}

fn parse_ip_from_url(url_str: &str) -> Option<String> {
    let without_scheme = url_str.split("://").nth(1)?;
    let host_port = without_scheme.split('/').next()?;
    let host = host_port.split(':').next()?;
    Some(host.to_string())
}

pub async fn cast_playlist_helper(
    state: &AppState,
    playlist_id: i64,
    tv_name_query: &str,
    track_index: usize,
) -> Result<serde_json::Value, String> {
    let tv_name_query = tv_name_query.to_lowercase();

    // Get playlist tracks
    let tracks = state
        .database
        .get_playlist_tracks(playlist_id)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    if tracks.is_empty() {
        return Err("Cannot cast an empty playlist".to_string());
    }

    if track_index >= tracks.len() {
        return Err(format!(
            "track_index {} is out of bounds (playlist only has {} tracks)",
            track_index,
            tracks.len()
        ));
    }

    // Get selected track
    let selected_track = &tracks[track_index];
    let file_id = selected_track.id.ok_or("Media file is missing an ID")?;

    // Discover TVs and find a match
    let tvs = discover_tvs_and_cache(state).await?;

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
        file_id
    );

    let title = selected_track
        .title
        .as_deref()
        .unwrap_or(&selected_track.filename);

    // Cast selected track
    tv_control::cast_media(&matched_tv.control_url, &media_url, title, &selected_track.mime_type)
        .await
        .map_err(|e| format!("Cast error: {}", e))?;

    // Register active cast in global state
    {
        let mut casts = state.active_casts.lock().await;
        casts.insert(matched_tv.friendly_name.clone(), (selected_track.filename.clone(), std::time::Instant::now()));
    }

    // Cancel existing monitor for this TV if any
    {
        let mut monitors = state.active_monitors.lock().await;
        if let Some(cancel_tx) = monitors.remove(&matched_tv.control_url) {
            let _ = cancel_tx.send(());
        }
    }

    // Queue the next track if available (DLNA automatic transitioning)
    let mut queued_file = None;
    if track_index + 1 < tracks.len() {
        let next_track = &tracks[track_index + 1];
        if let Some(next_id) = next_track.id {
            let next_media_url = format!(
                "http://{}:{}/media/{}",
                server_ip,
                port,
                next_id
            );
            let next_title = next_track
                .title
                .as_deref()
                .unwrap_or(&next_track.filename);
            
            // Queue on the TV and log/ignore failures on non-compliant devices
            match tv_control::set_next_media(
                &matched_tv.control_url,
                &next_media_url,
                next_title,
                &next_track.mime_type,
            ).await {
                Ok(_) => {
                    queued_file = Some(next_track.filename.clone());
                }
                Err(e) => {
                    tracing::warn!("SetNextAVTransportURI not supported by TV: {}", e);
                }
            }
        }
    }

    // Spawn new queue monitor to dynamically handle subsequent track transitions
    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut monitors = state.active_monitors.lock().await;
        monitors.insert(matched_tv.control_url.clone(), cancel_tx);
    }

    let state_clone = state.clone();
    let control_url_clone = matched_tv.control_url.clone();
    let server_ip_clone = server_ip.clone();
    let matched_tv_friendly_name = matched_tv.friendly_name.clone();
    
    tokio::spawn(async move {
        let mut current_idx = track_index;
        let mut consecutive_stopped = 0;
        
        loop {
            // Check cancellation or sleep 4s
            tokio::select! {
                _ = &mut cancel_rx => {
                    debug!("Queue monitor cancelled for TV: {}", control_url_clone);
                    break;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(4)) => {}
            }
            
            // Poll TV current playing track URI and transport state
            let current_uri = match tv_control::get_position_info(&control_url_clone).await {
                Ok(uri) => uri,
                Err(e) => {
                    debug!("Queue monitor failed to get position info: {}", e);
                    continue;
                }
            };
            
            let transport_state = match tv_control::get_transport_state(&control_url_clone).await {
                Ok(st) => st,
                Err(_) => "STOPPED".to_string(),
            };
            
            // Fetch playlist tracks from DB to get the latest list
            let latest_tracks = match state_clone.database.get_playlist_tracks(playlist_id).await {
                Ok(t) => t,
                Err(_) => break,
            };
            
            if latest_tracks.is_empty() {
                break;
            }
            
            // If current track URI matches a track URL, check if the TV transitioned
            let mut matched_any = false;
            for (idx, track) in latest_tracks.iter().enumerate() {
                if let Some(id) = track.id {
                    let track_media_url = format!("http://{}:{}/media/{}", server_ip_clone, port, id);
                    if current_uri == track_media_url {
                        matched_any = true;
                        if idx != current_idx {
                            info!("Queue monitor: TV transitioned to track index {} ({})", idx, track.filename);
                            current_idx = idx;
                            
                            // Update active cast state with new playing file
                            {
                                let mut casts = state_clone.active_casts.lock().await;
                                casts.insert(matched_tv_friendly_name.clone(), (track.filename.clone(), std::time::Instant::now()));
                            }

                            // Queue the next track if available
                            if current_idx + 1 < latest_tracks.len() {
                                let next_track = &latest_tracks[current_idx + 1];
                                if let Some(next_id) = next_track.id {
                                    let next_media_url = format!("http://{}:{}/media/{}", server_ip_clone, port, next_id);
                                    let next_title = next_track.title.as_deref().unwrap_or(&next_track.filename);
                                    if let Err(e) = tv_control::set_next_media(
                                        &control_url_clone,
                                        &next_media_url,
                                        next_title,
                                        &next_track.mime_type,
                                    ).await {
                                        warn!("Queue monitor: Failed to queue next track: {}", e);
                                    } else {
                                        debug!("Queue monitor: Queued next track index {} ({})", current_idx + 1, next_track.filename);
                                    }
                                }
                            }
                        }
                        break;
                    }
                }
            }
            
            // If TV is stopped and not playing any of our tracks, increment stop checks count
            if matched_any {
                consecutive_stopped = 0;
            } else if transport_state == "STOPPED" {
                consecutive_stopped += 1;
                if consecutive_stopped >= 5 {
                    info!("Queue monitor: TV stopped playing the playlist (exited after 5 consecutive stopped checks)");
                    break;
                }
            } else {
                consecutive_stopped = 0;
            }
        }

        // Cleanup: remove active cast state on exit
        {
            let mut casts = state_clone.active_casts.lock().await;
            casts.remove(&matched_tv_friendly_name);
        }
    });

    Ok(serde_json::json!({
        "status": "playing",
        "playlist_id": playlist_id,
        "tracks_count": tracks.len(),
        "current_index": track_index,
        "current_file": selected_track.filename,
        "queued_next_file": queued_file,
        "tv": matched_tv.friendly_name,
        "media_url": media_url
    }))
}

async fn tool_cast_playlist_to_tv(
    state: &AppState,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let playlist_id = args
        .get("playlist_id")
        .and_then(|v| v.as_i64())
        .ok_or("Missing 'playlist_id' parameter")?;

    let tv_name_query = args
        .get("tv_name")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'tv_name' parameter")?;

    let track_index = args
        .get("track_index")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    cast_playlist_helper(state, playlist_id, tv_name_query, track_index).await
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
        assert!(tool_names.contains(&"list_playlists"));
        assert!(tool_names.contains(&"create_playlist"));
        assert!(tool_names.contains(&"delete_playlist"));
        assert!(tool_names.contains(&"add_to_playlist"));
        assert!(tool_names.contains(&"remove_from_playlist"));
        assert!(tool_names.contains(&"get_playlist_tracks"));
        assert!(tool_names.contains(&"cast_playlist_to_tv"));
        assert_eq!(tool_names.len(), 15);
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

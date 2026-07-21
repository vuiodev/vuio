use axum::{
    extract::{ConnectInfo, Json, Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
};
use futures_util::StreamExt;
use serde::Deserialize;
use std::{
    convert::Infallible,
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::tv_control;
use crate::web::format::format_bytes;
use crate::{
    database::{
        DatabaseManager, DatabaseReadSession, DirectoryView, FileLocation, MediaFileQuery,
        MediaFileView,
    },
    state::{AppState, McpClient},
};

const MAX_MCP_PAGE_SIZE: usize = 1000;

const MCP_MAX_CLIENTS: usize = 64;
const MCP_MAX_CLIENTS_PER_PEER: usize = 4;
const MCP_CLIENT_TTL: Duration = Duration::from_secs(30 * 60);
const MCP_MAX_RESPONSE_BYTES: usize = 1024 * 1024;

mod protocol;
pub use protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};

// ──────────────────────────────────────────
// JSON-RPC 2.0 types
// ──────────────────────────────────────────

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
                        },
                        "cursor": {
                            "type": "string",
                            "description": "Opaque cursor returned by the previous page"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Page size (defaults to 50, maximum 500)"
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
                "name": "list_renderers",
                "description": "List cached UPnP/DLNA MediaRenderer devices and their stable IDs.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "cast_media_to_renderer",
                "description": "Cast a media file to a renderer by stable ID. First use list_renderers.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_id": {
                            "type": "integer",
                            "description": "The numeric ID of the media file to cast"
                        },
                        "renderer_id": {
                            "type": "string",
                            "description": "Stable renderer ID returned by list_renderers"
                        }
                    },
                    "required": ["file_id", "renderer_id"]
                }
            },
            {
                "name": "control_renderer",
                "description": "Send a playback control command to a smart TV or media renderer. Use after casting media to control playback.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "renderer_id": {
                            "type": "string",
                            "description": "Stable renderer ID returned by list_renderers"
                        },
                        "action": {
                            "type": "string",
                            "enum": ["play", "pause", "stop"],
                            "description": "The playback action to perform"
                        }
                    },
                    "required": ["renderer_id", "action"]
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
                        },
                        "cursor": {
                            "type": "string",
                            "description": "Opaque cursor returned by the previous page"
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
                "name": "cast_playlist_to_renderer",
                "description": "Cast a playlist to a renderer by stable ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "playlist_id": {
                            "type": "integer",
                            "description": "The numeric ID of the playlist to cast"
                        },
                        "renderer_id": {
                            "type": "string",
                            "description": "Stable renderer ID returned by list_renderers"
                        },
                        "track_index": {
                            "type": "integer",
                            "description": "Optional 0-based index of the track in the playlist to start playing from (defaults to 0)"
                        }
                    },
                    "required": ["playlist_id", "renderer_id"]
                }
            }
        ]
    })
}

// ──────────────────────────────────────────
// SSE Handler — GET /sse
// ──────────────────────────────────────────

pub async fn sse_handler<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
) -> axum::response::Response {
    let client_id = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let (tx, rx) = mpsc::channel::<String>(64);
    let disconnect_sender = tx.clone();
    let expires_at = Instant::now() + MCP_CLIENT_TTL;

    // Register this client
    {
        let mut clients = state.mcp_clients.lock().await;
        let now = Instant::now();
        clients.retain(|_, client| client.expires_at > now);
        let peer_clients = clients
            .values()
            .filter(|client| client.peer == peer.ip())
            .count();
        if clients.len() >= MCP_MAX_CLIENTS || peer_clients >= MCP_MAX_CLIENTS_PER_PEER {
            return StatusCode::TOO_MANY_REQUESTS.into_response();
        }
        clients.insert(
            client_id.clone(),
            McpClient {
                sender: tx,
                peer: peer.ip(),
                expires_at,
            },
        );
    }

    info!("MCP client connected: {}", client_id);

    // Build the SSE stream
    let client_id_for_cleanup = client_id.clone();
    let state_for_cleanup = state.clone();

    let endpoint_url = format!("/mcp/message?client_id={}", client_id);

    let initial_event = Event::default().event("endpoint").data(endpoint_url);

    let rx_stream =
        ReceiverStream::new(rx).map(|msg| Ok(Event::default().event("message").data(msg)));

    let stream = futures_util::stream::once(async move { Ok::<_, Infallible>(initial_event) })
        .chain(rx_stream);

    // Sender::closed resolves as soon as Axum drops the SSE receiver, so stale
    // clients are removed immediately rather than by a coarse timeout.
    let cleanup_cancellation = state.cancellation.clone();
    state.background_tasks.spawn(async move {
        tokio::select! {
            _ = disconnect_sender.closed() => {}
            _ = cleanup_cancellation.cancelled() => {}
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(expires_at)) => {}
        }
        let mut clients = state_for_cleanup.mcp_clients.lock().await;
        let is_same_connection = clients
            .get(&client_id_for_cleanup)
            .is_some_and(|client| client.sender.same_channel(&disconnect_sender));
        if is_same_connection {
            clients.remove(&client_id_for_cleanup);
            info!("MCP client disconnected: {}", client_id_for_cleanup);
        }
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

// ──────────────────────────────────────────
// Message Handler — POST /mcp/message
// ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct MessageQuery {
    pub client_id: String,
}

pub async fn message_handler<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Query(query): Query<MessageQuery>,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let client_id = &query.client_id;

    // Validate the live, peer-bound capability before dispatching any method.
    let sender = {
        let mut clients = state.mcp_clients.lock().await;
        let now = Instant::now();
        clients.retain(|_, client| client.expires_at > now);
        clients
            .get(client_id)
            .filter(|client| client.peer == peer.ip())
            .map(|client| client.sender.clone())
    };
    let Some(sender) = sender else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    debug!("MCP request from {}: method={}", client_id, request.method);

    // Handle the method
    let response = handle_method(&state, &request).await;

    // Send the response back through the SSE channel
    let response_json = match serde_json::to_string(&response) {
        Ok(response) if response.len() <= MCP_MAX_RESPONSE_BYTES => response,
        Ok(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                axum::Json(JsonRpcResponse::error(
                    request.id.clone(),
                    -32001,
                    "Response exceeds server limit".to_owned(),
                )),
            )
                .into_response();
        }
        Err(error) => {
            warn!("Failed to serialize MCP response: {error}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    if let Err(e) = sender.send(response_json).await {
        warn!("Failed to send MCP response to client {}: {}", client_id, e);
        let mut clients = state.mcp_clients.lock().await;
        if clients
            .get(client_id)
            .is_some_and(|client| client.sender.same_channel(&sender))
        {
            clients.remove(client_id);
        }
        return StatusCode::GONE.into_response();
    }
    (StatusCode::ACCEPTED, "").into_response()
}

// ──────────────────────────────────────────
// Method dispatcher
// ──────────────────────────────────────────

async fn handle_method<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    request: &JsonRpcRequest,
) -> JsonRpcResponse {
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

async fn handle_tools_call<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    request: &JsonRpcRequest,
) -> JsonRpcResponse {
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
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let result = match tool_name {
        "search_media" => tool_search_media(state, &arguments).await,
        "browse_folder" => tool_browse_folder(state, &arguments).await,
        "get_media_info" => tool_get_media_info(state, &arguments).await,
        "get_server_stats" => tool_get_server_stats(state).await,
        "list_renderers" => tool_list_renderers(state).await,
        "cast_media_to_renderer" => tool_cast_media_to_renderer(state, &arguments).await,
        "control_renderer" => tool_control_renderer(state, &arguments).await,
        "list_media" => tool_list_media(state, &arguments).await,
        "list_playlists" => tool_list_playlists(state).await,
        "create_playlist" => tool_create_playlist(state, &arguments).await,
        "delete_playlist" => tool_delete_playlist(state, &arguments).await,
        "add_to_playlist" => tool_add_to_playlist(state, &arguments).await,
        "remove_from_playlist" => tool_remove_from_playlist(state, &arguments).await,
        "get_playlist_tracks" => tool_get_playlist_tracks(state, &arguments).await,
        "cast_playlist_to_renderer" => tool_cast_playlist_to_renderer(state, &arguments).await,
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

async fn tool_search_media<D: DatabaseManager + 'static>(
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

async fn tool_browse_folder<D: DatabaseManager + 'static>(
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
                MAX_MCP_PAGE_SIZE,
                |directory| {
                    directories.push(serde_json::json!({
                        "name": directory.name(),
                        "path": directory.path(),
                    }));
                    Ok(())
                },
            )?;
            let mut files = Vec::new();
            session.visit_files(&query, 0, MAX_MCP_PAGE_SIZE, |file| {
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

async fn tool_get_media_info<D: DatabaseManager + 'static>(
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

async fn tool_list_media<D: DatabaseManager + 'static>(
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

async fn tool_get_server_stats<D: DatabaseManager>(
    state: &AppState<D>,
) -> Result<serde_json::Value, String> {
    let stats = state
        .database
        .get_stats()
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    let server_ip = state.get_server_ip();
    let port = state.current_config().server.port;

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

async fn tool_list_renderers<D: DatabaseManager>(
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
                "location": tv.location_url
            })
        })
        .collect();

    Ok(serde_json::json!({
        "renderers_found": renderer_list.len(),
        "renderers": renderer_list
    }))
}

async fn tool_cast_media_to_renderer<D: DatabaseManager + 'static>(
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

    // Look up the media file
    let file = state
        .database
        .get_file_location_by_id(file_id)
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .ok_or(format!("File with ID {} not found", file_id))?;

    let renderers = state
        .discovered_tvs
        .get_or_refresh()
        .await
        .map_err(|e| format!("Renderer discovery error: {}", e))?;

    let matched_tv = renderers
        .iter()
        .find(|renderer| renderer.id == renderer_id)
        .ok_or(format!(
            "No renderer found with ID '{}'. Available renderers: {}",
            renderer_id,
            renderers
                .iter()
                .map(|tv| tv.friendly_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))?;

    // Build the media URL
    let server_ip = state.get_server_ip();
    let port = state.current_config().server.port;
    let media_url = format!("http://{}:{}/media/{}", server_ip, port, file.id);

    let title = file.title.as_deref().unwrap_or(&file.filename);

    // Cast
    tv_control::cast_media(&matched_tv.control_url, &media_url, title, &file.mime_type)
        .await
        .map_err(|e| format!("Cast error: {}", e))?;

    Ok(serde_json::json!({
        "status": "playing",
        "file": file.filename,
        "renderer": matched_tv.friendly_name,
        "renderer_id": matched_tv.id,
        "media_url": media_url
    }))
}

async fn tool_control_renderer<D: DatabaseManager>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let renderer_id = args
        .get("renderer_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'renderer_id' parameter")?;

    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'action' parameter")?;

    let soap_action = match action {
        "play" => "Play",
        "pause" => "Pause",
        "stop" => "Stop",
        _ => {
            return Err(format!(
                "Unknown action '{}'. Use play, pause, or stop.",
                action
            ))
        }
    };

    let renderers = state
        .discovered_tvs
        .get_or_refresh()
        .await
        .map_err(|e| format!("Renderer discovery error: {}", e))?;

    let matched_tv = renderers
        .iter()
        .find(|renderer| renderer.id == renderer_id)
        .ok_or(format!(
            "No renderer found with ID '{}'. Available renderers: {}",
            renderer_id,
            renderers
                .iter()
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
        "renderer": matched_tv.friendly_name,
        "renderer_id": matched_tv.id
    }))
}
async fn tool_list_playlists<D: DatabaseManager>(
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

async fn tool_create_playlist<D: DatabaseManager + 'static>(
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

async fn tool_delete_playlist<D: DatabaseManager + 'static>(
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

async fn tool_add_to_playlist<D: DatabaseManager + 'static>(
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

async fn tool_remove_from_playlist<D: DatabaseManager + 'static>(
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

async fn tool_get_playlist_tracks<D: DatabaseManager + 'static>(
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
                MAX_MCP_PAGE_SIZE,
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

pub async fn cached_renderers<D: DatabaseManager>(
    state: &AppState<D>,
) -> Result<Vec<tv_control::DiscoveredTv>, String> {
    state
        .discovered_tvs
        .get_or_refresh()
        .await
        .map_err(|e| format!("TV discovery error: {}", e))
}

async fn playlist_file_locations<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    playlist_id: i64,
) -> Result<Vec<FileLocation>, String> {
    state
        .database
        .clone()
        .read(move |session| {
            let mut tracks = Vec::new();
            session.visit_files(
                &MediaFileQuery::Playlist(playlist_id),
                0,
                MAX_MCP_PAGE_SIZE,
                |file| {
                    if let Some(location) = file.to_file_location() {
                        tracks.push(location);
                    }
                    Ok(())
                },
            )?;
            Ok(tracks)
        })
        .await
        .map_err(|error| format!("Database error: {error}"))
}

pub async fn cast_playlist_helper<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    playlist_id: i64,
    renderer_id: &str,
    track_index: usize,
) -> Result<serde_json::Value, String> {
    // Get playlist tracks
    let tracks = playlist_file_locations(state, playlist_id).await?;

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
    let file_id = selected_track.id;

    let renderers = cached_renderers(state).await?;

    let matched_tv = renderers
        .iter()
        .find(|renderer| renderer.id == renderer_id)
        .ok_or(format!(
            "No renderer found with ID '{}'. Available renderers: {}",
            renderer_id,
            renderers
                .iter()
                .map(|tv| tv.friendly_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))?;

    // Build the media URL
    let server_ip = state.get_server_ip();
    let port = state.current_config().server.port;
    let media_url = format!("http://{}:{}/media/{}", server_ip, port, file_id);

    let title = selected_track
        .title
        .as_deref()
        .unwrap_or(&selected_track.filename);

    // Cast selected track
    tv_control::cast_media(
        &matched_tv.control_url,
        &media_url,
        title,
        &selected_track.mime_type,
    )
    .await
    .map_err(|e| format!("Cast error: {}", e))?;

    // Register active cast in global state
    {
        let mut casts = state.active_casts.lock().await;
        casts.insert_labeled(
            matched_tv.id.clone(),
            matched_tv.friendly_name.clone(),
            selected_track.filename.clone(),
        );
    }

    // Cancel existing monitor for this TV if any
    {
        let mut monitors = state.active_monitors.lock().await;
        if let Some((_, cancellation)) = monitors.remove(&matched_tv.control_url) {
            cancellation.cancel();
        }
    }

    // Queue the next track if available (DLNA automatic transitioning)
    let mut queued_file = None;
    if track_index + 1 < tracks.len() {
        let next_track = &tracks[track_index + 1];
        {
            let next_media_url = format!("http://{}:{}/media/{}", server_ip, port, next_track.id);
            let next_title = next_track.title.as_deref().unwrap_or(&next_track.filename);

            // Queue on the TV and log/ignore failures on non-compliant devices
            match tv_control::set_next_media(
                &matched_tv.control_url,
                &next_media_url,
                next_title,
                &next_track.mime_type,
            )
            .await
            {
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
    let monitor_id = Uuid::new_v4();
    let monitor_cancellation = state.cancellation.child_token();
    {
        let mut monitors = state.active_monitors.lock().await;
        if monitors.len() >= crate::runtime_state::ACTIVE_CAST_MAX_ENTRIES
            && !monitors.contains_key(&matched_tv.control_url)
        {
            if let Some(oldest_key) = monitors.keys().next().cloned() {
                if let Some((_, oldest)) = monitors.remove(&oldest_key) {
                    oldest.cancel();
                }
            }
        }
        monitors.insert(
            matched_tv.control_url.clone(),
            (monitor_id, monitor_cancellation.clone()),
        );
    }

    let state_clone = state.clone();
    let control_url_clone = matched_tv.control_url.clone();
    let server_ip_clone = server_ip.clone();
    let matched_tv_friendly_name = matched_tv.friendly_name.clone();
    let matched_renderer_id = matched_tv.id.clone();

    state.background_tasks.spawn(async move {
        let mut current_idx = track_index;
        let mut consecutive_stopped = 0;

        'monitor: loop {
            // Check cancellation or sleep 4s
            tokio::select! {
                _ = monitor_cancellation.cancelled() => {
                    debug!("Queue monitor cancelled for TV: {}", control_url_clone);
                    break;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(4)) => {}
            }

            // Poll TV current playing track URI and transport state
            let current_uri = match tokio::select! {
                _ = monitor_cancellation.cancelled() => break,
                result = tv_control::get_position_info(&control_url_clone) => result,
            } {
                Ok(uri) => uri,
                Err(e) => {
                    debug!("Queue monitor failed to get position info: {}", e);
                    continue;
                }
            };

            let transport_state = match tokio::select! {
                _ = monitor_cancellation.cancelled() => break,
                result = tv_control::get_transport_state(&control_url_clone) => result,
            } {
                Ok(st) => st,
                Err(_) => "STOPPED".to_string(),
            };

            // Fetch playlist tracks from DB to get the latest list
            let latest_tracks = match tokio::select! {
                _ = monitor_cancellation.cancelled() => break,
                result = playlist_file_locations(&state_clone, playlist_id) => result,
            } {
                Ok(t) => t,
                Err(_) => break,
            };

            if latest_tracks.is_empty() {
                break;
            }

            // If current track URI matches a track URL, check if the TV transitioned
            let mut matched_any = false;
            for (idx, track) in latest_tracks.iter().enumerate() {
                {
                    let track_media_url = format!("http://{}:{}/media/{}", server_ip_clone, port, track.id);
                    if current_uri == track_media_url {
                        matched_any = true;
                        if idx != current_idx {
                            info!("Queue monitor: TV transitioned to track index {} ({})", idx, track.filename);
                            current_idx = idx;

                            // Update active cast state with new playing file
                            {
                                let mut casts = state_clone.active_casts.lock().await;
                                casts.insert_labeled(
                                    matched_renderer_id.clone(),
                                    matched_tv_friendly_name.clone(),
                                    track.filename.clone(),
                                );
                            }

                            // Queue the next track if available
                            if current_idx + 1 < latest_tracks.len() {
                                let next_track = &latest_tracks[current_idx + 1];
                                {
                                    let next_media_url = format!("http://{}:{}/media/{}", server_ip_clone, port, next_track.id);
                                    let next_title = next_track.title.as_deref().unwrap_or(&next_track.filename);
                                    let queue_result = tokio::select! {
                                        _ = monitor_cancellation.cancelled() => break 'monitor,
                                        result = tv_control::set_next_media(
                                            &control_url_clone,
                                            &next_media_url,
                                            next_title,
                                            &next_track.mime_type,
                                        ) => result,
                                    };
                                    if let Err(e) = queue_result {
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

        let removed_current_monitor = {
            let mut monitors = state_clone.active_monitors.lock().await;
            let is_current = monitors
                .get(&control_url_clone)
                .is_some_and(|(current_id, _)| *current_id == monitor_id);
            if is_current {
                monitors.remove(&control_url_clone);
            }
            is_current
        };
        // A replaced monitor must not clear the newer cast's telemetry.
        if removed_current_monitor {
            let mut casts = state_clone.active_casts.lock().await;
            casts.remove(&matched_renderer_id);
        }
    });

    Ok(serde_json::json!({
        "status": "playing",
        "playlist_id": playlist_id,
        "tracks_count": tracks.len(),
        "current_index": track_index,
        "current_file": selected_track.filename,
        "queued_next_file": queued_file,
        "renderer": matched_tv.friendly_name,
        "renderer_id": matched_tv.id,
        "media_url": media_url
    }))
}

async fn tool_cast_playlist_to_renderer<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let playlist_id = args
        .get("playlist_id")
        .and_then(|v| v.as_i64())
        .ok_or("Missing 'playlist_id' parameter")?;

    let renderer_id = args
        .get("renderer_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'renderer_id' parameter")?;

    let track_index = args
        .get("track_index")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    cast_playlist_helper(state, playlist_id, renderer_id, track_index).await
}

// ──────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────

fn media_file_view_to_json(f: &impl MediaFileView) -> serde_json::Value {
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

fn requested_limit(args: &serde_json::Value, default: usize) -> usize {
    args.get("limit")
        .and_then(|value| value.as_u64())
        .map(|value| value as usize)
        .unwrap_or(default)
        .clamp(1, 500)
}

fn cursor_after_id(args: &serde_json::Value) -> Result<Option<i64>, String> {
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

async fn query_media_page<D: DatabaseManager + 'static>(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_response_success() {
        let resp =
            JsonRpcResponse::success(Some(serde_json::json!(1)), serde_json::json!({"ok": true}));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"ok\":true"));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_json_rpc_response_error() {
        let resp = JsonRpcResponse::error(
            Some(serde_json::json!(2)),
            -32601,
            "Method not found".to_string(),
        );
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
        assert!(tool_names.contains(&"list_renderers"));
        assert!(tool_names.contains(&"cast_media_to_renderer"));
        assert!(tool_names.contains(&"control_renderer"));
        assert!(tool_names.contains(&"list_media"));
        assert!(tool_names.contains(&"list_playlists"));
        assert!(tool_names.contains(&"create_playlist"));
        assert!(tool_names.contains(&"delete_playlist"));
        assert!(tool_names.contains(&"add_to_playlist"));
        assert!(tool_names.contains(&"remove_from_playlist"));
        assert!(tool_names.contains(&"get_playlist_tracks"));
        assert!(tool_names.contains(&"cast_playlist_to_renderer"));
        assert_eq!(tool_names.len(), 15);
    }
}

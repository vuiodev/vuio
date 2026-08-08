use super::tools::*;
use super::*;

pub(super) async fn handle_method<D: DatabaseManager + 'static>(
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

pub(super) fn handle_initialize(request: &JsonRpcRequest) -> JsonRpcResponse {
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

pub(super) fn handle_tools_list(request: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(request.id.clone(), get_tools_list())
}

pub(super) async fn handle_tools_call<D: DatabaseManager + 'static>(
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

use super::tools::*;
use super::*;

/// Route one JSON-RPC method.
///
/// `None` means the method is not implemented, which the transport turns into a
/// `404` with `-32601`. The status matters: it is what lets a client tell an
/// unimplemented RPC apart from a proxy that never reached this server.
///
/// The two eras share every method that does real work. They differ only at the
/// edges: `initialize` and `ping` exist for older clients and were removed by
/// `2026-07-28`, and `server/discover` is the reverse.
pub(super) async fn handle_method<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    request: &JsonRpcRequest,
    era: Era,
) -> Option<JsonRpcResponse> {
    Some(match (request.method.as_str(), era) {
        ("server/discover", Era::Modern) => handle_discover(request),
        ("initialize", Era::Legacy) => handle_initialize(request),
        // Removed by `2026-07-28`; a client old enough to send it is old enough
        // to expect an answer.
        ("ping", Era::Legacy) => {
            JsonRpcResponse::success(era, request.id.clone(), serde_json::json!({}))
        }
        ("tools/list", _) => handle_tools_list(state, request, era),
        ("tools/call", _) => handle_tools_call(state, request, era).await,
        _ => return None,
    })
}

/// The mandatory identity RPC of `2026-07-28`.
///
/// It replaces `initialize`: a client may call it to pick a version up front,
/// or skip it entirely and let a version mismatch come back as an error on the
/// request it actually wanted to make.
pub(super) fn handle_discover(request: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(
        Era::Modern,
        request.id.clone(),
        serde_json::json!({
            "protocolVersions": supported_versions(),
            "capabilities": {
                // No `listChanged`: the catalog is fixed for the life of the
                // process, so there is nothing to notify about and no
                // `subscriptions/listen` stream to notify on.
                "tools": {}
            },
            "serverInfo": server_info(),
        }),
    )
}

/// The handshake of the revisions before `2026-07-28`.
pub(super) fn handle_initialize(request: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(
        Era::Legacy,
        request.id.clone(),
        serde_json::json!({
            "protocolVersion": negotiated_legacy_version(request.requested_protocol_version()),
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": server_info(),
        }),
    )
}

pub(super) fn handle_tools_list<D: DatabaseManager>(
    state: &AppState<D>,
    request: &JsonRpcRequest,
    era: Era,
) -> JsonRpcResponse {
    let mut result = get_tools_list(state.current_config().mcp.read_only);
    // `CacheableResult` arrived with `2026-07-28`. Older clients validate the
    // result against a schema that predates it, so the hints go only to callers
    // that asked in the new dialect.
    if era == Era::Modern {
        if let Some(object) = result.as_object_mut() {
            object.insert("ttlMs".to_owned(), serde_json::json!(TOOLS_LIST_TTL_MS));
            // Private, not public: which tools exist depends on this server's
            // cargo features and on `[mcp].read_only`, so a shared cache must
            // not hand one server's catalog to a client talking to another.
            object.insert("cacheScope".to_owned(), serde_json::json!("private"));
        }
    }
    JsonRpcResponse::success(era, request.id.clone(), result)
}

pub(super) async fn handle_tools_call<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    request: &JsonRpcRequest,
    era: Era,
) -> JsonRpcResponse {
    let params = match &request.params {
        Some(params) => params,
        None => {
            return JsonRpcResponse::error(
                request.id.clone(),
                INVALID_PARAMS,
                "Missing params".to_string(),
            );
        }
    };

    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    // An unknown tool is a malformed request, not a tool failure: the model
    // cannot fix it by adjusting arguments, so it is a protocol error rather
    // than an `isError` result.
    if !tool_is_available(tool_name, state.current_config().mcp.read_only) {
        return JsonRpcResponse::error(
            request.id.clone(),
            INVALID_PARAMS,
            format!("Unknown tool: {tool_name}"),
        );
    }

    let result = dispatch_tool(state, tool_name, &arguments).await;

    match result {
        Ok(content) => JsonRpcResponse::success(
            era,
            request.id.clone(),
            serde_json::json!({
                // The same value twice: `structuredContent` is what the client
                // validates against the tool's `outputSchema`, and the text
                // block is the backwards-compatible mirror the specification
                // asks servers to include alongside it.
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&content).unwrap_or_default()
                }],
                "structuredContent": content,
                "isError": false
            }),
        ),
        // A tool failure is a *successful* JSON-RPC call with `isError` set, so
        // the model sees the message and can correct itself.
        Err(e) => JsonRpcResponse::success(
            era,
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

async fn dispatch_tool<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    tool_name: &str,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    match tool_name {
        "search_media" => tool_search_media(state, arguments).await,
        "browse_folder" => tool_browse_folder(state, arguments).await,
        "list_library_roots" => tool_list_library_roots(state).await,
        "get_media_info" => tool_get_media_info(state, arguments).await,
        "get_server_stats" => tool_get_server_stats(state).await,
        "list_media" => tool_list_media(state, arguments).await,
        "list_music_categories" => tool_list_music_categories(state, arguments).await,
        "find_music" => tool_find_music(state, arguments).await,
        #[cfg(feature = "casting")]
        "list_renderers" => tool_list_renderers(state).await,
        #[cfg(feature = "casting")]
        "get_playback_status" => tool_get_playback_status(state, arguments).await,
        #[cfg(feature = "casting")]
        "cast_media_to_renderer" => tool_cast_media_to_renderer(state, arguments).await,
        #[cfg(feature = "casting")]
        "cast_folder_to_renderer" => tool_cast_folder_to_renderer(state, arguments).await,
        #[cfg(feature = "casting")]
        "control_renderer" => tool_control_renderer(state, arguments).await,
        "list_playlists" => tool_list_playlists(state).await,
        "create_playlist" => tool_create_playlist(state, arguments).await,
        "delete_playlist" => tool_delete_playlist(state, arguments).await,
        "add_to_playlist" => tool_add_to_playlist(state, arguments).await,
        "remove_from_playlist" => tool_remove_from_playlist(state, arguments).await,
        "reorder_playlist" => tool_reorder_playlist(state, arguments).await,
        "get_playlist_tracks" => tool_get_playlist_tracks(state, arguments).await,
        #[cfg(feature = "casting")]
        "cast_playlist_to_renderer" => tool_cast_playlist_to_renderer(state, arguments).await,
        // Unreachable: `tool_is_available` has already rejected anything the
        // catalog does not advertise. Kept so the match stays total.
        _ => Err(format!("Unknown tool: {tool_name}")),
    }
}

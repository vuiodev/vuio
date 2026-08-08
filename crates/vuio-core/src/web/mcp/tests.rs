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

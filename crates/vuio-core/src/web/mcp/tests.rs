use super::protocol::SERVER_NAME;
use super::*;

fn request(method: &str) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: method.to_string(),
        id: Some(serde_json::json!(1)),
        params: None,
    }
}

fn tool_names(read_only: bool) -> Vec<String> {
    get_tools_list(read_only)["tools"]
        .as_array()
        .expect("a tools array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("a name").to_owned())
        .collect()
}

#[test]
fn a_success_response_carries_no_error() {
    let response =
        JsonRpcResponse::success(
            Era::Modern,
            Some(serde_json::json!(1)),
            serde_json::json!({"ok": true}),
        );
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"jsonrpc\":\"2.0\""));
    assert!(json.contains("\"ok\":true"));
    assert!(!json.contains("\"error\""));
}

#[test]
fn an_error_response_carries_no_result() {
    let response = JsonRpcResponse::error(
        Some(serde_json::json!(2)),
        METHOD_NOT_FOUND,
        "Method not found".to_string(),
    );
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("-32601"));
    assert!(json.contains("Method not found"));
    assert!(!json.contains("\"result\""));
}

/// `server/discover` replaced `initialize` in `2026-07-28`. It has to advertise
/// the versions this server speaks, because a client that calls it is choosing
/// a version from the answer.
#[test]
fn discover_advertises_the_protocol_version_and_identity() {
    let result = handle_discover(&request("server/discover"))
        .result
        .expect("a result");
    assert_eq!(result["protocolVersions"][0], PROTOCOL_VERSION);
    assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
    assert!(result["capabilities"]["tools"].is_object());
    assert_eq!(result["resultType"], "complete");
}

#[test]
fn every_tool_carries_the_metadata_a_client_needs() {
    let tools = get_tools_list(false);
    let tools = tools["tools"].as_array().expect("a tools array");
    assert!(!tools.is_empty());
    for tool in tools {
        let name = tool["name"].as_str().expect("a name");
        assert!(
            tool["title"].is_string(),
            "{name} is missing a display title"
        );
        assert!(
            tool["description"].as_str().is_some_and(|d| d.len() > 40),
            "{name} needs a description a model can act on"
        );
        assert_eq!(
            tool["inputSchema"]["type"], "object",
            "{name} must take an object"
        );
        assert!(
            tool["outputSchema"].is_object(),
            "{name} is missing an outputSchema"
        );
        for hint in [
            "readOnlyHint",
            "destructiveHint",
            "idempotentHint",
            "openWorldHint",
        ] {
            assert!(
                tool["annotations"][hint].is_boolean(),
                "{name} is missing {hint}"
            );
        }
    }
}

#[test]
fn the_catalog_covers_the_whole_surface() {
    let names = tool_names(false);
    for expected in [
        "get_server_stats",
        "list_library_roots",
        "search_media",
        "list_media",
        "browse_folder",
        "get_media_info",
        "list_music_categories",
        "find_music",
        "list_playlists",
        "get_playlist_tracks",
        "create_playlist",
        "add_to_playlist",
        "reorder_playlist",
        "remove_from_playlist",
        "delete_playlist",
    ] {
        assert!(names.contains(&expected.to_owned()), "missing {expected}");
    }
    #[cfg(feature = "casting")]
    for expected in [
        "list_renderers",
        "get_playback_status",
        "cast_media_to_renderer",
        "cast_playlist_to_renderer",
        "cast_folder_to_renderer",
        "control_renderer",
    ] {
        assert!(names.contains(&expected.to_owned()), "missing {expected}");
    }
}

/// A tool that cannot work in this build must not be advertised: an agent has
/// no way to discover that from the failure it would otherwise get.
#[cfg(not(feature = "casting"))]
#[test]
fn casting_tools_vanish_without_a_cast_provider() {
    let names = tool_names(false);
    for absent in [
        "list_renderers",
        "cast_media_to_renderer",
        "control_renderer",
    ] {
        assert!(!names.contains(&absent.to_owned()), "{absent} leaked");
    }
}

#[test]
fn read_only_hides_everything_that_changes_anything() {
    let names = tool_names(true);
    assert!(names.contains(&"search_media".to_owned()));
    assert!(names.contains(&"list_playlists".to_owned()));
    for absent in [
        "create_playlist",
        "delete_playlist",
        "add_to_playlist",
        "reorder_playlist",
        "remove_from_playlist",
    ] {
        assert!(!names.contains(&absent.to_owned()), "{absent} leaked");
    }
    #[cfg(feature = "casting")]
    for absent in ["cast_media_to_renderer", "control_renderer"] {
        assert!(!names.contains(&absent.to_owned()), "{absent} leaked");
    }
}

/// Hiding a tool from `tools/list` is not enough on its own: an agent that
/// learned the name from another server would otherwise still reach the handler.
#[test]
fn read_only_also_refuses_the_hidden_tools_by_name() {
    assert!(tool_is_available("delete_playlist", false));
    assert!(!tool_is_available("delete_playlist", true));
    assert!(tool_is_available("search_media", true));
    assert!(!tool_is_available("no_such_tool", false));
}

/// The list is cached by clients and fed to a model, so its order has to be the
/// same on every call for the underlying set of tools.
#[test]
fn the_tool_order_is_deterministic() {
    assert_eq!(tool_names(false), tool_names(false));
}

/// A tool taking nothing must say "only an empty object", not "any object" —
/// the latter invites a model to invent arguments.
#[test]
fn parameterless_tools_reject_extra_properties() {
    let tools = get_tools_list(false);
    let stats = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "get_server_stats")
        .expect("get_server_stats");
    assert_eq!(stats["inputSchema"]["additionalProperties"], false);
}

/// `mcp/reference.json` is the tool catalog as JSON, for anyone building a
/// client without running the server.
///
/// It used to be maintained by hand, and drifted: it documented `list_tvs`,
/// `cast_media_to_tv` and `control_tv` long after those were renamed. Generating
/// it and checking it here means the next rename cannot leave it behind.
///
/// Run with `VUIO_UPDATE_MCP_REFERENCE=1` to rewrite it.
#[test]
#[cfg_attr(
    not(feature = "casting"),
    ignore = "the reference documents the full-featured build"
)]
fn the_published_tool_reference_matches_the_catalog() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../mcp/reference.json");

    let reference = serde_json::json!({
        "protocol": format!("Model Context Protocol (MCP) {PROTOCOL_VERSION}"),
        "transport": "Streamable HTTP — a single POST /mcp on the server's main port",
        "endpoint": "POST /mcp",
        "notes": [
            "Every request carries its version in `_meta` as \
             `io.modelcontextprotocol/protocolVersion`, mirrored in the \
             `MCP-Protocol-Version` header.",
            "`Mcp-Method` must equal the body's `method`; for `tools/call`, \
             `Mcp-Name` must equal `params.name`.",
            format!(
                "Clients that open with an `initialize` handshake are answered too, \
                 for protocol versions {}.",
                LEGACY_PROTOCOL_VERSIONS.join(", ")
            ),
            "Set `Authorization: Bearer <admin token>` when the server requires it.",
            "Generated from the tool catalog — edit `catalog.rs`, not this file."
        ],
        "tools": get_tools_list(false)["tools"],
    });
    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(&reference).expect("the catalog serialises")
    );

    if std::env::var("VUIO_UPDATE_MCP_REFERENCE").is_ok() {
        std::fs::write(&path, &rendered).expect("could not write the reference");
        return;
    }

    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(
        committed,
        rendered,
        "mcp/reference.json is out of date. Regenerate it:\n    \
         VUIO_UPDATE_MCP_REFERENCE=1 cargo test --all-features \
         the_published_tool_reference_matches_the_catalog"
    );
}

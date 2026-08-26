//! End-to-end tests for the MCP endpoint.
//!
//! Everything here goes through the real router, because the transport is most
//! of what changed: header validation, version rejection and Origin checking
//! all happen before a handler sees anything, and none of them can be exercised
//! by calling the dispatcher directly.

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::{tempdir, TempDir};
use tower::ServiceExt;

use vuio_core::config::AppConfig;
use vuio_core::database::sqlite::SqliteDatabase;
use vuio_core::database::{DatabaseManager, MediaFile, MediaRepository};
use vuio_core::platform::filesystem::create_platform_filesystem_manager;
use vuio_core::platform::PlatformInfo;
use vuio_core::state::AppState;
use vuio_core::web::diagnostics::WebHandlerMetrics;
use vuio_core::web::{create_router, Surface};

const PROTOCOL_VERSION: &str = "2026-07-28";
const TEST_TOKEN: &str = "Bearer test-management-token-which-is-long-enough";

fn test_peer() -> SocketAddr {
    "127.0.0.1:43123".parse().unwrap()
}

/// A well-formed MCP POST: the mirrored headers are derived from the body, the
/// way a conforming client derives them.
fn mcp_request(body: serde_json::Value) -> Request<Body> {
    let method = body["method"].as_str().unwrap_or_default().to_owned();
    let mut builder = Request::post("/mcp")
        .extension(ConnectInfo(test_peer()))
        .header("authorization", TEST_TOKEN)
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", PROTOCOL_VERSION)
        .header("mcp-method", &method);
    if method == "tools/call" {
        if let Some(name) = body["params"]["name"].as_str() {
            builder = builder.header("mcp-name", name);
        }
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

fn call(id: i64, tool: &str, arguments: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "_meta": { "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION },
            "name": tool,
            "arguments": arguments
        }
    })
}

fn plain(id: i64, method: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": {
            "_meta": { "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION }
        }
    })
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), 8 * 1024 * 1024)
        .await
        .expect("a body");
    serde_json::from_slice(&bytes).expect("JSON")
}

/// The `structuredContent` of a successful tool call.
fn tool_result(response: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(
        response["result"]["isError"], false,
        "tool failed: {}",
        response["result"]["content"][0]["text"]
    );
    &response["result"]["structuredContent"]
}

async fn make_test_state() -> (TempDir, AppState) {
    let temp = tempdir().unwrap();
    let database = Arc::new(
        SqliteDatabase::new(temp.path().join("mcp-tests.db"))
            .await
            .unwrap(),
    );
    database.initialize().await.unwrap();
    let config = Arc::new(AppConfig::default());

    let state = AppState {
        media_directories: Arc::new(tokio::sync::RwLock::new(config.media.directories.clone())),
        unavailable_roots: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
        config: config.clone(),
        config_source: std::sync::Arc::new(vuio_core::state::ConfigSource::default()),
        http_binding: std::sync::Arc::new(vuio_core::state::HttpBinding::new(8080)),
        live_config: Arc::new(vuio_core::state::LiveConfig::new(config)),
        database,
        auth: Arc::new(vuio_core::web::auth::AuthState::testing()),
        platform_info: Arc::new(PlatformInfo::detect().await.unwrap()),
        filesystem_manager: Arc::from(create_platform_filesystem_manager()),
        content_update_id: Arc::new(std::sync::atomic::AtomicU32::new(1)),
        web_metrics: Arc::new(WebHandlerMetrics::new()),
        runtime_diagnostics: Arc::new(
            vuio_core::platform::diagnostics::SystemDiagnosticsSampler::new(),
        ),
        lifecycle_stats: Arc::new(vuio_core::lifecycle::ApplicationStats::new()),
        bookmarks: Arc::new(tokio::sync::Mutex::new(
            vuio_core::runtime_state::BookmarkRegistry::new(
                vuio_core::runtime_state::BOOKMARK_MAX_ENTRIES,
            ),
        )),
        log_file_path: temp.path().join("vuio.log"),
        browse_cache: Arc::new(tokio::sync::Mutex::new(
            vuio_core::runtime_state::BrowseResponseCache::new(),
        )),
        active_monitors: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_casts: Arc::new(tokio::sync::Mutex::new(
            vuio_core::runtime_state::ActiveCastRegistry::new(),
        )),
        #[cfg(feature = "mediainfo")]
        mediainfo_job: Arc::new(tokio::sync::Mutex::new(Default::default())),
        discovered_tvs: Arc::new(vuio_core::runtime_state::RendererCache::new()),
        upnp_subscriptions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        radio: Arc::new(Default::default()),
        #[cfg(feature = "transcode")]
        transcode: Arc::new(Default::default()),
        cancellation: tokio_util::sync::CancellationToken::new(),
        background_tasks: tokio_util::task::TaskTracker::new(),
    };
    (temp, state)
}

fn sample_audio(path: &str, title: &str, artist: &str) -> MediaFile {
    MediaFile {
        id: None,
        path: PathBuf::from(path),
        filename: PathBuf::from(path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        size: 5000,
        modified: std::time::SystemTime::now(),
        mime_type: "audio/mpeg".to_string(),
        duration: None,
        title: Some(title.to_string()),
        artist: Some(artist.to_string()),
        album: Some("Led Zeppelin IV".to_string()),
        genre: Some("Rock".to_string()),
        track_number: Some(4),
        year: Some(1971),
        album_artist: None,
        tags: Default::default(),
        stream: Default::default(),
        extra_tags: Vec::new(),
        tags_version: 0,
        subtitle_available: false,
        created_at: std::time::SystemTime::now(),
        updated_at: std::time::SystemTime::now(),
    }
}

#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (mcp integration harness)"
)]
async fn content_publication_and_cache_only_invalidation_have_distinct_revisions() {
    use axum::body::Bytes;
    use std::sync::atomic::Ordering;
    use vuio_core::state::SoapCacheKey;
    use vuio_core::web::client::DlnaClientProfile;

    let (_temp, state) = make_test_state().await;
    let epoch = state.browse_cache.lock().await.epoch();
    let key = SoapCacheKey {
        object_id: "audio".to_string(),
        starting_index: 0,
        requested_count: 10,
        client_profile: DlnaClientProfile::Standard,
        content_update_id: 1,
        browse_epoch: epoch,
    };
    state
        .browse_cache
        .lock()
        .await
        .insert(key, Bytes::from_static(b"cached"));

    vuio_core::web::eventing::publish_content_change(&state).await;
    assert_eq!(state.content_update_id.load(Ordering::SeqCst), 2);
    assert!(state.browse_cache.lock().await.generation().is_none());

    let revision = state.content_update_id.load(Ordering::SeqCst);
    vuio_core::web::eventing::invalidate_browse_responses(&state).await;
    assert_eq!(state.content_update_id.load(Ordering::SeqCst), revision);

    state.background_tasks.close();
    state.background_tasks.wait().await;
}

/// The whole agent journey in one pass: discover the server, list its tools,
/// search, then build and read back a playlist.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (mcp integration harness)"
)]
async fn an_agent_can_discover_search_and_build_a_playlist() {
    let (_temp, state) = make_test_state().await;
    state
        .database
        .store_media_file(&sample_audio(
            "/media/music/song.mp3",
            "Stairway to Heaven",
            "Led Zeppelin",
        ))
        .await
        .unwrap();
    let router = create_router(state, Surface::Primary);

    // server/discover replaces the initialize handshake.
    let discover = body_json(
        router
            .clone()
            .oneshot(mcp_request(plain(1, "server/discover")))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(discover["id"], 1);
    assert_eq!(discover["result"]["protocolVersions"][0], PROTOCOL_VERSION);
    assert_eq!(discover["result"]["resultType"], "complete");
    assert_eq!(
        discover["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "vuio-media-server"
    );

    // tools/list carries the caching hints the current revision requires.
    let list = body_json(
        router
            .clone()
            .oneshot(mcp_request(plain(2, "tools/list")))
            .await
            .unwrap(),
    )
    .await;
    let tools = list["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|t| t["name"] == "search_media"));
    assert!(list["result"]["ttlMs"].as_u64().unwrap() > 0);
    assert_eq!(list["result"]["cacheScope"], "private");

    // A tool result carries structured content, not just a text blob.
    let search = body_json(
        router
            .clone()
            .oneshot(mcp_request(call(
                3,
                "search_media",
                serde_json::json!({ "query": "stairway" }),
            )))
            .await
            .unwrap(),
    )
    .await;
    let data = tool_result(&search);
    assert_eq!(data["total_matches"], 1);
    assert_eq!(data["files"][0]["title"], "Stairway to Heaven");
    assert_eq!(data["files"][0]["artist"], "Led Zeppelin");
    // The URL is the point: without it an agent can describe a file but cannot
    // give anyone a way to play it. The host depends on the machine running the
    // test, so only the parts this server decides are asserted.
    let stream_url = data["files"][0]["stream_url"].as_str().unwrap();
    assert!(stream_url.starts_with("http://"), "{stream_url}");
    assert!(stream_url.ends_with(":8080/media/1.mp3"), "{stream_url}");
    // The text block mirrors it, for clients that read only content.
    let mirrored: serde_json::Value =
        serde_json::from_str(search["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(mirrored, *data);

    let listed = body_json(
        router
            .clone()
            .oneshot(mcp_request(call(
                4,
                "list_media",
                serde_json::json!({ "category": "audio", "limit": 10 }),
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(tool_result(&listed)["total_files"], 1);

    let created = body_json(
        router
            .clone()
            .oneshot(mcp_request(call(
                5,
                "create_playlist",
                serde_json::json!({ "name": "My Favorites", "description": "Best tracks" }),
            )))
            .await
            .unwrap(),
    )
    .await;
    let playlist_id = tool_result(&created)["playlist_id"].as_i64().unwrap();
    assert!(playlist_id > 0);

    let added = body_json(
        router
            .clone()
            .oneshot(mcp_request(call(
                6,
                "add_to_playlist",
                serde_json::json!({ "playlist_id": playlist_id, "media_file_ids": [1] }),
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(tool_result(&added)["tracks_added"], 1);

    let tracks = body_json(
        router
            .oneshot(mcp_request(call(
                7,
                "get_playlist_tracks",
                serde_json::json!({ "playlist_id": playlist_id }),
            )))
            .await
            .unwrap(),
    )
    .await;
    let tracks = tool_result(&tracks);
    assert_eq!(tracks["tracks_count"], 1);
    assert_eq!(tracks["tracks"][0]["filename"], "song.mp3");
}

/// A load balancer routing on the header while this server executes the body is
/// exactly the split the mirrored-header check exists to prevent.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (mcp integration harness)"
)]
async fn mirrored_headers_must_agree_with_the_body() {
    let (_temp, state) = make_test_state().await;
    let router = create_router(state, Surface::Primary);

    // Mcp-Method says one thing, the body another.
    let mismatch = router
        .clone()
        .oneshot(
            Request::post("/mcp")
                .extension(ConnectInfo(test_peer()))
                .header("authorization", TEST_TOKEN)
                .header("content-type", "application/json")
                .header("mcp-protocol-version", PROTOCOL_VERSION)
                .header("mcp-method", "tools/list")
                .body(Body::from(plain(1, "server/discover").to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mismatch.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(mismatch).await["error"]["code"], -32020);

    // tools/call without the Mcp-Name header.
    let missing_name = router
        .clone()
        .oneshot(
            Request::post("/mcp")
                .extension(ConnectInfo(test_peer()))
                .header("authorization", TEST_TOKEN)
                .header("content-type", "application/json")
                .header("mcp-protocol-version", PROTOCOL_VERSION)
                .header("mcp-method", "tools/call")
                .body(Body::from(
                    call(2, "search_media", serde_json::json!({"query": "x"})).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_name.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(missing_name).await["error"]["code"], -32020);

    // No MCP-Protocol-Version header at all.
    let no_version = router
        .oneshot(
            Request::post("/mcp")
                .extension(ConnectInfo(test_peer()))
                .header("authorization", TEST_TOKEN)
                .header("content-type", "application/json")
                .header("mcp-method", "tools/list")
                .body(Body::from(plain(3, "tools/list").to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(no_version.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body_json(no_version).await["error"]["code"], -32020);
}

/// A client asking for a revision this server has never implemented has to be
/// told what it does speak, not merely refused.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (mcp integration harness)"
)]
async fn an_unsupported_version_is_answered_with_the_supported_list() {
    let (_temp, state) = make_test_state().await;
    let router = create_router(state, Surface::Primary);

    let response = router
        .oneshot(
            Request::post("/mcp")
                .extension(ConnectInfo(test_peer()))
                .header("authorization", TEST_TOKEN)
                .header("content-type", "application/json")
                .header("mcp-protocol-version", "2024-11-05")
                .header("mcp-method", "tools/list")
                .body(Body::from(
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "tools/list",
                        "params": {
                            "_meta": {
                                "io.modelcontextprotocol/protocolVersion": "2024-11-05"
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], -32022);
    assert_eq!(body["error"]["data"]["supported"][0], PROTOCOL_VERSION);
    let supported = body["error"]["data"]["supported"].as_array().unwrap();
    assert!(supported.iter().any(|v| v == "2025-11-25"));
}

/// The client this integration exists for. Claude Code 2.1.x opens with an
/// `initialize` at `2025-11-25`, sends no mirrored headers, and validates
/// results against a schema that predates `resultType` — so the whole handshake
/// is reproduced here byte for byte as it arrives on the wire.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (mcp integration harness)"
)]
async fn a_client_on_the_initialize_era_completes_the_handshake() {
    let (_temp, state) = make_test_state().await;
    state
        .database
        .store_media_file(&sample_audio(
            "/media/music/song.mp3",
            "Stairway to Heaven",
            "Led Zeppelin",
        ))
        .await
        .unwrap();
    let router = create_router(state, Surface::Primary);

    // 1. initialize — no MCP-Protocol-Version header at all, because the
    //    revision that introduced it is newer than the one being asked for.
    let init = router
        .clone()
        .oneshot(
            Request::post("/mcp")
                .extension(ConnectInfo(test_peer()))
                .header("authorization", TEST_TOKEN)
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(Body::from(
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": 0,
                        "method": "initialize",
                        "params": {
                            "protocolVersion": "2025-11-25",
                            "capabilities": { "roots": { "listChanged": true } },
                            "clientInfo": { "name": "claude-code", "version": "2.1.231" }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(init.status(), StatusCode::OK);
    let init = body_json(init).await;
    assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(init["result"]["serverInfo"]["name"], "vuio-media-server");
    assert!(
        init["result"].get("resultType").is_none(),
        "resultType is newer than this client's schema"
    );

    // 2. notifications/initialized — a notification, so 202 and no body.
    let ack = router
        .clone()
        .oneshot(
            Request::post("/mcp")
                .extension(ConnectInfo(test_peer()))
                .header("authorization", TEST_TOKEN)
                .header("content-type", "application/json")
                .header("mcp-protocol-version", "2025-11-25")
                .body(Body::from(
                    r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ack.status(), StatusCode::ACCEPTED);

    // 3. tools/list — version header only, no Mcp-Method, no _meta.
    let list = router
        .clone()
        .oneshot(
            Request::post("/mcp")
                .extension(ConnectInfo(test_peer()))
                .header("authorization", TEST_TOKEN)
                .header("content-type", "application/json")
                .header("mcp-protocol-version", "2025-11-25")
                .body(Body::from(
                    r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list = body_json(list).await;
    let tools = list["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|t| t["name"] == "search_media"));
    assert!(
        list["result"].get("ttlMs").is_none(),
        "CacheableResult is newer than this client's schema"
    );

    // 4. A tool call, likewise bare.
    let search = router
        .oneshot(
            Request::post("/mcp")
                .extension(ConnectInfo(test_peer()))
                .header("authorization", TEST_TOKEN)
                .header("content-type", "application/json")
                .header("mcp-protocol-version", "2025-11-25")
                .body(Body::from(
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": 2,
                        "method": "tools/call",
                        "params": {
                            "name": "search_media",
                            "arguments": { "query": "stairway" }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(search.status(), StatusCode::OK);
    let search = body_json(search).await;
    assert_eq!(search["result"]["isError"], false);
    let data: serde_json::Value =
        serde_json::from_str(search["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(data["total_matches"], 1);
}

/// `ping` was removed by `2026-07-28` but is part of the older revisions, and a
/// client old enough to send it is old enough to expect an answer.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (mcp integration harness)"
)]
async fn ping_answers_on_the_legacy_era_and_not_on_the_modern_one() {
    let (_temp, state) = make_test_state().await;
    let router = create_router(state, Surface::Primary);

    let legacy = router
        .clone()
        .oneshot(
            Request::post("/mcp")
                .extension(ConnectInfo(test_peer()))
                .header("authorization", TEST_TOKEN)
                .header("content-type", "application/json")
                .header("mcp-protocol-version", "2025-11-25")
                .body(Body::from(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(legacy.status(), StatusCode::OK);

    let modern = router.oneshot(mcp_request(plain(2, "ping"))).await.unwrap();
    assert_eq!(modern.status(), StatusCode::NOT_FOUND);
}

/// An unimplemented RPC must be distinguishable from a proxy's 404, which is
/// what the JSON-RPC error in the body is for.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (mcp integration harness)"
)]
async fn an_unknown_method_is_a_404_with_a_json_rpc_error() {
    let (_temp, state) = make_test_state().await;
    let router = create_router(state, Surface::Primary);

    let response = router
        .oneshot(mcp_request(plain(1, "resources/list")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(body_json(response).await["error"]["code"], -32601);
}

/// The session-based revisions used GET for a standalone stream and DELETE to
/// end a session. Neither exists here, and 405 is how an older client is told.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (mcp integration harness)"
)]
async fn get_and_delete_on_the_endpoint_are_refused() {
    let (_temp, state) = make_test_state().await;
    let router = create_router(state, Surface::Primary);

    for request in [Request::get("/mcp"), Request::delete("/mcp")] {
        let response = router
            .clone()
            .oneshot(
                request
                    .extension(ConnectInfo(test_peer()))
                    .header("authorization", TEST_TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}

/// A browser page on another origin must not be able to drive a media server on
/// the LAN. A client that sends no Origin at all is not a browser and is fine.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (mcp integration harness)"
)]
async fn a_foreign_origin_is_refused_but_an_absent_one_is_not() {
    let (_temp, state) = make_test_state().await;
    let router = create_router(state, Surface::Primary);

    let foreign = router
        .clone()
        .oneshot(
            Request::post("/mcp")
                .extension(ConnectInfo(test_peer()))
                .header("authorization", TEST_TOKEN)
                .header("content-type", "application/json")
                .header("mcp-protocol-version", PROTOCOL_VERSION)
                .header("mcp-method", "tools/list")
                .header("host", "127.0.0.1:8080")
                .header("origin", "http://evil.example")
                .body(Body::from(plain(1, "tools/list").to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);

    let matching = router
        .clone()
        .oneshot(
            Request::post("/mcp")
                .extension(ConnectInfo(test_peer()))
                .header("authorization", TEST_TOKEN)
                .header("content-type", "application/json")
                .header("mcp-protocol-version", PROTOCOL_VERSION)
                .header("mcp-method", "tools/list")
                .header("host", "127.0.0.1:8080")
                .header("origin", "http://127.0.0.1:8080")
                .body(Body::from(plain(2, "tools/list").to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(matching.status(), StatusCode::OK);

    let absent = router
        .oneshot(mcp_request(plain(3, "tools/list")))
        .await
        .unwrap();
    assert_eq!(absent.status(), StatusCode::OK);
}

/// A notification has no id, so there is nothing to answer it with.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (mcp integration harness)"
)]
async fn a_notification_is_accepted_without_a_response() {
    let (_temp, state) = make_test_state().await;
    let router = create_router(state, Surface::Primary);

    let response = router
        .oneshot(
            Request::post("/mcp")
                .extension(ConnectInfo(test_peer()))
                .header("authorization", TEST_TOKEN)
                .header("content-type", "application/json")
                .header("mcp-protocol-version", PROTOCOL_VERSION)
                .header("mcp-method", "notifications/progress")
                .body(Body::from(
                    r#"{"jsonrpc":"2.0","method":"notifications/progress"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

/// Hiding a tool from the catalog is not enough: a name learned elsewhere must
/// not reach the handler either.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (mcp integration harness)"
)]
async fn read_only_mode_refuses_a_mutating_tool_by_name() {
    let (_temp, mut state) = make_test_state().await;
    let mut config = AppConfig::default();
    config.mcp.read_only = true;
    let config = Arc::new(config);
    state.config = config.clone();
    state.live_config = Arc::new(vuio_core::state::LiveConfig::new(config));
    let router = create_router(state, Surface::Primary);

    let list = body_json(
        router
            .clone()
            .oneshot(mcp_request(plain(1, "tools/list")))
            .await
            .unwrap(),
    )
    .await;
    let tools = list["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|t| t["name"] == "search_media"));
    assert!(!tools.iter().any(|t| t["name"] == "create_playlist"));

    let refused = body_json(
        router
            .oneshot(mcp_request(call(
                2,
                "create_playlist",
                serde_json::json!({ "name": "sneaky" }),
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(refused["error"]["code"], -32602);
}

/// A path outside every configured root must be refused, and the message has to
/// say what to call instead — an agent cannot guess the roots.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (mcp integration harness)"
)]
async fn browsing_outside_the_configured_roots_is_refused() {
    let (_temp, state) = make_test_state().await;
    let router = create_router(state, Surface::Primary);

    let response = body_json(
        router
            .oneshot(mcp_request(call(
                1,
                "browse_folder",
                serde_json::json!({ "path": "/etc" }),
            )))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(response["result"]["isError"], true);
    let message = response["result"]["content"][0]["text"].as_str().unwrap();
    assert!(message.contains("list_library_roots"), "{message}");
}

#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (mcp integration harness)"
)]
async fn post_endpoints_enforce_tiered_body_limits() {
    let (_temp, state) = make_test_state().await;
    let router = create_router(state, Surface::Primary);

    let soap_response = router
        .clone()
        .oneshot(
            Request::post("/control/ContentDirectory")
                .header("content-type", "text/xml")
                .body(Body::from(vec![b'x'; 1024 * 1024 + 1]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(soap_response.status(), StatusCode::PAYLOAD_TOO_LARGE);

    for path in ["/mcp", "/api/cast/playlist"] {
        let response = router
            .clone()
            .oneshot(
                Request::post(path)
                    .extension(ConnectInfo(test_peer()))
                    .header("authorization", TEST_TOKEN)
                    .header("content-type", "application/json")
                    .header("mcp-protocol-version", PROTOCOL_VERSION)
                    .header("mcp-method", "tools/list")
                    .body(Body::from(vec![b'x'; 256 * 1024 + 1]))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE, "{path}");
    }

    let small_mcp = router
        .clone()
        .oneshot(mcp_request(plain(1, "tools/list")))
        .await
        .unwrap();
    assert_ne!(small_mcp.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let small_soap = router
        .oneshot(
            Request::post("/control/ConnectionManager")
                .header("content-type", "text/xml")
                .body(Body::from("<u:GetProtocolInfo/>"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(small_soap.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

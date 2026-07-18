use axum::{
    body::to_bytes,
    extract::State,
    http::{header::USER_AGENT, HeaderMap, HeaderValue, StatusCode},
};
use std::sync::Arc;
use tempfile::tempdir;
use vuio::{
    config::{AppConfig, MonitoredDirectoryConfig, ValidationMode},
    database::{redb::RedbDatabase, DatabaseManager, MediaFile, MediaRepository},
    lifecycle::ApplicationStats,
    platform::{
        diagnostics::SystemDiagnosticsSampler, filesystem::create_platform_filesystem_manager,
        PlatformInfo,
    },
    runtime_state::{
        ActiveCastRegistry, BookmarkRegistry, BrowseResponseCache, RendererCache,
        BOOKMARK_MAX_ENTRIES,
    },
    state::AppState,
    web::{diagnostics::WebHandlerMetrics, soap::content_directory_control},
};

fn browse_request(object_id: &str, starting_index: u32, requested_count: u32) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
  <s:Body>
    <u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
      <ObjectID>{object_id}</ObjectID>
      <BrowseFlag>BrowseDirectChildren</BrowseFlag>
      <Filter>*</Filter>
      <StartingIndex>{starting_index}</StartingIndex>
      <RequestedCount>{requested_count}</RequestedCount>
      <SortCriteria></SortCriteria>
    </u:Browse>
  </s:Body>
</s:Envelope>"#
    )
}

async fn browse(state: AppState, object_id: &str, start: u32, count: u32) -> String {
    let mut headers = HeaderMap::new();
    headers.insert(
        "soapaction",
        HeaderValue::from_static("\"urn:schemas-upnp-org:service:ContentDirectory:1#Browse\""),
    );
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Linux UPnP/1.0 Philips-TV/2.0 DLNADOC/1.50"),
    );
    let response = content_directory_control(
        State(state),
        headers,
        browse_request(object_id, start, count),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    String::from_utf8(
        to_bytes(response.into_body(), 128 * 1024)
            .await
            .expect("read Browse response")
            .to_vec(),
    )
    .expect("Browse response is UTF-8")
}

#[tokio::test]
async fn issue_24_philips_probe_reports_full_total_and_supports_followup_pages() {
    let temp = tempdir().expect("temporary test directory");
    let media_root = temp.path().join("mediatest");
    tokio::fs::create_dir(&media_root)
        .await
        .expect("create media directory");
    let canonical_root = media_root
        .canonicalize()
        .expect("canonical media directory");
    let first_path = canonical_root.join("first.mkv");
    let second_path = canonical_root.join("second.mkv");
    tokio::fs::write(&first_path, b"first")
        .await
        .expect("write first video");
    tokio::fs::write(&second_path, b"second")
        .await
        .expect("write second video");

    let database = Arc::new(
        RedbDatabase::new(temp.path().join("media.redb"))
            .await
            .expect("create database"),
    );
    database.initialize().await.expect("initialize database");
    for path in [&first_path, &second_path] {
        database
            .store_media_file(&MediaFile::new(
                path.to_path_buf(),
                5,
                "video/x-matroska".to_string(),
            ))
            .await
            .expect("index video");
    }

    let monitored_directory = MonitoredDirectoryConfig {
        path: media_root.to_string_lossy().into_owned(),
        recursive: false,
        extensions: Some(vec!["mkv".to_string()]),
        exclude_patterns: None,
        validation_mode: ValidationMode::Warn,
    };
    let mut config = AppConfig::default();
    config.server.ip = Some("127.0.0.1".to_string());
    config.media.directories = vec![monitored_directory.clone()];
    let config = Arc::new(config);
    let state = AppState {
        config: config.clone(),
        live_config: Arc::new(vuio::state::LiveConfig::new(config.clone())),
        media_directories: Arc::new(tokio::sync::RwLock::new(vec![monitored_directory])),
        unavailable_roots: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
        database,
        auth: Arc::new(vuio::web::auth::AuthState::testing()),
        platform_info: Arc::new(PlatformInfo::detect().await.expect("detect platform")),
        filesystem_manager: Arc::from(create_platform_filesystem_manager()),
        content_update_id: Arc::new(std::sync::atomic::AtomicU32::new(1)),
        web_metrics: Arc::new(WebHandlerMetrics::new()),
        runtime_diagnostics: Arc::new(SystemDiagnosticsSampler::new()),
        lifecycle_stats: Arc::new(ApplicationStats::new()),
        bookmarks: Arc::new(tokio::sync::Mutex::new(BookmarkRegistry::new(
            BOOKMARK_MAX_ENTRIES,
        ))),
        log_file_path: temp.path().join("vuio.log"),
        browse_cache: Arc::new(tokio::sync::Mutex::new(BrowseResponseCache::new())),
        mcp_clients: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_monitors: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_casts: Arc::new(tokio::sync::Mutex::new(ActiveCastRegistry::new())),
        discovered_tvs: Arc::new(RendererCache::new()),
        upnp_subscriptions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        cancellation: tokio_util::sync::CancellationToken::new(),
        background_tasks: tokio_util::task::TaskTracker::new(),
    };

    let first_page = browse(state.clone(), "video", 0, 1).await;
    assert!(first_page.contains("<NumberReturned>1</NumberReturned>"));
    assert!(first_page.contains("<TotalMatches>2</TotalMatches>"));
    assert_eq!(first_page.matches("&lt;item ").count(), 1);

    let second_page = browse(state.clone(), "video", 1, 1).await;
    assert!(second_page.contains("<NumberReturned>1</NumberReturned>"));
    assert!(second_page.contains("<TotalMatches>2</TotalMatches>"));
    assert_eq!(second_page.matches("&lt;item ").count(), 1);
    assert_ne!(first_page, second_page);
    for filename in ["first.mkv", "second.mkv"] {
        assert!(first_page.contains(filename) || second_page.contains(filename));
    }

    let exhausted_page = browse(state.clone(), "video", 2, 1).await;
    assert!(exhausted_page.contains("<NumberReturned>0</NumberReturned>"));
    assert!(exhausted_page.contains("<TotalMatches>2</TotalMatches>"));

    let root_probe = browse(state, "0", 0, 1).await;
    assert!(root_probe.contains("<NumberReturned>1</NumberReturned>"));
    assert!(root_probe.contains("<TotalMatches>4</TotalMatches>"));
}

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

fn browse_metadata_request(object_id: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
  <s:Body>
    <u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
      <ObjectID>{object_id}</ObjectID>
      <BrowseFlag>BrowseMetadata</BrowseFlag>
      <Filter>*</Filter>
      <StartingIndex>0</StartingIndex>
      <RequestedCount>0</RequestedCount>
      <SortCriteria></SortCriteria>
    </u:Browse>
  </s:Body>
</s:Envelope>"#
    )
}

async fn browse_samsung_metadata(state: AppState, object_id: &str) -> String {
    let mut headers = HeaderMap::new();
    headers.insert(
        "soapaction",
        HeaderValue::from_static("\"urn:schemas-upnp-org:service:ContentDirectory:1#Browse\""),
    );
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("DLNADOC/1.50 SEC_HHP_[TV]UE40D7000/1.0"),
    );
    let response =
        content_directory_control(State(state), headers, browse_metadata_request(object_id)).await;
    assert_eq!(response.status(), StatusCode::OK);
    String::from_utf8(
        to_bytes(response.into_body(), 128 * 1024)
            .await
            .expect("read BrowseMetadata response")
            .to_vec(),
    )
    .expect("BrowseMetadata response is UTF-8")
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
        case_sensitive: None,
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

#[tokio::test]
async fn dlna_browse_returns_naturally_sorted_episodes() {
    let temp = tempdir().expect("temporary test directory");
    let media_root = temp.path().join("shows");
    tokio::fs::create_dir(&media_root)
        .await
        .expect("create media directory");
    let canonical_root = media_root
        .canonicalize()
        .expect("canonical media directory");

    // Create files in deliberate non-alphabetical/non-numerical order
    let files = [
        "Show.s01e10.mkv",
        "Show.s01e02.mkv",
        "Show.s01e01.mkv",
        "Show.s01e32.mkv",
    ];

    let database = Arc::new(
        RedbDatabase::new(temp.path().join("media.redb"))
            .await
            .expect("create database"),
    );
    database.initialize().await.expect("initialize database");

    for fname in &files {
        let fpath = canonical_root.join(fname);
        tokio::fs::write(&fpath, b"test").await.unwrap();
        database
            .store_media_file(&MediaFile::new(fpath, 4, "video/x-matroska".to_string()))
            .await
            .unwrap();
    }

    let monitored_directory = MonitoredDirectoryConfig {
        path: media_root.to_string_lossy().into_owned(),
        recursive: false,
        case_sensitive: None,
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

    let result_xml = browse(state, "video", 0, 10).await;
    let pos_e01 = result_xml.find("Show.s01e01.mkv").expect("found e01");
    let pos_e02 = result_xml.find("Show.s01e02.mkv").expect("found e02");
    let pos_e10 = result_xml.find("Show.s01e10.mkv").expect("found e10");
    let pos_e32 = result_xml.find("Show.s01e32.mkv").expect("found e32");

    assert!(pos_e01 < pos_e02, "e01 must precede e02");
    assert!(pos_e02 < pos_e10, "e02 must precede e10");
    assert!(pos_e10 < pos_e32, "e10 must precede e32");
}

#[tokio::test]
async fn samsung_basicview_object_ids_alias_to_video_root() {
    let temp = tempdir().expect("temporary test directory");
    let media_root = temp.path().join("mediatest");
    tokio::fs::create_dir(&media_root)
        .await
        .expect("create media directory");
    let canonical_root = media_root
        .canonicalize()
        .expect("canonical media directory");
    let video_path = canonical_root.join("movie.mkv");
    tokio::fs::write(&video_path, b"video")
        .await
        .expect("write video");

    let database = Arc::new(
        RedbDatabase::new(temp.path().join("media.redb"))
            .await
            .expect("create database"),
    );
    database.initialize().await.expect("initialize database");
    database
        .store_media_file(&MediaFile::new(
            video_path,
            5,
            "video/x-matroska".to_string(),
        ))
        .await
        .expect("index video");

    let monitored_directory = MonitoredDirectoryConfig {
        path: media_root.to_string_lossy().into_owned(),
        recursive: false,
        case_sensitive: None,
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

    let expected = browse(state.clone(), "video", 0, 10).await;
    assert!(expected.contains("movie.mkv"));
    assert!(expected.contains("&lt;item "));

    // FeatureList id "2" and DCM10 id "V" must return the same video listing,
    // not an empty filesystem browse of path "2" / "V".
    for object_id in ["2", "V"] {
        let aliased = browse(state.clone(), object_id, 0, 10).await;
        assert!(
            aliased.contains("movie.mkv"),
            "ObjectID {object_id} should list video files"
        );
        assert!(
            aliased.contains("&lt;item "),
            "ObjectID {object_id} should return DIDL items"
        );
        assert_eq!(
            aliased.matches("&lt;item ").count(),
            expected.matches("&lt;item ").count()
        );
    }
}

#[tokio::test]
async fn samsung_browse_metadata_reports_child_count() {
    let temp = tempdir().expect("temporary test directory");
    let media_root = temp.path().join("mediatest");
    tokio::fs::create_dir(&media_root)
        .await
        .expect("create media directory");
    let canonical_root = media_root
        .canonicalize()
        .expect("canonical media directory");
    let video_path = canonical_root.join("movie.mkv");
    tokio::fs::write(&video_path, b"video")
        .await
        .expect("write video");

    let database = Arc::new(
        RedbDatabase::new(temp.path().join("media.redb"))
            .await
            .expect("create database"),
    );
    database.initialize().await.expect("initialize database");
    database
        .store_media_file(&MediaFile::new(
            video_path,
            5,
            "video/x-matroska".to_string(),
        ))
        .await
        .expect("index video");

    let monitored_directory = MonitoredDirectoryConfig {
        path: media_root.to_string_lossy().into_owned(),
        recursive: false,
        case_sensitive: None,
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

    let children = browse(state.clone(), "video", 0, 10).await;
    assert!(children.contains("movie.mkv"));

    let root = browse(state.clone(), "0", 0, 10).await;
    assert!(
        root.contains("childCount=&quot;1&quot;") || root.contains("childCount=\"1\""),
        "root containers must advertise childCount for Samsung"
    );

    let metadata = browse_samsung_metadata(state, "video").await;
    assert!(metadata.contains("<NumberReturned>1</NumberReturned>"));
    assert!(metadata.contains("<TotalMatches>1</TotalMatches>"));
    assert!(
        metadata.contains("childCount=&quot;1&quot;") || metadata.contains("childCount=\"1\""),
        "BrowseMetadata must include non-zero childCount, got: {metadata}"
    );
    assert!(
        !metadata.contains("movie.mkv"),
        "BrowseMetadata must return the container, not its children"
    );
}

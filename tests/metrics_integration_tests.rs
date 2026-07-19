use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

use vuio::config::{AppConfig, MonitoredDirectoryConfig, ValidationMode};
use vuio::database::redb::RedbDatabase;
use vuio::database::{DatabaseManager, MediaFile, MediaRepository, PlaylistRepository};
use vuio::platform::filesystem::create_platform_filesystem_manager;
use vuio::platform::PlatformInfo;
use vuio::state::AppState;
use vuio::web::diagnostics::{get_prometheus_metrics, get_web_metrics, WebHandlerMetrics};

#[tokio::test]
async fn test_metrics_endpoints_data() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test_metrics.redb");

    // 1. Initialize DB and insert files of different types
    let db = Arc::new(RedbDatabase::new(db_path).await.unwrap());
    db.initialize().await.unwrap();

    let video_file = MediaFile::new(
        PathBuf::from("/media/video.mp4"),
        1024,
        "video/mp4".to_string(),
    );
    let audio_file = MediaFile::new(
        PathBuf::from("/media/audio.mp3"),
        2048,
        "audio/mpeg".to_string(),
    );
    let image_file = MediaFile::new(
        PathBuf::from("/media/image.jpg"),
        512,
        "image/jpeg".to_string(),
    );

    db.store_media_file(&video_file).await.unwrap();
    db.store_media_file(&audio_file).await.unwrap();
    db.store_media_file(&image_file).await.unwrap();

    // Create a playlist
    let playlist_id = db.create_playlist("Test Playlist", None).await.unwrap();
    assert!(playlist_id > 0);

    // 2. Setup mock AppState
    let private_media_path = temp_dir.path().join("private-media-location");
    tokio::fs::create_dir(&private_media_path).await.unwrap();
    let mut config = AppConfig::default();
    config.media.directories = vec![MonitoredDirectoryConfig {
        path: private_media_path.to_string_lossy().into_owned(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Warn,
    }];
    let config = Arc::new(config);
    let platform_info = Arc::new(PlatformInfo::detect().await.unwrap());
    let filesystem_manager = Arc::from(create_platform_filesystem_manager());
    let content_update_id = Arc::new(std::sync::atomic::AtomicU32::new(1));
    let web_metrics = Arc::new(WebHandlerMetrics::new());

    // Record some mock web events to verify web metrics
    web_metrics.record_browse_request(15000, true); // Cache hit
    web_metrics.record_browse_request(30000, false); // Cache miss
    web_metrics.record_file_serve(50000, true); // 1 file served
    web_metrics
        .bytes_transferred
        .fetch_add(1024 * 1024 * 100, std::sync::atomic::Ordering::Relaxed); // 100MB served

    let app_state = AppState {
        media_directories: Arc::new(tokio::sync::RwLock::new(config.media.directories.clone())),
        unavailable_roots: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
        config: config.clone(),
        live_config: Arc::new(vuio::state::LiveConfig::new(config.clone())),
        database: db,
        auth: Arc::new(vuio::web::auth::AuthState::testing()),
        platform_info,
        filesystem_manager,
        content_update_id,
        web_metrics,
        runtime_diagnostics: Arc::new(vuio::platform::diagnostics::SystemDiagnosticsSampler::new()),
        lifecycle_stats: Arc::new(vuio::lifecycle::ApplicationStats::new()),
        bookmarks: Arc::new(tokio::sync::Mutex::new(
            vuio::runtime_state::BookmarkRegistry::new(vuio::runtime_state::BOOKMARK_MAX_ENTRIES),
        )),
        log_file_path: temp_dir.path().join("vuio.log"),
        browse_cache: Arc::new(tokio::sync::Mutex::new(
            vuio::runtime_state::BrowseResponseCache::new(),
        )),
        mcp_clients: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_monitors: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_casts: Arc::new(tokio::sync::Mutex::new(
            vuio::runtime_state::ActiveCastRegistry::new(),
        )),
        discovered_tvs: Arc::new(vuio::runtime_state::RendererCache::new()),
        upnp_subscriptions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        cancellation: tokio_util::sync::CancellationToken::new(),
        background_tasks: tokio_util::task::TaskTracker::new(),
    };

    // 3. Test get_web_metrics handler (JSON format)
    let json_resp = get_web_metrics(State(app_state.clone()))
        .await
        .into_response();
    assert_eq!(json_resp.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(json_resp.into_body(), 10000)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    let json_val: serde_json::Value = serde_json::from_str(&body_str).unwrap();

    // Verify DB stats in JSON
    let db_stats = &json_val["database_stats"];
    assert_eq!(db_stats["total_files"], 3);
    assert_eq!(db_stats["video_files"], 1);
    assert_eq!(db_stats["audio_files"], 1);
    assert_eq!(db_stats["image_files"], 1);
    assert_eq!(db_stats["playlists"], 1);

    // Verify Web metrics in JSON
    let web_stats = &json_val["web_handler_metrics"];
    assert_eq!(web_stats["browse_requests"], 2);
    assert_eq!(web_stats["cache_hits"], 1);
    assert_eq!(web_stats["cache_misses"], 1);

    // Runtime diagnostics expose operational values, not configured filesystem paths.
    let runtime = &json_val["runtime_diagnostics"];
    assert_eq!(runtime["monitored_directory_count"], 1);
    assert_eq!(runtime["accessible_directory_count"], 1);
    assert!(runtime["snapshot"]["system"]["cpu_count"]
        .as_u64()
        .is_some_and(|count| count > 0));
    assert_eq!(runtime["snapshot"]["process"]["pid"], std::process::id());
    assert!(!body_str.contains(private_media_path.to_string_lossy().as_ref()));

    // 4. Test get_prometheus_metrics handler (exposition text format)
    let prom_resp = get_prometheus_metrics(State(app_state))
        .await
        .into_response();
    assert_eq!(prom_resp.status(), StatusCode::OK);

    let prom_bytes = axum::body::to_bytes(prom_resp.into_body(), 10000)
        .await
        .unwrap();
    let prom_str = String::from_utf8(prom_bytes.to_vec()).unwrap();

    // Verify custom Prometheus gauges exist and have correct values
    assert!(prom_str.contains("vuio_database_video_files 1"));
    assert!(prom_str.contains("vuio_database_audio_files 1"));
    assert!(prom_str.contains("vuio_database_image_files 1"));
    assert!(prom_str.contains("vuio_database_playlists 1"));
    assert!(prom_str.contains("vuio_web_browse_requests_total 2"));
    assert!(prom_str.contains("vuio_monitored_directories 1"));
    assert!(prom_str.contains("vuio_accessible_directories 1"));
    assert!(prom_str.contains("vuio_system_uptime_seconds"));
    assert!(prom_str.contains("vuio_process_memory_bytes"));
    assert!(!prom_str.contains(private_media_path.to_string_lossy().as_ref()));
}

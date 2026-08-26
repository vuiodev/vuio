//! End-to-end coverage for online media info: storage, the endpoints, and the
//! two properties the feature would be quietly broken without — that a rescan
//! does not destroy fetched records, and that a saved credential never comes back
//! out of the API.
//!
//! Parser and scorer cases live beside the code in `src/mediainfo/matching.rs`.

#![cfg(feature = "mediainfo")]

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tempfile::{tempdir, TempDir};
use tower::ServiceExt;

use vuio_core::config::{AppConfig, MonitoredDirectoryConfig, ValidationMode};
use vuio_core::database::sqlite::SqliteDatabase;
use vuio_core::database::{
    DatabaseManager, MediaFile, MediaInfoRecord, MediaInfoRepository, MediaRepository,
};
use vuio_core::platform::filesystem::create_platform_filesystem_manager;
use vuio_core::platform::PlatformInfo;
use vuio_core::state::AppState;
use vuio_core::web::{create_router, Surface};

/// The fixed token `AuthState::testing()` accepts.
const TEST_TOKEN: &str = "test-management-token-which-is-long-enough";

fn test_peer() -> SocketAddr {
    "127.0.0.1:54321".parse().unwrap()
}

fn record_for(media_file_id: i64, confidence: u8) -> MediaInfoRecord {
    MediaInfoRecord {
        media_file_id,
        provider: "tvmaze".to_string(),
        remote_id: "143".to_string(),
        kind: "series".to_string(),
        title: Some("Some Show".to_string()),
        original_title: None,
        overview: Some("A tale.".to_string()),
        release_date: Some("2011-04-17".to_string()),
        year: Some(2011),
        rating: Some(8.9),
        genres: vec!["Drama".to_string(), "Fantasy".to_string()],
        season: Some(2),
        episode: Some(5),
        artwork_key: Some("abcdef0123456789".to_string()),
        payload: r#"{"id":143}"#.to_string(),
        confidence,
        fetched_at: SystemTime::now(),
        mediainfo_version: 1,
    }
}

async fn database_with_one_file() -> (Arc<SqliteDatabase>, i64, TempDir) {
    let temp = tempdir().unwrap();
    let database = Arc::new(
        SqliteDatabase::new(temp.path().join("test.db"))
            .await
            .unwrap(),
    );
    database.initialize().await.unwrap();
    let file = MediaFile::new(
        PathBuf::from("/media/Show.Name.S02E05.1080p.mkv"),
        1024,
        "video/x-matroska".to_string(),
    );
    let id = database.store_media_file(&file).await.unwrap();
    (database, id, temp)
}

#[tokio::test]
async fn a_record_round_trips_through_the_database() {
    let (database, id, _temp) = database_with_one_file().await;

    database
        .bulk_store_mediainfo(&[record_for(id, 92)])
        .await
        .unwrap();

    let stored = database.get_mediainfo(id).await.unwrap().unwrap();
    assert_eq!(stored.title.as_deref(), Some("Some Show"));
    assert_eq!(stored.year, Some(2011));
    assert_eq!(stored.season, Some(2));
    assert_eq!(stored.episode, Some(5));
    assert_eq!(stored.confidence, 92);
    // Genres go in as a JSON array and have to come back as a list, not a string.
    assert_eq!(stored.genres, vec!["Drama", "Fantasy"]);
    assert_eq!(stored.rating, Some(8.9));

    let batch = database.get_mediainfo_batch(&[id, 9999]).await.unwrap();
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].media_file_id, id);
}

#[tokio::test]
async fn storing_the_same_file_twice_replaces_rather_than_duplicates() {
    let (database, id, _temp) = database_with_one_file().await;

    database
        .bulk_store_mediainfo(&[record_for(id, 40)])
        .await
        .unwrap();
    let mut better = record_for(id, 95);
    better.title = Some("A Better Match".to_string());
    database.bulk_store_mediainfo(&[better]).await.unwrap();

    let stored = database.get_mediainfo(id).await.unwrap().unwrap();
    assert_eq!(stored.confidence, 95);
    assert_eq!(stored.title.as_deref(), Some("A Better Match"));
    let stats = database.mediainfo_stats(60).await.unwrap();
    assert_eq!(stats.total, 1);
}

#[tokio::test]
async fn stats_and_the_flagged_list_split_on_the_threshold() {
    let temp = tempdir().unwrap();
    let database = Arc::new(SqliteDatabase::new(temp.path().join("t.db")).await.unwrap());
    database.initialize().await.unwrap();

    let mut ids = Vec::new();
    for (index, confidence) in [95_u8, 80, 30, 10].iter().enumerate() {
        let file = MediaFile::new(
            PathBuf::from(format!("/media/file{index}.mkv")),
            1024,
            "video/x-matroska".to_string(),
        );
        let id = database.store_media_file(&file).await.unwrap();
        database
            .bulk_store_mediainfo(&[record_for(id, *confidence)])
            .await
            .unwrap();
        ids.push(id);
    }

    let stats = database.mediainfo_stats(60).await.unwrap();
    assert_eq!(stats.total, 4);
    assert_eq!(stats.confident, 2);
    assert_eq!(stats.low_confidence, 2);
    assert_eq!(stats.with_artwork, 4);

    // Least certain first, so the worst matches are the ones on screen.
    let flagged = database.list_low_confidence(60, 10).await.unwrap();
    assert_eq!(flagged.len(), 2);
    assert_eq!(flagged[0].confidence, 10);
    assert_eq!(flagged[1].confidence, 30);
}

#[tokio::test]
async fn work_is_whatever_is_missing_stale_or_not_good_enough() {
    let (database, id, _temp) = database_with_one_file().await;

    // Never looked up.
    assert_eq!(
        database.media_ids_missing_mediainfo(1, 60).await.unwrap(),
        vec![id]
    );

    // A good match is done.
    database
        .bulk_store_mediainfo(&[record_for(id, 90)])
        .await
        .unwrap();
    assert!(database
        .media_ids_missing_mediainfo(1, 60)
        .await
        .unwrap()
        .is_empty());

    // Raising the threshold past it puts it back in the queue.
    assert_eq!(
        database.media_ids_missing_mediainfo(1, 95).await.unwrap(),
        vec![id]
    );

    // So does bumping the reader version, which is the whole point of storing it.
    assert_eq!(
        database.media_ids_missing_mediainfo(2, 60).await.unwrap(),
        vec![id]
    );
}

#[tokio::test]
async fn a_rescan_does_not_wipe_fetched_media_info() {
    // The reason this lives in its own table rather than `media_tags`, which is
    // cleared and rewritten every time a record is re-scanned. If a scan could
    // destroy this, every run of the fetch would have to start over.
    let (database, id, _temp) = database_with_one_file().await;
    database
        .bulk_store_mediainfo(&[record_for(id, 88)])
        .await
        .unwrap();

    let mut rescanned = MediaFile::new(
        PathBuf::from("/media/Show.Name.S02E05.1080p.mkv"),
        4096,
        "video/x-matroska".to_string(),
    );
    rescanned.title = Some("Re-read from the file".to_string());
    database.store_media_file(&rescanned).await.unwrap();

    let stored = database.get_mediainfo(id).await.unwrap();
    assert!(
        stored.is_some(),
        "a rescan destroyed the fetched media info"
    );
    assert_eq!(stored.unwrap().confidence, 88);
}

#[tokio::test]
async fn removing_a_file_takes_its_media_info_with_it() {
    let (database, id, _temp) = database_with_one_file().await;
    database
        .bulk_store_mediainfo(&[record_for(id, 88)])
        .await
        .unwrap();

    database
        .remove_media_file(&PathBuf::from("/media/Show.Name.S02E05.1080p.mkv"))
        .await
        .unwrap();

    assert!(database.get_mediainfo(id).await.unwrap().is_none());
}

#[tokio::test]
async fn clearing_forgets_everything() {
    let (database, id, _temp) = database_with_one_file().await;
    database
        .bulk_store_mediainfo(&[record_for(id, 88)])
        .await
        .unwrap();

    assert_eq!(database.clear_mediainfo().await.unwrap(), 1);
    assert!(database.get_mediainfo(id).await.unwrap().is_none());
}

// ── Endpoints ──────────────────────────────────────────────────────────────

async fn state_with(database: Arc<SqliteDatabase>, temp: &TempDir) -> AppState<SqliteDatabase> {
    let media_path = temp.path().join("media");
    tokio::fs::create_dir_all(&media_path).await.unwrap();
    let mut config = AppConfig::default();
    config.media.directories = vec![MonitoredDirectoryConfig {
        path: media_path.to_string_lossy().into_owned(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Warn,
    }];
    config.mediainfo.enabled = true;
    let config = Arc::new(config);

    AppState {
        media_directories: Arc::new(tokio::sync::RwLock::new(config.media.directories.clone())),
        unavailable_roots: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
        config: config.clone(),
        config_source: Arc::new(vuio_core::state::ConfigSource::default()),
        http_binding: Arc::new(vuio_core::state::HttpBinding::new(8080)),
        live_config: Arc::new(vuio_core::state::LiveConfig::new(config.clone())),
        database,
        auth: Arc::new(vuio_core::web::auth::AuthState::testing()),
        platform_info: Arc::new(PlatformInfo::detect().await.unwrap()),
        filesystem_manager: Arc::from(create_platform_filesystem_manager()),
        content_update_id: Arc::new(std::sync::atomic::AtomicU32::new(1)),
        web_metrics: Arc::new(vuio_core::web::diagnostics::WebHandlerMetrics::new()),
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
        mediainfo_job: Arc::new(tokio::sync::Mutex::new(Default::default())),
        #[cfg(feature = "casting")]
        discovered_tvs: Arc::new(vuio_core::runtime_state::RendererCache::new()),
        upnp_subscriptions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        radio: Arc::new(Default::default()),
        #[cfg(feature = "transcode")]
        transcode: Arc::new(Default::default()),
        cancellation: tokio_util::sync::CancellationToken::new(),
        background_tasks: tokio_util::task::TaskTracker::new(),
    }
}

async fn json_of(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn authed(method: &str, uri: &str, body: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo(test_peer()))
        .header("authorization", format!("Bearer {TEST_TOKEN}"));
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    builder
        .body(
            body.map(|body| Body::from(body.to_owned()))
                .unwrap_or(Body::empty()),
        )
        .unwrap()
}

#[tokio::test]
async fn the_status_endpoint_lists_every_provider() {
    let (database, _id, temp) = database_with_one_file().await;
    let router = create_router(state_with(database, &temp).await, Surface::Primary);

    let response = router
        .oneshot(authed("GET", "/api/admin/mediainfo", None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = json_of(response).await;
    let providers = body["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 10);

    let by_id = |id: &str| {
        providers
            .iter()
            .find(|provider| provider["id"] == id)
            .unwrap_or_else(|| panic!("{id} missing"))
            .clone()
    };
    // The key-free ones are usable and on out of the box.
    assert_eq!(by_id("tvmaze")["needs_credential"], false);
    assert_eq!(by_id("tvmaze")["enabled"], true);

    // TheMovieDB is on despite needing a key, because a key alone does not
    // enable a provider that is not in the configured list — it would sit unused
    // by anyone who supplied one. With no key it is skipped at lookup time
    // rather than failing, so this costs nothing where none is set.
    assert_eq!(by_id("tmdb")["needs_credential"], true);
    assert_eq!(by_id("tmdb")["enabled"], true);
    assert_eq!(by_id("tmdb")["has_credential"], false);
    assert_eq!(by_id("tmdb")["credential_env_var"], "VUIO_TMDB_API_KEY");

    // The other keyed providers stay off until the operator asks for them.
    assert_eq!(by_id("omdb")["needs_credential"], true);
    assert_eq!(by_id("omdb")["enabled"], false);

    assert_eq!(body["job"]["running"], false);
    assert_eq!(body["stats"]["total"], 0);
}

#[tokio::test]
async fn a_saved_credential_is_never_returned_by_the_api() {
    let (database, _id, temp) = database_with_one_file().await;
    let router = create_router(state_with(database, &temp).await, Surface::Primary);

    let response = router
        .clone()
        .oneshot(authed(
            "POST",
            "/api/admin/mediainfo/credentials",
            Some(r#"{"provider":"tmdb","token":"super-secret-key"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_of(response).await["has_credential"], true);

    let response = router
        .oneshot(authed("GET", "/api/admin/mediainfo", None))
        .await
        .unwrap();
    let body = json_of(response).await;

    // The whole document, not just the field we remembered to check: a token
    // leaking through any key at all is the failure worth catching.
    let serialized = serde_json::to_string(&body).unwrap();
    assert!(
        !serialized.contains("super-secret-key"),
        "the status endpoint echoed a stored credential back"
    );

    let tmdb = body["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["id"] == "tmdb")
        .unwrap()
        .clone();
    assert_eq!(tmdb["has_credential"], true);
}

#[tokio::test]
async fn an_empty_token_clears_the_stored_one() {
    let (database, _id, temp) = database_with_one_file().await;
    let router = create_router(state_with(database, &temp).await, Surface::Primary);

    router
        .clone()
        .oneshot(authed(
            "POST",
            "/api/admin/mediainfo/credentials",
            Some(r#"{"provider":"omdb","token":"a-key"}"#),
        ))
        .await
        .unwrap();
    let response = router
        .clone()
        .oneshot(authed(
            "POST",
            "/api/admin/mediainfo/credentials",
            Some(r#"{"provider":"omdb","token":""}"#),
        ))
        .await
        .unwrap();
    assert_eq!(json_of(response).await["has_credential"], false);

    let body = json_of(
        router
            .oneshot(authed("GET", "/api/admin/mediainfo", None))
            .await
            .unwrap(),
    )
    .await;
    let omdb = body["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["id"] == "omdb")
        .unwrap()
        .clone();
    assert_eq!(omdb["has_credential"], false);
}

#[tokio::test]
async fn a_credential_for_an_unknown_or_keyless_provider_is_rejected() {
    let (database, _id, temp) = database_with_one_file().await;
    let router = create_router(state_with(database, &temp).await, Surface::Primary);

    let response = router
        .clone()
        .oneshot(authed(
            "POST",
            "/api/admin/mediainfo/credentials",
            Some(r#"{"provider":"nope","token":"x"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // TVmaze needs no account, so offering it a key is a mistake worth reporting
    // rather than a value to store and never use.
    let response = router
        .oneshot(authed(
            "POST",
            "/api/admin/mediainfo/credentials",
            Some(r#"{"provider":"tvmaze","token":"x"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn cancelling_when_nothing_is_running_is_a_conflict() {
    let (database, _id, temp) = database_with_one_file().await;
    let router = create_router(state_with(database, &temp).await, Surface::Primary);

    let response = router
        .oneshot(authed("POST", "/api/admin/mediainfo/cancel", None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn running_with_the_feature_turned_off_is_refused() {
    let (database, _id, temp) = database_with_one_file().await;
    let mut state = state_with(database, &temp).await;
    let mut config = (*state.config).clone();
    config.mediainfo.enabled = false;
    let config = Arc::new(config);
    state.config = config.clone();
    state.live_config = Arc::new(vuio_core::state::LiveConfig::new(config));

    let response = create_router(state, Surface::Primary)
        .oneshot(authed("POST", "/api/admin/mediainfo/run", None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn the_endpoints_require_management_auth() {
    let (database, _id, temp) = database_with_one_file().await;
    let router = create_router(state_with(database, &temp).await, Surface::Primary);

    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/admin/mediainfo")
                .extension(ConnectInfo(test_peer()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn browse_json_carries_the_fetched_title_and_synopsis() {
    let (database, id, temp) = database_with_one_file().await;
    database
        .bulk_store_mediainfo(&[record_for(id, 92)])
        .await
        .unwrap();

    let router = create_router(state_with(database, &temp).await, Surface::Primary);
    let response = router
        .oneshot(authed("GET", "/api/media", None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = json_of(response).await;
    let file = &body["files"][0];
    assert_eq!(file["info_title"], "Some Show");
    assert_eq!(file["info_overview"], "A tale.");
    assert_eq!(file["info_art"], true);
}

#[tokio::test]
async fn an_uncertain_match_is_stored_but_never_shown() {
    // Searching TVmaze for "Arrival" returns the series "Dead on Arrival". It is
    // kept so the operator can see what happened, but relabelling the film with it
    // would be worse than showing the filename.
    let (database, id, temp) = database_with_one_file().await;
    let mut weak = record_for(id, 5);
    weak.title = Some("Dead on Arrival".to_string());
    database.bulk_store_mediainfo(&[weak]).await.unwrap();

    // Still on record.
    assert_eq!(
        database
            .get_mediainfo(id)
            .await
            .unwrap()
            .unwrap()
            .confidence,
        5
    );

    let router = create_router(state_with(database, &temp).await, Surface::Primary);
    let body = json_of(
        router
            .clone()
            .oneshot(authed("GET", "/api/media", None))
            .await
            .unwrap(),
    )
    .await;
    let file = &body["files"][0];
    assert!(
        file["info_title"].is_null(),
        "a match below the threshold was shown as the title"
    );
    assert_eq!(file["info_art"], false);

    // And it is what the Admin tab lists for review.
    let status = json_of(
        router
            .oneshot(authed("GET", "/api/admin/mediainfo", None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status["flagged"][0]["confidence"], 5);
    assert_eq!(status["flagged"][0]["matched_title"], "Dead on Arrival");
}

#[tokio::test]
async fn browse_json_reports_no_media_info_when_none_was_fetched() {
    let (database, _id, temp) = database_with_one_file().await;
    let router = create_router(state_with(database, &temp).await, Surface::Primary);

    let body = json_of(
        router
            .oneshot(authed("GET", "/api/media", None))
            .await
            .unwrap(),
    )
    .await;
    let file = &body["files"][0];
    assert!(file["info_title"].is_null());
    assert_eq!(file["info_art"], false);
}

#[tokio::test]
async fn dlna_browse_shows_filename_not_mediainfo_title_or_description() {
    let temp = tempdir().unwrap();
    let database = Arc::new(
        SqliteDatabase::new(temp.path().join("test.db"))
            .await
            .unwrap(),
    );
    database.initialize().await.unwrap();

    let media_path = temp.path().join("media");
    tokio::fs::create_dir_all(&media_path).await.unwrap();
    let file = MediaFile::new(
        media_path.join("Show.Name.S02E05.1080p.mkv"),
        1024,
        "video/x-matroska".to_string(),
    );
    let id = database.store_media_file(&file).await.unwrap();

    database
        .bulk_store_mediainfo(&[record_for(id, 92)])
        .await
        .unwrap();

    let state = state_with(database, &temp).await;
    let router = create_router(state, Surface::Primary);

    let soap_browse = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
  <s:Body>
    <u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
      <ObjectID>video/d0</ObjectID>
      <BrowseFlag>BrowseDirectChildren</BrowseFlag>
      <Filter>*</Filter>
      <StartingIndex>0</StartingIndex>
      <RequestedCount>10</RequestedCount>
      <SortCriteria></SortCriteria>
    </u:Browse>
  </s:Body>
</s:Envelope>"#;

    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/control/ContentDirectory")
                .header(
                    "soapaction",
                    "\"urn:schemas-upnp-org:service:ContentDirectory:1#Browse\"",
                )
                .header("content-type", "text/xml; charset=utf-8")
                .extension(ConnectInfo(test_peer()))
                .body(Body::from(soap_browse))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(response.into_body(), 128 * 1024)
        .await
        .unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();

    // DLNA client should see original filename, not fetched online title "Some Show"
    assert!(body.contains("Show.Name.S02E05.1080p.mkv"));
    assert!(!body.contains("&lt;dc:title&gt;Some Show&lt;/dc:title&gt;"));
    // DLNA should not contain fetched overview / description
    assert!(!body.contains("A tale."));
}

//! Serving AC-3 as audio a renderer without a Dolby licence can play.
//!
//! These drive the real router, so what they assert is what a TV receives. The
//! contract that matters is the one a DLNA renderer depends on and cannot
//! recover from if it is wrong: the `Content-Length` promised to a `HEAD` is the
//! number of bytes a `GET` delivers, and a byte range returns exactly the slice
//! of the full decode that lives at that offset.

#![cfg(feature = "transcode-ac3")]

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{header, Method, Request, StatusCode},
};
use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::{tempdir, TempDir};
use tower::ServiceExt;

use vuio_core::config::{AppConfig, MonitoredDirectoryConfig, ValidationMode};
use vuio_core::database::sqlite::SqliteDatabase;
use vuio_core::database::{DatabaseManager, MediaFile, MediaRepository};
use vuio_core::platform::filesystem::create_platform_filesystem_manager;
use vuio_core::platform::PlatformInfo;
use vuio_core::state::AppState;
use vuio_core::web::diagnostics::WebHandlerMetrics;
use vuio_core::web::{create_router, Surface};

/// A real 48 kHz stereo AC-3 bitstream — the same ffmpeg-encoded 440 Hz sine the
/// vendored decoder validates itself against. Using it here rather than a
/// synthetic stub is the point: the response is only meaningful if the bytes
/// really decode.
const AC3: &[u8] = include_bytes!("../../vendor/oxideav-ac3/tests/fixtures/sine440_stereo.ac3");

/// One AC-3 file in a library, and the router over it.
async fn library() -> (TempDir, AppState, i64) {
    let temp = tempdir().unwrap();
    let root = temp.path().join("media");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("sine.ac3");
    std::fs::write(&path, AC3).unwrap();

    let database = Arc::new(
        SqliteDatabase::new(temp.path().join("transcode.db"))
            .await
            .unwrap(),
    );
    database.initialize().await.unwrap();
    database
        .bulk_store_media_files(&[MediaFile::new(
            path.clone(),
            AC3.len() as u64,
            "audio/ac3".to_string(),
        )])
        .await
        .unwrap();
    let id = database
        .collect_all_media_files()
        .await
        .unwrap()
        .first()
        .unwrap()
        .id
        .unwrap();

    let mut config = AppConfig::default();
    config.media.directories = vec![MonitoredDirectoryConfig {
        path: root.to_string_lossy().into_owned(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Skip,
    }];
    let config = Arc::new(config);

    let state = AppState {
        media_directories: Arc::new(tokio::sync::RwLock::new(config.media.directories.clone())),
        unavailable_roots: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
        config: config.clone(),
        config_source: Arc::new(Default::default()),
        http_binding: Arc::new(vuio_core::state::HttpBinding::new(8080)),
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
        #[cfg(feature = "casting")]
        discovered_tvs: Arc::new(vuio_core::runtime_state::RendererCache::new()),
        upnp_subscriptions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        radio: Arc::new(Default::default()),
        #[cfg(feature = "transcode")]
        transcode: Arc::new(Default::default()),
        cancellation: tokio_util::sync::CancellationToken::new(),
        background_tasks: tokio_util::task::TaskTracker::new(),
    };

    (temp, state, id)
}

fn peer() -> SocketAddr {
    "127.0.0.1:50000".parse().unwrap()
}

async fn request(
    state: &AppState,
    id: i64,
    method: Method,
    range: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut builder = Request::builder()
        .method(method)
        .uri(format!("/media/{id}/transcode/audio.wav"))
        .extension(ConnectInfo(peer()));
    if let Some(range) = range {
        builder = builder.header(header::RANGE, range);
    }
    let response = create_router(state.clone(), Surface::Primary)
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), 64 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    (status, headers, body)
}

fn header_u64(headers: &axum::http::HeaderMap, name: header::HeaderName) -> u64 {
    headers[&name].to_str().unwrap().parse().unwrap()
}

#[tokio::test]
async fn an_ac3_file_is_served_as_playable_wav() {
    let (_temp, state, id) = library().await;
    let (status, headers, body) = request(&state, id, Method::GET, None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "audio/vnd.wave; codec=1");
    assert_eq!(headers[header::ACCEPT_RANGES], "bytes");

    // A renderer matching on protocolInfo has to be told this was converted, or
    // it may assume the bytes are the stored file.
    let features = headers["contentFeatures.dlna.org"].to_str().unwrap();
    assert!(
        features.contains("DLNA.ORG_CI=1"),
        "a transcoded resource must declare the conversion: {features}"
    );

    assert_eq!(&body[0..4], b"RIFF");
    assert_eq!(&body[8..12], b"WAVE");
    assert_eq!(&body[36..40], b"data");
    assert_eq!(
        body.len() as u64,
        header_u64(&headers, header::CONTENT_LENGTH),
        "the body must be exactly as long as the header promised"
    );

    // 16-bit stereo at 48 kHz, and the payload length the header declares must
    // be the payload actually delivered.
    assert_eq!(u16::from_le_bytes([body[22], body[23]]), 2, "channels");
    assert_eq!(
        u32::from_le_bytes([body[24], body[25], body[26], body[27]]),
        48_000,
        "sample rate"
    );
    let declared = u32::from_le_bytes([body[40], body[41], body[42], body[43]]) as usize;
    assert_eq!(declared, body.len() - 44);

    // The fixture is a 440 Hz sine, so silence here would mean we produced a
    // correctly-shaped empty response instead of decoding anything.
    let mut sum = 0f64;
    for c in body[44..].chunks_exact(2) {
        let v = i16::from_le_bytes([c[0], c[1]]) as f64;
        sum += v * v;
    }
    let rms = (sum / ((body.len() - 44) as f64 / 2.0)).sqrt();
    assert!(rms > 100.0, "decoded audio is silent (rms {rms})");
}

#[tokio::test]
async fn head_promises_the_length_that_get_delivers() {
    let (_temp, state, id) = library().await;
    let (head_status, head_headers, head_body) = request(&state, id, Method::HEAD, None).await;
    let (_, get_headers, get_body) = request(&state, id, Method::GET, None).await;

    assert_eq!(head_status, StatusCode::OK);
    assert!(head_body.is_empty(), "HEAD carries no body");
    assert_eq!(
        header_u64(&head_headers, header::CONTENT_LENGTH),
        header_u64(&get_headers, header::CONTENT_LENGTH),
    );
    assert_eq!(get_body.len() as u64, header_u64(&head_headers, header::CONTENT_LENGTH));
}

#[tokio::test]
async fn a_byte_range_returns_exactly_that_slice_of_the_full_decode() {
    let (_temp, state, id) = library().await;
    let (_, _, whole) = request(&state, id, Method::GET, None).await;

    // A range starting inside the audio, deliberately not on a frame boundary,
    // so the seek has to land mid-frame and discard the right number of samples.
    let start = 44 + 1536 * 2 * 2 + 5000;
    let end = start + 20_000;
    let (status, headers, part) =
        request(&state, id, Method::GET, Some(&format!("bytes={start}-{end}"))).await;

    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        headers[header::CONTENT_RANGE].to_str().unwrap(),
        format!("bytes {start}-{end}/{}", whole.len())
    );
    assert_eq!(part.len(), end - start + 1);
    assert_eq!(
        part,
        whole[start..=end],
        "a range must be byte-identical to that slice of the whole"
    );
}

#[tokio::test]
async fn a_range_spanning_the_header_boundary_is_still_exact() {
    let (_temp, state, id) = library().await;
    let (_, _, whole) = request(&state, id, Method::GET, None).await;

    // Straddles the 44-byte header and the first samples — the case where the
    // header and the decode both have to contribute to one response.
    let (status, _, part) = request(&state, id, Method::GET, Some("bytes=20-2043")).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(part.len(), 2024);
    assert_eq!(part, whole[20..=2043]);
}

#[tokio::test]
async fn a_range_past_the_end_is_refused_rather_than_truncated() {
    let (_temp, state, id) = library().await;
    let (_, _, whole) = request(&state, id, Method::GET, None).await;
    let past = whole.len() + 10;
    let (status, _, _) =
        request(&state, id, Method::GET, Some(&format!("bytes={past}-"))).await;
    assert_eq!(status, StatusCode::RANGE_NOT_SATISFIABLE);
}

#[tokio::test]
async fn turning_the_feature_off_withdraws_the_resource() {
    let (_temp, mut state, id) = library().await;
    let mut config = (*state.config).clone();
    config.transcode.enabled = false;
    state.config = Arc::new(config);

    let (status, _, _) = request(&state, id, Method::GET, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_file_that_needs_no_transcoding_has_no_transcoded_resource() {
    let (temp, state, _) = library().await;
    let mp3 = temp.path().join("media").join("song.mp3");
    std::fs::write(&mp3, b"not really an mp3").unwrap();
    state
        .database
        .bulk_store_media_files(&[MediaFile::new(mp3, 17, "audio/mpeg".to_string())])
        .await
        .unwrap();
    let id = state
        .database
        .collect_all_media_files()
        .await
        .unwrap()
        .iter()
        .find(|f| f.filename == "song.mp3")
        .unwrap()
        .id
        .unwrap();

    let (status, _, _) = request(&state, id, Method::GET, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn concurrent_transcodes_are_capped_rather_than_queued() {
    let (_temp, state, id) = library().await;
    // One slot, so the second request must be refused outright.
    let state = AppState {
        transcode: Arc::new(vuio_core::media::transcode::TranscodeState::new(1)),
        ..state
    };

    let router = create_router(state.clone(), Surface::Primary);
    let first = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/media/{id}/transcode/audio.wav"))
                .extension(ConnectInfo(peer()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    // The first response's body still holds the permit until it is consumed.
    let second = router
        .oneshot(
            Request::builder()
                .uri(format!("/media/{id}/transcode/audio.wav"))
                .extension(ConnectInfo(peer()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(second.headers()[header::RETRY_AFTER], "5");
}

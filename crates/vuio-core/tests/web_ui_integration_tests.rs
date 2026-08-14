//! The secondary listener's surface: server-side folder browse, and the app
//! that consumes it.
//!
//! `/api/browse` exists because the folder hierarchy cannot be rebuilt in the
//! browser from a page of files — the folders to group by are spread across
//! pages the client has not fetched, so client-side grouping is not merely slow
//! on a large library, it is wrong. These drive the real router, so what they
//! assert is what the browser receives.

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use serde_json::Value;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
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

/// A library shaped so that folders, nested folders and loose files all appear
/// in one listing:
///
/// ```text
/// media/
///   Movies/
///     Action/boom.mp4
///     Sci-Fi/orbit.mp4
///     loose.mp4
///   Music/track.mp3
/// ```
async fn library() -> (TempDir, AppState, PathBuf) {
    let temp = tempdir().unwrap();
    let root = temp.path().join("media");

    let files = [
        ("Movies/Action/boom.mp4", "video/mp4"),
        ("Movies/Sci-Fi/orbit.mp4", "video/mp4"),
        ("Movies/loose.mp4", "video/mp4"),
        ("Music/track.mp3", "audio/mpeg"),
    ];
    let mut records = Vec::new();
    for (relative, mime) in files {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"media").unwrap();
        records.push(MediaFile::new(path, 5, mime.to_string()));
    }

    let database = Arc::new(
        SqliteDatabase::new(temp.path().join("browse.db"))
            .await
            .unwrap(),
    );
    database.initialize().await.unwrap();
    database.bulk_store_media_files(&records).await.unwrap();

    let mut config = AppConfig::default();
    config.media.directories = vec![MonitoredDirectoryConfig {
        path: root.to_string_lossy().into_owned(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Skip,
    }];
    // A real config file on disk, because the settings endpoint reads it back to
    // work out which keys the operator actually set — the distinction the editor
    // needs to tell "unset, showing the default" from "set to this value".
    let config_path = temp.path().join("config.toml");
    std::fs::write(
        &config_path,
        toml::to_string(&config).expect("the default config serialises"),
    )
    .unwrap();
    let config = Arc::new(config);

    let state = AppState {
        media_directories: Arc::new(tokio::sync::RwLock::new(config.media.directories.clone())),
        unavailable_roots: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
        config: config.clone(),
        config_source: Arc::new(vuio_core::state::ConfigSource {
            path: config_path,
            durable: true,
            ..Default::default()
        }),
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
        discovered_tvs: Arc::new(vuio_core::runtime_state::RendererCache::new()),
        upnp_subscriptions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        radio: Arc::new(Default::default()),
        cancellation: tokio_util::sync::CancellationToken::new(),
        background_tasks: tokio_util::task::TaskTracker::new(),
    };

    (temp, state, root)
}

/// `AuthState::testing()` has management auth on, which is the interesting
/// case: the browser app is management surface and sits behind exactly the same
/// middleware as the dashboard.
const TEST_TOKEN: &str = "test-management-token-which-is-long-enough";

/// The management middleware extracts the peer address, which `oneshot` does
/// not supply the way `into_make_service_with_connect_info` does at runtime.
fn anonymous(uri: &str) -> Request<Body> {
    Request::get(uri)
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 51234))))
        .body(Body::empty())
        .unwrap()
}

fn request(uri: &str) -> Request<Body> {
    Request::get(uri)
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 51234))))
        .header("authorization", format!("Bearer {TEST_TOKEN}"))
        .body(Body::empty())
        .unwrap()
}

async fn get(state: &AppState, surface: Surface, uri: &str) -> (StatusCode, Vec<u8>, String) {
    let response = create_router(state.clone(), surface)
        .oneshot(request(uri))
        .await
        .unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, body, content_type)
}

async fn browse(state: &AppState, query: &str) -> Value {
    let (status, body, content_type) =
        get(state, Surface::Primary, &format!("/api/browse?{query}")).await;
    assert_eq!(status, StatusCode::OK, "browsing {query} failed");
    assert!(
        content_type.starts_with("application/json"),
        "{content_type}"
    );
    serde_json::from_slice(&body).expect("browse returns JSON")
}

fn names(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap().to_owned())
        .collect()
}

fn encode(path: &Path) -> String {
    // Only what a path can contain and a query string cannot carry literally.
    path.to_string_lossy()
        .replace(' ', "%20")
        .replace('#', "%23")
}

#[tokio::test]
async fn browse_without_a_path_lists_the_media_roots() {
    let (_temp, state, root) = library().await;

    let page = browse(&state, "").await;

    assert!(page["path"].is_null(), "the roots have no parent directory");
    assert_eq!(names(&page, "folders"), vec!["media"]);
    assert_eq!(
        page["folders"][0]["path"].as_str().unwrap(),
        root.to_string_lossy()
    );
    assert!(page["files"].as_array().unwrap().is_empty());
    assert_eq!(page["total"], 1);
}

#[tokio::test]
async fn browse_lists_subfolders_then_files() {
    let (_temp, state, root) = library().await;

    let page = browse(&state, &format!("path={}", encode(&root))).await;
    assert_eq!(names(&page, "folders"), vec!["Movies", "Music"]);
    assert!(
        page["files"].as_array().unwrap().is_empty(),
        "the root holds no files of its own"
    );
    assert_eq!(page["total"], 2);

    let movies = browse(&state, &format!("path={}", encode(&root.join("Movies")))).await;
    assert_eq!(names(&movies, "folders"), vec!["Action", "Sci-Fi"]);
    assert_eq!(names(&movies, "files"), vec!["loose.mp4"]);
    assert_eq!(movies["total"], 3);
    assert_eq!(
        movies["parent"].as_str().map(std::path::PathBuf::from),
        Some(std::path::PathBuf::from(
            page["path"]
                .as_str()
                .expect("a browsed directory reports itself")
        )),
        "a subfolder points back at the directory it was reached through"
    );
}

/// The count a folder card shows. Recursive, because a folder whose media all
/// sits in grandchildren is not empty and must not read as though it were.
#[tokio::test]
async fn a_folder_reports_how_much_its_whole_subtree_holds() {
    let (_temp, state, root) = library().await;

    let page = browse(&state, &format!("path={}", encode(&root))).await;
    let counts: Vec<u64> = page["folders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|folder| folder["file_count"].as_u64().unwrap())
        .collect();
    // Movies holds three files, none of them directly.
    assert_eq!(counts, vec![3, 1]);
}

/// One offset walks the folders and then continues into the files, so a client
/// pages the listing exactly as it is displayed.
#[tokio::test]
async fn offset_paging_crosses_the_folder_to_file_boundary() {
    let (_temp, state, root) = library().await;
    let movies = encode(&root.join("Movies"));

    let first = browse(&state, &format!("path={movies}&offset=0&limit=2")).await;
    assert_eq!(names(&first, "folders"), vec!["Action", "Sci-Fi"]);
    assert!(first["files"].as_array().unwrap().is_empty());
    assert_eq!(
        first["total"], 3,
        "total counts the whole listing, not the page"
    );

    let second = browse(&state, &format!("path={movies}&offset=2&limit=2")).await;
    assert!(second["folders"].as_array().unwrap().is_empty());
    assert_eq!(names(&second, "files"), vec!["loose.mp4"]);
    assert_eq!(second["total"], 3);

    // Straddling the boundary returns the tail of the folders and the head of
    // the files in one page.
    let straddle = browse(&state, &format!("path={movies}&offset=1&limit=2")).await;
    assert_eq!(names(&straddle, "folders"), vec!["Sci-Fi"]);
    assert_eq!(names(&straddle, "files"), vec!["loose.mp4"]);
}

#[tokio::test]
async fn a_category_narrows_folders_and_files_together() {
    let (_temp, state, root) = library().await;

    let audio = browse(&state, &format!("path={}&category=audio", encode(&root))).await;
    assert_eq!(
        names(&audio, "folders"),
        vec!["Music"],
        "a folder with no audio beneath it is not offered"
    );

    let video = browse(&state, &format!("path={}&category=video", encode(&root))).await;
    assert_eq!(names(&video, "folders"), vec!["Movies"]);
}

/// Without this the endpoint is a directory listing of the whole host.
#[tokio::test]
async fn a_path_outside_every_media_root_is_refused() {
    let (_temp, state, root) = library().await;

    for outside in [
        PathBuf::from("/etc"),
        root.parent().unwrap().to_path_buf(),
        // The traversal that a prefix check applied before canonicalization
        // would wave through.
        root.join("Movies/../../.."),
    ] {
        let (status, _, _) = get(
            &state,
            Surface::Primary,
            &format!("/api/browse?path={}", encode(&outside)),
        )
        .await;
        assert!(
            status.is_client_error(),
            "{} was accepted",
            outside.display()
        );
    }
}

#[tokio::test]
async fn an_unknown_category_is_refused() {
    let (_temp, state, root) = library().await;
    let (status, _, _) = get(
        &state,
        Surface::Primary,
        &format!("/api/browse?path={}&category=holograms", encode(&root)),
    )
    .await;
    assert!(status.is_client_error());
}

/// The two listeners differ in exactly one respect: what lives at `/`.
#[cfg(feature = "web-ui")]
#[tokio::test]
async fn the_two_surfaces_differ_only_at_the_root() {
    let (_temp, state, _root) = library().await;

    let (status, body, content_type) = get(&state, Surface::WebUi, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("text/html"), "{content_type}");
    let shell = String::from_utf8_lossy(&body);
    assert!(
        shell.contains("/_app/immutable/"),
        "the web UI surface should serve the Svelte shell, got: {}",
        &shell[..shell.len().min(200)]
    );

    let (status, body, _) = get(&state, Surface::Primary, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !String::from_utf8_lossy(&body).contains("/_app/immutable/"),
        "the main port keeps serving the built-in dashboard"
    );
}

/// The app and the API are one origin, so the second listener has to carry the
/// whole surface. Anything less would need CORS, and the session cookie is
/// SameSite=Strict.
#[cfg(feature = "web-ui")]
#[tokio::test]
async fn the_web_ui_surface_serves_the_same_api() {
    let (_temp, state, root) = library().await;

    for uri in [
        "/api/media?limit=1".to_string(),
        "/api/server-info".to_string(),
        format!("/api/browse?path={}", encode(&root)),
        "/description.xml".to_string(),
        "/healthz".to_string(),
    ] {
        let (status, _, _) = get(&state, Surface::WebUi, &uri).await;
        assert_eq!(status, StatusCode::OK, "{uri} was not served on the web UI");
    }
}

/// A client-side route has to reach the app; an API typo must not be answered
/// with 200 bytes of HTML that `res.json()` chokes on somewhere far away.
#[cfg(feature = "web-ui")]
#[tokio::test]
async fn unknown_paths_reach_the_app_but_unknown_api_paths_do_not() {
    let (_temp, state, _root) = library().await;

    let (status, body, content_type) = get(&state, Surface::WebUi, "/library/movies/42").await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("text/html"), "{content_type}");
    assert!(String::from_utf8_lossy(&body).contains("/_app/immutable/"));

    for uri in ["/api/nope", "/media/nope", "/metrics/nope"] {
        let (status, _, content_type) = get(&state, Surface::WebUi, uri).await;
        assert!(
            status.is_client_error() || status.is_server_error(),
            "{uri} answered {status}"
        );
        assert!(
            !content_type.starts_with("text/html"),
            "{uri} was answered with the app shell"
        );
    }
}

/// A navigation may be redirected to the login page; a subresource must never
/// be, because the browser would parse 200 bytes of HTML as JavaScript. The
/// dashboard's `/assets` was already excluded for this reason and the app's
/// `/_app` has to be too — every file under it is fetched by a `<script>` or a
/// dynamic import.
#[cfg(feature = "web-ui")]
#[tokio::test]
async fn the_apps_bundles_are_refused_rather_than_redirected() {
    let (_temp, state, _root) = library().await;

    let router = create_router(state.clone(), Surface::WebUi);
    let landing = router.oneshot(anonymous("/")).await.unwrap();
    assert_eq!(
        landing.status(),
        StatusCode::SEE_OTHER,
        "an anonymous navigation should be sent to the login page"
    );

    let router = create_router(state.clone(), Surface::WebUi);
    let bundle = router
        .oneshot(anonymous("/_app/immutable/entry/app.js"))
        .await
        .unwrap();
    assert_eq!(
        bundle.status(),
        StatusCode::UNAUTHORIZED,
        "a bundle must get a bare 401, never a 200 login page a <script> would parse"
    );
}

/// Hashed bundle names change on most builds, so this asks the shell which one
/// it wants rather than hard-coding a name that would rot immediately.
#[cfg(feature = "web-ui")]
#[tokio::test]
async fn the_apps_bundles_are_served_with_immutable_caching() {
    let (_temp, state, _root) = library().await;

    let (_, body, _) = get(&state, Surface::WebUi, "/").await;
    let shell = String::from_utf8_lossy(&body);
    let start = shell
        .find("/_app/immutable/")
        .expect("the shell loads a bundle");
    let bundle: String = shell[start..]
        .chars()
        .take_while(|character| !"\"'".contains(*character))
        .collect();

    let response = create_router(state.clone(), Surface::WebUi)
        .oneshot(request(&bundle))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "{bundle}");
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("public, max-age=31536000, immutable"),
        "{bundle}"
    );
}

// ---------------------------------------------------------------------------
// Contract between the server and the browser interface.
//
// The interface reads these responses by field name, and nothing in either
// language checks that the names still line up. They did not: `/metrics/json`
// nests everything under `web_handler_metrics` and calls it
// `average_response_time_ms`, while the interface read a flat object with
// `avg_response_time_ms` — so every tile was `undefined` and the one that
// called `.toFixed()` took the whole screen down.
//
// These assert the names the interface actually depends on. A rename in Rust
// now fails here rather than silently blanking a screen nobody looks at until
// a user reports it.
// ---------------------------------------------------------------------------

fn field<'a>(value: &'a Value, path: &str) -> &'a Value {
    let mut current = value;
    for segment in path.split('.') {
        current = current.get(segment).unwrap_or_else(|| {
            panic!("{path} is missing from the response (stopped at {segment})")
        });
    }
    current
}

#[tokio::test]
async fn metrics_carry_every_field_the_interface_reads() {
    let (_temp, state, _root) = library().await;
    let (status, body, _) = get(&state, Surface::WebUi, "/metrics/json").await;
    assert_eq!(status, StatusCode::OK);
    let metrics: Value = serde_json::from_slice(&body).expect("metrics are JSON");

    for path in [
        "web_handler_metrics.browse_requests",
        "web_handler_metrics.cache_hits",
        "web_handler_metrics.cache_hit_rate_percent",
        "web_handler_metrics.directory_listings",
        "web_handler_metrics.file_serves",
        "web_handler_metrics.errors",
        "web_handler_metrics.average_response_time_ms",
        "web_handler_metrics.gigabytes_transferred",
        "web_handler_metrics.database_backend",
        "database_stats.total_files",
        "database_stats.total_size_bytes",
        "database_stats.database_size_bytes",
        "database_stats.video_files",
        "database_stats.audio_files",
        "database_stats.image_files",
        "database_stats.playlists",
        "runtime_diagnostics.monitored_directory_count",
        "runtime_diagnostics.accessible_directory_count",
        "runtime_diagnostics.platform",
        "runtime_diagnostics.architecture",
        "runtime_diagnostics.unavailable_or_incomplete_roots",
    ] {
        field(&metrics, path);
    }

    // Present but nullable: absent on a build without the `diagnostics` feature,
    // which is why the interface reads every one of its fields defensively.
    let snapshot = field(&metrics, "runtime_diagnostics.snapshot");
    if !snapshot.is_null() {
        for path in ["system.uptime_seconds", "process.pid", "disks.filesystems"] {
            field(snapshot, path);
        }
    }
}

#[tokio::test]
async fn the_config_schema_is_shaped_the_way_the_editor_expects() {
    let (_temp, state, _root) = library().await;
    let (status, body, _) = get(&state, Surface::WebUi, "/api/admin/config").await;
    assert_eq!(status, StatusCode::OK);
    let schema: Value = serde_json::from_slice(&body).expect("the schema is JSON");

    for key in [
        "sections",
        "values",
        "present",
        "overrides",
        "directories",
        "effective_directories",
        "library_defaults",
        "runtime",
    ] {
        field(&schema, key);
    }
    for path in [
        "runtime.config_path",
        "runtime.writable",
        "runtime.version",
        "library_defaults.exclude_patterns",
    ] {
        field(&schema, path);
    }

    // The editor switches on `type`, lower snake_case, and renders nothing at
    // all for a variant it does not know. It must see every one that exists.
    let known = ["bool", "int", "text", "path", "enum", "string_list"];
    let mut seen = std::collections::HashSet::new();
    let sections = field(&schema, "sections")
        .as_array()
        .expect("sections is a list");
    assert!(!sections.is_empty());
    for section in sections {
        for spec in field(section, "fields")
            .as_array()
            .expect("fields is a list")
        {
            let kind = field(spec, "type").as_str().expect("type is a string");
            assert!(
                known.contains(&kind),
                "{} has field type {kind:?}, which the editor cannot render",
                field(spec, "key")
            );
            seen.insert(kind.to_owned());

            let impact = field(spec, "impact").as_str().expect("impact is a string");
            assert!(
                ["live", "restart", "next_start"].contains(&impact),
                "unknown impact {impact:?}"
            );
            field(spec, "removable");
            field(spec, "help");
            field(spec, "label");
        }
    }

    // The libraries section is the one the editor renders as cards rather than
    // as fields, and it is recognised by this flag alone.
    assert!(
        sections
            .iter()
            .any(|section| section.get("directories").and_then(Value::as_bool) == Some(true)),
        "no section is marked as the libraries editor"
    );

    // Every key the editor can show must have a value and a presence entry, or
    // the field renders empty with no way to tell "unset" from "missing".
    let values = field(&schema, "values");
    let present = field(&schema, "present");
    for section in sections {
        for spec in field(section, "fields").as_array().unwrap() {
            let key = field(spec, "key").as_str().unwrap();
            assert!(values.get(key).is_some(), "{key} has no value");
            assert!(present.get(key).is_some(), "{key} has no presence entry");
        }
    }
    assert!(seen.contains("bool") && seen.contains("int") && seen.contains("text"));
}

#[cfg(feature = "mediainfo")]
#[tokio::test]
async fn provider_status_says_where_each_credential_comes_from() {
    let (_temp, state, _root) = library().await;
    let (status, body, _) = get(&state, Surface::WebUi, "/api/admin/mediainfo").await;
    assert_eq!(status, StatusCode::OK);
    let payload: Value = serde_json::from_slice(&body).expect("status is JSON");

    let providers = field(&payload, "providers").as_array().expect("a list");
    assert!(!providers.is_empty());
    for provider in providers {
        for key in [
            "id",
            "label",
            "group",
            "provides",
            "needs_credential",
            "has_credential",
            "credential_source",
            "enabled",
        ] {
            field(provider, key);
        }
        let source = field(provider, "credential_source").as_str().unwrap();
        assert!(
            ["user", "environment", "none"].contains(&source),
            "unknown credential source {source:?}"
        );

        // A provider that takes a key has to say which variable supplies it,
        // because in a container that is the only way to set one.
        if field(provider, "needs_credential").as_bool() == Some(true) {
            let name = field(provider, "credential_env_var").as_str().unwrap();
            let id = field(provider, "id").as_str().unwrap();
            assert_eq!(name, format!("VUIO_{}_API_KEY", id.to_uppercase()));
        }
    }

    // TheMovieDB ships on, so a key supplied to the server is used without the
    // operator also having to edit `mediainfo.providers`.
    let tmdb = providers
        .iter()
        .find(|p| p.get("id").and_then(Value::as_str) == Some("tmdb"))
        .expect("tmdb is a known provider");
    assert_eq!(field(tmdb, "enabled").as_bool(), Some(true));
}

/// A station's audio has to be reachable by things that cannot log in.
///
/// This is the whole reason the stream and the public station list sit outside
/// the management middleware: a hi-fi, VLC, or another VuIO server building its
/// local-stations list has nowhere to put a bearer token. Everything that
/// *runs* a station stays behind the login.
#[tokio::test]
async fn radio_listening_is_public_and_running_a_station_is_not() {
    let (_temp, state, _root) = library().await;

    let public = [
        "/api/radio/stations",
        "/api/radio/stations/1/stream",
        "/api/radio/stations/1/stream.mp3",
    ];
    for uri in public {
        let response = create_router(state.clone(), Surface::Primary)
            .oneshot(anonymous(uri))
            .await
            .unwrap();
        assert_ne!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{uri} must not require a login"
        );
        assert_ne!(
            response.status(),
            StatusCode::FOUND,
            "{uri} must not redirect a player to a login page"
        );
    }

    // No station is on the air in this fixture, so the stream is a 404 while
    // the list is an empty array.
    let (status, body, content_type) = get(&state, Surface::Primary, "/api/radio/stations").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        content_type.starts_with("application/json"),
        "{content_type}"
    );
    let stations: Value = serde_json::from_slice(&body).expect("the station list is JSON");
    assert_eq!(stations.as_array().map(Vec::len), Some(0));

    let response = create_router(state.clone(), Surface::Primary)
        .oneshot(anonymous("/api/radio/stations/1/stream"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    for uri in ["/api/radio/admin/stations", "/api/radio/peers"] {
        let response = create_router(state.clone(), Surface::Primary)
            .oneshot(anonymous(uri))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{uri} is administration and must require a login"
        );
    }

    // Both listeners serve the radio API: the browser app lives on the second
    // one and calls exactly these endpoints.
    let (status, _, _) = get(&state, Surface::WebUi, "/api/radio/admin/stations").await;
    assert_eq!(status, StatusCode::OK);
}

/// A station is created stopped, starts, and comes back enabled — which is what
/// makes a restart resume it.
#[tokio::test]
async fn a_station_is_created_stopped_and_remembers_being_started() {
    let (_temp, state, root) = library().await;

    // Take the folder from the browse API rather than from the fixture's own
    // path, because that is where the studio gets it: a media root reached
    // through a symlink — /var on macOS — is stored canonicalised, and a test
    // that passes its own spelling of the path is testing the wrong thing.
    let listing = browse(&state, &format!("path={}", encode(&root))).await;
    let movies = listing["folders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|folder| folder["name"] == "Movies")
        .expect("the fixture has a Movies folder")["path"]
        .as_str()
        .unwrap()
        .to_owned();

    let create = Request::post("/api/radio/admin/stations")
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 51234))))
        .header("authorization", format!("Bearer {TEST_TOKEN}"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "name": "Test Station",
                "genre": "Test",
                "folders": [movies],
                "mode": "linear",
            })
            .to_string(),
        ))
        .unwrap();

    let response = create_router(state.clone(), Surface::Primary)
        .oneshot(create)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let station: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(station["name"], "Test Station");
    assert_eq!(
        station["enabled"], false,
        "creating a station must not put it on the air"
    );
    assert_eq!(station["is_live"], false);

    // That folder holds only video, so there is nothing to broadcast and
    // starting fails with a reason rather than leaving a silent station "live".
    let id = station["id"].as_i64().unwrap();
    let start = Request::post(format!("/api/radio/admin/stations/{id}/start"))
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 51234))))
        .header("authorization", format!("Bearer {TEST_TOKEN}"))
        .body(Body::empty())
        .unwrap();
    let response = create_router(state.clone(), Surface::Primary)
        .oneshot(start)
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "a station with nothing broadcastable must say so"
    );
    let message = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let message = String::from_utf8_lossy(&message);
    assert!(
        message.contains("folders"),
        "the reason must point at the folders, which is what the operator can fix: {message}"
    );
}

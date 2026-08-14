use audiotags::Tag;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

use vuio_core::database::playlist_formats::PlaylistFileManager;
use vuio_core::database::sqlite::SqliteDatabase;
use vuio_core::database::{DatabaseManager, MediaRepository, PlaylistRepository};
use vuio_core::media::MediaScanner;

fn decode_base64(s: &str) -> Vec<u8> {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bytes = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;

    for c in s.chars() {
        if c == '=' {
            break;
        }
        if let Some(pos) = CHARSET.iter().position(|&x| x == c as u8) {
            buffer = (buffer << 6) | pos as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                bytes.push((buffer >> bits) as u8);
            }
        }
    }
    bytes
}

#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI when writing ID3 via audiotags (QEMU guest)"
)]
async fn test_audio_implementation_and_features() {
    // 1. Setup temporary directory for media and database
    let temp_dir = tempdir().unwrap();
    let raw_media_dir = temp_dir.path().join("media");
    fs::create_dir_all(&raw_media_dir).unwrap();
    let media_dir = fs::canonicalize(raw_media_dir).unwrap();

    let db_path = temp_dir.path().join("test_media.db");
    let db = Arc::new(SqliteDatabase::new(db_path).await.unwrap());
    db.initialize().await.unwrap();

    // 2. Generate minimal valid silent MP3 file
    let silent_mp3_base64 = "SUQzBAAAAAAAI1RTU0UAAAAPAAADTGF2ZjU2LjM2LjEwMAAAAAAAAAAAAAAA//OEAAAAAAAAAAAAAAAAAAAAAAAASW5mbwAAAA8AAAAEAAABIADAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV6urq6urq6urq6urq6urq6urq6urq6urq6v////////////////////////////////8AAAAATGF2YzU2LjQxAAAAAAAAAAAAAAAAJAAAAAAAAAAAASDs90hvAAAAAAAAAAAAAAAAAAAA//MUZAAAAAGkAAAAAAAAA0gAAAAATEFN//MUZAMAAAGkAAAAAAAAA0gAAAAARTMu//MUZAYAAAGkAAAAAAAAA0gAAAAAOTku//MUZAkAAAGkAAAAAAAAA0gAAAAANVVV";
    let mp3_bytes = decode_base64(silent_mp3_base64);

    let file1_path = media_dir.join("AC-DC - Back In Black.mp3");
    let file2_path = media_dir.join("02 - Metallica - Enter Sandman.mp3");
    let file3_path = media_dir.join("03 - Pink Floyd - Time.mp3");
    let file4_path = media_dir.join("Led Zeppelin - Stairway to Heaven.mp3"); // No ID3 tags to test fallback filename parsing

    fs::write(&file1_path, &mp3_bytes).unwrap();
    fs::write(&file2_path, &mp3_bytes).unwrap();
    fs::write(&file3_path, &mp3_bytes).unwrap();
    fs::write(&file4_path, &mp3_bytes).unwrap();

    // Write ID3 tags using audiotags to file1, file2, file3
    let mut tag1 = Tag::new().read_from_path(&file1_path).unwrap();
    tag1.set_title("Back In Black");
    tag1.set_artist("AC/DC"); // Tests slash handling
    tag1.set_album_title("Back In Black");
    tag1.set_genre("Rock");
    tag1.set_year(1980);
    tag1.set_track_number(1);
    tag1.write_to_path(&file1_path.to_string_lossy()).unwrap();

    let mut tag2 = Tag::new().read_from_path(&file2_path).unwrap();
    tag2.set_title("Enter Sandman");
    tag2.set_artist("Metallica");
    tag2.set_album_title("Metallica");
    tag2.set_genre("Metal");
    tag2.set_year(1991);
    tag2.set_track_number(2);
    tag2.write_to_path(&file2_path.to_string_lossy()).unwrap();

    let mut tag3 = Tag::new().read_from_path(&file3_path).unwrap();
    tag3.set_title("Time");
    tag3.set_artist("Pink Floyd");
    tag3.set_album_title("Dark Side of the Moon");
    tag3.set_genre("Progressive Rock");
    tag3.set_year(1973);
    tag3.set_track_number(3);
    tag3.write_to_path(&file3_path.to_string_lossy()).unwrap();

    // 3. Scan the directory with MediaScanner
    let scanner = MediaScanner::with_database(db.clone());
    let scan_result = scanner.scan_directory_recursive(&media_dir).await.unwrap();
    assert_eq!(scan_result.new, 4);

    // 4. Verify tag metadata is correctly populated in DB
    // Check AC/DC
    let f1_db = db.get_file_by_path(&file1_path).await.unwrap().unwrap();
    assert_eq!(f1_db.title.as_deref(), Some("Back In Black"));
    assert_eq!(f1_db.artist.as_deref(), Some("AC/DC"));
    assert_eq!(f1_db.album.as_deref(), Some("Back In Black"));
    assert_eq!(f1_db.genre.as_deref(), Some("Rock"));
    assert_eq!(f1_db.track_number, Some(1));
    assert_eq!(f1_db.year, Some(1980));

    // Check Metallica
    let f2_db = db.get_file_by_path(&file2_path).await.unwrap().unwrap();
    assert_eq!(f2_db.title.as_deref(), Some("Enter Sandman"));
    assert_eq!(f2_db.artist.as_deref(), Some("Metallica"));
    assert_eq!(f2_db.album.as_deref(), Some("Metallica"));
    assert_eq!(f2_db.genre.as_deref(), Some("Metal"));
    assert_eq!(f2_db.track_number, Some(2));
    assert_eq!(f2_db.year, Some(1991));

    // Check Led Zeppelin (no tags, parses from filename fallback)
    let f4_db = db.get_file_by_path(&file4_path).await.unwrap().unwrap();
    assert_eq!(f4_db.title.as_deref(), Some("Stairway to Heaven"));
    assert_eq!(f4_db.artist.as_deref(), Some("Led Zeppelin"));
    assert_eq!(f4_db.album, None);
    assert_eq!(f4_db.track_number, None); // Should be None since filename didn't have track number

    // 5. Verify music categorization queries (e.g. artists, genres, albums, years)
    let artists = db.get_artists().await.unwrap();
    let artist_names: Vec<&str> = artists.iter().map(|c| c.name.as_str()).collect();
    assert!(artist_names.contains(&"AC/DC"));
    assert!(artist_names.contains(&"Metallica"));
    assert!(artist_names.contains(&"Pink Floyd"));
    assert!(artist_names.contains(&"Led Zeppelin"));

    // Verify querying tracks by artist (including slash handling)
    let acdc_tracks = db.get_music_by_artist("AC/DC").await.unwrap();
    assert_eq!(acdc_tracks.len(), 1);
    assert_eq!(acdc_tracks[0].title.as_deref(), Some("Back In Black"));

    let metallica_tracks = db.get_music_by_artist("Metallica").await.unwrap();
    assert_eq!(metallica_tracks.len(), 1);
    assert_eq!(metallica_tracks[0].title.as_deref(), Some("Enter Sandman"));

    // Verify querying albums and tracks by album
    let albums = db.get_albums(None).await.unwrap();
    let album_names: Vec<&str> = albums.iter().map(|c| c.name.as_str()).collect();
    assert!(album_names.contains(&"Back In Black"));
    assert!(album_names.contains(&"Dark Side of the Moon"));
    assert!(album_names.contains(&"Metallica"));

    let back_in_black_tracks = db.get_music_by_album("Back In Black", None).await.unwrap();
    assert_eq!(back_in_black_tracks.len(), 1);
    assert_eq!(
        back_in_black_tracks[0].title.as_deref(),
        Some("Back In Black")
    );

    // Verify querying genres and tracks by genre
    let genres = db.get_genres().await.unwrap();
    let genre_names: Vec<&str> = genres.iter().map(|c| c.name.as_str()).collect();
    assert!(genre_names.contains(&"Rock"));
    assert!(genre_names.contains(&"Metal"));

    let rock_tracks = db.get_music_by_genre("Rock").await.unwrap();
    assert_eq!(rock_tracks.len(), 1);
    assert_eq!(rock_tracks[0].title.as_deref(), Some("Back In Black"));

    // Verify querying years and tracks by year
    let years = db.get_years().await.unwrap();
    let year_values: Vec<u32> = years
        .iter()
        .map(|c| c.name.parse::<u32>().unwrap())
        .collect();
    assert!(year_values.contains(&1980));
    assert!(year_values.contains(&1991));

    let tracks_1980 = db.get_music_by_year(1980).await.unwrap();
    assert_eq!(tracks_1980.len(), 1);
    assert_eq!(tracks_1980[0].title.as_deref(), Some("Back In Black"));

    // 6. Test relative playlist importing (M3U & PLS)
    // favorites.m3u using relative paths
    let m3u_content = r#"#EXTM3U
#EXTINF:250,AC/DC - Back In Black
AC-DC - Back In Black.mp3
#EXTINF:331,Metallica - Enter Sandman
02 - Metallica - Enter Sandman.mp3
#EXTINF:421,Led Zeppelin - Stairway to Heaven
Led Zeppelin - Stairway to Heaven.mp3
"#;
    let m3u_path = media_dir.join("favorites.m3u");
    fs::write(&m3u_path, m3u_content).unwrap();

    // rock.pls using relative paths
    let pls_content = r#"[playlist]
NumberOfEntries=2
File1=AC-DC - Back In Black.mp3
Title1=AC/DC - Back In Black
Length1=250
File2=03 - Pink Floyd - Time.mp3
Title2=Pink Floyd - Time
Length2=421
Version=2
"#;
    let pls_path = media_dir.join("rock.pls");
    fs::write(&pls_path, pls_content).unwrap();

    // Import playlists recursively
    let playlist_ids =
        PlaylistFileManager::scan_and_import_playlists_recursive(db.as_ref(), &media_dir)
            .await
            .unwrap();
    assert_eq!(playlist_ids.len(), 2);

    let playlists = db.get_playlists().await.unwrap();
    assert_eq!(playlists.len(), 2);

    let playlist_names: Vec<&str> = playlists.iter().map(|p| p.name.as_str()).collect();
    assert!(playlist_names.contains(&"favorites"));
    assert!(playlist_names.contains(&"rock"));

    // Verify favorites tracks
    let favorites_id = playlists
        .iter()
        .find(|p| p.name == "favorites")
        .unwrap()
        .id
        .unwrap();
    let favorites_tracks = db.get_playlist_tracks(favorites_id).await.unwrap();
    assert_eq!(favorites_tracks.len(), 3);
    let fav_titles: Vec<&str> = favorites_tracks
        .iter()
        .map(|t| t.title.as_deref().unwrap())
        .collect();
    assert!(fav_titles.contains(&"Back In Black"));
    assert!(fav_titles.contains(&"Enter Sandman"));
    assert!(fav_titles.contains(&"Stairway to Heaven"));

    // Verify rock tracks
    let rock_id = playlists
        .iter()
        .find(|p| p.name == "rock")
        .unwrap()
        .id
        .unwrap();
    let rock_tracks = db.get_playlist_tracks(rock_id).await.unwrap();
    assert_eq!(rock_tracks.len(), 2);
    let rock_titles: Vec<&str> = rock_tracks
        .iter()
        .map(|t| t.title.as_deref().unwrap())
        .collect();
    assert!(rock_titles.contains(&"Back In Black"));
    assert!(rock_titles.contains(&"Time"));
}

#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI when writing ID3 via audiotags (QEMU guest)"
)]
async fn test_cover_art_retrieval_and_xml() {
    use axum::extract::{Path, State};
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use std::fs;
    use std::sync::Arc;
    use tempfile::tempdir;
    use vuio_core::config::AppConfig;
    use vuio_core::database::sqlite::SqliteDatabase;
    use vuio_core::database::DatabaseManager;
    use vuio_core::media::MediaScanner;
    use vuio_core::platform::filesystem::create_platform_filesystem_manager;
    use vuio_core::platform::PlatformInfo;
    use vuio_core::state::AppState;
    use vuio_core::web::xml::generate_browse_response;
    use vuio_core::web::{diagnostics::WebHandlerMetrics, streaming::serve_cover};

    // 1. Setup temporary directory for media and database
    let temp_dir = tempdir().unwrap();
    let raw_media_dir = temp_dir.path().join("media");
    fs::create_dir_all(&raw_media_dir).unwrap();
    let media_dir = fs::canonicalize(raw_media_dir).unwrap();

    let db_path = temp_dir.path().join("test_cover.db");
    let db = Arc::new(SqliteDatabase::new(db_path).await.unwrap());
    db.initialize().await.unwrap();

    // 2. Generate minimal valid silent MP3 file
    let silent_mp3_base64 = "SUQzBAAAAAAAI1RTU0UAAAAPAAADTGF2ZjU2LjM2LjEwMAAAAAAAAAAAAAAA//OEAAAAAAAAAAAAAAAAAAAAAAAASW5mbwAAAA8AAAAEAAABIADAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV6urq6urq6urq6urq6urq6urq6urq6urq6v////////////////////////////////8AAAAATGF2YzU2LjQxAAAAAAAAAAAAAAAAJAAAAAAAAAAAASDs90hvAAAAAAAAAAAAAAAAAAAA//MUZAAAAAGkAAAAAAAAA0gAAAAATEFN//MUZAMAAAGkAAAAAAAAA0gAAAAARTMu//MUZAYAAAGkAAAAAAAAA0gAAAAAOTku//MUZAkAAAGkAAAAAAAAA0gAAAAANVVV";
    let mp3_bytes = decode_base64(silent_mp3_base64);

    let audio_path = media_dir.join("song.mp3");
    fs::write(&audio_path, &mp3_bytes).unwrap();

    // 3. Write a fake cover.jpg in the same directory
    let cover_path = media_dir.join("cover.jpg");
    let fake_cover_data = b"fake image bytes content";
    fs::write(&cover_path, fake_cover_data).unwrap();

    // 4. Scan the directory with MediaScanner
    let scanner = MediaScanner::with_database(db.clone());
    let scan_result = scanner.scan_directory_recursive(&media_dir).await.unwrap();
    assert_eq!(scan_result.new, 2);

    // 5. Get file from DB to find its assigned ID
    let db_file = db.get_file_by_path(&audio_path).await.unwrap().unwrap();
    let file_id = db_file.id.unwrap();

    // 6. Setup mock AppState
    let config = Arc::new(AppConfig::default());
    let platform_info = Arc::new(PlatformInfo::detect().await.unwrap());
    let filesystem_manager = Arc::from(create_platform_filesystem_manager());
    let content_update_id = Arc::new(std::sync::atomic::AtomicU32::new(1));
    let web_metrics = Arc::new(WebHandlerMetrics::new());

    let app_state = AppState {
        media_directories: Arc::new(tokio::sync::RwLock::new(config.media.directories.clone())),
        unavailable_roots: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
        config: config.clone(),
        config_source: std::sync::Arc::new(vuio_core::state::ConfigSource::default()),
        http_binding: std::sync::Arc::new(vuio_core::state::HttpBinding::new(8080)),
        live_config: Arc::new(vuio_core::state::LiveConfig::new(config.clone())),
        database: db.clone(),
        auth: Arc::new(vuio_core::web::auth::AuthState::testing()),
        platform_info,
        filesystem_manager,
        content_update_id,
        web_metrics,
        runtime_diagnostics: Arc::new(
            vuio_core::platform::diagnostics::SystemDiagnosticsSampler::new(),
        ),
        lifecycle_stats: Arc::new(vuio_core::lifecycle::ApplicationStats::new()),
        bookmarks: Arc::new(tokio::sync::Mutex::new(
            vuio_core::runtime_state::BookmarkRegistry::new(
                vuio_core::runtime_state::BOOKMARK_MAX_ENTRIES,
            ),
        )),
        log_file_path: temp_dir.path().join("vuio.log"),
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

    // 7. Verify UPnP XML response generation contains upnp:albumArtURI
    let xml_response = generate_browse_response(
        "audio",
        &[],
        std::slice::from_ref(&db_file),
        &app_state,
        "127.0.0.1",
        1,
    )
    .await;

    let expected_url = format!(
        "http://127.0.0.1:{}/media/{}/cover",
        app_state.config.server.port, file_id
    );
    assert!(
        xml_response.contains("upnp:albumArtURI"),
        "XML response did not contain upnp:albumArtURI tag"
    );
    assert!(
        xml_response.contains(&expected_url),
        "XML response did not contain expected cover URL: {}",
        xml_response
    );

    // 8. Test serve_cover endpoint directly
    let response = serve_cover(State(app_state.clone()), Path(file_id.to_string()))
        .await
        .unwrap()
        .into_response();

    assert_eq!(response.status(), StatusCode::OK);

    let headers = response.headers();
    assert_eq!(headers.get("content-type").unwrap(), "image/jpeg");

    let body_bytes = axum::body::to_bytes(response.into_body(), 10000)
        .await
        .unwrap();
    assert_eq!(body_bytes.as_ref(), fake_cover_data);
}

#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI when writing ID3 via audiotags (QEMU guest)"
)]
async fn test_radio_playlist_import() {
    use axum::extract::{ConnectInfo, Path, State};
    use axum::http::StatusCode;
    use axum::http::{HeaderMap, Method};
    use axum::response::IntoResponse;
    use futures_util::StreamExt;
    use std::fs;
    use std::sync::Arc;
    use tempfile::tempdir;
    use vuio_core::config::AppConfig;
    use vuio_core::database::playlist_formats::PlaylistFileManager;
    use vuio_core::database::sqlite::SqliteDatabase;
    use vuio_core::database::DatabaseManager;
    use vuio_core::platform::filesystem::create_platform_filesystem_manager;
    use vuio_core::platform::PlatformInfo;
    use vuio_core::state::AppState;
    use vuio_core::web::xml::generate_browse_response;
    use vuio_core::web::{diagnostics::WebHandlerMetrics, streaming::serve_media};

    // 1. Setup temporary directory for media and database
    let temp_dir = tempdir().unwrap();
    let media_dir = temp_dir.path().join("media");
    let db_path = temp_dir.path().join("test_radio.db");

    fs::create_dir_all(&media_dir).unwrap();

    // Create radio subdirectory
    let radio_dir = media_dir.join("radio");
    fs::create_dir_all(&radio_dir).unwrap();

    // Create radio playlist file
    let m3u_content = r#"#EXTM3U
#EXTINF:-1,ABC Chill
https://cast1.asurahosting.com/proxy/julien/stream
"#;
    let m3u_path = radio_dir.join("chill.m3u");
    fs::write(&m3u_path, m3u_content).unwrap();

    // 2. Initialize SqliteDatabase
    let db = Arc::new(SqliteDatabase::new(db_path).await.unwrap());
    db.initialize().await.unwrap();

    // 3. Scan and import playlist files recursively
    let playlist_ids =
        PlaylistFileManager::scan_and_import_playlists_recursive(db.as_ref(), &media_dir)
            .await
            .unwrap();

    // Virtual radio playlists don't return standard playlist IDs (they add directly to files table)
    assert_eq!(playlist_ids.len(), 0);

    // Verify radio stream was stored in database
    let mut stream = db.stream_all_media_files();
    let mut radio_files = Vec::new();
    while let Some(res) = stream.next().await {
        let file = res.unwrap();
        if file.mime_type == "audio/radio" {
            radio_files.push(file);
        }
    }

    assert_eq!(radio_files.len(), 1);
    let radio = &radio_files[0];
    assert_eq!(radio.filename, "ABC Chill");
    assert_eq!(radio.title.as_deref().unwrap(), "ABC Chill");
    assert_eq!(
        radio.path.to_string_lossy().to_string(),
        "https://cast1.asurahosting.com/proxy/julien/stream"
    );

    // 4. Initialize AppState
    let mut app_config = AppConfig::default();
    app_config.server.port = 8099;
    app_config.media.autoplay_enabled = true;
    let config = Arc::new(app_config);

    let platform_info = Arc::new(PlatformInfo::detect().await.unwrap());
    let filesystem_manager = Arc::from(create_platform_filesystem_manager());
    let content_update_id = Arc::new(std::sync::atomic::AtomicU32::new(1));
    let web_metrics = Arc::new(WebHandlerMetrics::new());

    let app_state = AppState {
        media_directories: Arc::new(tokio::sync::RwLock::new(config.media.directories.clone())),
        unavailable_roots: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
        config: config.clone(),
        config_source: std::sync::Arc::new(vuio_core::state::ConfigSource::default()),
        http_binding: std::sync::Arc::new(vuio_core::state::HttpBinding::new(8080)),
        live_config: Arc::new(vuio_core::state::LiveConfig::new(config)),
        database: db.clone(),
        auth: Arc::new(vuio_core::web::auth::AuthState::testing()),
        platform_info,
        filesystem_manager,
        content_update_id,
        web_metrics,
        runtime_diagnostics: Arc::new(
            vuio_core::platform::diagnostics::SystemDiagnosticsSampler::new(),
        ),
        lifecycle_stats: Arc::new(vuio_core::lifecycle::ApplicationStats::new()),
        bookmarks: Arc::new(tokio::sync::Mutex::new(
            vuio_core::runtime_state::BookmarkRegistry::new(
                vuio_core::runtime_state::BOOKMARK_MAX_ENTRIES,
            ),
        )),
        log_file_path: temp_dir.path().join("vuio.log"),
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

    // 5. Test UPnP XML Browse response
    // Root container browse (ObjectID "0")
    let server_ip = app_state.get_server_ip();
    let root_containers = [
        ("video", "Video"),
        ("audio", "Music"),
        ("image", "Pictures"),
        ("radio", "Radio"),
    ]
    .map(|(path, name)| vuio_core::database::MediaDirectory {
        path: std::path::PathBuf::from(path),
        name: name.to_string(),
    });
    let root_xml = generate_browse_response(
        "0",
        &root_containers,
        &[],
        &app_state,
        &server_ip,
        root_containers.len(),
    )
    .await;
    assert!(
        root_xml.contains("id=&quot;radio&quot;"),
        "Root XML did not contain radio container: {}",
        root_xml
    );

    // Radio container browse (ObjectID "radio")
    let radio_xml = generate_browse_response(
        "radio",
        &[],
        &radio_files,
        &app_state,
        &server_ip,
        radio_files.len(),
    )
    .await;
    assert!(
        radio_xml.contains("ABC Chill"),
        "Radio XML did not contain ABC Chill stream: {}",
        radio_xml
    );
    assert!(
        radio_xml.contains("protocolInfo=&quot;http-get:*:audio/mpeg:"),
        "Radio XML did not contain correct protocolInfo: {}",
        radio_xml
    );
    assert!(
        radio_xml.contains("size=&quot;0&quot;"),
        "Radio XML did not contain size=\"0\": {}",
        radio_xml
    );
    assert!(
        !radio_xml.contains("duration="),
        "Radio XML should not contain duration: {}",
        radio_xml
    );

    // 6. Test serve_media redirection endpoint
    let file_id = radio.id.unwrap();
    let client_addr = "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap();
    let response = serve_media(
        State(app_state.clone()),
        ConnectInfo(client_addr),
        Path(file_id.to_string()),
        Method::GET,
        HeaderMap::new(),
    )
    .await
    .unwrap()
    .into_response();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT); // axum::response::Redirect::temporary returns 307 Temporary Redirect
    let location_header = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(
        location_header,
        "https://cast1.asurahosting.com/proxy/julien/stream"
    );
}

/// An ID3v2.3 tag, built by hand.
///
/// The tests need a *writer*, and symphonia only reads, so the tag bytes are
/// assembled here rather than by a second tagging library.
fn id3v2_tag(frames: &[(&[u8; 4], &str)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (id, text) in frames {
        let mut payload = vec![0u8]; // ISO-8859-1 encoding marker
        payload.extend_from_slice(text.as_bytes());
        body.extend_from_slice(*id);
        // v2.3 frame sizes are plain big-endian, unlike the synchsafe header.
        body.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        body.extend_from_slice(&[0, 0]); // flags
        body.extend_from_slice(&payload);
    }

    let size = body.len();
    let mut tag = b"ID3".to_vec();
    tag.extend_from_slice(&[3, 0, 0]); // version 2.3, no flags
    tag.extend_from_slice(&[
        ((size >> 21) & 0x7f) as u8,
        ((size >> 14) & 0x7f) as u8,
        ((size >> 7) & 0x7f) as u8,
        (size & 0x7f) as u8,
    ]);
    tag.extend_from_slice(&body);
    tag
}

/// A minimal AIFF carrying an ID3 chunk.
///
/// AIFF is the point of the exercise: it is a container the previous tag reader
/// could not open at all, so a library of these indexed with no artist, album
/// or genre — which is what "the categories are empty" on issue #11 looked like.
fn aiff_with_id3(title: &str, artist: &str, album: &str, genre: &str, year: &str) -> Vec<u8> {
    fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + payload.len() + 1);
        out.extend_from_slice(id);
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            out.push(0); // IFF chunks are word aligned
        }
        out
    }

    // COMM: channels, frame count, bits per sample, then a 10-byte extended
    // float sample rate. 0x400EAC44… is 44100 Hz.
    let mut comm = Vec::new();
    comm.extend_from_slice(&2i16.to_be_bytes());
    comm.extend_from_slice(&2u32.to_be_bytes());
    comm.extend_from_slice(&16i16.to_be_bytes());
    comm.extend_from_slice(&[0x40, 0x0e, 0xac, 0x44, 0, 0, 0, 0, 0, 0]);

    let tag = id3v2_tag(&[
        (b"TIT2", title),
        (b"TPE1", artist),
        (b"TALB", album),
        (b"TCON", genre),
        (b"TYER", year),
        (b"TRCK", "4"),
    ]);

    let mut ssnd = vec![0u8; 8]; // offset and block size
    ssnd.extend_from_slice(&[0u8; 8]); // one frame of silence

    let mut body = b"AIFF".to_vec();
    body.extend(chunk(b"COMM", &comm));
    body.extend(chunk(b"ID3 ", &tag));
    body.extend(chunk(b"SSND", &ssnd));

    let mut file = b"FORM".to_vec();
    file.extend_from_slice(&(body.len() as u32).to_be_bytes());
    file.extend_from_slice(&body);
    file
}

/// Append an APEv2 tag, the way a tagger writes one onto an existing file.
fn with_apev2_tag(mut audio: Vec<u8>, items: &[(&str, &str)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (key, value) in items {
        body.extend_from_slice(&(value.len() as u32).to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes()); // flags: UTF-8 text
        body.extend_from_slice(key.as_bytes());
        body.push(0);
        body.extend_from_slice(value.as_bytes());
    }

    audio.extend_from_slice(&body);
    audio.extend_from_slice(b"APETAGEX");
    audio.extend_from_slice(&2000u32.to_le_bytes()); // version 2
    audio.extend_from_slice(&((body.len() + 32) as u32).to_le_bytes());
    audio.extend_from_slice(&(items.len() as u32).to_le_bytes());
    audio.extend_from_slice(&0u32.to_le_bytes()); // footer only, no header
    audio.extend_from_slice(&[0u8; 8]); // reserved
    audio
}

/// The regression behind issue #11: a library in a container the old tag reader
/// could not open produced categories with nothing in them.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (scanner harness)"
)]
async fn tags_from_a_container_the_old_reader_could_not_open() {
    use vuio_core::database::{MusicCategoryFilter, MusicCategoryType};

    let temp_dir = tempdir().unwrap();
    let raw_media_dir = temp_dir.path().join("media");
    fs::create_dir_all(&raw_media_dir).unwrap();
    let media_dir = fs::canonicalize(raw_media_dir).unwrap();

    let db = Arc::new(
        SqliteDatabase::new(temp_dir.path().join("aiff.db"))
            .await
            .unwrap(),
    );
    db.initialize().await.unwrap();

    let path = media_dir.join("silence.aiff");
    fs::write(
        &path,
        aiff_with_id3("Quiet", "Aphex Twin", "Selected Ambient", "Ambient", "1992"),
    )
    .unwrap();

    let scanner = MediaScanner::with_database(db.clone());
    scanner.scan_directory_recursive(&media_dir).await.unwrap();

    let file = db.get_file_by_path(&path).await.unwrap().unwrap();
    assert_eq!(file.title.as_deref(), Some("Quiet"));
    assert_eq!(file.artist.as_deref(), Some("Aphex Twin"));
    assert_eq!(file.album.as_deref(), Some("Selected Ambient"));
    assert_eq!(file.genre.as_deref(), Some("Ambient"));
    assert_eq!(file.year, Some(1992));
    assert_eq!(file.track_number, Some(4));

    // Stream properties come off the same probe, and DIDL advertises them.
    assert_eq!(file.stream.sample_rate, Some(44_100));
    assert_eq!(file.stream.channels, Some(2));
    assert_eq!(file.stream.bits_per_sample, Some(16));

    // Stamped with the reader's version, so it is not rewritten on every scan.
    assert!(file.tags_version >= 1);

    // And the categories the issue asked for are populated, not empty.
    let names = |categories: Vec<vuio_core::database::MusicCategory>| {
        categories
            .into_iter()
            .map(|category| category.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(names(db.get_artists().await.unwrap()), ["Aphex Twin"]);
    assert_eq!(
        names(db.get_albums(None).await.unwrap()),
        ["Selected Ambient"]
    );
    assert_eq!(names(db.get_genres().await.unwrap()), ["Ambient"]);
    assert_eq!(names(db.get_years().await.unwrap()), ["1992"]);
    assert_eq!(
        names(
            db.get_music_categories(
                MusicCategoryType::Album,
                &MusicCategoryFilter::artist("Aphex Twin"),
                None,
            )
            .await
            .unwrap()
        ),
        ["Selected Ambient"]
    );
}

/// APEv2 tags sit at the end of the file, in a metadata revision of their own.
///
/// A file can carry both those and ID3 frames, so the reader drains the whole
/// metadata log rather than skipping to the newest revision, which would drop
/// whichever set came first.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (scanner harness)"
)]
async fn apev2_tags_are_read_alongside_id3() {
    let temp_dir = tempdir().unwrap();
    let raw_media_dir = temp_dir.path().join("media");
    fs::create_dir_all(&raw_media_dir).unwrap();
    let media_dir = fs::canonicalize(raw_media_dir).unwrap();

    let db = Arc::new(
        SqliteDatabase::new(temp_dir.path().join("ape.db"))
            .await
            .unwrap(),
    );
    db.initialize().await.unwrap();

    let silent_mp3_base64 = "SUQzBAAAAAAAI1RTU0UAAAAPAAADTGF2ZjU2LjM2LjEwMAAAAAAAAAAAAAAA//OEAAAAAAAAAAAAAAAAAAAAAAAASW5mbwAAAA8AAAAEAAABIADAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDV1dXV1dXV1dXV1dXV1dXV1dXV1dXV1dXV6urq6urq6urq6urq6urq6urq6urq6urq6v////////////////////////////////8AAAAATGF2YzU2LjQxAAAAAAAAAAAAAAAAJAAAAAAAAAAAASDs90hvAAAAAAAAAAAAAAAAAAAA//MUZAAAAAGkAAAAAAAAA0gAAAAATEFN//MUZAMAAAGkAAAAAAAAA0gAAAAARTMu//MUZAYAAAGkAAAAAAAAA0gAAAAAOTku//MUZAkAAAGkAAAAAAAAA0gAAAAANVVV";
    let path = media_dir.join("apetagged.mp3");
    fs::write(
        &path,
        with_apev2_tag(
            decode_base64(silent_mp3_base64),
            &[
                ("Title", "Roygbiv"),
                ("Artist", "Boards of Canada"),
                ("Album", "Music Has the Right"),
                ("Genre", "Electronic"),
                ("Year", "1998"),
                ("Track", "4"),
            ],
        ),
    )
    .unwrap();

    let scanner = MediaScanner::with_database(db.clone());
    scanner.scan_directory_recursive(&media_dir).await.unwrap();

    let file = db.get_file_by_path(&path).await.unwrap().unwrap();
    assert_eq!(file.title.as_deref(), Some("Roygbiv"));
    assert_eq!(file.artist.as_deref(), Some("Boards of Canada"));
    assert_eq!(file.album.as_deref(), Some("Music Has the Right"));
    assert_eq!(file.genre.as_deref(), Some("Electronic"));
    assert_eq!(file.year, Some(1998));
    assert_eq!(file.track_number, Some(4));

    // The ID3 frame the encoder wrote lives in a different revision of the
    // metadata log than the APE items, and both survive: the APE values reached
    // the columns asserted above, and the ID3 one reached the side table.
    let stored = db.get_media_tags(file.id.unwrap()).await.unwrap();
    assert!(
        stored
            .iter()
            .any(|(key, value)| key == "Encoder" && value.starts_with("Lavf")),
        "the ID3 revision must not be dropped in favour of the APE one: {stored:?}"
    );

    // A tag with a column of its own is not repeated in the side table.
    assert!(
        !stored.iter().any(|(key, _)| key == "Artist"),
        "promoted tags belong in their column, not both places: {stored:?}"
    );
}

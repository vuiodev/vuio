use audiotags::Tag;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

use vuio::database::playlist_formats::PlaylistFileManager;
use vuio::database::redb::RedbDatabase;
use vuio::database::DatabaseManager;
use vuio::media::MediaScanner;

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
async fn test_audio_implementation_and_features() {
    // 1. Setup temporary directory for media and database
    let temp_dir = tempdir().unwrap();
    let raw_media_dir = temp_dir.path().join("media");
    fs::create_dir_all(&raw_media_dir).unwrap();
    let media_dir = fs::canonicalize(raw_media_dir).unwrap();

    let db_path = temp_dir.path().join("test_media.redb");
    let db = Arc::new(RedbDatabase::new(db_path).await.unwrap());
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
    assert_eq!(scan_result.new_files.len(), 4);

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
async fn test_cover_art_retrieval_and_xml() {
    use axum::extract::{Path, State};
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use std::fs;
    use std::sync::Arc;
    use tempfile::tempdir;
    use vuio::config::AppConfig;
    use vuio::database::redb::RedbDatabase;
    use vuio::database::DatabaseManager;
    use vuio::media::MediaScanner;
    use vuio::platform::filesystem::create_platform_filesystem_manager;
    use vuio::platform::PlatformInfo;
    use vuio::state::AppState;
    use vuio::web::handlers::{serve_cover, WebHandlerMetrics};
    use vuio::web::xml::generate_browse_response;

    // 1. Setup temporary directory for media and database
    let temp_dir = tempdir().unwrap();
    let raw_media_dir = temp_dir.path().join("media");
    fs::create_dir_all(&raw_media_dir).unwrap();
    let media_dir = fs::canonicalize(raw_media_dir).unwrap();

    let db_path = temp_dir.path().join("test_cover.redb");
    let db = Arc::new(RedbDatabase::new(db_path).await.unwrap());
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
    assert_eq!(scan_result.new_files.len(), 2);

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
        config,
        database: db.clone(),
        platform_info,
        filesystem_manager,
        content_update_id,
        web_metrics,
        bookmarks: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        log_file_path: temp_dir.path().join("vuio.log"),
        browse_cache: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        mcp_clients: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_monitors: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_casts: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        discovered_tvs: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        upnp_subscriptions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
    };

    // 7. Verify UPnP XML response generation contains upnp:albumArtURI
    let xml_response =
        generate_browse_response("audio", &[], &[db_file.clone()], &app_state, "127.0.0.1").await;

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
async fn test_radio_playlist_import() {
    use axum::extract::{ConnectInfo, Path, State};
    use axum::http::StatusCode;
    use axum::http::{HeaderMap, Method};
    use axum::response::IntoResponse;
    use futures_util::StreamExt;
    use std::fs;
    use std::sync::Arc;
    use tempfile::tempdir;
    use vuio::config::AppConfig;
    use vuio::database::playlist_formats::PlaylistFileManager;
    use vuio::database::redb::RedbDatabase;
    use vuio::database::DatabaseManager;
    use vuio::platform::filesystem::create_platform_filesystem_manager;
    use vuio::platform::PlatformInfo;
    use vuio::state::AppState;
    use vuio::web::handlers::{serve_media, WebHandlerMetrics};
    use vuio::web::xml::generate_browse_response;

    // 1. Setup temporary directory for media and database
    let temp_dir = tempdir().unwrap();
    let media_dir = temp_dir.path().join("media");
    let db_path = temp_dir.path().join("test_radio.redb");

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

    // 2. Initialize RedbDatabase
    let db = Arc::new(RedbDatabase::new(db_path).await.unwrap());
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
        config,
        database: db.clone(),
        platform_info,
        filesystem_manager,
        content_update_id,
        web_metrics,
        bookmarks: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        log_file_path: temp_dir.path().join("vuio.log"),
        browse_cache: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        mcp_clients: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_monitors: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_casts: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        discovered_tvs: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        upnp_subscriptions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
    };

    // 5. Test UPnP XML Browse response
    // Root container browse (ObjectID "0")
    let server_ip = app_state.get_server_ip();
    let root_xml = generate_browse_response("0", &[], &[], &app_state, &server_ip).await;
    assert!(
        root_xml.contains("id=&quot;radio&quot;"),
        "Root XML did not contain radio container: {}",
        root_xml
    );

    // Radio container browse (ObjectID "radio")
    let radio_xml =
        generate_browse_response("radio", &[], &radio_files, &app_state, &server_ip).await;
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

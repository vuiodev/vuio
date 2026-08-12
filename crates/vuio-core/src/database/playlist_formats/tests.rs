use super::*;
use crate::database::{MediaRepository, PlaylistRepository};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_format_detection() {
    assert_eq!(
        PlaylistFormat::from_extension(Path::new("test.m3u")),
        Some(PlaylistFormat::M3U)
    );
    assert_eq!(
        PlaylistFormat::from_extension(Path::new("test.M3U")),
        Some(PlaylistFormat::M3U)
    );
    assert_eq!(
        PlaylistFormat::from_extension(Path::new("test.m3u8")),
        Some(PlaylistFormat::M3U)
    );
    assert_eq!(
        PlaylistFormat::from_extension(Path::new("test.pls")),
        Some(PlaylistFormat::PLS)
    );
    assert_eq!(
        PlaylistFormat::from_extension(Path::new("test.PLS")),
        Some(PlaylistFormat::PLS)
    );
    assert_eq!(PlaylistFormat::from_extension(Path::new("test.txt")), None);
}

#[test]
fn test_output_filename_generation() {
    assert_eq!(
        PlaylistFileManager::get_output_filename("My Playlist", PlaylistFormat::M3U),
        "My Playlist.m3u"
    );
    assert_eq!(
        PlaylistFileManager::get_output_filename("Rock/Metal Mix", PlaylistFormat::PLS),
        "Rock_Metal Mix.pls"
    );
    assert_eq!(
        PlaylistFileManager::get_output_filename("Test<>Playlist", PlaylistFormat::M3U),
        "Test__Playlist.m3u"
    );
}

#[test]
fn test_resolve_playlist_path() {
    let base_dir = Path::new("/media/music");

    // Absolute path
    let resolved = resolve_playlist_path(base_dir, "/other/track.mp3");
    assert_eq!(resolved, PathBuf::from("/other/track.mp3"));

    // Relative path
    let resolved = resolve_playlist_path(base_dir, "album/track.mp3");
    assert_eq!(resolved, PathBuf::from("/media/music/album/track.mp3"));

    // Relative path with parent directory (..)
    let resolved = resolve_playlist_path(base_dir, "../other/track.mp3");
    assert_eq!(resolved, PathBuf::from("/media/other/track.mp3"));

    // Windows-style backslashes replaced with forward slashes
    let resolved = resolve_playlist_path(base_dir, r"album\track.mp3");
    assert_eq!(resolved, PathBuf::from("/media/music/album/track.mp3"));
}

#[test]
fn test_stream_entries_are_not_resolved_as_filesystem_paths() {
    let base_dir = Path::new("/Users/alex/Downloads/radio");
    let url = "https://cast1.asurahosting.com/proxy/julien/stream";

    assert_eq!(resolve_playlist_entry(base_dir, url), url);
    assert!(is_radio_playlist_path(Path::new(
        "/Users/alex/Downloads/radio/stations.m3u"
    )));
}

#[tokio::test]
async fn test_generic_playlist_import_materializes_http_stream() {
    use crate::database::sqlite::SqliteDatabase;
    use crate::database::DatabaseManager;
    use tempfile::tempdir;

    let temp = tempdir().unwrap();
    let playlist_path = temp.path().join("stations.m3u");
    let url = "https://cast1.asurahosting.com/proxy/julien/stream";
    fs::write(&playlist_path, format!("#EXTM3U\n{url}\n")).unwrap();

    let database = SqliteDatabase::new(temp.path().join("playlist.db"))
        .await
        .unwrap();
    database.initialize().await.unwrap();
    let playlist_id = PlaylistFileManager::import_playlist(&database, &playlist_path, None)
        .await
        .unwrap();

    let tracks = database.get_playlist_tracks(playlist_id).await.unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].path, PathBuf::from(url));
    assert_eq!(tracks[0].mime_type, "audio/radio");
    assert!(database
        .get_file_by_path(Path::new(
            "/Users/alex/Downloads/radio/https:/cast1.asurahosting.com/proxy/julien/stream"
        ))
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn test_non_recursive_radio_root_uses_radio_importer() {
    use crate::database::sqlite::SqliteDatabase;
    use crate::database::DatabaseManager;
    use tempfile::tempdir;

    let temp = tempdir().unwrap();
    let radio_dir = temp.path().join("radio");
    fs::create_dir(&radio_dir).unwrap();
    let playlist_path = radio_dir.join("stations.m3u");
    let url = "https://radio.example/stream";
    fs::write(
        &playlist_path,
        format!("#EXTM3U\n#EXTINF:-1,Example Radio\n{url}\n"),
    )
    .unwrap();

    let database = SqliteDatabase::new(temp.path().join("radio.db"))
        .await
        .unwrap();
    database.initialize().await.unwrap();
    PlaylistFileManager::scan_and_import_playlists(&database, &radio_dir)
        .await
        .unwrap();

    let station = database
        .get_file_by_path(Path::new(url))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(station.filename, "Example Radio");
    assert_eq!(station.mime_type, "audio/radio");
}

#[test]
fn test_m3u_parsing() {
    let m3u_content = r#"#EXTM3U
#EXTINF:123,Artist Name - Song Title
/path/to/song1.mp3
#EXTINF:456,Another Artist - Another Song
/path/to/song2.mp3
/path/to/song3.mp3
"#;

    // We can't test the full import without a database, but we can test parsing logic
    // This would be expanded in a real test with a mock database
    let lines: Vec<&str> = m3u_content.lines().collect();
    assert!(lines[0] == "#EXTM3U");
    assert!(lines[1].starts_with("#EXTINF"));
    assert!(lines[2] == "/path/to/song1.mp3");
}

#[test]
fn test_pls_parsing() {
    let pls_content = r#"[playlist]
NumberOfEntries=2

File1=/path/to/song1.mp3
Title1=Artist Name - Song Title
Length1=123

File2=/path/to/song2.mp3
Title2=Another Artist - Another Song
Length2=456

Version=2
"#;

    // Basic parsing test
    let lines: Vec<&str> = pls_content.lines().collect();
    assert!(lines[0] == "[playlist]");

    let file_lines: Vec<&str> = lines
        .iter()
        .filter(|line| line.starts_with("File"))
        .cloned()
        .collect();
    assert_eq!(file_lines.len(), 2);
}

#[tokio::test]
async fn test_m3u_export() {
    let playlist = Playlist {
        id: Some(1),
        name: "Test Playlist".to_string(),
        description: Some("Test Description".to_string()),
        created_at: std::time::SystemTime::now(),
        updated_at: std::time::SystemTime::now(),
    };

    let tracks = vec![MediaFile {
        id: Some(1),
        path: PathBuf::from("/test/song1.mp3"),
        filename: "song1.mp3".to_string(),
        size: 1000,
        modified: std::time::SystemTime::now(),
        mime_type: "audio/mpeg".to_string(),
        duration: Some(std::time::Duration::from_secs(180)),
        title: Some("Test Song 1".to_string()),
        artist: Some("Test Artist".to_string()),
        album: Some("Test Album".to_string()),
        genre: Some("Rock".to_string()),
        track_number: Some(1),
        year: Some(2023),
        album_artist: Some("Test Artist".to_string()),
        subtitle_available: false,
        created_at: std::time::SystemTime::now(),
        updated_at: std::time::SystemTime::now(),
    }];

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"").unwrap(); // Ensure file exists

    let result = PlaylistFileManager::export_m3u(&playlist, &tracks, temp_file.path()).await;
    assert!(result.is_ok());

    let content = fs::read_to_string(temp_file.path()).unwrap();
    assert!(content.contains("#EXTM3U"));
    assert!(content.contains("#EXTINF:180,Test Artist - Test Song 1"));
    assert!(content.contains("/test/song1.mp3"));
}

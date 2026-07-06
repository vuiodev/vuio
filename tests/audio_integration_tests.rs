use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use audiotags::Tag;

use vuio::database::DatabaseManager;
use vuio::database::redb::RedbDatabase;
use vuio::database::playlist_formats::PlaylistFileManager;
use vuio::media::MediaScanner;

fn decode_base64(s: &str) -> Vec<u8> {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bytes = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    
    for c in s.chars() {
        if c == '=' { break; }
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
    assert_eq!(back_in_black_tracks[0].title.as_deref(), Some("Back In Black"));

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
    let year_values: Vec<u32> = years.iter().map(|c| c.name.parse::<u32>().unwrap()).collect();
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
    let playlist_ids = PlaylistFileManager::scan_and_import_playlists_recursive(db.as_ref(), &media_dir).await.unwrap();
    assert_eq!(playlist_ids.len(), 2);

    let playlists = db.get_playlists().await.unwrap();
    assert_eq!(playlists.len(), 2);
    
    let playlist_names: Vec<&str> = playlists.iter().map(|p| p.name.as_str()).collect();
    assert!(playlist_names.contains(&"favorites"));
    assert!(playlist_names.contains(&"rock"));

    // Verify favorites tracks
    let favorites_id = playlists.iter().find(|p| p.name == "favorites").unwrap().id.unwrap();
    let favorites_tracks = db.get_playlist_tracks(favorites_id).await.unwrap();
    assert_eq!(favorites_tracks.len(), 3);
    let fav_titles: Vec<&str> = favorites_tracks.iter().map(|t| t.title.as_deref().unwrap()).collect();
    assert!(fav_titles.contains(&"Back In Black"));
    assert!(fav_titles.contains(&"Enter Sandman"));
    assert!(fav_titles.contains(&"Stairway to Heaven"));

    // Verify rock tracks
    let rock_id = playlists.iter().find(|p| p.name == "rock").unwrap().id.unwrap();
    let rock_tracks = db.get_playlist_tracks(rock_id).await.unwrap();
    assert_eq!(rock_tracks.len(), 2);
    let rock_titles: Vec<&str> = rock_tracks.iter().map(|t| t.title.as_deref().unwrap()).collect();
    assert!(rock_titles.contains(&"Back In Black"));
    assert!(rock_titles.contains(&"Time"));
}

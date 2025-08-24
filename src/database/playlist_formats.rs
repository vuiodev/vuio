use anyhow::{anyhow, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

use crate::database::{DatabaseManager, MediaFile, Playlist};

/// Supported playlist file formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistFormat {
    M3U,
    PLS,
}

impl PlaylistFormat {
    /// Get the file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            PlaylistFormat::M3U => "m3u",
            PlaylistFormat::PLS => "pls",
        }
    }

    /// Detect format from file extension
    pub fn from_extension(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_lowercase().as_str() {
            "m3u" | "m3u8" => Some(PlaylistFormat::M3U),
            "pls" => Some(PlaylistFormat::PLS),
            _ => None,
        }
    }
}

/// Playlist file import/export functionality
pub struct PlaylistFileManager;

impl PlaylistFileManager {
    /// Import a playlist from a file
    pub async fn import_playlist<D: DatabaseManager + ?Sized>(
        database: &D,
        file_path: &Path,
        playlist_name: Option<String>,
    ) -> Result<i64> {
        let format = PlaylistFormat::from_extension(file_path)
            .ok_or_else(|| anyhow!("Unsupported playlist format for file: {}", file_path.display()))?;

        let file_content = fs::read_to_string(file_path)?;
        let playlist_name = playlist_name.unwrap_or_else(|| {
            file_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });

        match format {
            PlaylistFormat::M3U => Self::import_m3u(database, &file_content, &playlist_name).await,
            PlaylistFormat::PLS => Self::import_pls(database, &file_content, &playlist_name).await,
        }
    }

    /// Export a playlist to a file
    pub async fn export_playlist<D: DatabaseManager + ?Sized>(
        database: &D,
        playlist_id: i64,
        output_path: &Path,
        format: PlaylistFormat,
    ) -> Result<()> {
        let playlist = database
            .get_playlist(playlist_id)
            .await?
            .ok_or_else(|| anyhow!("Playlist with ID {} not found", playlist_id))?;

        let tracks = database.get_playlist_tracks(playlist_id).await?;

        match format {
            PlaylistFormat::M3U => Self::export_m3u(&playlist, &tracks, output_path).await,
            PlaylistFormat::PLS => Self::export_pls(&playlist, &tracks, output_path).await,
        }
    }

    /// Import M3U playlist format
    async fn import_m3u<D: DatabaseManager + ?Sized>(
        database: &D,
        content: &str,
        playlist_name: &str,
    ) -> Result<i64> {
        debug!("Importing M3U playlist: {}", playlist_name);

        // Create the playlist
        let playlist_id = database.create_playlist(playlist_name, None).await?;

        let mut position = 0u32;
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Skip empty lines and comments (except #EXTINF)
            if line.is_empty() || (line.starts_with('#') && !line.starts_with("#EXTINF")) {
                i += 1;
                continue;
            }

            // Handle extended M3U format
            if line.starts_with("#EXTINF") {
                // Next line should be the file path
                i += 1;
                if i < lines.len() {
                    let file_path_str = lines[i].trim();
                    if let Err(e) = Self::add_track_to_playlist(
                        database,
                        playlist_id,
                        file_path_str,
                        position,
                    )
                    .await
                    {
                        warn!("Failed to add track to playlist: {}", e);
                    } else {
                        position += 1;
                    }
                }
            } else if !line.starts_with('#') {
                // Simple M3U format - just file paths
                if let Err(e) = Self::add_track_to_playlist(database, playlist_id, line, position).await {
                    warn!("Failed to add track to playlist: {}", e);
                } else {
                    position += 1;
                }
            }

            i += 1;
        }

        debug!("Imported {} tracks to playlist '{}'", position, playlist_name);
        Ok(playlist_id)
    }

    /// Import PLS playlist format
    async fn import_pls<D: DatabaseManager + ?Sized>(
        database: &D,
        content: &str,
        playlist_name: &str,
    ) -> Result<i64> {
        debug!("Importing PLS playlist: {}", playlist_name);

        // Create the playlist
        let playlist_id = database.create_playlist(playlist_name, None).await?;

        let mut tracks: Vec<(u32, String)> = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("File") {
                if let Some(eq_pos) = line.find('=') {
                    let (key, value) = line.split_at(eq_pos);
                    let value = &value[1..]; // Skip the '='

                    // Extract the number from "File1", "File2", etc.
                    if let Ok(track_num) = key[4..].parse::<u32>() {
                        tracks.push((track_num, value.to_string()));
                    }
                }
            }
        }

        // Sort tracks by number to maintain order
        tracks.sort_by_key(|(num, _)| *num);

        // Add tracks to playlist
        for (i, (_, file_path)) in tracks.into_iter().enumerate() {
            if let Err(e) = Self::add_track_to_playlist(database, playlist_id, &file_path, i as u32).await {
                warn!("Failed to add track to playlist: {}", e);
            }
        }

        debug!("Imported playlist '{}'", playlist_name);
        Ok(playlist_id)
    }

    /// Export playlist to M3U format
    async fn export_m3u(
        playlist: &Playlist,
        tracks: &[MediaFile],
        output_path: &Path,
    ) -> Result<()> {
        debug!("Exporting playlist '{}' to M3U format: {}", playlist.name, output_path.display());

        let mut file = fs::File::create(output_path)?;
        
        // Write M3U header
        writeln!(file, "#EXTM3U")?;
        
        for track in tracks {
            // Write extended info if available
            let duration = track.duration
                .map(|d| d.as_secs() as i32)
                .unwrap_or(-1);
            
            let title = track.title.as_deref()
                .or(track.filename.strip_suffix(&format!(".{}", 
                    track.path.extension()
                        .and_then(|ext| ext.to_str())
                        .unwrap_or(""))))
                .unwrap_or(&track.filename);
            
            let artist = track.artist.as_deref().unwrap_or("Unknown Artist");
            
            writeln!(file, "#EXTINF:{},{} - {}", duration, artist, title)?;
            writeln!(file, "{}", track.path.display())?;
        }

        debug!("Successfully exported {} tracks to M3U", tracks.len());
        Ok(())
    }

    /// Export playlist to PLS format
    async fn export_pls(
        playlist: &Playlist,
        tracks: &[MediaFile],
        output_path: &Path,
    ) -> Result<()> {
        debug!("Exporting playlist '{}' to PLS format: {}", playlist.name, output_path.display());

        let mut file = fs::File::create(output_path)?;
        
        // Write PLS header
        writeln!(file, "[playlist]")?;
        writeln!(file, "NumberOfEntries={}", tracks.len())?;
        writeln!(file)?;
        
        for (i, track) in tracks.iter().enumerate() {
            let track_num = i + 1;
            
            writeln!(file, "File{}={}", track_num, track.path.display())?;
            
            if let Some(ref title) = track.title {
                let artist = track.artist.as_deref().unwrap_or("Unknown Artist");
                writeln!(file, "Title{}={} - {}", track_num, artist, title)?;
            } else {
                writeln!(file, "Title{}={}", track_num, track.filename)?;
            }
            
            if let Some(duration) = track.duration {
                writeln!(file, "Length{}={}", track_num, duration.as_secs())?;
            } else {
                writeln!(file, "Length{}=-1", track_num)?;
            }
            
            writeln!(file)?;
        }
        
        writeln!(file, "Version=2")?;

        debug!("Successfully exported {} tracks to PLS", tracks.len());
        Ok(())
    }

    /// Add a track to playlist by file path
    async fn add_track_to_playlist<D: DatabaseManager + ?Sized>(
        database: &D,
        playlist_id: i64,
        file_path_str: &str,
        position: u32,
    ) -> Result<()> {
        let file_path = PathBuf::from(file_path_str);
        
        // Try to find the file in the database
        match database.get_file_by_path(&file_path).await? {
            Some(media_file) => {
                if let Some(file_id) = media_file.id {
                    database.add_to_playlist(playlist_id, file_id, Some(position)).await?;
                    debug!("Added track to playlist: {}", file_path.display());
                } else {
                    warn!("Media file found but has no ID: {}", file_path.display());
                }
            }
            None => {
                warn!("File not found in media database: {}", file_path.display());
                // Optionally, we could add the file to the database here if it exists on disk
            }
        }
        
        Ok(())
    }

    /// Scan a directory for playlist files and import them
    pub async fn scan_and_import_playlists<D: DatabaseManager + ?Sized>(
        database: &D,
        directory: &Path,
    ) -> Result<Vec<i64>> {
        debug!("Scanning directory for playlist files: {}", directory.display());
        
        let mut imported_playlists = Vec::new();
        
        if !directory.is_dir() {
            return Err(anyhow!("Path is not a directory: {}", directory.display()));
        }
        
        let entries = fs::read_dir(directory)?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                if let Some(_format) = PlaylistFormat::from_extension(&path) {
                    match Self::import_playlist(database, &path, None).await {
                        Ok(playlist_id) => {
                            debug!("Successfully imported playlist: {}", path.display());
                            imported_playlists.push(playlist_id);
                        }
                        Err(e) => {
                            warn!("Failed to import playlist {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }
        
        debug!("Imported {} playlists from directory", imported_playlists.len());
        Ok(imported_playlists)
    }

    /// Get the appropriate file extension for a playlist export
    pub fn get_output_filename(playlist_name: &str, format: PlaylistFormat) -> String {
        // Sanitize the playlist name for use as filename
        let safe_name = playlist_name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
            .collect::<String>()
            .trim()
            .replace("  ", " ");
        
        format!("{}.{}", safe_name, format.extension())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_format_detection() {
        assert_eq!(PlaylistFormat::from_extension(Path::new("test.m3u")), Some(PlaylistFormat::M3U));
        assert_eq!(PlaylistFormat::from_extension(Path::new("test.M3U")), Some(PlaylistFormat::M3U));
        assert_eq!(PlaylistFormat::from_extension(Path::new("test.m3u8")), Some(PlaylistFormat::M3U));
        assert_eq!(PlaylistFormat::from_extension(Path::new("test.pls")), Some(PlaylistFormat::PLS));
        assert_eq!(PlaylistFormat::from_extension(Path::new("test.PLS")), Some(PlaylistFormat::PLS));
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
        
        let file_lines: Vec<&str> = lines.iter()
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

        let tracks = vec![
            MediaFile {
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
                created_at: std::time::SystemTime::now(),
                updated_at: std::time::SystemTime::now(),
            }
        ];

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"").unwrap(); // Ensure file exists
        
        let result = PlaylistFileManager::export_m3u(&playlist, &tracks, temp_file.path()).await;
        assert!(result.is_ok());

        let content = fs::read_to_string(temp_file.path()).unwrap();
        assert!(content.contains("#EXTM3U"));
        assert!(content.contains("#EXTINF:180,Test Artist - Test Song 1"));
        assert!(content.contains("/test/song1.mp3"));
    }
}
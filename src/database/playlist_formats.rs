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
        let format = PlaylistFormat::from_extension(file_path).ok_or_else(|| {
            anyhow!(
                "Unsupported playlist format for file: {}",
                file_path.display()
            )
        })?;

        let file_content = fs::read_to_string(file_path)?;
        let playlist_name = playlist_name.unwrap_or_else(|| {
            file_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });

        let base_dir = file_path.parent().unwrap_or_else(|| Path::new(""));
        let source_path = crate::platform::filesystem::create_platform_path_normalizer()
            .to_canonical(file_path)
            .unwrap_or_else(|_| file_path.to_string_lossy().to_string());

        let playlist_id = match format {
            PlaylistFormat::M3U => {
                Self::import_m3u(
                    database,
                    &file_content,
                    &playlist_name,
                    base_dir,
                    &source_path,
                )
                .await
            }
            PlaylistFormat::PLS => {
                Self::import_pls(
                    database,
                    &file_content,
                    &playlist_name,
                    base_dir,
                    &source_path,
                )
                .await
            }
        }?;
        Ok(playlist_id)
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
        base_dir: &Path,
        source_path: &str,
    ) -> Result<i64> {
        debug!("Importing M3U playlist: {}", playlist_name);

        // Collect all track paths first
        let mut track_paths = Vec::new();
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
                    track_paths.push(resolve_playlist_entry(base_dir, file_path_str));
                }
            } else if !line.starts_with('#') {
                // Simple M3U format - just file paths
                track_paths.push(resolve_playlist_entry(base_dir, line));
            }

            i += 1;
        }

        // Create list of (path, position) pairs
        let file_paths_with_positions: Vec<(String, u32)> = track_paths
            .into_iter()
            .enumerate()
            .map(|(index, path)| (path, index as u32))
            .collect();

        // Add all tracks to playlist in batch operation
        let media_entries =
            Self::resolve_playlist_tracks(database, &file_paths_with_positions, Some(source_path))
                .await?;
        let playlist_id = database
            .replace_playlist_from_source(Path::new(source_path), playlist_name, &media_entries)
            .await?;

        debug!(
            "Imported {} tracks to playlist '{}'",
            media_entries.len(),
            playlist_name
        );
        Ok(playlist_id)
    }

    /// Import PLS playlist format
    async fn import_pls<D: DatabaseManager + ?Sized>(
        database: &D,
        content: &str,
        playlist_name: &str,
        base_dir: &Path,
        source_path: &str,
    ) -> Result<i64> {
        debug!("Importing PLS playlist: {}", playlist_name);

        let mut tracks: Vec<(u32, String)> = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("File") {
                if let Some(eq_pos) = line.find('=') {
                    let (key, value) = line.split_at(eq_pos);
                    let value = &value[1..]; // Skip the '='

                    // Extract the number from "File1", "File2", etc.
                    if let Ok(track_num) = key[4..].parse::<u32>() {
                        tracks.push((track_num, resolve_playlist_entry(base_dir, value.trim())));
                    }
                }
            }
        }

        // Sort tracks by number to maintain order
        tracks.sort_by_key(|(num, _)| *num);

        // Create list of (path, position) pairs for batch operation
        let file_paths_with_positions: Vec<(String, u32)> = tracks
            .into_iter()
            .enumerate()
            .map(|(index, (_, file_path))| (file_path, index as u32))
            .collect();

        // Add all tracks to playlist in batch operation
        let media_entries =
            Self::resolve_playlist_tracks(database, &file_paths_with_positions, Some(source_path))
                .await?;
        let playlist_id = database
            .replace_playlist_from_source(Path::new(source_path), playlist_name, &media_entries)
            .await?;

        debug!(
            "Imported {} tracks to playlist '{}'",
            media_entries.len(),
            playlist_name
        );
        Ok(playlist_id)
    }

    /// Export playlist to M3U format
    async fn export_m3u(
        playlist: &Playlist,
        tracks: &[MediaFile],
        output_path: &Path,
    ) -> Result<()> {
        debug!(
            "Exporting playlist '{}' to M3U format: {}",
            playlist.name,
            output_path.display()
        );

        let mut file = fs::File::create(output_path)?;

        // Write M3U header
        writeln!(file, "#EXTM3U")?;

        for track in tracks {
            // Write extended info if available
            let duration = track.duration.map(|d| d.as_secs() as i32).unwrap_or(-1);

            let title = track
                .title
                .as_deref()
                .or(track.filename.strip_suffix(&format!(
                    ".{}",
                    track
                        .path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .unwrap_or("")
                )))
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
        debug!(
            "Exporting playlist '{}' to PLS format: {}",
            playlist.name,
            output_path.display()
        );

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

    /// Add tracks to playlist by file paths using batch operations
    async fn resolve_playlist_tracks<D: DatabaseManager + ?Sized>(
        database: &D,
        file_paths_with_positions: &[(String, u32)],
        source_path: Option<&str>,
    ) -> Result<Vec<(i64, u32)>> {
        if file_paths_with_positions.is_empty() {
            return Ok(Vec::new());
        }

        // Extract paths for batch query
        let paths: Vec<PathBuf> = file_paths_with_positions
            .iter()
            .map(|(path_str, _)| PathBuf::from(path_str))
            .collect();

        // Get all media files in a single query
        let media_files = database.get_files_by_paths(&paths).await?;

        // Create a map from path to media file for quick lookup
        let mut path_to_file = std::collections::HashMap::new();
        for media_file in media_files {
            if let Some(file_id) = media_file.id {
                path_to_file.insert(media_file.path.clone(), file_id);
            }
        }

        // Streams are valid playlist entries even though they are not present on
        // the local filesystem. Materialize missing HTTP(S) entries as virtual
        // radio files before building the playlist entry batch.
        for (location, _) in file_paths_with_positions {
            if !is_http_stream(location) {
                continue;
            }
            let path = PathBuf::from(location);
            if path_to_file.contains_key(&path) {
                continue;
            }

            let mut stream = MediaFile::new(path.clone(), 0, "audio/radio".to_string());
            stream.title = Some(location.clone());
            stream.album = source_path.map(str::to_owned);
            let file_id = database.store_media_file(&stream).await?;
            path_to_file.insert(path, file_id);
        }

        // Build list of (media_file_id, position) pairs for files that exist in database
        let mut media_file_entries = Vec::new();

        for (file_path_str, position) in file_paths_with_positions {
            let file_path = PathBuf::from(file_path_str);

            if let Some(&file_id) = path_to_file.get(&file_path) {
                media_file_entries.push((file_id, *position));
            } else {
                warn!("File not found in media database: {}", file_path.display());
            }
        }

        Ok(media_file_entries)
    }

    /// Scan a directory for playlist files and import them
    pub async fn scan_and_import_playlists<D: DatabaseManager + ?Sized>(
        database: &D,
        directory: &Path,
    ) -> Result<Vec<i64>> {
        debug!(
            "Scanning directory for playlist files: {}",
            directory.display()
        );

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
                    if is_radio_playlist_path(&path) {
                        if let Err(e) = Self::import_radio_playlist(database, &path).await {
                            warn!("Failed to import radio playlist {}: {}", path.display(), e);
                        }
                    } else {
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
        }

        debug!(
            "Imported {} playlists from directory",
            imported_playlists.len()
        );
        Ok(imported_playlists)
    }

    /// Recursively scan a directory tree for playlist files and import them
    pub async fn scan_and_import_playlists_recursive<D: DatabaseManager + ?Sized>(
        database: &D,
        directory: &Path,
    ) -> Result<Vec<i64>> {
        use tracing::info;

        info!(
            "Recursively scanning for playlist files: {}",
            directory.display()
        );

        let mut imported_playlists = Vec::new();
        let mut dirs_to_scan = vec![directory.to_path_buf()];

        while let Some(current_dir) = dirs_to_scan.pop() {
            if !current_dir.is_dir() {
                continue;
            }

            let mut entries = match tokio::fs::read_dir(&current_dir).await {
                Ok(entries) => entries,
                Err(e) => {
                    warn!("Failed to read directory {}: {}", current_dir.display(), e);
                    continue;
                }
            };

            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();

                if path.is_dir() {
                    // Skip hidden directories
                    if path
                        .file_name()
                        .is_some_and(|name| !name.to_string_lossy().starts_with('.'))
                    {
                        dirs_to_scan.push(path);
                    }
                } else if path.is_file() && PlaylistFormat::from_extension(&path).is_some() {
                    if is_radio_playlist_path(&path) {
                        if let Err(e) = Self::import_radio_playlist(database, &path).await {
                            warn!("Failed to import radio playlist {}: {}", path.display(), e);
                        }
                    } else {
                        match Self::import_playlist(database, &path, None).await {
                            Ok(playlist_id) => {
                                debug!("Imported playlist: {}", path.display());
                                imported_playlists.push(playlist_id);
                            }
                            Err(e) => {
                                warn!("Failed to import playlist {}: {}", path.display(), e);
                            }
                        }
                    }
                }
            }
        }

        info!(
            "Imported {} playlists from directory tree",
            imported_playlists.len()
        );
        Ok(imported_playlists)
    }

    /// Import a radio playlist (e.g. under /radio/ folder) and index its streams as virtual files.
    pub async fn import_radio_playlist<D: DatabaseManager + ?Sized>(
        database: &D,
        file_path: &Path,
    ) -> Result<()> {
        use futures_util::StreamExt;

        let format = PlaylistFormat::from_extension(file_path).ok_or_else(|| {
            anyhow!(
                "Unsupported playlist format for file: {}",
                file_path.display()
            )
        })?;

        let file_content = fs::read_to_string(file_path)?;
        let playlist_path_str = crate::platform::filesystem::create_platform_path_normalizer()
            .to_canonical(file_path)
            .unwrap_or_else(|_| file_path.to_string_lossy().to_string());

        let mut stations = Vec::new();
        match format {
            PlaylistFormat::M3U => {
                let lines: Vec<&str> = file_content.lines().collect();
                let mut i = 0;
                while i < lines.len() {
                    let line = lines[i].trim();
                    if line.starts_with("#EXTINF") {
                        let name = if let Some(comma_pos) = line.find(',') {
                            line[comma_pos + 1..].trim().to_string()
                        } else {
                            "Unknown Radio".to_string()
                        };

                        i += 1;
                        if i < lines.len() {
                            let url = lines[i].trim().to_string();
                            if is_http_stream(&url) {
                                stations.push((name, url));
                            }
                        }
                    } else if !line.starts_with('#') && !line.is_empty() && is_http_stream(line) {
                        stations.push((line.to_string(), line.to_string()));
                    }
                    i += 1;
                }
            }
            PlaylistFormat::PLS => {
                let mut urls = std::collections::HashMap::new();
                let mut titles = std::collections::HashMap::new();

                for line in file_content.lines() {
                    let line = line.trim();
                    if line.starts_with("File") {
                        if let Some(eq_pos) = line.find('=') {
                            if let Ok(num) = line[4..eq_pos].parse::<u32>() {
                                let val = line[eq_pos + 1..].trim().to_string();
                                if is_http_stream(&val) {
                                    urls.insert(num, val);
                                }
                            }
                        }
                    } else if line.starts_with("Title") {
                        if let Some(eq_pos) = line.find('=') {
                            if let Ok(num) = line[5..eq_pos].parse::<u32>() {
                                let val = line[eq_pos + 1..].trim().to_string();
                                titles.insert(num, val);
                            }
                        }
                    }
                }

                for (num, url) in urls {
                    let name = titles.remove(&num).unwrap_or_else(|| url.clone());
                    stations.push((name, url));
                }
            }
        }

        // Discover old derived records, propagating storage corruption rather
        // than silently committing a partial replacement.
        let mut old_paths = Vec::new();
        let mut stream = database.stream_all_media_files();
        while let Some(res) = stream.next().await {
            let file = res?;
            if file.mime_type == "audio/radio" && file.album.as_deref() == Some(&playlist_path_str)
            {
                old_paths.push(file.path.clone());
            }
        }
        drop(stream);

        // Materialize the complete new set first and commit it in one database
        // transaction. Stale records are removed only after that succeeds.
        let mut new_files = Vec::with_capacity(stations.len());
        let mut new_paths = std::collections::HashSet::with_capacity(stations.len());
        for (name, url) in stations {
            let path = PathBuf::from(&url);
            new_paths.insert(path.clone());
            if let Some(existing_file) = database.get_file_by_path(&path).await? {
                let mut file = existing_file;
                file.filename = name.clone();
                file.title = Some(name);
                file.album = Some(playlist_path_str.clone());
                new_files.push(file);
            } else {
                let mut file = MediaFile::new(path, 0, "audio/radio".to_string());
                file.filename = name.clone();
                file.title = Some(name);
                file.album = Some(playlist_path_str.clone());
                new_files.push(file);
            }
        }
        database.bulk_store_media_files(&new_files).await?;
        let stale_paths = old_paths
            .into_iter()
            .filter(|path| !new_paths.contains(path))
            .collect::<Vec<_>>();
        if !stale_paths.is_empty() {
            database.bulk_remove_media_files(&stale_paths).await?;
        }

        Ok(())
    }

    /// Get the appropriate file extension for a playlist export
    pub fn get_output_filename(playlist_name: &str, format: PlaylistFormat) -> String {
        // Sanitize the playlist name for use as filename
        let safe_name = playlist_name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim()
            .replace("  ", " ");

        format!("{}.{}", safe_name, format.extension())
    }
}

fn is_http_stream(location: &str) -> bool {
    let location = location.trim();
    location
        .get(..7)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("http://"))
        || location
            .get(..8)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
}

fn is_radio_playlist_path(path: &Path) -> bool {
    path.parent().is_some_and(|parent| {
        parent.components().any(|component| {
            component
                .as_os_str()
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("radio"))
        })
    })
}

fn resolve_playlist_entry(base_dir: &Path, entry: &str) -> String {
    let entry = entry.trim();
    if is_http_stream(entry) {
        entry.to_string()
    } else {
        resolve_playlist_path(base_dir, entry)
            .to_string_lossy()
            .into_owned()
    }
}

/// Resolve a relative playlist path to absolute
fn resolve_playlist_path(base_dir: &Path, track_path_str: &str) -> PathBuf {
    let raw_path = PathBuf::from(track_path_str.replace('\\', "/"));
    let absolute_path = if raw_path.is_absolute() {
        raw_path
    } else {
        base_dir.join(raw_path)
    };

    if let Ok(canonical) = std::fs::canonicalize(&absolute_path) {
        canonical
    } else {
        let mut components = Vec::new();
        for component in absolute_path.components() {
            match component {
                std::path::Component::ParentDir => {
                    components.pop();
                }
                std::path::Component::CurDir => {}
                c => components.push(c),
            }
        }
        components.iter().collect()
    }
}

#[cfg(test)]
mod tests {
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
        use crate::database::redb::RedbDatabase;
        use crate::database::DatabaseManager;
        use tempfile::tempdir;

        let temp = tempdir().unwrap();
        let playlist_path = temp.path().join("stations.m3u");
        let url = "https://cast1.asurahosting.com/proxy/julien/stream";
        fs::write(&playlist_path, format!("#EXTM3U\n{url}\n")).unwrap();

        let database = RedbDatabase::new(temp.path().join("playlist.redb"))
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
        use crate::database::redb::RedbDatabase;
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

        let database = RedbDatabase::new(temp.path().join("radio.redb"))
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
}

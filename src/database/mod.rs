use anyhow::Result;
use async_trait::async_trait;
use futures_util::Stream;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{Duration, SystemTime};

use crate::platform::DatabaseError;

pub mod playlist_formats;
pub mod redb;


/// Represents a subdirectory in the media library.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MediaDirectory {
    pub path: PathBuf,
    pub name: String,
}

/// Represents a playlist
#[derive(Clone, Debug)]
pub struct Playlist {
    pub id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

/// Represents a playlist entry (track in a playlist)
#[derive(Clone, Debug)]
pub struct PlaylistEntry {
    pub id: Option<i64>,
    pub playlist_id: i64,
    pub media_file_id: i64,
    pub position: u32,
    pub created_at: SystemTime,
}

/// Music categorization container for organizing music content
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MusicCategory {
    pub id: String,
    pub name: String,
    pub category_type: MusicCategoryType,
    pub count: usize,
}

/// Types of music categorization
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MusicCategoryType {
    Artist,
    Album,
    Genre,
    AlbumArtist,
    Year,
    Playlist,
}

/// Enhanced MediaFile structure for database storage
#[derive(Clone, Debug)]
pub struct MediaFile {
    pub id: Option<i64>,
    pub path: PathBuf,
    pub filename: String,
    pub size: u64,
    pub modified: SystemTime,
    pub mime_type: String,
    pub duration: Option<Duration>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub track_number: Option<u32>,
    pub year: Option<u32>,
    pub album_artist: Option<String>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl MediaFile {
    pub fn new(path: PathBuf, size: u64, mime_type: String) -> Self {
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let now = SystemTime::now();

        Self {
            id: None,
            path,
            filename,
            size,
            modified: now,
            mime_type,
            duration: None,
            title: None,
            artist: None,
            album: None,
            genre: None,
            track_number: None,
            year: None,
            album_artist: None,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Database manager trait for media file operations
#[async_trait]
pub trait DatabaseManager: Send + Sync {
    /// Initialize the database and create tables if needed
    async fn initialize(&self) -> Result<()>;

    /// Store a new media file record
    async fn store_media_file(&self, file: &MediaFile) -> Result<i64>;

    /// Stream all media files from the database (memory efficient)
    /// 
    /// This method provides a memory-efficient way to process all media files without
    /// loading them all into memory at once. Use this instead of `get_all_media_files()`
    /// for large media libraries.
    /// 
    /// # Example
    /// ```rust,ignore
    /// use futures_util::StreamExt;
    /// 
    /// let mut stream = database.stream_all_media_files();
    /// while let Some(result) = stream.next().await {
    ///     match result {
    ///         Ok(media_file) => {
    ///             // Process each file individually
    ///             println!("Processing: {}", media_file.filename);
    ///         }
    ///         Err(e) => {
    ///             eprintln!("Error reading file: {}", e);
    ///         }
    ///     }
    /// }
    /// ```
    fn stream_all_media_files(&self) -> Pin<Box<dyn Stream<Item = Result<MediaFile, DatabaseError>> + Send + '_>>;

    /// Collect all media files from the stream (helper for tests)
    /// 
    /// This is a convenience method that collects all files from the stream into a Vec.
    /// Use this only for testing or when you need all files at once.
    async fn collect_all_media_files(&self) -> Result<Vec<MediaFile>> {
        use futures_util::StreamExt;
        
        let mut stream = self.stream_all_media_files();
        let mut files = Vec::new();
        
        while let Some(result) = stream.next().await {
            files.push(result?);
        }
        
        Ok(files)
    }

    /// Remove a media file record by path
    async fn remove_media_file(&self, path: &Path) -> Result<bool>;

    /// Update an existing media file record
    async fn update_media_file(&self, file: &MediaFile) -> Result<()>;

    /// Get all files in a specific directory
    async fn get_files_in_directory(&self, dir: &Path) -> Result<Vec<MediaFile>>;

    /// Get directory listing (subdirectories and files) for a given path and media type
    async fn get_directory_listing(
        &self,
        parent_path: &Path,
        media_type_filter: &str,
    ) -> Result<(Vec<MediaDirectory>, Vec<MediaFile>)>;

    /// Remove media files that no longer exist on disk
    async fn cleanup_missing_files(&self, existing_paths: &[PathBuf]) -> Result<usize>;

    /// Get a specific file by path
    async fn get_file_by_path(&self, path: &Path) -> Result<Option<MediaFile>>;

    /// Get a specific file by ID
    async fn get_file_by_id(&self, id: i64) -> Result<Option<MediaFile>>;

    /// Get database statistics
    async fn get_stats(&self) -> Result<DatabaseStats>;

    /// Check database integrity and repair if needed
    async fn check_and_repair(&self) -> Result<DatabaseHealth>;

    /// Create a backup of the database
    async fn create_backup(&self, backup_path: &Path) -> Result<()>;

    /// Restore database from backup
    async fn restore_from_backup(&self, backup_path: &Path) -> Result<()>;

    /// Vacuum the database to reclaim space and optimize performance
    async fn vacuum(&self) -> Result<()>;

    // Music categorization methods
    /// Get all unique artists
    async fn get_artists(&self) -> Result<Vec<MusicCategory>>;

    /// Get all albums, optionally filtered by artist
    async fn get_albums(&self, artist: Option<&str>) -> Result<Vec<MusicCategory>>;

    /// Get all genres
    async fn get_genres(&self) -> Result<Vec<MusicCategory>>;

    /// Get all years
    async fn get_years(&self) -> Result<Vec<MusicCategory>>;

    /// Get all album artists
    async fn get_album_artists(&self) -> Result<Vec<MusicCategory>>;

    /// Get music files by artist
    async fn get_music_by_artist(&self, artist: &str) -> Result<Vec<MediaFile>>;

    /// Get music files by album
    async fn get_music_by_album(&self, album: &str, artist: Option<&str>) -> Result<Vec<MediaFile>>;

    /// Get music files by genre
    async fn get_music_by_genre(&self, genre: &str) -> Result<Vec<MediaFile>>;

    /// Get music files by year
    async fn get_music_by_year(&self, year: u32) -> Result<Vec<MediaFile>>;

    /// Get music files by album artist
    async fn get_music_by_album_artist(&self, album_artist: &str) -> Result<Vec<MediaFile>>;

    // Playlist management methods
    /// Create a new playlist
    async fn create_playlist(&self, name: &str, description: Option<&str>) -> Result<i64>;

    /// Get all playlists
    async fn get_playlists(&self) -> Result<Vec<Playlist>>;

    /// Get a specific playlist by ID
    async fn get_playlist(&self, playlist_id: i64) -> Result<Option<Playlist>>;

    /// Update a playlist
    async fn update_playlist(&self, playlist: &Playlist) -> Result<()>;

    /// Delete a playlist
    async fn delete_playlist(&self, playlist_id: i64) -> Result<bool>;

    /// Add a track to a playlist
    async fn add_to_playlist(&self, playlist_id: i64, media_file_id: i64, position: Option<u32>) -> Result<i64>;

    /// Add multiple tracks to a playlist in a single transaction (batch operation)
    async fn batch_add_to_playlist(&self, playlist_id: i64, media_file_ids: &[(i64, u32)]) -> Result<Vec<i64>>;

    /// Get multiple files by their paths in a single query
    async fn get_files_by_paths(&self, paths: &[PathBuf]) -> Result<Vec<MediaFile>>;

    // Bulk operations for high-performance batch processing
    /// Store multiple media files in a single batch operation
    async fn bulk_store_media_files(&self, files: &[MediaFile]) -> Result<Vec<i64>>;

    /// Update multiple media files in a single batch operation
    async fn bulk_update_media_files(&self, files: &[MediaFile]) -> Result<()>;

    /// Remove multiple media files by paths in a single batch operation
    async fn bulk_remove_media_files(&self, paths: &[PathBuf]) -> Result<usize>;

    /// Get multiple files by their paths in a single batch query (alias for get_files_by_paths)
    async fn bulk_get_files_by_paths(&self, paths: &[PathBuf]) -> Result<Vec<MediaFile>> {
        self.get_files_by_paths(paths).await
    }

    /// Remove a track from a playlist
    async fn remove_from_playlist(&self, playlist_id: i64, media_file_id: i64) -> Result<bool>;

    /// Get all tracks in a playlist
    async fn get_playlist_tracks(&self, playlist_id: i64) -> Result<Vec<MediaFile>>;

    /// Reorder tracks in a playlist
    async fn reorder_playlist(&self, playlist_id: i64, track_positions: &[(i64, u32)]) -> Result<()>;

    // Playlist file format operations
    /// Import a playlist from a file (.m3u or .pls)
    async fn import_playlist_file(&self, file_path: &Path, playlist_name: Option<String>) -> Result<i64> {
        playlist_formats::PlaylistFileManager::import_playlist(self, file_path, playlist_name).await
    }

    /// Export a playlist to a file
    async fn export_playlist_file(&self, playlist_id: i64, output_path: &Path, format: playlist_formats::PlaylistFormat) -> Result<()> {
        playlist_formats::PlaylistFileManager::export_playlist(self, playlist_id, output_path, format).await
    }

    /// Scan directory for playlist files and import them
    async fn scan_and_import_playlists(&self, directory: &Path) -> Result<Vec<i64>> {
        playlist_formats::PlaylistFileManager::scan_and_import_playlists(self, directory).await
    }

    // New methods for efficient path-based queries using canonical paths

    /// Get files with a specific canonical path prefix (for efficient directory deletion)
    async fn get_files_with_path_prefix(&self, canonical_prefix: &str) -> Result<Vec<MediaFile>>;

    /// Get direct subdirectories using canonical paths (optimized two-query approach)
    async fn get_direct_subdirectories(&self, canonical_parent_path: &str) -> Result<Vec<MediaDirectory>>;

    /// Batch cleanup missing files using canonical paths and HashSet difference logic
    async fn batch_cleanup_missing_files(&self, existing_canonical_paths: &HashSet<String>) -> Result<usize>;

    /// Database-native file cleanup that performs cleanup entirely in SQL
    /// This method accepts existing paths and performs cleanup using temporary tables
    /// for better performance and memory efficiency with large datasets
    async fn database_native_cleanup(&self, existing_canonical_paths: &[String]) -> Result<usize>;

    /// Get direct subdirectories that contain files matching the media type filter (internal helper)
    async fn get_filtered_direct_subdirectories(
        &self,
        canonical_parent_path: &str,
        mime_filter: &str,
    ) -> Result<Vec<MediaDirectory>>;
}

#[derive(Debug)]
pub struct DatabaseStats {
    pub total_files: usize,
    pub total_size: u64,
    pub database_size: u64,
}

#[derive(Debug, Clone)]
pub struct DatabaseHealth {
    pub is_healthy: bool,
    pub corruption_detected: bool,
    pub integrity_check_passed: bool,
    pub issues: Vec<DatabaseIssue>,
    pub repair_attempted: bool,
    pub repair_successful: bool,
}

#[derive(Debug, Clone)]
pub struct DatabaseIssue {
    pub severity: IssueSeverity,
    pub description: String,
    pub table_affected: Option<String>,
    pub suggested_action: String,
}

#[derive(Debug, Clone)]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
    Critical,
}
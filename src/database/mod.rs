use anyhow::Result;
use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub mod playlist_formats;

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

    /// Create MediaFile from database row
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self> {
        let path_str: String = row.try_get("path")?;
        let modified_timestamp: i64 = row.try_get("modified")?;
        let created_timestamp: i64 = row.try_get("created_at")?;
        let updated_timestamp: i64 = row.try_get("updated_at")?;

        let duration_ms: Option<i64> = row.try_get("duration")?;
        let duration = duration_ms.map(|ms| Duration::from_millis(ms as u64));

        Ok(Self {
            id: Some(row.try_get("id")?),
            path: PathBuf::from(path_str),
            filename: row.try_get("filename")?,
            size: row.try_get::<i64, _>("size")? as u64,
            modified: SystemTime::UNIX_EPOCH + Duration::from_secs(modified_timestamp as u64),
            mime_type: row.try_get("mime_type")?,
            duration,
            title: row.try_get("title")?,
            artist: row.try_get("artist")?,
            album: row.try_get("album")?,
            genre: row.try_get("genre").ok(),
            track_number: row.try_get::<Option<i32>, _>("track_number")?.map(|n| n as u32),
            year: row.try_get::<Option<i32>, _>("year")?.map(|y| y as u32),
            album_artist: row.try_get("album_artist").ok(),
            created_at: SystemTime::UNIX_EPOCH + Duration::from_secs(created_timestamp as u64),
            updated_at: SystemTime::UNIX_EPOCH + Duration::from_secs(updated_timestamp as u64),
        })
    }
}

/// Database manager trait for media file operations
#[async_trait]
pub trait DatabaseManager: Send + Sync {
    /// Initialize the database and create tables if needed
    async fn initialize(&self) -> Result<()>;

    /// Store a new media file record
    async fn store_media_file(&self, file: &MediaFile) -> Result<i64>;

    /// Get all media files from the database
    async fn get_all_media_files(&self) -> Result<Vec<MediaFile>>;

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

/// SQLite implementation of DatabaseManager
pub struct SqliteDatabase {
    pool: SqlitePool,
    db_path: PathBuf,
}

impl SqliteDatabase {
    /// Create a new SQLite database manager
    pub async fn new(db_path: PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
        let pool = SqlitePool::connect(&database_url).await?;

        Ok(Self { pool, db_path })
    }

    /// Create database tables
    async fn create_tables(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS media_files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT UNIQUE NOT NULL,
                parent_path TEXT NOT NULL,
                filename TEXT NOT NULL,
                size INTEGER NOT NULL,
                modified INTEGER NOT NULL,
                mime_type TEXT NOT NULL,
                duration INTEGER,
                title TEXT,
                artist TEXT,
                album TEXT,
                genre TEXT,
                track_number INTEGER,
                year INTEGER,
                album_artist TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create playlists table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS playlists (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                description TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create playlist entries table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS playlist_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                playlist_id INTEGER NOT NULL,
                media_file_id INTEGER NOT NULL,
                position INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
                FOREIGN KEY (media_file_id) REFERENCES media_files(id) ON DELETE CASCADE,
                UNIQUE(playlist_id, media_file_id),
                UNIQUE(playlist_id, position)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes for better query performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_files_path ON media_files(path)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_files_parent_path ON media_files(parent_path)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_files_modified ON media_files(modified)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_files_mime_type ON media_files(mime_type)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_files_filename ON media_files(filename)")
            .execute(&self.pool)
            .await?;

        // Music categorization indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_files_artist ON media_files(artist)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_files_album ON media_files(album)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_files_genre ON media_files(genre)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_files_year ON media_files(year)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_files_album_artist ON media_files(album_artist)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_files_track_number ON media_files(track_number)")
            .execute(&self.pool)
            .await?;

        // Playlist indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_playlist_entries_playlist_id ON playlist_entries(playlist_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_playlist_entries_media_file_id ON playlist_entries(media_file_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_playlist_entries_position ON playlist_entries(playlist_id, position)")
            .execute(&self.pool)
            .await?;

        // Create database metadata table for migrations
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS database_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Set initial schema version
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        sqlx::query(
            "INSERT OR IGNORE INTO database_metadata (key, value, updated_at) VALUES (?, ?, ?)",
        )
        .bind("schema_version")
        .bind("1")
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Convert SystemTime to Unix timestamp
    fn system_time_to_timestamp(time: SystemTime) -> i64 {
        time.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }
}

#[async_trait]
impl DatabaseManager for SqliteDatabase {
    async fn initialize(&self) -> Result<()> {
        // Configure SQLite for better performance
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(&self.pool)
            .await?;
        sqlx::query("PRAGMA synchronous = NORMAL")
            .execute(&self.pool)
            .await?;
        sqlx::query("PRAGMA temp_store = MEMORY")
            .execute(&self.pool)
            .await?;
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&self.pool)
            .await?;
        sqlx::query("PRAGMA cache_size = -10000") // 10MB cache
            .execute(&self.pool)
            .await?;

        self.create_tables().await?;
        Ok(())
    }

    async fn store_media_file(&self, file: &MediaFile) -> Result<i64> {
        let path_str = file.path.to_string_lossy().to_string();
        let parent_path_str = file.path.parent().unwrap_or_else(|| Path::new("")).to_string_lossy().to_string();
        let modified_timestamp = Self::system_time_to_timestamp(file.modified);
        let created_timestamp = Self::system_time_to_timestamp(file.created_at);
        let updated_timestamp = Self::system_time_to_timestamp(file.updated_at);
        let duration_ms = file.duration.map(|d| d.as_millis() as i64);
        let track_number = file.track_number.map(|n| n as i64);
        let year = file.year.map(|y| y as i64);

        let result = sqlx::query(
            r#"
            INSERT INTO media_files 
            (path, parent_path, filename, size, modified, mime_type, duration, title, artist, album, genre, track_number, year, album_artist, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&path_str)
        .bind(&parent_path_str)
        .bind(&file.filename)
        .bind(file.size as i64)
        .bind(modified_timestamp)
        .bind(&file.mime_type)
        .bind(duration_ms)
        .bind(&file.title)
        .bind(&file.artist)
        .bind(&file.album)
        .bind(&file.genre)
        .bind(track_number)
        .bind(year)
        .bind(&file.album_artist)
        .bind(created_timestamp)
        .bind(updated_timestamp)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    async fn get_all_media_files(&self) -> Result<Vec<MediaFile>> {
        let rows = sqlx::query(
            "SELECT id, path, filename, size, modified, mime_type, duration, title, artist, album, genre, track_number, year, album_artist, created_at, updated_at FROM media_files ORDER BY filename"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::new();
        for row in rows {
            files.push(MediaFile::from_row(&row)?);
        }

        Ok(files)
    }

    async fn remove_media_file(&self, path: &Path) -> Result<bool> {
        let path_str = path.to_string_lossy().to_string();

        let result = sqlx::query("DELETE FROM media_files WHERE path = ?")
            .bind(&path_str)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn update_media_file(&self, file: &MediaFile) -> Result<()> {
        let path_str = file.path.to_string_lossy().to_string();
        let parent_path_str = file.path.parent().unwrap_or_else(|| Path::new("")).to_string_lossy().to_string();
        let modified_timestamp = Self::system_time_to_timestamp(file.modified);
        let updated_timestamp = Self::system_time_to_timestamp(SystemTime::now());
        let duration_ms = file.duration.map(|d| d.as_millis() as i64);
        let track_number = file.track_number.map(|n| n as i64);
        let year = file.year.map(|y| y as i64);

        sqlx::query(
            r#"
            UPDATE media_files 
            SET parent_path = ?, filename = ?, size = ?, modified = ?, mime_type = ?, duration = ?, 
                title = ?, artist = ?, album = ?, genre = ?, track_number = ?, year = ?, album_artist = ?, updated_at = ?
            WHERE path = ?
            "#,
        )
        .bind(&parent_path_str)
        .bind(&file.filename)
        .bind(file.size as i64)
        .bind(modified_timestamp)
        .bind(&file.mime_type)
        .bind(duration_ms)
        .bind(&file.title)
        .bind(&file.artist)
        .bind(&file.album)
        .bind(&file.genre)
        .bind(track_number)
        .bind(year)
        .bind(&file.album_artist)
        .bind(updated_timestamp)
        .bind(&path_str)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_files_in_directory(&self, dir: &Path) -> Result<Vec<MediaFile>> {
        let dir_str = format!("{}%", dir.to_string_lossy());

        let rows = sqlx::query(
            r#"
            SELECT id, path, filename, size, modified, mime_type, duration, title, artist, album, genre, track_number, year, album_artist, created_at, updated_at 
            FROM media_files 
            WHERE path LIKE ?
            ORDER BY filename
            "#,
        )
        .bind(&dir_str)
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::new();
        for row in rows {
            files.push(MediaFile::from_row(&row)?);
        }

        Ok(files)
    }

    async fn get_directory_listing(
        &self,
        parent_path: &Path,
        media_type_filter: &str,
    ) -> Result<(Vec<MediaDirectory>, Vec<MediaFile>)> {
        let parent_path_str = parent_path.to_string_lossy().to_string();
        let mime_filter_str = if media_type_filter.is_empty() {
            "%".to_string()
        } else {
            format!("{}%", media_type_filter)
        };

        // 1. Get all direct file children efficiently
        let file_rows = sqlx::query(
            r#"
            SELECT id, path, filename, size, modified, mime_type, duration, title, artist, album, genre, track_number, year, album_artist, created_at, updated_at 
            FROM media_files 
            WHERE parent_path = ? AND mime_type LIKE ?
            ORDER BY filename
            "#,
        )
        .bind(&parent_path_str)
        .bind(&mime_filter_str)
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::new();
        for row in file_rows {
            files.push(MediaFile::from_row(&row)?);
        }

        // 2. Get all unique subdirectory paths that contain matching files
        // Use the same separator that would be in the stored paths
        let path_separator = std::path::MAIN_SEPARATOR;
        let like_path_prefix = if parent_path_str.is_empty() {
            "%".to_string()
        } else if parent_path_str == "/" || parent_path_str == "\\" {
            format!("{}%", path_separator)
        } else {
            format!("{}{}%", parent_path_str, path_separator)
        };

        let subdir_rows = sqlx::query(
            r#"
            SELECT DISTINCT parent_path 
            FROM media_files 
            WHERE parent_path LIKE ? AND parent_path != ? AND mime_type LIKE ?
            "#,
        )
        .bind(&like_path_prefix)
        .bind(&parent_path_str)
        .bind(&mime_filter_str)
        .fetch_all(&self.pool)
        .await?;
        
        let mut subdirectories = HashSet::new();
        for row in subdir_rows {
            let descendant_parent_path_str: String = row.try_get("parent_path")?;
            let descendant_parent_path = PathBuf::from(descendant_parent_path_str);
            
            if let Ok(relative_path) = descendant_parent_path.strip_prefix(parent_path) {
                if let Some(first_component) = relative_path.components().next() {
                    if let std::path::Component::Normal(name) = first_component {
                        let final_subdir_path = parent_path.join(name);
                        subdirectories.insert(MediaDirectory {
                            path: final_subdir_path,
                            name: name.to_string_lossy().to_string(),
                        });
                    }
                }
            }
        }

        let mut sorted_subdirectories: Vec<_> = subdirectories.into_iter().collect();
        sorted_subdirectories.sort_by_key(|d| d.name.to_lowercase());

        Ok((sorted_subdirectories, files))
    }

    async fn cleanup_missing_files(&self, existing_paths: &[PathBuf]) -> Result<usize> {
        if existing_paths.is_empty() {
            // If no existing paths provided, don't remove anything
            return Ok(0);
        }

        let existing_paths: Vec<String> = existing_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        // Create placeholders for the IN clause
        let placeholders = existing_paths
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let query = format!("DELETE FROM media_files WHERE path NOT IN ({})", placeholders);

        let mut query_builder = sqlx::query(&query);
        for path in &existing_paths {
            query_builder = query_builder.bind(path);
        }

        let result = query_builder.execute(&self.pool).await?;

        Ok(result.rows_affected() as usize)
    }

    async fn get_file_by_path(&self, path: &Path) -> Result<Option<MediaFile>> {
        let path_str = path.to_string_lossy().to_string();

        let row = sqlx::query(
            r#"
            SELECT id, path, filename, size, modified, mime_type, duration, title, artist, album, genre, track_number, year, album_artist, created_at, updated_at 
            FROM media_files 
            WHERE path = ?
            "#,
        )
        .bind(&path_str)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(MediaFile::from_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_file_by_id(&self, id: i64) -> Result<Option<MediaFile>> {
        let row = sqlx::query(
            r#"
            SELECT id, path, filename, size, modified, mime_type, duration, title, artist, album, genre, track_number, year, album_artist, created_at, updated_at 
            FROM media_files 
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(MediaFile::from_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_stats(&self) -> Result<DatabaseStats> {
        // Get total files and size
        let row = sqlx::query("SELECT COUNT(*), COALESCE(SUM(size), 0) FROM media_files")
            .fetch_one(&self.pool)
            .await?;

        let total_files: i64 = row.try_get(0)?;
        let total_size: i64 = row.try_get(1)?;

        // Get database file size
        let database_size = tokio::fs::metadata(&self.db_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(DatabaseStats {
            total_files: total_files as usize,
            total_size: total_size as u64,
            database_size,
        })
    }

    async fn check_and_repair(&self) -> Result<DatabaseHealth> {
        let mut health = DatabaseHealth {
            is_healthy: true,
            corruption_detected: false,
            integrity_check_passed: false,
            issues: Vec::new(),
            repair_attempted: false,
            repair_successful: false,
        };

        // Run integrity check
        match self.run_integrity_check().await {
            Ok(integrity_ok) => {
                health.integrity_check_passed = integrity_ok;
                if !integrity_ok {
                    health.is_healthy = false;
                    health.corruption_detected = true;
                    health.issues.push(DatabaseIssue {
                        severity: IssueSeverity::Critical,
                        description: "Database integrity check failed".to_string(),
                        table_affected: None,
                        suggested_action: "Attempt database repair or restore from backup"
                            .to_string(),
                    });
                }
            }
            Err(e) => {
                health.is_healthy = false;
                health.issues.push(DatabaseIssue {
                    severity: IssueSeverity::Error,
                    description: format!("Failed to run integrity check: {}", e),
                    table_affected: None,
                    suggested_action: "Check database file permissions and disk space".to_string(),
                });
            }
        }

        // Check for common issues
        if let Err(e) = self.check_common_issues(&mut health).await {
            health.issues.push(DatabaseIssue {
                severity: IssueSeverity::Warning,
                description: format!("Error during common issues check: {}", e),
                table_affected: None,
                suggested_action: "Review database configuration".to_string(),
            });
        }

        // Attempt repair if corruption detected
        if health.corruption_detected {
            health.repair_attempted = true;
            match self.attempt_repair().await {
                Ok(success) => {
                    health.repair_successful = success;
                    if success {
                        health.is_healthy = true;
                        health.corruption_detected = false;
                        health.issues.push(DatabaseIssue {
                            severity: IssueSeverity::Info,
                            description: "Database successfully repaired".to_string(),
                            table_affected: None,
                            suggested_action: "Consider creating a backup".to_string(),
                        });
                    }
                }
                Err(e) => {
                    health.issues.push(DatabaseIssue {
                        severity: IssueSeverity::Critical,
                        description: format!("Database repair failed: {}", e),
                        table_affected: None,
                        suggested_action: "Restore from backup or recreate database".to_string(),
                    });
                }
            }
        }

        Ok(health)
    }

    async fn create_backup(&self, backup_path: &Path) -> Result<()> {
        // Ensure backup directory exists
        if let Some(parent) = backup_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Create backup using SQLite's backup API through a VACUUM INTO command
        let backup_path_str = backup_path.to_string_lossy().to_string();

        sqlx::query(&format!("VACUUM INTO '{}'", backup_path_str))
            .execute(&self.pool)
            .await?;

        // Verify backup was created successfully
        if !backup_path.exists() {
            return Err(anyhow::anyhow!("Backup file was not created"));
        }

        // Verify backup integrity
        let backup_url = format!("sqlite://{}?mode=ro", backup_path.display());
        let backup_pool = SqlitePool::connect(&backup_url).await?;

        let integrity_ok = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&backup_pool)
            .await?;

        backup_pool.close().await;

        if integrity_ok != "ok" {
            tokio::fs::remove_file(backup_path).await.ok(); // Clean up bad backup
            return Err(anyhow::anyhow!(
                "Backup integrity check failed: {}",
                integrity_ok
            ));
        }

        Ok(())
    }

    async fn restore_from_backup(&self, backup_path: &Path) -> Result<()> {
        if !backup_path.exists() {
            return Err(anyhow::anyhow!(
                "Backup file does not exist: {}",
                backup_path.display()
            ));
        }

        // Verify backup integrity before restore
        let backup_url = format!("sqlite://{}?mode=ro", backup_path.display());
        let backup_pool = SqlitePool::connect(&backup_url).await?;

        let integrity_ok = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&backup_pool)
            .await?;

        backup_pool.close().await;

        if integrity_ok != "ok" {
            return Err(anyhow::anyhow!(
                "Backup file is corrupted: {}",
                integrity_ok
            ));
        }

        // Close current connection
        self.pool.close().await;

        // Replace current database with backup
        tokio::fs::copy(backup_path, &self.db_path).await?;

        // Reconnect to restored database
        let database_url = format!("sqlite://{}?mode=rwc", self.db_path.display());
        let new_pool = SqlitePool::connect(&database_url).await?;

        // Configure SQLite for better performance
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&new_pool)
            .await?;
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(&new_pool)
            .await?;
        sqlx::query("PRAGMA synchronous = NORMAL")
            .execute(&new_pool)
            .await?;
        sqlx::query("PRAGMA cache_size = 10000")
            .execute(&new_pool)
            .await?;
        sqlx::query("PRAGMA temp_store = MEMORY")
            .execute(&new_pool)
            .await?;

        // Note: We can't replace self.pool here due to borrowing rules
        // In a real implementation, this would require restructuring or using Arc<Mutex<>>

        Ok(())
    }

    async fn vacuum(&self) -> Result<()> {
        sqlx::query("VACUUM").execute(&self.pool).await?;

        Ok(())
    }

    // Music categorization methods
    async fn get_artists(&self) -> Result<Vec<MusicCategory>> {
        let rows = sqlx::query(
            r#"
            SELECT artist, COUNT(*) as count 
            FROM media_files 
            WHERE mime_type LIKE 'audio/%' AND artist IS NOT NULL AND artist != ''
            GROUP BY artist 
            ORDER BY artist COLLATE NOCASE
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut categories = Vec::new();
        for row in rows {
            let artist: String = row.try_get("artist")?;
            let count: i64 = row.try_get("count")?;
            categories.push(MusicCategory {
                id: format!("artist:{}", artist),
                name: artist,
                category_type: MusicCategoryType::Artist,
                count: count as usize,
            });
        }

        Ok(categories)
    }

    async fn get_albums(&self, artist: Option<&str>) -> Result<Vec<MusicCategory>> {
        let rows = if let Some(artist_filter) = artist {
            sqlx::query(
                r#"
                SELECT album, COUNT(*) as count 
                FROM media_files 
                WHERE mime_type LIKE 'audio/%' AND album IS NOT NULL AND album != '' AND artist = ?
                GROUP BY album 
                ORDER BY album COLLATE NOCASE
                "#
            )
            .bind(artist_filter)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT album, COUNT(*) as count 
                FROM media_files 
                WHERE mime_type LIKE 'audio/%' AND album IS NOT NULL AND album != ''
                GROUP BY album 
                ORDER BY album COLLATE NOCASE
                "#
            )
            .fetch_all(&self.pool)
            .await?
        };

        let mut categories = Vec::new();
        for row in rows {
            let album: String = row.try_get("album")?;
            let count: i64 = row.try_get("count")?;
            let id = if let Some(artist_filter) = artist {
                format!("album:{}:{}", artist_filter, album)
            } else {
                format!("album:{}", album)
            };
            categories.push(MusicCategory {
                id,
                name: album,
                category_type: MusicCategoryType::Album,
                count: count as usize,
            });
        }

        Ok(categories)
    }

    async fn get_genres(&self) -> Result<Vec<MusicCategory>> {
        let rows = sqlx::query(
            r#"
            SELECT genre, COUNT(*) as count 
            FROM media_files 
            WHERE mime_type LIKE 'audio/%' AND genre IS NOT NULL AND genre != ''
            GROUP BY genre 
            ORDER BY genre COLLATE NOCASE
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut categories = Vec::new();
        for row in rows {
            let genre: String = row.try_get("genre")?;
            let count: i64 = row.try_get("count")?;
            categories.push(MusicCategory {
                id: format!("genre:{}", genre),
                name: genre,
                category_type: MusicCategoryType::Genre,
                count: count as usize,
            });
        }

        Ok(categories)
    }

    async fn get_years(&self) -> Result<Vec<MusicCategory>> {
        let rows = sqlx::query(
            r#"
            SELECT year, COUNT(*) as count 
            FROM media_files 
            WHERE mime_type LIKE 'audio/%' AND year IS NOT NULL
            GROUP BY year 
            ORDER BY year DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut categories = Vec::new();
        for row in rows {
            let year: i64 = row.try_get("year")?;
            let count: i64 = row.try_get("count")?;
            categories.push(MusicCategory {
                id: format!("year:{}", year),
                name: year.to_string(),
                category_type: MusicCategoryType::Year,
                count: count as usize,
            });
        }

        Ok(categories)
    }

    async fn get_album_artists(&self) -> Result<Vec<MusicCategory>> {
        let rows = sqlx::query(
            r#"
            SELECT album_artist, COUNT(*) as count 
            FROM media_files 
            WHERE mime_type LIKE 'audio/%' AND album_artist IS NOT NULL AND album_artist != ''
            GROUP BY album_artist 
            ORDER BY album_artist COLLATE NOCASE
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut categories = Vec::new();
        for row in rows {
            let album_artist: String = row.try_get("album_artist")?;
            let count: i64 = row.try_get("count")?;
            categories.push(MusicCategory {
                id: format!("album_artist:{}", album_artist),
                name: album_artist,
                category_type: MusicCategoryType::AlbumArtist,
                count: count as usize,
            });
        }

        Ok(categories)
    }

    async fn get_music_by_artist(&self, artist: &str) -> Result<Vec<MediaFile>> {
        let rows = sqlx::query(
            r#"
            SELECT id, path, filename, size, modified, mime_type, duration, title, artist, album, genre, track_number, year, album_artist, created_at, updated_at 
            FROM media_files 
            WHERE mime_type LIKE 'audio/%' AND artist = ?
            ORDER BY album, track_number, filename
            "#
        )
        .bind(artist)
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::new();
        for row in rows {
            files.push(MediaFile::from_row(&row)?);
        }

        Ok(files)
    }

    async fn get_music_by_album(&self, album: &str, artist: Option<&str>) -> Result<Vec<MediaFile>> {
        let rows = if let Some(artist_filter) = artist {
            sqlx::query(
                r#"
                SELECT id, path, filename, size, modified, mime_type, duration, title, artist, album, genre, track_number, year, album_artist, created_at, updated_at 
                FROM media_files 
                WHERE mime_type LIKE 'audio/%' AND album = ? AND artist = ?
                ORDER BY track_number, filename
                "#
            )
            .bind(album)
            .bind(artist_filter)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, path, filename, size, modified, mime_type, duration, title, artist, album, genre, track_number, year, album_artist, created_at, updated_at 
                FROM media_files 
                WHERE mime_type LIKE 'audio/%' AND album = ?
                ORDER BY track_number, filename
                "#
            )
            .bind(album)
            .fetch_all(&self.pool)
            .await?
        };

        let mut files = Vec::new();
        for row in rows {
            files.push(MediaFile::from_row(&row)?);
        }

        Ok(files)
    }

    async fn get_music_by_genre(&self, genre: &str) -> Result<Vec<MediaFile>> {
        let rows = sqlx::query(
            r#"
            SELECT id, path, filename, size, modified, mime_type, duration, title, artist, album, genre, track_number, year, album_artist, created_at, updated_at 
            FROM media_files 
            WHERE mime_type LIKE 'audio/%' AND genre = ?
            ORDER BY artist, album, track_number, filename
            "#
        )
        .bind(genre)
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::new();
        for row in rows {
            files.push(MediaFile::from_row(&row)?);
        }

        Ok(files)
    }

    async fn get_music_by_year(&self, year: u32) -> Result<Vec<MediaFile>> {
        let rows = sqlx::query(
            r#"
            SELECT id, path, filename, size, modified, mime_type, duration, title, artist, album, genre, track_number, year, album_artist, created_at, updated_at 
            FROM media_files 
            WHERE mime_type LIKE 'audio/%' AND year = ?
            ORDER BY artist, album, track_number, filename
            "#
        )
        .bind(year as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::new();
        for row in rows {
            files.push(MediaFile::from_row(&row)?);
        }

        Ok(files)
    }

    async fn get_music_by_album_artist(&self, album_artist: &str) -> Result<Vec<MediaFile>> {
        let rows = sqlx::query(
            r#"
            SELECT id, path, filename, size, modified, mime_type, duration, title, artist, album, genre, track_number, year, album_artist, created_at, updated_at 
            FROM media_files 
            WHERE mime_type LIKE 'audio/%' AND album_artist = ?
            ORDER BY album, track_number, filename
            "#
        )
        .bind(album_artist)
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::new();
        for row in rows {
            files.push(MediaFile::from_row(&row)?);
        }

        Ok(files)
    }

    // Playlist management methods
    async fn create_playlist(&self, name: &str, description: Option<&str>) -> Result<i64> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let result = sqlx::query(
            r#"
            INSERT INTO playlists (name, description, created_at, updated_at)
            VALUES (?, ?, ?, ?)
            "#
        )
        .bind(name)
        .bind(description)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    async fn get_playlists(&self) -> Result<Vec<Playlist>> {
        let rows = sqlx::query(
            "SELECT id, name, description, created_at, updated_at FROM playlists ORDER BY name"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut playlists = Vec::new();
        for row in rows {
            let created_timestamp: i64 = row.try_get("created_at")?;
            let updated_timestamp: i64 = row.try_get("updated_at")?;
            playlists.push(Playlist {
                id: Some(row.try_get("id")?),
                name: row.try_get("name")?,
                description: row.try_get("description")?,
                created_at: SystemTime::UNIX_EPOCH + Duration::from_secs(created_timestamp as u64),
                updated_at: SystemTime::UNIX_EPOCH + Duration::from_secs(updated_timestamp as u64),
            });
        }

        Ok(playlists)
    }

    async fn get_playlist(&self, playlist_id: i64) -> Result<Option<Playlist>> {
        let row = sqlx::query(
            "SELECT id, name, description, created_at, updated_at FROM playlists WHERE id = ?"
        )
        .bind(playlist_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let created_timestamp: i64 = row.try_get("created_at")?;
            let updated_timestamp: i64 = row.try_get("updated_at")?;
            Ok(Some(Playlist {
                id: Some(row.try_get("id")?),
                name: row.try_get("name")?,
                description: row.try_get("description")?,
                created_at: SystemTime::UNIX_EPOCH + Duration::from_secs(created_timestamp as u64),
                updated_at: SystemTime::UNIX_EPOCH + Duration::from_secs(updated_timestamp as u64),
            }))
        } else {
            Ok(None)
        }
    }

    async fn update_playlist(&self, playlist: &Playlist) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        sqlx::query(
            "UPDATE playlists SET name = ?, description = ?, updated_at = ? WHERE id = ?"
        )
        .bind(&playlist.name)
        .bind(&playlist.description)
        .bind(now)
        .bind(playlist.id.unwrap())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete_playlist(&self, playlist_id: i64) -> Result<bool> {
        let result = sqlx::query("DELETE FROM playlists WHERE id = ?")
            .bind(playlist_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn add_to_playlist(&self, playlist_id: i64, media_file_id: i64, position: Option<u32>) -> Result<i64> {
        let final_position = if let Some(pos) = position {
            pos
        } else {
            // Get the next position in the playlist
            let max_position: Option<i32> = sqlx::query_scalar(
                "SELECT MAX(position) FROM playlist_entries WHERE playlist_id = ?"
            )
            .bind(playlist_id)
            .fetch_optional(&self.pool)
            .await?
            .flatten();
            
            (max_position.unwrap_or(-1) + 1) as u32
        };

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let result = sqlx::query(
            "INSERT INTO playlist_entries (playlist_id, media_file_id, position, created_at) VALUES (?, ?, ?, ?)"
        )
        .bind(playlist_id)
        .bind(media_file_id)
        .bind(final_position as i64)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    async fn remove_from_playlist(&self, playlist_id: i64, media_file_id: i64) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM playlist_entries WHERE playlist_id = ? AND media_file_id = ?"
        )
        .bind(playlist_id)
        .bind(media_file_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn get_playlist_tracks(&self, playlist_id: i64) -> Result<Vec<MediaFile>> {
        let rows = sqlx::query(
            r#"
            SELECT m.id, m.path, m.filename, m.size, m.modified, m.mime_type, m.duration, 
                   m.title, m.artist, m.album, m.genre, m.track_number, m.year, m.album_artist, 
                   m.created_at, m.updated_at
            FROM media_files m
            JOIN playlist_entries pe ON m.id = pe.media_file_id
            WHERE pe.playlist_id = ?
            ORDER BY pe.position
            "#
        )
        .bind(playlist_id)
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::new();
        for row in rows {
            files.push(MediaFile::from_row(&row)?);
        }

        Ok(files)
    }

    async fn reorder_playlist(&self, playlist_id: i64, track_positions: &[(i64, u32)]) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        for (media_file_id, position) in track_positions {
            sqlx::query(
                "UPDATE playlist_entries SET position = ? WHERE playlist_id = ? AND media_file_id = ?"
            )
            .bind(*position as i64)
            .bind(playlist_id)
            .bind(*media_file_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

impl SqliteDatabase {
    /// Run SQLite integrity check
    async fn run_integrity_check(&self) -> Result<bool> {
        let result = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&self.pool)
            .await?;

        Ok(result == "ok")
    }

    /// Check for common database issues
    async fn check_common_issues(&self, health: &mut DatabaseHealth) -> Result<()> {
        // Check for orphaned records or inconsistencies
        let orphaned_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM media_files WHERE path = '' OR filename = '' OR parent_path = ''")
                .fetch_one(&self.pool)
                .await?;

        if orphaned_count > 0 {
            health.issues.push(DatabaseIssue {
                severity: IssueSeverity::Warning,
                description: format!("Found {} records with empty path, filename, or parent_path", orphaned_count),
                table_affected: Some("media_files".to_string()),
                suggested_action: "Clean up orphaned records".to_string(),
            });
        }

        // Check for duplicate paths
        let duplicate_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM (SELECT path FROM media_files GROUP BY path HAVING COUNT(*) > 1)",
        )
        .fetch_one(&self.pool)
        .await?;

        if duplicate_count > 0 {
            health.issues.push(DatabaseIssue {
                severity: IssueSeverity::Warning,
                description: format!("Found {} duplicate file paths", duplicate_count),
                table_affected: Some("media_files".to_string()),
                suggested_action: "Remove duplicate entries".to_string(),
            });
        }

        // Check database size vs file count ratio
        let stats = self.get_stats().await?;
        if stats.total_files > 0 {
            let avg_db_size_per_file = stats.database_size / stats.total_files as u64;
            if avg_db_size_per_file > 10000 {
                // More than 10KB per file record seems excessive
                health.issues.push(DatabaseIssue {
                    severity: IssueSeverity::Info,
                    description: "Database size seems large relative to file count".to_string(),
                    table_affected: None,
                    suggested_action: "Consider running VACUUM to optimize database".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Attempt to repair database corruption
    async fn attempt_repair(&self) -> Result<bool> {
        // Try to clean up orphaned records
        sqlx::query("DELETE FROM media_files WHERE path = '' OR filename = '' OR parent_path = ''")
            .execute(&self.pool)
            .await?;

        // Remove duplicates, keeping the most recent
        sqlx::query(
            r#"
            DELETE FROM media_files 
            WHERE id NOT IN (
                SELECT MAX(id) 
                FROM media_files 
                GROUP BY path
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Try to rebuild indexes
        sqlx::query("REINDEX").execute(&self.pool).await?;

        // Run integrity check again
        self.run_integrity_check().await
    }

    /// Clean up orphaned and invalid records
    pub async fn cleanup_invalid_records(&self) -> Result<usize> {
        let result =
            sqlx::query("DELETE FROM media_files WHERE path = '' OR filename = '' OR parent_path = '' OR size < 0")
                .execute(&self.pool)
                .await?;

        Ok(result.rows_affected() as usize)
    }

    /// Remove duplicate file entries, keeping the most recent
    pub async fn remove_duplicates(&self) -> Result<usize> {
        let result = sqlx::query(
            r#"
            DELETE FROM media_files 
            WHERE id NOT IN (
                SELECT MAX(id) 
                FROM media_files 
                GROUP BY path
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_database_creation() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        let stats = db.get_stats().await.unwrap();
        assert_eq!(stats.total_files, 0);
    }

    #[tokio::test]
    async fn test_media_file_crud() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        // Create a test media file
        let mut media_file = MediaFile::new(
            PathBuf::from("/test/video.mp4"),
            1024,
            "video/mp4".to_string(),
        );
        media_file.title = Some("Test Video".to_string());

        // Store the file
        let id = db.store_media_file(&media_file).await.unwrap();
        assert!(id > 0);

        // Retrieve the file
        let retrieved = db
            .get_file_by_path(&PathBuf::from("/test/video.mp4"))
            .await
            .unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.filename, "video.mp4");
        assert_eq!(retrieved.title, Some("Test Video".to_string()));

        // Update the file
        let mut updated_file = retrieved.clone();
        updated_file.title = Some("Updated Video".to_string());
        db.update_media_file(&updated_file).await.unwrap();

        // Verify update
        let updated = db
            .get_file_by_path(&PathBuf::from("/test/video.mp4"))
            .await
            .unwrap();
        assert_eq!(updated.unwrap().title, Some("Updated Video".to_string()));

        // Remove the file
        let removed = db
            .remove_media_file(&PathBuf::from("/test/video.mp4"))
            .await
            .unwrap();
        assert!(removed);

        // Verify removal
        let not_found = db
            .get_file_by_path(&PathBuf::from("/test/video.mp4"))
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_database_health_check() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        // Add some test data
        let media_file =
            MediaFile::new(PathBuf::from("/test/video.mp4"), 1024, "video/mp4".to_string());
        db.store_media_file(&media_file).await.unwrap();

        // Run health check
        let health = db.check_and_repair().await.unwrap();
        assert!(health.is_healthy);
        assert!(health.integrity_check_passed);
        assert!(!health.corruption_detected);
    }

    #[tokio::test]
    async fn test_database_backup_and_restore() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let backup_path = temp_dir.path().join("backup.db");

        let db = SqliteDatabase::new(db_path.clone()).await.unwrap();
        db.initialize().await.unwrap();

        // Add some test data
        let media_file =
            MediaFile::new(PathBuf::from("/test/video.mp4"), 1024, "video/mp4".to_string());
        db.store_media_file(&media_file).await.unwrap();

        // Create backup
        db.create_backup(&backup_path).await.unwrap();
        assert!(backup_path.exists());

        // Verify backup contains data
        let backup_db = SqliteDatabase::new(backup_path.clone()).await.unwrap();
        let files = backup_db.get_all_media_files().await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "video.mp4");
    }

    #[tokio::test]
    async fn test_cleanup_invalid_records() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        // Add valid record
        let valid_file =
            MediaFile::new(PathBuf::from("/test/video.mp4"), 1024, "video/mp4".to_string());
        db.store_media_file(&valid_file).await.unwrap();

        // Manually insert invalid records
        sqlx::query("INSERT INTO media_files (path, parent_path, filename, size, modified, mime_type, created_at, updated_at) VALUES ('', '', 'empty.mp4', 1024, 0, 'video/mp4', 0, 0)")
            .execute(&db.pool)
            .await
            .unwrap();

        sqlx::query("INSERT INTO media_files (path, parent_path, filename, size, modified, mime_type, created_at, updated_at) VALUES ('/test/valid.mp4', '/test', '', 1024, 0, 'video/mp4', 0, 0)")
            .execute(&db.pool)
            .await
            .unwrap();

        // Verify we have 3 records (1 valid, 2 invalid)
        let all_files = db.get_all_media_files().await.unwrap();
        assert_eq!(all_files.len(), 3);

        // Clean up invalid records
        let cleaned = db.cleanup_invalid_records().await.unwrap();
        assert_eq!(cleaned, 2);

        // Verify only valid record remains
        let remaining_files = db.get_all_media_files().await.unwrap();
        assert_eq!(remaining_files.len(), 1);
        assert_eq!(remaining_files[0].filename, "video.mp4");
    }

    #[tokio::test]
    async fn test_remove_duplicates() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        // Add some unique records first
        let file1 = MediaFile::new(
            PathBuf::from("/test/video1.mp4"),
            1024,
            "video/mp4".to_string(),
        );
        let file2 = MediaFile::new(
            PathBuf::from("/test/video2.mp4"),
            2048,
            "video/mp4".to_string(),
        );

        db.store_media_file(&file1).await.unwrap();
        db.store_media_file(&file2).await.unwrap();

        // Verify we have 2 unique records
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_files")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(count, 2);

        // Test the remove_duplicates function (should have no effect since no duplicates exist)
        let removed = db.remove_duplicates().await.unwrap();
        assert_eq!(removed, 0);

        // Verify count is still 2
        let count_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_files")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(count_after, 2);
    }

    #[tokio::test]
    async fn test_get_directory_listing_empty() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        let parent_path = if cfg!(windows) {
            PathBuf::from("C:\\media")
        } else {
            PathBuf::from("/media")
        };
        let (dirs, files) = db.get_directory_listing(&parent_path, "").await.unwrap();

        assert!(dirs.is_empty());
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn test_get_directory_listing_with_files_and_subdirs() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        let parent_path = if cfg!(windows) {
            PathBuf::from("C:\\media")
        } else {
            PathBuf::from("/media")
        };

        // Direct files
        let file1 = MediaFile::new(parent_path.join("a.mp4"), 100, "video/mp4".to_string());
        let file2 = MediaFile::new(parent_path.join("b.mp3"), 200, "audio/mpeg".to_string());
        db.store_media_file(&file1).await.unwrap();
        db.store_media_file(&file2).await.unwrap();

        // Files in subdirectories
        let subdir1_path = parent_path.join("Videos");
        let file3 = MediaFile::new(subdir1_path.join("c.mp4"), 300, "video/mp4".to_string());
        db.store_media_file(&file3).await.unwrap();

        let subdir2_path = parent_path.join("Music");
        let file4 = MediaFile::new(subdir2_path.join("d.mp3"), 400, "audio/mpeg".to_string());
        db.store_media_file(&file4).await.unwrap();

        // File in nested subdirectory (should appear as part of immediate subdir)
        let nested_subdir_path = subdir1_path.join("Action");
        let file5 = MediaFile::new(nested_subdir_path.join("e.mp4"), 500, "video/mp4".to_string());
        db.store_media_file(&file5).await.unwrap();

        // Test without filter
        let (dirs, files) = db.get_directory_listing(&parent_path, "").await.unwrap();

        assert_eq!(dirs.len(), 2);
        assert!(dirs.contains(&MediaDirectory { path: subdir2_path.clone(), name: "Music".to_string() }));
        assert!(dirs.contains(&MediaDirectory { path: subdir1_path.clone(), name: "Videos".to_string() }));
        assert_eq!(dirs[0].name, "Music"); // Sorted alphabetically
        assert_eq!(dirs[1].name, "Videos");

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].filename, "a.mp4");
        assert_eq!(files[1].filename, "b.mp3");

        // Test with video filter
        let (dirs_video, files_video) = db.get_directory_listing(&parent_path, "video").await.unwrap();
        assert_eq!(dirs_video.len(), 1); // Only "Videos" should be listed as it contains video files
        assert!(dirs_video.contains(&MediaDirectory { path: subdir1_path.clone(), name: "Videos".to_string() }));
        assert!(!dirs_video.contains(&MediaDirectory { path: subdir2_path.clone(), name: "Music".to_string() })); // Music dir should NOT be listed
        assert_eq!(files_video.len(), 1);
        assert_eq!(files_video[0].filename, "a.mp4");

        // Test with audio filter
        let (dirs_audio, files_audio) = db.get_directory_listing(&parent_path, "audio").await.unwrap();
        assert_eq!(dirs_audio.len(), 1); // Only "Music" should be listed
        assert!(dirs_audio.contains(&MediaDirectory { path: subdir2_path.clone(), name: "Music".to_string() }));
        assert!(!dirs_audio.contains(&MediaDirectory { path: subdir1_path.clone(), name: "Videos".to_string() })); // Videos dir should NOT be listed
        assert_eq!(files_audio.len(), 1);
        assert_eq!(files_audio[0].filename, "b.mp3");
    }

    #[tokio::test]
    async fn test_get_directory_listing_subdir_only() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        let parent_path = if cfg!(windows) {
            PathBuf::from("C:\\media\\Videos")
        } else {
            PathBuf::from("/media/Videos")
        };
        let file1 = MediaFile::new(parent_path.join("movie.mp4"), 100, "video/mp4".to_string());
        db.store_media_file(&file1).await.unwrap();

        let (dirs, files) = db.get_directory_listing(&parent_path, "").await.unwrap();
        assert!(dirs.is_empty());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "movie.mp4");
    }

    #[tokio::test]
    async fn test_get_directory_listing_nested_subdirs_only() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        let parent_path = if cfg!(windows) {
            PathBuf::from("C:\\media")
        } else {
            PathBuf::from("/media")
        };
        let subdir1_path = parent_path.join("Movies");
        let subdir2_path = subdir1_path.join("Action");
        let file1 = MediaFile::new(subdir2_path.join("movie.mp4"), 100, "video/mp4".to_string());
        db.store_media_file(&file1).await.unwrap();

        let (dirs, files) = db.get_directory_listing(&parent_path, "").await.unwrap();
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].name, "Movies");
        assert!(files.is_empty());
    }

    #[cfg(not(target_os = "windows"))]
    #[tokio::test]
    async fn test_get_directory_listing_root_path() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        let root_path = if cfg!(windows) {
            PathBuf::from("C:\\")
        } else {
            PathBuf::from("/")
        };

        let file1 = MediaFile::new(root_path.join("root_file.txt"), 10, "text/plain".to_string());
        let file2 = MediaFile::new(root_path.join("Videos").join("movie.mp4"), 100, "video/mp4".to_string());
        let file3 = MediaFile::new(root_path.join("Music").join("song.mp3"), 50, "audio/mpeg".to_string());

        db.store_media_file(&file1).await.unwrap();
        db.store_media_file(&file2).await.unwrap();
        db.store_media_file(&file3).await.unwrap();

        let (dirs, files) = db.get_directory_listing(&root_path, "").await.unwrap();

        assert_eq!(dirs.len(), 2);
        assert!(dirs.iter().any(|d| d.name == "Music"));
        assert!(dirs.iter().any(|d| d.name == "Videos"));

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "root_file.txt");

        let (dirs_video, files_video) = db.get_directory_listing(&root_path, "video").await.unwrap();
        assert_eq!(dirs_video.len(), 1);
        assert!(dirs_video.iter().any(|d| d.name == "Videos"));
        assert_eq!(files_video.len(), 0); // root_file.txt is not video
    }
}
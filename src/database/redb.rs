//! RedbDatabase - ACID-compliant embedded database using redb
//!
//! This module provides a robust, memory-efficient database implementation
//! using the redb crate. Unlike RAM-based indexes, redb uses B-trees on disk,
//! allowing it to handle databases larger than available RAM.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use redb::{Database, ReadableTable, ReadableDatabase, TableDefinition};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::platform::DatabaseError;

use super::{
    DatabaseHealth, DatabaseIssue, DatabaseManager, DatabaseStats, IssueSeverity, MediaDirectory,
    MediaFile, MusicCategory, MusicCategoryType, Playlist,
};

// Table definitions for redb
// Primary table: stores MediaFile data keyed by ID
const FILES_TABLE: TableDefinition<i64, &[u8]> = TableDefinition::new("files");
// Index: path -> file ID (for lookups by path)
const PATH_INDEX: TableDefinition<&str, i64> = TableDefinition::new("path_index");
// Index: directory path -> list of file IDs (stored as comma-separated string)
const DIR_INDEX: TableDefinition<&str, &str> = TableDefinition::new("dir_index");
// Playlists table
const PLAYLISTS_TABLE: TableDefinition<i64, &[u8]> = TableDefinition::new("playlists");
// Playlist entries: (playlist_id, position) -> media_file_id
const PLAYLIST_ENTRIES: TableDefinition<&str, i64> = TableDefinition::new("playlist_entries");

/// RedbDatabase - ACID-compliant embedded database
pub struct RedbDatabase {
    db: Arc<RwLock<Database>>,
    db_path: PathBuf,
    next_file_id: AtomicI64,
    next_playlist_id: AtomicI64,
}

impl std::fmt::Debug for RedbDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedbDatabase")
            .field("db_path", &self.db_path)
            .field("next_file_id", &self.next_file_id.load(Ordering::Relaxed))
            .field("next_playlist_id", &self.next_playlist_id.load(Ordering::Relaxed))
            .finish()
    }
}

impl RedbDatabase {
    /// Create a new RedbDatabase at the specified path
    pub async fn new(path: PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Open or create the database
        let db = Database::create(&path)
            .with_context(|| format!("Failed to open redb database at {}", path.display()))?;

        // Initialize tables
        {
            let write_txn = db.begin_write()?;
            {
                let _ = write_txn.open_table(FILES_TABLE)?;
                let _ = write_txn.open_table(PATH_INDEX)?;
                let _ = write_txn.open_table(DIR_INDEX)?;
                let _ = write_txn.open_table(PLAYLISTS_TABLE)?;
                let _ = write_txn.open_table(PLAYLIST_ENTRIES)?;
            }
            write_txn.commit()?;
        }

        // Get max IDs for atomic counters
        let (max_file_id, max_playlist_id) = {
            let read_txn = db.begin_read()?;
            let files_table = read_txn.open_table(FILES_TABLE)?;
            let playlists_table = read_txn.open_table(PLAYLISTS_TABLE)?;

            let mut max_file: i64 = 0;
            for result in files_table.iter()? {
                if let Ok((k, _)) = result {
                    max_file = max_file.max(k.value());
                }
            }

            let mut max_playlist: i64 = 0;
            for result in playlists_table.iter()? {
                if let Ok((k, _)) = result {
                    max_playlist = max_playlist.max(k.value());
                }
            }

            (max_file, max_playlist)
        };

        info!("Opened RedbDatabase at {} (max_file_id={}, max_playlist_id={})", 
              path.display(), max_file_id, max_playlist_id);

        Ok(Self {
            db: Arc::new(RwLock::new(db)),
            db_path: path,
            next_file_id: AtomicI64::new(max_file_id + 1),
            next_playlist_id: AtomicI64::new(max_playlist_id + 1),
        })
    }

    /// Serialize a MediaFile to bytes
    fn serialize_media_file(file: &MediaFile) -> Result<Vec<u8>> {
        let json = serde_json::to_vec(&MediaFileSerializable::from(file))
            .context("Failed to serialize MediaFile")?;
        Ok(json)
    }

    /// Deserialize a MediaFile from bytes
    fn deserialize_media_file(data: &[u8]) -> Result<MediaFile> {
        let serializable: MediaFileSerializable = serde_json::from_slice(data)
            .context("Failed to deserialize MediaFile")?;
        Ok(serializable.into())
    }

    /// Serialize a Playlist to bytes
    fn serialize_playlist(playlist: &Playlist) -> Result<Vec<u8>> {
        let json = serde_json::to_vec(&PlaylistSerializable::from(playlist))
            .context("Failed to serialize Playlist")?;
        Ok(json)
    }

    /// Deserialize a Playlist from bytes
    fn deserialize_playlist(data: &[u8]) -> Result<Playlist> {
        let serializable: PlaylistSerializable = serde_json::from_slice(data)
            .context("Failed to deserialize Playlist")?;
        Ok(serializable.into())
    }

    /// Get the directory key for a path
    fn get_dir_key(path: &Path) -> String {
        path.parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    /// Add a file ID to a directory index
    fn add_to_dir_index(current: Option<&str>, file_id: i64) -> String {
        match current {
            Some(ids) if !ids.is_empty() => {
                let mut id_set: HashSet<i64> = ids.split(',').filter_map(|s| s.parse().ok()).collect();
                id_set.insert(file_id);
                id_set.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",")
            }
            _ => file_id.to_string(),
        }
    }

    /// Remove a file ID from a directory index
    fn remove_from_dir_index(current: Option<&str>, file_id: i64) -> String {
        match current {
            Some(ids) if !ids.is_empty() => {
                let id_set: HashSet<i64> = ids.split(',')
                    .filter_map(|s| s.parse().ok())
                    .filter(|&id| id != file_id)
                    .collect();
                id_set.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",")
            }
            _ => String::new(),
        }
    }

    /// Parse directory index to file IDs
    fn parse_dir_index(ids_str: &str) -> Vec<i64> {
        ids_str.split(',')
            .filter_map(|s| s.parse().ok())
            .collect()
    }
}

// Serializable versions of structs for JSON storage
#[derive(serde::Serialize, serde::Deserialize)]
struct MediaFileSerializable {
    id: Option<i64>,
    path: String,
    filename: String,
    size: u64,
    modified_secs: u64,
    mime_type: String,
    duration_secs: Option<f64>,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,
    track_number: Option<u32>,
    year: Option<u32>,
    album_artist: Option<String>,
    created_at_secs: u64,
    updated_at_secs: u64,
}

impl From<&MediaFile> for MediaFileSerializable {
    fn from(file: &MediaFile) -> Self {
        Self {
            id: file.id,
            path: file.path.to_string_lossy().to_string(),
            filename: file.filename.clone(),
            size: file.size,
            modified_secs: file.modified.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
            mime_type: file.mime_type.clone(),
            duration_secs: file.duration.map(|d| d.as_secs_f64()),
            title: file.title.clone(),
            artist: file.artist.clone(),
            album: file.album.clone(),
            genre: file.genre.clone(),
            track_number: file.track_number,
            year: file.year,
            album_artist: file.album_artist.clone(),
            created_at_secs: file.created_at.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
            updated_at_secs: file.updated_at.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        }
    }
}

impl From<MediaFileSerializable> for MediaFile {
    fn from(s: MediaFileSerializable) -> Self {
        Self {
            id: s.id,
            path: PathBuf::from(s.path),
            filename: s.filename,
            size: s.size,
            modified: UNIX_EPOCH + Duration::from_secs(s.modified_secs),
            mime_type: s.mime_type,
            duration: s.duration_secs.map(Duration::from_secs_f64),
            title: s.title,
            artist: s.artist,
            album: s.album,
            genre: s.genre,
            track_number: s.track_number,
            year: s.year,
            album_artist: s.album_artist,
            created_at: UNIX_EPOCH + Duration::from_secs(s.created_at_secs),
            updated_at: UNIX_EPOCH + Duration::from_secs(s.updated_at_secs),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PlaylistSerializable {
    id: Option<i64>,
    name: String,
    description: Option<String>,
    created_at_secs: u64,
    updated_at_secs: u64,
}

impl From<&Playlist> for PlaylistSerializable {
    fn from(playlist: &Playlist) -> Self {
        Self {
            id: playlist.id,
            name: playlist.name.clone(),
            description: playlist.description.clone(),
            created_at_secs: playlist.created_at.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
            updated_at_secs: playlist.updated_at.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        }
    }
}

impl From<PlaylistSerializable> for Playlist {
    fn from(s: PlaylistSerializable) -> Self {
        Self {
            id: s.id,
            name: s.name,
            description: s.description,
            created_at: UNIX_EPOCH + Duration::from_secs(s.created_at_secs),
            updated_at: UNIX_EPOCH + Duration::from_secs(s.updated_at_secs),
        }
    }
}

#[async_trait]
impl DatabaseManager for RedbDatabase {
    async fn initialize(&self) -> Result<()> {
        // Tables are already created in new()
        info!("RedbDatabase initialized");
        Ok(())
    }

    async fn store_media_file(&self, file: &MediaFile) -> Result<i64> {
        let file_id = file.id.unwrap_or_else(|| self.next_file_id.fetch_add(1, Ordering::SeqCst));
        let mut file_with_id = file.clone();
        file_with_id.id = Some(file_id);

        let serialized = Self::serialize_media_file(&file_with_id)?;
        let path_str = file.path.to_string_lossy().to_string();
        let dir_key = Self::get_dir_key(&file.path);
        debug!("store_media_file: storing file '{}' in dir_key '{}'", path_str, dir_key);

        let db = self.db.write().await;
        let write_txn = db.begin_write()?;
        {
            let mut files_table = write_txn.open_table(FILES_TABLE)?;
            let mut path_index = write_txn.open_table(PATH_INDEX)?;
            let mut dir_index = write_txn.open_table(DIR_INDEX)?;

            // Check if path already exists (update case)
            if let Some(existing_id) = path_index.get(path_str.as_str())?.map(|v| v.value()) {
                // Remove old entry
                files_table.remove(existing_id)?;
            }

            files_table.insert(file_id, serialized.as_slice())?;
            path_index.insert(path_str.as_str(), file_id)?;

            // Update directory index
            let current_dir_ids = dir_index.get(dir_key.as_str())?.map(|v| v.value().to_string());
            let new_dir_ids = Self::add_to_dir_index(current_dir_ids.as_deref(), file_id);
            dir_index.insert(dir_key.as_str(), new_dir_ids.as_str())?;
        }
        write_txn.commit()?;

        debug!("Stored media file {} with ID {}", path_str, file_id);
        Ok(file_id)
    }

    fn stream_all_media_files(&self) -> Pin<Box<dyn futures_util::Stream<Item = Result<MediaFile, DatabaseError>> + Send + '_>> {
        let db = self.db.clone();
        
        Box::pin(async_stream::try_stream! {
            let db_guard = db.read().await;
            let read_txn = db_guard.begin_read().map_err(|e| DatabaseError::QueryFailed { query: "begin_read".into(), reason: e.to_string() })?;
            let files_table = read_txn.open_table(FILES_TABLE).map_err(|e| DatabaseError::QueryFailed { query: "open_table".into(), reason: e.to_string() })?;

            for result in files_table.iter().map_err(|e| DatabaseError::QueryFailed { query: "iter".into(), reason: e.to_string() })? {
                let (_, value) = result.map_err(|e| DatabaseError::QueryFailed { query: "next".into(), reason: e.to_string() })?;
                let file = Self::deserialize_media_file(value.value())
                    .map_err(|e| DatabaseError::QueryFailed { query: "deserialize".into(), reason: e.to_string() })?;
                yield file;
            }
        })
    }

    async fn remove_media_file(&self, path: &Path) -> Result<bool> {
        let path_str = path.to_string_lossy().to_string();
        let dir_key = Self::get_dir_key(path);

        let db = self.db.write().await;
        let write_txn = db.begin_write()?;
        
        // First, get the file ID in a separate scope
        let file_id_opt = {
            let path_index = write_txn.open_table(PATH_INDEX)?;
            let x = path_index.get(path_str.as_str())?.map(|v| v.value()); x
        };
        
        let removed = if let Some(file_id) = file_id_opt {
            let mut files_table = write_txn.open_table(FILES_TABLE)?;
            let mut path_index = write_txn.open_table(PATH_INDEX)?;
            let mut dir_index = write_txn.open_table(DIR_INDEX)?;

            files_table.remove(file_id)?;
            path_index.remove(path_str.as_str())?;

            // Update directory index
            let current_dir_ids = dir_index.get(dir_key.as_str())?.map(|v| v.value().to_string());
            let new_dir_ids = Self::remove_from_dir_index(current_dir_ids.as_deref(), file_id);
            if new_dir_ids.is_empty() {
                dir_index.remove(dir_key.as_str())?;
            } else {
                dir_index.insert(dir_key.as_str(), new_dir_ids.as_str())?;
            }
            true
        } else {
            false
        };
        write_txn.commit()?;

        if removed {
            debug!("Removed media file: {}", path_str);
        }
        Ok(removed)
    }

    async fn update_media_file(&self, file: &MediaFile) -> Result<()> {
        let Some(file_id) = file.id else {
            return Err(anyhow!("Cannot update file without ID"));
        };

        let serialized = Self::serialize_media_file(file)?;
        let path_str = file.path.to_string_lossy().to_string();

        let db = self.db.write().await;
        let write_txn = db.begin_write()?;
        {
            let mut files_table = write_txn.open_table(FILES_TABLE)?;
            let mut path_index = write_txn.open_table(PATH_INDEX)?;

            files_table.insert(file_id, serialized.as_slice())?;
            path_index.insert(path_str.as_str(), file_id)?;
        }
        write_txn.commit()?;

        debug!("Updated media file {} with ID {}", path_str, file_id);
        Ok(())
    }

    async fn get_files_in_directory(&self, dir: &Path) -> Result<Vec<MediaFile>> {
        let dir_key = dir.to_string_lossy().to_string();

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;
        let dir_index = read_txn.open_table(DIR_INDEX)?;

        let file_ids = dir_index.get(dir_key.as_str())?
            .map(|v| Self::parse_dir_index(v.value()))
            .unwrap_or_default();

        let mut files = Vec::new();
        for file_id in file_ids {
            if let Some(data) = files_table.get(file_id)? {
                let file = Self::deserialize_media_file(data.value())?;
                files.push(file);
            }
        }

        Ok(files)
    }

    async fn get_directory_listing(
        &self,
        parent_path: &Path,
        media_type_filter: &str,
    ) -> Result<(Vec<MediaDirectory>, Vec<MediaFile>)> {
        let raw_parent_str = parent_path.to_string_lossy().to_string();
        
        // Strip trailing slash if present, unless it's the root path "/"
        let parent_str = if raw_parent_str.len() > 1 && (raw_parent_str.ends_with('/') || raw_parent_str.ends_with('\\')) {
            raw_parent_str[..raw_parent_str.len()-1].to_string()
        } else {
            raw_parent_str
        };
        
        info!("get_directory_listing: querying for parent_path='{}' (raw='{}'), filter='{}'", parent_str, parent_path.to_string_lossy(), media_type_filter);
        
        // Ensure prefix ends with a slash for subdirectory matching
        let prefix = if parent_str.is_empty() {
             String::new()
        } else if parent_str == "/" {
             "/".to_string()
        } else if !parent_str.ends_with('/') {
            format!("{}/", parent_str)
        } else {
            parent_str.clone()
        };

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;
        let dir_index = read_txn.open_table(DIR_INDEX)?;

        let mut subdirs = HashSet::new();
        let mut files = Vec::new();

        // Get files in this directory
        let file_ids = dir_index.get(parent_str.as_str())?
            .map(|v| Self::parse_dir_index(v.value()))
            .unwrap_or_default();
        
        info!("get_directory_listing: found {} file IDs for dir '{}'", file_ids.len(), parent_str);
        
        if file_ids.is_empty() {
             info!("get_directory_listing: NO FILES FOUND. Dumping available DIR_INDEX keys (limit 20):");
             let mut count = 0;
             for result in dir_index.iter()? {
                 let (key, value) = result?;
                 info!("  Key: '{}', Value len: {}", key.value(), value.value().len());
                 count += 1;
                 if count >= 20 { break; }
             }
             
             // Also try stripping trailing slash if present
             if parent_str.ends_with('/') || parent_str.ends_with('\\') {
                 let stripped = &parent_str[..parent_str.len()-1];
                 info!("get_directory_listing: Trying stripped path '{}'...", stripped);
                 if let Some(v) = dir_index.get(stripped)? {
                     info!("  SUCCESS! Found entry for stripped path '{}'", stripped);
                 } else {
                     info!("  Failed for stripped path too.");
                 }
             }
        }

        for file_id in file_ids {
            if let Some(data) = files_table.get(file_id)? {
                let file = Self::deserialize_media_file(data.value())?;
                if media_type_filter.is_empty() || file.mime_type.starts_with(media_type_filter) {
                    files.push(file);
                }
            }
        }

        // Find subdirectories
        for result in dir_index.iter()? {
            let (key, _) = result?;
            let key_str = key.value();
            if key_str.starts_with(&prefix) && key_str != parent_str {
                let relative = &key_str[prefix.len()..];
                if let Some(first_component) = relative.split('/').next() {
                    if !first_component.is_empty() {
                        let subdir_path = if prefix.is_empty() {
                            first_component.to_string()
                        } else {
                            format!("{}{}", prefix, first_component)
                        };
                        subdirs.insert(subdir_path);
                    }
                }
            }
        }

        let directories: Vec<MediaDirectory> = subdirs
            .into_iter()
            .map(|path| {
                let name = PathBuf::from(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                MediaDirectory {
                    path: PathBuf::from(&path),
                    name,
                }
            })
            .collect();

        Ok((directories, files))
    }

    async fn cleanup_missing_files(&self, existing_paths: &[PathBuf]) -> Result<usize> {
        let existing_set: HashSet<String> = existing_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        let db = self.db.write().await;
        
        // First, collect all paths to remove
        let paths_to_remove: Vec<String> = {
            let read_txn = db.begin_read()?;
            let path_index = read_txn.open_table(PATH_INDEX)?;
            
            path_index.iter()?
                .filter_map(|r| r.ok())
                .filter(|(k, _)| !existing_set.contains(k.value()))
                .map(|(k, _)| k.value().to_string())
                .collect()
        };

        let count = paths_to_remove.len();

        // Then remove them
        for path_str in &paths_to_remove {
            let write_txn = db.begin_write()?;
            
            // First, get the file ID in a separate scope
            let file_id_opt = {
                let path_index = write_txn.open_table(PATH_INDEX)?;
                let x = path_index.get(path_str.as_str())?.map(|v| v.value()); x
            };

            if let Some(file_id) = file_id_opt {
                let mut files_table = write_txn.open_table(FILES_TABLE)?;
                let mut path_index = write_txn.open_table(PATH_INDEX)?;
                let mut dir_index = write_txn.open_table(DIR_INDEX)?;

                let path = PathBuf::from(path_str);
                let dir_key = Self::get_dir_key(&path);

                files_table.remove(file_id)?;
                path_index.remove(path_str.as_str())?;

                let current_dir_ids = dir_index.get(dir_key.as_str())?.map(|v| v.value().to_string());
                let new_dir_ids = Self::remove_from_dir_index(current_dir_ids.as_deref(), file_id);
                if new_dir_ids.is_empty() {
                    dir_index.remove(dir_key.as_str())?;
                } else {
                    dir_index.insert(dir_key.as_str(), new_dir_ids.as_str())?;
                }
            }
            write_txn.commit()?;
        }

        info!("Cleaned up {} missing files", count);
        Ok(count)
    }

    async fn get_file_by_path(&self, path: &Path) -> Result<Option<MediaFile>> {
        let path_str = path.to_string_lossy().to_string();

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let path_index = read_txn.open_table(PATH_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        if let Some(file_id) = path_index.get(path_str.as_str())?.map(|v| v.value()) {
            if let Some(data) = files_table.get(file_id)? {
                return Ok(Some(Self::deserialize_media_file(data.value())?));
            }
        }

        Ok(None)
    }

    async fn get_file_by_id(&self, id: i64) -> Result<Option<MediaFile>> {
        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        if let Some(data) = files_table.get(id)? {
            return Ok(Some(Self::deserialize_media_file(data.value())?));
        }

        Ok(None)
    }

    async fn get_stats(&self) -> Result<DatabaseStats> {
        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut total_files = 0;
        let mut total_size = 0;

        for result in files_table.iter()? {
            let (_, value) = result?;
            if let Ok(file) = Self::deserialize_media_file(value.value()) {
                total_files += 1;
                total_size += file.size;
            }
        }

        let database_size = tokio::fs::metadata(&self.db_path).await
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(DatabaseStats {
            total_files,
            total_size,
            database_size,
        })
    }

    async fn check_and_repair(&self) -> Result<DatabaseHealth> {
        // Redb handles its own consistency, so we just verify we can read
        let db = self.db.read().await;
        let read_result = db.begin_read();

        let is_healthy = read_result.is_ok();
        
        Ok(DatabaseHealth {
            is_healthy,
            corruption_detected: !is_healthy,
            integrity_check_passed: is_healthy,
            issues: if is_healthy {
                Vec::new()
            } else {
                vec![DatabaseIssue {
                    severity: IssueSeverity::Critical,
                    description: "Failed to read database".to_string(),
                    table_affected: None,
                    suggested_action: "Restore from backup or reinitialize".to_string(),
                }]
            },
            repair_attempted: false,
            repair_successful: false,
        })
    }

    async fn create_backup(&self, backup_path: &Path) -> Result<()> {
        tokio::fs::copy(&self.db_path, backup_path).await?;
        info!("Created backup at {}", backup_path.display());
        Ok(())
    }

    async fn restore_from_backup(&self, backup_path: &Path) -> Result<()> {
        tokio::fs::copy(backup_path, &self.db_path).await?;
        info!("Restored from backup {}", backup_path.display());
        Ok(())
    }

    async fn vacuum(&self) -> Result<()> {
        // Redb compacts automatically, but we can trigger a compact
        let mut db = self.db.write().await;
        db.compact()?;
        info!("Compacted database");
        Ok(())
    }

    async fn get_artists(&self) -> Result<Vec<MusicCategory>> {
        let mut artists: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        for result in files_table.iter()? {
            let (_, value) = result?;
            if let Ok(file) = Self::deserialize_media_file(value.value()) {
                if let Some(artist) = file.artist {
                    *artists.entry(artist).or_insert(0) += 1;
                }
            }
        }

        Ok(artists
            .into_iter()
            .map(|(name, count)| MusicCategory {
                id: name.clone(),
                name,
                category_type: MusicCategoryType::Artist,
                count,
            })
            .collect())
    }

    async fn get_albums(&self, artist_filter: Option<&str>) -> Result<Vec<MusicCategory>> {
        let mut albums: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        for result in files_table.iter()? {
            let (_, value) = result?;
            if let Ok(file) = Self::deserialize_media_file(value.value()) {
                if let Some(album) = file.album {
                    if let Some(filter) = artist_filter {
                        if file.artist.as_deref() != Some(filter) {
                            continue;
                        }
                    }
                    *albums.entry(album).or_insert(0) += 1;
                }
            }
        }

        Ok(albums
            .into_iter()
            .map(|(name, count)| MusicCategory {
                id: name.clone(),
                name,
                category_type: MusicCategoryType::Album,
                count,
            })
            .collect())
    }

    async fn get_genres(&self) -> Result<Vec<MusicCategory>> {
        let mut genres: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        for result in files_table.iter()? {
            let (_, value) = result?;
            if let Ok(file) = Self::deserialize_media_file(value.value()) {
                if let Some(genre) = file.genre {
                    *genres.entry(genre).or_insert(0) += 1;
                }
            }
        }

        Ok(genres
            .into_iter()
            .map(|(name, count)| MusicCategory {
                id: name.clone(),
                name,
                category_type: MusicCategoryType::Genre,
                count,
            })
            .collect())
    }

    async fn get_years(&self) -> Result<Vec<MusicCategory>> {
        let mut years: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        for result in files_table.iter()? {
            let (_, value) = result?;
            if let Ok(file) = Self::deserialize_media_file(value.value()) {
                if let Some(year) = file.year {
                    *years.entry(year).or_insert(0) += 1;
                }
            }
        }

        Ok(years
            .into_iter()
            .map(|(year, count)| MusicCategory {
                id: year.to_string(),
                name: year.to_string(),
                category_type: MusicCategoryType::Year,
                count,
            })
            .collect())
    }

    async fn get_album_artists(&self) -> Result<Vec<MusicCategory>> {
        let mut album_artists: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        for result in files_table.iter()? {
            let (_, value) = result?;
            if let Ok(file) = Self::deserialize_media_file(value.value()) {
                if let Some(album_artist) = file.album_artist {
                    *album_artists.entry(album_artist).or_insert(0) += 1;
                }
            }
        }

        Ok(album_artists
            .into_iter()
            .map(|(name, count)| MusicCategory {
                id: name.clone(),
                name,
                category_type: MusicCategoryType::AlbumArtist,
                count,
            })
            .collect())
    }

    async fn get_music_by_artist(&self, artist: &str) -> Result<Vec<MediaFile>> {
        let mut files = Vec::new();

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        for result in files_table.iter()? {
            let (_, value) = result?;
            if let Ok(file) = Self::deserialize_media_file(value.value()) {
                if file.artist.as_deref() == Some(artist) {
                    files.push(file);
                }
            }
        }

        Ok(files)
    }

    async fn get_music_by_album(&self, album: &str, artist: Option<&str>) -> Result<Vec<MediaFile>> {
        let mut files = Vec::new();

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        for result in files_table.iter()? {
            let (_, value) = result?;
            if let Ok(file) = Self::deserialize_media_file(value.value()) {
                if file.album.as_deref() == Some(album) {
                    if let Some(art) = artist {
                        if file.artist.as_deref() != Some(art) {
                            continue;
                        }
                    }
                    files.push(file);
                }
            }
        }

        Ok(files)
    }

    async fn get_music_by_genre(&self, genre: &str) -> Result<Vec<MediaFile>> {
        let mut files = Vec::new();

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        for result in files_table.iter()? {
            let (_, value) = result?;
            if let Ok(file) = Self::deserialize_media_file(value.value()) {
                if file.genre.as_deref() == Some(genre) {
                    files.push(file);
                }
            }
        }

        Ok(files)
    }

    async fn get_music_by_year(&self, year: u32) -> Result<Vec<MediaFile>> {
        let mut files = Vec::new();

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        for result in files_table.iter()? {
            let (_, value) = result?;
            if let Ok(file) = Self::deserialize_media_file(value.value()) {
                if file.year == Some(year) {
                    files.push(file);
                }
            }
        }

        Ok(files)
    }

    async fn get_music_by_album_artist(&self, album_artist: &str) -> Result<Vec<MediaFile>> {
        let mut files = Vec::new();

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        for result in files_table.iter()? {
            let (_, value) = result?;
            if let Ok(file) = Self::deserialize_media_file(value.value()) {
                if file.album_artist.as_deref() == Some(album_artist) {
                    files.push(file);
                }
            }
        }

        Ok(files)
    }

    async fn create_playlist(&self, name: &str, description: Option<&str>) -> Result<i64> {
        let playlist_id = self.next_playlist_id.fetch_add(1, Ordering::SeqCst);
        let now = SystemTime::now();
        
        let playlist = Playlist {
            id: Some(playlist_id),
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        };

        let serialized = Self::serialize_playlist(&playlist)?;

        let db = self.db.write().await;
        let write_txn = db.begin_write()?;
        {
            let mut playlists_table = write_txn.open_table(PLAYLISTS_TABLE)?;
            playlists_table.insert(playlist_id, serialized.as_slice())?;
        }
        write_txn.commit()?;

        info!("Created playlist '{}' with ID {}", name, playlist_id);
        Ok(playlist_id)
    }

    async fn get_playlists(&self) -> Result<Vec<Playlist>> {
        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let playlists_table = read_txn.open_table(PLAYLISTS_TABLE)?;

        let mut playlists = Vec::new();
        for result in playlists_table.iter()? {
            let (_, value) = result?;
            if let Ok(playlist) = Self::deserialize_playlist(value.value()) {
                playlists.push(playlist);
            }
        }

        Ok(playlists)
    }

    async fn get_playlist(&self, playlist_id: i64) -> Result<Option<Playlist>> {
        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let playlists_table = read_txn.open_table(PLAYLISTS_TABLE)?;

        if let Some(data) = playlists_table.get(playlist_id)? {
            return Ok(Some(Self::deserialize_playlist(data.value())?));
        }

        Ok(None)
    }

    async fn update_playlist(&self, playlist: &Playlist) -> Result<()> {
        let Some(playlist_id) = playlist.id else {
            return Err(anyhow!("Cannot update playlist without ID"));
        };

        let serialized = Self::serialize_playlist(playlist)?;

        let db = self.db.write().await;
        let write_txn = db.begin_write()?;
        {
            let mut playlists_table = write_txn.open_table(PLAYLISTS_TABLE)?;
            playlists_table.insert(playlist_id, serialized.as_slice())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    async fn delete_playlist(&self, playlist_id: i64) -> Result<bool> {
        let db = self.db.write().await;
        let write_txn = db.begin_write()?;
        let removed = {
            let mut playlists_table = write_txn.open_table(PLAYLISTS_TABLE)?;
            let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;

            let existed = playlists_table.remove(playlist_id)?.is_some();

            // Remove all entries for this playlist
            let prefix = format!("{}:", playlist_id);
            let keys_to_remove: Vec<String> = playlist_entries.iter()?
                .filter_map(|r| r.ok())
                .filter(|(k, _)| k.value().starts_with(&prefix))
                .map(|(k, _)| k.value().to_string())
                .collect();

            for key in keys_to_remove {
                playlist_entries.remove(key.as_str())?;
            }

            existed
        };
        write_txn.commit()?;

        Ok(removed)
    }

    async fn add_to_playlist(&self, playlist_id: i64, media_file_id: i64, position: Option<u32>) -> Result<i64> {
        let pos = position.unwrap_or(0);
        let key = format!("{}:{}", playlist_id, pos);

        let db = self.db.write().await;
        let write_txn = db.begin_write()?;
        {
            let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
            playlist_entries.insert(key.as_str(), media_file_id)?;
        }
        write_txn.commit()?;

        Ok(media_file_id)
    }

    async fn batch_add_to_playlist(&self, playlist_id: i64, media_file_ids: &[(i64, u32)]) -> Result<Vec<i64>> {
        let db = self.db.write().await;
        let write_txn = db.begin_write()?;
        {
            let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
            for (file_id, position) in media_file_ids {
                let key = format!("{}:{}", playlist_id, position);
                playlist_entries.insert(key.as_str(), *file_id)?;
            }
        }
        write_txn.commit()?;

        Ok(media_file_ids.iter().map(|(id, _)| *id).collect())
    }

    async fn get_files_by_paths(&self, paths: &[PathBuf]) -> Result<Vec<MediaFile>> {
        let mut files = Vec::new();

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let path_index = read_txn.open_table(PATH_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        for path in paths {
            let path_str = path.to_string_lossy().to_string();
            if let Some(file_id) = path_index.get(path_str.as_str())?.map(|v| v.value()) {
                if let Some(data) = files_table.get(file_id)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
        }

        Ok(files)
    }

    async fn bulk_store_media_files(&self, files: &[MediaFile]) -> Result<Vec<i64>> {
        let mut ids = Vec::with_capacity(files.len());

        let db = self.db.write().await;
        let write_txn = db.begin_write()?;
        {
            let mut files_table = write_txn.open_table(FILES_TABLE)?;
            let mut path_index = write_txn.open_table(PATH_INDEX)?;
            let mut dir_index = write_txn.open_table(DIR_INDEX)?;

            for file in files {
                let file_id = file.id.unwrap_or_else(|| self.next_file_id.fetch_add(1, Ordering::SeqCst));
                ids.push(file_id);

                let mut file_with_id = file.clone();
                file_with_id.id = Some(file_id);

                let serialized = Self::serialize_media_file(&file_with_id)?;
                let path_str = file.path.to_string_lossy().to_string();
                let dir_key = Self::get_dir_key(&file.path);

                // Check if path already exists
                if let Some(existing_id) = path_index.get(path_str.as_str())?.map(|v| v.value()) {
                    files_table.remove(existing_id)?;
                }

                files_table.insert(file_id, serialized.as_slice())?;
                path_index.insert(path_str.as_str(), file_id)?;

                let current_dir_ids = dir_index.get(dir_key.as_str())?.map(|v| v.value().to_string());
                let new_dir_ids = Self::add_to_dir_index(current_dir_ids.as_deref(), file_id);
                dir_index.insert(dir_key.as_str(), new_dir_ids.as_str())?;
            }
        }
        write_txn.commit()?;

        debug!("Bulk stored {} media files", ids.len());
        Ok(ids)
    }

    async fn bulk_update_media_files(&self, files: &[MediaFile]) -> Result<()> {
        let db = self.db.write().await;
        let write_txn = db.begin_write()?;
        {
            let mut files_table = write_txn.open_table(FILES_TABLE)?;
            let mut path_index = write_txn.open_table(PATH_INDEX)?;

            for file in files {
                if let Some(file_id) = file.id {
                    let serialized = Self::serialize_media_file(file)?;
                    let path_str = file.path.to_string_lossy().to_string();

                    files_table.insert(file_id, serialized.as_slice())?;
                    path_index.insert(path_str.as_str(), file_id)?;
                }
            }
        }
        write_txn.commit()?;

        debug!("Bulk updated {} media files", files.len());
        Ok(())
    }

    async fn bulk_remove_media_files(&self, paths: &[PathBuf]) -> Result<usize> {
        let mut count = 0;

        let db = self.db.write().await;
        let write_txn = db.begin_write()?;
        
        // First pass: collect file IDs for all paths
        let file_ids_and_info: Vec<(i64, String, String)> = {
            let path_index = write_txn.open_table(PATH_INDEX)?;
            let mut result = Vec::new();
            for path in paths {
                let path_str = path.to_string_lossy().to_string();
                let dir_key = Self::get_dir_key(path);
                if let Some(file_id) = {
                    let x = path_index.get(path_str.as_str())?.map(|v| v.value()); x
                } {
                    result.push((file_id, path_str, dir_key));
                }
            }
            result
        };

        // Second pass: remove all entries
        {
            let mut files_table = write_txn.open_table(FILES_TABLE)?;
            let mut path_index = write_txn.open_table(PATH_INDEX)?;
            let mut dir_index = write_txn.open_table(DIR_INDEX)?;

            for (file_id, path_str, dir_key) in &file_ids_and_info {
                files_table.remove(*file_id)?;
                path_index.remove(path_str.as_str())?;

                let current_dir_ids = {
                    let x = dir_index.get(dir_key.as_str())?.map(|v| v.value().to_string()); x
                };
                let new_dir_ids = Self::remove_from_dir_index(current_dir_ids.as_deref(), *file_id);
                if new_dir_ids.is_empty() {
                    dir_index.remove(dir_key.as_str())?;
                } else {
                    dir_index.insert(dir_key.as_str(), new_dir_ids.as_str())?;
                }
                count += 1;
            }
        }
        write_txn.commit()?;

        debug!("Bulk removed {} media files", count);
        Ok(count)
    }

    async fn remove_from_playlist(&self, playlist_id: i64, media_file_id: i64) -> Result<bool> {
        let db = self.db.write().await;
        let write_txn = db.begin_write()?;
        let removed = {
            let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
            
            let prefix = format!("{}:", playlist_id);
            let key_to_remove: Option<String> = playlist_entries.iter()?
                .filter_map(|r| r.ok())
                .find(|(k, v)| k.value().starts_with(&prefix) && v.value() == media_file_id)
                .map(|(k, _)| k.value().to_string());

            if let Some(key) = key_to_remove {
                playlist_entries.remove(key.as_str())?;
                true
            } else {
                false
            }
        };
        write_txn.commit()?;

        Ok(removed)
    }

    async fn get_playlist_tracks(&self, playlist_id: i64) -> Result<Vec<MediaFile>> {
        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let playlist_entries = read_txn.open_table(PLAYLIST_ENTRIES)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let prefix = format!("{}:", playlist_id);
        let mut entries: Vec<(u32, i64)> = playlist_entries.iter()?
            .filter_map(|r| r.ok())
            .filter(|(k, _)| k.value().starts_with(&prefix))
            .map(|(k, v)| {
                let pos: u32 = k.value()[prefix.len()..].parse().unwrap_or(0);
                (pos, v.value())
            })
            .collect();

        entries.sort_by_key(|(pos, _)| *pos);

        let mut files = Vec::new();
        for (_, file_id) in entries {
            if let Some(data) = files_table.get(file_id)? {
                files.push(Self::deserialize_media_file(data.value())?);
            }
        }

        Ok(files)
    }

    async fn reorder_playlist(&self, playlist_id: i64, track_positions: &[(i64, u32)]) -> Result<()> {
        let db = self.db.write().await;
        let write_txn = db.begin_write()?;
        {
            let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
            
            // Remove existing entries for this playlist
            let prefix = format!("{}:", playlist_id);
            let keys_to_remove: Vec<String> = playlist_entries.iter()?
                .filter_map(|r| r.ok())
                .filter(|(k, _)| k.value().starts_with(&prefix))
                .map(|(k, _)| k.value().to_string())
                .collect();

            for key in keys_to_remove {
                playlist_entries.remove(key.as_str())?;
            }

            // Insert new order
            for (file_id, position) in track_positions {
                let key = format!("{}:{}", playlist_id, position);
                playlist_entries.insert(key.as_str(), *file_id)?;
            }
        }
        write_txn.commit()?;

        Ok(())
    }

    async fn get_files_with_path_prefix(&self, canonical_prefix: &str) -> Result<Vec<MediaFile>> {
        let mut files = Vec::new();

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let path_index = read_txn.open_table(PATH_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        for result in path_index.iter()? {
            let (key, value) = result?;
            if key.value().starts_with(canonical_prefix) {
                if let Some(data) = files_table.get(value.value())? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
        }

        Ok(files)
    }

    async fn get_direct_subdirectories(&self, canonical_parent_path: &str) -> Result<Vec<MediaDirectory>> {
        let prefix = if canonical_parent_path.is_empty() || canonical_parent_path == "/" {
            String::new()
        } else {
            format!("{}/", canonical_parent_path)
        };

        let db = self.db.read().await;
        let read_txn = db.begin_read()?;
        let dir_index = read_txn.open_table(DIR_INDEX)?;

        let mut subdirs = HashSet::new();

        for result in dir_index.iter()? {
            let (key, _) = result?;
            let key_str = key.value();
            if key_str.starts_with(&prefix) && key_str != canonical_parent_path {
                let relative = &key_str[prefix.len()..];
                if let Some(first_component) = relative.split('/').next() {
                    if !first_component.is_empty() {
                        let subdir_path = if prefix.is_empty() {
                            first_component.to_string()
                        } else {
                            format!("{}{}", prefix, first_component)
                        };
                        subdirs.insert(subdir_path);
                    }
                }
            }
        }

        Ok(subdirs
            .into_iter()
            .map(|path| {
                let name = PathBuf::from(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                MediaDirectory {
                    path: PathBuf::from(&path),
                    name,
                }
            })
            .collect())
    }

    async fn batch_cleanup_missing_files(&self, existing_canonical_paths: &HashSet<String>) -> Result<usize> {
        let paths_vec: Vec<PathBuf> = existing_canonical_paths
            .iter()
            .map(|s| PathBuf::from(s))
            .collect();
        self.cleanup_missing_files(&paths_vec).await
    }

    async fn database_native_cleanup(&self, existing_canonical_paths: &[String]) -> Result<usize> {
        let existing_set: HashSet<String> = existing_canonical_paths.iter().cloned().collect();
        let paths_vec: Vec<PathBuf> = existing_set.iter().map(|s| PathBuf::from(s)).collect();
        self.cleanup_missing_files(&paths_vec).await
    }

    async fn get_filtered_direct_subdirectories(
        &self,
        canonical_parent_path: &str,
        _mime_filter: &str,
    ) -> Result<Vec<MediaDirectory>> {
        // For now, just return all subdirectories and let the caller filter
        // A more optimized version would filter during iteration
        self.get_direct_subdirectories(canonical_parent_path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_redb_database_basic_operations() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.redb");

        let db = RedbDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        // Store a file
        let file = MediaFile::new(
            PathBuf::from("/music/test.mp3"),
            1024,
            "audio/mpeg".to_string(),
        );
        let id = db.store_media_file(&file).await.unwrap();
        assert!(id > 0);

        // Retrieve by path
        let retrieved = db.get_file_by_path(&PathBuf::from("/music/test.mp3")).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().filename, "test.mp3");

        // Retrieve by ID
        let by_id = db.get_file_by_id(id).await.unwrap();
        assert!(by_id.is_some());

        // Remove
        let removed = db.remove_media_file(&PathBuf::from("/music/test.mp3")).await.unwrap();
        assert!(removed);

        let removed_check = db.get_file_by_path(&PathBuf::from("/music/test.mp3")).await.unwrap();
        assert!(removed_check.is_none());
    }

    #[tokio::test]
    async fn test_redb_database_bulk_operations() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_bulk.redb");

        let db = RedbDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        let files: Vec<MediaFile> = (0..100)
            .map(|i| MediaFile::new(
                PathBuf::from(format!("/music/song{}.mp3", i)),
                1024,
                "audio/mpeg".to_string(),
            ))
            .collect();

        let ids = db.bulk_store_media_files(&files).await.unwrap();
        assert_eq!(ids.len(), 100);

        let stats = db.get_stats().await.unwrap();
        assert_eq!(stats.total_files, 100);
    }

    #[tokio::test]
    async fn test_redb_database_playlist_operations() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_playlist.redb");

        let db = RedbDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        // Store some test files
        let files: Vec<MediaFile> = (0..5)
            .map(|i| MediaFile::new(
                PathBuf::from(format!("/music/song{}.mp3", i)),
                1024,
                "audio/mpeg".to_string(),
            ))
            .collect();
        let file_ids = db.bulk_store_media_files(&files).await.unwrap();

        // Create a playlist
        let playlist_id = db.create_playlist("Test Playlist", Some("A test playlist")).await.unwrap();
        assert!(playlist_id > 0);

        // Verify playlist was created
        let playlists = db.get_playlists().await.unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].name, "Test Playlist");

        // Add tracks to playlist
        for (i, file_id) in file_ids.iter().enumerate() {
            db.add_to_playlist(playlist_id, *file_id, Some(i as u32)).await.unwrap();
        }

        // Get playlist tracks
        let tracks = db.get_playlist_tracks(playlist_id).await.unwrap();
        assert_eq!(tracks.len(), 5);
        assert_eq!(tracks[0].filename, "song0.mp3");
        assert_eq!(tracks[4].filename, "song4.mp3");

        // Remove a track
        let removed = db.remove_from_playlist(playlist_id, file_ids[2]).await.unwrap();
        assert!(removed);

        // Verify track was removed
        let tracks_after_remove = db.get_playlist_tracks(playlist_id).await.unwrap();
        assert_eq!(tracks_after_remove.len(), 4);

        // Delete the playlist
        let deleted = db.delete_playlist(playlist_id).await.unwrap();
        assert!(deleted);

        // Verify playlist was deleted
        let playlists_after_delete = db.get_playlists().await.unwrap();
        assert_eq!(playlists_after_delete.len(), 0);
    }
}

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
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
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

// Secondary index tables (Value is comma-separated string of file IDs, e.g. "1,2,3")
const ARTIST_INDEX: TableDefinition<&str, &str> = TableDefinition::new("artist_index");
const ALBUM_INDEX: TableDefinition<&str, &str> = TableDefinition::new("album_index");
const GENRE_INDEX: TableDefinition<&str, &str> = TableDefinition::new("genre_index");
const YEAR_INDEX: TableDefinition<u32, &str> = TableDefinition::new("year_index");
const ALBUM_ARTIST_INDEX: TableDefinition<&str, &str> = TableDefinition::new("album_artist_index");

/// RedbDatabase - ACID-compliant embedded database
pub struct RedbDatabase {
    db: Arc<Database>,
    db_path: PathBuf,
    next_file_id: AtomicI64,
    next_playlist_id: AtomicI64,
    total_files: AtomicU64,
    total_size: AtomicU64,
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

        // Open database, checking for incompatible/corrupt format
        let db = match Database::create(&path) {
            Ok(db) => {
                let init_res = {
                    let write_txn = db.begin_write()?;
                    let res = write_txn.open_table(FILES_TABLE).and_then(|_| {
                        write_txn.open_table(PATH_INDEX)?;
                        write_txn.open_table(DIR_INDEX)?;
                        write_txn.open_table(PLAYLISTS_TABLE)?;
                        write_txn.open_table(PLAYLIST_ENTRIES)?;
                        write_txn.open_table(ARTIST_INDEX)?;
                        write_txn.open_table(ALBUM_INDEX)?;
                        write_txn.open_table(GENRE_INDEX)?;
                        write_txn.open_table(YEAR_INDEX)?;
                        write_txn.open_table(ALBUM_ARTIST_INDEX)?;
                        Ok(())
                    });
                    write_txn.abort()?;
                    res
                };
                if init_res.is_err() {
                    Err(anyhow!("Database schema mismatch or legacy format detected"))
                } else {
                    Ok(db)
                }
            }
            Err(e) => Err(anyhow::Error::from(e)),
        };

        let db = match db {
            Ok(db) => db,
            Err(e) => {
                info!("Database validation failed: {}. Recreating fresh database at {}", e, path.display());
                if path.exists() {
                    let _ = std::fs::remove_file(&path);
                }
                Database::create(&path)
                    .with_context(|| format!("Failed to recreate redb database at {}", path.display()))?
            }
        };

        // Initialize tables if they don't exist
        {
            let write_txn = db.begin_write()?;
            {
                let _ = write_txn.open_table(FILES_TABLE)?;
                let _ = write_txn.open_table(PATH_INDEX)?;
                let _ = write_txn.open_table(DIR_INDEX)?;
                let _ = write_txn.open_table(PLAYLISTS_TABLE)?;
                let _ = write_txn.open_table(PLAYLIST_ENTRIES)?;
                let _ = write_txn.open_table(ARTIST_INDEX)?;
                let _ = write_txn.open_table(ALBUM_INDEX)?;
                let _ = write_txn.open_table(GENRE_INDEX)?;
                let _ = write_txn.open_table(YEAR_INDEX)?;
                let _ = write_txn.open_table(ALBUM_ARTIST_INDEX)?;
            }
            write_txn.commit()?;
        }

        // Get max IDs and stats for atomic counters
        let (max_file_id, max_playlist_id, total_files_count, total_size_sum) = {
            let read_txn = db.begin_read()?;
            let files_table = read_txn.open_table(FILES_TABLE)?;
            let playlists_table = read_txn.open_table(PLAYLISTS_TABLE)?;

            let mut max_file: i64 = 0;
            let mut total_files_c: u64 = 0;
            let mut total_size_s: u64 = 0;
            for result in files_table.iter()? {
                if let Ok((k, v)) = result {
                    max_file = max_file.max(k.value());
                    total_files_c += 1;
                    if let Ok(file) = Self::deserialize_media_file(v.value()) {
                        total_size_s += file.size;
                    }
                }
            }

            let mut max_playlist: i64 = 0;
            for result in playlists_table.iter()? {
                if let Ok((k, _)) = result {
                    max_playlist = max_playlist.max(k.value());
                }
            }

            (max_file, max_playlist, total_files_c, total_size_s)
        };

        info!(
            "Opened RedbDatabase at {} (max_file_id={}, max_playlist_id={}, files={}, size={} bytes)",
            path.display(),
            max_file_id,
            max_playlist_id,
            total_files_count,
            total_size_sum
        );

        Ok(Self {
            db: Arc::new(db),
            db_path: path,
            next_file_id: AtomicI64::new(max_file_id + 1),
            next_playlist_id: AtomicI64::new(max_playlist_id + 1),
            total_files: AtomicU64::new(total_files_count),
            total_size: AtomicU64::new(total_size_sum),
        })
    }

    /// Serialize a MediaFile to bytes using bitcode
    fn serialize_media_file(file: &MediaFile) -> Result<Vec<u8>> {
        let data = bitcode::encode(&MediaFileSerializable::from(file));
        Ok(data)
    }

    /// Deserialize a MediaFile from bytes using bitcode
    fn deserialize_media_file(data: &[u8]) -> Result<MediaFile> {
        let serializable: MediaFileSerializable = bitcode::decode(data)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize MediaFile using bitcode: {}", e))?;
        Ok(serializable.into())
    }

    /// Serialize a Playlist to bytes using bitcode
    fn serialize_playlist(playlist: &Playlist) -> Result<Vec<u8>> {
        let data = bitcode::encode(&PlaylistSerializable::from(playlist));
        Ok(data)
    }

    /// Deserialize a Playlist from bytes using bitcode
    fn deserialize_playlist(data: &[u8]) -> Result<Playlist> {
        let serializable: PlaylistSerializable = bitcode::decode(data)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize Playlist using bitcode: {}", e))?;
        Ok(serializable.into())
    }

    /// Get the directory key for a path
    fn get_dir_key(path: &Path) -> String {
        path.parent()
            .map(|p| {
                let s = p.to_string_lossy().to_string().replace('\\', "/");
                if cfg!(target_os = "windows") {
                    s.to_lowercase()
                } else {
                    s
                }
            })
            .unwrap_or_default()
    }

    /// Add a file ID to a directory index
    fn add_to_dir_index(current: Option<&str>, file_id: i64) -> String {
        match current {
            Some(ids) if !ids.is_empty() => {
                let mut id_set: HashSet<i64> = ids.split(',').filter_map(|s| s.parse().ok()).collect();
                id_set.insert(file_id);
                let mut v: Vec<_> = id_set.into_iter().collect();
                v.sort();
                v.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",")
            }
            _ => file_id.to_string(),
        }
    }

    /// Remove a file ID from a directory index
    fn remove_from_dir_index(current: Option<&str>, file_id: i64) -> String {
        match current {
            Some(ids) if !ids.is_empty() => {
                let mut id_set: HashSet<i64> = ids.split(',')
                    .filter_map(|s| s.parse().ok())
                    .collect();
                id_set.remove(&file_id);
                let mut v: Vec<_> = id_set.into_iter().collect();
                v.sort();
                v.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",")
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

    fn update_category_index(
        table: &mut redb::Table<&str, &str>,
        key: &str,
        file_id: i64,
        add: bool,
    ) -> Result<()> {
        let current = table.get(key)?.map(|v| v.value().to_string());
        let new_val = if add {
            Self::add_to_id_list(current.as_deref(), file_id)
        } else {
            Self::remove_from_id_list(current.as_deref(), file_id)
        };
        if new_val.is_empty() {
            table.remove(key)?;
        } else {
            table.insert(key, new_val.as_str())?;
        }
        Ok(())
    }

    fn update_year_index(
        table: &mut redb::Table<u32, &str>,
        key: u32,
        file_id: i64,
        add: bool,
    ) -> Result<()> {
        let current = table.get(key)?.map(|v| v.value().to_string());
        let new_val = if add {
            Self::add_to_id_list(current.as_deref(), file_id)
        } else {
            Self::remove_from_id_list(current.as_deref(), file_id)
        };
        if new_val.is_empty() {
            table.remove(key)?;
        } else {
            table.insert(key, new_val.as_str())?;
        }
        Ok(())
    }

    fn add_to_id_list(current: Option<&str>, file_id: i64) -> String {
        match current {
            Some(ids) if !ids.is_empty() => {
                let mut id_set: HashSet<i64> = ids.split(',').filter_map(|s| s.parse().ok()).collect();
                id_set.insert(file_id);
                let mut v: Vec<_> = id_set.into_iter().collect();
                v.sort();
                v.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",")
            }
            _ => file_id.to_string(),
        }
    }

    fn remove_from_id_list(current: Option<&str>, file_id: i64) -> String {
        match current {
            Some(ids) if !ids.is_empty() => {
                let mut id_set: HashSet<i64> = ids.split(',')
                    .filter_map(|s| s.parse().ok())
                    .collect();
                id_set.remove(&file_id);
                let mut v: Vec<_> = id_set.into_iter().collect();
                v.sort();
                v.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",")
            }
            _ => String::new(),
        }
    }
}

// Serializable versions of structs for bitcode storage
#[derive(serde::Serialize, serde::Deserialize, bitcode::Encode, bitcode::Decode)]
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

#[derive(serde::Serialize, serde::Deserialize, bitcode::Encode, bitcode::Decode)]
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

        let write_txn = self.db.begin_write()?;
        {
            let mut files_table = write_txn.open_table(FILES_TABLE)?;
            let mut path_index = write_txn.open_table(PATH_INDEX)?;
            let mut dir_index = write_txn.open_table(DIR_INDEX)?;

            let mut artist_index = write_txn.open_table(ARTIST_INDEX)?;
            let mut album_index = write_txn.open_table(ALBUM_INDEX)?;
            let mut genre_index = write_txn.open_table(GENRE_INDEX)?;
            let mut year_index = write_txn.open_table(YEAR_INDEX)?;
            let mut album_artist_index = write_txn.open_table(ALBUM_ARTIST_INDEX)?;

            // Check if path already exists (update case)
            if let Some(existing_id) = path_index.get(path_str.as_str())?.map(|v| v.value()) {
                if let Some(old_data) = files_table.get(existing_id)? {
                    if let Ok(old_file) = Self::deserialize_media_file(old_data.value()) {
                        self.total_size.fetch_sub(old_file.size, Ordering::SeqCst);
                        self.total_files.fetch_sub(1, Ordering::SeqCst);

                        // Remove old secondary indexes
                        if let Some(artist) = &old_file.artist {
                            Self::update_category_index(&mut artist_index, artist, existing_id, false)?;
                        }
                        if let Some(album) = &old_file.album {
                            Self::update_category_index(&mut album_index, album, existing_id, false)?;
                        }
                        if let Some(genre) = &old_file.genre {
                            Self::update_category_index(&mut genre_index, genre, existing_id, false)?;
                        }
                        if let Some(year) = old_file.year {
                            Self::update_year_index(&mut year_index, year, existing_id, false)?;
                        }
                        if let Some(album_artist) = &old_file.album_artist {
                            Self::update_category_index(&mut album_artist_index, album_artist, existing_id, false)?;
                        }
                    }
                }
                files_table.remove(existing_id)?;
            }

            files_table.insert(file_id, serialized.as_slice())?;
            path_index.insert(path_str.as_str(), file_id)?;

            // Update directory index
            let current_dir_ids = dir_index.get(dir_key.as_str())?.map(|v| v.value().to_string());
            let new_dir_ids = Self::add_to_dir_index(current_dir_ids.as_deref(), file_id);
            dir_index.insert(dir_key.as_str(), new_dir_ids.as_str())?;

            // Add new secondary indexes
            if let Some(artist) = &file.artist {
                Self::update_category_index(&mut artist_index, artist, file_id, true)?;
            }
            if let Some(album) = &file.album {
                Self::update_category_index(&mut album_index, album, file_id, true)?;
            }
            if let Some(genre) = &file.genre {
                Self::update_category_index(&mut genre_index, genre, file_id, true)?;
            }
            if let Some(year) = file.year {
                Self::update_year_index(&mut year_index, year, file_id, true)?;
            }
            if let Some(album_artist) = &file.album_artist {
                Self::update_category_index(&mut album_artist_index, album_artist, file_id, true)?;
            }

            self.total_size.fetch_add(file.size, Ordering::SeqCst);
            self.total_files.fetch_add(1, Ordering::SeqCst);
        }
        write_txn.commit()?;

        debug!("Stored media file {} with ID {}", path_str, file_id);
        Ok(file_id)
    }

    fn stream_all_media_files(&self) -> Pin<Box<dyn futures_util::Stream<Item = Result<MediaFile, DatabaseError>> + Send + '_>> {
        let db = self.db.clone();
        
        Box::pin(async_stream::try_stream! {
            let read_txn = db.begin_read().map_err(|e| DatabaseError::QueryFailed { query: "begin_read".into(), reason: e.to_string() })?;
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

        let write_txn = self.db.begin_write()?;
        
        // First, get the file ID in a separate scope
        let file_id_opt = {
            let path_index = write_txn.open_table(PATH_INDEX)?;
            let guard = path_index.get(path_str.as_str())?;
            guard.map(|v| v.value())
        };
        
        let removed = if let Some(file_id) = file_id_opt {
            let mut files_table = write_txn.open_table(FILES_TABLE)?;
            let mut path_index = write_txn.open_table(PATH_INDEX)?;
            let mut dir_index = write_txn.open_table(DIR_INDEX)?;

            let mut artist_index = write_txn.open_table(ARTIST_INDEX)?;
            let mut album_index = write_txn.open_table(ALBUM_INDEX)?;
            let mut genre_index = write_txn.open_table(GENRE_INDEX)?;
            let mut year_index = write_txn.open_table(YEAR_INDEX)?;
            let mut album_artist_index = write_txn.open_table(ALBUM_ARTIST_INDEX)?;

            if let Some(old_data) = files_table.get(file_id)? {
                if let Ok(old_file) = Self::deserialize_media_file(old_data.value()) {
                    self.total_size.fetch_sub(old_file.size, Ordering::SeqCst);
                    self.total_files.fetch_sub(1, Ordering::SeqCst);

                    if let Some(artist) = &old_file.artist {
                        Self::update_category_index(&mut artist_index, artist, file_id, false)?;
                    }
                    if let Some(album) = &old_file.album {
                        Self::update_category_index(&mut album_index, album, file_id, false)?;
                    }
                    if let Some(genre) = &old_file.genre {
                        Self::update_category_index(&mut genre_index, genre, file_id, false)?;
                    }
                    if let Some(year) = old_file.year {
                        Self::update_year_index(&mut year_index, year, file_id, false)?;
                    }
                    if let Some(album_artist) = &old_file.album_artist {
                        Self::update_category_index(&mut album_artist_index, album_artist, file_id, false)?;
                    }
                }
            }

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

        let write_txn = self.db.begin_write()?;
        {
            let mut files_table = write_txn.open_table(FILES_TABLE)?;
            let mut path_index = write_txn.open_table(PATH_INDEX)?;
            let mut artist_index = write_txn.open_table(ARTIST_INDEX)?;
            let mut album_index = write_txn.open_table(ALBUM_INDEX)?;
            let mut genre_index = write_txn.open_table(GENRE_INDEX)?;
            let mut year_index = write_txn.open_table(YEAR_INDEX)?;
            let mut album_artist_index = write_txn.open_table(ALBUM_ARTIST_INDEX)?;

            if let Some(old_data) = files_table.get(file_id)? {
                if let Ok(old_file) = Self::deserialize_media_file(old_data.value()) {
                    self.total_size.fetch_sub(old_file.size, Ordering::SeqCst);
                    self.total_files.fetch_sub(1, Ordering::SeqCst);

                    // Remove old secondary indexes
                    if let Some(artist) = &old_file.artist {
                        Self::update_category_index(&mut artist_index, artist, file_id, false)?;
                    }
                    if let Some(album) = &old_file.album {
                        Self::update_category_index(&mut album_index, album, file_id, false)?;
                    }
                    if let Some(genre) = &old_file.genre {
                        Self::update_category_index(&mut genre_index, genre, file_id, false)?;
                    }
                    if let Some(year) = old_file.year {
                        Self::update_year_index(&mut year_index, year, file_id, false)?;
                    }
                    if let Some(album_artist) = &old_file.album_artist {
                        Self::update_category_index(&mut album_artist_index, album_artist, file_id, false)?;
                    }
                }
            }

            files_table.insert(file_id, serialized.as_slice())?;
            path_index.insert(path_str.as_str(), file_id)?;

            // Add secondary indexes
            if let Some(artist) = &file.artist {
                Self::update_category_index(&mut artist_index, artist, file_id, true)?;
            }
            if let Some(album) = &file.album {
                Self::update_category_index(&mut album_index, album, file_id, true)?;
            }
            if let Some(genre) = &file.genre {
                Self::update_category_index(&mut genre_index, genre, file_id, true)?;
            }
            if let Some(year) = file.year {
                Self::update_year_index(&mut year_index, year, file_id, true)?;
            }
            if let Some(album_artist) = &file.album_artist {
                Self::update_category_index(&mut album_artist_index, album_artist, file_id, true)?;
            }

            self.total_size.fetch_add(file.size, Ordering::SeqCst);
            self.total_files.fetch_add(1, Ordering::SeqCst);
        }
        write_txn.commit()?;

        debug!("Updated media file {} with ID {}", path_str, file_id);
        Ok(())
    }

    async fn get_files_in_directory(&self, dir: &Path) -> Result<Vec<MediaFile>> {
        let dir_key = dir.to_string_lossy().to_string();

        let read_txn = self.db.begin_read()?;
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
        
        debug!("get_directory_listing: querying for parent_path='{}' (raw='{}'), filter='{}'", parent_str, parent_path.to_string_lossy(), media_type_filter);
        
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

        let read_txn = self.db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;
        let dir_index = read_txn.open_table(DIR_INDEX)?;

        let mut subdirs = HashSet::new();
        let mut files = Vec::new();

        // Get files in this directory
        let file_ids = dir_index.get(parent_str.as_str())?
            .map(|v| Self::parse_dir_index(v.value()))
            .unwrap_or_default();
        
        debug!("get_directory_listing: found {} file IDs for dir '{}'", file_ids.len(), parent_str);
        
        for file_id in file_ids {
            if let Some(data) = files_table.get(file_id)? {
                let file = Self::deserialize_media_file(data.value())?;
                if media_type_filter.is_empty() || file.mime_type.starts_with(media_type_filter) {
                    files.push(file);
                }
            }
        }

        // Find subdirectories using B-tree range queries
        for result in dir_index.range(prefix.as_str()..)? {
            let (key, value) = result?;
            let key_str = key.value();
            if !key_str.starts_with(&prefix) {
                break;
            }
            if key_str == parent_str {
                continue;
            }
            
            // Skip empty directories
            let value_str = value.value();
            if value_str.trim().is_empty() {
                continue;
            }
            
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

        let mut directories: Vec<MediaDirectory> = subdirs
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

        // Sort subdirectories case-insensitively
        directories.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        
        // Sort files by track number if available, then case-insensitively by filename
        files.sort_by(|a, b| {
            match (a.track_number, b.track_number) {
                (Some(ta), Some(tb)) if ta != tb => ta.cmp(&tb),
                _ => a.filename.to_lowercase().cmp(&b.filename.to_lowercase()),
            }
        });

        Ok((directories, files))
    }

    async fn cleanup_missing_files(&self, existing_paths: &[PathBuf]) -> Result<usize> {
        let existing_set: HashSet<String> = existing_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        // First, collect all paths to remove
        let paths_to_remove: Vec<PathBuf> = {
            let read_txn = self.db.begin_read()?;
            let path_index = read_txn.open_table(PATH_INDEX)?;
            
            path_index.iter()?
                .filter_map(|r| r.ok())
                .filter(|(k, _)| !existing_set.contains(k.value()))
                .map(|(k, _)| PathBuf::from(k.value()))
                .collect()
        };

        // Use batch removal
        self.bulk_remove_media_files(&paths_to_remove).await
    }

    async fn get_file_by_path(&self, path: &Path) -> Result<Option<MediaFile>> {
        let path_str = path.to_string_lossy().to_string();

        let read_txn = self.db.begin_read()?;
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
        let read_txn = self.db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        if let Some(data) = files_table.get(id)? {
            return Ok(Some(Self::deserialize_media_file(data.value())?));
        }

        Ok(None)
    }

    async fn get_stats(&self) -> Result<DatabaseStats> {
        let total_files = self.total_files.load(Ordering::SeqCst) as usize;
        let total_size = self.total_size.load(Ordering::SeqCst);
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
        let read_result = self.db.begin_read();
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
        info!("Created database backup at {}", backup_path.display());
        Ok(())
    }

    async fn restore_from_backup(&self, backup_path: &Path) -> Result<()> {
        tokio::fs::copy(backup_path, &self.db_path).await?;
        info!("Restored database from backup {}", backup_path.display());
        Ok(())
    }

    async fn vacuum(&self) -> Result<()> {
        debug!("Compaction requested, skipping to preserve concurrent MVCC access");
        Ok(())
    }

    async fn get_artists(&self) -> Result<Vec<MusicCategory>> {
        let read_txn = self.db.begin_read()?;
        let artist_index = read_txn.open_table(ARTIST_INDEX)?;
        
        let mut categories = Vec::new();
        for result in artist_index.iter()? {
            let (key, value) = result?;
            let artist_name = key.value().to_string();
            let count = value.value().split(',').filter(|s| !s.is_empty()).count();
            categories.push(MusicCategory {
                id: artist_name.clone(),
                name: artist_name,
                category_type: MusicCategoryType::Artist,
                count,
            });
        }
        Ok(categories)
    }

    async fn get_albums(&self, artist_filter: Option<&str>) -> Result<Vec<MusicCategory>> {
        let read_txn = self.db.begin_read()?;
        let album_index = read_txn.open_table(ALBUM_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;
        
        let mut categories = Vec::new();
        for result in album_index.iter()? {
            let (key, value) = result?;
            let album_name = key.value().to_string();
            let file_ids: Vec<i64> = value.value().split(',').filter_map(|s| s.parse().ok()).collect();
            
            let count = if let Some(artist) = artist_filter {
                let mut matched = 0;
                for fid in file_ids {
                    if let Some(data) = files_table.get(fid)? {
                        if let Ok(file) = Self::deserialize_media_file(data.value()) {
                            if file.artist.as_deref() == Some(artist) {
                                matched += 1;
                            }
                        }
                    }
                }
                matched
            } else {
                file_ids.len()
            };
            
            if count > 0 {
                categories.push(MusicCategory {
                    id: album_name.clone(),
                    name: album_name,
                    category_type: MusicCategoryType::Album,
                    count,
                });
            }
        }
        Ok(categories)
    }

    async fn get_genres(&self) -> Result<Vec<MusicCategory>> {
        let read_txn = self.db.begin_read()?;
        let genre_index = read_txn.open_table(GENRE_INDEX)?;
        
        let mut categories = Vec::new();
        for result in genre_index.iter()? {
            let (key, value) = result?;
            let name = key.value().to_string();
            let count = value.value().split(',').filter(|s| !s.is_empty()).count();
            categories.push(MusicCategory {
                id: name.clone(),
                name,
                category_type: MusicCategoryType::Genre,
                count,
            });
        }
        Ok(categories)
    }

    async fn get_years(&self) -> Result<Vec<MusicCategory>> {
        let read_txn = self.db.begin_read()?;
        let year_index = read_txn.open_table(YEAR_INDEX)?;
        
        let mut categories = Vec::new();
        for result in year_index.iter()? {
            let (key, value) = result?;
            let year = key.value();
            let count = value.value().split(',').filter(|s| !s.is_empty()).count();
            categories.push(MusicCategory {
                id: year.to_string(),
                name: year.to_string(),
                category_type: MusicCategoryType::Year,
                count,
            });
        }
        Ok(categories)
    }

    async fn get_album_artists(&self) -> Result<Vec<MusicCategory>> {
        let read_txn = self.db.begin_read()?;
        let album_artist_index = read_txn.open_table(ALBUM_ARTIST_INDEX)?;
        
        let mut categories = Vec::new();
        for result in album_artist_index.iter()? {
            let (key, value) = result?;
            let name = key.value().to_string();
            let count = value.value().split(',').filter(|s| !s.is_empty()).count();
            categories.push(MusicCategory {
                id: name.clone(),
                name,
                category_type: MusicCategoryType::AlbumArtist,
                count,
            });
        }
        Ok(categories)
    }

    async fn get_music_by_artist(&self, artist: &str) -> Result<Vec<MediaFile>> {
        let read_txn = self.db.begin_read()?;
        let artist_index = read_txn.open_table(ARTIST_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;
        
        let mut files = Vec::new();
        if let Some(val) = artist_index.get(artist)? {
            let file_ids: Vec<i64> = val.value().split(',').filter_map(|s| s.parse().ok()).collect();
            for fid in file_ids {
                if let Some(data) = files_table.get(fid)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
        }
        Ok(files)
    }

    async fn get_music_by_album(&self, album: &str, artist: Option<&str>) -> Result<Vec<MediaFile>> {
        let read_txn = self.db.begin_read()?;
        let album_index = read_txn.open_table(ALBUM_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;
        
        let mut files = Vec::new();
        if let Some(val) = album_index.get(album)? {
            let file_ids: Vec<i64> = val.value().split(',').filter_map(|s| s.parse().ok()).collect();
            for fid in file_ids {
                if let Some(data) = files_table.get(fid)? {
                    let file = Self::deserialize_media_file(data.value())?;
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
        let read_txn = self.db.begin_read()?;
        let genre_index = read_txn.open_table(GENRE_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;
        
        let mut files = Vec::new();
        if let Some(val) = genre_index.get(genre)? {
            let file_ids: Vec<i64> = val.value().split(',').filter_map(|s| s.parse().ok()).collect();
            for fid in file_ids {
                if let Some(data) = files_table.get(fid)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
        }
        Ok(files)
    }

    async fn get_music_by_year(&self, year: u32) -> Result<Vec<MediaFile>> {
        let read_txn = self.db.begin_read()?;
        let year_index = read_txn.open_table(YEAR_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;
        
        let mut files = Vec::new();
        if let Some(val) = year_index.get(year)? {
            let file_ids: Vec<i64> = val.value().split(',').filter_map(|s| s.parse().ok()).collect();
            for fid in file_ids {
                if let Some(data) = files_table.get(fid)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
        }
        Ok(files)
    }

    async fn get_music_by_album_artist(&self, album_artist: &str) -> Result<Vec<MediaFile>> {
        let read_txn = self.db.begin_read()?;
        let album_artist_index = read_txn.open_table(ALBUM_ARTIST_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;
        
        let mut files = Vec::new();
        if let Some(val) = album_artist_index.get(album_artist)? {
            let file_ids: Vec<i64> = val.value().split(',').filter_map(|s| s.parse().ok()).collect();
            for fid in file_ids {
                if let Some(data) = files_table.get(fid)? {
                    files.push(Self::deserialize_media_file(data.value())?);
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

        let write_txn = self.db.begin_write()?;
        {
            let mut playlists_table = write_txn.open_table(PLAYLISTS_TABLE)?;
            playlists_table.insert(playlist_id, serialized.as_slice())?;
        }
        write_txn.commit()?;

        info!("Created playlist '{}' with ID {}", name, playlist_id);
        Ok(playlist_id)
    }

    async fn get_playlists(&self) -> Result<Vec<Playlist>> {
        let read_txn = self.db.begin_read()?;
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
        let read_txn = self.db.begin_read()?;
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

        let write_txn = self.db.begin_write()?;
        {
            let mut playlists_table = write_txn.open_table(PLAYLISTS_TABLE)?;
            playlists_table.insert(playlist_id, serialized.as_slice())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    async fn delete_playlist(&self, playlist_id: i64) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
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

        let write_txn = self.db.begin_write()?;
        {
            let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
            playlist_entries.insert(key.as_str(), media_file_id)?;
        }
        write_txn.commit()?;

        Ok(media_file_id)
    }

    async fn batch_add_to_playlist(&self, playlist_id: i64, media_file_ids: &[(i64, u32)]) -> Result<Vec<i64>> {
        let write_txn = self.db.begin_write()?;
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

        let read_txn = self.db.begin_read()?;
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

        let write_txn = self.db.begin_write()?;
        {
            let mut files_table = write_txn.open_table(FILES_TABLE)?;
            let mut path_index = write_txn.open_table(PATH_INDEX)?;
            let mut dir_index = write_txn.open_table(DIR_INDEX)?;

            let mut artist_index = write_txn.open_table(ARTIST_INDEX)?;
            let mut album_index = write_txn.open_table(ALBUM_INDEX)?;
            let mut genre_index = write_txn.open_table(GENRE_INDEX)?;
            let mut year_index = write_txn.open_table(YEAR_INDEX)?;
            let mut album_artist_index = write_txn.open_table(ALBUM_ARTIST_INDEX)?;

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

                // Update secondary indexes
                if let Some(ref artist) = file.artist {
                    let current_ids = artist_index.get(artist.as_str())?.map(|v| v.value().to_string());
                    let new_ids = Self::add_to_dir_index(current_ids.as_deref(), file_id);
                    artist_index.insert(artist.as_str(), new_ids.as_str())?;
                }
                if let Some(ref album) = file.album {
                    let current_ids = album_index.get(album.as_str())?.map(|v| v.value().to_string());
                    let new_ids = Self::add_to_dir_index(current_ids.as_deref(), file_id);
                    album_index.insert(album.as_str(), new_ids.as_str())?;
                }
                if let Some(ref genre) = file.genre {
                    let current_ids = genre_index.get(genre.as_str())?.map(|v| v.value().to_string());
                    let new_ids = Self::add_to_dir_index(current_ids.as_deref(), file_id);
                    genre_index.insert(genre.as_str(), new_ids.as_str())?;
                }
                if let Some(year) = file.year {
                    let current_ids = year_index.get(year)?.map(|v| v.value().to_string());
                    let new_ids = Self::add_to_dir_index(current_ids.as_deref(), file_id);
                    year_index.insert(year, new_ids.as_str())?;
                }
                if let Some(ref album_artist) = file.album_artist {
                    let current_ids = album_artist_index.get(album_artist.as_str())?.map(|v| v.value().to_string());
                    let new_ids = Self::add_to_dir_index(current_ids.as_deref(), file_id);
                    album_artist_index.insert(album_artist.as_str(), new_ids.as_str())?;
                }

                // Update atomic counters
                self.total_files.fetch_add(1, Ordering::SeqCst);
                self.total_size.fetch_add(file.size, Ordering::SeqCst);
            }
        }
        write_txn.commit()?;

        debug!("Bulk stored {} media files", ids.len());
        Ok(ids)
    }

    async fn bulk_update_media_files(&self, files: &[MediaFile]) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut files_table = write_txn.open_table(FILES_TABLE)?;
            let mut path_index = write_txn.open_table(PATH_INDEX)?;
            
            let mut artist_index = write_txn.open_table(ARTIST_INDEX)?;
            let mut album_index = write_txn.open_table(ALBUM_INDEX)?;
            let mut genre_index = write_txn.open_table(GENRE_INDEX)?;
            let mut year_index = write_txn.open_table(YEAR_INDEX)?;
            let mut album_artist_index = write_txn.open_table(ALBUM_ARTIST_INDEX)?;

            for file in files {
                if let Some(file_id) = file.id {
                    // Fetch existing size to update total_size counter accurately
                    let old_size = if let Some(old_data) = files_table.get(file_id)? {
                        if let Ok(old_file) = Self::deserialize_media_file(old_data.value()) {
                            // Also remove from old indexes if values changed
                            if old_file.artist != file.artist {
                                if let Some(ref old_artist) = old_file.artist {
                                    let current_ids = artist_index.get(old_artist.as_str())?.map(|v| v.value().to_string());
                                    let new_ids = Self::remove_from_dir_index(current_ids.as_deref(), file_id);
                                    if new_ids.is_empty() { artist_index.remove(old_artist.as_str())?; }
                                    else { artist_index.insert(old_artist.as_str(), new_ids.as_str())?; }
                                }
                            }
                            if old_file.album != file.album {
                                if let Some(ref old_album) = old_file.album {
                                    let current_ids = album_index.get(old_album.as_str())?.map(|v| v.value().to_string());
                                    let new_ids = Self::remove_from_dir_index(current_ids.as_deref(), file_id);
                                    if new_ids.is_empty() { album_index.remove(old_album.as_str())?; }
                                    else { album_index.insert(old_album.as_str(), new_ids.as_str())?; }
                                }
                            }
                            if old_file.genre != file.genre {
                                if let Some(ref old_genre) = old_file.genre {
                                    let current_ids = genre_index.get(old_genre.as_str())?.map(|v| v.value().to_string());
                                    let new_ids = Self::remove_from_dir_index(current_ids.as_deref(), file_id);
                                    if new_ids.is_empty() { genre_index.remove(old_genre.as_str())?; }
                                    else { genre_index.insert(old_genre.as_str(), new_ids.as_str())?; }
                                }
                            }
                            if old_file.year != file.year {
                                if let Some(old_year) = old_file.year {
                                    let current_ids = year_index.get(old_year)?.map(|v| v.value().to_string());
                                    let new_ids = Self::remove_from_dir_index(current_ids.as_deref(), file_id);
                                    if new_ids.is_empty() { year_index.remove(old_year)?; }
                                    else { year_index.insert(old_year, new_ids.as_str())?; }
                                }
                            }
                            if old_file.album_artist != file.album_artist {
                                if let Some(ref old_album_artist) = old_file.album_artist {
                                    let current_ids = album_artist_index.get(old_album_artist.as_str())?.map(|v| v.value().to_string());
                                    let new_ids = Self::remove_from_dir_index(current_ids.as_deref(), file_id);
                                    if new_ids.is_empty() { album_artist_index.remove(old_album_artist.as_str())?; }
                                    else { album_artist_index.insert(old_album_artist.as_str(), new_ids.as_str())?; }
                                }
                            }
                            old_file.size
                        } else { 0 }
                    } else { 0 };

                    let serialized = Self::serialize_media_file(file)?;
                    let path_str = file.path.to_string_lossy().to_string();

                    files_table.insert(file_id, serialized.as_slice())?;
                    path_index.insert(path_str.as_str(), file_id)?;

                    // Add to new indexes
                    if let Some(ref artist) = file.artist {
                        let current_ids = artist_index.get(artist.as_str())?.map(|v| v.value().to_string());
                        let new_ids = Self::add_to_dir_index(current_ids.as_deref(), file_id);
                        artist_index.insert(artist.as_str(), new_ids.as_str())?;
                    }
                    if let Some(ref album) = file.album {
                        let current_ids = album_index.get(album.as_str())?.map(|v| v.value().to_string());
                        let new_ids = Self::add_to_dir_index(current_ids.as_deref(), file_id);
                        album_index.insert(album.as_str(), new_ids.as_str())?;
                    }
                    if let Some(ref genre) = file.genre {
                        let current_ids = genre_index.get(genre.as_str())?.map(|v| v.value().to_string());
                        let new_ids = Self::add_to_dir_index(current_ids.as_deref(), file_id);
                        genre_index.insert(genre.as_str(), new_ids.as_str())?;
                    }
                    if let Some(year) = file.year {
                        let current_ids = year_index.get(year)?.map(|v| v.value().to_string());
                        let new_ids = Self::add_to_dir_index(current_ids.as_deref(), file_id);
                        year_index.insert(year, new_ids.as_str())?;
                    }
                    if let Some(ref album_artist) = file.album_artist {
                        let current_ids = album_artist_index.get(album_artist.as_str())?.map(|v| v.value().to_string());
                        let new_ids = Self::add_to_dir_index(current_ids.as_deref(), file_id);
                        album_artist_index.insert(album_artist.as_str(), new_ids.as_str())?;
                    }

                    // Update total size counter
                    if file.size >= old_size {
                        self.total_size.fetch_add(file.size - old_size, Ordering::SeqCst);
                    } else {
                        self.total_size.fetch_sub(old_size - file.size, Ordering::SeqCst);
                    }
                }
            }
        }
        write_txn.commit()?;

        debug!("Bulk updated {} media files", files.len());
        Ok(())
    }

    async fn bulk_remove_media_files(&self, paths: &[PathBuf]) -> Result<usize> {
        let mut count = 0;

        let write_txn = self.db.begin_write()?;
        
        // First pass: collect file IDs and info for all paths
        let file_ids_and_info: Vec<(i64, String, String, Option<String>, Option<String>, Option<String>, Option<u32>, Option<String>, u64)> = {
            let path_index = write_txn.open_table(PATH_INDEX)?;
            let files_table = write_txn.open_table(FILES_TABLE)?;
            let mut result = Vec::new();
            for path in paths {
                let path_str = path.to_string_lossy().to_string();
                let dir_key = Self::get_dir_key(path);
                let guard = path_index.get(path_str.as_str())?;
                if let Some(file_id) = guard.map(|v| v.value()) {
                    let mut artist = None;
                    let mut album = None;
                    let mut genre = None;
                    let mut year = None;
                    let mut album_artist = None;
                    let mut size = 0;
                    if let Some(data) = files_table.get(file_id)? {
                        if let Ok(file) = Self::deserialize_media_file(data.value()) {
                            artist = file.artist;
                            album = file.album;
                            genre = file.genre;
                            year = file.year;
                            album_artist = file.album_artist;
                            size = file.size;
                        }
                    }
                    result.push((file_id, path_str, dir_key, artist, album, genre, year, album_artist, size));
                }
            }
            result
        };

        // Second pass: remove all entries
        {
            let mut files_table = write_txn.open_table(FILES_TABLE)?;
            let mut path_index = write_txn.open_table(PATH_INDEX)?;
            let mut dir_index = write_txn.open_table(DIR_INDEX)?;

            let mut artist_index = write_txn.open_table(ARTIST_INDEX)?;
            let mut album_index = write_txn.open_table(ALBUM_INDEX)?;
            let mut genre_index = write_txn.open_table(GENRE_INDEX)?;
            let mut year_index = write_txn.open_table(YEAR_INDEX)?;
            let mut album_artist_index = write_txn.open_table(ALBUM_ARTIST_INDEX)?;

            for &(file_id, ref path_str, ref dir_key, ref artist, ref album, ref genre, year, ref album_artist, size) in &file_ids_and_info {
                files_table.remove(file_id)?;
                path_index.remove(path_str.as_str())?;

                let current_dir_ids = dir_index.get(dir_key.as_str())?.map(|v| v.value().to_string());
                let new_dir_ids = Self::remove_from_dir_index(current_dir_ids.as_deref(), file_id);
                if new_dir_ids.is_empty() {
                    dir_index.remove(dir_key.as_str())?;
                } else {
                    dir_index.insert(dir_key.as_str(), new_dir_ids.as_str())?;
                }

                // Remove from secondary indexes
                if let Some(ref art) = artist {
                    let current_ids = artist_index.get(art.as_str())?.map(|v| v.value().to_string());
                    let new_ids = Self::remove_from_dir_index(current_ids.as_deref(), file_id);
                    if new_ids.is_empty() { artist_index.remove(art.as_str())?; }
                    else { artist_index.insert(art.as_str(), new_ids.as_str())?; }
                }
                if let Some(ref alb) = album {
                    let current_ids = album_index.get(alb.as_str())?.map(|v| v.value().to_string());
                    let new_ids = Self::remove_from_dir_index(current_ids.as_deref(), file_id);
                    if new_ids.is_empty() { album_index.remove(alb.as_str())?; }
                    else { album_index.insert(alb.as_str(), new_ids.as_str())?; }
                }
                if let Some(ref gen) = genre {
                    let current_ids = genre_index.get(gen.as_str())?.map(|v| v.value().to_string());
                    let new_ids = Self::remove_from_dir_index(current_ids.as_deref(), file_id);
                    if new_ids.is_empty() { genre_index.remove(gen.as_str())?; }
                    else { genre_index.insert(gen.as_str(), new_ids.as_str())?; }
                }
                if let Some(yr) = year {
                    let current_ids = year_index.get(yr)?.map(|v| v.value().to_string());
                    let new_ids = Self::remove_from_dir_index(current_ids.as_deref(), file_id);
                    if new_ids.is_empty() { year_index.remove(yr)?; }
                    else { year_index.insert(yr, new_ids.as_str())?; }
                }
                if let Some(ref alb_art) = album_artist {
                    let current_ids = album_artist_index.get(alb_art.as_str())?.map(|v| v.value().to_string());
                    let new_ids = Self::remove_from_dir_index(current_ids.as_deref(), file_id);
                    if new_ids.is_empty() { album_artist_index.remove(alb_art.as_str())?; }
                    else { album_artist_index.insert(alb_art.as_str(), new_ids.as_str())?; }
                }

                // Update atomic counters
                self.total_files.fetch_sub(1, Ordering::SeqCst);
                self.total_size.fetch_sub(size, Ordering::SeqCst);

                count += 1;
            }
        }
        write_txn.commit()?;

        debug!("Bulk removed {} media files", count);
        Ok(count)
    }

    async fn remove_from_playlist(&self, playlist_id: i64, media_file_id: i64) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
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
        let read_txn = self.db.begin_read()?;
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
        let write_txn = self.db.begin_write()?;
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

        let read_txn = self.db.begin_read()?;
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

        let read_txn = self.db.begin_read()?;
        let dir_index = read_txn.open_table(DIR_INDEX)?;

        let mut subdirs = HashSet::new();

        for result in dir_index.range(prefix.as_str()..)? {
            let (key, _) = result?;
            let key_str = key.value();
            if !key_str.starts_with(&prefix) {
                break;
            }
            if key_str == canonical_parent_path {
                continue;
            }
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
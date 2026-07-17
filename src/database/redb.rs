//! RedbDatabase - ACID-compliant embedded database using redb
//!
//! This module provides a robust, memory-efficient database implementation
//! using the redb crate. Unlike RAM-based indexes, redb uses B-trees on disk,
//! allowing it to handle databases larger than available RAM.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

use crate::platform::DatabaseError;

use super::{
    DatabaseHealth, DatabaseManager, DatabaseStats, MediaDirectory, MediaFile, MusicCategory,
    MusicCategoryType, Playlist, RemovalSummary,
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
const PLAYLIST_SOURCES: TableDefinition<i64, &str> = TableDefinition::new("playlist_sources");
const METADATA_TABLE: TableDefinition<&str, u64> = TableDefinition::new("metadata");

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
    mutation_lock: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for RedbDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedbDatabase")
            .field("db_path", &self.db_path)
            .field("next_file_id", &self.next_file_id.load(Ordering::Relaxed))
            .field(
                "next_playlist_id",
                &self.next_playlist_id.load(Ordering::Relaxed),
            )
            .finish()
    }
}

impl RedbDatabase {
    fn canonical_path(path: &Path) -> Result<PathBuf> {
        let raw = path.to_string_lossy();
        if raw.starts_with("http://") || raw.starts_with("https://") {
            return Ok(path.to_path_buf());
        }
        let normalizer = crate::platform::filesystem::create_platform_path_normalizer();
        Ok(PathBuf::from(normalizer.to_canonical(path)?))
    }

    fn canonical_file(file: &MediaFile) -> Result<MediaFile> {
        let mut file = file.clone();
        file.path = Self::canonical_path(&file.path)?;
        Ok(file)
    }

    fn mime_family(mime: &str) -> String {
        mime.split_once('/')
            .map(|(v, _)| format!("{v}/"))
            .unwrap_or_else(|| mime.to_string())
    }
    /// Create a new RedbDatabase at the specified path
    pub async fn new(path: PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Opening or schema initialization failures are returned to the caller.
        // The application preserves the unusable file before creating a replacement.
        let db = Database::create(&path)
            .with_context(|| format!("Failed to open redb database at {}", path.display()))?;

        // Initialize tables if they don't exist
        {
            let write_txn = db.begin_write()?;
            {
                let _ = write_txn.open_table(FILES_TABLE)?;
                let _ = write_txn.open_table(PATH_INDEX)?;
                let _ = write_txn.open_table(DIR_INDEX)?;
                let _ = write_txn.open_table(PLAYLISTS_TABLE)?;
                let _ = write_txn.open_table(PLAYLIST_ENTRIES)?;
                let _ = write_txn.open_table(PLAYLIST_SOURCES)?;
                let _ = write_txn.open_table(METADATA_TABLE)?;
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
            mutation_lock: tokio::sync::Mutex::new(()),
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

    fn remove_file_indexes(
        dir: &mut redb::Table<&str, &str>,
        artist: &mut redb::Table<&str, &str>,
        album: &mut redb::Table<&str, &str>,
        genre: &mut redb::Table<&str, &str>,
        year: &mut redb::Table<u32, &str>,
        album_artist: &mut redb::Table<&str, &str>,
        id: i64,
        file: &MediaFile,
    ) -> Result<()> {
        let key = Self::get_dir_key(&file.path);
        let old = dir.get(key.as_str())?.map(|v| v.value().to_string());
        let new = Self::remove_from_dir_index(old.as_deref(), id);
        if new.is_empty() {
            dir.remove(key.as_str())?;
        } else {
            dir.insert(key.as_str(), new.as_str())?;
        }
        if let Some(v) = &file.artist {
            Self::update_category_index(artist, v, id, false)?;
        }
        if let Some(v) = &file.album {
            Self::update_category_index(album, v, id, false)?;
        }
        if let Some(v) = &file.genre {
            Self::update_category_index(genre, v, id, false)?;
        }
        if let Some(v) = file.year {
            Self::update_year_index(year, v, id, false)?;
        }
        if let Some(v) = &file.album_artist {
            Self::update_category_index(album_artist, v, id, false)?;
        }
        Ok(())
    }

    fn add_file_indexes(
        dir: &mut redb::Table<&str, &str>,
        artist: &mut redb::Table<&str, &str>,
        album: &mut redb::Table<&str, &str>,
        genre: &mut redb::Table<&str, &str>,
        year: &mut redb::Table<u32, &str>,
        album_artist: &mut redb::Table<&str, &str>,
        id: i64,
        file: &MediaFile,
    ) -> Result<()> {
        let key = Self::get_dir_key(&file.path);
        let old = dir.get(key.as_str())?.map(|v| v.value().to_string());
        let new = Self::add_to_dir_index(old.as_deref(), id);
        dir.insert(key.as_str(), new.as_str())?;
        if let Some(v) = &file.artist {
            Self::update_category_index(artist, v, id, true)?;
        }
        if let Some(v) = &file.album {
            Self::update_category_index(album, v, id, true)?;
        }
        if let Some(v) = &file.genre {
            Self::update_category_index(genre, v, id, true)?;
        }
        if let Some(v) = file.year {
            Self::update_year_index(year, v, id, true)?;
        }
        if let Some(v) = &file.album_artist {
            Self::update_category_index(album_artist, v, id, true)?;
        }
        Ok(())
    }

    fn remove_files_from_transaction(
        transaction: &redb::WriteTransaction,
        files: &[(String, i64, MediaFile)],
    ) -> Result<(usize, u64)> {
        let mut files_table = transaction.open_table(FILES_TABLE)?;
        let mut path_index = transaction.open_table(PATH_INDEX)?;
        let mut dir_index = transaction.open_table(DIR_INDEX)?;
        let mut artist_index = transaction.open_table(ARTIST_INDEX)?;
        let mut album_index = transaction.open_table(ALBUM_INDEX)?;
        let mut genre_index = transaction.open_table(GENRE_INDEX)?;
        let mut year_index = transaction.open_table(YEAR_INDEX)?;
        let mut album_artist_index = transaction.open_table(ALBUM_ARTIST_INDEX)?;
        let mut playlist_entries = transaction.open_table(PLAYLIST_ENTRIES)?;

        let mut removed_size = 0_u64;
        for (path, id, file) in files {
            files_table.remove(*id)?;
            path_index.remove(path.as_str())?;
            Self::remove_file_indexes(
                &mut dir_index,
                &mut artist_index,
                &mut album_index,
                &mut genre_index,
                &mut year_index,
                &mut album_artist_index,
                *id,
                file,
            )?;

            let dangling = playlist_entries
                .iter()?
                .filter_map(|entry| entry.ok())
                .filter(|(_, value)| value.value() == *id)
                .map(|(key, _)| key.value().to_string())
                .collect::<Vec<_>>();
            for key in dangling {
                playlist_entries.remove(key.as_str())?;
            }
            removed_size = removed_size.saturating_add(file.size);
        }

        Ok((files.len(), removed_size))
    }

    /// Add a file ID to a directory index
    fn add_to_dir_index(current: Option<&str>, file_id: i64) -> String {
        match current {
            Some(ids) if !ids.is_empty() => {
                let mut id_set: HashSet<i64> =
                    ids.split(',').filter_map(|s| s.parse().ok()).collect();
                id_set.insert(file_id);
                let mut v: Vec<_> = id_set.into_iter().collect();
                v.sort();
                v.iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            }
            _ => file_id.to_string(),
        }
    }

    /// Remove a file ID from a directory index
    fn remove_from_dir_index(current: Option<&str>, file_id: i64) -> String {
        match current {
            Some(ids) if !ids.is_empty() => {
                let mut id_set: HashSet<i64> =
                    ids.split(',').filter_map(|s| s.parse().ok()).collect();
                id_set.remove(&file_id);
                let mut v: Vec<_> = id_set.into_iter().collect();
                v.sort();
                v.iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            }
            _ => String::new(),
        }
    }

    /// Parse directory index to file IDs
    fn parse_dir_index(ids_str: &str) -> Vec<i64> {
        ids_str.split(',').filter_map(|s| s.parse().ok()).collect()
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
                let mut id_set: HashSet<i64> =
                    ids.split(',').filter_map(|s| s.parse().ok()).collect();
                id_set.insert(file_id);
                let mut v: Vec<_> = id_set.into_iter().collect();
                v.sort();
                v.iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            }
            _ => file_id.to_string(),
        }
    }

    fn remove_from_id_list(current: Option<&str>, file_id: i64) -> String {
        match current {
            Some(ids) if !ids.is_empty() => {
                let mut id_set: HashSet<i64> =
                    ids.split(',').filter_map(|s| s.parse().ok()).collect();
                id_set.remove(&file_id);
                let mut v: Vec<_> = id_set.into_iter().collect();
                v.sort();
                v.iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
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
            modified_secs: file
                .modified
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            mime_type: file.mime_type.clone(),
            duration_secs: file.duration.map(|d| d.as_secs_f64()),
            title: file.title.clone(),
            artist: file.artist.clone(),
            album: file.album.clone(),
            genre: file.genre.clone(),
            track_number: file.track_number,
            year: file.year,
            album_artist: file.album_artist.clone(),
            created_at_secs: file
                .created_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            updated_at_secs: file
                .updated_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
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
            created_at_secs: playlist
                .created_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            updated_at_secs: playlist
                .updated_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
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
        self.bulk_store_media_files(std::slice::from_ref(file))
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("media upsert returned no ID"))
    }

    fn stream_all_media_files(
        &self,
    ) -> Pin<Box<dyn futures_util::Stream<Item = Result<MediaFile, DatabaseError>> + Send + '_>>
    {
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
        Ok(self.bulk_remove_media_files(&[path.to_path_buf()]).await? > 0)
    }

    async fn update_media_file(&self, file: &MediaFile) -> Result<()> {
        if file.id.is_none() {
            return Err(anyhow!("Cannot update file without ID"));
        }
        self.bulk_store_media_files(std::slice::from_ref(file))
            .await?;
        Ok(())
    }

    async fn get_files_in_directory(&self, dir: &Path) -> Result<Vec<MediaFile>> {
        let dir_key = Self::canonical_path(dir)?.to_string_lossy().to_string();

        let read_txn = self.db.begin_read()?;
        let files_table = read_txn.open_table(FILES_TABLE)?;
        let dir_index = read_txn.open_table(DIR_INDEX)?;

        let file_ids = dir_index
            .get(dir_key.as_str())?
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
        let canonical_parent = Self::canonical_path(parent_path)?;
        let raw_parent_str = canonical_parent.to_string_lossy().to_string();

        // Strip trailing slash if present, unless it's the root path "/"
        let parent_str = if raw_parent_str.len() > 1
            && (raw_parent_str.ends_with('/') || raw_parent_str.ends_with('\\'))
        {
            raw_parent_str[..raw_parent_str.len() - 1].to_string()
        } else {
            raw_parent_str
        };

        debug!(
            "get_directory_listing: querying for parent_path='{}' (raw='{}'), filter='{}'",
            parent_str,
            parent_path.to_string_lossy(),
            media_type_filter
        );

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
        let file_ids = dir_index
            .get(parent_str.as_str())?
            .map(|v| Self::parse_dir_index(v.value()))
            .unwrap_or_default();

        debug!(
            "get_directory_listing: found {} file IDs for dir '{}'",
            file_ids.len(),
            parent_str
        );

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
            let has_matching = Self::parse_dir_index(value_str).into_iter().any(|id| {
                files_table
                    .get(id)
                    .ok()
                    .flatten()
                    .and_then(|data| Self::deserialize_media_file(data.value()).ok())
                    .map(|file| {
                        media_type_filter.is_empty()
                            || file.mime_type.starts_with(media_type_filter)
                    })
                    .unwrap_or(false)
            });
            if !has_matching {
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
        files.sort_by(|a, b| match (a.track_number, b.track_number) {
            (Some(ta), Some(tb)) if ta != tb => ta.cmp(&tb),
            _ => a.filename.to_lowercase().cmp(&b.filename.to_lowercase()),
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

            path_index
                .iter()?
                .filter_map(|r| r.ok())
                .filter(|(k, _)| !existing_set.contains(k.value()))
                .map(|(k, _)| PathBuf::from(k.value()))
                .collect()
        };

        // Use batch removal
        self.bulk_remove_media_files(&paths_to_remove).await
    }

    async fn get_file_by_path(&self, path: &Path) -> Result<Option<MediaFile>> {
        let path_str = Self::canonical_path(path)?.to_string_lossy().to_string();

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
        let database_size = tokio::fs::metadata(&self.db_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        let mut video_files = 0;
        let mut audio_files = 0;
        let mut image_files = 0;
        let mut playlists = 0;

        if let Ok(read_txn) = self.db.begin_read() {
            if let Ok(files_table) = read_txn.open_table(FILES_TABLE) {
                if let Ok(iter) = files_table.iter() {
                    for result in iter {
                        if let Ok((_, value)) = result {
                            if let Ok(file) = Self::deserialize_media_file(value.value()) {
                                if file.mime_type.starts_with("video/") {
                                    video_files += 1;
                                } else if file.mime_type.starts_with("audio/") {
                                    audio_files += 1;
                                } else if file.mime_type.starts_with("image/") {
                                    image_files += 1;
                                }
                            }
                        }
                    }
                }
            }
            if let Ok(playlists_table) = read_txn.open_table(PLAYLISTS_TABLE) {
                if let Ok(iter) = playlists_table.iter() {
                    playlists = iter.count();
                }
            }
        }

        Ok(DatabaseStats {
            total_files,
            total_size,
            database_size,
            video_files,
            audio_files,
            image_files,
            playlists,
        })
    }

    async fn check_and_repair(&self) -> Result<DatabaseHealth> {
        self.rebuild_derived_indexes().await
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
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut categories = Vec::new();
        for result in artist_index.iter()? {
            let (key, value) = result?;
            let artist_name = key.value().to_string();
            let count = Self::parse_dir_index(value.value())
                .into_iter()
                .filter(|id| files_table.get(*id).ok().flatten().is_some())
                .count();
            if count == 0 {
                continue;
            }
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
            let file_ids: Vec<i64> = value
                .value()
                .split(',')
                .filter_map(|s| s.parse().ok())
                .collect();

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
                file_ids
                    .into_iter()
                    .filter(|id| files_table.get(*id).ok().flatten().is_some())
                    .count()
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
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut categories = Vec::new();
        for result in genre_index.iter()? {
            let (key, value) = result?;
            let name = key.value().to_string();
            let count = Self::parse_dir_index(value.value())
                .into_iter()
                .filter(|id| files_table.get(*id).ok().flatten().is_some())
                .count();
            if count == 0 {
                continue;
            }
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
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut categories = Vec::new();
        for result in year_index.iter()? {
            let (key, value) = result?;
            let year = key.value();
            let count = Self::parse_dir_index(value.value())
                .into_iter()
                .filter(|id| files_table.get(*id).ok().flatten().is_some())
                .count();
            if count == 0 {
                continue;
            }
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
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut categories = Vec::new();
        for result in album_artist_index.iter()? {
            let (key, value) = result?;
            let name = key.value().to_string();
            let count = Self::parse_dir_index(value.value())
                .into_iter()
                .filter(|id| files_table.get(*id).ok().flatten().is_some())
                .count();
            if count == 0 {
                continue;
            }
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
            let file_ids: Vec<i64> = val
                .value()
                .split(',')
                .filter_map(|s| s.parse().ok())
                .collect();
            for fid in file_ids {
                if let Some(data) = files_table.get(fid)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
        }
        Ok(files)
    }

    async fn get_music_by_album(
        &self,
        album: &str,
        artist: Option<&str>,
    ) -> Result<Vec<MediaFile>> {
        let read_txn = self.db.begin_read()?;
        let album_index = read_txn.open_table(ALBUM_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut files = Vec::new();
        if let Some(val) = album_index.get(album)? {
            let file_ids: Vec<i64> = val
                .value()
                .split(',')
                .filter_map(|s| s.parse().ok())
                .collect();
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
            let file_ids: Vec<i64> = val
                .value()
                .split(',')
                .filter_map(|s| s.parse().ok())
                .collect();
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
            let file_ids: Vec<i64> = val
                .value()
                .split(',')
                .filter_map(|s| s.parse().ok())
                .collect();
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
            let file_ids: Vec<i64> = val
                .value()
                .split(',')
                .filter_map(|s| s.parse().ok())
                .collect();
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
            let mut playlist_sources = write_txn.open_table(PLAYLIST_SOURCES)?;

            let existed = playlists_table.remove(playlist_id)?.is_some();

            // Remove all entries for this playlist
            let prefix = format!("{}:", playlist_id);
            let keys_to_remove: Vec<String> = playlist_entries
                .iter()?
                .filter_map(|r| r.ok())
                .filter(|(k, _)| k.value().starts_with(&prefix))
                .map(|(k, _)| k.value().to_string())
                .collect();

            for key in keys_to_remove {
                playlist_entries.remove(key.as_str())?;
            }
            playlist_sources.remove(playlist_id)?;

            existed
        };
        write_txn.commit()?;

        Ok(removed)
    }

    async fn set_playlist_source(&self, playlist_id: i64, source_path: &Path) -> Result<()> {
        let source = Self::canonical_path(source_path)?
            .to_string_lossy()
            .to_string();
        let txn = self.db.begin_write()?;
        {
            txn.open_table(PLAYLIST_SOURCES)?
                .insert(playlist_id, source.as_str())?;
        }
        txn.commit()?;
        Ok(())
    }

    async fn remove_derived_content_by_source(&self, source_path: &Path) -> Result<usize> {
        let source = Self::canonical_path(source_path)?
            .to_string_lossy()
            .to_string();
        let child_prefix = format!("{}/", source.trim_end_matches('/'));
        let matches_source =
            |candidate: &str| candidate == source || candidate.starts_with(&child_prefix);
        let ids = {
            let txn = self.db.begin_read()?;
            let table = txn.open_table(PLAYLIST_SOURCES)?;
            table
                .iter()?
                .filter_map(|e| e.ok())
                .filter(|(_, value)| matches_source(value.value()))
                .map(|(k, _)| k.value())
                .collect::<Vec<_>>()
        };
        let mut removed = 0;
        for id in ids {
            removed += usize::from(self.delete_playlist(id).await?);
        }
        let mut radio_paths = Vec::new();
        use futures_util::StreamExt;
        let mut stream = self.stream_all_media_files();
        while let Some(file) = stream.next().await {
            let file = file?;
            if file.mime_type == "audio/radio" && file.album.as_deref().is_some_and(matches_source)
            {
                radio_paths.push(file.path);
            }
        }
        drop(stream);
        removed += self.bulk_remove_media_files(&radio_paths).await?;
        Ok(removed)
    }

    async fn add_to_playlist(
        &self,
        playlist_id: i64,
        media_file_id: i64,
        position: Option<u32>,
    ) -> Result<i64> {
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

    async fn batch_add_to_playlist(
        &self,
        playlist_id: i64,
        media_file_ids: &[(i64, u32)],
    ) -> Result<Vec<i64>> {
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
            let path_str = Self::canonical_path(path)?.to_string_lossy().to_string();
            if let Some(file_id) = path_index.get(path_str.as_str())?.map(|v| v.value()) {
                if let Some(data) = files_table.get(file_id)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
        }

        Ok(files)
    }

    async fn bulk_store_media_files(&self, files: &[MediaFile]) -> Result<Vec<i64>> {
        let _mutation_guard = self.mutation_lock.lock().await;
        let mut ids = Vec::with_capacity(files.len());
        let mut added_files = 0_u64;
        let mut replaced_size = 0_u64;
        let mut stored_size = 0_u64;

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

            for input in files {
                let file = Self::canonical_file(input)?;
                let path_str = file.path.to_string_lossy().to_string();
                let existing_path_id = path_index.get(path_str.as_str())?.map(|v| v.value());
                let file_id = existing_path_id
                    .or(file.id)
                    .unwrap_or_else(|| self.next_file_id.fetch_add(1, Ordering::SeqCst));
                ids.push(file_id);

                let mut file_with_id = file.clone();
                file_with_id.id = Some(file_id);
                let old_file = files_table
                    .get(file_id)?
                    .map(|data| Self::deserialize_media_file(data.value()))
                    .transpose()?;
                if let Some(old) = &old_file {
                    Self::remove_file_indexes(
                        &mut dir_index,
                        &mut artist_index,
                        &mut album_index,
                        &mut genre_index,
                        &mut year_index,
                        &mut album_artist_index,
                        file_id,
                        old,
                    )?;
                    let old_path = old.path.to_string_lossy().to_string();
                    if old_path != path_str {
                        path_index.remove(old_path.as_str())?;
                    }
                    replaced_size = replaced_size.saturating_add(old.size);
                } else {
                    added_files = added_files.saturating_add(1);
                }

                let serialized = Self::serialize_media_file(&file_with_id)?;
                files_table.insert(file_id, serialized.as_slice())?;
                path_index.insert(path_str.as_str(), file_id)?;
                Self::add_file_indexes(
                    &mut dir_index,
                    &mut artist_index,
                    &mut album_index,
                    &mut genre_index,
                    &mut year_index,
                    &mut album_artist_index,
                    file_id,
                    &file_with_id,
                )?;
                stored_size = stored_size.saturating_add(file.size);
            }
        }
        write_txn.commit()?;
        self.total_files.fetch_add(added_files, Ordering::SeqCst);
        if stored_size >= replaced_size {
            self.total_size
                .fetch_add(stored_size - replaced_size, Ordering::SeqCst);
        } else {
            let decrease = replaced_size - stored_size;
            let _ = self
                .total_size
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                    Some(current.saturating_sub(decrease))
                });
        }

        debug!("Bulk stored {} media files", ids.len());
        Ok(ids)
    }

    async fn bulk_update_media_files(&self, files: &[MediaFile]) -> Result<()> {
        if files.iter().any(|file| file.id.is_none()) {
            return Err(anyhow!("cannot update a media file without an ID"));
        }
        self.bulk_store_media_files(files).await?;
        Ok(())
    }

    async fn bulk_remove_media_files(&self, paths: &[PathBuf]) -> Result<usize> {
        let _mutation_guard = self.mutation_lock.lock().await;
        let transaction = self.db.begin_write()?;
        let mut files = Vec::new();
        let mut orphan_paths = Vec::new();
        let mut seen_ids = HashSet::new();

        {
            let path_index = transaction.open_table(PATH_INDEX)?;
            let files_table = transaction.open_table(FILES_TABLE)?;
            for input_path in paths {
                let path = Self::canonical_path(input_path)?;
                let path_string = path.to_string_lossy().to_string();
                let Some(id) = path_index
                    .get(path_string.as_str())?
                    .map(|value| value.value())
                else {
                    continue;
                };
                if !seen_ids.insert(id) {
                    continue;
                }
                if let Some(data) = files_table.get(id)? {
                    let file = Self::canonical_file(&Self::deserialize_media_file(data.value())?)?;
                    files.push((path_string, id, file));
                } else {
                    orphan_paths.push(path_string);
                }
            }
        }

        if !orphan_paths.is_empty() {
            let mut path_index = transaction.open_table(PATH_INDEX)?;
            for path in orphan_paths {
                path_index.remove(path.as_str())?;
            }
        }

        let (removed, removed_size) = Self::remove_files_from_transaction(&transaction, &files)?;
        transaction.commit()?;
        let _ = self
            .total_files
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                Some(current.saturating_sub(removed as u64))
            });
        let _ = self
            .total_size
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                Some(current.saturating_sub(removed_size))
            });

        debug!("Bulk removed {} media files", removed);
        Ok(removed)
    }

    async fn remove_media_under_path(&self, path: &Path) -> Result<RemovalSummary> {
        let _mutation_guard = self.mutation_lock.lock().await;
        let canonical = Self::canonical_path(path)?;
        let prefix = canonical
            .to_string_lossy()
            .trim_end_matches('/')
            .to_string();
        let child_prefix = format!("{prefix}/");
        let transaction = self.db.begin_write()?;
        let mut files = Vec::new();
        let mut orphan_paths = Vec::new();
        let mut seen_ids = HashSet::new();
        let mut summary = RemovalSummary::default();

        {
            let path_index = transaction.open_table(PATH_INDEX)?;
            let files_table = transaction.open_table(FILES_TABLE)?;
            for entry in path_index.range(prefix.as_str()..)? {
                let (path_key, id) = entry?;
                let path_string = path_key.value();
                if path_string != prefix && !path_string.starts_with(&child_prefix) {
                    break;
                }

                let id = id.value();
                if !seen_ids.insert(id) {
                    continue;
                }
                if let Some(data) = files_table.get(id)? {
                    let file = Self::canonical_file(&Self::deserialize_media_file(data.value())?)?;
                    if let Some(parent) = file.path.parent() {
                        summary.affected_parents.push(parent.to_path_buf());
                    }
                    summary
                        .mime_families
                        .insert(Self::mime_family(&file.mime_type));
                    files.push((path_string.to_string(), id, file));
                } else {
                    orphan_paths.push(path_string.to_string());
                }
            }
        }

        if !orphan_paths.is_empty() {
            let mut path_index = transaction.open_table(PATH_INDEX)?;
            for path in orphan_paths {
                path_index.remove(path.as_str())?;
            }
        }

        summary.affected_parents.sort();
        summary.affected_parents.dedup();
        let (removed, removed_size) = Self::remove_files_from_transaction(&transaction, &files)?;
        transaction.commit()?;

        let _ = self
            .total_files
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                Some(current.saturating_sub(removed as u64))
            });
        let _ = self
            .total_size
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                Some(current.saturating_sub(removed_size))
            });
        summary.removed_files = removed;
        Ok(summary)
    }

    async fn rebuild_derived_indexes(&self) -> Result<DatabaseHealth> {
        let _mutation_guard = self.mutation_lock.lock().await;
        let txn = self.db.begin_write()?;
        let mut winners: HashMap<String, (i64, MediaFile)> = HashMap::new();
        let mut remap = HashMap::new();
        {
            let files = txn.open_table(FILES_TABLE)?;
            for entry in files.iter()? {
                let (id, bytes) = entry?;
                let file = Self::canonical_file(&Self::deserialize_media_file(bytes.value())?)?;
                let path = file.path.to_string_lossy().to_string();
                if let Some((old_id, old)) = winners.get(&path) {
                    if (file.updated_at, id.value()) > (old.updated_at, *old_id) {
                        remap.insert(*old_id, id.value());
                        winners.insert(path, (id.value(), file));
                    } else {
                        remap.insert(id.value(), *old_id);
                    }
                } else {
                    winners.insert(path, (id.value(), file));
                }
            }
        }
        {
            let mut files = txn.open_table(FILES_TABLE)?;
            let keys = files
                .iter()?
                .filter_map(|e| e.ok().map(|(k, _)| k.value()))
                .collect::<Vec<_>>();
            for key in keys {
                files.remove(key)?;
            }
            for (id, file) in winners.values_mut() {
                file.id = Some(*id);
                let bytes = Self::serialize_media_file(file)?;
                files.insert(*id, bytes.as_slice())?;
            }
        }
        macro_rules! clear_str {
            ($def:expr) => {{
                let mut table = txn.open_table($def)?;
                let keys = table
                    .iter()?
                    .filter_map(|e| e.ok().map(|(k, _)| k.value().to_string()))
                    .collect::<Vec<_>>();
                for key in keys {
                    table.remove(key.as_str())?;
                }
            }};
        }
        clear_str!(PATH_INDEX);
        clear_str!(DIR_INDEX);
        clear_str!(ARTIST_INDEX);
        clear_str!(ALBUM_INDEX);
        clear_str!(GENRE_INDEX);
        clear_str!(ALBUM_ARTIST_INDEX);
        {
            let mut table = txn.open_table(YEAR_INDEX)?;
            let keys = table
                .iter()?
                .filter_map(|e| e.ok().map(|(k, _)| k.value()))
                .collect::<Vec<_>>();
            for key in keys {
                table.remove(key)?;
            }
        }
        {
            let mut paths = txn.open_table(PATH_INDEX)?;
            let mut dirs = txn.open_table(DIR_INDEX)?;
            let mut artists = txn.open_table(ARTIST_INDEX)?;
            let mut albums = txn.open_table(ALBUM_INDEX)?;
            let mut genres = txn.open_table(GENRE_INDEX)?;
            let mut years = txn.open_table(YEAR_INDEX)?;
            let mut album_artists = txn.open_table(ALBUM_ARTIST_INDEX)?;
            for (path, (id, file)) in &winners {
                paths.insert(path.as_str(), *id)?;
                Self::add_file_indexes(
                    &mut dirs,
                    &mut artists,
                    &mut albums,
                    &mut genres,
                    &mut years,
                    &mut album_artists,
                    *id,
                    file,
                )?;
            }
        }
        let legacy = {
            let meta = txn.open_table(METADATA_TABLE)?;
            let v = meta.get("schema_version")?.is_none();
            v
        };
        if legacy {
            {
                let mut table = txn.open_table(PLAYLISTS_TABLE)?;
                let keys = table
                    .iter()?
                    .filter_map(|e| e.ok().map(|(k, _)| k.value()))
                    .collect::<Vec<_>>();
                for key in keys {
                    table.remove(key)?;
                }
            }
            clear_str!(PLAYLIST_ENTRIES);
            {
                let mut table = txn.open_table(PLAYLIST_SOURCES)?;
                let keys = table
                    .iter()?
                    .filter_map(|e| e.ok().map(|(k, _)| k.value()))
                    .collect::<Vec<_>>();
                for key in keys {
                    table.remove(key)?;
                }
            }
            txn.open_table(METADATA_TABLE)?
                .insert("schema_version", 1)?;
        }
        {
            let live = winners.values().map(|(id, _)| *id).collect::<HashSet<_>>();
            let mut entries = txn.open_table(PLAYLIST_ENTRIES)?;
            let snapshot = entries
                .iter()?
                .filter_map(|e| e.ok().map(|(k, v)| (k.value().to_string(), v.value())))
                .collect::<Vec<_>>();
            for (key, old) in snapshot {
                let id = remap.get(&old).copied().unwrap_or(old);
                if !live.contains(&id) {
                    entries.remove(key.as_str())?;
                } else if id != old {
                    entries.insert(key.as_str(), id)?;
                }
            }
        }
        txn.commit()?;
        self.total_files
            .store(winners.len() as u64, Ordering::SeqCst);
        self.total_size.store(
            winners.values().map(|(_, f)| f.size).sum(),
            Ordering::SeqCst,
        );
        Ok(DatabaseHealth {
            is_healthy: true,
            corruption_detected: !remap.is_empty(),
            integrity_check_passed: true,
            issues: Vec::new(),
            repair_attempted: true,
            repair_successful: true,
        })
    }

    async fn remove_from_playlist(&self, playlist_id: i64, media_file_id: i64) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;

            let prefix = format!("{}:", playlist_id);
            let key_to_remove: Option<String> = playlist_entries
                .iter()?
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
        let mut entries: Vec<(u32, i64)> = playlist_entries
            .iter()?
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

    async fn reorder_playlist(
        &self,
        playlist_id: i64,
        track_positions: &[(i64, u32)],
    ) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;

            // Remove existing entries for this playlist
            let prefix = format!("{}:", playlist_id);
            let keys_to_remove: Vec<String> = playlist_entries
                .iter()?
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
        let canonical = Self::canonical_path(Path::new(canonical_prefix))?;
        let prefix = canonical
            .to_string_lossy()
            .trim_end_matches('/')
            .to_string();
        let child = format!("{prefix}/");

        let read_txn = self.db.begin_read()?;
        let path_index = read_txn.open_table(PATH_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        for result in path_index.range(prefix.as_str()..)? {
            let (key, value) = result?;
            if key.value() == prefix || key.value().starts_with(&child) {
                if let Some(data) = files_table.get(value.value())? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            } else {
                break;
            }
        }

        Ok(files)
    }

    async fn get_direct_subdirectories(
        &self,
        canonical_parent_path: &str,
    ) -> Result<Vec<MediaDirectory>> {
        let canonical = Self::canonical_path(Path::new(canonical_parent_path))?;
        let canonical_parent_path = canonical.to_string_lossy().to_string();
        let prefix = if canonical_parent_path.is_empty() || canonical_parent_path == "/" {
            String::new()
        } else {
            format!("{}/", canonical_parent_path)
        };

        let read_txn = self.db.begin_read()?;
        let dir_index = read_txn.open_table(DIR_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut subdirs = HashSet::new();

        for result in dir_index.range(prefix.as_str()..)? {
            let (key, value) = result?;
            let key_str = key.value();
            if !key_str.starts_with(&prefix) {
                break;
            }
            if key_str == canonical_parent_path {
                continue;
            }
            if !Self::parse_dir_index(value.value())
                .into_iter()
                .any(|id| files_table.get(id).ok().flatten().is_some())
            {
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

    async fn batch_cleanup_missing_files(
        &self,
        existing_canonical_paths: &HashSet<String>,
    ) -> Result<usize> {
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
        mime_filter: &str,
    ) -> Result<Vec<MediaDirectory>> {
        Ok(self
            .get_directory_listing(Path::new(canonical_parent_path), mime_filter)
            .await?
            .0)
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
        let retrieved = db
            .get_file_by_path(&PathBuf::from("/music/test.mp3"))
            .await
            .unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().filename, "test.mp3");

        // Retrieve by ID
        let by_id = db.get_file_by_id(id).await.unwrap();
        assert!(by_id.is_some());

        // Remove
        let removed = db
            .remove_media_file(&PathBuf::from("/music/test.mp3"))
            .await
            .unwrap();
        assert!(removed);

        let removed_check = db
            .get_file_by_path(&PathBuf::from("/music/test.mp3"))
            .await
            .unwrap();
        assert!(removed_check.is_none());
    }

    #[tokio::test]
    async fn opening_corrupt_database_does_not_delete_original() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("corrupt.redb");
        let original = b"not a redb database";
        std::fs::write(&path, original).unwrap();

        assert!(RedbDatabase::new(path.clone()).await.is_err());
        assert_eq!(std::fs::read(path).unwrap(), original);
    }

    #[tokio::test]
    async fn test_redb_database_bulk_operations() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_bulk.redb");

        let db = RedbDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        let files: Vec<MediaFile> = (0..100)
            .map(|i| {
                MediaFile::new(
                    PathBuf::from(format!("/music/song{}.mp3", i)),
                    1024,
                    "audio/mpeg".to_string(),
                )
            })
            .collect();

        let ids = db.bulk_store_media_files(&files).await.unwrap();
        assert_eq!(ids.len(), 100);

        let stats = db.get_stats().await.unwrap();
        assert_eq!(stats.total_files, 100);
    }

    #[tokio::test]
    async fn duplicate_upsert_then_delete_does_not_leave_ghost_directory() {
        let temp = tempdir().unwrap();
        let db = RedbDatabase::new(temp.path().join("ghost.redb"))
            .await
            .unwrap();
        db.initialize().await.unwrap();
        let mut file = MediaFile::new(
            PathBuf::from("/media/deleted/movie.mkv"),
            1,
            "video/x-matroska".to_string(),
        );
        file.artist = Some("Ghost Artist".to_string());
        file.album = Some("Ghost Album".to_string());
        file.genre = Some("Ghost Genre".to_string());
        file.year = Some(2026);
        file.album_artist = Some("Ghost Album Artist".to_string());
        let first = db.store_media_file(&file).await.unwrap();
        let second = db
            .bulk_store_media_files(std::slice::from_ref(&file))
            .await
            .unwrap();
        assert_eq!(vec![first], second);
        assert_eq!(
            db.remove_media_under_path(Path::new("/media/deleted"))
                .await
                .unwrap()
                .removed_files,
            1
        );
        let (dirs, files) = db
            .get_directory_listing(Path::new("/media"), "video/")
            .await
            .unwrap();
        assert!(dirs.is_empty(), "ghost directories: {dirs:?}");
        assert!(files.is_empty());
        assert!(db.get_artists().await.unwrap().is_empty());
        assert!(db.get_albums(None).await.unwrap().is_empty());
        assert!(db.get_genres().await.unwrap().is_empty());
        assert!(db.get_years().await.unwrap().is_empty());
        assert!(db.get_album_artists().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn rebuild_removes_orphan_indexes_and_repairs_counters() {
        let temp = tempdir().unwrap();
        let db = RedbDatabase::new(temp.path().join("repair.redb"))
            .await
            .unwrap();
        db.initialize().await.unwrap();
        db.rebuild_derived_indexes().await.unwrap();

        let mut file = MediaFile::new(
            PathBuf::from("/music/orphan/song.mp3"),
            42,
            "audio/mpeg".to_string(),
        );
        file.artist = Some("Orphan Artist".to_string());
        let id = db.store_media_file(&file).await.unwrap();
        let playlist = db.create_playlist("Kept playlist", None).await.unwrap();
        db.add_to_playlist(playlist, id, Some(0)).await.unwrap();

        let transaction = db.db.begin_write().unwrap();
        transaction
            .open_table(FILES_TABLE)
            .unwrap()
            .remove(id)
            .unwrap();
        transaction.commit().unwrap();

        let health = db.rebuild_derived_indexes().await.unwrap();
        assert!(health.is_healthy);
        assert!(db.get_artists().await.unwrap().is_empty());
        assert!(db
            .get_directory_listing(Path::new("/music"), "audio/")
            .await
            .unwrap()
            .0
            .is_empty());
        assert!(db.get_playlist_tracks(playlist).await.unwrap().is_empty());
        let stats = db.get_stats().await.unwrap();
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_size, 0);
    }

    #[tokio::test]
    async fn path_prefix_removal_respects_component_boundaries() {
        let temp = tempdir().unwrap();
        let db = RedbDatabase::new(temp.path().join("prefix.redb"))
            .await
            .unwrap();
        db.initialize().await.unwrap();
        let a = MediaFile::new(PathBuf::from("/media/Film/a.mkv"), 1, "video/x".to_string());
        let b = MediaFile::new(
            PathBuf::from("/media/Films/b.mkv"),
            1,
            "video/x".to_string(),
        );
        db.bulk_store_media_files(&[a, b.clone()]).await.unwrap();
        db.remove_media_under_path(Path::new("/media/Film"))
            .await
            .unwrap();
        assert!(db.get_file_by_path(&b.path).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn directory_visibility_respects_mime_family() {
        let temp = tempdir().unwrap();
        let db = RedbDatabase::new(temp.path().join("mime-filter.redb"))
            .await
            .unwrap();
        db.initialize().await.unwrap();
        let video = MediaFile::new(
            PathBuf::from("/media/mixed/movie.mkv"),
            1,
            "video/x-matroska".to_string(),
        );
        let audio = MediaFile::new(
            PathBuf::from("/media/mixed/song.mp3"),
            1,
            "audio/mpeg".to_string(),
        );
        db.bulk_store_media_files(&[video.clone(), audio])
            .await
            .unwrap();

        assert_eq!(
            db.get_directory_listing(Path::new("/media"), "video/")
                .await
                .unwrap()
                .0
                .len(),
            1
        );
        db.remove_media_file(&video.path).await.unwrap();
        assert!(db
            .get_directory_listing(Path::new("/media"), "video/")
            .await
            .unwrap()
            .0
            .is_empty());
        assert_eq!(
            db.get_directory_listing(Path::new("/media"), "audio/")
                .await
                .unwrap()
                .0
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn test_redb_database_playlist_operations() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_playlist.redb");

        let db = RedbDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        // Store some test files
        let files: Vec<MediaFile> = (0..5)
            .map(|i| {
                MediaFile::new(
                    PathBuf::from(format!("/music/song{}.mp3", i)),
                    1024,
                    "audio/mpeg".to_string(),
                )
            })
            .collect();
        let file_ids = db.bulk_store_media_files(&files).await.unwrap();

        // Create a playlist
        let playlist_id = db
            .create_playlist("Test Playlist", Some("A test playlist"))
            .await
            .unwrap();
        assert!(playlist_id > 0);

        // Verify playlist was created
        let playlists = db.get_playlists().await.unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].name, "Test Playlist");

        // Add tracks to playlist
        for (i, file_id) in file_ids.iter().enumerate() {
            db.add_to_playlist(playlist_id, *file_id, Some(i as u32))
                .await
                .unwrap();
        }

        // Get playlist tracks
        let tracks = db.get_playlist_tracks(playlist_id).await.unwrap();
        assert_eq!(tracks.len(), 5);
        assert_eq!(tracks[0].filename, "song0.mp3");
        assert_eq!(tracks[4].filename, "song4.mp3");

        // Remove a track
        let removed = db
            .remove_from_playlist(playlist_id, file_ids[2])
            .await
            .unwrap();
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

    #[tokio::test]
    async fn deleting_source_tree_removes_derived_playlist_and_radio() {
        let temp = tempdir().unwrap();
        let db = RedbDatabase::new(temp.path().join("playlist-source.redb"))
            .await
            .unwrap();
        db.initialize().await.unwrap();
        db.rebuild_derived_indexes().await.unwrap();

        let source = PathBuf::from("/music/playlists/stations.m3u");
        let playlist = db.create_playlist("Stations", None).await.unwrap();
        db.set_playlist_source(playlist, &source).await.unwrap();
        let mut radio = MediaFile::new(
            PathBuf::from("https://radio.example/stream"),
            0,
            "audio/radio".to_string(),
        );
        radio.album = Some(
            RedbDatabase::canonical_path(&source)
                .unwrap()
                .to_string_lossy()
                .to_string(),
        );
        let radio_id = db.store_media_file(&radio).await.unwrap();

        assert_eq!(
            db.remove_derived_content_by_source(Path::new("/music/playlists"))
                .await
                .unwrap(),
            2
        );
        assert!(db.get_playlist(playlist).await.unwrap().is_none());
        assert!(db.get_file_by_id(radio_id).await.unwrap().is_none());
    }
}

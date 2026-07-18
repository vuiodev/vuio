//! RedbDatabase - ACID-compliant embedded database using redb
//!
//! This module provides a robust, memory-efficient database implementation
//! using the redb crate. Unlike RAM-based indexes, redb uses B-trees on disk,
//! allowing it to handle databases larger than available RAM.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use redb::{
    Database, MultimapTableDefinition, ReadableDatabase, ReadableMultimapTable, ReadableTable,
    TableDefinition,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

use crate::platform::DatabaseError;

use super::{
    DatabaseHealth, DatabaseManager, DatabaseReadSession, DatabaseStats, MediaDirectory, MediaFile,
    MediaFileQuery, MediaFileView, MusicCategory, MusicCategoryType, Playlist, PlaylistView,
    RemovalSummary, VisitSummary,
};

// Table definitions for redb
// Primary table: stores MediaFile data keyed by ID
const FILES_TABLE: TableDefinition<i64, &[u8]> = TableDefinition::new("files");
// Index: path -> file ID (for lookups by path)
const PATH_INDEX: TableDefinition<&str, i64> = TableDefinition::new("path_index");
// Native one-to-many indexes. ReDB keeps each ID independently ordered, so
// inserts/removals no longer parse and rewrite an entire comma-separated blob.
const DIRECTORY_PATH_INDEX: TableDefinition<&str, u64> =
    TableDefinition::new("directory_path_index");
const DIRECTORY_RECORDS: TableDefinition<u64, &str> = TableDefinition::new("directory_records");
const DIRECTORY_CHILDREN: MultimapTableDefinition<u64, u64> =
    MultimapTableDefinition::new("directory_children");
const DIRECTORY_FILES: MultimapTableDefinition<u64, i64> =
    MultimapTableDefinition::new("directory_files");
// Composite key `<directory id>:<MIME family>`. Counts are recursive, so a
// direct child can be filtered without opening or decoding any media record.
const DIRECTORY_MIME_COUNTS: TableDefinition<&str, u64> =
    TableDefinition::new("directory_mime_counts");
// Playlists table
const PLAYLISTS_TABLE: TableDefinition<i64, &[u8]> = TableDefinition::new("playlists");
// Packed `(playlist_id, position)` key -> media file, plus a reverse mapping
// used to remove dangling playlist entries without scanning every playlist.
const PLAYLIST_ENTRIES: TableDefinition<u128, i64> = TableDefinition::new("playlist_entries");
const FILE_PLAYLIST_ENTRIES: MultimapTableDefinition<i64, u128> =
    MultimapTableDefinition::new("file_playlist_entries");
const PLAYLIST_SOURCES: TableDefinition<i64, &str> = TableDefinition::new("playlist_sources");
const SOURCE_PLAYLISTS: MultimapTableDefinition<&str, i64> =
    MultimapTableDefinition::new("source_playlists");
const METADATA_TABLE: TableDefinition<&str, u64> = TableDefinition::new("metadata");

const ARTIST_INDEX: MultimapTableDefinition<&str, i64> =
    MultimapTableDefinition::new("artist_index");
const ALBUM_INDEX: MultimapTableDefinition<&str, i64> = MultimapTableDefinition::new("album_index");
const GENRE_INDEX: MultimapTableDefinition<&str, i64> = MultimapTableDefinition::new("genre_index");
const YEAR_INDEX: MultimapTableDefinition<u32, i64> = MultimapTableDefinition::new("year_index");
const ALBUM_ARTIST_INDEX: MultimapTableDefinition<&str, i64> =
    MultimapTableDefinition::new("album_artist_index");
const SCHEMA_VERSION: u64 = 4;
const CODEC_VERSION: u64 = 1;

/// RedbDatabase - ACID-compliant embedded database
pub struct RedbDatabase {
    db: Arc<Database>,
    db_path: PathBuf,
    next_file_id: AtomicI64,
    next_playlist_id: AtomicI64,
    next_directory_id: Arc<AtomicU64>,
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
        if raw
            .get(..7)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("http://"))
            || raw
                .get(..8)
                .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
        {
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
        Self::new_with_cache(path, 128).await
    }

    pub async fn new_with_cache(path: PathBuf, cache_size_mb: usize) -> Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::task::spawn_blocking(move || Self::open_sync(path, cache_size_mb))
            .await
            .context("ReDB initialization task failed")?
    }

    fn open_sync(path: PathBuf, cache_size_mb: usize) -> Result<Self> {
        // Opening or schema initialization failures are returned to the caller.
        // The application preserves the unusable file before creating a replacement.
        let mut builder = Database::builder();
        builder.set_cache_size(cache_size_mb.saturating_mul(1024 * 1024));
        let db = builder
            .create(&path)
            .with_context(|| format!("Failed to open redb database at {}", path.display()))?;

        // Initialize tables if they don't exist
        {
            let write_txn = db.begin_write()?;
            {
                let _ = write_txn.open_table(FILES_TABLE)?;
                let _ = write_txn.open_table(PATH_INDEX)?;
                let _ = write_txn.open_table(DIRECTORY_PATH_INDEX)?;
                let _ = write_txn.open_table(DIRECTORY_RECORDS)?;
                let _ = write_txn.open_multimap_table(DIRECTORY_CHILDREN)?;
                let _ = write_txn.open_multimap_table(DIRECTORY_FILES)?;
                let _ = write_txn.open_table(DIRECTORY_MIME_COUNTS)?;
                let _ = write_txn.open_table(PLAYLISTS_TABLE)?;
                let _ = write_txn.open_table(PLAYLIST_ENTRIES)?;
                let _ = write_txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
                let _ = write_txn.open_table(PLAYLIST_SOURCES)?;
                let _ = write_txn.open_multimap_table(SOURCE_PLAYLISTS)?;
                let _ = write_txn.open_table(METADATA_TABLE)?;
                let _ = write_txn.open_multimap_table(ARTIST_INDEX)?;
                let _ = write_txn.open_multimap_table(ALBUM_INDEX)?;
                let _ = write_txn.open_multimap_table(GENRE_INDEX)?;
                let _ = write_txn.open_multimap_table(YEAR_INDEX)?;
                let _ = write_txn.open_multimap_table(ALBUM_ARTIST_INDEX)?;
            }
            let existing_schema = {
                let metadata = write_txn.open_table(METADATA_TABLE)?;
                let version = metadata.get("schema_version")?.map(|value| value.value());
                version
            };
            let has_files = {
                let files = write_txn.open_table(FILES_TABLE)?;
                let present = files.iter()?.next().transpose()?.is_some();
                present
            };
            if has_files && existing_schema != Some(SCHEMA_VERSION) {
                return Err(anyhow!(
                    "Incompatible database schema {:?}; expected Rkyv schema {}",
                    existing_schema,
                    SCHEMA_VERSION
                ));
            }
            {
                let mut metadata = write_txn.open_table(METADATA_TABLE)?;
                metadata.insert("schema_version", SCHEMA_VERSION)?;
                metadata.insert("codec_version", CODEC_VERSION)?;
            }
            write_txn.commit()?;
        }

        // Get max IDs and stats for atomic counters
        let (max_file_id, max_playlist_id, max_directory_id, total_files_count, total_size_sum) = {
            let read_txn = db.begin_read()?;
            let files_table = read_txn.open_table(FILES_TABLE)?;
            let playlists_table = read_txn.open_table(PLAYLISTS_TABLE)?;
            let directories_table = read_txn.open_table(DIRECTORY_RECORDS)?;

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

            let mut max_directory = 0_u64;
            for result in directories_table.iter()? {
                if let Ok((key, _)) = result {
                    max_directory = max_directory.max(key.value());
                }
            }

            (
                max_file,
                max_playlist,
                max_directory,
                total_files_c,
                total_size_s,
            )
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
            next_directory_id: Arc::new(AtomicU64::new(max_directory_id + 1)),
            total_files: AtomicU64::new(total_files_count),
            total_size: AtomicU64::new(total_size_sum),
            mutation_lock: tokio::sync::Mutex::new(()),
        })
    }

    /// Serialize a media record into Rkyv's directly-accessible wire format.
    fn serialize_media_file(file: &MediaFile) -> Result<rkyv::util::AlignedVec> {
        rkyv::to_bytes::<rkyv::rancor::Error>(&MediaFileSerializable::from(file))
            .map_err(|error| anyhow!("Failed to archive MediaFile using Rkyv: {error}"))
    }

    /// Materialize an owned media record for legacy/ownership-requiring callers.
    fn deserialize_media_file(data: &[u8]) -> Result<MediaFile> {
        let serializable = rkyv::from_bytes::<MediaFileSerializable, rkyv::rancor::Error>(data)
            .map_err(|error| anyhow!("Failed to deserialize MediaFile using Rkyv: {error}"))?;
        Ok(serializable.into())
    }

    fn serialize_playlist(playlist: &Playlist) -> Result<rkyv::util::AlignedVec> {
        rkyv::to_bytes::<rkyv::rancor::Error>(&PlaylistSerializable::from(playlist))
            .map_err(|error| anyhow!("Failed to archive Playlist using Rkyv: {error}"))
    }

    fn deserialize_playlist(data: &[u8]) -> Result<Playlist> {
        let serializable = rkyv::from_bytes::<PlaylistSerializable, rkyv::rancor::Error>(data)
            .map_err(|error| anyhow!("Failed to deserialize Playlist using Rkyv: {error}"))?;
        Ok(serializable.into())
    }

    async fn execute_read<R, F>(&self, operation: F) -> Result<R>
    where
        R: Send + 'static,
        F: FnOnce(&Database) -> Result<R> + Send + 'static,
    {
        let database = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || operation(&database))
            .await
            .context("ReDB read task failed")?
    }

    async fn execute_write<R, F>(&self, operation: F) -> Result<R>
    where
        R: Send + 'static,
        F: FnOnce(&Database) -> Result<R> + Send + 'static,
    {
        let _mutation_guard = self.mutation_lock.lock().await;
        let database = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || operation(&database))
            .await
            .context("ReDB write task failed")?
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

    fn get_dir_key_str(path: &str) -> String {
        Self::get_dir_key(Path::new(path))
    }

    fn parent_directory(path: &str) -> Option<String> {
        let parent = Path::new(path)
            .parent()?
            .to_string_lossy()
            .replace('\\', "/");
        if parent == path || parent.is_empty() {
            None
        } else {
            Some(if cfg!(target_os = "windows") {
                parent.to_lowercase()
            } else {
                parent
            })
        }
    }

    fn ensure_directory(
        paths: &mut redb::Table<&str, u64>,
        records: &mut redb::Table<u64, &str>,
        children: &mut redb::MultimapTable<u64, u64>,
        next_directory_id: &AtomicU64,
        path: &str,
    ) -> Result<u64> {
        if let Some(id) = paths.get(path)?.map(|value| value.value()) {
            return Ok(id);
        }

        let parent_id = if let Some(parent) = Self::parent_directory(path) {
            Some(Self::ensure_directory(
                paths,
                records,
                children,
                next_directory_id,
                &parent,
            )?)
        } else {
            None
        };

        let id = next_directory_id.fetch_add(1, Ordering::SeqCst);
        paths.insert(path, id)?;
        records.insert(id, path)?;
        if let Some(parent_id) = parent_id {
            children.insert(parent_id, id)?;
        }
        Ok(id)
    }

    fn mime_count_key(directory_id: u64, mime_family: &str) -> String {
        format!("{directory_id}:{mime_family}")
    }

    fn playlist_entry_key(playlist_id: i64, position: u32) -> u128 {
        ((playlist_id as u64 as u128) << 32) | position as u128
    }

    fn playlist_entry_range(playlist_id: i64) -> std::ops::RangeInclusive<u128> {
        Self::playlist_entry_key(playlist_id, 0)
            ..=Self::playlist_entry_key(playlist_id, u32::MAX)
    }

    fn change_recursive_mime_count(
        paths: &redb::Table<&str, u64>,
        counts: &mut redb::Table<&str, u64>,
        directory_path: &str,
        mime_family: &str,
        delta: i8,
    ) -> Result<()> {
        let mut current = Some(directory_path.to_owned());
        while let Some(path) = current {
            if let Some(directory_id) = paths.get(path.as_str())?.map(|value| value.value()) {
                let key = Self::mime_count_key(directory_id, mime_family);
                let old = counts
                    .get(key.as_str())?
                    .map(|value| value.value())
                    .unwrap_or(0);
                if delta > 0 {
                    counts.insert(key.as_str(), old.saturating_add(delta as u64))?;
                } else {
                    let new = old.saturating_sub((-delta) as u64);
                    if new == 0 {
                        counts.remove(key.as_str())?;
                    } else {
                        counts.insert(key.as_str(), new)?;
                    }
                }
            }
            current = Self::parent_directory(&path);
        }
        Ok(())
    }

    fn add_directory_membership<V: MediaFileView>(
        paths: &mut redb::Table<&str, u64>,
        records: &mut redb::Table<u64, &str>,
        children: &mut redb::MultimapTable<u64, u64>,
        directory_files: &mut redb::MultimapTable<u64, i64>,
        counts: &mut redb::Table<&str, u64>,
        next_directory_id: &AtomicU64,
        file_id: i64,
        file: &V,
    ) -> Result<()> {
        let directory_path = Self::get_dir_key_str(file.path());
        let directory_id =
            Self::ensure_directory(paths, records, children, next_directory_id, &directory_path)?;
        directory_files.insert(directory_id, file_id)?;
        Self::change_recursive_mime_count(
            paths,
            counts,
            &directory_path,
            &Self::mime_family(file.mime_type()),
            1,
        )?;
        Self::change_recursive_mime_count(paths, counts, &directory_path, "*", 1)
    }

    fn remove_directory_membership<V: MediaFileView>(
        paths: &mut redb::Table<&str, u64>,
        records: &mut redb::Table<u64, &str>,
        children: &mut redb::MultimapTable<u64, u64>,
        directory_files: &mut redb::MultimapTable<u64, i64>,
        counts: &mut redb::Table<&str, u64>,
        file_id: i64,
        file: &V,
    ) -> Result<()> {
        let directory_path = Self::get_dir_key_str(file.path());
        let Some(directory_id) = paths
            .get(directory_path.as_str())?
            .map(|value| value.value())
        else {
            return Ok(());
        };
        directory_files.remove(directory_id, file_id)?;
        Self::change_recursive_mime_count(
            paths,
            counts,
            &directory_path,
            &Self::mime_family(file.mime_type()),
            -1,
        )?;
        Self::change_recursive_mime_count(paths, counts, &directory_path, "*", -1)?;

        // Prune now-empty leaf directories bottom-up. This is what guarantees
        // that a deleted folder cannot survive a restart as a stale container.
        let mut current_path = Some(directory_path);
        while let Some(path) = current_path {
            let Some(id) = paths.get(path.as_str())?.map(|value| value.value()) else {
                break;
            };
            let has_files = directory_files.get(id)?.next().transpose()?.is_some();
            let has_children = children.get(id)?.next().transpose()?.is_some();
            if has_files || has_children {
                break;
            }
            let parent_path = Self::parent_directory(&path);
            if let Some(parent) = parent_path.as_deref() {
                if let Some(parent_id) = paths.get(parent)?.map(|value| value.value()) {
                    children.remove(parent_id, id)?;
                }
            }
            paths.remove(path.as_str())?;
            records.remove(id)?;
            current_path = parent_path;
        }
        Ok(())
    }

    fn remove_file_indexes<V: MediaFileView>(
        artist: &mut redb::MultimapTable<&str, i64>,
        album: &mut redb::MultimapTable<&str, i64>,
        genre: &mut redb::MultimapTable<&str, i64>,
        year: &mut redb::MultimapTable<u32, i64>,
        album_artist: &mut redb::MultimapTable<&str, i64>,
        id: i64,
        file: &V,
    ) -> Result<()> {
        if let Some(v) = file.artist() {
            artist.remove(v, id)?;
        }
        if let Some(v) = file.album() {
            album.remove(v, id)?;
        }
        if let Some(v) = file.genre() {
            genre.remove(v, id)?;
        }
        if let Some(v) = file.year() {
            year.remove(v, id)?;
        }
        if let Some(v) = file.album_artist() {
            album_artist.remove(v, id)?;
        }
        Ok(())
    }

    fn add_file_indexes<V: MediaFileView>(
        artist: &mut redb::MultimapTable<&str, i64>,
        album: &mut redb::MultimapTable<&str, i64>,
        genre: &mut redb::MultimapTable<&str, i64>,
        year: &mut redb::MultimapTable<u32, i64>,
        album_artist: &mut redb::MultimapTable<&str, i64>,
        id: i64,
        file: &V,
    ) -> Result<()> {
        if let Some(v) = file.artist() {
            artist.insert(v, id)?;
        }
        if let Some(v) = file.album() {
            album.insert(v, id)?;
        }
        if let Some(v) = file.genre() {
            genre.insert(v, id)?;
        }
        if let Some(v) = file.year() {
            year.insert(v, id)?;
        }
        if let Some(v) = file.album_artist() {
            album_artist.insert(v, id)?;
        }
        Ok(())
    }

    fn remove_files_from_transaction(
        transaction: &redb::WriteTransaction,
        files: &[(String, i64, MediaFile)],
    ) -> Result<(usize, u64)> {
        let mut files_table = transaction.open_table(FILES_TABLE)?;
        let mut path_index = transaction.open_table(PATH_INDEX)?;
        let mut directory_paths = transaction.open_table(DIRECTORY_PATH_INDEX)?;
        let mut directory_records = transaction.open_table(DIRECTORY_RECORDS)?;
        let mut directory_children = transaction.open_multimap_table(DIRECTORY_CHILDREN)?;
        let mut directory_files = transaction.open_multimap_table(DIRECTORY_FILES)?;
        let mut directory_mime_counts = transaction.open_table(DIRECTORY_MIME_COUNTS)?;
        let mut artist_index = transaction.open_multimap_table(ARTIST_INDEX)?;
        let mut album_index = transaction.open_multimap_table(ALBUM_INDEX)?;
        let mut genre_index = transaction.open_multimap_table(GENRE_INDEX)?;
        let mut year_index = transaction.open_multimap_table(YEAR_INDEX)?;
        let mut album_artist_index = transaction.open_multimap_table(ALBUM_ARTIST_INDEX)?;
        let mut playlist_entries = transaction.open_table(PLAYLIST_ENTRIES)?;
        let mut reverse_playlist_entries =
            transaction.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;

        let mut removed_size = 0_u64;
        for (path, id, file) in files {
            files_table.remove(*id)?;
            path_index.remove(path.as_str())?;
            Self::remove_directory_membership(
                &mut directory_paths,
                &mut directory_records,
                &mut directory_children,
                &mut directory_files,
                &mut directory_mime_counts,
                *id,
                file,
            )?;
            Self::remove_file_indexes(
                &mut artist_index,
                &mut album_index,
                &mut genre_index,
                &mut year_index,
                &mut album_artist_index,
                *id,
                file,
            )?;

            let dangling = reverse_playlist_entries
                .get(*id)?
                .map(|entry| entry.map(|key| key.value()))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for key in dangling {
                playlist_entries.remove(key)?;
                reverse_playlist_entries.remove(*id, key)?;
            }
            removed_size = removed_size.saturating_add(file.size);
        }

        Ok((files.len(), removed_size))
    }
}

// Stable storage records. Keep these independent from application structs so
// schema changes are explicit and versioned.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
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
    subtitle_available: bool,
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
            subtitle_available: file.subtitle_available,
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
            subtitle_available: s.subtitle_available,
            created_at: UNIX_EPOCH + Duration::from_secs(s.created_at_secs),
            updated_at: UNIX_EPOCH + Duration::from_secs(s.updated_at_secs),
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
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

/// Validated borrowed view into one Rkyv value held by a ReDB access guard.
pub struct RkyvMediaFileView<'a> {
    archived: &'a ArchivedMediaFileSerializable,
}

pub struct RkyvPlaylistView<'a> {
    archived: &'a ArchivedPlaylistSerializable,
}

impl PlaylistView for RkyvPlaylistView<'_> {
    fn id(&self) -> Option<i64> {
        self.archived.id.as_ref().map(|value| value.to_native())
    }
    fn name(&self) -> &str {
        self.archived.name.as_str()
    }
    fn description(&self) -> Option<&str> {
        self.archived.description.as_ref().map(|value| value.as_str())
    }
    fn created_at_secs(&self) -> u64 {
        self.archived.created_at_secs.to_native()
    }
    fn updated_at_secs(&self) -> u64 {
        self.archived.updated_at_secs.to_native()
    }
}

impl MediaFileView for RkyvMediaFileView<'_> {
    fn id(&self) -> Option<i64> {
        self.archived.id.as_ref().map(|value| value.to_native())
    }
    fn path(&self) -> &str {
        self.archived.path.as_str()
    }
    fn filename(&self) -> &str {
        self.archived.filename.as_str()
    }
    fn size(&self) -> u64 {
        self.archived.size.to_native()
    }
    fn modified_secs(&self) -> u64 {
        self.archived.modified_secs.to_native()
    }
    fn mime_type(&self) -> &str {
        self.archived.mime_type.as_str()
    }
    fn duration_secs(&self) -> Option<f64> {
        self.archived
            .duration_secs
            .as_ref()
            .map(|value| value.to_native())
    }
    fn title(&self) -> Option<&str> {
        self.archived.title.as_ref().map(|value| value.as_str())
    }
    fn artist(&self) -> Option<&str> {
        self.archived.artist.as_ref().map(|value| value.as_str())
    }
    fn album(&self) -> Option<&str> {
        self.archived.album.as_ref().map(|value| value.as_str())
    }
    fn genre(&self) -> Option<&str> {
        self.archived.genre.as_ref().map(|value| value.as_str())
    }
    fn track_number(&self) -> Option<u32> {
        self.archived
            .track_number
            .as_ref()
            .map(|value| value.to_native())
    }
    fn year(&self) -> Option<u32> {
        self.archived.year.as_ref().map(|value| value.to_native())
    }
    fn album_artist(&self) -> Option<&str> {
        self.archived
            .album_artist
            .as_ref()
            .map(|value| value.as_str())
    }
    fn subtitle_available(&self) -> bool {
        self.archived.subtitle_available
    }
    fn created_at_secs(&self) -> u64 {
        self.archived.created_at_secs.to_native()
    }
    fn updated_at_secs(&self) -> u64 {
        self.archived.updated_at_secs.to_native()
    }
}

pub struct RedbReadSession {
    transaction: redb::ReadTransaction,
}

impl RedbReadSession {
    fn view(data: &[u8]) -> Result<RkyvMediaFileView<'_>> {
        let archived = rkyv::access::<ArchivedMediaFileSerializable, rkyv::rancor::Error>(data)
            .map_err(|error| anyhow!("Invalid archived MediaFile: {error}"))?;
        Ok(RkyvMediaFileView { archived })
    }
}

impl DatabaseReadSession for RedbReadSession {
    type File<'a> = RkyvMediaFileView<'a>;
    type Playlist<'a> = RkyvPlaylistView<'a>;

    fn visit_files<F>(
        &mut self,
        query: &MediaFileQuery,
        offset: usize,
        limit: usize,
        mut visitor: F,
    ) -> Result<VisitSummary>
    where
        F: for<'a> FnMut(Self::File<'a>) -> Result<()>,
    {
        let files = self.transaction.open_table(FILES_TABLE)?;
        let mut summary = VisitSummary::default();

        macro_rules! emit_id {
            ($id:expr) => {{
                let id = $id;
                summary.matched += 1;
                if summary.matched > offset && summary.visited < limit {
                    if let Some(bytes) = files.get(id)? {
                        visitor(Self::view(bytes.value())?)?;
                        summary.visited += 1;
                    }
                }
            }};
        }

        match query {
            MediaFileQuery::All => {
                for entry in files.iter()? {
                    let (_, bytes) = entry?;
                    summary.matched += 1;
                    if summary.matched > offset && summary.visited < limit {
                        visitor(Self::view(bytes.value())?)?;
                        summary.visited += 1;
                    }
                }
            }
            MediaFileQuery::Id(id) => emit_id!(*id),
            MediaFileQuery::Path(path) => {
                let paths = self.transaction.open_table(PATH_INDEX)?;
                if let Some(id) = paths.get(path.as_str())? {
                    emit_id!(id.value());
                }
            }
            MediaFileQuery::Directory { path, mime_family } => {
                let paths = self.transaction.open_table(DIRECTORY_PATH_INDEX)?;
                if let Some(directory_id) = paths.get(path.as_str())? {
                    let index = self.transaction.open_multimap_table(DIRECTORY_FILES)?;
                    for id in index.get(directory_id.value())? {
                        let id = id?.value();
                        if let Some(family) = mime_family {
                            if let Some(bytes) = files.get(id)? {
                                if !Self::view(bytes.value())?.mime_type().starts_with(family) {
                                    continue;
                                }
                            }
                        }
                        emit_id!(id);
                    }
                }
            }
            MediaFileQuery::Artist(value) => {
                let index = self.transaction.open_multimap_table(ARTIST_INDEX)?;
                for id in index.get(value.as_str())? {
                    emit_id!(id?.value());
                }
            }
            MediaFileQuery::Album { album, artist } => {
                let index = self.transaction.open_multimap_table(ALBUM_INDEX)?;
                for id in index.get(album.as_str())? {
                    let id = id?.value();
                    if let Some(expected_artist) = artist {
                        if let Some(bytes) = files.get(id)? {
                            if Self::view(bytes.value())?.artist() != Some(expected_artist.as_str())
                            {
                                continue;
                            }
                        }
                    }
                    emit_id!(id);
                }
            }
            MediaFileQuery::Genre(value) => {
                let index = self.transaction.open_multimap_table(GENRE_INDEX)?;
                for id in index.get(value.as_str())? {
                    emit_id!(id?.value());
                }
            }
            MediaFileQuery::Year(value) => {
                let index = self.transaction.open_multimap_table(YEAR_INDEX)?;
                for id in index.get(*value)? {
                    emit_id!(id?.value());
                }
            }
            MediaFileQuery::AlbumArtist(value) => {
                let index = self.transaction.open_multimap_table(ALBUM_ARTIST_INDEX)?;
                for id in index.get(value.as_str())? {
                    emit_id!(id?.value());
                }
            }
            MediaFileQuery::Playlist(playlist_id) => {
                let entries = self.transaction.open_table(PLAYLIST_ENTRIES)?;
                for entry in entries.range(RedbDatabase::playlist_entry_range(*playlist_id))? {
                    let (_, id) = entry?;
                    emit_id!(id.value());
                }
            }
        }

        Ok(summary)
    }

    fn direct_subdirectories(
        &mut self,
        canonical_parent: &str,
        mime_family: Option<&str>,
    ) -> Result<Vec<MediaDirectory>> {
        let paths = self.transaction.open_table(DIRECTORY_PATH_INDEX)?;
        let records = self.transaction.open_table(DIRECTORY_RECORDS)?;
        let children = self.transaction.open_multimap_table(DIRECTORY_CHILDREN)?;
        let counts = self.transaction.open_table(DIRECTORY_MIME_COUNTS)?;
        let Some(parent_id) = paths
            .get(canonical_parent)?
            .map(|value| value.value())
        else {
            return Ok(Vec::new());
        };
        let family = mime_family.filter(|value| !value.is_empty()).unwrap_or("*");
        let mut directories = Vec::new();
        for child in children.get(parent_id)? {
            let child_id = child?.value();
            let key = RedbDatabase::mime_count_key(child_id, family);
            if counts
                .get(key.as_str())?
                .is_none_or(|value| value.value() == 0)
            {
                continue;
            }
            if let Some(path) = records.get(child_id)? {
                let path = path.value().to_owned();
                let name = Path::new(&path)
                    .file_name()
                    .map(|value| value.to_string_lossy().into_owned())
                    .unwrap_or_default();
                directories.push(MediaDirectory {
                    path: PathBuf::from(path),
                    name,
                });
            }
        }
        directories.sort_by_cached_key(|directory| directory.name.to_lowercase());
        Ok(directories)
    }

    fn visit_playlists<F>(
        &mut self,
        offset: usize,
        limit: usize,
        mut visitor: F,
    ) -> Result<VisitSummary>
    where
        F: for<'a> FnMut(Self::Playlist<'a>) -> Result<()>,
    {
        let table = self.transaction.open_table(PLAYLISTS_TABLE)?;
        let mut summary = VisitSummary::default();
        for entry in table.iter()? {
            let (_, bytes) = entry?;
            summary.matched += 1;
            if summary.matched <= offset || summary.visited >= limit {
                continue;
            }
            let archived = rkyv::access::<ArchivedPlaylistSerializable, rkyv::rancor::Error>(
                bytes.value(),
            )
            .map_err(|error| anyhow!("Invalid archived Playlist: {error}"))?;
            visitor(RkyvPlaylistView { archived })?;
            summary.visited += 1;
        }
        Ok(summary)
    }
}

#[async_trait]
impl DatabaseManager for RedbDatabase {
    type ReadSession = RedbReadSession;

    async fn read<R, F>(self: Arc<Self>, operation: F) -> Result<R>
    where
        R: Send + 'static,
        F: FnOnce(&mut Self::ReadSession) -> Result<R> + Send + 'static,
    {
        let database = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let transaction = database.begin_read()?;
            let mut session = RedbReadSession { transaction };
            operation(&mut session)
        })
        .await
        .context("ReDB read task failed")?
    }

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
        let (sender, receiver) = tokio::sync::mpsc::channel(32);
        tokio::task::spawn_blocking(move || {
            let operation = || -> std::result::Result<(), DatabaseError> {
                let read_txn = db.begin_read().map_err(|error| DatabaseError::QueryFailed {
                    query: "begin_read".into(),
                    reason: error.to_string(),
                })?;
                let files = read_txn.open_table(FILES_TABLE).map_err(|error| {
                    DatabaseError::QueryFailed {
                        query: "open_table".into(),
                        reason: error.to_string(),
                    }
                })?;
                for entry in files.iter().map_err(|error| DatabaseError::QueryFailed {
                    query: "iter".into(),
                    reason: error.to_string(),
                })? {
                    let (_, bytes) = entry.map_err(|error| DatabaseError::QueryFailed {
                        query: "next".into(),
                        reason: error.to_string(),
                    })?;
                    let file = Self::deserialize_media_file(bytes.value()).map_err(|error| {
                        DatabaseError::QueryFailed {
                            query: "deserialize".into(),
                            reason: error.to_string(),
                        }
                    })?;
                    if sender.blocking_send(Ok(file)).is_err() {
                        return Ok(());
                    }
                }
                Ok(())
            };
            if let Err(error) = operation() {
                let _ = sender.blocking_send(Err(error));
            }
        });
        Box::pin(tokio_stream::wrappers::ReceiverStream::new(receiver))
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
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let files_table = read_txn.open_table(FILES_TABLE)?;
            let directory_paths = read_txn.open_table(DIRECTORY_PATH_INDEX)?;
            let directory_files = read_txn.open_multimap_table(DIRECTORY_FILES)?;
            let file_ids = if let Some(directory_id) = directory_paths.get(dir_key.as_str())? {
                directory_files
                    .get(directory_id.value())?
                    .map(|value| value.map(|value| value.value()))
                    .collect::<std::result::Result<Vec<_>, _>>()?
            } else {
                Vec::new()
            };

            let mut files = Vec::new();
            for file_id in file_ids {
                if let Some(data) = files_table.get(file_id)? {
                    let file = Self::deserialize_media_file(data.value())?;
                    files.push(file);
                }
            }

            Ok(files)
        })
        .await
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
        let media_type_filter = media_type_filter.to_owned();
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let files_table = read_txn.open_table(FILES_TABLE)?;
            let directory_paths = read_txn.open_table(DIRECTORY_PATH_INDEX)?;
            let directory_records = read_txn.open_table(DIRECTORY_RECORDS)?;
            let directory_children = read_txn.open_multimap_table(DIRECTORY_CHILDREN)?;
            let directory_files = read_txn.open_multimap_table(DIRECTORY_FILES)?;
            let directory_mime_counts = read_txn.open_table(DIRECTORY_MIME_COUNTS)?;

            let Some(parent_id) = directory_paths
                .get(parent_str.as_str())?
                .map(|value| value.value())
            else {
                return Ok((Vec::new(), Vec::new()));
            };

            let mut files = Vec::new();

            // Get files in this directory
            let file_ids = directory_files
                .get(parent_id)?
                .map(|value| value.map(|value| value.value()))
                .collect::<std::result::Result<Vec<_>, _>>()?;

            debug!(
                "get_directory_listing: found {} file IDs for dir '{}'",
                file_ids.len(),
                parent_str
            );

            for file_id in file_ids {
                if let Some(data) = files_table.get(file_id)? {
                    let file = Self::deserialize_media_file(data.value())?;
                    if media_type_filter.is_empty()
                        || file.mime_type.starts_with(&media_type_filter)
                    {
                        files.push(file);
                    }
                }
            }

            let count_family = if media_type_filter.is_empty() {
                "*"
            } else {
                media_type_filter.as_str()
            };
            let mut directories = Vec::new();
            for child in directory_children.get(parent_id)? {
                let child_id = child?.value();
                let count_key = Self::mime_count_key(child_id, count_family);
                if directory_mime_counts
                    .get(count_key.as_str())?
                    .is_none_or(|count| count.value() == 0)
                {
                    continue;
                }
                if let Some(path) = directory_records.get(child_id)? {
                    let path = path.value().to_owned();
                    let name = PathBuf::from(&path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    directories.push(MediaDirectory {
                        path: PathBuf::from(&path),
                        name,
                    });
                }
            }

            // Sort subdirectories case-insensitively
            directories.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

            // Sort files by track number if available, then case-insensitively by filename
            files.sort_by(|a, b| match (a.track_number, b.track_number) {
                (Some(ta), Some(tb)) if ta != tb => ta.cmp(&tb),
                _ => a.filename.to_lowercase().cmp(&b.filename.to_lowercase()),
            });

            Ok((directories, files))
        })
        .await
    }

    async fn cleanup_missing_files(&self, existing_paths: &[PathBuf]) -> Result<usize> {
        let existing_set: HashSet<String> = existing_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        // First, collect all paths to remove
        let paths_to_remove: Vec<PathBuf> = self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let path_index = read_txn.open_table(PATH_INDEX)?;

            Ok(path_index
                .iter()?
                .filter_map(|r| r.ok())
                .filter(|(k, _)| !existing_set.contains(k.value()))
                .map(|(k, _)| PathBuf::from(k.value()))
                .collect())
        }).await?;

        // Use batch removal
        self.bulk_remove_media_files(&paths_to_remove).await
    }

    async fn get_file_by_path(&self, path: &Path) -> Result<Option<MediaFile>> {
        let path_str = Self::canonical_path(path)?.to_string_lossy().to_string();
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let path_index = read_txn.open_table(PATH_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            if let Some(file_id) = path_index.get(path_str.as_str())?.map(|v| v.value()) {
                if let Some(data) = files_table.get(file_id)? {
                    return Ok(Some(Self::deserialize_media_file(data.value())?));
                }
            }

            Ok(None)
        })
        .await
    }

    async fn get_file_by_id(&self, id: i64) -> Result<Option<MediaFile>> {
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            if let Some(data) = files_table.get(id)? {
                return Ok(Some(Self::deserialize_media_file(data.value())?));
            }

            Ok(None)
        })
        .await
    }

    async fn get_stats(&self) -> Result<DatabaseStats> {
        let total_files = self.total_files.load(Ordering::SeqCst) as usize;
        let total_size = self.total_size.load(Ordering::SeqCst);
        let database_size = tokio::fs::metadata(&self.db_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        let (video_files, audio_files, image_files, playlists) = self.execute_read(|database| {
            let transaction = database.begin_read()?;
            let files = transaction.open_table(FILES_TABLE)?;
            let mut video = 0;
            let mut audio = 0;
            let mut image = 0;
            for entry in files.iter()? {
                let (_, bytes) = entry?;
                let view = RedbReadSession::view(bytes.value())?;
                if view.mime_type().starts_with("video/") {
                    video += 1;
                } else if view.mime_type().starts_with("audio/") {
                    audio += 1;
                } else if view.mime_type().starts_with("image/") {
                    image += 1;
                }
            }
            let playlists = transaction.open_table(PLAYLISTS_TABLE)?.iter()?.count();
            Ok((video, audio, image, playlists))
        }).await?;

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
        self.execute_read(|database| {
        let read_txn = database.begin_read()?;
        let artist_index = read_txn.open_multimap_table(ARTIST_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut categories = Vec::new();
        for result in artist_index.iter()? {
            let (key, value) = result?;
            let artist_name = key.value().to_string();
            let count = value
                .filter_map(|id| id.ok().map(|id| id.value()))
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
        }).await
    }

    async fn get_albums(&self, artist_filter: Option<&str>) -> Result<Vec<MusicCategory>> {
        let artist_filter = artist_filter.map(str::to_owned);
        self.execute_read(move |database| {
        let read_txn = database.begin_read()?;
        let album_index = read_txn.open_multimap_table(ALBUM_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut categories = Vec::new();
        for result in album_index.iter()? {
            let (key, value) = result?;
            let album_name = key.value().to_string();
            let file_ids = value
                .map(|id| id.map(|id| id.value()))
                .collect::<std::result::Result<Vec<_>, _>>()?;

            let count = if let Some(artist) = artist_filter.as_deref() {
                let mut matched = 0;
                for fid in file_ids {
                    if let Some(data) = files_table.get(fid)? {
                        if let Ok(file) = RedbReadSession::view(data.value()) {
                            if file.artist() == Some(artist) {
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
        }).await
    }

    async fn get_genres(&self) -> Result<Vec<MusicCategory>> {
        self.execute_read(|database| {
        let read_txn = database.begin_read()?;
        let genre_index = read_txn.open_multimap_table(GENRE_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut categories = Vec::new();
        for result in genre_index.iter()? {
            let (key, value) = result?;
            let name = key.value().to_string();
            let count = value
                .filter_map(|id| id.ok().map(|id| id.value()))
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
        }).await
    }

    async fn get_years(&self) -> Result<Vec<MusicCategory>> {
        self.execute_read(|database| {
        let read_txn = database.begin_read()?;
        let year_index = read_txn.open_multimap_table(YEAR_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut categories = Vec::new();
        for result in year_index.iter()? {
            let (key, value) = result?;
            let year = key.value();
            let count = value
                .filter_map(|id| id.ok().map(|id| id.value()))
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
        }).await
    }

    async fn get_album_artists(&self) -> Result<Vec<MusicCategory>> {
        self.execute_read(|database| {
        let read_txn = database.begin_read()?;
        let album_artist_index = read_txn.open_multimap_table(ALBUM_ARTIST_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut categories = Vec::new();
        for result in album_artist_index.iter()? {
            let (key, value) = result?;
            let name = key.value().to_string();
            let count = value
                .filter_map(|id| id.ok().map(|id| id.value()))
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
        }).await
    }

    async fn get_music_by_artist(&self, artist: &str) -> Result<Vec<MediaFile>> {
        let artist = artist.to_owned();
        self.execute_read(move |database| {
        let read_txn = database.begin_read()?;
        let artist_index = read_txn.open_multimap_table(ARTIST_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut files = Vec::new();
        for fid in artist_index.get(artist.as_str())? {
            let fid = fid?.value();
            if let Some(data) = files_table.get(fid)? {
                files.push(Self::deserialize_media_file(data.value())?);
            }
        }
        Ok(files)
        }).await
    }

    async fn get_music_by_album(
        &self,
        album: &str,
        artist: Option<&str>,
    ) -> Result<Vec<MediaFile>> {
        let album = album.to_owned();
        let artist = artist.map(str::to_owned);
        self.execute_read(move |database| {
        let read_txn = database.begin_read()?;
        let album_index = read_txn.open_multimap_table(ALBUM_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut files = Vec::new();
        for fid in album_index.get(album.as_str())? {
            let fid = fid?.value();
            if let Some(data) = files_table.get(fid)? {
                let file = Self::deserialize_media_file(data.value())?;
                if let Some(art) = artist.as_deref() {
                    if file.artist.as_deref() != Some(art) {
                        continue;
                    }
                }
                files.push(file);
            }
        }
        Ok(files)
        }).await
    }

    async fn get_music_by_genre(&self, genre: &str) -> Result<Vec<MediaFile>> {
        let genre = genre.to_owned();
        self.execute_read(move |database| {
        let read_txn = database.begin_read()?;
        let genre_index = read_txn.open_multimap_table(GENRE_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut files = Vec::new();
        for fid in genre_index.get(genre.as_str())? {
            let fid = fid?.value();
            if let Some(data) = files_table.get(fid)? {
                files.push(Self::deserialize_media_file(data.value())?);
            }
        }
        Ok(files)
        }).await
    }

    async fn get_music_by_year(&self, year: u32) -> Result<Vec<MediaFile>> {
        self.execute_read(move |database| {
        let read_txn = database.begin_read()?;
        let year_index = read_txn.open_multimap_table(YEAR_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut files = Vec::new();
        for fid in year_index.get(year)? {
            let fid = fid?.value();
            if let Some(data) = files_table.get(fid)? {
                files.push(Self::deserialize_media_file(data.value())?);
            }
        }
        Ok(files)
        }).await
    }

    async fn get_music_by_album_artist(&self, album_artist: &str) -> Result<Vec<MediaFile>> {
        let album_artist = album_artist.to_owned();
        self.execute_read(move |database| {
        let read_txn = database.begin_read()?;
        let album_artist_index = read_txn.open_multimap_table(ALBUM_ARTIST_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut files = Vec::new();
        for fid in album_artist_index.get(album_artist.as_str())? {
            let fid = fid?.value();
            if let Some(data) = files_table.get(fid)? {
                files.push(Self::deserialize_media_file(data.value())?);
            }
        }
        Ok(files)
        }).await
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

        self.execute_write(move |database| {
            let write_txn = database.begin_write()?;
            {
                write_txn
                    .open_table(PLAYLISTS_TABLE)?
                    .insert(playlist_id, serialized.as_slice())?;
            }
            write_txn.commit()?;
            Ok(())
        })
        .await?;

        info!("Created playlist '{}' with ID {}", name, playlist_id);
        Ok(playlist_id)
    }

    async fn get_playlists(&self) -> Result<Vec<Playlist>> {
        self.execute_read(move |database| {
        let read_txn = database.begin_read()?;
        let playlists_table = read_txn.open_table(PLAYLISTS_TABLE)?;

        let mut playlists = Vec::new();
        for result in playlists_table.iter()? {
            let (_, value) = result?;
            if let Ok(playlist) = Self::deserialize_playlist(value.value()) {
                playlists.push(playlist);
            }
        }

        Ok(playlists)
        }).await
    }

    async fn get_playlist(&self, playlist_id: i64) -> Result<Option<Playlist>> {
        self.execute_read(move |database| {
        let read_txn = database.begin_read()?;
        let playlists_table = read_txn.open_table(PLAYLISTS_TABLE)?;

        if let Some(data) = playlists_table.get(playlist_id)? {
            return Ok(Some(Self::deserialize_playlist(data.value())?));
        }

        Ok(None)
        }).await
    }

    async fn update_playlist(&self, playlist: &Playlist) -> Result<()> {
        let Some(playlist_id) = playlist.id else {
            return Err(anyhow!("Cannot update playlist without ID"));
        };

        let serialized = Self::serialize_playlist(playlist)?;

        self.execute_write(move |database| {
            let write_txn = database.begin_write()?;
            {
                write_txn
                    .open_table(PLAYLISTS_TABLE)?
                    .insert(playlist_id, serialized.as_slice())?;
            }
            write_txn.commit()?;
            Ok(())
        }).await
    }

    async fn delete_playlist(&self, playlist_id: i64) -> Result<bool> {
        self.execute_write(move |database| {
        let write_txn = database.begin_write()?;
        let removed = {
            let mut playlists_table = write_txn.open_table(PLAYLISTS_TABLE)?;
            let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
            let mut reverse_entries = write_txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
            let mut playlist_sources = write_txn.open_table(PLAYLIST_SOURCES)?;
            let mut source_playlists = write_txn.open_multimap_table(SOURCE_PLAYLISTS)?;

            let existed = playlists_table.remove(playlist_id)?.is_some();

            // Remove all entries for this playlist
            let entries = playlist_entries
                .range(Self::playlist_entry_range(playlist_id))?
                .filter_map(|entry| entry.ok().map(|(key, file)| (key.value(), file.value())))
                .collect::<Vec<_>>();
            for (key, file_id) in entries {
                playlist_entries.remove(key)?;
                reverse_entries.remove(file_id, key)?;
            }
            if let Some(source) = playlist_sources
                .get(playlist_id)?
                .map(|value| value.value().to_owned())
            {
                source_playlists.remove(source.as_str(), playlist_id)?;
            }
            playlist_sources.remove(playlist_id)?;

            existed
        };
        write_txn.commit()?;

        Ok(removed)
        }).await
    }

    async fn set_playlist_source(&self, playlist_id: i64, source_path: &Path) -> Result<()> {
        let source = Self::canonical_path(source_path)?
            .to_string_lossy()
            .to_string();
        self.execute_write(move |database| {
            let txn = database.begin_write()?;
            {
                let mut sources = txn.open_table(PLAYLIST_SOURCES)?;
                let mut reverse = txn.open_multimap_table(SOURCE_PLAYLISTS)?;
                if let Some(old) = sources
                    .get(playlist_id)?
                    .map(|value| value.value().to_owned())
                {
                    reverse.remove(old.as_str(), playlist_id)?;
                }
                sources.insert(playlist_id, source.as_str())?;
                reverse.insert(source.as_str(), playlist_id)?;
            }
            txn.commit()?;
            Ok(())
        }).await
    }

    async fn remove_derived_content_by_source(&self, source_path: &Path) -> Result<usize> {
        let source = Self::canonical_path(source_path)?
            .to_string_lossy()
            .to_string();
        let child_prefix = format!("{}/", source.trim_end_matches('/'));
        let matches_source =
            |candidate: &str| candidate == source || candidate.starts_with(&child_prefix);
        let source_for_query = source.clone();
        let child_for_query = child_prefix.clone();
        let ids = self.execute_read(move |database| {
            let txn = database.begin_read()?;
            let table = txn.open_multimap_table(SOURCE_PLAYLISTS)?;
            let mut ids = Vec::new();
            for entry in table.range(source_for_query.as_str()..)? {
                let (key, values) = entry?;
                if key.value() != source_for_query && !key.value().starts_with(&child_for_query) {
                    break;
                }
                for value in values {
                    ids.push(value?.value());
                }
            }
            ids.sort_unstable();
            ids.dedup();
            Ok(ids)
        }).await?;
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
        let key = Self::playlist_entry_key(playlist_id, pos);

        self.execute_write(move |database| {
            let write_txn = database.begin_write()?;
            {
                let mut entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
                let old = entries.insert(key, media_file_id)?.map(|value| value.value());
                let mut reverse = write_txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
                if let Some(old) = old {
                    reverse.remove(old, key)?;
                }
                reverse.insert(media_file_id, key)?;
            }
            write_txn.commit()?;
            Ok(media_file_id)
        }).await
    }

    async fn batch_add_to_playlist(
        &self,
        playlist_id: i64,
        media_file_ids: &[(i64, u32)],
    ) -> Result<Vec<i64>> {
        let media_file_ids = media_file_ids.to_vec();
        self.execute_write(move |database| {
        let write_txn = database.begin_write()?;
        {
            let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
            let mut reverse_entries = write_txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
            for (file_id, position) in &media_file_ids {
                let key = Self::playlist_entry_key(playlist_id, *position);
                if let Some(old) = playlist_entries.insert(key, *file_id)?.map(|value| value.value()) {
                    reverse_entries.remove(old, key)?;
                }
                reverse_entries.insert(*file_id, key)?;
            }
        }
        write_txn.commit()?;

        Ok(media_file_ids.iter().map(|(id, _)| *id).collect())
        }).await
    }

    async fn get_files_by_paths(&self, paths: &[PathBuf]) -> Result<Vec<MediaFile>> {
        let paths = paths.iter().map(|path| Self::canonical_path(path).map(|value| value.to_string_lossy().into_owned())).collect::<Result<Vec<_>>>()?;
        self.execute_read(move |database| {
        let mut files = Vec::new();
        let read_txn = database.begin_read()?;
        let path_index = read_txn.open_table(PATH_INDEX)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        for path_str in paths {
            if let Some(file_id) = path_index.get(path_str.as_str())?.map(|v| v.value()) {
                if let Some(data) = files_table.get(file_id)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
        }

        Ok(files)
        }).await
    }

    async fn bulk_store_media_files(&self, files: &[MediaFile]) -> Result<Vec<i64>> {
        let inputs = files.to_vec();
        let candidate_ids = inputs
            .iter()
            .map(|file| {
                file.id
                    .unwrap_or_else(|| self.next_file_id.fetch_add(1, Ordering::SeqCst))
            })
            .collect::<Vec<_>>();
        let next_directory_id = Arc::clone(&self.next_directory_id);
        let (ids, added_files, replaced_size, stored_size) = self
            .execute_write(move |database| {
                let mut ids = Vec::with_capacity(inputs.len());
                let mut added_files = 0_u64;
                let mut replaced_size = 0_u64;
                let mut stored_size = 0_u64;

                let write_txn = database.begin_write()?;
                {
                    let mut files_table = write_txn.open_table(FILES_TABLE)?;
                    let mut path_index = write_txn.open_table(PATH_INDEX)?;
                    let mut directory_paths = write_txn.open_table(DIRECTORY_PATH_INDEX)?;
                    let mut directory_records = write_txn.open_table(DIRECTORY_RECORDS)?;
                    let mut directory_children =
                        write_txn.open_multimap_table(DIRECTORY_CHILDREN)?;
                    let mut directory_files = write_txn.open_multimap_table(DIRECTORY_FILES)?;
                    let mut directory_mime_counts = write_txn.open_table(DIRECTORY_MIME_COUNTS)?;

                    let mut artist_index = write_txn.open_multimap_table(ARTIST_INDEX)?;
                    let mut album_index = write_txn.open_multimap_table(ALBUM_INDEX)?;
                    let mut genre_index = write_txn.open_multimap_table(GENRE_INDEX)?;
                    let mut year_index = write_txn.open_multimap_table(YEAR_INDEX)?;
                    let mut album_artist_index =
                        write_txn.open_multimap_table(ALBUM_ARTIST_INDEX)?;
                    let mut archive_scratch: rkyv::util::AlignedVec =
                        rkyv::util::AlignedVec::new();

                    for (input, candidate_id) in inputs.iter().zip(candidate_ids) {
                        let file = Self::canonical_file(input)?;
                        let path_str = file.path.to_string_lossy().to_string();
                        let existing_path_id =
                            path_index.get(path_str.as_str())?.map(|v| v.value());
                        let file_id = existing_path_id.or(file.id).unwrap_or(candidate_id);
                        ids.push(file_id);

                        let mut file_with_id = file.clone();
                        file_with_id.id = Some(file_id);
                        let had_old = if let Some(old_bytes) = files_table.get(file_id)? {
                            let old = RedbReadSession::view(old_bytes.value())?;
                            Self::remove_directory_membership(
                                &mut directory_paths,
                                &mut directory_records,
                                &mut directory_children,
                                &mut directory_files,
                                &mut directory_mime_counts,
                                file_id,
                                &old,
                            )?;
                            Self::remove_file_indexes(
                                &mut artist_index,
                                &mut album_index,
                                &mut genre_index,
                                &mut year_index,
                                &mut album_artist_index,
                                file_id,
                                &old,
                            )?;
                            if old.path() != path_str {
                                path_index.remove(old.path())?;
                            }
                            replaced_size = replaced_size.saturating_add(old.size());
                            true
                        } else {
                            false
                        };
                        if !had_old {
                            added_files = added_files.saturating_add(1);
                        }

                        archive_scratch.clear();
                        archive_scratch = rkyv::api::high::to_bytes_in::<
                            _,
                            rkyv::rancor::Error,
                        >(
                            &MediaFileSerializable::from(&file_with_id),
                            archive_scratch,
                        )
                        .map_err(|error| {
                            anyhow!("Failed to archive MediaFile using Rkyv: {error}")
                        })?;
                        files_table.insert(file_id, archive_scratch.as_slice())?;
                        path_index.insert(path_str.as_str(), file_id)?;
                        Self::add_directory_membership(
                            &mut directory_paths,
                            &mut directory_records,
                            &mut directory_children,
                            &mut directory_files,
                            &mut directory_mime_counts,
                            &next_directory_id,
                            file_id,
                            &file_with_id,
                        )?;
                        Self::add_file_indexes(
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
                Ok((ids, added_files, replaced_size, stored_size))
            })
            .await?;
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
        let paths = paths
            .iter()
            .map(|path| Self::canonical_path(path).map(|path| path.to_string_lossy().to_string()))
            .collect::<Result<Vec<_>>>()?;
        let (removed, removed_size) = self
            .execute_write(move |database| {
                let transaction = database.begin_write()?;
                let mut files = Vec::new();
                let mut orphan_paths = Vec::new();
                let mut seen_ids = HashSet::new();

                {
                    let path_index = transaction.open_table(PATH_INDEX)?;
                    let files_table = transaction.open_table(FILES_TABLE)?;
                    for path_string in &paths {
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
                            let file =
                                Self::canonical_file(&Self::deserialize_media_file(data.value())?)?;
                            files.push((path_string.clone(), id, file));
                        } else {
                            orphan_paths.push(path_string.clone());
                        }
                    }
                }

                if !orphan_paths.is_empty() {
                    let mut path_index = transaction.open_table(PATH_INDEX)?;
                    for path in orphan_paths {
                        path_index.remove(path.as_str())?;
                    }
                }

                let result = Self::remove_files_from_transaction(&transaction, &files)?;
                transaction.commit()?;
                Ok(result)
            })
            .await?;
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
        let canonical = Self::canonical_path(path)?;
        let prefix = canonical
            .to_string_lossy()
            .trim_end_matches('/')
            .to_string();
        let (mut summary, removed, removed_size) = self
            .execute_write(move |database| {
                let transaction = database.begin_write()?;
                let mut files = Vec::new();
                let mut seen_ids = HashSet::new();
                let mut summary = RemovalSummary::default();

                {
                    let path_index = transaction.open_table(PATH_INDEX)?;
                    let files_table = transaction.open_table(FILES_TABLE)?;
                    let directory_paths = transaction.open_table(DIRECTORY_PATH_INDEX)?;
                    let directory_children =
                        transaction.open_multimap_table(DIRECTORY_CHILDREN)?;
                    let directory_files = transaction.open_multimap_table(DIRECTORY_FILES)?;

                    let mut file_ids = Vec::new();
                    if let Some(root_id) = directory_paths
                        .get(prefix.as_str())?
                        .map(|value| value.value())
                    {
                        let mut stack = vec![root_id];
                        while let Some(directory_id) = stack.pop() {
                            for child in directory_children.get(directory_id)? {
                                stack.push(child?.value());
                            }
                            for file_id in directory_files.get(directory_id)? {
                                file_ids.push(file_id?.value());
                            }
                        }
                    } else if let Some(file_id) =
                        path_index.get(prefix.as_str())?.map(|value| value.value())
                    {
                        file_ids.push(file_id);
                    }

                    for id in file_ids {
                        if !seen_ids.insert(id) {
                            continue;
                        }
                        if let Some(data) = files_table.get(id)? {
                            let file =
                                Self::canonical_file(&Self::deserialize_media_file(data.value())?)?;
                            if let Some(parent) = file.path.parent() {
                                summary.affected_parents.push(parent.to_path_buf());
                            }
                            summary
                                .mime_families
                                .insert(Self::mime_family(&file.mime_type));
                            files.push((file.path.to_string_lossy().into_owned(), id, file));
                        }
                    }
                }

                summary.affected_parents.sort();
                summary.affected_parents.dedup();
                let (removed, removed_size) =
                    Self::remove_files_from_transaction(&transaction, &files)?;
                transaction.commit()?;
                Ok((summary, removed, removed_size))
            })
            .await?;

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
        let next_directory_id = Arc::clone(&self.next_directory_id);
        let (health, total_files, total_size) = self.execute_write(move |database| {
        let txn = database.begin_write()?;
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
        macro_rules! clear_table_str {
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
        clear_table_str!(PATH_INDEX);
        clear_table_str!(DIRECTORY_PATH_INDEX);
        clear_table_str!(DIRECTORY_MIME_COUNTS);
        {
            let mut table = txn.open_table(DIRECTORY_RECORDS)?;
            let keys = table
                .iter()?
                .filter_map(|entry| entry.ok().map(|(key, _)| key.value()))
                .collect::<Vec<_>>();
            for key in keys {
                table.remove(key)?;
            }
        }
        macro_rules! clear_multimap_str {
            ($def:expr) => {{
                let mut table = txn.open_multimap_table($def)?;
                let keys = table
                    .iter()?
                    .filter_map(|entry| entry.ok().map(|(key, _)| key.value().to_string()))
                    .collect::<Vec<_>>();
                for key in keys {
                    let _ = table.remove_all(key.as_str())?;
                }
            }};
        }
        clear_multimap_str!(ARTIST_INDEX);
        clear_multimap_str!(ALBUM_INDEX);
        clear_multimap_str!(GENRE_INDEX);
        clear_multimap_str!(ALBUM_ARTIST_INDEX);
        macro_rules! clear_multimap_u64 {
            ($definition:expr) => {{
                let mut table = txn.open_multimap_table($definition)?;
                let keys = table
                    .iter()?
                    .filter_map(|entry| entry.ok().map(|(key, _)| key.value()))
                    .collect::<Vec<_>>();
                for key in keys {
                    let _ = table.remove_all(key)?;
                }
            }};
        }
        clear_multimap_u64!(DIRECTORY_CHILDREN);
        clear_multimap_u64!(DIRECTORY_FILES);
        {
            let mut table = txn.open_multimap_table(YEAR_INDEX)?;
            let keys = table
                .iter()?
                .filter_map(|e| e.ok().map(|(k, _)| k.value()))
                .collect::<Vec<_>>();
            for key in keys {
                let _ = table.remove_all(key)?;
            }
        }
        {
            let mut paths = txn.open_table(PATH_INDEX)?;
            let mut directory_paths = txn.open_table(DIRECTORY_PATH_INDEX)?;
            let mut directory_records = txn.open_table(DIRECTORY_RECORDS)?;
            let mut directory_children = txn.open_multimap_table(DIRECTORY_CHILDREN)?;
            let mut directory_files = txn.open_multimap_table(DIRECTORY_FILES)?;
            let mut directory_mime_counts = txn.open_table(DIRECTORY_MIME_COUNTS)?;
            let mut artists = txn.open_multimap_table(ARTIST_INDEX)?;
            let mut albums = txn.open_multimap_table(ALBUM_INDEX)?;
            let mut genres = txn.open_multimap_table(GENRE_INDEX)?;
            let mut years = txn.open_multimap_table(YEAR_INDEX)?;
            let mut album_artists = txn.open_multimap_table(ALBUM_ARTIST_INDEX)?;
            for (path, (id, file)) in &winners {
                paths.insert(path.as_str(), *id)?;
                Self::add_directory_membership(
                    &mut directory_paths,
                    &mut directory_records,
                    &mut directory_children,
                    &mut directory_files,
                    &mut directory_mime_counts,
                    &next_directory_id,
                    *id,
                    file,
                )?;
                Self::add_file_indexes(
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
            let v = meta
                .get("schema_version")?
                .is_none_or(|version| version.value() != SCHEMA_VERSION);
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
            {
                let mut table = txn.open_table(PLAYLIST_ENTRIES)?;
                let keys = table
                    .iter()?
                    .filter_map(|entry| entry.ok().map(|(key, _)| key.value()))
                    .collect::<Vec<_>>();
                for key in keys {
                    table.remove(key)?;
                }
            }
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
        }
        {
            let mut metadata = txn.open_table(METADATA_TABLE)?;
            metadata.insert("schema_version", SCHEMA_VERSION)?;
            metadata.insert("codec_version", CODEC_VERSION)?;
        }
        {
            let live = winners.values().map(|(id, _)| *id).collect::<HashSet<_>>();
            let mut entries = txn.open_table(PLAYLIST_ENTRIES)?;
            let mut reverse = txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
            let reverse_keys = reverse
                .iter()?
                .filter_map(|entry| entry.ok().map(|(key, _)| key.value()))
                .collect::<Vec<_>>();
            for key in reverse_keys {
                reverse.remove_all(key)?;
            }
            let snapshot = entries
                .iter()?
                .filter_map(|e| e.ok().map(|(k, v)| (k.value(), v.value())))
                .collect::<Vec<_>>();
            for (key, old) in snapshot {
                let id = remap.get(&old).copied().unwrap_or(old);
                if !live.contains(&id) {
                    entries.remove(key)?;
                } else if id != old {
                    entries.insert(key, id)?;
                }
                if live.contains(&id) {
                    reverse.insert(id, key)?;
                }
            }
        }
        {
            let sources = txn.open_table(PLAYLIST_SOURCES)?;
            let mut reverse = txn.open_multimap_table(SOURCE_PLAYLISTS)?;
            let keys = reverse
                .iter()?
                .filter_map(|entry| entry.ok().map(|(key, _)| key.value().to_owned()))
                .collect::<Vec<_>>();
            for key in keys {
                reverse.remove_all(key.as_str())?;
            }
            for entry in sources.iter()? {
                let (playlist_id, source) = entry?;
                reverse.insert(source.value(), playlist_id.value())?;
            }
        }
        txn.commit()?;
        let total_files = winners.len() as u64;
        let total_size = winners.values().map(|(_, file)| file.size).sum();
        let health = DatabaseHealth {
            is_healthy: true,
            corruption_detected: !remap.is_empty(),
            integrity_check_passed: true,
            issues: Vec::new(),
            repair_attempted: true,
            repair_successful: true,
        };
        Ok((health, total_files, total_size))
        }).await?;
        self.total_files.store(total_files, Ordering::SeqCst);
        self.total_size.store(total_size, Ordering::SeqCst);
        Ok(health)
    }

    async fn remove_from_playlist(&self, playlist_id: i64, media_file_id: i64) -> Result<bool> {
        self.execute_write(move |database| {
        let write_txn = database.begin_write()?;
        let removed = {
            let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
            let mut reverse = write_txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
            let key_to_remove = playlist_entries
                .range(Self::playlist_entry_range(playlist_id))?
                .filter_map(|entry| entry.ok())
                .find(|(_, value)| value.value() == media_file_id)
                .map(|(key, _)| key.value());

            if let Some(key) = key_to_remove {
                playlist_entries.remove(key)?;
                reverse.remove(media_file_id, key)?;
                true
            } else {
                false
            }
        };
        write_txn.commit()?;

        Ok(removed)
        }).await
    }

    async fn get_playlist_tracks(&self, playlist_id: i64) -> Result<Vec<MediaFile>> {
        self.execute_read(move |database| {
        let read_txn = database.begin_read()?;
        let playlist_entries = read_txn.open_table(PLAYLIST_ENTRIES)?;
        let files_table = read_txn.open_table(FILES_TABLE)?;

        let mut files = Vec::new();
        for entry in playlist_entries.range(Self::playlist_entry_range(playlist_id))? {
            let (_, file_id) = entry?;
            let file_id = file_id.value();
            if let Some(data) = files_table.get(file_id)? {
                files.push(Self::deserialize_media_file(data.value())?);
            }
        }

        Ok(files)
        }).await
    }

    async fn reorder_playlist(
        &self,
        playlist_id: i64,
        track_positions: &[(i64, u32)],
    ) -> Result<()> {
        let track_positions = track_positions.to_vec();
        self.execute_write(move |database| {
        let write_txn = database.begin_write()?;
        {
            let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
            let mut reverse_entries = write_txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;

            // Remove existing entries for this playlist
            let entries = playlist_entries
                .range(Self::playlist_entry_range(playlist_id))?
                .filter_map(|entry| entry.ok().map(|(key, file)| (key.value(), file.value())))
                .collect::<Vec<_>>();
            for (key, file_id) in entries {
                playlist_entries.remove(key)?;
                reverse_entries.remove(file_id, key)?;
            }

            // Insert new order
            for (file_id, position) in track_positions {
                let key = Self::playlist_entry_key(playlist_id, position);
                playlist_entries.insert(key, file_id)?;
                reverse_entries.insert(file_id, key)?;
            }
        }
        write_txn.commit()?;

        Ok(())
        }).await
    }

    async fn get_files_with_path_prefix(&self, canonical_prefix: &str) -> Result<Vec<MediaFile>> {
        let mut files = Vec::new();
        let canonical = Self::canonical_path(Path::new(canonical_prefix))?;
        let prefix = canonical
            .to_string_lossy()
            .trim_end_matches('/')
            .to_string();
        let child = format!("{prefix}/");

        self.execute_read(move |database| {
        let read_txn = database.begin_read()?;
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
        }).await
    }

    async fn get_direct_subdirectories(
        &self,
        canonical_parent_path: &str,
    ) -> Result<Vec<MediaDirectory>> {
        let canonical = Self::canonical_path(Path::new(canonical_parent_path))?;
        let canonical_parent_path = canonical.to_string_lossy().to_string();

        self.execute_read(move |database| {
        let read_txn = database.begin_read()?;
        let paths = read_txn.open_table(DIRECTORY_PATH_INDEX)?;
        let records = read_txn.open_table(DIRECTORY_RECORDS)?;
        let children = read_txn.open_multimap_table(DIRECTORY_CHILDREN)?;
        let counts = read_txn.open_table(DIRECTORY_MIME_COUNTS)?;
        let Some(parent_id) = paths
            .get(canonical_parent_path.as_str())?
            .map(|value| value.value())
        else {
            return Ok(Vec::new());
        };

        let mut result = Vec::new();
        for child in children.get(parent_id)? {
            let child_id = child?.value();
            let count_key = Self::mime_count_key(child_id, "*");
            if counts
                .get(count_key.as_str())?
                .is_none_or(|value| value.value() == 0)
            {
                continue;
            }
            if let Some(path) = records.get(child_id)? {
                let path = path.value().to_owned();
                let name = PathBuf::from(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                result.push(MediaDirectory {
                    path: PathBuf::from(&path),
                    name,
                });
            }
        }
        Ok(result)
        }).await
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

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
    DatabaseBackend, DatabaseHealth, DatabaseManager, DatabaseReadSession, DatabaseStats,
    DirectoryView, FileFingerprint, FileLocation, HealthRepository, IndexSnapshot, MediaDirectory,
    MediaFile, MediaFileQuery, MediaFileView, MediaRepository, MusicCategory, MusicCategoryType,
    Playlist, PlaylistRepository, PlaylistView, RemovalSummary, RootAvailability, StatsRepository,
    VisitSummary,
};

include!("schema.rs");

mod health;
mod media_repo;
mod playlist_repo;
mod root_repo;
mod stats;

/// RedbDatabase - ACID-compliant embedded database
pub struct RedbDatabase {
    db: Arc<std::sync::RwLock<Database>>,
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

#[async_trait]
impl DatabaseBackend for RedbDatabase {
    async fn open(path: PathBuf, cache_size_mb: usize) -> Result<Self> {
        Self::new_with_cache(path, cache_size_mb).await
    }

    fn backend_name() -> &'static str {
        "redb"
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
                macro_rules! open_schema_entry {
                    (table, $constant:ident, $key:ty, $value:ty, $name:literal, $role:ident) => {
                        let _ = write_txn.open_table($constant)?;
                    };
                    (multimap, $constant:ident, $key:ty, $value:ty, $name:literal, $role:ident) => {
                        let _ = write_txn.open_multimap_table($constant)?;
                    };
                }
                redb_schema!(open_schema_entry);
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
            for entry in files_table.iter()? {
                let (key, value) = entry?;
                max_file = max_file.max(key.value());
                total_files_c += 1;
                let file = Self::deserialize_media_file(value.value())
                    .with_context(|| format!("corrupt media record {}", key.value()))?;
                total_size_s += file.size;
            }

            let mut max_playlist: i64 = 0;
            for entry in playlists_table.iter()? {
                let (key, _) = entry?;
                max_playlist = max_playlist.max(key.value());
            }

            let mut max_directory = 0_u64;
            for entry in directories_table.iter()? {
                let (key, _) = entry?;
                max_directory = max_directory.max(key.value());
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
            db: Arc::new(std::sync::RwLock::new(db)),
            db_path: path,
            next_file_id: AtomicI64::new(max_file_id + 1),
            next_playlist_id: AtomicI64::new(max_playlist_id + 1),
            next_directory_id: Arc::new(AtomicU64::new(max_directory_id + 1)),
            total_files: AtomicU64::new(total_files_count),
            total_size: AtomicU64::new(total_size_sum),
            mutation_lock: tokio::sync::Mutex::new(()),
        })
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
        tokio::task::spawn_blocking(move || {
            let database = database
                .read()
                .map_err(|_| anyhow!("ReDB handle lock is poisoned"))?;
            operation(&database)
        })
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
        tokio::task::spawn_blocking(move || {
            let database = database
                .read()
                .map_err(|_| anyhow!("ReDB handle lock is poisoned"))?;
            operation(&database)
        })
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

    fn directory_name(path: &str) -> &str {
        path.rsplit(['/', '\\']).next().unwrap_or(path)
    }

    fn directory_order_key(parent_id: u64, path: &str, child_id: u64) -> String {
        format!(
            "{parent_id:016x}\0{}\0{child_id:016x}",
            Self::directory_name(path).to_lowercase()
        )
    }

    fn directory_order_range(parent_id: u64) -> (String, String) {
        (
            format!("{parent_id:016x}\0"),
            format!("{parent_id:016x}\u{1}"),
        )
    }

    fn ensure_directory(
        paths: &mut redb::Table<&str, u64>,
        records: &mut redb::Table<u64, &str>,
        children: &mut redb::MultimapTable<u64, u64>,
        ordered_children: &mut redb::Table<&str, u64>,
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
                ordered_children,
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
            let order_key = Self::directory_order_key(parent_id, path, id);
            ordered_children.insert(order_key.as_str(), id)?;
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
        Self::playlist_entry_key(playlist_id, 0)..=Self::playlist_entry_key(playlist_id, u32::MAX)
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

    #[allow(clippy::too_many_arguments)] // One atomic directory-index update spans these tables.
    fn add_directory_membership<V: MediaFileView>(
        paths: &mut redb::Table<&str, u64>,
        records: &mut redb::Table<u64, &str>,
        children: &mut redb::MultimapTable<u64, u64>,
        ordered_children: &mut redb::Table<&str, u64>,
        directory_files: &mut redb::MultimapTable<u64, i64>,
        counts: &mut redb::Table<&str, u64>,
        next_directory_id: &AtomicU64,
        file: &V,
    ) -> Result<()> {
        let file_id = file
            .id()
            .ok_or_else(|| anyhow!("cannot index directory membership without a file ID"))?;
        let directory_path = Self::get_dir_key_str(file.path());
        let directory_id = Self::ensure_directory(
            paths,
            records,
            children,
            ordered_children,
            next_directory_id,
            &directory_path,
        )?;
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

    #[allow(clippy::too_many_arguments)] // One atomic directory-index update spans these tables.
    fn remove_directory_membership<V: MediaFileView>(
        paths: &mut redb::Table<&str, u64>,
        records: &mut redb::Table<u64, &str>,
        children: &mut redb::MultimapTable<u64, u64>,
        ordered_children: &mut redb::Table<&str, u64>,
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
                    let order_key = Self::directory_order_key(parent_id, &path, id);
                    ordered_children.remove(order_key.as_str())?;
                }
            }
            paths.remove(path.as_str())?;
            records.remove(id)?;
            current_path = parent_path;
        }
        Ok(())
    }

    /// Remove every directory-index artifact in a canonical subtree. Normal
    /// file removal already prunes empty ancestors; this defensive pass also
    /// handles legacy/corrupt records that have no surviving file membership.
    fn prune_directory_subtree(
        transaction: &redb::WriteTransaction,
        canonical_root: &str,
    ) -> Result<usize> {
        let directories = {
            let paths = transaction.open_table(DIRECTORY_PATH_INDEX)?;
            paths
                .iter()?
                .filter_map(|entry| match entry {
                    Ok((path, id))
                        if Path::new(path.value()).starts_with(Path::new(canonical_root)) =>
                    {
                        Some(Ok((path.value().to_owned(), id.value())))
                    }
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect::<std::result::Result<Vec<_>, redb::StorageError>>()?
        };
        if directories.is_empty() {
            return Ok(0);
        }

        let directory_ids = directories
            .iter()
            .map(|(_, id)| *id)
            .collect::<HashSet<_>>();

        {
            let mut children = transaction.open_multimap_table(DIRECTORY_CHILDREN)?;
            let edges = children
                .iter()?
                .flat_map(|entry| match entry {
                    Ok((parent, values)) => values
                        .map(|child| child.map(|child| (parent.value(), child.value())))
                        .collect::<Vec<_>>(),
                    Err(error) => vec![Err(error)],
                })
                .collect::<std::result::Result<Vec<_>, redb::StorageError>>()?;
            for (parent, child) in edges {
                if directory_ids.contains(&parent) || directory_ids.contains(&child) {
                    children.remove(parent, child)?;
                }
            }
        }
        {
            let mut ordered = transaction.open_table(DIRECTORY_CHILDREN_BY_NAME)?;
            let keys = ordered
                .iter()?
                .filter_map(|entry| match entry {
                    Ok((key, child)) if directory_ids.contains(&child.value()) => {
                        Some(Ok(key.value().to_owned()))
                    }
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect::<std::result::Result<Vec<_>, redb::StorageError>>()?;
            for key in keys {
                ordered.remove(key.as_str())?;
            }
        }
        {
            let mut directory_files = transaction.open_multimap_table(DIRECTORY_FILES)?;
            for id in &directory_ids {
                let _ = directory_files.remove_all(*id)?;
            }
        }
        {
            let mut counts = transaction.open_table(DIRECTORY_MIME_COUNTS)?;
            let keys = counts
                .iter()?
                .filter_map(|entry| match entry {
                    Ok((key, _)) => key
                        .value()
                        .split_once(':')
                        .and_then(|(id, _)| id.parse::<u64>().ok())
                        .filter(|id| directory_ids.contains(id))
                        .map(|_| Ok(key.value().to_owned())),
                    Err(error) => Some(Err(error)),
                })
                .collect::<std::result::Result<Vec<_>, redb::StorageError>>()?;
            for key in keys {
                counts.remove(key.as_str())?;
            }
        }
        {
            let mut paths = transaction.open_table(DIRECTORY_PATH_INDEX)?;
            let mut records = transaction.open_table(DIRECTORY_RECORDS)?;
            for (path, id) in &directories {
                paths.remove(path.as_str())?;
                records.remove(*id)?;
            }
        }

        Ok(directories.len())
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
        files: &[(String, i64, IndexSnapshot)],
    ) -> Result<(usize, u64)> {
        let mut files_table = transaction.open_table(FILES_TABLE)?;
        let mut path_index = transaction.open_table(PATH_INDEX)?;
        let mut directory_paths = transaction.open_table(DIRECTORY_PATH_INDEX)?;
        let mut directory_records = transaction.open_table(DIRECTORY_RECORDS)?;
        let mut directory_children = transaction.open_multimap_table(DIRECTORY_CHILDREN)?;
        let mut ordered_children = transaction.open_table(DIRECTORY_CHILDREN_BY_NAME)?;
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
                &mut ordered_children,
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

mod traits;

#[cfg(test)]
mod tests;

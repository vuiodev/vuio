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

mod health;
mod media_repo;
mod playlist_repo;
mod root_repo;
mod stats;

include!("schema.rs");

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
                let _ = write_txn.open_table(FILES_TABLE)?;
                let _ = write_txn.open_table(PATH_INDEX)?;
                let _ = write_txn.open_table(DIRECTORY_PATH_INDEX)?;
                let _ = write_txn.open_table(DIRECTORY_RECORDS)?;
                let _ = write_txn.open_multimap_table(DIRECTORY_CHILDREN)?;
                let _ = write_txn.open_table(DIRECTORY_CHILDREN_BY_NAME)?;
                let _ = write_txn.open_multimap_table(DIRECTORY_FILES)?;
                let _ = write_txn.open_table(DIRECTORY_MIME_COUNTS)?;
                let _ = write_txn.open_table(PLAYLISTS_TABLE)?;
                let _ = write_txn.open_table(PLAYLIST_ENTRIES)?;
                let _ = write_txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
                let _ = write_txn.open_table(PLAYLIST_SOURCES)?;
                let _ = write_txn.open_multimap_table(SOURCE_PLAYLISTS)?;
                let _ = write_txn.open_table(METADATA_TABLE)?;
                let _ = write_txn.open_table(ROOT_AVAILABILITY)?;
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

#[async_trait]
impl MediaRepository for RedbDatabase {
    type ReadSession = RedbReadSession;

    async fn read<R, F>(self: Arc<Self>, operation: F) -> Result<R>
    where
        R: Send + 'static,
        F: FnOnce(&mut Self::ReadSession) -> Result<R> + Send + 'static,
    {
        RedbDatabase::read_impl(self, operation).await
    }

    async fn store_media_file(&self, file: &MediaFile) -> Result<i64> {
        RedbDatabase::store_media_file_impl(self, file).await
    }

    fn stream_all_media_files(
        &self,
    ) -> Pin<Box<dyn futures_util::Stream<Item = Result<MediaFile, DatabaseError>> + Send + '_>>
    {
        RedbDatabase::stream_all_media_files_impl(self)
    }

    async fn remove_media_file(&self, path: &Path) -> Result<bool> {
        RedbDatabase::remove_media_file_impl(self, path).await
    }

    async fn update_media_file(&self, file: &MediaFile) -> Result<()> {
        RedbDatabase::update_media_file_impl(self, file).await
    }

    async fn get_files_in_directory(&self, dir: &Path) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_files_in_directory_impl(self, dir).await
    }

    async fn get_directory_listing(
        &self,
        parent_path: &Path,
        media_type_filter: &str,
    ) -> Result<(Vec<MediaDirectory>, Vec<MediaFile>)> {
        RedbDatabase::get_directory_listing_impl(self, parent_path, media_type_filter).await
    }

    async fn cleanup_missing_files(&self, existing_paths: &[PathBuf]) -> Result<usize> {
        RedbDatabase::cleanup_missing_files_impl(self, existing_paths).await
    }

    async fn get_file_by_path(&self, path: &Path) -> Result<Option<MediaFile>> {
        RedbDatabase::get_file_by_path_impl(self, path).await
    }

    async fn get_file_by_id(&self, id: i64) -> Result<Option<MediaFile>> {
        RedbDatabase::get_file_by_id_impl(self, id).await
    }

    async fn get_file_location_by_id(&self, id: i64) -> Result<Option<FileLocation>> {
        RedbDatabase::get_file_location_by_id_impl(self, id).await
    }

    async fn load_file_fingerprints(&self) -> Result<Vec<FileFingerprint>> {
        RedbDatabase::load_file_fingerprints_impl(self).await
    }

    async fn get_root_availability(&self, path: &Path) -> Result<Option<RootAvailability>> {
        RedbDatabase::get_root_availability_impl(self, path).await
    }

    async fn list_root_availability(&self) -> Result<Vec<RootAvailability>> {
        RedbDatabase::list_root_availability_impl(self).await
    }

    async fn set_root_availability(&self, state: &RootAvailability) -> Result<()> {
        RedbDatabase::set_root_availability_impl(self, state).await
    }

    async fn remove_root_availability(&self, path: &Path) -> Result<()> {
        RedbDatabase::remove_root_availability_impl(self, path).await
    }

    async fn get_artists(&self) -> Result<Vec<MusicCategory>> {
        RedbDatabase::get_artists_impl(self).await
    }

    async fn get_albums(&self, artist_filter: Option<&str>) -> Result<Vec<MusicCategory>> {
        RedbDatabase::get_albums_impl(self, artist_filter).await
    }

    async fn get_genres(&self) -> Result<Vec<MusicCategory>> {
        RedbDatabase::get_genres_impl(self).await
    }

    async fn get_years(&self) -> Result<Vec<MusicCategory>> {
        RedbDatabase::get_years_impl(self).await
    }

    async fn get_album_artists(&self) -> Result<Vec<MusicCategory>> {
        RedbDatabase::get_album_artists_impl(self).await
    }

    async fn get_music_by_artist(&self, artist: &str) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_music_by_artist_impl(self, artist).await
    }

    async fn get_music_by_album(
        &self,
        album: &str,
        artist: Option<&str>,
    ) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_music_by_album_impl(self, album, artist).await
    }

    async fn get_music_by_genre(&self, genre: &str) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_music_by_genre_impl(self, genre).await
    }

    async fn get_music_by_year(&self, year: u32) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_music_by_year_impl(self, year).await
    }

    async fn get_music_by_album_artist(&self, album_artist: &str) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_music_by_album_artist_impl(self, album_artist).await
    }

    async fn get_files_by_paths(&self, paths: &[PathBuf]) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_files_by_paths_impl(self, paths).await
    }

    async fn bulk_store_media_files(&self, files: &[MediaFile]) -> Result<Vec<i64>> {
        RedbDatabase::bulk_store_media_files_impl(self, files).await
    }

    async fn bulk_store_canonical_media_files(&self, files: &[MediaFile]) -> Result<Vec<i64>> {
        RedbDatabase::bulk_store_canonical_media_files_impl(self, files).await
    }

    async fn bulk_update_media_files(&self, files: &[MediaFile]) -> Result<()> {
        RedbDatabase::bulk_update_media_files_impl(self, files).await
    }

    async fn bulk_update_canonical_media_files(&self, files: &[MediaFile]) -> Result<()> {
        RedbDatabase::bulk_update_canonical_media_files_impl(self, files).await
    }

    async fn bulk_remove_media_files(&self, paths: &[PathBuf]) -> Result<usize> {
        RedbDatabase::bulk_remove_media_files_impl(self, paths).await
    }

    async fn remove_media_under_path(&self, path: &Path) -> Result<RemovalSummary> {
        RedbDatabase::remove_media_under_path_impl(self, path).await
    }

    async fn get_files_with_path_prefix(&self, canonical_prefix: &str) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_files_with_path_prefix_impl(self, canonical_prefix).await
    }

    async fn get_direct_subdirectories(
        &self,
        canonical_parent_path: &str,
    ) -> Result<Vec<MediaDirectory>> {
        RedbDatabase::get_direct_subdirectories_impl(self, canonical_parent_path).await
    }

    async fn batch_cleanup_missing_files(
        &self,
        existing_canonical_paths: &HashSet<String>,
    ) -> Result<usize> {
        RedbDatabase::batch_cleanup_missing_files_impl(self, existing_canonical_paths).await
    }

    async fn database_native_cleanup(&self, existing_canonical_paths: &[String]) -> Result<usize> {
        RedbDatabase::database_native_cleanup_impl(self, existing_canonical_paths).await
    }

    async fn get_filtered_direct_subdirectories(
        &self,
        canonical_parent_path: &str,
        mime_filter: &str,
    ) -> Result<Vec<MediaDirectory>> {
        RedbDatabase::get_filtered_direct_subdirectories_impl(
            self,
            canonical_parent_path,
            mime_filter,
        )
        .await
    }
}

#[async_trait]
impl PlaylistRepository for RedbDatabase {
    async fn create_playlist(&self, name: &str, description: Option<&str>) -> Result<i64> {
        RedbDatabase::create_playlist_impl(self, name, description).await
    }

    async fn get_playlists(&self) -> Result<Vec<Playlist>> {
        RedbDatabase::get_playlists_impl(self).await
    }

    async fn get_playlist(&self, playlist_id: i64) -> Result<Option<Playlist>> {
        RedbDatabase::get_playlist_impl(self, playlist_id).await
    }

    async fn update_playlist(&self, playlist: &Playlist) -> Result<()> {
        RedbDatabase::update_playlist_impl(self, playlist).await
    }

    async fn delete_playlist(&self, playlist_id: i64) -> Result<bool> {
        RedbDatabase::delete_playlist_impl(self, playlist_id).await
    }

    async fn set_playlist_source(&self, playlist_id: i64, source_path: &Path) -> Result<()> {
        RedbDatabase::set_playlist_source_impl(self, playlist_id, source_path).await
    }

    async fn replace_playlist_from_source(
        &self,
        source_path: &Path,
        name: &str,
        media_file_ids: &[(i64, u32)],
    ) -> Result<i64> {
        RedbDatabase::replace_playlist_from_source_impl(self, source_path, name, media_file_ids)
            .await
    }

    async fn remove_derived_content_by_source(&self, source_path: &Path) -> Result<usize> {
        RedbDatabase::remove_derived_content_by_source_impl(self, source_path).await
    }

    async fn add_to_playlist(
        &self,
        playlist_id: i64,
        media_file_id: i64,
        position: Option<u32>,
    ) -> Result<i64> {
        RedbDatabase::add_to_playlist_impl(self, playlist_id, media_file_id, position).await
    }

    async fn batch_add_to_playlist(
        &self,
        playlist_id: i64,
        media_file_ids: &[(i64, u32)],
    ) -> Result<Vec<i64>> {
        RedbDatabase::batch_add_to_playlist_impl(self, playlist_id, media_file_ids).await
    }

    async fn remove_from_playlist(&self, playlist_id: i64, media_file_id: i64) -> Result<bool> {
        RedbDatabase::remove_from_playlist_impl(self, playlist_id, media_file_id).await
    }

    async fn get_playlist_tracks(&self, playlist_id: i64) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_playlist_tracks_impl(self, playlist_id).await
    }

    async fn reorder_playlist(
        &self,
        playlist_id: i64,
        track_positions: &[(i64, u32)],
    ) -> Result<()> {
        RedbDatabase::reorder_playlist_impl(self, playlist_id, track_positions).await
    }
}

#[async_trait]
impl HealthRepository for RedbDatabase {
    async fn check_and_repair(&self) -> Result<DatabaseHealth> {
        RedbDatabase::check_and_repair_impl(self).await
    }

    async fn create_backup(&self, backup_path: &Path) -> Result<()> {
        RedbDatabase::create_backup_impl(self, backup_path).await
    }

    async fn vacuum(&self) -> Result<bool> {
        RedbDatabase::vacuum_impl(self).await
    }

    async fn rebuild_derived_indexes(&self) -> Result<DatabaseHealth> {
        RedbDatabase::rebuild_derived_indexes_impl(self).await
    }
}

#[async_trait]
impl StatsRepository for RedbDatabase {
    async fn get_stats(&self) -> Result<DatabaseStats> {
        RedbDatabase::get_stats_impl(self).await
    }
}

#[async_trait]
impl DatabaseManager for RedbDatabase {
    async fn initialize(&self) -> Result<()> {
        info!("RedbDatabase initialized");
        Ok(())
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
    async fn opening_incompatible_schema_preserves_database_contents() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("old-schema.redb");
        {
            let raw = Database::create(&path).unwrap();
            let write = raw.begin_write().unwrap();
            {
                let mut files = write.open_table(FILES_TABLE).unwrap();
                files.insert(42, &[1_u8, 2, 3][..]).unwrap();
                let mut metadata = write.open_table(METADATA_TABLE).unwrap();
                metadata
                    .insert("schema_version", SCHEMA_VERSION - 1)
                    .unwrap();
            }
            write.commit().unwrap();
        }

        assert!(RedbDatabase::new(path.clone()).await.is_err());
        assert!(path.exists());

        let raw = Database::open(&path).unwrap();
        let read = raw.begin_read().unwrap();
        let files = read.open_table(FILES_TABLE).unwrap();
        assert_eq!(files.get(42).unwrap().unwrap().value(), &[1_u8, 2, 3]);
        let metadata = read.open_table(METADATA_TABLE).unwrap();
        assert_eq!(
            metadata.get("schema_version").unwrap().unwrap().value(),
            SCHEMA_VERSION - 1
        );
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

        {
            let database = db.db.read().unwrap();
            let transaction = database.begin_write().unwrap();
            transaction
                .open_table(FILES_TABLE)
                .unwrap()
                .remove(id)
                .unwrap();
            transaction.commit().unwrap();
        }

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

    #[tokio::test]
    async fn backup_and_offline_restore_preserve_a_valid_snapshot() {
        let temp = tempdir().unwrap();
        let database_path = temp.path().join("active.redb");
        let backup_path = temp.path().join("snapshot.redb");
        let db = RedbDatabase::new(database_path.clone()).await.unwrap();
        db.initialize().await.unwrap();
        db.rebuild_derived_indexes().await.unwrap();
        db.store_media_file(&MediaFile::new(
            PathBuf::from("/media/original.mp4"),
            42,
            "video/mp4".to_owned(),
        ))
        .await
        .unwrap();
        db.create_backup(&backup_path).await.unwrap();
        drop(db);

        let replacement = RedbDatabase::new(database_path.clone()).await.unwrap();
        replacement.initialize().await.unwrap();
        replacement
            .store_media_file(&MediaFile::new(
                PathBuf::from("/media/replacement.mp4"),
                7,
                "video/mp4".to_owned(),
            ))
            .await
            .unwrap();
        drop(replacement);

        RedbDatabase::restore_backup_file(backup_path, database_path.clone())
            .await
            .unwrap();
        let restored = RedbDatabase::new(database_path).await.unwrap();
        restored.initialize().await.unwrap();
        assert!(restored
            .get_file_by_path(Path::new("/media/original.mp4"))
            .await
            .unwrap()
            .is_some());
        assert!(restored
            .get_file_by_path(Path::new("/media/replacement.mp4"))
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn direct_directory_visitor_orders_and_pages_before_loading_records() {
        let temp = tempdir().unwrap();
        let db = Arc::new(
            RedbDatabase::new(temp.path().join("ordered.redb"))
                .await
                .unwrap(),
        );
        db.initialize().await.unwrap();
        db.bulk_store_media_files(&[
            MediaFile::new(
                PathBuf::from("/media/Zeta/z.mp4"),
                1,
                "video/mp4".to_owned(),
            ),
            MediaFile::new(
                PathBuf::from("/media/alpha/a.mp4"),
                1,
                "video/mp4".to_owned(),
            ),
        ])
        .await
        .unwrap();

        let (summary, names) = db
            .clone()
            .read(|session| {
                let mut names = Vec::new();
                let summary = session.visit_direct_subdirectories(
                    "/media",
                    Some("video/"),
                    0,
                    1,
                    |directory| {
                        names.push(directory.name().to_owned());
                        Ok(())
                    },
                )?;
                Ok((summary, names))
            })
            .await
            .unwrap();
        assert_eq!(summary.matched, 2);
        assert_eq!(summary.visited, 1);
        assert_eq!(names, ["alpha"]);
    }
}

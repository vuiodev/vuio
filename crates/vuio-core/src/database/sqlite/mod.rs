//! SQLite storage backend.
//!
//! One writer and a small pool of readers, all driven from Tokio's blocking
//! pool. Write-ahead logging lets the readers run while the writer commits, so
//! a library scan does not stall browsing.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

use super::{
    DatabaseBackend, DatabaseHealth, DatabaseManager, DatabaseSettings, DatabaseStats,
    FileFingerprint, FileLocation, HealthRepository, MediaDirectory, MediaFile, MediaFileQuery,
    MediaInfoRecord, MediaInfoRepository, MediaInfoStats, MediaRepository, MusicCategory, Playlist,
    PlaylistRepository, RemovalSummary, RootAvailability, SecretStore, StatsRepository,
};

mod directory;
mod health;
mod media_repo;
mod mediainfo_repo;
mod playlist_repo;
mod query;
mod root_repo;
mod schema;
mod secret_repo;
mod session;
mod station_repo;
mod stats;
mod traits;

#[cfg(test)]
mod tests;

pub use session::SqliteReadSession;

/// Register the natural-order collation on a caller-supplied connection.
///
/// Two of the browse indexes are declared `COLLATE natural_order`, so any
/// connection that writes `media_files` has to know it — including one opened
/// outside this crate. Exposed for the benchmark generator, which bulk-loads a
/// library over a direct handle; without this it would need its own copy of
/// `natural_cmp`, and a copy is a thing that drifts.
#[cfg(feature = "unstable-internals")]
pub fn register_collations(connection: &rusqlite::Connection) -> anyhow::Result<()> {
    schema::register_collations(connection)
}

/// Readers held open for reuse.
///
/// Reads run on Tokio's blocking pool, which is unbounded by design; without a
/// bound of our own a burst of browse requests would open a SQLite connection
/// per blocked thread. The semaphore caps concurrency, and the stack keeps the
/// connections (and their compiled statements) warm between requests.
struct ReaderPool {
    idle: Mutex<Vec<Connection>>,
    permits: Arc<tokio::sync::Semaphore>,
    path: PathBuf,
    cache_mb: usize,
}

impl ReaderPool {
    fn new(path: PathBuf, cache_mb: usize, size: usize) -> Self {
        Self {
            idle: Mutex::new(Vec::with_capacity(size)),
            permits: Arc::new(tokio::sync::Semaphore::new(size)),
            path,
            cache_mb,
        }
    }

    fn checkout(
        self: &Arc<Self>,
        permit: tokio::sync::OwnedSemaphorePermit,
    ) -> Result<PooledConnection> {
        let reused = self
            .idle
            .lock()
            .map_err(|_| anyhow!("SQLite reader pool lock is poisoned"))?
            .pop();
        let connection = match reused {
            Some(connection) => connection,
            None => schema::open_connection(&self.path, self.cache_mb)?,
        };
        Ok(PooledConnection {
            pool: Arc::clone(self),
            connection: Some(connection),
            _permit: permit,
        })
    }
}

/// A reader borrowed from the pool, returned when it goes out of scope.
pub struct PooledConnection {
    pool: Arc<ReaderPool>,
    connection: Option<Connection>,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl std::ops::Deref for PooledConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        self.connection
            .as_ref()
            .expect("connection is taken only while dropping")
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            if let Ok(mut idle) = self.pool.idle.lock() {
                idle.push(connection);
            }
        }
    }
}

/// SQLite-backed media index.
pub struct SqliteDatabase {
    /// SQLite permits one writer at a time; this is that writer.
    write: Arc<Mutex<Connection>>,
    readers: Arc<ReaderPool>,
    db_path: PathBuf,
    /// Held for the duration of a write so writers queue in async code rather
    /// than piling up as blocked threads inside the blocking pool.
    mutation_lock: tokio::sync::Mutex<()>,
    /// Bumped by every write. Read-side caches compare against it to know
    /// whether what they hold can still be true.
    write_generation: std::sync::atomic::AtomicU64,
    /// Library totals, and the generation they were computed at.
    ///
    /// `get_stats` aggregates the whole of `media_files` — it needs both `size`
    /// and `mime_family`, which no index covers together — and it is asked for
    /// by `/metrics`, `/metrics/json`, `/readyz` and the dashboard's five-second
    /// poll. On a large library that is a multi-gigabyte scan several times a
    /// minute to produce an answer that only changes when something is written.
    stats_cache: Mutex<Option<(u64, DatabaseStats)>>,
}

impl std::fmt::Debug for SqliteDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteDatabase")
            .field("db_path", &self.db_path)
            .finish()
    }
}

/// Readers to keep open. Browsing is bursty and shallow, so a handful of
/// connections saturates the disk long before the CPU.
fn reader_pool_size() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(2)
        .clamp(2, 4)
}

#[async_trait]
impl DatabaseBackend for SqliteDatabase {
    async fn open(settings: &DatabaseSettings) -> Result<Self> {
        Self::open_at(settings.path.clone(), settings.cache_mb).await
    }

    fn backend_name() -> &'static str {
        "sqlite"
    }

    fn file_extension() -> &'static str {
        "db"
    }

    fn sidecar_extensions() -> &'static [&'static str] {
        // Write-ahead logging keeps committed data outside the main file until
        // a checkpoint, so neither sidecar may be separated from it.
        &["db-wal", "db-shm"]
    }

    async fn restore_backup_file(backup: &Path, destination: &Path) -> Result<()> {
        Self::install_backup(backup.to_path_buf(), destination.to_path_buf()).await
    }
}

impl SqliteDatabase {
    /// Open (creating if absent) the database at `path`.
    pub async fn open_at(path: PathBuf, cache_mb: usize) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await.with_context(|| {
                    format!("Failed to create database directory {}", parent.display())
                })?;
            }
        }

        let open_path = path.clone();
        let write = tokio::task::spawn_blocking(move || -> Result<Connection> {
            let connection = schema::open_connection(&open_path, cache_mb)?;
            // Reject an unusable file here rather than at the first query, so
            // startup can quarantine it before anything else touches it.
            let _: i64 = connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .with_context(|| {
                    format!("{} is not a usable SQLite database", open_path.display())
                })?;
            Ok(connection)
        })
        .await
        .context("SQLite open task failed")??;

        info!("Opened SQLite database at {}", path.display());
        Ok(Self {
            write: Arc::new(Mutex::new(write)),
            readers: Arc::new(ReaderPool::new(
                path.clone(),
                cache_mb,
                reader_pool_size(),
            )),
            db_path: path,
            mutation_lock: tokio::sync::Mutex::new(()),
            write_generation: std::sync::atomic::AtomicU64::new(0),
            stats_cache: Mutex::new(None),
        })
    }

    /// Convenience constructor used by tests and embedders.
    pub async fn new(path: PathBuf) -> Result<Self> {
        Self::open_at(path, 128).await
    }

    /// Run a read on the blocking pool against a pooled connection.
    pub(super) async fn execute_read<T, F>(&self, operation: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T> + Send + 'static,
    {
        let permit = Arc::clone(&self.readers.permits)
            .acquire_owned()
            .await
            .context("SQLite reader pool is closed")?;
        let readers = Arc::clone(&self.readers);
        tokio::task::spawn_blocking(move || {
            let connection = readers.checkout(permit)?;
            operation(&connection)
        })
        .await
        .context("SQLite read task failed")?
    }

    /// Run a write on the blocking pool, serialized against other writes.
    pub(super) async fn execute_write<T, F>(&self, operation: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    {
        let _guard = self.mutation_lock.lock().await;
        // Before the write, not after: a reader that samples the generation
        // mid-write must not be able to cache a result taken from the old state
        // under the new number.
        self.write_generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let write = Arc::clone(&self.write);
        tokio::task::spawn_blocking(move || {
            let mut connection = write
                .lock()
                .map_err(|_| anyhow!("SQLite write connection lock is poisoned"))?;
            operation(&mut connection)
        })
        .await
        .context("SQLite write task failed")?
    }

    /// Run a write inside a transaction that rolls back on any error.
    pub(super) async fn transact<T, F>(&self, operation: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&rusqlite::Transaction<'_>) -> Result<T> + Send + 'static,
    {
        self.execute_write(move |connection| {
            let transaction = connection.transaction()?;
            let value = operation(&transaction)?;
            transaction.commit()?;
            Ok(value)
        })
        .await
    }

    pub(super) async fn read_impl<R, F>(self: Arc<Self>, operation: F) -> Result<R>
    where
        R: Send + 'static,
        F: FnOnce(&mut SqliteReadSession) -> Result<R> + Send + 'static,
    {
        let permit = Arc::clone(&self.readers.permits)
            .acquire_owned()
            .await
            .context("SQLite reader pool is closed")?;
        let readers = Arc::clone(&self.readers);
        tokio::task::spawn_blocking(move || {
            let connection = readers.checkout(permit)?;
            let mut session = SqliteReadSession::begin(connection)?;
            operation(&mut session)
        })
        .await
        .context("SQLite read task failed")?
    }

    // ── Path handling ──────────────────────────────────────────────────────

    /// Normalize a path the way every stored record has been normalized.
    ///
    /// Network sources are stored verbatim: a radio stream URL has no
    /// filesystem to be resolved against.
    pub(super) fn canonical_path(path: &Path) -> Result<PathBuf> {
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

    pub(super) fn canonical_string(path: &Path) -> Result<String> {
        Ok(Self::canonical_path(path)?.to_string_lossy().into_owned())
    }

    fn canonical_file(file: &MediaFile) -> Result<MediaFile> {
        let mut file = file.clone();
        file.path = Self::canonical_path(&file.path)?;
        Ok(file)
    }

    /// The family segment of a MIME type: the part before the slash.
    ///
    /// Kept as the raw prefix rather than bucketed into known types, so a
    /// filter for an unusual family still matches what it should.
    pub(super) fn mime_family(mime: &str) -> &str {
        mime.split_once('/').map(|(family, _)| family).unwrap_or(mime)
    }

    /// Directory holding `path`, in the same normalized form as stored paths.
    pub(super) fn parent_directory(path: &str) -> Option<String> {
        let parent = Path::new(path)
            .parent()?
            .to_string_lossy()
            .replace('\\', "/");
        if parent == path || parent.is_empty() {
            None
        } else if cfg!(target_os = "windows") {
            Some(parent.to_lowercase())
        } else {
            Some(parent)
        }
    }

    pub(super) fn directory_name(path: &str) -> &str {
        path.rsplit(['/', '\\']).next().unwrap_or(path)
    }

    /// Half-open key range covering everything beneath `directory`.
    ///
    /// Ending the range at the byte after `/` keeps `"/media/Films"` out of a
    /// scan of `"/media/Film"`, which a plain `LIKE 'prefix%'` would not.
    pub(super) fn subtree_range(directory: &str) -> (String, String) {
        let trimmed = directory.trim_end_matches('/');
        (format!("{trimmed}/"), format!("{trimmed}0"))
    }
}

#[async_trait]
impl DatabaseManager for SqliteDatabase {
    async fn initialize(&self) -> Result<()> {
        self.execute_write(|connection| schema::initialize_schema(connection))
            .await?;
        debug!("SQLite schema initialized");
        Ok(())
    }
}

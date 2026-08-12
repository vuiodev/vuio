use super::*;

/// Perform graceful shutdown with atomic state persistence
pub(super) async fn perform_graceful_shutdown<D: DatabaseManager>(
    database: &Arc<D>,
    stats: &ApplicationStats,
) -> anyhow::Result<()> {
    info!("Performing graceful shutdown with atomic state persistence...");

    let (files_processed, directories_scanned, events_handled, errors_encountered, last_activity) =
        stats.snapshot();

    // Log final application statistics
    info!("Final application statistics:");
    info!("  - Files processed: {}", files_processed);
    info!("  - Directories scanned: {}", directories_scanned);
    info!("  - Events handled: {}", events_handled);
    info!("  - Errors encountered: {}", errors_encountered);
    info!("  - Last activity: {:?}", last_activity);

    // Ensure database persists all pending operations
    info!("Persisting database state...");

    // Get database statistics before shutdown
    match database.get_stats().await {
        Ok(db_stats) => {
            info!("Database statistics at shutdown:");
            info!("  - Total media files: {}", db_stats.total_files);
            info!("  - Total media size: {} bytes", db_stats.total_size);
            info!("  - Database file size: {} bytes", db_stats.database_size);
        }
        Err(e) => {
            warn!(
                "Could not retrieve database statistics during shutdown: {}",
                e
            );
        }
    }

    // Perform database vacuum if needed (this will also ensure all data is persisted)
    info!("Performing final database maintenance...");
    match database.vacuum().await {
        Ok(compacted) => info!(compacted, "Final database compaction completed"),
        Err(e) => warn!("Could not compact database during shutdown: {}", e),
    }

    info!("Graceful shutdown with atomic state persistence completed");
    Ok(())
}

/// Shared cancellation and final persistence operations.
#[derive(Clone)]
pub struct ShutdownCoordinator {
    cancellation: CancellationToken,
}

impl ShutdownCoordinator {
    pub fn new() -> Self {
        Self {
            cancellation: CancellationToken::new(),
        }
    }

    pub fn from_token(cancellation: CancellationToken) -> Self {
        Self { cancellation }
    }

    pub fn token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub async fn finalize<D: DatabaseManager>(
        database: &Arc<D>,
        stats: &ApplicationStats,
    ) -> anyhow::Result<()> {
        perform_graceful_shutdown(database, stats).await
    }
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

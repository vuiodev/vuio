/// Wait for shutdown signals (Ctrl+C, SIGTERM, etc.)
/// Supports graceful shutdown on first signal, force quit on second signal
async fn wait_for_shutdown_signal() -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut sigterm = signal(SignalKind::terminate())
            .context("Failed to register SIGTERM handler")?;
        let mut sigint = signal(SignalKind::interrupt())
            .context("Failed to register SIGINT handler")?;

        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM signal");
            }
            _ = sigint.recv() => {
                info!("Received SIGINT signal (Ctrl+C)");

                // Set up a second signal handler for force quit
                tokio::spawn(async move {
                    if sigint.recv().await.is_some() {
                        warn!("Received second SIGINT signal - forcing immediate exit");
                        std::process::exit(1);
                    }
                });
            }
        }
    }

    #[cfg(not(unix))]
    {
        // On Windows, only Ctrl+C is available
        tokio::signal::ctrl_c()
            .await
            .context("Failed to listen for Ctrl+C signal")?;
        info!("Received Ctrl+C signal");

        // Set up a second signal handler for force quit
        tokio::spawn(async {
            if tokio::signal::ctrl_c().await.is_ok() {
                warn!("Received second Ctrl+C signal - forcing immediate exit");
                std::process::exit(1);
            }
        });
    }

    Ok(())
}

/// Perform graceful shutdown with ReDB atomic state persistence
async fn perform_graceful_shutdown<D: DatabaseManager>(
    database: &Arc<D>,
    stats: &ApplicationStats,
    config: &crate::config::AppConfig,
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

    // Ensure ReDB database persists all pending operations
    info!("Persisting ReDB database state...");

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

    // Perform database vacuum if enabled
    if config.database.compact_on_shutdown {
        info!("Performing final database maintenance...");
        match database.vacuum().await {
            Ok(compacted) => info!(compacted, "Final database compaction completed"),
            Err(e) => warn!("Could not compact database during shutdown: {}", e),
        }
    } else {
        info!("Skipping database compaction on shutdown (compact_on_shutdown = false)");
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

    pub fn token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub async fn wait_for_signal(&self) -> anyhow::Result<()> {
        wait_for_shutdown_signal().await
    }

    pub async fn finalize<D: DatabaseManager>(
        database: &Arc<D>,
        stats: &ApplicationStats,
        config: &crate::config::AppConfig,
    ) -> anyhow::Result<()> {
        perform_graceful_shutdown(database, stats, config).await
    }
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

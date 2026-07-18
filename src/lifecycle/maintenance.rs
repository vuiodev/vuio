/// Start atomic application statistics monitoring
async fn start_atomic_monitoring<D: DatabaseManager + 'static>(
    database: Arc<D>,
    stats: Arc<ApplicationStats>,
    cancellation: CancellationToken,
) -> anyhow::Result<tokio::task::JoinHandle<()>> {
    info!("Starting atomic application statistics monitoring...");

    let handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60)); // Monitor every minute

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let (files_processed, directories_scanned, events_handled, errors_encountered, last_activity) = stats.snapshot();

                    // Log periodic statistics
                    debug!("Atomic application statistics:");
                    debug!("  - Files processed: {}", files_processed);
                    debug!("  - Directories scanned: {}", directories_scanned);
                    debug!("  - Events handled: {}", events_handled);
                    debug!("  - Errors encountered: {}", errors_encountered);
                    debug!("  - Last activity: {:?}", last_activity);

                    // Get database statistics
                    if let Ok(db_stats) = database.get_stats().await {
                        debug!("ReDB database statistics:");
                        debug!("  - Total media files: {}", db_stats.total_files);
                        debug!("  - Total media size: {} bytes", db_stats.total_size);
                        debug!("  - Database file size: {} bytes", db_stats.database_size);
                    }

                    // Check for inactivity (no events in last 5 minutes)
                    if let Ok(elapsed) = last_activity.elapsed() {
                        if elapsed > std::time::Duration::from_secs(300) {
                            debug!("Application has been inactive for {:?}", elapsed);
                        }
                    }
                }
                _ = cancellation.cancelled() => {
                    info!("Atomic monitoring service received cancellation");
                    break;
                }
            }
        }

        info!("Atomic monitoring service stopped");
    });

    info!("Atomic application statistics monitoring started");
    Ok(handle)
}

/// Periodic application and database statistics monitoring.
pub struct MaintenanceService;

impl MaintenanceService {
    pub async fn start<D: DatabaseManager + 'static>(
        database: Arc<D>,
        stats: Arc<ApplicationStats>,
        cancellation: CancellationToken,
    ) -> anyhow::Result<tokio::task::JoinHandle<()>> {
        start_atomic_monitoring(database, stats, cancellation).await
    }
}

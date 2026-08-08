use super::super::*;

/// Media scanning, filesystem-event, and reconciliation lifecycle operations.
pub struct MediaLifecycleService;

impl MediaLifecycleService {
    pub async fn initial_scan<D: DatabaseManager + 'static>(
        config: &AppConfig,
        database: &Arc<D>,
    ) -> anyhow::Result<()> {
        perform_initial_media_scan(config, database).await?;
        perform_initial_playlist_scan(config, database).await
    }

    pub async fn handle_event<D: DatabaseManager + 'static>(
        event: FileSystemEvent,
        state: &AppState<D>,
    ) -> anyhow::Result<()> {
        handle_file_system_event(event, state).await
    }

    pub async fn start_monitoring<D: DatabaseManager + 'static>(
        watcher: Arc<CrossPlatformWatcher>,
        state: AppState<D>,
        cancellation: CancellationToken,
    ) -> anyhow::Result<Option<tokio::task::JoinHandle<()>>> {
        start_file_monitoring(watcher, state, cancellation).await
    }
}

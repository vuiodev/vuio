use crate::{
    config::{
        AppConfig, ConfigChangeEvent, ConfigManager, MonitoredDirectoryConfig, ValidationMode,
    },
    database::{
        self, DatabaseManager, HealthRepository, MediaRepository, PlaylistRepository,
        StatsRepository,
    },
    logging, media,
    platform::{
        self,
        filesystem::{create_platform_filesystem_manager, create_platform_path_normalizer},
        PlatformInfo,
    },
    ssdp,
    state::AppState,
    watcher::{
        classify_media_rename, CrossPlatformWatcher, FileSystemEvent, FileSystemWatcher,
        MediaRenameKind,
    },
    web,
};
use anyhow::Context;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

include!("cli.rs");
include!("bootstrap.rs");
include!("media.rs");
include!("network.rs");
include!("maintenance.rs");
include!("shutdown.rs");
include!("runner.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn shutdown_coordinator_propagates_cancellation() {
        let coordinator = ShutdownCoordinator::new();
        let token = coordinator.token();
        let waiter = tokio::spawn(async move {
            token.cancelled().await;
        });

        coordinator.cancel();
        tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("cancellation was not propagated")
            .expect("cancellation waiter panicked");
    }

    #[tokio::test]
    async fn downloaded_media_paths_are_indexed_and_persisted() {
        let temp = tempdir().unwrap();
        let downloads = [
            ("movie.mkv", "video/x-matroska"),
            ("track.flac", "audio/flac"),
            ("cover.webp", "image/webp"),
        ];

        let database_path = temp.path().join("media.redb");
        let database = database::redb::RedbDatabase::new(database_path.clone())
            .await
            .unwrap();
        database.initialize().await.unwrap();
        for (filename, _) in downloads {
            let completed = temp.path().join(filename);
            tokio::fs::write(&completed, b"media").await.unwrap();
            index_media_file_path(&database, &completed).await.unwrap();
        }
        drop(database);

        let reopened = database::redb::RedbDatabase::new(database_path)
            .await
            .unwrap();
        reopened.initialize().await.unwrap();
        for (filename, mime_type) in downloads {
            let completed = temp.path().join(filename);
            let indexed = reopened
                .get_file_by_path(&completed)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(indexed.path, completed.canonicalize().unwrap());
            assert_eq!(indexed.size, 5);
            assert_eq!(indexed.mime_type, mime_type);
        }
    }

    #[test]
    fn failed_database_is_quarantined_without_changing_its_contents() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("media.redb");
        let original = b"unreadable database data";
        std::fs::write(&path, original).unwrap();

        let quarantine = preserve_failed_database(&path).unwrap().unwrap();

        assert!(!path.exists());
        assert_eq!(std::fs::read(&quarantine).unwrap(), original);
        let name = quarantine.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("media.failed-"));
        assert!(name.ends_with(".redb"));
    }
}

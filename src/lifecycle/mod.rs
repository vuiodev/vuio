use crate::{
    config::{
        AppConfig, ConfigChangeEvent, ConfigManager, MonitoredDirectoryConfig, ValidationMode,
    },
    database::{self, DatabaseManager, HealthRepository, StatsRepository},
    logging, media,
    platform::{
        self,
        filesystem::{create_platform_filesystem_manager, create_platform_path_normalizer},
        PlatformInfo,
    },
    ssdp,
    state::AppState,
    watcher::{CrossPlatformWatcher, FileSystemEvent, FileSystemWatcher, MediaRenameKind},
    web,
};
use anyhow::Context;
use std::collections::{HashMap, HashSet};
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
include!("update.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::MediaRepository;
    use futures_util::StreamExt;
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
        let policy = media::ScanPolicy::platform_default(temp.path(), false);
        let filesystem_manager = create_platform_filesystem_manager();
        for (filename, _) in downloads {
            let completed = temp.path().join(filename);
            tokio::fs::write(&completed, b"media").await.unwrap();
            index_media_file_path(&database, &completed, &policy, filesystem_manager.as_ref())
                .await
                .unwrap()
                .unwrap();
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

    #[cfg(unix)]
    #[tokio::test]
    async fn watcher_index_helper_rejects_symlinked_media() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().unwrap();
        let media_root = temp.path().join("media");
        tokio::fs::create_dir(&media_root).await.unwrap();
        let outside = temp.path().join("outside.mp4");
        tokio::fs::write(&outside, b"secret").await.unwrap();
        let link = media_root.join("created.mp4");
        symlink(&outside, &link).unwrap();

        let database = database::redb::RedbDatabase::new(temp.path().join("watcher.redb"))
            .await
            .unwrap();
        database.initialize().await.unwrap();
        let policy = media::ScanPolicy::platform_default(&media_root, true);
        let filesystem_manager = create_platform_filesystem_manager();

        assert!(
            index_media_file_path(&database, &link, &policy, filesystem_manager.as_ref(),)
                .await
                .unwrap()
                .is_none()
        );
        assert!(database
            .stream_all_media_files()
            .collect::<Vec<_>>()
            .await
            .is_empty());
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

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.0.33", "0.0.34"));
        assert!(is_newer_version("0.0.33", "0.1.0"));
        assert!(is_newer_version("0.0.33", "1.0.0"));
        assert!(is_newer_version("v0.0.33", "v0.0.34"));
        assert!(is_newer_version("0.0.33", "v0.0.34"));
        assert!(is_newer_version("v0.0.33", "0.0.34"));

        assert!(!is_newer_version("0.0.33", "0.0.33"));
        assert!(!is_newer_version("0.0.33", "0.0.32"));
        assert!(!is_newer_version("0.1.0", "0.0.33"));
        assert!(!is_newer_version("1.0.0", "0.9.9"));
        assert!(!is_newer_version("v0.0.33", "v0.0.33"));
    }
}

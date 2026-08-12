use crate::{
    config::{AppConfig, ConfigChangeEvent, ConfigManager},
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
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

mod bootstrap;
mod maintenance;
#[path = "media.rs"]
mod media_service;
mod network;
mod runner;
mod shutdown;
mod supervisor;

use bootstrap::*;
pub use bootstrap::ApplicationContext;
use maintenance::*;
use media_service::*;
pub use media_service::ApplicationStats;
use network::*;
pub use runner::ApplicationRunner;
pub use shutdown::ShutdownCoordinator;
use shutdown::*;

/// The handful of settings a host can override without owning the whole
/// configuration file.
///
/// These are applied *on top of* the loaded configuration rather than replacing it.
/// Carrying them as options, instead of as a whole `AppConfig`, is what makes that
/// possible: a full struct cannot say which of its 36 fields the caller actually
/// meant to set. Defined alongside the configuration it overrides, and re-exported
/// here because that is where callers expect it.
pub use crate::config::ConfigOverrides;

/// Host-provided options for starting the VuIO runtime.
///
/// Command-line parsing deliberately lives outside `vuio-core`; desktop and
/// service hosts construct this type directly.
#[derive(Clone)]
pub struct RuntimeOptions {
    pub debug: bool,
    pub config_path: Option<String>,
    pub log_file: Option<String>,
    pub log_level: Option<String>,
    pub overrides: ConfigOverrides,
    pub restore_backup: Option<String>,
    pub auth: bool,
    pub cancellation: CancellationToken,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            debug: false,
            config_path: None,
            log_file: None,
            log_level: None,
            overrides: ConfigOverrides::default(),
            restore_backup: None,
            auth: false,
            cancellation: CancellationToken::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::MediaRepository;
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

use anyhow::{Context, Result};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

mod diff;
pub mod generator;
mod model;
pub mod validation;

use model::{
    default_redb_cache_mb, default_session_ttl_hours, default_unavailable_root_grace_hours,
};
pub use model::{
    AppConfig, DatabaseConfig, ManagementConfig, MediaConfig, MonitoredDirectoryConfig,
    NetworkConfig, NetworkInterfaceConfig, ServerConfig, ValidationMode,
};

use crate::platform::config::PlatformConfig;
use generator::ConfigGenerator;
use validation::ConfigValidator;

mod loading;
mod platform;

/// Operational effect of changing a VuIO configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigChangeImpact {
    NoChange,
    LiveReload,
    RestartRequired,
}


impl Default for AppConfig {
    fn default() -> Self {
        Self::default_for_platform()
    }
}

/// Configuration change event
#[derive(Debug, Clone)]
pub enum ConfigChangeEvent {
    /// Configuration file was modified and reloaded
    Reloaded(Box<AppConfig>),
    /// Monitored directories changed
    DirectoriesChanged {
        added: Vec<PathBuf>,
        removed: Vec<PathBuf>,
        modified: Vec<PathBuf>,
    },
    /// Network configuration changed
    NetworkChanged {
        old_interface: NetworkInterfaceConfig,
        new_interface: NetworkInterfaceConfig,
        old_port: u16,
        new_port: u16,
    },
}

/// Configuration manager for handling runtime configuration operations
pub struct ConfigManager {
    config: Arc<RwLock<AppConfig>>,
    config_path: PathBuf,
    change_sender: broadcast::Sender<ConfigChangeEvent>,
    /// Held so the watcher outlives this manager; dropping it stops reloads.
    debouncer: Option<
        notify_debouncer_full::Debouncer<
            notify::RecommendedWatcher,
            notify_debouncer_full::FileIdMap,
        >,
    >,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let config_path = config_path.as_ref().to_path_buf();
        let config = AppConfig::load_or_create(&config_path)?;
        let (change_sender, _) = broadcast::channel(100);

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
            change_sender,
            debouncer: None,
        })
    }

    /// Create a new configuration manager with file watching enabled
    pub async fn new_with_watching<P: AsRef<Path>>(
        config_path: P,
        cancellation: tokio_util::sync::CancellationToken,
        background_tasks: tokio_util::task::TaskTracker,
    ) -> Result<Self> {
        let config_path = config_path.as_ref().to_path_buf();
        let config = AppConfig::load_or_create(&config_path)?;
        let (change_sender, _) = broadcast::channel(100);

        let config_arc = Arc::new(RwLock::new(config));
        let sender_clone = change_sender.clone();
        let path_clone = config_path.clone();
        let config_clone = config_arc.clone();

        // Set up file watcher
        let debouncer = Self::setup_file_watcher(
            path_clone,
            config_clone,
            sender_clone,
            cancellation,
            background_tasks,
        )
        .await?;

        Ok(Self {
            config: config_arc,
            config_path,
            change_sender,
            debouncer: Some(debouncer),
        })
    }

    /// Load a config the same way the manager's in-memory copy was built.
    ///
    /// `load_or_create` applies platform defaults; a bare `load_from_file` does not.
    /// Reloading without them made every reload look like a change — an unset
    /// `exclude_patterns` or `database.path` came back as `None` against an
    /// in-memory `Some`, which reports every media root as modified and drops and
    /// rescans it.
    fn load_comparable(config_path: &Path) -> Result<AppConfig> {
        let mut config = AppConfig::load_from_file(config_path)?;
        config.apply_platform_defaults()?;
        Ok(config)
    }

    /// Set up file watcher for configuration changes using notify-debouncer-full
    async fn setup_file_watcher(
        config_path: PathBuf,
        config: Arc<RwLock<AppConfig>>,
        sender: broadcast::Sender<ConfigChangeEvent>,
        cancellation: tokio_util::sync::CancellationToken,
        background_tasks: tokio_util::task::TaskTracker,
    ) -> Result<
        notify_debouncer_full::Debouncer<
            notify::RecommendedWatcher,
            notify_debouncer_full::FileIdMap,
        >,
    > {
        use notify_debouncer_full::{new_debouncer_opt, DebounceEventResult, Debouncer, FileIdMap};
        use tokio::sync::mpsc;

        let (tx, mut rx) = mpsc::channel(100);

        // Create debounced watcher with 500ms debounce duration
        let mut debouncer: Debouncer<notify::RecommendedWatcher, FileIdMap> = new_debouncer_opt(
            Duration::from_millis(500),
            None,
            move |result: DebounceEventResult| {
                let _ = tx.try_send(result);
            },
            FileIdMap::new(),
            notify::Config::default(),
        )?;

        // Watch the config file's parent directory
        if let Some(parent) = config_path.parent() {
            debouncer.watch(parent, notify::RecursiveMode::Recursive)?;
        }

        // Spawn task to handle debounced file events
        background_tasks.spawn(async move {
            loop {
                let result = tokio::select! {
                    _ = cancellation.cancelled() => break,
                    result = rx.recv() => match result {
                        Some(result) => result,
                        None => break,
                    },
                };
                match result {
                    Ok(events) => {
                        // Check if any event is for our config file
                        let config_file_modified = events
                            .iter()
                            .any(|event| event.paths.iter().any(|path| path == &config_path));

                        if !config_file_modified {
                            continue;
                        }

                        // Check if this is a modify or create event
                        let is_relevant_event = events.iter().any(|event| {
                            matches!(
                                event.kind,
                                notify::EventKind::Modify(_) | notify::EventKind::Create(_)
                            )
                        });

                        if !is_relevant_event {
                            continue;
                        }

                        // Attempt to reload configuration
                        match Self::load_comparable(&config_path) {
                            Ok(new_config) => {
                                // Validate the new configuration
                                if let Err(e) = ConfigValidator::validate_flexible(&new_config) {
                                    tracing::warn!(
                                        "Invalid configuration file, ignoring changes: {}",
                                        e
                                    );
                                    continue;
                                }

                                let old_config = {
                                    let mut config_guard = config.write().await;
                                    if *config_guard == new_config {
                                        // A rewrite that changed nothing still reaches us —
                                        // an editor saving, or the server writing the file
                                        // back itself. Broadcasting it would bump the
                                        // ContentDirectory update id, drop the browse cache
                                        // and NOTIFY every UPnP subscriber for no reason.
                                        tracing::debug!(
                                            "Configuration file rewritten with no changes, ignoring"
                                        );
                                        continue;
                                    }
                                    let old = config_guard.clone();
                                    *config_guard = new_config.clone();
                                    old
                                };

                                // Send change notifications
                                Self::send_change_notifications(&sender, &old_config, &new_config)
                                    .await;

                                tracing::info!("Configuration reloaded from file");
                            }
                            Err(e) => {
                                tracing::warn!("Failed to reload configuration: {}", e);
                            }
                        }
                    }
                    Err(errors) => {
                        for error in errors {
                            tracing::warn!("File watcher error: {}", error);
                        }
                    }
                }
            }
        });

        Ok(debouncer)
    }

    /// Send appropriate change notifications based on configuration differences
    async fn send_change_notifications(
        sender: &broadcast::Sender<ConfigChangeEvent>,
        old_config: &AppConfig,
        new_config: &AppConfig,
    ) {
        for change in diff::changes(old_config, new_config) {
            let _ = sender.send(change);
        }
    }

    /// Get the current configuration
    pub async fn get_config(&self) -> AppConfig {
        self.config.read().await.clone()
    }

    /// Update the configuration in memory only - do not save to file
    pub async fn update_config(&self, new_config: AppConfig) -> Result<()> {
        // Validate the new configuration
        ConfigValidator::validate_flexible(&new_config)?;

        let old_config = {
            let mut config_guard = self.config.write().await;
            let old = config_guard.clone();
            *config_guard = new_config.clone();
            old
        };

        // Send change notifications
        Self::send_change_notifications(&self.change_sender, &old_config, &new_config).await;

        Ok(())
    }

    /// Reload configuration from file
    pub async fn reload(&self) -> Result<()> {
        let new_config = Self::load_comparable(&self.config_path)?;

        let old_config = {
            let mut config_guard = self.config.write().await;
            let old = config_guard.clone();
            *config_guard = new_config.clone();
            old
        };

        // Send change notifications
        Self::send_change_notifications(&self.change_sender, &old_config, &new_config).await;

        Ok(())
    }

    /// Get the configuration file path
    pub fn get_config_path(&self) -> &Path {
        &self.config_path
    }

    /// Whether the config file is a durable one this manager watches.
    ///
    /// It is not in two cases: a container, whose configuration comes from
    /// environment variables and is dumped to a scratch file, and a run with
    /// command-line overrides, which are written to a scratch file so a reload
    /// cannot drop them. Writing to either would be discarded on restart, so the
    /// admin API refuses to.
    pub fn is_watched(&self) -> bool {
        self.debouncer.is_some()
    }

    /// Subscribe to configuration change events
    pub fn subscribe_to_changes(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.change_sender.subscribe()
    }

}

#[cfg(test)]
mod tests;

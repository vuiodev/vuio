//! The embedding entry point: start a VuIO server inside a host application.
//!
//! This module is the whole of `vuio-core`'s stable surface. Everything else is
//! internal, and hosts that need richer interaction use the server's HTTP and
//! MCP APIs rather than Rust types, so the promise made here stays small enough
//! to keep.

use crate::error::{Error, ErrorKind, Result};
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Current state of an embedded VuIO runtime.
///
/// New states may be added, so match with a `_` arm.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeStatus {
    /// Serving requests.
    Running,
    /// Shutdown has been requested and is in progress.
    Stopping,
    /// No longer running.
    Stopped,
    /// Stopped because of a failure; the string describes it.
    Failed(String),
}

/// How to start a runtime.
///
/// Fields are private and set through the methods below, so future options are
/// additive rather than breaking. Anything not covered here belongs in the
/// configuration file named by [`RuntimeOptions::config_path`].
///
/// ```no_run
/// use vuio_core::{Runtime, RuntimeOptions};
///
/// let handle = Runtime::start(
///     RuntimeOptions::new()
///         .media_dir("/srv/media")
///         .port(8080)
///         .server_name("Lounge"),
/// );
/// # let _ = handle;
/// ```
#[derive(Clone, Debug, Default)]
pub struct RuntimeOptions {
    pub(crate) debug: bool,
    pub(crate) config_path: Option<String>,
    pub(crate) log_file: Option<String>,
    pub(crate) log_level: Option<String>,
    pub(crate) restore_backup: Option<String>,
    pub(crate) management_auth: bool,
    pub(crate) port: Option<u16>,
    pub(crate) server_name: Option<String>,
    pub(crate) media_dirs: Vec<PathBuf>,
}

impl RuntimeOptions {
    /// Options with every default in place.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read configuration from this file instead of the platform default.
    pub fn config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into().to_string_lossy().into_owned());
        self
    }

    /// Serve this directory. Call repeatedly to serve several.
    pub fn media_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.media_dirs.push(path.into());
        self
    }

    /// Listen on this TCP port instead of the configured one.
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// The name shown to clients that discover this server.
    pub fn server_name(mut self, name: impl Into<String>) -> Self {
        self.server_name = Some(name.into());
        self
    }

    /// Emit verbose diagnostics.
    pub fn debug(mut self, enabled: bool) -> Self {
        self.debug = enabled;
        self
    }

    /// Write logs to this file in addition to the console.
    pub fn log_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.log_file = Some(path.into().to_string_lossy().into_owned());
        self
    }

    /// Set the log filter, in `RUST_LOG` syntax.
    pub fn log_level(mut self, level: impl Into<String>) -> Self {
        self.log_level = Some(level.into());
        self
    }

    /// Require authentication for the management API and dashboard.
    pub fn management_auth(mut self, enabled: bool) -> Self {
        self.management_auth = enabled;
        self
    }

    /// Restore this database backup before serving.
    pub fn restore_backup(mut self, path: impl Into<PathBuf>) -> Self {
        self.restore_backup = Some(path.into().to_string_lossy().into_owned());
        self
    }
}


impl RuntimeOptions {
    /// Translate the public options into the internal lifecycle options.
    ///
    /// The common overrides (media directories, port, name) are folded into a
    /// config override here, which is what keeps `AppConfig` and its 36 fields
    /// out of the public API.
    pub(crate) fn into_internal(
        self,
        cancellation: CancellationToken,
    ) -> crate::lifecycle::RuntimeOptions {
        let config_override = self.build_config_override();
        crate::lifecycle::RuntimeOptions {
            debug: self.debug,
            config_path: self.config_path,
            log_file: self.log_file,
            log_level: self.log_level,
            config_override,
            restore_backup: self.restore_backup,
            auth: self.management_auth,
            cancellation,
        }
    }

    fn build_config_override(&self) -> Option<crate::config::AppConfig> {
        if self.media_dirs.is_empty() && self.port.is_none() && self.server_name.is_none() {
            return None;
        }
        let mut config = crate::config::AppConfig::default_for_platform();
        if let Some(port) = self.port {
            config.server.port = port;
        }
        if let Some(name) = &self.server_name {
            config.server.name = name.clone();
        }
        if !self.media_dirs.is_empty() {
            config.media.directories = self
                .media_dirs
                .iter()
                .map(|path| {
                    if !path.is_dir() {
                        tracing::warn!("Media directory is not available: {}", path.display());
                    }
                    crate::config::MonitoredDirectoryConfig {
                        path: path.to_string_lossy().into_owned(),
                        recursive: true,
                        case_sensitive: None,
                        extensions: None,
                        exclude_patterns: None,
                        validation_mode: crate::config::ValidationMode::Warn,
                    }
                })
                .collect();
        }
        Some(config)
    }
}

/// Starts VuIO without installing command-line or process-signal behaviour.
pub struct Runtime;

impl Runtime {
    /// Start a runtime on the current Tokio executor.
    ///
    /// Returns immediately; the server runs in a spawned task. Dropping the
    /// returned handle requests shutdown.
    pub fn start(options: RuntimeOptions) -> RuntimeHandle {
        // Hosts embedding VuIO may not otherwise select a rustls provider.
        // Installation is process-global and harmless when a host selected one first.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let cancellation = CancellationToken::new();
        let internal = options.into_internal(cancellation.clone());
        let task = tokio::spawn(crate::lifecycle::ApplicationRunner::run(internal));

        RuntimeHandle {
            cancellation,
            task: Mutex::new(Some(task)),
        }
    }
}

/// Owns one in-process VuIO runtime and provides bounded lifecycle control.
pub struct RuntimeHandle {
    cancellation: CancellationToken,
    task: Mutex<Option<JoinHandle<anyhow::Result<()>>>>,
}

impl RuntimeHandle {
    /// What the runtime is doing right now.
    pub fn status(&self) -> RuntimeStatus {
        let task = self.task.lock().unwrap_or_else(|error| error.into_inner());
        match task.as_ref() {
            Some(_) if self.cancellation.is_cancelled() => RuntimeStatus::Stopping,
            Some(task) if !task.is_finished() => RuntimeStatus::Running,
            Some(_) => RuntimeStatus::Stopped,
            None => RuntimeStatus::Stopped,
        }
    }

    /// Ask the runtime to stop, without waiting for it.
    pub fn request_shutdown(&self) {
        self.cancellation.cancel();
    }

    /// Ask the runtime to stop and wait for it to finish.
    pub async fn shutdown(&self) -> Result<()> {
        self.request_shutdown();
        self.wait().await
    }

    /// Wait for the runtime to finish, however it was asked to stop.
    pub async fn wait(&self) -> Result<()> {
        let task = self
            .task
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        match task {
            Some(task) => match task.await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(error)) => Err(Error::with_source(
                    ErrorKind::Runtime,
                    "the VuIO runtime stopped with an error",
                    Box::<dyn std::error::Error + Send + Sync>::from(format!("{error:#}")),
                )),
                Err(error) => Err(Error::with_source(
                    ErrorKind::Runtime,
                    "the VuIO runtime task did not complete",
                    error,
                )),
            },
            None => Ok(()),
        }
    }
}

impl Drop for RuntimeHandle {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_collects_every_option() {
        let options = RuntimeOptions::new()
            .media_dir("/srv/a")
            .media_dir("/srv/b")
            .port(9999)
            .server_name("Lounge")
            .debug(true)
            .management_auth(true)
            .log_level("debug");
        assert_eq!(options.media_dirs.len(), 2);
        assert_eq!(options.port, Some(9999));
        assert_eq!(options.server_name.as_deref(), Some("Lounge"));
        assert!(options.debug && options.management_auth);
        assert_eq!(options.log_level.as_deref(), Some("debug"));
    }

    #[test]
    fn defaults_override_nothing() {
        let options = RuntimeOptions::new();
        assert!(options.media_dirs.is_empty());
        assert!(options.port.is_none() && options.server_name.is_none());
        assert!(!options.debug);
    }
}

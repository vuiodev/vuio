//! The embedding entry point: start a VuIO server inside a host application.
//!
//! This module is the whole of `vuio-core`'s stable surface. Everything else is
//! internal, and hosts that need richer interaction use the server's HTTP and
//! MCP APIs rather than Rust types, so the promise made here stays small enough
//! to keep.

use crate::error::{Error, ErrorKind, Result};
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
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
    /// The overrides travel as individual options rather than as a whole
    /// `AppConfig`, so they can be layered onto the configuration file instead
    /// of replacing it — and so `AppConfig` and its 36 fields stay out of the
    /// public API.
    pub(crate) fn into_internal(
        self,
        cancellation: CancellationToken,
    ) -> crate::lifecycle::RuntimeOptions {
        crate::lifecycle::RuntimeOptions {
            debug: self.debug,
            config_path: self.config_path,
            log_file: self.log_file,
            log_level: self.log_level,
            overrides: crate::lifecycle::ConfigOverrides {
                port: self.port,
                server_name: self.server_name,
                media_dirs: self.media_dirs,
            },
            restore_backup: self.restore_backup,
            auth: self.management_auth,
            cancellation,
        }
    }

}

/// Starts VuIO without installing command-line or process-signal behaviour.
///
/// A namespace rather than a value: `#[non_exhaustive]` keeps callers from
/// writing `Runtime` as a literal, so this can gain state later without a
/// major release.
#[non_exhaustive]
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
        spawn(crate::lifecycle::ApplicationRunner::run(internal), cancellation)
    }
}

/// Spawn a runtime future and wrap it in a handle.
///
/// Separate from [`Runtime::start`] so the status state machine can be tested
/// against a future that fails or blocks on demand, without binding sockets.
fn spawn(
    runtime: impl Future<Output = anyhow::Result<()>> + Send + 'static,
    cancellation: CancellationToken,
) -> RuntimeHandle {
    let failure: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let recorder = Arc::clone(&failure);

    // The outcome is recorded from inside the task, so it is written before the
    // handle reports finished. `status()` can then distinguish a clean stop
    // from a crash without awaiting, which `wait()` would require.
    let task = tokio::spawn(async move {
        let outcome = runtime.await;
        if let Err(error) = &outcome {
            *lock(&recorder) = Some(format!("{error:#}"));
        }
        outcome
    });

    RuntimeHandle {
        cancellation,
        task: Mutex::new(Some(task)),
        failure,
    }
}

/// Take a lock, ignoring poisoning: a panicked holder leaves readable state.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

/// Owns one in-process VuIO runtime and provides bounded lifecycle control.
pub struct RuntimeHandle {
    cancellation: CancellationToken,
    task: Mutex<Option<JoinHandle<anyhow::Result<()>>>>,
    failure: Arc<Mutex<Option<String>>>,
}

impl RuntimeHandle {
    /// What the runtime is doing right now.
    pub fn status(&self) -> RuntimeStatus {
        // Whether the task is still running is asked first: a runtime that was
        // cancelled and has since finished is stopped, not stopping.
        let still_running = lock(&self.task)
            .as_ref()
            .is_some_and(|task| !task.is_finished());

        if still_running {
            return if self.cancellation.is_cancelled() {
                RuntimeStatus::Stopping
            } else {
                RuntimeStatus::Running
            };
        }

        // Survives `wait()` taking the join handle, so a host that awaited a
        // failed runtime still sees why it stopped.
        match lock(&self.failure).clone() {
            Some(reason) => RuntimeStatus::Failed(reason),
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
        let task = lock(&self.task).take();
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

    #[tokio::test]
    async fn a_live_runtime_is_running_until_shutdown_is_asked_for() {
        let cancellation = CancellationToken::new();
        // Never resolves, so the task is still alive after cancellation and the
        // in-between state is observable rather than racy.
        let handle = spawn(std::future::pending::<anyhow::Result<()>>(), cancellation);

        assert_eq!(handle.status(), RuntimeStatus::Running);
        handle.request_shutdown();
        assert_eq!(handle.status(), RuntimeStatus::Stopping);
    }

    #[tokio::test]
    async fn a_runtime_that_honours_shutdown_ends_stopped() {
        let cancellation = CancellationToken::new();
        let token = cancellation.clone();
        let handle = spawn(
            async move {
                token.cancelled().await;
                Ok(())
            },
            cancellation,
        );

        handle.shutdown().await.expect("a clean stop is not an error");
        assert_eq!(handle.status(), RuntimeStatus::Stopped);
    }

    #[tokio::test]
    async fn a_crash_is_reported_as_failed_with_the_reason() {
        let handle = spawn(
            async { Err(anyhow::anyhow!("address 0.0.0.0:8080 already in use")) },
            CancellationToken::new(),
        );

        let error = handle.wait().await.expect_err("the runtime failed");
        assert_eq!(error.kind(), ErrorKind::Runtime);

        // `wait` consumed the join handle; the reason still has to be there,
        // because a host that awaits a failed runtime then asks why.
        match handle.status() {
            RuntimeStatus::Failed(reason) => assert!(reason.contains("already in use")),
            other => panic!("expected Failed, got {other:?}"),
        }
    }
}

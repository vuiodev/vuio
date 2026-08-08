use crate::lifecycle::{ApplicationRunner, RuntimeOptions};
use std::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Current state of an embedded VuIO runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeStatus {
    Running,
    Stopping,
    Stopped,
    Failed(String),
}

/// Starts VuIO without installing command-line or process-signal behavior.
pub struct VuioRuntime;

impl VuioRuntime {
    pub fn start(mut options: RuntimeOptions) -> RuntimeHandle {
        // Hosts embedding VuIO may not otherwise select a rustls provider.
        // Installation is process-global and harmless when a host selected one first.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let cancellation = CancellationToken::new();
        options.cancellation = cancellation.clone();
        let task = tokio::spawn(ApplicationRunner::run(options));

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
    pub fn status(&self) -> RuntimeStatus {
        let task = self.task.lock().unwrap_or_else(|error| error.into_inner());
        match task.as_ref() {
            Some(_) if self.cancellation.is_cancelled() => RuntimeStatus::Stopping,
            Some(task) if !task.is_finished() => RuntimeStatus::Running,
            Some(_) => RuntimeStatus::Stopped,
            None => RuntimeStatus::Stopped,
        }
    }

    pub fn request_shutdown(&self) {
        self.cancellation.cancel();
    }

    pub async fn shutdown(&self) -> anyhow::Result<()> {
        self.request_shutdown();
        self.wait().await
    }

    pub async fn wait(&self) -> anyhow::Result<()> {
        let task = self
            .task
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        match task {
            Some(task) => task.await.map_err(anyhow::Error::from)?,
            None => Ok(()),
        }
    }
}

impl Drop for RuntimeHandle {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

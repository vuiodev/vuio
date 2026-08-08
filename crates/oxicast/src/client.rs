//! The main Cast client — connects to a device and provides control methods.

pub mod builder;
pub mod connection;
pub mod framing;
pub mod heartbeat;
pub mod request_tracker;
pub mod router;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{self, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};
use tokio_rustls::client::TlsStream;

use crate::channel;
use crate::error::{Error, Result};
use crate::event::CastEvent;
use crate::state::{self, ConnectionState, StateHolder, StateWatchers};
use crate::types::*;

use self::request_tracker::RequestTracker;
use tokio_util::sync::CancellationToken;

/// Holds JoinHandles for background tasks so they can be deterministically
/// aborted on reconnect or disconnect.
struct TaskHandles {
    cancel: CancellationToken,
    reader: Option<tokio::task::JoinHandle<()>>,
    writer: Option<tokio::task::JoinHandle<()>>,
    heartbeat: Option<tokio::task::JoinHandle<()>>,
}

impl TaskHandles {
    fn new(cancel: CancellationToken) -> Self {
        Self { cancel, reader: None, writer: None, heartbeat: None }
    }

    /// Abort all running tasks and wait for them to finish.
    async fn shutdown(&mut self) {
        self.cancel.cancel();
        if let Some(h) = self.reader.take() {
            h.abort();
            let _ = h.await;
        }
        if let Some(h) = self.writer.take() {
            h.abort();
            let _ = h.await;
        }
        if let Some(h) = self.heartbeat.take() {
            h.abort();
            let _ = h.await;
        }
    }
}

/// A connected Google Cast device client.
///
/// `CastClient` is the primary API surface. It is [`Clone`], [`Send`], and [`Sync`] —
/// cloning shares the same underlying connection.
///
/// # Cleanup
///
/// Call [`disconnect()`](Self::disconnect) for graceful shutdown with a terminal
/// `Disconnected` event and `None` from [`next_event()`](Self::next_event).
///
/// If you just drop all `CastClient` handles, background tasks are cancelled
/// automatically via a `CancellationToken` hierarchy. This is safe for app-exit
/// scenarios where the async runtime may be shutting down.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> oxicast::Result<()> {
/// let client = oxicast::CastClient::connect("192.168.1.100", 8009).await?;
///
/// client.launch_app(&oxicast::CastApp::DefaultMediaReceiver).await?;
/// client.load_media(
///     &oxicast::MediaInfo::new("https://example.com/video.mp4", "video/mp4"),
///     true,
///     0.0,
///     None,
/// ).await?;
///
/// while let Some(event) = client.next_event().await {
///     println!("{event:?}");
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
#[must_use]
pub struct CastClient {
    inner: Arc<ClientInner>,
}

/// Shared client configuration (immutable after creation).
#[derive(Clone)]
pub(crate) struct ClientConfig {
    pub host: String,
    pub port: u16,
    pub auto_reconnect: bool,
    pub max_reconnect_attempts: u32,
    pub reconnect_delay: std::time::Duration,
    pub heartbeat_interval: std::time::Duration,
    pub heartbeat_timeout: std::time::Duration,
    pub verify_tls: bool,
}

struct ClientInner {
    /// Swappable write channel — replaced on reconnect.
    write_tx: tokio::sync::RwLock<mpsc::Sender<crate::proto::CastMessage>>,
    request_tracker: Arc<RequestTracker>,
    /// Swappable event sender — replaced on disconnect, kept on reconnect.
    event_tx: tokio::sync::RwLock<mpsc::Sender<CastEvent>>,
    event_rx: tokio::sync::Mutex<mpsc::Receiver<CastEvent>>,
    /// Parent cancellation token — cancelled on Drop to stop all background tasks.
    parent_cancel: CancellationToken,
    /// State holder shared with reader tasks — survives reconnect.
    state: Arc<StateHolder>,
    watchers: StateWatchers,
    alive: Arc<AtomicBool>,
    /// Explicit user-initiated shutdown flag (prevents auto-reconnect after disconnect).
    shutting_down: Arc<AtomicBool>,
    /// Task handles for deterministic shutdown on reconnect.
    task_handles: tokio::sync::Mutex<TaskHandles>,
    /// Serializes reconnect attempts so manual reconnect() and auto-reconnect don't race.
    reconnect_lock: tokio::sync::Mutex<()>,
    config: ClientConfig,
    /// Shared liveness tracker — updated by reader on every inbound message.
    last_activity: heartbeat::LastActivity,
    /// Current app transport ID (set after launch_app).
    transport_id: tokio::sync::Mutex<Option<String>>,
    /// Current app session ID.
    session_id: tokio::sync::Mutex<Option<String>>,
}

impl Drop for ClientInner {
    fn drop(&mut self) {
        // Cancel all background tasks when the last CastClient handle is dropped.
        self.parent_cancel.cancel();
    }
}

impl CastClient {
    /// Connect to a Cast device with default settings.
    pub async fn connect(host: &str, port: u16) -> Result<Self> {
        Self::builder(host, port).connect().await
    }

    /// Create a builder for advanced configuration.
    pub fn builder(host: impl Into<String>, port: u16) -> builder::CastClientBuilder {
        builder::CastClientBuilder::new(host, port)
    }

    /// Internal constructor called by the builder.
    pub(crate) async fn from_builder(config: &builder::CastClientBuilder) -> Result<Self> {
        // Install default crypto provider if not already set
        let _ = rustls::crypto::ring::default_provider().install_default();

        // Establish TLS connection
        let tls_stream = connection::connect(&config.host, config.port, config.verify_tls).await?;
        let (read_half, write_half) = io::split(tls_stream);

        // Create channels and shared state
        let (write_tx, write_rx) = mpsc::channel::<crate::proto::CastMessage>(64);
        let (event_tx, event_rx) = mpsc::channel::<CastEvent>(config.event_buffer_size);
        let request_tracker = Arc::new(RequestTracker::new(config.request_timeout));
        let (state_holder, watchers) = state::new_state();
        let alive = Arc::new(AtomicBool::new(true));
        let shutting_down = Arc::new(AtomicBool::new(false));
        let parent_cancel = CancellationToken::new();
        let cancel = parent_cancel.child_token();
        let last_activity = heartbeat::new_last_activity();

        // Spawn background tasks with cancellation support
        let mut handles = TaskHandles::new(cancel.clone());
        handles.writer = Some(tokio::spawn(writer_loop(
            write_half,
            write_rx,
            cancel.clone(),
            alive.clone(),
            state_holder.clone(),
        )));
        handles.reader = Some(tokio::spawn(reader_loop(
            read_half,
            ReaderContext {
                request_tracker: request_tracker.clone(),
                event_tx: event_tx.clone(),
                state: state_holder.clone(),
                write_tx: write_tx.clone(),
                cancel: cancel.clone(),
                alive: alive.clone(),
                last_activity: last_activity.clone(),
            },
        )));
        handles.heartbeat = Some(heartbeat::spawn_heartbeat_task(heartbeat::HeartbeatConfig {
            write_tx: write_tx.clone(),
            interval: config.heartbeat_interval,
            cancel: cancel.clone(),
            last_activity: last_activity.clone(),
            timeout: config.heartbeat_timeout,
            alive: alive.clone(),
            event_tx: event_tx.clone(),
            connection_tx: state_holder.connection_tx.clone(),
        }));

        // Send CONNECT to receiver-0
        write_tx
            .send(channel::connection::connect_msg("receiver-0"))
            .await
            .map_err(|_| Error::Disconnected)?;

        let _ = event_tx.try_send(CastEvent::Connected);
        let _ = state_holder.connection_tx.send(ConnectionState::Connected);

        let client_config = ClientConfig {
            host: config.host.clone(),
            port: config.port,
            auto_reconnect: config.auto_reconnect,
            max_reconnect_attempts: config.max_reconnect_attempts,
            reconnect_delay: config.reconnect_delay,
            heartbeat_interval: config.heartbeat_interval,
            heartbeat_timeout: config.heartbeat_timeout,
            verify_tls: config.verify_tls,
        };

        let client = Self {
            inner: Arc::new(ClientInner {
                write_tx: tokio::sync::RwLock::new(write_tx),
                request_tracker,
                event_tx: tokio::sync::RwLock::new(event_tx),
                event_rx: tokio::sync::Mutex::new(event_rx),
                parent_cancel,
                state: state_holder,
                watchers,
                alive,
                shutting_down,
                task_handles: tokio::sync::Mutex::new(handles),
                reconnect_lock: tokio::sync::Mutex::new(()),
                config: client_config,
                last_activity,
                transport_id: tokio::sync::Mutex::new(None),
                session_id: tokio::sync::Mutex::new(None),
            }),
        };

        spawn_auto_reconnect(client.clone());

        Ok(client)
    }

    // ── Events ───────────────────────────────────────────────

    /// Receive the next event from the device.
    ///
    /// Returns `None` when the connection is permanently closed.
    /// Use this in a `tokio::select!` loop.
    ///
    /// **Note:** This is single-consumer — only one task should call `next_event()`
    /// at a time. If you need multiple listeners, use [`watch_media_status()`](Self::watch_media_status)
    /// or [`watch_receiver_status()`](Self::watch_receiver_status) instead.
    pub async fn next_event(&self) -> Option<CastEvent> {
        self.inner.event_rx.lock().await.recv().await
    }

    /// Get a watch receiver for the latest media status (always up-to-date).
    pub fn watch_media_status(&self) -> watch::Receiver<Option<MediaStatus>> {
        self.inner.watchers.media.clone()
    }

    /// Get a watch receiver for the latest receiver status.
    pub fn watch_receiver_status(&self) -> watch::Receiver<Option<ReceiverStatus>> {
        self.inner.watchers.receiver.clone()
    }

    /// Get the current connection state.
    pub fn connection_state(&self) -> ConnectionState {
        self.inner.watchers.connection.borrow().clone()
    }

    /// Check if the connection is alive.
    pub fn is_connected(&self) -> bool {
        self.inner.alive.load(Ordering::Acquire)
    }

    /// Disconnect from the device.
    ///
    /// Stops media playback, closes the connection, and shuts down all
    /// background tasks. The device stops immediately — it won't continue
    /// playing buffered content. Auto-reconnect is disabled.
    pub async fn disconnect(&self) -> Result<()> {
        self.inner.shutting_down.store(true, Ordering::Release);
        // Stop media first so the device stops playback immediately
        // instead of draining its buffer for several more seconds.
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            self.stop_media().await.ok();
            self.send(channel::connection::close_msg("receiver-0")).await.ok();
        })
        .await;
        self.inner.alive.store(false, Ordering::Release);
        // Deterministically stop all background tasks
        self.inner.task_handles.lock().await.shutdown().await;
        self.inner.request_tracker.clear().await;
        let _ = self.inner.state.connection_tx.send(ConnectionState::Disconnected);
        // Emit terminal Disconnected event, then drop the sender so
        // next_event() returns None instead of hanging forever.
        self.try_send_event(CastEvent::Disconnected(None));
        // Replace sender with a dummy that will cause recv() to return None
        // once the buffer drains. The old sender is dropped.
        let (dead_tx, _dead_rx) = mpsc::channel(1);
        *self.inner.event_tx.write().await = dead_tx;
        Ok(())
    }

    /// Manually trigger a reconnection.
    ///
    /// Creates a new TLS connection, spawns new reader/writer/heartbeat tasks,
    /// and re-sends CONNECT to the receiver. Previous transport/session state
    /// is preserved if you need to re-launch the app.
    pub async fn reconnect(&self) -> Result<()> {
        self.reconnect_with_attempt(1).await
    }

    /// Internal reconnect with attempt tracking for ConnectionState watchers.
    async fn reconnect_with_attempt(&self, attempt: u32) -> Result<()> {
        // Serialize reconnects so manual reconnect() and auto-reconnect don't race.
        let _reconnect_guard = self.inner.reconnect_lock.lock().await;

        // If disconnect() was called, don't re-establish the connection.
        if self.inner.shutting_down.load(Ordering::Acquire) {
            return Err(Error::Disconnected);
        }

        // If we got here but someone else already reconnected, bail early.
        if self.inner.alive.load(Ordering::Acquire) {
            tracing::debug!("reconnect: already connected, skipping");
            return Ok(());
        }

        let config = &self.inner.config;
        tracing::info!("reconnecting to {}:{} (attempt {attempt})", config.host, config.port);

        let _ = self.inner.state.connection_tx.send(ConnectionState::Reconnecting { attempt });
        self.try_send_event(CastEvent::Reconnecting { attempt });

        // 1. Deterministically shut down all old tasks
        {
            let mut handles = self.inner.task_handles.lock().await;
            handles.shutdown().await;
        }

        // 2. Establish new TLS connection
        let tls_stream = connection::connect(&config.host, config.port, config.verify_tls).await?;
        let (read_half, write_half) = io::split(tls_stream);

        // Re-check shutting_down after the TLS handshake. If disconnect() was
        // called while we were connecting, bail before spawning new tasks.
        if self.inner.shutting_down.load(Ordering::Acquire) {
            tracing::debug!("reconnect: disconnect() called during TLS handshake, aborting");
            return Err(Error::Disconnected);
        }

        // 3. New write channel and cancellation token.
        // Event channels are NOT replaced — the same event_tx/event_rx pair stays alive
        // across reconnects so next_event() never sees a spurious None.
        let (new_write_tx, new_write_rx) = mpsc::channel::<crate::proto::CastMessage>(64);
        let cancel = self.inner.parent_cancel.child_token();

        *self.inner.write_tx.write().await = new_write_tx.clone();
        let event_tx = self.inner.event_tx.read().await.clone();
        self.inner.alive.store(true, Ordering::Release);
        self.inner.shutting_down.store(false, Ordering::Release);
        self.inner.request_tracker.clear().await;

        // 4. Spawn fresh tasks with new cancellation token
        let mut handles = TaskHandles::new(cancel.clone());
        handles.writer = Some(tokio::spawn(writer_loop(
            write_half,
            new_write_rx,
            cancel.clone(),
            self.inner.alive.clone(),
            self.inner.state.clone(),
        )));
        heartbeat::touch(&self.inner.last_activity);
        handles.reader = Some(tokio::spawn(reader_loop(
            read_half,
            ReaderContext {
                request_tracker: self.inner.request_tracker.clone(),
                event_tx: event_tx.clone(),
                state: self.inner.state.clone(),
                write_tx: new_write_tx.clone(),
                cancel: cancel.clone(),
                alive: self.inner.alive.clone(),
                last_activity: self.inner.last_activity.clone(),
            },
        )));
        handles.heartbeat = Some(heartbeat::spawn_heartbeat_task(heartbeat::HeartbeatConfig {
            write_tx: new_write_tx,
            interval: config.heartbeat_interval,
            cancel,
            last_activity: self.inner.last_activity.clone(),
            timeout: config.heartbeat_timeout,
            alive: self.inner.alive.clone(),
            event_tx,
            connection_tx: self.inner.state.connection_tx.clone(),
        }));

        *self.inner.task_handles.lock().await = handles;

        // 5. Re-establish Cast protocol connections
        self.send(channel::connection::connect_msg("receiver-0")).await?;
        if let Some(tid) = self.inner.transport_id.lock().await.as_ref() {
            self.send(channel::connection::connect_msg(tid)).await?;
        }

        let _ = self.inner.state.connection_tx.send(ConnectionState::Connected);
        self.try_send_event(CastEvent::Reconnected);
        tracing::info!("reconnected to {}:{}", config.host, config.port);
        Ok(())
    }

    // ── Receiver Control ─────────────────────────────────────

    /// Get the current receiver status.
    pub async fn receiver_status(&self) -> Result<ReceiverStatus> {
        let (id, rx) = self.inner.request_tracker.register().await;
        self.send(channel::receiver::get_status(id)).await?;
        let json = self.inner.request_tracker.wait_for(id, rx).await?;
        Self::check_device_error(&json)?;

        router::parse_receiver_status_from_json(&json)
            .ok_or_else(|| Error::Internal("failed to parse receiver status".into()))
    }

    /// Launch an application on the device.
    pub async fn launch_app(&self, app: &CastApp) -> Result<Application> {
        let (id, rx) = self.inner.request_tracker.register().await;
        self.send(channel::receiver::launch_app(id, app.app_id())).await?;
        let json = self.inner.request_tracker.wait_for(id, rx).await?;
        tracing::debug!("launch_app response: {}", json);
        Self::check_device_error(&json)?;

        // Custom receivers may take longer to load. If the first response doesn't
        // contain the app, wait for a subsequent RECEIVER_STATUS that does.
        let target_id = app.app_id().to_string();

        let status = router::parse_receiver_status_from_json(&json);
        if let Some(status) = status {
            if let Some(app_info) = status.applications.into_iter().find(|a| a.app_id == target_id)
            {
                self.send(channel::connection::connect_msg(&app_info.transport_id)).await?;
                *self.inner.transport_id.lock().await = Some(app_info.transport_id.clone());
                *self.inner.session_id.lock().await = Some(app_info.session_id.clone());
                return Ok(app_info);
            }
        }

        // App not in first response — wait for a status update (custom receiver loading)
        tracing::debug!("launch_app: app not in first response, waiting for status update...");
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
        while tokio::time::Instant::now() < deadline {
            if let Some(event) =
                tokio::time::timeout(std::time::Duration::from_secs(3), self.next_event())
                    .await
                    .ok()
                    .flatten()
            {
                #[allow(clippy::collapsible_match)]
                if let CastEvent::ReceiverStatusChanged(ref rs) = event {
                    if let Some(app_info) = rs.applications.iter().find(|a| a.app_id == target_id) {
                        let app_info = app_info.clone();
                        self.send(channel::connection::connect_msg(&app_info.transport_id)).await?;
                        *self.inner.transport_id.lock().await = Some(app_info.transport_id.clone());
                        *self.inner.session_id.lock().await = Some(app_info.session_id.clone());
                        return Ok(app_info);
                    }
                }
            }
        }

        Err(Error::LaunchFailed {
            reason: format!("app {target_id} not found after launch (timeout)"),
        })
    }

    /// Stop the specified application.
    pub async fn stop_app(&self, session_id: &str) -> Result<()> {
        let (id, rx) = self.inner.request_tracker.register().await;
        self.send(channel::receiver::stop_app(id, session_id)).await?;
        let json = self.inner.request_tracker.wait_for(id, rx).await?;
        Self::check_device_error(&json)?;
        *self.inner.transport_id.lock().await = None;
        *self.inner.session_id.lock().await = None;
        Ok(())
    }

    /// Set the device volume (0.0 to 1.0). Values outside this range are clamped.
    pub async fn set_volume(&self, level: f32) -> Result<Volume> {
        let level = level.clamp(0.0, 1.0);
        let (id, rx) = self.inner.request_tracker.register().await;
        self.send(channel::receiver::set_volume(id, Some(level), None)).await?;
        let json = self.inner.request_tracker.wait_for(id, rx).await?;
        Self::check_device_error(&json)?;
        router::parse_receiver_status_from_json(&json)
            .map(|s| s.volume)
            .ok_or_else(|| Error::Internal("failed to parse volume from response".into()))
    }

    /// Mute or unmute the device.
    pub async fn set_muted(&self, muted: bool) -> Result<Volume> {
        let (id, rx) = self.inner.request_tracker.register().await;
        self.send(channel::receiver::set_volume(id, None, Some(muted))).await?;
        let json = self.inner.request_tracker.wait_for(id, rx).await?;
        Self::check_device_error(&json)?;
        router::parse_receiver_status_from_json(&json)
            .map(|s| s.volume)
            .ok_or_else(|| Error::Internal("failed to parse volume from response".into()))
    }

    // ── Media Control ────────────────────────────────────────

    /// Load media onto the device.
    ///
    /// Requires an app to be running (call [`launch_app`](Self::launch_app) first).
    /// Returns the initial media status after the load command.
    ///
    /// Pass `custom_data` to send application-specific data to a Custom Web Receiver
    /// (read via `setMessageInterceptor` for `MessageType.LOAD`).
    pub async fn load_media(
        &self,
        media: &MediaInfo,
        autoplay: bool,
        current_time: f64,
        custom_data: Option<&serde_json::Value>,
    ) -> Result<MediaStatus> {
        let transport_id = self.get_transport_id().await?;
        let session_id = self.get_session_id().await?;

        let (id, rx) = self.inner.request_tracker.register().await;
        self.send(channel::media::load(
            id,
            &transport_id,
            &session_id,
            media,
            autoplay,
            current_time,
            custom_data,
        ))
        .await?;
        let json = self.inner.request_tracker.wait_for(id, rx).await?;

        Self::check_device_error(&json)?;

        // Extract media session ID from response
        if let Some(entries) = json.get("status").and_then(|s| s.as_array()) {
            if let Some(entry) = entries.first() {
                if let Some(msid) = entry.get("mediaSessionId").and_then(|m| m.as_i64()) {
                    self.inner
                        .state
                        .media_session_id
                        .store(msid as i32, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }

        let status = router::parse_media_status_from_json(&json).ok_or_else(|| {
            Error::LoadFailed { reason: "no media status in response".into(), detailed_error: None }
        })?;

        // The device sometimes sends an IDLE MEDIA_STATUS (clearing previous state)
        // before the actual LOAD_FAILED arrives. Detect this: if the response is
        // IDLE with no media loaded, the load did not succeed.
        if status.player_state == PlayerState::Idle && status.media.is_none() {
            return Err(Error::LoadFailed {
                reason: "media not accepted by device (received IDLE with no media)".into(),
                detailed_error: None,
            });
        }

        Ok(status)
    }

    /// Resume playback.
    pub async fn play(&self) -> Result<MediaStatus> {
        let transport_id = self.get_transport_id().await?;
        let msid = self.get_media_session_id().await?;
        let (id, rx) = self.inner.request_tracker.register().await;
        self.send(channel::media::play(id, &transport_id, msid)).await?;
        let json = self.inner.request_tracker.wait_for(id, rx).await?;
        Self::check_device_error(&json)?;
        router::parse_media_status_from_json(&json).ok_or(Error::NoMediaSession)
    }

    /// Pause playback.
    pub async fn pause(&self) -> Result<MediaStatus> {
        let transport_id = self.get_transport_id().await?;
        let msid = self.get_media_session_id().await?;
        let (id, rx) = self.inner.request_tracker.register().await;
        self.send(channel::media::pause(id, &transport_id, msid)).await?;
        let json = self.inner.request_tracker.wait_for(id, rx).await?;
        Self::check_device_error(&json)?;
        router::parse_media_status_from_json(&json).ok_or(Error::NoMediaSession)
    }

    /// Stop the current media session.
    pub async fn stop_media(&self) -> Result<MediaStatus> {
        let transport_id = self.get_transport_id().await?;
        let msid = self.get_media_session_id().await?;
        let (id, rx) = self.inner.request_tracker.register().await;
        self.send(channel::media::stop(id, &transport_id, msid)).await?;
        let json = self.inner.request_tracker.wait_for(id, rx).await?;
        Self::check_device_error(&json)?;
        self.inner.state.media_session_id.store(0, std::sync::atomic::Ordering::Relaxed);
        router::parse_media_status_from_json(&json).ok_or(Error::NoMediaSession)
    }

    /// Seek to a position in seconds.
    pub async fn seek(&self, position: f64) -> Result<MediaStatus> {
        let transport_id = self.get_transport_id().await?;
        let msid = self.get_media_session_id().await?;
        let (id, rx) = self.inner.request_tracker.register().await;
        self.send(channel::media::seek(id, &transport_id, msid, position)).await?;
        let json = self.inner.request_tracker.wait_for(id, rx).await?;
        Self::check_device_error(&json)?;
        router::parse_media_status_from_json(&json).ok_or(Error::NoMediaSession)
    }

    /// Get the current media status.
    pub async fn media_status(&self) -> Result<Option<MediaStatus>> {
        let transport_id = match self.get_transport_id().await {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };
        let (id, rx) = self.inner.request_tracker.register().await;
        self.send(channel::media::get_status(id, &transport_id)).await?;
        let json = self.inner.request_tracker.wait_for(id, rx).await?;
        Self::check_device_error(&json)?;
        Ok(router::parse_media_status_from_json(&json))
    }

    // ── Queue Management ─────────────────────────────────────

    /// Load a queue of media items.
    pub async fn queue_load(
        &self,
        items: &[QueueItem],
        start_index: u32,
        repeat_mode: RepeatMode,
    ) -> Result<MediaStatus> {
        let transport_id = self.get_transport_id().await?;
        let session_id = self.get_session_id().await?;
        let (id, rx) = self.inner.request_tracker.register().await;
        self.send(channel::media::queue_load(
            id,
            &transport_id,
            &session_id,
            items,
            start_index,
            repeat_mode,
        ))
        .await?;
        let json = self.inner.request_tracker.wait_for(id, rx).await?;
        Self::check_device_error(&json)?;
        if let Some(entries) = json.get("status").and_then(|s| s.as_array()) {
            if let Some(entry) = entries.first() {
                if let Some(msid) = entry.get("mediaSessionId").and_then(|m| m.as_i64()) {
                    self.inner
                        .state
                        .media_session_id
                        .store(msid as i32, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
        let status =
            router::parse_media_status_from_json(&json).ok_or_else(|| Error::LoadFailed {
                reason: "no media status in queue load response".into(),
                detailed_error: None,
            })?;

        if status.player_state == PlayerState::Idle && status.media.is_none() {
            return Err(Error::LoadFailed {
                reason: "media not accepted by device (received IDLE with no media)".into(),
                detailed_error: None,
            });
        }

        Ok(status)
    }

    /// Insert items into the current queue.
    pub async fn queue_insert(
        &self,
        items: &[QueueItem],
        insert_before: Option<u32>,
    ) -> Result<MediaStatus> {
        let transport_id = self.get_transport_id().await?;
        let msid = self.get_media_session_id().await?;
        let (id, rx) = self.inner.request_tracker.register().await;
        self.send(channel::media::queue_insert(id, &transport_id, msid, items, insert_before))
            .await?;
        let json = self.inner.request_tracker.wait_for(id, rx).await?;
        Self::check_device_error(&json)?;
        router::parse_media_status_from_json(&json).ok_or(Error::NoMediaSession)
    }

    // ── Local File Casting ────────────────────────────────────

    /// Serve a local file via HTTP and cast it to the device in one call.
    ///
    /// Starts a temporary HTTP server, registers the file, and loads it
    /// onto the currently running Cast app. The returned [`FileServer`](crate::serve::FileServer)
    /// must be kept alive for the duration of playback.
    ///
    /// Requires an app to be running (call [`launch_app`](Self::launch_app) first).
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn example(client: &oxicast::CastClient) -> oxicast::Result<()> {
    /// let (_server, status) = client.serve_and_cast(
    ///     "/path/to/video.mp4",
    ///     "video/mp4",
    ///     true,
    ///     0.0,
    /// ).await?;
    /// println!("Playing: {:?}", status.player_state);
    /// // Keep `_server` alive until playback is done!
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "serve")]
    pub async fn serve_and_cast(
        &self,
        path: impl Into<std::path::PathBuf>,
        content_type: &str,
        autoplay: bool,
        current_time: f64,
    ) -> Result<(crate::serve::FileServer, MediaStatus)> {
        let server = crate::serve::FileServer::start("0.0.0.0:0").await?;
        let url = server.serve_file(path, content_type)?;
        let media = MediaInfo::new(&url, content_type);
        let status = self.load_media(&media, autoplay, current_time, None).await?;
        Ok((server, status))
    }

    // ── Raw / Advanced ───────────────────────────────────────

    /// Send a raw JSON message and wait for a correlated response.
    ///
    /// The payload must be a JSON object. A `requestId` field is injected
    /// automatically for response correlation.
    pub async fn send_raw(
        &self,
        namespace: &str,
        destination: &str,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let mut payload = match payload {
            serde_json::Value::Object(map) => serde_json::Value::Object(map),
            _ => {
                return Err(Error::InvalidPayload);
            }
        };
        let (id, rx) = self.inner.request_tracker.register().await;
        payload["requestId"] = serde_json::json!(id);
        let msg = framing::build_message(namespace, "sender-0", destination, &payload.to_string());
        self.send(msg).await?;
        self.inner.request_tracker.wait_for(id, rx).await
    }

    /// Send a raw JSON message without waiting for a response.
    pub async fn send_raw_no_reply(
        &self,
        namespace: &str,
        destination: &str,
        payload: serde_json::Value,
    ) -> Result<()> {
        let msg = framing::build_message(namespace, "sender-0", destination, &payload.to_string());
        self.send(msg).await
    }

    // ── Internal ─────────────────────────────────────────────

    async fn send(&self, msg: crate::proto::CastMessage) -> Result<()> {
        // Clone sender under lock, drop lock, then await send.
        // Avoids holding RwLock read guard across .await (reduces contention during reconnect).
        let tx = self.inner.write_tx.read().await.clone();
        tx.send(msg).await.map_err(|_| Error::Disconnected)
    }

    /// Check a device response JSON for protocol error types.
    /// Returns Ok(()) if no error, or the appropriate Error variant.
    fn check_device_error(json: &serde_json::Value) -> Result<()> {
        match json.get("type").and_then(|t| t.as_str()) {
            Some("LOAD_FAILED") => Err(Error::LoadFailed {
                reason: "Chromecast rejected the media".into(),
                detailed_error: json
                    .get("detailedErrorCode")
                    .and_then(|c| c.as_i64())
                    .map(|c| format!("error code {c}")),
            }),
            Some("LOAD_CANCELLED") => {
                Err(Error::LoadFailed { reason: "load was cancelled".into(), detailed_error: None })
            }
            Some("INVALID_REQUEST") => {
                let reason = json.get("reason").and_then(|r| r.as_str()).unwrap_or("unknown");
                let req_id = json.get("requestId").and_then(|r| r.as_u64()).unwrap_or(0);
                Err(Error::InvalidRequest { request_id: req_id as u32, reason: reason.to_string() })
            }
            Some("LAUNCH_ERROR") => {
                let reason = json.get("reason").and_then(|r| r.as_str()).unwrap_or("unknown");
                Err(Error::LaunchFailed { reason: reason.to_string() })
            }
            _ => Ok(()),
        }
    }

    /// Try to send an event without blocking. Used throughout to avoid backpressure.
    fn try_send_event(&self, event: CastEvent) {
        // RwLock::try_read avoids blocking; if locked during reconnect, drop the event.
        if let Ok(tx) = self.inner.event_tx.try_read() {
            let _ = tx.try_send(event);
        }
    }

    async fn get_transport_id(&self) -> Result<String> {
        self.inner.transport_id.lock().await.clone().ok_or(Error::NoApplication)
    }

    async fn get_session_id(&self) -> Result<String> {
        self.inner.session_id.lock().await.clone().ok_or(Error::NoApplication)
    }

    async fn get_media_session_id(&self) -> Result<i32> {
        let id = self.inner.state.media_session_id.load(std::sync::atomic::Ordering::Relaxed);
        if id > 0 { Ok(id) } else { Err(Error::NoMediaSession) }
    }
}

// ── Background Tasks ─────────────────────────────────────────

/// Writer task — owns the write half, sends messages from the channel.
async fn writer_loop(
    mut writer: WriteHalf<TlsStream<TcpStream>>,
    mut rx: mpsc::Receiver<crate::proto::CastMessage>,
    cancel: CancellationToken,
    alive: Arc<AtomicBool>,
    state: Arc<state::StateHolder>,
) {
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Some(msg) => {
                        tracing::trace!(
                            ns = %msg.namespace,
                            dst = %msg.destination_id,
                            payload = ?msg.payload_utf8.as_deref().unwrap_or(""),
                            "→ send"
                        );
                        if let Err(e) = framing::write_message(&mut writer, &msg).await {
                            tracing::error!("write error: {e}");
                            alive.store(false, Ordering::Release);
                            let _ = state.connection_tx.send(ConnectionState::Disconnected);
                            break;
                        }
                    }
                    None => break, // channel closed
                }
            }
            _ = cancel.cancelled() => {
                tracing::debug!("writer task cancelled");
                break;
            }
        }
    }
    tracing::debug!("writer task exited");
}

/// Context passed to the reader task.
struct ReaderContext {
    request_tracker: Arc<RequestTracker>,
    event_tx: mpsc::Sender<CastEvent>,
    state: Arc<state::StateHolder>,
    write_tx: mpsc::Sender<crate::proto::CastMessage>,
    cancel: CancellationToken,
    alive: Arc<AtomicBool>,
    last_activity: heartbeat::LastActivity,
}

/// Reader task — owns the read half, routes all inbound messages.
async fn reader_loop(mut reader: ReadHalf<TlsStream<TcpStream>>, ctx: ReaderContext) {
    loop {
        tokio::select! {
            result = framing::read_message(&mut reader) => {
                match result {
                    Ok(msg) => {
                        heartbeat::touch(&ctx.last_activity);
                        tracing::trace!(
                            ns = %msg.namespace,
                            src = %msg.source_id,
                            dst = %msg.destination_id,
                            payload = ?msg.payload_utf8.as_deref().unwrap_or(""),
                            "← recv"
                        );
                        router::route(&msg, &ctx.request_tracker, &ctx.event_tx, &ctx.state, &ctx.write_tx).await;
                    }
                    Err(Error::Disconnected) => {
                        tracing::info!("connection closed by device");
                        ctx.alive.store(false, Ordering::Release);
                        let _ = ctx.event_tx.try_send(CastEvent::Disconnected(None));
                        break;
                    }
                    Err(e) => {
                        tracing::error!("read error: {e}");
                        ctx.alive.store(false, Ordering::Release);
                        let _ = ctx.event_tx.try_send(CastEvent::Disconnected(Some(e.to_string())));
                        break;
                    }
                }
            }
            _ = ctx.cancel.cancelled() => {
                tracing::debug!("reader task cancelled");
                // Don't emit Disconnected — this is a controlled shutdown (reconnect or disconnect).
                // The caller manages state transitions.
                ctx.request_tracker.clear().await;
                tracing::debug!("reader task exited (cancelled)");
                return;
            }
        }
    }

    // Only emit Disconnected for uncontrolled exits (I/O errors, device close)
    let _ = ctx.state.connection_tx.send(ConnectionState::Disconnected);
    ctx.request_tracker.clear().await;
    tracing::debug!("reader task exited");
}

/// Spawn auto-reconnect monitoring task. Watches the `alive` flag
/// and triggers reconnection when it becomes false.
pub(crate) fn spawn_auto_reconnect(client: CastClient) {
    let config = client.inner.config.clone();
    if !config.auto_reconnect || config.max_reconnect_attempts == 0 {
        return;
    }

    // Use Weak to break the Arc cycle: task → ClientInner → StateHolder → watch sender.
    // If all user-held CastClient handles are dropped, the Weak upgrades fail and the task exits.
    let weak = Arc::downgrade(&client.inner);
    let mut conn_rx = client.inner.watchers.connection.clone();
    drop(client); // release the strong Arc

    tokio::spawn(async move {
        loop {
            // Wait for a state change (reactive, no polling)
            if conn_rx.changed().await.is_err() {
                return; // sender dropped
            }

            // Try to upgrade Weak — if all CastClient handles dropped, exit
            let Some(inner) = weak.upgrade() else {
                tracing::debug!("auto-reconnect: all client handles dropped, exiting");
                return;
            };
            let client = CastClient { inner };

            if client.inner.shutting_down.load(Ordering::Acquire) {
                tracing::debug!("auto-reconnect: shutdown flag set, exiting");
                return;
            }

            let is_disconnected =
                matches!(*conn_rx.borrow_and_update(), state::ConnectionState::Disconnected);

            if is_disconnected {
                tracing::info!("auto-reconnect: connection lost, attempting recovery");

                for attempt in 1..=config.max_reconnect_attempts {
                    let base_delay = config.reconnect_delay * 2u32.saturating_pow(attempt - 1);
                    // Jitter: use wall-clock nanos for entropy (not monotonic Instant)
                    let seed = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .subsec_nanos() as u64;
                    let jitter_range_ms = (base_delay.as_millis() as u64) / 4; // ±25%
                    let delay = if jitter_range_ms > 0 {
                        // Centered jitter: [base * 0.75, base * 1.25)
                        let raw = seed % (jitter_range_ms * 2);
                        let offset = raw as i64 - jitter_range_ms as i64;
                        if offset >= 0 {
                            base_delay + std::time::Duration::from_millis(offset as u64)
                        } else {
                            base_delay.saturating_sub(std::time::Duration::from_millis(
                                offset.unsigned_abs(),
                            ))
                        }
                    } else {
                        base_delay
                    };
                    tracing::info!(
                        "auto-reconnect: attempt {attempt}/{} in {delay:?}",
                        config.max_reconnect_attempts
                    );

                    tokio::time::sleep(delay).await;

                    match client.reconnect_with_attempt(attempt).await {
                        Ok(()) => {
                            tracing::info!("auto-reconnect: success on attempt {attempt}");
                            break;
                        }
                        Err(e) => {
                            tracing::warn!("auto-reconnect: attempt {attempt} failed: {e}");
                            if attempt == config.max_reconnect_attempts {
                                tracing::error!("auto-reconnect: all attempts exhausted");
                                let _ = client
                                    .inner
                                    .state
                                    .connection_tx
                                    .send(ConnectionState::Disconnected);
                                client.try_send_event(CastEvent::Disconnected(Some(
                                    "reconnect failed".into(),
                                )));
                                return; // Give up
                            }
                        }
                    }
                }
            }
        }
    });
}

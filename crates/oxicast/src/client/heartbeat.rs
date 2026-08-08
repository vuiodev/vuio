//! Automatic heartbeat management.
//!
//! The Cast protocol requires PING/PONG messages every ~5 seconds or
//! the device drops the connection. This module handles heartbeats
//! completely transparently — users never see them.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::channel::ns;
use crate::client::framing::build_message;
use crate::event::CastEvent;
use crate::proto::CastMessage;

/// Shared monotonic timestamp of the last received message.
/// Uses Instant (monotonic) to avoid clock drift issues.
pub type LastActivity = Arc<std::sync::Mutex<Instant>>;

/// Create a new last-activity tracker initialized to now.
pub fn new_last_activity() -> LastActivity {
    Arc::new(std::sync::Mutex::new(Instant::now()))
}

/// Record that activity was observed (called by the reader on every message).
pub fn touch(last_activity: &LastActivity) {
    if let Ok(mut t) = last_activity.lock() {
        *t = Instant::now();
    }
}

/// Check if a message is a heartbeat PING.
///
/// Uses a lightweight string check instead of full JSON parsing
/// since the heartbeat payload is always `{"type":"PING"}`.
pub fn is_ping(msg: &CastMessage) -> bool {
    if msg.namespace != ns::NS_HEARTBEAT {
        return false;
    }
    msg.payload_utf8.as_deref().is_some_and(|p| p.contains("\"PING\""))
}

/// Build a PONG response message.
pub fn pong() -> CastMessage {
    let payload = serde_json::json!({ "type": ns::MSG_PONG });
    build_message(ns::NS_HEARTBEAT, ns::SENDER_ID, ns::RECEIVER_ID, &payload.to_string())
}

/// Build a PING message.
pub fn ping() -> CastMessage {
    let payload = serde_json::json!({ "type": ns::MSG_PING });
    build_message(ns::NS_HEARTBEAT, ns::SENDER_ID, ns::RECEIVER_ID, &payload.to_string())
}

/// Check if a message is a heartbeat PONG.
#[cfg(test)]
pub fn is_pong(msg: &CastMessage) -> bool {
    if msg.namespace != ns::NS_HEARTBEAT {
        return false;
    }
    msg.payload_utf8.as_deref().is_some_and(|p| p.contains("\"PONG\""))
}

/// Configuration for spawning a heartbeat task.
pub struct HeartbeatConfig {
    /// Channel to send outbound PING messages.
    pub write_tx: mpsc::Sender<CastMessage>,
    /// Interval between PING sends.
    pub interval: Duration,
    /// Cancellation token for graceful shutdown.
    pub cancel: CancellationToken,
    /// Shared liveness tracker.
    pub last_activity: LastActivity,
    /// How long without activity before declaring timeout.
    pub timeout: Duration,
    /// Shared alive flag.
    pub alive: Arc<AtomicBool>,
    /// Channel to emit CastEvents.
    pub event_tx: mpsc::Sender<CastEvent>,
    /// Channel to signal connection state changes.
    pub connection_tx: tokio::sync::watch::Sender<crate::state::ConnectionState>,
}

/// Spawn a heartbeat task that sends PINGs and monitors for liveness.
///
/// If no message is received within `timeout` after a PING, emits
/// `CastEvent::HeartbeatTimeout` and sets `alive=false` to trigger auto-reconnect.
pub fn spawn_heartbeat_task(cfg: HeartbeatConfig) -> tokio::task::JoinHandle<()> {
    let HeartbeatConfig {
        write_tx,
        interval,
        cancel,
        last_activity,
        timeout,
        alive,
        event_tx,
        connection_tx,
    } = cfg;
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.tick().await; // skip first immediate tick

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if write_tx.send(ping()).await.is_err() {
                        tracing::debug!("heartbeat task stopping (write channel closed)");
                        break;
                    }
                    tracing::trace!("sent PING");

                    // Check liveness using monotonic Instant
                    let elapsed = last_activity
                        .lock()
                        .map(|t| t.elapsed())
                        .unwrap_or(Duration::ZERO);
                    if elapsed > timeout {
                        tracing::warn!("heartbeat timeout: no activity for {elapsed:?}");
                        alive.store(false, Ordering::Release);
                        let _ = connection_tx.send(crate::state::ConnectionState::Disconnected);
                        let _ = event_tx.try_send(CastEvent::HeartbeatTimeout);
                        break;
                    }
                }
                _ = cancel.cancelled() => {
                    tracing::debug!("heartbeat task cancelled");
                    break;
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_conn_tx() -> tokio::sync::watch::Sender<crate::state::ConnectionState> {
        let (tx, _rx) = tokio::sync::watch::channel(crate::state::ConnectionState::Connected);
        tx
    }

    fn make_heartbeat_msg(payload: &str) -> CastMessage {
        CastMessage {
            protocol_version: 0,
            source_id: "receiver-0".to_string(),
            destination_id: "sender-0".to_string(),
            namespace: ns::NS_HEARTBEAT.to_string(),
            payload_type: 0,
            payload_utf8: Some(payload.to_string()),
            payload_binary: None,
            continued: None,
            remaining_length: None,
        }
    }

    #[test]
    fn test_is_ping_valid() {
        let msg = make_heartbeat_msg(r#"{"type":"PING"}"#);
        assert!(is_ping(&msg));
    }

    #[test]
    fn test_is_ping_pong_returns_false() {
        let msg = make_heartbeat_msg(r#"{"type":"PONG"}"#);
        assert!(!is_ping(&msg));
    }

    #[test]
    fn test_is_ping_wrong_namespace() {
        let mut msg = make_heartbeat_msg(r#"{"type":"PING"}"#);
        msg.namespace = "urn:x-cast:com.google.cast.receiver".to_string();
        assert!(!is_ping(&msg));
    }

    #[test]
    fn test_is_ping_no_payload() {
        let mut msg = make_heartbeat_msg(r#"{"type":"PING"}"#);
        msg.payload_utf8 = None;
        assert!(!is_ping(&msg));
    }

    #[test]
    fn test_is_ping_invalid_json() {
        let msg = make_heartbeat_msg("not json at all");
        assert!(!is_ping(&msg));
    }

    #[test]
    fn test_is_pong_valid() {
        let msg = make_heartbeat_msg(r#"{"type":"PONG"}"#);
        assert!(is_pong(&msg));
    }

    #[test]
    fn test_is_pong_ping_returns_false() {
        let msg = make_heartbeat_msg(r#"{"type":"PING"}"#);
        assert!(!is_pong(&msg));
    }

    #[test]
    fn test_touch_updates_last_activity() {
        let la = new_last_activity();
        // Sleep briefly to ensure the initial Instant is in the past
        std::thread::sleep(Duration::from_millis(10));
        let elapsed_before = la.lock().unwrap().elapsed();
        assert!(elapsed_before >= Duration::from_millis(10));

        touch(&la);
        let elapsed_after = la.lock().unwrap().elapsed();
        assert!(elapsed_after < Duration::from_millis(5));
    }

    #[tokio::test]
    async fn test_heartbeat_sends_ping() {
        let (write_tx, mut write_rx) = mpsc::channel(16);
        let (event_tx, _event_rx) = mpsc::channel(16);
        let cancel = CancellationToken::new();
        let la = new_last_activity();
        let alive = Arc::new(AtomicBool::new(true));

        let _handle = spawn_heartbeat_task(HeartbeatConfig {
            write_tx,
            interval: Duration::from_millis(50),
            cancel: cancel.clone(),
            last_activity: la.clone(),
            timeout: Duration::from_secs(60),
            alive,
            event_tx,
            connection_tx: make_conn_tx(),
        });

        // Touch to keep alive
        touch(&la);

        // Wait for at least one PING
        let msg =
            tokio::time::timeout(Duration::from_secs(1), write_rx.recv()).await.unwrap().unwrap();
        assert!(is_ping(&msg));

        cancel.cancel();
    }

    #[tokio::test]
    async fn test_heartbeat_timeout_detection() {
        let (write_tx, _write_rx) = mpsc::channel(16);
        let (event_tx, mut event_rx) = mpsc::channel(16);
        let cancel = CancellationToken::new();
        let la = new_last_activity();
        let alive = Arc::new(AtomicBool::new(true));

        // Set last activity far in the past to trigger timeout immediately
        {
            let mut t = la.lock().unwrap();
            *t = Instant::now() - Duration::from_secs(100);
        }

        let _handle = spawn_heartbeat_task(HeartbeatConfig {
            write_tx,
            interval: Duration::from_millis(50),
            cancel: cancel.clone(),
            last_activity: la,
            timeout: Duration::from_millis(10),
            alive: alive.clone(),
            event_tx,
            connection_tx: make_conn_tx(),
        });

        // Should receive HeartbeatTimeout event
        let event =
            tokio::time::timeout(Duration::from_secs(2), event_rx.recv()).await.unwrap().unwrap();
        assert_eq!(event, CastEvent::HeartbeatTimeout);
        assert!(!alive.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn test_heartbeat_cancellation() {
        let (write_tx, _write_rx) = mpsc::channel(16);
        let (event_tx, _event_rx) = mpsc::channel(16);
        let cancel = CancellationToken::new();
        let la = new_last_activity();
        let alive = Arc::new(AtomicBool::new(true));

        let handle = spawn_heartbeat_task(HeartbeatConfig {
            write_tx,
            interval: Duration::from_millis(50),
            cancel: cancel.clone(),
            last_activity: la,
            timeout: Duration::from_secs(60),
            alive: alive.clone(),
            event_tx,
            connection_tx: make_conn_tx(),
        });

        cancel.cancel();
        let result = tokio::time::timeout(Duration::from_secs(1), handle).await;
        assert!(result.is_ok()); // Task completed without panic
        assert!(alive.load(Ordering::Acquire)); // alive not changed by cancellation
    }
}

//! Reactive status model for Cast device state.
//!
//! Uses `tokio::sync::watch` channels so consumers always have access to the
//! latest status without polling. The router updates these on every incoming
//! `RECEIVER_STATUS` and `MEDIA_STATUS` message.

use crate::types::{MediaStatus, ReceiverStatus};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use tokio::sync::watch;

/// Connection lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connected and operational.
    Connected,
    /// Connection lost.
    Disconnected,
    /// Attempting to reconnect.
    Reconnecting {
        /// Current attempt number.
        attempt: u32,
    },
}

/// Internal state holder, updated by the router task.
pub(crate) struct StateHolder {
    pub media_tx: watch::Sender<Option<MediaStatus>>,
    pub receiver_tx: watch::Sender<Option<ReceiverStatus>>,
    pub connection_tx: watch::Sender<ConnectionState>,
    /// Tracks the current media session ID from broadcasts.
    /// Shared with CastClient via Arc.
    pub media_session_id: Arc<AtomicI32>,
}

/// Read-only handles for consumers to watch state changes.
pub struct StateWatchers {
    /// Latest media playback status. `None` when no media is loaded.
    pub media: watch::Receiver<Option<MediaStatus>>,
    /// Latest receiver status. `None` before first status update.
    pub receiver: watch::Receiver<Option<ReceiverStatus>>,
    /// Current connection state.
    pub connection: watch::Receiver<ConnectionState>,
}

/// Create a new state holder and its corresponding watchers.
/// The StateHolder is wrapped in Arc so it can be shared between
/// the client and reader tasks, surviving reconnections.
pub(crate) fn new_state() -> (Arc<StateHolder>, StateWatchers) {
    let (media_tx, media_rx) = watch::channel(None);
    let (receiver_tx, receiver_rx) = watch::channel(None);
    let (connection_tx, connection_rx) = watch::channel(ConnectionState::Connected);

    (
        Arc::new(StateHolder {
            media_tx,
            receiver_tx,
            connection_tx,
            media_session_id: Arc::new(AtomicI32::new(0)),
        }),
        StateWatchers { media: media_rx, receiver: receiver_rx, connection: connection_rx },
    )
}

impl StateHolder {
    /// Update the media session ID.
    pub fn set_media_session_id(&self, id: i32) {
        self.media_session_id.store(id, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state_defaults() {
        let (state, watchers) = new_state();
        assert_eq!(state.media_session_id.load(Ordering::Relaxed), 0);
        assert_eq!(*watchers.media.borrow(), None);
        assert_eq!(*watchers.receiver.borrow(), None);
        assert_eq!(*watchers.connection.borrow(), ConnectionState::Connected);
    }

    #[tokio::test]
    async fn test_media_watch_channel_updates() {
        let (state, mut watchers) = new_state();
        let status = crate::types::MediaStatus {
            media_session_id: 42,
            player_state: crate::types::PlayerState::Playing,
            idle_reason: None,
            current_time: 10.0,
            duration: Some(120.0),
            volume: crate::types::Volume { level: 1.0, muted: false },
            media: None,
        };
        let _ = state.media_tx.send(Some(status.clone()));

        watchers.media.changed().await.unwrap();
        let current = watchers.media.borrow_and_update().clone();
        assert_eq!(current.unwrap().media_session_id, 42);
    }

    #[tokio::test]
    async fn test_connection_state_transitions() {
        let (state, mut watchers) = new_state();

        let _ = state.connection_tx.send(ConnectionState::Disconnected);
        watchers.connection.changed().await.unwrap();
        assert_eq!(*watchers.connection.borrow_and_update(), ConnectionState::Disconnected);

        let _ = state.connection_tx.send(ConnectionState::Reconnecting { attempt: 3 });
        watchers.connection.changed().await.unwrap();
        assert_eq!(
            *watchers.connection.borrow_and_update(),
            ConnectionState::Reconnecting { attempt: 3 }
        );

        let _ = state.connection_tx.send(ConnectionState::Connected);
        watchers.connection.changed().await.unwrap();
        assert_eq!(*watchers.connection.borrow_and_update(), ConnectionState::Connected);
    }

    #[test]
    fn test_set_media_session_id() {
        let (state, _watchers) = new_state();
        state.set_media_session_id(99);
        assert_eq!(state.media_session_id.load(Ordering::Relaxed), 99);
    }

    #[test]
    fn test_connection_state_eq() {
        assert_eq!(ConnectionState::Connected, ConnectionState::Connected);
        assert_ne!(ConnectionState::Connected, ConnectionState::Disconnected);
        assert_eq!(
            ConnectionState::Reconnecting { attempt: 1 },
            ConnectionState::Reconnecting { attempt: 1 }
        );
        assert_ne!(
            ConnectionState::Reconnecting { attempt: 1 },
            ConnectionState::Reconnecting { attempt: 2 }
        );
    }
}

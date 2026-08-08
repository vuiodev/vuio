//! Events emitted by a Cast device connection.

use crate::types::{IdleReason, MediaStatus, ReceiverStatus};

/// Events received from a Cast device.
///
/// Use [`CastClient::next_event()`](crate::CastClient::next_event) to receive these.
///
/// # Example
///
/// ```no_run
/// # async fn example(client: oxicast::CastClient) {
/// loop {
///     tokio::select! {
///         Some(event) = client.next_event() => match event {
///             oxicast::CastEvent::MediaStatusChanged(status) => {
///                 println!("Position: {:.1}s", status.current_time);
///             }
///             oxicast::CastEvent::Disconnected(reason) => {
///                 println!("Lost connection: {reason:?}");
///                 break;
///             }
///             _ => {}
///         },
///         _ = tokio::signal::ctrl_c() => break,
///     }
/// }
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum CastEvent {
    /// Successfully connected to the device.
    Connected,

    /// Connection to the device was lost.
    Disconnected(Option<String>),

    /// Attempting to reconnect after connection loss.
    Reconnecting {
        /// The current reconnection attempt number.
        attempt: u32,
    },

    /// Successfully reconnected after connection loss.
    Reconnected,

    /// The device stopped responding to heartbeat pings within the configured timeout.
    /// The connection is considered dead — auto-reconnect will trigger if enabled.
    HeartbeatTimeout,

    /// The receiver status changed (volume, running apps, etc.).
    ReceiverStatusChanged(ReceiverStatus),

    /// Media playback status changed (state, position, etc.).
    MediaStatusChanged(MediaStatus),

    /// A media session ended.
    MediaSessionEnded {
        /// The ID of the ended media session.
        media_session_id: i32,
        /// Why the session ended.
        idle_reason: IdleReason,
    },

    /// A message was received on an unhandled or custom namespace.
    RawMessage {
        /// The Cast namespace of the message.
        namespace: String,
        /// The sender ID.
        source: String,
        /// The destination ID.
        destination: String,
        /// The JSON payload.
        payload: String,
    },
}

impl CastEvent {
    /// Returns the media status if this is a `MediaStatusChanged` event.
    pub fn as_media_status(&self) -> Option<&MediaStatus> {
        match self {
            CastEvent::MediaStatusChanged(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the receiver status if this is a `ReceiverStatusChanged` event.
    pub fn as_receiver_status(&self) -> Option<&ReceiverStatus> {
        match self {
            CastEvent::ReceiverStatusChanged(s) => Some(s),
            _ => None,
        }
    }

    /// Returns true if this is a disconnection event.
    pub fn is_disconnected(&self) -> bool {
        matches!(self, CastEvent::Disconnected(_))
    }

    /// Returns true if this is a media status update.
    pub fn is_media_status(&self) -> bool {
        matches!(self, CastEvent::MediaStatusChanged(_))
    }
}

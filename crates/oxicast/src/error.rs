//! Error types for oxicast.

use std::time::Duration;

/// All errors that can occur when using oxicast.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// TCP connection to the Cast device failed.
    #[error("connection failed: {0}")]
    Connect(#[source] std::io::Error),

    /// TLS handshake with the Cast device failed.
    #[error("TLS handshake failed: {0}")]
    Tls(String),

    /// The connection to the device was closed.
    #[error("connection closed by device")]
    Disconnected,

    /// Error reading or writing the length-prefixed protobuf framing.
    #[error("message framing error: {0}")]
    Framing(String),

    /// Failed to decode a protobuf message.
    #[error("protobuf decode error: {0}")]
    Protobuf(#[from] prost::DecodeError),

    /// Failed to serialize or deserialize a JSON payload.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// A request to the device timed out.
    #[error("request timed out after {0:?}")]
    Timeout(Duration),

    /// The device refused to launch the requested application.
    #[error("application launch failed: {reason}")]
    LaunchFailed {
        /// The reason provided by the device.
        reason: String,
    },

    /// The device refused to load the requested media.
    #[error("media load failed: {reason}")]
    LoadFailed {
        /// The reason provided by the device.
        reason: String,
        /// Detailed error code (if available).
        detailed_error: Option<String>,
    },

    /// The device reported an invalid request.
    #[error("invalid request (id={request_id}): {reason}")]
    InvalidRequest {
        /// The request ID that was rejected.
        request_id: u32,
        /// The reason provided by the device.
        reason: String,
    },

    /// The payload passed to `send_raw()` is not a JSON object.
    #[error("payload must be a JSON object")]
    InvalidPayload,

    /// mDNS device discovery failed.
    #[error("discovery failed: {0}")]
    Discovery(String),

    /// File not found (for the `serve` feature).
    #[error("file not found: {0}")]
    FileNotFound(String),

    /// No active media session to control.
    #[error("no active media session")]
    NoMediaSession,

    /// No running application on the device.
    #[error("no running application")]
    NoApplication,

    /// Internal error.
    #[error("{0}")]
    Internal(String),
}

/// Convenience type alias for `Result<T, oxicast::Error>`.
pub type Result<T> = std::result::Result<T, Error>;

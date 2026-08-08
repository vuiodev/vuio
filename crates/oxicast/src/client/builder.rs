//! Builder for CastClient with advanced configuration.

use crate::CastClient;
use crate::error::Result;
use std::time::Duration;

/// Builder for creating a [`CastClient`] with custom configuration.
#[must_use]
pub struct CastClientBuilder {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) heartbeat_interval: Duration,
    pub(crate) heartbeat_timeout: Duration,
    pub(crate) request_timeout: Duration,
    pub(crate) auto_reconnect: bool,
    pub(crate) max_reconnect_attempts: u32,
    pub(crate) reconnect_delay: Duration,
    pub(crate) event_buffer_size: usize,
    pub(crate) verify_tls: bool,
}

impl CastClientBuilder {
    /// Create a new builder with default configuration for the given host and port.
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            heartbeat_interval: Duration::from_secs(5),
            heartbeat_timeout: Duration::from_secs(15),
            request_timeout: Duration::from_secs(10),
            auto_reconnect: true,
            max_reconnect_attempts: 5,
            reconnect_delay: Duration::from_secs(2),
            event_buffer_size: 64,
            verify_tls: false,
        }
    }

    /// Set the interval between heartbeat PING messages.
    pub fn heartbeat_interval(mut self, interval: Duration) -> Self {
        self.heartbeat_interval = interval;
        self
    }

    /// Set the timeout for heartbeat liveness detection.
    ///
    /// If no message is received from the device within this duration,
    /// the connection is considered dead and auto-reconnect triggers.
    pub fn heartbeat_timeout(mut self, timeout: Duration) -> Self {
        self.heartbeat_timeout = timeout;
        self
    }

    /// Set the timeout for request-response operations.
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Enable or disable automatic reconnection on connection loss.
    pub fn auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }

    /// Set the maximum number of reconnection attempts.
    pub fn max_reconnect_attempts(mut self, max: u32) -> Self {
        self.max_reconnect_attempts = max;
        self
    }

    /// Set the base delay between reconnection attempts.
    pub fn reconnect_delay(mut self, delay: Duration) -> Self {
        self.reconnect_delay = delay;
        self
    }

    /// Set the buffer size for the event channel.
    pub fn event_buffer_size(mut self, size: usize) -> Self {
        self.event_buffer_size = size;
        self
    }

    /// Enable or disable TLS certificate verification.
    /// Default: `false` (Cast devices use self-signed certificates).
    pub fn verify_tls(mut self, verify: bool) -> Self {
        self.verify_tls = verify;
        self
    }

    /// Connect to the Cast device with the configured settings.
    pub async fn connect(self) -> Result<CastClient> {
        CastClient::from_builder(&self).await
    }
}

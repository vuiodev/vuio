//! Chromecast Castv2 client implementation.
//!
//! Connects to a Chromecast device over TLS on port 8009, performs
//! the handshake, manages heartbeats, and provides media control
//! operations (load, play, pause, stop, seek, volume).

use super::proto::{self, CastMessage};
use anyhow::{Context, Result};
use prost::Message;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_rustls::TlsStream;
use tracing::{debug, error, trace, warn};

/// Chromecast controller for a single device.
pub struct ChromecastClient {
    address: SocketAddr,
    stream: Arc<Mutex<TlsStream<TcpStream>>>,
    transport_id: Arc<Mutex<Option<String>>>,
    media_session_id: Arc<Mutex<Option<i32>>>,
    request_counter: AtomicI32,
    heartbeat_cancel: tokio_util::sync::CancellationToken,
    heartbeat_task: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    closed: AtomicBool,
}

impl ChromecastClient {
    /// Connect, launch the receiver, and load media under one operation-level deadline.
    pub async fn connect_and_load(
        address: SocketAddr,
        media_url: &str,
        mime_type: &str,
        title: &str,
    ) -> Result<Self> {
        tokio::time::timeout(Duration::from_secs(20), async {
            let client = Self::connect(address).await?;
            client.launch_media_receiver().await?;
            client.load(media_url, mime_type, title).await?;
            Ok(client)
        })
        .await
        .context("Chromecast cast operation exceeded its 20 second deadline")?
    }

    /// Connect to a Chromecast device at the given address.
    pub async fn connect(address: SocketAddr) -> Result<Self> {
        debug!(%address, "Connecting to Chromecast");

        let tcp_stream = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(address))
            .await
            .context("timed out connecting to Chromecast TCP")?
            .context("failed to connect to Chromecast TCP")?;

        // Chromecast uses a self-signed certificate, so we must disable verification.
        let config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoCertVerifier))
            .with_no_client_auth();

        let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
        let domain =
            rustls::pki_types::ServerName::try_from("chromecast").expect("valid server name");

        let tls_stream = tokio::time::timeout(
            Duration::from_secs(5),
            connector.connect(domain, tcp_stream),
        )
        .await
        .context("Chromecast TLS handshake timed out")?
        .context("TLS handshake with Chromecast failed")?;

        let stream = Arc::new(Mutex::new(TlsStream::Client(tls_stream)));

        // Send CONNECT to the receiver
        let connect_msg = CastMessage::connect(proto::DEFAULT_RECEIVER_ID);
        send_message(&stream, &connect_msg).await?;
        debug!("Sent CONNECT to Chromecast receiver");

        // Start the heartbeat loop
        let heartbeat_cancel = tokio_util::sync::CancellationToken::new();
        let hb_stream = stream.clone();
        let hb_cancel = heartbeat_cancel.clone();
        let heartbeat_task = tokio::spawn(async move {
            heartbeat_loop(hb_stream, hb_cancel).await;
        });

        Ok(Self {
            address,
            stream,
            transport_id: Arc::new(Mutex::new(None)),
            media_session_id: Arc::new(Mutex::new(None)),
            request_counter: AtomicI32::new(10),
            heartbeat_cancel,
            heartbeat_task: std::sync::Mutex::new(Some(heartbeat_task)),
            closed: AtomicBool::new(false),
        })
    }

    fn next_request_id(&self) -> i32 {
        self.request_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Launch the Default Media Receiver app and connect to it.
    pub async fn launch_media_receiver(&self) -> Result<String> {
        let launch_msg = CastMessage::launch_default_media_receiver();
        send_message(&self.stream, &launch_msg).await?;
        debug!("Sent LAUNCH for Default Media Receiver");

        // Read responses to find the transport ID
        let transport_id = self.wait_for_transport_id().await?;
        debug!(transport_id = %transport_id, "Media receiver launched");

        // Connect to the transport
        let connect_msg = CastMessage::connect(&transport_id);
        send_message(&self.stream, &connect_msg).await?;

        *self.transport_id.lock().await = Some(transport_id.clone());
        Ok(transport_id)
    }

    /// Load a media URL for playback.
    pub async fn load(&self, media_url: &str, mime_type: &str, title: &str) -> Result<()> {
        let transport_id = self
            .transport_id
            .lock()
            .await
            .clone()
            .context("no transport ID; call launch_media_receiver first")?;

        let request_id = self.next_request_id();
        let load_msg =
            CastMessage::load_media(&transport_id, media_url, mime_type, title, request_id);
        send_message(&self.stream, &load_msg).await?;
        debug!(url = %media_url, "Sent LOAD command to Chromecast");

        // Wait for the media session ID
        let session_id = self.wait_for_media_session_id().await?;
        *self.media_session_id.lock().await = Some(session_id);
        debug!(session_id, "Media session started");

        Ok(())
    }

    /// Pause playback.
    pub async fn pause(&self) -> Result<()> {
        self.send_media_command("PAUSE").await
    }

    /// Resume playback.
    pub async fn play(&self) -> Result<()> {
        self.send_media_command("PLAY").await
    }

    /// Stop playback.
    pub async fn stop(&self) -> Result<()> {
        self.send_media_command("STOP").await
    }

    /// Seek to a position in seconds.
    pub async fn seek(&self, position_secs: f64) -> Result<()> {
        let transport_id = self
            .transport_id
            .lock()
            .await
            .clone()
            .context("no transport ID")?;
        let session_id = self
            .media_session_id
            .lock()
            .await
            .context("no media session ID")?;
        let request_id = self.next_request_id();

        let msg = CastMessage::seek(&transport_id, session_id, position_secs, request_id);
        send_message(&self.stream, &msg).await?;
        debug!(position = position_secs, "Sent SEEK command");
        Ok(())
    }

    /// Set volume level (0.0 to 1.0).
    pub async fn set_volume(&self, level: f64) -> Result<()> {
        let request_id = self.next_request_id();
        let msg = CastMessage::set_volume(level.clamp(0.0, 1.0), request_id);
        send_message(&self.stream, &msg).await?;
        debug!(level, "Sent SET_VOLUME command");
        Ok(())
    }

    /// Disconnect from the Chromecast.
    pub async fn disconnect(&self) -> Result<()> {
        if self.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        self.heartbeat_cancel.cancel();
        if let Some(transport_id) = self.transport_id.lock().await.as_ref() {
            let close_msg = CastMessage::close(transport_id);
            let _ = send_message(&self.stream, &close_msg).await;
        }
        let close_receiver = CastMessage::close(proto::DEFAULT_RECEIVER_ID);
        let _ = send_message(&self.stream, &close_receiver).await;
        let task = self
            .heartbeat_task
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(mut task) = task {
            if tokio::time::timeout(Duration::from_secs(2), &mut task)
                .await
                .is_err()
            {
                task.abort();
            }
        }
        debug!(%self.address, "Disconnected from Chromecast");
        Ok(())
    }

    /// Send a media control command (PLAY, PAUSE, STOP).
    async fn send_media_command(&self, command: &str) -> Result<()> {
        let transport_id = self
            .transport_id
            .lock()
            .await
            .clone()
            .context("no transport ID")?;
        let session_id = self
            .media_session_id
            .lock()
            .await
            .context("no media session ID")?;
        let request_id = self.next_request_id();

        let msg = CastMessage::media_command(&transport_id, command, session_id, request_id);
        send_message(&self.stream, &msg).await?;
        debug!(command, "Sent media command to Chromecast");
        Ok(())
    }

    /// Wait for a RECEIVER_STATUS response containing a transport ID.
    async fn wait_for_transport_id(&self) -> Result<String> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);

        loop {
            if tokio::time::Instant::now() > deadline {
                anyhow::bail!("timed out waiting for Chromecast transport ID");
            }

            match tokio::time::timeout(Duration::from_secs(2), read_message(&self.stream)).await {
                Ok(Ok(msg)) => {
                    if let Some(ref payload) = msg.payload_utf8 {
                        // Respond to PINGs
                        if payload.contains("\"PING\"") {
                            let pong = CastMessage::pong();
                            let _ = send_message(&self.stream, &pong).await;
                            continue;
                        }

                        if msg.namespace == proto::namespace::RECEIVER
                            && payload.contains("RECEIVER_STATUS")
                        {
                            if let Some(tid) = extract_transport_id(payload) {
                                return Ok(tid);
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    warn!("Error reading Chromecast message: {}", e);
                }
                Err(_) => {
                    // Timeout on single read, continue waiting
                }
            }
        }
    }

    /// Wait for a MEDIA_STATUS response containing a media session ID.
    async fn wait_for_media_session_id(&self) -> Result<i32> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);

        loop {
            if tokio::time::Instant::now() > deadline {
                anyhow::bail!("timed out waiting for media session ID");
            }

            match tokio::time::timeout(Duration::from_secs(2), read_message(&self.stream)).await {
                Ok(Ok(msg)) => {
                    if let Some(ref payload) = msg.payload_utf8 {
                        if payload.contains("\"PING\"") {
                            let pong = CastMessage::pong();
                            let _ = send_message(&self.stream, &pong).await;
                            continue;
                        }

                        if msg.namespace == proto::namespace::MEDIA
                            && payload.contains("MEDIA_STATUS")
                        {
                            if let Some(sid) = extract_media_session_id(payload) {
                                return Ok(sid);
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    warn!("Error reading Chromecast message: {}", e);
                }
                Err(_) => {
                    // Timeout on single read, continue
                }
            }
        }
    }
}

impl Drop for ChromecastClient {
    fn drop(&mut self) {
        self.heartbeat_cancel.cancel();
        if let Some(task) = self
            .heartbeat_task
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        {
            task.abort();
        }
    }
}

/// Send a `CastMessage` over the TLS stream with length-prefix framing.
async fn send_message(
    stream: &Arc<Mutex<TlsStream<TcpStream>>>,
    message: &CastMessage,
) -> Result<()> {
    let mut buf = Vec::new();
    message
        .encode(&mut buf)
        .context("failed to encode CastMessage")?;

    let len = buf.len() as u32;
    let mut stream = stream.lock().await;
    tokio::time::timeout(Duration::from_secs(5), async {
        stream
            .write_all(&len.to_be_bytes())
            .await
            .context("failed to write message length")?;
        stream
            .write_all(&buf)
            .await
            .context("failed to write message body")?;
        stream
            .flush()
            .await
            .context("failed to flush Castv2 message")
    })
    .await
    .context("Castv2 write timed out")??;
    trace!(len, "Sent Castv2 message ({} bytes)", len);
    Ok(())
}

/// Read a single `CastMessage` from the TLS stream.
async fn read_message(stream: &Arc<Mutex<TlsStream<TcpStream>>>) -> Result<CastMessage> {
    let mut stream = stream.lock().await;

    // Read the 4-byte big-endian length prefix.
    let mut len_buf = [0u8; 4];
    tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut len_buf))
        .await
        .context("Castv2 message-length read timed out")?
        .context("failed to read message length")?;
    let len = u32::from_be_bytes(len_buf) as usize;

    anyhow::ensure!(
        len <= 256 * 1024,
        "Castv2 message too large ({} bytes)",
        len
    );

    let mut msg_buf = vec![0u8; len];
    tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut msg_buf))
        .await
        .context("Castv2 message-body read timed out")?
        .context("failed to read message body")?;

    CastMessage::decode(&msg_buf[..]).context("failed to decode CastMessage")
}

/// Run the heartbeat loop, sending PINGs every 5 seconds.
async fn heartbeat_loop(
    stream: Arc<Mutex<TlsStream<TcpStream>>>,
    cancel: tokio_util::sync::CancellationToken,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                debug!("Chromecast heartbeat loop stopped");
                return;
            }
            _ = interval.tick() => {
                let ping = CastMessage::ping();
                if let Err(e) = send_message(&stream, &ping).await {
                    error!("Failed to send heartbeat PING: {}", e);
                    return;
                }
                trace!("Sent heartbeat PING to Chromecast");
            }
        }
    }
}

/// Extract the transport ID from a RECEIVER_STATUS JSON payload.
fn extract_transport_id(payload: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    let status = value.get("status")?;
    let applications = status.get("applications")?.as_array()?;
    let app = applications.first()?;
    app.get("transportId")?.as_str().map(|s| s.to_string())
}

/// Extract the media session ID from a MEDIA_STATUS JSON payload.
fn extract_media_session_id(payload: &str) -> Option<i32> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    let status_list = value.get("status")?.as_array()?;
    let status = status_list.first()?;
    status.get("mediaSessionId")?.as_i64().map(|id| id as i32)
}

/// A TLS certificate verifier that accepts any certificate.
/// Chromecast devices use self-signed certificates.
#[derive(Debug)]
struct NoCertVerifier;

impl rustls::client::danger::ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_transport_id_from_status() {
        let payload = r#"{
            "type": "RECEIVER_STATUS",
            "requestId": 2,
            "status": {
                "applications": [
                    {
                        "appId": "CC1AD845",
                        "displayName": "Default Media Receiver",
                        "transportId": "web-5",
                        "sessionId": "abc-123"
                    }
                ]
            }
        }"#;
        assert_eq!(extract_transport_id(payload), Some("web-5".to_string()));
    }

    #[test]
    fn extract_transport_id_missing_apps() {
        let payload = r#"{"type":"RECEIVER_STATUS","status":{}}"#;
        assert_eq!(extract_transport_id(payload), None);
    }

    #[test]
    fn extract_media_session_from_status() {
        let payload = r#"{
            "type": "MEDIA_STATUS",
            "status": [
                {
                    "mediaSessionId": 42,
                    "playerState": "PLAYING"
                }
            ]
        }"#;
        assert_eq!(extract_media_session_id(payload), Some(42));
    }

    #[test]
    fn extract_media_session_empty_status() {
        let payload = r#"{"type":"MEDIA_STATUS","status":[]}"#;
        assert_eq!(extract_media_session_id(payload), None);
    }
}

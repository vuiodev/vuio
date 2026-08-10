//! AirPlay URL-video discovery, pairing, and control.

mod audio;
mod credentials;
mod pair_verify;
mod raop;
mod transient;
mod transport;

use super::{
    is_safe_renderer_address, CastProvider, PairingChallenge, PairingStatus, PlaybackAction,
    PlaybackItem, PlaybackState, PlaybackStatus, RendererCapabilities, RendererDevice,
    RendererEndpoint, RendererProtocol,
};
use anyhow::Context as _;
use async_trait::async_trait;
use futures_util::StreamExt;
use hap_crypto::{PairSetupClient, PairSetupStep, SessionKeys};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use quick_xml::{events::Event, Reader};
use std::{collections::HashMap, net::SocketAddr, time::Duration};
use tokio::sync::Mutex;

use self::{
    credentials::CredentialStore,
    pair_verify::{derive_key_from, PairVerifier},
    transport::AirplayConnection,
};

const SERVICE_TYPE: &str = "_airplay._tcp.local.";
const MAX_RESPONSE_BYTES: usize = 256 * 1024;
const PAIRING_TTL: Duration = Duration::from_secs(120);
const AIRPLAY_VIDEO_V1: u64 = 1 << 0;
const AIRPLAY_VIDEO_V2: u64 = 1 << 49;
const AIRPLAY_AUDIO: u64 = 1 << 9;
const AIRPLAY_SYSTEM_PAIRING: u64 = 1 << 43;
const AIRPLAY_CORE_UTILS_PAIRING: u64 = 1 << 48;
const AIRPLAY_2_ENABLED: bool = true;
const PLAY_RETRIES: u32 = 3;
const PLAYBACK_INFO_ATTEMPTS: u32 = 5;
const EVENT_CHANNEL_ATTEMPTS: u32 = 5;
const USER_AGENT: &str = "AirPlay/550.10";
const BINARY_PLIST: &str = "application/x-apple-binary-plist";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const FEEDBACK_INTERVAL: Duration = Duration::from_secs(2);

struct PendingPairing {
    renderer_id: String,
    connection: AirplayConnection,
    first_response: Vec<u8>,
    expires_at: tokio::time::Instant,
}

struct SecureSession {
    control: std::sync::Arc<Mutex<SecureControl>>,
    event_task: tokio::task::JoinHandle<()>,
    feedback_task: tokio::task::JoinHandle<()>,
    timing_task: tokio::task::JoinHandle<()>,
    /// RTP sender and clock-sync tasks, present only for audio sessions.
    audio_tasks: Vec<tokio::task::JoinHandle<()>>,
    /// Tracks queued behind the one playing, drained by the RTP sender.
    audio_queue: Option<AudioQueue>,
    /// RTSP session URI, needed to TEARDOWN so the receiver stops rendering
    /// instead of playing out whatever it has buffered.
    rtsp_session: String,
}

/// Files waiting to be streamed on an open audio session.
type AudioQueue = std::sync::Arc<Mutex<std::collections::VecDeque<std::path::PathBuf>>>;

/// Identifiers a receiver expects to stay constant for the life of a session.
/// pyatv randomises them once per `RtspSession` and repeats them on every request.
struct SessionHeaders {
    /// Lowercase UUID sent as `X-Apple-Session-ID` on the HTTP `/play` request.
    session_id: String,
    dacp_id: String,
    active_remote: String,
}

impl SessionHeaders {
    fn new() -> Self {
        let bytes = uuid::Uuid::new_v4().into_bytes();
        let dacp_id = u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        let active_remote =
            u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]).saturating_add(1);
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            dacp_id: format!("{dacp_id:X}"),
            active_remote: active_remote.to_string(),
        }
    }
}

struct SecureControl {
    connection: AirplayConnection,
    headers: SessionHeaders,
    cseq: u32,
}

impl SecureControl {
    fn new(connection: AirplayConnection) -> Self {
        Self {
            connection,
            headers: SessionHeaders::new(),
            cseq: 0,
        }
    }

    /// `SETUP`, `RECORD`, `/feedback`, `/setProperty` and `/rate` travel as RTSP,
    /// mirroring pyatv's `RtspSession.exchange`.
    async fn rtsp(
        &mut self,
        method: &str,
        path: &str,
        content_type: Option<&str>,
        body: &[u8],
    ) -> anyhow::Result<transport::Response> {
        let sequence = self.cseq;
        self.cseq = self.cseq.saturating_add(1);
        let mut headers = vec![
            ("User-Agent", USER_AGENT.to_string()),
            ("CSeq", sequence.to_string()),
            ("DACP-ID", self.headers.dacp_id.clone()),
            ("Active-Remote", self.headers.active_remote.clone()),
            ("Client-Instance", self.headers.dacp_id.clone()),
        ];
        if let Some(content_type) = content_type {
            headers.push(("Content-Type", content_type.to_string()));
        }
        self.request("RTSP/1.0", method, path, &headers, body).await
    }

    /// RTSP with additional headers, for the few requests that need them.
    async fn rtsp_with(
        &mut self,
        method: &str,
        path: &str,
        content_type: Option<&str>,
        extra_headers: &[(&'static str, String)],
        body: &[u8],
    ) -> anyhow::Result<transport::Response> {
        let sequence = self.cseq;
        self.cseq = self.cseq.saturating_add(1);
        let mut headers = vec![
            ("User-Agent", USER_AGENT.to_string()),
            ("CSeq", sequence.to_string()),
            ("DACP-ID", self.headers.dacp_id.clone()),
            ("Active-Remote", self.headers.active_remote.clone()),
            ("Client-Instance", self.headers.dacp_id.clone()),
        ];
        if let Some(content_type) = content_type {
            headers.push(("Content-Type", content_type.to_string()));
        }
        headers.extend(extra_headers.iter().cloned());
        self.request("RTSP/1.0", method, path, &headers, body).await
    }

    /// `/play`, `/playback-info` and `/stop` are plain HTTP even inside an RTSP
    /// session: pyatv sends them through `RtspSession.connection`, not `exchange`.
    /// Receivers that dispatch on the protocol line reject them as RTSP.
    async fn http(
        &mut self,
        method: &str,
        path: &str,
        extra_headers: &[(&'static str, String)],
        body: &[u8],
    ) -> anyhow::Result<transport::Response> {
        let mut headers = vec![("User-Agent", USER_AGENT.to_string())];
        headers.extend(extra_headers.iter().cloned());
        self.request("HTTP/1.1", method, path, &headers, body).await
    }

    async fn request(
        &mut self,
        protocol: &str,
        method: &str,
        path: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> anyhow::Result<transport::Response> {
        tracing::trace!(
            request = %format!("{method} {path} {protocol}"),
            headers = %headers
                .iter()
                .map(|(name, value)| format!("{name}: {value}"))
                .collect::<Vec<_>>()
                .join(" | "),
            body = %describe_body(body),
            "AirPlay request"
        );
        let response = tokio::time::timeout(
            REQUEST_TIMEOUT,
            self.connection
                .request_while_serving_events(method, path, protocol, headers, body),
        )
        .await
        .with_context(|| format!("AirPlay {method} {path} timed out"))??;
        tracing::debug!(
            request = %format!("{method} {path} {protocol}"),
            status = response.status,
            body = %describe_body(&response.body),
            "AirPlay response"
        );
        Ok(response)
    }
}

impl Drop for SecureSession {
    fn drop(&mut self) {
        self.event_task.abort();
        self.feedback_task.abort();
        self.timing_task.abort();
        for task in &self.audio_tasks {
            task.abort();
        }
    }
}

enum ActiveSession {
    Legacy(String),
    Secure(Box<SecureSession>),
}

pub struct AirplayProvider {
    /// Feature bits seen during discovery, keyed by renderer id. They decide
    /// which handshake and which playback command set a receiver expects.
    features: Mutex<HashMap<String, u64>>,
    sessions: Mutex<HashMap<String, ActiveSession>>,
    pending_pairings: Mutex<HashMap<String, PendingPairing>>,
    credentials: CredentialStore,
}

impl AirplayProvider {
    pub fn new() -> Self {
        Self::with_credentials(CredentialStore::memory())
    }

    pub async fn persistent(
        secrets: std::sync::Arc<dyn crate::database::SecretStore>,
    ) -> anyhow::Result<Self> {
        Ok(Self::with_credentials(
            CredentialStore::load(secrets).await?,
        ))
    }

    fn with_credentials(credentials: CredentialStore) -> Self {
        Self {
            features: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            pending_pairings: Mutex::new(HashMap::new()),
            credentials,
        }
    }

    async fn session_for_play(&self, device: &RendererDevice) -> String {
        let mut sessions = self.sessions.lock().await;
        if sessions.len() >= crate::runtime_state::ACTIVE_CAST_MAX_ENTRIES
            && !sessions.contains_key(&device.id)
        {
            if let Some(oldest) = sessions.keys().next().cloned() {
                sessions.remove(&oldest);
            }
        }
        match sessions
            .entry(device.id.clone())
            .or_insert_with(|| ActiveSession::Legacy(uuid::Uuid::new_v4().to_string()))
        {
            ActiveSession::Legacy(session) => session.clone(),
            ActiveSession::Secure(_) => uuid::Uuid::new_v4().to_string(),
        }
    }

    async fn active_session(&self, device: &RendererDevice) -> anyhow::Result<String> {
        self.sessions
            .lock()
            .await
            .get(&device.id)
            .and_then(|session| match session {
                ActiveSession::Legacy(id) => Some(id.clone()),
                ActiveSession::Secure(_) => None,
            })
            .ok_or_else(|| anyhow::anyhow!("no active AirPlay session for this renderer"))
    }

    async fn request(
        &self,
        device: &RendererDevice,
        method: reqwest::Method,
        path: &str,
        body: Option<String>,
        session: &str,
    ) -> anyhow::Result<String> {
        let address = socket_endpoint(device)?;
        let url = reqwest::Url::parse(&format!("http://{address}{path}"))?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let mut request = client
            .request(method, url)
            .header("User-Agent", "MediaControl/1.0")
            .header("X-Apple-Session-ID", session);
        if let Some(body) = body {
            request = request.header("Content-Type", "text/parameters").body(body);
        }
        let response = request.send().await?;
        anyhow::ensure!(
            response.status().is_success(),
            "AirPlay request failed with HTTP {}",
            response.status()
        );
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            anyhow::bail!("AirPlay response exceeded {MAX_RESPONSE_BYTES} bytes");
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            anyhow::ensure!(
                bytes.len().saturating_add(chunk.len()) <= MAX_RESPONSE_BYTES,
                "AirPlay response exceeded {MAX_RESPONSE_BYTES} bytes"
            );
            bytes.extend_from_slice(&chunk);
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    async fn pairing_request(
        connection: &mut AirplayConnection,
        path: &str,
        body: &[u8],
    ) -> anyhow::Result<Vec<u8>> {
        let response = connection
            .request(
                "POST",
                path,
                "HTTP/1.1",
                &[
                    ("User-Agent", "AirPlay/550.10".to_string()),
                    ("Connection", "keep-alive".to_string()),
                    ("X-Apple-HKP", "3".to_string()),
                    ("Content-Type", "application/octet-stream".to_string()),
                ],
                body,
            )
            .await?;
        anyhow::ensure!(
            (200..300).contains(&response.status),
            "AirPlay pairing request {path} failed with status {}",
            response.status
        );
        Ok(response.body)
    }

    async fn verified_connection(
        &self,
        device: &RendererDevice,
    ) -> anyhow::Result<(AirplayConnection, [u8; 32])> {
        let (controller, accessory) = self
            .credentials
            .pairing(&device.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("AirPlay pairing is required for this receiver"))?;
        let mut connection = AirplayConnection::connect(socket_endpoint(device)?).await?;
        let mut verify = PairVerifier::new(controller, accessory);
        let first = Self::pairing_request(&mut connection, "/pair-verify", &verify.start()).await?;
        let second_request = verify.handle_m2(&first)?;
        let second =
            Self::pairing_request(&mut connection, "/pair-verify", &second_request).await?;
        let verified = verify.finish(&second)?;
        connection.secure(verified.keys);
        Ok((connection, verified.shared_secret))
    }

    /// Establish a session with no stored pairing at all.
    ///
    /// Receivers advertising `SupportsSystemPairing` or
    /// `SupportsCoreUtilsPairingAndEncryption` accept transient Pair Setup:
    /// SRP M1-M4 against the fixed code 3939, after which the SRP session key
    /// is the shared secret. This is the handshake iOS uses with third-party
    /// AirPlay 2 sets, and it needs no PIN and nothing persisted.
    async fn transient_connection(
        &self,
        address: SocketAddr,
    ) -> anyhow::Result<(AirplayConnection, Vec<u8>)> {
        let mut connection = AirplayConnection::connect(address).await?;
        let mut pairing = transient::TransientPairing::new()?;
        let first = Self::transient_request(&mut connection, &pairing.start()).await?;
        let second_request = pairing.handle_m2(&first)?;
        let second = Self::transient_request(&mut connection, &second_request).await?;
        let shared = pairing.finish(&second)?;
        connection.secure(SessionKeys {
            read_key: derive_key_from(&shared, b"Control-Salt", b"Control-Read-Encryption-Key")?,
            write_key: derive_key_from(&shared, b"Control-Salt", b"Control-Write-Encryption-Key")?,
        });
        tracing::debug!("AirPlay transient session established");
        Ok((connection, shared))
    }

    /// Transient pairing is announced with `X-Apple-HKP: 4`; regular HAP
    /// pairing uses 3.
    async fn transient_request(
        connection: &mut AirplayConnection,
        body: &[u8],
    ) -> anyhow::Result<Vec<u8>> {
        let response = connection
            .request(
                "POST",
                "/pair-setup",
                "HTTP/1.1",
                &[
                    ("User-Agent", "AirPlay/550.10".to_string()),
                    ("Connection", "keep-alive".to_string()),
                    ("X-Apple-HKP", "4".to_string()),
                    ("Content-Type", "application/octet-stream".to_string()),
                ],
                body,
            )
            .await?;
        anyhow::ensure!(
            (200..300).contains(&response.status),
            "AirPlay transient pairing failed with status {}{}",
            response.status,
            describe_body(&response.body)
        );
        Ok(response.body)
    }

    /// Stream a decoded audio file to the receiver over RTP.
    ///
    /// This is the only media path a video-less AirPlay 2 receiver exposes, and
    /// it is also how audio reaches receivers that do support video.
    async fn audio_play(
        &self,
        device: &RendererDevice,
        item: &PlaybackItem,
        connection: AirplayConnection,
        shared_secret: Vec<u8>,
    ) -> anyhow::Result<()> {
        let address = socket_endpoint(device)?;
        let path = std::path::PathBuf::from(&item.local_path);
        let source = tokio::task::spawn_blocking(move || audio::PcmSource::open(&path))
            .await
            .context("joining the AirPlay audio decoder")??;

        let mut control = SecureControl::new(connection);
        let session_bytes = uuid::Uuid::new_v4().into_bytes();
        let session_id = u32::from_be_bytes([
            session_bytes[0],
            session_bytes[1],
            session_bytes[2],
            session_bytes[3],
        ]);
        let rtsp_session = format!(
            "rtsp://{}/{}",
            control.connection.local_addr()?.ip(),
            session_id
        );

        // The device-level SETUP must land before any stream can be allocated.
        let device_id = controller_device_id();
        let timing_socket = tokio::net::UdpSocket::bind(match address.ip() {
            std::net::IpAddr::V4(_) => "0.0.0.0:0",
            std::net::IpAddr::V6(_) => "[::]:0",
        })
        .await?;
        let timing_port = i64::from(timing_socket.local_addr()?.port());
        // A receiver requires `GET /info` before it will accept SETUP.
        match control.rtsp("GET", "/info", None, &[]).await {
            Ok(info) => tracing::debug!(status = info.status, "AirPlay audio /info"),
            Err(error) => tracing::debug!(%error, "AirPlay audio /info failed"),
        }

        let setup_uuid = uuid::Uuid::new_v4().to_string().to_uppercase();
        let setup = binary_plist(setup_parameters(&device_id, &setup_uuid, timing_port))?;
        let response = control
            .rtsp("SETUP", &rtsp_session, Some(BINARY_PLIST), &setup)
            .await?;
        anyhow::ensure!(
            (200..300).contains(&response.status),
            "AirPlay audio SETUP failed with status {}{}",
            response.status,
            describe_body(&response.body)
        );
        let mut timing_task = AbortOnDrop(Some(tokio::spawn(run_timing_server(timing_socket))));

        // The receiver expects the reverse event channel to be connected before
        // RECORD; leaving it idle makes third-party sets stall the request.
        let event_port = plist_port(&response.body, "eventPort")
            .context("AirPlay audio SETUP did not return an event port")?;
        let mut event_connection =
            connect_event_channel(SocketAddr::new(address.ip(), event_port)).await?;
        event_connection.secure(SessionKeys {
            write_key: derive_key_from(
                &shared_secret,
                b"Events-Salt",
                b"Events-Read-Encryption-Key",
            )?,
            read_key: derive_key_from(
                &shared_secret,
                b"Events-Salt",
                b"Events-Write-Encryption-Key",
            )?,
        });
        let (event_sender, _event_replies) = tokio::sync::mpsc::unbounded_channel();
        let mut event_task = AbortOnDrop(Some(tokio::spawn(async move {
            if let Err(error) = event_connection.serve_events(event_sender).await {
                tracing::debug!(%error, "AirPlay audio event channel closed");
            }
        })));

        // RECORD goes after the session SETUP and before the stream SETUP; the
        // reverse order yields RECORD=500 / FLUSH=455.
        let record = control.rtsp("RECORD", &rtsp_session, None, &[]).await?;
        if !(200..300).contains(&record.status) {
            tracing::warn!(status = record.status, "AirPlay audio RECORD was refused");
        }

        let key = raop::stream_key(&shared_secret)?;
        if std::env::var("VUIO_AIRPLAY_PROBE_FORMATS")
            .is_ok_and(|value| matches!(value.trim(), "1" | "true" | "yes"))
        {
            probe_audio_formats(&mut control, &rtsp_session, &key, session_id).await;
        }
        let stream = binary_plist(audio_stream_parameters(&key, session_id))?;
        let response = control
            .rtsp("SETUP", &rtsp_session, Some(BINARY_PLIST), &stream)
            .await?;
        anyhow::ensure!(
            (200..300).contains(&response.status),
            "AirPlay audio stream SETUP failed with status {}{}",
            response.status,
            describe_body(&response.body)
        );
        let (data_port, control_port) = parse_audio_ports(&response.body)
            .context("AirPlay audio SETUP did not return stream ports")?;
        let receiver_session = response.headers.get("session").cloned();
        tracing::info!(data_port, control_port, "AirPlay audio stream allocated");

        let rtptime = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let (mut sender, control_socket, control_target) = raop::AudioSender::connect(
            address,
            data_port,
            control_port,
            key,
            session_id,
            rtptime.clone(),
        )
        .await?;
        // FLUSH anchors the stream: it tells the receiver which sequence number
        // and RTP timestamp the audio about to arrive starts from. Without it a
        // receiver has nothing to align against and renders noise, then keeps
        // playing whatever it buffered.
        let mut flush_headers = vec![
            ("Range", "npt=0-".to_string()),
            (
                "RTP-Info",
                format!(
                    "seq={};rtptime={}",
                    sender.start_sequence(),
                    sender.start_rtptime()
                ),
            ),
        ];
        if let Some(session) = receiver_session.clone() {
            flush_headers.push(("Session", session));
        }
        // FLUSH declares where the stream restarts: it tells the receiver to
        // drop anything buffered and expect audio from this seq/rtptime. A
        // fresh session has nothing to discard, so it is optional at start, but
        // it is the mechanism a seek or track skip needs.
        let flush = control
            .rtsp_with("FLUSH", &rtsp_session, None, &flush_headers, &[])
            .await?;
        if !(200..300).contains(&flush.status) {
            tracing::warn!(status = flush.status, "AirPlay audio FLUSH was refused");
        } else {
            tracing::debug!(status = flush.status, "AirPlay audio stream anchored");
        }

        // Only now start announcing the clock, and seed it with the position
        // the first packet will carry -- a zero here makes the very first sync
        // packet claim a stream position of `0 - latency`, which wraps.
        rtptime.store(sender.start_rtptime(), std::sync::atomic::Ordering::Relaxed);
        let mut sync_task = AbortOnDrop(Some(raop::spawn_sync_task(
            control_socket,
            control_target,
            rtptime,
        )));

        let control = std::sync::Arc::new(Mutex::new(control));
        let mut feedback_task = AbortOnDrop(Some(spawn_feedback_task(control.clone())));

        let queue: AudioQueue = std::sync::Arc::new(Mutex::new(std::collections::VecDeque::new()));
        let stream_queue = queue.clone();
        let stream_control = control.clone();
        let stream_rtsp_session = rtsp_session.clone();
        let stream_session = receiver_session.clone();
        let stream_task = tokio::spawn(async move {
            if let Err(error) = sender
                .stream(
                    source,
                    stream_queue,
                    stream_control,
                    stream_rtsp_session,
                    stream_session,
                )
                .await
            {
                tracing::warn!(%error, "AirPlay audio streaming stopped");
            }
        });

        self.sessions.lock().await.insert(
            device.id.clone(),
            ActiveSession::Secure(Box::new(SecureSession {
                control,
                event_task: event_task.take(),
                feedback_task: feedback_task.take(),
                timing_task: timing_task.take(),
                audio_tasks: vec![stream_task, sync_task.take()],
                audio_queue: Some(queue),
                rtsp_session: rtsp_session.clone(),
            })),
        );
        Ok(())
    }

    async fn secure_play(
        &self,
        device: &RendererDevice,
        item: &PlaybackItem,
    ) -> anyhow::Result<()> {
        let address = socket_endpoint(device)?;
        let (connection, shared_secret) = self.verified_connection(device).await?;
        self.start_secure_session(
            &device.id,
            address,
            connection,
            shared_secret.to_vec(),
            &item.url,
        )
        .await
    }

    /// Establish an AirPlay 2 media session and start playback.
    ///
    /// This is a port of pyatv's `AirPlayV2.play_url`: `SETUP`, encrypted event
    /// channel, NTP timing server, feedback loop, `RECORD`, then `POST /play`
    /// carrying the media URL untouched. Taking the receiver address explicitly
    /// (rather than re-deriving it from `device`) keeps the sequence testable
    /// against a loopback receiver.
    async fn start_secure_session(
        &self,
        renderer_id: &str,
        address: SocketAddr,
        connection: AirplayConnection,
        shared_secret: Vec<u8>,
        media_url: &str,
    ) -> anyhow::Result<()> {
        let mut control = SecureControl::new(connection);
        let setup_uuid = uuid::Uuid::new_v4().to_string().to_uppercase();
        let media_id = uuid::Uuid::new_v4().to_string();
        let device_id = controller_device_id();

        let bind_address = match address.ip() {
            std::net::IpAddr::V4(_) => "0.0.0.0:0",
            std::net::IpAddr::V6(_) => "[::]:0",
        };
        let timing_socket = tokio::net::UdpSocket::bind(bind_address).await?;
        let timing_port = i64::from(timing_socket.local_addr()?.port());

        let rtsp_session_bytes = uuid::Uuid::new_v4().into_bytes();
        let rtsp_session_id = u32::from_be_bytes([
            rtsp_session_bytes[0],
            rtsp_session_bytes[1],
            rtsp_session_bytes[2],
            rtsp_session_bytes[3],
        ]);
        let rtsp_session = format!(
            "rtsp://{}/{}",
            control.connection.local_addr()?.ip(),
            rtsp_session_id
        );

        // Diagnostic only: `/info` reports which endpoints and features the
        // receiver actually implements, which is the fastest way to tell a
        // legacy `/play` receiver apart from a play-queue-only one. pyatv sends
        // it with `allow_error=True`, so a failure here is never fatal.
        match control.rtsp("GET", "/info", None, &[]).await {
            Ok(info) => tracing::info!(
                status = info.status,
                info = %describe_body(&info.body),
                "AirPlay receiver /info"
            ),
            Err(error) => tracing::debug!(%error, "AirPlay receiver did not answer /info"),
        }

        let setup_body = binary_plist(setup_parameters(&device_id, &setup_uuid, timing_port))?;
        let setup_response = control
            .rtsp("SETUP", &rtsp_session, Some(BINARY_PLIST), &setup_body)
            .await?;
        anyhow::ensure!(
            (200..300).contains(&setup_response.status),
            "AirPlay 2 SETUP failed with status {}{}",
            setup_response.status,
            describe_body(&setup_response.body)
        );
        let event_port = plist::Value::from_reader(std::io::Cursor::new(&setup_response.body))
            .ok()
            .and_then(|value| value.into_dictionary())
            .and_then(|dictionary| {
                dictionary
                    .get("eventPort")
                    .and_then(plist::Value::as_unsigned_integer)
            })
            .and_then(|port| u16::try_from(port).ok())
            .context("AirPlay 2 SETUP did not return a valid event port")?;

        let event_address = SocketAddr::new(address.ip(), event_port);
        let mut event_connection = connect_event_channel(event_address).await?;
        // Sender write uses the receiver's "Read" label and vice versa (HAP event channel).
        event_connection.secure(SessionKeys {
            write_key: derive_key_from(
                &shared_secret,
                b"Events-Salt",
                b"Events-Read-Encryption-Key",
            )?,
            read_key: derive_key_from(
                &shared_secret,
                b"Events-Salt",
                b"Events-Write-Encryption-Key",
            )?,
        });
        // Serve events and timing before RECORD/play. Leaving the reverse event
        // TCP idle during session setup can stall third-party receivers.
        let (event_reply_sender, mut event_replies) = tokio::sync::mpsc::unbounded_channel();
        let mut event_task = AbortOnDrop(Some(tokio::spawn(async move {
            if let Err(error) = event_connection.serve_events(event_reply_sender).await {
                tracing::debug!(%error, "AirPlay event channel closed");
            }
        })));
        let mut timing_task = AbortOnDrop(Some(tokio::spawn(run_timing_server(timing_socket))));
        tokio::spawn(async move {
            while let Some((sequence, body)) = event_replies.recv().await {
                tracing::debug!(
                    sequence,
                    body = %describe_body(&body),
                    "AirPlay event data-stream reply"
                );
            }
        });

        let control = std::sync::Arc::new(Mutex::new(control));
        // pyatv starts feedback before RECORD; some receivers drop a session that
        // stays silent between SETUP and playback.
        let mut feedback_task = AbortOnDrop(Some(spawn_feedback_task(control.clone())));

        let record_response = control
            .lock()
            .await
            .rtsp("RECORD", &rtsp_session, None, &[])
            .await?;
        if !(200..300).contains(&record_response.status) {
            // Some third-party stacks accept the session even when RECORD is odd.
            tracing::warn!(
                status = record_response.status,
                "AirPlay 2 RECORD returned non-success; continuing"
            );
        }

        let session_id = control.lock().await.headers.session_id.clone();
        let play_headers = [
            ("Content-Type", BINARY_PLIST.to_string()),
            ("X-Apple-ProtocolVersion", "1".to_string()),
            ("X-Apple-Session-ID", session_id),
            ("X-Apple-Stream-ID", "1".to_string()),
        ];
        let play_body = binary_plist(play_parameters(media_url, &media_id, &device_id))?;
        let mut play_response = None;
        for attempt in 1..=PLAY_RETRIES {
            let response = control
                .lock()
                .await
                .http("POST", "/play", &play_headers, &play_body)
                .await?;
            // Receivers routinely answer the first attempt with a 500 while they
            // are still tearing down a previous session.
            if response.status == 500 && attempt < PLAY_RETRIES {
                tracing::debug!(
                    attempt,
                    retries = PLAY_RETRIES,
                    "AirPlay 2 POST /play returned 500; retrying"
                );
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            play_response = Some(response);
            break;
        }
        let play_response =
            play_response.context("AirPlay 2 POST /play exhausted its retries with no response")?;
        if play_response.status == 404 {
            // /play is the AirPlay-video-v1 endpoint. Receivers that do not
            // advertise SupportsAirPlayVideoV1 (bit 0) simply do not have it --
            // they expect the play-queue command set instead.
            tracing::warn!(
                "AirPlay 2 receiver has no /play endpoint; probing its playback surface"
            );
            probe_playback_surface(&control).await;
            if let Err(error) =
                probe_audio_stream(&control, &rtsp_session, &shared_secret, rtsp_session_id).await
            {
                tracing::info!(%error, "AirPlay audio-stream probe stopped");
            }
            anyhow::bail!(
                "this AirPlay receiver has no video endpoint (HTTP 404 on /play); it is an \
                 AirPlay audio and mirroring receiver only. Cast video to it over Chromecast \
                 or DLNA instead"
            );
        }
        anyhow::ensure!(
            (200..300).contains(&play_response.status),
            "AirPlay 2 POST /play was rejected with status {}{}",
            play_response.status,
            describe_body(&play_response.body)
        );
        tracing::info!(url = %media_url, "AirPlay 2 POST /play accepted");

        // pyatv's order matters here: /rate is what actually starts playback, and
        // the end-time properties are only accepted once a rate has been set.
        let interested = binary_plist(property_value(plist::Value::Boolean(true)))?;
        let action_at_end = binary_plist(property_value(plist::Value::Integer(0.into())))?;
        let end_time = binary_plist(property_value(plist::Value::Dictionary({
            let mut value = plist::Dictionary::new();
            value.insert("flags".into(), plist::Value::Integer(0.into()));
            value.insert("value".into(), plist::Value::Integer(0.into()));
            value.insert("epoch".into(), plist::Value::Integer(0.into()));
            value.insert("timescale".into(), plist::Value::Integer(0.into()));
            value
        })))?;
        for (method, path, body) in [
            (
                "PUT",
                "/setProperty?isInterestedInDateRange",
                interested.as_slice(),
            ),
            (
                "PUT",
                "/setProperty?actionAtItemEnd",
                action_at_end.as_slice(),
            ),
            ("POST", "/rate?value=1.000000", [].as_slice()),
            ("PUT", "/setProperty?forwardEndTime", end_time.as_slice()),
            ("PUT", "/setProperty?reverseEndTime", end_time.as_slice()),
        ] {
            let content_type = (!body.is_empty()).then_some(BINARY_PLIST);
            let response = control
                .lock()
                .await
                .rtsp(method, path, content_type, body)
                .await?;
            if !(200..300).contains(&response.status) {
                tracing::debug!(path, status = response.status, "AirPlay 2 command skipped");
            }
        }

        soft_check_playback_info(&control).await?;

        let mut sessions = self.sessions.lock().await;
        if sessions.len() >= crate::runtime_state::ACTIVE_CAST_MAX_ENTRIES
            && !sessions.contains_key(renderer_id)
        {
            if let Some(oldest) = sessions.keys().next().cloned() {
                sessions.remove(&oldest);
            }
        }
        sessions.insert(
            renderer_id.to_string(),
            ActiveSession::Secure(Box::new(SecureSession {
                control,
                event_task: event_task.take(),
                feedback_task: feedback_task.take(),
                timing_task: timing_task.take(),
                audio_tasks: Vec::new(),
                audio_queue: None,
                rtsp_session: rtsp_session.clone(),
            })),
        );
        Ok(())
    }
}

fn controller_device_id() -> String {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    bytes[..6]
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// The `SETUP` payload pyatv sends for an AirPlay 2 media session.
fn setup_parameters(device_id: &str, session_uuid: &str, timing_port: i64) -> plist::Value {
    let mut setup = plist::Dictionary::new();
    setup.insert(
        "deviceID".into(),
        plist::Value::String(device_id.to_string()),
    );
    setup.insert(
        "sessionUUID".into(),
        plist::Value::String(session_uuid.to_string()),
    );
    setup.insert(
        "timingPort".into(),
        plist::Value::Integer(timing_port.into()),
    );
    setup.insert("timingProtocol".into(), plist::Value::String("NTP".into()));
    setup.insert("isMultiSelectAirPlay".into(), plist::Value::Boolean(true));
    setup.insert(
        "groupContainsGroupLeader".into(),
        plist::Value::Boolean(false),
    );
    setup.insert("senderSupportsRelay".into(), plist::Value::Boolean(false));
    setup.insert(
        "statsCollectionEnabled".into(),
        plist::Value::Boolean(false),
    );
    setup.insert(
        "macAddress".into(),
        plist::Value::String(device_id.to_string()),
    );
    setup.insert("name".into(), plist::Value::String("VuIO".into()));
    setup.insert("model".into(), plist::Value::String("iPhone14,3".into()));
    setup.insert(
        "osBuildVersion".into(),
        plist::Value::String("20F66".into()),
    );
    setup.insert("osName".into(), plist::Value::String("iPhone OS".into()));
    setup.insert("osVersion".into(), plist::Value::String("16.5".into()));
    setup.insert(
        "sourceVersion".into(),
        plist::Value::String("690.7.1".into()),
    );
    plist::Value::Dictionary(setup)
}

/// The `POST /play` payload pyatv sends. `Content-Location` is the media URL
/// exactly as advertised -- receivers fetch it themselves over plain HTTP, so
/// rewriting it (for example into a synthesised HLS playlist) only breaks them.
fn play_parameters(media_url: &str, media_id: &str, device_id: &str) -> plist::Value {
    let mut play = plist::Dictionary::new();
    play.insert(
        "Content-Location".into(),
        plist::Value::String(media_url.to_string()),
    );
    play.insert("Start-Position-Seconds".into(), plist::Value::Real(0.0));
    play.insert("uuid".into(), plist::Value::String(media_id.to_string()));
    play.insert("streamType".into(), plist::Value::Integer(1.into()));
    play.insert("mediaType".into(), plist::Value::String("file".into()));
    play.insert(
        "mightSupportStorePastisKeyRequests".into(),
        plist::Value::Boolean(true),
    );
    play.insert(
        "playbackRestrictions".into(),
        plist::Value::Integer(0.into()),
    );
    play.insert(
        "referenceRestrictions".into(),
        plist::Value::Integer(3.into()),
    );
    play.insert(
        "SenderMACAddress".into(),
        plist::Value::String(device_id.to_string()),
    );
    play.insert("model".into(), plist::Value::String("iPhone14,3".into()));
    play.insert(
        "clientBundleID".into(),
        plist::Value::String("dev.vuio.app".into()),
    );
    play.insert("clientProcName".into(), plist::Value::String("VuIO".into()));
    play.insert(
        "osBuildVersion".into(),
        plist::Value::String("20G1116".into()),
    );
    play.insert("volume".into(), plist::Value::Real(1.0));
    play.insert("rate".into(), plist::Value::Real(1.0));
    play.insert(
        "secureConnectionMs".into(),
        plist::Value::Integer(22.into()),
    );
    play.insert("infoMs".into(), plist::Value::Integer(122.into()));
    play.insert("connectMs".into(), plist::Value::Integer(18.into()));
    for field in ["authMs", "bonjourMs", "postAuthMs"] {
        play.insert(field.into(), plist::Value::Integer(0.into()));
    }
    plist::Value::Dictionary(play)
}

fn property_value(value: plist::Value) -> plist::Value {
    let mut wrapper = plist::Dictionary::new();
    wrapper.insert("value".into(), value);
    plist::Value::Dictionary(wrapper)
}

/// Render a response body for an error message or log line, preferring the
/// decoded plist so receiver-side error codes stay readable.
fn describe_body(body: &[u8]) -> String {
    // Generous, because `/info` payloads are the ones worth reading in full.
    const MAX_DESCRIPTION_BYTES: usize = 4096;
    if body.is_empty() {
        return String::new();
    }
    let mut described = match plist::Value::from_reader(std::io::Cursor::new(body)) {
        Ok(value) => format!("{value:?}"),
        Err(_) => String::from_utf8_lossy(body).trim().to_string(),
    };
    if described.len() > MAX_DESCRIPTION_BYTES {
        described.truncate(
            (0..=MAX_DESCRIPTION_BYTES)
                .rev()
                .find(|index| described.is_char_boundary(*index))
                .unwrap_or(0),
        );
        described.push('…');
    }
    format!(": {described}")
}

/// Probe the receiver's playback surface and log what it answers.
///
/// `insertPlayQueueItem` and the rest of the AirPlay 2 "unified media control"
/// command set are undocumented, so when the legacy `/play` endpoint is missing
/// the only reliable way forward is to ask the receiver what it does implement.
/// Every probe here is read-only or already-failed state, and a 405 (rather than
/// 404) is the interesting answer: it means the path exists under another method.
async fn probe_playback_surface(control: &std::sync::Arc<Mutex<SecureControl>>) {
    // RTSP OPTIONS advertises the supported method set in its `Public` header.
    match control.lock().await.rtsp("OPTIONS", "*", None, &[]).await {
        Ok(response) => tracing::info!(
            status = response.status,
            public = response
                .headers
                .get("public")
                .map_or("<absent>", String::as_str),
            "AirPlay probe: OPTIONS *"
        ),
        Err(error) => tracing::info!(%error, "AirPlay probe: OPTIONS * failed"),
    }

    for (protocol, method, path) in [
        // Does /play exist at all under a different method?
        ("HTTP/1.1", "GET", "/play"),
        ("RTSP/1.0", "POST", "/play"),
        ("HTTP/1.1", "GET", "/playback-info"),
        ("HTTP/1.1", "GET", "/server-info"),
        ("HTTP/1.1", "GET", "/playqueue"),
        ("HTTP/1.1", "POST", "/playqueue"),
        ("HTTP/1.1", "GET", "/scrub"),
        ("RTSP/1.0", "GET", "/playback-info"),
        // The AirPlay 2 remote-control command endpoint, which is where the
        // play-queue command set lives when there is no HTTP video surface.
        ("RTSP/1.0", "POST", "/command"),
        ("HTTP/1.1", "POST", "/command"),
        ("RTSP/1.0", "GET", "/getProperty?playbackState"),
        ("RTSP/1.0", "POST", "/audioMode"),
        ("RTSP/1.0", "POST", "/rate?value=1.000000"),
        ("HTTP/1.1", "POST", "/stop"),
        ("RTSP/1.0", "POST", "/fp-setup"),
        ("RTSP/1.0", "POST", "/auth-setup"),
    ] {
        let mut guard = control.lock().await;
        let result = if protocol == "HTTP/1.1" {
            guard.http(method, path, &[], &[]).await
        } else {
            guard.rtsp(method, path, None, &[]).await
        };
        drop(guard);
        match result {
            Ok(response) => tracing::info!(
                probe = %format!("{method} {path} {protocol}"),
                status = response.status,
                body = %describe_body(&response.body),
                "AirPlay probe"
            ),
            Err(error) => {
                tracing::info!(probe = %format!("{method} {path} {protocol}"), %error, "AirPlay probe failed");
                // A transport error means the session is gone; further probes
                // would only produce noise.
                return;
            }
        }
    }
}

/// Ask the receiver to allocate a buffered-audio stream (RAOP type 96).
///
/// This is pyatv's `AirPlayV2.setup_audio_stream` body. A 2xx reply naming
/// `dataPort` and `controlPort` means the receiver will accept RTP audio, which
/// is the only media path a video-less AirPlay 2 set exposes.
async fn probe_audio_stream(
    control: &std::sync::Arc<Mutex<SecureControl>>,
    rtsp_session: &str,
    shared_secret: &[u8],
    session_id: u32,
) -> anyhow::Result<()> {
    let shared_key = derive_key_from(
        shared_secret,
        b"Events-Salt",
        b"Events-Write-Encryption-Key",
    )?;
    let mut stream = plist::Dictionary::new();
    stream.insert("audioFormat".into(), plist::Value::Integer(0x800.into()));
    stream.insert("audioMode".into(), plist::Value::String("default".into()));
    stream.insert("controlPort".into(), plist::Value::Integer(0.into()));
    stream.insert("ct".into(), plist::Value::Integer(2.into()));
    stream.insert("isMedia".into(), plist::Value::Boolean(true));
    stream.insert("latencyMax".into(), plist::Value::Integer(88200.into()));
    stream.insert("latencyMin".into(), plist::Value::Integer(11025.into()));
    stream.insert("shk".into(), plist::Value::Data(shared_key.to_vec()));
    stream.insert("spf".into(), plist::Value::Integer(352.into()));
    stream.insert("sr".into(), plist::Value::Integer(44100.into()));
    stream.insert("type".into(), plist::Value::Integer(0x60.into()));
    stream.insert(
        "supportsDynamicStreamID".into(),
        plist::Value::Boolean(false),
    );
    stream.insert(
        "streamConnectionID".into(),
        plist::Value::Integer(i64::from(session_id).into()),
    );
    let mut body = plist::Dictionary::new();
    body.insert(
        "streams".into(),
        plist::Value::Array(vec![plist::Value::Dictionary(stream)]),
    );
    let encoded = binary_plist(plist::Value::Dictionary(body))?;
    let response = control
        .lock()
        .await
        .rtsp("SETUP", rtsp_session, Some(BINARY_PLIST), &encoded)
        .await?;
    tracing::info!(
        status = response.status,
        body = %describe_body(&response.body),
        "AirPlay buffered-audio SETUP probe"
    );
    Ok(())
}

/// The realtime audio stream description.
///
/// `audioFormat`/`ct` announce ALAC because that is what the receiver decodes;
/// it hardcodes ALAC on this stream and ignores what it was offered.
fn audio_stream_parameters(shared_key: &[u8; 32], session_id: u32) -> plist::Value {
    let mut stream = plist::Dictionary::new();
    stream.insert("audioFormat".into(), plist::Value::Integer(0x40000.into()));
    stream.insert("audioMode".into(), plist::Value::String("default".into()));
    stream.insert("controlPort".into(), plist::Value::Integer(0.into()));
    stream.insert("ct".into(), plist::Value::Integer(2.into()));
    stream.insert("isMedia".into(), plist::Value::Boolean(true));
    stream.insert("latencyMax".into(), plist::Value::Integer(88200.into()));
    stream.insert("latencyMin".into(), plist::Value::Integer(11025.into()));
    stream.insert("shk".into(), plist::Value::Data(shared_key.to_vec()));
    stream.insert(
        "spf".into(),
        plist::Value::Integer((raop::FRAMES_PER_PACKET as i64).into()),
    );
    stream.insert("sr".into(), plist::Value::Integer(44100.into()));
    stream.insert("type".into(), plist::Value::Integer(96.into()));
    stream.insert(
        "supportsDynamicStreamID".into(),
        plist::Value::Boolean(false),
    );
    stream.insert(
        "streamConnectionID".into(),
        plist::Value::Integer(i64::from(session_id).into()),
    );
    let mut body = plist::Dictionary::new();
    body.insert(
        "streams".into(),
        plist::Value::Array(vec![plist::Value::Dictionary(stream)]),
    );
    plist::Value::Dictionary(body)
}

fn plist_port(body: &[u8], key: &str) -> Option<u16> {
    plist::Value::from_reader(std::io::Cursor::new(body))
        .ok()?
        .into_dictionary()?
        .get(key)
        .and_then(plist::Value::as_unsigned_integer)
        .and_then(|port| u16::try_from(port).ok())
}

fn parse_audio_ports(body: &[u8]) -> Option<(u16, u16)> {
    let stream = plist::Value::from_reader(std::io::Cursor::new(body))
        .ok()?
        .into_dictionary()?
        .get("streams")
        .cloned()?
        .into_array()?
        .into_iter()
        .next()?
        .into_dictionary()?;
    let port = |key: &str| {
        stream
            .get(key)
            .and_then(plist::Value::as_unsigned_integer)
            .and_then(|value| u16::try_from(value).ok())
    };
    Some((port("dataPort")?, port("controlPort")?))
}

/// Ask the receiver which audio formats it will actually allocate a stream for.
///
/// `/info` does not advertise an `audioFormats` list, so the only way to learn
/// what a set supports is to request each one and see which SETUPs succeed.
/// Enabled with `VUIO_AIRPLAY_PROBE_FORMATS=1`.
async fn probe_audio_formats(
    control: &mut SecureControl,
    rtsp_session: &str,
    shared_key: &[u8; 32],
    session_id: u32,
) {
    for (label, format) in [
        ("PCM/44100/16/2", 0x800i64),
        ("PCM/44100/24/2", 0x2000),
        ("PCM/48000/16/2", 0x8000),
        ("ALAC/44100/16/2", 0x40000),
        ("ALAC/44100/24/2", 0x80000),
        ("AAC-LC/44100/2", 0x400000),
        ("AAC-ELD/44100/2", 0x1000000),
        ("nonsense", 0x2),
    ] {
        let mut value = audio_stream_parameters(shared_key, session_id);
        if let Some(stream) = value
            .as_dictionary_mut()
            .and_then(|body| body.get_mut("streams"))
            .and_then(plist::Value::as_array_mut)
            .and_then(|streams| streams.first_mut())
            .and_then(plist::Value::as_dictionary_mut)
        {
            stream.insert("audioFormat".into(), plist::Value::Integer(format.into()));
        }
        let Ok(body) = binary_plist(value) else {
            continue;
        };
        match control
            .rtsp("SETUP", rtsp_session, Some(BINARY_PLIST), &body)
            .await
        {
            Ok(response) => tracing::info!(
                format = label,
                bitmask = format!("{format:#x}"),
                status = response.status,
                body = %describe_body(&response.body),
                "AirPlay audio format probe"
            ),
            Err(error) => {
                tracing::info!(format = label, %error, "AirPlay audio format probe failed");
                return;
            }
        }
    }
}

/// Encode one DMAP tag: a four-character code, a big-endian length, the value.
fn dmap_tag(code: &[u8; 4], value: &[u8]) -> Vec<u8> {
    let mut tag = Vec::with_capacity(8 + value.len());
    tag.extend_from_slice(code);
    tag.extend_from_slice(&(value.len() as u32).to_be_bytes());
    tag.extend_from_slice(value);
    tag
}

/// The now-playing payload a receiver renders: title, album, artist.
fn dmap_metadata(metadata: &audio::TrackMetadata) -> Vec<u8> {
    let mut payload = Vec::new();
    if let Some(title) = &metadata.title {
        payload.extend_from_slice(&dmap_tag(b"minm", title.as_bytes()));
    }
    if let Some(album) = &metadata.album {
        payload.extend_from_slice(&dmap_tag(b"asal", album.as_bytes()));
    }
    if let Some(artist) = &metadata.artist {
        payload.extend_from_slice(&dmap_tag(b"asar", artist.as_bytes()));
    }
    dmap_tag(b"mlit", &payload)
}

/// Announce a track to the receiver's now-playing screen.
///
/// `progress` carries RTP timestamps and is what gives the seek bar its extent
/// and position; the DMAP payload supplies the text.
#[allow(clippy::too_many_arguments)]
async fn announce_track(
    control: &std::sync::Arc<Mutex<SecureControl>>,
    rtsp_session: &str,
    receiver_session: Option<&str>,
    sequence: u16,
    rtptime: u32,
    end: u32,
    metadata: &audio::TrackMetadata,
) {
    let progress = format!("progress: {rtptime}/{rtptime}/{end}\r\n");
    let mut guard = control.lock().await;
    if let Err(error) = guard
        .rtsp(
            "SET_PARAMETER",
            rtsp_session,
            Some("text/parameters"),
            progress.as_bytes(),
        )
        .await
    {
        tracing::debug!(%error, "AirPlay progress update failed");
        return;
    }

    let mut headers = vec![("RTP-Info", format!("seq={sequence};rtptime={rtptime}"))];
    if let Some(session) = receiver_session {
        headers.push(("Session", session.to_string()));
    }
    match guard
        .rtsp_with(
            "SET_PARAMETER",
            rtsp_session,
            Some("application/x-dmap-tagged"),
            &headers,
            &dmap_metadata(metadata),
        )
        .await
    {
        Ok(response) => tracing::debug!(
            status = response.status,
            title = metadata.title.as_deref().unwrap_or("<none>"),
            "AirPlay now-playing metadata sent"
        ),
        Err(error) => tracing::debug!(%error, "AirPlay metadata update failed"),
    }

    if let Some((media_type, artwork)) = &metadata.artwork {
        match guard
            .rtsp_with("SET_PARAMETER", "/", Some(media_type), &headers, artwork)
            .await
        {
            Ok(response) => tracing::debug!(
                status = response.status,
                bytes = artwork.len(),
                "AirPlay cover art sent"
            ),
            Err(error) => tracing::debug!(%error, "AirPlay cover art failed"),
        }
    }
}

/// Refresh only the position, leaving the text metadata in place.
async fn update_progress(
    control: &std::sync::Arc<Mutex<SecureControl>>,
    rtsp_session: &str,
    start: u32,
    now: u32,
    end: u32,
) {
    let progress = format!("progress: {start}/{now}/{end}\r\n");
    if let Err(error) = control
        .lock()
        .await
        .rtsp(
            "SET_PARAMETER",
            rtsp_session,
            Some("text/parameters"),
            progress.as_bytes(),
        )
        .await
    {
        tracing::debug!(%error, "AirPlay progress refresh failed");
    }
}

/// Named AirPlay feature bits, from pyatv's `AirPlayFlags`. Only the ones that
/// decide which playback path a receiver supports are listed.
const FEATURE_NAMES: &[(u64, &str)] = &[
    (0, "SupportsAirPlayVideoV1"),
    (7, "SupportsAirPlayScreen"),
    (9, "SupportsAirPlayAudio"),
    (27, "SupportsLegacyPairing"),
    (33, "SupportsAirPlayVideoPlayQueue"),
    (38, "SupportsUnifiedMediaControl"),
    (43, "SupportsSystemPairing"),
    (46, "SupportsHKPairingAndAccessControl"),
    (48, "SupportsCoreUtilsPairingAndEncryption"),
    (49, "SupportsAirPlayVideoV2"),
    (58, "SupportsHangdogRemoteControl"),
];

fn describe_features(bits: u64) -> String {
    let named = FEATURE_NAMES
        .iter()
        .filter(|(bit, _)| bits & (1 << bit) != 0)
        .map(|(_, name)| *name)
        .collect::<Vec<_>>();
    if named.is_empty() {
        return "none of the known playback features".to_string();
    }
    named.join(", ")
}

/// Receivers advertise the event port in their `SETUP` reply before they start
/// listening on it, so a first connect can legitimately be refused.
async fn connect_event_channel(address: SocketAddr) -> anyhow::Result<AirplayConnection> {
    let mut last_error = None;
    for attempt in 1..=EVENT_CHANNEL_ATTEMPTS {
        match AirplayConnection::connect(address).await {
            Ok(connection) => return Ok(connection),
            Err(error) => {
                tracing::debug!(
                    attempt,
                    attempts = EVENT_CHANNEL_ATTEMPTS,
                    %error,
                    "AirPlay event channel is not accepting connections yet"
                );
                last_error = Some(error);
            }
        }
        if attempt < EVENT_CHANNEL_ATTEMPTS {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("AirPlay event channel could not be reached")))
}

async fn run_timing_server(socket: tokio::net::UdpSocket) {
    let mut request = [0u8; 64];
    loop {
        let Ok((length, peer)) = socket.recv_from(&mut request).await else {
            return;
        };
        if length < 32 {
            continue;
        }
        let (seconds, fraction) = ntp_now();
        let mut response = [0u8; 32];
        response[0] = request[0];
        response[1] = 0xD3;
        response[2..4].copy_from_slice(&7u16.to_be_bytes());
        response[8..16].copy_from_slice(&request[24..32]);
        response[16..20].copy_from_slice(&seconds.to_be_bytes());
        response[20..24].copy_from_slice(&fraction.to_be_bytes());
        response[24..28].copy_from_slice(&seconds.to_be_bytes());
        response[28..32].copy_from_slice(&fraction.to_be_bytes());
        let _ = socket.send_to(&response, peer).await;
    }
}

fn ntp_now() -> (u32, u32) {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = duration.as_secs().saturating_add(2_208_988_800) as u32;
    let fraction = ((u64::from(duration.subsec_nanos()) << 32) / 1_000_000_000) as u32;
    (seconds, fraction)
}

fn binary_plist(value: plist::Value) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::new();
    plist::to_writer_binary(&mut body, &value)?;
    Ok(body)
}

struct AbortOnDrop(Option<tokio::task::JoinHandle<()>>);

impl AbortOnDrop {
    fn take(&mut self) -> tokio::task::JoinHandle<()> {
        self.0.take().expect("AbortOnDrop already taken")
    }
}

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}

fn spawn_feedback_task(
    control: std::sync::Arc<Mutex<SecureControl>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(FEEDBACK_INTERVAL);
        loop {
            // The first tick completes immediately, matching pyatv's feedback loop.
            interval.tick().await;
            let result = control
                .lock()
                .await
                .rtsp("POST", "/feedback", None, &[])
                .await;
            if let Err(error) = result {
                tracing::debug!(%error, "AirPlay feedback loop stopped");
                return;
            }
        }
    })
}

/// Poll `/playback-info` until the receiver reports a duration, mirroring
/// pyatv's `_wait_for_media_to_end` start-up window. A receiver-side error is
/// fatal; anything else is treated as "still buffering" and left alone.
async fn soft_check_playback_info(
    control: &std::sync::Arc<Mutex<SecureControl>>,
) -> anyhow::Result<()> {
    for attempt in 1..=PLAYBACK_INFO_ATTEMPTS {
        let response = match control
            .lock()
            .await
            .http("GET", "/playback-info", &[], &[])
            .await
        {
            Ok(response) => response,
            Err(error) => {
                tracing::debug!(%error, "AirPlay /playback-info unavailable");
                return Ok(());
            }
        };
        if !(200..300).contains(&response.status) {
            tracing::debug!(
                status = response.status,
                "AirPlay /playback-info unavailable; continuing"
            );
            return Ok(());
        }
        if let Some(message) = playback_info_error_message(&response.body) {
            anyhow::bail!("{message}");
        }
        if playback_info_reports_duration(&response.body) {
            tracing::debug!(attempt, "AirPlay playback-info reported duration");
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}

fn playback_info_error_message(body: &[u8]) -> Option<String> {
    let value = plist::Value::from_reader(std::io::Cursor::new(body)).ok()?;
    let error = value.as_dictionary()?.get("error")?.as_dictionary()?;
    let code = error.get("code").map(|value| match value {
        plist::Value::Integer(integer) => integer
            .as_signed()
            .map(|signed| signed.to_string())
            .or_else(|| integer.as_unsigned().map(|unsigned| unsigned.to_string()))
            .unwrap_or_else(|| format!("{value:?}")),
        other => format!("{other:?}"),
    })?;
    let domain = error
        .get("domain")
        .and_then(plist::Value::as_string)
        .unwrap_or("unknown domain");
    Some(format!(
        "AirPlay playback failed with error {code} ({domain})"
    ))
}

fn playback_info_reports_duration(body: &[u8]) -> bool {
    if let Ok(value) = plist::Value::from_reader(std::io::Cursor::new(body)) {
        if value
            .as_dictionary()
            .is_some_and(|dictionary| dictionary.contains_key("duration"))
        {
            return true;
        }
    }
    parse_playback_info(&String::from_utf8_lossy(body))
        .ok()
        .is_some_and(|values| values.contains_key("duration"))
}

#[async_trait]
impl CastProvider for AirplayProvider {
    fn protocol(&self) -> RendererProtocol {
        RendererProtocol::Airplay
    }

    async fn discover(&self, timeout: Duration) -> anyhow::Result<Vec<RendererDevice>> {
        let daemon = ServiceDaemon::new()?;
        let receiver = daemon.browse(SERVICE_TYPE)?;
        let deadline = tokio::time::Instant::now() + timeout;
        let mut devices = HashMap::new();
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, receiver.recv_async()).await {
            let ServiceEvent::ServiceResolved(info) = event else {
                continue;
            };
            let feature_text = info
                .get_property_val_str("features")
                .or_else(|| info.get_property_val_str("ft"));
            let Some(features) = parse_features(feature_text) else {
                continue;
            };
            tracing::info!(
                name = info.get_property_val_str("name").unwrap_or(info.get_fullname()),
                raw_features = feature_text.unwrap_or("<absent>"),
                features = %describe_features(features),
                "AirPlay receiver discovered"
            );
            if !supports_url_video_bits(features) && !supports_audio_bits(features) {
                continue;
            }
            let Some(ip) = info
                .get_addresses()
                .iter()
                .map(mdns_sd::ScopedIp::to_ip_addr)
                .find(|address| is_safe_renderer_address(*address))
            else {
                continue;
            };
            let address = SocketAddr::new(ip, info.get_port());
            let raw_id = info
                .get_property_val_str("deviceid")
                .filter(|value| !value.is_empty())
                .unwrap_or(info.get_fullname());
            let id = format!("airplay:{raw_id}");
            self.features.lock().await.insert(id.clone(), features);
            // Only ask for a PIN when the receiver leaves us no alternative.
            // A set that advertises system or CoreUtils pairing takes a
            // transient session, which needs no code and stores nothing.
            let pairing = if !airplay2_enabled() || features & AIRPLAY_VIDEO_V2 == 0 {
                PairingStatus::NotRequired
            } else if self.credentials.is_paired(&id).await {
                PairingStatus::Paired
            } else if supports_transient_pairing(features) {
                PairingStatus::NotRequired
            } else {
                PairingStatus::Required
            };
            devices.entry(id.clone()).or_insert_with(|| RendererDevice {
                id,
                friendly_name: info
                    .get_property_val_str("name")
                    .unwrap_or_else(|| info.get_fullname().split('.').next().unwrap_or("AirPlay"))
                    .to_string(),
                control_url: format!("airplay://{address}"),
                location_url: format!("airplay://{address}"),
                model_name: info
                    .get_property_val_str("model")
                    .unwrap_or("AirPlay")
                    .to_string(),
                protocol: RendererProtocol::Airplay,
                pairing,
                capabilities: RendererCapabilities {
                    video: features & AIRPLAY_VIDEO_V1 != 0,
                    audio: true,
                    image: false,
                    playlists: true,
                    controls: vec![
                        PlaybackAction::Play,
                        PlaybackAction::Pause,
                        PlaybackAction::Stop,
                    ],
                },
                endpoint: RendererEndpoint::Socket(address),
            });
        }
        let _ = daemon.stop_browse(SERVICE_TYPE);
        let _ = daemon.shutdown();
        Ok(devices.into_values().collect())
    }

    fn validate(&self, item: &PlaybackItem) -> Result<(), String> {
        // Audio is decoded and pushed as PCM, so the container does not matter
        // as long as VuIO can decode it.
        if audio::is_streamable_audio(&item.mime_type, &item.filename) {
            return Ok(());
        }
        if is_native_airplay_video(&item.mime_type, &item.filename) {
            Ok(())
        } else {
            Err(format!(
                "{} ({}) is not an AirPlay-native video container; use MP4/M4V, MOV, HLS, or MPEG-TS",
                item.filename, item.mime_type
            ))
        }
    }

    async fn play(&self, device: &RendererDevice, item: &PlaybackItem) -> anyhow::Result<()> {
        let wants_audio = audio::is_streamable_audio(&item.mime_type, &item.filename);
        if !wants_audio {
            self.validate(item).map_err(anyhow::Error::msg)?;
        }
        if airplay2_enabled() {
            // Prefer the handshake that needs no user interaction. A receiver
            // advertising system or CoreUtils pairing takes a transient session,
            // which is what iOS uses with these sets; a stored PIN pairing is
            // only the fallback, and asking for a PIN is the last resort.
            let features = self.features.lock().await.get(&device.id).copied();
            if features.is_some_and(supports_transient_pairing) {
                let address = socket_endpoint(device)?;
                match self.transient_connection(address).await {
                    Ok((connection, shared)) => {
                        if wants_audio {
                            return self.audio_play(device, item, connection, shared).await;
                        }
                        return self
                            .start_secure_session(
                                &device.id, address, connection, shared, &item.url,
                            )
                            .await;
                    }
                    // macOS answers 403 when access control is on, so fall
                    // through to the paired and PIN paths.
                    Err(error) => tracing::debug!(
                        %error,
                        "AirPlay transient pairing refused; trying a stored pairing"
                    ),
                }
            }
            if self.credentials.is_paired(&device.id).await {
                if wants_audio {
                    let (connection, shared) = self.verified_connection(device).await?;
                    return self
                        .audio_play(device, item, connection, shared.to_vec())
                        .await;
                }
                return self.secure_play(device, item).await;
            }
        }
        anyhow::ensure!(
            device.pairing != PairingStatus::Required,
            "AirPlay pairing is required for this receiver"
        );
        let session = self.session_for_play(device).await;
        self.request(
            device,
            reqwest::Method::POST,
            "/play",
            Some(format!(
                "Content-Location: {}\r\nStart-Position: 0\r\n",
                item.url
            )),
            &session,
        )
        .await?;
        Ok(())
    }

    async fn control(&self, device: &RendererDevice, action: PlaybackAction) -> anyhow::Result<()> {
        let mut sessions = self.sessions.lock().await;
        if let Some(ActiveSession::Secure(session)) = sessions.get_mut(&device.id) {
            let is_audio = session.audio_queue.is_some();
            let rtsp_session = session.rtsp_session.clone();
            let mut control = session.control.lock().await;
            // `/rate` is an RTSP command and `/stop` a plain-HTTP one, but an
            // audio session has neither: it ends with TEARDOWN, which is what
            // makes the receiver drop whatever it still has buffered.
            let response = match action {
                _ if is_audio => control.rtsp("TEARDOWN", &rtsp_session, None, &[]).await,
                PlaybackAction::Play => {
                    control
                        .rtsp("POST", "/rate?value=1.000000", None, &[])
                        .await
                }
                PlaybackAction::Pause => {
                    control
                        .rtsp("POST", "/rate?value=0.000000", None, &[])
                        .await
                }
                PlaybackAction::Stop => control.http("POST", "/stop", &[], &[]).await,
            }?;
            anyhow::ensure!(
                (200..300).contains(&response.status),
                "AirPlay control failed with status {}{}",
                response.status,
                describe_body(&response.body)
            );
            drop(control);
            if action == PlaybackAction::Stop || is_audio {
                sessions.remove(&device.id);
            }
            return Ok(());
        }
        drop(sessions);
        let session = self.active_session(device).await?;
        let path = match action {
            PlaybackAction::Play => "/rate?value=1.000000",
            PlaybackAction::Pause => "/rate?value=0.000000",
            PlaybackAction::Stop => "/stop",
        };
        self.request(device, reqwest::Method::POST, path, None, &session)
            .await?;
        if action == PlaybackAction::Stop {
            self.sessions.lock().await.remove(&device.id);
        }
        Ok(())
    }

    async fn status(&self, device: &RendererDevice) -> anyhow::Result<PlaybackStatus> {
        let mut sessions = self.sessions.lock().await;
        if let Some(ActiveSession::Secure(session)) = sessions.get_mut(&device.id) {
            let response = session
                .control
                .lock()
                .await
                .http("GET", "/playback-info", &[], &[])
                .await?;
            anyhow::ensure!(
                (200..300).contains(&response.status),
                "AirPlay status failed with status {}{}",
                response.status,
                describe_body(&response.body)
            );
            return playback_status_from_bytes(&response.body);
        }
        drop(sessions);
        let session = self.active_session(device).await?;
        let body = self
            .request(
                device,
                reqwest::Method::GET,
                "/playback-info",
                None,
                &session,
            )
            .await?;
        let values = parse_playback_info(&body)?;
        let rate = values.get("rate").copied().unwrap_or(0.0);
        let duration = values.get("duration").copied();
        let position = values.get("position").copied();
        let state = if rate > 0.0 {
            PlaybackState::Playing
        } else if duration
            .zip(position)
            .is_some_and(|(duration, position)| duration > 0.0 && duration - position <= 1.0)
        {
            PlaybackState::Finished
        } else if duration.is_some() {
            PlaybackState::Paused
        } else {
            PlaybackState::Stopped
        };
        Ok(PlaybackStatus {
            state,
            current_url: None,
        })
    }

    async fn begin_pairing(&self, device: &RendererDevice) -> anyhow::Result<PairingChallenge> {
        anyhow::ensure!(airplay2_enabled(), "AirPlay 2 pairing is disabled");
        anyhow::ensure!(
            device.protocol == RendererProtocol::Airplay,
            "renderer protocol mismatch"
        );
        let controller = self.credentials.controller().await?;
        let mut connection = AirplayConnection::connect(socket_endpoint(device)?).await?;
        Self::pairing_request(&mut connection, "/pair-pin-start", &[]).await?;
        let first_request = PairSetupClient::new("0000", controller)?.start();
        let first_response =
            Self::pairing_request(&mut connection, "/pair-setup", &first_request).await?;
        let id = uuid::Uuid::new_v4().to_string();
        let mut pending = self.pending_pairings.lock().await;
        pending.retain(|_, challenge| challenge.expires_at > tokio::time::Instant::now());
        anyhow::ensure!(
            pending.len() < 16,
            "too many pending AirPlay pairing attempts"
        );
        pending.insert(
            id.clone(),
            PendingPairing {
                renderer_id: device.id.clone(),
                connection,
                first_response,
                expires_at: tokio::time::Instant::now() + PAIRING_TTL,
            },
        );
        Ok(PairingChallenge {
            id,
            renderer_id: device.id.clone(),
            expires_in_seconds: PAIRING_TTL.as_secs(),
        })
    }

    async fn finish_pairing(&self, challenge_id: &str, pin: &str) -> anyhow::Result<()> {
        anyhow::ensure!(airplay2_enabled(), "AirPlay 2 pairing is disabled");
        anyhow::ensure!(
            valid_pin(pin),
            "enter the PIN displayed by the AirPlay receiver"
        );
        let mut challenge = self
            .pending_pairings
            .lock()
            .await
            .remove(challenge_id)
            .ok_or_else(|| anyhow::anyhow!("AirPlay pairing request expired or was not found"))?;
        anyhow::ensure!(
            challenge.expires_at > tokio::time::Instant::now(),
            "AirPlay pairing request expired"
        );
        let controller = self.credentials.controller().await?;
        let mut setup = PairSetupClient::new(pin, controller)?;
        let _ = setup.start();
        let mut response = challenge.first_response;
        loop {
            match setup.handle(&response)? {
                PairSetupStep::Send(outgoing) => {
                    response =
                        Self::pairing_request(&mut challenge.connection, "/pair-setup", &outgoing)
                            .await?;
                }
                PairSetupStep::Done(pairing) => {
                    self.credentials
                        .save_pairing(&challenge.renderer_id, &pairing)
                        .await?;
                    return Ok(());
                }
            }
        }
    }

    /// Append a track to an open audio session so folders play through.
    async fn queue_next(
        &self,
        device: &RendererDevice,
        item: &PlaybackItem,
    ) -> anyhow::Result<bool> {
        if !audio::is_streamable_audio(&item.mime_type, &item.filename) {
            return Ok(false);
        }
        let sessions = self.sessions.lock().await;
        let Some(ActiveSession::Secure(session)) = sessions.get(&device.id) else {
            return Ok(false);
        };
        let Some(queue) = session.audio_queue.as_ref() else {
            return Ok(false);
        };
        queue.lock().await.push_back(item.local_path.clone());
        Ok(true)
    }

    async fn forget_pairing(&self, device: &RendererDevice) -> anyhow::Result<bool> {
        self.sessions.lock().await.remove(&device.id);
        self.credentials.forget(&device.id).await
    }

    async fn shutdown(&self) {
        // Without TEARDOWN a receiver keeps rendering its buffer after VuIO
        // exits, so tell it to stop before dropping the sessions.
        let sessions: Vec<_> = self.sessions.lock().await.drain().collect();
        for (id, session) in &sessions {
            let ActiveSession::Secure(session) = session else {
                continue;
            };
            let rtsp_session = session.rtsp_session.clone();
            let result = tokio::time::timeout(Duration::from_secs(2), async {
                session
                    .control
                    .lock()
                    .await
                    .rtsp("TEARDOWN", &rtsp_session, None, &[])
                    .await
            })
            .await;
            match result {
                Ok(Ok(response)) => {
                    tracing::debug!(id, status = response.status, "AirPlay session torn down")
                }
                Ok(Err(error)) => tracing::debug!(id, %error, "AirPlay TEARDOWN failed"),
                Err(_) => tracing::debug!(id, "AirPlay TEARDOWN timed out"),
            }
        }
        drop(sessions);
        self.pending_pairings.lock().await.clear();
    }
}

fn valid_pin(pin: &str) -> bool {
    let digit_count = pin.bytes().filter(u8::is_ascii_digit).count();
    (4..=8).contains(&digit_count)
        && pin.len() <= 16
        && pin
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'-')
}

fn is_native_airplay_video(mime: &str, filename: &str) -> bool {
    matches!(
        mime.split(';')
            .next()
            .unwrap_or(mime)
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "video/mp4"
            | "video/x-m4v"
            | "video/quicktime"
            | "application/vnd.apple.mpegurl"
            | "application/x-mpegurl"
            | "video/mp2t"
    ) || filename.rsplit_once('.').is_some_and(|(_, extension)| {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            "mp4" | "m4v" | "mov" | "m3u8" | "ts"
        )
    })
}

impl Default for AirplayProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
fn supports_url_video(features: Option<&str>) -> bool {
    parse_features(features).is_some_and(supports_url_video_bits)
}

/// `/play` is the AirPlay-video-v1 endpoint, so bit 0 decides whether a
/// receiver can play a *URL* at all.
///
/// A receiver advertising only v2 (bit 49) drives video through Apple's
/// play-queue command set. Probing a Sony XR-75X90L showed there is no way to
/// reach it as a third-party sender: every HTTP video path answers 404,
/// `/command` exists but 500s because no stream can be bound to it, the
/// type-130 remote-control stream is refused (pyatv likewise only attempts it
/// against Apple TV and HomePod), and `OPTIONS *` advertises just the RAOP
/// audio and mirroring methods. Such a receiver is still listed, because audio
/// reaches it over RTP -- it simply reports no video capability.
fn supports_url_video_bits(bits: u64) -> bool {
    bits & AIRPLAY_VIDEO_V1 != 0 || (allow_video_v2_only() && bits & AIRPLAY_VIDEO_V2 != 0)
}

/// Receivers that accept a buffered-audio stream, which is every AirPlay 2 set.
fn supports_audio_bits(bits: u64) -> bool {
    bits & AIRPLAY_AUDIO != 0
}

/// Escape hatch for retrying video on v2-only receivers, e.g. after a firmware
/// update. Set `VUIO_AIRPLAY_ALLOW_V2_ONLY=1`. Audio never needs this.
fn allow_video_v2_only() -> bool {
    std::env::var("VUIO_AIRPLAY_ALLOW_V2_ONLY")
        .is_ok_and(|value| matches!(value.trim(), "1" | "true" | "yes"))
}

/// Receivers advertising system or CoreUtils pairing accept a transient
/// session, so no PIN and no stored credentials are needed.
fn supports_transient_pairing(bits: u64) -> bool {
    bits & (AIRPLAY_SYSTEM_PAIRING | AIRPLAY_CORE_UTILS_PAIRING) != 0
}

fn airplay2_enabled() -> bool {
    AIRPLAY_2_ENABLED
}

fn parse_features(features: Option<&str>) -> Option<u64> {
    let mut words = features?.split(',');
    let low = parse_feature_word(words.next()?)?;
    let high = match words.next() {
        Some(word) => parse_feature_word(word)?,
        None => 0,
    };
    if words.next().is_some() || low > u32::MAX as u64 || high > u32::MAX as u64 {
        return None;
    }
    Some(low | (high << 32))
}

fn parse_feature_word(word: &str) -> Option<u64> {
    let word = word.trim();
    if let Some(hex) = word.strip_prefix("0x").or_else(|| word.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        word.parse().ok()
    }
}

fn socket_endpoint(device: &RendererDevice) -> anyhow::Result<SocketAddr> {
    anyhow::ensure!(
        device.protocol == RendererProtocol::Airplay,
        "renderer protocol mismatch"
    );
    let RendererEndpoint::Socket(address) = device.endpoint else {
        anyhow::bail!("invalid AirPlay endpoint");
    };
    anyhow::ensure!(
        is_safe_renderer_address(address.ip()),
        "unsafe renderer address"
    );
    Ok(address)
}

fn parse_playback_info(xml: &str) -> anyhow::Result<HashMap<String, f64>> {
    let mut reader = Reader::from_str(xml);
    let mut current = String::new();
    let mut pending_key = None;
    let mut values = HashMap::new();
    loop {
        match reader.read_event()? {
            Event::Start(element) => {
                current = String::from_utf8_lossy(element.name().as_ref()).into_owned();
            }
            Event::Text(text) => {
                let value = reader.decoder().decode(text.as_ref())?.into_owned();
                match current.as_str() {
                    "key" => pending_key = Some(value),
                    "real" | "integer" => {
                        if let (Some(key), Ok(number)) = (pending_key.take(), value.parse()) {
                            values.insert(key, number);
                        }
                    }
                    _ => {}
                }
            }
            Event::End(_) => current.clear(),
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(values)
}

fn playback_status_from_bytes(bytes: &[u8]) -> anyhow::Result<PlaybackStatus> {
    let values = if let Ok(plist::Value::Dictionary(dictionary)) =
        plist::Value::from_reader(std::io::Cursor::new(bytes))
    {
        dictionary
            .into_iter()
            .filter_map(|(key, value)| {
                let number = match value {
                    plist::Value::Real(value) => Some(value),
                    plist::Value::Integer(value) => value.as_signed().map(|value| value as f64),
                    _ => None,
                }?;
                Some((key, number))
            })
            .collect()
    } else {
        parse_playback_info(&String::from_utf8_lossy(bytes))?
    };
    let rate = values.get("rate").copied().unwrap_or(0.0);
    let duration = values.get("duration").copied();
    let position = values.get("position").copied();
    let state = if rate > 0.0 {
        PlaybackState::Playing
    } else if duration
        .zip(position)
        .is_some_and(|(duration, position)| duration > 0.0 && duration - position <= 1.0)
    {
        PlaybackState::Finished
    } else if duration.is_some() {
        PlaybackState::Paused
    } else {
        PlaybackState::Stopped
    };
    Ok(PlaybackStatus {
        state,
        current_url: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn renderer() -> RendererDevice {
        let address = "192.168.1.20:7000".parse().unwrap();
        RendererDevice {
            id: "airplay:AA:BB:CC:DD:EE:FF".to_string(),
            friendly_name: "Apple TV".to_string(),
            control_url: format!("airplay://{address}"),
            location_url: format!("airplay://{address}"),
            model_name: "AppleTV".to_string(),
            protocol: RendererProtocol::Airplay,
            pairing: PairingStatus::NotRequired,
            capabilities: RendererCapabilities {
                video: true,
                audio: false,
                image: false,
                playlists: true,
                controls: vec![
                    PlaybackAction::Play,
                    PlaybackAction::Pause,
                    PlaybackAction::Stop,
                ],
            },
            endpoint: RendererEndpoint::Socket(address),
        }
    }

    #[test]
    fn feature_parser_requires_url_video_bit() {
        assert!(supports_url_video(Some("0x1,0x0")));
        assert!(supports_url_video(Some("3")));
        // Bit 49 alone is the play-queue-only case; see
        // `url_video_requires_the_airplay_video_v1_bit`.
        assert!(!supports_url_video(Some("0x0,0x20000")));
        // Both receivers on the test network advertise transient pairing.
        assert!(supports_transient_pairing(
            parse_features(Some("0x7F8AD0,0x18BCF46")).unwrap()
        ));
        assert!(supports_transient_pairing(
            parse_features(Some("0x4A7FCFD5,0x38174FDE")).unwrap()
        ));
        assert!(!supports_transient_pairing(AIRPLAY_VIDEO_V1));
        assert!(!supports_url_video(Some("0x200")));
        assert!(!supports_url_video(Some("invalid")));
        assert!(!supports_url_video(None));
    }

    #[test]
    fn url_video_requires_the_airplay_video_v1_bit() {
        assert!(airplay2_enabled());
        assert!(supports_url_video_bits(AIRPLAY_VIDEO_V1));
        assert!(supports_url_video_bits(AIRPLAY_VIDEO_V1 | AIRPLAY_VIDEO_V2));
        // A v2-only receiver has no /play endpoint; a Sony XR-75X90L reports
        // 0x7F8AD0,0x18BCF46 and 404s every HTTP playback path.
        assert!(!supports_url_video_bits(AIRPLAY_VIDEO_V2));
        assert!(!supports_url_video(Some("0x7F8AD0,0x18BCF46")));
        // A macOS receiver reports 0x4A7FCFD5,0x38174FDE, which does have bit 0.
        assert!(supports_url_video(Some("0x4A7FCFD5,0x38174FDE")));
        assert!(!supports_url_video_bits(0));
    }

    #[test]
    fn feature_parser_combines_both_airplay_words() {
        let features = parse_features(Some("0x7F8AD0,0x18BCF46")).unwrap();
        assert_ne!(features & (1 << 38), 0);
        assert_ne!(features & (1 << 46), 0);
        assert_ne!(features & (1 << 48), 0);
        assert_ne!(features & (1 << 49), 0);
    }

    #[test]
    fn play_parameters_send_the_media_url_untouched() {
        let value = play_parameters(
            "http://192.168.1.2:8080/media/15.mp4",
            "36c0f1ba-3f8f-4a1e-8b0f-1b7a2a3d4e5f",
            "AA:BB:CC:DD:EE:FF",
        );
        let play = value.as_dictionary().unwrap();
        assert_eq!(
            play.get("Content-Location")
                .and_then(plist::Value::as_string),
            Some("http://192.168.1.2:8080/media/15.mp4")
        );
        assert_eq!(
            play.get("mediaType").and_then(plist::Value::as_string),
            Some("file")
        );
        assert_eq!(play.get("rate").and_then(plist::Value::as_real), Some(1.0));
    }

    #[test]
    fn playback_info_plist_error_is_detected() {
        let mut error = plist::Dictionary::new();
        error.insert("code".into(), plist::Value::Integer((-6707).into()));
        error.insert(
            "domain".into(),
            plist::Value::String("NSOSStatusErrorDomain".into()),
        );
        let mut dictionary = plist::Dictionary::new();
        dictionary.insert("error".into(), plist::Value::Dictionary(error));
        let body = binary_plist(plist::Value::Dictionary(dictionary)).unwrap();
        let message = playback_info_error_message(&body).unwrap();
        assert!(message.contains("-6707"));
        assert!(message.contains("NSOSStatusErrorDomain"));
        assert!(playback_info_error_message(&[]).is_none());
    }

    #[test]
    fn playback_info_extracts_timeline() {
        let values = parse_playback_info(
            "<plist><dict><key>duration</key><real>83.5</real><key>position</key><real>12.25</real><key>rate</key><real>1</real></dict></plist>",
        )
        .unwrap();
        assert_eq!(values.get("duration"), Some(&83.5));
        assert_eq!(values.get("position"), Some(&12.25));
        assert_eq!(values.get("rate"), Some(&1.0));
    }

    #[test]
    fn native_airplay_video_matrix_accepts_supported_containers() {
        for (mime, filename) in [
            ("video/mp4", "movie.mp4"),
            ("video/x-m4v", "movie.m4v"),
            ("video/quicktime", "movie.mov"),
            ("application/vnd.apple.mpegurl", "movie.m3u8"),
            ("video/mp2t", "movie.ts"),
        ] {
            assert!(is_native_airplay_video(mime, filename), "{mime}");
        }
        assert!(!is_native_airplay_video("video/x-matroska", "movie.mkv"));
        assert!(!is_native_airplay_video("video/webm", "movie.webm"));
    }

    #[test]
    fn pairing_pin_validation_accepts_receiver_pins_only() {
        assert!(valid_pin("1234"));
        assert!(valid_pin("123-45-678"));
        assert!(!valid_pin("123"));
        assert!(!valid_pin("1234\r\nX-Test: injected"));
    }

    /// A receiver that requires pairing must surface that, not a transient
    /// failure, so the dashboard offers the PIN flow.
    #[tokio::test]
    async fn pairing_required_receivers_ask_for_a_pin() {
        let provider = AirplayProvider::new();
        let mut renderer = renderer();
        renderer.pairing = PairingStatus::Required;
        let item = PlaybackItem {
            id: 1,
            url: "http://192.168.1.2:8080/media/1.mp4".to_string(),
            local_path: std::path::PathBuf::from("/tmp/test.mp4"),
            title: "Test".to_string(),
            filename: "test.mp4".to_string(),
            mime_type: "video/mp4".to_string(),
        };
        let error = provider
            .play(&renderer, &item)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("pairing is required"), "{error}");
    }

    #[tokio::test]
    async fn session_ids_are_valid_and_stable_for_a_playlist() {
        let provider = AirplayProvider::new();
        let renderer = renderer();
        let first = provider.session_for_play(&renderer).await;
        let second = provider.session_for_play(&renderer).await;
        assert_eq!(first, second);
        assert!(uuid::Uuid::parse_str(&first).is_ok());
    }
}

#[cfg(test)]
mod session_tests {
    use super::*;
    use hap_transport::record_test_support::{decrypt_frame, encrypt_frame, NonceCounter};
    use std::sync::{Arc, Mutex as StdMutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    const SHARED_SECRET: [u8; 32] = [7u8; 32];

    /// Request head plus body, exactly as the receiver saw it.
    type CapturedRequests = Arc<StdMutex<Vec<(String, Vec<u8>)>>>;

    /// A minimal AirPlay 2 receiver: HAP record framing plus just enough of the
    /// RTSP/HTTP surface to complete one media session. It answers every request
    /// with 200 so the test asserts on what VuIO *sent*, not on receiver quirks.
    struct FakeReceiver {
        stream: TcpStream,
        read_key: [u8; 32],
        write_key: [u8; 32],
        read_counter: NonceCounter,
        write_counter: NonceCounter,
        wire: Vec<u8>,
        plain: Vec<u8>,
    }

    impl FakeReceiver {
        fn new(stream: TcpStream) -> anyhow::Result<Self> {
            Ok(Self {
                stream,
                // Mirror image of the sender: it writes with the "Write" key.
                read_key: derive_key_from(
                    &SHARED_SECRET,
                    b"Control-Salt",
                    b"Control-Write-Encryption-Key",
                )?,
                write_key: derive_key_from(
                    &SHARED_SECRET,
                    b"Control-Salt",
                    b"Control-Read-Encryption-Key",
                )?,
                read_counter: NonceCounter::new(),
                write_counter: NonceCounter::new(),
                wire: Vec::new(),
                plain: Vec::new(),
            })
        }

        async fn fill(&mut self) -> anyhow::Result<()> {
            loop {
                if let Some(block) =
                    decrypt_frame(&self.read_key, &mut self.read_counter, &self.wire)?
                {
                    self.wire.drain(..2 + block.len() + 16);
                    self.plain.extend_from_slice(&block);
                    return Ok(());
                }
                let read = self.stream.read_buf(&mut self.wire).await?;
                anyhow::ensure!(read != 0, "sender closed the connection");
            }
        }

        async fn read_request(&mut self) -> anyhow::Result<(String, Vec<u8>)> {
            loop {
                if let Some(end) = self
                    .plain
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                {
                    let head = String::from_utf8(self.plain[..end].to_vec())?;
                    let length = content_length(&head);
                    if self.plain.len() >= end + 4 + length {
                        let body = self.plain[end + 4..end + 4 + length].to_vec();
                        self.plain.drain(..end + 4 + length);
                        return Ok((head, body));
                    }
                }
                self.fill().await?;
            }
        }

        async fn respond(&mut self, protocol: &str, body: &[u8]) -> anyhow::Result<()> {
            let mut message = format!(
                "{protocol} 200 OK\r\nContent-Length: {}\r\n\r\n",
                body.len()
            )
            .into_bytes();
            message.extend_from_slice(body);
            for block in message.chunks(1024) {
                let frame = encrypt_frame(&self.write_key, &mut self.write_counter, block)?;
                self.stream.write_all(&frame).await?;
            }
            self.stream.flush().await?;
            Ok(())
        }
    }

    fn content_length(head: &str) -> usize {
        head.split("\r\n")
            .skip(1)
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
            .and_then(|(_, value)| value.trim().parse().ok())
            .unwrap_or(0)
    }

    fn request_line(head: &str) -> &str {
        head.split("\r\n").next().unwrap_or_default()
    }

    async fn secured_sender(address: SocketAddr) -> AirplayConnection {
        let mut connection = AirplayConnection::connect(address).await.unwrap();
        connection.secure(SessionKeys {
            read_key: derive_key_from(
                &SHARED_SECRET,
                b"Control-Salt",
                b"Control-Read-Encryption-Key",
            )
            .unwrap(),
            write_key: derive_key_from(
                &SHARED_SECRET,
                b"Control-Salt",
                b"Control-Write-Encryption-Key",
            )
            .unwrap(),
        });
        connection
    }

    /// Drive a whole session against the fake receiver and return every request
    /// VuIO sent, in order.
    async fn run_session(media_url: &str) -> Vec<(String, Vec<u8>)> {
        let control_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let control_address = control_listener.local_addr().unwrap();
        let event_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let event_port = event_listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            // Accept the reverse event channel and hold it open; the sender only
            // reads from it in this scenario.
            let _connection = event_listener.accept().await;
            std::future::pending::<()>().await;
        });

        let requests: CapturedRequests = Arc::new(StdMutex::new(Vec::new()));
        let recorded = requests.clone();
        tokio::spawn(async move {
            let (stream, _) = control_listener.accept().await.unwrap();
            let mut receiver = FakeReceiver::new(stream).unwrap();
            while let Ok((head, body)) = receiver.read_request().await {
                let line = request_line(&head).to_string();
                let protocol = line
                    .split_whitespace()
                    .nth(2)
                    .unwrap_or("RTSP/1.0")
                    .to_string();
                recorded.lock().unwrap().push((head, body));
                let reply = if line.starts_with("SETUP ") {
                    let mut setup = plist::Dictionary::new();
                    setup.insert(
                        "eventPort".into(),
                        plist::Value::Integer(u64::from(event_port).into()),
                    );
                    binary_plist(plist::Value::Dictionary(setup)).unwrap()
                } else if line.starts_with("GET /playback-info ") {
                    let mut info = plist::Dictionary::new();
                    info.insert("duration".into(), plist::Value::Real(120.0));
                    info.insert("position".into(), plist::Value::Real(0.0));
                    info.insert("rate".into(), plist::Value::Real(1.0));
                    binary_plist(plist::Value::Dictionary(info)).unwrap()
                } else {
                    Vec::new()
                };
                if receiver.respond(&protocol, &reply).await.is_err() {
                    return;
                }
            }
        });

        let connection = secured_sender(control_address).await;
        let provider = AirplayProvider::new();
        provider
            .start_secure_session(
                "airplay:test",
                control_address,
                connection,
                SHARED_SECRET.to_vec(),
                media_url,
            )
            .await
            .expect("the session should establish against a well-behaved receiver");

        let captured = requests.lock().unwrap().clone();
        captured
    }

    #[tokio::test]
    async fn secure_session_follows_the_reference_play_sequence() {
        let media_url = "http://192.168.1.2:8080/media/15.mp4";
        let requests = run_session(media_url).await;
        let lines: Vec<String> = requests
            .iter()
            .map(|(head, _)| request_line(head).to_string())
            .collect();

        // The /info probe and SETUP both run before the event channel and the
        // feedback loop exist, so their positions are deterministic.
        assert_eq!(
            lines.first().map(String::as_str),
            Some("GET /info RTSP/1.0"),
            "{lines:?}"
        );
        let setup = lines.get(1).expect("no SETUP request was captured");
        assert!(setup.starts_with("SETUP rtsp://"), "{lines:?}");
        assert!(setup.ends_with(" RTSP/1.0"), "{lines:?}");

        let position = |prefix: &str| {
            lines
                .iter()
                .position(|line| line.starts_with(prefix))
                .unwrap_or_else(|| panic!("no request matched {prefix:?} in {lines:?}"))
        };

        // /play must go out as HTTP: receivers that dispatch on the protocol
        // line reject "POST /play RTSP/1.0".
        let play = position("POST /play ");
        assert_eq!(lines[play], "POST /play HTTP/1.1", "{lines:?}");
        assert_eq!(
            lines[position("GET /playback-info ")],
            "GET /playback-info HTTP/1.1"
        );

        // RECORD precedes playback, and /rate lands before the end-time
        // properties, matching pyatv's ordering.
        assert!(position("RECORD ") < play, "{lines:?}");
        assert!(position("POST /rate?value=1.000000 ") > play, "{lines:?}");
        assert!(
            position("POST /rate?value=1.000000 ") < position("PUT /setProperty?forwardEndTime "),
            "{lines:?}"
        );
        assert_eq!(
            lines[position("POST /rate?value=1.000000 ")],
            "POST /rate?value=1.000000 RTSP/1.0"
        );
    }

    #[tokio::test]
    async fn play_request_carries_the_media_url_and_apple_session_headers() {
        let media_url = "http://192.168.1.2:8080/media/15.mp4";
        let requests = run_session(media_url).await;

        let (head, body) = requests
            .iter()
            .find(|(head, _)| request_line(head).starts_with("POST /play "))
            .expect("no /play request was sent");
        assert!(
            head.contains("Content-Type: application/x-apple-binary-plist"),
            "{head}"
        );
        assert!(head.contains("X-Apple-ProtocolVersion: 1"), "{head}");
        assert!(head.contains("X-Apple-Session-ID: "), "{head}");
        assert!(head.contains("X-Apple-Stream-ID: 1"), "{head}");
        assert!(head.contains("User-Agent: AirPlay/550.10"), "{head}");

        let play = plist::Value::from_reader(std::io::Cursor::new(body)).unwrap();
        let play = play.as_dictionary().unwrap();
        // The receiver fetches this URL itself; it must arrive unmodified.
        assert_eq!(
            play.get("Content-Location")
                .and_then(plist::Value::as_string),
            Some(media_url)
        );

        // RTSP requests carry the session identity triple pyatv sends.
        let (setup_head, _) = requests
            .iter()
            .find(|(head, _)| request_line(head).starts_with("SETUP "))
            .expect("no SETUP request was sent");
        assert!(setup_head.contains("DACP-ID: "), "{setup_head}");
        assert!(setup_head.contains("Active-Remote: "), "{setup_head}");
        assert!(setup_head.contains("Client-Instance: "), "{setup_head}");
    }
}

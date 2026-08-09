//! AirPlay URL-video discovery, pairing, and control.

mod credentials;
mod pair_verify;
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
use std::{collections::HashMap, net::SocketAddr, path::PathBuf, time::Duration};
use tokio::sync::Mutex;

use self::{
    credentials::CredentialStore,
    pair_verify::{derive_key, PairVerifier},
    transport::AirplayConnection,
};

const SERVICE_TYPE: &str = "_airplay._tcp.local.";
const MAX_RESPONSE_BYTES: usize = 256 * 1024;
const PAIRING_TTL: Duration = Duration::from_secs(120);

struct PendingPairing {
    renderer_id: String,
    connection: AirplayConnection,
    first_response: Vec<u8>,
    expires_at: tokio::time::Instant,
}

struct SecureSession {
    control: std::sync::Arc<Mutex<SecureControl>>,
    session_id: String,
    event_task: tokio::task::JoinHandle<()>,
    feedback_task: tokio::task::JoinHandle<()>,
    timing_task: tokio::task::JoinHandle<()>,
}

struct SecureControl {
    connection: AirplayConnection,
    cseq: u32,
}

impl Drop for SecureSession {
    fn drop(&mut self) {
        self.event_task.abort();
        self.feedback_task.abort();
        self.timing_task.abort();
    }
}

enum ActiveSession {
    Legacy(String),
    Secure(Box<SecureSession>),
}

pub struct AirplayProvider {
    sessions: Mutex<HashMap<String, ActiveSession>>,
    pending_pairings: Mutex<HashMap<String, PendingPairing>>,
    credentials: CredentialStore,
}

impl AirplayProvider {
    pub fn new() -> Self {
        Self::with_credentials(CredentialStore::memory())
    }

    pub async fn persistent(path: PathBuf) -> anyhow::Result<Self> {
        Ok(Self::with_credentials(CredentialStore::load(path).await?))
    }

    fn with_credentials(credentials: CredentialStore) -> Self {
        Self {
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

    async fn secure_play(
        &self,
        device: &RendererDevice,
        item: &PlaybackItem,
    ) -> anyhow::Result<()> {
        let (mut connection, shared_secret) = self.verified_connection(device).await?;
        let session_id = uuid::Uuid::new_v4().to_string().to_uppercase();
        let bind_address = match socket_endpoint(device)?.ip() {
            std::net::IpAddr::V4(_) => "0.0.0.0:0",
            std::net::IpAddr::V6(_) => "[::]:0",
        };
        let timing_socket = tokio::net::UdpSocket::bind(bind_address).await?;
        let timing_port = i64::from(timing_socket.local_addr()?.port());

        let mut setup = plist::Dictionary::new();
        setup.insert(
            "deviceID".into(),
            plist::Value::String(controller_device_id()),
        );
        setup.insert(
            "sessionUUID".into(),
            plist::Value::String(session_id.clone()),
        );
        setup.insert(
            "timingPort".into(),
            plist::Value::Integer(timing_port.into()),
        );
        setup.insert("timingProtocol".into(), plist::Value::String("NTP".into()));
        setup.insert("isRemoteControlOnly".into(), plist::Value::Boolean(false));
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
        setup.insert("name".into(), plist::Value::String("VuIO".into()));
        setup.insert("model".into(), plist::Value::String("VuIO".into()));
        setup.insert("osName".into(), plist::Value::String("VuIO".into()));
        setup.insert(
            "sourceVersion".into(),
            plist::Value::String("550.10".into()),
        );
        let setup_body = binary_plist(plist::Value::Dictionary(setup))?;
        let mut cseq = 1;
        let setup_response = secure_request(
            &mut connection,
            "SETUP",
            "/stream",
            &session_id,
            &mut cseq,
            "application/x-apple-binary-plist",
            &setup_body,
        )
        .await?;
        anyhow::ensure!(
            (200..300).contains(&setup_response.status),
            "AirPlay 2 SETUP failed with status {}",
            setup_response.status
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
        let event_address = SocketAddr::new(socket_endpoint(device)?.ip(), event_port);
        let mut event_connection = AirplayConnection::connect(event_address).await?;
        event_connection.secure(SessionKeys {
            write_key: derive_key(
                &shared_secret,
                b"Events-Salt",
                b"Events-Read-Encryption-Key",
            )?,
            read_key: derive_key(
                &shared_secret,
                b"Events-Salt",
                b"Events-Write-Encryption-Key",
            )?,
        });
        for (method, path) in [("POST", "/feedback"), ("RECORD", "/stream")] {
            let response = secure_request(
                &mut connection,
                method,
                path,
                &session_id,
                &mut cseq,
                "application/octet-stream",
                &[],
            )
            .await?;
            anyhow::ensure!(
                (200..300).contains(&response.status),
                "AirPlay 2 {method} failed with status {}",
                response.status
            );
        }

        let mut play = plist::Dictionary::new();
        play.insert(
            "Content-Location".into(),
            plist::Value::String(item.url.clone()),
        );
        play.insert("Start-Position-Seconds".into(), plist::Value::Real(0.0));
        play.insert("mediaType".into(), plist::Value::String("file".into()));
        play.insert("streamType".into(), plist::Value::Integer(1.into()));
        play.insert("uuid".into(), plist::Value::String(session_id.clone()));
        play.insert("rate".into(), plist::Value::Real(1.0));
        play.insert("volume".into(), plist::Value::Real(1.0));
        let play_body = binary_plist(plist::Value::Dictionary(play))?;
        let response = secure_request(
            &mut connection,
            "POST",
            "/play",
            &session_id,
            &mut cseq,
            "application/x-apple-binary-plist",
            &play_body,
        )
        .await?;
        anyhow::ensure!(
            (200..300).contains(&response.status),
            "AirPlay 2 play failed with status {}",
            response.status
        );
        let rate_response = secure_request(
            &mut connection,
            "POST",
            "/rate?value=1.000000",
            &session_id,
            &mut cseq,
            "application/octet-stream",
            &[],
        )
        .await?;
        anyhow::ensure!(
            (200..300).contains(&rate_response.status),
            "AirPlay 2 rate command failed with status {}",
            rate_response.status
        );
        let event_task = tokio::spawn(async move {
            if let Err(error) = event_connection.serve_events().await {
                tracing::debug!(%error, "AirPlay event channel closed");
            }
        });
        let timing_task = tokio::spawn(run_timing_server(timing_socket));
        let control = std::sync::Arc::new(Mutex::new(SecureControl { connection, cseq }));
        let feedback_control = control.clone();
        let feedback_session_id = session_id.clone();
        let feedback_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            interval.tick().await;
            loop {
                interval.tick().await;
                let mut control = feedback_control.lock().await;
                let SecureControl { connection, cseq } = &mut *control;
                if secure_request(
                    connection,
                    "POST",
                    "/feedback",
                    &feedback_session_id,
                    cseq,
                    "application/octet-stream",
                    &[],
                )
                .await
                .is_err()
                {
                    return;
                }
            }
        });
        self.sessions.lock().await.insert(
            device.id.clone(),
            ActiveSession::Secure(Box::new(SecureSession {
                control,
                session_id,
                event_task,
                feedback_task,
                timing_task,
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

async fn secure_request(
    connection: &mut AirplayConnection,
    method: &str,
    path: &str,
    session_id: &str,
    cseq: &mut u32,
    content_type: &str,
    body: &[u8],
) -> anyhow::Result<transport::Response> {
    let sequence = *cseq;
    *cseq = cseq.saturating_add(1);
    connection
        .request(
            method,
            path,
            "RTSP/1.0",
            &[
                ("User-Agent", "AirPlay/550.10".to_string()),
                ("CSeq", sequence.to_string()),
                ("DACP-ID", session_id.replace('-', "")),
                ("Active-Remote", "1".to_string()),
                ("X-Apple-Session-ID", session_id.to_string()),
                ("X-Apple-ProtocolVersion", "1".to_string()),
                ("X-Apple-Stream-ID", "1".to_string()),
                ("Content-Type", content_type.to_string()),
            ],
            body,
        )
        .await
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
            if !supports_url_video_bits(features) {
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
            let pairing = if features & (1 << 49) == 0 {
                PairingStatus::NotRequired
            } else if self.credentials.is_paired(&id).await {
                PairingStatus::Paired
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
            });
        }
        let _ = daemon.stop_browse(SERVICE_TYPE);
        let _ = daemon.shutdown();
        Ok(devices.into_values().collect())
    }

    fn validate(&self, item: &PlaybackItem) -> Result<(), String> {
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
        self.validate(item).map_err(anyhow::Error::msg)?;
        if self.credentials.is_paired(&device.id).await {
            return self.secure_play(device, item).await;
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
            let (method, path) = match action {
                PlaybackAction::Play => ("POST", "/rate?value=1.000000"),
                PlaybackAction::Pause => ("POST", "/rate?value=0.000000"),
                PlaybackAction::Stop => ("POST", "/stop"),
            };
            let mut control = session.control.lock().await;
            let SecureControl { connection, cseq } = &mut *control;
            let response = secure_request(
                connection,
                method,
                path,
                &session.session_id,
                cseq,
                "application/octet-stream",
                &[],
            )
            .await?;
            anyhow::ensure!(
                (200..300).contains(&response.status),
                "AirPlay control failed with status {}",
                response.status
            );
            drop(control);
            if action == PlaybackAction::Stop {
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
            let mut control = session.control.lock().await;
            let SecureControl { connection, cseq } = &mut *control;
            let response = secure_request(
                connection,
                "GET",
                "/playback-info",
                &session.session_id,
                cseq,
                "application/octet-stream",
                &[],
            )
            .await?;
            anyhow::ensure!(
                (200..300).contains(&response.status),
                "AirPlay status failed with status {}",
                response.status
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

    async fn forget_pairing(&self, device: &RendererDevice) -> anyhow::Result<bool> {
        self.sessions.lock().await.remove(&device.id);
        self.credentials.forget(&device.id).await
    }

    async fn shutdown(&self) {
        self.sessions.lock().await.clear();
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

fn supports_url_video_bits(bits: u64) -> bool {
    const AIRPLAY_VIDEO_V1: u64 = 1 << 0;
    const AIRPLAY_VIDEO_V2: u64 = 1 << 49;
    bits & (AIRPLAY_VIDEO_V1 | AIRPLAY_VIDEO_V2) != 0
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
        assert!(supports_url_video(Some("0x7F8AD0,0x18BCF46")));
        assert!(!supports_url_video(Some("0x200")));
        assert!(!supports_url_video(Some("invalid")));
        assert!(!supports_url_video(None));
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
    fn native_airplay_video_matrix_accepts_airplay2_containers() {
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

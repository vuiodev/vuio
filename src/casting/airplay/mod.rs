//! Dependency-free AirPlay URL-video discovery and control.

use super::{
    is_safe_renderer_address, CastProvider, PlaybackAction, PlaybackItem, PlaybackState,
    PlaybackStatus, RendererCapabilities, RendererDevice, RendererEndpoint, RendererProtocol,
};
use async_trait::async_trait;
use futures_util::StreamExt;
use mdns_sd::{ServiceDaemon, ServiceEvent};
use quick_xml::{events::Event, Reader};
use std::{collections::HashMap, net::SocketAddr, time::Duration};
use tokio::sync::Mutex;

const SERVICE_TYPE: &str = "_airplay._tcp.local.";
const MAX_RESPONSE_BYTES: usize = 256 * 1024;

pub struct AirplayProvider {
    sessions: Mutex<HashMap<String, String>>,
}

impl AirplayProvider {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
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
        sessions
            .entry(device.id.clone())
            .or_insert_with(|| uuid::Uuid::new_v4().to_string())
            .clone()
    }

    async fn active_session(&self, device: &RendererDevice) -> anyhow::Result<String> {
        self.sessions
            .lock()
            .await
            .get(&device.id)
            .cloned()
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
            if !supports_url_video(info.get_property_val_str("features")) {
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
        if item.mime_type == "video/mp4" {
            Ok(())
        } else {
            Err(format!(
                "AirPlay URL video only supports progressive MP4; {} is {}",
                item.filename, item.mime_type
            ))
        }
    }

    async fn play(&self, device: &RendererDevice, item: &PlaybackItem) -> anyhow::Result<()> {
        self.validate(item).map_err(anyhow::Error::msg)?;
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

    async fn shutdown(&self) {
        self.sessions.lock().await.clear();
    }
}

impl Default for AirplayProvider {
    fn default() -> Self {
        Self::new()
    }
}

fn supports_url_video(features: Option<&str>) -> bool {
    let Some(first) = features.and_then(|value| value.split(',').next()) else {
        return false;
    };
    let parsed = first
        .strip_prefix("0x")
        .or_else(|| first.strip_prefix("0X"))
        .map(|value| u64::from_str_radix(value, 16))
        .unwrap_or_else(|| first.parse::<u64>());
    parsed.is_ok_and(|bits| bits & 1 != 0)
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
        assert!(!supports_url_video(Some("0x200")));
        assert!(!supports_url_video(None));
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

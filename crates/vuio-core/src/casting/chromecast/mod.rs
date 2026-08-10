//! Google Cast discovery and Default Media Receiver control.

use super::{
    is_safe_renderer_address, CastProvider, PlaybackAction, PlaybackItem, PlaybackState,
    PlaybackStatus, RendererCapabilities, RendererDevice, RendererEndpoint, RendererProtocol,
};
use async_trait::async_trait;
use mdns_sd::{ServiceDaemon, ServiceEvent};
use oxicast::{
    CastApp, CastClient, IdleReason, MediaInfo, MediaMetadata, PlayerState, QueueItem, StreamType,
};
use std::{collections::HashMap, net::SocketAddr, time::Duration};
use tokio::sync::Mutex;

const SERVICE_TYPE: &str = "_googlecast._tcp.local.";
const MAX_SESSIONS: usize = 128;

pub struct ChromecastProvider {
    sessions: Mutex<HashMap<String, CastClient>>,
}

impl ChromecastProvider {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    async fn client_for(&self, device: &RendererDevice) -> anyhow::Result<CastClient> {
        if let Some(client) = self.sessions.lock().await.get(&device.id).cloned() {
            if client.is_connected() {
                return Ok(client);
            }
        }
        let address = socket_endpoint(device, RendererProtocol::Chromecast)?;
        let client = CastClient::connect(&address.ip().to_string(), address.port()).await?;
        let mut sessions = self.sessions.lock().await;
        if sessions.len() >= MAX_SESSIONS && !sessions.contains_key(&device.id) {
            if let Some(key) = sessions.keys().next().cloned() {
                if let Some(old) = sessions.remove(&key) {
                    tokio::spawn(async move {
                        let _ = old.disconnect().await;
                    });
                }
            }
        }
        sessions.insert(device.id.clone(), client.clone());
        Ok(client)
    }

    async fn active_client(&self, device: &RendererDevice) -> anyhow::Result<CastClient> {
        let client = self
            .sessions
            .lock()
            .await
            .get(&device.id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no active Chromecast session for this renderer"))?;
        anyhow::ensure!(client.is_connected(), "Chromecast session is disconnected");
        Ok(client)
    }
}

impl Default for ChromecastProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CastProvider for ChromecastProvider {
    fn protocol(&self) -> RendererProtocol {
        RendererProtocol::Chromecast
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
                .get_property_val_str("id")
                .filter(|value| !value.is_empty())
                .unwrap_or(info.get_fullname());
            let id = format!("chromecast:{raw_id}");
            devices.entry(id.clone()).or_insert_with(|| RendererDevice {
                id,
                friendly_name: info
                    .get_property_val_str("fn")
                    .unwrap_or(info.get_fullname())
                    .to_string(),
                control_url: format!("cast://{address}"),
                location_url: format!("cast://{address}"),
                model_name: info
                    .get_property_val_str("md")
                    .unwrap_or("Google Cast")
                    .to_string(),
                protocol: RendererProtocol::Chromecast,
                pairing: super::PairingStatus::NotRequired,
                capabilities: RendererCapabilities {
                    video: true,
                    audio: true,
                    image: true,
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
        const SUPPORTED: &[&str] = &[
            "video/mp4",
            "video/webm",
            "video/mp2t",
            "audio/mpeg",
            "audio/mp4",
            "audio/ogg",
            "audio/wav",
            "audio/flac",
            "audio/webm",
            "image/jpeg",
            "image/png",
            "image/gif",
            "image/bmp",
            "image/webp",
        ];
        if SUPPORTED.contains(&item.mime_type.as_str()) {
            Ok(())
        } else {
            Err(format!(
                "Chromecast direct play does not support {} ({})",
                item.filename, item.mime_type
            ))
        }
    }

    async fn play(&self, device: &RendererDevice, item: &PlaybackItem) -> anyhow::Result<()> {
        self.validate(item).map_err(anyhow::Error::msg)?;
        let client = self.client_for(device).await?;
        client.launch_app(&CastApp::DefaultMediaReceiver).await?;
        let media = media_info(item);
        client.load_media(&media, true, 0.0, None).await?;
        Ok(())
    }

    /// Append the next item to the receiver's own queue.
    ///
    /// The Default Media Receiver advances a queue by itself, which is what
    /// makes a series play through: the alternative, polling for "finished",
    /// misses the transition because the media session disappears as soon as an
    /// item ends and the status reads as stopped rather than finished.
    async fn queue_next(
        &self,
        device: &RendererDevice,
        item: &PlaybackItem,
    ) -> anyhow::Result<bool> {
        self.validate(item).map_err(anyhow::Error::msg)?;
        let client = self.active_client(device).await?;
        let queue_item = QueueItem {
            media: media_info(item),
            autoplay: true,
            start_time: 0.0,
        };
        client.queue_insert(&[queue_item], None).await?;
        Ok(true)
    }

    async fn control(&self, device: &RendererDevice, action: PlaybackAction) -> anyhow::Result<()> {
        let client = self.active_client(device).await?;
        match action {
            PlaybackAction::Play => {
                client.play().await?;
            }
            PlaybackAction::Pause => {
                client.pause().await?;
            }
            PlaybackAction::Stop => {
                client.stop_media().await?;
            }
        }
        Ok(())
    }

    async fn status(&self, device: &RendererDevice) -> anyhow::Result<PlaybackStatus> {
        let client = self.active_client(device).await?;
        let Some(status) = client.media_status().await? else {
            return Ok(PlaybackStatus {
                state: PlaybackState::Stopped,
                current_url: None,
            });
        };
        let state = match (status.player_state, status.idle_reason) {
            (PlayerState::Playing | PlayerState::Buffering, _) => PlaybackState::Playing,
            (PlayerState::Paused, _) => PlaybackState::Paused,
            (PlayerState::Idle, Some(IdleReason::Finished)) => PlaybackState::Finished,
            (PlayerState::Idle, Some(IdleReason::Error)) => PlaybackState::Error,
            (PlayerState::Idle, _) => PlaybackState::Stopped,
            _ => PlaybackState::Unknown,
        };
        Ok(PlaybackStatus {
            state,
            current_url: status.media.map(|media| media.content_id),
        })
    }

    async fn shutdown(&self) {
        let clients = self
            .sessions
            .lock()
            .await
            .drain()
            .map(|(_, client)| client)
            .collect::<Vec<_>>();
        for client in clients {
            let _ = client.disconnect().await;
        }
    }
}

fn socket_endpoint(
    device: &RendererDevice,
    expected: RendererProtocol,
) -> anyhow::Result<SocketAddr> {
    anyhow::ensure!(device.protocol == expected, "renderer protocol mismatch");
    let RendererEndpoint::Socket(address) = device.endpoint else {
        anyhow::bail!("invalid renderer endpoint");
    };
    anyhow::ensure!(
        is_safe_renderer_address(address.ip()),
        "unsafe renderer address"
    );
    Ok(address)
}

/// Describe one item for the Cast receiver, including the metadata it shows.
fn media_info(item: &PlaybackItem) -> MediaInfo {
    let metadata = if item.mime_type.starts_with("audio/") {
        MediaMetadata::MusicTrack {
            title: Some(item.title.clone()),
            artist: None,
            album_name: None,
            composer: None,
            track_number: None,
            disc_number: None,
            images: Vec::new(),
        }
    } else if item.mime_type.starts_with("image/") {
        MediaMetadata::Photo {
            title: Some(item.title.clone()),
            artist: None,
            location: None,
            latitude: None,
            longitude: None,
            width: None,
            height: None,
            images: Vec::new(),
        }
    } else {
        MediaMetadata::Movie {
            title: Some(item.title.clone()),
            subtitle: None,
            studio: None,
            images: Vec::new(),
        }
    };
    MediaInfo::new(&item.url, &item.mime_type)
        .stream_type(StreamType::Buffered)
        .metadata(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(mime_type: &str) -> PlaybackItem {
        PlaybackItem {
            local_path: std::path::PathBuf::new(),
            id: 1,
            url: "http://192.168.1.2/media/1".to_string(),
            title: "Test".to_string(),
            filename: "test.bin".to_string(),
            mime_type: mime_type.to_string(),
        }
    }

    #[test]
    fn direct_play_matrix_rejects_mkv_and_accepts_mp4() {
        let provider = ChromecastProvider::new();
        assert!(provider.validate(&item("video/mp4")).is_ok());
        assert!(provider.validate(&item("video/x-matroska")).is_err());
        assert!(provider.validate(&item("video/mpeg")).is_err());
    }
}

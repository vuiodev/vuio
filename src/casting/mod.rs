//! Protocol-neutral discovery and playback control.

pub mod airplay;
pub mod chromecast;
mod dlna;

use async_trait::async_trait;
use serde::Serialize;
use std::{net::SocketAddr, sync::Arc, time::Duration};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererProtocol {
    Dlna,
    Chromecast,
    Airplay,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RendererCapabilities {
    pub video: bool,
    pub audio: bool,
    pub image: bool,
    pub playlists: bool,
    pub controls: Vec<PlaybackAction>,
}

#[derive(Clone, Debug)]
pub enum RendererEndpoint {
    Dlna {
        control_url: String,
        location_url: String,
    },
    Socket(SocketAddr),
}

#[derive(Clone, Debug, Serialize)]
pub struct RendererDevice {
    pub id: String,
    pub friendly_name: String,
    pub control_url: String,
    pub location_url: String,
    pub model_name: String,
    pub protocol: RendererProtocol,
    pub capabilities: RendererCapabilities,
    #[serde(skip)]
    pub endpoint: RendererEndpoint,
}

impl RendererDevice {
    pub fn peer_ip(&self) -> Option<std::net::IpAddr> {
        match &self.endpoint {
            RendererEndpoint::Socket(address) => Some(address.ip()),
            RendererEndpoint::Dlna { location_url, .. } => reqwest::Url::parse(location_url)
                .ok()?
                .host_str()?
                .parse()
                .ok(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlaybackItem {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub filename: String,
    pub mime_type: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackAction {
    Play,
    Pause,
    Stop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackState {
    Playing,
    Paused,
    Stopped,
    Finished,
    Error,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct PlaybackStatus {
    pub state: PlaybackState,
    pub current_url: Option<String>,
}

#[async_trait]
pub trait CastProvider: Send + Sync {
    fn protocol(&self) -> RendererProtocol;
    async fn discover(&self, timeout: Duration) -> anyhow::Result<Vec<RendererDevice>>;
    fn validate(&self, item: &PlaybackItem) -> Result<(), String>;
    async fn play(&self, device: &RendererDevice, item: &PlaybackItem) -> anyhow::Result<()>;
    async fn control(&self, device: &RendererDevice, action: PlaybackAction) -> anyhow::Result<()>;
    async fn status(&self, device: &RendererDevice) -> anyhow::Result<PlaybackStatus>;
    async fn queue_next(
        &self,
        _device: &RendererDevice,
        _item: &PlaybackItem,
    ) -> anyhow::Result<bool> {
        Ok(false)
    }
    async fn shutdown(&self) {}
}

pub struct CastingManager {
    providers: Vec<Arc<dyn CastProvider>>,
}

pub struct DiscoveryBatch {
    pub devices: Vec<RendererDevice>,
    pub failed_protocols: Vec<RendererProtocol>,
}

impl CastingManager {
    pub fn new() -> Self {
        Self {
            providers: vec![
                Arc::new(dlna::DlnaProvider),
                Arc::new(chromecast::ChromecastProvider::new()),
                Arc::new(airplay::AirplayProvider::new()),
            ],
        }
    }

    pub async fn discover(&self, timeout: Duration) -> anyhow::Result<DiscoveryBatch> {
        let results =
            futures_util::future::join_all(self.providers.iter().map(|provider| async move {
                (provider.protocol(), provider.discover(timeout).await)
            }))
            .await;
        let mut devices = Vec::new();
        let mut errors = Vec::new();
        let mut failed_protocols = Vec::new();
        for (protocol, result) in results {
            match result {
                Ok(mut discovered) => devices.append(&mut discovered),
                Err(error) => {
                    failed_protocols.push(protocol);
                    errors.push(error);
                }
            }
        }
        if devices.is_empty() && errors.len() == self.providers.len() {
            anyhow::bail!(
                "all renderer discovery providers failed: {}",
                errors
                    .iter()
                    .map(|error| error.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
        for error in errors {
            tracing::warn!(%error, "Renderer discovery provider failed");
        }
        Ok(DiscoveryBatch {
            devices,
            failed_protocols,
        })
    }

    fn provider(&self, protocol: RendererProtocol) -> anyhow::Result<&Arc<dyn CastProvider>> {
        self.providers
            .iter()
            .find(|provider| provider.protocol() == protocol)
            .ok_or_else(|| anyhow::anyhow!("renderer protocol is unavailable"))
    }

    pub fn validate(&self, device: &RendererDevice, item: &PlaybackItem) -> Result<(), String> {
        self.provider(device.protocol)
            .map_err(|error| error.to_string())?
            .validate(item)
    }

    pub async fn play(&self, device: &RendererDevice, item: &PlaybackItem) -> anyhow::Result<()> {
        self.provider(device.protocol)?.play(device, item).await
    }

    pub async fn control(
        &self,
        device: &RendererDevice,
        action: PlaybackAction,
    ) -> anyhow::Result<()> {
        self.provider(device.protocol)?
            .control(device, action)
            .await
    }

    pub async fn status(&self, device: &RendererDevice) -> anyhow::Result<PlaybackStatus> {
        self.provider(device.protocol)?.status(device).await
    }

    pub async fn queue_next(
        &self,
        device: &RendererDevice,
        item: &PlaybackItem,
    ) -> anyhow::Result<bool> {
        self.provider(device.protocol)?
            .queue_next(device, item)
            .await
    }

    pub async fn shutdown(&self) {
        for provider in &self.providers {
            provider.shutdown().await;
        }
    }
}

impl Default for CastingManager {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn is_safe_renderer_address(address: std::net::IpAddr) -> bool {
    match address {
        std::net::IpAddr::V4(ip) => {
            !ip.is_loopback()
                && !ip.is_unspecified()
                && !ip.is_link_local()
                && !ip.is_multicast()
                && ip != std::net::Ipv4Addr::BROADCAST
        }
        std::net::IpAddr::V6(ip) => {
            !ip.is_loopback()
                && !ip.is_unspecified()
                && !ip.is_unicast_link_local()
                && !ip.is_multicast()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_protocol_serializes_for_public_apis() {
        assert_eq!(
            serde_json::to_string(&RendererProtocol::Chromecast).unwrap(),
            "\"chromecast\""
        );
        assert_eq!(
            serde_json::to_string(&RendererProtocol::Airplay).unwrap(),
            "\"airplay\""
        );
    }

    #[test]
    fn unsafe_renderer_addresses_are_rejected() {
        assert!(!is_safe_renderer_address("127.0.0.1".parse().unwrap()));
        assert!(!is_safe_renderer_address("169.254.1.1".parse().unwrap()));
        assert!(is_safe_renderer_address("192.168.1.20".parse().unwrap()));
    }
}

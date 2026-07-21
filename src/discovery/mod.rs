//! Unified device discovery combining SSDP (UPnP/DLNA) and mDNS (Chromecast/AirPlay).

pub mod compat;
pub mod mdns;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// The kind of playback target protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    /// UPnP/DLNA MediaRenderer
    Dlna,
    /// Google Chromecast (Castv2 protocol)
    Chromecast,
    /// Apple AirPlay (HTTP video control)
    AirPlay,
}

impl std::fmt::Display for TargetKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TargetKind::Dlna => write!(f, "DLNA"),
            TargetKind::Chromecast => write!(f, "Chromecast"),
            TargetKind::AirPlay => write!(f, "AirPlay"),
        }
    }
}

/// Codec/container capabilities for a target device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetCapabilities {
    /// Supported video codecs (e.g., "h264", "vp9", "hevc")
    pub video_codecs: Vec<String>,
    /// Supported audio codecs (e.g., "aac", "mp3", "opus")
    pub audio_codecs: Vec<String>,
    /// Supported container formats (e.g., "mp4", "mkv", "webm")
    pub containers: Vec<String>,
}

impl TargetCapabilities {
    pub fn chromecast() -> Self {
        Self {
            video_codecs: vec!["h264".into(), "vp8".into(), "vp9".into(), "av1".into()],
            audio_codecs: vec![
                "aac".into(),
                "mp3".into(),
                "opus".into(),
                "vorbis".into(),
                "flac".into(),
            ],
            containers: vec![
                "mp4".into(),
                "webm".into(),
                "mkv".into(),
                "mp3".into(),
                "flac".into(),
                "wav".into(),
                "ogg".into(),
            ],
        }
    }

    pub fn airplay() -> Self {
        Self {
            video_codecs: vec!["h264".into(), "hevc".into()],
            audio_codecs: vec!["aac".into(), "mp3".into(), "alac".into()],
            containers: vec![
                "mp4".into(),
                "mov".into(),
                "mp3".into(),
                "wav".into(),
                "alac".into(),
            ],
        }
    }

    pub fn dlna() -> Self {
        Self {
            video_codecs: vec!["h264".into(), "mpeg2".into(), "mpeg4".into()],
            audio_codecs: vec![
                "aac".into(),
                "mp3".into(),
                "wav".into(),
                "flac".into(),
                "lpcm".into(),
            ],
            containers: vec![
                "mp4".into(),
                "mkv".into(),
                "avi".into(),
                "ts".into(),
                "mp3".into(),
                "flac".into(),
                "wav".into(),
            ],
        }
    }
}

/// A discovered playback target on the local network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackTarget {
    /// Unique identifier for this target
    pub id: String,
    /// Human-friendly device name
    pub friendly_name: String,
    /// The protocol this target uses
    pub kind: TargetKind,
    /// Network address of the device
    pub address: SocketAddr,
    /// Device model (if known)
    pub model: Option<String>,
    /// Protocol-specific control endpoint
    pub control_url: Option<String>,
    /// Codec/container capabilities
    pub capabilities: TargetCapabilities,
}

/// Configuration for the discovery service.
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub chromecast_enabled: bool,
    pub airplay_enabled: bool,
    pub discovery_interval: Duration,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            chromecast_enabled: true,
            airplay_enabled: true,
            discovery_interval: Duration::from_secs(30),
        }
    }
}

/// Shared snapshot of discovered targets.
pub struct DiscoveryService {
    targets: Arc<RwLock<Vec<PlaybackTarget>>>,
    config: RwLock<DiscoveryConfig>,
    config_changed: tokio::sync::Notify,
}

impl DiscoveryService {
    pub fn new(config: DiscoveryConfig) -> Self {
        Self {
            targets: Arc::new(RwLock::new(Vec::new())),
            config: RwLock::new(config),
            config_changed: tokio::sync::Notify::new(),
        }
    }

    pub async fn reconfigure(&self, config: DiscoveryConfig) {
        *self.config.write().await = config;
        self.config_changed.notify_one();
    }

    /// Get the current snapshot of all discovered targets.
    pub async fn targets(&self) -> Vec<PlaybackTarget> {
        self.targets.read().await.clone()
    }

    /// Run a single discovery cycle: SSDP (DLNA) + mDNS (Cast/AirPlay).
    pub async fn refresh(&self) -> Vec<PlaybackTarget> {
        let config = self.config.read().await.clone();
        let mut all_targets = Vec::new();

        // 1. Discover DLNA renderers (reuse existing discover_tvs)
        match crate::tv_control::discover_tvs().await {
            Ok(tvs) => {
                for tv in tvs {
                    let addr = extract_socket_addr(&tv.location_url)
                        .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 0)));
                    all_targets.push(PlaybackTarget {
                        id: tv.id.clone(),
                        friendly_name: tv.friendly_name,
                        kind: TargetKind::Dlna,
                        address: addr,
                        model: Some(tv.model_name),
                        control_url: Some(tv.control_url),
                        capabilities: TargetCapabilities::dlna(),
                    });
                }
                debug!("DLNA discovery found {} renderer(s)", all_targets.len());
            }
            Err(e) => {
                warn!(error = %e, "DLNA renderer discovery failed");
            }
        }

        // 2. Discover Chromecast and AirPlay via mDNS
        if config.chromecast_enabled || config.airplay_enabled {
            match mdns::discover_mdns_targets(config.chromecast_enabled, config.airplay_enabled)
                .await
            {
                Ok(mdns_targets) => {
                    debug!("mDNS discovery found {} target(s)", mdns_targets.len());
                    all_targets.extend(mdns_targets);
                }
                Err(e) => {
                    warn!(error = %e, "mDNS discovery failed");
                }
            }
        }

        // Deduplicate by ID
        let mut seen = HashMap::new();
        all_targets.retain(|target| seen.insert(target.id.clone(), ()).is_none());

        info!(
            "Discovery cycle complete: {} target(s) ({} DLNA, {} Cast, {} AirPlay)",
            all_targets.len(),
            all_targets
                .iter()
                .filter(|t| t.kind == TargetKind::Dlna)
                .count(),
            all_targets
                .iter()
                .filter(|t| t.kind == TargetKind::Chromecast)
                .count(),
            all_targets
                .iter()
                .filter(|t| t.kind == TargetKind::AirPlay)
                .count(),
        );

        *self.targets.write().await = all_targets.clone();
        all_targets
    }

    /// Run the discovery loop as a background task until the cancellation token is triggered.
    pub async fn run(self: Arc<Self>, cancel: tokio_util::sync::CancellationToken) {
        let initial_interval = self.config.read().await.discovery_interval;
        info!(
            "Starting unified discovery service (interval: {}s)",
            initial_interval.as_secs()
        );

        // Initial discovery
        self.refresh().await;

        loop {
            let interval = self.config.read().await.discovery_interval;
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("Discovery service shutting down");
                    break;
                }
                _ = tokio::time::sleep(interval) => {
                    self.refresh().await;
                }
                _ = self.config_changed.notified() => {
                    self.refresh().await;
                }
            }
        }
    }
}

impl Default for DiscoveryService {
    fn default() -> Self {
        Self::new(DiscoveryConfig::default())
    }
}

/// Extract a SocketAddr from a URL string (best-effort).
fn extract_socket_addr(url: &str) -> Option<SocketAddr> {
    let url = reqwest::Url::parse(url).ok()?;
    let host = url.host_str()?;
    let port = url.port().unwrap_or(80);
    let ip: std::net::IpAddr = host.parse().ok()?;
    Some(SocketAddr::new(ip, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_kind_display() {
        assert_eq!(TargetKind::Dlna.to_string(), "DLNA");
        assert_eq!(TargetKind::Chromecast.to_string(), "Chromecast");
        assert_eq!(TargetKind::AirPlay.to_string(), "AirPlay");
    }

    #[test]
    fn target_kind_serializes_lowercase() {
        let json = serde_json::to_string(&TargetKind::Chromecast).unwrap();
        assert_eq!(json, "\"chromecast\"");
    }

    #[test]
    fn extract_addr_from_url() {
        let addr = extract_socket_addr("http://192.168.1.100:8080/desc.xml");
        assert_eq!(addr, Some(SocketAddr::from(([192, 168, 1, 100], 8080))));
    }

    #[test]
    fn extract_addr_default_port() {
        let addr = extract_socket_addr("http://10.0.0.5/desc.xml");
        assert_eq!(addr, Some(SocketAddr::from(([10, 0, 0, 5], 80))));
    }

    #[test]
    fn chromecast_capabilities_include_h264() {
        let caps = TargetCapabilities::chromecast();
        assert!(caps.video_codecs.contains(&"h264".to_string()));
        assert!(caps.audio_codecs.contains(&"aac".to_string()));
    }

    #[test]
    fn airplay_capabilities_include_hevc() {
        let caps = TargetCapabilities::airplay();
        assert!(caps.video_codecs.contains(&"hevc".to_string()));
    }
}

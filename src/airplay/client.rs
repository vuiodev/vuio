//! AirPlay HTTP video control client.
//!
//! Apple AirPlay video receivers expose an HTTP API on port 7000
//! for controlling video playback. This module implements all the
//! control endpoints.

use anyhow::{Context, Result};
use std::net::SocketAddr;
use tracing::debug;

/// AirPlay video playback controller.
pub struct AirPlayClient {
    address: SocketAddr,
    client: reqwest::Client,
}

/// Current playback status from an AirPlay device.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AirPlayStatus {
    /// Current playback position in seconds.
    pub position: f64,
    /// Total duration in seconds.
    pub duration: f64,
    /// Current playback rate (0.0 = paused, 1.0 = playing).
    pub rate: f64,
}

impl AirPlayClient {
    /// Create a new AirPlay client for a device at the given address.
    /// The address should point to the AirPlay receiver (usually port 7000).
    pub fn new(address: SocketAddr) -> Self {
        let client = crate::http_clients::local()
            .expect("shared local HTTP client configuration must be valid");

        Self { address, client }
    }

    /// Base URL for the AirPlay HTTP API.
    fn base_url(&self) -> String {
        format!("http://{}", self.address)
    }

    /// Start playing a media URL on the AirPlay device.
    pub async fn play(&self, media_url: &str, start_position: f64) -> Result<()> {
        let url = format!("{}/play", self.base_url());
        let body = format!("Content-Location: {media_url}\nStart-Position: {start_position:.6}\n");

        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "text/parameters")
            .body(body)
            .send()
            .await
            .context("failed to send play command to AirPlay device")?;

        if resp.status().is_success() {
            debug!(%media_url, "AirPlay play command sent");
            Ok(())
        } else {
            anyhow::bail!("AirPlay play command failed with HTTP {}", resp.status());
        }
    }

    /// Pause playback.
    pub async fn pause(&self) -> Result<()> {
        self.set_rate(0.0).await
    }

    /// Resume playback.
    pub async fn resume(&self) -> Result<()> {
        self.set_rate(1.0).await
    }

    /// Stop playback entirely.
    pub async fn stop(&self) -> Result<()> {
        let url = format!("{}/stop", self.base_url());

        let resp = self
            .client
            .post(&url)
            .send()
            .await
            .context("failed to send stop command to AirPlay device")?;

        if resp.status().is_success() {
            debug!("AirPlay stop command sent");
            Ok(())
        } else {
            anyhow::bail!("AirPlay stop command failed with HTTP {}", resp.status());
        }
    }

    /// Seek to a specific position in seconds.
    pub async fn seek(&self, position_secs: f64) -> Result<()> {
        let url = format!("{}/scrub?position={:.6}", self.base_url(), position_secs);

        let resp = self
            .client
            .post(&url)
            .send()
            .await
            .context("failed to send seek command to AirPlay device")?;

        if resp.status().is_success() {
            debug!(position = position_secs, "AirPlay seek command sent");
            Ok(())
        } else {
            anyhow::bail!("AirPlay seek command failed with HTTP {}", resp.status());
        }
    }

    /// Get the current playback position and duration.
    pub async fn get_scrub_status(&self) -> Result<AirPlayStatus> {
        let url = format!("{}/scrub", self.base_url());

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("failed to query AirPlay scrub status")?;

        if !resp.status().is_success() {
            anyhow::bail!("AirPlay scrub query failed with HTTP {}", resp.status());
        }

        let body = resp
            .text()
            .await
            .context("failed to read AirPlay scrub response")?;

        let mut duration = 0.0;
        let mut position = 0.0;

        for line in body.lines() {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("duration:") {
                duration = value.trim().parse::<f64>().unwrap_or(0.0);
            } else if let Some(value) = line.strip_prefix("position:") {
                position = value.trim().parse::<f64>().unwrap_or(0.0);
            }
        }

        Ok(AirPlayStatus {
            position,
            duration,
            rate: if position > 0.0 { 1.0 } else { 0.0 },
        })
    }

    /// Get detailed playback info (plist format).
    pub async fn get_playback_info(&self) -> Result<String> {
        let url = format!("{}/playback-info", self.base_url());

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("failed to query AirPlay playback info")?;

        if !resp.status().is_success() {
            anyhow::bail!("AirPlay playback-info failed with HTTP {}", resp.status());
        }

        resp.text()
            .await
            .context("failed to read AirPlay playback info")
    }

    /// Set the playback rate (0.0 = pause, 1.0 = normal speed).
    async fn set_rate(&self, rate: f64) -> Result<()> {
        let url = format!("{}/rate?value={:.6}", self.base_url(), rate);

        let resp = self
            .client
            .post(&url)
            .send()
            .await
            .context("failed to send rate command to AirPlay device")?;

        if resp.status().is_success() {
            debug!(rate, "AirPlay rate command sent");
            Ok(())
        } else {
            anyhow::bail!("AirPlay rate command failed with HTTP {}", resp.status());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn airplay_client_creates_correct_base_url() {
        let client = AirPlayClient::new(SocketAddr::from(([192, 168, 1, 50], 7000)));
        assert_eq!(client.base_url(), "http://192.168.1.50:7000");
    }

    #[test]
    fn airplay_status_serializes() {
        let status = AirPlayStatus {
            position: 30.5,
            duration: 120.0,
            rate: 1.0,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("30.5"));
        assert!(json.contains("120.0"));
    }
}

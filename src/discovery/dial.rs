//! DIAL (Discovery and Launch) protocol discovery.
//!
//! DIAL uses SSDP to discover smart TVs that support launching apps
//! (YouTube, Netflix, etc.) and then queries their REST API for available apps.

use super::{PlaybackTarget, TargetCapabilities, TargetKind};
use anyhow::{Context, Result};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use tokio::net::UdpSocket;
use tracing::{debug, trace, warn};

/// SSDP search target for DIAL devices.
const DIAL_SEARCH_TARGET: &str = "urn:dial-multiscreen-org:service:dial:1";

/// Maximum number of DIAL devices to collect per scan.
const MAX_DIAL_DEVICES: usize = 32;

/// SSDP response timeout.
const SSDP_TIMEOUT: Duration = Duration::from_secs(3);

/// A discovered DIAL device with its Application-URL.
#[derive(Debug, Clone)]
pub struct DialDevice {
    /// The DIAL REST API base URL for launching apps.
    pub application_url: String,
    /// Device friendly name (from XML descriptor).
    pub friendly_name: String,
    /// Device model name.
    pub model_name: String,
    /// Unique service name.
    pub usn: String,
    /// Device IP address.
    pub address: IpAddr,
}

/// Discover DIAL devices on the local network.
pub async fn discover_dial_devices() -> Result<Vec<PlaybackTarget>> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_broadcast(true)?;

    let search_request = format!(
        "M-SEARCH * HTTP/1.1\r\n\
         HOST: 239.255.255.250:1900\r\n\
         MAN: \"ssdp:discover\"\r\n\
         MX: 3\r\n\
         ST: {DIAL_SEARCH_TARGET}\r\n\
         \r\n"
    );

    let target: SocketAddr = "239.255.255.250:1900".parse()?;
    socket.send_to(search_request.as_bytes(), target).await?;

    let mut locations: Vec<(String, String, IpAddr)> = Vec::new();
    let mut buf = vec![0u8; 2048];

    let deadline = tokio::time::Instant::now() + SSDP_TIMEOUT;

    loop {
        let recv_future = socket.recv_from(&mut buf);
        match tokio::time::timeout_at(deadline, recv_future).await {
            Ok(Ok((len, addr))) => {
                let response = String::from_utf8_lossy(&buf[..len]);
                let mut location = None;
                let mut usn = None;
                for line in response.lines() {
                    let lower = line.to_lowercase();
                    if lower.starts_with("location:") {
                        location = Some(line[9..].trim().to_string());
                    } else if lower.starts_with("usn:") {
                        usn = Some(line[4..].trim().to_string());
                    }
                }
                if let Some(location) = location {
                    if locations.len() < MAX_DIAL_DEVICES
                        && !locations
                            .iter()
                            .any(|(existing, _, _)| existing == &location)
                    {
                        locations.push((location, usn.unwrap_or_default(), addr.ip()));
                    }
                }
            }
            Ok(Err(e)) => {
                warn!("SSDP receive error during DIAL discovery: {}", e);
                break;
            }
            Err(_) => break, // Timeout
        }
    }

    debug!(
        "DIAL SSDP found {} potential device location(s)",
        locations.len()
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let mut targets = Vec::new();
    for (location, usn, responder) in locations {
        match fetch_dial_device(&client, &location, &usn, responder).await {
            Ok(Some(device)) => {
                debug!(
                    "Discovered DIAL device: {} (apps at {})",
                    device.friendly_name, device.application_url
                );
                let port = reqwest::Url::parse(&device.application_url)
                    .ok()
                    .and_then(|u| u.port())
                    .unwrap_or(80);
                targets.push(PlaybackTarget {
                    id: device.usn.clone(),
                    friendly_name: device.friendly_name,
                    kind: TargetKind::Dial,
                    address: SocketAddr::new(device.address, port),
                    model: Some(device.model_name),
                    control_url: Some(device.application_url),
                    capabilities: TargetCapabilities::dial(),
                });
            }
            Ok(None) => {
                trace!("Location {} is not a DIAL device", location);
            }
            Err(e) => {
                warn!("Failed to fetch DIAL device info from {}: {}", location, e);
            }
        }
    }

    Ok(targets)
}

/// Fetch the device descriptor XML and extract the `Application-URL` header.
async fn fetch_dial_device(
    client: &reqwest::Client,
    location: &str,
    usn: &str,
    responder: IpAddr,
) -> Result<Option<DialDevice>> {
    const MAX_DESCRIPTOR_BYTES: usize = 256 * 1024;

    let response = client
        .get(location)
        .send()
        .await
        .context("failed to fetch DIAL device descriptor")?;

    if !response.status().is_success() {
        return Ok(None);
    }

    // The Application-URL header tells us where to manage DIAL apps.
    let application_url = response
        .headers()
        .get("Application-URL")
        .or_else(|| response.headers().get("application-url"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim_end_matches('/').to_string());

    let application_url = match application_url {
        Some(url) => url,
        None => return Ok(None), // Not a DIAL device
    };

    // Read the body to extract friendly name and model.
    let body = response
        .text()
        .await
        .context("failed to read descriptor body")?;
    if body.len() > MAX_DESCRIPTOR_BYTES {
        anyhow::bail!("DIAL descriptor too large");
    }

    let friendly_name = extract_xml_element(&body, "friendlyName").unwrap_or_default();
    let model_name = extract_xml_element(&body, "modelName").unwrap_or_default();

    if friendly_name.is_empty() {
        return Ok(None);
    }

    Ok(Some(DialDevice {
        application_url,
        friendly_name,
        model_name,
        usn: usn.to_string(),
        address: responder,
    }))
}

/// Simple XML element text extractor (no full parser needed for descriptors).
fn extract_xml_element(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim().to_string())
}

/// Check whether a DIAL app is available on a device.
pub async fn query_dial_app(application_url: &str, app_name: &str) -> Result<DialAppStatus> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let url = format!("{application_url}/{app_name}");
    let resp = client.get(&url).send().await?;

    match resp.status().as_u16() {
        200 => {
            let body = resp.text().await.unwrap_or_default();
            let state = extract_xml_element(&body, "state").unwrap_or_default();
            Ok(match state.as_str() {
                "running" => DialAppStatus::Running,
                "stopped" => DialAppStatus::Stopped,
                "installable" => DialAppStatus::Installable,
                _ => DialAppStatus::Stopped,
            })
        }
        404 => Ok(DialAppStatus::NotAvailable),
        status => {
            anyhow::bail!("DIAL app query returned HTTP {}", status);
        }
    }
}

/// Launch a DIAL app on a device.
pub async fn launch_dial_app(
    application_url: &str,
    app_name: &str,
    body: Option<&str>,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let url = format!("{application_url}/{app_name}");
    let mut req = client.post(&url);
    if let Some(body) = body {
        req = req
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(body.to_string());
    }

    let resp = req.send().await?;

    match resp.status().as_u16() {
        201 | 200 => {
            debug!("DIAL app {} launched successfully", app_name);
            Ok(())
        }
        status => {
            anyhow::bail!("DIAL app launch failed with HTTP {}", status);
        }
    }
}

/// Stop a running DIAL app.
pub async fn stop_dial_app(application_url: &str, app_name: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let url = format!("{application_url}/{app_name}/run");
    let resp = client.delete(&url).send().await?;

    if resp.status().is_success() || resp.status().as_u16() == 404 {
        debug!("DIAL app {} stopped", app_name);
        Ok(())
    } else {
        anyhow::bail!("DIAL app stop failed with HTTP {}", resp.status());
    }
}

/// Status of a DIAL application on a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DialAppStatus {
    Running,
    Stopped,
    Installable,
    NotAvailable,
}

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_friendly_name_from_xml() {
        let xml = r#"<?xml version="1.0"?>
<root>
  <device>
    <friendlyName>Samsung TV</friendlyName>
    <modelName>UN55NU8000</modelName>
  </device>
</root>"#;
        assert_eq!(
            extract_xml_element(xml, "friendlyName"),
            Some("Samsung TV".to_string())
        );
        assert_eq!(
            extract_xml_element(xml, "modelName"),
            Some("UN55NU8000".to_string())
        );
    }

    #[test]
    fn extract_missing_element_returns_none() {
        let xml = "<root><name>Test</name></root>";
        assert_eq!(extract_xml_element(xml, "missing"), None);
    }

    #[test]
    fn extract_state_from_dial_response() {
        let xml = r#"<service><state>running</state></service>"#;
        assert_eq!(
            extract_xml_element(xml, "state"),
            Some("running".to_string())
        );
    }
}

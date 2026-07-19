//! mDNS/DNS-SD based discovery for Chromecast and AirPlay devices.
//!
//! Browses for `_googlecast._tcp` and `_airplay._tcp` service types
//! on the local network using the `mdns-sd` crate.

use super::{PlaybackTarget, TargetCapabilities, TargetKind};
use anyhow::{Context, Result};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::net::SocketAddr;
use std::time::Duration;
use tracing::{debug, warn};

/// Service type for Google Chromecast devices.
const CHROMECAST_SERVICE: &str = "_googlecast._tcp.local.";

/// Service type for Apple AirPlay video receivers.
const AIRPLAY_SERVICE: &str = "_airplay._tcp.local.";

/// How long to listen for mDNS responses.
const MDNS_BROWSE_DURATION: Duration = Duration::from_secs(4);

/// Maximum number of targets to collect per scan.
const MAX_TARGETS: usize = 64;

/// Discover Chromecast and AirPlay devices via mDNS.
pub async fn discover_mdns_targets(chromecast: bool, airplay: bool) -> Result<Vec<PlaybackTarget>> {
    if !chromecast && !airplay {
        return Ok(Vec::new());
    }

    // mDNS browsing is synchronous / uses its own threads, so we run it
    // on a blocking task to avoid starving the Tokio runtime.
    tokio::task::spawn_blocking(move || discover_blocking(chromecast, airplay))
        .await
        .context("mDNS browse task panicked")?
}

fn discover_blocking(chromecast: bool, airplay: bool) -> Result<Vec<PlaybackTarget>> {
    let daemon = ServiceDaemon::new().context("failed to create mDNS daemon")?;
    let mut targets = Vec::new();

    let mut receivers = Vec::new();

    if chromecast {
        let receiver = daemon
            .browse(CHROMECAST_SERVICE)
            .context("failed to browse for Chromecast services")?;
        receivers.push((receiver, TargetKind::Chromecast));
    }

    if airplay {
        let receiver = daemon
            .browse(AIRPLAY_SERVICE)
            .context("failed to browse for AirPlay services")?;
        receivers.push((receiver, TargetKind::AirPlay));
    }

    let deadline = std::time::Instant::now() + MDNS_BROWSE_DURATION;

    // Poll all receivers until the deadline.
    while std::time::Instant::now() < deadline && targets.len() < MAX_TARGETS {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        // Use a short poll interval so we can interleave receivers.
        let poll_timeout = remaining.min(Duration::from_millis(200));

        for (receiver, kind) in &receivers {
            match receiver.recv_timeout(poll_timeout) {
                Ok(event) => {
                    if let Some(target) = process_event(event, *kind) {
                        debug!(
                            kind = %kind,
                            name = %target.friendly_name,
                            address = %target.address,
                            "Discovered mDNS target"
                        );
                        targets.push(target);
                    }
                }
                Err(_) => {
                    // Timeout or disconnected on this receiver, try the next one.
                    continue;
                }
            }
        }
    }

    // Shut down the daemon (stops background threads).
    if let Err(e) = daemon.shutdown() {
        warn!("mDNS daemon shutdown error: {}", e);
    }

    Ok(targets)
}

/// Process a single mDNS service event into a `PlaybackTarget`.
fn process_event(event: ServiceEvent, kind: TargetKind) -> Option<PlaybackTarget> {
    match event {
        ServiceEvent::ServiceResolved(info) => {
            let addresses = info.get_addresses();
            let ip = addresses.iter().next()?;
            let port = info.get_port();
            let fullname = info.get_fullname().to_string();

            let friendly_name = extract_friendly_name(&fullname, kind);
            let model = info.get_property_val_str("md").map(|s| s.to_string());

            let capabilities = match kind {
                TargetKind::Chromecast => TargetCapabilities::chromecast(),
                TargetKind::AirPlay => TargetCapabilities::airplay(),
                _ => unreachable!("mDNS discovery only produces Chromecast or AirPlay targets"),
            };

            // Chromecast control port is always 8009 (Castv2 TLS).
            // AirPlay control port is the port from the service record (usually 7000).
            let control_port = match kind {
                TargetKind::Chromecast => 8009,
                _ => port,
            };

            Some(PlaybackTarget {
                id: fullname.clone(),
                friendly_name,
                kind,
                address: SocketAddr::new(ip.to_ip_addr(), control_port),
                model,
                control_url: None,
                capabilities,
            })
        }
        _ => None,
    }
}

/// Extract a human-friendly name from the mDNS full service name.
///
/// Chromecast names look like: `Chromecast-abc123._googlecast._tcp.local.`
/// AirPlay names look like: `Living Room._airplay._tcp.local.`
fn extract_friendly_name(fullname: &str, kind: TargetKind) -> String {
    let service_suffix = match kind {
        TargetKind::Chromecast => "._googlecast._tcp.local.",
        TargetKind::AirPlay => "._airplay._tcp.local.",
        _ => "._tcp.local.",
    };

    fullname
        .strip_suffix(service_suffix)
        .unwrap_or(fullname)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friendly_name_strips_chromecast_suffix() {
        let name = extract_friendly_name(
            "Living Room._googlecast._tcp.local.",
            TargetKind::Chromecast,
        );
        assert_eq!(name, "Living Room");
    }

    #[test]
    fn friendly_name_strips_airplay_suffix() {
        let name = extract_friendly_name("Apple TV._airplay._tcp.local.", TargetKind::AirPlay);
        assert_eq!(name, "Apple TV");
    }

    #[test]
    fn friendly_name_preserves_unknown_suffix() {
        let name = extract_friendly_name("Something Else", TargetKind::Chromecast);
        assert_eq!(name, "Something Else");
    }
}

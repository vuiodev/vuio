//! mDNS device discovery for Cast devices on the local network.
//!
//! Requires the `discovery` feature (enabled by default).

use std::collections::HashSet;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::types::DeviceInfo;

const SERVICE_TYPE: &str = "_googlecast._tcp.local.";

/// A stream of discovered Cast devices, yielding each device as it's found.
///
/// Created by [`discover`]. Implements an async iterator pattern.
pub struct DiscoveryStream {
    rx: mpsc::Receiver<DeviceInfo>,
    _task: tokio::task::JoinHandle<()>,
}

impl DiscoveryStream {
    /// Receive the next discovered device, or `None` when the timeout expires.
    pub async fn recv(&mut self) -> Option<DeviceInfo> {
        self.rx.recv().await
    }
}

use tokio::sync::mpsc;

/// Discover Cast devices as a stream, yielding each device as it's found.
///
/// Returns a [`DiscoveryStream`] that produces devices in real-time.
/// The stream closes after the `timeout` duration expires.
///
/// Unlike [`discover_devices`], this returns an error immediately if
/// the mDNS daemon fails to initialize.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> oxicast::Result<()> {
/// let mut stream = oxicast::discovery::discover(
///     std::time::Duration::from_secs(5)
/// )?;
/// while let Some(device) = stream.recv().await {
///     println!("Found: {} at {}:{}", device.name, device.ip, device.port);
/// }
/// # Ok(())
/// # }
/// ```
pub fn discover(timeout: Duration) -> Result<DiscoveryStream> {
    use mdns_sd::ServiceDaemon;

    // Initialize mDNS on the calling thread so errors propagate immediately
    let mdns = ServiceDaemon::new().map_err(|e| Error::Discovery(format!("mDNS daemon: {e}")))?;
    let receiver =
        mdns.browse(SERVICE_TYPE).map_err(|e| Error::Discovery(format!("mDNS browse: {e}")))?;

    let (tx, rx) = mpsc::channel(32);

    let _task = tokio::task::spawn_blocking(move || {
        discover_streaming(tx, mdns, receiver, timeout);
    });

    Ok(DiscoveryStream { rx, _task })
}

fn discover_streaming(
    tx: mpsc::Sender<DeviceInfo>,
    mdns: mdns_sd::ServiceDaemon,
    receiver: mdns_sd::Receiver<mdns_sd::ServiceEvent>,
    timeout: Duration,
) {
    use mdns_sd::ServiceEvent;

    let mut seen = HashSet::new();
    let deadline = std::time::Instant::now() + timeout;

    while std::time::Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(200)) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let ip = match info.get_addresses_v4().iter().next() {
                    Some(addr) => std::net::IpAddr::V4(*addr),
                    None => continue,
                };

                if !seen.insert(ip) {
                    continue;
                }

                let name =
                    info.get_property_val_str("fn").unwrap_or(info.get_fullname()).to_string();
                let model = info.get_property_val_str("md").map(String::from);
                let uuid = info.get_property_val_str("id").map(String::from);

                tracing::debug!("discovered: {name} at {ip}:{}", info.get_port());

                let device = DeviceInfo { name, ip, port: info.get_port(), model, uuid };
                if tx.blocking_send(device).is_err() {
                    break; // receiver dropped
                }
            }
            Ok(_) => {}
            Err(e) => {
                let err = format!("{e:?}");
                if err.contains("Timeout") {
                    continue;
                }
                break;
            }
        }
    }

    let _ = mdns.shutdown();
}

/// Discover Cast devices and collect them into a Vec.
///
/// Scans the local network for the given `timeout` duration.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> oxicast::Result<()> {
/// let devices = oxicast::discovery::discover_devices(
///     std::time::Duration::from_secs(3)
/// ).await?;
/// for device in &devices {
///     println!("{} at {}:{}", device.name, device.ip, device.port);
/// }
/// # Ok(())
/// # }
/// ```
pub async fn discover_devices(timeout: Duration) -> Result<Vec<DeviceInfo>> {
    tokio::task::spawn_blocking(move || discover_blocking(timeout))
        .await
        .map_err(|e| Error::Discovery(format!("discovery task: {e}")))?
}

fn discover_blocking(timeout: Duration) -> Result<Vec<DeviceInfo>> {
    use mdns_sd::{ServiceDaemon, ServiceEvent};

    let mdns = ServiceDaemon::new().map_err(|e| Error::Discovery(format!("mDNS daemon: {e}")))?;

    let receiver =
        mdns.browse(SERVICE_TYPE).map_err(|e| Error::Discovery(format!("mDNS browse: {e}")))?;

    let mut devices = Vec::new();
    let mut seen = HashSet::new();
    let deadline = std::time::Instant::now() + timeout;

    while std::time::Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(200)) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let ip = match info.get_addresses_v4().iter().next() {
                    Some(addr) => std::net::IpAddr::V4(*addr),
                    None => continue,
                };

                if !seen.insert(ip) {
                    continue;
                }

                let name =
                    info.get_property_val_str("fn").unwrap_or(info.get_fullname()).to_string();

                let model = info.get_property_val_str("md").map(String::from);
                let uuid = info.get_property_val_str("id").map(String::from);

                tracing::debug!("discovered: {name} at {ip}:{}", info.get_port());

                devices.push(DeviceInfo { name, ip, port: info.get_port(), model, uuid });
            }
            Ok(_) => {}
            Err(e) => {
                let err = format!("{e:?}");
                if err.contains("Timeout") {
                    continue;
                }
                break;
            }
        }
    }

    let _ = mdns.shutdown();
    Ok(devices)
}

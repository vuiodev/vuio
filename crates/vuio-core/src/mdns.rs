//! Advertise this server over mDNS / DNS-SD, so first-party clients can find
//! it without being told an address.
//!
//! SSDP already announces VuIO to DLNA controllers and remains the way those
//! clients discover it. This is the parallel announcement for everything that
//! looks the Bonjour way instead: the VuIO phone and tablet apps, and
//! `vuio-tower`.
//!
//! Two services are published:
//!
//! - `_vuio._tcp` — the first-party record. Its TXT entries carry the same
//!   identity SSDP publishes plus the paths a client needs, so one lookup is
//!   enough to start browsing.
//! - `_http._tcp` with `path=/` — the conventional record for a browsable web
//!   page, which makes the server show up in generic browsers such as
//!   `dns-sd -B _http._tcp`.
//!
//! Deliberately absent are `_webdav._tcp`, `_smb._tcp` and the rest of the set
//! third-party players browse for. Advertising those would make VLC and Infuse
//! list a server that cannot answer them, which is worse than not appearing at
//! all — those apps reach VuIO over SSDP.

use anyhow::{Context, Result};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::collections::HashMap;
use std::net::IpAddr;

const VUIO_SERVICE: &str = "_vuio._tcp.local.";
const HTTP_SERVICE: &str = "_http._tcp.local.";

/// What a client needs to know to start talking to this server.
pub struct ServerAdvertisement {
    pub uuid: String,
    pub name: String,
    pub ip: IpAddr,
    pub port: u16,
    /// Whether the management API and dashboard require a login.
    pub requires_auth: bool,
}

/// A live mDNS advertisement.
///
/// Dropping it withdraws both services. That matters: without an explicit
/// goodbye the record lingers in client caches for its full TTL, and clients
/// keep offering a server that is no longer there.
pub struct MdnsAdvertiser {
    daemon: ServiceDaemon,
    fullnames: Vec<String>,
}

impl Drop for MdnsAdvertiser {
    fn drop(&mut self) {
        for fullname in &self.fullnames {
            let _ = self.daemon.unregister(fullname);
        }
        let _ = self.daemon.shutdown();
    }
}

impl MdnsAdvertiser {
    /// Publish this server on the local network.
    pub fn start(server: &ServerAdvertisement) -> Result<Self> {
        let daemon = ServiceDaemon::new().context("starting the mDNS responder")?;

        // The instance name is what a user sees in a device list, so it is the
        // configured server name. mdns-sd escapes it; the host name has to be a
        // valid label either way, so it is derived from the UUID rather than
        // from free text a user typed.
        let host = format!("vuio-{}.local.", short_id(&server.uuid));

        let vuio = ServiceInfo::new(
            VUIO_SERVICE,
            &server.name,
            &host,
            server.ip,
            server.port,
            txt_records(server),
        )
        .context("describing the VuIO mDNS service")?;

        let http = ServiceInfo::new(
            HTTP_SERVICE,
            &server.name,
            &host,
            server.ip,
            server.port,
            HashMap::from([("path".to_string(), "/".to_string())]),
        )
        .context("describing the HTTP mDNS service")?;

        let mut fullnames = Vec::with_capacity(2);
        for service in [vuio, http] {
            let fullname = service.get_fullname().to_string();
            daemon
                .register(service)
                .with_context(|| format!("advertising {fullname}"))?;
            fullnames.push(fullname);
        }

        tracing::info!(
            name = %server.name,
            address = %server.ip,
            port = server.port,
            "Advertising over mDNS as _vuio._tcp and _http._tcp"
        );

        Ok(Self { daemon, fullnames })
    }
}

/// The TXT entries clients read. Keys are lowercase by DNS-SD convention.
fn txt_records(server: &ServerAdvertisement) -> HashMap<String, String> {
    HashMap::from([
        // The same identity SSDP puts in its USN, so a client that finds this
        // server both ways can tell it is one server and not two.
        ("uuid".to_string(), server.uuid.clone()),
        ("name".to_string(), server.name.clone()),
        ("version".to_string(), env!("CARGO_PKG_VERSION").to_string()),
        ("path".to_string(), "/".to_string()),
        ("dlna".to_string(), "/description.xml".to_string()),
        ("api".to_string(), "/api/media".to_string()),
        ("auth".to_string(), server.requires_auth.to_string()),
    ])
}

/// A short, DNS-label-safe fragment of the server UUID for the host name.
fn short_id(uuid: &str) -> String {
    uuid.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(12)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ServerAdvertisement {
        ServerAdvertisement {
            uuid: "4d696e69-444c-164e-9d41-000000000001".to_string(),
            name: "VuIO Test Server".to_string(),
            ip: "127.0.0.1".parse().unwrap(),
            port: 8080,
            requires_auth: false,
        }
    }

    #[test]
    fn host_name_is_a_valid_dns_label() {
        let host = short_id("4d696e69-444c-164e-9d41-000000000001");
        assert_eq!(host, "4d696e69444c");
        assert!(host.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn txt_carries_identity_and_paths() {
        let txt = txt_records(&sample());
        assert_eq!(
            txt.get("uuid").map(String::as_str),
            Some("4d696e69-444c-164e-9d41-000000000001")
        );
        assert_eq!(txt.get("dlna").map(String::as_str), Some("/description.xml"));
        assert_eq!(txt.get("api").map(String::as_str), Some("/api/media"));
        assert_eq!(txt.get("auth").map(String::as_str), Some("false"));
        assert_eq!(
            txt.get("version").map(String::as_str),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }

    /// Registering is not the same as being discoverable; browse for the
    /// service we just published and confirm it comes back with its TXT intact.
    #[tokio::test]
    async fn advertised_service_is_discoverable() {
        let server = sample();
        let Ok(advertiser) = MdnsAdvertiser::start(&server) else {
            eprintln!("mDNS unavailable in this environment; skipping");
            return;
        };

        let Ok(daemon) = ServiceDaemon::new() else {
            eprintln!("mDNS unavailable in this environment; skipping");
            return;
        };
        let receiver = daemon.browse(VUIO_SERVICE).unwrap();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut resolved = None;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, receiver.recv_async()).await {
            if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                // Match on this advertisement's own uuid, not on the name.
                // Anything containing "VuIO" used to do, which meant a real
                // VuIO running on the same network — a developer's own server,
                // most often — was picked up instead and failed the assertions
                // below against a uuid that was never ours.
                if info.get_property_val_str("uuid") == Some(server.uuid.as_str()) {
                    resolved = Some(info);
                    break;
                }
            }
        }
        drop(advertiser);

        let info = resolved.expect("the VuIO service was registered but never resolved");
        assert_eq!(info.get_port(), 8080);
        assert_eq!(info.get_property_val_str("uuid"), Some(server.uuid.as_str()));
        assert_eq!(info.get_property_val_str("api"), Some("/api/media"));
    }
}

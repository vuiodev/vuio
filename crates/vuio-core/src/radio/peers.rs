//! Finding the radio stations other VuIO servers are broadcasting.
//!
//! VuIO already announces itself over mDNS as `_vuio._tcp` — that is how the
//! phone apps and `vuio-tower` find a server without being told an address.
//! Nothing consumed that announcement until now. Browsing for it gives a list
//! of servers on the network; asking each one for `/api/radio/stations` gives
//! what it is playing. That endpoint is public for exactly this reason: a
//! station is meant to be listened to, so neither a peer nor a hi-fi should
//! need a login to find one.
//!
//! Servers that do not answer, answer slowly, or answer with something else are
//! dropped without comment. A neighbour being absent is the normal case, not an
//! error worth showing an operator.

use crate::http_client::HttpClient;
use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceEvent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

const VUIO_SERVICE: &str = "_vuio._tcp.local.";

/// How long to listen for mDNS answers. Long enough for a quiet network to
/// reply, short enough that a browser waiting on the tab does not notice.
const BROWSE_TIMEOUT: Duration = Duration::from_millis(1500);

/// How long a peer list stays good. The tab polls while it is open, and
/// re-browsing the network on every poll would be rude to it and to the network.
const CACHE_TTL: Duration = Duration::from_secs(20);

const FETCH_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_RESPONSE: usize = 256 * 1024;

/// A station as one server publishes it to anyone who asks.
///
/// This is the wire format of `GET /api/radio/stations`, so it is both what
/// this server serves and what it parses from its neighbours.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublishedStation {
    pub id: i64,
    pub name: String,
    pub genre: String,
    /// `mp3` or `aac`.
    pub codec: String,
    /// Absolute, so a listener can play it without knowing where it came from.
    pub stream_url: String,
    pub listeners: usize,
    pub uptime_secs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now_playing: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// What one server on the network is broadcasting.
#[derive(Clone, Debug, Serialize)]
pub struct PeerServer {
    pub uuid: String,
    pub name: String,
    pub address: String,
    /// Whether this is the server answering the request.
    pub is_self: bool,
    pub stations: Vec<PublishedStation>,
}

/// A server found over mDNS, before it has been asked anything.
#[derive(Clone, Debug)]
struct DiscoveredServer {
    uuid: String,
    name: String,
    address: SocketAddr,
}

/// The last set of neighbours, and when it was collected.
#[derive(Default)]
pub struct PeerCache {
    collected_at: Option<Instant>,
    peers: Vec<PeerServer>,
}

impl PeerCache {
    fn fresh(&self) -> Option<Vec<PeerServer>> {
        let collected_at = self.collected_at?;
        (collected_at.elapsed() < CACHE_TTL).then(|| self.peers.clone())
    }

    fn store(&mut self, peers: Vec<PeerServer>) {
        self.collected_at = Some(Instant::now());
        self.peers = peers;
    }
}

/// Browse the network and ask every VuIO server what it is broadcasting.
///
/// `own_uuid` is left out of the result: this server's own stations are
/// reported from memory rather than fetched from itself over the loopback.
pub async fn discover(own_uuid: &str) -> Vec<PeerServer> {
    let servers = match browse(own_uuid).await {
        Ok(servers) => servers,
        Err(error) => {
            tracing::debug!("Could not browse for VuIO servers: {error:#}");
            return Vec::new();
        }
    };

    let client = HttpClient::new(FETCH_TIMEOUT);
    let mut peers = Vec::new();
    for server in servers {
        match fetch_stations(&client, &server).await {
            Ok(stations) if !stations.is_empty() => peers.push(PeerServer {
                uuid: server.uuid,
                name: server.name,
                address: server.address.to_string(),
                is_self: false,
                stations,
            }),
            Ok(_) => {}
            Err(error) => {
                tracing::debug!(
                    server = %server.address,
                    "A VuIO server did not report its stations: {error:#}"
                );
            }
        }
    }
    peers.sort_by(|left, right| left.name.cmp(&right.name));
    peers
}

/// Collect the VuIO servers answering on this network.
async fn browse(own_uuid: &str) -> Result<Vec<DiscoveredServer>> {
    let daemon = ServiceDaemon::new()?;
    let receiver = daemon.browse(VUIO_SERVICE)?;
    let deadline = tokio::time::Instant::now() + BROWSE_TIMEOUT;
    let mut found: HashMap<String, DiscoveredServer> = HashMap::new();

    while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, receiver.recv_async()).await {
        let ServiceEvent::ServiceResolved(info) = event else {
            continue;
        };
        let Some(uuid) = info.get_property_val_str("uuid") else {
            continue;
        };
        // Our own record comes back too; this server's stations are already
        // known without asking.
        if uuid == own_uuid || found.contains_key(uuid) {
            continue;
        }
        let Some(ip) = info
            .get_addresses()
            .iter()
            .map(mdns_sd::ScopedIp::to_ip_addr)
            .find(is_routable)
        else {
            continue;
        };
        let name = info
            .get_property_val_str("name")
            .filter(|value| !value.is_empty())
            .unwrap_or(info.get_fullname())
            .to_owned();

        found.insert(
            uuid.to_owned(),
            DiscoveredServer {
                uuid: uuid.to_owned(),
                name,
                address: SocketAddr::new(ip, info.get_port()),
            },
        );
    }

    // Withdrawing the browse is what stops the daemon's background threads.
    let _ = daemon.shutdown();
    Ok(found.into_values().collect())
}

/// Loopback and link-local addresses reach nothing useful from here.
fn is_routable(address: &IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => !v4.is_loopback() && !v4.is_link_local() && !v4.is_unspecified(),
        IpAddr::V6(v6) => !v6.is_loopback() && !v6.is_unspecified(),
    }
}

async fn fetch_stations(
    client: &HttpClient,
    server: &DiscoveredServer,
) -> Result<Vec<PublishedStation>> {
    let uri: http::Uri = format!("http://{}/api/radio/stations", server.address).parse()?;
    let response = client.get(&uri, MAX_RESPONSE).await?;
    if !response.status.is_success() {
        anyhow::bail!("answered {}", response.status);
    }
    let mut stations: Vec<PublishedStation> = serde_json::from_slice(&response.body)?;

    // A peer builds its URLs from the address it believes it has, which is not
    // always the one we reached it on. The address that just worked is the one
    // a listener should use.
    for station in &mut stations {
        station.stream_url = rehost(&station.stream_url, server.address);
    }
    Ok(stations)
}

/// Point a peer's stream URL back at the address we reached that peer on.
fn rehost(url: &str, address: SocketAddr) -> String {
    match url.parse::<http::Uri>() {
        Ok(uri) => {
            let path = uri.path_and_query().map_or("/", |value| value.as_str());
            format!("http://{address}{path}")
        }
        Err(_) => url.to_owned(),
    }
}

/// The peer list, from cache when it is recent enough.
pub async fn cached(cache: &tokio::sync::Mutex<PeerCache>, own_uuid: &str) -> Vec<PeerServer> {
    if let Some(peers) = cache.lock().await.fresh() {
        return peers;
    }
    let peers = discover(own_uuid).await;
    cache.lock().await.store(peers.clone());
    peers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_peer_url_is_rewritten_to_the_address_that_answered() {
        let address: SocketAddr = "192.168.1.40:8080".parse().unwrap();
        assert_eq!(
            rehost("http://10.0.0.9:8080/api/radio/stations/3/stream", address),
            "http://192.168.1.40:8080/api/radio/stations/3/stream"
        );
    }

    #[test]
    fn an_unparseable_url_is_left_alone() {
        let address: SocketAddr = "192.168.1.40:8080".parse().unwrap();
        assert_eq!(rehost("not a url", address), "not a url");
    }

    #[test]
    fn addresses_that_reach_nothing_are_rejected() {
        assert!(!is_routable(&"127.0.0.1".parse().unwrap()));
        assert!(!is_routable(&"169.254.3.4".parse().unwrap()));
        assert!(is_routable(&"192.168.1.40".parse().unwrap()));
        assert!(is_routable(&"10.0.0.9".parse().unwrap()));
    }

    #[test]
    fn a_cache_expires() {
        let mut cache = PeerCache::default();
        assert!(cache.fresh().is_none(), "an empty cache has nothing to give");
        cache.store(Vec::new());
        assert!(cache.fresh().is_some());
        cache.collected_at = Some(Instant::now() - CACHE_TTL - Duration::from_secs(1));
        assert!(cache.fresh().is_none());
    }
}

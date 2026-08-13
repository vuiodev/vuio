//! Shared application state handed to every service and HTTP handler.
use crate::{
    config::AppConfig,
    database::DatabaseManager,
    platform::{filesystem::FileSystemManager, PlatformInfo},
};
use std::sync::Arc;

pub struct LiveConfig {
    config: std::sync::RwLock<Arc<AppConfig>>,
    /// Notifies subsystems that hold resources built from the configuration — the
    /// listener, the SSDP service — that they may need to rebuild.
    ///
    /// Published by `store` itself rather than by its callers. There are two writers,
    /// the admin API's eager save and the file watcher's echo of the same bytes, and
    /// only one of them goes through `ConfigManager`'s events; hanging the signal off
    /// the write makes both correct in either order with no coordination. A `watch` is
    /// level-triggered, so a subscriber asks "what should I be running" rather than
    /// "what happened", and cannot miss the final state by being slow.
    changes: tokio::sync::watch::Sender<Arc<AppConfig>>,
}

impl LiveConfig {
    pub fn new(config: Arc<AppConfig>) -> Self {
        let (changes, _) = tokio::sync::watch::channel(config.clone());
        Self {
            config: std::sync::RwLock::new(config),
            changes,
        }
    }

    pub fn load(&self) -> Arc<AppConfig> {
        self.config
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn store(&self, config: Arc<AppConfig>) {
        // The value first: a subscriber woken by the notification reads through
        // `current_config()`, and must not see the config it is being told to leave.
        *self
            .config
            .write()
            .unwrap_or_else(|error| error.into_inner()) = config.clone();
        // Equal configs raise nothing, which is what dedupes the eager save against
        // the watcher echo of the same bytes.
        self.changes.send_if_modified(|current| {
            if **current == *config {
                false
            } else {
                *current = config;
                true
            }
        });
    }

    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<Arc<AppConfig>> {
        self.changes.subscribe()
    }
}

/// The address the HTTP server is actually accepting on, as opposed to the one the
/// configuration asks for.
///
/// Those are not the same thing, and treating them as one is a live bug: a reload
/// publishes a new `server.port` into the config immediately, but the listener does not
/// move, so every `res` URL and SSDP `LOCATION` starts naming a port nothing is bound to.
/// Reading the bound address instead keeps what the server advertises equal to what it
/// answers on, whatever the file currently says.
#[derive(Debug)]
pub struct HttpBinding {
    /// Read on every URL that goes out. Cheaper than the config read it replaces.
    port: std::sync::atomic::AtomicU16,
    detail: std::sync::RwLock<BindingDetail>,
    /// Bumped every time the listener moves.
    ///
    /// Discovery subscribes to this rather than to the configuration, which makes the
    /// ordering structural: the advertisement can only be rebuilt after the address it
    /// will announce is real. Watching the config instead raced the rebind, and the
    /// loser announced the address the server had just left.
    generation: tokio::sync::watch::Sender<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct BindingDetail {
    /// `None` until the first successful bind.
    pub addr: Option<std::net::SocketAddr>,
    /// Set only while the configuration asks for an address the server could not take,
    /// so a settings screen can say "configured X, serving Y" after a page reload.
    pub desired: Option<String>,
    pub last_error: Option<String>,
}

impl HttpBinding {
    /// Seeded from the configured port before the first bind, so nothing reads a zero.
    pub fn new(configured_port: u16) -> Self {
        let (generation, _) = tokio::sync::watch::channel(0);
        Self {
            port: std::sync::atomic::AtomicU16::new(configured_port),
            detail: std::sync::RwLock::new(BindingDetail::default()),
            generation,
        }
    }

    /// Notifies on every move of the listener.
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
        self.generation.subscribe()
    }

    pub fn port(&self) -> u16 {
        self.port.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn detail(&self) -> BindingDetail {
        self.detail
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    /// Publish the address a listener is now accepting on.
    pub fn publish_serving(&self, addr: std::net::SocketAddr) {
        self.port
            .store(addr.port(), std::sync::atomic::Ordering::Relaxed);
        let mut detail = self
            .detail
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let moved = detail.addr != Some(addr);
        detail.addr = Some(addr);
        detail.desired = None;
        detail.last_error = None;
        drop(detail);
        if moved {
            // Sent last: it is the edge subscribers observe, so everything they will
            // read must already be in place.
            self.generation.send_modify(|generation| *generation += 1);
        }
    }

    /// Record that the configured address could not be taken. The published port is
    /// deliberately left alone: it still names the socket that is accepting.
    pub fn publish_failure(&self, desired: impl Into<String>, error: impl Into<String>) {
        let mut detail = self
            .detail
            .write()
            .unwrap_or_else(|error| error.into_inner());
        detail.desired = Some(desired.into());
        detail.last_error = Some(error.into());
    }
}

impl Default for HttpBinding {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Where this run's configuration came from, for handlers that want to change it.
///
/// `AppState` only ever held the parsed config, never its origin, so nothing served
/// over HTTP could locate the file it was loaded from.
#[derive(Clone, Debug, Default)]
pub struct ConfigSource {
    pub path: std::path::PathBuf,
    /// False when the file is a scratch copy that a restart discards, which today
    /// means only a container configured by environment variables. Edits to it would
    /// silently evaporate, so the admin API refuses them.
    pub durable: bool,
    /// Command-line settings layered over the file for this run. The file is still
    /// editable; these just win until the next start, and a settings screen has to say
    /// so rather than letting a saved value look like it took effect.
    pub overrides: crate::config::ConfigOverrides,
}

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub struct SoapCacheKey {
    pub object_id: String,
    pub starting_index: u32,
    pub requested_count: u32,
    pub client_profile: crate::web::client::DlnaClientProfile,
    pub content_update_id: u32,
    pub browse_epoch: u64,
}

#[derive(Clone)]
pub struct UpnpSubscription {
    pub callback_url: String,
    pub peer: std::net::IpAddr,
    pub generation: uuid::Uuid,
    pub expires_at: std::time::Instant,
    pub next_sequence: u32,
    pub consecutive_failures: u8,
    pub last_notification_at: std::time::Instant,
}

#[derive(Clone)]
pub struct McpClient {
    pub sender: tokio::sync::mpsc::Sender<String>,
    pub peer: std::net::IpAddr,
    pub expires_at: std::time::Instant,
}

pub struct AppState<D: DatabaseManager = crate::database::ActiveDatabase> {
    pub config: Arc<AppConfig>,
    pub live_config: Arc<LiveConfig>,
    pub config_source: Arc<ConfigSource>,
    /// Where the HTTP server is actually listening. Authoritative for every URL the
    /// server hands out; `config.server.port` is only what was asked for.
    pub http_binding: Arc<HttpBinding>,
    pub media_directories:
        Arc<tokio::sync::RwLock<Vec<crate::config::MonitoredDirectoryConfig>>>,
    pub unavailable_roots:
        Arc<tokio::sync::RwLock<std::collections::HashSet<std::path::PathBuf>>>,
    pub database: Arc<D>,
    pub auth: Arc<crate::web::auth::AuthState>,
    pub platform_info: Arc<PlatformInfo>,
    pub filesystem_manager: Arc<dyn FileSystemManager>,
    pub content_update_id: Arc<std::sync::atomic::AtomicU32>,
    pub web_metrics: Arc<crate::web::diagnostics::WebHandlerMetrics>,
    pub runtime_diagnostics: Arc<crate::platform::diagnostics::SystemDiagnosticsSampler>,
    pub lifecycle_stats: Arc<crate::lifecycle::ApplicationStats>,
    pub bookmarks: Arc<tokio::sync::Mutex<crate::runtime_state::BookmarkRegistry>>,
    pub log_file_path: std::path::PathBuf,
    pub browse_cache: Arc<tokio::sync::Mutex<crate::runtime_state::BrowseResponseCache>>,
    pub mcp_clients: Arc<tokio::sync::Mutex<std::collections::HashMap<String, McpClient>>>,
    pub active_monitors: Arc<
        tokio::sync::Mutex<
            std::collections::HashMap<
                String,
                (uuid::Uuid, tokio_util::sync::CancellationToken),
            >,
        >,
    >,
    pub active_casts: Arc<tokio::sync::Mutex<crate::runtime_state::ActiveCastRegistry>>,
    /// Progress of the online media info fetch, which the dashboard polls.
    ///
    /// A library run takes minutes to hours — the providers' rate limits see to
    /// that — so it cannot be the body of a request. The state lives here for the
    /// same reason `active_monitors` does: a handler starts the work, and later
    /// handlers need to report on it or stop it.
    #[cfg(feature = "mediainfo")]
    pub mediainfo_job: Arc<tokio::sync::Mutex<crate::mediainfo::MediaInfoJobState>>,
    #[cfg(feature = "casting")]
    pub discovered_tvs: Arc<crate::runtime_state::RendererCache>,
    pub upnp_subscriptions:
        Arc<tokio::sync::Mutex<std::collections::HashMap<String, UpnpSubscription>>>,
    pub radio_broadcast: Arc<crate::web::radio_broadcast::RadioBroadcastState>,
    pub cancellation: tokio_util::sync::CancellationToken,
    pub background_tasks: tokio_util::task::TaskTracker,
}

impl<D: DatabaseManager> Clone for AppState<D> {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            live_config: self.live_config.clone(),
            config_source: self.config_source.clone(),
            http_binding: self.http_binding.clone(),
            media_directories: self.media_directories.clone(),
            unavailable_roots: self.unavailable_roots.clone(),
            database: self.database.clone(),
            auth: self.auth.clone(),
            platform_info: self.platform_info.clone(),
            filesystem_manager: self.filesystem_manager.clone(),
            content_update_id: self.content_update_id.clone(),
            web_metrics: self.web_metrics.clone(),
            runtime_diagnostics: self.runtime_diagnostics.clone(),
            lifecycle_stats: self.lifecycle_stats.clone(),
            bookmarks: self.bookmarks.clone(),
            log_file_path: self.log_file_path.clone(),
            browse_cache: self.browse_cache.clone(),
            mcp_clients: self.mcp_clients.clone(),
            active_monitors: self.active_monitors.clone(),
            active_casts: self.active_casts.clone(),
            #[cfg(feature = "mediainfo")]
            mediainfo_job: self.mediainfo_job.clone(),
            #[cfg(feature = "casting")]
            discovered_tvs: self.discovered_tvs.clone(),
            upnp_subscriptions: self.upnp_subscriptions.clone(),
            radio_broadcast: self.radio_broadcast.clone(),
            cancellation: self.cancellation.clone(),
            background_tasks: self.background_tasks.clone(),
        }
    }
}

impl<D: DatabaseManager> AppState<D> {
    pub fn current_config(&self) -> Arc<AppConfig> {
        self.live_config.load()
    }
    /// Get the server's IP address using unified logic from platform_info
    pub fn get_server_ip(&self) -> String {
        // An explicit host address must win over container/interface auto-detection.
        if let Ok(host_ip) = std::env::var("VUIO_IP") {
            if !host_ip.is_empty() {
                return host_ip;
            }
        }

        // Check if server IP is explicitly configured (important for Docker)
        if let Some(server_ip) = &self.current_config().server.ip {
            if !server_ip.is_empty() && server_ip != "0.0.0.0" {
                return server_ip.clone();
            }
        }

        // Use the SSDP interface from config if it's a specific IP address
        match &self.current_config().network.interface_selection {
            crate::config::NetworkInterfaceConfig::Specific(interface) => {
                if interface.parse::<std::net::IpAddr>().is_ok() {
                    return interface.clone();
                }
                if let Some(selected) = self
                    .platform_info
                    .network_interfaces
                    .iter()
                    .find(|candidate| candidate.name == *interface && candidate.is_up)
                {
                    return selected.ip_address.to_string();
                }
            }
            _ => {
                // For Auto or All, fallback to server interface if it's not 0.0.0.0
                if self.current_config().server.interface != "0.0.0.0"
                    && !self.current_config().server.interface.is_empty()
                {
                    return self.current_config().server.interface.clone();
                }
            }
        }

        // Use the primary interface detected at startup instead of re-detecting
        if let Some(primary_interface) = self.platform_info.get_primary_interface() {
            return primary_interface.ip_address.to_string();
        }

        // Last resort
        tracing::warn!("Could not auto-detect IP, falling back to 127.0.0.1");
        "127.0.0.1".to_string()
    }

    /// Pick the local address whose route reaches a renderer. Explicit
    /// configuration still wins, which is required for host-networked
    /// containers where the kernel reports an internal address.
    pub async fn advertised_http_origin_for_peer(&self, peer_url: &str) -> String {
        let has_explicit_address = std::env::var("VUIO_IP")
            .is_ok_and(|value| !value.is_empty())
            || self
                .current_config()
                .server
                .ip
                .as_deref()
                .is_some_and(|value| !value.is_empty() && value != "0.0.0.0")
            || match &self.current_config().network.interface_selection {
                crate::config::NetworkInterfaceConfig::Specific(interface) => {
                    interface.parse::<std::net::IpAddr>().is_ok()
                        || self
                            .platform_info
                            .network_interfaces
                            .iter()
                            .any(|candidate| candidate.name == *interface && candidate.is_up)
                }
                _ => false,
            }
            || (!self.current_config().server.interface.is_empty()
                && self.current_config().server.interface != "0.0.0.0");
        if has_explicit_address {
            return self.advertised_http_origin();
        }

        let peer = peer_url
            .parse::<http::Uri>()
            .ok()
            .and_then(|url| url.host()?.parse::<std::net::IpAddr>().ok());
        if let Some(peer) = peer {
            let bind = match peer {
                std::net::IpAddr::V4(_) => "0.0.0.0:0",
                std::net::IpAddr::V6(_) => "[::]:0",
            };
            if let Ok(socket) = tokio::net::UdpSocket::bind(bind).await {
                if socket
                    .connect(std::net::SocketAddr::new(peer, 9))
                    .await
                    .is_ok()
                {
                    if let Ok(local) = socket.local_addr() {
                        let host = match local.ip() {
                            std::net::IpAddr::V4(ip) => ip.to_string(),
                            std::net::IpAddr::V6(ip) => format!("[{ip}]"),
                        };
                        return format!("http://{}:{}", host, self.http_binding.port());
                    }
                }
            }
        }
        self.advertised_http_origin()
    }

    /// Absolute HTTP origin advertised to DLNA clients. Request `Host`
    /// headers are deliberately excluded because they describe untrusted
    /// inbound routing, not this server's public identity.
    pub fn advertised_http_origin(&self) -> String {
        let address = self.get_server_ip();
        let host = address
            .parse::<std::net::IpAddr>()
            .map_or(address.clone(), |ip| match ip {
                std::net::IpAddr::V4(_) => ip.to_string(),
                std::net::IpAddr::V6(_) => format!("[{ip}]"),
            });
        format!("http://{}:{}", host, self.http_binding.port())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    /// The configured port and the bound port are different things, and a reload moves
    /// only the first. Before the binding was authoritative, changing `server.port` in
    /// the file made every `res` URL and SSDP LOCATION name a port nothing was listening
    /// on, while the listener stayed where it was.
    #[test]
    fn the_binding_follows_the_socket_not_the_config() {
        let binding = HttpBinding::new(8080);
        assert_eq!(binding.port(), 8080, "seeded before the first bind");

        let bound: SocketAddr = "0.0.0.0:9090".parse().unwrap();
        binding.publish_serving(bound);
        assert_eq!(binding.port(), 9090);
        assert_eq!(binding.detail().addr, Some(bound));

        // A configured address the server could not take must not move the published
        // port: it still names the socket that is accepting.
        binding.publish_failure("0.0.0.0:80", "permission denied");
        assert_eq!(binding.port(), 9090, "a failed bind never moves the URLs");
        let detail = binding.detail();
        assert_eq!(detail.addr, Some(bound));
        assert_eq!(detail.desired.as_deref(), Some("0.0.0.0:80"));
        assert!(detail.last_error.is_some());

        // Succeeding afterwards clears the standing warning.
        binding.publish_serving("0.0.0.0:8081".parse().unwrap());
        assert_eq!(binding.port(), 8081);
        assert!(binding.detail().desired.is_none());
        assert!(binding.detail().last_error.is_none());
    }

    /// The published port is whatever the socket actually took, which is not always the
    /// number that was asked for. Validation rejects `port = 0` in a config file, but the
    /// binding is the thing that would make it work if that ever changed, and asking for
    /// one here is the cheapest way to have a bound port differ from the requested one.
    #[tokio::test]
    async fn the_published_port_is_the_one_the_socket_took() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let actual = listener.local_addr().expect("addr");
        assert_ne!(actual.port(), 0);

        let binding = HttpBinding::new(0);
        binding.publish_serving(actual);
        assert_eq!(binding.port(), actual.port());
    }
}

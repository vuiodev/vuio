#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod config;
pub mod database;
pub mod error;
pub mod lifecycle;
pub mod logging;
pub mod media;
pub mod platform;
pub mod runtime_state;
pub mod ssdp;
pub mod tv_control;
pub mod watcher;
pub mod web;
pub mod natural_sort;

pub mod airplay;
pub mod chromecast;
pub mod dial;
pub mod discovery;

pub type DefaultDatabase = crate::database::redb::RedbDatabase;
pub type DefaultAppState = crate::state::AppState<DefaultDatabase>;

pub mod state {
    use crate::{
        config::AppConfig,
        database::DatabaseManager,
        platform::{filesystem::FileSystemManager, PlatformInfo},
    };
    use std::sync::Arc;

    pub struct LiveConfig(std::sync::RwLock<Arc<AppConfig>>);

    impl LiveConfig {
        pub fn new(config: Arc<AppConfig>) -> Self {
            Self(std::sync::RwLock::new(config))
        }

        pub fn load(&self) -> Arc<AppConfig> {
            self.0
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone()
        }

        pub fn store(&self, config: Arc<AppConfig>) {
            *self.0.write().unwrap_or_else(|error| error.into_inner()) = config;
        }
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

    pub struct AppState<D: DatabaseManager = crate::database::redb::RedbDatabase> {
        pub config: Arc<AppConfig>,
        pub live_config: Arc<LiveConfig>,
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
        pub discovered_tvs: Arc<crate::runtime_state::RendererCache>,
        pub discovery_service: Arc<crate::discovery::DiscoveryService>,
        pub upnp_subscriptions:
            Arc<tokio::sync::Mutex<std::collections::HashMap<String, UpnpSubscription>>>,
        pub cancellation: tokio_util::sync::CancellationToken,
        pub background_tasks: tokio_util::task::TaskTracker,
    }

    impl<D: DatabaseManager> Clone for AppState<D> {
        fn clone(&self) -> Self {
            Self {
                config: self.config.clone(),
                live_config: self.live_config.clone(),
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
                discovered_tvs: self.discovered_tvs.clone(),
                discovery_service: self.discovery_service.clone(),
                upnp_subscriptions: self.upnp_subscriptions.clone(),
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
            // Check if server IP is explicitly configured (important for Docker)
            if let Some(server_ip) = &self.config.server.ip {
                if !server_ip.is_empty() && server_ip != "0.0.0.0" {
                    return server_ip.clone();
                }
            }

            // Use the SSDP interface from config if it's a specific IP address
            match &self.config.network.interface_selection {
                crate::config::NetworkInterfaceConfig::Specific(ip) => {
                    return ip.clone();
                }
                _ => {
                    // For Auto or All, fallback to server interface if it's not 0.0.0.0
                    if self.config.server.interface != "0.0.0.0"
                        && !self.config.server.interface.is_empty()
                    {
                        return self.config.server.interface.clone();
                    }
                }
            }

            // Use the primary interface detected at startup instead of re-detecting
            if let Some(primary_interface) = self.platform_info.get_primary_interface() {
                return primary_interface.ip_address.to_string();
            }

            // Check if host IP is overridden via environment variable (for containers)
            if let Ok(host_ip) = std::env::var("VUIO_IP") {
                if !host_ip.is_empty() {
                    return host_ip;
                }
            }

            // Last resort
            tracing::warn!("Could not auto-detect IP, falling back to 127.0.0.1");
            "127.0.0.1".to_string()
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
            format!("http://{}:{}", host, self.current_config().server.port)
        }
    }
}

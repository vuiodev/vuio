pub mod config;
pub mod database;
pub mod error;
pub mod logging;
pub mod media;
pub mod platform;
pub mod ssdp;
pub mod tv_control;
pub mod watcher;
pub mod web;

pub mod state {
    use crate::{
        config::AppConfig,
        database::DatabaseManager,
        platform::{filesystem::FileSystemManager, PlatformInfo},
    };
    use std::sync::Arc;

    #[derive(Hash, PartialEq, Eq, Clone, Debug)]
    pub struct SoapCacheKey {
        pub object_id: String,
        pub starting_index: u32,
        pub requested_count: u32,
        pub client_profile: crate::web::client::DlnaClientProfile,
        pub content_update_id: u32,
    }

    #[derive(Clone)]
    pub struct UpnpSubscription {
        pub callback_url: String,
        pub expires_at: std::time::Instant,
        pub next_sequence: u32,
        pub consecutive_failures: u8,
    }

    pub struct AppState<D: DatabaseManager = crate::database::redb::RedbDatabase> {
        pub config: Arc<AppConfig>,
        pub media_directories:
            Arc<tokio::sync::RwLock<Vec<crate::config::MonitoredDirectoryConfig>>>,
        pub database: Arc<D>,
        pub platform_info: Arc<PlatformInfo>,
        pub filesystem_manager: Arc<dyn FileSystemManager>,
        pub content_update_id: Arc<std::sync::atomic::AtomicU32>,
        pub web_metrics: Arc<crate::web::handlers::WebHandlerMetrics>,
        pub bookmarks: Arc<tokio::sync::Mutex<std::collections::HashMap<i64, u32>>>,
        pub log_file_path: std::path::PathBuf,
        pub browse_cache:
            Arc<tokio::sync::Mutex<std::collections::HashMap<SoapCacheKey, axum::body::Bytes>>>,
        pub mcp_clients: Arc<
            tokio::sync::Mutex<
                std::collections::HashMap<String, tokio::sync::mpsc::Sender<String>>,
            >,
        >,
        pub active_monitors: Arc<
            tokio::sync::Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<()>>>,
        >,
        pub active_casts: Arc<
            tokio::sync::Mutex<std::collections::HashMap<String, (String, std::time::Instant)>>,
        >,
        pub discovered_tvs: Arc<tokio::sync::Mutex<std::collections::HashMap<String, String>>>,
        pub upnp_subscriptions:
            Arc<tokio::sync::Mutex<std::collections::HashMap<String, UpnpSubscription>>>,
    }

    impl<D: DatabaseManager> Clone for AppState<D> {
        fn clone(&self) -> Self {
            Self {
                config: self.config.clone(),
                media_directories: self.media_directories.clone(),
                database: self.database.clone(),
                platform_info: self.platform_info.clone(),
                filesystem_manager: self.filesystem_manager.clone(),
                content_update_id: self.content_update_id.clone(),
                web_metrics: self.web_metrics.clone(),
                bookmarks: self.bookmarks.clone(),
                log_file_path: self.log_file_path.clone(),
                browse_cache: self.browse_cache.clone(),
                mcp_clients: self.mcp_clients.clone(),
                active_monitors: self.active_monitors.clone(),
                active_casts: self.active_casts.clone(),
                discovered_tvs: self.discovered_tvs.clone(),
                upnp_subscriptions: self.upnp_subscriptions.clone(),
            }
        }
    }

    impl<D: DatabaseManager> AppState<D> {
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
    }
}

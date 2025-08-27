pub mod config;
pub mod database;
pub mod error;
pub mod logging;
pub mod media;
pub mod platform;
pub mod ssdp;
pub mod watcher;
pub mod web;

pub mod state {
    use crate::{
        config::AppConfig,
        database::DatabaseManager,
        platform::{filesystem::FileSystemManager, PlatformInfo},
    };
    use std::sync::Arc;

    #[derive(Clone)]
    pub struct AppState {
        pub config: Arc<AppConfig>,
        pub database: Arc<dyn DatabaseManager>,
        pub platform_info: Arc<PlatformInfo>,
        pub filesystem_manager: Arc<dyn FileSystemManager>,
        pub content_update_id: Arc<std::sync::atomic::AtomicU32>,
    }

    impl AppState {
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
                    if self.config.server.interface != "0.0.0.0" && !self.config.server.interface.is_empty() {
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
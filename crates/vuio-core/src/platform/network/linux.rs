use crate::platform::{
    network::{
        FirewallStatus, InterfaceStatus, NetworkDiagnostics, NetworkManager, SsdpConfig,
        SsdpSocket, LOOPBACK_IPV4, SSDP_MULTICAST_IPV4,
    },
    InterfaceType, NetworkInterface, PlatformError, PlatformResult,
};
use async_trait::async_trait;
use std::net::{IpAddr, SocketAddr};
use std::process::Command;
use tokio::net::UdpSocket;
use tracing::{debug, error, info, warn};

/// Linux-specific network manager implementation
pub struct LinuxNetworkManager {
    config: SsdpConfig,
    cached_interfaces: std::sync::Arc<tokio::sync::RwLock<Option<Vec<NetworkInterface>>>>,
}

impl LinuxNetworkManager {
    /// Create a new Linux network manager
    pub fn new() -> Self {
        Self {
            config: SsdpConfig::default(),
            cached_interfaces: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Create a new Linux network manager with custom configuration
    pub fn with_config(config: SsdpConfig) -> Self {
        Self {
            config,
            cached_interfaces: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Clear the cached network interfaces (forces re-detection on next call)
    pub async fn clear_interface_cache(&self) {
        let mut cached = self.cached_interfaces.write().await;
        *cached = None;
        debug!("Cleared network interface cache");
    }

    /// Check if running as root
    fn is_elevated(&self) -> bool {
        std::env::var("USER")
            .map(|user| user == "root")
            .unwrap_or(false)
            // SAFETY: `geteuid` takes no pointers and has no preconditions.
            || unsafe { libc::geteuid() == 0 }
    }

    /// Check if a port requires root privileges on Linux
    fn requires_elevation(&self, port: u16) -> bool {
        // Ports below 1024 require root privileges or CAP_NET_BIND_SERVICE capability
        port < 1024
    }

    /// Try to bind to a port with Linux-specific handling
    async fn try_bind_port_linux(&self, port: u16) -> PlatformResult<UdpSocket> {
        let socket_addr = SocketAddr::from(([0, 0, 0, 0], port));

        match UdpSocket::bind(socket_addr).await {
            Ok(socket) => {
                debug!("Successfully bound to port {} on Linux", port);

                // Set socket options for better multicast support
                if let Err(e) = self.configure_multicast_socket(&socket) {
                    warn!("Failed to configure multicast socket options: {}", e);
                }

                Ok(socket)
            }
            Err(e) => {
                if self.requires_elevation(port) && !self.is_elevated() {
                    warn!("Port {} requires root privileges on Linux", port);
                    Err(PlatformError::NetworkConfig(format!(
                        "Port {} requires root privileges on Linux. Please run with sudo or use a port >= 1024. Error: {}",
                        port, e
                    )))
                } else {
                    Err(PlatformError::NetworkConfig(format!(
                        "Failed to bind to port {} on Linux: {}",
                        port, e
                    )))
                }
            }
        }
    }

    /// Configure socket for optimal multicast support
    fn configure_multicast_socket(
        &self,
        socket: &UdpSocket,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let socket = socket2::SockRef::from(socket);
        socket.set_reuse_address(true)?;
        socket.set_multicast_ttl_v4(4)?;
        socket.set_multicast_loop_v4(true)?;

        debug!("Configured multicast socket options for optimal Docker compatibility");
        Ok(())
    }

    /// Detect Linux firewall status
    async fn detect_firewall_status(&self) -> FirewallStatus {
        let mut detected = false;
        let mut blocking_ssdp = None;
        let mut suggestions = Vec::new();

        // Check for common firewall tools
        let has_iptables = Command::new("which")
            .arg("iptables")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);

        let has_ufw = Command::new("which")
            .arg("ufw")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);

        let has_firewalld = Command::new("which")
            .arg("firewall-cmd")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);

        if has_ufw {
            // Check UFW status
            match Command::new("ufw").arg("status").output() {
                Ok(output) if output.status.success() => {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    detected = output_str.contains("Status: active");

                    if detected {
                        info!("UFW firewall detected and active");
                        blocking_ssdp = Some(true); // Assume it might block SSDP
                        suggestions.push("Check UFW rules: sudo ufw status verbose".to_string());
                        suggestions.push("Allow SSDP traffic: sudo ufw allow 1900/udp".to_string());
                        suggestions.push(
                            "Allow your HTTP server port: sudo ufw allow <port>/tcp".to_string(),
                        );
                    }
                }
                _ => {}
            }
        } else if has_firewalld {
            // Check firewalld status
            match Command::new("firewall-cmd").arg("--state").output() {
                Ok(output) if output.status.success() => {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    detected = output_str.trim() == "running";

                    if detected {
                        info!("firewalld detected and running");
                        blocking_ssdp = Some(true); // Assume it might block SSDP
                        suggestions.push(
                            "Check firewalld rules: sudo firewall-cmd --list-all".to_string(),
                        );
                        suggestions.push(
                            "Allow SSDP service: sudo firewall-cmd --add-service=ssdp --permanent"
                                .to_string(),
                        );
                        suggestions
                            .push("Reload firewalld: sudo firewall-cmd --reload".to_string());
                    }
                }
                _ => {}
            }
        } else if has_iptables {
            // Check iptables rules
            match Command::new("iptables").args(["-L", "-n"]).output() {
                Ok(output) if output.status.success() => {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    detected = !output_str.is_empty() && output_str.lines().count() > 3; // More than just headers

                    if detected {
                        info!("iptables rules detected");
                        // Check if there are DROP or REJECT rules
                        if output_str.contains("DROP") || output_str.contains("REJECT") {
                            blocking_ssdp = Some(true);
                        } else {
                            blocking_ssdp = Some(false);
                        }
                        suggestions.push("Check iptables rules: sudo iptables -L -n".to_string());
                        suggestions.push("Allow SSDP traffic: sudo iptables -A INPUT -p udp --dport 1900 -j ACCEPT".to_string());
                    }
                }
                _ => {}
            }
        }

        if !detected && (has_iptables || has_ufw || has_firewalld) {
            suggestions
                .push("Firewall tools detected but status unclear. Check manually.".to_string());
        }

        if detected {
            suggestions
                .push("Consider temporarily disabling firewall to test connectivity".to_string());
            suggestions
                .push("Ensure multicast traffic is allowed on your network interfaces".to_string());
        }

        FirewallStatus {
            detected,
            blocking_ssdp,
            suggestions,
        }
    }
}

mod discovery;
mod runtime;

#[cfg(test)]
mod tests;

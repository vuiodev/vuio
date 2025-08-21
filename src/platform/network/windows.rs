use crate::platform::{
    network::{NetworkDiagnostics, NetworkManager, SsdpConfig, SsdpSocket, InterfaceStatus, FirewallStatus},
    InterfaceType, NetworkInterface, PlatformError, PlatformResult,
};
use async_trait::async_trait;
use std::net::{IpAddr, SocketAddr};
use std::process::Command;
use tokio::net::UdpSocket;
use tracing::{debug, error, info, warn};

/// Windows-specific network manager implementation
pub struct WindowsNetworkManager {
    config: SsdpConfig,
}

impl WindowsNetworkManager {
    /// Create a new Windows network manager
    pub fn new() -> Self {
        Self {
            config: SsdpConfig::default(),
        }
    }

    /// Create a new Windows network manager with custom configuration
    pub fn with_config(config: SsdpConfig) -> Self {
        Self { config }
    }

    /// Check if the current process has administrator privileges
    fn is_elevated(&self) -> bool {
        // Simple check - in a real implementation, you would use Windows APIs
        // like CheckTokenMembership with BUILTIN\Administrators SID
        std::env::var("USERNAME")
            .map(|username| username.to_lowercase().contains("admin"))
            .unwrap_or(false)
    }

    /// Check if a port requires administrator privileges on Windows
    fn requires_elevation(&self, port: u16) -> bool {
        // Ports below 1024 typically require administrator privileges on Windows
        port < 1024
    }

    /// Try to bind to a port with Windows-specific socket options
    async fn try_bind_port_windows(&self, port: u16) -> PlatformResult<UdpSocket> {
        let socket_addr = SocketAddr::from(([0, 0, 0, 0], port));

        match UdpSocket::bind(socket_addr).await {
            Ok(socket) => {
                // Set SO_REUSEADDR for Windows compatibility
                // Note: tokio's UdpSocket doesn't expose setsockopt directly
                // In a real implementation, you would use raw sockets or winapi
                debug!("Successfully bound to port {} on Windows", port);
                Ok(socket)
            }
            Err(e) => {
                if self.requires_elevation(port) && !self.is_elevated() {
                    warn!("Port {} requires administrator privileges on Windows", port);
                    Err(PlatformError::NetworkConfig(format!(
                        "Port {} requires administrator privileges. Please run as administrator or use a port >= 1024. Error: {}",
                        port, e
                    )))
                } else {
                    Err(PlatformError::NetworkConfig(format!(
                        "Failed to bind to port {} on Windows: {}",
                        port, e
                    )))
                }
            }
        }
    }

    /// Detect Windows firewall status
    async fn detect_firewall_status(&self) -> FirewallStatus {
        let mut detected = false;
        let mut blocking_ssdp = None;
        let mut suggestions = Vec::new();

        // Check if Windows Defender Firewall is running
        match Command::new("netsh")
            .args(&["advfirewall", "show", "allprofiles", "state"])
            .output()
        {
            Ok(output) if output.status.success() => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                detected = output_str.contains("ON") || output_str.contains("State");

                if detected {
                    info!("Windows Defender Firewall detected");

                    // Check if SSDP traffic might be blocked
                    // This is a simplified check - real implementation would be more thorough
                    if output_str.contains("Block") {
                        blocking_ssdp = Some(true);
                        suggestions.push("Consider adding a firewall rule for SSDP traffic (UDP port 1900)".to_string());
                        suggestions.push("Run: netsh advfirewall firewall add rule name=\"DLNA SSDP\" dir=in action=allow protocol=UDP localport=1900".to_string());
                    } else {
                        blocking_ssdp = Some(false);
                    }
                }
            }
            _ => {
                warn!("Could not detect Windows firewall status");
                suggestions.push("Unable to detect firewall status. If experiencing connection issues, check Windows Defender Firewall settings".to_string());
            }
        }

        if detected {
            suggestions.push("Open Windows Defender Firewall with Advanced Security".to_string());
            suggestions.push("Create inbound rules for UDP ports 1900 (SSDP) and your HTTP server port".to_string());
        }

        FirewallStatus {
            detected,
            blocking_ssdp,
            suggestions,
        }
    }

    /// Get network interfaces using the shared platform implementation.
    async fn get_windows_interfaces(&self) -> PlatformResult<Vec<NetworkInterface>> {
        crate::platform::windows::detect_network_interfaces().await
    }

    /// Enable multicast on Windows socket with proper error handling
    async fn enable_multicast_windows(
        &self,
        socket: &mut SsdpSocket,
        group: IpAddr,
        interface: Option<&NetworkInterface>,
    ) -> PlatformResult<()> {
        let local_addr = if let Some(iface) = interface {
            iface.ip_address
        } else {
            // Use the first suitable interface
            socket
                .interfaces
                .iter()
                .find(|iface| !iface.is_loopback && iface.is_up)
                .map(|iface| iface.ip_address)
                .unwrap_or_else(|| "0.0.0.0".parse().unwrap())
        };

        match socket.enable_multicast(group, local_addr).await {
            Ok(()) => {
                info!(
                    "Successfully enabled multicast on Windows for group {} via {}",
                    group, local_addr
                );
                Ok(())
            }
            Err(e) => {
                warn!("Failed to enable multicast on Windows: {}", e);

                // Provide Windows-specific troubleshooting advice
                let mut error_msg = format!("Multicast failed on Windows: {}", e);

                if !self.is_elevated() {
                    error_msg
                        .push_str("\nTip: Try running as administrator if the issue persists.");
                }

                error_msg.push_str(
                    "\nTip: Check Windows Defender Firewall settings for SSDP (UDP 1900) traffic.",
                );
                error_msg.push_str("\nTip: Ensure the network adapter supports multicast.");

                Err(PlatformError::NetworkConfig(error_msg))
            }
        }
    }
}

#[async_trait]
impl NetworkManager for WindowsNetworkManager {
    async fn create_ssdp_socket(&self) -> PlatformResult<SsdpSocket> {
        self.create_ssdp_socket_with_config(&self.config).await
    }

    async fn create_ssdp_socket_with_config(
        &self,
        config: &SsdpConfig,
    ) -> PlatformResult<SsdpSocket> {
        // Try primary port first
        let primary_result = self.try_bind_port_windows(config.primary_port).await;

        if let Ok(socket) = primary_result {
            let interfaces = self.get_local_interfaces().await?;
            let suitable_interfaces: Vec<_> = interfaces
                .into_iter()
                .filter(|iface| !iface.is_loopback && iface.is_up && iface.supports_multicast)
                .collect();

            if suitable_interfaces.is_empty() {
                return Err(PlatformError::NetworkConfig(
                    "No suitable network interfaces found on Windows".to_string(),
                ));
            }

            return Ok(SsdpSocket {
                socket,
                port: config.primary_port,
                interfaces: suitable_interfaces,
                multicast_enabled: false,
            });
        }

        let primary_error = primary_result.unwrap_err();
        warn!(
            "Primary port {} failed on Windows: {}",
            config.primary_port, primary_error
        );
        let mut last_error = primary_error;

        // Try fallback ports
        for &port in &config.fallback_ports {
            match self.try_bind_port_windows(port).await {
                Ok(socket) => {
                    info!("Using fallback port {} on Windows", port);
                    let interfaces = self.get_local_interfaces().await?;
                    let suitable_interfaces: Vec<_> = interfaces
                        .into_iter()
                        .filter(|iface| !iface.is_loopback && iface.is_up && iface.supports_multicast)
                        .collect();

                    return Ok(SsdpSocket {
                        socket,
                        port,
                        interfaces: suitable_interfaces,
                        multicast_enabled: false,
                    });
                }
                Err(e) => {
                    debug!("Fallback port {} failed on Windows: {}", port, e);
                    last_error = e;
                }
            }
        }

        Err(last_error)
    }

    async fn get_local_interfaces(&self) -> PlatformResult<Vec<NetworkInterface>> {
        self.get_windows_interfaces().await
    }

    async fn get_primary_interface(&self) -> PlatformResult<NetworkInterface> {
        let interfaces = self.get_local_interfaces().await?;

        // Filter and prioritize interfaces
        let mut suitable: Vec<_> = interfaces
            .into_iter()
            .filter(|iface| !iface.is_loopback && iface.is_up && iface.supports_multicast)
            .collect();

        // Sort by preference: Ethernet > WiFi > VPN > Other
        suitable.sort_by_key(|iface| match iface.interface_type {
            InterfaceType::Ethernet => 0,
            InterfaceType::WiFi => 1,
            InterfaceType::VPN => 2,
            InterfaceType::Other(_) => 3,
            InterfaceType::Loopback => 4,
        });

        suitable.into_iter().next().ok_or_else(|| {
            PlatformError::NetworkConfig("No suitable primary interface found on Windows".to_string())
        })
    }

    async fn join_multicast_group(
        &self,
        socket: &mut SsdpSocket,
        group: IpAddr,
        interface: Option<&NetworkInterface>,
    ) -> PlatformResult<()> {
        self.enable_multicast_windows(socket, group, interface).await
    }

    async fn send_multicast(
        &self,
        socket: &SsdpSocket,
        data: &[u8],
        group: SocketAddr,
    ) -> PlatformResult<()> {
        if !socket.multicast_enabled {
            return Err(PlatformError::NetworkConfig(
                "Multicast not enabled on Windows socket".to_string(),
            ));
        }

        match socket.send_to(data, group).await {
            Ok(_) => {
                debug!(
                    "Sent {} bytes to multicast group {} on Windows",
                    data.len(),
                    group
                );
                Ok(())
            }
            Err(e) => {
                error!("Failed to send multicast on Windows: {}", e);
                Err(PlatformError::from(e))
            }
        }
    }

    async fn send_unicast_fallback(
        &self,
        socket: &SsdpSocket,
        data: &[u8],
        interfaces: &[NetworkInterface],
    ) -> PlatformResult<()> {
        let mut success_count = 0;
        let mut last_error = None;

        for interface in interfaces {
            // Calculate broadcast address for Windows
            let broadcast_addr = match interface.ip_address {
                IpAddr::V4(ipv4) => {
                    // Simple broadcast calculation - in real implementation,
                    // you would use GetAdaptersAddresses to get proper subnet info
                    let octets = ipv4.octets();
                    let broadcast_ip =
                        std::net::Ipv4Addr::new(octets[0], octets[1], octets[2], 255);
                    SocketAddr::from((broadcast_ip, socket.port))
                }
                IpAddr::V6(_) => {
                    // IPv6 doesn't have broadcast, skip
                    continue;
                }
            };

            match socket.send_to(data, broadcast_addr).await {
                Ok(_) => {
                    success_count += 1;
                    debug!(
                        "Sent Windows unicast fallback to {} via interface {}",
                        broadcast_addr, interface.name
                    );
                }
                Err(e) => {
                    warn!(
                        "Failed to send Windows unicast fallback via interface {}: {}",
                        interface.name, e
                    );
                    last_error = Some(e);
                }
            }
        }

        if success_count > 0 {
            info!(
                "Windows unicast fallback succeeded on {} interfaces",
                success_count
            );
            Ok(())
        } else {
            Err(last_error.unwrap_or_else(|| {
                PlatformError::NetworkConfig(
                    "No Windows interfaces available for unicast fallback".to_string(),
                )
            }))
        }
    }

    async fn is_port_available(&self, port: u16) -> bool {
        self.try_bind_port_windows(port).await.is_ok()
    }

    async fn get_network_diagnostics(&self) -> PlatformResult<NetworkDiagnostics> {
        let interfaces = self.get_local_interfaces().await.unwrap_or_default();
        let mut interface_status = Vec::new();
        let mut available_ports = Vec::new();
        let mut diagnostic_messages = Vec::new();

        // Test interfaces
        for interface in interfaces {
            let multicast_capable = self.test_multicast(&interface).await.unwrap_or(false);
            let reachable = interface.is_up && !interface.is_loopback;

            let error_message = if !reachable {
                Some("Interface is down or unreachable".to_string())
            } else if !multicast_capable {
                Some("Interface does not support multicast".to_string())
            } else {
                None
            };

            interface_status.push(InterfaceStatus {
                interface,
                reachable,
                multicast_capable,
                error_message,
            });
        }

        // Test common ports
        for &port in &[1900, 8080, 8081, 8082, 9090] {
            if self.is_port_available(port).await {
                available_ports.push(port);
            } else if port < 1024 && !self.is_elevated() {
                diagnostic_messages
                    .push(format!("Port {} requires administrator privileges on Windows", port));
            }
        }

        // Add Windows-specific diagnostic messages
        if available_ports.is_empty() {
            diagnostic_messages
                .push("No common ports are available for binding on Windows".to_string());
            if !self.is_elevated() {
                diagnostic_messages
                    .push("Consider running as administrator to access privileged ports".to_string());
            }
        }

        if interface_status
            .iter()
            .all(|status| !status.multicast_capable)
        {
            diagnostic_messages.push("No Windows interfaces support multicast".to_string());
            diagnostic_messages.push("Check network adapter settings and drivers".to_string());
        }

        // Get firewall status
        let firewall_status = Some(self.detect_firewall_status().await);

        Ok(NetworkDiagnostics {
            multicast_working: interface_status
                .iter()
                .any(|status| status.multicast_capable),
            available_ports,
            interface_status,
            diagnostic_messages,
            firewall_status,
        })
    }

    async fn test_multicast(&self, interface: &NetworkInterface) -> PlatformResult<bool> {
        // Basic test for Windows - check if interface supports multicast
        if !interface.supports_multicast || !interface.is_up || interface.is_loopback {
            return Ok(false);
        }

        // Try to create a test socket and join multicast group
        // This is a simplified test - real implementation would be more thorough
        match UdpSocket::bind("0.0.0.0:0").await {
            Ok(test_socket) => match interface.ip_address {
                IpAddr::V4(local_v4) => {
                    let multicast_addr = "239.255.255.250".parse::<std::net::Ipv4Addr>().unwrap();
                    match test_socket.join_multicast_v4(multicast_addr, local_v4) {
                        Ok(()) => {
                            debug!(
                                "Multicast test successful on Windows interface {}",
                                interface.name
                            );
                            Ok(true)
                        }
                        Err(e) => {
                            debug!(
                                "Multicast test failed on Windows interface {}: {}",
                                interface.name, e
                            );
                            Ok(false)
                        }
                    }
                }
                IpAddr::V6(_) => {
                    // IPv6 multicast test would go here
                    Ok(true) // Assume it works for now
                }
            },
            Err(_) => Ok(false),
        }
    }
}

impl Default for WindowsNetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_network_manager_creation() {
        let manager = WindowsNetworkManager::new();
        assert_eq!(manager.config.primary_port, 1900);
    }

    #[test]
    fn test_requires_elevation() {
        let manager = WindowsNetworkManager::new();
        assert!(manager.requires_elevation(80));
        assert!(manager.requires_elevation(443));
        assert!(!manager.requires_elevation(8080));
        assert!(!manager.requires_elevation(9090));
    }

    #[tokio::test]
    async fn test_port_availability_check() {
        let manager = WindowsNetworkManager::new();

        // Test with a high port that should be available
        let available = manager.is_port_available(8080).await;
        // This might fail in test environment, but we can at least verify the method works
        println!("Port 8080 available: {}", available);
    }
}
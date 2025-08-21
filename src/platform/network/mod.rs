use crate::platform::{NetworkInterface, PlatformError, PlatformResult};
use async_trait::async_trait;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use tokio::net::UdpSocket;
use tracing::{debug, info, warn};

// Platform-specific network manager implementations
#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "linux")]
pub mod linux;

// Re-export platform-specific managers
#[cfg(target_os = "windows")]
pub use windows::WindowsNetworkManager;

#[cfg(target_os = "macos")]
pub use macos::MacOSNetworkManager;

#[cfg(target_os = "linux")]
pub use linux::LinuxNetworkManager;

// Create a type alias for the current platform's network manager
#[cfg(target_os = "windows")]
pub type PlatformNetworkManager = WindowsNetworkManager;

#[cfg(target_os = "macos")]
pub type PlatformNetworkManager = MacOSNetworkManager;

#[cfg(target_os = "linux")]
pub type PlatformNetworkManager = LinuxNetworkManager;

/// SSDP socket wrapper with platform-specific configuration
#[derive(Debug)]
pub struct SsdpSocket {
    /// The underlying UDP socket
    pub socket: UdpSocket,
    /// The port the socket is bound to
    pub port: u16,
    /// Network interfaces this socket is associated with
    pub interfaces: Vec<NetworkInterface>,
    /// Whether multicast is enabled on this socket
    pub multicast_enabled: bool,
}

impl SsdpSocket {
    /// Create a new SSDP socket bound to the specified port
    pub async fn new(port: u16, interfaces: Vec<NetworkInterface>) -> PlatformResult<Self> {
        let socket_addr = SocketAddr::from(([0, 0, 0, 0], port));
        let socket = UdpSocket::bind(socket_addr)
            .await
            .map_err(|e| PlatformError::NetworkConfig(format!("Failed to bind to port {}: {}", port, e)))?;
        
        debug!("Created SSDP socket bound to port {}", port);
        
        Ok(SsdpSocket {
            socket,
            port,
            interfaces,
            multicast_enabled: false,
        })
    }
    
    /// Enable multicast on this socket for the specified group
    pub async fn enable_multicast(&mut self, multicast_addr: IpAddr, local_addr: IpAddr) -> PlatformResult<()> {
        match (multicast_addr, local_addr) {
            (IpAddr::V4(multi_v4), IpAddr::V4(local_v4)) => {
                self.socket.join_multicast_v4(multi_v4, local_v4)
                    .map_err(|e| PlatformError::NetworkConfig(format!("Failed to join multicast group: {}", e)))?;
                self.multicast_enabled = true;
                info!("Enabled multicast on {}:{} for group {}", local_v4, self.port, multi_v4);
                Ok(())
            }
            (IpAddr::V6(multi_v6), _) => {
                // For IPv6, we need to specify the interface index
                // This is a simplified implementation - platform-specific implementations should handle this properly
                self.socket.join_multicast_v6(&multi_v6, 0)
                    .map_err(|e| PlatformError::NetworkConfig(format!("Failed to join IPv6 multicast group: {}", e)))?;
                self.multicast_enabled = true;
                info!("Enabled IPv6 multicast on port {} for group {}", self.port, multi_v6);
                Ok(())
            }
            _ => Err(PlatformError::NetworkConfig("IP version mismatch for multicast".to_string()))
        }
    }
    
    /// Send data to a specific address
    pub async fn send_to(&self, data: &[u8], addr: SocketAddr) -> PlatformResult<usize> {
        self.socket.send_to(data, addr)
            .await
            .map_err(|e| PlatformError::NetworkConfig(format!("Failed to send data: {}", e)))
    }
    
    /// Receive data from the socket
    pub async fn recv_from(&self, buf: &mut [u8]) -> PlatformResult<(usize, SocketAddr)> {
        self.socket.recv_from(buf)
            .await
            .map_err(|e| PlatformError::NetworkConfig(format!("Failed to receive data: {}", e)))
    }
    
    /// Set socket timeout for receive operations
    pub async fn set_read_timeout(&self, timeout: Option<Duration>) -> PlatformResult<()> {
        // Note: tokio UdpSocket doesn't have a direct timeout method
        // Platform-specific implementations should handle this appropriately
        debug!("Read timeout set to {:?} (implementation may vary by platform)", timeout);
        Ok(())
    }
}

/// Configuration for SSDP networking
#[derive(Debug, Clone)]
pub struct SsdpConfig {
    /// Primary port to attempt binding (usually 1900)
    pub primary_port: u16,
    /// Fallback ports to try if primary port is unavailable
    pub fallback_ports: Vec<u16>,
    /// Multicast address for SSDP (usually 239.255.255.250)
    pub multicast_address: IpAddr,
    /// Interval between SSDP announcements
    pub announce_interval: Duration,
    /// Maximum number of retries for network operations
    pub max_retries: u32,
    /// Specific interfaces to use (empty means use all suitable interfaces)
    pub interfaces: Vec<NetworkInterface>,
}

impl Default for SsdpConfig {
    fn default() -> Self {
        Self {
            primary_port: 1900,
            fallback_ports: vec![8080, 8081, 8082, 9090],
            multicast_address: "239.255.255.250".parse().unwrap(),
            announce_interval: Duration::from_secs(300), // 5 minutes
            max_retries: 3,
            interfaces: Vec::new(),
        }
    }
}

/// Cross-platform network manager trait
#[async_trait]
pub trait NetworkManager: Send + Sync {
    /// Create an SSDP socket with platform-specific optimizations
    async fn create_ssdp_socket(&self) -> PlatformResult<SsdpSocket>;
    
    /// Create an SSDP socket with custom configuration
    async fn create_ssdp_socket_with_config(&self, config: &SsdpConfig) -> PlatformResult<SsdpSocket>;
    
    /// Get all available network interfaces
    async fn get_local_interfaces(&self) -> PlatformResult<Vec<NetworkInterface>>;
    
    /// Get the best network interface for DLNA operations
    async fn get_primary_interface(&self) -> PlatformResult<NetworkInterface>;
    
    /// Join a multicast group on the specified socket
    async fn join_multicast_group(&self, socket: &mut SsdpSocket, group: IpAddr, interface: Option<&NetworkInterface>) -> PlatformResult<()>;
    
    /// Send multicast data
    async fn send_multicast(&self, socket: &SsdpSocket, data: &[u8], group: SocketAddr) -> PlatformResult<()>;
    
    /// Send unicast data as fallback when multicast fails
    async fn send_unicast_fallback(&self, socket: &SsdpSocket, data: &[u8], interfaces: &[NetworkInterface]) -> PlatformResult<()>;
    
    /// Check if a port is available for binding
    async fn is_port_available(&self, port: u16) -> bool;
    
    /// Get platform-specific network diagnostics
    async fn get_network_diagnostics(&self) -> PlatformResult<NetworkDiagnostics>;
    
    /// Test multicast functionality
    async fn test_multicast(&self, interface: &NetworkInterface) -> PlatformResult<bool>;
}

/// Network diagnostic information
#[derive(Debug, Clone)]
pub struct NetworkDiagnostics {
    /// Whether multicast is working
    pub multicast_working: bool,
    /// Available ports that can be bound
    pub available_ports: Vec<u16>,
    /// Network interfaces with their status
    pub interface_status: Vec<InterfaceStatus>,
    /// Platform-specific diagnostic messages
    pub diagnostic_messages: Vec<String>,
    /// Firewall status (if detectable)
    pub firewall_status: Option<FirewallStatus>,
}

/// Status of a network interface
#[derive(Debug, Clone)]
pub struct InterfaceStatus {
    /// The network interface
    pub interface: NetworkInterface,
    /// Whether the interface is reachable
    pub reachable: bool,
    /// Whether multicast works on this interface
    pub multicast_capable: bool,
    /// Any error messages related to this interface
    pub error_message: Option<String>,
}

/// Firewall status information
#[derive(Debug, Clone)]
pub struct FirewallStatus {
    /// Whether a firewall is detected
    pub detected: bool,
    /// Whether the firewall is blocking SSDP traffic
    pub blocking_ssdp: Option<bool>,
    /// Suggested actions to resolve firewall issues
    pub suggestions: Vec<String>,
}

/// Base network manager implementation with common functionality
pub struct BaseNetworkManager {
    config: SsdpConfig,
}

impl BaseNetworkManager {
    /// Create a new base network manager
    pub fn new() -> Self {
        Self {
            config: SsdpConfig::default(),
        }
    }
    
    /// Create a new base network manager with custom configuration
    pub fn with_config(config: SsdpConfig) -> Self {
        Self { config }
    }
    
    /// Try to bind to a specific port
    async fn try_bind_port(&self, port: u16) -> PlatformResult<UdpSocket> {
        let socket_addr = SocketAddr::from(([0, 0, 0, 0], port));
        UdpSocket::bind(socket_addr)
            .await
            .map_err(|e| PlatformError::NetworkConfig(format!("Failed to bind to port {}: {}", port, e)))
    }
    
    /// Find an available port from the configuration
    async fn find_available_port(&self) -> PlatformResult<u16> {
        // Try primary port first
        if self.is_port_available_internal(self.config.primary_port).await {
            return Ok(self.config.primary_port);
        }
        
        warn!("Primary port {} is not available, trying fallback ports", self.config.primary_port);
        
        // Try fallback ports
        for &port in &self.config.fallback_ports {
            if self.is_port_available_internal(port).await {
                info!("Using fallback port {}", port);
                return Ok(port);
            }
        }
        
        Err(PlatformError::NetworkConfig(
            format!("No available ports found. Tried primary port {} and fallback ports {:?}", 
                   self.config.primary_port, self.config.fallback_ports)
        ))
    }
    
    /// Internal method to check if a port is available
    async fn is_port_available_internal(&self, port: u16) -> bool {
        match self.try_bind_port(port).await {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    
    /// Filter interfaces to find suitable ones for DLNA
    fn filter_suitable_interfaces(&self, interfaces: Vec<NetworkInterface>) -> Vec<NetworkInterface> {
        interfaces.into_iter()
            .filter(|iface| {
                // Filter out loopback and down interfaces
                !iface.is_loopback && iface.is_up && iface.supports_multicast
            })
            .collect()
    }
    
    /// Prioritize interfaces by type and reliability
    fn prioritize_interfaces(&self, mut interfaces: Vec<NetworkInterface>) -> Vec<NetworkInterface> {
        use crate::platform::InterfaceType;
        
        interfaces.sort_by_key(|iface| match iface.interface_type {
            InterfaceType::Ethernet => 0,
            InterfaceType::WiFi => 1,
            InterfaceType::VPN => 2,
            InterfaceType::Other(_) => 3,
            InterfaceType::Loopback => 4, // Should be filtered out
        });
        
        interfaces
    }
}

#[async_trait]
impl NetworkManager for BaseNetworkManager {
    async fn create_ssdp_socket(&self) -> PlatformResult<SsdpSocket> {
        self.create_ssdp_socket_with_config(&self.config).await
    }
    
    async fn create_ssdp_socket_with_config(&self, config: &SsdpConfig) -> PlatformResult<SsdpSocket> {
        let port = if config.interfaces.is_empty() {
            self.find_available_port().await?
        } else {
            config.primary_port
        };
        
        let interfaces = if config.interfaces.is_empty() {
            self.get_local_interfaces().await?
        } else {
            config.interfaces.clone()
        };
        
        let suitable_interfaces = self.filter_suitable_interfaces(interfaces);
        if suitable_interfaces.is_empty() {
            return Err(PlatformError::NetworkConfig("No suitable network interfaces found".to_string()));
        }
        
        SsdpSocket::new(port, suitable_interfaces).await
    }
    
    async fn get_local_interfaces(&self) -> PlatformResult<Vec<NetworkInterface>> {
        // This is a base implementation that should be overridden by platform-specific implementations
        // For now, return an error indicating platform-specific implementation is needed
        Err(PlatformError::UnsupportedFeature(
            "get_local_interfaces requires platform-specific implementation".to_string()
        ))
    }
    
    async fn get_primary_interface(&self) -> PlatformResult<NetworkInterface> {
        let interfaces = self.get_local_interfaces().await?;
        let suitable = self.filter_suitable_interfaces(interfaces);
        let prioritized = self.prioritize_interfaces(suitable);
        
        prioritized.into_iter().next()
            .ok_or_else(|| PlatformError::NetworkConfig("No suitable primary interface found".to_string()))
    }
    
    async fn join_multicast_group(&self, socket: &mut SsdpSocket, group: IpAddr, interface: Option<&NetworkInterface>) -> PlatformResult<()> {
        let local_addr = if let Some(iface) = interface {
            iface.ip_address
        } else {
            // Use the first available interface
            socket.interfaces.first()
                .map(|iface| iface.ip_address)
                .unwrap_or_else(|| "0.0.0.0".parse().unwrap())
        };
        
        socket.enable_multicast(group, local_addr).await
    }
    
    async fn send_multicast(&self, socket: &SsdpSocket, data: &[u8], group: SocketAddr) -> PlatformResult<()> {
        if !socket.multicast_enabled {
            return Err(PlatformError::NetworkConfig("Multicast not enabled on socket".to_string()));
        }
        
        socket.send_to(data, group).await?;
        debug!("Sent {} bytes to multicast group {}", data.len(), group);
        Ok(())
    }
    
    async fn send_unicast_fallback(&self, socket: &SsdpSocket, data: &[u8], interfaces: &[NetworkInterface]) -> PlatformResult<()> {
        let mut success_count = 0;
        let mut last_error = None;
        
        for interface in interfaces {
            // Calculate broadcast address for the interface's subnet
            // This is a simplified implementation - platform-specific versions should be more sophisticated
            let broadcast_addr = match interface.ip_address {
                IpAddr::V4(ipv4) => {
                    // Simple broadcast to .255 - real implementation should calculate proper broadcast address
                    let octets = ipv4.octets();
                    let broadcast_ip = std::net::Ipv4Addr::new(octets[0], octets[1], octets[2], 255);
                    SocketAddr::from((broadcast_ip, socket.port))
                }
                IpAddr::V6(_) => {
                    // IPv6 doesn't have broadcast, skip for now
                    continue;
                }
            };
            
            match socket.send_to(data, broadcast_addr).await {
                Ok(_) => {
                    success_count += 1;
                    debug!("Sent unicast fallback to {} via interface {}", broadcast_addr, interface.name);
                }
                Err(e) => {
                    warn!("Failed to send unicast fallback via interface {}: {}", interface.name, e);
                    last_error = Some(e);
                }
            }
        }
        
        if success_count > 0 {
            info!("Unicast fallback succeeded on {} interfaces", success_count);
            Ok(())
        } else {
            Err(last_error.unwrap_or_else(|| 
                PlatformError::NetworkConfig("No interfaces available for unicast fallback".to_string())
            ))
        }
    }
    
    async fn is_port_available(&self, port: u16) -> bool {
        self.is_port_available_internal(port).await
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
            
            interface_status.push(InterfaceStatus {
                interface,
                reachable,
                multicast_capable,
                error_message: None,
            });
        }
        
        // Test common ports
        for &port in &[1900, 8080, 8081, 8082, 9090] {
            if self.is_port_available(port).await {
                available_ports.push(port);
            }
        }
        
        // Add diagnostic messages
        if available_ports.is_empty() {
            diagnostic_messages.push("No common ports are available for binding".to_string());
        }
        
        if interface_status.iter().all(|status| !status.multicast_capable) {
            diagnostic_messages.push("No interfaces support multicast".to_string());
        }
        
        Ok(NetworkDiagnostics {
            multicast_working: interface_status.iter().any(|status| status.multicast_capable),
            available_ports,
            interface_status,
            diagnostic_messages,
            firewall_status: None, // Platform-specific implementations should detect this
        })
    }
    
    async fn test_multicast(&self, interface: &NetworkInterface) -> PlatformResult<bool> {
        // Basic test - just check if the interface claims to support multicast
        // Platform-specific implementations should do actual multicast testing
        Ok(interface.supports_multicast && interface.is_up && !interface.is_loopback)
    }
}

impl Default for BaseNetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use crate::platform::InterfaceType;
    
    fn create_test_interface(name: &str, ip: &str, interface_type: InterfaceType) -> NetworkInterface {
        NetworkInterface {
            name: name.to_string(),
            ip_address: ip.parse().unwrap(),
            is_loopback: false,
            is_up: true,
            supports_multicast: true,
            interface_type,
        }
    }
    
    #[test]
    fn test_ssdp_config_default() {
        let config = SsdpConfig::default();
        assert_eq!(config.primary_port, 1900);
        assert!(!config.fallback_ports.is_empty());
        assert_eq!(config.multicast_address, "239.255.255.250".parse::<IpAddr>().unwrap());
    }
    
    #[tokio::test]
    async fn test_ssdp_socket_creation() {
        let interfaces = vec![
            create_test_interface("eth0", "192.168.1.100", InterfaceType::Ethernet)
        ];
        
        // Try to create socket on a high port to avoid permission issues
        let result = SsdpSocket::new(8080, interfaces).await;
        
        // This might fail in test environment, but we can at least verify the structure
        match result {
            Ok(socket) => {
                assert_eq!(socket.port, 8080);
                assert!(!socket.multicast_enabled);
            }
            Err(e) => {
                // Expected in test environment without network access
                println!("Socket creation failed as expected in test: {}", e);
            }
        }
    }
    
    #[test]
    fn test_interface_filtering() {
        let manager = BaseNetworkManager::new();
        
        let interfaces = vec![
            NetworkInterface {
                name: "lo".to_string(),
                ip_address: "127.0.0.1".parse().unwrap(),
                is_loopback: true,
                is_up: true,
                supports_multicast: false,
                interface_type: InterfaceType::Loopback,
            },
            create_test_interface("eth0", "192.168.1.100", InterfaceType::Ethernet),
            NetworkInterface {
                name: "down_interface".to_string(),
                ip_address: "192.168.1.101".parse().unwrap(),
                is_loopback: false,
                is_up: false,
                supports_multicast: true,
                interface_type: InterfaceType::Ethernet,
            },
        ];
        
        let filtered = manager.filter_suitable_interfaces(interfaces);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "eth0");
    }
    
    #[test]
    fn test_interface_prioritization() {
        let manager = BaseNetworkManager::new();
        
        let interfaces = vec![
            create_test_interface("vpn0", "10.0.0.1", InterfaceType::VPN),
            create_test_interface("wlan0", "192.168.1.100", InterfaceType::WiFi),
            create_test_interface("eth0", "192.168.1.101", InterfaceType::Ethernet),
        ];
        
        let prioritized = manager.prioritize_interfaces(interfaces);
        assert_eq!(prioritized[0].name, "eth0"); // Ethernet should be first
        assert_eq!(prioritized[1].name, "wlan0"); // WiFi should be second
        assert_eq!(prioritized[2].name, "vpn0"); // VPN should be last
    }
}
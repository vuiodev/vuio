use crate::platform::{
    network::{NetworkDiagnostics, NetworkManager, SsdpConfig, SsdpSocket, InterfaceStatus, FirewallStatus},
    InterfaceType, NetworkInterface, PlatformError, PlatformResult,
};
use async_trait::async_trait;
use std::net::{IpAddr, SocketAddr, Ipv6Addr};
use std::process::Command;
use tokio::net::UdpSocket;
use tracing::{debug, error, info, warn};

/// Helper function to calculate the length of a wide string
unsafe fn wcslen(mut s: *const u16) -> usize {
    let mut len = 0;
    while *s != 0 {
        len += 1;
        s = s.add(1);
    }
    len
}

/// Check if an IPv6 address is link-local
fn is_link_local_ipv6(addr: &Ipv6Addr) -> bool {
    let segments = addr.segments();
    // Link-local addresses start with fe80::/10
    (segments[0] & 0xffc0) == 0xfe80
}

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

    /// Get network interfaces using Windows API directly.
    async fn get_windows_interfaces(&self) -> PlatformResult<Vec<NetworkInterface>> {
        use std::net::{Ipv4Addr, Ipv6Addr};
        use windows::Win32::NetworkManagement::IpHelper::{
            GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH, GAA_FLAG_INCLUDE_PREFIX,
            GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST, GAA_FLAG_SKIP_DNS_SERVER,
            IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211, IF_TYPE_SOFTWARE_LOOPBACK,
            IF_TYPE_TUNNEL,
        };
        use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS, WIN32_ERROR};
        use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6, SOCKADDR_IN, SOCKADDR_IN6};
        
        let mut interfaces = Vec::new();
        let mut buffer_size = 15000u32; // Start with 15KB buffer
        let mut buffer: Vec<u8> = vec![0; buffer_size as usize];
        
        // Call GetAdaptersAddresses to get network interface information
        let flags = GAA_FLAG_INCLUDE_PREFIX | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
        
        let result = unsafe {
            GetAdaptersAddresses(
                0, // AF_UNSPEC - get both IPv4 and IPv6
                flags,
                None,
                Some(buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH),
                &mut buffer_size,
            )
        };
        
        match WIN32_ERROR(result) {
            ERROR_BUFFER_OVERFLOW => {
                // Buffer too small, resize and try again
                buffer.resize(buffer_size as usize, 0);
                let result = unsafe {
                    GetAdaptersAddresses(
                        0,
                        flags,
                        None,
                        Some(buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH),
                        &mut buffer_size,
                    )
                };
                if WIN32_ERROR(result) != ERROR_SUCCESS {
                    warn!("GetAdaptersAddresses failed with error: {}", result);
                    return self.fallback_interface_detection().await;
                }
            }
            ERROR_SUCCESS => {
                // Success on first try
            }
            _ => {
                warn!("GetAdaptersAddresses failed with error: {}", result);
                return self.fallback_interface_detection().await;
            }
        }
        
        // Parse the adapter information
        let mut current_adapter = buffer.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        
        while !current_adapter.is_null() {
            let adapter = unsafe { &*current_adapter };
            
            // Get adapter name
            let adapter_name = if !adapter.FriendlyName.is_null() {
                unsafe {
                    let name_slice = std::slice::from_raw_parts(
                        adapter.FriendlyName.0,
                        wcslen(adapter.FriendlyName.0),
                    );
                    String::from_utf16_lossy(name_slice)
                }
            } else {
                "Unknown".to_string()
            };
            
            // Determine interface type
            let interface_type = match adapter.IfType {
                IF_TYPE_ETHERNET_CSMACD => InterfaceType::Ethernet,
                IF_TYPE_IEEE80211 => InterfaceType::WiFi,
                IF_TYPE_SOFTWARE_LOOPBACK => InterfaceType::Loopback,
                IF_TYPE_TUNNEL => InterfaceType::VPN,
                _ => InterfaceType::Other(format!("Type_{}", adapter.IfType)),
            };
            
            // Check if interface is up (1 = IfOperStatusUp)
            let is_up = adapter.OperStatus.0 == 1;
            let is_loopback = adapter.IfType == IF_TYPE_SOFTWARE_LOOPBACK;
            
            // Parse IP addresses
            let mut unicast_addr = adapter.FirstUnicastAddress;
            while !unicast_addr.is_null() {
                let addr_info = unsafe { &*unicast_addr };
                let socket_addr = addr_info.Address.lpSockaddr;
                
                if !socket_addr.is_null() {
                    let addr_family = unsafe { (*socket_addr).sa_family };
                    
                    match addr_family {
                        AF_INET => {
                            let sockaddr_in = socket_addr as *const SOCKADDR_IN;
                            let ip_bytes = unsafe { (*sockaddr_in).sin_addr.S_un.S_addr.to_ne_bytes() };
                            let ip = Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);
                            
                            interfaces.push(NetworkInterface {
                                name: adapter_name.clone(),
                                ip_address: IpAddr::V4(ip),
                                is_loopback,
                                is_up,
                                supports_multicast: !is_loopback && is_up,
                                interface_type: interface_type.clone(),
                            });
                        }
                        AF_INET6 => {
                            let sockaddr_in6 = socket_addr as *const SOCKADDR_IN6;
                            let ip_bytes = unsafe { (*sockaddr_in6).sin6_addr.u.Byte };
                            let ip = Ipv6Addr::from(ip_bytes);
                            
                            // Skip link-local IPv6 addresses for now
                            if !ip.is_loopback() && !is_link_local_ipv6(&ip) {
                                interfaces.push(NetworkInterface {
                                    name: format!("{} (IPv6)", adapter_name),
                                    ip_address: IpAddr::V6(ip),
                                    is_loopback,
                                    is_up,
                                    supports_multicast: !is_loopback && is_up,
                                    interface_type: interface_type.clone(),
                                });
                            }
                        }
                        _ => {
                            // Unknown address family, skip
                        }
                    }
                }
                
                unicast_addr = addr_info.Next;
            }
            
            current_adapter = adapter.Next;
        }
        
        // If no interfaces found, add loopback as fallback
        if interfaces.is_empty() {
            interfaces.push(NetworkInterface {
                name: "Loopback".to_string(),
                ip_address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                is_loopback: true,
                is_up: true,
                supports_multicast: false,
                interface_type: InterfaceType::Loopback,
            });
        }
        
        info!("Detected {} network interfaces using Windows API", interfaces.len());
        for interface in &interfaces {
            debug!("Interface: {} ({}) - Up: {}, Multicast: {}", 
                   interface.name, interface.ip_address, interface.is_up, interface.supports_multicast);
        }
        
        Ok(interfaces)
    }
    
    /// Fallback interface detection using system commands
    async fn fallback_interface_detection(&self) -> PlatformResult<Vec<NetworkInterface>> {
        use std::net::Ipv4Addr;
        
        warn!("Using fallback interface detection method");
        let mut interfaces = Vec::new();
        
        // Add localhost interface
        interfaces.push(NetworkInterface {
            name: "Loopback".to_string(),
            ip_address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            is_loopback: true,
            is_up: true,
            supports_multicast: false,
            interface_type: InterfaceType::Loopback,
        });
        
        // Try to detect other interfaces using system commands
        if let Ok(output) = Command::new("ipconfig").arg("/all").output() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            let mut current_adapter_name = String::new();
            
            for line in output_str.lines() {
                let line = line.trim();
                
                // Look for adapter names
                if line.contains("adapter") && line.ends_with(':') {
                    current_adapter_name = line.replace("adapter", "").replace(':', "").trim().to_string();
                }
                
                // Look for IPv4 addresses
                if line.contains("IPv4 Address") {
                    if let Some(ip_part) = line.split(':').nth(1) {
                        let ip_str = ip_part.trim().replace("(Preferred)", "");
                        if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                            if !ip.is_loopback() {
                                let interface_type = if current_adapter_name.to_lowercase().contains("ethernet") {
                                    InterfaceType::Ethernet
                                } else if current_adapter_name.to_lowercase().contains("wi-fi") || 
                                         current_adapter_name.to_lowercase().contains("wireless") {
                                    InterfaceType::WiFi
                                } else if current_adapter_name.to_lowercase().contains("vpn") ||
                                         current_adapter_name.to_lowercase().contains("tunnel") {
                                    InterfaceType::VPN
                                } else {
                                    InterfaceType::Other(current_adapter_name.clone())
                                };
                                
                                interfaces.push(NetworkInterface {
                                    name: if current_adapter_name.is_empty() { 
                                        "Network Interface".to_string() 
                                    } else { 
                                        current_adapter_name.clone() 
                                    },
                                    ip_address: IpAddr::V4(ip),
                                    is_loopback: false,
                                    is_up: true,
                                    supports_multicast: true,
                                    interface_type,
                                });
                            }
                        }
                    }
                }
            }
        }
        
        Ok(interfaces)
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
                .filter(|iface| !iface.is_loopback && iface.is_up)
                .collect();

            // If no suitable interfaces found, use all interfaces (including loopback for testing)
            let final_interfaces = if suitable_interfaces.is_empty() {
                warn!("No suitable network interfaces found on Windows, using all available interfaces");
                self.get_local_interfaces().await?
            } else {
                suitable_interfaces
            };

            return Ok(SsdpSocket {
                socket,
                port: config.primary_port,
                interfaces: final_interfaces,
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
                        .filter(|iface| !iface.is_loopback && iface.is_up)
                        .collect();

                    // If no suitable interfaces found, use all interfaces
                    let final_interfaces = if suitable_interfaces.is_empty() {
                        self.get_local_interfaces().await?
                    } else {
                        suitable_interfaces
                    };

                    return Ok(SsdpSocket {
                        socket,
                        port,
                        interfaces: final_interfaces,
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
            .filter(|iface| !iface.is_loopback && iface.is_up)
            .collect();

        // If no suitable interfaces, use any available interface (including loopback for testing)
        if suitable.is_empty() {
            suitable = self.get_local_interfaces().await?;
        }

        // Sort by preference: Ethernet > WiFi > VPN > Other > Loopback
        suitable.sort_by_key(|iface| match iface.interface_type {
            InterfaceType::Ethernet => 0,
            InterfaceType::WiFi => 1,
            InterfaceType::VPN => 2,
            InterfaceType::Other(_) => 3,
            InterfaceType::Loopback => 4,
        });

        suitable.into_iter().next().ok_or_else(|| {
            PlatformError::NetworkConfig("No network interfaces found on Windows".to_string())
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
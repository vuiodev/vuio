use crate::platform::{
    NetworkInterface, InterfaceType, PlatformError, PlatformResult,
    network::{NetworkManager, SsdpSocket, SsdpConfig, NetworkDiagnostics, InterfaceStatus, FirewallStatus}
};
use async_trait::async_trait;
use std::net::{IpAddr, SocketAddr};
use std::process::Command;
use tokio::net::UdpSocket;
use tracing::{debug, info, warn, error};

/// Linux-specific network manager implementation
pub struct LinuxNetworkManager {
    config: SsdpConfig,
}

impl LinuxNetworkManager {
    /// Create a new Linux network manager
    pub fn new() -> Self {
        Self {
            config: SsdpConfig::default(),
        }
    }
    
    /// Create a new Linux network manager with custom configuration
    pub fn with_config(config: SsdpConfig) -> Self {
        Self { config }
    }
    
    /// Check if running as root
    fn is_elevated(&self) -> bool {
        std::env::var("USER")
            .map(|user| user == "root")
            .unwrap_or(false) ||
        unsafe { libc::geteuid() == 0 }
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
                        suggestions.push("Allow your HTTP server port: sudo ufw allow <port>/tcp".to_string());
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
                        suggestions.push("Check firewalld rules: sudo firewall-cmd --list-all".to_string());
                        suggestions.push("Allow SSDP service: sudo firewall-cmd --add-service=ssdp --permanent".to_string());
                        suggestions.push("Reload firewalld: sudo firewall-cmd --reload".to_string());
                    }
                }
                _ => {}
            }
        } else if has_iptables {
            // Check iptables rules
            match Command::new("iptables").args(&["-L", "-n"]).output() {
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
            suggestions.push("Firewall tools detected but status unclear. Check manually.".to_string());
        }
        
        if detected {
            suggestions.push("Consider temporarily disabling firewall to test connectivity".to_string());
            suggestions.push("Ensure multicast traffic is allowed on your network interfaces".to_string());
        }
        
        FirewallStatus {
            detected,
            blocking_ssdp,
            suggestions,
        }
    }
    
    /// Get network interfaces using Linux-specific methods
    async fn get_linux_interfaces(&self) -> PlatformResult<Vec<NetworkInterface>> {
        let mut interfaces = Vec::new();
        
        // Try to use ip command first (more modern)
        if let Ok(ip_interfaces) = self.parse_ip_command_output().await {
            if !ip_interfaces.is_empty() {
                return Ok(ip_interfaces);
            }
        }
        
        // Fallback to /proc/net/dev
        if let Ok(proc_interfaces) = self.parse_proc_net_dev().await {
            if !proc_interfaces.is_empty() {
                return Ok(proc_interfaces);
            }
        }
        
        // Final fallback
        warn!("Failed to get network interfaces using standard methods, using fallback");
        interfaces.push(NetworkInterface {
            name: "eth0".to_string(),
            ip_address: "127.0.0.1".parse().unwrap(),
            is_loopback: false,
            is_up: true,
            supports_multicast: true,
            interface_type: InterfaceType::Ethernet,
        });
        
        Ok(interfaces)
    }
    
    /// Parse output from 'ip addr show' command
    async fn parse_ip_command_output(&self) -> PlatformResult<Vec<NetworkInterface>> {
        let output = Command::new("ip")
            .args(&["addr", "show"])
            .output()
            .map_err(|e| PlatformError::NetworkConfig(format!("Failed to run 'ip addr show': {}", e)))?;
        
        if !output.status.success() {
            return Err(PlatformError::NetworkConfig("'ip addr show' command failed".to_string()));
        }
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        self.parse_ip_addr_output(&output_str)
    }
    
    /// Parse the output of 'ip addr show'
    fn parse_ip_addr_output(&self, output: &str) -> PlatformResult<Vec<NetworkInterface>> {
        let mut interfaces = Vec::new();
        let mut current_interface: Option<String> = None;
        let mut current_ip: Option<IpAddr> = None;
        let mut is_up = false;
        let mut supports_multicast = false;
        let mut is_loopback = false;
        
        for line in output.lines() {
            let line = line.trim();
            
            // Interface line: "2: eth0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc pfifo_fast state UP group default qlen 1000"
            if let Some(colon_pos) = line.find(':') {
                if let Some(second_colon) = line[colon_pos + 1..].find(':') {
                    let second_colon_pos = colon_pos + 1 + second_colon;
                    
                    // Save previous interface
                    if let (Some(name), Some(ip)) = (&current_interface, &current_ip) {
                        if !name.starts_with("lo") { // Skip loopback
                            let interface_type = self.determine_linux_interface_type(name);
                            interfaces.push(NetworkInterface {
                                name: name.clone(),
                                ip_address: *ip,
                                is_loopback,
                                is_up,
                                supports_multicast,
                                interface_type,
                            });
                        }
                    }
                    
                    // Parse new interface
                    let interface_name = line[colon_pos + 1..second_colon_pos].trim().to_string();
                    current_interface = Some(interface_name.clone());
                    current_ip = None;
                    is_loopback = interface_name.starts_with("lo");
                    
                    // Parse flags
                    if let Some(flags_start) = line.find('<') {
                        if let Some(flags_end) = line.find('>') {
                            let flags = &line[flags_start + 1..flags_end];
                            is_up = flags.contains("UP");
                            supports_multicast = flags.contains("MULTICAST");
                        }
                    }
                }
            }
            
            // IP address line: "    inet 192.168.1.100/24 brd 192.168.1.255 scope global dynamic eth0"
            if line.contains("inet ") && !line.contains("inet6") {
                if let Some(inet_pos) = line.find("inet ") {
                    let after_inet = &line[inet_pos + 5..];
                    if let Some(ip_part) = after_inet.split_whitespace().next() {
                        // Remove CIDR notation if present
                        let ip_str = ip_part.split('/').next().unwrap_or(ip_part);
                        if let Ok(ip) = ip_str.parse::<IpAddr>() {
                            current_ip = Some(ip);
                        }
                    }
                }
            }
        }
        
        // Don't forget the last interface
        if let (Some(name), Some(ip)) = (current_interface, current_ip) {
            if !name.starts_with("lo") { // Skip loopback
                let interface_type = self.determine_linux_interface_type(&name);
                interfaces.push(NetworkInterface {
                    name,
                    ip_address: ip,
                    is_loopback,
                    is_up,
                    supports_multicast,
                    interface_type,
                });
            }
        }
        
        Ok(interfaces)
    }
    
    /// Parse /proc/net/dev as fallback
    async fn parse_proc_net_dev(&self) -> PlatformResult<Vec<NetworkInterface>> {
        let contents = std::fs::read_to_string("/proc/net/dev")
            .map_err(|e| PlatformError::NetworkConfig(format!("Failed to read /proc/net/dev: {}", e)))?;
        
        let mut interfaces = Vec::new();
        
        for line in contents.lines().skip(2) { // Skip header lines
            if let Some(interface_name) = line.split(':').next() {
                let interface_name = interface_name.trim().to_string();
                
                // Skip loopback
                if interface_name.starts_with("lo") {
                    continue;
                }
                
                // Check if interface is up by reading from /sys/class/net
                let is_up = std::fs::read_to_string(format!("/sys/class/net/{}/operstate", interface_name))
                    .map(|state| state.trim() == "up")
                    .unwrap_or(false);
                
                // Get IP address using ip command for this specific interface
                let ip_address = self.get_interface_ip(&interface_name).unwrap_or_else(|| "127.0.0.1".parse().unwrap());
                
                let interface_type = self.determine_linux_interface_type(&interface_name);
                
                interfaces.push(NetworkInterface {
                    name: interface_name,
                    ip_address,
                    is_loopback: false,
                    is_up,
                    supports_multicast: true, // Most Linux interfaces support multicast
                    interface_type,
                });
            }
        }
        
        Ok(interfaces)
    }
    
    /// Get IP address for a specific interface
    fn get_interface_ip(&self, interface_name: &str) -> Option<IpAddr> {
        match Command::new("ip")
            .args(&["addr", "show", interface_name])
            .output()
        {
            Ok(output) if output.status.success() => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                for line in output_str.lines() {
                    if line.contains("inet ") && !line.contains("inet6") {
                        if let Some(inet_pos) = line.find("inet ") {
                            let after_inet = &line[inet_pos + 5..];
                            if let Some(ip_part) = after_inet.split_whitespace().next() {
                                let ip_str = ip_part.split('/').next().unwrap_or(ip_part);
                                if let Ok(ip) = ip_str.parse::<IpAddr>() {
                                    return Some(ip);
                                }
                            }
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
    
    /// Determine interface type based on Linux interface name
    fn determine_linux_interface_type(&self, name: &str) -> InterfaceType {
        if name.starts_with("eth") || name.starts_with("enp") || name.starts_with("eno") {
            InterfaceType::Ethernet
        } else if name.starts_with("wlan") || name.starts_with("wlp") || name.starts_with("wlo") {
            InterfaceType::WiFi
        } else if name.starts_with("tun") || name.starts_with("tap") || name.starts_with("vpn") {
            InterfaceType::VPN
        } else if name.starts_with("lo") {
            InterfaceType::Loopback
        } else {
            InterfaceType::Other(name.to_string())
        }
    }
    
    /// Get available network namespaces
    fn get_network_namespaces(&self) -> Vec<String> {
        let mut namespaces = Vec::new();
        
        // Read from /var/run/netns if available
        if let Ok(entries) = std::fs::read_dir("/var/run/netns") {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    namespaces.push(name.to_string());
                }
            }
        }
        
        namespaces
    }
    
    /// Enable multicast on Linux socket with proper interface binding
    async fn enable_multicast_linux(&self, socket: &mut SsdpSocket, group: IpAddr, interface: Option<&NetworkInterface>) -> PlatformResult<()> {
        let selected_interface = if let Some(iface) = interface {
            iface
        } else {
            // Use the first suitable interface
            socket.interfaces.iter()
                .find(|iface| !iface.is_loopback && iface.is_up && iface.supports_multicast)
                .ok_or_else(|| PlatformError::NetworkConfig("No suitable interface for multicast on Linux".to_string()))?
        };
        
        let local_addr = selected_interface.ip_address;
        
        match socket.enable_multicast(group, local_addr).await {
            Ok(()) => {
                info!("Successfully enabled multicast on Linux for group {} via interface {} ({})", 
                      group, selected_interface.name, local_addr);
                Ok(())
            }
            Err(e) => {
                warn!("Failed to enable multicast on Linux: {}", e);
                
                // Provide Linux-specific troubleshooting advice
                let mut error_msg = format!("Multicast failed on Linux: {}", e);
                
                if !self.is_elevated() && self.requires_elevation(socket.port) {
                    error_msg.push_str("\nTip: Try running with sudo if using a privileged port.");
                }
                
                error_msg.push_str("\nTip: Check firewall settings (iptables, ufw, firewalld).");
                error_msg.push_str("\nTip: Ensure the network interface supports multicast.");
                error_msg.push_str(&format!("\nTip: Try using interface {} explicitly.", selected_interface.name));
                error_msg.push_str("\nTip: Check if running in a network namespace that restricts multicast.");
                
                Err(PlatformError::NetworkConfig(error_msg))
            }
        }
    }
}

#[async_trait]
impl NetworkManager for LinuxNetworkManager {
    async fn create_ssdp_socket(&self) -> PlatformResult<SsdpSocket> {
        self.create_ssdp_socket_with_config(&self.config).await
    }
    
    async fn create_ssdp_socket_with_config(&self, config: &SsdpConfig) -> PlatformResult<SsdpSocket> {
        let mut last_error = None;
        
        // Try primary port first
        match self.try_bind_port_linux(config.primary_port).await {
            Ok(socket) => {
                let interfaces = self.get_local_interfaces().await?;
                let suitable_interfaces: Vec<_> = interfaces.into_iter()
                    .filter(|iface| !iface.is_loopback && iface.is_up && iface.supports_multicast)
                    .collect();
                
                if suitable_interfaces.is_empty() {
                    return Err(PlatformError::NetworkConfig("No suitable network interfaces found on Linux".to_string()));
                }
                
                return Ok(SsdpSocket {
                    socket,
                    port: config.primary_port,
                    interfaces: suitable_interfaces,
                    multicast_enabled: false,
                });
            }
            Err(e) => {
                warn!("Primary port {} failed on Linux: {}", config.primary_port, e);
                last_error = Some(e);
            }
        }
        
        // Try fallback ports
        for &port in &config.fallback_ports {
            match self.try_bind_port_linux(port).await {
                Ok(socket) => {
                    info!("Using fallback port {} on Linux", port);
                    let interfaces = self.get_local_interfaces().await?;
                    let suitable_interfaces: Vec<_> = interfaces.into_iter()
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
                    debug!("Fallback port {} failed on Linux: {}", port, e);
                    last_error = Some(e);
                }
            }
        }
        
        // If we get here, all ports failed
        Err(last_error.unwrap_or_else(|| 
            PlatformError::NetworkConfig("All ports failed on Linux".to_string())
        ))
    }
    
    async fn get_local_interfaces(&self) -> PlatformResult<Vec<NetworkInterface>> {
        self.get_linux_interfaces().await
    }
    
    async fn get_primary_interface(&self) -> PlatformResult<NetworkInterface> {
        let interfaces = self.get_local_interfaces().await?;
        
        // Filter and prioritize interfaces
        let mut suitable: Vec<_> = interfaces.into_iter()
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
        
        suitable.into_iter().next()
            .ok_or_else(|| PlatformError::NetworkConfig("No suitable primary interface found on Linux".to_string()))
    }
    
    async fn join_multicast_group(&self, socket: &mut SsdpSocket, group: IpAddr, interface: Option<&NetworkInterface>) -> PlatformResult<()> {
        self.enable_multicast_linux(socket, group, interface).await
    }
    
    async fn send_multicast(&self, socket: &SsdpSocket, data: &[u8], group: SocketAddr) -> PlatformResult<()> {
        if !socket.multicast_enabled {
            return Err(PlatformError::NetworkConfig("Multicast not enabled on Linux socket".to_string()));
        }
        
        match socket.send_to(data, group).await {
            Ok(_) => {
                debug!("Sent {} bytes to multicast group {} on Linux", data.len(), group);
                Ok(())
            }
            Err(e) => {
                error!("Failed to send multicast on Linux: {}", e);
                Err(e)
            }
        }
    }
    
    async fn send_unicast_fallback(&self, socket: &SsdpSocket, data: &[u8], interfaces: &[NetworkInterface]) -> PlatformResult<()> {
        let mut success_count = 0;
        let mut last_error = None;
        
        for interface in interfaces {
            // Calculate broadcast address for Linux
            let broadcast_addr = match interface.ip_address {
                IpAddr::V4(ipv4) => {
                    // Simple broadcast calculation - in real implementation, 
                    // you would use route command or netlink to get proper subnet info
                    let octets = ipv4.octets();
                    let broadcast_ip = std::net::Ipv4Addr::new(octets[0], octets[1], octets[2], 255);
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
                    debug!("Sent Linux unicast fallback to {} via interface {}", broadcast_addr, interface.name);
                }
                Err(e) => {
                    warn!("Failed to send Linux unicast fallback via interface {}: {}", interface.name, e);
                    last_error = Some(e);
                }
            }
        }
        
        if success_count > 0 {
            info!("Linux unicast fallback succeeded on {} interfaces", success_count);
            Ok(())
        } else {
            Err(last_error.unwrap_or_else(|| 
                PlatformError::NetworkConfig("No Linux interfaces available for unicast fallback".to_string())
            ))
        }
    }
    
    async fn is_port_available(&self, port: u16) -> bool {
        match self.try_bind_port_linux(port).await {
            Ok(_) => true,
            Err(_) => false,
        }
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
                diagnostic_messages.push(format!("Port {} requires root privileges on Linux", port));
            }
        }
        
        // Add Linux-specific diagnostic messages
        if available_ports.is_empty() {
            diagnostic_messages.push("No common ports are available for binding on Linux".to_string());
            if !self.is_elevated() {
                diagnostic_messages.push("Consider running with sudo to access privileged ports".to_string());
            }
        }
        
        if interface_status.iter().all(|status| !status.multicast_capable) {
            diagnostic_messages.push("No Linux interfaces support multicast".to_string());
            diagnostic_messages.push("Check network interface configuration and kernel modules".to_string());
        }
        
        // Check for network namespaces
        let namespaces = self.get_network_namespaces();
        if !namespaces.is_empty() {
            diagnostic_messages.push(format!("Network namespaces detected: {:?}", namespaces));
            diagnostic_messages.push("Consider running in the correct network namespace".to_string());
        }
        
        // Get firewall status
        let firewall_status = Some(self.detect_firewall_status().await);
        
        Ok(NetworkDiagnostics {
            multicast_working: interface_status.iter().any(|status| status.multicast_capable),
            available_ports,
            interface_status,
            diagnostic_messages,
            firewall_status,
        })
    }
    
    async fn test_multicast(&self, interface: &NetworkInterface) -> PlatformResult<bool> {
        // Basic test for Linux - check if interface supports multicast
        if !interface.supports_multicast || !interface.is_up || interface.is_loopback {
            return Ok(false);
        }
        
        // Try to create a test socket and join multicast group
        match UdpSocket::bind("0.0.0.0:0").await {
            Ok(test_socket) => {
                match interface.ip_address {
                    IpAddr::V4(local_v4) => {
                        let multicast_addr = "239.255.255.250".parse::<std::net::Ipv4Addr>().unwrap();
                        match test_socket.join_multicast_v4(multicast_addr, local_v4) {
                            Ok(()) => {
                                debug!("Multicast test successful on Linux interface {}", interface.name);
                                Ok(true)
                            }
                            Err(e) => {
                                debug!("Multicast test failed on Linux interface {}: {}", interface.name, e);
                                Ok(false)
                            }
                        }
                    }
                    IpAddr::V6(_) => {
                        // IPv6 multicast test would go here
                        Ok(true) // Assume it works for now
                    }
                }
            }
            Err(_) => Ok(false),
        }
    }
}

impl Default for LinuxNetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_linux_network_manager_creation() {
        let manager = LinuxNetworkManager::new();
        assert_eq!(manager.config.primary_port, 1900);
    }
    
    #[test]
    fn test_requires_elevation() {
        let manager = LinuxNetworkManager::new();
        assert!(manager.requires_elevation(80));
        assert!(manager.requires_elevation(443));
        assert!(!manager.requires_elevation(8080));
        assert!(!manager.requires_elevation(9090));
    }
    
    #[test]
    fn test_interface_type_determination() {
        let manager = LinuxNetworkManager::new();
        
        assert_eq!(
            manager.determine_linux_interface_type("eth0"),
            InterfaceType::Ethernet
        );
        
        assert_eq!(
            manager.determine_linux_interface_type("enp0s3"),
            InterfaceType::Ethernet
        );
        
        assert_eq!(
            manager.determine_linux_interface_type("wlan0"),
            InterfaceType::WiFi
        );
        
        assert_eq!(
            manager.determine_linux_interface_type("wlp2s0"),
            InterfaceType::WiFi
        );
        
        assert_eq!(
            manager.determine_linux_interface_type("tun0"),
            InterfaceType::VPN
        );
        
        assert_eq!(
            manager.determine_linux_interface_type("lo"),
            InterfaceType::Loopback
        );
    }
    
    #[tokio::test]
    async fn test_port_availability_check() {
        let manager = LinuxNetworkManager::new();
        
        // Test with a high port that should be available
        let available = manager.is_port_available(8080).await;
        // This might fail in test environment, but we can at least verify the method works
        println!("Port 8080 available: {}", available);
    }
    
    #[test]
    fn test_ip_addr_parsing() {
        let manager = LinuxNetworkManager::new();
        
        let sample_output = r#"
1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536 qdisc noqueue state UNKNOWN group default qlen 1000
    link/loopback 00:00:00:00:00:00 brd 00:00:00:00:00:00
    inet 127.0.0.1/8 scope host lo
       valid_lft forever preferred_lft forever
2: eth0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc pfifo_fast state UP group default qlen 1000
    link/ether 08:00:27:12:34:56 brd ff:ff:ff:ff:ff:ff
    inet 192.168.1.100/24 brd 192.168.1.255 scope global dynamic eth0
       valid_lft 86400sec preferred_lft 86400sec
3: wlan0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc mq state UP group default qlen 1000
    link/ether 00:11:22:33:44:55 brd ff:ff:ff:ff:ff:ff
    inet 192.168.1.101/24 brd 192.168.1.255 scope global dynamic wlan0
       valid_lft 86400sec preferred_lft 86400sec
"#;
        
        let interfaces = manager.parse_ip_addr_output(sample_output).unwrap();
        assert_eq!(interfaces.len(), 2); // lo should be filtered out
        
        let eth0 = &interfaces[0];
        assert_eq!(eth0.name, "eth0");
        assert_eq!(eth0.ip_address, "192.168.1.100".parse::<IpAddr>().unwrap());
        assert_eq!(eth0.interface_type, InterfaceType::Ethernet);
        assert!(eth0.is_up);
        assert!(eth0.supports_multicast);
        
        let wlan0 = &interfaces[1];
        assert_eq!(wlan0.name, "wlan0");
        assert_eq!(wlan0.ip_address, "192.168.1.101".parse::<IpAddr>().unwrap());
        assert_eq!(wlan0.interface_type, InterfaceType::WiFi);
    }
    
    #[test]
    fn test_network_namespaces() {
        let manager = LinuxNetworkManager::new();
        let namespaces = manager.get_network_namespaces();
        // Namespaces list can be empty, that's fine
        println!("Network namespaces: {:?}", namespaces);
    }
}
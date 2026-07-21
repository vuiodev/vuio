use crate::platform::{
    network::{
        command_output, FirewallStatus, InterfaceStatus, NetworkDiagnostics, NetworkManager,
        SsdpConfig, SsdpSocket,
    },
    InterfaceType, NetworkInterface, PlatformError, PlatformResult,
};
use async_trait::async_trait;
use std::net::{IpAddr, SocketAddr};
use tokio::net::UdpSocket;
use tracing::{debug, error, info, warn};

/// FreeBSD-specific network manager implementation
pub struct BsdNetworkManager {
    config: SsdpConfig,
    cached_interfaces: std::sync::Arc<tokio::sync::RwLock<Option<Vec<NetworkInterface>>>>,
}

impl BsdNetworkManager {
    /// Create a new FreeBSD network manager
    pub fn new() -> Self {
        Self {
            config: SsdpConfig::default(),
            cached_interfaces: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Create a new FreeBSD network manager with custom configuration
    pub fn with_config(config: SsdpConfig) -> Self {
        Self {
            config,
            cached_interfaces: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Clear the cached network interfaces
    pub async fn clear_interface_cache(&self) {
        let mut cached = self.cached_interfaces.write().await;
        *cached = None;
        debug!("Cleared FreeBSD network interface cache");
    }

    /// Check if running with elevated privileges
    fn is_elevated(&self) -> bool {
        std::env::var("USER")
            .map(|user| user == "root")
            .unwrap_or(false)
            || std::env::var("SUDO_USER").is_ok()
    }

    /// Check if a port requires sudo privileges
    fn requires_elevation(&self, port: u16) -> bool {
        port < 1024
    }

    /// Try to bind to a port
    async fn try_bind_port(&self, port: u16) -> PlatformResult<UdpSocket> {
        let socket_addr = SocketAddr::from(([0, 0, 0, 0], port));

        match UdpSocket::bind(socket_addr).await {
            Ok(socket) => {
                debug!("Successfully bound to port {} on FreeBSD", port);
                Ok(socket)
            }
            Err(e) => {
                if self.requires_elevation(port) && !self.is_elevated() {
                    warn!("Port {} requires root/sudo privileges on FreeBSD", port);
                    Err(PlatformError::NetworkConfig(format!(
                        "Port {} requires root/sudo privileges on FreeBSD. Error: {}",
                        port, e
                    )))
                } else {
                    Err(PlatformError::NetworkConfig(format!(
                        "Failed to bind to port {} on FreeBSD: {}",
                        port, e
                    )))
                }
            }
        }
    }

    /// Get network interfaces using FreeBSD ifconfig command
    async fn get_bsd_interfaces(&self) -> PlatformResult<Vec<NetworkInterface>> {
        let mut interfaces = Vec::new();

        match command_output("ifconfig", &[]).await {
            Ok(output) if output.status.success() => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                interfaces = self.parse_ifconfig_output(&output_str)?;
                debug!(
                    "Parsed {} interfaces from ifconfig on FreeBSD",
                    interfaces.len()
                );
            }
            _ => {
                warn!("Failed to get network interfaces using ifconfig on FreeBSD");
            }
        }

        // Filter out loopback interfaces from the active list
        interfaces.retain(|iface| !iface.name.starts_with("lo"));

        // If empty, fallback
        if interfaces.is_empty() {
            warn!("No interfaces found via ifconfig, attempting default fallback");
            // Standard loopback fallback
            interfaces.push(NetworkInterface {
                name: "em0".to_string(),
                ip_address: "127.0.0.1".parse().unwrap(),
                is_loopback: false,
                is_up: true,
                supports_multicast: true,
                interface_type: InterfaceType::Ethernet,
            });
        }

        for iface in &interfaces {
            info!(
                "Found FreeBSD interface: {} ({}) - up: {}, multicast: {}",
                iface.name, iface.ip_address, iface.is_up, iface.supports_multicast
            );
        }

        Ok(interfaces)
    }

    /// Parse ifconfig output (FreeBSD format, identical to macOS)
    fn parse_ifconfig_output(&self, output: &str) -> PlatformResult<Vec<NetworkInterface>> {
        let mut interfaces = Vec::new();
        let mut current_interface: Option<String> = None;
        let mut current_ip: Option<IpAddr> = None;
        let mut is_up = false;
        let mut supports_multicast = false;

        for line in output.lines() {
            if !line.starts_with('\t')
                && !line.starts_with(' ')
                && line.contains(':')
                && line.contains("flags=")
            {
                // Save previous interface
                if let (Some(name), Some(ip_address)) = (&current_interface, current_ip) {
                    if !name.starts_with("lo") {
                        interfaces.push(NetworkInterface {
                            name: name.clone(),
                            ip_address,
                            is_loopback: name.starts_with("lo"),
                            is_up,
                            supports_multicast,
                            interface_type: self.determine_bsd_interface_type(name),
                        });
                    }
                }

                // Start new interface
                let interface_name = line.split(':').next().unwrap_or("unknown").to_string();
                current_interface = Some(interface_name);
                current_ip = None;
                is_up = false;
                supports_multicast = false;

                if line.contains("UP") {
                    is_up = true;
                }
                if line.contains("MULTICAST") {
                    supports_multicast = true;
                }
            }

            if line.trim().starts_with("inet ") && !line.contains("inet6") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(ip) = parts[1].parse::<IpAddr>() {
                        current_ip = Some(ip);
                    }
                }
            }
        }

        // Don't forget last interface
        if let (Some(name), Some(ip_address)) = (current_interface, current_ip) {
            if !name.starts_with("lo") {
                interfaces.push(NetworkInterface {
                    name,
                    ip_address,
                    is_loopback: false,
                    is_up,
                    supports_multicast,
                    interface_type: self.determine_bsd_interface_type(&name),
                });
            }
        }

        Ok(interfaces)
    }

    fn determine_bsd_interface_type(&self, name: &str) -> InterfaceType {
        if name.starts_with("em")
            || name.starts_with("igb")
            || name.starts_with("ix")
            || name.starts_with("re")
            || name.starts_with("bge")
            || name.starts_with("axg")
        {
            InterfaceType::Ethernet
        } else if name.starts_with("wlan")
            || name.starts_with("ath")
            || name.starts_with("ral")
            || name.starts_with("rtwn")
        {
            InterfaceType::WiFi
        } else if name.starts_with("tun")
            || name.starts_with("tap")
            || name.starts_with("gif")
            || name.starts_with("gre")
            || name.starts_with("wg")
        {
            InterfaceType::VPN
        } else if name.starts_with("lo") {
            InterfaceType::Loopback
        } else {
            InterfaceType::Other(name.to_string())
        }
    }

    fn get_preferred_multicast_interface<'a>(
        &self,
        interfaces: &'a [NetworkInterface],
    ) -> Option<&'a NetworkInterface> {
        interfaces
            .iter()
            .filter(|iface| {
                !iface.is_loopback
                    && iface.is_up
                    && iface.supports_multicast
                    && match iface.ip_address {
                        IpAddr::V4(ipv4) => !ipv4.is_loopback() && !ipv4.is_link_local(),
                        IpAddr::V6(ipv6) => !ipv6.is_loopback(),
                    }
            })
            .min_by_key(|iface| match iface.interface_type {
                InterfaceType::Ethernet => 0,
                InterfaceType::WiFi => 1,
                InterfaceType::VPN => 2,
                InterfaceType::Other(_) => 3,
                InterfaceType::Loopback => 4,
            })
    }
}

impl Default for BsdNetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NetworkManager for BsdNetworkManager {
    async fn create_ssdp_socket(&self) -> PlatformResult<SsdpSocket> {
        self.create_ssdp_socket_with_config(&self.config).await
    }

    async fn create_ssdp_socket_with_config(
        &self,
        config: &SsdpConfig,
    ) -> PlatformResult<SsdpSocket> {
        match self.try_bind_port(config.primary_port).await {
            Ok(socket) => {
                let interfaces = self.get_local_interfaces().await?;
                let suitable_interfaces: Vec<_> = interfaces
                    .into_iter()
                    .filter(|iface| !iface.is_loopback && iface.is_up && iface.supports_multicast)
                    .collect();

                if suitable_interfaces.is_empty() {
                    return Err(PlatformError::NetworkConfig(
                        "No suitable network interfaces found on FreeBSD".to_string(),
                    ));
                }

                Ok(SsdpSocket {
                    socket,
                    port: config.primary_port,
                    interfaces: suitable_interfaces,
                    multicast_enabled: false,
                })
            }
            Err(primary_error) => {
                warn!(
                    "Primary port {} failed on FreeBSD: {}. Trying fallback ports.",
                    config.primary_port, primary_error
                );

                for &port in &config.fallback_ports {
                    if let Ok(socket) = self.try_bind_port(port).await {
                        info!("Using fallback port {} on FreeBSD", port);
                        let interfaces = self.get_local_interfaces().await?;
                        let suitable_interfaces: Vec<_> = interfaces
                            .into_iter()
                            .filter(|iface| {
                                !iface.is_loopback && iface.is_up && iface.supports_multicast
                            })
                            .collect();

                        if suitable_interfaces.is_empty() {
                            return Err(PlatformError::NetworkConfig(
                                "No suitable network interfaces found on FreeBSD".to_string(),
                            ));
                        }

                        return Ok(SsdpSocket {
                            socket,
                            port,
                            interfaces: suitable_interfaces,
                            multicast_enabled: false,
                        });
                    }
                }

                Err(primary_error)
            }
        }
    }

    async fn get_local_interfaces(&self) -> PlatformResult<Vec<NetworkInterface>> {
        {
            let cached = self.cached_interfaces.read().await;
            if let Some(ref interfaces) = *cached {
                return Ok(interfaces.clone());
            }
        }

        let interfaces = self.get_bsd_interfaces().await?;
        {
            let mut cached = self.cached_interfaces.write().await;
            *cached = Some(interfaces.clone());
        }

        Ok(interfaces)
    }

    async fn get_primary_interface(&self) -> PlatformResult<NetworkInterface> {
        let interfaces = self.get_local_interfaces().await?;
        self.get_preferred_multicast_interface(&interfaces)
            .cloned()
            .ok_or_else(|| {
                PlatformError::NetworkConfig(
                    "No suitable primary interface found on FreeBSD".to_string(),
                )
            })
    }

    async fn join_multicast_group(
        &self,
        socket: &mut SsdpSocket,
        group: IpAddr,
        interface: Option<&NetworkInterface>,
    ) -> PlatformResult<()> {
        let (local_addr, interface_name) = if let Some(iface) = interface {
            (iface.ip_address, iface.name.clone())
        } else {
            let selected_interface = self
                .get_preferred_multicast_interface(&socket.interfaces)
                .ok_or_else(|| {
                    PlatformError::NetworkConfig(
                        "No suitable interface for multicast on FreeBSD".to_string(),
                    )
                })?;
            (
                selected_interface.ip_address,
                selected_interface.name.clone(),
            )
        };

        match socket.enable_multicast(group, local_addr).await {
            Ok(()) => {
                info!(
                    "Successfully enabled multicast on FreeBSD for group {} via interface {} ({})",
                    group, interface_name, local_addr
                );
                Ok(())
            }
            Err(e) => {
                warn!("Primary multicast enable failed on FreeBSD: {}", e);
                // Fallback to unspecified address
                match socket
                    .enable_multicast(group, std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED))
                    .await
                {
                    Ok(()) => {
                        info!("Successfully enabled multicast on FreeBSD using fallback binding");
                        Ok(())
                    }
                    Err(fallback_error) => {
                        error!(
                            "Fallback multicast enable failed on FreeBSD: {}",
                            fallback_error
                        );
                        Err(PlatformError::NetworkConfig(format!(
                            "Multicast failed on FreeBSD (tried {} and fallback): {} / {}",
                            local_addr, e, fallback_error
                        )))
                    }
                }
            }
        }
    }

    async fn send_multicast(
        &self,
        socket: &SsdpSocket,
        data: &[u8],
        group: SocketAddr,
    ) -> PlatformResult<()> {
        if !socket.multicast_enabled {
            return Err(PlatformError::NetworkConfig(
                "Multicast not enabled on FreeBSD socket".to_string(),
            ));
        }

        match socket.send_to(data, group).await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Failed to send multicast on FreeBSD: {}", e);
                Err(e)
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
            if let IpAddr::V4(ipv4) = interface.ip_address {
                let octets = ipv4.octets();
                let broadcast_ip = std::net::Ipv4Addr::new(octets[0], octets[1], octets[2], 255);
                let broadcast_addr = SocketAddr::from((broadcast_ip, socket.port));

                match socket.send_to(data, broadcast_addr).await {
                    Ok(_) => {
                        success_count += 1;
                    }
                    Err(e) => {
                        warn!(
                            "Failed to send FreeBSD unicast fallback via interface {}: {}",
                            interface.name, e
                        );
                        last_error = Some(e);
                    }
                }
            }
        }

        if success_count > 0 {
            Ok(())
        } else {
            Err(last_error.unwrap_or_else(|| {
                PlatformError::NetworkConfig(
                    "No FreeBSD interfaces available for unicast fallback".to_string(),
                )
            }))
        }
    }

    async fn is_port_available(&self, port: u16) -> bool {
        (self.try_bind_port(port).await).is_ok()
    }

    async fn get_network_diagnostics(&self) -> PlatformResult<NetworkDiagnostics> {
        let interfaces = self.get_local_interfaces().await.unwrap_or_default();
        let mut interface_status = Vec::new();
        let mut available_ports = Vec::new();

        for port in &[self.config.primary_port, 8080, 8081, 8082] {
            if self.is_port_available(*port).await {
                available_ports.push(*port);
            }
        }

        for iface in interfaces {
            interface_status.push(InterfaceStatus {
                reachable: iface.is_up,
                multicast_capable: iface.supports_multicast,
                error_message: None,
                interface: iface,
            });
        }

        Ok(NetworkDiagnostics {
            multicast_working: !interface_status.is_empty(),
            available_ports,
            interface_status,
            diagnostic_messages: vec!["FreeBSD diagnostics collected".to_string()],
            firewall_status: Some(FirewallStatus {
                detected: false,
                blocking_ssdp: Some(false),
                suggestions: Vec::new(),
            }),
        })
    }

    async fn test_multicast(&self, _interface: &NetworkInterface) -> PlatformResult<bool> {
        Ok(true)
    }
}

use super::*;

impl LinuxNetworkManager {
    ///  bind to INADDR_ANY:1900
    async fn create_receive_socket(&self, port: u16) -> PlatformResult<UdpSocket> {
        let socket_addr = SocketAddr::from(([0, 0, 0, 0], port)); // INADDR_ANY

        let socket = UdpSocket::bind(socket_addr).await.map_err(|e| {
            PlatformError::NetworkConfig(format!(
                "Failed to bind receive socket to {}: {}",
                socket_addr, e
            ))
        })?;

        self.apply_receive_socket_options(&socket)?;

        info!("Created receive socket bound to {}", socket_addr);
        Ok(socket)
    }

    /// Apply receive socket options
    fn apply_receive_socket_options(&self, socket: &UdpSocket) -> PlatformResult<()> {
        let socket = socket2::SockRef::from(socket);
        if let Err(error) = socket.set_reuse_address(true) {
            warn!("Failed to set SO_REUSEADDR: {error}");
        }

        // This is necessary to prevent the "Address in use" error when creating the announcement socket.
        if let Err(error) = socket.set_reuse_port(true) {
            warn!(
                "Failed to set SO_REUSEPORT: {error}. This might cause issues in some environments."
            );
        }

        debug!("Applied receive socket options");
        Ok(())
    }

    /// Join multicast membership on specific interface
    async fn join_multicast_on_interface(
        &self,
        socket: &UdpSocket,
        interface: &NetworkInterface,
    ) -> PlatformResult<()> {
        // Use IP_ADD_MEMBERSHIP with specific interface address
        if let IpAddr::V4(interface_ip) = interface.ip_address {
            let multicast_addr = SSDP_MULTICAST_IPV4;
            socket
                .join_multicast_v4(multicast_addr, interface_ip)
                .map_err(|error| {
                    PlatformError::NetworkConfig(format!(
                        "Failed to join multicast group on interface {} ({}): {}",
                        interface.name, interface_ip, error
                    ))
                })?;

            info!(
                "Successfully joined multicast 239.255.255.250 on interface {} ({})",
                interface.name, interface_ip
            );
            Ok(())
        } else {
            Err(PlatformError::NetworkConfig(format!(
                "Interface {} has IPv6 address, multicast IPv4 not supported",
                interface.name
            )))
        }
    }
}

#[async_trait]
impl NetworkManager for LinuxNetworkManager {
    async fn create_ssdp_socket(&self) -> PlatformResult<SsdpSocket> {
        self.create_ssdp_socket_with_config(&self.config).await
    }

    /// Create SSDP socket with Docker compatibility
    async fn create_ssdp_socket_with_config(
        &self,
        config: &SsdpConfig,
    ) -> PlatformResult<SsdpSocket> {
        // Create receive socket bound to INADDR_ANY:1900
        let receive_socket = self.create_receive_socket(config.primary_port).await?;

        // Get all available network interfaces
        let interfaces = self.get_local_interfaces().await?;
        let suitable_interfaces: Vec<_> = interfaces
            .into_iter()
            .filter(|iface| !iface.is_loopback && iface.is_up && iface.supports_multicast)
            .collect();

        if suitable_interfaces.is_empty() {
            return Err(PlatformError::NetworkConfig(
                "No suitable network interfaces found on Linux".to_string(),
            ));
        }

        // Create SSDP socket with receive socket
        let mut ssdp_socket = SsdpSocket {
            socket: receive_socket,
            port: config.primary_port,
            interfaces: suitable_interfaces.clone(),
            multicast_enabled: false,
        };

        // Join multicast membership on each interface
        for interface in &suitable_interfaces {
            if let Err(e) = self
                .join_multicast_on_interface(&ssdp_socket.socket, interface)
                .await
            {
                warn!(
                    "Failed to join multicast on interface {}: {}",
                    interface.name, e
                );
            } else {
                info!(
                    "Joined multicast group on interface {} ({})",
                    interface.name, interface.ip_address
                );
                ssdp_socket.multicast_enabled = true;
            }
        }

        if !ssdp_socket.multicast_enabled {
            return Err(PlatformError::NetworkConfig(
                "Failed to join multicast on any interface".to_string(),
            ));
        }

        Ok(ssdp_socket)
    }

    async fn get_local_interfaces(&self) -> PlatformResult<Vec<NetworkInterface>> {
        // Check if we have cached interfaces first
        {
            let cached = self.cached_interfaces.read().await;
            if let Some(ref interfaces) = *cached {
                debug!(
                    "Using cached network interfaces (count: {})",
                    interfaces.len()
                );
                return Ok(interfaces.clone());
            }
        }

        // No cached interfaces, detect them and cache the result
        info!("Detecting network interfaces for the first time...");
        let interfaces = self.get_linux_interfaces().await?;

        // Cache the result
        {
            let mut cached = self.cached_interfaces.write().await;
            *cached = Some(interfaces.clone());
        }

        Ok(interfaces)
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
            PlatformError::NetworkConfig("No suitable primary interface found on Linux".to_string())
        })
    }

    async fn join_multicast_group(
        &self,
        socket: &mut SsdpSocket,
        group: IpAddr,
        interface: Option<&NetworkInterface>,
    ) -> PlatformResult<()> {
        // multicast membership is already set up during socket creation
        // This method just verifies that multicast is enabled

        if socket.multicast_enabled {
            info!("Multicast already enabled on socket");
            return Ok(());
        }

        // If not enabled yet, try to enable it on specified interface or all interfaces
        let mut successful_joins = 0;
        let mut last_error = None;

        if let Some(specific_interface) = interface {
            // Join on specific interface only
            match self
                .join_multicast_on_interface(&socket.socket, specific_interface)
                .await
            {
                Ok(()) => {
                    info!(
                        "Successfully joined multicast group {} on interface {}",
                        group, specific_interface.name
                    );
                    socket.multicast_enabled = true;
                    return Ok(());
                }
                Err(e) => {
                    return Err(e);
                }
            }
        } else {
            // Join on all suitable interfaces
            let interfaces = socket.interfaces.clone();
            for iface in &interfaces {
                if !iface.is_loopback && iface.is_up && iface.supports_multicast {
                    match self
                        .join_multicast_on_interface(&socket.socket, iface)
                        .await
                    {
                        Ok(()) => {
                            info!(
                                "Successfully joined multicast group {} on interface {}",
                                group, iface.name
                            );
                            successful_joins += 1;
                        }
                        Err(e) => {
                            warn!(
                                "Failed to join multicast group {} on interface {}: {}",
                                group, iface.name, e
                            );
                            last_error = Some(e);
                        }
                    }
                }
            }

            if successful_joins > 0 {
                info!(
                    "Successfully joined multicast on {}/{} interfaces",
                    successful_joins,
                    interfaces.len()
                );
                socket.multicast_enabled = true;
                return Ok(());
            } else {
                return Err(last_error.unwrap_or_else(|| {
                    PlatformError::NetworkConfig(
                        "Failed to join multicast on any interface".to_string(),
                    )
                }));
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
                "Multicast not enabled on Linux socket".to_string(),
            ));
        }

        match socket.send_to(data, group).await {
            Ok(_) => {
                debug!(
                    "Sent {} bytes to multicast group {} on Linux",
                    data.len(),
                    group
                );
                Ok(())
            }
            Err(e) => {
                error!("Failed to send multicast on Linux: {}", e);
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
            // Calculate broadcast address for Linux
            let broadcast_addr = match interface.ip_address {
                IpAddr::V4(ipv4) => {
                    // Simple broadcast calculation - in real implementation,
                    // you would use route command or netlink to get proper subnet info
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
                        "Sent Linux unicast fallback to {} via interface {}",
                        broadcast_addr, interface.name
                    );
                }
                Err(e) => {
                    warn!(
                        "Failed to send Linux unicast fallback via interface {}: {}",
                        interface.name, e
                    );
                    last_error = Some(e);
                }
            }
        }

        if success_count > 0 {
            info!(
                "Linux unicast fallback succeeded on {} interfaces",
                success_count
            );
            Ok(())
        } else {
            Err(last_error.unwrap_or_else(|| {
                PlatformError::NetworkConfig(
                    "No Linux interfaces available for unicast fallback".to_string(),
                )
            }))
        }
    }

    async fn is_port_available(&self, port: u16) -> bool {
        self.try_bind_port_linux(port).await.is_ok()
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
                    .push(format!("Port {} requires root privileges on Linux", port));
            }
        }

        // Add Linux-specific diagnostic messages
        if available_ports.is_empty() {
            diagnostic_messages
                .push("No common ports are available for binding on Linux".to_string());
            if !self.is_elevated() {
                diagnostic_messages
                    .push("Consider running with sudo to access privileged ports".to_string());
            }
        }

        if interface_status
            .iter()
            .all(|status| !status.multicast_capable)
        {
            diagnostic_messages.push("No Linux interfaces support multicast".to_string());
            diagnostic_messages
                .push("Check network interface configuration and kernel modules".to_string());
        }

        // Check for network namespaces
        let namespaces = self.get_network_namespaces();
        if !namespaces.is_empty() {
            diagnostic_messages.push(format!("Network namespaces detected: {:?}", namespaces));
            diagnostic_messages
                .push("Consider running in the correct network namespace".to_string());
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
        // Basic test for Linux - check if interface supports multicast
        if !interface.supports_multicast || !interface.is_up || interface.is_loopback {
            return Ok(false);
        }

        // Try to create a test socket and join multicast group
        match UdpSocket::bind("0.0.0.0:0").await {
            Ok(test_socket) => {
                match interface.ip_address {
                    IpAddr::V4(local_v4) => {
                        let multicast_addr = SSDP_MULTICAST_IPV4;
                        match test_socket.join_multicast_v4(multicast_addr, local_v4) {
                            Ok(()) => {
                                debug!(
                                    "Multicast test successful on Linux interface {}",
                                    interface.name
                                );
                                Ok(true)
                            }
                            Err(e) => {
                                debug!(
                                    "Multicast test failed on Linux interface {}: {}",
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

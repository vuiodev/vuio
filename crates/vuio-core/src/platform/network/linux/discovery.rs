use super::*;

impl LinuxNetworkManager {
    /// Get network interfaces using Linux-specific methods
    pub(super) async fn get_linux_interfaces(&self) -> PlatformResult<Vec<NetworkInterface>> {
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
            ip_address: LOOPBACK_IPV4,
            is_loopback: false,
            is_up: true,
            supports_multicast: true,
            interface_type: InterfaceType::Ethernet,
        });

        Ok(interfaces)
    }

    /// Parse output from 'ip addr show' command
    pub(super) async fn parse_ip_command_output(&self) -> PlatformResult<Vec<NetworkInterface>> {
        let output = Command::new("ip")
            .args(["addr", "show"])
            .output()
            .map_err(|e| {
                PlatformError::NetworkConfig(format!("Failed to run 'ip addr show': {}", e))
            })?;

        if !output.status.success() {
            return Err(PlatformError::NetworkConfig(
                "'ip addr show' command failed".to_string(),
            ));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        self.parse_ip_addr_output(&output_str)
    }

    /// Parse the output of 'ip addr show'
    pub(super) fn parse_ip_addr_output(
        &self,
        output: &str,
    ) -> PlatformResult<Vec<NetworkInterface>> {
        let mut interfaces = Vec::new();
        let mut current_interface: Option<String> = None;
        let mut current_ips: Vec<IpAddr> = Vec::new();
        let mut is_up = false;
        let mut supports_multicast = false;
        let mut is_loopback = false;

        for line in output.lines() {
            let line = line.trim();

            // Interface line: "2: eth0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc pfifo_fast state UP group default qlen 1000"
            if let Some(colon_pos) = line.find(':') {
                if let Some(second_colon) = line[colon_pos + 1..].find(':') {
                    let second_colon_pos = colon_pos + 1 + second_colon;

                    // Save previous interface with the best IP
                    if let Some(name) = &current_interface {
                        if !name.starts_with("lo") && !current_ips.is_empty() {
                            // Choose the best IP address (prefer non-localhost)
                            let best_ip = current_ips
                                .iter()
                                .find(|ip| !ip.is_loopback())
                                .or_else(|| current_ips.first())
                                .copied()
                                .unwrap_or(LOOPBACK_IPV4);

                            let interface_type = self.determine_linux_interface_type(name);
                            interfaces.push(NetworkInterface {
                                name: name.clone(),
                                ip_address: best_ip,
                                is_loopback: is_loopback && best_ip.is_loopback(),
                                is_up,
                                supports_multicast,
                                interface_type,
                            });
                        }
                    }

                    // Parse new interface
                    let interface_name = line[colon_pos + 1..second_colon_pos].trim().to_string();
                    current_interface = Some(interface_name.clone());
                    current_ips.clear();
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
                            current_ips.push(ip);
                        }
                    }
                }
            }
        }

        // Don't forget the last interface
        if let Some(name) = current_interface {
            if !name.starts_with("lo") && !current_ips.is_empty() {
                // Choose the best IP address (prefer non-localhost)
                let best_ip = current_ips
                    .iter()
                    .find(|ip| !ip.is_loopback())
                    .or_else(|| current_ips.first())
                    .copied()
                    .unwrap_or(LOOPBACK_IPV4);

                let interface_type = self.determine_linux_interface_type(&name);
                interfaces.push(NetworkInterface {
                    name,
                    ip_address: best_ip,
                    is_loopback: is_loopback && best_ip.is_loopback(),
                    is_up,
                    supports_multicast,
                    interface_type,
                });
            }
        }

        // If we still don't have any good interfaces, try a different approach
        if interfaces.is_empty() || interfaces.iter().all(|i| i.ip_address.is_loopback()) {
            debug!("Standard interface detection failed, trying alternative methods");
            if let Ok(alt_interfaces) = self.get_interfaces_alternative_method() {
                if !alt_interfaces.is_empty() {
                    return Ok(alt_interfaces);
                }
            }
        }

        // Special handling for Docker containers - prioritize configured server IP
        if self.is_running_in_docker() && interfaces.iter().all(|i| i.ip_address.is_loopback()) {
            if let Ok(server_ip_str) = std::env::var("VUIO_IP") {
                if let Ok(server_ip) = server_ip_str.parse::<IpAddr>() {
                    // Replace all loopback IPs with the configured server IP for the primary interface
                    if let Some(primary_interface) = interfaces
                        .iter_mut()
                        .find(|i| i.is_up && i.supports_multicast)
                    {
                        info!("Docker container detected: Overriding interface {} IP from {} to configured server IP {}",
                              primary_interface.name, primary_interface.ip_address, server_ip);
                        primary_interface.ip_address = server_ip;
                        primary_interface.is_loopback = false;
                    }
                }
            }
        }

        Ok(interfaces)
    }

    /// Parse /proc/net/dev as fallback
    pub(super) async fn parse_proc_net_dev(&self) -> PlatformResult<Vec<NetworkInterface>> {
        let contents = std::fs::read_to_string("/proc/net/dev").map_err(|e| {
            PlatformError::NetworkConfig(format!("Failed to read /proc/net/dev: {}", e))
        })?;

        let mut interfaces = Vec::new();

        for line in contents.lines().skip(2) {
            // Skip header lines
            if let Some(interface_name) = line.split(':').next() {
                let interface_name = interface_name.trim().to_string();

                // Skip loopback
                if interface_name.starts_with("lo") {
                    continue;
                }

                // Check if interface is up by reading from /sys/class/net
                let is_up =
                    std::fs::read_to_string(format!("/sys/class/net/{}/operstate", interface_name))
                        .map(|state| state.trim() == "up")
                        .unwrap_or(false);

                // Get IP address using ip command for this specific interface
                let ip_address = self
                    .get_interface_ip(&interface_name)
                    .unwrap_or(LOOPBACK_IPV4);

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

    /// Check if we're running in a Docker container
    pub(super) fn is_running_in_docker(&self) -> bool {
        // Check for Docker-specific files
        std::path::Path::new("/.dockerenv").exists()
            || std::fs::read_to_string("/proc/1/cgroup")
                .map(|content| content.contains("docker") || content.contains("containerd"))
                .unwrap_or(false)
    }

    /// Alternative method to get network interfaces when standard detection fails
    pub(super) fn get_interfaces_alternative_method(
        &self,
    ) -> PlatformResult<Vec<NetworkInterface>> {
        let mut interfaces = Vec::new();

        // Priority 1: If we have VUIO_SERVER_IP configured and we're in Docker, use it directly
        if self.is_running_in_docker() {
            if let Ok(server_ip_str) = std::env::var("VUIO_IP") {
                if let Ok(server_ip) = server_ip_str.parse::<IpAddr>() {
                    // Find the interface that should be used for this IP
                    let interface_name =
                        std::env::var("VUIO_SSDP_INTERFACE").unwrap_or_else(|_| {
                            // Try to determine from routing table
                            if let Ok(output) = Command::new("ip")
                                .args(["route", "get", &server_ip_str])
                                .output()
                            {
                                let output_str = String::from_utf8_lossy(&output.stdout);
                                output_str
                                    .lines()
                                    .find_map(|line| {
                                        if line.contains("dev") {
                                            let parts: Vec<&str> =
                                                line.split_whitespace().collect();
                                            if let Some(dev_idx) =
                                                parts.iter().position(|&x| x == "dev")
                                            {
                                                parts.get(dev_idx + 1).map(|s| s.to_string())
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap_or_else(|| "enp12s0".to_string())
                            } else {
                                "enp12s0".to_string()
                            }
                        });

                    let interface_type = self.determine_linux_interface_type(&interface_name);
                    interfaces.push(NetworkInterface {
                        name: interface_name.clone(),
                        ip_address: server_ip,
                        is_loopback: false,
                        is_up: true,
                        supports_multicast: true,
                        interface_type,
                    });

                    info!(
                        "Docker detected: Using configured server IP {} for interface {}",
                        server_ip, interface_name
                    );
                    return Ok(interfaces);
                }
            }
        }

        // Priority 2: Try to get the default route interface and its IP
        if let Ok(output) = Command::new("ip")
            .args(["route", "show", "default"])
            .output()
        {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                for line in output_str.lines() {
                    if line.contains("default") && line.contains("dev") {
                        // Parse: "default via 192.168.1.1 dev enp12s0 proto dhcp metric 100"
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if let Some(dev_idx) = parts.iter().position(|&x| x == "dev") {
                            if let Some(interface_name) = parts.get(dev_idx + 1) {
                                if let Some(ip) = self.get_interface_ip_robust(interface_name) {
                                    let interface_type =
                                        self.determine_linux_interface_type(interface_name);
                                    interfaces.push(NetworkInterface {
                                        name: interface_name.to_string(),
                                        ip_address: ip,
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
        }

        // Priority 3: If still no interfaces, try to use the configured server IP with best guess interface
        if interfaces.is_empty() {
            if let Ok(server_ip_str) = std::env::var("VUIO_IP") {
                if let Ok(server_ip) = server_ip_str.parse::<IpAddr>() {
                    // Find the most likely interface name
                    let interface_name = if let Ok(output) = Command::new("ip")
                        .args(["route", "get", &server_ip_str])
                        .output()
                    {
                        let output_str = String::from_utf8_lossy(&output.stdout);
                        output_str
                            .lines()
                            .find_map(|line| {
                                if line.contains("dev") {
                                    let parts: Vec<&str> = line.split_whitespace().collect();
                                    if let Some(dev_idx) = parts.iter().position(|&x| x == "dev") {
                                        parts.get(dev_idx + 1).map(|s| s.to_string())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| "enp12s0".to_string()) // Fallback based on your logs
                    } else {
                        "enp12s0".to_string()
                    };

                    let interface_type = self.determine_linux_interface_type(&interface_name);
                    interfaces.push(NetworkInterface {
                        name: interface_name,
                        ip_address: server_ip,
                        is_loopback: false,
                        is_up: true,
                        supports_multicast: true,
                        interface_type,
                    });
                }
            }
        }

        Ok(interfaces)
    }

    /// Get IP address for a specific interface with more robust methods
    pub(super) fn get_interface_ip_robust(&self, interface_name: &str) -> Option<IpAddr> {
        // First check if we're in Docker and have a configured server IP
        if self.is_running_in_docker() {
            if let Ok(server_ip_str) = std::env::var("VUIO_IP") {
                if let Ok(server_ip) = server_ip_str.parse::<IpAddr>() {
                    // Check if this interface should use the configured server IP
                    if let Ok(ssdp_interface) = std::env::var("VUIO_SSDP_INTERFACE") {
                        if interface_name == ssdp_interface {
                            debug!(
                                "Using configured server IP {} for Docker interface {}",
                                server_ip, interface_name
                            );
                            return Some(server_ip);
                        }
                    } else {
                        // If no specific SSDP interface is configured, use server IP for primary interfaces
                        if interface_name.starts_with("enp") || interface_name.starts_with("eth") {
                            debug!(
                                "Using configured server IP {} for Docker interface {}",
                                server_ip, interface_name
                            );
                            return Some(server_ip);
                        }
                    }
                }
            }
        }

        // First try the standard method
        if let Some(ip) = self.get_interface_ip(interface_name) {
            if !ip.is_loopback() {
                return Some(ip);
            }
        }

        // Try using ip route to find the source IP for this interface
        if let Ok(output) = Command::new("ip")
            .args(["route", "show", "dev", interface_name])
            .output()
        {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                for line in output_str.lines() {
                    if line.contains("src") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if let Some(src_idx) = parts.iter().position(|&x| x == "src") {
                            if let Some(ip_str) = parts.get(src_idx + 1) {
                                if let Ok(ip) = ip_str.parse::<IpAddr>() {
                                    if !ip.is_loopback() {
                                        return Some(ip);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Get IP address for a specific interface
    pub(super) fn get_interface_ip(&self, interface_name: &str) -> Option<IpAddr> {
        match Command::new("ip")
            .args(["addr", "show", interface_name])
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
    pub(super) fn determine_linux_interface_type(&self, name: &str) -> InterfaceType {
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
    pub(super) fn get_network_namespaces(&self) -> Vec<String> {
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
}

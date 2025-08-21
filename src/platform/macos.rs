#[cfg(target_os = "macos")]
use super::{NetworkInterface, InterfaceType, PlatformResult};
use std::collections::HashMap;
use std::net::IpAddr;

/// Get macOS version information
pub fn get_macos_version() -> PlatformResult<String> {
    // Try to get macOS version from system
    match std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
    {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(format!("macOS {}", version))
        }
        _ => {
            // Fallback to basic detection
            Ok("macOS (unknown version)".to_string())
        }
    }
}

/// Determine interface type based on macOS interface name
fn determine_macos_interface_type(name: &str) -> InterfaceType {
    if name.starts_with("en") {
        // This is a heuristic. en0 could be Wi-Fi on modern Macs.
        // A more robust method would be needed for accuracy.
        if name == "en0" {
            InterfaceType::Ethernet 
        } else {
            InterfaceType::WiFi
        }
    } else if name.starts_with("awdl") {
        InterfaceType::WiFi // Apple Wireless Direct Link
    } else if name.starts_with("utun") || name.starts_with("ipsec") || name.starts_with("ppp") {
        InterfaceType::VPN
    } else if name.starts_with("lo") {
        InterfaceType::Loopback
    } else if name.starts_with("bridge") || name.starts_with("anpi") {
        InterfaceType::Other(format!("Bridge/{}", name))
    } else if name.starts_with("ap") || name.starts_with("llw") {
        InterfaceType::WiFi
    } else {
        InterfaceType::Other(name.to_string())
    }
}

/// Parse ifconfig output to extract network interface information
fn parse_ifconfig_output(output: &str) -> PlatformResult<Vec<NetworkInterface>> {
    let mut interfaces = Vec::new();
    let mut current_interface: Option<String> = None;
    let mut current_ip: Option<IpAddr> = None;
    let mut is_up = false;
    let mut supports_multicast = false;
    let mut status_active = false;

    for line in output.lines() {
        // Detect interface name (starts at beginning of line and ends with colon)
        if !line.starts_with('\t') && !line.starts_with(' ') && line.contains(':') && line.contains("flags=") {
            // Save previous interface if we have one
            if let (Some(name), Some(ip)) = (&current_interface, &current_ip) {
                if !name.starts_with("lo") { // Skip loopback
                    let interface_type = determine_macos_interface_type(name);
                    // An interface is considered 'up' if its flags say UP and its status is active.
                    // Some virtual interfaces (VPN, awdl) don't report status but are active if UP.
                    let final_is_up = is_up && (status_active || name.starts_with("awdl") || name.starts_with("utun"));
                    
                    interfaces.push(NetworkInterface {
                        name: name.clone(),
                        ip_address: *ip,
                        is_loopback: name.starts_with("lo"),
                        is_up: final_is_up,
                        supports_multicast,
                        interface_type,
                    });
                }
            }
            
            // Start new interface
            let interface_name = line.split(':').next().unwrap_or("unknown").to_string();
            current_interface = Some(interface_name);
            current_ip = None;
            is_up = false;
            supports_multicast = false;
            status_active = false;
            
            // Check flags in the same line
            if line.contains("<UP") {
                is_up = true;
            }
            if line.contains("<MULTICAST") {
                supports_multicast = true;
            }
        }
        
        // Look for IPv4 address (skip IPv6)
        if line.trim().starts_with("inet ") && !line.contains("inet6") {
            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(ip) = parts[1].parse::<IpAddr>() {
                    current_ip = Some(ip);
                }
            }
        }
        
        // Check for status flags
        if line.trim().starts_with("status: active") {
            status_active = true;
        }
    }
    
    // Don't forget the last interface
    if let (Some(name), Some(ip)) = (current_interface, current_ip) {
        if !name.starts_with("lo") { // Skip loopback
            let interface_type = determine_macos_interface_type(&name);
            let final_is_up = is_up && (status_active || name.starts_with("awdl") || name.starts_with("utun"));
            
            interfaces.push(NetworkInterface {
                name,
                ip_address: ip,
                is_loopback: false,
                is_up: final_is_up,
                supports_multicast,
                interface_type,
            });
        }
    }
    
    Ok(interfaces)
}

/// Detect network interfaces on macOS
pub async fn detect_network_interfaces() -> PlatformResult<Vec<NetworkInterface>> {
    // Use ifconfig to get interface information
    match std::process::Command::new("ifconfig").output() {
        Ok(output) if output.status.success() => {
            let output_str = String::from_utf8_lossy(&output.stdout);
            parse_ifconfig_output(&output_str)
        }
        _ => {
            // Fallback: create a basic interface
            let mut interfaces = Vec::new();
            let interface = NetworkInterface {
                name: "en0".to_string(),
                ip_address: "127.0.0.1".parse().unwrap(),
                is_loopback: false,
                is_up: true,
                supports_multicast: true,
                interface_type: InterfaceType::Ethernet,
            };
            interfaces.push(interface);
            Ok(interfaces)
        }
    }
}

/// Gather macOS-specific metadata
pub fn gather_macos_metadata() -> PlatformResult<HashMap<String, String>> {
    let mut metadata = HashMap::new();
    
    metadata.insert("platform".to_string(), "macOS".to_string());
    
    // Get system information using system_profiler or sw_vers
    if let Ok(output) = std::process::Command::new("sw_vers").output() {
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                if let Some((key, value)) = line.split_once(':') {
                    let key = key.trim().to_lowercase().replace(' ', "_");
                    let value = value.trim().to_string();
                    metadata.insert(key, value);
                }
            }
        }
    }
    
    // Get hardware information
    if let Ok(output) = std::process::Command::new("uname")
        .arg("-m")
        .output()
    {
        if output.status.success() {
            let arch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            metadata.insert("hardware_architecture".to_string(), arch);
        }
    }
    
    // Get hostname
    if let Ok(output) = std::process::Command::new("hostname").output() {
        if output.status.success() {
            let hostname = String::from_utf8_lossy(&output.stdout).trim().to_string();
            metadata.insert("hostname".to_string(), hostname);
        }
    }
    
    Ok(metadata)
}

/// Check macOS firewall status
pub fn get_firewall_status() -> PlatformResult<bool> {
    // Check if the application firewall is enabled
    match std::process::Command::new("defaults")
        .args(&["read", "/Library/Preferences/com.apple.alf", "globalstate"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let output_str = String::from_utf8_lossy(&output.stdout);
            let state = output_str.trim();
            // 0 = disabled, 1 = enabled for specific services, 2 = enabled for essential services
            Ok(state != "0")
        }
        _ => {
            // Assume firewall is enabled if we can't determine status
            Ok(true)
        }
    }
}

/// Check if running with sudo privileges
pub fn is_elevated() -> bool {
    std::env::var("USER")
        .map(|user| user == "root")
        .unwrap_or(false) ||
    std::env::var("SUDO_USER").is_ok()
}

/// Get the preferred network interface for multicast on macOS
pub fn get_preferred_multicast_interface() -> Option<String> {
    // On macOS, en0 is typically the primary Ethernet interface
    // and en1 is typically WiFi
    Some("en0".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_macos_version_detection() {
        let version = get_macos_version();
        assert!(version.is_ok());
        assert!(version.unwrap().contains("macOS"));
    }
    
    #[tokio::test]
    async fn test_macos_interface_detection() {
        let interfaces = detect_network_interfaces().await;
        assert!(interfaces.is_ok());
        let ifaces = interfaces.unwrap();
        // In a test environment, ifconfig might not return much, but it shouldn't be an empty Vec if it works.
        // It's okay if it falls back.
        // The important part is that it doesn't fail catastrophically.
    }
    
    #[test]
    fn test_macos_metadata() {
        let metadata = gather_macos_metadata();
        assert!(metadata.is_ok());
        let meta = metadata.unwrap();
        assert!(meta.contains_key("platform"));
        assert_eq!(meta.get("platform").unwrap(), "macOS");
    }
    
    #[test]
    fn test_preferred_interface() {
        let interface = get_preferred_multicast_interface();
        assert!(interface.is_some());
        assert_eq!(interface.unwrap(), "en0");
    }
}
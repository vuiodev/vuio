#[cfg(target_os = "linux")]
use super::{NetworkInterface, InterfaceType, PlatformResult};
use std::collections::HashMap;

/// Get Linux version information
pub fn get_linux_version() -> PlatformResult<String> {
    // Try to read from /etc/os-release first
    if let Ok(contents) = std::fs::read_to_string("/etc/os-release") {
        let mut name = None;
        let mut version = None;
        
        for line in contents.lines() {
            if let Some((key, value)) = line.split_once('=') {
                let value = value.trim_matches('"');
                match key {
                    "NAME" => name = Some(value),
                    "VERSION" => version = Some(value),
                    _ => {}
                }
            }
        }
        
        match (name, version) {
            (Some(name), Some(version)) => return Ok(format!("{} {}", name, version)),
            (Some(name), None) => return Ok(name.to_string()),
            _ => {}
        }
    }
    
    // Fallback to uname
    match std::process::Command::new("uname")
        .args(&["-sr"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(version)
        }
        _ => Ok("Linux (unknown version)".to_string()),
    }
}

/// Detect network interfaces on Linux
pub async fn detect_network_interfaces() -> PlatformResult<Vec<NetworkInterface>> {
    let mut interfaces = Vec::new();
    
    // Read from /proc/net/dev to get interface list
    if let Ok(contents) = std::fs::read_to_string("/proc/net/dev") {
        for line in contents.lines().skip(2) { // Skip header lines
            if let Some(interface_name) = line.split(':').next() {
                let interface_name = interface_name.trim().to_string();
                
                // Skip loopback
                if interface_name == "lo" {
                    continue;
                }
                
                // Determine interface type based on name
                let interface_type = if interface_name.starts_with("eth") {
                    InterfaceType::Ethernet
                } else if interface_name.starts_with("wlan") || interface_name.starts_with("wlp") {
                    InterfaceType::WiFi
                } else if interface_name.starts_with("tun") || interface_name.starts_with("tap") {
                    InterfaceType::VPN
                } else {
                    InterfaceType::Other(interface_name.clone())
                };
                
                // Check if interface is up by reading from /sys/class/net
                let is_up = std::fs::read_to_string(format!("/sys/class/net/{}/operstate", interface_name))
                    .map(|state| state.trim() == "up")
                    .unwrap_or(false);
                
                // Create interface (with placeholder IP - would need proper parsing from ip command)
                let interface = NetworkInterface {
                    name: interface_name,
                    ip_address: "127.0.0.1".parse().unwrap(), // Placeholder
                    is_loopback: false,
                    is_up,
                    supports_multicast: true, // Most Linux interfaces support multicast
                    interface_type,
                };
                
                interfaces.push(interface);
            }
        }
    } else {
        // Fallback: create a basic interface
        let interface = NetworkInterface {
            name: "eth0".to_string(),
            ip_address: "127.0.0.1".parse().unwrap(),
            is_loopback: false,
            is_up: true,
            supports_multicast: true,
            interface_type: InterfaceType::Ethernet,
        };
        interfaces.push(interface);
    }
    
    Ok(interfaces)
}

/// Gather Linux-specific metadata
pub fn gather_linux_metadata() -> PlatformResult<HashMap<String, String>> {
    let mut metadata = HashMap::new();
    
    metadata.insert("platform".to_string(), "Linux".to_string());
    
    // Read distribution information from /etc/os-release
    if let Ok(contents) = std::fs::read_to_string("/etc/os-release") {
        for line in contents.lines() {
            if let Some((key, value)) = line.split_once('=') {
                let value = value.trim_matches('"');
                let key = key.to_lowercase();
                metadata.insert(key, value.to_string());
            }
        }
    }
    
    // Get kernel version
    if let Ok(contents) = std::fs::read_to_string("/proc/version") {
        metadata.insert("kernel_version".to_string(), contents.trim().to_string());
    }
    
    // Get hostname
    if let Ok(contents) = std::fs::read_to_string("/proc/sys/kernel/hostname") {
        metadata.insert("hostname".to_string(), contents.trim().to_string());
    }
    
    // Get architecture
    if let Ok(output) = std::process::Command::new("uname").arg("-m").output() {
        if output.status.success() {
            let arch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            metadata.insert("architecture".to_string(), arch);
        }
    }
    
    // Check for systemd
    let has_systemd = std::path::Path::new("/run/systemd/system").exists();
    metadata.insert("has_systemd".to_string(), has_systemd.to_string());
    
    // Check for common security frameworks
    let has_selinux = std::path::Path::new("/sys/fs/selinux").exists();
    metadata.insert("has_selinux".to_string(), has_selinux.to_string());
    
    let has_apparmor = std::path::Path::new("/sys/kernel/security/apparmor").exists();
    metadata.insert("has_apparmor".to_string(), has_apparmor.to_string());
    
    Ok(metadata)
}

/// Check if running as root
pub fn is_elevated() -> bool {
    std::env::var("USER")
        .map(|user| user == "root")
        .unwrap_or(false) ||
    unsafe { libc::geteuid() == 0 }
}

/// Check Linux firewall status (simplified)
pub fn get_firewall_status() -> PlatformResult<bool> {
    // Check for common firewall tools
    let has_iptables = std::process::Command::new("which")
        .arg("iptables")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    
    let has_ufw = std::process::Command::new("which")
        .arg("ufw")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    
    let has_firewalld = std::process::Command::new("which")
        .arg("firewall-cmd")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    
    // If any firewall tool is present, assume firewall might be active
    Ok(has_iptables || has_ufw || has_firewalld)
}

/// Check if a port requires special privileges on Linux
pub fn requires_elevation(port: u16) -> bool {
    // Ports below 1024 require root privileges or CAP_NET_BIND_SERVICE capability
    port < 1024
}

/// Get available network namespaces
pub fn get_network_namespaces() -> PlatformResult<Vec<String>> {
    let mut namespaces = Vec::new();
    
    // Read from /proc/net/netns if available
    if let Ok(entries) = std::fs::read_dir("/var/run/netns") {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                namespaces.push(name.to_string());
            }
        }
    }
    
    Ok(namespaces)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_linux_version_detection() {
        let version = get_linux_version();
        assert!(version.is_ok());
        let ver = version.unwrap();
        assert!(!ver.is_empty());
    }
    
    #[tokio::test]
    async fn test_linux_interface_detection() {
        let interfaces = detect_network_interfaces().await;
        assert!(interfaces.is_ok());
        let ifaces = interfaces.unwrap();
        assert!(!ifaces.is_empty());
    }
    
    #[test]
    fn test_linux_metadata() {
        let metadata = gather_linux_metadata();
        assert!(metadata.is_ok());
        let meta = metadata.unwrap();
        assert!(meta.contains_key("platform"));
        assert_eq!(meta.get("platform").unwrap(), "Linux");
    }
    
    #[test]
    fn test_elevation_check() {
        let requires_root = requires_elevation(80);
        assert!(requires_root);
        
        let no_root_needed = requires_elevation(8080);
        assert!(!no_root_needed);
    }
    
    #[test]
    fn test_network_namespaces() {
        let namespaces = get_network_namespaces();
        assert!(namespaces.is_ok());
        // Namespaces list can be empty, that's fine
    }
}
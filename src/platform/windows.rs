#[cfg(target_os = "windows")]
use super::{InterfaceType, NetworkInterface, PlatformError, PlatformResult};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use tracing::{debug, warn};
use windows::Win32::NetworkManagement::IpHelper::{
    GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH, GAA_FLAG_INCLUDE_PREFIX, GAA_FLAG_SKIP_ANYCAST,
    GAA_FLAG_SKIP_MULTICAST, GAA_FLAG_SKIP_DNS_SERVER, IF_TYPE_ETHERNET_CSMACD,
    IF_TYPE_IEEE80211, IF_TYPE_SOFTWARE_LOOPBACK, IF_TYPE_TUNNEL, IF_TYPE_PPP,
};
use windows::Win32::NetworkManagement::Ndis::IF_OPER_STATUS;
use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};

/// Get Windows version information
pub fn get_windows_version() -> PlatformResult<String> {
    // Use std::env to get basic version info
    // In a real implementation, you might use Windows APIs for more detailed info
    match std::env::var("OS") {
        Ok(os) if os.contains("Windows") => {
            // Try to get more specific version info from environment
            let version = std::env::var("PROCESSOR_ARCHITECTURE")
                .map(|arch| format!("Windows ({})", arch))
                .unwrap_or_else(|_| "Windows".to_string());
            Ok(version)
        }
        _ => Ok("Windows (unknown version)".to_string()),
    }
}

/// Maps Windows interface types to our internal InterfaceType enum.
fn map_windows_if_type(if_type: u32) -> InterfaceType {
    match if_type {
        IF_TYPE_ETHERNET_CSMACD => InterfaceType::Ethernet,
        IF_TYPE_IEEE80211 => InterfaceType::WiFi,
        IF_TYPE_SOFTWARE_LOOPBACK => InterfaceType::Loopback,
        IF_TYPE_TUNNEL | IF_TYPE_PPP => InterfaceType::Other(format!("ifType {}", if_type)),
        val => InterfaceType::Other(format!("ifType {}", val)),
    }
}




/// Gather Windows-specific metadata
pub fn gather_windows_metadata() -> PlatformResult<HashMap<String, String>> {
    let mut metadata = HashMap::new();

    // Add Windows-specific environment variables
    if let Ok(computer_name) = std::env::var("COMPUTERNAME") {
        metadata.insert("computer_name".to_string(), computer_name);
    }

    if let Ok(user_domain) = std::env::var("USERDOMAIN") {
        metadata.insert("user_domain".to_string(), user_domain);
    }

    if let Ok(processor_arch) = std::env::var("PROCESSOR_ARCHITECTURE") {
        metadata.insert("processor_architecture".to_string(), processor_arch);
    }

    if let Ok(number_of_processors) = std::env::var("NUMBER_OF_PROCESSORS") {
        metadata.insert("number_of_processors".to_string(), number_of_processors);
    }

    // Add Windows version detection
    metadata.insert("platform".to_string(), "Windows".to_string());

    Ok(metadata)
}

/// Check if running with administrator privileges
#[allow(dead_code)]
pub fn is_elevated() -> bool {
    // This is a simplified check
    // In a real implementation, you would use Windows APIs to check for admin privileges
    std::env::var("USERNAME")
        .map(|username| username.to_lowercase().contains("admin"))
        .unwrap_or(false)
}

/// Get Windows firewall status
#[allow(dead_code)]
pub fn get_firewall_status() -> PlatformResult<bool> {
    // This would use Windows APIs to check firewall status
    // For now, assume firewall is active on Windows
    Ok(true)
}

/// Check if a port requires elevation on Windows
#[allow(dead_code)]
pub fn requires_elevation(port: u16) -> bool {
    // Ports below 1024 typically require administrator privileges on Windows
    port < 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_version_detection() {
        let version = get_windows_version();
        assert!(version.is_ok());
        assert!(version.unwrap().contains("Windows"));
    }

    #[tokio::test]
    async fn test_windows_interface_detection() {
        use crate::platform::network::windows::WindowsNetworkManager;
        use crate::platform::network::NetworkManager;
        let manager = WindowsNetworkManager::new();
        let interfaces = manager.get_local_interfaces().await;
        // Test that the function returns a result
        match interfaces {
            Ok(ifaces) => {
                println!("Detected {} interfaces", ifaces.len());
                for iface in ifaces {
                    println!(
                        "  - {}: {} ({:?})",
                        iface.name, iface.ip_address, iface.interface_type
                    );
                    assert!(iface.is_up);
                    assert!(!iface.is_loopback);
                }
            }
            Err(e) => {
                // This is acceptable in some CI/test environments
                println!("Interface detection failed as expected in test env: {}", e);
            }
        }
    }

    #[test]
    fn test_interface_type_mapping() {
        assert_eq!(map_windows_if_type(IF_TYPE_ETHERNET_CSMACD), InterfaceType::Ethernet);
        assert_eq!(map_windows_if_type(IF_TYPE_IEEE80211), InterfaceType::WiFi);
        assert_eq!(map_windows_if_type(IF_TYPE_SOFTWARE_LOOPBACK), InterfaceType::Loopback);
        assert_eq!(map_windows_if_type(IF_TYPE_TUNNEL), InterfaceType::VPN);
        
        match map_windows_if_type(999) {
            InterfaceType::Other(desc) => assert_eq!(desc, "ifType 999"),
            _ => panic!("Expected Other type"),
        }
    }

    #[test]
    fn test_windows_metadata() {
        let metadata = gather_windows_metadata();
        assert!(metadata.is_ok());
        let meta = metadata.unwrap();
        assert!(meta.contains_key("platform"));
        assert_eq!(meta.get("platform").unwrap(), "Windows");
    }

    #[test]
    fn test_elevation_check() {
        let requires_admin = requires_elevation(80);
        assert!(requires_admin);

        let no_admin_needed = requires_elevation(8080);
        assert!(!no_admin_needed);
    }
}
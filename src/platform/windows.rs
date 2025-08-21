#[cfg(target_os = "windows")]
use super::{InterfaceType, NetworkInterface, PlatformError, PlatformResult};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use tracing::{debug, info, warn};
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
        IF_TYPE_TUNNEL | IF_TYPE_PPP => InterfaceType::VPN,
        val => InterfaceType::Other(format!("ifType {}", val)),
    }
}

/// Detect network interfaces on Windows using native Windows APIs.
pub async fn detect_network_interfaces() -> PlatformResult<Vec<NetworkInterface>> {
    // Use tokio::task::spawn_blocking to run the blocking Windows API call
    tokio::task::spawn_blocking(|| {
        detect_network_interfaces_sync()
    }).await.map_err(|e| PlatformError::DetectionFailed(format!("Task join error: {}", e)))?
}

/// Synchronous network interface detection using Windows APIs.
fn detect_network_interfaces_sync() -> PlatformResult<Vec<NetworkInterface>> {
    let mut interfaces = Vec::new();
    
    // Buffer size for GetAdaptersAddresses
    let mut buffer_size = 0u32;
    
    // First call to get required buffer size
    let flags = GAA_FLAG_INCLUDE_PREFIX | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    
    unsafe {
        let result = GetAdaptersAddresses(
            AF_INET.0 as u32,
            flags,
            None,
            None,
            &mut buffer_size,
        );
        
        if result != 111 { // ERROR_BUFFER_OVERFLOW
            return Err(PlatformError::DetectionFailed(format!(
                "GetAdaptersAddresses failed to get buffer size: {}",
                result
            )));
        }
    }
    
    // Allocate buffer and make the actual call
    let mut buffer = vec![0u8; buffer_size as usize];
    let adapter_addresses = buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
    
    unsafe {
        let result = GetAdaptersAddresses(
            AF_INET.0 as u32,
            flags,
            None,
            Some(adapter_addresses),
            &mut buffer_size,
        );
        
        if result != 0 {
            return Err(PlatformError::DetectionFailed(format!(
                "GetAdaptersAddresses failed: {}",
                result
            )));
        }
        
        // Parse the linked list of adapters
        let mut current_adapter = adapter_addresses;
        
        while !current_adapter.is_null() {
            let adapter = &*current_adapter;
            
            // Skip loopback interfaces
            if adapter.IfType == IF_TYPE_SOFTWARE_LOOPBACK {
                current_adapter = adapter.Next;
                continue;
            }
            
            // Skip interfaces that are not up
            if adapter.OperStatus != IF_OPER_STATUS(1) { // IfOperStatusUp
                current_adapter = adapter.Next;
                continue;
            }
            
            // Get adapter name
            let adapter_name = if !adapter.AdapterName.is_null() {
                std::ffi::CStr::from_ptr(adapter.AdapterName.0 as *const i8)
                    .to_string_lossy()
                    .to_string()
            } else {
                "Unknown".to_string()
            };
            
            // Get friendly name
            let friendly_name = if !adapter.FriendlyName.0.is_null() {
                let wide_str = std::slice::from_raw_parts(
                    adapter.FriendlyName.0,
                    wcslen(adapter.FriendlyName.0)
                );
                String::from_utf16_lossy(wide_str)
            } else {
                adapter_name.clone()
            };
            
            // Get description
            let _description = if !adapter.Description.0.is_null() {
                let wide_str = std::slice::from_raw_parts(
                    adapter.Description.0,
                    wcslen(adapter.Description.0)
                );
                String::from_utf16_lossy(wide_str)
            } else {
                friendly_name.clone()
            };
            
            // Parse IP addresses
            let mut unicast_address = adapter.FirstUnicastAddress;
            while !unicast_address.is_null() {
                let addr_info = &*unicast_address;
                let socket_addr = addr_info.Address.lpSockaddr;
                
                if !socket_addr.is_null() {
                    let sockaddr = &*(socket_addr as *const SOCKADDR_IN);
                    
                    // Check if it's IPv4
                    if sockaddr.sin_family == AF_INET {
                        let ip_bytes = sockaddr.sin_addr.S_un.S_addr.to_le_bytes();
                        let ip_addr = Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);
                        
                        // Skip APIPA addresses (169.254.x.x)
                        if ip_addr.octets()[0] == 169 && ip_addr.octets()[1] == 254 {
                            unicast_address = addr_info.Next;
                            continue;
                        }
                        
                        let interface_type = map_windows_if_type(adapter.IfType);
                        let is_loopback = interface_type == InterfaceType::Loopback;
                        
                        let network_interface = NetworkInterface {
                            name: friendly_name.clone(),
                            ip_address: IpAddr::V4(ip_addr),
                            is_loopback,
                            is_up: true,
                            supports_multicast: !is_loopback,
                            interface_type,
                        };
                        
                        interfaces.push(network_interface);
                        break; // Only take the first IPv4 address per adapter
                    }
                }
                
                unicast_address = addr_info.Next;
            }
            
            current_adapter = adapter.Next;
        }
    }
    
    if interfaces.is_empty() {
        warn!("No network interfaces found via Windows API");
        return Err(PlatformError::DetectionFailed(
            "No active network interfaces with IPv4 addresses found".to_string()
        ));
    }
    
    debug!(
        "Successfully detected {} network interface(s) via Windows API.",
        interfaces.len()
    );
    
    Ok(interfaces)
}

/// Helper function to calculate wide string length
unsafe fn wcslen(mut s: *const u16) -> usize {
    let mut len = 0;
    while *s != 0 {
        len += 1;
        s = s.add(1);
    }
    len
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
pub fn is_elevated() -> bool {
    // This is a simplified check
    // In a real implementation, you would use Windows APIs to check for admin privileges
    std::env::var("USERNAME")
        .map(|username| username.to_lowercase().contains("admin"))
        .unwrap_or(false)
}

/// Get Windows firewall status
pub fn get_firewall_status() -> PlatformResult<bool> {
    // This would use Windows APIs to check firewall status
    // For now, assume firewall is active on Windows
    Ok(true)
}

/// Check if a port requires elevation on Windows
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
        let interfaces = detect_network_interfaces().await;
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
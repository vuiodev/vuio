#[cfg(target_os = "freebsd")]
use super::{NetworkInterface, InterfaceType, PlatformResult};
use std::collections::HashMap;
use std::net::IpAddr;
use std::process::Command;

/// Get BSD version information
pub fn get_bsd_version() -> PlatformResult<String> {
    // Try to get detailed system information using uname
    match Command::new("uname").args(&["-sr"]).output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(version)
        }
        _ => {
            // Fallback to basic FreeBSD identification
            Ok("FreeBSD (unknown version)".to_string())
        }
    }
}


/// Gather BSD-specific metadata
pub fn gather_bsd_metadata() -> PlatformResult<HashMap<String, String>> {
    let mut metadata = HashMap::new();
    
    metadata.insert("platform".to_string(), "BSD".to_string());
    
    // Get system information using uname
    if let Ok(output) = Command::new("uname").arg("-a").output() {
        if output.status.success() {
            let uname_output = String::from_utf8_lossy(&output.stdout).trim().to_string();
            metadata.insert("uname".to_string(), uname_output);
        }
    }
    
    // Get FreeBSD version
    if let Ok(output) = Command::new("uname").arg("-r").output() {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            metadata.insert("version".to_string(), version);
        }
    }
    
    // Get architecture
    if let Ok(output) = Command::new("uname").arg("-m").output() {
        if output.status.success() {
            let arch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            metadata.insert("architecture".to_string(), arch);
        }
    }
    
    // Get hostname
    if let Ok(output) = Command::new("hostname").output() {
        if output.status.success() {
            let hostname = String::from_utf8_lossy(&output.stdout).trim().to_string();
            metadata.insert("hostname".to_string(), hostname);
        }
    }
    
    // Check for common BSD services and features
    let has_pf = std::path::Path::new("/etc/pf.conf").exists();
    metadata.insert("has_pf".to_string(), has_pf.to_string());
    
    let has_ipfw = std::path::Path::new("/sbin/ipfw").exists();
    metadata.insert("has_ipfw".to_string(), has_ipfw.to_string());
    
    // Check for package manager
    let has_pkg = std::path::Path::new("/usr/sbin/pkg").exists() || 
                  std::path::Path::new("/usr/local/sbin/pkg").exists();
    metadata.insert("has_pkg".to_string(), has_pkg.to_string());
    
    // Check for jails
    if let Ok(output) = Command::new("sysctl").arg("security.jail.jailed").output() {
        if output.status.success() {
            let jail_status = String::from_utf8_lossy(&output.stdout);
            let is_jailed = jail_status.contains("security.jail.jailed: 1");
            metadata.insert("is_jailed".to_string(), is_jailed.to_string());
        }
    }
    
    Ok(metadata)
}

/// Check if running as root
pub fn _is_elevated() -> bool {
    // Check USER environment variable first
    if let Ok(user) = std::env::var("USER") {
        return user == "root";
    }
    
    // Conservative fallback - assume not elevated to avoid linking issues
    false
}

/// Check BSD firewall status
pub fn _get_firewall_status() -> PlatformResult<bool> {
    // Check for pf (Packet Filter)
    let has_pf = Command::new("pfctl")
        .arg("-si")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    
    // Check for ipfw
    let has_ipfw = Command::new("ipfw")
        .arg("list")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    
    Ok(has_pf || has_ipfw)
}

/// Check if a port requires special privileges on BSD
pub fn _requires_elevation(port: u16) -> bool {
    // Ports below 1024 require root privileges
    port < 1024
}

/// Get system information using sysctl
pub fn _get_sysctl_info(key: &str) -> PlatformResult<String> {
    match Command::new("sysctl").arg("-n").arg(key).output() {
        Ok(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        }
        Ok(_) => Err(super::PlatformError::Bsd(super::BsdError::SysctlFailed {
            key: key.to_string(),
            reason: "Command failed".to_string(),
        })),
        Err(e) => Err(super::PlatformError::Bsd(super::BsdError::SysctlFailed {
            key: key.to_string(),
            reason: e.to_string(),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bsd_version_detection() {
        let version = get_bsd_version();
        assert!(version.is_ok());
        let ver = version.unwrap();
        assert!(!ver.is_empty());
    }
    
    #[tokio::test]
    async fn test_bsd_interface_detection() {
        use crate::platform::network::linux::LinuxNetworkManager;
        use crate::platform::network::NetworkManager;
        let manager = LinuxNetworkManager::new();
        let interfaces = manager.get_local_interfaces().await;
        assert!(interfaces.is_ok());
        let ifaces = interfaces.unwrap();
        assert!(!ifaces.is_empty());
    }
    
    #[test]
    fn test_bsd_metadata() {
        let metadata = gather_bsd_metadata();
        assert!(metadata.is_ok());
        let meta = metadata.unwrap();
        assert!(meta.contains_key("platform"));
        assert_eq!(meta.get("platform").unwrap(), "BSD");
    }
    
    #[test]
    fn test_elevation_check() {
        let requires_root = _requires_elevation(80);
        assert!(requires_root);
        
        let no_root_needed = _requires_elevation(8080);
        assert!(!no_root_needed);
    }
    
    #[test]
    fn test_ifconfig_parsing() {
        let sample_output = r#"
em0: flags=8843<UP,BROADCAST,RUNNING,SIMPLEX,MULTICAST> metric 0 mtu 1500
	options=481249b<RXCSUM,TXCSUM,VLAN_MTU,VLAN_HWTAGGING,VLAN_HWCSUM,LRO,WOL_MAGIC,VLAN_HWFILTER,NOMAP>
	ether 08:00:27:12:34:56
	inet 192.168.1.100 netmask 0xffffff00 broadcast 192.168.1.255
	media: Ethernet autoselect (1000baseT <full-duplex>)
	status: active
lo0: flags=8049<UP,LOOPBACK,RUNNING,MULTICAST> metric 0 mtu 16384
	options=680003<RXCSUM,TXCSUM,LINKSTATE,RXCSUM_IPV6,TXCSUM_IPV6>
	inet6 ::1 prefixlen 128
	inet6 fe80::1%lo0 prefixlen 64 scopeid 0x1
	inet 127.0.0.1 netmask 0xff000000
	groups: lo
"#;
        
        let interfaces = parse_ifconfig_output(sample_output);
        assert!(interfaces.is_ok());
        let ifaces = interfaces.unwrap();
        assert_eq!(ifaces.len(), 1); // Should exclude loopback
        assert_eq!(ifaces[0].name, "em0");
        assert!(ifaces[0].is_up);
        assert_eq!(ifaces[0].ip_address.to_string(), "192.168.1.100");
    }
}

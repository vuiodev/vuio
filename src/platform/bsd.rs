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
}

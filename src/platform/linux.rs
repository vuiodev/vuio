#[cfg(target_os = "linux")]
use super::PlatformResult;
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
        use crate::platform::network::linux::LinuxNetworkManager;
        use crate::platform::network::NetworkManager;
        let manager = LinuxNetworkManager::new();
        let interfaces = manager.get_local_interfaces().await;
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
    

}
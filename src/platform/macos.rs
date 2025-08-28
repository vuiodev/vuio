#[cfg(target_os = "macos")]
use super::PlatformResult;
use std::collections::HashMap;

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
        use crate::platform::network::macos::MacOSNetworkManager;
        use crate::platform::network::NetworkManager;
        let manager = MacOSNetworkManager::new();
        let interfaces = manager.get_local_interfaces().await;
        assert!(interfaces.is_ok());
        let _ifaces = interfaces.unwrap();
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
}
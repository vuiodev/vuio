use std::collections::HashMap;
use std::net::IpAddr;


pub mod config;
pub mod diagnostics;
pub mod error;
pub mod filesystem;
pub mod network;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "freebsd")]
mod bsd;

// Re-export the comprehensive error types from the error module
pub use error::{
    BsdError, ConfigurationError, DatabaseError, LinuxError, MacOSError, PlatformError, PlatformResult,
    WindowsError,
};

/// Operating system types supported by the platform abstraction layer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OsType {
    Windows,
    MacOS,
    Linux,
    Bsd,
}

impl OsType {
    /// Detect the current operating system
    pub fn current() -> Self {
        #[cfg(target_os = "windows")]
        return OsType::Windows;

        #[cfg(target_os = "macos")]
        return OsType::MacOS;

        #[cfg(target_os = "linux")]
        return OsType::Linux;

        #[cfg(target_os = "freebsd")]
        return OsType::Bsd;

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux", target_os = "freebsd")))]
        compile_error!("Unsupported operating system");
    }

    /// Get the display name for the operating system
    pub fn display_name(&self) -> &'static str {
        match self {
            OsType::Windows => "Windows",
            OsType::MacOS => "macOS",
            OsType::Linux => "Linux",
            OsType::Bsd => "FreeBSD",
        }
    }
}

/// Platform capabilities that affect application behavior
#[derive(Debug, Clone)]
pub struct PlatformCapabilities {
    /// Whether the file system is case-sensitive
    pub case_sensitive_fs: bool,
}

impl PlatformCapabilities {
    /// Get platform capabilities for the current operating system
    pub fn for_current_platform() -> Self {
        #[cfg(target_os = "windows")]
        return Self {
            case_sensitive_fs: false, // NTFS is case-insensitive by default
        };

        #[cfg(target_os = "macos")]
        return Self {
            case_sensitive_fs: true, // APFS is case-sensitive
        };

        #[cfg(target_os = "linux")]
        return Self {
            case_sensitive_fs: true, // ext4/xfs are case-sensitive
        };

        #[cfg(target_os = "freebsd")]
        return Self {
            case_sensitive_fs: true, // UFS/ZFS are case-sensitive
        };
    }
}

/// Network interface information
#[derive(Debug, Clone)]
pub struct NetworkInterface {
    /// Interface name (e.g., "eth0", "wlan0", "Ethernet")
    pub name: String,

    /// Primary IP address of the interface
    pub ip_address: IpAddr,

    /// Whether this is a loopback interface
    pub is_loopback: bool,

    /// Whether the interface is currently up and active
    pub is_up: bool,

    /// Whether the interface supports multicast
    pub supports_multicast: bool,

    /// Type of network interface
    pub interface_type: InterfaceType,
}

/// Types of network interfaces
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterfaceType {
    Ethernet,
    WiFi,
    VPN,
    Loopback,
    Other(String),
}

/// Comprehensive platform information
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    /// Operating system type
    pub os_type: OsType,

    /// Operating system version string
    pub version: String,

    /// Platform-specific capabilities
    pub capabilities: PlatformCapabilities,

    /// Available network interfaces
    pub network_interfaces: Vec<NetworkInterface>,

    /// Additional platform-specific metadata
    pub metadata: HashMap<String, String>,
}

impl PlatformInfo {
    /// Detect and gather comprehensive platform information
    pub async fn detect() -> Result<Self, PlatformError> {
        let os_type = OsType::current();
        let capabilities = PlatformCapabilities::for_current_platform();

        // Get OS version
        let version = Self::get_os_version()?;

        // Detect network interfaces
        let network_interfaces = Self::detect_network_interfaces().await?;

        // Gather platform-specific metadata
        let metadata = Self::gather_metadata(&os_type)?;

        Ok(PlatformInfo {
            os_type,
            version,
            capabilities,
            network_interfaces,
            metadata,
        })
    }

    /// Get the operating system version string
    fn get_os_version() -> Result<String, PlatformError> {
        #[cfg(target_os = "windows")]
        {
            windows::get_windows_version()
        }

        #[cfg(target_os = "macos")]
        {
            macos::get_macos_version()
        }

        #[cfg(target_os = "linux")]
        {
            linux::get_linux_version()
        }

        #[cfg(target_os = "freebsd")]
        {
            bsd::get_bsd_version()
        }
    }

    /// Detect available network interfaces
    async fn detect_network_interfaces() -> Result<Vec<NetworkInterface>, PlatformError> {
        #[cfg(target_os = "windows")]
        {
            windows::detect_network_interfaces().await
        }

        #[cfg(target_os = "macos")]
        {
            macos::detect_network_interfaces().await
        }

        #[cfg(target_os = "linux")]
        {
            linux::detect_network_interfaces().await
        }

        #[cfg(target_os = "freebsd")]
        {
            bsd::detect_network_interfaces().await
        }
    }

    /// Gather platform-specific metadata
    fn gather_metadata(os_type: &OsType) -> Result<HashMap<String, String>, PlatformError> {
        let mut metadata = HashMap::new();

        // Add common metadata
        metadata.insert(
            "architecture".to_string(),
            std::env::consts::ARCH.to_string(),
        );

        // Add platform-specific metadata
        match os_type {
            #[cfg(target_os = "windows")]
            OsType::Windows => {
                if let Ok(additional) = windows::gather_windows_metadata() {
                    metadata.extend(additional);
                }
            }

            #[cfg(target_os = "macos")]
            OsType::MacOS => {
                if let Ok(additional) = macos::gather_macos_metadata() {
                    metadata.extend(additional);
                }
            }

            #[cfg(target_os = "linux")]
            OsType::Linux => {
                if let Ok(additional) = linux::gather_linux_metadata() {
                    metadata.extend(additional);
                }
            }

            #[cfg(target_os = "freebsd")]
            OsType::Bsd => {
                if let Ok(additional) = bsd::gather_bsd_metadata() {
                    metadata.extend(additional);
                }
            }

            // Handle cases where we're compiling for a different target
            _ => {}
        }

        Ok(metadata)
    }

    /// Get the best network interface for DLNA operations using a deterministic priority.
    pub fn get_primary_interface(&self) -> Option<&NetworkInterface> {
        // A simple, deterministic approach to finding the best interface.
        
        // Priority 1: Find the first active, non-loopback Ethernet interface with a private IPv4 address.
        if let Some(iface) = self.network_interfaces.iter().find(|i| {
            i.is_up && !i.is_loopback && i.interface_type == InterfaceType::Ethernet &&
            matches!(i.ip_address, IpAddr::V4(ip) if ip.is_private())
        }) {
            return Some(iface);
        }

        // Priority 2: Find the first active, non-loopback Wi-Fi interface with a private IPv4 address.
        if let Some(iface) = self.network_interfaces.iter().find(|i| {
            i.is_up && !i.is_loopback && i.interface_type == InterfaceType::WiFi &&
            matches!(i.ip_address, IpAddr::V4(ip) if ip.is_private())
        }) {
            return Some(iface);
        }

        // Priority 3: Find any other active, non-loopback interface with a private IPv4 address.
        if let Some(iface) = self.network_interfaces.iter().find(|i| {
            i.is_up && !i.is_loopback &&
            matches!(i.ip_address, IpAddr::V4(ip) if ip.is_private())
        }) {
            return Some(iface);
        }
        
        // Priority 4: As a last resort, take the first active, non-loopback interface of any kind.
        self.network_interfaces.iter().find(|i| {
            i.is_up && !i.is_loopback
        })
    }

    /// Check if the platform supports a specific feature
    pub fn supports_feature(&self, feature: &str) -> bool {
        match feature {
            "case_sensitive_fs" => self.capabilities.case_sensitive_fs,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_type_detection() {
        let os_type = OsType::current();

        // Verify we get a valid OS type
        match os_type {
            OsType::Windows | OsType::MacOS | OsType::Linux | OsType::Bsd => {
                // Valid OS type detected
                assert!(!os_type.display_name().is_empty());
            }
        }
    }

    #[test]
    fn test_platform_capabilities() {
        let capabilities = PlatformCapabilities::for_current_platform();

        // Test case sensitivity detection works
        // (actual value depends on platform)
    }

    #[tokio::test]
    async fn test_platform_info_detection() {
        let platform_info = PlatformInfo::detect().await;

        // Platform detection should succeed
        assert!(platform_info.is_ok());

        let info = platform_info.unwrap();
        assert!(!info.version.is_empty());
        assert!(!info.metadata.is_empty());
    }
}
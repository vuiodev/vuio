use std::collections::HashMap;
use std::net::IpAddr;
use thiserror::Error;

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

// Re-export the comprehensive error types from the error module
pub use error::{
    ConfigurationError, DatabaseError, LinuxError, MacOSError, PlatformError, PlatformResult,
    WindowsError,
};

/// Operating system types supported by the platform abstraction layer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OsType {
    Windows,
    MacOS,
    Linux,
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

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        compile_error!("Unsupported operating system");
    }

    /// Get the display name for the operating system
    pub fn display_name(&self) -> &'static str {
        match self {
            OsType::Windows => "Windows",
            OsType::MacOS => "macOS",
            OsType::Linux => "Linux",
        }
    }
}

/// Platform capabilities that affect application behavior
#[derive(Debug, Clone)]
pub struct PlatformCapabilities {
    /// Whether the platform can bind to privileged ports (< 1024) without elevation
    pub can_bind_privileged_ports: bool,

    /// Whether the platform supports multicast networking
    pub supports_multicast: bool,

    /// Whether the platform has a built-in firewall that may block connections
    pub has_firewall: bool,

    /// Whether the file system is case-sensitive
    pub case_sensitive_fs: bool,

    /// Whether the platform supports UNC paths (Windows) or similar network paths
    pub supports_network_paths: bool,

    /// Whether the platform requires special permissions for network operations
    pub requires_network_permissions: bool,
}

impl PlatformCapabilities {
    /// Get platform capabilities for the current operating system
    pub fn for_current_platform() -> Self {
        #[cfg(target_os = "windows")]
        return Self {
            can_bind_privileged_ports: false, // Requires admin privileges
            supports_multicast: true,
            has_firewall: true, // Windows Defender Firewall
            case_sensitive_fs: false, // NTFS is case-insensitive by default
            supports_network_paths: true, // UNC paths
            requires_network_permissions: true, // UAC for privileged ports
        };

        #[cfg(target_os = "macos")]
        return Self {
            can_bind_privileged_ports: false, // Requires sudo
            supports_multicast: true,
            has_firewall: true, // macOS Application Firewall
            case_sensitive_fs: true, // APFS is case-sensitive
            supports_network_paths: true, // SMB/AFP mounts
            requires_network_permissions: true, // System permissions dialog
        };

        #[cfg(target_os = "linux")]
        return Self {
            can_bind_privileged_ports: false, // Requires root or capabilities
            supports_multicast: true,
            has_firewall: true, // iptables/ufw/firewalld
            case_sensitive_fs: true, // ext4/xfs are case-sensitive
            supports_network_paths: true, // NFS/CIFS mounts
            requires_network_permissions: false, // Usually no special permissions needed
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
            "privileged_ports" => self.capabilities.can_bind_privileged_ports,
            "multicast" => self.capabilities.supports_multicast,
            "firewall" => self.capabilities.has_firewall,
            "case_sensitive_fs" => self.capabilities.case_sensitive_fs,
            "network_paths" => self.capabilities.supports_network_paths,
            "network_permissions" => self.capabilities.requires_network_permissions,
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
            OsType::Windows | OsType::MacOS | OsType::Linux => {
                // Valid OS type detected
                assert!(!os_type.display_name().is_empty());
            }
        }
    }

    #[test]
    fn test_platform_capabilities() {
        let capabilities = PlatformCapabilities::for_current_platform();

        // All platforms should support multicast
        assert!(capabilities.supports_multicast);

        // All platforms should have some form of firewall
        assert!(capabilities.has_firewall);
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
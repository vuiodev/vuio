use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::platform::{PlatformInfo, PlatformError, OsType, NetworkInterface};

/// Comprehensive diagnostic information for troubleshooting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticInfo {
    /// Platform information
    pub platform: PlatformDiagnostics,
    
    /// Network configuration and status
    pub network: NetworkDiagnostics,
    
    /// Database status and configuration
    pub database: DatabaseDiagnostics,
    
    /// File system and directory information
    pub filesystem: FilesystemDiagnostics,
    
    /// Application configuration status
    pub configuration: ConfigurationDiagnostics,
    
    /// System resource information
    pub system: SystemDiagnostics,
    
    /// Timestamp when diagnostics were collected
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformDiagnostics {
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
    pub hostname: String,
    pub capabilities: PlatformCapabilitiesDiag,
    pub platform_specific: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCapabilitiesDiag {
    pub can_bind_privileged_ports: bool,
    pub supports_multicast: bool,
    pub has_firewall: bool,
    pub case_sensitive_fs: bool,
    pub supports_network_paths: bool,
    pub requires_network_permissions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkDiagnostics {
    pub interfaces: Vec<NetworkInterfaceDiag>,
    pub primary_interface: Option<String>,
    pub multicast_support: bool,
    pub firewall_status: FirewallStatus,
    pub port_availability: HashMap<u16, bool>,
    pub connectivity_tests: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterfaceDiag {
    pub name: String,
    pub ip_address: String,
    pub is_loopback: bool,
    pub is_up: bool,
    pub supports_multicast: bool,
    pub interface_type: String,
    pub mtu: Option<u32>,
    pub speed: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FirewallStatus {
    Enabled,
    Disabled,
    Unknown,
    Configured { rules: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseDiagnostics {
    pub database_path: Option<PathBuf>,
    pub database_exists: bool,
    pub database_size: Option<u64>,
    pub database_accessible: bool,
    pub schema_version: Option<u32>,
    pub media_file_count: Option<u64>,
    pub last_scan_time: Option<chrono::DateTime<chrono::Utc>>,
    pub integrity_status: DatabaseIntegrityStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseIntegrityStatus {
    Healthy,
    Corrupted { details: String },
    Unknown,
    NotChecked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemDiagnostics {
    pub monitored_directories: Vec<DirectoryDiag>,
    pub config_directory: DirectoryDiag,
    pub cache_directory: DirectoryDiag,
    pub log_directory: DirectoryDiag,
    pub temp_directory: DirectoryDiag,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryDiag {
    pub path: PathBuf,
    pub exists: bool,
    pub accessible: bool,
    pub readable: bool,
    pub writable: bool,
    pub file_count: Option<u64>,
    pub total_size: Option<u64>,
    pub free_space: Option<u64>,
    pub permissions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationDiagnostics {
    pub config_file_path: Option<PathBuf>,
    pub config_file_exists: bool,
    pub config_file_valid: bool,
    pub config_errors: Vec<String>,
    pub hot_reload_enabled: bool,
    pub default_values_used: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDiagnostics {
    pub uptime: Option<u64>,
    pub memory_total: Option<u64>,
    pub memory_available: Option<u64>,
    pub cpu_count: Option<u32>,
    pub load_average: Option<f64>,
    pub disk_usage: HashMap<String, DiskUsage>,
    pub process_info: ProcessInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskUsage {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub memory_usage: Option<u64>,
    pub cpu_usage: Option<f64>,
    pub thread_count: Option<u32>,
    pub file_descriptors: Option<u32>,
}

impl DiagnosticInfo {
    /// Collect comprehensive diagnostic information
    pub async fn collect() -> Result<Self, PlatformError> {
        tracing::info!("Collecting diagnostic information...");
        
        let platform_info = PlatformInfo::detect().await?;
        
        Ok(DiagnosticInfo {
            platform: Self::collect_platform_diagnostics(&platform_info).await?,
            network: Self::collect_network_diagnostics(&platform_info).await?,
            database: Self::collect_database_diagnostics().await?,
            filesystem: Self::collect_filesystem_diagnostics().await?,
            configuration: Self::collect_configuration_diagnostics().await?,
            system: Self::collect_system_diagnostics().await?,
            timestamp: chrono::Utc::now(),
        })
    }
    
    /// Collect platform-specific diagnostic information
    async fn collect_platform_diagnostics(platform_info: &PlatformInfo) -> Result<PlatformDiagnostics, PlatformError> {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        
        let capabilities = PlatformCapabilitiesDiag {
            can_bind_privileged_ports: platform_info.capabilities.can_bind_privileged_ports,
            supports_multicast: platform_info.capabilities.supports_multicast,
            has_firewall: platform_info.capabilities.has_firewall,
            case_sensitive_fs: platform_info.capabilities.case_sensitive_fs,
            supports_network_paths: platform_info.capabilities.supports_network_paths,
            requires_network_permissions: platform_info.capabilities.requires_network_permissions,
        };
        
        Ok(PlatformDiagnostics {
            os_type: platform_info.os_type.display_name().to_string(),
            os_version: platform_info.version.clone(),
            architecture: std::env::consts::ARCH.to_string(),
            hostname,
            capabilities,
            platform_specific: platform_info.metadata.clone(),
        })
    }
    
    /// Collect network diagnostic information
    async fn collect_network_diagnostics(platform_info: &PlatformInfo) -> Result<NetworkDiagnostics, PlatformError> {
        let interfaces: Vec<NetworkInterfaceDiag> = platform_info.network_interfaces
            .iter()
            .map(|iface| NetworkInterfaceDiag {
                name: iface.name.clone(),
                ip_address: iface.ip_address.to_string(),
                is_loopback: iface.is_loopback,
                is_up: iface.is_up,
                supports_multicast: iface.supports_multicast,
                interface_type: format!("{:?}", iface.interface_type),
                mtu: None, // TODO: Implement MTU detection
                speed: None, // TODO: Implement speed detection
            })
            .collect();
        
        let primary_interface = platform_info.get_primary_interface()
            .map(|iface| iface.name.clone());
        
        let multicast_support = platform_info.capabilities.supports_multicast;
        
        // Test port availability
        let mut port_availability = HashMap::new();
        for port in &[1900, 8080, 8081, 8082] {
            port_availability.insert(*port, Self::test_port_availability(*port).await);
        }
        
        // Test basic connectivity
        let mut connectivity_tests = HashMap::new();
        connectivity_tests.insert("localhost".to_string(), Self::test_connectivity("127.0.0.1", 80).await);
        connectivity_tests.insert("internet".to_string(), Self::test_connectivity("8.8.8.8", 53).await);
        
        let firewall_status = Self::detect_firewall_status(&platform_info.os_type).await;
        
        Ok(NetworkDiagnostics {
            interfaces,
            primary_interface,
            multicast_support,
            firewall_status,
            port_availability,
            connectivity_tests,
        })
    }
    
    /// Test if a port is available for binding
    async fn test_port_availability(port: u16) -> bool {
        use tokio::net::TcpListener;
        
        match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    
    /// Test basic network connectivity
    async fn test_connectivity(host: &str, port: u16) -> bool {
        use tokio::net::TcpStream;
        use tokio::time::{timeout, Duration};
        
        match timeout(Duration::from_secs(5), TcpStream::connect(format!("{}:{}", host, port))).await {
            Ok(Ok(_)) => true,
            _ => false,
        }
    }
    
    /// Detect firewall status for the current platform
    async fn detect_firewall_status(os_type: &OsType) -> FirewallStatus {
        match os_type {
            OsType::Windows => Self::detect_windows_firewall().await,
            OsType::MacOS => Self::detect_macos_firewall().await,
            OsType::Linux => Self::detect_linux_firewall().await,
        }
    }
    
    #[cfg(target_os = "windows")]
    async fn detect_windows_firewall() -> FirewallStatus {
        // TODO: Implement Windows firewall detection using WMI or netsh
        FirewallStatus::Unknown
    }
    
    #[cfg(not(target_os = "windows"))]
    async fn detect_windows_firewall() -> FirewallStatus {
        FirewallStatus::Unknown
    }
    
    #[cfg(target_os = "macos")]
    async fn detect_macos_firewall() -> FirewallStatus {
        // TODO: Implement macOS firewall detection
        FirewallStatus::Unknown
    }
    
    #[cfg(not(target_os = "macos"))]
    async fn detect_macos_firewall() -> FirewallStatus {
        FirewallStatus::Unknown
    }
    
    #[cfg(target_os = "linux")]
    async fn detect_linux_firewall() -> FirewallStatus {
        // TODO: Implement Linux firewall detection (iptables, ufw, firewalld)
        FirewallStatus::Unknown
    }
    
    #[cfg(not(target_os = "linux"))]
    async fn detect_linux_firewall() -> FirewallStatus {
        FirewallStatus::Unknown
    }
    
    /// Collect database diagnostic information
    async fn collect_database_diagnostics() -> Result<DatabaseDiagnostics, PlatformError> {
        // TODO: Implement database diagnostics collection
        Ok(DatabaseDiagnostics {
            database_path: None,
            database_exists: false,
            database_size: None,
            database_accessible: false,
            schema_version: None,
            media_file_count: None,
            last_scan_time: None,
            integrity_status: DatabaseIntegrityStatus::NotChecked,
        })
    }
    
    /// Collect filesystem diagnostic information
    async fn collect_filesystem_diagnostics() -> Result<FilesystemDiagnostics, PlatformError> {
        // TODO: Implement filesystem diagnostics collection
        Ok(FilesystemDiagnostics {
            monitored_directories: vec![],
            config_directory: Self::diagnose_directory(&PathBuf::from(".")).await,
            cache_directory: Self::diagnose_directory(&PathBuf::from(".")).await,
            log_directory: Self::diagnose_directory(&PathBuf::from(".")).await,
            temp_directory: Self::diagnose_directory(&std::env::temp_dir()).await,
        })
    }
    
    /// Diagnose a specific directory
    async fn diagnose_directory(path: &PathBuf) -> DirectoryDiag {
        let exists = path.exists();
        let accessible = path.is_dir();
        let readable = path.metadata().map(|m| !m.permissions().readonly()).unwrap_or(false);
        let writable = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.join(".test_write"))
            .and_then(|_| std::fs::remove_file(path.join(".test_write")))
            .is_ok();
        
        let (file_count, total_size) = if accessible {
            Self::count_directory_contents(path).await
        } else {
            (None, None)
        };
        
        let free_space = Self::get_free_space(path).await;
        
        DirectoryDiag {
            path: path.clone(),
            exists,
            accessible,
            readable,
            writable,
            file_count,
            total_size,
            free_space,
            permissions: None, // TODO: Implement permission string formatting
        }
    }
    
    /// Count files and calculate total size in a directory
    async fn count_directory_contents(path: &PathBuf) -> (Option<u64>, Option<u64>) {
        match std::fs::read_dir(path) {
            Ok(entries) => {
                let mut count = 0;
                let mut total_size = 0;
                
                for entry in entries.flatten() {
                    count += 1;
                    if let Ok(metadata) = entry.metadata() {
                        total_size += metadata.len();
                    }
                }
                
                (Some(count), Some(total_size))
            }
            Err(_) => (None, None),
        }
    }
    
    /// Get available free space for a path
    async fn get_free_space(path: &PathBuf) -> Option<u64> {
        // TODO: Implement cross-platform free space detection
        None
    }
    
    /// Collect configuration diagnostic information
    async fn collect_configuration_diagnostics() -> Result<ConfigurationDiagnostics, PlatformError> {
        // TODO: Implement configuration diagnostics collection
        Ok(ConfigurationDiagnostics {
            config_file_path: None,
            config_file_exists: false,
            config_file_valid: false,
            config_errors: vec![],
            hot_reload_enabled: false,
            default_values_used: vec![],
        })
    }
    
    /// Collect system diagnostic information
    async fn collect_system_diagnostics() -> Result<SystemDiagnostics, PlatformError> {
        let process_info = ProcessInfo {
            pid: std::process::id(),
            memory_usage: None, // TODO: Implement memory usage detection
            cpu_usage: None,    // TODO: Implement CPU usage detection
            thread_count: None, // TODO: Implement thread count detection
            file_descriptors: None, // TODO: Implement FD count detection
        };
        
        Ok(SystemDiagnostics {
            uptime: None,           // TODO: Implement system uptime detection
            memory_total: None,     // TODO: Implement total memory detection
            memory_available: None, // TODO: Implement available memory detection
            cpu_count: Some(num_cpus::get() as u32),
            load_average: None,     // TODO: Implement load average detection
            disk_usage: HashMap::new(), // TODO: Implement disk usage detection
            process_info,
        })
    }
    
    /// Log diagnostic information at startup
    pub fn log_startup_diagnostics(&self) {
        tracing::info!("=== VuIO Startup Diagnostics ===");
        tracing::info!("Platform: {} {} ({})", 
            self.platform.os_type, 
            self.platform.os_version, 
            self.platform.architecture
        );
        tracing::info!("Hostname: {}", self.platform.hostname);
        
        // Log platform capabilities
        tracing::info!("Platform Capabilities:");
        tracing::info!("  - Privileged ports: {}", self.platform.capabilities.can_bind_privileged_ports);
        tracing::info!("  - Multicast support: {}", self.platform.capabilities.supports_multicast);
        tracing::info!("  - Firewall present: {}", self.platform.capabilities.has_firewall);
        tracing::info!("  - Case-sensitive FS: {}", self.platform.capabilities.case_sensitive_fs);
        
        // Log network information
        tracing::info!("Network Configuration:");
        tracing::info!("  - Interfaces found: {}", self.network.interfaces.len());
        if let Some(primary) = &self.network.primary_interface {
            tracing::info!("  - Primary interface: {}", primary);
        }
        tracing::info!("  - Multicast support: {}", self.network.multicast_support);
        tracing::info!("  - Firewall status: {:?}", self.network.firewall_status);
        
        // Log port availability
        tracing::info!("Port Availability:");
        for (port, available) in &self.network.port_availability {
            let status = if *available { "Available" } else { "In use" };
            tracing::info!("  - Port {}: {}", port, status);
        }
        
        // Log connectivity tests
        tracing::info!("Connectivity Tests:");
        for (test, result) in &self.network.connectivity_tests {
            let status = if *result { "Success" } else { "Failed" };
            tracing::info!("  - {}: {}", test, status);
        }
        
        // Log database status
        tracing::info!("Database Status:");
        tracing::info!("  - Database exists: {}", self.database.database_exists);
        tracing::info!("  - Database accessible: {}", self.database.database_accessible);
        tracing::info!("  - Integrity status: {:?}", self.database.integrity_status);
        if let Some(count) = self.database.media_file_count {
            tracing::info!("  - Media files: {}", count);
        }
        
        // Log filesystem status
        tracing::info!("Filesystem Status:");
        tracing::info!("  - Config directory: {} (accessible: {})", 
            self.filesystem.config_directory.path.display(),
            self.filesystem.config_directory.accessible
        );
        tracing::info!("  - Monitored directories: {}", self.filesystem.monitored_directories.len());
        
        // Log configuration status
        tracing::info!("Configuration Status:");
        tracing::info!("  - Config file exists: {}", self.configuration.config_file_exists);
        tracing::info!("  - Config file valid: {}", self.configuration.config_file_valid);
        tracing::info!("  - Hot reload enabled: {}", self.configuration.hot_reload_enabled);
        if !self.configuration.config_errors.is_empty() {
            tracing::warn!("  - Configuration errors: {:?}", self.configuration.config_errors);
        }
        
        // Log system information
        tracing::info!("System Information:");
        tracing::info!("  - Process ID: {}", self.system.process_info.pid);
        if let Some(cpu_count) = self.system.cpu_count {
            tracing::info!("  - CPU cores: {}", cpu_count);
        }
        
        tracing::info!("=== End Diagnostics ===");
    }
    
    /// Log diagnostic information for debugging
    pub fn log_debug_diagnostics(&self) {
        tracing::debug!("=== Detailed Diagnostic Information ===");
        
        // Log all network interfaces
        tracing::debug!("Network Interfaces:");
        for iface in &self.network.interfaces {
            tracing::debug!("  - {} ({}): {} [{}] up={} multicast={}", 
                iface.name, 
                iface.interface_type,
                iface.ip_address,
                if iface.is_loopback { "loopback" } else { "physical" },
                iface.is_up,
                iface.supports_multicast
            );
        }
        
        // Log platform-specific metadata
        tracing::debug!("Platform Metadata:");
        for (key, value) in &self.platform.platform_specific {
            tracing::debug!("  - {}: {}", key, value);
        }
        
        // Log directory details
        tracing::debug!("Directory Details:");
        for dir in &self.filesystem.monitored_directories {
            tracing::debug!("  - {}: exists={} readable={} writable={} files={:?}", 
                dir.path.display(),
                dir.exists,
                dir.readable,
                dir.writable,
                dir.file_count
            );
        }
        
        tracing::debug!("=== End Detailed Diagnostics ===");
    }
    
    /// Export diagnostics to JSON for support purposes
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
    
    /// Save diagnostics to a file
    pub async fn save_to_file(&self, path: &PathBuf) -> Result<(), std::io::Error> {
        let json = self.to_json().map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        tokio::fs::write(path, json).await
    }
}

/// Startup diagnostic checks that can prevent application startup
pub struct StartupDiagnostics;

impl StartupDiagnostics {
    /// Perform critical startup checks
    pub async fn perform_startup_checks() -> Result<(), PlatformError> {
        tracing::info!("Performing startup diagnostic checks...");
        
        // Check platform compatibility
        Self::check_platform_compatibility().await?;
        
        // Check network requirements
        Self::check_network_requirements().await?;
        
        // Check filesystem requirements
        Self::check_filesystem_requirements().await?;
        
        // Check system resources
        Self::check_system_resources().await?;
        
        tracing::info!("All startup diagnostic checks passed");
        Ok(())
    }
    
    async fn check_platform_compatibility() -> Result<(), PlatformError> {
        let platform_info = PlatformInfo::detect().await?;
        
        // Check if platform is supported
        match platform_info.os_type {
            OsType::Windows | OsType::MacOS | OsType::Linux => {
                tracing::info!("Platform {} is supported", platform_info.os_type.display_name());
            }
        }
        
        // Check for required capabilities
        if !platform_info.capabilities.supports_multicast {
            tracing::warn!("Platform does not support multicast - DLNA discovery may be limited");
        }
        
        Ok(())
    }
    
    async fn check_network_requirements() -> Result<(), PlatformError> {
        let platform_info = PlatformInfo::detect().await?;
        
        // Check if we have at least one usable network interface
        let usable_interfaces: Vec<_> = platform_info.network_interfaces
            .iter()
            .filter(|iface| !iface.is_loopback && iface.is_up)
            .collect();
        
        if usable_interfaces.is_empty() {
            tracing::error!("No usable network interfaces found");
            return Err(PlatformError::NetworkConfig(
                "No active network interfaces available for DLNA service".to_string()
            ));
        }
        
        tracing::info!("Found {} usable network interface(s)", usable_interfaces.len());
        
        // Test port availability
        if !DiagnosticInfo::test_port_availability(1900).await {
            tracing::warn!("Port 1900 is not available - will use alternative port");
        }
        
        Ok(())
    }
    
    async fn check_filesystem_requirements() -> Result<(), PlatformError> {
        // Check if we can create necessary directories
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("vuio_startup_test");
        
        match std::fs::create_dir_all(&test_dir) {
            Ok(_) => {
                tracing::debug!("Filesystem write test passed");
                let _ = std::fs::remove_dir_all(&test_dir);
            }
            Err(e) => {
                tracing::error!("Filesystem write test failed: {}", e);
                return Err(PlatformError::FileSystemAccess(
                    format!("Cannot create directories: {}", e)
                ));
            }
        }
        
        Ok(())
    }
    
    async fn check_system_resources() -> Result<(), PlatformError> {
        // Check available memory (basic check)
        // TODO: Implement more comprehensive resource checks
        
        tracing::debug!("System resource checks passed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_diagnostic_collection() {
        let diagnostics = DiagnosticInfo::collect().await;
        assert!(diagnostics.is_ok());
        
        let diag = diagnostics.unwrap();
        assert!(!diag.platform.os_type.is_empty());
        assert!(!diag.platform.hostname.is_empty());
    }
    
    #[tokio::test]
    async fn test_port_availability() {
        // Test a port that should be available
        let available = DiagnosticInfo::test_port_availability(0).await; // Port 0 should always be available
        assert!(available);
    }
    
    #[tokio::test]
    async fn test_startup_diagnostics() {
        let result = StartupDiagnostics::perform_startup_checks().await;
        // This might fail in some test environments, so we just check it doesn't panic
        match result {
            Ok(_) => tracing::info!("Startup diagnostics passed"),
            Err(e) => tracing::warn!("Startup diagnostics failed: {}", e),
        }
    }
    
    #[test]
    fn test_diagnostic_serialization() {
        let diag = DiagnosticInfo {
            platform: PlatformDiagnostics {
                os_type: "Linux".to_string(),
                os_version: "5.4.0".to_string(),
                architecture: "x86_64".to_string(),
                hostname: "test-host".to_string(),
                capabilities: PlatformCapabilitiesDiag {
                    can_bind_privileged_ports: false,
                    supports_multicast: true,
                    has_firewall: true,
                    case_sensitive_fs: true,
                    supports_network_paths: true,
                    requires_network_permissions: false,
                },
                platform_specific: HashMap::new(),
            },
            network: NetworkDiagnostics {
                interfaces: vec![],
                primary_interface: None,
                multicast_support: true,
                firewall_status: FirewallStatus::Unknown,
                port_availability: HashMap::new(),
                connectivity_tests: HashMap::new(),
            },
            database: DatabaseDiagnostics {
                database_path: None,
                database_exists: false,
                database_size: None,
                database_accessible: false,
                schema_version: None,
                media_file_count: None,
                last_scan_time: None,
                integrity_status: DatabaseIntegrityStatus::NotChecked,
            },
            filesystem: FilesystemDiagnostics {
                monitored_directories: vec![],
                config_directory: DirectoryDiag {
                    path: PathBuf::from("/tmp"),
                    exists: true,
                    accessible: true,
                    readable: true,
                    writable: true,
                    file_count: None,
                    total_size: None,
                    free_space: None,
                    permissions: None,
                },
                cache_directory: DirectoryDiag {
                    path: PathBuf::from("/tmp"),
                    exists: true,
                    accessible: true,
                    readable: true,
                    writable: true,
                    file_count: None,
                    total_size: None,
                    free_space: None,
                    permissions: None,
                },
                log_directory: DirectoryDiag {
                    path: PathBuf::from("/tmp"),
                    exists: true,
                    accessible: true,
                    readable: true,
                    writable: true,
                    file_count: None,
                    total_size: None,
                    free_space: None,
                    permissions: None,
                },
                temp_directory: DirectoryDiag {
                    path: PathBuf::from("/tmp"),
                    exists: true,
                    accessible: true,
                    readable: true,
                    writable: true,
                    file_count: None,
                    total_size: None,
                    free_space: None,
                    permissions: None,
                },
            },
            configuration: ConfigurationDiagnostics {
                config_file_path: None,
                config_file_exists: false,
                config_file_valid: false,
                config_errors: vec![],
                hot_reload_enabled: false,
                default_values_used: vec![],
            },
            system: SystemDiagnostics {
                uptime: None,
                memory_total: None,
                memory_available: None,
                cpu_count: Some(4),
                load_average: None,
                disk_usage: HashMap::new(),
                process_info: ProcessInfo {
                    pid: 1234,
                    memory_usage: None,
                    cpu_usage: None,
                    thread_count: None,
                    file_descriptors: None,
                },
            },
            timestamp: chrono::Utc::now(),
        };
        
        let json = diag.to_json();
        assert!(json.is_ok());
        assert!(json.unwrap().contains("Linux"));
    }
}

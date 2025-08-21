use std::path::PathBuf;
use thiserror::Error;

/// Comprehensive platform-specific error types with recovery strategies
#[derive(Error, Debug)]
pub enum PlatformError {
    #[error("Windows-specific error: {0}")]
    Windows(#[from] WindowsError),
    
    #[error("macOS-specific error: {0}")]
    MacOS(#[from] MacOSError),
    
    #[error("Linux-specific error: {0}")]
    Linux(#[from] LinuxError),
    
    #[error("Network configuration error: {0}")]
    NetworkConfig(String),
    
    #[error("File system access error: {0}")]
    FileSystemAccess(String),
    
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
    
    #[error("Configuration error: {0}")]
    Configuration(#[from] ConfigurationError),
    
    #[error("Platform detection failed: {0}")]
    DetectionFailed(String),
    
    #[error("Unsupported platform feature: {0}")]
    UnsupportedFeature(String),
}

/// Windows-specific error types with detailed troubleshooting information
#[derive(Error, Debug)]
pub enum WindowsError {
    #[error("Administrator privileges required for port {port}. Try running as administrator or use an alternative port (8080, 8081, 8082)")]
    PrivilegedPortAccess { port: u16 },
    
    #[error("Windows Firewall is blocking multicast traffic. Please add an exception for VuIO in Windows Defender Firewall settings")]
    FirewallBlocked,
    
    #[error("UNC path access denied: {path}. Check network credentials and path accessibility")]
    UncPathDenied { path: String },
    
    #[error("Windows service registration failed: {reason}. Try running with administrator privileges")]
    ServiceRegistrationFailed { reason: String },
    
    #[error("Registry access denied: {key}. Administrator privileges may be required")]
    RegistryAccessDenied { key: String },
    
    #[error("Windows API call failed: {function} returned error code {code}")]
    ApiCallFailed { function: String, code: u32 },
    
    #[error("COM initialization failed: {reason}")]
    ComInitFailed { reason: String },
    
    #[error("WMI query failed: {query}. Error: {reason}")]
    WmiQueryFailed { query: String, reason: String },
}

/// macOS-specific error types with system integration guidance
#[derive(Error, Debug)]
pub enum MacOSError {
    #[error("macOS permission denied for network operations. Grant network permissions in System Preferences > Security & Privacy")]
    NetworkPermissionDenied,
    
    #[error("Keychain access denied: {reason}. Check keychain permissions")]
    KeychainAccessDenied { reason: String },
    
    #[error("macOS Application Firewall is blocking connections. Add VuIO to allowed applications")]
    ApplicationFirewallBlocked,
    
    #[error("Sandbox restriction: {operation}. The application may be running in a restricted environment")]
    SandboxRestriction { operation: String },
    
    #[error("Core Foundation error: {reason}")]
    CoreFoundationError { reason: String },
    
    #[error("System Configuration framework error: {reason}")]
    SystemConfigurationError { reason: String },
    
    #[error("Bonjour service registration failed: {reason}")]
    BonjourRegistrationFailed { reason: String },
    
    #[error("macOS version {version} is not supported. Minimum required version is 10.15")]
    UnsupportedVersion { version: String },
}

/// Linux-specific error types with distribution-specific guidance
#[derive(Error, Debug)]
pub enum LinuxError {
    #[error("Insufficient capabilities for port {port}. Try: sudo setcap 'cap_net_bind_service=+ep' /path/to/vuio")]
    InsufficientCapabilities { port: u16 },
    
    #[error("SELinux policy violation: {context}. Try: sudo setsebool -P httpd_can_network_connect 1")]
    SelinuxViolation { context: String },
    
    #[error("AppArmor restriction: {profile}. Check AppArmor profile configuration")]
    AppArmorRestriction { profile: String },
    
    #[error("Systemd service error: {reason}. Check: systemctl status vuio")]
    SystemdServiceError { reason: String },
    
    #[error("Network namespace error: {namespace}. Check network configuration")]
    NetworkNamespaceError { namespace: String },
    
    #[error("D-Bus connection failed: {reason}. Check D-Bus service status")]
    DBusConnectionFailed { reason: String },
    
    #[error("Firewall blocking connections. Check: sudo ufw status or sudo iptables -L")]
    FirewallBlocked,
    
    #[error("User lacks permission for {operation}. Add user to group: {group}")]
    UserPermissionDenied { operation: String, group: String },
    
    #[error("Distribution {distro} is not fully supported. Some features may not work correctly")]
    UnsupportedDistribution { distro: String },
}

/// Database-related error types with recovery strategies
#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Database connection failed: {reason}. The database file may be corrupted")]
    ConnectionFailed { reason: String },
    
    #[error("Database corruption detected at {location}. Attempting automatic recovery")]
    CorruptionDetected { location: String },
    
    #[error("Database migration failed from version {from} to {to}: {reason}")]
    MigrationFailed { from: u32, to: u32, reason: String },
    
    #[error("Database lock timeout after {seconds}s. Another process may be using the database")]
    LockTimeout { seconds: u64 },
    
    #[error("Insufficient disk space for database operation. Required: {required_mb}MB, Available: {available_mb}MB")]
    InsufficientDiskSpace { required_mb: u64, available_mb: u64 },
    
    #[error("Database backup failed: {reason}")]
    BackupFailed { reason: String },
    
    #[error("Database restore failed: {reason}")]
    RestoreFailed { reason: String },
    
    #[error("Query execution failed: {query}. Error: {reason}")]
    QueryFailed { query: String, reason: String },
}

/// Configuration-related error types with user guidance
#[derive(Error, Debug)]
pub enum ConfigurationError {
    #[error("Configuration file not found at {path}. A default configuration will be created")]
    FileNotFound { path: PathBuf },
    
    #[error("Configuration file parse error at line {line}: {reason}")]
    ParseError { line: usize, reason: String },
    
    #[error("Invalid configuration value for {key}: {value}. Expected: {expected}")]
    InvalidValue { key: String, value: String, expected: String },
    
    #[error("Configuration file permission denied: {path}. Check file permissions")]
    PermissionDenied { path: PathBuf },
    
    #[error("Configuration directory creation failed: {path}. Error: {reason}")]
    DirectoryCreationFailed { path: PathBuf, reason: String },
    
    #[error("Configuration validation failed: {reason}")]
    ValidationFailed { reason: String },
    
    #[error("Configuration file watcher error: {reason}. Hot-reloading disabled")]
    WatcherError { reason: String },
    
    #[error("Configuration backup failed: {reason}")]
    BackupFailed { reason: String },
}

impl PlatformError {
    /// Get user-friendly error message with troubleshooting guidance
    pub fn user_message(&self) -> String {
        match self {
            PlatformError::Windows(err) => format!("Windows Error: {}\n\nTroubleshooting: {}", err, err.troubleshooting_guide()),
            PlatformError::MacOS(err) => format!("macOS Error: {}\n\nTroubleshooting: {}", err, err.troubleshooting_guide()),
            PlatformError::Linux(err) => format!("Linux Error: {}\n\nTroubleshooting: {}", err, err.troubleshooting_guide()),
            PlatformError::Database(err) => format!("Database Error: {}\n\nRecovery: {}", err, err.recovery_strategy()),
            PlatformError::Configuration(err) => format!("Configuration Error: {}\n\nSolution: {}", err, err.solution_guide()),
            _ => self.to_string(),
        }
    }
    
    /// Check if the error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            PlatformError::Windows(err) => err.is_recoverable(),
            PlatformError::MacOS(err) => err.is_recoverable(),
            PlatformError::Linux(err) => err.is_recoverable(),
            PlatformError::Database(err) => err.is_recoverable(),
            PlatformError::Configuration(err) => err.is_recoverable(),
            PlatformError::NetworkConfig(_) => true,
            PlatformError::FileSystemAccess(_) => false,
            PlatformError::DetectionFailed(_) => false,
            PlatformError::UnsupportedFeature(_) => false,
        }
    }
    
    /// Get suggested recovery actions
    pub fn recovery_actions(&self) -> Vec<String> {
        match self {
            PlatformError::Windows(err) => err.recovery_actions(),
            PlatformError::MacOS(err) => err.recovery_actions(),
            PlatformError::Linux(err) => err.recovery_actions(),
            PlatformError::Database(err) => err.recovery_actions(),
            PlatformError::Configuration(err) => err.recovery_actions(),
            PlatformError::NetworkConfig(msg) => vec![
                "Check network interface configuration".to_string(),
                "Verify network connectivity".to_string(),
                format!("Review error details: {}", msg),
            ],
            _ => vec!["Contact support with error details".to_string()],
        }
    }
}

impl WindowsError {
    pub fn troubleshooting_guide(&self) -> &'static str {
        match self {
            WindowsError::PrivilegedPortAccess { .. } => 
                "1. Run as Administrator, or 2. Use alternative ports (8080-8082), or 3. Configure Windows to allow non-privileged port binding",
            WindowsError::FirewallBlocked => 
                "1. Open Windows Defender Firewall, 2. Click 'Allow an app through firewall', 3. Add VuIO to exceptions",
            WindowsError::UncPathDenied { .. } => 
                "1. Check network connectivity, 2. Verify credentials, 3. Test path accessibility in File Explorer",
            WindowsError::ServiceRegistrationFailed { .. } => 
                "1. Run as Administrator, 2. Check Windows Event Log, 3. Verify service name is unique",
            WindowsError::RegistryAccessDenied { .. } => 
                "1. Run as Administrator, 2. Check registry permissions, 3. Use RegEdit to verify access",
            WindowsError::ApiCallFailed { .. } => 
                "1. Check Windows version compatibility, 2. Verify system integrity (sfc /scannow), 3. Update Windows",
            WindowsError::ComInitFailed { .. } => 
                "1. Restart application, 2. Check for COM+ corruption, 3. Re-register COM components",
            WindowsError::WmiQueryFailed { .. } => 
                "1. Restart WMI service, 2. Check WMI repository integrity, 3. Run as Administrator",
        }
    }
    
    pub fn is_recoverable(&self) -> bool {
        match self {
            WindowsError::PrivilegedPortAccess { .. } => true,
            WindowsError::FirewallBlocked => true,
            WindowsError::UncPathDenied { .. } => true,
            WindowsError::ServiceRegistrationFailed { .. } => true,
            WindowsError::RegistryAccessDenied { .. } => false,
            WindowsError::ApiCallFailed { .. } => false,
            WindowsError::ComInitFailed { .. } => true,
            WindowsError::WmiQueryFailed { .. } => true,
        }
    }
    
    pub fn recovery_actions(&self) -> Vec<String> {
        match self {
            WindowsError::PrivilegedPortAccess { port } => vec![
                "Try alternative ports (8080, 8081, 8082)".to_string(),
                format!("Run as Administrator to use port {}", port),
                "Configure port forwarding if needed".to_string(),
            ],
            WindowsError::FirewallBlocked => vec![
                "Add VuIO to Windows Firewall exceptions".to_string(),
                "Temporarily disable firewall for testing".to_string(),
                "Configure specific port exceptions".to_string(),
            ],
            _ => vec!["Follow troubleshooting guide".to_string()],
        }
    }
}

impl MacOSError {
    pub fn troubleshooting_guide(&self) -> &'static str {
        match self {
            MacOSError::NetworkPermissionDenied => 
                "1. Open System Preferences > Security & Privacy, 2. Grant network permissions to VuIO",
            MacOSError::KeychainAccessDenied { .. } => 
                "1. Open Keychain Access, 2. Check application permissions, 3. Reset keychain if necessary",
            MacOSError::ApplicationFirewallBlocked => 
                "1. Open System Preferences > Security & Privacy > Firewall, 2. Add VuIO to allowed apps",
            MacOSError::SandboxRestriction { .. } => 
                "1. Check app entitlements, 2. Disable sandbox if possible, 3. Request additional permissions",
            MacOSError::CoreFoundationError { .. } => 
                "1. Restart application, 2. Check system integrity, 3. Update macOS",
            MacOSError::SystemConfigurationError { .. } => 
                "1. Reset network configuration, 2. Check system preferences, 3. Restart networking",
            MacOSError::BonjourRegistrationFailed { .. } => 
                "1. Restart mDNSResponder service, 2. Check network connectivity, 3. Verify service name uniqueness",
            MacOSError::UnsupportedVersion { .. } => 
                "1. Update macOS to 10.15 or later, 2. Use compatibility mode if available",
        }
    }
    
    pub fn is_recoverable(&self) -> bool {
        match self {
            MacOSError::NetworkPermissionDenied => true,
            MacOSError::KeychainAccessDenied { .. } => true,
            MacOSError::ApplicationFirewallBlocked => true,
            MacOSError::SandboxRestriction { .. } => false,
            MacOSError::CoreFoundationError { .. } => true,
            MacOSError::SystemConfigurationError { .. } => true,
            MacOSError::BonjourRegistrationFailed { .. } => true,
            MacOSError::UnsupportedVersion { .. } => false,
        }
    }
    
    pub fn recovery_actions(&self) -> Vec<String> {
        match self {
            MacOSError::NetworkPermissionDenied => vec![
                "Grant network permissions in System Preferences".to_string(),
                "Restart application after granting permissions".to_string(),
            ],
            MacOSError::ApplicationFirewallBlocked => vec![
                "Add VuIO to firewall exceptions".to_string(),
                "Temporarily disable firewall for testing".to_string(),
            ],
            _ => vec!["Follow troubleshooting guide".to_string()],
        }
    }
}

impl LinuxError {
    pub fn troubleshooting_guide(&self) -> &'static str {
        match self {
            LinuxError::InsufficientCapabilities { .. } => 
                "1. Use setcap to grant network capabilities, 2. Run as root, 3. Use alternative ports",
            LinuxError::SelinuxViolation { .. } => 
                "1. Configure SELinux policy, 2. Use audit2allow to create custom policy, 3. Set permissive mode temporarily",
            LinuxError::AppArmorRestriction { .. } => 
                "1. Modify AppArmor profile, 2. Disable profile temporarily, 3. Add necessary permissions",
            LinuxError::SystemdServiceError { .. } => 
                "1. Check service status with systemctl, 2. Review service logs, 3. Verify service configuration",
            LinuxError::NetworkNamespaceError { .. } => 
                "1. Check network namespace configuration, 2. Verify routing tables, 3. Test connectivity",
            LinuxError::DBusConnectionFailed { .. } => 
                "1. Restart D-Bus service, 2. Check user permissions, 3. Verify D-Bus configuration",
            LinuxError::FirewallBlocked => 
                "1. Configure iptables/ufw rules, 2. Check firewall status, 3. Add port exceptions",
            LinuxError::UserPermissionDenied { .. } => 
                "1. Add user to required group, 2. Check file permissions, 3. Use sudo if necessary",
            LinuxError::UnsupportedDistribution { .. } => 
                "1. Check compatibility documentation, 2. Use generic Linux configuration, 3. Report compatibility issues",
        }
    }
    
    pub fn is_recoverable(&self) -> bool {
        match self {
            LinuxError::InsufficientCapabilities { .. } => true,
            LinuxError::SelinuxViolation { .. } => true,
            LinuxError::AppArmorRestriction { .. } => true,
            LinuxError::SystemdServiceError { .. } => true,
            LinuxError::NetworkNamespaceError { .. } => true,
            LinuxError::DBusConnectionFailed { .. } => true,
            LinuxError::FirewallBlocked => true,
            LinuxError::UserPermissionDenied { .. } => true,
            LinuxError::UnsupportedDistribution { .. } => false,
        }
    }
    
    pub fn recovery_actions(&self) -> Vec<String> {
        match self {
            LinuxError::InsufficientCapabilities { port } => vec![
                format!("sudo setcap 'cap_net_bind_service=+ep' $(which vuio)"),
                format!("Use alternative port instead of {}", port),
                "Run with sudo (not recommended for production)".to_string(),
            ],
            LinuxError::FirewallBlocked => vec![
                "sudo ufw allow 1900/udp".to_string(),
                "sudo ufw allow 8080/tcp".to_string(),
                "Check iptables rules: sudo iptables -L".to_string(),
            ],
            _ => vec!["Follow troubleshooting guide".to_string()],
        }
    }
}

impl DatabaseError {
    pub fn recovery_strategy(&self) -> &'static str {
        match self {
            DatabaseError::ConnectionFailed { .. } => 
                "Automatic database integrity check and repair will be attempted",
            DatabaseError::CorruptionDetected { .. } => 
                "Database will be automatically rebuilt from media scan",
            DatabaseError::MigrationFailed { .. } => 
                "Database will be backed up and recreated with current schema",
            DatabaseError::LockTimeout { .. } => 
                "Retry operation after ensuring no other instances are running",
            DatabaseError::InsufficientDiskSpace { .. } => 
                "Free up disk space or change database location",
            DatabaseError::BackupFailed { .. } => 
                "Continue without backup, but data loss risk exists",
            DatabaseError::RestoreFailed { .. } => 
                "Fallback to fresh database creation",
            DatabaseError::QueryFailed { .. } => 
                "Retry with simplified query or rebuild database",
        }
    }
    
    pub fn is_recoverable(&self) -> bool {
        match self {
            DatabaseError::ConnectionFailed { .. } => true,
            DatabaseError::CorruptionDetected { .. } => true,
            DatabaseError::MigrationFailed { .. } => true,
            DatabaseError::LockTimeout { .. } => true,
            DatabaseError::InsufficientDiskSpace { .. } => false,
            DatabaseError::BackupFailed { .. } => true,
            DatabaseError::RestoreFailed { .. } => true,
            DatabaseError::QueryFailed { .. } => true,
        }
    }
    
    pub fn recovery_actions(&self) -> Vec<String> {
        match self {
            DatabaseError::CorruptionDetected { .. } => vec![
                "Backup existing database".to_string(),
                "Rebuild database from media scan".to_string(),
                "Verify data integrity".to_string(),
            ],
            DatabaseError::LockTimeout { .. } => vec![
                "Check for other running instances".to_string(),
                "Wait and retry operation".to_string(),
                "Restart application if necessary".to_string(),
            ],
            _ => vec!["Follow recovery strategy".to_string()],
        }
    }
}

impl ConfigurationError {
    pub fn solution_guide(&self) -> &'static str {
        match self {
            ConfigurationError::FileNotFound { .. } => 
                "A default configuration file will be created automatically",
            ConfigurationError::ParseError { .. } => 
                "Fix the syntax error in the configuration file or restore from backup",
            ConfigurationError::InvalidValue { .. } => 
                "Correct the invalid value or remove the line to use default",
            ConfigurationError::PermissionDenied { .. } => 
                "Fix file permissions or run with appropriate privileges",
            ConfigurationError::DirectoryCreationFailed { .. } => 
                "Check parent directory permissions and available disk space",
            ConfigurationError::ValidationFailed { .. } => 
                "Review and correct the configuration values",
            ConfigurationError::WatcherError { .. } => 
                "Configuration changes will require application restart",
            ConfigurationError::BackupFailed { .. } => 
                "Continue without backup, but configuration changes may be lost",
        }
    }
    
    pub fn is_recoverable(&self) -> bool {
        match self {
            ConfigurationError::FileNotFound { .. } => true,
            ConfigurationError::ParseError { .. } => true,
            ConfigurationError::InvalidValue { .. } => true,
            ConfigurationError::PermissionDenied { .. } => false,
            ConfigurationError::DirectoryCreationFailed { .. } => false,
            ConfigurationError::ValidationFailed { .. } => true,
            ConfigurationError::WatcherError { .. } => true,
            ConfigurationError::BackupFailed { .. } => true,
        }
    }
    
    pub fn recovery_actions(&self) -> Vec<String> {
        match self {
            ConfigurationError::FileNotFound { path } => vec![
                format!("Create default configuration at {}", path.display()),
                "Review and customize default settings".to_string(),
            ],
            ConfigurationError::ParseError { line, .. } => vec![
                format!("Fix syntax error at line {}", line),
                "Restore from backup if available".to_string(),
                "Reset to default configuration".to_string(),
            ],
            _ => vec!["Follow solution guide".to_string()],
        }
    }
}

/// Result type for platform operations
pub type PlatformResult<T> = Result<T, PlatformError>;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_windows_error_recovery() {
        let error = WindowsError::PrivilegedPortAccess { port: 1900 };
        assert!(error.is_recoverable());
        assert!(!error.recovery_actions().is_empty());
    }
    
    #[test]
    fn test_database_error_recovery() {
        let error = DatabaseError::CorruptionDetected { 
            location: "table_media".to_string() 
        };
        assert!(error.is_recoverable());
        assert_eq!(error.recovery_strategy(), "Database will be automatically rebuilt from media scan");
    }
    
    #[test]
    fn test_platform_error_user_message() {
        let error = PlatformError::Windows(WindowsError::FirewallBlocked);
        let message = error.user_message();
        assert!(message.contains("Windows Error"));
        assert!(message.contains("Troubleshooting"));
    }
    
    #[test]
    fn test_configuration_error_recovery() {
        let error = ConfigurationError::FileNotFound { 
            path: PathBuf::from("/etc/vuio/config.toml") 
        };
        assert!(error.is_recoverable());
        assert!(!error.recovery_actions().is_empty());
    }
}
use anyhow::{anyhow, Context, Result};
use std::{
    net::{IpAddr, SocketAddr},
    path::Path,
};

use super::{AppConfig, MonitoredDirectoryConfig, NetworkInterfaceConfig};
use crate::platform::config::PlatformConfig;

/// Configuration validator for ensuring configuration integrity
pub struct ConfigValidator;

impl ConfigValidator {
    /// Validate the entire application configuration
    pub fn validate(config: &AppConfig) -> Result<()> {
        Self::validate_server_config(config)?;
        Self::validate_network_config(config)?;
        Self::validate_media_config(config)?;
        Self::validate_database_config(config)?;
        Self::validate_platform_specific(config)?;
        Ok(())
    }

    /// Validate server configuration
    fn validate_server_config(config: &AppConfig) -> Result<()> {
        // Validate port range
        if config.server.port == 0 {
            return Err(anyhow!("Server port cannot be 0"));
        }

        // Validate interface address
        if config.server.interface != "0.0.0.0" && config.server.interface != "::" {
            config.server.interface.parse::<IpAddr>()
                .with_context(|| format!("Invalid server interface address: {}", config.server.interface))?;
        }

        // Validate server name
        if config.server.name.trim().is_empty() {
            return Err(anyhow!("Server name cannot be empty"));
        }

        // Validate UUID format (basic check)
        if config.server.uuid.len() != 36 || config.server.uuid.chars().filter(|&c| c == '-').count() != 4 {
            return Err(anyhow!("Invalid UUID format: {}", config.server.uuid));
        }

        Ok(())
    }

    /// Validate network configuration
    fn validate_network_config(config: &AppConfig) -> Result<()> {
        // Validate SSDP port
        if config.network.ssdp_port == 0 {
            return Err(anyhow!("SSDP port cannot be 0"));
        }

        // Check for port conflicts
        if config.network.ssdp_port == config.server.port {
            return Err(anyhow!("SSDP port and server port cannot be the same"));
        }

        // Validate multicast TTL
        if config.network.multicast_ttl == 0 {
            return Err(anyhow!("Multicast TTL must be greater than 0"));
        }

        // Validate announce interval
        if config.network.announce_interval_seconds == 0 {
            return Err(anyhow!("Announce interval must be greater than 0 seconds"));
        }

        // Validate interface selection
        match &config.network.interface_selection {
            NetworkInterfaceConfig::Specific(interface) => {
                if interface.trim().is_empty() {
                    return Err(anyhow!("Specific network interface name cannot be empty"));
                }
            }
            NetworkInterfaceConfig::Auto | NetworkInterfaceConfig::All => {
                // These are always valid
            }
        }

        Ok(())
    }

    /// Validate media configuration
    fn validate_media_config(config: &AppConfig) -> Result<()> {
        // Check that we have at least one monitored directory
        if config.media.directories.is_empty() {
            return Err(anyhow!("At least one monitored directory must be configured"));
        }

        // Validate each monitored directory
        for (index, dir) in config.media.directories.iter().enumerate() {
            Self::validate_monitored_directory(dir, index)?;
        }

        // Validate supported extensions
        if config.media.supported_extensions.is_empty() {
            return Err(anyhow!("At least one supported file extension must be configured"));
        }

        // Check for duplicate extensions
        let mut extensions = config.media.supported_extensions.clone();
        extensions.sort();
        extensions.dedup();
        if extensions.len() != config.media.supported_extensions.len() {
            return Err(anyhow!("Duplicate file extensions found in supported_extensions"));
        }

        Ok(())
    }

    /// Validate a single monitored directory configuration
    fn validate_monitored_directory(dir: &MonitoredDirectoryConfig, index: usize) -> Result<()> {
        let context = format!("monitored directory {}", index);

        // Validate path
        if dir.path.trim().is_empty() {
            return Err(anyhow!("{}: path cannot be empty", context));
        }

        let path = Path::new(&dir.path);
        
        // Check if path exists and is a directory
        if !path.exists() {
            return Err(anyhow!("{}: path does not exist: {}", context, dir.path));
        }

        if !path.is_dir() {
            return Err(anyhow!("{}: path is not a directory: {}", context, dir.path));
        }

        // Platform-specific path validation
        let platform_config = PlatformConfig::for_current_platform();
        let path_buf = std::path::PathBuf::from(&dir.path);
        platform_config.validate_path(&path_buf)
            .with_context(|| format!("{}: path failed platform validation", context))?;

        // Validate extensions if specified
        if let Some(extensions) = &dir.extensions {
            if extensions.is_empty() {
                return Err(anyhow!("{}: extensions list cannot be empty if specified", context));
            }

            for ext in extensions {
                if ext.trim().is_empty() {
                    return Err(anyhow!("{}: extension cannot be empty", context));
                }
                
                // Validate extension format
                if !ext.chars().all(|c| c.is_alphanumeric() || c == '.') {
                    return Err(anyhow!("{}: invalid extension format: {}", context, ext));
                }
            }
        }

        // Validate exclude patterns if specified
        if let Some(patterns) = &dir.exclude_patterns {
            for pattern in patterns {
                if pattern.trim().is_empty() {
                    return Err(anyhow!("{}: exclude pattern cannot be empty", context));
                }
            }
        }

        Ok(())
    }

    /// Validate database configuration
    fn validate_database_config(config: &AppConfig) -> Result<()> {
        // Validate database path if specified
        if let Some(db_path) = &config.database.path {
            if db_path.trim().is_empty() {
                return Err(anyhow!("Database path cannot be empty if specified"));
            }

            let path = Path::new(db_path);
            
            // Check if parent directory exists or can be created
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    // Try to create the parent directory to validate it's writable
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("Cannot create database directory: {}", parent.display()))?;
                }
            }
        }

        Ok(())
    }

    /// Validate platform-specific configuration constraints
    fn validate_platform_specific(config: &AppConfig) -> Result<()> {
        let platform_config = PlatformConfig::for_current_platform();
        
        // Validate monitored directories against platform constraints
        for (index, dir) in config.media.directories.iter().enumerate() {
            let path = std::path::PathBuf::from(&dir.path);
            platform_config.validate_path(&path)
                .with_context(|| format!("Monitored directory {} failed platform validation: {}", index, dir.path))?;
        }
        
        // Validate database directory against platform constraints
        let db_path = config.get_database_path();
        if let Some(parent) = db_path.parent() {
            // Only validate the parent directory, not the database file itself
            platform_config.validate_path(&parent.to_path_buf())
                .with_context(|| format!("Database directory failed platform validation: {}", parent.display()))?;
        }
        
        // Validate server port is reasonable for the platform
        if config.server.port < 1024 && !platform_config.is_case_sensitive() {
            // On Windows, warn about privileged ports
            tracing::warn!(
                "Server port {} may require administrator privileges on this platform",
                config.server.port
            );
        }
        
        // Check if preferred ports are being used
        if !platform_config.preferred_ports.contains(&config.server.port) {
            tracing::info!(
                "Server port {} is not in platform preferred ports: {:?}",
                config.server.port,
                platform_config.preferred_ports
            );
        }
        
        Ok(())
    }

    /// Validate that ports are available (basic check)
    pub fn validate_port_availability(port: u16) -> Result<()> {
        use std::net::TcpListener;
        
        match TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!("Port {} is not available: {}", port, e)),
        }
    }

    /// Validate file system permissions for a directory
    pub fn validate_directory_permissions(path: &Path) -> Result<()> {
        if !path.exists() {
            return Err(anyhow!("Directory does not exist: {}", path.display()));
        }

        if !path.is_dir() {
            return Err(anyhow!("Path is not a directory: {}", path.display()));
        }

        // Test read permissions by trying to read the directory
        match std::fs::read_dir(path) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!("Cannot read directory {}: {}", path.display(), e)),
        }
    }

    /// Comprehensive validation including system checks
    pub fn validate_with_system_checks(config: &AppConfig) -> Result<()> {
        // Basic configuration validation
        Self::validate(config)?;

        // Ensure platform directories exist
        AppConfig::ensure_platform_directories_exist()
            .context("Failed to create platform directories")?;

        // System-level validations
        Self::validate_port_availability(config.server.port)
            .with_context(|| "Server port validation failed")?;

        // Note: We don't validate SSDP port availability as it might be in use by other DLNA servers
        // and we have fallback mechanisms

        // Validate directory permissions
        for dir in &config.media.directories {
            let path = Path::new(&dir.path);
            Self::validate_directory_permissions(path)
                .with_context(|| format!("Directory permission validation failed for: {}", dir.path))?;
        }

        // Validate database directory permissions
        let db_path = config.get_database_path();
        if let Some(parent) = db_path.parent() {
            if parent.exists() {
                Self::validate_directory_permissions(parent)
                    .with_context(|| "Database directory permission validation failed")?;
            }
        }

        // Platform-specific validation with system checks
        config.validate_for_platform()
            .context("Platform-specific validation failed")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use tempfile::TempDir;

    #[test]
    fn test_valid_config() {
        let config = AppConfig::default_for_platform();
        
        // This might fail if the default media directory doesn't exist
        // So we'll create a minimal valid config for testing
        let temp_dir = TempDir::new().unwrap();
        let mut test_config = config;
        test_config.media.directories = vec![
            super::MonitoredDirectoryConfig {
                path: temp_dir.path().to_string_lossy().to_string(),
                recursive: true,
                extensions: None,
                exclude_patterns: None,
            }
        ];
        
        assert!(ConfigValidator::validate(&test_config).is_ok());
    }

    #[test]
    fn test_invalid_server_config() {
        let mut config = AppConfig::default_for_platform();
        
        // Test invalid port
        config.server.port = 0;
        assert!(ConfigValidator::validate(&config).is_err());
        
        // Reset port and test empty name
        config.server.port = 8080;
        config.server.name = "".to_string();
        assert!(ConfigValidator::validate(&config).is_err());
        
        // Reset name and test invalid UUID
        config.server.name = "Test Server".to_string();
        config.server.uuid = "invalid-uuid".to_string();
        assert!(ConfigValidator::validate(&config).is_err());
    }

    #[test]
    fn test_invalid_network_config() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = AppConfig::default_for_platform();
        config.media.directories = vec![
            super::MonitoredDirectoryConfig {
                path: temp_dir.path().to_string_lossy().to_string(),
                recursive: true,
                extensions: None,
                exclude_patterns: None,
            }
        ];
        
        // Test invalid SSDP port
        config.network.ssdp_port = 0;
        assert!(ConfigValidator::validate(&config).is_err());
        
        // Test port conflict
        config.network.ssdp_port = config.server.port;
        assert!(ConfigValidator::validate(&config).is_err());
        
        // Test invalid TTL
        config.network.ssdp_port = 1900;
        config.network.multicast_ttl = 0;
        assert!(ConfigValidator::validate(&config).is_err());
    }

    #[test]
    fn test_invalid_media_config() {
        let mut config = AppConfig::default_for_platform();
        
        // Test empty directories
        config.media.directories = vec![];
        assert!(ConfigValidator::validate(&config).is_err());
        
        // Test empty supported extensions
        config.media.directories = vec![
            super::MonitoredDirectoryConfig {
                path: "/tmp".to_string(),
                recursive: true,
                extensions: None,
                exclude_patterns: None,
            }
        ];
        config.media.supported_extensions = vec![];
        assert!(ConfigValidator::validate(&config).is_err());
    }

    #[test]
    fn test_directory_validation() {
        let temp_dir = TempDir::new().unwrap();
        
        // Valid directory
        let valid_dir = super::MonitoredDirectoryConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            recursive: true,
            extensions: Some(vec!["mp4".to_string()]),
            exclude_patterns: Some(vec!["*.tmp".to_string()]),
        };
        assert!(ConfigValidator::validate_monitored_directory(&valid_dir, 0).is_ok());
        
        // Invalid directory (doesn't exist)
        let invalid_dir = super::MonitoredDirectoryConfig {
            path: "/nonexistent/directory".to_string(),
            recursive: true,
            extensions: None,
            exclude_patterns: None,
        };
        assert!(ConfigValidator::validate_monitored_directory(&invalid_dir, 0).is_err());
        
        // Empty path
        let empty_path_dir = super::MonitoredDirectoryConfig {
            path: "".to_string(),
            recursive: true,
            extensions: None,
            exclude_patterns: None,
        };
        assert!(ConfigValidator::validate_monitored_directory(&empty_path_dir, 0).is_err());
    }
}
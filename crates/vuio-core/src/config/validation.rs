use anyhow::{anyhow, Context, Result};
use std::{
    net::{IpAddr, SocketAddr},
    path::Path,
};

use super::{AppConfig, MonitoredDirectoryConfig, NetworkInterfaceConfig, ValidationMode};
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
        Self::validate_management(config)?;
        Self::validate_platform_specific(config)?;
        Ok(())
    }

    /// Validate management/authentication configuration.
    ///
    /// These values are only consumed once, by `AuthState::load` at startup, which
    /// bails on a CIDR it cannot parse. Rejecting them here instead means a bad value
    /// is refused when it is written, rather than accepted into a running server and
    /// then preventing the next boot.
    pub fn validate_management(config: &AppConfig) -> Result<()> {
        if config.management.session_ttl_hours == 0 {
            return Err(anyhow!("Management session TTL must be greater than 0 hours"));
        }

        for network in &config.management.allowed_networks {
            network
                .parse::<ipnet::IpNet>()
                .with_context(|| format!("Invalid management network CIDR: {network}"))?;
        }

        Ok(())
    }

    /// Validate server configuration
    fn validate_server_config(config: &AppConfig) -> Result<()> {
        // Validate port range
        if config.server.port == 0 {
            return Err(anyhow!("Server port cannot be 0"));
        }

        if config.web_ui.enabled {
            if config.web_ui.port == 0 {
                return Err(anyhow!("Web UI port cannot be 0"));
            }
            // Caught here rather than at bind time, where one of the two
            // listeners would simply lose the race and the operator would be
            // told a port was busy without being told by what.
            if config.web_ui.port == config.server.port {
                return Err(anyhow!(
                    "Web UI port {} is already the server port; they are separate listeners and cannot share one",
                    config.web_ui.port
                ));
            }
        }

        // Validate interface address
        if config.server.interface != "0.0.0.0" && config.server.interface != "::" {
            config.server.interface.parse::<IpAddr>().with_context(|| {
                format!(
                    "Invalid server interface address: {}",
                    config.server.interface
                )
            })?;
        }

        // Validate server name
        if config.server.name.trim().is_empty() {
            return Err(anyhow!("Server name cannot be empty"));
        }

        // Validate UUID format (basic check)
        if config.server.uuid.len() != 36
            || config.server.uuid.chars().filter(|&c| c == '-').count() != 4
        {
            return Err(anyhow!("Invalid UUID format: {}", config.server.uuid));
        }

        // Validate server IP if specified
        if let Some(ip) = &config.server.ip {
            if !ip.is_empty() && ip != "0.0.0.0" {
                // Trim whitespace before parsing
                let trimmed_ip = ip.trim();
                trimmed_ip
                    .parse::<IpAddr>()
                    .with_context(|| format!("Invalid server IP address: {}", ip))?;
            }
        }

        Ok(())
    }

    /// Validate network configuration
    fn validate_network_config(config: &AppConfig) -> Result<()> {
        // Note: SSDP port is hardcoded to 1900 and no longer configurable

        // Validate multicast TTL
        if config.network.multicast_ttl == 0 {
            return Err(anyhow!("Multicast TTL must be greater than 0"));
        }

        // Validate announce interval
        if config.network.announce_interval_seconds == 0 {
            return Err(anyhow!("Announce interval must be greater than 0 seconds"));
        }

        for network in &config.network.upnp_callback_allowed_networks {
            network
                .parse::<ipnet::IpNet>()
                .with_context(|| format!("Invalid UPnP callback network CIDR: {network}"))?;
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
            return Err(anyhow!(
                "At least one monitored directory must be configured"
            ));
        }

        // Validate each monitored directory (strict mode)
        for (index, dir) in config.media.directories.iter().enumerate() {
            Self::validate_monitored_directory_strict(dir, index)?;
        }

        // Check for duplicate extensions
        let mut extensions = config.media.supported_extensions.clone();
        extensions.sort();
        extensions.dedup();
        if extensions.len() != config.media.supported_extensions.len() {
            return Err(anyhow!(
                "Duplicate file extensions found in supported_extensions"
            ));
        }

        Ok(())
    }

    /// Validate a single monitored directory configuration (strict mode - ignores validation_mode)
    fn validate_monitored_directory_strict(
        dir: &MonitoredDirectoryConfig,
        index: usize,
    ) -> Result<()> {
        let context = format!("monitored directory {}", index);

        // Validate path
        if dir.path.trim().is_empty() {
            return Err(anyhow!("{}: path cannot be empty", context));
        }

        let path = Path::new(&dir.path);

        // Always perform strict validation regardless of validation_mode
        if !path.exists() {
            return Err(anyhow!("{}: path does not exist: {}", context, dir.path));
        }

        if !path.is_dir() {
            return Err(anyhow!(
                "{}: path is not a directory: {}",
                context,
                dir.path
            ));
        }

        // Platform-specific path validation
        let platform_config = PlatformConfig::for_current_platform();
        let path_buf = std::path::PathBuf::from(&dir.path);
        platform_config
            .validate_path(&path_buf)
            .with_context(|| format!("{}: path failed platform validation", context))?;

        // Validate extensions if specified
        if let Some(extensions) = &dir.extensions {
            if extensions.is_empty() {
                return Err(anyhow!(
                    "{}: extensions list cannot be empty if specified",
                    context
                ));
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

    /// Validate a single monitored directory configuration (respects validation_mode)
    fn validate_monitored_directory(dir: &MonitoredDirectoryConfig, index: usize) -> Result<()> {
        let context = format!("monitored directory {}", index);

        // Validate path
        if dir.path.trim().is_empty() {
            return Err(anyhow!("{}: path cannot be empty", context));
        }

        let path = Path::new(&dir.path);

        // Handle validation based on validation mode
        match dir.validation_mode {
            ValidationMode::Skip => {
                tracing::debug!("{}: validation skipped", context);
                // Skip all path validation for this directory
            }
            ValidationMode::Warn => {
                // Check if path exists and is a directory, but only warn
                if !path.exists() {
                    tracing::warn!(
                        "{}: path does not exist: {} (continuing startup)",
                        context,
                        dir.path
                    );
                } else if !path.is_dir() {
                    tracing::warn!(
                        "{}: path is not a directory: {} (continuing startup)",
                        context,
                        dir.path
                    );
                } else {
                    // Platform-specific path validation - warn on failure
                    let platform_config = PlatformConfig::for_current_platform();
                    let path_buf = std::path::PathBuf::from(&dir.path);
                    if let Err(e) = platform_config.validate_path(&path_buf) {
                        tracing::warn!(
                            "{}: path failed platform validation: {} (continuing startup)",
                            context,
                            e
                        );
                    }
                }
            }
            ValidationMode::Strict => {
                // Original strict validation behavior
                if !path.exists() {
                    return Err(anyhow!("{}: path does not exist: {}", context, dir.path));
                }

                if !path.is_dir() {
                    return Err(anyhow!(
                        "{}: path is not a directory: {}",
                        context,
                        dir.path
                    ));
                }

                // Platform-specific path validation
                let platform_config = PlatformConfig::for_current_platform();
                let path_buf = std::path::PathBuf::from(&dir.path);
                platform_config
                    .validate_path(&path_buf)
                    .with_context(|| format!("{}: path failed platform validation", context))?;
            }
        }

        // Validate extensions if specified
        if let Some(extensions) = &dir.extensions {
            if extensions.is_empty() {
                return Err(anyhow!(
                    "{}: extensions list cannot be empty if specified",
                    context
                ));
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
                    std::fs::create_dir_all(parent).with_context(|| {
                        format!("Cannot create database directory: {}", parent.display())
                    })?;
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
            platform_config.validate_path(&path).with_context(|| {
                format!(
                    "Monitored directory {} failed platform validation: {}",
                    index, dir.path
                )
            })?;
        }

        // Validate database directory against platform constraints
        let db_path = config.get_database_path();
        if let Some(parent) = db_path.parent() {
            // Only validate the parent directory, not the database file itself
            platform_config.validate_path(parent).with_context(|| {
                format!(
                    "Database directory failed platform validation: {}",
                    parent.display()
                )
            })?;
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
        if !platform_config
            .preferred_ports
            .contains(&config.server.port)
        {
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

    /// Validate configuration with flexible directory validation
    /// This method respects ValidationMode settings and allows startup to continue
    /// when directories are temporarily unavailable
    pub fn validate_flexible(config: &AppConfig) -> Result<()> {
        Self::validate_server_config(config)?;
        Self::validate_network_config(config)?;
        Self::validate_media_config_flexible(config)?;
        Self::validate_database_config(config)?;
        Self::validate_management(config)?;
        Self::validate_platform_specific(config)?;
        Ok(())
    }

    /// Validate media configuration with flexible directory validation
    fn validate_media_config_flexible(config: &AppConfig) -> Result<()> {
        // Check that we have at least one monitored directory
        if config.media.directories.is_empty() {
            return Err(anyhow!(
                "At least one monitored directory must be configured"
            ));
        }

        // Validate each monitored directory with flexible validation
        for (index, dir) in config.media.directories.iter().enumerate() {
            Self::validate_monitored_directory(dir, index)?;
        }

        // Check for duplicate extensions
        let mut extensions = config.media.supported_extensions.clone();
        extensions.sort();
        extensions.dedup();
        if extensions.len() != config.media.supported_extensions.len() {
            return Err(anyhow!(
                "Duplicate file extensions found in supported_extensions"
            ));
        }

        Ok(())
    }

}

#[cfg(test)]
mod tests;

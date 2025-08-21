use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

pub mod validation;
pub mod watcher;

#[cfg(test)]
pub mod example;

use crate::platform::config::PlatformConfig;
use validation::ConfigValidator;

fn default_cleanup_deleted_files() -> bool {
    true
}

/// Main application configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub network: NetworkConfig,
    pub media: MediaConfig,
    pub database: DatabaseConfig,
}

/// Server configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub interface: String,
    pub name: String,
    pub uuid: String,
}

/// Network configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub ssdp_port: u16,
    pub interface_selection: NetworkInterfaceConfig,
    pub multicast_ttl: u8,
    pub announce_interval_seconds: u64,
}

/// Network interface selection configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum NetworkInterfaceConfig {
    Auto,
    Specific(String),
    All,
}

/// Media configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaConfig {
    pub directories: Vec<MonitoredDirectoryConfig>,
    pub scan_on_startup: bool,
    pub watch_for_changes: bool,
    #[serde(default = "default_cleanup_deleted_files")]
    pub cleanup_deleted_files: bool,
    pub supported_extensions: Vec<String>,
}

/// Configuration for a monitored directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoredDirectoryConfig {
    pub path: String,
    pub recursive: bool,
    pub extensions: Option<Vec<String>>,
    pub exclude_patterns: Option<Vec<String>>,
}

/// Database configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: Option<String>,
    pub vacuum_on_startup: bool,
    pub backup_enabled: bool,
}

impl AppConfig {
    /// Create configuration from command line arguments (compatibility with old interface)
    pub async fn from_args() -> Result<(Self, bool, Option<String>)> {
        use clap::Parser;
        
        #[derive(Parser, Debug)]
        #[command(author, version, about, long_about = None)]
        struct Args {
            /// The directory containing media files to serve
            media_dir: Option<String>,

            /// The network port to listen on
            #[arg(short, long)]
            port: Option<u16>,

            /// The friendly name for the DLNA server
            #[arg(short, long, default_value = "VuIO Server")]
            name: String,

            /// Enable debug logging
            #[arg(long)]
            debug: bool,

            /// Path to configuration file
            #[arg(short, long)]
            config: Option<String>,
        }
        
        let args = Args::parse();
        
        // If config file is provided, load from there first
        if let Some(config_path) = &args.config {
            let config_path = PathBuf::from(config_path);
            if !config_path.exists() {
                anyhow::bail!("Configuration file does not exist: {}", config_path.display());
            }
            
            let mut config = Self::load_from_file(&config_path)?;
            
            // Override config file settings with command line arguments
            if let Some(media_dir) = &args.media_dir {
                let media_dir = PathBuf::from(media_dir);
                if !media_dir.exists() {
                    anyhow::bail!("Media directory does not exist: {}", media_dir.display());
                }
                if !media_dir.is_dir() {
                    anyhow::bail!("Media path is not a directory: {}", media_dir.display());
                }
                
                config.media.directories = vec![
                    MonitoredDirectoryConfig {
                        path: media_dir.to_string_lossy().to_string(),
                        recursive: true,
                        extensions: None,
                        exclude_patterns: None,
                    }
                ];
            }
            
            if let Some(port) = args.port {
                config.server.port = port;
            }
            
            if args.name != "VuIO Server" {
                config.server.name = args.name;
            }
            
            return Ok((config, args.debug, args.config));
        }
        
        // If no media directory provided, return error to indicate no args
        let media_dir_str = args.media_dir.ok_or_else(|| {
            anyhow::anyhow!("No media directory provided in command line arguments")
        })?;
        
        let media_dir = PathBuf::from(&media_dir_str);
        
        tracing::info!("Processing command line arguments with media directory: {}", media_dir.display());
        
        // Validate that the directory exists before doing platform validation
        if !media_dir.exists() {
            anyhow::bail!("Media directory does not exist: {}", media_dir.display());
        }
        
        if !media_dir.is_dir() {
            anyhow::bail!("Media path is not a directory: {}", media_dir.display());
        }

        // Now validate the path for platform compatibility
        let platform_config = PlatformConfig::for_current_platform();
        platform_config.validate_path(&media_dir)
            .with_context(|| format!("Invalid media directory for current platform: {}", media_dir.display()))?;

        let mut config = Self::default_for_platform();
        
        // Override defaults with command line arguments
        if let Some(port) = args.port {
            config.server.port = port;
        }
        config.server.name = args.name;
        config.media.directories = vec![
            MonitoredDirectoryConfig {
                path: media_dir.to_string_lossy().to_string(),
                recursive: true,
                extensions: None,
                exclude_patterns: Some(platform_config.get_default_exclude_patterns()),
            }
        ];
        
        tracing::info!("Using command line media directory: {}", media_dir.display());
        
        Ok((config, args.debug, args.config))
    }

    /// Get the primary media directory (for compatibility)
    pub fn get_primary_media_dir(&self) -> PathBuf {
        if let Some(first_dir) = self.media.directories.first() {
            PathBuf::from(&first_dir.path)
        } else {
            let platform_config = PlatformConfig::for_current_platform();
            platform_config.default_media_dir
        }
    }
    /// Load configuration from file or create with defaults
    pub fn load_or_create<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let config_path = config_path.as_ref();
        
        if config_path.exists() {
            let mut config = Self::load_from_file(config_path)?;
            
            // Ensure the loaded configuration uses platform-appropriate defaults for missing values
            config.apply_platform_defaults()?;
            
            // Re-save the configuration if it was updated with platform defaults
            config.save_to_file(config_path)?;
            
            Ok(config)
        } else {
            // Ensure platform directories exist before creating configuration
            AppConfig::ensure_platform_directories_exist()?;
            
            let default_config = Self::default_for_platform();
            default_config.save_to_file(config_path)
                .with_context(|| format!("Failed to create default configuration file at: {}", config_path.display()))?;
            
            tracing::info!("Created default configuration file at: {}", config_path.display());
            Ok(default_config)
        }
    }

    /// Load configuration from a TOML file
    pub fn load_from_file<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let content = std::fs::read_to_string(config_path.as_ref())
            .with_context(|| format!("Failed to read config file: {}", config_path.as_ref().display()))?;
        
        let config: AppConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", config_path.as_ref().display()))?;
        
        // Validate the loaded configuration
        ConfigValidator::validate(&config)?;
        
        Ok(config)
    }

    /// Save configuration to a TOML file with platform-specific comments
    pub fn save_to_file<P: AsRef<Path>>(&self, config_path: P) -> Result<()> {
        let config_path = config_path.as_ref();
        
        // Create parent directories if they don't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
        }
        
        // Generate TOML content with platform-specific template
        let content = self.to_toml_with_platform_comments()
            .context("Failed to serialize configuration to TOML")?;
        
        std::fs::write(config_path, content)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;
        
        Ok(())
    }

    /// Generate TOML content with platform-specific comments and examples
    fn to_toml_with_platform_comments(&self) -> Result<String> {
        let platform_config = PlatformConfig::for_current_platform();
        
        // First generate the standard TOML
        let base_toml = toml::to_string_pretty(self)
            .context("Failed to serialize configuration to TOML")?;
        
        // Add platform-specific header comments
        let mut content = format!(
            "# VuIO Server Configuration\n# Platform: {}\n# Auto-generated configuration with platform-specific defaults\n\n",
            match platform_config.os_type {
                crate::platform::OsType::Windows => "Windows",
                crate::platform::OsType::MacOS => "macOS", 
                crate::platform::OsType::Linux => "Linux",
            }
        );
        
        // Add platform-specific comments before each section
        let lines: Vec<&str> = base_toml.lines().collect();
        
        for line in lines {
            if line.starts_with("[server]") {
                content.push_str("# Server configuration\n");
                content.push_str(&format!("# Recommended ports for this platform: {:?}\n", platform_config.preferred_ports));
            } else if line.starts_with("[network]") {
                content.push_str("\n# Network configuration\n");
                content.push_str("# SSDP is used for DLNA device discovery\n");
            } else if line.starts_with("[media]") {
                content.push_str("\n# Media configuration\n");
                content.push_str(&format!("# Default media directories for this platform: {:?}\n", 
                    platform_config.get_default_media_directories().iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect::<Vec<_>>()));
            } else if line.starts_with("[database]") {
                content.push_str("\n# Database configuration\n");
                content.push_str(&format!("# Platform default database location: {}\n", 
                    platform_config.get_database_path().display()));
            } else if line.starts_with("[[media.directories]]") {
                content.push_str("\n# Monitored media directories\n");
                content.push_str(&format!("# Platform-specific exclude patterns: {:?}\n", 
                    platform_config.get_default_exclude_patterns()));
            }
            
            content.push_str(line);
            content.push('\n');
        }
        
        // Add additional platform-specific guidance at the end
        content.push_str("\n# Platform-specific notes:\n");
        match platform_config.os_type {
            crate::platform::OsType::Windows => {
                content.push_str("# - Ports below 1024 may require administrator privileges\n");
                content.push_str("# - Windows Firewall may block network access\n");
                content.push_str("# - UNC paths (\\\\server\\share) are supported\n");
                content.push_str("# - Consider excluding 'Thumbs.db' and 'desktop.ini' files\n");
                content.push_str(&format!("# - Configuration directory: {}\n", platform_config.config_dir.display()));
                content.push_str(&format!("# - Database directory: {}\n", platform_config.database_dir.display()));
                content.push_str(&format!("# - Log directory: {}\n", platform_config.log_dir.display()));
            }
            crate::platform::OsType::MacOS => {
                content.push_str("# - System may prompt for network access permissions\n");
                content.push_str("# - Ports below 1024 require administrator privileges\n");
                content.push_str("# - Network mounted volumes are supported\n");
                content.push_str("# - Consider excluding '.DS_Store' and '.AppleDouble' files\n");
                content.push_str(&format!("# - Configuration directory: {}\n", platform_config.config_dir.display()));
                content.push_str(&format!("# - Database directory: {}\n", platform_config.database_dir.display()));
                content.push_str(&format!("# - Log directory: {}\n", platform_config.log_dir.display()));
            }
            crate::platform::OsType::Linux => {
                content.push_str("# - Ports below 1024 require root privileges\n");
                content.push_str("# - SELinux/AppArmor policies may affect file access\n");
                content.push_str("# - Mounted filesystems under /media and /mnt are supported\n");
                content.push_str("# - Consider excluding 'lost+found' and '.Trash-*' directories\n");
                content.push_str(&format!("# - Configuration directory: {}\n", platform_config.config_dir.display()));
                content.push_str(&format!("# - Database directory: {}\n", platform_config.database_dir.display()));
                content.push_str(&format!("# - Log directory: {}\n", platform_config.log_dir.display()));
            }
        }
        
        Ok(content)
    }

    /// Create default configuration for the current platform
    pub fn default_for_platform() -> Self {
        let platform_config = PlatformConfig::for_current_platform();
        
        // Get all potential media directories for the platform
        let media_directories = platform_config.get_default_media_directories();
        let monitored_dirs = if media_directories.is_empty() {
            // Fallback to current directory if no platform directories found
            vec![MonitoredDirectoryConfig {
                path: std::env::current_dir()
                    .unwrap_or_else(|_| platform_config.default_media_dir.clone())
                    .to_string_lossy()
                    .to_string(),
                recursive: true,
                extensions: None,
                exclude_patterns: Some(platform_config.get_default_exclude_patterns()),
            }]
        } else {
            // Use the primary media directory (first one) as default
            vec![MonitoredDirectoryConfig {
                path: media_directories[0].to_string_lossy().to_string(),
                recursive: true,
                extensions: None, // Use global supported_extensions
                exclude_patterns: Some(platform_config.get_default_exclude_patterns()),
            }]
        };
        
        Self {
            server: ServerConfig {
                port: platform_config.preferred_ports.first().copied().unwrap_or(8080),
                interface: Self::get_platform_default_interface(&platform_config),
                name: Self::get_platform_server_name(&platform_config),
                uuid: Uuid::new_v4().to_string(),
            },
            network: NetworkConfig {
                ssdp_port: Self::get_platform_default_ssdp_port(&platform_config),
                interface_selection: NetworkInterfaceConfig::Auto,
                multicast_ttl: Self::get_platform_default_multicast_ttl(&platform_config),
                announce_interval_seconds: Self::get_platform_default_announce_interval(&platform_config),
            },
            media: MediaConfig {
                directories: monitored_dirs,
                scan_on_startup: true,
                watch_for_changes: true,
                cleanup_deleted_files: true,
                supported_extensions: platform_config.get_default_media_extensions(),
            },
            database: DatabaseConfig {
                path: Some(platform_config.get_database_path().to_string_lossy().to_string()),
                vacuum_on_startup: false,
                backup_enabled: true,
            },
        }
    }

    /// Get platform-appropriate server name
    fn get_platform_server_name(platform_config: &PlatformConfig) -> String {
        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "Unknown".to_string());
        
        match platform_config.os_type {
            crate::platform::OsType::Windows => format!("VuIO Server ({})", hostname),
            crate::platform::OsType::MacOS => format!("VuIO Server on {}", hostname),
            crate::platform::OsType::Linux => format!("VuIO Server - {}", hostname),
        }
    }

    /// Get platform-appropriate default interface
    fn get_platform_default_interface(platform_config: &PlatformConfig) -> String {
        match platform_config.os_type {
            crate::platform::OsType::Windows => {
                // Windows often has issues with 0.0.0.0, prefer specific binding
                "0.0.0.0".to_string()
            }
            crate::platform::OsType::MacOS => {
                // macOS works well with 0.0.0.0
                "0.0.0.0".to_string()
            }
            crate::platform::OsType::Linux => {
                // Linux works well with 0.0.0.0
                "0.0.0.0".to_string()
            }
        }
    }

    /// Get platform-appropriate default SSDP port
    fn get_platform_default_ssdp_port(platform_config: &PlatformConfig) -> u16 {
        match platform_config.os_type {
            crate::platform::OsType::Windows => {
                // Windows may have issues with port 1900, but it's the DLNA standard
                1900
            }
            crate::platform::OsType::MacOS => {
                // macOS typically works fine with standard SSDP port
                1900
            }
            crate::platform::OsType::Linux => {
                // Linux typically works fine with standard SSDP port
                1900
            }
        }
    }

    /// Get platform-appropriate default multicast TTL
    fn get_platform_default_multicast_ttl(platform_config: &PlatformConfig) -> u8 {
        match platform_config.os_type {
            crate::platform::OsType::Windows => {
                // Windows may need higher TTL for complex network setups
                4
            }
            crate::platform::OsType::MacOS => {
                // macOS typically works well with standard TTL
                4
            }
            crate::platform::OsType::Linux => {
                // Linux typically works well with standard TTL
                4
            }
        }
    }

    /// Get platform-appropriate default announce interval
    fn get_platform_default_announce_interval(platform_config: &PlatformConfig) -> u64 {
        match platform_config.os_type {
            crate::platform::OsType::Windows => {
                // Windows may benefit from more frequent announcements due to firewall issues
                30
            }
            crate::platform::OsType::MacOS => {
                // macOS works well with standard interval
                30
            }
            crate::platform::OsType::Linux => {
                // Linux works well with standard interval
                30
            }
        }
    }

    /// Get the database file path, using platform default if not specified
    pub fn get_database_path(&self) -> PathBuf {
        match &self.database.path {
            Some(path) => PathBuf::from(path),
            None => {
                let platform_config = PlatformConfig::for_current_platform();
                platform_config.get_database_path()
            }
        }
    }

    /// Get all monitored directories as PathBuf objects
    pub fn get_monitored_directories(&self) -> Vec<PathBuf> {
        self.media.directories
            .iter()
            .map(|dir| PathBuf::from(&dir.path))
            .collect()
    }

    /// Get supported file extensions for a specific directory, or global defaults
    pub fn get_extensions_for_directory(&self, dir_path: &Path) -> Vec<String> {
        // Find the directory configuration
        for dir_config in &self.media.directories {
            if PathBuf::from(&dir_config.path) == dir_path {
                if let Some(extensions) = &dir_config.extensions {
                    return extensions.clone();
                }
                break;
            }
        }
        
        // Fall back to global supported extensions
        self.media.supported_extensions.clone()
    }

    /// Get exclude patterns for a specific directory
    pub fn get_exclude_patterns_for_directory(&self, dir_path: &Path) -> Vec<String> {
        for dir_config in &self.media.directories {
            if PathBuf::from(&dir_config.path) == dir_path {
                return dir_config.exclude_patterns.clone().unwrap_or_default();
            }
        }
        
        Vec::new()
    }

    /// Check if a file should be excluded based on patterns
    pub fn should_exclude_file(&self, file_path: &Path, dir_path: &Path) -> bool {
        let patterns = self.get_exclude_patterns_for_directory(dir_path);
        let file_name = file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        
        for pattern in &patterns {
            if Self::matches_pattern(file_name, pattern) {
                return true;
            }
        }
        
        false
    }

    /// Simple pattern matching for exclude patterns
    fn matches_pattern(filename: &str, pattern: &str) -> bool {
        if pattern.starts_with("*.") {
            // Extension pattern like "*.tmp"
            let ext = &pattern[2..];
            filename.ends_with(&format!(".{}", ext))
        } else if pattern == ".*" {
            // Hidden file pattern - matches files starting with dot
            filename.starts_with('.')
        } else {
            // Exact match
            filename == pattern
        }
    }

    /// Get the platform configuration file path
    pub fn get_platform_config_file_path() -> PathBuf {
        let platform_config = PlatformConfig::for_current_platform();
        platform_config.get_config_file_path()
    }

    /// Create a configuration file with platform-specific template and examples
    pub fn create_platform_template<P: AsRef<Path>>(config_path: P) -> Result<()> {
        let config_path = config_path.as_ref();
        
        // Don't overwrite existing configuration
        if config_path.exists() {
            return Err(anyhow::anyhow!(
                "Configuration file already exists: {}",
                config_path.display()
            ));
        }
        
        // Ensure platform directories exist
        Self::ensure_platform_directories_exist()?;
        
        // Create default configuration with platform-specific settings
        let config = Self::default_for_platform();
        
        // Validate the configuration before saving
        config.validate_for_platform()
            .context("Generated platform configuration is invalid")?;
        
        // Save with platform-specific comments and examples
        config.save_to_file(config_path)
            .with_context(|| format!("Failed to create configuration template at: {}", config_path.display()))?;
        
        tracing::info!(
            "Created platform-specific configuration template at: {}",
            config_path.display()
        );
        
        Ok(())
    }

    /// Get all potential media directories for the current platform
    pub fn get_platform_media_directories() -> Vec<PathBuf> {
        let platform_config = PlatformConfig::for_current_platform();
        platform_config.get_default_media_directories()
    }

    /// Apply platform-specific defaults to missing or invalid configuration values
    pub fn apply_platform_defaults(&mut self) -> Result<()> {
        let platform_config = PlatformConfig::for_current_platform();
        
        // Update database path if not set or invalid
        if self.database.path.is_none() {
            self.database.path = Some(platform_config.get_database_path().to_string_lossy().to_string());
        }
        
        // Ensure media directories have platform-appropriate exclude patterns
        for dir_config in &mut self.media.directories {
            if dir_config.exclude_patterns.is_none() || dir_config.exclude_patterns.as_ref().unwrap().is_empty() {
                dir_config.exclude_patterns = Some(platform_config.get_default_exclude_patterns());
            } else {
                // Merge with platform defaults if not already present
                let mut patterns = dir_config.exclude_patterns.clone().unwrap_or_default();
                let platform_patterns = platform_config.get_default_exclude_patterns();
                
                for platform_pattern in platform_patterns {
                    if !patterns.contains(&platform_pattern) {
                        patterns.push(platform_pattern);
                    }
                }
                
                dir_config.exclude_patterns = Some(patterns);
            }
        }
        
        // Update supported extensions if empty
        if self.media.supported_extensions.is_empty() {
            self.media.supported_extensions = platform_config.get_default_media_extensions();
        } else {
            // Merge with platform-specific extensions if not already present
            let platform_extensions = platform_config.get_default_media_extensions();
            for ext in platform_extensions {
                if !self.media.supported_extensions.contains(&ext) {
                    self.media.supported_extensions.push(ext);
                }
            }
        }
        
        // Update server interface if it's empty or default
        if self.server.interface.is_empty() {
            self.server.interface = Self::get_platform_default_interface(&platform_config);
        }
        
        // Update network settings with platform defaults if they're at default values
        if self.network.multicast_ttl == 4 {
            self.network.multicast_ttl = Self::get_platform_default_multicast_ttl(&platform_config);
        }
        
        if self.network.announce_interval_seconds == 30 {
            self.network.announce_interval_seconds = Self::get_platform_default_announce_interval(&platform_config);
        }
        
        // Update server name if it's generic
        if self.server.name == "VuIO Server" || self.server.name.is_empty() {
            self.server.name = Self::get_platform_server_name(&platform_config);
        }
        
        // Validate and potentially update server port
        if !platform_config.preferred_ports.contains(&self.server.port) {
            tracing::warn!(
                "Server port {} is not in platform preferred ports, considering fallback",
                self.server.port
            );
            
            // Don't automatically change the port, but log the recommendation
            tracing::info!(
                "Recommended ports for this platform: {:?}",
                platform_config.preferred_ports
            );
        }
        
        // Ensure all platform directories exist
        platform_config.ensure_directories_exist()
            .context("Failed to create platform directories")?;
        
        Ok(())
    }

    /// Validate configuration against platform-specific constraints
    pub fn validate_for_platform(&self) -> Result<()> {
        let platform_config = PlatformConfig::for_current_platform();
        
        // Validate monitored directories
        for dir_config in &self.media.directories {
            let path = PathBuf::from(&dir_config.path);
            platform_config.validate_path(&path)
                .with_context(|| format!("Invalid media directory: {}", path.display()))?;
        }
        
        // Validate database path - ensure parent directory can be created
        let db_path = self.get_database_path();
        if let Some(parent) = db_path.parent() {
            if !parent.exists() {
                // Try to create the directory
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create database directory: {}", parent.display()))?;
            }
            
            // Verify the directory is writable by attempting to create a test file
            let test_file = parent.join(".write_test");
            match std::fs::write(&test_file, b"test") {
                Ok(_) => {
                    // Clean up test file
                    let _ = std::fs::remove_file(&test_file);
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Database directory is not writable: {} ({})",
                        parent.display(),
                        e
                    ));
                }
            }
        }
        
        // Validate server port is in preferred range
        if !platform_config.preferred_ports.contains(&self.server.port) {
            tracing::warn!(
                "Server port {} is not in platform preferred ports: {:?}",
                self.server.port,
                platform_config.preferred_ports
            );
        }
        
        // Validate network interface configuration
        match &self.network.interface_selection {
            NetworkInterfaceConfig::Specific(interface_name) => {
                if interface_name.is_empty() {
                    return Err(anyhow::anyhow!("Specific network interface name cannot be empty"));
                }
            }
            _ => {} // Auto and All are always valid
        }
        
        // Validate server interface address for platform compatibility
        if !self.server.interface.is_empty() && self.server.interface != "0.0.0.0" && self.server.interface != "::" {
            self.server.interface.parse::<std::net::IpAddr>()
                .with_context(|| format!("Invalid server interface address: {}", self.server.interface))?;
        }
        
        // Platform-specific validations
        match platform_config.os_type {
            crate::platform::OsType::Windows => {
                self.validate_windows_specific(&platform_config)?;
            }
            crate::platform::OsType::MacOS => {
                self.validate_macos_specific(&platform_config)?;
            }
            crate::platform::OsType::Linux => {
                self.validate_linux_specific(&platform_config)?;
            }
        }
        
        Ok(())
    }

    /// Windows-specific configuration validation
    fn validate_windows_specific(&self, _platform_config: &PlatformConfig) -> Result<()> {
        // Check for privileged ports
        if self.network.ssdp_port < 1024 && self.network.ssdp_port != 1900 {
            tracing::warn!(
                "SSDP port {} may require administrator privileges on Windows",
                self.network.ssdp_port
            );
        }
        
        if self.server.port < 1024 {
            tracing::warn!(
                "Server port {} may require administrator privileges on Windows",
                self.server.port
            );
        }
        
        // Validate UNC paths if any
        for dir_config in &self.media.directories {
            if dir_config.path.starts_with("\\\\") {
                tracing::info!("UNC path detected: {}", dir_config.path);
                // UNC paths are supported on Windows, just log for awareness
            }
        }
        
        // Check if database path is on a network drive
        let db_path = self.get_database_path();
        if db_path.to_string_lossy().starts_with("\\\\") {
            tracing::warn!(
                "Database path is on a network drive, this may cause performance issues: {}",
                db_path.display()
            );
        }
        
        // Validate Windows-specific exclude patterns are present
        let has_windows_patterns = self.media.directories.iter().any(|dir| {
            dir.exclude_patterns.as_ref().map_or(false, |patterns| {
                patterns.iter().any(|p| p == "Thumbs.db" || p == "desktop.ini")
            })
        });
        
        if !has_windows_patterns {
            tracing::info!("Consider adding Windows-specific exclude patterns like 'Thumbs.db' and 'desktop.ini'");
        }
        
        Ok(())
    }

    /// macOS-specific configuration validation
    fn validate_macos_specific(&self, _platform_config: &PlatformConfig) -> Result<()> {
        // Check for privileged ports
        if self.server.port < 1024 {
            tracing::warn!(
                "Server port {} may require administrator privileges on macOS",
                self.server.port
            );
        }
        
        if self.network.ssdp_port < 1024 && self.network.ssdp_port != 1900 {
            tracing::warn!(
                "SSDP port {} may require administrator privileges on macOS",
                self.network.ssdp_port
            );
        }
        
        // Check for macOS-specific paths
        for dir_config in &self.media.directories {
            let path = PathBuf::from(&dir_config.path);
            if path.starts_with("/Volumes/") {
                tracing::info!("Network volume detected: {}", dir_config.path);
            }
        }
        
        // Validate macOS-specific exclude patterns are present
        let has_macos_patterns = self.media.directories.iter().any(|dir| {
            dir.exclude_patterns.as_ref().map_or(false, |patterns| {
                patterns.iter().any(|p| p == ".DS_Store" || p == ".AppleDouble")
            })
        });
        
        if !has_macos_patterns {
            tracing::info!("Consider adding macOS-specific exclude patterns like '.DS_Store' and '.AppleDouble'");
        }
        
        Ok(())
    }

    /// Linux-specific configuration validation
    fn validate_linux_specific(&self, _platform_config: &PlatformConfig) -> Result<()> {
        // Check for privileged ports
        if self.server.port < 1024 {
            tracing::warn!(
                "Server port {} may require root privileges on Linux",
                self.server.port
            );
        }
        
        if self.network.ssdp_port < 1024 && self.network.ssdp_port != 1900 {
            tracing::warn!(
                "SSDP port {} may require root privileges on Linux",
                self.network.ssdp_port
            );
        }
        
        // Check for common Linux mount points
        for dir_config in &self.media.directories {
            let path = PathBuf::from(&dir_config.path);
            if path.starts_with("/media/") || path.starts_with("/mnt/") {
                tracing::info!("Mounted filesystem detected: {}", dir_config.path);
            }
        }
        
        // Validate Linux-specific exclude patterns are present
        let has_linux_patterns = self.media.directories.iter().any(|dir| {
            dir.exclude_patterns.as_ref().map_or(false, |patterns| {
                patterns.iter().any(|p| p == "lost+found" || p.starts_with(".Trash-"))
            })
        });
        
        if !has_linux_patterns {
            tracing::info!("Consider adding Linux-specific exclude patterns like 'lost+found' and '.Trash-*'");
        }
        
        Ok(())
    }

    /// Ensure all platform directories exist
    pub fn ensure_platform_directories_exist() -> Result<()> {
        let platform_config = PlatformConfig::for_current_platform();
        platform_config.ensure_directories_exist()
            .context("Failed to create platform directories")?;
        Ok(())
    }

    /// Get platform-specific cache directory
    pub fn get_platform_cache_dir() -> PathBuf {
        let platform_config = PlatformConfig::for_current_platform();
        platform_config.get_cache_dir().clone()
    }

    /// Get platform-specific log file path
    pub fn get_platform_log_file_path() -> PathBuf {
        let platform_config = PlatformConfig::for_current_platform();
        platform_config.get_log_file_path()
    }

    /// Get platform-specific configuration recommendations
    pub fn get_platform_recommendations() -> Vec<String> {
        let platform_config = PlatformConfig::for_current_platform();
        let mut recommendations = Vec::new();
        
        match platform_config.os_type {
            crate::platform::OsType::Windows => {
                recommendations.push("Use ports 8080-8082 to avoid administrator privilege requirements".to_string());
                recommendations.push("Configure Windows Firewall to allow VuIO Server".to_string());
                recommendations.push("UNC paths (\\\\server\\share) are supported for network drives".to_string());
                recommendations.push("Exclude Windows system files: Thumbs.db, desktop.ini".to_string());
                recommendations.push("Consider using Windows Service for automatic startup".to_string());
            }
            crate::platform::OsType::MacOS => {
                recommendations.push("Grant network access permissions when prompted by macOS".to_string());
                recommendations.push("Use ports 8080-8082 to avoid administrator privilege requirements".to_string());
                recommendations.push("Network mounted volumes under /Volumes are supported".to_string());
                recommendations.push("Exclude macOS system files: .DS_Store, .AppleDouble".to_string());
                recommendations.push("Consider using launchd for automatic startup".to_string());
            }
            crate::platform::OsType::Linux => {
                recommendations.push("Use ports 8080-8082 to avoid root privilege requirements".to_string());
                recommendations.push("Configure SELinux/AppArmor policies if file access is denied".to_string());
                recommendations.push("Mounted filesystems under /media and /mnt are supported".to_string());
                recommendations.push("Exclude Linux system directories: lost+found, .Trash-*".to_string());
                recommendations.push("Consider using systemd for automatic startup".to_string());
            }
        }
        
        recommendations.push(format!(
            "Recommended media directories: {:?}",
            platform_config.get_default_media_directories()
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>()
        ));
        
        recommendations.push(format!(
            "Configuration will be stored in: {}",
            platform_config.get_config_file_path().display()
        ));
        
        recommendations.push(format!(
            "Database will be stored in: {}",
            platform_config.get_database_path().display()
        ));
        
        recommendations
    }

    /// Check if the current configuration follows platform best practices
    pub fn check_platform_best_practices(&self) -> Vec<String> {
        let platform_config = PlatformConfig::for_current_platform();
        let mut issues = Vec::new();
        
        // Check port usage
        if !platform_config.preferred_ports.contains(&self.server.port) {
            issues.push(format!(
                "Server port {} is not in recommended ports: {:?}",
                self.server.port,
                platform_config.preferred_ports
            ));
        }
        
        // Check exclude patterns
        for (index, dir_config) in self.media.directories.iter().enumerate() {
            let platform_patterns = platform_config.get_default_exclude_patterns();
            let empty_patterns = Vec::new();
            let current_patterns = dir_config.exclude_patterns.as_ref().unwrap_or(&empty_patterns);
            
            for platform_pattern in &platform_patterns {
                if !current_patterns.contains(platform_pattern) {
                    issues.push(format!(
                        "Directory {} missing recommended exclude pattern: {}",
                        index,
                        platform_pattern
                    ));
                }
            }
        }
        
        // Check media extensions
        let platform_extensions = platform_config.get_default_media_extensions();
        let missing_extensions: Vec<_> = platform_extensions
            .iter()
            .filter(|ext| !self.media.supported_extensions.contains(ext))
            .collect();
        
        if !missing_extensions.is_empty() {
            issues.push(format!(
                "Missing recommended media extensions: {:?}",
                missing_extensions
            ));
        }
        
        // Platform-specific checks
        match platform_config.os_type {
            crate::platform::OsType::Windows => {
                if self.server.port < 1024 {
                    issues.push("Server port requires administrator privileges on Windows".to_string());
                }
                if self.network.ssdp_port < 1024 && self.network.ssdp_port != 1900 {
                    issues.push("SSDP port requires administrator privileges on Windows".to_string());
                }
            }
            crate::platform::OsType::MacOS => {
                if self.server.port < 1024 {
                    issues.push("Server port requires administrator privileges on macOS".to_string());
                }
            }
            crate::platform::OsType::Linux => {
                if self.server.port < 1024 {
                    issues.push("Server port requires root privileges on Linux".to_string());
                }
            }
        }
        
        issues
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::default_for_platform()
    }
}

/// Configuration change event
#[derive(Debug, Clone)]
pub enum ConfigChangeEvent {
    /// Configuration file was modified and reloaded
    Reloaded(AppConfig),
    /// Monitored directories changed
    DirectoriesChanged {
        added: Vec<PathBuf>,
        removed: Vec<PathBuf>,
        modified: Vec<PathBuf>,
    },
    /// Network configuration changed
    NetworkChanged {
        old_interface: NetworkInterfaceConfig,
        new_interface: NetworkInterfaceConfig,
        old_port: u16,
        new_port: u16,
    },
}

/// Configuration manager for handling runtime configuration operations
pub struct ConfigManager {
    config: Arc<RwLock<AppConfig>>,
    config_path: PathBuf,
    change_sender: broadcast::Sender<ConfigChangeEvent>,
    _watcher: Option<notify::RecommendedWatcher>,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let config_path = config_path.as_ref().to_path_buf();
        let config = AppConfig::load_or_create(&config_path)?;
        let (change_sender, _) = broadcast::channel(100);
        
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
            change_sender,
            _watcher: None,
        })
    }

    /// Create a new configuration manager with file watching enabled
    pub async fn new_with_watching<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let config_path = config_path.as_ref().to_path_buf();
        let config = AppConfig::load_or_create(&config_path)?;
        let (change_sender, _) = broadcast::channel(100);
        
        let config_arc = Arc::new(RwLock::new(config));
        let sender_clone = change_sender.clone();
        let path_clone = config_path.clone();
        let config_clone = config_arc.clone();
        
        // Set up file watcher
        let watcher = Self::setup_file_watcher(path_clone, config_clone, sender_clone).await?;
        
        Ok(Self {
            config: config_arc,
            config_path,
            change_sender,
            _watcher: Some(watcher),
        })
    }

    /// Set up file watcher for configuration changes
    async fn setup_file_watcher(
        config_path: PathBuf,
        config: Arc<RwLock<AppConfig>>,
        sender: broadcast::Sender<ConfigChangeEvent>,
    ) -> Result<notify::RecommendedWatcher> {
        use notify::{Event, EventKind, Watcher};
        use tokio::sync::mpsc;
        
        let (tx, mut rx) = mpsc::channel(100);
        
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                let _ = tx.try_send(event);
            }
        })?;
        
        // Watch the config file's parent directory
        if let Some(parent) = config_path.parent() {
            watcher.watch(parent, notify::RecursiveMode::NonRecursive)?;
        }
        
        // Spawn task to handle file events
        tokio::spawn(async move {
            let mut last_reload = std::time::Instant::now();
            const DEBOUNCE_DURATION: Duration = Duration::from_millis(500);
            
            while let Some(event) = rx.recv().await {
                // Check if this event is for our config file
                let is_config_file = event.paths.iter().any(|path| path == &config_path);
                
                if !is_config_file {
                    continue;
                }
                
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        // Debounce rapid file changes
                        let now = std::time::Instant::now();
                        if now.duration_since(last_reload) < DEBOUNCE_DURATION {
                            continue;
                        }
                        last_reload = now;
                        
                        // Attempt to reload configuration
                        match AppConfig::load_from_file(&config_path) {
                            Ok(new_config) => {
                                // Validate the new configuration
                                if let Err(e) = ConfigValidator::validate(&new_config) {
                                    tracing::warn!("Invalid configuration file, ignoring changes: {}", e);
                                    continue;
                                }
                                
                                let old_config = {
                                    let mut config_guard = config.write().await;
                                    let old = config_guard.clone();
                                    *config_guard = new_config.clone();
                                    old
                                };
                                
                                // Send change notifications
                                Self::send_change_notifications(&sender, &old_config, &new_config).await;
                                
                                tracing::info!("Configuration reloaded from file");
                            }
                            Err(e) => {
                                tracing::warn!("Failed to reload configuration: {}", e);
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
        
        Ok(watcher)
    }

    /// Send appropriate change notifications based on configuration differences
    async fn send_change_notifications(
        sender: &broadcast::Sender<ConfigChangeEvent>,
        old_config: &AppConfig,
        new_config: &AppConfig,
    ) {
        // Send general reload event
        let _ = sender.send(ConfigChangeEvent::Reloaded(new_config.clone()));
        
        // Check for directory changes
        let old_dirs: std::collections::HashSet<_> = old_config
            .media
            .directories
            .iter()
            .map(|d| PathBuf::from(&d.path))
            .collect();
        
        let new_dirs: std::collections::HashSet<_> = new_config
            .media
            .directories
            .iter()
            .map(|d| PathBuf::from(&d.path))
            .collect();
        
        let added: Vec<_> = new_dirs.difference(&old_dirs).cloned().collect();
        let removed: Vec<_> = old_dirs.difference(&new_dirs).cloned().collect();
        let modified: Vec<_> = new_dirs.intersection(&old_dirs).cloned().collect();
        
        if !added.is_empty() || !removed.is_empty() || !modified.is_empty() {
            let _ = sender.send(ConfigChangeEvent::DirectoriesChanged {
                added,
                removed,
                modified,
            });
        }
        
        // Check for network changes
        if old_config.network.interface_selection != new_config.network.interface_selection
            || old_config.server.port != new_config.server.port
        {
            let _ = sender.send(ConfigChangeEvent::NetworkChanged {
                old_interface: old_config.network.interface_selection.clone(),
                new_interface: new_config.network.interface_selection.clone(),
                old_port: old_config.server.port,
                new_port: new_config.server.port,
            });
        }
    }

    /// Get the current configuration
    pub async fn get_config(&self) -> AppConfig {
        self.config.read().await.clone()
    }

    /// Update the configuration and save to file
    pub async fn update_config(&self, new_config: AppConfig) -> Result<()> {
        // Validate the new configuration
        ConfigValidator::validate(&new_config)?;
        
        // Save to file
        new_config.save_to_file(&self.config_path)?;
        
        let old_config = {
            let mut config_guard = self.config.write().await;
            let old = config_guard.clone();
            *config_guard = new_config.clone();
            old
        };
        
        // Send change notifications
        Self::send_change_notifications(&self.change_sender, &old_config, &new_config).await;
        
        Ok(())
    }

    /// Reload configuration from file
    pub async fn reload(&self) -> Result<()> {
        let new_config = AppConfig::load_from_file(&self.config_path)?;
        
        let old_config = {
            let mut config_guard = self.config.write().await;
            let old = config_guard.clone();
            *config_guard = new_config.clone();
            old
        };
        
        // Send change notifications
        Self::send_change_notifications(&self.change_sender, &old_config, &new_config).await;
        
        Ok(())
    }

    /// Get the configuration file path
    pub fn get_config_path(&self) -> &Path {
        &self.config_path
    }

    /// Subscribe to configuration change events
    pub fn subscribe_to_changes(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.change_sender.subscribe()
    }

    /// Get a clone of the configuration that can be used across async boundaries
    pub async fn get_config_arc(&self) -> Arc<RwLock<AppConfig>> {
        self.config.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn test_default_config_creation() {
        let config = AppConfig::default_for_platform();
        let platform_config = PlatformConfig::for_current_platform();
        
        // Test that platform defaults are used
        assert_eq!(config.server.port, platform_config.preferred_ports.first().copied().unwrap_or(8080));
        assert_eq!(config.network.ssdp_port, 1900); // SSDP port should always be 1900 for DLNA compatibility
        assert!(config.media.scan_on_startup);
        assert!(config.media.watch_for_changes);
        assert!(!config.media.supported_extensions.is_empty());
    }

    #[test]
    fn test_config_serialization() -> Result<()> {
        let config = AppConfig::default_for_platform();
        
        let toml_str = toml::to_string_pretty(&config)?;
        assert!(toml_str.contains("[server]"));
        assert!(toml_str.contains("[network]"));
        assert!(toml_str.contains("[media]"));
        assert!(toml_str.contains("[database]"));
        
        // Test deserialization
        let parsed_config: AppConfig = toml::from_str(&toml_str)?;
        assert_eq!(config.server.port, parsed_config.server.port);
        assert_eq!(config.network.ssdp_port, parsed_config.network.ssdp_port);
        
        Ok(())
    }

    #[test]
    fn test_config_file_operations() -> Result<()> {
        let temp_file = NamedTempFile::new()?;
        let config_path = temp_file.path().to_path_buf();
        
        // Delete the temp file so we can test creation
        std::fs::remove_file(&config_path).ok();
        
        // Test creating default config
        let config = AppConfig::load_or_create(&config_path)?;
        assert!(config_path.exists());
        
        // Test loading existing config
        let loaded_config = AppConfig::load_from_file(&config_path)?;
        assert_eq!(config.server.port, loaded_config.server.port);
        
        Ok(())
    }

    #[test]
    fn test_exclude_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_path_buf();
        
        let mut config = AppConfig::default_for_platform();
        config.media.directories = vec![
            MonitoredDirectoryConfig {
                path: dir_path.to_string_lossy().to_string(),
                recursive: true,
                extensions: None,
                exclude_patterns: Some(vec![
                    ".*".to_string(),           // Hidden files
                    "Thumbs.db".to_string(),    // Windows thumbnails
                    ".DS_Store".to_string(),    // macOS metadata
                    "*.tmp".to_string(),        // Temporary files
                ]),
            }
        ];
        
        // Test hidden file exclusion
        assert!(config.should_exclude_file(&dir_path.join(".hidden"), &dir_path));
        
        // Test Thumbs.db exclusion
        assert!(config.should_exclude_file(&dir_path.join("Thumbs.db"), &dir_path));
        
        // Test .DS_Store exclusion
        assert!(config.should_exclude_file(&dir_path.join(".DS_Store"), &dir_path));
        
        // Test tmp file exclusion
        assert!(config.should_exclude_file(&dir_path.join("temp.tmp"), &dir_path));
        
        // Test normal file inclusion
        assert!(!config.should_exclude_file(&dir_path.join("movie.mp4"), &dir_path));
    }

    #[tokio::test]
    async fn test_config_manager() -> Result<()> {
        let temp_file = NamedTempFile::new()?;
        let config_path = temp_file.path().to_path_buf();
        
        // Delete the temp file so we can test creation
        std::fs::remove_file(&config_path).ok();
        
        let manager = ConfigManager::new(&config_path)?;
        let _original_port = manager.get_config().await.server.port;
        
        // Update configuration
        let mut new_config = manager.get_config().await;
        new_config.server.port = 9090;
        manager.update_config(new_config).await?;
        
        assert_eq!(manager.get_config().await.server.port, 9090);
        
        // Test reload
        manager.reload().await?;
        assert_eq!(manager.get_config().await.server.port, 9090);
        
        Ok(())
    }

    #[test]
    fn test_platform_defaults_application() -> Result<()> {
        let mut config = AppConfig::default_for_platform();
        
        // Simulate a config with missing platform defaults
        config.database.path = None;
        config.media.supported_extensions.clear();
        
        // Apply platform defaults
        config.apply_platform_defaults()?;
        
        // Verify defaults were applied
        assert!(config.database.path.is_some());
        assert!(!config.media.supported_extensions.is_empty());
        
        Ok(())
    }

    #[test]
    fn test_platform_config_template_creation() -> Result<()> {
        let temp_file = NamedTempFile::new()?;
        let config_path = temp_file.path().to_path_buf();
        
        // Delete the temp file so we can test creation
        std::fs::remove_file(&config_path).ok();
        
        // Create platform template
        AppConfig::create_platform_template(&config_path)?;
        
        // Verify file was created
        assert!(config_path.exists());
        
        // Verify content contains platform-specific information
        let content = std::fs::read_to_string(&config_path)?;
        assert!(content.contains("VuIO Server Configuration"));
        assert!(content.contains("Platform:"));
        assert!(content.contains("Recommended ports"));
        
        // Verify we can load the created configuration
        let loaded_config = AppConfig::load_from_file(&config_path)?;
        assert!(!loaded_config.media.supported_extensions.is_empty());
        
        Ok(())
    }

    #[test]
    fn test_platform_validation() -> Result<()> {
        let config = AppConfig::default_for_platform();
        
        // Should validate successfully with platform defaults
        config.validate_for_platform()?;
        
        Ok(())
    }

    #[test]
    fn test_comprehensive_platform_integration() -> Result<()> {
        let platform_config = PlatformConfig::for_current_platform();
        let config = AppConfig::default_for_platform();
        
        // Test that platform defaults are properly applied
        assert!(platform_config.preferred_ports.contains(&config.server.port));
        assert_eq!(config.network.ssdp_port, 1900); // DLNA standard
        assert!(!config.server.name.is_empty());
        assert!(!config.media.supported_extensions.is_empty());
        
        // Test that platform-specific exclude patterns are included
        for dir_config in &config.media.directories {
            if let Some(patterns) = &dir_config.exclude_patterns {
                let platform_patterns = platform_config.get_default_exclude_patterns();
                // At least some platform patterns should be present
                let has_platform_patterns = platform_patterns.iter()
                    .any(|p| patterns.contains(p));
                assert!(has_platform_patterns, "No platform-specific exclude patterns found");
            }
        }
        
        // Test platform validation
        assert!(config.validate_for_platform().is_ok());
        
        // Test platform recommendations
        let recommendations = AppConfig::get_platform_recommendations();
        assert!(!recommendations.is_empty());
        assert!(recommendations.iter().any(|r| r.contains("port")));
        
        // Test best practices check
        let issues = config.check_platform_best_practices();
        // Issues may or may not exist depending on the platform and configuration
        // But the function should not panic
        
        // Test platform-specific helper methods
        assert!(!AppConfig::get_platform_default_interface(&platform_config).is_empty());
        assert_eq!(AppConfig::get_platform_default_ssdp_port(&platform_config), 1900);
        assert!(AppConfig::get_platform_default_multicast_ttl(&platform_config) > 0);
        assert!(AppConfig::get_platform_default_announce_interval(&platform_config) > 0);
        
        Ok(())
    }

    #[test]
    fn test_enhanced_platform_defaults_application() -> Result<()> {
        let mut config = AppConfig::default_for_platform();
        let platform_config = PlatformConfig::for_current_platform();
        
        // Modify config to remove some platform defaults
        config.media.supported_extensions.clear();
        config.server.interface = String::new();
        config.server.name = String::new();
        for dir_config in &mut config.media.directories {
            dir_config.exclude_patterns = None;
        }
        
        // Apply platform defaults
        assert!(config.apply_platform_defaults().is_ok());
        
        // Verify defaults were applied
        assert!(!config.media.supported_extensions.is_empty());
        assert!(!config.server.interface.is_empty());
        assert!(!config.server.name.is_empty());
        
        for dir_config in &config.media.directories {
            assert!(dir_config.exclude_patterns.is_some());
            let patterns = dir_config.exclude_patterns.as_ref().unwrap();
            assert!(!patterns.is_empty());
            
            // Should contain platform-specific patterns
            let platform_patterns = platform_config.get_default_exclude_patterns();
            let has_platform_patterns = platform_patterns.iter()
                .any(|p| patterns.contains(p));
            assert!(has_platform_patterns);
        }
        
        Ok(())
    }
}
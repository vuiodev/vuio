use anyhow::{Context, Result};
use toml_edit::{DocumentMut, value, Array, Table, Item};
use crate::platform::config::PlatformConfig;
use super::{AppConfig, MonitoredDirectoryConfig, ValidationMode};

/// Robust configuration generator that preserves comments and structure
pub struct ConfigGenerator {
    template_doc: DocumentMut,
}

impl ConfigGenerator {
    /// Create a new ConfigGenerator with the embedded template
    pub fn new() -> Result<Self> {
        let template_content = include_str!("template.toml");
        let template_doc = template_content.parse::<DocumentMut>()
            .context("Failed to parse configuration template")?;
        
        Ok(Self { template_doc })
    }

    /// Generate configuration TOML with preserved comments and structure
    pub fn generate_config(&mut self, config: &AppConfig) -> Result<String> {
        let platform_config = PlatformConfig::for_current_platform();
        
        // Update server section
        self.update_server_config(config)?;
        
        // Update network section
        self.update_network_config(config)?;
        
        // Update media section
        self.update_media_config(config)?;
        
        // Update database section
        self.update_database_config(config)?;
        
        // Replace platform-specific placeholders
        let mut content = self.template_doc.to_string();
        content = self.replace_platform_placeholders(content, &platform_config)?;
        
        Ok(content)
    }

    /// Update server configuration values while preserving structure
    fn update_server_config(&mut self, config: &AppConfig) -> Result<()> {
        let server_table = self.template_doc["server"]
            .as_table_mut()
            .context("Server section not found in template")?;
        
        server_table["port"] = value(config.server.port as i64);
        server_table["interface"] = value(&config.server.interface);
        server_table["name"] = value(&config.server.name);
        server_table["uuid"] = value(&config.server.uuid);
        
        // Handle optional IP field
        if let Some(ip) = &config.server.ip {
            server_table["ip"] = value(ip);
        } else {
            server_table["ip"] = value("");
        }
        
        Ok(())
    }

    /// Update network configuration values while preserving structure
    fn update_network_config(&mut self, config: &AppConfig) -> Result<()> {
        let network_table = self.template_doc["network"]
            .as_table_mut()
            .context("Network section not found in template")?;
        
        // Handle interface_selection enum
        let interface_selection = match &config.network.interface_selection {
            super::NetworkInterfaceConfig::Auto => "Auto",
            super::NetworkInterfaceConfig::All => "All",
            super::NetworkInterfaceConfig::Specific(name) => name,
        };
        network_table["interface_selection"] = value(interface_selection);
        
        network_table["multicast_ttl"] = value(config.network.multicast_ttl as i64);
        network_table["announce_interval_seconds"] = value(config.network.announce_interval_seconds as i64);
        
        Ok(())
    }

    /// Update media configuration values while preserving structure
    fn update_media_config(&mut self, config: &AppConfig) -> Result<()> {
        let media_table = self.template_doc["media"]
            .as_table_mut()
            .context("Media section not found in template")?;
        
        media_table["scan_on_startup"] = value(config.media.scan_on_startup);
        media_table["watch_for_changes"] = value(config.media.watch_for_changes);
        media_table["cleanup_deleted_files"] = value(config.media.cleanup_deleted_files);
        media_table["autoplay_enabled"] = value(config.media.autoplay_enabled);
        
        // Update supported extensions array
        let mut extensions_array = Array::new();
        for ext in &config.media.supported_extensions {
            extensions_array.push(ext);
        }
        media_table["supported_extensions"] = value(extensions_array);
        
        // Clear existing directories from template
        if let Some(media_table) = self.template_doc["media"].as_table_mut() {
            media_table.remove("directories");
        }
        
        // Add directories as array of tables - only use the provided directories, not platform defaults
        for (index, dir_config) in config.media.directories.iter().enumerate() {
            self.add_directory_config(dir_config, index)?;
        }
        
        Ok(())
    }

    /// Add a directory configuration to the media.directories array
    fn add_directory_config(&mut self, dir_config: &MonitoredDirectoryConfig, _index: usize) -> Result<()> {
        let mut dir_table = Table::new();
        
        // Escape backslashes in Windows paths for TOML compatibility
        let escaped_path = dir_config.path.replace("\\", "\\\\");
        dir_table["path"] = value(&escaped_path);
        dir_table["recursive"] = value(dir_config.recursive);
        
        // Handle optional extensions - only set if there are actual extensions
        if let Some(extensions) = &dir_config.extensions {
            if !extensions.is_empty() {
                let mut ext_array = Array::new();
                for ext in extensions {
                    ext_array.push(ext);
                }
                dir_table["extensions"] = value(ext_array);
            } else {
                // Don't set extensions field if empty - let it use global defaults
                dir_table.remove("extensions");
            }
        } else {
            // Don't set extensions field if None - let it use global defaults
            dir_table.remove("extensions");
        }
        
        // Handle optional exclude patterns - only set if there are actual patterns
        if let Some(patterns) = &dir_config.exclude_patterns {
            if !patterns.is_empty() {
                let mut patterns_array = Array::new();
                for pattern in patterns {
                    patterns_array.push(pattern);
                }
                dir_table["exclude_patterns"] = value(patterns_array);
            } else {
                // Don't set exclude_patterns field if empty
                dir_table.remove("exclude_patterns");
            }
        } else {
            // Don't set exclude_patterns field if None
            dir_table.remove("exclude_patterns");
        }
        
        // Handle validation mode
        let validation_mode = match dir_config.validation_mode {
            ValidationMode::Strict => "Strict",
            ValidationMode::Warn => "Warn",
            ValidationMode::Skip => "Skip",
        };
        dir_table["validation_mode"] = value(validation_mode);
        
        // Add to document as array of tables
        if !self.template_doc.contains_key("media") {
            self.template_doc["media"] = Item::Table(Table::new());
        }
        
        let media_table = self.template_doc["media"].as_table_mut()
            .context("Failed to get media table")?;
        
        if !media_table.contains_key("directories") {
            media_table["directories"] = Item::ArrayOfTables(toml_edit::ArrayOfTables::new());
        }
        
        let directories_array = media_table["directories"].as_array_of_tables_mut()
            .context("Failed to get directories array")?;
        
        directories_array.push(dir_table);
        
        Ok(())
    }

    /// Update database configuration values while preserving structure
    fn update_database_config(&mut self, config: &AppConfig) -> Result<()> {
        let database_table = self.template_doc["database"]
            .as_table_mut()
            .context("Database section not found in template")?;
        
        if let Some(path) = &config.database.path {
            // Escape backslashes in Windows paths for TOML compatibility
            let escaped_path = path.replace("\\", "\\\\");
            database_table["path"] = value(&escaped_path);
        } else {
            database_table["path"] = value("");
        }
        
        database_table["vacuum_on_startup"] = value(config.database.vacuum_on_startup);
        database_table["backup_enabled"] = value(config.database.backup_enabled);
        
        Ok(())
    }

    /// Replace platform-specific placeholders in the generated content
    fn replace_platform_placeholders(&self, mut content: String, platform_config: &PlatformConfig) -> Result<String> {
        // Replace platform name
        let platform_name = match platform_config.os_type {
            crate::platform::OsType::Windows => "Windows",
            crate::platform::OsType::MacOS => "macOS",
            crate::platform::OsType::Linux => "Linux",
            crate::platform::OsType::Bsd => "BSD",
        };
        content = content.replace("PLACEHOLDER_PLATFORM", platform_name);
        
        // Replace preferred ports
        let preferred_ports = format!("{:?}", platform_config.preferred_ports);
        content = content.replace("PLACEHOLDER_PREFERRED_PORTS", &preferred_ports);
        
        // Replace default media directories
        let default_media_dirs: Vec<String> = platform_config.get_default_media_directories()
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        let default_media_dirs_str = format!("{:?}", default_media_dirs);
        content = content.replace("PLACEHOLDER_DEFAULT_MEDIA_DIRECTORIES", &default_media_dirs_str);
        
        // Replace default exclude patterns
        let default_exclude_patterns = format!("{:?}", platform_config.get_default_exclude_patterns());
        content = content.replace("PLACEHOLDER_DEFAULT_EXCLUDE_PATTERNS", &default_exclude_patterns);
        
        // Replace default media path (first directory or fallback)
        let default_media_path = platform_config.get_default_media_directories()
            .first()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| platform_config.default_media_dir.to_string_lossy().to_string());
        // Escape backslashes for TOML compatibility
        let escaped_default_media_path = default_media_path.replace("\\", "\\\\");
        content = content.replace("PLACEHOLDER_DEFAULT_MEDIA_PATH", &escaped_default_media_path);
        
        // Replace database path
        let database_path_buf = platform_config.get_database_path();
        let database_path = database_path_buf.to_string_lossy();
        // Escape backslashes for TOML compatibility
        let escaped_database_path = database_path.replace("\\", "\\\\");
        content = content.replace("PLACEHOLDER_DEFAULT_DATABASE_PATH", &escaped_database_path);
        content = content.replace("PLACEHOLDER_DATABASE_PATH", &escaped_database_path);
        
        // Replace platform-specific notes
        let platform_notes = self.get_platform_notes(platform_config);
        content = content.replace("PLACEHOLDER_PLATFORM_NOTES", &platform_notes);
        
        Ok(content)
    }

    /// Generate platform-specific notes section
    fn get_platform_notes(&self, platform_config: &PlatformConfig) -> String {
        let mut notes = String::new();
        
        match platform_config.os_type {
            crate::platform::OsType::Windows => {
                notes.push_str("# - Ports below 1024 may require administrator privileges\n");
                notes.push_str("# - Windows Firewall may block network access\n");
                notes.push_str("# - UNC paths (\\\\\\\\server\\\\share) are supported\n");
                notes.push_str("# - Consider excluding 'Thumbs.db' and 'desktop.ini' files\n");
                notes.push_str(&format!("# - Configuration directory: {}\n", platform_config.config_dir.display().to_string().replace("\\", "\\\\")));
                notes.push_str(&format!("# - Database directory: {}\n", platform_config.database_dir.display().to_string().replace("\\", "\\\\")));
                notes.push_str(&format!("# - Log directory: {}\n", platform_config.log_dir.display().to_string().replace("\\", "\\\\")));
                notes.push_str("# - All directories are relative to executable location");
            }
            crate::platform::OsType::MacOS => {
                notes.push_str("# - System may prompt for network access permissions\n");
                notes.push_str("# - Ports below 1024 require administrator privileges\n");
                notes.push_str("# - Network mounted volumes are supported\n");
                notes.push_str("# - Consider excluding '.DS_Store' and '.AppleDouble' files\n");
                notes.push_str(&format!("# - Configuration directory: {}\n", platform_config.config_dir.display()));
                notes.push_str(&format!("# - Database directory: {}\n", platform_config.database_dir.display()));
                notes.push_str(&format!("# - Log directory: {}\n", platform_config.log_dir.display()));
                notes.push_str("# - All directories are relative to executable location");
            }
            crate::platform::OsType::Linux => {
                notes.push_str("# - Ports below 1024 require root privileges\n");
                notes.push_str("# - SELinux/AppArmor policies may affect file access\n");
                notes.push_str("# - Mounted filesystems under /media and /mnt are supported\n");
                notes.push_str("# - Consider excluding 'lost+found' and '.Trash-*' directories\n");
                notes.push_str(&format!("# - Configuration directory: {}\n", platform_config.config_dir.display()));
                notes.push_str(&format!("# - Database directory: {}\n", platform_config.database_dir.display()));
                notes.push_str(&format!("# - Log directory: {}\n", platform_config.log_dir.display()));
                notes.push_str("# - All directories are relative to executable location");
            }
            crate::platform::OsType::Bsd => {
                notes.push_str("# - Ports below 1024 require root privileges\n");
                notes.push_str("# - pf firewall rules may affect network access\n");
                notes.push_str("# - Mounted filesystems under /mnt are supported\n");
                notes.push_str("# - Consider excluding 'lost+found' directories\n");
                notes.push_str(&format!("# - Configuration directory: {}\n", platform_config.config_dir.display()));
                notes.push_str(&format!("# - Database directory: {}\n", platform_config.database_dir.display()));
                notes.push_str(&format!("# - Log directory: {}\n", platform_config.log_dir.display()));
                notes.push_str("# - All directories are relative to executable location");
            }
        }
        
        notes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, ServerConfig, NetworkConfig, MediaConfig, DatabaseConfig, MonitoredDirectoryConfig, ValidationMode, NetworkInterfaceConfig};
    use uuid::Uuid;

    #[test]
    fn test_config_generator_creates_valid_toml() {
        let mut generator = ConfigGenerator::new().expect("Failed to create generator");
        
        // Create a test configuration
        let config = AppConfig {
            server: ServerConfig {
                port: 9090,
                interface: "127.0.0.1".to_string(),
                name: "Test Server".to_string(),
                uuid: Uuid::new_v4().to_string(),
                ip: Some("192.168.1.100".to_string()),
            },
            network: NetworkConfig {
                interface_selection: NetworkInterfaceConfig::Specific("eth0".to_string()),
                multicast_ttl: 8,
                announce_interval_seconds: 60,
            },
            media: MediaConfig {
                directories: vec![
                    MonitoredDirectoryConfig {
                        path: "/test/media".to_string(),
                        recursive: true,
                        extensions: Some(vec!["mp4".to_string(), "mkv".to_string()]),
                        exclude_patterns: Some(vec!["*.tmp".to_string()]),
                        validation_mode: ValidationMode::Strict,
                    }
                ],
                scan_on_startup: false,
                watch_for_changes: false,
                cleanup_deleted_files: false,
                autoplay_enabled: false,
                supported_extensions: vec!["mp4".to_string(), "avi".to_string()],
            },
            database: DatabaseConfig {
                path: Some("/test/db.sqlite".to_string()),
                vacuum_on_startup: true,
                backup_enabled: false,
            },
        };
        
        // Generate TOML
        let toml_content = generator.generate_config(&config).expect("Failed to generate config");
        
        // Verify the generated TOML contains expected values
        assert!(toml_content.contains("port = 9090"));
        assert!(toml_content.contains("interface = \"127.0.0.1\""));
        assert!(toml_content.contains("name = \"Test Server\""));
        assert!(toml_content.contains("ip = \"192.168.1.100\""));
        assert!(toml_content.contains("interface_selection = \"eth0\""));
        assert!(toml_content.contains("multicast_ttl = 8"));
        assert!(toml_content.contains("announce_interval_seconds = 60"));
        assert!(toml_content.contains("scan_on_startup = false"));
        assert!(toml_content.contains("watch_for_changes = false"));
        assert!(toml_content.contains("cleanup_deleted_files = false"));
        assert!(toml_content.contains("autoplay_enabled = false"));
        assert!(toml_content.contains("path = \"/test/media\""));
        assert!(toml_content.contains("recursive = true"));
        assert!(toml_content.contains("validation_mode = \"Strict\""));
        assert!(toml_content.contains("path = \"/test/db.sqlite\""));
        assert!(toml_content.contains("vacuum_on_startup = true"));
        assert!(toml_content.contains("backup_enabled = false"));
        
        // Verify comments are preserved
        assert!(toml_content.contains("# VuIO Server Configuration"));
        assert!(toml_content.contains("# Server configuration"));
        assert!(toml_content.contains("# Network configuration"));
        assert!(toml_content.contains("# Media configuration"));
        assert!(toml_content.contains("# Database configuration"));
        assert!(toml_content.contains("# Platform-specific notes:"));
        
        // Verify the generated TOML can be parsed back
        let parsed_config: AppConfig = toml::from_str(&toml_content)
            .expect("Generated TOML should be parseable");
        
        // Verify key values match
        assert_eq!(parsed_config.server.port, 9090);
        assert_eq!(parsed_config.server.interface, "127.0.0.1");
        assert_eq!(parsed_config.server.name, "Test Server");
        assert_eq!(parsed_config.server.ip, Some("192.168.1.100".to_string()));
        assert_eq!(parsed_config.network.multicast_ttl, 8);
        assert_eq!(parsed_config.network.announce_interval_seconds, 60);
        assert!(!parsed_config.media.scan_on_startup);
        assert!(!parsed_config.media.watch_for_changes);
        assert!(!parsed_config.media.cleanup_deleted_files);
        assert!(!parsed_config.media.autoplay_enabled);
        assert_eq!(parsed_config.media.directories.len(), 1);
        assert_eq!(parsed_config.media.directories[0].path, "/test/media");
        assert!(parsed_config.media.directories[0].recursive);
        assert_eq!(parsed_config.media.directories[0].validation_mode, ValidationMode::Strict);
        assert_eq!(parsed_config.database.path, Some("/test/db.sqlite".to_string()));
        assert!(parsed_config.database.vacuum_on_startup);
        assert!(!parsed_config.database.backup_enabled);
    }

    #[test]
    fn test_config_generator_handles_empty_optional_fields() {
        let mut generator = ConfigGenerator::new().expect("Failed to create generator");
        
        // Create a minimal configuration with empty optional fields
        let config = AppConfig {
            server: ServerConfig {
                port: 8080,
                interface: "0.0.0.0".to_string(),
                name: "VuIO Server".to_string(),
                uuid: Uuid::new_v4().to_string(),
                ip: None, // Test None case
            },
            network: NetworkConfig {
                interface_selection: NetworkInterfaceConfig::Auto,
                multicast_ttl: 4,
                announce_interval_seconds: 30,
            },
            media: MediaConfig {
                directories: vec![
                    MonitoredDirectoryConfig {
                        path: "/media".to_string(),
                        recursive: true,
                        extensions: None, // Test None case
                        exclude_patterns: None, // Test None case
                        validation_mode: ValidationMode::Warn,
                    }
                ],
                scan_on_startup: true,
                watch_for_changes: true,
                cleanup_deleted_files: true,
                autoplay_enabled: true,
                supported_extensions: vec!["mp4".to_string()],
            },
            database: DatabaseConfig {
                path: None, // Test None case
                vacuum_on_startup: false,
                backup_enabled: true,
            },
        };
        
        // Generate TOML
        let toml_content = generator.generate_config(&config).expect("Failed to generate config");
        
        // Verify empty optional fields are handled correctly
        assert!(toml_content.contains("ip = \"\""));
        assert!(toml_content.contains("interface_selection = \"Auto\""));
        // Extensions and exclude_patterns should not be present when None/empty
        assert!(!toml_content.contains("extensions = []"));
        assert!(!toml_content.contains("exclude_patterns = []"));
        assert!(toml_content.contains("validation_mode = \"Warn\""));
        assert!(toml_content.contains("path = \"")); // Empty path for database
        
        // Verify the generated TOML can be parsed back
        let parsed_config: AppConfig = toml::from_str(&toml_content)
            .expect("Generated TOML should be parseable");
        
        // Verify None values are handled correctly
        assert_eq!(parsed_config.server.ip, Some("".to_string())); // Empty string for None IP
        assert_eq!(parsed_config.media.directories[0].extensions, None); // None for unspecified extensions
        assert_eq!(parsed_config.media.directories[0].exclude_patterns, None); // None for unspecified patterns
    }
}
use super::*;

impl AppConfig {
    /// Parse and validate a complete VuIO TOML document without writing it.
    pub fn from_toml_str(content: &str) -> Result<Self> {
        let config: Self = toml::from_str(content).context("Failed to parse configuration TOML")?;
        ConfigValidator::validate_flexible(&config)?;
        Ok(config)
    }

    /// Check if running in Docker container
    pub fn is_running_in_docker() -> bool {
        // Check for Docker-specific environment variables
        std::env::var("DOCKER_CONTAINER").is_ok()
            || std::env::var("CONTAINER").is_ok()
            || std::path::Path::new("/.dockerenv").exists()
            || std::fs::read_to_string("/proc/1/cgroup")
                .map(|content| content.contains("docker") || content.contains("containerd"))
                .unwrap_or(false)
    }

    /// Create configuration from environment variables (Docker mode)
    pub fn from_env() -> Result<Self> {
        let server = ServerConfig {
            port: std::env::var("VUIO_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .context("Invalid VUIO_PORT")?,
            interface: std::env::var("VUIO_INTERFACE").unwrap_or_else(|_| "0.0.0.0".to_string()),
            name: std::env::var("VUIO_SERVER_NAME")
                .unwrap_or_else(|_| "VuIO DLNA Server".to_string()),
            uuid: std::env::var("VUIO_UUID").unwrap_or_else(|_| Uuid::new_v4().to_string()),
            ip: std::env::var("VUIO_IP").ok(),
        };

        let network = NetworkConfig {
            interface_selection: NetworkInterfaceConfig::Auto,
            multicast_ttl: std::env::var("VUIO_MULTICAST_TTL")
                .unwrap_or_else(|_| "4".to_string())
                .parse()
                .context("Invalid VUIO_MULTICAST_TTL")?,
            announce_interval_seconds: std::env::var("VUIO_ANNOUNCE_INTERVAL")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .context("Invalid VUIO_ANNOUNCE_INTERVAL")?,
            upnp_callback_allowed_networks: std::env::var("VUIO_UPNP_CALLBACK_ALLOWED_NETWORKS")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect(),
        };

        let media_dirs = std::env::var("VUIO_MEDIA_DIRS")
            .unwrap_or_else(|_| "/media".to_string())
            .split(',')
            .map(|path| MonitoredDirectoryConfig {
                path: path.trim().to_string(),
                recursive: true,
                case_sensitive: None,
                extensions: None,
                exclude_patterns: Some(vec![
                    ".*".to_string(),
                    "*.tmp".to_string(),
                    "*.part".to_string(),
                ]),
                validation_mode: ValidationMode::Warn,
            })
            .collect();

        let media = MediaConfig {
            directories: media_dirs,
            scan_on_startup: std::env::var("VUIO_SCAN_ON_STARTUP")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(true),
            watch_for_changes: std::env::var("VUIO_WATCH_CHANGES")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(true),
            cleanup_deleted_files: std::env::var("VUIO_CLEANUP_DELETED")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(true),
            autoplay_enabled: std::env::var("VUIO_AUTOPLAY")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(true),
            scan_playlists: std::env::var("VUIO_SCAN_PLAYLISTS")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(true),
            unavailable_root_grace_hours: std::env::var("VUIO_UNAVAILABLE_ROOT_GRACE_HOURS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or_else(default_unavailable_root_grace_hours),
            supported_extensions: vec![
                "mp4".to_string(),
                "mkv".to_string(),
                "avi".to_string(),
                "mov".to_string(),
                "wmv".to_string(),
                "flv".to_string(),
                "webm".to_string(),
                "m4v".to_string(),
                "3gp".to_string(),
                "mp3".to_string(),
                "flac".to_string(),
                "wav".to_string(),
                "aac".to_string(),
                "ogg".to_string(),
                "wma".to_string(),
                "jpg".to_string(),
                "jpeg".to_string(),
                "png".to_string(),
                "gif".to_string(),
                "bmp".to_string(),
                "webp".to_string(),
                "heif".to_string(),
                "heic".to_string(),
                "avif".to_string(),
            ],
        };

        let database = DatabaseConfig {
            path: Some(
                std::env::var("VUIO_DB_PATH").unwrap_or_else(|_| "/data/vuio.db".to_string()),
            ),
            vacuum_on_startup: std::env::var("VUIO_DB_VACUUM")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false),
            backup_enabled: std::env::var("VUIO_DB_BACKUP")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false),
            redb_cache_mb: std::env::var("VUIO_REDB_CACHE_MB")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or_else(default_redb_cache_mb),
        };

        Ok(AppConfig {
            server,
            network,
            media,
            database,
            management: ManagementConfig {
                enabled: std::env::var("VUIO_MANAGEMENT_ENABLED")
                    .map(|value| value.eq_ignore_ascii_case("true"))
                    .unwrap_or(true),
                token_file: std::env::var("VUIO_ADMIN_TOKEN_FILE").ok(),
                session_ttl_hours: std::env::var("VUIO_ADMIN_SESSION_TTL_HOURS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or_else(default_session_ttl_hours),
                allowed_networks: std::env::var("VUIO_MANAGEMENT_ALLOWED_NETWORKS")
                    .unwrap_or_default()
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
                    .collect(),
            },
        })
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

            Ok(config)
        } else {
            // Ensure platform directories exist before creating configuration
            AppConfig::ensure_platform_directories_exist()?;

            let default_config = Self::default_for_platform();
            default_config.save_to_file(config_path).with_context(|| {
                format!(
                    "Failed to create default configuration file at: {}",
                    config_path.display()
                )
            })?;

            tracing::info!(
                "Created default configuration file at: {}",
                config_path.display()
            );
            Ok(default_config)
        }
    }

    /// Load configuration from a TOML file
    pub fn load_from_file<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let content = std::fs::read_to_string(config_path.as_ref()).with_context(|| {
            format!(
                "Failed to read config file: {}",
                config_path.as_ref().display()
            )
        })?;

        let config: AppConfig = toml::from_str(&content).with_context(|| {
            format!(
                "Failed to parse config file: {}",
                config_path.as_ref().display()
            )
        })?;

        // Validate the loaded configuration with flexible directory validation
        ConfigValidator::validate_flexible(&config)?;

        Ok(config)
    }

    /// Save configuration to a TOML file with platform-specific comments
    pub fn save_to_file<P: AsRef<Path>>(&self, config_path: P) -> Result<()> {
        let config_path = config_path.as_ref();

        // Create parent directories if they don't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        // Generate TOML content with platform-specific template using robust generator
        let mut generator =
            ConfigGenerator::new().context("Failed to create configuration generator")?;
        let content = generator
            .generate_config(self)
            .context("Failed to generate configuration TOML")?;

        std::fs::write(config_path, content)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

        Ok(())
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
                case_sensitive: None,
                extensions: None,
                exclude_patterns: Some(platform_config.get_default_exclude_patterns()),
                validation_mode: ValidationMode::Warn,
            }]
        } else {
            // Use the primary media directory (first one) as default
            vec![MonitoredDirectoryConfig {
                path: media_directories[0].to_string_lossy().to_string(),
                recursive: true,
                case_sensitive: None,
                extensions: None, // Use global supported_extensions
                exclude_patterns: Some(platform_config.get_default_exclude_patterns()),
                validation_mode: ValidationMode::Warn,
            }]
        };

        Self {
            server: ServerConfig {
                port: platform_config
                    .preferred_ports
                    .first()
                    .copied()
                    .unwrap_or(8080),
                interface: Self::get_platform_default_interface(&platform_config),
                name: Self::get_platform_server_name(&platform_config),
                uuid: Uuid::new_v4().to_string(),
                ip: None,
            },
            network: NetworkConfig {
                interface_selection: NetworkInterfaceConfig::Auto,
                multicast_ttl: Self::get_platform_default_multicast_ttl(&platform_config),
                announce_interval_seconds: Self::get_platform_default_announce_interval(
                    &platform_config,
                ),
                upnp_callback_allowed_networks: Vec::new(),
            },
            media: MediaConfig {
                directories: monitored_dirs,
                scan_on_startup: true,
                watch_for_changes: true,
                cleanup_deleted_files: true,
                autoplay_enabled: true,
                scan_playlists: true,
                unavailable_root_grace_hours: default_unavailable_root_grace_hours(),
                supported_extensions: platform_config.get_default_media_extensions(),
            },
            database: DatabaseConfig {
                path: Some(
                    platform_config
                        .get_database_path()
                        .to_string_lossy()
                        .to_string(),
                ),
                vacuum_on_startup: false,
                backup_enabled: false,
                redb_cache_mb: default_redb_cache_mb(),
            },
            management: ManagementConfig::default(),
        }
    }

    /// Get platform-appropriate server name
    pub(super) fn get_platform_server_name(platform_config: &PlatformConfig) -> String {
        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "Unknown".to_string());

        match platform_config.os_type {
            crate::platform::OsType::Windows => format!("VuIO Server ({})", hostname),
            crate::platform::OsType::MacOS => format!("VuIO Server on {}", hostname),
            crate::platform::OsType::Linux => format!("VuIO Server - {}", hostname),
            crate::platform::OsType::Bsd => format!("VuIO Server - {}", hostname),
        }
    }

    /// Get platform-appropriate default interface
    pub(super) fn get_platform_default_interface(_platform_config: &PlatformConfig) -> String {
        "0.0.0.0".to_string()
    }

    /// Get platform-appropriate default multicast TTL
    pub(super) fn get_platform_default_multicast_ttl(_platform_config: &PlatformConfig) -> u8 {
        4
    }

    /// Get platform-appropriate default announce interval
    pub(super) fn get_platform_default_announce_interval(_platform_config: &PlatformConfig) -> u64 {
        30
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
        self.media
            .directories
            .iter()
            .map(|dir| PathBuf::from(&dir.path))
            .collect()
    }

    /// Get supported file extensions for a specific directory, or global defaults
    pub fn get_extensions_for_directory(&self, dir_path: &Path) -> Vec<String> {
        // Find the directory configuration
        for dir_config in &self.media.directories {
            if Path::new(&dir_config.path) == dir_path {
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
            if Path::new(&dir_config.path) == dir_path {
                return dir_config.exclude_patterns.clone().unwrap_or_default();
            }
        }

        Vec::new()
    }

    /// Check if a file should be excluded based on patterns
    pub fn should_exclude_file(&self, file_path: &Path, dir_path: &Path) -> bool {
        let patterns = self.get_exclude_patterns_for_directory(dir_path);
        let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        for pattern in &patterns {
            if Self::matches_pattern(file_name, pattern) {
                return true;
            }
        }

        false
    }

    /// Simple pattern matching for exclude patterns
    fn matches_pattern(filename: &str, pattern: &str) -> bool {
        if let Some(ext) = pattern.strip_prefix("*.") {
            // Extension pattern like "*.tmp"
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
        config
            .validate_for_platform()
            .context("Generated platform configuration is invalid")?;

        // Save with platform-specific comments and examples
        config.save_to_file(config_path).with_context(|| {
            format!(
                "Failed to create configuration template at: {}",
                config_path.display()
            )
        })?;

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
}

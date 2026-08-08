use super::*;

impl AppConfig {
    /// Apply platform-specific defaults to missing or invalid configuration values
    pub fn apply_platform_defaults(&mut self) -> Result<()> {
        let platform_config = PlatformConfig::for_current_platform();

        // Update database path if not set or invalid
        if self.database.path.is_none() {
            self.database.path = Some(
                platform_config
                    .get_database_path()
                    .to_string_lossy()
                    .to_string(),
            );
        }

        // Ensure media directories have platform-appropriate exclude patterns
        for dir_config in &mut self.media.directories {
            if dir_config.exclude_patterns.is_none() {
                dir_config.exclude_patterns = Some(platform_config.get_default_exclude_patterns());
            }
        }

        // Update server interface if it's empty or default
        if self.server.interface.is_empty() {
            self.server.interface = Self::get_platform_default_interface(&platform_config);
        }

        // Update network settings with platform defaults if they're uninitialized/zero
        if self.network.multicast_ttl == 0 {
            self.network.multicast_ttl = Self::get_platform_default_multicast_ttl(&platform_config);
        }

        if self.network.announce_interval_seconds == 0 {
            self.network.announce_interval_seconds =
                Self::get_platform_default_announce_interval(&platform_config);
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
        platform_config
            .ensure_directories_exist()
            .context("Failed to create platform directories")?;

        Ok(())
    }

    /// Validate configuration against platform-specific constraints
    pub fn validate_for_platform(&self) -> Result<()> {
        let platform_config = PlatformConfig::for_current_platform();

        // Validate monitored directories
        for dir_config in &self.media.directories {
            let path = PathBuf::from(&dir_config.path);
            platform_config
                .validate_path(&path)
                .with_context(|| format!("Invalid media directory: {}", path.display()))?;
        }

        // Validate database path - ensure parent directory can be created
        let db_path = self.get_database_path();
        if let Some(parent) = db_path.parent() {
            if !parent.exists() {
                // Try to create the directory
                std::fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create database directory: {}", parent.display())
                })?;
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
        if let NetworkInterfaceConfig::Specific(interface_name) = &self.network.interface_selection
        {
            if interface_name.is_empty() {
                return Err(anyhow::anyhow!(
                    "Specific network interface name cannot be empty"
                ));
            }
        }

        // Validate server interface address for platform compatibility
        if !self.server.interface.is_empty()
            && self.server.interface != "0.0.0.0"
            && self.server.interface != "::"
        {
            self.server
                .interface
                .parse::<std::net::IpAddr>()
                .with_context(|| {
                    format!(
                        "Invalid server interface address: {}",
                        self.server.interface
                    )
                })?;
        }

        anyhow::ensure!(
            self.management.session_ttl_hours > 0,
            "management session TTL must be greater than zero"
        );
        for network in &self.management.allowed_networks {
            network
                .parse::<ipnet::IpNet>()
                .with_context(|| format!("Invalid management allowed network: {network}"))?;
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
            crate::platform::OsType::Bsd => {
                self.validate_bsd_specific(&platform_config)?;
            }
        }

        Ok(())
    }

    /// Windows-specific configuration validation
    fn validate_windows_specific(&self, _platform_config: &PlatformConfig) -> Result<()> {
        // Note: SSDP port is hardcoded to 1900 and may require administrator privileges on Windows

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
            dir.exclude_patterns.as_ref().is_some_and(|patterns| {
                patterns
                    .iter()
                    .any(|p| p == "Thumbs.db" || p == "desktop.ini")
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

        // Note: SSDP port is hardcoded to 1900 and may require administrator privileges on macOS

        // Check for macOS-specific paths
        for dir_config in &self.media.directories {
            let path = PathBuf::from(&dir_config.path);
            if path.starts_with("/Volumes/") {
                tracing::info!("Network volume detected: {}", dir_config.path);
            }
        }

        // Validate macOS-specific exclude patterns are present
        let has_macos_patterns = self.media.directories.iter().any(|dir| {
            dir.exclude_patterns.as_ref().is_some_and(|patterns| {
                patterns
                    .iter()
                    .any(|p| p == ".DS_Store" || p == ".AppleDouble")
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

        // Note: SSDP port is hardcoded to 1900 and may require root privileges on Linux

        // Check for common Linux mount points
        for dir_config in &self.media.directories {
            let path = PathBuf::from(&dir_config.path);
            if path.starts_with("/media/") || path.starts_with("/mnt/") {
                tracing::info!("Mounted filesystem detected: {}", dir_config.path);
            }
        }

        // Validate Linux-specific exclude patterns are present
        let has_linux_patterns = self.media.directories.iter().any(|dir| {
            dir.exclude_patterns.as_ref().is_some_and(|patterns| {
                patterns
                    .iter()
                    .any(|p| p == "lost+found" || p.starts_with(".Trash-"))
            })
        });

        if !has_linux_patterns {
            tracing::info!(
                "Consider adding Linux-specific exclude patterns like 'lost+found' and '.Trash-*'"
            );
        }

        Ok(())
    }

    /// Ensure all platform directories exist
    pub fn ensure_platform_directories_exist() -> Result<()> {
        let platform_config = PlatformConfig::for_current_platform();
        platform_config
            .ensure_directories_exist()
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

    /// BSD-specific configuration validation
    fn validate_bsd_specific(&self, _platform_config: &PlatformConfig) -> Result<()> {
        // Check for privileged ports
        if self.server.port < 1024 {
            tracing::warn!(
                "Server port {} may require root privileges on BSD",
                self.server.port
            );
        }

        // Note: SSDP port is hardcoded to 1900 and may require root privileges on BSD

        // Check for common BSD mount points
        for dir_config in &self.media.directories {
            let path = PathBuf::from(&dir_config.path);
            if path.starts_with("/mnt/") {
                tracing::info!("Mounted filesystem detected: {}", dir_config.path);
            }
        }

        // Validate BSD-specific exclude patterns are present
        let has_bsd_patterns = self.media.directories.iter().any(|dir| {
            dir.exclude_patterns
                .as_ref()
                .is_some_and(|patterns| patterns.iter().any(|p| p == "lost+found"))
        });

        if !has_bsd_patterns {
            tracing::info!("Consider adding BSD-specific exclude patterns like 'lost+found'");
        }

        Ok(())
    }

    /// Get platform-specific configuration recommendations
    pub fn get_platform_recommendations() -> Vec<String> {
        let platform_config = PlatformConfig::for_current_platform();
        let mut recommendations = Vec::new();

        match platform_config.os_type {
            crate::platform::OsType::Windows => {
                recommendations.push(
                    "Use ports 8080-8082 to avoid administrator privilege requirements".to_string(),
                );
                recommendations.push("Configure Windows Firewall to allow VuIO Server".to_string());
                recommendations.push(
                    "UNC paths (\\\\server\\share) are supported for network drives".to_string(),
                );
                recommendations
                    .push("Exclude Windows system files: Thumbs.db, desktop.ini".to_string());
                recommendations
                    .push("Consider using Windows Service for automatic startup".to_string());
            }
            crate::platform::OsType::MacOS => {
                recommendations
                    .push("Grant network access permissions when prompted by macOS".to_string());
                recommendations.push(
                    "Use ports 8080-8082 to avoid administrator privilege requirements".to_string(),
                );
                recommendations
                    .push("Network mounted volumes under /Volumes are supported".to_string());
                recommendations
                    .push("Exclude macOS system files: .DS_Store, .AppleDouble".to_string());
                recommendations.push("Consider using launchd for automatic startup".to_string());
            }
            crate::platform::OsType::Linux => {
                recommendations
                    .push("Use ports 8080-8082 to avoid root privilege requirements".to_string());
                recommendations.push(
                    "Configure SELinux/AppArmor policies if file access is denied".to_string(),
                );
                recommendations
                    .push("Mounted filesystems under /media and /mnt are supported".to_string());
                recommendations
                    .push("Exclude Linux system directories: lost+found, .Trash-*".to_string());
                recommendations.push("Consider using systemd for automatic startup".to_string());
            }
            crate::platform::OsType::Bsd => {
                recommendations
                    .push("Use ports 8080-8082 to avoid root privilege requirements".to_string());
                recommendations
                    .push("Configure pf firewall rules if network access is denied".to_string());
                recommendations.push("Mounted filesystems under /mnt are supported".to_string());
                recommendations.push("Exclude BSD system directories: lost+found".to_string());
                recommendations
                    .push("Consider using rc.d scripts for automatic startup".to_string());
            }
        }

        recommendations.push(format!(
            "Recommended media directories: {:?}",
            platform_config
                .get_default_media_directories()
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
                self.server.port, platform_config.preferred_ports
            ));
        }

        // Check exclude patterns
        for (index, dir_config) in self.media.directories.iter().enumerate() {
            let platform_patterns = platform_config.get_default_exclude_patterns();
            let empty_patterns = Vec::new();
            let current_patterns = dir_config
                .exclude_patterns
                .as_ref()
                .unwrap_or(&empty_patterns);

            for platform_pattern in &platform_patterns {
                if !current_patterns.contains(platform_pattern) {
                    issues.push(format!(
                        "Directory {} missing recommended exclude pattern: {}",
                        index, platform_pattern
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
                    issues.push(
                        "Server port requires administrator privileges on Windows".to_string(),
                    );
                }
                // Note: SSDP port is hardcoded to 1900 and requires administrator privileges on Windows
            }
            crate::platform::OsType::MacOS => {
                if self.server.port < 1024 {
                    issues
                        .push("Server port requires administrator privileges on macOS".to_string());
                }
            }
            crate::platform::OsType::Linux => {
                if self.server.port < 1024 {
                    issues.push("Server port requires root privileges on Linux".to_string());
                }
            }
            crate::platform::OsType::Bsd => {
                if self.server.port < 1024 {
                    issues.push("Server port requires root privileges on BSD".to_string());
                }
            }
        }

        issues
    }
}

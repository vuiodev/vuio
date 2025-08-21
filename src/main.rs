use anyhow::Context;
use vuio::{
    config::AppConfig,
    database::{self, DatabaseManager, SqliteDatabase},
    logging, media,
    platform::{self, PlatformInfo},
    ssdp,
    state::AppState,
    watcher::{CrossPlatformWatcher, FileSystemEvent, FileSystemWatcher},
    web,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Parse early command line arguments to get debug flag and config file path
/// This is needed before logging initialization
fn parse_early_args() -> (bool, Option<String>) {
    use clap::Parser;
    
    #[derive(Parser, Debug)]
    #[command(author, version, about, long_about = None)]
    struct EarlyArgs {
        /// The directory containing media files to serve
        _media_dir: Option<String>,

        /// The network port to listen on
        #[arg(short, long)]
        _port: Option<u16>,

        /// The friendly name for the DLNA server
        #[arg(short, long, default_value = "VuIO Server")]
        _name: String,

        /// Enable debug logging
        #[arg(long)]
        debug: bool,

        /// Path to configuration file
        #[arg(short, long)]
        config: Option<String>,
    }
    
    // Parse args, but ignore errors since we'll parse them again later
    match EarlyArgs::try_parse() {
        Ok(args) => (args.debug, args.config),
        Err(_) => (false, None), // Default to no debug and no config file
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse command line arguments first to get debug flag
    let (debug_enabled, config_file_path) = parse_early_args();
    
    // Initialize logging with debug flag
    if debug_enabled {
        logging::init_logging_with_debug(true).context("Failed to initialize debug logging")?;
    } else {
        logging::init_logging().context("Failed to initialize logging")?;
    }

    info!("Starting VuIO Server...");

    // Detect platform information with comprehensive diagnostics
    let platform_info = match detect_platform_with_diagnostics().await {
        Ok(info) => Arc::new(info),
        Err(e) => {
            error!("Failed to detect platform information: {}", e);
            return Err(e);
        }
    };

    // Perform platform-specific security checks and permission requests
    if let Err(e) = perform_security_checks(&platform_info).await {
        error!("Security checks failed: {}", e);
        return Err(e);
    }

    // Load or create configuration with platform-specific defaults
    let config = match initialize_configuration(&platform_info).await {
        Ok(config) => Arc::new(config),
        Err(e) => {
            error!("Failed to initialize configuration: {}", e);
            return Err(e);
        }
    };

    // Initialize database manager
    let database = match initialize_database(&config).await {
        Ok(db) => Arc::new(db) as Arc<dyn DatabaseManager>,
        Err(e) => {
            error!("Failed to initialize database: {}", e);
            return Err(e);
        }
    };

    // Initialize file system watcher
    let file_watcher = match initialize_file_watcher(&config, database.clone()).await {
        Ok(watcher) => Arc::new(watcher),
        Err(e) => {
            error!("Failed to initialize file system watcher: {}", e);
            return Err(e);
        }
    };

    // Perform initial media scan or load from database
    let media_files = match perform_initial_media_scan(&config, &database).await {
        Ok(files) => Arc::new(RwLock::new(files)),
        Err(e) => {
            error!("Failed to perform initial media scan: {}", e);
            return Err(e);
        }
    };

    // Create shared application state
    let app_state = AppState {
        config: config.clone(),
        media_files: media_files.clone(),
        database: database.clone(),
        platform_info: platform_info.clone(),
        content_update_id: Arc::new(std::sync::atomic::AtomicU32::new(1)),
    };

    // Start file system monitoring
    if let Err(e) = start_file_monitoring(file_watcher.clone(), app_state.clone()).await {
        warn!("Failed to start file system monitoring: {}", e);
        warn!("Continuing without real-time file monitoring");
    }

    // Start runtime platform adaptation services
    let adaptation_handle = start_platform_adaptation(
        platform_info.clone(),
        config.clone(),
        database.clone(),
        media_files.clone(),
    ).await?;

    // Start SSDP discovery service with platform abstraction
    if let Err(e) = start_ssdp_service(app_state.clone()).await {
        error!("Failed to start SSDP service: {}", e);
        return Err(e);
    }

    // Start the HTTP server
    if let Err(e) = start_http_server(app_state).await {
        error!("Failed to start HTTP server: {}", e);
        return Err(e);
    }

    // Wait for shutdown signal and cleanup
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal (Ctrl+C)");
        }
        _ = adaptation_handle => {
            warn!("Platform adaptation service stopped unexpectedly");
        }
    }

    // Perform graceful shutdown
    perform_graceful_shutdown(database, file_watcher).await?;
    
    info!("Shutdown completed successfully");
    Ok(())
}

/// Start platform adaptation services for runtime detection and adaptation
async fn start_platform_adaptation(
    platform_info: Arc<PlatformInfo>,
    config: Arc<AppConfig>,
    database: Arc<dyn DatabaseManager>,
    media_files: Arc<RwLock<Vec<database::MediaFile>>>,
) -> anyhow::Result<tokio::task::JoinHandle<()>> {
    info!("Starting platform adaptation services...");
    
    let platform_info_clone = platform_info.clone();
    let config_clone = config.clone();
    let database_clone = database.clone();
    let media_files_clone = media_files.clone();
    
    let handle = tokio::spawn(async move {
        let mut network_check_interval = tokio::time::interval(std::time::Duration::from_secs(30));
        let mut config_check_interval = tokio::time::interval(std::time::Duration::from_secs(60));
        
        loop {
            tokio::select! {
                _ = network_check_interval.tick() => {
                    if let Err(e) = check_and_adapt_network_changes(&platform_info_clone).await {
                        warn!("Network adaptation check failed: {}", e);
                    }
                }
                _ = config_check_interval.tick() => {
                    if let Err(e) = check_and_reload_configuration(&config_clone, &database_clone, &media_files_clone).await {
                        warn!("Configuration reload check failed: {}", e);
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Platform adaptation service received shutdown signal");
                    break;
                }
            }
        }
        
        info!("Platform adaptation service stopped");
    });
    
    info!("Platform adaptation services started");
    Ok(handle)
}

/// Check for network changes and adapt accordingly
async fn check_and_adapt_network_changes(platform_info: &Arc<PlatformInfo>) -> anyhow::Result<()> {
    // Re-detect network interfaces to check for changes
    let current_platform_info = PlatformInfo::detect().await
        .context("Failed to re-detect platform information")?;
    
    // Compare network interfaces
    let old_interfaces = &platform_info.network_interfaces;
    let new_interfaces = &current_platform_info.network_interfaces;
    
    // Check if interfaces have changed
    let interfaces_changed = old_interfaces.len() != new_interfaces.len() ||
        old_interfaces.iter().any(|old_iface| {
            !new_interfaces.iter().any(|new_iface| {
                old_iface.name == new_iface.name &&
                old_iface.ip_address == new_iface.ip_address &&
                old_iface.is_up == new_iface.is_up
            })
        });
    
    if interfaces_changed {
        info!("Network interface changes detected");
        
        // Log changes
        for old_iface in old_interfaces {
            if !new_interfaces.iter().any(|new_iface| new_iface.name == old_iface.name) {
                info!("Network interface removed: {} ({})", old_iface.name, old_iface.ip_address);
            }
        }
        
        for new_iface in new_interfaces {
            if !old_interfaces.iter().any(|old_iface| old_iface.name == new_iface.name) {
                info!("Network interface added: {} ({})", new_iface.name, new_iface.ip_address);
            } else if let Some(old_iface) = old_interfaces.iter().find(|old| old.name == new_iface.name) {
                if old_iface.ip_address != new_iface.ip_address {
                    info!("Network interface IP changed: {} ({} -> {})", 
                        new_iface.name, old_iface.ip_address, new_iface.ip_address);
                }
                if old_iface.is_up != new_iface.is_up {
                    let status = if new_iface.is_up { "UP" } else { "DOWN" };
                    info!("Network interface status changed: {} is now {}", new_iface.name, status);
                }
            }
        }
        
        // Check if primary interface changed
        let old_primary = platform_info.get_primary_interface();
        let new_primary = current_platform_info.get_primary_interface();
        
        match (old_primary, new_primary) {
            (Some(old), Some(new)) if old.name != new.name || old.ip_address != new.ip_address => {
                info!("Primary network interface changed: {} ({}) -> {} ({})",
                    old.name, old.ip_address, new.name, new.ip_address);
                // TODO: Restart SSDP service with new interface
            }
            (Some(old), None) => {
                warn!("Primary network interface lost: {} ({})", old.name, old.ip_address);
                warn!("DLNA discovery may not work properly");
            }
            (None, Some(new)) => {
                info!("Primary network interface available: {} ({})", new.name, new.ip_address);
                // TODO: Start SSDP service if it wasn't running
            }
            _ => {} // No change in primary interface
        }
        
        // Implement graceful degradation
        if new_interfaces.is_empty() {
            error!("No network interfaces available - DLNA functionality will be severely limited");
        } else if new_interfaces.iter().all(|iface| !iface.supports_multicast) {
            warn!("No multicast-capable interfaces available - DLNA discovery may not work");
        }
    }
    
    Ok(())
}

/// Check for configuration changes and reload if necessary
async fn check_and_reload_configuration(
    config: &Arc<AppConfig>,
    database: &Arc<dyn DatabaseManager>,
    media_files: &Arc<RwLock<Vec<database::MediaFile>>>,
) -> anyhow::Result<()> {
    let config_path = AppConfig::get_platform_config_file_path();
    
    // Check if configuration file has been modified
    if let Ok(metadata) = tokio::fs::metadata(&config_path).await {
        let modified = metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        
        // For simplicity, we'll check if the file was modified in the last minute
        // In a real implementation, we'd track the last known modification time
        let one_minute_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
        
        if modified > one_minute_ago {
            info!("Configuration file may have been modified, checking for changes...");
            
            match AppConfig::load_from_file(&config_path) {
                Ok(new_config) => {
                    if let Err(e) = handle_configuration_changes(config, &new_config, database, media_files).await {
                        warn!("Failed to handle configuration changes: {}", e);
                    }
                }
                Err(e) => {
                    warn!("Failed to load updated configuration: {}", e);
                }
            }
        }
    }
    
    Ok(())
}

/// Handle configuration changes by updating relevant services
async fn handle_configuration_changes(
    old_config: &Arc<AppConfig>,
    new_config: &AppConfig,
    database: &Arc<dyn DatabaseManager>,
    media_files: &Arc<RwLock<Vec<database::MediaFile>>>,
) -> anyhow::Result<()> {
    let mut changes_detected = false;
    
    // Check for media directory changes
    let old_dirs: std::collections::HashSet<_> = old_config.media.directories
        .iter()
        .map(|d| &d.path)
        .collect();
    let new_dirs: std::collections::HashSet<_> = new_config.media.directories
        .iter()
        .map(|d| &d.path)
        .collect();
    
    if old_dirs != new_dirs {
        info!("Media directory configuration changed");
        changes_detected = true;
        
        let scanner = media::MediaScanner::with_database(database.clone());
        let mut cache_needs_reload = false;

        // Find added directories
        for new_dir_path_str in &new_dirs {
            if !old_dirs.contains(new_dir_path_str) {
                info!("New media directory added: {}", new_dir_path_str);
                
                if let Some(dir_config) = new_config.media.directories.iter().find(|d| &d.path == *new_dir_path_str) {
                    let dir_path = std::path::PathBuf::from(&dir_config.path);
                    if dir_path.exists() && dir_path.is_dir() {
                        let scan_result = if dir_config.recursive {
                            scanner.scan_directory_recursive(&dir_path).await
                        } else {
                            scanner.scan_directory(&dir_path).await
                        };

                        match scan_result {
                            Ok(result) => {
                                info!("Scanned new directory {}: {}", dir_config.path, result.summary());
                                if result.has_changes() {
                                    cache_needs_reload = true;
                                }
                            }
                            Err(e) => {
                                warn!("Failed to scan new directory {}: {}", dir_config.path, e);
                            }
                        }
                    } else {
                        warn!("Newly added directory does not exist or is not a directory: {}", dir_path.display());
                    }
                }
            }
        }
        
        // Find removed directories
        for old_dir in &old_dirs {
            if !new_dirs.contains(old_dir) {
                info!("Media directory removed: {}", old_dir);
                
                // Remove files from this directory from database and cache
                let dir_path = std::path::PathBuf::from(old_dir);
                let files_to_remove = database.get_files_in_directory(&dir_path).await
                    .unwrap_or_default();
                
                if !files_to_remove.is_empty() {
                    cache_needs_reload = true;
                }

                for file in &files_to_remove {
                    if let Err(e) = database.remove_media_file(&file.path).await {
                        warn!("Failed to remove media file from database: {} - {}", file.path.display(), e);
                    }
                }
                
                info!("Removed {} files from removed directory", files_to_remove.len());
            }
        }

        if cache_needs_reload {
            info!("Reloading in-memory media cache due to directory changes...");
            let all_files = database.get_all_media_files().await?;
            *media_files.write().await = all_files;
            info!("In-memory cache updated with {} files.", media_files.read().await.len());
        }
    }
    
    // Check for file watching changes
    if old_config.media.watch_for_changes != new_config.media.watch_for_changes {
        info!("File watching configuration changed: {} -> {}", 
            old_config.media.watch_for_changes, new_config.media.watch_for_changes);
        changes_detected = true;
        
        if new_config.media.watch_for_changes {
            info!("File watching enabled - new file changes will be detected");
            // TODO: Start file watcher if not already running
        } else {
            info!("File watching disabled - file changes will not be detected automatically");
            // TODO: Stop file watcher if running
        }
    }
    
    // Check for network configuration changes
    if old_config.network.ssdp_port != new_config.network.ssdp_port ||
       old_config.network.interface_selection != new_config.network.interface_selection {
        info!("Network configuration changed");
        changes_detected = true;
        
        if old_config.network.ssdp_port != new_config.network.ssdp_port {
            info!("SSDP port changed: {} -> {}", old_config.network.ssdp_port, new_config.network.ssdp_port);
        }
        
        if old_config.network.interface_selection != new_config.network.interface_selection {
            info!("Network interface selection changed: {:?} -> {:?}", 
                old_config.network.interface_selection, new_config.network.interface_selection);
        }
        
        // TODO: Restart SSDP service with new configuration
        warn!("Network configuration changes require service restart to take effect");
    }
    
    // Check for server configuration changes
    if old_config.server.port != new_config.server.port ||
       old_config.server.interface != new_config.server.interface {
        info!("Server configuration changed");
        changes_detected = true;
        
        if old_config.server.port != new_config.server.port {
            info!("Server port changed: {} -> {}", old_config.server.port, new_config.server.port);
        }
        
        if old_config.server.interface != new_config.server.interface {
            info!("Server interface changed: {} -> {}", old_config.server.interface, new_config.server.interface);
        }
        
        warn!("Server configuration changes require application restart to take effect");
    }
    
    if changes_detected {
        info!("Configuration changes processed successfully");
    }
    
    Ok(())
}

/// Implement graceful degradation when platform features are unavailable
async fn handle_platform_feature_unavailable(feature: &str, error: &anyhow::Error) -> anyhow::Result<()> {
    match feature {
        "multicast" => {
            warn!("Multicast networking unavailable: {}", error);
            warn!("DLNA discovery will be limited - clients may need manual configuration");
            info!("Consider using unicast discovery or manual IP configuration");
        }
        "privileged_ports" => {
            warn!("Privileged port access unavailable: {}", error);
            info!("Using alternative ports for DLNA services");
            info!("SSDP will use port 8080 instead of 1900");
        }
        "file_watching" => {
            warn!("File system watching unavailable: {}", error);
            warn!("Media library changes will not be detected automatically");
            info!("Consider periodic manual rescans or application restart after adding media");
        }
        "database" => {
            error!("Database functionality unavailable: {}", error);
            error!("Media library persistence will not work");
            warn!("Falling back to in-memory media scanning on each startup");
        }
        "network_interfaces" => {
            error!("No network interfaces available: {}", error);
            error!("DLNA functionality will be severely limited");
            warn!("Check network configuration and try again");
        }
        _ => {
            warn!("Platform feature '{}' unavailable: {}", feature, error);
        }
    }
    
    Ok(())
}

/// Detect platform information with comprehensive diagnostics and error reporting
async fn detect_platform_with_diagnostics() -> anyhow::Result<PlatformInfo> {
    info!("Detecting platform information...");
    
    let platform_info = PlatformInfo::detect().await
        .context("Failed to detect platform information")?;
    
    // Log comprehensive platform information
    info!("Platform: {} {}", platform_info.os_type.display_name(), platform_info.version);
    info!("Architecture: {}", std::env::consts::ARCH);
    
    info!("Platform capabilities:");
    info!("  - Multicast support: {}", platform_info.capabilities.supports_multicast);
    info!("  - Firewall present: {}", platform_info.capabilities.has_firewall);
    info!("  - Case-sensitive filesystem: {}", platform_info.capabilities.case_sensitive_fs);
    
    // Log network interface information
    if platform_info.network_interfaces.is_empty() {
        warn!("No network interfaces detected - network functionality may be limited");
    } else {
        info!("Detected {} network interface(s):", platform_info.network_interfaces.len());
        for interface in &platform_info.network_interfaces {
            info!("  - {} ({}): {} - Up: {}, Multicast: {}", 
                interface.name, 
                interface.ip_address,
                match interface.interface_type {
                    platform::InterfaceType::Ethernet => "Ethernet",
                    platform::InterfaceType::WiFi => "WiFi",
                    platform::InterfaceType::VPN => "VPN",
                    platform::InterfaceType::Loopback => "Loopback",
                    platform::InterfaceType::Other(ref name) => name,
                },
                interface.is_up,
                interface.supports_multicast
            );
        }
        
        if let Some(primary_interface) = platform_info.get_primary_interface() {
            info!("Primary network interface: {} ({})", primary_interface.name, primary_interface.ip_address);
        } else {
            warn!("No suitable primary network interface found for DLNA operations");
        }
    }
    
    // Platform-specific diagnostics
    match platform_info.os_type {
        platform::OsType::Windows => {
            info!("Windows-specific diagnostics:");
            if !platform_info.capabilities.can_bind_privileged_ports {
                info!("  - Administrator privileges may be required for ports < 1024");
            }
            if platform_info.capabilities.has_firewall {
                info!("  - Windows Firewall may block network connections");
            }
        }
        platform::OsType::MacOS => {
            info!("macOS-specific diagnostics:");
            info!("  - System may prompt for network access permissions");
            if platform_info.capabilities.has_firewall {
                info!("  - macOS Application Firewall may block connections");
            }
        }
        platform::OsType::Linux => {
            info!("Linux-specific diagnostics:");
            if platform_info.capabilities.has_firewall {
                info!("  - Firewall (iptables/ufw/firewalld) may block connections");
            }
            info!("  - SELinux/AppArmor policies may affect file access");
        }
    }
    
    Ok(platform_info)
}

/// Initialize configuration with platform-specific defaults and validation
async fn initialize_configuration(_platform_info: &PlatformInfo) -> anyhow::Result<AppConfig> {
    info!("Initializing configuration...");
    
    let config_path = AppConfig::get_platform_config_file_path();
    info!("Configuration file path: {}", config_path.display());
    
    // First, try to load from command line arguments
    match AppConfig::from_args().await {
        Ok((config, debug, config_path)) => {
            if let Some(path) = config_path {
                info!("Using configuration from file: {}", path);
            } else {
                info!("Using configuration from command line arguments");
            }
            
            if debug {
                debug!("Debug logging enabled via command line");
            }
            
            // Apply platform-specific defaults for any missing values
            let mut config = config;
            config.apply_platform_defaults()
                .context("Failed to apply platform-specific defaults to command line configuration")?;
            
            // Validate the final configuration
            config.validate_for_platform()
                .context("Command line configuration validation failed")?;
            
            info!("Configuration validated successfully");
            info!("Server will listen on: {}:{}", config.server.interface, config.server.port);
            info!("SSDP will use port: {}", config.network.ssdp_port);
            info!("Monitoring {} director(ies) for media files", config.media.directories.len());
            
            for (i, dir) in config.media.directories.iter().enumerate() {
                info!("  {}. {} (recursive: {})", i + 1, dir.path, dir.recursive);
            }
            
            return Ok(config);
        }
        Err(e) => {
            debug!("No valid command line arguments provided: {}", e);
            info!("Falling back to configuration file or platform defaults");
        }
    }
    
    // Fall back to configuration file or defaults
    let mut config = if config_path.exists() {
        info!("Loading existing configuration from: {}", config_path.display());
        AppConfig::load_from_file(&config_path)
            .context("Failed to load configuration file")?
    } else {
        info!("Creating new configuration with platform defaults");
        AppConfig::default_for_platform()
    };
    
    // Apply platform-specific defaults and validation
    config.apply_platform_defaults()
        .context("Failed to apply platform-specific defaults")?;
    
    config.validate_for_platform()
        .context("Configuration validation failed")?;
    
    // Save the configuration (creates file if it doesn't exist, updates if needed)
    config.save_to_file(&config_path)
        .context("Failed to save configuration file")?;
    
    info!("Configuration initialized successfully");
    info!("Server will listen on: {}:{}", config.server.interface, config.server.port);
    info!("SSDP will use port: {}", config.network.ssdp_port);
    info!("Monitoring {} director(ies) for media files", config.media.directories.len());
    
    for (i, dir) in config.media.directories.iter().enumerate() {
        info!("  {}. {} (recursive: {})", i + 1, dir.path, dir.recursive);
    }
    
    Ok(config)
}

/// Initialize database manager with health checks and recovery
async fn initialize_database(config: &AppConfig) -> anyhow::Result<SqliteDatabase> {
    info!("Initializing database...");
    
    let db_path = config.get_database_path();
    info!("Database path: {}", db_path.display());
    
    // Create database manager
    let database = SqliteDatabase::new(db_path.clone()).await
        .context("Failed to create database manager")?;
    
    // Initialize database schema
    database.initialize().await
        .context("Failed to initialize database schema")?;
    
    // Perform health check and repair if needed
    info!("Performing database health check...");
    let health = database.check_and_repair().await
        .context("Failed to perform database health check")?;
    
    if !health.is_healthy {
        warn!("Database health issues detected:");
        for issue in &health.issues {
            match issue.severity {
                database::IssueSeverity::Critical => error!("  CRITICAL: {}", issue.description),
                database::IssueSeverity::Error => error!("  ERROR: {}", issue.description),
                database::IssueSeverity::Warning => warn!("  WARNING: {}", issue.description),
                database::IssueSeverity::Info => info!("  INFO: {}", issue.description),
            }
        }
        
        if health.repair_attempted && health.repair_successful {
            info!("Database repair completed successfully");
        } else if health.repair_attempted && !health.repair_successful {
            error!("Database repair failed - some functionality may be limited");
        }
    } else {
        info!("Database health check passed");
    }
    
    // Get database statistics
    let stats = database.get_stats().await
        .context("Failed to get database statistics")?;
    
    info!("Database statistics:");
    info!("  - Total media files: {}", stats.total_files);
    info!("  - Total media size: {} bytes", stats.total_size);
    info!("  - Database file size: {} bytes", stats.database_size);
    
    // Vacuum database if configured
    if config.database.vacuum_on_startup {
        info!("Performing database vacuum...");
        database.vacuum().await
            .context("Failed to vacuum database")?;
        info!("Database vacuum completed");
    }
    
    info!("Database initialized successfully");
    Ok(database)
}

/// Initialize file system watcher for real-time media monitoring
async fn initialize_file_watcher(config: &AppConfig, _database: Arc<dyn DatabaseManager>) -> anyhow::Result<CrossPlatformWatcher> {
    info!("Initializing file system watcher...");
    
    if !config.media.watch_for_changes {
        info!("File system watching disabled in configuration");
        return Ok(CrossPlatformWatcher::new());
    }
    
    let watcher = CrossPlatformWatcher::new();
    
    // Validate that all monitored directories exist
    let mut valid_directories = Vec::new();
    for dir_config in &config.media.directories {
        let dir_path = std::path::PathBuf::from(&dir_config.path);
        if dir_path.exists() && dir_path.is_dir() {
            valid_directories.push(dir_path);
        } else {
            warn!("Monitored directory does not exist or is not a directory: {}", dir_config.path);
        }
    }
    
    if valid_directories.is_empty() {
        warn!("No valid directories to monitor - file watching will be disabled");
        return Ok(watcher);
    }
    
    info!("File system watcher initialized for {} directories", valid_directories.len());
    Ok(watcher)
}

/// Validate cached files and remove any that no longer exist on disk
async fn validate_and_cleanup_deleted_files(
    database: Arc<dyn DatabaseManager>,
    cached_files: Vec<database::MediaFile>,
) -> anyhow::Result<Vec<database::MediaFile>> {
    info!("Validating {} cached media files...", cached_files.len());
    
    let mut valid_files = Vec::new();
    let mut removed_count = 0;
    
    for file in cached_files {
        if file.path.exists() {
            valid_files.push(file);
        } else {
            info!("Removing deleted file from database: {}", file.path.display());
            if database.remove_media_file(&file.path).await? {
                removed_count += 1;
            }
        }
    }
    
    if removed_count > 0 {
        info!("Cleaned up {} deleted files from database", removed_count);
    } else {
        info!("All cached files are still present on disk");
    }
    

    Ok(valid_files)
}

/// Perform initial media scan, using database cache when possible
async fn perform_initial_media_scan(config: &AppConfig, database: &Arc<dyn DatabaseManager>) -> anyhow::Result<Vec<database::MediaFile>> {
    info!("Performing initial media scan...");

    if config.media.scan_on_startup {
        info!("Full media scan enabled - scanning all directories");

        let scanner = media::MediaScanner::with_database(database.clone());
        let mut total_changes = 0;
        let mut total_files_scanned = 0;

        for dir_config in &config.media.directories {
            let dir_path = std::path::PathBuf::from(&dir_config.path);

            if !dir_path.exists() {
                warn!("Media directory does not exist: {}", dir_config.path);
                continue;
            }

            info!("Scanning directory: {}", dir_config.path);

            let scan_result = if dir_config.recursive {
                scanner.scan_directory_recursive(&dir_path).await
                    .with_context(|| format!("Failed to recursively scan directory: {}", dir_config.path))?
            } else {
                scanner.scan_directory(&dir_path).await
                    .with_context(|| format!("Failed to scan directory: {}", dir_config.path))?
            };

            info!("Scan of {} completed: {}", dir_path.display(), scan_result.summary());
            if !scan_result.errors.is_empty() {
                // FIX: Iterate over a reference to avoid moving scan_result.errors
                for err in &scan_result.errors {
                    warn!("Scan error in {}: {}", err.path.display(), err.error);
                }
            }
            total_changes += scan_result.total_changes();
            total_files_scanned += scan_result.total_scanned;
        }

        info!("Initial media scan completed - total files scanned: {}, total changes: {}", total_files_scanned, total_changes);

        // After scan, load all media files from the database for the application state
        let all_media_files = database.get_all_media_files().await
            .context("Failed to load media files from database after scan")?;

        info!("Loaded {} total media files from database", all_media_files.len());
        
        // Even after a full scan, validate files to catch any that were deleted while app was offline
        // This is important because the scan only covers configured directories
        let validated_files = if config.media.cleanup_deleted_files {
            validate_and_cleanup_deleted_files(database.clone(), all_media_files).await?
        } else {
            all_media_files
        };
        
        Ok(validated_files)
    } else {
        info!("Loading media files from database cache (scan on startup disabled)");

        let cached_files = database.get_all_media_files().await
            .context("Failed to load media files from database")?;

        info!("Loaded {} media files from database cache", cached_files.len());

        // Validate that cached files still exist on disk and remove any that don't (if enabled)
        let validated_files = if config.media.cleanup_deleted_files {
            validate_and_cleanup_deleted_files(database.clone(), cached_files).await?
        } else {
            info!("Cleanup of deleted files is disabled");
            cached_files
        };

        Ok(validated_files)
    }
}

/// Start file system monitoring with database integration
async fn start_file_monitoring(
    watcher: Arc<CrossPlatformWatcher>,
    app_state: AppState,
) -> anyhow::Result<()> {
    if !app_state.config.media.watch_for_changes {
        info!("File system monitoring disabled");
        return Ok(());
    }
    
    info!("Starting file system monitoring...");
    
    // Get directories to monitor
    let directories: Vec<std::path::PathBuf> = app_state.config.media.directories
        .iter()
        .map(|dir| std::path::PathBuf::from(&dir.path))
        .filter(|path| path.exists() && path.is_dir())
        .collect();
    
    if directories.is_empty() {
        warn!("No valid directories to monitor");
        return Ok(());
    }
    
    info!("Starting to monitor {} directories:", directories.len());
    for (i, dir) in directories.iter().enumerate() {
        info!("  {}: {}", i + 1, dir.display());
    }
    
    // Start watching directories
    watcher.start_watching(&directories).await
        .context("Failed to start watching directories")?;
    
    info!("File system watcher successfully started for all directories");
    
    // Get event receiver
    let mut event_receiver = watcher.get_event_receiver();
    
    // Spawn task to handle file system events
    let app_state_clone = app_state.clone();
    
    tokio::spawn(async move {
        info!("File system event handler started");
        
        while let Some(event) = event_receiver.recv().await {
            if let Err(e) = handle_file_system_event(event, &app_state_clone).await {
                error!("Failed to handle file system event: {}", e);
            }
        }
        
        warn!("File system event handler stopped");
    });
    
    info!("File system monitoring started for {} directories", directories.len());
    Ok(())
}

/// Increment the content update ID to notify DLNA clients of changes
fn increment_content_update_id(app_state: &AppState) {
    let old_id = app_state.content_update_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let new_id = old_id + 1;
    info!("Content update ID incremented from {} to {}", old_id, new_id);
    
    // Send UPnP event notifications to subscribed clients
    // In a full implementation, we would maintain a list of subscribed clients
    // For now, we'll just log that an event should be sent
    info!("UPnP event notification should be sent with UpdateID: {}", new_id);
}

/// Handle individual file system events
async fn handle_file_system_event(
    event: FileSystemEvent,
    app_state: &AppState,
) -> anyhow::Result<()> {
    let database = &app_state.database;
    let media_files = &app_state.media_files;
    match event {
        FileSystemEvent::Created(path) => {
            // Check if this is a directory or a file
            if path.is_dir() {
                info!("Directory created: {}", path.display());
                
                // Scan the new directory for media files
                let scanner = media::MediaScanner::with_database(database.clone());
                match scanner.scan_directory_recursive(&path).await {
                    Ok(scan_result) => {
                        info!("Scanned new directory {}: {}", path.display(), scan_result.summary());
                        
                        // Update in-memory cache with newly found files
                        if !scan_result.new_files.is_empty() {
                            let mut files = media_files.write().await;
                            for new_file in &scan_result.new_files {
                                // Only add if not already in cache
                                if !files.iter().any(|f| f.path == new_file.path) {
                                    files.push(new_file.clone());
                                }
                            }
                        }
                        
                        info!("Added {} media files from new directory: {}", scan_result.new_files.len(), path.display());
                        
                        // Increment update ID to notify DLNA clients
                        if !scan_result.new_files.is_empty() {
                            increment_content_update_id(app_state);
                        }
                    }
                    Err(e) => {
                        error!("Failed to scan new directory {}: {}", path.display(), e);
                    }
                }
            } else {
                // Handle individual media file creation
                info!("Media file created: {}", path.display());
                
                // Check if it's actually a media file
                let is_media_file = if let Some(extension) = path.extension() {
                    if let Some(ext_str) = extension.to_str() {
                        crate::platform::filesystem::is_supported_media_extension(ext_str)
                    } else {
                        false
                    }
                } else {
                    false
                };
                
                if !is_media_file {
                    debug!("Not a supported media file, ignoring: {}", path.display());
                    return Ok(());
                }
                
                // Create MediaFile record
                let metadata = tokio::fs::metadata(&path).await?;
                let mime_type = media::get_mime_type(&path);
                let mut media_file = database::MediaFile::new(path.clone(), metadata.len(), mime_type);
                media_file.modified = metadata.modified().unwrap_or(std::time::SystemTime::now());
                
                // Store in database
                let file_id = database.store_media_file(&media_file).await?;
                media_file.id = Some(file_id);
                
                // Add to in-memory cache
                let mut files = media_files.write().await;
                files.push(media_file);
                
                info!("Added new media file to database: {}", path.display());
                
                // Increment update ID to notify DLNA clients
                increment_content_update_id(app_state);
            }
        }
        
        FileSystemEvent::Modified(path) => {
            info!("Media file modified: {}", path.display());
            
            // Update database record
            if let Some(mut existing_file) = database.get_file_by_path(&path).await? {
                let metadata = tokio::fs::metadata(&path).await?;
                existing_file.size = metadata.len();
                existing_file.modified = metadata.modified().unwrap_or(std::time::SystemTime::now());
                
                database.update_media_file(&existing_file).await?;
                
                // Update in-memory cache
                let mut files = media_files.write().await;
                if let Some(cached_file) = files.iter_mut().find(|f| f.path == path) {
                    *cached_file = existing_file;
                }
                
                info!("Updated media file in database: {}", path.display());
                
                // Increment update ID to notify DLNA clients
                increment_content_update_id(app_state);
            }
        }
        
        FileSystemEvent::Deleted(path) => {
            // Since the path no longer exists, we can't check if it was a directory
            // We'll handle both cases: try to remove as a single file, and also
            // remove any files that were in this path (in case it was a directory)
            
            info!("Path deleted: {}", path.display());
            
            // First, try to remove as a single file
            let single_file_removed = match database.remove_media_file(&path).await {
                Ok(removed) => {
                    if removed {
                        info!("Removed single file from database: {}", path.display());
                    } else {
                        info!("Single file not found in database: {}", path.display());
                    }
                    removed
                }
                Err(e) => {
                    warn!("Error removing single file from database {}: {}", path.display(), e);
                    false
                }
            };
            
            // Also check for files that were in this directory path
            let all_files = match database.get_all_media_files().await {
                Ok(files) => files,
                Err(e) => {
                    warn!("Error getting all media files: {}", e);
                    return Ok(());
                }
            };
            
            // Normalize paths for case-insensitive comparison on Windows
            let normalized_deleted_path = path.to_string_lossy().to_lowercase();
            let files_in_deleted_path: Vec<_> = all_files
                .iter()
                .filter(|file| {
                    let normalized_file_path = file.path.to_string_lossy().to_lowercase();
                    let matches = normalized_file_path.starts_with(&normalized_deleted_path);
                    if matches {
                        info!("Found file in deleted path: {} starts with {}", file.path.display(), path.display());
                    }
                    matches
                })
                .collect();
            
            let mut total_removed = if single_file_removed { 1 } else { 0 };
            
            if !files_in_deleted_path.is_empty() {
                info!("Found {} media files in deleted directory: {}", files_in_deleted_path.len(), path.display());
                
                for file in &files_in_deleted_path {
                    match database.remove_media_file(&file.path).await {
                        Ok(true) => {
                            total_removed += 1;
                            info!("Removed file from database: {}", file.path.display());
                        }
                        Ok(false) => {
                            info!("File not found in database: {}", file.path.display());
                        }
                        Err(e) => {
                            warn!("Error removing file from database {}: {}", file.path.display(), e);
                        }
                    }
                }
            } else {
                info!("No files found in deleted path: {}", path.display());
                // Debug: show some database paths for comparison
                let sample_paths: Vec<_> = all_files.iter().take(5).map(|f| f.path.display().to_string()).collect();
                info!("Sample database paths: {:?}", sample_paths);
            }
            
            // Remove from in-memory cache (case-insensitive on Windows)
            let mut files = media_files.write().await;
            let initial_count = files.len();
            files.retain(|f| {
                let normalized_file_path = f.path.to_string_lossy().to_lowercase();
                !normalized_file_path.starts_with(&normalized_deleted_path)
            });
            let removed_from_cache = initial_count - files.len();
            
            info!("Cache cleanup: removed {} files from in-memory cache", removed_from_cache);
            
            if total_removed > 0 || removed_from_cache > 0 {
                info!("Total cleanup: {} files from database, {} from cache for path: {}", 
                      total_removed, removed_from_cache, path.display());
                
                // Increment update ID to notify DLNA clients
                increment_content_update_id(app_state);
                info!("Notified DLNA clients of content change");
            } else {
                info!("No files were removed for deleted path: {}", path.display());
            }
        }
        
        FileSystemEvent::Renamed { from, to } => {
            info!("Path renamed: {} -> {}", from.display(), to.display());
            
            // Check if the destination is a directory or file
            if to.is_dir() {
                // Handle directory rename
                info!("Directory renamed: {} -> {}", from.display(), to.display());
                
                // Get all files that were in the old directory path
                let all_files = database.get_all_media_files().await?;
                let files_in_old_path: Vec<_> = all_files
                    .iter()
                    .filter(|file| file.path.starts_with(&from))
                    .collect();
                
                if !files_in_old_path.is_empty() {
                    info!("Updating {} media files for renamed directory", files_in_old_path.len());
                    
                    // Remove old files from database and cache
                    let mut files = media_files.write().await;
                    for old_file in &files_in_old_path {
                        database.remove_media_file(&old_file.path).await?;
                        files.retain(|f| f.path != old_file.path);
                    }
                    drop(files); // Release the lock before scanning
                    
                    // Scan the new directory location
                    let scanner = media::MediaScanner::with_database(database.clone());
                    match scanner.scan_directory_recursive(&to).await {
                        Ok(scan_result) => {
                            info!("Rescanned renamed directory {}: {}", to.display(), scan_result.summary());
                            
                            // Update in-memory cache with newly found files
                            if !scan_result.new_files.is_empty() {
                                let mut files = media_files.write().await;
                                for new_file in &scan_result.new_files {
                                    files.push(new_file.clone());
                                }
                            }
                            
                            // Increment update ID to notify DLNA clients
                            increment_content_update_id(app_state);
                        }
                        Err(e) => {
                            error!("Failed to rescan renamed directory {}: {}", to.display(), e);
                        }
                    }
                }
            } else {
                // Handle individual file rename
                info!("File renamed: {} -> {}", from.display(), to.display());
                
                // Check if it's a media file
                let is_media_file = if let Some(extension) = to.extension() {
                    if let Some(ext_str) = extension.to_str() {
                        crate::platform::filesystem::is_supported_media_extension(ext_str)
                    } else {
                        false
                    }
                } else {
                    false
                };
                
                if !is_media_file {
                    debug!("Renamed file is not a media file, ignoring: {}", to.display());
                    return Ok(());
                }
                
                // Remove old file from database and cache
                database.remove_media_file(&from).await?;
                let mut files = media_files.write().await;
                files.retain(|f| f.path != from);
                
                // Create MediaFile record for new location
                let metadata = tokio::fs::metadata(&to).await?;
                let mime_type = media::get_mime_type(&to);
                let mut media_file = database::MediaFile::new(to.clone(), metadata.len(), mime_type);
                media_file.modified = metadata.modified().unwrap_or(std::time::SystemTime::now());
                
                // Store in database
                let file_id = database.store_media_file(&media_file).await?;
                media_file.id = Some(file_id);
                
                // Add to in-memory cache
                files.push(media_file);
                
                info!("Renamed media file: {} -> {}", from.display(), to.display());
                
                // Increment update ID to notify DLNA clients
                increment_content_update_id(app_state);
            }
        }
    }
    
    Ok(())
}

/// Start SSDP service with platform abstraction
async fn start_ssdp_service(app_state: AppState) -> anyhow::Result<()> {
    info!("Starting SSDP discovery service...");
    
    // Start SSDP service using existing implementation
    ssdp::run_ssdp_service(app_state)
        .context("Failed to start SSDP service")?;
    
    info!("SSDP discovery service started successfully");
    Ok(())
}

/// Start HTTP server with proper error handling
async fn start_http_server(app_state: AppState) -> anyhow::Result<()> {
    info!("Starting HTTP server...");
    
    let config = app_state.config.clone();
    
    // Create the Axum web server
    let app = web::create_router(app_state);
    
    // Parse server interface address
    let interface_addr = if config.server.interface == "0.0.0.0" || config.server.interface.is_empty() {
        "0.0.0.0".parse().unwrap()
    } else {
        config.server.interface.parse()
            .with_context(|| format!("Invalid server interface address: {}", config.server.interface))?
    };
    
    let addr = SocketAddr::new(interface_addr, config.server.port);
    
    info!("Server UUID: {}", config.server.uuid);
    info!("Server name: {}", config.server.name);
    info!("Listening on http://{}", addr);
    
    // Attempt to bind to the address
    let listener = tokio::net::TcpListener::bind(addr).await
        .with_context(|| format!("Failed to bind to address: {}", addr))?;
    
    info!("HTTP server started successfully");
    
    // Start the server
    axum::serve(listener, app.into_make_service())
        .await
        .context("HTTP server failed")?;
    
    Ok(())
}

///
// Perform platform-specific security checks and permission requests
async fn perform_security_checks(platform_info: &PlatformInfo) -> anyhow::Result<()> {
    info!("Performing platform-specific security checks...");
    
    match platform_info.os_type {
        platform::OsType::Windows => {
            perform_windows_security_checks(platform_info).await?;
        }
        platform::OsType::MacOS => {
            perform_macos_security_checks(platform_info).await?;
        }
        platform::OsType::Linux => {
            perform_linux_security_checks(platform_info).await?;
        }
    }
    
    info!("Platform security checks completed successfully");
    Ok(())
}

/// Perform Windows-specific security checks
async fn perform_windows_security_checks(_platform_info: &PlatformInfo) -> anyhow::Result<()> {
    info!("Performing Windows security checks...");
    
    // Check if running with administrator privileges
    let is_elevated = check_windows_elevation().await?;
    if is_elevated {
        info!("Running with administrator privileges");
    } else {
        info!("Running without administrator privileges");
        
        // Check if we need privileged ports
        if needs_privileged_ports() {
            warn!("Application may need administrator privileges for ports < 1024");
            warn!("Consider running as administrator or using alternative ports");
        }
    }
    
    // Check Windows Firewall status
    if let Ok(firewall_enabled) = check_windows_firewall().await {
        if firewall_enabled {
            info!("Windows Firewall is enabled");
            info!("You may need to allow VuIO through the firewall");
        } else {
            info!("Windows Firewall is disabled");
        }
    } else {
        warn!("Could not determine Windows Firewall status");
    }
    
    // Check Windows Defender status
    if let Ok(defender_enabled) = check_windows_defender().await {
        if defender_enabled {
            info!("Windows Defender is active");
            info!("Real-time protection may scan media files during serving");
        }
    } else {
        warn!("Could not determine Windows Defender status");
    }
    
    Ok(())
}

/// Perform macOS-specific security checks
async fn perform_macos_security_checks(_platform_info: &PlatformInfo) -> anyhow::Result<()> {
    info!("Performing macOS security checks...");
    
    // Check if running with sudo
    let is_elevated = check_macos_elevation().await?;
    if is_elevated {
        warn!("Running with elevated privileges (sudo)");
        warn!("Consider running without sudo for better security");
    } else {
        info!("Running with normal user privileges");
    }
    
    // Check macOS Application Firewall
    if let Ok(firewall_enabled) = check_macos_firewall().await {
        if firewall_enabled {
            info!("macOS Application Firewall is enabled");
            info!("System may prompt for network access permissions");
        } else {
            info!("macOS Application Firewall is disabled");
        }
    } else {
        warn!("Could not determine macOS Application Firewall status");
    }
    
    // Check Gatekeeper status
    if let Ok(gatekeeper_enabled) = check_macos_gatekeeper().await {
        if gatekeeper_enabled {
            info!("Gatekeeper is enabled - application security is enforced");
        } else {
            warn!("Gatekeeper is disabled - reduced security");
        }
    } else {
        warn!("Could not determine Gatekeeper status");
    }
    
    // Check System Integrity Protection (SIP)
    if let Ok(sip_enabled) = check_macos_sip().await {
        if sip_enabled {
            info!("System Integrity Protection (SIP) is enabled");
        } else {
            warn!("System Integrity Protection (SIP) is disabled");
        }
    } else {
        warn!("Could not determine SIP status");
    }
    
    Ok(())
}

/// Perform Linux-specific security checks
async fn perform_linux_security_checks(_platform_info: &PlatformInfo) -> anyhow::Result<()> {
    info!("Performing Linux security checks...");
    
    // Check if running as root
    let is_root = check_linux_root().await?;
    if is_root {
        warn!("Running as root user");
        warn!("Consider running as a non-root user for better security");
        warn!("Use capabilities or systemd for privileged port access");
    } else {
        info!("Running as non-root user");
        
        // Check capabilities for privileged ports
        if needs_privileged_ports() {
            if let Ok(has_net_bind) = check_linux_capabilities().await {
                if has_net_bind {
                    info!("CAP_NET_BIND_SERVICE capability available for privileged ports");
                } else {
                    warn!("No capability for privileged ports - may need root or systemd");
                }
            }
        }
    }
    
    // Check SELinux status
    if let Ok(selinux_status) = check_selinux_status().await {
        match selinux_status.as_str() {
            "Enforcing" => {
                info!("SELinux is in enforcing mode");
                warn!("SELinux policies may restrict network and file access");
            }
            "Permissive" => {
                info!("SELinux is in permissive mode");
                info!("SELinux violations will be logged but not enforced");
            }
            "Disabled" => {
                info!("SELinux is disabled");
            }
            _ => {
                warn!("Unknown SELinux status: {}", selinux_status);
            }
        }
    } else {
        info!("SELinux not detected or not available");
    }
    
    // Check AppArmor status
    if let Ok(apparmor_enabled) = check_apparmor_status().await {
        if apparmor_enabled {
            info!("AppArmor is enabled");
            warn!("AppArmor profiles may restrict application behavior");
        } else {
            info!("AppArmor is not active");
        }
    } else {
        info!("AppArmor not detected or not available");
    }
    
    // Check firewall status (iptables/ufw/firewalld)
    if let Ok(firewall_info) = check_linux_firewall().await {
        if !firewall_info.is_empty() {
            info!("Firewall detected: {}", firewall_info);
            warn!("Firewall rules may block network connections");
        } else {
            info!("No active firewall detected");
        }
    } else {
        warn!("Could not determine firewall status");
    }
    
    Ok(())
}

/// Check if the application needs privileged ports (< 1024)
fn needs_privileged_ports() -> bool {
    // Check if SSDP port 1900 is needed (it's > 1024, so not privileged)
    // But we might want to bind to port 80 for HTTP in some configurations
    false // For now, we don't need privileged ports
}

/// Windows-specific security check functions
async fn check_windows_elevation() -> anyhow::Result<bool> {
    // Check if running with administrator privileges
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        
        let output = Command::new("net")
            .args(&["session"])
            .output()
            .context("Failed to check Windows elevation")?;
        
        // If the command succeeds, we likely have admin privileges
        Ok(output.status.success())
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}

async fn check_windows_firewall() -> anyhow::Result<bool> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        
        let output = Command::new("netsh")
            .args(&["advfirewall", "show", "allprofiles", "state"])
            .output()
            .context("Failed to check Windows Firewall status")?;
        
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            Ok(output_str.contains("ON"))
        } else {
            Ok(false)
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}

async fn check_windows_defender() -> anyhow::Result<bool> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        
        let output = Command::new("powershell")
            .args(&["-Command", "Get-MpComputerStatus | Select-Object -ExpandProperty RealTimeProtectionEnabled"])
            .output()
            .context("Failed to check Windows Defender status")?;
        
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            Ok(output_str.trim() == "True")
        } else {
            Ok(false)
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}

/// macOS-specific security check functions
async fn check_macos_elevation() -> anyhow::Result<bool> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        
        let output = Command::new("id")
            .args(&["-u"])
            .output()
            .context("Failed to check macOS elevation")?;
        
        if output.status.success() {
            let uid_str = String::from_utf8_lossy(&output.stdout);
            let uid: u32 = uid_str.trim().parse().unwrap_or(1000);
            Ok(uid == 0)
        } else {
            Ok(false)
        }
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        Ok(false)
    }
}

async fn check_macos_firewall() -> anyhow::Result<bool> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        
        let output = Command::new("defaults")
            .args(&["read", "/Library/Preferences/com.apple.alf", "globalstate"])
            .output()
            .context("Failed to check macOS firewall status")?;
        
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            let state: u32 = output_str.trim().parse().unwrap_or(0);
            Ok(state > 0) // 0 = off, 1 = on for specific services, 2 = on for essential services
        } else {
            Ok(false)
        }
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        Ok(false)
    }
}

async fn check_macos_gatekeeper() -> anyhow::Result<bool> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        
        let output = Command::new("spctl")
            .args(&["--status"])
            .output()
            .context("Failed to check Gatekeeper status")?;
        
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            Ok(output_str.contains("assessments enabled"))
        } else {
            Ok(false)
        }
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        Ok(false)
    }
}

async fn check_macos_sip() -> anyhow::Result<bool> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        
        let output = Command::new("csrutil")
            .args(&["status"])
            .output()
            .context("Failed to check SIP status")?;
        
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            Ok(output_str.contains("enabled"))
        } else {
            Ok(false)
        }
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        Ok(false)
    }
}

/// Linux-specific security check functions
async fn check_linux_root() -> anyhow::Result<bool> {
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        
        let output = Command::new("id")
            .args(&["-u"])
            .output()
            .context("Failed to check Linux user ID")?;
        
        if output.status.success() {
            let uid_str = String::from_utf8_lossy(&output.stdout);
            let uid: u32 = uid_str.trim().parse().unwrap_or(1000);
            Ok(uid == 0)
        } else {
            Ok(false)
        }
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        Ok(false)
    }
}

async fn check_linux_capabilities() -> anyhow::Result<bool> {
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        
        let output = Command::new("capsh")
            .args(&["--print"])
            .output()
            .context("Failed to check Linux capabilities")?;
        
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            Ok(output_str.contains("cap_net_bind_service"))
        } else {
            Ok(false)
        }
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        Ok(false)
    }
}

async fn check_selinux_status() -> anyhow::Result<String> {
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        
        let output = Command::new("getenforce")
            .output()
            .context("Failed to check SELinux status")?;
        
        if output.status.success() {
            let status = String::from_utf8_lossy(&output.stdout);
            Ok(status.trim().to_string())
        } else {
            Err(anyhow::anyhow!("SELinux not available"))
        }
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        Err(anyhow::anyhow!("SELinux not available on this platform"))
    }
}

async fn check_apparmor_status() -> anyhow::Result<bool> {
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        
        let output = Command::new("aa-status")
            .output()
            .context("Failed to check AppArmor status")?;
        
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            Ok(output_str.contains("profiles are loaded"))
        } else {
            Ok(false)
        }
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        Ok(false)
    }
}

async fn check_linux_firewall() -> anyhow::Result<String> {
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        
        // Check for ufw
        if let Ok(output) = Command::new("ufw").args(&["status"]).output() {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                if output_str.contains("Status: active") {
                    return Ok("ufw (active)".to_string());
                }
            }
        }
        
        // Check for firewalld
        if let Ok(output) = Command::new("firewall-cmd").args(&["--state"]).output() {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                if output_str.trim() == "running" {
                    return Ok("firewalld (running)".to_string());
                }
            }
        }
        
        // Check for iptables
        if let Ok(output) = Command::new("iptables").args(&["-L", "-n"]).output() {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                if !output_str.is_empty() && !output_str.contains("Chain INPUT (policy ACCEPT)") {
                    return Ok("iptables (configured)".to_string());
                }
            }
        }
        
        Ok(String::new())
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        Ok(String::new())
    }
}

/// Perform graceful shutdown with proper cleanup of all resources
async fn perform_graceful_shutdown(
    database: Arc<dyn DatabaseManager>,
    file_watcher: Arc<CrossPlatformWatcher>,
) -> anyhow::Result<()> {
    info!("Starting graceful shutdown sequence...");
    
    // Step 1: Stop file system monitoring
    info!("Stopping file system monitoring...");
    if let Err(e) = file_watcher.stop_watching().await {
        warn!("Failed to stop file system watcher cleanly: {}", e);
    } else {
        info!("File system monitoring stopped");
    }
    
    // Step 2: Flush any pending database operations
    info!("Flushing database operations...");
    if let Err(e) = flush_database_operations(&database).await {
        warn!("Failed to flush database operations: {}", e);
    } else {
        info!("Database operations flushed");
    }
    
    // Step 3: Create final database backup if enabled
    info!("Creating shutdown backup...");
    if let Err(e) = create_shutdown_backup(&database).await {
        warn!("Failed to create shutdown backup: {}", e);
    } else {
        info!("Shutdown backup created");
    }
    
    // Step 4: Vacuum database for optimization
    info!("Optimizing database...");
    if let Err(e) = database.vacuum().await {
        warn!("Failed to vacuum database: {}", e);
    } else {
        info!("Database optimized");
    }
    
    // Step 5: Log final statistics
    if let Ok(stats) = database.get_stats().await {
        info!("Final database statistics:");
        info!("  - Total media files: {}", stats.total_files);
        info!("  - Total media size: {} bytes", stats.total_size);
        info!("  - Database file size: {} bytes", stats.database_size);
    }
    
    info!("Graceful shutdown sequence completed");
    Ok(())
}

/// Flush any pending database operations
async fn flush_database_operations(database: &Arc<dyn DatabaseManager>) -> anyhow::Result<()> {
    // Check database health one final time
    let health = database.check_and_repair().await
        .context("Failed to perform final database health check")?;
    
    if !health.is_healthy {
        warn!("Database health issues detected during shutdown:");
        for issue in &health.issues {
            match issue.severity {
                database::IssueSeverity::Critical => error!("  CRITICAL: {}", issue.description),
                database::IssueSeverity::Error => error!("  ERROR: {}", issue.description),
                database::IssueSeverity::Warning => warn!("  WARNING: {}", issue.description),
                database::IssueSeverity::Info => info!("  INFO: {}", issue.description),
            }
        }
    }
    
    Ok(())
}

/// Create a backup during shutdown if backup is enabled
async fn create_shutdown_backup(database: &Arc<dyn DatabaseManager>) -> anyhow::Result<()> {
    // Create backup with timestamp
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let backup_name = format!("vuio_shutdown_backup_{}.db", timestamp);
    
    // Use platform-appropriate backup directory
    let platform_config = crate::platform::config::PlatformConfig::for_current_platform();
    let backup_dir = platform_config.database_dir.join("backups");
    
    // Ensure backup directory exists
    tokio::fs::create_dir_all(&backup_dir).await
        .context("Failed to create backup directory")?;
    
    let backup_path = backup_dir.join(backup_name);
    
    database.create_backup(&backup_path).await
        .context("Failed to create shutdown backup")?;
    
    info!("Shutdown backup created at: {}", backup_path.display());
    
    // Clean up old backups (keep only last 5)
    if let Err(e) = cleanup_old_backups(&backup_dir).await {
        warn!("Failed to clean up old backups: {}", e);
    }
    
    Ok(())
}

/// Clean up old backup files, keeping only the most recent ones
async fn cleanup_old_backups(backup_dir: &std::path::Path) -> anyhow::Result<()> {
    let mut entries = tokio::fs::read_dir(backup_dir).await?;
    let mut backup_files = Vec::new();
    
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "db") {
            if let Ok(metadata) = entry.metadata().await {
                backup_files.push((path, metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)));
            }
        }
    }
    
    // Sort by modification time, newest first
    backup_files.sort_by(|a, b| b.1.cmp(&a.1));
    
    // Keep only the 5 most recent backups
    const MAX_BACKUPS: usize = 5;
    if backup_files.len() > MAX_BACKUPS {
        for (old_backup, _) in backup_files.iter().skip(MAX_BACKUPS) {
            if let Err(e) = tokio::fs::remove_file(old_backup).await {
                warn!("Failed to remove old backup {}: {}", old_backup.display(), e);
            } else {
                info!("Removed old backup: {}", old_backup.display());
            }
        }
    }
    
    Ok(())
}
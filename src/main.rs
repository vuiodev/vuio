use anyhow::Context;
use vuio::{
    config::AppConfig,
    database::{self, DatabaseManager, SqliteDatabase},
    logging, media,
    platform::{self, filesystem::{create_platform_filesystem_manager, create_platform_path_normalizer}, PlatformInfo},
    ssdp,
    state::AppState,
    watcher::{CrossPlatformWatcher, FileSystemEvent, FileSystemWatcher},
    web,
};
use std::{net::SocketAddr, sync::Arc, path::PathBuf};
use tracing::{debug, error, info, warn};

/// Wait for shutdown signals (Ctrl+C, SIGTERM, etc.)
/// Supports graceful shutdown on first signal, force quit on second signal
async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        
        let mut sigterm = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");
        
        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM signal");
            }
            _ = sigint.recv() => {
                info!("Received SIGINT signal (Ctrl+C)");
                
                // Set up a second signal handler for force quit
                tokio::spawn(async move {
                    if sigint.recv().await.is_some() {
                        warn!("Received second SIGINT signal - forcing immediate exit");
                        std::process::exit(1);
                    }
                });
            }
        }
    }
    
    #[cfg(not(unix))]
    {
        // On Windows, only Ctrl+C is available
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!("Failed to listen for Ctrl+C signal: {}", e);
        } else {
            info!("Received Ctrl+C signal");
            
            // Set up a second signal handler for force quit
            tokio::spawn(async {
                if tokio::signal::ctrl_c().await.is_ok() {
                    warn!("Received second Ctrl+C signal - forcing immediate exit");
                    std::process::exit(1);
                }
            });
        }
    }
}

/// Parse early command line arguments to get debug flag and config file path
/// This is needed before logging initialization
fn parse_early_args() -> (bool, Option<String>) {
    use clap::Parser;
    
    #[derive(Parser, Debug)]
    #[command(author, version, about, long_about = None)]
    struct EarlyArgs {
        /// The directory containing media files to serve
        media_dir: Option<String>,

        /// Additional media directories to serve (can be used multiple times)
        #[arg(long = "media-dir", action = clap::ArgAction::Append)]
        additional_media_dirs: Vec<String>,

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
    
    // Parse args, but ignore errors since we'll parse them again later
    match EarlyArgs::try_parse() {
        Ok(args) => (args.debug, args.config),
        Err(_) => (false, None), // Default to no debug and no config file
    }
}

#[tokio::main(flavor = "current_thread")]
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

    // Security checks removed for faster startup

    // Load or create configuration with platform-specific defaults
    let config = match initialize_configuration(&platform_info, config_file_path).await {
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

    // Perform initial media scan (database only, no in-memory cache)
    if let Err(e) = perform_initial_media_scan(&config, &database).await {
        error!("Failed to perform initial media scan: {}", e);
        return Err(e);
    }

    // Create shared application state
    let filesystem_manager: Arc<dyn vuio::platform::filesystem::FileSystemManager> = 
        Arc::from(create_platform_filesystem_manager());
    let app_state = AppState {
        config: config.clone(),
        database: database.clone(),
        platform_info: platform_info.clone(),
        filesystem_manager,
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
    ).await?;

    // Start SSDP discovery service with platform abstraction
    if let Err(e) = start_ssdp_service(app_state.clone()).await {
        error!("Failed to start SSDP service: {}", e);
        return Err(e);
    }

    // Start the HTTP server as a background task
    let server_handle = match start_http_server_task(app_state).await {
        Ok(handle) => handle,
        Err(e) => {
            error!("Failed to start HTTP server: {}", e);
            return Err(e);
        }
    };

    // Wait for shutdown signal and cleanup
    tokio::select! {
        _ = wait_for_shutdown_signal() => {
            info!("Received shutdown signal");
        }
        _ = adaptation_handle => {
            warn!("Platform adaptation service stopped unexpectedly");
        }
        result = server_handle => {
            match result {
                Ok(Ok(())) => info!("HTTP server stopped gracefully"),
                Ok(Err(e)) => error!("HTTP server failed: {}", e),
                Err(e) => error!("HTTP server task panicked: {}", e),
            }
        }
    }

    // Graceful shutdown with timeout
    info!("Shutting down gracefully...");
    
    // Give services a reasonable time to shut down gracefully
    let shutdown_timeout = std::time::Duration::from_secs(5);
    let shutdown_start = std::time::Instant::now();
    
    // Wait for any remaining tasks to complete or timeout
    tokio::select! {
        _ = tokio::time::sleep(shutdown_timeout) => {
            warn!("Shutdown timeout reached after {:?}, forcing exit", shutdown_timeout);
        }
        _ = async {
            // Allow some time for background tasks to clean up
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        } => {
            let shutdown_duration = shutdown_start.elapsed();
            info!("Shutdown completed successfully in {:?}", shutdown_duration);
        }
    }
    
    Ok(())
}

/// Start platform adaptation services for runtime detection and adaptation
async fn start_platform_adaptation(
    platform_info: Arc<PlatformInfo>,
    config: Arc<AppConfig>,
    database: Arc<dyn DatabaseManager>,
) -> anyhow::Result<tokio::task::JoinHandle<()>> {
    info!("Starting platform adaptation services...");
    
    let config_clone = config.clone();
    let database_clone = database.clone();
    
    let handle = tokio::spawn(async move {
        // Disable frequent network checks to avoid repeated interface detection
        // Network interfaces are detected once at startup and that's sufficient for most use cases
        let mut config_check_interval = tokio::time::interval(std::time::Duration::from_secs(60));
        
        loop {
            tokio::select! {
                _ = config_check_interval.tick() => {
                    if let Err(e) = check_and_reload_configuration(&config_clone, &database_clone).await {
                        warn!("Configuration reload check failed: {}", e);
                    }
                }
                _ = wait_for_shutdown_signal() => {
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
async fn check_and_adapt_network_changes(_platform_info: &Arc<PlatformInfo>) -> anyhow::Result<()> {
    // Skip network interface re-detection to avoid repeated logging
    // Network interface detection should only happen once at startup
    debug!("Skipping network interface re-detection - using startup configuration");
    Ok(())
}

/// Check for configuration changes and reload if necessary
async fn check_and_reload_configuration(
    config: &Arc<AppConfig>,
    database: &Arc<dyn DatabaseManager>,
) -> anyhow::Result<()> {
    let config_path = AppConfig::get_platform_config_file_path();
    
    // Check if configuration file has been modified
    if let Ok(metadata) = tokio::fs::metadata(&config_path).await {
        let modified = metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        
        // Skip change detection for files modified in the last 2 minutes to avoid
        // detecting newly created config files as "changed"
        let two_minutes_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(120);
        let five_minutes_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(300);
        
        // Only check for changes if file was modified between 2-5 minutes ago
        if modified > five_minutes_ago && modified < two_minutes_ago {
            info!("Configuration file may have been modified, checking for changes...");
            
            match AppConfig::load_from_file(&config_path) {
                Ok(new_config) => {
                    if let Err(e) = handle_configuration_changes(config, &new_config, database).await {
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
            info!("Directory changes detected - database has been updated");
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
    if old_config.network.interface_selection != new_config.network.interface_selection {
        info!("Network configuration changed");
        changes_detected = true;
        
        if old_config.network.interface_selection != new_config.network.interface_selection {
            info!("Network interface selection changed: {:?} -> {:?}", 
                old_config.network.interface_selection, new_config.network.interface_selection);
        }
        
        // TODO: Restart SSDP service with new configuration
        warn!("Network configuration changes require service restart to take effect");
    }
    
    // Check for server configuration changes
    if old_config.server.port != new_config.server.port ||
       old_config.server.interface != new_config.server.interface ||
       old_config.server.ip != new_config.server.ip {
        info!("Server configuration changed");
        changes_detected = true;
        
        if old_config.server.port != new_config.server.port {
            info!("Server port changed: {} -> {}", old_config.server.port, new_config.server.port);
        }
        
        if old_config.server.interface != new_config.server.interface {
            info!("Server interface changed: {} -> {}", old_config.server.interface, new_config.server.interface);
        }
        
        if old_config.server.ip != new_config.server.ip {
            info!("Server IP changed: {:?} -> {:?}", old_config.server.ip, new_config.server.ip);
        }
        
        warn!("Server configuration changes require application restart to take effect");
    }
    
    if changes_detected {
        info!("Configuration changes processed successfully");
    }
    
    Ok(())
}


/// Detect platform information with comprehensive diagnostics and error reporting
/// This function should only be called once at startup to avoid repeated interface detection
async fn detect_platform_with_diagnostics() -> anyhow::Result<PlatformInfo> {
    info!("Detecting platform information...");
    
    let platform_info = PlatformInfo::detect().await
        .context("Failed to detect platform information")?;
    
    // Log comprehensive platform information
    info!("Platform: {} {}", platform_info.os_type.display_name(), platform_info.version);
    info!("Architecture: {}", std::env::consts::ARCH);
    
    info!("Platform capabilities:");
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
    
    
    Ok(platform_info)
}

/// Initialize configuration with platform-specific defaults and validation
async fn initialize_configuration(_platform_info: &PlatformInfo, config_file_path: Option<String>) -> anyhow::Result<AppConfig> {
    info!("Initializing configuration...");
    
    // Check if running in Docker container
    if AppConfig::is_running_in_docker() {
        info!("Docker environment detected - using environment variables for configuration");
        let config = AppConfig::from_env()
            .context("Failed to load configuration from environment variables")?;
        
        info!("Configuration initialized from environment variables");
        info!("Server will listen on: {}:{}", config.server.interface, config.server.port);
        info!("SSDP will use hardcoded port: 1900");
        info!("Monitoring {} director(ies) for media files", config.media.directories.len());
        
        for (i, dir) in config.media.directories.iter().enumerate() {
            info!("  {}. {} (recursive: {})", i + 1, dir.path, dir.recursive);
        }
        
        return Ok(config);
    }
    
    // Native platform mode - use config files
    info!("Native platform detected - using configuration files");
    
    // Use provided config file path if available, otherwise use platform default
    let config_path = if let Some(path) = config_file_path {
        let custom_path = PathBuf::from(path);
        if !custom_path.exists() {
            anyhow::bail!("Configuration file does not exist: {}", custom_path.display());
        }
        info!("Using custom configuration file: {}", custom_path.display());
        custom_path
    } else {
        let default_path = AppConfig::get_platform_config_file_path();
        info!("Using default configuration file path: {}", default_path.display());
        default_path
    };
    
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
            info!("SSDP will use hardcoded port: 1900");
            info!("Monitoring {} director(ies) for media files", config.media.directories.len());
            
            for (i, dir) in config.media.directories.iter().enumerate() {
                info!("  {}. {} (recursive: {})", i + 1, dir.path, dir.recursive);
            }
            
            return Ok(config);
        }
        Err(_) => {
            // Fall back to file-based configuration
        }
    }
    
    // Load or create configuration from file
    let (mut config, _config_was_created) = if config_path.exists() {
        info!("Loading existing configuration from: {}", config_path.display());
        (AppConfig::load_or_create(&config_path)?, false)
    } else {
        info!("No configuration file found, creating default configuration");
        let default_config = AppConfig::default_for_platform();
        
        // Create the config file with platform defaults
        default_config.save_to_file(&config_path)
            .with_context(|| format!("Failed to create default configuration file at: {}", config_path.display()))?;
        
        info!("Created default configuration file at: {}", config_path.display());
        (default_config, true)
    };
    
    // Apply platform-specific defaults and validation
    config.apply_platform_defaults()
        .context("Failed to apply platform-specific defaults")?;
    
    config.validate_for_platform()
        .context("Configuration validation failed")?;
    
    info!("Configuration initialized successfully");
    info!("Server will listen on: {}:{}", config.server.interface, config.server.port);
    info!("SSDP will use hardcoded port: 1900");
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

/// Validate cached files and remove any that no longer exist on disk (streaming version)
async fn validate_and_cleanup_deleted_files(
    database: Arc<dyn DatabaseManager>,
) -> anyhow::Result<()> {
    use futures_util::StreamExt;
    
    info!("Validating cached media files (streaming)...");
    
    let mut stream = database.stream_all_media_files();
    let mut removed_count = 0;
    let mut total_checked = 0;
    
    while let Some(media_file_result) = stream.next().await {
        let media_file = media_file_result
            .context("Failed to read media file from database stream")?;
        
        total_checked += 1;
        
        if !media_file.path.exists() {
            info!("Removing deleted file from database: {}", media_file.path.display());
            if database.remove_media_file(&media_file.path).await? {
                removed_count += 1;
            }
        }
        
        // Log progress every 1000 files to show we're making progress
        if total_checked % 1000 == 0 {
            info!("Validated {} files, removed {} deleted files so far", total_checked, removed_count);
        }
    }
    
    if removed_count > 0 {
        info!("Cleaned up {} deleted files from database (checked {} total)", removed_count, total_checked);
    } else {
        info!("All {} cached files are still present on disk", total_checked);
    }

    Ok(())
}

/// Perform initial media scan, using database cache when possible
async fn perform_initial_media_scan(config: &AppConfig, database: &Arc<dyn DatabaseManager>) -> anyhow::Result<()> {
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

        // Validate files to catch any that were deleted while app was offline
        if config.media.cleanup_deleted_files {
            validate_and_cleanup_deleted_files(database.clone()).await?;
        }
        
        Ok(())
    } else {
        info!("Skipping full scan (scan on startup disabled)");

        // Validate that cached files still exist on disk and remove any that don't (if enabled)
        if config.media.cleanup_deleted_files {
            validate_and_cleanup_deleted_files(database.clone()).await?;
        }

        Ok(())
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
                        
                        // Files are already stored in database by the scanner
                        
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
                
                // File is already stored in database
                
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
                
                // File is already updated in database
                
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
            
            // Use efficient path prefix query to find files in the deleted directory
            let path_normalizer = create_platform_path_normalizer();
            let canonical_prefix = match path_normalizer.to_canonical(&path) {
                Ok(canonical) => canonical,
                Err(e) => {
                    warn!("Error normalizing deleted path {}: {}", path.display(), e);
                    return Ok(());
                }
            };
            
            let files_in_deleted_path = match database.get_files_with_path_prefix(&canonical_prefix).await {
                Ok(files) => files,
                Err(e) => {
                    warn!("Error getting files with path prefix '{}': {}", canonical_prefix, e);
                    return Ok(());
                }
            };
            
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
            }
            
            if total_removed > 0 {
                info!("Total cleanup: {} files removed from database for path: {}", 
                      total_removed, path.display());
                
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
                
                // Use efficient path prefix query to find files in the old directory path
                let path_normalizer = create_platform_path_normalizer();
                let canonical_from_prefix = path_normalizer.to_canonical(&from)?;
                let files_in_old_path = database.get_files_with_path_prefix(&canonical_from_prefix).await?;
                
                if !files_in_old_path.is_empty() {
                    info!("Updating {} media files for renamed directory", files_in_old_path.len());
                    
                    // Remove old files from database
                    for old_file in &files_in_old_path {
                        database.remove_media_file(&old_file.path).await?;
                    }
                    
                    // Scan the new directory location
                    let scanner = media::MediaScanner::with_database(database.clone());
                    match scanner.scan_directory_recursive(&to).await {
                        Ok(scan_result) => {
                            info!("Rescanned renamed directory {}: {}", to.display(), scan_result.summary());
                            
                            // Files are already stored in database by the scanner
                            
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
                
                // Remove old file from database
                database.remove_media_file(&from).await?;
                
                // Create MediaFile record for new location
                let metadata = tokio::fs::metadata(&to).await?;
                let mime_type = media::get_mime_type(&to);
                let mut media_file = database::MediaFile::new(to.clone(), metadata.len(), mime_type);
                media_file.modified = metadata.modified().unwrap_or(std::time::SystemTime::now());
                
                // Store in database
                let file_id = database.store_media_file(&media_file).await?;
                media_file.id = Some(file_id);
                
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

/// Start HTTP server as a background task with proper error handling
async fn start_http_server_task(app_state: AppState) -> anyhow::Result<tokio::task::JoinHandle<anyhow::Result<()>>> {
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
    
    // Spawn the server as a background task
    let handle = tokio::spawn(async move {
        axum::serve(listener, app.into_make_service())
            .await
            .context("HTTP server failed")
    });
    
    Ok(handle)
}



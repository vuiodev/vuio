use anyhow::Context;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tracing::{debug, error, info, warn};
use vuio::{
    config::{
        AppConfig, ConfigChangeEvent, ConfigManager, MonitoredDirectoryConfig, ValidationMode,
    },
    database::{self, DatabaseManager},
    logging, media,
    platform::{
        self,
        filesystem::{create_platform_filesystem_manager, create_platform_path_normalizer},
        PlatformInfo,
    },
    ssdp,
    state::AppState,
    watcher::{CrossPlatformWatcher, FileSystemEvent, FileSystemWatcher},
    web,
};

/// Wait for shutdown signals (Ctrl+C, SIGTERM, etc.)
/// Supports graceful shutdown on first signal, force quit on second signal
async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut sigterm =
            signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
        let mut sigint =
            signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");

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

/// Struct containing parsed command line arguments
struct CommandLineArgs {
    debug: bool,
    config_path: Option<String>,
    log_file: Option<String>,
    log_level: Option<String>,
    config_override: Option<AppConfig>,
}

/// Parse command line arguments once and return configuration overrides
/// This consolidates argument parsing into a single operation
fn parse_args_once() -> anyhow::Result<CommandLineArgs> {
    use clap::Parser;

    #[derive(Parser, Debug)]
    #[command(author, version, about, long_about = None)]
    struct Args {
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

        /// Path to log file
        #[arg(long = "log-file")]
        log_file: Option<String>,

        /// Set log level (off, error, warn, info, debug, trace)
        #[arg(long = "log-level")]
        log_level: Option<String>,
    }

    let args = Args::parse();

    // If no media directories provided, return early args only
    if args.media_dir.is_none() && args.additional_media_dirs.is_empty() {
        return Ok(CommandLineArgs {
            debug: args.debug,
            config_path: args.config,
            log_file: args.log_file,
            log_level: args.log_level,
            config_override: None,
        });
    }

    // Build configuration from command line arguments
    let mut config_override = AppConfig::default_for_platform();

    // Apply command line overrides
    if let Some(port) = args.port {
        config_override.server.port = port;
    }

    if args.name != "VuIO Server" {
        config_override.server.name = args.name;
    }

    // Build media directories from arguments
    let mut media_directories = vec![];

    // Add primary media directory if provided
    if let Some(media_dir_str) = &args.media_dir {
        let media_dir = std::path::PathBuf::from(media_dir_str);
        if !media_dir.exists() || !media_dir.is_dir() {
            tracing::warn!(
                "Media directory does not exist or is not a directory: {}",
                media_dir.display()
            );
        }
        media_directories.push(MonitoredDirectoryConfig {
            path: media_dir.to_string_lossy().to_string(),
            recursive: true,
            extensions: None,
            exclude_patterns: None,
            validation_mode: ValidationMode::Warn,
        });
    }

    // Add additional media directories
    for additional_dir_str in &args.additional_media_dirs {
        let additional_dir = std::path::PathBuf::from(additional_dir_str);
        if !additional_dir.exists() || !additional_dir.is_dir() {
            tracing::warn!(
                "Additional media directory does not exist or is not a directory: {}",
                additional_dir.display()
            );
        }
        media_directories.push(MonitoredDirectoryConfig {
            path: additional_dir.to_string_lossy().to_string(),
            recursive: true,
            extensions: None,
            exclude_patterns: None,
            validation_mode: ValidationMode::Warn,
        });
    }

    config_override.media.directories = media_directories;

    Ok(CommandLineArgs {
        debug: args.debug,
        config_path: args.config,
        log_file: args.log_file,
        log_level: args.log_level,
        config_override: Some(config_override),
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    // Parse command line arguments once and get configuration overrides
    let cli_args = parse_args_once().context("Failed to parse command line arguments")?;

    // Initialize logging with options
    let log_file_path = cli_args.log_file.as_ref().map(PathBuf::from);
    logging::init_logging_with_options(
        cli_args.log_level.as_deref(),
        log_file_path.clone(),
        cli_args.debug,
    )
    .context("Failed to initialize logging")?;

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

    // Initialize configuration manager with file watching
    let config_manager = match initialize_config_manager(
        &platform_info,
        cli_args.config_path,
        cli_args.config_override,
    )
    .await
    {
        Ok(manager) => Arc::new(manager),
        Err(e) => {
            error!("Failed to initialize configuration manager: {}", e);
            return Err(e);
        }
    };

    // Get the current configuration
    let config = Arc::new(config_manager.get_config().await);

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

    // Perform initial playlist file scan (after media scan so referenced files exist)
    if let Err(e) = perform_initial_playlist_scan(&config, &database).await {
        // Log warning but don't fail startup - playlists are not critical
        warn!("Failed to scan playlist files: {}", e);
    }

    // Create shared application state
    let filesystem_manager: Arc<dyn vuio::platform::filesystem::FileSystemManager> =
        Arc::from(create_platform_filesystem_manager());
    let resolved_log_file =
        log_file_path.unwrap_or_else(|| vuio::config::AppConfig::get_platform_log_file_path());
    let app_state = AppState {
        config: config.clone(),
        media_directories: Arc::new(tokio::sync::RwLock::new(config.media.directories.clone())),
        database: database.clone(),
        platform_info: platform_info.clone(),
        filesystem_manager,
        content_update_id: Arc::new(std::sync::atomic::AtomicU32::new(1)),
        web_metrics: Arc::new(vuio::web::handlers::WebHandlerMetrics::new()),
        bookmarks: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        log_file_path: resolved_log_file,
        browse_cache: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        mcp_clients: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_monitors: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_casts: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        discovered_tvs: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        upnp_subscriptions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
    };

    {
        let subscriptions = app_state.upnp_subscriptions.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let now = std::time::Instant::now();
                subscriptions
                    .lock()
                    .await
                    .retain(|_, subscription| subscription.expires_at > now);
            }
        });
    }

    // Start file system monitoring
    if let Err(e) = start_file_monitoring(file_watcher.clone(), app_state.clone()).await {
        warn!("Failed to start file system monitoring: {}", e);
        warn!("Continuing without real-time file monitoring");
    }

    // Start runtime platform adaptation services
    let adaptation_handle = start_platform_adaptation(
        platform_info.clone(),
        config_manager.clone(),
        file_watcher.clone(),
        app_state.clone(),
    )
    .await?;

    // Start atomic application statistics monitoring
    let monitoring_handle = start_atomic_monitoring(database.clone()).await?;

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

    // Determine if console logging is verbose
    let is_rust_log_set = std::env::var("RUST_LOG").is_ok();
    let in_docker = AppConfig::is_running_in_docker();
    let console_is_verbose =
        cli_args.debug || is_rust_log_set || cli_args.log_level.is_some() || in_docker;

    if !console_is_verbose {
        let display_ip =
            if config.server.interface == "0.0.0.0" || config.server.interface.is_empty() {
                if let Some(primary) = platform_info.get_primary_interface() {
                    primary.ip_address.to_string()
                } else {
                    "127.0.0.1".to_string()
                }
            } else {
                config.server.interface.clone()
            };
        let web_url = format!("http://{}:{}", display_ip, config.server.port);
        let db_path = config.get_database_path().with_extension("redb");

        let name_str = config.server.name.clone();
        let display_name = if name_str.len() > 41 {
            format!("...{}", &name_str[name_str.len() - 38..])
        } else {
            name_str
        };

        let display_url = if web_url.len() > 41 {
            format!("...{}", &web_url[web_url.len() - 38..])
        } else {
            web_url
        };

        let db_path_str = db_path.to_string_lossy().to_string();
        let display_db_path = if db_path_str.len() > 41 {
            format!("...{}", &db_path_str[db_path_str.len() - 38..])
        } else {
            db_path_str
        };

        println!("┌────────────────────────────────────────────────────────┐");
        println!("│  VuIO Media Server                                     │");
        println!("├────────────────────────────────────────────────────────┤");
        println!("│  Name:       {:<41} │", display_name);
        println!("│  Version:    {:<41} │", env!("CARGO_PKG_VERSION"));
        println!("│  Status:     Online & Streaming                        │");
        println!("│  Address:    {:<41} │", display_url);
        println!("│  SSDP:       Active on port 1900                       │");
        println!("│  Database:   {:<41} │", display_db_path);
        println!("│                                                        │");
        println!("│  Monitored Directories:                                │");
        if config.media.directories.is_empty() {
            println!("│    (none configured)                                   │");
        } else {
            for dir in &config.media.directories {
                let path_str = &dir.path;
                let display_path = if path_str.len() > 49 {
                    format!("...{}", &path_str[path_str.len() - 46..])
                } else {
                    path_str.clone()
                };
                println!("│    • {:<49} │", display_path);
            }
        }
        println!("│                                                        │");
        println!("│  Press Ctrl+C to stop the server safely.               │");
        println!("└────────────────────────────────────────────────────────┘");
    }

    // Wait for shutdown signal and cleanup
    tokio::select! {
        _ = wait_for_shutdown_signal() => {
            info!("Received shutdown signal");
        }
        _ = adaptation_handle => {
            warn!("Platform adaptation service stopped unexpectedly");
        }
        _ = monitoring_handle => {
            warn!("Atomic monitoring service stopped unexpectedly");
        }
        result = server_handle => {
            match result {
                Ok(Ok(())) => info!("HTTP server stopped gracefully"),
                Ok(Err(e)) => error!("HTTP server failed: {}", e),
                Err(e) => error!("HTTP server task panicked: {}", e),
            }
        }
    }

    // Graceful shutdown with ReDB atomic state persistence
    info!("Shutting down gracefully...");

    // Perform atomic state persistence before shutdown
    if let Err(e) = perform_graceful_shutdown(&database).await {
        error!("Error during graceful shutdown: {}", e);
    }

    // Give services a reasonable time to shut down gracefully
    let shutdown_timeout = std::time::Duration::from_secs(10); // Increased timeout for database persistence
    let shutdown_start = std::time::Instant::now();

    // Wait for any remaining tasks to complete or timeout
    tokio::select! {
        _ = tokio::time::sleep(shutdown_timeout) => {
            warn!("Shutdown timeout reached after {:?}, forcing exit", shutdown_timeout);
        }
        _ = async {
            // Allow time for ReDB database to persist state
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        } => {
            let shutdown_duration = shutdown_start.elapsed();
            info!("Shutdown completed successfully in {:?}", shutdown_duration);
        }
    }

    Ok(())
}

/// Start platform adaptation services for runtime detection and adaptation
async fn start_platform_adaptation(
    _platform_info: Arc<PlatformInfo>,
    config_manager: Arc<ConfigManager>,
    watcher: Arc<CrossPlatformWatcher>,
    app_state: AppState,
) -> anyhow::Result<tokio::task::JoinHandle<()>> {
    info!("Starting platform adaptation services...");

    let config_manager_clone = config_manager.clone();
    let handle = tokio::spawn(async move {
        // Subscribe to configuration changes from ConfigManager
        let mut config_changes = config_manager_clone.subscribe_to_changes();

        loop {
            tokio::select! {
                config_event = config_changes.recv() => {
                    match config_event {
                        Ok(event) => {
                            info!("Configuration change detected: {:?}", event);
                            match event {
                                ConfigChangeEvent::Reloaded(new_config) => {
                                    *app_state.media_directories.write().await =
                                        new_config.media.directories.clone();
                                    increment_content_update_id(&app_state).await;
                                }
                                ConfigChangeEvent::DirectoriesChanged { added, removed, .. } => {
                                    for path in &removed {
                                        if let Err(error) = watcher.remove_watch_path(path).await {
                                            warn!("Failed to remove watch {}: {}", path.display(), error);
                                        }
                                        if let Err(error) = app_state
                                            .database
                                            .remove_derived_content_by_source(path)
                                            .await
                                        {
                                            error!("Failed to remove derived content for removed root {}: {}", path.display(), error);
                                        }
                                        if let Err(error) = app_state.database.remove_media_under_path(path).await {
                                            error!("Failed to remove indexed content for removed root {}: {}", path.display(), error);
                                        }
                                    }
                                    for path in &added {
                                        if path.is_dir() {
                                            if let Err(error) = watcher.add_watch_path(path).await {
                                                warn!("Failed to add watch {}: {}", path.display(), error);
                                            } else {
                                                let scanner = media::MediaScanner::with_database(app_state.database.clone());
                                                if let Err(error) = scanner.scan_directory_recursive(path).await {
                                                    warn!("Failed to scan added root {}: {}", path.display(), error);
                                                }
                                            }
                                        }
                                    }
                                    // If a removed parent root contained another root that remains
                                    // configured, repopulate that nested root immediately.
                                    let active_roots = app_state
                                        .media_directories
                                        .read()
                                        .await
                                        .iter()
                                        .map(|directory| PathBuf::from(&directory.path))
                                        .collect::<Vec<_>>();
                                    for root in active_roots {
                                        if root.is_dir()
                                            && removed.iter().any(|removed| root.starts_with(removed))
                                        {
                                            let scanner = media::MediaScanner::with_database(app_state.database.clone());
                                            if let Err(error) = scanner.scan_directory_recursive(&root).await {
                                                warn!("Failed to restore nested root {}: {}", root.display(), error);
                                            }
                                        }
                                    }
                                    increment_content_update_id(&app_state).await;
                                }
                                ConfigChangeEvent::NetworkChanged { .. } => {}
                            }
                        }
                        Err(e) => {
                            warn!("Configuration change subscription error: {}", e);
                        }
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

    info!("Platform adaptation services started with configuration change monitoring");
    Ok(handle)
}

/// Detect platform information with comprehensive diagnostics and error reporting
/// This function should only be called once at startup to avoid repeated interface detection
async fn detect_platform_with_diagnostics() -> anyhow::Result<PlatformInfo> {
    info!("Detecting platform information...");

    let platform_info = PlatformInfo::detect()
        .await
        .context("Failed to detect platform information")?;

    // Log comprehensive platform information
    info!(
        "Platform: {} {}",
        platform_info.os_type.display_name(),
        platform_info.version
    );
    info!("Architecture: {}", std::env::consts::ARCH);

    info!("Platform capabilities:");
    info!(
        "  - Case-sensitive filesystem: {}",
        platform_info.capabilities.case_sensitive_fs
    );

    // Log network interface information
    if platform_info.network_interfaces.is_empty() {
        warn!("No network interfaces detected - network functionality may be limited");
    } else {
        info!(
            "Detected {} network interface(s):",
            platform_info.network_interfaces.len()
        );
        for interface in &platform_info.network_interfaces {
            info!(
                "  - {} ({}): {} - Up: {}, Multicast: {}",
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
            info!(
                "Primary network interface: {} ({})",
                primary_interface.name, primary_interface.ip_address
            );
        } else {
            warn!("No suitable primary network interface found for DLNA operations");
        }
    }

    Ok(platform_info)
}

/// Initialize configuration manager with platform-specific defaults, file loading, and command line overrides
async fn initialize_config_manager(
    _platform_info: &PlatformInfo,
    config_file_path: Option<String>,
    config_override: Option<AppConfig>,
) -> anyhow::Result<ConfigManager> {
    info!("Initializing configuration...");

    // Check if running in Docker container
    if AppConfig::is_running_in_docker() {
        info!("Docker environment detected - using environment variables for configuration");
        let config = AppConfig::from_env()
            .context("Failed to load configuration from environment variables")?;

        info!("Configuration initialized from environment variables");
        info!(
            "Server will listen on: {}:{}",
            config.server.interface, config.server.port
        );
        info!("SSDP will use hardcoded port: 1900");
        info!(
            "Monitoring {} director(ies) for media files",
            config.media.directories.len()
        );

        for (i, dir) in config.media.directories.iter().enumerate() {
            info!("  {}. {} (recursive: {})", i + 1, dir.path, dir.recursive);
        }

        // Create a temporary config file for the ConfigManager
        let temp_config_path = std::env::temp_dir().join("vuio_docker_config.toml");
        config
            .save_to_file(&temp_config_path)
            .context("Failed to save Docker configuration to temporary file")?;

        // Create ConfigManager without file watching for Docker (config is static from env vars)
        let config_manager = ConfigManager::new(&temp_config_path)
            .context("Failed to create ConfigManager for Docker configuration")?;

        return Ok(config_manager);
    }

    // Native platform mode - use config files with command line overrides
    info!("Native platform detected - using configuration files");

    // If we have command line overrides, use them directly
    if let Some(override_config) = config_override {
        info!("Using configuration from command line arguments");

        // Apply platform-specific defaults for any missing values
        let mut config = override_config;
        config
            .apply_platform_defaults()
            .context("Failed to apply platform-specific defaults to command line configuration")?;

        // Validate the final configuration
        config
            .validate_for_platform()
            .context("Command line configuration validation failed")?;

        info!("Configuration validated successfully");
        info!(
            "Server will listen on: {}:{}",
            config.server.interface, config.server.port
        );
        info!("SSDP will use hardcoded port: 1900");
        info!(
            "Monitoring {} director(ies) for media files",
            config.media.directories.len()
        );

        for (i, dir) in config.media.directories.iter().enumerate() {
            info!("  {}. {} (recursive: {})", i + 1, dir.path, dir.recursive);
        }

        // Create a temporary config file for the ConfigManager
        let temp_config_path = std::env::temp_dir().join("vuio_cmdline_config.toml");
        config
            .save_to_file(&temp_config_path)
            .context("Failed to save command line configuration to temporary file")?;

        // Create ConfigManager without file watching for command line overrides
        let config_manager = ConfigManager::new(&temp_config_path)
            .context("Failed to create ConfigManager for command line configuration")?;

        return Ok(config_manager);
    }

    // Use provided config file path if available, otherwise use platform default
    let config_path = if let Some(path) = config_file_path {
        let custom_path = PathBuf::from(path);
        if !custom_path.exists() {
            anyhow::bail!(
                "Configuration file does not exist: {}",
                custom_path.display()
            );
        }
        info!("Using custom configuration file: {}", custom_path.display());
        custom_path
    } else {
        let default_path = AppConfig::get_platform_config_file_path();
        info!(
            "Using default configuration file path: {}",
            default_path.display()
        );
        default_path
    };

    // Create ConfigManager with file watching for configuration files
    let config_manager = if config_path.exists() {
        info!(
            "Loading existing configuration from: {}",
            config_path.display()
        );
        ConfigManager::new_with_watching(&config_path)
            .await
            .context("Failed to create ConfigManager with file watching")?
    } else {
        info!("No configuration file found, creating default configuration");
        let default_config = AppConfig::default_for_platform();

        // Apply platform-specific defaults and validation
        let mut config = default_config;
        config
            .apply_platform_defaults()
            .context("Failed to apply platform-specific defaults")?;

        config
            .validate_for_platform()
            .context("Configuration validation failed")?;

        // Create the config file with platform defaults
        config.save_to_file(&config_path).with_context(|| {
            format!(
                "Failed to create default configuration file at: {}",
                config_path.display()
            )
        })?;

        info!(
            "Created default configuration file at: {}",
            config_path.display()
        );

        // Create ConfigManager with file watching
        ConfigManager::new_with_watching(&config_path)
            .await
            .context("Failed to create ConfigManager with file watching")?
    };

    // Get the current configuration for logging
    let config = config_manager.get_config().await;

    info!("Configuration initialized successfully with file watching enabled");
    info!(
        "Server will listen on: {}:{}",
        config.server.interface, config.server.port
    );
    info!("SSDP will use hardcoded port: 1900");
    info!(
        "Monitoring {} director(ies) for media files",
        config.media.directories.len()
    );

    for (i, dir) in config.media.directories.iter().enumerate() {
        info!("  {}. {} (recursive: {})", i + 1, dir.path, dir.recursive);
    }

    Ok(config_manager)
}

fn preserve_failed_database(db_path: &std::path::Path) -> anyhow::Result<Option<PathBuf>> {
    if !db_path.exists() {
        return Ok(None);
    }
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.fZ");
    let backup_path = db_path.with_extension(format!("failed-{timestamp}.redb"));
    std::fs::rename(db_path, &backup_path).with_context(|| {
        format!(
            "Failed to preserve unusable database as {}",
            backup_path.display()
        )
    })?;
    warn!("Preserved unusable database at {}", backup_path.display());
    Ok(Some(backup_path))
}

/// Initialize database manager with health checks and recovery
async fn initialize_database(config: &AppConfig) -> anyhow::Result<database::redb::RedbDatabase> {
    info!("Initializing Redb database...");

    let db_path = config.get_database_path();
    // Change extension from .db to .redb
    let db_path = db_path.with_extension("redb");
    info!("Database path: {}", db_path.display());

    // Create Redb database manager
    let mut database = match database::redb::RedbDatabase::new(db_path.clone()).await {
        Ok(database) => database,
        Err(error) => {
            error!("Failed to open ReDB database: {}", error);
            preserve_failed_database(&db_path)?;
            database::redb::RedbDatabase::new(db_path.clone())
                .await
                .context("Failed to create replacement ReDB database")?
        }
    };

    // Initialize database schema
    if let Err(error) = database.initialize().await {
        error!("Failed to initialize ReDB schema: {}", error);
        drop(database);
        preserve_failed_database(&db_path)?;
        database = database::redb::RedbDatabase::new(db_path.clone())
            .await
            .context("Failed to create replacement ReDB database")?;
        database
            .initialize()
            .await
            .context("Failed to initialize replacement database schema")?;
    }

    // Perform health check
    info!("Performing database health check...");
    let health = match database.check_and_repair().await {
        Ok(health) => health,
        Err(repair_error) => {
            error!("Database index rebuild failed: {}", repair_error);
            drop(database);
            preserve_failed_database(&db_path)?;
            database = database::redb::RedbDatabase::new(db_path.clone())
                .await
                .context("Failed to create replacement ReDB database")?;
            database.initialize().await?;
            database
                .check_and_repair()
                .await
                .context("Replacement ReDB database failed initial index construction")?
        }
    };

    if !health.is_healthy || !health.issues.is_empty() {
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
    let stats = database
        .get_stats()
        .await
        .context("Failed to get database statistics")?;

    info!("Database statistics:");
    info!("  - Total media files: {}", stats.total_files);
    info!("  - Total media size: {} bytes", stats.total_size);
    info!("  - Database file size: {} bytes", stats.database_size);

    // Vacuum database if configured
    if config.database.vacuum_on_startup {
        info!("Performing database compaction...");
        database
            .vacuum()
            .await
            .context("Failed to compact database")?;
        info!("Database compaction completed");
    }

    info!("Database initialized successfully");
    Ok(database)
}

/// Initialize file system watcher for real-time media monitoring
async fn initialize_file_watcher(
    config: &AppConfig,
    _database: Arc<dyn DatabaseManager>,
) -> anyhow::Result<CrossPlatformWatcher> {
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
            warn!(
                "Monitored directory does not exist or is not a directory: {}",
                dir_config.path
            );
        }
    }

    if valid_directories.is_empty() {
        warn!("No valid directories to monitor - file watching will be disabled");
        return Ok(watcher);
    }

    info!(
        "File system watcher initialized for {} directories",
        valid_directories.len()
    );
    Ok(watcher)
}

/// Validate cached files and remove any that no longer exist on disk
///
/// Uses two-phase approach to avoid RwLock deadlock:
/// 1. Stream all files and collect paths to delete (read lock)
/// 2. Drop stream, then bulk delete (write lock)
async fn validate_and_cleanup_deleted_files(
    database: Arc<dyn DatabaseManager>,
    monitored_roots: &[PathBuf],
) -> anyhow::Result<usize> {
    use futures_util::StreamExt;

    info!("Validating cached media files...");

    // Phase 1: Collect paths to delete (holds read lock)
    let mut paths_to_delete = Vec::new();
    let mut total_checked = 0;

    {
        let mut stream = database.stream_all_media_files();

        while let Some(media_file_result) = stream.next().await {
            let media_file =
                media_file_result.context("Failed to read media file from database stream")?;

            total_checked += 1;

            let unavailable_root = monitored_roots
                .iter()
                .any(|root| media_file.path.starts_with(root) && !root.is_dir());
            if !unavailable_root && !media_file.path.exists() {
                paths_to_delete.push(media_file.path.clone());
            }

            // Log progress every 1000 files
            if total_checked % 1000 == 0 {
                info!("Validated {} files so far...", total_checked);
            }
        }
    } // Stream dropped here, read lock released

    // Phase 2: Bulk delete (acquires write lock)
    let removed_count = paths_to_delete.len();
    if !paths_to_delete.is_empty() {
        info!("Removing {} deleted files from database", removed_count);
        database.bulk_remove_media_files(&paths_to_delete).await?;
    }

    if removed_count > 0 {
        info!(
            "Cleaned up {} deleted files from database (checked {} total)",
            removed_count, total_checked
        );
    } else {
        info!(
            "All {} cached files are still present on disk",
            total_checked
        );
    }

    Ok(removed_count)
}

async fn hide_unavailable_media_roots(
    database: &Arc<dyn DatabaseManager>,
    roots: &[PathBuf],
) -> anyhow::Result<usize> {
    let mut removed = 0;
    for root in roots {
        if root.is_dir() {
            continue;
        }
        removed += database.remove_derived_content_by_source(root).await?;
        removed += database.remove_media_under_path(root).await?.removed_files;
    }
    Ok(removed)
}

/// Perform initial media scan, using database cache when possible
async fn perform_initial_media_scan(
    config: &AppConfig,
    database: &Arc<dyn DatabaseManager>,
) -> anyhow::Result<()> {
    info!("Performing initial media scan...");

    let configured_roots = config
        .media
        .directories
        .iter()
        .map(|directory| PathBuf::from(&directory.path))
        .collect::<Vec<_>>();
    let hidden = hide_unavailable_media_roots(database, &configured_roots).await?;
    if hidden > 0 {
        info!(
            "Removed {} cached items belonging to unavailable media roots",
            hidden
        );
    }

    let database_is_empty = database.get_stats().await?.total_files == 0;
    if config.media.scan_on_startup || database_is_empty {
        if database_is_empty && !config.media.scan_on_startup {
            warn!("Database is empty; forcing a full media scan despite scan_on_startup=false");
        }
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
                scanner
                    .scan_directory_recursive(&dir_path)
                    .await
                    .with_context(|| {
                        format!("Failed to recursively scan directory: {}", dir_config.path)
                    })?
            } else {
                scanner
                    .scan_directory(&dir_path)
                    .await
                    .with_context(|| format!("Failed to scan directory: {}", dir_config.path))?
            };

            info!(
                "Scan of {} completed: {}",
                dir_path.display(),
                scan_result.summary()
            );
            if !scan_result.errors.is_empty() {
                // FIX: Iterate over a reference to avoid moving scan_result.errors
                for err in &scan_result.errors {
                    warn!("Scan error in {}: {}", err.path.display(), err.error);
                }
            }
            total_changes += scan_result.total_changes();
            total_files_scanned += scan_result.total_scanned;
        }

        info!(
            "Initial media scan completed - total files scanned: {}, total changes: {}",
            total_files_scanned, total_changes
        );

        // Validate files to catch any that were deleted while app was offline
        if config.media.cleanup_deleted_files {
            let roots: Vec<_> = config
                .media
                .directories
                .iter()
                .map(|d| PathBuf::from(&d.path))
                .collect();
            validate_and_cleanup_deleted_files(database.clone(), &roots).await?;
        }

        Ok(())
    } else {
        info!("Skipping full scan (scan on startup disabled)");

        // Validate that cached files still exist on disk and remove any that don't (if enabled)
        if config.media.cleanup_deleted_files {
            let roots: Vec<_> = config
                .media
                .directories
                .iter()
                .map(|d| PathBuf::from(&d.path))
                .collect();
            validate_and_cleanup_deleted_files(database.clone(), &roots).await?;
        }

        Ok(())
    }
}

/// Perform initial playlist file scan
async fn perform_initial_playlist_scan(
    config: &AppConfig,
    database: &Arc<dyn DatabaseManager>,
) -> anyhow::Result<()> {
    if !config.media.scan_playlists {
        info!("Playlist scanning disabled in configuration");
        return Ok(());
    }

    info!("Scanning for playlist files...");

    let mut total_playlists = 0;

    for dir_config in &config.media.directories {
        let dir_path = std::path::PathBuf::from(&dir_config.path);

        if !dir_path.exists() {
            warn!(
                "Media directory does not exist, skipping playlist scan: {}",
                dir_config.path
            );
            continue;
        }

        info!("Scanning for playlists in: {}", dir_config.path);

        let playlist_ids = if dir_config.recursive {
            database
                .scan_and_import_playlists_recursive(&dir_path)
                .await
                .with_context(|| format!("Failed to scan playlists in: {}", dir_config.path))?
        } else {
            database
                .scan_and_import_playlists(&dir_path)
                .await
                .with_context(|| format!("Failed to scan playlists in: {}", dir_config.path))?
        };

        if !playlist_ids.is_empty() {
            info!(
                "Imported {} playlist(s) from {}",
                playlist_ids.len(),
                dir_config.path
            );
        }

        total_playlists += playlist_ids.len();
    }

    if total_playlists > 0 {
        info!(
            "Playlist scan completed: {} playlist(s) imported",
            total_playlists
        );
    } else {
        info!("Playlist scan completed: no playlist files found");
    }

    Ok(())
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
    let all_directories: Vec<std::path::PathBuf> = app_state
        .config
        .media
        .directories
        .iter()
        .map(|dir| std::path::PathBuf::from(&dir.path))
        .collect();
    let directories: Vec<_> = all_directories
        .iter()
        .filter(|path| path.is_dir())
        .cloned()
        .collect();

    if directories.is_empty() {
        warn!("No media roots are currently available; recovery probing remains active");
    }

    info!("Starting to monitor {} directories:", directories.len());
    for (i, dir) in directories.iter().enumerate() {
        info!("  {}: {}", i + 1, dir.display());
    }

    // Start watching directories
    watcher
        .start_watching(&directories)
        .await
        .context("Failed to start watching directories")?;

    info!("File system watcher successfully started for all directories");

    // Get event receiver
    let mut event_receiver = watcher.get_event_receiver();

    // Spawn task to handle file system events
    let app_state_clone = app_state.clone();
    let watcher_clone = watcher.clone();

    tokio::spawn(async move {
        info!("File system event handler started");

        let mut reconciliation = tokio::time::interval(std::time::Duration::from_secs(300));
        reconciliation.tick().await;
        loop {
            tokio::select! {
                event = event_receiver.recv() => {
                    let Some(event) = event else { break; };
                    if let Err(e) = handle_file_system_event(event, &app_state_clone).await {
                        error!("Failed to handle file system event; reconciling all roots: {}", e);
                        let configured_directories = app_state_clone
                            .media_directories
                            .read()
                            .await
                            .iter()
                            .map(|directory| PathBuf::from(&directory.path))
                            .collect::<Vec<_>>();
                        for root in &configured_directories {
                            if root.is_dir() {
                                let scanner = media::MediaScanner::with_database(app_state_clone.database.clone());
                                if scanner.scan_directory_recursive(root).await.is_ok() {
                                    increment_content_update_id(&app_state_clone).await;
                                }
                            }
                        }
                    }
                }
                _ = reconciliation.tick() => {
                    let configured_directories = app_state_clone
                        .media_directories
                        .read()
                        .await
                        .iter()
                        .map(|directory| PathBuf::from(&directory.path))
                        .collect::<Vec<_>>();
                    match hide_unavailable_media_roots(
                        &app_state_clone.database,
                        &configured_directories,
                    )
                    .await
                    {
                        Ok(removed) if removed > 0 => {
                            increment_content_update_id(&app_state_clone).await
                        }
                        Ok(_) => {}
                        Err(error) => {
                            error!("Failed to hide unavailable media roots: {}", error)
                        }
                    }
                    for root in &configured_directories {
                        if root.is_dir() && !watcher_clone.is_watching(root).await {
                            if let Err(error) = watcher_clone.add_watch_path(root).await {
                                error!("Failed to restore watch for {}: {}", root.display(), error);
                            } else {
                                let scanner = media::MediaScanner::with_database(app_state_clone.database.clone());
                                if scanner.scan_directory_recursive(root).await.is_ok() {
                                    increment_content_update_id(&app_state_clone).await;
                                }
                            }
                        }
                    }
                    for root in watcher_clone.take_dirty_roots() {
                        if root.is_dir() {
                            let scanner = media::MediaScanner::with_database(app_state_clone.database.clone());
                            match scanner.scan_directory_recursive(&root).await {
                                Ok(result) if result.total_changes() > 0 => increment_content_update_id(&app_state_clone).await,
                                Ok(_) => {}
                                Err(error) => error!("Dirty-root reconciliation failed for {}: {}", root.display(), error),
                            }
                        }
                    }
                    match validate_and_cleanup_deleted_files(app_state_clone.database.clone(), &configured_directories).await {
                        Ok(removed) if removed > 0 => increment_content_update_id(&app_state_clone).await,
                        Ok(_) => {}
                        Err(error) => error!("Periodic missing-file reconciliation failed: {}", error),
                    }
                }
            }
        }

        warn!("File system event handler stopped");
    });

    info!(
        "File system monitoring started for {} directories",
        directories.len()
    );
    Ok(())
}

/// Increment the content update ID to notify DLNA clients of changes
async fn increment_content_update_id(app_state: &AppState) {
    let old_id = app_state
        .content_update_id
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let new_id = old_id.wrapping_add(1);
    app_state.browse_cache.lock().await.clear();
    info!(
        "Content update ID incremented from {} to {}",
        old_id, new_id
    );

    let state = app_state.clone();
    tokio::spawn(async move {
        vuio::web::handlers::notify_content_change(&state, new_id).await;
    });
}

/// Atomic application statistics for monitoring
#[derive(Debug)]
struct AtomicAppStats {
    files_processed: AtomicU64,
    directories_scanned: AtomicU64,
    events_handled: AtomicU64,
    errors_encountered: AtomicU64,
    last_activity: AtomicU64,
}

impl AtomicAppStats {
    fn new() -> Self {
        let initial_secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            files_processed: AtomicU64::new(0),
            directories_scanned: AtomicU64::new(0),
            events_handled: AtomicU64::new(0),
            errors_encountered: AtomicU64::new(0),
            last_activity: AtomicU64::new(initial_secs),
        }
    }

    fn update_last_activity(&self) {
        let secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_activity.store(secs, Ordering::Relaxed);
    }

    fn record_files_processed(&self, count: u64) {
        self.files_processed.fetch_add(count, Ordering::Relaxed);
        self.update_last_activity();
    }

    fn record_directory_scanned(&self) {
        self.directories_scanned.fetch_add(1, Ordering::Relaxed);
        self.update_last_activity();
    }

    fn record_event_handled(&self) {
        self.events_handled.fetch_add(1, Ordering::Relaxed);
        self.update_last_activity();
    }

    fn record_error(&self) {
        self.errors_encountered.fetch_add(1, Ordering::Relaxed);
        self.update_last_activity();
    }

    fn get_stats(&self) -> (u64, u64, u64, u64, SystemTime) {
        let last_secs = self.last_activity.load(Ordering::Relaxed);
        let last_activity = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(last_secs);
        (
            self.files_processed.load(Ordering::Relaxed),
            self.directories_scanned.load(Ordering::Relaxed),
            self.events_handled.load(Ordering::Relaxed),
            self.errors_encountered.load(Ordering::Relaxed),
            last_activity,
        )
    }
}

// Global atomic application statistics
static APP_STATS: std::sync::OnceLock<AtomicAppStats> = std::sync::OnceLock::new();

fn get_app_stats() -> &'static AtomicAppStats {
    APP_STATS.get_or_init(|| AtomicAppStats::new())
}

/// Handle individual file system events with ReDB bulk operations
async fn handle_file_system_event(
    event: FileSystemEvent,
    app_state: &AppState,
) -> anyhow::Result<()> {
    let database = &app_state.database;
    let stats = get_app_stats();

    // Record event handling with atomic counter
    stats.record_event_handled();

    match event {
        FileSystemEvent::Created(path) => {
            // Check if this is a directory or a file
            if path.is_dir() {
                info!("Directory created: {}", path.display());

                // Scan the new directory for media files using ReDB bulk operations
                let scanner = media::MediaScanner::with_database(database.clone());
                match scanner.scan_directory_recursive(&path).await {
                    Ok(scan_result) => {
                        info!(
                            "Scanned new directory {}: {}",
                            path.display(),
                            scan_result.summary()
                        );

                        // Files are already stored in database by the scanner using bulk operations

                        // Record atomic statistics
                        stats.record_directory_scanned();
                        stats.record_files_processed(scan_result.new_files.len() as u64);

                        info!("Added {} media files from new directory using ReDB bulk operations: {}", 
                              scan_result.new_files.len(), path.display());

                        // Increment update ID to notify DLNA clients
                        if !scan_result.new_files.is_empty() {
                            increment_content_update_id(app_state).await;
                        }
                    }
                    Err(e) => {
                        error!("Failed to scan new directory {}: {}", path.display(), e);
                    }
                }
            } else {
                // Handle individual media file creation using bulk operations (single-item batch)
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
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let mime_type = crate::platform::filesystem::get_mime_type_for_extension(ext);
                let mut media_file =
                    database::MediaFile::new(path.clone(), metadata.len(), mime_type);
                media_file.modified = metadata.modified().unwrap_or(std::time::SystemTime::now());

                // Store in database using ReDB bulk operation (single-item batch for atomic consistency)
                let file_ids = database
                    .bulk_store_media_files(&[media_file.clone()])
                    .await?;
                if let Some(file_id) = file_ids.first() {
                    media_file.id = Some(*file_id);
                }

                // Record atomic statistics
                stats.record_files_processed(1);

                info!("Added new media file to ReDB database: {}", path.display());

                // Increment update ID to notify DLNA clients
                increment_content_update_id(app_state).await;
            }
        }

        FileSystemEvent::Modified(path) => {
            info!("Media file modified: {}", path.display());

            // Update database record using ReDB bulk operation
            if let Some(mut existing_file) = database.get_file_by_path(&path).await? {
                let metadata = tokio::fs::metadata(&path).await?;
                existing_file.size = metadata.len();
                existing_file.modified =
                    metadata.modified().unwrap_or(std::time::SystemTime::now());
                existing_file.updated_at = std::time::SystemTime::now();

                // Use ReDB bulk update operation (single-item batch for atomic consistency)
                database.bulk_update_media_files(&[existing_file]).await?;

                // Record atomic statistics
                stats.record_files_processed(1);

                info!("Updated media file in ReDB database: {}", path.display());

                // Increment update ID to notify DLNA clients
                increment_content_update_id(app_state).await;
            }
        }

        FileSystemEvent::Deleted { path, is_directory } => {
            info!("Path deleted: {}", path.display());
            let derived_removed = database.remove_derived_content_by_source(&path).await?;
            let summary = database
                .remove_media_under_path(&path)
                .await
                .map_err(|error| {
                    stats.record_error();
                    error
                })?;
            stats.record_files_processed(summary.removed_files as u64);
            info!(
                "Removed {} indexed files and {} derived items below deleted path {}",
                summary.removed_files,
                derived_removed,
                path.display()
            );
            // Publish empty/duplicate directory events because they can retire an
            // older browse generation. Known unrelated file deletions do not churn
            // the library revision unless they removed indexed/derived content.
            if is_directory != Some(false) || summary.removed_files > 0 || derived_removed > 0 {
                increment_content_update_id(app_state).await;
            }
        }

        FileSystemEvent::Renamed { from, to } => {
            info!("Path renamed: {} -> {}", from.display(), to.display());

            // Check if the destination is a directory or file
            if to.is_dir() {
                // Handle directory rename using ReDB bulk operations
                info!("Directory renamed: {} -> {}", from.display(), to.display());

                // Use efficient path prefix query to find files in the old directory path
                let path_normalizer = create_platform_path_normalizer();
                let canonical_from_prefix = path_normalizer.to_canonical(&from)?;
                let files_in_old_path = database
                    .get_files_with_path_prefix(&canonical_from_prefix)
                    .await?;

                if !files_in_old_path.is_empty() {
                    info!(
                        "Updating {} media files for renamed directory using ReDB bulk operations",
                        files_in_old_path.len()
                    );

                    // Collect paths for bulk removal
                    let old_paths: Vec<PathBuf> =
                        files_in_old_path.iter().map(|f| f.path.clone()).collect();

                    // Remove old files from database using ReDB bulk operation
                    let removed_count = database.bulk_remove_media_files(&old_paths).await?;
                    info!(
                        "ReDB bulk removal: {} files removed for renamed directory",
                        removed_count
                    );

                    // Scan the new directory location using ReDB bulk operations
                    let scanner = media::MediaScanner::with_database(database.clone());
                    match scanner.scan_directory_recursive(&to).await {
                        Ok(scan_result) => {
                            info!(
                                "Rescanned renamed directory {}: {}",
                                to.display(),
                                scan_result.summary()
                            );

                            // Files are already stored in database by the scanner using ReDB bulk operations

                            // Increment update ID to notify DLNA clients
                            increment_content_update_id(app_state).await;
                        }
                        Err(e) => {
                            error!("Failed to rescan renamed directory {}: {}", to.display(), e);
                        }
                    }
                }
            } else {
                // Handle individual file rename using ReDB bulk operations
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
                    debug!(
                        "Renamed file is not a media file, ignoring: {}",
                        to.display()
                    );
                    return Ok(());
                }

                // Remove old file and add new file using ReDB bulk operations for atomic consistency
                let removed_count = database.bulk_remove_media_files(&[from.clone()]).await?;

                if removed_count > 0 {
                    // Create MediaFile record for new location
                    let metadata = tokio::fs::metadata(&to).await?;
                    let ext = to.extension().and_then(|e| e.to_str()).unwrap_or("");
                    let mime_type = crate::platform::filesystem::get_mime_type_for_extension(ext);
                    let media_file =
                        database::MediaFile::new(to.clone(), metadata.len(), mime_type);

                    // Store in database using ReDB bulk operation
                    let _file_ids = database.bulk_store_media_files(&[media_file]).await?;

                    info!(
                        "Renamed media file using ReDB atomic operations: {} -> {}",
                        from.display(),
                        to.display()
                    );

                    // Increment update ID to notify DLNA clients
                    increment_content_update_id(app_state).await;
                } else {
                    warn!(
                        "Original file not found in database for rename: {}",
                        from.display()
                    );
                }
            }
        }
    }

    Ok(())
}

/// Perform graceful shutdown with ReDB atomic state persistence
async fn perform_graceful_shutdown(database: &Arc<dyn DatabaseManager>) -> anyhow::Result<()> {
    info!("Performing graceful shutdown with atomic state persistence...");

    let stats = get_app_stats();
    let (files_processed, directories_scanned, events_handled, errors_encountered, last_activity) =
        stats.get_stats();

    // Log final application statistics
    info!("Final application statistics:");
    info!("  - Files processed: {}", files_processed);
    info!("  - Directories scanned: {}", directories_scanned);
    info!("  - Events handled: {}", events_handled);
    info!("  - Errors encountered: {}", errors_encountered);
    info!("  - Last activity: {:?}", last_activity);

    // Ensure ReDB database persists all pending operations
    info!("Persisting ReDB database state...");

    // Get database statistics before shutdown
    match database.get_stats().await {
        Ok(db_stats) => {
            info!("Database statistics at shutdown:");
            info!("  - Total media files: {}", db_stats.total_files);
            info!("  - Total media size: {} bytes", db_stats.total_size);
            info!("  - Database file size: {} bytes", db_stats.database_size);
        }
        Err(e) => {
            warn!(
                "Could not retrieve database statistics during shutdown: {}",
                e
            );
        }
    }

    // Perform database vacuum if needed (this will also ensure all data is persisted)
    info!("Performing final database maintenance...");
    if let Err(e) = database.vacuum().await {
        warn!("Could not vacuum database during shutdown: {}", e);
    }

    info!("Graceful shutdown with atomic state persistence completed");
    Ok(())
}

/// Start atomic application statistics monitoring
async fn start_atomic_monitoring(
    database: Arc<dyn DatabaseManager>,
) -> anyhow::Result<tokio::task::JoinHandle<()>> {
    info!("Starting atomic application statistics monitoring...");

    let handle = tokio::spawn(async move {
        let stats = get_app_stats();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60)); // Monitor every minute

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let (files_processed, directories_scanned, events_handled, errors_encountered, last_activity) = stats.get_stats();

                    // Log periodic statistics
                    debug!("Atomic application statistics:");
                    debug!("  - Files processed: {}", files_processed);
                    debug!("  - Directories scanned: {}", directories_scanned);
                    debug!("  - Events handled: {}", events_handled);
                    debug!("  - Errors encountered: {}", errors_encountered);
                    debug!("  - Last activity: {:?}", last_activity);

                    // Get database statistics
                    if let Ok(db_stats) = database.get_stats().await {
                        debug!("ReDB database statistics:");
                        debug!("  - Total media files: {}", db_stats.total_files);
                        debug!("  - Total media size: {} bytes", db_stats.total_size);
                        debug!("  - Database file size: {} bytes", db_stats.database_size);
                    }

                    // Check for inactivity (no events in last 5 minutes)
                    if let Ok(elapsed) = last_activity.elapsed() {
                        if elapsed > std::time::Duration::from_secs(300) {
                            debug!("Application has been inactive for {:?}", elapsed);
                        }
                    }
                }
                _ = wait_for_shutdown_signal() => {
                    info!("Atomic monitoring service received shutdown signal");
                    break;
                }
            }
        }

        info!("Atomic monitoring service stopped");
    });

    info!("Atomic application statistics monitoring started");
    Ok(handle)
}

/// Start SSDP service with platform abstraction
async fn start_ssdp_service(app_state: AppState) -> anyhow::Result<()> {
    info!("Starting SSDP discovery service...");

    // Start SSDP service using existing implementation
    ssdp::run_ssdp_service(app_state).context("Failed to start SSDP service")?;

    info!("SSDP discovery service started successfully");
    Ok(())
}

/// Start HTTP server as a background task with proper error handling
async fn start_http_server_task(
    app_state: AppState,
) -> anyhow::Result<tokio::task::JoinHandle<anyhow::Result<()>>> {
    info!("Starting HTTP server...");

    let config = app_state.config.clone();

    // Create the Axum web server
    let app = web::create_router(app_state.clone());

    // Parse server interface address
    let interface_addr =
        if config.server.interface == "0.0.0.0" || config.server.interface.is_empty() {
            "0.0.0.0".parse().unwrap()
        } else {
            config.server.interface.parse().with_context(|| {
                format!(
                    "Invalid server interface address: {}",
                    config.server.interface
                )
            })?
        };

    let addr = SocketAddr::new(interface_addr, config.server.port);

    info!("Server UUID: {}", config.server.uuid);
    info!("Server name: {}", config.server.name);
    info!("Listening on http://{}", addr);

    // Attempt to bind to the address
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind to address: {}", addr))?;

    info!("HTTP server started successfully");

    // Helper to parse IP from location_url
    fn parse_ip_from_url_helper(url_str: &str) -> Option<String> {
        let without_scheme = url_str.split("://").nth(1)?;
        let host_port = without_scheme.split('/').next()?;
        let host = host_port.split(':').next()?;
        Some(host.to_string())
    }

    // Spawn background SSDP TV discovery cache refresher every 60s
    let state_clone = app_state.clone();
    tokio::spawn(async move {
        loop {
            if let Ok(tvs) = vuio::tv_control::discover_tvs().await {
                let mut cache = state_clone.discovered_tvs.lock().await;
                for tv in tvs {
                    if let Some(ip) = parse_ip_from_url_helper(&tv.location_url) {
                        cache.insert(ip, tv.friendly_name);
                    }
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });

    // Spawn the server as a background task
    let handle = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .context("HTTP server failed")
    });

    Ok(handle)
}

/// Start platform adaptation services for runtime detection and adaptation
async fn start_platform_adaptation<D: DatabaseManager + 'static>(
    _platform_info: Arc<PlatformInfo>,
    config_manager: Arc<ConfigManager>,
    watcher: Arc<CrossPlatformWatcher>,
    app_state: AppState<D>,
    cancellation: CancellationToken,
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
                                    let new_config = *new_config;
                                    let old_config = app_state.current_config();
                                    app_state.desired_config.store(Arc::new(new_config.clone()));
                                    let mut effective = (*old_config).clone();
                                    let mut errors = Vec::new();
                                    let mut pending = Vec::new();
                                    let mut rebind_http = false;

                                    // Listener rebinding is intentionally conservative: identity
                                    // fields hot-reload, while a bind-address change remains pending
                                    // until a replacement listener can be introduced safely.
                                    if old_config.server.port == new_config.server.port
                                        && old_config.server.interface == new_config.server.interface
                                    {
                                        effective.server = new_config.server.clone();
                                    } else {
                                        pending.extend(["server.port".to_owned(), "server.interface".to_owned()]);
                                        rebind_http = true;
                                    }
                                    effective.network = new_config.network.clone();

                                    let mut management = new_config.management.clone();
                                    if app_state.auth_forced {
                                        management.auth_enabled = true;
                                    }
                                    match crate::web::auth::AuthState::load(
                                        &management,
                                        config_manager_clone.get_config_path(),
                                    ) {
                                        Ok(auth) => {
                                            app_state.auth.store(Arc::new(auth));
                                            effective.management = management;
                                        }
                                        Err(error) => errors.push(format!("management: {error}")),
                                    }

                                    let discovery_config = crate::discovery::DiscoveryConfig {
                                        chromecast_enabled: new_config.cast.chromecast_enabled,
                                        airplay_enabled: new_config.cast.airplay_enabled,
                                        discovery_interval: std::time::Duration::from_secs(
                                            new_config.cast.discovery_interval_seconds.max(1),
                                        ),
                                    };
                                    app_state.discovery_service.reconfigure(discovery_config).await;
                                    effective.cast = new_config.cast.clone();

                                    let active_directories = new_config
                                        .media
                                        .directories
                                        .iter()
                                        .map(|directory| PathBuf::from(&directory.path))
                                        .filter(|path| path.is_dir())
                                        .collect::<Vec<_>>();
                                    let watcher_result = match (
                                        old_config.media.watch_for_changes,
                                        new_config.media.watch_for_changes,
                                    ) {
                                        (true, false) => watcher.stop_watching().await,
                                        (false, true) => watcher.start_watching(&active_directories).await,
                                        _ => Ok(()),
                                    };
                                    if let Err(error) = watcher_result {
                                        errors.push(format!("media watcher: {error}"));
                                    } else {
                                        watcher.set_policies(media::ScanPolicy::policies(&new_config));
                                        *app_state.media_directories.write().await =
                                            new_config.media.directories.clone();
                                        effective.media = new_config.media.clone();
                                    }

                                    if !old_config.database.backup_enabled
                                        && new_config.database.backup_enabled
                                    {
                                        if let Err(error) = create_lifecycle_backup(&app_state.database, &new_config).await {
                                            errors.push(format!("database backup: {error}"));
                                        } else {
                                            effective.database.backup_enabled = true;
                                        }
                                    } else {
                                        effective.database.backup_enabled = new_config.database.backup_enabled;
                                    }
                                    effective.database.compact_on_shutdown =
                                        new_config.database.compact_on_shutdown;
                                    if old_config.database.path != new_config.database.path {
                                        pending.push("database.path".to_owned());
                                    }
                                    if old_config.database.redb_cache_mb != new_config.database.redb_cache_mb {
                                        pending.push("database.redb_cache_mb".to_owned());
                                    }
                                    if old_config.database.vacuum_on_startup != new_config.database.vacuum_on_startup {
                                        pending.push("database.vacuum_on_startup".to_owned());
                                    }

                                    app_state.live_config.store(Arc::new(effective));
                                    *app_state.config_reload_errors.write().unwrap_or_else(|e| e.into_inner()) = errors;
                                    *app_state.pending_restart_fields.write().unwrap_or_else(|e| e.into_inner()) = pending;
                                    app_state.ssdp_reload_notify.notify_one();
                                    if rebind_http {
                                        app_state.http_rebind_notify.notify_one();
                                    }
                                    increment_content_update_id(&app_state).await;
                                }
                                ConfigChangeEvent::DirectoriesChanged { added, removed, modified } => {
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
                                        if let Err(error) = app_state.database.remove_root_availability(path).await {
                                            error!("Failed to remove availability state for removed root {}: {}", path.display(), error);
                                        }
                                    }
                                    for path in added.iter().chain(modified.iter()) {
                                        if path.is_dir() {
                                            if modified.contains(path) {
                                                let _ = watcher.remove_watch_path(path).await;
                                            }
                                            let current = app_state.current_config();
                                            let Some(root) = current.media.directories.iter().find(|root| Path::new(&root.path) == path) else {
                                                continue;
                                            };
                                            let policy = media::ScanPolicy::from_config(&current, root);
                                            if let Err(error) = watcher.add_watch_policy(policy.clone()).await {
                                                warn!("Failed to add watch {}: {}", path.display(), error);
                                            } else {
                                                let scanner = media::MediaScanner::with_database(app_state.database.clone());
                                                let scan = if root.recursive {
                                                    scanner.scan_directory_recursive_with_policy(&policy).await
                                                } else {
                                                    scanner.scan_directory_with_policy(&policy).await
                                                };
                                                if let Err(error) = scan {
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
                _ = cancellation.cancelled() => {
                    info!("Platform adaptation service received cancellation");
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
    cancellation: CancellationToken,
    background_tasks: tokio_util::task::TaskTracker,
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
        ConfigManager::new_with_watching(
            &config_path,
            cancellation.clone(),
            background_tasks.clone(),
        )
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
        ConfigManager::new_with_watching(&config_path, cancellation, background_tasks)
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
    let quarantine_id = uuid::Uuid::new_v4();
    let backup_path =
        db_path.with_extension(format!("failed-{timestamp}-{quarantine_id}.redb"));
    if backup_path.try_exists()? {
        anyhow::bail!(
            "Refusing to overwrite existing database quarantine file {}",
            backup_path.display()
        );
    }
    std::fs::rename(db_path, &backup_path).with_context(|| {
        format!(
            "Failed to preserve unusable database as {}",
            backup_path.display()
        )
    })?;
    warn!("Preserved unusable database at {}", backup_path.display());
    Ok(Some(backup_path))
}

fn discard_incompatible_database(db_path: &std::path::Path) -> anyhow::Result<()> {
    if db_path.exists() {
        std::fs::remove_file(db_path).with_context(|| {
            format!("Failed to remove incompatible database {}", db_path.display())
        })?;
        warn!("Removed incompatible alpha database {}", db_path.display());
    }
    Ok(())
}

/// Initialize database manager with health checks and recovery
async fn initialize_database(config: &AppConfig) -> anyhow::Result<database::redb::RedbDatabase> {
    info!("Initializing Redb database...");

    let db_path = config.get_database_path();
    // Change extension from .db to .redb
    let db_path = db_path.with_extension("redb");
    let cache_size_mb = config.database.redb_cache_mb;
    info!("Database path: {}", db_path.display());

    // Create Redb database manager
    let mut database =
        match database::redb::RedbDatabase::new_with_cache(db_path.clone(), cache_size_mb).await {
            Ok(database) => database,
            Err(error) => {
                error!("Failed to open ReDB database: {}", error);
                if database::redb::is_incompatible_database(&error) {
                    discard_incompatible_database(&db_path)?;
                } else {
                    preserve_failed_database(&db_path)?;
                }
                database::redb::RedbDatabase::new_with_cache(db_path.clone(), cache_size_mb)
                    .await
                    .context("Failed to create replacement ReDB database")?
            }
        };

    // Initialize database schema
    if let Err(error) = database.initialize().await {
        error!("Failed to initialize ReDB schema: {}", error);
        drop(database);
        preserve_failed_database(&db_path)?;
        database = database::redb::RedbDatabase::new_with_cache(db_path.clone(), cache_size_mb)
            .await
            .context("Failed to create replacement ReDB database")?;
        database
            .initialize()
            .await
            .context("Failed to initialize replacement database schema")?;
    }

    // Perform health check
    info!("Performing database health check...");
    if config.database.backup_enabled {
        let backup_dir = db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("backups");
        let pre_repair = backup_dir.join(format!(
            "pre-repair-{}-{}.redb",
            chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
            uuid::Uuid::new_v4().simple()
        ));
        database
            .create_backup(&pre_repair)
            .await
            .with_context(|| {
                format!(
                    "Failed to create mandatory pre-repair backup {}",
                    pre_repair.display()
                )
            })?;
    }
    let health = match database.check_and_repair().await {
        Ok(health) => health,
        Err(repair_error) => {
            error!("Database index rebuild failed: {}", repair_error);
            drop(database);
            preserve_failed_database(&db_path)?;
            database = database::redb::RedbDatabase::new_with_cache(db_path.clone(), cache_size_mb)
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
        let compacted = database
            .vacuum()
            .await
            .context("Failed to compact database")?;
        info!(compacted, "Database compaction completed");
    }

    info!("Database initialized successfully");
    Ok(database)
}

/// Initialize file system watcher for real-time media monitoring
async fn initialize_file_watcher(
    config: &AppConfig,
) -> anyhow::Result<CrossPlatformWatcher> {
    info!("Initializing file system watcher...");

    if !config.media.watch_for_changes {
        info!("File system watching disabled in configuration");
        return Ok(CrossPlatformWatcher::with_policies(
            media::ScanPolicy::policies(config),
        ));
    }

    let watcher = CrossPlatformWatcher::with_policies(media::ScanPolicy::policies(config));

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

/// Fully initialized resources shared by lifecycle services.
#[derive(Clone)]
pub struct ApplicationContext<D: DatabaseManager = database::redb::RedbDatabase> {
    pub config: Arc<AppConfig>,
    pub config_manager: Arc<ConfigManager>,
    pub database: Arc<D>,
    pub file_watcher: Arc<CrossPlatformWatcher>,
    pub platform_info: Arc<PlatformInfo>,
    pub app_state: AppState<D>,
}

/// Public bootstrap operations for embedders that manage the lifecycle themselves.
pub struct BootstrapService;

impl BootstrapService {
    pub async fn detect_platform() -> anyhow::Result<PlatformInfo> {
        detect_platform_with_diagnostics().await
    }

    pub async fn initialize_database(
        config: &AppConfig,
    ) -> anyhow::Result<database::redb::RedbDatabase> {
        initialize_database(config).await
    }

    pub async fn initialize_watcher(
        config: &AppConfig,
    ) -> anyhow::Result<CrossPlatformWatcher> {
        initialize_file_watcher(config).await
    }

    pub async fn start_platform_adaptation<D: DatabaseManager + 'static>(
        platform_info: Arc<PlatformInfo>,
        config_manager: Arc<ConfigManager>,
        watcher: Arc<CrossPlatformWatcher>,
        state: AppState<D>,
        cancellation: CancellationToken,
    ) -> anyhow::Result<tokio::task::JoinHandle<()>> {
        start_platform_adaptation(platform_info, config_manager, watcher, state, cancellation).await
    }
}

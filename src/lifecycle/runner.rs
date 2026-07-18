async fn run_application(cli_args: LaunchOptions) -> anyhow::Result<()> {
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
        Ok(db) => Arc::new(db),
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

    // Create shared application state
    let filesystem_manager: Arc<dyn crate::platform::filesystem::FileSystemManager> =
        Arc::from(create_platform_filesystem_manager());
    let resolved_log_file =
        log_file_path.unwrap_or_else(|| crate::config::AppConfig::get_platform_log_file_path());
    let lifecycle_stats = Arc::new(ApplicationStats::new());
    let app_state = AppState {
        config: config.clone(),
        media_directories: Arc::new(tokio::sync::RwLock::new(config.media.directories.clone())),
        database: database.clone(),
        platform_info: platform_info.clone(),
        filesystem_manager,
        content_update_id: Arc::new(std::sync::atomic::AtomicU32::new(1)),
        web_metrics: Arc::new(crate::web::diagnostics::WebHandlerMetrics::new()),
        lifecycle_stats: lifecycle_stats.clone(),
        bookmarks: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        log_file_path: resolved_log_file,
        browse_cache: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        mcp_clients: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_monitors: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_casts: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        discovered_tvs: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        upnp_subscriptions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
    };

    let ApplicationContext {
        config,
        config_manager,
        database,
        file_watcher,
        platform_info,
        app_state,
    } = ApplicationContext {
        config,
        config_manager,
        database,
        file_watcher,
        platform_info,
        app_state,
    };

    let shutdown = ShutdownCoordinator::new();
    let cancellation = shutdown.token();
    let mut services = tokio::task::JoinSet::<(&'static str, anyhow::Result<()>)>::new();

    let subscription_handle = {
        let subscriptions = app_state.upnp_subscriptions.clone();
        let cancellation = cancellation.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                tokio::select! {
                    _ = cancellation.cancelled() => break,
                    _ = interval.tick() => {
                        let now = std::time::Instant::now();
                        subscriptions
                            .lock()
                            .await
                            .retain(|_, subscription| subscription.expires_at > now);
                    }
                }
            }
        })
    };
    services.spawn(async move {
        (
            "subscription cleanup",
            subscription_handle.await.map_err(anyhow::Error::from),
        )
    });

    // Start file system monitoring
    match start_file_monitoring(
        file_watcher.clone(),
        app_state.clone(),
        cancellation.clone(),
    )
    .await
    {
        Ok(Some(handle)) => {
            services.spawn(async move {
                (
                    "media monitoring",
                    handle.await.map_err(anyhow::Error::from),
                )
            });
        }
        Ok(None) => {}
        Err(e) => {
            warn!("Failed to start file system monitoring: {}", e);
            warn!("Continuing without real-time file monitoring");
        }
    }

    // Scan only after the watcher is active. This closes the startup blind
    // window: a download that lands while the scan is running is either found
    // by the scan or delivered by the watcher (and duplicate upserts are safe).
    if let Err(e) = perform_initial_media_scan(&config, &database).await {
        error!("Failed to perform initial media scan: {}", e);
        return Err(e);
    }

    // Perform initial playlist file scan after media scan so referenced files exist.
    if let Err(e) = perform_initial_playlist_scan(&config, &database).await {
        // Log warning but don't fail startup - playlists are not critical
        warn!("Failed to scan playlist files: {}", e);
    }

    // Start runtime platform adaptation services
    let adaptation_handle = start_platform_adaptation(
        platform_info.clone(),
        config_manager.clone(),
        file_watcher.clone(),
        app_state.clone(),
        cancellation.clone(),
    )
    .await?;
    services.spawn(async move {
        (
            "platform adaptation",
            adaptation_handle.await.map_err(anyhow::Error::from),
        )
    });

    // Start atomic application statistics monitoring
    let monitoring_handle = start_atomic_monitoring(
        database.clone(),
        lifecycle_stats.clone(),
        cancellation.clone(),
    )
    .await?;
    services.spawn(async move {
        (
            "maintenance",
            monitoring_handle.await.map_err(anyhow::Error::from),
        )
    });

    // Start SSDP discovery service with platform abstraction
    let ssdp_handle = start_ssdp_service(app_state.clone(), cancellation.clone()).await?;
    services.spawn(async move {
        let result = ssdp_handle
            .await
            .map_err(anyhow::Error::from)
            .and_then(|result| result);
        ("SSDP", result)
    });

    // Start the HTTP server as a background task
    let network_handles =
        match start_http_server_task(app_state.clone(), cancellation.clone()).await {
            Ok(handles) => handles,
            Err(e) => {
                error!("Failed to start HTTP server: {}", e);
                return Err(e);
            }
        };
    services.spawn(async move {
        let result = network_handles
            .http
            .await
            .map_err(anyhow::Error::from)
            .and_then(|result| result);
        ("HTTP", result)
    });
    services.spawn(async move {
        (
            "TV discovery",
            network_handles
                .tv_discovery
                .await
                .map_err(anyhow::Error::from),
        )
    });

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

    // One signal listener and one supervisor own the application lifetime.
    tokio::select! {
        _ = shutdown.wait_for_signal() => {
            info!("Received shutdown signal");
        }
        completed = services.join_next() => {
            match completed {
                Some(Ok((name, Ok(())))) => warn!("Critical service stopped unexpectedly: {}", name),
                Some(Ok((name, Err(error)))) => error!("Critical service {} failed: {}", name, error),
                Some(Err(error)) => error!("Critical service task panicked: {}", error),
                None => warn!("All lifecycle services stopped unexpectedly"),
            }
        }
    }

    info!("Shutting down gracefully...");
    shutdown.cancel();
    if let Err(error) = file_watcher.stop_watching().await {
        warn!("Failed to stop file watcher cleanly: {}", error);
    }

    let shutdown_timeout = std::time::Duration::from_secs(10);
    let shutdown_start = std::time::Instant::now();
    let joined = tokio::time::timeout(shutdown_timeout, async {
        while let Some(result) = services.join_next().await {
            match result {
                Ok((name, Err(error))) => warn!("Service {} stopped with error: {}", name, error),
                Err(error) => warn!("Service join failed during shutdown: {}", error),
                _ => {}
            }
        }
    })
    .await;
    if joined.is_err() {
        warn!(
            "Shutdown timeout reached after {:?}; aborting remaining services",
            shutdown_timeout
        );
        services.abort_all();
    }

    if let Err(e) = perform_graceful_shutdown(&database, &lifecycle_stats).await {
        error!("Error during graceful shutdown: {}", e);
    }
    info!("Shutdown completed in {:?}", shutdown_start.elapsed());

    Ok(())
}

/// Top-level owner of VuIO startup, services, and shutdown.
pub struct ApplicationRunner;

impl ApplicationRunner {
    pub async fn run(options: LaunchOptions) -> anyhow::Result<()> {
        run_application(options).await
    }
}

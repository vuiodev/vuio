use super::*;

pub(super) async fn create_lifecycle_backup<B: DatabaseBackend>(
    database: &Arc<B>,
    config: &AppConfig,
) -> anyhow::Result<PathBuf> {
    let extension = B::file_extension();
    let database_path = database_path_for::<B>(config);
    let backup_dir = database_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("backups");
    tokio::fs::create_dir_all(&backup_dir).await?;
    let filename = format!(
        "vuio-{}-{}.{extension}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
        uuid::Uuid::new_v4().simple()
    );
    let destination = backup_dir.join(filename);
    database.create_backup(&destination).await?;

    let mut entries = tokio::fs::read_dir(&backup_dir).await?;
    let mut backups = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some(extension) {
            backups.push(path);
        }
    }
    backups.sort();
    let remove_count = backups.len().saturating_sub(3);
    for old in backups.into_iter().take(remove_count) {
        tokio::fs::remove_file(old).await?;
    }
    Ok(destination)
}

pub(super) async fn run_with_database<D, Initialize, InitializeFuture, Restore, RestoreFuture>(
    runtime_options: RuntimeOptions,
    initialize_backend: Initialize,
    restore_backend: Restore,
) -> anyhow::Result<()>
where
    D: DatabaseBackend,
    Initialize: FnOnce(Arc<AppConfig>) -> InitializeFuture,
    InitializeFuture: std::future::Future<Output = anyhow::Result<D>>,
    Restore: FnOnce(Arc<AppConfig>, PathBuf) -> RestoreFuture,
    RestoreFuture: std::future::Future<Output = anyhow::Result<()>>,
{
    // Initialize logging with options
    let log_file_path = runtime_options.log_file.as_ref().map(PathBuf::from);
    logging::init_logging_with_options(
        runtime_options.log_level.as_deref(),
        log_file_path.clone(),
        runtime_options.debug,
    )
    .context("Failed to initialize logging")?;

    info!("Starting VuIO Server...");

    let shutdown = ShutdownCoordinator::from_token(runtime_options.cancellation.clone());
    let cancellation = shutdown.token();
    let background_tasks = tokio_util::task::TaskTracker::new();

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
        runtime_options.config_path.clone(),
        runtime_options.overrides.clone(),
        cancellation.clone(),
        background_tasks.clone(),
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

    if let Some(backup) = runtime_options.restore_backup.as_deref() {
        restore_backend(config.clone(), PathBuf::from(backup))
            .await
            .with_context(|| format!("Failed to restore database backup {backup}"))?;
        info!("Restored database backup from {}", backup);
    }

    // Initialize database manager
    let database = match initialize_backend(config.clone()).await {
        Ok(db) => Arc::new(db),
        Err(e) => {
            error!("Failed to initialize database: {}", e);
            return Err(e);
        }
    };

    if config.database.backup_enabled {
        match create_lifecycle_backup(&database, &config).await {
            Ok(path) => info!("Created startup database backup at {}", path.display()),
            Err(error) => warn!("Startup database backup failed: {}", error),
        }
    }

    // Initialize file system watcher
    let file_watcher = match initialize_file_watcher(&config).await {
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
        log_file_path.unwrap_or_else(crate::config::AppConfig::get_platform_log_file_path);
    let lifecycle_stats = Arc::new(ApplicationStats::new());
    let auth = Arc::new(crate::web::auth::AuthState::load(
        &config.management,
        config_manager.get_config_path(),
        runtime_options.auth,
    )?);
    #[cfg(feature = "casting")]
    let renderer_cache = crate::runtime_state::RendererCache::persistent(database.clone())
        .await
        .context("Failed to initialize AirPlay credential storage")?;
    // Provider API keys are read from the environment, and `.env` is looked for
    // beside the configuration first. Published before anything can ask for a
    // credential, because the search path is resolved once and then frozen.
    #[cfg(feature = "mediainfo")]
    crate::mediainfo::env_keys::set_config_dir(config_manager.get_config_path());

    let app_state = AppState {
        config: config.clone(),
        live_config: Arc::new(crate::state::LiveConfig::new(config.clone())),
        // Seeded from the configured port so nothing reads a zero before the first
        // bind; overwritten with the address actually taken once the listener is up.
        http_binding: Arc::new(crate::state::HttpBinding::new(config.server.port)),
        config_source: Arc::new(crate::state::ConfigSource {
            path: config_manager.get_config_path().to_path_buf(),
            // The manager only watches a config it loaded from a durable location; a
            // container's env-var configuration gets an unwatched scratch file.
            durable: config_manager.is_watched(),
            overrides: config_manager.overrides().clone(),
        }),
        media_directories: Arc::new(tokio::sync::RwLock::new(config.media.directories.clone())),
        unavailable_roots: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
        database: database.clone(),
        auth: auth.clone(),
        platform_info: platform_info.clone(),
        filesystem_manager,
        content_update_id: Arc::new(std::sync::atomic::AtomicU32::new(1)),
        web_metrics: Arc::new(crate::web::diagnostics::WebHandlerMetrics::new()),
        runtime_diagnostics: Arc::new(crate::platform::diagnostics::SystemDiagnosticsSampler::new()),
        lifecycle_stats: lifecycle_stats.clone(),
        bookmarks: Arc::new(tokio::sync::Mutex::new(
            crate::runtime_state::BookmarkRegistry::new(crate::runtime_state::BOOKMARK_MAX_ENTRIES),
        )),
        log_file_path: resolved_log_file,
        browse_cache: Arc::new(tokio::sync::Mutex::new(
            crate::runtime_state::BrowseResponseCache::new(),
        )),
        mcp_clients: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_monitors: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_casts: Arc::new(tokio::sync::Mutex::new(
            crate::runtime_state::ActiveCastRegistry::new(),
        )),
        #[cfg(feature = "mediainfo")]
        mediainfo_job: Arc::new(tokio::sync::Mutex::new(Default::default())),
        #[cfg(feature = "casting")]
        discovered_tvs: Arc::new(renderer_cache),
        upnp_subscriptions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        cancellation: cancellation.clone(),
        background_tasks: background_tasks.clone(),
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

    let mut services = tokio::task::JoinSet::<(&'static str, anyhow::Result<()>)>::new();

    // Always spawned, gated per tick. Spawning only when backups were on at boot made
    // the setting one-way: turning it off worked, turning it on did nothing until a
    // restart, because there was no task left to notice.
    {
        let backup_database = database.clone();
        let backup_state = app_state.clone();
        let backup_cancellation = cancellation.clone();
        services.spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
            interval.tick().await;
            loop {
                tokio::select! {
                    _ = backup_cancellation.cancelled() => break,
                    _ = interval.tick() => {
                        let current = backup_state.current_config();
                        if current.database.backup_enabled {
                            if let Err(error) = create_lifecycle_backup(&backup_database, &current).await {
                                warn!("Periodic database backup failed: {}", error);
                            }
                        }
                    }
                }
            }
            ("database backup", Ok(()))
        });
    }

    let subscription_handle = {
        let subscriptions = app_state.upnp_subscriptions.clone();
        let active_casts = app_state.active_casts.clone();
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
                        active_casts.lock().await.prune();
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

    // Supervised rather than started once, so watch_for_changes can be toggled without
    // a restart.
    let monitoring_watcher = file_watcher.clone();
    let monitoring_state = app_state.clone();
    let monitoring_cancellation = cancellation.clone();
    services.spawn(async move {
        (
            "media monitoring",
            supervisor::run_monitoring_supervisor(
                monitoring_watcher,
                monitoring_state,
                monitoring_cancellation,
            )
            .await,
        )
    });

    // Scan only after the watcher is active. This closes the startup blind
    // window: a download that lands while the scan is running is either found
    // by the scan or delivered by the watcher (and duplicate upserts are safe).
    if let Err(e) = perform_initial_media_scan(&config, &database).await {
        error!("Failed to perform initial media scan: {}", e);
        return Err(e);
    }
    refresh_unavailable_roots(&app_state).await?;

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

    // The listener and the discovery advertisement are supervised rather than started,
    // so a port, identity or discovery change can be applied by rebuilding them instead
    // of by restarting the process. Both tasks run until shutdown; see `supervisor`.
    let http_state = app_state.clone();
    let http_cancellation = cancellation.clone();
    let http_started = supervisor::bind_first_listener(&http_state).await?;
    services.spawn(async move {
        (
            "HTTP",
            supervisor::run_http_supervisor(http_state, http_cancellation, http_started).await,
        )
    });

    // The browser interface answers on its own port, with the same router over
    // the same state: a second front end rather than a second server. Supervised
    // separately from the main listener because it can be turned off, and
    // because a port it cannot take must not be able to stop the media server.
    #[cfg(feature = "web-ui")]
    {
        let web_ui_state = app_state.clone();
        let web_ui_cancellation = cancellation.clone();
        services.spawn(async move {
            (
                "web UI",
                supervisor::run_web_ui_supervisor(web_ui_state, web_ui_cancellation).await,
            )
        });
    }

    let discovery_state = app_state.clone();
    let discovery_cancellation = cancellation.clone();
    services.spawn(async move {
        (
            "discovery",
            supervisor::run_advertisement_supervisor(discovery_state, discovery_cancellation).await,
        )
    });

    // Renderer discovery has nothing to do with the listener; it used to be started
    // beside it and would otherwise be cycled by every rebind.
    let tv_discovery = start_tv_discovery(app_state.clone(), cancellation.clone());
    services.spawn(async move {
        ("TV discovery", tv_discovery.await.map_err(anyhow::Error::from))
    });

    // Determine if console logging is verbose
    let is_rust_log_set = std::env::var("RUST_LOG").is_ok();
    let in_docker = AppConfig::is_running_in_docker();
    let console_is_verbose = runtime_options.debug
        || is_rust_log_set
        || runtime_options.log_level.is_some()
        || in_docker;

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
        // The bound port, not the configured one: a banner naming a port nothing answers
        // on is worse than no banner.
        let web_url = format!("http://{}:{}", display_ip, app_state.http_binding.port());
        let db_path = database_path_for::<D>(&config);

        fn tail_with_ellipsis(value: &str, max_chars: usize) -> String {
            let count = value.chars().count();
            if count <= max_chars {
                return value.to_owned();
            }
            let tail = value
                .chars()
                .skip(count.saturating_sub(max_chars.saturating_sub(3)))
                .collect::<String>();
            format!("...{tail}")
        }

        let display_name = tail_with_ellipsis(&config.server.name, 41);
        let display_url = tail_with_ellipsis(&web_url, 41);

        let db_path_str = db_path.to_string_lossy().to_string();
        let display_db_path = tail_with_ellipsis(&db_path_str, 41);

        let auth_status = if auth.enabled() {
            "Enabled (see token path below)".to_string()
        } else {
            "Disabled (Public Access)".to_string()
        };
        let display_auth = tail_with_ellipsis(&auth_status, 41);

        println!("┌────────────────────────────────────────────────────────┐");
        println!("│  VuIO Media Server                                     │");
        println!("├────────────────────────────────────────────────────────┤");
        println!("│  Name:       {:<41} │", display_name);
        println!("│  Version:    {:<41} │", env!("CARGO_PKG_VERSION"));
        println!("│  Status:     Online & Streaming                        │");
        println!("│  Address:    {:<41} │", display_url);
        println!("│  SSDP:       Active on port 1900                       │");
        println!("│  Database:   {:<41} │", display_db_path);
        println!("│  Auth:       {:<41} │", display_auth);
        println!("│                                                        │");
        println!("│  Monitored Directories:                                │");
        if config.media.directories.is_empty() {
            println!("│    (none configured)                                   │");
        } else {
            for dir in &config.media.directories {
                let path_str = &dir.path;
                let display_path = tail_with_ellipsis(path_str, 49);
                println!("│    • {:<49} │", display_path);
            }
        }
        println!("│                                                        │");
        println!("│  Press Ctrl+C to stop the server safely.               │");
        println!("└────────────────────────────────────────────────────────┘");

        if auth.enabled() {
            println!("Management Authentication is active.");
            println!("Token file: {}", auth.token_path().display());
            println!();
        }
    }

    // One signal listener and one supervisor own the application lifetime.
    let mut shutdown_error = None;
    tokio::select! {
        _ = cancellation.cancelled() => {
            info!("Received host shutdown request");
        }
        completed = services.join_next() => {
            let failure = match completed {
                Some(Ok((name, Ok(())))) => anyhow::anyhow!("critical service {name} stopped unexpectedly"),
                Some(Ok((name, Err(error)))) => anyhow::anyhow!("critical service {name} failed: {error}"),
                Some(Err(error)) => anyhow::anyhow!("critical service task panicked: {error}"),
                None => anyhow::anyhow!("all lifecycle services stopped unexpectedly"),
            };
            error!("{}", failure);
            shutdown_error = Some(failure);
        }
    }

    info!("Shutting down gracefully...");
    // Tear renderer sessions down first: a receiver keeps rendering what it has
    // buffered unless it is told to stop, and the control connection has to
    // still be alive to tell it.
    #[cfg(feature = "casting")]
    app_state.discovered_tvs.shutdown().await;
    shutdown.cancel();
    background_tasks.close();
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
        background_tasks.wait().await;
    })
    .await;
    if joined.is_err() {
        warn!(
            "Shutdown timeout reached after {:?}; aborting remaining services",
            shutdown_timeout
        );
        services.abort_all();
    }

    if app_state.current_config().database.backup_enabled {
        match create_lifecycle_backup(&database, &app_state.current_config()).await {
            Ok(path) => info!("Created shutdown database backup at {}", path.display()),
            Err(error) => warn!("Shutdown database backup failed: {}", error),
        }
    }
    if let Err(e) = perform_graceful_shutdown(&database, &lifecycle_stats).await {
        error!("Error during graceful shutdown: {}", e);
    }
    info!("Shutdown completed in {:?}", shutdown_start.elapsed());

    if let Some(error) = shutdown_error {
        Err(error)
    } else {
        Ok(())
    }
}

pub(super) async fn run_application(runtime_options: RuntimeOptions) -> anyhow::Result<()> {
    run_application_with::<database::ActiveDatabase>(runtime_options).await
}

/// The startup path, written once against the backend seam.
///
/// Nothing here names a storage engine: the backend arrives as `B`, and every
/// path, extension and restore rule comes from `DatabaseBackend`.
async fn run_application_with<B: DatabaseBackend>(
    runtime_options: RuntimeOptions,
) -> anyhow::Result<()> {
    run_with_database::<B, _, _, _, _>(
        runtime_options,
        |config| async move { initialize_database::<B>(&config).await },
        |config, backup| async move {
            B::restore_backup_file(&backup, &database_path_for::<B>(&config)).await
        },
    )
    .await
}

/// Top-level owner of VuIO startup, services, and shutdown.
pub struct ApplicationRunner;

impl ApplicationRunner {
    pub async fn run(options: RuntimeOptions) -> anyhow::Result<()> {
        run_application(options).await
    }
}

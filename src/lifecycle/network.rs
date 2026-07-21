/// Start SSDP service with platform abstraction
async fn start_ssdp_service<D: DatabaseManager + 'static>(
    app_state: AppState<D>,
    cancellation: CancellationToken,
) -> anyhow::Result<tokio::task::JoinHandle<anyhow::Result<()>>> {
    info!("Starting SSDP discovery service...");

    let handle = tokio::spawn(async move {
        let mut instance_cancel = CancellationToken::new();
        let mut instance = tokio::spawn(ssdp::run_ssdp_service_until_cancelled(
            app_state.clone(),
            instance_cancel.clone(),
        ));
        loop {
            tokio::select! {
                result = &mut instance => {
                    return result.map_err(anyhow::Error::from)?.context("SSDP service failed");
                }
                _ = cancellation.cancelled() => {
                    instance_cancel.cancel();
                    return instance.await.map_err(anyhow::Error::from)?.context("SSDP service failed");
                }
                _ = app_state.ssdp_reload_notify.notified() => {
                    instance_cancel.cancel();
                    match tokio::time::timeout(std::time::Duration::from_secs(10), &mut instance).await {
                        Ok(Ok(Ok(()))) => {}
                        Ok(Ok(Err(error))) => warn!(%error, "SSDP service failed during reload"),
                        Ok(Err(error)) => warn!(%error, "SSDP task failed during reload"),
                        Err(_) => instance.abort(),
                    }
                    instance_cancel = CancellationToken::new();
                    instance = tokio::spawn(ssdp::run_ssdp_service_until_cancelled(
                        app_state.clone(),
                        instance_cancel.clone(),
                    ));
                    info!("SSDP service reloaded");
                }
            }
        }
    });
    Ok(handle)
}

/// Start HTTP server as a background task with proper error handling
async fn start_http_server_task<D: DatabaseManager + 'static>(
    app_state: AppState<D>,
    cancellation: CancellationToken,
) -> anyhow::Result<NetworkTaskHandles> {
    info!("Starting HTTP server...");

    let config = app_state.current_config();

    let addr = server_address(&config.server)?;

    info!("Server UUID: {}", config.server.uuid);
    info!("Server name: {}", config.server.name);
    info!("Listening on http://{}", addr);

    // Attempt to bind to the address
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind to address: {}", addr))?;

    info!("HTTP server started successfully");

    // A supervisor owns the active listener so bind/port changes can be
    // pre-bound and swapped without publishing an endpoint that is not live.
    let http = tokio::spawn(async move {
        let mut instance_cancel = CancellationToken::new();
        let mut instance = spawn_http_instance(
            listener,
            app_state.clone(),
            instance_cancel.clone(),
        );
        loop {
            tokio::select! {
                result = &mut instance => {
                    return result.map_err(anyhow::Error::from)?.context("HTTP server failed");
                }
                _ = cancellation.cancelled() => {
                    instance_cancel.cancel();
                    return instance.await.map_err(anyhow::Error::from)?.context("HTTP server failed");
                }
                _ = app_state.http_rebind_notify.notified() => {
                    let desired = app_state.desired_config.load();
                    let desired_addr = match server_address(&desired.server) {
                        Ok(address) => address,
                        Err(error) => {
                            record_http_reload_error(&app_state, error.to_string());
                            continue;
                        }
                    };
                    let replacement = match tokio::net::TcpListener::bind(desired_addr).await {
                        Ok(listener) => listener,
                        Err(error) => {
                            record_http_reload_error(
                                &app_state,
                                format!("failed to bind replacement listener {desired_addr}: {error}"),
                            );
                            continue;
                        }
                    };
                    let replacement_cancel = CancellationToken::new();
                    let replacement_task = spawn_http_instance(
                        replacement,
                        app_state.clone(),
                        replacement_cancel.clone(),
                    );

                    let mut effective = (*app_state.current_config()).clone();
                    effective.server = desired.server.clone();
                    app_state.live_config.store(Arc::new(effective));
                    app_state.ssdp_reload_notify.notify_one();
                    app_state
                        .pending_restart_fields
                        .write()
                        .unwrap_or_else(|error| error.into_inner())
                        .retain(|field| field != "server.port" && field != "server.interface");
                    app_state
                        .config_reload_errors
                        .write()
                        .unwrap_or_else(|error| error.into_inner())
                        .retain(|error| !error.starts_with("http listener:"));

                    instance_cancel.cancel();
                    if tokio::time::timeout(std::time::Duration::from_secs(10), &mut instance)
                        .await
                        .is_err()
                    {
                        instance.abort();
                    }
                    instance_cancel = replacement_cancel;
                    instance = replacement_task;
                    info!(%desired_addr, "HTTP listener reloaded");
                }
            }
        }
    });

    Ok(NetworkTaskHandles { http })
}

fn server_address(config: &crate::config::ServerConfig) -> anyhow::Result<SocketAddr> {
    let interface = if config.interface == "0.0.0.0" || config.interface.is_empty() {
        std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)
    } else {
        config
            .interface
            .parse()
            .with_context(|| format!("Invalid server interface address: {}", config.interface))?
    };
    Ok(SocketAddr::new(interface, config.port))
}

fn spawn_http_instance<D: DatabaseManager + 'static>(
    listener: tokio::net::TcpListener,
    state: AppState<D>,
    cancellation: CancellationToken,
) -> tokio::task::JoinHandle<std::io::Result<()>> {
    tokio::spawn(async move {
        axum::serve(
            listener,
            web::create_router(state)
                .into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(cancellation.cancelled_owned())
        .await
    })
}

fn record_http_reload_error<D: DatabaseManager>(state: &AppState<D>, error: String) {
    let mut errors = state
        .config_reload_errors
        .write()
        .unwrap_or_else(|error| error.into_inner());
    errors.retain(|error| !error.starts_with("http listener:"));
    errors.push(format!("http listener: {error}"));
}

/// HTTP, SSDP, and television-discovery lifecycle operations.
pub struct NetworkLifecycleService;

pub struct NetworkTaskHandles {
    pub http: tokio::task::JoinHandle<anyhow::Result<()>>,
}

impl NetworkLifecycleService {
    pub async fn start_http<D: DatabaseManager + 'static>(
        state: AppState<D>,
        cancellation: CancellationToken,
    ) -> anyhow::Result<NetworkTaskHandles> {
        start_http_server_task(state, cancellation).await
    }

    pub async fn start_ssdp<D: DatabaseManager + 'static>(
        state: AppState<D>,
        cancellation: CancellationToken,
    ) -> anyhow::Result<tokio::task::JoinHandle<anyhow::Result<()>>> {
        start_ssdp_service(state, cancellation).await
    }
}

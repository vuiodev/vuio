/// Start SSDP service with platform abstraction
async fn start_ssdp_service(
    app_state: AppState,
    cancellation: CancellationToken,
) -> anyhow::Result<tokio::task::JoinHandle<anyhow::Result<()>>> {
    info!("Starting SSDP discovery service...");

    // Start SSDP service using existing implementation
    let handle = tokio::spawn(async move {
        ssdp::run_ssdp_service_until_cancelled(app_state, cancellation)
            .await
            .context("SSDP service failed")
    });
    Ok(handle)
}

/// Start HTTP server as a background task with proper error handling
async fn start_http_server_task(
    app_state: AppState,
    cancellation: CancellationToken,
) -> anyhow::Result<NetworkTaskHandles> {
    info!("Starting HTTP server...");

    let config = app_state.config.clone();

    // Create the Axum web server
    let app = web::create_router(app_state.clone());

    // Parse server interface address
    let interface_addr =
        if config.server.interface == "0.0.0.0" || config.server.interface.is_empty() {
            std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)
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

    // Spawn background SSDP TV discovery cache refresher every 60s
    let state_clone = app_state.clone();
    let discovery_cancellation = cancellation.clone();
    let tv_discovery = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = discovery_cancellation.cancelled() => break,
                _ = async {
                    if let Err(error) = state_clone.discovered_tvs.refresh().await {
                        tracing::warn!(%error, "Background renderer discovery failed");
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                } => {}
            }
        }
    });

    // Spawn the server as a background task
    let http = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(cancellation.cancelled_owned())
        .await
        .context("HTTP server failed")
    });

    Ok(NetworkTaskHandles { http, tv_discovery })
}

/// HTTP, SSDP, and television-discovery lifecycle operations.
pub struct NetworkLifecycleService;

pub struct NetworkTaskHandles {
    pub http: tokio::task::JoinHandle<anyhow::Result<()>>,
    pub tv_discovery: tokio::task::JoinHandle<()>,
}

impl NetworkLifecycleService {
    pub async fn start_http(
        state: AppState,
        cancellation: CancellationToken,
    ) -> anyhow::Result<NetworkTaskHandles> {
        start_http_server_task(state, cancellation).await
    }

    pub async fn start_ssdp(
        state: AppState,
        cancellation: CancellationToken,
    ) -> anyhow::Result<tokio::task::JoinHandle<anyhow::Result<()>>> {
        start_ssdp_service(state, cancellation).await
    }
}

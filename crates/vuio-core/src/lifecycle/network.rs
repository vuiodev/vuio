use super::*;

/// Start SSDP service with platform abstraction
const TV_DISCOVERY_INTERVAL: tokio::time::Duration = tokio::time::Duration::from_secs(30);

pub(super) async fn start_ssdp_service<D: DatabaseManager + 'static>(
    app_state: AppState<D>,
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
pub(super) async fn start_http_server_task<D: DatabaseManager + 'static>(
    app_state: AppState<D>,
    cancellation: CancellationToken,
) -> anyhow::Result<NetworkTaskHandles> {
    info!("Starting HTTP server...");

    let config = app_state.current_config();

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

    // Keep one shared DLNA, Google Cast, and AirPlay renderer snapshot fresh.
    #[cfg(feature = "casting")]
    let state_clone = app_state.clone();
    let discovery_cancellation = cancellation.clone();
    let tv_discovery = tokio::spawn(async move {
        let mut interval = tokio::time::interval(TV_DISCOVERY_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = discovery_cancellation.cancelled() => break,
                _ = interval.tick() => {
                    #[cfg(feature = "casting")]
                    if let Err(error) = state_clone.discovered_tvs.refresh().await {
                        tracing::warn!(%error, "Background multi-protocol renderer discovery failed");
                    }
                }
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

pub struct NetworkTaskHandles {
    pub http: tokio::task::JoinHandle<anyhow::Result<()>>,
    pub tv_discovery: tokio::task::JoinHandle<()>,
}


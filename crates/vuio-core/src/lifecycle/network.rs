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

    // One source of truth for "where we actually answer": everything that hands out a
    // URL reads this, so mDNS must not compute its own answer beside it.
    app_state.http_binding.publish_serving(listener.local_addr()?);
    start_mdns_advertisement(&app_state, app_state.http_binding.port(), cancellation.clone());

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

/// Announce this server over mDNS alongside SSDP.
///
/// The advertisement is held by a background task rather than returned,
/// because withdrawing it is what matters at shutdown: the task drops the
/// advertiser when cancelled, which sends the goodbye packets. Without that a
/// stopped server lingers in client caches for the record's full TTL.
///
/// `port` is the port actually bound, not the configured one, so a server
/// started on port 0 still advertises where it can be reached.
fn start_mdns_advertisement<D: DatabaseManager + 'static>(
    app_state: &AppState<D>,
    port: u16,
    cancellation: CancellationToken,
) {
    let config = app_state.current_config();
    if !config.network.mdns_enabled {
        debug!("mDNS advertisement disabled by configuration");
        return;
    }

    // The same address SSDP advertises: announcing a different one over mDNS
    // would send clients somewhere the DLNA path does not point.
    let advertised = app_state.get_server_ip();
    let Ok(ip) = advertised.parse::<std::net::IpAddr>() else {
        warn!(address = %advertised, "Skipping mDNS advertisement: not a usable address");
        return;
    };

    let server = crate::mdns::ServerAdvertisement {
        uuid: config.server.uuid.clone(),
        name: config.server.name.clone(),
        ip,
        port,
        requires_auth: app_state.auth.enabled(),
    };

    app_state.background_tasks.spawn(async move {
        let advertiser = match crate::mdns::MdnsAdvertiser::start(&server) {
            Ok(advertiser) => advertiser,
            Err(error) => {
                // A server that cannot advertise still serves; SSDP is
                // unaffected and the address still works if typed in.
                warn!(%error, "Could not advertise over mDNS");
                return;
            }
        };
        cancellation.cancelled().await;
        drop(advertiser);
    });
}


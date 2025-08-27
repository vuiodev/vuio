use crate::config::AppConfig;
use crate::platform::network::{NetworkManager, PlatformNetworkManager, SsdpConfig};
use crate::state::AppState;
use anyhow::Result;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

const SSDP_MULTICAST_ADDR: &str = "239.255.255.250";
const SSDP_PORT: u16 = 1900;
const ANNOUNCE_INTERVAL_SECS: u64 = 300; // Announce every 5 minutes

pub fn run_ssdp_service(state: AppState) -> Result<()> {
    // Check if running in Docker and use appropriate implementation
    if AppConfig::is_running_in_docker() {
        info!("Docker environment detected - using Docker-optimized SSDP implementation");
        return run_ssdp_service_docker(state);
    }

    info!("Native environment detected - using native SSDP implementation");

    let network_manager = Arc::new(PlatformNetworkManager::new());

    // Task for responding to M-SEARCH requests
    let search_state = state.clone();
    let search_manager = network_manager.clone();
    tokio::spawn(async move {
        if let Err(e) = ssdp_search_responder(search_state, search_manager).await {
            error!("SSDP search responder failed: {}", e);
        }
    });

    // Task for periodically sending NOTIFY announcements
    let announce_state = state;
    let announce_manager = network_manager;
    tokio::spawn(async move {
        ssdp_announcer(announce_state, announce_manager).await;
    });

    info!("SSDP service started with platform abstraction");
    Ok(())
}

// Docker-specific SSDP implementation
fn run_ssdp_service_docker(state: AppState) -> Result<()> {
    let network_manager = Arc::new(PlatformNetworkManager::new());

    // Log the IP that will be used for SSDP announcements
    let server_ip = get_server_ip_docker(&state);
    info!(
        "SSDP service starting (Docker mode) - using server IP: {}",
        server_ip
    );
    info!("SSDP will listen on hardcoded port: {}", SSDP_PORT);

    // Use unified service for Docker compatibility (single socket like MiniDLNA)
    let service_state = state.clone();
    let service_manager = network_manager.clone();
    tokio::spawn(async move {
        if let Err(e) = ssdp_unified_service_docker(service_state, service_manager).await {
            error!("SSDP unified service failed: {}", e);
        }
    });

    Ok(())
}

async fn ssdp_search_responder(
    state: AppState,
    network_manager: Arc<PlatformNetworkManager>,
) -> Result<()> {
    const MAX_SOCKET_RETRIES: u32 = 3;
    const MAX_MULTICAST_RETRIES: u32 = 5;
    const RETRY_DELAY_MS: u64 = 1000;

    // Create SSDP socket with retry logic
    let mut socket = None;
    for attempt in 1..=MAX_SOCKET_RETRIES {
        let ssdp_config = SsdpConfig::default();
        match network_manager
            .create_ssdp_socket_with_config(&ssdp_config)
            .await
        {
            Ok(s) => {
                info!(
                    "Successfully created SSDP socket on port {} (attempt {})",
                    s.port, attempt
                );
                socket = Some(s);
                break;
            }
            Err(e) => {
                error!("Failed to create SSDP socket (attempt {}): {}", attempt, e);
                if attempt < MAX_SOCKET_RETRIES {
                    warn!("Retrying socket creation in {}ms...", RETRY_DELAY_MS);
                    tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                } else {
                    return Err(anyhow::anyhow!(
                        "SSDP socket creation failed after {} attempts: {}",
                        MAX_SOCKET_RETRIES,
                        e
                    ));
                }
            }
        }
    }

    let mut socket = socket.unwrap();
    let socket_port = socket.port;

    // Join multicast group with retry logic, using the primary interface from AppState
    let multicast_addr = SSDP_MULTICAST_ADDR.parse().unwrap();
    let primary_interface = state.platform_info.get_primary_interface().cloned();
    let mut multicast_enabled = false;

    for attempt in 1..=MAX_MULTICAST_RETRIES {
        match network_manager
            .join_multicast_group(&mut socket, multicast_addr, primary_interface.as_ref())
            .await
        {
            Ok(()) => {
                info!(
                    "Successfully joined SSDP multicast group on port {} (attempt {})",
                    socket_port, attempt
                );
                multicast_enabled = true;
                break;
            }
            Err(e) => {
                warn!(
                    "Failed to join multicast group (attempt {}): {}",
                    attempt, e
                );
                if attempt < MAX_MULTICAST_RETRIES {
                    warn!("Retrying multicast join in {}ms...", RETRY_DELAY_MS);
                    tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                } else {
                    error!("Failed to join multicast group after {} attempts, continuing with unicast only", MAX_MULTICAST_RETRIES);
                    break;
                }
            }
        }
    }

    if !multicast_enabled {
        warn!("SSDP search responder running without multicast support - discovery may be limited");
        warn!("Troubleshooting tips:");
        warn!("  - Check firewall settings for UDP port {}", socket_port);
        warn!("  - Ensure network interface supports multicast");
        warn!("  - Try running with elevated privileges if using port < 1024");
    }

    let mut buf = vec![0u8; 2048];
    let mut consecutive_errors = 0;
    const MAX_CONSECUTIVE_ERRORS: u32 = 10;

    loop {
        match socket.recv_from(&mut buf).await {
            Ok((len, addr)) => {
                consecutive_errors = 0; // Reset error counter on success
                let request = String::from_utf8_lossy(&buf[..len]);

                if request.contains("M-SEARCH") {
                    debug!("Received M-SEARCH from {}", addr);
                    debug!("M-SEARCH request content: {}", request.trim());

                    // Check for various SSDP discovery patterns and determine response types
                    let mut response_types = Vec::new();

                    if request.contains("ssdp:all") {
                        // Respond with all service types
                        response_types.push("upnp:rootdevice");
                        response_types.push("urn:schemas-upnp-org:device:MediaServer:1");
                        response_types.push("urn:schemas-upnp-org:service:ContentDirectory:1");
                    } else if request.contains("upnp:rootdevice") {
                        response_types.push("upnp:rootdevice");
                    } else if request.contains("urn:schemas-upnp-org:device:MediaServer") {
                        response_types.push("urn:schemas-upnp-org:device:MediaServer:1");
                    } else if request.contains("urn:schemas-upnp-org:service:ContentDirectory") {
                        response_types.push("urn:schemas-upnp-org:service:ContentDirectory:1");
                    } else if request.contains("ssdp:discover") {
                        // Generic discovery - respond with main device type
                        response_types.push("urn:schemas-upnp-org:device:MediaServer:1");
                    }

                    if !response_types.is_empty() {
                        debug!(
                            "Sending {} SSDP response(s) to {} for types: {:?}",
                            response_types.len(),
                            addr,
                            response_types
                        );

                        let mut all_responses_sent = true;
                        let response_count = response_types.len();

                        for response_type in response_types {
                            let response =
                                create_ssdp_response(&state, socket_port, response_type).await;
                            debug!(
                                "Sending SSDP response to {} ({}): {}",
                                addr,
                                response_type,
                                response.trim()
                            );

                            // Retry response sending with exponential backoff
                            let mut response_sent = false;
                            for retry in 0..3 {
                                match socket.send_to(response.as_bytes(), addr).await {
                                    Ok(_) => {
                                        debug!("Successfully sent M-SEARCH response to {} for {} (attempt {})", addr, response_type, retry + 1);
                                        response_sent = true;
                                        break;
                                    }
                                    Err(e) => {
                                        warn!("Failed to send M-SEARCH response to {} for {} (attempt {}): {}", addr, response_type, retry + 1, e);
                                        if retry < 2 {
                                            tokio::time::sleep(Duration::from_millis(
                                                100 * (1 << retry),
                                            ))
                                            .await;
                                        }
                                    }
                                }
                            }

                            if !response_sent {
                                error!("Failed to send M-SEARCH response to {} for {} after 3 attempts", addr, response_type);
                                all_responses_sent = false;
                            }

                            // Small delay between multiple responses to avoid overwhelming the client
                            if response_count > 1 {
                                tokio::time::sleep(Duration::from_millis(50)).await;
                            }
                        }

                        if !all_responses_sent {
                            warn!("Some M-SEARCH responses to {} failed to send", addr);
                        }
                    } else {
                        info!(
                            "M-SEARCH request from {} doesn't match our service types, ignoring",
                            addr
                        );
                    }
                }
            }
            Err(e) => {
                consecutive_errors += 1;
                error!(
                    "Error receiving SSDP data (consecutive errors: {}): {}",
                    consecutive_errors, e
                );

                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    error!(
                        "Too many consecutive errors ({}), attempting to recreate socket",
                        MAX_CONSECUTIVE_ERRORS
                    );

                    // Try to recreate the socket
                    let ssdp_config = SsdpConfig::default();
                    match network_manager
                        .create_ssdp_socket_with_config(&ssdp_config)
                        .await
                    {
                        Ok(new_socket) => {
                            info!(
                                "Successfully recreated SSDP socket on port {}",
                                new_socket.port
                            );
                            socket = new_socket;
                            consecutive_errors = 0;

                            // Try to rejoin multicast group
                            if let Err(e) = network_manager
                                .join_multicast_group(
                                    &mut socket,
                                    multicast_addr,
                                    primary_interface.as_ref(),
                                )
                                .await
                            {
                                warn!(
                                    "Failed to rejoin multicast group after socket recreation: {}",
                                    e
                                );
                            }
                        }
                        Err(e) => {
                            error!("Failed to recreate SSDP socket: {}", e);
                            return Err(anyhow::anyhow!("SSDP socket recreation failed: {}", e));
                        }
                    }
                } else {
                    // Exponential backoff for error recovery
                    let delay = std::cmp::min(1000 * (1 << consecutive_errors.min(5)), 30000);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
            }
        }
    }
}

async fn ssdp_announcer(state: AppState, network_manager: Arc<PlatformNetworkManager>) {
    let mut interval = interval(Duration::from_secs(ANNOUNCE_INTERVAL_SECS));
    let mut consecutive_failures = 0;
    const MAX_CONSECUTIVE_FAILURES: u32 = 5;

    loop {
        interval.tick().await;

        match send_ssdp_alive(&state, &network_manager).await {
            Ok(()) => {
                consecutive_failures = 0; // Reset failure counter on success
            }
            Err(e) => {
                consecutive_failures += 1;
                error!(
                    "Failed to send SSDP NOTIFY (failure {}): {}",
                    consecutive_failures, e
                );

                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    error!(
                        "Too many consecutive SSDP announcement failures ({})",
                        MAX_CONSECUTIVE_FAILURES
                    );
                    error!("Troubleshooting suggestions:");
                    error!("  - Check network connectivity");
                    error!("  - Verify firewall allows UDP traffic on SSDP ports");
                    error!("  - Ensure network interfaces are up and support multicast");
                    error!("  - Try restarting the application with elevated privileges");

                    // Reset counter to avoid spam, but continue trying
                    consecutive_failures = 0;

                    // Wait longer before next attempt
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        }
    }
}

async fn send_ssdp_alive(state: &AppState, network_manager: &PlatformNetworkManager) -> Result<()> {
    const MAX_SOCKET_CREATION_RETRIES: u32 = 3;
    const MAX_SEND_RETRIES: u32 = 3;

    info!("Sending SSDP NOTIFY (alive) broadcast");

    // Create a temporary socket for announcements with retry logic
    let mut socket = None;
    for attempt in 1..=MAX_SOCKET_CREATION_RETRIES {
        let ssdp_config = SsdpConfig::default();
        match network_manager
            .create_ssdp_socket_with_config(&ssdp_config)
            .await
        {
            Ok(s) => {
                socket = Some(s);
                break;
            }
            Err(e) => {
                warn!(
                    "Failed to create announcement socket (attempt {}): {}",
                    attempt, e
                );
                if attempt < MAX_SOCKET_CREATION_RETRIES {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                } else {
                    error!(
                        "Failed to create announcement socket after {} attempts",
                        MAX_SOCKET_CREATION_RETRIES
                    );
                    return Err(anyhow::anyhow!(
                        "Announcement socket creation failed: {}",
                        e
                    ));
                }
            }
        }
    }

    let mut socket = socket.unwrap();

    // Enable multicast on the announcement socket, using the primary interface from AppState
    let multicast_addr_ip = SSDP_MULTICAST_ADDR.parse().unwrap();
    let primary_interface = state.platform_info.get_primary_interface().cloned();
    if let Err(e) = network_manager
        .join_multicast_group(&mut socket, multicast_addr_ip, primary_interface.as_ref())
        .await
    {
        warn!("Failed to enable multicast on announcement socket: {}", e);
    }

    let server_ip = get_server_ip(state).await;
    let config = &state.config;

    // Send NOTIFY for multiple service types
    let service_types = [
        "upnp:rootdevice",
        "urn:schemas-upnp-org:device:MediaServer:1",
        "urn:schemas-upnp-org:service:ContentDirectory:1",
    ];

    let multicast_addr = format!("{}:{}", SSDP_MULTICAST_ADDR, SSDP_PORT).parse::<SocketAddr>()?;

    for service_type in &service_types {
        let (nt, usn) = match *service_type {
            "upnp:rootdevice" => (
                "upnp:rootdevice".to_string(),
                format!("uuid:{}::upnp:rootdevice", config.server.uuid),
            ),
            "urn:schemas-upnp-org:device:MediaServer:1" => (
                "urn:schemas-upnp-org:device:MediaServer:1".to_string(),
                format!(
                    "uuid:{}::urn:schemas-upnp-org:device:MediaServer:1",
                    config.server.uuid
                ),
            ),
            "urn:schemas-upnp-org:service:ContentDirectory:1" => (
                "urn:schemas-upnp-org:service:ContentDirectory:1".to_string(),
                format!(
                    "uuid:{}::urn:schemas-upnp-org:service:ContentDirectory:1",
                    config.server.uuid
                ),
            ),
            _ => continue,
        };

        let message = format!(
            "NOTIFY * HTTP/1.1\r\n\
            HOST: {}:{}\r\n\
            CACHE-CONTROL: max-age=1800\r\n\
            LOCATION: http://{}:{}/description.xml\r\n\
            NT: {}\r\n\
            NTS: ssdp:alive\r\n\
            SERVER: VuIO/1.0 UPnP/1.0\r\n\
            USN: {}\r\n\
            \r\n",
            SSDP_MULTICAST_ADDR, SSDP_PORT, server_ip, config.server.port, nt, usn
        );

        // Try multicast first with retry logic
        let mut multicast_success = false;
        for attempt in 1..=MAX_SEND_RETRIES {
            match network_manager
                .send_multicast(&socket, message.as_bytes(), multicast_addr)
                .await
            {
                Ok(()) => {
                    info!(
                        "Successfully sent SSDP NOTIFY for {} via multicast (attempt {})",
                        service_type, attempt
                    );
                    multicast_success = true;
                    break;
                }
                Err(e) => {
                    warn!(
                        "Multicast NOTIFY for {} failed (attempt {}): {}",
                        service_type, attempt, e
                    );
                    if attempt < MAX_SEND_RETRIES {
                        tokio::time::sleep(Duration::from_millis(200 * attempt as u64)).await;
                    }
                }
            }
        }

        if !multicast_success {
            warn!(
                "Multicast NOTIFY for {} failed after {} attempts, trying unicast fallback",
                service_type, MAX_SEND_RETRIES
            );

            // Fall back to unicast broadcast on all interfaces with retry logic
            let mut unicast_success = false;
            for attempt in 1..=MAX_SEND_RETRIES {
                match network_manager
                    .send_unicast_fallback(&socket, message.as_bytes(), &socket.interfaces)
                    .await
                {
                    Ok(()) => {
                        info!("Successfully sent SSDP NOTIFY for {} via unicast fallback (attempt {})", service_type, attempt);
                        unicast_success = true;
                        break;
                    }
                    Err(e) => {
                        warn!(
                            "Unicast fallback for {} failed (attempt {}): {}",
                            service_type, attempt, e
                        );
                        if attempt < MAX_SEND_RETRIES {
                            tokio::time::sleep(Duration::from_millis(300 * attempt as u64)).await;
                        }
                    }
                }
            }

            if !unicast_success {
                error!(
                    "Both multicast and unicast fallback failed for {} after {} attempts each",
                    service_type, MAX_SEND_RETRIES
                );
            }
        }

        // Small delay between different service type announcements
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    info!("All SSDP NOTIFY announcements completed");
    Ok(())
}

async fn create_ssdp_response(state: &AppState, _ssdp_port: u16, service_type: &str) -> String {
    let server_ip = get_server_ip(state).await;
    let config = &state.config;

    let (st, usn) = match service_type {
        "upnp:rootdevice" => (
            "upnp:rootdevice".to_string(),
            format!("uuid:{}::upnp:rootdevice", config.server.uuid),
        ),
        "urn:schemas-upnp-org:device:MediaServer:1" => (
            "urn:schemas-upnp-org:device:MediaServer:1".to_string(),
            format!(
                "uuid:{}::urn:schemas-upnp-org:device:MediaServer:1",
                config.server.uuid
            ),
        ),
        "urn:schemas-upnp-org:service:ContentDirectory:1" => (
            "urn:schemas-upnp-org:service:ContentDirectory:1".to_string(),
            format!(
                "uuid:{}::urn:schemas-upnp-org:service:ContentDirectory:1",
                config.server.uuid
            ),
        ),
        _ => (
            "urn:schemas-upnp-org:device:MediaServer:1".to_string(),
            format!(
                "uuid:{}::urn:schemas-upnp-org:device:MediaServer:1",
                config.server.uuid
            ),
        ),
    };

    format!(
        "HTTP/1.1 200 OK\r\n\
        CACHE-CONTROL: max-age=1800\r\n\
        EXT:\r\n\
        LOCATION: http://{}:{}/description.xml\r\n\
        SERVER: VuIO/1.0 UPnP/1.0\r\n\
        ST: {}\r\n\
        USN: {}\r\n\
        \r\n",
        server_ip, config.server.port, st, usn
    )
}

async fn get_server_ip(state: &AppState) -> String {
    // 1. Use the primary interface detected at startup. This is the main path.
    if let Some(iface) = state.platform_info.get_primary_interface() {
        return iface.ip_address.to_string();
    }

    // 2. If the primary selection logic fails, log a clear warning and try the configured interface.
    warn!("Primary interface selection failed. This might happen if no suitable network connection (Ethernet/WiFi with a private IP) was found.");
    if state.config.server.interface != "0.0.0.0" && !state.config.server.interface.is_empty() {
        warn!(
            "Falling back to configured server interface: {}",
            state.config.server.interface
        );
        return state.config.server.interface.clone();
    }

    // 3. As a last resort, log a critical error and use localhost.
    // NO re-detection.
    error!("FATAL: Could not determine a usable server IP address from startup information.");
    error!("Please check your network connection and ensure you have a valid private IP (e.g., 192.168.x.x).");
    error!("Falling back to 127.0.0.1 - DLNA clients will NOT be able to connect.");
    "127.0.0.1".to_string()
}

// ============================================================================
// Docker-specific SSDP implementation
// ============================================================================

/// Unified SSDP service using a single socket for both M-SEARCH responses and NOTIFY announcements
/// This approach works better in Docker environments and follows MiniDLNA's pattern
async fn ssdp_unified_service_docker(
    state: AppState,
    network_manager: Arc<PlatformNetworkManager>,
) -> Result<()> {
    const MAX_SOCKET_RETRIES: u32 = 5;
    const RETRY_DELAY_MS: u64 = 2000;

    info!("Starting unified SSDP service for Docker environment");

    // Create SSDP socket with Docker-friendly configuration
    let mut socket = None;
    for attempt in 1..=MAX_SOCKET_RETRIES {
        let ssdp_config = SsdpConfig {
            primary_port: SSDP_PORT,
            fallback_ports: vec![], // Don't use fallback ports in Docker
            multicast_address: SSDP_MULTICAST_ADDR.parse().unwrap(),
            announce_interval: Duration::from_secs(state.config.network.announce_interval_seconds),
            max_retries: 3,
            interfaces: Vec::new(),
        };

        match create_docker_ssdp_socket(&network_manager, &ssdp_config).await {
            Ok(s) => {
                info!(
                    "Successfully created SSDP socket on port {} (attempt {})",
                    s.port, attempt
                );
                socket = Some(s);
                break;
            }
            Err(e) => {
                error!("Failed to create SSDP socket (attempt {}): {}", attempt, e);
                if attempt < MAX_SOCKET_RETRIES {
                    warn!("Retrying socket creation in {}ms...", RETRY_DELAY_MS);
                    tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                } else {
                    return Err(anyhow::anyhow!(
                        "SSDP socket creation failed after {} attempts: {}",
                        MAX_SOCKET_RETRIES,
                        e
                    ));
                }
            }
        }
    }

    let socket = socket.unwrap();
    let socket_port = socket.port;

    info!(
        "Successfully configured SSDP socket on port {}",
        socket_port
    );

    // Wrap socket in Arc<Mutex> for shared access
    let shared_socket = Arc::new(tokio::sync::Mutex::new(socket));

    // Start periodic NOTIFY announcements
    let announce_state = state.clone();
    let announce_manager = network_manager.clone();
    let announce_socket = shared_socket.clone();
    tokio::spawn(async move {
        ssdp_announcer_task_docker(announce_state, announce_manager, announce_socket).await;
    });

    // Main loop for handling M-SEARCH requests
    ssdp_responder_task_docker(state, network_manager, shared_socket).await
}

/// Create SSDP socket with Docker-specific configuration
async fn create_docker_ssdp_socket(
    network_manager: &PlatformNetworkManager,
    config: &SsdpConfig,
) -> Result<crate::platform::network::SsdpSocket> {
    use std::net::{IpAddr, Ipv4Addr};

    // In Docker, we need to bind to 0.0.0.0 to receive multicast traffic
    let _bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), SSDP_PORT);

    match network_manager.create_ssdp_socket_with_config(config).await {
        Ok(mut socket) => {
            // Join the SSDP multicast group - CRITICAL for receiving M-SEARCH requests
            let multicast_addr = SSDP_MULTICAST_ADDR.parse().unwrap();
            if let Err(e) = network_manager
                .join_multicast_group(&mut socket, multicast_addr, None)
                .await
            {
                error!("Failed to join SSDP multicast group: {}", e);
                return Err(e.into());
            }
            info!(
                "Successfully joined SSDP multicast group {}",
                SSDP_MULTICAST_ADDR
            );

            // Set additional socket options for Docker
            if let Err(e) = set_docker_socket_options(&mut socket).await {
                warn!("Failed to set Docker socket options: {}", e);
            }
            Ok(socket)
        }
        Err(e) => Err(e.into()),
    }
}

/// Set socket options optimized for Docker environment
async fn set_docker_socket_options(
    _socket: &mut crate::platform::network::SsdpSocket,
) -> Result<()> {
    // These would be platform-specific socket option calls
    // For now, we'll assume the PlatformNetworkManager handles basic options
    debug!("Setting Docker-optimized socket options");
    Ok(())
}

/// Task for handling SSDP announcements (Docker version)
async fn ssdp_announcer_task_docker(
    state: AppState,
    network_manager: Arc<PlatformNetworkManager>,
    socket: Arc<tokio::sync::Mutex<crate::platform::network::SsdpSocket>>,
) {
    let mut interval = interval(Duration::from_secs(ANNOUNCE_INTERVAL_SECS));
    let mut consecutive_failures = 0;
    const MAX_CONSECUTIVE_FAILURES: u32 = 3;

    // Send initial announcement immediately
    if let Err(e) = send_ssdp_notify_docker(&state, &network_manager, &socket).await {
        error!("Failed to send initial SSDP announcement: {}", e);
    } else {
        info!("Sent initial SSDP announcement");
    }

    loop {
        interval.tick().await;

        match send_ssdp_notify_docker(&state, &network_manager, &socket).await {
            Ok(()) => {
                consecutive_failures = 0;
                debug!("Successfully sent SSDP NOTIFY announcements");
            }
            Err(e) => {
                consecutive_failures += 1;
                error!(
                    "Failed to send SSDP NOTIFY (failure {}): {}",
                    consecutive_failures, e
                );

                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    error!("Too many consecutive SSDP announcement failures, resetting counter");
                    consecutive_failures = 0;
                    // Wait longer before next attempt
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        }
    }
}

/// Task for handling M-SEARCH responses (Docker version)
async fn ssdp_responder_task_docker(
    state: AppState,
    _network_manager: Arc<PlatformNetworkManager>,
    socket: Arc<tokio::sync::Mutex<crate::platform::network::SsdpSocket>>,
) -> Result<()> {
    let mut buf = vec![0u8; 2048];
    let mut consecutive_errors = 0;
    const MAX_CONSECUTIVE_ERRORS: u32 = 5;

    info!("SSDP M-SEARCH responder started on port {}", SSDP_PORT);

    loop {
        let recv_result = {
            let locked_socket = socket.lock().await;
            locked_socket.recv_from(&mut buf).await
        };

        match recv_result {
            Ok((len, addr)) => {
                consecutive_errors = 0;
                let request = String::from_utf8_lossy(&buf[..len]);

                if request.contains("M-SEARCH") {
                    debug!(
                        "Received M-SEARCH from {}: {}",
                        addr,
                        request.lines().next().unwrap_or("")
                    );

                    let response_types = determine_response_types_docker(&request);

                    if !response_types.is_empty() {
                        debug!(
                            "Sending {} SSDP response(s) to {}",
                            response_types.len(),
                            addr
                        );

                        for response_type in response_types {
                            let response = create_ssdp_response_docker(&state, response_type);

                            // Send response with retry logic
                            let mut sent = false;
                            for attempt in 1..=3 {
                                let send_result = {
                                    let locked_socket = socket.lock().await;
                                    locked_socket.send_to(response.as_bytes(), addr).await
                                };

                                match send_result {
                                    Ok(_) => {
                                        debug!(
                                            "Sent SSDP response to {} for {} (attempt {})",
                                            addr, response_type, attempt
                                        );
                                        sent = true;
                                        break;
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to send response to {} (attempt {}): {}",
                                            addr, attempt, e
                                        );
                                        if attempt < 3 {
                                            tokio::time::sleep(Duration::from_millis(100)).await;
                                        }
                                    }
                                }
                            }

                            if !sent {
                                error!(
                                    "Failed to send M-SEARCH response to {} after 3 attempts",
                                    addr
                                );
                            }

                            // Small delay between multiple responses
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    } else {
                        debug!("M-SEARCH from {} doesn't match our service types", addr);
                    }
                }
            }
            Err(e) => {
                consecutive_errors += 1;
                error!(
                    "Error receiving SSDP data (consecutive errors: {}): {}",
                    consecutive_errors, e
                );

                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    error!("Too many consecutive errors, attempting recovery");

                    // In a real implementation, you might try to recreate the socket here
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    consecutive_errors = 0;
                }
            }
        }
    }
}

/// Determine what SSDP response types to send based on M-SEARCH request (Docker version)
fn determine_response_types_docker(request: &str) -> Vec<&'static str> {
    let mut response_types = Vec::new();

    if request.contains("ssdp:all") {
        // Respond with all service types
        response_types.extend_from_slice(&[
            "upnp:rootdevice",
            "urn:schemas-upnp-org:device:MediaServer:1",
            "urn:schemas-upnp-org:service:ContentDirectory:1",
        ]);
    } else if request.contains("upnp:rootdevice") {
        response_types.push("upnp:rootdevice");
    } else if request.contains("urn:schemas-upnp-org:device:MediaServer") {
        response_types.push("urn:schemas-upnp-org:device:MediaServer:1");
    } else if request.contains("urn:schemas-upnp-org:service:ContentDirectory") {
        response_types.push("urn:schemas-upnp-org:service:ContentDirectory:1");
    } else if request.contains("ssdp:discover") {
        response_types.push("urn:schemas-upnp-org:device:MediaServer:1");
    }

    response_types
}

/// Send SSDP NOTIFY announcements (Docker version)
async fn send_ssdp_notify_docker(
    state: &AppState,
    network_manager: &PlatformNetworkManager,
    socket: &Arc<tokio::sync::Mutex<crate::platform::network::SsdpSocket>>,
) -> Result<()> {
    let server_ip = get_server_ip_docker(state);

    // Always use standard SSDP multicast port (1900) for announcements
    let multicast_addr: SocketAddr = format!("{}:{}", SSDP_MULTICAST_ADDR, SSDP_PORT).parse()?;

    let service_types = [
        (
            "upnp:rootdevice",
            format!("uuid:{}::upnp:rootdevice", state.config.server.uuid),
        ),
        (
            "urn:schemas-upnp-org:device:MediaServer:1",
            format!(
                "uuid:{}::urn:schemas-upnp-org:device:MediaServer:1",
                state.config.server.uuid
            ),
        ),
        (
            "urn:schemas-upnp-org:service:ContentDirectory:1",
            format!(
                "uuid:{}::urn:schemas-upnp-org:service:ContentDirectory:1",
                state.config.server.uuid
            ),
        ),
    ];

    for (nt, usn) in &service_types {
        let message = format!(
            "NOTIFY * HTTP/1.1\r\n\
            HOST: {}:{}\r\n\
            CACHE-CONTROL: max-age=1800\r\n\
            LOCATION: http://{}:{}/description.xml\r\n\
            NT: {}\r\n\
            NTS: ssdp:alive\r\n\
            SERVER: VuIO/1.0 UPnP/1.0\r\n\
            USN: {}\r\n\
            BOOTID.UPNP.ORG: 1\r\n\
            CONFIGID.UPNP.ORG: 1\r\n\
            \r\n",
            SSDP_MULTICAST_ADDR,
            SSDP_PORT, // Use standard multicast port in HOST header
            server_ip,
            state.config.server.port, // Use your server port for LOCATION
            nt,
            usn
        );

        // Try multicast first
        let mut success = false;
        for attempt in 1..=3 {
            let locked_socket = socket.lock().await;
            let result = network_manager
                .send_multicast(&*locked_socket, message.as_bytes(), multicast_addr)
                .await;
            drop(locked_socket);

            match result {
                Ok(()) => {
                    debug!(
                        "Successfully sent NOTIFY for {} via multicast to {}:{}",
                        nt, SSDP_MULTICAST_ADDR, SSDP_PORT
                    );
                    success = true;
                    break;
                }
                Err(e) => {
                    warn!(
                        "Multicast NOTIFY failed for {} (attempt {}): {}",
                        nt, attempt, e
                    );
                    if attempt < 3 {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            }
        }

        if !success {
            warn!(
                "All multicast attempts failed for {}, trying unicast fallback",
                nt
            );

            // Fallback to unicast broadcast
            let locked_socket = socket.lock().await;
            let interfaces = locked_socket.interfaces.clone();
            let result = network_manager
                .send_unicast_fallback(&*locked_socket, message.as_bytes(), &interfaces)
                .await;
            drop(locked_socket);

            match result {
                Ok(()) => debug!("Successfully sent NOTIFY for {} via unicast fallback", nt),
                Err(e) => error!("Both multicast and unicast failed for {}: {}", nt, e),
            }
        }

        // Small delay between announcements
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    Ok(())
}

/// Create SSDP response message (Docker version)
fn create_ssdp_response_docker(state: &AppState, service_type: &str) -> String {
    let server_ip = get_server_ip_docker(state);

    let (st, usn) = match service_type {
        "upnp:rootdevice" => (
            "upnp:rootdevice",
            format!("uuid:{}::upnp:rootdevice", state.config.server.uuid),
        ),
        "urn:schemas-upnp-org:device:MediaServer:1" => (
            "urn:schemas-upnp-org:device:MediaServer:1",
            format!(
                "uuid:{}::urn:schemas-upnp-org:device:MediaServer:1",
                state.config.server.uuid
            ),
        ),
        "urn:schemas-upnp-org:service:ContentDirectory:1" => (
            "urn:schemas-upnp-org:service:ContentDirectory:1",
            format!(
                "uuid:{}::urn:schemas-upnp-org:service:ContentDirectory:1",
                state.config.server.uuid
            ),
        ),
        _ => (
            "urn:schemas-upnp-org:device:MediaServer:1",
            format!(
                "uuid:{}::urn:schemas-upnp-org:device:MediaServer:1",
                state.config.server.uuid
            ),
        ),
    };

    format!(
        "HTTP/1.1 200 OK\r\n\
        CACHE-CONTROL: max-age=1800\r\n\
        DATE: {}\r\n\
        EXT:\r\n\
        LOCATION: http://{}:{}/description.xml\r\n\
        SERVER: VuIO/1.0 UPnP/1.0\r\n\
        ST: {}\r\n\
        USN: {}\r\n\
        BOOTID.UPNP.ORG: 1\r\n\
        CONFIGID.UPNP.ORG: 1\r\n\
        \r\n",
        chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S GMT"),
        server_ip,
        state.config.server.port, // Use your server port for LOCATION
        st,
        usn
    )
}

/// Get server IP address with Docker-aware logic
fn get_server_ip_docker(state: &AppState) -> String {
    // 1. Check if server IP is explicitly configured (important for Docker)
    if let Some(server_ip) = &state.config.server.ip {
        if !server_ip.is_empty() && server_ip != "0.0.0.0" {
            debug!("Using configured server IP: {}", server_ip);
            return server_ip.clone();
        }
    }

    // 2. In Docker, try to detect the container's IP address
    if let Ok(docker_ip) = std::env::var("DOCKER_HOST_IP") {
        if !docker_ip.is_empty() {
            info!("Using Docker host IP from environment: {}", docker_ip);
            return docker_ip;
        }
    }

    // 3. Use the primary interface if available
    if let Some(iface) = state.platform_info.get_primary_interface() {
        debug!("Using primary interface IP: {}", iface.ip_address);
        return iface.ip_address.to_string();
    }

    // 4. Try to get the default gateway (Docker bridge) IP
    if let Ok(gateway_ip) = get_default_gateway_ip_docker() {
        warn!("Using default gateway IP as fallback: {}", gateway_ip);
        return gateway_ip;
    }

    // 5. Final fallback - this will only work for local connections
    error!("Could not determine server IP address!");
    error!("For Docker deployment, set DOCKER_HOST_IP environment variable");
    error!("or configure server.ip in your configuration file");
    "172.17.0.1".to_string() // Docker default bridge IP
}

/// Get default gateway IP (useful in Docker environments)
fn get_default_gateway_ip_docker() -> Result<String> {
    // This is a simplified implementation
    // In practice, you'd parse /proc/net/route or use system calls

    // Try common Docker gateway addresses
    let common_gateways = ["172.17.0.1", "172.18.0.1", "192.168.1.1"];

    for gateway in &common_gateways {
        // In a real implementation, you'd ping or check connectivity
        if gateway.starts_with("172.17") {
            return Ok(gateway.to_string());
        }
    }

    Err(anyhow::anyhow!("No default gateway found"))
}

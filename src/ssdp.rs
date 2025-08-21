use crate::state::AppState;
use crate::platform::network::{NetworkManager, SsdpConfig, PlatformNetworkManager};
use anyhow::Result;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

const SSDP_MULTICAST_ADDR: &str = "239.255.255.250";
const SSDP_PORT: u16 = 1900;
const ANNOUNCE_INTERVAL_SECS: u64 = 300; // Announce every 5 minutes

pub fn run_ssdp_service(state: AppState) -> Result<()> {
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

async fn ssdp_search_responder(state: AppState, network_manager: Arc<PlatformNetworkManager>) -> Result<()> {
    const MAX_SOCKET_RETRIES: u32 = 3;
    const MAX_MULTICAST_RETRIES: u32 = 5;
    const RETRY_DELAY_MS: u64 = 1000;

    // Create SSDP socket with retry logic
    let mut socket = None;
    for attempt in 1..=MAX_SOCKET_RETRIES {
        let ssdp_config = SsdpConfig::default();
        match network_manager.create_ssdp_socket_with_config(&ssdp_config).await {
            Ok(s) => {
                info!("Successfully created SSDP socket on port {} (attempt {})", s.port, attempt);
                socket = Some(s);
                break;
            }
            Err(e) => {
                error!("Failed to create SSDP socket (attempt {}): {}", attempt, e);
                if attempt < MAX_SOCKET_RETRIES {
                    warn!("Retrying socket creation in {}ms...", RETRY_DELAY_MS);
                    tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                } else {
                    return Err(anyhow::anyhow!("SSDP socket creation failed after {} attempts: {}", MAX_SOCKET_RETRIES, e));
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
        match network_manager.join_multicast_group(&mut socket, multicast_addr, primary_interface.as_ref()).await {
            Ok(()) => {
                info!("Successfully joined SSDP multicast group on port {} (attempt {})", socket_port, attempt);
                multicast_enabled = true;
                break;
            }
            Err(e) => {
                warn!("Failed to join multicast group (attempt {}): {}", attempt, e);
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
                        debug!("Sending {} SSDP response(s) to {} for types: {:?}", response_types.len(), addr, response_types);
                        
                        let mut all_responses_sent = true;
                        let response_count = response_types.len();
                        for response_type in response_types {
                            let response = create_ssdp_response(&state, socket_port, response_type).await;
                            debug!("Sending SSDP response to {} ({}): {}", addr, response_type, response.trim());
                            
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
                                            tokio::time::sleep(Duration::from_millis(100 * (1 << retry))).await;
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
                        info!("M-SEARCH request from {} doesn't match our service types, ignoring", addr);
                    }
                }
            }
            Err(e) => {
                consecutive_errors += 1;
                error!("Error receiving SSDP data (consecutive errors: {}): {}", consecutive_errors, e);
                
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    error!("Too many consecutive errors ({}), attempting to recreate socket", MAX_CONSECUTIVE_ERRORS);
                    
                    // Try to recreate the socket
                    let ssdp_config = SsdpConfig::default();
                    match network_manager.create_ssdp_socket_with_config(&ssdp_config).await {
                        Ok(new_socket) => {
                            info!("Successfully recreated SSDP socket on port {}", new_socket.port);
                            socket = new_socket;
                            consecutive_errors = 0;
                            
                            // Try to rejoin multicast group
                            if let Err(e) = network_manager.join_multicast_group(&mut socket, multicast_addr, primary_interface.as_ref()).await {
                                warn!("Failed to rejoin multicast group after socket recreation: {}", e);
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
                error!("Failed to send SSDP NOTIFY (failure {}): {}", consecutive_failures, e);
                
                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    error!("Too many consecutive SSDP announcement failures ({})", MAX_CONSECUTIVE_FAILURES);
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
        match network_manager.create_ssdp_socket_with_config(&ssdp_config).await {
            Ok(s) => {
                socket = Some(s);
                break;
            }
            Err(e) => {
                warn!("Failed to create announcement socket (attempt {}): {}", attempt, e);
                if attempt < MAX_SOCKET_CREATION_RETRIES {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                } else {
                    error!("Failed to create announcement socket after {} attempts", MAX_SOCKET_CREATION_RETRIES);
                    return Err(anyhow::anyhow!("Announcement socket creation failed: {}", e));
                }
            }
        }
    }

    let mut socket = socket.unwrap();

    // Enable multicast on the announcement socket, using the primary interface from AppState
    let multicast_addr_ip = SSDP_MULTICAST_ADDR.parse().unwrap();
    let primary_interface = state.platform_info.get_primary_interface().cloned();
    if let Err(e) = network_manager.join_multicast_group(&mut socket, multicast_addr_ip, primary_interface.as_ref()).await {
        warn!("Failed to enable multicast on announcement socket: {}", e);
    }

    let server_ip = get_server_ip(state).await;
    let config = &state.config;

    // Send NOTIFY for multiple service types
    let service_types = [
        "upnp:rootdevice",
        "urn:schemas-upnp-org:device:MediaServer:1",
        "urn:schemas-upnp-org:service:ContentDirectory:1"
    ];
    
    let multicast_addr = format!("{}:{}", SSDP_MULTICAST_ADDR, SSDP_PORT).parse::<SocketAddr>()?;
    
    for service_type in &service_types {
        let (nt, usn) = match *service_type {
            "upnp:rootdevice" => (
                "upnp:rootdevice".to_string(),
                format!("uuid:{}::upnp:rootdevice", config.server.uuid)
            ),
            "urn:schemas-upnp-org:device:MediaServer:1" => (
                "urn:schemas-upnp-org:device:MediaServer:1".to_string(),
                format!("uuid:{}::urn:schemas-upnp-org:device:MediaServer:1", config.server.uuid)
            ),
            "urn:schemas-upnp-org:service:ContentDirectory:1" => (
                "urn:schemas-upnp-org:service:ContentDirectory:1".to_string(),
                format!("uuid:{}::urn:schemas-upnp-org:service:ContentDirectory:1", config.server.uuid)
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
            USN: {}\r\n\r\n",
            SSDP_MULTICAST_ADDR, SSDP_PORT,
            server_ip, config.server.port, nt, usn
        );

        // Try multicast first with retry logic
        let mut multicast_success = false;
        for attempt in 1..=MAX_SEND_RETRIES {
            match network_manager.send_multicast(&socket, message.as_bytes(), multicast_addr).await {
                Ok(()) => {
                    info!("Successfully sent SSDP NOTIFY for {} via multicast (attempt {})", service_type, attempt);
                    multicast_success = true;
                    break;
                }
                Err(e) => {
                    warn!("Multicast NOTIFY for {} failed (attempt {}): {}", service_type, attempt, e);
                    if attempt < MAX_SEND_RETRIES {
                        tokio::time::sleep(Duration::from_millis(200 * attempt as u64)).await;
                    }
                }
            }
        }
        
        if !multicast_success {
            warn!("Multicast NOTIFY for {} failed after {} attempts, trying unicast fallback", service_type, MAX_SEND_RETRIES);
            
            // Fall back to unicast broadcast on all interfaces with retry logic
            let mut unicast_success = false;
            for attempt in 1..=MAX_SEND_RETRIES {
                match network_manager.send_unicast_fallback(&socket, message.as_bytes(), &socket.interfaces).await {
                    Ok(()) => {
                        info!("Successfully sent SSDP NOTIFY for {} via unicast fallback (attempt {})", service_type, attempt);
                        unicast_success = true;
                        break;
                    }
                    Err(e) => {
                        warn!("Unicast fallback for {} failed (attempt {}): {}", service_type, attempt, e);
                        if attempt < MAX_SEND_RETRIES {
                            tokio::time::sleep(Duration::from_millis(300 * attempt as u64)).await;
                        }
                    }
                }
            }
            
            if !unicast_success {
                error!("Both multicast and unicast fallback failed for {} after {} attempts each", service_type, MAX_SEND_RETRIES);
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
            format!("uuid:{}::upnp:rootdevice", config.server.uuid)
        ),
        "urn:schemas-upnp-org:device:MediaServer:1" => (
            "urn:schemas-upnp-org:device:MediaServer:1".to_string(),
            format!("uuid:{}::urn:schemas-upnp-org:device:MediaServer:1", config.server.uuid)
        ),
        "urn:schemas-upnp-org:service:ContentDirectory:1" => (
            "urn:schemas-upnp-org:service:ContentDirectory:1".to_string(),
            format!("uuid:{}::urn:schemas-upnp-org:service:ContentDirectory:1", config.server.uuid)
        ),
        _ => (
            "urn:schemas-upnp-org:device:MediaServer:1".to_string(),
            format!("uuid:{}::urn:schemas-upnp-org:device:MediaServer:1", config.server.uuid)
        )
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
        warn!("Falling back to configured server interface: {}", state.config.server.interface);
        return state.config.server.interface.clone();
    }
    
    // 3. As a last resort, log a critical error and use localhost.
    // NO re-detection.
    error!("FATAL: Could not determine a usable server IP address from startup information.");
    error!("Please check your network connection and ensure you have a valid private IP (e.g., 192.168.x.x).");
    error!("Falling back to 127.0.0.1 - DLNA clients will NOT be able to connect.");
    "127.0.0.1".to_string()
}
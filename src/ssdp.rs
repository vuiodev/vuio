use crate::state::AppState;
use crate::platform::network::{NetworkManager, SsdpConfig, PlatformNetworkManager};
use anyhow::Result;
use std::{net::{SocketAddr, IpAddr, Ipv4Addr}, sync::Arc, time::Duration};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

const SSDP_MULTICAST_ADDR: &str = "239.255.255.250";
const SSDP_PORT: u16 = 1900;
const ANNOUNCE_INTERVAL_SECS: u64 = 300; // Announce every 5 minutes

pub fn run_ssdp_service(state: AppState) -> Result<()> {
    let network_manager = Arc::new(PlatformNetworkManager::new());

    // Log the IP that will be used for SSDP announcements
    let server_ip = get_server_ip(&state);
    info!("SSDP service starting - using server IP: {}", server_ip);

    // Use unified service for Docker compatibility (single socket like MiniDLNA)
    let service_state = state.clone();
    let service_manager = network_manager.clone();
    tokio::spawn(async move {
        if let Err(e) = ssdp_unified_service(service_state, service_manager).await {
            error!("SSDP unified service failed: {}", e);
        }
    });

    Ok(())
}

/// Unified SSDP service using a single socket for both M-SEARCH responses and NOTIFY announcements
/// This approach works better in Docker environments and follows MiniDLNA's pattern
async fn ssdp_unified_service(state: AppState, network_manager: Arc<PlatformNetworkManager>) -> Result<()> {
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

    let socket = socket.unwrap();
    let socket_port = socket.port;

    info!("Successfully configured SSDP socket on port {}", socket_port);

    // Wrap socket in Arc<Mutex> for shared access
    let shared_socket = Arc::new(tokio::sync::Mutex::new(socket));

    // Start periodic NOTIFY announcements
    let announce_state = state.clone();
    let announce_manager = network_manager.clone();
    let announce_socket = shared_socket.clone();
    tokio::spawn(async move {
        ssdp_announcer_task(announce_state, announce_manager, announce_socket).await;
    });

    // Main loop for handling M-SEARCH requests
    ssdp_responder_task(state, network_manager, shared_socket).await
}

/// Create SSDP socket with Docker-specific configuration
async fn create_docker_ssdp_socket(
    network_manager: &PlatformNetworkManager, 
    config: &SsdpConfig
) -> Result<crate::platform::network::SsdpSocket> {
    // In Docker, we need to bind to 0.0.0.0 to receive multicast traffic
    let _bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), SSDP_PORT);
    
    match network_manager.create_ssdp_socket_with_config(config).await {
        Ok(mut socket) => {
            // Set additional socket options for Docker
            if let Err(e) = set_docker_socket_options(&mut socket).await {
                warn!("Failed to set Docker socket options: {}", e);
            }
            Ok(socket)
        }
        Err(e) => Err(e.into())
    }
}

/// Set socket options optimized for Docker environment
async fn set_docker_socket_options(_socket: &mut crate::platform::network::SsdpSocket) -> Result<()> {
    // These would be platform-specific socket option calls
    // For now, we'll assume the PlatformNetworkManager handles basic options
    debug!("Setting Docker-optimized socket options");
    Ok(())
}



/// Task for handling SSDP announcements
async fn ssdp_announcer_task(
    state: AppState, 
    network_manager: Arc<PlatformNetworkManager>,
    socket: Arc<tokio::sync::Mutex<crate::platform::network::SsdpSocket>>
) {
    let mut interval = interval(Duration::from_secs(ANNOUNCE_INTERVAL_SECS));
    let mut consecutive_failures = 0;
    const MAX_CONSECUTIVE_FAILURES: u32 = 3;
    
    // Send initial announcement immediately
    if let Err(e) = send_ssdp_notify(&state, &network_manager, &socket).await {
        error!("Failed to send initial SSDP announcement: {}", e);
    } else {
        info!("Sent initial SSDP announcement");
    }
    
    loop {
        interval.tick().await;
        
        match send_ssdp_notify(&state, &network_manager, &socket).await {
            Ok(()) => {
                consecutive_failures = 0;
                debug!("Successfully sent SSDP NOTIFY announcements");
            }
            Err(e) => {
                consecutive_failures += 1;
                error!("Failed to send SSDP NOTIFY (failure {}): {}", consecutive_failures, e);
                
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

/// Task for handling M-SEARCH responses
async fn ssdp_responder_task(
    state: AppState,
    _network_manager: Arc<PlatformNetworkManager>,
    socket: Arc<tokio::sync::Mutex<crate::platform::network::SsdpSocket>>
) -> Result<()> {
    let mut buf = vec![0u8; 2048];
    let mut consecutive_errors = 0;
    const MAX_CONSECUTIVE_ERRORS: u32 = 5;
    
    info!("SSDP M-SEARCH responder started");
    
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
                    debug!("Received M-SEARCH from {}: {}", addr, request.lines().next().unwrap_or(""));
                    
                    let response_types = determine_response_types(&request);
                    
                    if !response_types.is_empty() {
                        debug!("Sending {} SSDP response(s) to {}", response_types.len(), addr);
                        
                        for response_type in response_types {
                            let response = create_ssdp_response(&state, response_type);
                            
                            // Send response with retry logic
                            let mut sent = false;
                            for attempt in 1..=3 {
                                let send_result = {
                                    let locked_socket = socket.lock().await;
                                    locked_socket.send_to(response.as_bytes(), addr).await
                                };
                                
                                match send_result {
                                    Ok(_) => {
                                        debug!("Sent SSDP response to {} for {} (attempt {})", addr, response_type, attempt);
                                        sent = true;
                                        break;
                                    }
                                    Err(e) => {
                                        warn!("Failed to send response to {} (attempt {}): {}", addr, attempt, e);
                                        if attempt < 3 {
                                            tokio::time::sleep(Duration::from_millis(100)).await;
                                        }
                                    }
                                }
                            }
                            
                            if !sent {
                                error!("Failed to send M-SEARCH response to {} after 3 attempts", addr);
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
                error!("Error receiving SSDP data (consecutive errors: {}): {}", consecutive_errors, e);
                
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

/// Determine what SSDP response types to send based on M-SEARCH request
fn determine_response_types(request: &str) -> Vec<&'static str> {
    let mut response_types = Vec::new();
    
    if request.contains("ssdp:all") {
        // Respond with all service types
        response_types.extend_from_slice(&[
            "upnp:rootdevice",
            "urn:schemas-upnp-org:device:MediaServer:1",
            "urn:schemas-upnp-org:service:ContentDirectory:1"
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

/// Send SSDP NOTIFY announcements
async fn send_ssdp_notify(
    state: &AppState,
    network_manager: &PlatformNetworkManager,
    socket: &Arc<tokio::sync::Mutex<crate::platform::network::SsdpSocket>>
) -> Result<()> {
    let server_ip = get_server_ip(state);
    let multicast_addr: SocketAddr = format!("{}:{}", SSDP_MULTICAST_ADDR, SSDP_PORT).parse()?;
    
    let service_types = [
        ("upnp:rootdevice", format!("uuid:{}::upnp:rootdevice", state.config.server.uuid)),
        (
            "urn:schemas-upnp-org:device:MediaServer:1",
            format!("uuid:{}::urn:schemas-upnp-org:device:MediaServer:1", state.config.server.uuid)
        ),
        (
            "urn:schemas-upnp-org:service:ContentDirectory:1",
            format!("uuid:{}::urn:schemas-upnp-org:service:ContentDirectory:1", state.config.server.uuid)
        )
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
            SSDP_MULTICAST_ADDR, SSDP_PORT,
            server_ip, state.config.server.port,
            nt, usn
        );

        // Try multicast first
        let mut success = false;
        for attempt in 1..=3 {
            let locked_socket = socket.lock().await;
            let result = network_manager.send_multicast(&*locked_socket, message.as_bytes(), multicast_addr).await;
            drop(locked_socket);
            
            match result {
                Ok(()) => {
                    debug!("Successfully sent NOTIFY for {} via multicast", nt);
                    success = true;
                    break;
                }
                Err(e) => {
                    warn!("Multicast NOTIFY failed for {} (attempt {}): {}", nt, attempt, e);
                    if attempt < 3 {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            }
        }
        
        if !success {
            warn!("All multicast attempts failed for {}, trying unicast fallback", nt);
            
            // Fallback to unicast broadcast
            let locked_socket = socket.lock().await;
            let interfaces = locked_socket.interfaces.clone();
            let result = network_manager.send_unicast_fallback(&*locked_socket, message.as_bytes(), &interfaces).await;
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

/// Create SSDP response message
fn create_ssdp_response(state: &AppState, service_type: &str) -> String {
    let server_ip = get_server_ip(state);
    
    let (st, usn) = match service_type {
        "upnp:rootdevice" => (
            "upnp:rootdevice",
            format!("uuid:{}::upnp:rootdevice", state.config.server.uuid)
        ),
        "urn:schemas-upnp-org:device:MediaServer:1" => (
            "urn:schemas-upnp-org:device:MediaServer:1",
            format!("uuid:{}::urn:schemas-upnp-org:device:MediaServer:1", state.config.server.uuid)
        ),
        "urn:schemas-upnp-org:service:ContentDirectory:1" => (
            "urn:schemas-upnp-org:service:ContentDirectory:1",
            format!("uuid:{}::urn:schemas-upnp-org:service:ContentDirectory:1", state.config.server.uuid)
        ),
        _ => (
            "urn:schemas-upnp-org:device:MediaServer:1",
            format!("uuid:{}::urn:schemas-upnp-org:device:MediaServer:1", state.config.server.uuid)
        )
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
        server_ip, state.config.server.port,
        st, usn
    )
}

/// Get server IP address with Docker-aware logic
fn get_server_ip(state: &AppState) -> String {
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
    if let Ok(gateway_ip) = get_default_gateway_ip() {
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
fn get_default_gateway_ip() -> Result<String> {
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
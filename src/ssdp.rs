use crate::config::AppConfig;
use crate::platform::network::{NetworkManager, PlatformNetworkManager, SsdpConfig, SsdpSocket};
use crate::platform::NetworkInterface;
use crate::state::AppState;
use anyhow::Result;
use async_trait::async_trait;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

const SSDP_MULTICAST_ADDR: &str = "239.255.255.250";
const SSDP_PORT: u16 = 1900;
const ANNOUNCE_INTERVAL_SECS: u64 = 300; // Announce every 5 minutes

/// Platform-specific adapter for SSDP service behavior
#[async_trait]
pub trait SsdpPlatformAdapter: Send + Sync {
    /// Configure socket with platform-specific options
    async fn configure_socket(&self, socket: &mut SsdpSocket) -> Result<()>;
    
    /// Get network interfaces suitable for this platform
    async fn get_suitable_interfaces(&self, network_manager: &dyn NetworkManager) -> Result<Vec<NetworkInterface>>;
    
    /// Determine if this platform should bind to a specific interface
    fn should_bind_to_specific_interface(&self) -> bool;
    
    /// Get server IP address using platform-specific logic
    fn get_server_ip(&self, state: &AppState) -> String;
    
    /// Get platform-specific SSDP configuration
    fn get_ssdp_config(&self, state: &AppState) -> SsdpConfig;
}

/// Unified SSDP service that works across all platforms
pub struct UnifiedSsdpService {
    network_manager: Arc<dyn NetworkManager>,
    platform_adapter: Box<dyn SsdpPlatformAdapter>,
    state: AppState,
}

impl UnifiedSsdpService {
    /// Create a new unified SSDP service with the appropriate platform adapter
    pub fn new(state: AppState) -> Self {
        let network_manager = Arc::new(PlatformNetworkManager::new());
        let platform_adapter: Box<dyn SsdpPlatformAdapter> = if AppConfig::is_running_in_docker() {
            Box::new(DockerSsdpAdapter::new())
        } else {
            #[cfg(target_os = "windows")]
            {
                Box::new(WindowsSsdpAdapter::new())
            }
            #[cfg(not(target_os = "windows"))]
            {
                Box::new(UnixSsdpAdapter::new())
            }
        };

        Self {
            network_manager,
            platform_adapter,
            state,
        }
    }

    /// Start the unified SSDP service
    pub async fn start(&self) -> Result<()> {
        info!("Starting unified SSDP service");
        
        let server_ip = self.platform_adapter.get_server_ip(&self.state);
        info!("SSDP service using server IP: {}", server_ip);

        // Create SSDP socket with platform-specific configuration
        let ssdp_config = self.platform_adapter.get_ssdp_config(&self.state);
        let mut socket = self.network_manager.create_ssdp_socket_with_config(&ssdp_config).await
            .map_err(|e| anyhow::anyhow!("Failed to create SSDP socket: {}", e))?;

        // Apply platform-specific socket configuration
        self.platform_adapter.configure_socket(&mut socket).await?;

        // Join multicast group
        let multicast_addr = SSDP_MULTICAST_ADDR.parse().unwrap();
        let primary_interface = self.state.platform_info.get_primary_interface().cloned();
        if let Err(e) = self.network_manager
            .join_multicast_group(&mut socket, multicast_addr, primary_interface.as_ref())
            .await
        {
            warn!("Failed to join multicast group: {}", e);
        }

        let socket = Arc::new(tokio::sync::Mutex::new(socket));

        // Start M-SEARCH responder task
        let responder_state = self.state.clone();
        let responder_manager = self.network_manager.clone();
        let responder_socket = socket.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::search_responder_task(responder_state, responder_manager, responder_socket).await {
                error!("SSDP search responder failed: {}", e);
            }
        });

        // Start announcement task
        let announcer_state = self.state.clone();
        let announcer_manager = self.network_manager.clone();
        let announcer_socket = socket.clone();
        tokio::spawn(async move {
            Self::announcer_task(announcer_state, announcer_manager, announcer_socket).await;
        });

        info!("Unified SSDP service started successfully");
        Ok(())
    }

    /// Task for handling M-SEARCH requests
    async fn search_responder_task(
        state: AppState,
        network_manager: Arc<dyn NetworkManager>,
        socket: Arc<tokio::sync::Mutex<SsdpSocket>>,
    ) -> Result<()> {
        let mut buf = vec![0u8; 2048];
        let mut consecutive_errors = 0;
        const MAX_CONSECUTIVE_ERRORS: u32 = 10;

        loop {
            let (len, addr) = {
                let mut socket_guard = socket.lock().await;
                match socket_guard.recv_from(&mut buf).await {
                    Ok(result) => result,
                    Err(e) => {
                        consecutive_errors += 1;
                        error!("Error receiving SSDP data (consecutive errors: {}): {}", consecutive_errors, e);
                        
                        if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                            error!("Too many consecutive errors, recreating socket");
                            let ssdp_config = SsdpConfig::default();
                            match network_manager.create_ssdp_socket_with_config(&ssdp_config).await {
                                Ok(new_socket) => {
                                    *socket_guard = new_socket;
                                    consecutive_errors = 0;
                                }
                                Err(e) => {
                                    error!("Failed to recreate socket: {}", e);
                                    return Err(anyhow::anyhow!("Socket recreation failed: {}", e));
                                }
                            }
                        }
                        
                        tokio::time::sleep(Duration::from_millis(1000)).await;
                        continue;
                    }
                }
            };

            consecutive_errors = 0;
            let request = String::from_utf8_lossy(&buf[..len]);

            if request.contains("M-SEARCH") {
                debug!("Received M-SEARCH from {}", addr);
                Self::handle_msearch_request(&state, &socket, &request, addr).await;
            }
        }
    }

    /// Handle M-SEARCH request and send appropriate responses
    async fn handle_msearch_request(
        state: &AppState,
        socket: &Arc<tokio::sync::Mutex<SsdpSocket>>,
        request: &str,
        addr: SocketAddr,
    ) {
        let mut response_types = Vec::new();

        if request.contains("ssdp:all") {
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

        let response_count = response_types.len();
        for response_type in response_types {
            let response = Self::create_ssdp_response(state, response_type);
            
            let socket_guard = socket.lock().await;
            for retry in 0..3 {
                match socket_guard.send_to(response.as_bytes(), addr).await {
                    Ok(_) => {
                        debug!("Successfully sent M-SEARCH response to {} for {}", addr, response_type);
                        break;
                    }
                    Err(e) => {
                        warn!("Failed to send M-SEARCH response (attempt {}): {}", retry + 1, e);
                        if retry < 2 {
                            tokio::time::sleep(Duration::from_millis(100 * (1 << retry))).await;
                        }
                    }
                }
            }
            
            if response_count > 1 {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }

    /// Task for periodic SSDP announcements
    async fn announcer_task(
        state: AppState,
        network_manager: Arc<dyn NetworkManager>,
        socket: Arc<tokio::sync::Mutex<SsdpSocket>>,
    ) {
        let mut interval = interval(Duration::from_secs(ANNOUNCE_INTERVAL_SECS));
        let mut consecutive_failures = 0;
        const MAX_CONSECUTIVE_FAILURES: u32 = 5;

        loop {
            interval.tick().await;

            match Self::send_ssdp_announcements(&state, &network_manager, &socket).await {
                Ok(()) => {
                    consecutive_failures = 0;
                }
                Err(e) => {
                    consecutive_failures += 1;
                    error!("Failed to send SSDP announcements (failure {}): {}", consecutive_failures, e);
                    
                    if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        error!("Too many consecutive announcement failures, resetting counter");
                        consecutive_failures = 0;
                        tokio::time::sleep(Duration::from_secs(30)).await;
                    }
                }
            }
        }
    }

    /// Send SSDP NOTIFY announcements
    async fn send_ssdp_announcements(
        state: &AppState,
        network_manager: &Arc<dyn NetworkManager>,
        socket: &Arc<tokio::sync::Mutex<SsdpSocket>>,
    ) -> Result<()> {
        info!("Sending SSDP NOTIFY announcements");

        let server_ip = Self::get_server_ip(state);
        let service_types = [
            "upnp:rootdevice",
            "urn:schemas-upnp-org:device:MediaServer:1",
            "urn:schemas-upnp-org:service:ContentDirectory:1",
        ];

        let multicast_addr = format!("{}:{}", SSDP_MULTICAST_ADDR, SSDP_PORT).parse::<SocketAddr>()?;

        for service_type in &service_types {
            let message = Self::create_notify_message(state, &server_ip, service_type);
            
            let socket_guard = socket.lock().await;
            match network_manager.send_multicast(&*socket_guard, message.as_bytes(), multicast_addr).await {
                Ok(()) => {
                    info!("Successfully sent SSDP NOTIFY for {}", service_type);
                }
                Err(e) => {
                    warn!("Multicast NOTIFY for {} failed: {}, trying unicast fallback", service_type, e);
                    
                    if let Err(e) = network_manager.send_unicast_fallback(&*socket_guard, message.as_bytes(), &socket_guard.interfaces).await {
                        error!("Both multicast and unicast fallback failed for {}: {}", service_type, e);
                    }
                }
            }
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(())
    }

    /// Create SSDP response message
    fn create_ssdp_response(state: &AppState, service_type: &str) -> String {
        let server_ip = Self::get_server_ip(state);
        let config = &state.config;

        let (st, usn) = match service_type {
            "upnp:rootdevice" => (
                "upnp:rootdevice".to_string(),
                format!("uuid:{}::upnp:rootdevice", config.server.uuid),
            ),
            "urn:schemas-upnp-org:device:MediaServer:1" => (
                "urn:schemas-upnp-org:device:MediaServer:1".to_string(),
                format!("uuid:{}::urn:schemas-upnp-org:device:MediaServer:1", config.server.uuid),
            ),
            "urn:schemas-upnp-org:service:ContentDirectory:1" => (
                "urn:schemas-upnp-org:service:ContentDirectory:1".to_string(),
                format!("uuid:{}::urn:schemas-upnp-org:service:ContentDirectory:1", config.server.uuid),
            ),
            _ => (
                "urn:schemas-upnp-org:device:MediaServer:1".to_string(),
                format!("uuid:{}::urn:schemas-upnp-org:device:MediaServer:1", config.server.uuid),
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

    /// Create SSDP NOTIFY message
    fn create_notify_message(state: &AppState, server_ip: &str, service_type: &str) -> String {
        let config = &state.config;

        let (nt, usn) = match service_type {
            "upnp:rootdevice" => (
                "upnp:rootdevice".to_string(),
                format!("uuid:{}::upnp:rootdevice", config.server.uuid),
            ),
            "urn:schemas-upnp-org:device:MediaServer:1" => (
                "urn:schemas-upnp-org:device:MediaServer:1".to_string(),
                format!("uuid:{}::urn:schemas-upnp-org:device:MediaServer:1", config.server.uuid),
            ),
            "urn:schemas-upnp-org:service:ContentDirectory:1" => (
                "urn:schemas-upnp-org:service:ContentDirectory:1".to_string(),
                format!("uuid:{}::urn:schemas-upnp-org:service:ContentDirectory:1", config.server.uuid),
            ),
            _ => return String::new(),
        };

        format!(
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
        )
    }

    /// Get server IP address using unified logic from AppState
    fn get_server_ip(state: &AppState) -> String {
        state.get_server_ip()
    }
}

/// Main entry point for SSDP service - now uses unified implementation
pub fn run_ssdp_service(state: AppState) -> Result<()> {
    info!("Starting unified SSDP service");
    
    let service = UnifiedSsdpService::new(state);
    
    tokio::spawn(async move {
        if let Err(e) = service.start().await {
            error!("Unified SSDP service failed: {}", e);
        }
    });
    
    Ok(())
}

/// Windows-specific SSDP platform adapter
#[cfg(target_os = "windows")]
pub struct WindowsSsdpAdapter;

#[cfg(target_os = "windows")]
impl WindowsSsdpAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "windows")]
#[async_trait]
impl SsdpPlatformAdapter for WindowsSsdpAdapter {
    async fn configure_socket(&self, socket: &mut SsdpSocket) -> Result<()> {
        // Windows-specific socket configuration
        debug!("Applying Windows-specific socket configuration");
        // The socket is already configured with Windows-specific options in SsdpSocket::new
        Ok(())
    }
    
    async fn get_suitable_interfaces(&self, network_manager: &dyn NetworkManager) -> Result<Vec<NetworkInterface>> {
        let interfaces = network_manager.get_local_interfaces().await
            .map_err(|e| anyhow::anyhow!("Failed to get interfaces: {}", e))?;
        
        // Filter for Windows - prefer Ethernet and WiFi, avoid VPN interfaces
        let suitable: Vec<_> = interfaces.into_iter()
            .filter(|iface| !iface.is_loopback && iface.is_up && iface.supports_multicast)
            .collect();
            
        Ok(suitable)
    }
    
    fn should_bind_to_specific_interface(&self) -> bool {
        false // Windows works better with INADDR_ANY
    }
    
    fn get_server_ip(&self, state: &AppState) -> String {
        state.get_server_ip()
    }
    
    fn get_ssdp_config(&self, state: &AppState) -> SsdpConfig {
        SsdpConfig {
            primary_port: SSDP_PORT,
            fallback_ports: vec![], // Don't use fallback ports on Windows
            multicast_address: SSDP_MULTICAST_ADDR.parse().unwrap(),
            announce_interval: Duration::from_secs(state.config.network.announce_interval_seconds),
            max_retries: 3,
            interfaces: Vec::new(),
        }
    }
}

/// Docker-specific SSDP platform adapter
pub struct DockerSsdpAdapter;

impl DockerSsdpAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SsdpPlatformAdapter for DockerSsdpAdapter {
    async fn configure_socket(&self, _socket: &mut SsdpSocket) -> Result<()> {
        // Docker-specific socket configuration
        debug!("Applying Docker-specific socket configuration");
        // Additional Docker-specific options could be set here
        Ok(())
    }
    
    async fn get_suitable_interfaces(&self, network_manager: &dyn NetworkManager) -> Result<Vec<NetworkInterface>> {
        let interfaces = network_manager.get_local_interfaces().await
            .map_err(|e| anyhow::anyhow!("Failed to get interfaces: {}", e))?;
        
        // In Docker, we typically want all non-loopback interfaces
        let suitable: Vec<_> = interfaces.into_iter()
            .filter(|iface| !iface.is_loopback && iface.is_up)
            .collect();
            
        Ok(suitable)
    }
    
    fn should_bind_to_specific_interface(&self) -> bool {
        false // Docker needs to bind to 0.0.0.0 for multicast
    }
    
    fn get_server_ip(&self, state: &AppState) -> String {
        state.get_server_ip()
    }
    
    fn get_ssdp_config(&self, state: &AppState) -> SsdpConfig {
        SsdpConfig {
            primary_port: SSDP_PORT,
            fallback_ports: vec![], // Don't use fallback ports in Docker
            multicast_address: SSDP_MULTICAST_ADDR.parse().unwrap(),
            announce_interval: Duration::from_secs(state.config.network.announce_interval_seconds),
            max_retries: 3,
            interfaces: Vec::new(),
        }
    }
}

/// Unix/Linux-specific SSDP platform adapter
pub struct UnixSsdpAdapter;

impl UnixSsdpAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SsdpPlatformAdapter for UnixSsdpAdapter {
    async fn configure_socket(&self, _socket: &mut SsdpSocket) -> Result<()> {
        // Unix-specific socket configuration
        debug!("Applying Unix-specific socket configuration");
        // The socket is already configured with Unix-specific options in SsdpSocket::new
        Ok(())
    }
    
    async fn get_suitable_interfaces(&self, network_manager: &dyn NetworkManager) -> Result<Vec<NetworkInterface>> {
        let interfaces = network_manager.get_local_interfaces().await
            .map_err(|e| anyhow::anyhow!("Failed to get interfaces: {}", e))?;
        
        // Filter for Unix - prefer Ethernet and WiFi
        let suitable: Vec<_> = interfaces.into_iter()
            .filter(|iface| !iface.is_loopback && iface.is_up && iface.supports_multicast)
            .collect();
            
        Ok(suitable)
    }
    
    fn should_bind_to_specific_interface(&self) -> bool {
        false // Unix works well with INADDR_ANY
    }
    
    fn get_server_ip(&self, state: &AppState) -> String {
        state.get_server_ip()
    }
    
    fn get_ssdp_config(&self, state: &AppState) -> SsdpConfig {
        SsdpConfig {
            primary_port: SSDP_PORT,
            fallback_ports: vec![8080, 8081, 8082, 9090], // Use fallback ports on Unix
            multicast_address: SSDP_MULTICAST_ADDR.parse().unwrap(),
            announce_interval: Duration::from_secs(state.config.network.announce_interval_seconds),
            max_retries: 3,
            interfaces: Vec::new(),
        }
    }
}

// ============================================================================
// Legacy implementations removed - now using UnifiedSsdpService with platform adapters
// ============================================================================


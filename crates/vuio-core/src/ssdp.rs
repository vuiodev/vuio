use crate::config::AppConfig;
use crate::database::DatabaseManager;
use crate::platform::network::{
    NetworkManager, PlatformNetworkManager, SsdpConfig, SsdpSocket, SSDP_MULTICAST_IP,
};
use crate::platform::NetworkInterface;
use crate::state::AppState;
use anyhow::Result;
use async_trait::async_trait;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

const SSDP_PORT: u16 = 1900;
type SharedSsdpSocket = Arc<std::sync::RwLock<Arc<SsdpSocket>>>;

/// Which interfaces this server announces on, and what address it names in each
/// announcement.
///
/// `network.interface_selection` used to reach none of this. The multicast group was
/// joined on the primary interface and nowhere else, so an M-SEARCH arriving on any
/// other interface was never received; "All" behaved exactly like "Auto"; and naming an
/// interface changed only the address written into LOCATION, which left a server
/// advertising one interface's address while listening on another's. On a host with a
/// second NIC, or a bridge beside the LAN, half the televisions in the house could not
/// find the server at all.
#[derive(Clone, Debug)]
struct Advertised {
    /// The interfaces the group is joined on and announcements go out of. Empty means
    /// "whatever the host routes multicast by", which is the fallback for a machine
    /// with nothing that qualifies.
    interfaces: Vec<NetworkInterface>,
    /// The address every LOCATION names, when the operator pinned one. A
    /// host-networked container reports an address the outside cannot reach, which is
    /// why an explicit setting has to win over anything worked out from an interface.
    fixed: Option<String>,
    /// What to name when nothing better can be worked out.
    fallback: String,
}

impl Advertised {
    /// The address to advertise when announcing out of `interface`.
    fn location_for_interface(&self, interface: &NetworkInterface) -> String {
        match &self.fixed {
            Some(address) => address.clone(),
            None => interface.ip_address.to_string(),
        }
    }

    /// The address to advertise to `peer`, which is the address of whichever
    /// interface the host would answer it on.
    ///
    /// Asked of the routing table rather than matched against the interface list: a
    /// `NetworkInterface` here carries no netmask, and the kernel already knows the
    /// answer. Connecting a UDP socket sends nothing; it only fixes a local address.
    async fn location_for_peer(&self, peer: IpAddr) -> String {
        if let Some(address) = &self.fixed {
            return address.clone();
        }
        if self.interfaces.len() == 1 {
            return self.interfaces[0].ip_address.to_string();
        }
        let bind = match peer {
            IpAddr::V4(_) => "0.0.0.0:0",
            IpAddr::V6(_) => "[::]:0",
        };
        if let Ok(socket) = tokio::net::UdpSocket::bind(bind).await {
            // Port 9 is discard; nothing is sent either way.
            if socket.connect(SocketAddr::new(peer, 9)).await.is_ok() {
                if let Ok(local) = socket.local_addr() {
                    if !local.ip().is_unspecified() {
                        return local.ip().to_string();
                    }
                }
            }
        }
        self.fallback.clone()
    }
}

/// One announcement pass: which interface it leaves by, and the address it names.
struct AnnouncementPass {
    interface: Option<Ipv4Addr>,
    location_ip: String,
}

impl AnnouncementPass {
    /// Point the socket's multicast sends at this pass's interface.
    ///
    /// Best effort: a host that refuses the option still announces, just by whatever
    /// route it would have chosen anyway, which is what it did before any of this.
    fn select_on(&self, socket: &SsdpSocket) {
        let Some(address) = self.interface else {
            return;
        };
        if let Err(error) = socket.set_multicast_interface(address) {
            warn!("Could not announce from {address}: {error}");
        }
    }
}

impl Advertised {
    /// The announcement passes to make: one per interface, or a single default pass
    /// for a host with no interface of its own to choose between.
    fn passes(&self) -> Vec<AnnouncementPass> {
        if self.interfaces.is_empty() {
            return vec![AnnouncementPass {
                interface: None,
                location_ip: self.fixed.clone().unwrap_or_else(|| self.fallback.clone()),
            }];
        }
        self.interfaces
            .iter()
            .map(|interface| AnnouncementPass {
                interface: match interface.ip_address {
                    IpAddr::V4(address) => Some(address),
                    // The group joined here is IPv4; an IPv6 interface has nothing to
                    // select and announces by the default route.
                    IpAddr::V6(_) => None,
                },
                location_ip: self.location_for_interface(interface),
            })
            .collect()
    }
}

/// Join `group` on every interface announced on, keeping going past one that refuses.
///
/// A host where none of them works still gets the single default join it always had,
/// which is better than a service that cannot answer at all.
async fn join_group_on_each(
    network_manager: &dyn NetworkManager,
    socket: &mut SsdpSocket,
    group: IpAddr,
    interfaces: &[NetworkInterface],
    primary: Option<&NetworkInterface>,
) {
    if interfaces.is_empty() {
        if let Err(error) = network_manager
            .join_multicast_group(socket, group, primary)
            .await
        {
            warn!("Failed to join multicast group: {error}");
        }
        return;
    }

    let mut joined = 0usize;
    for interface in interfaces {
        match network_manager
            .join_multicast_group(socket, group, Some(interface))
            .await
        {
            Ok(()) => joined += 1,
            Err(error) => warn!(
                "Failed to join multicast group on {} ({}): {error}",
                interface.name, interface.ip_address
            ),
        }
    }
    if joined == 0 {
        warn!("No interface accepted the multicast join; discovery may not work");
    }
}

/// Whether the operator has pinned the address to advertise.
///
/// The same question [`crate::state::AppState::advertised_http_origin_for_peer`] asks,
/// and for the same reason: under Docker with host networking the kernel reports an
/// address the outside cannot reach, so a configured one has to win over anything
/// derived from an interface.
fn address_is_pinned(config: &AppConfig) -> bool {
    std::env::var("VUIO_IP").is_ok_and(|value| !value.trim().is_empty())
        || config
            .server
            .ip
            .as_deref()
            .is_some_and(|value| !value.is_empty() && value != "0.0.0.0")
        || matches!(
            &config.network.interface_selection,
            crate::config::NetworkInterfaceConfig::Specific(value)
                if value.parse::<IpAddr>().is_ok()
        )
        // A server bound to one address can only be reached at it.
        || (!config.server.interface.is_empty() && config.server.interface != "0.0.0.0")
}

/// Resolve `network.interface_selection` against the interfaces this host has.
///
/// Returns the interfaces to join the multicast group on and announce from, which was
/// the whole of what this setting never did.
fn advertise_interfaces(
    selection: &crate::config::NetworkInterfaceConfig,
    interfaces: &[NetworkInterface],
    primary: Option<&NetworkInterface>,
) -> Vec<NetworkInterface> {
    use crate::config::NetworkInterfaceConfig;

    let usable = || {
        interfaces
            .iter()
            .filter(|interface| {
                interface.is_up
                    && !interface.is_loopback
                    && interface.supports_multicast
                    && interface.ip_address.is_ipv4()
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    let primary_only = || {
        primary
            .into_iter()
            .filter(|interface| interface.ip_address.is_ipv4())
            .cloned()
            .collect::<Vec<_>>()
    };

    match selection {
        NetworkInterfaceConfig::All => {
            let all = usable();
            if all.is_empty() {
                primary_only()
            } else {
                all
            }
        }
        NetworkInterfaceConfig::Specific(value) => {
            let value = value.trim();
            let named = usable()
                .into_iter()
                .filter(|interface| {
                    interface.name == value || interface.ip_address.to_string() == value
                })
                .collect::<Vec<_>>();
            if named.is_empty() {
                // Naming an interface the host does not have is a mistake worth saying
                // out loud: the alternative is a server that silently announces itself
                // somewhere the operator did not ask for.
                warn!(
                    "No usable network interface matches '{value}'; \
                     announcing on the primary interface instead"
                );
                primary_only()
            } else {
                named
            }
        }
        // What this has always done, and what a host with one interface wants.
        NetworkInterfaceConfig::Auto => {
            let primary = primary_only();
            if primary.is_empty() {
                usable().into_iter().take(1).collect()
            } else {
                primary
            }
        }
    }
}

fn load_ssdp_socket(socket: &SharedSsdpSocket) -> Arc<SsdpSocket> {
    socket
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

/// Platform-specific adapter for SSDP service behavior
#[async_trait]
pub trait SsdpPlatformAdapter: Send + Sync {
    /// Configure socket with platform-specific options
    async fn configure_socket(&self, socket: &mut SsdpSocket) -> Result<()>;

    /// Get network interfaces suitable for this platform
    async fn get_suitable_interfaces(
        &self,
        network_manager: &dyn NetworkManager,
    ) -> Result<Vec<NetworkInterface>>;

    /// Determine if this platform should bind to a specific interface
    fn should_bind_to_specific_interface(&self) -> bool;

    /// Get server IP address using platform-specific logic
    /// Get platform-specific SSDP configuration
    fn get_ssdp_config(&self, config: &AppConfig) -> SsdpConfig;
}

/// Unified SSDP service that works across all platforms
pub struct UnifiedSsdpService {
    network_manager: Arc<dyn NetworkManager>,
    platform_adapter: Box<dyn SsdpPlatformAdapter>,
    config: Arc<AppConfig>,
    /// Read live rather than snapshotted: LOCATION must name the port the HTTP server
    /// is actually accepting on, which a rebind can move under a running SSDP service.
    http_binding: Arc<crate::state::HttpBinding>,
    server_ip: String,
    primary_interface: Option<NetworkInterface>,
    /// Where announcements go out of, and what they say. See [`Advertised`].
    advertised: Advertised,
}

impl UnifiedSsdpService {
    /// Create a new unified SSDP service with the appropriate platform adapter
    pub fn new<D: DatabaseManager>(state: AppState<D>) -> Self {
        let network_manager = Arc::new(PlatformNetworkManager::new());
        let platform_adapter: Box<dyn SsdpPlatformAdapter> = Box::new(DefaultSsdpAdapter::new(
            !AppConfig::is_running_in_docker(),
        ));

        let config = state.current_config();
        let server_ip = state.get_server_ip();
        let primary_interface = state.platform_info.get_primary_interface().cloned();
        let interfaces = advertise_interfaces(
            &config.network.interface_selection,
            &state.platform_info.network_interfaces,
            primary_interface.as_ref(),
        );
        info!(
            "SSDP will announce on {}",
            if interfaces.is_empty() {
                "the host's default multicast route".to_owned()
            } else {
                interfaces
                    .iter()
                    .map(|interface| format!("{} ({})", interface.name, interface.ip_address))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        );
        let advertised = Advertised {
            interfaces,
            fixed: address_is_pinned(&config).then(|| server_ip.clone()),
            fallback: server_ip.clone(),
        };

        Self {
            network_manager,
            platform_adapter,
            config,
            http_binding: state.http_binding.clone(),
            server_ip,
            primary_interface,
            advertised,
        }
    }

    async fn spawn_tasks(
        &self,
        cancellation: CancellationToken,
    ) -> Result<(
        tokio::task::JoinHandle<Result<()>>,
        tokio::task::JoinHandle<Result<()>>,
    )> {
        info!("Starting unified SSDP service");

        let server_ip = self.server_ip.clone();
        info!("SSDP service using server IP: {}", server_ip);

        // Create SSDP socket with platform-specific configuration
        let mut ssdp_config = self.platform_adapter.get_ssdp_config(&self.config);
        // Linux establishes membership while creating the socket. Passing the
        // resolved selection here is therefore essential: trying to narrow it
        // afterwards is too late, and Linux's join method quite reasonably
        // returns once the socket says multicast is already enabled.
        ssdp_config.interfaces = self.advertised.interfaces.clone();
        let mut socket = self
            .network_manager
            .create_ssdp_socket_with_config(&ssdp_config)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create SSDP socket: {}", e))?;

        // Apply platform-specific socket configuration
        self.platform_adapter.configure_socket(&mut socket).await?;

        // Join the group on every interface that is announced on, not only the
        // primary one: a group joined on one interface receives nothing that arrives on
        // another, which is why an M-SEARCH from the second subnet of a multi-homed
        // host was never even seen.
        let multicast_addr = SSDP_MULTICAST_IP;
        join_group_on_each(
            self.network_manager.as_ref(),
            &mut socket,
            multicast_addr,
            &self.advertised.interfaces,
            self.primary_interface.as_ref(),
        )
        .await;

        // Applied after the socket exists so it lands on every platform, including the
        // Linux path that never runs the shared multicast configuration.
        if let Err(error) = socket.set_multicast_ttl(ssdp_config.multicast_ttl) {
            warn!(
                "Failed to set multicast TTL to {}: {}",
                ssdp_config.multicast_ttl, error
            );
        } else {
            // Logged at info deliberately: this setting spent its whole life being
            // validated and never applied, so the log should say what actually took.
            info!("SSDP multicast TTL set to {}", ssdp_config.multicast_ttl);
        }

        // Tokio's UDP socket supports concurrent send and receive through
        // shared references. Keeping the configured wrapper in an Arc avoids
        // holding an async mutex across the indefinitely pending receive.
        let socket = Arc::new(std::sync::RwLock::new(Arc::new(socket)));

        // Start M-SEARCH responder task
        let responder_config = self.config.clone();
        let responder_binding = self.http_binding.clone();
        let responder_advertised = self.advertised.clone();
        let responder_manager = self.network_manager.clone();
        let responder_ssdp_config = ssdp_config.clone();
        let responder_primary = self.primary_interface.clone();
        let responder_socket = socket.clone();
        let responder = tokio::spawn(async move {
            Self::search_responder_task(
                responder_config,
                responder_binding,
                responder_advertised,
                responder_manager,
                responder_ssdp_config,
                responder_primary,
                responder_socket,
            )
            .await
        });

        // Start announcement task
        let announcer_config = self.config.clone();
        let announcer_binding = self.http_binding.clone();
        let announcer_advertised = self.advertised.clone();
        let announcer_manager = self.network_manager.clone();
        let announcer_socket = socket.clone();
        let announcer = tokio::spawn(async move {
            Self::announcer_task(
                announcer_config,
                announcer_binding,
                announcer_advertised,
                announcer_manager,
                announcer_socket,
                cancellation,
            )
            .await
        });

        info!("Unified SSDP service started successfully");
        Ok((responder, announcer))
    }

    /// Run SSDP until cancellation while retaining ownership of both worker tasks.
    pub async fn run_until_cancelled(self, cancellation: CancellationToken) -> Result<()> {
        let (mut responder, mut announcer) = self.spawn_tasks(cancellation.clone()).await?;
        tokio::select! {
            _ = cancellation.cancelled() => {
                responder.abort();
                let _ = tokio::time::timeout(Duration::from_secs(2), &mut announcer).await;
                Ok(())
            }
            result = &mut responder => {
                announcer.abort();
                result.map_err(|error| anyhow::anyhow!("SSDP responder task failed: {error}"))?
            }
            result = &mut announcer => {
                responder.abort();
                result.map_err(|error| anyhow::anyhow!("SSDP announcer task failed: {error}"))?
            }
        }
    }

    /// Task for handling M-SEARCH requests
    async fn search_responder_task(
        config: Arc<AppConfig>,
        http_binding: Arc<crate::state::HttpBinding>,
        advertised: Advertised,
        network_manager: Arc<dyn NetworkManager>,
        ssdp_config: SsdpConfig,
        primary_interface: Option<NetworkInterface>,
        socket: SharedSsdpSocket,
    ) -> Result<()> {
        let mut buf = vec![0u8; 2048];
        let mut consecutive_errors = 0;
        const MAX_CONSECUTIVE_ERRORS: u32 = 10;

        loop {
            let (len, addr) = {
                let active_socket = load_ssdp_socket(&socket);
                match active_socket.recv_from(&mut buf).await {
                    Ok(result) => result,
                    Err(e) => {
                        consecutive_errors += 1;
                        error!(
                            "Error receiving SSDP data (consecutive errors: {}): {}",
                            consecutive_errors, e
                        );

                        if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                            warn!("Too many consecutive SSDP receive errors; recreating socket");
                            let mut replacement = network_manager
                                .create_ssdp_socket_with_config(&ssdp_config)
                                .await
                                .map_err(|error| {
                                    anyhow::anyhow!("SSDP socket recreation failed: {error}")
                                })?;
                            join_group_on_each(
                                network_manager.as_ref(),
                                &mut replacement,
                                SSDP_MULTICAST_IP,
                                &advertised.interfaces,
                                primary_interface.as_ref(),
                            )
                            .await;
                            // A replacement socket starts from platform defaults, so the
                            // configured hop limit has to be re-applied or a recovered
                            // socket quietly announces with a shorter reach than before.
                            if let Err(error) =
                                replacement.set_multicast_ttl(ssdp_config.multicast_ttl)
                            {
                                warn!("Failed to set multicast TTL on the replacement socket: {error}");
                            }
                            *socket.write().unwrap_or_else(|error| error.into_inner()) =
                                Arc::new(replacement);
                            consecutive_errors = 0;
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
                Self::handle_msearch_request(
                    &config,
                    &http_binding,
                    &advertised,
                    &socket,
                    &request,
                    addr,
                )
                .await;
            }
        }
    }

    /// Handle M-SEARCH request and send appropriate responses
    async fn handle_msearch_request(
        config: &AppConfig,
        http_binding: &crate::state::HttpBinding,
        advertised: &Advertised,
        socket: &SharedSsdpSocket,
        request: &str,
        addr: SocketAddr,
    ) {
        let response_types = Self::msearch_response_types(request);
        if response_types.is_empty() {
            return;
        }
        // The address of whichever interface this search arrived on, so a renderer on
        // the second subnet of a multi-homed host is told where *it* can reach the
        // server rather than where the primary interface is.
        let server_ip = advertised.location_for_peer(addr.ip()).await;

        let response_count = response_types.len();
        for response_type in response_types {
            let response =
                Self::create_ssdp_response(config, http_binding, &server_ip, response_type);
            let active_socket = load_ssdp_socket(socket);

            for retry in 0..3 {
                match active_socket.send_to(response.as_bytes(), addr).await {
                    Ok(_) => {
                        debug!(
                            "Successfully sent M-SEARCH response to {} for {}",
                            addr, response_type
                        );
                        break;
                    }
                    Err(e) => {
                        warn!(
                            "Failed to send M-SEARCH response (attempt {}): {}",
                            retry + 1,
                            e
                        );
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

    fn msearch_response_types(request: &str) -> Vec<&'static str> {
        let request = request.to_ascii_lowercase();
        if request.contains("ssdp:all") {
            vec![
                "upnp:rootdevice",
                "urn:schemas-upnp-org:device:MediaServer:1",
                "urn:schemas-upnp-org:service:ContentDirectory:1",
            ]
        } else if request.contains("upnp:rootdevice") {
            vec!["upnp:rootdevice"]
        } else if request.contains("urn:schemas-upnp-org:device:mediaserver") {
            vec!["urn:schemas-upnp-org:device:MediaServer:1"]
        } else if request.contains("urn:schemas-upnp-org:service:contentdirectory") {
            vec!["urn:schemas-upnp-org:service:ContentDirectory:1"]
        } else {
            Vec::new()
        }
    }

    /// Task for periodic SSDP announcements
    async fn announcer_task(
        config: Arc<AppConfig>,
        http_binding: Arc<crate::state::HttpBinding>,
        advertised: Advertised,
        network_manager: Arc<dyn NetworkManager>,
        socket: SharedSsdpSocket,
        cancellation: CancellationToken,
    ) -> Result<()> {
        let mut interval = interval(Duration::from_secs(
            config.network.announce_interval_seconds.max(1),
        ));
        let mut consecutive_failures = 0;
        const MAX_CONSECUTIVE_FAILURES: u32 = 5;

        loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    Self::send_ssdp_byebye(
                        &config,
                        &http_binding,
                        &advertised,
                        &network_manager,
                        &socket,
                    )
                    .await?;
                    return Ok(());
                }
                _ = interval.tick() => {}
            }

            match Self::send_ssdp_announcements(
                &config,
                &http_binding,
                &advertised,
                &network_manager,
                &socket,
            )
                .await
            {
                Ok(()) => {
                    consecutive_failures = 0;
                }
                Err(e) => {
                    consecutive_failures += 1;
                    error!(
                        "Failed to send SSDP announcements (failure {}): {}",
                        consecutive_failures, e
                    );

                    if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        error!("Too many consecutive announcement failures, resetting counter");
                        consecutive_failures = 0;
                        tokio::time::sleep(Duration::from_secs(30)).await;
                    }
                }
            }
        }
    }

    /// Send SSDP NOTIFY announcements, once out of every interface announced on.
    ///
    /// One pass per interface, each naming that interface's own address: multicast
    /// leaves a host by one interface unless the socket is told which, so a single
    /// pass announced the server to one subnet and left it invisible on the rest.
    async fn send_ssdp_announcements(
        config: &AppConfig,
        http_binding: &crate::state::HttpBinding,
        advertised: &Advertised,
        network_manager: &Arc<dyn NetworkManager>,
        socket: &SharedSsdpSocket,
    ) -> Result<()> {
        info!("Sending SSDP NOTIFY announcements");
        const SERVICE_TYPES: [&str; 3] = [
            "upnp:rootdevice",
            "urn:schemas-upnp-org:device:MediaServer:1",
            "urn:schemas-upnp-org:service:ContentDirectory:1",
        ];
        let multicast_addr = SocketAddr::new(SSDP_MULTICAST_IP, SSDP_PORT);

        for outgoing in advertised.passes() {
            let active_socket = load_ssdp_socket(socket);
            outgoing.select_on(&active_socket);

            for service_type in &SERVICE_TYPES {
                let message = Self::create_notify_message(
                    config,
                    http_binding,
                    &outgoing.location_ip,
                    service_type,
                );

                match network_manager
                    .send_multicast(&active_socket, message.as_bytes(), multicast_addr)
                    .await
                {
                    Ok(()) => {
                        info!(
                            "Successfully sent SSDP NOTIFY for {} from {}",
                            service_type, outgoing.location_ip
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Multicast NOTIFY for {} failed: {}, trying unicast fallback",
                            service_type, e
                        );

                        if let Err(e) = network_manager
                            .send_unicast_fallback(
                                &active_socket,
                                message.as_bytes(),
                                &active_socket.interfaces,
                            )
                            .await
                        {
                            error!(
                                "Both multicast and unicast fallback failed for {}: {}",
                                service_type, e
                            );
                        }
                    }
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        Ok(())
    }

    async fn send_ssdp_byebye(
        config: &AppConfig,
        http_binding: &crate::state::HttpBinding,
        advertised: &Advertised,
        network_manager: &Arc<dyn NetworkManager>,
        socket: &SharedSsdpSocket,
    ) -> Result<()> {
        let target = SocketAddr::new(SSDP_MULTICAST_IP, SSDP_PORT);
        for outgoing in advertised.passes() {
            let active_socket = load_ssdp_socket(socket);
            outgoing.select_on(&active_socket);
            for service_type in [
                "upnp:rootdevice",
                "urn:schemas-upnp-org:device:MediaServer:1",
                "urn:schemas-upnp-org:service:ContentDirectory:1",
            ] {
                let message = Self::create_notify_message(
                    config,
                    http_binding,
                    &outgoing.location_ip,
                    service_type,
                )
                .replace("NTS: ssdp:alive", "NTS: ssdp:byebye");
                network_manager
                    .send_multicast(&active_socket, message.as_bytes(), target)
                    .await?;
            }
        }
        info!("Sent SSDP byebye announcements");
        Ok(())
    }

    /// Create SSDP response message
    fn create_ssdp_response(
        config: &AppConfig,
        http_binding: &crate::state::HttpBinding,
        server_ip: &str,
        service_type: &str,
    ) -> String {
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
            server_ip, http_binding.port(), st, usn
        )
    }

    /// Create SSDP NOTIFY message
    fn create_notify_message(
        config: &AppConfig,
        http_binding: &crate::state::HttpBinding,
        server_ip: &str,
        service_type: &str,
    ) -> String {
        let (nt, usn) = match service_type {
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
            SSDP_MULTICAST_IP, SSDP_PORT, server_ip, http_binding.port(), nt, usn
        )
    }
}

/// Run SSDP as an owned lifecycle service until cancellation.
pub async fn run_ssdp_service_until_cancelled<D: DatabaseManager>(
    state: AppState<D>,
    cancellation: CancellationToken,
) -> Result<()> {
    UnifiedSsdpService::new(state)
        .run_until_cancelled(cancellation)
        .await
}

/// Default, parameterized platform adapter for SSDP service behavior
pub struct DefaultSsdpAdapter {
    require_multicast_support: bool,
}

impl DefaultSsdpAdapter {
    pub fn new(require_multicast_support: bool) -> Self {
        Self {
            require_multicast_support,
        }
    }
}

#[async_trait]
impl SsdpPlatformAdapter for DefaultSsdpAdapter {
    async fn configure_socket(&self, _socket: &mut SsdpSocket) -> Result<()> {
        Ok(())
    }

    async fn get_suitable_interfaces(
        &self,
        network_manager: &dyn NetworkManager,
    ) -> Result<Vec<NetworkInterface>> {
        let interfaces = network_manager
            .get_local_interfaces()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get interfaces: {}", e))?;

        let suitable: Vec<_> = interfaces
            .into_iter()
            .filter(|iface| {
                let basic = !iface.is_loopback && iface.is_up;
                if self.require_multicast_support {
                    basic && iface.supports_multicast
                } else {
                    basic
                }
            })
            .collect();

        Ok(suitable)
    }

    fn should_bind_to_specific_interface(&self) -> bool {
        false
    }

    fn get_ssdp_config(&self, config: &AppConfig) -> SsdpConfig {
        SsdpConfig {
            primary_port: SSDP_PORT,
            multicast_address: SSDP_MULTICAST_IP,
            announce_interval: Duration::from_secs(config.network.announce_interval_seconds),
            max_retries: 3,
            multicast_ttl: config.network.multicast_ttl,
            interfaces: Vec::new(),
        }
    }
}

// ============================================================================
// Legacy implementations removed - now using UnifiedSsdpService with platform adapters
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NetworkInterfaceConfig;
    use crate::platform::InterfaceType;

    fn interface(name: &str, address: &str) -> NetworkInterface {
        NetworkInterface {
            name: name.to_owned(),
            ip_address: address.parse().expect("address"),
            is_loopback: false,
            is_up: true,
            supports_multicast: true,
            interface_type: InterfaceType::Ethernet,
        }
    }

    fn two_networks() -> Vec<NetworkInterface> {
        vec![
            interface("eth0", "192.168.1.10"),
            interface("eth1", "10.10.0.5"),
            // Interface discovery can report one record per address. SSDP uses
            // the IPv4 group, so the IPv6 record for the same NIC is not a
            // second announcement pass or membership attempt.
            interface("eth0", "fe80::1"),
            NetworkInterface {
                is_up: false,
                ..interface("eth2", "172.16.0.9")
            },
            NetworkInterface {
                is_loopback: true,
                ..interface("lo0", "127.0.0.1")
            },
            NetworkInterface {
                supports_multicast: false,
                ..interface("tun0", "10.8.0.2")
            },
        ]
    }

    fn names(interfaces: &[NetworkInterface]) -> Vec<&str> {
        interfaces
            .iter()
            .map(|interface| interface.name.as_str())
            .collect()
    }

    /// The setting reached nothing: the group was joined on the primary interface and
    /// nowhere else, so an M-SEARCH arriving on any other interface was never received,
    /// and "All" was indistinguishable from "Auto".
    #[test]
    fn every_interface_is_announced_on_when_all_is_asked_for() {
        let interfaces = two_networks();
        let primary = interfaces[0].clone();

        let all = advertise_interfaces(
            &NetworkInterfaceConfig::All,
            &interfaces,
            Some(&primary),
        );
        assert_eq!(
            names(&all),
            ["eth0", "eth1"],
            "every interface that is up, routable and multicast-capable"
        );

        let auto = advertise_interfaces(
            &NetworkInterfaceConfig::Auto,
            &interfaces,
            Some(&primary),
        );
        assert_eq!(names(&auto), ["eth0"], "Auto is still the primary alone");
    }

    /// Naming an interface has to move the join, not only the advertised address —
    /// otherwise the server announces one interface's address while listening on
    /// another's, which is worse than either on its own.
    #[test]
    fn a_named_interface_is_the_one_announced_on() {
        let interfaces = two_networks();
        let primary = interfaces[0].clone();

        for spelling in ["eth1", "10.10.0.5"] {
            let chosen = advertise_interfaces(
                &NetworkInterfaceConfig::Specific(spelling.to_owned()),
                &interfaces,
                Some(&primary),
            );
            assert_eq!(names(&chosen), ["eth1"], "by {spelling}");
        }

        // An interface the host does not have falls back to the primary rather than
        // announcing nowhere.
        let missing = advertise_interfaces(
            &NetworkInterfaceConfig::Specific("eth9".to_owned()),
            &interfaces,
            Some(&primary),
        );
        assert_eq!(names(&missing), ["eth0"]);
    }

    /// A machine with nothing usable still announces, by whatever route the host
    /// chooses — which is what it did before any interface was resolved at all.
    #[test]
    fn a_host_with_no_usable_interface_still_announces() {
        let advertised = Advertised {
            interfaces: Vec::new(),
            fixed: None,
            fallback: "192.168.1.10".to_owned(),
        };
        let passes = advertised.passes();
        assert_eq!(passes.len(), 1);
        assert!(passes[0].interface.is_none());
        assert_eq!(passes[0].location_ip, "192.168.1.10");
    }

    /// One pass per interface, each naming its own address — and a pinned address
    /// overriding both, because a host-networked container reports an address the
    /// outside cannot reach.
    #[test]
    fn each_pass_names_the_address_it_leaves_by() {
        let advertised = Advertised {
            interfaces: vec![interface("eth0", "192.168.1.10"), interface("eth1", "10.10.0.5")],
            fixed: None,
            fallback: "192.168.1.10".to_owned(),
        };
        let passes = advertised.passes();
        assert_eq!(passes.len(), 2);
        assert_eq!(passes[0].location_ip, "192.168.1.10");
        assert_eq!(passes[1].location_ip, "10.10.0.5");
        assert_eq!(passes[1].interface, Some("10.10.0.5".parse().unwrap()));

        let pinned = Advertised {
            fixed: Some("203.0.113.7".to_owned()),
            ..advertised
        };
        for pass in pinned.passes() {
            assert_eq!(pass.location_ip, "203.0.113.7");
        }
    }

    /// And the address a searcher is told about is the one it can reach, which for a
    /// server with a single interface is that interface without asking the kernel.
    #[tokio::test]
    async fn a_search_is_answered_with_a_reachable_address() {
        let advertised = Advertised {
            interfaces: vec![interface("eth1", "10.10.0.5")],
            fixed: None,
            fallback: "192.168.1.10".to_owned(),
        };
        assert_eq!(
            advertised.location_for_peer("10.10.0.99".parse().unwrap()).await,
            "10.10.0.5"
        );

        let pinned = Advertised {
            fixed: Some("203.0.113.7".to_owned()),
            ..advertised
        };
        assert_eq!(
            pinned.location_for_peer("10.10.0.99".parse().unwrap()).await,
            "203.0.113.7",
            "an explicitly configured address wins over any interface"
        );
    }

    #[test]
    fn media_renderer_search_does_not_receive_a_media_server_response() {
        let request = "M-SEARCH * HTTP/1.1\r\n\
            MAN: \"ssdp:discover\"\r\n\
            ST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n\r\n";
        assert!(UnifiedSsdpService::msearch_response_types(request).is_empty());
    }
}

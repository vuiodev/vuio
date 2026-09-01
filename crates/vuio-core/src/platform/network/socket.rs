use crate::platform::{NetworkInterface, PlatformError, PlatformResult};
use std::net::{IpAddr, SocketAddr};
use tokio::net::UdpSocket;
use tracing::{debug, info, warn};

/// Hop limit used until the configured value is applied. Matches the shipped default.
pub const DEFAULT_MULTICAST_TTL: u8 = 4;

fn socket_error(action: &str, port: u16, error: std::io::Error) -> PlatformError {
    PlatformError::NetworkConfig(format!(
        "Failed to {action} reusable SSDP socket on 0.0.0.0:{port}: {error}"
    ))
}

/// Create the wildcard UDP socket used by SSDP.
///
/// Reuse options have to be present before `bind`. Setting them on a Tokio
/// socket after `UdpSocket::bind` makes startup order decide whether VuIO can
/// coexist with another SSDP listener on port 1900.
pub(crate) fn bind_ssdp_socket(port: u16) -> PlatformResult<UdpSocket> {
    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )
    .map_err(|error| socket_error("create", port, error))?;

    socket
        .set_reuse_address(true)
        .map_err(|error| socket_error("enable SO_REUSEADDR on", port, error))?;

    // BSD-derived kernels use SO_REUSEPORT for duplicate multicast bindings.
    // Linux supports it as well and still needs SO_REUSEADDR for compatibility
    // with SSDP implementations that only set address reuse.
    #[cfg(target_os = "linux")]
    if let Err(error) = socket.set_reuse_port(true) {
        warn!(
            "SO_REUSEPORT is unavailable for SSDP on Linux; continuing with SO_REUSEADDR: {error}"
        );
    }

    #[cfg(any(target_os = "macos", target_os = "freebsd"))]
    socket
        .set_reuse_port(true)
        .map_err(|error| socket_error("enable SO_REUSEPORT on", port, error))?;

    socket
        .set_broadcast(true)
        .map_err(|error| socket_error("enable broadcast on", port, error))?;
    socket
        .bind(&socket2::SockAddr::from(address))
        .map_err(|error| socket_error("bind", port, error))?;
    socket
        .set_nonblocking(true)
        .map_err(|error| socket_error("make nonblocking", port, error))?;

    let socket = std::net::UdpSocket::from(socket);
    UdpSocket::from_std(socket)
        .map_err(|error| socket_error("register with the async runtime", port, error))
}

/// SSDP socket construction and socket-option behavior, independent from
/// platform interface discovery.
#[derive(Debug)]
pub struct SsdpSocket {
    pub socket: UdpSocket,
    pub port: u16,
    pub interfaces: Vec<NetworkInterface>,
    pub multicast_enabled: bool,
}

impl SsdpSocket {
    pub async fn new(port: u16, interfaces: Vec<NetworkInterface>) -> PlatformResult<Self> {
        let socket = bind_ssdp_socket(port)?;
        debug!("Created SSDP socket bound to port {port}");
        Ok(Self {
            socket,
            port,
            interfaces,
            multicast_enabled: false,
        })
    }

    pub async fn enable_multicast(
        &mut self,
        multicast_addr: IpAddr,
        local_addr: IpAddr,
    ) -> PlatformResult<()> {
        match (multicast_addr, local_addr) {
            (IpAddr::V4(multicast), IpAddr::V4(local)) => {
                let bind_addr = if local.is_loopback() {
                    info!("Using INADDR_ANY instead of loopback {local} for multicast binding");
                    std::net::Ipv4Addr::UNSPECIFIED
                } else {
                    local
                };
                if let Err(error) = Self::configure_multicast(&self.socket, bind_addr) {
                    warn!("Failed to configure multicast socket options: {error}");
                }
                self.socket
                    .join_multicast_v4(multicast, bind_addr)
                    .map_err(|error| {
                        PlatformError::NetworkConfig(format!(
                            "Failed to join multicast group: {error}"
                        ))
                    })?;
                self.multicast_enabled = true;
                info!(
                    "Enabled multicast on {local}:{} for group {multicast} (bind addr: {bind_addr})",
                    self.port
                );
                Ok(())
            }
            (IpAddr::V6(multicast), _) => {
                self.socket
                    .join_multicast_v6(&multicast, 0)
                    .map_err(|error| {
                        PlatformError::NetworkConfig(format!(
                            "Failed to join IPv6 multicast group: {error}"
                        ))
                    })?;
                self.multicast_enabled = true;
                Ok(())
            }
            _ => Err(PlatformError::NetworkConfig(
                "IP version mismatch for multicast".to_string(),
            )),
        }
    }

    /// Apply the configured multicast hop limit to the live socket.
    ///
    /// This is deliberately a separate, explicit step rather than a constructor argument:
    /// the per-platform socket creation paths diverge, and the Linux one never reaches
    /// `configure_multicast` at all — its SSDP socket was left on the OS default of 1,
    /// so announcements could not cross a single router hop no matter what the
    /// configuration said. One call after the socket exists covers every platform.
    pub fn set_multicast_ttl(&self, ttl: u8) -> std::io::Result<()> {
        socket2::SockRef::from(&self.socket).set_multicast_ttl_v4(u32::from(ttl.max(1)))
    }

    fn configure_multicast(
        socket: &UdpSocket,
        bind_addr: std::net::Ipv4Addr,
    ) -> std::io::Result<()> {
        let socket = socket2::SockRef::from(socket);
        socket.set_multicast_ttl_v4(u32::from(DEFAULT_MULTICAST_TTL))?;
        socket.set_multicast_loop_v4(false)?;
        if !bind_addr.is_unspecified() {
            socket.set_multicast_if_v4(&bind_addr)?;
        }
        Ok(())
    }

    pub async fn send_to(&self, data: &[u8], addr: SocketAddr) -> PlatformResult<usize> {
        self.socket
            .send_to(data, addr)
            .await
            .map_err(|error| PlatformError::NetworkConfig(format!("Failed to send data: {error}")))
    }

    pub async fn recv_from(&self, buffer: &mut [u8]) -> PlatformResult<(usize, SocketAddr)> {
        self.socket.recv_from(buffer).await.map_err(|error| {
            PlatformError::NetworkConfig(format!("Failed to receive data: {error}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bind_compatible_peer(port: u16) -> UdpSocket {
        let socket = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::DGRAM,
            Some(socket2::Protocol::UDP),
        )
        .expect("peer socket");
        socket
            .set_reuse_address(true)
            .expect("peer SO_REUSEADDR");
        #[cfg(target_os = "linux")]
        let _ = socket.set_reuse_port(true);
        #[cfg(any(target_os = "macos", target_os = "freebsd"))]
        socket.set_reuse_port(true).expect("peer SO_REUSEPORT");
        socket
            .bind(&socket2::SockAddr::from(SocketAddr::from((
                [0, 0, 0, 0],
                port,
            ))))
            .expect("peer bind");
        socket.set_nonblocking(true).expect("peer nonblocking");
        UdpSocket::from_std(std::net::UdpSocket::from(socket)).expect("peer async socket")
    }

    /// `network.multicast_ttl` was validated, serialised, documented — and never applied.
    /// A test that only checks the config value would have passed for that entire history,
    /// so this reads the option back off the live socket.
    #[tokio::test]
    async fn the_configured_ttl_reaches_the_socket() {
        // Port 0: the OS picks a free one, so this never collides with a running server.
        let socket = SsdpSocket::new(0, Vec::new()).await.expect("socket");
        let read_back = || {
            socket2::SockRef::from(&socket.socket)
                .multicast_ttl_v4()
                .expect("read TTL")
        };

        socket.set_multicast_ttl(8).expect("set 8");
        assert_eq!(read_back(), 8);

        socket.set_multicast_ttl(1).expect("set 1");
        assert_eq!(read_back(), 1);

        // A zero hop limit would keep every packet on the host, which is never what an
        // operator means; validation rejects it, and this is the second line of defence.
        socket.set_multicast_ttl(0).expect("set 0");
        assert_eq!(read_back(), 1);
    }

    #[tokio::test]
    async fn reusable_ssdp_sockets_can_bind_in_either_startup_order() {
        let peer_first = bind_compatible_peer(0);
        let port = peer_first.local_addr().expect("peer address").port();
        let vuio_second = bind_ssdp_socket(port).expect("VuIO after peer");

        assert_eq!(vuio_second.local_addr().expect("VuIO address").port(), port);
        drop((peer_first, vuio_second));

        let vuio_first = bind_ssdp_socket(0).expect("VuIO before peer");
        let port = vuio_first.local_addr().expect("VuIO address").port();
        let peer_second = bind_compatible_peer(port);

        assert_eq!(peer_second.local_addr().expect("peer address").port(), port);
        assert!(
            socket2::SockRef::from(&vuio_first)
                .reuse_address()
                .expect("VuIO SO_REUSEADDR")
        );

        #[cfg(any(target_os = "macos", target_os = "freebsd"))]
        {
            assert!(
                socket2::SockRef::from(&vuio_first)
                    .reuse_port()
                    .expect("VuIO SO_REUSEPORT")
            );
        }
    }

    #[tokio::test]
    async fn shared_ssdp_sockets_both_receive_multicast() {
        let first = bind_ssdp_socket(0).expect("first reusable socket");
        let port = first.local_addr().expect("first local address").port();
        let second = bind_ssdp_socket(port).expect("second reusable socket");
        let group = std::net::Ipv4Addr::new(239, 255, 255, 250);

        first
            .join_multicast_v4(group, std::net::Ipv4Addr::UNSPECIFIED)
            .expect("first multicast membership");
        second
            .join_multicast_v4(group, std::net::Ipv4Addr::UNSPECIFIED)
            .expect("second multicast membership");

        let sender = UdpSocket::bind("0.0.0.0:0").await.expect("sender");
        sender
            .set_multicast_loop_v4(true)
            .expect("multicast loopback");
        sender
            .send_to(b"M-SEARCH test", (group, port))
            .await
            .expect("multicast send");

        let receive = async |socket: &UdpSocket| {
            let mut buffer = [0_u8; 64];
            let (length, _) = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                socket.recv_from(&mut buffer),
            )
            .await
            .expect("multicast receive timeout")
            .expect("multicast receive");
            assert_eq!(&buffer[..length], b"M-SEARCH test");
        };

        tokio::join!(receive(&first), receive(&second));
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "freebsd"))]
    #[tokio::test]
    async fn exclusive_port_owner_reports_a_bind_error() {
        let exclusive = std::net::UdpSocket::bind("0.0.0.0:0").expect("exclusive socket");
        let port = exclusive.local_addr().expect("exclusive address").port();

        let error = bind_ssdp_socket(port).expect_err("exclusive owner must reject sharing");
        assert!(error.to_string().contains("Failed to bind reusable SSDP socket"));
    }
}

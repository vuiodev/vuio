use crate::platform::{NetworkInterface, PlatformError, PlatformResult};
use std::net::{IpAddr, SocketAddr};
use tokio::net::UdpSocket;
use tracing::{debug, info, warn};

/// Hop limit used until the configured value is applied. Matches the shipped default.
pub const DEFAULT_MULTICAST_TTL: u8 = 4;

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
        let socket_addr = SocketAddr::from(([0, 0, 0, 0], port));
        let socket = UdpSocket::bind(socket_addr).await.map_err(|error| {
            PlatformError::NetworkConfig(format!("Failed to bind to port {port}: {error}"))
        })?;
        if let Err(error) = Self::configure_socket(&socket) {
            warn!("Failed to configure socket options: {error}");
        }
        debug!("Created SSDP socket bound to port {port}");
        Ok(Self {
            socket,
            port,
            interfaces,
            multicast_enabled: false,
        })
    }

    fn configure_socket(socket: &UdpSocket) -> std::io::Result<()> {
        let socket = socket2::SockRef::from(socket);
        socket.set_reuse_address(true)?;
        socket.set_broadcast(true)
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
}

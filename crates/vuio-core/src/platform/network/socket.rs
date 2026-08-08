use crate::platform::{NetworkInterface, PlatformError, PlatformResult};
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use tokio::net::UdpSocket;
use tracing::{debug, info, warn};

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

    fn configure_multicast(
        socket: &UdpSocket,
        bind_addr: std::net::Ipv4Addr,
    ) -> std::io::Result<()> {
        let socket = socket2::SockRef::from(socket);
        socket.set_multicast_ttl_v4(4)?;
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

    pub async fn set_read_timeout(&self, timeout: Option<Duration>) -> PlatformResult<()> {
        debug!("Read timeout set to {timeout:?} (implemented by async callers)");
        Ok(())
    }
}

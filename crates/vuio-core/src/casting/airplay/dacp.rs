//! The control endpoint a receiver calls back into.
//!
//! An AirPlay receiver does not invent transport controls: it offers them only
//! once the sender advertises somewhere to send them. That is DACP -- the
//! `DACP-ID` and `Active-Remote` headers on every RTSP request name a
//! `_dacp._tcp` service the receiver looks up as `iTunes_Ctrl_<DACP-ID>`, then
//! calls with `GET /ctrl-int/1/<command>`. Without the advertisement the
//! receiver has no callback address, so its remote shows no buttons at all.

use super::raop::Transport;
use anyhow::{Context, Result};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const SERVICE_TYPE: &str = "_dacp._tcp.local.";

/// A live DACP advertisement. Dropping it withdraws the service and stops the
/// listener, so a receiver stops offering controls for a finished session.
pub struct DacpServer {
    daemon: ServiceDaemon,
    fullname: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for DacpServer {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        self.task.abort();
    }
}

impl DacpServer {
    /// Publish a control endpoint for `dacp_id` and route commands to `transport`.
    pub async fn start(dacp_id: &str, local_ip: IpAddr, transport: Arc<Transport>) -> Result<Self> {
        let listener = tokio::net::TcpListener::bind(SocketAddr::new(local_ip, 0))
            .await
            .context("binding the AirPlay DACP control port")?;
        let port = listener.local_addr()?.port();

        let daemon = ServiceDaemon::new().context("starting mDNS for the DACP service")?;
        let instance = format!("iTunes_Ctrl_{dacp_id}");
        let host = format!("vuio-{dacp_id}.local.");
        let service = ServiceInfo::new(SERVICE_TYPE, &instance, &host, local_ip, port, None)
            .context("describing the DACP service")?;
        let fullname = service.get_fullname().to_string();
        daemon
            .register(service)
            .context("advertising the DACP service")?;
        tracing::info!(instance, port, "AirPlay DACP control endpoint advertised");

        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let transport = transport.clone();
                tokio::spawn(async move {
                    if let Err(error) = serve(stream, transport).await {
                        tracing::debug!(%error, "AirPlay DACP request failed");
                    }
                });
            }
        });

        Ok(Self {
            daemon,
            fullname,
            task,
        })
    }
}

/// Handle one control request. The receiver keeps the connection alive and
/// sends a request per button press.
async fn serve(mut stream: tokio::net::TcpStream, transport: Arc<Transport>) -> Result<()> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(&chunk[..read]);
        // Requests are header-only, so a blank line ends each one.
        while let Some(end) = buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|position| position + 4)
        {
            let request: Vec<u8> = buffer.drain(..end).collect();
            let head = String::from_utf8_lossy(&request);
            let path = head
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or_default();
            apply(path, &transport);
            stream
                .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
                .await?;
        }
        anyhow::ensure!(buffer.len() < 64 * 1024, "oversized DACP request");
    }
}

/// Map a DACP path onto a transport action.
fn apply(path: &str, transport: &Arc<Transport>) {
    let command = path.rsplit('/').next().unwrap_or_default();
    // Commands can carry query parameters, e.g. `setproperty?dmcp.volume=50`.
    let command = command.split('?').next().unwrap_or(command);
    match command {
        "nextitem" => {
            transport.skip_next.store(true, Ordering::Relaxed);
            tracing::info!("AirPlay DACP: next track");
        }
        "previtem" => {
            transport.restart.store(true, Ordering::Relaxed);
            tracing::info!("AirPlay DACP: previous track");
        }
        "play" | "pause" | "playpause" | "stop" => {
            transport.paused.fetch_xor(true, Ordering::Relaxed);
            tracing::info!(command, "AirPlay DACP: play/pause");
        }
        other => tracing::debug!(command = other, "AirPlay DACP command ignored"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Registering is not the same as being discoverable; browse for the
    /// service we just published and confirm it comes back.
    #[tokio::test]
    async fn advertised_service_is_discoverable() {
        let local_ip: IpAddr = "127.0.0.1".parse().unwrap();
        let transport = Arc::new(Transport::default());
        let Ok(server) = DacpServer::start("TESTDACPID01", local_ip, transport).await else {
            eprintln!("mDNS unavailable in this environment; skipping");
            return;
        };

        let daemon = ServiceDaemon::new().unwrap();
        let receiver = daemon.browse(SERVICE_TYPE).unwrap();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut found = false;
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, receiver.recv_async()).await {
            if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                if info.get_fullname().contains("iTunes_Ctrl_TESTDACPID01") {
                    found = true;
                    break;
                }
            }
        }
        drop(server);
        assert!(found, "the DACP service was registered but never resolved");
    }

    #[test]
    fn dacp_paths_map_to_transport_actions() {
        let transport = Arc::new(Transport::default());
        apply("/ctrl-int/1/nextitem", &transport);
        assert!(transport.skip_next.load(Ordering::Relaxed));

        apply("/ctrl-int/1/previtem", &transport);
        assert!(transport.restart.load(Ordering::Relaxed));

        assert!(!transport.paused.load(Ordering::Relaxed));
        apply("/ctrl-int/1/playpause", &transport);
        assert!(transport.paused.load(Ordering::Relaxed));
        apply("/ctrl-int/1/playpause", &transport);
        assert!(!transport.paused.load(Ordering::Relaxed));

        // Unknown commands, and query strings, must not panic or misfire.
        apply("/ctrl-int/1/setproperty?dmcp.volume=50", &transport);
        apply("/", &transport);
    }
}

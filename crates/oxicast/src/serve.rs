//! Built-in HTTP server for casting local files to Chromecast devices.
//!
//! Requires the `serve` feature:
//! ```toml
//! oxicast = { version = "0.1", features = ["serve"] }
//! ```
//!
//! # Example
//!
//! ```no_run
//! # async fn example(client: &oxicast::CastClient) -> oxicast::Result<()> {
//! use oxicast::serve::FileServer;
//!
//! let server = FileServer::start("0.0.0.0:0").await?;
//! let url = server.serve_file("/path/to/video.mp4", "video/mp4")?;
//! println!("Serving at: {url}");
//!
//! // Cast the URL to the Chromecast
//! client.load_media(
//!     &oxicast::MediaInfo::new(&url, "video/mp4"),
//!     true,
//!     0.0,
//!     None,
//! ).await?;
//!
//! // Server stays alive until dropped
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;

use crate::error::{Error, Result};

/// A lightweight HTTP file server for casting local files.
///
/// Serves files on a random available port. The Chromecast can access
/// them via the machine's LAN IP.
pub struct FileServer {
    addr: SocketAddr,
    lan_ip: String,
    files: Arc<RwLock<HashMap<String, FileEntry>>>,
    _shutdown: tokio::sync::oneshot::Sender<()>,
}

struct FileEntry {
    path: PathBuf,
    content_type: String,
}

impl FileServer {
    /// Start a file server on the given bind address.
    ///
    /// Use `"0.0.0.0:0"` to bind to all interfaces on a random port.
    pub async fn start(bind: &str) -> Result<Self> {
        let files: Arc<RwLock<HashMap<String, FileEntry>>> = Arc::new(RwLock::new(HashMap::new()));

        let app = Router::new().route("/file/{id}", get(serve_file)).with_state(files.clone());

        let listener = tokio::net::TcpListener::bind(bind)
            .await
            .map_err(|e| Error::Internal(format!("bind file server: {e}")))?;
        let addr =
            listener.local_addr().map_err(|e| Error::Internal(format!("local addr: {e}")))?;

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            let server = axum::serve(listener, app);
            tokio::select! {
                result = server => {
                    if let Err(e) = result {
                        tracing::error!("file server error: {e}");
                    }
                }
                _ = shutdown_rx => {
                    tracing::debug!("file server shutting down");
                }
            }
        });

        let lan_ip = detect_lan_ip()?;

        tracing::info!("file server started on http://{lan_ip}:{}", addr.port());

        Ok(Self { addr, lan_ip, files, _shutdown: shutdown_tx })
    }

    /// Register a local file for serving.
    ///
    /// Returns the full URL that the Chromecast can access.
    pub fn serve_file(&self, path: impl Into<PathBuf>, content_type: &str) -> Result<String> {
        let path = path.into();
        if !path.exists() {
            return Err(Error::FileNotFound(path.display().to_string()));
        }

        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = format!("{:x}", COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        let url = format!("http://{}:{}/file/{id}", self.lan_ip, self.addr.port());

        match self.files.write() {
            Ok(mut files) => {
                files.insert(id, FileEntry { path, content_type: content_type.to_string() });
                Ok(url)
            }
            Err(_) => Err(Error::Internal("file registry lock poisoned".into())),
        }
    }

    /// Get the server's address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Get the LAN IP the server is reachable on.
    pub fn lan_ip(&self) -> &str {
        &self.lan_ip
    }
}

async fn serve_file(
    Path(id): Path<String>,
    headers: HeaderMap,
    State(files): State<Arc<RwLock<HashMap<String, FileEntry>>>>,
) -> impl IntoResponse {
    // Clone path and content_type under a brief blocking lock, then drop immediately.
    let (path, content_type) = {
        let files = match files.read() {
            Ok(f) => f,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "lock poisoned").into_response(),
        };
        match files.get(&id) {
            Some(e) => (e.path.clone(), e.content_type.clone()),
            None => return (StatusCode::NOT_FOUND, "file not found").into_response(),
        }
    };

    let file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("open: {e}")).into_response(),
    };

    let metadata = match file.metadata().await {
        Ok(m) => m,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("metadata: {e}")).into_response();
        }
    };

    let file_size = metadata.len();

    // Parse Range header
    let has_range_header = headers.contains_key(header::RANGE);
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| parse_range(s, file_size));

    // Return 416 if Range header present but unsatisfiable
    if has_range_header && range.is_none() {
        let mut h = HeaderMap::new();
        if let Ok(v) = format!("bytes */{file_size}").parse() {
            h.insert(header::CONTENT_RANGE, v);
        }
        return (StatusCode::RANGE_NOT_SATISFIABLE, h, Body::empty()).into_response();
    }

    let ct: header::HeaderValue = content_type
        .parse()
        .unwrap_or_else(|_| header::HeaderValue::from_static("application/octet-stream"));

    match range {
        Some((start, end)) => {
            use tokio::io::{AsyncReadExt, AsyncSeekExt};
            let mut file = file;
            if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
                return (StatusCode::INTERNAL_SERVER_ERROR, "seek failed").into_response();
            }

            let len = end - start + 1;
            let limited = file.take(len);
            let stream = tokio_util::io::ReaderStream::new(limited);

            let mut h = HeaderMap::new();
            h.insert(header::CONTENT_TYPE, ct);
            if let Ok(v) = len.to_string().parse() {
                h.insert(header::CONTENT_LENGTH, v);
            }
            if let Ok(v) = format!("bytes {start}-{end}/{file_size}").parse() {
                h.insert(header::CONTENT_RANGE, v);
            }
            h.insert(header::ACCEPT_RANGES, header::HeaderValue::from_static("bytes"));
            h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, header::HeaderValue::from_static("*"));

            (StatusCode::PARTIAL_CONTENT, h, Body::from_stream(stream)).into_response()
        }
        None => {
            let stream = tokio_util::io::ReaderStream::new(file);

            let mut h = HeaderMap::new();
            h.insert(header::CONTENT_TYPE, ct);
            if let Ok(v) = file_size.to_string().parse() {
                h.insert(header::CONTENT_LENGTH, v);
            }
            h.insert(header::ACCEPT_RANGES, header::HeaderValue::from_static("bytes"));
            h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, header::HeaderValue::from_static("*"));

            (StatusCode::OK, h, Body::from_stream(stream)).into_response()
        }
    }
}

fn parse_range(header: &str, file_size: u64) -> Option<(u64, u64)> {
    if file_size == 0 {
        return None;
    }

    let s = header.strip_prefix("bytes=")?;
    let parts: Vec<&str> = s.splitn(2, '-').collect();
    if parts.len() != 2 {
        return None;
    }

    if parts[0].is_empty() {
        // bytes=-500 → last 500 bytes
        let suffix: u64 = parts[1].parse().ok()?;
        let start = file_size.saturating_sub(suffix);
        let end = file_size - 1;
        return if start <= end { Some((start, end)) } else { None };
    }

    let start: u64 = parts[0].parse().ok()?;

    let end: u64 = if parts[1].is_empty() { file_size - 1 } else { parts[1].parse().ok()? };

    if start <= end && end < file_size { Some((start, end)) } else { None }
}

fn detect_lan_ip() -> Result<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| Error::Internal(format!("UDP bind: {e}")))?;
    socket.connect("8.8.8.8:80").map_err(|e| Error::Internal(format!("UDP connect: {e}")))?;
    let addr = socket.local_addr().map_err(|e| Error::Internal(format!("local addr: {e}")))?;
    Ok(addr.ip().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range_basic() {
        assert_eq!(parse_range("bytes=0-10", 100), Some((0, 10)));
    }

    #[test]
    fn test_parse_range_open_end() {
        assert_eq!(parse_range("bytes=50-", 100), Some((50, 99)));
    }

    #[test]
    fn test_parse_range_suffix() {
        assert_eq!(parse_range("bytes=-20", 100), Some((80, 99)));
    }

    #[test]
    fn test_parse_range_full_file() {
        assert_eq!(parse_range("bytes=0-99", 100), Some((0, 99)));
    }

    #[test]
    fn test_parse_range_single_byte() {
        assert_eq!(parse_range("bytes=0-0", 100), Some((0, 0)));
    }

    #[test]
    fn test_parse_range_end_beyond_file() {
        assert_eq!(parse_range("bytes=0-200", 100), None);
    }

    #[test]
    fn test_parse_range_start_beyond_end() {
        assert_eq!(parse_range("bytes=50-10", 100), None);
    }

    #[test]
    fn test_parse_range_start_beyond_file() {
        assert_eq!(parse_range("bytes=100-", 100), None);
    }

    #[test]
    fn test_parse_range_non_numeric() {
        assert_eq!(parse_range("bytes=abc-def", 100), None);
    }

    #[test]
    fn test_parse_range_zero_length_file() {
        assert_eq!(parse_range("bytes=0-0", 0), None);
        assert_eq!(parse_range("bytes=-10", 0), None);
    }

    #[test]
    fn test_parse_range_no_bytes_prefix() {
        assert_eq!(parse_range("0-10", 100), None);
    }

    #[test]
    fn test_parse_range_suffix_larger_than_file() {
        assert_eq!(parse_range("bytes=-200", 100), Some((0, 99)));
    }
}

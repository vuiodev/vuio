//! Media, radio, subtitle, and cover-art streaming handlers.

use crate::{database::DatabaseManager, error::AppError, state::AppState};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use std::{path::PathBuf, sync::atomic::Ordering, time::Instant};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;
use tracing::{debug, error};

use super::diagnostics::WebHandlerMetrics;

struct MetricsTrackingReader<R> {
    inner: R,
    metrics: std::sync::Arc<WebHandlerMetrics>,
}

impl<R: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for MetricsTrackingReader<R> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        match std::pin::Pin::new(&mut self.inner).poll_read(cx, buf) {
            std::task::Poll::Ready(Ok(())) => {
                let after = buf.filled().len();
                let bytes_read = after - before;
                if bytes_read > 0 {
                    self.metrics
                        .bytes_transferred
                        .fetch_add(bytes_read as u64, Ordering::Relaxed);
                }
                std::task::Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

pub async fn serve_media<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    axum::extract::ConnectInfo(client_addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Path(id): Path<String>,
    method: Method,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let start_time = Instant::now();

    let file_id = id.parse::<i64>().map_err(|_| {
        state.web_metrics.record_error();
        AppError::NotFound
    })?;

    // Use ReDB database with atomic cache lookup
    let file_info = state
        .database
        .get_file_location_by_id(file_id)
        .await
        .map_err(|e| {
            error!("ReDB database error getting file by ID {}: {}", file_id, e);
            state.web_metrics.record_error();
            AppError::NotFound
        })?
        .ok_or_else(|| {
            debug!("ReDB database: file ID {} not found", file_id);
            state.web_metrics.record_error();
            AppError::NotFound
        })?;

    if file_info.mime_type == "audio/radio" {
        return Ok(
            axum::response::Redirect::temporary(file_info.path.to_string_lossy().as_ref())
                .into_response(),
        );
    }

    // Record dynamic client telemetry for GET requests (playing)
    if method == Method::GET {
        let client_ip = client_addr.ip().to_string();

        let device_name = {
            if let Some(name) = state.discovered_tvs.name_for_ip(&client_ip).await {
                name
            } else if let Some(ua) = headers
                .get(axum::http::header::USER_AGENT)
                .and_then(|h| h.to_str().ok())
            {
                let ua_lower = ua.to_lowercase();
                if ua_lower.contains("ipad") {
                    format!("iPad ({})", client_ip)
                } else if ua_lower.contains("iphone") {
                    format!("iPhone ({})", client_ip)
                } else if ua_lower.contains("android") {
                    format!("Android ({})", client_ip)
                } else if ua_lower.contains("macintosh") || ua_lower.contains("mac os x") {
                    format!("Mac ({})", client_ip)
                } else if ua_lower.contains("windows") {
                    format!("Windows PC ({})", client_ip)
                } else {
                    format!("Device ({})", client_ip)
                }
            } else {
                format!("Device ({})", client_ip)
            }
        };

        {
            let mut casts = state.active_casts.lock().await;
            casts.insert(device_name, file_info.filename.clone());
        }
    }

    // Enforce read-only access to media files
    let mut file = tokio::fs::OpenOptions::new()
        .read(true)
        .write(false)
        .open(&file_info.path)
        .await
        .map_err(AppError::Io)?;

    // Use actual file size from disk to avoid stale DB values causing range mismatches
    let metadata = file.metadata().await.map_err(AppError::Io)?;
    let file_size = metadata.len();

    let client = crate::web::client::detect_client(&headers);

    let mime_override = match client {
        crate::web::client::DlnaClientProfile::SamsungTv
        | crate::web::client::DlnaClientProfile::SamsungTvQ
            if file_info.mime_type == "video/x-matroska" =>
        {
            "video/x-mkv".to_string()
        }
        crate::web::client::DlnaClientProfile::SamsungTv
        | crate::web::client::DlnaClientProfile::SamsungTvQ
            if file_info.mime_type == "video/x-msvideo" =>
        {
            "video/mpeg".to_string()
        }
        crate::web::client::DlnaClientProfile::SonyBdp
            if file_info.mime_type == "video/x-matroska" || file_info.mime_type == "video/mpeg" =>
        {
            "video/divx".to_string()
        }
        crate::web::client::DlnaClientProfile::Xbox if file_info.mime_type == "video/x-msvideo" => {
            "video/avi".to_string()
        }
        _ => file_info.mime_type.clone(),
    };

    let encoded_filename = percent_encoding::utf8_percent_encode(
        &file_info.filename,
        percent_encoding::NON_ALPHANUMERIC,
    )
    .to_string();
    let content_disposition = format!(
        "inline; filename=\"{}\"; filename*=UTF-8''{}",
        file_info.filename.replace('"', "\\\""),
        encoded_filename
    );

    let mut response_builder = Response::builder()
        .header(header::CONTENT_TYPE, &mime_override)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_DISPOSITION, &content_disposition)
        .header("transferMode.dlna.org", "Streaming")
        .header(
            "contentFeatures.dlna.org",
            "DLNA.ORG_OP=11;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000",
        );

    // CaptionInfo.sec injection for Samsung TVs when subtitles exist
    if let Some(caption_req) = headers
        .get("getcaptioninfo.sec")
        .and_then(|h| h.to_str().ok())
    {
        if caption_req == "1" && file_info.subtitle_available {
            let server_ip = headers
                .get(header::HOST)
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.split(':').next())
                .unwrap_or("127.0.0.1");
            let srt_url = format!(
                "http://{}:{}/media/{}/subtitle",
                server_ip, state.config.server.port, file_id
            );
            debug!(
                "Injecting Samsung subtitle header CaptionInfo.sec: {}",
                srt_url
            );
            response_builder = response_builder.header("CaptionInfo.sec", srt_url);
        }
    }

    let (start, end, is_range_request) = if let Some(range_header) = headers.get(header::RANGE) {
        let range_str = range_header.to_str().map_err(|_| AppError::InvalidRange)?;
        debug!("Received range request: {}", range_str);

        if file_size == 0 {
            return Err(AppError::InvalidRange);
        }

        // Parse the range header manually to avoid enum variant issues
        let (start, end) = parse_range_header(range_str, file_size)?;
        (start, end, true)
    } else {
        // No range requested, serve the whole file
        (0, file_size.saturating_sub(1), false)
    };

    let len = if file_size == 0 { 0 } else { end - start + 1 };

    let response_status = if is_range_request {
        response_builder = response_builder.header(
            header::CONTENT_RANGE,
            format!("bytes {}-{}/{}", start, end, file_size),
        );
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };

    response_builder = response_builder.header(header::CONTENT_LENGTH, len);

    // For HEAD requests, return headers only without streaming the body
    if method == Method::HEAD {
        debug!(
            "HEAD request for media file ID {} (size: {})",
            file_id, file_size
        );
        let response_time = start_time.elapsed().as_micros() as u64;
        state.web_metrics.record_file_serve(response_time, false);
        return Ok(response_builder
            .status(response_status)
            .body(Body::empty())?);
    }

    file.seek(std::io::SeekFrom::Start(start)).await?;
    let tracking_reader = MetricsTrackingReader {
        inner: file.take(len),
        metrics: state.web_metrics.clone(),
    };
    let stream = ReaderStream::with_capacity(tracking_reader, 64 * 1024);
    let body = Body::from_stream(stream);

    // Record atomic performance metrics for file serving
    let response_time = start_time.elapsed().as_micros() as u64;
    let is_actual_serve = method == Method::GET && start == 0 && (len > 2 || len == file_size);
    state
        .web_metrics
        .record_file_serve(response_time, is_actual_serve);

    debug!(
        "Served media file ID {} ({} bytes from offset {}) in {}ms",
        file_id, len, start, response_time
    );

    Ok(response_builder.status(response_status).body(body)?)
}

// Helper function to parse range header manually
fn parse_range_header(range_str: &str, file_size: u64) -> Result<(u64, u64), AppError> {
    if file_size == 0 {
        return Err(AppError::InvalidRange);
    }

    let range_str = range_str.trim();
    // Remove "bytes=" prefix
    let range_part = range_str
        .strip_prefix("bytes=")
        .ok_or(AppError::InvalidRange)?
        .trim();

    // Split on comma to get individual ranges (we'll just handle the first one)
    let first_range = range_part
        .split(',')
        .next()
        .ok_or(AppError::InvalidRange)?
        .trim();

    // Parse the range
    if let Some((start_str, end_str)) = first_range.split_once('-') {
        let start_str = start_str.trim();
        let end_str = end_str.trim();

        if start_str.is_empty() {
            // Suffix range like "-500" (last 500 bytes).
            let suffix_len: u64 = end_str.parse().map_err(|_| AppError::InvalidRange)?;
            if suffix_len == 0 {
                return Err(AppError::InvalidRange);
            }
            return Ok((file_size.saturating_sub(suffix_len), file_size - 1));
        }

        let start = start_str.parse().map_err(|_| AppError::InvalidRange)?;

        let end = if end_str.is_empty() {
            // Range like "500-" (from 500 to end)
            file_size - 1
        } else {
            let parsed_end: u64 = end_str.parse().map_err(|_| AppError::InvalidRange)?;
            parsed_end.min(file_size - 1)
        };

        // Validate range
        if start > end || start >= file_size {
            return Err(AppError::InvalidRange);
        }

        Ok((start, end))
    } else {
        Err(AppError::InvalidRange)
    }
}

pub async fn serve_subtitle<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let file_id = id.parse::<i64>().map_err(|_| {
        state.web_metrics.record_error();
        AppError::NotFound
    })?;

    let file_info = state
        .database
        .get_file_location_by_id(file_id)
        .await
        .map_err(|e| {
            error!("Error getting file by ID for subtitle {}: {}", file_id, e);
            state.web_metrics.record_error();
            AppError::NotFound
        })?
        .ok_or_else(|| {
            state.web_metrics.record_error();
            AppError::NotFound
        })?;

    let srt_path = PathBuf::from(&file_info.path).with_extension("srt");
    if !srt_path.exists() {
        return Err(AppError::NotFound);
    }

    let file = tokio::fs::OpenOptions::new()
        .read(true)
        .write(false)
        .open(&srt_path)
        .await
        .map_err(AppError::Io)?;

    let stream = tokio_util::io::ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Response::builder()
        .header(header::CONTENT_TYPE, "text/srt")
        .body(body)
        .map_err(|_| AppError::NotFound)
}

pub async fn serve_cover<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let file_id = id.parse::<i64>().map_err(|_| {
        state.web_metrics.record_error();
        AppError::NotFound
    })?;

    let file_info = state
        .database
        .get_file_location_by_id(file_id)
        .await
        .map_err(|e| {
            error!("Error getting file by ID for cover {}: {}", file_id, e);
            state.web_metrics.record_error();
            AppError::NotFound
        })?
        .ok_or_else(|| {
            state.web_metrics.record_error();
            AppError::NotFound
        })?;

    if !file_info.mime_type.starts_with("audio/") {
        return Err(AppError::NotFound);
    }

    // 1. Primary: Search parent directory for cover images (fast)
    if let Some(parent) = file_info.path.parent() {
        let base_name = file_info
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        let cover_filenames = [
            "cover", "Cover", "COVER", "folder", "Folder", "FOLDER", "album", "Album", "ALBUM",
            "artwork", "Artwork", "ARTWORK", base_name,
        ];

        let extensions = ["jpg", "jpeg", "png", "webp", "heif", "heic", "avif"];

        for name in &cover_filenames {
            for ext in &extensions {
                let img_path = parent.join(format!("{}.{}", name, ext));
                if img_path.exists() && img_path.is_file() {
                    if let Ok(data) = tokio::fs::read(&img_path).await {
                        let content_type =
                            crate::platform::filesystem::get_mime_type_for_extension(ext);
                        return Response::builder()
                            .header(header::CONTENT_TYPE, content_type)
                            .body(Body::from(data))
                            .map_err(|_| AppError::NotFound);
                    }
                }
            }
        }
    }

    // 2. Secondary: Extract embedded artwork from audio tags using audiotags (blocking task)
    let path = file_info.path.clone();
    let tag_result =
        tokio::task::spawn_blocking(move || audiotags::Tag::new().read_from_path(&path)).await;

    if let Ok(Ok(tag)) = tag_result {
        if let Some(cover) = tag.album_cover() {
            let content_type = match cover.mime_type {
                audiotags::MimeType::Jpeg => "image/jpeg",
                audiotags::MimeType::Png => "image/png",
                _ => "image/jpeg",
            };
            return Response::builder()
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(cover.data.to_vec()))
                .map_err(|_| AppError::NotFound);
        }
    }

    Err(AppError::NotFound)
}

#[cfg(test)]
mod range_tests {
    use super::*;

    #[test]
    fn empty_files_reject_every_range_without_underflowing() {
        for range in ["bytes=0-", "bytes=-1", "bytes=0-0"] {
            assert!(matches!(
                parse_range_header(range, 0),
                Err(AppError::InvalidRange)
            ));
        }
    }

    #[test]
    fn parses_valid_bounded_open_and_suffix_ranges() {
        assert_eq!(parse_range_header("bytes=2-5", 10).unwrap(), (2, 5));
        assert_eq!(parse_range_header("bytes=7-", 10).unwrap(), (7, 9));
        assert_eq!(parse_range_header("bytes=-3", 10).unwrap(), (7, 9));
        assert_eq!(parse_range_header("bytes=7-99", 10).unwrap(), (7, 9));
    }

    #[test]
    fn rejects_ranges_outside_the_file() {
        assert!(matches!(
            parse_range_header("bytes=10-", 10),
            Err(AppError::InvalidRange)
        ));
        assert!(matches!(
            parse_range_header("bytes=8-3", 10),
            Err(AppError::InvalidRange)
        ));
    }
}

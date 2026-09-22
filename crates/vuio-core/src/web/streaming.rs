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

fn content_disposition(filename: &str) -> String {
    let mut fallback = String::with_capacity(filename.len().min(255));
    for character in filename.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, ' ' | '.' | '_' | '-') {
            fallback.push(character);
        } else if !matches!(character, '\r' | '\n') {
            fallback.push('_');
        }
    }
    let fallback = fallback.trim();
    let fallback = if fallback.is_empty() {
        "media"
    } else {
        fallback
    };
    let encoded =
        percent_encoding::utf8_percent_encode(filename, percent_encoding::NON_ALPHANUMERIC);
    format!("inline; filename=\"{fallback}\"; filename*=UTF-8''{encoded}")
}

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

    // Only the opening request of a playback, not every range request that
    // follows it — what this is for is seeing which resource a renderer chose
    // and how it seeks, and one line per scrub says that without one line per
    // buffer refill.
    if !headers.contains_key(header::RANGE) {
        crate::web::client::log_renderer_request(&format!("/media/{id}"), &method, &headers);
    }

    let file_id = media_id_from_path_segment(&id).ok_or_else(|| {
        state.web_metrics.record_error();
        AppError::NotFound
    })?;

    // Use the database with atomic cache lookup
    let file_info = state
        .database
        .get_file_location_by_id(file_id)
        .await
        .map_err(|e| {
            error!("Database error getting file by ID {}: {}", file_id, e);
            state.web_metrics.record_error();
            AppError::NotFound
        })?
        .ok_or_else(|| {
            debug!("Database: file ID {} not found", file_id);
            state.web_metrics.record_error();
            AppError::NotFound
        })?;

    if file_info.mime_type == "audio/radio" {
        let path_str = file_info.path.to_string_lossy();
        if path_str.starts_with("http://") || path_str.starts_with("https://") {
            return Ok(axum::response::Redirect::temporary(&path_str).into_response());
        }
        if let Ok(content) = std::fs::read_to_string(&file_info.path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                    return Ok(axum::response::Redirect::temporary(trimmed).into_response());
                }
            }
        }
        return Ok(
            axum::response::Redirect::temporary(&path_str).into_response(),
        );
    }

    // Record dynamic client telemetry for GET requests (playing)
    if method == Method::GET {
        let client_ip = client_addr.ip().to_string();

        // A discovered renderer gives the nicest name; without casting there
        // are no discovered renderers, so the User-Agent guess below is used.
        #[cfg(feature = "casting")]
        let discovered = state.discovered_tvs.name_for_ip(&client_ip).await;
        #[cfg(not(feature = "casting"))]
        let discovered: Option<String> = None;

        let device_name = {
            if let Some(name) = discovered {
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

    let content_disposition = content_disposition(&file_info.filename);

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
            let srt_url = format!(
                "{}/media/{}/subtitle",
                state.advertised_http_origin(),
                file_id,
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

    if method == Method::GET {
        tracing::info!(
            file_id,
            filename = %file_info.filename,
            client = %client_addr.ip(),
            start,
            len,
            "media GET"
        );
    } else {
        debug!(
            "Served media file ID {} ({} bytes from offset {}) in {}ms",
            file_id, len, start, response_time
        );
    }

    Ok(response_builder.status(response_status).body(body)?)
}

pub(crate) fn media_id_from_path_segment(segment: &str) -> Option<i64> {
    let id = match segment.split_once('.') {
        Some((id, extension)) => {
            if extension.is_empty()
                || extension.len() > 16
                || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
            {
                return None;
            }
            id
        }
        None => segment,
    };
    id.parse().ok()
}

// Helper function to parse range header manually
pub(crate) fn parse_range_header(range_str: &str, file_size: u64) -> Result<(u64, u64), AppError> {
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

/// Largest sidecar subtitle the WebVTT converter will pull into memory. Real subtitle
/// tracks run to a few hundred kilobytes; past this it is not a subtitle.
const MAX_SUBTITLE_BYTES: u64 = 8 * 1024 * 1024;

/// Largest sidecar image `/media/{id}/cover` will serve.
///
/// Cover art is a few hundred kilobytes and a generous one is a couple of megabytes.
/// The names searched for are ordinary ones — `cover`, `folder`, `album`, `artwork`,
/// the track's own stem — so whatever happens to sit under one of them in a music
/// folder was being read whole into memory, once per request, on an endpoint that
/// needs no credential. A file past this is not cover art; the search moves on to the
/// next candidate, and to the embedded and fetched artwork behind it.
const MAX_COVER_BYTES: u64 = 16 * 1024 * 1024;
#[cfg(feature = "metadata")]
static EMBEDDED_COVER_READS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

/// Resolve a media id to its sidecar `.srt`, shared by the raw and WebVTT handlers.
async fn resolve_srt_path<D: DatabaseManager>(
    state: &AppState<D>,
    id: &str,
) -> Result<PathBuf, AppError> {
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
    if !tokio::fs::try_exists(&srt_path).await.unwrap_or(false) {
        return Err(AppError::NotFound);
    }
    Ok(srt_path)
}

/// Raw SubRip, for the TVs that ask for it — Samsung reaches this through the
/// `CaptionInfo.sec` header, LG and Panasonic through `pv:subtitleFileUri`.
pub async fn serve_subtitle<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let srt_path = resolve_srt_path(&state, &id).await?;

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

/// The same sidecar converted to WebVTT, which is the only caption format the browser
/// `<track>` element accepts.
pub async fn serve_subtitle_vtt<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let srt_path = resolve_srt_path(&state, &id).await?;

    let metadata = tokio::fs::metadata(&srt_path).await.map_err(AppError::Io)?;
    if metadata.len() > MAX_SUBTITLE_BYTES {
        return Err(AppError::NotFound);
    }

    let raw = tokio::fs::read(&srt_path).await.map_err(AppError::Io)?;
    // Sidecar SRTs are routinely CP1251 or Windows-1252 rather than UTF-8. Lossy decoding
    // keeps every timing intact and mangles at worst a handful of accented characters,
    // which beats refusing to show subtitles at all.
    let vtt = crate::web::subtitles::srt_to_vtt(&String::from_utf8_lossy(&raw));

    Response::builder()
        .header(header::CONTENT_TYPE, "text/vtt; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(vtt))
        .map_err(|_| AppError::NotFound)
}

/// Stream one candidate sidecar image, or `None` if it is not one to serve.
///
/// Streamed rather than buffered, and refused past [`MAX_COVER_BYTES`]: this used to
/// read the file whole into memory before answering, so N concurrent requests for a
/// folder holding a large image under one of the searched names cost N copies of it.
async fn serve_cover_file(path: &std::path::Path, extension: &str) -> Option<Response> {
    let file = tokio::fs::OpenOptions::new()
        .read(true)
        .write(false)
        .open(path)
        .await
        .ok()?;
    let metadata = file.metadata().await.ok()?;
    if !metadata.is_file() {
        return None;
    }
    let length = metadata.len();
    if length > MAX_COVER_BYTES {
        debug!(
            "Ignoring {} as cover art: {} bytes is past the limit",
            path.display(),
            length
        );
        return None;
    }

    let content_type = crate::platform::filesystem::get_mime_type_for_extension(extension);
    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, length)
        .body(Body::from_stream(ReaderStream::with_capacity(
            file,
            64 * 1024,
        )))
        .ok()
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

    // Video used to be rejected outright, because the only artwork VuIO could find
    // was a sidecar file or an embedded audio tag and a video file has neither. A
    // fetched poster is artwork it does have, so video falls through to the cache
    // below instead of 404ing here.
    let local_sources_apply = file_info.mime_type.starts_with("audio/");

    // 1. Primary: Search parent directory for cover images (fast)
    if let Some(parent) = file_info.path.parent().filter(|_| local_sources_apply) {
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
                if let Some(response) = serve_cover_file(&img_path, ext).await {
                    return Ok(response);
                }
            }
        }
    }

    // 2. Secondary: Extract embedded artwork from the file's own metadata
    // (blocking task). Only the embedded path needs a tag reader — the
    // directory search above still serves cover art without the feature.
    #[cfg(feature = "metadata")]
    if local_sources_apply {
        // The tag reader materialises an embedded image whole while parsing it,
        // whatever size it is — symphonia does not enforce the visual limit it
        // is handed (see `extract_embedded_cover`) — so the bound on that memory
        // is how many parses run at once.
        //
        // The permit travels into the blocking task. Held here instead, it was
        // released when this request was dropped — a client that disconnects
        // mid-parse — while the parse carried on without it, so a stream of
        // requests abandoned as soon as they were sent ran any number at once.
        let permit = EMBEDDED_COVER_READS
            .acquire()
            .await
            .map_err(|_| AppError::NotFound)?;
        let path = file_info.path.clone();
        let cover = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            crate::platform::filesystem::extract_embedded_cover(&path, MAX_COVER_BYTES as usize)
        })
        .await;

        if let Ok(Some((content_type, data))) = cover {
            return Response::builder()
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(data))
                .map_err(|_| AppError::NotFound);
        }
    }

    // 3. Last: a poster downloaded by the media info fetch. Comes last so anything
    // shipped alongside the file still wins — the operator's own artwork is a
    // deliberate choice, and a provider's guess is not.
    #[cfg(feature = "mediainfo")]
    {
        if let Some(response) = serve_cached_artwork(&state, file_id).await {
            return Ok(response);
        }
    }

    Err(AppError::NotFound)
}

/// Serve the artwork the media info fetch cached for this file, if any.
#[cfg(feature = "mediainfo")]
async fn serve_cached_artwork<D: DatabaseManager>(
    state: &AppState<D>,
    file_id: i64,
) -> Option<Response> {
    let config = state.current_config();
    if !config.mediainfo.artwork_enabled {
        return None;
    }
    let root = config.mediainfo.artwork_path.as_ref()?;
    let key = state
        .database
        .get_mediainfo(file_id)
        .await
        .ok()
        .flatten()?
        .artwork_key?;

    let cache = crate::mediainfo::ArtworkCache::new(root);
    let path = cache.lookup(&key)?;
    let extension = path.extension()?.to_str()?;
    serve_cover_file(&path, extension).await
}

#[cfg(test)]
mod range_tests {
    use super::*;

    #[test]
    fn content_disposition_has_safe_ascii_and_utf8_names() {
        let value = content_disposition("résumé\"\r\n.mkv");
        assert!(value.starts_with("inline; filename=\"r_sum__.mkv\""));
        assert!(value.contains("filename*=UTF-8''r%C3%A9sum%C3%A9%22%0D%0A%2Emkv"));
        assert!(!value.split("filename*=",).next().unwrap().contains('\r'));
        assert!(!value.split("filename*=",).next().unwrap().contains('\n'));
    }

    #[test]
    fn media_paths_accept_a_safe_format_extension() {
        assert_eq!(media_id_from_path_segment("15"), Some(15));
        assert_eq!(media_id_from_path_segment("15.mp4"), Some(15));
        assert_eq!(media_id_from_path_segment("15.m3u8"), Some(15));
        assert_eq!(media_id_from_path_segment("15."), None);
        assert_eq!(media_id_from_path_segment("15.mp4/cover"), None);
    }

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

    /// Cover art used to be read whole into memory before it was answered, on an
    /// endpoint that needs no credential and searches for ordinary filenames — so
    /// whatever happened to be called `cover.jpg` in a music folder was a copy of
    /// itself in the server's heap per concurrent request.
    #[tokio::test]
    async fn oversized_cover_art_is_passed_over() {
        let temp = tempfile::TempDir::new().expect("temp dir");

        let small = temp.path().join("cover.jpg");
        std::fs::write(&small, vec![0_u8; 4096]).expect("write");
        let response = serve_cover_file(&small, "jpg")
            .await
            .expect("a real cover is served");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_LENGTH)
                .and_then(|value| value.to_str().ok()),
            Some("4096"),
            "and its length is stated rather than accumulated"
        );

        let huge = temp.path().join("folder.jpg");
        let file = std::fs::File::create(&huge).expect("create");
        file.set_len(MAX_COVER_BYTES + 1).expect("grow");
        drop(file);
        assert!(
            serve_cover_file(&huge, "jpg").await.is_none(),
            "a file past the limit is not cover art"
        );

        assert!(
            serve_cover_file(&temp.path().join("absent.jpg"), "jpg")
                .await
                .is_none()
        );
        std::fs::create_dir(temp.path().join("album.png")).expect("mkdir");
        assert!(
            serve_cover_file(&temp.path().join("album.png"), "png")
                .await
                .is_none(),
            "a directory with a cover's name is not one"
        );
    }
}

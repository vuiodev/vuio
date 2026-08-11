//! Browser dashboard handler and compile-time UI template rendering.

use crate::web::format::format_bytes;
use crate::{
    database::{DatabaseManager, DatabaseReadSession, MediaFileQuery, MediaFileView},
    error::AppError,
    state::AppState,
};
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::io::Write as _;

const DASHBOARD_TEMPLATE: &str = include_str!("ui/dashboard.html");

// Third-party player libraries, checked in under ui/vendor/ and compiled into the
// binary. A CDN reference would leave the player dead on the isolated LAN this server
// is built for; see ui/vendor/README.md.
const PLYR_JS: &str = include_str!("ui/vendor/plyr.min.js");
const PLYR_CSS: &str = include_str!("ui/vendor/plyr.css");
const PLYR_SVG: &str = include_str!("ui/vendor/plyr.svg");
const HLS_JS: &str = include_str!("ui/vendor/hls.min.js");
// Plyr's destroy() runs cancelRequests(), which parks the element on `blankVideo` to
// drop the open connection. Its default is a file on cdn.plyr.io, so every closed
// player would reach out to a third party. This is that file, served locally.
const BLANK_MP4: &[u8] = include_bytes!("ui/vendor/blank.mp4");

/// Cache forever: every asset body is a compile-time constant, and the dashboard
/// busts the cache with a `?v=` query when a library is upgraded.
const ASSET_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";

pub async fn root_handler() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        DASHBOARD_TEMPLATE,
    )
}

pub async fn asset_handler(Path(file): Path<String>) -> Result<Response, AppError> {
    let (body, content_type): (&'static [u8], &'static str) = match file.as_str() {
        "plyr.min.js" => (PLYR_JS.as_bytes(), "text/javascript; charset=utf-8"),
        "hls.min.js" => (HLS_JS.as_bytes(), "text/javascript; charset=utf-8"),
        "plyr.css" => (PLYR_CSS.as_bytes(), "text/css; charset=utf-8"),
        "plyr.svg" => (PLYR_SVG.as_bytes(), "image/svg+xml; charset=utf-8"),
        "blank.mp4" => (BLANK_MP4, "video/mp4"),
        _ => return Err(AppError::NotFound),
    };
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, ASSET_CACHE_CONTROL),
        ],
        Bytes::from_static(body),
    )
        .into_response())
}

#[derive(serde::Serialize)]
pub struct ServerInfo {
    server_name: String,
    monitored_directories: Vec<String>,
}

pub async fn server_info_handler<D: DatabaseManager>(
    State(state): State<AppState<D>>,
) -> Json<ServerInfo> {
    let monitored_directories = state
        .media_directories
        .read()
        .await
        .iter()
        .map(|directory| directory.path.clone())
        .collect();
    Json(ServerInfo {
        server_name: state.current_config().server.name.clone(),
        monitored_directories,
    })
}

#[derive(serde::Deserialize)]
pub struct MediaPageQuery {
    cursor: Option<String>,
    limit: Option<usize>,
    category: Option<String>,
    query: Option<String>,
}

pub async fn media_page_handler<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    Query(params): Query<MediaPageQuery>,
) -> Result<Response, AppError> {
    let after_id = params
        .cursor
        .as_deref()
        .map(str::parse::<i64>)
        .transpose()
        .map_err(|_| AppError::InvalidInput("Invalid media cursor".to_string()))?;
    let limit = params.limit.unwrap_or(250).clamp(1, 500);
    let mime_family = match params.category.as_deref().unwrap_or("all") {
        "all" | "" => None,
        "audio" => Some("audio/".to_string()),
        "video" => Some("video/".to_string()),
        "image" => Some("image/".to_string()),
        "radio" => Some("audio/radio".to_string()),
        _ => return Err(AppError::InvalidInput("Unknown media category".to_string())),
    };
    let text = params.query.filter(|value| !value.is_empty());
    let query = MediaFileQuery::Filtered {
        after_id,
        mime_family,
        text,
    };
    let fetch_limit = limit + 1;
    let response = state
        .database
        .clone()
        .read(move |session| {
            let mut output = Vec::with_capacity(limit.saturating_mul(320));
            output.extend_from_slice(b"{\"files\":[");
            let mut emitted = 0_usize;
            let mut last_id = None;
            let summary = session.visit_files(&query, 0, fetch_limit, |file| {
                if emitted >= limit {
                    return Ok(());
                }
                if emitted > 0 {
                    output.push(b',');
                }
                write_web_media_file(&mut output, &file)?;
                last_id = file.id();
                emitted += 1;
                Ok(())
            })?;
            output.extend_from_slice(b"],\"next_cursor\":");
            if summary.visited > limit {
                serde_json::to_writer(&mut output, &last_id.map(|id| id.to_string()))?;
            } else {
                output.extend_from_slice(b"null");
            }
            output.push(b'}');
            Ok(Bytes::from(output))
        })
        .await
        .map_err(AppError::Internal)?;
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        response,
    )
        .into_response())
}

fn write_web_media_file(output: &mut Vec<u8>, file: &impl MediaFileView) -> anyhow::Result<()> {
    let mime_type = file.mime_type();
    let category = if mime_type == "audio/radio" {
        "radio"
    } else {
        mime_type.split('/').next().unwrap_or("file")
    };
    let extension = std::path::Path::new(file.path())
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_lowercase();
    write!(
        output,
        "{{\"id\":{},\"path\":",
        file.id().unwrap_or_default()
    )?;
    serde_json::to_writer(&mut *output, file.path())?;
    output.extend_from_slice(b",\"name\":");
    serde_json::to_writer(&mut *output, file.filename())?;
    output.extend_from_slice(b",\"title\":");
    serde_json::to_writer(&mut *output, &file.title())?;
    output.extend_from_slice(b",\"artist\":");
    serde_json::to_writer(&mut *output, &file.artist())?;
    output.extend_from_slice(b",\"album\":");
    serde_json::to_writer(&mut *output, &file.album())?;
    output.extend_from_slice(b",\"size_str\":");
    serde_json::to_writer(&mut *output, &format_bytes(file.size()))?;
    output.extend_from_slice(b",\"ext\":");
    serde_json::to_writer(&mut *output, &extension)?;
    output.extend_from_slice(b",\"cat\":");
    serde_json::to_writer(&mut *output, category)?;
    // The browser player needs the real MIME to pick a source type, and needs to know
    // whether a sidecar subtitle exists before it attaches a <track>.
    output.extend_from_slice(b",\"mime\":");
    serde_json::to_writer(&mut *output, mime_type)?;
    output.extend_from_slice(b",\"subs\":");
    output.extend_from_slice(if file.subtitle_available() {
        b"true".as_slice()
    } else {
        b"false".as_slice()
    });
    // Null for every video today: the scanner only reads tags for audio/*.
    output.extend_from_slice(b",\"dur\":");
    // serde_json cannot encode NaN/Infinity, and a bad tag must not fail the whole page.
    serde_json::to_writer(&mut *output, &file.duration_secs().filter(|d| d.is_finite()))?;
    output.push(b'}');
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_contains_no_runtime_data_markers() {
        assert!(!DASHBOARD_TEMPLATE.contains("__VUIO_"));
        assert!(DASHBOARD_TEMPLATE.contains("/api/server-info"));
        assert!(DASHBOARD_TEMPLATE.contains("/api/cast"));
        assert!(DASHBOARD_TEMPLATE.contains("playVideoFileOnTv(file)"));
        assert!(DASHBOARD_TEMPLATE.contains("showRendererSelectionModal(renderers, label, source)"));
        assert!(!DASHBOARD_TEMPLATE.contains("renderers.length === 1"));
        assert!(DASHBOARD_TEMPLATE.contains("renderer.pairing === 'required'"));
        assert!(DASHBOARD_TEMPLATE.contains("/api/renderers/pair/start"));
    }

    /// The previous video modal was dead markup for its whole life: `closeVideoPlayer()`
    /// sat in two onclick attributes and was never defined, and nothing opened it. These
    /// assertions tie the template to the handlers and assets that make it work.
    #[test]
    fn dashboard_wires_the_video_player() {
        assert!(DASHBOARD_TEMPLATE.contains("openVideoPlayer(file)"));
        assert!(DASHBOARD_TEMPLATE.contains("function closeVideoPlayer()"));
        assert!(DASHBOARD_TEMPLATE.contains("/assets/plyr.min.js"));
        assert!(DASHBOARD_TEMPLATE.contains("/assets/plyr.css"));
        assert!(DASHBOARD_TEMPLATE.contains("/assets/plyr.svg"));
        assert!(DASHBOARD_TEMPLATE.contains("/assets/hls.min.js"));
        assert!(DASHBOARD_TEMPLATE.contains("/subtitle.vtt"));
        // Every asset the template names must resolve; a typo here is a blank player.
        for asset in ["plyr.min.js", "plyr.css", "plyr.svg", "hls.min.js"] {
            assert!(
                DASHBOARD_TEMPLATE.contains(&format!("/assets/{asset}?v=")),
                "{asset} must be referenced with a cache-busting version"
            );
        }
    }

    #[tokio::test]
    async fn asset_handler_serves_vendored_libraries() {
        for (file, expected_type) in [
            ("plyr.min.js", "text/javascript; charset=utf-8"),
            ("hls.min.js", "text/javascript; charset=utf-8"),
            ("plyr.css", "text/css; charset=utf-8"),
            ("plyr.svg", "image/svg+xml; charset=utf-8"),
            ("blank.mp4", "video/mp4"),
        ] {
            let response = asset_handler(Path(file.to_string()))
                .await
                .unwrap_or_else(|_| panic!("{file} must be served"));
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get(header::CONTENT_TYPE).unwrap(),
                expected_type
            );
        }
        assert!(asset_handler(Path("../ui.rs".to_string())).await.is_err());
    }

    /// Plyr reaches for cdn.plyr.io unless both `iconUrl` and the `source` setter are
    /// avoided. Neither works on the isolated LAN this server targets.
    #[test]
    fn vendored_plyr_is_not_pointed_at_a_cdn() {
        assert!(!PLYR_CSS.contains("http"));
        assert!(DASHBOARD_TEMPLATE.contains("iconUrl: '/assets/plyr.svg?v='"));
        assert!(DASHBOARD_TEMPLATE.contains("blankVideo: '/assets/blank.mp4'"));
        assert!(!DASHBOARD_TEMPLATE.contains("cdn.plyr.io"));
        assert!(PLYR_SVG.contains("id=\"plyr-play\""));
        assert!(BLANK_MP4.starts_with(b"\0\0\0"), "blank.mp4 must be a real MP4");
    }
}

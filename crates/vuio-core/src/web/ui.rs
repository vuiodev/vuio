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
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::io::Write as _;

const DASHBOARD_TEMPLATE: &str = include_str!("ui/dashboard.html");

// The dashboard was one 2800-line file; it is now a shell plus these. They are
// served as separate requests rather than concatenated back together so the
// browser caches each one and devtools line numbers point at real source.
// Load order is fixed by the <script defer> tags in the shell.
const BASE_CSS: &str = include_str!("ui/css/base.css");
const BROWSE_CSS: &str = include_str!("ui/css/browse.css");
const IMAGES_CSS: &str = include_str!("ui/css/images.css");
const VIDEO_CSS: &str = include_str!("ui/css/video.css");
const PLAYER_CSS: &str = include_str!("ui/css/player.css");
const CAST_CSS: &str = include_str!("ui/css/cast.css");
const ADMIN_CSS: &str = include_str!("ui/css/admin.css");

const NAV_JS: &str = include_str!("ui/js/nav.js");
const TOAST_JS: &str = include_str!("ui/js/toast.js");
const MEDIA_DATA_JS: &str = include_str!("ui/js/media-data.js");
const AUDIO_PLAYER_JS: &str = include_str!("ui/js/audio-player.js");
const IMAGES_JS: &str = include_str!("ui/js/images.js");
const VIDEO_PLAYER_JS: &str = include_str!("ui/js/video-player.js");
const CAST_JS: &str = include_str!("ui/js/cast.js");
const BROWSE_JS: &str = include_str!("ui/js/browse.js");
const STATS_JS: &str = include_str!("ui/js/stats.js");
const ADMIN_JS: &str = include_str!("ui/js/admin.js");
const MEDIAINFO_JS: &str = include_str!("ui/js/mediainfo.js");
const INIT_JS: &str = include_str!("ui/js/init.js");

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

/// Cache forever: a vendored library body never changes without its filename
/// gaining a new `?v=`, which the dashboard bumps on upgrade.
const VENDOR_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
/// The dashboard's own CSS and JS change on almost every commit, so a hand-bumped
/// `?v=` would rot immediately. Revalidate instead and let the ETag — a hash of
/// the compiled-in body — decide, which costs one 304 and never serves staleness.
const APP_CACHE_CONTROL: &str = "no-cache";

const JAVASCRIPT: &str = "text/javascript; charset=utf-8";
const STYLESHEET: &str = "text/css; charset=utf-8";

/// Whether an asset's body is frozen for the lifetime of a URL, or has to be
/// revalidated because it changes whenever the dashboard does.
enum AssetCache {
    Immutable,
    Revalidate,
}

fn lookup_asset(file: &str) -> Option<(&'static [u8], &'static str, AssetCache)> {
    use AssetCache::{Immutable, Revalidate};
    let (body, content_type, cache): (&'static str, &'static str, AssetCache) = match file {
        // Vendored third-party libraries.
        "plyr.min.js" => (PLYR_JS, JAVASCRIPT, Immutable),
        "hls.min.js" => (HLS_JS, JAVASCRIPT, Immutable),
        "plyr.css" => (PLYR_CSS, STYLESHEET, Immutable),
        "plyr.svg" => (PLYR_SVG, "image/svg+xml; charset=utf-8", Immutable),
        "blank.mp4" => return Some((BLANK_MP4, "video/mp4", Immutable)),
        // Dashboard stylesheets.
        "base.css" => (BASE_CSS, STYLESHEET, Revalidate),
        "browse.css" => (BROWSE_CSS, STYLESHEET, Revalidate),
        "images.css" => (IMAGES_CSS, STYLESHEET, Revalidate),
        "video.css" => (VIDEO_CSS, STYLESHEET, Revalidate),
        "player.css" => (PLAYER_CSS, STYLESHEET, Revalidate),
        "cast.css" => (CAST_CSS, STYLESHEET, Revalidate),
        "admin.css" => (ADMIN_CSS, STYLESHEET, Revalidate),
        // Dashboard scripts.
        "nav.js" => (NAV_JS, JAVASCRIPT, Revalidate),
        "toast.js" => (TOAST_JS, JAVASCRIPT, Revalidate),
        "media-data.js" => (MEDIA_DATA_JS, JAVASCRIPT, Revalidate),
        "audio-player.js" => (AUDIO_PLAYER_JS, JAVASCRIPT, Revalidate),
        "images.js" => (IMAGES_JS, JAVASCRIPT, Revalidate),
        "video-player.js" => (VIDEO_PLAYER_JS, JAVASCRIPT, Revalidate),
        "cast.js" => (CAST_JS, JAVASCRIPT, Revalidate),
        "browse.js" => (BROWSE_JS, JAVASCRIPT, Revalidate),
        "stats.js" => (STATS_JS, JAVASCRIPT, Revalidate),
        "admin.js" => (ADMIN_JS, JAVASCRIPT, Revalidate),
        "mediainfo.js" => (MEDIAINFO_JS, JAVASCRIPT, Revalidate),
        "init.js" => (INIT_JS, JAVASCRIPT, Revalidate),
        _ => return None,
    };
    Some((body.as_bytes(), content_type, cache))
}

/// FNV-1a over the compiled-in body. Every asset is a compile-time constant, so
/// this is an exact content fingerprint: it changes when and only when the file does.
const fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }
    hash
}

fn etag_for(body: &[u8]) -> String {
    format!("\"{:016x}\"", fnv1a(body))
}

fn etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|candidate| candidate.trim() == etag))
}

pub async fn root_handler() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        DASHBOARD_TEMPLATE,
    )
}

pub async fn asset_handler(
    Path(file): Path<String>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let (body, content_type, cache) = lookup_asset(&file).ok_or(AppError::NotFound)?;
    let AssetCache::Revalidate = cache else {
        return Ok((
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, content_type),
                (header::CACHE_CONTROL, VENDOR_CACHE_CONTROL),
            ],
            Bytes::from_static(body),
        )
            .into_response());
    };

    let etag = etag_for(body);
    if etag_matches(&headers, &etag) {
        return Ok((
            StatusCode::NOT_MODIFIED,
            [
                (header::CACHE_CONTROL, APP_CACHE_CONTROL),
                (header::ETAG, etag.as_str()),
            ],
        )
            .into_response());
    }
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, APP_CACHE_CONTROL),
            (header::ETAG, etag.as_str()),
        ],
        Bytes::from_static(body),
    )
        .into_response())
}

#[derive(serde::Serialize)]
pub struct ServerInfo {
    server_name: String,
    monitored_directories: Vec<String>,
    /// Whether a token was required to get here, so the dashboard knows whether to
    /// offer a sign-out. Nothing previously exposed this and the page could not tell.
    auth_enabled: bool,
    /// The ContentDirectory revision, bumped whenever the library changes.
    ///
    /// UPnP renderers learn about a change by subscription; the dashboard has no
    /// such channel, so it compares this against the revision it last rendered to
    /// know when its view has gone stale.
    library_revision: u32,
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
        auth_enabled: state.auth.enabled(),
        library_revision: state
            .content_update_id
            .load(std::sync::atomic::Ordering::SeqCst),
    })
}

#[derive(serde::Deserialize)]
pub struct MediaPageQuery {
    cursor: Option<String>,
    limit: Option<usize>,
    category: Option<String>,
    query: Option<String>,
}

/// The MIME constraint behind one of the browser's category tabs.
///
/// Shared by the flat listing and the folder listing so a tab cannot come to
/// mean two different things depending on which view is open.
fn mime_family_for_category(category: Option<&str>) -> Result<Option<String>, AppError> {
    match category.unwrap_or("all") {
        "all" | "" => Ok(None),
        "audio" => Ok(Some("audio/".to_string())),
        "video" => Ok(Some("video/".to_string())),
        "image" => Ok(Some("image/".to_string())),
        "radio" => Ok(Some("audio/radio".to_string())),
        _ => Err(AppError::InvalidInput("Unknown media category".to_string())),
    }
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
    let mime_family = mime_family_for_category(params.category.as_deref())?;
    let text = params.query.filter(|value| !value.is_empty());
    let query = MediaFileQuery::Filtered {
        after_id,
        mime_family,
        text,
    };
    let fetch_limit = limit + 1;
    // Uncertain matches stay out of the listing; they are reviewed in the Admin
    // tab rather than shown as though they were the file's real title.
    let min_confidence = state.current_config().mediainfo.min_confidence;
    let response = state
        .database
        .clone()
        .read(move |session| {
            let mut output = Vec::with_capacity(limit.saturating_mul(320));
            output.extend_from_slice(b"{\"files\":[");
            let mut emitted = 0_usize;
            let mut last_id = None;
            // Fetched titles and synopses, collected up front because the writer
            // cannot query the session while the session is lending it a row.
            let mut ids = Vec::with_capacity(fetch_limit);
            session.visit_files(&query, 0, fetch_limit, |file| {
                if let Some(id) = file.id().filter(|id| *id > 0) {
                    ids.push(id);
                }
                Ok(())
            })?;
            let overlays = session
                .mediainfo_overlays(&ids, min_confidence)
                .unwrap_or_default();

            let summary = session.visit_files(&query, 0, fetch_limit, |file| {
                if emitted >= limit {
                    return Ok(());
                }
                if emitted > 0 {
                    output.push(b',');
                }
                let overlay = file.id().and_then(|id| overlays.get(&id));
                write_web_media_file(&mut output, &file, overlay)?;
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

#[derive(serde::Deserialize)]
pub struct BrowseQuery {
    path: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    category: Option<String>,
}

/// One directory's direct children: its subfolders, then its files.
///
/// The flat `/api/media` listing exists to page the whole library and is right
/// for the category tabs. It is the wrong shape for a folder view, which needs
/// to know what is *in one directory* — a question the browser cannot answer
/// from a page of files, because the folders it would have to group by are
/// spread across pages it has not fetched. Grouping client-side therefore does
/// not merely get slow on a large library, it gets the answer wrong.
///
/// The database already indexes this: `directories` is keyed by `parent_path`,
/// and `directory_mime_counts` carries a recursive per-family count. So this is
/// the same work the DLNA browse does (`web::xml::generate_indexed_browse_response`),
/// answered as JSON.
///
/// Paging is by offset rather than by cursor, and deliberately: folders and
/// files are one ordered listing, and an offset here is bounded by the size of
/// a single directory rather than of the library, so it does not reintroduce
/// the deep-paging cost that `/api/media`'s cursor exists to avoid.
pub async fn browse_handler<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    Query(params): Query<BrowseQuery>,
) -> Result<Response, AppError> {
    let mime_family = mime_family_for_category(params.category.as_deref())?;
    let limit = params.limit.unwrap_or(250).clamp(1, 500);
    let offset = params.offset.unwrap_or(0);

    let roots = state
        .media_directories
        .read()
        .await
        .iter()
        .map(|directory| std::path::PathBuf::from(&directory.path))
        .collect::<Vec<_>>();

    let requested = params
        .path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(requested) = requested else {
        return Ok(browse_roots_response(&roots, offset, limit));
    };

    // Canonicalize before the containment check, never after: `..` in the
    // request would otherwise walk out of a root that still looked like a
    // prefix, and turn this into a directory listing of the whole host.
    let canonical = canonical_for_browse(&state, std::path::Path::new(requested));
    let canonical_roots = roots
        .iter()
        .map(|root| canonical_for_browse(&state, root))
        .collect::<Vec<_>>();
    if !canonical_roots
        .iter()
        .any(|root| canonical == *root || canonical.starts_with(root))
    {
        return Err(AppError::InvalidInput(
            "Path is outside every configured media directory".to_string(),
        ));
    }

    let parent = canonical
        .parent()
        .filter(|_| !canonical_roots.contains(&canonical))
        .map(|parent| parent.to_string_lossy().into_owned());
    let canonical_parent = canonical.to_string_lossy().into_owned();
    let min_confidence = state.current_config().mediainfo.min_confidence;

    let response = state
        .database
        .clone()
        .read(move |session| {
            // How many subfolders there are decides where the file page starts:
            // one offset walks the folders first and then continues into the
            // files, so a client pages the listing as it is displayed.
            let directory_count = session
                .visit_direct_subdirectories(
                    &canonical_parent,
                    mime_family.as_deref(),
                    0,
                    0,
                    |_| Ok(()),
                )?
                .matched;
            let directory_limit = limit.min(directory_count.saturating_sub(offset));
            let file_offset = offset.saturating_sub(directory_count);
            let file_limit = limit.saturating_sub(directory_limit);

            let mut output = Vec::with_capacity(limit.saturating_mul(320));
            output.extend_from_slice(b"{\"path\":");
            serde_json::to_writer(&mut output, &canonical_parent)?;
            output.extend_from_slice(b",\"parent\":");
            serde_json::to_writer(&mut output, &parent)?;
            output.extend_from_slice(b",\"folders\":[");
            let mut emitted = 0_usize;
            session.visit_direct_subdirectories(
                &canonical_parent,
                mime_family.as_deref(),
                offset,
                directory_limit,
                |directory| {
                    if emitted > 0 {
                        output.push(b',');
                    }
                    write_browse_folder(&mut output, &directory)?;
                    emitted += 1;
                    Ok(())
                },
            )?;

            let query = MediaFileQuery::Directory {
                path: canonical_parent.clone(),
                mime_family: mime_family.clone(),
            };
            // Collected up front because the writer cannot query the session
            // while the session is lending it a row.
            let mut ids = Vec::with_capacity(file_limit);
            session.visit_files(&query, file_offset, file_limit, |file| {
                if let Some(id) = file.id().filter(|id| *id > 0) {
                    ids.push(id);
                }
                Ok(())
            })?;
            let overlays = session
                .mediainfo_overlays(&ids, min_confidence)
                .unwrap_or_default();

            output.extend_from_slice(b"],\"files\":[");
            let mut emitted = 0_usize;
            let files = session.visit_files(&query, file_offset, file_limit, |file| {
                if emitted > 0 {
                    output.push(b',');
                }
                let overlay = file.id().and_then(|id| overlays.get(&id));
                write_web_media_file(&mut output, &file, overlay)?;
                emitted += 1;
                Ok(())
            })?;

            write!(
                &mut output,
                "],\"total\":{},\"offset\":{offset}}}",
                directory_count + files.matched
            )?;
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

/// The top of the tree: the configured media roots, presented as folders.
///
/// They have no common parent to browse from, and no counter of their own —
/// `directory_mime_counts` describes directories the scan discovered, and a
/// root that is currently empty still has to be listed so the operator can see
/// it is configured.
fn browse_roots_response(roots: &[std::path::PathBuf], offset: usize, limit: usize) -> Response {
    let mut output = Vec::with_capacity(roots.len().saturating_mul(128) + 64);
    output.extend_from_slice(b"{\"path\":null,\"parent\":null,\"folders\":[");
    for (emitted, root) in roots.iter().skip(offset).take(limit).enumerate() {
        if emitted > 0 {
            output.push(b',');
        }
        let path = root.to_string_lossy();
        let name = root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone().into_owned());
        output.extend_from_slice(b"{\"name\":");
        let _ = serde_json::to_writer(&mut output, &name);
        output.extend_from_slice(b",\"path\":");
        let _ = serde_json::to_writer(&mut output, path.as_ref());
        output.extend_from_slice(b",\"file_count\":null}");
    }
    let _ = write!(
        &mut output,
        "],\"files\":[],\"total\":{},\"offset\":{offset}}}",
        roots.len()
    );
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        Bytes::from(output),
    )
        .into_response()
}

/// The stored form of a path, matching how the scanner wrote it.
///
/// Falls back to plain normalization the way the DLNA browse does: a path that
/// cannot be canonicalized — a root that has been unplugged, most often — must
/// still produce a lookup rather than an error.
fn canonical_for_browse<D: DatabaseManager>(
    state: &AppState<D>,
    path: &std::path::Path,
) -> std::path::PathBuf {
    match state.filesystem_manager.get_canonical_path(path) {
        Ok(canonical) => std::path::PathBuf::from(canonical),
        Err(_) => state.filesystem_manager.normalize_path(path),
    }
}

fn write_browse_folder(
    output: &mut Vec<u8>,
    directory: &impl crate::database::DirectoryView,
) -> anyhow::Result<()> {
    output.extend_from_slice(b"{\"name\":");
    serde_json::to_writer(&mut *output, directory.name())?;
    output.extend_from_slice(b",\"path\":");
    serde_json::to_writer(&mut *output, directory.path())?;
    // Recursive, so a folder whose media all sits in grandchildren still reads
    // as full rather than empty.
    write!(output, ",\"file_count\":{}}}", directory.file_count())?;
    Ok(())
}

fn write_web_media_file(
    output: &mut Vec<u8>,
    file: &impl MediaFileView,
    overlay: Option<&crate::database::MediaInfoOverlay>,
) -> anyhow::Result<()> {
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

    // What the media info fetch found, kept under its own keys so the browse view
    // can show it as fetched rather than passing it off as a local tag.
    output.extend_from_slice(b",\"info_title\":");
    serde_json::to_writer(&mut *output, &overlay.and_then(|info| info.title.as_deref()))?;
    output.extend_from_slice(b",\"info_overview\":");
    serde_json::to_writer(
        &mut *output,
        &overlay.and_then(|info| info.overview.as_deref()),
    )?;
    output.extend_from_slice(b",\"info_art\":");
    output.extend_from_slice(if overlay.is_some_and(|info| info.has_artwork) {
        b"true".as_slice()
    } else {
        b"false".as_slice()
    });
    output.push(b'}');
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dashboard's behaviour is spread across the shell and the scripts it loads,
    /// so the assertions that tie markup to handlers have to see all of it.
    fn dashboard_sources() -> String {
        [
            DASHBOARD_TEMPLATE,
            NAV_JS,
            TOAST_JS,
            MEDIA_DATA_JS,
            AUDIO_PLAYER_JS,
            IMAGES_JS,
            VIDEO_PLAYER_JS,
            CAST_JS,
            BROWSE_JS,
            STATS_JS,
            ADMIN_JS,
            MEDIAINFO_JS,
            INIT_JS,
        ]
        .concat()
    }

    #[test]
    fn dashboard_contains_no_runtime_data_markers() {
        let sources = dashboard_sources();
        assert!(!sources.contains("__VUIO_"));
        assert!(sources.contains("/api/server-info"));
        assert!(sources.contains("/api/cast"));
        assert!(sources.contains("playVideoFileOnTv(file)"));
        assert!(sources.contains("showRendererSelectionModal(renderers, label, source)"));
        assert!(!sources.contains("renderers.length === 1"));
        assert!(sources.contains("renderer.pairing === 'required'"));
        assert!(sources.contains("/api/renderers/pair/start"));
    }

    /// The previous video modal was dead markup for its whole life: `closeVideoPlayer()`
    /// sat in two onclick attributes and was never defined, and nothing opened it. These
    /// assertions tie the template to the handlers and assets that make it work.
    #[test]
    fn dashboard_wires_the_video_player() {
        let sources = dashboard_sources();
        assert!(sources.contains("openVideoPlayer(file)"));
        assert!(sources.contains("function closeVideoPlayer()"));
        assert!(sources.contains("/subtitle.vtt"));
        // A vendored library is frozen behind an immutable cache, so its URL is the only
        // thing that can invalidate it; a missing `?v=` would pin browsers to the old copy.
        for asset in ["plyr.min.js", "plyr.css", "plyr.svg", "hls.min.js"] {
            assert!(
                sources.contains(&format!("/assets/{asset}?v=")),
                "{asset} must be referenced with a cache-busting version"
            );
        }
    }

    /// The Admin tab is three moving parts that have to agree: a nav button, a view the
    /// switcher knows about, and the endpoints the script calls.
    #[test]
    fn dashboard_wires_the_admin_tab() {
        assert!(DASHBOARD_TEMPLATE.contains("switchNav('admin')"));
        assert!(DASHBOARD_TEMPLATE.contains("id=\"view-admin\""));
        // Without an entry in NAV_VIEWS the button would do nothing at all.
        assert!(NAV_JS.contains("admin:"));
        assert!(ADMIN_JS.contains("/api/admin/config"));
        assert!(ADMIN_JS.contains("/api/admin/restart"));
        // Sign-out is the only caller of an endpoint that shipped with none.
        assert!(ADMIN_JS.contains("'/logout'"));
        for element in ["admin-nav", "admin-pane-body", "admin-banner", "admin-footer"] {
            assert!(
                DASHBOARD_TEMPLATE.contains(&format!("id=\"{element}\"")),
                "{element} is scripted but missing from the shell"
            );
        }
    }

    /// Every `/assets/…` the dashboard names has to resolve. A typo used to mean a blank
    /// player or an unstyled page discovered by hand; now it fails here.
    #[test]
    fn every_referenced_asset_resolves() {
        let sources = dashboard_sources();
        let mut checked = 0;
        for (_, tail) in sources.match_indices("/assets/").map(|(index, matched)| {
            (index, &sources[index + matched.len()..])
        }) {
            let name: String = tail
                .chars()
                .take_while(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
                })
                .collect();
            // Skip interpolated references such as `/assets/${file}`.
            if name.is_empty() {
                continue;
            }
            assert!(
                lookup_asset(&name).is_some(),
                "{name} is referenced by the dashboard but not served"
            );
            checked += 1;
        }
        assert!(checked >= 18, "expected every split asset to be referenced");
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
            let response = asset_handler(Path(file.to_string()), HeaderMap::new())
                .await
                .unwrap_or_else(|_| panic!("{file} must be served"));
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get(header::CONTENT_TYPE).unwrap(),
                expected_type
            );
            assert_eq!(
                response.headers().get(header::CACHE_CONTROL).unwrap(),
                VENDOR_CACHE_CONTROL
            );
            assert!(response.headers().get(header::ETAG).is_none());
        }
        assert!(asset_handler(Path("../ui.rs".to_string()), HeaderMap::new())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn dashboard_assets_revalidate_against_their_etag() {
        let response = asset_handler(Path("browse.js".to_string()), HeaderMap::new())
            .await
            .expect("browse.js must be served");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            APP_CACHE_CONTROL
        );
        let etag = response
            .headers()
            .get(header::ETAG)
            .expect("app assets must carry an ETag")
            .clone();

        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, etag.clone());
        let cached = asset_handler(Path("browse.js".to_string()), headers)
            .await
            .expect("browse.js must be served");
        assert_eq!(cached.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(cached.headers().get(header::ETAG).unwrap(), &etag);

        // A stale validator must not win.
        let mut stale = HeaderMap::new();
        stale.insert(header::IF_NONE_MATCH, "\"0000000000000000\"".parse().unwrap());
        let refetched = asset_handler(Path("browse.js".to_string()), stale)
            .await
            .expect("browse.js must be served");
        assert_eq!(refetched.status(), StatusCode::OK);

        // Distinct bodies must not collide onto one validator.
        assert_ne!(etag_for(BROWSE_JS.as_bytes()), etag_for(CAST_JS.as_bytes()));
    }

    /// Plyr reaches for cdn.plyr.io unless both `iconUrl` and the `source` setter are
    /// avoided. Neither works on the isolated LAN this server targets.
    #[test]
    fn vendored_plyr_is_not_pointed_at_a_cdn() {
        let sources = dashboard_sources();
        assert!(!PLYR_CSS.contains("http"));
        assert!(sources.contains("iconUrl: '/assets/plyr.svg?v='"));
        assert!(sources.contains("blankVideo: '/assets/blank.mp4'"));
        assert!(!sources.contains("cdn.plyr.io"));
        assert!(PLYR_SVG.contains("id=\"plyr-play\""));
        assert!(BLANK_MP4.starts_with(b"\0\0\0"), "blank.mp4 must be a real MP4");
    }
}

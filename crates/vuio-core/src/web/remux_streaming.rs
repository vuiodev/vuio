//! HLS remux streaming endpoints for MKV web playback.
//!
//! Every rendition (the video track, and each browser-playable — i.e. AAC — audio
//! track) is served as its own self-contained single-track fMP4 stream: its own
//! `index.m3u8`, its own `init.mp4` (a single-track `moov`, not one shared between
//! renditions), and its own numbered segments. MSE requires this: a `SourceBuffer` for
//! one track cannot be initialised from a `moov` that also describes another track.

use crate::{
    database::DatabaseManager,
    error::AppError,
    media::remux::{
        mkv_demuxer::{browser_audio_tracks, browser_video_track, FileInfo, TrackInfo},
        Fmp4Writer, HlsGenerator, MkvDemuxer,
    },
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use std::path::PathBuf;
use tracing::error;

/// Default segment duration target in seconds.
const SEGMENT_DURATION_SECS: u32 = 4;

/// Resolve `{id}` to a file path and probe its tracks/duration. A probe failure (e.g.
/// the file went away) is treated as "no browser-playable tracks" rather than a hard
/// error, so callers fall through to their normal "unsupported" handling.
async fn load_file_info<D: DatabaseManager>(
    state: &AppState<D>,
    id: &str,
) -> Result<(PathBuf, FileInfo), AppError> {
    let file_id = crate::web::streaming::media_id_from_path_segment(id).ok_or(AppError::NotFound)?;
    let file_info = state
        .database
        .get_file_location_by_id(file_id)
        .await
        .map_err(|e| {
            error!("Database error getting file by ID {}: {}", file_id, e);
            AppError::NotFound
        })?
        .ok_or(AppError::NotFound)?;

    let info = MkvDemuxer::inspect(&file_info.path).unwrap_or_else(|_| FileInfo {
        tracks: Vec::new(),
        duration_secs: None,
    });

    Ok((file_info.path, info))
}

pub async fn serve_hls_master<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let (_path, info) = load_file_info(&state, &id).await?;
    let master_playlist = HlsGenerator::build_master_playlist(&id, &info.tracks);

    Ok((
        [
            (header::CONTENT_TYPE, "application/x-mpegURL"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        master_playlist,
    )
        .into_response())
}

pub async fn serve_hls_video_playlist<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let (_path, info) = load_file_info(&state, &id).await?;
    browser_video_track(&info.tracks).ok_or(AppError::NotFound)?;

    let playlist =
        HlsGenerator::build_media_playlist(info.duration_secs.unwrap_or(0.0), SEGMENT_DURATION_SECS);

    Ok((
        [
            (header::CONTENT_TYPE, "application/x-mpegURL"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        playlist,
    )
        .into_response())
}

pub async fn serve_hls_audio_playlist<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path((id, audio_idx)): Path<(String, usize)>,
) -> Result<Response, AppError> {
    let (_path, info) = load_file_info(&state, &id).await?;
    browser_audio_tracks(&info.tracks)
        .get(audio_idx)
        .ok_or(AppError::NotFound)?;

    // Every rendition of the same file shares the same overall timeline.
    let playlist =
        HlsGenerator::build_media_playlist(info.duration_secs.unwrap_or(0.0), SEGMENT_DURATION_SECS);

    Ok((
        [
            (header::CONTENT_TYPE, "application/x-mpegURL"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        playlist,
    )
        .into_response())
}

fn init_segment_response(track: &TrackInfo) -> Response {
    let mut init_bytes = Fmp4Writer::build_ftyp();
    init_bytes.extend_from_slice(&Fmp4Writer::build_moov(track));

    (
        [
            (header::CONTENT_TYPE, "video/mp4"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        init_bytes,
    )
        .into_response()
}

pub async fn serve_hls_video_init_segment<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let (_path, info) = load_file_info(&state, &id).await?;
    let video_track = browser_video_track(&info.tracks).ok_or(AppError::NotFound)?;
    Ok(init_segment_response(video_track))
}

pub async fn serve_hls_audio_init_segment<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path((id, audio_idx)): Path<(String, usize)>,
) -> Result<Response, AppError> {
    let (_path, info) = load_file_info(&state, &id).await?;
    let audio_tracks = browser_audio_tracks(&info.tracks);
    let track = audio_tracks.get(audio_idx).copied().ok_or(AppError::NotFound)?;
    Ok(init_segment_response(track))
}

/// Extract packets for `track` starting at `seq * SEGMENT_DURATION_SECS` and mux them
/// into an fMP4 segment (`moof` + `mdat`). Shared by the video- and audio-segment
/// routes — they differ only in which track they resolve `{id}`/`{idx}` to.
fn build_segment_response(path: &std::path::Path, track: &TrackInfo, seq: u32) -> Response {
    let timescale = Fmp4Writer::timescale_for(track);
    let start_secs = seq as f64 * SEGMENT_DURATION_SECS as f64;

    let packets = MkvDemuxer::extract_track_packets(
        path,
        track.id,
        track.codec_kind,
        timescale,
        start_secs,
        SEGMENT_DURATION_SECS as f64,
    )
    .unwrap_or_default();

    // Seeking lands at (or before) `start_secs`, not exactly on it, so the fragment's
    // base decode time comes from the packets themselves (`build_segment` takes it from
    // the first one's decode timestamp). The nominal `seq`-derived position is only a
    // fallback for a segment that came back empty.
    let nominal_decode_time = (start_secs * timescale as f64).round() as u64;
    let segment_bytes = Fmp4Writer::build_segment(seq + 1, track, nominal_decode_time, &packets);

    (
        [
            (header::CONTENT_TYPE, "video/mp4"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        segment_bytes,
    )
        .into_response()
}

pub async fn serve_hls_video_segment<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path((id, seq)): Path<(String, u32)>,
) -> Result<Response, AppError> {
    let (path, info) = load_file_info(&state, &id).await?;
    let video_track = browser_video_track(&info.tracks).ok_or(AppError::NotFound)?;
    Ok(build_segment_response(&path, video_track, seq))
}

pub async fn serve_hls_audio_segment<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path((id, audio_idx, seq)): Path<(String, usize, u32)>,
) -> Result<Response, AppError> {
    let (path, info) = load_file_info(&state, &id).await?;
    let audio_tracks = browser_audio_tracks(&info.tracks);
    let track = audio_tracks.get(audio_idx).copied().ok_or(AppError::NotFound)?;
    Ok(build_segment_response(&path, track, seq))
}

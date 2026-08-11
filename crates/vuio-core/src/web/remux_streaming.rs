//! HLS remux streaming endpoints for MKV web playback.

use crate::{
    database::DatabaseManager,
    error::AppError,
    media::remux::{Fmp4Writer, HlsGenerator, MkvDemuxer, TrackKind},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use tracing::error;

/// Default segment duration target in seconds.
const SEGMENT_DURATION_SECS: u32 = 4;

/// Maximum packets to extract per segment request.
const PACKETS_PER_SEGMENT: usize = 120;

pub async fn serve_hls_master<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let file_id =
        crate::web::streaming::media_id_from_path_segment(&id).ok_or(AppError::NotFound)?;
    let file_info = state
        .database
        .get_file_location_by_id(file_id)
        .await
        .map_err(|e| {
            error!("Database error getting file by ID {}: {}", file_id, e);
            AppError::NotFound
        })?
        .ok_or(AppError::NotFound)?;

    let info = MkvDemuxer::inspect(&file_info.path).unwrap_or_else(|_| {
        crate::media::remux::mkv_demuxer::FileInfo {
            tracks: Vec::new(),
            duration_secs: None,
        }
    });
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
    let file_id =
        crate::web::streaming::media_id_from_path_segment(&id).ok_or(AppError::NotFound)?;
    let file_info = state
        .database
        .get_file_location_by_id(file_id)
        .await
        .map_err(|_| AppError::NotFound)?
        .ok_or(AppError::NotFound)?;

    let info = MkvDemuxer::inspect(&file_info.path).unwrap_or_else(|_| {
        crate::media::remux::mkv_demuxer::FileInfo {
            tracks: Vec::new(),
            duration_secs: None,
        }
    });

    let duration_secs = info.duration_secs.unwrap_or(40.0);
    let segment_count =
        (duration_secs / SEGMENT_DURATION_SECS as f64).ceil().max(1.0) as usize;

    let playlist =
        HlsGenerator::build_media_playlist("../init.mp4", segment_count, SEGMENT_DURATION_SECS);

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
    Path((id, _audio_idx)): Path<(String, usize)>,
) -> Result<Response, AppError> {
    let file_id =
        crate::web::streaming::media_id_from_path_segment(&id).ok_or(AppError::NotFound)?;
    let file_info = state
        .database
        .get_file_location_by_id(file_id)
        .await
        .map_err(|_| AppError::NotFound)?
        .ok_or(AppError::NotFound)?;

    let info = MkvDemuxer::inspect(&file_info.path).unwrap_or_else(|_| {
        crate::media::remux::mkv_demuxer::FileInfo {
            tracks: Vec::new(),
            duration_secs: None,
        }
    });

    let duration_secs = info.duration_secs.unwrap_or(40.0);
    let segment_count =
        (duration_secs / SEGMENT_DURATION_SECS as f64).ceil().max(1.0) as usize;

    // Audio init segments are served from the audio/{idx}/ path, so the
    // relative URI goes up one level to reach the shared namespace.
    let playlist =
        HlsGenerator::build_media_playlist("../../init.mp4", segment_count, SEGMENT_DURATION_SECS);

    Ok((
        [
            (header::CONTENT_TYPE, "application/x-mpegURL"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        playlist,
    )
        .into_response())
}

pub async fn serve_hls_init_segment<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let file_id =
        crate::web::streaming::media_id_from_path_segment(&id).ok_or(AppError::NotFound)?;
    let file_info = state
        .database
        .get_file_location_by_id(file_id)
        .await
        .map_err(|_| AppError::NotFound)?
        .ok_or(AppError::NotFound)?;

    let info = MkvDemuxer::inspect(&file_info.path).unwrap_or_else(|_| {
        crate::media::remux::mkv_demuxer::FileInfo {
            tracks: Vec::new(),
            duration_secs: None,
        }
    });

    // Build ftyp + moov for *each* relevant track so that both video and audio
    // decoders can initialise from this single init segment.
    let mut init_bytes = Fmp4Writer::build_ftyp();

    let video_track = info
        .tracks
        .iter()
        .find(|t| t.track_kind == TrackKind::Video);
    let first_audio = info
        .tracks
        .iter()
        .find(|t| t.track_kind == TrackKind::Audio);

    if let Some(track) = video_track {
        let moov = Fmp4Writer::build_moov(track);
        init_bytes.extend_from_slice(&moov);
    }
    if let Some(track) = first_audio {
        let moov = Fmp4Writer::build_moov(track);
        init_bytes.extend_from_slice(&moov);
    }

    // If there were no tracks at all, still return a valid (though empty) ftyp.
    Ok((
        [
            (header::CONTENT_TYPE, "video/mp4"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        init_bytes,
    )
        .into_response())
}

pub async fn serve_hls_media_segment<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path((id, seq)): Path<(String, u32)>,
) -> Result<Response, AppError> {
    let file_id =
        crate::web::streaming::media_id_from_path_segment(&id).ok_or(AppError::NotFound)?;
    let file_info = state
        .database
        .get_file_location_by_id(file_id)
        .await
        .map_err(|_| AppError::NotFound)?
        .ok_or(AppError::NotFound)?;

    let info = MkvDemuxer::inspect(&file_info.path).unwrap_or_else(|_| {
        crate::media::remux::mkv_demuxer::FileInfo {
            tracks: Vec::new(),
            duration_secs: None,
        }
    });

    let video_track = info
        .tracks
        .iter()
        .find(|t| t.track_kind == TrackKind::Video)
        .cloned()
        .unwrap_or_else(|| crate::media::remux::mkv_demuxer::TrackInfo {
            id: 1,
            track_kind: TrackKind::Video,
            codec: String::new(),
            language: None,
            name: None,
            sample_rate: None,
            channels: None,
            width: None,
            height: None,
            extra_data: Vec::new(),
        });

    let timescale = Fmp4Writer::timescale_for(&video_track);

    // Skip packets for previous segments. This is a simple but functional
    // approach — each segment request re-opens the file and skips ahead.
    let skip = seq as usize * PACKETS_PER_SEGMENT;
    let packets = MkvDemuxer::extract_track_packets(
        &file_info.path,
        video_track.id,
        skip,
        PACKETS_PER_SEGMENT,
    )
    .unwrap_or_default();

    let base_decode_time = seq as u64 * SEGMENT_DURATION_SECS as u64 * timescale as u64;
    let segment_bytes =
        Fmp4Writer::build_segment(seq + 1, &video_track, base_decode_time, &packets);

    Ok((
        [
            (header::CONTENT_TYPE, "video/mp4"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        segment_bytes,
    )
        .into_response())
}

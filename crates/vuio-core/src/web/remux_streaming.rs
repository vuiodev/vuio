//! HLS remux streaming endpoints for MKV web playback.
//!
//! Every rendition (the video track, and each audio track this build can put in
//! front of a browser) is served as its own self-contained single-track fMP4
//! stream: its own `index.m3u8`, its own `init.mp4` (a single-track `moov`, not
//! one shared between renditions), and its own numbered segments. MSE requires
//! this: a `SourceBuffer` for one track cannot be initialised from a `moov` that
//! also describes another track.
//!
//! Video is always a copy. Audio is a copy when it is already AAC and a decode
//! plus a re-encode when it is AC-3, E-AC-3 or DTS — codecs no browser has ever
//! shipped a decoder for, and which this path used to drop, leaving the film
//! playing silently in the tab. That changes what a segment costs: `moof`+`mdat`
//! around a byte copy became four seconds of audio through a decoder and an
//! encoder, which is why segments are now built off the runtime, rationed by the
//! same permit pool as the DLNA path, and cached.

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

/// The track as the browser will actually receive it.
///
/// A video track, and an AAC audio track, arrive as they are. An AC-3, E-AC-3 or
/// DTS track does not exist in a browser at all, so what arrives is the AAC it
/// was re-encoded into — and the init segment has to describe *that*: an `mp4a`
/// entry whose `esds` carries the encoder's `AudioSpecificConfig`, at the
/// encoder's channel count rather than the source's 5.1.
///
/// Both ends of the HLS path call this, which is the point: the `esds` a player
/// initialises its decoder from and the samples it then feeds that decoder are
/// built from one description, in two different requests.
fn rendition_track(track: &TrackInfo) -> TrackInfo {
    #[cfg(all(feature = "transcode-aac", feature = "demux"))]
    if track.codec_kind.transcode_codec().is_some() {
        let sample_rate = track.sample_rate.unwrap_or(48_000);
        let channels = DECODED_CHANNELS;
        return TrackInfo {
            codec: format!("{} → AAC", track.codec),
            codec_kind: crate::media::remux::TrackCodec::Aac,
            channels: Some(channels as u8),
            extra_data: crate::media::transcode::audio_specific_config(sample_rate, channels),
            ..track.clone()
        };
    }
    track.clone()
}

/// Channels a decoded rendition is delivered in.
///
/// Stereo. A browser tab is not a 5.1 speaker set, and the decoders apply the
/// bitstream's own downmix coefficients when asked for two channels, which is a
/// better mix than anything computed after the fact.
#[cfg(all(feature = "transcode-aac", feature = "demux"))]
const DECODED_CHANNELS: u16 = 2;

fn init_segment_response(track: &TrackInfo) -> Response {
    let mut init_bytes = Fmp4Writer::build_ftyp();
    init_bytes.extend_from_slice(&Fmp4Writer::build_moov(&rendition_track(track)));

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
///
/// Blocking. Demuxing was always file I/O and parsing; a decoded rendition adds
/// a decode and an encode of four seconds of audio on top. Callers run it under
/// `spawn_blocking`, without which one seeking browser takes a runtime worker
/// out of service for the duration.
fn build_segment_bytes(path: &std::path::Path, track: &TrackInfo, seq: u32) -> Vec<u8> {
    let out_track = rendition_track(track);
    let timescale = Fmp4Writer::timescale_for(&out_track);
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

    #[cfg(all(feature = "transcode-aac", feature = "demux"))]
    let packets = match track.codec_kind.transcode_codec() {
        Some(codec) => crate::media::transcode::reencode_to_aac(
            codec,
            &packets,
            out_track.sample_rate.unwrap_or(48_000),
            DECODED_CHANNELS,
            out_track.id,
        )
        .unwrap_or_default(),
        None => packets,
    };

    // Seeking lands at (or before) `start_secs`, not exactly on it, so the fragment's
    // base decode time comes from the packets themselves (`build_segment` takes it from
    // the first one's decode timestamp). The nominal `seq`-derived position is only a
    // fallback for a segment that came back empty.
    let nominal_decode_time = (start_secs * timescale as f64).round() as u64;
    Fmp4Writer::build_segment(seq + 1, &out_track, nominal_decode_time, &packets)
}

/// Build (or recall) one segment and wrap it in a response.
///
/// Three things happen here that did not when this path only copied bytes. The
/// build runs on a blocking thread. It takes a transcoding permit, from the same
/// pool the DLNA path draws on, so the two share one CPU ceiling rather than
/// each keeping its own. And the result is cached: a scrub or a re-buffer asks
/// for the same segment again, and rebuilding it means decoding those four
/// seconds again.
#[cfg_attr(not(feature = "transcode"), allow(unused_variables))]
async fn segment_response<D: DatabaseManager>(
    state: &AppState<D>,
    file_id: i64,
    path: &std::path::Path,
    track: &TrackInfo,
    seq: u32,
) -> Result<Response, AppError> {
    let headers = [
        (header::CONTENT_TYPE, "video/mp4"),
        (header::CACHE_CONTROL, "public, max-age=3600"),
    ];

    #[cfg(feature = "transcode")]
    let key = crate::media::transcode::SegmentKey {
        id: file_id,
        track: track.id,
        seq,
    };
    #[cfg(feature = "transcode")]
    if let Some(cached) = state.transcode.cached_segment(&key).await {
        return Ok((headers, cached).into_response());
    }

    // Only a decoded rendition is rationed. A passthrough copy is file I/O, and
    // refusing it under load would stop a browser from playing a film the CPU
    // was never being asked to work on.
    #[cfg(feature = "transcode")]
    let _permit = if track.codec_kind.transcode_codec().is_some() {
        match state.transcode.try_acquire() {
            Some(permit) => Some(permit),
            None => {
                return Ok((
                    axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    [(header::RETRY_AFTER, "5")],
                    "All transcoding slots are in use.",
                )
                    .into_response())
            }
        }
    } else {
        None
    };

    let owned_path = path.to_path_buf();
    let owned_track = track.clone();
    let bytes = tokio::task::spawn_blocking(move || {
        build_segment_bytes(&owned_path, &owned_track, seq)
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("segment builder panicked: {e}")))?;
    let bytes = bytes::Bytes::from(bytes);

    #[cfg(feature = "transcode")]
    state.transcode.remember_segment(key, bytes.clone()).await;
    #[cfg(not(feature = "transcode"))]
    let _ = file_id;

    Ok((headers, bytes).into_response())
}

pub async fn serve_hls_video_segment<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path((id, seq)): Path<(String, u32)>,
) -> Result<Response, AppError> {
    let file_id = crate::web::streaming::media_id_from_path_segment(&id).ok_or(AppError::NotFound)?;
    let (path, info) = load_file_info(&state, &id).await?;
    let video_track = browser_video_track(&info.tracks)
        .ok_or(AppError::NotFound)?
        .clone();
    segment_response(&state, file_id, &path, &video_track, seq).await
}

pub async fn serve_hls_audio_segment<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path((id, audio_idx, seq)): Path<(String, usize, u32)>,
) -> Result<Response, AppError> {
    let file_id = crate::web::streaming::media_id_from_path_segment(&id).ok_or(AppError::NotFound)?;
    let (path, info) = load_file_info(&state, &id).await?;
    let audio_tracks = browser_audio_tracks(&info.tracks);
    let track = audio_tracks
        .get(audio_idx)
        .copied()
        .ok_or(AppError::NotFound)?
        .clone();
    segment_response(&state, file_id, &path, &track, seq).await
}

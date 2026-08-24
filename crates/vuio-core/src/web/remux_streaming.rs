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
        mkv_cues,
        mkv_demuxer::{browser_audio_tracks, browser_video_track, FileInfo, TrackInfo},
        Fmp4Writer, HlsGenerator, MediaPacket, MkvDemuxer,
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

/// Shortest a segment may be, in seconds.
///
/// A target rather than a length: segments open on the film's own keyframes, so
/// what this actually controls is how many of them get run together. A film with
/// a keyframe every ten seconds has ten-second segments.
const SEGMENT_DURATION_SECS: u32 = 4;

/// How one film's segments are laid out on its timeline.
struct Segmentation {
    /// Where each segment starts, followed by where the last one ends: one more
    /// value than there are segments.
    boundaries: Vec<f64>,
    /// Whether every boundary is a keyframe, and so whether a player may start
    /// decoding at any of them.
    independent: bool,
}

impl Segmentation {
    /// The half-open range segment `seq` covers, or `None` past the end.
    fn range(&self, seq: u32) -> Option<(f64, f64)> {
        let at = seq as usize;
        match (self.boundaries.get(at), self.boundaries.get(at + 1)) {
            (Some(start), Some(end)) if end > start => Some((*start, *end)),
            _ => None,
        }
    }
}

/// Where to cut this film, from the keyframes its container indexes.
///
/// An HLS segment has to open on a keyframe, and a film does not have one every
/// four seconds: a Blu-ray remux runs eight to twelve seconds between them. Cut
/// on a fixed grid and each segment has to round forward to the next keyframe,
/// which lands it several segments further into the film than the player asked
/// for — the picture stops within a few seconds of pressing play, which is the
/// defect this exists to prevent. So the grid comes from the film: its own cue
/// index says where the keyframes are, and consecutive ones closer together
/// than [`SEGMENT_DURATION_SECS`] are run into one segment so that a short-GOP
/// file does not produce thousands of tiny ones.
///
/// A file that indexes nothing falls back to the fixed grid, which is what it
/// always was — right for the web-encoded files that keep their keyframes close
/// together, and honestly declared as not independently startable.
fn segmentation(path: &std::path::Path, info: &FileInfo) -> Segmentation {
    let duration = info.duration_secs.unwrap_or(0.0).max(0.0);
    let keyframes = browser_video_track(&info.tracks)
        .filter(|_| duration > 0.0)
        .map(|video| mkv_cues::cue_times_ms(path, u64::from(video.id)).unwrap_or_default())
        .unwrap_or_default();

    match keyframe_boundaries(&keyframes, duration, f64::from(SEGMENT_DURATION_SECS)) {
        Some(boundaries) => Segmentation {
            boundaries,
            independent: true,
        },
        None => Segmentation {
            boundaries: HlsGenerator::uniform_boundaries(duration, SEGMENT_DURATION_SECS),
            independent: false,
        },
    }
}

/// Turn a film's keyframe times into segment boundaries, or `None` if they will
/// not make a usable set.
///
/// Two rules, and they pull against each other. A keyframe closer to the
/// previous boundary than `target` is passed over, so a file that puts one every
/// second does not produce a playlist of thousands of one-second segments. And
/// the run-up to the end is not cut at all: a final segment shorter than the
/// target is the classic runt that players handle badly, so the last real
/// keyframe before it is skipped and the segment before absorbs the remainder —
/// at most twice the target long.
fn keyframe_boundaries(keyframes_ms: &[u64], duration: f64, target: f64) -> Option<Vec<f64>> {
    if duration <= 0.0 || keyframes_ms.is_empty() {
        return None;
    }
    // The film starts at zero whatever its first keyframe says, or whatever
    // precedes that keyframe would be in no segment at all.
    let mut boundaries = vec![0.0f64];
    for time in keyframes_ms.iter().map(|ms| *ms as f64 / 1000.0) {
        if time >= duration - target {
            break;
        }
        if time - boundaries[boundaries.len() - 1] >= target {
            boundaries.push(time);
        }
    }
    if boundaries.len() < 2 {
        return None;
    }
    boundaries.push(duration);
    Some(boundaries)
}

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
    let (path, info) = load_file_info(&state, &id).await?;
    let master_playlist =
        HlsGenerator::build_master_playlist(&id, &info.tracks, segmentation(&path, &info).independent);

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
    let (path, info) = load_file_info(&state, &id).await?;
    browser_video_track(&info.tracks).ok_or(AppError::NotFound)?;

    let playlist = HlsGenerator::build_media_playlist(&segmentation(&path, &info).boundaries);

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
    let (path, info) = load_file_info(&state, &id).await?;
    browser_audio_tracks(&info.tracks)
        .get(audio_idx)
        .ok_or(AppError::NotFound)?;

    // Every rendition of the same file is cut at the same places, so that the
    // player's audio and video timelines describe the same film.
    let playlist = HlsGenerator::build_media_playlist(&segmentation(&path, &info).boundaries);

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

/// Extract the packets `track` contributes to `[start_secs, end_secs)` and mux
/// them into an fMP4 segment (`moof` + `mdat`). Shared by the video- and
/// audio-segment routes — they differ only in which track they resolve
/// `{id}`/`{idx}` to.
///
/// The range comes from [`segmentation`] rather than from the sequence number,
/// so the segment covers what the playlist said it would. A segment that
/// silently covers something else is the whole defect: a player's timeline is
/// built from the playlist, and media that disagrees with it stalls playback
/// rather than correcting it.
///
/// Blocking. Demuxing was always file I/O and parsing; a decoded rendition adds
/// a decode and an encode of the segment's audio on top. Callers run it under
/// `spawn_blocking`, without which one seeking browser takes a runtime worker
/// out of service for the duration.
fn build_segment_bytes(
    path: &std::path::Path,
    track: &TrackInfo,
    seq: u32,
    start_secs: f64,
    end_secs: f64,
) -> Vec<u8> {
    let out_track = rendition_track(track);
    let timescale = Fmp4Writer::timescale_for(&out_track);

    #[cfg(all(feature = "transcode-aac", feature = "demux"))]
    if let Some(codec) = track.codec_kind.transcode_codec() {
        return build_reencoded_segment(
            path, track, &out_track, codec, seq, timescale, start_secs, end_secs,
        );
    }

    let mut packets = MkvDemuxer::extract_track_packets(
        path,
        track.id,
        track.codec_kind,
        timescale,
        start_secs,
        end_secs - start_secs,
    )
    .unwrap_or_default();

    let nominal_decode_time = (start_secs * timescale as f64).round() as u64;
    close_the_segment(&mut packets, (end_secs * timescale as f64).round() as u64);
    Fmp4Writer::build_segment(seq + 1, &out_track, nominal_decode_time, &packets)
}

/// Stretch the last sample to where the segment ends.
///
/// Every other sample's duration is the gap to the next one's decode time, but
/// the last has no successor and falls back to whatever duration the container
/// declared for it. On a 23.976 fps film stored with millisecond timestamps
/// that is 41 ms where the real gap to the next segment's first frame is 42,
/// which leaves a one-millisecond hole in the player's video buffer at every
/// single segment join. A hole is a hole: the player finds no picture at the
/// playhead, nudges over it, and does that once per segment for the length of
/// the film — until the nudges run out and playback stops.
///
/// The segment's own end is the honest duration for that sample, and it makes
/// the samples cover exactly what the playlist promised.
fn close_the_segment(packets: &mut [MediaPacket], end_ticks: u64) {
    if let Some(last) = packets.last_mut() {
        if let Some(remaining) = end_ticks.checked_sub(last.dts).filter(|d| *d > 0) {
            last.duration = remaining;
        }
    }
}

/// The longest a source frame of any codec this decodes can be, in samples.
///
/// DTS Core's `NBLKS` tops out at 127, for `(127 + 1) * 32` samples; AC-3 and
/// E-AC-3 are 1536. Only used as a guard band on the demuxer request below, so
/// generous is free and short is a lost frame of pre-roll.
#[cfg(all(feature = "transcode-aac", feature = "demux"))]
const LONGEST_SOURCE_FRAME: i64 = 4096;

/// One segment of an audio track that has to be decoded and re-encoded.
///
/// Different from the passthrough path in what it asks the demuxer for: not the
/// segment's own span, but the span the encoder has to be *fed* to produce this
/// segment's frames — which reaches back before the segment begins, to cancel
/// the encoder's delay and to warm its MDCT window up on real audio, and
/// forward past where it ends, because that same delay means the last frame is
/// not finished until samples beyond it have been seen. The re-encode positions
/// what it gets by absolute sample index, so the guard band on the request
/// costs a little decoding and nothing else.
#[cfg(all(feature = "transcode-aac", feature = "demux"))]
#[allow(clippy::too_many_arguments)]
fn build_reencoded_segment(
    path: &std::path::Path,
    track: &TrackInfo,
    out_track: &TrackInfo,
    codec: crate::media::transcode::TranscodeCodec,
    seq: u32,
    timescale: u32,
    start_secs: f64,
    end_secs: f64,
) -> Vec<u8> {
    use crate::media::transcode::AacWindow;

    let rate = out_track.sample_rate.unwrap_or(48_000) as u64;
    let window = AacWindow::covering(
        (start_secs * rate as f64).round() as u64,
        (end_secs * rate as f64).round() as u64,
    );

    let (from, len) = window.source_span();
    let request_from = (from - LONGEST_SOURCE_FRAME).max(0);
    let request_len = (from + len as i64 - request_from).max(0);

    let packets = MkvDemuxer::extract_track_packets(
        path,
        track.id,
        track.codec_kind,
        timescale,
        request_from as f64 / timescale as f64,
        request_len as f64 / timescale as f64,
    )
    .unwrap_or_default();

    let packets = crate::media::transcode::reencode_to_aac(
        codec,
        &packets,
        rate as u32,
        DECODED_CHANNELS,
        out_track.id,
        window,
    )
    .unwrap_or_default();

    Fmp4Writer::build_segment(seq + 1, out_track, window.start_sample(), &packets)
}

/// Build (or recall) one segment and wrap it in a response.
///
/// Three things happen here that did not when this path only copied bytes. The
/// build runs on a blocking thread. It takes a transcoding permit, from the same
/// pool the DLNA path draws on, so the two share one CPU ceiling rather than
/// each keeping its own. And the result is cached: a scrub or a re-buffer asks
/// for the same segment again, and rebuilding it means decoding those seconds
/// again.
#[cfg_attr(not(feature = "transcode"), allow(unused_variables))]
async fn segment_response<D: DatabaseManager>(
    state: &AppState<D>,
    file_id: i64,
    path: &std::path::Path,
    track: &TrackInfo,
    seq: u32,
    range: (f64, f64),
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
    let (start_secs, end_secs) = range;
    let bytes = tokio::task::spawn_blocking(move || {
        build_segment_bytes(&owned_path, &owned_track, seq, start_secs, end_secs)
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
    // A request past the last segment is a request for something the playlist
    // never offered, which is a 404 rather than an empty segment a player would
    // append and then wait on.
    let range = segmentation(&path, &info)
        .range(seq)
        .ok_or(AppError::NotFound)?;
    segment_response(&state, file_id, &path, &video_track, seq, range).await
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
    let range = segmentation(&path, &info)
        .range(seq)
        .ok_or(AppError::NotFound)?;
    segment_response(&state, file_id, &path, &track, seq, range).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A film's keyframes, as a Blu-ray remux actually spaces them.
    const GOP: &[u64] = &[0, 2_002, 12_429, 22_814, 33_242, 43_627, 54_012, 64_398];

    #[test]
    fn boundaries_land_on_keyframes_and_skip_the_ones_too_close_together() {
        let boundaries = keyframe_boundaries(GOP, 74.0, 4.0).expect("a usable set");
        // 2.002 is passed over: two seconds is not a segment.
        assert_eq!(
            boundaries,
            vec![0.0, 12.429, 22.814, 33.242, 43.627, 54.012, 64.398, 74.0]
        );
    }

    /// Every segment has to meet the next, or the player's timeline and the
    /// media stop describing the same film — which is heard as a stall.
    #[test]
    fn boundaries_tile_the_whole_film() {
        let boundaries = keyframe_boundaries(GOP, 74.0, 4.0).expect("a usable set");
        assert_eq!(boundaries.first(), Some(&0.0));
        assert_eq!(boundaries.last(), Some(&74.0));
        assert!(boundaries.windows(2).all(|w| w[1] > w[0]));
    }

    /// A runt final segment is worse than a long one, so the last keyframe
    /// before the end is passed over rather than cut on.
    #[test]
    fn the_film_does_not_end_on_a_fragment_of_a_segment() {
        // A keyframe 0.4s before the end would leave a 0.4s final segment.
        let keyframes = &[0u64, 12_429, 22_814, 23_600];
        let boundaries = keyframe_boundaries(keyframes, 24.0, 4.0).expect("a usable set");
        assert_eq!(boundaries, vec![0.0, 12.429, 24.0]);
    }

    /// A film that indexes nothing, or indexes only its own first frame, has no
    /// keyframe grid to offer and falls back to the fixed one.
    #[test]
    fn a_film_with_nothing_to_cut_on_declines_rather_than_inventing_boundaries() {
        assert!(keyframe_boundaries(&[], 100.0, 4.0).is_none());
        assert!(keyframe_boundaries(&[0], 100.0, 4.0).is_none());
        // No duration means no last boundary, and so no segments.
        assert!(keyframe_boundaries(GOP, 0.0, 4.0).is_none());
    }

    /// Short GOPs must not become thousands of tiny segments.
    #[test]
    fn a_keyframe_every_second_still_makes_segments_of_the_target_length() {
        let keyframes: Vec<u64> = (0..60).map(|i| i * 1_000).collect();
        let boundaries = keyframe_boundaries(&keyframes, 60.0, 4.0).expect("a usable set");
        assert_eq!(
            boundaries,
            vec![0.0, 4.0, 8.0, 12.0, 16.0, 20.0, 24.0, 28.0, 32.0, 36.0, 40.0, 44.0, 48.0, 52.0, 60.0]
        );
    }
}

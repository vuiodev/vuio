//! A film served as fragmented MP4, with the soundtrack decoded on the way past.
//!
//! What phase 4 is for. The television plays the picture and the AC-3 or DTS
//! track produces nothing, so it is offered a second resource: the same film,
//! the same video bitstream copied through untouched, and an AAC audio track
//! made by decoding the original and re-encoding it.
//!
//! ## Why this resource has no `Content-Length`
//!
//! It does not exist until it is produced, and unlike the LPCM resource next
//! door its length cannot be worked out in advance. Video passthrough is
//! predictable only if every sample's size is known, which for Matroska means
//! reading the whole film; the AAC half is not predictable at all, because a
//! lossy encoder's frame sizes depend on the audio. So the body is chunked and
//! its length is unstated. A guessed `Content-Length` would be far worse: a
//! renderer that is promised bytes it never receives reports a failed transfer,
//! where an unstated length costs only the byte-seek nobody could honour anyway.
//!
//! ## How it is seekable regardless
//!
//! By time. `TimeSeekRange.dlna.org` is the DLNA mechanism for exactly this
//! case, and `DLNA.ORG_OP=01` is how a renderer is told to use it: byte seeking
//! unsupported, time seeking supported. A seek is a fresh response built from
//! the same film at a different point — the demuxer seeks by timestamp to the
//! keyframe at or before the request, and the fragments that follow carry the
//! real timeline, so the renderer's position display stays true.
//!
//! Because the seek is by time rather than by byte, it works the same for every
//! audio codec: nothing in it depends on the audio being predictable in size, or
//! on it being passed through rather than re-encoded. A film with an AC-3 track
//! and a film with an AAC one are scrubbed identically.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use tracing::{debug, warn};

use crate::media::remux::{browser_audio_tracks, browser_video_track, FileInfo, MkvDemuxer, TrackInfo, TrackKind};
use crate::media::transcode::ProgressiveStream;
use crate::{database::DatabaseManager, error::AppError, state::AppState};

use super::streaming::media_id_from_path_segment;

/// How many fragments may sit between the muxer and the socket.
///
/// This is the backpressure that stops a television which opens a stream and
/// then reads slowly from pulling a whole film through the decoder and into
/// memory. Two fragments is about four seconds of video.
const PIPELINE_DEPTH: usize = 2;

/// DLNA flags: streaming and background transfer modes, connection stalling,
/// and the DLNA 1.5 marker. Identical to the other transcoded resources — what
/// differs between them is `DLNA.ORG_OP`, which states what can actually be
/// seeked and is therefore set per resource rather than shared.
const DLNA_FLAGS: &str = "DLNA.ORG_FLAGS=01700000000000000000000000000000";

/// `?t=<seconds>` for seeking, `?audio_track=<index>` for selecting audio track.
#[derive(serde::Deserialize, Default)]
pub struct VideoQuery {
    t: Option<f64>,
    audio_track: Option<usize>,
}

/// `GET`/`HEAD /media/{id}/transcode/video.mp4`.
pub async fn serve_transcoded_video<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
    method: Method,
    Query(query): Query<VideoQuery>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let (path, filename, info) = resolve(&state, &id).await?;
    let video = browser_video_track(&info.tracks)
        .ok_or(AppError::NotFound)?
        .clone();
    let audio = if let Some(idx) = query.audio_track {
        let playable = browser_audio_tracks(&info.tracks);
        playable
            .get(idx)
            .copied()
            .cloned()
            .or_else(|| default_audio_track(&info.tracks).cloned())
    } else {
        default_audio_track(&info.tracks).cloned()
    };

    let duration = info.duration_secs.filter(|d| *d > 0.0);
    let requested = headers
        .get("TimeSeekRange.dlna.org")
        .and_then(|value| value.to_str().ok())
        .and_then(parse_npt_start)
        .or(query.t);
    // A seek past the end is clamped rather than refused: a renderer that has
    // drifted a little past a film's declared duration should see the last
    // moment of it, not an error.
    let start = requested
        .unwrap_or(0.0)
        .max(0.0)
        .min(duration.map(|d| (d - 0.1).max(0.0)).unwrap_or(f64::MAX));

    let mut response = Response::builder()
        .status(if requested.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("transferMode.dlna.org", "Streaming")
        // OP=01: time seek yes, byte seek no. Saying otherwise would be worse
        // than saying nothing — a renderer that byte-seeks a resource which
        // cannot honour it stops playing rather than falling back.
        .header(
            "contentFeatures.dlna.org",
            format!("DLNA.ORG_OP=01;DLNA.ORG_CI=1;{DLNA_FLAGS}"),
        );
    if let Some(duration) = duration {
        // A renderer with no length to divide has nothing else to draw a scrub
        // bar from. `mehd` in the init segment says the same thing; this is for
        // the ones that read headers and not boxes.
        response = response.header("X-Content-Duration", format!("{duration:.3}"));
        if requested.is_some() {
            response = response.header(
                "TimeSeekRange.dlna.org",
                format!("npt={start:.3}-{duration:.3}/{duration:.3}"),
            );
        }
    }

    if method == Method::HEAD {
        return Ok(response.body(Body::empty())?);
    }

    // Ration the CPU only once there is real work to do. A `HEAD` decoded
    // nothing, and holding a slot for a renderer that only probes would starve
    // one that is playing.
    let Some(permit) = state.transcode.try_acquire() else {
        return Ok(busy(&state, &filename));
    };

    Ok(response.body(fmp4_body(path, video, audio, start, duration, permit))?)
}

/// Mux the film on a blocking thread, handing fragments over a bounded channel.
///
/// The permit rides along and is released when the body is dropped — which is
/// also what happens when a television disconnects, or seeks, mid-film.
fn fmp4_body(
    path: std::path::PathBuf,
    video: TrackInfo,
    audio: Option<TrackInfo>,
    start: f64,
    duration: Option<f64>,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Body {
    let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<bytes::Bytes>>(PIPELINE_DEPTH);

    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut stream =
            match ProgressiveStream::open(&path, &video, audio.as_ref(), start, duration) {
                Ok(stream) => stream,
                Err(e) => {
                    let _ = tx.blocking_send(Err(std::io::Error::other(e.to_string())));
                    return;
                }
            };

        if tx
            .blocking_send(Ok(bytes::Bytes::from(stream.init_segment())))
            .is_err()
        {
            return;
        }
        while let Some(fragment) = stream.next_fragment() {
            if tx
                .blocking_send(Ok(bytes::Bytes::from(fragment)))
                .is_err()
            {
                return;
            }
        }
    });

    Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx))
}

/// Look the item up and confirm it is a film with a track worth decoding.
async fn resolve<D: DatabaseManager>(
    state: &AppState<D>,
    id: &str,
) -> Result<(std::path::PathBuf, String, FileInfo), AppError> {
    let Some(file_id) = media_id_from_path_segment(id) else {
        return Err(AppError::NotFound);
    };
    if !state.current_config().transcode.enabled {
        return Err(AppError::NotFound);
    }
    let file = state
        .database
        .get_file_location_by_id(file_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let info = MkvDemuxer::inspect(&file.path).map_err(|error| {
        debug!("cannot inspect {} for remuxing: {error}", file.filename);
        AppError::NotFound
    })?;
    Ok((file.path, file.filename, info))
}

/// Which audio track to carry.
///
/// The one the container marks default, then the first this build can produce.
/// Deliberately not clever about language: a wrong guess is worse than a
/// predictable one, and the fix if it turns out to matter is one `<res>` per
/// audio track, which the DIDL already supports.
fn default_audio_track(tracks: &[TrackInfo]) -> Option<&TrackInfo> {
    let playable = |t: &&TrackInfo| t.track_kind == TrackKind::Audio && t.codec_kind.is_playable();
    tracks
        .iter()
        .find(|t| playable(t) && t.is_default)
        .or_else(|| tracks.iter().find(playable))
}

fn busy<D: DatabaseManager>(state: &AppState<D>, filename: &str) -> Response {
    warn!(
        "refusing to remux {}: all {} transcode slots are in use",
        filename,
        state.current_config().transcode.max_concurrent
    );
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::RETRY_AFTER, "5")],
        "All transcoding slots are in use.",
    )
        .into_response()
}

/// The start time of a `TimeSeekRange.dlna.org` header, in seconds.
///
/// Two spellings are legal and both turn up: decimal seconds (`npt=120.5-`) and
/// `hh:mm:ss.fff` (`npt=0:02:00.500-`). A header naming a byte range as well is
/// answered on its time half, which is the half this resource can honour.
pub(crate) fn parse_npt_start(header: &str) -> Option<f64> {
    let npt = header
        .split(&[' ', ';'][..])
        .find_map(|part| part.trim().strip_prefix("npt="))?;
    let start = npt.split('-').next()?.trim();
    if start.is_empty() {
        return None;
    }
    if !start.contains(':') {
        return start.parse::<f64>().ok();
    }
    let mut seconds = 0f64;
    for part in start.split(':') {
        seconds = seconds * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::remux::TrackCodec;

    fn audio(id: u32, codec_kind: TrackCodec, is_default: bool) -> TrackInfo {
        TrackInfo {
            id,
            track_kind: TrackKind::Audio,
            codec: "AC-3".into(),
            codec_kind,
            language: None,
            name: None,
            sample_rate: Some(48_000),
            channels: Some(6),
            width: None,
            height: None,
            is_default,
            extra_data: Vec::new(),
        }
    }

    #[test]
    fn a_multi_audio_film_carries_the_track_the_container_marks_default() {
        let tracks = vec![
            audio(2, TrackCodec::Ac3, false),
            audio(3, TrackCodec::Ac3, true),
            audio(4, TrackCodec::Ac3, false),
        ];
        assert_eq!(default_audio_track(&tracks).map(|t| t.id), Some(3));
    }

    #[test]
    fn with_no_default_marked_the_first_playable_track_is_carried() {
        // The first track here is TrueHD: named, and decoded by nothing
        // vendored. Carrying it would produce a stream of noise.
        let tracks = vec![
            audio(2, TrackCodec::Unsupported, false),
            audio(3, TrackCodec::Ac3, false),
        ];
        let expected = if TrackCodec::Ac3.is_playable() {
            Some(3)
        } else {
            None
        };
        assert_eq!(default_audio_track(&tracks).map(|t| t.id), expected);
    }

    /// A default-marked track this build cannot produce must not win over one it
    /// can: the point of the preference is which track to carry, not whether to
    /// carry a broken one.
    #[test]
    fn a_default_track_this_build_cannot_decode_is_passed_over() {
        let tracks = vec![
            audio(2, TrackCodec::Unsupported, true),
            audio(3, TrackCodec::Aac, false),
        ];
        assert_eq!(default_audio_track(&tracks).map(|t| t.id), Some(3));
    }

    #[test]
    fn npt_is_read_in_both_of_its_legal_spellings() {
        assert_eq!(parse_npt_start("npt=120.5-"), Some(120.5));
        assert_eq!(parse_npt_start("npt=0:02:00.500-"), Some(120.5));
        assert_eq!(parse_npt_start("npt=00:00:30-00:01:00"), Some(30.0));
        // A header that also names bytes is answered on the half we can honour.
        assert_eq!(
            parse_npt_start("npt=10.0-100.0/100.0 bytes=1024-2048/2048"),
            Some(10.0)
        );
    }

    #[test]
    fn a_header_with_no_start_time_is_not_a_seek() {
        assert_eq!(parse_npt_start("npt=-30"), None);
        assert_eq!(parse_npt_start("bytes=0-100"), None);
        assert_eq!(parse_npt_start(""), None);
    }
}

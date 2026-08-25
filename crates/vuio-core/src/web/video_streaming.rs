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

use crate::media::remux::{browser_audio_tracks, browser_video_track, FileInfo, MkvDemuxer, TrackInfo};
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

/// `?t=<seconds>` for seeking, `?audio_track=<index>` to carry one track alone.
///
/// The index is into [`browser_audio_tracks`], the same order the HLS renditions
/// use. Omitted — which is how a television reaches this, since the DIDL never
/// writes the parameter — every playable track is carried and the renderer
/// chooses between them itself.
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
    let audio = audio_tracks(&info.tracks, query.audio_track);

    let duration = info.duration_secs.filter(|d| *d > 0.0);
    // A time seek asked for in a header is a range request and is answered as
    // one. `?t=` is not: it is how the browser player and a test name a starting
    // point, and it gets a plain 200 — a 206 nobody asked for is a response to a
    // range request that was never made.
    let seek_header = headers
        .get("TimeSeekRange.dlna.org")
        .or_else(|| headers.get("timeseekrange.dlna.org"))
        .or_else(|| headers.get(header::RANGE))
        .and_then(|value| value.to_str().ok())
        .and_then(parse_npt_start);
    let requested = seek_header.or(query.t);
    // A seek past the end is clamped rather than refused: a renderer that has
    // drifted a little past a film's declared duration should see the last
    // moment of it, not an error.
    let start = requested
        .unwrap_or(0.0)
        .max(0.0)
        .min(duration.map(|d| (d - 0.1).max(0.0)).unwrap_or(f64::MAX));

    tracing::debug!(
        "transcoded video: id={id}, file={filename}, start={start:.3}s, \
         requested={requested:?}, audio={:?}",
        audio
            .iter()
            .map(|track| (track.id, track.language.as_deref(), track.name.as_deref()))
            .collect::<Vec<_>>()
    );

    let mut response = Response::builder()
        .status(if seek_header.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("transferMode.dlna.org", "Streaming")
        // `DLNA.ORG_OP=ab` is two independent answers: `a` is whether
        // `TimeSeekRange.dlna.org` is honoured and `b` is whether byte ranges
        // are. So this is time seek yes, byte seek no — and the order matters
        // more than anything else in this header, because `01` says the
        // opposite. A renderer told it may byte-seek a resource with no length
        // and no `Accept-Ranges` sends `Range: bytes=` for every scrub, gets the
        // film from the beginning each time, and concludes the file cannot be
        // seeked at all.
        .header(
            "contentFeatures.dlna.org",
            format!("DLNA.ORG_OP=10;DLNA.ORG_CI=1;{DLNA_FLAGS}"),
        );
    if let Some(duration) = duration {
        // A renderer with no length to divide has nothing else to draw a scrub
        // bar from. `mehd` in the init segment says the same thing; this is for
        // the ones that read headers and not boxes.
        response = response.header("X-Content-Duration", format!("{duration:.3}"));
        // The range actually being answered, which DLNA asks for in reply to a
        // request that named one — and only then, since it is an answer rather
        // than an announcement.
        if seek_header.is_some() {
            response = response.header(
                "TimeSeekRange.dlna.org",
                format!("npt={start:.3}-{duration:.3}/{duration:.3}"),
            );
        }
        // Not a DLNA header — DLNA's own `availableSeekRange.dlna.org` belongs
        // to limited-operation content, which this is not. This is the
        // PlayStation spelling of the same statement, read by a handful of
        // renderers and ignored by the rest, and what it states is true: the
        // whole film is reachable, from the first moment to the last.
        response = response.header("X-AvailableSeekRange", format!("1 npt=0.000-{duration:.3}"));
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
    audio: Vec<TrackInfo>,
    start: f64,
    duration: Option<f64>,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Body {
    let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<bytes::Bytes>>(PIPELINE_DEPTH);

    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut stream = match ProgressiveStream::open(&path, &video, &audio, start, duration) {
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

/// Which of a film's soundtracks to carry, and in what order.
///
/// All of them, because a television switches audio track inside its own
/// demuxer and never tells this server it did — so a track it can be switched
/// to has to already be in the body. The one the container marks default is put
/// first, which is what a renderer that takes the leading audio track without
/// asking will play; the rest follow in container order so a viewer reading down
/// the audio menu sees them as the file lists them.
///
/// `only` restricts the answer to one track, by index into
/// [`browser_audio_tracks`] — the same order the HLS renditions are numbered in.
/// That is for the browser player and for narrowing down a report of a bad
/// track; a television never sends it.
fn audio_tracks(tracks: &[TrackInfo], only: Option<usize>) -> Vec<TrackInfo> {
    let playable = browser_audio_tracks(tracks);
    if let Some(index) = only {
        return playable.get(index).copied().cloned().into_iter().collect();
    }
    let default = playable.iter().position(|track| track.is_default);
    let mut ordered: Vec<TrackInfo> = playable.into_iter().cloned().collect();
    if let Some(index) = default {
        ordered[..=index].rotate_right(1);
    }
    ordered
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
    use crate::media::remux::{TrackCodec, TrackKind};

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

    fn ids(tracks: &[TrackInfo], only: Option<usize>) -> Vec<u32> {
        audio_tracks(tracks, only).iter().map(|t| t.id).collect()
    }

    /// Every soundtrack is carried, because the television switches between
    /// them without asking — and the default leads, because a renderer that
    /// takes the first one instead must still get the right one.
    #[test]
    fn every_playable_track_is_carried_with_the_default_leading() {
        let tracks = vec![
            audio(2, TrackCodec::Ac3, false),
            audio(3, TrackCodec::Ac3, true),
            audio(4, TrackCodec::Ac3, false),
        ];
        let expected: Vec<u32> = if TrackCodec::Ac3.is_playable() {
            vec![3, 2, 4]
        } else {
            vec![]
        };
        assert_eq!(ids(&tracks, None), expected);
    }

    /// Moving the default to the front must not reshuffle anything else: the
    /// audio menu should read in the order the file lists them.
    #[test]
    fn the_tracks_behind_the_default_keep_their_container_order() {
        let tracks = vec![
            audio(2, TrackCodec::Aac, false),
            audio(3, TrackCodec::Aac, false),
            audio(4, TrackCodec::Aac, true),
            audio(5, TrackCodec::Aac, false),
        ];
        assert_eq!(ids(&tracks, None), vec![4, 2, 3, 5]);
    }

    #[test]
    fn with_no_default_marked_the_container_order_stands() {
        // The first track here is TrueHD: named, and decoded by nothing
        // vendored. Carrying it would produce a stream of noise.
        let tracks = vec![
            audio(2, TrackCodec::Unsupported, false),
            audio(3, TrackCodec::Ac3, false),
        ];
        let expected: Vec<u32> = if TrackCodec::Ac3.is_playable() {
            vec![3]
        } else {
            vec![]
        };
        assert_eq!(ids(&tracks, None), expected);
    }

    /// A default-marked track this build cannot produce must not take the lead
    /// from one it can: the point of the preference is which track plays first,
    /// not whether to lead with a broken one.
    #[test]
    fn a_default_track_this_build_cannot_decode_is_passed_over() {
        let tracks = vec![
            audio(2, TrackCodec::Unsupported, true),
            audio(3, TrackCodec::Aac, false),
            audio(4, TrackCodec::Aac, false),
        ];
        assert_eq!(ids(&tracks, None), vec![3, 4]);
    }

    /// `?audio_track=` indexes the playable tracks, not the container's, which
    /// is what makes it mean the same track as the HLS rendition of that number.
    #[test]
    fn an_explicit_index_carries_that_track_alone() {
        let tracks = vec![
            audio(2, TrackCodec::Unsupported, false),
            audio(3, TrackCodec::Aac, false),
            audio(4, TrackCodec::Aac, true),
        ];
        assert_eq!(ids(&tracks, Some(0)), vec![3]);
        assert_eq!(ids(&tracks, Some(1)), vec![4]);
        // Out of range is no track rather than a fallback to the default: a
        // renderer asking for a track that is not there has a bug worth seeing.
        assert!(ids(&tracks, Some(2)).is_empty());
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

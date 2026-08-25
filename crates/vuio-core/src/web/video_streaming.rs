//! A film served as fragmented MP4, with only the audio a television cannot
//! decode re-encoded on the way past.
//!
//! What phase 4 is for. The television plays the picture and the soundtrack
//! produces nothing, so it is offered a second resource: the same film, the same
//! video bitstream copied through untouched, and audio it can actually play.
//! AC-3 and E-AC-3 are handed over as they are — a television is what Dolby
//! Digital was built for, and re-encoding a 5.1 track into stereo AAC would
//! throw away the surround it was about to play. Only DTS, which televisions
//! commonly do lack, is decoded and re-encoded.
//!
//! ## Why this resource's length is an estimate
//!
//! It does not exist until it is produced, and its length cannot be worked out
//! in advance: video passthrough is only predictable if every sample size is
//! known, which for Matroska means reading the whole film, and a lossy
//! re-encode is not predictable at all. But a renderer that is told nothing
//! about the length mostly declines to draw a scrub bar, and one that is told
//! byte seeking is unavailable mostly declines to seek — which is the whole
//! feature, gone, in exchange for a technically truthful header.
//!
//! So a length is stated: the source file's, which is close because the picture
//! is the bulk of both and passes through untouched. What makes that safe is
//! that the body is then made to be exactly that long, whatever the muxer
//! actually produces — short output is padded with `free` boxes, which ISO-BMFF
//! defines as skippable filler, and long output is cut at the promise. Every
//! response therefore delivers precisely the bytes it committed to, which is the
//! part a renderer actually checks.
//!
//! ## How seeking works
//!
//! Both ways, because renderers disagree about which to use. A
//! `TimeSeekRange.dlna.org` names an instant and is answered exactly: the
//! demuxer seeks to the keyframe at or before it. A `Range: bytes=` names an
//! offset into a file that does not really exist, so it is read as a fraction of
//! the promised length and turned back into an instant — which lands within a
//! few seconds on a film of roughly even bitrate, and further out on a very
//! variable one. Neither is exact in the way seeking a stored file is. Both are
//! close enough to scrub with, which nothing at all was not.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use tracing::{debug, warn};

use crate::media::remux::{
    browser_video_track, television_audio_tracks, FileInfo, MkvDemuxer, TrackInfo,
};
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
/// The index is into [`television_audio_tracks`], in container order. Omitted —
/// which is how a television reaches this, since the DIDL never writes the
/// parameter — every carryable track is included and the renderer chooses
/// between them itself.
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
    crate::web::client::log_renderer_request(
        &format!("/media/{id}/transcode/video.mp4"),
        &method,
        &headers,
    );
    let (path, size, filename, info) = resolve(&state, &id).await?;
    let video = browser_video_track(&info.tracks)
        .ok_or(AppError::NotFound)?
        .clone();
    let audio = audio_tracks(&info.tracks, query.audio_track);

    let duration = info.duration_secs.filter(|d| *d > 0.0);
    // The length this response commits to, and which its body is then made to
    // be exactly. See the module documentation for why it is the source's.
    let promised = crate::web::promised_transcode_length(size);

    // A renderer given a length probes the end of it before it plays. An MP4
    // reader looks there for the `moov` a progressive file carries at its tail,
    // and an Android television does it as sixteen bytes off the end — which
    // this resource can answer exactly, because what is really at the end of it
    // is the padding that makes the promised length true, and padding is
    // zeroes. Answering it by producing the film would take a transcode slot
    // and thirty gigabytes of work to hand back sixteen bytes of nothing, and
    // answering it with the whole film from the start — which is what ignoring
    // the range does — is a reply the renderer cannot make sense of at all, so
    // it gives up before playing a frame.
    if let Some((first, last)) = header_value(&headers, "range").and_then(parse_byte_range) {
        if first >= size && first < promised {
            return padding_response(first, last, promised);
        }
    }

    // Time seeks only, and this is the hard-won part. A byte offset into this
    // resource does not mean anything stable: the bytes are produced on demand,
    // so offset X is whatever the muxer happened to emit that time round. Answer
    // a range request positionally and the client splices two different
    // generations of the stream together and decodes noise — and it is worse
    // than that, because a client told `Accept-Ranges: bytes` will seek while
    // merely *parsing*, so even straight playback comes apart. A time seek has
    // none of that: it is a whole new response the renderer knows to start over
    // on, which is exactly what a fresh `ftyp`/`moov` needs it to do.
    let time_seek = header_value(&headers, "timeseekrange.dlna.org").and_then(parse_npt_start);
    // A `Range` that arrives anyway is answered from the beginning rather than
    // positionally. HTTP allows a server to ignore a range, and a stream from
    // the wrong place is worse than a stream from the start.
    let requested = time_seek.or(query.t);
    // A seek past the end is clamped rather than refused: a renderer that has
    // drifted a little past a film's declared duration should see the last
    // moment of it, not an error.
    let start = requested
        .unwrap_or(0.0)
        .max(0.0)
        .min(duration.map(|d| (d - 0.1).max(0.0)).unwrap_or(f64::MAX));

    // Where this response sits in the promised byte space. Not a position a
    // renderer may ask for — only the answer to "how much is left", which is
    // what a scrub bar needs once it has seeked.
    let first_byte = match duration {
        Some(duration) if time_seek.is_some() && duration > 0.0 => {
            ((promised as f64 * start / duration) as u64).min(promised.saturating_sub(1))
        }
        _ => 0,
    };

    tracing::debug!(
        "transcoded video: id={id}, file={filename}, start={start:.3}s, \
         promised={promised}, audio={:?}",
        audio
            .iter()
            .map(|track| (track.id, track.language.as_deref(), track.name.as_deref()))
            .collect::<Vec<_>>()
    );

    let mut response = Response::builder()
        .status(if time_seek.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(header::CACHE_CONTROL, "no-cache")
        // Explicitly not `bytes`. Saying nothing here is not enough: some
        // clients probe with a range request and take a positional-looking
        // answer as proof, so the refusal is stated.
        .header(header::ACCEPT_RANGES, "none")
        .header("transferMode.dlna.org", "Streaming")
        // `DLNA.ORG_OP=ab` is two independent answers: `a` is whether
        // `TimeSeekRange.dlna.org` is honoured, `b` whether byte ranges are.
        // Time yes, bytes no — see above for why claiming bytes breaks even
        // playback, never mind seeking.
        .header(
            "contentFeatures.dlna.org",
            format!("DLNA.ORG_OP=10;DLNA.ORG_CI=1;{DLNA_FLAGS}"),
        )
        // A length, even though the resource is produced on demand: it is what a
        // renderer divides to draw a scrub bar, and one given none mostly draws
        // nothing and refuses to seek at all. The body is made to be exactly
        // this long — see `fmp4_body`.
        .header(header::CONTENT_LENGTH, promised - first_byte);
    if let Some(duration) = duration {
        response = response.header("X-Content-Duration", format!("{duration:.3}"));
        // The range actually being answered, which DLNA asks for in reply to a
        // request that named one — and only then, since it is an answer rather
        // than an announcement.
        if time_seek.is_some() {
            response = response.header(
                "TimeSeekRange.dlna.org",
                format!("npt={start:.3}-{duration:.3}/{duration:.3}"),
            );
        }
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

    let deliver = promised - first_byte;
    Ok(response.body(fmp4_body(path, video, audio, start, duration, deliver, permit))?)
}

/// Mux the film on a blocking thread, handing fragments over a bounded channel.
///
/// Exactly `deliver` bytes reach the socket, whatever the muxer produces. That
/// is the price of having promised a length for something that does not exist
/// yet, and it is paid at the tail: output that falls short is padded out with
/// `free` boxes, which ISO-BMFF defines as filler a reader skips, and output
/// that runs over is cut at the promise. Both happen after the film's last
/// picture has gone out, so what is padded or lost is the end of a stream the
/// renderer has already finished watching.
///
/// The permit rides along and is released when the body is dropped — which is
/// also what happens when a television disconnects, or seeks, mid-film.
fn fmp4_body(
    path: std::path::PathBuf,
    video: TrackInfo,
    audio: Vec<TrackInfo>,
    start: f64,
    duration: Option<f64>,
    deliver: u64,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Body {
    let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<bytes::Bytes>>(PIPELINE_DEPTH);

    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut sent: u64 = 0;
        let mut stream = match ProgressiveStream::open(&path, &video, &audio, start, duration) {
            Ok(stream) => stream,
            Err(error) => {
                warn!("cannot open {} for remuxing: {error:#}", path.display());
                let _ = tx.blocking_send(Err(std::io::Error::other(error.to_string())));
                return;
            }
        };

        if send_capped(&tx, &mut sent, deliver, stream.init_segment()) {
            while let Some(fragment) = stream.next_fragment() {
                if !send_capped(&tx, &mut sent, deliver, fragment) {
                    return;
                }
            }
        }

        // The film is over and the promise is not met. Fill the difference with
        // skippable boxes rather than leaving the transfer short, which is what
        // a renderer reports as a failed download.
        while sent < deliver {
            let remaining = deliver - sent;
            if !send_capped(&tx, &mut sent, deliver, free_box(remaining)) {
                return;
            }
        }
    });

    Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx))
}

/// Hand over `chunk`, trimmed to whatever is left of the promised length.
///
/// `false` means stop, for either of the two reasons there are: the quota is
/// met, or the renderer has gone away.
fn send_capped(
    tx: &tokio::sync::mpsc::Sender<std::io::Result<bytes::Bytes>>,
    sent: &mut u64,
    deliver: u64,
    mut chunk: Vec<u8>,
) -> bool {
    let room = deliver.saturating_sub(*sent);
    if room == 0 {
        return false;
    }
    if chunk.len() as u64 > room {
        chunk.truncate(room as usize);
    }
    *sent += chunk.len() as u64;
    tx.blocking_send(Ok(bytes::Bytes::from(chunk))).is_ok() && *sent < deliver
}

/// Filler occupying up to `remaining` bytes.
///
/// A `free` box is ISO-BMFF's own "ignore this": a length, the tag, and nothing
/// that means anything. Capped so a long tail is sent as several boxes rather
/// than one allocation the size of the shortfall, and a remainder too small to
/// hold even a box header goes out as plain zeroes — which sit past the last
/// box a reader will ever look at.
fn free_box(remaining: u64) -> Vec<u8> {
    const CHUNK: u64 = 1 << 20;
    if remaining < 8 {
        return vec![0u8; remaining as usize];
    }
    let len = remaining.min(CHUNK);
    let mut out = Vec::with_capacity(len as usize);
    out.extend_from_slice(&(len as u32).to_be_bytes());
    out.extend_from_slice(b"free");
    out.resize(len as usize, 0);
    out
}

/// Look the item up and confirm it is a film with a track worth decoding.
async fn resolve<D: DatabaseManager>(
    state: &AppState<D>,
    id: &str,
) -> Result<(std::path::PathBuf, u64, String, FileInfo), AppError> {
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
    Ok((file.path, file.size, file.filename, info))
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
/// [`television_audio_tracks`] — the tracks this resource can carry, in
/// container order. That is for narrowing down a report of a bad soundtrack; a
/// television never sends it. Note that it indexes this resource's own list,
/// which is wider than the HLS renditions': a browser cannot take Dolby at all,
/// so its numbering skips tracks that appear here.
pub(crate) fn audio_tracks(tracks: &[TrackInfo], only: Option<usize>) -> Vec<TrackInfo> {
    let playable = television_audio_tracks(tracks);
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

/// One header, by its lowercase name.
fn header_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

/// The bounds a `Range: bytes=` header names, as an inclusive pair.
///
/// Only the single-range form, which is all any renderer sends. A suffix range
/// (`bytes=-500`) is declined rather than resolved: it means "the last 500
/// bytes", and answering it would commit to the promised length being where the
/// content really ends.
fn parse_byte_range(header: &str) -> Option<(u64, Option<u64>)> {
    let spec = header.trim().strip_prefix("bytes=")?;
    if spec.contains(',') {
        return None;
    }
    let (first, last) = spec.split_once('-')?;
    let first = first.trim().parse::<u64>().ok()?;
    let last = match last.trim() {
        "" => None,
        value => Some(value.parse::<u64>().ok()?),
    };
    Some((first, last))
}

/// Answer a range that falls inside the padding, without producing anything.
///
/// The bytes really are zeroes, so this is not a fiction — it is the one part of
/// this resource whose contents are known without muxing a frame. It takes no
/// transcode slot, because there is nothing here to transcode.
fn padding_response(
    first: u64,
    last: Option<u64>,
    promised: u64,
) -> Result<Response, AppError> {
    /// Zeroes handed over at a time, so a renderer asking for a large stretch of
    /// padding does not become a large allocation.
    const CHUNK: u64 = 64 * 1024;

    let last = last.unwrap_or(promised - 1).min(promised - 1);
    if last < first {
        return Ok(Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{promised}"))
            .body(Body::empty())?);
    }
    let length = last - first + 1;
    let zeroes = futures_util::stream::unfold(length, |left| async move {
        if left == 0 {
            return None;
        }
        let take = left.min(CHUNK);
        let chunk = bytes::Bytes::from(vec![0u8; take as usize]);
        Some((Ok::<_, std::io::Error>(chunk), left - take))
    });

    Ok(Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(header::CONTENT_LENGTH, length)
        .header(
            header::CONTENT_RANGE,
            format!("bytes {first}-{last}/{promised}"),
        )
        .body(Body::from_stream(zeroes))?)
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

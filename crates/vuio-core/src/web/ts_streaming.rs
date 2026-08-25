//! A film served as a transport stream, which is the one a television can seek.
//!
//! The `video.mp4` next door is the right answer for the browser player, where
//! the client fetches whole responses and starts each one over. It is the wrong
//! answer for a television, which scrubs by asking for a byte offset — and a
//! byte offset into a fragmented MP4 produced on demand names nothing stable, so
//! answering it positionally hands the renderer the middle of a structure it
//! cannot interpret.
//!
//! A transport stream is built for exactly this. Sync bytes every 188 bytes, the
//! programme tables repeated throughout rather than written once, and the
//! parameter sets a decoder needs travelling beside every keyframe. Land at an
//! arbitrary offset and it finds its footing: the next sync byte, the next
//! tables, the next random-access point, and it plays. That is why a broadcast
//! format is what every DLNA server transcodes to, and why this resource can
//! honestly say `DLNA.ORG_OP=11`.
//!
//! ## What a byte offset means here
//!
//! A fraction. The response commits to a length, and an offset is read as that
//! fraction of the film, then produced from the keyframe at or before it. It is
//! not exact the way seeking a stored file is: a film whose bitrate varies a lot
//! will land seconds away from where the scrub bar said. It is close enough to
//! watch, which is the thing that was not previously true at all.
//!
//! Which makes the promised length the load-bearing number in the whole
//! mechanism, and it is one this resource has to state before a byte of it
//! exists. Promising the source file's own size — the obvious guess, since the
//! picture passes through untouched — is wrong by a factor of three on the films
//! this path exists for: five DTS soundtracks at a megabit and a half each leave
//! as stereo AAC at a fifth of that, so two thirds of the promise is padding and
//! every byte offset names a moment two thirds too far along. So the promise is
//! built instead from what the output will really weigh, with each soundtrack's
//! cost measured off the file rather than assumed from its codec. See
//! [`crate::media::transcode::promised_ts_length`].
//!
//! The body is then made to be exactly that length, padded with null packets —
//! transport stream's own defined filler — so the transfer completes rather than
//! coming up short.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use tracing::{debug, warn};

use crate::media::remux::{browser_video_track, MkvDemuxer, TrackInfo, TS_PACKET_LEN};
use crate::media::transcode::{
    audio_disposition, measure_track_rates, promised_ts_length, AudioDisposition, IndexKey,
    TrackRates, TsStream,
};
use crate::{database::DatabaseManager, error::AppError, state::AppState};

use std::sync::Arc;

use super::streaming::media_id_from_path_segment;
use super::video_streaming::{audio_tracks, parse_npt_start};

/// How many chunks may sit between the muxer and the socket.
const PIPELINE_DEPTH: usize = 2;

/// DLNA flags: streaming and background transfer modes, connection stalling,
/// and the DLNA 1.5 marker.
const DLNA_FLAGS: &str = "DLNA.ORG_FLAGS=01700000000000000000000000000000";

/// `?t=<seconds>` for seeking, `?audio_track=<index>` to carry one track alone.
#[derive(serde::Deserialize, Default)]
pub struct TsQuery {
    t: Option<f64>,
    audio_track: Option<usize>,
}

/// `GET`/`HEAD /media/{id}/transcode/video.ts`.
pub async fn serve_transcoded_ts<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
    method: Method,
    Query(query): Query<TsQuery>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    super::client::log_renderer_request(
        &format!("/media/{id}/transcode/video.ts"),
        &method,
        &headers,
    );
    let (file_id, path, size, filename, info) = resolve(&state, &id).await?;
    let video = browser_video_track(&info.tracks)
        .ok_or(AppError::NotFound)?
        .clone();
    let audio = audio_tracks(&info.tracks, query.audio_track);
    let duration = info.duration_secs.filter(|d| *d > 0.0);
    let promised = match duration {
        Some(duration) => {
            let rates = track_rates(&state, file_id, &path, &info.tracks).await;
            promised_ts_length(size, duration, &info.tracks, &audio, &rates)
        }
        // No duration is no way to turn bytes into instants, so there is nothing
        // better than the source's own size to promise.
        None => super::promised_transcode_length(size),
    };

    // Either mechanism, and both mean the same thing here. A time seek names the
    // instant; a byte offset names the fraction of the promised length that the
    // scrub bar was dragged to, which is the same instant expressed the only way
    // a set that scrubs by byte knows how to express it.
    let time_seek = header_value(&headers, "timeseekrange.dlna.org").and_then(parse_npt_start);
    let byte_seek = header_value(&headers, "range").and_then(parse_byte_start);
    let requested = time_seek
        .or_else(|| {
            byte_seek
                .zip(duration)
                .map(|(offset, duration)| duration * offset as f64 / promised as f64)
        })
        .or(query.t);
    let start = requested
        .unwrap_or(0.0)
        .max(0.0)
        .min(duration.map(|d| (d - 0.1).max(0.0)).unwrap_or(f64::MAX));

    let is_range = byte_seek.is_some() || time_seek.is_some();
    // Where the response sits in the promised byte space. A byte seek already
    // said; a time seek has to be converted so that the `Content-Range` and the
    // length agree with the bar the renderer drew.
    let first_byte = match byte_seek {
        Some(offset) => offset.min(promised.saturating_sub(1)),
        None => match duration {
            Some(duration) if time_seek.is_some() && duration > 0.0 => {
                ((promised as f64 * start / duration) as u64).min(promised.saturating_sub(1))
            }
            _ => 0,
        },
    };

    // A renderer reading the end of the resource rather than seeking into it.
    //
    // It has to be given the *film* there, not filler. A television works out
    // how long a transport stream is by reading its last hundred kilobytes or so
    // and taking the newest timestamp it finds — there is no header to ask, a
    // transport stream not having one. Answer that read with null packets and
    // the set learns nothing: no duration, so no scrub bar, so no seeking, while
    // the picture and every soundtrack play perfectly. Which is the exact shape
    // of the fault reported against this resource.
    //
    // So it is produced like any other seek. But not from the very last instant:
    // a stream can only open on a random-access point, and the offset a duration
    // probe names is past the film's final keyframe, which produces nothing at
    // all — filler again, by a different route. It is pulled back far enough to
    // be sure of landing on one, which costs the set a fractionally short
    // duration and buys it a duration at all.
    let is_probe = byte_seek.is_some() && super::is_probe_tail(first_byte, promised);
    let start = match (is_probe, duration) {
        (true, Some(duration)) => {
            // Comfortably more than any group of pictures, and small against
            // any film — a fifth of a per cent of a feature.
            let backoff = (duration * 0.01).clamp(2.0, 20.0);
            (duration - backoff).max(0.0).min(start)
        }
        _ => start,
    };

    let deliver = ((promised - first_byte) / TS_PACKET_LEN as u64) * TS_PACKET_LEN as u64;
    if deliver == 0 {
        // A range starting inside the stream's final packet. There is no whole
        // packet left to produce, but the question is a perfectly ordinary one —
        // a client sizing up the file reads its last handful of bytes — and
        // refusing it as unsatisfiable is reported as a transfer error rather
        // than shrugged off. So it is answered from the final packet itself,
        // which is padding, and whose bytes are therefore known exactly.
        return final_packet_response(first_byte, promised);
    }

    // Codecs, not just track numbers. Which codec a track is in decides whether
    // it is passed through or decoded, and a stream that will not play is
    // nearly always one whose codec was not what the container claimed or not
    // one this build handles — neither of which is visible from a track id.
    debug!(
        "transcoded ts: id={id}, file={filename}, start={start:.3}s, byte={first_byte}, \
         promised={promised}, video={} ({:?}), audio=[{}]",
        video.codec,
        video.codec_kind,
        audio
            .iter()
            .map(|track| format!(
                "{}:{} {:?} {}ch {}",
                track.id,
                track.language.as_deref().unwrap_or("und"),
                track.codec_kind,
                track.channels.unwrap_or(0),
                match audio_disposition(track) {
                    AudioDisposition::Passthrough => "passthrough",
                    AudioDisposition::Reencoded => "re-encoded",
                    AudioDisposition::Dropped => "dropped",
                }
            ))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let mut response = Response::builder()
        .status(if is_range {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CONTENT_TYPE, "video/mpeg")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::ACCEPT_RANGES, "bytes")
        .header("transferMode.dlna.org", "Streaming")
        // Both, and both are honest: a transport stream resynchronises wherever
        // it is joined, so an approximate byte offset is a usable seek rather
        // than a corrupt one.
        .header(
            "contentFeatures.dlna.org",
            format!("DLNA.ORG_OP=11;DLNA.ORG_CI=1;{DLNA_FLAGS}"),
        )
        // Whole packets, which for a range starting part way through means
        // ending a little short of the promise. HTTP allows a server to answer
        // with less of a range than was asked for, and a decoder handed a
        // fragment of a packet can do nothing with it.
        .header(header::CONTENT_LENGTH, deliver);
    if is_range {
        response = response.header(
            header::CONTENT_RANGE,
            format!("bytes {first_byte}-{}/{promised}", first_byte + deliver - 1),
        );
    }
    if let Some(duration) = duration {
        response = response.header("X-Content-Duration", format!("{duration:.3}"));
        if is_range {
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

    let Some(permit) = state.transcode.try_acquire() else {
        // Nothing left to produce this with. A renderer sizing the resource up
        // rather than playing it can still be told something true — the padding
        // is the one part of this resource whose bytes are known without
        // producing them — and that beats refusing the request outright.
        if is_probe {
            return padding_tail(first_byte, promised);
        }
        return Ok(busy(&state, &filename));
    };

    Ok(response.body(ts_body(path, video, audio, start, deliver, permit))?)
}

/// Mux the film on a blocking thread, handing packets over a bounded channel.
///
/// Exactly `deliver` bytes reach the socket. Short output is padded with null
/// packets, which is transport stream's own filler and what a decoder is already
/// built to skip; long output is cut at the promise, on a packet boundary so
/// that what arrives is never half a packet.
fn ts_body(
    path: std::path::PathBuf,
    video: TrackInfo,
    audio: Vec<TrackInfo>,
    start: f64,
    deliver: u64,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Body {
    let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<bytes::Bytes>>(PIPELINE_DEPTH);

    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut sent: u64 = 0;
        let opened = std::time::Instant::now();
        let mut stream = match TsStream::open(&path, &video, &audio, start) {
            Ok(stream) => stream,
            Err(error) => {
                // Loud, because the renderer's only symptom is a film that will
                // not start: the failure goes down the body as a broken
                // transfer and is otherwise invisible from either end.
                warn!("cannot open {} as a transport stream: {error:#}", path.display());
                let _ = tx.blocking_send(Err(std::io::Error::other(error.to_string())));
                return;
            }
        };

        let mut chunks = 0usize;
        while let Some(chunk) = stream.next_chunk() {
            if chunks == 0 {
                // How long a renderer waited before its first byte, which is
                // the other way this fails: a set that gives up before the
                // first group of pictures has been muxed shows the same nothing
                // as one that was handed something it could not decode.
                debug!(
                    "first chunk for {}: {} bytes after {:.2}s",
                    path.display(),
                    chunk.len(),
                    opened.elapsed().as_secs_f64()
                );
            }
            chunks += 1;
            if !send_capped(&tx, &mut sent, deliver, chunk) {
                debug!("{} stopped after {chunks} chunks, {sent} bytes", path.display());
                return;
            }
        }
        debug!(
            "{} finished: {chunks} chunks, {sent} of {deliver} bytes before padding",
            path.display()
        );
        while sent < deliver {
            let filler = null_packets(deliver - sent);
            if !send_capped(&tx, &mut sent, deliver, filler) {
                return;
            }
        }
    });

    Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx))
}

/// Hand over `chunk`, trimmed to a whole number of packets within what is left.
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
        // On a packet boundary: half a transport packet is not something a
        // decoder can do anything with, and the promise is met by padding.
        chunk.truncate((room as usize / TS_PACKET_LEN) * TS_PACKET_LEN);
    }
    *sent += chunk.len() as u64;
    if chunk.is_empty() {
        return false;
    }
    tx.blocking_send(Ok(bytes::Bytes::from(chunk))).is_ok() && *sent < deliver
}

/// Filler occupying up to `remaining` bytes, as whole null packets.
///
/// A remainder too small to hold one goes out as zeroes, which sit past the last
/// packet any decoder will look at.
fn null_packets(remaining: u64) -> Vec<u8> {
    const CHUNK: u64 = 64 * 1024;
    let len = remaining.min(CHUNK);
    let packets = len as usize / TS_PACKET_LEN;
    if packets == 0 {
        return vec![0u8; remaining as usize];
    }
    let mut out = Vec::with_capacity(packets * TS_PACKET_LEN);
    for _ in 0..packets {
        crate::media::remux::TsMuxer::null(&mut out);
    }
    out
}

/// Answer a range lying inside the stream's last packet, without producing
/// anything.
fn final_packet_response(first: u64, promised: u64) -> Result<Response, AppError> {
    padding_tail(first, promised)
}

/// Serve `[first, promised)` from the stream's padding.
///
/// The stream is padded out to its promised length with null packets, and a null
/// packet is the same 188 bytes every time — so these are the exact bytes a
/// complete read would end with, aligned the same way, rather than a convenient
/// substitute for them. Nothing is demuxed, nothing is decoded, and no transcode
/// slot is taken.
fn padding_tail(first: u64, promised: u64) -> Result<Response, AppError> {
    /// Filler generated at a time, so a renderer asking for a large stretch of
    /// padding does not become a large allocation.
    const CHUNK: u64 = 64 * 1024;

    let length = promised.saturating_sub(first);
    if length == 0 {
        return Ok(Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{promised}"))
            .body(Body::empty())?);
    }
    // Where `first` falls inside a packet, so the bytes line up with the ones a
    // full read would have delivered at this offset.
    let phase = (first % TS_PACKET_LEN as u64) as usize;
    let mut packet = Vec::new();
    crate::media::remux::TsMuxer::null(&mut packet);

    let zeroes = futures_util::stream::unfold((length, phase), |(left, phase)| async move {
        if left == 0 {
            return None;
        }
        let mut packet = Vec::new();
        crate::media::remux::TsMuxer::null(&mut packet);
        let mut out = Vec::with_capacity(CHUNK as usize + TS_PACKET_LEN);
        out.extend_from_slice(&packet[phase..]);
        while (out.len() as u64) < CHUNK.min(left) {
            out.extend_from_slice(&packet);
        }
        let take = (left.min(out.len() as u64)) as usize;
        out.truncate(take);
        Some((
            Ok::<_, std::io::Error>(bytes::Bytes::from(out)),
            (left - take as u64, 0),
        ))
    });

    Ok(Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, "video/mpeg")
        .header(header::CONTENT_LENGTH, length)
        .header(
            header::CONTENT_RANGE,
            format!("bytes {first}-{}/{promised}", promised - 1),
        )
        .body(Body::from_stream(zeroes))?)
}

fn header_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

/// The first byte a `Range` header asks for.
///
/// Only the offset matters: this resource is produced from that point to the end
/// whatever end the header named, which HTTP allows a server to do.
fn parse_byte_start(header: &str) -> Option<u64> {
    let range = header.trim().strip_prefix("bytes=")?;
    let start = range.split('-').next()?.trim();
    if start.is_empty() {
        return None;
    }
    start.parse::<u64>().ok()
}

type Resolved = (
    i64,
    std::path::PathBuf,
    u64,
    String,
    crate::media::remux::FileInfo,
);

async fn resolve<D: DatabaseManager>(state: &AppState<D>, id: &str) -> Result<Resolved, AppError> {
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
    Ok((file_id, file.path, file.size, file.filename, info))
}

/// What this film's tracks cost it, measured once and then remembered.
///
/// Every response for one film has to state the same promised length — a
/// `HEAD`, the `GET` after it and each scrub all divide byte offsets by it, and
/// two different answers are two different films as far as the renderer's scrub
/// bar is concerned. Caching is therefore not only the cheap thing but the
/// correct one.
///
/// A measurement that cannot be taken is not an error. Each track then falls
/// back to its codec's nominal shape, which is what the estimate did before
/// there was anything to measure.
async fn track_rates<D: DatabaseManager>(
    state: &AppState<D>,
    file_id: i64,
    path: &std::path::Path,
    tracks: &[TrackInfo],
) -> Arc<TrackRates> {
    let key = match tokio::fs::metadata(path).await {
        Ok(metadata) => IndexKey {
            id: file_id,
            size: metadata.len(),
            modified: metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        },
        Err(_) => return Arc::new(TrackRates::default()),
    };
    if let Some(rates) = state.transcode.cached_rates(&key).await {
        return rates;
    }

    let owned = path.to_path_buf();
    let wanted = tracks.to_vec();
    let measured = tokio::task::spawn_blocking(move || measure_track_rates(&owned, &wanted))
        .await
        .ok()
        .and_then(|result| result.ok())
        .unwrap_or_default();
    if measured.is_empty() {
        debug!(
            "no track of {} could be measured; falling back to nominal rates",
            path.display()
        );
    }
    let measured = Arc::new(measured);
    state.transcode.remember_rates(key, measured.clone()).await;
    measured
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_names_its_first_byte() {
        assert_eq!(parse_byte_start("bytes=1024-"), Some(1024));
        assert_eq!(parse_byte_start("bytes=1024-2048"), Some(1024));
        assert_eq!(parse_byte_start("bytes=0-"), Some(0));
        // A suffix range needs a real length to resolve against.
        assert_eq!(parse_byte_start("bytes=-500"), None);
        assert_eq!(parse_byte_start("npt=10-"), None);
    }

    /// Padding is whole packets, because a decoder reading the tail should find
    /// the format it was promised rather than a partial one.
    #[test]
    fn padding_is_made_of_whole_null_packets() {
        let filler = null_packets(1000);
        assert_eq!(filler.len() % TS_PACKET_LEN, 0);
        assert_eq!(filler.len(), 5 * TS_PACKET_LEN, "1000 bytes holds five");
        for packet in filler.chunks(TS_PACKET_LEN) {
            assert_eq!(packet[0], 0x47);
            // PID 0x1FFF is the null packet.
            assert_eq!(
                (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]),
                0x1FFF
            );
        }
        // A remainder too small for a packet still fills the promise exactly.
        assert_eq!(null_packets(100).len(), 100);
    }
}

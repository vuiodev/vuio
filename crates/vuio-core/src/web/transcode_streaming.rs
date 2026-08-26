//! Serving AC-3, E-AC-3 and DTS as audio a renderer can actually play.
//!
//! A separate handler from [`super::streaming::serve_media`] rather than a mode
//! of it, because almost every assumption differs. That one opens a file and
//! streams bytes out of it; this one has no file to open — the bytes do not
//! exist until they are decoded — so its length comes from a plan, its DLNA
//! headers have to declare a conversion (`DLNA.ORG_CI=1`, hardcoded to `0`
//! there), and its work is CPU-bound and therefore rationed.
//!
//! What it keeps is the contract a renderer depends on: an exact
//! `Content-Length`, `Accept-Ranges: bytes`, and real 206 responses. Decoded PCM
//! is constant-bitrate, so a byte offset divides cleanly back into a sample and
//! a seek is genuinely a seek. That is worth the pass over the file's headers it
//! takes to know where the frames are.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use tracing::{debug, warn};

use crate::media::transcode::{AudioPlan, IndexKey, PcmStream, TranscodeCodec};
use crate::{database::DatabaseManager, error::AppError, state::AppState};

use super::streaming::{media_id_from_path_segment, parse_range_header};

/// How many decoded frames may sit between the decoder and the socket.
///
/// Small on purpose. This is the backpressure that stops a renderer which opens
/// a stream and then reads slowly from pulling a whole film through the decoder
/// and into memory; a handful of frames is a fraction of a second of audio.
const PIPELINE_DEPTH: usize = 8;

/// `GET`/`HEAD /media/{id}/transcode/audio.aac`.
///
/// The compressed alternative. Unlike the WAV resource this one is chunked: the
/// encoder's output size is not known until it has produced it, so there is no
/// honest `Content-Length` to send and therefore no byte-range seeking either.
/// A renderer gets a stream it can play from the start and not scrub within,
/// which is the trade the operator made by choosing `audio_format = "aac"`.
#[cfg(feature = "transcode-aac")]
pub async fn serve_transcoded_aac<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
    method: Method,
) -> Result<Response, AppError> {
    let file = resolve(&state, &id).await?;
    let Some(permit) = state.transcode.try_acquire() else {
        return Ok(busy(&state, &file.filename));
    };
    let plan = plan_for(&state, &file).await?;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "audio/aac")
        // No Accept-Ranges: saying "bytes" and then refusing every range is
        // worse than never claiming it.
        .header("transferMode.dlna.org", "Streaming")
        // OP=00 — no seeking, in either the time or byte dimension. CI=1 as
        // ever, because these bytes were produced rather than stored.
        .header(
            "contentFeatures.dlna.org",
            "DLNA.ORG_OP=00;DLNA.ORG_CI=1;DLNA.ORG_FLAGS=01700000000000000000000000000000",
        );

    if method == Method::HEAD {
        drop(permit);
        return Ok(response.body(Body::empty())?);
    }
    Ok(response.body(aac_body(plan, permit))?)
}

/// Decode the whole track and re-encode it, frame by frame.
#[cfg(feature = "transcode-aac")]
fn aac_body(plan: Arc<AudioPlan>, permit: tokio::sync::OwnedSemaphorePermit) -> Body {
    use crate::media::transcode::AacEncoder;

    let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<bytes::Bytes>>(PIPELINE_DEPTH);

    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut stream = match PcmStream::open(&plan, 0) {
            Ok(stream) => stream,
            Err(e) => {
                let _ = tx.blocking_send(Err(std::io::Error::other(e.to_string())));
                return;
            }
        };
        let mut encoder = match AacEncoder::new(plan.sample_rate(), plan.channels) {
            Ok(e) => e,
            Err(e) => {
                let _ = tx.blocking_send(Err(std::io::Error::other(e.to_string())));
                return;
            }
        };

        while let Some(pcm) = stream.next_block() {
            match encoder.push(&pcm) {
                Ok(adts) if adts.is_empty() => continue,
                Ok(adts) => {
                    if tx.blocking_send(Ok(bytes::Bytes::from(adts))).is_err() {
                        return;
                    }
                }
                Err(_) => break,
            }
        }
        let tail = encoder.finish();
        if !tail.is_empty() {
            let _ = tx.blocking_send(Ok(bytes::Bytes::from(tail)));
        }
    });

    Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx))
}

/// `GET`/`HEAD /media/{id}/transcode/audio.ac3`.
///
/// The default transcode resource, and the one most televisions were built to
/// decode. Unlike the AAC sibling this one is seekable: AC-3 is constant
/// bitrate and the encoder holds one frame-size code for the whole stream, so
/// the length is known before a byte is produced and a byte offset divides
/// straight back into a syncframe.
///
/// A source that will not say how long it is falls back to a chunked body with
/// no length and no ranges, exactly as the WAV resource does — the same trade,
/// for the same reason.
#[cfg(feature = "transcode-ac3")]
pub async fn serve_transcoded_ac3<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
    method: Method,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let file = resolve(&state, &id).await?;
    let Some(permit) = state.transcode.try_acquire() else {
        return Ok(busy(&state, &file.filename));
    };
    let plan = plan_for(&state, &file).await?;

    let Some(total) = plan.ac3_size() else {
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "audio/ac3")
            .header("transferMode.dlna.org", "Streaming")
            .header(
                "contentFeatures.dlna.org",
                "DLNA.ORG_OP=00;DLNA.ORG_CI=1;DLNA.ORG_FLAGS=01700000000000000000000000000000",
            );
        if method == Method::HEAD {
            drop(permit);
            return Ok(response.body(Body::empty())?);
        }
        return Ok(response.body(ac3_body(plan, 0, u64::MAX, permit))?);
    };

    let (start, end) = match headers.get(header::RANGE) {
        Some(value) => {
            let text = value.to_str().map_err(|_| AppError::InvalidRange)?;
            parse_range_header(text, total)?
        }
        None => (0, total.saturating_sub(1)),
    };
    let len = end.saturating_sub(start) + 1;
    let partial = headers.contains_key(header::RANGE);

    let mut response = Response::builder()
        .status(if partial {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CONTENT_TYPE, "audio/ac3")
        .header(header::CONTENT_LENGTH, len)
        .header(header::ACCEPT_RANGES, "bytes")
        .header("transferMode.dlna.org", "Streaming")
        // CI=1 because these bytes were produced rather than stored; OP=11
        // because constant-bitrate AC-3 genuinely supports the byte seeking
        // that claim promises.
        .header(
            "contentFeatures.dlna.org",
            "DLNA.ORG_OP=11;DLNA.ORG_CI=1;DLNA.ORG_FLAGS=01700000000000000000000000000000",
        );
    if partial {
        response = response.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total}"),
        );
    }

    if method == Method::HEAD {
        drop(permit);
        return Ok(response.body(Body::empty())?);
    }
    Ok(response.body(ac3_body(plan, start, len, permit))?)
}

/// Decode the track and re-encode it as AC-3, from `start` bytes in.
#[cfg(feature = "transcode-ac3")]
fn ac3_body(
    plan: Arc<AudioPlan>,
    start: u64,
    len: u64,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Body {
    use crate::media::transcode::{Ac3Encoder, AC3_FRAME_SAMPLES};

    let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<bytes::Bytes>>(PIPELINE_DEPTH);

    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let (rate, channels) = (plan.sample_rate(), plan.channels);

        // A byte offset lands inside some syncframe. Encoding restarts at that
        // frame's first sample and the odd bytes come off the front, so the
        // renderer receives exactly the bytes it asked for.
        //
        // `None` is a sample rate AC-3 has no frame size for, which is also the
        // case that reached the lengthless body above — there is no offset to
        // resolve, so the encode starts where it would have anyway.
        let (start_sample, mut skip) = match Ac3Encoder::frame_bytes(rate, channels) {
            Some(frame_bytes) if frame_bytes != 0 => {
                let frame_bytes = u64::from(frame_bytes);
                (
                    (start / frame_bytes) * AC3_FRAME_SAMPLES,
                    (start % frame_bytes) as usize,
                )
            }
            _ => (0u64, 0usize),
        };

        let mut stream = match PcmStream::open(&plan, start_sample) {
            Ok(stream) => stream,
            Err(e) => {
                let _ = tx.blocking_send(Err(std::io::Error::other(e.to_string())));
                return;
            }
        };
        let mut encoder = match Ac3Encoder::new(rate, channels) {
            Ok(e) => e,
            Err(e) => {
                let _ = tx.blocking_send(Err(std::io::Error::other(e.to_string())));
                return;
            }
        };

        let mut remaining = len;
        let send = |frames: Vec<Vec<u8>>, remaining: &mut u64, skip: &mut usize| -> bool {
            for frame in frames {
                if *remaining == 0 {
                    return false;
                }
                let frame = if *skip >= frame.len() {
                    *skip -= frame.len();
                    continue;
                } else {
                    &frame[*skip..]
                };
                *skip = 0;
                let take = frame.len().min(*remaining as usize);
                if tx
                    .blocking_send(Ok(bytes::Bytes::copy_from_slice(&frame[..take])))
                    .is_err()
                {
                    return false;
                }
                *remaining -= take as u64;
            }
            true
        };

        while remaining > 0 {
            let Some(pcm) = stream.next_block() else { break };
            match encoder.push(&pcm) {
                Ok(frames) => {
                    if !send(frames, &mut remaining, &mut skip) {
                        return;
                    }
                }
                Err(_) => break,
            }
        }
        if remaining > 0 {
            let _ = send(encoder.finish(), &mut remaining, &mut skip);
        }

        // A renderer promised `len` bytes must receive `len` bytes; the same
        // rule the PCM path follows, for the same reason.
        while remaining > 0 && remaining != u64::MAX {
            let chunk = remaining.min(64 * 1024) as usize;
            if tx
                .blocking_send(Ok(bytes::Bytes::from(vec![0u8; chunk])))
                .is_err()
            {
                return;
            }
            remaining -= chunk as u64;
        }
    });

    Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx))
}

/// `GET`/`HEAD /media/{id}/transcode/audio.wav`.
pub async fn serve_transcoded_wav<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
    method: Method,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let file = resolve(&state, &id).await?;

    // Ration the CPU before doing any of it. A refusal here is deliberate: a
    // renderer told to wait looks to its user like a file that will not open,
    // and the streams already playing would lose CPU to it meanwhile.
    let Some(permit) = state.transcode.try_acquire() else {
        return Ok(busy(&state, &file.filename));
    };

    let plan = plan_for(&state, &file).await?;

    // A source that will not say how long it is gets a chunked body: no length,
    // no ranges, `DLNA.ORG_OP=00`. That loses the scrub bar; a guessed length
    // would lose the transfer, because a `Content-Length` a renderer cannot be
    // given reads as a truncated download rather than a shorter film.
    let Some(total) = plan.wav_size() else {
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "audio/vnd.wave; codec=1")
            .header("transferMode.dlna.org", "Streaming")
            .header(
                "contentFeatures.dlna.org",
                "DLNA.ORG_OP=00;DLNA.ORG_CI=1;DLNA.ORG_FLAGS=01700000000000000000000000000000",
            );
        if method == Method::HEAD {
            drop(permit);
            return Ok(response.body(Body::empty())?);
        }
        return Ok(response.body(pcm_body(plan, 0, u64::MAX, permit))?);
    };

    // Range handling is byte-identical to the passthrough path — the resource
    // just happens not to exist on disk.
    let (start, end) = match headers.get(header::RANGE) {
        Some(value) => {
            let text = value.to_str().map_err(|_| AppError::InvalidRange)?;
            parse_range_header(text, total)?
        }
        None => (0, total.saturating_sub(1)),
    };
    let len = end.saturating_sub(start) + 1;
    let partial = headers.contains_key(header::RANGE);

    let mut response = Response::builder()
        .status(if partial {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CONTENT_TYPE, "audio/vnd.wave; codec=1")
        .header(header::CONTENT_LENGTH, len)
        .header(header::ACCEPT_RANGES, "bytes")
        .header("transferMode.dlna.org", "Streaming")
        // CI=1 says this resource was converted rather than served as stored.
        // OP=11 keeps byte-range seeking, which constant-bitrate PCM genuinely
        // supports — the whole reason the frame index exists.
        .header(
            "contentFeatures.dlna.org",
            "DLNA.ORG_OP=11;DLNA.ORG_CI=1;DLNA.ORG_FLAGS=01700000000000000000000000000000",
        );
    if partial {
        response = response.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total}"),
        );
    }

    if method == Method::HEAD {
        // The permit is dropped here: a HEAD decoded nothing and holding a slot
        // for it would let a renderer that probes before playing starve one that
        // is playing.
        drop(permit);
        return Ok(response.body(Body::empty())?);
    }

    Ok(response.body(pcm_body(plan, start, len, permit))?)
}

/// One library entry, resolved to something transcodable.
pub(crate) struct Resolved {
    pub id: i64,
    pub path: std::path::PathBuf,
    pub filename: String,
    /// The codec of the audio to be decoded.
    pub codec: TranscodeCodec,
    /// How its frames are reached.
    pub kind: SourceKind,
}

/// Whether the frames are the file, or are inside it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SourceKind {
    /// A raw `.ac3`/`.eac3`/`.dts` file.
    Elementary,
    /// One track inside a container.
    #[cfg(feature = "demux")]
    Container,
}

/// Look the item up and confirm this build can decode it.
///
/// A 404 for anything that is not decodable here, rather than an error: the URL
/// describes a resource that, for this file and this build, simply does not
/// exist. Only a renderer that was told about it should be asking.
async fn resolve<D: DatabaseManager>(
    state: &AppState<D>,
    id: &str,
) -> Result<Resolved, AppError> {
    let Some(file_id) = media_id_from_path_segment(id) else {
        return Err(AppError::NotFound);
    };
    if !state.current_config().transcode.enabled {
        return Err(AppError::NotFound);
    }
    let file = state
        .database
        .get_file_by_id(file_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // Everything here comes from what the scanner recorded, not from opening the
    // file: re-probing to answer "is there a second resource?" would repeat work
    // the scan already did, once per item of every Browse response.
    let Some((codec, kind)) = source_for(
        file.stream.codec.as_deref(),
        &file.mime_type,
        &file.filename,
    ) else {
        return Err(AppError::NotFound);
    };
    if !codec.is_decodable() {
        debug!(
            "refusing to transcode {} — this build has no {} decoder",
            file.filename,
            codec.as_str()
        );
        return Err(AppError::NotFound);
    }
    Ok(Resolved {
        id: file_id,
        path: file.path,
        filename: file.filename,
        codec,
        kind,
    })
}

/// Every slot is busy.
fn busy<D: DatabaseManager>(state: &AppState<D>, filename: &str) -> Response {
    warn!(
        "refusing to transcode {}: all {} transcode slots are in use",
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

/// Fetch a cached plan, or build one off the async runtime.
pub(crate) async fn plan_for<D: DatabaseManager>(
    state: &AppState<D>,
    file: &Resolved,
) -> Result<Arc<AudioPlan>, AppError> {
    let metadata = tokio::fs::metadata(&file.path).await?;
    let key = IndexKey {
        id: file.id,
        size: metadata.len(),
        modified: metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    };

    if let Some(plan) = state.transcode.cached(&key).await {
        return Ok(plan);
    }

    let owned = file.path.clone();
    let codec = file.codec;
    let kind = file.kind;
    let plan = tokio::task::spawn_blocking(move || match kind {
        SourceKind::Elementary => AudioPlan::elementary(&owned, codec),
        #[cfg(feature = "demux")]
        SourceKind::Container => AudioPlan::container(&owned),
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("transcode planner panicked: {e}")))?
    .map_err(AppError::Internal)?;
    let plan = Arc::new(plan);
    state.transcode.remember(key, plan.clone()).await;
    Ok(plan)
}

/// Build the response body: the WAV header, then decoded PCM, clipped to the
/// requested byte range.
///
/// Decoding runs on a blocking thread and hands frames over a bounded channel,
/// so a renderer that reads slowly slows the decoder down instead of filling
/// memory. The permit rides along and is released when the body is dropped —
/// which is also what happens when a renderer disconnects mid-stream.
fn pcm_body(
    plan: Arc<AudioPlan>,
    start: u64,
    len: u64,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Body {
    let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<bytes::Bytes>>(PIPELINE_DEPTH);

    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let seeked = plan.seek(start);
        let mut remaining = len;

        // The header, from wherever the range began inside it.
        if seeked.header_skip < 44 {
            let header = plan.wav_header();
            let slice = &header[seeked.header_skip..];
            let take = slice.len().min(remaining as usize);
            if tx
                .blocking_send(Ok(bytes::Bytes::copy_from_slice(&slice[..take])))
                .is_err()
            {
                return;
            }
            remaining -= take as u64;
        }

        if remaining == 0 {
            return;
        }

        let mut stream = match PcmStream::open(&plan, seeked.start_sample) {
            Ok(stream) => stream,
            Err(e) => {
                let _ = tx.blocking_send(Err(std::io::Error::other(e.to_string())));
                return;
            }
        };

        // A range may begin partway through a sample frame, which no decoder
        // can be positioned on — so the odd bytes come off the front here.
        let mut skip = seeked.byte_skip;
        while remaining > 0 {
            let Some(pcm) = stream.next_block() else { break };
            let pcm = if skip >= pcm.len() {
                skip -= pcm.len();
                continue;
            } else {
                &pcm[skip..]
            };
            skip = 0;
            let take = pcm.len().min(remaining as usize);
            if tx
                .blocking_send(Ok(bytes::Bytes::copy_from_slice(&pcm[..take])))
                .is_err()
            {
                return;
            }
            remaining -= take as u64;
        }

        // A renderer promised `len` bytes must receive `len` bytes. If the file
        // shrank under us, or a frame would not read, pad rather than truncate:
        // a short body is a failed transfer, silence is a glitch.
        while remaining > 0 {
            let chunk = remaining.min(64 * 1024) as usize;
            if tx
                .blocking_send(Ok(bytes::Bytes::from(vec![0u8; chunk])))
                .is_err()
            {
                return;
            }
            remaining -= chunk as u64;
        }
    });

    Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx))
}

/// Which codec a library entry holds, from what the scanner recorded.
///
/// The MIME type is the primary signal because it is what the scanner assigned
/// and what the DIDL writer will have advertised. The extension is a fallback
/// for a library indexed before those MIME types existed.
pub(crate) fn codec_for(mime: &str, filename: &str) -> Option<TranscodeCodec> {
    match mime {
        "audio/ac3" => return Some(TranscodeCodec::Ac3),
        "audio/eac3" => return Some(TranscodeCodec::Eac3),
        "audio/vnd.dts" => return Some(TranscodeCodec::Dts),
        _ => {}
    }
    let ext = filename.rsplit('.').next()?.to_ascii_lowercase();
    match ext.as_str() {
        "ac3" => Some(TranscodeCodec::Ac3),
        "eac3" | "ec3" => Some(TranscodeCodec::Eac3),
        "dts" => Some(TranscodeCodec::Dts),
        _ => None,
    }
}

/// Which codec an item's audio is in, and where its frames live.
///
/// The elementary check comes first, and on the file's own identity: an `.ac3`
/// file *is* the bitstream, so its frames are found by walking sync words and
/// the resource it produces is seekable to the sample. Anything else with a
/// recorded AC-3/E-AC-3/DTS codec is a container holding a track — a film — and
/// is demuxed instead. A file with neither has no decoded resource at all.
pub(crate) fn source_for(
    stored_codec: Option<&str>,
    mime: &str,
    filename: &str,
) -> Option<(TranscodeCodec, SourceKind)> {
    if let Some(codec) = codec_for(mime, filename) {
        return Some((codec, SourceKind::Elementary));
    }
    #[cfg(feature = "demux")]
    {
        stored_codec
            .and_then(TranscodeCodec::from_stored_codec)
            .map(|codec| (codec, SourceKind::Container))
    }
    // Without symphonia there is nothing that can open a container, so a film's
    // audio track is simply out of reach and no resource is offered for it.
    #[cfg(not(feature = "demux"))]
    {
        let _ = stored_codec;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_raw_bitstream_is_framed_by_walking_it_and_a_film_is_demuxed() {
        assert_eq!(
            source_for(Some("ac3"), "audio/ac3", "track.ac3"),
            Some((TranscodeCodec::Ac3, SourceKind::Elementary))
        );
        #[cfg(feature = "demux")]
        assert_eq!(
            source_for(Some("ac3"), "video/x-matroska", "Film.mkv"),
            Some((TranscodeCodec::Ac3, SourceKind::Container))
        );
        // A film whose audio is already playable everywhere gets nothing.
        assert_eq!(source_for(Some("aac"), "video/mp4", "Film.mp4"), None);
        // Nor does one nothing vendored decodes.
        assert_eq!(source_for(Some("truehd"), "video/x-matroska", "Film.mkv"), None);
    }

    #[test]
    fn codec_is_read_from_the_mime_the_scanner_assigned() {
        assert_eq!(codec_for("audio/ac3", "x.bin"), Some(TranscodeCodec::Ac3));
        assert_eq!(codec_for("audio/eac3", "x.bin"), Some(TranscodeCodec::Eac3));
        assert_eq!(
            codec_for("audio/vnd.dts", "x.bin"),
            Some(TranscodeCodec::Dts)
        );
    }

    #[test]
    fn a_library_indexed_before_those_mime_types_falls_back_to_the_extension() {
        assert_eq!(
            codec_for("application/octet-stream", "Movie.AC3"),
            Some(TranscodeCodec::Ac3)
        );
        assert_eq!(
            codec_for("application/octet-stream", "track.ec3"),
            Some(TranscodeCodec::Eac3)
        );
    }

    #[test]
    fn anything_that_already_plays_everywhere_is_not_claimed() {
        assert_eq!(codec_for("audio/mpeg", "song.mp3"), None);
        assert_eq!(codec_for("audio/flac", "song.flac"), None);
        assert_eq!(codec_for("video/x-matroska", "film.mkv"), None);
    }
}

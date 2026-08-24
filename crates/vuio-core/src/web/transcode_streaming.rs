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

use crate::media::transcode::{AudioPlan, IndexKey, PcmDecoder, TranscodeCodec};
use crate::{database::DatabaseManager, error::AppError, state::AppState};

use super::streaming::{media_id_from_path_segment, parse_range_header};

/// How many decoded frames may sit between the decoder and the socket.
///
/// Small on purpose. This is the backpressure that stops a renderer which opens
/// a stream and then reads slowly from pulling a whole film through the decoder
/// and into memory; a handful of frames is a fraction of a second of audio.
const PIPELINE_DEPTH: usize = 8;

/// `GET`/`HEAD /media/{id}/transcode/audio.wav`.
pub async fn serve_transcoded_wav<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Path(id): Path<String>,
    method: Method,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let Some(file_id) = media_id_from_path_segment(&id) else {
        return Err(AppError::NotFound);
    };
    if !state.config.transcode.enabled {
        return Err(AppError::NotFound);
    }

    let file = state
        .database
        .get_file_location_by_id(file_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // The codec comes from what the scanner recorded, not from opening the file:
    // this handler is reached by a renderer that was told the resource exists,
    // and re-probing here would repeat work the scan already did.
    let Some(codec) = codec_for(&file.mime_type, &file.filename) else {
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

    // Ration the CPU before doing any of it. A refusal here is deliberate: a
    // renderer told to wait looks to its user like a file that will not open,
    // and the streams already playing would lose CPU to it meanwhile.
    let Some(permit) = state.transcode.try_acquire() else {
        warn!(
            "refusing to transcode {}: all {} transcode slots are in use",
            file.filename, state.config.transcode.max_concurrent
        );
        return Ok((
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::RETRY_AFTER, "5")],
            "All transcoding slots are in use.",
        )
            .into_response());
    };

    let plan = plan_for(&state, file_id, &file.path, codec).await?;
    let total = plan.wav_size();

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

/// Fetch a cached plan, or build one off the async runtime.
async fn plan_for<D: DatabaseManager>(
    state: &AppState<D>,
    file_id: i64,
    path: &std::path::Path,
    codec: TranscodeCodec,
) -> Result<Arc<AudioPlan>, AppError> {
    let metadata = tokio::fs::metadata(path).await?;
    let key = IndexKey {
        id: file_id,
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

    let owned = path.to_path_buf();
    let plan = tokio::task::spawn_blocking(move || AudioPlan::build(&owned, codec))
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

        let file = match std::fs::File::open(&plan.source_path) {
            Ok(f) => f,
            Err(e) => {
                let _ = tx.blocking_send(Err(e));
                return;
            }
        };
        let mut source = std::io::BufReader::with_capacity(256 * 1024, file);

        // A decoder must be primed with the frame it starts on, and AC-3/DTS
        // frames overlap by half a window, so the sample right at a seek point
        // is reconstructed from state the previous frame carried. Starting one
        // frame early and discarding its output removes the transient that
        // would otherwise tick at the start of every seek.
        let preroll = seeked.frame.saturating_sub(1);
        let mut decoder = match prime(&plan, &mut source, preroll) {
            Ok(d) => d,
            Err(e) => {
                let _ = tx.blocking_send(Err(std::io::Error::other(e.to_string())));
                return;
            }
        };

        let mut skip = seeked.pcm_skip;
        for i in preroll..plan.index.frames.len() {
            if remaining == 0 {
                break;
            }
            let frame = plan.index.frames[i];
            let mut raw = vec![0u8; frame.len as usize];
            if read_frame(&mut source, frame.offset, &mut raw).is_err() {
                break;
            }
            let pcm = decoder.decode_or_silence(&raw, frame.samples);

            // Frames before the seek point are decoded for their state only.
            if i < seeked.frame {
                continue;
            }
            let pcm = if skip >= pcm.len() {
                skip -= pcm.len();
                continue;
            } else {
                let out = &pcm[skip..];
                skip = 0;
                out
            };
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

/// Decode every frame up to `upto` so the decoder carries the right state.
fn prime<R: std::io::Read + std::io::Seek>(
    plan: &AudioPlan,
    source: &mut R,
    upto: usize,
) -> anyhow::Result<PcmDecoder> {
    let first = plan.index.frames[upto];
    let mut raw = vec![0u8; first.len as usize];
    read_frame(source, first.offset, &mut raw)?;
    let (decoder, _) = PcmDecoder::open(
        plan.codec,
        plan.index.sample_rate,
        Some(plan.channels),
        &raw,
    )?;
    Ok(decoder)
}

fn read_frame<R: std::io::Read + std::io::Seek>(
    source: &mut R,
    offset: u64,
    into: &mut [u8],
) -> std::io::Result<()> {
    source.seek(std::io::SeekFrom::Start(offset))?;
    source.read_exact(into)
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

#[cfg(test)]
mod tests {
    use super::*;

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

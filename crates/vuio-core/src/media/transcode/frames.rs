//! Locating compressed frames in an elementary stream.
//!
//! A transcoded resource has to answer two questions before a single sample is
//! decoded: how long is it, and where do I start for a byte range? Both come
//! from this index. Decoded PCM is constant-bitrate, so an exact total sample
//! count is an exact `Content-Length`, and a byte offset divides straight back
//! into a sample — which is what lets the transcoded resource support seeking
//! instead of being a one-shot chunked stream.
//!
//! Building it reads headers only. Each frame's header declares its own byte
//! length, so the walk hops frame to frame without decoding anything, and the
//! file is streamed through a fixed buffer rather than read into memory — the
//! AC-3 track of a two-hour film is around 170 MB.

use anyhow::{bail, Context, Result};
use std::io::Read;

use super::TranscodeCodec;

/// How much of the file to hold at once while walking headers.
///
/// Only has to exceed the largest legal frame (DTS caps at 16 384 bytes) by
/// enough that the walk is not re-filling constantly.
const CHUNK: usize = 256 * 1024;

/// Bytes of header needed to determine a frame's length and duration.
/// AC-3/E-AC-3 need six (bsid lives at byte 5); DTS needs its full 14-byte
/// header, and `parse_frame_header` is handed more than that anyway.
const MIN_HEADER: usize = 16;

/// One compressed frame located in the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexedFrame {
    /// Byte offset of the frame's syncword from the start of the stream.
    pub offset: u64,
    /// Frame length in bytes, as the frame's own header declares it.
    pub len: u32,
    /// Sample frames this yields when decoded — per channel, not per sample.
    pub samples: u32,
}

/// Every frame of one elementary stream, with the totals that describe it.
#[derive(Debug, Clone)]
pub struct FrameIndex {
    /// Which codec the frames are in.
    pub codec: TranscodeCodec,
    /// Sample rate declared by the first frame, in Hz.
    pub sample_rate: u32,
    /// The frames, in stream order.
    pub frames: Vec<IndexedFrame>,
    /// Sum of every frame's `samples` — the exact decoded length.
    pub total_samples: u64,
}

impl FrameIndex {
    /// Walk `reader` from its current position, indexing every frame.
    ///
    /// Bytes that do not parse as a frame header are skipped by scanning for the
    /// next syncword: an elementary file can open with an ID3 tag, and a stream
    /// pulled off a disc can carry a damaged frame in the middle. Neither should
    /// cost the whole file.
    pub fn build<R: Read>(codec: TranscodeCodec, reader: &mut R) -> Result<Self> {
        let mut buf: Vec<u8> = Vec::with_capacity(CHUNK);
        let mut frames = Vec::new();
        let mut total_samples: u64 = 0;
        let mut sample_rate = 0u32;
        // Absolute offset of buf[0] within the stream.
        let mut base: u64 = 0;
        let mut pos: usize = 0;
        let mut eof = false;

        loop {
            // Refill: drop what has been consumed, then top up to CHUNK.
            if pos > 0 {
                buf.drain(..pos);
                base += pos as u64;
                pos = 0;
            }
            while !eof && buf.len() < CHUNK {
                let mut tmp = [0u8; 64 * 1024];
                let n = reader.read(&mut tmp).context("reading elementary stream")?;
                if n == 0 {
                    eof = true;
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            if buf.len().saturating_sub(pos) < MIN_HEADER {
                break;
            }

            match parse_header(codec, &buf[pos..]) {
                Ok(Some(hdr)) => {
                    // A frame whose tail is past the buffer end needs a refill,
                    // unless there is nothing left to read — then it is truncated
                    // and the index simply stops before it.
                    if pos + hdr.len as usize > buf.len() {
                        if eof {
                            break;
                        }
                        // Force a drain-and-refill without consuming the header.
                        if pos == 0 {
                            // Already at the front and still short: the declared
                            // length exceeds anything legal, so resync past it.
                            pos += 2;
                            continue;
                        }
                        buf.drain(..pos);
                        base += pos as u64;
                        pos = 0;
                        while !eof && buf.len() < CHUNK {
                            let mut tmp = [0u8; 64 * 1024];
                            let n = reader.read(&mut tmp).context("reading elementary stream")?;
                            if n == 0 {
                                eof = true;
                                break;
                            }
                            buf.extend_from_slice(&tmp[..n]);
                        }
                        continue;
                    }
                    if sample_rate == 0 {
                        sample_rate = hdr.sample_rate;
                    }
                    frames.push(IndexedFrame {
                        offset: base + pos as u64,
                        len: hdr.len,
                        samples: hdr.samples,
                    });
                    total_samples += u64::from(hdr.samples);
                    pos += hdr.len as usize;
                }
                Ok(None) | Err(_) => {
                    // Not a frame here. Scan for the next syncword rather than
                    // giving up: leading tags and single damaged frames are both
                    // recoverable, and a file that is not this codec at all
                    // simply produces no frames and is rejected below.
                    match next_sync(codec, &buf[pos + 1..]) {
                        Some(skip) => pos += 1 + skip,
                        None => {
                            pos = buf.len().saturating_sub(MIN_HEADER.saturating_sub(1));
                            if eof {
                                break;
                            }
                        }
                    }
                }
            }
        }

        if frames.is_empty() {
            bail!("no {} frames found in the stream", codec.as_str());
        }
        if sample_rate == 0 {
            bail!("{} stream declares no usable sample rate", codec.as_str());
        }

        Ok(Self {
            codec,
            sample_rate,
            frames,
            total_samples,
        })
    }

    /// Duration in seconds, from the exact sample count.
    pub fn duration_secs(&self) -> f64 {
        self.total_samples as f64 / self.sample_rate as f64
    }

    /// Index of the frame containing `sample`, and the samples to discard from
    /// its front to land exactly on `sample`.
    ///
    /// Returns the last frame when `sample` is past the end, so a range request
    /// beyond the stream produces silence rather than an error.
    pub fn locate(&self, sample: u64) -> (usize, u32) {
        let mut acc: u64 = 0;
        for (i, f) in self.frames.iter().enumerate() {
            let next = acc + u64::from(f.samples);
            if sample < next {
                return (i, (sample - acc) as u32);
            }
            acc = next;
        }
        (self.frames.len().saturating_sub(1), 0)
    }
}

/// What a frame header tells us.
struct Header {
    len: u32,
    samples: u32,
    sample_rate: u32,
}

#[cfg(feature = "transcode-ac3")]
/// Blocks per E-AC-3 syncframe, indexed by `numblkscod` (Table E1.2).
const EAC3_BLOCKS: [u32; 4] = [1, 2, 3, 6];
#[cfg(feature = "transcode-ac3")]
/// Samples per audio block — the 256-point half of the 512-point TDAC window.
const SAMPLES_PER_BLOCK: u32 = 256;
#[cfg(feature = "transcode-ac3")]
/// §E.2.3.1.4 `fscod2` rates, used only when `fscod == 3` (half-rate streams).
const EAC3_HALF_RATES: [u32; 3] = [24_000, 22_050, 16_000];
#[cfg(feature = "transcode-ac3")]
/// Base A/52 `fscod` rates (Table 5.6).
const AC3_RATES: [u32; 3] = [48_000, 44_100, 32_000];

/// A codec this build has no decoder for is also a codec it will not index:
/// an index exists to promise a `Content-Length` we can then deliver, and
/// promising one we cannot decode would be worse than declining up front.
#[cfg(not(feature = "transcode-ac3"))]
fn parse_ac3_family(_data: &[u8]) -> Result<Option<Header>> {
    bail!("this build of vuio-core was compiled without the `transcode-ac3` feature")
}

#[cfg(not(feature = "transcode-dts"))]
fn parse_dts(_data: &[u8]) -> Result<Option<Header>> {
    bail!("this build of vuio-core was compiled without the `transcode-dts` feature")
}

/// Sample frames one compressed frame decodes to, read from its own header.
///
/// The container path's equivalent of [`IndexedFrame::samples`]: symphonia hands
/// over a packet, and this is how its decoded length is known before it is
/// decoded — which is what lets a frame that fails to decode be replaced by
/// silence of exactly the right length instead of shortening the track.
pub(crate) fn frame_samples(codec: TranscodeCodec, data: &[u8]) -> Option<u32> {
    parse_header(codec, data).ok().flatten().map(|h| h.samples)
}

fn parse_header(codec: TranscodeCodec, data: &[u8]) -> Result<Option<Header>> {
    match codec {
        TranscodeCodec::Ac3 | TranscodeCodec::Eac3 => parse_ac3_family(data),
        TranscodeCodec::Dts => parse_dts(data),
    }
}

/// AC-3 and E-AC-3 share a syncword and are told apart by `bsid`, which both
/// syntaxes place at bit 40 — byte 5's top five bits. Base AC-3 gets there via
/// `crc1(16) fscod(2) frmsizecod(6)`, Annex E via
/// `strmtyp(2) substreamid(3) frmsiz(11) fscod(2) numblkscod(2) acmod(3) lfeon(1)`.
/// Both land on 40, so one probe serves both and a stream may even switch.
#[cfg(feature = "transcode-ac3")]
fn parse_ac3_family(data: &[u8]) -> Result<Option<Header>> {
    if data.len() < 6 {
        return Ok(None);
    }
    if data[0] != 0x0B || data[1] != 0x77 {
        return Ok(None);
    }
    let bsid = data[5] >> 3;

    if bsid <= oxideav_ac3::eac3::bsi::BSID_BASE_AC3_MAX {
        // Base AC-3: the vendored parser owns Table 5.18.
        let si = oxideav_ac3::syncinfo::parse(data)
            .map_err(|e| anyhow::anyhow!("ac3 syncinfo: {e}"))?;
        return Ok(Some(Header {
            len: si.frame_length,
            // Six blocks, always, in base AC-3 (§2.2).
            samples: 6 * SAMPLES_PER_BLOCK,
            sample_rate: si.sample_rate,
        }));
    }
    if bsid > 16 {
        return Ok(None);
    }

    // Annex E, Table E1.2.
    let frmsiz = (u32::from(data[2] & 0x07) << 8) | u32::from(data[3]);
    let len = (frmsiz + 1) * 2;
    let fscod = (data[4] >> 6) & 0x03;
    let next2 = (data[4] >> 4) & 0x03;
    let (sample_rate, blocks) = if fscod == 3 {
        // fscod2 replaces numblkscod, and the block count is implicitly six.
        let Some(rate) = EAC3_HALF_RATES.get(next2 as usize) else {
            return Ok(None);
        };
        (*rate, 6)
    } else {
        (AC3_RATES[fscod as usize], EAC3_BLOCKS[next2 as usize])
    };

    Ok(Some(Header {
        len,
        samples: blocks * SAMPLES_PER_BLOCK,
        sample_rate,
    }))
}

#[cfg(feature = "transcode-dts")]
fn parse_dts(data: &[u8]) -> Result<Option<Header>> {
    let hdr = match oxideav_dts::parse_frame_header(data) {
        Ok(h) => h,
        Err(_) => return Ok(None),
    };
    let Some(sample_rate) = hdr.sample_rate_hz() else {
        return Ok(None);
    };
    // `blocks_per_frame` carries the raw NBLKS field; the block count is
    // NBLKS + 1 and each block is 32 PCM samples (§5.3.1, and the same
    // arithmetic the vendored crate uses for its Rev2 subsubframe count).
    let blocks = u32::from(hdr.blocks_per_frame) + 1;
    Ok(Some(Header {
        len: u32::from(hdr.frame_size_bytes),
        samples: blocks * 32,
        sample_rate,
    }))
}

/// Offset of the next plausible syncword in `data`, if any.
#[allow(unused_variables)]
fn next_sync(codec: TranscodeCodec, data: &[u8]) -> Option<usize> {
    match codec {
        #[cfg(feature = "transcode-ac3")]
        TranscodeCodec::Ac3 | TranscodeCodec::Eac3 => {
            oxideav_ac3::syncinfo::find_syncword(data, 0)
        }
        #[cfg(feature = "transcode-dts")]
        TranscodeCodec::Dts => oxideav_dts::find_next_sync(data, 0).map(|m| m.offset),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "transcode-ac3")]
    const AC3_FIXTURE: &[u8] =
        include_bytes!("../../../../vendor/oxideav-ac3/tests/fixtures/sine440_stereo.ac3");
    #[cfg(feature = "transcode-dts")]
    const DTS_FIXTURE: &[u8] =
        include_bytes!("../../../../vendor/oxideav-dts/tests/fixtures/dts_5_frames.bin");

    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn indexes_every_frame_of_a_real_ac3_stream() {
        let idx = FrameIndex::build(TranscodeCodec::Ac3, &mut &AC3_FIXTURE[..]).unwrap();
        assert_eq!(idx.sample_rate, 48_000);
        // 48 kHz / 192 kbps → Table 5.18 frmsizecod 20 → 768-byte frames.
        assert!(idx.frames.len() >= 4, "got {} frames", idx.frames.len());
        for f in &idx.frames {
            assert_eq!(f.len, 768);
            assert_eq!(f.samples, 1536, "base AC-3 is always six 256-sample blocks");
        }
        // Offsets must be contiguous — no gaps, no overlap.
        for pair in idx.frames.windows(2) {
            assert_eq!(pair[0].offset + u64::from(pair[0].len), pair[1].offset);
        }
        assert_eq!(idx.total_samples, 1536 * idx.frames.len() as u64);
    }

    #[cfg(feature = "transcode-dts")]
    #[test]
    fn indexes_every_frame_of_a_real_dts_stream() {
        let idx = FrameIndex::build(TranscodeCodec::Dts, &mut &DTS_FIXTURE[..]).unwrap();
        assert_eq!(idx.frames.len(), 5, "the fixture is five frames");
        assert!(idx.sample_rate > 0);
        for pair in idx.frames.windows(2) {
            assert_eq!(pair[0].offset + u64::from(pair[0].len), pair[1].offset);
        }
        assert_eq!(
            idx.total_samples,
            idx.frames.iter().map(|f| u64::from(f.samples)).sum::<u64>()
        );
    }

    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn a_leading_junk_prefix_is_skipped_rather_than_failing() {
        // An .ac3 file can open with an ID3 tag. Anything before the first
        // syncword must be walked past, not treated as a parse failure.
        let mut stream = vec![0xFFu8; 300];
        stream.extend_from_slice(AC3_FIXTURE);
        let idx = FrameIndex::build(TranscodeCodec::Ac3, &mut &stream[..]).unwrap();
        assert_eq!(idx.frames[0].offset, 300);
        assert_eq!(idx.total_samples, 1536 * idx.frames.len() as u64);
    }

    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn locate_maps_a_sample_back_to_its_frame_and_offset() {
        let idx = FrameIndex::build(TranscodeCodec::Ac3, &mut &AC3_FIXTURE[..]).unwrap();
        assert_eq!(idx.locate(0), (0, 0));
        assert_eq!(idx.locate(1535), (0, 1535));
        assert_eq!(idx.locate(1536), (1, 0));
        assert_eq!(idx.locate(1536 + 7), (1, 7));
        // Past the end clamps to the last frame instead of erroring.
        let (i, _) = idx.locate(u64::MAX);
        assert_eq!(i, idx.frames.len() - 1);
    }

    #[test]
    fn a_stream_that_is_not_this_codec_is_rejected() {
        let junk = vec![0u8; 4096];
        assert!(FrameIndex::build(TranscodeCodec::Ac3, &mut &junk[..]).is_err());
    }
}

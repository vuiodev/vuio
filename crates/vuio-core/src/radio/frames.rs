//! Frame-level readers for the formats a station can broadcast.
//!
//! A radio stream is one unbroken sequence of bytes that a listener may join at
//! any moment, so it cannot carry a container: there is no header to have
//! missed, and no file boundary to resynchronise on. What it can carry is a run
//! of self-describing frames. Every MPEG audio frame and every ADTS AAC frame
//! states its own bitrate, sample rate and channel count in its first four
//! bytes, which is why a decoder that joins mid-stream finds its footing within
//! one frame, and why two tracks can be spliced together with nothing between
//! them.
//!
//! So nothing here decodes. Frames are lifted out of the source file and handed
//! on unchanged, paired with the wall-clock time they represent — which is what
//! [`super::engine`] paces the stream by. Three sources produce them:
//!
//! - `.mp3` — MPEG-1/2/2.5 Layer I/II/III, walked header by header.
//! - `.aac`, `.adts` — already a run of ADTS frames; passed straight through.
//! - `.m4a`, `.mp4`, `.m4b` — AAC access units inside an MP4 container, which
//!   have had their framing stripped by the muxer. Symphonia demuxes them
//!   (without decoding) and each one is re-wrapped in the ADTS header it needs
//!   to stand alone. Anything else inside an MP4 — ALAC most often — is not AAC
//!   and cannot be spliced into an AAC stream, so it is refused by name.
//!
//! Everything else a library holds (FLAC, WAV, Opus, Vorbis) would have to be
//! re-encoded to join a stream, and VuIO ships no encoder. Those files are
//! skipped when a station's queue is built, and counted so the studio can say
//! so.

use anyhow::{bail, Context, Result};
use bytes::{Bytes, BytesMut};
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncReadExt;

/// How much of the source file is held in memory while frames are cut from it.
const READ_CHUNK: usize = 64 * 1024;

/// The codec family a station broadcasts in.
///
/// A station has exactly one. MPEG and AAC frames cannot share a stream — the
/// `Content-Type` names one of them and a decoder is entitled to believe it —
/// so a queue that mixes formats broadcasts the majority family and skips the
/// rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    Mp3,
    Aac,
}

impl Codec {
    /// What the stream response says it is.
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Mp3 => "audio/mpeg",
            Self::Aac => "audio/aac",
        }
    }

    /// The suffix for players that will only open a URL that looks like a file.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Aac => "aac",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Aac => "aac",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "mp3" => Some(Self::Mp3),
            "aac" => Some(Self::Aac),
            _ => None,
        }
    }
}

/// The codec a file can be broadcast as, or `None` if it cannot be broadcast.
///
/// Decided by extension rather than by opening the file: a station's queue is
/// built from thousands of database rows at once, and the answer only has to be
/// good enough to choose candidates. A file whose contents turn out not to
/// match is dropped when it is opened.
pub fn codec_for_path(path: &Path) -> Option<Codec> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase();
    match extension.as_str() {
        "mp3" | "mp2" => Some(Codec::Mp3),
        "aac" | "adts" => Some(Codec::Aac),
        #[cfg(feature = "metadata")]
        "m4a" | "m4b" | "mp4" => Some(Codec::Aac),
        _ => None,
    }
}

/// One frame, and the time it occupies when played.
#[derive(Debug, Clone)]
pub struct Frame {
    pub bytes: Bytes,
    pub duration: Duration,
}

/// Which framing a byte stream uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    Mpeg,
    Adts,
}

/// A track being read out frame by frame.
pub enum TrackReader {
    /// A file that is already a run of frames, cut from a rolling buffer.
    Framed {
        file: tokio::fs::File,
        buffer: BytesMut,
        framing: Framing,
        eof: bool,
        /// MPEG only: the first frame may be a Xing/Info header, which encodes
        /// no audio and must not be broadcast.
        first_frame: bool,
    },
    /// A container that had to be demuxed up front.
    Prepared {
        frames: std::vec::IntoIter<Frame>,
    },
}

impl TrackReader {
    /// Open a track for broadcast, refusing anything that would not splice into
    /// a `codec` stream.
    pub async fn open(path: &Path, codec: Codec) -> Result<Self> {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        match (codec, extension.as_str()) {
            (Codec::Mp3, "mp3" | "mp2") => Self::open_framed(path, Framing::Mpeg).await,
            (Codec::Aac, "aac" | "adts") => Self::open_framed(path, Framing::Adts).await,
            #[cfg(feature = "metadata")]
            (Codec::Aac, "m4a" | "m4b" | "mp4") => {
                let owned = path.to_path_buf();
                let frames =
                    tokio::task::spawn_blocking(move || demux_mp4_aac(&owned)).await??;
                Ok(Self::Prepared {
                    frames: frames.into_iter(),
                })
            }
            _ => bail!(
                "{} cannot be broadcast on a {} station",
                path.display(),
                codec.as_str()
            ),
        }
    }

    async fn open_framed(path: &Path, framing: Framing) -> Result<Self> {
        let mut file = tokio::fs::File::open(path)
            .await
            .with_context(|| format!("opening {} for broadcast", path.display()))?;

        // Resynchronisation could walk past an ID3v2 tag on its own, but a tag
        // holding cover art is a few hundred kilobytes of JPEG, and a JPEG is
        // long enough to contain a byte pair that looks exactly like a frame
        // header. Reading the declared length is both cheaper and certain.
        let mut header = [0u8; 10];
        if file.read_exact(&mut header).await.is_ok() {
            if let Some(len) = id3v2_len(&header) {
                use tokio::io::AsyncSeekExt;
                file.seek(std::io::SeekFrom::Start(len)).await?;
            } else {
                use tokio::io::AsyncSeekExt;
                file.seek(std::io::SeekFrom::Start(0)).await?;
            }
        }

        Ok(Self::Framed {
            file,
            buffer: BytesMut::with_capacity(READ_CHUNK * 2),
            framing,
            eof: false,
            first_frame: true,
        })
    }

    /// The next frame, or `None` at the end of the track.
    pub async fn next_frame(&mut self) -> Result<Option<Frame>> {
        match self {
            Self::Prepared { frames } => Ok(frames.next()),
            Self::Framed {
                file,
                buffer,
                framing,
                eof,
                first_frame,
            } => {
                loop {
                    match scan_frame(buffer, *framing) {
                        Scan::Frame { len, duration } => {
                            let bytes = buffer.split_to(len).freeze();
                            if *first_frame {
                                *first_frame = false;
                                // A Xing/Info/VBRI frame is a real MPEG frame
                                // carrying VBR seek tables instead of audio.
                                // Broadcasting it would emit a moment of
                                // silence and hand listeners a table that
                                // describes a file they are not receiving.
                                if *framing == Framing::Mpeg && is_vbr_header_frame(&bytes) {
                                    continue;
                                }
                            }
                            return Ok(Some(Frame { bytes, duration }));
                        }
                        Scan::Incomplete => {
                            if *eof {
                                // A truncated final frame is not worth
                                // reporting: the track simply ends here.
                                return Ok(None);
                            }
                            if !fill(file, buffer, eof).await? {
                                continue;
                            }
                        }
                        Scan::Invalid => {
                            // Tags, padding and junk sit between frames in real
                            // files. Walk forward to the next plausible sync
                            // word rather than giving up on the track.
                            match next_sync(buffer, *framing) {
                                Some(offset) => {
                                    let _ = buffer.split_to(offset);
                                }
                                None => {
                                    if *eof {
                                        return Ok(None);
                                    }
                                    // Keep the last few bytes: a sync word may
                                    // straddle the boundary of what has been read.
                                    let keep = buffer.len().saturating_sub(3);
                                    let _ = buffer.split_to(keep);
                                    if !fill(file, buffer, eof).await? {
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Read more of the file. Returns whether anything was added.
async fn fill(file: &mut tokio::fs::File, buffer: &mut BytesMut, eof: &mut bool) -> Result<bool> {
    let before = buffer.len();
    buffer.reserve(READ_CHUNK);
    let read = file.read_buf(buffer).await?;
    if read == 0 {
        *eof = true;
    }
    Ok(buffer.len() > before)
}

/// What sits at the front of the buffer.
#[derive(Debug, PartialEq)]
enum Scan {
    Frame { len: usize, duration: Duration },
    /// A valid header, but the frame runs past what has been read.
    Incomplete,
    /// Not a frame.
    Invalid,
}

fn scan_frame(buffer: &[u8], framing: Framing) -> Scan {
    match framing {
        Framing::Mpeg => scan_mpeg_frame(buffer),
        Framing::Adts => scan_adts_frame(buffer),
    }
}

/// The offset of the next byte worth scanning from, if there is one.
fn next_sync(buffer: &[u8], framing: Framing) -> Option<usize> {
    for offset in 1..buffer.len() {
        if buffer[offset] != 0xFF {
            continue;
        }
        match scan_frame(&buffer[offset..], framing) {
            Scan::Invalid => continue,
            _ => return Some(offset),
        }
    }
    None
}

// ---------------------------------------------------------------------------
// MPEG audio (Layer I/II/III)
// ---------------------------------------------------------------------------

/// Bitrates in kbit/s, indexed by [version group][layer][bitrate index].
const MPEG1_BITRATES: [[u16; 16]; 3] = [
    // Layer I
    [
        0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448, 0,
    ],
    // Layer II
    [
        0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 0,
    ],
    // Layer III
    [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
    ],
];

const MPEG2_BITRATES: [[u16; 16]; 3] = [
    // Layer I
    [
        0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256, 0,
    ],
    // Layer II and III share a table
    [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0],
    [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0],
];

const SAMPLE_RATES: [[u32; 3]; 3] = [
    [44100, 48000, 32000], // MPEG-1
    [22050, 24000, 16000], // MPEG-2
    [11025, 12000, 8000],  // MPEG-2.5
];

fn scan_mpeg_frame(buffer: &[u8]) -> Scan {
    if buffer.len() < 4 {
        return if buffer.first() == Some(&0xFF) || buffer.is_empty() {
            Scan::Incomplete
        } else {
            Scan::Invalid
        };
    }
    if buffer[0] != 0xFF || buffer[1] & 0xE0 != 0xE0 {
        return Scan::Invalid;
    }

    // 00 = MPEG-2.5, 01 = reserved, 10 = MPEG-2, 11 = MPEG-1
    let version = (buffer[1] >> 3) & 0b11;
    // 00 = reserved, 01 = Layer III, 10 = Layer II, 11 = Layer I
    let layer = (buffer[1] >> 1) & 0b11;
    if version == 0b01 || layer == 0b00 {
        return Scan::Invalid;
    }

    let bitrate_index = usize::from(buffer[2] >> 4);
    let sample_rate_index = usize::from((buffer[2] >> 2) & 0b11);
    let padding = usize::from((buffer[2] >> 1) & 0b1);
    if bitrate_index == 0 || bitrate_index == 0b1111 || sample_rate_index == 0b11 {
        // A free-format or reserved frame has no length this side of decoding it.
        return Scan::Invalid;
    }

    // Table rows run Layer I, II, III; the field counts down from Layer I = 3.
    let layer_row = 3 - usize::from(layer);
    let rate_row = match version {
        0b11 => 0,
        0b10 => 1,
        _ => 2,
    };
    let bitrate = if version == 0b11 {
        MPEG1_BITRATES[layer_row][bitrate_index]
    } else {
        MPEG2_BITRATES[layer_row][bitrate_index]
    };
    if bitrate == 0 {
        return Scan::Invalid;
    }
    let sample_rate = SAMPLE_RATES[rate_row][sample_rate_index];
    let bits_per_second = u32::from(bitrate) * 1000;

    // Layer I is measured in 4-byte slots; the others in bytes.
    let (samples, len) = if layer == 0b11 {
        let len = (12 * bits_per_second as usize / sample_rate as usize + padding) * 4;
        (384u32, len)
    } else {
        let samples = if layer == 0b01 && version != 0b11 {
            // Layer III at MPEG-2/2.5 rates carries half a granule pair.
            576u32
        } else {
            1152u32
        };
        let len = (samples as usize / 8) * bits_per_second as usize / sample_rate as usize + padding;
        (samples, len)
    };

    if len < 4 {
        return Scan::Invalid;
    }
    if buffer.len() < len {
        return Scan::Incomplete;
    }
    Scan::Frame {
        len,
        duration: Duration::from_secs_f64(f64::from(samples) / f64::from(sample_rate)),
    }
}

/// The total length of an ID3v2 tag whose first ten bytes are `header`.
///
/// The size is stored "syncsafe": seven bits per byte, so that no byte of it
/// can be mistaken for a frame sync.
fn id3v2_len(header: &[u8; 10]) -> Option<u64> {
    if &header[..3] != b"ID3" || header[3] == 0xFF || header[4] == 0xFF {
        return None;
    }
    if header[6..10].iter().any(|byte| byte & 0x80 != 0) {
        return None;
    }
    let size = header[6..10]
        .iter()
        .fold(0u64, |total, byte| (total << 7) | u64::from(*byte));
    // A footer is present when the tag flags say so, and adds ten more bytes.
    let footer = if header[5] & 0x10 != 0 { 10 } else { 0 };
    Some(10 + size + footer)
}

/// Whether an MPEG frame is a Xing, Info or VBRI header rather than audio.
fn is_vbr_header_frame(frame: &[u8]) -> bool {
    let window = &frame[..frame.len().min(64)];
    window
        .windows(4)
        .any(|tag| tag == b"Xing" || tag == b"Info" || tag == b"VBRI")
}

// ---------------------------------------------------------------------------
// ADTS AAC
// ---------------------------------------------------------------------------

const AAC_SAMPLE_RATES: [u32; 13] = [
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
];

fn scan_adts_frame(buffer: &[u8]) -> Scan {
    if buffer.len() < 7 {
        return if buffer.is_empty() || buffer[0] == 0xFF {
            Scan::Incomplete
        } else {
            Scan::Invalid
        };
    }
    // Sync is 12 bits, then a 1-bit MPEG version and a 2-bit layer that is
    // always zero for ADTS.
    if buffer[0] != 0xFF || buffer[1] & 0xF0 != 0xF0 || buffer[1] & 0b0110 != 0 {
        return Scan::Invalid;
    }

    let sample_rate_index = usize::from((buffer[2] >> 2) & 0b1111);
    if sample_rate_index >= AAC_SAMPLE_RATES.len() {
        return Scan::Invalid;
    }
    let sample_rate = AAC_SAMPLE_RATES[sample_rate_index];

    let len = (usize::from(buffer[3] & 0b11) << 11)
        | (usize::from(buffer[4]) << 3)
        | (usize::from(buffer[5]) >> 5);
    let blocks = u32::from(buffer[6] & 0b11) + 1;
    if len < 7 {
        return Scan::Invalid;
    }
    if buffer.len() < len {
        return Scan::Incomplete;
    }
    Scan::Frame {
        len,
        duration: Duration::from_secs_f64(f64::from(1024 * blocks) / f64::from(sample_rate)),
    }
}

/// What an ADTS header has to state about a stream, recovered from the
/// AudioSpecificConfig an MP4 keeps in its `esds` box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AdtsConfig {
    /// The ADTS `profile` field: the MPEG-4 object type minus one.
    profile: u8,
    sample_rate_index: u8,
    channel_config: u8,
    sample_rate: u32,
}

impl AdtsConfig {
    /// Parse an AudioSpecificConfig.
    ///
    /// For High-Efficiency AAC the config nests: object type 5 (SBR) or 29 (PS)
    /// is followed by the extension sample rate and then the real object type of
    /// the core stream. ADTS has no field for any of that, so — as every other
    /// ADTS muxer does — the header states the core object type and the *first*
    /// sample rate, and the decoder infers SBR from the payload.
    pub(crate) fn from_asc(asc: &[u8]) -> Option<Self> {
        let mut bits = BitReader::new(asc);
        let mut object_type = read_object_type(&mut bits)?;
        let (sample_rate_index, sample_rate) = read_sample_rate(&mut bits)?;
        let channel_config = bits.read(4)? as u8;

        if object_type == 5 || object_type == 29 {
            let _extension_rate = read_sample_rate(&mut bits)?;
            object_type = read_object_type(&mut bits)?;
        }

        // ADTS can only name the four object types that fit in two bits.
        let profile = match object_type {
            1..=4 => (object_type - 1) as u8,
            _ => 1, // Low Complexity, the only sane thing to claim.
        };
        if channel_config == 0 {
            // The channel layout lives in the payload instead; ADTS cannot say
            // so, and a decoder handed a zero here has nothing to work with.
            return None;
        }
        Some(Self {
            profile,
            sample_rate_index,
            channel_config,
            sample_rate,
        })
    }

    /// Build a config from what the container reported, for the rare file whose
    /// `esds` carries no AudioSpecificConfig.
    pub(crate) fn from_parameters(sample_rate: u32, channels: u8) -> Option<Self> {
        let sample_rate_index = AAC_SAMPLE_RATES
            .iter()
            .position(|rate| *rate == sample_rate)? as u8;
        if channels == 0 || channels > 7 {
            return None;
        }
        Some(Self {
            profile: 1,
            sample_rate_index,
            channel_config: channels,
            sample_rate,
        })
    }

    pub(crate) fn sample_rate(self) -> u32 {
        self.sample_rate
    }

    /// The 7-byte header that lets one access unit stand on its own.
    pub(crate) fn header(self, payload_len: usize) -> [u8; 7] {
        let len = (payload_len + 7).min(0x1FFF) as u32;
        [
            0xFF,
            // MPEG-4, layer 0, no CRC — so the header is 7 bytes, not 9.
            0xF1,
            (self.profile << 6) | (self.sample_rate_index << 2) | (self.channel_config >> 2),
            ((self.channel_config & 0b11) << 6) | ((len >> 11) & 0b11) as u8,
            ((len >> 3) & 0xFF) as u8,
            (((len & 0b111) << 5) as u8) | 0b11111,
            // Variable bit rate, one raw data block per frame.
            0b1111_1100,
        ]
    }
}

fn read_object_type(bits: &mut BitReader<'_>) -> Option<u32> {
    let value = bits.read(5)?;
    if value == 31 {
        Some(32 + bits.read(6)?)
    } else {
        Some(value)
    }
}

fn read_sample_rate(bits: &mut BitReader<'_>) -> Option<(u8, u32)> {
    let index = bits.read(4)?;
    if index == 0b1111 {
        let explicit = bits.read(24)?;
        let index = AAC_SAMPLE_RATES
            .iter()
            .position(|rate| *rate == explicit)
            .unwrap_or(4) as u8;
        Some((index, explicit))
    } else {
        let index = index as usize;
        if index >= AAC_SAMPLE_RATES.len() {
            return None;
        }
        Some((index as u8, AAC_SAMPLE_RATES[index]))
    }
}

struct BitReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn read(&mut self, count: u32) -> Option<u32> {
        let mut value = 0u32;
        for _ in 0..count {
            let byte = self.bytes.get(self.position / 8)?;
            let bit = (byte >> (7 - (self.position % 8))) & 1;
            value = (value << 1) | u32::from(bit);
            self.position += 1;
        }
        Some(value)
    }
}

// ---------------------------------------------------------------------------
// AAC inside MP4
// ---------------------------------------------------------------------------

/// Demux an MP4 into the ADTS frames an AAC stream is made of.
///
/// Blocking, and done in one pass when the track is opened rather than
/// incrementally: symphonia's reader is synchronous, and one track's worth of
/// frames is the same order of memory as the file itself.
#[cfg(feature = "metadata")]
fn demux_mp4_aac(path: &Path) -> Result<Vec<Frame>> {
    use symphonia::core::codecs::audio::well_known::CODEC_ID_AAC;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;

    let file = std::fs::File::open(path)
        .with_context(|| format!("opening {} for broadcast", path.display()))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .with_context(|| format!("{} is not a readable container", path.display()))?;

    let track = format
        .default_track(TrackType::Audio)
        .with_context(|| format!("{} has no audio track", path.display()))?;
    let track_id = track.id;
    let time_base = track.time_base;
    let audio = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .with_context(|| format!("{} has no audio codec parameters", path.display()))?;

    if audio.codec != CODEC_ID_AAC {
        // ALAC is the common case here, and re-encoding is not on the table.
        bail!(
            "{} holds {:?} rather than AAC, which cannot join an AAC stream",
            path.display(),
            audio.codec
        );
    }

    let config = audio
        .extra_data
        .as_deref()
        .and_then(AdtsConfig::from_asc)
        .or_else(|| {
            AdtsConfig::from_parameters(
                audio.sample_rate.unwrap_or_default(),
                audio.channels.as_ref().map_or(0, |c| c.count() as u8),
            )
        })
        .with_context(|| {
            format!(
                "{} does not describe its AAC stream well enough to re-frame",
                path.display()
            )
        })?;

    let fallback = Duration::from_secs_f64(1024.0 / f64::from(config.sample_rate()));
    let mut frames = Vec::new();
    while let Some(packet) = format.next_packet()? {
        if packet.track_id != track_id || packet.data.is_empty() {
            continue;
        }
        // The container's own timing is more trustworthy than anything derived
        // from the config, which for HE-AAC does not say which rate it means.
        let duration = time_base
            .and_then(|base| base.calc_duration(packet.dur))
            .map(|time| Duration::from_secs_f64(time.as_secs_f64().max(0.0)))
            .filter(|value| !value.is_zero())
            .unwrap_or(fallback);

        let mut bytes = BytesMut::with_capacity(7 + packet.data.len());
        bytes.extend_from_slice(&config.header(packet.data.len()));
        bytes.extend_from_slice(&packet.data);
        frames.push(Frame {
            bytes: bytes.freeze(),
            duration,
        });
    }

    if frames.is_empty() {
        bail!("{} yielded no AAC frames", path.display());
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 128 kbit/s, 44.1 kHz MPEG-1 Layer III frame header.
    fn mpeg1_layer3_header() -> [u8; 4] {
        [0xFF, 0xFB, 0x90, 0x00]
    }

    #[test]
    fn mpeg1_layer3_frame_length_and_duration() {
        let mut frame = vec![0u8; 417];
        frame[..4].copy_from_slice(&mpeg1_layer3_header());
        // 1152 samples / 8 * 128000 / 44100 = 417 bytes, no padding.
        let Scan::Frame { len, duration } = scan_mpeg_frame(&frame) else {
            panic!("expected a frame");
        };
        assert_eq!(len, 417);
        assert!((duration.as_secs_f64() - 1152.0 / 44100.0).abs() < 1e-9);
    }

    #[test]
    fn mpeg_padding_bit_adds_one_byte() {
        let mut frame = vec![0u8; 418];
        frame[..4].copy_from_slice(&[0xFF, 0xFB, 0x92, 0x00]);
        let Scan::Frame { len, .. } = scan_mpeg_frame(&frame) else {
            panic!("expected a frame");
        };
        assert_eq!(len, 418);
    }

    #[test]
    fn mpeg2_layer3_carries_half_as_many_samples() {
        // MPEG-2, Layer III, 64 kbit/s, 22.05 kHz.
        let mut frame = vec![0u8; 1024];
        frame[..4].copy_from_slice(&[0xFF, 0xF3, 0x80, 0x00]);
        let Scan::Frame { len, duration } = scan_mpeg_frame(&frame) else {
            panic!("expected a frame");
        };
        assert_eq!(len, 576 / 8 * 64000 / 22050);
        assert!((duration.as_secs_f64() - 576.0 / 22050.0).abs() < 1e-9);
    }

    #[test]
    fn mpeg_rejects_reserved_and_free_format_headers() {
        // Reserved version.
        assert_eq!(scan_mpeg_frame(&[0xFF, 0xEB, 0x90, 0x00]), Scan::Invalid);
        // Reserved layer.
        assert_eq!(scan_mpeg_frame(&[0xFF, 0xF9, 0x90, 0x00]), Scan::Invalid);
        // Free-format bitrate.
        assert_eq!(scan_mpeg_frame(&[0xFF, 0xFB, 0x00, 0x00]), Scan::Invalid);
        // Reserved sample rate.
        assert_eq!(scan_mpeg_frame(&[0xFF, 0xFB, 0x9C, 0x00]), Scan::Invalid);
        // Not a sync word at all.
        assert_eq!(scan_mpeg_frame(&[0x49, 0x44, 0x33, 0x04]), Scan::Invalid);
    }

    #[test]
    fn a_header_without_its_frame_is_incomplete() {
        let mut frame = vec![0u8; 100];
        frame[..4].copy_from_slice(&mpeg1_layer3_header());
        assert_eq!(scan_mpeg_frame(&frame), Scan::Incomplete);
    }

    #[test]
    fn resync_walks_past_junk_to_the_next_frame() {
        let mut buffer = vec![0x00, 0xFF, 0x13, 0x37];
        let mut frame = vec![0u8; 417];
        frame[..4].copy_from_slice(&mpeg1_layer3_header());
        buffer.extend_from_slice(&frame);
        assert_eq!(next_sync(&buffer, Framing::Mpeg), Some(4));
    }

    #[test]
    fn xing_frames_are_recognised() {
        let mut frame = vec![0u8; 417];
        frame[..4].copy_from_slice(&mpeg1_layer3_header());
        frame[36..40].copy_from_slice(b"Xing");
        assert!(is_vbr_header_frame(&frame));

        let mut audio = vec![0x11u8; 417];
        audio[..4].copy_from_slice(&mpeg1_layer3_header());
        assert!(!is_vbr_header_frame(&audio));
    }

    #[test]
    fn id3v2_length_is_read_syncsafe() {
        // A 1 KiB tag: 0x08 << 7 = 1024.
        let mut header = [0u8; 10];
        header[..3].copy_from_slice(b"ID3");
        header[3] = 4;
        header[8] = 0x08;
        assert_eq!(id3v2_len(&header), Some(10 + 1024));

        // With a footer.
        header[5] = 0x10;
        assert_eq!(id3v2_len(&header), Some(10 + 1024 + 10));

        // A high bit anywhere in the size means this is not a syncsafe integer.
        header[5] = 0;
        header[7] = 0x80;
        assert_eq!(id3v2_len(&header), None);
    }

    #[test]
    fn a_frame_header_is_not_mistaken_for_a_tag() {
        let mut header = [0u8; 10];
        header[..4].copy_from_slice(&mpeg1_layer3_header());
        assert_eq!(id3v2_len(&header), None);
    }

    #[test]
    fn adts_frame_length_comes_from_the_header() {
        // 44.1 kHz, stereo, 512-byte frame.
        let mut frame = vec![0u8; 512];
        frame[..7].copy_from_slice(&[0xFF, 0xF1, 0x50, 0x80, 0x40, 0x1F, 0xFC]);
        let Scan::Frame { len, duration } = scan_adts_frame(&frame) else {
            panic!("expected a frame");
        };
        assert_eq!(len, 512);
        assert!((duration.as_secs_f64() - 1024.0 / 44100.0).abs() < 1e-9);
    }

    #[test]
    fn adts_rejects_a_non_zero_layer() {
        assert_eq!(
            scan_adts_frame(&[0xFF, 0xF3, 0x50, 0x80, 0x40, 0x1F, 0xFC]),
            Scan::Invalid
        );
    }

    /// AAC-LC, 44.1 kHz, stereo: object type 2, rate index 4, channels 2.
    #[test]
    fn asc_for_plain_low_complexity() {
        let config = AdtsConfig::from_asc(&[0x12, 0x10]).expect("a valid config");
        assert_eq!(config.profile, 1);
        assert_eq!(config.sample_rate_index, 4);
        assert_eq!(config.channel_config, 2);
        assert_eq!(config.sample_rate(), 44100);
    }

    /// HE-AAC: object type 5, core rate 22.05 kHz, stereo, extension rate
    /// 44.1 kHz, then the real object type 2. The header must state the core
    /// rate and Low Complexity.
    #[test]
    fn asc_for_high_efficiency_states_the_core_stream() {
        let config = AdtsConfig::from_asc(&[0x2B, 0x92, 0x08, 0x00]).expect("a valid config");
        assert_eq!(config.profile, 1, "must claim Low Complexity");
        assert_eq!(config.sample_rate_index, 7, "must keep the core rate");
        assert_eq!(config.sample_rate(), 22050);
        assert_eq!(config.channel_config, 2);
    }

    #[test]
    fn a_synthesised_header_parses_back_as_the_frame_it_describes() {
        let config = AdtsConfig::from_asc(&[0x12, 0x10]).expect("a valid config");
        let payload = vec![0xAAu8; 380];
        let mut frame = config.header(payload.len()).to_vec();
        frame.extend_from_slice(&payload);

        let Scan::Frame { len, duration } = scan_adts_frame(&frame) else {
            panic!("the header we wrote must parse as a frame");
        };
        assert_eq!(len, 387);
        assert!((duration.as_secs_f64() - 1024.0 / 44100.0).abs() < 1e-9);
    }

    #[test]
    fn channel_configurations_survive_the_round_trip() {
        for channels in 1..=6u8 {
            let config =
                AdtsConfig::from_parameters(48000, channels).expect("a supported configuration");
            let header = config.header(100);
            let recovered = ((header[2] & 0b1) << 2) | (header[3] >> 6);
            assert_eq!(recovered, channels, "channel config for {channels}");
        }
    }

    #[test]
    fn extensions_map_to_the_codec_they_can_be_broadcast_as() {
        assert_eq!(codec_for_path(Path::new("a/b.mp3")), Some(Codec::Mp3));
        assert_eq!(codec_for_path(Path::new("a/b.MP3")), Some(Codec::Mp3));
        assert_eq!(codec_for_path(Path::new("a/b.aac")), Some(Codec::Aac));
        assert_eq!(codec_for_path(Path::new("a/b.flac")), None);
        assert_eq!(codec_for_path(Path::new("a/b.wav")), None);
        assert_eq!(codec_for_path(Path::new("a/b")), None);
    }

    #[cfg(feature = "metadata")]
    #[test]
    fn mp4_audio_is_broadcastable() {
        assert_eq!(codec_for_path(Path::new("a/b.m4a")), Some(Codec::Aac));
    }
}

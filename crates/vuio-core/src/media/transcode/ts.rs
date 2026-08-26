//! A film remuxed into a transport stream, for a television that seeks by byte.
//!
//! The same work as [`super::ProgressiveStream`] — picture copied through, DTS
//! decoded and re-encoded, Dolby passed through — poured into a different
//! container, and the container is the whole point. A fragmented MP4 has one
//! header at the front describing everything that follows, so a renderer that
//! jumps to a byte offset lands in the middle of a structure it has no way to
//! interpret. A transport stream has no front: the tables are repeated
//! throughout, every packet begins with a sync byte, and the parameter sets a
//! decoder needs travel beside each keyframe. Land anywhere and it recovers.
//!
//! That is what makes a scrub bar work on hardware that scrubs by byte, which is
//! most televisions. See [`crate::media::remux::ts_writer`] for the packet layer.
//!
//! ## Why this batches
//!
//! Matroska stores presentation timestamps in decode order and no decode
//! timestamps at all; a transport stream needs both, because a decoder has to be
//! told when to decode a frame it will not display yet. Recovering the decode
//! timeline needs a run of frames to sort, not a single one — so packets are
//! gathered a second or so at a time, the timeline is worked out over the batch,
//! and the batch is then written out interleaved by decode time. Which is also
//! what a transport stream wants: audio and video arriving together, in the
//! order a decoder will want them, rather than in track-sized runs.

use anyhow::Result;
use std::path::Path;

use super::{AacEncoder, PcmDecoder, TranscodeCodec};
use crate::media::remux::{
    derive_decode_timestamps, packet_is_keyframe, rescale_ticks, to_annexb, ParameterSets,
    PesTiming, TrackCodec, TrackInfo, TsMuxer, TsStreamSpec, FIRST_ES_PID, TS_CLOCK_HZ,
    TS_PACKET_LEN,
};

/// Seconds of film gathered before a batch is written.
///
/// Long enough that the decode timeline can be recovered across any reordering a
/// real encoder produces, short enough that a renderer starts promptly and a
/// dropped connection wastes little.
///
/// It is the floor on how long a renderer waits for its first byte, and that
/// turns out to matter far more than it looks. A television seeking a transport
/// stream binary-searches it: twenty-odd ranged requests, each read only far
/// enough to find one clock value, each converging on the instant the viewer
/// dragged to. Every one of them pays this. At a second and a half a probe the
/// search takes half a minute and the set gives up; at a fifth of that it is a
/// seek.
const BATCH_SECS: f64 = 0.4;

/// Channels the re-encoded audio track carries.
const DECODED_CHANNELS: u16 = 2;

/// Packets to hear from before placing a re-encoded run on the timeline.
/// See [`super::run_anchor`] for what the spread is and why the first
/// timestamp alone will not do.
const ANCHOR_PACKETS: usize = 96;

/// How far the presentation clock runs ahead of the programme clock.
///
/// A decoder starts its own clock from the PCR it is given and shows each
/// picture when that clock reaches the picture's PTS. Write the two equal and
/// every frame is due the instant it arrives, so the decoder has no buffer at
/// all and the first jitter starves it — which on a television is a film that
/// stutters, or one that never starts. Every muxer in the field leaves a gap
/// here; this is ffmpeg's, near enough, and it is what the renderer spends
/// filling its buffer before the first frame is due.
const CLOCK_HEADROOM: u64 = TS_CLOCK_HZ / 2;

/// How often the programme tables are repeated, in output clock ticks.
///
/// A decoder that joins the stream anywhere can interpret nothing until it has
/// seen a PAT and a PMT, so the wait for the next pair is the floor on how long
/// a seek takes to produce a picture. A tenth of a second is what the broadcast
/// profiles require and costs two packets to honour.
const TABLE_INTERVAL: u64 = TS_CLOCK_HZ / 10;

/// Packets to read while looking for the first frame of every soundtrack.
///
/// Bounded twice over, because the two things that go wrong are different: a
/// film whose tracks are interleaved normally shows all of them within a
/// fraction of a second, and one that never shows a track at all should cost
/// something small and bounded to give up on. The byte bound is the one that
/// matters on a film with a large picture, where a thousand packets could be a
/// hundred megabytes.
const PRIME_PACKETS: usize = 1024;
const PRIME_BYTES: usize = 8 * 1024 * 1024;

/// A packet read during priming, waiting to be replayed.
type PrimedPacket = (u32, i64, Vec<u8>);

/// What a soundtrack that cannot be passed through becomes.
///
/// Only DTS reaches here — Dolby and AAC ride as they are, see
/// [`audio_disposition`] — so in practice this is the answer to "a DTS film,
/// and a television that cannot decode DTS: then what".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SoundtrackFormat {
    /// AC-3, keeping the film's channel layout.
    ///
    /// The default. It is what a television was built to decode, and it is the
    /// only one of the two that keeps the surround: DTS 5.1 becomes AC-3 5.1 at
    /// 640 kbps rather than being folded down to a stereo pair.
    #[default]
    Ac3,
    /// AAC-LC, folded down to stereo.
    ///
    /// What this path used to do unconditionally. Kept for a renderer that
    /// turns out to prefer it, and it is what a build without the AC-3 encoder
    /// falls back to.
    Aac,
}

impl From<crate::config::TranscodeAudioFormat> for SoundtrackFormat {
    /// LPCM is not among a soundtrack's options — it shares a transport stream
    /// with the picture — so `audio_format = "lpcm"`, which is an answer about
    /// standalone audio files, leaves a film on the default.
    fn from(value: crate::config::TranscodeAudioFormat) -> Self {
        use crate::config::TranscodeAudioFormat as F;
        match value.soundtrack() {
            F::Aac => Self::Aac,
            _ => Self::Ac3,
        }
    }
}

impl SoundtrackFormat {
    /// The format this build can actually produce.
    ///
    /// A server compiled without the AC-3 encoder falls back to AAC rather than
    /// dropping the soundtrack: stereo is better than silence.
    pub fn available(self) -> Self {
        #[cfg(not(feature = "transcode-ac3"))]
        {
            let _ = self;
            Self::Aac
        }
        #[cfg(feature = "transcode-ac3")]
        self
    }

    /// The codec a track re-encoded to this format is described as from here on.
    fn carried_as(self) -> TrackCodec {
        match self {
            Self::Ac3 => TrackCodec::Ac3,
            Self::Aac => TrackCodec::Aac,
        }
    }

    /// Channels a track re-encoded to this format carries.
    ///
    /// AC-3 keeps what the film declared, clamped to the six channels Table 5.8
    /// can lay out; a track that declares nothing is treated as stereo. AAC
    /// folds everything down, which is what it has always done.
    fn channels(self, source: Option<u8>) -> u16 {
        match self {
            Self::Ac3 => u16::from(source.unwrap_or(DECODED_CHANNELS as u8)).clamp(1, 6),
            Self::Aac => DECODED_CHANNELS,
        }
    }
}

/// Whichever encoder this build of the stream is re-encoding through.
enum Reencoder {
    Aac(Box<AacEncoder>),
    #[cfg(feature = "transcode-ac3")]
    Ac3(super::Ac3Encoder),
}

impl Reencoder {
    /// Samples one output frame covers, which is what the run advances by.
    fn frame_samples(&self) -> u64 {
        match self {
            Self::Aac(_) => super::AAC_FRAME_SAMPLES,
            #[cfg(feature = "transcode-ac3")]
            Self::Ac3(_) => super::AC3_FRAME_SAMPLES,
        }
    }

    /// How far the encoder's output trails its input.
    fn delay(&self) -> i64 {
        match self {
            Self::Aac(_) => super::ENCODER_DELAY as i64,
            // Measured against its own input rather than assumed, and this
            // encoder emits the frame it was given.
            #[cfg(feature = "transcode-ac3")]
            Self::Ac3(_) => 0,
        }
    }

    fn push(&mut self, pcm: &[u8]) -> Result<Vec<Vec<u8>>> {
        match self {
            Self::Aac(encoder) => Ok(adts_frames(&encoder.push(pcm)?)
                .into_iter()
                .map(<[u8]>::to_vec)
                .collect()),
            #[cfg(feature = "transcode-ac3")]
            Self::Ac3(encoder) => encoder.push(pcm),
        }
    }

    fn finish(&mut self) -> Vec<Vec<u8>> {
        match self {
            Self::Aac(encoder) => adts_frames(&encoder.finish())
                .into_iter()
                .map(<[u8]>::to_vec)
                .collect(),
            #[cfg(feature = "transcode-ac3")]
            Self::Ac3(encoder) => encoder.finish(),
        }
    }
}

/// What becomes of one soundtrack on its way into a transport stream.
///
/// Asked in two places that must agree — [`TsStream::open`], which carries the
/// track, and the handler that commits to a length before the muxer has
/// produced a byte — so it is answered once, here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioDisposition {
    /// Carried exactly as it arrived. A television decodes Dolby and AAC for
    /// itself, and copying costs nothing and keeps the 5.1 a stereo re-encode
    /// would throw away.
    Passthrough,
    /// Decoded and re-encoded as stereo AAC, which is the whole point of this
    /// path: a set with no DTS licence plays the picture and nothing else.
    Reencoded,
    /// Neither possible. Better a film with two working soundtracks than one
    /// with a third that plays noise.
    Dropped,
}

/// How `track` reaches the output, and what it is described as when it gets
/// there.
pub fn audio_disposition(track: &TrackInfo) -> AudioDisposition {
    match track.codec_kind {
        TrackCodec::Aac | TrackCodec::Ac3 | TrackCodec::Eac3 => AudioDisposition::Passthrough,
        other => match other.transcode_codec() {
            Some(codec) if codec.is_decodable() && cfg!(feature = "transcode-aac") => {
                AudioDisposition::Reencoded
            }
            _ => AudioDisposition::Dropped,
        },
    }
}

/// What one track costs the film it is in, and how often it costs it.
///
/// Measured from the file rather than assumed from the codec, because the
/// assumption is the thing that goes wrong. AC-3 runs anywhere from 192 to 640
/// kilobits, DTS from 754 to 1509 for its core and several times that with the
/// lossless extension on top — so a table keyed on codec and channel count is
/// out by a factor of three on real films, in whichever direction happens to be
/// wrong for the film in front of it.
///
/// The frame rate is here for a less obvious reason. A transport stream charges
/// by the packet, and the last packet of every frame is padded out to 188 bytes
/// whatever is left over — so what a track costs to carry depends on how large
/// its frames are, not only on how many bits a second they add up to. On 768-byte
/// AC-3 frames that padding is a fifth of the track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackRate {
    pub id: u32,
    /// Bits a second this track occupies in the source.
    pub bits_per_second: u64,
    /// Access units a second.
    pub frames_per_second: f64,
}

/// Every track of one film, measured.
///
/// What makes measuring cheap is that the soundtracks these films carry are all
/// constant bitrate, and frame rates do not change. Half a second of a film says
/// what two hours of it will cost.
#[derive(Debug, Default, Clone)]
pub struct TrackRates(Vec<TrackRate>);

impl TrackRates {
    /// What track `id` was measured at, if it was reached.
    pub fn get(&self, id: u32) -> Option<TrackRate> {
        self.0.iter().find(|rate| rate.id == id).copied()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Seconds of each track to hear before its rate is settled.
///
/// A DTS frame is eleven milliseconds and an AC-3 frame thirty-two, so half a
/// second is fifteen frames at worst — far more than a constant bitrate needs
/// to reveal itself, and short enough that the read stays inside the film's
/// first couple of megabytes.
const MEASURE_SECS: f64 = 0.5;
const MEASURE_FRAMES: usize = 8;
const MEASURE_PACKETS: usize = 4096;
const MEASURE_BYTES: usize = 12 * 1024 * 1024;

/// Measure what each of `tracks` costs the source.
///
/// Reads from the head of the film, which for constant-bitrate audio and a
/// fixed frame rate is representative of all of it, and gives up on whatever has
/// not appeared within a bounded read — a track measured as nothing falls back
/// to its codec's nominal shape at the call site.
pub fn measure_track_rates(path: &Path, tracks: &[TrackInfo]) -> Result<TrackRates> {
    /// One track's accumulating account of itself.
    #[derive(Default)]
    struct Meter {
        first: Option<i64>,
        last: i64,
        /// Bytes of the frames wholly inside `[first, last)`.
        bytes: u64,
        /// The most recent frame's, which has no span behind it yet.
        carry: u64,
        frames: usize,
    }

    if tracks.is_empty() {
        return Ok(TrackRates::default());
    }
    let mut format = super::source::open_format(path)?;
    let mut meters: std::collections::HashMap<u32, Meter> = tracks
        .iter()
        .map(|track| (track.id, Meter::default()))
        .collect();
    let bases: std::collections::HashMap<u32, Option<symphonia::core::units::TimeBase>> = tracks
        .iter()
        .map(|track| (track.id, track_time_base(format.as_ref(), track.id)))
        .collect();

    let span = |id: &u32, meter: &Meter| -> Option<f64> {
        let base = (*bases.get(id)?)?;
        let at = |ticks: i64| {
            base.calc_time(symphonia::core::units::Timestamp::new(ticks))
                .map(|time| time.as_secs_f64())
        };
        Some(at(meter.last)? - at(meter.first?)?)
    };

    let mut read = 0usize;
    for _ in 0..MEASURE_PACKETS {
        if read >= MEASURE_BYTES {
            break;
        }
        let Ok(Some(packet)) = format.next_packet() else {
            break;
        };
        let (id, pts, len) = (packet.track_id, packet.pts.get(), packet.data.len());
        read += len;
        let Some(meter) = meters.get_mut(&id) else {
            continue;
        };
        meter.bytes += meter.carry;
        meter.carry = len as u64;
        meter.first.get_or_insert(pts);
        meter.last = pts;
        meter.frames += 1;

        // Every track heard from for long enough: nothing further to learn.
        if meters.iter().all(|(id, meter)| {
            meter.frames >= MEASURE_FRAMES
                && span(id, meter).is_some_and(|span| span >= MEASURE_SECS)
        }) {
            break;
        }
    }

    let mut rates: Vec<TrackRate> = meters
        .iter()
        .filter_map(|(id, meter)| {
            let span = span(id, meter)?;
            // Too little of it heard to say anything, which is not an error: the
            // caller has a nominal shape to fall back on.
            if meter.frames < MEASURE_FRAMES || span <= 0.0 {
                return None;
            }
            Some(TrackRate {
                id: *id,
                bits_per_second: (meter.bytes as f64 * 8.0 / span) as u64,
                frames_per_second: (meter.frames - 1) as f64 / span,
            })
        })
        .collect();
    rates.sort_unstable_by_key(|rate| rate.id);
    Ok(TrackRates(rates))
}

/// What a track of this codec and shape usually looks like, for one
/// [`measure_track_rates`] could not reach.
///
/// Every bitrate here is at the low end of what the codec is used at, and that
/// is deliberate rather than sloppy. A soundtrack's rate is subtracted from the
/// file's own to leave the picture's, so guessing one *small* leaves the picture
/// looking large, which leaves the promised length long, which costs padding.
/// Guessing it large does the opposite and cuts off the end of the film.
fn nominal_rate(track: &TrackInfo) -> TrackRate {
    use crate::media::remux::TrackKind;

    let sample_rate = f64::from(track.sample_rate.unwrap_or(48_000));
    let surround = track.channels.unwrap_or(2) >= 6;
    let (bits, frames) = match track.codec_kind {
        TrackCodec::Ac3 => (if surround { 384_000 } else { 192_000 }, sample_rate / 1536.0),
        TrackCodec::Eac3 => (if surround { 384_000 } else { 128_000 }, sample_rate / 1536.0),
        TrackCodec::Dts => (if surround { 768_000 } else { 384_000 }, sample_rate / 512.0),
        TrackCodec::Aac => (
            64_000 * u64::from(track.channels.unwrap_or(2)),
            sample_rate / 1024.0,
        ),
        // Video, and anything the demuxer would not name — including TrueHD,
        // which is not carried. Claiming no bitrate for an unnamed soundtrack
        // leaves its bytes attributed to the picture, which is the safe
        // direction.
        _ => (
            0,
            if track.track_kind == TrackKind::Video {
                24.0
            } else {
                sample_rate / 1536.0
            },
        ),
    };
    TrackRate {
        id: track.id,
        bits_per_second: bits,
        frames_per_second: frames,
    }
}

/// What carrying `bits` a second in frames of `fps` costs in a transport stream.
///
/// Not a percentage. A transport stream is a run of 188-byte packets and the
/// last one of every frame is stuffed out to 188 whatever is left over, so the
/// cost is a step function of the frame size: two per cent on a 130-kilobyte
/// picture, twenty-two on a 768-byte AC-3 frame. A flat multiplier that is right
/// for one is badly wrong for the other, and being wrong low here is the film's
/// last minutes cut off.
fn transport_cost(bits: u64, fps: f64) -> u64 {
    /// A PES header carrying both timestamps, which is what this muxer writes.
    const PES_HEADER: f64 = 19.0;
    /// Payload in a packet: 188 less the four-byte header, less a two-byte
    /// adaptation field for the flags the first packet of a frame carries.
    const PAYLOAD: f64 = 182.0;

    // `is_finite` first, so a track measured as NaN falls back rather than
    // multiplying its way into the promise.
    if bits == 0 || !fps.is_finite() || fps <= 0.0 {
        return bits;
    }
    let per_frame = bits as f64 / (8.0 * fps) + PES_HEADER;
    let packets = (per_frame / PAYLOAD).ceil();
    (packets * TS_PACKET_LEN as f64 * 8.0 * fps) as u64
}

/// The bits a second the transport stream itself will run at.
///
/// Which is the number the whole seek mechanism rests on. A byte offset into
/// this resource is read as a fraction of its promised length, so the promise
/// being an honest account of what the stream weighs is what makes an offset
/// mean the moment the viewer dragged to. Promise three times the truth — which
/// assuming the output weighs what the source did does, on a film carrying five
/// DTS soundtracks that leave as stereo AAC — and every byte offset names a
/// moment three times too far along.
///
/// `tracks` is every track the file has, because what the picture costs is what
/// is left of the file once its soundtracks are accounted for. `carried` is the
/// subset this response will actually write.
fn stream_bitrate(
    source_size: u64,
    duration_secs: f64,
    tracks: &[TrackInfo],
    carried: &[TrackInfo],
    rates: &TrackRates,
    soundtrack: SoundtrackFormat,
) -> u64 {
    use crate::media::remux::TrackKind;

    /// The programme tables, at [`TABLE_INTERVAL`]: two packets, ten times a
    /// second.
    const TABLE_BITS: u64 = 20 * TS_PACKET_LEN as u64 * 8;

    let rate_of = |track: &TrackInfo| rates.get(track.id).unwrap_or_else(|| nominal_rate(track));

    let source_bits = (source_size as f64 * 8.0 / duration_secs) as u64;
    let source_audio: u64 = tracks
        .iter()
        .filter(|track| track.track_kind == TrackKind::Audio)
        .map(|track| rate_of(track).bits_per_second)
        .sum();
    // A film whose soundtracks appear to outweigh it has been measured badly,
    // or is mostly soundtrack. Either way the picture is not nothing.
    let video_bits = source_bits
        .saturating_sub(source_audio)
        .max(source_bits / 20);
    let video_fps = tracks
        .iter()
        .find(|track| track.track_kind == TrackKind::Video)
        .map(|track| rate_of(track).frames_per_second)
        .unwrap_or(24.0);

    let carried_bits: u64 = carried
        .iter()
        .map(|track| {
            let rate = rate_of(track);
            match audio_disposition(track) {
                AudioDisposition::Passthrough => {
                    transport_cost(rate.bits_per_second, rate.frames_per_second)
                }
                AudioDisposition::Reencoded => {
                    let sample_rate = f64::from(track.sample_rate.unwrap_or(48_000));
                    let channels = soundtrack.channels(track.channels);
                    match soundtrack {
                        #[cfg(feature = "transcode-ac3")]
                        SoundtrackFormat::Ac3 => return_ac3_cost(sample_rate, channels),
                        // Unreachable in practice — `available()` has already
                        // turned Ac3 into Aac for a build without the encoder —
                        // but the arm has to exist for the match to compile.
                        #[cfg(not(feature = "transcode-ac3"))]
                        SoundtrackFormat::Ac3 => transport_cost(
                            u64::from(AacEncoder::bitrate_for(channels)),
                            sample_rate / super::AAC_FRAME_SAMPLES as f64,
                        ),
                        SoundtrackFormat::Aac => transport_cost(
                            u64::from(AacEncoder::bitrate_for(channels)),
                            sample_rate / super::AAC_FRAME_SAMPLES as f64,
                        ),
                    }
                }
                AudioDisposition::Dropped => 0,
            }
        })
        .sum();

    transport_cost(video_bits, video_fps) + carried_bits + TABLE_BITS
}

/// What the AC-3 experiment's soundtracks cost in the stream.
#[cfg(feature = "transcode-ac3")]
fn return_ac3_cost(sample_rate: f64, channels: u16) -> u64 {
    transport_cost(
        u64::from(super::Ac3Encoder::bitrate_for(channels)),
        sample_rate / super::AC3_FRAME_SAMPLES as f64,
    )
}

/// The length a transport stream of this film commits to, in bytes.
///
/// Leaning long, and the lean is the only part that is not an estimate. The
/// response is made exactly this length whatever the muxer produces: short of it
/// is padding a renderer skips, over it is the film's last seconds cut off. So
/// the margin buys the second outcome off with a little of the first.
pub fn promised_ts_length(
    source_size: u64,
    duration_secs: f64,
    tracks: &[TrackInfo],
    carried: &[TrackInfo],
    rates: &TrackRates,
    soundtrack: SoundtrackFormat,
) -> u64 {
    /// Slack over the estimate. Small, because with the tracks measured and the
    /// packet cost counted rather than guessed the estimate lands within a few
    /// per cent, and every point of it is bytes a renderer fetches and throws
    /// away. It covers what is left: the parameter sets beside every keyframe,
    /// and a picture whose average bitrate the file's own size understates.
    const MARGIN: f64 = 1.10;
    /// Nothing shorter, so a very short film is not trimmed by the tables.
    const FLOOR: u64 = 1 << 18;

    let bits = stream_bitrate(source_size, duration_secs, tracks, carried, rates, soundtrack);
    let bytes = (bits as f64 * duration_secs * MARGIN / 8.0) as u64;
    let aligned = (bytes.max(FLOOR) / TS_PACKET_LEN as u64) * TS_PACKET_LEN as u64;
    aligned.max(TS_PACKET_LEN as u64)
}

/// One track being carried, and what has to happen to its packets.
struct Stream {
    spec: TsStreamSpec,
    source_id: u32,
    time_base: Option<symphonia::core::units::TimeBase>,
    /// `None` for a track passed through as it stands.
    codec: Option<TranscodeCodec>,
    /// Set only for video: what to write before each keyframe.
    parameter_sets: Option<ParameterSets>,
    /// The rate a re-encoded track runs at, from the container's declaration.
    sample_rate: u32,
    /// The decoder for a re-encoded track, kept apart from the rest of its
    /// chain because it is the half that can be run beside the other tracks'.
    /// See [`TsStream::decode_held`].
    decoder: Option<PcmDecoder>,
    /// Everything downstream of the decoder.
    encode: Option<Encode>,
    /// What this track is being re-encoded to, and how wide. `None` for a track
    /// passed through as it stands.
    reencode: Option<Reencode>,
    /// Packets read but not yet decoded, with the instant each arrived at.
    ///
    /// Held rather than decoded where they are read, so that every soundtrack
    /// can be decoded at once when the batch is written. A film with four DTS
    /// tracks decodes four of them, and doing that one after another on the
    /// muxing thread is most of what a renderer waits through before its first
    /// byte.
    held: Vec<(u64, Vec<u8>)>,
    /// What the decoder produced, waiting for the encoder.
    pcm: Vec<(u64, Vec<u8>)>,
    /// Access units waiting for the batch to be written.
    pending: Vec<Unit>,
}

/// What a re-encoded track is becoming: the format, and the channel count that
/// format keeps for this particular soundtrack.
#[derive(Debug, Clone, Copy)]
struct Reencode {
    format: SoundtrackFormat,
    channels: u16,
}

/// The state of one track's re-encoding, downstream of its decoder.
///
/// The decoder is not here. It lives on the [`Stream`] so that the decoding of
/// every soundtrack can be run side by side, which the rest of this cannot be:
/// the AAC encoder wraps a C library holding raw pointers and does not cross a
/// thread boundary at all.
struct Encode {
    encoder: Reencoder,
    /// Channels the encoder was built for — the film's own layout on the AC-3
    /// path, stereo on the AAC one. What the decoder emits is fitted to this.
    target_channels: u16,
    decoded_channels: u16,
    sample_rate: u32,
    /// Where the re-encoded run sits, in samples. `None` until enough packets
    /// have been seen to say.
    next_pts: Option<u64>,
    anchors: Vec<i64>,
    decoded: u64,
    /// Frames encoded before the run could be placed.
    held: Vec<Vec<u8>>,
}

/// One access unit, ready to become a PES packet once its decode time is known.
struct Unit {
    pts: u64,
    dts: u64,
    keyframe: bool,
    data: Vec<u8>,
}

/// A film being rewritten as a transport stream.
pub struct TsStream {
    format: Box<dyn symphonia::core::formats::FormatReader>,
    muxer: TsMuxer,
    /// Video first, then every soundtrack in the order the caller gave them.
    streams: Vec<Stream>,
    specs: Vec<TsStreamSpec>,
    video_pid: u16,
    video_id: u32,
    video_codec: TrackCodec,
    /// Packets read while priming the decoders, replayed ahead of anything
    /// further from the demuxer. See [`prime_decoders`].
    primed: std::collections::VecDeque<PrimedPacket>,
    /// Where the demuxer actually landed, in the source's own units.
    ///
    /// Not where it was asked to land. A coarse seek snaps back to the nearest
    /// random-access point, so every byte offset inside one group of pictures
    /// opens the same stream and produces the same first batch — which is what
    /// makes that batch worth remembering. See `web::ts_streaming`.
    origin: u64,
    /// Presentation time of the batch's first picture, in the output clock.
    batch_start: Option<u64>,
    /// The latest presentation time held in the batch, which is what says
    /// whether it can be cut here. See [`TsStream::next_chunk`].
    batch_max_pts: u64,
    started: bool,
    finished: bool,
}

impl TsStream {
    /// Open `path` positioned at `start_secs`, ready to emit packets.
    ///
    /// A track this build can neither pass through nor produce is dropped rather
    /// than carried, exactly as in the MP4 path: better a film with two working
    /// soundtracks than one with a third that plays noise.
    pub fn open(
        path: &Path,
        video: &TrackInfo,
        audio: &[TrackInfo],
        start_secs: f64,
        soundtrack: SoundtrackFormat,
    ) -> Result<Self> {
        use symphonia::core::formats::{SeekMode, SeekTo};
        use symphonia::core::units::Time;

        let mut format = super::source::open_format(path)?;
        let mut origin = 0u64;
        if start_secs > 0.0 {
            // Coarse, not Accurate: a stream has to open on a random-access
            // point or the renderer has no reference frame to decode the first
            // picture against.
            let seeked = format.seek(
                SeekMode::Coarse,
                SeekTo::Time {
                    time: Time::try_from_secs_f64(super::seek_target(start_secs))
                        .unwrap_or(Time::ZERO),
                    track_id: Some(video.id),
                },
            );
            if let Ok(seeked) = seeked {
                origin = seeked.actual_ts.get().max(0) as u64;
            }
        }

        let parameter_sets = ParameterSets::parse(video.codec_kind, &video.extra_data)
            .ok_or_else(|| anyhow::anyhow!("no parameter sets for the video track"))?;
        let mut next_pid = FIRST_ES_PID;
        let mut streams = Vec::new();
        streams.push(Stream {
            spec: TsStreamSpec::for_codec(video.codec_kind, next_pid)
                .ok_or_else(|| anyhow::anyhow!("{} has no transport mapping", video.codec))?,
            source_id: video.id,
            time_base: track_time_base(format.as_ref(), video.id),
            codec: None,
            parameter_sets: Some(parameter_sets),
            sample_rate: 0,
            decoder: None,
            encode: None,
            reencode: None,
            held: Vec::new(),
            pcm: Vec::new(),
            pending: Vec::new(),
        });

        for track in audio {
            next_pid += 1;
            // Dolby and AAC ride as they are; anything else — which in
            // practice means DTS — is re-encoded, and is described as whatever
            // it became from here on.
            let (carried, codec, reencode) = match audio_disposition(track) {
                AudioDisposition::Passthrough => (track.codec_kind, None, None),
                AudioDisposition::Reencoded => (
                    soundtrack.carried_as(),
                    track.codec_kind.transcode_codec(),
                    Some(Reencode {
                        format: soundtrack,
                        channels: soundtrack.channels(track.channels),
                    }),
                ),
                AudioDisposition::Dropped => continue,
            };
            let Some(spec) = TsStreamSpec::for_codec(carried, next_pid) else {
                continue;
            };
            streams.push(Stream {
                spec,
                source_id: track.id,
                time_base: track_time_base(format.as_ref(), track.id),
                codec,
                parameter_sets: None,
                sample_rate: track.sample_rate.unwrap_or(48_000),
                decoder: None,
                encode: None,
                reencode,
                held: Vec::new(),
                pcm: Vec::new(),
                pending: Vec::new(),
            });
        }

        // Everything the programme map is about to promise, proved before it
        // promises it.
        let primed = prime_decoders(format.as_mut(), &mut streams);

        let specs = streams.iter().map(|s| s.spec.clone()).collect();
        Ok(Self {
            format,
            muxer: TsMuxer::new(),
            video_pid: streams[0].spec.pid,
            video_id: video.id,
            video_codec: if streams[0].spec.stream_type == 0x24 {
                TrackCodec::Hevc
            } else {
                TrackCodec::Avc
            },
            streams,
            specs,
            primed,
            origin,
            batch_start: None,
            batch_max_pts: 0,
            started: false,
            finished: false,
        })
    }

    /// Where the demuxer landed, which is the same for every byte offset inside
    /// one group of pictures.
    pub fn origin(&self) -> u64 {
        self.origin
    }

    /// The next run of transport packets, or `None` at the end of the film.
    pub fn next_chunk(&mut self) -> Option<Vec<u8>> {
        if self.finished {
            return None;
        }
        let batch_ticks = (BATCH_SECS * TS_CLOCK_HZ as f64) as u64;

        loop {
            let Some((id, pts, data)) = self.next_source_packet() else {
                self.finished = true;
                self.decode_held();
                self.flush_encoders();
                return self.emit();
            };

            if id == self.video_id {
                let keyframe = packet_is_keyframe(&data, self.video_codec);
                // Nothing before the first random-access point is decodable.
                if !self.started && !keyframe {
                    continue;
                }
                self.started = true;
                let ticks = self.rescale(0, pts);
                let elapsed = ticks.saturating_sub(*self.batch_start.get_or_insert(ticks));
                // Where the batch may be cut. Not only at a keyframe: a film
                // with five-second groups of pictures would then hand a renderer
                // five seconds of decoded soundtrack before its first byte, and
                // a set binary-searching for a seek point pays that twenty times
                // over.
                //
                // Anywhere the reordering is closed will do, and this frame
                // being displayed after everything already held is exactly that:
                // the batch is then a complete run in both orders, so sorting it
                // recovers its decode times without reference to what follows,
                // and the next batch's times all fall after this one's. A
                // keyframe satisfies it too, which is why the old rule worked.
                let closed = ticks > self.batch_max_pts;
                if (keyframe || closed) && elapsed >= batch_ticks && !self.streams[0].pending.is_empty()
                {
                    let chunk = self.emit();
                    self.batch_start = Some(ticks);
                    self.batch_max_pts = 0;
                    self.push_video(ticks, data, keyframe);
                    if chunk.is_some() {
                        return chunk;
                    }
                    continue;
                }
                self.push_video(ticks, data, keyframe);
                continue;
            }

            // Audio ahead of the first picture is dropped: a renderer given
            // sound before it has a frame has nothing to synchronise against.
            if !self.started {
                continue;
            }
            if let Err(error) = self.take_audio(id, pts, &data) {
                tracing::debug!(%error, "dropping an audio packet that would not re-encode");
            }
        }
    }

    /// The next packet from the film: what priming read, then the demuxer.
    fn next_source_packet(&mut self) -> Option<PrimedPacket> {
        if let Some(packet) = self.primed.pop_front() {
            return Some(packet);
        }
        match self.format.next_packet() {
            Ok(Some(packet)) => Some((
                packet.track_id,
                packet.pts.get(),
                packet.data.to_vec(),
            )),
            _ => None,
        }
    }

    fn rescale(&self, index: usize, ticks: i64) -> u64 {
        match self.streams[index].time_base {
            Some(time_base) => rescale_ticks(ticks, time_base, TS_CLOCK_HZ as u32),
            None => ticks.max(0) as u64,
        }
    }

    fn push_video(&mut self, ticks: u64, data: Vec<u8>, keyframe: bool) {
        let sets = self.streams[0]
            .parameter_sets
            .clone()
            .unwrap_or_default();
        let codec = self.video_codec;
        self.batch_max_pts = self.batch_max_pts.max(ticks);
        self.streams[0].pending.push(Unit {
            pts: ticks,
            dts: ticks,
            keyframe,
            data: to_annexb(&data, &sets, keyframe, codec),
        });
    }

    /// Feed one audio packet to whichever carried track it belongs to.
    fn take_audio(&mut self, track_id: u32, pts: i64, data: &[u8]) -> Result<()> {
        let Some(index) = self
            .streams
            .iter()
            .position(|stream| stream.source_id == track_id && stream.spec.pid != self.video_pid)
        else {
            return Ok(());
        };
        let ticks = self.rescale(index, pts);
        let stream = &mut self.streams[index];

        if stream.codec.is_none() {
            // Passed through: the container's frame is the access unit, and
            // there is no work to put off.
            stream.pending.push(Unit {
                pts: ticks,
                dts: ticks,
                keyframe: true,
                data: data.to_vec(),
            });
            return Ok(());
        }
        stream.held.push((ticks, data.to_vec()));
        Ok(())
    }

    /// Turn every held packet into access units, decoding all the soundtracks
    /// at once.
    ///
    /// Two phases, and the split is forced by what can cross a thread boundary.
    /// A decoder owns nothing shared and is `Send`, so one thread per soundtrack
    /// makes the wait the slowest track rather than the sum of them all — which
    /// on a film with four DTS soundtracks is most of the wait. The AAC encoder
    /// wraps a C library holding raw pointers, is not `Send`, and stays here.
    fn decode_held(&mut self) {
        std::thread::scope(|scope| {
            for stream in self.streams.iter_mut().skip(1) {
                if stream.held.is_empty() {
                    continue;
                }
                let (Some(codec), Some(decoder)) = (stream.codec, stream.decoder.as_mut()) else {
                    continue;
                };
                let held = std::mem::take(&mut stream.held);
                let pcm = &mut stream.pcm;
                scope.spawn(move || {
                    for (ticks, frame) in held {
                        let expect = super::frames::frame_samples(codec, &frame);
                        pcm.push((ticks, decoder.decode_or_silence(&frame, expect)));
                    }
                });
            }
        });

        for stream in self.streams.iter_mut().skip(1) {
            let Some(encode) = stream.encode.as_mut() else {
                stream.pcm.clear();
                continue;
            };
            for (ticks, pcm) in std::mem::take(&mut stream.pcm) {
                if let Err(error) = take_decoded(&mut stream.pending, encode, ticks, pcm) {
                    tracing::debug!(%error, "dropping an audio packet that would not re-encode");
                }
            }
        }
    }

    fn flush_encoders(&mut self) {
        for stream in &mut self.streams {
            if let Some(decode) = stream.encode.as_mut() {
                let tail = decode.encoder.finish();
                place_frames(&mut stream.pending, decode, tail, true);
            }
        }
    }

    /// Write everything held as transport packets, interleaved by decode time.
    fn emit(&mut self) -> Option<Vec<u8>> {
        self.decode_held();
        // A batch about to be written cannot wait for more packets before
        // deciding where a re-encoded run sits, so this is where it is forced.
        for stream in &mut self.streams {
            if let Some(decode) = stream.encode.as_mut() {
                place_frames(&mut stream.pending, decode, Vec::new(), true);
            }
        }

        // Matroska stores presentation timestamps in decode order and no decode
        // timestamps; sorting the batch's presentation times recovers the decode
        // timeline exactly, because each frame is decoded once and shown once.
        let mut video = std::mem::take(&mut self.streams[0].pending);
        let mut times: Vec<crate::media::remux::MediaPacket> = video
            .iter()
            .map(|unit| crate::media::remux::MediaPacket {
                track_id: 0,
                pts: unit.pts,
                dts: unit.dts,
                duration: 0,
                is_keyframe: unit.keyframe,
                data: Vec::new(),
            })
            .collect();
        derive_decode_timestamps(&mut times);
        for (unit, derived) in video.iter_mut().zip(&times) {
            unit.dts = derived.dts;
        }

        // (decode time, stream index, unit)
        let mut ordered: Vec<(u64, usize, Unit)> =
            video.drain(..).map(|unit| (unit.dts, 0, unit)).collect();
        for index in 1..self.streams.len() {
            for unit in self.streams[index].pending.drain(..) {
                ordered.push((unit.dts, index, unit));
            }
        }
        if ordered.is_empty() {
            return None;
        }
        ordered.sort_by_key(|(dts, index, _)| (*dts, *index));

        // The tables lead every batch, and are then repeated inside it.
        // Repeating them is what lets a decoder that joined part way through
        // learn what the programme contains without having seen the beginning
        // of it — so the gap between one pair and the next is the floor on how
        // long a seek takes to show a picture, and a batch is far too long to
        // make a viewer wait.
        let mut out = self.muxer.pat();
        out.extend_from_slice(&self.muxer.pmt(self.video_pid, &self.specs));
        let mut tables_at = ordered[0].0;

        for (_, index, unit) in ordered {
            let spec = self.streams[index].spec.clone();
            let is_video = index == 0;
            if unit.dts.saturating_sub(tables_at) >= TABLE_INTERVAL {
                tables_at = unit.dts;
                out.extend_from_slice(&self.muxer.pat());
                out.extend_from_slice(&self.muxer.pmt(self.video_pid, &self.specs));
            }
            // The programme clock rides on the video track, at every picture —
            // a decoder needs it far more often than the hundred milliseconds
            // the standard allows between one and the next. It is deliberately
            // the *unshifted* decode time: the presentation stamps run
            // `CLOCK_HEADROOM` ahead of it, and that gap is the renderer's
            // buffer.
            self.muxer.pes(
                &mut out,
                &spec,
                &unit.data,
                &PesTiming {
                    pts: unit.pts + CLOCK_HEADROOM,
                    dts: Some(unit.dts + CLOCK_HEADROOM),
                    random_access: unit.keyframe,
                    pcr: is_video.then_some(unit.dts),
                },
            );
        }
        Some(out)
    }
}

/// Read far enough ahead to prove every soundtrack in `streams` can produce
/// packets, and drop the ones that cannot.
///
/// A stream declared in the programme map and then silent forever is worse than
/// one that was never declared. A renderer reads the map, sees a PID it is
/// expecting audio on, and waits for it — so a DTS track whose decoder will not
/// open, or a track the container lists but never writes a block for, does not
/// cost that soundtrack. It costs the film.
///
/// Two ways a track fails that, and both are checked here. It may never appear
/// in the stream at all, which the read below finds; or its decoder may refuse
/// its first frame, which is found by opening one on that frame and throwing it
/// away. The decode chain is then built again, lazily, on the same frame when it
/// is replayed — one frame decoded twice at the start of a film, against a
/// television that would otherwise sit on a black screen.
///
/// Every packet read on the way is handed back to be replayed rather than
/// dropped: one of them is the keyframe the film has to start on.
fn prime_decoders(
    format: &mut dyn symphonia::core::formats::FormatReader,
    streams: &mut Vec<Stream>,
) -> std::collections::VecDeque<PrimedPacket> {
    let mut primed = std::collections::VecDeque::new();
    let wanted: Vec<u32> = streams[1..].iter().map(|stream| stream.source_id).collect();
    if wanted.is_empty() {
        return primed;
    }

    let mut first_frames: std::collections::HashMap<u32, Vec<u8>> =
        std::collections::HashMap::new();
    let mut read = 0usize;
    for _ in 0..PRIME_PACKETS {
        if read >= PRIME_BYTES {
            break;
        }
        let Ok(Some(packet)) = format.next_packet() else {
            break;
        };
        let (id, pts, data) = (packet.track_id, packet.pts.get(), packet.data.to_vec());
        read += data.len();
        if wanted.contains(&id) {
            first_frames.entry(id).or_insert_with(|| data.clone());
        }
        primed.push_back((id, pts, data));
        if wanted.iter().all(|id| first_frames.contains_key(id)) {
            break;
        }
    }

    streams.retain_mut(|stream| {
        if stream.spec.pid == FIRST_ES_PID {
            return true;
        }
        let Some(frame) = first_frames.get(&stream.source_id) else {
            tracing::warn!(
                track = stream.source_id,
                "dropping a soundtrack that carries no packets, rather than \
                 declaring a stream a renderer would wait on forever"
            );
            return false;
        };
        let Some(codec) = stream.codec else {
            return true;
        };
        match open_chain(stream, codec, frame) {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(
                    track = stream.source_id,
                    "dropping a soundtrack whose decoder will not open: {error:#}"
                );
                false
            }
        }
    });
    primed
}

/// Build both halves of one track's re-encoding, on its first frame.
///
/// Kept rather than proved and thrown away: the frame is decoded once here to
/// find out whether the chain works and what shape its output is, and the
/// decoder that did it is the one the film then runs through. The frame itself
/// is decoded again when the priming read is replayed, which for these codecs —
/// where every frame stands alone — costs one frame and keeps the packet path
/// with no special case in it.
fn open_chain(stream: &mut Stream, codec: TranscodeCodec, frame: &[u8]) -> Result<()> {
    let sample_rate = stream.sample_rate;
    // A track only reaches here because it is being re-encoded, so it has a
    // target; treat a missing one as the stereo AAC this path used to assume.
    let target = stream.reencode.unwrap_or(Reencode {
        format: SoundtrackFormat::Aac,
        channels: DECODED_CHANNELS,
    });
    let want = target.channels;
    // Ask the decoder for the width the encoder wants: for AC-3 that is the
    // film's own layout, so the surround survives instead of being folded down
    // and back up again.
    let (decoder, _) = PcmDecoder::open(codec, sample_rate, Some(want), frame)?;
    let decoded_channels = decoder.channels();
    let encoder = match target.format {
        #[cfg(feature = "transcode-ac3")]
        SoundtrackFormat::Ac3 => Reencoder::Ac3(super::Ac3Encoder::new(sample_rate, want)?),
        #[cfg(not(feature = "transcode-ac3"))]
        SoundtrackFormat::Ac3 => Reencoder::Aac(Box::new(AacEncoder::new(sample_rate, want)?)),
        SoundtrackFormat::Aac => Reencoder::Aac(Box::new(AacEncoder::new(sample_rate, want)?)),
    };
    stream.decoder = Some(decoder);
    stream.encode = Some(Encode {
        encoder,
        decoded_channels,
        target_channels: want,
        sample_rate,
        next_pts: None,
        anchors: Vec::new(),
        decoded: 0,
        held: Vec::new(),
    });
    Ok(())
}

/// Take one frame's decoded PCM and encode it.
fn take_decoded(pending: &mut Vec<Unit>, decode: &mut Encode, ticks: u64, pcm: Vec<u8>) -> Result<()> {
    // Where this run starts, if it is contiguous — asked of every packet, so
    // that one rounded container timestamp cannot place the whole run.
    let samples = (ticks as i128 * i64::from(decode.sample_rate) as i128
        / TS_CLOCK_HZ as i128) as i64;
    decode.anchors.push(samples - decode.decoded as i64);
    decode.decoded += (pcm.len() / (decode.decoded_channels as usize * 2)) as u64;
    let pcm = super::fit_channels(&pcm, decode.decoded_channels, decode.target_channels);
    let frames = decode.encoder.push(&pcm)?;
    place_frames(pending, decode, frames, false);
    Ok(())
}

/// Hold the encoder's ADTS frames until the run's place is settled, then queue
/// them as access units.
///
/// The ADTS headers stay on, unlike the MP4 path which strips them: a transport
/// stream carries AAC exactly as the encoder framed it, because there is no
/// sample entry alongside to repeat what the header says.
fn place_frames(pending: &mut Vec<Unit>, decode: &mut Encode, frames: Vec<Vec<u8>>, settle: bool) {
    decode.held.extend(frames);
    if decode.next_pts.is_none() {
        if !settle && decode.anchors.len() < ANCHOR_PACKETS {
            return;
        }
        if decode.anchors.is_empty() {
            return;
        }
        let mut anchors = std::mem::take(&mut decode.anchors);
        // Placed early by the encoder's own delay, which is what a decoder's
        // output trails its input by.
        let anchor = super::run_anchor(&mut anchors).saturating_sub(decode.encoder.delay());
        decode.next_pts = Some(anchor.max(0) as u64);
    }
    let frame_samples = decode.encoder.frame_samples();
    let mut samples = decode.next_pts.unwrap_or(0);
    for frame in decode.held.drain(..) {
        let ticks = samples * TS_CLOCK_HZ / u64::from(decode.sample_rate);
        pending.push(Unit {
            pts: ticks,
            dts: ticks,
            keyframe: true,
            data: frame,
        });
        samples += frame_samples;
    }
    decode.next_pts = Some(samples);
}

/// Whole ADTS frames, headers included.
fn adts_frames(stream: &[u8]) -> Vec<&[u8]> {
    let mut frames = Vec::new();
    let mut pos = 0usize;
    while pos + 7 <= stream.len() {
        let header = &stream[pos..];
        if header[0] != 0xFF || (header[1] & 0xF0) != 0xF0 {
            break;
        }
        let len = (((u32::from(header[3]) & 0x03) << 11)
            | (u32::from(header[4]) << 3)
            | (u32::from(header[5]) >> 5)) as usize;
        if len < 7 || pos + len > stream.len() {
            break;
        }
        frames.push(&stream[pos..pos + len]);
        pos += len;
    }
    frames
}

fn track_time_base(
    format: &dyn symphonia::core::formats::FormatReader,
    track_id: u32,
) -> Option<symphonia::core::units::TimeBase> {
    format
        .tracks()
        .iter()
        .find(|t| t.id == track_id)
        .and_then(|t| t.time_base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::remux::TrackKind;

    fn track(id: u32, kind: TrackKind, codec: TrackCodec, channels: u8) -> TrackInfo {
        TrackInfo {
            id,
            track_kind: kind,
            codec: format!("{codec:?}"),
            codec_kind: codec,
            language: None,
            name: None,
            sample_rate: Some(48_000),
            channels: Some(channels),
            width: None,
            height: None,
            is_default: id == 1,
            extra_data: Vec::new(),
        }
    }

    fn rate(id: u32, bits: u64, fps: f64) -> TrackRate {
        TrackRate {
            id,
            bits_per_second: bits,
            frames_per_second: fps,
        }
    }

    /// The case the whole estimate exists for, and the one no fixture in this
    /// repository is big enough to reproduce.
    ///
    /// A film carrying five DTS soundtracks, which between them are most of what
    /// it weighs. Not one of them reaches the renderer at the rate it left the
    /// disc at, so promising the file's own size names every byte offset far
    /// further along than it belongs — a scrub bar that lands nowhere near where
    /// it was dragged to.
    ///
    /// How much further depends on what the soundtracks become, which is why the
    /// promise is derived from the output's codecs rather than the input's, and
    /// why this checks both: 5.1 AC-3 at 640 kbps is three times the stereo AAC
    /// the path used to produce unconditionally, and a promise that did not know
    /// the difference would be wrong by that much in one direction or the other.
    #[test]
    fn five_dts_soundtracks_do_not_reach_the_renderer_weighing_what_they_did() {
        let duration = 120.0;
        let source = 124_000_000u64;

        let mut tracks = vec![track(1, TrackKind::Video, TrackCodec::Avc, 0)];
        tracks.extend((2..7).map(|id| track(id, TrackKind::Audio, TrackCodec::Dts, 6)));
        let carried: Vec<TrackInfo> = tracks[1..].to_vec();

        // Measured: the picture at a megabit and a half, each soundtrack near
        // the DTS core rate in eleven-millisecond frames.
        let mut measured = vec![rate(1, 0, 24.0)];
        measured.extend((2..7).map(|id| rate(id, 1_360_000, 93.75)));
        let rates = TrackRates(measured);

        // A megabit and a half of picture, the programme tables, and five
        // soundtracks at whatever the format in hand costs — each reached
        // without reference to the source's size, which is the whole point.
        let promise_of = |format| {
            promised_ts_length(source, duration, &tracks, &carried, &rates, format)
        };
        let weighing = |audio_each: f64| (1_470_000.0 * 1.03 + 5.0 * audio_each + 30_080.0) * duration / 8.0;

        // Folded down to stereo at 192 kbps, five soundtracks shrink eightfold
        // and the stream is a third of the file.
        let aac = promise_of(SoundtrackFormat::Aac);
        assert!(
            aac < source / 2,
            "five soundtracks shrank eightfold and the promise did not: {aac} \
             against a {source}-byte source"
        );
        let expected = weighing(211_500.0);
        assert!(
            (aac as f64) > expected && (aac as f64) < expected * 1.2,
            "{aac} is not the {expected} a stereo AAC stream actually weighs"
        );

        // Kept at 5.1 and re-encoded to AC-3 at 640 kbps — the default, and
        // three and a half times the AAC — the soundtracks still arrive at half
        // the rate they left at, so the stream is two thirds of the file rather
        // than a third. Honest either way is the requirement; identical is not.
        let ac3 = promise_of(SoundtrackFormat::Ac3.available());
        let expected = weighing(if cfg!(feature = "transcode-ac3") {
            705_000.0
        } else {
            // No AC-3 encoder in this build, so `available()` has already
            // turned the default back into stereo AAC.
            211_500.0
        });
        assert!(
            (ac3 as f64) > expected && (ac3 as f64) < expected * 1.2,
            "{ac3} is not the {expected} this stream actually weighs"
        );
        assert!(
            ac3 < source,
            "{ac3} promises more than the {source} the film weighs with its DTS \
             still on it"
        );
    }

    /// The estimate leans long on purpose: short of the promise is padding a
    /// renderer skips, over it is the film's last minutes cut off.
    #[test]
    fn the_picture_always_fits_inside_the_promise() {
        const GIB: u64 = 1 << 30;
        let duration = 7200.0;

        let mut tracks = vec![track(1, TrackKind::Video, TrackCodec::Avc, 0)];
        tracks.extend((2..7).map(|id| track(id, TrackKind::Audio, TrackCodec::Dts, 6)));
        let carried: Vec<TrackInfo> = tracks[1..].to_vec();
        let mut measured = vec![rate(1, 0, 24.0)];
        measured.extend((2..7).map(|id| rate(id, 1_509_000, 93.75)));
        let rates = TrackRates(measured);

        let source = 30 * GIB;
        let promised = promised_ts_length(source, duration, &tracks, &carried, &rates, SoundtrackFormat::default());
        // The picture is the file less its five soundtracks, and it passes
        // through untouched, so every byte of it has to fit.
        let picture = source - (5 * 1_509_000 * 7200 / 8);
        assert!(
            promised > picture,
            "{promised} does not leave room for {picture} bytes of picture"
        );
        assert!(
            promised < source + source / 16,
            "and should still be under what the source's own size would promise"
        );
    }

    /// A soundtrack that is passed through costs what it always cost, so a film
    /// of Dolby leaves at roughly the weight it arrived — which is the case the
    /// old source-sized promise got right and this must not get wrong.
    #[test]
    fn a_film_that_only_passes_its_dolby_through_is_promised_what_it_weighs() {
        let duration = 3600.0;
        let tracks = vec![
            track(1, TrackKind::Video, TrackCodec::Avc, 0),
            track(2, TrackKind::Audio, TrackCodec::Ac3, 6),
        ];
        let carried = vec![tracks[1].clone()];
        let rates = TrackRates(vec![rate(1, 0, 24.0), rate(2, 640_000, 31.25)]);

        let source = 4 * (1u64 << 30);
        let promised = promised_ts_length(source, duration, &tracks, &carried, &rates, SoundtrackFormat::default());
        assert!(
            promised > source && promised < source * 6 / 5,
            "a passthrough film should be promised its own size and a little over, \
             not {promised} against {source}"
        );
    }

    /// Frames small enough that the packet they are stuffed into costs more than
    /// they do. A flat percentage overhead is badly wrong here, and wrong low —
    /// which is the film's last minutes cut off rather than a little padding.
    #[test]
    fn the_packet_cost_is_counted_rather_than_guessed_at() {
        // 192 kbps in 768-byte AC-3 frames: 782 bytes of PES, which is five
        // packets of 188 rather than the four and a bit a percentage would give.
        let cost = transport_cost(192_000, 31.25);
        assert_eq!(cost, (5.0 * 188.0 * 8.0 * 31.25) as u64);
        assert!(
            cost > 192_000 * 6 / 5,
            "a fifth of this track is stuffing, and {cost} does not show it"
        );

        // A 130-kilobyte picture pays for its header and almost nothing else.
        let cost = transport_cost(25_000_000, 24.0);
        assert!(
            cost > 25_000_000 && cost < 25_000_000 * 21 / 20,
            "a large frame should cost a couple of per cent, not {cost}"
        );

        // Nothing to say about a track with no rate or no frames.
        assert_eq!(transport_cost(0, 24.0), 0);
        assert_eq!(transport_cost(500, 0.0), 500);
    }

    /// The disposition decides both what the muxer does and what the handler
    /// promises, so the two cannot be allowed to answer it differently.
    #[test]
    fn dolby_and_aac_ride_as_they_are_and_dts_is_re_encoded() {
        assert_eq!(
            audio_disposition(&track(1, TrackKind::Audio, TrackCodec::Ac3, 6)),
            AudioDisposition::Passthrough
        );
        assert_eq!(
            audio_disposition(&track(1, TrackKind::Audio, TrackCodec::Eac3, 6)),
            AudioDisposition::Passthrough
        );
        assert_eq!(
            audio_disposition(&track(1, TrackKind::Audio, TrackCodec::Aac, 2)),
            AudioDisposition::Passthrough
        );
        assert_eq!(
            audio_disposition(&track(1, TrackKind::Audio, TrackCodec::Unsupported, 2)),
            AudioDisposition::Dropped,
            "TrueHD and anything else unnamed has no way into a transport stream"
        );
        let dts = audio_disposition(&track(1, TrackKind::Audio, TrackCodec::Dts, 6));
        assert_eq!(
            dts,
            if cfg!(all(feature = "transcode-dts", feature = "transcode-aac")) {
                AudioDisposition::Reencoded
            } else {
                AudioDisposition::Dropped
            }
        );
    }

    #[test]
    fn adts_frames_are_returned_whole_unlike_the_mp4_paths_payloads() {
        // Two frames of seven-byte headers and one byte of payload each.
        let mut stream = Vec::new();
        for _ in 0..2 {
            stream.extend_from_slice(&[0xFF, 0xF1, 0x4C, 0x80, 0x01, 0x1F, 0xFC, 0xAA]);
        }
        let frames = adts_frames(&stream);
        assert_eq!(frames.len(), 2);
        for frame in frames {
            assert_eq!(frame.len(), 8, "the header stays on");
            assert_eq!(frame[0], 0xFF, "and the frame opens with its syncword");
        }
    }

    #[test]
    fn a_truncated_frame_is_not_returned() {
        let stream = [0xFF, 0xF1, 0x4C, 0x80, 0x7F, 0xFF, 0xFC];
        assert!(adts_frames(&stream).is_empty(), "a frame running past the end");
    }
}

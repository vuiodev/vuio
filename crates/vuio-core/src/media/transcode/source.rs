//! Where compressed audio frames come from.
//!
//! Phases 1-3 had one answer: a raw `.ac3`/`.dts` file, framed by walking sync
//! words. That is the rarer shape of the problem. The common one is a film —
//! `Movie.mkv` with an AC-3 or DTS track — where the frames are inside a
//! container and symphonia is what gets them out.
//!
//! Both answers have to satisfy the same contract, because the resource built on
//! top of them does: the decoder must be openable at an arbitrary point in the
//! stream, and it must produce, from there on, the same samples a decode from
//! the beginning would have produced. [`PcmStream`] is that contract — open at a
//! sample, then pull blocks — and the two variants below are the two ways of
//! honouring it.

#[allow(unused_imports)]
use anyhow::{bail, Context, Result};
use std::path::Path;

use super::{FrameIndex, PcmDecoder, TranscodeCodec};

/// Seconds of audio decoded and thrown away before a seek point.
///
/// AC-3 and DTS frames overlap by half a window, so the sample sitting exactly
/// at a seek point is reconstructed partly from state the previous frames
/// carried. Decoding a little run-up and discarding it removes the transient
/// that would otherwise tick at the start of every seek. A quarter of a second
/// is a handful of frames — far more overlap than any of these codecs carries,
/// and still nothing next to the seek it follows.
#[cfg(feature = "demux")]
const PREROLL_SECS: f64 = 0.25;

/// How a file's audio frames are reached.
pub enum PacketSource {
    /// A raw `.ac3`/`.eac3`/`.dts` file, framed by walking sync words.
    ///
    /// Every frame's offset, length and decoded sample count is known before a
    /// byte is decoded, which is what makes the resource exactly as long as its
    /// `Content-Length` claims and seekable to the sample.
    Elementary(FrameIndex),
    /// One audio track inside a container symphonia can demux.
    #[cfg(feature = "demux")]
    Container(ContainerAudio),
}

/// What one container audio track declares about itself.
#[cfg(feature = "demux")]
pub struct ContainerAudio {
    /// The track to demux, as symphonia numbers them.
    pub track_id: u32,
    /// Sample rate declared by the track, in Hz.
    pub sample_rate: u32,
    /// Total decoded sample frames, when the container says enough to know.
    ///
    /// `None` is not a failure — it means the resource degrades to a chunked
    /// body with no `Content-Length` and no seeking, which loses a scrub bar.
    /// Guessing instead would lose the download: a `Content-Length` a renderer
    /// cannot be given is a truncated transfer.
    pub total_samples: Option<u64>,
}

impl PacketSource {
    /// Sample rate of the source, in Hz.
    pub fn sample_rate(&self) -> u32 {
        match self {
            Self::Elementary(index) => index.sample_rate,
            #[cfg(feature = "demux")]
            Self::Container(audio) => audio.sample_rate,
        }
    }

    /// Total decoded sample frames, if they can be known up front.
    pub fn total_samples(&self) -> Option<u64> {
        match self {
            Self::Elementary(index) => Some(index.total_samples),
            #[cfg(feature = "demux")]
            Self::Container(audio) => audio.total_samples,
        }
    }
}

/// A decoded PCM stream, positioned at a sample and pulled block by block.
///
/// Blocking throughout: it reads files and decodes. Callers on an async task run
/// it inside `spawn_blocking`.
pub enum PcmStream {
    Elementary(ElementaryStream),
    #[cfg(feature = "demux")]
    Container(ContainerStream),
}

impl PcmStream {
    /// Open at `start_sample`, with the decoder already warmed on the frames
    /// before it.
    pub fn open(plan: &super::AudioPlan, start_sample: u64) -> Result<Self> {
        match &plan.source {
            PacketSource::Elementary(index) => Ok(Self::Elementary(ElementaryStream::open(
                &plan.source_path,
                plan.codec,
                index,
                plan.channels,
                start_sample,
            )?)),
            #[cfg(feature = "demux")]
            PacketSource::Container(audio) => Ok(Self::Container(ContainerStream::open(
                &plan.source_path,
                plan.codec,
                audio,
                plan.channels,
                start_sample,
            )?)),
        }
    }

    /// The next block of interleaved little-endian S16, or `None` at the end.
    ///
    /// A block is whatever one compressed frame decodes to; blocks are not a
    /// fixed size and callers must not assume one.
    pub fn next_block(&mut self) -> Option<Vec<u8>> {
        match self {
            Self::Elementary(stream) => stream.next_block(),
            #[cfg(feature = "demux")]
            Self::Container(stream) => stream.next_block(),
        }
    }
}

/// The elementary-stream reader: seek to a byte offset, decode forward.
pub struct ElementaryStream {
    source: std::io::BufReader<std::fs::File>,
    decoder: PcmDecoder,
    index: FrameIndex,
    /// Next frame to read, as an index into `index.frames`.
    next: usize,
    /// Frame at which output starts; earlier ones are decoded for state only.
    from: usize,
    /// PCM from the frame the decoder was opened on, not yet handed out.
    primed: Option<Vec<u8>>,
    /// Decoded bytes still to discard from the front of the output.
    skip: usize,
    channels: u16,
}

impl ElementaryStream {
    fn open(
        path: &Path,
        codec: TranscodeCodec,
        index: &FrameIndex,
        channels: u16,
        start_sample: u64,
    ) -> Result<Self> {
        let (from, samples_into_frame) = index.locate(start_sample);
        // Priming already decodes the frame it opens on. Feeding that frame to
        // the decoder a second time would run its samples through the overlap
        // buffer twice, and every frame after it would then land somewhere a
        // sequential decode never goes — so the range would stop being the slice
        // of the whole that it claims to be.
        let preroll = from.saturating_sub(1);

        let file = std::fs::File::open(path)
            .with_context(|| format!("opening {} for transcoding", path.display()))?;
        let mut source = std::io::BufReader::with_capacity(256 * 1024, file);
        let first = index.frames[preroll];
        let mut raw = vec![0u8; first.len as usize];
        read_at(&mut source, first.offset, &mut raw)?;
        let (decoder, primed) =
            PcmDecoder::open(codec, index.sample_rate, Some(channels), &raw)?;

        Ok(Self {
            source,
            decoder,
            index: index.clone(),
            next: preroll,
            from,
            primed: Some(primed),
            skip: samples_into_frame as usize * channels as usize * 2,
            channels,
        })
    }

    fn next_block(&mut self) -> Option<Vec<u8>> {
        loop {
            if self.next >= self.index.frames.len() {
                return None;
            }
            let i = self.next;
            let frame = self.index.frames[i];
            let want = frame.samples as usize * self.channels as usize * 2;
            let pcm = match self.primed.take() {
                Some(mut pcm) => {
                    pcm.resize(want, 0);
                    pcm
                }
                None => {
                    let mut raw = vec![0u8; frame.len as usize];
                    if read_at(&mut self.source, frame.offset, &mut raw).is_err() {
                        return None;
                    }
                    self.decoder.decode_or_silence(&raw, Some(frame.samples))
                }
            };
            self.next += 1;

            // Frames before the seek point were decoded for their state only.
            if i < self.from {
                continue;
            }
            if self.skip >= pcm.len() {
                self.skip -= pcm.len();
                continue;
            }
            let out = pcm[self.skip..].to_vec();
            self.skip = 0;
            return Some(out);
        }
    }
}

fn read_at<R: std::io::Read + std::io::Seek>(
    source: &mut R,
    offset: u64,
    into: &mut [u8],
) -> std::io::Result<()> {
    source.seek(std::io::SeekFrom::Start(offset))?;
    source.read_exact(into)
}

/// The container reader: seek symphonia by time, decode forward.
///
/// A container track has no byte index to divide into — its frames are
/// interleaved with video and scattered across clusters — so the seek is by
/// time and the landing point is refined by counting samples from the timestamp
/// of the first packet after it. That leaves a seek accurate to within the
/// rounding of one packet timestamp rather than exact to the sample, which is
/// inaudible and, importantly, does not affect the resource's length: that comes
/// from the plan, and the streaming half pads or clips to it either way.
#[cfg(feature = "demux")]
pub struct ContainerStream {
    format: Box<dyn symphonia::core::formats::FormatReader>,
    track_id: u32,
    codec: TranscodeCodec,
    decoder: PcmDecoder,
    /// PCM decoded while positioning, not yet handed out.
    pending: Option<Vec<u8>>,
}

#[cfg(feature = "demux")]
impl ContainerStream {
    fn open(
        path: &Path,
        codec: TranscodeCodec,
        audio: &ContainerAudio,
        channels: u16,
        start_sample: u64,
    ) -> Result<Self> {
        use symphonia::core::formats::{SeekMode, SeekTo};
        use symphonia::core::units::Time;

        let mut format = open_format(path)?;
        let time_base = track_time_base(format.as_ref(), audio.track_id);

        let start_secs = start_sample as f64 / f64::from(audio.sample_rate.max(1));
        // Sample zero needs no seek, and asking for one on a track whose first
        // packet is already under the reader would only risk moving it.
        if start_sample > 0 {
            let target = (start_secs - PREROLL_SECS).max(0.0);
            // A reader that cannot seek leaves itself at the start, which is
            // still correct — just slower, because the discard loop below then
            // walks there. The landing point is read back from the first
            // packet's timestamp either way, so a failed seek needs no special
            // case here.
            let _ = format.seek(
                SeekMode::Coarse,
                SeekTo::Time {
                    time: Time::try_from_secs_f64(target).unwrap_or(Time::ZERO),
                    track_id: Some(audio.track_id),
                },
            );
        }

        // Prime on the first packet the reader hands back, and take its
        // timestamp as where we actually landed.
        let (first_packet, first_pts) = next_track_packet(format.as_mut(), audio.track_id)
            .context("reading the first packet of the audio track")?
            .ok_or_else(|| anyhow::anyhow!("the audio track carries no packets"))?;
        let position = if start_sample > 0 {
            sample_at(first_pts, time_base, audio.sample_rate)
        } else {
            0
        };
        let (decoder, primed) =
            PcmDecoder::open(codec, audio.sample_rate, Some(channels), &first_packet)?;

        let mut me = Self {
            format,
            track_id: audio.track_id,
            codec,
            decoder,
            pending: Some(primed),
        };

        // Walk forward to the requested sample, discarding as we go. The blocks
        // dropped here are what warms the decoder's overlap state.
        let stride = channels as usize * 2;
        let mut to_drop = start_sample.saturating_sub(position) as usize * stride;
        while to_drop > 0 {
            let Some(block) = me.next_block() else { break };
            if to_drop >= block.len() {
                to_drop -= block.len();
            } else {
                me.pending = Some(block[to_drop..].to_vec());
                to_drop = 0;
            }
        }
        Ok(me)
    }

    fn next_block(&mut self) -> Option<Vec<u8>> {
        if let Some(pending) = self.pending.take() {
            return Some(pending);
        }
        loop {
            let (data, _) = next_track_packet(self.format.as_mut(), self.track_id).ok()??;
            // The frame's own header says how long it decodes to, which is what
            // lets a corrupt frame inside a film cost its own duration in
            // silence rather than shortening the track and shifting everything
            // after it out of sync with the picture.
            let expect = super::frames::frame_samples(self.codec, &data);
            let pcm = self.decoder.decode_or_silence(&data, expect);
            if !pcm.is_empty() {
                return Some(pcm);
            }
            // A packet that produced nothing is not the end of the track; keep
            // pulling until the reader itself runs out.
        }
    }
}

/// Open `path` with symphonia, hinting the extension.
#[cfg(feature = "demux")]
pub(crate) fn open_format(
    path: &Path,
) -> Result<Box<dyn symphonia::core::formats::FormatReader>> {
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;

    let file = std::fs::File::open(path)
        .with_context(|| format!("opening {} for transcoding", path.display()))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }
    symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .with_context(|| format!("probing {}", path.display()))
}

#[cfg(feature = "demux")]
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

/// The next packet belonging to `track_id`, with its presentation timestamp.
#[cfg(feature = "demux")]
pub(crate) fn next_track_packet(
    format: &mut dyn symphonia::core::formats::FormatReader,
    track_id: u32,
) -> Result<Option<(Vec<u8>, i64)>> {
    loop {
        match format.next_packet() {
            Ok(Some(packet)) => {
                if packet.track_id != track_id {
                    continue;
                }
                return Ok(Some((packet.data.to_vec(), packet.pts.get())));
            }
            Ok(None) => return Ok(None),
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                return Ok(None)
            }
            Err(e) => return Err(anyhow::anyhow!("demuxing the audio track: {e}")),
        }
    }
}

/// Which decoded sample a packet timestamp lands on.
#[cfg(feature = "demux")]
fn sample_at(
    pts: i64,
    time_base: Option<symphonia::core::units::TimeBase>,
    sample_rate: u32,
) -> u64 {
    use symphonia::core::units::Timestamp;

    let Some(time_base) = time_base else {
        return 0;
    };
    time_base
        .calc_time(Timestamp::new(pts.max(0)))
        .map(|time| (time.as_secs_f64() * f64::from(sample_rate)).round() as u64)
        .unwrap_or(0)
}

/// Find the audio track to transcode, and describe it.
///
/// Multi-audio films pick the track the container marks DEFAULT, then the first
/// audio track. Deliberately not clever about language: a wrong guess is worse
/// than a predictable one, and the fix if it turns out to matter is one resource
/// per track rather than a better guess.
#[cfg(feature = "demux")]
pub(crate) fn probe_container_audio(
    path: &Path,
) -> Result<(Box<dyn symphonia::core::formats::FormatReader>, ContainerAudio, TranscodeCodec)> {
    use symphonia::core::formats::TrackFlags;

    let format = open_format(path)?;
    let audio_tracks: Vec<_> = format
        .tracks()
        .iter()
        .filter(|t| {
            t.codec_params
                .as_ref()
                .is_some_and(|params| params.audio().is_some())
        })
        .collect();
    let track = audio_tracks
        .iter()
        .find(|t| t.flags.contains(TrackFlags::DEFAULT))
        .or_else(|| audio_tracks.first())
        .ok_or_else(|| anyhow::anyhow!("{} has no audio track", path.display()))?;

    let params = track.codec_params.as_ref().and_then(|p| p.audio()).unwrap();
    let Some(codec) = codec_of(params.codec) else {
        bail!(
            "the audio track of {} is not one this server decodes",
            path.display()
        );
    };
    let Some(sample_rate) = params.sample_rate.filter(|rate| *rate > 0) else {
        bail!("the audio track of {} declares no sample rate", path.display());
    };

    // `num_frames` is exact where a container carries it (MP4's `stts` does).
    // Matroska usually does not, and its Segment duration is the next best
    // thing: it is what every player already trusts for the scrub bar, and the
    // streaming half pads or clips to whatever length is settled on here, so a
    // few milliseconds of disagreement cannot truncate a transfer.
    //
    // Not done here: a demux-only counting pass. It would be exact, and it
    // would read the whole film — twenty gigabytes of I/O — to answer a `HEAD`.
    let total_samples = track
        .num_frames
        .or_else(|| {
            let time_base = track.time_base?;
            let duration = track.duration?;
            time_base
                .calc_duration(duration)
                .map(|time| (time.as_secs_f64() * f64::from(sample_rate)).round() as u64)
        })
        .or_else(|| {
            let media_info = format.media_info();
            let time_base = media_info.time_base?;
            let duration = media_info.duration?;
            time_base
                .calc_duration(duration)
                .map(|time| (time.as_secs_f64() * f64::from(sample_rate)).round() as u64)
        })
        .filter(|samples| *samples > 0);

    let audio = ContainerAudio {
        track_id: track.id,
        sample_rate,
        total_samples,
    };
    drop(audio_tracks);
    Ok((format, audio, codec))
}

/// Which of the three a symphonia audio codec id is, if any.
#[cfg(feature = "demux")]
pub(crate) fn codec_of(
    codec: symphonia::core::codecs::audio::AudioCodecId,
) -> Option<TranscodeCodec> {
    use symphonia::core::codecs::audio::well_known::{CODEC_ID_AC3, CODEC_ID_DCA, CODEC_ID_EAC3};

    match codec {
        CODEC_ID_AC3 => Some(TranscodeCodec::Ac3),
        CODEC_ID_EAC3 => Some(TranscodeCodec::Eac3),
        CODEC_ID_DCA => Some(TranscodeCodec::Dts),
        _ => None,
    }
}

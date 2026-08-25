//! A film, remuxed on the fly, with its audio decoded on the way past.
//!
//! The deliverable of phase 4. A television that cannot decode AC-3 or DTS shows
//! the picture and produces no sound; what it is offered instead is this — the
//! same file, in fragmented MP4, with the video track copied out bit for bit and
//! every audio track decoded and re-encoded as AAC. The picture is never
//! touched, which is what keeps the CPU cost proportional to the soundtracks
//! rather than to the film.
//!
//! Every soundtrack, not the one we guessed at. A television switches audio
//! track inside its own demuxer, on bytes it already holds — no second request
//! is made and nothing about the switch reaches this server, so a track it can
//! be switched to has to be in the body before it asks. Carrying them all is
//! what makes the audio button work; the alternative is guessing which one the
//! viewer wanted and being wrong for every film with a commentary. It is
//! affordable because the picture is passthrough either way: one decode and
//! re-encode chain measures at about a hundredth of a core, so a film with four
//! soundtracks costs four hundredths rather than twice anything.
//!
//! Nothing about the output is written twice or seeked back into, because a
//! fragmented MP4 has no index to fix up at the end: an init segment describing
//! both tracks, then `moof`/`mdat` pairs forever. That is what lets it go
//! straight down an HTTP body of unknown length.
//!
//! Seeking is by time, not by byte. Byte-seeking would need the output's length
//! and layout known before it exists, and the audio half of it is a lossy
//! re-encode whose frame sizes are not predictable from anything — so a
//! byte offset cannot be turned into a position in a film without producing the
//! film first. A time seek needs none of that: it is a new response, built from
//! the same source at a different point, and the demuxer already seeks by
//! timestamp. See `crate::web::video_streaming` for the DLNA side of that.

use anyhow::Result;
use std::path::Path;

use super::{AacEncoder, PcmDecoder, TranscodeCodec};
use crate::media::remux::{
    packet_is_keyframe, rescale_ticks, Fmp4Writer, MediaPacket, TrackCodec, TrackInfo, TrackKind,
};

/// Seconds of video per movie fragment.
///
/// Short enough that a renderer starts playing promptly and that a dropped
/// connection wastes little work; long enough that the per-fragment box overhead
/// stays negligible against the samples it wraps.
const FRAGMENT_SECS: f64 = 2.0;

/// Timescale for the video track: the MPEG-TS/HLS/DASH convention, and divisible
/// by every common frame rate.
const VIDEO_TIMESCALE: u32 = 90_000;

/// Channels the re-encoded audio track carries.
const DECODED_CHANNELS: u16 = 2;

/// A film being rewritten as it is read.
pub struct ProgressiveStream {
    format: Box<dyn symphonia::core::formats::FormatReader>,
    video: TrackSink,
    /// Every soundtrack being carried, in the order they are written into the
    /// `moov` — which is the order a renderer that takes the first audio track
    /// without asking will read them in, so the caller puts the default first.
    audio: Vec<AudioSink>,
    /// Total length of the film, for `mehd`.
    duration_secs: Option<f64>,
    sequence: u32,
    /// Set once the first video sample has been taken, so audio that predates it
    /// is dropped rather than started early.
    started: bool,
    finished: bool,
}

struct TrackSink {
    track: TrackInfo,
    time_base: Option<symphonia::core::units::TimeBase>,
    pending: Vec<MediaPacket>,
    /// Presentation time of the fragment's first sample, in the track timescale.
    fragment_start: Option<u64>,
    /// Where the next fragment starts if this track contributes nothing to it.
    next_decode_time: u64,
}

struct AudioSink {
    sink: TrackSink,
    /// The container track this consumes. The output track keeps the source's
    /// id — one track in, one track out — so these are the same number; naming
    /// it separately keeps the two roles distinguishable at the call site.
    source_id: u32,
    /// `None` when the source is already AAC and is passed through untouched.
    codec: Option<TranscodeCodec>,
    decode: Option<AudioDecode>,
}

struct AudioDecode {
    codec: TranscodeCodec,
    decoder: PcmDecoder,
    encoder: AacEncoder,
    decoded_channels: u16,
    /// Decode time of the next AAC frame, in samples. `None` until enough of
    /// the track has been seen to say where the run belongs.
    next_dts: Option<u64>,
    /// Each packet's own account of where the run starts. Read once, by
    /// [`super::run_anchor`], and then dropped.
    anchors: Vec<i64>,
    /// Samples decoded so far, which is what each estimate is measured against.
    decoded: u64,
    /// Frames encoded before the run could be placed.
    held: Vec<Vec<u8>>,
}

/// Packets to hear from before placing the run.
///
/// A single Matroska timestamp is only good to a millisecond, and on a track
/// whose frames are not a whole number of them it can be seventy-five out —
/// enough to lose lip-sync for the length of the film, because a progressive
/// stream anchors once and never again. A hundred of them settle it. Two
/// seconds of video accumulate before the first fragment is written, which is
/// more audio packets than this for every codec here, so nothing is delayed by
/// the wait.
const ANCHOR_PACKETS: usize = 96;

impl ProgressiveStream {
    /// Open `path` positioned at `start_secs`, ready to emit fragments.
    ///
    /// `video` and `audio` come from the same probe the caller used to decide
    /// this resource exists at all, so nothing here re-inspects the file. Order
    /// in `audio` is preserved into the output, and a track this build cannot
    /// produce is dropped rather than carried: better a film with two working
    /// soundtracks than one with a third that plays noise.
    pub fn open(
        path: &Path,
        video: &TrackInfo,
        audio: &[TrackInfo],
        start_secs: f64,
        duration_secs: Option<f64>,
    ) -> Result<Self> {
        use symphonia::core::formats::{SeekMode, SeekTo};
        use symphonia::core::units::Time;

        let mut format = super::source::open_format(path)?;

        if start_secs > 0.0 {
            // Coarse, not Accurate: a stream has to open on a random-access
            // point or the renderer has no reference frame to decode the first
            // picture against. Coarse lands on the container's own cue point at
            // or before the requested time, which is a keyframe by construction.
            let _ = format.seek(
                SeekMode::Coarse,
                SeekTo::Time {
                    time: Time::try_from_secs_f64(start_secs).unwrap_or(Time::ZERO),
                    track_id: Some(video.id),
                },
            );
        }

        let video_tb = track_time_base(format.as_ref(), video.id);
        let audio: Vec<AudioSink> = audio
            .iter()
            .filter_map(|track| {
                let codec = track.codec_kind.transcode_codec();
                if codec.is_none() && track.codec_kind != TrackCodec::Aac {
                    return None;
                }
                let audio_tb = track_time_base(format.as_ref(), track.id);
                Some(AudioSink {
                    source_id: track.id,
                    codec,
                    sink: TrackSink::new(aac_track(track), audio_tb),
                    decode: None,
                })
            })
            .collect();

        Ok(Self {
            format,
            video: TrackSink::new(video.clone(), video_tb),
            audio,
            duration_secs,
            sequence: 0,
            started: false,
            finished: false,
        })
    }

    /// `ftyp` + `moov`: the init segment describing every track.
    pub fn init_segment(&self) -> Vec<u8> {
        let mut tracks: Vec<&TrackInfo> = vec![&self.video.track];
        tracks.extend(self.audio.iter().map(|audio| &audio.sink.track));
        let duration_ms = self
            .duration_secs
            .filter(|d| *d > 0.0)
            .map(|d| (d * 1000.0).round() as u64);

        let mut init = Fmp4Writer::build_ftyp();
        init.extend_from_slice(&Fmp4Writer::build_moov_for(&tracks, duration_ms));
        init
    }

    /// The next `moof`+`mdat`, or `None` at the end of the film.
    pub fn next_fragment(&mut self) -> Option<Vec<u8>> {
        if self.finished {
            return None;
        }
        let fragment_ticks = (FRAGMENT_SECS * f64::from(VIDEO_TIMESCALE)).round() as u64;

        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                _ => {
                    // End of the film: flush the encoder's tail and emit whatever
                    // is held, then stop.
                    self.finished = true;
                    self.flush_audio();
                    return self.emit();
                }
            };
            let track_id = packet.track_id;
            let pts = packet.pts.get();
            let data = packet.data.to_vec();

            if track_id == self.video.track.id {
                let ticks = self.video.rescale(pts, VIDEO_TIMESCALE);
                let keyframe = packet_is_keyframe(&data, self.video.track.codec_kind);
                // Nothing before the first random-access point is decodable: it
                // depends on references a renderer starting here will not have.
                if !self.started && !keyframe {
                    continue;
                }
                self.started = true;

                let elapsed = ticks.saturating_sub(*self.video.fragment_start.get_or_insert(ticks));
                if elapsed >= fragment_ticks && !self.video.pending.is_empty() {
                    let fragment = self.emit();
                    self.video.push(ticks, data, true);
                    self.video.fragment_start = Some(ticks);
                    if fragment.is_some() {
                        return fragment;
                    }
                    continue;
                }
                self.video.push(ticks, data, keyframe);
                continue;
            }

            // Audio that predates the first picture is dropped rather than
            // started early: a renderer given sound before it has a frame to show
            // has nothing to synchronise it against.
            if !self.started {
                continue;
            }
            if let Err(error) = self.take_audio(track_id, pts, &data) {
                tracing::debug!(%error, "dropping an audio packet that would not re-encode");
            }
        }
    }

    /// Feed one audio packet to whichever carried track it belongs to.
    ///
    /// A packet from a track that is not being carried — TrueHD, or a codec no
    /// decoder here handles — finds no sink and is dropped, which is the whole
    /// of what "not carried" means.
    fn take_audio(&mut self, track_id: u32, pts: i64, data: &[u8]) -> Result<()> {
        let Some(audio) = self
            .audio
            .iter_mut()
            .find(|audio| audio.source_id == track_id)
        else {
            return Ok(());
        };
        let sample_rate = audio.sink.track.sample_rate.unwrap_or(48_000);
        let ticks = audio.sink.rescale(pts, sample_rate);

        let Some(codec) = audio.codec else {
            // Already AAC. The container's frames are MP4 samples as they stand.
            audio.sink.push(ticks, data.to_vec(), true);
            return Ok(());
        };

        let decode = match audio.decode.as_mut() {
            Some(decode) => decode,
            None => {
                let (decoder, mut primed) =
                    PcmDecoder::open(codec, sample_rate, Some(DECODED_CHANNELS), data)?;
                let decoded_channels = decoder.channels();
                let encoder = AacEncoder::new(sample_rate, DECODED_CHANNELS)?;
                // The probe frame contributes the sample count its own header
                // declared, like every frame after it, so the running position
                // the estimates are measured against stays true.
                if let Some(samples) = super::frames::frame_samples(codec, data) {
                    primed.resize(samples as usize * decoded_channels as usize * 2, 0);
                }
                audio.decode = Some(AudioDecode {
                    codec,
                    decoder,
                    encoder,
                    decoded_channels,
                    next_dts: None,
                    anchors: Vec::new(),
                    decoded: 0,
                    held: Vec::new(),
                });
                let decode = audio.decode.as_mut().unwrap();
                decode.take(&mut audio.sink, ticks, primed)?;
                return Ok(());
            }
        };

        let expect = super::frames::frame_samples(decode.codec, data);
        let pcm = decode.decoder.decode_or_silence(data, expect);
        decode.take(&mut audio.sink, ticks, pcm)?;
        Ok(())
    }

    fn flush_audio(&mut self) {
        for audio in &mut self.audio {
            let Some(decode) = audio.decode.as_mut() else {
                continue;
            };
            let tail = decode.encoder.finish();
            push_aac(&mut audio.sink, decode, &tail, true);
        }
    }

    /// Wrap whatever every track holds into one fragment.
    fn emit(&mut self) -> Option<Vec<u8>> {
        // A fragment about to be written cannot wait for more packets before
        // deciding where each audio run sits, so this is where the decision is
        // forced if it has not been made already. Every track settles its own:
        // they are separate runs off separate decoders and nothing about one
        // says where another belongs.
        for audio in &mut self.audio {
            if let Some(decode) = audio.decode.as_mut() {
                push_aac(&mut audio.sink, decode, &[], true);
            }
        }
        let video_packets = self.video.take();
        let audio_packets: Vec<Vec<MediaPacket>> = self
            .audio
            .iter_mut()
            .map(|audio| audio.sink.take())
            .collect();
        if video_packets.is_empty() && audio_packets.iter().all(Vec::is_empty) {
            return None;
        }

        self.sequence += 1;
        let mut tracks: Vec<(&TrackInfo, &[MediaPacket])> =
            vec![(&self.video.track, &video_packets)];
        let mut fallbacks = vec![self.video.next_decode_time];
        for (audio, packets) in self.audio.iter().zip(&audio_packets) {
            tracks.push((&audio.sink.track, packets));
            fallbacks.push(audio.sink.next_decode_time);
        }
        let fragment = Fmp4Writer::build_multi_track_segment(self.sequence, &tracks, &fallbacks);

        // Remember where each track's timeline reached, so a fragment a track
        // contributes nothing to still declares a sane base decode time.
        self.video.advance(&video_packets);
        for (audio, packets) in self.audio.iter_mut().zip(&audio_packets) {
            audio.sink.advance(packets);
        }
        Some(fragment)
    }
}

/// Hold the AAC frames in `adts`, and hand over everything held once the run's
/// place on the film's timeline is settled.
///
/// `settle` decides that with however many packets have been seen so far,
/// because the caller is about to write a fragment and cannot wait.
fn push_aac(sink: &mut TrackSink, decode: &mut AudioDecode, adts: &[u8], settle: bool) {
    for payload in super::adts_payloads(adts) {
        decode.held.push(payload.to_vec());
    }
    if decode.next_dts.is_none() {
        if !settle && decode.anchors.len() < ANCHOR_PACKETS {
            return;
        }
        if decode.anchors.is_empty() {
            return;
        }
        let mut anchors = std::mem::take(&mut decode.anchors);
        // Placed early by exactly the encoder's delay, which is what a decoder's
        // output trails its input by. Nothing here has to land on a frame
        // boundary — this is one continuous run, not a tile of a grid other
        // requests also write to — so the shift is the measured sample count
        // rather than a rounded number of frames.
        let anchor = super::run_anchor(&mut anchors).saturating_sub(super::ENCODER_DELAY as i64);
        decode.next_dts = Some(anchor.max(0) as u64);
    }
    let mut dts = decode.next_dts.unwrap_or(0);
    for payload in decode.held.drain(..) {
        sink.pending.push(MediaPacket {
            track_id: sink.track.id,
            pts: dts,
            dts,
            duration: super::AAC_FRAME_SAMPLES,
            is_keyframe: true,
            data: payload,
        });
        dts += super::AAC_FRAME_SAMPLES;
    }
    decode.next_dts = Some(dts);
}

impl AudioDecode {
    /// Take one frame's decoded PCM: note where the packet says the run starts,
    /// widen the samples to the output's channel count, and encode them.
    fn take(&mut self, sink: &mut TrackSink, ticks: u64, pcm: Vec<u8>) -> Result<()> {
        self.anchors.push(ticks as i64 - self.decoded as i64);
        self.decoded += (pcm.len() / (self.decoded_channels as usize * 2)) as u64;
        let pcm = super::fit_channels(&pcm, self.decoded_channels, DECODED_CHANNELS);
        let adts = self.encoder.push(&pcm)?;
        push_aac(sink, self, &adts, false);
        Ok(())
    }
}

impl TrackSink {
    fn new(track: TrackInfo, time_base: Option<symphonia::core::units::TimeBase>) -> Self {
        Self {
            track,
            time_base,
            pending: Vec::new(),
            fragment_start: None,
            next_decode_time: 0,
        }
    }

    fn rescale(&self, ticks: i64, output_timescale: u32) -> u64 {
        match self.time_base {
            Some(time_base) => rescale_ticks(ticks, time_base, output_timescale),
            None => ticks.max(0) as u64,
        }
    }

    fn push(&mut self, pts: u64, data: Vec<u8>, is_keyframe: bool) {
        self.pending.push(MediaPacket {
            track_id: self.track.id,
            pts,
            dts: pts,
            duration: 0,
            is_keyframe,
            data,
        });
    }

    fn take(&mut self) -> Vec<MediaPacket> {
        let mut packets = std::mem::take(&mut self.pending);
        // Matroska stores presentation timestamps in decode order and no decode
        // timestamps at all; ISO-BMFF needs the opposite. Sorting this run's
        // presentation timestamps recovers the decode timeline exactly, because
        // each frame is decoded once and presented once.
        crate::media::remux::derive_decode_timestamps(&mut packets);
        packets
    }

    /// Remember where this track's decode timeline reached.
    ///
    /// Only ever read as the base decode time of a fragment this track
    /// contributed nothing to, which happens when one track runs out before the
    /// other. The last sample's own duration is unknown for a passthrough
    /// packet, so the gap to the previous sample stands in for it — which is the
    /// same estimate the fragment writer uses for its final sample.
    fn advance(&mut self, emitted: &[MediaPacket]) {
        let Some(last) = emitted.last() else { return };
        let step = emitted
            .len()
            .checked_sub(2)
            .map(|i| last.dts.saturating_sub(emitted[i].dts))
            .filter(|gap| *gap > 0)
            .or(Some(last.duration).filter(|d| *d > 0))
            .unwrap_or(1);
        self.next_decode_time = last.dts + step;
    }
}

/// The audio track as it will be written into the output.
///
/// A decoded track is restated as the AAC it becomes: an `mp4a` sample entry
/// whose `esds` carries the encoder's own `AudioSpecificConfig`, at the
/// encoder's channel count rather than the source's 5.1. A track that is already
/// AAC keeps everything it had, including the config the container carried.
fn aac_track(track: &TrackInfo) -> TrackInfo {
    let name = track.name.clone().or_else(|| Some(source_label(track)));
    if track.codec_kind == TrackCodec::Aac {
        return TrackInfo {
            name,
            ..track.clone()
        };
    }
    let sample_rate = track.sample_rate.unwrap_or(48_000);
    TrackInfo {
        codec: format!("{} → AAC", track.codec),
        codec_kind: TrackCodec::Aac,
        channels: Some(DECODED_CHANNELS as u8),
        extra_data: super::audio_specific_config(sample_rate, DECODED_CHANNELS),
        track_kind: TrackKind::Audio,
        name,
        ..track.clone()
    }
}

/// What to call this track in a renderer's audio menu.
///
/// Every carried track leaves here as stereo AAC, so naming them after what they
/// have become would print the same three words three times. What tells a film's
/// soundtracks apart is what they arrived as — the main mix in DTS 5.1, the
/// commentary in AC-3 stereo — so that is what the label states. Matroska's own
/// track name would be better still, and is preferred when there is one; there
/// is not, today, because the demuxer this reads from does not surface it.
fn source_label(track: &TrackInfo) -> String {
    let layout = match track.channels {
        Some(1) => "Mono".to_string(),
        Some(2) => "Stereo".to_string(),
        Some(6) => "5.1".to_string(),
        Some(8) => "7.1".to_string(),
        Some(count) => format!("{count}ch"),
        None => return track.codec.clone(),
    };
    format!("{} {layout}", track.codec)
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

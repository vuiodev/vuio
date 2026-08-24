//! A film, remuxed on the fly, with its audio decoded on the way past.
//!
//! The deliverable of phase 4. A television that cannot decode AC-3 or DTS shows
//! the picture and produces no sound; what it is offered instead is this — the
//! same file, in fragmented MP4, with the video track copied out bit for bit and
//! only the audio track decoded and re-encoded as AAC. The picture is never
//! touched, which is what keeps the CPU cost proportional to the soundtrack
//! rather than to the film.
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
    audio: Option<AudioSink>,
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
    /// Decode time of the next AAC frame, in samples.
    next_dts: Option<u64>,
}

impl ProgressiveStream {
    /// Open `path` positioned at `start_secs`, ready to emit fragments.
    ///
    /// `video` and `audio` come from the same probe the caller used to decide
    /// this resource exists at all, so nothing here re-inspects the file.
    pub fn open(
        path: &Path,
        video: &TrackInfo,
        audio: Option<&TrackInfo>,
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
        // An audio track this build cannot produce is no audio track: better a
        // silent film with a picture that plays than a stream carrying samples
        // the renderer will read as noise.
        let audio = audio.and_then(|track| {
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
        });

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

    /// `ftyp` + `moov`: the init segment describing both tracks.
    pub fn init_segment(&self) -> Vec<u8> {
        let mut tracks: Vec<&TrackInfo> = vec![&self.video.track];
        if let Some(audio) = &self.audio {
            tracks.push(&audio.sink.track);
        }
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

    /// Feed one audio packet, if it belongs to the track being carried.
    fn take_audio(&mut self, track_id: u32, pts: i64, data: &[u8]) -> Result<()> {
        let Some(audio) = self.audio.as_mut() else {
            return Ok(());
        };
        if track_id != audio.source_id {
            return Ok(());
        }
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
                let (decoder, primed) =
                    PcmDecoder::open(codec, sample_rate, Some(DECODED_CHANNELS), data)?;
                let decoded_channels = decoder.channels();
                let encoder = AacEncoder::new(sample_rate, DECODED_CHANNELS)?;
                audio.decode = Some(AudioDecode {
                    codec,
                    decoder,
                    encoder,
                    decoded_channels,
                    // One frame early, cancelling the encoder's delay: its MDCT
                    // window spans the previous hop and this one, so a decoder's
                    // output trails its input by exactly one frame.
                    next_dts: Some(ticks.saturating_sub(super::AAC_FRAME_SAMPLES)),
                });
                let decode = audio.decode.as_mut().unwrap();
                let pcm = super::fit_channels(&primed, decoded_channels, DECODED_CHANNELS);
                let adts = decode.encoder.push(&pcm)?;
                push_aac(&mut audio.sink, decode, &adts);
                return Ok(());
            }
        };

        let expect = super::frames::frame_samples(decode.codec, data);
        let pcm = decode.decoder.decode_or_silence(data, expect);
        let pcm = super::fit_channels(&pcm, decode.decoded_channels, DECODED_CHANNELS);
        let adts = decode.encoder.push(&pcm)?;
        push_aac(&mut audio.sink, decode, &adts);
        Ok(())
    }

    fn flush_audio(&mut self) {
        let Some(audio) = self.audio.as_mut() else {
            return;
        };
        let Some(decode) = audio.decode.as_mut() else {
            return;
        };
        let tail = decode.encoder.finish();
        push_aac(&mut audio.sink, decode, &tail);
    }

    /// Wrap whatever both tracks hold into one fragment.
    fn emit(&mut self) -> Option<Vec<u8>> {
        let video_packets = self.video.take();
        let audio_packets = self
            .audio
            .as_mut()
            .map(|audio| audio.sink.take())
            .unwrap_or_default();
        if video_packets.is_empty() && audio_packets.is_empty() {
            return None;
        }

        self.sequence += 1;
        let mut tracks: Vec<(&TrackInfo, &[MediaPacket])> =
            vec![(&self.video.track, &video_packets)];
        let mut fallbacks = vec![self.video.next_decode_time];
        if let Some(audio) = &self.audio {
            tracks.push((&audio.sink.track, &audio_packets));
            fallbacks.push(audio.sink.next_decode_time);
        }
        let fragment = Fmp4Writer::build_multi_track_segment(self.sequence, &tracks, &fallbacks);

        // Remember where each track's timeline reached, so a fragment a track
        // contributes nothing to still declares a sane base decode time.
        self.video.advance(&video_packets);
        if let Some(audio) = self.audio.as_mut() {
            audio.sink.advance(&audio_packets);
        }
        Some(fragment)
    }
}

/// Append the AAC frames in `adts` to the audio track's pending run.
fn push_aac(sink: &mut TrackSink, decode: &mut AudioDecode, adts: &[u8]) {
    for payload in super::adts_payloads(adts) {
        let dts = decode.next_dts.unwrap_or(0);
        sink.pending.push(MediaPacket {
            track_id: sink.track.id,
            pts: dts,
            dts,
            duration: super::AAC_FRAME_SAMPLES,
            is_keyframe: true,
            data: payload.to_vec(),
        });
        decode.next_dts = Some(dts + super::AAC_FRAME_SAMPLES);
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
    if track.codec_kind == TrackCodec::Aac {
        return track.clone();
    }
    let sample_rate = track.sample_rate.unwrap_or(48_000);
    TrackInfo {
        codec: format!("{} → AAC", track.codec),
        codec_kind: TrackCodec::Aac,
        channels: Some(DECODED_CHANNELS as u8),
        extra_data: super::audio_specific_config(sample_rate, DECODED_CHANNELS),
        track_kind: TrackKind::Audio,
        ..track.clone()
    }
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

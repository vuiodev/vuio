//! `oxideav_core::Encoder` wiring for the AAC-LC encoder.
//!
//! Adapts [`crate::encoder::StreamEncoder`] (PCM → ADTS, see the
//! `encoder` module for the §4.5/§4.6 analysis chain) into the
//! framework's frame-in / packet-out [`oxideav_core::Encoder`] trait so
//! pipelines and muxers can drive AAC encoding via the registry.
//!
//! ## Trait-API adaptation
//!
//! * [`send_frame`](Encoder::send_frame) accepts [`Frame::Audio`]
//!   frames carrying **interleaved little-endian `i16`**
//!   (`SampleFormat::S16`, one data plane) at any per-frame sample
//!   count. Samples buffer internally; every completed 1024-sample
//!   hop becomes one ADTS frame.
//! * [`receive_packet`](Encoder::receive_packet) returns one
//!   [`Packet`] per encoded ADTS frame ([`Error::NeedMore`] while the
//!   buffer holds less than a hop). `pts` counts input samples
//!   (time base `1/sample_rate`); the packet's `duration` is 1024.
//!   Every AAC frame is independently decodable after the previous
//!   frame's overlap, and each packet is flagged as a keyframe (the
//!   ADTS stream is random-access at any frame boundary after a
//!   1-frame warmup).
//! * [`flush`](Encoder::flush) zero-pads the pending partial hop (if
//!   any) into a final content frame and appends the encoder's
//!   overlap-flush frame; subsequent `receive_packet` calls drain
//!   those then return [`Error::Eof`].
//!
//! ## Registration
//!
//! [`crate::codec_decoder::register_codecs`] installs
//! [`make_encoder`] alongside the decoder under codec id `"aac"`.
//! The historical direct factory path is also re-exported as
//! [`crate::encoder::make_encoder`].

use std::collections::VecDeque;

use oxideav_core::{
    CodecId, CodecParameters, Encoder, Error, Frame, Packet, Result, SampleFormat, TimeBase,
};

use crate::codec_decoder::CODEC_ID_STR;
use crate::encoder::{EncoderConfig, StreamEncoder, FRAME_LEN};

/// Default target bitrate (bits/second) when `params.bit_rate` is
/// absent: 64 kbps per channel, the conventional "good quality"
/// AAC-LC operating point.
pub const DEFAULT_BITRATE_PER_CHANNEL: u32 = 64_000;

/// Build a boxed AAC [`Encoder`] from `params`.
///
/// Honoured parameters:
///
/// * `sample_rate` (default 44 100) — must be an ISO/IEC 14496-3
///   Table 1.18 rate with a §4.5.4 long-window band table
///   (96 000 … 8 000 Hz).
/// * `channels` (default 2) — any count with a Table 1.19 default
///   `channelConfiguration`: 1, 2, 3, 4, 5, 6 (5.1) or 8 (7.1);
///   input interleaved in the canonical [`crate::channel_map`]
///   order the decoder emits. 7 has no default configuration and is
///   rejected.
/// * `bit_rate` (default 64 kbps × channels) — the rate-loop target.
/// * `sample_format` — must be [`SampleFormat::S16`] (or unset).
///
/// Anything unsupported surfaces as [`Error::Unsupported`] /
/// [`Error::invalid`] at construction time, per the registry's
/// init-time-fallback contract.
pub fn make_encoder(params: &CodecParameters) -> Result<Box<dyn Encoder>> {
    let sample_rate = params.sample_rate.unwrap_or(44_100);
    let channels = params.channels.unwrap_or(2);
    if let Some(fmt) = params.sample_format {
        if fmt != SampleFormat::S16 {
            return Err(Error::unsupported(
                "oxideav-aac encoder accepts interleaved S16 input only",
            ));
        }
    }
    if !(1..=6).contains(&channels) && channels != 8 {
        return Err(Error::unsupported(
            "oxideav-aac encoder supports the Table 1.19 default channel \
             configurations: 1-6 or 8 channels",
        ));
    }
    let bitrate = params
        .bit_rate
        .map(|b| b.min(u64::from(u32::MAX)) as u32)
        .unwrap_or(DEFAULT_BITRATE_PER_CHANNEL * u32::from(channels));
    let config = EncoderConfig {
        sample_rate,
        channels: channels as u8,
        bitrate,
    };
    let stream = StreamEncoder::new(config)
        .map_err(|e| Error::invalid(format!("oxideav-aac encoder config: {e}")))?;

    let mut out_params = CodecParameters::audio(CodecId::new(CODEC_ID_STR));
    out_params.sample_rate = Some(sample_rate);
    out_params.channels = Some(channels);
    out_params.sample_format = Some(SampleFormat::S16);
    out_params.bit_rate = Some(u64::from(bitrate));

    Ok(Box::new(AacEncoder {
        codec_id: CodecId::new(CODEC_ID_STR),
        out_params,
        stream,
        time_base: TimeBase::new(1, i64::from(sample_rate)),
        pending_pcm: Vec::new(),
        packets: VecDeque::new(),
        samples_emitted: 0,
        flushed: false,
    }))
}

/// Frame-to-packet adaptor wrapping [`StreamEncoder`] in the
/// framework [`Encoder`] trait.
struct AacEncoder {
    codec_id: CodecId,
    out_params: CodecParameters,
    stream: StreamEncoder,
    time_base: TimeBase,
    /// Interleaved samples not yet forming a whole 1024-sample hop.
    pending_pcm: Vec<i16>,
    /// Encoded ADTS frames awaiting `receive_packet`.
    packets: VecDeque<Packet>,
    /// Per-channel input samples consumed into emitted packets —
    /// drives `pts`.
    samples_emitted: i64,
    flushed: bool,
}

impl AacEncoder {
    /// Encode every complete hop sitting in `pending_pcm`.
    fn drain_hops(&mut self) -> Result<()> {
        let ch = usize::from(self.out_params.channels.unwrap_or(1)).max(1);
        let hop = FRAME_LEN * ch;
        while self.pending_pcm.len() >= hop {
            let chunk: Vec<i16> = self.pending_pcm.drain(..hop).collect();
            let bytes = self
                .stream
                .encode_frame(&chunk)
                .map_err(|e| Error::invalid(format!("oxideav-aac encode: {e}")))?;
            self.push_packet(bytes);
        }
        Ok(())
    }

    fn push_packet(&mut self, bytes: Vec<u8>) {
        let pkt = Packet::new(0, self.time_base, bytes)
            .with_pts(self.samples_emitted)
            .with_duration(FRAME_LEN as i64)
            .with_keyframe(true);
        self.samples_emitted += FRAME_LEN as i64;
        self.packets.push_back(pkt);
    }
}

impl Encoder for AacEncoder {
    fn codec_id(&self) -> &CodecId {
        &self.codec_id
    }

    fn output_params(&self) -> &CodecParameters {
        &self.out_params
    }

    fn send_frame(&mut self, frame: &Frame) -> Result<()> {
        if self.flushed {
            return Err(Error::invalid("send_frame after flush"));
        }
        let audio = match frame {
            Frame::Audio(a) => a,
            _ => return Err(Error::invalid("oxideav-aac encoder accepts audio frames")),
        };
        let plane = match audio.data.as_slice() {
            [p] => p,
            _ => {
                return Err(Error::invalid(
                    "oxideav-aac encoder expects one interleaved S16 plane",
                ))
            }
        };
        if plane.len() % 2 != 0 {
            return Err(Error::invalid("odd byte count in S16 plane"));
        }
        self.pending_pcm.extend(
            plane
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]])),
        );
        self.drain_hops()
    }

    fn receive_packet(&mut self) -> Result<Packet> {
        if let Some(pkt) = self.packets.pop_front() {
            return Ok(pkt);
        }
        if self.flushed {
            Err(Error::Eof)
        } else {
            Err(Error::NeedMore)
        }
    }

    fn flush(&mut self) -> Result<()> {
        if self.flushed {
            return Ok(());
        }
        // Zero-pad any partial hop into a final content frame…
        if !self.pending_pcm.is_empty() {
            let chunk: Vec<i16> = std::mem::take(&mut self.pending_pcm);
            let bytes = self
                .stream
                .encode_frame(&chunk)
                .map_err(|e| Error::invalid(format!("oxideav-aac encode: {e}")))?;
            self.push_packet(bytes);
        }
        // …then emit the overlap-flush frame.
        let bytes = self
            .stream
            .finish()
            .map_err(|e| Error::invalid(format!("oxideav-aac flush: {e}")))?;
        self.push_packet(bytes);
        self.flushed = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxideav_core::AudioFrame;

    fn params(rate: u32, channels: u16, bitrate: Option<u64>) -> CodecParameters {
        let mut p = CodecParameters::audio(CodecId::new(CODEC_ID_STR));
        p.sample_rate = Some(rate);
        p.channels = Some(channels);
        p.sample_format = Some(SampleFormat::S16);
        p.bit_rate = bitrate;
        p
    }

    fn tone_frame(samples: usize, channels: usize) -> Frame {
        let mut bytes = Vec::with_capacity(samples * channels * 2);
        for i in 0..samples {
            let v = (8000.0 * (0.05 * i as f64).sin()) as i16;
            for _ in 0..channels {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        Frame::Audio(AudioFrame {
            samples: samples as u32,
            pts: None,
            data: vec![bytes],
        })
    }

    #[test]
    fn encoder_builds_and_reports_output_params() {
        let enc = make_encoder(&params(44_100, 2, Some(128_000))).expect("builds");
        assert_eq!(enc.codec_id().as_str(), "aac");
        let out = enc.output_params();
        assert_eq!(out.sample_rate, Some(44_100));
        assert_eq!(out.channels, Some(2));
        assert_eq!(out.bit_rate, Some(128_000));
    }

    #[test]
    fn encoder_rejects_unsupported_shapes() {
        // 7 channels has no Table 1.19 default configuration; 6
        // (5.1) and 8 (7.1) do and build.
        assert!(make_encoder(&params(44_100, 7, None)).is_err());
        assert!(make_encoder(&params(44_100, 9, None)).is_err());
        assert!(make_encoder(&params(44_100, 6, None)).is_ok());
        assert!(make_encoder(&params(44_100, 8, None)).is_ok());
        assert!(make_encoder(&params(44_055, 1, None)).is_err());
        let mut p = params(44_100, 2, None);
        p.sample_format = Some(SampleFormat::F32);
        assert!(make_encoder(&p).is_err());
    }

    #[test]
    fn frames_in_packets_out_with_flush() {
        let mut enc = make_encoder(&params(44_100, 1, Some(96_000))).unwrap();
        // 2.5 hops of input.
        enc.send_frame(&tone_frame(2_560, 1)).unwrap();
        // Two whole hops → two packets.
        let p0 = enc.receive_packet().unwrap();
        assert_eq!(p0.pts, Some(0));
        assert_eq!(p0.duration, Some(1024));
        assert!(p0.flags.keyframe);
        assert!(p0.data.starts_with(&[0xFF]));
        let p1 = enc.receive_packet().unwrap();
        assert_eq!(p1.pts, Some(1024));
        assert!(matches!(enc.receive_packet(), Err(Error::NeedMore)));
        // Flush: the padded half hop + the overlap-flush frame.
        enc.flush().unwrap();
        let p2 = enc.receive_packet().unwrap();
        assert_eq!(p2.pts, Some(2048));
        let p3 = enc.receive_packet().unwrap();
        assert_eq!(p3.pts, Some(3072));
        assert!(matches!(enc.receive_packet(), Err(Error::Eof)));
    }

    #[test]
    fn registry_round_trip_decodes_encoder_output() {
        let mut enc = make_encoder(&params(44_100, 1, Some(128_000))).unwrap();
        let n = 4 * FRAME_LEN;
        enc.send_frame(&tone_frame(n, 1)).unwrap();
        enc.flush().unwrap();
        let mut stream_bytes = Vec::new();
        loop {
            match enc.receive_packet() {
                Ok(p) => stream_bytes.extend_from_slice(&p.data),
                Err(Error::Eof) => break,
                Err(e) => panic!("unexpected: {e}"),
            }
        }

        // Feed the whole ADTS stream to the registered decoder.
        let mut dec = crate::codec_decoder::make_decoder(&params(44_100, 1, None)).unwrap();
        let pkt = Packet::new(0, TimeBase::new(1, 44_100), stream_bytes);
        dec.send_packet(&pkt).unwrap();
        let mut decoded_samples = 0usize;
        loop {
            match dec.receive_frame() {
                Ok(Frame::Audio(a)) => decoded_samples += a.samples as usize,
                Ok(_) => panic!("non-audio frame"),
                Err(_) => break,
            }
        }
        // n/1024 content frames + 1 flush frame, 1024 samples each.
        assert_eq!(decoded_samples, n + FRAME_LEN);
    }
}

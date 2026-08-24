//! Re-encoding decoded PCM as AAC-LC in ADTS framing.
//!
//! The alternative to LPCM, for a network where 1.5 Mbps of uncompressed audio
//! is not free. It is a lossy re-encode of an already-lossy source, which is why
//! it is not the default, and its output size is not known before it is produced,
//! which is why the resource it serves carries no `Content-Length` and cannot be
//! scrubbed. Those are real costs; the config key exists so an operator who is
//! paying them is choosing to.
//!
//! ADTS rather than a container: every frame carries its own header, so the
//! stream is self-describing from any point and needs no muxer, no seek table
//! and no rewrite at the end.

use anyhow::{Context, Result};

/// One AAC-LC encoder bound to a stream's shape.
pub struct AacEncoder {
    inner: Box<dyn oxideav_core::Encoder>,
    channels: u16,
    sample_rate: u32,
}

impl AacEncoder {
    /// Build an encoder producing `channels` at `sample_rate`.
    ///
    /// The bitrate is the vendored encoder's default of 64 kbps per channel —
    /// the conventional AAC-LC "good quality" operating point, and around a
    /// tenth of the LPCM the same audio would cost. Left unconfigurable
    /// deliberately: it is one more knob whose wrong setting is audible, and
    /// nothing about this path benefits from tuning it.
    pub fn new(sample_rate: u32, channels: u16) -> Result<Self> {
        use oxideav_core::{CodecId, CodecParameters, SampleFormat};

        let mut params = CodecParameters::audio(CodecId::new("aac"));
        params.sample_rate = Some(sample_rate);
        params.channels = Some(channels);
        params.sample_format = Some(SampleFormat::S16);

        let inner = oxideav_aac::codec_encoder::make_encoder(&params)
            .map_err(|e| anyhow::anyhow!("AAC encoder: {e}"))
            .context("configuring the AAC encoder")?;

        Ok(Self {
            inner,
            channels,
            sample_rate,
        })
    }

    /// Feed interleaved S16 and collect whatever ADTS frames come out.
    ///
    /// The encoder buffers to its own 1024-sample frame length, so a call may
    /// well produce nothing; that is normal, not an error.
    pub fn push(&mut self, pcm: &[u8]) -> Result<Vec<u8>> {
        use oxideav_core::{AudioFrame, Frame};

        let samples = pcm.len() / (self.channels as usize * 2);
        if samples == 0 {
            return Ok(Vec::new());
        }
        let frame = Frame::Audio(AudioFrame {
            samples: samples as u32,
            pts: None,
            data: vec![pcm.to_vec()],
        });
        self.inner
            .send_frame(&frame)
            .map_err(|e| anyhow::anyhow!("AAC encode: {e}"))?;
        Ok(self.drain())
    }

    /// Flush the encoder's lookahead and overlap, ending the stream cleanly.
    pub fn finish(&mut self) -> Vec<u8> {
        if self.inner.flush().is_err() {
            return Vec::new();
        }
        self.drain()
    }

    /// Sample rate the encoder was configured for.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn drain(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        // `receive_packet` returns `NeedMore` once drained, which is the normal
        // exit rather than a failure.
        while let Ok(packet) = self.inner.receive_packet() {
            out.extend_from_slice(&packet.data);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A second of 440 Hz stereo, interleaved S16 — the encoder's input shape.
    fn sine(seconds: u32, sample_rate: u32) -> Vec<u8> {
        let total = seconds * sample_rate;
        let mut pcm = Vec::with_capacity(total as usize * 4);
        for n in 0..total {
            let t = n as f64 / sample_rate as f64;
            let v = ((t * 440.0 * std::f64::consts::TAU).sin() * 12_000.0) as i16;
            pcm.extend_from_slice(&v.to_le_bytes());
            pcm.extend_from_slice(&v.to_le_bytes());
        }
        pcm
    }

    #[test]
    fn encodes_pcm_into_adts_frames() {
        let mut enc = AacEncoder::new(48_000, 2).unwrap();
        let mut out = enc.push(&sine(1, 48_000)).unwrap();
        out.extend_from_slice(&enc.finish());

        assert!(!out.is_empty(), "a second of audio must produce frames");
        // Every ADTS frame opens with the 12-bit syncword 0xFFF.
        assert_eq!(out[0], 0xFF, "ADTS syncword high byte");
        assert_eq!(out[1] & 0xF0, 0xF0, "ADTS syncword low nibble");
    }

    #[test]
    fn the_result_is_far_smaller_than_the_pcm_it_came_from() {
        let pcm = sine(1, 48_000);
        let mut enc = AacEncoder::new(48_000, 2).unwrap();
        let mut out = enc.push(&pcm).unwrap();
        out.extend_from_slice(&enc.finish());
        // The whole reason to offer this format at all.
        assert!(
            out.len() * 4 < pcm.len(),
            "AAC {} bytes vs PCM {} bytes — expected a large saving",
            out.len(),
            pcm.len()
        );
    }

    #[test]
    fn an_empty_push_is_not_an_error() {
        let mut enc = AacEncoder::new(48_000, 2).unwrap();
        assert!(enc.push(&[]).unwrap().is_empty());
    }

    #[test]
    fn a_channel_count_the_encoder_cannot_express_is_refused_at_construction() {
        // Seven channels has no Table 1.19 default configuration.
        assert!(AacEncoder::new(48_000, 7).is_err());
    }
}

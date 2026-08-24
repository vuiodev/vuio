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
    inner: xaac_rs::Encoder,
    frame_bytes: usize,
    buffer: Vec<u8>,
    channels: u16,
    sample_rate: u32,
}

impl AacEncoder {
    /// Build an encoder producing `channels` at `sample_rate`.
    ///
    /// The bitrate is 64 kbps per channel — the conventional AAC-LC "good quality"
    /// operating point, and around a tenth of the LPCM the same audio would cost.
    pub fn new(sample_rate: u32, channels: u16) -> Result<Self> {
        if !matches!(channels, 1 | 2 | 3 | 4 | 5 | 6 | 8) {
            anyhow::bail!("unsupported channel count for AAC: {channels}");
        }

        let bitrate = match channels {
            1 => 96_000,
            2 => 192_000,
            6 => 384_000,
            8 => 512_000,
            _ => 96_000 * u32::from(channels),
        };

        let config = xaac_rs::EncoderConfig {
            profile: xaac_rs::Profile::AacLc,
            sample_rate,
            channels,
            bitrate,
            output_format: xaac_rs::OutputFormat::Adts,
            ..Default::default()
        };

        let inner = xaac_rs::Encoder::new(config)
            .map_err(|e| anyhow::anyhow!("AAC encoder: {e:?}"))
            .context("configuring the AAC encoder")?;
        let frame_bytes = inner.input_frame_bytes();

        Ok(Self {
            inner,
            frame_bytes,
            buffer: Vec::with_capacity(frame_bytes * 2),
            channels,
            sample_rate,
        })
    }

    /// Feed interleaved S16 and collect whatever ADTS frames come out.
    ///
    /// The encoder buffers to its own frame length (typically 1024 samples per channel),
    /// so a call may well produce nothing; that is normal, not an error.
    pub fn push(&mut self, pcm: &[u8]) -> Result<Vec<u8>> {
        self.buffer.extend_from_slice(pcm);
        let mut out = Vec::new();
        while self.buffer.len() >= self.frame_bytes {
            let chunk: Vec<u8> = self.buffer.drain(..self.frame_bytes).collect();
            let encoded = self
                .inner
                .encode_pcm_bytes(&chunk)
                .map_err(|e| anyhow::anyhow!("AAC encode: {e:?}"))?;
            out.extend_from_slice(&encoded.data);
        }
        Ok(out)
    }

    /// Flush the encoder's lookahead and trailing buffer, ending the stream cleanly.
    pub fn finish(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        if !self.buffer.is_empty() {
            let chunk = std::mem::take(&mut self.buffer);
            if let Ok(encoded) = self.inner.encode_pcm_bytes_with_padding(&chunk) {
                out.extend_from_slice(&encoded.packet.data);
            }
        }
        out
    }

    /// Sample rate the encoder was configured for.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Number of channels the encoder was configured for.
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

/// ISO/IEC 14496-3 Table 1.18 sampling frequency indices.
const SAMPLING_FREQUENCIES: [u32; 13] = [
    96_000, 88_200, 64_000, 48_000, 44_100, 32_000, 24_000, 22_050, 16_000, 12_000, 11_025, 8_000,
    7_350,
];

/// The raw `AudioSpecificConfig` for AAC-LC at this shape.
///
/// An ADTS stream needs none of this — every frame carries its own header — but
/// an MP4 does: the `esds` box in the init segment is where a player learns the
/// stream's rate and channel layout, and it is written *before* a single frame
/// has been encoded. So it is derived from the shape the encoder was configured
/// with rather than read back out of its output. `asc_matches_the_encoders_own_adts_header`
/// is what keeps the two from drifting.
///
/// A rate outside Table 1.18 uses the escape index (15) and an explicit 24-bit
/// rate, which makes the config four bytes instead of two.
pub fn audio_specific_config(sample_rate: u32, channels: u16) -> Vec<u8> {
    const AAC_LC: u32 = 2;
    let channel_configuration = u32::from(channels).min(7);

    let mut bits: Vec<(u32, u32)> = vec![(AAC_LC, 5)];
    match SAMPLING_FREQUENCIES.iter().position(|r| *r == sample_rate) {
        Some(index) => bits.push((index as u32, 4)),
        None => {
            bits.push((0x0F, 4));
            bits.push((sample_rate, 24));
        }
    }
    bits.push((channel_configuration, 4));
    // GASpecificConfig: frameLengthFlag = 0 (1024 samples), dependsOnCoreCoder
    // = 0, extensionFlag = 0.
    bits.push((0, 3));

    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut used = 0u32;
    for (value, width) in bits {
        for i in (0..width).rev() {
            acc = (acc << 1) | ((value >> i) & 1);
            used += 1;
            if used == 8 {
                out.push(acc as u8);
                acc = 0;
                used = 0;
            }
        }
    }
    if used > 0 {
        out.push((acc << (8 - used)) as u8);
    }
    out
}

/// The payloads of an ADTS stream, with each frame's header removed.
///
/// ADTS framing is what makes the encoder's output self-describing over a bare
/// socket, and exactly what an MP4 sample must not contain: the `stsd` entry
/// already says everything the header repeats, and a decoder handed the header
/// as sample data reads it as spectral coefficients.
pub fn adts_payloads(stream: &[u8]) -> Vec<&[u8]> {
    let mut frames = Vec::new();
    let mut pos = 0usize;
    while pos + 7 <= stream.len() {
        let header = &stream[pos..];
        if header[0] != 0xFF || (header[1] & 0xF0) != 0xF0 {
            // Not a syncword. The encoder does not produce these, so rather than
            // resynchronising, stop: a stream that has gone wrong here would
            // produce garbage samples, not merely a lost frame.
            break;
        }
        let frame_len = ((u32::from(header[3]) & 0x03) << 11)
            | (u32::from(header[4]) << 3)
            | (u32::from(header[5]) >> 5);
        let frame_len = frame_len as usize;
        // `protection_absent == 0` adds a two-byte CRC after the fixed header.
        let header_len = if header[1] & 0x01 == 0 { 9 } else { 7 };
        if frame_len < header_len || pos + frame_len > stream.len() {
            break;
        }
        frames.push(&stream[pos + header_len..pos + frame_len]);
        pos += frame_len;
    }
    frames
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

    /// The init segment's `esds` and the segments' samples come from different
    /// requests and different code paths, and a player that disagrees with one
    /// of them produces noise rather than an error. This is the check that they
    /// describe the same stream: the config written into the container has to
    /// match what the encoder itself declares in every ADTS header it emits.
    #[test]
    fn asc_matches_the_encoders_own_adts_header() {
        for (rate, channels) in [(48_000u32, 2u16), (44_100, 2), (32_000, 1)] {
            let mut enc = AacEncoder::new(rate, channels).unwrap();
            let mut out = enc.push(&sine(1, rate)).unwrap();
            out.extend_from_slice(&enc.finish());

            let profile = (out[2] >> 6) & 0x03;
            let frequency_index = (out[2] >> 2) & 0x0F;
            let channel_configuration = ((out[2] & 0x01) << 2) | (out[3] >> 6);

            let asc = audio_specific_config(rate, channels);
            assert_eq!(
                asc[0] >> 3,
                profile + 1,
                "audioObjectType at {rate} Hz: ASC vs ADTS profile"
            );
            assert_eq!(
                ((asc[0] & 0x07) << 1) | (asc[1] >> 7),
                frequency_index,
                "samplingFrequencyIndex at {rate} Hz"
            );
            assert_eq!(
                (asc[1] >> 3) & 0x0F,
                channel_configuration,
                "channelConfiguration at {channels} channels"
            );
        }
    }

    #[test]
    fn adts_framing_is_stripped_leaving_the_payloads() {
        let mut enc = AacEncoder::new(48_000, 2).unwrap();
        let mut stream = enc.push(&sine(1, 48_000)).unwrap();
        stream.extend_from_slice(&enc.finish());

        let payloads = adts_payloads(&stream);
        assert!(payloads.len() > 40, "a second is ~47 frames of 1024 samples");
        let total: usize = payloads.iter().map(|f| f.len()).sum();
        assert_eq!(
            total + payloads.len() * 7,
            stream.len(),
            "every byte is either a seven-byte header or payload"
        );
        // Nothing may start with a syncword any more — that is the header we removed.
        for payload in payloads {
            assert!(!(payload[0] == 0xFF && payload[1] & 0xF0 == 0xF0));
        }
    }
}

//! Re-encoding a decoded soundtrack as AC-3, keeping its surround channels.
//!
//! An experiment, reached only through `VUIO_TS_AUDIO=ac3`. The default path
//! re-encodes to stereo AAC; this one encodes 5.1 AC-3 instead, which is
//! interesting for two reasons that have nothing to do with each other.
//!
//! A television plays AC-3 natively — that is what Dolby Digital was designed
//! for, and the passthrough track of a film with one is proof the set can do it.
//! So a DTS soundtrack re-encoded to AC-3 arrives as something the set decodes
//! itself rather than something it has to be persuaded to accept. And AC-3 has
//! room for the surround channels, where the AAC path downmixes to stereo and
//! throws them away.
//!
//! Against that: six channels at 640 kbps is more encoding work and more bytes
//! than two at 192.

use anyhow::{Context, Result};
use oxideav_core::{
    AudioFrame, CodecId, CodecParameters, Encoder, Frame, SampleFormat,
};

/// Samples in an AC-3 syncframe: six blocks of 256. Fixed by the format.
pub const AC3_FRAME_SAMPLES: u64 = 1536;

/// One decoded soundtrack on its way to AC-3.
pub struct Ac3Encoder {
    inner: Box<dyn Encoder>,
    channels: u16,
}

impl Ac3Encoder {
    /// What an encoder for `channels` runs at, in bits per second.
    ///
    /// 640 kbps for 5.1 — the top of Table 5.18, and what a Blu-ray carries.
    /// The point of this path is to keep the surround, so it is not the place
    /// to economise.
    pub fn bitrate_for(channels: u16) -> u32 {
        match channels {
            1 => 96_000,
            2 => 192_000,
            3 => 256_000,
            4 => 384_000,
            5 => 448_000,
            _ => 640_000,
        }
    }

    pub fn new(sample_rate: u32, channels: u16) -> Result<Self> {
        let mut params = CodecParameters::audio(CodecId::new("ac3"));
        params.sample_rate = Some(sample_rate);
        params.channels = Some(channels);
        params.sample_format = Some(SampleFormat::S16);
        params.bit_rate = Some(u64::from(Self::bitrate_for(channels)));
        let inner = oxideav_ac3::encoder::make_encoder(&params)
            .map_err(|e| anyhow::anyhow!("AC-3 encoder: {e}"))
            .context("configuring the AC-3 encoder")?;
        Ok(Self { inner, channels })
    }

    /// Feed interleaved signed 16-bit PCM, and take whatever syncframes fall out.
    pub fn push(&mut self, pcm: &[u8]) -> Result<Vec<Vec<u8>>> {
        let stride = usize::from(self.channels) * 2;
        let samples = pcm.len() / stride.max(1);
        if samples == 0 {
            return Ok(Vec::new());
        }
        let ordered = to_bitstream_order(pcm, self.channels);
        self.inner
            .send_frame(&Frame::Audio(AudioFrame {
                samples: samples as u32,
                pts: None,
                data: vec![ordered],
            }))
            .map_err(|e| anyhow::anyhow!("AC-3 encode: {e}"))?;
        Ok(self.drain())
    }

    /// The tail the encoder was still holding.
    pub fn finish(&mut self) -> Vec<Vec<u8>> {
        let _ = self.inner.flush();
        self.drain()
    }

    fn drain(&mut self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while let Ok(packet) = self.inner.receive_packet() {
            out.push(packet.data);
        }
        out
    }
}

/// Reorder interleaved PCM from WAVE order into the order the bitstream wants.
///
/// The decoders hand back WAVE order — front left, front right, centre, LFE,
/// then the surrounds — because that is what a `.wav` file and every sink that
/// reads a channel mask expects. AC-3 numbers its channels differently: left,
/// centre, right, then the surrounds, with the LFE last. Feed one order to an
/// encoder expecting the other and the film plays with dialogue in a surround
/// speaker and the bass in the centre.
fn to_bitstream_order(pcm: &[u8], channels: u16) -> Vec<u8> {
    // WAVE slot for each bitstream slot, for the layouts with an ambiguity.
    const FIVE_ZERO: [usize; 5] = [0, 2, 1, 3, 4];
    const FIVE_ONE: [usize; 6] = [0, 2, 1, 4, 5, 3];
    let map: &[usize] = match channels {
        5 => &FIVE_ZERO,
        6 => &FIVE_ONE,
        // Mono and stereo agree in both orders
        _ => return pcm.to_vec(),
    };
    let stride = usize::from(channels) * 2;
    let mut out = vec![0u8; pcm.len()];
    for (frame, source) in pcm.chunks_exact(stride).enumerate() {
        let base = frame * stride;
        for (slot, &from) in map.iter().enumerate() {
            out[base + slot * 2..base + slot * 2 + 2]
                .copy_from_slice(&source[from * 2..from * 2 + 2]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dialogue belongs in the centre channel at both ends of the reorder.
    #[test]
    fn wave_order_becomes_bitstream_order() {
        // One frame, each channel carrying its own WAVE slot number.
        let pcm: Vec<u8> = (0i16..6).flat_map(|c| c.to_le_bytes()).collect();
        let out = to_bitstream_order(&pcm, 6);
        let slots: Vec<i16> = out
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&b| i16::from_le_bytes(b))
            .collect();
        // L, C, R, Ls, Rs, LFE — read as the WAVE slot each came from.
        assert_eq!(slots, vec![0, 2, 1, 4, 5, 3]);
        // Stereo is the same in both orders.
        assert_eq!(to_bitstream_order(&pcm[..4], 2), pcm[..4].to_vec());
    }

    #[test]
    fn five_one_is_encoded_at_the_rate_a_blu_ray_carries() {
        assert_eq!(Ac3Encoder::bitrate_for(6), 640_000);
        assert_eq!(Ac3Encoder::bitrate_for(2), 192_000);
    }
}

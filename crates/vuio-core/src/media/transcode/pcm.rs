//! Turning compressed frames into interleaved little-endian S16.
//!
//! The vendored decoders emit exactly that layout in `AudioFrame::data[0]` when
//! asked for [`SampleFormat::S16`], so this is a thin driver over the push/pull
//! `Decoder` trait rather than any DSP of its own — the one substantive choice
//! is asking the decoder for the channel count we want instead of mixing down
//! afterwards. AC-3 carries the §7.8 downmix coefficients in the bitstream, so
//! the decoder's own two-channel output is the mix the encoder intended; summing
//! channels outside it would throw that away and clip besides.

use anyhow::{bail, Context, Result};

use super::TranscodeCodec;

/// Bytes each sample occupies per channel.
const BYTES_PER_SAMPLE: usize = 2;

/// A decoder bound to one stream, with its output shape resolved.
pub struct PcmDecoder {
    inner: Box<dyn oxideav_core::Decoder>,
    codec: TranscodeCodec,
    sample_rate: u32,
    channels: u16,
}

impl PcmDecoder {
    /// Open a decoder for `codec` and resolve its output shape from `first_frame`.
    ///
    /// The channel count is measured from a real decoded frame rather than
    /// predicted from the stream's `acmod`: what matters downstream is how many
    /// channels the decoder actually emits after any downmix, and asking is both
    /// shorter and impossible to get wrong. The decoded bytes of that first
    /// frame are returned so the probe is not paid for twice.
    pub fn open(
        codec: TranscodeCodec,
        sample_rate: u32,
        want_channels: Option<u16>,
        first_frame: &[u8],
    ) -> Result<(Self, Vec<u8>)> {
        let inner = super::make_decoder(codec, sample_rate, want_channels)?;
        let mut me = Self {
            inner,
            codec,
            sample_rate,
            // Provisional: replaced by the measurement below before anyone sees it.
            channels: want_channels.unwrap_or(2),
        };

        let (pcm, samples) = me.decode_measured(first_frame)?;
        if samples == 0 || pcm.is_empty() {
            bail!(
                "{} decoder produced no samples for the first frame",
                codec.as_str()
            );
        }
        let stride = pcm.len() / (samples as usize * BYTES_PER_SAMPLE);
        if stride == 0 || stride > 8 {
            bail!(
                "{} decoder reported {samples} samples in {} bytes, which is not a sane channel count",
                codec.as_str(),
                pcm.len()
            );
        }
        me.channels = stride as u16;
        Ok((me, pcm))
    }

    /// Channels in the decoded output.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Sample rate of the decoded output, in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Decode one compressed frame into interleaved S16.
    ///
    /// A frame that fails to decode yields silence of the length the index said
    /// it would occupy — the caller has already committed to a `Content-Length`
    /// built from that index, so a mid-stream error must not change how many
    /// bytes the response carries. One corrupt frame in a film is a tick; a
    /// short body is a truncated download.
    pub fn decode_or_silence(&mut self, frame: &[u8], expect_samples: u32) -> Vec<u8> {
        let want = expect_samples as usize * self.channels as usize * BYTES_PER_SAMPLE;
        match self.decode_measured(frame) {
            Ok((mut pcm, _)) => {
                pcm.resize(want, 0);
                pcm
            }
            Err(_) => vec![0u8; want],
        }
    }

    /// Feed one frame and collect everything it produces.
    fn decode_measured(&mut self, frame: &[u8]) -> Result<(Vec<u8>, u32)> {
        use oxideav_core::{Frame, Packet, TimeBase};

        let packet = Packet::new(0, TimeBase::new(1, self.sample_rate as i64), frame.to_vec());
        self.inner
            .send_packet(&packet)
            .map_err(|e| anyhow::anyhow!("{}: {e}", self.codec.as_str()))
            .context("feeding a frame to the decoder")?;

        let mut out = Vec::new();
        let mut samples = 0u32;
        // `receive_frame` returns `NeedMore` once the packet is drained, which is
        // the normal exit, not an error.
        while let Ok(frame) = self.inner.receive_frame() {
            if let Frame::Audio(af) = frame {
                if let Some(plane) = af.data.first() {
                    out.extend_from_slice(plane);
                }
                samples += af.samples;
            }
        }
        Ok((out, samples))
    }
}

#[cfg(test)]
mod tests {
    use super::super::frames::FrameIndex;
    use super::*;

    #[cfg(feature = "transcode-ac3")]
    const AC3_FIXTURE: &[u8] =
        include_bytes!("../../../../vendor/oxideav-ac3/tests/fixtures/sine440_stereo.ac3");
    #[cfg(feature = "transcode-dts")]
    const DTS_FIXTURE: &[u8] =
        include_bytes!("../../../../vendor/oxideav-dts/tests/fixtures/dts_5_frames.bin");

    /// Decode a whole fixture, returning the PCM and the index that described it.
    fn decode_all(codec: TranscodeCodec, bytes: &[u8]) -> (Vec<u8>, FrameIndex, u16) {
        let idx = FrameIndex::build(codec, &mut &bytes[..]).unwrap();
        let first = &bytes[idx.frames[0].offset as usize..][..idx.frames[0].len as usize];
        let (mut dec, head) = PcmDecoder::open(codec, idx.sample_rate, Some(2), first).unwrap();
        let channels = dec.channels();
        let mut pcm = head;
        pcm.resize(
            idx.frames[0].samples as usize * channels as usize * 2,
            0,
        );
        for f in &idx.frames[1..] {
            let raw = &bytes[f.offset as usize..][..f.len as usize];
            pcm.extend_from_slice(&dec.decode_or_silence(raw, f.samples));
        }
        (pcm, idx, channels)
    }

    fn rms(pcm: &[u8]) -> f64 {
        let mut sum = 0.0f64;
        let mut n = 0u64;
        for c in pcm.chunks_exact(2) {
            let v = i16::from_le_bytes([c[0], c[1]]) as f64;
            sum += v * v;
            n += 1;
        }
        if n == 0 {
            0.0
        } else {
            (sum / n as f64).sqrt()
        }
    }

    /// The load-bearing guarantee: the byte count the index predicts is the byte
    /// count the decoder produces. `Content-Length` is computed from the former
    /// before a single sample is decoded, so if these ever disagree every
    /// transcoded response is truncated or over-long.
    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn ac3_decodes_to_exactly_the_length_the_index_predicted() {
        let (pcm, idx, channels) = decode_all(TranscodeCodec::Ac3, AC3_FIXTURE);
        assert_eq!(
            pcm.len() as u64,
            idx.total_samples * channels as u64 * 2,
            "decoded PCM length must match the indexed sample count"
        );
    }

    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn ac3_decodes_to_audible_stereo() {
        let (pcm, _, channels) = decode_all(TranscodeCodec::Ac3, AC3_FIXTURE);
        assert_eq!(channels, 2, "a stereo source asked for stereo stays stereo");
        // The fixture is a 440 Hz sine, so anything near zero means we produced
        // a correctly-sized block of silence instead of decoding.
        assert!(rms(&pcm) > 100.0, "decoded RMS {} is silence", rms(&pcm));
    }

    #[cfg(feature = "transcode-dts")]
    #[test]
    fn dts_decodes_to_exactly_the_length_the_index_predicted() {
        let (pcm, idx, channels) = decode_all(TranscodeCodec::Dts, DTS_FIXTURE);
        assert_eq!(
            pcm.len() as u64,
            idx.total_samples * channels as u64 * 2,
            "decoded PCM length must match the indexed sample count"
        );
    }

    #[cfg(feature = "transcode-dts")]
    #[test]
    fn dts_decodes_to_audible_audio() {
        let (pcm, _, _) = decode_all(TranscodeCodec::Dts, DTS_FIXTURE);
        assert!(rms(&pcm) > 10.0, "decoded RMS {} is silence", rms(&pcm));
    }

    /// A corrupt frame must cost its own duration in silence and nothing more,
    /// because the response length was already promised.
    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn a_corrupt_frame_yields_silence_of_the_right_length() {
        let idx = FrameIndex::build(TranscodeCodec::Ac3, &mut &AC3_FIXTURE[..]).unwrap();
        let first = &AC3_FIXTURE[idx.frames[0].offset as usize..][..idx.frames[0].len as usize];
        let (mut dec, _) =
            PcmDecoder::open(TranscodeCodec::Ac3, idx.sample_rate, Some(2), first).unwrap();
        let garbage = vec![0u8; 768];
        let out = dec.decode_or_silence(&garbage, 1536);
        assert_eq!(out.len(), 1536 * 2 * 2);
        assert!(out.iter().all(|&b| b == 0));
    }
}

//! Driving the vendored DTS decoder at a scale `i32` can actually hold.
//!
//! `oxideav-dts` reconstructs the Core profile faithfully — its per-channel PCM
//! is shape-identical to a reference decode, Pearson 1.000000 across every
//! channel of a real film — but two things about the *numbers* it hands back
//! make its `Decoder` trait impl unusable as it stands.
//!
//! The first is the output scale. §C.2.5 ends at
//! `naCh[nChIndex++] = int(rScale * raZ[i])`, and the specification does not fix
//! `rScale`: it is whatever brings a particular implementation's filterbank
//! output up to integer full scale. The crate's [`output_r_scale`] returns the
//! derivation for an implementation whose `raZ` is unit-normalised —
//! `2^(PCMR_bits - 1)`, so 2^23 for the 24-bit-sourced films that make up most
//! of a library — but its own `raZ` is not unit-normalised: a full-scale sample
//! leaves the filterbank at [`FULL_SCALE_RA_Z`], not at 1.0. Multiplying the two
//! together overflows `i32` by a factor of 180, and `as i32` saturates rather
//! than wrapping, so every sample above about -45 dBFS comes back as
//! `i32::MAX`. The result is a square wave — which is exactly what a television
//! played when the film's DTS track was selected.
//!
//! So this drives [`CoreStreamDecoder`] directly with the frame header's `PCMR`
//! forced to the 16-bit code. `rScale` is then 2^15, full scale lands at
//! `2^30 · √2`, and `i32` has √2 of headroom over it — enough that no
//! inter-sample overshoot can saturate. The scaling back down to S16 happens
//! here, in `f64`, where it costs nothing. Nothing else in the decode reads
//! `PCMR`; it feeds `output_r_scale` and no other call site.
//!
//! The second is the channel layout: the planes come back in the source's own
//! AMODE order (5.1 is `C, L, R, Ls, Rs`, not the `L, R, C, LFE, Ls, Rs` an
//! MP4 would use), and there are up to six of them where a browser tab wants
//! two. AC-3 carries the §7.8 downmix coefficients so its decoder can fold to
//! stereo itself; DTS Core does not, so the fold is [`fold_for`] below — the
//! ITU-R BS.775 coefficients, normalised the same way the AC-3 decoder
//! normalises its own, so selecting one audio track or the other does not move
//! the volume.
//!
//! [`output_r_scale`]: oxideav_dts::DtsFrameHeader::output_r_scale

use anyhow::{anyhow, bail, Context, Result};
use oxideav_dts::{AmodeArrangement, CoreStreamDecoder, DtsFrameHeader, FourteenBitByteOrder};

/// The value `raZ` reaches at full scale, where §C.2.5 assumes 1.0.
///
/// Measured, not derived: a reference decode of a real 5.1 film lines up with
/// this crate's output at exactly this ratio on all five primary channels, and
/// the vendored 5-frame fixture agrees to seven digits. It is `2^15 · √2`,
/// which is suggestive of a filterbank normalisation missing upstream, but the
/// number is load-bearing whatever its provenance, so it is stated as what it
/// is: what this decoder's filterbank puts out for a full-scale sample.
const FULL_SCALE_RA_Z: f64 = 46_340.950_011_841_18;

/// The `PCMR` code for 16-bit source PCM (ETSI TS 102 114 §5.3.1 Table 5-17).
const PCMR_16_BIT: u8 = 0b000;

/// The `rScale` that code resolves to, and therefore the one every frame is
/// decoded at here.
const R_SCALE: f64 = 32_768.0;

/// Scale from the decoder's `i32` output to S16: full scale arrives at
/// `R_SCALE · FULL_SCALE_RA_Z` and has to leave at 32768.
const TO_S16: f64 = 32_768.0 / (R_SCALE * FULL_SCALE_RA_Z);

/// -3 dB: the ITU-R BS.775 coefficient for folding a centre or surround channel
/// into both halves of a stereo pair.
const FOLD: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// Bytes per sample per channel in the output.
const BYTES_PER_SAMPLE: usize = 2;

/// One half of a fold: which planes it draws on, and at what weight.
type Taps = Vec<(usize, f64)>;

/// One DTS Core stream, decoded and folded to at most two channels.
///
/// Holds the §C.2.5 filter tail across frames — a DTS elementary stream's QMF
/// filter is continuous, and restarting it per frame injects a warm-up
/// transient at every frame boundary.
pub struct DtsDecoder {
    stream: Option<CoreStreamDecoder>,
    /// Channels the fold emits. Two unless the caller asked for one.
    channels: u16,
}

impl DtsDecoder {
    /// A decoder emitting `want_channels`, which is honoured for one and two and
    /// otherwise taken as two — every caller in this crate asks for stereo, and
    /// a caller that wants more widens the fold's output itself.
    pub fn new(want_channels: Option<u16>) -> Self {
        Self {
            stream: None,
            channels: if want_channels == Some(1) { 1 } else { 2 },
        }
    }

    /// Decode one frame into interleaved S16, with its sample count per channel.
    pub fn decode(&mut self, frame: &[u8]) -> Result<(Vec<u8>, u32)> {
        // Both 14-bit container byte orders carry the same logical bitstream as
        // the raw-16-bit forms, packed 14 payload bits per 16-bit word, and the
        // raw-16-bit parser is what says so: it refuses a 14-bit sync by name.
        // Unpack those into the domain the reconstruction operates on and
        // decode them through the identical chain.
        let (unpacked, mut header) = match oxideav_dts::parse_frame_header(frame) {
            Ok(header) => (None, header),
            Err(oxideav_dts::Error::UnsupportedFourteenBit) => {
                let packed = oxideav_dts::parse_frame_header_14bit(frame)
                    .map_err(|e| anyhow!("DTS 14-bit header: {e}"))?;
                let order = FourteenBitByteOrder::from_sync(packed.sync_word_encoding)
                    .ok_or_else(|| anyhow!("DTS: a 14-bit sync with no container byte order"))?;
                let bytes = oxideav_dts::unpack_14bit_to_16bit(frame, order)
                    .map_err(|e| anyhow!("DTS 14-bit unpack: {e}"))?;
                let header = oxideav_dts::parse_frame_header(&bytes)
                    .map_err(|e| anyhow!("DTS header: {e}"))?;
                (Some(bytes), header)
            }
            Err(e) => bail!("DTS header: {e}"),
        };
        let bytes = unpacked.as_deref().unwrap_or(frame);

        // See the module comment: the declared resolution's `rScale` saturates
        // `i32` against this decoder's filterbank output, and the 16-bit code's
        // does not.
        header.source_pcm_resolution_index = PCMR_16_BIT;

        let channels = primary_channels(bytes, &header)?;
        let mut stream = match self.stream.take() {
            // A stream's channel count is constant in practice; restarting the
            // filter for a new layout is what the vendored driver does too.
            Some(stream) if stream.channel_count() == channels => stream,
            _ => CoreStreamDecoder::new(channels),
        };

        let planes = stream.decode_frame(bytes, &header);
        // The LFE plane comes back through a separate accessor and on a
        // different scale from the primary channels. The fold drops it, as a
        // stereo downmix conventionally does, so it is never read — but it must
        // still be taken, or it accumulates into the next frame's.
        let _ = stream.take_last_lfe_pcm();
        self.stream = Some(stream);
        let planes = planes.map_err(|e| anyhow!("DTS decode: {e:?}"))?;

        let samples = planes.first().map_or(0, Vec::len);
        Ok((
            self.fold(&planes, header.amode_arrangement(), samples),
            samples as u32,
        ))
    }

    /// Fold the frame's planes into interleaved S16 at `self.channels`.
    fn fold(&self, planes: &[Vec<i32>], arrangement: AmodeArrangement, samples: usize) -> Vec<u8> {
        let (left, right) = fold_for(arrangement, planes.len());
        let left_gain = normalise(&left);
        let right_gain = normalise(&right);
        let mono = self.channels == 1;

        let mut out = Vec::with_capacity(samples * self.channels as usize * BYTES_PER_SAMPLE);
        for n in 0..samples {
            let mix = |taps: &[(usize, f64)], gain: f64| -> f64 {
                taps.iter()
                    .map(|(plane, weight)| {
                        weight * planes[*plane].get(n).copied().unwrap_or(0) as f64
                    })
                    .sum::<f64>()
                    * gain
            };
            let l = mix(&left, left_gain);
            if mono {
                out.extend_from_slice(&to_s16((l + mix(&right, right_gain)) * 0.5).to_le_bytes());
            } else {
                out.extend_from_slice(&to_s16(l).to_le_bytes());
                out.extend_from_slice(&to_s16(mix(&right, right_gain)).to_le_bytes());
            }
        }
        out
    }
}

/// Scale one summed sample to S16, clamping rather than wrapping.
fn to_s16(value: f64) -> i16 {
    (value * TO_S16).clamp(-32_768.0, 32_767.0) as i16
}

/// The gain that keeps a fold's coefficients summing to unity.
///
/// This is the same clip guard the AC-3 decoder applies to its own §7.8
/// downmix — measured at `1 / (1 + 2/√2)` against a reference decode of the
/// same film's AC-3 track — so a viewer switching between a film's AC-3 and DTS
/// renditions hears the same level rather than an 8 dB jump.
fn normalise(taps: &[(usize, f64)]) -> f64 {
    let sum: f64 = taps.iter().map(|(_, weight)| weight.abs()).sum();
    if sum > 0.0 {
        1.0 / sum
    } else {
        0.0
    }
}

/// Which planes make up each half of the stereo fold, and at what weight.
///
/// The plane order is the arrangement's own (ETSI TS 102 114 §5.3.1 Table 5-4):
/// `AMODE 9`, the one essentially every 5.1 film carries, delivers
/// `C, L, R, Ls, Rs` — centre first — which is why this is a table and not an
/// index arithmetic. Weights are the ITU-R BS.775 fold; [`normalise`] applies
/// the clip guard afterwards, so a layout with nothing to fold in (plain
/// stereo) is passed through at unity rather than attenuated.
///
/// `planes` is the trailing fallback for the arrangements above `AMODE 9`,
/// which pair their channels left-then-right across the layout and do not
/// appear in circulation; taking the even planes as left and the odd as right
/// is an approximation of them, not a reading of Table 5-4.
fn fold_for(arrangement: AmodeArrangement, planes: usize) -> (Taps, Taps) {
    use AmodeArrangement as A;
    match arrangement {
        // A single channel, heard from both speakers.
        A::Mono => (vec![(0, 1.0)], vec![(0, 1.0)]),
        // Two independent channels, or an already-folded pair: straight across.
        A::DualMono | A::Stereo | A::LtRt if planes >= 2 => {
            (vec![(0, 1.0)], vec![(1, 1.0)])
        }
        // Sum and difference: L = (S+D)/2, R = (S-D)/2.
        A::SumDifference if planes >= 2 => {
            (vec![(0, 0.5), (1, 0.5)], vec![(0, 0.5), (1, -0.5)])
        }
        // C, L, R.
        A::ClR if planes >= 3 => (vec![(1, 1.0), (0, FOLD)], vec![(2, 1.0), (0, FOLD)]),
        // L, R, S — one shared surround into both halves.
        A::LrS if planes >= 3 => (vec![(0, 1.0), (2, FOLD)], vec![(1, 1.0), (2, FOLD)]),
        // C, L, R, S.
        A::ClRS if planes >= 4 => (
            vec![(1, 1.0), (0, FOLD), (3, FOLD)],
            vec![(2, 1.0), (0, FOLD), (3, FOLD)],
        ),
        // L, R, Ls, Rs.
        A::LrSlSr if planes >= 4 => (
            vec![(0, 1.0), (2, FOLD)],
            vec![(1, 1.0), (3, FOLD)],
        ),
        // C, L, R, Ls, Rs — 5.1 without its LFE, and the layout that matters.
        A::ClRSlSr if planes >= 5 => (
            vec![(1, 1.0), (0, FOLD), (3, FOLD)],
            vec![(2, 1.0), (0, FOLD), (4, FOLD)],
        ),
        _ => {
            if planes >= 2 {
                (
                    (0..planes).step_by(2).map(|i| (i, 1.0)).collect(),
                    (1..planes).step_by(2).map(|i| (i, 1.0)).collect(),
                )
            } else {
                (vec![(0, 1.0)], vec![(0, 1.0)])
            }
        }
    }
}

/// The frame's §5.3.2 primary-channel count (`nPCHS`), which sizes the filter.
///
/// Read from the audio coding header rather than from `AMODE`, because that is
/// where the reconstruction itself reads it: a filter sized from a disagreeing
/// count decodes into the wrong number of planes.
fn primary_channels(bytes: &[u8], header: &DtsFrameHeader) -> Result<usize> {
    let header_bits = header.header_bit_length() as usize;
    let (coding, _) =
        oxideav_dts::decode_audio_coding_header_at(bytes, header_bits, header.crc_present)
            .map_err(|e| anyhow!("{e}"))
            .context("reading the DTS audio coding header")?;
    Ok(coding.n_pchs)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../../../../vendor/oxideav-dts/tests/fixtures/dts_5_frames.bin");

    /// The fixture is 2-channel, so each frame's planes fold straight across.
    #[test]
    fn the_fixture_decodes_to_stereo_at_a_sane_level() {
        let mut decoder = DtsDecoder::new(Some(2));
        // Frame boundaries: the fixture is five 1024-byte frames.
        let mut peak = 0i32;
        let mut frames = 0;
        for frame in FIXTURE.chunks_exact(1024) {
            let (pcm, samples) = decoder.decode(frame).expect("a fixture frame decodes");
            assert_eq!(pcm.len(), samples as usize * 2 * BYTES_PER_SAMPLE);
            for pair in pcm.as_chunks::<2>().0 {
                peak = peak.max(i16::from_le_bytes(*pair).unsigned_abs() as i32);
            }
            frames += 1;
        }
        assert_eq!(frames, 5);
        // The whole point: full scale is 32768, and before the `PCMR` override
        // this railed at it on every sample. A real signal sits below it.
        assert!(
            (1_000..32_000).contains(&peak),
            "peak {peak} is either silence or the saturation this exists to prevent"
        );
    }

    /// A fold with nothing to fold in must not be quieter than its source.
    #[test]
    fn a_stereo_layout_is_passed_through_at_unity() {
        let (left, right) = fold_for(AmodeArrangement::Stereo, 2);
        assert_eq!(normalise(&left), 1.0);
        assert_eq!(normalise(&right), 1.0);
    }

    /// The 5.1 fold is the ITU one, normalised to unity gain.
    #[test]
    fn the_five_one_fold_takes_l_c_and_ls_into_the_left_half() {
        let (left, right) = fold_for(AmodeArrangement::ClRSlSr, 5);
        assert_eq!(left, vec![(1, 1.0), (0, FOLD), (3, FOLD)]);
        assert_eq!(right, vec![(2, 1.0), (0, FOLD), (4, FOLD)]);
        // 1 / (1 + 2/√2), the AC-3 decoder's own clip guard.
        assert!((normalise(&left) - 0.414_213_562_373).abs() < 1e-9);
    }

    /// Sum/difference recovers the pair rather than folding it.
    #[test]
    fn sum_difference_is_undone_rather_than_mixed() {
        let (left, right) = fold_for(AmodeArrangement::SumDifference, 2);
        assert_eq!(left, vec![(0, 0.5), (1, 0.5)]);
        assert_eq!(right, vec![(0, 0.5), (1, -0.5)]);
        assert_eq!(normalise(&left), 1.0);
    }
}

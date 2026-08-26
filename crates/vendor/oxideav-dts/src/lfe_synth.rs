//! §C.2.6 `InterpolationFIR()` driver body — the DTS Core
//! low-frequency-effects (LFE) polyphase upsampling convolution loop.
//!
//! Transcribed from ETSI TS 102 114 V1.3.1 Annex C §C.2.6
//! (`InterpolationFIR()`, printed PDF p.186), staged as pseudocode in
//! `docs/audio/dts/dts-lfe-interpolation-and-audio-walker.md` §1. The
//! companion table *selection* step lives in
//! [`crate::LfeInterpolationSelection`]; this module is the
//! **convolution body** that consumes the selected §D.8 512-tap
//! coefficient set and the decimated LFE sample stream and emits the
//! upsampled per-channel PCM.
//!
//! # Polyphase structure
//!
//! The 512-tap kernel is read as a polyphase bank of `nDeciFactor`
//! phases, each `NumFIRCoef / nDeciFactor` taps long. Per the doc §1
//! "Polyphase structure", for phase `k ∈ [0, nDeciFactor)`:
//!
//! ```text
//!     output[nDeciFactor*j + k]
//!         = Σ_{J = 0 .. taps_per_phase - 1}  prCoeff[k + J*nDeciFactor]
//!                                          * rLFE[j - J]
//! ```
//!
//! where `taps_per_phase = NumFIRCoef / nDeciFactor`:
//!
//! - 128× filter (`nDecimationSelect == 1`, `raCoeff128`):
//!   `512 / 128 = 4` taps per phase.
//! - 64× filter (`nDecimationSelect == 0`, `raCoeff64`):
//!   `512 / 64 = 8` taps per phase.
//!
//! Each decimated input sample `rLFE[j]` therefore expands to exactly
//! `nDeciFactor` interpolated output samples.
//!
//! # The spurious-increment transcription artefact
//!
//! The spec PDF's `InterpolationFIR()` pseudocode prints a trailing
//! `nDeciIndex++;` *inside* the outer for-loop body, just before the
//! closing brace. The doc §1 "Implementation note" flags this as a
//! long-standing transcription artefact: the C `for` loop already
//! increments `nDeciIndex`, so reproducing the inner increment
//! literally would read every *second* decimated sample and emit zeros
//! for the gaps. This implementation follows the doc's resolution and
//! does **not** emit the inner increment — every decimated sample is
//! consumed, in order.
//!
//! # Persistent per-channel history
//!
//! `rLFE[j - J]` for `J ≥ 1` reaches *before* the current sub-frame's
//! first decimated sample. Per the doc §1 "History buffer requirement",
//! the decoder must carry the previous sub-frame's last
//! `taps_per_phase - 1` decimated samples across sub-frame boundaries
//! (≥ 7 for the 64× filter, ≥ 3 for the 128× filter). [`LfeInterpolator`]
//! owns that history and starts it cleared (the spec's per-channel LFE
//! filter has no history before the first sub-frame).

use crate::lfe_fir_coeff::LFE_FIR_COEFF_LEN;
use crate::lfe_interp::LfeInterpolationSelection;

/// The longest polyphase history any §C.2.6 LFE filter needs:
/// `taps_per_phase - 1` decimated samples. The 64× filter
/// (`512 / 64 = 8` taps per phase) needs `8 - 1 = 7`; the 128× filter
/// needs `4 - 1 = 3`. Both fit in this fixed-size ring.
pub const LFE_HISTORY_LEN: usize = 7;

/// Errors from the §C.2.6 LFE interpolation driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LfeInterpError {
    /// The supplied output slice is shorter than
    /// `decimated.len() * nDeciFactor`, the exact upsampled length the
    /// §C.2.6 loop produces.
    OutputSliceTooShort {
        /// Output samples required (`decimated.len() * nDeciFactor`).
        required: usize,
        /// Output samples actually available.
        available: usize,
    },
}

impl core::fmt::Display for LfeInterpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LfeInterpError::OutputSliceTooShort {
                required,
                available,
            } => write!(
                f,
                "LFE interpolation output slice too short: need {required}, have {available}"
            ),
        }
    }
}

impl std::error::Error for LfeInterpError {}

/// Persistent per-channel §C.2.6 LFE interpolation filter — the
/// `LFECh` object whose `InterpolationFIR(nDecimationSelect)` call
/// upsamples one sub-frame's decimated LFE samples to PCM rate.
///
/// Holds the `taps_per_phase - 1` decimated-sample history the
/// polyphase convolution reads across sub-frame boundaries (the doc §1
/// "History buffer requirement"). Construct once per LFE channel and
/// drive each sub-frame's samples through [`LfeInterpolator::process`]
/// (or [`LfeInterpolator::interpolate`]) in order so the inter-sub-frame
/// tail carries correctly.
#[derive(Debug, Clone)]
pub struct LfeInterpolator {
    /// The decimated-sample history `rLFE[-1], rLFE[-2], …`, most-recent
    /// first: `history[0]` is the previous sub-frame's last decimated
    /// sample, `history[1]` the one before it, and so on. Sized for the
    /// deepest (64×) filter; the 128× filter reads only the first 3
    /// entries. Cleared at construction.
    history: [f64; LFE_HISTORY_LEN],
}

impl Default for LfeInterpolator {
    fn default() -> Self {
        Self::new()
    }
}

impl LfeInterpolator {
    /// Construct a fresh per-channel LFE interpolation filter with a
    /// cleared decimated-sample history, matching the spec's
    /// per-channel LFE filter state before the first sub-frame.
    #[must_use]
    pub fn new() -> Self {
        Self {
            history: [0.0; LFE_HISTORY_LEN],
        }
    }

    /// Borrow the current decimated-sample history (most-recent first).
    /// Exposed for callers that want to inspect or checkpoint the
    /// inter-sub-frame tail; [`LfeInterpolator::process`] maintains it
    /// automatically.
    #[must_use]
    pub fn history(&self) -> &[f64; LFE_HISTORY_LEN] {
        &self.history
    }

    /// Run the §C.2.6 `InterpolationFIR()` polyphase convolution over
    /// one sub-frame's decimated LFE samples, writing the upsampled PCM
    /// into `output` and advancing the per-channel history.
    ///
    /// `decimated[j]` is the spec's `rLFE[j]` for the current sub-frame
    /// (already scaled at dequant time: `LFECh.rLFE[k] = LFE[n]*rScale`
    /// with `rScale = nScale*0.035`, per the §5.5 LFE phase). `selection`
    /// resolves `nDecimationSelect` to one of the two §D.8 512-tap
    /// coefficient sets and to the decimation factor (`64` / `128`).
    ///
    /// Exactly `decimated.len() * nDeciFactor` output samples are
    /// produced, in time order, as `f64` (the spec casts to integer at
    /// store time — [`LfeInterpolator::interpolate`] does the cast for a
    /// PCM caller). The history is advanced by the last
    /// `taps_per_phase - 1` decimated samples so the next sub-frame's
    /// convolution sees the correct tail.
    ///
    /// # Errors
    ///
    /// Returns [`LfeInterpError::OutputSliceTooShort`] if `output` is
    /// shorter than `decimated.len() * nDeciFactor`. The history is not
    /// advanced on error.
    pub fn process(
        &mut self,
        decimated: &[f64],
        selection: LfeInterpolationSelection,
        output: &mut [f64],
    ) -> Result<usize, LfeInterpError> {
        let n_deci_factor = selection.decimation_factor() as usize;
        // taps_per_phase = NumFIRCoef / nDeciFactor (doc §1): 4 for the
        // 128× filter, 8 for the 64× filter.
        let taps_per_phase = LFE_FIR_COEFF_LEN / n_deci_factor;
        let pr_coeff = selection.coefficients();

        let required = decimated.len() * n_deci_factor;
        if output.len() < required {
            return Err(LfeInterpError::OutputSliceTooShort {
                required,
                available: output.len(),
            });
        }

        // Polyphase convolution. For decimated index j and phase k:
        //   output[nDeciFactor*j + k]
        //     = Σ_{J=0..taps_per_phase-1} prCoeff[k + J*nDeciFactor]
        //                               * rLFE[j - J]
        // rLFE[j - J] for J > j reaches into the carried history:
        //   rLFE[-1] = history[0], rLFE[-2] = history[1], …
        for (j, _) in decimated.iter().enumerate() {
            for k in 0..n_deci_factor {
                let mut acc = 0.0_f64;
                for big_j in 0..taps_per_phase {
                    // rLFE[j - big_j]
                    let sample = if big_j <= j {
                        decimated[j - big_j]
                    } else {
                        // History index: rLFE[-1] is history[0], so
                        // rLFE[j - big_j] with (big_j - j) >= 1 maps to
                        // history[big_j - j - 1].
                        self.history[big_j - j - 1]
                    };
                    acc += pr_coeff[k + big_j * n_deci_factor] * sample;
                }
                output[n_deci_factor * j + k] = acc;
            }
        }

        // Advance the history: the new most-recent samples are the last
        // taps_per_phase - 1 of this sub-frame, most-recent first. If the
        // sub-frame is shorter than that, the remaining slots come from
        // the old history (shifted down).
        self.advance_history(decimated, taps_per_phase - 1);

        Ok(required)
    }

    /// Convenience over [`LfeInterpolator::process`] that allocates the
    /// output `Vec<f64>` and returns it — the upsampled LFE PCM at full
    /// sample rate for one sub-frame.
    pub fn process_to_vec(
        &mut self,
        decimated: &[f64],
        selection: LfeInterpolationSelection,
    ) -> Vec<f64> {
        let n_deci_factor = selection.decimation_factor() as usize;
        let mut out = vec![0.0_f64; decimated.len() * n_deci_factor];
        // The output is sized exactly, so process() cannot return the
        // too-short error.
        let _ = self.process(decimated, selection, &mut out);
        out
    }

    /// Like [`LfeInterpolator::process_to_vec`] but casts each upsampled
    /// sample to `i32` (the spec's `naCh[nInterpIndex++] = (int)rTmp`
    /// store), truncating toward zero — the integer PCM the LFE channel
    /// contributes to the decoded output.
    pub fn interpolate(
        &mut self,
        decimated: &[f64],
        selection: LfeInterpolationSelection,
    ) -> Vec<i32> {
        self.process_to_vec(decimated, selection)
            .into_iter()
            // `(int)rTmp` in C truncates toward zero.
            .map(|v| v as i32)
            .collect()
    }

    /// Slide the `keep` most-recent decimated samples into the history,
    /// most-recent first. Handles a sub-frame shorter than `keep` by
    /// retaining older history entries behind the new ones.
    fn advance_history(&mut self, decimated: &[f64], keep: usize) {
        // Build the new history most-recent-first: the last samples of
        // this sub-frame, then (if the sub-frame was shorter than `keep`)
        // the previous history entries.
        let mut new_history = [0.0_f64; LFE_HISTORY_LEN];
        let n = decimated.len();
        for (slot, h) in new_history.iter_mut().take(keep).enumerate() {
            if slot < n {
                // decimated[n - 1 - slot]: most-recent first.
                *h = decimated[n - 1 - slot];
            } else {
                // Older than this sub-frame: pull from the old history.
                // The (slot - n)-th old entry, shifted past the n new
                // ones we just consumed.
                let old_idx = slot - n;
                *h = self.history[old_idx];
            }
        }
        self.history = new_history;
    }
}

/// The §5.5 LFE-phase quantiser step constant: `rScale = nScale * 0.035`
/// where `nScale` is the [`crate::RMS_7BIT`] (§D.1.2) entry the 8-bit
/// `LFEscaleIndex` selects. Per the §5.5 LFE phase pseudocode in
/// `docs/audio/dts/dts-lfe-interpolation-and-audio-walker.md` §2.2.
pub const LFE_SCALE_STEP: f64 = 0.035;

/// Errors from the §5.5 LFE-phase dequant + interpolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LfeChannelError {
    /// The 8-bit `LFEscaleIndex` selected a reserved/invalid
    /// [`crate::RMS_7BIT`] entry (§D.1.2 indices 125..=127), so no
    /// quantiser scale is defined.
    ReservedScaleIndex {
        /// The offending 8-bit scale index.
        index: u8,
    },
    /// `lfe_mode` had no LFE channel present (raw code 0), so there is
    /// no LFE phase to decode.
    NoLfeChannel,
}

impl core::fmt::Display for LfeChannelError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LfeChannelError::ReservedScaleIndex { index } => {
                write!(f, "LFE scale index {index} is reserved (RMS_7BIT §D.1.2)")
            }
            LfeChannelError::NoLfeChannel => write!(f, "no LFE channel present (LFF == 0)"),
        }
    }
}

impl std::error::Error for LfeChannelError {}

/// The §5.5 LFE channel — composes the §5.5 LFE-phase dequant
/// (`LFEscaleIndex → pLFE_RMS → nScale; rScale = nScale·0.035`) with
/// the §C.2.6 [`LfeInterpolator`] convolution, owning the
/// inter-sub-frame decimated-sample history.
///
/// Per `docs/audio/dts/dts-lfe-interpolation-and-audio-walker.md` §2.2,
/// the LFE phase (present only when the frame header's `LFF` flag is
/// non-zero) reads `2·LFF·nSSC` 8-bit two's-complement decimated LFE
/// samples and an 8-bit `LFEscaleIndex`, dequantises
/// `rLFE[n] = LFE[n]·nScale·0.035`, then calls
/// `InterpolationFIR(LFF)` to upsample to PCM rate. `LFF == 1` selects
/// the 128× filter, `LFF == 2` the 64× filter (the §C.2.6
/// `nDecimationSelect == 1 ? 128× : 64×` split with `nDecimationSelect
/// = LFF`).
#[derive(Debug, Clone)]
pub struct LfeChannel {
    interp: LfeInterpolator,
}

impl Default for LfeChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl LfeChannel {
    /// Construct a fresh LFE channel with a cleared interpolation
    /// history.
    #[must_use]
    pub fn new() -> Self {
        Self {
            interp: LfeInterpolator::new(),
        }
    }

    /// Borrow the underlying §C.2.6 interpolator (for history
    /// inspection / checkpointing).
    #[must_use]
    pub fn interpolator(&self) -> &LfeInterpolator {
        &self.interp
    }

    /// Resolve the §C.2.6 decimation selection from the frame header's
    /// `LFF` value (the §5.5 LFE phase's `InterpolationFIR(LFF)` call):
    /// `LFF == 1 → 128×`, every other non-zero `LFF → 64×`.
    #[must_use]
    pub fn selection_for_lff(lff: u8) -> LfeInterpolationSelection {
        LfeInterpolationSelection::from_decimation_select(lff)
    }

    /// Run the §5.5 LFE phase for one sub-frame: dequantise the 8-bit
    /// two's-complement `lfe_samples` with the `scale_index`-selected
    /// §D.1.2 RMS scale and the `0.035` quantiser step, then upsample
    /// via the §C.2.6 polyphase convolution.
    ///
    /// `lfe_samples` are the raw `LFE[n]` bytes the §5.5 walker read
    /// (`2·LFF·nSSC` of them); `scale_index` is the 8-bit
    /// `LFEscaleIndex`; `lff` is the frame header's non-zero `LFF`
    /// selecting the decimation factor. Returns the integer PCM
    /// (`(int)rTmp`, truncate-toward-zero) the LFE channel contributes,
    /// `lfe_samples.len() * (64 | 128)` samples long, and advances the
    /// inter-sub-frame history.
    ///
    /// # Errors
    ///
    /// [`LfeChannelError::NoLfeChannel`] if `lff == 0`;
    /// [`LfeChannelError::ReservedScaleIndex`] if `scale_index` selects a
    /// reserved §D.1.2 entry (125..=127).
    pub fn decode_subframe(
        &mut self,
        lfe_samples: &[i8],
        scale_index: u8,
        lff: u8,
    ) -> Result<Vec<i32>, LfeChannelError> {
        if lff == 0 {
            return Err(LfeChannelError::NoLfeChannel);
        }
        // §D.1.2 reserves indices 125..=127 (the RMS_7BIT tail).
        if (scale_index as usize) >= crate::side_info::RMS_7BIT.len() - 3 {
            return Err(LfeChannelError::ReservedScaleIndex { index: scale_index });
        }
        let n_scale = crate::side_info::RMS_7BIT[scale_index as usize] as f64;
        let r_scale = n_scale * LFE_SCALE_STEP;

        // rLFE[n] = LFE[n] * rScale.
        let decimated: Vec<f64> = lfe_samples
            .iter()
            .map(|&s| f64::from(s) * r_scale)
            .collect();

        let selection = Self::selection_for_lff(lff);
        Ok(self.interp.interpolate(&decimated, selection))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh interpolator starts with a cleared decimated-sample
    /// history.
    #[test]
    fn new_starts_with_cleared_history() {
        let lfe = LfeInterpolator::new();
        assert!(lfe.history().iter().all(|&v| v == 0.0));
    }

    /// The output length is exactly `decimated.len() * nDeciFactor` for
    /// both decimation factors.
    #[test]
    fn output_length_is_decimated_times_factor() {
        for sel in [
            LfeInterpolationSelection::Decimation64,
            LfeInterpolationSelection::Decimation128,
        ] {
            let mut lfe = LfeInterpolator::new();
            let decimated = vec![0.0_f64; 5];
            let out = lfe.process_to_vec(&decimated, sel);
            assert_eq!(out.len(), 5 * sel.decimation_factor() as usize);
        }
    }

    /// A too-short output slice is rejected before any sample is written
    /// and the history is left untouched.
    #[test]
    fn rejects_short_output_slice() {
        let mut lfe = LfeInterpolator::new();
        let decimated = vec![1.0_f64; 3];
        let mut out = vec![0.0_f64; 3 * 64 - 1]; // one short for 64×
        let err = lfe
            .process(
                &decimated,
                LfeInterpolationSelection::Decimation64,
                &mut out,
            )
            .unwrap_err();
        assert_eq!(
            err,
            LfeInterpError::OutputSliceTooShort {
                required: 3 * 64,
                available: 3 * 64 - 1,
            }
        );
        assert!(lfe.history().iter().all(|&v| v == 0.0));
    }

    /// An all-zero decimated input with cleared history yields all-zero
    /// PCM: every convolution accumulator sums zero products.
    #[test]
    fn zero_input_yields_zero_pcm() {
        for sel in [
            LfeInterpolationSelection::Decimation64,
            LfeInterpolationSelection::Decimation128,
        ] {
            let mut lfe = LfeInterpolator::new();
            let out = lfe.interpolate(&[0.0; 4], sel);
            assert_eq!(out.len(), 4 * sel.decimation_factor() as usize);
            assert!(out.iter().all(|&s| s == 0));
        }
    }

    /// The phase-0 output of a single unit impulse (with cleared
    /// history) is exactly the polyphase phase-0 lead tap `prCoeff[0]`,
    /// and phase `k`'s first output is `prCoeff[k]` — the J = 0 term of
    /// the convolution with `rLFE[0] = 1`, all later taps reaching zero
    /// history.
    #[test]
    fn unit_impulse_reproduces_phase_lead_taps() {
        let sel = LfeInterpolationSelection::Decimation128;
        let n = sel.decimation_factor() as usize;
        let coeff = sel.coefficients();
        let mut lfe = LfeInterpolator::new();
        let out = lfe.process_to_vec(&[1.0], sel);
        assert_eq!(out.len(), n);
        // output[k] = Σ_J coeff[k + J*n] * rLFE[0 - J]; only J=0 survives
        // (rLFE[-1..] = 0), so output[k] = coeff[k].
        for k in 0..n {
            assert!(
                (out[k] - coeff[k]).abs() < 1e-12,
                "phase {k}: got {}, want {}",
                out[k],
                coeff[k]
            );
        }
    }

    /// The history carries across calls: splitting a decimated stream
    /// into two `process` calls must equal one call over the
    /// concatenation, because the polyphase convolution reads the
    /// previous sub-frame's tail.
    #[test]
    fn split_calls_match_single_concatenated_call() {
        let sel = LfeInterpolationSelection::Decimation64;
        // A deterministic non-trivial decimated stream of 12 samples.
        let decimated: Vec<f64> = (0..12).map(|i| ((i * 5 + 3) % 9) as f64 - 4.0).collect();

        // Single call.
        let mut single_lfe = LfeInterpolator::new();
        let single = single_lfe.process_to_vec(&decimated, sel);

        // Split 5 + 7, reusing the same filter.
        let mut split_lfe = LfeInterpolator::new();
        let mut split = split_lfe.process_to_vec(&decimated[..5], sel);
        split.extend(split_lfe.process_to_vec(&decimated[5..], sel));

        assert_eq!(single.len(), split.len());
        for (i, (a, b)) in single.iter().zip(split.iter()).enumerate() {
            assert!((a - b).abs() < 1e-9, "sample {i}: {a} vs {b}");
        }
        assert!(single.iter().any(|&s| s != 0.0));
    }

    /// History depth is honoured: after a sub-frame, the stored history
    /// holds the last `taps_per_phase - 1` decimated samples,
    /// most-recent first.
    #[test]
    fn history_holds_last_samples_most_recent_first() {
        let sel = LfeInterpolationSelection::Decimation64; // 8 taps/phase → keep 7
        let decimated: Vec<f64> = (0..10).map(|i| i as f64 + 1.0).collect();
        let mut lfe = LfeInterpolator::new();
        let _ = lfe.process_to_vec(&decimated, sel);
        // keep = 7: history[0] = decimated[9] = 10, history[1] = 9, …
        for slot in 0..7 {
            assert_eq!(lfe.history()[slot], decimated[9 - slot]);
        }
    }

    /// A sub-frame shorter than the history depth retains older entries
    /// behind the new ones (no history is dropped prematurely).
    #[test]
    fn short_subframe_retains_older_history() {
        let sel = LfeInterpolationSelection::Decimation64; // keep 7
        let mut lfe = LfeInterpolator::new();
        // First a long sub-frame to seed the history.
        let first: Vec<f64> = (0..8).map(|i| (i + 1) as f64).collect();
        let _ = lfe.process_to_vec(&first, sel);
        // history = [8,7,6,5,4,3,2]
        // Now a short sub-frame of 2 samples [100, 200].
        let _ = lfe.process_to_vec(&[100.0, 200.0], sel);
        // New history most-recent first: [200, 100, then old[0..5] = 8,7,6,5,4]
        assert_eq!(lfe.history()[0], 200.0);
        assert_eq!(lfe.history()[1], 100.0);
        assert_eq!(lfe.history()[2], 8.0);
        assert_eq!(lfe.history()[3], 7.0);
        assert_eq!(lfe.history()[4], 6.0);
        assert_eq!(lfe.history()[5], 5.0);
        assert_eq!(lfe.history()[6], 4.0);
    }

    // -----------------------------------------------------------
    // §5.5 LFE phase — LfeChannel dequant + interpolation.
    // -----------------------------------------------------------

    /// `LFF == 0` has no LFE channel, so decode_subframe declines.
    #[test]
    fn lfe_channel_declines_no_lfe() {
        let mut ch = LfeChannel::new();
        assert_eq!(
            ch.decode_subframe(&[0, 0], 0, 0).unwrap_err(),
            LfeChannelError::NoLfeChannel
        );
    }

    /// A reserved §D.1.2 scale index (125..=127) is rejected.
    #[test]
    fn lfe_channel_rejects_reserved_scale_index() {
        let mut ch = LfeChannel::new();
        for idx in [125u8, 126, 127] {
            assert_eq!(
                ch.decode_subframe(&[0], idx, 1).unwrap_err(),
                LfeChannelError::ReservedScaleIndex { index: idx }
            );
        }
    }

    /// `selection_for_lff` maps `LFF` per the §C.2.6 / §5.5 split:
    /// `1 → 128×`, `2 → 64×`.
    #[test]
    fn lfe_channel_selection_for_lff() {
        assert_eq!(
            LfeChannel::selection_for_lff(1),
            LfeInterpolationSelection::Decimation128
        );
        assert_eq!(
            LfeChannel::selection_for_lff(2),
            LfeInterpolationSelection::Decimation64
        );
    }

    /// The decoded LFE PCM has length `lfe_samples.len() * nDeciFactor`
    /// and an all-zero input yields silence.
    #[test]
    fn lfe_channel_decodes_zero_input_to_silence() {
        let mut ch = LfeChannel::new();
        // LFF == 1 → 128×.
        let pcm = ch.decode_subframe(&[0, 0, 0], 10, 1).unwrap();
        assert_eq!(pcm.len(), 3 * 128);
        assert!(pcm.iter().all(|&s| s == 0));
    }

    /// A non-zero LFE sample is dequantised by `nScale·0.035` and feeds
    /// the polyphase phase-lead taps: the phase-0 first output equals
    /// `(int)(LFE[0]·nScale·0.035·prCoeff[0])`.
    #[test]
    fn lfe_channel_applies_rms_scale_and_step() {
        let scale_index = 60u8; // a mid-table RMS_7BIT entry
        let n_scale = crate::side_info::RMS_7BIT[scale_index as usize] as f64;
        let r_scale = n_scale * LFE_SCALE_STEP;
        let lfe0 = 5_i8;
        let lff = 1u8; // 128×
        let sel = LfeInterpolationSelection::Decimation128;
        let coeff = sel.coefficients();
        let expected0 = (f64::from(lfe0) * r_scale * coeff[0]) as i32;

        let mut ch = LfeChannel::new();
        let pcm = ch.decode_subframe(&[lfe0], scale_index, lff).unwrap();
        assert_eq!(pcm[0], expected0);
    }

    /// The integer cast truncates toward zero (the spec's `(int)rTmp`),
    /// not floor: a small negative accumulator rounds toward zero.
    #[test]
    fn integer_cast_truncates_toward_zero() {
        // Build an impulse whose phase-0 lead tap is negative, then scale
        // so the magnitude is < 1: (int) of -0.x is 0, not -1.
        let sel = LfeInterpolationSelection::Decimation128;
        let coeff = sel.coefficients();
        // Find a phase with a non-zero coefficient to scale.
        let k0 = (0..(sel.decimation_factor() as usize))
            .find(|&k| coeff[k] != 0.0)
            .unwrap();
        let scale = 0.5 / coeff[k0].abs(); // make |output[k0]| ≈ 0.5
        let mut lfe = LfeInterpolator::new();
        let out_i = lfe.interpolate(&[scale], sel);
        // |0.5| truncates to 0 regardless of sign.
        assert_eq!(out_i[k0], 0);
    }
}

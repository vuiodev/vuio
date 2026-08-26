//! Fused 32-band synthesis QMF driver — the §C.2.5
//! `QMFInterpolation()` per-channel outer loop, transcribed verbatim
//! in `docs/audio/dts/dts-core-extracts.md` §2.4 (ETSI TS 102 114
//! V1.3.1 Annex C §C.2.5, staged PDF p.185).
//!
//! Rounds 255–285 landed every FIR-independent per-sample step of the
//! §C.2.5 loop body as a standalone primitive:
//!
//! * [`crate::assemble_xin`] — step (a), build `raXin[0..32]` from one
//!   sample per active subband (inactive subbands zero-filled);
//! * [`crate::cos_mod_stage`] — step (b), the cosine-modulation
//!   matrix multiply that refreshes `raX[0..32]`;
//! * [`crate::fir_step`] — step (c), the 512-tap §D.8 FIR convolution
//!   that accumulates into `raZ[0..64]`;
//! * [`crate::write_pcm_output`] — step (d), `int(rScale·raZ[i])` for
//!   the 32 low accumulator entries;
//! * [`crate::shift_x_history`] / [`crate::shift_z_output`] — step
//!   (e), the per-sample shift-register / accumulator rotates.
//!
//! This module is the **fused driver** that composes those primitives
//! into the complete §C.2.5 outer loop:
//!
//! ```text
//! QMFInterpolation(FILTS, int nSUBS) {
//!     if (FILTS==0) prCoeff = raCoeffLossy; else prCoeff = raCoeffLossLess;
//!     nChIndex = 0;
//!     for (nSubIndex=nStart; nSubIndex<nEnd; nSubIndex++) {
//!         // (a) assemble raXin from aSubband[i].raSample[nSubIndex]
//!         // (b) cosine-modulation -> raX[0..32]
//!         // (c) 512-tap FIR -> raZ[0..64]
//!         // (d) int(rScale*raZ[i]) -> naCh[nChIndex++]   (32 samples)
//!         // (e) shift raX history by 32; rotate raZ down by 32
//!     }
//! }
//! ```
//!
//! The driving call is, per channel (§C.2.5):
//! `aPrmCh[ch].QMFInterpolation(FILTS, nSUBS[ch]);`.
//!
//! # Persistent per-channel state
//!
//! The §C.2.5 pseudocode keeps `raX[]` (the 512-tap shift register)
//! and `raZ[]` (the 64-entry output accumulator) **across** the
//! per-sample iterations of one `QMFInterpolation()` call, and — since
//! `aPrmCh[ch]` is a persistent per-channel object the decoder calls
//! once per subframe — across successive subframes of the same
//! channel. [`QmfSynthesis`] is that per-channel object: it owns the
//! `raX[]` / `raZ[]` state and zero-initialises both at construction
//! (the spec's per-channel filter starts with a cleared history before
//! the first subframe). A caller that decodes a multi-subframe channel
//! constructs one [`QmfSynthesis`] for the channel and feeds each
//! subframe's subband samples through [`QmfSynthesis::synthesize`] in
//! order, so the inter-subframe filter tail (`raX[]`) carries
//! correctly.
//!
//! # FIR independence
//!
//! Only step (c) ([`crate::fir_step`]) reads the §D.8 coefficient
//! tables. This driver selects the table once per call from the
//! caller-supplied [`crate::FilterBankSelection`] (the resolved
//! `FILTS` branch) and threads it into every per-sample FIR step,
//! exactly as the spec hoists the `prCoeff = …` assignment out of the
//! per-sample loop.

use crate::cos_mod::{cos_mod_stage, precal_cos_mod, COS_MOD_LEN, NUM_SUBBAND};
use crate::filter_bank::FilterBankSelection;
use crate::qmf_assemble::{
    assemble_xin, fir_step, shift_x_history, shift_z_output, write_pcm_output, QmfAssembleError,
    PCM_OUTPUT_PER_SAMPLE, X_HISTORY_LEN, Z_OUTPUT_LEN,
};

/// Persistent per-channel 32-band synthesis QMF state — the §C.2.5
/// `aPrmCh[ch]` filter object that [`QmfSynthesis::synthesize`] drives
/// once per subframe.
///
/// Holds the 512-tap shift register `raX[]` ([`X_HISTORY_LEN`]) and
/// the 64-entry output accumulator `raZ[]` ([`Z_OUTPUT_LEN`]) the
/// §C.2.5 per-sample loop carries across iterations and across
/// successive subframes of the same channel. Both arrays start cleared
/// ([`QmfSynthesis::new`]); the inter-subframe filter tail lives in
/// `raX[]`, so a multi-subframe channel must reuse one instance and
/// feed its subframes in order.
///
/// The 544-entry cosine-modulation matrix `raCosMod[]`
/// ([`precal_cos_mod`]) is precomputed once at construction and reused
/// for every per-sample [`cos_mod_stage`] call (the spec's
/// `PreCalCosMod()` runs once before any `QMFInterpolation()` call).
#[derive(Debug, Clone)]
pub struct QmfSynthesis {
    /// The §C.2.5 `raX[]` 512-tap synthesis shift register. The low 32
    /// entries `raX[0..32]` are refreshed every per-sample iteration by
    /// [`cos_mod_stage`]; the FIR step convolves the full register
    /// against the §D.8 coefficients; the post-PCM shift then rotates
    /// the whole register up by 32, carrying the filter tail across
    /// per-sample iterations and across subframes.
    ra_x: [f64; X_HISTORY_LEN],
    /// The §C.2.5 `raZ[]` 64-entry output accumulator. The FIR step
    /// accumulates into it; the PCM step reads `raZ[0..32]`; the rotate
    /// slides `raZ[32..64]` down into `raZ[0..32]` and clears the high
    /// block. Carries each output sample's partial sums between the two
    /// consecutive per-sample iterations that complete it.
    ra_z: [f64; Z_OUTPUT_LEN],
    /// The §C.2.5 544-entry `raCosMod[]` matrix
    /// ([`precal_cos_mod`]), precomputed once and reused for every
    /// per-sample [`cos_mod_stage`] call.
    ra_cos_mod: [f64; COS_MOD_LEN],
}

impl Default for QmfSynthesis {
    fn default() -> Self {
        Self::new()
    }
}

impl QmfSynthesis {
    /// Construct a fresh per-channel synthesis filter with a cleared
    /// history (`raX[] = raZ[] = 0`), matching the §C.2.5 per-channel
    /// filter's initial state before the first subframe. The
    /// cosine-modulation matrix is precomputed here once.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ra_x: [0.0; X_HISTORY_LEN],
            ra_z: [0.0; Z_OUTPUT_LEN],
            ra_cos_mod: precal_cos_mod(),
        }
    }

    /// Run the §C.2.5 `QMFInterpolation()` outer loop over one block of
    /// subband samples, appending the reconstructed PCM to `output`.
    ///
    /// `subband_samples[s]` is the §C.2.5 per-sample subband vector at
    /// sample index `nSubIndex = nStart + s` — one `f64` per subband,
    /// `aSubband[i].raSample[nSubIndex]` for `i ∈ 0..32`. Subbands at
    /// or beyond `n_subs` are zero-filled by [`assemble_xin`] before
    /// the cosine-modulation step (the spec's
    /// `for (i=nSUBS; i<NumSubband; i++) raXin[i] = 0.0;`), so the
    /// caller may pass either exactly `n_subs` active values (the rest
    /// taken as zero) per row or the full 32-slot vector — only the
    /// leading `n_subs` entries are read.
    ///
    /// `filter` is the resolved §C.2.5 `FILTS` branch
    /// ([`FilterBankSelection::from_filts`]); its
    /// [`FilterBankSelection::coefficients`] §D.8 table is selected
    /// once and threaded into every per-sample FIR step. `r_scale` is
    /// the §C.2.5 `rScale` output multiplier applied before the
    /// integer cast in the PCM step (the §C.2.5 clause uses `rScale`
    /// without assigning it inside `QMFInterpolation()`, so it is a
    /// caller-supplied parameter — see the round CHANGELOG docs gap).
    ///
    /// Each per-sample iteration emits exactly
    /// [`PCM_OUTPUT_PER_SAMPLE`] (= 32) samples, so `subband_samples`
    /// of length `L` appends `L * 32` samples to `output`. The driver
    /// reserves that capacity up front, runs the loop body — assemble,
    /// cosine-modulate (refreshing `raX[0..32]`), FIR-convolve, write
    /// 32 PCM samples, then shift `raX[]` / rotate `raZ[]` — for every
    /// row, and persists the updated `raX[]` / `raZ[]` state for the
    /// channel's next subframe.
    ///
    /// # Errors
    ///
    /// Returns [`QmfAssembleError::SubsOutOfRange`] if
    /// `n_subs > NUM_SUBBAND`, and
    /// [`QmfAssembleError::SampleSliceTooShort`] if any row carries
    /// fewer than `n_subs` values. (The PCM-output step writes into a
    /// scratch buffer sized for exactly 32 samples, so its
    /// [`QmfAssembleError::OutputSliceTooShort`] precondition is
    /// satisfied by construction and never surfaces here.)
    pub fn synthesize(
        &mut self,
        subband_samples: &[[f64; NUM_SUBBAND]],
        n_subs: usize,
        filter: FilterBankSelection,
        r_scale: f64,
        output: &mut Vec<i32>,
    ) -> Result<(), QmfAssembleError> {
        if n_subs > NUM_SUBBAND {
            return Err(QmfAssembleError::SubsOutOfRange { n_subs });
        }

        // Spec line 175-178: select prCoeff once, outside the
        // per-sample loop.
        let pr_coeff = filter.coefficients();

        // Per-sample PCM scratch: write_pcm_output emits exactly
        // PCM_OUTPUT_PER_SAMPLE (= 32) samples at n_ch_index = 0, so a
        // 32-slot scratch always satisfies its length precondition. We
        // copy the scratch into `output` after each iteration, which
        // is the spec's `naCh[nChIndex++]` running append (nChIndex
        // advances 32 per sample) flattened into the caller's buffer.
        let mut scratch = [0_i32; PCM_OUTPUT_PER_SAMPLE];

        output.reserve(subband_samples.len() * PCM_OUTPUT_PER_SAMPLE);

        for row in subband_samples {
            // (a) raXin = active subbands then zero tail.
            let ra_xin = assemble_xin(row, n_subs)?;

            // (b) cosine-modulation refreshes raX[0..32]; the high
            //     block raX[32..512] holds the carried history.
            let low = cos_mod_stage(&ra_xin, &self.ra_cos_mod);
            self.ra_x[..NUM_SUBBAND].copy_from_slice(&low);

            // (c) 512-tap FIR convolution accumulates into raZ[0..64].
            fir_step(&self.ra_x, pr_coeff, &mut self.ra_z);

            // (d) int(rScale*raZ[i]) for the 32 low accumulator entries.
            //     The scratch is exactly 32 long, so n_ch_index = 0
            //     never trips OutputSliceTooShort.
            write_pcm_output(&self.ra_z, r_scale, &mut scratch, 0)?;
            output.extend_from_slice(&scratch);

            // (e) shift raX history up by 32 (freeing raX[0..32] for the
            //     next iteration's cos_mod write) and rotate raZ down by
            //     32 (carrying the next sample's partials into raZ[0..32]
            //     and clearing raZ[32..64]).
            shift_x_history(&mut self.ra_x);
            shift_z_output(&mut self.ra_z);
        }

        Ok(())
    }

    /// Borrow the current `raX[]` shift register (the §C.2.5 512-tap
    /// synthesis history). Exposed for callers that want to inspect or
    /// checkpoint the inter-subframe filter tail; the driver maintains
    /// it automatically across [`QmfSynthesis::synthesize`] calls.
    #[must_use]
    pub fn x_history(&self) -> &[f64; X_HISTORY_LEN] {
        &self.ra_x
    }

    /// Borrow the current `raZ[]` output accumulator (the §C.2.5
    /// 64-entry partial-sum buffer). Exposed for the same
    /// checkpoint/inspection use as [`QmfSynthesis::x_history`].
    #[must_use]
    pub fn z_accumulator(&self) -> &[f64; Z_OUTPUT_LEN] {
        &self.ra_z
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh filter starts with a fully-cleared history, matching the
    /// §C.2.5 per-channel filter state before the first subframe.
    #[test]
    fn new_starts_with_cleared_history() {
        let q = QmfSynthesis::new();
        assert!(q.x_history().iter().all(|&v| v == 0.0));
        assert!(q.z_accumulator().iter().all(|&v| v == 0.0));
        // The cosine-modulation matrix is precomputed (non-trivial).
        assert_eq!(q.ra_cos_mod.len(), COS_MOD_LEN);
    }

    /// Each per-sample row appends exactly 32 PCM samples to the
    /// output, per the §C.2.5 PCM-output step's `i < 32` loop.
    #[test]
    fn each_row_appends_32_pcm_samples() {
        let mut q = QmfSynthesis::new();
        let rows = vec![[0.0_f64; NUM_SUBBAND]; 4];
        let mut out = Vec::new();
        q.synthesize(
            &rows,
            32,
            FilterBankSelection::NonPerfectReconstruction,
            1.0,
            &mut out,
        )
        .unwrap();
        assert_eq!(out.len(), 4 * PCM_OUTPUT_PER_SAMPLE);
    }

    /// An all-zero subband input produces all-zero PCM, regardless of
    /// the filter selection or scale: the cosine-modulation of a zero
    /// vector is zero, the FIR convolution of a zero history is zero,
    /// and `int(rScale·0) == 0`.
    #[test]
    fn zero_input_yields_zero_pcm() {
        for filter in [
            FilterBankSelection::NonPerfectReconstruction,
            FilterBankSelection::PerfectReconstruction,
        ] {
            let mut q = QmfSynthesis::new();
            let rows = vec![[0.0_f64; NUM_SUBBAND]; 8];
            let mut out = Vec::new();
            q.synthesize(&rows, 16, filter, 32768.0, &mut out).unwrap();
            assert_eq!(out.len(), 8 * PCM_OUTPUT_PER_SAMPLE);
            assert!(out.iter().all(|&s| s == 0));
        }
    }

    /// The fused driver must produce byte-identical output to a manual
    /// hand-composition of the same per-sample primitives — this pins
    /// the driver as a faithful composition with no hidden state or
    /// reordering. We drive a small DC-impulse input through both.
    #[test]
    fn fused_driver_matches_manual_composition() {
        let n_subs = 4;
        let filter = FilterBankSelection::PerfectReconstruction;
        // A large scale so the impulse response truncates to non-zero
        // integer PCM (int(rScale·raZ) would round small responses to
        // zero at a small scale, making the non-vacuity check fail
        // without the equality check itself being wrong).
        let r_scale = 1_000_000.0;
        // A unit impulse in subband 0 followed by several silent rows —
        // exercises the inter-sample history shift (the impulse's
        // filter tail reaches later samples' output through raX[]).
        let mut row0 = [0.0_f64; NUM_SUBBAND];
        row0[0] = 1.0;
        let silent = [0.0_f64; NUM_SUBBAND];
        let rows = [row0, silent, silent, silent];

        // Fused.
        let mut q = QmfSynthesis::new();
        let mut fused = Vec::new();
        q.synthesize(&rows, n_subs, filter, r_scale, &mut fused)
            .unwrap();

        // Manual: replicate the §C.2.5 loop body with the same
        // primitives and the same starting (cleared) state.
        let cos = precal_cos_mod();
        let pr_coeff = filter.coefficients();
        let mut ra_x = [0.0_f64; X_HISTORY_LEN];
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        let mut manual = Vec::new();
        let mut scratch = [0_i32; PCM_OUTPUT_PER_SAMPLE];
        for row in &rows {
            let xin = assemble_xin(row, n_subs).unwrap();
            let low = cos_mod_stage(&xin, &cos);
            ra_x[..NUM_SUBBAND].copy_from_slice(&low);
            fir_step(&ra_x, pr_coeff, &mut ra_z);
            write_pcm_output(&ra_z, r_scale, &mut scratch, 0).unwrap();
            manual.extend_from_slice(&scratch);
            shift_x_history(&mut ra_x);
            shift_z_output(&mut ra_z);
        }

        assert_eq!(fused, manual);
        // The impulse must produce at least one non-zero PCM sample
        // (otherwise the test would pass vacuously on all-zero output).
        assert!(fused.iter().any(|&s| s != 0));
    }

    /// Persisting one [`QmfSynthesis`] across two `synthesize` calls
    /// must equal a single call over the concatenated input — the
    /// inter-subframe filter tail (`raX[]`) carries across calls.
    #[test]
    fn split_calls_match_single_concatenated_call() {
        let n_subs = 6;
        let filter = FilterBankSelection::NonPerfectReconstruction;
        let r_scale = 8192.0;
        // Build a deterministic non-trivial input.
        let mut rows = Vec::new();
        for s in 0..10 {
            let mut row = [0.0_f64; NUM_SUBBAND];
            for (i, slot) in row.iter_mut().take(n_subs).enumerate() {
                *slot = ((s * 7 + i * 3) % 11) as f64 - 5.0;
            }
            rows.push(row);
        }

        // Single call over all 10 rows.
        let mut q_single = QmfSynthesis::new();
        let mut single = Vec::new();
        q_single
            .synthesize(&rows, n_subs, filter, r_scale, &mut single)
            .unwrap();

        // Two calls split 4 + 6, reusing the same filter instance.
        let mut q_split = QmfSynthesis::new();
        let mut split = Vec::new();
        q_split
            .synthesize(&rows[..4], n_subs, filter, r_scale, &mut split)
            .unwrap();
        q_split
            .synthesize(&rows[4..], n_subs, filter, r_scale, &mut split)
            .unwrap();

        assert_eq!(single, split);
        assert!(single.iter().any(|&s| s != 0));
    }

    /// `n_subs > 32` is rejected before any sample is processed.
    #[test]
    fn rejects_n_subs_beyond_num_subband() {
        let mut q = QmfSynthesis::new();
        let rows = vec![[0.0_f64; NUM_SUBBAND]; 1];
        let mut out = Vec::new();
        assert_eq!(
            q.synthesize(
                &rows,
                33,
                FilterBankSelection::NonPerfectReconstruction,
                1.0,
                &mut out
            )
            .unwrap_err(),
            QmfAssembleError::SubsOutOfRange { n_subs: 33 }
        );
        // No output written and the history is untouched on the error.
        assert!(out.is_empty());
        assert!(q.x_history().iter().all(|&v| v == 0.0));
    }

    /// An empty input block is a no-op: no PCM is appended and the
    /// per-channel history is untouched (the §C.2.5 outer loop runs
    /// zero iterations when `nStart == nEnd`).
    #[test]
    fn empty_input_is_a_noop() {
        let mut q = QmfSynthesis::new();
        let mut out = Vec::new();
        q.synthesize(
            &[],
            8,
            FilterBankSelection::PerfectReconstruction,
            1.0,
            &mut out,
        )
        .unwrap();
        assert!(out.is_empty());
        assert!(q.x_history().iter().all(|&v| v == 0.0));
    }

    /// `n_subs = 0` is the spec's fully-silenced channel: `raXin` is
    /// all-zero, so every PCM sample is zero even with arbitrary scale.
    #[test]
    fn zero_active_subbands_is_silence() {
        let mut q = QmfSynthesis::new();
        let mut row = [0.0_f64; NUM_SUBBAND];
        // Even with non-zero values in the (inactive) subbands, n_subs=0
        // zero-fills the whole raXin.
        row.iter_mut().for_each(|v| *v = 123.0);
        let rows = vec![row; 3];
        let mut out = Vec::new();
        q.synthesize(
            &rows,
            0,
            FilterBankSelection::NonPerfectReconstruction,
            10000.0,
            &mut out,
        )
        .unwrap();
        assert_eq!(out.len(), 3 * PCM_OUTPUT_PER_SAMPLE);
        assert!(out.iter().all(|&s| s == 0));
    }
}

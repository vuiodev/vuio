//! Per-sample input assembly + shift-register update for the DTS
//! Core 32-band synthesis QMF.
//!
//! These two FIR-independent primitives bracket round 255's
//! [`crate::cos_mod_stage`] inside the §C.2.5 `QMFInterpolation()`
//! per-sample loop body, transcribed verbatim in
//! `docs/audio/dts/dts-core-extracts.md` §2.4 (ETSI TS 102 114
//! V1.3.1 Annex C §C.2.5, staged PDF p.185).
//!
//! Per the staged §2.4 pseudocode, the body of the outer
//! `for (nSubIndex=nStart; nSubIndex<nEnd; nSubIndex++)` loop is:
//!
//! ```text
//!     // (a) Per-sample raXin assembly — this module's
//!     //     assemble_xin().
//!     for (i=0;     i<nSUBS;       i++) raXin[i] = aSubband[i].raSample[nSubIndex];
//!     for (i=nSUBS; i<NumSubband;  i++) raXin[i] = 0.0;
//!
//!     // (b) Cosine-modulation stage — round 255 cos_mod_stage().
//!     //     Reads raXin[], raCosMod[], writes raX[0..32].
//!
//!     // (c) 512-tap FIR convolution against the §D.8 prCoeff set —
//!     //     this module's fir_step(). Reads raX[] and prCoeff
//!     //     (FilterBankSelection::coefficients()), accumulates
//!     //     into raZ[0..64].
//!     for (k=31,i=0; i<32; i++,k--)
//!         for (j=0; j<512; j+=64)
//!             raZ[i]    += prCoeff[i+j]   *( raX[i+j]-raX[j+k]);
//!     for (k=31,i=0; i<32; i++,k--)
//!         for (j=0; j<512; j+=64)
//!             raZ[32+i] += prCoeff[32+i+j]*(-raX[i+j]-raX[j+k]);
//!
//!     // (d) PCM output step — this module's write_pcm_output().
//!     //     Reads raZ[0..32], scales by rScale, int()-casts, writes
//!     //     32 samples to naCh[]. FIR-independent in structure (it
//!     //     consumes the already-accumulated raZ[0..32]).
//!     for (i=0; i<32; i++) naCh[nChIndex++] = int(rScale*raZ[i]);
//!
//!     // (e) Per-sample shift-register update — this module's
//!     //     shift_x_history() and shift_z_output().
//!     for (i=511; i>=32; i--)        raX[i]    = raX[i-32];
//!     for (i=0; i<NumSubband; i++)   raZ[i]    = raZ[i+32];
//!     for (i=0; i<NumSubband; i++)   raZ[i+32] = (real)0.0;
//! ```
//!
//! Steps (a) and (e)'s `raX` shift only depend on `nSUBS`, the
//! per-subband sample arrays, and the synthesis filter's `raX[]`
//! shift register. Step (c), [`fir_step`], convolves the §D.8
//! `raCoeffLossy` / `raCoeffLossLess` 512-tap FIR coefficient
//! tables ([`crate::RA_COEFF_LOSSY`] / [`crate::RA_COEFF_LOSSLESS`],
//! selected through
//! [`crate::FilterBankSelection::coefficients`]) against `raX[]`,
//! accumulating into the `raZ[]` output accumulator the PCM step
//! (d) reads.
//!
//! The `raZ[]` rotate at the end of step (e)
//! ([`shift_z_output`]) operates on the §C.2.5 output accumulator
//! `raZ[0..64]`: the FIR step (c) accumulates into it and the PCM
//! step (d) reads `raZ[0..32]` from it, but the rotate *itself* is
//! pure index manipulation — it shifts the low 32 entries down by
//! 32 and zeros the freed high block, never reading any §D.8 FIR
//! coefficient. It is therefore FIR-independent in exactly the same
//! way [`shift_x_history`] is, and lands here as the symmetric
//! companion shift.

use crate::cos_mod::NUM_SUBBAND;

/// Length of the synthesis filter's `raX[]` shift register, per
/// §C.2.5 / `dts-core-extracts.md` §2.4.
///
/// The staged pseudocode declares `raX[]` implicitly via its post-
/// PCM-output shift step:
///
/// ```text
///     for (i=511; i>=32; i--) raX[i] = raX[i-32];
/// ```
///
/// — the loop's upper bound (`i = 511`) fixes `raX[]` at 512
/// entries, matching the 512-tap `prCoeff` set (`raCoeffLossy` /
/// `raCoeffLossLess`, §D.8) the FIR step that drives `raX[]`
/// consumes.
pub const X_HISTORY_LEN: usize = 512;

/// Length of the synthesis filter's `raZ[]` output accumulator, per
/// §C.2.5 / `dts-core-extracts.md` §2.4.
///
/// The staged pseudocode indexes `raZ[]` at both `raZ[i]` and
/// `raZ[32+i]` for `i ∈ 0..32` (the FIR step writes `raZ[i]` and
/// `raZ[32+i]`; the PCM step reads `raZ[0..32]`; the rotate step
/// reads `raZ[i+32]` and writes `raZ[32+i]`):
///
/// ```text
///     for (i=0; i<NumSubband; i++)   raZ[i]    = raZ[i+32];
///     for (i=0; i<NumSubband; i++)   raZ[i+32] = (real)0.0;
/// ```
///
/// — so `raZ[]` spans indices `0..64`, i.e. `2 * NumSubband`
/// (`2 * 32 = 64`) entries.
pub const Z_OUTPUT_LEN: usize = 2 * NUM_SUBBAND;

/// Per-sample subband-input assembly step of the §C.2.5
/// `QMFInterpolation()` outer-loop body (per
/// `dts-core-extracts.md` §2.4, PDF p.185, lines 182-183 of the
/// staged pseudocode).
///
/// Builds the per-sample input vector `raXin[0..32]` that
/// [`crate::cos_mod_stage`] consumes by:
///
/// 1. Copying `subband_samples[i]` into `raXin[i]` for every
///    active subband `i ∈ 0..n_subs`, per the spec's
///    `for (i=0; i<nSUBS; i++) raXin[i] = aSubband[i].raSample[nSubIndex];`.
/// 2. Zero-filling `raXin[i]` for every inactive subband
///    `i ∈ n_subs..NumSubband`, per the spec's
///    `for (i=nSUBS; i<NumSubband; i++) raXin[i] = 0.0;`.
///
/// `subband_samples[i]` is the single sample `aSubband[i].raSample[nSubIndex]`
/// the spec reads at sample-index `nSubIndex` — the caller has
/// already selected `nSubIndex` from each active subband's sample
/// array. This narrows the assembly step to a pure vector-build
/// over per-subband scalars (independent of the per-subband sample
/// array layout that future bitstream-decode rounds will land).
///
/// `n_subs` is the spec's per-channel `nSUBS[ch]` — the count of
/// active subbands the channel transmitted, per the §C.2.5
/// `QMFInterpolation(FILTS, int nSUBS)` signature; it must be in
/// `0..=NUM_SUBBAND` (a `n_subs == 0` channel is a fully-silenced
/// pass through `raXin = [0.0; 32]`, valid by spec; `n_subs ==
/// NUM_SUBBAND` is the full 32-band case with no inactive tail).
///
/// Returns `Err(QmfAssembleError::SubsOutOfRange)` if
/// `n_subs > NUM_SUBBAND`, and `Err(QmfAssembleError::SampleSliceTooShort)`
/// if `subband_samples.len() < n_subs` (the caller didn't supply
/// one scalar per active subband). The per-call zero-fill of the
/// inactive tail is guaranteed even when the caller's
/// `subband_samples` slice is longer than `n_subs` — the spec's
/// zero-fill step ignores the high end past `nSUBS`.
pub fn assemble_xin(
    subband_samples: &[f64],
    n_subs: usize,
) -> Result<[f64; NUM_SUBBAND], QmfAssembleError> {
    if n_subs > NUM_SUBBAND {
        return Err(QmfAssembleError::SubsOutOfRange { n_subs });
    }
    if subband_samples.len() < n_subs {
        return Err(QmfAssembleError::SampleSliceTooShort {
            provided: subband_samples.len(),
            required: n_subs,
        });
    }

    // Step (a)(1): active subbands raXin[0..nSUBS]. Bulk-copy is
    // semantically identical to the spec's
    // `for (i=0; i<nSUBS; i++) raXin[i] = aSubband[i].raSample[nSubIndex];`
    // (the runtime-i and runtime-sample-array forms compile to the
    // same memcpy on contiguous f64s).
    let mut ra_xin = [0.0_f64; NUM_SUBBAND];
    ra_xin[..n_subs].copy_from_slice(&subband_samples[..n_subs]);
    // Step (a)(2): inactive subbands raXin[nSUBS..32] = 0.0.
    // The array starts at 0.0 from the [0.0_f64; NUM_SUBBAND]
    // initialiser above; no per-index re-zeroing is needed.
    Ok(ra_xin)
}

/// Per-sample shift-register update for the synthesis filter's
/// `raX[]` register, per `dts-core-extracts.md` §2.4 line 217
/// (`for (i=511; i>=32; i--) raX[i] = raX[i-32];`).
///
/// Rotates the 512-entry `raX[]` register by 32 entries toward the
/// high end: after the call, `raX[32..512]` holds what `raX[0..480]`
/// held on entry, and `raX[0..32]` is left untouched. The §C.2.5
/// driver overwrites `raX[0..32]` with the next per-sample
/// cosine-modulation output ([`crate::cos_mod_stage`]) before the
/// following FIR step reads it.
///
/// The shift runs from `i = 511` down to `i = 32` (inclusive) so
/// each write reads a slot that has not yet been overwritten —
/// directly translating the spec's reverse-iteration pseudocode.
/// `raX[0..32]` is left undefined after the shift (the spec's next
/// per-sample step writes those slots from `cos_mod_stage`'s
/// output before the FIR convolution reads them); callers are
/// expected to immediately overwrite that range before the next
/// FIR step.
///
/// This primitive is independent of the §D.8 `raCoeffLossy` /
/// `raCoeffLossLess` 512-tap FIR coefficient tables: it only
/// rotates the shift register's content, never reads any
/// coefficients.
pub fn shift_x_history(ra_x: &mut [f64; X_HISTORY_LEN]) {
    // Walk from i=511 down to i=32 (inclusive), writing
    // raX[i] = raX[i - 32]. The reverse iteration is essential —
    // forward iteration would overwrite low-index entries before
    // the high-index entries that depend on them are read.
    for i in (NUM_SUBBAND..X_HISTORY_LEN).rev() {
        ra_x[i] = ra_x[i - NUM_SUBBAND];
    }
}

/// Per-sample rotate of the synthesis filter's `raZ[]` output
/// accumulator, per `dts-core-extracts.md` §2.4 lines 218-219:
///
/// ```text
///     for (i=0; i<NumSubband; i++)   raZ[i]    = raZ[i+32];
///     for (i=0; i<NumSubband; i++)   raZ[i+32] = (real)0.0;
/// ```
///
/// Runs after the §C.2.5 PCM-output step has consumed `raZ[0..32]`
/// for the current per-sample iteration. The 32 high entries
/// `raZ[32..64]` (which the FIR step accumulated for the *next*
/// iteration's PCM output) slide down into `raZ[0..32]`, and the
/// freed high block `raZ[32..64]` is reset to `+0.0` so the next
/// per-sample FIR step accumulates into a cleared region.
///
/// The forward iteration (`i = 0` upward) is correct here — unlike
/// [`shift_x_history`]'s reverse walk — because the read range
/// `raZ[32..64]` and the write range `raZ[0..32]` are disjoint: no
/// write clobbers a slot a later read depends on. The subsequent
/// zero-fill of `raZ[32..64]` then overwrites the (now-stale) source
/// range, exactly mirroring the spec's two sequential loops.
///
/// This primitive reads no §D.8 `raCoeffLossy` / `raCoeffLossLess`
/// FIR coefficients — it only rotates and clears the accumulator's
/// content; the FIR convolution step that fills `raZ[]` is
/// [`fir_step`].
pub fn shift_z_output(ra_z: &mut [f64; Z_OUTPUT_LEN]) {
    // Loop 1: raZ[i] = raZ[i+32] for i in 0..32. The source range
    // [32, 64) and destination range [0, 32) are disjoint, so a
    // forward walk needs no scratch buffer (matching the spec's
    // straight `for (i=0; i<NumSubband; i++)`).
    for i in 0..NUM_SUBBAND {
        ra_z[i] = ra_z[i + NUM_SUBBAND];
    }
    // Loop 2: raZ[32+i] = 0.0 for i in 0..32. Reset the high block
    // to positive zero so the next per-sample FIR step accumulates
    // into a cleared region.
    for slot in ra_z.iter_mut().skip(NUM_SUBBAND) {
        *slot = 0.0;
    }
}

/// 512-tap FIR convolution step (c) of the §C.2.5
/// `QMFInterpolation()` per-sample loop body, per
/// `dts-core-extracts.md` §2.4 (PDF p.185):
///
/// ```text
///     // Multiply by filter coefficients
///     for (k=31,i=0; i<32; i++,k--)
///         for (j=0; j<512; j+=64)
///             raZ[i]    += prCoeff[i+j]   *( raX[i+j]-raX[j+k]);
///     for (k=31,i=0; i<32; i++,k--)
///         for (j=0; j<512; j+=64)
///             raZ[32+i] += prCoeff[32+i+j]*(-raX[i+j]-raX[j+k]);
/// ```
///
/// `pr_coeff` is the §D.8 512-tap coefficient set the §C.2.5 `FILTS`
/// branch selected — pass
/// [`crate::FilterBankSelection::coefficients`]'s result
/// ([`crate::RA_COEFF_LOSSY`] for `FILTS == 0`,
/// [`crate::RA_COEFF_LOSSLESS`] otherwise). `ra_x` is the 512-entry
/// shift register whose low block [`crate::cos_mod_stage`] just
/// refreshed (steps (b)/(e)); `ra_z` is the 64-entry output
/// accumulator this step **accumulates into** (`+=`, exactly as the
/// pseudocode writes it — the rotate step [`shift_z_output`] cleared
/// `raZ[32..64]` and carried the previous iteration's partials down
/// into `raZ[0..32]`, so the accumulation across two consecutive
/// per-sample iterations is what completes each output sample before
/// [`write_pcm_output`] consumes it).
///
/// Index shape, faithful to the pseudocode's `(k=31-i, j += 64)`
/// walk: the first loop reads `prCoeff[i + j]` (indices
/// `{0..32} + {0, 64, …, 448}`, i.e. the even 32-blocks) into
/// `raZ[0..32]`, the second reads `prCoeff[32 + i + j]` (the odd
/// 32-blocks) into `raZ[32..64]`; together the 2 × 32 × 8 reads
/// consume each of the 512 coefficients exactly once per call. Both
/// loops difference the shift register at `raX[i+j]` against the
/// mirrored slot `raX[j+k] = raX[j + 31 - i]` from the same
/// 64-entry window.
pub fn fir_step(
    ra_x: &[f64; X_HISTORY_LEN],
    pr_coeff: &[f64; X_HISTORY_LEN],
    ra_z: &mut [f64; Z_OUTPUT_LEN],
) {
    let window = 2 * NUM_SUBBAND; // the spec's `j += 64` stride
    for i in 0..NUM_SUBBAND {
        let k = NUM_SUBBAND - 1 - i; // spec: k starts at 31, k-- per i++
        for j in (0..X_HISTORY_LEN).step_by(window) {
            ra_z[i] += pr_coeff[i + j] * (ra_x[i + j] - ra_x[j + k]);
        }
    }
    for i in 0..NUM_SUBBAND {
        let k = NUM_SUBBAND - 1 - i;
        for j in (0..X_HISTORY_LEN).step_by(window) {
            ra_z[NUM_SUBBAND + i] += pr_coeff[NUM_SUBBAND + i + j] * (-ra_x[i + j] - ra_x[j + k]);
        }
    }
}

/// Number of PCM output samples the §C.2.5 PCM-output step emits per
/// per-sample iteration of the `QMFInterpolation()` outer loop, per
/// `dts-core-extracts.md` §2.4 lines 213-214:
///
/// ```text
///     for (i=0; i<32; i++) naCh[nChIndex++] = int(rScale*raZ[i]);
/// ```
///
/// — the loop bound (`i < 32`) fixes the per-iteration output count
/// at 32, equal to `NumSubband`. Each per-sample iteration of the
/// synthesis QMF consumes the 32 low entries of the `raZ[]`
/// accumulator and produces exactly 32 reconstructed PCM samples
/// into the channel output buffer.
pub const PCM_OUTPUT_PER_SAMPLE: usize = NUM_SUBBAND;

/// Per-sample PCM-output step of the §C.2.5 `QMFInterpolation()`
/// outer-loop body, per `dts-core-extracts.md` §2.4 lines 213-214
/// (PDF p.185):
///
/// ```text
///     for (i=0; i<32; i++) naCh[nChIndex++] = int(rScale*raZ[i]);
/// ```
///
/// Consumes the 32 low entries `raZ[0..32]` of the synthesis filter's
/// output accumulator (the FIR step (c) accumulated them for the
/// current per-sample iteration), scales each by the per-channel
/// output multiplier `r_scale`, applies the spec's `int()` cast, and
/// writes the 32 resulting integer PCM samples into `na_ch` starting
/// at the running cursor `n_ch_index`. Returns the advanced cursor
/// `n_ch_index + 32`, mirroring the spec's `naCh[nChIndex++]`
/// post-increment across the 32-iteration loop.
///
/// `r_scale` is the §C.2.5 `rScale` output multiplier the clause
/// applies before the integer cast. The §C.2.5 pseudocode uses
/// `rScale` in the PCM-output step without assigning it inside the
/// `QMFInterpolation()` block, so this function takes it as a
/// caller-supplied parameter rather than deriving a value (the
/// derivation of the QMF-output `rScale` is not fixed by the staged
/// §C.2.5 clause — see the docs gap noted in the round CHANGELOG).
///
/// The spec's `int()` cast truncates toward zero — the C semantics of
/// casting a floating value to `int` discard the fractional part — so
/// this step uses [`f64::trunc`] followed by an `as i32` conversion.
/// The §C.2.5 clause writes the result into the integer `naCh[]`
/// array without a stated per-width saturation; any clamping to the
/// transmitted source-PCM resolution
/// ([`crate::SourcePcmResolution`]) is a separate output-format step
/// the clause does not define here, so this primitive emits the
/// faithful truncated value.
///
/// This primitive reads no §D.8 `raCoeffLossy` / `raCoeffLossLess`
/// FIR coefficients: it consumes the already-accumulated `raZ[0..32]`
/// values, applies a scalar multiply + integer cast, and never
/// touches the coefficient tables. It is therefore FIR-independent in
/// the same way [`shift_z_output`] is, and is the companion of the
/// accumulator rotate it precedes in the per-sample loop body; the
/// FIR step that fills `raZ[]` is [`fir_step`].
///
/// Returns `Err(QmfAssembleError::OutputSliceTooShort)` if `na_ch`
/// does not have room for 32 samples starting at `n_ch_index` (the
/// caller's channel buffer is too small for this per-sample
/// iteration's output).
pub fn write_pcm_output(
    ra_z: &[f64; Z_OUTPUT_LEN],
    r_scale: f64,
    na_ch: &mut [i32],
    n_ch_index: usize,
) -> Result<usize, QmfAssembleError> {
    let end = n_ch_index.checked_add(PCM_OUTPUT_PER_SAMPLE).ok_or(
        QmfAssembleError::OutputSliceTooShort {
            n_ch_index,
            available: na_ch.len(),
        },
    )?;
    if end > na_ch.len() {
        return Err(QmfAssembleError::OutputSliceTooShort {
            n_ch_index,
            available: na_ch.len(),
        });
    }

    // for (i=0; i<32; i++) naCh[nChIndex++] = int(rScale*raZ[i]);
    //
    // The C `int()` cast truncates toward zero, so scale then trunc
    // then narrow to i32. Only raZ[0..32] is read — the accumulator's
    // high block raZ[32..64] holds the *next* iteration's pre-rotate
    // partial sums and is not part of this iteration's PCM output.
    for i in 0..PCM_OUTPUT_PER_SAMPLE {
        let scaled = r_scale * ra_z[i];
        na_ch[n_ch_index + i] = scaled.trunc() as i32;
    }

    Ok(end)
}

/// Error returned by [`assemble_xin`] when its inputs violate the
/// spec's preconditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum QmfAssembleError {
    /// `n_subs` exceeds the §C.2.5 `NumSubband = 32` cap, so the
    /// active-subband loop would write past the end of the
    /// `raXin[0..32]` vector.
    SubsOutOfRange {
        /// The out-of-range `n_subs` value the caller passed.
        n_subs: usize,
    },
    /// The caller supplied fewer than `n_subs` per-subband samples,
    /// so the assembly loop would read past the end of
    /// `subband_samples`.
    SampleSliceTooShort {
        /// `subband_samples.len()` — the number of per-subband
        /// scalars the caller supplied.
        provided: usize,
        /// The minimum length the §C.2.5 step requires
        /// (`n_subs`).
        required: usize,
    },
    /// The channel output buffer `na_ch` does not have room for the
    /// 32 PCM samples the §C.2.5 PCM-output step writes starting at
    /// `n_ch_index`, so the write loop would run past the end of the
    /// buffer.
    OutputSliceTooShort {
        /// The running output cursor `nChIndex` the write would
        /// start at.
        n_ch_index: usize,
        /// `na_ch.len()` — the number of `i32` slots the caller's
        /// channel buffer provides.
        available: usize,
    },
}

impl core::fmt::Display for QmfAssembleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            QmfAssembleError::SubsOutOfRange { n_subs } => {
                write!(
                    f,
                    "n_subs={n_subs} exceeds NumSubband={NUM_SUBBAND} for the §C.2.5 32-band synthesis QMF"
                )
            }
            QmfAssembleError::SampleSliceTooShort { provided, required } => {
                write!(
                    f,
                    "subband_samples.len()={provided} is shorter than the n_subs={required} per-sample scalars required by §C.2.5"
                )
            }
            QmfAssembleError::OutputSliceTooShort {
                n_ch_index,
                available,
            } => {
                write!(
                    f,
                    "na_ch.len()={available} has no room for the 32 §C.2.5 PCM samples at n_ch_index={n_ch_index}"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------
    // assemble_xin() — per-sample raXin assembly.
    // -----------------------------------------------------------

    #[test]
    fn assemble_xin_full_active_count_copies_all_thirty_two_entries() {
        // nSUBS = 32: every slot is an active subband, no
        // zero-fill tail.
        let samples: Vec<f64> = (0..NUM_SUBBAND).map(|i| (i as f64) + 0.5).collect();
        let ra_xin = assemble_xin(&samples, NUM_SUBBAND).expect("assemble succeeds at n_subs=32");
        for (i, v) in ra_xin.iter().enumerate() {
            assert_eq!(*v, (i as f64) + 0.5, "raXin[{i}] = {v} mismatch");
        }
    }

    #[test]
    fn assemble_xin_zero_active_count_produces_silent_input() {
        // nSUBS = 0: spec's first loop does nothing, second loop
        // zeros the entire raXin[0..32]. Valid by the §C.2.5
        // signature.
        let samples: Vec<f64> = vec![];
        let ra_xin = assemble_xin(&samples, 0).expect("assemble succeeds at n_subs=0");
        for (i, v) in ra_xin.iter().enumerate() {
            assert_eq!(*v, 0.0, "raXin[{i}] = {v} should be zero");
        }
    }

    #[test]
    fn assemble_xin_partial_active_count_zero_fills_inactive_tail() {
        // nSUBS = 5: raXin[0..5] gets the supplied samples,
        // raXin[5..32] = 0.0.
        let samples = [1.0, 2.0, 3.0, 4.0, 5.0];
        let ra_xin = assemble_xin(&samples, 5).expect("assemble succeeds at n_subs=5");
        for (i, expected) in samples.iter().enumerate() {
            assert_eq!(ra_xin[i], *expected, "raXin[{i}] active mismatch");
        }
        for (i, v) in ra_xin.iter().enumerate().skip(5) {
            assert_eq!(*v, 0.0, "raXin[{i}] should be zero-filled");
        }
    }

    #[test]
    fn assemble_xin_ignores_extra_trailing_samples_past_n_subs() {
        // §C.2.5's inactive-fill step zeros raXin past nSUBS even
        // if the caller's sample slice has more entries; the
        // assembly must follow nSUBS exactly, not the slice
        // length.
        let samples: Vec<f64> = (0..NUM_SUBBAND).map(|i| 100.0 + i as f64).collect();
        let ra_xin = assemble_xin(&samples, 7).expect("assemble succeeds at n_subs=7");
        for (i, v) in ra_xin.iter().enumerate().take(7) {
            assert_eq!(*v, 100.0 + i as f64, "raXin[{i}] active mismatch");
        }
        for (i, v) in ra_xin.iter().enumerate().skip(7) {
            assert_eq!(
                *v, 0.0,
                "raXin[{i}] should be zero past nSUBS regardless of slice length"
            );
        }
    }

    #[test]
    fn assemble_xin_rejects_n_subs_past_thirty_two() {
        // n_subs = 33 would write past raXin[31]; the §C.2.5
        // signature caps nSUBS at NumSubband = 32.
        let samples = [0.0_f64; 64];
        let err = assemble_xin(&samples, 33).expect_err("n_subs=33 rejected");
        assert_eq!(err, QmfAssembleError::SubsOutOfRange { n_subs: 33 });
    }

    #[test]
    fn assemble_xin_rejects_short_sample_slice() {
        // n_subs = 4 but only 2 samples supplied.
        let samples = [10.0, 20.0];
        let err = assemble_xin(&samples, 4).expect_err("short slice rejected");
        assert_eq!(
            err,
            QmfAssembleError::SampleSliceTooShort {
                provided: 2,
                required: 4
            }
        );
    }

    #[test]
    fn assemble_xin_accepts_exact_length_sample_slice() {
        // Boundary: subband_samples.len() == n_subs (no trailing
        // slack) — spec only requires nSUBS scalars.
        let samples = [7.5, 8.25, 9.125];
        let ra_xin = assemble_xin(&samples, 3).expect("exact-length slice accepted");
        assert_eq!(ra_xin[0], 7.5);
        assert_eq!(ra_xin[1], 8.25);
        assert_eq!(ra_xin[2], 9.125);
        for v in ra_xin.iter().skip(3) {
            assert_eq!(*v, 0.0);
        }
    }

    #[test]
    fn assemble_xin_preserves_negative_and_subnormal_values() {
        // Bit-identical pass-through for the active range:
        // §C.2.5 reads `raXin[i] = aSubband[i].raSample[...]`
        // without scaling. Cover signed + subnormal inputs to
        // confirm the copy preserves them verbatim.
        let samples = [-1.5_f64, f64::MIN_POSITIVE / 4.0, -0.0, 3.0];
        let ra_xin = assemble_xin(&samples, 4).expect("assemble succeeds");
        for (i, sample) in samples.iter().enumerate() {
            assert_eq!(
                ra_xin[i].to_bits(),
                sample.to_bits(),
                "raXin[{i}] bit-mismatch"
            );
        }
    }

    #[test]
    fn assemble_xin_inactive_tail_is_positive_zero() {
        // The spec writes `raXin[i] = 0.0;` — positive zero. A
        // negative-zero in the tail would be a spec deviation that
        // could perturb the cosine-modulation stage's behaviour at
        // `i=0`'s asymmetric B[k] = raXin[0] * raCosMod[…] step.
        let samples = [1.0];
        let ra_xin = assemble_xin(&samples, 1).expect("assemble succeeds");
        for (i, v) in ra_xin.iter().enumerate().skip(1) {
            assert_eq!(
                v.to_bits(),
                0.0_f64.to_bits(),
                "raXin[{i}] should be +0.0 bit-pattern"
            );
        }
    }

    // -----------------------------------------------------------
    // shift_x_history() — post-PCM shift of the raX[] register.
    // -----------------------------------------------------------

    #[test]
    fn shift_x_history_moves_low_half_to_high_half_by_thirty_two() {
        // After the shift, raX[32..512] should hold what
        // raX[0..480] held on entry.
        let mut ra_x = [0.0_f64; X_HISTORY_LEN];
        for (i, slot) in ra_x.iter_mut().enumerate() {
            *slot = i as f64;
        }
        let snapshot = ra_x;
        shift_x_history(&mut ra_x);
        for (i, v) in ra_x.iter().enumerate().skip(NUM_SUBBAND) {
            assert_eq!(
                *v,
                snapshot[i - NUM_SUBBAND],
                "raX[{i}] should equal pre-shift raX[{}]",
                i - NUM_SUBBAND
            );
        }
    }

    #[test]
    fn shift_x_history_leaves_first_thirty_two_entries_untouched() {
        // raX[0..32] is not written by the shift (the spec's loop
        // condition is `i >= 32`); the driver overwrites them
        // immediately afterwards via cos_mod_stage().
        let mut ra_x = [0.0_f64; X_HISTORY_LEN];
        for (i, slot) in ra_x.iter_mut().enumerate() {
            *slot = (i as f64) * 0.25;
        }
        let snapshot_low: Vec<f64> = ra_x[..NUM_SUBBAND].to_vec();
        shift_x_history(&mut ra_x);
        for (i, expected) in snapshot_low.iter().enumerate() {
            assert_eq!(
                ra_x[i], *expected,
                "raX[{i}] should be unchanged by the shift"
            );
        }
    }

    #[test]
    fn shift_x_history_is_identity_on_uniform_register() {
        // If every entry already holds the same value, the shift
        // is a no-op (each slot is replaced with an equal value).
        let mut ra_x = [4.25_f64; X_HISTORY_LEN];
        shift_x_history(&mut ra_x);
        for (i, v) in ra_x.iter().enumerate() {
            assert_eq!(*v, 4.25, "raX[{i}] = {v} should stay 4.25");
        }
    }

    #[test]
    fn shift_x_history_zeroes_propagate_into_low_block() {
        // A common §C.2.5 startup state: raX[] = 0.0 everywhere.
        // The shift is then a no-op and the register stays silent.
        let mut ra_x = [0.0_f64; X_HISTORY_LEN];
        shift_x_history(&mut ra_x);
        for (i, v) in ra_x.iter().enumerate() {
            assert_eq!(*v, 0.0, "raX[{i}] = {v} should stay 0");
        }
    }

    #[test]
    fn shift_x_history_top_block_after_shift_is_from_pre_shift_indices_448_to_479() {
        // Spot check: after the shift, raX[480..512] holds what
        // raX[448..480] held on entry.
        let mut ra_x = [0.0_f64; X_HISTORY_LEN];
        for (i, slot) in ra_x.iter_mut().enumerate() {
            *slot = (i as f64) - 256.0;
        }
        shift_x_history(&mut ra_x);
        for (i, v) in ra_x.iter().enumerate().skip(480) {
            let expected = ((i - NUM_SUBBAND) as f64) - 256.0;
            assert_eq!(
                *v,
                expected,
                "raX[{i}] should be pre-shift raX[{}] = {expected}",
                i - NUM_SUBBAND
            );
        }
    }

    #[test]
    fn shift_x_history_reverse_iteration_does_not_chain_overwrites() {
        // Sanity check: if the implementation walked forward
        // instead of in reverse, raX[32] would be overwritten with
        // raX[0], then raX[64] would be overwritten with raX[32]
        // (which is now raX[0]), etc. — every i ≡ 0 (mod 32) slot
        // would collapse to raX[0]'s original value.
        // Construct an input where this failure mode would be
        // visible (distinct values at the 32-step boundaries) and
        // confirm the reverse-walking implementation handles it
        // correctly.
        let mut ra_x = [0.0_f64; X_HISTORY_LEN];
        for (i, slot) in ra_x.iter_mut().enumerate() {
            *slot = i as f64; // raX[i] = i, all 512 values distinct
        }
        shift_x_history(&mut ra_x);
        // raX[64] should hold the pre-shift raX[32] = 32, not
        // raX[0] = 0 (the forward-iteration mistake).
        assert_eq!(ra_x[64], 32.0, "raX[64] reverse-shift mismatch");
        assert_eq!(ra_x[96], 64.0, "raX[96] reverse-shift mismatch");
        // And the top of the register holds the highest pre-shift
        // index that's still in range.
        assert_eq!(ra_x[511], (511 - NUM_SUBBAND) as f64);
    }

    #[test]
    fn shift_x_history_repeated_calls_walk_block_by_block() {
        // Two consecutive shifts displace the low block by 64
        // entries; three shifts displace it by 96; etc. — confirm
        // the primitive composes correctly across per-sample
        // iterations.
        let mut ra_x = [0.0_f64; X_HISTORY_LEN];
        for (i, slot) in ra_x.iter_mut().enumerate() {
            *slot = i as f64;
        }
        // Reserve raX[0..32] = sentinel before the first shift
        // (matching what cos_mod_stage would write into raX[0..32]
        // before the next per-sample iteration writes again).
        for v in ra_x.iter_mut().take(NUM_SUBBAND) {
            *v = -1.0;
        }
        shift_x_history(&mut ra_x);
        // After 1 shift, raX[64] = pre-shift raX[32] = 32.
        assert_eq!(ra_x[64], 32.0);
        // Reserve raX[0..32] = sentinel again.
        for v in ra_x.iter_mut().take(NUM_SUBBAND) {
            *v = -2.0;
        }
        shift_x_history(&mut ra_x);
        // After 2 shifts, raX[96] holds what raX[64] held one
        // shift ago, which held what raX[32] held before any
        // shift — i.e., raX[96] = 32.
        assert_eq!(ra_x[96], 32.0);
    }

    // -----------------------------------------------------------
    // shift_z_output() — post-PCM rotate of the raZ[] accumulator.
    // -----------------------------------------------------------

    #[test]
    fn shift_z_output_moves_high_block_down_into_low_block() {
        // After the rotate, raZ[0..32] should hold what raZ[32..64]
        // held on entry (the spec's `raZ[i] = raZ[i+32]`).
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        for (i, slot) in ra_z.iter_mut().enumerate() {
            *slot = i as f64;
        }
        let snapshot = ra_z;
        shift_z_output(&mut ra_z);
        for (i, v) in ra_z.iter().enumerate().take(NUM_SUBBAND) {
            assert_eq!(
                *v,
                snapshot[i + NUM_SUBBAND],
                "raZ[{i}] should equal pre-rotate raZ[{}]",
                i + NUM_SUBBAND
            );
        }
    }

    #[test]
    fn shift_z_output_zeros_the_high_block() {
        // After the rotate, raZ[32..64] should all be +0.0 (the
        // spec's `raZ[i+32] = 0.0`), readying it for the next
        // per-sample FIR accumulation.
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        for (i, slot) in ra_z.iter_mut().enumerate() {
            *slot = (i as f64) + 1.0; // all non-zero so a missed clear is visible
        }
        shift_z_output(&mut ra_z);
        for (i, v) in ra_z.iter().enumerate().skip(NUM_SUBBAND) {
            assert_eq!(
                v.to_bits(),
                0.0_f64.to_bits(),
                "raZ[{i}] should be cleared to +0.0"
            );
        }
    }

    #[test]
    fn shift_z_output_high_block_is_positive_zero_not_negative() {
        // The spec writes `(real)0.0` — positive zero. A negative
        // zero in the cleared block could perturb a later FIR
        // accumulation that starts from `raZ[32+i]`.
        let mut ra_z = [-3.0_f64; Z_OUTPUT_LEN];
        shift_z_output(&mut ra_z);
        for (i, v) in ra_z.iter().enumerate().skip(NUM_SUBBAND) {
            assert_eq!(
                v.to_bits(),
                0.0_f64.to_bits(),
                "raZ[{i}] should be the +0.0 bit-pattern, not -0.0"
            );
        }
    }

    #[test]
    fn shift_z_output_low_block_is_independent_of_prior_low_values() {
        // raZ[0..32] is fully overwritten by raZ[32..64]; the
        // pre-rotate low-block content must not leak through.
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        // Distinct sentinel in the low block; a known ramp in the
        // high block.
        for slot in ra_z.iter_mut().take(NUM_SUBBAND) {
            *slot = 999.0;
        }
        for (i, slot) in ra_z.iter_mut().enumerate().skip(NUM_SUBBAND) {
            *slot = (i - NUM_SUBBAND) as f64 - 100.0;
        }
        shift_z_output(&mut ra_z);
        for (i, v) in ra_z.iter().enumerate().take(NUM_SUBBAND) {
            assert_eq!(
                *v,
                (i as f64) - 100.0,
                "raZ[{i}] should come from the high block, not the prior low block"
            );
        }
    }

    #[test]
    fn shift_z_output_on_all_zero_accumulator_is_a_no_op() {
        // A common §C.2.5 startup state: raZ[] = 0.0 everywhere.
        // The rotate leaves it silent.
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        shift_z_output(&mut ra_z);
        for (i, v) in ra_z.iter().enumerate() {
            assert_eq!(*v, 0.0, "raZ[{i}] = {v} should stay 0");
        }
    }

    #[test]
    fn shift_z_output_preserves_signed_and_subnormal_high_block_values() {
        // The down-shift is a verbatim copy — signed and subnormal
        // f64s in the high block must arrive bit-identically in the
        // low block.
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        let specials = [-1.5_f64, f64::MIN_POSITIVE / 4.0, -0.0, 7.25];
        for (k, s) in specials.iter().enumerate() {
            ra_z[NUM_SUBBAND + k] = *s;
        }
        shift_z_output(&mut ra_z);
        for (k, s) in specials.iter().enumerate() {
            assert_eq!(
                ra_z[k].to_bits(),
                s.to_bits(),
                "raZ[{k}] bit-mismatch after down-shift"
            );
        }
    }

    #[test]
    fn shift_z_output_two_rotates_walk_the_accumulator_block_by_block() {
        // Simulate two per-sample iterations: between the rotates the
        // driver's FIR step would refill raZ[32..64]. Confirm the
        // first rotate exposes the original high block and the second
        // rotate exposes whatever the (simulated) FIR step wrote.
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        for (i, slot) in ra_z.iter_mut().enumerate().skip(NUM_SUBBAND) {
            *slot = (i - NUM_SUBBAND) as f64; // raZ[32..64] = 0..32
        }
        shift_z_output(&mut ra_z);
        // raZ[0..32] now holds 0..32; raZ[32..64] cleared.
        assert_eq!(ra_z[0], 0.0);
        assert_eq!(ra_z[31], 31.0);
        // Simulated FIR step for the next iteration refills the high
        // block with a fresh ramp.
        for (i, slot) in ra_z.iter_mut().enumerate().skip(NUM_SUBBAND) {
            *slot = 1000.0 + (i - NUM_SUBBAND) as f64;
        }
        shift_z_output(&mut ra_z);
        // The second rotate exposes the freshly-written high block.
        assert_eq!(ra_z[0], 1000.0);
        assert_eq!(ra_z[31], 1031.0);
        for v in ra_z.iter().skip(NUM_SUBBAND) {
            assert_eq!(*v, 0.0, "high block should be cleared after second rotate");
        }
    }

    // -----------------------------------------------------------
    // fir_step() — §C.2.5 512-tap FIR convolution step (c).
    // -----------------------------------------------------------

    /// Verbatim line-for-line transcription of the §C.2.5 FIR
    /// pseudocode (C-style index walk with the paired `i++, k--`
    /// updates and the `j += 64` stride), used as a bit-exact
    /// reference against the production [`fir_step`].
    fn reference_fir_step(
        ra_x: &[f64; X_HISTORY_LEN],
        pr_coeff: &[f64; X_HISTORY_LEN],
        ra_z: &mut [f64; Z_OUTPUT_LEN],
    ) {
        // for (k=31,i=0; i<32; i++,k--)
        //     for (j=0; j<512; j+=64)
        //         raZ[i] += prCoeff[i+j] * ( raX[i+j]-raX[j+k]);
        let mut k: isize = 31;
        let mut i: isize = 0;
        while i < 32 {
            let mut j: isize = 0;
            while j < 512 {
                ra_z[i as usize] +=
                    pr_coeff[(i + j) as usize] * (ra_x[(i + j) as usize] - ra_x[(j + k) as usize]);
                j += 64;
            }
            i += 1;
            k -= 1;
        }
        // for (k=31,i=0; i<32; i++,k--)
        //     for (j=0; j<512; j+=64)
        //         raZ[32+i] += prCoeff[32+i+j] * (-raX[i+j]-raX[j+k]);
        let mut k: isize = 31;
        let mut i: isize = 0;
        while i < 32 {
            let mut j: isize = 0;
            while j < 512 {
                ra_z[(32 + i) as usize] += pr_coeff[(32 + i + j) as usize]
                    * (-ra_x[(i + j) as usize] - ra_x[(j + k) as usize]);
                j += 64;
            }
            i += 1;
            k -= 1;
        }
    }

    /// Deterministic pseudo-random `raX[]` fill (multiplicative LCG;
    /// no external randomness) spanning distinct signs/magnitudes.
    fn pseudo_random_x(seed: u64) -> [f64; X_HISTORY_LEN] {
        let mut state = seed;
        let mut ra_x = [0.0_f64; X_HISTORY_LEN];
        for slot in ra_x.iter_mut() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            // Map the top 32 bits onto roughly [-1, 1).
            *slot = ((state >> 32) as i64 - (1_i64 << 31)) as f64 / (1_i64 << 31) as f64;
        }
        ra_x
    }

    #[test]
    fn fir_step_on_silent_register_leaves_accumulator_unchanged() {
        // raX[] = 0 → every (raX[i+j] - raX[j+k]) term is zero; the
        // accumulator (pre-loaded with sentinels) must be unchanged
        // for both §D.8 coefficient sets.
        use crate::fir_coeff::{RA_COEFF_LOSSLESS, RA_COEFF_LOSSY};
        let ra_x = [0.0_f64; X_HISTORY_LEN];
        for table in [&RA_COEFF_LOSSLESS, &RA_COEFF_LOSSY] {
            let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
            for (i, slot) in ra_z.iter_mut().enumerate() {
                *slot = 100.0 + i as f64;
            }
            let snapshot = ra_z;
            fir_step(&ra_x, table, &mut ra_z);
            assert_eq!(ra_z, snapshot, "silent raX must not move the accumulator");
        }
    }

    #[test]
    fn fir_step_accumulates_into_ra_z_instead_of_overwriting() {
        // The pseudocode writes `raZ[…] += …`; pre-loaded partials
        // from the previous per-sample iteration must survive and the
        // accumulation must walk in the spec's order. Run both the
        // production step and the verbatim reference from the SAME
        // non-zero accumulator and require bit-identical results.
        use crate::fir_coeff::RA_COEFF_LOSSLESS;
        let ra_x = pseudo_random_x(7);
        let mut got = [0.0_f64; Z_OUTPUT_LEN];
        for (i, slot) in got.iter_mut().enumerate() {
            *slot = 1000.0 + i as f64;
        }
        let preload = got;
        let mut expected = got;
        fir_step(&ra_x, &RA_COEFF_LOSSLESS, &mut got);
        reference_fir_step(&ra_x, &RA_COEFF_LOSSLESS, &mut expected);
        for i in 0..Z_OUTPUT_LEN {
            assert_eq!(
                got[i].to_bits(),
                expected[i].to_bits(),
                "raZ[{i}] mismatch vs reference on preloaded accumulator"
            );
            assert_ne!(
                got[i], preload[i],
                "raZ[{i}] must have received a contribution"
            );
        }
        // Exact-arithmetic spot check: a single dyadic tap on a
        // preloaded slot adds exactly.
        let mut pr_coeff = [0.0_f64; X_HISTORY_LEN];
        pr_coeff[64] = 1.0;
        let mut ra_x = [0.0_f64; X_HISTORY_LEN];
        ra_x[64] = 5.0;
        ra_x[95] = 2.0;
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        ra_z[0] = 1000.0;
        fir_step(&ra_x, &pr_coeff, &mut ra_z);
        assert_eq!(ra_z[0], 1003.0, "raZ[0] = 1000.0 + 1.0*(5.0-2.0)");
    }

    #[test]
    fn fir_step_matches_verbatim_reference_bit_exactly_on_both_d8_tables() {
        // Bit-exact (to_bits) agreement with the line-for-line
        // §C.2.5 transcription across ramp, alternating-sign, and
        // pseudo-random raX[] fills, for both §D.8 sets.
        use crate::fir_coeff::{RA_COEFF_LOSSLESS, RA_COEFF_LOSSY};
        let mut ramp = [0.0_f64; X_HISTORY_LEN];
        for (i, slot) in ramp.iter_mut().enumerate() {
            *slot = (i as f64) * 0.001 - 0.25;
        }
        let mut alternating = [0.0_f64; X_HISTORY_LEN];
        for (i, slot) in alternating.iter_mut().enumerate() {
            *slot = if i % 2 == 0 { 1.0 } else { -1.0 };
        }
        let inputs = [ramp, alternating, pseudo_random_x(1), pseudo_random_x(42)];
        for table in [&RA_COEFF_LOSSLESS, &RA_COEFF_LOSSY] {
            for (n, ra_x) in inputs.iter().enumerate() {
                let mut got = [0.0_f64; Z_OUTPUT_LEN];
                let mut expected = [0.0_f64; Z_OUTPUT_LEN];
                fir_step(ra_x, table, &mut got);
                reference_fir_step(ra_x, table, &mut expected);
                for i in 0..Z_OUTPUT_LEN {
                    assert_eq!(
                        got[i].to_bits(),
                        expected[i].to_bits(),
                        "raZ[{i}] mismatch vs reference on input #{n}"
                    );
                }
            }
        }
    }

    #[test]
    fn fir_step_low_half_tap_maps_pr_coeff_i_plus_j() {
        // Isolate prCoeff[64]: it is read exactly once, by the first
        // loop at (i=0, j=64, k=31), contributing
        // prCoeff[64] * (raX[64] - raX[95]) to raZ[0].
        let mut pr_coeff = [0.0_f64; X_HISTORY_LEN];
        pr_coeff[64] = 1.0;
        let mut ra_x = [0.0_f64; X_HISTORY_LEN];
        ra_x[64] = 5.0;
        ra_x[95] = 2.0; // j + k = 64 + 31
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        fir_step(&ra_x, &pr_coeff, &mut ra_z);
        assert_eq!(ra_z[0], 3.0, "raZ[0] = prCoeff[64]*(raX[64]-raX[95])");
        for (i, v) in ra_z.iter().enumerate().skip(1) {
            assert_eq!(*v, 0.0, "raZ[{i}] must receive no contribution");
        }
    }

    #[test]
    fn fir_step_high_half_tap_maps_pr_coeff_32_plus_i_plus_j() {
        // Isolate prCoeff[32]: it is read exactly once, by the second
        // loop at (i=0, j=0, k=31), contributing
        // prCoeff[32] * (-raX[0] - raX[31]) to raZ[32].
        let mut pr_coeff = [0.0_f64; X_HISTORY_LEN];
        pr_coeff[32] = 1.0;
        let mut ra_x = [0.0_f64; X_HISTORY_LEN];
        ra_x[0] = 2.0;
        ra_x[31] = 3.0; // j + k = 0 + 31
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        fir_step(&ra_x, &pr_coeff, &mut ra_z);
        assert_eq!(ra_z[32], -5.0, "raZ[32] = prCoeff[32]*(-raX[0]-raX[31])");
        for (i, v) in ra_z.iter().enumerate() {
            if i != 32 {
                assert_eq!(*v, 0.0, "raZ[{i}] must receive no contribution");
            }
        }
    }

    #[test]
    fn fir_step_uses_each_coefficient_exactly_eight_taps_per_output() {
        // With prCoeff ≡ 1 and raX ≡ c: the first loop's terms are
        // (c - c) = 0, so raZ[0..32] stays silent; the second loop's
        // terms are (-c - c) = -2c, eight j-steps each, so
        // raZ[32..64] = 8 * (-2c) = -16c — confirming the 8-tap
        // (j = 0, 64, …, 448) walk per output slot.
        let pr_coeff = [1.0_f64; X_HISTORY_LEN];
        let ra_x = [0.5_f64; X_HISTORY_LEN];
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        fir_step(&ra_x, &pr_coeff, &mut ra_z);
        for (i, v) in ra_z.iter().enumerate().take(NUM_SUBBAND) {
            assert_eq!(*v, 0.0, "raZ[{i}]: (c - c) terms must cancel");
        }
        for (i, v) in ra_z.iter().enumerate().skip(NUM_SUBBAND) {
            assert_eq!(*v, -16.0 * 0.5, "raZ[{i}]: 8 taps of -2c expected");
        }
    }

    #[test]
    fn fir_step_is_linear_in_the_shift_register() {
        // The step is a fixed linear map of raX[] (sums of products
        // against constant coefficients); scaling the register by a
        // power of two scales the accumulation exactly.
        use crate::fir_coeff::RA_COEFF_LOSSY;
        let ra_x = pseudo_random_x(99);
        let mut doubled = ra_x;
        for slot in doubled.iter_mut() {
            *slot *= 2.0;
        }
        let mut z1 = [0.0_f64; Z_OUTPUT_LEN];
        let mut z2 = [0.0_f64; Z_OUTPUT_LEN];
        fir_step(&ra_x, &RA_COEFF_LOSSY, &mut z1);
        fir_step(&doubled, &RA_COEFF_LOSSY, &mut z2);
        for i in 0..Z_OUTPUT_LEN {
            assert_eq!(
                z2[i].to_bits(),
                (2.0 * z1[i]).to_bits(),
                "raZ[{i}]: doubling raX must exactly double the contribution"
            );
        }
    }

    // -----------------------------------------------------------
    // write_pcm_output() — per-sample PCM-output step.
    // -----------------------------------------------------------

    #[test]
    fn write_pcm_output_emits_thirty_two_samples_and_advances_cursor() {
        // raZ[0..32] = i, rScale = 1.0 → naCh[i] = int(1.0*i) = i.
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        for (i, slot) in ra_z.iter_mut().take(NUM_SUBBAND).enumerate() {
            *slot = i as f64;
        }
        let mut na_ch = [0_i32; NUM_SUBBAND];
        let next = write_pcm_output(&ra_z, 1.0, &mut na_ch, 0).expect("fits exactly");
        assert_eq!(next, NUM_SUBBAND, "cursor advances by 32");
        for (i, v) in na_ch.iter().enumerate() {
            assert_eq!(*v, i as i32, "naCh[{i}] mismatch");
        }
    }

    #[test]
    fn write_pcm_output_applies_scale_before_cast() {
        // rScale = 4.0, raZ[i] = i → naCh[i] = int(4.0*i) = 4*i.
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        for (i, slot) in ra_z.iter_mut().take(NUM_SUBBAND).enumerate() {
            *slot = i as f64;
        }
        let mut na_ch = [0_i32; NUM_SUBBAND];
        write_pcm_output(&ra_z, 4.0, &mut na_ch, 0).expect("fits");
        for (i, v) in na_ch.iter().enumerate() {
            assert_eq!(*v, 4 * i as i32, "naCh[{i}] = 4*{i} expected");
        }
    }

    #[test]
    fn write_pcm_output_truncates_toward_zero() {
        // int() in C discards the fractional part toward zero for both
        // signs: int(2.9) = 2, int(-2.9) = -2, int(-0.4) = 0.
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        ra_z[0] = 2.9;
        ra_z[1] = -2.9;
        ra_z[2] = -0.4;
        ra_z[3] = 0.99;
        ra_z[4] = -0.99;
        let mut na_ch = [0_i32; NUM_SUBBAND];
        write_pcm_output(&ra_z, 1.0, &mut na_ch, 0).expect("fits");
        assert_eq!(na_ch[0], 2, "int(2.9) = 2");
        assert_eq!(na_ch[1], -2, "int(-2.9) = -2");
        assert_eq!(na_ch[2], 0, "int(-0.4) = 0 (toward zero)");
        assert_eq!(na_ch[3], 0, "int(0.99) = 0");
        assert_eq!(na_ch[4], 0, "int(-0.99) = 0");
    }

    #[test]
    fn write_pcm_output_scaling_then_truncation_order_matters() {
        // The cast happens AFTER the scale: int(0.5 * 3.0) = int(1.5)
        // = 1, not int(0.5)*3 = 0.
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        ra_z[0] = 3.0;
        let mut na_ch = [0_i32; NUM_SUBBAND];
        write_pcm_output(&ra_z, 0.5, &mut na_ch, 0).expect("fits");
        assert_eq!(na_ch[0], 1, "int(0.5 * 3.0) = int(1.5) = 1");
    }

    #[test]
    fn write_pcm_output_writes_at_running_cursor_and_returns_new_cursor() {
        // A buffer of three iterations' worth; write into the second
        // slot-block and confirm the first and third are untouched.
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        for (i, slot) in ra_z.iter_mut().take(NUM_SUBBAND).enumerate() {
            *slot = (100 + i) as f64;
        }
        let mut na_ch = [-1_i32; 3 * NUM_SUBBAND];
        let next = write_pcm_output(&ra_z, 1.0, &mut na_ch, NUM_SUBBAND).expect("fits");
        assert_eq!(next, 2 * NUM_SUBBAND, "cursor advances 32→64");
        // First block untouched.
        for v in &na_ch[..NUM_SUBBAND] {
            assert_eq!(*v, -1, "first block must not be written");
        }
        // Second block holds the output.
        for (i, v) in na_ch[NUM_SUBBAND..2 * NUM_SUBBAND].iter().enumerate() {
            assert_eq!(*v, 100 + i as i32, "naCh[{}] mismatch", NUM_SUBBAND + i);
        }
        // Third block untouched.
        for v in &na_ch[2 * NUM_SUBBAND..] {
            assert_eq!(*v, -1, "third block must not be written");
        }
    }

    #[test]
    fn write_pcm_output_reads_only_low_block_not_high_accumulator() {
        // raZ[32..64] holds the next iteration's pre-rotate partials;
        // they must NOT leak into this iteration's output.
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        for slot in ra_z.iter_mut().skip(NUM_SUBBAND) {
            *slot = 9999.0; // high block — must be ignored
        }
        let mut na_ch = [0_i32; NUM_SUBBAND];
        write_pcm_output(&ra_z, 1.0, &mut na_ch, 0).expect("fits");
        for (i, v) in na_ch.iter().enumerate() {
            assert_eq!(
                *v, 0,
                "naCh[{i}] must come from raZ[0..32]=0, not the high block"
            );
        }
    }

    #[test]
    fn write_pcm_output_rejects_buffer_too_short() {
        let ra_z = [0.0_f64; Z_OUTPUT_LEN];
        let mut na_ch = [0_i32; NUM_SUBBAND - 1]; // one slot short
        let err = write_pcm_output(&ra_z, 1.0, &mut na_ch, 0).unwrap_err();
        assert_eq!(
            err,
            QmfAssembleError::OutputSliceTooShort {
                n_ch_index: 0,
                available: NUM_SUBBAND - 1,
            }
        );
    }

    #[test]
    fn write_pcm_output_rejects_cursor_past_room() {
        // Buffer has room for exactly 32 samples but the cursor starts
        // at 1, so 1+32 = 33 > 32.
        let ra_z = [0.0_f64; Z_OUTPUT_LEN];
        let mut na_ch = [0_i32; NUM_SUBBAND];
        let err = write_pcm_output(&ra_z, 1.0, &mut na_ch, 1).unwrap_err();
        assert_eq!(
            err,
            QmfAssembleError::OutputSliceTooShort {
                n_ch_index: 1,
                available: NUM_SUBBAND,
            }
        );
    }

    #[test]
    fn write_pcm_output_negative_scale_flips_sign() {
        // A negative rScale negates each sample before the cast.
        let mut ra_z = [0.0_f64; Z_OUTPUT_LEN];
        ra_z[0] = 5.0;
        ra_z[1] = -3.0;
        let mut na_ch = [0_i32; NUM_SUBBAND];
        write_pcm_output(&ra_z, -2.0, &mut na_ch, 0).expect("fits");
        assert_eq!(na_ch[0], -10, "int(-2.0 * 5.0) = -10");
        assert_eq!(na_ch[1], 6, "int(-2.0 * -3.0) = 6");
    }

    #[test]
    fn write_pcm_output_per_sample_constant_is_num_subband() {
        assert_eq!(PCM_OUTPUT_PER_SAMPLE, NUM_SUBBAND);
        assert_eq!(PCM_OUTPUT_PER_SAMPLE, 32);
    }

    #[test]
    fn write_pcm_output_error_renders_human_readable_message() {
        let err = QmfAssembleError::OutputSliceTooShort {
            n_ch_index: 64,
            available: 80,
        };
        let msg = format!("{err}");
        assert!(msg.contains("64"), "message names the cursor: {msg}");
        assert!(
            msg.contains("80"),
            "message names the available length: {msg}"
        );
    }

    // -----------------------------------------------------------
    // Constants
    // -----------------------------------------------------------

    #[test]
    fn z_output_len_is_sixty_four() {
        // §C.2.5 / §2.4 lines 218-219 index raZ[] at raZ[i] and
        // raZ[i+32] for i in 0..32, so the accumulator spans
        // 2 * NumSubband = 64 entries.
        assert_eq!(Z_OUTPUT_LEN, 64);
    }

    #[test]
    fn z_output_len_is_twice_num_subband() {
        // The rotate writes raZ[i] = raZ[i+32] and raZ[32+i] = 0.0;
        // both index ranges fit exactly when Z_OUTPUT_LEN = 2 *
        // NUM_SUBBAND.
        assert_eq!(Z_OUTPUT_LEN, 2 * NUM_SUBBAND);
    }

    #[test]
    fn x_history_len_is_five_hundred_twelve() {
        // §C.2.5 / §2.4 line 217 caps `raX[]` at 512 entries,
        // matching the 512-tap §D.8 FIR set the driver consumes.
        assert_eq!(X_HISTORY_LEN, 512);
    }

    #[test]
    fn x_history_len_is_a_whole_multiple_of_num_subband() {
        // The shift step writes `raX[i] = raX[i-32]`, which would
        // overrun or under-fill the register if X_HISTORY_LEN
        // weren't a multiple of NUM_SUBBAND. 512 = 16 * 32.
        assert_eq!(X_HISTORY_LEN % NUM_SUBBAND, 0);
        assert_eq!(X_HISTORY_LEN / NUM_SUBBAND, 16);
    }

    // -----------------------------------------------------------
    // Error rendering
    // -----------------------------------------------------------

    #[test]
    fn subs_out_of_range_error_renders_human_readable_message() {
        let err = QmfAssembleError::SubsOutOfRange { n_subs: 64 };
        let s = format!("{err}");
        assert!(s.contains("64"), "message should include the bad n_subs");
        assert!(
            s.contains("NumSubband"),
            "message should reference the spec's cap"
        );
    }

    #[test]
    fn sample_slice_too_short_error_renders_provided_and_required() {
        let err = QmfAssembleError::SampleSliceTooShort {
            provided: 2,
            required: 5,
        };
        let s = format!("{err}");
        assert!(s.contains("2"), "message should include provided length");
        assert!(s.contains("5"), "message should include required length");
    }
}

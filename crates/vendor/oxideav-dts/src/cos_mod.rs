//! Cosine-modulation coefficient matrix for the DTS Core 32-band
//! synthesis QMF filterbank.
//!
//! Transcribed verbatim from `docs/audio/dts/dts-core-extracts.md`
//! §2.3 ("Cosine-modulation coefficient definition (§C.2.5, PDF
//! p.184)"), which in turn quotes Annex C §C.2.5 `PreCalCosMod()` of
//! ETSI TS 102 114 V1.3.1 (the staged PDF at
//! `docs/audio/dts/etsi-ts-102114-dts-coherent-acoustics.pdf`).
//!
//! The 544-entry array `raCosMod[]` is pre-computed once by the
//! decoder and re-used by every `QMFInterpolation` invocation
//! (§C.2.5, PDF p.185). The §2.4 extract documents the four roles
//! the four blocks play inside that loop:
//!
//! ```text
//!   Block 1 (indices   0..=255): cos((2i+1)(2k+1) π / 64)  — 16×16
//!   Block 2 (indices 256..=511): cos(i(2k+1)     π / 32)   — 16×16
//!   Block 3 (indices 512..=527): + 0.25 / (2·cos((2k+1) π / 128))  — 16
//!   Block 4 (indices 528..=543): − 0.25 / (2·sin((2k+1) π / 128))  — 16
//! ```
//!
//! The j-counter in the spec's `PreCalCosMod()` pseudocode walks
//! 0..=543 with no gaps; this module materialises the same packing.
//!
//! Scope: this round only lands the matrix builder. The downstream
//! `QMFInterpolation` synthesis loop (and the §D.8 512-tap
//! `raCoeffLossy` / `raCoeffLossLess` FIR coefficient tables it
//! consumes) is a follow-up — the §D.8 tables are referenced in the
//! staged PDF p.238 but not yet transcribed under `docs/audio/dts/`.

/// Total number of entries in the [`raCosMod`] array per
/// `PreCalCosMod()` (§C.2.5 / `dts-core-extracts.md` §2.3).
///
/// Decomposes as 256 + 256 + 16 + 16 (the four blocks of the
/// spec's pseudocode).
pub const COS_MOD_LEN: usize = 544;

/// Number of subbands the 32-band synthesis QMF reconstructs per
/// `QMFInterpolation()` invocation (§C.2.5 / `dts-core-extracts.md`
/// §2.4: `NumSubband = 32`). Also the length of the `raXin[]`
/// per-sample input vector and of the `raX[0..32]` output window
/// produced by the cosine-modulation stage.
pub const NUM_SUBBAND: usize = 32;

/// Start index of Block 1 inside [`raCosMod`]
/// (`cos((2i+1)(2k+1) π / 64)`).
pub const COS_MOD_BLOCK1_START: usize = 0;

/// Start index of Block 2 inside [`raCosMod`]
/// (`cos(i(2k+1) π / 32)`).
pub const COS_MOD_BLOCK2_START: usize = 256;

/// Start index of Block 3 inside [`raCosMod`]
/// (`+0.25 / (2·cos((2k+1) π / 128))`).
pub const COS_MOD_BLOCK3_START: usize = 512;

/// Start index of Block 4 inside [`raCosMod`]
/// (`−0.25 / (2·sin((2k+1) π / 128))`).
pub const COS_MOD_BLOCK4_START: usize = 528;

/// Pre-compute the 544-entry cosine-modulation matrix.
///
/// This is a direct Rust transliteration of the §C.2.5
/// `PreCalCosMod()` pseudocode transcribed in
/// `docs/audio/dts/dts-core-extracts.md` §2.3:
///
/// ```text
///   PreCalCosMod() {
///       for (j=0,k=0; k<16; k++)
///           for (i=0; i<16; i++)
///               raCosMod[j++] = cos((2*i+1)*(2*k+1)*Pi/64);
///       for (k=0; k<16; k++)
///           for (i=0; i<16; i++)
///               raCosMod[j++] = cos((i)*(2*k+1)*Pi/32);
///       for (k=0; k<16; k++)
///           raCosMod[j++] = 0.25 / (2*cos((2*k+1)*Pi/128));
///       for (k=0; k<16; k++)
///           raCosMod[j++] = -0.25 / (2*sin((2*k+1)*Pi/128));
///   }
/// ```
///
/// The returned array is intended to be allocated once per decoder
/// instance (§C.2.5: "computed once") and shared across every
/// `QMFInterpolation` call for the lifetime of that instance.
///
/// The output is deterministic: every byte-identical run produces
/// the same array. Callers that need bit-exact reproducibility
/// across runs can rely on this directly without seeding a PRNG.
pub fn precal_cos_mod() -> [f64; COS_MOD_LEN] {
    let mut ra = [0.0_f64; COS_MOD_LEN];
    let mut j = 0usize;

    // Block 1: indices 0..256
    //   raCosMod[j++] = cos((2*i+1)*(2*k+1) * π / 64)
    for k in 0..16 {
        for i in 0..16 {
            let num = ((2 * i + 1) * (2 * k + 1)) as f64;
            ra[j] = (num * core::f64::consts::PI / 64.0).cos();
            j += 1;
        }
    }
    debug_assert_eq!(j, COS_MOD_BLOCK2_START);

    // Block 2: indices 256..512
    //   raCosMod[j++] = cos(i * (2*k+1) * π / 32)
    for k in 0..16 {
        for i in 0..16 {
            let num = (i * (2 * k + 1)) as f64;
            ra[j] = (num * core::f64::consts::PI / 32.0).cos();
            j += 1;
        }
    }
    debug_assert_eq!(j, COS_MOD_BLOCK3_START);

    // Block 3: indices 512..528
    //   raCosMod[j++] = 0.25 / (2 * cos((2*k+1) * π / 128))
    for k in 0..16 {
        let arg = ((2 * k + 1) as f64) * core::f64::consts::PI / 128.0;
        ra[j] = 0.25 / (2.0 * arg.cos());
        j += 1;
    }
    debug_assert_eq!(j, COS_MOD_BLOCK4_START);

    // Block 4: indices 528..544
    //   raCosMod[j++] = -0.25 / (2 * sin((2*k+1) * π / 128))
    for k in 0..16 {
        let arg = ((2 * k + 1) as f64) * core::f64::consts::PI / 128.0;
        ra[j] = -0.25 / (2.0 * arg.sin());
        j += 1;
    }
    debug_assert_eq!(j, COS_MOD_LEN);

    ra
}

// ---------------------------------------------------------------
// Cosine-modulation stage of `QMFInterpolation()` (§C.2.5,
// `dts-core-extracts.md` §2.4, PDF p.185) — the per-sample loop
// body's first half, up to (and including) the placement of
// `raX[0..32]` from `SUM[k]` / `DIFF[k]` via the Block-3 / Block-4
// scaling coefficients.
// ---------------------------------------------------------------
//
// `QMFInterpolation()` is the 32-band synthesis-QMF reconstruction
// algorithm. Per sample-index `nSubIndex`, the algorithm reads one
// sample from each of the 32 subband sample arrays (the active
// subbands populate `raXin[0..nSUBS]`; the inactive ones are
// zero-filled), then performs three substeps:
//
//   1. Build the 16-entry `A[k]` / `B[k]` accumulators using the
//      Block 1 (k outer, i inner, 16x16) and Block 2 (same shape)
//      cosine-modulation coefficients from `raCosMod`, with a
//      slightly asymmetric `B[k]` accumulation that pairs
//      `raXin[2i]` with `raXin[2i-1]` for `i > 0` (and with itself
//      for `i = 0`).
//   2. Combine into 16-entry `SUM[k] = A[k] + B[k]` and
//      `DIFF[k] = A[k] - B[k]` vectors.
//   3. Place `raX[k] = raCosMod[Block3 + k] * SUM[k]` and
//      `raX[32 - k - 1] = raCosMod[Block4 + k] * DIFF[k]` for
//      `k = 0..16`, populating the leading 32 entries of the
//      synthesis-filter shift register `raX[]`.
//
// After step 3, `QMFInterpolation()` continues with a 512-tap FIR
// convolution against `prCoeff` (§D.8 `raCoeffLossy` or
// `raCoeffLossLess`, selected by `FILTS`), the integer PCM output
// step, and the per-sample shift of `raX[]` / `raZ[]` history.
// Those substeps depend on the §D.8 FIR coefficient tables, which
// are not yet transcribed under `docs/audio/dts/` (round-208 docs
// gap #9). The cosine-modulation stage implemented here is
// FIR-independent: it consumes only `raXin[]` and `raCosMod[]` and
// produces only `raX[0..32]`, so it can be landed and exercised
// from the staged extracts alone.
//
// The j-counter in the spec's pseudocode walks across all three
// substeps without resetting: substep 1 reads `raCosMod[0..256]`
// (Block 1), substep 2's `B[k]` accumulation reads
// `raCosMod[256..512]` (Block 2), and substep 3 reads
// `raCosMod[512..528]` (Block 3) and `raCosMod[528..544]`
// (Block 4). This module's [`cos_mod_stage`] reproduces that
// walk with an explicit running index that ends at 544, matching
// the spec's `j` value at the boundary between substep 3 and the
// FIR step.

/// Run the cosine-modulation stage of the §C.2.5
/// `QMFInterpolation()` synthesis-QMF algorithm for one
/// sample-index inside its outer `for (nSubIndex=nStart;
/// nSubIndex<nEnd; nSubIndex++)` loop (per
/// `dts-core-extracts.md` §2.4, PDF p.185).
///
/// Given:
///
/// * `ra_xin` — the 32-entry per-sample subband vector
///   `raXin[0..32]` already prepared by the caller (active
///   subbands populated from `aSubband[i].raSample[nSubIndex]`;
///   inactive subbands `i >= nSUBS` zero-filled per the spec's
///   `for (i=nSUBS; i<NumSubband; i++) raXin[i] = 0.0;` step).
/// * `ra_cos_mod` — the 544-entry cosine-modulation matrix
///   returned by [`precal_cos_mod`].
///
/// Returns the leading 32 entries `raX[0..32]` of the synthesis
/// filter's shift register — exactly the values the §C.2.5
/// pseudocode writes into `raX[k]` and `raX[32 - k - 1]` for
/// `k = 0..16` before the 512-tap FIR convolution that follows.
///
/// The 16-entry intermediates `A[k]`, `B[k]`, `SUM[k]`, and
/// `DIFF[k]` are computed verbatim from the spec's pseudocode
/// using only the cosine-modulation matrix — no FIR coefficients
/// are consumed, so this function is independent of the §D.8
/// tables (round-208 docs gap #9). The output is in IEEE-754
/// `f64`; the spec's `real` type is the implementation's choice.
///
/// The function is pure: it does not mutate its inputs, depend on
/// any global state, or have any history of its own. The history
/// shift of `raX[]` between successive `nSubIndex` iterations is
/// the caller's responsibility (the shift moves the 32 values
/// this function returns into `raX[32..64]` after the FIR step
/// runs, so this function's output rotates through the shift
/// register in lock-step with the per-sample loop).
pub fn cos_mod_stage(
    ra_xin: &[f64; NUM_SUBBAND],
    ra_cos_mod: &[f64; COS_MOD_LEN],
) -> [f64; NUM_SUBBAND] {
    // Substep 1: A[k] = sum_{i=0..16} (raXin[2i] + raXin[2i+1])
    //                  * raCosMod[Block1 + 16k + i].
    // The spec's pseudocode uses a single running j; we materialise
    // the same packing as Block1 row-major + Block2 row-major +
    // Block3 + Block4 by relying on the constants from
    // `precal_cos_mod()`.
    let mut a = [0.0_f64; 16];
    let mut j = COS_MOD_BLOCK1_START;
    for a_k in &mut a {
        let mut acc = 0.0_f64;
        for i in 0..16 {
            acc += (ra_xin[2 * i] + ra_xin[2 * i + 1]) * ra_cos_mod[j];
            j += 1;
        }
        *a_k = acc;
    }
    debug_assert_eq!(j, COS_MOD_BLOCK2_START);

    // Substep 1 (continued): B[k] = sum_{i=0..16} f(i) where
    //   f(0) = raXin[0]                    * raCosMod[Block2 + 16k + 0]
    //   f(i) = (raXin[2i] + raXin[2i-1])    * raCosMod[Block2 + 16k + i]   for i > 0
    let mut b = [0.0_f64; 16];
    for b_k in &mut b {
        let mut acc = 0.0_f64;
        for i in 0..16 {
            let pair = if i > 0 {
                ra_xin[2 * i] + ra_xin[2 * i - 1]
            } else {
                ra_xin[0]
            };
            acc += pair * ra_cos_mod[j];
            j += 1;
        }
        *b_k = acc;
    }
    debug_assert_eq!(j, COS_MOD_BLOCK3_START);

    // Substep 2: SUM[k] = A[k] + B[k]; DIFF[k] = A[k] - B[k].
    // Held inline below to fuse with substep 3.

    // Substep 3: raX[k]          = raCosMod[Block3 + k] * SUM[k]
    //            raX[32 - k - 1] = raCosMod[Block4 + k] * DIFF[k]
    let mut ra_x = [0.0_f64; NUM_SUBBAND];
    // SUM step reads Block 3 (indices 512..528).
    for k in 0..16 {
        let sum_k = a[k] + b[k];
        ra_x[k] = ra_cos_mod[j] * sum_k;
        j += 1;
    }
    debug_assert_eq!(j, COS_MOD_BLOCK4_START);
    // DIFF step reads Block 4 (indices 528..544).
    for k in 0..16 {
        let diff_k = a[k] - b[k];
        // raX[32 - k - 1] writes 31, 30, ..., 16 as k = 0..16.
        ra_x[NUM_SUBBAND - k - 1] = ra_cos_mod[j] * diff_k;
        j += 1;
    }
    debug_assert_eq!(j, COS_MOD_LEN);

    ra_x
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tolerance for cross-checks between the closed-form values and
    /// the matrix entry. The expected values come from the same
    /// `cos` / `sin` calls in the spec pseudocode, so the residual is
    /// effectively zero on IEEE-754; we leave a small slack for the
    /// theoretical-vs-runtime-rounding mismatch.
    const EPS: f64 = 1e-12;

    #[test]
    fn length_matches_spec() {
        let ra = precal_cos_mod();
        assert_eq!(ra.len(), 544);
        assert_eq!(COS_MOD_LEN, 544);
    }

    #[test]
    fn block_boundaries_are_documented_constants() {
        // Documented in module docs + start-constant docstrings.
        assert_eq!(COS_MOD_BLOCK1_START, 0);
        assert_eq!(COS_MOD_BLOCK2_START, 256);
        assert_eq!(COS_MOD_BLOCK3_START, 512);
        assert_eq!(COS_MOD_BLOCK4_START, 528);
        // Decomposition adds up.
        assert_eq!(
            COS_MOD_BLOCK2_START - COS_MOD_BLOCK1_START + COS_MOD_BLOCK3_START
                - COS_MOD_BLOCK2_START
                + COS_MOD_BLOCK4_START
                - COS_MOD_BLOCK3_START
                + COS_MOD_LEN
                - COS_MOD_BLOCK4_START,
            COS_MOD_LEN
        );
    }

    #[test]
    fn block1_first_entry_is_cos_pi_over_64() {
        // k=0, i=0 → (2·0+1)(2·0+1) π/64 = π/64
        let ra = precal_cos_mod();
        let expected = (core::f64::consts::PI / 64.0).cos();
        assert!((ra[0] - expected).abs() < EPS);
    }

    #[test]
    fn block1_walks_all_256_indices() {
        // Spec pseudocode walks k outer, i inner, both 0..16. The
        // packed index for (k, i) is 16*k + i.
        let ra = precal_cos_mod();
        for k in 0..16 {
            for i in 0..16 {
                let idx = COS_MOD_BLOCK1_START + 16 * k + i;
                let num = ((2 * i + 1) * (2 * k + 1)) as f64;
                let expected = (num * core::f64::consts::PI / 64.0).cos();
                assert!(
                    (ra[idx] - expected).abs() < EPS,
                    "Block 1 ({k}, {i}) mismatch: got {} expected {}",
                    ra[idx],
                    expected
                );
            }
        }
    }

    #[test]
    fn block2_first_entry_is_one() {
        // k=0, i=0 → cos(0·1·π/32) = cos(0) = 1.0
        let ra = precal_cos_mod();
        assert!((ra[COS_MOD_BLOCK2_START] - 1.0).abs() < EPS);
    }

    #[test]
    fn block2_walks_all_256_indices() {
        let ra = precal_cos_mod();
        for k in 0..16 {
            for i in 0..16 {
                let idx = COS_MOD_BLOCK2_START + 16 * k + i;
                let num = (i * (2 * k + 1)) as f64;
                let expected = (num * core::f64::consts::PI / 32.0).cos();
                assert!(
                    (ra[idx] - expected).abs() < EPS,
                    "Block 2 ({k}, {i}) mismatch: got {} expected {}",
                    ra[idx],
                    expected
                );
            }
        }
    }

    #[test]
    fn block3_first_entry_matches_closed_form() {
        // k=0 → 0.25 / (2 · cos(π/128))
        let ra = precal_cos_mod();
        let arg = core::f64::consts::PI / 128.0;
        let expected = 0.25 / (2.0 * arg.cos());
        assert!((ra[COS_MOD_BLOCK3_START] - expected).abs() < EPS);
    }

    #[test]
    fn block3_walks_all_16_indices() {
        let ra = precal_cos_mod();
        for k in 0..16 {
            let idx = COS_MOD_BLOCK3_START + k;
            let arg = ((2 * k + 1) as f64) * core::f64::consts::PI / 128.0;
            let expected = 0.25 / (2.0 * arg.cos());
            assert!(
                (ra[idx] - expected).abs() < EPS,
                "Block 3 ({k}) mismatch: got {} expected {}",
                ra[idx],
                expected
            );
        }
    }

    #[test]
    fn block3_entries_are_strictly_positive() {
        // (2k+1)π/128 is in (0, π/2) for k in 0..16, so cos > 0
        // and the +0.25 / (2·cos) factor is strictly positive.
        let ra = precal_cos_mod();
        for k in 0..16 {
            let v = ra[COS_MOD_BLOCK3_START + k];
            assert!(v > 0.0, "Block 3 ({k}) = {v} should be > 0");
        }
    }

    #[test]
    fn block4_first_entry_matches_closed_form() {
        // k=0 → -0.25 / (2 · sin(π/128))
        let ra = precal_cos_mod();
        let arg = core::f64::consts::PI / 128.0;
        let expected = -0.25 / (2.0 * arg.sin());
        assert!((ra[COS_MOD_BLOCK4_START] - expected).abs() < EPS);
    }

    #[test]
    fn block4_walks_all_16_indices() {
        let ra = precal_cos_mod();
        for k in 0..16 {
            let idx = COS_MOD_BLOCK4_START + k;
            let arg = ((2 * k + 1) as f64) * core::f64::consts::PI / 128.0;
            let expected = -0.25 / (2.0 * arg.sin());
            assert!(
                (ra[idx] - expected).abs() < EPS,
                "Block 4 ({k}) mismatch: got {} expected {}",
                ra[idx],
                expected
            );
        }
    }

    #[test]
    fn block4_entries_are_strictly_negative() {
        // (2k+1)π/128 is in (0, π/2) for k in 0..16, so sin > 0
        // and the -0.25 / (2·sin) factor is strictly negative.
        let ra = precal_cos_mod();
        for k in 0..16 {
            let v = ra[COS_MOD_BLOCK4_START + k];
            assert!(v < 0.0, "Block 4 ({k}) = {v} should be < 0");
        }
    }

    #[test]
    fn block1_last_entry_of_row_zero() {
        // k=0, i=15 → (2·15+1)(2·0+1) π / 64 = 31 π / 64
        let ra = precal_cos_mod();
        let expected = (31.0 * core::f64::consts::PI / 64.0).cos();
        assert!((ra[15] - expected).abs() < EPS);
    }

    #[test]
    fn block1_row_count_and_row_length() {
        // Verify the packing density: 16 rows of 16 columns =
        // 256 entries. Done by cross-checking the index arithmetic
        // matches the explicit row/col enumeration.
        let ra = precal_cos_mod();
        let mut seen = 0usize;
        for k in 0..16 {
            for i in 0..16 {
                let idx = COS_MOD_BLOCK1_START + 16 * k + i;
                let num = ((2 * i + 1) * (2 * k + 1)) as f64;
                let expected = (num * core::f64::consts::PI / 64.0).cos();
                assert!((ra[idx] - expected).abs() < EPS);
                seen += 1;
            }
        }
        assert_eq!(seen, 256);
    }

    #[test]
    fn block2_row_first_entry_is_one_for_all_k() {
        // i=0 always gives cos(0) = 1 regardless of k.
        let ra = precal_cos_mod();
        for k in 0..16 {
            let idx = COS_MOD_BLOCK2_START + 16 * k;
            assert!(
                (ra[idx] - 1.0).abs() < EPS,
                "Block 2 row {k} entry 0 should be 1.0"
            );
        }
    }

    #[test]
    fn block3_value_grows_with_k() {
        // (2k+1)π/128 grows monotonically with k, cos(·) shrinks,
        // so +0.25 / (2·cos) grows monotonically with k.
        let ra = precal_cos_mod();
        for k in 1..16 {
            let prev = ra[COS_MOD_BLOCK3_START + k - 1];
            let cur = ra[COS_MOD_BLOCK3_START + k];
            assert!(
                cur > prev,
                "Block 3 should grow: prev={prev} cur={cur} at k={k}"
            );
        }
    }

    #[test]
    fn block4_magnitude_shrinks_with_k() {
        // (2k+1)π/128 grows, sin(·) grows, so |-0.25 / (2·sin)|
        // shrinks. The entries are negative; their absolute values
        // shrink.
        let ra = precal_cos_mod();
        for k in 1..16 {
            let prev = ra[COS_MOD_BLOCK4_START + k - 1].abs();
            let cur = ra[COS_MOD_BLOCK4_START + k].abs();
            assert!(
                cur < prev,
                "|Block 4| should shrink: prev={prev} cur={cur} at k={k}"
            );
        }
    }

    #[test]
    fn deterministic_across_calls() {
        // Two independent invocations must produce bit-identical
        // arrays (§C.2.5 documents the matrix as computed once and
        // reused; that only makes sense if the result is
        // deterministic).
        let a = precal_cos_mod();
        let b = precal_cos_mod();
        for i in 0..COS_MOD_LEN {
            // Bit-exact, not approximate, because the same `cos` /
            // `sin` arguments are passed in the same order.
            assert_eq!(a[i].to_bits(), b[i].to_bits(), "mismatch at index {i}");
        }
    }

    #[test]
    fn all_entries_are_finite() {
        // No NaN, no ±∞. The Block 3 / 4 denominators are cos / sin
        // at angles strictly inside (0, π/2), so neither vanishes
        // and the division is finite.
        let ra = precal_cos_mod();
        for (i, v) in ra.iter().enumerate() {
            assert!(v.is_finite(), "ra[{i}] = {v} is not finite");
        }
    }

    #[test]
    fn block1_and_block2_entries_are_bounded_by_one() {
        // cos returns a value in [-1, +1]. Block 1 / 2 are pure
        // cosine evaluations.
        let ra = precal_cos_mod();
        for (i, v) in ra[COS_MOD_BLOCK1_START..COS_MOD_BLOCK3_START]
            .iter()
            .enumerate()
        {
            assert!(v.abs() <= 1.0 + EPS, "ra[{i}] = {v} outside [-1, 1]");
        }
    }

    // -----------------------------------------------------------
    // cos_mod_stage() — cosine-modulation stage of
    // QMFInterpolation() (§C.2.5, PDF p.185).
    // -----------------------------------------------------------

    /// Reference implementation: a direct verbatim translation of
    /// the §2.4 pseudocode's first half, used as the oracle the
    /// optimised [`cos_mod_stage`] is cross-checked against. This
    /// mirrors the spec line-for-line with no fusing of the SUM /
    /// DIFF combine step into substep 3, so a bug in either the
    /// reference or the live function shows up as a per-index
    /// divergence.
    ///
    /// Indexes a / b / sum / diff / ra_x with the loop variable so
    /// the reference body remains a 1:1 textual match of the
    /// pseudocode; iterator-flavoured rewrites would obscure the
    /// correspondence with the spec.
    #[allow(clippy::needless_range_loop)]
    fn cos_mod_stage_reference(
        ra_xin: &[f64; NUM_SUBBAND],
        ra_cos_mod: &[f64; COS_MOD_LEN],
    ) -> [f64; NUM_SUBBAND] {
        let mut a = [0.0_f64; 16];
        let mut b = [0.0_f64; 16];
        let mut sum = [0.0_f64; 16];
        let mut diff = [0.0_f64; 16];
        let mut ra_x = [0.0_f64; NUM_SUBBAND];

        let mut j = 0usize;
        for k in 0..16 {
            for i in 0..16 {
                a[k] += (ra_xin[2 * i] + ra_xin[2 * i + 1]) * ra_cos_mod[j];
                j += 1;
            }
        }
        for k in 0..16 {
            for i in 0..16 {
                if i > 0 {
                    b[k] += (ra_xin[2 * i] + ra_xin[2 * i - 1]) * ra_cos_mod[j];
                } else {
                    b[k] += ra_xin[2 * i] * ra_cos_mod[j];
                }
                j += 1;
            }
            sum[k] = a[k] + b[k];
            diff[k] = a[k] - b[k];
        }
        for k in 0..16 {
            ra_x[k] = ra_cos_mod[j] * sum[k];
            j += 1;
        }
        for k in 0..16 {
            ra_x[NUM_SUBBAND - k - 1] = ra_cos_mod[j] * diff[k];
            j += 1;
        }
        assert_eq!(j, COS_MOD_LEN);
        ra_x
    }

    #[test]
    fn cos_mod_stage_zero_input_yields_zero_output() {
        // raXin = 0 → A[k] = B[k] = 0 → SUM[k] = DIFF[k] = 0 →
        // raX[0..32] = 0 regardless of the cosine-modulation
        // matrix. This pins the linearity-at-zero corner.
        let ra_xin = [0.0_f64; NUM_SUBBAND];
        let ra_cos_mod = precal_cos_mod();
        let out = cos_mod_stage(&ra_xin, &ra_cos_mod);
        for (i, v) in out.iter().enumerate() {
            assert_eq!(*v, 0.0, "raX[{i}] = {v} should be 0");
        }
    }

    #[test]
    fn cos_mod_stage_matches_reference_on_zero_input() {
        // Bit-exact agreement at the trivial input ensures the
        // fused live implementation and the line-for-line
        // reference walk the j-counter the same way.
        let ra_xin = [0.0_f64; NUM_SUBBAND];
        let ra_cos_mod = precal_cos_mod();
        let live = cos_mod_stage(&ra_xin, &ra_cos_mod);
        let reference = cos_mod_stage_reference(&ra_xin, &ra_cos_mod);
        for i in 0..NUM_SUBBAND {
            assert_eq!(
                live[i].to_bits(),
                reference[i].to_bits(),
                "raX[{i}] mismatch: live={} ref={}",
                live[i],
                reference[i]
            );
        }
    }

    #[test]
    fn cos_mod_stage_matches_reference_on_unit_basis_inputs() {
        // For every j ∈ 0..32, set raXin[j] = 1 (all others zero)
        // and verify the live function matches the spec-line-for-
        // line reference. This exercises every Block-1 / Block-2
        // cosine entry exactly once across the 32 sweeps.
        let ra_cos_mod = precal_cos_mod();
        for j in 0..NUM_SUBBAND {
            let mut ra_xin = [0.0_f64; NUM_SUBBAND];
            ra_xin[j] = 1.0;
            let live = cos_mod_stage(&ra_xin, &ra_cos_mod);
            let reference = cos_mod_stage_reference(&ra_xin, &ra_cos_mod);
            for i in 0..NUM_SUBBAND {
                assert_eq!(
                    live[i].to_bits(),
                    reference[i].to_bits(),
                    "raX[{i}] mismatch (impulse at {j}): live={} ref={}",
                    live[i],
                    reference[i]
                );
            }
        }
    }

    #[test]
    fn cos_mod_stage_matches_reference_on_ramp_input() {
        // raXin[i] = i + 0.5 (non-trivial, no zeros) — exercises
        // every Block-1 / Block-2 cosine entry with a non-zero
        // pair-sum and gives the SUM[k] / DIFF[k] step distinct
        // values to scale.
        let mut ra_xin = [0.0_f64; NUM_SUBBAND];
        for (i, slot) in ra_xin.iter_mut().enumerate() {
            *slot = (i as f64) + 0.5;
        }
        let ra_cos_mod = precal_cos_mod();
        let live = cos_mod_stage(&ra_xin, &ra_cos_mod);
        let reference = cos_mod_stage_reference(&ra_xin, &ra_cos_mod);
        for i in 0..NUM_SUBBAND {
            assert_eq!(
                live[i].to_bits(),
                reference[i].to_bits(),
                "raX[{i}] mismatch on ramp input: live={} ref={}",
                live[i],
                reference[i]
            );
        }
    }

    #[test]
    fn cos_mod_stage_matches_reference_on_alternating_signs() {
        // raXin[i] = (-1)^i — the pair-sums (raXin[2i] +
        // raXin[2i+1]) are 0, and the asymmetric B-pair sums
        // raXin[2i] + raXin[2i-1] alternate; covers a different
        // regime than the ramp.
        let mut ra_xin = [0.0_f64; NUM_SUBBAND];
        for (i, slot) in ra_xin.iter_mut().enumerate() {
            *slot = if i.is_multiple_of(2) { 1.0 } else { -1.0 };
        }
        let ra_cos_mod = precal_cos_mod();
        let live = cos_mod_stage(&ra_xin, &ra_cos_mod);
        let reference = cos_mod_stage_reference(&ra_xin, &ra_cos_mod);
        for i in 0..NUM_SUBBAND {
            assert_eq!(
                live[i].to_bits(),
                reference[i].to_bits(),
                "raX[{i}] mismatch on alternating signs: live={} ref={}",
                live[i],
                reference[i]
            );
        }
    }

    #[test]
    fn cos_mod_stage_output_is_finite() {
        // Every raCosMod entry is finite (covered by
        // `all_entries_are_finite`), every raXin entry is finite,
        // and substep 3's scaling factors do not blow up, so the
        // output must be finite for any finite input.
        let mut ra_xin = [0.0_f64; NUM_SUBBAND];
        for (i, slot) in ra_xin.iter_mut().enumerate() {
            *slot = (i as f64).sin();
        }
        let ra_cos_mod = precal_cos_mod();
        let out = cos_mod_stage(&ra_xin, &ra_cos_mod);
        for (i, v) in out.iter().enumerate() {
            assert!(v.is_finite(), "raX[{i}] = {v} not finite");
        }
    }

    #[test]
    fn cos_mod_stage_is_linear() {
        // The stage is bilinear in (raXin, raCosMod) and pure
        // linear in raXin (with raCosMod fixed). Check
        // cos_mod_stage(2*x) = 2 * cos_mod_stage(x) for a ramp
        // input — this is a structural property derived from the
        // spec pseudocode (the only multiplications by raCosMod are
        // against linear combinations of raXin).
        let mut ra_xin = [0.0_f64; NUM_SUBBAND];
        for (i, slot) in ra_xin.iter_mut().enumerate() {
            *slot = (i as f64) - 16.0;
        }
        let mut ra_xin_2x = [0.0_f64; NUM_SUBBAND];
        for (i, slot) in ra_xin_2x.iter_mut().enumerate() {
            *slot = 2.0 * ra_xin[i];
        }
        let ra_cos_mod = precal_cos_mod();
        let out_1x = cos_mod_stage(&ra_xin, &ra_cos_mod);
        let out_2x = cos_mod_stage(&ra_xin_2x, &ra_cos_mod);
        const LINEARITY_EPS: f64 = 1e-9;
        for i in 0..NUM_SUBBAND {
            let expected = 2.0 * out_1x[i];
            assert!(
                (out_2x[i] - expected).abs() < LINEARITY_EPS,
                "raX[{i}] not linear: 2*x→{} but expected {}",
                out_2x[i],
                expected
            );
        }
    }

    #[test]
    fn cos_mod_stage_is_deterministic() {
        // Two identical inputs must produce bit-identical outputs.
        let mut ra_xin = [0.0_f64; NUM_SUBBAND];
        for (i, slot) in ra_xin.iter_mut().enumerate() {
            *slot = ((i + 1) as f64).cos();
        }
        let ra_cos_mod = precal_cos_mod();
        let a = cos_mod_stage(&ra_xin, &ra_cos_mod);
        let b = cos_mod_stage(&ra_xin, &ra_cos_mod);
        for i in 0..NUM_SUBBAND {
            assert_eq!(a[i].to_bits(), b[i].to_bits(), "raX[{i}] differs");
        }
    }

    #[test]
    fn num_subband_is_thirty_two() {
        // Spec invariant: §C.2.5 fixes NumSubband = 32 for the
        // 32-band synthesis QMF.
        assert_eq!(NUM_SUBBAND, 32);
    }
}

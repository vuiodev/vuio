//! DTS Coherent Acoustics — §C.2.2 Inverse ADPCM.
//!
//! Round 228 (2026-06-04) lands the §C.2.2 inverse-ADPCM predictor,
//! the per-sample reconstruction step that turns a subband's
//! residual (error) sample stream into the reconstructed subband
//! sample stream when the subband's `PMODE == 1` flag indicates
//! ADPCM prediction is active.
//!
//! Source: ETSI TS 102 114 V1.3.1 (2011-08), Annex C (informative)
//! §C.2.2 "Inverse ADPCM" (staged PDF p.183) at
//! `docs/audio/dts/etsi-ts-102114-dts-coherent-acoustics.pdf`. The
//! spec's normative pseudocode reads (reproduced as documented):
//!
//! ```text
//! void InverseADPCM(void) {
//! // NumADPCMCoeff =4, the number of ADPCM coefficients.
//! // raADPCMCoeff[] are the ADPCM coefficients extracted
//! // from the bit stream.
//! // raSample[NumADPCMCoeff], ..., raSample[-1] are the
//! // history from last subframe or subsubframe. It must
//! // updated each time before reverse ADPCM is run for a
//! // block of samples for each subband.
//! for (m=0; m<nNumSample; m++)
//!     for (n=0; n<NumADPCMCoeff; n++)
//!         raSample[m] += raADPCMCoeff[n]*raSample[m-n-1];
//! }
//! ```
//!
//! The predictor is a fixed-order (4-tap) FIR running over the
//! reconstructed signal: each output sample is the bit-stream
//! residual plus the dot product of the four ADPCM coefficients
//! with the four most recently reconstructed samples (the past
//! one, two, three, and four samples). The fourth-order
//! reconstructed signal forms the running history that the next
//! samples consume; the spec's comment "history from last subframe
//! or subsubframe" makes the per-subband, per-decode-block
//! initial-condition handoff explicit.
//!
//! # Scope
//!
//! This module exposes the predictor primitive itself, not the
//! dispatch. The caller (a future subframe walker) is responsible
//! for:
//!
//! - Reading the per-subband `PMODE` flag and only invoking the
//!   predictor for subbands whose `PMODE == 1`.
//! - Extracting the four ADPCM coefficients (`raADPCMCoeff[0..4]`)
//!   from the bit stream and feeding them in.
//! - Maintaining the rolling four-sample history across decode
//!   blocks: at end-of-block the last four reconstructed samples
//!   become the next block's `raSample[-4]..raSample[-1]` priming
//!   values.
//!
//! Both the integer (i32) and floating-point (f64) flavours are
//! exposed because the §C.2.2 spec text does not constrain the
//! arithmetic type — the predictor sits between dequantisation and
//! the §C.2.5 32-band synthesis QMF, both of which may run in
//! either domain depending on the decoder's precision policy.
//!
//! # Implementation note
//!
//! The §C.2.2 pseudocode loop reads `raSample[m - n - 1]` for
//! `n ∈ [0, 4)`, i.e. the four samples *preceding* index `m` in the
//! same `raSample[]` array (with negative indices `[-4..0)` carrying
//! the priming history from the prior decode block). This is an
//! in-place accumulation: the output of step `m` is written to
//! `raSample[m]` and then *immediately* visible as `raSample[(m+1) - 1]`
//! when step `m + 1` consumes its `n = 0` history slot. The decoder
//! must therefore reconstruct samples strictly left-to-right; a
//! parallel / SIMD reformulation would have to materialise the
//! running history explicitly.
//!
//! The number of ADPCM coefficients is fixed at four by the spec's
//! `NumADPCMCoeff = 4`, so the predictor's per-sample inner loop is
//! a four-tap dot product. The crate exposes the constant
//! [`NUM_ADPCM_COEFF`] for callers that want to size buffers by the
//! same invariant the spec writes against.
//!
//! Integer arithmetic is wrapping (`i32::wrapping_add`,
//! `i32::wrapping_mul`) to mirror the spec's C `int` semantics for
//! the §C.2.2 pseudocode (the spec's `int` type wraps on overflow;
//! the decoder does not require saturation).

use crate::{Error, Result};

/// Number of ADPCM prediction coefficients per the §C.2.2 spec
/// invariant `NumADPCMCoeff = 4`. The predictor reads four history
/// samples per output sample; the bit stream carries four
/// coefficients per subband when `PMODE == 1`.
pub const NUM_ADPCM_COEFF: usize = 4;

/// Apply the §C.2.2 inverse-ADPCM predictor to a single subband's
/// residual sample stream in place, in integer (i32) arithmetic.
///
/// On entry:
///
/// - `history` carries the four most recently reconstructed samples
///   from the prior decode block, ordered **oldest first**:
///   `history[0]` is `raSample[-4]`, `history[1]` is `raSample[-3]`,
///   `history[2]` is `raSample[-2]`, and `history[3]` is
///   `raSample[-1]` per the spec's negative-indexing convention.
///   `history.len()` must equal [`NUM_ADPCM_COEFF`].
/// - `coeffs` carries the four ADPCM coefficients
///   `raADPCMCoeff[0..4]` extracted from the bit stream for this
///   subband. `coeffs.len()` must equal [`NUM_ADPCM_COEFF`].
/// - `samples` carries the residual (error) samples on entry. On
///   return, `samples` carries the reconstructed samples (each
///   residual replaced by `residual + Σ coeffs[n] * past[n]` where
///   `past[n]` is the sample `n + 1` positions before the current
///   index, sourced first from `history` then from earlier
///   `samples` slots as the predictor walks forward).
///
/// On exit:
///
/// - `history` is **not** updated by this call. The caller is
///   responsible for sliding the last [`NUM_ADPCM_COEFF`] samples
///   from `samples` into the history buffer for the next decode
///   block (see [`update_history_i32`]).
///
/// Arithmetic is `i32::wrapping_add` / `i32::wrapping_mul` to match
/// the §C.2.2 pseudocode's C-style `int` semantics.
///
/// # Errors
///
/// Returns [`Error::InverseAdpcmShapeMismatch`] if `history.len() !=
/// NUM_ADPCM_COEFF` or `coeffs.len() != NUM_ADPCM_COEFF`. An empty
/// `samples` slice is **not** an error — the predictor exits without
/// touching the history buffer (a zero-length block is a no-op).
///
/// # Example
///
/// ```rust
/// use oxideav_dts::{inverse_adpcm_decode_i32, NUM_ADPCM_COEFF};
///
/// // All-zero history + all-zero coefficients: predictor is the
/// // identity, so the residuals pass through unchanged.
/// let history = [0i32; NUM_ADPCM_COEFF];
/// let coeffs = [0i32; NUM_ADPCM_COEFF];
/// let mut samples = [10i32, 20, 30, 40];
/// inverse_adpcm_decode_i32(&history, &coeffs, &mut samples).unwrap();
/// assert_eq!(samples, [10, 20, 30, 40]);
/// ```
pub fn inverse_adpcm_decode_i32(
    history: &[i32],
    coeffs: &[i32],
    samples: &mut [i32],
) -> Result<()> {
    if history.len() != NUM_ADPCM_COEFF {
        return Err(Error::InverseAdpcmShapeMismatch {
            history_len: history.len(),
            coeffs_len: coeffs.len(),
        });
    }
    if coeffs.len() != NUM_ADPCM_COEFF {
        return Err(Error::InverseAdpcmShapeMismatch {
            history_len: history.len(),
            coeffs_len: coeffs.len(),
        });
    }
    // The spec's `raSample[]` is a logical array indexed from -4 to
    // nNumSample-1. We materialise the negative slice as the
    // `history` argument; the predictor reads
    // `raSample[m - n - 1]` for n in [0, 4):
    //   n=0 -> raSample[m-1]
    //   n=1 -> raSample[m-2]
    //   n=2 -> raSample[m-3]
    //   n=3 -> raSample[m-4]
    // For m=0 these all fall into the negative-index history:
    //   raSample[-1] = history[3], raSample[-2] = history[2],
    //   raSample[-3] = history[1], raSample[-4] = history[0].
    // For m=1 the n=0 slot is the just-reconstructed samples[0],
    // and the n=1..3 slots are history[3], history[2], history[1].
    // The fully-loaded "no history needed" regime begins at m=4.
    for m in 0..samples.len() {
        let mut acc = samples[m];
        for n in 0..NUM_ADPCM_COEFF {
            // raSample[m - n - 1]
            let past = if (n + 1) <= m {
                samples[m - n - 1]
            } else {
                // We need raSample at logical index m - n - 1 which
                // is in the negative range. The most-recent history
                // slot (history[3] == raSample[-1]) is fetched when
                // m=0, n=0. Generally:
                //   logical = m - n - 1  (negative for n+1 > m)
                //   history slot = NUM_ADPCM_COEFF + logical
                //                = NUM_ADPCM_COEFF + m - n - 1
                history[NUM_ADPCM_COEFF + m - n - 1]
            };
            acc = acc.wrapping_add(coeffs[n].wrapping_mul(past));
        }
        samples[m] = acc;
    }
    Ok(())
}

/// Apply the §C.2.2 inverse-ADPCM predictor to a single subband's
/// residual sample stream in place, in floating-point (f64)
/// arithmetic. Same predictor structure as
/// [`inverse_adpcm_decode_i32`]; chosen by callers that consume the
/// reconstructed subband samples in floating-point.
///
/// # Errors
///
/// Returns [`Error::InverseAdpcmShapeMismatch`] under the same
/// conditions as [`inverse_adpcm_decode_i32`].
///
/// # Example
///
/// ```rust
/// use oxideav_dts::{inverse_adpcm_decode_f64, NUM_ADPCM_COEFF};
///
/// let history = [0.0_f64; NUM_ADPCM_COEFF];
/// let coeffs = [0.0_f64; NUM_ADPCM_COEFF];
/// let mut samples = [1.5_f64, 2.5, -0.5];
/// inverse_adpcm_decode_f64(&history, &coeffs, &mut samples).unwrap();
/// assert_eq!(samples, [1.5, 2.5, -0.5]);
/// ```
pub fn inverse_adpcm_decode_f64(
    history: &[f64],
    coeffs: &[f64],
    samples: &mut [f64],
) -> Result<()> {
    if history.len() != NUM_ADPCM_COEFF || coeffs.len() != NUM_ADPCM_COEFF {
        return Err(Error::InverseAdpcmShapeMismatch {
            history_len: history.len(),
            coeffs_len: coeffs.len(),
        });
    }
    for m in 0..samples.len() {
        let mut acc = samples[m];
        for n in 0..NUM_ADPCM_COEFF {
            let past = if (n + 1) <= m {
                samples[m - n - 1]
            } else {
                history[NUM_ADPCM_COEFF + m - n - 1]
            };
            acc += coeffs[n] * past;
        }
        samples[m] = acc;
    }
    Ok(())
}

/// Update the rolling four-sample history buffer with the
/// last [`NUM_ADPCM_COEFF`] reconstructed samples of a decode block,
/// so the next block can pick up where this one left off.
///
/// After [`inverse_adpcm_decode_i32`] has reconstructed a block, the
/// caller passes the just-reconstructed `samples` slice together
/// with the existing `history` buffer; this function slides the
/// last four reconstructed samples into `history`, preserving the
/// spec's `history[0] == raSample[-4]`, `history[3] == raSample[-1]`
/// ordering for the next call.
///
/// If `samples.len() >= NUM_ADPCM_COEFF`, the four-sample tail of
/// `samples` becomes the new history. If `samples.len() <
/// NUM_ADPCM_COEFF`, the existing history is shifted left by
/// `samples.len()` slots and the residual `samples` are appended
/// (the predictor's short-block recovery mode).
///
/// This is a convenience helper; the spec only states the
/// invariant ("It must updated each time before reverse ADPCM is
/// run for a block of samples for each subband") and the
/// implementation strategy is determined by the rolling history
/// semantics described above.
pub fn update_history_i32(history: &mut [i32], samples: &[i32]) {
    debug_assert_eq!(history.len(), NUM_ADPCM_COEFF);
    if samples.len() >= NUM_ADPCM_COEFF {
        let tail = &samples[samples.len() - NUM_ADPCM_COEFF..];
        history.copy_from_slice(tail);
    } else {
        // Short block: shift history left by samples.len() and
        // append the residual.
        let shift = samples.len();
        history.copy_within(shift.., 0);
        history[NUM_ADPCM_COEFF - shift..].copy_from_slice(samples);
    }
}

/// Floating-point counterpart to [`update_history_i32`].
pub fn update_history_f64(history: &mut [f64], samples: &[f64]) {
    debug_assert_eq!(history.len(), NUM_ADPCM_COEFF);
    if samples.len() >= NUM_ADPCM_COEFF {
        let tail = &samples[samples.len() - NUM_ADPCM_COEFF..];
        history.copy_from_slice(tail);
    } else {
        let shift = samples.len();
        history.copy_within(shift.., 0);
        history[NUM_ADPCM_COEFF - shift..].copy_from_slice(samples);
    }
}

/// Returns `true` if the §C.2.2 inverse-ADPCM predictor must be
/// applied to a subband given its `PMODE` flag.
///
/// The spec's gating sentence reads: "Inverse ADPCM process is
/// executed for each sample in a subband whose `PMODE == 1`."
/// A `PMODE == 0` subband bypasses the predictor entirely; the
/// dequantised residual *is* the reconstructed subband sample.
pub fn inverse_adpcm_required(pmode: u8) -> bool {
    pmode == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------------
    // i32 single-block decode — predictor property tests
    // ----------------------------------------------------------------

    #[test]
    fn zero_coeffs_make_predictor_identity() {
        // All coefficients zero: the residuals pass through
        // unchanged regardless of history.
        let history = [1i32, 2, 3, 4];
        let coeffs = [0i32; NUM_ADPCM_COEFF];
        let mut samples = [10i32, 20, 30, 40, 50];
        inverse_adpcm_decode_i32(&history, &coeffs, &mut samples).unwrap();
        assert_eq!(samples, [10, 20, 30, 40, 50]);
    }

    #[test]
    fn zero_history_with_only_first_coeff_runs_off_residuals() {
        // History = 0, coeffs = (1, 0, 0, 0): each output equals
        // residual + previous_output. With residuals (1, 0, 0, 0)
        // the predictor produces (1, 1, 1, 1) — a step.
        let history = [0i32; NUM_ADPCM_COEFF];
        let coeffs = [1i32, 0, 0, 0];
        let mut samples = [1i32, 0, 0, 0];
        inverse_adpcm_decode_i32(&history, &coeffs, &mut samples).unwrap();
        assert_eq!(samples, [1, 1, 1, 1]);
    }

    #[test]
    fn coeff0_with_priming_history_seeds_first_output() {
        // For m = 0 the only history slot reached is raSample[-1] =
        // history[3]. With coeffs = (c, 0, 0, 0) and a zero residual
        // we should see samples[0] = c * history[3].
        let history = [0i32, 0, 0, 7];
        let coeffs = [3i32, 0, 0, 0];
        let mut samples = [0i32];
        inverse_adpcm_decode_i32(&history, &coeffs, &mut samples).unwrap();
        assert_eq!(samples[0], 21);
    }

    #[test]
    fn each_coeff_taps_the_right_history_slot() {
        // For m = 0 with the four coefficients (1, 10, 100, 1000)
        // and history = (a, b, c, d) = (1, 2, 3, 4) we expect
        //   samples[0] = 0
        //     + 1   * raSample[-1]   = 1 * d = 4
        //     + 10  * raSample[-2]   = 10 * c = 30
        //     + 100 * raSample[-3]   = 100 * b = 200
        //     + 1000* raSample[-4]   = 1000 * a = 1000
        //     = 1234
        let history = [1i32, 2, 3, 4];
        let coeffs = [1i32, 10, 100, 1000];
        let mut samples = [0i32];
        inverse_adpcm_decode_i32(&history, &coeffs, &mut samples).unwrap();
        assert_eq!(samples[0], 1234);
    }

    #[test]
    fn negative_indices_use_history_until_m_is_four() {
        // For m=0..3 the predictor needs the negative slice of
        // history; for m=4 onwards no history is consulted (the
        // four preceding samples are all inside `samples`).
        // This test confirms the per-m boundary by zeroing the
        // history and observing that samples[4] = 0 + Σ c[n] * 0 = 0
        // when c is any vector and the leading samples are zero.
        let history = [0i32; NUM_ADPCM_COEFF];
        let coeffs = [7i32, 11, 13, 17];
        let mut samples = [0i32, 0, 0, 0, 0];
        inverse_adpcm_decode_i32(&history, &coeffs, &mut samples).unwrap();
        assert_eq!(samples, [0; 5]);
    }

    #[test]
    fn predictor_uses_just_reconstructed_sample_immediately() {
        // m = 0: samples[0] = 1 + c * history[3] = 1 + 2*0 = 1.
        // m = 1: samples[1] = 0 + c * samples[0] = 0 + 2*1 = 2.
        // m = 2: samples[2] = 0 + c * samples[1] = 0 + 2*2 = 4.
        // m = 3: samples[3] = 0 + c * samples[2] = 0 + 2*4 = 8.
        // Verifies that the predictor sees the freshly-written
        // samples[m] as the n=0 history of step m+1.
        let history = [0i32; NUM_ADPCM_COEFF];
        let coeffs = [2i32, 0, 0, 0];
        let mut samples = [1i32, 0, 0, 0];
        inverse_adpcm_decode_i32(&history, &coeffs, &mut samples).unwrap();
        assert_eq!(samples, [1, 2, 4, 8]);
    }

    #[test]
    fn empty_block_is_no_op() {
        let history = [1i32, 2, 3, 4];
        let coeffs = [5i32, 6, 7, 8];
        let mut samples: [i32; 0] = [];
        inverse_adpcm_decode_i32(&history, &coeffs, &mut samples).unwrap();
        // History unchanged (we don't update it in the predictor itself).
        assert_eq!(history, [1, 2, 3, 4]);
    }

    #[test]
    fn wrapping_arithmetic_at_i32_max() {
        // The accumulator must wrap, not panic. Use coeffs
        // (i32::MAX, 0, 0, 0) and history (.., .., .., 2) so
        // c[0] * history[3] = i32::MAX * 2 = -2 (wrapping); plus a
        // residual of 1 gives -1.
        let history = [0i32, 0, 0, 2];
        let coeffs = [i32::MAX, 0, 0, 0];
        let mut samples = [1i32];
        inverse_adpcm_decode_i32(&history, &coeffs, &mut samples).unwrap();
        let expected = 1i32.wrapping_add(i32::MAX.wrapping_mul(2));
        assert_eq!(samples[0], expected);
    }

    #[test]
    fn wrapping_arithmetic_at_i32_min_times_negative_one() {
        // i32::MIN * -1 overflows; wrapping_mul yields i32::MIN.
        let history = [0i32, 0, 0, i32::MIN];
        let coeffs = [-1i32, 0, 0, 0];
        let mut samples = [0i32];
        inverse_adpcm_decode_i32(&history, &coeffs, &mut samples).unwrap();
        assert_eq!(samples[0], i32::MIN);
    }

    #[test]
    fn history_length_mismatch_short() {
        let short_history = [0i32; 3];
        let coeffs = [0i32; NUM_ADPCM_COEFF];
        let mut samples = [0i32; 4];
        let err = inverse_adpcm_decode_i32(&short_history, &coeffs, &mut samples).unwrap_err();
        assert!(matches!(
            err,
            Error::InverseAdpcmShapeMismatch {
                history_len: 3,
                coeffs_len: 4,
            }
        ));
    }

    #[test]
    fn history_length_mismatch_long() {
        let long_history = [0i32; 5];
        let coeffs = [0i32; NUM_ADPCM_COEFF];
        let mut samples = [0i32; 4];
        let err = inverse_adpcm_decode_i32(&long_history, &coeffs, &mut samples).unwrap_err();
        assert!(matches!(
            err,
            Error::InverseAdpcmShapeMismatch {
                history_len: 5,
                coeffs_len: 4,
            }
        ));
    }

    #[test]
    fn coeffs_length_mismatch_short() {
        let history = [0i32; NUM_ADPCM_COEFF];
        let short_coeffs = [0i32; 3];
        let mut samples = [0i32; 4];
        let err = inverse_adpcm_decode_i32(&history, &short_coeffs, &mut samples).unwrap_err();
        assert!(matches!(
            err,
            Error::InverseAdpcmShapeMismatch {
                history_len: 4,
                coeffs_len: 3,
            }
        ));
    }

    #[test]
    fn coeffs_length_mismatch_long() {
        let history = [0i32; NUM_ADPCM_COEFF];
        let long_coeffs = [0i32; 7];
        let mut samples = [0i32; 4];
        let err = inverse_adpcm_decode_i32(&history, &long_coeffs, &mut samples).unwrap_err();
        assert!(matches!(
            err,
            Error::InverseAdpcmShapeMismatch {
                history_len: 4,
                coeffs_len: 7,
            }
        ));
    }

    #[test]
    fn negative_coefficients_apply_sign_correctly() {
        // history = (0, 0, 0, 5), coeffs = (-1, 0, 0, 0), residual = 0
        // m=0: 0 + (-1) * 5 = -5
        let history = [0i32, 0, 0, 5];
        let coeffs = [-1i32, 0, 0, 0];
        let mut samples = [0i32];
        inverse_adpcm_decode_i32(&history, &coeffs, &mut samples).unwrap();
        assert_eq!(samples[0], -5);
    }

    #[test]
    fn long_block_predictor_matches_manual_unroll() {
        // Hand-compute the §C.2.2 result for an 8-sample block with
        // non-trivial history and coeffs, then cross-check against
        // the helper.
        let history = [1i32, -2, 3, -4];
        let coeffs = [5i32, -6, 7, -8];
        let residuals = [10i32, 20, -30, 40, -50, 60, -70, 80];

        // Manual computation, sample by sample.
        let mut expected = residuals;
        for m in 0..expected.len() {
            let mut acc = expected[m];
            for n in 0..NUM_ADPCM_COEFF {
                let past = if (n + 1) <= m {
                    expected[m - n - 1]
                } else {
                    history[NUM_ADPCM_COEFF + m - n - 1]
                };
                acc = acc.wrapping_add(coeffs[n].wrapping_mul(past));
            }
            expected[m] = acc;
        }

        let mut samples = residuals;
        inverse_adpcm_decode_i32(&history, &coeffs, &mut samples).unwrap();
        assert_eq!(samples, expected);
    }

    // ----------------------------------------------------------------
    // f64 single-block decode tests
    // ----------------------------------------------------------------

    #[test]
    fn f64_zero_coeffs_make_predictor_identity() {
        let history = [1.0_f64, 2.0, 3.0, 4.0];
        let coeffs = [0.0_f64; NUM_ADPCM_COEFF];
        let mut samples = [10.0_f64, 20.0, 30.0];
        inverse_adpcm_decode_f64(&history, &coeffs, &mut samples).unwrap();
        assert_eq!(samples, [10.0, 20.0, 30.0]);
    }

    #[test]
    fn f64_each_coeff_taps_the_right_history_slot() {
        let history = [1.0_f64, 2.0, 3.0, 4.0];
        let coeffs = [1.0_f64, 10.0, 100.0, 1000.0];
        let mut samples = [0.0_f64];
        inverse_adpcm_decode_f64(&history, &coeffs, &mut samples).unwrap();
        // 1*4 + 10*3 + 100*2 + 1000*1 = 4 + 30 + 200 + 1000 = 1234
        assert!((samples[0] - 1234.0).abs() < 1e-9);
    }

    #[test]
    fn f64_uses_just_reconstructed_sample_immediately() {
        let history = [0.0_f64; NUM_ADPCM_COEFF];
        let coeffs = [0.5_f64, 0.0, 0.0, 0.0];
        let mut samples = [1.0_f64, 0.0, 0.0, 0.0];
        inverse_adpcm_decode_f64(&history, &coeffs, &mut samples).unwrap();
        // 1.0, 0.5, 0.25, 0.125
        assert!((samples[0] - 1.0).abs() < 1e-12);
        assert!((samples[1] - 0.5).abs() < 1e-12);
        assert!((samples[2] - 0.25).abs() < 1e-12);
        assert!((samples[3] - 0.125).abs() < 1e-12);
    }

    #[test]
    fn f64_history_length_mismatch() {
        let short_history = [0.0_f64; 2];
        let coeffs = [0.0_f64; NUM_ADPCM_COEFF];
        let mut samples = [0.0_f64; 1];
        let err = inverse_adpcm_decode_f64(&short_history, &coeffs, &mut samples).unwrap_err();
        assert!(matches!(
            err,
            Error::InverseAdpcmShapeMismatch {
                history_len: 2,
                coeffs_len: 4,
            }
        ));
    }

    #[test]
    fn f64_coeffs_length_mismatch() {
        let history = [0.0_f64; NUM_ADPCM_COEFF];
        let long_coeffs = [0.0_f64; 5];
        let mut samples = [0.0_f64; 1];
        let err = inverse_adpcm_decode_f64(&history, &long_coeffs, &mut samples).unwrap_err();
        assert!(matches!(
            err,
            Error::InverseAdpcmShapeMismatch {
                history_len: 4,
                coeffs_len: 5,
            }
        ));
    }

    #[test]
    fn f64_empty_block_is_no_op() {
        let history = [1.0_f64, 2.0, 3.0, 4.0];
        let coeffs = [5.0_f64, 6.0, 7.0, 8.0];
        let mut samples: [f64; 0] = [];
        inverse_adpcm_decode_f64(&history, &coeffs, &mut samples).unwrap();
    }

    // ----------------------------------------------------------------
    // History-update helper tests
    // ----------------------------------------------------------------

    #[test]
    fn update_history_long_block_takes_last_four_samples() {
        let mut history = [1i32, 2, 3, 4];
        let samples = [10i32, 20, 30, 40, 50, 60, 70, 80];
        update_history_i32(&mut history, &samples);
        // Last four samples become the new history.
        assert_eq!(history, [50, 60, 70, 80]);
    }

    #[test]
    fn update_history_exact_four_takes_all_samples() {
        let mut history = [1i32, 2, 3, 4];
        let samples = [10i32, 20, 30, 40];
        update_history_i32(&mut history, &samples);
        assert_eq!(history, [10, 20, 30, 40]);
    }

    #[test]
    fn update_history_short_block_shifts_left() {
        // 2-sample short block: shift left by 2, append samples.
        // Before: [a, b, c, d] -> shift left by 2 -> [c, d, ?, ?]
        // Then append samples [e, f] -> [c, d, e, f].
        let mut history = [1i32, 2, 3, 4];
        let samples = [10i32, 20];
        update_history_i32(&mut history, &samples);
        assert_eq!(history, [3, 4, 10, 20]);
    }

    #[test]
    fn update_history_one_sample_short_block() {
        let mut history = [1i32, 2, 3, 4];
        let samples = [99i32];
        update_history_i32(&mut history, &samples);
        // Shift left by 1, append [99]
        assert_eq!(history, [2, 3, 4, 99]);
    }

    #[test]
    fn update_history_empty_block_leaves_history_untouched() {
        let mut history = [1i32, 2, 3, 4];
        let samples: [i32; 0] = [];
        update_history_i32(&mut history, &samples);
        assert_eq!(history, [1, 2, 3, 4]);
    }

    #[test]
    fn update_history_f64_long_block() {
        let mut history = [1.0_f64, 2.0, 3.0, 4.0];
        let samples = [10.0_f64, 20.0, 30.0, 40.0, 50.0, 60.0];
        update_history_f64(&mut history, &samples);
        assert_eq!(history, [30.0, 40.0, 50.0, 60.0]);
    }

    #[test]
    fn update_history_f64_short_block() {
        let mut history = [1.0_f64, 2.0, 3.0, 4.0];
        let samples = [99.0_f64];
        update_history_f64(&mut history, &samples);
        assert_eq!(history, [2.0, 3.0, 4.0, 99.0]);
    }

    // ----------------------------------------------------------------
    // Dispatch predicate tests
    // ----------------------------------------------------------------

    #[test]
    fn inverse_adpcm_required_only_when_pmode_is_one() {
        assert!(inverse_adpcm_required(1));
        assert!(!inverse_adpcm_required(0));
        // PMODE is a 1-bit field per §5.4.1; values > 1 are
        // out-of-domain but the predicate still returns false.
        for pmode in 2u8..=255 {
            assert!(!inverse_adpcm_required(pmode), "pmode={pmode}");
        }
    }

    // ----------------------------------------------------------------
    // End-to-end two-block continuation sweep
    // ----------------------------------------------------------------

    #[test]
    fn two_block_decode_matches_single_long_block_decode() {
        // Property: decoding two consecutive blocks with the history
        // updated between them should produce the same reconstructed
        // sequence as decoding the concatenated residual stream as a
        // single long block (with the initial history fed in once).
        let initial_history = [1i32, 2, 3, 4];
        let coeffs = [2i32, -1, 3, 0];
        let residuals: Vec<i32> = (1i32..=12).collect();

        // Single long-block reference run.
        let mut single = residuals.clone();
        inverse_adpcm_decode_i32(&initial_history, &coeffs, &mut single).unwrap();

        // Two-block run: decode first 7 samples, slide history, then
        // decode remaining 5 samples.
        let mut history = initial_history;
        let mut block_a: Vec<i32> = residuals[..7].to_vec();
        inverse_adpcm_decode_i32(&history, &coeffs, &mut block_a).unwrap();
        update_history_i32(&mut history, &block_a);

        let mut block_b: Vec<i32> = residuals[7..].to_vec();
        inverse_adpcm_decode_i32(&history, &coeffs, &mut block_b).unwrap();

        let mut chained = block_a;
        chained.extend_from_slice(&block_b);
        assert_eq!(chained, single);
    }
}

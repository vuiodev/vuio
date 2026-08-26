//! DTS Coherent Acoustics — §C.2.4 Sum/Difference Decoding.
//!
//! Round 214 (2026-06-03) lands the §C.2.4 sum/difference matrix
//! decoder, the inverse of the encoder-side joint sum/difference
//! coding that the `FRONT_SUM` (`SUMF`) and `SURROUND_SUM` (`SUMS`)
//! header flags signal (and that the `AMODE == 3` Sum/Difference
//! channel-arrangement code implies for the front pair).
//!
//! Source: ETSI TS 102 114 V1.3.1 (2011-08), Annex C (informative)
//! §C.2.4 "Sum/Difference Decoding" (PDF p.184) — staged at
//! `docs/audio/dts/etsi-ts-102114-dts-coherent-acoustics.pdf`. The
//! reproduced normative spec pseudocode is:
//!
//! ```text
//! // SUMF — front L/R (also when AMODE == 3, Sum/Difference)
//! for (n=0; n<nSUBS; n++)               // All active subbands.
//!     for (nSample=0; nSample<8*nSSC; nSample++) {  // Samples in all subsubframes
//!         FrontLeft[nSample]   = Lfeft[nSample] + Fright[nSample];
//!         Frontright[nSample]  = Lfeft[nSample] - Fright[nSample];
//!     }
//!
//! // SUMS — surround L/R
//! for (n=0; n<nSUBS; n++)
//!     for (nSample=0; nSample<8*nSSC; nSample++) {
//!         SurroundLeft[nSample]   = Sleft[nSample] + Sright[nSample];
//!         Surroundright[nSample]  = Sleft[nSample] - Sright[nSample];
//!     }
//! ```
//!
//! The spec works in the subband-sample domain (reconstructed
//! subband samples, before the §C.2.5 32-band synthesis QMF) so the
//! decode is purely a per-sample (`L', R') = (L+R, L-R)` matrix
//! multiplication. The matrix is symmetric and self-inverse up to
//! a factor of 2: applying it twice produces `(2 L, 2 R)`. The
//! decoder applies it once, undoing a single encoder-side application
//! (the encoder produces `(L+R, L-R)` from `(L, R)`; the decoder
//! recovers something proportional to the original `(L, R)` pair).
//!
//! # Scope
//!
//! This module exposes the matrix decode itself, not the dispatch.
//! The caller (a future subframe walker) is responsible for:
//!
//! - Reading the `FRONT_SUM` / `SURROUND_SUM` header flags from the
//!   frame header (already surfaced by [`crate::DtsFrameHeader`] as
//!   [`crate::DtsFrameHeader::front_sum`] / [`crate::DtsFrameHeader::surround_sum`]).
//! - Recognising that `AMODE == 3` ([`crate::AmodeArrangement::SumDifference`])
//!   forces the front-channel decode regardless of the `FRONT_SUM` bit
//!   (per the §C.2.4 narrative: "This decoding is also required when
//!   AMODE = 3.").
//! - Identifying which pair of channels carries the encoder's
//!   `(L+R, L-R)` (front L / front R for `SUMF`; surround L / surround R
//!   for `SUMS`).
//! - Determining the number of active subbands (`nSUBS`) and the
//!   number of sub-sub-frame samples (`8*nSSC`), and arranging the
//!   slice geometry so each `(left, right)` pair the decoder consumes
//!   is "all subbands × all samples per subsubframe" for that channel.
//!
//! Both the integer (i32) and floating-point (f64) decode flavours are
//! exposed because the §C.2.4 spec text does not constrain the
//! arithmetic type — the inverse-quantisation path that feeds the
//! sum/difference decode may run in either; the decoder picks per
//! its precision requirements.
//!
//! # Implementation note
//!
//! The decode is an **in-place** sample-by-sample operation: the
//! caller passes mutable slices for the left and right channels of
//! the pair, and on return each slice has been overwritten with its
//! decoded value (`left` ← `left + right`, `right` ← `left - right`,
//! using the **pre-update** value of `left` for the right channel —
//! exactly the §C.2.4 pseudocode's read-old / write-new ordering).
//! Integer arithmetic is wrapping (`i32::wrapping_add` /
//! `i32::wrapping_sub`) to mirror C semantics for the §C.2.4
//! pseudocode (the spec's `int` type wraps on overflow; the decoder
//! does not require saturation).

use crate::{Error, Result};

/// Decode one (left, right) sample pair in place via the §C.2.4
/// sum/difference matrix: `(L', R') = (L + R, L - R)` with the
/// pre-update value of `L` consumed for both outputs. The two
/// inputs must be the same length; the lengths are checked at the
/// slice boundary.
///
/// Arithmetic is `i32::wrapping_add` / `i32::wrapping_sub` to match
/// the §C.2.4 pseudocode's C-style `int` semantics.
///
/// # Errors
///
/// Returns [`Error::SumDiffLengthMismatch`] if `left.len() !=
/// right.len()`.
///
/// # Example
///
/// ```rust
/// use oxideav_dts::sum_difference_decode_i32;
///
/// // Encoder produced (L+R, L-R) = (15, 5) from (L, R) = (10, 5).
/// // Decoding once gives back something proportional to the original.
/// let mut left = [15i32];   // L+R
/// let mut right = [5i32];   // L-R
/// sum_difference_decode_i32(&mut left, &mut right).unwrap();
/// assert_eq!(left[0], 20);  // (L+R) + (L-R) = 2L
/// assert_eq!(right[0], 10); // (L+R) - (L-R) = 2R
/// ```
pub fn sum_difference_decode_i32(left: &mut [i32], right: &mut [i32]) -> Result<()> {
    if left.len() != right.len() {
        return Err(Error::SumDiffLengthMismatch {
            left_len: left.len(),
            right_len: right.len(),
        });
    }
    for (l, r) in left.iter_mut().zip(right.iter_mut()) {
        let prev_left = *l;
        let prev_right = *r;
        *l = prev_left.wrapping_add(prev_right);
        *r = prev_left.wrapping_sub(prev_right);
    }
    Ok(())
}

/// Decode one (left, right) sample pair in place via the §C.2.4
/// sum/difference matrix, in floating-point arithmetic. Same matrix
/// as [`sum_difference_decode_i32`]; chosen by callers that consume
/// the reconstructed-subband samples in floating-point.
///
/// # Errors
///
/// Returns [`Error::SumDiffLengthMismatch`] if `left.len() !=
/// right.len()`.
///
/// # Example
///
/// ```rust
/// use oxideav_dts::sum_difference_decode_f64;
///
/// let mut left = [15.0_f64];
/// let mut right = [5.0_f64];
/// sum_difference_decode_f64(&mut left, &mut right).unwrap();
/// assert_eq!(left[0], 20.0);
/// assert_eq!(right[0], 10.0);
/// ```
pub fn sum_difference_decode_f64(left: &mut [f64], right: &mut [f64]) -> Result<()> {
    if left.len() != right.len() {
        return Err(Error::SumDiffLengthMismatch {
            left_len: left.len(),
            right_len: right.len(),
        });
    }
    for (l, r) in left.iter_mut().zip(right.iter_mut()) {
        let prev_left = *l;
        let prev_right = *r;
        *l = prev_left + prev_right;
        *r = prev_left - prev_right;
    }
    Ok(())
}

/// Decode the §C.2.4 sum/difference matrix across **all active
/// subbands × all sub-sub-frame samples** for a single channel pair.
/// The two argument slice-of-slices each carry `n_subs` inner slices
/// (one per active subband, ordered subband 0..`n_subs`), each
/// inner slice holding the `8 * n_ssc` sub-sub-frame samples for
/// that subband × that channel.
///
/// This is the direct shape of the §C.2.4 pseudocode loop:
///
/// ```text
/// for (n=0; n<nSUBS; n++)
///     for (nSample=0; nSample<8*nSSC; nSample++) { ... }
/// ```
///
/// Per-subband lengths are independently checked. The outer
/// (subband-count) lengths must match; the inner (samples-per-subband)
/// lengths must match per-subband but may differ across subbands
/// (the spec quantifies samples by `8 * nSSC`, a frame-level
/// invariant, but this primitive doesn't enforce that — it just
/// requires per-subband (left, right) length agreement).
///
/// # Errors
///
/// - [`Error::SumDiffLengthMismatch`] (with the outer subband-count
///   in `left_len` / `right_len`) if `left_subbands.len() !=
///   right_subbands.len()`, OR if any per-subband (left, right)
///   inner-slice length pair disagrees (in which case `left_len` /
///   `right_len` carry the *inner* lengths).
pub fn sum_difference_decode_subband_pair_i32(
    left_subbands: &mut [&mut [i32]],
    right_subbands: &mut [&mut [i32]],
) -> Result<()> {
    if left_subbands.len() != right_subbands.len() {
        return Err(Error::SumDiffLengthMismatch {
            left_len: left_subbands.len(),
            right_len: right_subbands.len(),
        });
    }
    for (left_band, right_band) in left_subbands.iter_mut().zip(right_subbands.iter_mut()) {
        sum_difference_decode_i32(left_band, right_band)?;
    }
    Ok(())
}

/// Floating-point counterpart to [`sum_difference_decode_subband_pair_i32`].
///
/// # Errors
///
/// Same error contract as the i32 variant.
pub fn sum_difference_decode_subband_pair_f64(
    left_subbands: &mut [&mut [f64]],
    right_subbands: &mut [&mut [f64]],
) -> Result<()> {
    if left_subbands.len() != right_subbands.len() {
        return Err(Error::SumDiffLengthMismatch {
            left_len: left_subbands.len(),
            right_len: right_subbands.len(),
        });
    }
    for (left_band, right_band) in left_subbands.iter_mut().zip(right_subbands.iter_mut()) {
        sum_difference_decode_f64(left_band, right_band)?;
    }
    Ok(())
}

/// Returns `true` if the §C.2.4 front-channel sum/difference decode
/// must be applied given the frame-header's `FRONT_SUM` flag and
/// `AMODE` field.
///
/// Per §C.2.4: the decoding is required when `SUMF` is set, and
/// **also** when `AMODE == 3` (Sum/Difference channel arrangement).
/// The two triggers compose disjunctively.
///
/// `amode` is the raw 6-bit `AMODE` value from the frame header
/// (`DtsFrameHeader::amode`); the function checks `amode == 3` directly
/// rather than going through the [`crate::AmodeArrangement`] enum so
/// the call site can dispatch without resolving the user-defined
/// codes (`16..=63`).
pub fn front_sum_difference_required(front_sum: bool, amode: u8) -> bool {
    front_sum || amode == 3
}

/// Returns `true` if the §C.2.4 surround-channel sum/difference decode
/// must be applied given the `SURROUND_SUM` (`SUMS`) flag.
///
/// Unlike the front-pair case, the spec does not name an `AMODE` code
/// that forces the surround decode independent of `SUMS`; the
/// function therefore reduces to a pass-through of the flag.
pub fn surround_sum_difference_required(surround_sum: bool) -> bool {
    surround_sum
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------
    // i32 single-pair decode — matrix property tests
    // -----------------------------------------------------------

    #[test]
    fn single_pair_basic_decode_recovers_2l_2r() {
        // Encoder: (L, R) -> (L+R, L-R)
        // Decoder: (L+R, L-R) -> (2L, 2R)
        let (l, r) = (10i32, 5i32);
        let mut enc_left = [l + r];
        let mut enc_right = [l - r];
        sum_difference_decode_i32(&mut enc_left, &mut enc_right).unwrap();
        assert_eq!(enc_left[0], 2 * l);
        assert_eq!(enc_right[0], 2 * r);
    }

    #[test]
    fn single_pair_zero_inputs_decode_to_zero() {
        let mut left = [0i32; 8];
        let mut right = [0i32; 8];
        sum_difference_decode_i32(&mut left, &mut right).unwrap();
        for v in left.iter().chain(right.iter()) {
            assert_eq!(*v, 0);
        }
    }

    #[test]
    fn single_pair_left_only_decode() {
        // (L+R, L-R) with R=0 -> (L, L) ... but here we feed raw
        // (L, R) = (5, 0) to confirm the matrix:
        //   left_out = 5 + 0 = 5
        //   right_out = 5 - 0 = 5
        let mut left = [5i32];
        let mut right = [0i32];
        sum_difference_decode_i32(&mut left, &mut right).unwrap();
        assert_eq!(left[0], 5);
        assert_eq!(right[0], 5);
    }

    #[test]
    fn single_pair_right_only_decode() {
        // (0, R) -> (R, -R): matrix turns a pure-right input into a
        // sign-mirrored output pair.
        let mut left = [0i32];
        let mut right = [3i32];
        sum_difference_decode_i32(&mut left, &mut right).unwrap();
        assert_eq!(left[0], 3);
        assert_eq!(right[0], -3);
    }

    #[test]
    fn single_pair_uses_pre_update_left_for_right() {
        // Verify the spec's read-old-then-write ordering: the right
        // channel reads the **pre-update** value of left, not the
        // post-update one. If we wrote left first and then computed
        // right as left-right, we'd get (l+r, (l+r)-r) = (l+r, l) —
        // which is wrong; the correct result is (l+r, l-r).
        let mut left = [10i32];
        let mut right = [3i32];
        sum_difference_decode_i32(&mut left, &mut right).unwrap();
        assert_eq!(left[0], 13);
        assert_eq!(right[0], 7);
    }

    #[test]
    fn single_pair_length_mismatch_reports_lengths() {
        let mut left = [0i32; 4];
        let mut right = [0i32; 5];
        let err = sum_difference_decode_i32(&mut left, &mut right).unwrap_err();
        assert!(matches!(
            err,
            Error::SumDiffLengthMismatch {
                left_len: 4,
                right_len: 5,
            }
        ));
    }

    #[test]
    fn single_pair_empty_slices_succeed() {
        let mut left: [i32; 0] = [];
        let mut right: [i32; 0] = [];
        sum_difference_decode_i32(&mut left, &mut right).unwrap();
    }

    #[test]
    fn single_pair_decoded_twice_yields_2x_pair() {
        // Applying the matrix twice should multiply each component
        // by 2 (mod wrapping): from (L, R) -> (L+R, L-R) -> ((L+R)+(L-R), (L+R)-(L-R))
        //                                              = (2L, 2R).
        // The encoder runs it once; if the decoder ran it twice we'd
        // see the 2x scaling. This test cross-checks the matrix
        // self-product = 2I.
        let (l, r) = (7i32, 11i32);
        let mut left = [l];
        let mut right = [r];
        sum_difference_decode_i32(&mut left, &mut right).unwrap();
        sum_difference_decode_i32(&mut left, &mut right).unwrap();
        assert_eq!(left[0], 2 * l);
        assert_eq!(right[0], 2 * r);
    }

    #[test]
    fn single_pair_negative_inputs_decode_correctly() {
        let mut left = [-5i32, -10, -100];
        let mut right = [3i32, -7, 50];
        sum_difference_decode_i32(&mut left, &mut right).unwrap();
        assert_eq!(left, [-2, -17, -50]);
        assert_eq!(right, [-8, -3, -150]);
    }

    #[test]
    fn single_pair_wrapping_arithmetic_at_i32_max() {
        // Wrapping behaviour at the i32 boundary: the spec's
        // C `int` semantics wrap on overflow; we use
        // `i32::wrapping_add` so the test must not panic.
        let mut left = [i32::MAX];
        let mut right = [1i32];
        sum_difference_decode_i32(&mut left, &mut right).unwrap();
        // i32::MAX.wrapping_add(1) = i32::MIN
        assert_eq!(left[0], i32::MIN);
        // i32::MAX.wrapping_sub(1) = i32::MAX - 1
        assert_eq!(right[0], i32::MAX - 1);
    }

    #[test]
    fn single_pair_walk_long_slice() {
        // Independent per-sample property: each (l[i], r[i]) pair
        // satisfies (l_out, r_out) = (l_in + r_in, l_in - r_in)
        // regardless of position.
        let l_in: Vec<i32> = (0i32..256).map(|i| i - 128).collect();
        let r_in: Vec<i32> = (0i32..256).map(|i| (i * 2) - 100).collect();
        let mut left = l_in.clone();
        let mut right = r_in.clone();
        sum_difference_decode_i32(&mut left, &mut right).unwrap();
        for i in 0..256 {
            assert_eq!(left[i], l_in[i].wrapping_add(r_in[i]));
            assert_eq!(right[i], l_in[i].wrapping_sub(r_in[i]));
        }
    }

    // -----------------------------------------------------------
    // f64 single-pair decode tests
    // -----------------------------------------------------------

    #[test]
    fn single_pair_f64_basic() {
        let mut left = [1.5_f64, 2.5];
        let mut right = [0.25_f64, -0.5];
        sum_difference_decode_f64(&mut left, &mut right).unwrap();
        assert!((left[0] - 1.75).abs() < 1e-12);
        assert!((right[0] - 1.25).abs() < 1e-12);
        assert!((left[1] - 2.0).abs() < 1e-12);
        assert!((right[1] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn single_pair_f64_self_product_is_2i() {
        // Matrix^2 = 2 I, same property as the i32 variant — but
        // here we get exact 2x scaling because the test inputs are
        // small dyadic rationals.
        let l_in = 0.25_f64;
        let r_in = 0.125_f64;
        let mut left = [l_in];
        let mut right = [r_in];
        sum_difference_decode_f64(&mut left, &mut right).unwrap();
        sum_difference_decode_f64(&mut left, &mut right).unwrap();
        assert!((left[0] - 2.0 * l_in).abs() < 1e-12);
        assert!((right[0] - 2.0 * r_in).abs() < 1e-12);
    }

    #[test]
    fn single_pair_f64_length_mismatch() {
        let mut left = [0.0_f64; 3];
        let mut right = [0.0_f64; 7];
        let err = sum_difference_decode_f64(&mut left, &mut right).unwrap_err();
        assert!(matches!(
            err,
            Error::SumDiffLengthMismatch {
                left_len: 3,
                right_len: 7,
            }
        ));
    }

    // -----------------------------------------------------------
    // Subband-pair (slice-of-slices) decode tests
    // -----------------------------------------------------------

    #[test]
    fn subband_pair_walks_all_active_subbands_i32() {
        // Three subbands, two samples each (mocking nSUBS=3, 8*nSSC=2).
        let mut subband_a_l = [1i32, 2];
        let mut subband_a_r = [3i32, 4];
        let mut subband_b_l = [5i32, 6];
        let mut subband_b_r = [7i32, 8];
        let mut subband_c_l = [9i32, 10];
        let mut subband_c_r = [11i32, 12];
        // Capture expected outputs before consuming the &mut.
        let exp_a_l = [1 + 3, 2 + 4];
        let exp_a_r = [1 - 3, 2 - 4];
        let exp_b_l = [5 + 7, 6 + 8];
        let exp_b_r = [5 - 7, 6 - 8];
        let exp_c_l = [9 + 11, 10 + 12];
        let exp_c_r = [9 - 11, 10 - 12];
        {
            let mut left: [&mut [i32]; 3] = [&mut subband_a_l, &mut subband_b_l, &mut subband_c_l];
            let mut right: [&mut [i32]; 3] = [&mut subband_a_r, &mut subband_b_r, &mut subband_c_r];
            sum_difference_decode_subband_pair_i32(&mut left, &mut right).unwrap();
        }
        // Verify each subband decoded independently per §C.2.4.
        assert_eq!(subband_a_l, exp_a_l);
        assert_eq!(subband_a_r, exp_a_r);
        assert_eq!(subband_b_l, exp_b_l);
        assert_eq!(subband_b_r, exp_b_r);
        assert_eq!(subband_c_l, exp_c_l);
        assert_eq!(subband_c_r, exp_c_r);
    }

    #[test]
    fn subband_pair_empty_subband_list_is_no_op() {
        let mut left: [&mut [i32]; 0] = [];
        let mut right: [&mut [i32]; 0] = [];
        sum_difference_decode_subband_pair_i32(&mut left, &mut right).unwrap();
    }

    #[test]
    fn subband_pair_outer_length_mismatch() {
        let mut s_a = [0i32; 4];
        let mut s_b = [0i32; 4];
        let mut left: [&mut [i32]; 2] = [&mut s_a, &mut s_b];
        let mut t = [0i32; 4];
        let mut right: [&mut [i32]; 1] = [&mut t];
        let err = sum_difference_decode_subband_pair_i32(&mut left, &mut right).unwrap_err();
        assert!(matches!(
            err,
            Error::SumDiffLengthMismatch {
                left_len: 2,
                right_len: 1,
            }
        ));
    }

    #[test]
    fn subband_pair_per_subband_length_mismatch() {
        let mut s_a = [0i32; 4];
        let mut s_b = [0i32; 4];
        let mut left: [&mut [i32]; 2] = [&mut s_a, &mut s_b];
        let mut t_a = [0i32; 4];
        let mut t_b = [0i32; 3];
        let mut right: [&mut [i32]; 2] = [&mut t_a, &mut t_b];
        let err = sum_difference_decode_subband_pair_i32(&mut left, &mut right).unwrap_err();
        // The error reports the *inner* per-subband length pair (the
        // first one that disagrees).
        assert!(matches!(
            err,
            Error::SumDiffLengthMismatch {
                left_len: 4,
                right_len: 3,
            }
        ));
    }

    #[test]
    fn subband_pair_f64_walks_independently() {
        let mut s_a_l = [1.0_f64, 2.0];
        let mut s_a_r = [3.0_f64, 4.0];
        let mut s_b_l = [10.0_f64];
        let mut s_b_r = [20.0_f64];
        {
            let mut left: [&mut [f64]; 2] = [&mut s_a_l, &mut s_b_l];
            let mut right: [&mut [f64]; 2] = [&mut s_a_r, &mut s_b_r];
            sum_difference_decode_subband_pair_f64(&mut left, &mut right).unwrap();
        }
        assert_eq!(s_a_l, [4.0, 6.0]);
        assert_eq!(s_a_r, [-2.0, -2.0]);
        assert_eq!(s_b_l, [30.0]);
        assert_eq!(s_b_r, [-10.0]);
    }

    // -----------------------------------------------------------
    // Dispatch predicate tests
    // -----------------------------------------------------------

    #[test]
    fn front_sum_required_when_flag_set() {
        // SUMF = true forces the decode regardless of AMODE.
        for amode in 0u8..=15 {
            assert!(front_sum_difference_required(true, amode), "amode={amode}");
        }
    }

    #[test]
    fn front_sum_required_when_amode_is_three() {
        // AMODE == 3 (Sum/Difference channel arrangement) forces the
        // decode even when SUMF = false. Per §C.2.4: "This decoding is
        // also required when AMODE = 3."
        assert!(front_sum_difference_required(false, 3));
    }

    #[test]
    fn front_sum_not_required_when_flag_clear_and_amode_not_three() {
        // Spot-check every standard AMODE != 3 with SUMF = false.
        for amode in 0u8..=15 {
            if amode == 3 {
                continue;
            }
            assert!(
                !front_sum_difference_required(false, amode),
                "amode={amode} should not require decode"
            );
        }
        // Same for the user-defined range (16..=63) — none of those
        // codes carries an implicit Sum/Difference signal in the spec.
        for amode in 16u8..=63 {
            assert!(
                !front_sum_difference_required(false, amode),
                "user-defined amode={amode} should not require decode"
            );
        }
    }

    #[test]
    fn surround_sum_required_when_flag_set() {
        assert!(surround_sum_difference_required(true));
        assert!(!surround_sum_difference_required(false));
    }

    // -----------------------------------------------------------
    // End-to-end full §C.2.4 sweep
    // -----------------------------------------------------------

    #[test]
    fn full_sweep_matches_spec_pseudocode_directly() {
        // Hand-compute the §C.2.4 result for nSUBS = 4, 8*nSSC = 8
        // and a deterministic input, then cross-check against the
        // subband-pair helper.
        let n_subs: i32 = 4;
        let n_samples: i32 = 8;
        let mut left_storage: Vec<Vec<i32>> = (0..n_subs)
            .map(|s| (0..n_samples).map(|i| s * 100 + i).collect())
            .collect();
        let mut right_storage: Vec<Vec<i32>> = (0..n_subs)
            .map(|s| (0..n_samples).map(|i| -(s * 100 + i) / 2).collect())
            .collect();
        let expected_left: Vec<Vec<i32>> = left_storage
            .iter()
            .zip(right_storage.iter())
            .map(|(l, r)| l.iter().zip(r.iter()).map(|(a, b)| a + b).collect())
            .collect();
        let expected_right: Vec<Vec<i32>> = left_storage
            .iter()
            .zip(right_storage.iter())
            .map(|(l, r)| l.iter().zip(r.iter()).map(|(a, b)| a - b).collect())
            .collect();

        {
            let mut left_slices: Vec<&mut [i32]> =
                left_storage.iter_mut().map(|v| v.as_mut_slice()).collect();
            let mut right_slices: Vec<&mut [i32]> =
                right_storage.iter_mut().map(|v| v.as_mut_slice()).collect();
            sum_difference_decode_subband_pair_i32(&mut left_slices, &mut right_slices).unwrap();
        }
        assert_eq!(left_storage, expected_left);
        assert_eq!(right_storage, expected_right);
    }
}

//! DTS Coherent Acoustics — §C.2.3 Joint Subband Coding.
//!
//! Round 223 (2026-06-03) lands the §C.2.3 joint-subband decode, the
//! per-channel reconstruction step that copies the high-end subband
//! samples of a source channel into a destination channel and scales
//! them by the destination channel's per-subband
//! `JOIN_SCALES[ch][n]` factor. The encoder uses joint-subband coding
//! to drop redundant high-frequency content from a destination
//! channel: only the source channel's high subbands are coded on the
//! wire; the decoder re-synthesises the destination's high subbands
//! from them at unpack time.
//!
//! Source: ETSI TS 102 114 V1.3.1 (2011-08), Annex C (informative)
//! §C.2.3 "Joint Subband Coding" (PDF p.184) — staged at
//! `docs/audio/dts/etsi-ts-102114-dts-coherent-acoustics.pdf`. The
//! reproduced normative spec pseudocode is:
//!
//! ```text
//! for (ch=0; ch<nPCHS; ch++)
//!     if ( JOINX[ch]>0 ){                       // Joint subband coding enabled.
//!         nSourceCh = JOINX[ch]-1;              // Get source channel. JOINX counts
//!                                               // channels as 1,2,3,4,5, so minus 1.
//!         for (n=nSUBS[ch]; n<nSUBS[nSourceCh]; n++)
//!             for (nSample=0; nSample<8*nSSC; nSample++)
//!                 aPrmCh[ch].aSubband[n].aSample[nSample] =
//!                     JOIN_SCALES[ch][n] *
//!                     aPrmCh[nSourceCh].aSubband[n].aSample[nSample];
//!     }
//! ```
//!
//! Key structural facts the pseudocode locks down:
//!
//! - The trigger is `JOINX[ch] > 0`; `JOINX[ch] == 0` means the
//!   destination channel does not import joint-coded subbands from
//!   any source.
//! - `nSourceCh = JOINX[ch] - 1`: `JOINX` is one-based per the
//!   pseudocode's inline comment ("counts channels as 1,2,3,4,5, so
//!   minus 1"); the array indexing into `aPrmCh[]` is zero-based.
//! - The imported subband range is exactly
//!   `n ∈ [nSUBS[ch], nSUBS[nSourceCh])`: the destination's own
//!   subbands `0..nSUBS[ch]` are unchanged; subbands above the source
//!   channel's `nSUBS[nSourceCh]` upper bound are not touched (they
//!   remain whatever they were before the joint-subband step, i.e.
//!   zero for an inactive subband).
//! - Per-subband scaling: a single scalar `JOIN_SCALES[ch][n]` is
//!   broadcast over all `8 * nSSC` samples in that subband.
//! - The operation is unconditionally **write**, not accumulate: the
//!   destination subband is replaced by the scaled copy.
//!
//! # Scope
//!
//! This module exposes the matrix copy + scale itself, not the
//! dispatch. The caller (a future subframe walker) is responsible
//! for:
//!
//! - Reading the `JOINX[ch]` per-channel selector from the AUDIO
//!   CODING HEADER (`JOINX` is per-channel and 0 when joint-subband
//!   coding is disabled for that channel; > 0 when enabled, with the
//!   one-based source-channel index).
//! - Reading the per-channel-per-subband `JOIN_SCALES[ch][n]` scale
//!   factors from the bit stream (the §5.4.x joint-scale Huffman /
//!   linear decode for the active range
//!   `n ∈ [nSUBS[ch], nSUBS[nSourceCh])`).
//! - Translating the one-based `JOINX[ch]` to the zero-based
//!   `nSourceCh = JOINX[ch] - 1` (see [`joint_source_channel`]).
//! - Reading the per-channel `nSUBS[ch]` active-subband count for both
//!   the destination and source channels and confirming
//!   `nSUBS[ch] < nSUBS[nSourceCh]` (an empty `[nSUBS[ch],
//!   nSUBS[nSourceCh])` range yields a no-op decode but is otherwise
//!   well-formed: see [`joint_subband_decode_range_i32`] /
//!   [`joint_subband_decode_range_f64`]).
//!
//! Both the integer (i32) and floating-point (f64) decode flavours are
//! exposed because the §C.2.3 spec text does not constrain the
//! arithmetic type — the inverse-quantisation path that feeds the
//! joint-subband decode may run in either; the decoder picks per its
//! precision requirements. The i32 flavour uses `i32::wrapping_mul`
//! to mirror the §C.2.3 pseudocode's C-style `int` overflow
//! semantics; callers that require saturating arithmetic perform the
//! conversion before invoking the primitive.

use crate::{Error, Result};

/// Resolve the one-based `JOINX[ch]` field to the zero-based
/// source-channel index `nSourceCh` per §C.2.3.
///
/// Returns `None` when `joinx == 0` (joint-subband coding disabled
/// for the channel — the §C.2.3 `if (JOINX[ch] > 0)` predicate
/// rejects this code, so no source channel is named). Returns
/// `Some(joinx - 1)` for `joinx > 0`.
///
/// The dispatch predicate matches the §C.2.3 pseudocode's
/// `if (JOINX[ch] > 0)` gate directly: callers route through this
/// helper to obtain the source-channel index in one step.
///
/// # Example
///
/// ```rust
/// use oxideav_dts::joint_source_channel;
///
/// // JOINX[ch] == 0 -> joint-subband coding disabled.
/// assert_eq!(joint_source_channel(0), None);
/// // JOINX[ch] == 1 -> source channel index 0 (first primary channel).
/// assert_eq!(joint_source_channel(1), Some(0));
/// // JOINX[ch] == 5 -> source channel index 4 (fifth primary channel).
/// assert_eq!(joint_source_channel(5), Some(4));
/// ```
#[must_use]
pub fn joint_source_channel(joinx: u8) -> Option<u8> {
    if joinx == 0 {
        None
    } else {
        Some(joinx - 1)
    }
}

/// Returns `true` when channel `ch`'s `JOINX[ch]` selector enables
/// joint-subband coding per §C.2.3.
///
/// This is the §C.2.3 dispatch predicate (`JOINX[ch] > 0`). Callers
/// that already hold a `JOINX[ch]` value can branch on this directly
/// before calling [`joint_subband_decode_range_i32`] /
/// [`joint_subband_decode_range_f64`].
#[must_use]
pub fn joint_subband_required(joinx: u8) -> bool {
    joinx > 0
}

/// Decode the §C.2.3 joint-subband copy + scale for **one
/// destination channel** across the active-subband range
/// `[n_subs_dst, n_subs_src)`.
///
/// `dst_subbands` is the destination channel's per-subband
/// `aSubband[n].aSample[]` slice-of-slices, ordered subband
/// `0..n_subs_src` (i.e. enough storage to hold subbands up to the
/// source's upper bound). `src_subbands` is the source channel's
/// per-subband sample slice-of-slices, same layout. `scales` carries
/// one `JOIN_SCALES[ch][n]` value per subband in the imported range
/// `[n_subs_dst, n_subs_src)` (i.e. `scales.len() == n_subs_src -
/// n_subs_dst`).
///
/// For each `n ∈ [n_subs_dst, n_subs_src)` the destination subband
/// is overwritten with `scales[n - n_subs_dst] * src_subbands[n]`,
/// sample-by-sample. Subbands outside that range are not touched.
///
/// # Errors
///
/// - [`Error::JointSubbandShapeMismatch`] if any structural
///   invariant of the §C.2.3 pseudocode is violated:
///   `n_subs_dst > n_subs_src` (the imported range would run
///   backwards), `dst_subbands.len() < n_subs_src` or
///   `src_subbands.len() < n_subs_src` (the per-channel subband
///   arrays do not extend up to `n_subs_src`),
///   `scales.len() != n_subs_src - n_subs_dst` (the scales slice does
///   not cover the imported range exactly), or a per-subband length
///   disagreement between the destination and source samples for
///   any `n ∈ [n_subs_dst, n_subs_src)`.
///
/// # Example
///
/// ```rust
/// use oxideav_dts::joint_subband_decode_range_i32;
///
/// // nSUBS[dst] = 1, nSUBS[src] = 3 -> import subbands 1 and 2 from src.
/// // Each subband carries 8 * nSSC = 4 samples; subband 0 of dst is
/// // left alone.
/// let mut dst_s0 = [0i32; 4];          // dst subband 0 (unchanged)
/// let mut dst_s1 = [0i32; 4];          // dst subband 1 (will be overwritten)
/// let mut dst_s2 = [0i32; 4];          // dst subband 2 (will be overwritten)
/// let mut dst: [&mut [i32]; 3] = [&mut dst_s0, &mut dst_s1, &mut dst_s2];
///
/// let src_s0 = [0i32; 4];              // not in import range
/// let src_s1 = [10i32, 20, 30, 40];
/// let src_s2 = [-1i32, -2, -3, -4];
/// let src: [&[i32]; 3] = [&src_s0, &src_s1, &src_s2];
///
/// // JOIN_SCALES[dst][1] = 2, JOIN_SCALES[dst][2] = 3
/// let scales = [2i32, 3];
/// joint_subband_decode_range_i32(&mut dst, &src, &scales, 1, 3).unwrap();
///
/// assert_eq!(dst_s0, [0, 0, 0, 0]);             // untouched
/// assert_eq!(dst_s1, [20, 40, 60, 80]);         // 2 * src_s1
/// assert_eq!(dst_s2, [-3, -6, -9, -12]);        // 3 * src_s2
/// ```
pub fn joint_subband_decode_range_i32(
    dst_subbands: &mut [&mut [i32]],
    src_subbands: &[&[i32]],
    scales: &[i32],
    n_subs_dst: usize,
    n_subs_src: usize,
) -> Result<()> {
    validate_joint_shape(
        dst_subbands.len(),
        src_subbands.len(),
        scales.len(),
        n_subs_dst,
        n_subs_src,
    )?;
    for n in n_subs_dst..n_subs_src {
        let dst = &mut dst_subbands[n];
        let src = src_subbands[n];
        if dst.len() != src.len() {
            return Err(Error::JointSubbandShapeMismatch {
                dst_len: dst.len(),
                src_len: src.len(),
            });
        }
        let scale = scales[n - n_subs_dst];
        for (d, s) in dst.iter_mut().zip(src.iter()) {
            *d = scale.wrapping_mul(*s);
        }
    }
    Ok(())
}

/// Floating-point counterpart to [`joint_subband_decode_range_i32`].
///
/// Same matrix copy + scale; chosen by callers that consume the
/// reconstructed-subband samples in floating-point.
///
/// # Errors
///
/// Same error contract as the i32 variant.
///
/// # Example
///
/// ```rust
/// use oxideav_dts::joint_subband_decode_range_f64;
///
/// let mut dst_s0 = [0.0_f64; 2];
/// let mut dst_s1 = [0.0_f64; 2];
/// let mut dst: [&mut [f64]; 2] = [&mut dst_s0, &mut dst_s1];
///
/// let src_s0 = [0.0_f64; 2];
/// let src_s1 = [0.5_f64, 1.5];
/// let src: [&[f64]; 2] = [&src_s0, &src_s1];
///
/// let scales = [2.0_f64];
/// joint_subband_decode_range_f64(&mut dst, &src, &scales, 1, 2).unwrap();
/// assert_eq!(dst_s0, [0.0, 0.0]);
/// assert_eq!(dst_s1, [1.0, 3.0]);
/// ```
pub fn joint_subband_decode_range_f64(
    dst_subbands: &mut [&mut [f64]],
    src_subbands: &[&[f64]],
    scales: &[f64],
    n_subs_dst: usize,
    n_subs_src: usize,
) -> Result<()> {
    validate_joint_shape(
        dst_subbands.len(),
        src_subbands.len(),
        scales.len(),
        n_subs_dst,
        n_subs_src,
    )?;
    for n in n_subs_dst..n_subs_src {
        let dst = &mut dst_subbands[n];
        let src = src_subbands[n];
        if dst.len() != src.len() {
            return Err(Error::JointSubbandShapeMismatch {
                dst_len: dst.len(),
                src_len: src.len(),
            });
        }
        let scale = scales[n - n_subs_dst];
        for (d, s) in dst.iter_mut().zip(src.iter()) {
            *d = scale * *s;
        }
    }
    Ok(())
}

/// Shape-check the §C.2.3 join-range geometry common to both the
/// integer and floating-point primitives.
fn validate_joint_shape(
    dst_outer: usize,
    src_outer: usize,
    scales_len: usize,
    n_subs_dst: usize,
    n_subs_src: usize,
) -> Result<()> {
    // The §C.2.3 loop `for (n = nSUBS[ch]; n < nSUBS[nSourceCh]; n++)`
    // is well-defined only when the destination's active-subband upper
    // bound does not exceed the source's. An empty range
    // `nSUBS[ch] == nSUBS[nSourceCh]` is permitted (it yields a no-op
    // decode); strict `>` is the error.
    if n_subs_dst > n_subs_src {
        return Err(Error::JointSubbandShapeMismatch {
            dst_len: n_subs_dst,
            src_len: n_subs_src,
        });
    }
    // Both per-channel subband arrays must extend up to `n_subs_src`
    // — the imported range `[n_subs_dst, n_subs_src)` must have valid
    // backing storage in both `aPrmCh[ch].aSubband[n]` (destination,
    // written) and `aPrmCh[nSourceCh].aSubband[n]` (source, read).
    if dst_outer < n_subs_src {
        return Err(Error::JointSubbandShapeMismatch {
            dst_len: dst_outer,
            src_len: n_subs_src,
        });
    }
    if src_outer < n_subs_src {
        return Err(Error::JointSubbandShapeMismatch {
            dst_len: n_subs_src,
            src_len: src_outer,
        });
    }
    // `JOIN_SCALES[ch][n]` is indexed by the inner loop variable `n`
    // over the imported range exactly — neither shorter nor longer.
    let expected_scales = n_subs_src - n_subs_dst;
    if scales_len != expected_scales {
        return Err(Error::JointSubbandShapeMismatch {
            dst_len: scales_len,
            src_len: expected_scales,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------
    // Dispatch / source-channel-resolution tests
    // -----------------------------------------------------------

    #[test]
    fn source_channel_zero_means_disabled() {
        assert_eq!(joint_source_channel(0), None);
        assert!(!joint_subband_required(0));
    }

    #[test]
    fn source_channel_resolves_one_based_to_zero_based() {
        // Per the §C.2.3 inline comment: "counts channels as 1,2,3,4,5,
        // so minus 1." Walk the documented 1..=5 range to confirm.
        for joinx in 1u8..=5 {
            assert_eq!(joint_source_channel(joinx), Some(joinx - 1));
            assert!(joint_subband_required(joinx));
        }
    }

    #[test]
    fn source_channel_handles_full_u8_range() {
        // Defensive: a future round may surface a wider JOINX field;
        // the resolver still handles the full u8 domain (the only
        // special-cased code is 0).
        for joinx in 1u8..=u8::MAX {
            assert_eq!(joint_source_channel(joinx), Some(joinx - 1));
            assert!(joint_subband_required(joinx));
        }
    }

    // -----------------------------------------------------------
    // i32 range decode — happy-path matrix tests
    // -----------------------------------------------------------

    #[test]
    fn i32_basic_copy_and_scale() {
        // nSUBS[dst] = 0 -> all subbands of dst are imported from src.
        // Two subbands, three samples each.
        let mut dst_s0 = [0i32; 3];
        let mut dst_s1 = [0i32; 3];
        let mut dst: [&mut [i32]; 2] = [&mut dst_s0, &mut dst_s1];
        let src_s0 = [1i32, 2, 3];
        let src_s1 = [4i32, 5, 6];
        let src: [&[i32]; 2] = [&src_s0, &src_s1];
        let scales = [2i32, 3];
        joint_subband_decode_range_i32(&mut dst, &src, &scales, 0, 2).unwrap();
        assert_eq!(dst_s0, [2, 4, 6]);
        assert_eq!(dst_s1, [12, 15, 18]);
    }

    #[test]
    fn i32_leaves_subbands_below_n_subs_dst_untouched() {
        // nSUBS[dst] = 1: subband 0 of dst must remain unchanged.
        let mut dst_s0 = [7i32, 8, 9];
        let mut dst_s1 = [0i32; 3];
        let mut dst: [&mut [i32]; 2] = [&mut dst_s0, &mut dst_s1];
        let src_s0 = [1i32, 2, 3]; // not imported
        let src_s1 = [4i32, 5, 6];
        let src: [&[i32]; 2] = [&src_s0, &src_s1];
        let scales = [1i32];
        joint_subband_decode_range_i32(&mut dst, &src, &scales, 1, 2).unwrap();
        assert_eq!(dst_s0, [7, 8, 9]);
        assert_eq!(dst_s1, [4, 5, 6]);
    }

    #[test]
    fn i32_empty_range_is_no_op() {
        // nSUBS[dst] == nSUBS[src]: the §C.2.3 loop has zero
        // iterations. Decoder is well-formed and the destination is
        // not touched.
        let mut dst_s0 = [9i32, 9, 9];
        let mut dst: [&mut [i32]; 1] = [&mut dst_s0];
        let src_s0 = [1i32, 1, 1];
        let src: [&[i32]; 1] = [&src_s0];
        let scales: [i32; 0] = [];
        joint_subband_decode_range_i32(&mut dst, &src, &scales, 1, 1).unwrap();
        assert_eq!(dst_s0, [9, 9, 9]);
    }

    #[test]
    fn i32_zero_scale_zeroes_destination() {
        // JOIN_SCALES[ch][n] = 0 -> the imported subband is zeroed.
        let mut dst_s0 = [99i32; 4];
        let mut dst: [&mut [i32]; 1] = [&mut dst_s0];
        let src_s0 = [1i32, 2, 3, 4];
        let src: [&[i32]; 1] = [&src_s0];
        let scales = [0i32];
        joint_subband_decode_range_i32(&mut dst, &src, &scales, 0, 1).unwrap();
        assert_eq!(dst_s0, [0; 4]);
    }

    #[test]
    fn i32_negative_scale_inverts_sign() {
        let mut dst_s0 = [0i32; 4];
        let mut dst: [&mut [i32]; 1] = [&mut dst_s0];
        let src_s0 = [1i32, -2, 3, -4];
        let src: [&[i32]; 1] = [&src_s0];
        let scales = [-1i32];
        joint_subband_decode_range_i32(&mut dst, &src, &scales, 0, 1).unwrap();
        assert_eq!(dst_s0, [-1, 2, -3, 4]);
    }

    #[test]
    fn i32_wrapping_multiplication_does_not_panic() {
        // The §C.2.3 pseudocode uses `int * int` C semantics: wraps
        // on overflow. Confirm the i32 variant uses wrapping_mul, so
        // the worst-case `i32::MIN * -1` (which is undefined in
        // safe-Rust as `*`) does not panic.
        let mut dst_s0 = [0i32; 1];
        let mut dst: [&mut [i32]; 1] = [&mut dst_s0];
        let src_s0 = [i32::MIN];
        let src: [&[i32]; 1] = [&src_s0];
        let scales = [-1i32];
        joint_subband_decode_range_i32(&mut dst, &src, &scales, 0, 1).unwrap();
        // i32::MIN.wrapping_mul(-1) == i32::MIN
        assert_eq!(dst_s0[0], i32::MIN);
    }

    #[test]
    fn i32_writes_only_inside_range() {
        // nSUBS[dst] = 2, nSUBS[src] = 4: only subbands 2 and 3 of dst
        // are overwritten. Subbands 0 and 1 of dst stay as-is; we
        // don't need source data for them.
        let mut dst_s0 = [100i32; 2];
        let mut dst_s1 = [101i32; 2];
        let mut dst_s2 = [0i32; 2];
        let mut dst_s3 = [0i32; 2];
        let mut dst: [&mut [i32]; 4] = [&mut dst_s0, &mut dst_s1, &mut dst_s2, &mut dst_s3];
        let src_s0 = [0i32; 2];
        let src_s1 = [0i32; 2];
        let src_s2 = [10i32, 20];
        let src_s3 = [-5i32, 7];
        let src: [&[i32]; 4] = [&src_s0, &src_s1, &src_s2, &src_s3];
        let scales = [2i32, 3];
        joint_subband_decode_range_i32(&mut dst, &src, &scales, 2, 4).unwrap();
        assert_eq!(dst_s0, [100, 100]);
        assert_eq!(dst_s1, [101, 101]);
        assert_eq!(dst_s2, [20, 40]);
        assert_eq!(dst_s3, [-15, 21]);
    }

    // -----------------------------------------------------------
    // i32 range decode — error-path shape tests
    // -----------------------------------------------------------

    #[test]
    fn i32_rejects_dst_above_src() {
        // n_subs_dst > n_subs_src: the loop is undefined per §C.2.3.
        let mut dst: [&mut [i32]; 0] = [];
        let src: [&[i32]; 0] = [];
        let scales: [i32; 0] = [];
        let err = joint_subband_decode_range_i32(&mut dst, &src, &scales, 5, 2).unwrap_err();
        assert!(matches!(
            err,
            Error::JointSubbandShapeMismatch {
                dst_len: 5,
                src_len: 2,
            }
        ));
    }

    #[test]
    fn i32_rejects_dst_outer_too_short() {
        // dst_subbands.len() < n_subs_src: storage doesn't reach the
        // upper bound of the imported range.
        let mut s0 = [0i32; 2];
        let mut dst: [&mut [i32]; 1] = [&mut s0];
        let src_s0 = [0i32; 2];
        let src_s1 = [0i32; 2];
        let src: [&[i32]; 2] = [&src_s0, &src_s1];
        let scales = [1i32, 1];
        let err = joint_subband_decode_range_i32(&mut dst, &src, &scales, 0, 2).unwrap_err();
        assert!(matches!(
            err,
            Error::JointSubbandShapeMismatch {
                dst_len: 1,
                src_len: 2,
            }
        ));
    }

    #[test]
    fn i32_rejects_src_outer_too_short() {
        let mut s0 = [0i32; 2];
        let mut s1 = [0i32; 2];
        let mut dst: [&mut [i32]; 2] = [&mut s0, &mut s1];
        let src_s0 = [0i32; 2];
        let src: [&[i32]; 1] = [&src_s0];
        let scales = [1i32, 1];
        let err = joint_subband_decode_range_i32(&mut dst, &src, &scales, 0, 2).unwrap_err();
        assert!(matches!(
            err,
            Error::JointSubbandShapeMismatch {
                dst_len: 2,
                src_len: 1,
            }
        ));
    }

    #[test]
    fn i32_rejects_scales_length_mismatch() {
        // scales.len() must equal n_subs_src - n_subs_dst exactly.
        let mut s0 = [0i32; 2];
        let mut s1 = [0i32; 2];
        let mut dst: [&mut [i32]; 2] = [&mut s0, &mut s1];
        let src_s0 = [0i32; 2];
        let src_s1 = [0i32; 2];
        let src: [&[i32]; 2] = [&src_s0, &src_s1];
        let scales = [1i32; 3]; // expected 2
        let err = joint_subband_decode_range_i32(&mut dst, &src, &scales, 0, 2).unwrap_err();
        assert!(matches!(
            err,
            Error::JointSubbandShapeMismatch {
                dst_len: 3,
                src_len: 2,
            }
        ));
    }

    #[test]
    fn i32_rejects_inner_length_mismatch() {
        let mut s0 = [0i32; 2];
        let mut s1 = [0i32; 3]; // inner mismatch with src_s1
        let mut dst: [&mut [i32]; 2] = [&mut s0, &mut s1];
        let src_s0 = [0i32; 2];
        let src_s1 = [0i32; 2];
        let src: [&[i32]; 2] = [&src_s0, &src_s1];
        let scales = [1i32, 1];
        let err = joint_subband_decode_range_i32(&mut dst, &src, &scales, 0, 2).unwrap_err();
        assert!(matches!(
            err,
            Error::JointSubbandShapeMismatch {
                dst_len: 3,
                src_len: 2,
            }
        ));
    }

    // -----------------------------------------------------------
    // f64 range decode — happy-path + error-path
    // -----------------------------------------------------------

    #[test]
    fn f64_basic_copy_and_scale() {
        let mut dst_s0 = [0.0_f64; 3];
        let mut dst_s1 = [0.0_f64; 3];
        let mut dst: [&mut [f64]; 2] = [&mut dst_s0, &mut dst_s1];
        let src_s0 = [1.0_f64, 2.0, 3.0];
        let src_s1 = [4.0_f64, 5.0, 6.0];
        let src: [&[f64]; 2] = [&src_s0, &src_s1];
        let scales = [0.5_f64, -0.25];
        joint_subband_decode_range_f64(&mut dst, &src, &scales, 0, 2).unwrap();
        assert_eq!(dst_s0, [0.5, 1.0, 1.5]);
        assert_eq!(dst_s1, [-1.0, -1.25, -1.5]);
    }

    #[test]
    fn f64_empty_range_is_no_op() {
        let mut dst_s0 = [42.0_f64; 2];
        let mut dst: [&mut [f64]; 1] = [&mut dst_s0];
        let src_s0 = [0.0_f64; 2];
        let src: [&[f64]; 1] = [&src_s0];
        let scales: [f64; 0] = [];
        joint_subband_decode_range_f64(&mut dst, &src, &scales, 1, 1).unwrap();
        assert_eq!(dst_s0, [42.0, 42.0]);
    }

    #[test]
    fn f64_propagates_shape_errors() {
        let mut dst_s0 = [0.0_f64; 2];
        let mut dst: [&mut [f64]; 1] = [&mut dst_s0];
        let src_s0 = [0.0_f64; 2];
        let src_s1 = [0.0_f64; 2];
        let src: [&[f64]; 2] = [&src_s0, &src_s1];
        let scales = [1.0_f64, 1.0];
        let err = joint_subband_decode_range_f64(&mut dst, &src, &scales, 0, 2).unwrap_err();
        assert!(matches!(
            err,
            Error::JointSubbandShapeMismatch {
                dst_len: 1,
                src_len: 2,
            }
        ));
    }

    #[test]
    fn f64_inner_length_mismatch() {
        let mut s0 = [0.0_f64; 2];
        let mut s1 = [0.0_f64; 4];
        let mut dst: [&mut [f64]; 2] = [&mut s0, &mut s1];
        let src_s0 = [0.0_f64; 2];
        let src_s1 = [0.0_f64; 3];
        let src: [&[f64]; 2] = [&src_s0, &src_s1];
        let scales = [1.0_f64, 1.0];
        let err = joint_subband_decode_range_f64(&mut dst, &src, &scales, 0, 2).unwrap_err();
        assert!(matches!(
            err,
            Error::JointSubbandShapeMismatch {
                dst_len: 4,
                src_len: 3,
            }
        ));
    }

    // -----------------------------------------------------------
    // End-to-end §C.2.3 sweep — hand-computed expected
    // -----------------------------------------------------------

    #[test]
    fn full_sweep_matches_spec_pseudocode_directly() {
        // nSUBS[dst] = 2, nSUBS[src] = 5, 8 * nSSC = 8.
        // Cross-check against an independent hand-computed expected.
        let n_dst_subs = 2usize;
        let n_src_subs = 5usize;
        let n_samples = 8usize;

        let mut dst_storage: Vec<Vec<i32>> =
            (0..n_src_subs).map(|_| vec![999_i32; n_samples]).collect();
        let src_storage: Vec<Vec<i32>> = (0..n_src_subs)
            .map(|s| (0..n_samples).map(|i| (s * 10 + i) as i32).collect())
            .collect();
        let scales: Vec<i32> = (n_dst_subs..n_src_subs).map(|n| n as i32).collect();

        // Hand-compute the expected.
        let mut expected: Vec<Vec<i32>> = dst_storage.clone();
        for n in n_dst_subs..n_src_subs {
            let scale = scales[n - n_dst_subs];
            for (i, sample) in src_storage[n].iter().enumerate() {
                expected[n][i] = scale.wrapping_mul(*sample);
            }
        }

        {
            let mut dst_slices: Vec<&mut [i32]> =
                dst_storage.iter_mut().map(|v| v.as_mut_slice()).collect();
            let src_slices: Vec<&[i32]> = src_storage.iter().map(|v| v.as_slice()).collect();
            joint_subband_decode_range_i32(
                &mut dst_slices,
                &src_slices,
                &scales,
                n_dst_subs,
                n_src_subs,
            )
            .unwrap();
        }
        assert_eq!(dst_storage, expected);
    }
}

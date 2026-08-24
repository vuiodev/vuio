//! DTS Coherent Acoustics — §C.2.1 Block Code.
//!
//! Round 232 (2026-06-04) lands the §C.2.1 block-code decoder, the
//! prerequisite step that turns a single multi-symbol code word into
//! the array of quantisation indices the rest of the §C.2 chain
//! (§C.2.2 inverse ADPCM, §C.2.3 joint subband, §C.2.4 sum/difference,
//! and downstream stages) consumes. Each "block" packs `n_elements`
//! quantisation indices from a `n_levels`-level alphabet into one
//! integer code word using mixed-radix arithmetic.
//!
//! Source: ETSI TS 102 114 V1.3.1 (2011-08), Annex C (informative)
//! §C.2.1 "Block Code" (staged PDF p.182–183) at
//! `docs/audio/dts/etsi-ts-102114-dts-coherent-acoustics.pdf`. The
//! spec gives two decoder variants:
//!
//! 1. A **table look-up** decoder that walks a rearranged §D.6
//!    code-book row by row, subtracting the largest entry that fits
//!    and recording the row index.
//! 2. A **modulus / integer-division** decoder that does the same
//!    extraction with one `nCode % nNumLevel` + one
//!    `nCode /= nNumLevel` per quantisation index, no table needed.
//!
//! Both produce the same quantisation-index array, ordered
//! "first-element first" (i.e. element 0 is the **first**
//! quantisation index extracted, element `n_elements - 1` is the
//! last). The spec's reproduced normative pseudocode for the
//! modulus/division variant reads (reproduced as documented):
//!
//! ```text
//! int DecodeBlockCode(int nCode, int *pnValue) {
//!     // nCode: Input code to be decoded.
//!     // nNumElement: Number of elements (samples) encoded in a block.
//!     // nNumLevel: Number of quantization levels.
//!     // *pnValue: Array of decoded sample values.
//!     nOffset = (nNumLevel-1)>>1;
//!     for (int n=0; n< nNumElement; n++) {
//!         pnValue[n] = (nCode % nNumLevel) - nOffset;
//!         nCode /= nNumLevel;
//!     }
//!     if ( nCode == 0 ) return 1;
//!     else { printf("ERROR: block code lock-up fail.\n"); return NULL; }
//! }
//! ```
//!
//! Worked example reproduced from the spec text: the three-level
//! four-element block-code `nCode = 64` decodes element-by-element
//! as
//!
//! | step | reads          | quotient | remainder | index             |
//! |------|----------------|----------|-----------|-------------------|
//! | 1    | `64 = 3·21+1`  | `21`     | `1`       | `1 - 1 = 0`       |
//! | 2    | `21 = 3·7+0`   | `7`      | `0`       | `0 - 1 = -1`      |
//! | 3    | `7  = 3·2+1`   | `2`      | `1`       | `1 - 1 = 0`       |
//! | 4    | `2  = 3·0+2`   | `0`      | `2`       | `2 - 1 = +1`      |
//!
//! producing the quantisation-index array `[0, -1, 0, +1]`. (The
//! spec text walks the same example for both decoder variants and
//! records the identical output.)
//!
//! # Mid-range offset
//!
//! `nOffset = (nNumLevel - 1) >> 1` centres the index alphabet on
//! zero: a 3-level alphabet uses indices `{-1, 0, +1}` (`offset =
//! 1`), a 5-level alphabet `{-2, -1, 0, +1, +2}` (`offset = 2`), a
//! 7-level alphabet `{-3, -2, -1, 0, +1, +2, +3}` (`offset = 3`),
//! etc. The spec's `(nNumLevel - 1) >> 1` form is integer
//! arithmetic (right shift by one); for any odd `n_levels` it equals
//! `(n_levels - 1) / 2`. The §C.2.1 spec text only worked-examples
//! odd-level alphabets (3, 5, 7, 9, 13, 17, 25 per the §D.6
//! sub-clauses); even-level alphabets are not enumerated as
//! block-code variants.
//!
//! # End-of-code consistency check
//!
//! After consuming all `n_elements` quotient steps, the §C.2.1
//! pseudocode's success criterion is `nCode == 0`. A non-zero
//! residual means either the input code word was out-of-range for
//! the declared `(n_elements, n_levels)` block, or one of the
//! parameters was wrong. The Rust API surfaces this as a recoverable
//! [`Error::BlockCodeResidual`] rather than the spec's
//! `printf + return NULL`.
//!
//! # Scope
//!
//! This round lands the modulus / integer-division decoder
//! ([`decode_block_code`]) plus the dispatch predicate / accessor
//! surface. The table-look-up decoder variant requires the §D.6
//! "rearranged" code-book rows enumerated as Table C-1 (3-level
//! 4-element), with parallel rearranged tables for 5/7/9/13/17/25
//! levels not transcribed into this crate yet — left to a follow-up
//! round once the §D.6 + Table C-1 tables are extracted.
//!
//! Both decoders produce the same quantisation-index array per the
//! spec's worked example, so the table variant is purely an
//! optimisation; the modulus/division variant is sufficient to
//! decode the entire §C.2.1 surface end-to-end.
//!
//! # Arithmetic
//!
//! The §C.2.1 pseudocode uses plain C `int` arithmetic for the
//! quantisation-index offset (`(nCode % nNumLevel) - nOffset`); the
//! quantisation indices are small signed values
//! (`-(n_levels-1)/2..=(n_levels-1)/2`) so overflow is not a
//! concern. The Rust impl works in `i32` for the indices and `u32`
//! for the code-word arithmetic (the modulus / integer-division
//! steps); the spec does not bound the code-word width, but the
//! §D.6.1 worked example fits within seven bits (`64`) and the
//! larger-level §D.6.x tables fit within the spec's documented bit
//! widths for those clauses (cited in §5.4 / §6 unpack routines).

use crate::{Error, Result};

/// Decode one §C.2.1 block-code word into its quantisation-index
/// array, in place.
///
/// On entry:
///
/// - `code` is the unsigned block-code word read from the bit
///   stream.
/// - `n_levels` is the quantisation-level count of the alphabet
///   (`nNumLevel` in the spec). Must be `>= 2`.
/// - `output` is the destination quantisation-index array. The
///   spec's `nNumElement` is taken from `output.len()`. On return,
///   `output[0..output.len()]` carries the decoded quantisation
///   indices, ordered first-element-first.
///
/// On return:
///
/// - `Ok(())` if every quantisation index extracted cleanly and
///   the residual code word reached zero after the last element
///   (the §C.2.1 success criterion `nCode == 0`).
///
/// # Errors
///
/// - [`Error::BlockCodeLevelsOutOfRange`] if `n_levels < 2`. A
///   one-level alphabet has only the index `0` and cannot encode
///   information; the spec's `(nNumLevel-1)>>1` offset and
///   `nCode % nNumLevel` recurrence are not defined for `n_levels
///   == 0` or `n_levels == 1` (the latter would cause `nCode /=
///   nNumLevel` to never advance).
/// - [`Error::BlockCodeResidual`] if, after walking all
///   `output.len()` elements, the residual code word is non-zero.
///   The §C.2.1 spec text treats this as a fatal "ERROR: block
///   code look-up fail" condition; the Rust API surfaces it as a
///   recoverable error so callers can distinguish a corrupted
///   bit-stream segment from a structural decoder bug.
///
/// An empty `output` slice is **not** an error: the §C.2.1
/// pseudocode's success condition is `nCode == 0` regardless of
/// element count, so the zero-element decode succeeds when (and
/// only when) `code == 0`.
///
/// # Example
///
/// Spec worked example (§C.2.1 PDF p.182):
///
/// ```rust
/// use oxideav_dts::decode_block_code;
///
/// let mut out = [0_i32; 4];
/// decode_block_code(64, 3, &mut out).unwrap();
/// // 3-level 4-element block code: `64` decodes to (0, -1, 0, +1).
/// assert_eq!(out, [0, -1, 0, 1]);
/// ```
pub fn decode_block_code(code: u32, n_levels: u32, output: &mut [i32]) -> Result<()> {
    if n_levels < 2 {
        return Err(Error::BlockCodeLevelsOutOfRange { n_levels });
    }
    // The spec writes `nOffset = (nNumLevel - 1) >> 1`. For
    // `n_levels >= 2` this fits in i32 because `n_levels` would
    // have to exceed 2^31 + 1 to overflow, and the §C.2.1 block-code
    // alphabets enumerated by §D.6 cap at 25 levels.
    let offset = ((n_levels - 1) >> 1) as i32;
    let mut residual = code;
    for slot in output.iter_mut() {
        // `nCode % nNumLevel` — the index of this element within
        // the alphabet, biased by `nOffset` so the alphabet is
        // centred on zero.
        let remainder = (residual % n_levels) as i32;
        *slot = remainder - offset;
        // `nCode /= nNumLevel` — shift to the next element's
        // mixed-radix digit.
        residual /= n_levels;
    }
    // §C.2.1 success criterion: after consuming all elements, the
    // residual code word must equal zero. A non-zero residual means
    // the input code word was out-of-range for the declared
    // (n_elements, n_levels) block.
    if residual != 0 {
        return Err(Error::BlockCodeResidual {
            residual,
            n_elements: output.len(),
            n_levels,
        });
    }
    Ok(())
}

/// Compute the §C.2.1 mid-range offset `nOffset = (nNumLevel - 1) >>
/// 1` for the given quantisation-level count.
///
/// This is the bias applied to each `(nCode % nNumLevel)` remainder
/// so the decoded quantisation-index alphabet is centred on zero
/// (`{-(n_levels-1)/2, ..., 0, ..., (n_levels-1)/2}`). Exposed as a
/// public helper for callers that need to size index buffers or
/// validate alphabet ranges by the same invariant the spec writes
/// against.
///
/// Returns the offset for any `n_levels >= 1`. For `n_levels == 0`
/// the offset is mathematically undefined and the function returns
/// `0` (the integer-arithmetic interpretation of `(0 - 1) >> 1`
/// after an unsigned `wrapping_sub` would be `u32::MAX >> 1`, which
/// is not what the spec means); guard `n_levels == 0` at the call
/// site if a numeric answer is required.
///
/// # Example
///
/// ```rust
/// use oxideav_dts::block_code_offset;
///
/// // 3-level: indices in {-1, 0, +1} → offset 1
/// assert_eq!(block_code_offset(3), 1);
/// // 5-level: indices in {-2, -1, 0, +1, +2} → offset 2
/// assert_eq!(block_code_offset(5), 2);
/// // 7-level: indices in {-3, -2, -1, 0, +1, +2, +3} → offset 3
/// assert_eq!(block_code_offset(7), 3);
/// // 25-level (largest in §D.6): offset 12
/// assert_eq!(block_code_offset(25), 12);
/// ```
pub fn block_code_offset(n_levels: u32) -> i32 {
    if n_levels == 0 {
        0
    } else {
        ((n_levels - 1) >> 1) as i32
    }
}

/// Compute the maximum code-word value the §C.2.1 block decoder
/// will accept without a [`Error::BlockCodeResidual`] error, for the
/// given `(n_elements, n_levels)` block dimensions.
///
/// The mixed-radix encoding represents each element as a digit in
/// base `n_levels`, so an `n_elements`-element block uses values in
/// `0..n_levels.pow(n_elements as u32)`. The largest valid code word
/// is therefore `n_levels.pow(n_elements as u32) - 1`. Returns
/// `None` when the exponentiation overflows `u32` (which the §D.6
/// tables stay within: even 25-level × 1-element fits in five bits,
/// and the largest tabulated block-size product fits well inside
/// 32 bits).
///
/// # Example
///
/// ```rust
/// use oxideav_dts::block_code_max_code;
///
/// // §D.6.1 3-level 4-element block code: 3^4 - 1 = 80
/// assert_eq!(block_code_max_code(4, 3), Some(80));
/// // 5-level 3-element: 5^3 - 1 = 124
/// assert_eq!(block_code_max_code(3, 5), Some(124));
/// ```
pub fn block_code_max_code(n_elements: usize, n_levels: u32) -> Option<u32> {
    if n_elements == 0 {
        return Some(0);
    }
    let exp = u32::try_from(n_elements).ok()?;
    let total = n_levels.checked_pow(exp)?;
    Some(total.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §C.2.1 worked example (PDF p.182): the three-level
    /// four-element block-code `nCode = 64` decodes to the
    /// quantisation-index array `(0, -1, 0, +1)` ordered
    /// first-element-first.
    #[test]
    fn spec_worked_example_three_level_four_element_code_sixty_four() {
        let mut out = [0_i32; 4];
        decode_block_code(64, 3, &mut out).unwrap();
        assert_eq!(out, [0, -1, 0, 1]);
    }

    /// All-zero code word decodes to the all-`-offset` (bottom of
    /// alphabet) quantisation-index array: every digit is the
    /// remainder `0 % n_levels = 0`, biased by `-offset`. For a
    /// 3-level alphabet (offset 1) this is `-1`; for a 5-level
    /// alphabet (offset 2) it is `-2`; etc.
    #[test]
    fn zero_code_decodes_to_bottom_of_alphabet() {
        for (n_elements, n_levels) in [(1, 3), (4, 3), (1, 5), (3, 5), (4, 7), (3, 25)] {
            let mut out = vec![i32::MAX; n_elements];
            decode_block_code(0, n_levels, &mut out).unwrap();
            let expected = -block_code_offset(n_levels);
            assert!(
                out.iter().all(|&v| v == expected),
                "n_elements={n_elements} n_levels={n_levels} expected all={expected} got {out:?}"
            );
        }
    }

    /// Maximum valid code word for `(n_elements, n_levels)` decodes
    /// to the all-`+offset` quantisation-index array (every digit
    /// is the largest in the alphabet).
    #[test]
    fn max_code_decodes_to_top_of_alphabet() {
        // 3-level 4-element: max code = 3^4 - 1 = 80
        let mut out = [0_i32; 4];
        decode_block_code(80, 3, &mut out).unwrap();
        assert_eq!(out, [1, 1, 1, 1]);
        // 5-level 3-element: max code = 5^3 - 1 = 124
        let mut out = [0_i32; 3];
        decode_block_code(124, 5, &mut out).unwrap();
        assert_eq!(out, [2, 2, 2]);
    }

    /// One past the maximum valid code word produces a non-zero
    /// residual and surfaces [`Error::BlockCodeResidual`]: §C.2.1
    /// success requires `nCode == 0` after the last extraction.
    #[test]
    fn over_max_code_produces_residual_error() {
        // 3-level 4-element: 81 = 3^4. After 4 elements the
        // residual is 1 (digit-array carry).
        let mut out = [0_i32; 4];
        let err = decode_block_code(81, 3, &mut out).unwrap_err();
        assert!(matches!(
            err,
            Error::BlockCodeResidual {
                residual: 1,
                n_elements: 4,
                n_levels: 3,
            }
        ));
    }

    /// `n_levels < 2` is rejected: a one-level alphabet has only
    /// the index `0` and the spec's mixed-radix recurrence is
    /// undefined (division by zero / one degenerates to an
    /// infinite loop on any non-zero `code`).
    #[test]
    fn levels_less_than_two_rejected() {
        let mut out = [0_i32; 4];
        assert!(matches!(
            decode_block_code(0, 0, &mut out).unwrap_err(),
            Error::BlockCodeLevelsOutOfRange { n_levels: 0 }
        ));
        assert!(matches!(
            decode_block_code(0, 1, &mut out).unwrap_err(),
            Error::BlockCodeLevelsOutOfRange { n_levels: 1 }
        ));
    }

    /// Empty `output` slice with `code == 0` succeeds (the §C.2.1
    /// success criterion `nCode == 0` is met trivially without
    /// extracting any digits).
    #[test]
    fn empty_output_succeeds_when_code_is_zero() {
        let mut out: [i32; 0] = [];
        decode_block_code(0, 3, &mut out).unwrap();
    }

    /// Empty `output` slice with `code != 0` surfaces the residual
    /// error: no extraction step ran, so the residual is the full
    /// input code word.
    #[test]
    fn empty_output_with_non_zero_code_residual_error() {
        let mut out: [i32; 0] = [];
        let err = decode_block_code(42, 3, &mut out).unwrap_err();
        assert!(matches!(
            err,
            Error::BlockCodeResidual {
                residual: 42,
                n_elements: 0,
                n_levels: 3,
            }
        ));
    }

    /// Round-trip: every valid `code ∈ [0, n_levels.pow(n_elements))`
    /// for a small `(n_elements, n_levels)` block decodes to a
    /// distinct quantisation-index array, and re-encoding the array
    /// via the mixed-radix base recovers the original `code`. This
    /// exercises the entire valid-code-word domain.
    #[test]
    fn round_trip_three_level_four_element_block() {
        for code in 0..81_u32 {
            let mut out = [0_i32; 4];
            decode_block_code(code, 3, &mut out).unwrap();
            let offset = block_code_offset(3);
            // Re-encode least-significant-first (matches the
            // decoder's first-element-first extraction).
            let mut recovered = 0_u32;
            for &idx in out.iter().rev() {
                recovered = recovered * 3 + (idx + offset) as u32;
            }
            assert_eq!(recovered, code, "round-trip failed for code={code}");
        }
    }

    /// Round-trip for the 5-level 3-element block-code domain.
    #[test]
    fn round_trip_five_level_three_element_block() {
        for code in 0..125_u32 {
            let mut out = [0_i32; 3];
            decode_block_code(code, 5, &mut out).unwrap();
            let offset = block_code_offset(5);
            let mut recovered = 0_u32;
            for &idx in out.iter().rev() {
                recovered = recovered * 5 + (idx + offset) as u32;
            }
            assert_eq!(recovered, code, "round-trip failed for code={code}");
        }
    }

    /// Every decoded index falls within the §C.2.1 alphabet bounds
    /// `[-(n_levels - 1) / 2, (n_levels - 1) / 2]`. Exhaustive over
    /// the 3-level 4-element domain.
    #[test]
    fn decoded_indices_within_alphabet_bounds() {
        let n_levels = 3_u32;
        let offset = block_code_offset(n_levels);
        for code in 0..81_u32 {
            let mut out = [0_i32; 4];
            decode_block_code(code, n_levels, &mut out).unwrap();
            for v in out {
                assert!(
                    (-offset..=offset).contains(&v),
                    "code={code} produced out-of-alphabet index {v}"
                );
            }
        }
    }

    /// `block_code_offset` matches the spec's `(n_levels - 1) >> 1`
    /// integer-shift formula across the §D.6 enumerated alphabets.
    #[test]
    fn offset_helper_matches_spec_formula() {
        for n_levels in [3_u32, 5, 7, 9, 13, 17, 25] {
            assert_eq!(block_code_offset(n_levels), ((n_levels - 1) >> 1) as i32);
        }
    }

    /// `block_code_offset(0)` is `0` (guarded fallback), and
    /// `block_code_offset(1)` is `0` (the single-element alphabet
    /// is just the zero index).
    #[test]
    fn offset_helper_degenerate_alphabets() {
        assert_eq!(block_code_offset(0), 0);
        assert_eq!(block_code_offset(1), 0);
        assert_eq!(block_code_offset(2), 0);
    }

    /// `block_code_max_code` matches `n_levels.pow(n_elements) - 1`
    /// for the §D.6 enumerated dimensions and reports `None` on
    /// overflow.
    #[test]
    fn max_code_helper_matches_formula() {
        // §D.6.1 3-level 4-element: 80
        assert_eq!(block_code_max_code(4, 3), Some(80));
        // 5-level 3-element: 124
        assert_eq!(block_code_max_code(3, 5), Some(124));
        // 7-level 2-element: 48
        assert_eq!(block_code_max_code(2, 7), Some(48));
        // 25-level 1-element: 24
        assert_eq!(block_code_max_code(1, 25), Some(24));
        // Zero-element block: max code is 0 (only the empty
        // decode succeeds).
        assert_eq!(block_code_max_code(0, 3), Some(0));
    }

    /// Worked-example trace — the §C.2.1 PDF p.182 step-by-step
    /// table for `nCode = 64` (3-level 4-element) records
    /// quotient/remainder pairs `(21, 1)`, `(7, 0)`, `(2, 1)`,
    /// `(0, 2)` producing indices `(0, -1, 0, +1)`. Verify each
    /// intermediate quotient by stepping the recurrence manually.
    #[test]
    fn spec_worked_example_intermediate_quotients() {
        let n_levels = 3_u32;
        let offset = block_code_offset(n_levels);
        let mut residual = 64_u32;
        let expected = [(1_u32, 21_u32), (0, 7), (1, 2), (2, 0)];
        for (i, &(rem, next_q)) in expected.iter().enumerate() {
            assert_eq!(
                residual % n_levels,
                rem,
                "step {i} expected remainder {rem}"
            );
            let idx = (residual % n_levels) as i32 - offset;
            // Expected indices from the spec's worked example.
            assert_eq!(idx, [0, -1, 0, 1][i], "step {i} index disagrees");
            residual /= n_levels;
            assert_eq!(residual, next_q, "step {i} expected next quotient {next_q}");
        }
        assert_eq!(residual, 0);
    }

    /// Three-level 1-element block decode (smallest non-trivial
    /// alphabet × single element). Codes `0/1/2` decode to indices
    /// `-1/0/+1`.
    #[test]
    fn three_level_one_element_block() {
        for (code, expected) in [(0_u32, -1_i32), (1, 0), (2, 1)] {
            let mut out = [0_i32; 1];
            decode_block_code(code, 3, &mut out).unwrap();
            assert_eq!(out, [expected], "code={code} expected index {expected}");
        }
        // Code = 3 produces residual 1 after one element.
        let mut out = [0_i32; 1];
        let err = decode_block_code(3, 3, &mut out).unwrap_err();
        assert!(matches!(
            err,
            Error::BlockCodeResidual {
                residual: 1,
                n_elements: 1,
                n_levels: 3,
            }
        ));
    }

    /// `n_levels == 2` is the smallest accepted alphabet. Index
    /// alphabet is `{0, 1}` (offset = 0), and the recurrence
    /// reads the code's bits LSB-first.
    #[test]
    fn two_level_block_decodes_binary() {
        // 4-bit code 0b1011 = 11 decodes to bits LSB-first.
        let mut out = [0_i32; 4];
        decode_block_code(0b1011, 2, &mut out).unwrap();
        assert_eq!(out, [1, 1, 0, 1]);
        assert_eq!(block_code_offset(2), 0);
    }

    /// 25-level 1-element decode — the largest §D.6 alphabet.
    /// Codes `0..25` decode to indices `-12..=12`.
    #[test]
    fn twenty_five_level_single_element_alphabet() {
        let offset = block_code_offset(25);
        for code in 0..25_u32 {
            let mut out = [0_i32; 1];
            decode_block_code(code, 25, &mut out).unwrap();
            assert_eq!(out[0], code as i32 - offset);
        }
    }
}

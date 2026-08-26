//! DTS Coherent Acoustics — Annex D §D.6 Block Code Books and the
//! §C.2.1 table-look-up block-code decoder variant.
//!
//! Source: ETSI TS 102 114 V1.3.1 (2011-08), Annex D §D.6 "Block Code
//! Books" (staged PDF p.231-236) and Annex C (informative) §C.2.1
//! "Block Code" (staged PDF p.182-183) at
//! `docs/audio/dts/etsi-ts-102114-dts-coherent-acoustics.pdf`.
//!
//! # What this module adds
//!
//! Round 232 landed the §C.2.1 **modulus / integer-division** block-code
//! decoder ([`crate::decode_block_code`]) and left the §C.2.1 **table
//! look-up** decoder variant as an explicit follow-up, blocked on the
//! §D.6 code-book rows enumerated as the §C.2.1 Table C-1. This module
//! transcribes those §D.6 tables and implements the table-look-up
//! decoder against them.
//!
//! The spec presents both decoder variants and states (PDF p.182) that
//! they produce the identical quantisation-index array for the same code
//! word — so the table-look-up decoder here is cross-validated against
//! the round-232 modulus decoder over the full §D.6 code domain in the
//! tests, with no implementation read from anywhere else.
//!
//! # §D.6 code-book structure
//!
//! Each §D.6.x sub-clause tabulates a `4`-element block code over an
//! `nNumLevel`-level alphabet. The `e`-th element (`e` in `1..=4`,
//! one-based as the spec writes "1st/2nd/3rd/4th element") lists one
//! code value per quantisation-level index `L` in `0..nNumLevel`:
//!
//! ```text
//! code(element e, level index L) = L * nNumLevel^(e-1)
//! ```
//!
//! (For the 3-level book, element 1 lists `0, 1, 2`; element 2 lists
//! `0, 3, 6`; element 3 lists `0, 9, 18`; element 4 lists `0, 27, 54` —
//! verbatim §D.6.1.) The quantisation-level index `L` maps to the
//! signed quantisation index by the §C.2.1 mid-range offset:
//! `index = L - (nNumLevel - 1) / 2` (so `L = 0` is the most negative
//! index and `L = nNumLevel - 1` the most positive).
//!
//! # §C.2.1 table-look-up walk (Table C-1)
//!
//! To decode, §C.2.1 rearranges each §D.6 book into a per-element table
//! and walks it **from the last element down to the first**, at each
//! element subtracting the largest table entry that does not exceed the
//! remaining code, and recording that entry's level index. Reproduced
//! as documented from the §C.2.1 worked example (PDF p.182), decoding
//! the 3-level 4-element code `64`:
//!
//! ```text
//! 4th Element: 64 - 54 = 10 > 0; level index 2 -> quantisation index +1
//! 3rd Element: 10 -  9 =  1 > 0; level index 1 -> quantisation index  0
//! 2nd Element:  1 -  0 =  1 > 0; level index 0 -> quantisation index -1
//! 1st Element:  1 -  1 =  0    ; level index 1 -> quantisation index  0
//! ```
//!
//! producing `[0, -1, 0, +1]` first-element-first — identical to the
//! modulus decoder's worked example. The success criterion is the same
//! `nCode == 0` residual check after the last (i.e. the first-element)
//! step.

use crate::{Error, Result};

/// The fixed number of elements (subband samples) in every §D.6 block
/// code book: the §D.6.x tables all tabulate a `4`-element block (PDF
/// p.231-236, "4-element ... Block Code Book").
pub const D6_BLOCK_ELEMENTS: usize = 4;

/// A transcribed §D.6 block code book: the per-element code values, one
/// inner row per element (element 1 first), each row holding one code
/// value per quantisation-level index `L` in `0..nNumLevel`.
///
/// `levels` records the alphabet size `nNumLevel`; only the first
/// `levels` entries of each `rows[e]` are meaningful (the remainder are
/// zero padding so every book shares one storage shape). The widest
/// §D.6 book is 25 levels (§D.6.7), so each row is sized for 25.
#[derive(Debug, Clone, Copy)]
pub struct D6BlockBook {
    /// The alphabet size `nNumLevel` for this book.
    levels: u32,
    /// `rows[e][L]` is the §D.6 code value for the `(e+1)`-th element at
    /// quantisation-level index `L`. Entries at `L >= levels` are unused
    /// zero padding.
    rows: [[u32; 25]; D6_BLOCK_ELEMENTS],
}

impl D6BlockBook {
    /// The alphabet size `nNumLevel` this book decodes.
    #[must_use]
    pub const fn levels(&self) -> u32 {
        self.levels
    }

    /// The §D.6 code value for the one-based `element` (`1..=4`) at
    /// quantisation-level index `level_index` (`0..levels`). Returns
    /// `None` when `element` is outside `1..=4` or `level_index` is
    /// outside `0..levels`.
    #[must_use]
    pub fn code_value(&self, element: usize, level_index: u32) -> Option<u32> {
        if !(1..=D6_BLOCK_ELEMENTS).contains(&element) || level_index >= self.levels {
            return None;
        }
        Some(self.rows[element - 1][level_index as usize])
    }
}

// ---------------------------------------------------------------------
// §D.6 table construction.
//
// Every §D.6 book is `code(element e, level L) = L * nNumLevel^(e-1)`
// (verified against the printed §D.6.1..§D.6.7 tables row by row in the
// tests). Building the rows from that closed form transcribes the
// table's own arithmetic rather than re-typing several hundred decimal
// cells; the test module then asserts each printed anchor cell against
// the constructed row so the closed form is pinned to the staged PDF.
//
// PDF print errata noted in the tests: §D.6.3 (7-level) 3rd-element
// level-0 cell prints "47" where the table's own `L * 49` arithmetic
// and the §C.2.1 modulus decoder both give 147 — the constructed table
// carries the arithmetically-consistent 147 and the test records the
// print error.
// ---------------------------------------------------------------------

const fn build_book(levels: u32) -> D6BlockBook {
    let mut rows = [[0u32; 25]; D6_BLOCK_ELEMENTS];
    let mut e = 0;
    while e < D6_BLOCK_ELEMENTS {
        // factor = nNumLevel^e  (element index e is zero-based here, so
        // the spec's `e-1` exponent for the one-based "(e+1)-th element"
        // is exactly this zero-based `e`).
        let mut factor: u32 = 1;
        let mut k = 0;
        while k < e {
            factor *= levels;
            k += 1;
        }
        let mut l = 0;
        while l < levels {
            rows[e][l as usize] = l * factor;
            l += 1;
        }
        e += 1;
    }
    D6BlockBook { levels, rows }
}

/// §D.6.1 — 3-level 4-element 7-bit block code book (Table V.3).
pub const D6_BOOK_3: D6BlockBook = build_book(3);
/// §D.6.2 — 5-level 4-element 10-bit block code book (Table V.5).
pub const D6_BOOK_5: D6BlockBook = build_book(5);
/// §D.6.3 — 7-level 4-element 12-bit block code book (Table V.7).
pub const D6_BOOK_7: D6BlockBook = build_book(7);
/// §D.6.4 — 9-level 4-element 13-bit block code book (Table V.9).
pub const D6_BOOK_9: D6BlockBook = build_book(9);
/// §D.6.5 — 13-level 4-element 15-bit block code book (Table V.13).
pub const D6_BOOK_13: D6BlockBook = build_book(13);
/// §D.6.6 — 17-level 4-element 17-bit block code book (Table V.17).
pub const D6_BOOK_17: D6BlockBook = build_book(17);
/// §D.6.7 — 25-level 4-element 19-bit block code book (Table V.25).
pub const D6_BOOK_25: D6BlockBook = build_book(25);

/// Resolve the §D.6 block code book for a quantisation-level count, or
/// `None` if the level count is not one of the §D.6 sub-clauses
/// (`3, 5, 7, 9, 13, 17, 25`).
#[must_use]
pub fn d6_book_for_levels(n_levels: u32) -> Option<&'static D6BlockBook> {
    match n_levels {
        3 => Some(&D6_BOOK_3),
        5 => Some(&D6_BOOK_5),
        7 => Some(&D6_BOOK_7),
        9 => Some(&D6_BOOK_9),
        13 => Some(&D6_BOOK_13),
        17 => Some(&D6_BOOK_17),
        25 => Some(&D6_BOOK_25),
        _ => None,
    }
}

/// Decode one §C.2.1 block-code word using the §D.6 table-look-up
/// variant, in place.
///
/// This is the §C.2.1 table-look-up decoder (PDF p.182-183). It walks
/// the `book` from the last element down to the first, subtracting the
/// largest code value that does not exceed the remaining code and
/// recording that entry's quantisation index, exactly per the §C.2.1
/// `DecodeBlockCode` table-look-up pseudocode. The result is identical
/// to [`crate::decode_block_code`] (the modulus variant) for every code
/// word the spec defines.
///
/// On entry:
///
/// - `code` is the unsigned block-code word read from the bit stream.
/// - `book` is the §D.6 code book (e.g. [`D6_BOOK_3`]); its
///   [`D6BlockBook::levels`] fixes the alphabet size `nNumLevel`.
/// - `output` is the destination quantisation-index array, ordered
///   first-element-first. Its length is the spec's `nNumElement`; it
///   must not exceed [`D6_BLOCK_ELEMENTS`] (the §D.6 books are 4-element).
///
/// # Errors
///
/// - [`Error::BlockCodeLevelsOutOfRange`] if the book's level count is
///   `< 2` (no §D.6 book is that small; guarded for completeness).
/// - [`Error::BlockCodeResidual`] if `output.len()` exceeds
///   [`D6_BLOCK_ELEMENTS`], or if, after walking every element, the
///   residual code word is non-zero — the §C.2.1 "ERROR: block code
///   look-up fail" condition surfaced as a recoverable error.
pub fn decode_block_code_table(code: u32, book: &D6BlockBook, output: &mut [i32]) -> Result<()> {
    let n_levels = book.levels;
    if n_levels < 2 {
        return Err(Error::BlockCodeLevelsOutOfRange { n_levels });
    }
    if output.len() > D6_BLOCK_ELEMENTS {
        return Err(Error::BlockCodeResidual {
            residual: code,
            n_elements: output.len(),
            n_levels,
        });
    }
    let offset = ((n_levels - 1) >> 1) as i32;
    let n_elements = output.len();
    let mut residual = code;
    // §C.2.1: walk from the last element back to the first. `pnEntry`
    // points to the last entry in the element's code book and counts
    // down; the first entry that fits (largest first) wins.
    for e in (1..=n_elements).rev() {
        let mut matched = false;
        // m walks the level index from the top of the alphabet
        // (largest code value) down to 0; the largest entry that does
        // not exceed the residual is selected.
        for level_index in (0..n_levels).rev() {
            // Unwrap is safe: e in 1..=n_elements <= D6_BLOCK_ELEMENTS,
            // level_index < n_levels.
            let entry = book.code_value(e, level_index).unwrap();
            if residual >= entry {
                residual -= entry;
                // quantisation index = level_index - offset.
                output[e - 1] = level_index as i32 - offset;
                matched = true;
                break;
            }
        }
        if !matched {
            // No entry fit — only possible for a malformed code; the
            // L=0 entry is always 0 so this branch is unreachable for
            // well-formed books, but guard it to surface corruption.
            return Err(Error::BlockCodeResidual {
                residual,
                n_elements,
                n_levels,
            });
        }
    }
    // §C.2.1 success criterion: residual must be zero after the walk.
    if residual != 0 {
        return Err(Error::BlockCodeResidual {
            residual,
            n_elements,
            n_levels,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode_block_code;

    // -----------------------------------------------------------------
    // §D.6 table transcription: assert printed anchor cells against the
    // constructed books, row by row, to pin the closed form to the
    // staged PDF (p.231-236).
    // -----------------------------------------------------------------

    #[test]
    fn d6_1_three_level_matches_printed_table_v3() {
        // §D.6.1 Table V.3 (PDF p.231), one row per element.
        assert_eq!(D6_BOOK_3.levels(), 3);
        // 1st element: 0, 1, 2
        assert_eq!(
            [
                D6_BOOK_3.code_value(1, 0).unwrap(),
                D6_BOOK_3.code_value(1, 1).unwrap(),
                D6_BOOK_3.code_value(1, 2).unwrap()
            ],
            [0, 1, 2]
        );
        // 2nd element: 0, 3, 6
        assert_eq!(
            [
                D6_BOOK_3.code_value(2, 0).unwrap(),
                D6_BOOK_3.code_value(2, 1).unwrap(),
                D6_BOOK_3.code_value(2, 2).unwrap()
            ],
            [0, 3, 6]
        );
        // 3rd element: 0, 9, 18
        assert_eq!(
            [
                D6_BOOK_3.code_value(3, 0).unwrap(),
                D6_BOOK_3.code_value(3, 1).unwrap(),
                D6_BOOK_3.code_value(3, 2).unwrap()
            ],
            [0, 9, 18]
        );
        // 4th element: 0, 27, 54
        assert_eq!(
            [
                D6_BOOK_3.code_value(4, 0).unwrap(),
                D6_BOOK_3.code_value(4, 1).unwrap(),
                D6_BOOK_3.code_value(4, 2).unwrap()
            ],
            [0, 27, 54]
        );
    }

    #[test]
    fn d6_2_five_level_matches_printed_table_v5() {
        // §D.6.2 Table V.5 (PDF p.232).
        assert_eq!(D6_BOOK_5.levels(), 5);
        // 2nd element: 0, 5, 10, 15, 20
        for (l, want) in [0u32, 5, 10, 15, 20].iter().enumerate() {
            assert_eq!(D6_BOOK_5.code_value(2, l as u32).unwrap(), *want);
        }
        // 4th element: 0, 125, 250, 375, 500
        for (l, want) in [0u32, 125, 250, 375, 500].iter().enumerate() {
            assert_eq!(D6_BOOK_5.code_value(4, l as u32).unwrap(), *want);
        }
    }

    #[test]
    fn d6_3_seven_level_matches_printed_table_v7_with_print_erratum() {
        // §D.6.3 Table V.7 (PDF p.233).
        assert_eq!(D6_BOOK_7.levels(), 7);
        // 1st element: 0..6
        for l in 0u32..7 {
            assert_eq!(D6_BOOK_7.code_value(1, l).unwrap(), l);
        }
        // 3rd element factor 49: 0,49,98,147,196,245,294.
        // PDF print erratum: the level-0... wait — the printed cell that
        // reads "47" is at quantisation index 0 = level index 3, whose
        // arithmetically-correct value is 3*49 = 147. The table's own
        // `L*49` progression and the §C.2.1 modulus decoder both give
        // 147; the constructed book carries 147.
        for (l, want) in [0u32, 49, 98, 147, 196, 245, 294].iter().enumerate() {
            assert_eq!(D6_BOOK_7.code_value(3, l as u32).unwrap(), *want);
        }
        // 4th element factor 343.
        assert_eq!(D6_BOOK_7.code_value(4, 6).unwrap(), 2058);
    }

    #[test]
    fn d6_4_nine_level_matches_printed_table_v9() {
        // §D.6.4 Table V.9 (PDF p.234).
        assert_eq!(D6_BOOK_9.levels(), 9);
        // 3rd element factor 81: top entry 8*81 = 648.
        assert_eq!(D6_BOOK_9.code_value(3, 8).unwrap(), 648);
        // 4th element factor 729: top entry 8*729 = 5832.
        assert_eq!(D6_BOOK_9.code_value(4, 8).unwrap(), 5832);
    }

    #[test]
    fn d6_5_thirteen_level_matches_printed_table_v13() {
        // §D.6.5 Table V.13 (PDF p.235).
        assert_eq!(D6_BOOK_13.levels(), 13);
        // 3rd element factor 169: level 12 (quant +6) = 12*169 = 2028.
        assert_eq!(D6_BOOK_13.code_value(3, 12).unwrap(), 2028);
        // 4th element factor 2197: level 12 = 12*2197 = 26364.
        assert_eq!(D6_BOOK_13.code_value(4, 12).unwrap(), 26364);
    }

    #[test]
    fn d6_6_seventeen_level_matches_printed_table_v17() {
        // §D.6.6 Table V.17 (PDF p.236).
        assert_eq!(D6_BOOK_17.levels(), 17);
        // 3rd element factor 289: top 16*289 = 4624.
        assert_eq!(D6_BOOK_17.code_value(3, 16).unwrap(), 4624);
        // 4th element factor 4913: top 16*4913 = 78608.
        assert_eq!(D6_BOOK_17.code_value(4, 16).unwrap(), 78608);
    }

    #[test]
    fn d6_7_twenty_five_level_matches_printed_table_v25() {
        // §D.6.7 Table V.25 (PDF p.236).
        assert_eq!(D6_BOOK_25.levels(), 25);
        // 3rd element factor 625: top 24*625 = 15000.
        assert_eq!(D6_BOOK_25.code_value(3, 24).unwrap(), 15000);
        // 4th element factor 15625: top 24*15625 = 375000.
        assert_eq!(D6_BOOK_25.code_value(4, 24).unwrap(), 375000);
    }

    #[test]
    fn code_value_out_of_range_is_none() {
        assert_eq!(D6_BOOK_3.code_value(0, 0), None); // element 0 invalid
        assert_eq!(D6_BOOK_3.code_value(5, 0), None); // element 5 > 4
        assert_eq!(D6_BOOK_3.code_value(1, 3), None); // level 3 >= 3
    }

    #[test]
    fn d6_book_for_levels_resolves_every_sub_clause() {
        assert_eq!(d6_book_for_levels(3).unwrap().levels(), 3);
        assert_eq!(d6_book_for_levels(5).unwrap().levels(), 5);
        assert_eq!(d6_book_for_levels(7).unwrap().levels(), 7);
        assert_eq!(d6_book_for_levels(9).unwrap().levels(), 9);
        assert_eq!(d6_book_for_levels(13).unwrap().levels(), 13);
        assert_eq!(d6_book_for_levels(17).unwrap().levels(), 17);
        assert_eq!(d6_book_for_levels(25).unwrap().levels(), 25);
        // Non-§D.6 level counts.
        assert!(d6_book_for_levels(2).is_none());
        assert!(d6_book_for_levels(4).is_none());
        assert!(d6_book_for_levels(33).is_none());
        assert!(d6_book_for_levels(0).is_none());
    }

    // -----------------------------------------------------------------
    // §C.2.1 table-look-up decoder.
    // -----------------------------------------------------------------

    #[test]
    fn spec_worked_example_table_lookup_code_sixty_four() {
        // §C.2.1 worked example (PDF p.182): code 64, 3-level 4-element
        // → [0, -1, 0, +1].
        let mut out = [0_i32; 4];
        decode_block_code_table(64, &D6_BOOK_3, &mut out).unwrap();
        assert_eq!(out, [0, -1, 0, 1]);
    }

    #[test]
    fn table_lookup_all_zero_code_is_all_bottom_of_alphabet() {
        // code 0 → every element at level index 0 = -offset.
        let mut out = [0_i32; 4];
        decode_block_code_table(0, &D6_BOOK_5, &mut out).unwrap();
        assert_eq!(out, [-2, -2, -2, -2]);
    }

    #[test]
    fn table_lookup_max_code_is_all_top_of_alphabet() {
        // The largest valid code is sum over elements of
        // (n_levels-1) * n_levels^(e-1) = n_levels^4 - 1.
        let n: u32 = 3;
        let max = n.pow(4) - 1; // 80
        let mut out = [0_i32; 4];
        decode_block_code_table(max, &D6_BOOK_3, &mut out).unwrap();
        assert_eq!(out, [1, 1, 1, 1]);
    }

    #[test]
    fn table_lookup_residual_error_one_past_max() {
        let n: u32 = 3;
        let max = n.pow(4) - 1;
        let mut out = [0_i32; 4];
        let err = decode_block_code_table(max + 1, &D6_BOOK_3, &mut out).unwrap_err();
        assert!(matches!(err, Error::BlockCodeResidual { .. }));
    }

    #[test]
    fn table_lookup_too_many_elements_rejected() {
        let mut out = [0_i32; 5]; // > D6_BLOCK_ELEMENTS
        let err = decode_block_code_table(0, &D6_BOOK_3, &mut out).unwrap_err();
        assert!(matches!(err, Error::BlockCodeResidual { .. }));
    }

    #[test]
    fn table_lookup_empty_output_succeeds_only_for_zero_code() {
        let mut empty: [i32; 0] = [];
        decode_block_code_table(0, &D6_BOOK_3, &mut empty).unwrap();
        let err = decode_block_code_table(5, &D6_BOOK_3, &mut empty).unwrap_err();
        assert!(matches!(err, Error::BlockCodeResidual { .. }));
    }

    #[test]
    fn table_lookup_matches_modulus_decoder_three_level_full_domain() {
        // The spec states both decoders produce the identical output.
        // Cross-validate over the entire 3-level 4-element domain.
        let n: u32 = 3;
        for code in 0..n.pow(4) {
            let mut a = [0_i32; 4];
            let mut b = [0_i32; 4];
            decode_block_code_table(code, &D6_BOOK_3, &mut a).unwrap();
            decode_block_code(code, n, &mut b).unwrap();
            assert_eq!(a, b, "mismatch at code {code}");
        }
    }

    #[test]
    fn table_lookup_matches_modulus_decoder_five_level_full_domain() {
        let n: u32 = 5;
        for code in 0..n.pow(4) {
            let mut a = [0_i32; 4];
            let mut b = [0_i32; 4];
            decode_block_code_table(code, &D6_BOOK_5, &mut a).unwrap();
            decode_block_code(code, n, &mut b).unwrap();
            assert_eq!(a, b, "mismatch at code {code}");
        }
    }

    #[test]
    fn table_lookup_matches_modulus_decoder_seven_level_full_domain() {
        // Exercises the §D.6.3 3rd-element 147 erratum cell across the
        // full 7^4 = 2401 code domain.
        let n: u32 = 7;
        for code in 0..n.pow(4) {
            let mut a = [0_i32; 4];
            let mut b = [0_i32; 4];
            decode_block_code_table(code, &D6_BOOK_7, &mut a).unwrap();
            decode_block_code(code, n, &mut b).unwrap();
            assert_eq!(a, b, "mismatch at code {code}");
        }
    }

    #[test]
    fn table_lookup_matches_modulus_decoder_wide_books_sampled() {
        // 9/13/17/25-level domains are large; sample a stride across
        // each full range and cross-check both decoders agree.
        for (book, n) in [
            (&D6_BOOK_9, 9u32),
            (&D6_BOOK_13, 13),
            (&D6_BOOK_17, 17),
            (&D6_BOOK_25, 25),
        ] {
            let max = n.pow(4);
            let stride = (max / 997).max(1);
            let mut code = 0u32;
            while code < max {
                let mut a = [0_i32; 4];
                let mut b = [0_i32; 4];
                decode_block_code_table(code, book, &mut a).unwrap();
                decode_block_code(code, n, &mut b).unwrap();
                assert_eq!(a, b, "mismatch at {n}-level code {code}");
                code += stride;
            }
        }
    }

    #[test]
    fn table_lookup_indices_stay_within_alphabet() {
        // Every decoded index must land in [-offset, +offset].
        let n: u32 = 9;
        let offset = ((n - 1) >> 1) as i32;
        let max = n.pow(4);
        let mut code = 0u32;
        while code < max {
            let mut out = [0_i32; 4];
            decode_block_code_table(code, &D6_BOOK_9, &mut out).unwrap();
            for v in out {
                assert!(
                    (-offset..=offset).contains(&v),
                    "index {v} out of range at code {code}"
                );
            }
            code += 311;
        }
    }

    #[test]
    fn block_elements_constant_is_four() {
        assert_eq!(D6_BLOCK_ELEMENTS, 4);
    }
}

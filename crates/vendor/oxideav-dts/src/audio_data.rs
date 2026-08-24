//! DTS Coherent Acoustics — §5.5 Table 5-29 `Audio Data` quantization-
//! type dispatch and the Table 5-26 `(ABITS, SEL)` codebook-group
//! geometry it dispatches on.
//!
//! Source: ETSI TS 102 114 V1.3.1 (2011-08), staged PDF at
//! `docs/audio/dts/etsi-ts-102114-dts-coherent-acoustics.pdf`.
//!
//! Two clauses combine here:
//!
//! * **Table 5-26 "Selection of Quantization Levels and Codebooks"**
//!   (staged PDF p.27) tabulates, per `ABITS` bit-allocation index,
//!   the number of mid-tread quantizer levels and the list of
//!   quantization-index code books selectable by the `SEL` field. The
//!   per-`ABITS` code-book group has a fixed size (`nNumQ` in the §5.5
//!   pseudocode); the *last* entry of each group is special — it is a
//!   block-code book (the `V…` column) for `ABITS ≤ 7`, or a
//!   "no further encoding" (`NFE`) entry for `ABITS ≥ 8`.
//!
//! * **§5.5 Table 5-29 `Audio Data`** (staged PDF p.31-32) resolves
//!   the per-subband quantization *type* `nQType` from the
//!   `(ABITS, SEL)` pair before it extracts the eight `AUDIO[m]`
//!   indices. Transcribed verbatim from the staged pseudocode:
//!
//!   ```text
//!   nABITS = ABITS[ch][n];
//!   pCQGroup = &pCQGroupAUDIO[nABITS-1];
//!   nNumQ = pCQGroupAUDIO[nABITS-1].nNumQ-1; // top SEL index of group
//!   nSEL = SEL[ch][nABITS-1];
//!   nQType = 1;                  // Assume Huffman type by default
//!   if ( nSEL==nNumQ ) {         // Not Huffman type (last group entry)
//!     if ( nABITS<=7 ) nQType = 3;   // Block code
//!     else             nQType = 2;   // No further encoding
//!   }
//!   if ( nABITS==0 ) nQType = 0;     // No bits allocated
//!   ```
//!
//! This module exposes the Table 5-26 geometry verbatim (levels +
//! group size per `ABITS`) and the `nQType` resolver
//! ([`AudioQuantType`] / [`audio_quant_type`]) that the §5.5
//! per-subsubframe `Audio Data` walker dispatches on. It is the
//! decision core that routes each subband into one of the four
//! already-landed extraction paths:
//!
//! * [`AudioQuantType::NoBits`] → eight zero `AUDIO[m]` values;
//! * [`AudioQuantType::Huffman`] → eight Huffman-coded indices (the
//!   §D.6 audio code books, a separate transcription);
//! * [`AudioQuantType::NoEncoding`] → eight plain sign-extended
//!   quantization indices read directly from the bit stream;
//! * [`AudioQuantType::BlockCode`] → two block-code words, each
//!   expanded to four samples by the round-232
//!   [`crate::decode_block_code`].
//!
//! # Scope and follow-ups
//!
//! The actual §D.6 audio quantization code books (`A3`, `B12`, …) and
//! the `SEL[ch][ABITS]` field decode (Table 5-21 header) are *not*
//! part of this module — they are larger separate transcriptions. The
//! `nQType` dispatch is fully fixed by the spec text and Table 5-26
//! alone, so it lands ahead of the code-book tables it will route
//! into.

/// The number of distinct `ABITS` bit-allocation indices Table 5-26
/// tabulates a quantizer for: `ABITS = 0..=11` (PDF p.27). Index `0`
/// is "no bits allocated" (no quantizer); `1..=11` carry a mid-tread
/// quantizer. `ABITS > 11` is not a Table 5-26 row (no SEL is
/// transmitted and no further encoding is applied — see
/// [`audio_quant_type`]).
pub const ABITS_TABLE_LEN: usize = 12;

/// The largest `ABITS` index that selects a code-book group with a
/// transmitted `SEL` field (Table 5-26, PDF p.27): "No SEL is
/// transmitted for `ABITS[ch] > 11`, because no further encoding is
/// used for those quantizers." `ABITS` in `1..=ABITS_MAX_SEL` carries
/// a `SEL` field; `ABITS == 0` is "Not transmitted".
pub const ABITS_MAX_SEL: u8 = 11;

/// The largest `ABITS` whose group's terminal code book is a block
/// code (the `V…` column of Table 5-26): for `ABITS ≤ 7` the last
/// group entry is a block-code book, for `ABITS ≥ 8` it is a "no
/// further encoding" (`NFE`) entry. The §5.5 `nQType` resolver tests
/// `if (nABITS <= 7)`.
pub const ABITS_MAX_BLOCK_CODE: u8 = 7;

/// Per-`ABITS` "Number of Index Quantization Levels" column of
/// Table 5-26 (PDF p.27), indexed by `ABITS = 0..=11`. The mid-tread
/// linear quantizer for `ABITS` has this many output levels (the two
/// values written "33 or 32" / "65 or 64" / "129 or 128" in the PDF —
/// the symmetric-with-zero vs. symmetric-without-zero variants — are
/// tabulated here at their odd "with zero" form; the alternate even
/// form is noted in the spec for the NFE code-book variant). `ABITS 0`
/// has zero levels ("no bits allocated").
pub const QUANT_LEVELS: [u16; ABITS_TABLE_LEN] = [
    0,   // ABITS 0 — no bits allocated
    3,   // ABITS 1
    5,   // ABITS 2
    7,   // ABITS 3
    9,   // ABITS 4
    13,  // ABITS 5
    17,  // ABITS 6
    25,  // ABITS 7
    33,  // ABITS 8 (33 or 32)
    65,  // ABITS 9 (65 or 64)
    129, // ABITS 10 (129 or 128)
    256, // ABITS 11
];

/// Per-`ABITS` code-book group size `nNumQ` of Table 5-26 (PDF p.27),
/// indexed by `ABITS = 0..=11`: the number of code books listed in the
/// `SEL` columns of that row. The §5.5 pseudocode reads this struct
/// field and subtracts one (`nNumQ - 1`) to obtain the *top* valid
/// `SEL` index of the group; `nSEL == nNumQ - 1` selects the group's
/// terminal (block-code or NFE) entry. `ABITS 0` lists no code books
/// ("Not transmitted").
///
/// Group sizes read straight off Table 5-26's `SEL` columns:
///
/// | ABITS | code books listed                        | nNumQ |
/// |-------|------------------------------------------|-------|
/// | 1     | `A3 V3`                                  | 2     |
/// | 2     | `A5 B5 C5 V5`                            | 4     |
/// | 3     | `A7 B7 C7 V7`                            | 4     |
/// | 4     | `A9 B9 C9 V9`                            | 4     |
/// | 5     | `A13 B13 C13 V13`                        | 4     |
/// | 6     | `A17 B17 C17 D17 E17 F17 G17 V17`        | 8     |
/// | 7     | `A25 B25 C25 D25 E25 F25 G25 V25`        | 8     |
/// | 8     | `A33 B33 C33 D33 E33 F33 G33 NFE`        | 8     |
/// | 9     | `A65 B65 C65 D65 E65 F65 G65 NFE`        | 8     |
/// | 10    | `A129 B129 C129 D129 E129 F129 G129 NFE` | 8     |
/// | 11    | `NFE`                                    | 1     |
pub const CODEBOOK_GROUP_SIZE: [u8; ABITS_TABLE_LEN] = [
    0, // ABITS 0 — no code books transmitted
    2, // ABITS 1
    4, // ABITS 2
    4, // ABITS 3
    4, // ABITS 4
    4, // ABITS 5
    8, // ABITS 6
    8, // ABITS 7
    8, // ABITS 8
    8, // ABITS 9
    8, // ABITS 10
    1, // ABITS 11
];

/// The §5.5 quantization *type* `nQType` of the Table 5-29 `Audio
/// Data` block: how the eight `AUDIO[m]` quantization indices of one
/// `(ch, n, subsubframe)` subband are represented in the bit stream
/// (PDF p.31-32). Each variant routes into a distinct extraction path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioQuantType {
    /// `nQType == 0` — "No bits allocated" (`ABITS == 0`): the eight
    /// `AUDIO[m]` values are all zero; no bits are read.
    NoBits,
    /// `nQType == 1` — "Huffman code" (the default for every
    /// `(ABITS, SEL)` pair whose `SEL` is *not* the group's terminal
    /// entry): eight Huffman-coded indices follow, decoded through the
    /// `SEL`-selected §D.6 audio code book.
    Huffman,
    /// `nQType == 2` — "No further encoding" (`ABITS >= 8` with `SEL`
    /// at the group's terminal `NFE` entry): eight plain quantization
    /// indices follow, each read as a fixed-width field and
    /// sign-extended.
    NoEncoding,
    /// `nQType == 3` — "Block code" (`ABITS <= 7` with `SEL` at the
    /// group's terminal `V…` entry): two block-code words follow, each
    /// expanded to four samples by the §C.2.1 block-code book
    /// ([`crate::decode_block_code`]).
    BlockCode,
}

/// The top valid `SEL` index for an `ABITS` code-book group: the §5.5
/// pseudocode's `nNumQ - 1` (PDF p.31). Returns `None` for
/// `ABITS == 0` (no code books transmitted) and for `ABITS` outside
/// the Table 5-26 range (`ABITS > 11`), which carry no `SEL` field.
///
/// `nSEL == terminal_sel_index(ABITS)` is the §5.5 "Not Huffman type"
/// condition that selects the group's block-code (`V…`) or
/// no-further-encoding (`NFE`) entry.
#[must_use]
pub fn terminal_sel_index(abits: u8) -> Option<u8> {
    let idx = abits as usize;
    if idx == 0 || idx >= ABITS_TABLE_LEN {
        return None;
    }
    // CODEBOOK_GROUP_SIZE[abits] is >= 1 for every 1..=11 row, so the
    // subtraction never underflows.
    Some(CODEBOOK_GROUP_SIZE[idx] - 1)
}

/// Resolve the §5.5 Table 5-29 quantization type `nQType` for one
/// subband from its `(ABITS, SEL)` pair, exactly per the staged
/// pseudocode (PDF p.31-32):
///
/// ```text
/// nQType = 1;                  // Assume Huffman type by default
/// if ( nSEL == nNumQ-1 ) {     // Not Huffman type (last group entry)
///   if ( nABITS <= 7 ) nQType = 3;   // Block code
///   else               nQType = 2;   // No further encoding
/// }
/// if ( nABITS == 0 ) nQType = 0;     // No bits allocated
/// ```
///
/// * `abits` is `ABITS[ch][n]`; `0` is "no bits allocated".
/// * `sel` is `SEL[ch][ABITS-1]`, the code-book selector for this
///   quantizer (only meaningful for `ABITS >= 1`).
///
/// `ABITS == 0` resolves to [`AudioQuantType::NoBits`] regardless of
/// `sel` (the `nABITS == 0` test runs last and overrides). For
/// `ABITS > 11` (no Table 5-26 row, no `SEL` transmitted) the spec's
/// "no further encoding is used for those quantizers" sentence
/// (PDF p.27) makes the type [`AudioQuantType::NoEncoding`]: the eight
/// indices are read as plain sign-extended fields.
#[must_use]
pub fn audio_quant_type(abits: u8, sel: u8) -> AudioQuantType {
    if abits == 0 {
        return AudioQuantType::NoBits;
    }
    if abits > ABITS_MAX_SEL {
        // No SEL transmitted, no further encoding (PDF p.27).
        return AudioQuantType::NoEncoding;
    }
    // Table 5-26 row: the terminal SEL entry is block (ABITS<=7) or
    // NFE (ABITS>=8); every earlier SEL is a Huffman code book.
    match terminal_sel_index(abits) {
        Some(top) if sel == top => {
            if abits <= ABITS_MAX_BLOCK_CODE {
                AudioQuantType::BlockCode
            } else {
                AudioQuantType::NoEncoding
            }
        }
        _ => AudioQuantType::Huffman,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quant_levels_match_table_5_26() {
        // PDF p.27 "Number of Index Quantization Levels" column.
        assert_eq!(QUANT_LEVELS[0], 0);
        assert_eq!(QUANT_LEVELS[1], 3);
        assert_eq!(QUANT_LEVELS[2], 5);
        assert_eq!(QUANT_LEVELS[3], 7);
        assert_eq!(QUANT_LEVELS[4], 9);
        assert_eq!(QUANT_LEVELS[5], 13);
        assert_eq!(QUANT_LEVELS[6], 17);
        assert_eq!(QUANT_LEVELS[7], 25);
        assert_eq!(QUANT_LEVELS[8], 33);
        assert_eq!(QUANT_LEVELS[9], 65);
        assert_eq!(QUANT_LEVELS[10], 129);
        assert_eq!(QUANT_LEVELS[11], 256);
    }

    #[test]
    fn group_sizes_match_table_5_26() {
        // SEL-column counts off Table 5-26 (PDF p.27).
        assert_eq!(CODEBOOK_GROUP_SIZE[0], 0);
        assert_eq!(CODEBOOK_GROUP_SIZE[1], 2); // A3 V3
        assert_eq!(CODEBOOK_GROUP_SIZE[2], 4); // A5 B5 C5 V5
        assert_eq!(CODEBOOK_GROUP_SIZE[3], 4);
        assert_eq!(CODEBOOK_GROUP_SIZE[4], 4);
        assert_eq!(CODEBOOK_GROUP_SIZE[5], 4);
        assert_eq!(CODEBOOK_GROUP_SIZE[6], 8); // A17..G17 V17
        assert_eq!(CODEBOOK_GROUP_SIZE[7], 8);
        assert_eq!(CODEBOOK_GROUP_SIZE[8], 8); // A33..G33 NFE
        assert_eq!(CODEBOOK_GROUP_SIZE[9], 8);
        assert_eq!(CODEBOOK_GROUP_SIZE[10], 8);
        assert_eq!(CODEBOOK_GROUP_SIZE[11], 1); // NFE
    }

    #[test]
    fn tables_have_twelve_rows() {
        assert_eq!(QUANT_LEVELS.len(), ABITS_TABLE_LEN);
        assert_eq!(CODEBOOK_GROUP_SIZE.len(), ABITS_TABLE_LEN);
    }

    #[test]
    fn terminal_sel_index_is_group_size_minus_one() {
        assert_eq!(terminal_sel_index(0), None); // not transmitted
        assert_eq!(terminal_sel_index(1), Some(1)); // group of 2 -> top 1
        assert_eq!(terminal_sel_index(2), Some(3)); // group of 4 -> top 3
        assert_eq!(terminal_sel_index(6), Some(7)); // group of 8 -> top 7
        assert_eq!(terminal_sel_index(11), Some(0)); // group of 1 -> top 0
        assert_eq!(terminal_sel_index(12), None); // no Table 5-26 row
        assert_eq!(terminal_sel_index(40), None);
    }

    #[test]
    fn abits_zero_is_no_bits_regardless_of_sel() {
        for sel in 0u8..=7 {
            assert_eq!(audio_quant_type(0, sel), AudioQuantType::NoBits);
        }
    }

    #[test]
    fn non_terminal_sel_is_huffman() {
        // ABITS 6 group of 8 (top SEL 7): SEL 0..6 are Huffman code
        // books A17..G17.
        for sel in 0u8..=6 {
            assert_eq!(audio_quant_type(6, sel), AudioQuantType::Huffman);
        }
        // ABITS 2 group of 4 (top SEL 3): SEL 0..2 are A5/B5/C5.
        for sel in 0u8..=2 {
            assert_eq!(audio_quant_type(2, sel), AudioQuantType::Huffman);
        }
    }

    #[test]
    fn terminal_sel_block_code_for_low_abits() {
        // ABITS 1..=7 terminal entry is the V… block-code book.
        for abits in 1u8..=ABITS_MAX_BLOCK_CODE {
            let top = terminal_sel_index(abits).unwrap();
            assert_eq!(
                audio_quant_type(abits, top),
                AudioQuantType::BlockCode,
                "ABITS {abits} terminal SEL {top}"
            );
        }
    }

    #[test]
    fn terminal_sel_no_encoding_for_high_abits() {
        // ABITS 8..=11 terminal entry is the NFE (no-further-encoding)
        // slot.
        for abits in 8u8..=ABITS_MAX_SEL {
            let top = terminal_sel_index(abits).unwrap();
            assert_eq!(
                audio_quant_type(abits, top),
                AudioQuantType::NoEncoding,
                "ABITS {abits} terminal SEL {top}"
            );
        }
    }

    #[test]
    fn abits_eleven_only_entry_is_nfe() {
        // ABITS 11 group has a single NFE entry at SEL 0.
        assert_eq!(audio_quant_type(11, 0), AudioQuantType::NoEncoding);
    }

    #[test]
    fn abits_above_table_is_no_encoding() {
        // PDF p.27: "No SEL is transmitted for ABITS[ch]>11, because
        // no further encoding is used for those quantizers."
        for abits in 12u8..=31 {
            for sel in 0u8..=7 {
                assert_eq!(audio_quant_type(abits, sel), AudioQuantType::NoEncoding);
            }
        }
    }

    #[test]
    fn full_abits_sel_dispatch_matrix() {
        // Exhaustively cross-check audio_quant_type against the §5.5
        // pseudocode for every Table 5-26 (ABITS, SEL) pair.
        for abits in 0u8..=ABITS_MAX_SEL {
            let group = CODEBOOK_GROUP_SIZE[abits as usize];
            // ABITS 0 has no SEL field; the loop below is empty for it
            // and the explicit NoBits check covers it.
            if abits == 0 {
                assert_eq!(audio_quant_type(0, 0), AudioQuantType::NoBits);
                continue;
            }
            let top = group - 1;
            for sel in 0..group {
                let want = if sel == top {
                    if abits <= ABITS_MAX_BLOCK_CODE {
                        AudioQuantType::BlockCode
                    } else {
                        AudioQuantType::NoEncoding
                    }
                } else {
                    AudioQuantType::Huffman
                };
                assert_eq!(
                    audio_quant_type(abits, sel),
                    want,
                    "ABITS {abits} SEL {sel}"
                );
            }
        }
    }

    #[test]
    fn block_code_two_words_of_four_equals_eight_samples() {
        // The §5.5 block-code path expands two code words to four
        // samples each, matching SAMPLES_PER_SUBSUBFRAME = 8.
        assert_eq!(crate::SAMPLES_PER_SUBSUBFRAME, 8);
    }

    #[test]
    fn constants_match_spec_bounds() {
        assert_eq!(ABITS_MAX_SEL, 11);
        assert_eq!(ABITS_MAX_BLOCK_CODE, 7);
        assert_eq!(ABITS_TABLE_LEN, 12);
    }
}

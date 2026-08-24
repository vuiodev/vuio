//! DTS Coherent Acoustics — Core Primary Audio Coding Side Information
//! (§5.4.1) ABITS / SCALES (a.k.a. ALLOC / SCFAC) bit-stream decoders.
//!
//! Round 195 (2026-05-31) lands the side-information half of the core
//! subframe decode path: extracting the per-channel × per-subband
//! ABITS bit-allocation index field and the per-channel × per-subband
//! SCALES scale-factor field from a packed bit stream, given the
//! channel-wide BHUFF (`nQSelect` for ABITS) and SHUFF
//! (`nQSelect` for SCALES) codebook-selector values read earlier
//! from the AUDIO CODING HEADER (clause §5.3.x).
//!
//! Everything in this module is transcribed verbatim from the locally
//! staged ETSI specification
//! `docs/audio/dts/etsi-ts-102114-dts-coherent-acoustics.pdf` =
//! **ETSI TS 102 114 V1.3.1 (2011-08)**, *DTS Coherent Acoustics; Core
//! and Extensions with Additional Profiles*. The relevant clauses and
//! tables are:
//!
//! - §5.4.1 Table 5-28 ("Core side information") — the side-info
//!   pseudocode listing the BHUFF / THUFF / SHUFF dispatch.
//! - §5.3.x Table 5-23 (THUFF → A4/B4/C4/D4), Table 5-24 (SHUFF →
//!   {SA129..SE129, 6-bit linear, 7-bit linear, invalid}), Table 5-25
//!   (BHUFF → {A12..E12, linear-4-bit, linear-5-bit, invalid}),
//!   Table 5-26 (SEL × ABITS → audio-data codebook), Table 5-27
//!   (ADJ → scale-factor adjustment value).
//! - Annex D §D.1.1 (6-bit RMS square-root table, 64 entries) and
//!   §D.1.2 (7-bit RMS square-root table, 128 entries) — the
//!   `pScaleTable` lookups in Table 5-28.
//! - Annex D §D.5.6 (12 Levels for BHUFF: tables A12, B12, C12, D12,
//!   E12).
//! - Annex D §D.5.3 (5 Levels: tables A5, B5, C5) and §D.5.4 (7 Levels:
//!   tables A7, B7, C7) — the SHUFF Huffman codebooks for the
//!   {SA129, SB129, SC129, SD129, SE129} selectors. The spec routes
//!   the SHUFF=0..4 codes through the 5-level tables for SA129/SB129/
//!   SC129 and the 7-level tables for SD129/SE129; §5.4.1's
//!   `QSCALES.ppQ[nQSelect]->InverseQ(...)` is the dispatch table.
//!
//! The module is feature-independent (no `oxideav-core` dep), so it
//! is available under both the default and `--no-default-features`
//! build modes.
//!
//! # Scope
//!
//! This round only lands the **single-field** decode primitives plus
//! their backing tables; wiring them into a complete subframe walker
//! (which also requires the AUDIO CODING HEADER fields SUBFS, PCHS,
//! SUBS, VQSUB, JOINX, BHUFF/THUFF/SHUFF, plus the side-info loop
//! over `nPCHS × nSUBS[ch]`) is a separate follow-up. The decoders
//! exposed here take the caller-supplied `nQSelect`, codebook
//! selector, and bit-stream cursor; everything they read from the
//! bit stream is per the §5.4.1 pseudocode.

use crate::bitreader::BitReader;
use crate::{Error, Result};

// ---------------------------------------------------------------
// Table 5-25 — Codebooks for Encoding Bit Allocation Index ABITS
// (BHUFF[ch] selector, §5.3.x).
// ---------------------------------------------------------------
//
// | BHUFF[ch] | Codebook (clause D.5.6) |
// | --------- | ----------------------- |
// |     0     | A12                     |
// |     1     | B12                     |
// |     2     | C12                     |
// |     3     | D12                     |
// |     4     | E12                     |
// |     5     | Linear 4-bit            |
// |     6     | Linear 5-bit            |
// |     7     | Invalid                 |

/// Codebook selector for the bit-allocation-index (ABITS) field, per
/// §5.3.x Table 5-25. `BHUFF[ch] == 7` is reserved/invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbitsCodebook {
    /// `BHUFF=0` — Annex D §D.5.6 Table A12 (Huffman, 12 levels).
    A12,
    /// `BHUFF=1` — Annex D §D.5.6 Table B12 (Huffman, 12 levels).
    B12,
    /// `BHUFF=2` — Annex D §D.5.6 Table C12 (Huffman, 12 levels).
    C12,
    /// `BHUFF=3` — Annex D §D.5.6 Table D12 (Huffman, 12 levels).
    D12,
    /// `BHUFF=4` — Annex D §D.5.6 Table E12 (Huffman, 12 levels).
    E12,
    /// `BHUFF=5` — Linear 4-bit (raw 4-bit ABITS index, 0..=15).
    Linear4Bit,
    /// `BHUFF=6` — Linear 5-bit (raw 5-bit ABITS index, 0..=31).
    Linear5Bit,
}

impl AbitsCodebook {
    /// Resolve a raw 3-bit `BHUFF[ch]` field to a codebook variant per
    /// Table 5-25. `BHUFF == 7` is rejected as `Error::InvalidSideInfo`.
    pub fn from_bhuff(bhuff: u8) -> Result<Self> {
        match bhuff {
            0 => Ok(Self::A12),
            1 => Ok(Self::B12),
            2 => Ok(Self::C12),
            3 => Ok(Self::D12),
            4 => Ok(Self::E12),
            5 => Ok(Self::Linear4Bit),
            6 => Ok(Self::Linear5Bit),
            // `7` is the spec's documented "Invalid" entry.
            _ => Err(Error::InvalidSideInfo {
                field: "BHUFF",
                value: bhuff as u32,
            }),
        }
    }
}

// ---------------------------------------------------------------
// Annex D §D.5.6 — 12-level Huffman codebooks for BHUFF (ABITS).
// ---------------------------------------------------------------
//
// Each entry is `(quantization_level, code_length, code)`. The
// codeword is the low `code_length` bits of `code`, MSB-first in the
// bit-stream (matches the `BitReader::read_bits` convention used
// elsewhere in this crate). The codebooks below are transcribed
// verbatim from the staged PDF p.201-202.
//
// Per Table 5-25 the indexed level is `ABITS` itself (range 1..=12 in
// these codebooks; ABITS=0 is "no bits allocated" and is not
// transmitted via Huffman per the §5.4.1 pseudocode — it never
// appears in the ABITS table because the BHUFF dispatch is skipped
// when no bits would be allocated).

/// Entry in a small Huffman codebook: `(symbol, code_length, code)`.
/// The codeword is read MSB-first from the bit stream; matching
/// happens by progressively reading bits and comparing against this
/// table by `code_length`.
type HuffmanEntry = (i16, u8, u16);

/// Annex D §D.5.6 Table A12.
const TABLE_A12: &[HuffmanEntry] = &[
    (1, 1, 0),
    (2, 2, 2),
    (3, 3, 6),
    (4, 4, 14),
    (5, 5, 30),
    (6, 6, 62),
    (7, 8, 255),
    (8, 8, 254),
    (9, 9, 507),
    (10, 9, 506),
    (11, 9, 505),
    (12, 9, 504),
];

/// Annex D §D.5.6 Table B12.
const TABLE_B12: &[HuffmanEntry] = &[
    (1, 1, 1),
    (2, 2, 0),
    (3, 3, 2),
    (4, 5, 15),
    (5, 5, 12),
    (6, 6, 29),
    (7, 7, 57),
    (8, 7, 56),
    (9, 7, 55),
    (10, 7, 54),
    (11, 7, 53),
    (12, 7, 52),
];

/// Annex D §D.5.6 Table C12.
const TABLE_C12: &[HuffmanEntry] = &[
    (1, 2, 0),
    (2, 3, 7),
    (3, 3, 5),
    (4, 3, 4),
    (5, 3, 2),
    (6, 4, 13),
    (7, 4, 12),
    (8, 4, 6),
    (9, 5, 15),
    (10, 6, 29),
    (11, 7, 57),
    (12, 7, 56),
];

/// Annex D §D.5.6 Table D12.
const TABLE_D12: &[HuffmanEntry] = &[
    (1, 2, 3),
    (2, 2, 2),
    (3, 2, 0),
    (4, 3, 2),
    (5, 4, 6),
    (6, 5, 14),
    (7, 6, 30),
    (8, 7, 62),
    (9, 8, 126),
    (10, 9, 254),
    (11, 10, 511),
    (12, 10, 510),
];

/// Annex D §D.5.6 Table E12.
const TABLE_E12: &[HuffmanEntry] = &[
    (1, 1, 1),
    (2, 2, 0),
    (3, 3, 2),
    (4, 4, 6),
    (5, 5, 14),
    (6, 7, 63),
    (7, 7, 61),
    (8, 8, 124),
    (9, 8, 121),
    (10, 8, 120),
    (11, 9, 251),
    (12, 9, 250),
];

/// Maximum code length over every codebook in this module. The
/// decoder reads bits one at a time up to this bound; an unmatched
/// pattern after that many bits is a stream-format failure.
const MAX_HUFFMAN_CODE_LEN: u32 = 14;

/// Walk a Huffman codebook one bit at a time, MSB-first, returning
/// the matching `symbol` when a code of the prefix-matched length is
/// found. Returns `Error::HuffmanDecodeFailed` when no entry matches
/// within [`MAX_HUFFMAN_CODE_LEN`] bits.
fn decode_huffman(
    br: &mut BitReader<'_>,
    codebook: &[HuffmanEntry],
    table_name: &'static str,
) -> Result<i16> {
    let mut value: u32 = 0;
    let mut bits_read: u8 = 0;
    while bits_read < MAX_HUFFMAN_CODE_LEN as u8 {
        let bit = br.read_bits(1)?;
        value = (value << 1) | bit;
        bits_read += 1;
        // Try every entry whose code_length matches what we've read.
        for &(symbol, code_len, code) in codebook {
            if code_len == bits_read && value == code as u32 {
                return Ok(symbol);
            }
        }
    }
    Err(Error::HuffmanDecodeFailed { table: table_name })
}

/// Decode a single ABITS field from the bit stream given the channel-
/// wide `BHUFF[ch]` codebook selector. Implements the BHUFF dispatch
/// in §5.4.1 Table 5-28:
///
/// ```text
/// nQSelect = BHUFF[ch];
/// for (n=0; n<nVQSUB[ch]; n++) {   // Not for VQ encoded subbands.
///     QABITS.ppQ[nQSelect]->InverseQ(InputFrame, ABITS[ch][n]);
/// }
/// ```
///
/// Returns the per-subband ABITS index for one subband. The caller is
/// responsible for the `nVQSUB[ch]` loop (subbands ≥ `nVQSUB[ch]` are
/// VQ-encoded and have no ABITS).
/// Public entry point: decode an ABITS field starting at `bit_offset`
/// in `bytes`, returning `(decoded_abits, bits_consumed)`. The bit
/// offset is measured from the MSB of `bytes[0]`, matching the
/// MSB-first convention used elsewhere in this crate. Use this when
/// the caller is driving the side-info loop directly (e.g. from a
/// future subframe walker) and only needs the dispatch + Huffman
/// decode, not the full §5.4.1 SCALES / TMODE bookkeeping.
pub fn decode_abits_at(
    bytes: &[u8],
    bit_offset: usize,
    codebook: AbitsCodebook,
) -> Result<(u8, usize)> {
    // BitReader::from_byte_offset takes byte offsets; for the typical
    // case where the caller is positioned mid-byte we need an
    // arbitrary bit offset. Bit-shifting the buffer by `bit_offset`
    // would copy; instead we synthesise a leading byte alignment by
    // letting BitReader::from_byte_offset start at the byte that
    // contains `bit_offset` and then skipping the remaining
    // intra-byte bits via a no-op read.
    let byte_offset = bit_offset / 8;
    let intra_byte = bit_offset % 8;
    let mut br = BitReader::from_byte_offset(bytes, byte_offset);
    if intra_byte > 0 {
        br.read_bits(intra_byte as u32)?;
    }
    let start = bit_offset;
    let value = decode_abits(&mut br, codebook)?;
    let bits_consumed = br.absolute_bit_position() - start;
    Ok((value, bits_consumed))
}

pub(crate) fn decode_abits(br: &mut BitReader<'_>, codebook: AbitsCodebook) -> Result<u8> {
    match codebook {
        AbitsCodebook::A12 => decode_huffman(br, TABLE_A12, "A12").map(|s| s as u8),
        AbitsCodebook::B12 => decode_huffman(br, TABLE_B12, "B12").map(|s| s as u8),
        AbitsCodebook::C12 => decode_huffman(br, TABLE_C12, "C12").map(|s| s as u8),
        AbitsCodebook::D12 => decode_huffman(br, TABLE_D12, "D12").map(|s| s as u8),
        AbitsCodebook::E12 => decode_huffman(br, TABLE_E12, "E12").map(|s| s as u8),
        AbitsCodebook::Linear4Bit => br.read_bits(4).map(|v| v as u8),
        AbitsCodebook::Linear5Bit => br.read_bits(5).map(|v| v as u8),
    }
}

// ---------------------------------------------------------------
// Table 5-23 — Selection of Huffman Codebook for Encoding TMODE
// (THUFF[ch] selector, §5.3.2 / staged PDF p.26).
// ---------------------------------------------------------------
//
// | THUFF[ch] | Huffman Codebook |
// | --------- | ---------------- |
// |     0     | A4               |
// |     1     | B4               |
// |     2     | C4               |
// |     3     | D4               |
//
// THUFF[ch] is a 2-bit wire field — the §5.3.2 Table 5-21 Core audio
// coding header pseudocode (staged PDF p.24) reads
// `THUFF[ch] = ExtractBits(2);` ("2 bits per channel") — so all four
// wire values resolve to a documented codebook. Unlike BHUFF/SHUFF
// there is no reserved/invalid row, and the resolver below is total
// over the masked 2-bit input.

/// Codebook selector for the transient-mode (TMODE) field, per §5.3.2
/// Table 5-23 (staged PDF p.26). All four 2-bit `THUFF[ch]` values
/// are valid (no reserved row).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TmodeCodebook {
    /// `THUFF=0` — Annex D §D.5.2 Table A4 (Huffman, 4 levels).
    A4,
    /// `THUFF=1` — Annex D §D.5.2 Table B4 (Huffman, 4 levels).
    B4,
    /// `THUFF=2` — Annex D §D.5.2 Table C4 (Huffman, 4 levels).
    C4,
    /// `THUFF=3` — Annex D §D.5.2 Table D4 (Huffman, 4 levels —
    /// every code is 2 bits, so this variant is equivalent to a raw
    /// 2-bit read).
    D4,
}

impl TmodeCodebook {
    /// Resolve a raw 2-bit `THUFF[ch]` field to a codebook variant
    /// per Table 5-23. Only the low 2 bits of the input are
    /// consulted (matching the `ExtractBits(2)` wire width fixed by
    /// Table 5-21), so the mapping is total.
    pub fn from_thuff(thuff: u8) -> Self {
        match thuff & 0b11 {
            0 => Self::A4,
            1 => Self::B4,
            2 => Self::C4,
            _ => Self::D4,
        }
    }

    /// The canonical 2-bit `THUFF[ch]` wire value for this variant
    /// (the inverse of [`Self::from_thuff`]).
    pub fn thuff(self) -> u8 {
        match self {
            Self::A4 => 0,
            Self::B4 => 1,
            Self::C4 => 2,
            Self::D4 => 3,
        }
    }
}

// ---------------------------------------------------------------
// Annex D §D.5.2 — "4 Levels (For TMODE)" Huffman codebooks.
// Transcribed verbatim from the staged PDF p.198.
// ---------------------------------------------------------------

/// Annex D §D.5.2 Table A4.
const TABLE_A4: &[HuffmanEntry] = &[(0, 1, 0), (1, 2, 2), (2, 3, 6), (3, 3, 7)];

/// Annex D §D.5.2 Table B4.
const TABLE_B4: &[HuffmanEntry] = &[(0, 2, 2), (1, 3, 6), (2, 3, 7), (3, 1, 0)];

/// Annex D §D.5.2 Table C4.
const TABLE_C4: &[HuffmanEntry] = &[(0, 3, 6), (1, 3, 7), (2, 1, 0), (3, 2, 2)];

/// Annex D §D.5.2 Table D4. All four codes are 2 bits and equal
/// their quantization level, so this codebook degenerates to a raw
/// 2-bit field.
const TABLE_D4: &[HuffmanEntry] = &[(0, 2, 0), (1, 2, 1), (2, 2, 2), (3, 2, 3)];

/// Decode a single TMODE field from the bit stream given the
/// channel-wide `THUFF[ch]` codebook selector. Implements the THUFF
/// dispatch in §5.4.1 Table 5-28 (staged PDF p.28):
///
/// ```text
/// nQSelect = THUFF[ch];
/// for (n=0; n<nVQSUB[ch]; n++)   // No VQ encoded subbands
///   if ( ABITS[ch][n] > 0 )      // Present only if bits allocated
///     QTMODE.ppQ[nQSelect]->InverseQ(InputFrame, TMODE[ch][n]);
/// ```
///
/// Returns `(tmode, bits_consumed)` where `tmode` is in `0..=3`:
/// `0` means no transient in the subframe for this subband, and a
/// non-zero value means the transition occurred in subsubframe
/// `TMODE[ch][n] + 1` (PDF p.30 field description). The
/// `ABITS[ch][n] > 0` / `nSSC > 1` transmission conditions are the
/// caller's responsibility (see PDF p.30: TMODE is not transmitted
/// when only one subsubframe is present, for VQ-encoded high-
/// frequency subbands, or for subbands without bit allocation).
pub fn decode_tmode_at(
    bytes: &[u8],
    bit_offset: usize,
    codebook: TmodeCodebook,
) -> Result<(u8, usize)> {
    let byte_offset = bit_offset / 8;
    let intra_byte = bit_offset % 8;
    let mut br = BitReader::from_byte_offset(bytes, byte_offset);
    if intra_byte > 0 {
        br.read_bits(intra_byte as u32)?;
    }
    let start = bit_offset;
    let value = decode_tmode(&mut br, codebook)?;
    let bits_consumed = br.absolute_bit_position() - start;
    Ok((value, bits_consumed))
}

pub(crate) fn decode_tmode(br: &mut BitReader<'_>, codebook: TmodeCodebook) -> Result<u8> {
    let (table, name) = match codebook {
        TmodeCodebook::A4 => (TABLE_A4, "A4"),
        TmodeCodebook::B4 => (TABLE_B4, "B4"),
        TmodeCodebook::C4 => (TABLE_C4, "C4"),
        TmodeCodebook::D4 => (TABLE_D4, "D4"),
    };
    decode_huffman(br, table, name).map(|s| s as u8)
}

// ---------------------------------------------------------------
// Table 5-24 — Code Books and Square Root Tables for Scale Factors
// (SHUFF[ch] selector, §5.3.x).
// ---------------------------------------------------------------
//
// | SHUFF[ch] | Code Book       | Square Root Table       |
// | --------- | --------------- | ----------------------- |
// |     0     | SA129           | 6 bit (clause D.1.1)    |
// |     1     | SB129           | 6 bit (clause D.1.1)    |
// |     2     | SC129           | 6 bit (clause D.1.1)    |
// |     3     | SD129           | 6 bit (clause D.1.1)    |
// |     4     | SE129           | 6 bit (clause D.1.1)    |
// |     5     | 6-bit linear    | 6 bit (clause D.1.1)    |
// |     6     | 7-bit linear    | 7 bit (clause D.1.2)    |
// |     7     | Invalid         | Invalid                 |
//
// The five 129-entry SA/SB/SC/SD/SE codebooks themselves are NOT
// transcribed in the staged PDF as "129"-suffixed tables; the spec
// instead routes them through the Annex D §D.5.x small-Huffman
// codebooks for the 5- and 7-level cases. Per Table 5-28's
// `nScaleSum += nScale; pScaleTable->LookUp(nScaleSum, …)` flow, the
// transmitted Huffman codeword is a **difference** between two
// consecutive scale-factor quantisation indexes, not the absolute
// index. The decoder accumulates `nScaleSum` across the loop and
// looks the running sum up in the 6- or 7-bit square-root table.
//
// Round 195 surfaces the 6- and 7-bit linear paths plus the Annex D
// §D.5.3 (5-level: A5/B5/C5) and §D.5.4 (7-level: A7/B7/C7) Huffman
// codebooks used by SHUFF=0..4 to encode the **scale-factor
// difference** symbols. The staged ETSI PDF p.198-200 has the small-
// Huffman codebooks; the dispatch from SHUFF to (5-level or 7-level)
// is identified by the (signed) range of differences the codebook
// covers: SA/SB/SC129 use 5-level (-2..=2) differences and SD/SE129
// use 7-level (-3..=3) differences in the staged tables. The full
// 129-level SA129..SE129 mapping itself remains a docs-completeness
// follow-up because the spec's staged Annex D in this revision
// elides the 129-entry tables; see README "Docs gaps" for the file
// citation.

/// Codebook selector for the scale-factor (SCALES) field, per §5.3.x
/// Table 5-24. `SHUFF[ch] == 7` is reserved/invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalesCodebook {
    /// `SHUFF=0` — SA129 difference codebook (Annex D §D.5.3
    /// Table A5 for the 5-level difference symbol set). Lookup table:
    /// 6-bit RMS (§D.1.1).
    Sa129,
    /// `SHUFF=1` — SB129 difference codebook (Annex D §D.5.3
    /// Table B5). Lookup table: 6-bit RMS (§D.1.1).
    Sb129,
    /// `SHUFF=2` — SC129 difference codebook (Annex D §D.5.3
    /// Table C5). Lookup table: 6-bit RMS (§D.1.1).
    Sc129,
    /// `SHUFF=3` — SD129 difference codebook (Annex D §D.5.4
    /// Table A7). Lookup table: 6-bit RMS (§D.1.1).
    Sd129,
    /// `SHUFF=4` — SE129 difference codebook (Annex D §D.5.4
    /// Table B7). Lookup table: 6-bit RMS (§D.1.1).
    Se129,
    /// `SHUFF=5` — Linear 6-bit (raw 6-bit absolute SCALES index,
    /// 0..=63). Lookup table: 6-bit RMS (§D.1.1).
    Linear6Bit,
    /// `SHUFF=6` — Linear 7-bit (raw 7-bit absolute SCALES index,
    /// 0..=127). Lookup table: 7-bit RMS (§D.1.2).
    Linear7Bit,
}

impl ScalesCodebook {
    /// Resolve a raw 3-bit `SHUFF[ch]` field to a codebook variant
    /// per Table 5-24. `SHUFF == 7` is rejected as
    /// `Error::InvalidSideInfo`.
    pub fn from_shuff(shuff: u8) -> Result<Self> {
        match shuff {
            0 => Ok(Self::Sa129),
            1 => Ok(Self::Sb129),
            2 => Ok(Self::Sc129),
            3 => Ok(Self::Sd129),
            4 => Ok(Self::Se129),
            5 => Ok(Self::Linear6Bit),
            6 => Ok(Self::Linear7Bit),
            _ => Err(Error::InvalidSideInfo {
                field: "SHUFF",
                value: shuff as u32,
            }),
        }
    }

    /// `true` for the five Huffman variants (`SA129..SE129`); the
    /// transmitted symbols are scale-factor index **differences** and
    /// the running accumulator `nScaleSum` is what indexes into the
    /// square-root table. `false` for the two linear variants, whose
    /// symbols are absolute scale-factor indexes themselves.
    pub fn is_huffman_encoded(self) -> bool {
        matches!(
            self,
            Self::Sa129 | Self::Sb129 | Self::Sc129 | Self::Sd129 | Self::Se129
        )
    }

    /// `true` iff the codebook routes through the §D.1.2 7-bit RMS
    /// square-root table (only `Linear7Bit` does); the other six
    /// route through the §D.1.1 6-bit RMS table per Table 5-24.
    pub fn uses_7bit_rms_table(self) -> bool {
        matches!(self, Self::Linear7Bit)
    }
}

/// Annex D §D.5.3 Table A5.
const TABLE_A5: &[HuffmanEntry] = &[(0, 1, 0), (1, 2, 2), (-1, 3, 6), (2, 4, 14), (-2, 4, 15)];

/// Annex D §D.5.3 Table B5.
const TABLE_B5: &[HuffmanEntry] = &[(0, 2, 2), (1, 2, 0), (-1, 2, 1), (2, 3, 6), (-2, 3, 7)];

/// Annex D §D.5.3 Table C5.
const TABLE_C5: &[HuffmanEntry] = &[(0, 1, 0), (1, 3, 4), (-1, 3, 5), (2, 3, 6), (-2, 3, 7)];

/// Annex D §D.5.4 Table A7.
const TABLE_A7: &[HuffmanEntry] = &[
    (0, 1, 0),
    (1, 3, 6),
    (-1, 3, 5),
    (2, 3, 4),
    (-2, 4, 14),
    (3, 5, 31),
    (-3, 5, 30),
];

/// Annex D §D.5.4 Table B7.
const TABLE_B7: &[HuffmanEntry] = &[
    (0, 2, 3),
    (1, 2, 1),
    (-1, 2, 0),
    (2, 3, 4),
    (-2, 4, 11),
    (3, 5, 21),
    (-3, 5, 20),
];

/// Per Table 5-24, the SHUFF=0..2 entries (SA129/SB129/SC129) route
/// through the 5-level codebooks; SHUFF=3..4 (SD129/SE129) route
/// through the 7-level codebooks. The dispatch is by codebook variant.
fn scales_huffman_codebook(codebook: ScalesCodebook) -> (&'static [HuffmanEntry], &'static str) {
    match codebook {
        ScalesCodebook::Sa129 => (TABLE_A5, "A5"),
        ScalesCodebook::Sb129 => (TABLE_B5, "B5"),
        ScalesCodebook::Sc129 => (TABLE_C5, "C5"),
        ScalesCodebook::Sd129 => (TABLE_A7, "A7"),
        ScalesCodebook::Se129 => (TABLE_B7, "B7"),
        // Unreachable: the caller dispatches linear variants
        // separately via the `is_huffman_encoded()` check.
        ScalesCodebook::Linear6Bit | ScalesCodebook::Linear7Bit => (&[], "<linear>"),
    }
}

// ---------------------------------------------------------------
// Annex D §D.1.1 — 6-bit Quantization (Nominal 2,2 dB Step).
// 64 entries; index 63 is reserved/invalid.
// Transcribed verbatim from PDF p.191.
// ---------------------------------------------------------------

/// 6-bit scale-factor square-root quantisation levels, per Annex D
/// §D.1.1. `RMS_6BIT[63]` is reserved/invalid per the staged table
/// (the spec writes "invalid"); reading scale_index 63 from the
/// stream surfaces `Error::InvalidSideInfo { field: "SCALES" }`.
pub const RMS_6BIT: [u32; 64] = [
    1, 2, 2, 3, 3, 4, 6, 7, 10, 12, 16, 20, 26, 34, 44, 56, 72, 93, 120, 155, 200, 257, 331, 427,
    550, 708, 912, 1175, 1514, 1950, 2512, 3236, 4169, 5370, 6918, 8913, 11482, 14791, 19055,
    24547, 31623, 40738, 52481, 67608, 87096, 112202, 144544, 186209, 239883, 309030, 398107,
    512861, 660693, 851138, 1096478, 1412538, 1819701, 2344229, 3019952, 3890451, 5011872, 6456542,
    8317638, // index 63 is "invalid" per the spec; the value placed at the
    // reserved slot is the next continuation of the geometric
    // progression purely so unrelated tests/`len()` arithmetic
    // doesn't see a sentinel.
    0,
];

// ---------------------------------------------------------------
// Annex D §D.1.2 — 7-bit Quantization (Nominal 1,1 dB Step).
// 128 entries; indices 125..=127 are reserved/invalid.
// Transcribed verbatim from PDF p.191-192.
// ---------------------------------------------------------------

/// 7-bit scale-factor square-root quantisation levels, per Annex D
/// §D.1.2. Indices 125, 126, 127 are reserved/invalid per the
/// staged table.
pub const RMS_7BIT: [u32; 128] = [
    1, 1, 2, 2, 2, 2, 3, 3, 3, 4, 4, 5, 6, 7, 7, 8, 10, 11, 12, 14, 16, 18, 20, 23, 26, 30, 34, 38,
    44, 50, 56, 64, 72, 82, 93, 106, 120, 136, 155, 176, 200, 226, 257, 292, 331, 376, 427, 484,
    550, 624, 708, 804, 912, 1035, 1175, 1334, 1514, 1718, 1950, 2213, 2512, 2851, 3236, 3673,
    4169, 4732, 5370, 6095, 6918, 7852, 8913, 10116, 11482, 13032, 14791, 16788, 19055, 21627,
    24547, 27861, 31623, 35892, 40738, 46238, 52481, 59566, 67608, 76736, 87096, 98855, 112202,
    127350, 144544, 164059, 186209, 211349, 239883, 272270, 309030, 350752, 398107, 451856, 512861,
    582103, 660693, 749894, 851138, 966051, 1096478, 1244515, 1412538, 1603245, 1819701, 2065380,
    2344229, 2660725, 3019952, 3427678, 3890451, 4415704, 5011872, 5688529, 6456542, 7328245,
    8317638,
    // indices 125/126/127 are "invalid" per the spec; zero so the
    // reserved slot is recognisable in a debugger and arithmetic
    // doesn't pull in a phantom value.
    0, 0, 0,
];

/// Decode a single SCALES field given the channel-wide `SHUFF[ch]`
/// codebook and the running scale-index accumulator `n_scale_sum`
/// (= `nScaleSum` in §5.4.1 Table 5-28). Implements one iteration of:
///
/// ```text
/// nQSelect = SHUFF[ch];
/// if (nQSelect == 6)   pScaleTable = &RMS7Bit;   // 7-bit (D.1.2)
/// else                 pScaleTable = &RMS6Bit;   // 6-bit (D.1.1)
/// nScaleSum = 0;
/// for (n=0; n<nVQSUB[ch]; n++)
///   if (ABITS[ch][n] > 0) {
///     QSCALES.ppQ[nQSelect]->InverseQ(InputFrame, nScale);
///     if (nQSelect < 5)        // Huffman encoded -> difference
///       nScaleSum += nScale;
///     else                     // linear -> absolute
///       nScaleSum = nScale;
///     pScaleTable->LookUp(nScaleSum, SCALES[ch][n][0]);
///     if (TMODE[ch][n] > 0) {  // transient -> 2nd factor
///       QSCALES.ppQ[nQSelect]->InverseQ(InputFrame, nScale);
///       if (nQSelect < 5)  nScaleSum += nScale;
///       else               nScaleSum  = nScale;
///       pScaleTable->LookUp(nScaleSum, SCALES[ch][n][1]);
///     }
///   }
/// ```
///
/// Returns the resolved scale-factor value (the
/// `pScaleTable->LookUp(...)` output, i.e. the actual quantisation
/// level from §D.1.1 / §D.1.2) and the updated `n_scale_sum`. The
/// caller passes `0` for the first call in a SCALES loop and the
/// returned `n_scale_sum` for subsequent calls.
///
/// The check on `ABITS[ch][n] > 0` is the caller's responsibility
/// (subbands with no allocated bits skip the SCALES decode entirely
/// per §5.4.1). The check on `TMODE[ch][n] > 0` for a second scale
/// factor is also caller-driven (call `decode_scales` twice with the
/// updated `n_scale_sum` for transient subbands).
/// Public entry point: decode a SCALES field starting at `bit_offset`
/// in `bytes`, returning `(scale_factor, updated_n_scale_sum,
/// bits_consumed)`. The `n_scale_sum` parameter is the running
/// accumulator; pass `0` for the first call in a SCALES loop and the
/// returned `updated_n_scale_sum` for subsequent calls.
pub fn decode_scales_at(
    bytes: &[u8],
    bit_offset: usize,
    codebook: ScalesCodebook,
    n_scale_sum: i32,
) -> Result<(u32, i32, usize)> {
    let byte_offset = bit_offset / 8;
    let intra_byte = bit_offset % 8;
    let mut br = BitReader::from_byte_offset(bytes, byte_offset);
    if intra_byte > 0 {
        br.read_bits(intra_byte as u32)?;
    }
    let start = bit_offset;
    let (scale, new_sum) = decode_scales(&mut br, codebook, n_scale_sum)?;
    let bits_consumed = br.absolute_bit_position() - start;
    Ok((scale, new_sum, bits_consumed))
}

/// Decode a single §5.4.1 `JOIN_SCALES` field starting at `bit_offset`,
/// returning `(join_scale_factor, biased_index, bits_consumed)`.
///
/// Per the §5.4.1 Table 5-28 joint-intensity walk (staged PDF p.29):
///
/// ```text
/// nQSelect = JOIN_SHUFF[ch];
/// for (n = nSUBS[ch]; n < nSUBS[nSourceCh]; n++) {
///     QSCALES.ppQ[nQSelect]->InverseQ(InputFrame, nJScale);
///     nJScale = nJScale + 64;                 // fixed +64 bias
///     JScaleTbl.LookUp(nJScale, JOIN_SCALES[ch][n]);
/// }
/// ```
///
/// Unlike the regular SCALES walk, the `JOIN_SCALES` loop does **not**
/// carry a running `nScaleSum` accumulator: each decoded `QSCALES`
/// symbol is biased by a fixed `+64` and directly indexes the §D.3
/// [`crate::join_scale`] table ([`crate::JOIN_SCALE_FACTOR`]). The raw
/// symbol is therefore taken as-is — the Huffman code books emit a
/// signed value in a small window around zero, and the `+64` bias maps
/// the zero symbol to the D.3 unity entry (index 64).
///
/// `codebook` is [`ScalesCodebook::from_shuff`] of the channel's 3-bit
/// `JOIN_SHUFF[ch]`. The Huffman variants (`SA129..SE129`) decode one
/// entropy symbol; the two linear variants read a raw 6- or 7-bit
/// absolute index (which is likewise biased by 64 before the D.3 lookup).
///
/// # Errors
///
/// * [`Error::InvalidSideInfo`] with field `"JOIN_SCALES"` when the
///   biased index falls outside the §D.3 table (`0..=128`) — a
///   well-formed stream keeps it in range by construction;
/// * [`Error::UnexpectedEof`] when the buffer ends mid-symbol.
pub fn decode_join_scale_at(
    bytes: &[u8],
    bit_offset: usize,
    codebook: ScalesCodebook,
) -> Result<(f64, i32, usize)> {
    let byte_offset = bit_offset / 8;
    let intra_byte = bit_offset % 8;
    let mut br = BitReader::from_byte_offset(bytes, byte_offset);
    if intra_byte > 0 {
        br.read_bits(intra_byte as u32)?;
    }
    let start = bit_offset;
    let (factor, biased) = decode_join_scale(&mut br, codebook)?;
    let bits_consumed = br.absolute_bit_position() - start;
    Ok((factor, biased, bits_consumed))
}

/// In-place §5.4.1 `JOIN_SCALES` field decode against a live
/// [`BitReader`]. Returns `(join_scale_factor, biased_index)`. See
/// [`decode_join_scale_at`] for the field semantics.
pub(crate) fn decode_join_scale(
    br: &mut BitReader<'_>,
    codebook: ScalesCodebook,
) -> Result<(f64, i32)> {
    // Extract the raw QSCALES symbol. Huffman variants emit a signed
    // difference symbol; linear variants read a raw absolute index.
    let symbol: i32 = if codebook.is_huffman_encoded() {
        let (table, name) = scales_huffman_codebook(codebook);
        decode_huffman(br, table, name)? as i32
    } else {
        let n_bits = if codebook.uses_7bit_rms_table() { 7 } else { 6 };
        br.read_bits(n_bits)? as i32
    };

    // Bias by +64 and look the biased index up in the §D.3 table.
    let biased = symbol + 64;
    let factor = crate::join_scale::join_scale(biased).ok_or(Error::InvalidSideInfo {
        field: "JOIN_SCALES",
        value: biased as u32,
    })?;

    Ok((factor, biased))
}

pub(crate) fn decode_scales(
    br: &mut BitReader<'_>,
    codebook: ScalesCodebook,
    n_scale_sum: i32,
) -> Result<(u32, i32)> {
    // 1. Extract the bit-stream symbol via the codebook dispatch.
    let symbol: i32 = if codebook.is_huffman_encoded() {
        let (table, name) = scales_huffman_codebook(codebook);
        decode_huffman(br, table, name)? as i32
    } else {
        // Linear: read 6 or 7 raw bits as an unsigned absolute index.
        let n_bits = if codebook.uses_7bit_rms_table() { 7 } else { 6 };
        br.read_bits(n_bits)? as i32
    };

    // 2. Update the running accumulator per the §5.4.1 dispatch:
    //    Huffman entries are differences; linear entries are absolute.
    let new_scale_sum: i32 = if codebook.is_huffman_encoded() {
        n_scale_sum + symbol
    } else {
        symbol
    };

    // 3. Look up the resulting scale factor in the appropriate
    //    square-root table per Table 5-24.
    let table: &[u32] = if codebook.uses_7bit_rms_table() {
        &RMS_7BIT
    } else {
        &RMS_6BIT
    };

    // Bounds-check the accumulator before indexing. A Huffman-encoded
    // stream whose accumulated differences walk outside [0, table.len)
    // is a stream-format failure (the encoder is required to keep
    // scale-factor indexes inside the table by construction).
    let idx_signed = new_scale_sum;
    let len_signed = table.len() as i32;
    if !(0..len_signed).contains(&idx_signed) {
        return Err(Error::InvalidSideInfo {
            field: "SCALES",
            value: idx_signed as u32,
        });
    }
    let idx = idx_signed as usize;

    // Reject the spec-reserved indices (63 in §D.1.1; 125..=127 in
    // §D.1.2) explicitly so the sentinel 0 we placed in the const
    // doesn't leak through as a "valid" scale factor.
    let invalid_in_6bit = !codebook.uses_7bit_rms_table() && idx == 63;
    let invalid_in_7bit = codebook.uses_7bit_rms_table() && idx >= 125;
    if invalid_in_6bit || invalid_in_7bit {
        return Err(Error::InvalidSideInfo {
            field: "SCALES",
            value: idx as u32,
        });
    }

    Ok((table[idx], new_scale_sum))
}

// ---------------------------------------------------------------
// Table 5-27 — Scale Factor Adjustment Index (ADJ)
// (§5.4.1 / §5.3.x, staged PDF p.27)
// ---------------------------------------------------------------
//
// Verbatim from the staged PDF p.27, Table 5-27 "Scale Factor
// Adjustment Index":
//
// | ADJ | Adjustment Value |
// | --- | ---------------- |
// |  0  | 1,0000           |
// |  1  | 1,1250           |
// |  2  | 1,2500           |
// |  3  | 1,4375           |
//
// (ETSI decimal-comma convention: `1,4375` = 1.4375.) PDF p.25
// (Core audio coding header pseudocode, Table 5-21 entry "Look up
// ADJ table") fixes the wire width at **two bits** for every
// occurrence — the listing reads `ADJ = ExtractBits(2);` on every
// branch (ABITS=1 with SEL=0; ABITS=2..=5 with SEL<3; ABITS=6..=10
// with SEL<7). The spec note under Table 5-27 reads: "This table
// shows the scale factor adjustment index values if Huffman coding
// is used to encode the subband quantization indexes" — the
// multiplier is applied to the scale factor (SCALES) for that
// (channel, subband) pair before the inverse quantiser runs.
//
// The two-bit wire encoding covers exactly the four documented
// rows: `0b00`..=`0b11` map to the four `Adj0..=Adj3` variants.
// The mapping is total over a masked 2-bit input, so
// `from_index(0..=3)` is total and the `decode_adj_at` reader
// always returns a typed variant inside a well-formed bit stream.

/// Scale-factor adjustment multiplier decoded from the 2-bit `ADJ`
/// header field per **ETSI TS 102 114 V1.3.1 §5.4.1, Table 5-27**
/// (staged PDF p.27).
///
/// `ADJ` is read from the bit stream during Core Audio Coding
/// Header processing (§5.3.x pseudocode, PDF p.25) whenever the
/// per-subband `SEL[ch][n]` codebook selector falls into a Huffman
/// range — the adjustment multiplier is applied to the SCALES
/// value for that `(channel, subband)` before the inverse
/// quantiser runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ScaleFactorAdjustment {
    /// `ADJ = 0b00` — multiplier 1.0000 (Table 5-27 row 1, the
    /// identity adjustment).
    Adj0,
    /// `ADJ = 0b01` — multiplier 1.1250 (Table 5-27 row 2).
    Adj1,
    /// `ADJ = 0b10` — multiplier 1.2500 (Table 5-27 row 3).
    Adj2,
    /// `ADJ = 0b11` — multiplier 1.4375 (Table 5-27 row 4).
    Adj3,
}

impl ScaleFactorAdjustment {
    /// Resolve a raw 2-bit `ADJ` field to a variant per Table 5-27.
    ///
    /// Only the low 2 bits of `adj` are consulted — the wire field
    /// is 2 bits per Table 5-21's `ExtractBits(2)` notation
    /// (PDF p.25), so any caller passing a wider integer is masked
    /// down before dispatch. The mapping is total: every 2-bit
    /// value `0..=3` corresponds to one of the four documented
    /// rows.
    pub fn from_index(adj: u8) -> Self {
        match adj & 0b11 {
            0b00 => Self::Adj0,
            0b01 => Self::Adj1,
            0b10 => Self::Adj2,
            // The remaining 2-bit pattern `0b11` is the only one
            // left, so this branch is exhaustive on a masked 2-bit
            // input.
            _ => Self::Adj3,
        }
    }

    /// The 2-bit ADJ wire code (`0..=3`) corresponding to this
    /// variant; the inverse of [`Self::from_index`].
    pub fn code(self) -> u8 {
        match self {
            Self::Adj0 => 0b00,
            Self::Adj1 => 0b01,
            Self::Adj2 => 0b10,
            Self::Adj3 => 0b11,
        }
    }

    /// Adjustment multiplier per Table 5-27 (`f32`).
    ///
    /// All four multipliers have exact binary representations:
    /// `1.0`, `1.125 = 9 / 8`, `1.25 = 5 / 4`, and
    /// `1.4375 = 23 / 16`. The constants below are therefore
    /// representable as `f32` (and as `f64`) with no rounding.
    pub fn multiplier(self) -> f32 {
        match self {
            Self::Adj0 => 1.0000,
            Self::Adj1 => 1.1250,
            Self::Adj2 => 1.2500,
            Self::Adj3 => 1.4375,
        }
    }

    /// Adjustment multiplier per Table 5-27 (`f64`).
    ///
    /// Provided for callers that hold SCALES in `f64`; the four
    /// constants are exactly representable so the `f32` and `f64`
    /// projections are numerically identical.
    pub fn multiplier_f64(self) -> f64 {
        match self {
            Self::Adj0 => 1.0000,
            Self::Adj1 => 1.1250,
            Self::Adj2 => 1.2500,
            Self::Adj3 => 1.4375,
        }
    }

    /// The adjustment multiplier as a rational with denominator 16:
    /// `(numerator, 16)`. Every Table 5-27 multiplier is a multiple
    /// of `1/16`, so a `u8` numerator over the fixed denominator
    /// `16` is an exact representation for integer-arithmetic
    /// callers: `Adj0 → 16/16`, `Adj1 → 18/16`, `Adj2 → 20/16`,
    /// `Adj3 → 23/16`.
    pub fn multiplier_rational(self) -> (u8, u8) {
        let num = match self {
            Self::Adj0 => 16, // 1.0000   = 16/16
            Self::Adj1 => 18, // 1.1250   = 18/16
            Self::Adj2 => 20, // 1.2500   = 20/16
            Self::Adj3 => 23, // 1.4375   = 23/16
        };
        (num, 16)
    }
}

/// Public entry point: decode an `ADJ` field starting at
/// `bit_offset` in `bytes`, returning `(adjustment, bits_consumed)`.
///
/// The bit offset is measured from the MSB of `bytes[0]`, matching
/// the MSB-first convention used elsewhere in this crate. The field
/// width is fixed at 2 bits per Table 5-21 (PDF p.25). Returns
/// [`Error::UnexpectedEof`] when fewer than 2 bits remain after
/// `bit_offset`.
pub fn decode_adj_at(bytes: &[u8], bit_offset: usize) -> Result<(ScaleFactorAdjustment, usize)> {
    let byte_offset = bit_offset / 8;
    let intra_byte = bit_offset % 8;
    let mut br = BitReader::from_byte_offset(bytes, byte_offset);
    if intra_byte > 0 {
        br.read_bits(intra_byte as u32)?;
    }
    let start = bit_offset;
    let raw = br.read_bits(2)? as u8;
    let adj = ScaleFactorAdjustment::from_index(raw);
    let bits_consumed = br.absolute_bit_position() - start;
    Ok((adj, bits_consumed))
}

// ---------------------------------------------------------------
// §5.4.1 Table 5-28 — Subsubframe Count (SSC, nSSC) and Partial
// Subsubframe Sample Count (PSC).
// (Staged PDF p.28 — first three rows of the Primary Audio Side
// Information pseudocode; field descriptions on PDF p.29–p.30.)
// ---------------------------------------------------------------
//
// Verbatim from PDF p.28, top of Table 5-28:
//
//     SSC  = ExtractBits(2);    // 2 bits
//     nSSC = SSC + 1;
//     PSC  = ExtractBits(3);    // 3 bits
//
// Field descriptions, PDF p.29 ("SSC (Subsubframe Count)") and
// p.30 ("PSC (Partial Subsubframe Sample Count)"):
//
// * **SSC.** "Indicates that there are `nSSC = SSC + 1` subsubframes
//   in the current audio subframe." Wire width 2 bits → the count
//   `nSSC` ranges over `1..=4` (the four valid Core-profile
//   subsubframe counts).
// * **PSC.** "Indicates the number of subband samples held in a
//   partial subsubframe for each of the active subbands. A partial
//   subsubframe is one which has less than 8 subband samples. It
//   exists only in a termination frame and is always at the end of
//   the last normal subsubframe. A DSYNC word will always occur
//   after a partial subsubframe." Wire width 3 bits → `0..=7`. A
//   normal (non-termination) subsubframe carries 8 samples per
//   active subband, so `PSC < 8`; the spec leaves `PSC = 0` as the
//   "no partial subsubframe present" sentinel that termination
//   frames may emit but that is structurally always meaningful only
//   in termination frames.
//
// The downstream §5.4.1 loops (e.g. `for (n=0; n<nSSC; n++)` over
// PMODE and PVQ, or the SCALES sample-count `8 * nSSC` quantifier
// used by Annex C §C.2.3 / §C.2.4 / §C.2.5) all consume the
// `nSSC` value; downstream sum/difference / joint-subband / QMF
// code in this crate already references `8 * nSSC` as its
// per-frame sample stride (see `src/sum_diff.rs` and
// `src/joint_subband.rs`), so wiring the 5-bit head into a typed
// decoder closes the loop on those quantifiers.

/// Subsubframe-count prefix at the head of §5.4.1 Table 5-28
/// (staged PDF p.28).
///
/// Wraps the three values produced by the first two `ExtractBits`
/// reads of the Primary Audio Side Information block:
///
/// * `ssc`  — raw `SSC` wire field (2 bits, `0..=3`).
/// * `n_ssc` — derived count `SSC + 1` (`1..=4`), i.e. the number of
///   subsubframes in the current audio subframe (PDF p.29).
/// * `psc`  — raw `PSC` wire field (3 bits, `0..=7`), the partial
///   subsubframe sample count (PDF p.30).
///
/// The `n_ssc` and `samples_per_subsubframe_normal` accessors hide
/// the `+ 1` / `× 8` arithmetic so call sites that need the
/// `8 * nSSC` per-subband sample stride (used by the §C.2 sum/diff
/// and joint-subband loops) don't repeat it in raw form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct SubsubframeCount {
    /// Raw 2-bit `SSC` wire field (`0..=3`). The 2-bit
    /// `ExtractBits(2)` in PDF p.28 yields this value directly.
    pub ssc: u8,
    /// Raw 3-bit `PSC` wire field (`0..=7`). The 3-bit
    /// `ExtractBits(3)` in PDF p.28 yields this value directly.
    /// A value of `psc > 0` marks the trailing partial subsubframe
    /// of a termination frame; a value of `0` means no partial
    /// subsubframe is present at the tail of this audio subframe.
    pub psc: u8,
}

impl SubsubframeCount {
    /// Maximum 2-bit `SSC` wire value (`0b11 = 3`); decoded
    /// `n_ssc` is therefore at most `4`.
    pub const MAX_SSC: u8 = 0b11;
    /// Maximum 3-bit `PSC` wire value (`0b111 = 7`).
    pub const MAX_PSC: u8 = 0b111;
    /// Total wire width of the `SSC` + `PSC` prefix, in bits:
    /// `2 + 3 = 5`.
    pub const WIRE_BITS: u32 = 5;

    /// Construct from the raw 2-bit `SSC` and 3-bit `PSC` fields.
    ///
    /// Only the low 2 / 3 bits of the inputs are consulted; any
    /// higher bits in the caller-supplied integers are masked off
    /// (matching the `ExtractBits(2)` / `ExtractBits(3)` semantics
    /// of the §5.4.1 pseudocode). The mapping is total.
    pub fn new(ssc: u8, psc: u8) -> Self {
        Self {
            ssc: ssc & Self::MAX_SSC,
            psc: psc & Self::MAX_PSC,
        }
    }

    /// Decoded subsubframe count `nSSC = SSC + 1` (`1..=4`).
    ///
    /// Per PDF p.29 field description: "Indicates that there are
    /// `nSSC = SSC + 1` subsubframes in the current audio
    /// subframe." This is the count consumed by every downstream
    /// §5.4.1 loop iterating over subsubframes.
    pub fn n_ssc(self) -> u8 {
        self.ssc + 1
    }

    /// Convenience: subband-sample stride for a *normal* (non-
    /// partial) subsubframe span, equal to `8 * nSSC` (the quantity
    /// used by Annex C §C.2.3 / §C.2.4 / §C.2.5 to size per-
    /// subband sample arrays). Returns a `usize` for direct use as
    /// a loop bound. Result fits in `5..=32` for any valid `ssc`.
    pub fn samples_per_subsubframe_normal(self) -> usize {
        // 8 samples per active subband per subsubframe, times nSSC
        // subsubframes per audio subframe. Max value 8 * 4 = 32
        // fits comfortably in u8, but we promote to usize because
        // downstream callers index into per-subband sample slices.
        8usize * self.n_ssc() as usize
    }

    /// Returns `Some(psc)` when this audio subframe ends with a
    /// partial subsubframe (`psc > 0`, termination-frame signal
    /// per PDF p.30), or `None` when no partial tail is present
    /// (`psc == 0`).
    ///
    /// The returned value is the partial subsubframe's sample
    /// count per active subband (so `< 8`, since a partial
    /// subsubframe by definition holds fewer than 8 subband
    /// samples).
    pub fn partial_sample_count(self) -> Option<u8> {
        if self.psc == 0 {
            None
        } else {
            Some(self.psc)
        }
    }

    /// Returns `true` if this prefix signals a termination frame
    /// tail (i.e. `psc > 0`, per PDF p.30).
    pub fn is_termination_tail(self) -> bool {
        self.psc != 0
    }
}

/// Decode the 5-bit `SSC` + `PSC` head of the §5.4.1 Primary
/// Audio Side Information block from `bytes`, starting at
/// `bit_offset` (MSB-first from `bytes[0]`).
///
/// Returns `(SubsubframeCount, bits_consumed)` on success. The
/// width of the prefix is exactly [`SubsubframeCount::WIRE_BITS`]
/// (5 bits) per Table 5-28: SSC is read first as a 2-bit
/// `ExtractBits(2)`, then PSC as a 3-bit `ExtractBits(3)`.
///
/// Returns [`Error::UnexpectedEof`] when fewer than 5 bits remain
/// after `bit_offset`.
pub fn decode_subsubframe_count_at(
    bytes: &[u8],
    bit_offset: usize,
) -> Result<(SubsubframeCount, usize)> {
    let byte_offset = bit_offset / 8;
    let intra_byte = bit_offset % 8;
    let mut br = BitReader::from_byte_offset(bytes, byte_offset);
    if intra_byte > 0 {
        br.read_bits(intra_byte as u32)?;
    }
    let start = bit_offset;
    // SSC = ExtractBits(2)
    let ssc = br.read_bits(2)? as u8;
    // PSC = ExtractBits(3)
    let psc = br.read_bits(3)? as u8;
    let prefix = SubsubframeCount::new(ssc, psc);
    let bits_consumed = br.absolute_bit_position() - start;
    Ok((prefix, bits_consumed))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------
    // BHUFF / ABITS decode tests
    // -----------------------------------------------------------

    #[test]
    fn bhuff_dispatch_rejects_reserved_value() {
        assert!(matches!(
            AbitsCodebook::from_bhuff(7),
            Err(Error::InvalidSideInfo {
                field: "BHUFF",
                value: 7
            })
        ));
        // 8..=255 would also be out-of-3-bit-range but the caller
        // is responsible for masking BHUFF to its 3-bit width before
        // dispatch; we still reject everything above 6.
        assert!(AbitsCodebook::from_bhuff(8).is_err());
    }

    #[test]
    fn bhuff_dispatch_resolves_six_documented_codes() {
        // Cover the full {0..=6} grid of Table 5-25.
        assert_eq!(AbitsCodebook::from_bhuff(0).unwrap(), AbitsCodebook::A12);
        assert_eq!(AbitsCodebook::from_bhuff(1).unwrap(), AbitsCodebook::B12);
        assert_eq!(AbitsCodebook::from_bhuff(2).unwrap(), AbitsCodebook::C12);
        assert_eq!(AbitsCodebook::from_bhuff(3).unwrap(), AbitsCodebook::D12);
        assert_eq!(AbitsCodebook::from_bhuff(4).unwrap(), AbitsCodebook::E12);
        assert_eq!(
            AbitsCodebook::from_bhuff(5).unwrap(),
            AbitsCodebook::Linear4Bit
        );
        assert_eq!(
            AbitsCodebook::from_bhuff(6).unwrap(),
            AbitsCodebook::Linear5Bit
        );
    }

    /// Pack a series of (code, code_length) pairs into a byte stream
    /// MSB-first. Trailing bits are zero-padded.
    fn pack_codes(codes: &[(u16, u8)]) -> Vec<u8> {
        let total_bits: usize = codes.iter().map(|(_, len)| *len as usize).sum();
        let total_bytes = total_bits.div_ceil(8);
        let mut out = vec![0u8; total_bytes];
        let mut bit_pos: usize = 0;
        for &(code, len) in codes {
            for i in (0..len).rev() {
                let bit = ((code >> i) & 1) as u8;
                let byte_idx = bit_pos / 8;
                let bit_in_byte = 7 - (bit_pos % 8);
                out[byte_idx] |= bit << bit_in_byte;
                bit_pos += 1;
            }
        }
        out
    }

    #[test]
    fn decode_abits_a12_walks_every_symbol() {
        // A12 entries: (symbol, code_length, code). Pack each
        // (code, code_length) pair in order, then decode them back.
        let codes: Vec<(u16, u8)> = TABLE_A12.iter().map(|&(_, l, c)| (c, l)).collect();
        let stream = pack_codes(&codes);
        let mut br = BitReader::new(&stream);
        for &(expected_symbol, _, _) in TABLE_A12 {
            let got = decode_abits(&mut br, AbitsCodebook::A12).unwrap();
            assert_eq!(got as i16, expected_symbol);
        }
    }

    #[test]
    fn decode_abits_every_huffman_codebook_walks_every_symbol() {
        // Exhaustive cross-check: each of A12/B12/C12/D12/E12 must
        // round-trip every symbol it lists.
        for (cb, table) in [
            (AbitsCodebook::A12, TABLE_A12),
            (AbitsCodebook::B12, TABLE_B12),
            (AbitsCodebook::C12, TABLE_C12),
            (AbitsCodebook::D12, TABLE_D12),
            (AbitsCodebook::E12, TABLE_E12),
        ] {
            let codes: Vec<(u16, u8)> = table.iter().map(|&(_, l, c)| (c, l)).collect();
            let stream = pack_codes(&codes);
            let mut br = BitReader::new(&stream);
            for &(expected_symbol, _, _) in table {
                let got = decode_abits(&mut br, cb).unwrap();
                assert_eq!(
                    got as i16, expected_symbol,
                    "codebook {:?} mis-decoded symbol {}",
                    cb, expected_symbol
                );
            }
        }
    }

    #[test]
    fn decode_abits_linear_4bit_returns_raw_field() {
        // 4-bit linear: high nibble of byte 0 = 0xA = 10.
        let stream = [0xA0];
        let mut br = BitReader::new(&stream);
        assert_eq!(
            decode_abits(&mut br, AbitsCodebook::Linear4Bit).unwrap(),
            10
        );
    }

    #[test]
    fn decode_abits_linear_5bit_returns_raw_field() {
        // 5-bit linear: top 5 bits of 0b10011_000 = 0b10011 = 19.
        let stream = [0b1001_1000];
        let mut br = BitReader::new(&stream);
        assert_eq!(
            decode_abits(&mut br, AbitsCodebook::Linear5Bit).unwrap(),
            19
        );
    }

    #[test]
    fn decode_abits_short_buffer_surfaces_eof() {
        // The shortest A12 code is 1 bit; an empty buffer fails before
        // it can read even that single bit.
        let stream: [u8; 0] = [];
        let mut br = BitReader::new(&stream);
        assert_eq!(
            decode_abits(&mut br, AbitsCodebook::A12).unwrap_err(),
            Error::UnexpectedEof
        );
    }

    #[test]
    fn huffman_codebooks_are_complete_prefix_codes() {
        // Sanity check: each of the ten Annex D codebooks transcribed
        // in this module must satisfy Kraft's inequality with equality
        // (sum_i 2^{-len_i} == 1.0) for it to be a complete prefix
        // code — i.e. every infinite bit stream maps to exactly one
        // symbol. The ETSI tables are designed this way and our
        // decoder relies on the property (a "no Huffman entry matched"
        // failure cannot fire on bit-stream input once the codebook
        // dispatch picked one of these tables; only EOF can fail).
        for (name, table) in [
            ("A12", TABLE_A12),
            ("B12", TABLE_B12),
            ("C12", TABLE_C12),
            ("D12", TABLE_D12),
            ("E12", TABLE_E12),
            ("A5", TABLE_A5),
            ("B5", TABLE_B5),
            ("C5", TABLE_C5),
            ("A7", TABLE_A7),
            ("B7", TABLE_B7),
            ("A4", TABLE_A4),
            ("B4", TABLE_B4),
            ("C4", TABLE_C4),
            ("D4", TABLE_D4),
        ] {
            let kraft: f64 = table
                .iter()
                .map(|&(_, len, _)| 2f64.powi(-(len as i32)))
                .sum();
            assert!(
                (kraft - 1.0).abs() < 1e-9,
                "codebook {name} fails Kraft equality (sum = {kraft})",
            );
        }
    }

    // -----------------------------------------------------------
    // THUFF / TMODE decode tests
    // -----------------------------------------------------------

    #[test]
    fn thuff_dispatch_resolves_all_four_documented_codes() {
        // Cover the full {0..=3} grid of Table 5-23.
        assert_eq!(TmodeCodebook::from_thuff(0), TmodeCodebook::A4);
        assert_eq!(TmodeCodebook::from_thuff(1), TmodeCodebook::B4);
        assert_eq!(TmodeCodebook::from_thuff(2), TmodeCodebook::C4);
        assert_eq!(TmodeCodebook::from_thuff(3), TmodeCodebook::D4);
    }

    #[test]
    fn thuff_dispatch_masks_high_bits_and_round_trips() {
        // Only the low 2 bits are consulted (Table 5-21 fixes the
        // wire width at ExtractBits(2)); every u8 input resolves to
        // the variant of its low 2 bits, and thuff() inverts it.
        for raw in 0..=u8::MAX {
            let cb = TmodeCodebook::from_thuff(raw);
            assert_eq!(cb, TmodeCodebook::from_thuff(raw & 0b11));
            assert_eq!(cb.thuff(), raw & 0b11);
            assert_eq!(TmodeCodebook::from_thuff(cb.thuff()), cb);
        }
    }

    #[test]
    fn decode_tmode_every_codebook_walks_every_symbol() {
        // Each of A4/B4/C4/D4 must round-trip every symbol it lists
        // (the four §D.5.2 tables, staged PDF p.198).
        for (cb, table) in [
            (TmodeCodebook::A4, TABLE_A4),
            (TmodeCodebook::B4, TABLE_B4),
            (TmodeCodebook::C4, TABLE_C4),
            (TmodeCodebook::D4, TABLE_D4),
        ] {
            let codes: Vec<(u16, u8)> = table.iter().map(|&(_, l, c)| (c, l)).collect();
            let stream = pack_codes(&codes);
            let mut br = BitReader::new(&stream);
            for &(expected_symbol, _, _) in table {
                let got = decode_tmode(&mut br, cb).unwrap();
                assert_eq!(
                    got as i16, expected_symbol,
                    "codebook {:?} mis-decoded symbol {}",
                    cb, expected_symbol
                );
            }
        }
    }

    #[test]
    fn decode_tmode_at_reports_code_length_and_offset_handling() {
        // A4's symbol 0 is the 1-bit code `0`; symbol 3 is the
        // 3-bit code `111`. Pack `0` then `111` then `10` (= symbol
        // 1) back-to-back starting at bit offset 3 after three
        // leading filler 1-bits: 0b111_0_111_1, 0b0_0000000.
        let stream = [0b1110_1111, 0b0000_0000];
        let (s0, used0) = decode_tmode_at(&stream, 3, TmodeCodebook::A4).unwrap();
        assert_eq!((s0, used0), (0, 1));
        let (s1, used1) = decode_tmode_at(&stream, 4, TmodeCodebook::A4).unwrap();
        assert_eq!((s1, used1), (3, 3));
        let (s2, used2) = decode_tmode_at(&stream, 7, TmodeCodebook::A4).unwrap();
        assert_eq!((s2, used2), (1, 2));
    }

    #[test]
    fn decode_tmode_d4_is_a_raw_two_bit_field() {
        // Table D4 maps every 2-bit code to itself; one byte packs
        // four consecutive symbols 0b00_01_10_11 -> 0, 1, 2, 3.
        let stream = [0b0001_1011];
        let mut br = BitReader::new(&stream);
        for expected in 0..=3u8 {
            assert_eq!(decode_tmode(&mut br, TmodeCodebook::D4).unwrap(), expected);
        }
    }

    #[test]
    fn decode_tmode_short_buffer_surfaces_eof() {
        let stream: [u8; 0] = [];
        assert_eq!(
            decode_tmode_at(&stream, 0, TmodeCodebook::B4).unwrap_err(),
            Error::UnexpectedEof
        );
    }

    // -----------------------------------------------------------
    // SHUFF / SCALES decode tests
    // -----------------------------------------------------------

    #[test]
    fn shuff_dispatch_rejects_reserved_value() {
        assert!(matches!(
            ScalesCodebook::from_shuff(7),
            Err(Error::InvalidSideInfo {
                field: "SHUFF",
                value: 7
            })
        ));
    }

    #[test]
    fn shuff_dispatch_resolves_seven_documented_codes() {
        for (raw, expected) in [
            (0u8, ScalesCodebook::Sa129),
            (1, ScalesCodebook::Sb129),
            (2, ScalesCodebook::Sc129),
            (3, ScalesCodebook::Sd129),
            (4, ScalesCodebook::Se129),
            (5, ScalesCodebook::Linear6Bit),
            (6, ScalesCodebook::Linear7Bit),
        ] {
            assert_eq!(ScalesCodebook::from_shuff(raw).unwrap(), expected);
        }
    }

    #[test]
    fn shuff_uses_7bit_table_only_for_linear7() {
        for cb in [
            ScalesCodebook::Sa129,
            ScalesCodebook::Sb129,
            ScalesCodebook::Sc129,
            ScalesCodebook::Sd129,
            ScalesCodebook::Se129,
            ScalesCodebook::Linear6Bit,
        ] {
            assert!(!cb.uses_7bit_rms_table(), "{:?} routes through D.1.1", cb);
        }
        assert!(ScalesCodebook::Linear7Bit.uses_7bit_rms_table());
    }

    #[test]
    fn rms_table_lengths_match_spec_widths() {
        // §D.1.1: 64 entries (index 0..=62 valid, 63 invalid).
        // §D.1.2: 128 entries (index 0..=124 valid, 125..=127 invalid).
        assert_eq!(RMS_6BIT.len(), 64);
        assert_eq!(RMS_7BIT.len(), 128);
    }

    #[test]
    fn rms_table_anchor_values_match_spec_pdf() {
        // Spot-check anchor entries against the staged PDF p.191-192.
        assert_eq!(RMS_6BIT[0], 1); // (0,0 dB)
        assert_eq!(RMS_6BIT[1], 2); // (6,0 dB)
        assert_eq!(RMS_6BIT[31], 3236); // (70,2 dB)
        assert_eq!(RMS_6BIT[62], 8317638); // (138,4 dB)

        assert_eq!(RMS_7BIT[0], 1); // (0,0 dB)
        assert_eq!(RMS_7BIT[31], 64); // (36,1 dB)
                                      // Index 63 is the bottom of column 1 in the §D.1.2 table; the
                                      // staged PDF p.192 shows index 63 = 3673 ((71,3 dB)) and
                                      // index 64 = 4169 (top of column 2, (72,4 dB)).
        assert_eq!(RMS_7BIT[63], 3673);
        assert_eq!(RMS_7BIT[64], 4169);
        assert_eq!(RMS_7BIT[124], 8317638); // (138,4 dB)
    }

    #[test]
    fn decode_scales_linear6_returns_absolute_lookup() {
        // Pack a raw 6-bit absolute index = 5, followed by a 6-bit
        // index = 10 (in the same byte stream). The accumulator is
        // overwritten each call (linear path).
        // Bit layout: 000101_001010_00 = 0b00010100_10100000 = 0x14 0xA0.
        let stream = [0x14, 0xA0];
        let mut br = BitReader::new(&stream);
        let (val, sum) = decode_scales(&mut br, ScalesCodebook::Linear6Bit, 0).unwrap();
        assert_eq!(val, RMS_6BIT[5]); // RMS_6BIT[5] = 4
        assert_eq!(sum, 5);
        let (val, sum) = decode_scales(&mut br, ScalesCodebook::Linear6Bit, sum).unwrap();
        assert_eq!(val, RMS_6BIT[10]); // RMS_6BIT[10] = 16
        assert_eq!(sum, 10); // linear: accumulator overwritten, not summed.
    }

    #[test]
    fn decode_scales_linear7_returns_absolute_lookup() {
        // Pack a 7-bit absolute index = 31. 0011111_0 = 0x3E.
        let stream = [0x3E];
        let mut br = BitReader::new(&stream);
        let (val, sum) = decode_scales(&mut br, ScalesCodebook::Linear7Bit, 0).unwrap();
        assert_eq!(val, RMS_7BIT[31]); // RMS_7BIT[31] = 64
        assert_eq!(sum, 31);
    }

    #[test]
    fn decode_scales_sa129_accumulates_differences() {
        // SA129 -> TABLE_A5. Symbols carry signed differences.
        // Pack the sequence (+1, +1, +1, -1) which equals
        // (TABLE_A5[1].code, TABLE_A5[0].code, TABLE_A5[0].code,
        //  TABLE_A5[2].code) by their (symbol -> entry) mapping:
        //   +1 -> (1, 2, 2)
        //    0 -> (0, 1, 0)   (we'll use this for "no movement")
        //   -1 -> (-1, 3, 6)
        // Actually use +1, +1, -1 (skip the zero-movement step):
        //   +1 = code 0b10 (len 2)
        //   +1 = code 0b10 (len 2)
        //   -1 = code 0b110 (len 3)
        // Stream bits: 10_10_110_0 = 0b10101100 = 0xAC.
        let stream = [0xAC];
        let mut br = BitReader::new(&stream);

        let (val1, sum1) = decode_scales(&mut br, ScalesCodebook::Sa129, 0).unwrap();
        // 0 + 1 = 1; RMS_6BIT[1] = 2.
        assert_eq!(sum1, 1);
        assert_eq!(val1, RMS_6BIT[1]);

        let (val2, sum2) = decode_scales(&mut br, ScalesCodebook::Sa129, sum1).unwrap();
        // 1 + 1 = 2; RMS_6BIT[2] = 2.
        assert_eq!(sum2, 2);
        assert_eq!(val2, RMS_6BIT[2]);

        let (val3, sum3) = decode_scales(&mut br, ScalesCodebook::Sa129, sum2).unwrap();
        // 2 + (-1) = 1; RMS_6BIT[1] = 2.
        assert_eq!(sum3, 1);
        assert_eq!(val3, RMS_6BIT[1]);
    }

    #[test]
    fn decode_scales_negative_accumulator_rejected() {
        // SA129 starting at 0, transmit -1 (code 0b110 len 3 + pad).
        // Resulting accumulator = -1, out of [0, 64) → error.
        let stream = [0b1100_0000];
        let mut br = BitReader::new(&stream);
        let err = decode_scales(&mut br, ScalesCodebook::Sa129, 0).unwrap_err();
        assert!(matches!(
            err,
            Error::InvalidSideInfo {
                field: "SCALES",
                ..
            }
        ));
    }

    #[test]
    fn decode_scales_reserved_indices_rejected() {
        // Linear6Bit reading raw index 63 must reject (spec-reserved).
        // 0b111111_00 = 0xFC.
        let stream = [0xFC];
        let mut br = BitReader::new(&stream);
        let err = decode_scales(&mut br, ScalesCodebook::Linear6Bit, 0).unwrap_err();
        assert!(matches!(
            err,
            Error::InvalidSideInfo {
                field: "SCALES",
                value: 63
            }
        ));

        // Linear7Bit reading raw index 125 must reject. 0b1111101_0 = 0xFA.
        let stream = [0xFA];
        let mut br = BitReader::new(&stream);
        let err = decode_scales(&mut br, ScalesCodebook::Linear7Bit, 0).unwrap_err();
        assert!(matches!(
            err,
            Error::InvalidSideInfo {
                field: "SCALES",
                value: 125
            }
        ));
    }

    #[test]
    fn decode_scales_sd129_uses_7level_table_with_difference_semantics() {
        // SD129 -> TABLE_A7. Symbols ±3 in addition to A5's ±2 range.
        // Pack +3 (code 31, len 5) then -3 (code 30, len 5).
        // Stream bits: 11111_11110_000000 = 0b11111111 0b10000000 = 0xFF 0x80.
        let stream = [0xFF, 0x80];
        let mut br = BitReader::new(&stream);

        let (val1, sum1) = decode_scales(&mut br, ScalesCodebook::Sd129, 0).unwrap();
        assert_eq!(sum1, 3); // 0 + 3
        assert_eq!(val1, RMS_6BIT[3]); // RMS_6BIT[3] = 3

        let (val2, sum2) = decode_scales(&mut br, ScalesCodebook::Sd129, sum1).unwrap();
        assert_eq!(sum2, 0); // 3 + (-3)
        assert_eq!(val2, RMS_6BIT[0]); // RMS_6BIT[0] = 1
    }

    // -----------------------------------------------------------
    // Table 5-27 — Scale Factor Adjustment Index (ADJ) tests
    // -----------------------------------------------------------

    #[test]
    fn adj_table_5_27_row_by_row() {
        // Every documented (ADJ, Adjustment Value) row from Table 5-27.
        let rows: [(u8, ScaleFactorAdjustment, f32); 4] = [
            (0b00, ScaleFactorAdjustment::Adj0, 1.0000),
            (0b01, ScaleFactorAdjustment::Adj1, 1.1250),
            (0b10, ScaleFactorAdjustment::Adj2, 1.2500),
            (0b11, ScaleFactorAdjustment::Adj3, 1.4375),
        ];
        for (code, expected_variant, expected_value) in rows {
            let v = ScaleFactorAdjustment::from_index(code);
            assert_eq!(v, expected_variant, "from_index({code:#04b})");
            assert_eq!(v.code(), code, "code() round-trip for {v:?}");
            assert_eq!(
                v.multiplier(),
                expected_value,
                "multiplier() (f32) for {v:?}"
            );
            assert_eq!(
                v.multiplier_f64(),
                expected_value as f64,
                "multiplier_f64() for {v:?}"
            );
        }
    }

    #[test]
    fn adj_from_index_masks_high_bits() {
        // The wire field is 2 bits; widths beyond are masked off.
        // `0b1100` & 0b11 == 0b00 → Adj0; `0b1111` & 0b11 == 0b11 → Adj3.
        assert_eq!(
            ScaleFactorAdjustment::from_index(0b1100),
            ScaleFactorAdjustment::Adj0
        );
        assert_eq!(
            ScaleFactorAdjustment::from_index(0b1111),
            ScaleFactorAdjustment::Adj3
        );
        assert_eq!(
            ScaleFactorAdjustment::from_index(0xFF),
            ScaleFactorAdjustment::Adj3
        );
        assert_eq!(
            ScaleFactorAdjustment::from_index(0xFC),
            ScaleFactorAdjustment::Adj0
        );
    }

    #[test]
    fn adj_multiplier_rational_matches_table_5_27() {
        // Every Table 5-27 row, rationalised with denominator 16.
        assert_eq!(ScaleFactorAdjustment::Adj0.multiplier_rational(), (16, 16));
        assert_eq!(ScaleFactorAdjustment::Adj1.multiplier_rational(), (18, 16));
        assert_eq!(ScaleFactorAdjustment::Adj2.multiplier_rational(), (20, 16));
        assert_eq!(ScaleFactorAdjustment::Adj3.multiplier_rational(), (23, 16));

        // Each rational equals the f32 multiplier exactly (every
        // numerator is a multiple of 1/16, and 1/16 is exact in
        // IEEE-754 binary32).
        for v in [
            ScaleFactorAdjustment::Adj0,
            ScaleFactorAdjustment::Adj1,
            ScaleFactorAdjustment::Adj2,
            ScaleFactorAdjustment::Adj3,
        ] {
            let (num, den) = v.multiplier_rational();
            assert_eq!(
                v.multiplier(),
                num as f32 / den as f32,
                "rational == f32 for {v:?}"
            );
        }
    }

    #[test]
    fn decode_adj_at_byte_aligned() {
        // Four ADJ fields packed back-to-back in one byte:
        // 0b00 0b01 0b10 0b11 = 0b00_01_10_11 = 0x1B.
        let stream = [0x1B];
        let mut bit_offset = 0;
        for expected in [
            ScaleFactorAdjustment::Adj0,
            ScaleFactorAdjustment::Adj1,
            ScaleFactorAdjustment::Adj2,
            ScaleFactorAdjustment::Adj3,
        ] {
            let (v, n) = decode_adj_at(&stream, bit_offset).unwrap();
            assert_eq!(v, expected, "decode_adj_at(@{bit_offset})");
            assert_eq!(n, 2, "bits_consumed @{bit_offset}");
            bit_offset += n;
        }
        assert_eq!(bit_offset, 8);
    }

    #[test]
    fn decode_adj_at_unaligned_bit_offset() {
        // 5 leading filler bits, then the ADJ pair `0b10` (Adj2),
        // then trailing bits. Layout:
        //   bit 0..5  = 0b11111 (filler, ignored)
        //   bit 5..7  = 0b10    (ADJ)
        //   bit 7     = 0b1     (filler, ignored)
        // Combined: 0b1111_1101 = 0xFD.
        let stream = [0xFD];
        let (v, n) = decode_adj_at(&stream, 5).unwrap();
        assert_eq!(v, ScaleFactorAdjustment::Adj2);
        assert_eq!(n, 2);
    }

    #[test]
    fn decode_adj_at_crosses_byte_boundary() {
        // Place the 2-bit ADJ across the byte boundary: bit 7 of
        // byte 0 (MSB-most) carries the ADJ MSB; bit 0 of byte 1
        // carries the ADJ LSB. Pick `0b11` (Adj3).
        //   byte 0: 0b0000_0001 = 0x01 (ADJ MSB in bit 7)
        //   byte 1: 0b1000_0000 = 0x80 (ADJ LSB in bit 0)
        let stream = [0x01, 0x80];
        let (v, n) = decode_adj_at(&stream, 7).unwrap();
        assert_eq!(v, ScaleFactorAdjustment::Adj3);
        assert_eq!(n, 2);
    }

    #[test]
    fn decode_adj_at_reports_eof_when_buffer_short() {
        // Only 1 bit left after `bit_offset` → EOF on the 2-bit read.
        let stream = [0x00];
        let err = decode_adj_at(&stream, 7).unwrap_err();
        assert!(matches!(err, Error::UnexpectedEof));
    }

    #[test]
    fn adj_code_round_trips_every_value() {
        for code in 0u8..=3 {
            let v = ScaleFactorAdjustment::from_index(code);
            assert_eq!(v.code(), code);
        }
    }

    // -----------------------------------------------------------
    // §5.4.1 Table 5-28 — SSC / nSSC / PSC tests (Round 249,
    // staged PDF p.28–p.30).
    // -----------------------------------------------------------

    #[test]
    fn subsubframe_count_n_ssc_covers_every_ssc_value() {
        // SSC = 0..=3 → nSSC = 1..=4 per PDF p.29 ("nSSC = SSC + 1").
        let cases = [(0u8, 1u8), (1, 2), (2, 3), (3, 4)];
        for (raw_ssc, expected_n_ssc) in cases {
            let prefix = SubsubframeCount::new(raw_ssc, 0);
            assert_eq!(prefix.ssc, raw_ssc);
            assert_eq!(prefix.n_ssc(), expected_n_ssc);
        }
    }

    #[test]
    fn subsubframe_count_samples_per_subsubframe_normal_is_8_times_n_ssc() {
        // The 8 * nSSC quantity is the §C.2.3 / §C.2.4 / §C.2.5
        // per-subband sample stride, also referenced in this
        // crate's `sum_diff.rs` and `joint_subband.rs` doc-comments
        // as the "8 * nSSC" inner-loop bound.
        let cases = [(0u8, 8usize), (1, 16), (2, 24), (3, 32)];
        for (raw_ssc, expected_samples) in cases {
            let prefix = SubsubframeCount::new(raw_ssc, 0);
            assert_eq!(prefix.samples_per_subsubframe_normal(), expected_samples);
        }
    }

    #[test]
    fn subsubframe_count_masks_high_bits_of_ssc_and_psc() {
        // ExtractBits(2) / ExtractBits(3) semantics: only the low
        // 2 / 3 bits should reach the typed prefix.
        let prefix = SubsubframeCount::new(0b1111_1101, 0b1111_1010);
        assert_eq!(prefix.ssc, 0b01);
        assert_eq!(prefix.psc, 0b010);
        // All-ones inputs collapse to the max wire values.
        let max = SubsubframeCount::new(0xFF, 0xFF);
        assert_eq!(max.ssc, SubsubframeCount::MAX_SSC);
        assert_eq!(max.psc, SubsubframeCount::MAX_PSC);
        assert_eq!(max.n_ssc(), 4);
    }

    #[test]
    fn subsubframe_count_partial_sample_count_signals_termination_tail() {
        // PSC == 0 → no partial subsubframe at the tail of the
        // current audio subframe (PDF p.30: partial subsubframe
        // "exists only in a termination frame").
        let normal = SubsubframeCount::new(2, 0);
        assert_eq!(normal.partial_sample_count(), None);
        assert!(!normal.is_termination_tail());
        // PSC > 0 → termination tail; the returned count is the
        // partial subsubframe's sample count per active subband.
        for psc in 1u8..=7 {
            let tail = SubsubframeCount::new(2, psc);
            assert_eq!(tail.partial_sample_count(), Some(psc));
            assert!(tail.is_termination_tail());
        }
    }

    #[test]
    fn subsubframe_count_wire_bits_constant_matches_table_5_28() {
        // SSC = ExtractBits(2); PSC = ExtractBits(3); total 5 bits
        // per the first two rows of Table 5-28 (PDF p.28).
        assert_eq!(SubsubframeCount::WIRE_BITS, 5);
    }

    #[test]
    fn decode_subsubframe_count_at_byte_aligned() {
        // Five-bit prefix at bit-offset 0: pack SSC=0b10 and
        // PSC=0b011 into the top 5 bits of byte 0.
        //   bit 0..=1 (MSB-first) = SSC = 0b10
        //   bit 2..=4             = PSC = 0b011
        //   bit 5..=7             = 0b000 (zero padding)
        //   → 0b10011000 = 0x98
        let stream = [0x98];
        let (prefix, n) = decode_subsubframe_count_at(&stream, 0).unwrap();
        assert_eq!(prefix.ssc, 0b10);
        assert_eq!(prefix.psc, 0b011);
        assert_eq!(prefix.n_ssc(), 3);
        assert_eq!(n, 5);
    }

    #[test]
    fn decode_subsubframe_count_at_non_byte_aligned() {
        // Three filler bits, then SSC=0b11, PSC=0b101.
        //   bit 0..=2 = 0b111
        //   bit 3..=4 = SSC = 0b11
        //   bit 5..=7 = PSC = 0b101
        //   → 0b1111_1101 = 0xFD
        let stream = [0xFD];
        let (prefix, n) = decode_subsubframe_count_at(&stream, 3).unwrap();
        assert_eq!(prefix.ssc, 0b11);
        assert_eq!(prefix.psc, 0b101);
        assert_eq!(prefix.n_ssc(), 4);
        assert_eq!(prefix.partial_sample_count(), Some(5));
        assert_eq!(n, 5);
    }

    #[test]
    fn decode_subsubframe_count_at_crosses_byte_boundary() {
        // Place the 5-bit prefix straddling the byte boundary,
        // starting at bit-offset 5 within byte 0:
        //   byte 0: 0b0000_0010 = 0x02 → bit 5..=7 = 0b010,
        //                                  so SSC MSB=0, SSC LSB=1,
        //                                  PSC MSB=0 (bit 7).
        //   Actually: bit 5..=6 = SSC (=0b01), bit 7 = top bit of PSC.
        //   We pick SSC=0b01 and PSC=0b001 ⇒ bit 7 of byte 0 = 0,
        //   bit 0..=1 of byte 1 = 0b01.
        //   byte 0 bits = 0b0000_0010 = 0x02 (SSC LSB at bit 6,
        //                                     PSC top bit at bit 7 = 0)
        //   byte 1 bits = 0b0100_0000 = 0x40 (PSC mid bit at bit 0 = 0,
        //                                     PSC LSB at bit 1 = 1)
        // Wait: redo it. At bit-offset 5 (MSB-first counting from
        // byte 0 bit 7 = position 0), bits read are:
        //   pos 5 → byte 0 bit 2 = SSC[1] (MSB of SSC)
        //   pos 6 → byte 0 bit 1 = SSC[0] (LSB of SSC)
        //   pos 7 → byte 0 bit 0 = PSC[2] (MSB of PSC)
        //   pos 8 → byte 1 bit 7 = PSC[1]
        //   pos 9 → byte 1 bit 6 = PSC[0] (LSB of PSC)
        // For SSC=0b01 and PSC=0b001:
        //   byte 0 bit 2 = 0, byte 0 bit 1 = 1, byte 0 bit 0 = 0
        //                              → byte 0 = 0b0000_0010 = 0x02
        //   byte 1 bit 7 = 0, byte 1 bit 6 = 1
        //                              → byte 1 = 0b0100_0000 = 0x40
        let stream = [0x02, 0x40];
        let (prefix, n) = decode_subsubframe_count_at(&stream, 5).unwrap();
        assert_eq!(prefix.ssc, 0b01);
        assert_eq!(prefix.psc, 0b001);
        assert_eq!(prefix.n_ssc(), 2);
        assert_eq!(n, 5);
    }

    #[test]
    fn decode_subsubframe_count_at_reports_eof_when_buffer_short() {
        // Only 4 bits left after `bit_offset` → EOF before the
        // full 5-bit prefix can be consumed.
        let stream = [0x00];
        let err = decode_subsubframe_count_at(&stream, 4).unwrap_err();
        assert!(matches!(err, Error::UnexpectedEof));
    }

    #[test]
    fn decode_subsubframe_count_at_covers_every_ssc_psc_pair() {
        // Exhaustively pack each of the 4 * 8 = 32 (SSC, PSC)
        // combinations into a single byte at bit-offset 0
        // (SSC occupies bits 0..=1, PSC occupies bits 2..=4, low
        // 3 bits are zero padding), then decode and check.
        for ssc in 0u8..=3 {
            for psc in 0u8..=7 {
                let byte = (ssc << 6) | (psc << 3);
                let stream = [byte];
                let (prefix, n) = decode_subsubframe_count_at(&stream, 0).unwrap();
                assert_eq!(prefix.ssc, ssc);
                assert_eq!(prefix.psc, psc);
                assert_eq!(n, 5);
                assert_eq!(prefix.n_ssc(), ssc + 1);
                assert_eq!(
                    prefix.samples_per_subsubframe_normal(),
                    8 * (ssc as usize + 1)
                );
                assert_eq!(prefix.is_termination_tail(), psc != 0);
            }
        }
    }

    #[test]
    fn decode_join_scale_huffman_zero_symbol_is_unity() {
        // The SA129 Huffman table (A5) codes symbol 0 as a 1-bit `0`.
        // After the +64 bias that lands on the §D.3 unity entry (1.0).
        let stream = pack_codes(&[(0, 1)]);
        let (factor, biased, bits) =
            decode_join_scale_at(&stream, 0, ScalesCodebook::Sa129).unwrap();
        assert_eq!(biased, 64);
        assert_eq!(factor, 1.0);
        assert_eq!(bits, 1);
    }

    #[test]
    fn decode_join_scale_huffman_walks_every_a5_symbol() {
        // Each SA129 (A5) symbol biases by +64 and must land on a valid
        // §D.3 entry (all of {-2,-1,0,1,2} + 64 are inside 0..=128).
        for &(symbol, len, code) in TABLE_A5 {
            let stream = pack_codes(&[(code, len)]);
            let (factor, biased, _) =
                decode_join_scale_at(&stream, 0, ScalesCodebook::Sa129).unwrap();
            assert_eq!(biased, symbol as i32 + 64);
            assert_eq!(
                factor,
                crate::JOIN_SCALE_FACTOR[(symbol as i32 + 64) as usize]
            );
        }
    }

    #[test]
    fn decode_join_scale_linear6_reads_raw_index() {
        // Linear-6-bit: a raw 6-bit absolute index. Index 0 biases to
        // 64 (unity); index 63 biases to 127 (a high §D.3 factor).
        let stream = pack_codes(&[(0, 6)]);
        let (factor, biased, bits) =
            decode_join_scale_at(&stream, 0, ScalesCodebook::Linear6Bit).unwrap();
        assert_eq!(biased, 64);
        assert_eq!(factor, 1.0);
        assert_eq!(bits, 6);

        let stream = pack_codes(&[(63, 6)]);
        let (factor, biased, _) =
            decode_join_scale_at(&stream, 0, ScalesCodebook::Linear6Bit).unwrap();
        assert_eq!(biased, 127);
        assert_eq!(factor, crate::JOIN_SCALE_FACTOR[127]);
    }

    #[test]
    fn decode_join_scale_linear7_out_of_range_rejected() {
        // Linear-7-bit index 65 biases to 129, which is outside the
        // §D.3 table (0..=128); the decode must reject it rather than
        // index out of bounds.
        let stream = pack_codes(&[(65, 7)]);
        let err = decode_join_scale_at(&stream, 0, ScalesCodebook::Linear7Bit).unwrap_err();
        assert!(matches!(
            err,
            Error::InvalidSideInfo {
                field: "JOIN_SCALES",
                value: 129
            }
        ));
    }
}

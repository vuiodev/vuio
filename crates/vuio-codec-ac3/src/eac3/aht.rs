//! Adaptive Hybrid Transform (AHT) decode helpers — A/52:2018 Annex E §3.4.
//!
//! AHT layers a non-overlapped, non-windowed DCT-II of length 6 on top
//! of the standard 256-coefficient MDCT for blocks where the encoder
//! decides the signal is stationary enough that the coding gain of a
//! longer transform beats the time-domain smearing it would normally
//! incur. The AHT path lives entirely inside the §7.3 mantissa unpack
//! step: instead of reading 256 mantissas per audblk, the decoder reads
//! 6×N mantissas (one per (block, bin) pair) for the **first** AHT-active
//! audblk of each frame, dequantises them with a high-efficiency
//! quantiser table (`hebap`), and then inverse-DCT-II's per bin to
//! recover the per-block MDCT coefficients §3.4.5.
//!
//! ## Per-spec eligibility
//!
//! AHT is only available when:
//!
//! * `numblkscod == 0x3` (6 blocks per syncframe) — the AHT transform
//!   length is hard-coded to 6.
//! * `ahte == 1` is set in `audfrm()`.
//! * For each AHT-active channel, `nchregs[ch] == 1` (i.e. exponents
//!   are transmitted exactly once per syncframe — block 0 carries a
//!   non-`REUSE` strategy and blocks 1..5 all reuse). Same rule for
//!   `ncplregs`/`nlferegs` on coupling and LFE channels.
//!
//! ## Quantiser stack (§3.4.4)
//!
//! Once `hebap` is computed for a given mantissa bin (§3.4.3.1 — same
//! masking model as base AC-3 with a finer 64-entry pointer table),
//! each of the 6 cross-block coefficients in that bin is quantised by
//! one of three modes:
//!
//! * **`hebap == 0`** — bin contributes no bits (zero-mantissa).
//! * **`1 ≤ hebap ≤ 7`** — vector-quantised: a single 2..9-bit codeword
//!   indexes into a 6-D codebook (Tables E4.1..E4.7 in
//!   [`super::tables::aht_codebooks`]). The codebook entry IS the
//!   6-tuple of dequantised values.
//! * **`8 ≤ hebap ≤ 19`** — symmetric scalar quantiser (with optional
//!   gain-adaptive step-size scaling per the per-frame `gaqmod`). Each
//!   of the 6 mantissas reads its own short codeword; if the GAQ tag
//!   (full-scale negative) is detected, an extra "large mantissa"
//!   codeword follows.
//!
//! ## Cross-block IDCT (§3.4.5)
//!
//! After all 6 mantissas per AHT bin are dequantised, the spec's
//! inverse DCT-II reconstructs the per-block MDCT spectrum value:
//!
//! ```text
//!     C(k, m) = 2 · Σ_{j=0..5}  R(j) · X(k, j) · cos[ j·(2m+1)·π / 12 ]
//!     R(j)   = 1                  for j != 0
//!            = 1/√2               for j == 0
//! ```
//!
//! Then the standard `coeff = mantissa · 2^(-exp)` reconstruction
//! converts each per-block C(k, m) into the regular fbw transform
//! coefficient slot, ready for IMDCT + overlap-add via the existing
//! [`crate::audblk::dsp_block`] path.
//!
//! ## Round-6 scope (this commit)
//!
//! * VQ codebooks E4.1..E4.7 transcribed (956 entries × 6 i16).
//! * `hebap` pointer table (Table E3.1) + quantiser-bit table (E3.2).
//! * GAQ gain-element decode (Table E3.4) + dead-zone large-mantissa
//!   remap (Table E3.6).
//! * fbw-channel AHT mantissa unpack + cross-block IDCT.
//! * Coupling / LFE AHT (`cplahtinu`, `lfeahtinu`) — wired through
//!   the per-channel iteration (LFE round 113, coupling round 117) so
//!   6-block-coupling and AHT-LFE syncframes decode; the helper routines
//!   here (`hebap_from_address`, `fill_gaqbin`, `gaq_sections`,
//!   `read_gaq_gains`, `vq_lookup`, `read_scalar_aht_mantissas`,
//!   `idct_ii_6`) are channel-agnostic and serve all three paths
//!   unchanged. No corpus fixture currently exercises coupling/LFE AHT,
//!   so those paths are covered by unit tests in `super::dsp`.
//!
//! Symbol naming follows the spec: `hebap`, `chgaqmod`, `chgaqgain`,
//! `chgaqbin`, `pre_chmant` and the `[k][j]` (bin, AHT-block) ordering
//! are kept literal so the implementation can be cross-referenced
//! against §3.4 paragraph by paragraph.

use std::f32::consts::PI;

use oxideav_core::bits::BitReader;
use oxideav_core::Result;

use super::tables::aht_codebooks::{
    VQ_HEBAP1, VQ_HEBAP2, VQ_HEBAP3, VQ_HEBAP4, VQ_HEBAP5, VQ_HEBAP6, VQ_HEBAP7,
};

/// AHT works on exactly six audio blocks per syncframe (§3.4.1).
pub const AHT_BLOCKS: usize = 6;

/// Table E3.1 — high-efficiency bit-allocation pointer lookup.
/// Indexed by a 6-bit address derived from `(psd - mask) >> 5` clamped
/// to 0..63. Table values are 5-bit `hebap` codes per §3.4.3.1.
pub const HEBAPTAB: [u8; 64] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 8, 8, 8, 9, 9, 9, 10, 10, 10, 10, 11, 11, 11, 11, 12, 12, 12, 12,
    13, 13, 13, 13, 14, 14, 14, 14, 15, 15, 15, 15, 16, 16, 16, 16, 17, 17, 17, 17, 18, 18, 18, 18,
    18, 18, 18, 18, 19, 19, 19, 19, 19, 19, 19, 19, 19,
];

/// Table E3.2 — number of mantissa bits per `hebap` index for the
/// scalar/GAQ regime (`hebap >= 8`). Indices 0..7 are zero (VQ regime
/// has its own bit-count baked into the codebook size).
pub const HEBAP_MANT_BITS: [u8; 20] = [
    0, 0, 0, 0, 0, 0, 0, 0, // 0..7  → VQ or zero
    3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14, 16,
];

/// Number of bits per VQ codeword for `hebap` 1..7 (Table E3.2 column
/// "Mantissa Bits" rounded up over 6 values: 2/6 → 2 bits, 3/6 → 3 bits,
/// etc.). Index 0 unused (signals zero-mantissa).
pub const VQ_BITS: [u8; 8] = [0, 2, 3, 4, 5, 7, 8, 9];

/// Decode a `hebap` value from `(psd, mask)` using the §3.4.3.1
/// pseudo-code. Mirrors [`crate::audblk::run_bit_allocation`]'s tail
/// step but emits `hebap` (via [`HEBAPTAB`]) instead of the 5-bit
/// `bap`.
#[inline]
pub fn hebap_from_address(psd: i16, mask_after_floor: i32) -> u8 {
    let addr = (((psd as i32) - mask_after_floor) >> 5).clamp(0, 63) as usize;
    HEBAPTAB[addr]
}

/// `endbap` for the GAQ-active range — mode 0/1 cap at 12 (i.e. apply
/// gains only for `8 <= hebap < 12`); mode 2/3 cap at 17. Per §3.4.2
/// pseudo-code paragraph that builds `chgaqbin[ch][bin]`.
#[inline]
pub fn endbap_for_gaqmod(gaqmod: u8) -> u8 {
    if gaqmod < 2 {
        12
    } else {
        17
    }
}

/// Compute `chgaqbin[ch][bin]` per the §3.4.2 pseudo-code:
///
/// * `+1` — bin is GAQ-coded, gain word follows for it.
/// * `-1` — bin is large-only (no GAQ gain word, falls through to the
///   fixed-quantiser regime).
/// * `0` — bin not in the GAQ regime.
///
/// `gaqmod` is the per-channel `chgaqmod` (or `cplgaqmod` /
/// `lfegaqmod`); pass `0` to disable GAQ for the entire bin range.
pub fn fill_gaqbin(hebap: &[u8], gaqmod: u8, gaqbin: &mut [i8]) -> usize {
    let endbap = endbap_for_gaqmod(gaqmod);
    let mut active = 0usize;
    for (i, &h) in hebap.iter().enumerate() {
        if i >= gaqbin.len() {
            break;
        }
        if h > 7 && h < endbap {
            gaqbin[i] = 1;
            active += 1;
        } else if h >= endbap {
            gaqbin[i] = -1;
        } else {
            gaqbin[i] = 0;
        }
    }
    active
}

/// Number of GAQ gain words transmitted for a channel given its
/// `gaqmod` and the active-bin count returned by [`fill_gaqbin`]. Per
/// the §3.4.2 final pseudo-code chunk (chgaqsections / cplgaqsections /
/// lfegaqsections).
pub fn gaq_sections(gaqmod: u8, active_gaqbins: usize) -> usize {
    match gaqmod {
        0 => 0,
        1 | 2 => active_gaqbins,
        3 => active_gaqbins.div_ceil(3),
        _ => 0,
    }
}

/// Unpack `nsections` GAQ gain words from the bit stream into a flat
/// `[u8; nbins]` array of mapped values 0..2 (per Table E3.4).
///
/// * `gaqmod == 1` or `2` — 1 bit per active bin, mapped value = bit
///   (0 → Gk=1, 1 → Gk=2 or 4).
/// * `gaqmod == 3` — 5-bit composite triplet, decoded via
///   `M1 = grpgain/9`, `M2 = (grpgain%9)/3`, `M3 = grpgain%9%3`.
///
/// Returns the per-bin mapped values aligned to the GAQ-active bins
/// (in order). The caller threads them onto `chgaqbin[bin] == 1`
/// positions as it walks the AHT mantissa loop.
pub fn read_gaq_gains(
    br: &mut BitReader<'_>,
    gaqmod: u8,
    nsections: usize,
    out: &mut [u8],
) -> Result<usize> {
    if nsections == 0 || gaqmod == 0 {
        return Ok(0);
    }
    let mut written = 0usize;
    for _ in 0..nsections {
        match gaqmod {
            1 | 2 => {
                if written >= out.len() {
                    break;
                }
                let bit = br.read_u32(1)? as u8;
                out[written] = bit;
                written += 1;
            }
            3 => {
                let grp = br.read_u32(5)?;
                // Triplet decode per §3.4.4.2 pseudo-code.
                let m1 = (grp / 9) as u8;
                let m2 = ((grp % 9) / 3) as u8;
                let m3 = ((grp % 9) % 3) as u8;
                for v in [m1, m2, m3] {
                    if written < out.len() {
                        out[written] = v.min(2);
                        written += 1;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(written)
}

/// Map GAQ gain code (0/1/2 from Table E3.4) to the linear gain factor
/// `Gk` (1 / 2 / 4). Used both in the bit-stream reader (to know how
/// many tag bits to peek for) and in the dequantiser (to invert the
/// encoder's amplification).
#[inline]
pub fn gaq_gain_value(code: u8, gaqmod: u8) -> u8 {
    match gaqmod {
        1 => match code {
            0 => 1,
            _ => 2, // mode 1: Gk ∈ {1, 2}
        },
        2 => match code {
            0 => 1,
            _ => 4, // mode 2: Gk ∈ {1, 4}
        },
        3 => match code {
            0 => 1,
            1 => 2,
            _ => 4, // mode 3: Gk ∈ {1, 2, 4}
        },
        _ => 1,
    }
}

/// Look up the 6-element VQ codebook entry for a given `hebap` (1..=7)
/// and `index`. Returns the entry as f32 normalised into `(-1.0, 1.0)`
/// by dividing by `32768.0` (i.e. interpreting the 16-bit value as a
/// signed Q15 fractional).
pub fn vq_lookup(hebap: u8, index: usize) -> [f32; 6] {
    let raw: &[i16; 6] = match hebap {
        1 => &VQ_HEBAP1[index & (VQ_HEBAP1.len() - 1)],
        2 => &VQ_HEBAP2[index & (VQ_HEBAP2.len() - 1)],
        3 => &VQ_HEBAP3[index & (VQ_HEBAP3.len() - 1)],
        4 => &VQ_HEBAP4[index & (VQ_HEBAP4.len() - 1)],
        5 => &VQ_HEBAP5[index & (VQ_HEBAP5.len() - 1)],
        6 => &VQ_HEBAP6[index & (VQ_HEBAP6.len() - 1)],
        7 => &VQ_HEBAP7[index & (VQ_HEBAP7.len() - 1)],
        _ => &[0; 6],
    };
    let mut out = [0.0f32; 6];
    for i in 0..6 {
        out[i] = raw[i] as f32 / 32768.0;
    }
    out
}

/// Table E3.6 — large-mantissa inverse-quantisation (remapping)
/// constants, transcribed literally from A/52:2018 Annex E. One row
/// per `hebap` in `8..=19`. Each `(a, b_pos, b_neg)` triple is a set
/// of 16-bit signed two's-complement Q15 fractions for the
/// `y = x + a·x + b` post-process (§3.4.4.2), with `b` selected by
/// the sign of the codeword `x` (`b_pos` for `x >= 0`, `b_neg` for
/// `x < 0`).
///
/// Columns: `Gk = 1` (applies to ALL `gaqmod == 0` / gain-1 scalar
/// quantisers; `b == 0` in the spec table so only `a` is stored),
/// `Gk = 2` and `Gk = 4` (large-mantissa dead-zone remaps). The spec
/// marks `Gk = 2` / `Gk = 4` rows for `hebap >= 17` as N/A — those
/// hebaps sit outside every GAQ-active range (Table E3.3), so the
/// rows are unreachable; they are stored as zeros.
struct GaqRemap {
    /// `Gk = 1` column: `a` (Q15); `b` is 0x0000 for every row.
    a_g1: i16,
    /// `Gk = 2` column: `(a, b for x>=0, b for x<0)` (Q15).
    g2: (i16, i16, i16),
    /// `Gk = 4` column: `(a, b for x>=0, b for x<0)` (Q15).
    g4: (i16, i16, i16),
}

/// Rows indexed by `hebap - 8` (hebap 8..=19).
#[rustfmt::skip]
const GAQ_REMAP: [GaqRemap; 12] = [
    /*  8 */ GaqRemap { a_g1: 0x1249, g2: (0xd555u16 as i16, 0x4000, 0xeaabu16 as i16), g4: (0xedb7u16 as i16, 0x2000, 0xfb6eu16 as i16) },
    /*  9 */ GaqRemap { a_g1: 0x0889, g2: (0xc925u16 as i16, 0x4000, 0xd249u16 as i16), g4: (0xe666u16 as i16, 0x2000, 0xeccdu16 as i16) },
    /* 10 */ GaqRemap { a_g1: 0x0421, g2: (0xc444u16 as i16, 0x4000, 0xc889u16 as i16), g4: (0xe319u16 as i16, 0x2000, 0xe632u16 as i16) },
    /* 11 */ GaqRemap { a_g1: 0x0208, g2: (0xc211u16 as i16, 0x4000, 0xc421u16 as i16), g4: (0xe186u16 as i16, 0x2000, 0xe30cu16 as i16) },
    /* 12 */ GaqRemap { a_g1: 0x0102, g2: (0xc104u16 as i16, 0x4000, 0xc208u16 as i16), g4: (0xe0c2u16 as i16, 0x2000, 0xe183u16 as i16) },
    /* 13 */ GaqRemap { a_g1: 0x0081, g2: (0xc081u16 as i16, 0x4000, 0xc102u16 as i16), g4: (0xe060u16 as i16, 0x2000, 0xe0c1u16 as i16) },
    /* 14 */ GaqRemap { a_g1: 0x0040, g2: (0xc040u16 as i16, 0x4000, 0xc081u16 as i16), g4: (0xe030u16 as i16, 0x2000, 0xe060u16 as i16) },
    /* 15 */ GaqRemap { a_g1: 0x0020, g2: (0xc020u16 as i16, 0x4000, 0xc040u16 as i16), g4: (0xe018u16 as i16, 0x2000, 0xe030u16 as i16) },
    /* 16 */ GaqRemap { a_g1: 0x0010, g2: (0xc010u16 as i16, 0x4000, 0xc020u16 as i16), g4: (0xe00cu16 as i16, 0x2000, 0xe018u16 as i16) },
    /* 17 */ GaqRemap { a_g1: 0x0008, g2: (0, 0, 0), g4: (0, 0, 0) },
    /* 18 */ GaqRemap { a_g1: 0x0002, g2: (0, 0, 0), g4: (0, 0, 0) },
    /* 19 */ GaqRemap { a_g1: 0x0000, g2: (0, 0, 0), g4: (0, 0, 0) },
];

/// Q15 fraction → f32.
#[inline]
fn q15(v: i16) -> f32 {
    v as f32 / 32768.0
}

/// §3.4.4.2 / Table E3.6 post-process `y = x + a·x + b` for a
/// large-mantissa (or `Gk = 1` symmetric) codeword. `x` is the
/// codeword interpreted as a signed two's-complement fraction.
///
/// `pub(crate)` so the encoder side ([`super::ahtenc`]) can invert the
/// EXACT mapping (quantise by minimising `|gaq_remap(code) - target|`)
/// instead of duplicating the constants.
#[inline]
pub(crate) fn gaq_remap(hebap: u8, gk: u8, x: f32) -> f32 {
    let row = &GAQ_REMAP[(hebap as usize - 8).min(11)];
    let (a, b) = match gk {
        1 => (row.a_g1, 0i16),
        2 => (row.g2.0, if x >= 0.0 { row.g2.1 } else { row.g2.2 }),
        _ => (row.g4.0, if x >= 0.0 { row.g4.1 } else { row.g4.2 }),
    };
    x + q15(a) * x + q15(b)
}

/// Read 6 mantissas for a single AHT bin with `hebap` in the
/// scalar/GAQ regime (`hebap >= 8`), per §3.4.4.2 / Tables E3.5-E3.6.
///
/// When `gaqbin == 1` the encoder may have applied a gain (signalled
/// via the per-section gain word `gain_code`):
///
/// * **`Gk = 1`** (or no GAQ for this bin) — a single `m`-bit
///   two's-complement codeword per mantissa, post-processed with the
///   Table E3.6 `Gk = 1` column (`y = x·(1 + a)`, symmetric
///   `2^m - 1`-level quantiser of step `2 / (2^m - 1)`).
/// * **`Gk = 2`** (mode 1/3) — an `(m-1)`-bit small codeword of step
///   `1 / 2^(m-1)`; the full-scale-negative tag announces an
///   `(m-1)`-bit large codeword remapped via the `Gk = 2` column
///   (dead-zone quantiser, `2^(m-1)` points of step
///   `1 / (2^(m-1) - 1)`). The `(m-1)` LARGE width is easy to misread
///   as `m` off the printed Table E3.5 layout — the clarification
///   (with the `2^(m-1)` reconstruction-point cross-check) is
///   codified as `docs/audio/ac3/ac3-errata.md` entry **E2**; pinned
///   by `tests::scalar_gk2_small_and_large`.
/// * **`Gk = 4`** (mode 2/3) — an `(m-2)`-bit small codeword of step
///   `1 / 2^(m-1)`; the tag announces an `m`-bit large codeword
///   remapped via the `Gk = 4` column (step `3 / (2^(m+1) - 2)`).
///
/// Returns the 6 dequantised mantissas in `(-1, 1)` normalised range.
pub fn read_scalar_aht_mantissas(
    br: &mut BitReader<'_>,
    hebap: u8,
    gaqmod: u8,
    gaqbin: i8,
    gain_code: u8,
    out: &mut [f32; 6],
) -> Result<()> {
    if hebap < 8 {
        for v in out.iter_mut() {
            *v = 0.0;
        }
        return Ok(());
    }
    let m = HEBAP_MANT_BITS[hebap as usize] as u32;
    if m == 0 {
        for v in out.iter_mut() {
            *v = 0.0;
        }
        return Ok(());
    }

    // gain == 1 (or no GAQ for this bin) → fixed quantiser, m bits.
    let gk = if gaqbin == 1 {
        gaq_gain_value(gain_code, gaqmod)
    } else {
        1
    };

    let scale_full = 1.0f32 / ((1u32 << (m - 1)) as f32); // 1 / 2^(m-1)

    for sample in out.iter_mut() {
        match gk {
            1 => {
                // Symmetric quantiser: read m bits, sign-extend,
                // interpret as a Q(m-1) fraction, stretch by (1 + a)
                // to the Table E3.5 step of 2/(2^m - 1).
                let raw = br.read_u32(m)? as i32;
                let signed = (raw << (32 - m)) >> (32 - m);
                *sample = gaq_remap(hebap, 1, signed as f32 * scale_full);
            }
            2 => {
                // Mode 1 / 3 with Gk=2: (m-1)-bit small value; the
                // full-scale-negative tag announces an (m-1)-bit
                // large value (Table E3.5: 2^(m-1) output points).
                let small_bits = m - 1;
                let raw_s = br.read_u32(small_bits)? as i32;
                let small_signed = (raw_s << (32 - small_bits)) >> (32 - small_bits);
                let small_min = -(1i32 << (small_bits - 1));
                if small_signed == small_min {
                    // Tag — (m-1)-bit large codeword, Q(m-2) fraction.
                    let raw_l = br.read_u32(small_bits)? as i32;
                    let large_signed = (raw_l << (32 - small_bits)) >> (32 - small_bits);
                    let x = large_signed as f32 / ((1u32 << (m - 2)) as f32);
                    *sample = gaq_remap(hebap, 2, x);
                } else {
                    // Small mantissa: the encoder amplified by Gk=2
                    // and coded with one fewer bit, so the composed
                    // step is 1/2^(m-1) (Table E3.5).
                    *sample = small_signed as f32 * scale_full;
                }
            }
            4 => {
                // Mode 2 / 3 with Gk=4: (m-2)-bit small value; the
                // tag announces an m-bit large value.
                let small_bits = m - 2;
                let raw_s = br.read_u32(small_bits)? as i32;
                let small_signed = (raw_s << (32 - small_bits)) >> (32 - small_bits);
                let small_min = -(1i32 << (small_bits - 1));
                if small_signed == small_min {
                    let raw_l = br.read_u32(m)? as i32;
                    let large_signed = (raw_l << (32 - m)) >> (32 - m);
                    let x = large_signed as f32 * scale_full;
                    *sample = gaq_remap(hebap, 4, x);
                } else {
                    // Encoder amplified by Gk=4, coded with two fewer
                    // bits; composed step 1/2^(m-1) (Table E3.5).
                    *sample = small_signed as f32 * scale_full;
                }
            }
            _ => {
                // Unknown gain → zero.
                *sample = 0.0;
            }
        }
    }
    Ok(())
}

/// Apply the §3.4.5 inverse DCT-II to the 6 AHT-domain coefficients
/// `x[0..6]` to reconstruct the per-block MDCT spectrum values
/// `c[0..6]`.
///
/// `c(m) = √2 · Σ_{j=0..5} R(j) · x(j) · cos[ j·(2m+1)·π / 12 ]`
/// where `R(0) = 1/√2` and `R(j) = 1` for `j != 0`.
///
/// **Deviation from the printed formula** — the A/52:2018 §3.4.5 text
/// shows a leading constant of `2`, but black-box cross-validation
/// against an independent production decoder shows the deployed
/// convention is `√2` (globally — pure-DC and modulated fixtures both
/// fit `ours(2·Σ) = external · √2` with ~89 dB residual). With `√2`
/// the DC basis weight is exactly 1 (`√2 · R(0) = 1`), i.e. a
/// constant cross-block signal reconstructs unchanged, which is the
/// natural quantiser-range convention. We follow the deployed
/// constant; the printed `2` is an erratum, codified with the fitted
/// evidence as `docs/audio/ac3/ac3-errata.md` entry **E1** (the
/// `R_0 = 1/√2` corollary keeping the DC gain at 1 is recorded
/// there). Pinned by `tests::idct_inverse_of_dct_constants`.
pub fn idct_ii_6(x: [f32; 6]) -> [f32; 6] {
    let mut c = [0.0f32; 6];
    let r0 = std::f32::consts::FRAC_1_SQRT_2;
    for m in 0..6 {
        let mut acc = 0.0f32;
        for j in 0..6 {
            let r = if j == 0 { r0 } else { 1.0 };
            let theta = (j as f32) * ((2 * m + 1) as f32) * PI / 12.0;
            acc += r * x[j] * theta.cos();
        }
        c[m] = std::f32::consts::SQRT_2 * acc;
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hebaptab_has_expected_extremes() {
        assert_eq!(HEBAPTAB[0], 0);
        assert_eq!(HEBAPTAB[63], 19);
    }

    #[test]
    fn vq_lookup_in_range() {
        // VQ_HEBAP1[0] = [7167, 4739, 1106, 4269, 10412, 4820] / 32768.
        let v = vq_lookup(1, 0);
        assert!((v[0] - 7167.0 / 32768.0).abs() < 1e-6);
        assert!(v.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn idct_inverse_of_dct_constants() {
        // Constant input → DC output only at m=0..5 should equal
        // √2 · R(0) · x(0) · cos(0) = 1 for x = (1, 0, 0, 0, 0, 0):
        // the deployed convention's DC basis weight is exactly 1.
        let c = idct_ii_6([1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        for v in c.iter() {
            assert!((v - 1.0).abs() < 1e-5, "got {v}, expected 1.0");
        }
    }

    use oxideav_core::bits::BitWriter;

    fn bits_of(f: impl FnOnce(&mut BitWriter)) -> Vec<u8> {
        let mut bw = BitWriter::with_capacity(16);
        f(&mut bw);
        bw.write_u32(0, 32); // tail padding so short reads never starve
        bw.into_bytes()
    }

    /// Table E3.5 `Gk = 1` column: hebap 8 (m = 3) is a 7-level
    /// symmetric quantiser of step 2/7 — codeword 3 (`011`)
    /// reconstructs to 6/7 via the Table E3.6 stretch `y = x·(1 + a)`.
    #[test]
    fn scalar_gk1_symmetric_step() {
        let bytes = bits_of(|bw| bw.write_u32(0b011, 3));
        let mut br = BitReader::new(&bytes);
        let mut out = [0.0f32; 6];
        // Consume ONE mantissa then stop (remaining reads eat padding).
        read_scalar_aht_mantissas(&mut br, 8, 0, 0, 0, &mut out).unwrap();
        assert!(
            (out[0] - 6.0 / 7.0).abs() < 1e-4,
            "Gk=1 hebap=8 code 3 → 6/7, got {}",
            out[0]
        );
        // Codeword 0 → exactly 0 (mid-tread).
        assert_eq!(out[1], 0.0);
    }

    /// Table E3.5 `Gk = 2` column, hebap 8 (m = 3): a 2-bit small
    /// codeword of step 1/4, with the full-scale-negative tag (`10`)
    /// announcing a 2-bit large codeword remapped by Table E3.6 to the
    /// dead-zone points {±1/2, ±5/6} (step 1/3).
    #[test]
    fn scalar_gk2_small_and_large() {
        let bytes = bits_of(|bw| {
            bw.write_u32(0b01, 2); // small = +1 → 1/4
            bw.write_u32(0b10, 2); // tag
            bw.write_u32(0b01, 2); // large = +1 → x=1/2 → 5/6
            bw.write_u32(0b10, 2); // tag
            bw.write_u32(0b00, 2); // large = 0 → x=0 → 1/2
            bw.write_u32(0b10, 2); // tag
            bw.write_u32(0b10, 2); // large = -2 → x=-1 → -5/6
        });
        let mut br = BitReader::new(&bytes);
        let mut out = [0.0f32; 6];
        // gaqmod=1, gaqbin=1, gain code 1 → Gk=2.
        read_scalar_aht_mantissas(&mut br, 8, 1, 1, 1, &mut out).unwrap();
        assert!((out[0] - 0.25).abs() < 1e-6, "small: got {}", out[0]);
        assert!(
            (out[1] - 5.0 / 6.0).abs() < 1e-4,
            "large +1: got {}",
            out[1]
        );
        assert!((out[2] - 0.5).abs() < 1e-4, "large 0: got {}", out[2]);
        assert!(
            (out[3] + 5.0 / 6.0).abs() < 1e-4,
            "large -2: got {}",
            out[3]
        );
    }

    /// Table E3.5 `Gk = 4` column, hebap 9 (m = 4): 2-bit small of
    /// step 1/8; tag announces an m-bit (4-bit) large codeword of
    /// step 3/(2^5 - 2) = 0.1 anchored at ±1/4.
    #[test]
    fn scalar_gk4_large_step() {
        let bytes = bits_of(|bw| {
            bw.write_u32(0b10, 2); // tag (small_bits = m-2 = 2)
            bw.write_u32(0b0000, 4); // large = 0 → 0.25
            bw.write_u32(0b10, 2); // tag
            bw.write_u32(0b0010, 4); // large = +2 → 0.45
            bw.write_u32(0b01, 2); // small = +1 → 1/8
        });
        let mut br = BitReader::new(&bytes);
        let mut out = [0.0f32; 6];
        // gaqmod=2, gaqbin=1, gain code 1 → Gk=4.
        read_scalar_aht_mantissas(&mut br, 9, 2, 1, 1, &mut out).unwrap();
        assert!((out[0] - 0.25).abs() < 1e-4, "large 0: got {}", out[0]);
        assert!((out[1] - 0.45).abs() < 1e-4, "large +2: got {}", out[1]);
        assert!((out[2] - 0.125).abs() < 1e-6, "small: got {}", out[2]);
    }

    /// The Gk = 2 escape consumes (m-1) + (m-1) bits — the large
    /// codeword is `m-1` bits per Table E3.5 (2^(m-1) output points),
    /// NOT m bits.
    #[test]
    fn scalar_gk2_escape_bit_length() {
        let bytes = bits_of(|bw| {
            for _ in 0..6 {
                bw.write_u32(0b100, 3); // tag (m-1 = 3 bits for hebap 9)
                bw.write_u32(0b001, 3); // large (m-1 = 3 bits)
            }
        });
        let mut br = BitReader::new(&bytes);
        let mut out = [0.0f32; 6];
        read_scalar_aht_mantissas(&mut br, 9, 1, 1, 1, &mut out).unwrap();
        assert_eq!(br.bit_position(), 6 * 6, "6 escapes × (3 tag + 3 large)");
    }

    #[test]
    fn gaq_remap_table_row_consistency() {
        // Gk=1 `a` constants track 1/(2^m - 1) (Q15, spec-rounded).
        for hebap in 8u8..=19 {
            let m = HEBAP_MANT_BITS[hebap as usize] as i32;
            let expect = 32768.0 / ((1i64 << m) - 1) as f64;
            let got = GAQ_REMAP[hebap as usize - 8].a_g1 as f64;
            assert!(
                (got - expect).abs() <= 1.0,
                "hebap {hebap}: a_g1 {got} vs 1/(2^m-1) {expect:.2}"
            );
        }
    }

    #[test]
    fn gaq_sections_matches_spec() {
        assert_eq!(gaq_sections(0, 100), 0);
        assert_eq!(gaq_sections(1, 7), 7);
        assert_eq!(gaq_sections(2, 3), 3);
        assert_eq!(gaq_sections(3, 9), 3);
        assert_eq!(gaq_sections(3, 10), 4);
    }
}

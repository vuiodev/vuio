//! Encoder-side Adaptive Hybrid Transform — A/52:2018 Annex E §3.4,
//! encode direction.
//!
//! Mirrors the decode helpers in [`super::aht`] tool for tool:
//!
//! * **Forward DCT-II** ([`dct_ii_6`]) — the exact inverse of the
//!   §3.4.5 IDCT ([`super::aht::idct_ii_6`]): per spectral bin, the 6
//!   per-block mantissas `C(k, m)` become 6 AHT-domain coefficients
//!   `X(k, j)` that the quantiser stack codes once per frame.
//! * **Vector quantisation** ([`vq_search`]) — §3.4.4.1: for
//!   `1 <= hebap <= 7` the encoder "selects the best vector to
//!   transmit ... by locating the vector which minimizes the Euclidean
//!   distance between the actual mantissa vector and the table
//!   vector".
//! * **Gain-adaptive quantisation** ([`plan_aht_channel`] /
//!   [`write_aht_channel`]) — §3.4.4.2: per GAQ-eligible bin the
//!   encoder picks the gain `Gk ∈ {1, 2, 4}` (as allowed by the
//!   per-channel `gaqmod`, Table E3.3) that minimises the bit cost of
//!   the 6 codewords; small mantissas ride the shortened `m-1`/`m-2`
//!   bit codewords, large ones pay the full-scale-negative tag plus a
//!   Table E3.5 large codeword. The per-channel `gaqmod` itself is
//!   chosen by exact bit-count comparison over all four modes
//!   (including the mode-3 composite 5-bit gain triplets).
//!
//! Every quantiser decision is made against the decoder's own
//! dequantisation map ([`super::aht::gaq_remap`], the literal Table
//! E3.6 constants), so the encode→decode round trip reconstructs the
//! nearest representable level by construction.

use oxideav_core::bits::BitWriter;

use super::aht::{endbap_for_gaqmod, gaq_remap, HEBAP_MANT_BITS, VQ_BITS};
use super::tables::aht_codebooks::{
    VQ_HEBAP1, VQ_HEBAP2, VQ_HEBAP3, VQ_HEBAP4, VQ_HEBAP5, VQ_HEBAP6, VQ_HEBAP7,
};
use crate::audblk::N_COEFFS;
use crate::encoder::{compute_bap_table, BitAllocParams, DbaPlan};

/// Forward DCT-II of length 6 — the inverse of the §3.4.5 transform
/// as deployed (see [`super::aht::idct_ii_6`]: leading constant `√2`,
/// black-box-validated; the printed `2` is an erratum, codified as
/// `docs/audio/ac3/ac3-errata.md` entry E1).
///
/// The IDCT is
/// `C(m) = √2 · Σ_j R(j) · X(j) · cos[j·(2m+1)·π/12]` with
/// `R(0) = 1/√2`, `R(j≠0) = 1`. Orthogonality of the DCT basis over
/// 6 points (`Σ_m cos²[j·(2m+1)·π/12] = 3` for `j ≥ 1`, `= 6` for
/// `j = 0`) inverts it as:
///
/// ```text
///     X(0) = (1 / 6)  · Σ_m C(m)
///     X(j) = (√2 / 6) · Σ_m C(m) · cos[j·(2m+1)·π/12]   (j ≥ 1)
/// ```
pub fn dct_ii_6(c: &[f32; 6]) -> [f32; 6] {
    use std::f32::consts::{PI, SQRT_2};
    let mut x = [0.0f32; 6];
    let sum: f32 = c.iter().sum();
    x[0] = sum / 6.0;
    for (j, xj) in x.iter_mut().enumerate().skip(1) {
        let mut acc = 0.0f32;
        for (m, &cm) in c.iter().enumerate() {
            let theta = (j as f32) * ((2 * m + 1) as f32) * PI / 12.0;
            acc += cm * theta.cos();
        }
        *xj = acc * SQRT_2 / 6.0;
    }
    x
}

/// §3.4.4.1 vector-quantiser search: return the index of the codebook
/// entry (Tables E4.1..E4.7 for `hebap` 1..=7) with minimum Euclidean
/// distance to `x`. Exhaustive — the largest book (hebap 7) has 512
/// entries.
pub fn vq_search(hebap: u8, x: &[f32; 6]) -> usize {
    let book: &[[i16; 6]] = match hebap {
        1 => &VQ_HEBAP1[..],
        2 => &VQ_HEBAP2[..],
        3 => &VQ_HEBAP3[..],
        4 => &VQ_HEBAP4[..],
        5 => &VQ_HEBAP5[..],
        6 => &VQ_HEBAP6[..],
        7 => &VQ_HEBAP7[..],
        _ => return 0,
    };
    let mut best = 0usize;
    let mut best_d = f32::INFINITY;
    for (idx, entry) in book.iter().enumerate() {
        let mut d = 0.0f32;
        for (xi, &ei) in x.iter().zip(entry.iter()) {
            let diff = xi - ei as f32 / 32768.0;
            d += diff * diff;
        }
        if d < best_d {
            best_d = d;
            best = idx;
        }
    }
    best
}

/// One quantised AHT codeword: `(bit_count, raw_bits)` pairs as they
/// go on the wire. A small (or Gk=1) mantissa is one pair; an escape
/// is the tag pair followed by the large pair.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScalarCode {
    /// Tag + small codeword (always present).
    pub first: (u32, u32),
    /// Large codeword following a tag (escape only).
    pub second: Option<(u32, u32)>,
}

impl ScalarCode {
    #[inline]
    fn bits(&self) -> u32 {
        self.first.0 + self.second.map_or(0, |(n, _)| n)
    }
}

/// Two's-complement truncation of `v` to `n` bits.
#[inline]
fn to_bits(v: i32, n: u32) -> u32 {
    (v as u32) & ((1u32 << n) - 1)
}

/// Quantise one mantissa `x` for the scalar/GAQ regime with gain `gk`
/// (§3.4.4.2 / Tables E3.5-E3.6), returning the codeword(s) and the
/// reconstruction the decoder will produce.
///
/// * `gk == 1` — one `m`-bit codeword; level `k` chosen to minimise
///   `|gaq_remap(k/2^(m-1)) - x|` over `k ∈ [-(2^(m-1)-1),
///   2^(m-1)-1]` (the full-scale negative is never emitted: Table
///   E3.5 gives the Gk=1 quantiser `2^m - 1` levels).
/// * `gk == 2` — small `s = round(x·2^(m-1))` in `(m-1)` bits when
///   representable (tag `-2^(m-2)` excluded); otherwise the tag plus
///   an `(m-1)`-bit large codeword against the Table E3.6 Gk=2 remap.
/// * `gk == 4` — small in `(m-2)` bits; escape pays the tag plus an
///   `m`-bit large codeword against the Gk=4 remap.
pub fn quantise_scalar(hebap: u8, gk: u8, x: f32) -> (ScalarCode, f32) {
    let m = HEBAP_MANT_BITS[hebap as usize] as u32;
    let full = (1i32 << (m - 1)) as f32; // 2^(m-1)
    match gk {
        1 => {
            // Ideal level from the exact remap: y = (k/2^(m-1))·(1+a)
            // is affine in k, so start from the closed form and probe
            // ±1 to absorb the Q15 rounding of `a`.
            let kmax = (1i32 << (m - 1)) - 1;
            let unit = gaq_remap(hebap, 1, 1.0 / full);
            let est = if unit > 0.0 {
                (x / unit).round() as i32
            } else {
                0
            };
            let (mut best_k, mut best_e) = (0i32, f32::INFINITY);
            for k in [est - 1, est, est + 1] {
                let k = k.clamp(-kmax, kmax);
                let y = gaq_remap(hebap, 1, k as f32 / full);
                let e = (y - x).abs();
                if e < best_e {
                    best_e = e;
                    best_k = k;
                }
            }
            let y = gaq_remap(hebap, 1, best_k as f32 / full);
            (
                ScalarCode {
                    first: (m, to_bits(best_k, m)),
                    second: None,
                },
                y,
            )
        }
        2 | 4 => {
            let small_bits = if gk == 2 { m - 1 } else { m - 2 };
            let tag = -(1i32 << (small_bits - 1));
            let s = (x * full).round() as i32;
            if s > tag && s < -tag {
                // Small codeword — composed step 1/2^(m-1).
                let y = s as f32 / full;
                return (
                    ScalarCode {
                        first: (small_bits, to_bits(s, small_bits)),
                        second: None,
                    },
                    y,
                );
            }
            // Escape: tag + large codeword.
            let (large_bits, large_full) = if gk == 2 {
                (m - 1, (1i32 << (m - 2)) as f32)
            } else {
                (m, full)
            };
            let lmax = (1i32 << (large_bits - 1)) - 1;
            let lmin = -(1i32 << (large_bits - 1));
            // Invert y = x'·(1+a) + b(sign) around the requested sign
            // (the `b` offset differs between the x >= 0 and x < 0
            // halves of Table E3.6), then probe ±1 against the exact
            // remap to absorb the Q15 rounding.
            let probe = |l: i32| gaq_remap(hebap, gk, l as f32 / large_full);
            let slope = (probe(1.min(lmax)) - probe(0)).max(1e-9);
            let est = if x >= 0.0 {
                (((x - probe(0)) / slope).round() as i32).clamp(0, lmax)
            } else {
                (-1 + ((x - probe(-1)) / slope).round() as i32).clamp(lmin, -1)
            };
            let (mut best_l, mut best_e) = (est, f32::INFINITY);
            for l in [est - 1, est, est + 1] {
                let l = l.clamp(lmin, lmax);
                let e = (probe(l) - x).abs();
                if e < best_e {
                    best_e = e;
                    best_l = l;
                }
            }
            let y = probe(best_l);
            (
                ScalarCode {
                    first: (small_bits, to_bits(tag, small_bits)),
                    second: Some((large_bits, to_bits(best_l, large_bits))),
                },
                y,
            )
        }
        _ => (
            ScalarCode {
                first: (0, 0),
                second: None,
            },
            0.0,
        ),
    }
}

/// Quantise the 6 AHT-domain coefficients of one bin with gain `gk`,
/// returning the codewords, total bit count, and total squared error.
fn quantise_bin(hebap: u8, gk: u8, x: &[f32; 6]) -> ([ScalarCode; 6], u32, f32) {
    let mut codes = [ScalarCode::default(); 6];
    let mut bits = 0u32;
    let mut err = 0.0f32;
    for (j, &xj) in x.iter().enumerate() {
        let (code, y) = quantise_scalar(hebap, gk, xj);
        bits += code.bits();
        err += (y - xj) * (y - xj);
        codes[j] = code;
    }
    (codes, bits, err)
}

/// Exact bit count of one mantissa under gain `gk` — the cheap
/// (no-probe) twin of [`quantise_scalar`]; the small-vs-escape
/// decision (`s = round(x·2^(m-1))` against the tag boundary) is the
/// same expression, so the planner's bit accounting always matches
/// the writer's emission.
#[inline]
fn scalar_bits(hebap: u8, gk: u8, x: f32) -> u32 {
    let m = HEBAP_MANT_BITS[hebap as usize] as u32;
    match gk {
        1 => m,
        2 | 4 => {
            let small_bits = if gk == 2 { m - 1 } else { m - 2 };
            let tag = -(1i32 << (small_bits - 1));
            let s = (x * (1i32 << (m - 1)) as f32).round() as i32;
            if s > tag && s < -tag {
                small_bits
            } else if gk == 2 {
                small_bits + (m - 1)
            } else {
                small_bits + m
            }
        }
        _ => 0,
    }
}

/// Exact bit count of one bin's 6 codewords under gain `gk`.
#[inline]
fn bin_bits(hebap: u8, gk: u8, x: &[f32; 6]) -> u32 {
    x.iter().map(|&v| scalar_bits(hebap, gk, v)).sum()
}

/// Per-channel GAQ plan: the chosen `gaqmod`, the per-active-bin
/// mapped gain values (Table E3.4: 0 → Gk=1, 1 → Gk=2, 2 → Gk=4, in
/// ascending bin order), and the exact mantissa-payload bit count
/// (everything after the 2-bit `chgaqmod`, gains included).
#[derive(Clone, Debug, Default)]
pub struct AhtChannelPlan {
    pub gaqmod: u8,
    /// Mapped gain value per GAQ-active bin (ascending bin order).
    pub gains: Vec<u8>,
    /// Total payload bits: gain words + all VQ/scalar codewords.
    /// Excludes the 2-bit `chgaqmod` field itself.
    pub payload_bits: u32,
}

impl AhtChannelPlan {
    /// Total bits this channel's AHT block occupies in the audblk
    /// (including the 2-bit `chgaqmod`).
    pub fn total_bits(&self) -> u32 {
        2 + self.payload_bits
    }
}

/// Map a Table E3.4 mapped value to the gain factor.
#[inline]
fn gain_of_mapped(mapped: u8) -> u8 {
    match mapped {
        0 => 1,
        1 => 2,
        _ => 4,
    }
}

/// Gains selectable per mode: `(allowed mapped values, gain-word bits
/// per active bin)`. Mode 3's 5-bit triplets are handled separately.
fn mode_allowed(gaqmod: u8) -> &'static [u8] {
    match gaqmod {
        1 => &[0, 1],    // Gk ∈ {1, 2}
        2 => &[0, 2],    // Gk ∈ {1, 4}
        3 => &[0, 1, 2], // Gk ∈ {1, 2, 4}
        _ => &[0],
    }
}

/// §3.4.4.2 encoder-side GAQ planning for one AHT channel.
///
/// `hebap[bin]` and `x[bin]` (6 AHT-domain coefficients per bin) must
/// cover `start..end`; bins outside the range are ignored. Evaluates
/// all four `gaqmod` values with exact bit accounting (VQ bins cost
/// their Table E3.2 codeword width regardless of mode; scalar bins
/// pick the cheapest allowed gain, Gk=1 on ties since its large-value
/// levels are finer) and returns the cheapest mode (lowest `gaqmod`
/// on ties).
///
/// Bit-count only — no codeword search — so it is cheap enough for
/// the SNR-offset tuner's inner loop. The writer's emission consumes
/// exactly `total_bits()` because the small-vs-escape decision here
/// ([`scalar_bits`]) is the same expression [`quantise_scalar`] uses.
pub fn plan_aht_channel(hebap: &[u8], start: usize, end: usize, x: &[[f32; 6]]) -> AhtChannelPlan {
    let mut best: Option<AhtChannelPlan> = None;
    let max_mode: u8 = std::env::var("EAC3_AHT_MAX_GAQMOD")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    for gaqmod in 0..=max_mode {
        let endbap = endbap_for_gaqmod(gaqmod);
        let mut gains: Vec<u8> = Vec::new();
        let mut bits = 0u32;
        for bin in start..end {
            let h = hebap[bin];
            if h == 0 {
                continue;
            }
            if (1..=7).contains(&h) {
                // VQ regime — cost is fixed by the codebook.
                bits += VQ_BITS[h as usize] as u32;
                continue;
            }
            if gaqmod > 0 && h < endbap {
                // GAQ-active bin: pick the cheapest allowed gain
                // (strict `<` keeps Gk=1 on ties).
                let (mut b_gain, mut b_bits) = (0u8, u32::MAX);
                for &mapped in mode_allowed(gaqmod) {
                    let gk = gain_of_mapped(mapped);
                    let gbits = bin_bits(h, gk, &x[bin]);
                    if gbits < b_bits {
                        b_gain = mapped;
                        b_bits = gbits;
                    }
                }
                gains.push(b_gain);
                bits += b_bits;
            } else {
                // Fixed quantiser (h >= endbap, or gaqmod == 0).
                bits += bin_bits(h, 1, &x[bin]);
            }
        }
        // Gain-word side information (§3.4.2 gaqsections).
        let gain_bits = match gaqmod {
            1 | 2 => gains.len() as u32,
            3 => (gains.len().div_ceil(3) as u32) * 5,
            _ => 0,
        };
        bits += gain_bits;
        let plan = AhtChannelPlan {
            gaqmod,
            gains,
            payload_bits: bits,
        };
        let better = match &best {
            None => true,
            Some(b) => plan.payload_bits < b.payload_bits,
        };
        if better {
            best = Some(plan);
        }
    }
    best.expect("at least gaqmod 0 evaluated")
}

/// Emit one channel's front-loaded AHT mantissa block (§3.4.4 read
/// order): 2-bit `chgaqmod`, the gain words, then per ascending bin
/// the VQ index or 6 scalar/GAQ codewords. Must mirror
/// `decode_aht_channel_mantissas` in [`super::dsp`] exactly.
pub fn write_aht_channel(
    bw: &mut BitWriter,
    plan: &AhtChannelPlan,
    hebap: &[u8],
    start: usize,
    end: usize,
    x: &[[f32; 6]],
) {
    bw.write_u32(plan.gaqmod as u32, 2);
    // Gain words.
    match plan.gaqmod {
        1 | 2 => {
            for &g in &plan.gains {
                bw.write_u32(u32::from(g != 0), 1);
            }
        }
        3 => {
            for triple in plan.gains.chunks(3) {
                let m1 = *triple.first().unwrap_or(&0) as u32;
                let m2 = *triple.get(1).unwrap_or(&0) as u32;
                let m3 = *triple.get(2).unwrap_or(&0) as u32;
                bw.write_u32(m1 * 9 + m2 * 3 + m3, 5);
            }
        }
        _ => {}
    }
    // Mantissas.
    let endbap = endbap_for_gaqmod(plan.gaqmod);
    let mut gain_iter = plan.gains.iter();
    for bin in start..end {
        let h = hebap[bin];
        if h == 0 {
            continue;
        }
        if (1..=7).contains(&h) {
            let idx = vq_search(h, &x[bin]);
            bw.write_u32(idx as u32, VQ_BITS[h as usize] as u32);
            continue;
        }
        let gk = if plan.gaqmod > 0 && h < endbap {
            gain_of_mapped(*gain_iter.next().unwrap_or(&0))
        } else {
            1
        };
        let (codes, _, _) = quantise_bin(h, gk, &x[bin]);
        for code in codes.iter() {
            bw.write_u32(code.first.1, code.first.0);
            if let Some((n, v)) = code.second {
                bw.write_u32(v, n);
            }
        }
    }
}

/// Derive the §3.4.3.1 high-efficiency bit-allocation pointers
/// (`hebap[]`) for one channel: the base-AC-3 psd / excitation /
/// masking pipeline with the final lookup routed through the Table
/// E3.1 `hebaptab[]` instead of `baptab[]`.
pub(crate) fn compute_hebap(
    exp: &[u8; N_COEFFS],
    end: usize,
    fscod: u8,
    ba: &BitAllocParams,
    hebap_out: &mut [u8; N_COEFFS],
    dba: Option<(&DbaPlan, usize)>,
) {
    compute_bap_table(exp, end, fscod, ba, hebap_out, dba, &super::aht::HEBAPTAB)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eac3::aht::{
        self, fill_gaqbin, gaq_sections, idct_ii_6, read_gaq_gains, read_scalar_aht_mantissas,
        AHT_BLOCKS,
    };
    use oxideav_core::bits::BitReader;

    #[test]
    fn dct_idct_round_trip() {
        let c = [0.3f32, -0.7, 0.11, 0.02, -0.4, 0.55];
        let x = dct_ii_6(&c);
        let back = idct_ii_6(x);
        for (a, b) in c.iter().zip(back.iter()) {
            assert!((a - b).abs() < 1e-5, "{a} vs {b}");
        }
    }

    #[test]
    fn dct_of_constant_is_dc_only() {
        // DC basis weight is 1 in the deployed convention: a constant
        // cross-block mantissa maps to X(0) unchanged.
        let x = dct_ii_6(&[0.5; 6]);
        assert!((x[0] - 0.5).abs() < 1e-6);
        for &v in &x[1..] {
            assert!(v.abs() < 1e-6);
        }
    }

    #[test]
    fn vq_search_recovers_exact_codebook_entry() {
        for hebap in 1u8..=7 {
            // Probe a few entries spread across the book.
            for idx in [0usize, 1, 3] {
                let target = aht::vq_lookup(hebap, idx);
                let found = vq_search(hebap, &target);
                let recon = aht::vq_lookup(hebap, found);
                // The index may differ if two entries are identical;
                // the reconstruction must be exact either way.
                assert_eq!(recon, target, "hebap {hebap} idx {idx} → {found}");
            }
        }
    }

    /// Quantise → bitstream → decoder read → compare, across every
    /// scalar hebap and every gain, on a sweep of values.
    #[test]
    fn scalar_quantise_round_trips_through_decoder() {
        use oxideav_core::bits::BitWriter;
        for hebap in 8u8..=19 {
            let m = HEBAP_MANT_BITS[hebap as usize] as i32;
            let step = 2.0f32 / ((1i64 << m) - 1) as f32;
            for (gaqmod, gain_code, gk) in [
                (0u8, 0u8, 1u8),
                (1, 0, 1),
                (1, 1, 2),
                (2, 1, 4),
                (3, 1, 2),
                (3, 2, 4),
            ] {
                // GAQ gains only exist inside the mode's active range.
                if gk > 1 && hebap >= endbap_for_gaqmod(gaqmod) {
                    continue;
                }
                let vals: [f32; 6] = [-0.83, -0.31, -0.02, 0.0, 0.27, 0.78];
                let mut bw = BitWriter::with_capacity(64);
                let mut expect = [0.0f32; 6];
                for (j, &v) in vals.iter().enumerate() {
                    let (code, y) = quantise_scalar(hebap, gk, v);
                    expect[j] = y;
                    assert!(
                        (y - v).abs() <= step * 1.01,
                        "hebap {hebap} gk {gk}: |{y} - {v}| > step {step}"
                    );
                    bw.write_u32(code.first.1, code.first.0);
                    if let Some((n, val)) = code.second {
                        bw.write_u32(val, n);
                    }
                }
                bw.write_u32(0, 32);
                let bytes = bw.into_bytes();
                let mut br = BitReader::new(&bytes);
                let mut out = [0.0f32; 6];
                let gaqbin = if gk > 1 { 1 } else { 0 };
                read_scalar_aht_mantissas(&mut br, hebap, gaqmod, gaqbin, gain_code, &mut out)
                    .unwrap();
                for j in 0..6 {
                    assert!(
                        (out[j] - expect[j]).abs() < 1e-6,
                        "hebap {hebap} gaqmod {gaqmod} gk {gk} j {j}: decoder {} vs encoder {}",
                        out[j],
                        expect[j]
                    );
                }
            }
        }
    }

    /// Full-channel plan + write → mirror of the decoder's AHT read
    /// loop (fill_gaqbin / gaq_sections / read_gaq_gains / VQ / scalar)
    /// → the reconstruction matches within the quantiser step, the bit
    /// count matches the plan, and the whole thing survives all four
    /// hebap regimes at once.
    #[test]
    fn channel_plan_write_decode_round_trip() {
        use oxideav_core::bits::BitWriter;
        // Craft an hebap profile that hits: zero (0), VQ (2, 6),
        // GAQ-active scalar (8, 10), and fixed scalar (17, 19).
        let hebap: Vec<u8> = vec![0, 2, 6, 8, 10, 17, 19, 9, 0, 12];
        let end = hebap.len();
        // Deterministic mantissa content: mixture of small and large
        // values so GAQ escapes fire.
        let mut x = vec![[0.0f32; 6]; end];
        let mut seed = 0x1357_9bdfu32;
        for bin in 0..end {
            for j in 0..6 {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let v = (seed as f32 / u32::MAX as f32) * 1.6 - 0.8;
                // Bias most values small so GAQ gains win.
                x[bin][j] = if j % 3 == 0 { v } else { v * 0.2 };
            }
        }
        let plan = plan_aht_channel(&hebap, 0, end, &x);
        let mut bw = BitWriter::with_capacity(256);
        write_aht_channel(&mut bw, &plan, &hebap, 0, end, &x);
        let written = bw.bit_position();
        assert_eq!(
            written,
            plan.total_bits() as u64,
            "write must consume exactly the planned bit count"
        );
        bw.write_u32(0, 32);
        let bytes = bw.into_bytes();

        // ---- decoder-mirror read ----
        let mut br = BitReader::new(&bytes);
        let gaqmod = br.read_u32(2).unwrap() as u8;
        assert_eq!(gaqmod, plan.gaqmod);
        let mut gaqbin = vec![0i8; end];
        let active = fill_gaqbin(&hebap, gaqmod, &mut gaqbin);
        assert_eq!(active, plan.gains.len());
        let nsections = gaq_sections(gaqmod, active);
        let mut gain_words = vec![0u8; active];
        read_gaq_gains(&mut br, gaqmod, nsections, &mut gain_words).unwrap();
        let mut gain_iter = gain_words.into_iter();
        for bin in 0..end {
            let h = hebap[bin];
            if h == 0 {
                continue;
            }
            if (1..=7).contains(&h) {
                let nb = VQ_BITS[h as usize] as u32;
                let idx = br.read_u32(nb).unwrap() as usize;
                let v = aht::vq_lookup(h, idx);
                // VQ error bounded by codebook coverage — just check
                // the decode is the entry the encoder picked.
                assert_eq!(v, aht::vq_lookup(h, vq_search(h, &x[bin])));
                continue;
            }
            let gain_code = if gaqbin[bin] == 1 {
                gain_iter.next().unwrap_or(0)
            } else {
                0
            };
            let mut out = [0.0f32; 6];
            read_scalar_aht_mantissas(&mut br, h, gaqmod, gaqbin[bin], gain_code, &mut out)
                .unwrap();
            let m = HEBAP_MANT_BITS[h as usize] as i32;
            let step = 2.0f32 / ((1i64 << m) - 1) as f32;
            for j in 0..6 {
                assert!(
                    (out[j] - x[bin][j]).abs() <= step * 1.01,
                    "bin {bin} (hebap {h}) j {j}: {} vs {} (step {step})",
                    out[j],
                    x[bin][j]
                );
            }
        }
        assert_eq!(
            br.bit_position(),
            written,
            "decoder mirror must consume exactly the written bits"
        );
        let _ = AHT_BLOCKS;
    }

    /// GAQ must actually engage: an all-small-mantissa channel in the
    /// GAQ-active hebap range must plan a non-zero gaqmod and beat the
    /// gaqmod=0 fixed-quantiser bit count.
    #[test]
    fn gaq_engages_on_small_mantissas() {
        let hebap = vec![9u8; 20];
        let x = vec![[0.05f32, -0.08, 0.11, -0.03, 0.06, -0.1]; 20];
        let plan = plan_aht_channel(&hebap, 0, 20, &x);
        assert_ne!(plan.gaqmod, 0, "small mantissas must select a GAQ mode");
        // gaqmod 0 cost: 20 bins × 6 × 4 bits = 480.
        assert!(
            plan.payload_bits < 480,
            "GAQ plan ({} bits) must beat the fixed 480-bit cost",
            plan.payload_bits
        );
    }
}

//! SIMD-accelerated DSP routines for AC-3 encoding and decoding.
//!
//! Provides optimized kernels for ARM NEON (ARM64 / Apple Silicon) and
//! x86_64 (AVX2 / FMA / SSE2), with clean portable scalar fallbacks.

#[allow(unused_imports)]
use std::f32::consts::PI;

/// Apply symmetric 512-point AC-3 window to 512 time samples.
/// `win_buf[n] = in_buf[n] * window[n]`, `win_buf[511 - n] = in_buf[511 - n] * window[n]` for n in 0..256.
#[inline(always)]
pub fn window_512(in_buf: &[f32; 512], window: &[f32; 256], win_buf: &mut [f32; 512]) {
    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is baseline on aarch64, so no runtime check is needed.
        unsafe { neon_window_512(in_buf, window, win_buf) };
    }

    // Cfg'd out wholesale on aarch64 rather than left after an early return,
    // so the scalar path is not dead code there.
    #[cfg(not(target_arch = "aarch64"))]
    {
        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            // SAFETY: guarded by the feature detection above.
            unsafe { avx2_window_512(in_buf, window, win_buf) };
            return;
        }

        for n in 0..256 {
            let w = window[n];
            win_buf[n] = in_buf[n] * w;
            win_buf[511 - n] = in_buf[511 - n] * w;
        }
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn neon_window_512(in_buf: &[f32; 512], window: &[f32; 256], win_buf: &mut [f32; 512]) {
    use std::arch::aarch64::*;
    let in_ptr = in_buf.as_ptr();
    let win_ptr = window.as_ptr();
    let out_ptr = win_buf.as_mut_ptr();

    for i in (0..256).step_by(4) {
        let w = vld1q_f32(win_ptr.add(i));

        // Front half: in_buf[i..i+4] * window[i..i+4]
        let x_front = vld1q_f32(in_ptr.add(i));
        let out_front = vmulq_f32(x_front, w);
        vst1q_f32(out_ptr.add(i), out_front);

        // Back half: in_buf[511-i-3..=511-i] * reverse(window[i..i+4])
        // Reverse w for back half: [w[0], w[1], w[2], w[3]] -> [w[3], w[2], w[1], w[0]]
        let back_idx = 512 - i - 4;
        let x_back = vld1q_f32(in_ptr.add(back_idx));

        let w_rev = vsetq_lane_f32(vgetq_lane_f32(w, 3), vdupq_n_f32(0.0), 0);
        let w_rev = vsetq_lane_f32(vgetq_lane_f32(w, 2), w_rev, 1);
        let w_rev = vsetq_lane_f32(vgetq_lane_f32(w, 1), w_rev, 2);
        let w_rev = vsetq_lane_f32(vgetq_lane_f32(w, 0), w_rev, 3);

        let out_back = vmulq_f32(x_back, w_rev);
        vst1q_f32(out_ptr.add(back_idx), out_back);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn avx2_window_512(in_buf: &[f32; 512], window: &[f32; 256], win_buf: &mut [f32; 512]) {
    use std::arch::x86_64::*;
    let in_ptr = in_buf.as_ptr();
    let win_ptr = window.as_ptr();
    let out_ptr = win_buf.as_mut_ptr();

    for i in (0..256).step_by(8) {
        let w = _mm256_loadu_ps(win_ptr.add(i));

        // Front half
        let x_front = _mm256_loadu_ps(in_ptr.add(i));
        let out_front = _mm256_mul_ps(x_front, w);
        _mm256_storeu_ps(out_ptr.add(i), out_front);

        // Back half with reverse permutation
        let back_idx = 512 - i - 8;
        let x_back = _mm256_loadu_ps(in_ptr.add(back_idx));
        // Reverse 8 floats in 256-bit vector
        let rev_perm = _mm256_setr_epi32(7, 6, 5, 4, 3, 2, 1, 0);
        let w_rev = _mm256_permutevar8x32_ps(w, rev_perm);
        let out_back = _mm256_mul_ps(x_back, w_rev);
        _mm256_storeu_ps(out_ptr.add(back_idx), out_back);
    }
}

/// Computes sum of squares ($\sum x_i^2$) across a slice of f32 samples.
#[inline(always)]
pub fn energy_sum(slice: &[f32]) -> f64 {
    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is baseline on aarch64.
        return unsafe { neon_energy_sum(slice) };
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            // SAFETY: guarded by the feature detection above.
            return unsafe { avx2_energy_sum(slice) };
        }

        slice.iter().map(|&v| (v as f64) * (v as f64)).sum()
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn neon_energy_sum(slice: &[f32]) -> f64 {
    use std::arch::aarch64::*;
    let len = slice.len();
    let ptr = slice.as_ptr();
    let chunks = len / 4;
    let mut sum_v = vdupq_n_f32(0.0);

    for i in 0..chunks {
        let v = vld1q_f32(ptr.add(i * 4));
        sum_v = vfmaq_f32(sum_v, v, v);
    }

    let mut sum = (vaddvq_f32(sum_v)) as f64;
    for i in (chunks * 4)..len {
        let v = *ptr.add(i) as f64;
        sum += v * v;
    }
    sum
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn avx2_energy_sum(slice: &[f32]) -> f64 {
    use std::arch::x86_64::*;
    let len = slice.len();
    let ptr = slice.as_ptr();
    let chunks = len / 8;
    let mut sum_v = _mm256_setzero_ps();

    for i in 0..chunks {
        let v = _mm256_loadu_ps(ptr.add(i * 8));
        sum_v = _mm256_fmadd_ps(v, v, sum_v);
    }

    let mut arr = [0.0f32; 8];
    _mm256_storeu_ps(arr.as_mut_ptr(), sum_v);
    let mut sum = arr.iter().map(|&x| x as f64).sum::<f64>();

    for i in (chunks * 8)..len {
        let v = *ptr.add(i) as f64;
        sum += v * v;
    }
    sum
}

/// Vectorized stereo mid-side rematrixing:
/// `l[i] = 0.5 * (l[i] + r[i])`, `r[i] = 0.5 * (l[i] - r[i])` for i in 0..len.
#[inline(always)]
pub fn rematrix_stereo(l: &mut [f32], r: &mut [f32]) {
    assert_eq!(l.len(), r.len());

    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is baseline on aarch64.
        unsafe { neon_rematrix_stereo(l, r) };
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            // SAFETY: guarded by the feature detection above.
            unsafe { avx2_rematrix_stereo(l, r) };
            return;
        }

        for i in 0..l.len() {
            let lv = l[i];
            let rv = r[i];
            l[i] = 0.5 * (lv + rv);
            r[i] = 0.5 * (lv - rv);
        }
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn neon_rematrix_stereo(l: &mut [f32], r: &mut [f32]) {
    use std::arch::aarch64::*;
    let len = l.len();
    let l_ptr = l.as_mut_ptr();
    let r_ptr = r.as_mut_ptr();
    let half_v = vdupq_n_f32(0.5);
    let chunks = len / 4;

    for i in 0..chunks {
        let lv = vld1q_f32(l_ptr.add(i * 4));
        let rv = vld1q_f32(r_ptr.add(i * 4));

        let sum = vmulq_f32(vaddq_f32(lv, rv), half_v);
        let diff = vmulq_f32(vsubq_f32(lv, rv), half_v);

        vst1q_f32(l_ptr.add(i * 4), sum);
        vst1q_f32(r_ptr.add(i * 4), diff);
    }

    for i in (chunks * 4)..len {
        let lv = *l_ptr.add(i);
        let rv = *r_ptr.add(i);
        *l_ptr.add(i) = 0.5 * (lv + rv);
        *r_ptr.add(i) = 0.5 * (lv - rv);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn avx2_rematrix_stereo(l: &mut [f32], r: &mut [f32]) {
    use std::arch::x86_64::*;
    let len = l.len();
    let l_ptr = l.as_mut_ptr();
    let r_ptr = r.as_mut_ptr();
    let half_v = _mm256_set1_ps(0.5);
    let chunks = len / 8;

    for i in 0..chunks {
        let lv = _mm256_loadu_ps(l_ptr.add(i * 8));
        let rv = _mm256_loadu_ps(r_ptr.add(i * 8));

        let sum = _mm256_mul_ps(_mm256_add_ps(lv, rv), half_v);
        let diff = _mm256_mul_ps(_mm256_sub_ps(lv, rv), half_v);

        _mm256_storeu_ps(l_ptr.add(i * 8), sum);
        _mm256_storeu_ps(r_ptr.add(i * 8), diff);
    }

    for i in (chunks * 8)..len {
        let lv = *l_ptr.add(i);
        let rv = *r_ptr.add(i);
        *l_ptr.add(i) = 0.5 * (lv + rv);
        *r_ptr.add(i) = 0.5 * (lv - rv);
    }
}

/// Fast leading-zero based exponent extraction for float coefficients.
#[inline(always)]
pub fn extract_exponent_fast(c: f32) -> u8 {
    let bits = c.to_bits() & 0x7FFF_FFFF;
    if bits == 0 {
        return 24;
    }
    // IEEE 754 float: exponent field is bits 30..23 (bias 127).
    // An input normalized to [-1.0, 1.0] has exp <= 127.
    let exp_field = (bits >> 23) as i32;
    let diff = 127 - exp_field;
    if diff <= 0 {
        0
    } else if diff >= 24 {
        24
    } else {
        diff as u8
    }
}

/// §8.2.7 exponent extraction over a whole run of coefficients.
///
/// Matches `encoder::extract_exponent` element for element, minus its
/// `bits == 0` early return: a zero coefficient has an exponent field of 0,
/// which the clamp already carries to 24, and so does every denormal. Dropping
/// the test leaves a branchless body that both NEON and AVX2 vectorise, on a
/// loop the encoder runs over every coded bin of every block.
#[inline]
pub fn extract_exponents(coeffs: &[f32], out: &mut [u8]) {
    debug_assert_eq!(coeffs.len(), out.len());
    for (o, &x) in out.iter_mut().zip(coeffs) {
        let exp_field = ((x.to_bits() & 0x7FFF_FFFF) >> 23) as i32;
        *o = (126 - exp_field).clamp(0, 24) as u8;
    }
}

/// Per-coefficient contribution tables for [`tally_bap_bits`], indexed by bap.
///
/// The three grouped quantisers (§7.3.5 packs bap 1, 2 and 4 in triples and
/// pairs) are counted; everything else owes its fixed §7.3 width. Split into
/// four 16-byte tables so a vector unit can look all four up by bap.
const TALLY_G1: [u8; 16] = tally_flag(1);
const TALLY_G2: [u8; 16] = tally_flag(2);
const TALLY_G4: [u8; 16] = tally_flag(4);
const TALLY_BITS: [u8; 16] = {
    let mut t = [0u8; 16];
    let mut b = 0usize;
    while b < 16 {
        t[b] = match b {
            0 | 1 | 2 | 4 => 0,
            _ => crate::tables::QUANTIZATION_BITS[b],
        };
        b += 1;
    }
    t
};

const fn tally_flag(want: usize) -> [u8; 16] {
    let mut t = [0u8; 16];
    t[want] = 1;
    t
}

/// Mantissa-bit tally for a run of coefficients, from their exponents and the
/// masking curve's per-*band* allocation base.
///
/// Each bin's §7.2.2.7 address is `clamp(base[band] - 4 * exp, 0, 63)`, and
/// the bap it selects either counts towards one of the three grouped classes
/// (§7.3.5 packs bap 1, 2 and 4 in triples and pairs) or owes a fixed number
/// of bits. Returns `[bap-1 count, bap-2 count, bap-4 count, bits owed by the
/// rest]`.
///
/// `bands` is `masktab[bin]` for each coefficient — its band number — and
/// `band_base` is indexed by that. §7.2.2 has 50 bands, so both the band
/// lookup and the address lookup are 64-entry tables, which is exactly what a
/// vector unit does in one instruction: no per-bin base array has to be
/// materialised for them.
///
/// This is the encoder's hottest loop by a wide margin — the rate-control
/// search walks every coded bin of every block a dozen times a frame — which
/// is what earns it a vector kernel.
pub fn tally_bap_bits(exp: &[u8], bands: &[u8], band_base: &[u8; 64]) -> [u32; 4] {
    debug_assert_eq!(exp.len(), bands.len());
    debug_assert!(
        exp.iter().all(|&e| e <= 63),
        "exponents are 0..=24 (§7.1.3)"
    );
    debug_assert!(bands.iter().all(|&b| b < 64), "§7.2.2 has 50 bands");

    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is baseline on aarch64, so no runtime check is needed.
        return unsafe { neon_tally_bap_bits(exp, bands, band_base) };
    }

    #[cfg(not(target_arch = "aarch64"))]
    scalar_tally_bap_bits(exp, bands, band_base)
}

/// Portable [`tally_bap_bits`]. Also the oracle the NEON kernel is checked
/// against by `neon_tally_matches_scalar`.
#[inline]
pub(crate) fn scalar_tally_bap_bits(exp: &[u8], bands: &[u8], band_base: &[u8; 64]) -> [u32; 4] {
    let mut out = [0u32; 4];
    for (&e, &bnd) in exp.iter().zip(bands) {
        let addr = band_base[bnd as usize].saturating_sub(e << 2).min(63) as usize;
        let bap = crate::tables::BAPTAB[addr] as usize;
        out[0] += TALLY_G1[bap] as u32;
        out[1] += TALLY_G2[bap] as u32;
        out[2] += TALLY_G4[bap] as u32;
        out[3] += TALLY_BITS[bap] as u32;
    }
    out
}

#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn neon_tally_bap_bits(exp: &[u8], bands: &[u8], band_base: &[u8; 64]) -> [u32; 4] {
    use std::arch::aarch64::*;

    let n = exp.len();
    let ep = exp.as_ptr();
    let np = bands.as_ptr();

    let load4 = |p: *const u8| {
        uint8x16x4_t(
            vld1q_u8(p),
            vld1q_u8(p.add(16)),
            vld1q_u8(p.add(32)),
            vld1q_u8(p.add(48)),
        )
    };
    // The band-number-to-base table, and §7.2.2.4's address-to-bap table.
    let base_tab = load4(band_base.as_ptr());
    let baptab = load4(crate::tables::BAPTAB.as_ptr());
    let g1 = vld1q_u8(TALLY_G1.as_ptr());
    let g2 = vld1q_u8(TALLY_G2.as_ptr());
    let g4 = vld1q_u8(TALLY_G4.as_ptr());
    let bits = vld1q_u8(TALLY_BITS.as_ptr());
    let cap = vdupq_n_u8(63);

    // Widening accumulators: `vpadalq_u8` folds each chunk's byte lanes in
    // pairs, so even a full 253-bin run cannot come near overflowing them.
    let mut a1 = vdupq_n_u16(0);
    let mut a2 = vdupq_n_u16(0);
    let mut a4 = vdupq_n_u16(0);
    let mut ab = vdupq_n_u16(0);

    let mut i = 0usize;
    while i + 16 <= n {
        let e = vld1q_u8(ep.add(i));
        let base = vqtbl4q_u8(base_tab, vld1q_u8(np.add(i)));
        // addr = clamp(base - 4*exp, 0, 63): the saturating subtract supplies
        // the lower clamp, and exponents cap `4 * exp` at 96 so nothing wraps.
        let addr = vminq_u8(vqsubq_u8(base, vshlq_n_u8(e, 2)), cap);
        let bap = vqtbl4q_u8(baptab, addr);
        a1 = vpadalq_u8(a1, vqtbl1q_u8(g1, bap));
        a2 = vpadalq_u8(a2, vqtbl1q_u8(g2, bap));
        a4 = vpadalq_u8(a4, vqtbl1q_u8(g4, bap));
        ab = vpadalq_u8(ab, vqtbl1q_u8(bits, bap));
        i += 16;
    }

    let mut out = [
        vaddvq_u16(a1) as u32,
        vaddvq_u16(a2) as u32,
        vaddvq_u16(a4) as u32,
        vaddvq_u16(ab) as u32,
    ];
    if i < n {
        let tail = scalar_tally_bap_bits(&exp[i..], &bands[i..], band_base);
        for k in 0..4 {
            out[k] += tail[k];
        }
    }
    out
}

#[cfg(test)]
mod tally_tests {
    use super::*;

    /// The vector kernel and the portable one must agree exactly — the encoder
    /// picks whichever the target has, and its bitstream must not depend on
    /// that choice.
    #[test]
    fn neon_tally_matches_scalar() {
        let mut rng = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        for len in [0usize, 1, 7, 15, 16, 17, 31, 120, 253] {
            for _ in 0..64 {
                let exp: Vec<u8> = (0..len).map(|_| (next() % 25) as u8).collect();
                let bands: Vec<u8> = (0..len).map(|_| (next() % 50) as u8).collect();
                let mut band_base = [0u8; 64];
                for b in band_base.iter_mut() {
                    // Spans the whole reachable range, including the ends where
                    // every bin in a band saturates to bap 0 or bap 15.
                    *b = (next() % 170) as u8;
                }
                assert_eq!(
                    tally_bap_bits(&exp, &bands, &band_base),
                    scalar_tally_bap_bits(&exp, &bands, &band_base),
                    "len={len}"
                );
            }
        }
    }
}

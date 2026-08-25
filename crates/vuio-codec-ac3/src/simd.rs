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
    unsafe {
        neon_window_512(in_buf, window, win_buf);
        return;
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe {
                avx2_window_512(in_buf, window, win_buf);
                return;
            }
        }
    }

    // Scalar fallback
    for n in 0..256 {
        let w = window[n];
        win_buf[n] = in_buf[n] * w;
        win_buf[511 - n] = in_buf[511 - n] * w;
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
    unsafe {
        return neon_energy_sum(slice);
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            unsafe {
                return avx2_energy_sum(slice);
            }
        }
    }

    // Scalar fallback
    slice.iter().map(|&v| (v as f64) * (v as f64)).sum()
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
    let len = l.len();

    #[cfg(target_arch = "aarch64")]
    unsafe {
        neon_rematrix_stereo(l, r);
        return;
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe {
                avx2_rematrix_stereo(l, r);
                return;
            }
        }
    }

    // Scalar fallback
    for i in 0..len {
        let lv = l[i];
        let rv = r[i];
        l[i] = 0.5 * (lv + rv);
        r[i] = 0.5 * (lv - rv);
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

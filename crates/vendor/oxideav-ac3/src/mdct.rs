//! Forward MDCT (Modified Discrete Cosine Transform) for AC-3 encoding.
//!
//! Implements fast SIMD-accelerated forward MDCT using time-domain TDAC folding
//! and precomputed DCT-IV basis tables per ATSC A/52:2018 §8.2.3.2.

use std::f32::consts::PI;
use std::sync::OnceLock;

/// Precomputed 256x256 DCT-IV cosine table:
/// `TABLE[k][j] = cos( π / 1024 * (2k + 1) * (2j + 1) )`
struct Dct4Table {
    table: Box<[[f32; 256]; 256]>,
}

static DCT4_TABLE: OnceLock<Dct4Table> = OnceLock::new();

fn get_dct4_table() -> &'static Dct4Table {
    DCT4_TABLE.get_or_init(|| {
        let mut table = vec![[0.0f32; 256]; 256].into_boxed_slice();
        for k in 0..256 {
            let two_k1 = (2 * k + 1) as f32;
            for j in 0..256 {
                let two_j1 = (2 * j + 1) as f32;
                let angle = PI / 1024.0 * two_k1 * two_j1;
                table[k][j] = angle.cos();
            }
        }
        let ptr = Box::into_raw(table) as *mut [[f32; 256]; 256];
        Dct4Table {
            table: unsafe { Box::from_raw(ptr) },
        }
    })
}

/// Precomputed tables for short-block pairs (alpha = -1.0 and +1.0):
/// 128x256 floats per table.
struct ShortDctTable {
    alpha_neg: Box<[[f32; 256]; 128]>,
    alpha_pos: Box<[[f32; 256]; 128]>,
}

static SHORT_DCT_TABLE: OnceLock<ShortDctTable> = OnceLock::new();

fn get_short_dct_table() -> &'static ShortDctTable {
    SHORT_DCT_TABLE.get_or_init(|| {
        let mut neg = vec![[0.0f32; 256]; 128].into_boxed_slice();
        let mut pos = vec![[0.0f32; 256]; 128].into_boxed_slice();

        let n = 256.0f32;
        let pi_over_2n = PI / (2.0 * n);
        let quarter_pi = PI / 4.0;

        for k in 0..128 {
            let two_k1 = (2 * k + 1) as f32;
            let phase_neg = quarter_pi * two_k1 * 0.0; // 1 + (-1) = 0
            let phase_pos = quarter_pi * two_k1 * 2.0; // 1 + (+1) = 2
            for nn in 0..256 {
                let two_n1 = (2 * nn + 1) as f32;
                neg[k][nn] = (pi_over_2n * two_n1 * two_k1 + phase_neg).cos();
                pos[k][nn] = (pi_over_2n * two_n1 * two_k1 + phase_pos).cos();
            }
        }

        let ptr_neg = Box::into_raw(neg) as *mut [[f32; 256]; 128];
        let ptr_pos = Box::into_raw(pos) as *mut [[f32; 256]; 128];
        ShortDctTable {
            alpha_neg: unsafe { Box::from_raw(ptr_neg) },
            alpha_pos: unsafe { Box::from_raw(ptr_pos) },
        }
    })
}

/// Fast SIMD-accelerated 512-point forward MDCT (§8.2.3.2, α=0 long transform).
///
/// Folds 512 input samples into a 256-point DCT-IV, evaluated with SIMD FMA
/// dot products against precomputed trigonometric basis vectors.
#[inline]
pub fn mdct_512(input: &[f32; 512], output: &mut [f32; 256]) {
    // 1. Time folding into x_sym[256]:
    // For q in 0..128: x_sym[127 - q] = -input[256 + q] - input[511 - q]
    // For p in 0..128: x_sym[128 + p] = input[p] - input[255 - p]
    let mut x_sym = [0.0f32; 256];
    for q in 0..128 {
        x_sym[127 - q] = -input[256 + q] - input[511 - q];
    }
    for p in 0..128 {
        x_sym[128 + p] = input[p] - input[255 - p];
    }

    let tbl = get_dct4_table();
    let scale = -1.0f32 / 256.0f32;

    for k in 0..256 {
        let row = &tbl.table[k];
        let mut s = 0.0f32;

        #[cfg(target_arch = "aarch64")]
        unsafe {
            use std::arch::aarch64::*;
            let x_ptr = x_sym.as_ptr();
            let row_ptr = row.as_ptr();
            let mut sum_v = vdupq_n_f32(0.0);

            for i in (0..256).step_by(4) {
                let vx = vld1q_f32(x_ptr.add(i));
                let vr = vld1q_f32(row_ptr.add(i));
                sum_v = vfmaq_f32(sum_v, vx, vr);
            }
            s = vaddvq_f32(sum_v);
        }

        #[cfg(target_arch = "x86_64")]
        unsafe {
            use std::arch::x86_64::*;
            if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
                let x_ptr = x_sym.as_ptr();
                let row_ptr = row.as_ptr();
                let mut sum_v = _mm256_setzero_ps();

                for i in (0..256).step_by(8) {
                    let vx = _mm256_loadu_ps(x_ptr.add(i));
                    let vr = _mm256_loadu_ps(row_ptr.add(i));
                    sum_v = _mm256_fmadd_ps(vx, vr, sum_v);
                }

                let mut arr = [0.0f32; 8];
                _mm256_storeu_ps(arr.as_mut_ptr(), sum_v);
                s = arr.iter().sum();
            } else {
                for j in 0..256 {
                    s += x_sym[j] * row[j];
                }
            }
        }

        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        {
            for j in 0..256 {
                s += x_sym[j] * row[j];
            }
        }

        output[k] = scale * s;
    }
}

/// 256-point forward MDCT for one half of a short-block pair (§8.2.3.2).
#[inline]
fn mdct_256_half(input: &[f32; 256], alpha: f32, output: &mut [f32; 128]) {
    let tbl = get_short_dct_table();
    let rows = if alpha < 0.0 {
        &tbl.alpha_neg
    } else {
        &tbl.alpha_pos
    };
    let scale = -2.0f32 / 256.0f32;

    for k in 0..128 {
        let row = &rows[k];
        let mut s = 0.0f32;

        #[cfg(target_arch = "aarch64")]
        unsafe {
            use std::arch::aarch64::*;
            let in_ptr = input.as_ptr();
            let row_ptr = row.as_ptr();
            let mut sum_v = vdupq_n_f32(0.0);

            for i in (0..256).step_by(4) {
                let vx = vld1q_f32(in_ptr.add(i));
                let vr = vld1q_f32(row_ptr.add(i));
                sum_v = vfmaq_f32(sum_v, vx, vr);
            }
            s = vaddvq_f32(sum_v);
        }

        #[cfg(target_arch = "x86_64")]
        unsafe {
            use std::arch::x86_64::*;
            if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
                let in_ptr = input.as_ptr();
                let row_ptr = row.as_ptr();
                let mut sum_v = _mm256_setzero_ps();

                for i in (0..256).step_by(8) {
                    let vx = _mm256_loadu_ps(in_ptr.add(i));
                    let vr = _mm256_loadu_ps(row_ptr.add(i));
                    sum_v = _mm256_fmadd_ps(vx, vr, sum_v);
                }

                let mut arr = [0.0f32; 8];
                _mm256_storeu_ps(arr.as_mut_ptr(), sum_v);
                s = arr.iter().sum();
            } else {
                for j in 0..256 {
                    s += input[j] * row[j];
                }
            }
        }

        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        {
            for j in 0..256 {
                s += input[j] * row[j];
            }
        }

        output[k] = scale * s;
    }
}

/// Forward short-block MDCT pair (§8.2.3.2 + §7.9.4.2).
#[inline]
pub fn mdct_256_pair(input: &[f32; 512], output: &mut [f32; 256]) {
    let mut h1 = [0.0f32; 256];
    let mut h2 = [0.0f32; 256];
    h1.copy_from_slice(&input[..256]);
    h2.copy_from_slice(&input[256..]);

    let mut x1 = [0.0f32; 128];
    let mut x2 = [0.0f32; 128];

    mdct_256_half(&h1, -1.0, &mut x1);
    mdct_256_half(&h2, 1.0, &mut x2);

    for k in 0..128 {
        output[2 * k] = x1[k];
        output[2 * k + 1] = x2[k];
    }
}

/// Reference O(N^2) direct-form 512-point forward MDCT.
pub fn mdct_512_ref(input: &[f32; 512], output: &mut [f32; 256]) {
    let n: usize = 512;
    let scale: f32 = -2.0 / n as f32;
    let two_pi_over_4n = 2.0 * PI / (4.0 * n as f32);
    let pi_over_4 = PI / 4.0;
    for k in 0..256 {
        let mut s = 0.0f32;
        let two_k_plus_1 = (2 * k + 1) as f32;
        let phase_b = pi_over_4 * two_k_plus_1;
        for nn in 0..n {
            let phase = two_pi_over_4n * (2 * nn + 1) as f32 * two_k_plus_1 + phase_b;
            s += input[nn] * phase.cos();
        }
        output[k] = scale * s;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audblk;
    use crate::tables::WINDOW;

    #[test]
    fn mdct_512_matches_reference() {
        let mut input = [0.0f32; 512];
        for i in 0..512 {
            input[i] = ((i as f32 * 0.37).sin() + (i as f32 * 0.91).cos()) * 0.5;
        }

        let mut out_ref = [0.0f32; 256];
        let mut out_fft = [0.0f32; 256];

        mdct_512_ref(&input, &mut out_ref);
        mdct_512(&input, &mut out_fft);

        let mut max_err: f32 = 0.0;
        for k in 0..256 {
            let err = (out_fft[k] - out_ref[k]).abs();
            max_err = max_err.max(err);
        }
        eprintln!("mdct_512 max diff vs direct reference: {max_err:.8e}");
        assert!(max_err < 1e-4, "max error {max_err} exceeds tolerance");
    }

    #[test]
    fn mdct_imdct_roundtrip_identity_window_tdac() {
        let sig_len = 512 + 256;
        let mut sig = vec![0.0f32; sig_len];
        for (i, s) in sig.iter_mut().enumerate() {
            let t = i as f32 / 48_000.0;
            *s = (2.0 * PI * 440.0 * t).sin() * 0.3;
        }

        let mut full_win = [0.0f32; 512];
        for n in 0..256 {
            full_win[n] = WINDOW[n];
            full_win[511 - n] = WINDOW[n];
        }

        let mut blk0 = [0.0f32; 512];
        for n in 0..512 {
            blk0[n] = sig[n] * full_win[n];
        }
        let mut x0 = [0.0f32; 256];
        mdct_512(&blk0, &mut x0);

        let mut blk1 = [0.0f32; 512];
        for n in 0..512 {
            blk1[n] = sig[256 + n] * full_win[n];
        }
        let mut x1 = [0.0f32; 256];
        mdct_512(&blk1, &mut x1);

        let mut delay = [0.0f32; 256];

        let mut time0 = [0.0f32; 512];
        audblk::imdct_512(&x0, &mut time0);
        for n in 0..256 {
            time0[n] *= WINDOW[n];
            time0[511 - n] *= WINDOW[n];
        }
        let mut _pcm0 = [0.0f32; 256];
        for n in 0..256 {
            _pcm0[n] = 2.0 * (time0[n] + delay[n]);
            delay[n] = time0[256 + n];
        }

        let mut time1 = [0.0f32; 512];
        audblk::imdct_512(&x1, &mut time1);
        for n in 0..256 {
            time1[n] *= WINDOW[n];
            time1[511 - n] *= WINDOW[n];
        }
        let mut pcm1 = [0.0f32; 256];
        for n in 0..256 {
            pcm1[n] = 2.0 * (time1[n] + delay[n]);
        }

        let mut worst: f32 = 0.0;
        let mut sse: f32 = 0.0;
        for n in 0..256 {
            let err = (pcm1[n] - sig[256 + n]).abs();
            worst = worst.max(err);
            sse += err * err;
        }
        let rms = (sse / 256.0).sqrt();
        eprintln!("mdct-imdct roundtrip: worst={worst:.5}, rms={rms:.5}");
        assert!(worst < 0.01, "worst {worst} too large");
        assert!(rms < 5e-3, "rms {rms} too large");
    }

    #[test]
    fn mdct_256_pair_recovers_subspace_signal() {
        let mut x_target = [0.0f32; 256];
        for k in 0..16 {
            x_target[2 * k] = 0.7 * (k as f32).sin();
            x_target[2 * k + 1] = 0.5 * (k as f32 * 1.3).cos();
        }
        let mut sig = [0.0f32; 512];
        crate::imdct::imdct_256_pair_fft(&x_target, &mut sig);
        let mut x_back = [0.0f32; 256];
        mdct_256_pair(&sig, &mut x_back);
        let mut max_err: f32 = 0.0;
        for k in 0..256 {
            max_err = max_err.max((x_back[k] - x_target[k]).abs());
        }
        eprintln!("subspace round-trip: max coeff err = {max_err:.6e}");
        assert!(max_err < 1e-3, "mismatch: {max_err}");
    }
}

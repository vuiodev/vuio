//! Forward MDCT (Modified Discrete Cosine Transform) for AC-3 encoding.
//!
//! Implements the §8.2.3.2 forward transform as time-domain TDAC folding
//! followed by a DCT-IV, with the DCT-IV evaluated through a half-length
//! complex FFT (ATSC A/52:2018 §8.2.3.2).
//!
//! # Why an FFT
//!
//! This used to evaluate the DCT-IV directly, as 256 dot products against a
//! precomputed 256x256 cosine matrix: 65,536 multiply-accumulates and 256 KiB
//! of table streamed through cache *per block per channel*. At 5.1 that is 36
//! transforms per frame, and it made the transform the single largest cost in
//! the encoder. The decomposition below costs a few thousand operations
//! against a table of a few hundred bytes that stays resident in L1.
//!
//! # The decomposition
//!
//! For an `M`-point DCT-IV, with `H = M/2`:
//!
//! ```text
//!   c[n] = (x[2n] + i·x[M-1-2n]) · exp(-iπ(4n+1)/(4M))     n in 0..H
//!   U    = FFT_H(c)
//!   v[k] = U[k] · exp(-iπk/M)                              k in 0..H
//!   X[2k] = Re(v[k]),   X[M-1-2k] = -Im(v[k])
//! ```
//!
//! Note the post-twiddle is `exp(-iπk/M)`, not the `exp(-iπ(4k+1)/(4M))` that
//! the symmetry with the pre-twiddle suggests — the two differ by a constant
//! `exp(iπ/(4M))` which the algebra puts on only one side. Getting that wrong
//! yields a transform correct to about half a percent, which is exactly the
//! kind of error that survives casual listening, so
//! `mdct_512_matches_reference` checks against the direct-form
//! [`mdct_512_ref`] oracle rather than against itself.
//!
//! The FFT here is deliberately not [`crate::imdct`]'s: that one advances its
//! twiddles by repeated complex multiplication, which is a serial dependency
//! per butterfly and accumulates f32 error across stages. This one reads them
//! from a table built once.

use std::f32::consts::PI;
use std::sync::OnceLock;

/// Complex value as `(re, im)`, matching [`crate::imdct`]'s convention rather
/// than pulling in `num-complex` for four arithmetic operations.
type C = (f32, f32);

/// Iterative radix-2 decimation-in-time FFT with precomputed twiddles and a
/// precomputed bit-reversal permutation.
struct Fft {
    n: usize,
    /// Per-stage twiddles laid out contiguously: the stage with `half`
    /// butterflies per group starts at offset `half - 1` and holds
    /// `exp(-i·2πj/(2·half))` for `j in 0..half`. Total `n - 1` entries.
    tw: Box<[C]>,
    /// `rev[i]` is `i` with its `log2(n)` low bits reversed.
    rev: Box<[u16]>,
}

impl Fft {
    fn new(n: usize) -> Self {
        assert!(n.is_power_of_two() && n <= u16::MAX as usize + 1);
        let log2n = n.trailing_zeros();

        let mut tw = Vec::with_capacity(n - 1);
        let mut half = 1usize;
        while half < n {
            for j in 0..half {
                // exp(-i·2πj/(2·half)) = exp(-i·πj/half)
                let theta = -PI * j as f32 / half as f32;
                tw.push((theta.cos(), theta.sin()));
            }
            half *= 2;
        }

        let rev = (0..n)
            .map(|i| {
                let mut x = i;
                let mut r = 0usize;
                for _ in 0..log2n {
                    r = (r << 1) | (x & 1);
                    x >>= 1;
                }
                r as u16
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self {
            n,
            tw: tw.into_boxed_slice(),
            rev,
        }
    }

    /// In-place forward FFT, `X[k] = Σ_n x[n]·exp(-i·2πkn/N)`, unnormalised.
    #[inline]
    fn run(&self, buf: &mut [C]) {
        debug_assert_eq!(buf.len(), self.n);

        for i in 0..self.n {
            let j = self.rev[i] as usize;
            if j > i {
                buf.swap(i, j);
            }
        }

        let mut half = 1usize;
        let mut off = 0usize;
        while half < self.n {
            let step = half * 2;
            let stage = &self.tw[off..off + half];
            let mut k = 0usize;
            while k < self.n {
                let (lo, hi) = buf[k..k + step].split_at_mut(half);
                for j in 0..half {
                    let (wr, wi) = stage[j];
                    let b = hi[j];
                    let tr = wr * b.0 - wi * b.1;
                    let ti = wr * b.1 + wi * b.0;
                    let a = lo[j];
                    hi[j] = (a.0 - tr, a.1 - ti);
                    lo[j] = (a.0 + tr, a.1 + ti);
                }
                k += step;
            }
            off += half;
            half = step;
        }
    }
}

/// An `M`-point DCT-IV evaluated through an `M/2`-point complex FFT.
///
/// The caller's output scale is folded into the post-twiddle, so applying it
/// costs nothing beyond the multiply the post-twiddle already performs.
struct Dct4 {
    m: usize,
    /// `exp(-iπ(4n+1)/(4M))`, `n in 0..M/2`.
    pre: Box<[C]>,
    /// `exp(-iπk/M) · scale`, `k in 0..M/2`.
    post: Box<[C]>,
    fft: Fft,
}

/// Largest `M/2` any AC-3 forward transform needs: the 512-point MDCT folds to
/// a 256-point DCT-IV, whose FFT is 128 points.
const MAX_HALF: usize = 128;

impl Dct4 {
    fn new(m: usize, scale: f32) -> Self {
        let h = m / 2;
        let pre = (0..h)
            .map(|n| {
                let t = -PI * (4 * n + 1) as f32 / (4 * m) as f32;
                (t.cos(), t.sin())
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let post = (0..h)
            .map(|k| {
                let t = -PI * k as f32 / m as f32;
                (t.cos() * scale, t.sin() * scale)
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            m,
            pre,
            post,
            fft: Fft::new(h),
        }
    }

    #[inline]
    fn run(&self, x: &[f32], out: &mut [f32]) {
        let m = self.m;
        let h = m / 2;
        debug_assert_eq!(x.len(), m);
        debug_assert_eq!(out.len(), m);
        debug_assert!(h <= MAX_HALF);

        let mut buf = [(0.0f32, 0.0f32); MAX_HALF];
        let z = &mut buf[..h];
        for n in 0..h {
            let (re, im) = (x[2 * n], x[m - 1 - 2 * n]);
            let (cr, ci) = self.pre[n];
            z[n] = (re * cr - im * ci, im * cr + re * ci);
        }

        self.fft.run(z);

        for k in 0..h {
            let (cr, ci) = self.post[k];
            let u = z[k];
            let vr = u.0 * cr - u.1 * ci;
            let vi = u.1 * cr + u.0 * ci;
            out[2 * k] = vr;
            out[m - 1 - 2 * k] = -vi;
        }
    }
}

/// 256-point DCT-IV carrying the long block's `-1/256` output scale.
fn dct4_256() -> &'static Dct4 {
    static T: OnceLock<Dct4> = OnceLock::new();
    T.get_or_init(|| Dct4::new(256, -1.0 / 256.0))
}

/// 128-point DCT-IV carrying the short block's `-2/256` output scale.
fn dct4_128() -> &'static Dct4 {
    static T: OnceLock<Dct4> = OnceLock::new();
    T.get_or_init(|| Dct4::new(128, -2.0 / 256.0))
}

/// 512-point forward MDCT (§8.2.3.2, α=0 long transform).
///
/// Folds 512 windowed input samples into a 256-point DCT-IV via the TDAC
/// symmetry, then evaluates it through a 128-point complex FFT.
#[inline]
pub fn mdct_512(input: &[f32; 512], output: &mut [f32; 256]) {
    // Time folding into x_sym[256]:
    //   x_sym[127 - q] = -input[256 + q] - input[511 - q]
    //   x_sym[128 + p] =  input[p]       - input[255 - p]
    let mut x_sym = [0.0f32; 256];
    for q in 0..128 {
        x_sym[127 - q] = -input[256 + q] - input[511 - q];
    }
    for p in 0..128 {
        x_sym[128 + p] = input[p] - input[255 - p];
    }

    dct4_256().run(&x_sym, output);
}

/// [`mdct_512`] with the §7.9.5 window applied on the way in.
///
/// The fold reads each windowed sample exactly once, so windowing into a
/// buffer for it to read back is a pass over 512 samples and 2 KiB of traffic
/// that the fold can absorb for free. The encoder runs this 36 times a frame
/// at 5.1, and it is the same arithmetic in the same order — each term is
/// still one multiply, and the pair still combine the way they did.
///
/// The window is symmetric about its centre in the sense §7.9.5 gives it:
/// `w[n]` for the first half, and `w[511 - n]` for the second, which is what
/// puts `WINDOW[255 - q]` and `WINDOW[q]` on the upper terms below.
#[inline]
pub fn mdct_512_windowed(input: &[f32; 512], window: &[f32; 256], output: &mut [f32; 256]) {
    let mut x_sym = [0.0f32; 256];
    for q in 0..128 {
        x_sym[127 - q] = -(input[256 + q] * window[255 - q]) - input[511 - q] * window[q];
    }
    for p in 0..128 {
        x_sym[128 + p] = input[p] * window[p] - input[255 - p] * window[255 - p];
    }

    dct4_256().run(&x_sym, output);
}

/// 256-point forward MDCT for one half of a short-block pair (§8.2.3.2).
///
/// The two halves differ only in their phase term `(π/4)(2k+1)(1+α)`, which is
/// zero for `α = -1` and `(π/2)(2k+1)` for `α = +1`. Both reduce to the same
/// 128-point DCT-IV under a fold, because the DCT-IV kernel is antisymmetric
/// about each multiple of its half-period:
///
/// ```text
///   α = -1:  y[m] = in[m] - in[255 - m]
///   α = +1:  y[m] = -(in[127 - m] + in[m + 128])
/// ```
#[inline]
fn mdct_256_half(input: &[f32; 256], alpha: f32, output: &mut [f32; 128]) {
    let mut y = [0.0f32; 128];
    if alpha < 0.0 {
        for m in 0..128 {
            y[m] = input[m] - input[255 - m];
        }
    } else {
        for m in 0..128 {
            y[m] = -(input[127 - m] + input[m + 128]);
        }
    }

    let mut full = [0.0f32; 128];
    dct4_128().run(&y, &mut full);
    output.copy_from_slice(&full);
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

/// [`mdct_256_pair`] with the §7.9.5 window applied on the way in. See
/// [`mdct_512_windowed`].
///
/// Each half's fold is written against the unwindowed input directly, which
/// also drops the two 256-sample copies the pair used to make to split the
/// window buffer in two.
#[inline]
pub fn mdct_256_pair_windowed(input: &[f32; 512], window: &[f32; 256], output: &mut [f32; 256]) {
    // α = -1 half: y[m] = w[m]·in[m] - w[255-m]·in[255-m].
    let mut y1 = [0.0f32; 128];
    for m in 0..128 {
        y1[m] = input[m] * window[m] - input[255 - m] * window[255 - m];
    }
    // α = +1 half, over the second 256 samples: y[m] = -(w'[127-m]·in[383-m]
    // + w'[m+128]·in[384+m]), with the second half's window index mirrored.
    let mut y2 = [0.0f32; 128];
    for m in 0..128 {
        y2[m] = -(input[383 - m] * window[128 + m] + input[384 + m] * window[127 - m]);
    }

    let mut x1 = [0.0f32; 128];
    let mut x2 = [0.0f32; 128];
    dct4_128().run(&y1, &mut x1);
    dct4_128().run(&y2, &mut x2);

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

/// Reference O(N^2) direct-form short-block half, per §8.2.3.2.
///
/// The oracle `mdct_256_half`'s fold is checked against — without it, an
/// error in the fold would only show up as slightly wrong audio.
#[cfg(test)]
fn mdct_256_half_ref(input: &[f32; 256], alpha: f32, output: &mut [f32; 128]) {
    let n = 256.0f32;
    let scale = -2.0f32 / 256.0f32;
    for k in 0..128 {
        let two_k1 = (2 * k + 1) as f32;
        let phase = PI / 4.0 * two_k1 * (1.0 + alpha);
        let mut s = 0.0f32;
        for nn in 0..256 {
            s += input[nn] * (PI / (2.0 * n) * (2 * nn + 1) as f32 * two_k1 + phase).cos();
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

    /// A tone, an impulse and noise — the impulse in particular exercises every
    /// basis vector at equal weight, so a single mis-signed fold term shows up.
    #[test]
    fn mdct_512_matches_reference_across_signal_shapes() {
        let mut state = 0x1234_5678u32;
        let mut rand = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state as f32 / u32::MAX as f32) * 2.0 - 1.0
        };
        for case in 0..4 {
            let mut input = [0.0f32; 512];
            match case {
                0 => input[173] = 1.0,
                1 => input.iter_mut().for_each(|v| *v = rand()),
                2 => {
                    for (i, v) in input.iter_mut().enumerate() {
                        *v = (i as f32 * 0.013).sin();
                    }
                }
                _ => input.iter_mut().for_each(|v| *v = 1.0),
            }
            let mut a = [0.0f32; 256];
            let mut b = [0.0f32; 256];
            mdct_512_ref(&input, &mut a);
            mdct_512(&input, &mut b);
            let err = (0..256).fold(0.0f32, |m, k| m.max((a[k] - b[k]).abs()));
            assert!(err < 1e-3, "case {case}: max error {err}");
        }
    }

    #[test]
    fn mdct_256_half_matches_reference_for_both_alphas() {
        let mut input = [0.0f32; 256];
        for i in 0..256 {
            input[i] = ((i as f32 * 0.21).sin() * 0.7 + (i as f32 * 1.31).cos() * 0.3) * 0.8;
        }
        for &alpha in &[-1.0f32, 1.0f32] {
            let mut a = [0.0f32; 128];
            let mut b = [0.0f32; 128];
            mdct_256_half_ref(&input, alpha, &mut a);
            mdct_256_half(&input, alpha, &mut b);
            let err = (0..128).fold(0.0f32, |m, k| m.max((a[k] - b[k]).abs()));
            eprintln!("mdct_256_half alpha={alpha:+.0}: max diff {err:.8e}");
            assert!(err < 1e-4, "alpha {alpha}: max error {err}");
        }
    }

    /// The FFT itself, against a direct DFT — separated out so a failure says
    /// whether the transform or the twiddles around it are at fault.
    #[test]
    fn fft_matches_direct_dft() {
        for &n in &[2usize, 4, 8, 64, 128] {
            let fft = Fft::new(n);
            let mut buf: Vec<C> = (0..n)
                .map(|i| ((i as f32 * 0.7).sin(), (i as f32 * 1.9).cos()))
                .collect();
            let src = buf.clone();
            fft.run(&mut buf);
            for k in 0..n {
                let (mut re, mut im) = (0.0f32, 0.0f32);
                for (nn, s) in src.iter().enumerate() {
                    let t = -2.0 * PI * (k * nn % n) as f32 / n as f32;
                    let (c, s2) = (t.cos(), t.sin());
                    re += s.0 * c - s.1 * s2;
                    im += s.1 * c + s.0 * s2;
                }
                assert!(
                    (buf[k].0 - re).abs() < 2e-3 && (buf[k].1 - im).abs() < 2e-3,
                    "n={n} k={k}: got {:?} want ({re}, {im})",
                    buf[k]
                );
            }
        }
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

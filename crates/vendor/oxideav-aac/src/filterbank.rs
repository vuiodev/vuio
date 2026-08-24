//! §4.6.11 Filterbank and block switching — the inverse modified
//! discrete cosine transform (IMDCT), the analysis/synthesis windows
//! (sine and Kaiser-Bessel-derived), and the overlap-add that maps a
//! window-major decoded spectrum back to the time domain.
//!
//! This is the last stage of the per-channel decode chain
//! ([`crate::decoded_spectrum::decode_channel_spectrum`]) for a
//! single channel: it consumes the `num_windows × window_len = 1024`
//! window-major coefficients and emits 1024 PCM-domain samples per
//! frame after overlap-adding against the previous frame's tail.
//!
//! Spec basis (ISO/IEC 14496-3:2001, §4.6.11):
//!
//! * §4.6.11.3.1 — the IMDCT
//!   `x[n] = (2/N) · Σ_k spec[k] · cos((2π/N)·(n + n0)·(k + 1/2))`
//!   for `0 ≤ n < N`, with `n0 = (N/2 + 1)/2`. `N` is the
//!   *transform* window length (2048 for long sequences, 256 for each
//!   of the eight short windows). The crate carries the spectrum at
//!   `N/2` resolution (1024 long, 128 short) as
//!   [`crate::swb_offset::LONG_WINDOW_LEN`] /
//!   [`crate::swb_offset::SHORT_WINDOW_LEN`].
//! * §4.6.11.3.2 — windowing and block switching. The sine window is
//!   `W_SIN(n) = sin((π/N)·(n + 1/2))`; the KBD window is the
//!   normalized running sum of the Kaiser-Bessel kernel `W'(n, α)`
//!   with `α = 4` for the long transform and `α = 6` for the short
//!   transform. The four `window_sequence` shapes
//!   (`ONLY_LONG`, `LONG_START`, `EIGHT_SHORT`, `LONG_STOP`) compose
//!   left/right window halves; the left half's shape is inherited
//!   from the *previous* block's `window_shape`.
//! * §4.6.11.3.3 — the inter-block overlap-add
//!   `out[n] = z[i][n] + z[i-1][n + N/2]` for `0 ≤ n < N/2`,
//!   `N = 2048`, valid for all four sequences.
//!
//! The frame-length-960 (`N = 1920 / 240`) variant of the spec is
//! out of scope: the rest of the crate's `swb_offset` tables and
//! transmission-order machinery are wired to the 1024-coefficient
//! layout, so this module mirrors that and only implements the 2048
//! transform family.

use crate::ics_info::{IcsInfo, WindowSequence, WindowShape};
use crate::swb_offset::{FrameFamily, LONG_WINDOW_LEN, SHORT_WINDOW_LEN};
use crate::Error;

/// `N` for a long-sequence transform (§4.6.11.3.1): 2 ×
/// [`LONG_WINDOW_LEN`].
const LONG_TRANSFORM_LEN: usize = 2 * LONG_WINDOW_LEN as usize; // 2048
/// `N` for a single short-sequence transform: 2 ×
/// [`SHORT_WINDOW_LEN`].
const SHORT_TRANSFORM_LEN: usize = 2 * SHORT_WINDOW_LEN as usize; // 256
/// `M = N_l / N_s` = number of short windows in an `EIGHT_SHORT`
/// sequence.
const NUM_SHORT_WINDOWS: usize = 8;
/// `N_l` — the long transform length, used as the frame's PCM stride.
const N_L: usize = LONG_TRANSFORM_LEN; // 2048
/// `N_s` — the short transform length.
const N_S: usize = SHORT_TRANSFORM_LEN; // 256

/// Result of [`Filterbank::synthesize`]: one frame of
/// `LONG_WINDOW_LEN` (1024) PCM-domain samples for a single channel.
type Result<T> = core::result::Result<T, Error>;

/// §4.6.11.3.1 — inverse MDCT for a length-`n_transform` window.
///
/// `spec` holds the `N/2` transmitted coefficients; the returned
/// vector holds the `N` time-domain values
/// `x[n] = (2/N) · Σ_k spec[k] · cos((2π/N)·(n + n0)·(k + 1/2))`.
///
/// `n0 = (N/2 + 1)/2` is the §4.6.11.3.1 phase offset. The `2/N`
/// scale and the half-coefficient phase are the only normalization
/// the spec attaches to the inverse transform; the energy-correcting
/// window then follows in the per-sequence windowing step.
pub(crate) fn imdct(spec: &[f64], n_transform: usize) -> Vec<f64> {
    let half = n_transform / 2;
    debug_assert_eq!(spec.len(), half);
    let n0 = (half + 1) as f64 / 2.0;
    let scale = 2.0 / n_transform as f64;
    let phase_step = 2.0 * core::f64::consts::PI / n_transform as f64;
    let mut out = vec![0.0f64; n_transform];
    for (n, slot) in out.iter_mut().enumerate() {
        let np = n as f64 + n0;
        let mut acc = 0.0f64;
        for (k, &c) in spec.iter().enumerate() {
            acc += c * (phase_step * np * (k as f64 + 0.5)).cos();
        }
        *slot = scale * acc;
    }
    out
}

/// §4.6.15.3.3 / §4.6.11.3.1 — the forward (analysis) MDCT for a
/// length-`n_transform` window.
///
/// `time` holds the `N` windowed time-domain values `z[n]`; the
/// returned vector holds the `N/2` spectral coefficients
/// `X[k] = 2 · Σ_n z[n] · cos((2π/N)·(n + n0)·(k + 1/2))`,
/// `0 ≤ k < N/2`, with the §4.6.11.3.1 phase `n0 = (N/2 + 1)/2`.
///
/// This is the exact analysis pair of [`imdct`]: the IMDCT carries the
/// `2/N` scale, the analysis here carries the matching factor `2`, so
/// the windowed-and-overlap-added round trip is unity for a
/// power-complementary §4.6.11.3.2 window. The same transform is the
/// `MDCT(x_est)` of the §4.6.7.3 Long-Term-Prediction loop.
pub(crate) fn forward_mdct(time: &[f64], n_transform: usize) -> Vec<f64> {
    let half = n_transform / 2;
    debug_assert_eq!(time.len(), n_transform);
    let n0 = (half + 1) as f64 / 2.0;
    let step = 2.0 * core::f64::consts::PI / n_transform as f64;
    (0..half)
        .map(|k| {
            2.0 * time
                .iter()
                .enumerate()
                .map(|(n, &t)| t * (step * (n as f64 + n0) * (k as f64 + 0.5)).cos())
                .sum::<f64>()
        })
        .collect()
}

/// §4.6.11.3.2 — build the `ONLY_LONG_SEQUENCE` analysis window
/// `[W_LEFT_l | W_RIGHT_l]` at the family's long transform length,
/// with the family's window style (the LD families map
/// `window_shape == 1` to the §4.6.17.2.3 low-overlap window).
///
/// Exposed for the §4.6.7.3 LTP loop, which windows the predicted time
/// signal `x_est` with the current long window before the analysis
/// [`forward_mdct`]. (LTP is restricted to long windows, §4.6.7.1.)
pub(crate) fn long_only_window_family(
    family: FrameFamily,
    left_shape: WindowShape,
    right_shape: WindowShape,
) -> Vec<f64> {
    let n_l = family.long_transform_len();
    let halves = window_halves_style(
        n_l,
        left_shape,
        right_shape,
        WindowStyle::for_family(family),
    );
    let half_l = n_l / 2;
    let mut w = vec![0.0f64; n_l];
    w[..half_l].copy_from_slice(&halves.left);
    for (m, &rv) in halves.right.iter().enumerate() {
        w[half_l + m] = rv;
    }
    w
}

/// §4.6.11.3.2 — assemble the length-2048 window for any of the
/// three long-transform sequences. The window is shared between the
/// decoder's synthesis ([`Filterbank::long_window`] delegates here)
/// and the encoder's analysis (the §4.6.11 filterbank is its own
/// transpose up to the TDAC fold, so the same window applies on both
/// sides). Returns [`Error::FilterbankInvalid`] for
/// `EIGHT_SHORT_SEQUENCE` — use [`short_window_j`] per short window
/// instead.
pub(crate) fn long_sequence_window(
    sequence: WindowSequence,
    left_shape: WindowShape,
    right_shape: WindowShape,
) -> Result<Vec<f64>> {
    long_sequence_window_n(N_L, N_S, sequence, left_shape, right_shape)
}

/// §4.6.11.3.2 — the [`long_sequence_window`] construction generalized
/// to an arbitrary `(n_l, n_s)` transform family. The SSR gain-control
/// filterbank (§4.6.12.1) runs the same window geometry at
/// `(512, 64)` — one quarter of the standard family — per band.
pub(crate) fn long_sequence_window_n(
    n_l: usize,
    n_s: usize,
    sequence: WindowSequence,
    left_shape: WindowShape,
    right_shape: WindowShape,
) -> Result<Vec<f64>> {
    let kind = match sequence {
        WindowSequence::OnlyLong => LongKind::OnlyLong,
        WindowSequence::LongStart => LongKind::Start,
        WindowSequence::LongStop => LongKind::Stop,
        WindowSequence::EightShort => return Err(Error::FilterbankInvalid),
    };
    Ok(build_long_window_n(n_l, n_s, left_shape, right_shape, kind))
}

/// §4.6.11.3.2 c) — the length-256 window of short window `j`
/// (`0..8`) inside an `EIGHT_SHORT_SEQUENCE` frame: window 0's left
/// half inherits the previous block's shape, all other halves use
/// this block's shape.
pub(crate) fn short_window_j(
    j: usize,
    left_shape: WindowShape,
    right_shape: WindowShape,
) -> Vec<f64> {
    short_window_n(N_S, j, left_shape, right_shape)
}

/// §4.6.11.3.2 c) — [`short_window_j`] generalized to an arbitrary
/// short-transform length `n_s` (64 for the SSR §4.6.12.1 per-band
/// family).
pub(crate) fn short_window_n(
    n_s: usize,
    j: usize,
    left_shape: WindowShape,
    right_shape: WindowShape,
) -> Vec<f64> {
    let this_left = if j == 0 { left_shape } else { right_shape };
    let halves = window_halves(n_s, this_left, right_shape);
    let mut w = vec![0.0f64; n_s];
    w[..n_s / 2].copy_from_slice(&halves.left);
    for (m, &rv) in halves.right.iter().enumerate() {
        w[n_s / 2 + m] = rv;
    }
    w
}

/// §4.6.11.3.2 c) — offset of short window 0 inside the 2048-sample
/// frame window region: `(N_l − N_s)/4 = 448`.
pub(crate) const SHORT_SEQ_START: usize = (N_L - N_S) / 4;

/// §4.6.11.3.2 c) — hop between successive short windows:
/// `N_s/2 = 128`.
pub(crate) const SHORT_SEQ_HOP: usize = N_S / 2;

/// Modified Bessel function of the first kind, order 0, via its power
/// series `I0(x) = Σ_k ((x/2)^k / k!)^2` (§4.6.11.3.2). The series
/// converges quickly for the `x = π·α` arguments the KBD window uses
/// (`α ∈ {4, 6}`), so a fixed term cap with an early-out on negligible
/// terms is exact to f64 precision.
fn bessel_i0(x: f64) -> f64 {
    let half_x = x / 2.0;
    let mut term = 1.0f64; // k = 0 term: (half_x^0 / 0!)^2 = 1
    let mut sum = 1.0f64;
    let mut k = 1.0f64;
    loop {
        // term_k = term_{k-1} · (half_x / k)^2
        term *= (half_x / k) * (half_x / k);
        sum += term;
        if term <= sum * 1e-18 {
            break;
        }
        k += 1.0;
        if k > 256.0 {
            break;
        }
    }
    sum
}

/// §4.6.11.3.2 — the Kaiser-Bessel kernel
/// `W'(n, α) = I0(π·α·sqrt(1 − ((n − N/4)/(N/4))^2)) / I0(π·α)`
/// for `0 ≤ n ≤ N/2`, evaluated over `0..=half` (`half = N/2`).
fn kbd_kernel(half: usize, alpha: f64) -> Vec<f64> {
    let quarter = half as f64 / 2.0; // N/4
    let denom = bessel_i0(core::f64::consts::PI * alpha);
    (0..=half)
        .map(|n| {
            let t = (n as f64 - quarter) / quarter;
            let radicand = (1.0 - t * t).max(0.0);
            bessel_i0(core::f64::consts::PI * alpha * radicand.sqrt()) / denom
        })
        .collect()
}

/// §4.6.11.3.2 — the left half of the KBD window:
/// `W_KBD_LEFT(n) = sqrt( Σ_{p=0..n} W'(p) / Σ_{p=0..N/2} W'(p) )`
/// for `0 ≤ n < N/2`. Returns the `half = N/2` left-half samples.
///
/// `alpha` is 4 for the long transform and 6 for the short transform.
fn kbd_left(half: usize, alpha: f64) -> Vec<f64> {
    let kernel = kbd_kernel(half, alpha);
    let total: f64 = kernel.iter().sum();
    let mut running = 0.0f64;
    let mut out = Vec::with_capacity(half);
    for &w in kernel.iter().take(half) {
        running += w;
        out.push((running / total).sqrt());
    }
    out
}

/// §4.6.11.3.2 — the sine window left half
/// `W_SIN_LEFT(n) = sin((π/N)·(n + 1/2))`, `0 ≤ n < N/2`. Returns the
/// `half = N/2` samples.
fn sine_left(half: usize) -> Vec<f64> {
    let n_transform = (2 * half) as f64;
    (0..half)
        .map(|n| (core::f64::consts::PI / n_transform * (n as f64 + 0.5)).sin())
        .collect()
}

/// One transform's analysis/synthesis window halves, each `half = N/2`
/// long. The right half of a sine/KBD window is the mirror of its
/// left half (`W_RIGHT(n) = W_LEFT(N − 1 − n)`), so we store left
/// halves and index the right half by mirror at apply time.
struct WindowHalves {
    /// Left half, indices `0..half`.
    left: Vec<f64>,
    /// Right half, indices `0..half`; element `m` is the window value
    /// at transform position `half + m`.
    right: Vec<f64>,
}

/// §4.6.17.2.3 Table 4.171 — the ER AAC LD *low-overlap* window's
/// left half. Over the full length-`N` window:
///
/// ```text
/// W(i) = 0                                   i in [0, 3N/16)
///        sin(π(i − 3N/16 + 0.5) / (N/4))     i in [3N/16, 5N/16)
///        1                                   i in [5N/16, 11N/16)
///        sin(π(i − 9N/16 + 0.5) / (N/4))     i in [11N/16, 13N/16)
///        0                                   i in [13N/16, N)
/// ```
///
/// The two sine segments' arguments sum to π at mirrored positions
/// (`i` and `N − 1 − i`), so the right half is the exact spatial
/// mirror of this left half — the same mirror convention every other
/// window shape uses — and the TDAC partners inside the rise region
/// have arguments summing to π/2, making the window
/// power-complementary (`sin² + cos² = 1`), as §4.6.11.3.2 requires
/// for perfect reconstruction.
fn low_overlap_left(half: usize) -> Vec<f64> {
    let n = 2 * half; // full window length N (1024 or 960)
    let rise_start = 3 * n / 16;
    let rise_end = 5 * n / 16;
    let quarter = n as f64 / 4.0;
    (0..half)
        .map(|i| {
            if i < rise_start {
                0.0
            } else if i < rise_end {
                (core::f64::consts::PI * (i as f64 - rise_start as f64 + 0.5) / quarter).sin()
            } else {
                1.0
            }
        })
        .collect()
}

/// Which window family the `window_shape` bit selects between —
/// §4.6.11.3.2 (sine / KBD) for the general families, §4.6.17.2.3
/// Table 4.171 (sine / low-overlap) for ER AAC LD.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum WindowStyle {
    /// `window_shape == 1` selects the Kaiser-Bessel-derived window.
    Standard,
    /// `window_shape == 1` selects the §4.6.17.2.3 low-overlap
    /// window (ER AAC LD).
    LowDelay,
}

impl WindowStyle {
    /// The style a [`FrameFamily`] mandates.
    pub(crate) fn for_family(family: FrameFamily) -> Self {
        if family.is_ld() {
            WindowStyle::LowDelay
        } else {
            WindowStyle::Standard
        }
    }
}

/// Build the left half for the requested `shape` at transform length
/// `n_transform` under a [`WindowStyle`].
///
/// The KBD kernel alpha follows the transform's *role*: the long
/// transform of a family uses `α = 4`, the short transform `α = 6`.
/// §4.6.11.3.2 states this for the 2048/256 (1920/240) family; for the
/// SSR 512/64 family the same pair reproduces the normative
/// Table 4.A.14 / Table 4.A.13 window listings (each printed value
/// matches the α = 4 / α = 6 running-sum construction to the tables'
/// print precision — pinned by the `ssr_kbd_*` tests below). Under
/// [`WindowStyle::LowDelay`] the `window_shape == 1` bit selects the
/// §4.6.17.2.3 low-overlap window instead of KBD.
fn half_window_style(n_transform: usize, shape: WindowShape, style: WindowStyle) -> Vec<f64> {
    let half = n_transform / 2;
    match (shape, style) {
        (WindowShape::Sine, _) => sine_left(half),
        (WindowShape::Kbd, WindowStyle::LowDelay) => low_overlap_left(half),
        (WindowShape::Kbd, WindowStyle::Standard) => {
            let alpha = match n_transform {
                // Long transforms: 2048 (1920) per §4.6.11.3.2; 512 per
                // the Table 4.A.14 SSR window fit.
                2048 | 1920 | 512 => 4.0,
                // Short transforms: 256 (240) per §4.6.11.3.2; 64 per
                // the Table 4.A.13 SSR window fit.
                _ => 6.0,
            };
            kbd_left(half, alpha)
        }
    }
}

/// §4.6.11.3.2 — assemble a transform's window from a `left` shape
/// (inherited from the previous block) and a `right` shape (this
/// block's `window_shape`). For a sine/KBD window the right half is
/// the spatial mirror of that shape's *left* half, so we build the
/// `right`-shape left half and reverse it.
fn window_halves(
    n_transform: usize,
    left_shape: WindowShape,
    right_shape: WindowShape,
) -> WindowHalves {
    window_halves_style(n_transform, left_shape, right_shape, WindowStyle::Standard)
}

/// [`window_halves`] with an explicit [`WindowStyle`].
fn window_halves_style(
    n_transform: usize,
    left_shape: WindowShape,
    right_shape: WindowShape,
    style: WindowStyle,
) -> WindowHalves {
    let left = half_window_style(n_transform, left_shape, style);
    let mut right = half_window_style(n_transform, right_shape, style);
    right.reverse();
    WindowHalves { left, right }
}

/// The stateful per-channel §4.6.11 filterbank. One instance per
/// decoded channel; [`Filterbank::synthesize`] is called once per
/// frame and carries the overlap-add tail (`z[i-1][n + N/2]`) plus the
/// previous block's `window_shape` (which determines the left-half
/// shape of the next block, §4.6.11.3.2) across calls.
#[derive(Clone, Debug)]
pub struct Filterbank {
    /// The §4.5.1.1 frame-length family this filterbank synthesizes
    /// (transform lengths, overlap length, and — for LD — the
    /// §4.6.17.2.3 window style). Fixed at construction; a frame
    /// whose `ics_info.family` disagrees is rejected.
    family: FrameFamily,
    /// `z[i-1][N/2 .. N]` — the right half of the previous frame's
    /// windowed time signal, added to the left half of this frame's
    /// windowed signal (§4.6.11.3.3). `family.frame_len()` long.
    overlap: Vec<f64>,
    /// `window_shape` of the previous block, governing the left-half
    /// window shape of the next block. [`None`] before the first
    /// frame: per §4.6.11.3.2 the first block's left and right halves
    /// share its own `window_shape`.
    prev_shape: Option<WindowShape>,
}

impl Default for Filterbank {
    fn default() -> Self {
        Self::new()
    }
}

impl Filterbank {
    /// A fresh filterbank with a zeroed overlap buffer and no
    /// previous-block shape (so the first frame uses its own
    /// `window_shape` for both halves, per §4.6.11.3.2).
    pub fn new() -> Self {
        Self::new_family(FrameFamily::Lc1024)
    }

    /// A fresh filterbank for an arbitrary §4.5.1.1 [`FrameFamily`]:
    /// the 1024 / 960 block-switching families or the long-only LD
    /// 512 / 480 families (whose `window_shape == 1` selects the
    /// §4.6.17.2.3 low-overlap window in place of KBD).
    pub fn new_family(family: FrameFamily) -> Self {
        Filterbank {
            family,
            overlap: vec![0.0f64; family.frame_len()],
            prev_shape: None,
        }
    }

    /// The [`FrameFamily`] this filterbank was constructed for.
    pub fn family(&self) -> FrameFamily {
        self.family
    }

    /// §4.6.7.3 — the current frame's *aliased half window*
    /// `x_rec(0 … N/2 − 1)`: the right half of the just-synthesized
    /// frame's windowed (pre-overlap-add) time signal `z[i][N/2 … N]`.
    ///
    /// After a [`Self::synthesize`] call the internal overlap buffer
    /// holds exactly this tail (it is reused as the *next* frame's
    /// overlap-add term, §4.6.11.3.3). The LTP reconstruction history
    /// ([`crate::ltp::LtpState`]) needs the same vector — its
    /// `x_rec(0 … N/2 − 1)` region — so the element driver reads it here
    /// after each synthesis and feeds it to
    /// [`crate::ltp::LtpState::push_frame`]. Before the first frame this
    /// is the zero buffer, matching the §4.6.7.3 zero initialisation.
    pub fn aliased_tail(&self) -> &[f64] {
        &self.overlap
    }

    /// §4.6.11.3.2 — the previous block's `window_shape`, which governs
    /// the left-half shape of the *next* block's analysis/synthesis
    /// window. [`None`] before the first frame (the first block uses its
    /// own shape for both halves).
    ///
    /// The §4.6.7.4.1 LTP analysis MDCT must window `x_est` with the
    /// same composite long window the filterbank uses for this frame, so
    /// the element driver reads the previous shape here before
    /// synthesizing.
    pub fn prev_shape(&self) -> Option<WindowShape> {
        self.prev_shape
    }

    /// §4.6.11 — synthesize one frame of `LONG_WINDOW_LEN` (1024) PCM
    /// samples from `spec`, the window-major decoded spectrum produced
    /// by [`crate::decoded_spectrum::decode_channel_spectrum`].
    ///
    /// `spec` must be:
    ///
    /// * `LONG_WINDOW_LEN` (1024) coefficients for `ONLY_LONG`,
    ///   `LONG_START`, `LONG_STOP`;
    /// * `8 × SHORT_WINDOW_LEN` (1024 total) for `EIGHT_SHORT`,
    ///   laid out window-major: window `w` at `spec[w * 128 ..]`.
    ///
    /// The result is the §4.6.11.3.3 overlap-added output; the method
    /// updates the internal overlap tail and previous-block shape for
    /// the next call.
    ///
    /// Errors: [`Error::FilterbankInvalid`] if `spec.len()` disagrees
    /// with `ics_info.window_sequence`.
    pub fn synthesize(&mut self, spec: &[f64], ics_info: &IcsInfo) -> Result<Vec<f64>> {
        if ics_info.family != self.family {
            return Err(Error::FilterbankInvalid);
        }
        let z = self.windowed_signal(spec, ics_info)?;
        debug_assert_eq!(z.len(), self.family.long_transform_len());

        // §4.6.11.3.3 overlap-add: out[n] = z[i][n] + z[i-1][n + N/2].
        let half = self.family.frame_len();
        let out: Vec<f64> = z[..half]
            .iter()
            .zip(self.overlap.iter())
            .map(|(&zn, &on)| zn + on)
            .collect();

        // Retain z[i][N/2 .. N] as next frame's z[i-1][n + N/2].
        self.overlap.clear();
        self.overlap.extend_from_slice(&z[half..]);

        // §4.6.11.3.2: the left-half shape of the *next* block is this
        // block's window_shape.
        self.prev_shape = Some(ics_info.window_shape);
        Ok(out)
    }

    /// §4.6.11.3.1 + §4.6.11.3.2 — produce the full-length (`N_l =
    /// 2048`) windowed time signal `z[i][n]` for this frame, before
    /// the inter-block overlap-add. Dispatches on `window_sequence`.
    fn windowed_signal(&self, spec: &[f64], ics_info: &IcsInfo) -> Result<Vec<f64>> {
        let left_shape = self.prev_shape.unwrap_or(ics_info.window_shape);
        let right_shape = ics_info.window_shape;
        match ics_info.window_sequence {
            WindowSequence::OnlyLong => {
                self.long_windowed(spec, left_shape, right_shape, LongKind::OnlyLong)
            }
            WindowSequence::LongStart => {
                self.long_windowed(spec, left_shape, right_shape, LongKind::Start)
            }
            WindowSequence::LongStop => {
                self.long_windowed(spec, left_shape, right_shape, LongKind::Stop)
            }
            WindowSequence::EightShort => self.short_windowed(spec, left_shape, right_shape),
        }
    }

    /// §4.6.11.3.2 a)/b)/d) — the three long-transform sequences. Each
    /// runs a single length-2048 IMDCT and applies a composite window
    /// whose left half (`ONLY_LONG`, `LONG_START`) or right half
    /// (`LONG_STOP`) is the full long half-window, and whose other
    /// half is shaped by the start/stop transition (a short half-window
    /// flanked by a flat `1.0` plateau and a zero region).
    fn long_windowed(
        &self,
        spec: &[f64],
        left_shape: WindowShape,
        right_shape: WindowShape,
        kind: LongKind,
    ) -> Result<Vec<f64>> {
        if spec.len() != self.family.frame_len() {
            return Err(Error::FilterbankInvalid);
        }
        let x = imdct(spec, self.family.long_transform_len());
        let w = self.long_window(left_shape, right_shape, kind)?;
        let z: Vec<f64> = x.iter().zip(w.iter()).map(|(&xv, &wv)| xv * wv).collect();
        Ok(z)
    }

    /// §4.6.11.3.2 — assemble the length-2048 window vector for a
    /// long-transform sequence.
    ///
    /// * `OnlyLong` (a): `[W_LEFT_l | W_RIGHT_l]`.
    /// * `Start` (b): left half is `W_LEFT_l`; the right half is a
    ///   flat `1.0` plateau over `[N_l/2, (3N_l − N_s)/4)`, the short
    ///   right half-window over `[(3N_l − N_s)/4, (3N_l + N_s)/4)`, and
    ///   `0.0` over `[(3N_l + N_s)/4, N_l)`.
    /// * `Stop` (d): the left half is `0.0` over `[0, (N_l − N_s)/4)`,
    ///   the short left half-window over `[(N_l − N_s)/4, (N_l +
    ///   N_s)/4)`, and a flat `1.0` plateau over `[(N_l + N_s)/4,
    ///   N_l/2)`; the right half is `W_RIGHT_l`.
    fn long_window(
        &self,
        left_shape: WindowShape,
        right_shape: WindowShape,
        kind: LongKind,
    ) -> Result<Vec<f64>> {
        let n_l = self.family.long_transform_len();
        match self.family.short_transform_len() {
            Some(n_s) => Ok(build_long_window_style(
                n_l,
                n_s,
                left_shape,
                right_shape,
                kind,
                WindowStyle::for_family(self.family),
            )),
            // LD: long-only — Start / Stop transitions do not exist
            // (§4.6.17.2.2), so only the OnlyLong composite is legal.
            None => match kind {
                LongKind::OnlyLong => {
                    let halves =
                        window_halves_style(n_l, left_shape, right_shape, WindowStyle::LowDelay);
                    let half_l = n_l / 2;
                    let mut w = vec![0.0f64; n_l];
                    w[..half_l].copy_from_slice(&halves.left);
                    for (m, &rv) in halves.right.iter().enumerate() {
                        w[half_l + m] = rv;
                    }
                    Ok(w)
                }
                _ => Err(Error::LdShortWindow),
            },
        }
    }
}

/// §4.6.11.3.2 — [`build_long_window`] generalized to an arbitrary
/// `(n_l, n_s)` transform family; every breakpoint is the spec's
/// `N_l`/`N_s` expression evaluated at the caller's lengths (the
/// standard family passes `(2048, 256)`, the SSR §4.6.12.1 per-band
/// family `(512, 64)`).
fn build_long_window_n(
    n_l: usize,
    n_s: usize,
    left_shape: WindowShape,
    right_shape: WindowShape,
    kind: LongKind,
) -> Vec<f64> {
    build_long_window_style(
        n_l,
        n_s,
        left_shape,
        right_shape,
        kind,
        WindowStyle::Standard,
    )
}

/// [`build_long_window_n`] with an explicit [`WindowStyle`] (the LD
/// families map `window_shape == 1` to the §4.6.17.2.3 low-overlap
/// window; the LD long-only path never reaches the Start / Stop
/// composites, but the parameterization keeps the construction
/// uniform).
fn build_long_window_style(
    n_l: usize,
    n_s: usize,
    left_shape: WindowShape,
    right_shape: WindowShape,
    kind: LongKind,
    style: WindowStyle,
) -> Vec<f64> {
    let long = window_halves_style(n_l, left_shape, right_shape, style);
    let short = window_halves_style(n_s, left_shape, right_shape, style);
    let half_l = n_l / 2;
    let mut w = vec![0.0f64; n_l];

    // Left half is always the plain long left half for OnlyLong /
    // Start; Stop replaces it with the start-transition mirror.
    match kind {
        LongKind::OnlyLong | LongKind::Start => {
            w[..half_l].copy_from_slice(&long.left);
        }
        LongKind::Stop => {
            // 0.0 over [0, (N_l − N_s)/4); short left half over
            // [(N_l − N_s)/4, (N_l + N_s)/4); 1.0 over
            // [(N_l + N_s)/4, N_l/2).
            let a = (n_l - n_s) / 4;
            for (m, &sv) in short.left.iter().enumerate() {
                w[a + m] = sv;
            }
            for slot in w.iter_mut().take(half_l).skip(a + n_s / 2) {
                *slot = 1.0;
            }
        }
    }

    match kind {
        LongKind::OnlyLong => {
            for (m, &rv) in long.right.iter().enumerate() {
                w[half_l + m] = rv;
            }
        }
        LongKind::Start => {
            // 1.0 over [N_l/2, (3N_l − N_s)/4); short right half
            // over [(3N_l − N_s)/4, (3N_l + N_s)/4); 0.0 after.
            let b = (3 * n_l - n_s) / 4;
            for slot in w.iter_mut().take(b).skip(half_l) {
                *slot = 1.0;
            }
            for (m, &rv) in short.right.iter().enumerate() {
                w[b + m] = rv;
            }
            // [(3N_l + N_s)/4, N_l) stays 0.0 from the vec init.
        }
        LongKind::Stop => {
            for (m, &rv) in long.right.iter().enumerate() {
                w[half_l + m] = rv;
            }
        }
    }
    w
}

impl Filterbank {
    /// §4.6.11.3.2 c) — the `EIGHT_SHORT` sequence: eight length-256
    /// IMDCTs, each windowed with a short window, then overlapped and
    /// added into the 2048-sample frame with leading/trailing zeros.
    ///
    /// Window-shape inheritance (§4.6.11.3.2): the *first* short
    /// window's left half uses the previous block's shape; every
    /// later short window's left half — and every short window's right
    /// half — uses this block's `window_shape`.
    fn short_windowed(
        &self,
        spec: &[f64],
        left_shape: WindowShape,
        right_shape: WindowShape,
    ) -> Result<Vec<f64>> {
        let n_s = self
            .family
            .short_transform_len()
            .ok_or(Error::LdShortWindow)?;
        let n_l = self.family.long_transform_len();
        let short_len = n_s / 2; // 128 (120)
        if spec.len() != NUM_SHORT_WINDOWS * short_len {
            return Err(Error::FilterbankInvalid);
        }

        // Per-window windowed length-N_s time signals.
        let mut windowed: Vec<Vec<f64>> = Vec::with_capacity(NUM_SHORT_WINDOWS);
        for j in 0..NUM_SHORT_WINDOWS {
            let coeffs = &spec[j * short_len..(j + 1) * short_len];
            let x = imdct(coeffs, n_s);
            // W_0 left half inherits the previous block's shape; all
            // other windows' left halves use this block's shape.
            let this_left = if j == 0 { left_shape } else { right_shape };
            let halves = window_halves(n_s, this_left, right_shape);
            let mut z = vec![0.0f64; n_s];
            for n in 0..n_s / 2 {
                z[n] = x[n] * halves.left[n];
            }
            for n in n_s / 2..n_s {
                z[n] = x[n] * halves.right[n - n_s / 2];
            }
            windowed.push(z);
        }

        // §4.6.11.3.2 c) overlap-add of the eight short windows into a
        // N_l-sample frame. Short window `j` starts at offset
        // `(N_l − N_s)/4 + j·N_s/2` (each successive short window is
        // hopped by N_s/2 = 128 (120) samples) — the spec's piecewise
        // z_{i,n} is exactly this 50%-overlap-add with the first
        // window placed at (N_l − N_s)/4 = 448 (420).
        let mut z = vec![0.0f64; n_l];
        let start = (n_l - n_s) / 4; // 448 (420)
        let hop = n_s / 2; // 128 (120)
        for (j, win) in windowed.iter().enumerate() {
            let base = start + j * hop;
            for (n, &v) in win.iter().enumerate() {
                z[base + n] += v;
            }
        }
        Ok(z)
    }
}

/// Discriminates the three long-transform `window_sequence` shapes
/// inside [`Filterbank::long_window`].
#[derive(Clone, Copy)]
enum LongKind {
    OnlyLong,
    Start,
    Stop,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ics_info::IcsInfo;

    fn long_info(shape: WindowShape, seq: WindowSequence) -> IcsInfo {
        IcsInfo {
            family: crate::swb_offset::FrameFamily::Lc1024,
            ics_reserved_bit: false,
            window_sequence: seq,
            window_shape: shape,
            max_sfb: 49,
            scale_factor_grouping: None,
            predictor_data_present: false,
            predictor_data: None,
            ltp_data_present: false,
            ltp_data: None,
            ltp_data_present_pair: None,
            ltp_data_pair: None,
            num_windows: 1,
            num_window_groups: 1,
            window_group_length: vec![1],
            num_swb: 49,
        }
    }

    fn short_info(shape: WindowShape) -> IcsInfo {
        IcsInfo {
            family: crate::swb_offset::FrameFamily::Lc1024,
            ics_reserved_bit: false,
            window_sequence: WindowSequence::EightShort,
            window_shape: shape,
            max_sfb: 14,
            scale_factor_grouping: Some(0),
            predictor_data_present: false,
            predictor_data: None,
            ltp_data_present: false,
            ltp_data: None,
            ltp_data_present_pair: None,
            ltp_data_pair: None,
            num_windows: 8,
            num_window_groups: 8,
            window_group_length: vec![1; 8],
            num_swb: 14,
        }
    }

    #[test]
    fn sine_window_endpoints() {
        // W_SIN_LEFT(n) = sin((π/N)(n + 1/2)); for N = 2048 the first
        // sample is sin(π·0.5/2048) and the last left-half sample is
        // sin(π·1023.5/2048) ≈ sin(π/2 · 0.9995…).
        let left = sine_left(1024);
        assert_eq!(left.len(), 1024);
        let expect0 = (core::f64::consts::PI * 0.5 / 2048.0).sin();
        assert!((left[0] - expect0).abs() < 1e-15);
        // The window rises monotonically to ~1.0 at the centre.
        assert!(left[1023] > 0.9999 && left[1023] <= 1.0);
        for w in 1..1024 {
            assert!(left[w] > left[w - 1]);
        }
    }

    #[test]
    fn sine_window_unit_power_overlap() {
        // The sine window satisfies the Princen-Bradley condition:
        // W(n)^2 + W(n + N/2)^2 = 1 for a symmetric sine window. Build
        // a full OnlyLong sine window and check the squared-sum of the
        // overlapping halves is 1.
        let half = sine_left(1024);
        for n in 0..1024 {
            // Right half mirrors the left: W(N-1-n) = W_left(n).
            let wl = half[n];
            let wr = half[1023 - n]; // W(1024 + n) = W_left(1023 - n)
            let s = wl * wl + wr * wr;
            assert!((s - 1.0).abs() < 1e-12, "n={n} sum={s}");
        }
    }

    #[test]
    fn kbd_window_unit_power_overlap() {
        // The KBD window is constructed precisely so that
        // W(n)^2 + W(n + N/2)^2 = 1 (it is the canonical
        // perfect-reconstruction window). Verify against the long α=4
        // KBD window.
        let left = kbd_left(1024, 4.0);
        assert_eq!(left.len(), 1024);
        for n in 0..1024 {
            let wl = left[n];
            let wr = left[1023 - n];
            let s = wl * wl + wr * wr;
            assert!((s - 1.0).abs() < 1e-12, "n={n} sum={s}");
        }
        // KBD is monotonically increasing on its left half.
        for n in 1..1024 {
            assert!(left[n] >= left[n - 1]);
        }
    }

    #[test]
    fn bessel_i0_known_values() {
        // I0(0) = 1; I0(1) ≈ 1.2660658777520084;
        // I0(2) ≈ 2.2795853023360673 (standard tabulated values).
        assert!((bessel_i0(0.0) - 1.0).abs() < 1e-15);
        assert!((bessel_i0(1.0) - 1.266_065_877_752_008_4).abs() < 1e-12);
        assert!((bessel_i0(2.0) - 2.279_585_302_336_067_3).abs() < 1e-12);
    }

    #[test]
    fn imdct_dc_coefficient() {
        // A single non-zero spec[0] is a pure cosine basis function.
        // For N=8, half=4, n0=(4+1)/2=2.5: x[n] = (2/8)·cos((2π/8)(n+2.5)(0.5)).
        let n = 8usize;
        let spec = [1.0, 0.0, 0.0, 0.0];
        let x = imdct(&spec, n);
        let scale = 2.0 / 8.0;
        let n0 = 2.5;
        for (idx, &xv) in x.iter().enumerate() {
            let expect =
                scale * (2.0 * core::f64::consts::PI / 8.0 * (idx as f64 + n0) * 0.5).cos();
            assert!((xv - expect).abs() < 1e-15, "n={idx}");
        }
    }

    /// Time-domain aliasing cancellation (TDAC): for a windowed MDCT/
    /// IMDCT pair, two consecutive identical frames overlap-add to
    /// reconstruct the windowed input exactly in the steady state. We
    /// drive the filterbank with the production analysis [`forward_mdct`]
    /// of a known signal and confirm perfect reconstruction over the
    /// second frame. (The analysis/synthesis pair is unity for a
    /// power-complementary §4.6.11.3.2 window.)
    use super::forward_mdct;

    /// The full symmetric (sine) `OnlyLong` window, length `N`.
    fn long_sine_window() -> Vec<f64> {
        let left = sine_left(1024);
        let mut w = vec![0.0; LONG_TRANSFORM_LEN];
        w[..1024].copy_from_slice(&left);
        for m in 0..1024 {
            w[1024 + m] = left[1023 - m];
        }
        w
    }

    #[test]
    fn tdac_perfect_reconstruction_sine_long() {
        // Streaming time-domain aliasing cancellation. A long input is
        // analysed by a 50%-overlap forward MDCT (analysis window =
        // sine), each frame carried through the decoder's IMDCT +
        // synthesis window + overlap-add. For a power-complementary
        // window the central frames reconstruct the input exactly.
        //
        // The forward analysis used here is the transpose of the
        // decoder's §4.6.11.3.1 IMDCT basis with NO scale (the IMDCT
        // carries the 2/N), so the analysis/synthesis pair satisfies
        // TDAC for the sine window.
        let n = LONG_TRANSFORM_LEN; // 2048
        let hop = n / 2; // 1024
        let win = long_sine_window();

        // A long deterministic input; reconstruct the central hop.
        let total = 5 * hop;
        let input: Vec<f64> = (0..total)
            .map(|i| (0.013 * i as f64).sin() + 0.5 * (0.07 * i as f64).cos())
            .collect();

        let info = long_info(WindowShape::Sine, WindowSequence::OnlyLong);
        let mut fb = Filterbank::new();

        // Run four overlapping analysis frames (starts 0, 1024, 2048,
        // 3072), feeding each frame's MDCT to the filterbank. Collect
        // the decoder's per-frame outputs.
        let mut outputs = Vec::new();
        for f in 0..4 {
            let base = f * hop;
            let frame: Vec<f64> = (0..n)
                .map(|m| {
                    let idx = base + m;
                    if idx < total {
                        input[idx] * win[m]
                    } else {
                        0.0
                    }
                })
                .collect();
            let spec = forward_mdct(&frame, n);
            outputs.push(fb.synthesize(&spec, &info).unwrap());
        }

        // The decoder output for frame f covers input samples
        // [f·hop, f·hop + hop). The steady-state frames f = 1, 2
        // reconstruct the input (their window region is fully covered
        // by both the analysis-window taper and the overlap from the
        // neighbouring frames).
        for (f, out) in outputs.iter().enumerate().take(3).skip(1) {
            let base = f * hop;
            for k in 0..hop {
                let recon = out[k];
                let expect = input[base + k];
                assert!(
                    (recon - expect).abs() < 1e-9,
                    "frame={f} k={k} recon={recon} expect={expect}"
                );
            }
        }
    }

    /// Family-parameterized streaming TDAC harness: analyse a
    /// deterministic input with the 50%-overlap forward MDCT under
    /// the family's own long window, run the decoder filterbank, and
    /// require exact reconstruction on the steady-state frames.
    fn tdac_long_family(family: crate::swb_offset::FrameFamily, shape: WindowShape) {
        let n = family.long_transform_len();
        let hop = n / 2;
        let style = WindowStyle::for_family(family);
        let win = {
            let left = half_window_style(n, shape, style);
            let mut w = vec![0.0; n];
            w[..hop].copy_from_slice(&left);
            for m in 0..hop {
                w[hop + m] = left[hop - 1 - m];
            }
            w
        };
        let total = 5 * hop;
        let input: Vec<f64> = (0..total)
            .map(|i| (0.017 * i as f64).sin() + 0.4 * (0.043 * i as f64).cos())
            .collect();
        let mut info = long_info(shape, WindowSequence::OnlyLong);
        info.family = family;
        info.num_swb = 40; // geometry-irrelevant here
        let mut fb = Filterbank::new_family(family);
        let mut outputs = Vec::new();
        for f in 0..4 {
            let base = f * hop;
            let frame: Vec<f64> = (0..n)
                .map(|m| {
                    let idx = base + m;
                    if idx < total {
                        input[idx] * win[m]
                    } else {
                        0.0
                    }
                })
                .collect();
            let spec = forward_mdct(&frame, n);
            let out = fb.synthesize(&spec, &info).unwrap();
            assert_eq!(out.len(), family.frame_len());
            outputs.push(out);
        }
        for (f, out) in outputs.iter().enumerate().take(3).skip(1) {
            let base = f * hop;
            for k in 0..hop {
                assert!(
                    (out[k] - input[base + k]).abs() < 1e-9,
                    "{:?} {:?} frame={f} k={k}",
                    family,
                    shape
                );
            }
        }
    }

    #[test]
    fn tdac_lc960_sine_and_kbd() {
        tdac_long_family(crate::swb_offset::FrameFamily::Lc960, WindowShape::Sine);
        tdac_long_family(crate::swb_offset::FrameFamily::Lc960, WindowShape::Kbd);
    }

    #[test]
    fn tdac_ld512_sine_and_low_overlap() {
        // Under the LD families the window_shape == 1 bit selects the
        // §4.6.17.2.3 low-overlap window (Table 4.171).
        tdac_long_family(crate::swb_offset::FrameFamily::Ld512, WindowShape::Sine);
        tdac_long_family(crate::swb_offset::FrameFamily::Ld512, WindowShape::Kbd);
    }

    #[test]
    fn tdac_ld480_sine_and_low_overlap() {
        tdac_long_family(crate::swb_offset::FrameFamily::Ld480, WindowShape::Sine);
        tdac_long_family(crate::swb_offset::FrameFamily::Ld480, WindowShape::Kbd);
    }

    #[test]
    fn low_overlap_window_regions_and_pr() {
        // §4.6.17.2.3: zeros over [0, 3N/16), sine rise over
        // [3N/16, 5N/16), flat 1.0 over [5N/16, N/2) on the left
        // half; power-complementary at the TDAC partners.
        for n in [1024usize, 960] {
            let half = n / 2;
            let left = low_overlap_left(half);
            assert_eq!(left.len(), half);
            for (i, &v) in left.iter().enumerate().take(3 * n / 16) {
                assert_eq!(v, 0.0, "N={n} i={i}");
            }
            for (i, &v) in left.iter().enumerate().take(half).skip(5 * n / 16) {
                assert_eq!(v, 1.0, "N={n} i={i}");
            }
            // Monotone rise inside [3N/16, 5N/16).
            for i in 3 * n / 16 + 1..5 * n / 16 {
                assert!(left[i] > left[i - 1], "N={n} i={i}");
            }
            // Princen-Bradley: W(n)² + W(N/2−1−n)² = 1 over the half.
            for i in 0..half {
                let s = left[i] * left[i] + left[half - 1 - i] * left[half - 1 - i];
                assert!((s - 1.0).abs() < 1e-12, "N={n} i={i} s={s}");
            }
        }
    }

    #[test]
    fn ld_filterbank_rejects_non_only_long() {
        use crate::swb_offset::FrameFamily;
        let mut fb = Filterbank::new_family(FrameFamily::Ld512);
        let mut info = long_info(WindowShape::Sine, WindowSequence::LongStart);
        info.family = FrameFamily::Ld512;
        let spec = vec![0.0; 512];
        assert!(matches!(
            fb.synthesize(&spec, &info),
            Err(Error::LdShortWindow)
        ));
    }

    #[test]
    fn family_mismatch_rejected() {
        use crate::swb_offset::FrameFamily;
        let mut fb = Filterbank::new_family(FrameFamily::Lc1024);
        let mut info = long_info(WindowShape::Sine, WindowSequence::OnlyLong);
        info.family = FrameFamily::Lc960;
        let spec = vec![0.0; 960];
        assert!(matches!(
            fb.synthesize(&spec, &info),
            Err(Error::FilterbankInvalid)
        ));
    }

    #[test]
    fn eight_short_lc960_tdac() {
        // The 960-family EIGHT_SHORT: 8 × 120-line windows (240-point
        // transforms) at start 420, hop 120. A steady sine input
        // through forward-MDCT analysis per short window must
        // reconstruct inside the fully-overlapped interior region of
        // the frame's central section.
        use crate::swb_offset::FrameFamily;
        let family = FrameFamily::Lc960;
        let n_s = 240usize;
        let hop = 120usize;
        let start = 420usize;
        let win = {
            let left = half_window_style(n_s, WindowShape::Sine, WindowStyle::Standard);
            let mut w = vec![0.0; n_s];
            w[..hop].copy_from_slice(&left);
            for m in 0..hop {
                w[hop + m] = left[hop - 1 - m];
            }
            w
        };
        // Input signal over the frame's 1920-sample window region.
        let input: Vec<f64> = (0..1920).map(|i| (0.05 * i as f64).sin() * 0.7).collect();
        // Analyse the eight short windows.
        let mut spec = Vec::with_capacity(8 * hop);
        for j in 0..8 {
            let base = start + j * hop;
            let frame: Vec<f64> = (0..n_s).map(|m| input[base + m] * win[m]).collect();
            spec.extend(forward_mdct(&frame, n_s));
        }
        let mut info = short_info(WindowShape::Sine);
        info.family = family;
        info.num_swb = 14;
        let mut fb = Filterbank::new_family(family);
        // Prime the overlap with the previous frame's tail = zeros; the
        // first output frame covers window-region samples [0, 960).
        let out = fb.synthesize(&spec, &info).unwrap();
        assert_eq!(out.len(), 960);
        // Interior of the short-window train that lands in the first
        // output half: [start + hop, 960) = [540, 960) is covered by
        // two overlapping short windows each (TDAC-complete).
        for k in 540..960 {
            assert!(
                (out[k] - input[k]).abs() < 1e-9,
                "k={k} out={} in={}",
                out[k],
                input[k]
            );
        }
    }

    #[test]
    fn tdac_perfect_reconstruction_kbd_long() {
        // Same streaming TDAC check with the KBD (α=4) long window.
        let n = LONG_TRANSFORM_LEN;
        let hop = n / 2;
        let win = {
            let left = kbd_left(1024, 4.0);
            let mut w = vec![0.0; n];
            w[..1024].copy_from_slice(&left);
            for m in 0..1024 {
                w[1024 + m] = left[1023 - m];
            }
            w
        };
        let total = 5 * hop;
        let input: Vec<f64> = (0..total)
            .map(|i| 0.3 * (0.02 * i as f64).cos() - 0.6 * (0.05 * i as f64).sin())
            .collect();
        let info = long_info(WindowShape::Kbd, WindowSequence::OnlyLong);
        let mut fb = Filterbank::new();
        let mut outputs = Vec::new();
        for f in 0..4 {
            let base = f * hop;
            let frame: Vec<f64> = (0..n)
                .map(|m| {
                    let idx = base + m;
                    if idx < total {
                        input[idx] * win[m]
                    } else {
                        0.0
                    }
                })
                .collect();
            let spec = forward_mdct(&frame, n);
            outputs.push(fb.synthesize(&spec, &info).unwrap());
        }
        for (f, out) in outputs.iter().enumerate().take(3).skip(1) {
            let base = f * hop;
            for k in 0..hop {
                assert!((out[k] - input[base + k]).abs() < 1e-9, "frame={f} k={k}");
            }
        }
    }

    #[test]
    fn eight_short_internal_tdac() {
        // §4.6.11.3.2 c): the eight short windows overlap-add inside
        // the frame with a 128-sample hop, the first window placed at
        // offset (N_l − N_s)/4 = 448. Drive the eight short MDCTs from
        // a streaming short-window analysis of a continuous input and
        // confirm the frame's interior reconstructs that input over
        // the fully-overlapped central short windows.
        let n_s = SHORT_TRANSFORM_LEN; // 256
        let hop = n_s / 2; // 128
        let sine_short = {
            let left = sine_left(hop);
            let mut w = vec![0.0; n_s];
            w[..hop].copy_from_slice(&left);
            for m in 0..hop {
                w[hop + m] = left[hop - 1 - m];
            }
            w
        };
        // A continuous input long enough to cover all eight short
        // windows once placed at start=448, hop=128: last window starts
        // at 448 + 7·128 = 1344, ends at 1600.
        let total = N_L;
        let input: Vec<f64> = (0..total)
            .map(|i| (0.05 * i as f64).sin() + 0.4 * (0.11 * i as f64).cos())
            .collect();
        let start = (N_L - N_S) / 4; // 448

        // Build the eight short windows' MDCTs from the windowed input
        // segments at the same offsets the decoder overlaps them.
        let mut spec = Vec::with_capacity(NUM_SHORT_WINDOWS * SHORT_WINDOW_LEN as usize);
        for j in 0..NUM_SHORT_WINDOWS {
            let base = start + j * hop;
            let seg: Vec<f64> = (0..n_s).map(|m| input[base + m] * sine_short[m]).collect();
            let s = forward_mdct(&seg, n_s);
            spec.extend_from_slice(&s);
        }

        let info = short_info(WindowShape::Sine);
        let mut fb = Filterbank::new();
        let out = fb.synthesize(&spec, &info).unwrap();

        // The output frame is z[0:1024]; overlap with the (zero) prior
        // frame leaves the interior intact. The central short windows
        // j=1..6 are fully overlapped by their neighbours, so the
        // reconstructed signal equals the input over their shared
        // central hops: input indices [start + hop, start + 7·hop).
        // The decoder output covers input [0, 1024); the short-window
        // region [start, 1600) is partly past 1024, so check the
        // covered central hops [start+hop, 1024).
        for idx in (start + hop)..1024 {
            assert!(
                (out[idx] - input[idx]).abs() < 1e-9,
                "idx={idx} out={} input={}",
                out[idx],
                input[idx]
            );
        }
    }

    #[test]
    fn synthesize_long_length_and_shape() {
        let info = long_info(WindowShape::Sine, WindowSequence::OnlyLong);
        let mut fb = Filterbank::new();
        let spec = vec![0.25f64; LONG_WINDOW_LEN as usize];
        let out = fb.synthesize(&spec, &info).unwrap();
        assert_eq!(out.len(), LONG_WINDOW_LEN as usize);
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn synthesize_eight_short_length() {
        let info = short_info(WindowShape::Sine);
        let mut fb = Filterbank::new();
        let spec = vec![0.1f64; NUM_SHORT_WINDOWS * SHORT_WINDOW_LEN as usize];
        let out = fb.synthesize(&spec, &info).unwrap();
        assert_eq!(out.len(), LONG_WINDOW_LEN as usize);
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn synthesize_rejects_wrong_length() {
        let info = long_info(WindowShape::Sine, WindowSequence::OnlyLong);
        let mut fb = Filterbank::new();
        let spec = vec![0.0f64; 512];
        assert!(matches!(
            fb.synthesize(&spec, &info),
            Err(Error::FilterbankInvalid)
        ));
        let sinfo = short_info(WindowShape::Sine);
        let mut fb2 = Filterbank::new();
        let bad = vec![0.0f64; 1000];
        assert!(matches!(
            fb2.synthesize(&bad, &sinfo),
            Err(Error::FilterbankInvalid)
        ));
    }

    #[test]
    fn start_window_plateau_and_zero_regions() {
        // LONG_START: left half is the long left window, then a flat
        // 1.0 plateau, then the short right half, then zeros.
        let fb = Filterbank::new();
        let w = fb
            .long_window(WindowShape::Sine, WindowShape::Sine, LongKind::Start)
            .unwrap();
        assert_eq!(w.len(), N_L);
        // Plateau region [1024, 1472) is all 1.0.
        for v in w.iter().take(1472).skip(1024) {
            assert!((*v - 1.0).abs() < 1e-15);
        }
        // Tail [1600, 2048) is all 0.0.  (3N_l + N_s)/4 = 1600.
        for v in w.iter().take(N_L).skip(1600) {
            assert_eq!(*v, 0.0);
        }
        // The short-right transition [1472, 1600) falls from 1 to 0.
        assert!(w[1472] > w[1599]);
    }

    #[test]
    fn stop_window_zero_and_plateau_regions() {
        // LONG_STOP: leading zeros, short left half, 1.0 plateau, then
        // the long right window.
        let fb = Filterbank::new();
        let w = fb
            .long_window(WindowShape::Sine, WindowShape::Sine, LongKind::Stop)
            .unwrap();
        assert_eq!(w.len(), N_L);
        // Leading [0, 448) zeros. (N_l − N_s)/4 = 448.
        for v in w.iter().take(448) {
            assert_eq!(*v, 0.0);
        }
        // Plateau [576, 1024) all 1.0. (N_l + N_s)/4 = 576.
        for v in w.iter().take(1024).skip(576) {
            assert!((*v - 1.0).abs() < 1e-15);
        }
        // The short-left transition [448, 576) rises from 0 to 1.
        assert!(w[448] < w[575]);
    }

    #[test]
    fn first_frame_uses_own_shape_for_left_half() {
        // Before any frame, prev_shape is None, so the first frame's
        // left half uses its own window_shape (KBD here). Confirm the
        // left half equals the KBD left window, not the sine one.
        let info = long_info(WindowShape::Kbd, WindowSequence::OnlyLong);
        let fb = Filterbank::new();
        let w = fb
            .windowed_signal(&vec![0.0; LONG_WINDOW_LEN as usize], &info)
            .unwrap();
        // All-zero spectrum → zero time signal regardless, so instead
        // inspect the window directly.
        let _ = w;
        let win = fb
            .long_window(WindowShape::Kbd, WindowShape::Kbd, LongKind::OnlyLong)
            .unwrap();
        let kbd = kbd_left(1024, 4.0);
        for n in 0..1024 {
            assert!((win[n] - kbd[n]).abs() < 1e-15);
        }
    }

    /// Table 4.A.13 — the normative Kaiser-Bessel window for the AAC
    /// SSR object type `EIGHT_SHORT_SEQUENCE` (`N = 64`): all 32
    /// tabulated left-half values, transcribed from the spec PDF. The
    /// running-sum KBD construction with the short-transform `α = 6`
    /// reproduces every entry to the table's print precision.
    #[test]
    fn ssr_kbd_short_window_matches_table_4_a_13() {
        // Verbatim table transcription — keep every printed digit,
        // including redundant trailing zeros.
        #[allow(clippy::excessive_precision)]
        const TABLE_4_A_13: [(usize, f64); 32] = [
            (0, 0.0000875914060105),
            (1, 0.0009321760265333),
            (2, 0.0032114611466596),
            (3, 0.0081009893216786),
            (4, 0.0171240286619181),
            (5, 0.0320720743527833),
            (6, 0.0548307856028528),
            (7, 0.0871361822564870),
            (8, 0.1302923415174603),
            (9, 0.1848955425508276),
            (10, 0.2506163195331889),
            (11, 0.3260874142923209),
            (12, 0.4089316830907141),
            (13, 0.4959414909423747),
            (14, 0.5833939894958904),
            (15, 0.6674601983218376),
            (16, 0.7446454751465113),
            (17, 0.8121892962974020),
            (18, 0.8683559394406505),
            (19, 0.9125649996381605),
            (20, 0.9453396205809574),
            (21, 0.9680864942677585),
            (22, 0.9827581789763112),
            (23, 0.9914756203467121),
            (24, 0.9961964092194694),
            (25, 0.9984956609571091),
            (26, 0.9994855586984285),
            (27, 0.9998533730714648),
            (28, 0.9999671864476404),
            (29, 0.9999948432453556),
            (30, 0.9999995655238333),
            (31, 0.9999999961638728),
        ];
        let left = half_window_style(64, WindowShape::Kbd, WindowStyle::Standard);
        assert_eq!(left.len(), 32);
        for &(i, expect) in &TABLE_4_A_13 {
            assert!(
                (left[i] - expect).abs() < 1e-8,
                "Table 4.A.13 w({i}): got {} expect {expect}",
                left[i]
            );
        }
        // Discriminator: the long-transform α = 4 does NOT fit.
        let alt = kbd_left(32, 4.0);
        assert!((alt[0] - TABLE_4_A_13[0].1).abs() > 1e-4);
    }

    /// Table 4.A.14 — the normative Kaiser-Bessel window for the SSR
    /// object type's other window sequences (`N = 512`): a spread of
    /// tabulated left-half values transcribed from the spec PDF. The
    /// running-sum KBD construction with the long-transform `α = 4`
    /// reproduces each to the table's print precision.
    #[test]
    fn ssr_kbd_long_window_matches_table_4_a_14() {
        // Verbatim table transcription — keep every printed digit,
        // including redundant trailing zeros.
        #[allow(clippy::excessive_precision)]
        const TABLE_4_A_14_SPREAD: [(usize, f64); 15] = [
            (0, 0.0005851230124487),
            (1, 0.0009642149851497),
            (2, 0.0013558207534965),
            (16, 0.0116765080854300),
            (32, 0.0405466983507029),
            (64, 0.1811734433685097),
            (96, 0.4325622561631607),
            (128, 0.7110428359000029),
            (160, 0.9058173183656508),
            (192, 0.9845850806232530),
            (224, 0.9992757396582338),
            (240, 0.9999442511639580),
            (250, 0.9999962619864214),
            (254, 0.9999995351446231),
            (255, 0.9999998288155155),
        ];
        let left = half_window_style(512, WindowShape::Kbd, WindowStyle::Standard);
        assert_eq!(left.len(), 256);
        for &(i, expect) in &TABLE_4_A_14_SPREAD {
            assert!(
                (left[i] - expect).abs() < 1e-8,
                "Table 4.A.14 w({i}): got {} expect {expect}",
                left[i]
            );
        }
        // Discriminator: the short-transform α = 6 does NOT fit.
        let alt = kbd_left(256, 6.0);
        assert!((alt[0] - TABLE_4_A_14_SPREAD[0].1).abs() > 1e-4);
    }

    /// The generalized `(n_l, n_s)` long-window builder reproduces the
    /// standard-family construction exactly, and the SSR family's
    /// breakpoints land at the quarter-scaled positions.
    #[test]
    fn generalized_long_window_matches_standard_and_scales() {
        for kind in [LongKind::OnlyLong, LongKind::Start, LongKind::Stop] {
            let std = Filterbank::new()
                .long_window(WindowShape::Sine, WindowShape::Sine, kind)
                .unwrap();
            let gen = build_long_window_n(2048, 256, WindowShape::Sine, WindowShape::Sine, kind);
            assert_eq!(std, gen);
        }
        // SSR LONG_START at (512, 64): 1.0 plateau over [256, 368),
        // short descent over [368, 400), zero over [400, 512).
        let w = build_long_window_n(
            512,
            64,
            WindowShape::Sine,
            WindowShape::Sine,
            LongKind::Start,
        );
        assert_eq!(w.len(), 512);
        for v in w.iter().take(368).skip(256) {
            assert!((*v - 1.0).abs() < 1e-15);
        }
        assert!(w[368] < 1.0 && w[368] > w[399]);
        for v in w.iter().skip(400) {
            assert_eq!(*v, 0.0);
        }
        // SSR LONG_STOP mirrors: zero over [0, 112), ascent [112, 144),
        // plateau [144, 256).
        let w = build_long_window_n(
            512,
            64,
            WindowShape::Sine,
            WindowShape::Sine,
            LongKind::Stop,
        );
        for v in w.iter().take(112) {
            assert_eq!(*v, 0.0);
        }
        for v in w.iter().take(256).skip(144) {
            assert!((*v - 1.0).abs() < 1e-15);
        }
    }

    /// The SSR-family windows are TDAC power-complementary at every
    /// steady overlap: `w(n)² + w(n + N/2)²` over the flanks sums to 1
    /// for the 512 `ONLY_LONG` window (both shapes), which is the
    /// §4.6.11 perfect-reconstruction condition the §4.6.12.3.3
    /// per-band overlap relies on.
    #[test]
    fn ssr_only_long_window_is_power_complementary() {
        for shape in [WindowShape::Sine, WindowShape::Kbd] {
            let w = build_long_window_n(512, 64, shape, shape, LongKind::OnlyLong);
            for n in 0..256 {
                let s = w[n] * w[n] + w[n + 256] * w[n + 256];
                assert!(
                    (s - 1.0).abs() < 1e-10,
                    "{shape:?} w²({n}) + w²({}) = {s}",
                    n + 256
                );
            }
        }
    }
}

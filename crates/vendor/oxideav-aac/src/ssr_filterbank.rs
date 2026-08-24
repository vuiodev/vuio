//! SSR front-half filterbank — ISO/IEC 14496-3 §4.6.12.1 (matching
//! ISO/IEC 13818-7 §16.1): the spectrum → PQF-band de-interleave, the
//! even-band spectral reversal, and the per-band 256/32-line IMDCTs
//! with the quarter-scale §4.6.11.3.2 windows.
//!
//! When the gain control tool is active (the SSR object type, AOT 3),
//! the §4.6.11 filterbank configuration changes (§4.6.12.1):
//!
//! * the IMDCT is 256 lines instead of 1024 (one per PQF band) for the
//!   long window sequences, and 32 lines instead of 128 (eight per
//!   band) for `EIGHT_SHORT_SEQUENCE`;
//! * "the filter bank tool outputs a total of 2048 non-overlapped
//!   values per frame" — four bands × 512 windowed samples, handed to
//!   the §4.6.12.3.3 gain-control windowing/overlap stage as
//!   `U_{W,B}(j)`;
//! * "the order of the MDCT coefficients in each even PQF band must be
//!   reversed … exchanging the higher frequency MDCT coefficients with
//!   the lower frequency MDCT coefficients".
//!
//! ## The spectrum → band arrangement
//!
//! The PQF splits the input into "four equal width frequency bands"
//! (Annex C.2.1.1), band `B` covering the `B`-th quarter of the
//! spectrum in ascending frequency (its modulator is centred on
//! `(2B+1)π/8`). The transmitted spectrum keeps the ordinary
//! ascending-frequency coefficient order (the §4.5.2.3 scalefactor-band
//! machinery runs on it unchanged), so band `B`'s 256 (long) / 32
//! (short, per window) coefficient column is the contiguous quarter
//! `spec[256·B ..][..256]` / `spec[128·w + 32·B ..][..32]`.
//!
//! ## Which bands are "even"
//!
//! The §4.6.12.2 definitions count IPQF bands ordinally — `max_band`
//! is defined over "the 2nd / 3rd / 4th IPQF band" — so the "even PQF
//! band[s]" whose coefficients are reversed are the 2nd and 4th, i.e.
//! 0-based bands 1 and 3. This is also forced by the filterbank
//! mathematics: decimating band `B` by four spectrally inverts the
//! odd-indexed (0-based) bands, so exactly those bands need the
//! reversal for the assembled spectrum to be frequency-ascending. The
//! `tone_lands_at_its_spectral_bin` test pins this against the Annex
//! C.2.1.1 analysis PQF: a pure tone encoded through the PQF → MDCT →
//! reversal chain peaks at its global spectral bin only under this
//! convention (bands 1 and 3 mirror without it).
//!
//! ## Provenance
//!
//! Transform sizes, output layout and the reversal rule are the
//! §4.6.12.1 / §16.1 prose; the window geometry is §4.6.11.3.2
//! evaluated at the `(512, 64)` family with the KBD windows pinned
//! against Tables 4.A.13 / 4.A.14; the validation PQF is the Annex
//! C.2.1.1 formula. All from the spec PDFs staged under
//! `docs/audio/aac/`. No external SSR implementation was consulted.

use crate::filterbank::{imdct, long_sequence_window_n, short_window_n};
use crate::ics_info::{IcsInfo, WindowSequence, WindowShape};
use crate::ipqf::NUM_BANDS;
use crate::Error;

type Result<T> = core::result::Result<T, Error>;

/// The SSR per-band long transform length (§4.6.12.1: 256 lines →
/// `N = 512`).
pub const SSR_LONG_TRANSFORM: usize = 512;
/// The SSR per-band short transform length (§4.6.12.1: 32 lines →
/// `N = 64`).
pub const SSR_SHORT_TRANSFORM: usize = 64;
/// Spectral lines per band for the long window sequences.
pub const BAND_LINES_LONG: usize = SSR_LONG_TRANSFORM / 2; // 256
/// Spectral lines per band per short window.
pub const BAND_LINES_SHORT: usize = SSR_SHORT_TRANSFORM / 2; // 32
/// Short windows in an `EIGHT_SHORT_SEQUENCE`.
const NUM_SHORT_WINDOWS: usize = 8;
/// Non-overlapped windowed samples each band contributes per frame
/// (§4.6.12.1: `4 × 512 = 2048` total).
pub const BAND_SAMPLES_PER_FRAME: usize = SSR_LONG_TRANSFORM;

/// §4.6.12.1 — split the frame's 1024 decoded spectral coefficients
/// into the four PQF-band coefficient columns, applying the even-band
/// (0-based 1 and 3, see the module notes) spectral reversal.
///
/// * Long sequences: `spec` is the 1024-line frequency-ascending
///   spectrum; band `B`'s column is `spec[256·B ..][..256]`, reversed
///   for bands 1 and 3.
/// * `EIGHT_SHORT_SEQUENCE`: `spec` is window-major (window `w` at
///   `spec[128·w ..][..128]`); band `B`'s column concatenates the
///   eight per-window quarters `spec[128·w + 32·B ..][..32]` (each
///   reversed for bands 1 and 3), so it is itself window-major.
///
/// Errors with [`Error::FilterbankInvalid`] if `spec` is not 1024
/// coefficients.
pub fn split_bands(spec: &[f64], seq: WindowSequence) -> Result<[Vec<f64>; NUM_BANDS]> {
    if spec.len() != NUM_BANDS * BAND_LINES_LONG {
        return Err(Error::FilterbankInvalid);
    }
    let mut bands: [Vec<f64>; NUM_BANDS] =
        core::array::from_fn(|_| Vec::with_capacity(BAND_LINES_LONG));
    match seq {
        WindowSequence::EightShort => {
            for w in 0..NUM_SHORT_WINDOWS {
                let win =
                    &spec[w * (NUM_BANDS * BAND_LINES_SHORT)..][..NUM_BANDS * BAND_LINES_SHORT];
                for (b, band) in bands.iter_mut().enumerate() {
                    let col = &win[b * BAND_LINES_SHORT..][..BAND_LINES_SHORT];
                    if b % 2 == 1 {
                        band.extend(col.iter().rev());
                    } else {
                        band.extend_from_slice(col);
                    }
                }
            }
        }
        _ => {
            for (b, band) in bands.iter_mut().enumerate() {
                let col = &spec[b * BAND_LINES_LONG..][..BAND_LINES_LONG];
                if b % 2 == 1 {
                    band.extend(col.iter().rev());
                } else {
                    band.extend_from_slice(col);
                }
            }
        }
    }
    Ok(bands)
}

/// The stateful SSR front-half synthesis for one channel: the
/// §4.6.12.1 band split + per-band IMDCT + quarter-scale §4.6.11.3.2
/// windowing, producing the non-overlapped `U_{W,B}(j)` columns the
/// §4.6.12.3.3 gain-control stage consumes.
///
/// Carries the previous block's `window_shape` across frames (the left
/// half of every window inherits it, §4.6.11.3.2 — the SSR family
/// keeps the standard inheritance rule).
#[derive(Debug, Clone, Default)]
pub struct SsrSynthesis {
    /// `window_shape` of the previous block; `None` before the first
    /// frame (the first block uses its own shape for both halves).
    prev_shape: Option<WindowShape>,
}

impl SsrSynthesis {
    /// A fresh front half with no previous-block shape.
    #[must_use]
    pub fn new() -> Self {
        SsrSynthesis::default()
    }

    /// Produce the four per-band non-overlapped windowed columns
    /// `U_{W,B}` for one frame.
    ///
    /// `spec` is the frame's decoded 1024-line spectrum (window-major
    /// for `EIGHT_SHORT_SEQUENCE`). Each returned column holds
    /// [`BAND_SAMPLES_PER_FRAME`] (512) samples: a single windowed
    /// 512-sample block for the long sequences, or eight windowed
    /// 64-sample blocks concatenated window-major for
    /// `EIGHT_SHORT_SEQUENCE` — exactly the `u` layout
    /// [`crate::gain_control::GainBandState::window_overlap`] expects.
    pub fn windowed_bands(
        &mut self,
        spec: &[f64],
        ics_info: &IcsInfo,
    ) -> Result<[Vec<f64>; NUM_BANDS]> {
        let left_shape = self.prev_shape.unwrap_or(ics_info.window_shape);
        let right_shape = ics_info.window_shape;
        let seq = ics_info.window_sequence;
        let cols = split_bands(spec, seq)?;

        let mut out: [Vec<f64>; NUM_BANDS] = core::array::from_fn(|_| Vec::new());
        match seq {
            WindowSequence::EightShort => {
                // Eight per-band 32-line IMDCTs, each windowed with the
                // 64-sample short window (window 0's left half inherits
                // the previous block's shape). No intra-sequence
                // overlap-add here: §4.6.12.3.3 performs it after the
                // gain is applied.
                for (band, col) in out.iter_mut().zip(cols.iter()) {
                    let mut u = Vec::with_capacity(BAND_SAMPLES_PER_FRAME);
                    for w in 0..NUM_SHORT_WINDOWS {
                        let lines = &col[w * BAND_LINES_SHORT..][..BAND_LINES_SHORT];
                        let x = imdct(lines, SSR_SHORT_TRANSFORM);
                        let win = short_window_n(SSR_SHORT_TRANSFORM, w, left_shape, right_shape);
                        u.extend(x.iter().zip(win.iter()).map(|(&xv, &wv)| xv * wv));
                    }
                    band.extend_from_slice(&u);
                }
            }
            _ => {
                let win = long_sequence_window_n(
                    SSR_LONG_TRANSFORM,
                    SSR_SHORT_TRANSFORM,
                    seq,
                    left_shape,
                    right_shape,
                )?;
                for (band, col) in out.iter_mut().zip(cols.iter()) {
                    let x = imdct(col, SSR_LONG_TRANSFORM);
                    band.extend(x.iter().zip(win.iter()).map(|(&xv, &wv)| xv * wv));
                }
            }
        }

        self.prev_shape = Some(right_shape);
        Ok(out)
    }
}

/// Test-side mirror of the encoder PQF (Annex C.2.1.1), shared by the
/// front-half tests here and the full round-trip tests in
/// [`crate::ssr`].
#[cfg(test)]
pub(crate) mod pqf_test_support {
    use super::NUM_BANDS;
    use crate::ipqf::{prototype, PROTO_LEN};
    use core::f64::consts::PI;

    /// Annex C.2.1.1 — the encoder-side PQF analysis coefficients
    /// `h_i(n) = (1/4)·cos((2i+1)(2n+5)π/16)·Q(n)`, `0 ≤ n ≤ 95`,
    /// with `Q` the Table 4.110 prototype (test-side mirror of the
    /// §4.6.12.3.4 IPQF).
    pub(crate) fn analysis_coefs() -> [[f64; PROTO_LEN]; NUM_BANDS] {
        let q = prototype();
        core::array::from_fn(|i| {
            core::array::from_fn(|n| {
                0.25 * ((2.0 * i as f64 + 1.0) * (2.0 * n as f64 + 5.0) * PI / 16.0).cos() * q[n]
            })
        })
    }

    /// Critically-sampled PQF analysis: band sample
    /// `X_B(m) = Σ_n h_B(n)·x(4m + 3 − n)` — each band sample consumes
    /// one block of four new input samples (the `+3` reads up to the
    /// newest sample of block `m`; the resulting analysis+synthesis
    /// cascade delay is [`PQF_CASCADE_DELAY`] full-rate samples).
    pub(crate) fn pqf_analysis(x: &[f64]) -> [Vec<f64>; NUM_BANDS] {
        let h = analysis_coefs();
        let m_len = x.len() / NUM_BANDS;
        core::array::from_fn(|b| {
            (0..m_len)
                .map(|m| {
                    let mut acc = 0.0f64;
                    for (n, &hn) in h[b].iter().enumerate() {
                        let idx = 4 * m as isize + 3 - n as isize;
                        if idx >= 0 {
                            if let Some(&xv) = x.get(idx as usize) {
                                acc += hn * xv;
                            }
                        }
                    }
                    acc
                })
                .collect()
        })
    }

    /// Full-rate delay of the Annex C.2.1.1 analysis → §4.6.12.3.4
    /// synthesis cascade with the `+3` analysis alignment (measured by
    /// the near-perfect-reconstruction test).
    pub(crate) const PQF_CASCADE_DELAY: usize = 92;
}

#[cfg(test)]
mod tests {
    use super::pqf_test_support::{pqf_analysis, PQF_CASCADE_DELAY};
    use super::*;
    use crate::filterbank::forward_mdct;
    use crate::ipqf::Ipqf;
    use core::f64::consts::PI;

    /// The analysis PQF and the IPQF are a near-perfect-reconstruction
    /// pair: white input round-trips within the prototype's stopband
    /// leakage (measured ≈ 2.9e-4 err/sig) at a flat 92-sample delay.
    #[test]
    fn pqf_ipqf_cascade_is_near_perfect_reconstruction() {
        // Deterministic pseudo-random input.
        let mut state = 0x1234_5678u32;
        let mut rnd = || {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            (state >> 8) as f64 / (1u32 << 24) as f64 - 0.5
        };
        let x: Vec<f64> = (0..4000).map(|_| rnd()).collect();
        let bands = pqf_analysis(&x);
        let refs: [&[f64]; NUM_BANDS] = core::array::from_fn(|b| bands[b].as_slice());
        let mut ipqf = Ipqf::new();
        let y = ipqf.synthesize(&refs, bands[0].len());

        let (mut err, mut sig) = (0.0f64, 0.0f64);
        for n in 500..2500 {
            let d = y[n + PQF_CASCADE_DELAY] - x[n];
            err += d * d;
            sig += x[n] * x[n];
        }
        let ratio = (err / sig).sqrt();
        assert!(ratio < 1e-3, "cascade err/sig = {ratio}");
        // Discriminator: a wrong delay is nowhere near.
        let mut err_bad = 0.0f64;
        for n in 500..2500 {
            let d = y[n + PQF_CASCADE_DELAY + 4] - x[n];
            err_bad += d * d;
        }
        assert!((err_bad / sig).sqrt() > 0.1);
    }

    /// §4.6.12.1 — a pure tone at global spectral bin `k`, encoded
    /// through the Annex C.2.1.1 PQF → per-band windowed MDCT →
    /// even-band reversal → contiguous quarters, peaks at bin `k`.
    /// Without the reversal, the band-1 / band-3 tones mirror inside
    /// their quarter — this pins both the split arrangement and the
    /// reversal convention (0-based bands 1 and 3).
    #[test]
    fn tone_lands_at_its_spectral_bin() {
        let win: Vec<f64> = (0..SSR_LONG_TRANSFORM)
            .map(|n| (PI / SSR_LONG_TRANSFORM as f64 * (n as f64 + 0.5)).sin())
            .collect();
        // One tone per PQF band.
        for &k_target in &[100usize, 300, 550, 800] {
            let f = (k_target as f64 + 0.5) * PI / 1024.0;
            let x: Vec<f64> = (0..8192).map(|n| (f * n as f64).sin()).collect();
            let bands = pqf_analysis(&x);

            // Steady ONLY_LONG frame over band samples [768, 1280).
            let mut spec = vec![0.0f64; 1024];
            let mut spec_unreversed = vec![0.0f64; 1024];
            for b in 0..NUM_BANDS {
                let z: Vec<f64> = (0..SSR_LONG_TRANSFORM)
                    .map(|n| bands[b][768 + n] * win[n])
                    .collect();
                let mut coeffs = forward_mdct(&z, SSR_LONG_TRANSFORM);
                spec_unreversed[256 * b..256 * b + 256].copy_from_slice(&coeffs);
                if b % 2 == 1 {
                    coeffs.reverse();
                }
                spec[256 * b..256 * b + 256].copy_from_slice(&coeffs);
            }
            let peak = |s: &[f64]| {
                (0..s.len())
                    .max_by(|&a, &b| s[a].abs().partial_cmp(&s[b].abs()).unwrap())
                    .unwrap()
            };
            let got = peak(&spec);
            assert!(
                got.abs_diff(k_target) <= 2,
                "tone k={k_target} peaked at {got}"
            );
            let got_unrev = peak(&spec_unreversed);
            if k_target / 256 % 2 == 1 {
                // Bands 1 and 3 mirror without the reversal.
                let band = k_target / 256;
                let mirrored = 256 * band + (255 - (k_target - 256 * band));
                assert!(
                    got_unrev.abs_diff(mirrored) <= 2,
                    "unreversed tone k={k_target} peaked at {got_unrev}, expected ≈{mirrored}"
                );
            }
        }
    }

    /// `split_bands` long layout: contiguous ascending quarters, bands
    /// 1 and 3 reversed.
    #[test]
    fn split_bands_long_layout() {
        let spec: Vec<f64> = (0..1024).map(|i| i as f64).collect();
        let bands = split_bands(&spec, WindowSequence::OnlyLong).unwrap();
        for (b, band) in bands.iter().enumerate() {
            assert_eq!(band.len(), 256);
            if b % 2 == 0 {
                assert_eq!(band[0], (256 * b) as f64);
                assert_eq!(band[255], (256 * b + 255) as f64);
            } else {
                assert_eq!(band[0], (256 * b + 255) as f64);
                assert_eq!(band[255], (256 * b) as f64);
            }
        }
    }

    /// `split_bands` short layout: per short window, per-band 32-line
    /// quarters (window-major columns), bands 1 and 3 reversed within
    /// each window.
    #[test]
    fn split_bands_short_layout() {
        let spec: Vec<f64> = (0..1024).map(|i| i as f64).collect();
        let bands = split_bands(&spec, WindowSequence::EightShort).unwrap();
        for (b, band) in bands.iter().enumerate() {
            assert_eq!(band.len(), 256);
            for w in 0..8 {
                let base = (128 * w + 32 * b) as f64;
                if b % 2 == 0 {
                    assert_eq!(band[32 * w], base);
                    assert_eq!(band[32 * w + 31], base + 31.0);
                } else {
                    assert_eq!(band[32 * w], base + 31.0);
                    assert_eq!(band[32 * w + 31], base);
                }
            }
        }
    }

    /// Bad spectrum length is rejected.
    #[test]
    fn split_bands_rejects_bad_length() {
        assert!(split_bands(&[0.0; 512], WindowSequence::OnlyLong).is_err());
    }

    /// A minimal [`IcsInfo`] for the front-half tests.
    fn test_ics_info(shape: WindowShape, seq: WindowSequence) -> IcsInfo {
        let short = seq == WindowSequence::EightShort;
        IcsInfo {
            family: crate::swb_offset::FrameFamily::Lc1024,
            ics_reserved_bit: false,
            window_sequence: seq,
            window_shape: shape,
            max_sfb: 0,
            scale_factor_grouping: if short { Some(0) } else { None },
            predictor_data_present: false,
            predictor_data: None,
            ltp_data_present: false,
            ltp_data: None,
            ltp_data_present_pair: None,
            ltp_data_pair: None,
            num_windows: if short { 8 } else { 1 },
            num_window_groups: if short { 8 } else { 1 },
            window_group_length: if short { vec![1; 8] } else { vec![1] },
            num_swb: 0,
        }
    }

    /// `windowed_bands` output geometry: four 512-sample columns for
    /// every window sequence, and the long-start column goes silent
    /// after the §4.6.11.3.2 zero region (scaled: `[400, 512)`).
    #[test]
    fn windowed_bands_geometry() {
        let spec = vec![1.0f64; 1024];
        for seq in [
            WindowSequence::OnlyLong,
            WindowSequence::LongStart,
            WindowSequence::EightShort,
            WindowSequence::LongStop,
        ] {
            let mut synth = SsrSynthesis::new();
            let info = test_ics_info(WindowShape::Sine, seq);
            let u = synth.windowed_bands(&spec, &info).unwrap();
            for band in &u {
                assert_eq!(band.len(), BAND_SAMPLES_PER_FRAME);
                assert!(band.iter().all(|v| v.is_finite()));
            }
            if seq == WindowSequence::LongStart {
                for band in &u {
                    for &v in &band[400..] {
                        assert_eq!(v, 0.0, "LONG_START zero region");
                    }
                }
            }
        }
    }
}

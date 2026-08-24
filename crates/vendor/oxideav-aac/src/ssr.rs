//! SSR per-channel gain-control + IPQF back-end driver (ISO/IEC
//! 14496-3 §4.6.12).
//!
//! [`SsrGainControl`] composes the four-band gain-control state
//! ([`crate::gain_control::GainBandState`]) and the IPQF synthesizer
//! ([`crate::ipqf::Ipqf`]) into one persistent per-channel pipeline.
//! Per frame it consumes the four per-band IMDCT outputs `U_{W,B}` plus
//! the `gain_control_data()` side info and returns the reconstructed
//! PCM time signal `AS(n)`:
//!
//! ```text
//! for each PQF band B in 0..4:
//!     V_B = GainBandState[B].window_overlap(ladder[B], U_B, seq)   §4.6.12.3.3
//! AS = IPQF.synthesize([V_0, V_1, V_2, V_3])                       §4.6.12.3.4
//! ```
//!
//! ## Front half
//!
//! [`SsrGainControl`] runs the §4.6.12.3.3–4 *back half* of the SSR
//! tool: the per-band gain windowing/overlap and the IPQF synthesis,
//! from caller-supplied non-overlapped `U_{W,B}` columns. The
//! §4.6.12.1 *front half* — splitting the transmitted spectrum into
//! the four PQF-band coefficient columns, the even-band spectral
//! reversal, and the per-band 256-line (long) / 32-line (short)
//! IMDCTs + windows — lives in [`crate::ssr_filterbank`];
//! [`SsrChannelDecoder`] chains the two into the complete
//! spectrum → PCM pipeline.
//!
//! ## Provenance
//!
//! Composes the §4.6.12.1 front half ([`crate::ssr_filterbank`]) and
//! the §4.6.12.3.1–4 stages implemented in [`crate::gain_control`] and
//! [`crate::ipqf`]; no new tables. No external SSR implementation was
//! consulted — the full-pipeline tests below validate against the
//! Annex C.2.1.1 analysis PQF and the §4.6.11 TDAC property.

use crate::gain_control::{band_record, GainBandState};
use crate::gain_control_data::GainControlData;
use crate::ics_info::{IcsInfo, WindowSequence};
use crate::ipqf::{Ipqf, NUM_BANDS};
use crate::ssr_filterbank::SsrSynthesis;
use crate::Result;

/// One channel's persistent SSR gain-control + IPQF state: the four
/// per-band [`GainBandState`] carries plus the streaming [`Ipqf`].
#[derive(Debug, Clone)]
pub struct SsrGainControl {
    /// Per-PQF-band gain-control cross-frame state (`PFMD` / `PT`).
    bands: [GainBandState; NUM_BANDS],
    /// The streaming IPQF synthesizer (cross-frame band history).
    ipqf: Ipqf,
}

impl Default for SsrGainControl {
    fn default() -> Self {
        Self::new()
    }
}

impl SsrGainControl {
    /// A fresh per-channel SSR pipeline with the §4.6.12 spec initial
    /// state (`PFMD ≡ 1.0`, `PT ≡ 0.0`, zero IPQF history).
    #[must_use]
    pub fn new() -> Self {
        SsrGainControl {
            bands: core::array::from_fn(|_| GainBandState::new()),
            ipqf: Ipqf::new(),
        }
    }

    /// Reconstruct one frame of PCM `AS(n)` from the four per-band IMDCT
    /// outputs and the frame's gain-control side info.
    ///
    /// * `u` — the four non-overlapped per-band IMDCT outputs
    ///   `U_{W,B}`. `u[B]` is the band-`B` column: a single 512-sample
    ///   window for the long sequences, or eight 64-sample windows
    ///   concatenated for `EIGHT_SHORT_SEQUENCE`.
    /// * `gcd` — the decoded `gain_control_data()` (`None` ⇒ no gain
    ///   control active this frame, every band runs `T = U`).
    /// * `seq` — the frame's `window_sequence`.
    ///
    /// Returns `NUM_BANDS · |V_B|` PCM samples (`4 · 256 = 1024` for the
    /// steady `ONLY_LONG` / `EIGHT_SHORT` case).
    #[must_use]
    pub fn decode_frame(
        &mut self,
        u: &[Vec<f64>; NUM_BANDS],
        gcd: Option<&GainControlData>,
        seq: WindowSequence,
    ) -> Vec<f64> {
        // §4.6.12.3.3 — per-band gain windowing + overlap → V_B.
        let mut v: [Vec<f64>; NUM_BANDS] = core::array::from_fn(|_| Vec::new());
        for (b, slot) in v.iter_mut().enumerate() {
            // Spec band index is 1..=3 for gain-controlled bands; PQF
            // band 0 never carries a ladder (§4.6.12.3.3 `B == 0`).
            let ladder = gcd.and_then(|g| band_record(g, b));
            *slot = self.bands[b].window_overlap(ladder, &u[b], seq);
        }

        // §4.6.12.3.4 — IPQF synthesis. All four V_B share the same
        // per-frame length by construction.
        let len = v[0].len();
        debug_assert!(v.iter().all(|vb| vb.len() == len));
        let band_refs: [&[f64]; NUM_BANDS] = core::array::from_fn(|b| v[b].as_slice());
        self.ipqf.synthesize(&band_refs, len)
    }
}

/// One channel's *complete* §4.6.12 SSR reconstruction pipeline: the
/// §4.6.12.1 front-half filterbank ([`SsrSynthesis`] — band split,
/// even-band reversal, per-band 256/32-line IMDCTs + windows) chained
/// into the §4.6.12.3 gain-control + IPQF back end
/// ([`SsrGainControl`]).
///
/// This is the SSR (AOT 3) replacement for the per-channel §4.6.11
/// [`crate::filterbank::Filterbank`]: it consumes the same decoded
/// 1024-line spectrum (window-major for `EIGHT_SHORT_SEQUENCE`, after
/// TNS) and produces the frame's PCM time signal `AS(n)`.
#[derive(Debug, Clone, Default)]
pub struct SsrChannelDecoder {
    /// §4.6.12.1 front half (carries the previous block's
    /// `window_shape`).
    synth: SsrSynthesis,
    /// §4.6.12.3 back half (carries `PFMD` / `PT` / IPQF history).
    gain: SsrGainControl,
}

impl SsrChannelDecoder {
    /// A fresh SSR channel pipeline with the spec initial state.
    #[must_use]
    pub fn new() -> Self {
        SsrChannelDecoder::default()
    }

    /// Decode one frame: 1024-line spectrum (+ this frame's
    /// `gain_control_data()`, if any) → PCM `AS(n)`.
    ///
    /// The output length follows the §4.6.12.3.3 band fragment length
    /// times the four-band IPQF interpolation: 1024 samples for
    /// `ONLY_LONG` / `EIGHT_SHORT`, 1472 for `LONG_START`, 576 for
    /// `LONG_STOP` (a `START`/`STOP` pair still totals 2048, so stream
    /// timing is preserved).
    pub fn decode_frame(
        &mut self,
        spec: &[f64],
        ics_info: &IcsInfo,
        gcd: Option<&GainControlData>,
    ) -> Result<Vec<f64>> {
        let u = self.synth.windowed_bands(spec, ics_info)?;
        Ok(self.gain.decode_frame(&u, gcd, ics_info.window_sequence))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Four bands of constant-zero U give silence out.
    #[test]
    fn zero_bands_give_silence() {
        let mut ssr = SsrGainControl::new();
        let u: [Vec<f64>; NUM_BANDS] = core::array::from_fn(|_| vec![0.0f64; 512]);
        let pcm = ssr.decode_frame(&u, None, WindowSequence::OnlyLong);
        assert_eq!(pcm.len(), 1024);
        assert!(pcm.iter().all(|&x| x == 0.0));
    }

    /// A steady ONLY_LONG stream produces 1024 PCM samples per frame and
    /// the pipeline is finite + deterministic.
    #[test]
    fn only_long_frame_is_1024_pcm() {
        let mut ssr = SsrGainControl::new();
        let u: [Vec<f64>; NUM_BANDS] =
            core::array::from_fn(|b| (0..512).map(|j| ((b * 512 + j) as f64) * 1e-3).collect());
        let pcm0 = ssr.decode_frame(&u, None, WindowSequence::OnlyLong);
        assert_eq!(pcm0.len(), 1024);
        assert!(pcm0.iter().all(|x| x.is_finite()));
        // A second identical frame also yields 1024 and threads state.
        let pcm1 = ssr.decode_frame(&u, None, WindowSequence::OnlyLong);
        assert_eq!(pcm1.len(), 1024);
        // The first and second frames differ (the overlap tail carries).
        assert!(pcm0 != pcm1);
    }

    /// Gain control with `max_band == 0` (the bare 2-bit field, no
    /// ladders) is the identity: same PCM as `None`.
    #[test]
    fn max_band_zero_matches_no_gain() {
        let u: [Vec<f64>; NUM_BANDS] = core::array::from_fn(|b| {
            (0..512)
                .map(|j| ((b + 1) as f64 * (j as f64 + 1.0)).sin())
                .collect()
        });
        let gcd = GainControlData {
            max_band: 0,
            bands: Vec::new(),
        };
        let mut a = SsrGainControl::new();
        let mut b = SsrGainControl::new();
        let pa = a.decode_frame(&u, Some(&gcd), WindowSequence::OnlyLong);
        let pb = b.decode_frame(&u, None, WindowSequence::OnlyLong);
        assert_eq!(pa.len(), pb.len());
        for (x, y) in pa.iter().zip(pb.iter()) {
            assert!((x - y).abs() < 1e-12);
        }
    }

    /// EIGHT_SHORT bands (eight 64-sample windows each) also reconstruct
    /// 1024 PCM samples per frame.
    #[test]
    fn eight_short_frame_is_1024_pcm() {
        let mut ssr = SsrGainControl::new();
        let u: [Vec<f64>; NUM_BANDS] =
            core::array::from_fn(|_| (0..512).map(|j| (j as f64 * 0.01).cos()).collect());
        let pcm = ssr.decode_frame(&u, None, WindowSequence::EightShort);
        assert_eq!(pcm.len(), 1024);
        assert!(pcm.iter().all(|x| x.is_finite()));
    }
}

/// Full-pipeline round-trip tests: the Annex C.2.1.1 analysis PQF +
/// the §4.6.11.3.2 (quarter-scale) analysis windows + forward MDCTs
/// mirror the encoder; [`SsrChannelDecoder`] must reconstruct the
/// input within the PQF pair's near-perfect-reconstruction bound.
#[cfg(test)]
mod round_trip_tests {
    use super::*;
    use crate::filterbank::{forward_mdct, long_sequence_window_n, short_window_n};
    use crate::gain_control::{band_record, pfmd_len, BandGainFunction};
    use crate::gain_control_data::{GainAdjust, GainBand, GainWindow};
    use crate::ics_info::{IcsInfo, WindowShape};
    use crate::ssr_filterbank::pqf_test_support::{pqf_analysis, PQF_CASCADE_DELAY};
    use crate::ssr_filterbank::{SSR_LONG_TRANSFORM, SSR_SHORT_TRANSFORM};
    use core::f64::consts::PI;

    /// A minimal [`IcsInfo`] carrying just what the SSR pipeline reads.
    fn ics(shape: WindowShape, seq: WindowSequence) -> IcsInfo {
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

    /// A broadband deterministic test signal exciting all four PQF
    /// bands: four tones (one per band quarter) plus a slow envelope.
    fn test_signal(len: usize) -> Vec<f64> {
        (0..len)
            .map(|n| {
                let t = n as f64;
                let env = 0.6 + 0.4 * (2.0 * PI * t / 3000.0).sin();
                env * ((0.05 * t).sin()
                    + 0.7 * (0.9 * t).sin()
                    + 0.5 * (1.8 * t).sin()
                    + 0.4 * (2.9 * t).sin())
            })
            .collect()
    }

    /// Encoder-mirror state: per-band position of the next frame's
    /// window origin (in band samples) plus the previous block's
    /// window shape and the per-band `PFMD` gain threading.
    struct MirrorEncoder {
        /// Absolute band-sample position `P_f` where this frame's `V`
        /// starts.
        p: usize,
        prev_shape: Option<WindowShape>,
        /// Per-band `PFMD` carry for the encoder-side GMF (256
        /// entries, prefix-read like the decoder's).
        pfmd: [Vec<f64>; NUM_BANDS],
    }

    impl MirrorEncoder {
        fn new() -> Self {
            MirrorEncoder {
                p: 0,
                prev_shape: None,
                pfmd: core::array::from_fn(|_| vec![1.0f64; 256]),
            }
        }

        /// Encode one frame: window the four band signals at the
        /// §4.6.12.3.3-mirror positions, apply the §4.6.12.3.2 `GMF`
        /// (identity when `gcd` is `None`), forward-MDCT each band,
        /// reverse the even (0-based 1 and 3) bands and assemble the
        /// 1024-line spectrum. Advances the band position by the
        /// frame's `V` length.
        fn encode_frame(
            &mut self,
            bands: &[Vec<f64>; NUM_BANDS],
            seq: WindowSequence,
            shape: WindowShape,
            gcd: Option<&GainControlData>,
        ) -> Vec<f64> {
            let left = self.prev_shape.unwrap_or(shape);
            let mut spec = vec![0.0f64; 1024];

            // Per-band GMF (1/AD) for this frame, threading PFMD the
            // same way the decoder does.
            let gmf: [Vec<Vec<f64>>; NUM_BANDS] = core::array::from_fn(|b| {
                let record = gcd.and_then(|g| band_record(g, b));
                let f = match record {
                    Some(rec) => {
                        BandGainFunction::reconstruct(rec, seq, &self.pfmd[b][..pfmd_len(seq)])
                    }
                    None => BandGainFunction::identity(seq),
                };
                self.pfmd[b][..f.pfmd_next.len()].copy_from_slice(&f.pfmd_next);
                f.ad.iter()
                    .map(|w| w.iter().map(|&a| 1.0 / a).collect())
                    .collect()
            });

            match seq {
                WindowSequence::EightShort => {
                    // Window w over band samples [p + 32w, p + 32w + 64).
                    for w in 0..8 {
                        let win = short_window_n(SSR_SHORT_TRANSFORM, w, left, shape);
                        for (b, band) in bands.iter().enumerate() {
                            let z: Vec<f64> = (0..SSR_SHORT_TRANSFORM)
                                .map(|n| band[self.p + 32 * w + n] * gmf[b][w][n] * win[n])
                                .collect();
                            let mut coeffs = forward_mdct(&z, SSR_SHORT_TRANSFORM);
                            if b % 2 == 1 {
                                coeffs.reverse();
                            }
                            spec[128 * w + 32 * b..128 * w + 32 * b + 32].copy_from_slice(&coeffs);
                        }
                    }
                    self.p += 256;
                }
                _ => {
                    // Long window over [p, p+512) (LONG_STOP: the
                    // window origin sits 112 band samples *before* the
                    // frame's V start, mirroring §4.6.12.3.3).
                    let origin = match seq {
                        WindowSequence::LongStop => self.p - 112,
                        _ => self.p,
                    };
                    let win = long_sequence_window_n(
                        SSR_LONG_TRANSFORM,
                        SSR_SHORT_TRANSFORM,
                        seq,
                        left,
                        shape,
                    )
                    .unwrap();
                    for (b, band) in bands.iter().enumerate() {
                        let z: Vec<f64> = (0..SSR_LONG_TRANSFORM)
                            .map(|n| band[origin + n] * gmf[b][0][n] * win[n])
                            .collect();
                        let mut coeffs = forward_mdct(&z, SSR_LONG_TRANSFORM);
                        if b % 2 == 1 {
                            coeffs.reverse();
                        }
                        spec[256 * b..256 * b + 256].copy_from_slice(&coeffs);
                    }
                    self.p += match seq {
                        WindowSequence::OnlyLong => 256,
                        WindowSequence::LongStart => 368,
                        WindowSequence::LongStop => 144,
                        WindowSequence::EightShort => unreachable!(),
                    };
                }
            }
            self.prev_shape = Some(shape);
            spec
        }
    }

    /// Round-trip error-to-signal RMS of `y` (decoder output) against
    /// `x` delayed by the PQF cascade, over `[skip, n)`.
    fn err_ratio(x: &[f64], y: &[f64], skip: usize) -> f64 {
        let n = y.len().min(x.len().saturating_sub(PQF_CASCADE_DELAY));
        let (mut err, mut sig) = (0.0f64, 0.0f64);
        for i in skip..n {
            // y(i) reconstructs x(i - delay): compare shifted.
            let d = y[i] - x[i - PQF_CASCADE_DELAY];
            err += d * d;
            sig += x[i - PQF_CASCADE_DELAY] * x[i - PQF_CASCADE_DELAY];
        }
        (err / sig).sqrt()
    }

    /// Steady `ONLY_LONG` frames round-trip through the complete
    /// §4.6.12 pipeline within the PQF pair's reconstruction bound,
    /// for both window shapes.
    #[test]
    fn full_pipeline_round_trips_only_long() {
        let frames = 20usize;
        let x = test_signal(4 * 256 * (frames + 3));
        let bands = pqf_analysis(&x);
        for shape in [WindowShape::Sine, WindowShape::Kbd] {
            let mut enc = MirrorEncoder::new();
            let mut dec = SsrChannelDecoder::new();
            let info = ics(shape, WindowSequence::OnlyLong);
            let mut y = Vec::new();
            for _ in 0..frames {
                let spec = enc.encode_frame(&bands, WindowSequence::OnlyLong, shape, None);
                y.extend(dec.decode_frame(&spec, &info, None).unwrap());
            }
            assert_eq!(y.len(), 1024 * frames);
            let ratio = err_ratio(&x, &y, 2048);
            assert!(ratio < 1e-3, "{shape:?} round-trip err/sig = {ratio}");
        }
    }

    /// A full window-sequence transition chain (`ONLY_LONG →
    /// LONG_START → EIGHT_SHORT ×2 → LONG_STOP → ONLY_LONG`)
    /// round-trips, with the §4.6.12.3.3 variable per-frame output
    /// lengths (1024 / 1472 / 1024 / 576) preserving stream timing.
    #[test]
    fn full_pipeline_round_trips_window_transitions() {
        use WindowSequence::{EightShort, LongStart, LongStop, OnlyLong};
        let chain = [
            OnlyLong, OnlyLong, OnlyLong, LongStart, EightShort, EightShort, LongStop, OnlyLong,
            OnlyLong, LongStart, EightShort, LongStop, OnlyLong, OnlyLong,
        ];
        let x = test_signal(4 * 256 * (chain.len() + 3));
        let bands = pqf_analysis(&x);
        let mut enc = MirrorEncoder::new();
        let mut dec = SsrChannelDecoder::new();
        let mut y = Vec::new();
        let mut expect_len = 0usize;
        for &seq in &chain {
            let spec = enc.encode_frame(&bands, seq, WindowShape::Sine, None);
            let out = dec
                .decode_frame(&spec, &ics(WindowShape::Sine, seq), None)
                .unwrap();
            expect_len += match seq {
                OnlyLong | EightShort => 1024,
                LongStart => 1472,
                LongStop => 576,
            };
            y.extend(out);
        }
        assert_eq!(y.len(), expect_len);
        let ratio = err_ratio(&x, &y, 2048);
        assert!(
            ratio < 1e-3,
            "transition-chain round-trip err/sig = {ratio}"
        );
    }

    /// Gain ladders cancel end to end: the encoder applies the
    /// §4.6.12.3.2 `GMF`, the decoder its inverse `AD`, and the
    /// round-trip stays close to the input — while decoding the same
    /// stream *without* the gain data leaves the gain modification in
    /// the output (large error). Pins the orientation of the whole
    /// §4.6.12.3 gain path against the front half.
    #[test]
    fn gain_ladders_cancel_in_round_trip() {
        let frames = 16usize;
        let x = test_signal(4 * 256 * (frames + 3));
        let bands = pqf_analysis(&x);

        // Ladders on bands 1..=3 (spec 2nd..4th), one gain change per
        // window: modest ±1-exponent steps at varied positions.
        let gcd = GainControlData {
            max_band: 3,
            bands: vec![
                GainBand {
                    windows: vec![GainWindow {
                        adjustments: vec![GainAdjust {
                            alevcode: 5, // AdjLev = 1 ⇒ ALEV = 2.
                            aloccode: 4, // ALOC = 32.
                        }],
                    }],
                },
                GainBand {
                    windows: vec![GainWindow {
                        adjustments: vec![GainAdjust {
                            alevcode: 3,  // AdjLev = −1 ⇒ ALEV = 1/2.
                            aloccode: 12, // ALOC = 96.
                        }],
                    }],
                },
                GainBand {
                    windows: vec![GainWindow {
                        adjustments: vec![GainAdjust {
                            alevcode: 6,  // AdjLev = 2 ⇒ ALEV = 4.
                            aloccode: 20, // ALOC = 160.
                        }],
                    }],
                },
            ],
        };

        let mut enc = MirrorEncoder::new();
        let mut dec = SsrChannelDecoder::new();
        let mut dec_plain = SsrChannelDecoder::new();
        let info = ics(WindowShape::Sine, WindowSequence::OnlyLong);
        let mut y = Vec::new();
        let mut y_plain = Vec::new();
        for _ in 0..frames {
            let spec = enc.encode_frame(
                &bands,
                WindowSequence::OnlyLong,
                WindowShape::Sine,
                Some(&gcd),
            );
            y.extend(dec.decode_frame(&spec, &info, Some(&gcd)).unwrap());
            y_plain.extend(dec_plain.decode_frame(&spec, &info, None).unwrap());
        }
        let ratio = err_ratio(&x, &y, 2048);
        // Gain steps re-introduce a little aliasing at the transition
        // ramps (the §4.6.12.3.2 Inter() ramp bounds it); the
        // compensated round trip must stay small…
        assert!(ratio < 0.02, "gain-compensated err/sig = {ratio}");
        // …while dropping the gain data leaves the modification in.
        let ratio_plain = err_ratio(&x, &y_plain, 2048);
        assert!(
            ratio_plain > 5.0 * ratio,
            "uncompensated err/sig = {ratio_plain} vs compensated {ratio}"
        );
    }

    /// Per-sequence output lengths of [`SsrChannelDecoder`].
    #[test]
    fn decode_frame_output_lengths() {
        let spec = vec![0.5f64; 1024];
        let mut dec = SsrChannelDecoder::new();
        for (seq, len) in [
            (WindowSequence::OnlyLong, 1024),
            (WindowSequence::LongStart, 1472),
            (WindowSequence::EightShort, 1024),
            (WindowSequence::LongStop, 576),
        ] {
            let out = dec
                .decode_frame(&spec, &ics(WindowShape::Sine, seq), None)
                .unwrap();
            assert_eq!(out.len(), len, "{seq:?}");
        }
    }
}

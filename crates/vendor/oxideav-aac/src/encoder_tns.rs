//! Encoder-side §4.6.9 Temporal Noise Shaping — decision, PARCOR
//! quantisation, and the analysis filtering pass.
//!
//! TNS is defined normatively from the decoder side only: §4.6.9.3
//! specifies how transmitted filter coefficients are inverse-quantised
//! (`tns_decode_coef`), stepped up to LPC, and slid across the
//! spectrum as an **all-pole synthesis filter**. The encoder's job is
//! the inverse: pick a prediction filter over the spectral
//! coefficients, quantise it into the Table 4.54 wire fields, and run
//! the **all-zero analysis filter** (§4.6.7.4.1's `tns_ma_filter`,
//! the exact inverse of the synthesis filter over a shared region) on
//! the spectrum before quantisation, so the decoder's synthesis pass
//! reconstructs the original while shaping the quantisation noise in
//! time.
//!
//! Everything analysis-side (the autocorrelation, the Levinson-Durbin
//! recursion, the activation threshold) is an encoder degree of
//! freedom — any filter whose wire record is Table 4.54-conforming is
//! a conforming encode. The *applied* filter, however, must be
//! bit-identical to the one the decoder will derive from the wire, so
//! this module quantises the reflection coefficients first
//! ([`crate::tns_coef::tns_encode_coef`]) and then filters through
//! [`crate::tns_frame::tns_analysis_frame`], which re-derives the LPC
//! from the **wire** values exactly as `tns_decode_frame` does. The
//! encoder/decoder filter pair is therefore the §4.6.9.3
//! analysis∘synthesis identity by construction.
//!
//! ## Decision rule
//!
//! Per transform window the encoder computes the autocorrelation of
//! the coverable spectral region (the same
//! `min(num_swb, TNS_MAX_BANDS, max_sfb)`-clamped region the §4.6.9.3
//! walk will filter), runs Levinson-Durbin up to
//! [`TNS_ENC_MAX_ORDER`], and activates TNS only when the resulting
//! prediction gain `r(0) / err(order)` clears [`TNS_GAIN_MIN`]. A
//! high prediction gain over *frequency* coefficients means the
//! signal's *temporal* envelope inside the window is strongly
//! non-flat (the time/frequency duality TNS exploits, §4.6.9.1) —
//! exactly the windows where unshaped quantisation noise smears
//! audibly. Trailing reflection coefficients below
//! [`TNS_COEF_TRIM`] are trimmed to keep the order (and the 4-bit
//! coefficient payload) minimal.

use crate::ics_info::WindowSequence;
use crate::swb_offset::{
    long_window_offsets, short_window_offsets, LONG_WINDOW_LEN, SHORT_WINDOW_LEN,
};
use crate::tns_coef::tns_encode_coef;
use crate::tns_data::{num_windows, TnsData, TnsFilter, TnsWindow};
use crate::tns_frame::tns_analysis_frame;
use crate::tns_max::{clamp_tns_band, tns_max_order, AOT_AAC_LC};
use crate::Result;

/// Encoder-side cap on the TNS filter order. Table 4.102 allows up
/// to 12 for AAC LC long windows (7 short), but each tap costs 4
/// wire bits and the marginal gain past order 8 is small for a
/// first-order envelope model; the Levinson recursion below stops
/// early anyway once the prediction error stops shrinking.
pub const TNS_ENC_MAX_ORDER: usize = 8;

/// Minimum §4.6.9.1 prediction gain (`r(0) / err`) for TNS to
/// activate on a window. Below ~1.4 the temporal envelope is close
/// enough to flat that the side-info bits outweigh the shaping win.
pub const TNS_GAIN_MIN: f64 = 1.4;

/// Reflection-coefficient trim threshold: trailing PARCOR values
/// with `|k|` below this contribute negligible shaping and are
/// dropped to shorten the transmitted order.
pub const TNS_COEF_TRIM: f64 = 0.1;

/// `coef_res` the encoder always transmits: `true` selects the 4-bit
/// (`coef_res_bits == 4`) resolution of §4.6.9.3, the finer of the
/// two grids.
const TNS_COEF_RES: bool = true;

/// One window's TNS decision: the reflection coefficients that
/// survived the gain threshold and trim, ready for quantisation.
struct WindowDecision {
    /// PARCOR reflection coefficients, order `parcor.len()`.
    parcor: Vec<f64>,
}

/// Autocorrelation `r[0..=max_lag]` of `region`.
fn autocorrelation(region: &[f64], max_lag: usize) -> Vec<f64> {
    let n = region.len();
    (0..=max_lag.min(n.saturating_sub(1)))
        .map(|lag| (0..n - lag).map(|i| region[i] * region[i + lag]).sum())
        .collect()
}

/// Levinson-Durbin recursion on the autocorrelation `r`, returning
/// the reflection (PARCOR) coefficients and the final prediction
/// error. The per-step update matches the §4.6.9.3 step-up
/// ([`crate::tns_coef::lpc_step_up`]) convention — `a_m[i] =
/// a_{m-1}[i] + k_m · a_{m-1}[m-i]`, `a_m[m] = k_m` — so the
/// returned `k` values, once quantised and stepped up by the
/// decoder, reproduce this exact predictor. The analysis filter is
/// then `y(n) = x(n) + Σ a[i]·x(n-i)` (the §4.6.7.4.1
/// `tns_ma_filter` polarity), i.e. `a[]` is the prediction-*error*
/// filter tail.
fn levinson(r: &[f64], max_order: usize) -> (Vec<f64>, f64) {
    let mut err = r[0];
    if err <= 0.0 {
        return (Vec::new(), err);
    }
    let order = max_order.min(r.len().saturating_sub(1));
    let mut a = vec![0.0f64; order + 1];
    a[0] = 1.0;
    let mut k_out = Vec::with_capacity(order);
    let mut b = vec![0.0f64; order + 1];
    for m in 1..=order {
        // acc = r[m] + Σ_{i=1}^{m-1} a[i]·r[m-i]
        let mut acc = r[m];
        for i in 1..m {
            acc += a[i] * r[m - i];
        }
        let k = -acc / err;
        if !k.is_finite() || k.abs() >= 1.0 {
            // Numerically degenerate (r not positive definite at
            // this order) — stop with the taps found so far.
            break;
        }
        // Step-up update, mirroring lpc_step_up so the decoder's
        // reconstruction of `a` from the k's is this exact array.
        for i in 1..m {
            b[i] = a[i] + k * a[m - i];
        }
        a[1..m].copy_from_slice(&b[1..m]);
        a[m] = k;
        k_out.push(k);
        err *= 1.0 - k * k;
        if err <= 0.0 {
            break;
        }
    }
    (k_out, err)
}

/// Decide TNS for one window's coverable region. Returns `None`
/// when the prediction gain does not clear [`TNS_GAIN_MIN`] or the
/// trim leaves no taps.
fn decide_window(region: &[f64], max_order: usize) -> Option<WindowDecision> {
    if region.len() < 2 * max_order.max(1) {
        return None;
    }
    let r = autocorrelation(region, max_order);
    if r[0] <= 0.0 {
        return None;
    }
    let (mut parcor, err) = levinson(&r, max_order);
    if parcor.is_empty() || err <= 0.0 {
        return None;
    }
    let gain = r[0] / err;
    if gain < TNS_GAIN_MIN {
        return None;
    }
    while parcor.last().is_some_and(|k| k.abs() < TNS_COEF_TRIM) {
        parcor.pop();
    }
    if parcor.is_empty() {
        return None;
    }
    Some(WindowDecision { parcor })
}

/// Detect and apply §4.6.9 TNS to one channel's analysis spectrum in
/// place.
///
/// `spec` is the window-major forward-MDCT spectrum
/// (`num_windows × window_len`, the encoder's analysis output before
/// quantisation), `seq` / `max_sfb` / `fs_index` the surrounding
/// `ics_info()` parameters (the encoder transmits `max_sfb ==
/// num_swb`). `permit` is the caller's per-window **temporal** gate
/// (length [`num_windows`], see below); for every permitted window
/// whose coverable region clears the [`TNS_GAIN_MIN`]
/// prediction-gain threshold, one upward filter covering the full
/// §4.6.9.3-clamped band range is quantised into Table 4.54 wire
/// fields; the whole-frame [`TnsData`] is then run through
/// [`tns_analysis_frame`] — deriving the LPC from the **wire**
/// coefficient values exactly as the decoder's `tns_decode_frame`
/// will — so the applied analysis filter and the decoder's synthesis
/// filter are exact inverses.
///
/// ## Why a temporal gate
///
/// Spectral prediction gain alone over-fires: a *steady tonal*
/// window also shows LPC gain over its MDCT coefficients (the smooth
/// leakage skirts around each spectral line are highly predictable)
/// even though its temporal envelope is flat — exactly the windows
/// where TNS buys nothing and merely re-shapes (and, at spectral
/// peaks, locally amplifies) the quantisation noise of a
/// peak-anchored rate allocation. The §4.6.9.1 duality says TNS pays
/// off when the *time-domain* envelope inside the window is strongly
/// non-flat, which the encoder can measure directly on its input
/// samples — so the caller derives `permit[w]` from the raw
/// subblock-energy flatness of window `w`'s time region (see
/// `StreamEncoder`'s hop driver) and this module only spends
/// prediction-gain analysis on permitted windows.
///
/// Returns `Ok(None)` (spectrum untouched) when no window activates.
pub fn detect_and_apply_tns(
    spec: &mut [f64],
    seq: WindowSequence,
    max_sfb: u8,
    fs_index: u8,
    permit: &[bool],
) -> Result<Option<TnsData>> {
    let nw = num_windows(seq);
    let (window_len, offsets) = if seq.is_eight_short() {
        (SHORT_WINDOW_LEN as usize, short_window_offsets(fs_index)?)
    } else {
        (LONG_WINDOW_LEN as usize, long_window_offsets(fs_index)?)
    };
    let num_swb = offsets.len() - 1;
    // The §4.6.9.3 region for a full-length (length == num_swb,
    // bottom == 0) upward filter: [swb_offset[0],
    // swb_offset[min(num_swb, TNS_MAX_BANDS, max_sfb)]).
    let top = clamp_tns_band(num_swb as u8, max_sfb, AOT_AAC_LC, seq, fs_index)? as usize;
    let end = offsets[top] as usize;
    let max_order = TNS_ENC_MAX_ORDER.min(tns_max_order(AOT_AAC_LC, seq, fs_index)? as usize);

    let mut windows = Vec::with_capacity(nw);
    let mut any = false;
    for w in 0..nw {
        let region = &spec[w * window_len..w * window_len + end];
        let decision = if permit.get(w).copied().unwrap_or(false) {
            decide_window(region, max_order)
        } else {
            None
        };
        let filters = match decision {
            Some(d) => {
                // Quantise PARCOR → wire coef[] (4-bit grid). The
                // analysis pass below re-derives the LPC from these
                // wire values, so the filter actually applied is the
                // quantised one the decoder will invert.
                let coef = tns_encode_coef(4, 0, &d.parcor)?
                    .into_iter()
                    .map(|c| c as u8)
                    .collect::<Vec<u8>>();
                any = true;
                vec![TnsFilter {
                    length: num_swb as u8,
                    order: coef.len() as u8,
                    direction: false,
                    coef_compress: false,
                    coef,
                }]
            }
            None => Vec::new(),
        };
        windows.push(TnsWindow {
            coef_res: TNS_COEF_RES,
            filters,
        });
    }
    if !any {
        return Ok(None);
    }
    let tns = TnsData { windows };
    tns_analysis_frame(spec, &tns, seq, max_sfb, AOT_AAC_LC, fs_index)?;
    Ok(Some(tns))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ics_info::WindowSequence;
    use crate::tns_frame::tns_decode_frame;

    /// Deterministic pseudo-noise in [-1, 1).
    fn noise(n: usize, seed: u32) -> Vec<f64> {
        let mut state = seed;
        (0..n)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (state as i32) as f64 / 2_147_483_648.0
            })
            .collect()
    }

    /// Run `x` through the all-pole filter `1 / (1 + Σ a[i] z^-i)`
    /// (the synthesis polarity), producing a strongly correlated
    /// sequence whose optimal prediction-error filter is `a`.
    fn all_pole(x: &[f64], a: &[f64]) -> Vec<f64> {
        let mut y = vec![0.0f64; x.len()];
        for n in 0..x.len() {
            let mut v = x[n];
            for (i, &ai) in a.iter().enumerate() {
                let d = i + 1;
                if n >= d {
                    v -= ai * y[n - d];
                }
            }
            y[n] = v;
        }
        y
    }

    #[test]
    fn levinson_recovers_ar1_reflection() {
        // AR(1) with pole 0.8: prediction-error filter a = [-0.8],
        // reflection k1 = -0.8.
        let x = noise(4096, 0xC0FF_EE00);
        let y = all_pole(&x, &[-0.8]);
        let r = autocorrelation(&y, 4);
        let (k, err) = levinson(&r, 4);
        assert!(!k.is_empty());
        assert!(
            (k[0] + 0.8).abs() < 0.05,
            "k1 = {} should approximate -0.8",
            k[0]
        );
        // Prediction gain ≈ 1/(1 - 0.64) ≈ 2.8.
        let gain = r[0] / err;
        assert!(gain > 2.0, "gain {gain} too low for AR(1) 0.8");
    }

    #[test]
    fn white_region_stays_untns() {
        // A flat (white) region has prediction gain ≈ 1 — below the
        // threshold — so no filter fires.
        let region = noise(512, 0xDEAD_BEEF);
        assert!(decide_window(&region, TNS_ENC_MAX_ORDER).is_none());
    }

    #[test]
    fn correlated_region_activates_and_analysis_whitens() {
        // Long window, fs 48 kHz. Fill the coverable region with a
        // strongly correlated AR process; TNS must fire, the analysis
        // pass must reduce the region's energy (whitening), and the
        // decoder's tns_decode_frame must restore the original
        // spectrum exactly (the §4.6.9.3 analysis∘synthesis
        // identity on the shared quantised filter).
        let fs = 3u8;
        let seq = WindowSequence::OnlyLong;
        let n = LONG_WINDOW_LEN as usize;
        let x = noise(n, 0x1234_5678);
        let mut spec = all_pole(&x, &[-1.2, 0.5]);
        // Scale to a realistic coefficient magnitude.
        for v in spec.iter_mut() {
            *v *= 1000.0;
        }
        let original = spec.clone();
        let max_sfb = (long_window_offsets(fs).unwrap().len() - 1) as u8;

        let tns = detect_and_apply_tns(&mut spec, seq, max_sfb, fs, &[true])
            .unwrap()
            .expect("correlated spectrum must activate TNS");
        assert_eq!(tns.windows.len(), 1);
        assert_eq!(tns.windows[0].filters.len(), 1);
        let f = &tns.windows[0].filters[0];
        assert!(f.order >= 1);
        assert_eq!(f.coef.len(), f.order as usize);
        assert!(!f.direction);

        let e = |s: &[f64]| s.iter().map(|&v| v * v).sum::<f64>();
        assert!(
            e(&spec) < 0.8 * e(&original),
            "analysis should whiten: {} vs {}",
            e(&spec),
            e(&original)
        );

        // Round-trip: the decoder synthesis restores the original.
        tns_decode_frame(&mut spec, &tns, seq, max_sfb, AOT_AAC_LC, fs).unwrap();
        for (a, b) in spec.iter().zip(original.iter()) {
            assert!((a - b).abs() < 1e-6, "synthesis must invert analysis");
        }
    }

    #[test]
    fn short_windows_decide_independently() {
        // Eight short windows: give window 3 a correlated region and
        // leave the rest white — only window 3 fires.
        let fs = 3u8;
        let seq = WindowSequence::EightShort;
        let wlen = SHORT_WINDOW_LEN as usize;
        let mut spec = vec![0.0f64; 8 * wlen];
        for w in 0..8 {
            let seed = 0x9E37_79B9u32.wrapping_add(w as u32);
            let x = noise(wlen, seed);
            let win = if w == 3 {
                all_pole(&x, &[-1.4, 0.6])
            } else {
                x
            };
            for (i, v) in win.iter().enumerate() {
                spec[w * wlen + i] = v * 500.0;
            }
        }
        let max_sfb = (short_window_offsets(fs).unwrap().len() - 1) as u8;
        let tns = detect_and_apply_tns(&mut spec, seq, max_sfb, fs, &[true; 8])
            .unwrap()
            .expect("window 3 must activate");
        assert_eq!(tns.windows.len(), 8);
        assert!(!tns.windows[3].filters.is_empty(), "window 3 fires");
        for w in [0usize, 1, 2, 4, 5, 6, 7] {
            assert!(
                tns.windows[w].filters.is_empty(),
                "white window {w} must not fire"
            );
        }
        // Short-window field caps: order ≤ 7, length fits 4 bits.
        let f = &tns.windows[3].filters[0];
        assert!(f.order <= 7);
        assert!(f.length <= 15);
    }

    #[test]
    fn silent_spectrum_never_activates() {
        let fs = 4u8;
        let mut spec = vec![0.0f64; LONG_WINDOW_LEN as usize];
        let max_sfb = (long_window_offsets(fs).unwrap().len() - 1) as u8;
        let tns = detect_and_apply_tns(&mut spec, WindowSequence::OnlyLong, max_sfb, fs, &[true])
            .unwrap();
        assert!(tns.is_none());
        assert!(spec.iter().all(|&v| v == 0.0));
    }
}

//! Long-Term Prediction (LTP) synthesis — ISO/IEC 14496-3 §4.6.7.
//!
//! LTP is a forward-adaptive, single-tap time-domain predictor that
//! reduces inter-frame redundancy for signals with a clear pitch.
//! Because the predictor coefficients are transmitted as side
//! information (`ltp_data()`, Table 4.55, parsed by
//! [`crate::ics_info::LtpData`]), the decoder applies the predictor
//! without the round-off sensitivity of the backward-adaptive MPEG-2
//! frequency-domain predictor (§4.6.6).
//!
//! ## Scope of this module
//!
//! This module implements the §4.6.7.3 **long-window** decoding
//! process, which is the only window family LTP supports for the AAC
//! LTP audio object type (§4.6.7.1 restricts LTP to long windows for
//! bitstream compatibility with MPEG-2 AAC). The three long sequences
//! (`ONLY_LONG_SEQUENCE`, `LONG_START_SEQUENCE`, `LONG_STOP_SEQUENCE`)
//! are handled; `EIGHT_SHORT_SEQUENCE` is a no-op here (LTP is disabled
//! and the per-window predictors are reset, §4.6.7.3 / the short-block
//! reset note).
//!
//! The decode steps, transcribed from the §4.6.7.3 pseudo code:
//!
//! ```text
//! x_est = predict();              // 1-tap time-domain prediction
//! X_est = MDCT(x_est);            // windowed analysis transform
//! for (sfb = 0; sfb < num_sfb; sfb++)
//!     if (ltp_data_present && ltp_long_used[sfb])
//!         X_rec = X_est + Y_rec;  // add predicted spectrum
//!     else
//!         X_rec = Y_rec;          // pass the transmitted spectrum
//! ```
//!
//! * `predict()` forms `x_est(i) = ltp_coef · x_rec(i − M − ltp_lag)`,
//!   `i = 0 … N−1`, with `M = 0` for every non-LD AOT and `M = N/2`
//!   for ER AAC LD (§4.6.7.3; the LD lag is 10-bit with the
//!   `ltp_lag_update` repeat, §4.6.7.2). `x_rec` is the per-channel
//!   reconstruction history (see [`LtpState`]).
//! * `MDCT(x_est)` windows `x_est` with the current frame's §4.6.11
//!   long window and applies the §4.6.15.3.3 analysis transform
//!   ([`crate::filterbank::forward_mdct`]).
//! * `Y_rec` is the decoded (de-interleaved, inverse-quantised)
//!   spectrum; `X_est + Y_rec` replaces it on the sfb that carry
//!   `ltp_long_used == 1`.
//!
//! Per §4.6.7.4.1 (Figure 4.30) the LTP add precedes TNS synthesis in
//! the decode chain, so the spectrum passed in / out here is the
//! pre-TNS reconstructed spectrum.

use crate::filterbank::{forward_mdct, long_only_window_family, short_window_j};
use crate::ics_info::{IcsInfo, LtpData, WindowSequence, WindowShape};
#[cfg(test)]
use crate::swb_offset::long_window_offsets;
#[cfg(test)]
use crate::swb_offset::LONG_WINDOW_LEN;
use crate::swb_offset::{short_window_offsets, FrameFamily, SHORT_WINDOW_LEN};
use crate::Error;

type Result<T> = core::result::Result<T, Error>;

/// The short transform length `N_s = 2 · 128 = 256` (§4.6.11.3.1).
const SHORT_TRANSFORM_LEN: usize = 2 * SHORT_WINDOW_LEN as usize;

/// ISO/IEC 14496-3:2001 §4.6.7.3 — the number of scalefactor bands a
/// short-window LTP contribution covers ("for (sfb = 0; sfb < 8;
/// sfb++)": the first 8 SFBs of each predicted subwindow only).
pub const LTP_SHORT_MAX_SFB: usize = 8;

/// Table 4.98 — the 8-entry LTP coefficient codebook. `ltp_coef`
/// (3 bits) indexes this table; the value is the single-tap predictor
/// gain applied in [`LtpState::predict_long`].
pub const LTP_COEF: [f64; 8] = [
    0.570829, 0.696616, 0.813004, 0.911304, 0.984900, 1.067894, 1.194601, 1.369533,
];

/// Map a 3-bit `ltp_coef` index to its Table 4.98 gain.
///
/// Errors: [`Error::LtpInvalid`] if `index > 7`.
pub fn ltp_coefficient(index: u8) -> Result<f64> {
    LTP_COEF
        .get(index as usize)
        .copied()
        .ok_or(Error::LtpInvalid)
}

/// Per-channel LTP reconstruction-history buffer (§4.6.7.3).
///
/// The predictor reads `x_rec(i − M − ltp_lag)`; the buffer therefore
/// has to retain enough past output to cover the maximum lag
/// (`ltp_lag ≤ 2047`) plus the current transform window. The layout,
/// per §4.6.7.3:
///
/// * `x_rec(0 … N/2 − 1)` — the last aliased half window from the
///   current frame's IMDCT (the pre-overlap-add windowed tail);
/// * `x_rec(N/2 … N − 1)` — always all zeros;
/// * `x_rec(i < 0)` — the previous fully reconstructed time-domain
///   output of the decoder.
///
/// [`Self::history`] stores the `i < 0` region in chronological order
/// (oldest first), so `x_rec(j)` for `j < 0` is
/// `history[history.len() + j]`. [`Self::aliased_tail`] stores
/// `x_rec(0 … N/2 − 1)`. At the start of decoding the whole buffer is
/// zero, matching the §4.6.7.3 initialisation.
#[derive(Clone, Debug, Default)]
pub struct LtpState {
    /// The §4.5.1.1 frame-length family this channel decodes under.
    /// Sets the transform length `N`, the aliased-tail length `N/2`,
    /// the §4.6.7.3 LD lag offset `M = N/2`, and the history depth.
    family: FrameFamily,
    /// Previously reconstructed decoder output (the `i < 0` region),
    /// oldest sample first. Capped at [`Self::history_cap`] samples.
    history: Vec<f64>,
    /// `x_rec(0 … N/2 − 1)` — the current frame's aliased IMDCT half
    /// window, `family.frame_len()` samples. Empty before the first
    /// frame (treated as zeros).
    aliased_tail: Vec<f64>,
    /// §4.6.7.2 (ER AAC LD) `ltp_prev_lag` — the last transmitted
    /// `ltp_lag`, repeated when a frame signals
    /// `ltp_lag_update == 0`. Zero before any lag was transmitted.
    prev_lag: u16,
}

impl LtpState {
    /// Maximum 11-bit `ltp_lag` (§4.6.7.2), used to size the history
    /// buffer so the deepest possible prediction still has data.
    const MAX_LAG: usize = 2047;

    /// A fresh, all-zero LTP state (§4.6.7.3 initialisation) for the
    /// 1024-line family.
    pub fn new() -> Self {
        Self::default()
    }

    /// A fresh, all-zero LTP state for an arbitrary §4.5.1.1 family.
    /// For the LD families this arms the §4.6.7.3 `M = N/2` lag
    /// offset, the 10-bit lag range and the `ltp_prev_lag` repeat
    /// mechanism (§4.6.17.2.6 scales the delay buffer with the frame,
    /// 2048 / 1920 samples for N = 512 / 480).
    pub fn new_family(family: FrameFamily) -> Self {
        LtpState {
            family,
            ..Self::default()
        }
    }

    /// §4.6.7.3 — the LD lag offset `M`: `N/2` (== the frame length,
    /// since `N` is the transform window length `2 × frame_len`) for
    /// ER AAC LD, `0` otherwise.
    fn lag_offset(&self) -> usize {
        if self.family.is_ld() {
            self.family.frame_len()
        } else {
            0
        }
    }

    /// Resolve this frame's effective `ltp_lag` and update the
    /// `ltp_prev_lag` repeat state (§4.6.7.2, ER AAC LD): a
    /// transmitted lag becomes the new `ltp_prev_lag`; an absent lag
    /// (LD `ltp_lag_update == 0`) repeats the previous one. Non-LD
    /// streams always transmit the 11-bit lag, so the repeat arm is
    /// only reachable for LD.
    fn resolve_lag(&mut self, ltp: &LtpData) -> Result<u16> {
        match ltp.lag {
            Some(lag) => {
                self.prev_lag = lag;
                Ok(lag)
            }
            None => {
                if ltp.lag_update == Some(false) {
                    Ok(self.prev_lag)
                } else {
                    // A missing lag without the LD repeat signal is a
                    // malformed in-memory record.
                    Err(Error::LtpInvalid)
                }
            }
        }
    }

    /// Number of past-output samples to retain. The predictor needs
    /// `M + ltp_lag` samples before index 0 (`M = N/2` for LD), and
    /// the deepest window read is `N − 1`, so `MAX_LAG + M + N` past
    /// samples always suffice; the non-LD families keep the historic
    /// `MAX_LAG + N` depth.
    fn history_cap(&self) -> usize {
        Self::MAX_LAG + self.lag_offset() + self.family.long_transform_len()
    }

    /// Read `x_rec(j)` for any integer index `j` per the §4.6.7.3
    /// buffer arrangement. Out-of-range indices (deeper than the
    /// retained history, or `j ≥ N`) read as zero, matching the
    /// zero-initialised buffer.
    fn x_rec(&self, j: isize) -> f64 {
        let half = self.family.frame_len() as isize; // N/2
        if j < 0 {
            // Previous fully reconstructed output, chronological.
            let idx = self.history.len() as isize + j;
            if idx < 0 {
                0.0
            } else {
                self.history[idx as usize]
            }
        } else if j < half {
            // Aliased IMDCT half window.
            self.aliased_tail.get(j as usize).copied().unwrap_or(0.0)
        } else {
            // x_rec(N/2 … N−1) is always zero.
            0.0
        }
    }

    /// §4.6.7.3 `predict()` — form the predicted time-domain signal
    /// `x_est(i) = ltp_coef · x_rec(i − M − ltp_lag)`, `i = 0 … N−1`,
    /// with `M = N/2` for the ER AAC LD families and `M = 0`
    /// otherwise.
    fn predict_long(&self, lag: u16, coef: f64) -> Vec<f64> {
        let shift = lag as isize + self.lag_offset() as isize;
        (0..self.family.long_transform_len() as isize)
            .map(|i| coef * self.x_rec(i - shift))
            .collect()
    }

    /// Update the history after a frame is fully reconstructed.
    ///
    /// * `output` — this frame's `LONG_WINDOW_LEN` (1024) PCM samples,
    ///   i.e. the §4.6.11.3.3 overlap-added output, which become the
    ///   `i < 0` region for subsequent frames.
    /// * `aliased_tail` — this frame's `x_rec(0 … N/2 − 1)`, the
    ///   pre-overlap-add windowed IMDCT tail of length
    ///   `LONG_WINDOW_LEN`.
    ///
    /// Call once per frame, after synthesis, regardless of whether LTP
    /// was active, so the predictor history stays continuous.
    pub fn push_frame(&mut self, output: &[f64], aliased_tail: &[f64]) {
        self.history.extend_from_slice(output);
        let cap = self.history_cap();
        if self.history.len() > cap {
            let excess = self.history.len() - cap;
            self.history.drain(0..excess);
        }
        self.aliased_tail.clear();
        self.aliased_tail.extend_from_slice(aliased_tail);
    }

    /// §4.6.7.3 — apply long-window LTP to one channel's reconstructed
    /// spectrum in place.
    ///
    /// * `spec` — the `LONG_WINDOW_LEN` (1024) decoded coefficients
    ///   `Y_rec`, modified to `X_rec` on the predicted bands.
    /// * `ics_info` — provides `window_sequence`, `window_shape` and
    ///   `max_sfb`; LTP only acts on the three long sequences.
    /// * `ltp` — the parsed §4.6.7.2 side info for this channel.
    /// * `prev_shape` — the previous block's `window_shape`, governing
    ///   the left half of this block's analysis window
    ///   (§4.6.11.3.2). `None` before the first frame, in which case
    ///   the block's own shape is used for both halves.
    /// * `fs_index` — the sampling-frequency index, selecting the
    ///   §4.5.4 long-window scalefactor-band offsets.
    ///
    /// When LTP is inactive (short sequence, or `ltp.long_used` all
    /// false) the spectrum is left untouched. Errors:
    /// [`Error::LtpInvalid`] for an out-of-range `ltp_coef`, a missing
    /// `ltp_lag`, or a spectrum length that is not `LONG_WINDOW_LEN`;
    /// the [`Error`] surfaced by [`long_window_offsets`] for a bad
    /// `fs_index`.
    pub fn apply_long(
        &mut self,
        spec: &mut [f64],
        ics_info: &IcsInfo,
        ltp: &LtpData,
        prev_shape: Option<WindowShape>,
        fs_index: u8,
    ) -> Result<()> {
        self.apply_long_with_analysis(spec, ics_info, ltp, prev_shape, fs_index, |_| Ok(()))
    }

    /// §4.6.7.3 + §4.6.7.4.1 — the LTP long-window add with the
    /// Figure 4.30 **TNS analysis filter** inserted between
    /// `X_est = MDCT(x_est)` and the per-sfb `X_rec = X_est + Y_rec`.
    ///
    /// When TNS is active on the channel, the transmitted residual
    /// `Y_rec` carried in `spec` lives in the noise-shaped (pre-TNS-
    /// synthesis) domain. The LTP-predicted spectrum `X_est` is a clean
    /// MDCT, so it has to be pushed through the same all-zero TNS
    /// analysis filter before it can be added like-for-like. `analyze`
    /// applies that filter in place to the freshly transformed `X_est`
    /// (length `LONG_WINDOW_LEN`); pass a no-op closure when the channel
    /// carries no TNS (which is what [`Self::apply_long`] does).
    ///
    /// The subsequent §4.6.9 TNS *synthesis* pass over the combined
    /// `X_rec` (run by the element driver after this add) undoes the
    /// analysis on the LTP contribution while shaping the residual,
    /// per the §4.6.7.4.1 inverse-filter relationship.
    ///
    /// All other semantics match [`Self::apply_long`].
    pub fn apply_long_with_analysis<F>(
        &mut self,
        spec: &mut [f64],
        ics_info: &IcsInfo,
        ltp: &LtpData,
        prev_shape: Option<WindowShape>,
        fs_index: u8,
        analyze: F,
    ) -> Result<()>
    where
        F: FnOnce(&mut [f64]) -> Result<()>,
    {
        // §4.6.7.3 / short-block note: prediction is disabled for
        // EIGHT_SHORT_SEQUENCE in the long-window LTP path.
        if ics_info.window_sequence == WindowSequence::EightShort {
            return Ok(());
        }
        // The channel state and the frame side info must agree on the
        // §4.5.1.1 family (transform length, LD lag offset).
        if ics_info.family != self.family {
            return Err(Error::LtpInvalid);
        }
        if spec.len() != self.family.frame_len() {
            return Err(Error::LtpInvalid);
        }
        // The effective lag (with the LD ltp_prev_lag repeat) must be
        // resolved on EVERY LTP-bearing frame — even one that flags no
        // bands — so the repeat state tracks the wire exactly.
        let lag = self.resolve_lag(ltp)?;
        // No bands flagged → nothing to add.
        if !ltp.long_used.iter().any(|&u| u) {
            return Ok(());
        }

        let coef = ltp_coefficient(ltp.coef)?;

        // predict() → MDCT(x_est).
        let n_transform = self.family.long_transform_len();
        let x_est = self.predict_long(lag, coef);
        let left_shape = prev_shape.unwrap_or(ics_info.window_shape);
        let window = long_only_window_family(self.family, left_shape, ics_info.window_shape);
        let z: Vec<f64> = x_est
            .iter()
            .zip(window.iter())
            .map(|(&x, &w)| x * w)
            .collect();
        let mut x_est_spec = forward_mdct(&z, n_transform);

        // §4.6.7.4.1 / Figure 4.30: TNS analysis filter on X_est.
        analyze(&mut x_est_spec)?;

        // Per-sfb: X_rec = X_est + Y_rec where ltp_long_used[sfb].
        let offsets = crate::swb_offset::long_window_offsets_family(self.family, fs_index)?;
        let num_sfb = ics_info.max_sfb as usize;
        for sfb in 0..num_sfb {
            if !ltp.long_used.get(sfb).copied().unwrap_or(false) {
                continue;
            }
            let start = offsets[sfb] as usize;
            let end = offsets[sfb + 1] as usize;
            for c in start..end.min(spec.len()) {
                spec[c] += x_est_spec[c];
            }
        }
        Ok(())
    }

    /// ISO/IEC 14496-3:**2001** §4.6.7.3 — short-window LTP synthesis
    /// for one channel's `EIGHT_SHORT_SEQUENCE` spectrum, in place.
    ///
    /// This is the reconstruction counterpart of the 2001-edition
    /// `ltp_data()` short branch (`ltp_short_used[w]` /
    /// `ltp_short_lag[w]`, parsed under
    /// [`crate::ics_info::LtpEdition::Iso2001`]). The 2009 edition
    /// **removed** short-window LTP entirely (§4.6.7.1 "LTP is
    /// restricted to long windows only"), so this entry point is never
    /// reached by the 2009 decode chain; it exists for 2001-edition
    /// streams. Per the 2001 pseudo-code, for each of the eight
    /// subwindows `w` flagged `ltp_short_used[w]`:
    ///
    /// ```text
    /// x_est = predict();            // lag = ltp_lag + ltp_short_lag[w]
    /// X_est = MDCT(x_est);          // the 256-point short transform
    /// for (sfb = 0; sfb < 8; sfb++) // first 8 SFBs only
    ///     X_rec = X_est + Y_rec;
    /// ```
    ///
    /// with the same Table 4.98 `ltp_coef` for every subwindow, and
    /// `ltp_short_lag[w] ∈ −8..=7` a per-window *relative* delay added
    /// to the frame's 11-bit `ltp_lag` (`0` when
    /// `ltp_short_lag_present[w] == 0`). A negative combined lag
    /// (possible only when `ltp_lag < 8`) is floored at `0` — the
    /// history holds no future samples.
    ///
    /// ## The `window_origins` parameter — a documented spec ambiguity
    ///
    /// §4.6.7.3 (2001) states the `x_rec` buffer arrangement once, in
    /// terms of a single long transform, and never respecifies the
    /// **index origin of each subwindow** into that shared history —
    /// i.e. which absolute history position subwindow `w`'s
    /// `x_est(0)` reads from (see the staged analysis
    /// `docs/audio/aac/short-window-ltp-blocked.md` §5; no encoder
    /// emits this syntax and no reference decode exists to pin it).
    /// Rather than invent a convention, this routine takes the
    /// per-subwindow origin explicitly: subwindow `w` predicts
    /// `x_est(i) = ltp_coef · x_rec(window_origins[w] + i − lag_w)`
    /// for `i = 0..256`. When a fixture (or errata) eventually fixes
    /// the origin rule, the caller encodes it here without touching
    /// the pinned math.
    ///
    /// Errors: [`Error::LtpInvalid`] when `ics_info` is not
    /// `EIGHT_SHORT_SEQUENCE`, `spec` is not the 8 × 128 window-major
    /// short spectrum, `ltp.short` is missing / not 8 entries, the
    /// frame `ltp_lag` is absent, or `ltp_coef` is out of range.
    pub fn apply_short_2001(
        &self,
        spec: &mut [f64],
        ics_info: &IcsInfo,
        ltp: &LtpData,
        prev_shape: Option<WindowShape>,
        fs_index: u8,
        window_origins: &[isize; 8],
    ) -> Result<()> {
        if ics_info.window_sequence != WindowSequence::EightShort {
            return Err(Error::LtpInvalid);
        }
        let wlen = SHORT_WINDOW_LEN as usize;
        if spec.len() != 8 * wlen {
            return Err(Error::LtpInvalid);
        }
        let Some(short) = ltp.short.as_ref() else {
            return Err(Error::LtpInvalid);
        };
        if short.len() != 8 {
            return Err(Error::LtpInvalid);
        }
        if !short.iter().any(|s| s.used) {
            return Ok(());
        }
        let coef = ltp_coefficient(ltp.coef)?;
        let lag = ltp.lag.ok_or(Error::LtpInvalid)? as isize;
        let offsets = short_window_offsets(fs_index)?;
        let num_sfb = LTP_SHORT_MAX_SFB
            .min(ics_info.max_sfb as usize)
            .min(offsets.len() - 1);
        let left_shape = prev_shape.unwrap_or(ics_info.window_shape);

        for (w, sw) in short.iter().enumerate() {
            if !sw.used {
                continue;
            }
            // lag_w = ltp_lag + ltp_short_lag[w], floored at 0.
            let lag_w = (lag + isize::from(sw.lag)).max(0);
            let origin = window_origins[w];
            let x_est: Vec<f64> = (0..SHORT_TRANSFORM_LEN as isize)
                .map(|i| coef * self.x_rec(origin + i - lag_w))
                .collect();
            // Window subwindow w (window 0's left half inherits the
            // previous block's shape, §4.6.11.3.2) and run the
            // 256-point analysis transform.
            let window = short_window_j(w, left_shape, ics_info.window_shape);
            let z: Vec<f64> = x_est
                .iter()
                .zip(window.iter())
                .map(|(&x, &wv)| x * wv)
                .collect();
            let x_est_spec = forward_mdct(&z, SHORT_TRANSFORM_LEN);
            // X_rec = X_est + Y_rec on the first 8 SFBs.
            let base = w * wlen;
            for sfb in 0..num_sfb {
                let start = offsets[sfb] as usize;
                let end = (offsets[sfb + 1] as usize).min(wlen);
                for c in start..end {
                    spec[base + c] += x_est_spec[c];
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ics_info::WindowSequence;

    fn long_info(max_sfb: u8) -> IcsInfo {
        IcsInfo {
            family: crate::swb_offset::FrameFamily::Lc1024,
            ics_reserved_bit: false,
            window_sequence: WindowSequence::OnlyLong,
            window_shape: WindowShape::Sine,
            max_sfb,
            scale_factor_grouping: None,
            predictor_data_present: false,
            predictor_data: None,
            ltp_data_present: true,
            ltp_data: None,
            ltp_data_present_pair: None,
            ltp_data_pair: None,
            num_windows: 1,
            num_window_groups: 1,
            window_group_length: vec![1],
            num_swb: 49,
        }
    }

    fn ltp_with(coef: u8, lag: u16, long_used: Vec<bool>) -> LtpData {
        LtpData {
            lag_update: None,
            lag: Some(lag),
            coef,
            long_used,
            short: None,
        }
    }

    #[test]
    fn table_4_98_coefficients() {
        // Table 4.98 endpoints and a mid value.
        assert_eq!(ltp_coefficient(0).unwrap(), 0.570829);
        assert_eq!(ltp_coefficient(4).unwrap(), 0.984900);
        assert_eq!(ltp_coefficient(7).unwrap(), 1.369533);
        assert!(ltp_coefficient(8).is_err());
    }

    #[test]
    fn short_sequence_is_noop() {
        let mut st = LtpState::new();
        let mut info = long_info(40);
        info.window_sequence = WindowSequence::EightShort;
        let ltp = ltp_with(0, 100, vec![true; 40]);
        let mut spec = vec![1.0f64; LONG_WINDOW_LEN as usize];
        st.apply_long(&mut spec, &info, &ltp, None, 3).unwrap();
        assert!(spec.iter().all(|&v| v == 1.0));
    }

    #[test]
    fn no_bands_flagged_is_noop() {
        let mut st = LtpState::new();
        // Seed history so a predictor would otherwise fire.
        let out = vec![0.5f64; LONG_WINDOW_LEN as usize];
        let tail = vec![0.25f64; LONG_WINDOW_LEN as usize];
        st.push_frame(&out, &tail);
        let info = long_info(40);
        let ltp = ltp_with(0, 100, vec![false; 40]);
        let mut spec = vec![1.0f64; LONG_WINDOW_LEN as usize];
        st.apply_long(&mut spec, &info, &ltp, None, 3).unwrap();
        assert!(spec.iter().all(|&v| v == 1.0));
    }

    #[test]
    fn zero_history_predicts_zero() {
        // §4.6.7.3 initialisation: x_rec all zero ⇒ x_est all zero ⇒
        // X_est all zero ⇒ spectrum unchanged even with bands flagged.
        let mut st = LtpState::new();
        let info = long_info(40);
        let ltp = ltp_with(7, 50, vec![true; 40]);
        let mut spec = vec![2.0f64; LONG_WINDOW_LEN as usize];
        st.apply_long(&mut spec, &info, &ltp, None, 3).unwrap();
        for &v in &spec {
            assert!((v - 2.0).abs() < 1e-12, "got {v}");
        }
    }

    #[test]
    fn predict_long_applies_lag_and_gain() {
        // Drive the predictor from a known history. With lag L and the
        // i<0 region holding a DC level d, x_est(i) = coef·d for all i
        // whose source index i−L < 0 (i.e. i < L). Verify a handful of
        // sample values directly via the private predictor.
        let mut st = LtpState::new();
        let d = 1.0f64;
        st.history = vec![d; st.history_cap()];
        let coef = ltp_coefficient(2).unwrap(); // 0.813004
        let lag = 64u16;
        let x_est = st.predict_long(lag, coef);
        // i=0: source index −64 (in history) ⇒ coef·d.
        assert!((x_est[0] - coef * d).abs() < 1e-12);
        // i=63: source −1 ⇒ coef·d.
        assert!((x_est[63] - coef * d).abs() < 1e-12);
        // i=64: source 0 ⇒ aliased_tail (empty) ⇒ 0.
        assert!(x_est[64].abs() < 1e-12);
    }

    #[test]
    fn x_rec_regions_are_distinct() {
        let mut st = LtpState::new();
        st.history = vec![3.0; 10];
        st.aliased_tail = vec![7.0; LONG_WINDOW_LEN as usize];
        // i<0 region: most-recent past = 3.0.
        assert_eq!(st.x_rec(-1), 3.0);
        // Beyond retained history reads zero.
        assert_eq!(st.x_rec(-100), 0.0);
        // 0..N/2 is the aliased tail.
        assert_eq!(st.x_rec(0), 7.0);
        assert_eq!(st.x_rec(LONG_WINDOW_LEN as isize - 1), 7.0);
        // N/2..N is always zero.
        assert_eq!(st.x_rec(LONG_WINDOW_LEN as isize), 0.0);
    }

    #[test]
    fn push_frame_caps_history() {
        let mut st = LtpState::new();
        for _ in 0..4 {
            let out = vec![1.0f64; LONG_WINDOW_LEN as usize];
            let tail = vec![0.0f64; LONG_WINDOW_LEN as usize];
            st.push_frame(&out, &tail);
        }
        assert!(st.history.len() <= st.history_cap());
    }

    // ===== ISO/IEC 14496-3:2001 §4.6.7.3 short-window LTP =====

    use crate::ics_info::LtpShortWindow;

    fn short_info(max_sfb: u8) -> IcsInfo {
        IcsInfo {
            family: crate::swb_offset::FrameFamily::Lc1024,
            ics_reserved_bit: false,
            window_sequence: WindowSequence::EightShort,
            window_shape: WindowShape::Sine,
            max_sfb,
            scale_factor_grouping: Some(0),
            predictor_data_present: false,
            predictor_data: None,
            ltp_data_present: true,
            ltp_data: None,
            ltp_data_present_pair: None,
            ltp_data_pair: None,
            num_windows: 8,
            num_window_groups: 8,
            window_group_length: vec![1; 8],
            num_swb: 14,
        }
    }

    fn short_ltp(coef: u8, lag: u16, windows: [Option<i8>; 8]) -> LtpData {
        LtpData {
            lag_update: None,
            lag: Some(lag),
            coef,
            long_used: vec![],
            short: Some(
                windows
                    .iter()
                    .map(|w| match w {
                        Some(l) => LtpShortWindow {
                            used: true,
                            lag_present: *l != 0,
                            lag: *l,
                        },
                        None => LtpShortWindow {
                            used: false,
                            lag_present: false,
                            lag: 0,
                        },
                    })
                    .collect(),
            ),
        }
    }

    /// Natural subwindow-grid origins for the tests: subwindow w's
    /// x_est(0) reads history position w·128 (one convention among
    /// those the 2001 text admits — the routine deliberately takes
    /// the origins from the caller; see the method docs).
    fn grid_origins() -> [isize; 8] {
        core::array::from_fn(|w| (w as isize) * SHORT_WINDOW_LEN as isize)
    }

    #[test]
    fn short_2001_rejects_bad_shapes() {
        let st = LtpState::new();
        let ltp = short_ltp(0, 100, [Some(0); 8]);
        let origins = grid_origins();
        // Long sequence rejected.
        let mut spec = vec![0.0f64; 8 * SHORT_WINDOW_LEN as usize];
        let info = long_info(40);
        assert!(st
            .apply_short_2001(&mut spec, &info, &ltp, None, 3, &origins)
            .is_err());
        // Wrong spectrum length rejected.
        let sinfo = short_info(8);
        let mut bad = vec![0.0f64; 100];
        assert!(st
            .apply_short_2001(&mut bad, &sinfo, &ltp, None, 3, &origins)
            .is_err());
        // Missing short records rejected.
        let mut no_short = short_ltp(0, 100, [Some(0); 8]);
        no_short.short = None;
        assert!(st
            .apply_short_2001(&mut spec, &sinfo, &no_short, None, 3, &origins)
            .is_err());
    }

    #[test]
    fn short_2001_no_used_window_is_noop() {
        let mut st = LtpState::new();
        st.history = vec![1.0; st.history_cap()];
        st.aliased_tail = vec![0.5; LONG_WINDOW_LEN as usize];
        let info = short_info(8);
        let ltp = short_ltp(3, 64, [None; 8]);
        let mut spec = vec![2.0f64; 8 * SHORT_WINDOW_LEN as usize];
        st.apply_short_2001(&mut spec, &info, &ltp, None, 3, &grid_origins())
            .unwrap();
        assert!(spec.iter().all(|&v| v == 2.0));
    }

    #[test]
    fn short_2001_zero_history_predicts_zero() {
        let st = LtpState::new();
        let info = short_info(8);
        let ltp = short_ltp(7, 64, [Some(0); 8]);
        let mut spec = vec![1.5f64; 8 * SHORT_WINDOW_LEN as usize];
        st.apply_short_2001(&mut spec, &info, &ltp, None, 3, &grid_origins())
            .unwrap();
        for &v in &spec {
            assert!((v - 1.5).abs() < 1e-12);
        }
    }

    #[test]
    fn short_2001_only_used_windows_and_first_8_sfbs_change() {
        // Non-trivial history; flag only subwindow 2. Its first-8-sfb
        // region gains X_est energy, its upper bands stay untouched,
        // and every other subwindow is untouched entirely.
        let mut st = LtpState::new();
        st.history = (0..st.history_cap())
            .map(|i| ((i % 37) as f64) / 17.0 - 1.0)
            .collect();
        st.aliased_tail = vec![0.25; LONG_WINDOW_LEN as usize];
        let fs = 3u8;
        let info = short_info(14);
        let mut flags = [None; 8];
        flags[2] = Some(0);
        let ltp = short_ltp(4, 200, flags);
        let wlen = SHORT_WINDOW_LEN as usize;
        let mut spec = vec![0.0f64; 8 * wlen];
        st.apply_short_2001(&mut spec, &info, &ltp, None, fs, &grid_origins())
            .unwrap();

        let offsets = short_window_offsets(fs).unwrap();
        let cutoff = offsets[LTP_SHORT_MAX_SFB] as usize;
        // Subwindow 2, first 8 sfbs: changed.
        let low = &spec[2 * wlen..2 * wlen + cutoff];
        assert!(
            low.iter().any(|&v| v.abs() > 1e-9),
            "flagged region changed"
        );
        // Subwindow 2 above sfb 8: untouched.
        assert!(spec[2 * wlen + cutoff..3 * wlen].iter().all(|&v| v == 0.0));
        // All other subwindows: untouched.
        for w in [0usize, 1, 3, 4, 5, 6, 7] {
            assert!(
                spec[w * wlen..(w + 1) * wlen].iter().all(|&v| v == 0.0),
                "unflagged subwindow {w} must stay silent"
            );
        }
    }

    #[test]
    fn short_2001_relative_lag_shifts_the_source() {
        // Same frame lag, different ltp_short_lag: the predictor must
        // read a shifted history slice, so the two X_est contributions
        // differ. History is an impulse train so any shift changes
        // the windowed segment.
        let mut st = LtpState::new();
        st.history = (0..st.history_cap())
            .map(|i| if i % 64 == 0 { 1.0 } else { 0.0 })
            .collect();
        st.aliased_tail = vec![0.0; LONG_WINDOW_LEN as usize];
        let info = short_info(8);
        let wlen = SHORT_WINDOW_LEN as usize;
        let run = |short_lag: i8| -> Vec<f64> {
            let mut flags = [None; 8];
            flags[0] = Some(short_lag);
            let ltp = short_ltp(4, 300, flags);
            let mut spec = vec![0.0f64; 8 * wlen];
            st.apply_short_2001(&mut spec, &info, &ltp, None, 3, &grid_origins())
                .unwrap();
            spec[..wlen].to_vec()
        };
        let a = run(0);
        let b = run(7);
        let c = run(-8);
        assert!(a.iter().zip(&b).any(|(x, y)| (x - y).abs() > 1e-9));
        assert!(a.iter().zip(&c).any(|(x, y)| (x - y).abs() > 1e-9));
    }

    #[test]
    fn short_2001_origin_convention_is_callers_choice() {
        // The documented §4.6.7.3 (2001) ambiguity: the same frame
        // under two different origin conventions produces different
        // contributions — pinning that the routine faithfully defers
        // the choice rather than hard-coding one.
        let mut st = LtpState::new();
        st.history = (0..st.history_cap())
            .map(|i| ((i * 7919) % 251) as f64 / 125.0 - 1.0)
            .collect();
        st.aliased_tail = vec![0.0; LONG_WINDOW_LEN as usize];
        let info = short_info(8);
        let wlen = SHORT_WINDOW_LEN as usize;
        let mut flags = [None; 8];
        flags[5] = Some(0);
        let ltp = short_ltp(2, 500, flags);
        let run = |origins: [isize; 8]| -> Vec<f64> {
            let mut spec = vec![0.0f64; 8 * wlen];
            st.apply_short_2001(&mut spec, &info, &ltp, None, 3, &origins)
                .unwrap();
            spec
        };
        let grid = run(grid_origins());
        let zeroed = run([0; 8]);
        assert!(grid.iter().zip(&zeroed).any(|(x, y)| (x - y).abs() > 1e-9));
    }

    #[test]
    fn nonzero_history_modifies_flagged_bands_only() {
        // With nonzero history, a flagged sfb gains X_est energy while
        // an unflagged sfb is untouched. fs_index 3 (48 kHz) long
        // offsets: sfb 0 = [0,4), so flag sfb 0 only and check bins
        // 0..4 changed but a high bin is unchanged.
        let mut st = LtpState::new();
        st.history = vec![1.0; st.history_cap()];
        st.aliased_tail = vec![0.5; LONG_WINDOW_LEN as usize];
        let mut used = vec![false; 40];
        used[0] = true;
        let info = long_info(40);
        let ltp = ltp_with(5, 30, used);
        let baseline = vec![0.0f64; LONG_WINDOW_LEN as usize];
        let mut spec = baseline.clone();
        st.apply_long(&mut spec, &info, &ltp, None, 3).unwrap();
        let offsets = long_window_offsets(3).unwrap();
        let sfb0_end = offsets[1] as usize;
        let changed = (0..sfb0_end).any(|c| (spec[c] - baseline[c]).abs() > 1e-9);
        assert!(changed, "flagged sfb 0 should change");
        // A bin well above sfb 0 must be unchanged.
        assert!((spec[sfb0_end + 50] - baseline[sfb0_end + 50]).abs() < 1e-12);
    }

    // ---- ER AAC LD (§4.6.7.3 M = N/2, §4.6.7.2 ltp_prev_lag) ----

    fn ld_info(family: FrameFamily, max_sfb: u8) -> IcsInfo {
        let mut info = long_info(max_sfb);
        info.family = family;
        info.num_swb = 36;
        info
    }

    fn ld_ltp(coef: u8, lag: Option<u16>, long_used: Vec<bool>) -> LtpData {
        LtpData {
            lag_update: Some(lag.is_some()),
            lag,
            coef,
            long_used,
            short: None,
        }
    }

    #[test]
    fn ld_predict_reads_with_m_offset() {
        // Place a single impulse in the history and verify the LD
        // predictor reads it at i = M + lag − depth… i.e. that
        // x_est(i) = coef · x_rec(i − M − lag) with M = frame_len.
        let mut st = LtpState::new_family(FrameFamily::Ld512);
        // history: 2000 zeros with an impulse 100 samples back
        // (x_rec(−100) = 1.0).
        let mut hist = vec![0.0f64; 2000];
        let hlen = hist.len();
        hist[hlen - 100] = 1.0;
        st.history = hist;
        let coef = ltp_coefficient(0).unwrap();
        // lag = 40, M = 512: x_est(i) = coef·x_rec(i − 552); the
        // impulse at x_rec(−100) lands at i = 452.
        let x_est = st.predict_long(40, coef);
        assert_eq!(x_est.len(), 1024); // N = 1024 for LD512
        for (i, &v) in x_est.iter().enumerate() {
            if i == 452 {
                assert!((v - coef).abs() < 1e-15, "impulse at {i}: {v}");
            } else {
                assert_eq!(v, 0.0, "unexpected non-zero at {i}");
            }
        }
    }

    #[test]
    fn ld_480_predict_geometry() {
        let mut st = LtpState::new_family(FrameFamily::Ld480);
        let mut hist = vec![0.0f64; 2000];
        let hlen = hist.len();
        hist[hlen - 1] = 1.0; // x_rec(−1) = 1.0
        st.history = hist;
        let coef = ltp_coefficient(3).unwrap();
        // M = 480, lag = 0: impulse lands at i = 479.
        let x_est = st.predict_long(0, coef);
        assert_eq!(x_est.len(), 960);
        assert!((x_est[479] - coef).abs() < 1e-15);
        assert_eq!(x_est[480], 0.0);
    }

    #[test]
    fn ld_prev_lag_repeat() {
        // Frame 1 transmits lag 123 (ltp_lag_update == 1); frame 2
        // repeats it (ltp_lag_update == 0, no lag on the wire). Both
        // frames must predict identically from the same history.
        let mut info = ld_info(FrameFamily::Ld512, 36);
        info.num_swb = 36;
        let mut st = LtpState::new_family(FrameFamily::Ld512);
        st.history = (0..2048).map(|i| ((i * 37) % 101) as f64 * 0.01).collect();
        st.aliased_tail = vec![0.0; 512];

        let with_lag = ld_ltp(2, Some(123), vec![true; 36]);
        let repeat = ld_ltp(2, None, vec![true; 36]);

        let mut spec_a = vec![0.0f64; 512];
        let mut st_a = st.clone();
        st_a.apply_long(&mut spec_a, &info, &with_lag, None, 3)
            .unwrap();

        // Same state, but resolve the transmitted lag first and then
        // decode a repeat frame — must produce the same contribution.
        let mut st_b = st.clone();
        let mut warmup = vec![0.0f64; 512];
        st_b.apply_long(&mut warmup, &info, &with_lag, None, 3)
            .unwrap();
        let mut spec_b = vec![0.0f64; 512];
        st_b.apply_long(&mut spec_b, &info, &repeat, None, 3)
            .unwrap();

        assert!(spec_a.iter().any(|&v| v != 0.0), "LTP must contribute");
        for (a, b) in spec_a.iter().zip(spec_b.iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn ld_repeat_without_prior_lag_uses_zero() {
        // ltp_lag_update == 0 before any transmitted lag: the
        // §4.6.7.3 zero-initialised state gives ltp_prev_lag = 0.
        let info = ld_info(FrameFamily::Ld512, 36);
        let mut st = LtpState::new_family(FrameFamily::Ld512);
        st.aliased_tail = vec![0.0; 512];
        let repeat = ld_ltp(2, None, vec![true; 36]);
        let mut spec = vec![0.0f64; 512];
        st.apply_long(&mut spec, &info, &repeat, None, 3).unwrap();
        // Zero history → zero contribution, but no error.
        assert!(spec.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn ld_family_mismatch_rejected() {
        let info = ld_info(FrameFamily::Ld512, 36);
        let mut st = LtpState::new(); // Lc1024 state
        let ltp = ld_ltp(0, Some(1), vec![true; 36]);
        let mut spec = vec![0.0f64; 512];
        assert!(matches!(
            st.apply_long(&mut spec, &info, &ltp, None, 3),
            Err(Error::LtpInvalid)
        ));
    }

    #[test]
    fn missing_lag_without_repeat_signal_rejected() {
        let info = long_info(40);
        let mut st = LtpState::new();
        let ltp = LtpData {
            lag_update: None,
            lag: None,
            coef: 0,
            long_used: vec![true; 40],
            short: None,
        };
        let mut spec = vec![0.0f64; 1024];
        assert!(matches!(
            st.apply_long(&mut spec, &info, &ltp, None, 3),
            Err(Error::LtpInvalid)
        ));
    }
}

//! SBR frame driver — ISO/IEC 14496-3 §4.6.18.5 "SBR tool overview".
//!
//! Composes the whole SBR back-end for one channel element (SCE or
//! CPE): the §4.6.18.4.1 analysis QMF of the core decoder output, the
//! `XLow` buffer with its `tHFGen = 8`-slot cross-frame history, the
//! §4.6.18.6 HF generator, the §4.6.18.7 envelope adjuster, the
//! §4.6.18.5 output matrix `X` assembly (the `lTemp` splice of the
//! previous frame's `Y'` against the current `XLow` / `Y`), and the
//! §4.6.18.4.2 64-band synthesis QMF producing `numTimeSlots·RATE·64 =
//! 2048` output samples per 1024-sample core frame (dual-rate SBR).
//! [`SbrDecoder::set_downsampled`] selects the §4.6.18.4.3 downsampled
//! output mode instead: the 32-channel synthesis bank keeps the output
//! at the core rate (1024 samples per frame), discarding the assembled
//! `X` subbands above the core Nyquist.
//!
//! [`SbrDecoder::process_frame`] drives a parsed
//! [`crate::sbr_extension::SbrExtensionData`];
//! [`SbrDecoder::upsample_frame`] is the §4.6.18.5 "pure upsampling
//! without SBR processing" path used when a frame carries no SBR
//! payload, keeping the selected output rate and the QMF state
//! continuous.
//!
//! ## Provenance
//!
//! The buffer geometry (`tHFGen = 8`, `tHFAdj = 2`, `lf =
//! numTimeSlots·RATE = 32`), the `XLow` history splice, the `lTemp`
//! output splice, and the reset rules are from the §4.6.18.5 text and
//! Figure 4.47 of the staged spec. No part of this implementation is
//! derived from any external decoder.

use crate::ps_decoder::PsDecoder;
use crate::ps_hybrid::LOOKAHEAD;
use crate::sbr_dequant::{dequant_coupled, dequant_single, DequantizedSbr};
use crate::sbr_element::EXTENSION_ID_PS;
use crate::sbr_env_adjust::{adjust, EnvAdjustState, EnvParams};
use crate::sbr_extension::SbrExtensionData;
use crate::sbr_freq_bands::{k0 as derive_k0, k2 as derive_k2, master_table, HiLoTables};
use crate::sbr_header::SbrHeader;
use crate::sbr_hf_gen::{
    build_patches, chirp_factors, generate_hf, reflection_coefficient, Patches, T_HF_ADJ, T_HF_GEN,
};
use crate::sbr_limiter::limiter_table;
use crate::sbr_lp::{aliasing_degree, deg_patched};
use crate::sbr_qmf::{
    AnalysisQmf, Complex, DownsampledSynthesisQmf, RealAnalysisQmf, RealDownsampledSynthesisQmf,
    RealSynthesisQmf, SynthesisQmf,
};
use crate::sbr_reconstruct::{EnvelopeScalefactors, NoiseScalefactors};
use crate::sbr_time_grid::derive_time_grid;
use crate::{Error, Result};

/// `numTimeSlots` for the 1024-sample core frame (§4.6.18.2.6).
pub const NUM_TIME_SLOTS: i32 = 16;

/// `RATE = 2` (§4.6.18.2.5).
pub const RATE: i32 = 2;

/// Slots per frame at the SBR rate (`lf = numTimeSlots · RATE`).
const LF: usize = (NUM_TIME_SLOTS * RATE) as usize;

/// Total `XLow` / `XHigh` / `Y` columns (`lf + tHFGen`).
const COLS: usize = LF + T_HF_GEN;

/// The synthesis filterbank of one output channel: the §4.6.18.4.2
/// 64-band dual-rate bank, or the §4.6.18.4.3 32-channel downsampled
/// bank that keeps the output at the core rate (fed the first 32
/// subbands of the assembled `X` matrix; the SBR content above the
/// core Nyquist is discarded by construction).
#[derive(Debug)]
enum SynthesisBank {
    /// §4.6.18.4.2 — 64 output samples per slot (2× rate).
    Dual(SynthesisQmf),
    /// §4.6.18.4.3 — 32 output samples per slot (core rate).
    Down(DownsampledSynthesisQmf),
    /// §4.6.18.8.2.3 — the real-valued low-power dual-rate bank.
    RealDual(RealSynthesisQmf),
    /// §4.6.18.8.2.4 — the real-valued low-power core-rate bank.
    RealDown(RealDownsampledSynthesisQmf),
}

impl SynthesisBank {
    fn new(downsampled: bool, low_power: bool) -> Self {
        match (low_power, downsampled) {
            (false, false) => SynthesisBank::Dual(SynthesisQmf::new()),
            (false, true) => SynthesisBank::Down(DownsampledSynthesisQmf::new()),
            (true, false) => SynthesisBank::RealDual(RealSynthesisQmf::new()),
            (true, true) => SynthesisBank::RealDown(RealDownsampledSynthesisQmf::new()),
        }
    }

    /// Output samples per QMF slot (64 dual-rate, 32 downsampled).
    fn samples_per_slot(&self) -> usize {
        match self {
            SynthesisBank::Dual(_) | SynthesisBank::RealDual(_) => 64,
            SynthesisBank::Down(_) | SynthesisBank::RealDown(_) => 32,
        }
    }

    /// Synthesize one assembled `X` column, appending the slot's output
    /// samples to `out`. The real (low-power) banks consume the real
    /// parts — the LP signal path never populates the imaginary parts.
    fn push_slot(&mut self, x: &[Complex; 64], out: &mut Vec<f64>) -> Result<()> {
        match self {
            SynthesisBank::Dual(s) => out.extend_from_slice(&s.push_slot(x)?),
            SynthesisBank::Down(s) => out.extend_from_slice(&s.push_slot(&x[..32])?),
            SynthesisBank::RealDual(s) => {
                let mut re = [0.0f64; 64];
                for (r, c) in re.iter_mut().zip(x.iter()) {
                    *r = c.re;
                }
                out.extend_from_slice(&s.push_slot(&re)?);
            }
            SynthesisBank::RealDown(s) => {
                let mut re = [0.0f64; 32];
                for (r, c) in re.iter_mut().zip(x.iter()) {
                    *r = c.re;
                }
                out.extend_from_slice(&s.push_slot(&re)?);
            }
        }
        Ok(())
    }
}

/// The analysis filterbank of one core channel: the §4.6.18.4.1
/// complex bank, or the §4.6.18.8.2.2 real-valued low-power bank
/// (whose output rides the same `Complex` slots with zero imaginary
/// parts, so the HF generator and adjuster formulas apply unchanged).
#[derive(Debug)]
enum AnalysisBank {
    Complex(AnalysisQmf),
    Real(RealAnalysisQmf),
}

impl AnalysisBank {
    fn new(low_power: bool) -> Self {
        if low_power {
            AnalysisBank::Real(RealAnalysisQmf::new())
        } else {
            AnalysisBank::Complex(AnalysisQmf::new())
        }
    }

    fn push_slot(&mut self, samples: &[f64]) -> Result<[Complex; 32]> {
        match self {
            AnalysisBank::Complex(a) => a.push_slot(samples),
            AnalysisBank::Real(a) => {
                let w = a.push_slot(samples)?;
                let mut out = [Complex::default(); 32];
                for (o, &r) in out.iter_mut().zip(w.iter()) {
                    o.re = r;
                }
                Ok(out)
            }
        }
    }
}

/// Per-channel cross-frame state.
#[derive(Debug)]
struct ChannelState {
    analysis: AnalysisBank,
    synthesis: SynthesisBank,
    /// The previous frame's last `tHFGen` analysis slots (`W'`).
    w_hist: Vec<[Complex; 32]>,
    /// The previous frame's `Y` buffer (spec absolute columns).
    y_prev: Vec<[Complex; 64]>,
    /// `tE'(LE')` — the previous frame's trailing envelope border.
    t_e_last_prev: i32,
    /// The previous frame's `kx` / `M` (for the `lTemp` splice).
    k_x_prev: i32,
    m_prev: i32,
    env_state: EnvAdjustState,
    prev_invf: Vec<u8>,
    prev_bw: Vec<f64>,
    prev_env: Option<EnvelopeScalefactors>,
    prev_noise: Option<NoiseScalefactors>,
}

impl ChannelState {
    fn new(downsampled: bool, low_power: bool) -> Self {
        ChannelState {
            analysis: AnalysisBank::new(low_power),
            synthesis: SynthesisBank::new(downsampled, low_power),
            w_hist: vec![[Complex::default(); 32]; T_HF_GEN],
            y_prev: vec![[Complex::default(); 64]; COLS],
            t_e_last_prev: NUM_TIME_SLOTS,
            k_x_prev: 0,
            m_prev: 0,
            env_state: EnvAdjustState::new(),
            prev_invf: Vec::new(),
            prev_bw: Vec::new(),
            prev_env: None,
            prev_noise: None,
        }
    }

    /// Run the analysis QMF over one 1024-sample core frame and build
    /// the `XLow` buffer: columns `0..tHFGen` are the previous frame's
    /// trailing slots (`W'`), columns `tHFGen..` the current `W`.
    fn analyze(&mut self, core: &[f64]) -> Result<Vec<[Complex; 32]>> {
        if core.len() != 1024 {
            return Err(Error::SbrQmfInvalid);
        }
        let mut x_low = Vec::with_capacity(COLS);
        x_low.extend_from_slice(&self.w_hist);
        for slot in 0..LF {
            let w = self.analysis.push_slot(&core[slot * 32..(slot + 1) * 32])?;
            x_low.push(w);
        }
        self.w_hist.clear();
        self.w_hist.extend_from_slice(&x_low[COLS - T_HF_GEN..]);
        Ok(x_low)
    }
}

/// One SBR decoder per channel element (SCE: 1 channel, CPE: 2).
#[derive(Debug)]
pub struct SbrDecoder {
    fs_sbr: u32,
    header: Option<SbrHeader>,
    bands: Option<HiLoTables>,
    patches: Option<Patches>,
    f_table_lim: Vec<i32>,
    /// §4.6.18.4.3 downsampled output mode: the synthesis runs the
    /// 32-channel bank and every frame yields 1024 samples per channel
    /// at the *core* rate instead of 2048 at `fs_sbr`.
    downsampled: bool,
    /// §4.6.18.8 low-power mode: real-valued filterbanks, ×2 energy
    /// estimation, aliasing detection/reduction, modified sinusoid
    /// injection. PS payloads are rejected ([`Error::SbrLowPowerPs`]).
    low_power: bool,
    /// `k0` of the active band setup (the first `fMaster` subband;
    /// the §4.6.18.8.3 reflection coefficients cover `0 ≤ k < k0`).
    k0: i32,
    /// Set once the first frame is processed (mode switches are then
    /// rejected — the QMF synthesis state is rate-specific).
    started: bool,
    channels: Vec<ChannelState>,
    /// Annex 8.A parametric stereo state, created when a
    /// single-channel element first carries a PS extension. Holds the
    /// PS decoder plus the second (right-channel) synthesis bank; the
    /// channel's own bank renders the left channel.
    ps: Option<PsState>,
}

/// PS decoder + right-channel synthesis bank (Annex 8.A).
#[derive(Debug)]
struct PsState {
    dec: PsDecoder,
    synthesis_r: SynthesisBank,
}

impl SbrDecoder {
    /// A fresh SBR decoder. `fs_sbr` is the SBR internal rate (twice
    /// the core rate); `num_channels` is 1 (SCE) or 2 (CPE).
    pub fn new(fs_sbr: u32, num_channels: usize) -> Result<Self> {
        if num_channels == 0 || num_channels > 2 || fs_sbr == 0 {
            return Err(Error::SbrFreqBandInvalid);
        }
        Ok(SbrDecoder {
            fs_sbr,
            header: None,
            bands: None,
            patches: None,
            f_table_lim: Vec::new(),
            downsampled: false,
            low_power: false,
            k0: 0,
            started: false,
            channels: (0..num_channels)
                .map(|_| ChannelState::new(false, false))
                .collect(),
            ps: None,
        })
    }

    /// Select the §4.6.18.4.3 downsampled output mode: the SBR-processed
    /// subband signals are synthesized through the 32-channel QMF bank,
    /// so the output stays at the *core* coder rate (1024 samples per
    /// channel per frame) instead of the dual `fs_sbr` rate. The SBR
    /// range above the core Nyquist (assembled `X` subbands 32..64) is
    /// discarded by construction; the reconstructed bands below it are
    /// kept, so the mode is still an SBR decode, not a plain core decode.
    ///
    /// Must be selected before the first frame is processed — the QMF
    /// synthesis history is rate-specific ([`Error::SbrQmfInvalid`]
    /// otherwise).
    pub fn set_downsampled(&mut self, downsampled: bool) -> Result<()> {
        if self.started {
            return Err(Error::SbrQmfInvalid);
        }
        if self.downsampled != downsampled {
            self.downsampled = downsampled;
            self.rebuild_banks();
        }
        Ok(())
    }

    /// `true` ⇔ the §4.6.18.4.3 downsampled output mode is selected.
    #[must_use]
    pub fn is_downsampled(&self) -> bool {
        self.downsampled
    }

    /// Select the §4.6.18.8 low-power SBR mode: the whole signal path
    /// runs on real-valued subband signals (the §4.6.18.8.2 real
    /// filterbanks), the envelope adjuster applies the §4.6.18.8.4
    /// energy correction and §4.6.18.8.5 aliasing reduction / modified
    /// sinusoid injection, and gain smoothing is disabled. Composable
    /// with [`Self::set_downsampled`]. A PS payload on a low-power
    /// decoder is rejected with [`Error::SbrLowPowerPs`] — the
    /// subpart-8 tool needs the complex QMF domain.
    ///
    /// Must be selected before the first frame is processed
    /// ([`Error::SbrQmfInvalid`] otherwise).
    pub fn set_low_power(&mut self, low_power: bool) -> Result<()> {
        if self.started {
            return Err(Error::SbrQmfInvalid);
        }
        if self.low_power != low_power {
            self.low_power = low_power;
            self.rebuild_banks();
        }
        Ok(())
    }

    /// `true` ⇔ the §4.6.18.8 low-power mode is selected.
    #[must_use]
    pub fn is_low_power(&self) -> bool {
        self.low_power
    }

    /// Re-instantiate every filterbank for the current mode pair
    /// (only legal before the first frame).
    fn rebuild_banks(&mut self) {
        for ch in &mut self.channels {
            ch.analysis = AnalysisBank::new(self.low_power);
            ch.synthesis = SynthesisBank::new(self.downsampled, self.low_power);
        }
        if let Some(ps) = &mut self.ps {
            ps.synthesis_r = SynthesisBank::new(self.downsampled, self.low_power);
        }
    }

    /// §4.6.18.5 pure upsampling: no SBR data for this frame — run the
    /// analysis / synthesis pair with the high 32 bands zero, keeping
    /// the output rate steady and the QMF state continuous.
    ///
    /// `core` holds one 1024-sample time signal per channel; returns
    /// 2048 samples per channel (1024 in the §4.6.18.4.3 downsampled
    /// mode).
    pub fn upsample_frame(&mut self, core: &[&[f64]]) -> Result<Vec<Vec<f64>>> {
        if core.len() != self.channels.len() {
            return Err(Error::SbrQmfInvalid);
        }
        self.started = true;
        let mut out = Vec::with_capacity(core.len());
        let n_ch = self.channels.len();
        for (ch, core_ch) in self.channels.iter_mut().zip(core.iter()) {
            let x_low = ch.analyze(core_ch)?;
            let mut x_cols: Vec<[Complex; 64]> = Vec::with_capacity(LF);
            for l in 0..LF {
                let mut x = [Complex::default(); 64];
                x[..32].copy_from_slice(&x_low[l + T_HF_ADJ]);
                x_cols.push(x);
            }
            let sps = ch.synthesis.samples_per_slot();
            // A PS-active stream holds its stereo parameters over a
            // frame without SBR/PS payload (Annex 8.A.3); the whole
            // 32-band spectrum counts as SBR-covered for the partial
            // reset.
            let mut emitted = false;
            if n_ch == 1 {
                if let Some(ps) = self.ps.as_mut() {
                    let x_input = build_x_input(&x_cols, &x_low);
                    if let Some((lq, rq)) = ps.dec.process(None, &x_input, 32)? {
                        let mut pcm_l = Vec::with_capacity(LF * sps);
                        let mut pcm_r = Vec::with_capacity(LF * sps);
                        for l in 0..LF {
                            ch.synthesis.push_slot(&lq[l], &mut pcm_l)?;
                            ps.synthesis_r.push_slot(&rq[l], &mut pcm_r)?;
                        }
                        out.push(pcm_l);
                        out.push(pcm_r);
                        emitted = true;
                    }
                }
            }
            if !emitted {
                let mut pcm = Vec::with_capacity(LF * sps);
                for x in &x_cols {
                    ch.synthesis.push_slot(x, &mut pcm)?;
                }
                out.push(pcm);
            }
            // No Y for this frame; the next frame's lTemp splice sees
            // an empty previous envelope span.
            ch.y_prev
                .iter_mut()
                .for_each(|c| *c = [Complex::default(); 64]);
            ch.t_e_last_prev = NUM_TIME_SLOTS;
        }
        Ok(out)
    }

    /// Decode one SBR frame: `ext` is the parsed `sbr_extension_data()`
    /// for this element, `core` one 1024-sample signal per channel.
    /// Returns 2048 samples per channel at the SBR rate (1024 per
    /// channel at the core rate in the §4.6.18.4.3 downsampled mode).
    pub fn process_frame(
        &mut self,
        ext: &SbrExtensionData,
        core: &[&[f64]],
    ) -> Result<Vec<Vec<f64>>> {
        let n_ch = self.channels.len();
        if core.len() != n_ch || ext.element.channels.len() != n_ch {
            return Err(Error::SbrFreqBandInvalid);
        }
        self.started = true;

        // §4.6.18.3.3 reset: first header, or a transmitted header that
        // changes the band geometry.
        let reset = match &self.header {
            None => true,
            Some(prev) => prev.band_geometry_changed(&ext.header),
        };
        if reset {
            let k0v = derive_k0(self.fs_sbr, ext.header.start_freq)?;
            let k2v = derive_k2(self.fs_sbr, ext.header.stop_freq, k0v)?;
            let f_master = master_table(k0v, k2v, ext.header.freq_scale, ext.header.alter_scale)?;
            let bands =
                HiLoTables::derive(&f_master, ext.header.xover_band, ext.header.noise_bands)?;
            let patches = build_patches(&f_master, k0v, bands.k_x, bands.m, self.fs_sbr)?;
            self.f_table_lim = limiter_table(
                &bands,
                &patches.borders(bands.k_x),
                ext.header.limiter_bands,
            )?;
            self.bands = Some(bands);
            self.patches = Some(patches);
            self.k0 = k0v;
            for ch in &mut self.channels {
                ch.prev_invf.clear();
                ch.prev_bw.clear();
                ch.prev_env = None;
                ch.prev_noise = None;
            }
        }
        self.header = Some(ext.header);
        let bands = self.bands.as_ref().ok_or(Error::SbrFreqBandInvalid)?;
        let patches = self.patches.as_ref().ok_or(Error::SbrFreqBandInvalid)?;

        let coupling = ext.element.coupling;

        // Reconstruct the quantized scalefactors per transmitted
        // channel, then dequantize (jointly for a coupled pair).
        let mut recon: Vec<(EnvelopeScalefactors, NoiseScalefactors)> = Vec::with_capacity(n_ch);
        for (c, sbr_ch) in ext.element.channels.iter().enumerate() {
            let st = &self.channels[c];
            let env = EnvelopeScalefactors::reconstruct(
                &sbr_ch.envelope,
                &sbr_ch.grid,
                &sbr_ch.dtdf,
                bands,
                coupling,
                c == 1,
                if reset { None } else { st.prev_env.as_ref() },
            )?;
            let noise = NoiseScalefactors::reconstruct(
                &sbr_ch.noise,
                &sbr_ch.grid,
                &sbr_ch.dtdf,
                bands.n_q(),
                coupling,
                c == 1,
                if reset { None } else { st.prev_noise.as_ref() },
            )?;
            recon.push((env, noise));
        }

        let dequant: Vec<DequantizedSbr> = if coupling && n_ch == 2 {
            let amp_res = effective_amp_res(&ext.header, &ext.element.channels[0].grid);
            let (l, r) =
                dequant_coupled(&recon[0].0, &recon[0].1, &recon[1].0, &recon[1].1, amp_res);
            vec![l, r]
        } else {
            (0..n_ch)
                .map(|c| {
                    let amp_res = effective_amp_res(&ext.header, &ext.element.channels[c].grid);
                    dequant_single(&recon[c].0, &recon[c].1, amp_res)
                })
                .collect()
        };

        let mut out = Vec::with_capacity(n_ch);
        for c in 0..n_ch {
            let sbr_ch = &ext.element.channels[c];
            let grid = derive_time_grid(&sbr_ch.grid, NUM_TIME_SLOTS)?;

            // Coupling: the second channel transmits no sbr_invf()
            // (Table 4.66) — it shares the first channel's
            // inverse-filtering modes.
            let invf_modes = if coupling && c == 1 {
                &ext.element.channels[0].invf.invf_mode
            } else {
                &sbr_ch.invf.invf_mode
            };

            let ch = &mut self.channels[c];

            // Chirp factors (per noise band).
            let bw = chirp_factors(invf_modes, &ch.prev_invf, &ch.prev_bw);

            // Analysis + XLow (with tHFGen history).
            let x_low = ch.analyze(core[c])?;

            // HF generation over the envelope span.
            let l_range = (RATE * grid.t_e[0])..(RATE * grid.t_e[grid.t_e.len() - 1]);
            let x_high = generate_hf(&x_low, patches, &bw, bands, l_range, LF)?;

            // §4.6.18.8.3 aliasing detection (low power): reflection
            // coefficients over the low band, the Figure 4.53 degree
            // walk, and the patch carry onto the SBR range.
            let dp = if self.low_power {
                let k0_cnt = usize::try_from(self.k0).map_err(|_| Error::SbrFreqBandInvalid)?;
                let mut refl = Vec::with_capacity(k0_cnt);
                for k in 0..k0_cnt.min(32) {
                    refl.push(reflection_coefficient(&x_low, k, LF)?);
                }
                let deg = aliasing_degree(&refl);
                Some(deg_patched(&deg, patches, bands.k_x, bands.m)?)
            } else {
                None
            };

            // Envelope adjustment.
            let freq_res: Vec<bool> = sbr_ch.grid.freq_res.clone();
            let params = EnvParams {
                bands,
                f_table_lim: &self.f_table_lim,
                t_e: &grid.t_e,
                t_q: &grid.t_q,
                freq_res: &freq_res,
                l_a: grid.l_a,
                e_orig: &dequant[c].e_orig,
                q_orig: &dequant[c].q_orig,
                add_harmonic: &sbr_ch.add_harmonic,
                interpol_freq: ext.header.interpol_freq,
                smoothing_mode: ext.header.smoothing_mode,
                limiter_gains: ext.header.limiter_gains,
                reset,
                low_power: self.low_power,
                deg_patched: dp.as_deref(),
            };
            let y = adjust(&x_high, &params, &mut ch.env_state)?;

            // §4.6.18.5 X assembly.
            let l_temp = (RATE * ch.t_e_last_prev - NUM_TIME_SLOTS * RATE).max(0) as usize;
            let mut x_cols: Vec<[Complex; 64]> = Vec::with_capacity(LF);
            for l in 0..LF {
                let mut x = [Complex::default(); 64];
                let (kx_cur, m_cur, y_col) = if l < l_temp {
                    (ch.k_x_prev, ch.m_prev, &ch.y_prev[l + T_HF_ADJ + LF])
                } else {
                    (bands.k_x, bands.m, &y[l + T_HF_ADJ])
                };
                let kx_u = kx_cur.max(0) as usize;
                for (k, cell) in x.iter_mut().enumerate().take(kx_u.min(32)) {
                    *cell = x_low[l + T_HF_ADJ][k];
                }
                let hi = (kx_cur + m_cur).max(0) as usize;
                // §4.6.18.8.5: the low-power sinusoid spill extends the
                // Y range one subband above the SBR range (≤ 63)…
                let hi = if self.low_power {
                    (hi + 1).min(64)
                } else {
                    hi.min(64)
                };
                if kx_u < hi {
                    x[kx_u..hi].copy_from_slice(&y_col[kx_u..hi]);
                }
                // …and adds Y(kx − 1) onto the lowband subband rather
                // than replacing it.
                if self.low_power && (1..=32).contains(&kx_u) {
                    x[kx_u - 1] += y_col[kx_u - 1];
                }
                x_cols.push(x);
            }

            // Annex 8.A: a single-channel element carrying an
            // EXTENSION_ID_PS payload renders stereo through the PS
            // tool (the element's own bank = left, the PS state's =
            // right). Until the first decodable ps_data() the mono
            // path below stays in effect.
            let ps_payload = if n_ch == 1 {
                ext.element
                    .extension
                    .as_ref()
                    .filter(|e| e.id == EXTENSION_ID_PS)
                    .map(|e| e.data.as_slice())
            } else {
                None
            };
            if ps_payload.is_some() && self.low_power {
                // §4.6.18.8: the real-valued tool cannot host the
                // complex-domain PS processing.
                return Err(Error::SbrLowPowerPs);
            }
            if ps_payload.is_some() && self.ps.is_none() {
                self.ps = Some(PsState {
                    dec: PsDecoder::new(),
                    synthesis_r: SynthesisBank::new(self.downsampled, self.low_power),
                });
            }
            let sps = ch.synthesis.samples_per_slot();
            let mut emitted = false;
            if n_ch == 1 {
                if let Some(ps) = self.ps.as_mut() {
                    let x_input = build_x_input(&x_cols, &x_low);
                    let kx_plus_m = (bands.k_x + bands.m).max(0) as usize;
                    if let Some((lq, rq)) = ps.dec.process(ps_payload, &x_input, kx_plus_m)? {
                        let mut pcm_l = Vec::with_capacity(LF * sps);
                        let mut pcm_r = Vec::with_capacity(LF * sps);
                        for l in 0..LF {
                            ch.synthesis.push_slot(&lq[l], &mut pcm_l)?;
                            ps.synthesis_r.push_slot(&rq[l], &mut pcm_r)?;
                        }
                        out.push(pcm_l);
                        out.push(pcm_r);
                        emitted = true;
                    }
                }
            }
            if !emitted {
                let mut pcm = Vec::with_capacity(LF * sps);
                for x in &x_cols {
                    ch.synthesis.push_slot(x, &mut pcm)?;
                }
                out.push(pcm);
            }

            // Thread cross-frame state.
            ch.y_prev = y;
            ch.t_e_last_prev = grid.t_e[grid.t_e.len() - 1];
            ch.k_x_prev = bands.k_x;
            ch.m_prev = bands.m;
            ch.prev_invf = invf_modes.clone();
            ch.prev_bw = bw;
            let (env, noise) = recon[c].clone();
            ch.prev_env = Some(env);
            ch.prev_noise = Some(noise);
        }
        Ok(out)
    }
}

/// Assemble the Annex 8.A.3 `Xinput` matrix: the 32 assembled `X`
/// columns followed by `LOOKAHEAD` slots taken from `XLow` beyond the
/// frame (`XLow(k, l + tHFAdj)`, `k < 5` — the split bands the hybrid
/// filterbank consumes ahead of time).
fn build_x_input(x_cols: &[[Complex; 64]], x_low: &[[Complex; 32]]) -> Vec<[Complex; 64]> {
    let mut v = Vec::with_capacity(LF + LOOKAHEAD);
    v.extend_from_slice(x_cols);
    for l in LF..LF + LOOKAHEAD {
        let mut col = [Complex::default(); 64];
        col[..5].copy_from_slice(&x_low[l + T_HF_ADJ][..5]);
        v.push(col);
    }
    v
}

/// The effective `bs_amp_res` after the single-envelope FIXFIX
/// override (§4.4.2.8 Table 4.69 Note).
fn effective_amp_res(header: &SbrHeader, grid: &crate::sbr_grid::SbrGrid) -> bool {
    if grid.amp_res_override {
        false
    } else {
        header.amp_res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sbr_element::{SbrChannel, SbrElement};
    use crate::sbr_envelope::{SbrEnvelopeData, SbrNoiseData};
    use crate::sbr_grid::{FrameClass, SbrDtdf, SbrGrid, SbrInvf};

    fn sine(freq: f64, n: usize, offset: usize) -> Vec<f64> {
        (0..n)
            .map(|t| (2.0 * core::f64::consts::PI * freq * (t + offset) as f64).sin())
            .collect()
    }

    /// Pure upsampling reproduces a 2×-upsampled, delayed sine across
    /// frame boundaries.
    #[test]
    fn upsample_frames_are_continuous() {
        let mut dec = SbrDecoder::new(44_100, 1).unwrap();
        let freq = 0.02;
        let mut out = Vec::new();
        for f in 0..4 {
            let core = sine(freq, 1024, f * 1024);
            let o = dec.upsample_frame(&[&core]).unwrap();
            assert_eq!(o[0].len(), 2048);
            out.extend_from_slice(&o[0]);
        }
        // Steady-state fit against the ideal upsampled sine.
        let ideal = |t: f64, d: f64| (2.0 * core::f64::consts::PI * freq * (t - d) / 2.0).sin();
        let mut best = f64::INFINITY;
        for delay in 0..1500usize {
            let mut err = 0.0;
            let mut sig = 0.0;
            for (t, &o) in out.iter().enumerate().skip(2500) {
                let e = o - ideal(t as f64, delay as f64);
                err += e * e;
                sig += o * o;
            }
            best = best.min(err / sig.max(1e-30));
        }
        assert!(best < 1e-4, "upsample error ratio {best}");
    }

    /// Build a minimal single-channel SBR extension: one FIXFIX
    /// envelope, frequency-direction start values, flat noise floor.
    fn synthetic_ext(fs_sbr: u32, env_start: i32, noise_q: i32) -> SbrExtensionData {
        let header = SbrHeader {
            amp_res: true,
            start_freq: 5,
            stop_freq: 3,
            xover_band: 0,
            reserved: 0,
            header_extra_1: false,
            header_extra_2: false,
            freq_scale: 2,
            alter_scale: true,
            noise_bands: 2,
            limiter_bands: 2,
            limiter_gains: 2,
            interpol_freq: true,
            smoothing_mode: true,
        };
        let bands = header.derive_bands(fs_sbr).unwrap();
        let n_high = bands.n_high();
        let n_q = bands.n_q();
        let grid = SbrGrid {
            frame_class: FrameClass::FixFix,
            num_env: 1,
            num_noise: 1,
            freq_res: vec![true],
            var_bord_0: 0,
            var_bord_1: 0,
            rel_bord_0: vec![],
            rel_bord_1: vec![],
            pointer: 0,
            amp_res_override: true,
        };
        let dtdf = SbrDtdf {
            df_env: vec![false],
            df_noise: vec![false],
        };
        let invf = SbrInvf {
            invf_mode: vec![0; n_q],
        };
        let mut env_row = vec![0i32; n_high];
        env_row[0] = env_start;
        let envelope = SbrEnvelopeData {
            data: vec![env_row],
        };
        let noise = SbrNoiseData {
            data: vec![{
                let mut r = vec![0i32; n_q];
                r[0] = noise_q;
                r
            }],
        };
        SbrExtensionData {
            crc: None,
            crc_region: None,
            header_present: true,
            header,
            element: SbrElement {
                coupling: false,
                channels: vec![SbrChannel {
                    grid,
                    dtdf,
                    invf,
                    envelope,
                    noise,
                    add_harmonic: vec![],
                }],
                extension: None,
            },
            num_sbr_bits: 0,
        }
    }

    /// A full synthetic SBR frame produces finite 2048-sample output
    /// with energy in the SBR band, and threads state across frames
    /// (header reuse, no reset).
    #[test]
    fn synthetic_sbr_frame_produces_high_band() {
        let fs_sbr = 44_100;
        let ext = synthetic_ext(fs_sbr, 10, 6);
        let mut dec = SbrDecoder::new(fs_sbr, 1).unwrap();
        // A mid-band core tone so the patch sources carry signal.
        let freq = 0.11;
        let mut all = Vec::new();
        for f in 0..3 {
            let core = sine(freq, 1024, f * 1024);
            let out = dec.process_frame(&ext, &[&core]).unwrap();
            assert_eq!(out.len(), 1);
            assert_eq!(out[0].len(), 2048);
            assert!(out[0].iter().all(|v| v.is_finite()));
            all.extend_from_slice(&out[0]);
        }
        // The output must carry energy (base band at least).
        let energy: f64 = all.iter().map(|v| v * v).sum();
        assert!(energy > 1.0, "energy {energy}");
        // Deterministic: a second decoder over the same input matches
        // bit-exactly.
        let mut dec2 = SbrDecoder::new(fs_sbr, 1).unwrap();
        let mut all2 = Vec::new();
        for f in 0..3 {
            let core = sine(freq, 1024, f * 1024);
            all2.extend_from_slice(&dec2.process_frame(&ext, &[&core]).unwrap()[0]);
        }
        assert_eq!(all, all2);
    }

    /// The high band actually receives patched content: with a strong
    /// envelope target the spectrum above kx·(fs/128) is non-silent,
    /// and it scales with the envelope scalefactor.
    #[test]
    fn envelope_scalefactor_controls_high_band_level() {
        let fs_sbr = 44_100;
        let mut quiet = SbrDecoder::new(fs_sbr, 1).unwrap();
        let mut loud = SbrDecoder::new(fs_sbr, 1).unwrap();
        let ext_quiet = synthetic_ext(fs_sbr, 2, 10);
        let ext_loud = synthetic_ext(fs_sbr, 12, 10);
        let freq = 0.09;
        let mut hi_q = 0.0f64;
        let mut hi_l = 0.0f64;
        for f in 0..3 {
            let core = sine(freq, 1024, f * 1024);
            let oq = quiet.process_frame(&ext_quiet, &[&core]).unwrap();
            let ol = loud.process_frame(&ext_loud, &[&core]).unwrap();
            if f > 0 {
                // High-pass both outputs with a crude difference filter
                // to weight the HF region, then compare energies.
                for w in oq[0].windows(2) {
                    hi_q += (w[1] - w[0]) * (w[1] - w[0]);
                }
                for w in ol[0].windows(2) {
                    hi_l += (w[1] - w[0]) * (w[1] - w[0]);
                }
            }
        }
        assert!(hi_l > hi_q * 4.0, "loud {hi_l} vs quiet {hi_q}");
    }

    /// Downsampled pure upsampling is the identity at the core rate
    /// (up to the analysis+synthesis delay), and each frame yields
    /// 1024 samples.
    #[test]
    fn downsampled_upsample_is_identity_at_core_rate() {
        let mut dec = SbrDecoder::new(44_100, 1).unwrap();
        dec.set_downsampled(true).unwrap();
        assert!(dec.is_downsampled());
        let freq = 0.02;
        let mut input_all = Vec::new();
        let mut out = Vec::new();
        for f in 0..4 {
            let core = sine(freq, 1024, f * 1024);
            input_all.extend_from_slice(&core);
            let o = dec.upsample_frame(&[&core]).unwrap();
            assert_eq!(o[0].len(), 1024);
            out.extend_from_slice(&o[0]);
        }
        // Mode switches after the first frame are rejected.
        assert!(dec.set_downsampled(false).is_err());
        let mut best = (f64::INFINITY, 0usize);
        for delay in 0..1024usize {
            let mut err = 0.0;
            let mut sig = 0.0;
            for t in 1500..out.len() {
                if t < delay {
                    continue;
                }
                let e = out[t] - input_all[t - delay];
                err += e * e;
                sig += out[t] * out[t];
            }
            let ratio = err / sig.max(1e-30);
            if ratio < best.0 {
                best = (ratio, delay);
            }
        }
        assert!(
            best.0 < 1e-4,
            "identity error ratio {} at {}",
            best.0,
            best.1
        );
    }

    /// With the whole SBR range inside the first 32 QMF bands, the
    /// dual-rate output is band-limited below the core Nyquist, so the
    /// downsampled decode must match a straight 2:1 decimation of the
    /// dual-rate decode (same synthetic SBR frames, delay-searched).
    #[test]
    fn downsampled_matches_decimated_dual_rate() {
        // The synthetic header at 44.1 kHz derives kx 14, M 15 —
        // kx + M = 29 ≤ 32, so no SBR content crosses the core Nyquist.
        let fs_sbr = 44_100;
        let ext = synthetic_ext(fs_sbr, 8, 6);
        let bands = ext.header.derive_bands(fs_sbr).unwrap();
        assert!(
            bands.k_x + bands.m <= 32,
            "test premise: SBR range within 32 bands (kx {} M {})",
            bands.k_x,
            bands.m
        );
        let mut dual = SbrDecoder::new(fs_sbr, 1).unwrap();
        let mut down = SbrDecoder::new(fs_sbr, 1).unwrap();
        down.set_downsampled(true).unwrap();
        let freq = 0.055;
        let mut out_dual = Vec::new();
        let mut out_down = Vec::new();
        for f in 0..6 {
            let core = sine(freq, 1024, f * 1024);
            out_dual.extend_from_slice(&dual.process_frame(&ext, &[&core]).unwrap()[0]);
            let o = down.process_frame(&ext, &[&core]).unwrap();
            assert_eq!(o[0].len(), 1024);
            out_down.extend_from_slice(&o[0]);
        }
        // out_down[n] ≈ out_dual[2n − d] for some fixed integer d
        // (either parity): search d, then gate the steady-state error.
        let mut best = (f64::INFINITY, 0usize);
        for d in 0..1400usize {
            let mut err = 0.0;
            let mut sig = 0.0;
            for (n, &od) in out_down.iter().enumerate().skip(1200) {
                let idx = 2 * n;
                if idx < d || idx - d >= out_dual.len() {
                    continue;
                }
                let e = od - out_dual[idx - d];
                err += e * e;
                sig += od * od;
            }
            let ratio = err / sig.max(1e-30);
            if ratio < best.0 {
                best = (ratio, d);
            }
        }
        assert!(
            best.0 < 1e-3,
            "decimation mismatch ratio {} at delay {}",
            best.0,
            best.1
        );
    }

    /// The §4.6.18.8 low-power mode reconstructs the same synthetic
    /// SBR frame as the high-quality mode to a moderate tolerance
    /// (the LP tool is a real-valued approximation), stays finite and
    /// deterministic, and composes with the downsampled output.
    #[test]
    fn low_power_tracks_high_quality() {
        let fs_sbr = 44_100;
        let ext = synthetic_ext(fs_sbr, 8, 6);
        let mut hq = SbrDecoder::new(fs_sbr, 1).unwrap();
        let mut lp = SbrDecoder::new(fs_sbr, 1).unwrap();
        lp.set_low_power(true).unwrap();
        assert!(lp.is_low_power());
        let freq = 0.055;
        let mut out_hq = Vec::new();
        let mut out_lp = Vec::new();
        for f in 0..6 {
            let core = sine(freq, 1024, f * 1024);
            out_hq.extend_from_slice(&hq.process_frame(&ext, &[&core]).unwrap()[0]);
            let o = lp.process_frame(&ext, &[&core]).unwrap();
            assert_eq!(o[0].len(), 2048);
            assert!(o[0].iter().all(|v| v.is_finite()));
            out_lp.extend_from_slice(&o[0]);
        }
        assert!(lp.set_low_power(false).is_err(), "mode locked after start");
        // Energy tracks the HQ reconstruction (the two banks share the
        // prototype and delay).
        let e_hq: f64 = out_hq.iter().skip(4096).map(|v| v * v).sum();
        let e_lp: f64 = out_lp.iter().skip(4096).map(|v| v * v).sum();
        assert!(
            e_lp > 0.5 * e_hq && e_lp < 2.0 * e_hq,
            "LP {e_lp} vs HQ {e_hq}"
        );
        // The real-valued HF processing does not reproduce the complex
        // path's subband phases, so the comparison is energy-domain:
        // the core tone's amplitude (quadrature probe at the upsampled
        // frequency) must match tightly, and the per-block energy
        // envelope must track.
        let probe = |x: &[f64]| -> f64 {
            let w = 2.0 * core::f64::consts::PI * freq / 2.0;
            let (mut cs, mut sn) = (0.0f64, 0.0f64);
            let n0 = 4096;
            for (t, &v) in x.iter().enumerate().skip(n0) {
                cs += v * (w * t as f64).cos();
                sn += v * (w * t as f64).sin();
            }
            let n = (x.len() - n0) as f64;
            2.0 / n * (cs * cs + sn * sn).sqrt()
        };
        let (a_hq, a_lp) = (probe(&out_hq), probe(&out_lp));
        assert!(
            (a_lp - a_hq).abs() < 0.05 * a_hq,
            "core tone amplitude LP {a_lp} vs HQ {a_hq}"
        );
        for (block_hq, block_lp) in out_hq
            .chunks_exact(1024)
            .zip(out_lp.chunks_exact(1024))
            .skip(4)
        {
            let e_h: f64 = block_hq.iter().map(|v| v * v).sum();
            let e_l: f64 = block_lp.iter().map(|v| v * v).sum();
            assert!(
                e_l > 0.4 * e_h && e_l < 2.5 * e_h,
                "block energy LP {e_l} vs HQ {e_h}"
            );
        }

        // Determinism.
        let mut lp2 = SbrDecoder::new(fs_sbr, 1).unwrap();
        lp2.set_low_power(true).unwrap();
        let mut out_lp2 = Vec::new();
        for f in 0..6 {
            let core = sine(freq, 1024, f * 1024);
            out_lp2.extend_from_slice(&lp2.process_frame(&ext, &[&core]).unwrap()[0]);
        }
        assert_eq!(out_lp, out_lp2);

        // LP + downsampled: 1024 samples per frame, finite.
        let mut lpd = SbrDecoder::new(fs_sbr, 1).unwrap();
        lpd.set_low_power(true).unwrap();
        lpd.set_downsampled(true).unwrap();
        let core = sine(freq, 1024, 0);
        let o = lpd.process_frame(&ext, &[&core]).unwrap();
        assert_eq!(o[0].len(), 1024);
        assert!(o[0].iter().all(|v| v.is_finite()));
    }

    /// Low-power pure upsampling is still the identity at 2× rate
    /// (real analysis + real synthesis pair).
    #[test]
    fn low_power_upsample_is_identity() {
        let mut dec = SbrDecoder::new(44_100, 1).unwrap();
        dec.set_low_power(true).unwrap();
        let freq = 0.02;
        let mut out = Vec::new();
        for f in 0..4 {
            let core = sine(freq, 1024, f * 1024);
            let o = dec.upsample_frame(&[&core]).unwrap();
            assert_eq!(o[0].len(), 2048);
            out.extend_from_slice(&o[0]);
        }
        let ideal = |t: f64, d: f64| (2.0 * core::f64::consts::PI * freq * (t - d) / 2.0).sin();
        let mut best = f64::INFINITY;
        for delay in 0..1500usize {
            let mut err = 0.0;
            let mut sig = 0.0;
            for (t, &o) in out.iter().enumerate().skip(2500) {
                let e = o - ideal(t as f64, delay as f64);
                err += e * e;
                sig += o * o;
            }
            best = best.min(err / sig.max(1e-30));
        }
        assert!(best < 1e-4, "LP upsample error ratio {best}");
    }

    /// A PS payload on a low-power decoder is rejected — the
    /// subpart-8 tool needs the complex QMF domain.
    #[test]
    fn low_power_rejects_ps() {
        use crate::sbr_element::SbrExtension;
        let fs_sbr = 44_100;
        let mut ext = synthetic_ext(fs_sbr, 8, 6);
        ext.element.extension = Some(SbrExtension {
            id: EXTENSION_ID_PS,
            data: vec![0u8; 4],
        });
        let mut lp = SbrDecoder::new(fs_sbr, 1).unwrap();
        lp.set_low_power(true).unwrap();
        let core = sine(0.05, 1024, 0);
        assert!(matches!(
            lp.process_frame(&ext, &[&core]),
            Err(Error::SbrLowPowerPs)
        ));
    }

    /// A channel-count / buffer-length mismatch is rejected.
    #[test]
    fn shape_mismatches_rejected() {
        let mut dec = SbrDecoder::new(44_100, 1).unwrap();
        let core = vec![0.0; 512];
        assert!(dec.upsample_frame(&[&core]).is_err());
        let ext = synthetic_ext(44_100, 0, 6);
        let short = vec![0.0; 1024];
        assert!(dec.process_frame(&ext, &[&short[..], &short[..]]).is_err());
    }
}

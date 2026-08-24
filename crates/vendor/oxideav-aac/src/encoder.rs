//! End-to-end AAC-LC encoder — ISO/IEC 14496-3 §4.5/§4.6 written
//! forward.
//!
//! This module drives the crate's Phase-2 bit-exact wire writers
//! ([`crate::ics_body::IcsBody::write`],
//! [`crate::section_data::SectionData::write`],
//! [`crate::scale_factor_data::ScaleFactorData::write`],
//! [`crate::spectral_data::SpectralData::write`],
//! [`crate::raw_data_block::FrameAssembler`],
//! [`crate::adts::AdtsHeader::write`]) from PCM input, producing an
//! ADTS stream the crate's own [`crate::decode::StreamDecoder`]
//! round-trips.
//!
//! ## What the wire format fixes vs. what the encoder chooses
//!
//! ISO/IEC 14496-3 normatively defines the *decoder*: the §4.6.2
//! inverse quantizer `x = sign(q)·|q|^(4/3)·2^(0.25·(sf−100))`, the
//! §4.6.3 noiseless coding, and the §4.6.11 filterbank. Everything on
//! the analysis side — the psychoacoustic model, the
//! scalefactor/quantizer search, the codebook choice — is an encoder
//! degree of freedom; any choice that yields conforming syntax is a
//! conforming encoder. The choices here are deliberately simple and
//! fully derived from the normative decoder equations:
//!
//! * **Window decision (block switching, §4.6.11.3.2)** — an
//!   energy-jump transient detector on each incoming hop drives the
//!   `ONLY_LONG → LONG_START → EIGHT_SHORT → LONG_STOP` state
//!   machine: a 128-sample subblock whose energy jumps ≥12× over the
//!   running average of its predecessors (above an absolute floor)
//!   marks the hop transient; the frame *before* the transient hop
//!   becomes `LONG_START`, the transient hop's frame `EIGHT_SHORT`
//!   (extended while transients continue), and the run exits through
//!   `LONG_STOP`. All windows are the §4.6.11.3.2 sine shape. Within
//!   an `EIGHT_SHORT` frame the §4.5.2.3.4 `scale_factor_grouping`
//!   decision ([`decide_short_grouping`]) merges envelope-alike
//!   adjacent windows into shared window groups (one scalefactor /
//!   section track per group, §4.5.2.3.5 interleaved transmission
//!   order) — the attack window's energy jump keeps it in its own
//!   group.
//! * **Analysis filterbank** — the §4.6.11.3.1 forward MDCT (the
//!   transform whose windowed overlap-add against the decoder's IMDCT
//!   is unity — the same [`crate::filterbank::forward_mdct`] the
//!   §4.6.7 LTP loop uses). Long frames run one 2048-point transform
//!   under the sequence's composite window; `EIGHT_SHORT` frames run
//!   eight 256-point transforms at offsets `448 + j·128` within the
//!   window region. Frame `f` covers input samples
//!   `[f·1024 − 1024, f·1024 + 1024)`; the leading frame is primed
//!   with zeros, giving the standard 1024-sample encoder delay.
//! * **Quantizer** — the exact inverse of §4.6.2:
//!   `q = round((|x| / 2^(0.25·(sf−100)))^(3/4))`, with
//!   round-half-away-from-zero (the §1.3 `NINT` convention).
//! * **Psychoacoustics-lite** — a masking-spread rule: each
//!   scalefactor band's `sf` is chosen so the band's *peak*
//!   coefficient quantizes to a target magnitude
//!   `M_b = M · (peak_b / peak_frame)^½` (`sf = 100 + 4·log2(peak_b)
//!   − (16/3)·log2(M_b)`, the inversion of the dequant gain ladder).
//!   The square-root spread interpolates between constant-SNR
//!   (every band equally precise relative to itself — wasteful on
//!   the leakage skirts of tonal signals) and a flat noise floor
//!   (all precision on the loudest band): a band 40 dB below the
//!   frame peak is quantized ~20 dB more coarsely, and a band whose
//!   target falls below one quantizer step is culled to `ZERO_HCB`
//!   outright — a first-order simultaneous-masking model.
//! * **Rate loop** — an outer loop adds a uniform offset to every
//!   band's scalefactor (coarsening all quantizers by 1.5 dB per
//!   step, the §4.6.2.3.3 quarter-step ladder ×2) until the assembled
//!   frame fits the per-frame byte budget derived from the requested
//!   bitrate.
//! * **Codebook / section choice** — measured bit cost: a dynamic
//!   program over section boundaries picks, for every candidate run
//!   of same-class bands, the single Table 4.95 book (1..=11) whose
//!   *actual* coded size — Huffman codewords + sign bits + escapes,
//!   measured with the real tuple writer — plus the `section_data()`
//!   header overhead is minimal (see [`optimize_group_sections`]).
//!   This subsumes the classic smallest-LAV-fit + merge-equal-books
//!   rule and additionally exploits the signed/unsigned sibling
//!   books and header-saving LAV upgrades.
//! * **Pulse escape (§4.4.6.3, measured)** — a long-frame band whose
//!   few outlier lines force it onto a large-LAV book or into
//!   §4.6.3.3 escape sequences is also priced with a Table 4.7
//!   `pulse_data()` variant: up to four outliers are reduced toward
//!   zero to the magnitude floor of the rest of the band (4-bit
//!   `amp` reach) and the `(offset, amp)` chain rides the pulse
//!   record ([`extract_pulse_candidate`] picks the best-saving band
//!   by per-band measured cost). The decoder's §4.6.3.3 fix-up
//!   restores the *identical* quantized spectrum before
//!   dequantization, so the choice is purely a noiseless-coding one
//!   and is settled end-to-end by [`channel_wire_bits`] — the
//!   variant is kept only when the whole channel stream measures
//!   smaller.
//! * **Stereo** — a CPE with `common_window == 1` (one shared
//!   `ics_info()`) and per-band §4.6.8.1 M/S coding: for each
//!   scalefactor band the encoder forms `m = (l+r)/2`,
//!   `s = (l−r)/2` (the exact forward matrix of the normative
//!   `l = m+s` / `r = m−s` de-matrix) and selects M/S when it moves
//!   the band's energy into one dominant channel — i.e. when
//!   `min(e_m, e_s) ≤ (e_l + e_r) / 8` (the transformed pair is at
//!   least ~9 dB lopsided, so the quiet one culls or codes cheaply).
//!   The mask is emitted as `ms_mask_present = 2` when every band
//!   flags (identical / phase-inverted channels), `1` + explicit
//!   mask when mixed, `0` when no band benefits. `EIGHT_SHORT`
//!   frames decide per `(window group, sfb)` under the pair's joint
//!   grouping — the Table 4.5 `ms_used[g][sfb]` granularity
//!   ([`ms_decide_short`]).
//! * **Intensity stereo (§4.6.8.2, opt-in)** — with
//!   [`StreamEncoder::set_intensity_stereo`], a high-frequency
//!   long-frame CPE band whose channels correlate above
//!   [`IS_CORR_MIN`] is transmitted once: the right channel's band
//!   becomes the intensity pseudo codebook (15 in-phase / 14
//!   out-of-phase) carrying only `is_pos = 2·log2(e_l/e_r)` on the
//!   §4.6.8.1.4 DPCM track; the decoder derives
//!   `r = ±0.5^(0.25·is_pos)·l` (§4.6.8.2.3). IS bands are excluded
//!   from the M/S mask (per-band mutual exclusion; a set `ms_used`
//!   bit would signal phase reversal instead).
//! * **TNS (§4.6.9, default on)** — per analysis window, the
//!   [`crate::encoder_tns`] pass measures the prediction gain of an
//!   LPC over the coverable spectral region (Levinson-Durbin on the
//!   coefficient autocorrelation); a window whose gain clears the
//!   threshold transmits one upward Table 4.54 filter (PARCOR
//!   quantised on the §4.6.9.3 4-bit arcsine grid) and the spectrum
//!   is passed through the §4.6.7.4.1 all-zero analysis filter
//!   derived from the *wire* coefficients — the exact inverse of the
//!   decoder's §4.6.9.3 all-pole synthesis, run per channel in the
//!   L/R domain before the M/S forward matrix (mirroring the
//!   decoder's M/S-then-TNS order). See [`StreamEncoder::set_tns`].
//! * **PNS (§4.6.13, opt-in)** — with [`StreamEncoder::set_pns`], a
//!   long-frame band whose energy is spread across most of its
//!   coefficients (density `(Σ|x|)²/(width·Σx²)` above 0.4 — dense
//!   noise measures `≈2/π`, `k` spectral lines `≈k/width`) is
//!   transmitted as a `NOISE_HCB` band carrying only its energy
//!   (`noise_nrg = round(4·log2‖band‖₂)`, the `2^(0.25·nrg)` ladder)
//!   on the §4.6.13 DPCM track; the decoder re-synthesises the band
//!   from its own generator at exactly that L2 norm. In a CPE the
//!   decision runs per channel *before* the M/S matrix (mutual
//!   exclusion, §4.6.13.5); a both-channels-noise band correlating
//!   above [`PNS_CORR_MIN`] sets its `ms_used` bit — the §4.6.13.3
//!   correlated-noise signal (same random vector both channels), not
//!   an M/S flag. Off by default:
//!   a single-frame statistic cannot tell true noise from
//!   noise-shaped deterministic content (sweeps, dense leakage
//!   floors), which substitutes with the right energy but the wrong
//!   waveform — the default-on decision awaits a cross-frame
//!   tonality measure.
//!
//! ## Conformance envelope
//!
//! The assembled frame respects the wire-format hard limits:
//! scalefactors clamp to `0..=255` (8-bit `global_gain` seed) with
//! consecutive DPCM deltas in `−60..=+60` (Table 4.A.1's codeword
//! range), quantized magnitudes cap at
//! [`crate::spectral_codebook::MAX_QUANT`] (8191, the §4.6.3.3 ESC
//! ceiling), and `aac_frame_length` stays within its 13-bit field.

use crate::adts::{AdtsHeader, ADTS_HEADER_BYTES_NO_CRC, ADTS_SAMPLE_RATES_HZ};
use crate::encoder_tns::detect_and_apply_tns;
use crate::filterbank::{
    forward_mdct, long_sequence_window, short_window_j, SHORT_SEQ_HOP, SHORT_SEQ_START,
};
use crate::ics_body::IcsBody;
use crate::ics_info::{
    IcsInfo, WindowSequence, WindowShape, NUM_SWB_LONG_WINDOW, NUM_SWB_SHORT_WINDOW,
};
use crate::pulse_data::{Pulse, PulseData, MAX_PULSES};
use crate::raw_data_block::{FrameAssembler, IdSynEle};
use crate::scale_factor_data::{
    differentiate, AbsoluteScaleFactorEntry, AbsoluteScaleFactors, NOISE_OFFSET,
};
use crate::section_data::{
    Section, SectionData, INTENSITY_HCB, INTENSITY_HCB2, NOISE_HCB, ZERO_HCB,
};
use crate::spectral_codebook::MAX_QUANT;
use crate::spectral_data::SpectralData;
use crate::swb_offset::{
    long_window_offsets, short_window_offsets, LONG_WINDOW_LEN, SHORT_WINDOW_LEN,
};
use crate::tns_data::TnsData;
use crate::{Error, Result};

use oxideav_core::bits::BitWriter;

/// Historical direct-factory endpoint (the crate convention's
/// `<crate>::encoder::make_encoder` path) — re-exported from
/// [`crate::codec_encoder`].
pub use crate::codec_encoder::make_encoder;

/// Samples per channel per AAC frame (the 1024-line transform
/// family this crate implements).
pub const FRAME_LEN: usize = LONG_WINDOW_LEN as usize;

/// The long transform length `N = 2048`.
const LONG_TRANSFORM_LEN: usize = 2 * FRAME_LEN;

/// §4.6.2.3.3 `SF_OFFSET` — the scalefactor of unit gain.
const SF_OFFSET: i32 = 100;

/// Table 4.53 DPCM delta bound (the Table 4.A.1 codeword range).
const MAX_SF_DELTA: i32 = 60;

/// Target magnitude the *loudest* band's peak coefficient quantizes
/// to before the rate loop engages; quieter bands scale down with
/// the square-root masking spread.
const TARGET_PEAK_MAG: f64 = 42.0;

/// Masking-spread exponent: a band's target magnitude is
/// `TARGET_PEAK_MAG · (peak_b / peak_frame)^SPREAD`. `0` would be
/// constant-SNR, `1` a flat noise floor; `½` splits the difference.
const SPREAD: f64 = 0.5;

/// Cull threshold: a band whose spread target magnitude falls below
/// this fraction of one quantizer step carries no audible content
/// relative to the frame and is sent as `ZERO_HCB`.
const MIN_TARGET_MAG: f64 = 0.7;

/// Upper bound on rate-loop iterations. Each iteration coarsens
/// every quantizer by 3 dB (sf offset +4), so 48 iterations span
/// ~144 dB — beyond that the frame is all-zero anyway.
const MAX_RATE_ITERATIONS: usize = 48;

/// Deepest refinement the rate loop applies when a frame comes in
/// under budget: −32 scalefactors ≈ 24 dB of extra precision
/// (magnitudes ×2^6 over the [`TARGET_PEAK_MAG`] baseline).
const MAX_REFINE_OFFSET: i32 = 32;

/// Minimum §4.5.4 band width (coefficients) for the §4.6.13 PNS
/// noise-likeness statistic to be meaningful; narrower bands always
/// code spectrally.
const PNS_MIN_WIDTH: usize = 8;

/// PNS density floor on the `(Σ|x|)² / (width·Σx²)` statistic: dense
/// Gaussian-like noise measures `≈ 2/π ≈ 0.64` (`(E|x|)²/E[x²]`),
/// while `k` dominant spectral lines measure `≈ k/width` — a band
/// only counts as noise when its energy is spread across most of its
/// coefficients, so leakage skirts and harmonic combs keep spectral
/// coding.
const PNS_DENSITY_MIN: f64 = 0.4;

/// Configuration for [`StreamEncoder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncoderConfig {
    /// Output sample rate in Hz. Must be one of the ISO/IEC 14496-3
    /// Table 1.18 rates expressible in an ADTS header (index 0..=12).
    pub sample_rate: u32,
    /// Channel count. Every count with a Table 1.19 default
    /// `channelConfiguration` is accepted: `1` (SCE) / `2` (CPE) /
    /// `3` (SCE + CPE) / `4` (SCE + CPE + SCE) / `5` (SCE + 2 CPE) /
    /// `6` (5.1: SCE + 2 CPE + LFE) / `8` (7.1: SCE + 3 CPE + LFE).
    /// `7` has no default configuration (a PCE-defined layout, out
    /// of scope) and is rejected. Input PCM is interleaved in the
    /// canonical [`crate::channel_map`] order the decoder emits
    /// (5.1 = `L R C LFE Ls Rs`), and the encoder derives the
    /// bitstream element order from the Table 1.19 layout.
    pub channels: u8,
    /// Target bitrate in bits/second. Drives the per-frame byte
    /// budget of the rate loop. The output is not strictly CBR — each
    /// frame independently fits its budget — but averages at or below
    /// this rate.
    pub bitrate: u32,
}

impl EncoderConfig {
    /// Resolve `sample_rate` to its Table 1.18
    /// `sampling_frequency_index`. Index 12 (7350 Hz) is excluded:
    /// the §4.5.4 scalefactor-band tables only cover indices
    /// 0..=11 (7350 Hz content conventionally ships under index 11,
    /// see the staged corpus notes).
    fn fs_index(&self) -> Result<u8> {
        ADTS_SAMPLE_RATES_HZ
            .iter()
            .position(|&r| r == self.sample_rate)
            .filter(|&i| i < NUM_SWB_LONG_WINDOW.len())
            .map(|i| i as u8)
            .ok_or(Error::EncoderInvalidConfig)
    }

    /// Per-frame payload byte budget from the bitrate:
    /// `bitrate · 1024 / sample_rate` bits, minus the 7-byte ADTS
    /// header, floored at a minimum that always allows a syntactically
    /// valid (silent) frame.
    fn frame_budget_bytes(&self) -> usize {
        let bits = (self.bitrate as u64 * FRAME_LEN as u64) / self.sample_rate.max(1) as u64;
        let bytes = (bits / 8) as usize;
        bytes.saturating_sub(ADTS_HEADER_BYTES_NO_CRC).max(16)
    }

    /// The Table 1.19 default `channelConfiguration` for this channel
    /// count (see [`EncoderConfig::channels`]).
    fn channel_configuration(&self) -> Result<u8> {
        match self.channels {
            n @ 1..=6 => Ok(n),
            8 => Ok(7),
            _ => Err(Error::EncoderInvalidConfig),
        }
    }
}

/// One channel element of the §4.4.2.1 `raw_data_block()` plan, with
/// element-order channel indices.
#[derive(Debug, Clone, Copy)]
enum ElementPlan {
    Sce(usize),
    Cpe(usize, usize),
    Lfe(usize),
}

/// The Table 1.19 element sequence for a default
/// `channelConfiguration`, indexing element-order channels.
fn element_plan(channel_configuration: u8) -> Result<Vec<ElementPlan>> {
    use ElementPlan::*;
    Ok(match channel_configuration {
        1 => vec![Sce(0)],
        2 => vec![Cpe(0, 1)],
        3 => vec![Sce(0), Cpe(1, 2)],
        4 => vec![Sce(0), Cpe(1, 2), Sce(3)],
        5 => vec![Sce(0), Cpe(1, 2), Cpe(3, 4)],
        6 => vec![Sce(0), Cpe(1, 2), Cpe(3, 4), Lfe(5)],
        7 => vec![Sce(0), Cpe(1, 2), Cpe(3, 4), Cpe(5, 6), Lfe(7)],
        _ => return Err(Error::EncoderInvalidConfig),
    })
}

/// §4.5.2.1.3: only the lowest 12 spectral coefficients of an LFE
/// element may be non-zero.
const LFE_MAX_LINES: usize = 12;

/// A streaming AAC-LC encoder producing one ADTS frame per
/// 1024-sample input hop.
///
/// Feed interleaved `i16` PCM via [`StreamEncoder::encode_all`] (one
/// shot), or drive [`StreamEncoder::encode_frame`] hop by hop and
/// finish with [`StreamEncoder::finish`] to flush the analysis
/// overlap. The encoder delay is exactly [`FRAME_LEN`] samples: the
/// first decoded frame of the round-trip is silence, and decoded
/// frame `f ≥ 1` reconstructs input hop `f − 1`.
#[derive(Debug, Clone)]
pub struct StreamEncoder {
    config: EncoderConfig,
    fs_index: u8,
    /// The Table 1.19 `channelConfiguration` derived from the channel
    /// count (lands in the ADTS header and fixes the element plan).
    channel_configuration: u8,
    /// Canonical-input channel index feeding each *element-order*
    /// channel slot — the inverse of the decoder's
    /// [`crate::channel_map::reorder_permutation`], so encode ∘
    /// decode is channel-identity.
    element_src: Vec<usize>,
    /// Element-order slots that belong to an LFE element (§4.5.2.1.3
    /// restrictions apply: long-only analysis, no TNS, ≤ 12 lines).
    lfe_slot: Vec<bool>,
    /// Per-channel previous input hop (the left half of the next
    /// analysis window), [`FRAME_LEN`] samples each, in *element
    /// order*. Starts all-zero (the priming frame).
    history: Vec<Vec<f64>>,
    /// `window_sequence` of the previously emitted frame — drives
    /// the §4.6.11.3.2 block-switching state machine.
    prev_seq: WindowSequence,
    /// The previous frame flagged a transient in what is now the
    /// history hop, so this frame *must* be `EIGHT_SHORT_SEQUENCE`
    /// (the `LONG_START → EIGHT_SHORT` contract).
    short_pending: bool,
    /// §4.6.13 PNS emission toggle — see [`StreamEncoder::set_pns`].
    pns_enabled: bool,
    /// §4.6.9 TNS emission toggle — see [`StreamEncoder::set_tns`].
    tns_enabled: bool,
    /// §4.6.8.2 intensity-stereo emission toggle — see
    /// [`StreamEncoder::set_intensity_stereo`].
    is_enabled: bool,
}

impl StreamEncoder {
    /// Build an encoder for `config`.
    ///
    /// Errors with [`Error::EncoderInvalidConfig`] when the sample
    /// rate is not a Table 1.18 ADTS rate, the channel count has no
    /// Table 1.19 default configuration (see
    /// [`EncoderConfig::channels`]), or the bitrate is 0.
    pub fn new(config: EncoderConfig) -> Result<Self> {
        let fs_index = config.fs_index()?;
        let channel_configuration = config.channel_configuration()?;
        if config.bitrate == 0 {
            return Err(Error::EncoderInvalidConfig);
        }
        let ch = config.channels as usize;
        // canonical[i] = element[perm[i]] on the decode side, so the
        // element slot j sources canonical input channel i with
        // perm[i] == j.
        let perm = crate::channel_map::reorder_permutation(channel_configuration)
            .ok_or(Error::EncoderInvalidConfig)?;
        let mut element_src = vec![0usize; ch];
        for (i, &j) in perm.iter().enumerate() {
            element_src[j] = i;
        }
        let mut lfe_slot = vec![false; ch];
        for elem in element_plan(channel_configuration)? {
            if let ElementPlan::Lfe(slot) = elem {
                lfe_slot[slot] = true;
            }
        }
        Ok(Self {
            config,
            fs_index,
            channel_configuration,
            element_src,
            lfe_slot,
            history: vec![vec![0.0; FRAME_LEN]; ch],
            prev_seq: WindowSequence::OnlyLong,
            short_pending: false,
            pns_enabled: false,
            tns_enabled: true,
            is_enabled: false,
        })
    }

    /// Enable / disable §4.6.13 PNS emission (default **off**).
    ///
    /// When enabled, long frames transmit dense noise-like bands
    /// (see the module docs) as `NOISE_HCB` energies instead of
    /// spectra — a large bitrate win on noise content, validated
    /// energy-exact through the decoder's §4.6.13.3 synthesis. In a
    /// CPE the decision runs per channel on the pre-M/S spectra
    /// (PNS and M/S are mutually exclusive per band, §4.6.13.5);
    /// a band both channels noise-code whose content correlates
    /// above [`PNS_CORR_MIN`] additionally sets its `ms_used` bit,
    /// signalling the decoder to synthesise the *same* random
    /// vector into both channels (§4.6.13.3 correlated noise). It
    /// stays opt-in because a *single-frame* spectral statistic
    /// cannot distinguish true noise from noise-shaped deterministic
    /// content (a frequency sweep, a dense leakage floor): those
    /// substitute with the right energy but the wrong waveform.
    /// Turning the default on awaits a cross-frame tonality /
    /// predictability measure.
    pub fn set_pns(&mut self, enabled: bool) {
        self.pns_enabled = enabled;
    }

    /// Enable / disable §4.6.9 TNS emission (default **on**).
    ///
    /// When enabled, each analysis window whose spectrum shows a
    /// prediction gain above the [`crate::encoder_tns::TNS_GAIN_MIN`]
    /// threshold (a strongly non-flat temporal envelope) transmits a
    /// Table 4.54 noise-shaping filter, and the spectrum is passed
    /// through the matching §4.6.7.4.1 all-zero analysis filter
    /// before quantisation. The decoder's §4.6.9.3 all-pole synthesis
    /// pass is the exact inverse of the applied (wire-quantised)
    /// filter, so TNS is transparent to the reconstruction while
    /// confining quantisation noise under the signal's temporal
    /// envelope. Safe to leave on: windows without a clear envelope
    /// simply carry no filter.
    pub fn set_tns(&mut self, enabled: bool) {
        self.tns_enabled = enabled;
    }

    /// Enable / disable §4.6.8.2 intensity-stereo emission (default
    /// **off**).
    ///
    /// When enabled, a high-frequency scalefactor band of a
    /// long-frame CPE whose two channels are strongly correlated
    /// (normalised cross-correlation above
    /// [`IS_CORR_MIN`]) is transmitted **once**: the left channel
    /// carries its spectrum, the right channel's band becomes the
    /// pseudo codebook `INTENSITY_HCB` (15, in-phase) or
    /// `INTENSITY_HCB2` (14, out-of-phase) with an intensity
    /// position `is_pos = 2·log2(e_l/e_r)` on the §4.6.8.1.4 DPCM
    /// track, and the decoder derives
    /// `r = ±0.5^(0.25·is_pos) · l` per §4.6.8.2.3. Such bands are
    /// excluded from the M/S mask (M/S, IS and PNS are mutually
    /// exclusive per band, and a set `ms_used` bit on an intensity
    /// band would signal the §4.6.8.2.3 phase reversal instead).
    /// Off by default: intensity coding discards the side
    /// information of the pair (only the energy ratio survives), a
    /// perceptual trade appropriate for low-rate coding but not for
    /// transparent transcodes.
    pub fn set_intensity_stereo(&mut self, enabled: bool) {
        self.is_enabled = enabled;
    }

    /// The configuration this encoder was built with.
    pub fn config(&self) -> &EncoderConfig {
        &self.config
    }

    /// Encode one 1024-sample-per-channel hop of interleaved `i16`
    /// PCM into one complete ADTS frame.
    ///
    /// `interleaved` must hold at most `1024 × channels` samples; a
    /// shorter slice (the stream tail) is zero-padded. The analysis
    /// window spans the previous hop and this one, so the emitted
    /// frame carries the overlap-add contribution of both.
    pub fn encode_frame(&mut self, interleaved: &[i16]) -> Result<Vec<u8>> {
        let ch = self.config.channels as usize;
        if interleaved.len() > FRAME_LEN * ch || interleaved.len() % ch != 0 {
            return Err(Error::EncoderInvalidConfig);
        }
        // De-interleave onto the ±32768 axis the §4.6.11 output
        // contract uses (no rescaling — the decoder's PCM stage
        // rounds these values back to i16 directly), permuting the
        // canonical input order into bitstream element order
        // (`element_src` — the inverse of the decoder's Table 1.19
        // output reorder).
        let mut cur: Vec<Vec<f64>> = vec![vec![0.0; FRAME_LEN]; ch];
        for (j, chan) in cur.iter_mut().enumerate() {
            let src = self.element_src[j];
            for (n, slot) in chan.iter_mut().take(interleaved.len() / ch).enumerate() {
                *slot = f64::from(interleaved[n * ch + src]);
            }
        }
        let frame = self.encode_hop(&cur)?;
        self.history = cur;
        Ok(frame)
    }

    /// Flush the final analysis overlap: emits one trailing ADTS
    /// frame whose window covers the last real hop and a zero hop.
    pub fn finish(&mut self) -> Result<Vec<u8>> {
        let ch = self.config.channels as usize;
        let zeros: Vec<Vec<f64>> = vec![vec![0.0; FRAME_LEN]; ch];
        let frame = self.encode_hop(&zeros)?;
        self.history = zeros;
        Ok(frame)
    }

    /// One-shot convenience: encode a whole interleaved `i16` buffer
    /// to a complete ADTS stream (`⌈n/1024⌉ + 1` frames — the `+1`
    /// is the [`StreamEncoder::finish`] flush).
    pub fn encode_all(&mut self, interleaved: &[i16]) -> Result<Vec<u8>> {
        let ch = self.config.channels as usize;
        if interleaved.len() % ch != 0 {
            return Err(Error::EncoderInvalidConfig);
        }
        let mut out = Vec::new();
        let hop = FRAME_LEN * ch;
        let mut chunks = interleaved.chunks(hop);
        // Always emit at least one content frame (an empty input
        // yields one silent frame + the flush frame).
        let first = chunks.next().unwrap_or(&[]);
        out.extend_from_slice(&self.encode_frame(first)?);
        for chunk in chunks {
            out.extend_from_slice(&self.encode_frame(chunk)?);
        }
        out.extend_from_slice(&self.finish()?);
        Ok(out)
    }

    /// Window `[history | cur]`, transform, quantize under the rate
    /// loop, and wrap the raw data block in an ADTS header.
    fn encode_hop(&mut self, cur: &[Vec<f64>]) -> Result<Vec<u8>> {
        let ch = self.config.channels as usize;

        // §4.6.11.3.2 block-switching state machine. A transient in
        // `cur` means the *next* frame (whose window's left half is
        // `cur`) must be EIGHT_SHORT; this frame becomes the
        // LONG_START lead-in (or stays short if a short run is
        // already active). A pending short from the previous hop
        // forces EIGHT_SHORT now; a short run with no continuation
        // exits through LONG_STOP.
        // LFE channels neither trigger nor follow block switching —
        // §4.5.2.1.3 fixes their window_sequence to ONLY_LONG.
        let transient = self
            .history
            .iter()
            .zip(cur.iter())
            .zip(self.lfe_slot.iter())
            .any(|((h, c), &lfe)| !lfe && detect_transient(h, c));
        let seq = if self.short_pending {
            WindowSequence::EightShort
        } else if transient && self.prev_seq != WindowSequence::EightShort {
            WindowSequence::LongStart
        } else if self.prev_seq == WindowSequence::EightShort {
            if transient {
                WindowSequence::EightShort
            } else {
                WindowSequence::LongStop
            }
        } else {
            WindowSequence::OnlyLong
        };
        self.short_pending = transient;

        // Per-channel analysis transform for the chosen sequence
        // (LFE channels always analyze ONLY_LONG — their own window
        // chain stays long/sine per §4.5.2.1.3, independent of the
        // frame's switching state).
        let mut spectra: Vec<Vec<f64>> = Vec::with_capacity(ch);
        for ((hist, chan), &lfe) in self
            .history
            .iter()
            .zip(cur.iter())
            .zip(self.lfe_slot.iter())
        {
            let ch_seq = if lfe { WindowSequence::OnlyLong } else { seq };
            spectra.push(analyze_channel(hist, chan, ch_seq)?);
        }
        self.prev_seq = seq;

        // §4.6.9 TNS: per-channel decision + analysis filtering,
        // BEFORE the M/S forward matrix — the decoder applies TNS
        // synthesis per channel *after* the M/S de-matrix
        // (§4.6.9.3's place in the §4.6 tool chain), so the encoder's
        // analysis pass runs in the L/R domain. The filtering mutates
        // the spectra once, outside the rate loop (the filter choice
        // is independent of the scalefactor offset).
        let max_sfb = if seq == WindowSequence::EightShort {
            NUM_SWB_SHORT_WINDOW[self.fs_index as usize]
        } else {
            NUM_SWB_LONG_WINDOW[self.fs_index as usize]
        };
        let mut tns: Vec<Option<TnsData>> = vec![None; ch];
        if self.tns_enabled {
            for (j, (spec, slot)) in spectra.iter_mut().zip(tns.iter_mut()).enumerate() {
                if self.lfe_slot[j] {
                    continue; // §4.5.2.1.3: no TNS on an LFE element
                }
                let permit = tns_temporal_permits(&self.history[j], &cur[j], seq);
                *slot = detect_and_apply_tns(spec, seq, max_sfb, self.fs_index, &permit)?;
            }
        }

        // Rate loop: uniform scalefactor offset in ±4 steps (3 dB
        // per step on the §4.6.2.3.3 quarter-step ladder). Coarsen
        // until the raw data block fits the budget; when it already
        // fits, refine (spend the remaining budget on precision) as
        // long as the finer frame still fits, down to
        // `-MAX_REFINE_OFFSET`.
        let budget = self.config.frame_budget_bytes();
        let mut sf_offset = 0i32;
        let mut raw_block = self.assemble_raw_block(seq, &spectra, &tns, sf_offset)?;
        let mut iterations = 0usize;
        if raw_block.len() > budget {
            while raw_block.len() > budget && iterations < MAX_RATE_ITERATIONS {
                sf_offset += 4;
                raw_block = self.assemble_raw_block(seq, &spectra, &tns, sf_offset)?;
                iterations += 1;
            }
        } else {
            while sf_offset > -MAX_REFINE_OFFSET && iterations < MAX_RATE_ITERATIONS {
                let finer = self.assemble_raw_block(seq, &spectra, &tns, sf_offset - 4)?;
                if finer.len() > budget {
                    break;
                }
                sf_offset -= 4;
                raw_block = finer;
                iterations += 1;
            }
        }
        // Fine pass: the ±4 ladder can leave up to 3 scalefactors of
        // precision unspent at the budget boundary (a full −4 step
        // un-zeros a whole swath of near-threshold coefficients at
        // once). Try the intermediate offsets, finest first.
        if raw_block.len() <= budget && sf_offset > -MAX_REFINE_OFFSET {
            for fine in [3i32, 2, 1] {
                let cand = self.assemble_raw_block(seq, &spectra, &tns, sf_offset - fine)?;
                if cand.len() <= budget {
                    raw_block = cand;
                    break;
                }
            }
        }

        // ADTS wrap. aac_frame_length is 13 bits; the budget floor
        // (16 bytes) and MAX_RATE_ITERATIONS guarantee headroom for
        // every realistic configuration, but validate regardless.
        let frame_len = ADTS_HEADER_BYTES_NO_CRC + raw_block.len();
        if frame_len >= (1 << 13) {
            return Err(Error::EncoderFrameOverflow);
        }
        let header = AdtsHeader {
            mpeg_version_mpeg2: false,
            protection_absent: true,
            profile: 1, // AAC LC: profile_ObjectType = AOT − 1 = 1
            sampling_frequency_index: self.fs_index,
            channel_configuration: self.channel_configuration,
            aac_frame_length: frame_len as u16,
            adts_buffer_fullness: 0x7FF, // VBR sentinel
            number_of_raw_data_blocks_in_frame: 1,
        };
        let mut out = Vec::with_capacity(frame_len);
        out.extend_from_slice(&header.write()?);
        out.extend_from_slice(&raw_block);
        Ok(out)
    }

    /// Assemble one `raw_data_block()` — the Table 1.19 element plan
    /// for the configuration (SCE / `common_window` CPE / LFE
    /// elements, then END) at the given rate-loop scalefactor offset.
    ///
    /// `tns` carries the per-channel §4.6.9 filter records decided
    /// once per hop (the spectra arrive already analysis-filtered);
    /// they land in each channel's `tns_data_present` / `tns_data`
    /// wire slots. Element instance tags count up per element kind,
    /// so the decoder's per-`(id, tag)` state slots stay distinct.
    fn assemble_raw_block(
        &self,
        seq: WindowSequence,
        spectra: &[Vec<f64>],
        tns: &[Option<TnsData>],
        sf_offset: i32,
    ) -> Result<Vec<u8>> {
        let mut asm = FrameAssembler::new();
        let plan = element_plan(self.channel_configuration)?;
        let mut sce_tag = 0u8;
        let mut cpe_tag = 0u8;
        let mut lfe_tag = 0u8;
        for elem in plan {
            let mut body_bits = BitWriter::new();
            match elem {
                ElementPlan::Sce(ch) => {
                    asm.push_channel_header(IdSynEle::Sce, sce_tag)?;
                    sce_tag += 1;
                    self.assemble_sce(&mut body_bits, &spectra[ch], &tns[ch], seq, sf_offset)?;
                }
                ElementPlan::Lfe(ch) => {
                    asm.push_channel_header(IdSynEle::Lfe, lfe_tag)?;
                    lfe_tag += 1;
                    self.assemble_lfe(&mut body_bits, &spectra[ch], sf_offset)?;
                }
                ElementPlan::Cpe(l, r) => {
                    asm.push_channel_header(IdSynEle::Cpe, cpe_tag)?;
                    cpe_tag += 1;
                    self.assemble_cpe(
                        &mut body_bits,
                        &spectra[l],
                        &spectra[r],
                        &tns[l],
                        &tns[r],
                        seq,
                        sf_offset,
                    )?;
                }
            }
            let nbits = body_bits.bit_position();
            asm.push_channel_body_bits(&body_bits.finish(), nbits)?;
        }
        Ok(asm.push_end())
    }

    /// Quantize a spectrum for this frame: the grouped short-window
    /// path for `EIGHT_SHORT`, the long path otherwise.
    #[allow(clippy::too_many_arguments)]
    fn quantize_seq(
        &self,
        spec: &[f64],
        seq: WindowSequence,
        sf_offset: i32,
        peak: f64,
        pns_bands: &[bool],
        is_bands: &[IsBand],
    ) -> Result<QuantizedChannel> {
        if seq == WindowSequence::EightShort {
            quantize_channel_short(spec, self.fs_index, sf_offset, peak)
        } else {
            quantize_channel(
                spec,
                seq,
                self.fs_index,
                sf_offset,
                peak,
                pns_bands,
                is_bands,
            )
        }
    }

    /// One SCE body: quantize (with the mono blanket PNS allowance
    /// when enabled on a long frame) and write
    /// `individual_channel_stream(0)`.
    fn assemble_sce(
        &self,
        body_bits: &mut BitWriter,
        spec: &[f64],
        tns: &Option<TnsData>,
        seq: WindowSequence,
        sf_offset: i32,
    ) -> Result<()> {
        // §4.6.13 PNS emission is opt-in (set_pns) and long-frame
        // only; a lone channel grants a blanket per-band allowance
        // (the noise-likeness test in quantize_group decides).
        let pns_long = self.pns_enabled && seq != WindowSequence::EightShort;
        let num_swb_long = NUM_SWB_LONG_WINDOW[self.fs_index as usize] as usize;
        let peak = spec.iter().fold(0.0f64, |m, &v| m.max(v.abs()));
        let pns_bands = if pns_long {
            vec![true; num_swb_long]
        } else {
            Vec::new()
        };
        let mut chan = self.quantize_seq(spec, seq, sf_offset, peak, &pns_bands, &[])?;
        attach_tns(&mut chan, tns);
        chan.body.write(body_bits, 2, self.fs_index, false)?;
        chan.spectral.write(
            body_bits,
            &chan.info,
            &chan.body.section_data,
            self.fs_index,
        )
    }

    /// One LFE body under the §4.5.2.1.3 restrictions: the element is
    /// a plain `individual_channel_stream(0)`, always
    /// `ONLY_LONG_SEQUENCE` with the sine window (the analysis stage
    /// already ran this channel long), no TNS / PNS / prediction, and
    /// only the lowest [`LFE_MAX_LINES`] spectral lines non-zero.
    fn assemble_lfe(&self, body_bits: &mut BitWriter, spec: &[f64], sf_offset: i32) -> Result<()> {
        let mut lfe_spec = spec.to_vec();
        for c in lfe_spec[LFE_MAX_LINES..].iter_mut() {
            *c = 0.0;
        }
        let peak = lfe_spec.iter().fold(0.0f64, |m, &v| m.max(v.abs()));
        let chan = self.quantize_seq(
            &lfe_spec,
            WindowSequence::OnlyLong,
            sf_offset,
            peak,
            &[],
            &[],
        )?;
        chan.body.write(body_bits, 2, self.fs_index, false)?;
        chan.spectral.write(
            body_bits,
            &chan.info,
            &chan.body.section_data,
            self.fs_index,
        )
    }

    /// One `common_window` CPE body: the per-band IS / PNS / M/S
    /// decisions, joint quantization on the pair peak, and the
    /// shared-`ics_info` wire assembly.
    #[allow(clippy::too_many_arguments)]
    fn assemble_cpe(
        &self,
        body_bits: &mut BitWriter,
        l_spec: &[f64],
        r_spec: &[f64],
        tns_l: &Option<TnsData>,
        tns_r: &Option<TnsData>,
        seq: WindowSequence,
        sf_offset: i32,
    ) -> Result<()> {
        let pns_long = self.pns_enabled && seq != WindowSequence::EightShort;
        // §4.6.8.2: per-band intensity decision first (opt-in,
        // long frames) — an IS band is transmitted once via
        // the left channel and must not also be M/S-coded
        // (mutual exclusion; a set ms_used bit on an
        // intensity band signals the §4.6.8.2.3 phase
        // reversal, not an M/S de-matrix).
        let is_bands: Vec<IsBand> = if self.is_enabled && seq != WindowSequence::EightShort {
            is_decide(l_spec, r_spec, self.fs_index)?
        } else {
            Vec::new()
        };
        // §4.6.13 per-band PNS decision on the original l/r
        // spectra (a noise band must not be M/S-transformed —
        // mutual exclusion, §4.6.13.5 — and IS wins where the
        // two overlap).
        let pns = if pns_long {
            pns_decide_pair(l_spec, r_spec, &is_bands, self.fs_index)?
        } else {
            PairPns::default()
        };
        // §4.6.8.1: per-band M/S decision, then quantize the coding
        // spectra (m/s on flagged bands, l/r elsewhere). Both coding
        // channels share the pair's loudest peak for the masking
        // spread, so the side channel's noise floor is judged
        // against the pair, not against its own (often tiny) peak.
        // The mask is one row per window group (`ms_used[g][sfb]`);
        // long sequences have one group, EIGHT_SHORT frames decide
        // per (group, sfb) under the jointly-decided grouping.
        let (ms_rows, mut left, mut right);
        if seq == WindowSequence::EightShort {
            // A common_window CPE shares one ics_info, so the
            // §4.5.2.3.4 grouping is decided ONCE on the pair
            // envelope (per-coefficient L/R energy sum) and imposed
            // on both channels — independent decisions could
            // diverge and desync the shared wire layout.
            let combined: Vec<f64> = l_spec
                .iter()
                .zip(r_spec.iter())
                .map(|(&l, &r)| (l * l + r * r).sqrt())
                .collect();
            let offsets = short_window_offsets(self.fs_index)?;
            let num_swb = NUM_SWB_SHORT_WINDOW[self.fs_index as usize] as usize;
            let wgl = decide_short_grouping(&combined, offsets, num_swb);
            let rows = ms_decide_short(l_spec, r_spec, offsets, num_swb, &wgl);
            let (code_l, code_r) = if rows.iter().flatten().any(|&b| b) {
                apply_ms_short(l_spec, r_spec, &rows, offsets, &wgl)
            } else {
                (l_spec.to_vec(), r_spec.to_vec())
            };
            let pair_peak = code_l
                .iter()
                .chain(code_r.iter())
                .fold(0.0f64, |m, &v| m.max(v.abs()));
            left = quantize_channel_short_grouped(
                &code_l,
                self.fs_index,
                sf_offset,
                pair_peak,
                wgl.clone(),
            )?;
            right =
                quantize_channel_short_grouped(&code_r, self.fs_index, sf_offset, pair_peak, wgl)?;
            ms_rows = rows;
        } else {
            let mut ms_used = ms_decide(l_spec, r_spec, self.fs_index)?;
            for (sfb, band) in is_bands.iter().enumerate() {
                if band.is_some() {
                    if let Some(m) = ms_used.get_mut(sfb) {
                        *m = false;
                    }
                }
            }
            // A band either channel will noise-code is excluded
            // from the M/S transform; a both-channels-noise band
            // whose content correlates re-sets its ms_used bit,
            // which per §4.6.13.3 signals the decoder to draw the
            // *same* random vector for both channels (correlated
            // noise) rather than an M/S de-matrix.
            for (sfb, m) in ms_used.iter_mut().enumerate() {
                let l_n = pns.l_noise.get(sfb).copied().unwrap_or(false);
                let r_n = pns.r_noise.get(sfb).copied().unwrap_or(false);
                if l_n || r_n {
                    *m = pns.shared.get(sfb).copied().unwrap_or(false);
                }
            }
            let ms_transform: Vec<bool> = ms_used
                .iter()
                .enumerate()
                .map(|(sfb, &m)| {
                    m && !pns.l_noise.get(sfb).copied().unwrap_or(false)
                        && !pns.r_noise.get(sfb).copied().unwrap_or(false)
                })
                .collect();
            let (code_l, code_r) = if ms_transform.iter().any(|&b| b) {
                apply_ms(l_spec, r_spec, &ms_transform, self.fs_index)?
            } else {
                (l_spec.to_vec(), r_spec.to_vec())
            };
            let pair_peak = code_l
                .iter()
                .chain(code_r.iter())
                .fold(0.0f64, |m, &v| m.max(v.abs()));
            left = self.quantize_seq(&code_l, seq, sf_offset, pair_peak, &pns.l_noise, &[])?;
            right =
                self.quantize_seq(&code_r, seq, sf_offset, pair_peak, &pns.r_noise, &is_bands)?;
            ms_rows = vec![ms_used];
        }
        attach_tns(&mut left, tns_l);
        attach_tns(&mut right, tns_r);

        // §4.4.2.3: common_window = 1, one shared ics_info,
        // then the two-bit ms_mask_present (+ mask when 1: one bit
        // per (window group, sfb), group-major — Table 4.5).
        body_bits.write_bit(true);
        left.info.write(body_bits, 2, self.fs_index, true)?;
        let any_ms = ms_rows.iter().flatten().any(|&b| b);
        let all_ms = !ms_rows.is_empty()
            && ms_rows.iter().all(|row| !row.is_empty())
            && ms_rows.iter().flatten().all(|&b| b);
        if all_ms {
            body_bits.write_u32(2, 2); // all ones, no mask bits
        } else if any_ms {
            body_bits.write_u32(1, 2);
            for &b in ms_rows.iter().flatten() {
                body_bits.write_bit(b);
            }
        } else {
            body_bits.write_u32(0, 2);
        }
        for chan in [&left, &right] {
            chan.body
                .write_with_ics_info(body_bits, &chan.info, 2, false)?;
            chan.spectral.write(
                body_bits,
                &chan.info,
                &chan.body.section_data,
                self.fs_index,
            )?;
        }

        Ok(())
    }
}

/// Attach a channel's §4.6.9 TNS record to its wire body.
fn attach_tns(chan: &mut QuantizedChannel, slot: &Option<TnsData>) {
    if let Some(t) = slot {
        chan.body.tns_data_present = true;
        chan.body.tns_data = Some(t.clone());
    }
}

/// Subblock length of the transient detector — one short-window hop
/// (128 samples), so a detected attack aligns with the short-window
/// grid it triggers.
const TRANSIENT_SUBBLOCK: usize = SHORT_SEQ_HOP;

/// Energy jump (×) a subblock must show over the running average of
/// the preceding subblocks to count as a transient attack.
const TRANSIENT_RATIO: f64 = 12.0;

/// Absolute per-subblock energy floor below which an attack is
/// ignored (silence-to-quiet transitions don't warrant short
/// windows): a 128-sample block at ~±180 amplitude.
const TRANSIENT_FLOOR: f64 = 128.0 * 180.0 * 180.0;

/// Minimum established (pre-attack) average subblock energy for the
/// detector to arm. Below this the context is effectively silence
/// and an onset codes acceptably with the long-window pair (its
/// left flank is silence — there is no signal to smear pre-echo
/// into), so the detector stays quiet rather than switching on
/// every stream/passage onset.
const TRANSIENT_ARM: f64 = TRANSIENT_FLOOR / TRANSIENT_RATIO;

/// Detect a transient attack inside one channel's next hop.
///
/// The 2048-sample context `[hist | cur]` is split into sixteen
/// 128-sample subblocks; an attack fires when a subblock **in the
/// `cur` half** has energy that (a) clears the absolute
/// [`TRANSIENT_FLOOR`], and (b) jumps [`TRANSIENT_RATIO`]× above the
/// **maximum** energy of the preceding eight subblocks (one hop of
/// context) — provided that maximum itself clears [`TRANSIENT_ARM`]
/// (an established signal level to jump *from*). Using the recent
/// max rather than a mean keeps beat nulls in steady multi-tone
/// content from arming spurious triggers, and keeps zeroed history
/// (stream start) from diluting the reference: an onset out of true
/// digital silence codes acceptably with the long-window pair (its
/// left flank is silence — there is nothing to smear pre-echo into),
/// so the detector deliberately stays quiet there.
fn detect_transient(hist: &[f64], cur: &[f64]) -> bool {
    let energies: Vec<f64> = hist
        .chunks(TRANSIENT_SUBBLOCK)
        .chain(cur.chunks(TRANSIENT_SUBBLOCK))
        .map(|b| b.iter().map(|&v| v * v).sum())
        .collect();
    let hist_blocks = hist.len() / TRANSIENT_SUBBLOCK;
    for (j, &e) in energies.iter().enumerate().skip(hist_blocks) {
        let ctx = &energies[j.saturating_sub(hist_blocks.max(1))..j];
        let reference = ctx.iter().fold(0.0f64, |m, &v| m.max(v));
        if e > TRANSIENT_FLOOR && reference > TRANSIENT_ARM && e > TRANSIENT_RATIO * reference {
            return true;
        }
    }
    false
}

/// TNS temporal-envelope gate: minimum `max / mean` subblock-energy
/// flatness ratio of a transform window's time region for TNS to be
/// considered on it. A steady tone (or dense steady multitone)
/// measures close to 1; a burst-and-decay envelope inside the window
/// measures well above. See [`tns_temporal_permits`].
const TNS_TEMPORAL_RATIO: f64 = 3.0;

/// Absolute per-window mean subblock energy floor below which the
/// TNS gate stays closed (silence / near-silence windows carry no
/// audible envelope to protect). One 128-sample subblock at ~±90
/// amplitude.
const TNS_TEMPORAL_FLOOR: f64 = 128.0 * 90.0 * 90.0;

/// §4.6.9.1 temporal gate for the encode-side TNS decision: per
/// transform window, `true` iff the window's raw time samples show a
/// strongly non-flat energy envelope.
///
/// The window's time region (2048 samples for a long sequence; the
/// 256-sample `SHORT_SEQ_START + j·SHORT_SEQ_HOP` slice per short
/// window) is split into 16 subblocks whose energies are reduced to
/// the `max / mean` flatness ratio; the gate opens above
/// [`TNS_TEMPORAL_RATIO`] (with a [`TNS_TEMPORAL_FLOOR`] silence
/// guard). This is the *time-domain* half of the TNS decision — the
/// spectral prediction gain alone also fires on steady tonal windows
/// (their leakage skirts are highly predictable) where the temporal
/// envelope is flat and shaping buys nothing; measuring the envelope
/// directly on the input samples keeps TNS to the transient /
/// speech-like windows it exists for (§4.6.9.1's duality argument
/// run forward).
fn tns_temporal_permits(hist: &[f64], cur: &[f64], seq: WindowSequence) -> Vec<bool> {
    let region = |i: usize| -> f64 {
        if i < FRAME_LEN {
            hist[i]
        } else {
            cur[i - FRAME_LEN]
        }
    };
    let flatness_permits = |base: usize, len: usize| -> bool {
        let sub = len / 16;
        let energies: Vec<f64> = (0..16)
            .map(|j| {
                (0..sub)
                    .map(|m| {
                        let v = region(base + j * sub + m);
                        v * v
                    })
                    .sum()
            })
            .collect();
        let mean = energies.iter().sum::<f64>() / 16.0;
        let max = energies.iter().fold(0.0f64, |a, &b| a.max(b));
        // Normalise the floor to the subblock length (the constant is
        // stated for a 128-sample subblock).
        let floor = TNS_TEMPORAL_FLOOR * sub as f64 / 128.0;
        mean > floor && max > TNS_TEMPORAL_RATIO * mean
    };
    if seq == WindowSequence::EightShort {
        (0..8)
            .map(|j| {
                flatness_permits(
                    SHORT_SEQ_START + j * SHORT_SEQ_HOP,
                    2 * SHORT_WINDOW_LEN as usize,
                )
            })
            .collect()
    } else {
        vec![flatness_permits(0, LONG_TRANSFORM_LEN)]
    }
}

/// Run the §4.6.11.3.1 analysis transform for one channel under the
/// chosen `window_sequence`, over the 2048-sample region
/// `[hist | cur]`.
///
/// * Long sequences: one 2048-point MDCT under the
///   [`long_sequence_window`] (sine shape throughout — this encoder
///   never switches shapes, so left/right inheritance is trivial).
/// * `EIGHT_SHORT`: eight 256-point MDCTs at offsets
///   `448 + j·128` inside the region ([`SHORT_SEQ_START`] /
///   [`SHORT_SEQ_HOP`]), each under its [`short_window_j`];
///   concatenated window-major (`8 × 128` coefficients).
fn analyze_channel(hist: &[f64], cur: &[f64], seq: WindowSequence) -> Result<Vec<f64>> {
    debug_assert_eq!(hist.len(), FRAME_LEN);
    debug_assert_eq!(cur.len(), FRAME_LEN);
    let region = |i: usize| -> f64 {
        if i < FRAME_LEN {
            hist[i]
        } else {
            cur[i - FRAME_LEN]
        }
    };
    if seq == WindowSequence::EightShort {
        let short_len = SHORT_WINDOW_LEN as usize; // 128
        let n_s = 2 * short_len; // 256
        let mut out = Vec::with_capacity(8 * short_len);
        for j in 0..8 {
            let w = short_window_j(j, WindowShape::Sine, WindowShape::Sine);
            let base = SHORT_SEQ_START + j * SHORT_SEQ_HOP;
            let seg: Vec<f64> = (0..n_s).map(|m| region(base + m) * w[m]).collect();
            out.extend_from_slice(&forward_mdct(&seg, n_s));
        }
        Ok(out)
    } else {
        let w = long_sequence_window(seq, WindowShape::Sine, WindowShape::Sine)?;
        let z: Vec<f64> = (0..LONG_TRANSFORM_LEN).map(|m| region(m) * w[m]).collect();
        Ok(forward_mdct(&z, LONG_TRANSFORM_LEN))
    }
}

/// One quantized channel, ready for wire assembly.
struct QuantizedChannel {
    info: IcsInfo,
    body: IcsBody,
    spectral: SpectralData,
}

/// One band's §4.6.8.2 intensity-stereo decision: `None` codes the
/// band normally; `Some((codebook, is_pos))` transmits the right
/// channel's band as the intensity book (15 in-phase / 14
/// out-of-phase) at the given position on the `0.5^(0.25·is_pos)`
/// gain ladder.
type IsBand = Option<(u8, i32)>;

/// Lowest spectral line an intensity-coded band may start at:
/// intensity stereo exploits the ear's insensitivity to phase at
/// high frequencies (§4.6.8.2.1), so the bottom quarter of the
/// spectrum always keeps discrete coding.
const IS_MIN_SPECTRAL_LINE: usize = FRAME_LEN / 4;

/// Minimum normalised cross-correlation `|Σ l·r| / sqrt(Σl²·Σr²)`
/// for a band to qualify for intensity coding. Deliberately strict:
/// a genuine intensity image (shared content at a per-channel gain)
/// measures ≈ 1.0, while the leakage skirts of two *different*
/// tones — deterministic, slowly-decaying magnitude profiles — were
/// measured correlating as high as 0.93 on synthetic two-tone
/// content; IS-coding those would substitute the wrong (if masked)
/// waveform for no bit win over the cull they get anyway.
pub const IS_CORR_MIN: f64 = 0.95;

/// Relative peak floor for the intensity decision: a band whose
/// loudest coefficient (either channel) sits more than ~50 dB below
/// the pair's frame peak carries only leakage floor — the *distant*
/// skirts of any two windowed tones are smooth deterministic decays
/// that correlate near 1.0 regardless of the tones' relation
/// (measured 0.98 between two unrelated tones' far tails), so
/// correlation alone cannot vet an image down there, and a band that
/// quiet codes for almost nothing (or culls) discretely anyway.
const IS_PEAK_FLOOR_RATIO: f64 = 3e-3;

/// §4.6.8.2 per-band intensity-stereo decision (encode side) for a
/// long-frame channel pair.
///
/// A band qualifies when it lies above [`IS_MIN_SPECTRAL_LINE`],
/// both channels carry energy, and the normalised cross-correlation
/// clears [`IS_CORR_MIN`]. The transmitted position quantises the
/// energy ratio onto the §4.6.8.2.3 gain ladder —
/// `0.5^(0.25·is_pos) = sqrt(e_r/e_l)` ⇒ `is_pos = 2·log2(e_l/e_r)`
/// — and the codebook carries the phase: `INTENSITY_HCB` (15) when
/// the channels correlate positively, `INTENSITY_HCB2` (14) when
/// they anti-correlate.
fn is_decide(l_spec: &[f64], r_spec: &[f64], fs_index: u8) -> Result<Vec<IsBand>> {
    let offsets = long_window_offsets(fs_index)?;
    let num_swb = NUM_SWB_LONG_WINDOW[fs_index as usize] as usize;
    let frame_peak = l_spec
        .iter()
        .chain(r_spec.iter())
        .fold(0.0f64, |m, &v| m.max(v.abs()));
    let peak_floor = frame_peak * IS_PEAK_FLOOR_RATIO;
    let mut out: Vec<IsBand> = vec![None; num_swb];
    for (sfb, slot) in out.iter_mut().enumerate() {
        let start = offsets[sfb] as usize;
        let end = offsets[sfb + 1] as usize;
        if start < IS_MIN_SPECTRAL_LINE {
            continue;
        }
        let mut e_l = 0.0f64;
        let mut e_r = 0.0f64;
        let mut dot = 0.0f64;
        let mut band_peak = 0.0f64;
        for k in start..end {
            e_l += l_spec[k] * l_spec[k];
            e_r += r_spec[k] * r_spec[k];
            dot += l_spec[k] * r_spec[k];
            band_peak = band_peak.max(l_spec[k].abs()).max(r_spec[k].abs());
        }
        if e_l <= 0.0 || e_r <= 0.0 || band_peak < peak_floor {
            continue;
        }
        let corr = dot.abs() / (e_l * e_r).sqrt();
        if corr < IS_CORR_MIN {
            continue;
        }
        let pos = (2.0 * (e_l / e_r).log2()).round();
        // Keep the position within a range the ±60-delta track can
        // plausibly reach; a >±30 dB imbalance codes better discretely.
        if !(-80.0..=80.0).contains(&pos) {
            continue;
        }
        let cb = if dot >= 0.0 {
            INTENSITY_HCB
        } else {
            INTENSITY_HCB2
        };
        *slot = Some((cb, pos as i32));
    }
    Ok(out)
}

/// Minimum normalised cross-correlation for a both-channels-noise
/// band to be flagged *correlated* (§4.6.13.3): the decoder then
/// draws the **same** random vector for both channels. Positive
/// correlation only — the shared vector reproduces positively
/// correlated noise, so anti-correlated noise stays on independent
/// draws.
pub const PNS_CORR_MIN: f64 = 0.5;

/// Per-band §4.6.13 PNS decision for a channel pair (encode side).
#[derive(Debug, Default)]
struct PairPns {
    /// Left channel per-band PNS allowance (noise-like content).
    l_noise: Vec<bool>,
    /// Right channel per-band PNS allowance.
    r_noise: Vec<bool>,
    /// Both channels noise **and** correlated above
    /// [`PNS_CORR_MIN`] — emitted as a set `ms_used` bit
    /// (§4.6.13.3 correlated-noise signalling).
    shared: Vec<bool>,
}

/// Decide the §4.6.13 noise bands of a long-frame channel pair on
/// the original (pre-M/S) spectra.
///
/// Per band: each channel qualifies through the same
/// [`is_noise_like`] density statistic the mono path uses; a band
/// where **both** qualify additionally measures its normalised
/// cross-correlation — above [`PNS_CORR_MIN`] the band is flagged
/// `shared`, which the CPE assembler emits as a set `ms_used` bit so
/// the decoder synthesises the same random vector into both channels
/// (§4.6.13.3; no M/S de-matrix is performed on such a band — PNS
/// and M/S are mutually exclusive, §4.6.13.5). Bands claimed by
/// intensity stereo (`is_bands`) are skipped — M/S, IS and PNS are
/// pairwise exclusive on a band.
fn pns_decide_pair(
    l_spec: &[f64],
    r_spec: &[f64],
    is_bands: &[IsBand],
    fs_index: u8,
) -> Result<PairPns> {
    let offsets = long_window_offsets(fs_index)?;
    let num_swb = NUM_SWB_LONG_WINDOW[fs_index as usize] as usize;
    let mut out = PairPns {
        l_noise: vec![false; num_swb],
        r_noise: vec![false; num_swb],
        shared: vec![false; num_swb],
    };
    for sfb in 0..num_swb {
        if is_bands.get(sfb).copied().flatten().is_some() {
            continue; // intensity wins the band
        }
        let start = offsets[sfb] as usize;
        let end = offsets[sfb + 1] as usize;
        let l_band = &l_spec[start..end];
        let r_band = &r_spec[start..end];
        let l_n = is_noise_like(l_band);
        let r_n = is_noise_like(r_band);
        out.l_noise[sfb] = l_n;
        out.r_noise[sfb] = r_n;
        if l_n && r_n {
            let e_l: f64 = l_band.iter().map(|&v| v * v).sum();
            let e_r: f64 = r_band.iter().map(|&v| v * v).sum();
            let dot: f64 = l_band.iter().zip(r_band).map(|(&a, &b)| a * b).sum();
            if e_l > 0.0 && e_r > 0.0 && dot / (e_l * e_r).sqrt() >= PNS_CORR_MIN {
                out.shared[sfb] = true;
            }
        }
    }
    Ok(out)
}

/// §4.6.8.1 per-band M/S decision for a channel pair.
///
/// A band selects M/S coding when the mid/side transform
/// (`m = (l+r)/2`, `s = (l−r)/2`) concentrates its energy: with
/// `e_m + e_s = (e_l + e_r)/2` (exact, by the transform's geometry),
/// requiring `min(e_m, e_s) ≤ (e_l + e_r)/8` means the quieter
/// transformed channel holds at most a quarter of the transformed
/// energy (≥ ~5 dB below its partner) — it will cull or code
/// cheaply while the dominant channel carries the band once instead
/// of twice.
fn ms_decide(l_spec: &[f64], r_spec: &[f64], fs_index: u8) -> Result<Vec<bool>> {
    let offsets = long_window_offsets(fs_index)?;
    let num_swb = NUM_SWB_LONG_WINDOW[fs_index as usize] as usize;
    let mut used = Vec::with_capacity(num_swb);
    for sfb in 0..num_swb {
        let start = offsets[sfb] as usize;
        let end = offsets[sfb + 1] as usize;
        let mut e_lr = 0.0f64;
        let mut e_m = 0.0f64;
        let mut e_s = 0.0f64;
        for k in start..end {
            let (l, r) = (l_spec[k], r_spec[k]);
            e_lr += l * l + r * r;
            let m = 0.5 * (l + r);
            let s = 0.5 * (l - r);
            e_m += m * m;
            e_s += s * s;
        }
        used.push(e_lr > 0.0 && e_m.min(e_s) <= e_lr / 8.0);
    }
    Ok(used)
}

/// Forward M/S matrix: on flagged bands the coding pair is
/// `(m, s) = ((l+r)/2, (l−r)/2)` — the exact inverse of the
/// decoder's §4.6.8.1.3 `l = m+s` / `r = m−s` de-matrix — and the
/// identity elsewhere.
fn apply_ms(
    l_spec: &[f64],
    r_spec: &[f64],
    ms_used: &[bool],
    fs_index: u8,
) -> Result<(Vec<f64>, Vec<f64>)> {
    let offsets = long_window_offsets(fs_index)?;
    let mut code_l = l_spec.to_vec();
    let mut code_r = r_spec.to_vec();
    for (sfb, &used) in ms_used.iter().enumerate() {
        if !used {
            continue;
        }
        let start = offsets[sfb] as usize;
        let end = offsets[sfb + 1] as usize;
        for k in start..end {
            let m = 0.5 * (l_spec[k] + r_spec[k]);
            let s = 0.5 * (l_spec[k] - r_spec[k]);
            code_l[k] = m;
            code_r[k] = s;
        }
    }
    Ok((code_l, code_r))
}

/// §4.6.8.1 per-`(window group, sfb)` M/S decision for an
/// `EIGHT_SHORT_SEQUENCE` channel pair under the shared grouping
/// `wgl` — the same energy-concentration criterion as [`ms_decide`],
/// summed over every window of the group (the mask granularity the
/// Table 4.5 `ms_used[g][sfb]` wire provides). `l_spec` / `r_spec`
/// are window-major 8 × 128 buffers.
fn ms_decide_short(
    l_spec: &[f64],
    r_spec: &[f64],
    offsets: &[u16],
    num_swb: usize,
    wgl: &[u8],
) -> Vec<Vec<bool>> {
    let short_len = SHORT_WINDOW_LEN as usize;
    let mut rows = Vec::with_capacity(wgl.len());
    let mut win_base = 0usize;
    for &len in wgl {
        let mut row = Vec::with_capacity(num_swb);
        for sfb in 0..num_swb {
            let mut e_lr = 0.0f64;
            let mut e_m = 0.0f64;
            let mut e_s = 0.0f64;
            for w in win_base..win_base + len as usize {
                for k in offsets[sfb] as usize..offsets[sfb + 1] as usize {
                    let (l, r) = (l_spec[w * short_len + k], r_spec[w * short_len + k]);
                    e_lr += l * l + r * r;
                    let m = 0.5 * (l + r);
                    let s = 0.5 * (l - r);
                    e_m += m * m;
                    e_s += s * s;
                }
            }
            row.push(e_lr > 0.0 && e_m.min(e_s) <= e_lr / 8.0);
        }
        rows.push(row);
        win_base += len as usize;
    }
    rows
}

/// Forward M/S butterfly on the flagged `(group, sfb)` bands of a
/// window-major short-sequence pair — the short-frame counterpart of
/// [`apply_ms`], walking every window of a flagged group.
fn apply_ms_short(
    l_spec: &[f64],
    r_spec: &[f64],
    rows: &[Vec<bool>],
    offsets: &[u16],
    wgl: &[u8],
) -> (Vec<f64>, Vec<f64>) {
    let short_len = SHORT_WINDOW_LEN as usize;
    let mut code_l = l_spec.to_vec();
    let mut code_r = r_spec.to_vec();
    let mut win_base = 0usize;
    for (row, &len) in rows.iter().zip(wgl) {
        for (sfb, &used) in row.iter().enumerate() {
            if !used {
                continue;
            }
            for w in win_base..win_base + len as usize {
                for k in offsets[sfb] as usize..offsets[sfb + 1] as usize {
                    let i = w * short_len + k;
                    let m = 0.5 * (l_spec[i] + r_spec[i]);
                    let s = 0.5 * (l_spec[i] - r_spec[i]);
                    code_l[i] = m;
                    code_r[i] = s;
                }
            }
        }
        win_base += len as usize;
    }
    (code_l, code_r)
}

/// §4.6.2 forward quantizer for one coefficient at scalefactor `sf`:
/// `q = sign(x) · NINT((|x| · 2^(−0.25·(sf−100)))^(3/4))`, the exact
/// inverse of the normative `|q|^(4/3) · 2^(0.25·(sf−100))`
/// (round-half-away-from-zero per §1.3 `NINT`).
fn quantize_coef(x: f64, sf: i32) -> i32 {
    let gain = (0.25 * f64::from(sf - SF_OFFSET)).exp2();
    let mag = (x.abs() / gain).powf(0.75).round();
    let mag = mag.min(f64::from(MAX_QUANT)) as i32;
    if x < 0.0 {
        -mag
    } else {
        mag
    }
}

/// The masking-spread scalefactor for a band whose peak coefficient
/// is `peak` in a frame whose loudest band peaks at `frame_peak`:
/// solve `(peak / 2^(0.25·(sf−100)))^(3/4) = M_b` for `sf` with
/// `M_b = TARGET_PEAK_MAG · (peak/frame_peak)^SPREAD`, i.e.
/// `sf = 100 + 4·log2(peak) − (16/3)·log2(M_b)`.
///
/// Returns `None` when the band's spread target falls below
/// [`MIN_TARGET_MAG`] — such a band quantizes to silence anyway and
/// is culled to `ZERO_HCB` by the caller.
fn band_scalefactor(peak: f64, frame_peak: f64, sf_offset: i32) -> Option<i32> {
    if peak <= 0.0 || frame_peak <= 0.0 {
        return None;
    }
    let target = TARGET_PEAK_MAG * (peak / frame_peak).powf(SPREAD);
    if target < MIN_TARGET_MAG {
        return None;
    }
    let sf = f64::from(SF_OFFSET) + 4.0 * peak.log2() - (16.0 / 3.0) * target.log2();
    Some((sf.round() as i32 + sf_offset).clamp(0, 255))
}

/// Smallest Table 4.95 spectrum codebook whose LAV covers `qmax`.
/// `1`/`3` are the 4-tuple books (LAV 1 / 2), `5`/`7`/`9` the pair
/// books (LAV 4 / 7 / 12), `11` the ESC book.
fn codebook_for(qmax: i32) -> u8 {
    match qmax {
        0 => ZERO_HCB,
        1 => 1,
        2 => 3,
        3..=4 => 5,
        5..=7 => 7,
        8..=12 => 9,
        _ => 11,
    }
}

/// Per-band quantization result for one window group.
struct GroupQuant {
    x_quant: Vec<i32>,
    sfb_cb: Vec<u8>,
    sfs: Vec<Option<i32>>,
    /// §4.6.13 noise energies for PNS bands (`sfb_cb == NOISE_HCB`):
    /// the band's target L2 norm on the `2^(0.25·noise_nrg)` ladder.
    noise: Vec<Option<i32>>,
    /// §4.6.8.2 intensity positions for IS bands (`sfb_cb == 14/15`,
    /// right channel of a CPE only): the position on the
    /// `0.5^(0.25·is_pos)` gain ladder.
    is_pos: Vec<Option<i32>>,
}

/// Quantize the `num_swb` scalefactor bands of one window group.
///
/// `spec` is the group's coefficient buffer (1024 lines for a long
/// sequence, `window_group_length × 128` interleaved lines for a
/// short group); `offsets` the
/// matching §4.5.4 band-offset table. `prev_sf` threads the DPCM ±60
/// clamp across groups in wire order — the §4.6.2.3.2 accumulator is
/// a single track for the whole channel.
///
/// Pass 1 picks the masking-spread scalefactor per band; pass 2
/// re-quantizes with the clamped value and derives the codebook. A
/// band whose coefficients all quantize to zero (or whose target is
/// culled) stays `ZERO_HCB` and transmits no scalefactor.
#[allow(clippy::too_many_arguments)]
fn quantize_group(
    spec: &[f64],
    offsets: &[u16],
    num_swb: usize,
    sf_offset: i32,
    frame_peak: f64,
    prev_sf: &mut Option<i32>,
    pns_bands: &[bool],
) -> GroupQuant {
    let mut x_quant = vec![0i32; spec.len()];
    let mut sfb_cb = vec![ZERO_HCB; num_swb];
    let mut sfs: Vec<Option<i32>> = vec![None; num_swb];
    let mut noise: Vec<Option<i32>> = vec![None; num_swb];
    for sfb in 0..num_swb {
        let start = offsets[sfb] as usize;
        let end = (offsets[sfb + 1] as usize).min(spec.len());
        let peak = spec[start..end].iter().fold(0.0f64, |m, &v| m.max(v.abs()));
        let Some(mut sf) = band_scalefactor(peak, frame_peak, sf_offset) else {
            continue; // culled: below the frame's masking floor
        };
        // §4.6.13 PNS: a wide band with no dominant spectral line is
        // transmitted as a noise energy instead of coefficients. The
        // per-band allowance comes from the caller (blanket for mono,
        // the pre-M/S pair decision for a CPE channel).
        if pns_bands.get(sfb).copied().unwrap_or(false) && is_noise_like(&spec[start..end]) {
            let nrg: f64 = spec[start..end].iter().map(|&x| x * x).sum();
            // Target L2 norm 2^(0.25·noise_nrg) == sqrt(nrg).
            let noise_nrg = (4.0 * nrg.sqrt().log2()).round() as i32;
            sfb_cb[sfb] = NOISE_HCB;
            noise[sfb] = Some(noise_nrg);
            continue;
        }
        if let Some(p) = *prev_sf {
            sf = sf.clamp(p - MAX_SF_DELTA, p + MAX_SF_DELTA).clamp(0, 255);
        }
        // Raise sf until the band's peak fits the ESC ceiling (a
        // +4 step scales magnitudes by 2^(-3/4)).
        let mut qmax = quantize_coef(peak, sf).abs();
        while qmax >= MAX_QUANT && sf < 255 {
            sf = (sf + 4).min(255);
            qmax = quantize_coef(peak, sf).abs();
        }
        if qmax == 0 {
            continue; // all-zero band -> ZERO_HCB, no scalefactor
        }
        let mut band_max = 0i32;
        for k in start..end {
            let q = quantize_coef(spec[k], sf);
            x_quant[k] = q;
            band_max = band_max.max(q.abs());
        }
        if band_max == 0 {
            continue;
        }
        sfb_cb[sfb] = codebook_for(band_max);
        sfs[sfb] = Some(sf);
        *prev_sf = Some(sf);
    }
    GroupQuant {
        x_quant,
        sfb_cb,
        sfs,
        noise,
        is_pos: vec![None; num_swb],
    }
}

/// Rewrite the right channel's IS-selected bands (§4.6.8.2 encode
/// side): the band's codebook becomes the transmitted intensity book
/// (15 in-phase / 14 out-of-phase), its coefficients are dropped
/// (intensity bands carry no spectral data — the decoder derives
/// them from the left channel), its spectrum scalefactor is retired,
/// and the intensity position lands on the §4.6.8.1.4 `is_pos`
/// track.
fn apply_is_overrides(group: &mut GroupQuant, is_bands: &[IsBand], offsets: &[u16]) {
    for (sfb, band) in is_bands.iter().enumerate().take(group.sfb_cb.len()) {
        let Some((cb, pos)) = band else {
            continue;
        };
        let start = offsets[sfb] as usize;
        let end = (offsets[sfb + 1] as usize).min(group.x_quant.len());
        for q in &mut group.x_quant[start..end] {
            *q = 0;
        }
        group.sfb_cb[sfb] = *cb;
        group.sfs[sfb] = None;
        group.noise[sfb] = None;
        group.is_pos[sfb] = Some(*pos);
    }
}

/// §4.6.13 noise-likeness test on the `(Σ|x|)² / (width·Σx²)`
/// density statistic (see [`PNS_DENSITY_MIN`]): `true` only when the
/// band's energy is spread across most of its coefficients the way a
/// dense noise band's is. Bands narrower than [`PNS_MIN_WIDTH`]
/// never qualify (the statistic is meaningless on a handful of
/// coefficients).
fn is_noise_like(band: &[f64]) -> bool {
    let width = band.len();
    if width < PNS_MIN_WIDTH {
        return false;
    }
    let l1: f64 = band.iter().map(|&v| v.abs()).sum();
    let l2_sq: f64 = band.iter().map(|&v| v * v).sum();
    if l2_sq <= 0.0 {
        return false;
    }
    (l1 * l1) / (width as f64 * l2_sq) > PNS_DENSITY_MIN
}

/// Build the per-band absolute scalefactor / noise-energy records
/// and the frame's `global_gain`, then run the §4.6.2.3.2 / §4.6.13
/// inverse DPCM ([`differentiate`]) to obtain the transmitted entry
/// set.
///
/// `global_gain` is the first coded spectrum band's scalefactor
/// (making its delta 0). The §4.6.13 noise track is seeded at
/// `global_gain − NOISE_OFFSET − 256` with the first PNS band's
/// delta a 9-bit *unsigned* PCM (`0..=511`) and later noise deltas
/// Huffman `±60`; each requested `noise_nrg` is clamped into the
/// nearest feasible value on that track (a few 1.5 dB steps of
/// clamp at worst — noise energy is far less sensitive than a
/// spectral gain).
fn scalefactor_track(groups: &[GroupQuant]) -> (u8, AbsoluteScaleFactors) {
    let global_gain = groups
        .iter()
        .flat_map(|g| g.sfs.iter().copied().flatten())
        .next()
        .unwrap_or(SF_OFFSET) as u8;
    let mut last_nrg = i32::from(global_gain) - NOISE_OFFSET - 256;
    let mut first_noise = true;
    // §4.6.8.1.4: the intensity-position track seeds at 0 and takes
    // the same Huffman ±60 deltas as scalefactors; requested
    // positions are clamped onto the feasible track like the noise
    // energies above.
    let mut last_is = 0i32;
    let mut entries = Vec::with_capacity(groups.len());
    for g in groups {
        let mut group_out = Vec::new();
        for sfb in 0..g.sfb_cb.len() {
            if let Some(sf) = g.sfs[sfb] {
                group_out.push(AbsoluteScaleFactorEntry::Sf(sf as u8));
            } else if let Some(nrg) = g.noise[sfb] {
                let delta = nrg - last_nrg;
                let clamped = if first_noise {
                    delta.clamp(0, 511)
                } else {
                    delta.clamp(-MAX_SF_DELTA, MAX_SF_DELTA)
                };
                first_noise = false;
                last_nrg += clamped;
                group_out.push(AbsoluteScaleFactorEntry::NoiseNrg(last_nrg));
            } else if let Some(pos) = g.is_pos[sfb] {
                let delta = (pos - last_is).clamp(-MAX_SF_DELTA, MAX_SF_DELTA);
                last_is += delta;
                group_out.push(AbsoluteScaleFactorEntry::IsPos(last_is as i16));
            }
        }
        entries.push(group_out);
    }
    (global_gain, AbsoluteScaleFactors { entries })
}

/// Exact §4.6.3.3 wire cost, in bits, of coding one band's
/// coefficient range with spectrum book `cb` — Huffman codewords +
/// sign bits + escape sequences, measured by running the actual
/// [`crate::spectral_data`] tuple writer into a scratch buffer.
/// `None` when the book cannot carry the band (a magnitude beyond
/// the book's Table 4.95 LAV; book 11 escapes up to `MAX_QUANT`).
fn band_bits(cb: u8, coeffs: &[i32]) -> Option<u32> {
    let row = crate::spectral_codebook::table_4_95(cb).ok()?;
    let dim = row.dimension? as usize;
    let mut bw = BitWriter::new();
    let mut k = 0;
    while k + dim <= coeffs.len() {
        crate::spectral_data::write_tuple(&mut bw, cb, dim, &coeffs[k..k + dim]).ok()?;
        k += dim;
    }
    if k != coeffs.len() {
        return None; // band width not a whole number of tuples
    }
    Some(bw.bit_position() as u32)
}

/// The §4.4.2.7-adjacent `section_data()` header cost of one section
/// spanning `len` bands: 4 bits `sect_cb` plus the `sect_len_incr`
/// escape run (5-bit fields / escape 31 for long sequences, 3-bit /
/// escape 7 for `EIGHT_SHORT`).
fn section_header_bits(len: u32, long: bool) -> u32 {
    let (esc, w) = if long { (31, 5) } else { (7, 3) };
    4 + w * (len / esc + 1)
}

/// A band's sectioning class — sections may only span bands of one
/// class (the special codebooks are semantic, not a coding choice,
/// and a `ZERO_HCB` band folded into a spectrum section would owe a
/// scalefactor the track never assigned).
#[derive(PartialEq, Eq, Clone, Copy)]
enum BandClass {
    /// `ZERO_HCB` — no spectrum, no scalefactor.
    Zero,
    /// `NOISE_HCB` / intensity books — the codebook is fixed by the
    /// tool decision; adjacent equal books merge.
    Fixed(u8),
    /// Spectrum bands (provisional book 1..=11) — the section book
    /// is a free choice among every book that covers the run.
    Spectral,
}

/// Choose one window group's sections + codebooks by measured bit
/// cost (§4.6.3.1 leaves both entirely to the encoder).
///
/// Dynamic program over section boundaries: for every candidate run
/// of same-class bands the cost is the [`section_header_bits`]
/// overhead plus — for spectral runs — the cheapest single Table
/// 4.95 book (1..=11, measured per band via [`band_bits`], covering
/// the whole run) summed over the run's bands. This subsumes the
/// classic "smallest LAV fit + merge equal books" rule and beats it
/// wherever a signed/unsigned sibling book codes the actual
/// distribution cheaper, or one step up in LAV lets two sections
/// merge for less than the saved header.
///
/// `ranges` maps each band to its coefficient range inside the
/// group's (interleaved) buffer; `sfb_cb` carries the per-band class
/// in (provisional books on spectral bands) and the chosen books
/// out.
fn optimize_group_sections(
    x_quant: &[i32],
    ranges: &[(usize, usize)],
    sfb_cb: &mut [u8],
    long: bool,
) -> Result<Vec<Section>> {
    let n = sfb_cb.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    let class: Vec<BandClass> = sfb_cb
        .iter()
        .map(|&cb| match cb {
            ZERO_HCB => BandClass::Zero,
            NOISE_HCB | INTENSITY_HCB | INTENSITY_HCB2 => BandClass::Fixed(cb),
            _ => BandClass::Spectral,
        })
        .collect();
    // Per-band cost under each spectrum book (None = book can't
    // carry the band).
    let books: [u8; 11] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
    let cost: Vec<[Option<u32>; 11]> = (0..n)
        .map(|b| {
            let mut row = [None; 11];
            if class[b] == BandClass::Spectral {
                let (s, e) = ranges[b];
                for (i, &cb) in books.iter().enumerate() {
                    row[i] = band_bits(cb, &x_quant[s..e]);
                }
            }
            row
        })
        .collect();

    // dp[b] = (bits, cut, book) for the cheapest sectioning of bands
    // 0..b, where `cut` is the start of the final section and `book`
    // its codebook.
    let mut dp: Vec<(u64, usize, u8)> = vec![(u64::MAX, 0, 0); n + 1];
    dp[0] = (0, 0, 0);
    for b in 1..=n {
        for a in (0..b).rev() {
            // The run a..b must be one class (and one fixed book).
            if class[a] != class[b - 1] {
                break;
            }
            let header = u64::from(section_header_bits((b - a) as u32, long));
            let run = match class[a] {
                BandClass::Zero => Some((0u8, 0u64)),
                BandClass::Fixed(cb) => {
                    if sfb_cb[a..b].iter().any(|&c| c != cb) {
                        None // e.g. mixed intensity phases
                    } else {
                        Some((cb, 0))
                    }
                }
                BandClass::Spectral => {
                    let mut best: Option<(u8, u64)> = None;
                    for (i, &cb) in books.iter().enumerate() {
                        let mut sum = 0u64;
                        let mut ok = true;
                        for c in cost[a..b].iter() {
                            match c[i] {
                                Some(bits) => sum += u64::from(bits),
                                None => {
                                    ok = false;
                                    break;
                                }
                            }
                        }
                        if ok && best.map(|(_, s)| sum < s).unwrap_or(true) {
                            best = Some((cb, sum));
                        }
                    }
                    best
                }
            };
            let Some((book, run_bits)) = run else {
                continue;
            };
            let total = dp[a].0.saturating_add(header + run_bits);
            if total < dp[b].0 {
                dp[b] = (total, a, book);
            }
        }
    }
    if dp[n].0 == u64::MAX {
        return Err(Error::SpectralDataEncodeInvalid);
    }

    // Walk the cuts back into sections and stamp the chosen books.
    let mut bounds = Vec::new();
    let mut b = n;
    while b > 0 {
        let (_, a, book) = dp[b];
        bounds.push((a, b, book));
        b = a;
    }
    bounds.reverse();
    let mut sections = Vec::with_capacity(bounds.len());
    for (a, b, book) in bounds {
        for cb in sfb_cb[a..b].iter_mut() {
            *cb = book;
        }
        sections.push(Section {
            codebook: book,
            start: a as u8,
            end: b as u8,
        });
    }
    Ok(sections)
}

/// Wrap quantized groups + an `ics_info` into the wire record set.
/// The per-group band ranges come from the §4.5.2.3.4
/// `sect_sfb_offset` derivation, so grouped short-window buffers
/// (band widths × `window_group_length`) resolve correctly.
fn finish_channel(
    info: IcsInfo,
    groups: Vec<GroupQuant>,
    fs_index: u8,
    pulse_data: Option<PulseData>,
) -> Result<QuantizedChannel> {
    let (global_gain, abs) = scalefactor_track(&groups);
    let long = info.window_sequence != WindowSequence::EightShort;
    let per_group_offsets = crate::spectral_data::sect_sfb_offset(&info, fs_index)?;
    let mut sections = Vec::with_capacity(groups.len());
    let mut sfb_cb = Vec::with_capacity(groups.len());
    let mut x_quant = Vec::with_capacity(groups.len());
    for (mut g, offsets) in groups.into_iter().zip(per_group_offsets.iter()) {
        let ranges: Vec<(usize, usize)> = (0..g.sfb_cb.len())
            .map(|sfb| {
                (
                    (offsets[sfb] as usize).min(g.x_quant.len()),
                    (offsets[sfb + 1] as usize).min(g.x_quant.len()),
                )
            })
            .collect();
        sections.push(optimize_group_sections(
            &g.x_quant,
            &ranges,
            &mut g.sfb_cb,
            long,
        )?);
        sfb_cb.push(g.sfb_cb);
        x_quant.push(g.x_quant);
    }
    let scale_factor_data = differentiate(&abs, &sfb_cb, global_gain)?;
    let body = IcsBody {
        global_gain,
        ics_info: Some(info.clone()),
        section_data: SectionData { sections, sfb_cb },
        scale_factor_data,
        pulse_data_present: pulse_data.is_some(),
        pulse_data,
        tns_data_present: false,
        tns_data: None,
        gain_control_data_present: false,
        gain_control_data: None,
        spectral_data_bit_offset: 0,
        er_scale_factor_data: None,
        reordered_spectral_lengths: None,
    };
    let spectral = SpectralData { x_quant };
    Ok(QuantizedChannel {
        info,
        body,
        spectral,
    })
}

/// Exact wire size, in bits, of one channel's
/// `individual_channel_stream()` — the [`IcsBody`] side info
/// (sections, scalefactors, pulse / TNS dispatch) plus the
/// `spectral_data()` payload — measured by running the real writers
/// into a scratch buffer. Used to settle encoder tool decisions
/// (pulse escape) by measured cost, the same philosophy as
/// [`optimize_group_sections`].
fn channel_wire_bits(chan: &QuantizedChannel, fs_index: u8) -> Result<u64> {
    let mut bw = BitWriter::new();
    chan.body.write(&mut bw, 2, fs_index, false)?;
    chan.spectral
        .write(&mut bw, &chan.info, &chan.body.section_data, fs_index)?;
    Ok(bw.bit_position())
}

/// Wire cost, in bits, of one Table 4.7 pulse record carrying `n`
/// pulses: 2-bit `number_pulse` + 6-bit `pulse_start_sfb` + 9 bits
/// per `(offset, amp)` entry (the `pulse_data_present` dispatch bit
/// itself is paid on both variants).
fn pulse_record_bits(n: usize) -> u64 {
    8 + 9 * n as u64
}

/// §4.4.6.3 / Table 4.7 pulse-escape candidate for one long-frame
/// quantized spectrum.
///
/// The pulse tool pays off when a band's few outlier lines force the
/// whole band (and through sectioning, its neighbours) onto a large-
/// LAV codebook or into §4.6.3.3 escape sequences: transmitting the
/// outliers' excess as `(offset, amp)` fix-ups lets the residual
/// band code on the book the *rest* of its lines need. The decoder's
/// §4.6.3.3 reconstruction ([`crate::swb_offset::apply_pulse_data`],
/// `x_quant[k] ±= amp` on the transmitted sign) restores the exact
/// original quantized values, so the choice is purely one of
/// noiseless-coding cost.
///
/// Candidate search, per spectrum-coded band (`ZERO_HCB` bands
/// transmit no scalefactor and `NOISE_HCB` / intensity bands no
/// coefficients — excluded): for every outlier count `j` in
/// `1..=`[`MAX_PULSES`], reduce the band's `j` largest-magnitude
/// lines to the magnitude floor set by its `(j+1)`-th largest
/// (clamped to the 4-bit `amp` reach, keeping the residual >= 1 so
/// the transmitted sign survives), price the reduced band at its
/// cheapest Table 4.95 book via [`band_bits`] plus the
/// [`pulse_record_bits`] overhead, and keep the best-measuring `j`.
/// The best band across the spectrum wins (Table 4.7's single
/// `pulse_start_sfb` + 5-bit offset deltas make one band the
/// realistic carrier; cross-band chains are almost never
/// addressable). Pulses must ascend with in-band gaps <= 31 and the
/// first offset within 31 of the band start.
///
/// Returns the pulse record and the reduced spectrum, or `None` when
/// no band measures a saving. The caller re-prices the whole channel
/// with [`channel_wire_bits`] (capturing section-merge effects) and
/// keeps the variant only when the full stream measures smaller.
fn extract_pulse_candidate(
    x_quant: &[i32],
    sfb_cb: &[u8],
    offsets: &[u16],
) -> Option<(PulseData, Vec<i32>)> {
    // (measured saving, carrier sfb, [(line index, amp)]).
    #[allow(clippy::type_complexity)]
    let mut best: Option<(i64, usize, Vec<(usize, i32)>)> = None;
    for sfb in 0..sfb_cb.len().min(offsets.len().saturating_sub(1)).min(64) {
        match sfb_cb[sfb] {
            ZERO_HCB | NOISE_HCB | INTENSITY_HCB | INTENSITY_HCB2 => continue,
            _ => {}
        }
        let (s, e) = (
            offsets[sfb] as usize,
            (offsets[sfb + 1] as usize).min(x_quant.len()),
        );
        if e <= s {
            continue;
        }
        let band = &x_quant[s..e];
        // Baseline: the band's cheapest book as-is.
        let Some(base_cost) = cheapest_band_bits(band) else {
            continue;
        };
        // Magnitude-descending line order.
        let mut by_mag: Vec<usize> = (0..band.len()).collect();
        by_mag.sort_by_key(|&i| std::cmp::Reverse(band[i].abs()));
        for j in 1..=MAX_PULSES.min(band.len().saturating_sub(1)) {
            let floor = band[by_mag[j]].abs().max(1);
            // The j selected lines, ascending, with their amp.
            let mut lines: Vec<(usize, i32)> = by_mag[..j]
                .iter()
                .map(|&i| (i, (band[i].abs() - floor).min(15)))
                .filter(|&(_, amp)| amp >= 1)
                .collect();
            if lines.len() < j {
                continue; // an outlier is out of amp reach parity with the floor
            }
            lines.sort_by_key(|&(i, _)| i);
            // Table 4.7 addressability.
            if lines[0].0 > 0x1f {
                continue;
            }
            if lines.windows(2).any(|w| w[1].0 - w[0].0 > 0x1f) {
                continue;
            }
            let mut reduced = band.to_vec();
            for &(i, amp) in &lines {
                if reduced[i] > 0 {
                    reduced[i] -= amp;
                } else {
                    reduced[i] += amp;
                }
            }
            let Some(cost) = cheapest_band_bits(&reduced) else {
                continue;
            };
            let saving = i64::from(base_cost) - i64::from(cost) - pulse_record_bits(j) as i64;
            if saving > 0 && best.as_ref().map(|(bs, _, _)| saving > *bs).unwrap_or(true) {
                best = Some((
                    saving,
                    sfb,
                    lines.iter().map(|&(i, amp)| (s + i, amp)).collect(),
                ));
            }
        }
    }
    let (_, start_sfb, lines) = best?;
    let mut reduced = x_quant.to_vec();
    let mut pulses = Vec::with_capacity(lines.len());
    let mut prev_k = offsets[start_sfb] as usize;
    for &(k, amp) in &lines {
        pulses.push(Pulse {
            offset: (k - prev_k) as u8,
            amp: amp as u8,
        });
        prev_k = k;
        if reduced[k] > 0 {
            reduced[k] -= amp;
        } else {
            reduced[k] += amp;
        }
    }
    Some((
        PulseData {
            pulse_start_sfb: start_sfb as u8,
            pulses,
        },
        reduced,
    ))
}

/// Cheapest single Table 4.95 book cost for one band's coefficients
/// (books 1..=11 via [`band_bits`]).
fn cheapest_band_bits(coeffs: &[i32]) -> Option<u32> {
    (1u8..=11).filter_map(|cb| band_bits(cb, coeffs)).min()
}

/// Quantize one channel's 1024-line long-sequence spectrum into a
/// complete `individual_channel_stream()` record set. `seq` must be
/// one of the three long sequences (it lands in the `ics_info`);
/// `frame_peak` anchors the masking spread and cull — the channel's
/// own peak for mono, the pair's loudest peak for a jointly-coded
/// CPE. `pns_bands` grants the per-band §4.6.13 allowance (empty =
/// PNS off); a non-empty `is_bands` (the right channel of an
/// intensity-coding CPE) rewrites the selected bands into §4.6.8.2
/// intensity records after quantization. When the spectrum carries
/// escape-magnitude lines, a §4.4.6.3 `pulse_data()` variant is
/// priced against the plain coding and kept if it measures smaller
/// (the decode-side §4.6.3.3 fix-up restores the identical quantized
/// spectrum, so the choice never changes the reconstruction).
#[allow(clippy::too_many_arguments)]
fn quantize_channel(
    spec: &[f64],
    seq: WindowSequence,
    fs_index: u8,
    sf_offset: i32,
    frame_peak: f64,
    pns_bands: &[bool],
    is_bands: &[IsBand],
) -> Result<QuantizedChannel> {
    debug_assert_eq!(spec.len(), FRAME_LEN);
    debug_assert!(seq != WindowSequence::EightShort);
    let offsets = long_window_offsets(fs_index)?;
    let num_swb = NUM_SWB_LONG_WINDOW[fs_index as usize] as usize;
    let mut prev_sf: Option<i32> = None;
    let mut group = quantize_group(
        spec,
        offsets,
        num_swb,
        sf_offset,
        frame_peak,
        &mut prev_sf,
        pns_bands,
    );
    if !is_bands.is_empty() {
        apply_is_overrides(&mut group, is_bands, offsets);
    }
    let pulse_candidate = extract_pulse_candidate(&group.x_quant, &group.sfb_cb, offsets);
    let info = IcsInfo {
        family: crate::swb_offset::FrameFamily::Lc1024,
        ics_reserved_bit: false,
        window_sequence: seq,
        window_shape: WindowShape::Sine,
        max_sfb: num_swb as u8,
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
        num_swb: num_swb as u8,
    };
    // Measured pulse decision: price the reduced-spectrum +
    // pulse_data() variant against the plain coding with the real
    // writers and keep the smaller stream.
    if let Some((pd, reduced)) = pulse_candidate {
        let mut pulsed_group = GroupQuant {
            x_quant: reduced,
            sfb_cb: group.sfb_cb.clone(),
            sfs: group.sfs.clone(),
            noise: group.noise.clone(),
            is_pos: group.is_pos.clone(),
        };
        // Re-derive the reduced bands' provisional books (the DP
        // re-decides anyway; this keeps the class metadata honest).
        for sfb in 0..pulsed_group.sfb_cb.len() {
            match pulsed_group.sfb_cb[sfb] {
                ZERO_HCB | NOISE_HCB | INTENSITY_HCB | INTENSITY_HCB2 => continue,
                _ => {}
            }
            let (s, e) = (
                offsets[sfb] as usize,
                (offsets[sfb + 1] as usize).min(pulsed_group.x_quant.len()),
            );
            let band_max = pulsed_group.x_quant[s..e]
                .iter()
                .map(|q| q.abs())
                .max()
                .unwrap_or(0);
            if band_max > 0 {
                pulsed_group.sfb_cb[sfb] = codebook_for(band_max);
            }
        }
        let plain = finish_channel(info.clone(), vec![group], fs_index, None)?;
        let pulsed = finish_channel(info, vec![pulsed_group], fs_index, Some(pd))?;
        return if channel_wire_bits(&pulsed, fs_index)? < channel_wire_bits(&plain, fs_index)? {
            Ok(pulsed)
        } else {
            Ok(plain)
        };
    }
    finish_channel(info, vec![group], fs_index, None)
}

/// Maximum mean per-band log-energy distance (natural log) between
/// two adjacent short windows for the §4.5.2.3.4 grouping decision
/// to merge them into one window group. `ln 4 ≈ 1.39` — the windows'
/// band envelopes agree within ~6 dB on average.
const GROUP_MERGE_LOG_DIST: f64 = 1.386;

/// Energy floor (one squared unit coefficient) added to both sides
/// of the grouping log-ratio so empty bands compare as equal instead
/// of dividing by zero.
const GROUP_MERGE_EPS: f64 = 1.0;

/// §4.5.2.3.4 `scale_factor_grouping` decision for one channel's
/// `EIGHT_SHORT_SEQUENCE` spectrum (8 × 128 window-major
/// coefficients): merge adjacent windows whose per-band energy
/// envelopes agree within [`GROUP_MERGE_LOG_DIST`] on average.
///
/// Grouped windows share one scalefactor / section track — the whole
/// point of the tool (§4.6.2.3.2: "to achieve a most efficient
/// coding, several subsequent windows... can be grouped") — so the
/// merge criterion mirrors what sharing costs: windows with matching
/// band envelopes lose nothing to a common scalefactor, while an
/// attack window's jump keeps it in its own group. Returns the
/// `window_group_length` vector (summing to 8).
fn decide_short_grouping(spec: &[f64], offsets: &[u16], num_swb: usize) -> Vec<u8> {
    let short_len = SHORT_WINDOW_LEN as usize;
    let band_energy = |w: usize, sfb: usize| -> f64 {
        let base = w * short_len;
        spec[base + offsets[sfb] as usize..base + offsets[sfb + 1] as usize]
            .iter()
            .map(|&v| v * v)
            .sum()
    };
    let mut lengths: Vec<u8> = vec![1];
    for w in 1..8 {
        let dist: f64 = (0..num_swb)
            .map(|sfb| {
                let a = band_energy(w - 1, sfb) + GROUP_MERGE_EPS;
                let b = band_energy(w, sfb) + GROUP_MERGE_EPS;
                (a / b).ln().abs()
            })
            .sum::<f64>()
            / num_swb.max(1) as f64;
        if dist <= GROUP_MERGE_LOG_DIST {
            *lengths.last_mut().expect("non-empty") += 1;
        } else {
            lengths.push(1);
        }
    }
    lengths
}

/// The 7-bit `scale_factor_grouping` mask for a `window_group_length`
/// vector: bit `6 − (w − 1)` is set when window `w` (1..=7) stays in
/// the previous window's group — the inverse of the §4.5.2.3.4
/// derivation in [`crate::ics_info::derive_window_grouping`].
fn grouping_mask(window_group_length: &[u8]) -> u8 {
    let mut mask = 0u8;
    let mut w = 0usize;
    for &len in window_group_length {
        for j in 0..len as usize {
            if j > 0 {
                mask |= 1 << (6 - (w - 1));
            }
            w += 1;
        }
    }
    mask
}

/// Quantize one channel's `EIGHT_SHORT_SEQUENCE` spectrum (8 x 128
/// window-major coefficients) into a complete record set. The
/// §4.5.2.3.4 grouping decision ([`decide_short_grouping`]) merges
/// envelope-alike adjacent windows into shared window groups — one
/// scalefactor / section track per group instead of eight — and each
/// group's coefficients are laid out in the §4.5.2.3.5 interleaved
/// `(sfb, window, bin)` transmission order.
fn quantize_channel_short(
    spec: &[f64],
    fs_index: u8,
    sf_offset: i32,
    frame_peak: f64,
) -> Result<QuantizedChannel> {
    let offsets = short_window_offsets(fs_index)?;
    let num_swb = NUM_SWB_SHORT_WINDOW[fs_index as usize] as usize;
    let window_group_length = decide_short_grouping(spec, offsets, num_swb);
    quantize_channel_short_grouped(spec, fs_index, sf_offset, frame_peak, window_group_length)
}

/// [`quantize_channel_short`] under an *imposed* grouping — the
/// `common_window` CPE path must quantize both channels under one
/// shared `ics_info`, so the §4.5.2.3.4 grouping decision is made
/// once (jointly, on the pair envelope) and both channels' section /
/// scalefactor / interleave layouts follow it.
fn quantize_channel_short_grouped(
    spec: &[f64],
    fs_index: u8,
    sf_offset: i32,
    frame_peak: f64,
    window_group_length: Vec<u8>,
) -> Result<QuantizedChannel> {
    let short_len = SHORT_WINDOW_LEN as usize;
    debug_assert_eq!(spec.len(), 8 * short_len);
    let offsets = short_window_offsets(fs_index)?;
    let num_swb = NUM_SWB_SHORT_WINDOW[fs_index as usize] as usize;
    let mask = grouping_mask(&window_group_length);
    let mut prev_sf: Option<i32> = None;
    let mut groups = Vec::with_capacity(window_group_length.len());
    let mut win_base = 0usize;
    for &len in &window_group_length {
        let wgl = len as usize;
        // §4.5.2.3.5 interleave: for each band, the group's windows'
        // band coefficients ride consecutively.
        let mut buf = Vec::with_capacity(wgl * short_len);
        for sfb in 0..num_swb {
            let (s, e) = (offsets[sfb] as usize, offsets[sfb + 1] as usize);
            for w in 0..wgl {
                let base = (win_base + w) * short_len;
                buf.extend_from_slice(&spec[base + s..base + e]);
            }
        }
        // The group's sect_sfb_offset table: band widths × wgl.
        let mut scaled = Vec::with_capacity(num_swb + 1);
        let mut acc = 0u16;
        scaled.push(0u16);
        for sfb in 0..num_swb {
            acc += (offsets[sfb + 1] - offsets[sfb]) * len as u16;
            scaled.push(acc);
        }
        groups.push(quantize_group(
            &buf,
            &scaled,
            num_swb,
            sf_offset,
            frame_peak,
            &mut prev_sf,
            &[], // PNS stays long-frame-only for now
        ));
        win_base += wgl;
    }
    let info = IcsInfo {
        family: crate::swb_offset::FrameFamily::Lc1024,
        ics_reserved_bit: false,
        window_sequence: WindowSequence::EightShort,
        window_shape: WindowShape::Sine,
        max_sfb: num_swb as u8,
        scale_factor_grouping: Some(mask),
        predictor_data_present: false,
        predictor_data: None,
        ltp_data_present: false,
        ltp_data: None,
        ltp_data_present_pair: None,
        ltp_data_pair: None,
        num_windows: 8,
        num_window_groups: window_group_length.len() as u8,
        window_group_length,
        num_swb: num_swb as u8,
    };
    // §4.4.6.3: pulse_data is illegal on EIGHT_SHORT_SEQUENCE.
    finish_channel(info, groups, fs_index, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dequant::{inverse_quantize, scale_factor_gain};

    #[test]
    fn quantize_coef_inverts_dequant_within_rounding() {
        // For any q and sf, dequantizing then re-quantizing recovers
        // q exactly (the quantizer is the exact inverse map).
        for sf in [40i32, 100, 156, 200] {
            for q in [-8190i32, -1000, -12, -1, 0, 1, 7, 40, 999, 8190] {
                let x = inverse_quantize(q) * scale_factor_gain(sf as u8);
                assert_eq!(quantize_coef(x, sf), q, "sf={sf} q={q}");
            }
        }
    }

    #[test]
    fn band_scalefactor_hits_target_magnitude() {
        // A frame-loudest peak quantized with its own
        // band_scalefactor lands within rounding of TARGET_PEAK_MAG.
        for peak in [1.0f64, 100.0, 3.2e4, 6.7e7] {
            let sf = band_scalefactor(peak, peak, 0).expect("loudest band never culls");
            let q = quantize_coef(peak, sf).abs();
            let lo = (TARGET_PEAK_MAG / 2.0_f64.powf(0.375)).floor() as i32;
            let hi = (TARGET_PEAK_MAG * 2.0_f64.powf(0.375)).ceil() as i32;
            assert!(
                (lo..=hi).contains(&q),
                "peak={peak} sf={sf} q={q} not in [{lo},{hi}]"
            );
        }
    }

    #[test]
    fn band_scalefactor_spreads_and_culls() {
        let frame_peak = 1.0e6f64;
        // A band 40 dB down gets a ~20 dB smaller target: the target
        // is 42·(10^-2)^0.5 = 4.2, so its peak quantizes to ~4.
        let sf = band_scalefactor(frame_peak * 1e-2, frame_peak, 0).unwrap();
        let q = quantize_coef(frame_peak * 1e-2, sf).abs();
        assert!((2..=8).contains(&q), "spread target off: q={q}");
        // A band ~90 dB down is culled outright (target < 0.7).
        assert_eq!(band_scalefactor(frame_peak * 3.2e-5, frame_peak, 0), None);
        // Zero-peak bands cull.
        assert_eq!(band_scalefactor(0.0, frame_peak, 0), None);
    }

    #[test]
    fn codebook_selection_covers_table_4_95_lavs() {
        assert_eq!(codebook_for(0), ZERO_HCB);
        assert_eq!(codebook_for(1), 1);
        assert_eq!(codebook_for(2), 3);
        assert_eq!(codebook_for(4), 5);
        assert_eq!(codebook_for(7), 7);
        assert_eq!(codebook_for(12), 9);
        assert_eq!(codebook_for(13), 11);
        assert_eq!(codebook_for(8191), 11);
    }

    #[test]
    fn config_rejects_bad_parameters() {
        assert!(StreamEncoder::new(EncoderConfig {
            sample_rate: 44_100,
            channels: 1,
            bitrate: 64_000,
        })
        .is_ok());
        assert!(matches!(
            StreamEncoder::new(EncoderConfig {
                sample_rate: 44_056, // not a Table 1.18 rate
                channels: 1,
                bitrate: 64_000,
            }),
            Err(Error::EncoderInvalidConfig)
        ));
        // Every channel count with a Table 1.19 default
        // configuration builds; 0 / 7 / 9 have none and reject.
        for ok in [3u8, 4, 5, 6, 8] {
            assert!(
                StreamEncoder::new(EncoderConfig {
                    sample_rate: 44_100,
                    channels: ok,
                    bitrate: 64_000 * u32::from(ok),
                })
                .is_ok(),
                "channels {ok}"
            );
        }
        for bad in [0u8, 7, 9] {
            assert!(
                matches!(
                    StreamEncoder::new(EncoderConfig {
                        sample_rate: 44_100,
                        channels: bad,
                        bitrate: 64_000,
                    }),
                    Err(Error::EncoderInvalidConfig)
                ),
                "channels {bad}"
            );
        }
        assert!(matches!(
            StreamEncoder::new(EncoderConfig {
                sample_rate: 44_100,
                channels: 1,
                bitrate: 0,
            }),
            Err(Error::EncoderInvalidConfig)
        ));
    }

    /// Deterministic band fill for the sectioning tests.
    fn fill_band(buf: &mut [i32], range: (usize, usize), max: i32, seed: &mut u32) {
        for slot in buf[range.0..range.1].iter_mut() {
            *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            *slot = ((*seed >> 8) % (2 * max + 1) as u32) as i32 - max;
        }
    }

    /// Total wire bits of one long group under given sections /
    /// books: `section_data()` + `spectral_data()`, measured with
    /// the real writers.
    fn measure_group(
        sections: Vec<Section>,
        sfb_cb: Vec<u8>,
        x_quant: Vec<i32>,
        num_swb: usize,
        fs_index: u8,
    ) -> u64 {
        let info = IcsInfo {
            family: crate::swb_offset::FrameFamily::Lc1024,
            ics_reserved_bit: false,
            window_sequence: WindowSequence::OnlyLong,
            window_shape: WindowShape::Sine,
            max_sfb: num_swb as u8,
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
            num_swb: num_swb as u8,
        };
        let sd = SectionData {
            sections: vec![sections],
            sfb_cb: vec![sfb_cb],
        };
        let spectral = SpectralData {
            x_quant: vec![x_quant],
        };
        let mut bw = BitWriter::new();
        sd.write(&mut bw, WindowSequence::OnlyLong, num_swb as u8)
            .unwrap();
        spectral.write(&mut bw, &info, &sd, fs_index).unwrap();
        bw.bit_position()
    }

    /// [`band_bits`] agrees with the real `spectral_data()` writer:
    /// a one-section stream's spectral bits equal the summed band
    /// costs.
    #[test]
    fn band_bits_matches_wire_writer() {
        let fs_index = 4u8;
        let offsets = long_window_offsets(fs_index).unwrap();
        let mut buf = vec![0i32; FRAME_LEN];
        let mut seed = 0xB17u32;
        for sfb in 0..6 {
            fill_band(
                &mut buf,
                (offsets[sfb] as usize, offsets[sfb + 1] as usize),
                7,
                &mut seed,
            );
        }
        for cb in [7u8, 8, 9, 10, 11] {
            let per_band: u32 = (0..6)
                .map(|sfb| {
                    band_bits(cb, &buf[offsets[sfb] as usize..offsets[sfb + 1] as usize]).unwrap()
                })
                .sum();
            let sections = vec![Section {
                codebook: cb,
                start: 0,
                end: 6,
            }];
            let wire = measure_group(sections.clone(), vec![cb; 6], buf.clone(), 6, fs_index);
            let header = u64::from(section_header_bits(6, true));
            assert_eq!(wire, header + u64::from(per_band), "cb {cb}");
        }
        // A signed book rejects magnitudes past its LAV; the quad
        // books reject a pair-only width mismatch never (widths are
        // multiples of 4), but LAV 1 caps at |1|.
        assert!(band_bits(1, &[2, 0, 0, 0]).is_none());
        assert!(band_bits(3, &[3, 0, 0, 0]).is_none());
    }

    /// The measured-cost DP never codes a group larger than the
    /// classic smallest-LAV + merge-equal-books sectioning, over a
    /// spread of band shapes (zero runs, alternating magnitudes,
    /// escape bands).
    #[test]
    fn optimizer_never_loses_to_naive_sections() {
        let fs_index = 4u8;
        let offsets = long_window_offsets(fs_index).unwrap();
        let num_swb = NUM_SWB_LONG_WINDOW[fs_index as usize] as usize;
        for (case, seed0) in [(0u32, 1u32), (1, 0xACE), (2, 0x5EED), (3, 77)] {
            let mut buf = vec![0i32; FRAME_LEN];
            let mut seed = seed0;
            for sfb in 0..num_swb {
                let range = (offsets[sfb] as usize, offsets[sfb + 1] as usize);
                let max = match case {
                    0 => [0, 1, 1, 2, 0, 0, 4, 7, 1][sfb % 9],
                    1 => [1, 12, 1, 30, 0, 2][sfb % 6],
                    2 => (sfb as i32) % 5,
                    _ => [7, 7, 0, 0, 0, 12, 1, 1][sfb % 8],
                };
                if max > 0 {
                    fill_band(&mut buf, range, max, &mut seed);
                }
            }
            // Provisional per-band books (the DP input).
            let provisional: Vec<u8> = (0..num_swb)
                .map(|sfb| {
                    let band = &buf[offsets[sfb] as usize..offsets[sfb + 1] as usize];
                    codebook_for(band.iter().map(|&v| v.abs()).max().unwrap_or(0))
                })
                .collect();
            // Naive: keep the smallest-LAV books, merge equal runs.
            let mut naive_sections: Vec<Section> = Vec::new();
            for (sfb, &cb) in provisional.iter().enumerate() {
                match naive_sections.last_mut() {
                    Some(s) if s.codebook == cb => s.end = (sfb + 1) as u8,
                    _ => naive_sections.push(Section {
                        codebook: cb,
                        start: sfb as u8,
                        end: (sfb + 1) as u8,
                    }),
                }
            }
            let naive = measure_group(
                naive_sections,
                provisional.clone(),
                buf.clone(),
                num_swb,
                fs_index,
            );
            // Optimized.
            let ranges: Vec<(usize, usize)> = (0..num_swb)
                .map(|sfb| (offsets[sfb] as usize, offsets[sfb + 1] as usize))
                .collect();
            let mut books = provisional;
            let sections = optimize_group_sections(&buf, &ranges, &mut books, true).unwrap();
            // Every chosen book covers its bands (the writer would
            // reject otherwise) and the wire is never larger.
            let opt = measure_group(sections, books, buf, num_swb, fs_index);
            assert!(opt <= naive, "case {case}: opt {opt} > naive {naive}");
        }
    }

    /// [`decide_short_grouping`] merges alike windows and splits at
    /// an attack; [`grouping_mask`] is the exact inverse of the
    /// §4.5.2.3.4 mask derivation.
    #[test]
    fn short_grouping_decision_and_mask() {
        let fs_index = 4u8;
        let offsets = short_window_offsets(fs_index).unwrap();
        let num_swb = NUM_SWB_SHORT_WINDOW[fs_index as usize] as usize;
        let short_len = SHORT_WINDOW_LEN as usize;
        // Eight identical windows: one group of 8, mask all-ones.
        let mut spec = vec![0.0f64; 8 * short_len];
        for w in 0..8 {
            for k in 0..short_len {
                spec[w * short_len + k] = 1000.0 * ((k as f64) * 0.37).sin();
            }
        }
        assert_eq!(decide_short_grouping(&spec, offsets, num_swb), vec![8]);
        assert_eq!(grouping_mask(&[8]), 0x7F);
        // A 60 dB attack at window 3 splits the run there.
        for k in 0..short_len {
            for w in 3..8 {
                spec[w * short_len + k] *= 1000.0;
            }
        }
        let lengths = decide_short_grouping(&spec, offsets, num_swb);
        assert_eq!(lengths, vec![3, 5]);
        assert_eq!(grouping_mask(&lengths), 0b110_1111);
        // No grouping at all round-trips to mask 0.
        assert_eq!(grouping_mask(&[1; 8]), 0);
        // Every mask agrees with the decoder-side derivation.
        for lengths in [vec![8u8], vec![3, 5], vec![1; 8], vec![2, 1, 4, 1]] {
            let mask = grouping_mask(&lengths);
            let (_, n, derived, _) = crate::ics_info::derive_window_grouping(
                WindowSequence::EightShort,
                Some(mask),
                fs_index as usize,
            );
            assert_eq!(derived, lengths);
            assert_eq!(n as usize, lengths.len());
        }
    }

    /// A grouped short channel's wire records round-trip through the
    /// crate's own parsers: the ics_info grouping, the per-group
    /// section spans, and the §4.5.2.3.5 interleaved spectrum come
    /// back exactly.
    #[test]
    fn short_grouping_wire_roundtrip() {
        use oxideav_core::bits::BitReader;
        let fs_index = 4u8;
        let short_len = SHORT_WINDOW_LEN as usize;
        // Windows 0..3 carry pattern A, 3..8 a 40 dB louder pattern B
        // (the grouping decision splits at the jump).
        let mut spec = vec![0.0f64; 8 * short_len];
        for w in 0..8 {
            let (gain, phase) = if w < 3 {
                (300.0, 0.31)
            } else {
                (30000.0, 0.11)
            };
            for k in 0..short_len {
                spec[w * short_len + k] = gain * ((k as f64) * phase).sin();
            }
        }
        let frame_peak = spec.iter().fold(0.0f64, |m, &v| m.max(v.abs()));
        let chan = quantize_channel_short(&spec, fs_index, 0, frame_peak).unwrap();
        assert_eq!(chan.info.window_group_length, vec![3, 5]);

        let mut bw = BitWriter::new();
        chan.body.write(&mut bw, 2, fs_index, false).unwrap();
        chan.spectral
            .write(&mut bw, &chan.info, &chan.body.section_data, fs_index)
            .unwrap();
        let bytes = bw.finish();
        let mut reader = BitReader::new(&bytes);
        let body = IcsBody::parse(&mut reader, 2, fs_index, false).unwrap();
        let ics = body.ics_info.as_ref().unwrap();
        assert_eq!(ics.scale_factor_grouping, Some(0b110_1111));
        assert_eq!(ics.num_window_groups, 2);
        assert_eq!(ics.window_group_length, vec![3, 5]);
        let spectral = SpectralData::parse(&mut reader, ics, &body.section_data, fs_index).unwrap();
        assert_eq!(spectral, chan.spectral);
        assert_eq!(body.section_data, chan.body.section_data);
        assert_eq!(body.scale_factor_data, chan.body.scale_factor_data);
    }

    /// The `common_window` CPE short path imposes ONE grouping on
    /// both channels even when their independent decisions would
    /// diverge — a divergent pair would desync the shared-`ics_info`
    /// wire layout (verified: reverting to per-channel decisions
    /// fails this test). The frame must decode consistently through
    /// the stream decoder with the burst energy on the right
    /// channels.
    #[test]
    fn cpe_short_grouping_is_joint() {
        let fs_index = 4u8;
        let short_len = SHORT_WINDOW_LEN as usize;
        let offsets = short_window_offsets(fs_index).unwrap();
        let num_swb = NUM_SWB_SHORT_WINDOW[fs_index as usize] as usize;
        // L jumps 60 dB at window 2, R at window 5: the independent
        // groupings differ (the premise of the regression).
        let mut l_spec = vec![0.0f64; 8 * short_len];
        let mut r_spec = vec![0.0f64; 8 * short_len];
        for w in 0..8 {
            for k in 0..short_len {
                let l_gain = if w < 2 { 30.0 } else { 30000.0 };
                let r_gain = if w < 5 { 30.0 } else { 30000.0 };
                l_spec[w * short_len + k] = l_gain * ((k as f64) * 0.23).sin();
                r_spec[w * short_len + k] = r_gain * ((k as f64) * 0.19).sin();
            }
        }
        let gl = decide_short_grouping(&l_spec, offsets, num_swb);
        let gr = decide_short_grouping(&r_spec, offsets, num_swb);
        assert_ne!(gl, gr, "premise: independent groupings diverge");

        // Encode a stereo stream engineered to hit EIGHT_SHORT with
        // per-channel attacks at different windows, and decode it
        // with the crate's own decoder — a desynced CPE would fail
        // to parse (or reconstruct garbage).
        let n = 4 * FRAME_LEN;
        let mut pcm = Vec::with_capacity(n * 2);
        for i in 0..n {
            let t = i as f64;
            let in_l = (FRAME_LEN + 256..FRAME_LEN + 640).contains(&i);
            let in_r = (FRAME_LEN + 640..FRAME_LEN + 1024).contains(&i);
            let base = 400.0 * (0.09 * t).sin();
            let l = base + if in_l { 20000.0 * (0.5 * t).sin() } else { 0.0 };
            let r = base
                + if in_r {
                    20000.0 * (0.43 * t).sin()
                } else {
                    0.0
                };
            pcm.push(l.round() as i16);
            pcm.push(r.round() as i16);
        }
        let mut enc = StreamEncoder::new(EncoderConfig {
            sample_rate: 44_100,
            channels: 2,
            bitrate: 192_000,
        })
        .unwrap();
        let stream = enc.encode_all(&pcm).unwrap();
        let mut dec = crate::decode::StreamDecoder::new();
        let frames = dec.decode_all(&stream).unwrap();
        assert!(frames.len() >= 4);
        // The burst energy must come back on the right channels.
        let mut decoded = Vec::new();
        for f in &frames {
            decoded.extend_from_slice(&f.pcm);
        }
        let aligned = &decoded[FRAME_LEN * 2..];
        let window_energy = |c: usize, range: core::ops::Range<usize>| -> f64 {
            range.map(|i| f64::from(aligned[i * 2 + c]).powi(2)).sum()
        };
        let l_burst = window_energy(0, FRAME_LEN + 256..FRAME_LEN + 640);
        let r_quiet = window_energy(1, FRAME_LEN + 256..FRAME_LEN + 640);
        assert!(
            l_burst > 20.0 * r_quiet,
            "left burst region not reconstructed: {l_burst:.0} vs {r_quiet:.0}"
        );
    }

    /// Short-frame M/S: identical (and phase-inverted) channels
    /// flag every `(group, sfb)` cell, independent channels flag
    /// almost nothing, and an identical-channel transient stream
    /// decodes with L exactly equal to R (all-M/S ⇒ `s ≡ 0` ⇒ the
    /// de-matrix reproduces one channel twice).
    #[test]
    fn cpe_short_frame_ms_coding() {
        let fs_index = 4u8;
        let short_len = SHORT_WINDOW_LEN as usize;
        let offsets = short_window_offsets(fs_index).unwrap();
        let num_swb = NUM_SWB_SHORT_WINDOW[fs_index as usize] as usize;
        let mut spec = vec![0.0f64; 8 * short_len];
        let mut seed = 0x515u32;
        for v in spec.iter_mut() {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            *v = f64::from(seed >> 16) - 32768.0;
        }
        let inverted: Vec<f64> = spec.iter().map(|&v| -v).collect();
        let wgl = vec![2u8, 3, 3];
        let same = ms_decide_short(&spec, &spec, offsets, num_swb, &wgl);
        assert!(same.iter().flatten().all(|&b| b), "identical pair all-M/S");
        let anti = ms_decide_short(&spec, &inverted, offsets, num_swb, &wgl);
        assert!(anti.iter().flatten().all(|&b| b), "inverted pair all-M/S");
        let mut other = vec![0.0f64; 8 * short_len];
        for v in other.iter_mut() {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            *v = f64::from(seed >> 16) - 32768.0;
        }
        let indep = ms_decide_short(&spec, &other, offsets, num_swb, &wgl);
        let flagged = indep.iter().flatten().filter(|&&b| b).count();
        let total = indep.iter().flatten().count();
        assert!(
            flagged * 4 < total,
            "independent pair flagged {flagged}/{total}"
        );
        // apply ∘ decide on the identical pair zeroes the side chain.
        let (code_l, code_r) = apply_ms_short(&spec, &spec, &same, offsets, &wgl);
        assert_eq!(code_l, spec);
        assert!(code_r.iter().all(|&v| v == 0.0));

        // End to end: identical channels with a percussive burst
        // (short frames engaged) decode to L == R exactly.
        let n = 4 * FRAME_LEN;
        let mut pcm = Vec::with_capacity(n * 2);
        for i in 0..n {
            let t = i as f64;
            let burst = (FRAME_LEN + 256..FRAME_LEN + 640).contains(&i);
            let v = 500.0 * (0.07 * t).sin()
                + if burst {
                    18000.0 * (0.6 * t).sin()
                } else {
                    0.0
                };
            let s = v.round() as i16;
            pcm.push(s);
            pcm.push(s);
        }
        let mut enc = StreamEncoder::new(EncoderConfig {
            sample_rate: 44_100,
            channels: 2,
            bitrate: 128_000,
        })
        .unwrap();
        let stream = enc.encode_all(&pcm).unwrap();
        let mut dec = crate::decode::StreamDecoder::new();
        let frames = dec.decode_all(&stream).unwrap();
        for (f, frame) in frames.iter().enumerate() {
            for i in 0..(frame.pcm.len() / 2) {
                assert_eq!(
                    frame.pcm[2 * i],
                    frame.pcm[2 * i + 1],
                    "frame {f} sample {i}: identical channels must decode identical"
                );
            }
        }
    }

    #[test]
    fn silent_input_yields_valid_minimal_frames() {
        let mut enc = StreamEncoder::new(EncoderConfig {
            sample_rate: 44_100,
            channels: 1,
            bitrate: 64_000,
        })
        .unwrap();
        let stream = enc.encode_all(&[0i16; FRAME_LEN]).unwrap();
        // Two frames (content + flush), each parseable.
        let (h0, off) = AdtsHeader::parse(&stream).unwrap();
        assert_eq!(h0.channel_configuration, 1);
        assert_eq!(off, ADTS_HEADER_BYTES_NO_CRC);
        let second = &stream[h0.aac_frame_length as usize..];
        let (h1, _) = AdtsHeader::parse(second).unwrap();
        assert_eq!(
            h0.aac_frame_length as usize + h1.aac_frame_length as usize,
            stream.len()
        );
    }

    /// [`extract_pulse_candidate`] on a band with outlier lines: the
    /// reduced spectrum plus the §4.6.3.3 fix-up
    /// ([`crate::swb_offset::apply_pulse_data`]) restores the exact
    /// original quantized values, and the measured per-band saving
    /// is real (the reduced band's cheapest book costs at least the
    /// pulse record less).
    #[test]
    fn pulse_candidate_reduction_is_exactly_invertible() {
        let fs_index = 3u8; // 48 kHz
        let offsets = long_window_offsets(fs_index).unwrap();
        let num_swb = NUM_SWB_LONG_WINDOW[fs_index as usize] as usize;
        let mut x_quant = vec![0i32; FRAME_LEN];
        let mut sfb_cb = vec![ZERO_HCB; num_swb];
        // Band 29 (32 lines at 48 kHz): a dense small-magnitude
        // spectrum with two outliers (one negative) — the outliers
        // alone force the whole band onto the ESC book.
        let (s, e) = (offsets[29] as usize, offsets[30] as usize);
        for (k, slot) in x_quant.iter_mut().enumerate().take(e).skip(s) {
            *slot = 1 - ((k as i32) & 2);
        }
        x_quant[s + 1] = 17;
        x_quant[s + 5] = -19;
        sfb_cb[29] = codebook_for(19);
        let (pd, reduced) =
            extract_pulse_candidate(&x_quant, &sfb_cb, offsets).expect("outliers must qualify");
        assert_eq!(pd.pulse_start_sfb, 29);
        assert_eq!(pd.pulses.len(), 2);
        // The residual codes without the outliers' book demand
        // (amp reach is 15, so 17 → 2 and −19 → −4, signs kept).
        assert_eq!(reduced[s + 1], 2);
        assert_eq!(reduced[s + 5], -4);
        // Decode-side fix-up restores the original spectrum exactly.
        let mut restored = reduced.clone();
        crate::swb_offset::apply_pulse_data(&mut restored, fs_index, &pd).unwrap();
        assert_eq!(restored, x_quant);
        // The candidate's saving is real end to end.
        let plain_bits = cheapest_band_bits(&x_quant[s..e]).unwrap() as u64;
        let pulsed_bits =
            cheapest_band_bits(&reduced[s..e]).unwrap() as u64 + pulse_record_bits(pd.pulses.len());
        assert!(
            pulsed_bits < plain_bits,
            "pulsed {pulsed_bits} vs plain {plain_bits}"
        );
    }

    /// A spectrum-wide no-outlier profile yields no candidate, and a
    /// candidate that does not measure smaller is dropped by the
    /// [`quantize_channel`] decision (the emitted stream stays
    /// pulse-free on flat content).
    #[test]
    fn pulse_candidate_requires_a_measured_win() {
        let fs_index = 3u8;
        let offsets = long_window_offsets(fs_index).unwrap();
        let num_swb = NUM_SWB_LONG_WINDOW[fs_index as usize] as usize;
        // Flat band: every line the same magnitude — no outlier.
        let mut x_quant = vec![0i32; FRAME_LEN];
        let mut sfb_cb = vec![ZERO_HCB; num_swb];
        let (s, e) = (offsets[10] as usize, offsets[11] as usize);
        x_quant[s..e].fill(3);
        sfb_cb[10] = codebook_for(3);
        assert!(extract_pulse_candidate(&x_quant, &sfb_cb, offsets).is_none());
    }

    /// End-to-end pulse emission through [`quantize_channel`]: a
    /// long-frame spectrum whose loud band carries one outlier line
    /// over a low floor selects the pulse variant, the wire record
    /// round-trips, and the §4.6.3.3 fix-up on the transmitted
    /// spectrum reproduces the pulse-free quantization exactly (the
    /// reconstruction is bit-identical by construction).
    #[test]
    fn quantize_channel_emits_measured_pulse_data() {
        let fs_index = 3u8; // 48 kHz
        let offsets = long_window_offsets(fs_index).unwrap();
        // Craft a spectrum: wide band 29 carries a moderate peak
        // (the frame peak lives in band 20 so band 29's masking
        // target lands near the escape threshold) plus a dense low
        // floor across the band.
        let mut spec = vec![0.0f64; FRAME_LEN];
        let (s29, e29) = (offsets[29] as usize, offsets[30] as usize);
        spec[offsets[20] as usize] = 1.0; // frame peak, own band
        spec[s29 + 3] = 0.17; // the outlier line
        for (k, slot) in spec.iter_mut().enumerate().take(e29).skip(s29) {
            if k != s29 + 3 {
                *slot = 0.0095; // the band floor
            }
        }
        let chan =
            quantize_channel(&spec, WindowSequence::OnlyLong, fs_index, 0, 1.0, &[], &[]).unwrap();
        let plain = quantize_group(
            &spec,
            offsets,
            NUM_SWB_LONG_WINDOW[fs_index as usize] as usize,
            0,
            1.0,
            &mut None,
            &[],
        );
        assert!(
            chan.body.pulse_data_present,
            "outlier-over-floor band must select the pulse variant"
        );
        let pd = chan.body.pulse_data.as_ref().unwrap();
        assert!((1..=MAX_PULSES).contains(&pd.pulses.len()));
        // The transmitted spectrum restores to the plain quantization.
        let mut restored = chan.spectral.x_quant[0].clone();
        crate::swb_offset::apply_pulse_data(&mut restored, fs_index, pd).unwrap();
        assert_eq!(restored, plain.x_quant);
        // And the pulse variant is the smaller stream.
        let plain_chan = {
            let info = chan.info.clone();
            finish_channel(info, vec![plain], fs_index, None).unwrap()
        };
        assert!(
            channel_wire_bits(&chan, fs_index).unwrap()
                < channel_wire_bits(&plain_chan, fs_index).unwrap()
        );
    }
}

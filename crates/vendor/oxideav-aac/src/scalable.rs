//! Scalable AAC — ISO/IEC 14496-3 §4.4.2.2 (Tables 4.13–4.18) syntax
//! and the §4.5.2.2 / §4.6.14.2 AAC-only layer-combination decode for
//! the AAC scalable (AOT 6) and ER AAC scalable (AOT 20) object types.
//!
//! ## Payload shape
//!
//! A scalable program is one `aac_scalable_main_element()` (ASME,
//! Table 4.13 — layer 0) plus up to seven
//! `aac_scalable_extension_element()`s (ASEE, Table 4.14 — layers
//! 1..8), each riding its own elementary stream / LATM layer. Every
//! element is a header followed by one `individual_channel_stream(1,1)`
//! per channel (the Table 4.50 `scale_flag == 1` form: no inline
//! `ics_info()`, no pulse / TNS / gain-control dispatch — see
//! [`IcsBody::parse_scale`]), a trailing `extension_payload()` loop and
//! `byte_alignment()`.
//!
//! * `aac_scalable_main_header()` (Table 4.15, the AAC-only branch —
//!   `core_flag == 0`, `tvq_layer_present == 0`): `ics_reserved_bit`,
//!   `window_sequence`, `window_shape`, `max_sfb` (+
//!   `scale_factor_grouping` on `EIGHT_SHORT_SEQUENCE`), the stereo
//!   `ms_mask_present` / `ms_data()`, then per channel
//!   `tns_data_present` / `tns_data()` and `ltp_data_present` /
//!   `ltp_data()`.
//! * `aac_scalable_extension_header()` (Table 4.16): `max_sfb`, the
//!   stereo `ms_mask_present` / `ms_data()` (Table 4.60 — transmitted
//!   for the **additional** bands `last_max_sfb_ms..max_sfb` only,
//!   §4.6.8.1.4), per-channel `tns_data_present` / `tns_data()` on the
//!   *first stereo layer after mono layers* only (`mono_stereo_flag`,
//!   §4.6.9.5), and per-channel `diff_control_data_lr()` (Table 4.18)
//!   on every stereo layer of a mixed mono/stereo configuration.
//!
//! ## Layer combination (§4.5.2.2.4, Figure 4.4)
//!
//! The Scalable Inverse AAC Quantization module (SIAQ) adds the
//! dequantized spectra of all layers per output path; the per-band
//! tool interactions follow Tables 4.91–4.93:
//!
//! * mono→mono / stereo→stereo plain bands: **sum**;
//! * PNS bands: a lower layer's noise band survives only while every
//!   higher layer decodes the band to all-zero (§4.6.13.6); a higher
//!   layer's PNS **replaces** a lower PNS band; PNS on top of real
//!   coefficients (and vice versa within a channel pair) is invalid;
//! * intensity bands: only the left/mid channel accumulates across
//!   IS→IS layers, positions come from the highest layer; IS over a
//!   plain stereo band (or plain over IS) replaces the band with the
//!   highest layer's content per Table 4.92;
//! * at the mono→stereo transition the combined mono spectrum `M''`
//!   enters M/S-coded bands as `M = M'' + M'` and L/R-coded bands via
//!   the §4.6.14.2 FSS: `L/R += 2·M''` where the per-channel
//!   `diff_control_lr` bit is `0` (untransmitted bands default to
//!   `1`); a mono PNS band never crosses the transition (Table 4.93).
//!
//! M/S (§4.6.8.1.4: one cumulative mask across layers), intensity
//! (§4.6.8.2.3 — `invert_intensity() = +1` for the scalable AOT) and
//! PNS (§4.6.13.6 — `ms_used` still signals noise correlation) are
//! then applied on the combined spectra, followed by the §4.6.9.5
//! serial TNS layout (Table 4.158: the first mono layer's filter data
//! serves the `M` region up to the highest mono `max_sfb`, the first
//! stereo layer's filters serve L / R; an L/R filter reaching below
//! the mono boundary overrides the M filter) and the §4.6.11
//! filterbank.
//!
//! §4.6.7.5 LTP: prediction runs only on the lowest GA layer, its
//! reconstruction history is the time-domain output of the first
//! layer decoded **alone** — the driver keeps a parallel base-layer
//! synthesis chain for exactly that; intensity / PNS bands of the
//! base layer take precedence over prediction (§4.6.7.5 / §4.6.7.4.2).
//!
//! CELP-core (`dependsOnCoreCoder == 1`) and TwinVQ lower layers are
//! other subparts' codecs and are rejected
//! ([`Error::ScalableUnsupportedCore`]).

use oxideav_core::bits::{BitReader, BitWriter};

use crate::asc::AacResilienceFlags;
use crate::decoded_spectrum::quant_to_spec;
use crate::dequant::rescale_spectrum;
use crate::extension_payload::ExtensionPayload;
use crate::filterbank::Filterbank;
use crate::ics_body::IcsBody;
use crate::ics_info::{
    derive_window_grouping_family, parse_ltp_data, write_ltp_data, IcsInfo, LtpData,
    WindowSequence, WindowShape,
};
use crate::intensity_stereo::{apply_intensity_stereo, IntensityPairSpectra};
use crate::ltp::LtpState;
use crate::ms_stereo::{apply_ms_stereo, ChannelPairSpectra, MsMaskPresent};
use crate::pns::{apply_pns, apply_pns_pair, gen_rand_vector, PnsChannel};
use crate::scale_factor_data::{accumulate, AbsoluteScaleFactors};
use crate::section_data::{INTENSITY_HCB, INTENSITY_HCB2, NOISE_HCB};
use crate::spectral_data::SpectralData;
use crate::swb_offset::FrameFamily;
use crate::tns_data::TnsData;
use crate::tns_frame::{tns_analysis_frame_ics, tns_decode_frame_ics};
use crate::{Error, Result};

/// Maximum number of coding layers (§4.5.2.2.4: one AAC main layer
/// plus up to 7 AAC extension layers).
pub const MAX_LAYERS: usize = 8;

/// Static configuration of a scalable program, resolved from the
/// per-layer `AudioSpecificConfig`s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalableConfig {
    /// `audioObjectType` — 6 (AAC scalable) or 20 (ER AAC scalable).
    pub aot: u8,
    /// Table 1.18 `samplingFrequencyIndex` (all layers share it in
    /// the AAC-only combinations — §4.5.2.2.4 runs one filterbank).
    pub fs_index: u8,
    /// Resolved sampling rate in Hz.
    pub sample_rate: u32,
    /// §4.5.1.1 frame-length family — `Lc1024` or `Lc960`
    /// (`frameLengthFlag`); the LD families are not scalable shapes.
    pub family: FrameFamily,
    /// The ASC resilience triplet for AOT 20; all-false for AOT 6.
    pub resilience: AacResilienceFlags,
    /// `this_layer_stereo` per layer, in layer order (§4.5.2.2.1.1).
    /// Derived from each layer's `channelConfiguration` (1 or 2).
    pub layer_stereo: Vec<bool>,
}

impl ScalableConfig {
    /// Validate the §4.5.2.2 shape: 1..=8 layers, no mono layer after
    /// a stereo layer (Table 4.87), a non-LD family, a scalable AOT.
    pub fn validate(&self) -> Result<()> {
        if self.aot != 6 && self.aot != 20 {
            return Err(Error::ScalableInvalid);
        }
        if self.layer_stereo.is_empty() || self.layer_stereo.len() > MAX_LAYERS {
            return Err(Error::ScalableInvalid);
        }
        if self.family.is_ld() {
            return Err(Error::ScalableInvalid);
        }
        // Table 4.87: AAC mono may feed mono or stereo; AAC stereo
        // feeds stereo only.
        let mut seen_stereo = false;
        for &s in &self.layer_stereo {
            if seen_stereo && !s {
                return Err(Error::ScalableInvalid);
            }
            seen_stereo |= s;
        }
        Ok(())
    }

    /// `mono_layer_flag` (§4.5.2.2.1.1): any mono layer present.
    pub fn mono_layer_flag(&self) -> bool {
        self.layer_stereo.iter().any(|&s| !s)
    }

    /// Index of the first stereo layer, if any.
    pub fn first_stereo_layer(&self) -> Option<usize> {
        self.layer_stereo.iter().position(|&s| s)
    }

    /// `mono_stereo_flag` for layer `lay` (§4.5.2.2.1.1): at least one
    /// mono layer exists and `lay` is the first stereo layer.
    pub fn mono_stereo_flag(&self, lay: usize) -> bool {
        self.mono_layer_flag() && self.first_stereo_layer() == Some(lay)
    }

    /// Build a [`ScalableConfig`] from the per-layer
    /// `AudioSpecificConfig`s of a LATM program (§1.7.3: one layer per
    /// `streamID[prog][lay]`, in layer order).
    ///
    /// Shape rules enforced here: every layer carries the same
    /// scalable AOT (6 / 20), the same `samplingFrequencyIndex` and
    /// the same `frameLengthFlag`; each `channelConfiguration` is 1
    /// (mono) or 2 (stereo); a `dependsOnCoreCoder == 1` layer (CELP
    /// core, §4.5.2.2.5) is rejected with
    /// [`Error::ScalableUnsupportedCore`]; the AOT-20 resilience
    /// triplet comes from the first layer and must match on every
    /// layer.
    pub fn from_layer_ascs(ascs: &[&crate::asc::AudioSpecificConfig]) -> Result<Self> {
        let first = ascs.first().ok_or(Error::ScalableInvalid)?;
        if first.aot != 6 && first.aot != 20 {
            return Err(Error::ScalableInvalid);
        }
        let family = FrameFamily::from_aot_and_flag(
            first.aot,
            first.ga_body.frame_length == crate::asc::FrameLength::Long960,
        );
        let resilience = |asc: &crate::asc::AudioSpecificConfig| {
            asc.ga_body
                .extension_body
                .as_ref()
                .and_then(|ext| ext.resilience)
                .unwrap_or_default()
        };
        let res0 = resilience(first);
        let mut layer_stereo = Vec::with_capacity(ascs.len());
        for asc in ascs {
            if asc.aot != first.aot
                || asc.sampling_frequency_index != first.sampling_frequency_index
                || asc.ga_body.frame_length != first.ga_body.frame_length
                || resilience(asc) != res0
            {
                return Err(Error::ScalableInvalid);
            }
            if asc.ga_body.depends_on_core_coder {
                return Err(Error::ScalableUnsupportedCore);
            }
            layer_stereo.push(match asc.channel_configuration {
                1 => false,
                2 => true,
                _ => return Err(Error::ScalableInvalid),
            });
        }
        let cfg = ScalableConfig {
            aot: first.aot,
            fs_index: first.sampling_frequency_index,
            sample_rate: first.sample_rate,
            family,
            resilience: res0,
            layer_stereo,
        };
        cfg.validate()?;
        Ok(cfg)
    }

    /// Number of output channels (2 iff any layer is stereo).
    pub fn output_channels(&self) -> usize {
        if self.layer_stereo.iter().any(|&s| s) {
            2
        } else {
            1
        }
    }

    fn channels_of_layer(&self, lay: usize) -> usize {
        if self.layer_stereo[lay] {
            2
        } else {
            1
        }
    }
}

/// One channel of one layer: the `individual_channel_stream(1,1)`
/// body plus its decoded spectrum.
#[derive(Debug, Clone)]
pub struct ScalableChannel {
    /// The Table 4.50 `scale_flag == 1` body
    /// ([`IcsBody::parse_scale`]).
    pub body: IcsBody,
    /// The channel's quantized spectrum — from `spectral_data()` or,
    /// for AOT 20 with `aacSpectralDataResilienceFlag`, from the
    /// §4.6.16.3 `reordered_spectral_data()` payload.
    pub spectral: SpectralData,
}

/// One parsed layer of a scalable frame (main or extension element).
#[derive(Debug, Clone)]
pub struct ScalableLayer {
    /// Per-layer geometry: the main header's `window_sequence` /
    /// `window_shape` / grouping with **this layer's** `max_sfb`.
    pub ics: IcsInfo,
    /// `ms_mask_present` for a stereo layer ([`MsMaskPresent::AllZeros`]
    /// for mono layers, whose headers carry no mask).
    pub ms_mask_present: MsMaskPresent,
    /// The layer's newly transmitted `ms_used` rows (Table 4.60):
    /// `num_window_groups` rows covering `last_max_sfb_ms..max_sfb`.
    /// Empty unless `ms_mask_present == 1`.
    pub ms_used_new: Vec<Vec<bool>>,
    /// Per-channel `tns_data()`; populated only on layers whose header
    /// carries TNS bits (the main layer; the `mono_stereo_flag`
    /// extension layer).
    pub tns: Vec<Option<TnsData>>,
    /// Per-channel `ltp_data()` (main layer only, §4.6.7.5).
    pub ltp: Vec<Option<LtpData>>,
    /// Per-channel long-window `diff_control_lr` bits in transmission
    /// order (Table 4.18: bands `last_max_sfb_ms..min(last_mono_max_sfb,
    /// max_sfb)` whose cumulative `ms_used` is clear). Empty when the
    /// header carries none.
    pub diff_lr_long: Vec<Vec<bool>>,
    /// Per-channel short-window `diff_control_lr[win][0]` bits (first
    /// stereo layer only). `None` when absent.
    pub diff_lr_short: Vec<Option<[bool; 8]>>,
    /// The per-channel ICS bodies + spectra.
    pub channels: Vec<ScalableChannel>,
}

/// A fully parsed scalable frame: every layer element plus the
/// cumulative cross-layer tables.
#[derive(Debug, Clone)]
pub struct ScalableFrame {
    /// The per-layer parsed elements, in layer order.
    pub layers: Vec<ScalableLayer>,
    /// Cumulative `ms_used[g][sfb]` over `max_total_sfb` bands
    /// (§4.6.8.1.4 — one mask across all layers, each layer
    /// transmitting only its additional bands).
    pub ms_used: Vec<Vec<bool>>,
    /// Cumulative per-channel long-window `diff_control_lr[sfb]`
    /// (§4.6.14.2.1; `None` = untransmitted = `1`).
    pub diff_lr_long: [Vec<Option<bool>>; 2],
    /// Per-channel short-window `diff_control_lr[win][0]` (first
    /// stereo layer; `None` when the frame is long-window or has no
    /// stereo transition).
    pub diff_lr_short: [Option<[bool; 8]>; 2],
    /// Highest `max_sfb` across all layers.
    pub max_total_sfb: u8,
    /// Highest `max_sfb` across the mono layers (`0` when none).
    pub max_mono_sfb: u8,
}

impl ScalableFrame {
    /// Parse one frame's per-layer payloads (one byte buffer per
    /// layer, layer 0 first) into the element structures.
    pub fn parse(cfg: &ScalableConfig, payloads: &[&[u8]]) -> Result<Self> {
        cfg.validate()?;
        if payloads.len() != cfg.layer_stereo.len() {
            return Err(Error::ScalableInvalid);
        }

        let mut layers: Vec<ScalableLayer> = Vec::with_capacity(payloads.len());
        // Base geometry from the main header (window sequence / shape /
        // grouping are frame-global; only max_sfb varies per layer).
        let mut base_ics: Option<IcsInfo> = None;
        let mut ms_used: Vec<Vec<bool>> = Vec::new();
        let mut diff_lr_long: [Vec<Option<bool>>; 2] = [Vec::new(), Vec::new()];
        let mut diff_lr_short: [Option<[bool; 8]>; 2] = [None, None];
        let mut last_max_sfb_ms: u8 = 0; // previous *stereo* layer's max_sfb
        let mut max_mono_sfb: u8 = 0;
        let mut max_total_sfb: u8 = 0;

        for (lay, payload) in payloads.iter().enumerate() {
            let stereo = cfg.layer_stereo[lay];
            let n_ch = cfg.channels_of_layer(lay);
            let mut reader = BitReader::new(payload);

            let (ics, ms_mask_present, ms_used_new, tns, ltp, dl_long, dl_short);
            if lay == 0 {
                // ---- Table 4.15 aac_scalable_main_header() (AAC-only).
                let ics_reserved_bit = read_bit(&mut reader)?;
                let ws = WindowSequence::from_bits(read_u8(&mut reader, 2)?);
                let shape = WindowShape::from_bit(read_bit(&mut reader)?);
                let (msfb, sfg) = if ws.is_eight_short() {
                    let m = read_u8(&mut reader, 4)?;
                    let g = read_u8(&mut reader, 7)?;
                    (m, Some(g))
                } else {
                    (read_u8(&mut reader, 6)?, None)
                };
                let (num_windows, num_window_groups, window_group_length, num_swb) =
                    derive_window_grouping_family(cfg.family, ws, sfg, cfg.fs_index)?;
                let info = IcsInfo {
                    family: cfg.family,
                    ics_reserved_bit,
                    window_sequence: ws,
                    window_shape: shape,
                    max_sfb: msfb,
                    scale_factor_grouping: sfg,
                    predictor_data_present: false,
                    predictor_data: None,
                    ltp_data_present: false,
                    ltp_data: None,
                    ltp_data_present_pair: None,
                    ltp_data_pair: None,
                    num_windows,
                    num_window_groups,
                    window_group_length,
                    num_swb,
                };
                if msfb > num_swb {
                    return Err(Error::ScalableInvalid);
                }
                let groups = usize::from(num_window_groups);
                ms_used = vec![Vec::new(); groups];

                let (mask, new_rows) = if stereo {
                    parse_ms_data(&mut reader, groups, 0, msfb)?
                } else {
                    (MsMaskPresent::AllZeros, Vec::new())
                };
                merge_ms_rows(&mut ms_used, mask, &new_rows, 0, msfb);

                // Note: `mono_stereo_flag` cannot be set on the main
                // layer of an AAC-only configuration (a stereo main
                // layer means no mono layer exists), so the
                // `tns_channel_mono_layer` bit never occurs here.
                let mut tns_v: Vec<Option<TnsData>> = Vec::with_capacity(n_ch);
                let mut ltp_v: Vec<Option<LtpData>> = Vec::with_capacity(n_ch);
                for _ch in 0..n_ch {
                    // Table 4.15 per-channel loop: TNS then (AAC-only
                    // branch) LTP.
                    if read_bit(&mut reader)? {
                        tns_v.push(Some(TnsData::parse(&mut reader, ws)?));
                    } else {
                        tns_v.push(None);
                    }
                    if read_bit(&mut reader)? {
                        ltp_v.push(Some(parse_ltp_data(&mut reader, cfg.aot, ws, msfb)?));
                    } else {
                        ltp_v.push(None);
                    }
                }
                ics = info;
                ms_mask_present = mask;
                ms_used_new = new_rows;
                tns = tns_v;
                ltp = ltp_v;
                dl_long = Vec::new();
                dl_short = vec![None; n_ch];
                base_ics = Some(ics.clone());
            } else {
                // ---- Table 4.16 aac_scalable_extension_header().
                let base = base_ics.as_ref().ok_or(Error::ScalableInvalid)?;
                let ws = base.window_sequence;
                let msfb = if ws.is_eight_short() {
                    read_u8(&mut reader, 4)?
                } else {
                    read_u8(&mut reader, 6)?
                };
                if msfb > base.num_swb {
                    return Err(Error::ScalableInvalid);
                }
                let groups = usize::from(base.num_window_groups);
                let (mask, new_rows) = if stereo {
                    parse_ms_data(&mut reader, groups, last_max_sfb_ms, msfb)?
                } else {
                    (MsMaskPresent::AllZeros, Vec::new())
                };
                merge_ms_rows(&mut ms_used, mask, &new_rows, last_max_sfb_ms, msfb);

                let tns_v: Vec<Option<TnsData>> = if cfg.mono_stereo_flag(lay) {
                    let mut v = Vec::with_capacity(2);
                    for _ch in 0..2 {
                        if read_bit(&mut reader)? {
                            v.push(Some(TnsData::parse(&mut reader, ws)?));
                        } else {
                            v.push(None);
                        }
                    }
                    v
                } else {
                    vec![None; n_ch]
                };

                // Table 4.18 diff_control_data_lr(), one per channel.
                let mut dl_long_v: Vec<Vec<bool>> = Vec::new();
                let mut dl_short_v: Vec<Option<[bool; 8]>> = vec![None; n_ch];
                if cfg.mono_layer_flag() && stereo {
                    for ch in 0..2usize {
                        if ws != WindowSequence::EightShort {
                            let hi = core::cmp::min(max_mono_sfb, msfb);
                            let mut bits = Vec::new();
                            for sfb in last_max_sfb_ms..hi {
                                let on = ms_used
                                    .first()
                                    .and_then(|row| row.get(usize::from(sfb)))
                                    .copied()
                                    .unwrap_or(false);
                                if !on {
                                    let b = read_bit(&mut reader)?;
                                    bits.push(b);
                                    if usize::from(sfb) >= diff_lr_long[ch].len() {
                                        diff_lr_long[ch].resize(usize::from(sfb) + 1, None);
                                    }
                                    diff_lr_long[ch][usize::from(sfb)] = Some(b);
                                }
                            }
                            dl_long_v.push(bits);
                        } else {
                            dl_long_v.push(Vec::new());
                            if last_max_sfb_ms == 0 {
                                // Only in the first stereo layer.
                                let mut w = [false; 8];
                                for slot in w.iter_mut() {
                                    *slot = read_bit(&mut reader)?;
                                }
                                dl_short_v[ch] = Some(w);
                                diff_lr_short[ch] = Some(w);
                            }
                        }
                    }
                }

                let mut info = base.clone();
                info.max_sfb = msfb;
                ics = info;
                ms_mask_present = mask;
                ms_used_new = new_rows;
                tns = tns_v;
                ltp = vec![None; n_ch];
                dl_long = dl_long_v;
                dl_short = dl_short_v;
            }

            // ---- Per-channel individual_channel_stream(1, 1).
            let mut channels: Vec<ScalableChannel> = Vec::with_capacity(n_ch);
            for _ch in 0..n_ch {
                let body = IcsBody::parse_scale(&mut reader, &ics, cfg.resilience)?;
                let spectral = if cfg.resilience.spectral_data {
                    let (len_reordered, len_longest) = body
                        .reordered_spectral_lengths
                        .ok_or(Error::ScalableInvalid)?;
                    let len = crate::hcr::clamp_reordered_length(len_reordered, stereo);
                    let mut buf = vec![0u8; usize::from(len).div_ceil(8)];
                    for i in 0..usize::from(len) {
                        if read_bit(&mut reader)? {
                            buf[i / 8] |= 0x80 >> (i % 8);
                        }
                    }
                    crate::hcr_decode::decode_reordered_spectral_data(
                        &buf,
                        len,
                        len_longest,
                        &ics,
                        &body.section_data,
                        cfg.fs_index,
                    )?
                } else {
                    SpectralData::parse(&mut reader, &ics, &body.section_data, cfg.fs_index)?
                };
                channels.push(ScalableChannel { body, spectral });
            }

            // ---- Trailing extension_payload() loop + byte_alignment().
            let total_bits = (payload.len() as u64) * 8;
            let mut cnt = (total_bits.saturating_sub(reader.bit_position())) / 8;
            while cnt >= 1 {
                let p = ExtensionPayload::parse(&mut reader, cnt as u32)?;
                let used = u64::from(p.byte_length());
                if used == 0 || used > cnt {
                    return Err(Error::ScalableInvalid);
                }
                cnt -= used;
            }
            if reader.bit_position() > total_bits {
                return Err(Error::ScalableInvalid);
            }

            // ---- Cumulative bookkeeping.
            if stereo {
                last_max_sfb_ms = ics.max_sfb;
            } else {
                max_mono_sfb = core::cmp::max(max_mono_sfb, ics.max_sfb);
            }
            max_total_sfb = core::cmp::max(max_total_sfb, ics.max_sfb);

            layers.push(ScalableLayer {
                ics,
                ms_mask_present,
                ms_used_new,
                tns,
                ltp,
                diff_lr_long: dl_long,
                diff_lr_short: dl_short,
                channels,
            });
        }

        // Pad the cumulative mask rows to max_total_sfb.
        for row in &mut ms_used {
            if row.len() < usize::from(max_total_sfb) {
                row.resize(usize::from(max_total_sfb), false);
            }
        }

        Ok(ScalableFrame {
            layers,
            ms_used,
            diff_lr_long,
            diff_lr_short,
            max_total_sfb,
            max_mono_sfb,
        })
    }

    /// Re-emit the frame as one byte-aligned payload per layer — the
    /// bit-exact inverse of [`ScalableFrame::parse`] (no trailing
    /// extension payloads are emitted).
    pub fn write(&self, cfg: &ScalableConfig) -> Result<Vec<Vec<u8>>> {
        cfg.validate()?;
        if self.layers.len() != cfg.layer_stereo.len() {
            return Err(Error::ScalableInvalid);
        }
        let mut out = Vec::with_capacity(self.layers.len());
        let mut last_max_sfb_ms: u8 = 0;
        let mut max_mono_sfb: u8 = 0;
        for (lay, layer) in self.layers.iter().enumerate() {
            let stereo = cfg.layer_stereo[lay];
            let n_ch = cfg.channels_of_layer(lay);
            if layer.channels.len() != n_ch {
                return Err(Error::ScalableInvalid);
            }
            let mut w = BitWriter::new();
            let ics = &layer.ics;
            if lay == 0 {
                w.write_bit(ics.ics_reserved_bit);
                w.write_u32(u32::from(ics.window_sequence as u8), 2);
                w.write_bit(matches!(ics.window_shape, WindowShape::Kbd));
                if ics.window_sequence.is_eight_short() {
                    w.write_u32(u32::from(ics.max_sfb), 4);
                    w.write_u32(
                        u32::from(ics.scale_factor_grouping.ok_or(Error::ScalableInvalid)?),
                        7,
                    );
                } else {
                    w.write_u32(u32::from(ics.max_sfb), 6);
                }
                if stereo {
                    write_ms_data(
                        &mut w,
                        layer.ms_mask_present,
                        &layer.ms_used_new,
                        0,
                        ics.max_sfb,
                    )?;
                }
                for ch in 0..n_ch {
                    let tns = layer.tns.get(ch).ok_or(Error::ScalableInvalid)?;
                    w.write_bit(tns.is_some());
                    if let Some(t) = tns {
                        t.write(&mut w, ics.window_sequence)?;
                    }
                    let ltp = layer.ltp.get(ch).ok_or(Error::ScalableInvalid)?;
                    w.write_bit(ltp.is_some());
                    if let Some(l) = ltp {
                        write_ltp_data(&mut w, l, cfg.aot, ics.window_sequence, ics.max_sfb)?;
                    }
                }
            } else {
                if ics.window_sequence.is_eight_short() {
                    w.write_u32(u32::from(ics.max_sfb), 4);
                } else {
                    w.write_u32(u32::from(ics.max_sfb), 6);
                }
                if stereo {
                    write_ms_data(
                        &mut w,
                        layer.ms_mask_present,
                        &layer.ms_used_new,
                        last_max_sfb_ms,
                        ics.max_sfb,
                    )?;
                }
                if cfg.mono_stereo_flag(lay) {
                    for ch in 0..2usize {
                        let tns = layer.tns.get(ch).ok_or(Error::ScalableInvalid)?;
                        w.write_bit(tns.is_some());
                        if let Some(t) = tns {
                            t.write(&mut w, ics.window_sequence)?;
                        }
                    }
                }
                if cfg.mono_layer_flag() && stereo {
                    for ch in 0..2usize {
                        if ics.window_sequence != WindowSequence::EightShort {
                            let bits = layer.diff_lr_long.get(ch).ok_or(Error::ScalableInvalid)?;
                            let mut it = bits.iter();
                            let hi = core::cmp::min(max_mono_sfb, ics.max_sfb);
                            for sfb in last_max_sfb_ms..hi {
                                let on = self
                                    .ms_used
                                    .first()
                                    .and_then(|row| row.get(usize::from(sfb)))
                                    .copied()
                                    .unwrap_or(false);
                                if !on {
                                    w.write_bit(*it.next().ok_or(Error::ScalableInvalid)?);
                                }
                            }
                            if it.next().is_some() {
                                return Err(Error::ScalableInvalid);
                            }
                        } else if last_max_sfb_ms == 0 {
                            let bits = layer
                                .diff_lr_short
                                .get(ch)
                                .and_then(|b| *b)
                                .ok_or(Error::ScalableInvalid)?;
                            for b in bits {
                                w.write_bit(b);
                            }
                        }
                    }
                }
            }

            for chan in &layer.channels {
                chan.body.write_scale(&mut w, ics, cfg.resilience)?;
                if cfg.resilience.spectral_data {
                    let (buf, len, _longest) = crate::hcr_decode::encode_reordered_spectral_data(
                        &chan.spectral,
                        ics,
                        &chan.body.section_data,
                        cfg.fs_index,
                    )?;
                    // The body writer emitted the stored length fields;
                    // they must match the re-encoded payload.
                    let (stored_len, _stored_longest) = chan
                        .body
                        .reordered_spectral_lengths
                        .ok_or(Error::ScalableInvalid)?;
                    if stored_len != len {
                        return Err(Error::ScalableInvalid);
                    }
                    for i in 0..usize::from(len) {
                        w.write_bit(buf[i / 8] & (0x80 >> (i % 8)) != 0);
                    }
                } else {
                    chan.spectral
                        .write(&mut w, ics, &chan.body.section_data, cfg.fs_index)?;
                }
            }
            // byte_alignment()
            let pos = w.bit_position();
            for _ in 0..((8 - (pos % 8)) % 8) {
                w.write_bit(false);
            }
            out.push(w.finish());

            if stereo {
                last_max_sfb_ms = ics.max_sfb;
            } else {
                max_mono_sfb = core::cmp::max(max_mono_sfb, ics.max_sfb);
            }
        }
        Ok(out)
    }
}

/// Parse a stereo layer's `ms_mask_present` + Table 4.60 `ms_data()`
/// covering bands `lo..hi` (the §4.6.8.1.4 incremental range).
fn parse_ms_data(
    reader: &mut BitReader<'_>,
    groups: usize,
    lo: u8,
    hi: u8,
) -> Result<(MsMaskPresent, Vec<Vec<bool>>)> {
    let bits = read_u8(reader, 2)?;
    // §4.6.8.1.2: `11` is reserved.
    let mask = MsMaskPresent::from_bits(bits).map_err(|_| Error::ScalableInvalid)?;
    let mut rows = Vec::new();
    if mask == MsMaskPresent::Mask {
        for _g in 0..groups {
            let mut row = Vec::new();
            for _sfb in lo..hi {
                row.push(read_bit(reader)?);
            }
            rows.push(row);
        }
    }
    Ok((mask, rows))
}

/// Emit `ms_mask_present` + the incremental `ms_data()` rows.
fn write_ms_data(
    w: &mut BitWriter,
    mask: MsMaskPresent,
    rows: &[Vec<bool>],
    lo: u8,
    hi: u8,
) -> Result<()> {
    w.write_u32(u32::from(mask.to_bits()), 2);
    if mask == MsMaskPresent::Mask {
        let span = usize::from(hi.saturating_sub(lo));
        for row in rows {
            if row.len() != span {
                return Err(Error::ScalableInvalid);
            }
            for &b in row {
                w.write_bit(b);
            }
        }
    }
    Ok(())
}

/// Fold a layer's transmitted mask into the cumulative `ms_used`
/// (§4.6.8.1.4). `AllOnes` sets the whole incremental range; `Mask`
/// scatters the transmitted rows; `AllZeros` leaves the range clear.
fn merge_ms_rows(
    ms_used: &mut [Vec<bool>],
    mask: MsMaskPresent,
    rows: &[Vec<bool>],
    lo: u8,
    hi: u8,
) {
    for (g, row) in ms_used.iter_mut().enumerate() {
        if row.len() < usize::from(hi) {
            row.resize(usize::from(hi), false);
        }
        for sfb in lo..hi {
            let v = match mask {
                MsMaskPresent::AllZeros => false,
                MsMaskPresent::AllOnes => true,
                MsMaskPresent::Mask => rows
                    .get(g)
                    .and_then(|r| r.get(usize::from(sfb - lo)))
                    .copied()
                    .unwrap_or(false),
            };
            if v {
                row[usize::from(sfb)] = true;
            }
        }
    }
}

fn read_bit(reader: &mut BitReader<'_>) -> Result<bool> {
    reader.read_bit().map_err(|_| Error::UnexpectedEnd)
}

fn read_u8(reader: &mut BitReader<'_>, bits: u32) -> Result<u8> {
    Ok(reader.read_u32(bits).map_err(|_| Error::UnexpectedEnd)? as u8)
}

// ---------------------------------------------------------------------------
// Layer combination + decode driver (§4.5.2.2.4 / §4.6.14.2 / §4.6.9.5)
// ---------------------------------------------------------------------------

/// Per-band combination state across stereo layers (Tables 4.92/4.93).
#[derive(Debug, Clone, Copy, Default)]
struct BandState {
    covered_l: bool,
    covered_r: bool,
    noise_l: Option<i32>,
    noise_r: Option<i32>,
    /// `(in_phase, is_position)` — set while the band is
    /// intensity-coded; the position comes from the highest IS layer.
    intensity: Option<(bool, i32)>,
}

/// The window-major slices of one `(g, sfb)` band.
fn band_slices(ics: &IcsInfo, fs: u8, g: usize, sfb: usize) -> Result<Vec<(usize, usize)>> {
    let window_len = ics.window_len()?;
    let offsets = ics.swb_offsets(fs)?;
    let lo = *offsets.get(sfb).ok_or(Error::ScalableInvalid)? as usize;
    let hi = *offsets.get(sfb + 1).ok_or(Error::ScalableInvalid)? as usize;
    let mut window_base = 0usize;
    let mut out = Vec::new();
    for (gg, &wgl) in ics.window_group_length.iter().enumerate() {
        if gg == g {
            for b in 0..usize::from(wgl) {
                let base = (window_base + b) * window_len;
                out.push((base + lo, base + hi));
            }
            return Ok(out);
        }
        window_base += usize::from(wgl);
    }
    Err(Error::ScalableInvalid)
}

/// `true` iff every coefficient of the band is exactly zero in `spec`
/// (§4.6.13.6 "all spectral coefficients … are decoded to zero").
fn band_is_zero(spec: &[f64], slices: &[(usize, usize)]) -> bool {
    slices
        .iter()
        .all(|&(a, b)| spec[a..b].iter().all(|&v| v == 0.0))
}

fn add_band(dst: &mut [f64], src: &[f64], slices: &[(usize, usize)], gain: f64) {
    for &(a, b) in slices {
        for i in a..b {
            dst[i] += gain * src[i];
        }
    }
}

fn copy_band(dst: &mut [f64], src: &[f64], slices: &[(usize, usize)]) {
    for &(a, b) in slices {
        dst[a..b].copy_from_slice(&src[a..b]);
    }
}

fn zero_band(dst: &mut [f64], slices: &[(usize, usize)]) {
    for &(a, b) in slices {
        for v in &mut dst[a..b] {
            *v = 0.0;
        }
    }
}

/// One reconstructed layer: window-major dequantized spectra plus the
/// per-channel band tables.
struct LayerRecon {
    /// `[channel]` window-major spectra.
    specs: Vec<Vec<f64>>,
    /// `[channel]` band-indexed `noise_nrg[g][sfb]`.
    noise: Vec<Vec<Vec<i32>>>,
    /// Right-channel band-indexed `is_pos[g][sfb]` (stereo layers).
    is_pos: Option<Vec<Vec<i32>>>,
}

/// §4.6.9.5: the lowest sfb any of this `tns_data()`'s filters
/// reaches (the filters run downward from `max_sfb`), minimised over
/// windows. Used for the Table 4.158 serial-filter override rule.
fn tns_lower_boundary(tns: &TnsData, max_sfb: u8) -> u8 {
    let mut lowest = max_sfb;
    for w in &tns.windows {
        let total: u32 = w.filters.iter().map(|f| u32::from(f.length)).sum();
        let bottom = u32::from(max_sfb).saturating_sub(total) as u8;
        lowest = core::cmp::min(lowest, bottom);
    }
    lowest
}

/// Spectral-domain output of the layer-combination pipeline: one
/// combined spectrum per output channel, ready for the filterbank.
struct CombinedSpectra {
    chans: Vec<Vec<f64>>,
}

/// Stateful decoder for one scalable program (§4.5.2.2).
///
/// Feed one payload per layer per frame ([`ScalableDecoder::decode_frame`]);
/// the per-channel §4.6.11 overlap-add tails and the §4.6.7.5
/// base-layer LTP history persist across frames.
#[derive(Debug)]
pub struct ScalableDecoder {
    cfg: ScalableConfig,
    /// Output-path filterbanks (1 or 2).
    out_fbs: Vec<Filterbank>,
    /// Base-layer filterbanks for the §4.6.7.5 LTP history (used only
    /// when more than one layer is configured).
    base_fbs: Vec<Filterbank>,
    /// §4.6.7.5 base-layer LTP reconstruction state per base channel.
    base_ltp: Vec<LtpState>,
    /// §4.6.13.3 generator state for the output run.
    pns_state: u32,
    /// Independent generator state for the base-layer history run.
    base_pns_state: u32,
}

impl ScalableDecoder {
    /// Build a decoder for the given configuration.
    pub fn new(cfg: ScalableConfig) -> Result<Self> {
        cfg.validate()?;
        if cfg.aot == 6
            && (cfg.resilience.section_data
                || cfg.resilience.scalefactor_data
                || cfg.resilience.spectral_data)
        {
            return Err(Error::ScalableInvalid);
        }
        let n_out = cfg.output_channels();
        let n_base = cfg.channels_of_layer(0);
        Ok(ScalableDecoder {
            out_fbs: (0..n_out)
                .map(|_| Filterbank::new_family(cfg.family))
                .collect(),
            base_fbs: (0..n_base)
                .map(|_| Filterbank::new_family(cfg.family))
                .collect(),
            base_ltp: (0..n_base)
                .map(|_| LtpState::new_family(cfg.family))
                .collect(),
            pns_state: 0x0001_2345,
            base_pns_state: 0x0001_2345,
            cfg,
        })
    }

    /// The static configuration.
    pub fn config(&self) -> &ScalableConfig {
        &self.cfg
    }

    /// Decode one frame (one payload per layer, layer 0 first) to
    /// interleaved 16-bit PCM.
    pub fn decode_frame(&mut self, payloads: &[&[u8]]) -> Result<crate::decode::DecodedFrame> {
        let chans = self.decode_frame_channels(payloads)?;
        let pcm = crate::pcm::interleave_s16(&chans)?;
        Ok(crate::decode::DecodedFrame {
            pcm,
            channels: chans.len(),
            sample_rate: self.cfg.sample_rate,
        })
    }

    /// Decode one frame to per-channel `f64` time signals (`L, R` or
    /// mono), each `family.frame_len()` samples.
    pub fn decode_frame_channels(&mut self, payloads: &[&[u8]]) -> Result<Vec<Vec<f64>>> {
        let frame = ScalableFrame::parse(&self.cfg, payloads)?;
        let fs = self.cfg.fs_index;

        // ---- Per-layer reconstruction (SIAQ inverse quantisation).
        let mut recon: Vec<LayerRecon> = Vec::with_capacity(frame.layers.len());
        for layer in &frame.layers {
            let mut specs = Vec::new();
            let mut noise = Vec::new();
            let mut abs_all: Vec<AbsoluteScaleFactors> = Vec::new();
            for chan in &layer.channels {
                let abs = accumulate(
                    &chan.body.scale_factor_data,
                    &chan.body.section_data.sfb_cb,
                    chan.body.global_gain,
                )?;
                let rescaled = rescale_spectrum(
                    &chan.spectral,
                    &abs,
                    &chan.body.section_data.sfb_cb,
                    &layer.ics,
                    fs,
                )?;
                let spec = quant_to_spec(&rescaled, &layer.ics, fs)?;
                noise.push(crate::element_decode::noise_nrg_table(
                    &abs,
                    &chan.body.section_data.sfb_cb,
                    usize::from(layer.ics.max_sfb),
                )?);
                specs.push(spec);
                abs_all.push(abs);
            }
            let is_pos = if layer.channels.len() == 2 {
                Some(crate::element_decode::is_pos_table(
                    &abs_all[1],
                    &layer.channels[1].body.section_data.sfb_cb,
                    usize::from(layer.ics.max_sfb),
                )?)
            } else {
                None
            };
            recon.push(LayerRecon {
                specs,
                noise,
                is_pos,
            });
        }

        // ---- §4.6.7.5 base-layer LTP (prediction on layer 0 only;
        // IS / PNS bands of the base layer take precedence).
        let single_layer = frame.layers.len() == 1;
        {
            let layer0 = &frame.layers[0];
            let n_base = layer0.channels.len();
            for ch in 0..n_base {
                if let Some(ltp) = &layer0.ltp[ch] {
                    let mut masked = ltp.clone();
                    let sfb_cb_own = &layer0.channels[ch].body.section_data.sfb_cb;
                    let sfb_cb_right = &layer0.channels[n_base - 1].body.section_data.sfb_cb;
                    for (sfb, used) in masked.long_used.iter_mut().enumerate() {
                        let noise_band = sfb_cb_own
                            .first()
                            .and_then(|row| row.get(sfb))
                            .is_some_and(|&cb| cb == NOISE_HCB);
                        let is_band = n_base == 2
                            && sfb_cb_right
                                .first()
                                .and_then(|row| row.get(sfb))
                                .is_some_and(|&cb| cb == INTENSITY_HCB || cb == INTENSITY_HCB2);
                        if noise_band || is_band {
                            *used = false;
                        }
                    }
                    let fb = if single_layer {
                        &self.out_fbs[ch]
                    } else {
                        &self.base_fbs[ch]
                    };
                    let prev_shape = fb.prev_shape();
                    let tns = layer0.tns[ch].clone();
                    let ics0 = &layer0.ics;
                    let aot = self.cfg.aot;
                    let spec0 = &mut recon[0].specs[ch];
                    self.base_ltp[ch].apply_long_with_analysis(
                        spec0,
                        ics0,
                        &masked,
                        prev_shape,
                        fs,
                        |x_est| {
                            if let Some(tns) = &tns {
                                tns_analysis_frame_ics(x_est, tns, ics0, aot, fs)?;
                            }
                            Ok(())
                        },
                    )?;
                }
            }
        }

        // ---- Full combination run → output channels.
        let n_layers = frame.layers.len();
        let mut pns_state = self.pns_state;
        let combined = combine_layers(&self.cfg, &frame, &recon, n_layers, &mut pns_state)?;
        self.pns_state = pns_state;
        let mut out: Vec<Vec<f64>> = Vec::with_capacity(combined.chans.len());
        for (ch, spec) in combined.chans.iter().enumerate() {
            out.push(self.out_fbs[ch].synthesize(spec, &frame.layers[0].ics)?);
        }

        // ---- §4.6.7.5 LTP history: the time-domain output of the
        // first GA layer decoded alone.
        if single_layer {
            for (ch, o) in out.iter().enumerate() {
                let tail = self.out_fbs[ch].aliased_tail().to_vec();
                self.base_ltp[ch].push_frame(o, &tail);
            }
        } else {
            let mut base_pns = self.base_pns_state;
            let base = combine_layers(&self.cfg, &frame, &recon, 1, &mut base_pns)?;
            self.base_pns_state = base_pns;
            for (ch, spec) in base.chans.iter().enumerate() {
                let o = self.base_fbs[ch].synthesize(spec, &frame.layers[0].ics)?;
                let tail = self.base_fbs[ch].aliased_tail().to_vec();
                self.base_ltp[ch].push_frame(&o, &tail);
            }
        }
        Ok(out)
    }
}

/// Run the §4.5.2.2.4 layer combination over the first `n_layers`
/// layers: SIAQ accumulation with the Table 4.91–4.93 per-band rules,
/// the §4.6.14.2 FSS mono→stereo merge, cumulative M/S (§4.6.8.1.4),
/// intensity (§4.6.8.2.3), PNS (§4.6.13.6) and the §4.6.9.5 serial
/// TNS. Returns the combined spectra ready for the filterbank.
fn combine_layers(
    cfg: &ScalableConfig,
    frame: &ScalableFrame,
    recon: &[LayerRecon],
    n_layers: usize,
    pns_state: &mut u32,
) -> Result<CombinedSpectra> {
    let fs = cfg.fs_index;
    let base_ics = &frame.layers[0].ics;
    let window_len = base_ics.window_len()?;
    let num_windows = usize::from(base_ics.num_windows);
    let spec_len = num_windows * window_len;
    let num_groups = usize::from(base_ics.num_window_groups);

    // Coverage bounds inside this sub-run.
    let stereo_present = (0..n_layers).any(|l| cfg.layer_stereo[l]);
    let max_mono: u8 = (0..n_layers)
        .filter(|&l| !cfg.layer_stereo[l])
        .map(|l| frame.layers[l].ics.max_sfb)
        .max()
        .unwrap_or(0);
    let max_total: u8 = (0..n_layers)
        .map(|l| frame.layers[l].ics.max_sfb)
        .max()
        .unwrap_or(0);

    // The synthetic geometry every final band op runs under.
    let mut ics_total = base_ics.clone();
    ics_total.max_sfb = max_total;

    // Precompute band slices.
    let mut slices: Vec<Vec<Vec<(usize, usize)>>> = Vec::with_capacity(num_groups);
    for g in 0..num_groups {
        let mut per_sfb = Vec::with_capacity(usize::from(max_total));
        for sfb in 0..usize::from(max_total) {
            per_sfb.push(band_slices(&ics_total, fs, g, sfb)?);
        }
        slices.push(per_sfb);
    }

    // ---- Stage 1: mono prefix (Table 4.91).
    let mut m_acc = vec![0.0f64; spec_len];
    let mut m_noise: Vec<Vec<Option<i32>>> = vec![vec![None; usize::from(max_total)]; num_groups];
    let mut m_covered: Vec<Vec<bool>> = vec![vec![false; usize::from(max_total)]; num_groups];
    for (l, rec) in recon.iter().enumerate().take(n_layers) {
        if cfg.layer_stereo[l] {
            continue;
        }
        let layer = &frame.layers[l];
        let spec = &rec.specs[0];
        let sfb_cb = &layer.channels[0].body.section_data.sfb_cb;
        for g in 0..num_groups {
            for sfb in 0..usize::from(layer.ics.max_sfb) {
                let cb = sfb_cb[g][sfb];
                let sl = &slices[g][sfb];
                if cb == NOISE_HCB {
                    if m_covered[g][sfb] && m_noise[g][sfb].is_none() {
                        // Table 4.91: No Tool → PNS is invalid.
                        return Err(Error::ScalableLayerCombination);
                    }
                    // First coverage or PNS → PNS (layer N+1 wins).
                    m_noise[g][sfb] = Some(rec.noise[0][g][sfb]);
                } else {
                    if m_noise[g][sfb].is_some() && !band_is_zero(spec, sl) {
                        // §4.6.13.6: non-zero higher-layer content
                        // cancels the noise substitution.
                        m_noise[g][sfb] = None;
                    }
                    add_band(&mut m_acc, spec, sl, 1.0);
                }
                m_covered[g][sfb] = true;
            }
        }
    }

    // ---- Stage 2: stereo layers (Table 4.92).
    let mut l_acc = vec![0.0f64; spec_len];
    let mut r_acc = vec![0.0f64; spec_len];
    let mut st: Vec<Vec<BandState>> =
        vec![vec![BandState::default(); usize::from(max_total)]; num_groups];
    for (l, rec) in recon.iter().enumerate().take(n_layers) {
        if !cfg.layer_stereo[l] {
            continue;
        }
        let layer = &frame.layers[l];
        let (lspec, rspec) = (&rec.specs[0], &rec.specs[1]);
        let lcb_t = &layer.channels[0].body.section_data.sfb_cb;
        let rcb_t = &layer.channels[1].body.section_data.sfb_cb;
        for g in 0..num_groups {
            for sfb in 0..usize::from(layer.ics.max_sfb) {
                let sl = &slices[g][sfb];
                let lcb = lcb_t[g][sfb];
                let rcb = rcb_t[g][sfb];
                let s = &mut st[g][sfb];
                let is_band = rcb == INTENSITY_HCB || rcb == INTENSITY_HCB2;
                if is_band {
                    let pos = rec.is_pos.as_ref().map(|t| t[g][sfb]).unwrap_or(0);
                    let in_phase = rcb == INTENSITY_HCB;
                    if s.intensity.is_some() {
                        // IS → IS: sum the M/L channel, take the
                        // positions from layer N+1.
                        add_band(&mut l_acc, lspec, sl, 1.0);
                    } else if s.noise_l.is_some() || s.noise_r.is_some() {
                        // PNS → IS: layer N+1 only.
                        s.noise_l = None;
                        s.noise_r = None;
                        copy_band(&mut l_acc, lspec, sl);
                        zero_band(&mut r_acc, sl);
                    } else if s.covered_l || s.covered_r {
                        // No Tool / MS → IS: invalid (Table 4.92).
                        return Err(Error::ScalableLayerCombination);
                    } else {
                        copy_band(&mut l_acc, lspec, sl);
                    }
                    s.intensity = Some((in_phase, pos));
                    s.covered_l = true;
                    s.covered_r = true;
                    continue;
                }
                if s.intensity.is_some() {
                    if lcb == NOISE_HCB || rcb == NOISE_HCB {
                        // IS → PNS: invalid (Table 4.92).
                        return Err(Error::ScalableLayerCombination);
                    }
                    // IS → No Tool / MS: layer N+1 only.
                    s.intensity = None;
                    copy_band(&mut l_acc, lspec, sl);
                    copy_band(&mut r_acc, rspec, sl);
                    s.covered_l = true;
                    s.covered_r = true;
                    continue;
                }
                // Per-channel plain / noise handling.
                let ms_band = frame
                    .ms_used
                    .get(g)
                    .and_then(|row| row.get(sfb))
                    .copied()
                    .unwrap_or(false);
                let l_zero = band_is_zero(lspec, sl);
                let r_zero = band_is_zero(rspec, sl);
                // Table 4.93: a plain-coded mono band cannot turn
                // into a stereo PNS band (No Tool → PNS is invalid).
                let mono_plain = m_covered[g][sfb] && m_noise[g][sfb].is_none();
                // Left channel.
                if lcb == NOISE_HCB {
                    if s.covered_l && s.noise_l.is_none() {
                        return Err(Error::ScalableLayerCombination);
                    }
                    if !s.covered_l && mono_plain {
                        return Err(Error::ScalableLayerCombination);
                    }
                    s.noise_l = Some(rec.noise[0][g][sfb]);
                } else {
                    if s.noise_l.is_some() {
                        let cancels = if ms_band {
                            !(l_zero && r_zero)
                        } else {
                            !l_zero
                        };
                        if cancels {
                            s.noise_l = None;
                        }
                    }
                    add_band(&mut l_acc, lspec, sl, 1.0);
                }
                s.covered_l = true;
                // Right channel.
                if rcb == NOISE_HCB {
                    if s.covered_r && s.noise_r.is_none() {
                        return Err(Error::ScalableLayerCombination);
                    }
                    if !s.covered_r && mono_plain {
                        return Err(Error::ScalableLayerCombination);
                    }
                    s.noise_r = Some(rec.noise[1][g][sfb]);
                } else {
                    if s.noise_r.is_some() {
                        let cancels = if ms_band {
                            !(l_zero && r_zero)
                        } else {
                            !r_zero
                        };
                        if cancels {
                            s.noise_r = None;
                        }
                    }
                    add_band(&mut r_acc, rspec, sl, 1.0);
                }
                s.covered_r = true;
            }
        }
    }

    if !stereo_present {
        // ---- Mono-only output: PNS, then serial TNS (M source).
        let mut sfb_cb: Vec<Vec<u8>> = vec![vec![1u8; usize::from(max_total)]; num_groups];
        let mut noise_tab: Vec<Vec<i32>> = vec![vec![0i32; usize::from(max_total)]; num_groups];
        for g in 0..num_groups {
            for sfb in 0..usize::from(max_total) {
                if let Some(nrg) = m_noise[g][sfb] {
                    sfb_cb[g][sfb] = NOISE_HCB;
                    noise_tab[g][sfb] = nrg;
                }
            }
        }
        {
            let mut chan = PnsChannel {
                spec: &mut m_acc,
                sfb_cb: &sfb_cb,
                noise_nrg: &noise_tab,
            };
            apply_pns(&mut chan, &ics_total, fs, |out| {
                gen_rand_vector(out, pns_state)
            })?;
        }
        // First mono layer's TNS serves the M output (Table 4.158).
        let first_mono = (0..n_layers).find(|&l| !cfg.layer_stereo[l]);
        if let Some(l0) = first_mono {
            if let Some(tns) = frame.layers[l0].tns.first().and_then(|t| t.as_ref()) {
                tns_decode_frame_ics(&mut m_acc, tns, &frame.layers[l0].ics, cfg.aot, fs)?;
            }
        }
        return Ok(CombinedSpectra { chans: vec![m_acc] });
    }

    // ---- Stage 3: mono → stereo merge (Table 4.93 + §4.6.14.2.1).
    let has_mono = (0..n_layers).any(|l| !cfg.layer_stereo[l]);
    if has_mono {
        let short = base_ics.window_sequence.is_eight_short();
        if !short {
            for g in 0..num_groups {
                for sfb in 0..usize::from(max_mono) {
                    let s = &st[g][sfb];
                    if s.intensity.is_some() || s.noise_l.is_some() || s.noise_r.is_some() {
                        // Mono content never crosses into an IS / PNS
                        // band (Table 4.93).
                        continue;
                    }
                    if m_noise[g][sfb].is_some() {
                        // A mono PNS band never crosses the transition.
                        continue;
                    }
                    let sl = &slices[g][sfb];
                    let ms_band = frame.ms_used[g].get(sfb).copied().unwrap_or(false);
                    if ms_band {
                        // M = M'' + M' (§4.5.2.2.4).
                        add_band(&mut l_acc, &m_acc, sl, 1.0);
                    } else {
                        // §4.6.14.2.1 FSS: `+ 2·M''` where the bit is 0.
                        if frame.diff_lr_long[0].get(sfb).copied().flatten() == Some(false) {
                            add_band(&mut l_acc, &m_acc, sl, 2.0);
                        }
                        if frame.diff_lr_long[1].get(sfb).copied().flatten() == Some(false) {
                            add_band(&mut r_acc, &m_acc, sl, 2.0);
                        }
                    }
                }
            }
        } else {
            // §4.6.14.2.1 short windows: diff_control_lr[win][0]
            // covers every band up to the mono coverage per window.
            let offsets = ics_total.swb_offsets(fs)?;
            let hi_coef = usize::from(offsets[usize::from(max_mono)]);
            let mut window_of_group: Vec<usize> = Vec::with_capacity(num_windows);
            for (g, &wgl) in base_ics.window_group_length.iter().enumerate() {
                for _ in 0..wgl {
                    window_of_group.push(g);
                }
            }
            for w in 0..num_windows {
                let g = window_of_group[w];
                let base = w * window_len;
                for sfb in 0..usize::from(max_mono) {
                    let s = &st[g][sfb];
                    if s.intensity.is_some() || s.noise_l.is_some() || s.noise_r.is_some() {
                        continue;
                    }
                    if m_noise[g][sfb].is_some() {
                        continue;
                    }
                    let a = base + usize::from(offsets[sfb]);
                    let b = base + core::cmp::min(usize::from(offsets[sfb + 1]), hi_coef);
                    let ms_band = frame.ms_used[g].get(sfb).copied().unwrap_or(false);
                    if ms_band {
                        for i in a..b {
                            l_acc[i] += m_acc[i];
                        }
                    } else {
                        if frame.diff_lr_short[0].map(|bits| bits[w]) == Some(false) {
                            for i in a..b {
                                l_acc[i] += 2.0 * m_acc[i];
                            }
                        }
                        if frame.diff_lr_short[1].map(|bits| bits[w]) == Some(false) {
                            for i in a..b {
                                r_acc[i] += 2.0 * m_acc[i];
                            }
                        }
                    }
                }
            }
        }
    }

    // ---- Stage 4: synthetic band tables → M/S → IS → PNS.
    let mut synth_l: Vec<Vec<u8>> = vec![vec![1u8; usize::from(max_total)]; num_groups];
    let mut synth_r: Vec<Vec<u8>> = vec![vec![1u8; usize::from(max_total)]; num_groups];
    let mut noise_l_tab: Vec<Vec<i32>> = vec![vec![0i32; usize::from(max_total)]; num_groups];
    let mut noise_r_tab: Vec<Vec<i32>> = vec![vec![0i32; usize::from(max_total)]; num_groups];
    let mut is_pos_tab: Vec<Vec<i32>> = vec![vec![0i32; usize::from(max_total)]; num_groups];
    for g in 0..num_groups {
        for sfb in 0..usize::from(max_total) {
            let s = &st[g][sfb];
            if let Some((in_phase, pos)) = s.intensity {
                synth_r[g][sfb] = if in_phase {
                    INTENSITY_HCB
                } else {
                    INTENSITY_HCB2
                };
                is_pos_tab[g][sfb] = pos;
                continue;
            }
            if let Some(nrg) = s.noise_l {
                synth_l[g][sfb] = NOISE_HCB;
                noise_l_tab[g][sfb] = nrg;
            }
            if let Some(nrg) = s.noise_r {
                synth_r[g][sfb] = NOISE_HCB;
                noise_r_tab[g][sfb] = nrg;
            }
        }
    }

    {
        let mut pair = ChannelPairSpectra {
            left: &mut l_acc,
            right: &mut r_acc,
            left_sfb_cb: &synth_l,
            right_sfb_cb: &synth_r,
        };
        apply_ms_stereo(
            &mut pair,
            MsMaskPresent::Mask,
            &frame.ms_used,
            &ics_total,
            fs,
        )?;
    }
    {
        let mut pair = IntensityPairSpectra {
            left: &l_acc,
            right: &mut r_acc,
            right_sfb_cb: &synth_r,
            is_pos: &is_pos_tab,
        };
        // §4.6.8.2.3: invert_intensity() == +1 for the scalable AOT,
        // so the ms_used phase-reversal branch is disabled.
        apply_intensity_stereo(&mut pair, false, &[], &ics_total, fs)?;
    }
    {
        let mut left = PnsChannel {
            spec: &mut l_acc,
            sfb_cb: &synth_l,
            noise_nrg: &noise_l_tab,
        };
        let mut right = PnsChannel {
            spec: &mut r_acc,
            sfb_cb: &synth_r,
            noise_nrg: &noise_r_tab,
        };
        // §4.6.13.6: the cumulative ms_used still signals noise
        // correlation across the channel pair.
        apply_pns_pair(
            &mut left,
            &mut right,
            true,
            false,
            &frame.ms_used,
            &ics_total,
            fs,
            |out| gen_rand_vector(out, pns_state),
        )?;
    }

    // ---- Stage 5: §4.6.9.5 serial TNS (Table 4.158).
    let first_mono = (0..n_layers).find(|&l| !cfg.layer_stereo[l]);
    let first_stereo = (0..n_layers).find(|&l| cfg.layer_stereo[l]);
    let tns_m: Option<(&TnsData, &IcsInfo)> = first_mono.and_then(|l| {
        frame.layers[l]
            .tns
            .first()
            .and_then(|t| t.as_ref())
            .map(|t| (t, &frame.layers[l].ics))
    });
    for (ch, acc) in [&mut l_acc, &mut r_acc].into_iter().enumerate() {
        let tns_ch: Option<(&TnsData, &IcsInfo)> = first_stereo.and_then(|l| {
            frame.layers[l]
                .tns
                .get(ch)
                .and_then(|t| t.as_ref())
                .map(|t| (t, &frame.layers[l].ics))
        });
        match (tns_ch, tns_m) {
            (Some((t, ics)), Some((tm, ics_m))) => {
                // Serial L/M (R/M) layout: the M filter first (it
                // covers the low bands, stopping at the highest mono
                // max_sfb), then the channel filter — unless the
                // channel filter reaches below the mono boundary, in
                // which case the M filter is skipped.
                if tns_lower_boundary(t, ics.max_sfb) >= max_mono {
                    tns_decode_frame_ics(acc, tm, ics_m, cfg.aot, fs)?;
                }
                tns_decode_frame_ics(acc, t, ics, cfg.aot, fs)?;
            }
            (Some((t, ics)), None) => {
                tns_decode_frame_ics(acc, t, ics, cfg.aot, fs)?;
            }
            (None, Some((tm, ics_m))) => {
                tns_decode_frame_ics(acc, tm, ics_m, cfg.aot, fs)?;
            }
            (None, None) => {}
        }
    }

    Ok(CombinedSpectra {
        chans: vec![l_acc, r_acc],
    })
}

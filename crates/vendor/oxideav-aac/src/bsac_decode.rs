//! ER BSAC (AOT 22) decoder — ISO/IEC 14496-3:2009 §4.4.2.6 /
//! §4.5.2.6 / §4.6.4.
//!
//! Decodes a `bsac_raw_data_block()` end to end: the raw-bit
//! headers (Tables 4.34–4.36), the §4.5.2.6.2.5 layer roster, the
//! arithmetic-coded side information (`cband_si`, scalefactors,
//! stereo / PNS decisions) and the bit-sliced spectral data
//! (Tables 4.37–4.43 driving [`crate::bsac_arith`] over the
//! [`crate::bsac_tables`] models), then reconstructs PCM through
//! the standard AAC back end — §4.6.2 inverse quantization, the
//! §4.6.8.1 M/S and §4.6.8.2 intensity tools, §4.6.9 TNS and the
//! §4.6.11 filterbank — exactly as §4.6.4.1 prescribes ("the BSAC
//! noiseless coding module is an alternative to the AAC coding
//! module, with all other modules of the AAC-based coder remaining
//! unchanged").
//!
//! Not yet covered (surfaced as [`Error::BsacUnsupportedTool`]):
//! long-term prediction (`ltp_data_present == 1`), the
//! `zero_code`-prefixed extended part (BSAC channel extension /
//! SBR / MPEG-Surround payloads), and perceptual noise
//! substitution (`pns_data_present == 1`) pending an external
//! vector to pin its arithmetic-PCM offset conventions.

use crate::bsac_arith::{ArithDecoder, SegmentReader};
use crate::bsac_layer::{BsacGeometry, LayerInfo, BSAC_FRAME_LEN};
use crate::bsac_tables::{
    clamp_p0, context_position, spectral_p0, CBAND_SI_MODELS, CBAND_SI_MODEL_CBAND0,
    CBAND_SI_MSB_PLANE, CBAND_SI_TYPES, MS_USED_MODEL, SCF_MODELS, SIGN_P0, STEREO_INFO_MODEL,
};
use crate::dequant::{inverse_quantize, scale_factor_gain};
use crate::filterbank::Filterbank;
use crate::ics_info::{IcsInfo, WindowSequence, WindowShape};
use crate::ms_stereo::{apply_ms_stereo, ChannelPairSpectra, MsMaskPresent};
use crate::pcm::channel_to_s16;
use crate::swb_offset::FrameFamily;
use crate::tns_data::TnsData;
use crate::tns_frame::tns_decode_frame_ics;
use crate::{Error, Result};

use oxideav_core::bits::BitReader;

/// Parsed `bsac_header()` — Table 4.35.
#[derive(Debug, Clone)]
pub struct BsacHeader {
    /// `frame_length` (11 bits) — whole frame length in bytes.
    pub frame_length: usize,
    /// `header_length` (4 bits) — header length escape field
    /// (§4.5.2.6.2.2.3: values 1..=14 mean `(header_length + 7)`
    /// bytes; 0 / 15 defer to the decoded header length).
    pub header_length: u8,
    /// `sba_mode` (1 bit) — segmented binary arithmetic coding.
    pub sba_mode: bool,
    /// `top_layer` (6 bits).
    pub top_layer: usize,
    /// `base_snf_thr` (2 bits).
    pub base_snf_thr: u8,
    /// `max_scalefactor[ch]` (8 bits each).
    pub max_scalefactor: Vec<u8>,
    /// `base_band` (5 bits).
    pub base_band: usize,
    /// `cband_si_type[ch]` (5 bits each).
    pub cband_si_type: Vec<u8>,
    /// `base_scf_model[ch]` (3 bits each).
    pub base_scf_model: Vec<u8>,
    /// `enh_scf_model[ch]` (3 bits each).
    pub enh_scf_model: Vec<u8>,
    /// `max_sfb_si_len[ch]` (4 bits each, raw — the +5 offset is
    /// applied in the layer geometry).
    pub max_sfb_si_len: Vec<u8>,
}

/// Parsed `general_header()` — Table 4.36.
#[derive(Debug, Clone)]
pub struct GeneralHeader {
    /// `window_sequence` (2 bits).
    pub window_sequence: WindowSequence,
    /// `window_shape` (1 bit).
    pub window_shape: WindowShape,
    /// `max_sfb` (4 bits short / 6 bits long).
    pub max_sfb: usize,
    /// `scale_factor_grouping` (7 bits, `EIGHT_SHORT` only).
    pub scale_factor_grouping: u8,
    /// `pns_data_present` (1 bit).
    pub pns_data_present: bool,
    /// `pns_start_sfb` (6 bits, when PNS is present).
    pub pns_start_sfb: usize,
    /// `ms_mask_present` (2 bits, `nch == 2` only): 0 independent,
    /// 1 `ms_used` mask, 2 all ones, 3 `stereo_info` mask.
    pub ms_mask_present: u8,
    /// Per-channel §4.6.9 TNS record.
    pub tns: Vec<Option<TnsData>>,
}

/// One decoded `bsac_raw_data_block()`: quantized spectra plus the
/// side information the AAC back end consumes.
#[derive(Debug, Clone)]
pub struct DecodedBlock {
    /// The `bsac_header()`.
    pub header: BsacHeader,
    /// The `general_header()`.
    pub general: GeneralHeader,
    /// Signed quantized spectra, `[ch][g][group line]` in the
    /// §4.5.2.6.2.6 (possibly interleaved) group order.
    pub sample: Vec<Vec<Vec<i32>>>,
    /// Absolute scalefactors, `[ch][g][sfb]` (`None` where no band
    /// side info was decoded).
    pub scf: Vec<Vec<Vec<Option<u8>>>>,
    /// `ms_used[g][sfb]` (derived: `stereo_info == 1` counts).
    pub ms_used: Vec<Vec<bool>>,
    /// `stereo_info[g][sfb]` (0 independent / 1 M/S / 2 IS in
    /// phase / 3 IS out of phase).
    pub stereo_info: Vec<Vec<u8>>,
    /// Intensity position per `[g][sfb]` (`stereo_info >= 2`).
    pub is_position: Vec<Vec<i32>>,
    /// The layer geometry the block decoded under.
    pub geometry: BsacGeometry,
}

/// Per-(channel, group) bit-slice state.
#[derive(Debug, Clone, Default)]
struct LineState {
    /// Decoded bit-plane mask: bit `p-1` set = the plane-`p` sliced
    /// bit decoded 1. The magnitude equals the mask value.
    mask: Vec<u32>,
    /// Sign decoded (1 = negative).
    sign_neg: Vec<bool>,
    /// `sign_is_coded[]`.
    sign_coded: Vec<bool>,
    /// First-pass significance (`cur_snf`).
    cur_snf: Vec<i32>,
    /// Secondary-pass significance (`unc_snf`).
    unc_snf: Vec<i32>,
}

/// Which significance array a `bsac_spectral_data()` pass drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnfKind {
    /// The first coding pass (`bsac_layer_spectra`).
    Cur,
    /// The secondary passes (`bsac_lower_spectra` /
    /// `bsac_higher_spectra`).
    Unc,
}

/// The whole-block arithmetic decode driver.
struct BlockCtx<'a> {
    nch: usize,
    header: BsacHeader,
    general: GeneralHeader,
    geo: BsacGeometry,
    arith: ArithDecoder,
    reader: SegmentReader<'a>,
    /// Remaining budget of the current layer (bits).
    avail: i64,
    /// `cband_si[ch][g][cband]`.
    cband_si: Vec<Vec<Vec<u8>>>,
    /// Per-(ch, g) line state.
    lines: Vec<Vec<LineState>>,
    scf: Vec<Vec<Vec<Option<u8>>>>,
    stereo_side_info_coded: Vec<Vec<bool>>,
    ms_used: Vec<Vec<bool>>,
    stereo_info: Vec<Vec<u8>>,
    is_position: Vec<Vec<i32>>,
}

impl<'a> BlockCtx<'a> {
    fn layer_data_available(&self) -> bool {
        self.avail > 0
    }

    fn decode_symbol(&mut self, model: &[u16]) -> usize {
        let (sym, est) = self.arith.decode_symbol(&mut self.reader, model);
        self.avail -= i64::from(est);
        sym
    }

    fn decode_bit(&mut self, p0: u16) -> u8 {
        let (bit, est) = self.arith.decode_bit(&mut self.reader, p0);
        self.avail -= i64::from(est);
        bit
    }

    /// Table 4.38 `layer_cband_si()`.
    fn layer_cband_si(&mut self, layer: &LayerInfo) -> Result<()> {
        let g = layer.group;
        for ch in 0..self.nch {
            let params = &CBAND_SI_TYPES[self.header.cband_si_type[ch] as usize];
            for cband in layer.start_cband..layer.end_cband {
                let (model, largest): (&[u16], u8) = if cband == 0 {
                    (&CBAND_SI_MODEL_CBAND0, params.largest_cband0)
                } else {
                    (
                        CBAND_SI_MODELS[params.other_model as usize],
                        params.largest_other,
                    )
                };
                let si = self.decode_symbol(model);
                if si > usize::from(largest) {
                    return Err(Error::BsacBitError);
                }
                self.cband_si[ch][g][cband] = si as u8;
                // §4.5.2.6.2.5: cur_snf of the layer's new lines
                // initializes to the coding band's MSB plane.
                let plane = i32::from(CBAND_SI_MSB_PLANE[si]);
                let start = cband * 32;
                let end = (cband * 32 + 32).min(self.geo.group_len[g]);
                for i in start..end {
                    self.lines[ch][g].cur_snf[i] = plane;
                }
            }
        }
        Ok(())
    }

    /// The scalefactor-model symbol for the current layer.
    fn scf_symbol(&mut self, ch: usize, layer_idx: usize) -> Result<usize> {
        let model_idx = if layer_idx < self.geo.slayer_size {
            self.header.base_scf_model[ch]
        } else {
            self.header.enh_scf_model[ch]
        } as usize;
        match SCF_MODELS[model_idx] {
            Some(model) => Ok(self.decode_symbol(model)),
            // Model 0 is "not used" (Table 4.A.32): no symbol is
            // coded; the differential is zero.
            None => Ok(0),
        }
    }

    /// Table 4.39 `layer_sfb_si()`.
    fn layer_sfb_si(&mut self, layer_idx: usize, layer: &LayerInfo) -> Result<()> {
        let g = layer.group;
        let pns = self.general.pns_data_present;
        let msp = self.general.ms_mask_present;
        for ch in 0..self.nch {
            for sfb in layer.start_sfb..layer.end_sfb {
                if self.nch == 1 {
                    if pns && sfb >= self.general.pns_start_sfb {
                        // PNS decode needs the noise-energy PCM
                        // conventions pinned by an external vector.
                        return Err(Error::BsacUnsupportedTool);
                    }
                } else if !self.stereo_side_info_coded[g][sfb] {
                    if msp != 2 {
                        if msp == 1 {
                            let ms = self.decode_symbol(&MS_USED_MODEL);
                            self.ms_used[g][sfb] = ms == 1;
                        } else if msp == 3 {
                            let si = self.decode_symbol(&STEREO_INFO_MODEL) as u8;
                            self.stereo_info[g][sfb] = si;
                            self.ms_used[g][sfb] = si == 1;
                        }
                        if pns && sfb >= self.general.pns_start_sfb {
                            return Err(Error::BsacUnsupportedTool);
                        }
                    }
                    self.stereo_side_info_coded[g][sfb] = true;
                }
                // Per-channel scalefactor / intensity position.
                if self.stereo_info[g][sfb] >= 2 && ch == 1 {
                    let idx = self.scf_symbol(ch, layer_idx)? as i32;
                    // §4.6.4.4.3 zig-zag: odd → −(idx+1)/2, even →
                    // idx/2.
                    self.is_position[g][sfb] = if idx % 2 == 1 {
                        -(idx + 1) / 2
                    } else {
                        idx / 2
                    };
                } else {
                    let diff = self.scf_symbol(ch, layer_idx)? as i32;
                    let scf = i32::from(self.header.max_scalefactor[ch]) - diff;
                    if !(0..=255).contains(&scf) {
                        return Err(Error::BsacBitError);
                    }
                    self.scf[ch][g][sfb] = Some(scf as u8);
                }
            }
        }
        Ok(())
    }

    /// Table 4.43 `bsac_spectral_data()` over `regions`
    /// (`(group, start_index, end_index)`), down to (exclusive)
    /// `thr_snf`, driving the selected significance array.
    fn spectral_data(&mut self, regions: &[(usize, usize, usize)], thr_snf: i32, kind: SnfKind) {
        if !self.layer_data_available() {
            return;
        }
        // maxsnf over the region.
        let mut maxsnf = 0i32;
        for &(g, s, e) in regions {
            for ch in 0..self.nch {
                let st = &self.lines[ch][g];
                let arr = match kind {
                    SnfKind::Cur => &st.cur_snf,
                    SnfKind::Unc => &st.unc_snf,
                };
                for &v in arr[s..e.min(arr.len())].iter() {
                    maxsnf = maxsnf.max(v);
                }
            }
        }
        let mut snf = maxsnf;
        while snf > thr_snf {
            for &(g, s, e) in regions {
                let e = e.min(self.geo.group_len[g]);
                for i in s..e {
                    for ch in 0..self.nch {
                        {
                            let st = &self.lines[ch][g];
                            let v = match kind {
                                SnfKind::Cur => st.cur_snf[i],
                                SnfKind::Unc => st.unc_snf[i],
                            };
                            if v < snf {
                                continue;
                            }
                        }
                        let cband_si = self.cband_si[ch][g][i / 32];
                        let mask_i = self.lines[ch][g].mask[i];
                        let sign_coded = self.lines[ch][g].sign_coded[i];
                        if mask_i == 0 || sign_coded {
                            // Decode one sliced bit.
                            let hbv = mask_i >> snf;
                            let p0 = if hbv != 0 {
                                spectral_p0(cband_si, snf as u8, hbv, 0)
                            } else {
                                let a = i % 4;
                                let bit_at = |j: isize| -> u8 {
                                    if j < 0 {
                                        0
                                    } else {
                                        ((self.lines[ch][g].mask[j as usize] >> (snf - 1)) & 1)
                                            as u8
                                    }
                                };
                                let hb = |j: usize| -> u8 {
                                    if j >= self.geo.group_len[g] {
                                        0
                                    } else {
                                        u8::from(self.lines[ch][g].mask[j] >> snf != 0)
                                    }
                                };
                                let prev = [
                                    bit_at(i as isize - 3),
                                    bit_at(i as isize - 2),
                                    bit_at(i as isize - 1),
                                ];
                                let base = i - a;
                                let flags = [hb(base), hb(base + 1), hb(base + 2), hb(base + 3)];
                                spectral_p0(
                                    cband_si,
                                    snf as u8,
                                    0,
                                    context_position(a, prev, flags),
                                )
                            };
                            let p0 = clamp_p0(p0, self.avail);
                            let bit = self.decode_bit(p0);
                            if bit != 0 {
                                self.lines[ch][g].mask[i] |= 1 << (snf - 1);
                            }
                        }
                        if self.lines[ch][g].mask[i] != 0 && !self.lines[ch][g].sign_coded[i] {
                            if !self.layer_data_available() {
                                return;
                            }
                            let sign = self.decode_bit(SIGN_P0);
                            self.lines[ch][g].sign_neg[i] = sign == 1;
                            self.lines[ch][g].sign_coded[i] = true;
                        }
                        {
                            let st = &mut self.lines[ch][g];
                            match kind {
                                SnfKind::Cur => st.cur_snf[i] -= 1,
                                SnfKind::Unc => st.unc_snf[i] -= 1,
                            }
                        }
                        if !self.layer_data_available() {
                            return;
                        }
                    }
                }
            }
            snf -= 1;
        }
    }
}

/// Decode one `bsac_raw_data_block()` into quantized spectra + side
/// info.
///
/// `fs` / `fs_index` — the sampling rate from the ASC; `nch` — the
/// channel count (1 or 2).
pub fn decode_bsac_raw_data_block(
    frame: &[u8],
    fs: u32,
    fs_index: u8,
    nch: usize,
) -> Result<DecodedBlock> {
    if !(1..=2).contains(&nch) || frame.is_empty() {
        return Err(Error::BsacInvalidHeader);
    }
    let mut br = BitReader::new(frame);
    fn rd(br: &mut BitReader<'_>, n: u32) -> Result<u32> {
        br.read_u32(n).map_err(|_| Error::UnexpectedEnd)
    }

    // Table 4.34 / 4.35: frame_length + bsac_header().
    let frame_length = rd(&mut br, 11)? as usize;
    if frame_length > frame.len() || frame_length == 0 {
        return Err(Error::BsacInvalidHeader);
    }
    let header_length = rd(&mut br, 4)? as u8;
    let sba_mode = rd(&mut br, 1)? != 0;
    let top_layer = rd(&mut br, 6)? as usize;
    let base_snf_thr = rd(&mut br, 2)? as u8;
    let mut max_scalefactor = Vec::with_capacity(nch);
    for _ in 0..nch {
        max_scalefactor.push(rd(&mut br, 8)? as u8);
    }
    let base_band = rd(&mut br, 5)? as usize;
    let (mut cband_si_type, mut base_scf_model, mut enh_scf_model, mut max_sfb_si_len) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for _ in 0..nch {
        let t = rd(&mut br, 5)? as u8;
        if usize::from(t) >= CBAND_SI_TYPES.len() {
            return Err(Error::BsacInvalidHeader);
        }
        cband_si_type.push(t);
        base_scf_model.push(rd(&mut br, 3)? as u8);
        enh_scf_model.push(rd(&mut br, 3)? as u8);
        max_sfb_si_len.push(rd(&mut br, 4)? as u8);
    }

    // Table 4.36: general_header().
    let _reserved = rd(&mut br, 1)?;
    let window_sequence = match rd(&mut br, 2)? {
        0 => WindowSequence::OnlyLong,
        1 => WindowSequence::LongStart,
        2 => WindowSequence::EightShort,
        _ => WindowSequence::LongStop,
    };
    let window_shape = if rd(&mut br, 1)? != 0 {
        WindowShape::Kbd
    } else {
        WindowShape::Sine
    };
    let short = window_sequence == WindowSequence::EightShort;
    let (max_sfb, scale_factor_grouping) = if short {
        let m = rd(&mut br, 4)? as usize;
        let g = rd(&mut br, 7)? as u8;
        (m, g)
    } else {
        (rd(&mut br, 6)? as usize, 0)
    };
    let pns_data_present = rd(&mut br, 1)? != 0;
    let pns_start_sfb = if pns_data_present {
        rd(&mut br, 6)? as usize
    } else {
        0
    };
    let ms_mask_present = if nch == 2 { rd(&mut br, 2)? as u8 } else { 0 };
    let mut tns = Vec::with_capacity(nch);
    for _ in 0..nch {
        if rd(&mut br, 1)? != 0 {
            tns.push(Some(
                TnsData::parse(&mut br, window_sequence).map_err(|_| Error::BsacInvalidHeader)?,
            ));
        } else {
            tns.push(None);
        }
        // ltp_data_present.
        if rd(&mut br, 1)? != 0 {
            return Err(Error::BsacUnsupportedTool);
        }
    }
    let consumed = br.bit_position() as i64;
    // header_length escapes (§4.5.2.6.2.2.3): 1..=14 → (hl+7)
    // bytes; 0 / 15 → the byte-aligned actual length.
    let header_bits: i64 = if (1..=14).contains(&header_length) {
        (i64::from(header_length) + 7) * 8
    } else {
        (consumed + 7) / 8 * 8
    };
    if header_bits < (consumed + 7) / 8 * 8 || header_bits > (frame_length as i64) * 8 {
        return Err(Error::BsacInvalidHeader);
    }

    let geo = BsacGeometry::derive(
        fs,
        fs_index,
        window_sequence,
        scale_factor_grouping,
        max_sfb,
        nch,
        top_layer,
        base_band,
        header_bits,
        frame_length,
        &cband_si_type,
        &max_sfb_si_len,
    )?;

    let header = BsacHeader {
        frame_length,
        header_length,
        sba_mode,
        top_layer,
        base_snf_thr,
        max_scalefactor,
        base_band,
        cband_si_type,
        base_scf_model,
        enh_scf_model,
        max_sfb_si_len,
    };
    let general = GeneralHeader {
        window_sequence,
        window_shape,
        max_sfb,
        scale_factor_grouping,
        pns_data_present,
        pns_start_sfb,
        ms_mask_present,
        tns,
    };

    let ngroups = geo.num_window_groups;
    let mut ctx = BlockCtx {
        nch,
        geo,
        arith: ArithDecoder::new(),
        reader: SegmentReader::new(frame, 0, 0),
        avail: 0,
        cband_si: vec![Vec::new(); nch],
        lines: vec![Vec::new(); nch],
        scf: vec![vec![vec![None; max_sfb]; ngroups]; nch],
        stereo_side_info_coded: vec![vec![false; max_sfb]; ngroups],
        ms_used: vec![vec![false; max_sfb]; ngroups],
        stereo_info: vec![vec![0u8; max_sfb]; ngroups],
        is_position: vec![vec![0i32; max_sfb]; ngroups],
        header,
        general,
    };
    for ch in 0..nch {
        for g in 0..ngroups {
            let len = ctx.geo.group_len[g];
            ctx.cband_si[ch].push(vec![0u8; len.div_ceil(32)]);
            ctx.lines[ch].push(LineState {
                mask: vec![0; len],
                sign_neg: vec![false; len],
                sign_coded: vec![false; len],
                cur_snf: vec![0; len],
                unc_snf: vec![0; len],
            });
        }
    }

    // §4.6.4.3.3: ms_mask_present == 2 sets every ms_used without
    // decoding.
    if nch == 2 && ctx.general.ms_mask_present == 2 {
        for row in ctx.ms_used.iter_mut() {
            row.fill(true);
        }
    }

    if ctx.header.sba_mode {
        // SBA re-initializes the arithmetic code per segment; the
        // segment split + higher-spectra scheduling lands with an
        // SBA-bearing conformance vector.
        return Err(Error::BsacUnsupportedTool);
    }
    // Non-SBA: one arithmetic segment from the header end to the
    // frame end.
    ctx.reader = SegmentReader::new(frame, header_bits as u64, (frame_length as u64) * 8);
    ctx.arith = ArithDecoder::new();

    let total_layers = ctx.geo.layers.len();
    // Suffix sums of the static layer budgets: the Table 4.33
    // `data_available()` gate — an enhancement layer decodes only
    // while frame bits remain.
    let mut suffix_avail = vec![0i64; total_layers + 1];
    for k in (0..total_layers).rev() {
        suffix_avail[k] = suffix_avail[k + 1] + ctx.geo.layers[k].available_len;
    }
    // `prev_end[g]`: the highest end_index of any processed layer,
    // per group — the §4.5.2.6.2.2 lower-spectra region.
    let mut prev_end = vec![0usize; ngroups];
    let mut carry: i64 = -1; // segment start: 1 termination bit.
    #[allow(clippy::needless_range_loop)] // ctx.geo.layers cannot be
    // iterated while ctx is mutably borrowed inside the body.
    for layer_idx in 0..total_layers {
        let layer = ctx.geo.layers[layer_idx].clone();
        // Table 4.33: base sub-layers ride inside
        // bsac_base_element() unconditionally; enhancement layers
        // are gated on data_available().
        if layer_idx >= ctx.geo.slayer_size && carry + suffix_avail[layer_idx] <= 0 {
            break;
        }
        ctx.avail = carry + layer.available_len;
        // Side info.
        ctx.layer_cband_si(&layer)?;
        ctx.layer_sfb_si(layer_idx, &layer)?;
        // First pass: the layer's new spectra.
        let thr = if layer_idx < ctx.geo.slayer_size {
            i32::from(ctx.header.base_snf_thr)
        } else {
            0
        };
        let regions = [(layer.group, layer.start_index, layer.end_index)];
        ctx.spectral_data(&regions, thr, SnfKind::Cur);
        // Store cur_snf → unc_snf for the layer's range.
        for ch in 0..nch {
            let st = &mut ctx.lines[ch][layer.group];
            let e = layer.end_index.min(st.cur_snf.len());
            for i in layer.start_index..e {
                st.unc_snf[i] = st.cur_snf[i];
            }
        }
        // Secondary pass: refine every earlier line.
        let lower: Vec<(usize, usize, usize)> = (0..ngroups)
            .filter(|&g| prev_end[g] > 0)
            .map(|g| (g, 0, prev_end[g]))
            .collect();
        ctx.spectral_data(&lower, 0, SnfKind::Unc);
        prev_end[layer.group] = prev_end[layer.group].max(layer.end_index);
        carry = ctx.avail;
    }

    // Assemble the signed samples.
    let mut sample = vec![Vec::with_capacity(ngroups); nch];
    for (ch, sample_ch) in sample.iter_mut().enumerate().take(nch) {
        for g in 0..ngroups {
            let st = &ctx.lines[ch][g];
            let buf: Vec<i32> = st
                .mask
                .iter()
                .zip(st.sign_neg.iter())
                .map(|(&m, &neg)| {
                    let v = m as i32;
                    if neg {
                        -v
                    } else {
                        v
                    }
                })
                .collect();
            sample_ch.push(buf);
        }
    }
    Ok(DecodedBlock {
        header: ctx.header,
        general: ctx.general,
        sample,
        scf: ctx.scf,
        ms_used: ctx.ms_used,
        stereo_info: ctx.stereo_info,
        is_position: ctx.is_position,
        geometry: ctx.geo,
    })
}

/// Persistent ER BSAC stream decoder: one AU (`bsac_raw_data_block`)
/// in, one PCM frame out, carrying the §4.6.11 overlap-add state
/// across frames.
#[derive(Debug)]
pub struct BsacDecoder {
    fs: u32,
    fs_index: u8,
    nch: usize,
    filterbanks: Vec<Filterbank>,
}

impl BsacDecoder {
    /// A decoder for `nch` channels at `fs` Hz (Table 1.18 index
    /// `fs_index`).
    pub fn new(fs: u32, fs_index: u8, nch: usize) -> Result<Self> {
        if !(1..=2).contains(&nch) {
            return Err(Error::BsacInvalidHeader);
        }
        Ok(BsacDecoder {
            fs,
            fs_index,
            nch,
            filterbanks: (0..nch).map(|_| Filterbank::new()).collect(),
        })
    }

    /// Decode one access unit to interleaved 16-bit PCM
    /// (1024 samples per channel).
    pub fn decode_frame(&mut self, au: &[u8]) -> Result<Vec<i16>> {
        let block = decode_bsac_raw_data_block(au, self.fs, self.fs_index, self.nch)?;
        let spectra = reconstruct_spectra(&block, self.fs_index, self.nch)?;
        let info = block_ics_info(&block, self.fs_index)?;
        let mut channels = Vec::with_capacity(self.nch);
        for (ch, mut spec) in spectra.into_iter().enumerate() {
            if let Some(tns) = &block.general.tns[ch] {
                tns_decode_frame_ics(&mut spec, tns, &info, 22, self.fs_index)?;
            }
            let time = self.filterbanks[ch].synthesize(&spec, &info)?;
            channels.push(channel_to_s16(&time));
        }
        let mut out = Vec::with_capacity(BSAC_FRAME_LEN * self.nch);
        for i in 0..BSAC_FRAME_LEN {
            for chan in &channels {
                out.push(chan[i]);
            }
        }
        Ok(out)
    }

    /// Drop all cross-frame state (post-seek restart).
    pub fn reset(&mut self) {
        for fb in &mut self.filterbanks {
            *fb = Filterbank::new();
        }
    }
}

/// The `IcsInfo` equivalent of a decoded block (drives the shared
/// TNS / filterbank / stereo primitives).
fn block_ics_info(block: &DecodedBlock, fs_index: u8) -> Result<IcsInfo> {
    let short = block.general.window_sequence == WindowSequence::EightShort;
    let num_swb = if short {
        crate::ics_info::NUM_SWB_SHORT_WINDOW[fs_index as usize]
    } else {
        crate::ics_info::NUM_SWB_LONG_WINDOW[fs_index as usize]
    };
    Ok(IcsInfo {
        family: FrameFamily::Lc1024,
        ics_reserved_bit: false,
        window_sequence: block.general.window_sequence,
        window_shape: block.general.window_shape,
        max_sfb: block.general.max_sfb as u8,
        scale_factor_grouping: if short {
            Some(block.general.scale_factor_grouping)
        } else {
            None
        },
        predictor_data_present: false,
        predictor_data: None,
        ltp_data_present: false,
        ltp_data: None,
        ltp_data_present_pair: None,
        ltp_data_pair: None,
        num_windows: if short { 8 } else { 1 },
        num_window_groups: block.geometry.num_window_groups as u8,
        window_group_length: block.geometry.window_group_length.clone(),
        num_swb,
    })
}

/// Inverse-quantize + de-interleave one block into per-channel
/// window-major spectra, then run the §4.6.8.1 / §4.6.8.2 stereo
/// tools.
fn reconstruct_spectra(block: &DecodedBlock, fs_index: u8, nch: usize) -> Result<Vec<Vec<f64>>> {
    let geo = &block.geometry;
    let short = block.general.window_sequence == WindowSequence::EightShort;
    let max_sfb = block.general.max_sfb;
    let mut spectra = Vec::with_capacity(nch);
    for ch in 0..nch {
        let mut spec = vec![0.0f64; BSAC_FRAME_LEN];
        let mut window_base = 0usize; // first window of the group
        for g in 0..geo.num_window_groups {
            let wgl = geo.window_group_length[g] as usize;
            let buf = &block.sample[ch][g];
            for sfb in 0..max_sfb {
                let (s, e) = (geo.swb_offset[g][sfb], geo.swb_offset[g][sfb + 1]);
                let Some(scf) = block.scf[ch][g][sfb] else {
                    continue;
                };
                let gain = scale_factor_gain(scf);
                for (gi, &q) in buf.iter().enumerate().take(e.min(buf.len())).skip(s) {
                    if q == 0 {
                        continue;
                    }
                    let x = inverse_quantize(q) * gain;
                    let out_idx = if short {
                        // §4.5.2.6.2.6: within a group, 4-line
                        // chunks interleave across the group's
                        // windows: group index
                        // `4·(chunk·wgl + w) + j` carries window
                        // `w`'s line `4·chunk + j`.
                        let chunk = gi / (4 * wgl);
                        let rem = gi % (4 * wgl);
                        let w = rem / 4;
                        let j = rem % 4;
                        (window_base + w) * 128 + chunk * 4 + j
                    } else {
                        gi
                    };
                    spec[out_idx] = x;
                }
            }
            window_base += wgl;
        }
        spectra.push(spec);
    }

    if nch == 2 {
        let info = block_ics_info(block, fs_index)?;
        // Intensity stereo (stereo_info 2 / 3) reconstructs the
        // right channel from the left before the M/S de-matrix
        // (which skips intensity bands).
        let ms_present = match block.general.ms_mask_present {
            0 => MsMaskPresent::AllZeros,
            2 => MsMaskPresent::AllOnes,
            _ => MsMaskPresent::Mask,
        };
        // Per-band codebook shadows for the shared primitives:
        // intensity bands flag 15 (in phase) / 14 (out of phase) on
        // the right channel.
        let mut right_cb = vec![vec![1u8; max_sfb]; geo.num_window_groups];
        let mut is_pos = vec![vec![0i32; max_sfb]; geo.num_window_groups];
        let mut any_is = false;
        for g in 0..geo.num_window_groups {
            for sfb in 0..max_sfb {
                match block.stereo_info[g][sfb] {
                    2 => {
                        right_cb[g][sfb] = crate::section_data::INTENSITY_HCB;
                        is_pos[g][sfb] = block.is_position[g][sfb];
                        any_is = true;
                    }
                    3 => {
                        right_cb[g][sfb] = crate::section_data::INTENSITY_HCB2;
                        is_pos[g][sfb] = block.is_position[g][sfb];
                        any_is = true;
                    }
                    _ => {}
                }
            }
        }
        if any_is {
            let (left, right) = spectra.split_at_mut(1);
            let mut pair = crate::intensity_stereo::IntensityPairSpectra {
                left: &left[0],
                right: &mut right[0],
                right_sfb_cb: &right_cb,
                is_pos: &is_pos,
            };
            crate::intensity_stereo::apply_intensity_stereo(
                &mut pair,
                block.general.ms_mask_present != 0,
                &block.ms_used,
                &info,
                fs_index,
            )?;
        }
        let left_cb = vec![vec![1u8; max_sfb]; geo.num_window_groups];
        let (left, right) = spectra.split_at_mut(1);
        let mut pair = ChannelPairSpectra {
            left: &mut left[0],
            right: &mut right[0],
            left_sfb_cb: &left_cb,
            right_sfb_cb: &right_cb,
        };
        apply_ms_stereo(&mut pair, ms_present, &block.ms_used, &info, fs_index)?;
    }
    Ok(spectra)
}

//! BSAC fine-grain scalability layer geometry — ISO/IEC
//! 14496-3:2009 §4.5.2.6.2.4 / §4.5.2.6.2.5.
//!
//! A `bsac_raw_data_block()` is a stack of scalability layers: the
//! base layer (split into `slayer_size` sub-layers, one per base
//! coding band) followed by `top_layer` enhancement layers of
//! ~1 kbit/s/ch each. Every layer covers a slice of the spectrum
//! (`layer_start_index .. layer_end_index` in its window group), a
//! run of 32-line coding bands, a run of scalefactor bands whose
//! side info it carries, and a bit budget (`available_len`) cut out
//! of the frame by the §4.5.2.6.2.5 `layer_bit_offset` derivation.
//! [`BsacGeometry::derive`] computes the whole roster from the
//! header fields, transcribing the spec pseudo-code (including its
//! evident loop-variable typos, noted inline).

use crate::ics_info::WindowSequence;
use crate::swb_offset::{long_window_offsets, short_window_offsets};
use crate::{Error, Result};

/// Frame length of the 1024-line family this decoder covers.
pub const BSAC_FRAME_LEN: usize = 1024;

/// Short-window length.
const SHORT_LEN: usize = 128;

/// §4.5.2.6.2.5: `max_cband0_si_len` — the fixed maximum length of
/// the 0th coding band's side information.
const MAX_CBAND0_SI_LEN: u32 = 11;

/// One scalability layer's coverage and budget.
#[derive(Debug, Clone, Default)]
pub struct LayerInfo {
    /// `layer_group[layer]` — the window group whose spectrum the
    /// layer extends.
    pub group: usize,
    /// `layer_start_cband[layer]` .. `layer_end_cband[layer]`.
    pub start_cband: usize,
    /// Exclusive end coding band.
    pub end_cband: usize,
    /// `layer_start_index[layer]` .. `layer_end_index[layer]`
    /// (group-local spectral lines).
    pub start_index: usize,
    /// Exclusive end line.
    pub end_index: usize,
    /// `layer_start_sfb[layer]` .. `layer_end_sfb[layer]`.
    pub start_sfb: usize,
    /// Exclusive end scalefactor band.
    pub end_sfb: usize,
    /// `layer_si_maxlen[layer]` in bits.
    pub si_maxlen: u32,
    /// `layer_bit_offset[layer]` — the layer's first bit within the
    /// frame.
    pub bit_offset: i64,
    /// `available_len[layer]` in bits (before the segment-start
    /// `-1` termination adjustment, which the decode driver
    /// applies).
    pub available_len: i64,
    /// §4.6.4.6.3 `terminal_layer[layer]` — the layer ends an SBA
    /// segment (always true for the last layer).
    pub terminal: bool,
}

/// The §4.5.2.6.2.4 / §4.5.2.6.2.5 derived geometry for one
/// `bsac_raw_data_block()`.
#[derive(Debug, Clone)]
pub struct BsacGeometry {
    /// Number of window groups (1 for long sequences).
    pub num_window_groups: usize,
    /// Windows per group (sums to 8 for `EIGHT_SHORT`).
    pub window_group_length: Vec<u8>,
    /// Per-group scaled band offsets: `swb_offset[g][sfb] =
    /// swb_offset_window[sfb] · window_group_length[g]`, length
    /// `max_sfb + 1`.
    pub swb_offset: Vec<Vec<usize>>,
    /// Per-group group-buffer length (`1024` long, `wgl · 128`
    /// short).
    pub group_len: Vec<usize>,
    /// `last_index[g]` — the spectral cap from `max_sfb`.
    pub last_index: Vec<usize>,
    /// Number of base sub-layers.
    pub slayer_size: usize,
    /// The header's `top_layer`.
    pub top_layer: usize,
    /// Per-layer coverage/budget, `slayer_size + top_layer`
    /// entries.
    pub layers: Vec<LayerInfo>,
}

impl BsacGeometry {
    /// Derive the whole layer roster.
    ///
    /// * `fs` / `fs_index` — sampling frequency (Hz / Table 1.18
    ///   index).
    /// * `window_sequence` + `scale_factor_grouping` + `max_sfb` —
    ///   from `general_header()`.
    /// * `nch` — channels in the block (1 or 2).
    /// * `top_layer` / `base_band` — from `bsac_header()`.
    /// * `header_bits` — `layer_bit_offset[0]`, the total header
    ///   length in bits (byte-aligned).
    /// * `frame_length` — the frame length in bytes.
    /// * `cband_si_type` / `max_sfb_si_len` — per channel, from
    ///   `bsac_header()` (`max_sfb_si_len` raw, offset +5 applied
    ///   here).
    #[allow(clippy::too_many_arguments)]
    pub fn derive(
        fs: u32,
        fs_index: u8,
        window_sequence: WindowSequence,
        scale_factor_grouping: u8,
        max_sfb: usize,
        nch: usize,
        top_layer: usize,
        base_band: usize,
        header_bits: i64,
        frame_length: usize,
        cband_si_type: &[u8],
        max_sfb_si_len: &[u8],
    ) -> Result<Self> {
        // §4.5.2.6.2.4 grouping (identical to the AAC derivation).
        let short = window_sequence == WindowSequence::EightShort;
        let (num_window_groups, window_group_length) = if short {
            let mut wgl: Vec<u8> = vec![1];
            for i in 0..7 {
                if (scale_factor_grouping >> (6 - i)) & 1 == 0 {
                    wgl.push(1);
                } else {
                    *wgl.last_mut().unwrap() += 1;
                }
            }
            (wgl.len(), wgl)
        } else {
            (1, vec![1u8])
        };
        let window_offsets: &[u16] = if short {
            short_window_offsets(fs_index)?
        } else {
            long_window_offsets(fs_index)?
        };
        if max_sfb + 1 > window_offsets.len() {
            return Err(Error::BsacInvalidHeader);
        }
        let mut swb_offset = Vec::with_capacity(num_window_groups);
        let mut group_len = Vec::with_capacity(num_window_groups);
        let mut last_index = Vec::with_capacity(num_window_groups);
        for &wgl_u8 in window_group_length.iter().take(num_window_groups) {
            let wgl = wgl_u8 as usize;
            let offsets: Vec<usize> = (0..=max_sfb)
                .map(|sfb| window_offsets[sfb] as usize * if short { wgl } else { 1 })
                .collect();
            last_index.push(offsets[max_sfb]);
            swb_offset.push(offsets);
            group_len.push(if short {
                wgl * SHORT_LEN
            } else {
                BSAC_FRAME_LEN
            });
        }

        // §4.5.2.6.2.5: slayer_size + per-group base band limit.
        let mut end_index = vec![0usize; num_window_groups];
        let mut end_cband = vec![0usize; num_window_groups];
        let mut slayer_size = 0usize;
        for g in 0..num_window_groups {
            if short {
                let wgl = window_group_length[g] as usize;
                let mut ei = base_band * 4 * wgl;
                if fs == 44_100 || fs == 48_000 {
                    if ei % 32 >= 16 {
                        ei = ei / 32 * 32 + 20;
                    } else if ei % 32 >= 4 {
                        ei = ei / 32 * 32 + 8;
                    }
                } else if fs == 22_050 || fs == 24_000 || fs == 32_000 {
                    ei = ei / 16 * 16;
                } else if fs == 11_025 || fs == 12_000 || fs == 16_000 {
                    ei = ei / 32 * 32;
                } else {
                    ei = ei / 64 * 64;
                }
                end_index[g] = ei;
                end_cband[g] = ei.div_ceil(32);
            } else {
                end_cband[g] = base_band;
            }
            slayer_size += end_cband[g];
        }
        if slayer_size == 0 {
            return Err(Error::BsacInvalidHeader);
        }

        let total_layers = slayer_size + top_layer;
        let mut layers = vec![LayerInfo::default(); total_layers];

        // layer_group[]: base sub-layers walk the groups' cbands in
        // order; enhancement layers cycle through the groups
        // window-by-window (period `num_windows` — 8 for short, 1
        // for long; the spec writes the period-8 copy explicitly).
        {
            let mut layer = 0usize;
            for (g, &nc) in end_cband.iter().enumerate().take(num_window_groups) {
                for _ in 1..=nc {
                    layers[layer].group = g;
                    layer += 1;
                }
            }
            let mut seq = Vec::new();
            for (g, &wgl) in window_group_length
                .iter()
                .enumerate()
                .take(num_window_groups)
            {
                for _ in 0..wgl {
                    seq.push(g);
                }
            }
            for (k, layer) in layers.iter_mut().enumerate().skip(slayer_size) {
                layer.group = seq[(k - slayer_size) % seq.len()];
            }
        }

        // Base sub-layers: one coding band each.
        {
            let mut layer = 0usize;
            let mut end_index_run = vec![0usize; num_window_groups];
            for (g, &nc) in end_cband.iter().enumerate().take(num_window_groups) {
                for cband in 0..nc {
                    layers[layer].start_cband = cband;
                    layers[layer].end_cband = cband + 1;
                    layers[layer].start_index = cband * 32;
                    layers[layer].end_index = (cband + 1) * 32;
                    end_index_run[g] = (cband + 1) * 32;
                    layer += 1;
                }
            }
            // Enhancement layers extend the band limit at the
            // rate-dependent §4.5.2.6.2.5 step.
            let mut end_cband_run = end_cband.clone();
            let mut end_index_g = end_index_run;
            for layer_info in layers.iter_mut().skip(slayer_size) {
                let g = layer_info.group;
                layer_info.start_index = end_index_g[g];
                let mut ei = end_index_g[g];
                if fs == 44_100 || fs == 48_000 {
                    if ei % 32 == 0 {
                        ei += 8;
                    } else {
                        ei += 12;
                    }
                } else if fs == 22_050 || fs == 24_000 || fs == 32_000 {
                    ei += 16;
                } else if fs == 11_025 || fs == 12_000 || fs == 16_000 {
                    ei += 32;
                } else {
                    ei += 64;
                }
                if ei > last_index[g] {
                    ei = last_index[g];
                }
                end_index_g[g] = ei;
                layer_info.end_index = ei;
                layer_info.start_cband = end_cband_run[g];
                end_cband_run[g] = ei.div_ceil(32);
                layer_info.end_cband = end_cband_run[g];
            }
        }

        // layer_start_sfb / layer_end_sfb (transcribed literally,
        // `layer_end_sfb = sfb + 1` at the first band whose start
        // offset reaches the layer's end index).
        {
            let mut end_sfb = vec![0usize; num_window_groups];
            for layer_info in layers.iter_mut() {
                let g = layer_info.group;
                layer_info.start_sfb = end_sfb[g];
                layer_info.end_sfb = max_sfb;
                for (sfb, &off) in swb_offset[g].iter().enumerate().take(max_sfb) {
                    if layer_info.end_index <= off {
                        // Transcribed literally (`= sfb + 1`); the
                        // one-band lookahead is corpus-confirmed —
                        // the `= sfb` reading desyncs the arithmetic
                        // stream on frames that the `+ 1` reading
                        // decodes exactly.
                        layer_info.end_sfb = sfb + 1;
                        break;
                    }
                }
                end_sfb[g] = layer_info.end_sfb;
            }
        }

        // layer_si_maxlen.
        for layer_info in layers.iter_mut() {
            let mut si = 0u32;
            for cband in layer_info.start_cband..layer_info.end_cband {
                for &cst in cband_si_type.iter().take(nch) {
                    if cband == 0 {
                        si += MAX_CBAND0_SI_LEN;
                    } else {
                        si += u32::from(
                            crate::bsac_tables::CBAND_SI_TYPES
                                .get(cst as usize)
                                .ok_or(Error::BsacInvalidHeader)?
                                .max_len,
                        );
                    }
                }
            }
            for _sfb in layer_info.start_sfb..layer_info.end_sfb {
                for &msl in max_sfb_si_len.iter().take(nch) {
                    si += u32::from(msl) + 5;
                }
            }
            layer_info.si_maxlen = si;
        }

        // layer_bit_offset: rate anchors for the enhancement
        // layers, then top-down si-budget adjustments, the base
        // sub-layer split, and the header overflow/underflow
        // redistribution — §4.5.2.6.2.5, transcribed with the
        // evident typos fixed (`slayer--` for `layer--`, `layer <=`
        // for `m <=`).
        let frame_bits = (frame_length as i64) * 8;
        let mut bit_offset = vec![0i64; total_layers + 1];
        for (k, off) in bit_offset
            .iter_mut()
            .enumerate()
            .take(total_layers + 1)
            .skip(slayer_size)
        {
            let layer_bitrate = (nch as i64) * (((k - slayer_size) as i64) * 1000 + 16_000);
            let mut v = layer_bitrate * (BSAC_FRAME_LEN as i64);
            v = v / (fs as i64) / 8 * 8;
            *off = v.min(frame_bits);
        }
        // The frame may carry more bytes than the top layer's
        // nominal rate anchor (the encoder's bit reservoir); the
        // stream end is the frame end, so the last boundary extends
        // to it — the slack feeds the top layer's secondary
        // (refinement) pass.
        bit_offset[total_layers] = frame_bits;
        for k in (slayer_size..total_layers).rev() {
            let candidate = bit_offset[k + 1] - i64::from(layers[k].si_maxlen);
            if candidate < bit_offset[k] {
                bit_offset[k] = candidate;
            }
        }
        for k in (0..slayer_size).rev() {
            bit_offset[k] = bit_offset[k + 1] - i64::from(layers[k].si_maxlen);
        }
        let overflow = header_bits - bit_offset[0];
        bit_offset[0] = header_bits;
        if overflow > 0 {
            let mut overflow = overflow;
            for k in (slayer_size..total_layers).rev() {
                let mut layer_bit_size = bit_offset[k + 1] - bit_offset[k];
                layer_bit_size -= i64::from(layers[k].si_maxlen);
                if layer_bit_size >= overflow {
                    layer_bit_size = overflow;
                    overflow = 0;
                } else {
                    overflow -= layer_bit_size;
                }
                for off in bit_offset.iter_mut().take(k + 1).skip(1) {
                    *off += layer_bit_size;
                }
                if overflow <= 0 {
                    break;
                }
            }
        } else {
            let underflow = -overflow;
            let share = underflow / (slayer_size as i64);
            let extra = underflow % (slayer_size as i64);
            for m in 1..slayer_size {
                bit_offset[m] = bit_offset[m - 1] + i64::from(layers[m - 1].si_maxlen) + share;
                if (m as i64) <= extra {
                    bit_offset[m] += 1;
                }
            }
        }
        for (k, layer_info) in layers.iter_mut().enumerate() {
            layer_info.bit_offset = bit_offset[k];
            layer_info.available_len = bit_offset[k + 1] - bit_offset[k];
        }

        // §4.6.4.6.3 terminal_layer[]: a layer ends its segment when
        // the next layer starts a different coding band run; the
        // last layer always terminates.
        for k in 0..total_layers {
            layers[k].terminal = if k + 1 < total_layers {
                layers[k].start_cband != layers[k + 1].start_cband
            } else {
                true
            };
        }

        Ok(BsacGeometry {
            num_window_groups,
            window_group_length,
            swb_offset,
            group_len,
            last_index,
            slayer_size,
            top_layer,
            layers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 48 kHz mono long-window geometry with the header values of
    /// a real conformance frame (`top_layer = 48`, `base_band =
    /// 10`): the base splits into 10 sub-layers of one coding band,
    /// enhancement layers extend by the 8/12-line 48 kHz step, and
    /// the budgets partition the frame exactly.
    #[test]
    fn long_mono_layer_roster() {
        let geo = BsacGeometry::derive(
            48_000,
            3,
            WindowSequence::OnlyLong,
            0,
            40,
            1,
            48,
            10,
            72,
            171,
            &[27],
            &[0],
        )
        .unwrap();
        assert_eq!(geo.slayer_size, 10);
        assert_eq!(geo.layers.len(), 58);
        // Base sub-layers: one 32-line cband each.
        for (k, l) in geo.layers.iter().take(10).enumerate() {
            assert_eq!(l.group, 0);
            assert_eq!((l.start_cband, l.end_cband), (k, k + 1));
            assert_eq!((l.start_index, l.end_index), (32 * k, 32 * k + 32));
        }
        // First enhancement layer starts at the base band limit.
        assert_eq!(geo.layers[10].start_index, 320);
        assert_eq!(geo.layers[10].end_index, 328);
        assert_eq!(geo.layers[11].start_index, 328);
        assert_eq!(geo.layers[11].end_index, 340);
        // Budgets tile the frame: offsets ascend and the last layer
        // ends at or before the frame end.
        for w in geo.layers.windows(2) {
            assert_eq!(w[0].bit_offset + w[0].available_len, w[1].bit_offset);
        }
        let last = geo.layers.last().unwrap();
        assert!(last.bit_offset + last.available_len <= 171 * 8);
        assert_eq!(geo.layers[0].bit_offset, 72);
        // sfb coverage is monotone and capped.
        for l in &geo.layers {
            assert!(l.start_sfb <= l.end_sfb && l.end_sfb <= 40);
        }
        // Non-SBA streams still mark segment boundaries; the last
        // layer always terminates.
        assert!(geo.layers.last().unwrap().terminal);
    }

    /// The short-window 48 kHz band-limit rounding of
    /// §4.5.2.6.2.5 (`% 32 >= 16 → +20`, `% 32 >= 4 → +8`).
    #[test]
    fn short_window_base_band_rounding() {
        let geo = BsacGeometry::derive(
            48_000,
            3,
            WindowSequence::EightShort,
            0, // 8 groups of 1 window
            14,
            1,
            8,
            10,
            72,
            400,
            &[5],
            &[2],
        )
        .unwrap();
        assert_eq!(geo.num_window_groups, 8);
        // base_band·4·1 = 40 → 40 % 32 = 8 (>= 4) → 32 + 8 = 40.
        assert_eq!(geo.layers[0].end_index, 32);
        // Each group contributes ceil(40/32) = 2 sub-layers.
        assert_eq!(geo.slayer_size, 16);
        for (k, l) in geo.layers.iter().take(16).enumerate() {
            assert_eq!(l.group, k / 2);
            assert_eq!(l.start_cband, k % 2);
        }
        // Enhancement layers cycle the 8 groups round-robin.
        for k in 0..8 {
            assert_eq!(geo.layers[16 + k].group, k);
        }
    }
}

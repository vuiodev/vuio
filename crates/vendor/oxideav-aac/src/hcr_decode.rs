//! `reordered_spectral_data()` payload codec — ISO/IEC 14496-3
//! §4.6.16.3.3 / §4.6.16.3.4: the Huffman-codeword-reordering (HCR)
//! bitstream payload, both directions.
//!
//! The [`crate::hcr`] module owns the deterministic geometry half of
//! the tool (the Table 4.170 `maxCwLen` table, the pre-sorting metric,
//! the [`crate::hcr::Segmentation`] layout, and the
//! [`crate::hcr::ReorderPlan`] writing-scheme walk). This module binds
//! that geometry to the actual spectral payload:
//!
//! * [`encode_reordered_spectral_data`] — the §4.6.16.3.3.4
//!   `ReorderSpectralData()` encoder: enumerate the frame's codewords
//!   in §4.6.16.3.3.1 pre-sorted order, Huffman-encode each (the
//!   codeword plus sign bits plus escape sequences: the §4.5.2.3.2 HCR
//!   codeword unit), and scatter the bits over the segment grid with the
//!   PCW-then-non-PCW set / trial loop.
//! * [`decode_reordered_spectral_data`] — the §4.6.16.3.4 decode: the
//!   inverse walk. Codeword lengths are *not* transmitted; the PCWs
//!   are decoded first, each from the start of its own segment (the
//!   §4.6.16.3.3.2 `segmentWidth ≥` every same-book codeword
//!   guarantees they fit), then the non-PCW sets are decoded through
//!   the same trial loop the writer used — a codeword consumes bits
//!   from a segment's free region (in the set's direction) until its
//!   Huffman unit completes or the segment exhausts, in which case its
//!   remainder continues in the next trial's segment. Because every
//!   spectrum codebook is a complete prefix code, an incomplete bit
//!   prefix is exactly distinguishable (bit-source underflow) from a
//!   completed codeword, so the decoder discovers each codeword's
//!   length precisely where the writer defined it.
//!
//! ## Codeword enumeration and pre-sorting (§4.6.16.3.3.1)
//!
//! A *unit* covers four spectral lines of one window: one 4-D codeword
//! or two 2-D codewords in natural (ascending-frequency) order. Unit
//! groups are collected ascending in spectral direction with the
//! windows of one spectral region in temporal order (the unit-based
//! window interleaving of Table 4.169 — §4.5.2.3.5 grouping interleave
//! does *not* apply under HCR), then stably ordered by the
//! `assignedUnitNr` metric (codebook priority first). The §4.6.16.4
//! virtual codebooks 16..=31 carry ordinary codebook-11 spectrum (their
//! `maxCwLen` differs for the segment widths only).
//!
//! ## Provenance
//!
//! Everything follows the §4.6.16.3 text and pseudocode plus the
//! §4.5.2.3.2 codeword-unit definition ("the whole data necessary to
//! decode two or four lines … includes Huffman codeword, sign bits,
//! and escape sequences"), staged under `docs/audio/aac/`. No external
//! HCR implementation was consulted.

use oxideav_core::bits::{BitReader, BitWriter};

use crate::hcr::{assigned_unit_nr, segment_width, Direction, Segmentation};
use crate::ics_info::IcsInfo;
use crate::section_data::SectionData;
use crate::spectral_data::{
    decode_codeword, read_and_apply_signs, read_escape_sequence, write_tuple, SpectralData,
};
#[cfg(test)]
use crate::swb_offset::{long_window_offsets, short_window_offsets};
use crate::swb_offset::{LONG_WINDOW_LEN, SHORT_WINDOW_LEN};
use crate::{Error, Result};

/// The `ESC_FLAG` magnitude of the escape book (§4.6.3.3).
const ESC_FLAG: i32 = 16;

/// One HCR codeword in pre-sorted order: the §4.5.2.3.2 unit of "the
/// whole data necessary to decode two or four lines".
#[derive(Debug, Clone, Copy)]
struct HcrCodeword {
    /// The section codebook (1..=11 spectrum books, or a §4.6.16.4
    /// virtual codebook 16..=31) — drives the segment width.
    sect_cb: u8,
    /// The codebook whose Huffman tables encode the lines (the virtual
    /// codebooks decode as book 11).
    decode_cb: u8,
    /// Tuple dimension: 4 (books 1..=4) or 2.
    dim: usize,
    /// Window group index.
    group: usize,
    /// Index within the group's transmission-order buffer of the first
    /// line of this codeword.
    buf_index: usize,
}

/// Enumerate the frame's spectral codewords in §4.6.16.3.3.1
/// pre-sorted order.
///
/// Walks every window of every group over the active scalefactor bands
/// (`sfb_cb[g][sfb]`), skipping the spectrum-less books (`ZERO`,
/// `NOISE`, intensity), and sorts stably by the `assignedUnitNr`
/// metric. `buf_index` targets the §4.5.2.3.5 transmission-order group
/// buffer layout [`SpectralData`] uses (sfb-major, window-in-group,
/// line), which keeps the rest of the decode chain unchanged.
fn enumerate_presorted(
    ics_info: &IcsInfo,
    section_data: &SectionData,
    fs_index: u8,
) -> Result<Vec<HcrCodeword>> {
    let short = ics_info.window_sequence.is_eight_short();
    let window_len = ics_info.window_len()?;
    let offsets = ics_info.swb_offsets(fs_index)?;
    let max_lines = window_len as u32;
    let max_windows: u32 = if short { 8 } else { 1 };
    let max_sfb = usize::from(ics_info.max_sfb);
    if max_sfb > offsets.len() - 1 {
        return Err(Error::SpectralDataInvalid);
    }
    if section_data.sfb_cb.len() != usize::from(ics_info.num_window_groups) {
        return Err(Error::SpectralDataInvalid);
    }

    let mut cws: Vec<(u32, HcrCodeword)> = Vec::new();
    let mut window_base = 0usize; // absolute index of the group's first window
    for (g, cb_row) in section_data.sfb_cb.iter().enumerate() {
        if cb_row.len() < max_sfb {
            return Err(Error::SpectralDataInvalid);
        }
        let wgl = usize::from(ics_info.window_group_length[g]);
        // Transmission-order offset of band `sfb` for window-in-group
        // `b`: sum over earlier bands of `wgl · width`, plus
        // `b · width(sfb)`.
        let mut band_base = 0usize;
        for sfb in 0..max_sfb {
            let start = usize::from(offsets[sfb]);
            let end = usize::from(offsets[sfb + 1]);
            let width = end - start;
            let cb = cb_row[sfb];
            let spec = classify_hcr(cb)?;
            if let Some((decode_cb, dim)) = spec {
                for b in 0..wgl {
                    let window = (window_base + b) as u32;
                    for line_off in (0..width).step_by(dim) {
                        let line = (start + line_off) as u32;
                        // A unit covers four lines; both 2-D codewords
                        // of one unit share its assignedUnitNr and keep
                        // their natural order (stable sort below).
                        let unit_line = line & !3;
                        let key = assigned_unit_nr(cb, max_lines, unit_line, max_windows, window);
                        cws.push((
                            key,
                            HcrCodeword {
                                sect_cb: cb,
                                decode_cb,
                                dim,
                                group: g,
                                buf_index: band_base + b * width + line_off,
                            },
                        ));
                    }
                }
            }
            band_base += wgl * width;
        }
        window_base += wgl;
    }

    cws.sort_by_key(|&(key, _)| key);
    Ok(cws.into_iter().map(|(_, cw)| cw).collect())
}

/// Classify a section codebook for HCR: `None` for the spectrum-less
/// books, `(decode_cb, dim)` for the spectrum books, an error for the
/// reserved book 12.
fn classify_hcr(cb: u8) -> Result<Option<(u8, usize)>> {
    match cb {
        0 | 13 | 14 | 15 => Ok(None),
        1..=4 => Ok(Some((cb, 4))),
        5..=11 => Ok(Some((cb, 2))),
        // §4.6.16.4 virtual codebooks: ordinary book-11 spectrum with
        // a limited value range (the limit shapes maxCwLen only).
        16..=31 => Ok(Some((11, 2))),
        _ => Err(Error::SpectralDataInvalid),
    }
}

/// Encode one codeword unit (Huffman codeword + sign bits + escape
/// sequences) to a fresh bit vector.
fn encode_codeword_bits(cw: &HcrCodeword, values: &[i32]) -> Result<(Vec<u8>, u32)> {
    let mut w = BitWriter::new();
    write_tuple(&mut w, cw.decode_cb, cw.dim, values)?;
    let bits = w.bit_position() as u32;
    Ok((w.finish(), bits))
}

/// §4.6.16.3.3.4 `ReorderSpectralData()` — encode a frame's spectrum
/// as a `reordered_spectral_data()` payload.
///
/// Returns `(payload_bytes, length_of_reordered_spectral_data,
/// length_of_longest_codeword)`. The payload length is exactly the sum
/// of the codeword lengths (the writer transmits no slack), stored
/// MSB-first.
///
/// `spectral` must use the same transmission-order layout
/// [`SpectralData::write`] consumes; `section_data.sfb_cb` may carry
/// §4.6.16.4 virtual codebooks (16..=31).
pub fn encode_reordered_spectral_data(
    spectral: &SpectralData,
    ics_info: &IcsInfo,
    section_data: &SectionData,
    fs_index: u8,
) -> Result<(Vec<u8>, u16, u8)> {
    let cws = enumerate_presorted(ics_info, section_data, fs_index)?;
    if spectral.x_quant.len() != usize::from(ics_info.num_window_groups) {
        return Err(Error::SpectralDataEncodeInvalid);
    }

    // Encode every codeword unit to its bit string.
    let mut encoded: Vec<(Vec<u8>, u32)> = Vec::with_capacity(cws.len());
    let mut longest = 0u32;
    for cw in &cws {
        let buf = spectral
            .x_quant
            .get(cw.group)
            .ok_or(Error::SpectralDataEncodeInvalid)?;
        let vals = buf
            .get(cw.buf_index..cw.buf_index + cw.dim)
            .ok_or(Error::SpectralDataEncodeInvalid)?;
        let e = encode_codeword_bits(cw, vals)?;
        longest = longest.max(e.1);
        encoded.push(e);
    }
    if longest > 49 {
        // §4.6.16.3.2: valid lengths are 0..=49; the codeword units of
        // the spectrum books never exceed this by construction.
        return Err(Error::SpectralDataEncodeInvalid);
    }
    let total_bits: u32 = encoded.iter().map(|e| e.1).sum();
    if total_bits > 12288 {
        return Err(Error::SpectralDataEncodeInvalid);
    }

    // Segment grid + the writing-scheme bit placement.
    let widths: Vec<u8> = cws
        .iter()
        .map(|cw| segment_width(cw.sect_cb, longest as u8))
        .collect();
    let seg = Segmentation::new(&widths, total_bits);
    let lengths: Vec<u32> = encoded.iter().map(|e| e.1).collect();
    let plan =
        crate::hcr::ReorderPlan::build(&lengths, &seg).ok_or(Error::SpectralDataEncodeInvalid)?;

    // Scatter the codeword bits to their planned buffer positions.
    let mut out = vec![0u8; (total_bits as usize).div_ceil(8)];
    for (c, (bytes, len)) in encoded.iter().enumerate() {
        for bit in 0..*len {
            let set = bytes[(bit / 8) as usize] & (0x80 >> (bit % 8)) != 0;
            if set {
                let pos = plan.codeword_bits[c][bit as usize];
                out[(pos / 8) as usize] |= 0x80 >> (pos % 8);
            }
        }
    }
    Ok((out, total_bits as u16, longest as u8))
}

/// The per-segment cursor pair of the §4.6.16.3.3.3 walk: bits
/// consumed from the low (forward) and high (backward) ends.
struct SegCursor {
    start: u32,
    width: u32,
    low: u32,
    high: u32,
}

impl SegCursor {
    fn free(&self) -> u32 {
        self.width - self.low - self.high
    }

    /// Collect the segment's free bits in `dir` order (the order the
    /// writer would have placed a codeword's bits).
    fn free_bits(&self, payload: &[u8], dir: Direction) -> Vec<bool> {
        let read = |local: u32| {
            let pos = self.start + local;
            payload[(pos / 8) as usize] & (0x80 >> (pos % 8)) != 0
        };
        match dir {
            Direction::Forward => (self.low..self.width - self.high).map(read).collect(),
            Direction::Backward => (self.low..self.width - self.high).rev().map(read).collect(),
        }
    }

    /// Consume `n` bits from the `dir` end.
    fn consume(&mut self, n: u32, dir: Direction) {
        match dir {
            Direction::Forward => self.low += n,
            Direction::Backward => self.high += n,
        }
    }
}

/// The in-flight decode state of one codeword: the bits gathered so
/// far and, once complete, the decoded lines.
struct CodewordState {
    bits: Vec<bool>,
    done: bool,
    values: [i32; 4],
}

/// Try to decode a whole codeword unit from `bits`. Returns
/// `Ok(Some((consumed_bits, values)))` when the unit completes within
/// `bits`, `Ok(None)` when more bits are needed (bit-source
/// underflow), or a hard error for a genuinely invalid unit.
fn try_decode_unit(cw: &HcrCodeword, bits: &[bool]) -> Result<Option<(u32, [i32; 4])>> {
    // Pack MSB-first.
    let mut bytes = vec![0u8; bits.len().div_ceil(8)];
    for (i, &b) in bits.iter().enumerate() {
        if b {
            bytes[i / 8] |= 0x80 >> (i % 8);
        }
    }
    let mut r = BitReader::new(&bytes);
    // Mirror the SpectralData::parse per-tuple sequence: hcod → sign
    // bits → escape sequences.
    let step = (|| -> Result<[i32; 4]> {
        let idx = decode_codeword(&mut r, cw.decode_cb)?;
        let tuple = crate::spectral_codebook::decode_index_to_tuple(cw.decode_cb, idx)?;
        let mut tuple = read_and_apply_signs(&mut r, cw.decode_cb, cw.dim, tuple)?;
        if cw.decode_cb == 11 {
            for v in tuple.iter_mut().take(cw.dim) {
                if v.abs() == ESC_FLAG {
                    let mag = read_escape_sequence(&mut r)? as i32;
                    *v = if *v < 0 { -mag } else { mag };
                }
            }
        }
        Ok(tuple)
    })();
    match step {
        Ok(tuple) => {
            let consumed = r.bit_position() as u32;
            if consumed as usize > bits.len() {
                // The packed byte buffer is padded to a byte boundary;
                // a "completion" that consumed padding bits is phantom
                // — the genuine continuation bits arrive in a later
                // trial's segment.
                return Ok(None);
            }
            Ok(Some((consumed, tuple)))
        }
        Err(Error::UnexpectedEnd) => Ok(None),
        Err(e) => Err(e),
    }
}

/// §4.6.16.3.4 — decode a `reordered_spectral_data()` payload back to
/// the transmission-order [`SpectralData`].
///
/// * `payload` — the reordered buffer, MSB-first;
///   `length_of_reordered_spectral_data` (already clamped per
///   §4.6.16.3.2 by the caller if reserved) selects the bit count.
/// * `length_of_longest_codeword` — the transmitted 6-bit field
///   (clamped internally per §4.6.16.3.2).
///
/// The decode runs the exact §4.6.16.3.3.4 walk with the codeword
/// lengths discovered by Huffman completion; see the module notes.
pub fn decode_reordered_spectral_data(
    payload: &[u8],
    length_of_reordered_spectral_data: u16,
    length_of_longest_codeword: u8,
    ics_info: &IcsInfo,
    section_data: &SectionData,
    fs_index: u8,
) -> Result<SpectralData> {
    let total_bits = u32::from(length_of_reordered_spectral_data);
    if (payload.len() as u32) * 8 < total_bits {
        return Err(Error::UnexpectedEnd);
    }
    let cws = enumerate_presorted(ics_info, section_data, fs_index)?;

    // Segment grid, exactly as the writer derived it.
    let widths: Vec<u8> = cws
        .iter()
        .map(|cw| segment_width(cw.sect_cb, length_of_longest_codeword))
        .collect();
    let seg = Segmentation::new(&widths, total_bits);
    let num_segments = seg.number_of_segments();
    if num_segments == 0 {
        if cws.is_empty() {
            return empty_spectral(ics_info);
        }
        return Err(Error::SpectralDataInvalid);
    }

    let mut cursors: Vec<SegCursor> = (0..num_segments)
        .map(|s| SegCursor {
            start: seg.segment_start(s),
            width: seg.segment_bits[s],
            low: 0,
            high: 0,
        })
        .collect();
    let mut states: Vec<CodewordState> = cws
        .iter()
        .map(|_| CodewordState {
            bits: Vec::new(),
            done: false,
            values: [0; 4],
        })
        .collect();

    // Feed a codeword from one segment: append free bits, try to
    // complete; consume what the codeword actually used (all free bits
    // if it is still incomplete).
    let feed = |state: &mut CodewordState,
                cw: &HcrCodeword,
                cursor: &mut SegCursor,
                dir: Direction|
     -> Result<()> {
        if state.done || cursor.free() == 0 {
            return Ok(());
        }
        let already = state.bits.len() as u32;
        let fresh = cursor.free_bits(payload, dir);
        state.bits.extend_from_slice(&fresh);
        match try_decode_unit(cw, &state.bits)? {
            Some((consumed, values)) => {
                if consumed < already {
                    return Err(Error::SpectralDataInvalid);
                }
                cursor.consume(consumed - already, dir);
                state.bits.truncate(consumed as usize);
                state.values = values;
                state.done = true;
            }
            None => {
                // Uses every free bit of this segment and continues.
                cursor.consume(fresh.len() as u32, dir);
            }
        }
        Ok(())
    };

    // First step: decode PCWs (set 0), codeword i forward from segment i.
    for i in 0..num_segments.min(cws.len()) {
        feed(&mut states[i], &cws[i], &mut cursors[i], Direction::Forward)?;
        if !states[i].done {
            // A PCW always fits its own segment (§4.6.16.3.3.2); not
            // completing means the stream is corrupt.
            return Err(Error::SpectralDataInvalid);
        }
    }

    // Second step: the non-PCW sets with the per-set direction toggle
    // and the modulo-shift trial loop.
    let num_sets = cws.len().div_ceil(num_segments);
    let mut direction = Direction::Forward;
    for set in 1..num_sets {
        direction = direction.toggled();
        for trial in 0..num_segments {
            for codeword_base in 0..num_segments {
                let segment = (trial + codeword_base) % num_segments;
                let codeword = codeword_base + set * num_segments;
                if codeword >= cws.len() {
                    continue;
                }
                feed(
                    &mut states[codeword],
                    &cws[codeword],
                    &mut cursors[segment],
                    direction,
                )?;
            }
        }
        // §4.6.16.3.3.3: after at most N trials every codeword of the
        // set is complete on a conforming stream.
        for base in 0..num_segments {
            let codeword = base + set * num_segments;
            if codeword < cws.len() && !states[codeword].done {
                return Err(Error::SpectralDataInvalid);
            }
        }
    }

    // Scatter the decoded lines into the transmission-order buffers.
    let mut spectral = empty_spectral(ics_info)?;
    for (cw, state) in cws.iter().zip(states.iter()) {
        let buf = spectral
            .x_quant
            .get_mut(cw.group)
            .ok_or(Error::SpectralDataInvalid)?;
        let dst = buf
            .get_mut(cw.buf_index..cw.buf_index + cw.dim)
            .ok_or(Error::SpectralDataInvalid)?;
        dst.copy_from_slice(&state.values[..cw.dim]);
    }
    Ok(spectral)
}

/// An all-zero transmission-order [`SpectralData`] with the group
/// buffer geometry of `ics_info`.
fn empty_spectral(ics_info: &IcsInfo) -> Result<SpectralData> {
    let short = ics_info.window_sequence.is_eight_short();
    let x_quant = ics_info
        .window_group_length
        .iter()
        .map(|&wgl| {
            let len = if short {
                usize::from(wgl) * SHORT_WINDOW_LEN as usize
            } else {
                LONG_WINDOW_LEN as usize
            };
            vec![0i32; len]
        })
        .collect();
    Ok(SpectralData { x_quant })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ics_info::{WindowSequence, WindowShape, NUM_SWB_LONG_WINDOW};
    use crate::section_data::Section;

    const FS: u8 = 4; // 44.1 kHz

    fn long_ics(max_sfb: u8) -> IcsInfo {
        IcsInfo {
            family: crate::swb_offset::FrameFamily::Lc1024,
            ics_reserved_bit: false,
            window_sequence: WindowSequence::OnlyLong,
            window_shape: WindowShape::Sine,
            max_sfb,
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
            num_swb: NUM_SWB_LONG_WINDOW[FS as usize],
        }
    }

    /// An `EIGHT_SHORT` ics_info with two groups (3 + 5 windows).
    fn short_ics(max_sfb: u8) -> IcsInfo {
        IcsInfo {
            family: crate::swb_offset::FrameFamily::Lc1024,
            ics_reserved_bit: false,
            window_sequence: WindowSequence::EightShort,
            window_shape: WindowShape::Sine,
            max_sfb,
            scale_factor_grouping: Some(0),
            predictor_data_present: false,
            predictor_data: None,
            ltp_data_present: false,
            ltp_data: None,
            ltp_data_present_pair: None,
            ltp_data_pair: None,
            num_windows: 8,
            num_window_groups: 2,
            window_group_length: vec![3, 5],
            num_swb: crate::ics_info::NUM_SWB_SHORT_WINDOW[FS as usize],
        }
    }

    fn section_data_for(sfb_cb_rows: Vec<Vec<u8>>) -> SectionData {
        let sections = sfb_cb_rows
            .iter()
            .map(|row| {
                // One section per band keeps the geometry simple.
                row.iter()
                    .enumerate()
                    .map(|(sfb, &cb)| Section {
                        codebook: cb,
                        start: sfb as u8,
                        end: sfb as u8 + 1,
                    })
                    .collect()
            })
            .collect();
        SectionData {
            sections,
            sfb_cb: sfb_cb_rows,
        }
    }

    /// Deterministic pseudo-random value in `-max..=max`.
    fn prand(state: &mut u32, max: i32) -> i32 {
        *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let span = 2 * max + 1;
        ((*state >> 8) % span as u32) as i32 - max
    }

    /// Fill the active bands of a transmission-order spectrum with
    /// bounded pseudo-random values per the band's codebook LAV.
    fn fill_spectrum(ics: &IcsInfo, sd: &SectionData, seed: u32) -> SpectralData {
        let mut state = seed;
        let mut spectral = empty_spectral(ics).unwrap();
        let short = ics.window_sequence.is_eight_short();
        let offsets = if short {
            short_window_offsets(FS).unwrap()
        } else {
            long_window_offsets(FS).unwrap()
        };
        for (g, row) in sd.sfb_cb.iter().enumerate() {
            let wgl = usize::from(ics.window_group_length[g]);
            let mut base = 0usize;
            for (sfb, &cb) in row.iter().enumerate().take(usize::from(ics.max_sfb)) {
                let width = usize::from(offsets[sfb + 1] - offsets[sfb]);
                let max = match cb {
                    0 | 13 | 14 | 15 => 0,
                    1 | 2 => 1,
                    3 | 4 => 2,
                    5 | 6 => 4,
                    7 | 8 => 7,
                    9 | 10 => 12,
                    // ESC book: exercise escapes with magnitudes > 16.
                    11 => 40,
                    _ => 15,
                };
                if max > 0 {
                    for i in 0..wgl * width {
                        spectral.x_quant[g][base + i] = prand(&mut state, max);
                    }
                }
                base += wgl * width;
            }
        }
        spectral
    }

    /// Round-trip: encode → decode reproduces the exact quantized
    /// spectrum, across a codebook mix that forces multiple sets and
    /// non-PCW segment spanning (long window).
    #[test]
    fn round_trips_long_window_mixed_codebooks() {
        let ics = long_ics(12);
        let sd = section_data_for(vec![vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 11]]);
        let spectral = fill_spectrum(&ics, &sd, 0xC0FFEE);
        let (payload, len_bits, longest) =
            encode_reordered_spectral_data(&spectral, &ics, &sd, FS).unwrap();
        assert!(len_bits > 0 && longest > 0);
        let back =
            decode_reordered_spectral_data(&payload, len_bits, longest, &ics, &sd, FS).unwrap();
        assert_eq!(back.x_quant, spectral.x_quant);
    }

    /// Round-trip with ZERO_HCB holes and an intensity band mixed in
    /// (no spectrum transmitted for those bands).
    #[test]
    fn round_trips_with_spectrumless_bands() {
        let ics = long_ics(10);
        let sd = section_data_for(vec![vec![3, 0, 5, 15, 11, 0, 9, 1, 0, 7]]);
        let spectral = fill_spectrum(&ics, &sd, 0xBADF00D);
        let (payload, len_bits, longest) =
            encode_reordered_spectral_data(&spectral, &ics, &sd, FS).unwrap();
        let back =
            decode_reordered_spectral_data(&payload, len_bits, longest, &ics, &sd, FS).unwrap();
        assert_eq!(back.x_quant, spectral.x_quant);
    }

    /// Eight-short round-trip with two window groups: the §4.6.16.3.3.1
    /// unit-based window interleave (not the §4.5.2.3.5 grouping
    /// interleave) must be applied consistently on both sides.
    #[test]
    fn round_trips_eight_short_two_groups() {
        let ics = short_ics(8);
        let sd = section_data_for(vec![
            vec![1, 3, 5, 7, 9, 11, 2, 4],
            vec![11, 9, 7, 5, 3, 1, 4, 2],
        ]);
        let spectral = fill_spectrum(&ics, &sd, 0x5EED);
        let (payload, len_bits, longest) =
            encode_reordered_spectral_data(&spectral, &ics, &sd, FS).unwrap();
        let back =
            decode_reordered_spectral_data(&payload, len_bits, longest, &ics, &sd, FS).unwrap();
        assert_eq!(back.x_quant, spectral.x_quant);
    }

    /// The payload survives trailing slack: a buffer longer than the
    /// codeword bits (larger transmitted length) still decodes — the
    /// slack widens the last segment, exactly as §4.6.16.3.3.2
    /// specifies.
    #[test]
    fn decodes_with_trailing_slack_bits() {
        let ics = long_ics(6);
        let sd = section_data_for(vec![vec![2, 4, 6, 8, 10, 11]]);
        let spectral = fill_spectrum(&ics, &sd, 0xABCDEF);
        let (payload, len_bits, longest) =
            encode_reordered_spectral_data(&spectral, &ics, &sd, FS).unwrap();
        // Re-plan with 16 slack bits: the writer must scatter into the
        // wider grid and the decoder must follow.
        let slack_bits = len_bits + 16;
        let cws = enumerate_presorted(&ics, &sd, FS).unwrap();
        let widths: Vec<u8> = cws
            .iter()
            .map(|cw| segment_width(cw.sect_cb, longest))
            .collect();
        let seg = Segmentation::new(&widths, u32::from(slack_bits));
        let mut encoded = Vec::new();
        for cw in &cws {
            let vals = &spectral.x_quant[cw.group][cw.buf_index..cw.buf_index + cw.dim];
            encoded.push(encode_codeword_bits(cw, vals).unwrap());
        }
        let lengths: Vec<u32> = encoded.iter().map(|e| e.1).collect();
        let plan = crate::hcr::ReorderPlan::build(&lengths, &seg).unwrap();
        let mut wide = vec![0u8; (slack_bits as usize).div_ceil(8)];
        for (c, (bytes, len)) in encoded.iter().enumerate() {
            for bit in 0..*len {
                if bytes[(bit / 8) as usize] & (0x80 >> (bit % 8)) != 0 {
                    let pos = plan.codeword_bits[c][bit as usize];
                    wide[(pos / 8) as usize] |= 0x80 >> (pos % 8);
                }
            }
        }
        let back =
            decode_reordered_spectral_data(&wide, slack_bits, longest, &ics, &sd, FS).unwrap();
        assert_eq!(back.x_quant, spectral.x_quant);
        let _ = payload;
    }

    /// Virtual codebooks (16..=31) decode as book 11 with their own
    /// segment widths.
    #[test]
    fn round_trips_virtual_codebooks() {
        let ics = long_ics(6);
        // VCB 17 pairs with small magnitudes; VCB 31 with escapes.
        let sd = section_data_for(vec![vec![17, 31, 1, 16, 20, 11]]);
        let mut spectral = empty_spectral(&ics).unwrap();
        let offsets = long_window_offsets(FS).unwrap();
        let mut state = 0x1234u32;
        for sfb in 0..6usize {
            let (a, b) = (usize::from(offsets[sfb]), usize::from(offsets[sfb + 1]));
            let max = match sfb {
                0 | 3 => 3,  // VCB 17 / 16: modest values
                1 | 4 => 30, // VCB 31 / 20: escapes
                2 => 1,      // book 1 quads
                _ => 40,     // book 11
            };
            for i in a..b {
                spectral.x_quant[0][i] = prand(&mut state, max);
            }
        }
        let (payload, len_bits, longest) =
            encode_reordered_spectral_data(&spectral, &ics, &sd, FS).unwrap();
        let back =
            decode_reordered_spectral_data(&payload, len_bits, longest, &ics, &sd, FS).unwrap();
        assert_eq!(back.x_quant, spectral.x_quant);
    }

    /// A corrupt payload surfaces an error, not a panic: flip bits in
    /// the PCW region.
    #[test]
    fn corrupt_payload_errors_cleanly() {
        let ics = long_ics(8);
        let sd = section_data_for(vec![vec![1, 2, 3, 4, 5, 6, 7, 8]]);
        let spectral = fill_spectrum(&ics, &sd, 0xFEED);
        let (mut payload, len_bits, longest) =
            encode_reordered_spectral_data(&spectral, &ics, &sd, FS).unwrap();
        for byte in payload.iter_mut().take(4) {
            *byte ^= 0xFF;
        }
        // Either decodes to different values or errors — it must not
        // panic, and it must not silently return the original.
        if let Ok(back) = decode_reordered_spectral_data(&payload, len_bits, longest, &ics, &sd, FS)
        {
            assert_ne!(back.x_quant, spectral.x_quant);
        }
    }

    /// Pre-sorting puts the ESC-book codewords first (priority 0) and
    /// the book-1/2 codewords last (priority 21).
    #[test]
    fn presort_orders_esc_first() {
        let ics = long_ics(3);
        let sd = section_data_for(vec![vec![1, 11, 5]]);
        let cws = enumerate_presorted(&ics, &sd, FS).unwrap();
        assert!(!cws.is_empty());
        assert_eq!(cws.first().unwrap().sect_cb, 11);
        assert_eq!(cws.last().unwrap().sect_cb, 1);
    }
}

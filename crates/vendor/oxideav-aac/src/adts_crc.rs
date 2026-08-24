//! ADTS `error_check()` and SBR `bs_sbr_crc_bits` CRC verification.
//!
//! Two independent CRC mechanisms protect an AAC bitstream, each with
//! its own polynomial, initial value, and covered region:
//!
//! 1. the **ADTS `crc_check`** — a 16-bit CRC present when the ADTS
//!    fixed header signals `protection_absent == 0`. The protected-bit
//!    region is normatively described by ISO/IEC 13818-7:2004 §8.1.1.1
//!    (semantics of `adts_error_check()` and the multi-raw-data-block
//!    split variants, Tables 1.A.8–1.A.10 of ISO/IEC 14496-3:2009);
//!    the CRC code itself is cited by 13818-7 §8.1.1.2 to ISO/IEC
//!    11172-3 §2.4.3.1: generator polynomial
//!    `G(x) = x¹⁶ + x¹⁵ + x² + 1` (`0x8005`), initial register value
//!    all-ones (`0xFFFF`), bits fed MSB-first in order of appearance,
//!    no final inversion.
//! 2. the **SBR extension CRC** (`bs_sbr_crc_bits`) — a 10-bit CRC
//!    carried at the head of an `EXT_SBR_DATA_CRC` (extension type 14)
//!    fill payload. ISO/IEC 14496-3:2009 §4.4.2.8.1 (repeated in
//!    §4.5.2.8.1): generator polynomial
//!    `G10(x) = x¹⁰ + x⁹ + x⁵ + x⁴ + x + 1`, initial value **zero**,
//!    covering every `sbr_extension_data()` bit after the CRC field up
//!    to (but excluding) the trailing `bs_fill_bits` alignment — i.e.
//!    `num_sbr_bits − 10` bits (Table 4.62).
//!
//! Both codes run on the same MSB-first shift register: for each
//! message bit, the feedback is the incoming bit XORed with the
//! register's top bit; on feedback the register (shifted left one)
//! is XORed with the low-order generator terms. No zero-augmentation
//! flush and no output inversion follow — the register value after
//! the last message bit is the checksum. With a zero initial value
//! this equals the polynomial remainder `M(x)·xᵏ mod G(x)`, matching
//! the §4.4.2.8.1 "remainder" wording for the SBR code; the ADTS code
//! differs only by its all-ones initialisation. (The §1.8.4.5 CRC
//! family implemented in [`crate::crc`] is a *different* convention —
//! zero init **plus** a normative output-bit inversion — and covers
//! the LATM `crcCheckSum` / EP-tool codes, not these two.)
//!
//! ## ADTS protected-bit region (13818-7:2004 §8.1.1.1)
//!
//! For `adts_error_check()` (single raw data block,
//! `number_of_raw_data_blocks_in_frame == 0` on the wire) the bits fed
//! into the CRC, in order of appearance, are:
//!
//! * **all 56 bits** of `adts_fixed_header()` + `adts_variable_header()`;
//! * the **first 192 bits** of every SCE / CPE / CCE / LFE channel
//!   element — *excluding* the 3-bit `id_syn_ele`, zero-padded to 192
//!   when the element is shorter;
//! * **additionally** the first 128 bits of the *second*
//!   `individual_channel_stream` of every CPE (zero-padded to 128;
//!   when the second ICS starts before the element's 192nd bit the
//!   overlap is protected twice, each time in order of appearance);
//! * **all** bits of every `program_config_element()` and
//!   `data_stream_element()` (again excluding `id_syn_ele`).
//!
//! Fill elements, the END marker, and the `crc_check` field itself are
//! not covered.
//!
//! `adts_raw_data_block_error_check()` (multi-RDB form, one 16-bit CRC
//! after each `raw_data_block()`) covers the same per-element regions
//! scoped to its block, *without* re-including the headers; the
//! headers plus every 16-bit `raw_data_block_position` are covered
//! once by `adts_header_error_check()`.
//!
//! ## Provenance
//!
//! Region selection and code parameters are transcribed from the
//! staged format specifications (ISO/IEC 14496-3:2009 Tables
//! 1.A.5–1.A.10 / Table 4.62 / §4.4.2.8.1 and ISO/IEC 13818-7:2004
//! §8.1.1) via the clean-room region analysis in
//! `docs/audio/aac/aac-crc-regions.md`. ISO/IEC 11172-3 itself is not
//! staged; the `0x8005` / `0xFFFF` shift-register parameters are the
//! §2.4.3.1 values as recorded there.

use oxideav_core::bits::BitReader;

use crate::cce::CouplingChannelElement;
use crate::ics_body::IcsBody;
use crate::raw_data_block::{Element, IdSynEle, Walker};
use crate::spectral_data::SpectralData;
use crate::{Error, Result};

/// Low-order terms of the ADTS generator `x¹⁶ + x¹⁵ + x² + 1`
/// (ISO/IEC 11172-3 §2.4.3.1 via 13818-7:2004 §8.1.1.2).
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub const ADTS_CRC_POLY: u32 = 0x8005;

/// ADTS CRC initial register value (all ones).
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub const ADTS_CRC_INIT: u32 = 0xFFFF;

/// Low-order terms of the SBR generator `x¹⁰ + x⁹ + x⁵ + x⁴ + x + 1`
/// (ISO/IEC 14496-3:2009 §4.4.2.8.1).
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub const SBR_CRC_POLY: u32 = 0x0233;

/// MSB-first CRC shift register (see module docs for the feedback
/// convention shared by the ADTS and SBR codes).
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct CrcRegister {
    reg: u32,
    poly: u32,
    mask: u32,
    top: u32,
}

impl CrcRegister {
    /// A register configured for the ADTS `crc_check` code: 16 bits,
    /// generator `0x8005`, initial value `0xFFFF`.
    pub fn adts() -> Self {
        CrcRegister {
            reg: ADTS_CRC_INIT,
            poly: ADTS_CRC_POLY,
            mask: 0xFFFF,
            top: 0x8000,
        }
    }

    /// A register configured for the SBR `bs_sbr_crc_bits` code: 10
    /// bits, generator `G10` (`0x233`), initial value zero.
    pub fn sbr() -> Self {
        CrcRegister {
            reg: 0,
            poly: SBR_CRC_POLY,
            mask: 0x03FF,
            top: 0x0200,
        }
    }

    /// Feed one message bit (MSB-first order).
    #[inline]
    pub fn feed_bit(&mut self, bit: bool) {
        let feedback = ((self.reg & self.top) != 0) ^ bit;
        self.reg = (self.reg << 1) & self.mask;
        if feedback {
            self.reg ^= self.poly;
        }
    }

    /// Feed `n` zero bits (the §8.1.1.1 zero-padding of short
    /// elements).
    pub fn feed_zeros(&mut self, n: u64) {
        for _ in 0..n {
            self.feed_bit(false);
        }
    }

    /// Feed the bit range `[start_bit, end_bit)` of `data`, MSB-first
    /// within each byte. Bits past the end of `data` are fed as zero
    /// (a region that overruns its buffer only ever does so via the
    /// normative zero-padding).
    pub fn feed_bit_range(&mut self, data: &[u8], start_bit: u64, end_bit: u64) {
        for pos in start_bit..end_bit {
            let byte = (pos / 8) as usize;
            let bit = data.get(byte).is_some_and(|b| b & (0x80 >> (pos % 8)) != 0);
            self.feed_bit(bit);
        }
    }

    /// The current register value (the checksum once the whole
    /// protected region has been fed).
    pub fn value(&self) -> u16 {
        self.reg as u16
    }
}

/// Compute the 10-bit SBR CRC over the bit range `[start_bit,
/// end_bit)` of `data` — the `sbr_extension_data()` payload bits
/// after the `bs_sbr_crc_bits` field, before the `bs_fill_bits`
/// (ISO/IEC 14496-3:2009 Table 4.62 / §4.4.2.8.1).
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn sbr_crc(data: &[u8], start_bit: u64, end_bit: u64) -> u16 {
    let mut reg = CrcRegister::sbr();
    reg.feed_bit_range(data, start_bit, end_bit);
    reg.value()
}

/// One §8.1.1.1 protected region of a `raw_data_block()` payload:
/// the bit range `[start_bit, end_bit)` of the payload buffer, capped
/// and zero-padded to `pad_to` bits when a protection length applies
/// (192 for a channel element, 128 for a CPE's second ICS; `None`
/// feeds the whole range, the PCE / DSE case).
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectedRegion {
    /// First protected bit (absolute bit offset into the payload).
    pub start_bit: u64,
    /// One past the last payload bit of the region (the element end;
    /// the fed length is additionally capped by `pad_to`).
    pub end_bit: u64,
    /// Normative protection length: feed `min(end_bit - start_bit,
    /// pad_to)` payload bits, then zeros up to `pad_to`.
    pub pad_to: Option<u32>,
}

impl ProtectedRegion {
    fn feed(&self, reg: &mut CrcRegister, payload: &[u8]) {
        let len = self.end_bit.saturating_sub(self.start_bit);
        match self.pad_to {
            Some(pad) => {
                let take = len.min(u64::from(pad));
                reg.feed_bit_range(payload, self.start_bit, self.start_bit + take);
                reg.feed_zeros(u64::from(pad) - take);
            }
            None => reg.feed_bit_range(payload, self.start_bit, self.end_bit),
        }
    }
}

/// Compute the single-RDB `adts_error_check()` CRC (ISO/IEC 14496-3
/// Table 1.A.8, region per 13818-7:2004 §8.1.1.1): the 56 header bits
/// followed by every protected element region of the one
/// `raw_data_block()`.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn adts_single_crc(header: &[u8], payload: &[u8], regions: &[ProtectedRegion]) -> u16 {
    let mut reg = CrcRegister::adts();
    reg.feed_bit_range(header, 0, 56);
    for r in regions {
        r.feed(&mut reg, payload);
    }
    reg.value()
}

/// Compute the multi-RDB `adts_header_error_check()` CRC (Table
/// 1.A.9): the 56 header bits followed by every 16-bit
/// `raw_data_block_position`, in order.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn adts_header_crc(header: &[u8], positions: &[u16]) -> u16 {
    let mut reg = CrcRegister::adts();
    reg.feed_bit_range(header, 0, 56);
    for &p in positions {
        for i in (0..16).rev() {
            reg.feed_bit((p >> i) & 1 != 0);
        }
    }
    reg.value()
}

/// Compute one multi-RDB `adts_raw_data_block_error_check()` CRC
/// (Table 1.A.10): the protected element regions of a single
/// `raw_data_block()`, headers *not* re-included.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn adts_rdb_crc(payload: &[u8], regions: &[ProtectedRegion]) -> u16 {
    let mut reg = CrcRegister::adts();
    for r in regions {
        r.feed(&mut reg, payload);
    }
    reg.value()
}

/// Walk one `raw_data_block()` off `reader` (parse-only — no
/// reconstruction) and collect its §8.1.1.1 protected regions in
/// order of appearance: per channel element the post-`id_syn_ele`
/// 192-bit window (SCE / CPE / CCE / LFE), per CPE additionally the
/// second ICS's 128-bit window, and the full body of every PCE / DSE.
///
/// The reader is left positioned after the block's END marker (byte
/// aligned), exactly where a multi-RDB `adts_raw_data_block_error_
/// check()` field or the next block begins. Returns the regions; an
/// exhausted payload before an explicit END terminates the block the
/// same way the decode driver treats it.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn collect_block_regions(
    reader: &mut BitReader<'_>,
    aot: u8,
    fs: u8,
) -> Result<Vec<ProtectedRegion>> {
    let mut regions = Vec::new();
    loop {
        let elem_start = reader.bit_position();
        let Some(elem) = Walker::new(reader).next_element()? else {
            return Ok(regions);
        };
        match elem {
            Element::ChannelElement {
                kind: IdSynEle::Sce | IdSynEle::Lfe,
                ..
            } => {
                let body = IcsBody::parse(reader, aot, fs, false)?;
                let ics = body.ics_info.clone().ok_or(Error::ElementDecodeInvalid)?;
                SpectralData::parse(reader, &ics, &body.section_data, fs)?;
                regions.push(ProtectedRegion {
                    start_bit: elem_start + 3,
                    end_bit: reader.bit_position(),
                    pad_to: Some(192),
                });
            }
            Element::ChannelElement {
                kind: IdSynEle::Cpe,
                ..
            } => {
                let parsed = crate::decode::parse_cpe(reader, aot, fs)?;
                let end = reader.bit_position();
                regions.push(ProtectedRegion {
                    start_bit: elem_start + 3,
                    end_bit: end,
                    pad_to: Some(192),
                });
                regions.push(ProtectedRegion {
                    start_bit: parsed.second_ics_start_bit,
                    end_bit: end,
                    pad_to: Some(128),
                });
            }
            Element::ChannelElement {
                kind: IdSynEle::Cce,
                element_instance_tag,
            } => {
                CouplingChannelElement::parse_after_tag(reader, element_instance_tag, aot, fs)?;
                regions.push(ProtectedRegion {
                    start_bit: elem_start + 3,
                    end_bit: reader.bit_position(),
                    pad_to: Some(192),
                });
            }
            Element::ChannelElement { .. } => return Err(Error::ElementDecodeInvalid),
            Element::Data { .. } | Element::ProgramConfig(_) => {
                regions.push(ProtectedRegion {
                    start_bit: elem_start + 3,
                    end_bit: reader.bit_position(),
                    pad_to: None,
                });
            }
            Element::Fill { .. } => {}
            Element::End => return Ok(regions),
        }
    }
}

/// Rewrite one `protection_absent == 1` single-raw-data-block ADTS
/// frame into its CRC-protected form: `protection_absent` cleared,
/// `aac_frame_length` grown by the 2 CRC bytes, and the Table 1.A.8
/// `crc_check` computed over the §8.1.1.1 region inserted between the
/// header and the payload. Every other header bit (including the
/// fields [`AdtsHeader::parse`] does not surface) is preserved
/// verbatim.
///
/// A frame that already carries a CRC is returned unchanged. A
/// multi-raw-data-block frame is rejected with
/// [`Error::NotImplemented`] (the Table 1.A.9/1.A.10 split form needs
/// a `raw_data_block_position` policy this helper does not invent).
pub fn protect_adts_frame(frame: &[u8]) -> Result<Vec<u8>> {
    let (header, payload_offset) = crate::adts::AdtsHeader::parse(frame)?;
    let frame_len = header.aac_frame_length as usize;
    if frame_len < payload_offset || frame.len() < frame_len {
        return Err(Error::UnexpectedEnd);
    }
    let frame = &frame[..frame_len];
    if !header.protection_absent {
        return Ok(frame.to_vec());
    }
    if header.number_of_raw_data_blocks_in_frame != 1 {
        return Err(Error::NotImplemented);
    }
    let new_len = header.aac_frame_length + 2;
    if new_len >= (1 << 13) {
        return Err(Error::AdtsEncodeInvalid);
    }
    // Patch the header bytes in place: clear protection_absent (bit 0
    // of byte 1) and re-pack the 13-bit aac_frame_length (low 2 bits
    // of byte 3, byte 4, top 3 bits of byte 5).
    let mut h = [0u8; 7];
    h.copy_from_slice(&frame[..7]);
    h[1] &= 0xFE;
    h[3] = (h[3] & 0xFC) | ((new_len >> 11) as u8 & 0x03);
    h[4] = (new_len >> 3) as u8;
    h[5] = (h[5] & 0x1F) | (((new_len & 0x07) as u8) << 5);

    let payload = &frame[payload_offset..];
    let mut reader = BitReader::new(payload);
    let regions = collect_block_regions(
        &mut reader,
        header.audio_object_type(),
        header.sampling_frequency_index,
    )?;
    let crc = adts_single_crc(&h, payload, &regions);

    let mut out = Vec::with_capacity(frame.len() + 2);
    out.extend_from_slice(&h);
    out.extend_from_slice(&crc.to_be_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

/// [`protect_adts_frame`] applied to every frame of a raw ADTS byte
/// stream (`aac_frame_length`-delimited walk to exhaustion).
pub fn protect_adts_stream(data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(data.len());
    let mut pos = 0usize;
    while pos + crate::adts::ADTS_HEADER_BYTES_NO_CRC <= data.len() {
        let (header, _) = crate::adts::AdtsHeader::parse(&data[pos..])?;
        let frame_len = header.aac_frame_length as usize;
        if pos + frame_len > data.len() {
            return Err(Error::UnexpectedEnd);
        }
        out.extend_from_slice(&protect_adts_frame(&data[pos..pos + frame_len])?);
        pos += frame_len;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Independent long-division reference: the MSB-feedback register
    /// with initial value `I` over an `n`-bit message `M` computes, by
    /// linearity, the remainder `(M(x)·xᵏ + I(x)·xⁿ) mod G(x)` — the
    /// dividend is the k-zero-extended message with the init bits
    /// XORed onto its leading `k` positions.
    fn reference(poly_low: u32, k: u32, init: u32, bits: &[bool]) -> u32 {
        let full = u64::from(poly_low) | (1u64 << k);
        let mut dividend: Vec<bool> = bits.to_vec();
        dividend.extend(std::iter::repeat(false).take(k as usize));
        for (i, d) in dividend.iter_mut().enumerate().take(k as usize) {
            *d ^= (init >> (k as usize - 1 - i)) & 1 != 0;
        }
        let mut reg: u64 = 0;
        let topbit = 1u64 << k;
        for &b in &dividend {
            reg = (reg << 1) | u64::from(b);
            if reg & topbit != 0 {
                reg ^= full;
            }
        }
        (reg & ((1u64 << k) - 1)) as u32
    }

    fn to_bits(bytes: &[u8]) -> Vec<bool> {
        bytes
            .iter()
            .flat_map(|&b| (0..8).rev().map(move |i| (b >> i) & 1 != 0))
            .collect()
    }

    #[test]
    fn adts_register_matches_long_division_reference() {
        for msg in [
            &[][..],
            &[0x00][..],
            &[0xFF, 0xF1][..],
            &[0x12, 0x34, 0x56, 0x78, 0x9A][..],
            &[0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03][..],
        ] {
            let bits = to_bits(msg);
            let mut reg = CrcRegister::adts();
            for &b in &bits {
                reg.feed_bit(b);
            }
            assert_eq!(
                u32::from(reg.value()),
                reference(ADTS_CRC_POLY, 16, ADTS_CRC_INIT, &bits),
                "message {msg:x?}"
            );
        }
    }

    #[test]
    fn sbr_register_is_plain_remainder() {
        // Zero init ⇒ the register equals M(x)·x¹⁰ mod G10(x).
        for msg in [&[0x5Au8, 0x33][..], &[0xFF, 0x00, 0xAB, 0xCD][..]] {
            let bits = to_bits(msg);
            let mut reg = CrcRegister::sbr();
            for &b in &bits {
                reg.feed_bit(b);
            }
            assert_eq!(
                u32::from(reg.value()),
                reference(SBR_CRC_POLY, 10, 0, &bits),
                "message {msg:x?}"
            );
        }
    }

    #[test]
    fn sbr_poly_matches_crate_crc10_generator() {
        // §4.4.2.8.1's G10 is the same polynomial as the §1.8.4.5
        // CRC10 row; only the init / inversion conventions differ.
        assert_eq!(
            u64::from(SBR_CRC_POLY),
            crate::crc::CrcPoly::Crc10.generator()
        );
        assert_eq!(
            u64::from(ADTS_CRC_POLY),
            crate::crc::CrcPoly::Crc16.generator()
        );
    }

    #[test]
    fn empty_message_yields_init_for_adts() {
        // No message bits: the register never moves.
        let reg = CrcRegister::adts();
        assert_eq!(u32::from(reg.value()), ADTS_CRC_INIT);
        assert_eq!(CrcRegister::sbr().value(), 0);
    }

    #[test]
    fn appending_checksum_cancels_the_register() {
        // Defining property of the MSB-feedback register: feeding the
        // message and then its own checksum drives the register to 0.
        for msg in [&[0x53u8, 0x91, 0x2C][..], &[0xFF, 0xF9, 0x5C, 0x80][..]] {
            let bits = to_bits(msg);
            let mut reg = CrcRegister::adts();
            for &b in &bits {
                reg.feed_bit(b);
            }
            let crc = reg.value();
            for i in (0..16).rev() {
                reg.feed_bit((crc >> i) & 1 != 0);
            }
            assert_eq!(reg.value(), 0, "message {msg:x?}");
        }
    }

    #[test]
    fn region_pads_short_elements_with_zeros() {
        // A 40-bit element padded to 192 must equal feeding the 40
        // payload bits + 152 explicit zeros.
        let payload = [0xA5u8; 8];
        let region = ProtectedRegion {
            start_bit: 3,
            end_bit: 43,
            pad_to: Some(192),
        };
        let mut a = CrcRegister::adts();
        region.feed(&mut a, &payload);
        let mut b = CrcRegister::adts();
        b.feed_bit_range(&payload, 3, 43);
        b.feed_zeros(152);
        assert_eq!(a.value(), b.value());
    }

    #[test]
    fn region_caps_long_elements_at_pad_to() {
        // A 300-bit element only contributes its first 192 bits.
        let payload = [0x3Cu8; 64];
        let region = ProtectedRegion {
            start_bit: 5,
            end_bit: 305,
            pad_to: Some(192),
        };
        let mut a = CrcRegister::adts();
        region.feed(&mut a, &payload);
        let mut b = CrcRegister::adts();
        b.feed_bit_range(&payload, 5, 5 + 192);
        assert_eq!(a.value(), b.value());
    }

    #[test]
    fn header_crc_covers_positions() {
        let header = [0xFFu8, 0xF1, 0x50, 0x80, 0x2F, 0xFF, 0xFC];
        let a = adts_header_crc(&header, &[]);
        let b = adts_header_crc(&header, &[0x1234]);
        assert_ne!(a, b, "positions must alter the header CRC");
    }
}

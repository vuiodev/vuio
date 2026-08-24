//! `ep_frame()` — ISO/IEC 14496-3 §1.8.2.2 (Tables 1.50–1.53) and the
//! §1.8.4 decoding machinery of the error-protection tool: the
//! FEC-protected in-band header (`choice_of_pred` + `class_attrib()`,
//! §1.8.4.3), per-class CRC (§1.8.4.5) + SRCPC (§1.8.4.6) / shortened
//! Reed-Solomon (§1.8.4.7) protection, the §1.8.4.8 recursive
//! interleaver (modes 0 / 1 / 2) and the §1.8.4.9 class-reordered
//! output.
//!
//! [`EpFrameCodec`] is built from a parsed
//! [`ErrorProtectionSpecificConfig`]; [`EpFrameCodec::encode`] turns a
//! class-partitioned access unit into one error-protected `ep_frame()`
//! and [`EpFrameCodec::decode`] inverts it, verifying every CRC and
//! correcting transmission errors through the FEC layers. The
//! concatenation of the decoded classes is the `epConfig == 0` payload
//! (§1.8.1); §1.8.4.9 output reordering is applied on the decode side.
//!
//! Implementation notes on the two spec points the staged text leaves
//! loose (kept conservative; both surface [`Error::EpFrameInvalid`]
//! rather than guessing):
//!
//! * an escaped (`rate_escape == 1`) rate on an RS class has no
//!   in-band code table (Table 1.55 is the SRCPC puncture table), so
//!   it is rejected;
//! * the byte-wise recursive interleaving of an RS class (§1.8.4.8.2)
//!   is supported when the accumulated `Y` stream is a whole number of
//!   octets (the matrix then works in byte cells exactly as Figure
//!   1.18 draws it); a non-aligned `Y` is rejected.

use oxideav_core::bits::{BitReader, BitWriter};

use crate::crc::crc_bits;
use crate::ep_config::{EpClass, EpPredefinedSet, ErrorProtectionSpecificConfig};
use crate::ep_fec::{
    header_fec_decode, header_fec_encode, srcpc_coded_len, srcpc_decode, srcpc_encode,
};
use crate::ep_rs::{srs_decode, srs_encode};
use crate::{Error, Result};

/// Table 1.55 — the 3-bit in-band `class_code_rate` codes mapped onto
/// the out-of-band `class_rate` scale (0..=24).
pub const INBAND_RATE_TO_CLASS_RATE: [u8; 8] = [0, 3, 4, 6, 8, 12, 16, 24];

/// Table 1.56 — the 3-bit in-band `class_crc_count` codes mapped onto
/// CRC bit counts.
pub const INBAND_CRC_BITS: [u32; 8] = [0, 6, 8, 10, 12, 14, 16, 32];

/// One frame's worth of class content plus the per-frame escaped
/// parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpFrameData {
    /// Index into the §1.8.4.2 **expanded** pre-defined-set list.
    pub choice_of_pred: usize,
    /// Per-class information bits, in class-index order (their
    /// concatenation is the `epConfig == 0` payload).
    pub classes: Vec<Vec<bool>>,
    /// In-band `class_code_rate` values (Table 1.55 codes) for
    /// classes with `rate_escape == 1`; `None` on fixed-rate classes.
    pub rate_codes: Vec<Option<u8>>,
    /// In-band `class_crc_count` values (Table 1.56 codes) for
    /// classes with `crclen_escape == 1`; `None` on fixed-CRC classes.
    pub crc_codes: Vec<Option<u8>>,
}

/// Resolved per-class parameters for one frame.
#[derive(Debug, Clone)]
struct ClassRt {
    /// Information length in bits (`None` = "until the end").
    len_bits: Option<usize>,
    /// Field width of the in-band `class_bit_count` (escaped classes).
    len_field_bits: Option<u32>,
    /// SRCPC `class_rate` (0..=24) or RS correctable-byte count.
    rate: u8,
    rate_escaped: bool,
    /// CRC width in bits.
    crc_bits: u32,
    crc_escaped: bool,
    fec_type: u8,
    terminated: bool,
    interleave_switch: u8,
}

/// Codec for one EP-tool configuration.
#[derive(Debug, Clone)]
pub struct EpFrameCodec {
    cfg: ErrorProtectionSpecificConfig,
    sets: Vec<EpPredefinedSet>,
}

impl EpFrameCodec {
    /// Build a codec from a parsed configuration (running the
    /// §1.8.4.2 expansion once).
    pub fn new(cfg: ErrorProtectionSpecificConfig) -> Result<Self> {
        let sets = cfg.expand()?;
        Ok(EpFrameCodec { cfg, sets })
    }

    /// The §1.8.4.2 expanded pre-defined sets (the `choice_of_pred`
    /// index space).
    pub fn sets(&self) -> &[EpPredefinedSet] {
        &self.sets
    }

    /// `Npred = ceil(log2(number of expanded sets))` (Table 1.51).
    pub fn npred(&self) -> u32 {
        let n = self.sets.len();
        if n <= 1 {
            0
        } else {
            usize::BITS - (n - 1).leading_zeros()
        }
    }

    fn resolve_class(&self, c: &EpClass) -> Result<ClassRt> {
        let (len_bits, len_field_bits) = if c.length_escape {
            let w = u32::from(c.number_of_bits_for_length.ok_or(Error::EpConfigInvalid)?);
            if w == 0 {
                (None, None) // "until the end"
            } else {
                (None, Some(w))
            }
        } else {
            (
                Some(usize::from(c.class_length.ok_or(Error::EpConfigInvalid)?)),
                None,
            )
        };
        let rate = c.class_rate.unwrap_or(0);
        let crc = match c.class_crclen {
            Some(code) => EpClass::crclen_bits(code)?,
            None => 0,
        };
        Ok(ClassRt {
            len_bits,
            len_field_bits,
            rate,
            rate_escaped: c.rate_escape,
            crc_bits: crc,
            crc_escaped: c.crclen_escape,
            fec_type: c.fec_type,
            terminated: c.termination_switch.unwrap_or(false),
            interleave_switch: c.interleave_switch.unwrap_or(0),
        })
    }

    /// Coded bit length of one class's `ep_encoded_class` given its
    /// resolved parameters and info length. RS chains are handled by
    /// the caller (the chained parity rides the last member).
    fn coded_len(&self, rt: &ClassRt, info_bits: usize) -> Result<usize> {
        let with_crc = info_bits + rt.crc_bits as usize;
        Ok(match rt.fec_type {
            0 => srcpc_coded_len(with_crc, rt.rate, rt.terminated)?,
            1 | 2 => {
                if with_crc % 8 != 0 {
                    return Err(Error::EpFrameInvalid);
                }
                let bytes = with_crc / 8;
                let two_k = 2 * usize::from(rt.rate);
                if two_k == 0 {
                    with_crc
                } else {
                    if two_k >= 255 {
                        return Err(Error::EpConfigInvalid);
                    }
                    let parts = bytes.div_ceil(255 - two_k);
                    with_crc + 8 * two_k * parts
                }
            }
            _ => return Err(Error::EpConfigInvalid),
        })
    }

    /// Protect one class (CRC + FEC). RS chaining is resolved before
    /// this call (the info of a chain arrives concatenated).
    fn protect_class(&self, rt: &ClassRt, info: &[bool]) -> Result<Vec<bool>> {
        // §1.8.4.5 CRC first (the crc module applies the normative
        // output inversion).
        let mut with_crc: Vec<bool> = info.to_vec();
        if rt.crc_bits > 0 {
            let poly = EpClass::crc_poly(rt.crc_bits)?.ok_or(Error::EpFrameInvalid)?;
            let crc = crc_bits(poly, info);
            for i in (0..rt.crc_bits).rev() {
                with_crc.push(crc & (1u64 << i) != 0);
            }
        }
        match rt.fec_type {
            0 => srcpc_encode(&with_crc, rt.rate, rt.terminated),
            1 | 2 => {
                if with_crc.len() % 8 != 0 {
                    return Err(Error::EpFrameInvalid);
                }
                let bytes = bits_to_bytes(&with_crc);
                let parity = srs_encode(&bytes, usize::from(rt.rate))?;
                let mut out = with_crc;
                out.extend(bytes_to_bits(&parity));
                Ok(out)
            }
            _ => Err(Error::EpConfigInvalid),
        }
    }

    /// Undo [`Self::protect_class`]: FEC-decode (with correction) and
    /// verify + strip the CRC.
    fn unprotect_class(&self, rt: &ClassRt, coded: &[bool], info_bits: usize) -> Result<Vec<bool>> {
        let with_crc_len = info_bits + rt.crc_bits as usize;
        let mut with_crc: Vec<bool> = match rt.fec_type {
            0 => srcpc_decode(coded, with_crc_len, rt.rate, rt.terminated)?,
            1 | 2 => {
                if with_crc_len % 8 != 0 || coded.len() < with_crc_len {
                    return Err(Error::EpFrameInvalid);
                }
                let mut data = bits_to_bytes(&coded[..with_crc_len]);
                let parity = bits_to_bytes(&coded[with_crc_len..]);
                srs_decode(&mut data, &parity, usize::from(rt.rate))?;
                bytes_to_bits(&data)
            }
            _ => return Err(Error::EpConfigInvalid),
        };
        if rt.crc_bits > 0 {
            let poly = EpClass::crc_poly(rt.crc_bits)?.ok_or(Error::EpFrameInvalid)?;
            let rx_crc = with_crc.split_off(info_bits);
            let want = crc_bits(poly, &with_crc);
            let mut got = 0u64;
            for &b in &rx_crc {
                got = (got << 1) | u64::from(b);
            }
            if got != want {
                return Err(Error::EpFrameInvalid);
            }
        } else {
            with_crc.truncate(info_bits);
        }
        Ok(with_crc)
    }

    /// Encode one frame to a byte-aligned `ep_frame()`.
    pub fn encode(&self, frame: &EpFrameData) -> Result<Vec<u8>> {
        let set = self
            .sets
            .get(frame.choice_of_pred)
            .ok_or(Error::EpFrameInvalid)?;
        let n = set.classes.len();
        if frame.classes.len() != n || frame.rate_codes.len() != n || frame.crc_codes.len() != n {
            return Err(Error::EpFrameInvalid);
        }
        // Resolve runtime parameters (folding the in-band escapes in).
        let mut rts = Vec::with_capacity(n);
        for j in 0..n {
            let mut rt = self.resolve_class(&set.classes[j])?;
            if rt.rate_escaped {
                if rt.fec_type != 0 {
                    return Err(Error::EpFrameInvalid);
                }
                let code = frame.rate_codes[j].ok_or(Error::EpFrameInvalid)?;
                rt.rate = *INBAND_RATE_TO_CLASS_RATE
                    .get(usize::from(code))
                    .ok_or(Error::EpFrameInvalid)?;
            } else if frame.rate_codes[j].is_some() {
                return Err(Error::EpFrameInvalid);
            }
            if rt.crc_escaped {
                let code = frame.crc_codes[j].ok_or(Error::EpFrameInvalid)?;
                rt.crc_bits = *INBAND_CRC_BITS
                    .get(usize::from(code))
                    .ok_or(Error::EpFrameInvalid)?;
            } else if frame.crc_codes[j].is_some() {
                return Err(Error::EpFrameInvalid);
            }
            // Fixed-length classes must match the provided content.
            if let Some(l) = rt.len_bits {
                if frame.classes[j].len() != l {
                    return Err(Error::EpFrameInvalid);
                }
            } else if let Some(w) = rt.len_field_bits {
                if frame.classes[j].len() >= (1usize << w) {
                    return Err(Error::EpFrameInvalid);
                }
            }
            rts.push(rt);
        }

        // ---- Protect the classes (§1.8.4.4 RS chains resolved by
        // concatenating fec_type == 2 members with their successor).
        let mut coded: Vec<Vec<bool>> = vec![Vec::new(); n];
        let mut j = 0usize;
        while j < n {
            if rts[j].fec_type == 2 {
                // Chain: classes j..=last share one RS code.
                let mut last = j;
                while last < n && rts[last].fec_type == 2 {
                    last += 1;
                }
                if last >= n {
                    return Err(Error::EpFrameInvalid);
                }
                // §1.8.3.1: all chain members share class_rate.
                #[allow(clippy::needless_range_loop)]
                for m in j..=last {
                    if rts[m].rate != rts[last].rate || rts[m].crc_bits % 8 != 0 {
                        return Err(Error::EpFrameInvalid);
                    }
                }
                // Per-class CRCs, then one RS over the concatenation.
                let mut chain: Vec<bool> = Vec::new();
                let mut member_coded: Vec<Vec<bool>> = Vec::new();
                #[allow(clippy::needless_range_loop)]
                for m in j..=last {
                    let mut with_crc = frame.classes[m].clone();
                    if rts[m].crc_bits > 0 {
                        let poly =
                            EpClass::crc_poly(rts[m].crc_bits)?.ok_or(Error::EpFrameInvalid)?;
                        let crc = crc_bits(poly, &frame.classes[m]);
                        for i in (0..rts[m].crc_bits).rev() {
                            with_crc.push(crc & (1u64 << i) != 0);
                        }
                    }
                    if with_crc.len() % 8 != 0 {
                        return Err(Error::EpFrameInvalid);
                    }
                    chain.extend_from_slice(&with_crc);
                    member_coded.push(with_crc);
                }
                let parity = srs_encode(&bits_to_bytes(&chain), usize::from(rts[last].rate))?;
                // Every member transmits its own CRC-protected bits;
                // the parity rides the chain's last member.
                for (idx, m) in (j..=last).enumerate() {
                    coded[m] = member_coded[idx].clone();
                }
                coded[last].extend(bytes_to_bits(&parity));
                j = last + 1;
            } else {
                coded[j] = self.protect_class(&rts[j], &frame.classes[j])?;
                j += 1;
            }
        }

        // ---- In-band header bits.
        let npred = self.npred();
        let mut pred_bits: Vec<bool> = Vec::new();
        for i in (0..npred).rev() {
            pred_bits.push(frame.choice_of_pred & (1usize << i) != 0);
        }
        let mut attrib_bits: Vec<bool> = Vec::new();
        for jj in 0..n {
            let k = if set.class_reordered_output {
                usize::from(set.class_output_order[jj])
            } else {
                jj
            };
            if let Some(w) = rts[k].len_field_bits {
                let v = frame.classes[k].len();
                for i in (0..w).rev() {
                    attrib_bits.push(v & (1usize << i) != 0);
                }
            }
            if rts[k].rate_escaped {
                let code = frame.rate_codes[k].ok_or(Error::EpFrameInvalid)?;
                for i in (0..3).rev() {
                    attrib_bits.push(code & (1u8 << i) != 0);
                }
            }
            if rts[k].crc_escaped {
                let code = frame.crc_codes[k].ok_or(Error::EpFrameInvalid)?;
                for i in (0..3).rev() {
                    attrib_bits.push(code & (1u8 << i) != 0);
                }
            }
        }

        // The transmitted class order (Table 1.53).
        let tx_order: Vec<usize> = (0..n)
            .map(|jj| {
                if set.class_reordered_output {
                    usize::from(set.class_output_order[jj])
                } else {
                    jj
                }
            })
            .collect();

        match self.cfg.interleave_type {
            0 => self.assemble_mode0(&pred_bits, &attrib_bits, &tx_order, &coded, frame),
            1 | 2 => {
                self.assemble_interleaved(&pred_bits, &attrib_bits, &tx_order, &rts, &coded, frame)
            }
            _ => Err(Error::EpConfigInvalid),
        }
    }

    /// interleave_type == 0: `ep_header()`, `ep_encoded_classes()`,
    /// `stuffing_bits` (Table 1.50).
    fn assemble_mode0(
        &self,
        pred_bits: &[bool],
        attrib_bits: &[bool],
        tx_order: &[usize],
        coded: &[Vec<bool>],
        frame: &EpFrameData,
    ) -> Result<Vec<u8>> {
        let mut bits: Vec<bool> = Vec::new();
        bits.extend_from_slice(pred_bits);
        if !pred_bits.is_empty() {
            bits.extend(self.header_parity(pred_bits)?);
        }
        // class_attrib() + num_stuffing_bits — the stuffing count
        // depends on the total length, which the attrib field itself
        // is part of; everything except the 3-bit count is fixed, so
        // the count solves directly.
        let mut fixed = bits.len() + attrib_bits.len();
        if self.cfg.bit_stuffing == 1 {
            fixed += 3;
        }
        let attrib_parity_len = if attrib_bits.is_empty() && self.cfg.bit_stuffing != 1 {
            0
        } else {
            // parity spans class_attrib() incl. num_stuffing_bits.
            let l = attrib_bits.len() + if self.cfg.bit_stuffing == 1 { 3 } else { 0 };
            crate::ep_fec::HeaderFec::for_len(l)?.parity_bits(l)?
        };
        fixed += attrib_parity_len;
        let classes_len: usize = coded.iter().map(Vec::len).sum();
        let total_no_stuff = fixed + classes_len;
        let nstuff = if self.cfg.bit_stuffing == 1 {
            (8 - (total_no_stuff % 8)) % 8
        } else {
            0
        };
        let mut attrib_full: Vec<bool> = attrib_bits.to_vec();
        if self.cfg.bit_stuffing == 1 {
            for i in (0..3).rev() {
                attrib_full.push(nstuff & (1usize << i) != 0);
            }
        }
        bits.extend_from_slice(&attrib_full);
        if !attrib_full.is_empty() {
            bits.extend(self.header_parity(&attrib_full)?);
        }
        for &k in tx_order {
            bits.extend_from_slice(&coded[k]);
        }
        bits.resize(bits.len() + nstuff, false);
        let _ = frame;
        if self.cfg.bit_stuffing == 1 && bits.len() % 8 != 0 {
            return Err(Error::EpFrameInvalid);
        }
        Ok(bits_to_bytes_padded(&bits))
    }

    /// interleave_type == 1 / 2: the §1.8.4.8.2 multi-stage assembly.
    fn assemble_interleaved(
        &self,
        pred_bits: &[bool],
        attrib_bits: &[bool],
        tx_order: &[usize],
        rts: &[ClassRt],
        coded: &[Vec<bool>],
        frame: &EpFrameData,
    ) -> Result<Vec<u8>> {
        let mode2 = self.cfg.interleave_type == 2;
        let n = tx_order.len();
        // Stuffing count: the total bit count is invariant under
        // interleaving, so it solves exactly as in mode 0.
        let mut fixed = pred_bits.len();
        if !pred_bits.is_empty() {
            fixed += self.header_parity_len(pred_bits.len())?;
        }
        let attrib_l = attrib_bits.len() + if self.cfg.bit_stuffing == 1 { 3 } else { 0 };
        fixed += attrib_l;
        if attrib_l > 0 {
            fixed += self.header_parity_len(attrib_l)?;
        }
        let classes_len: usize = coded.iter().map(Vec::len).sum();
        let total_no_stuff = fixed + classes_len;
        let nstuff = if self.cfg.bit_stuffing == 1 {
            (8 - (total_no_stuff % 8)) % 8
        } else {
            0
        };

        // ---- Class stage.
        let mut buf_y: Vec<bool> = Vec::new();
        let mut buf_no: Vec<bool> = Vec::new();
        if mode2 {
            // Forward pass: switch-3 (concatenate) and switch-0
            // (non-interleaved) classes.
            for &k in tx_order.iter().take(n) {
                match rts[k].interleave_switch {
                    3 => buf_y.extend_from_slice(&coded[k]),
                    0 => buf_no.extend_from_slice(&coded[k]),
                    _ => {}
                }
            }
        }
        for jj in (0..n).rev() {
            let k = tx_order[jj];
            let sw = if mode2 { rts[k].interleave_switch } else { 1 };
            if mode2 && (sw == 0 || sw == 3) {
                continue;
            }
            // Width selection (Tables 1.63 / 1.64).
            let bytewise = rts[k].fec_type != 0;
            let w_units = if mode2 && sw == 2 {
                if bytewise {
                    return Err(Error::EpConfigInvalid);
                }
                28
            } else if bytewise {
                if coded[k].len() % 8 != 0 {
                    return Err(Error::EpFrameInvalid);
                }
                coded[k].len() / 8
            } else if mode2 {
                coded[k].len()
            } else {
                // Mode 1 SRCPC: 28 bits.
                28
            };
            buf_y = if bytewise {
                if buf_y.len() % 8 != 0 {
                    return Err(Error::EpFrameInvalid);
                }
                let x = bits_to_bytes(&coded[k]);
                let y = bits_to_bytes(&buf_y);
                bytes_to_bits(&interleave_units(&x, &y, w_units)?)
            } else {
                interleave_units(&coded[k], &buf_y, w_units)?
            };
        }
        buf_y.extend_from_slice(&buf_no);
        buf_y.resize(buf_y.len() + nstuff, false);

        // ---- Header stages: class_attrib (+ its parity) then
        // choice_of_pred (+ its parity), width = codeword length (or
        // 28 for the SRCPC header case).
        let mut attrib_full: Vec<bool> = attrib_bits.to_vec();
        if self.cfg.bit_stuffing == 1 {
            for i in (0..3).rev() {
                attrib_full.push(nstuff & (1usize << i) != 0);
            }
        }
        if !attrib_full.is_empty() {
            let mut x = attrib_full.clone();
            x.extend(self.header_parity(&attrib_full)?);
            let w = self.header_width(attrib_full.len())?;
            buf_y = interleave_units(&x, &buf_y, w)?;
        }
        if !pred_bits.is_empty() {
            let mut x = pred_bits.to_vec();
            x.extend(self.header_parity(pred_bits)?);
            let w = self.header_width(pred_bits.len())?;
            buf_y = interleave_units(&x, &buf_y, w)?;
        }
        let _ = frame;
        Ok(bits_to_bytes_padded(&buf_y))
    }

    /// Header parity via the Table 1.59 basic set, or the extended
    /// §1.8.4.3 protection when configured and the part exceeds 16
    /// bits.
    fn header_parity(&self, part: &[bool]) -> Result<Vec<bool>> {
        if self.cfg.header_protection && part.len() > 16 {
            let rate = self.cfg.header_rate.ok_or(Error::EpConfigInvalid)?;
            let crc = EpClass::crclen_bits(self.cfg.header_crclen.ok_or(Error::EpConfigInvalid)?)?;
            let mut with_crc = part.to_vec();
            if crc > 0 {
                let poly = EpClass::crc_poly(crc)?.ok_or(Error::EpFrameInvalid)?;
                let v = crc_bits(poly, part);
                for i in (0..crc).rev() {
                    with_crc.push(v & (1u64 << i) != 0);
                }
            }
            let coded = srcpc_encode(&with_crc, rate, true)?;
            // The parity is the codeword past the systematic prefix
            // is interleaved per-step; transmit the whole codeword
            // minus the raw part positionally — same convention as
            // ep_fec::header_fec_encode's SRCPC branch, generalised:
            // here we simply append the full codeword after the part
            // is *not* separately transmitted... To keep the wire
            // shape "part then parity", the parity carries the coded
            // stream with the leading systematic copies of the part
            // removed positionally.
            let mut parity = Vec::with_capacity(coded.len() - part.len());
            let p = crate::ep_fec::puncture_pattern(rate)?;
            let mut pos = 0usize;
            let steps = with_crc.len() + crate::ep_fec::SRCPC_TAIL_BITS;
            for t in 0..steps {
                for (i, &line) in p.iter().enumerate() {
                    if line & (0x80 >> (t % 8)) != 0 {
                        let bit = coded[pos];
                        pos += 1;
                        let systematic_of_part = i == 0 && t < part.len();
                        if !systematic_of_part {
                            parity.push(bit);
                        }
                    }
                }
            }
            Ok(parity)
        } else {
            header_fec_encode(part)
        }
    }

    /// Bit length of [`Self::header_parity`] for an `l`-bit part.
    fn header_parity_len(&self, l: usize) -> Result<usize> {
        if self.cfg.header_protection && l > 16 {
            let rate = self.cfg.header_rate.ok_or(Error::EpConfigInvalid)?;
            let crc = EpClass::crclen_bits(self.cfg.header_crclen.ok_or(Error::EpConfigInvalid)?)?
                as usize;
            Ok(srcpc_coded_len(l + crc, rate, true)? - l)
        } else {
            crate::ep_fec::HeaderFec::for_len(l)?.parity_bits(l)
        }
    }

    /// Decode a header part protected by [`Self::header_parity`].
    fn header_unprotect(&self, part: &[bool], parity: &[bool]) -> Result<Vec<bool>> {
        if self.cfg.header_protection && part.len() > 16 {
            let rate = self.cfg.header_rate.ok_or(Error::EpConfigInvalid)?;
            let crc = EpClass::crclen_bits(self.cfg.header_crclen.ok_or(Error::EpConfigInvalid)?)?;
            // Re-merge the positional layout of header_parity.
            let p = crate::ep_fec::puncture_pattern(rate)?;
            let l = part.len();
            let with_crc_len = l + crc as usize;
            let steps = with_crc_len + crate::ep_fec::SRCPC_TAIL_BITS;
            let mut coded = Vec::with_capacity(l + parity.len());
            let mut pi = 0usize;
            let mut ii = 0usize;
            for t in 0..steps {
                for (i, &line) in p.iter().enumerate() {
                    if line & (0x80 >> (t % 8)) != 0 {
                        if i == 0 && t < l {
                            coded.push(part[ii]);
                            ii += 1;
                        } else {
                            if pi >= parity.len() {
                                return Err(Error::EpFrameInvalid);
                            }
                            coded.push(parity[pi]);
                            pi += 1;
                        }
                    }
                }
            }
            let decoded = srcpc_decode(&coded, with_crc_len, rate, true)?;
            let (msg, rx_crc) = decoded.split_at(l);
            if crc > 0 {
                let poly = EpClass::crc_poly(crc)?.ok_or(Error::EpFrameInvalid)?;
                let want = crc_bits(poly, msg);
                let mut got = 0u64;
                for &b in rx_crc {
                    got = (got << 1) | u64::from(b);
                }
                if got != want {
                    return Err(Error::EpFrameInvalid);
                }
            }
            Ok(msg.to_vec())
        } else {
            header_fec_decode(part, parity)
        }
    }

    /// Interleaver width for a header part (§1.8.4.8.2.1: the block
    /// codeword length in bits, or 28 when SRCPC protects it).
    fn header_width(&self, l: usize) -> Result<usize> {
        if (self.cfg.header_protection && l > 16)
            || matches!(
                crate::ep_fec::HeaderFec::for_len(l)?,
                crate::ep_fec::HeaderFec::Srcpc16
            )
        {
            Ok(28)
        } else {
            Ok(l + self.header_parity_len(l)?)
        }
    }

    /// Decode one byte-aligned `ep_frame()`.
    pub fn decode(&self, data: &[u8]) -> Result<EpFrameData> {
        let total_bits = data.len() * 8;
        let all_bits: Vec<bool> = (0..total_bits)
            .map(|i| data[i / 8] & (0x80 >> (i % 8)) != 0)
            .collect();

        match self.cfg.interleave_type {
            0 => self.decode_mode0(&all_bits),
            1 | 2 => self.decode_interleaved(&all_bits),
            _ => Err(Error::EpConfigInvalid),
        }
    }

    /// Read + verify the two header parts from a bit reader position.
    fn read_headers(&self, bits: &[bool], pos: &mut usize) -> Result<(usize, usize, Vec<bool>)> {
        // choice_of_pred (+ parity).
        let npred = self.npred() as usize;
        let choice = if npred > 0 {
            let part = take(bits, pos, npred)?;
            let parity = take(bits, pos, self.header_parity_len(npred)?)?;
            let corrected = self.header_unprotect(&part, &parity)?;
            let mut v = 0usize;
            for &b in &corrected {
                v = (v << 1) | usize::from(b);
            }
            v
        } else {
            0
        };
        let set = self.sets.get(choice).ok_or(Error::EpFrameInvalid)?;
        // class_attrib() length is fixed by the chosen set.
        let mut attrib_l = 0usize;
        for c in &set.classes {
            if c.length_escape {
                let w = usize::from(c.number_of_bits_for_length.ok_or(Error::EpConfigInvalid)?);
                attrib_l += w; // 0 for "until the end"
            }
            if c.rate_escape {
                attrib_l += 3;
            }
            if c.crclen_escape {
                attrib_l += 3;
            }
        }
        if self.cfg.bit_stuffing == 1 {
            attrib_l += 3;
        }
        let attrib = if attrib_l > 0 {
            let part = take(bits, pos, attrib_l)?;
            let parity = take(bits, pos, self.header_parity_len(attrib_l)?)?;
            self.header_unprotect(&part, &parity)?
        } else {
            Vec::new()
        };
        Ok((choice, attrib_l, attrib))
    }

    /// Parse the decoded `class_attrib()` bits into per-class in-band
    /// values (`Table 1.52` order) + the stuffing count.
    #[allow(clippy::type_complexity)]
    fn parse_attrib(
        &self,
        choice: usize,
        attrib: &[bool],
    ) -> Result<(Vec<Option<usize>>, Vec<Option<u8>>, Vec<Option<u8>>, usize)> {
        let set = &self.sets[choice];
        let n = set.classes.len();
        let mut lens: Vec<Option<usize>> = vec![None; n];
        let mut rates: Vec<Option<u8>> = vec![None; n];
        let mut crcs: Vec<Option<u8>> = vec![None; n];
        let mut pos = 0usize;
        for jj in 0..n {
            let k = if set.class_reordered_output {
                usize::from(set.class_output_order[jj])
            } else {
                jj
            };
            let c = &set.classes[k];
            if c.length_escape {
                let w = usize::from(c.number_of_bits_for_length.ok_or(Error::EpConfigInvalid)?);
                if w > 0 {
                    let v = take(attrib, &mut pos, w)?;
                    let mut acc = 0usize;
                    for &b in &v {
                        acc = (acc << 1) | usize::from(b);
                    }
                    lens[k] = Some(acc);
                }
            }
            if c.rate_escape {
                let v = take(attrib, &mut pos, 3)?;
                let mut acc = 0u8;
                for &b in &v {
                    acc = (acc << 1) | u8::from(b);
                }
                rates[k] = Some(acc);
            }
            if c.crclen_escape {
                let v = take(attrib, &mut pos, 3)?;
                let mut acc = 0u8;
                for &b in &v {
                    acc = (acc << 1) | u8::from(b);
                }
                crcs[k] = Some(acc);
            }
        }
        let nstuff = if self.cfg.bit_stuffing == 1 {
            let v = take(attrib, &mut pos, 3)?;
            let mut acc = 0usize;
            for &b in &v {
                acc = (acc << 1) | usize::from(b);
            }
            acc
        } else {
            0
        };
        Ok((lens, rates, crcs, nstuff))
    }

    /// Resolve every class's runtime parameters + coded length; the
    /// "until the end" class absorbs the remaining budget.
    #[allow(clippy::too_many_arguments)]
    fn resolve_frame(
        &self,
        choice: usize,
        lens: &[Option<usize>],
        rates: &[Option<u8>],
        crcs: &[Option<u8>],
        budget_bits: usize,
    ) -> Result<(Vec<ClassRt>, Vec<usize>, Vec<usize>)> {
        let set = &self.sets[choice];
        let n = set.classes.len();
        let mut rts = Vec::with_capacity(n);
        for j in 0..n {
            let mut rt = self.resolve_class(&set.classes[j])?;
            if rt.rate_escaped {
                if rt.fec_type != 0 {
                    return Err(Error::EpFrameInvalid);
                }
                let code = rates[j].ok_or(Error::EpFrameInvalid)?;
                rt.rate = *INBAND_RATE_TO_CLASS_RATE
                    .get(usize::from(code))
                    .ok_or(Error::EpFrameInvalid)?;
            }
            if rt.crc_escaped {
                let code = crcs[j].ok_or(Error::EpFrameInvalid)?;
                rt.crc_bits = *INBAND_CRC_BITS
                    .get(usize::from(code))
                    .ok_or(Error::EpFrameInvalid)?;
            }
            rts.push(rt);
        }
        // Info lengths: fixed, in-band, or until-the-end.
        let mut info_lens: Vec<Option<usize>> = Vec::with_capacity(n);
        let mut open: Option<usize> = None;
        for (j, rt) in rts.iter().enumerate() {
            let l = match (rt.len_bits, rt.len_field_bits) {
                (Some(l), _) => Some(l),
                (None, Some(_)) => Some(lens[j].ok_or(Error::EpFrameInvalid)?),
                (None, None) => {
                    if open.is_some() {
                        // Only one until-the-end class can exist.
                        return Err(Error::EpFrameInvalid);
                    }
                    open = Some(j);
                    None
                }
            };
            info_lens.push(l);
        }
        // Coded lengths of the closed classes (RS chains share their
        // parity; compute chain-aware totals).
        let mut coded_lens: Vec<usize> = vec![0; n];
        let mut consumed = 0usize;
        let mut j = 0usize;
        while j < n {
            if rts[j].fec_type == 2 {
                let mut last = j;
                while last < n && rts[last].fec_type == 2 {
                    last += 1;
                }
                if last >= n {
                    return Err(Error::EpFrameInvalid);
                }
                if (j..=last).any(|m| info_lens[m].is_none()) {
                    // An until-the-end class inside an RS chain is
                    // not resolvable.
                    return Err(Error::EpFrameInvalid);
                }
                let mut chain_bits = 0usize;
                for m in j..=last {
                    let with_crc = info_lens[m].unwrap_or(0) + rts[m].crc_bits as usize;
                    if with_crc % 8 != 0 {
                        return Err(Error::EpFrameInvalid);
                    }
                    coded_lens[m] = with_crc;
                    chain_bits += with_crc;
                }
                let two_k = 2 * usize::from(rts[last].rate);
                if two_k > 0 {
                    if two_k >= 255 {
                        return Err(Error::EpConfigInvalid);
                    }
                    let parts = (chain_bits / 8).div_ceil(255 - two_k);
                    coded_lens[last] += 8 * two_k * parts;
                }
                for &cl in coded_lens.iter().take(last + 1).skip(j) {
                    consumed += cl;
                }
                j = last + 1;
            } else {
                if let Some(l) = info_lens[j] {
                    coded_lens[j] = self.coded_len(&rts[j], l)?;
                    consumed += coded_lens[j];
                }
                j += 1;
            }
        }
        if let Some(open_j) = open {
            let remaining = budget_bits
                .checked_sub(consumed)
                .ok_or(Error::EpFrameInvalid)?;
            // Search the info length whose coded length fills the
            // remainder exactly (§1.8.4.1: the boundary is known from
            // the access-unit length).
            let rt = &rts[open_j];
            let mut found = None;
            // The coded length grows monotonically with the info
            // length; scan candidates.
            let max_info = remaining;
            let mut lo = 0usize;
            let mut hi = max_info;
            while lo <= hi {
                let mid = (lo + hi) / 2;
                let cl = self.coded_len(rt, mid);
                match cl {
                    Ok(cl) => match cl.cmp(&remaining) {
                        core::cmp::Ordering::Equal => {
                            found = Some(mid);
                            break;
                        }
                        core::cmp::Ordering::Less => lo = mid + 1,
                        core::cmp::Ordering::Greater => {
                            if mid == 0 {
                                break;
                            }
                            hi = mid - 1;
                        }
                    },
                    Err(_) => {
                        // RS byte alignment: step to the next octet.
                        lo = mid + 1;
                    }
                }
            }
            // The binary search can miss non-monotone byte-alignment
            // gaps for RS classes; fall back to a linear scan near
            // the boundary.
            if found.is_none() {
                for cand in 0..=max_info {
                    if let Ok(cl) = self.coded_len(rt, cand) {
                        if cl == remaining {
                            found = Some(cand);
                            break;
                        }
                    }
                    if cand > 4096 && rt.fec_type == 0 {
                        break;
                    }
                }
            }
            let info = found.ok_or(Error::EpFrameInvalid)?;
            info_lens[open_j] = Some(info);
            coded_lens[open_j] = remaining;
        } else if consumed != budget_bits {
            return Err(Error::EpFrameInvalid);
        }
        let infos: Vec<usize> = info_lens.into_iter().map(|l| l.unwrap_or(0)).collect();
        Ok((rts, infos, coded_lens))
    }

    fn decode_mode0(&self, bits: &[bool]) -> Result<EpFrameData> {
        let mut pos = 0usize;
        let (choice, _attrib_l, attrib) = self.read_headers(bits, &mut pos)?;
        let (lens, rates, crcs, nstuff) = self.parse_attrib(choice, &attrib)?;
        let budget = bits
            .len()
            .checked_sub(pos + nstuff)
            .ok_or(Error::EpFrameInvalid)?;
        // Without bit stuffing the byte carrier can hold up to 7
        // slack bits that are not part of the frame; with stuffing the
        // budget is exact. Try the exact budget first, then shrink.
        let mut last_err = Error::EpFrameInvalid;
        let slack_range = if self.cfg.bit_stuffing == 1 { 0 } else { 7 };
        for slack in 0..=slack_range {
            let Some(b) = budget.checked_sub(slack) else {
                break;
            };
            match self.try_decode_classes(choice, &lens, &rates, &crcs, bits, pos, b) {
                Ok(mut frame) => {
                    frame.choice_of_pred = choice;
                    return Ok(frame);
                }
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    #[allow(clippy::too_many_arguments)]
    fn try_decode_classes(
        &self,
        choice: usize,
        lens: &[Option<usize>],
        rates: &[Option<u8>],
        crcs: &[Option<u8>],
        bits: &[bool],
        mut pos: usize,
        budget: usize,
    ) -> Result<EpFrameData> {
        let set = &self.sets[choice];
        let n = set.classes.len();
        let (rts, infos, coded_lens) = self.resolve_frame(choice, lens, rates, crcs, budget)?;
        // Slice the transmitted classes.
        let mut coded: Vec<Vec<bool>> = vec![Vec::new(); n];
        for jj in 0..n {
            let k = if set.class_reordered_output {
                usize::from(set.class_output_order[jj])
            } else {
                jj
            };
            coded[k] = take(bits, &mut pos, coded_lens[k])?;
        }
        self.unprotect_all(&rts, &infos, coded, rates, crcs, choice)
    }

    /// FEC/CRC-decode all classes (chain-aware) and assemble the
    /// frame data.
    fn unprotect_all(
        &self,
        rts: &[ClassRt],
        infos: &[usize],
        coded: Vec<Vec<bool>>,
        rates: &[Option<u8>],
        crcs: &[Option<u8>],
        choice: usize,
    ) -> Result<EpFrameData> {
        let n = rts.len();
        let mut classes: Vec<Vec<bool>> = vec![Vec::new(); n];
        let mut j = 0usize;
        while j < n {
            if rts[j].fec_type == 2 {
                let mut last = j;
                while last < n && rts[last].fec_type == 2 {
                    last += 1;
                }
                if last >= n {
                    return Err(Error::EpFrameInvalid);
                }
                // Reassemble the chain: members' CRC-protected bits +
                // the parity on the last member.
                let mut chain: Vec<bool> = Vec::new();
                for (m, c) in coded.iter().enumerate().take(last + 1).skip(j) {
                    let with_crc = infos[m] + rts[m].crc_bits as usize;
                    if c.len() < with_crc {
                        return Err(Error::EpFrameInvalid);
                    }
                    chain.extend_from_slice(&c[..with_crc]);
                }
                let parity_bits = &coded[last][infos[last] + rts[last].crc_bits as usize..];
                let mut data = bits_to_bytes(&chain);
                let parity = bits_to_bytes(parity_bits);
                srs_decode(&mut data, &parity, usize::from(rts[last].rate))?;
                let chain_bits = bytes_to_bits(&data);
                let mut off = 0usize;
                for m in j..=last {
                    let with_crc = infos[m] + rts[m].crc_bits as usize;
                    let seg = &chain_bits[off..off + with_crc];
                    off += with_crc;
                    let mut info = seg[..infos[m]].to_vec();
                    if rts[m].crc_bits > 0 {
                        let poly =
                            EpClass::crc_poly(rts[m].crc_bits)?.ok_or(Error::EpFrameInvalid)?;
                        let want = crc_bits(poly, &info);
                        let mut got = 0u64;
                        for &b in &seg[infos[m]..] {
                            got = (got << 1) | u64::from(b);
                        }
                        if got != want {
                            return Err(Error::EpFrameInvalid);
                        }
                    }
                    core::mem::swap(&mut classes[m], &mut info);
                }
                j = last + 1;
            } else {
                classes[j] = self.unprotect_class(&rts[j], &coded[j], infos[j])?;
                j += 1;
            }
        }
        Ok(EpFrameData {
            choice_of_pred: choice,
            classes,
            rate_codes: rates.to_vec(),
            crc_codes: crcs.to_vec(),
        })
    }

    fn decode_interleaved(&self, bits: &[bool]) -> Result<EpFrameData> {
        let mode2 = self.cfg.interleave_type == 2;
        // Reverse the header stages: choice_of_pred first.
        let npred = self.npred() as usize;
        let mut stream: Vec<bool> = bits.to_vec();
        let choice = if npred > 0 {
            let xl = npred + self.header_parity_len(npred)?;
            let w = self.header_width(npred)?;
            let (x, y) = deinterleave_units_bits(&stream, xl, w)?;
            stream = y;
            let corrected = self.header_unprotect(&x[..npred], &x[npred..])?;
            let mut v = 0usize;
            for &b in &corrected {
                v = (v << 1) | usize::from(b);
            }
            v
        } else {
            0
        };
        let set = self.sets.get(choice).ok_or(Error::EpFrameInvalid)?;
        let mut attrib_l = 0usize;
        for c in &set.classes {
            if c.length_escape {
                attrib_l += usize::from(c.number_of_bits_for_length.unwrap_or(0));
            }
            if c.rate_escape {
                attrib_l += 3;
            }
            if c.crclen_escape {
                attrib_l += 3;
            }
        }
        if self.cfg.bit_stuffing == 1 {
            attrib_l += 3;
        }
        let attrib = if attrib_l > 0 {
            let xl = attrib_l + self.header_parity_len(attrib_l)?;
            let w = self.header_width(attrib_l)?;
            let (x, y) = deinterleave_units_bits(&stream, xl, w)?;
            stream = y;
            self.header_unprotect(&x[..attrib_l], &x[attrib_l..])?
        } else {
            Vec::new()
        };
        let (lens, rates, crcs, nstuff) = self.parse_attrib(choice, &attrib)?;

        // The class stream: everything minus trailing slack/stuffing.
        let mut last_err = Error::EpFrameInvalid;
        let slack_range = if self.cfg.bit_stuffing == 1 { 0 } else { 7 };
        for slack in 0..=slack_range {
            let Some(budget) = stream.len().checked_sub(nstuff + slack) else {
                break;
            };
            match self.try_decode_interleaved_classes(
                choice,
                &lens,
                &rates,
                &crcs,
                &stream[..budget],
                mode2,
            ) {
                Ok(frame) => return Ok(frame),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    fn try_decode_interleaved_classes(
        &self,
        choice: usize,
        lens: &[Option<usize>],
        rates: &[Option<u8>],
        crcs: &[Option<u8>],
        class_stream: &[bool],
        mode2: bool,
    ) -> Result<EpFrameData> {
        let set = &self.sets[choice];
        let n = set.classes.len();
        let (rts, infos, coded_lens) =
            self.resolve_frame(choice, lens, rates, crcs, class_stream.len())?;
        let tx_order: Vec<usize> = (0..n)
            .map(|jj| {
                if set.class_reordered_output {
                    usize::from(set.class_output_order[jj])
                } else {
                    jj
                }
            })
            .collect();
        // Undo the class-stage interleaving: the encoder ran the
        // reverse loop last-to-first, so decode unwinds first-to-last.
        // In mode 2 the non-interleaved (switch-0) classes were
        // appended AFTER the interleave stages — split them off the
        // tail before unwinding.
        let mut buf_no_len = 0usize;
        if mode2 {
            for (k, rt) in rts.iter().enumerate() {
                if rt.interleave_switch == 0 {
                    buf_no_len += coded_lens[k];
                }
            }
        }
        if buf_no_len > class_stream.len() {
            return Err(Error::EpFrameInvalid);
        }
        let (inter_part, buf_no) = class_stream.split_at(class_stream.len() - buf_no_len);
        let mut stream = inter_part.to_vec();
        let mut coded: Vec<Vec<bool>> = vec![Vec::new(); n];
        // Interleaved classes, in the encoder's reverse-of-reverse
        // order (i.e. transmitted forward order).
        for &k in tx_order.iter().take(n) {
            let sw = if mode2 { rts[k].interleave_switch } else { 1 };
            if mode2 && (sw == 0 || sw == 3) {
                continue;
            }
            let bytewise = rts[k].fec_type != 0;
            let w_units = if mode2 && sw == 2 {
                28
            } else if bytewise {
                if coded_lens[k] % 8 != 0 {
                    return Err(Error::EpFrameInvalid);
                }
                coded_lens[k] / 8
            } else if mode2 {
                coded_lens[k]
            } else {
                28
            };
            if bytewise {
                if stream.len() % 8 != 0 {
                    return Err(Error::EpFrameInvalid);
                }
                let z = bits_to_bytes(&stream);
                let (x, y) = deinterleave_units(&z, coded_lens[k] / 8, w_units)?;
                coded[k] = bytes_to_bits(&x);
                stream = bytes_to_bits(&y);
            } else {
                let (x, y) = deinterleave_units_bits(&stream, coded_lens[k], w_units)?;
                coded[k] = x;
                stream = y;
            }
        }
        if mode2 {
            // The remaining stream is the innermost BUF_Y: the
            // switch-3 concatenated classes in forward order.
            let mut pos = 0usize;
            for &k in tx_order.iter().take(n) {
                if rts[k].interleave_switch == 3 {
                    coded[k] = take(&stream, &mut pos, coded_lens[k])?;
                }
            }
            if pos != stream.len() {
                return Err(Error::EpFrameInvalid);
            }
            // The switch-0 classes ride the tail suffix.
            let mut pos = 0usize;
            for &k in tx_order.iter().take(n) {
                if rts[k].interleave_switch == 0 {
                    coded[k] = take(buf_no, &mut pos, coded_lens[k])?;
                }
            }
            if pos != buf_no.len() {
                return Err(Error::EpFrameInvalid);
            }
        } else if !stream.is_empty() {
            return Err(Error::EpFrameInvalid);
        }
        self.unprotect_all(&rts, &infos, coded, rates, crcs, choice)
    }
}

/// §1.8.4.8.1 recursive interleaver over generic units: X row-major,
/// Y filling the residual cells column-wise, output read column-major
/// with `k = m·D + min(m, d) + n`.
fn interleave_units<T: Copy + Default>(x: &[T], y: &[T], w: usize) -> Result<Vec<T>> {
    if w == 0 {
        return Err(Error::EpFrameInvalid);
    }
    let total = x.len() + y.len();
    let d_rows = total / w;
    let d = total - d_rows * w;
    let col_height = |m: usize| d_rows + usize::from(m < d);
    let k_of = |m: usize, n: usize| m * d_rows + m.min(d) + n;

    let dp = x.len() / w;
    let dpr = x.len() - dp * w;

    let mut out = vec![T::default(); total];
    // X: row-major.
    for (i, &v) in x.iter().enumerate() {
        let m = i % w;
        let n = i / w;
        out[k_of(m, n)] = v;
    }
    // Y: column-wise into the residual cells.
    let mut yi = 0usize;
    for m in 0..w {
        let start = dp + usize::from(m < dpr);
        for n in start..col_height(m) {
            if yi >= y.len() {
                return Err(Error::EpFrameInvalid);
            }
            out[k_of(m, n)] = y[yi];
            yi += 1;
        }
    }
    if yi != y.len() {
        return Err(Error::EpFrameInvalid);
    }
    Ok(out)
}

/// Inverse of [`interleave_units`] given `lx` and the width.
fn deinterleave_units<T: Copy + Default>(z: &[T], lx: usize, w: usize) -> Result<(Vec<T>, Vec<T>)> {
    if w == 0 || lx > z.len() {
        return Err(Error::EpFrameInvalid);
    }
    let total = z.len();
    let d_rows = total / w;
    let d = total - d_rows * w;
    let col_height = |m: usize| d_rows + usize::from(m < d);
    let k_of = |m: usize, n: usize| m * d_rows + m.min(d) + n;
    let dp = lx / w;
    let dpr = lx - dp * w;
    let mut x = vec![T::default(); lx];
    for (i, xv) in x.iter_mut().enumerate() {
        let m = i % w;
        let n = i / w;
        *xv = z[k_of(m, n)];
    }
    let mut y = Vec::with_capacity(total - lx);
    for m in 0..w {
        let start = dp + usize::from(m < dpr);
        for n in start..col_height(m) {
            y.push(z[k_of(m, n)]);
        }
    }
    Ok((x, y))
}

fn deinterleave_units_bits(z: &[bool], lx: usize, w: usize) -> Result<(Vec<bool>, Vec<bool>)> {
    deinterleave_units(z, lx, w)
}

fn take(bits: &[bool], pos: &mut usize, n: usize) -> Result<Vec<bool>> {
    if *pos + n > bits.len() {
        return Err(Error::EpFrameInvalid);
    }
    let v = bits[*pos..*pos + n].to_vec();
    *pos += n;
    Ok(v)
}

fn bits_to_bytes(bits: &[bool]) -> Vec<u8> {
    debug_assert_eq!(bits.len() % 8, 0);
    bits.chunks(8)
        .map(|c| c.iter().fold(0u8, |acc, &b| (acc << 1) | u8::from(b)))
        .collect()
}

fn bits_to_bytes_padded(bits: &[bool]) -> Vec<u8> {
    let mut v = Vec::with_capacity(bits.len().div_ceil(8));
    for chunk in bits.chunks(8) {
        let mut b = 0u8;
        for (i, &bit) in chunk.iter().enumerate() {
            if bit {
                b |= 0x80 >> i;
            }
        }
        v.push(b);
    }
    v
}

fn bytes_to_bits(bytes: &[u8]) -> Vec<bool> {
    let mut v = Vec::with_capacity(bytes.len() * 8);
    for &b in bytes {
        for i in 0..8 {
            v.push(b & (0x80 >> i) != 0);
        }
    }
    v
}

/// Emit a parsed frame back through a [`BitWriter`] (whole bytes).
pub fn write_frame(w: &mut BitWriter, frame_bytes: &[u8]) {
    for &b in frame_bytes {
        w.write_u32(u32::from(b), 8);
    }
}

/// Convenience: read the remaining whole bytes of a reader.
pub fn read_remaining_bytes(reader: &mut BitReader<'_>, total_len: usize) -> Result<Vec<u8>> {
    let pos = reader.bit_position() as usize;
    if pos % 8 != 0 {
        return Err(Error::EpFrameInvalid);
    }
    let mut out = Vec::with_capacity(total_len - pos / 8);
    for _ in (pos / 8)..total_len {
        out.push(reader.read_u32(8).map_err(|_| Error::UnexpectedEnd)? as u8);
    }
    Ok(out)
}

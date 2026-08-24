//! `ErrorProtectionSpecificConfig()` — ISO/IEC 14496-3 §1.8.2.1
//! Table 1.49, the out-of-band half of the §1.8 error-protection (EP)
//! tool, plus the §1.8.4.2 pre-defined-set derivation.
//!
//! The EP tool protects an access unit as a sequence of *classes*
//! (§1.8.1): each class carries a CRC (§1.8.4.5), an FEC — SRCPC
//! (§1.8.4.6) or shortened Reed-Solomon (§1.8.4.7) — and optional
//! interleaving (§1.8.4.8). Everything constant across frames rides
//! this configuration; the per-frame remainder (choice of pre-defined
//! set, escaped class parameters, stuffing count) rides the in-band
//! `ep_header()` (§1.8.2.2 / §1.8.4.3).
//!
//! The `class_optional` unwrapping (§1.8.4.2) expands every wire
//! pre-defined set with `N` optional classes into `2^N` transmission
//! sets — from "all optional classes present" (`j == 0`) down to
//! "none present"; [`ErrorProtectionSpecificConfig::expand`] is that
//! algorithm verbatim, and the in-band `choice_of_pred` indexes the
//! **expanded** list.

use oxideav_core::bits::{BitReader, BitWriter};

use crate::crc::CrcPoly;
use crate::{Error, Result};

/// Per-class parameters of one wire pre-defined set (Table 1.49 inner
/// loop).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpClass {
    /// `length_escape` — `true` ⇒ the class length is signalled
    /// in-band with `number_of_bits_for_length` bits (0 = the
    /// §1.8.4.1 "until the end" class).
    pub length_escape: bool,
    /// `rate_escape` — `true` ⇒ the code rate is signalled in-band.
    pub rate_escape: bool,
    /// `crclen_escape` — `true` ⇒ the CRC length is signalled
    /// in-band.
    pub crclen_escape: bool,
    /// `concatenate_flag` — present on the wire only when
    /// `number_of_concatenated_frame != 1` (§1.8.4.4); `false`
    /// otherwise.
    pub concatenate_flag: bool,
    /// `fec_type` (2 bits): `0` SRCPC; `1` RS (last / independent);
    /// `2` RS concatenated with the next class.
    pub fec_type: u8,
    /// `termination_switch` — present iff `fec_type == 0`
    /// (§1.8.4.6.2).
    pub termination_switch: Option<bool>,
    /// `interleave_switch` (2 bits) — present iff
    /// `interleave_type == 2` (Table 1.64).
    pub interleave_switch: Option<u8>,
    /// `class_optional` — the §1.8.4.2 expansion flag.
    pub class_optional: bool,
    /// `number_of_bits_for_length` (4 bits) iff `length_escape`.
    pub number_of_bits_for_length: Option<u8>,
    /// `class_length` (16 bits) iff `!length_escape`. **Bits** for
    /// SRCPC classes; must be a whole number of octets for RS classes
    /// (§1.8.3.1 `fec_type`).
    pub class_length: Option<u16>,
    /// `class_rate` iff `!rate_escape` — 5 bits for SRCPC (0..=24 ⇒
    /// rate 8/8..8/32), 7 bits for RS (the number of correctable
    /// bytes `k`, §1.8.4.7).
    pub class_rate: Option<u8>,
    /// `class_crclen` (5 bits) iff `!crclen_escape` — 0..=18 ⇒ CRC
    /// length 0..=16 / 24 / 32 (§1.8.3.1).
    pub class_crclen: Option<u8>,
}

impl EpClass {
    /// Resolve the §1.8.3.1 `class_crclen` code (0..=18) to a CRC bit
    /// width.
    pub fn crclen_bits(code: u8) -> Result<u32> {
        Ok(match code {
            0..=16 => u32::from(code),
            17 => 24,
            18 => 32,
            _ => return Err(Error::EpConfigInvalid),
        })
    }

    /// The §1.8.4.5 generator for a CRC width produced by
    /// [`EpClass::crclen_bits`] (widths 1..=16, 24, 32).
    pub fn crc_poly(width: u32) -> Result<Option<CrcPoly>> {
        Ok(Some(match width {
            0 => return Ok(None),
            1 => CrcPoly::Crc1,
            2 => CrcPoly::Crc2,
            3 => CrcPoly::Crc3,
            4 => CrcPoly::Crc4,
            5 => CrcPoly::Crc5,
            6 => CrcPoly::Crc6,
            7 => CrcPoly::Crc7,
            8 => CrcPoly::Crc8,
            9 => CrcPoly::Crc9,
            10 => CrcPoly::Crc10,
            11 => CrcPoly::Crc11,
            12 => CrcPoly::Crc12,
            13 => CrcPoly::Crc13,
            14 => CrcPoly::Crc14,
            15 => CrcPoly::Crc15,
            16 => CrcPoly::Crc16,
            24 => CrcPoly::Crc24,
            32 => CrcPoly::Crc32,
            _ => return Err(Error::EpConfigInvalid),
        }))
    }
}

/// One pre-defined set (Table 1.49 outer loop): the class list plus
/// the §1.8.4.9 output reordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpPredefinedSet {
    /// The per-class parameter list.
    pub classes: Vec<EpClass>,
    /// `class_reordered_output` (§1.8.4.9).
    pub class_reordered_output: bool,
    /// `class_output_order[j]` (6 bits each) iff reordered: the j-th
    /// EP-frame class is output as the `class_output_order[j]`-th
    /// class to the audio decoder.
    pub class_output_order: Vec<u8>,
}

/// Parsed `ErrorProtectionSpecificConfig()` (Table 1.49).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorProtectionSpecificConfig {
    /// `interleave_type` (2 bits): 0 none, 1 intra-frame, 2 per-class
    /// fine tuning; 3 reserved (rejected).
    pub interleave_type: u8,
    /// `bit_stuffing` (3 bits): 1 ⇒ `num_stuffing_bits` rides
    /// `class_attrib()`.
    pub bit_stuffing: u8,
    /// `number_of_concatenated_frame` (3 bits): source frames per EP
    /// frame; 0 is reserved (Table 1.54).
    pub number_of_concatenated_frame: u8,
    /// The wire pre-defined sets (before §1.8.4.2 expansion).
    pub sets: Vec<EpPredefinedSet>,
    /// `header_protection`: extended in-band header FEC (§1.8.4.3).
    pub header_protection: bool,
    /// `header_rate` (5 bits) iff `header_protection`.
    pub header_rate: Option<u8>,
    /// `header_crclen` (5 bits) iff `header_protection`.
    pub header_crclen: Option<u8>,
}

impl ErrorProtectionSpecificConfig {
    /// Parse a Table 1.49 configuration.
    pub fn parse(reader: &mut BitReader<'_>) -> Result<Self> {
        let number_of_predefined_set = read_u8(reader, 8)?;
        let interleave_type = read_u8(reader, 2)?;
        if interleave_type == 3 {
            // §1.8.3.1: reserved.
            return Err(Error::EpConfigInvalid);
        }
        let bit_stuffing = read_u8(reader, 3)?;
        let number_of_concatenated_frame = read_u8(reader, 3)?;
        if number_of_concatenated_frame == 0 {
            // Table 1.54: codeword 000 is reserved.
            return Err(Error::EpConfigInvalid);
        }
        let mut sets = Vec::with_capacity(usize::from(number_of_predefined_set));
        for _i in 0..number_of_predefined_set {
            let number_of_class = read_u8(reader, 6)?;
            let mut classes = Vec::with_capacity(usize::from(number_of_class));
            for _j in 0..number_of_class {
                let length_escape = read_bit(reader)?;
                let rate_escape = read_bit(reader)?;
                let crclen_escape = read_bit(reader)?;
                let concatenate_flag = if number_of_concatenated_frame != 1 {
                    read_bit(reader)?
                } else {
                    false
                };
                let fec_type = read_u8(reader, 2)?;
                if fec_type == 3 {
                    return Err(Error::EpConfigInvalid);
                }
                let termination_switch = if fec_type == 0 {
                    Some(read_bit(reader)?)
                } else {
                    None
                };
                let interleave_switch = if interleave_type == 2 {
                    let v = read_u8(reader, 2)?;
                    // Table 1.64: width-28 intraclass interleaving is
                    // SRCPC-only.
                    if v == 2 && fec_type != 0 {
                        return Err(Error::EpConfigInvalid);
                    }
                    Some(v)
                } else {
                    None
                };
                let class_optional = read_bit(reader)?;
                let (number_of_bits_for_length, class_length) = if length_escape {
                    (Some(read_u8(reader, 4)?), None)
                } else {
                    (None, Some(read_u16(reader, 16)?))
                };
                let class_rate = if !rate_escape {
                    let bits = if fec_type != 0 { 7 } else { 5 };
                    let v = read_u8(reader, bits)?;
                    if fec_type == 0 && v > 24 {
                        // §1.8.3.1: 0..=24 map onto 8/8..8/32.
                        return Err(Error::EpConfigInvalid);
                    }
                    Some(v)
                } else {
                    None
                };
                let class_crclen = if !crclen_escape {
                    let v = read_u8(reader, 5)?;
                    EpClass::crclen_bits(v)?;
                    Some(v)
                } else {
                    None
                };
                classes.push(EpClass {
                    length_escape,
                    rate_escape,
                    crclen_escape,
                    concatenate_flag,
                    fec_type,
                    termination_switch,
                    interleave_switch,
                    class_optional,
                    number_of_bits_for_length,
                    class_length,
                    class_rate,
                    class_crclen,
                });
            }
            let class_reordered_output = read_bit(reader)?;
            let mut class_output_order = Vec::new();
            if class_reordered_output {
                for _j in 0..number_of_class {
                    let v = read_u8(reader, 6)?;
                    if v >= number_of_class {
                        return Err(Error::EpConfigInvalid);
                    }
                    class_output_order.push(v);
                }
                // The order must be a permutation of 0..number_of_class.
                let mut seen = vec![false; usize::from(number_of_class)];
                for &v in &class_output_order {
                    if core::mem::replace(&mut seen[usize::from(v)], true) {
                        return Err(Error::EpConfigInvalid);
                    }
                }
            }
            sets.push(EpPredefinedSet {
                classes,
                class_reordered_output,
                class_output_order,
            });
        }
        let header_protection = read_bit(reader)?;
        let (header_rate, header_crclen) = if header_protection {
            let rate = read_u8(reader, 5)?;
            if rate > 24 {
                return Err(Error::EpConfigInvalid);
            }
            let crclen = read_u8(reader, 5)?;
            EpClass::crclen_bits(crclen)?;
            (Some(rate), Some(crclen))
        } else {
            (None, None)
        };
        Ok(ErrorProtectionSpecificConfig {
            interleave_type,
            bit_stuffing,
            number_of_concatenated_frame,
            sets,
            header_protection,
            header_rate,
            header_crclen,
        })
    }

    /// Emit the Table 1.49 configuration — the bit-exact inverse of
    /// [`ErrorProtectionSpecificConfig::parse`].
    pub fn write(&self, w: &mut BitWriter) -> Result<()> {
        if self.sets.len() > 255
            || self.interleave_type > 2
            || self.number_of_concatenated_frame == 0
            || self.number_of_concatenated_frame > 7
            || self.bit_stuffing > 7
        {
            return Err(Error::EpConfigInvalid);
        }
        w.write_u32(self.sets.len() as u32, 8);
        w.write_u32(u32::from(self.interleave_type), 2);
        w.write_u32(u32::from(self.bit_stuffing), 3);
        w.write_u32(u32::from(self.number_of_concatenated_frame), 3);
        for set in &self.sets {
            if set.classes.len() > 63 {
                return Err(Error::EpConfigInvalid);
            }
            w.write_u32(set.classes.len() as u32, 6);
            for c in &set.classes {
                w.write_bit(c.length_escape);
                w.write_bit(c.rate_escape);
                w.write_bit(c.crclen_escape);
                if self.number_of_concatenated_frame != 1 {
                    w.write_bit(c.concatenate_flag);
                }
                if c.fec_type > 2 {
                    return Err(Error::EpConfigInvalid);
                }
                w.write_u32(u32::from(c.fec_type), 2);
                if c.fec_type == 0 {
                    w.write_bit(c.termination_switch.ok_or(Error::EpConfigInvalid)?);
                }
                if self.interleave_type == 2 {
                    w.write_u32(
                        u32::from(c.interleave_switch.ok_or(Error::EpConfigInvalid)?),
                        2,
                    );
                }
                w.write_bit(c.class_optional);
                if c.length_escape {
                    let n = c.number_of_bits_for_length.ok_or(Error::EpConfigInvalid)?;
                    if n > 15 {
                        return Err(Error::EpConfigInvalid);
                    }
                    w.write_u32(u32::from(n), 4);
                } else {
                    w.write_u32(u32::from(c.class_length.ok_or(Error::EpConfigInvalid)?), 16);
                }
                if !c.rate_escape {
                    let bits = if c.fec_type != 0 { 7 } else { 5 };
                    w.write_u32(u32::from(c.class_rate.ok_or(Error::EpConfigInvalid)?), bits);
                }
                if !c.crclen_escape {
                    w.write_u32(u32::from(c.class_crclen.ok_or(Error::EpConfigInvalid)?), 5);
                }
            }
            w.write_bit(set.class_reordered_output);
            if set.class_reordered_output {
                if set.class_output_order.len() != set.classes.len() {
                    return Err(Error::EpConfigInvalid);
                }
                for &v in &set.class_output_order {
                    w.write_u32(u32::from(v), 6);
                }
            }
        }
        w.write_bit(self.header_protection);
        if self.header_protection {
            w.write_u32(
                u32::from(self.header_rate.ok_or(Error::EpConfigInvalid)?),
                5,
            );
            w.write_u32(
                u32::from(self.header_crclen.ok_or(Error::EpConfigInvalid)?),
                5,
            );
        }
        Ok(())
    }

    /// §1.8.4.2 — expand the `class_optional` flags into the
    /// transmission pre-defined sets the in-band `choice_of_pred`
    /// indexes.
    ///
    /// Each wire set with `N` optional classes yields `2^N` sets, from
    /// "all optional classes present" (`j == 0`) to "none present"
    /// (`j == 2^N − 1`); bit `k` of `j` clears the `k`-th optional
    /// class. The expanded sets carry `class_optional == false`
    /// throughout.
    pub fn expand(&self) -> Result<Vec<EpPredefinedSet>> {
        let mut out = Vec::new();
        for set in &self.sets {
            let opt_idx: Vec<usize> = set
                .classes
                .iter()
                .enumerate()
                .filter(|(_, c)| c.class_optional)
                .map(|(i, _)| i)
                .collect();
            let nco = opt_idx.len();
            if nco > 16 {
                // 2^N sets would be unbounded; a conforming config
                // never needs this many optional classes.
                return Err(Error::EpConfigInvalid);
            }
            for j in 0u32..(1u32 << nco) {
                let mut classes = Vec::with_capacity(set.classes.len());
                let mut kept_index = Vec::with_capacity(set.classes.len());
                for (i, c) in set.classes.iter().enumerate() {
                    let keep = match opt_idx.iter().position(|&o| o == i) {
                        Some(k) => j & (1 << k) == 0,
                        None => true,
                    };
                    if keep {
                        let mut cc = c.clone();
                        cc.class_optional = false;
                        classes.push(cc);
                        kept_index.push(i);
                    }
                }
                // The output order shrinks with the dropped classes:
                // surviving entries keep their relative order.
                let class_output_order = if set.class_reordered_output {
                    let mut order: Vec<u8> = Vec::with_capacity(classes.len());
                    // Rank the surviving original output positions.
                    let mut kept_orders: Vec<u8> = kept_index
                        .iter()
                        .map(|&i| set.class_output_order[i])
                        .collect();
                    let mut sorted = kept_orders.clone();
                    sorted.sort_unstable();
                    for v in kept_orders.iter_mut() {
                        let rank = sorted.iter().position(|&s| s == *v).unwrap_or(0) as u8;
                        order.push(rank);
                    }
                    order
                } else {
                    Vec::new()
                };
                out.push(EpPredefinedSet {
                    classes,
                    class_reordered_output: set.class_reordered_output,
                    class_output_order,
                });
            }
        }
        if out.is_empty() {
            return Err(Error::EpConfigInvalid);
        }
        Ok(out)
    }
}

fn read_bit(reader: &mut BitReader<'_>) -> Result<bool> {
    reader.read_bit().map_err(|_| Error::UnexpectedEnd)
}

fn read_u8(reader: &mut BitReader<'_>, bits: u32) -> Result<u8> {
    Ok(reader.read_u32(bits).map_err(|_| Error::UnexpectedEnd)? as u8)
}

fn read_u16(reader: &mut BitReader<'_>, bits: u32) -> Result<u16> {
    Ok(reader.read_u32(bits).map_err(|_| Error::UnexpectedEnd)? as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_class(len: u16, rate: u8, crclen: u8) -> EpClass {
        EpClass {
            length_escape: false,
            rate_escape: false,
            crclen_escape: false,
            concatenate_flag: false,
            fec_type: 0,
            termination_switch: Some(true),
            interleave_switch: None,
            class_optional: false,
            number_of_bits_for_length: None,
            class_length: Some(len),
            class_rate: Some(rate),
            class_crclen: Some(crclen),
        }
    }

    #[test]
    fn roundtrip_two_sets() {
        let cfg = ErrorProtectionSpecificConfig {
            interleave_type: 0,
            bit_stuffing: 0,
            number_of_concatenated_frame: 1,
            sets: vec![
                EpPredefinedSet {
                    classes: vec![simple_class(40, 8, 6), simple_class(100, 0, 0)],
                    class_reordered_output: false,
                    class_output_order: Vec::new(),
                },
                EpPredefinedSet {
                    classes: vec![simple_class(24, 24, 8)],
                    class_reordered_output: false,
                    class_output_order: Vec::new(),
                },
            ],
            header_protection: false,
            header_rate: None,
            header_crclen: None,
        };
        let mut w = BitWriter::new();
        cfg.write(&mut w).unwrap();
        let bytes = w.finish();
        let mut r = BitReader::new(&bytes);
        let parsed = ErrorProtectionSpecificConfig::parse(&mut r).unwrap();
        assert_eq!(parsed, cfg);
    }

    /// The §1.8.4.2 example: pred #0 with optional classes A, C, E of
    /// {A, B, C, D, E} and pred #1 with optional F of {F, G} expand
    /// into the Table 1.58 ten sets.
    #[test]
    fn expansion_matches_table_1_58() {
        // Give every class a distinct length so the expanded sets are
        // recognisable.
        let mk = |len: u16, opt: bool| EpClass {
            class_optional: opt,
            ..simple_class(len, 0, 0)
        };
        let cfg = ErrorProtectionSpecificConfig {
            interleave_type: 0,
            bit_stuffing: 0,
            number_of_concatenated_frame: 1,
            sets: vec![
                EpPredefinedSet {
                    // A=1(opt) B=2 C=3(opt) D=4 E=5(opt)
                    classes: vec![
                        mk(1, true),
                        mk(2, false),
                        mk(3, true),
                        mk(4, false),
                        mk(5, true),
                    ],
                    class_reordered_output: false,
                    class_output_order: Vec::new(),
                },
                EpPredefinedSet {
                    // F=6(opt) G=7
                    classes: vec![mk(6, true), mk(7, false)],
                    class_reordered_output: false,
                    class_output_order: Vec::new(),
                },
            ],
            header_protection: false,
            header_rate: None,
            header_crclen: None,
        };
        let expanded = cfg.expand().unwrap();
        let lens: Vec<Vec<u16>> = expanded
            .iter()
            .map(|s| s.classes.iter().map(|c| c.class_length.unwrap()).collect())
            .collect();
        // Table 1.58 columns (A..G as 1..7).
        assert_eq!(
            lens,
            vec![
                vec![1, 2, 3, 4, 5], // all present
                vec![2, 3, 4, 5],    // A absent
                vec![1, 2, 4, 5],    // C absent
                vec![2, 4, 5],       // A, C absent
                vec![1, 2, 3, 4],    // E absent
                vec![2, 3, 4],       // A, E absent
                vec![1, 2, 4],       // C, E absent
                vec![2, 4],          // A, C, E absent
                vec![6, 7],          // pred #1, F present
                vec![7],             // pred #1, F absent
            ]
        );
    }

    #[test]
    fn reserved_fields_rejected() {
        // interleave_type == 3.
        let mut w = BitWriter::new();
        w.write_u32(1, 8); // number_of_predefined_set
        w.write_u32(3, 2); // interleave_type (reserved)
        w.write_u32(0, 3);
        w.write_u32(1, 3);
        let bytes = w.finish();
        let mut r = BitReader::new(&bytes);
        assert_eq!(
            ErrorProtectionSpecificConfig::parse(&mut r).unwrap_err(),
            Error::EpConfigInvalid
        );

        // number_of_concatenated_frame == 0 (Table 1.54 reserved).
        let mut w = BitWriter::new();
        w.write_u32(1, 8);
        w.write_u32(0, 2);
        w.write_u32(0, 3);
        w.write_u32(0, 3); // reserved
        let bytes = w.finish();
        let mut r = BitReader::new(&bytes);
        assert_eq!(
            ErrorProtectionSpecificConfig::parse(&mut r).unwrap_err(),
            Error::EpConfigInvalid
        );
    }
}

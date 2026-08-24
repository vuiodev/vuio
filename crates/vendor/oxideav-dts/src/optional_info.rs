//! §5.6 "Unpack Optional Information" (Table 5-30): the flag-gated
//! region that follows the last audio-data array of a Core frame.
//!
//! Transcribed from ETSI TS 102 114 V1.3.1 (2011-08) §5.6
//! (Table 5-30 + field descriptions, PDF p.33-34), staged at
//! `docs/audio/dts/etsi-ts-102114-dts-coherent-acoustics.pdf`:
//!
//! ```text
//! if ( TIMEF==1 )               // Present only when TIMEF=1.
//!   TIMES = ExtractBits(32);
//! if ( AUXF==1 )                // Present only if AUXF=1.
//!   AUXCT = ExtractBits(6);
//! else
//!   AUXCT = 0;                  // Clear it.
//! ByteAlign = ExtractBits(0 ... 7);
//! for (int n=0; n<AUXCT; n++)
//!   AUXD[n] = ExtractBits(8);
//! if ( (CPF==1) && (DYNF!=0) )
//!   OCRC = ExtractBits(16);
//! ```
//!
//! The walker starts at the bit cursor left by the last §5.5
//! audio-data array — [`crate::SubframePcmDecoder`] exposes it through
//! the `*_with_info` decode entry points, which run this walk after
//! the reconstruction loop.
//!
//! Two Table 5-30 caveats, kept as documented in the staged spec:
//!
//! - The pseudocode's `ByteAlign` consumes `0..=7` bits (to the next
//!   byte boundary), while the `ZeroPadAux` field description says
//!   the auxiliary data bytes begin "on the 32-bit boundary from the
//!   beginning of the core stream". The two disagree whenever the
//!   byte-aligned cursor is not also DWORD-aligned; §5.7.1 resolves
//!   the tension by recommending navigation *into* the aux content
//!   via the DWORD-aligned sync-word search
//!   ([`crate::find_aux_data`] / [`crate::parse_aux_data`]) instead
//!   of the `AUXCT` cursor. This walker follows the pseudocode
//!   literally (byte alignment) and surfaces the raw `AUXD` bytes;
//!   use the §5.7.1 parser for their content.
//! - `OCRC`: "The CRC value test shall not be applied" — the Annex B
//!   algorithm is documented ([`crate::dts_crc16`],
//!   `docs/audio/dts/dts-crc16.md`) but the core check words are
//!   normatively untested placeholders, so the word is surfaced raw.

use crate::bitreader::BitReader;
use crate::header::DtsFrameHeader;
use crate::Result;

/// The maximum §5.6 `AUXCT` auxiliary byte count (6-bit field,
/// documented range 1..=63 when `AUXF == 1`).
pub const MAX_AUX_BYTE_COUNT: u8 = 63;

/// A decoded §5.6 Table 5-30 optional-information region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalInfo {
    /// `TIMES` (present when the frame header's `TIMEF` flag is set):
    /// the 32-bit time code stamp, "used to align audio to video".
    pub time_code_stamp: Option<u32>,
    /// `AUXD`: the raw auxiliary data bytes (`AUXCT` of them; empty
    /// when `AUXF == 0`). Their §5.7.1 content is parsed with
    /// [`crate::parse_aux_data`] over the whole frame (the spec's
    /// recommended sync-word navigation).
    pub aux_bytes: Vec<u8>,
    /// `OCRC` (present when `CPF == 1 && DYNF != 0`): the optional
    /// CRC check word. Per §5.6 "The CRC value test shall not be
    /// applied" — surfaced raw.
    pub ocrc: Option<u16>,
}

/// Walk the §5.6 Table 5-30 optional-information region of a Core
/// frame from `bit_offset` (the cursor left by the last §5.5
/// audio-data array), gated by the frame header's `TIMEF` / `AUXF` /
/// `CPF` / `DYNF` flags.
///
/// Returns the decoded region plus the number of bits consumed from
/// `bit_offset`.
///
/// # Errors
///
/// [`crate::Error::UnexpectedEof`] when a gated field would walk past
/// the end of `bytes`.
pub fn decode_optional_info_at(
    bytes: &[u8],
    bit_offset: usize,
    header: &DtsFrameHeader,
) -> Result<(OptionalInfo, usize)> {
    let mut br = BitReader::from_byte_offset(bytes, 0);
    br.skip_bits(bit_offset as u32)?;

    let time_code_stamp = if header.time_stamp {
        Some(br.read_bits(32)?)
    } else {
        None
    };

    let aux_count = if header.aux_data {
        br.read_bits(6)? as usize
    } else {
        0
    };

    // ByteAlign = ExtractBits(0 ... 7): zero-pad to the next byte
    // boundary before the AUXD byte array (see the module docs for
    // the ZeroPadAux DWORD-alignment caveat).
    let misalign = (br.absolute_bit_position() % 8) as u32;
    if misalign != 0 {
        br.skip_bits(8 - misalign)?;
    }

    let mut aux_bytes = Vec::with_capacity(aux_count);
    for _ in 0..aux_count {
        aux_bytes.push(br.read_bits(8)? as u8);
    }

    let ocrc = if header.crc_present && header.dynamic_range {
        Some(br.read_bits(16)? as u16)
    } else {
        None
    };

    let consumed = br.absolute_bit_position() - bit_offset;
    Ok((
        OptionalInfo {
            time_code_stamp,
            aux_bytes,
            ocrc,
        },
        consumed,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{synth_header, BitWriter};

    /// 13-bit flag-window bit positions (MSB-first within the window):
    /// downmix, dynamic_range, time_stamp, aux_data, hdcd,
    /// ext_descr(3), ext_coding, aspf, lfe(2), predictor_history.
    const DYNF: u64 = 1 << 11;
    const TIMEF: u64 = 1 << 10;
    const AUXF: u64 = 1 << 9;

    #[test]
    fn all_flags_clear_consumes_nothing() {
        let header = synth_header(2, 0);
        let bytes = [0xFFu8; 8];
        let (info, consumed) = decode_optional_info_at(&bytes, 24, &header).unwrap();
        assert_eq!(info.time_code_stamp, None);
        assert_eq!(info.aux_bytes, Vec::<u8>::new());
        assert_eq!(info.ocrc, None);
        assert_eq!(consumed, 0);
    }

    #[test]
    fn times_and_aux_bytes_walk_with_byte_align() {
        let header = synth_header(2, TIMEF | AUXF);
        assert!(header.time_stamp);
        assert!(header.aux_data);
        // Region begins 3 bits into a byte: TIMES(32) + AUXCT(6)
        // leaves the cursor at bit 41 -> 7 align bits precede AUXD.
        let mut w = BitWriter::new();
        w.push_bits(0b101, 3); // pre-region audio bits
        w.push_bits(0xDEAD_BEEF, 32); // TIMES
        w.push_bits(2, 6); // AUXCT = 2
        w.align(8); // ByteAlign
        w.push_bits(0xAB, 8);
        w.push_bits(0xCD, 8);
        let bytes = w.into_bytes();
        let (info, consumed) = decode_optional_info_at(&bytes, 3, &header).unwrap();
        assert_eq!(info.time_code_stamp, Some(0xDEAD_BEEF));
        assert_eq!(info.aux_bytes, vec![0xAB, 0xCD]);
        assert_eq!(info.ocrc, None);
        // 32 + 6 + 7 (align from bit 41 to 48) + 16 = 61.
        assert_eq!(consumed, 61);
    }

    #[test]
    fn ocrc_requires_both_cpf_and_dynf() {
        // DYNF alone (CPF == 0 in the synthetic header): no OCRC.
        let header = synth_header(2, DYNF);
        assert!(header.dynamic_range);
        assert!(!header.crc_present);
        let bytes = [0x12u8, 0x34];
        let (info, consumed) = decode_optional_info_at(&bytes, 0, &header).unwrap();
        assert_eq!(info.ocrc, None);
        assert_eq!(consumed, 0);
    }

    #[test]
    fn ocrc_read_when_cpf_and_dynf_set() {
        let mut header = synth_header(2, DYNF);
        header.crc_present = true; // CPF (field-level override)
        let bytes = [0x12u8, 0x34];
        let (info, consumed) = decode_optional_info_at(&bytes, 0, &header).unwrap();
        assert_eq!(info.ocrc, Some(0x1234));
        assert_eq!(consumed, 16);
    }

    #[test]
    fn truncated_region_reports_eof() {
        let header = synth_header(2, TIMEF);
        let bytes = [0u8; 3]; // < 32 bits for TIMES
        assert_eq!(
            decode_optional_info_at(&bytes, 0, &header),
            Err(crate::Error::UnexpectedEof)
        );
    }
}

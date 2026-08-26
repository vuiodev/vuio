//! §5.7.1 Auxiliary Data chunk (Table 5-31): the optional
//! end-of-frame metadata block carrying a decode time stamp and the
//! dynamic (embedded) downmix coefficients.
//!
//! Transcribed from ETSI TS 102 114 V1.3.1 (2011-08) §5.6
//! (Table 5-30, the optional-information region after the audio-data
//! arrays) and §5.7.1 (Table 5-31 + field descriptions, PDF p.34-37),
//! staged at `docs/audio/dts/etsi-ts-102114-dts-coherent-acoustics.pdf`.
//!
//! ## Navigation
//!
//! Per §5.7.1: "Navigation to the start location of the auxiliary
//! data is achieved by either reading the AUXCT variable and
//! traversing to the next DWORD or by searching for the DWORD aligned
//! AUX sync word 0x9A1105A0 from the end of the audio frame. Since
//! the data in the auxiliary may be required prior to unpacking the
//! subframe data, the latter approach of searching for the AUX sync
//! word is the suggested method." [`find_aux_data`] implements the
//! suggested backward search over DWORD-aligned offsets ("aligned on
//! the 32-bit boundary from the beginning of the core stream", i.e.
//! `offset % 4 == 0` counting from the frame's first sync byte).
//!
//! ## Chunk layout (Table 5-31)
//!
//! ```text
//! nSYNCAUX          = ExtractBits(32);          // 0x9A1105A0
//! bAUXTimeStampFlag = ExtractBits(1);
//! if ( bAUXTimeStampFlag ) {
//!   Advance2Next4BitPos();
//!   nMSByte  = ExtractBits(8);
//!   nMarker  = ExtractBits(4);                  // == 0b1011
//!   nLSByte28 = ExtractBits(28);
//!   nMarker  = ExtractBits(4);                  // == 0b1011
//!   nAUXTimeStamp = (nMSByte << 28) | nLSByte28;
//! }
//! bAUXDynamCoeffFlag = ExtractBits(1);
//! if ( bAUXDynamCoeffFlag ) {
//!   nPrmChDownMixType = ExtractBits(3);         // Table 5-32
//!   nNumDwnMixCodeCoeffs = DeriveNumDwnMixCodeCoeffs();
//!   for (n = 0; n < nNumDwnMixCodeCoeffs; n++)
//!     panDwnMixCodeCoeffs[n] = ExtractBits(9);
//! }
//! ByteAlign = ExtractBits(0 ... 7);
//! nAUXCRC16 = ExtractBits(16);
//! ```
//!
//! `DeriveNumDwnMixCodeCoeffs()` multiplies the input channel count
//! `nPriCh` (`anNumCh[AMODE]`, plus one when `LFF > 0`) by the number
//! of resultant downmix channels the Table 5-32 type designates
//! (`m_nNumChPrevHierChSet`): `1/0 → 1`, `Lo/Ro`+`Lt/Rt → 2`,
//! `3/0`+`2/1 → 3`, `2/2`+`3/1 → 4`, `Unused → 0`. The coefficients
//! are packed as an N×M table walked output-channel-major
//! (`for nIndPrmCh ... for nIndXCh`), each a 9-bit code resolved
//! through [`crate::decode_dmix_code`] into the §D.11 `DmixTable`.
//!
//! The `nAUXCRC16` value "is calculated for the auxiliary data from
//! positions bAUXTimeStampFlag to the byte prior to the start of the
//! CRC inclusive" using the Annex B algorithm (CRC-CCITT, polynomial
//! `0x1021`, init `0xFFFF` — see [`crate::dts_crc16`] and
//! `docs/audio/dts/dts-crc16.md`). `bAUXTimeStampFlag` begins on a
//! byte boundary (right after the 32-bit sync word) and the
//! `ByteAlign` pad closes the region on one, so the coverage span is
//! the whole-byte window from `offset + 4` to the byte before the
//! CRC field. The parser recomputes it and reports the outcome in
//! [`AuxData::crc_valid`]; unlike the core `HCRC`-family fields, the
//! aux CRC is genuinely meant to be tested (it also disambiguates
//! false `nSYNCAUX` alias matches).

use crate::bitreader::BitReader;
use crate::crc16::dts_crc16;
use crate::header::DtsFrameHeader;
use crate::{decode_dmix_code, Error, Result};

/// The §5.7.1 auxiliary-data sync word (`nSYNCAUX`), DWORD-aligned
/// from the beginning of the core frame.
pub const AUX_SYNC_WORD: u32 = 0x9A11_05A0;

/// The 4-bit marker value (`0b1011`) bracketing the two halves of the
/// §5.7.1 36-bit decode time stamp.
pub const AUX_TIME_STAMP_MARKER: u8 = 0b1011;

/// §5.7.1 Table 5-32 "Downmix Channel Groups": the layout the primary
/// channel group folds down to (`nPrmChDownMixType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownmixType {
    /// `000` — mono (`1/0`).
    OneZero,
    /// `001` — stereo `Lo/Ro`.
    LoRo,
    /// `010` — stereo `Lt/Rt` (matrix surround encoded).
    LtRt,
    /// `011` — three front channels (`3/0`).
    ThreeZero,
    /// `100` — two front + one surround (`2/1`).
    TwoOne,
    /// `101` — two front + two surround (`2/2`).
    TwoTwo,
    /// `110` — three front + one surround (`3/1`).
    ThreeOne,
    /// `111` — marked "Unused" in Table 5-32; the Table 5-31
    /// pseudocode's `default:` arm derives zero coefficients for it.
    Unused,
}

impl DownmixType {
    /// Resolve the 3-bit `nPrmChDownMixType` field.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0b000 => DownmixType::OneZero,
            0b001 => DownmixType::LoRo,
            0b010 => DownmixType::LtRt,
            0b011 => DownmixType::ThreeZero,
            0b100 => DownmixType::TwoOne,
            0b101 => DownmixType::TwoTwo,
            0b110 => DownmixType::ThreeOne,
            0b111 => DownmixType::Unused,
            _ => return None,
        })
    }

    /// The on-wire 3-bit code.
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            DownmixType::OneZero => 0b000,
            DownmixType::LoRo => 0b001,
            DownmixType::LtRt => 0b010,
            DownmixType::ThreeZero => 0b011,
            DownmixType::TwoOne => 0b100,
            DownmixType::TwoTwo => 0b101,
            DownmixType::ThreeOne => 0b110,
            DownmixType::Unused => 0b111,
        }
    }

    /// Number of resultant downmix channels (the Table 5-31
    /// pseudocode's `m_nNumChPrevHierChSet`): `1`, `2`, `3`, or `4` —
    /// `0` for the `Unused` code (the pseudocode's `default:` arm).
    #[must_use]
    pub fn output_channel_count(self) -> usize {
        match self {
            DownmixType::OneZero => 1,
            DownmixType::LoRo | DownmixType::LtRt => 2,
            DownmixType::ThreeZero | DownmixType::TwoOne => 3,
            DownmixType::TwoTwo | DownmixType::ThreeOne => 4,
            DownmixType::Unused => 0,
        }
    }
}

/// The §5.7.1 dynamic (embedded) downmix coefficient set: the N×M
/// code table folding the frame's primary channels (plus LFE when
/// present) down to the [`DownmixType`] channel group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicDownmix {
    /// The Table 5-32 downmix channel group (`nPrmChDownMixType`).
    pub downmix_type: DownmixType,
    /// Input channel count `nPriCh`: `anNumCh[AMODE]` plus one when
    /// the frame carries an LFE channel (`LFF > 0`).
    pub input_channel_count: usize,
    /// The raw 9-bit `panDwnMixCodeCoeffs[]` code words,
    /// output-channel-major (`codes[out_ch * input_channel_count +
    /// in_ch]`), exactly `output_channel_count() *
    /// input_channel_count` long.
    pub codes: Vec<u16>,
}

impl DynamicDownmix {
    /// Number of resultant downmix channels (`M`).
    #[must_use]
    pub fn output_channel_count(&self) -> usize {
        self.downmix_type.output_channel_count()
    }

    /// The raw 9-bit code word folding input channel `in_ch` into
    /// output channel `out_ch`, or `None` when either index is out of
    /// range.
    #[must_use]
    pub fn code(&self, out_ch: usize, in_ch: usize) -> Option<u16> {
        if out_ch >= self.output_channel_count() || in_ch >= self.input_channel_count {
            return None;
        }
        self.codes
            .get(out_ch * self.input_channel_count + in_ch)
            .copied()
    }

    /// The real-valued coefficient folding input channel `in_ch` into
    /// output channel `out_ch`, resolved through
    /// [`crate::decode_dmix_code`] (§D.11 `DmixTable`, phase MSB,
    /// one-biased index, `0` → exact `0.0`).
    ///
    /// # Errors
    ///
    /// [`Error::DownmixCodeOutOfRange`] when the indices are out of
    /// range (surfaced with the sentinel code `u16::MAX`) or the
    /// stored code word does not resolve through the §D.11 table.
    pub fn coefficient(&self, out_ch: usize, in_ch: usize) -> Result<f64> {
        let code = self
            .code(out_ch, in_ch)
            .ok_or(Error::DownmixCodeOutOfRange { code: u16::MAX })?;
        decode_dmix_code(code)
    }

    /// The full M×N real-valued coefficient matrix,
    /// `matrix[out_ch][in_ch]`.
    ///
    /// # Errors
    ///
    /// [`Error::DownmixCodeOutOfRange`] when any stored code word does
    /// not resolve through the §D.11 table.
    pub fn coefficient_matrix(&self) -> Result<Vec<Vec<f64>>> {
        (0..self.output_channel_count())
            .map(|out_ch| {
                (0..self.input_channel_count)
                    .map(|in_ch| self.coefficient(out_ch, in_ch))
                    .collect()
            })
            .collect()
    }

    /// Fold planar PCM through the coefficient table:
    /// `output[m][t] = Σ_n coefficient(m, n) · input[n][t]`.
    ///
    /// `input` must carry exactly [`Self::input_channel_count`]
    /// equal-length planes (the §5.7.1 layout: the Table 5-4 primary
    /// channels in bitstream order, then the LFE plane when the frame
    /// carries one — matching the plane order the registry decoder
    /// emits). Each accumulated sample is truncated toward zero, the
    /// same `int()` cast convention as the §C.2.5 PCM output step,
    /// and saturated to the `i32` range.
    ///
    /// # Errors
    ///
    /// - [`Error::DownmixInputShapeMismatch`] when the plane count is
    ///   not `input_channel_count` or the planes have unequal
    ///   lengths.
    /// - [`Error::DownmixCodeOutOfRange`] when a stored code word
    ///   does not resolve through the §D.11 table.
    pub fn apply_planar(&self, input: &[Vec<i32>]) -> Result<Vec<Vec<i32>>> {
        if input.len() != self.input_channel_count {
            return Err(Error::DownmixInputShapeMismatch {
                expected: self.input_channel_count,
                found: input.len(),
            });
        }
        let samples = input.first().map_or(0, Vec::len);
        if let Some(plane) = input.iter().find(|p| p.len() != samples) {
            return Err(Error::DownmixInputShapeMismatch {
                expected: samples,
                found: plane.len(),
            });
        }
        let matrix = self.coefficient_matrix()?;
        let mut output = vec![vec![0i32; samples]; self.output_channel_count()];
        for (out_plane, row) in output.iter_mut().zip(&matrix) {
            for (t, out_sample) in out_plane.iter_mut().enumerate() {
                let mut acc = 0.0f64;
                for (coeff, in_plane) in row.iter().zip(input) {
                    acc += coeff * f64::from(in_plane[t]);
                }
                // Truncate toward zero (the §C.2.5 int() convention),
                // saturating at the i32 bounds.
                *out_sample = acc.trunc().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
            }
        }
        Ok(output)
    }
}

/// A parsed §5.7.1 Auxiliary Data chunk (Table 5-31).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuxData {
    /// Byte offset of the DWORD-aligned `nSYNCAUX` sync word within
    /// the frame.
    pub offset: usize,
    /// The 36-bit `nAUXTimeStamp` decode time stamp (present when
    /// `bAUXTimeStampFlag` is set): a running sample counter used to
    /// synchronize the core audio with another audio stream. The two
    /// 4-bit `0b1011` markers bracketing its halves are validated and
    /// stripped.
    pub time_stamp: Option<u64>,
    /// The dynamic downmix coefficient set (present when
    /// `bAUXDynamCoeffFlag` is set).
    pub dynamic_downmix: Option<DynamicDownmix>,
    /// The raw 16-bit `nAUXCRC16` word (Annex B CRC-CCITT, see
    /// [`crate::dts_crc16`]).
    pub crc16: u16,
    /// Whether [`Self::crc16`] matches the Annex B CRC-16 recomputed
    /// over the chunk's protected region — "the auxiliary data from
    /// positions bAUXTimeStampFlag to the byte prior to the start of
    /// the CRC inclusive" (§5.6). A `false` here means the chunk is
    /// corrupt or the DWORD-aligned sync match was a false alias;
    /// the parse result is surfaced either way so callers can decide.
    pub crc_valid: bool,
}

/// Search a core frame for the DWORD-aligned §5.7.1 auxiliary-data
/// sync word `0x9A1105A0`, returning its byte offset.
///
/// Implements the spec's suggested navigation: "searching for the
/// DWORD aligned AUX sync word 0x9A1105A0 from the end of the audio
/// frame" — the scan walks backward over 32-bit-aligned offsets
/// (alignment counted from the frame's first sync byte) and returns
/// the last aligned occurrence in the frame.
#[must_use]
pub fn find_aux_data(frame: &[u8]) -> Option<usize> {
    let sync = AUX_SYNC_WORD.to_be_bytes();
    if frame.len() < 4 {
        return None;
    }
    // Highest DWORD-aligned offset with room for the 4 sync bytes.
    let mut offset = (frame.len() - 4) & !3;
    loop {
        if frame[offset..offset + 4] == sync {
            return Some(offset);
        }
        if offset == 0 {
            return None;
        }
        offset -= 4;
    }
}

/// Parse the §5.7.1 Auxiliary Data chunk beginning at `byte_offset`
/// within `frame` (an offset produced by [`find_aux_data`]).
///
/// `header` supplies the input-channel derivation for
/// `DeriveNumDwnMixCodeCoeffs()`: `nPriCh = anNumCh[AMODE]` plus one
/// when `LFF > 0`.
///
/// # Errors
///
/// - [`Error::AuxSyncMismatch`] — the bytes at `byte_offset` are not
///   the `0x9A1105A0` sync word.
/// - [`Error::AuxTimeStampMarkerMismatch`] — a 4-bit time-stamp
///   marker was not `0b1011`.
/// - [`Error::AuxChannelCountUnresolved`] — the frame's `AMODE` is a
///   user-defined code whose channel count Table 5-4 does not define,
///   so the coefficient count cannot be derived.
/// - [`Error::UnexpectedEof`] — the chunk walked past the end of the
///   frame.
pub fn parse_aux_data_at(
    frame: &[u8],
    byte_offset: usize,
    header: &DtsFrameHeader,
) -> Result<AuxData> {
    let mut br = BitReader::from_byte_offset(frame, byte_offset);
    let sync = br.read_bits(32)?;
    if sync != AUX_SYNC_WORD {
        return Err(Error::AuxSyncMismatch { found: sync });
    }

    // bAUXTimeStampFlag + optional 36-bit time stamp.
    let time_stamp = if br.read_bit()? {
        // Advance2Next4BitPos: align the cursor to the next 4-bit
        // position (counted from the beginning of the core frame).
        let misalign = (br.absolute_bit_position() % 4) as u32;
        if misalign != 0 {
            br.skip_bits(4 - misalign)?;
        }
        let ms_byte = br.read_bits(8)? as u64;
        let marker = br.read_bits(4)? as u8;
        if marker != AUX_TIME_STAMP_MARKER {
            return Err(Error::AuxTimeStampMarkerMismatch { found: marker });
        }
        let ls_28 = br.read_bits(28)? as u64;
        let marker = br.read_bits(4)? as u8;
        if marker != AUX_TIME_STAMP_MARKER {
            return Err(Error::AuxTimeStampMarkerMismatch { found: marker });
        }
        Some((ms_byte << 28) | ls_28)
    } else {
        None
    };

    // bAUXDynamCoeffFlag + optional coefficient table.
    let dynamic_downmix = if br.read_bit()? {
        let type_code = br.read_bits(3)? as u8;
        let downmix_type =
            DownmixType::from_code(type_code).expect("3-bit field covers all Table 5-32 codes");
        // DeriveNumDwnMixCodeCoeffs(): nPriCh = anNumCh[AMODE] (+1
        // when LFF > 0).
        let base_channels = header
            .channel_count()
            .ok_or(Error::AuxChannelCountUnresolved {
                amode: header.amode,
            })?;
        let mut input_channel_count = usize::from(base_channels);
        if header.lfe.is_present() {
            input_channel_count += 1;
        }
        let n_codes = downmix_type.output_channel_count() * input_channel_count;
        let mut codes = Vec::with_capacity(n_codes);
        for _ in 0..n_codes {
            codes.push(br.read_bits(9)? as u16);
        }
        Some(DynamicDownmix {
            downmix_type,
            input_channel_count,
            codes,
        })
    } else {
        None
    };

    // ByteAlign (0..7 zero bits) then the 16-bit nAUXCRC16.
    let misalign = (br.absolute_bit_position() % 8) as u32;
    if misalign != 0 {
        br.skip_bits(8 - misalign)?;
    }
    // The CRC field starts here (byte-aligned); the protected region
    // is "from positions bAUXTimeStampFlag to the byte prior to the
    // start of the CRC inclusive" — bAUXTimeStampFlag begins right
    // after the 4 sync bytes, so the covered span is the whole-byte
    // window [offset + 4, crc_start).
    let crc_start = br.absolute_bit_position() / 8;
    let crc16 = br.read_bits(16)? as u16;
    let crc_valid = dts_crc16(&frame[byte_offset + 4..crc_start]) == crc16;

    Ok(AuxData {
        offset: byte_offset,
        time_stamp,
        dynamic_downmix,
        crc16,
        crc_valid,
    })
}

/// Locate ([`find_aux_data`]) and parse ([`parse_aux_data_at`]) the
/// §5.7.1 Auxiliary Data chunk of a core frame, returning `Ok(None)`
/// when the frame carries no DWORD-aligned aux sync word.
///
/// # Errors
///
/// Propagates the [`parse_aux_data_at`] errors when a sync word is
/// found but the chunk does not parse.
pub fn parse_aux_data(frame: &[u8], header: &DtsFrameHeader) -> Result<Option<AuxData>> {
    match find_aux_data(frame) {
        Some(offset) => parse_aux_data_at(frame, offset, header).map(Some),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{synth_header, BitWriter};

    /// AMODE 2 = stereo, 2 channels, no LFE.
    fn stereo_header() -> DtsFrameHeader {
        synth_header(2, 0)
    }

    /// Build a frame: 32 bytes of non-sync filler, then the aux chunk
    /// DWORD-aligned at offset 32.
    fn frame_with_chunk(chunk: &[u8]) -> Vec<u8> {
        let mut frame = vec![0x55u8; 32];
        frame.extend_from_slice(chunk);
        frame
    }

    fn empty_chunk() -> Vec<u8> {
        let mut w = BitWriter::new();
        w.push_bits(u64::from(AUX_SYNC_WORD), 32);
        w.push_bits(0, 1); // no time stamp
        w.push_bits(0, 1); // no dynamic coeffs
        w.align(8); // ByteAlign
        w.push_bits(0xBEEF, 16); // nAUXCRC16
        w.into_bytes()
    }

    /// Overwrite the trailing 16-bit `nAUXCRC16` of a test chunk with
    /// the correct Annex B value over the covered span (byte 4 through
    /// the byte before the CRC — the test builders always emit the CRC
    /// as the final two bytes).
    fn patch_crc(chunk: &mut [u8]) {
        let crc_start = chunk.len() - 2;
        let crc = dts_crc16(&chunk[4..crc_start]);
        chunk[crc_start..].copy_from_slice(&crc.to_be_bytes());
    }

    #[test]
    fn parses_empty_chunk() {
        let frame = frame_with_chunk(&empty_chunk());
        let header = stereo_header();
        let aux = parse_aux_data(&frame, &header).unwrap().unwrap();
        assert_eq!(aux.offset, 32);
        assert_eq!(aux.time_stamp, None);
        assert_eq!(aux.dynamic_downmix, None);
        assert_eq!(aux.crc16, 0xBEEF);
        // 0xBEEF is not the Annex B CRC of the covered span.
        assert!(!aux.crc_valid);
    }

    #[test]
    fn crc_valid_when_check_word_matches() {
        // The empty chunk's protected region is the single flag byte
        // between the sync word and the CRC.
        let mut chunk = empty_chunk();
        patch_crc(&mut chunk);
        let frame = frame_with_chunk(&chunk);
        let aux = parse_aux_data(&frame, &stereo_header()).unwrap().unwrap();
        assert_eq!(aux.crc16, dts_crc16(&chunk[4..chunk.len() - 2]));
        assert!(aux.crc_valid);
    }

    #[test]
    fn crc_valid_over_downmix_payload() {
        // A non-trivial protected region: flags + Table 5-32 type +
        // four 9-bit codes + ByteAlign, verified end to end.
        let codes: [u16; 4] = [0x100 | 241, 0x000, 216 + 1, 0x100 | 1];
        let mut w = BitWriter::new();
        w.push_bits(u64::from(AUX_SYNC_WORD), 32);
        w.push_bits(0, 1); // no time stamp
        w.push_bits(1, 1); // dynamic coeffs present
        w.push_bits(u64::from(DownmixType::LoRo.code()), 3);
        for &c in &codes {
            w.push_bits(u64::from(c), 9);
        }
        w.align(8);
        w.push_bits(0, 16); // placeholder CRC
        let mut chunk = w.into_bytes();
        patch_crc(&mut chunk);
        let frame = frame_with_chunk(&chunk);
        let aux = parse_aux_data(&frame, &stereo_header()).unwrap().unwrap();
        assert!(aux.crc_valid);

        // Any corruption inside the covered span must flip the verdict.
        let mut corrupt = frame.clone();
        corrupt[32 + 5] ^= 0x40; // inside the coefficient region
        let aux = parse_aux_data_at(&corrupt, 32, &stereo_header()).unwrap();
        assert!(!aux.crc_valid);
    }

    #[test]
    fn parses_time_stamp_with_markers_and_nibble_alignment() {
        // Chunk at offset 32: sync ends at bit 32*8+32 (multiple of
        // 4), the flag bit leaves the cursor 1 bit past a nibble, so
        // Advance2Next4BitPos skips 3 bits.
        let stamp: u64 = 0x8_1234_5678; // 36 bits
        let mut w = BitWriter::new();
        w.push_bits(u64::from(AUX_SYNC_WORD), 32);
        w.push_bits(1, 1); // time stamp present
        w.push_bits(0, 3); // Advance2Next4BitPos padding
        w.push_bits((stamp >> 28) & 0xFF, 8); // nMSByte
        w.push_bits(u64::from(AUX_TIME_STAMP_MARKER), 4);
        w.push_bits(stamp & 0x0FFF_FFFF, 28); // nLSByte28
        w.push_bits(u64::from(AUX_TIME_STAMP_MARKER), 4);
        w.push_bits(0, 1); // no dynamic coeffs
        w.align(8);
        w.push_bits(0x1234, 16);
        let frame = frame_with_chunk(&w.into_bytes());
        let aux = parse_aux_data(&frame, &stereo_header()).unwrap().unwrap();
        assert_eq!(aux.time_stamp, Some(stamp));
        assert_eq!(aux.crc16, 0x1234);
    }

    #[test]
    fn rejects_bad_time_stamp_marker() {
        let mut w = BitWriter::new();
        w.push_bits(u64::from(AUX_SYNC_WORD), 32);
        w.push_bits(1, 1);
        w.push_bits(0, 3);
        w.push_bits(0xAB, 8);
        w.push_bits(0b0110, 4); // wrong marker
        let frame = frame_with_chunk(&w.into_bytes());
        assert_eq!(
            parse_aux_data(&frame, &stereo_header()),
            Err(Error::AuxTimeStampMarkerMismatch { found: 0b0110 })
        );
    }

    #[test]
    fn parses_loro_downmix_for_stereo_input() {
        // Stereo (2 input channels, no LFE) folded Lo/Ro (2 output
        // channels) -> 4 nine-bit codes, output-channel-major.
        let codes: [u16; 4] = [
            0x100 | 241, // +unity   (Lo <- L)
            0x000,       // 0.0      (Lo <- R)
            216 + 1,     // -1/sqrt2 (Ro <- L), one-biased index 216
            0x100 | 1,   // +DmixTable[0] (Ro <- R)
        ];
        let mut w = BitWriter::new();
        w.push_bits(u64::from(AUX_SYNC_WORD), 32);
        w.push_bits(0, 1); // no time stamp
        w.push_bits(1, 1); // dynamic coeffs present
        w.push_bits(u64::from(DownmixType::LoRo.code()), 3);
        for &c in &codes {
            w.push_bits(u64::from(c), 9);
        }
        w.align(8);
        w.push_bits(0xCAFE, 16);
        let frame = frame_with_chunk(&w.into_bytes());
        let aux = parse_aux_data(&frame, &stereo_header()).unwrap().unwrap();
        let dmix = aux.dynamic_downmix.expect("downmix present");
        assert_eq!(dmix.downmix_type, DownmixType::LoRo);
        assert_eq!(dmix.input_channel_count, 2);
        assert_eq!(dmix.output_channel_count(), 2);
        assert_eq!(dmix.codes, codes);
        let m = dmix.coefficient_matrix().unwrap();
        assert_eq!(m[0][0], 1.0);
        assert_eq!(m[0][1], 0.0);
        assert!((m[1][0] - (-(23170.0 / 32768.0))).abs() < 1e-12);
        assert!((m[1][1] - 33.0 / 32768.0).abs() < 1e-12);
        assert_eq!(aux.crc16, 0xCAFE);
    }

    #[test]
    fn lfe_bearing_frame_adds_one_input_channel() {
        // DeriveNumDwnMixCodeCoeffs(): "if (LFF > 0) nPriCh++;" —
        // stereo + LFE folded 1/0 needs 3 codes, not 2. The 2-bit
        // `LFF` field sits one bit above `predictor_history` in the
        // 13-bit flag window.
        let header = synth_header(2, 0b10 << 1);
        assert!(header.lfe.is_present());
        let codes: [u16; 3] = [0x100 | 241, 0x100 | 241, 0x000];
        let mut w = BitWriter::new();
        w.push_bits(u64::from(AUX_SYNC_WORD), 32);
        w.push_bits(0, 1); // no time stamp
        w.push_bits(1, 1); // dynamic coeffs present
        w.push_bits(u64::from(DownmixType::OneZero.code()), 3);
        for &c in &codes {
            w.push_bits(u64::from(c), 9);
        }
        w.align(8);
        w.push_bits(0x5A5A, 16);
        let frame = frame_with_chunk(&w.into_bytes());
        let aux = parse_aux_data(&frame, &header).unwrap().unwrap();
        let dmix = aux.dynamic_downmix.expect("downmix present");
        assert_eq!(dmix.input_channel_count, 3);
        assert_eq!(dmix.output_channel_count(), 1);
        assert_eq!(dmix.codes, codes);
        assert_eq!(aux.crc16, 0x5A5A);
    }

    #[test]
    fn unused_downmix_type_carries_no_codes() {
        let mut w = BitWriter::new();
        w.push_bits(u64::from(AUX_SYNC_WORD), 32);
        w.push_bits(0, 1);
        w.push_bits(1, 1);
        w.push_bits(u64::from(DownmixType::Unused.code()), 3);
        w.align(8);
        w.push_bits(0x0001, 16);
        let frame = frame_with_chunk(&w.into_bytes());
        let aux = parse_aux_data(&frame, &stereo_header()).unwrap().unwrap();
        let dmix = aux.dynamic_downmix.expect("flag was set");
        assert_eq!(dmix.downmix_type, DownmixType::Unused);
        assert_eq!(dmix.output_channel_count(), 0);
        assert!(dmix.codes.is_empty());
        assert_eq!(dmix.coefficient_matrix().unwrap(), Vec::<Vec<f64>>::new());
    }

    #[test]
    fn find_ignores_unaligned_sync() {
        // Sync word at byte offset 33 (not DWORD-aligned) must not
        // match; the same word at offset 36 must.
        let mut frame = vec![0x55u8; 33];
        frame.extend_from_slice(&AUX_SYNC_WORD.to_be_bytes());
        assert_eq!(find_aux_data(&frame), None);

        let mut frame = vec![0x55u8; 36];
        frame.extend_from_slice(&AUX_SYNC_WORD.to_be_bytes());
        assert_eq!(find_aux_data(&frame), Some(36));
    }

    #[test]
    fn find_searches_backward_from_frame_end() {
        // Two aligned sync words: the backward search returns the
        // later one, per the spec's "from the end of the audio frame".
        let mut frame = Vec::new();
        frame.extend_from_slice(&AUX_SYNC_WORD.to_be_bytes());
        frame.extend_from_slice(&[0u8; 12]);
        frame.extend_from_slice(&AUX_SYNC_WORD.to_be_bytes());
        frame.extend_from_slice(&[0u8; 4]);
        assert_eq!(find_aux_data(&frame), Some(16));
    }

    #[test]
    fn parse_at_rejects_wrong_sync() {
        let frame = vec![0u8; 8];
        assert_eq!(
            parse_aux_data_at(&frame, 0, &stereo_header()),
            Err(Error::AuxSyncMismatch { found: 0 })
        );
    }

    #[test]
    fn truncated_chunk_reports_eof() {
        // Sync + flags but no room for the CRC.
        let mut w = BitWriter::new();
        w.push_bits(u64::from(AUX_SYNC_WORD), 32);
        w.push_bits(0, 1);
        w.push_bits(0, 1);
        let bytes = w.into_bytes();
        let frame = frame_with_chunk(&bytes[..bytes.len()]);
        assert_eq!(
            parse_aux_data(&frame, &stereo_header()),
            Err(Error::UnexpectedEof)
        );
    }

    #[test]
    fn dynamic_downmix_code_accessor_bounds() {
        let dmix = DynamicDownmix {
            downmix_type: DownmixType::OneZero,
            input_channel_count: 2,
            codes: vec![0x100 | 241, 0x100 | 241],
        };
        assert_eq!(dmix.code(0, 0), Some(0x100 | 241));
        assert_eq!(dmix.code(0, 2), None);
        assert_eq!(dmix.code(1, 0), None);
        assert!(dmix.coefficient(1, 0).is_err());
    }

    #[test]
    fn apply_planar_folds_and_truncates() {
        // Stereo -> mono with coefficients (+1.0, -1/sqrt(2)).
        let dmix = DynamicDownmix {
            downmix_type: DownmixType::OneZero,
            input_channel_count: 2,
            codes: vec![0x100 | 241, 216 + 1],
        };
        let input = vec![vec![1000, -1000, 0], vec![1000, 1000, 32768]];
        let out = dmix.apply_planar(&input).unwrap();
        assert_eq!(out.len(), 1);
        let c: f64 = 23170.0 / 32768.0;
        // 1000 - c*1000 = 292.9 -> 292 (truncate toward zero).
        assert_eq!(out[0][0], (1000.0 - c * 1000.0).trunc() as i32);
        // -1000 - c*1000 = -1707.09 -> -1707 (not -1708).
        assert_eq!(out[0][1], (-1000.0 - c * 1000.0).trunc() as i32);
        assert_eq!(out[0][2], (-c * 32768.0).trunc() as i32);
    }

    #[test]
    fn apply_planar_rejects_shape_mismatch() {
        let dmix = DynamicDownmix {
            downmix_type: DownmixType::OneZero,
            input_channel_count: 2,
            codes: vec![0x100 | 241, 0x100 | 241],
        };
        // Wrong plane count.
        assert_eq!(
            dmix.apply_planar(&[vec![0i32; 4]]),
            Err(Error::DownmixInputShapeMismatch {
                expected: 2,
                found: 1
            })
        );
        // Unequal plane lengths.
        assert_eq!(
            dmix.apply_planar(&[vec![0i32; 4], vec![0i32; 3]]),
            Err(Error::DownmixInputShapeMismatch {
                expected: 4,
                found: 3
            })
        );
    }

    #[test]
    fn downmix_type_round_trips_all_codes() {
        for code in 0..8u8 {
            let t = DownmixType::from_code(code).unwrap();
            assert_eq!(t.code(), code);
        }
        assert_eq!(DownmixType::from_code(8), None);
    }
}

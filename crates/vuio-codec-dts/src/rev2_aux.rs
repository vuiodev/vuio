//! §5.7.2 Rev2 Auxiliary Data Chunk (Table 5-33): the optional
//! DWORD-aligned broadcast-metadata block carrying the embedded-ES
//! downmix scale, per-subsubframe DRC values, and a dialog
//! normalization override.
//!
//! Transcribed from ETSI TS 102 114 V1.3.1 (2011-08) §5.7.2
//! (Table 5-33 + §5.7.2.2 field descriptions, PDF p.37-41), staged at
//! `docs/audio/dts/etsi-ts-102114-dts-coherent-acoustics.pdf`.
//!
//! ## Navigation
//!
//! Per §5.7.2: the chunk begins at the DWORD-aligned sync word
//! `nSYNCRev2AUX = 0x7004C070` ("aligned on the 32-bit boundary from
//! the beginning of the core stream"), located by "searching forward
//! starting after all auxiliary data bytes AUXD are extracted; or
//! searching backward starting from the end of the audio frame". The
//! chunk "may be encoded in the stream even when AUXF=FALSE".
//! [`find_rev2_aux`] implements the backward search.
//!
//! ## Chunk layout (Table 5-33)
//!
//! ```text
//! nSYNCRev2AUX = ExtractBits(32);              // 0x7004C070
//! nRev2AUXDataByteSize = ExtractBits(7) + 1;   // valid 3..=128
//! bESMetaDataFlag = ExtractBits(1);
//! if (bESMetaDataFlag)
//!   nEmbESDownMixScaleIndex = ExtractBits(8);  // valid 40..=240
//! if (nRev2AUXDataByteSize > 4)
//!   bBroadcastMetadataPresent = ExtractBits(1);
//! else
//!   bBroadcastMetadataPresent = FALSE;
//! if (bBroadcastMetadataPresent) {
//!   bDRCMetadataPresent  = ExtractBits(1);
//!   bDialnormMetadata    = ExtractBits(1);
//!   if (bDRCMetaDataPresent)
//!     DRCversion_Rev2AUX = ExtractBits(4);
//!   nByteAlign0 = ExtractBits(…);              // to byte boundary
//!   if (bDRCMetaDataPresent)                   // DRCversion == 1
//!     for (s = 0; s < nSSC; s++)
//!       subsubFrameDRC_Rev2AUX[s] = ExtractBits(8);
//!   if (bDialnormMetadata)
//!     DIALNORM_rev2aux = ExtractBits(5);       // DNG = -value dB
//! }
//! ReservedRev2Aux, ByteAlignforRev2AuxCRC;     // skipped via size
//! nRev2AUXCRC16 = ExtractBits(16);
//! ```
//!
//! `nRev2AUXDataByteSize` counts the bytes "from the
//! nRev2AUXDataByteSize to nRev2AUXCRC16 inclusive", so the 16-bit
//! CRC sits at byte offset `nRev2AUXDataByteSize - 2` from the size
//! field — the parser reads it there (the spec's own "jump forward"
//! locator), which also skips the reserved field of "unspecified
//! duration". The check word uses the Annex B algorithm (CRC-CCITT,
//! polynomial `0x1021`, init `0xFFFF` — [`crate::dts_crc16`],
//! `docs/audio/dts/dts-crc16.md`) over the `nRev2AUXDataByteSize - 2`
//! bytes starting at the size field; the parser recomputes it and
//! reports the outcome in [`Rev2AuxChunk::crc_valid`].
//!
//! One DRC value is transmitted per subsubframe of the frame
//! (Table 5-34: 512-sample frame → 2, 1024 → 4, 2048 → 8, i.e.
//! `32·(NBLKS+1)/256` — the frame's sample count divided by the
//! 256-sample subsubframe). Only `DRCversion_Rev2AUX == 1`
//! (single-band, one 8-bit value per subsubframe) is defined; for any
//! other version the spec instructs decoders to ignore the DRC
//! payload, whose layout is undefined — the parser then skips to the
//! size-located CRC and reports the DRC codes (and any dialnorm
//! field behind them) as unavailable.
//!
//! The spec converts each DRC byte "into a dB gain by function
//! dts_dynrng_to_db()", recovered as a closed form in
//! `docs/audio/dts/dts-drc-dynrng.md`: the byte is 8-bit signed Q2
//! two's-complement, `dB = (int8)code × 0.25`
//! ([`crate::dts_dynrng_to_db`]). §5.7.2.2 states these values
//! "should be used instead of any dynamic range control coefficients
//! found in the legacy core stream (indicated by flag DYNF)";
//! [`Rev2Drc::gains_db`] / [`Rev2Drc::multipliers`] resolve the codes
//! through that function, and the raw codes stay available.

use crate::bitreader::BitReader;
use crate::header::DtsFrameHeader;
use crate::{Error, Result, dmix_scale, dts_dynrng_to_db, dts_dynrng_to_linear};

/// The §5.7.2 Rev2 auxiliary-data sync word (`nSYNCRev2AUX`),
/// DWORD-aligned from the beginning of the core frame.
pub const REV2_AUX_SYNC_WORD: u32 = 0x7004_C070;

/// The §5.7.2 DRC version this parser can walk
/// (`DRCversion_Rev2AUX`): "Currently only DRCversion_Rev2AUX = 1 is
/// supported in the Rev2Aux chunk" (single-band, one 8-bit value per
/// subsubframe).
pub const REV2_DRC_VERSION_SINGLE_BAND: u8 = 1;

/// The §5.7.2 Rev2AUX per-subsubframe dynamic-range-control payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rev2Drc {
    /// The 4-bit `DRCversion_Rev2AUX` field ("the first version
    /// starts at 0x1").
    pub version: u8,
    /// One 8-bit `subsubFrameDRC_Rev2AUX[]` code per subsubframe of
    /// the frame. Empty when `version` is not
    /// [`REV2_DRC_VERSION_SINGLE_BAND`] (the payload layout is
    /// undefined and the spec instructs decoders to ignore it).
    pub codes: Vec<u8>,
}

impl Rev2Drc {
    /// The per-subsubframe DRC gains in dB, resolving each 8-bit code
    /// through the §5.7.2 `dts_dynrng_to_db()` function
    /// ([`crate::dts_dynrng_to_db`]: the code is 8-bit signed Q2
    /// two's-complement, `dB = (int8)code × 0.25`, per
    /// `docs/audio/dts/dts-drc-dynrng.md`).
    #[must_use]
    pub fn gains_db(&self) -> Vec<f64> {
        self.codes.iter().map(|&c| dts_dynrng_to_db(c)).collect()
    }

    /// The per-subsubframe linear DRC multipliers
    /// (`10^(gains_db/20)`, [`crate::dts_dynrng_to_linear`]) — the
    /// gain applied to the decoded samples of each subsubframe.
    /// §5.7.2.2: "the DRC values in the Rev2AUX data chunk should be
    /// used instead of any dynamic range control coefficients found
    /// in the legacy core stream" (the `DYNF`/`RANGE` path, which
    /// shares the same signed-Q2 code space). Callers needing the raw
    /// bytes can read [`Self::codes`] directly.
    #[must_use]
    pub fn multipliers(&self) -> Vec<f64> {
        self.codes
            .iter()
            .map(|&c| dts_dynrng_to_linear(c))
            .collect()
    }
}

/// A parsed §5.7.2 Rev2 Auxiliary Data Chunk (Table 5-33).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rev2AuxChunk {
    /// Byte offset of the DWORD-aligned `nSYNCRev2AUX` sync word
    /// within the frame.
    pub offset: usize,
    /// `nRev2AUXDataByteSize`: the chunk size in bytes from the size
    /// field to the CRC inclusive (valid `3..=128`).
    pub data_byte_size: u8,
    /// `nEmbESDownMixScaleIndex` (present when `bESMetaDataFlag` is
    /// set): the 8-bit §D.11 `DmixTable[]` index (valid `40..=240`,
    /// i.e. `[-40 dB, 0 dB]`) of the attenuation applied to the core
    /// channels during embedded-ES down-mixing.
    pub es_downmix_scale_index: Option<u8>,
    /// The broadcast-metadata DRC payload (present when the
    /// broadcast and DRC flags are set).
    pub drc: Option<Rev2Drc>,
    /// `DIALNORM_rev2aux` (present when the broadcast and dialnorm
    /// flags are set, and the DRC payload in front of it was
    /// walkable): the 5-bit dialog-normalization parameter that
    /// takes priority over the legacy `DIALNORM` header field.
    pub dialnorm: Option<u8>,
    /// The raw 16-bit `nRev2AUXCRC16` word, read at byte offset
    /// `data_byte_size - 2` from the size field (Annex B CRC-CCITT,
    /// see [`crate::dts_crc16`]).
    pub crc16: u16,
    /// Whether [`Self::crc16`] matches the Annex B CRC-16 recomputed
    /// over the chunk's protected region — the
    /// `nRev2AUXDataByteSize - 2` bytes starting at the size field
    /// (i.e. everything after the sync word up to the byte before the
    /// CRC). A `false` here means the chunk is corrupt or the
    /// DWORD-aligned sync match was a false alias; the parse result
    /// is surfaced either way so callers can decide.
    pub crc_valid: bool,
}

impl Rev2AuxChunk {
    /// The embedded-ES downmix scale factor `ESDmixScale` as a real
    /// value, resolved through the §D.11 `DmixTable`.
    #[must_use]
    pub fn es_downmix_scale(&self) -> Option<f64> {
        self.es_downmix_scale_index
            .and_then(|i| dmix_scale(usize::from(i)))
    }

    /// The Table 5-36 dialog normalization gain in dB applied to the
    /// decoder outputs: `DNG = -(DIALNORM_rev2aux)`, i.e. `0` to
    /// `-31` dB.
    #[must_use]
    pub fn dialog_normalization_gain_db(&self) -> Option<i8> {
        self.dialnorm.map(|v| -(v as i8))
    }
}

/// Search a core frame for the DWORD-aligned §5.7.2 Rev2 auxiliary
/// sync word `0x7004C070`, returning its byte offset.
///
/// Implements the spec's "searching backward starting from the end of
/// the audio frame" over 32-bit-aligned offsets (alignment counted
/// from the frame's first sync byte).
#[must_use]
pub fn find_rev2_aux(frame: &[u8]) -> Option<usize> {
    let sync = REV2_AUX_SYNC_WORD.to_be_bytes();
    if frame.len() < 4 {
        return None;
    }
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

/// Number of subsubframes in the frame (`32·(NBLKS+1)/256` — one DRC
/// byte is transmitted per subsubframe, Table 5-34).
fn subsubframes_per_frame(header: &DtsFrameHeader) -> Result<usize> {
    // `blocks_per_frame` carries the raw `NBLKS` field; the block
    // count is `NBLKS + 1` and each block is 32 PCM samples.
    let blocks = usize::from(header.blocks_per_frame) + 1;
    if blocks % 8 != 0 {
        return Err(Error::Rev2AuxDrcCountUnresolved {
            blocks: blocks as u8,
        });
    }
    Ok(blocks / 8)
}

/// Parse the §5.7.2 Rev2 Auxiliary Data Chunk beginning at
/// `byte_offset` within `frame` (an offset produced by
/// [`find_rev2_aux`]).
///
/// `header` supplies the frame's subsubframe count (one DRC value per
/// subsubframe).
///
/// # Errors
///
/// - [`Error::Rev2AuxSyncMismatch`] — the bytes at `byte_offset` are
///   not the `0x7004C070` sync word.
/// - [`Error::Rev2AuxSizeOutOfRange`] — `nRev2AUXDataByteSize` is
///   outside `3..=128`, or the declared fields overran the declared
///   chunk size.
/// - [`Error::Rev2AuxEsScaleIndexOutOfRange`] —
///   `nEmbESDownMixScaleIndex` is outside the valid `40..=240`.
/// - [`Error::Rev2AuxDrcCountUnresolved`] — the frame's block count
///   is not a whole number of 256-sample subsubframes.
/// - [`Error::UnexpectedEof`] — the chunk walked past the end of the
///   frame.
pub fn parse_rev2_aux_at(
    frame: &[u8],
    byte_offset: usize,
    header: &DtsFrameHeader,
) -> Result<Rev2AuxChunk> {
    let mut br = BitReader::from_byte_offset(frame, byte_offset);
    let sync = br.read_bits(32)?;
    if sync != REV2_AUX_SYNC_WORD {
        return Err(Error::Rev2AuxSyncMismatch { found: sync });
    }

    let data_byte_size = (br.read_bits(7)? + 1) as u8;
    if !(3..=128).contains(&data_byte_size) {
        return Err(Error::Rev2AuxSizeOutOfRange {
            size: data_byte_size,
        });
    }
    // The size counts bytes from the size field to the CRC inclusive;
    // the CRC therefore sits `data_byte_size - 2` bytes past the size
    // field (which begins right after the 4 sync bytes).
    let crc_byte_offset = byte_offset + 4 + usize::from(data_byte_size) - 2;
    if crc_byte_offset + 2 > frame.len() {
        return Err(Error::UnexpectedEof);
    }

    let es_downmix_scale_index = if br.read_bit()? {
        let index = br.read_bits(8)? as u8;
        if !(40..=240).contains(&index) {
            return Err(Error::Rev2AuxEsScaleIndexOutOfRange { index });
        }
        Some(index)
    } else {
        None
    };

    // "the bBroadcastMetaDataPresent flag is present in the stream if
    // and only if the nRev2AUXDataByteSize > 4".
    let broadcast = data_byte_size > 4 && br.read_bit()?;

    let mut drc = None;
    let mut dialnorm = None;
    if broadcast {
        let drc_present = br.read_bit()?;
        let dialnorm_present = br.read_bit()?;
        let version = if drc_present {
            br.read_bits(4)? as u8
        } else {
            0
        };
        // nByteAlign0: zero-pad to the next byte boundary (1 bit when
        // the 4-bit version field was present, 5 bits when not).
        let misalign = (br.absolute_bit_position() % 8) as u32;
        if misalign != 0 {
            br.skip_bits(8 - misalign)?;
        }
        let mut walkable = true;
        if drc_present {
            let codes = if version == REV2_DRC_VERSION_SINGLE_BAND {
                let n = subsubframes_per_frame(header)?;
                let mut codes = Vec::with_capacity(n);
                for _ in 0..n {
                    codes.push(br.read_bits(8)? as u8);
                }
                codes
            } else {
                // Undefined payload layout: the spec instructs
                // decoders to ignore the supplied DRC values; any
                // dialnorm field behind them cannot be located.
                walkable = false;
                Vec::new()
            };
            drc = Some(Rev2Drc { version, codes });
        }
        if dialnorm_present && walkable {
            dialnorm = Some(br.read_bits(5)? as u8);
        }
    }

    // The sequential walk must not have overrun the size-located CRC.
    if br.absolute_bit_position() > crc_byte_offset * 8 {
        return Err(Error::Rev2AuxSizeOutOfRange {
            size: data_byte_size,
        });
    }

    // ReservedRev2Aux ("unspecified duration") + ByteAlignforRev2AuxCRC
    // are skipped by reading the CRC at its size-located offset.
    let mut crc_reader = BitReader::from_byte_offset(frame, crc_byte_offset);
    let crc16 = crc_reader.read_bits(16)? as u16;
    // The size counts the protected span + CRC: the Annex B CRC-16
    // covers the `data_byte_size - 2` bytes starting at the size
    // field (right after the 4 sync bytes) up to the CRC exclusive.
    let crc_valid = crate::dts_crc16(&frame[byte_offset + 4..crc_byte_offset]) == crc16;

    Ok(Rev2AuxChunk {
        offset: byte_offset,
        data_byte_size,
        es_downmix_scale_index,
        drc,
        dialnorm,
        crc16,
        crc_valid,
    })
}

/// Locate ([`find_rev2_aux`]) and parse ([`parse_rev2_aux_at`]) the
/// §5.7.2 Rev2 Auxiliary Data Chunk of a core frame, returning
/// `Ok(None)` when the frame carries no DWORD-aligned Rev2 sync word.
///
/// # Errors
///
/// Propagates the [`parse_rev2_aux_at`] errors when a sync word is
/// found but the chunk does not parse.
pub fn parse_rev2_aux(frame: &[u8], header: &DtsFrameHeader) -> Result<Option<Rev2AuxChunk>> {
    match find_rev2_aux(frame) {
        Some(offset) => parse_rev2_aux_at(frame, offset, header).map(Some),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DMIX_TABLE_UNITY_INDEX;
    use crate::test_util::{BitWriter, synth_header, synth_header_with_blocks};

    /// Build a frame: 32 bytes of non-sync filler, then the chunk
    /// DWORD-aligned at offset 32, padded out to the declared size.
    fn frame_with_chunk(chunk: &[u8]) -> Vec<u8> {
        let mut frame = vec![0x55u8; 32];
        frame.extend_from_slice(chunk);
        frame
    }

    /// Assemble a chunk: sync + size + `body` bits, zero-padded so the
    /// CRC lands exactly `size - 2` bytes after the size field, then
    /// the CRC.
    fn chunk(size: u8, body: impl FnOnce(&mut BitWriter), crc: u16) -> Vec<u8> {
        let mut w = BitWriter::new();
        w.push_bits(u64::from(REV2_AUX_SYNC_WORD), 32);
        w.push_bits(u64::from(size) - 1, 7);
        body(&mut w);
        let crc_bit = (4 + usize::from(size) - 2) * 8;
        assert!(w.bit_len() <= crc_bit, "test chunk body overruns its size");
        while w.bit_len() < crc_bit {
            w.push_bits(0, 1);
        }
        w.push_bits(u64::from(crc), 16);
        w.into_bytes()
    }

    /// Overwrite a test chunk's size-located 16-bit `nRev2AUXCRC16`
    /// with the correct Annex B value over the covered span (the
    /// `size - 2` bytes starting at the size field, byte 4 of the
    /// chunk).
    fn patch_crc(chunk: &mut [u8], size: u8) {
        let crc_start = 4 + usize::from(size) - 2;
        let crc = crate::dts_crc16(&chunk[4..crc_start]);
        chunk[crc_start..crc_start + 2].copy_from_slice(&crc.to_be_bytes());
    }

    #[test]
    fn parses_minimal_chunk() {
        // size 3: size/ES-flag byte + 16-bit CRC, no broadcast flag.
        let bytes = chunk(3, |w| w.push_bits(0, 1), 0xABCD);
        let frame = frame_with_chunk(&bytes);
        let header = synth_header(2, 0);
        let chunk = parse_rev2_aux(&frame, &header).unwrap().unwrap();
        assert_eq!(chunk.offset, 32);
        assert_eq!(chunk.data_byte_size, 3);
        assert_eq!(chunk.es_downmix_scale_index, None);
        assert_eq!(chunk.drc, None);
        assert_eq!(chunk.dialnorm, None);
        assert_eq!(chunk.crc16, 0xABCD);
        // 0xABCD is not the Annex B CRC of the covered byte.
        assert!(!chunk.crc_valid);
    }

    #[test]
    fn crc_valid_when_check_word_matches() {
        // A broadcast-metadata chunk with the genuine Annex B check
        // word over the size-located span verifies; corrupting any
        // covered byte flips the verdict.
        let header = synth_header(2, 0);
        let mut bytes = chunk(
            8,
            |w| {
                w.push_bits(0, 1); // no ES metadata
                w.push_bits(1, 1); // broadcast
                w.push_bits(1, 1); // DRC
                w.push_bits(0, 1); // no dialnorm
                w.push_bits(u64::from(REV2_DRC_VERSION_SINGLE_BAND), 4);
                w.align(8);
                w.push_bits(0, 8); // DRC code, subsubframe 0 (unity)
                w.push_bits(80, 8); // DRC code, subsubframe 1
            },
            0,
        );
        patch_crc(&mut bytes, 8);
        let frame = frame_with_chunk(&bytes);
        let parsed = parse_rev2_aux(&frame, &header).unwrap().unwrap();
        assert!(parsed.crc_valid);
        assert_eq!(parsed.crc16, crate::dts_crc16(&bytes[4..4 + 8 - 2]));

        let mut corrupt = frame.clone();
        corrupt[32 + 6] ^= 0x01; // inside the covered span
        let parsed = parse_rev2_aux_at(&corrupt, 32, &header).unwrap();
        assert!(!parsed.crc_valid);
    }

    #[test]
    fn parses_es_downmix_scale() {
        let bytes = chunk(
            4,
            |w| {
                w.push_bits(1, 1); // bESMetaDataFlag
                w.push_bits(DMIX_TABLE_UNITY_INDEX as u64, 8);
            },
            0x0102,
        );
        let frame = frame_with_chunk(&bytes);
        let chunk = parse_rev2_aux(&frame, &synth_header(2, 0))
            .unwrap()
            .unwrap();
        assert_eq!(chunk.es_downmix_scale_index, Some(240));
        assert_eq!(chunk.es_downmix_scale(), Some(1.0));
    }

    #[test]
    fn rejects_es_scale_index_out_of_range() {
        // Valid range is 40..=240 ([-40 dB, 0 dB]).
        let bytes = chunk(
            4,
            |w| {
                w.push_bits(1, 1);
                w.push_bits(39, 8);
            },
            0,
        );
        let frame = frame_with_chunk(&bytes);
        assert_eq!(
            parse_rev2_aux(&frame, &synth_header(2, 0)),
            Err(Error::Rev2AuxEsScaleIndexOutOfRange { index: 39 })
        );
    }

    #[test]
    fn parses_broadcast_drc_and_dialnorm() {
        // 16-block frame -> 512 samples -> 2 subsubframes -> 2 DRC
        // bytes (Table 5-34 row 1).
        let header = synth_header(2, 0);
        // Signed-Q2 wire codes: 0 -> 0 dB (unity), 1 -> +0.25 dB.
        let drc_codes = [0u8, 1u8];
        let bytes = chunk(
            8,
            |w| {
                w.push_bits(0, 1); // no ES metadata
                w.push_bits(1, 1); // bBroadcastMetadataPresent
                w.push_bits(1, 1); // bDRCMetadataPresent
                w.push_bits(1, 1); // bDialnormMetadata
                w.push_bits(u64::from(REV2_DRC_VERSION_SINGLE_BAND), 4);
                w.align(8); // nByteAlign0 (1 bit here)
                for &c in &drc_codes {
                    w.push_bits(u64::from(c), 8);
                }
                w.push_bits(21, 5); // DIALNORM_rev2aux -> -21 dB
            },
            0xFEED,
        );
        let frame = frame_with_chunk(&bytes);
        let chunk = parse_rev2_aux(&frame, &header).unwrap().unwrap();
        let drc = chunk.drc.as_ref().expect("DRC present");
        assert_eq!(drc.version, 1);
        assert_eq!(drc.codes, drc_codes);
        assert_eq!(drc.gains_db(), vec![0.0, 0.25]);
        let mult = drc.multipliers();
        assert_eq!(mult[0], 1.0); // signed-Q2 code 0 = unity
        assert!((mult[1] - 10f64.powf(0.25 / 20.0)).abs() < 1e-12); // +0.25 dB
        assert_eq!(chunk.dialnorm, Some(21));
        assert_eq!(chunk.dialog_normalization_gain_db(), Some(-21));
        assert_eq!(chunk.crc16, 0xFEED);
    }

    #[test]
    fn dialnorm_without_drc_follows_five_bit_alignment() {
        // No DRC: nByteAlign0 is 5 bits (no 4-bit version field), the
        // dialnorm field follows byte-aligned.
        let bytes = chunk(
            6,
            |w| {
                w.push_bits(0, 1); // no ES metadata
                w.push_bits(1, 1); // broadcast
                w.push_bits(0, 1); // no DRC
                w.push_bits(1, 1); // dialnorm
                w.align(8); // 5 zero bits
                w.push_bits(31, 5);
            },
            0x0F0F,
        );
        let frame = frame_with_chunk(&bytes);
        let chunk = parse_rev2_aux(&frame, &synth_header(2, 0))
            .unwrap()
            .unwrap();
        assert_eq!(chunk.drc, None);
        assert_eq!(chunk.dialnorm, Some(31));
        assert_eq!(chunk.dialog_normalization_gain_db(), Some(-31));
    }

    #[test]
    fn unsupported_drc_version_skips_payload() {
        // Version 2 payload layout is undefined: codes stay empty,
        // the dialnorm behind it is unavailable, but the size-located
        // CRC still reads.
        let bytes = chunk(
            8,
            |w| {
                w.push_bits(0, 1);
                w.push_bits(1, 1); // broadcast
                w.push_bits(1, 1); // DRC
                w.push_bits(1, 1); // dialnorm (unreachable)
                w.push_bits(2, 4); // version 2
            },
            0xD00D,
        );
        let frame = frame_with_chunk(&bytes);
        let chunk = parse_rev2_aux(&frame, &synth_header(2, 0))
            .unwrap()
            .unwrap();
        assert_eq!(
            chunk.drc,
            Some(Rev2Drc {
                version: 2,
                codes: vec![]
            })
        );
        assert_eq!(chunk.dialnorm, None);
        assert_eq!(chunk.crc16, 0xD00D);
    }

    #[test]
    fn size_4_chunk_never_reads_broadcast_flag() {
        // "the bBroadcastMetaDataPresent flag is present in the
        // stream if and only if the nRev2AUXDataByteSize > 4": a
        // size-4 chunk with a set bit where the flag would sit still
        // parses with no broadcast metadata.
        let bytes = chunk(
            4,
            |w| {
                w.push_bits(0, 1); // no ES metadata
                w.push_bits(1, 1); // would-be broadcast flag: ignored
            },
            0x7777,
        );
        let frame = frame_with_chunk(&bytes);
        let chunk = parse_rev2_aux(&frame, &synth_header(2, 0))
            .unwrap()
            .unwrap();
        assert_eq!(chunk.drc, None);
        assert_eq!(chunk.dialnorm, None);
    }

    #[test]
    fn rejects_size_below_three() {
        let mut w = BitWriter::new();
        w.push_bits(u64::from(REV2_AUX_SYNC_WORD), 32);
        w.push_bits(1, 7); // size = 2
        w.push_bits(0, 1);
        w.push_bits(0, 16);
        let frame = frame_with_chunk(&w.into_bytes());
        assert_eq!(
            parse_rev2_aux(&frame, &synth_header(2, 0)),
            Err(Error::Rev2AuxSizeOutOfRange { size: 2 })
        );
    }

    #[test]
    fn rejects_fields_overrunning_declared_size() {
        // A size-5 chunk cannot hold broadcast DRC for 2 subsubframes:
        // the sequential walk lands past the size-located CRC.
        let bytes = chunk(
            5,
            |w| {
                w.push_bits(0, 1);
                w.push_bits(1, 1); // broadcast
                w.push_bits(1, 1); // DRC
                w.push_bits(0, 1);
                w.push_bits(1, 4); // version 1
                w.align(8);
                // Body writer stops here; the parser's own walk wants
                // 2 DRC bytes and overruns the CRC slot.
            },
            0,
        );
        let frame = frame_with_chunk(&bytes);
        assert_eq!(
            parse_rev2_aux(&frame, &synth_header(2, 0)),
            Err(Error::Rev2AuxSizeOutOfRange { size: 5 })
        );
    }

    #[test]
    fn drc_count_needs_whole_subsubframes() {
        // 12 blocks = 384 samples = 1.5 subsubframes: unresolvable.
        let header = synth_header_with_blocks(2, 0, 11);
        assert_eq!(header.blocks_per_frame, 11); // raw NBLKS -> 12 blocks
        let bytes = chunk(
            8,
            |w| {
                w.push_bits(0, 1);
                w.push_bits(1, 1);
                w.push_bits(1, 1);
                w.push_bits(0, 1);
                w.push_bits(1, 4);
            },
            0,
        );
        let frame = frame_with_chunk(&bytes);
        assert_eq!(
            parse_rev2_aux(&frame, &header),
            Err(Error::Rev2AuxDrcCountUnresolved { blocks: 12 })
        );
    }

    #[test]
    fn find_ignores_unaligned_sync() {
        let mut frame = vec![0x55u8; 34];
        frame.extend_from_slice(&REV2_AUX_SYNC_WORD.to_be_bytes());
        assert_eq!(find_rev2_aux(&frame), None);

        let mut frame = vec![0x55u8; 40];
        frame.extend_from_slice(&REV2_AUX_SYNC_WORD.to_be_bytes());
        assert_eq!(find_rev2_aux(&frame), Some(40));
    }

    #[test]
    fn parse_at_rejects_wrong_sync() {
        let frame = vec![0u8; 8];
        assert_eq!(
            parse_rev2_aux_at(&frame, 0, &synth_header(2, 0)),
            Err(Error::Rev2AuxSyncMismatch { found: 0 })
        );
    }

    #[test]
    fn truncated_chunk_reports_eof() {
        // Declared size 128 but the frame ends long before the CRC.
        let mut w = BitWriter::new();
        w.push_bits(u64::from(REV2_AUX_SYNC_WORD), 32);
        w.push_bits(127, 7); // size = 128
        w.push_bits(0, 1);
        let frame = frame_with_chunk(&w.into_bytes());
        assert_eq!(
            parse_rev2_aux(&frame, &synth_header(2, 0)),
            Err(Error::UnexpectedEof)
        );
    }
}

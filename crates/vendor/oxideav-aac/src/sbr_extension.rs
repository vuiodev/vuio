//! `sbr_extension_data()` top-level walker — ISO/IEC 14496-3 §4.4.2.8
//! Table 4.62.
//!
//! This is the glue between [`crate::extension_payload`] and the SBR
//! side-info parsers: it consumes a whole SBR extension payload from a
//! `fill_element()`'s `extension_payload()` body, in the exact spec
//! order:
//!
//! ```text
//! sbr_extension_data(id_aac, crc_flag) {
//!     num_sbr_bits = 0;
//!     if (crc_flag) { bs_sbr_crc_bits;            10  uimsbf  num_sbr_bits += 10; }
//!     // sbr_layer != SBR_STEREO_ENHANCE for a non-scalable core:
//!     bs_header_flag;                              1   uimsbf  num_sbr_bits += 1;
//!     if (bs_header_flag)  num_sbr_bits += sbr_header();
//!     num_sbr_bits += sbr_data(id_aac, bs_amp_res);
//!     num_align_bits = (8*cnt - 4 - num_sbr_bits) % 8;
//!     bs_fill_bits;                                num_align_bits  uimsbf
//! }
//! ```
//!
//! `sbr_data(id_aac, bs_amp_res)` dispatches on the AAC element type the
//! SBR payload extends: an `ID_SCE` core element pairs with
//! `sbr_single_channel_element()` ([`SbrElement::parse_single`]), an
//! `ID_CPE` core element with `sbr_channel_pair_element()`
//! ([`SbrElement::parse_pair`]). The band tables both need are derived
//! from the active [`SbrHeader`] at the SBR *internal* sample rate
//! `fs_sbr` (twice the AAC core rate) via [`SbrHeader::derive_bands`].
//!
//! ## Header reuse
//!
//! When `bs_header_flag == 0` the payload reuses the most recent
//! transmitted `sbr_header()`. The first SBR payload of a stream must
//! carry a header (`bs_header_flag == 1`); a clear flag with no prior
//! header is an ill-formed stream ([`Error::SbrFreqBandInvalid`]). The
//! caller threads the returned [`SbrExtensionData::header`] back in as
//! `prev_header` on the next payload so the reuse chain is continuous.
//!
//! ## Scope
//!
//! This decodes the SBR *bitstream* side info end to end (CRC field +
//! header + grid / dtdf / invf / envelope / noise / add-harmonic +
//! extended-data block). The SBR back-end DSP (dequantization to linear
//! energies, the QMF analysis / synthesis filterbanks, HF generation /
//! patching, the limiter, and the envelope adjustment that produces
//! up-sampled PCM) is **not** part of this walker — it keys off the
//! band tables and scalefactors this produces. The `bs_sbr_crc_bits`
//! value is captured along with its §4.4.2.8.1 coverage region (the
//! `num_sbr_bits − 10` payload bits after the CRC field); callers that
//! own the payload buffer verify it via
//! [`SbrExtensionData::verify_crc`] (the decode drivers do).
//!
//! ## Clean-room provenance
//!
//! The Table 4.62 syntax, the `num_align_bits = (8·cnt − 4 −
//! num_sbr_bits) % 8` fill computation, and the `sbr_data` dispatch on
//! `id_aac` are transcribed from ISO/IEC 14496-3:2009 §4.4.2.8 staged
//! under `docs/audio/aac/`. The non-scalable core fixes the helper
//! `sbr_layer` to `SBR_NOT_SCALABLE` (Table 4.62 Note 1), so the
//! `bs_header_flag` is always present.

use oxideav_core::bits::BitReader;

use crate::raw_data_block::IdSynEle;
use crate::sbr_element::SbrElement;
use crate::sbr_header::SbrHeader;
use crate::{Error, Result};

/// Field width of `bs_sbr_crc_bits` (Table 4.62).
pub const SBR_CRC_BITS: u32 = 10;

/// A fully-parsed `sbr_extension_data()` payload (Table 4.62).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbrExtensionData {
    /// `bs_sbr_crc_bits` (10-bit) when `crc_flag` was set (the
    /// `EXT_SBR_DATA_CRC` extension type); `None` for the plain
    /// `EXT_SBR_DATA` type. Verify with [`Self::verify_crc`].
    pub crc: Option<u16>,
    /// The protected bit range `[start, end)` — absolute positions in
    /// the buffer the parsing [`BitReader`] was constructed over —
    /// covering every `sbr_extension_data()` bit after the CRC field
    /// up to the end of `sbr_data()` (the §4.4.2.8.1 coverage region,
    /// `num_sbr_bits − 10` bits). `None` when no CRC was present.
    pub crc_region: Option<(u64, u64)>,
    /// `bs_header_flag` — whether this payload transmitted a fresh
    /// `sbr_header()`.
    pub header_present: bool,
    /// The active SBR header for this payload: the freshly parsed one
    /// when `header_present`, otherwise the reused `prev_header`. The
    /// caller threads this forward as the next payload's `prev_header`.
    pub header: SbrHeader,
    /// The decoded SBR data element (single channel or channel pair),
    /// dispatched on the core element's `id_aac`.
    pub element: SbrElement,
    /// The number of SBR side-info bits consumed before the trailing
    /// `bs_fill_bits` (the spec's `num_sbr_bits`). Useful for callers
    /// validating against the `extension_payload()` byte count.
    pub num_sbr_bits: u64,
}

impl SbrExtensionData {
    /// Parse an `sbr_extension_data(id_aac, crc_flag)` payload (Table
    /// 4.62) from `reader`, positioned at the first SBR bit (i.e. the
    /// caller — [`crate::extension_payload`] — has already consumed the
    /// 4-bit `extension_type`).
    ///
    /// * `id_aac` — the AAC core element this SBR payload extends: only
    ///   [`IdSynEle::Sce`] / [`IdSynEle::Cpe`] are valid (an SBR payload
    ///   only attaches to a channel element). Any other id is rejected
    ///   with [`Error::SbrFreqBandInvalid`].
    /// * `crc_flag` — `true` for the `EXT_SBR_DATA_CRC` extension type
    ///   (a 10-bit `bs_sbr_crc_bits` field precedes the header), `false`
    ///   for plain `EXT_SBR_DATA`.
    /// * `fs_sbr` — the SBR *internal* sample rate (twice the AAC core
    ///   `samplingFrequencyIndex` rate). Drives [`SbrHeader::derive_bands`].
    /// * `cnt` — the `extension_payload()` byte count `cnt` (Table 4.51),
    ///   used to size the trailing `bs_fill_bits` alignment. Pass `None`
    ///   to skip the fill consumption (when the caller bounds the reader
    ///   itself); the fill is then left in the reader.
    /// * `prev_header` — the most recent transmitted header for the reuse
    ///   path; `None` on the stream's first SBR payload. A clear
    ///   `bs_header_flag` with `prev_header == None` is ill-formed.
    pub fn parse(
        reader: &mut BitReader<'_>,
        id_aac: IdSynEle,
        crc_flag: bool,
        fs_sbr: u32,
        cnt: Option<u32>,
        prev_header: Option<SbrHeader>,
    ) -> Result<Self> {
        let start = reader.bit_position();

        let crc = if crc_flag {
            Some(read(reader, SBR_CRC_BITS)? as u16)
        } else {
            None
        };
        let region_start = reader.bit_position();

        // Non-scalable core ⇒ sbr_layer == SBR_NOT_SCALABLE, so the
        // bs_header_flag is always present (Table 4.62 Note 1).
        let header_present = read_flag(reader)?;
        Self::finish(
            reader,
            id_aac,
            crc,
            start,
            region_start,
            header_present,
            prev_header,
            fs_sbr,
            cnt,
        )
    }

    /// [`SbrExtensionData::parse`] for a caller that has already
    /// consumed the `extension_type` nibble, the optional 10-bit CRC
    /// field, **and** a set `bs_header_flag` (the pre-header probe in
    /// [`crate::extension_payload::ExtensionPayload::parse_with_sbr`]).
    /// `nibble_start` is the bit position of the `extension_type`
    /// nibble, from which the CRC coverage region and the Table 4.62
    /// `num_sbr_bits` accounting are reconstructed.
    #[allow(clippy::too_many_arguments)]
    pub fn parse_after_prefix(
        reader: &mut BitReader<'_>,
        id_aac: IdSynEle,
        crc: Option<u16>,
        nibble_start: u64,
        fs_sbr: u32,
        cnt: Option<u32>,
        prev_header: Option<SbrHeader>,
    ) -> Result<Self> {
        let start = nibble_start + 4;
        let region_start = start
            + if crc.is_some() {
                u64::from(SBR_CRC_BITS)
            } else {
                0
            };
        Self::finish(
            reader,
            id_aac,
            crc,
            start,
            region_start,
            true,
            prev_header,
            fs_sbr,
            cnt,
        )
    }

    /// Shared tail of the two parse entries: `sbr_header()` (when
    /// present), band derivation, `sbr_data()`, and the Table 4.62
    /// `bs_fill_bits` alignment.
    #[allow(clippy::too_many_arguments)]
    fn finish(
        reader: &mut BitReader<'_>,
        id_aac: IdSynEle,
        crc: Option<u16>,
        start: u64,
        region_start: u64,
        header_present: bool,
        prev_header: Option<SbrHeader>,
        fs_sbr: u32,
        cnt: Option<u32>,
    ) -> Result<Self> {
        let header = if header_present {
            SbrHeader::parse(reader)?
        } else {
            // Reuse the previous transmitted header. A stream that
            // opens with header-less SBR payloads is the §4.5.2.8.1
            // "upsampling and delay adjustment only" state — the
            // parse_with_sbr caller intercepts that case before
            // reaching here, so a missing header at this point is a
            // caller-contract violation.
            prev_header.ok_or(Error::SbrFreqBandInvalid)?
        };

        // sbr_data(id_aac, bs_amp_res): the band tables are derived from
        // the active header at the SBR internal rate; the element type is
        // selected by the core element id_aac.
        let bands = header.derive_bands(fs_sbr)?;
        let element = match id_aac {
            IdSynEle::Sce => SbrElement::parse_single(reader, &bands, header.amp_res)?,
            IdSynEle::Cpe => SbrElement::parse_pair(reader, &bands, header.amp_res)?,
            _ => return Err(Error::SbrFreqBandInvalid),
        };

        let region_end = reader.bit_position();
        let num_sbr_bits = region_end - start;

        // num_align_bits = (8*cnt - 4 - num_sbr_bits) % 8. The `- 4`
        // accounts for the extension_type nibble the caller already read;
        // when cnt is known, consume the trailing bs_fill_bits so the
        // reader lands on the next extension_payload element.
        let mut crc_end = region_end;
        if let Some(cnt) = cnt {
            let total = u64::from(cnt) * 8;
            let consumed = num_sbr_bits + 4; // + the extension_type nibble
            if total < consumed {
                return Err(Error::SbrFreqBandInvalid);
            }
            let align = (total - consumed) % 8;
            if align > 0 {
                read(reader, align as u32)?;
            }
            // §4.5.2.8.1: "The checksum shall be calculated covering
            // the whole SBR data range including possible
            // bs_fill_bits" — the coverage extends past the end of
            // sbr_data() through the alignment padding to the end of
            // the fill payload. Confirmed against the ISO/IEC
            // 14496-26 `al_sbr_*` type-14 vectors, whose
            // header-bearing payloads carry non-zero bs_fill_bits and
            // only verify over the padded region.
            // (`start` is 4 bits past the extension_type nibble; the
            // grouping avoids u64 underflow when a caller parses a
            // nibble-less buffer from position 0.)
            crc_end = start + (total - 4);
        }

        Ok(SbrExtensionData {
            crc,
            crc_region: crc.map(|_| (region_start, crc_end)),
            header_present,
            header,
            element,
            num_sbr_bits,
        })
    }

    /// Verify the `bs_sbr_crc_bits` checksum against the §4.5.2.8.1
    /// coverage region (every payload bit after the CRC field to the
    /// end of the fill payload — "the whole SBR data range including
    /// possible bs_fill_bits").
    ///
    /// `data` must be the same byte buffer the parsing [`BitReader`]
    /// was constructed over ([`Self::crc_region`] holds absolute bit
    /// positions into it). A payload without a CRC field (plain
    /// `EXT_SBR_DATA`) verifies vacuously. Returns
    /// [`Error::SbrCrcMismatch`] when the recomputed 10-bit `G10`
    /// (zero-init) CRC disagrees with the transmitted value.
    pub fn verify_crc(&self, data: &[u8]) -> Result<()> {
        if let (Some(crc), Some((start, end))) = (self.crc, self.crc_region) {
            if crate::adts_crc::sbr_crc(data, start, end) != crc {
                return Err(Error::SbrCrcMismatch);
            }
        }
        Ok(())
    }
}

#[inline]
fn read(reader: &mut BitReader<'_>, n: u32) -> Result<u32> {
    reader.read_u32(n).map_err(|_| Error::SbrFreqBandInvalid)
}

#[inline]
fn read_flag(reader: &mut BitReader<'_>) -> Result<bool> {
    reader.read_bit().map_err(|_| Error::SbrFreqBandInvalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sbr_freq_bands::HiLoTables;
    use crate::sbr_grid::FrameClass;
    use crate::sbr_huffman::{env_tables, noise_tables, SbrHuffContext};
    use oxideav_core::bits::BitWriter;

    const FS_SBR: u32 = 88_200; // 44.1 kHz core, doubled.

    /// A header carrying explicit extra-1 params (freq_scale 0,
    /// alter_scale false, noise_bands 2) so the derived band geometry is
    /// deterministic; extra-2 absent.
    fn write_header(w: &mut BitWriter, amp_res: bool) {
        w.write_bit(amp_res); // bs_amp_res
        w.write_u32(5, 4); // bs_start_freq
        w.write_u32(0, 4); // bs_stop_freq
        w.write_u32(1, 3); // bs_xover_band
        w.write_u32(0, 2); // bs_reserved
        w.write_bit(true); // bs_header_extra_1
        w.write_bit(false); // bs_header_extra_2
        w.write_u32(0, 2); // bs_freq_scale
        w.write_bit(false); // bs_alter_scale
        w.write_u32(2, 2); // bs_noise_bands
    }

    /// The band tables a `write_header(_, _)`-built header derives.
    fn header_bands() -> HiLoTables {
        let mut w = BitWriter::new();
        write_header(&mut w, false);
        let bytes = w.finish();
        let mut r = BitReader::new(&bytes);
        let h = SbrHeader::parse(&mut r).unwrap();
        h.derive_bands(FS_SBR).unwrap()
    }

    fn push_code(w: &mut BitWriter, table: &[(u8, u32)], idx: usize) {
        let (len, code) = table[idx];
        w.write_u32(code, len as u32);
    }

    /// Minimal single-channel SBR element body (FIXFIX single env, freq
    /// deltas, no sinusoidal / extended data). Mirrors the
    /// `sbr_element` test helper but inline so the band geometry comes
    /// from the header we just wrote.
    fn write_minimal_sce(w: &mut BitWriter, bands: &HiLoTables) {
        let n_high = bands.n_high();
        let n_q = bands.n_q();
        w.write_bit(false); // bs_data_extra
        w.write_u32(FrameClass::FixFix.to_bits(), 2);
        w.write_u32(0, 2); // 2^0 = 1 env
        w.write_bit(true); // freq_res[0] high
        w.write_bit(false); // df_env[0]
        w.write_bit(false); // df_noise[0]
        for _ in 0..n_q {
            w.write_u32(1, 2); // invf modes
        }
        let (_, (f_huff, f_lav)) = env_tables(SbrHuffContext {
            coupling: false,
            ch: false,
            amp_res: false,
        });
        w.write_u32(33, 7); // env start value (amp_res override → 7-bit)
        for i in 1..n_high {
            push_code(w, f_huff, (i + f_lav as usize) % f_huff.len());
        }
        let (_, (nf, nfl)) = noise_tables(SbrHuffContext {
            coupling: false,
            ch: false,
            amp_res: false,
        });
        w.write_u32(10, 5); // noise start
        for i in 1..n_q {
            push_code(w, nf, (i + nfl as usize) % nf.len());
        }
        w.write_bit(false); // bs_add_harmonic_flag
        w.write_bit(false); // bs_extended_data
    }

    #[test]
    fn parses_header_plus_single_channel() {
        let bands = header_bands();
        let mut w = BitWriter::new();
        w.write_bit(true); // bs_header_flag
        write_header(&mut w, true);
        write_minimal_sce(&mut w, &bands);
        let bytes = w.finish();

        let mut r = BitReader::new(&bytes);
        let sbr =
            SbrExtensionData::parse(&mut r, IdSynEle::Sce, false, FS_SBR, None, None).unwrap();
        assert!(sbr.header_present);
        assert!(sbr.crc.is_none());
        assert_eq!(sbr.header.start_freq, 5);
        assert_eq!(sbr.header.freq_scale, 0);
        assert!(!sbr.element.coupling);
        assert_eq!(sbr.element.channels.len(), 1);
        assert_eq!(sbr.element.channels[0].envelope.data[0][0], 33);
        assert_eq!(sbr.element.channels[0].noise.data[0][0], 10);
    }

    #[test]
    fn crc_flag_reads_ten_bit_field() {
        let bands = header_bands();
        let mut w = BitWriter::new();
        w.write_u32(0x2A5, SBR_CRC_BITS); // bs_sbr_crc_bits
        w.write_bit(true); // bs_header_flag
        write_header(&mut w, true);
        write_minimal_sce(&mut w, &bands);
        let bytes = w.finish();

        let mut r = BitReader::new(&bytes);
        let sbr = SbrExtensionData::parse(&mut r, IdSynEle::Sce, true, FS_SBR, None, None).unwrap();
        assert_eq!(sbr.crc, Some(0x2A5));
        assert!(sbr.header_present);
    }

    #[test]
    fn header_reuse_when_flag_clear() {
        // A prior header is reused when bs_header_flag == 0.
        let bands = header_bands();
        let prev = {
            let mut w = BitWriter::new();
            write_header(&mut w, true);
            let bytes = w.finish();
            SbrHeader::parse(&mut BitReader::new(&bytes)).unwrap()
        };
        let mut w = BitWriter::new();
        w.write_bit(false); // bs_header_flag clear
        write_minimal_sce(&mut w, &bands);
        let bytes = w.finish();

        let mut r = BitReader::new(&bytes);
        let sbr = SbrExtensionData::parse(&mut r, IdSynEle::Sce, false, FS_SBR, None, Some(prev))
            .unwrap();
        assert!(!sbr.header_present);
        assert_eq!(sbr.header, prev);
        assert_eq!(sbr.element.channels.len(), 1);
    }

    #[test]
    fn header_clear_without_prior_is_error() {
        let mut w = BitWriter::new();
        w.write_bit(false); // bs_header_flag clear, no prior header
        let bytes = w.finish();
        let mut r = BitReader::new(&bytes);
        assert!(matches!(
            SbrExtensionData::parse(&mut r, IdSynEle::Sce, false, FS_SBR, None, None),
            Err(Error::SbrFreqBandInvalid)
        ));
    }

    #[test]
    fn non_channel_id_aac_is_rejected() {
        let mut w = BitWriter::new();
        w.write_bit(true);
        write_header(&mut w, true);
        let bytes = w.finish();
        let mut r = BitReader::new(&bytes);
        assert!(matches!(
            SbrExtensionData::parse(&mut r, IdSynEle::Lfe, false, FS_SBR, None, None),
            Err(Error::SbrFreqBandInvalid)
        ));
    }

    #[test]
    fn fill_bits_consumed_when_cnt_given() {
        // Pad the payload to a known byte count and confirm the walker
        // consumes the trailing bs_fill_bits so the reader is byte-aligned
        // at `cnt` bytes (minus the extension_type nibble the caller owns).
        let bands = header_bands();
        let mut w = BitWriter::new();
        w.write_bit(true);
        write_header(&mut w, true);
        write_minimal_sce(&mut w, &bands);
        let mut body = w.finish();
        // cnt counts whole bytes of the extension_payload including its
        // 4-bit type nibble; add two trailing fill bytes so there is a
        // non-trivial bs_fill_bits to swallow and the reader has the bits.
        let cnt = (body.len() + 2) as u32;
        body.extend_from_slice(&[0u8, 0u8]);
        let mut r = BitReader::new(&body);
        let before = r.bit_position();
        let sbr =
            SbrExtensionData::parse(&mut r, IdSynEle::Sce, false, FS_SBR, Some(cnt), None).unwrap();
        let consumed = r.bit_position() - before;
        // Total consumed (+ the 4-bit type nibble) must be a multiple of 8.
        assert_eq!((consumed + 4) % 8, 0);
        assert_eq!(sbr.element.channels.len(), 1);
    }
}

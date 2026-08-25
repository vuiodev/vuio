//! AC-3 synchronization frame header — `syncinfo` (§5.3.1 / §5.4.1).
//!
//! The syncinfo is the first field of every AC-3 syncframe. It is
//! exactly **5 bytes** of fixed-width fields (no variable bits):
//!
//! ```text
//!   syncword     16 bits  (always 0x0B77, transmission is MSB-first)
//!   crc1         16 bits  (CRC over the first 5/8 of the syncframe)
//!   fscod         2 bits  (sample rate: Table 5.6)
//!   frmsizecod    6 bits  (frame length: Table 5.18)
//! ```
//!
//! `parse` does zero CRC validation — the spec allows either crc1 or
//! crc2 (or neither) to be checked; we defer that to an optional
//! verification pass once the whole frame is buffered.

use oxideav_core::{Error, Result};

use crate::tables::{frame_length_bytes, nominal_bitrate_kbps, sample_rate_hz, FRAME_SIZE_TABLE};

/// The 16-bit AC-3 syncword (§5.4.1.1).
pub const SYNCWORD: u16 = 0x0B77;

/// A fully-parsed syncinfo header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyncInfo {
    pub crc1: u16,
    pub fscod: u8,
    pub frmsizecod: u8,
    /// Sample rate in Hz as derived from `fscod` (Table 5.6).
    pub sample_rate: u32,
    /// Full frame length in **bytes** (including syncinfo itself, BSI,
    /// all 6 audio blocks, auxdata, and crc2). Derived from
    /// (fscod, frmsizecod) via Table 5.18.
    pub frame_length: u32,
}

impl SyncInfo {
    /// Typed view over [`Self::fscod`] per §5.4.1.3 / Table 5.6. The
    /// returned [`SampleRateCode`] decodes the 2-bit `fscod` field
    /// into one of the three valid sampling-rate codepoints
    /// ([`SampleRateCode::FortyEightKHz`] /
    /// [`SampleRateCode::FortyFourPointOneKHz`] /
    /// [`SampleRateCode::ThirtyTwoKHz`]) or
    /// [`SampleRateCode::Reserved`] for the spec-reserved `'11'`
    /// codepoint (per §5.4.1.3 the decoder must mute on receipt).
    ///
    /// [`parse`] already rejects the reserved codepoint at frame
    /// boundary — a [`SyncInfo`] handed back from [`parse`] therefore
    /// always reports a non-reserved [`SampleRateCode`]. The accessor
    /// remains a thin wrapper over [`Self::fscod`] for chain consumers
    /// that construct a [`SyncInfo`] by hand (e.g. resynthesising one
    /// from container-stored metadata) and want the typed surface
    /// without re-walking Table 5.6.
    pub fn sample_rate_code(&self) -> SampleRateCode {
        SampleRateCode::from_code(self.fscod)
    }

    /// Typed view over [`Self::frmsizecod`] per §5.4.1.4 / Table 5.18.
    /// The returned [`FrameSizeCode`] decodes the 6-bit `frmsizecod`
    /// field into one of the 38 valid frame-size codepoints
    /// (`0..=37`) or [`FrameSizeCode::Reserved`] for the `38..=63`
    /// codepoints that have no Table 5.18 row.
    ///
    /// [`parse`] already rejects an out-of-range `frmsizecod` at frame
    /// boundary — a [`SyncInfo`] handed back from [`parse`] therefore
    /// always reports a non-reserved [`FrameSizeCode`]. The accessor
    /// remains a thin wrapper over [`Self::frmsizecod`] for chain
    /// consumers that construct a [`SyncInfo`] by hand (e.g.
    /// resynthesising one from container-stored metadata) and want the
    /// typed surface — the nominal bit-rate and per-rate word count —
    /// without re-walking Table 5.18.
    pub fn frame_size_code(&self) -> FrameSizeCode {
        FrameSizeCode::from_code(self.frmsizecod)
    }
}

/// §5.4.1.3 sample-rate code (Table 5.6). A 2-bit codeword carried in
/// every AC-3 syncframe that selects one of the three supported
/// sampling rates — 48 kHz, 44.1 kHz, or 32 kHz — or the reserved
/// `'11'` codepoint that mandates a decoder mute.
///
/// Surfaced via [`SyncInfo::sample_rate_code`]. Callers that just
/// want the rate in Hz can keep reading the pre-resolved
/// [`SyncInfo::sample_rate`] field; the typed enum is for chain
/// consumers that branch on the codepoint itself (e.g. a §7.15
/// hearing-threshold table lookup that indexes into the per-`fscod`
/// row, or a §7.2.2.5 masking-curve evaluator that needs to flag a
/// reserved `fscod` separately from an out-of-range `dbpbcod`).
///
/// Per §5.4.1.3: "If the reserved code is indicated, the decoder
/// should not attempt to decode audio and should mute." [`parse`]
/// enforces that rule at frame boundary by returning
/// [`oxideav_core::Error::Invalid`] for the `'11'` codepoint, so a
/// [`SyncInfo`] obtained from [`parse`] never reports
/// [`Self::Reserved`]; the variant is preserved so a chain consumer
/// constructing a [`SyncInfo`] from container-stored metadata (where
/// the upstream demuxer may not have validated `fscod`) can detect
/// the reserved code without re-walking Table 5.6.
///
/// Annex E (E-AC-3) overloads the `'11'` codepoint as a reduced-rate
/// indicator that triggers a follow-on `fscod2` codeword (§E.2.3.1.4
/// / §E.2.3.1.5), so this enum's [`Self::Reserved`] variant
/// corresponds to the **base AC-3** decoder-mute semantics only. The
/// Annex E `bsi` carries its own `fscod` + `fscod2` raw pair and the
/// reduced-rate paths are surfaced via the E-AC-3 BSI; this base-AC-3
/// enum is not mirrored on the Annex E `Bsi`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleRateCode {
    /// `'00'` — 48 kHz.
    FortyEightKHz,
    /// `'01'` — 44.1 kHz.
    FortyFourPointOneKHz,
    /// `'10'` — 32 kHz.
    ThirtyTwoKHz,
    /// `'11'` — reserved. Per §5.4.1.3 the decoder "should not attempt
    /// to decode audio and should mute" on receipt; [`parse`] rejects
    /// this codepoint at frame boundary so a [`SyncInfo`] obtained
    /// from [`parse`] never carries it.
    Reserved,
}

impl SampleRateCode {
    /// Decode the 2-bit wire value verbatim per Table 5.6. Only the
    /// low 2 bits of `code` are consulted; the upper bits are
    /// ignored so a caller that passes a full `bsid`-style byte does
    /// not need to mask first.
    pub fn from_code(code: u8) -> Self {
        match code & 0x3 {
            0 => SampleRateCode::FortyEightKHz,
            1 => SampleRateCode::FortyFourPointOneKHz,
            2 => SampleRateCode::ThirtyTwoKHz,
            _ => SampleRateCode::Reserved,
        }
    }

    /// Raw 2-bit code as it appeared on the wire — the round-trip
    /// inverse of [`Self::from_code`].
    pub fn raw(self) -> u8 {
        match self {
            SampleRateCode::FortyEightKHz => 0,
            SampleRateCode::FortyFourPointOneKHz => 1,
            SampleRateCode::ThirtyTwoKHz => 2,
            SampleRateCode::Reserved => 3,
        }
    }

    /// Sample rate in **Hz** per Table 5.6, or `None` for the
    /// reserved codepoint (the spec mandates a decoder mute, with no
    /// associated playback rate).
    pub fn hertz(self) -> Option<u32> {
        match self {
            SampleRateCode::FortyEightKHz => Some(48_000),
            SampleRateCode::FortyFourPointOneKHz => Some(44_100),
            SampleRateCode::ThirtyTwoKHz => Some(32_000),
            SampleRateCode::Reserved => None,
        }
    }

    /// Sample rate in **kHz** per Table 5.6, or `None` for the
    /// reserved codepoint. Equivalent to [`Self::hertz`] divided by
    /// 1 000; kept as a separate accessor since the spec text /
    /// Table 5.6 / Annex D handbook tables phrase the rate in kHz
    /// throughout.
    pub fn kilohertz(self) -> Option<u32> {
        match self {
            SampleRateCode::FortyEightKHz => Some(48),
            SampleRateCode::FortyFourPointOneKHz => Some(44),
            SampleRateCode::ThirtyTwoKHz => Some(32),
            SampleRateCode::Reserved => None,
        }
    }

    /// `true` only for [`Self::Reserved`] — lets a probe / re-emit
    /// tool flag streams that carry the `'11'` reserved codepoint
    /// (per §5.4.1.3 such streams must trigger a decoder mute)
    /// without re-walking Table 5.6.
    pub fn is_reserved(self) -> bool {
        matches!(self, SampleRateCode::Reserved)
    }

    /// Row index into the §7.15 hearing-threshold table
    /// (`tables::HTH[fscod]`) and into [`crate::tables::HTH`] in
    /// general. Returns `None` for [`Self::Reserved`] since the spec
    /// only defines the three valid rows (fscod 0..=2). The index is
    /// equal to [`Self::raw`] for the three valid codepoints but
    /// returning it from a dedicated accessor keeps Table 5.6 →
    /// Table 7.15 routing one call away from the typed surface.
    pub fn hth_row_index(self) -> Option<usize> {
        match self {
            SampleRateCode::FortyEightKHz => Some(0),
            SampleRateCode::FortyFourPointOneKHz => Some(1),
            SampleRateCode::ThirtyTwoKHz => Some(2),
            SampleRateCode::Reserved => None,
        }
    }
}

/// §5.4.1.4 frame-size code (Table 5.18). A 6-bit codeword carried in
/// every AC-3 syncframe that — together with the [`SampleRateCode`] —
/// selects the number of 16-bit words in the syncframe before the next
/// syncword. Per §5.4.1.4: "The frame size code is used along with the
/// sample rate code to determine the number of (2-byte) words before
/// the next syncword."
///
/// Table 5.18 defines 38 valid codepoints (`frmsizecod = 0..=37`); the
/// remaining `38..=63` codepoints have no table row and are surfaced as
/// [`FrameSizeCode::Reserved`]. Each nominal bit-rate occupies two
/// neighbouring codepoints (the even/odd pair) because a 44.1 kHz
/// encoding must alternate frame sizes to hit the declared bit-rate on
/// average — both sizes appear in Table 5.18.
///
/// Surfaced via [`SyncInfo::frame_size_code`]. Callers that just want
/// the byte length the demuxer must consume can keep reading the
/// pre-resolved [`SyncInfo::frame_length`] field; the typed enum is for
/// chain consumers that branch on the nominal bit-rate
/// ([`Self::nominal_bitrate_kbps`]) or need the raw per-rate word count
/// ([`Self::words`]) without re-walking Table 5.18.
///
/// [`parse`] rejects an out-of-range `frmsizecod` at frame boundary by
/// returning [`oxideav_core::Error::Invalid`], so a [`SyncInfo`]
/// obtained from [`parse`] never reports [`Self::Reserved`]; the
/// variant is preserved so a chain consumer constructing a [`SyncInfo`]
/// from container-stored metadata (where the upstream demuxer may not
/// have validated `frmsizecod`) can detect the reserved range without
/// re-walking Table 5.18.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameSizeCode {
    /// A valid Table 5.18 codepoint (`frmsizecod = 0..=37`).
    Valid(u8),
    /// A `frmsizecod` in the `38..=63` range — no Table 5.18 row. Per
    /// §5.4.1.4 only `0..=37` are defined; [`parse`] rejects this range
    /// at frame boundary so a [`SyncInfo`] from [`parse`] never carries
    /// it.
    Reserved,
}

impl FrameSizeCode {
    /// Decode the 6-bit wire value verbatim per Table 5.18. The low 6
    /// bits of `code` are consulted; the upper 2 bits are ignored so a
    /// caller that passes byte 4 of the syncinfo (which packs `fscod`
    /// in the top two bits) does not need to mask first. A masked value
    /// of `0..=37` maps to [`Self::Valid`]; `38..=63` maps to
    /// [`Self::Reserved`].
    pub fn from_code(code: u8) -> Self {
        let frmsizecod = code & 0x3F;
        if (frmsizecod as usize) < FRAME_SIZE_TABLE.len() {
            FrameSizeCode::Valid(frmsizecod)
        } else {
            FrameSizeCode::Reserved
        }
    }

    /// Raw 6-bit code as it appeared on the wire — the round-trip
    /// inverse of [`Self::from_code`] for the valid range. Returns the
    /// stored codepoint for [`Self::Valid`]; for [`Self::Reserved`]
    /// there is no single wire value (the variant collapses the whole
    /// `38..=63` range) so this returns `None`.
    pub fn raw(self) -> Option<u8> {
        match self {
            FrameSizeCode::Valid(c) => Some(c),
            FrameSizeCode::Reserved => None,
        }
    }

    /// `true` only for [`Self::Reserved`] — lets a probe / re-emit tool
    /// flag streams that carry a `frmsizecod` in the undefined
    /// `38..=63` range without re-walking Table 5.18.
    pub fn is_reserved(self) -> bool {
        matches!(self, FrameSizeCode::Reserved)
    }

    /// Nominal bit rate in **kbps** per Table 5.18, or `None` for the
    /// reserved range. The two neighbouring codepoints that share a
    /// bit-rate (e.g. `frmsizecod = 0` and `1` both = 32 kbps) return
    /// the same value.
    pub fn nominal_bitrate_kbps(self) -> Option<u32> {
        match self {
            FrameSizeCode::Valid(c) => nominal_bitrate_kbps(c),
            FrameSizeCode::Reserved => None,
        }
    }

    /// The Table 5.18 syncframe length in **16-bit words** for the given
    /// [`SampleRateCode`], or `None` if either this code or the rate
    /// code is reserved. The byte length consumed by the demuxer is
    /// twice this value (the table is denominated in 2-byte words per
    /// §5.4.1.4).
    pub fn words(self, rate: SampleRateCode) -> Option<u32> {
        let c = match self {
            FrameSizeCode::Valid(c) => c as usize,
            FrameSizeCode::Reserved => return None,
        };
        let (_, w32, w44, w48) = FRAME_SIZE_TABLE[c];
        match rate {
            SampleRateCode::FortyEightKHz => Some(w48),
            SampleRateCode::FortyFourPointOneKHz => Some(w44),
            SampleRateCode::ThirtyTwoKHz => Some(w32),
            SampleRateCode::Reserved => None,
        }
    }

    /// The Table 5.18 syncframe length in **bytes** for the given
    /// [`SampleRateCode`] (`2 ×` [`Self::words`]), or `None` if either
    /// code is reserved. This matches the pre-resolved
    /// [`SyncInfo::frame_length`] field for any frame [`parse`] accepts.
    pub fn frame_length_bytes(self, rate: SampleRateCode) -> Option<u32> {
        self.words(rate).map(|w| w * 2)
    }
}

/// Parse the 5-byte syncinfo out of the front of `data`.
///
/// Returns `Error::Invalid` if the syncword is missing or if the
/// fscod/frmsizecod pair lands on a reserved entry (in which case the
/// spec mandates the decoder mute — see §5.4.1.3 and §5.4.1.4).
pub fn parse(data: &[u8]) -> Result<SyncInfo> {
    if data.len() < 5 {
        return Err(Error::invalid(format!(
            "ac3: syncinfo needs 5 bytes, got {}",
            data.len()
        )));
    }
    let syncword = u16::from_be_bytes([data[0], data[1]]);
    if syncword != SYNCWORD {
        return Err(Error::invalid(format!(
            "ac3: bad syncword 0x{syncword:04X} (expected 0x0B77)"
        )));
    }
    let crc1 = u16::from_be_bytes([data[2], data[3]]);
    // Byte 4: fscod (2 bits, MSB) + frmsizecod (6 bits, LSB).
    let b4 = data[4];
    let fscod = (b4 >> 6) & 0x03;
    let frmsizecod = b4 & 0x3F;
    let sample_rate = sample_rate_hz(fscod)
        .ok_or_else(|| Error::invalid("ac3: reserved fscod '11' — decoder must mute"))?;
    let frame_length = frame_length_bytes(fscod, frmsizecod)
        .ok_or_else(|| Error::invalid(format!("ac3: invalid frmsizecod {frmsizecod}")))?;
    Ok(SyncInfo {
        crc1,
        fscod,
        frmsizecod,
        sample_rate,
        frame_length,
    })
}

/// Find the byte offset of the next AC-3 syncword starting from `offset`.
/// Returns `None` if no syncword is found.
///
/// Used by demuxers that don't already know where frame boundaries are
/// (e.g. raw / corrupted elementary streams) — but well-formed
/// containers hand us frames pre-cut and we can call `parse` directly.
pub fn find_syncword(data: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 1 < data.len() {
        if data[i] == 0x0B && data[i + 1] == 0x77 {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Happy path: a hand-crafted 5-byte header for a 48 kHz, 192 kbps
    /// stream (frmsizecod = 20 → 384 words = 768 bytes).
    #[test]
    fn parses_valid_header() {
        let mut buf = [0u8; 5];
        buf[0] = 0x0B;
        buf[1] = 0x77;
        buf[2] = 0xAB;
        buf[3] = 0xCD;
        // fscod=0 (48 kHz), frmsizecod=20
        buf[4] = 20;
        let si = parse(&buf).unwrap();
        assert_eq!(si.sample_rate, 48_000);
        assert_eq!(si.frame_length, 768);
        assert_eq!(si.crc1, 0xABCD);
        assert_eq!(si.frmsizecod, 20);
    }

    #[test]
    fn rejects_bad_syncword() {
        let buf = [0xFF, 0x00, 0, 0, 0];
        assert!(parse(&buf).is_err());
    }

    #[test]
    fn rejects_reserved_fscod() {
        let buf = [0x0B, 0x77, 0, 0, 0xC0]; // fscod = 0b11
        assert!(parse(&buf).is_err());
    }

    #[test]
    fn rejects_reserved_frmsizecod() {
        // frmsizecod = 38 is past the end of Table 5.18.
        let buf = [0x0B, 0x77, 0, 0, 38];
        assert!(parse(&buf).is_err());
    }

    #[test]
    fn finds_syncword() {
        let mut buf = vec![0u8; 32];
        buf[17] = 0x0B;
        buf[18] = 0x77;
        assert_eq!(find_syncword(&buf, 0), Some(17));
        assert_eq!(find_syncword(&buf, 18), None);
    }

    /// Round-trip every Table 5.6 row through `SampleRateCode` —
    /// the four codepoints `'00'`/`'01'`/`'10'`/`'11'` decode to
    /// the four enum variants and `raw()` returns the original
    /// codepoint verbatim.
    #[test]
    fn sample_rate_code_round_trip_all_codepoints() {
        let rows = [
            (0u8, SampleRateCode::FortyEightKHz),
            (1, SampleRateCode::FortyFourPointOneKHz),
            (2, SampleRateCode::ThirtyTwoKHz),
            (3, SampleRateCode::Reserved),
        ];
        for (code, expected) in rows {
            let got = SampleRateCode::from_code(code);
            assert_eq!(got, expected, "code 0x{code:02X}");
            assert_eq!(got.raw(), code, "raw() round-trip for 0x{code:02X}");
        }
    }

    /// `SampleRateCode::from_code` ignores the upper bits of the
    /// argument so a caller can pass a full `bsid`-style byte without
    /// masking first.
    #[test]
    fn sample_rate_code_ignores_upper_bits() {
        assert_eq!(
            SampleRateCode::from_code(0xFC),
            SampleRateCode::FortyEightKHz
        );
        assert_eq!(
            SampleRateCode::from_code(0b1111_1101),
            SampleRateCode::FortyFourPointOneKHz,
        );
        assert_eq!(
            SampleRateCode::from_code(0x06),
            SampleRateCode::ThirtyTwoKHz
        );
        assert_eq!(SampleRateCode::from_code(0xFF), SampleRateCode::Reserved);
    }

    /// `hertz()` and `kilohertz()` return the Table 5.6 rates for the
    /// three valid codepoints and `None` for the reserved codepoint.
    #[test]
    fn sample_rate_code_hertz_and_kilohertz() {
        assert_eq!(SampleRateCode::FortyEightKHz.hertz(), Some(48_000));
        assert_eq!(SampleRateCode::FortyFourPointOneKHz.hertz(), Some(44_100));
        assert_eq!(SampleRateCode::ThirtyTwoKHz.hertz(), Some(32_000));
        assert_eq!(SampleRateCode::Reserved.hertz(), None);

        assert_eq!(SampleRateCode::FortyEightKHz.kilohertz(), Some(48));
        assert_eq!(SampleRateCode::FortyFourPointOneKHz.kilohertz(), Some(44));
        assert_eq!(SampleRateCode::ThirtyTwoKHz.kilohertz(), Some(32));
        assert_eq!(SampleRateCode::Reserved.kilohertz(), None);
    }

    /// `is_reserved()` flags only the `'11'` codepoint.
    #[test]
    fn sample_rate_code_is_reserved_only_for_eleven() {
        assert!(!SampleRateCode::FortyEightKHz.is_reserved());
        assert!(!SampleRateCode::FortyFourPointOneKHz.is_reserved());
        assert!(!SampleRateCode::ThirtyTwoKHz.is_reserved());
        assert!(SampleRateCode::Reserved.is_reserved());
    }

    /// `hth_row_index()` matches the Table 7.15 row order so callers
    /// can route a typed sample-rate code straight into a §7.2.2.5
    /// hearing-threshold lookup.
    #[test]
    fn sample_rate_code_hth_row_index_matches_table_7_15() {
        assert_eq!(SampleRateCode::FortyEightKHz.hth_row_index(), Some(0));
        assert_eq!(
            SampleRateCode::FortyFourPointOneKHz.hth_row_index(),
            Some(1)
        );
        assert_eq!(SampleRateCode::ThirtyTwoKHz.hth_row_index(), Some(2));
        assert_eq!(SampleRateCode::Reserved.hth_row_index(), None);
    }

    /// `SyncInfo::sample_rate_code()` mirrors the resolved
    /// `sample_rate_hz()` lookup on every valid codepoint — for any
    /// frame `parse` accepts, the typed surface and the raw
    /// `sample_rate` field always agree.
    #[test]
    fn syncinfo_sample_rate_code_matches_resolved_rate() {
        // fscod=0 / frmsizecod=20 = 48 kHz, 384 words.
        let buf = [0x0B, 0x77, 0xAB, 0xCD, 20];
        let si = parse(&buf).unwrap();
        let src = si.sample_rate_code();
        assert_eq!(src, SampleRateCode::FortyEightKHz);
        assert_eq!(src.hertz(), Some(si.sample_rate));
        assert!(!src.is_reserved());

        // fscod=1 / frmsizecod=0 = 44.1 kHz, 69 words.
        let buf = [0x0B, 0x77, 0, 0, 0x40];
        let si = parse(&buf).unwrap();
        let src = si.sample_rate_code();
        assert_eq!(src, SampleRateCode::FortyFourPointOneKHz);
        assert_eq!(src.hertz(), Some(si.sample_rate));

        // fscod=2 / frmsizecod=0 = 32 kHz, 96 words.
        let buf = [0x0B, 0x77, 0, 0, 0x80];
        let si = parse(&buf).unwrap();
        let src = si.sample_rate_code();
        assert_eq!(src, SampleRateCode::ThirtyTwoKHz);
        assert_eq!(src.hertz(), Some(si.sample_rate));
    }

    /// A caller constructing a `SyncInfo` by hand (e.g. from
    /// container-stored metadata) with the reserved `fscod = 3`
    /// code still gets the `SampleRateCode::Reserved` typed surface
    /// — `parse` itself never lets the reserved code through, but
    /// the typed accessor on a hand-built `SyncInfo` does flag it.
    #[test]
    fn syncinfo_sample_rate_code_surfaces_reserved_for_hand_built() {
        let si = SyncInfo {
            crc1: 0,
            fscod: 3,
            frmsizecod: 0,
            sample_rate: 0,
            frame_length: 0,
        };
        let src = si.sample_rate_code();
        assert_eq!(src, SampleRateCode::Reserved);
        assert!(src.is_reserved());
        assert_eq!(src.hertz(), None);
        assert_eq!(src.hth_row_index(), None);
    }

    /// Every valid Table 5.18 codepoint (`0..=37`) round-trips through
    /// `FrameSizeCode::Valid` and `raw()` returns the original code; the
    /// `38..=63` reserved range collapses to `FrameSizeCode::Reserved`
    /// with `raw() == None`.
    #[test]
    fn frame_size_code_round_trip_valid_and_reserved() {
        for code in 0u8..=37 {
            let fsc = FrameSizeCode::from_code(code);
            assert_eq!(fsc, FrameSizeCode::Valid(code), "code {code}");
            assert_eq!(fsc.raw(), Some(code), "raw() round-trip for {code}");
            assert!(!fsc.is_reserved(), "code {code} should not be reserved");
        }
        for code in 38u8..=63 {
            let fsc = FrameSizeCode::from_code(code);
            assert_eq!(fsc, FrameSizeCode::Reserved, "code {code}");
            assert_eq!(fsc.raw(), None, "reserved raw() for {code}");
            assert!(fsc.is_reserved(), "code {code} should be reserved");
        }
    }

    /// `from_code` masks off the upper 2 bits so a caller can pass byte
    /// 4 of the syncinfo (which packs `fscod` in bits 7..6) without
    /// masking first: `frmsizecod = 20` with `fscod = '00'` in the top
    /// bits, vs the same `20` with `fscod = '11'` (byte = 0xD4), both
    /// decode to `Valid(20)`.
    #[test]
    fn frame_size_code_ignores_upper_two_bits() {
        assert_eq!(FrameSizeCode::from_code(20), FrameSizeCode::Valid(20));
        // 0b11_010100 = fscod '11' packed over frmsizecod = 20.
        assert_eq!(FrameSizeCode::from_code(0xD4), FrameSizeCode::Valid(20));
        // 0b11_111111 = fscod '11' over frmsizecod = 63 (reserved).
        assert_eq!(FrameSizeCode::from_code(0xFF), FrameSizeCode::Reserved);
    }

    /// `nominal_bitrate_kbps` returns the Table 5.18 nominal rate for a
    /// valid code (and the two-codepoints-per-rate pairing), `None` for
    /// the reserved range.
    #[test]
    fn frame_size_code_nominal_bitrate() {
        // frmsizecod 0 and 1 both = 32 kbps; 20 and 21 both = 192 kbps;
        // 36 and 37 both = 640 kbps (the Table 5.18 endpoints).
        assert_eq!(FrameSizeCode::Valid(0).nominal_bitrate_kbps(), Some(32));
        assert_eq!(FrameSizeCode::Valid(1).nominal_bitrate_kbps(), Some(32));
        assert_eq!(FrameSizeCode::Valid(20).nominal_bitrate_kbps(), Some(192));
        assert_eq!(FrameSizeCode::Valid(21).nominal_bitrate_kbps(), Some(192));
        assert_eq!(FrameSizeCode::Valid(37).nominal_bitrate_kbps(), Some(640));
        assert_eq!(FrameSizeCode::Reserved.nominal_bitrate_kbps(), None);
    }

    /// `words` / `frame_length_bytes` return the per-rate Table 5.18
    /// values for a valid code and `None` when either this code or the
    /// rate code is reserved.
    #[test]
    fn frame_size_code_words_and_bytes_per_rate() {
        // frmsizecod = 20 (192 kbps): 576 words @ 32 kHz, 417 @ 44.1,
        // 384 @ 48 — verbatim from Table 5.18.
        let fsc = FrameSizeCode::Valid(20);
        assert_eq!(fsc.words(SampleRateCode::ThirtyTwoKHz), Some(576));
        assert_eq!(fsc.words(SampleRateCode::FortyFourPointOneKHz), Some(417));
        assert_eq!(fsc.words(SampleRateCode::FortyEightKHz), Some(384));
        assert_eq!(
            fsc.frame_length_bytes(SampleRateCode::FortyEightKHz),
            Some(768)
        );
        // Reserved rate → None even on a valid frame-size code.
        assert_eq!(fsc.words(SampleRateCode::Reserved), None);
        assert_eq!(fsc.frame_length_bytes(SampleRateCode::Reserved), None);
        // Reserved frame-size code → None on every rate.
        assert_eq!(
            FrameSizeCode::Reserved.words(SampleRateCode::FortyEightKHz),
            None
        );
        assert_eq!(
            FrameSizeCode::Reserved.frame_length_bytes(SampleRateCode::FortyEightKHz),
            None
        );
    }

    /// `SyncInfo::frame_size_code()` agrees with the pre-resolved
    /// `frame_length` field on every frame `parse` accepts — the typed
    /// surface's `frame_length_bytes(rate)` matches `si.frame_length`.
    #[test]
    fn syncinfo_frame_size_code_matches_resolved_length() {
        // fscod=0 (48 kHz) / frmsizecod=20 → 768 bytes.
        let buf = [0x0B, 0x77, 0xAB, 0xCD, 20];
        let si = parse(&buf).unwrap();
        let fsc = si.frame_size_code();
        assert_eq!(fsc, FrameSizeCode::Valid(20));
        assert_eq!(fsc.raw(), Some(si.frmsizecod));
        assert!(!fsc.is_reserved());
        assert_eq!(
            fsc.frame_length_bytes(si.sample_rate_code()),
            Some(si.frame_length)
        );
        assert_eq!(fsc.nominal_bitrate_kbps(), Some(192));

        // fscod=2 (32 kHz) / frmsizecod=0 → 96 words = 192 bytes.
        let buf = [0x0B, 0x77, 0, 0, 0x80];
        let si = parse(&buf).unwrap();
        let fsc = si.frame_size_code();
        assert_eq!(fsc, FrameSizeCode::Valid(0));
        assert_eq!(
            fsc.frame_length_bytes(si.sample_rate_code()),
            Some(si.frame_length)
        );
    }

    /// A caller constructing a `SyncInfo` by hand with an out-of-range
    /// `frmsizecod = 40` still gets the `FrameSizeCode::Reserved` typed
    /// surface — `parse` itself never lets a reserved frmsizecod
    /// through, but the typed accessor on a hand-built `SyncInfo` flags
    /// it.
    #[test]
    fn syncinfo_frame_size_code_surfaces_reserved_for_hand_built() {
        let si = SyncInfo {
            crc1: 0,
            fscod: 0,
            frmsizecod: 40,
            sample_rate: 48_000,
            frame_length: 0,
        };
        let fsc = si.frame_size_code();
        assert_eq!(fsc, FrameSizeCode::Reserved);
        assert!(fsc.is_reserved());
        assert_eq!(fsc.raw(), None);
        assert_eq!(fsc.nominal_bitrate_kbps(), None);
        assert_eq!(fsc.frame_length_bytes(SampleRateCode::FortyEightKHz), None);
    }
}

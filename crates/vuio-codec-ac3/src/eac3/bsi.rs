//! E-AC-3 (Annex E) Bit Stream Information parser — `bsi()` per
//! §E.2.2.2 / Table E1.2.
//!
//! Unlike base AC-3, the E-AC-3 syncinfo is just a 16-bit syncword
//! (`0x0B77`) — no `crc1`, no `fscod`, no `frmsizecod`. The
//! sample-rate and frame-size codes have moved into the BSI itself,
//! reordered, and joined by a stream-type tag, substream id, and a
//! variable number-of-blocks code (1, 2, 3, or 6 audio blocks per
//! syncframe instead of AC-3's hard-coded 6).
//!
//! This module parses the **entire** Table E1.2 BSI bit-by-bit,
//! including the optional `mixmdate`, `infomdate`, and `addbsi`
//! chains. Fields that the round-1 decoder does not act on are still
//! consumed exactly so the bit cursor lands on byte/bit position
//! `start + bits_consumed = start of audfrm()`.
//!
//! ## Bit-stream order (Table E1.2 verbatim)
//!
//! ```text
//!   strmtyp       2
//!   substreamid   3
//!   frmsiz       11        // size in 16-bit words minus one
//!   fscod         2
//!   if (fscod == 0x3) {
//!     fscod2      2         // numblkscod implicit = 0x3 (6 blocks)
//!   } else {
//!     numblkscod  2
//!   }
//!   acmod         3
//!   lfeon         1
//!   bsid          5         // ≥ 11 for E-AC-3 (16 = canonical)
//!   dialnorm      5
//!   compre        1
//!   if (compre)        compr        8
//!   if (acmod == 0) {
//!     dialnorm2   5
//!     compr2e     1
//!     if (compr2e)     compr2       8
//!   }
//!   if (strmtyp == 0x1) {           // dependent substream
//!     chanmape    1
//!     if (chanmape)    chanmap     16
//!   }
//!   mixmdate      1
//!   if (mixmdate)         /* parses 0..200 bits per Table E1.2 */
//!   infomdate     1
//!   if (infomdate)        /* parses 0..50 bits  per Table E1.2 */
//!   addbsie       1
//!   if (addbsie)     addbsil 6, addbsi (addbsil+1)*8 bits
//! ```
//!
//! Field semantics are described per §E.2.3.1.x in the spec PDF.

use oxideav_core::bits::BitReader;
use oxideav_core::{Error, Result};

use crate::bsi::{
    AdConverterType, AdditionalBitStreamInfo, AnnexDMixLevels, AudioProductionInfo,
    CompressionGain, CopyrightInfo, DialNorm, DolbyHeadphoneMode, DolbySurroundExMode,
    DolbySurroundMode, RoomType, StereoDownmixPreference,
};
use crate::tables::acmod_nfchans;

/// Largest `bsid` value still served by the base AC-3 parser. Streams
/// at `bsid` 11..=16 use the E-AC-3 (Annex E) syntax.
pub const BSID_BASE_AC3_MAX: u8 = 10;

/// Canonical Annex E stream identification value (§E.2.3.1.6, "10000").
/// Backwards-compatible variants 11..15 share the same syntax.
pub const EAC3_BSID: u8 = 16;

/// Stream type — Table E2.1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamType {
    /// Type 0: independent substream (or sole independent stream).
    Independent,
    /// Type 1: dependent substream (refers back to the immediately
    /// preceding independent substream).
    Dependent,
    /// Type 2: an AC-3 bit-stream wrapped inside an E-AC-3 sync layer
    /// (§E.2.3.1.1 "may not have any dependent substreams associated").
    Ac3Convert,
    /// Type 3: reserved.
    Reserved,
}

impl StreamType {
    fn from_u8(v: u8) -> Self {
        match v & 0x3 {
            0 => StreamType::Independent,
            1 => StreamType::Dependent,
            2 => StreamType::Ac3Convert,
            _ => StreamType::Reserved,
        }
    }

    /// Raw 2-bit value the parser read.
    pub fn raw(self) -> u8 {
        match self {
            StreamType::Independent => 0,
            StreamType::Dependent => 1,
            StreamType::Ac3Convert => 2,
            StreamType::Reserved => 3,
        }
    }
}

/// §E.2.3.1.12-17 program scale factor — the 6-bit gain word an
/// independent substream's mixing-metadata block can attach to its
/// own program (`pgmscl`, §E.2.3.1.13), to the second program of a
/// 1+1 dual-mono stream (`pgmscl2`, §E.2.3.1.15), or to an
/// *external* program carried in a different bit stream /
/// independent substream (`extpgmscl`, §E.2.3.1.17 — "this field
/// shall use the same scale as pgmscl").
///
/// Wire scale per §E.2.3.1.13: the value `0` shall be interpreted
/// as **mute**, and the values `1..=63` as a scale factor of
/// `-50 dB` to `+12 dB` in 1 dB steps — i.e.
/// `decibels() == code - 51`, with the `51` codepoint at 0 dB
/// (unity). When the corresponding exists-flag (`pgmscle` /
/// `pgmscl2e` / `extpgmscle`) is `0`, "the program scale factor
/// shall be 0 dB (no scaling)" per §E.2.3.1.12/.14/.16 — the BSI
/// fields represent that absent state as `None`.
///
/// Per §E.3.10.1-2 the gain is applied "during the mixing process"
/// of a dual-decoder main + associated-service mixer (`pgmscl`
/// attenuates the program carried in the *same* substream;
/// `extpgmscl` attenuates the program carried in a *different*
/// bit stream or independent substream). The single-stream decode
/// path is unchanged — this is pure surfaced metadata for a
/// downstream §E.3.10 mixer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramScaleFactor(u8);

impl ProgramScaleFactor {
    /// Wraps a 6-bit wire codepoint. The upper two bits of `code`
    /// are ignored so a caller can pass a wider word verbatim.
    pub fn from_code(code: u8) -> Self {
        Self(code & 0x3F)
    }

    /// The raw 6-bit codepoint (`0..=63`) for bit-stream round-trip.
    pub fn raw(self) -> u8 {
        self.0
    }

    /// `true` for the `0` codepoint — §E.2.3.1.13 "the value 0 shall
    /// be interpreted as mute".
    pub fn is_mute(self) -> bool {
        self.0 == 0
    }

    /// The scale factor in dB: `Some(code - 51)` spanning `-50..=+12`
    /// for the codepoints `1..=63`; `None` for the mute codepoint,
    /// which has no finite dB value.
    pub fn decibels(self) -> Option<i8> {
        if self.0 == 0 {
            None
        } else {
            Some(self.0 as i8 - 51)
        }
    }

    /// Linear amplitude scale a §E.3.10 mixer multiplies the program
    /// by: `0.0` for the mute codepoint, otherwise `10^(dB / 20)`.
    pub fn linear(self) -> f32 {
        match self.decibels() {
            None => 0.0,
            Some(db) => 10f32.powf(f32::from(db) / 20.0),
        }
    }
}

/// §E.2.3.1.53-58 pan information — the 8-bit mean-direction index
/// (`panmean`, §E.2.3.1.54) plus the 6-bit reserved trailer
/// (`paninfo`, §E.2.3.1.55) an independent mono / 1+1 dual-mono
/// substream's mixing-metadata block can attach so a §E.3.10.8
/// dual-decoder mixer pans the mono associated-audio program across
/// the channels of the main audio service.
///
/// Wire scale per §E.2.3.1.54: index `0` points the panned virtual
/// source toward the **center** speaker location (defined as 0
/// degrees); each step is 1.5 degrees of clockwise rotation, so
/// indices `0..=239` span `0..=358.5` degrees while `240..=255` are
/// reserved. When the exists-flag (`paninfoe` / `paninfo2e`) is `0`
/// "the pan position word is defaulted to center" per
/// §E.2.3.1.53/.56 — the BSI fields represent that absent state as
/// `None` ([`PanInfo::CENTER`] is the equivalent explicit value).
///
/// §E.3.10.8 derives per-output-channel scale factors for the
/// associated program from the index — [`Self::stereo_scale_factors`]
/// implements the stereo-output table and
/// [`Self::surround_scale_factors`] the 5.1-output pair of tables
/// (captioned Tables E3.15-E3.17; the §E.3.10.8 prose cites them
/// off-by-one as E3.16-E3.18 — the captions are followed here). The
/// single-stream decode path is unchanged — this is pure surfaced
/// metadata for a downstream §E.3.10 mixer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanInfo {
    panmean: u8,
    reserved: u8,
}

impl PanInfo {
    /// The §E.2.3.1.53/.56 default when the exists-flag is `0`: pan
    /// position "center" (index 0 → the center speaker location).
    pub const CENTER: Self = Self {
        panmean: 0,
        reserved: 0,
    };

    /// Wraps the raw wire fields: the 8-bit `panmean` index and the
    /// 6-bit reserved `paninfo` word (upper bits of `reserved` are
    /// masked so a caller can pass a wider word verbatim).
    pub fn from_fields(panmean: u8, reserved: u8) -> Self {
        Self {
            panmean,
            reserved: reserved & 0x3F,
        }
    }

    /// The raw 8-bit `panmean` index (`0..=255`) for bit-stream
    /// round-trip.
    pub fn panmean(self) -> u8 {
        self.panmean
    }

    /// The raw 6-bit reserved `paninfo` field (§E.2.3.1.55 "reserved
    /// for future mixing applications"), preserved verbatim for
    /// bit-stream round-trip.
    pub fn reserved(self) -> u8 {
        self.reserved
    }

    /// `true` for the reserved index codepoints `240..=255`
    /// (§E.2.3.1.54 "values 240 to 255 are reserved").
    pub fn is_reserved_index(self) -> bool {
        self.panmean >= 240
    }

    /// Mean angle of rotation relative to the center position,
    /// clockwise, in degrees: `Some(panmean × 1.5)` spanning
    /// `0.0..=358.5` for indices `0..=239`; `None` for the reserved
    /// indices.
    pub fn degrees(self) -> Option<f32> {
        if self.is_reserved_index() {
            None
        } else {
            Some(f32::from(self.panmean) * 1.5)
        }
    }

    /// Table E3.15 — associated-audio scale factors `(AL, AR)` for
    /// stereo-output panning: the mono associated program is split
    /// into two channels mixed with the main service's Left / Right
    /// respectively. `None` for the reserved indices. Every
    /// non-reserved index is power-preserving (`AL² + AR² == 1`).
    pub fn stereo_scale_factors(self) -> Option<(f32, f32)> {
        use std::f32::consts::FRAC_PI_2;
        let p = f32::from(self.panmean);
        match self.panmean {
            0..=19 => {
                let a = FRAC_PI_2 * ((p + 20.0) / 40.0);
                Some((a.cos(), a.sin()))
            }
            20..=99 => Some((0.0, 1.0)),
            100..=139 => {
                let a = FRAC_PI_2 * ((p - 100.0) / 40.0);
                Some((a.sin(), a.cos()))
            }
            140..=219 => Some((1.0, 0.0)),
            220..=239 => {
                let a = FRAC_PI_2 * ((p - 220.0) / 40.0);
                Some((a.cos(), a.sin()))
            }
            _ => None,
        }
    }

    /// Tables E3.16 + E3.17 — associated-audio scale factors
    /// `[AL, AC, AR, ALS, ARS]` for 5.1-channel-output panning: the
    /// mono associated program is split into five channels mixed
    /// with the main service's Left / Center / Right / Left Surround
    /// / Right Surround respectively (the LFE channel is not
    /// included per §E.3.10.8). `None` for the reserved indices.
    /// Every non-reserved index is power-preserving (the five
    /// squares sum to 1 — each range is a single sin/cos pair across
    /// two adjacent speakers).
    pub fn surround_scale_factors(self) -> Option<[f32; 5]> {
        use std::f32::consts::FRAC_PI_2;
        let p = f32::from(self.panmean);
        match self.panmean {
            0..=19 => {
                let a = FRAC_PI_2 * (p / 20.0);
                Some([0.0, a.cos(), a.sin(), 0.0, 0.0])
            }
            20..=72 => {
                let a = FRAC_PI_2 * ((p - 20.0) / 53.0);
                Some([0.0, 0.0, a.cos(), 0.0, a.sin()])
            }
            73..=166 => {
                let a = FRAC_PI_2 * ((p - 73.0) / 94.0);
                Some([0.0, 0.0, 0.0, a.sin(), a.cos()])
            }
            167..=219 => {
                let a = FRAC_PI_2 * ((p - 167.0) / 53.0);
                Some([a.sin(), 0.0, 0.0, a.cos(), 0.0])
            }
            220..=239 => {
                let a = FRAC_PI_2 * ((p - 220.0) / 20.0);
                Some([a.cos(), a.sin(), 0.0, 0.0, 0.0])
            }
            _ => None,
        }
    }
}

/// §E.2.3.1.19-21 premix-compression control — the three fields the
/// `mixdef == 0x1` ("mixing option 2") body of an independent
/// substream's mixing-metadata block carries to steer a §E.3.10
/// dual-decoder mixer's *premix compression* process (the dynamic-
/// range / gain adjustment applied to the main audio service before
/// the associated program is mixed in):
///
/// * `premixcmpsel` (§E.2.3.1.19, 1 bit) — *premix compression word
///   select*. `false` ⇒ the `dynrng` field is used in the premix
///   compression process; `true` ⇒ the `compr` field is used.
/// * `drcsrc` (§E.2.3.1.20, 1 bit) — *dynamic-range-control word
///   source*. `false` ⇒ the `dynrng` / `compr` fields of the
///   **external** program (the one in a separate bit stream /
///   independent substream) control the mix; `true` ⇒ the fields of
///   the **current** substream are used. Recommended `false`.
/// * `premixcmpscl` (§E.2.3.1.21, 3 bits) — *premix compression word
///   scale factor*. Table E2.7 maps the code to a compression
///   gain-reduction ratio applied before the main-service / external
///   mix. Recommended `0b000` (no compression).
///
/// Per §E.2.3.1.21 all three fields "shall be present in the
/// bitstream. However they should be set to the recommended values,
/// as decoders are not required to use them." The single-stream
/// decode path is unchanged — this is pure surfaced metadata for a
/// downstream §E.3.10 mixer. The same three fields also appear inside
/// the `mixdef == 0x3` ("mixing option 4") `mixdata` body; this
/// surface covers the `mixdef == 0x1` carriage only (the variable
/// option-4 body stays an opaque skip).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PremixCompression {
    premixcmpsel: bool,
    drcsrc: bool,
    premixcmpscl: u8,
}

/// The compression word a §E.3.10 premix process draws on, selected
/// by `premixcmpsel` (§E.2.3.1.19).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PremixCompressionWord {
    /// `premixcmpsel == 0` — the §5.4.3.x `dynrng` dynamic-range word.
    DynRng,
    /// `premixcmpsel == 1` — the §5.4.2.10 `compr` heavy-compression
    /// word.
    Compr,
}

/// Which program's dynamic-range-control words drive the mix, selected
/// by `drcsrc` (§E.2.3.1.20).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrcSource {
    /// `drcsrc == 0` — the **external** program's `dynrng` / `compr`
    /// fields (the recommended setting).
    ExternalProgram,
    /// `drcsrc == 1` — the **current** substream's `dynrng` / `compr`
    /// fields.
    CurrentSubstream,
}

impl PremixCompression {
    /// Wraps the raw `mixdef == 0x1` body: `premixcmpsel` (1 bit),
    /// `drcsrc` (1 bit), and the 3-bit `premixcmpscl` code (upper bits
    /// masked so a caller can pass a wider word verbatim).
    pub fn from_fields(premixcmpsel: bool, drcsrc: bool, premixcmpscl: u8) -> Self {
        Self {
            premixcmpsel,
            drcsrc,
            premixcmpscl: premixcmpscl & 0x7,
        }
    }

    /// Raw `premixcmpsel` bit (§E.2.3.1.19) for bit-stream round-trip.
    pub fn premixcmpsel(self) -> bool {
        self.premixcmpsel
    }

    /// Raw `drcsrc` bit (§E.2.3.1.20) for bit-stream round-trip.
    pub fn drcsrc(self) -> bool {
        self.drcsrc
    }

    /// Raw 3-bit `premixcmpscl` code (§E.2.3.1.21) for bit-stream
    /// round-trip.
    pub fn premixcmpscl(self) -> u8 {
        self.premixcmpscl
    }

    /// Typed view of `premixcmpsel` — which compression word the
    /// premix process uses (§E.2.3.1.19).
    pub fn compression_word(self) -> PremixCompressionWord {
        if self.premixcmpsel {
            PremixCompressionWord::Compr
        } else {
            PremixCompressionWord::DynRng
        }
    }

    /// Typed view of `drcsrc` — which program's DRC words drive the
    /// mix (§E.2.3.1.20).
    pub fn drc_source(self) -> DrcSource {
        if self.drcsrc {
            DrcSource::CurrentSubstream
        } else {
            DrcSource::ExternalProgram
        }
    }

    /// `true` for the `0b110` `premixcmpscl` codepoint, which has no
    /// row in Table E2.7 (the table lists `000,001,010,011,100,101,111`
    /// only). Reserved / undefined; [`Self::scale_ratio`] returns
    /// `None` for it.
    pub fn is_premixcmpscl_reserved(self) -> bool {
        self.premixcmpscl == 0b110
    }

    /// Table E2.7 compression gain-reduction ratio for `premixcmpscl`:
    /// the seven listed codes map to `code / 6` (`0b000` ⇒ 0% / no
    /// compression, ascending in `1/6` steps to `0b111` ⇒ 100% /
    /// maximum compression). `None` for the unlisted `0b110`
    /// codepoint. The spec captions the column "Scale Factor" with
    /// percentage values (16.7%, 33.3%, …) that are exactly the
    /// `n/6` ratios rounded to one decimal.
    pub fn scale_ratio(self) -> Option<f32> {
        let sixth = match self.premixcmpscl {
            0b000 => 0,
            0b001 => 1,
            0b010 => 2,
            0b011 => 3,
            0b100 => 4,
            0b101 => 5,
            0b111 => 6,
            _ => return None, // 0b110: not in Table E2.7
        };
        Some(sixth as f32 / 6.0)
    }

    /// `true` for the recommended default configuration
    /// (§E.2.3.1.19-21): `premixcmpsel == 0`, `drcsrc == 0`,
    /// `premixcmpscl == 0b000` (no compression). An encoder writing a
    /// `mixdef == 0x1` block "should" set this; a decoder seeing a
    /// non-default configuration is the signal that the encoder
    /// actually intends premix compression to take effect.
    pub fn is_recommended_default(self) -> bool {
        !self.premixcmpsel && !self.drcsrc && self.premixcmpscl == 0
    }
}

/// Parsed E-AC-3 BSI — the subset actually needed by the round-1
/// decoder + dispatcher. Fields not surfaced are still parsed (the
/// bit cursor walk has to land at the start of `audfrm()`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bsi {
    pub strmtyp: StreamType,
    pub substreamid: u8,
    /// `frmsiz` raw value. Frame size in bytes = `(frmsiz + 1) * 2`.
    pub frmsiz: u16,
    /// `fscod` raw value (2 bits). 0x3 indicates a reduced-rate stream
    /// (use `fscod2` for the actual rate).
    pub fscod: u8,
    /// `fscod2` (2 bits). Only valid when `fscod == 0x3`; 0xFF
    /// otherwise.
    pub fscod2: u8,
    /// Sample rate in Hz, derived from `(fscod, fscod2)`.
    pub sample_rate: u32,
    /// `numblkscod` raw value (2 bits). 0x3 = 6 blocks. 0/1/2 = 1/2/3
    /// blocks. When `fscod == 0x3` it is implicitly 0x3.
    pub numblkscod: u8,
    /// Number of audio blocks per syncframe (= 256 PCM samples each).
    /// Derived from `numblkscod`. Always 6 on reduced-rate streams.
    pub num_blocks: u8,
    /// AC-3 audio coding mode (Table 5.8, §5.4.2.3) — channel layout.
    pub acmod: u8,
    /// Number of full-bandwidth channels (`acmod_nfchans(acmod)`).
    pub nfchans: u8,
    /// Whether the LFE channel is coded in this substream.
    pub lfeon: bool,
    /// Total channel count = `nfchans + lfeon`.
    pub nchans: u8,
    /// `bsid` (5 bits). 16 = canonical Annex E; 11..15 = backward-
    /// compatible Annex E variants.
    pub bsid: u8,
    /// Dialogue normalization, 1..=31 dB below reference. 0 in the
    /// stream is reserved → mapped to 31 per §5.4.2.8 (Annex E reuses
    /// the base spec semantics). For a typed surface exposing the
    /// §7.6 reproduction-gain derivation, use
    /// [`Self::dialogue_normalization`].
    pub dialnorm: u8,
    /// §5.4.2.16 dialogue normalization for Ch2 in 1+1 dual-mono
    /// Annex E streams (`acmod == 0`). `None` outside `acmod == 0`.
    /// Same post-remap `1..=31` semantics as
    /// [`Self::dialnorm`] (the `0` wire codepoint maps to `31`).
    ///
    /// For the typed surface, see
    /// [`Self::dialogue_normalization_ch2`].
    pub dialnorm_ch2: Option<u8>,
    /// `chanmap` field (16 bits, dependent substream only). `None`
    /// when `strmtyp != Dependent` or `chanmape == 0`.
    ///
    /// Per Table E2.5, bit *i* (counted from the field's MSB → bit 0
    /// = MSB) flags channel location *i* — see the table for the
    /// label assignment.
    pub chanmap: Option<u16>,
    /// Annex E mixmdata mix levels (Table E1.2 §E.1.2.2). `Some` when
    /// `mixmdate == 1` AND the per-channel-presence guards in
    /// [`parse_mixing_metadata`] fire (3 front channels for the LtRt/LoRo
    /// **center** codes, a surround channel for the **surround** codes);
    /// fields that the guard skips read back as `0xFF` so callers can
    /// distinguish "spec-absent" from a legitimate 0b000 code.
    ///
    /// The 3-bit codewords map to linear gains via the same Tables
    /// D2.3-D2.6 used by base AC-3's Annex D xbsi1 — Annex E §E.2.3.1.3-6
    /// states "the value of [field] is the same as defined for AC-3
    /// in Annex D, §2.3.1.3 [-6]". Reuse of [`AnnexDMixLevels`] keeps a
    /// single source of truth.
    pub annex_e_mix_levels: Option<AnnexDMixLevels>,
    /// Annex E mixmdata preferred-stereo-downmix advisory (`dmixmod`,
    /// 2 bits) — Table E1.2 §E.1.2.2 reuses Annex D §2.3.1.2 semantics
    /// (`00` = not indicated, `01` = LtRt preferred, `10` = LoRo
    /// preferred, `11` = reserved). `0xFF` when `mixmdate == 0` or the
    /// `acmod > 2` guard fires.
    pub dmixmod: u8,
    /// Annex E mixmdata preferred stereo downmix mode (Table E1.2
    /// §E.1.2.2 reusing Annex D §2.3.1.2 / Table D2.2), surfaced as a
    /// typed [`StereoDownmixPreference`]. `Some` only when
    /// `mixmdate == 1` AND `acmod > 2`; `None` otherwise (the
    /// per-Table-E1.2 guard skips the 2-bit slot for mono / 2/0
    /// streams, and a `mixmdate == 0` syncframe skips the entire
    /// mixing-metadata block). Equivalent to the typed view of
    /// [`Bsi::dmixmod`] where the `0xFF` "absent" sentinel becomes
    /// `None`; the raw field stays authoritative for bit-stream
    /// round-trip and the typed surface is a thin convenience over
    /// it. Lets a §3.1.1 auto-mode two-channel-downmix router pick
    /// LtRt vs LoRo without consulting a magic-number sentinel —
    /// shared with the base-syntax [`crate::bsi::Bsi::dmixmod_preference`]
    /// so a single chain consumer can handle both syntaxes.
    pub dmixmod_preference: Option<StereoDownmixPreference>,
    /// Annex E mixmdata LFE mix level (`lfemixlevcod`, 5 bits, §E.1.2.2).
    /// `Some` when `lfeon == 1`, `mixmdate == 1`, and `lfemixlevcode == 1`;
    /// `None` otherwise. The 5-bit code is **not** consulted by the
    /// round-129 downmix (LFE stays muted per §7.8) but is surfaced so
    /// downstream tooling and a future LFE-into-stereo bass-route can
    /// honour it without re-parsing the BSI.
    pub lfemixlevcod: Option<u8>,
    /// §E.2.3.1.12-13 program scale factor (`pgmscl`) — the gain a
    /// §E.3.10 dual-decoder mixer applies to the program carried in
    /// *this* substream while mixing it with an associated service.
    /// `Some` only when `mixmdate == 1`, the substream is independent
    /// (Table E1.2 emits the `pgmscle` chain under
    /// `strmtyp == 0x0` only), AND `pgmscle == 1`; `None` otherwise —
    /// per §E.2.3.1.12 the absent state means "0 dB (no scaling)".
    /// See [`ProgramScaleFactor`] for the mute / `-50..=+12 dB` wire
    /// scale. The single-stream decode path does not consult it.
    pub pgmscl: Option<ProgramScaleFactor>,
    /// §E.2.3.1.14-15 program scale factor #2 (`pgmscl2`) — same
    /// scale as [`Bsi::pgmscl`] but applying to the second audio
    /// channel when `acmod` indicates two independent channels (1+1
    /// dual mono). `Some` only when `mixmdate == 1`, the substream is
    /// independent, `acmod == 0`, AND `pgmscl2e == 1`.
    pub pgmscl2: Option<ProgramScaleFactor>,
    /// §E.2.3.1.16-17 external program scale factor (`extpgmscl`) —
    /// the gain a §E.3.10 mixer applies to an *external* program (one
    /// carried in a separate bit stream or independent substream from
    /// the one carrying this instance). Same wire scale as
    /// [`Bsi::pgmscl`] per §E.2.3.1.17. `Some` only when
    /// `mixmdate == 1`, the substream is independent, AND
    /// `extpgmscle == 1`.
    pub extpgmscl: Option<ProgramScaleFactor>,
    /// §E.2.3.1.53-55 pan information (`panmean` + the reserved
    /// `paninfo` trailer) — the mean-direction index a §E.3.10.8
    /// mixer uses to pan this mono associated-audio program across
    /// the channels of the main audio service. `Some` only when
    /// `mixmdate == 1`, the substream is independent (Table E1.2
    /// emits the chain under `strmtyp == 0x0` only),
    /// `acmod < 0x2` (mono or 1+1 dual-mono source), AND
    /// `paninfoe == 1`; `None` otherwise — per §E.2.3.1.53 the
    /// absent state defaults the pan position word to "center"
    /// ([`PanInfo::CENTER`]). See [`PanInfo`] for the 1.5-degree
    /// index scale and the Table E3.15-E3.17 output scale factors.
    /// The single-stream decode path does not consult it.
    pub paninfo: Option<PanInfo>,
    /// §E.2.3.1.56-58 pan information #2 (`panmean2` + `paninfo2`)
    /// — same meaning as [`Bsi::paninfo`] but applying to the second
    /// audio channel when `acmod` indicates two independent channels
    /// (1+1 dual mono). `Some` only when `mixmdate == 1`, the
    /// substream is independent, `acmod == 0`, AND `paninfo2e == 1`.
    pub paninfo2: Option<PanInfo>,
    /// §E.2.3.1.19-21 premix-compression control (`premixcmpsel` /
    /// `drcsrc` / `premixcmpscl`) — the three fields that steer a
    /// §E.3.10 mixer's premix compression process. `Some` only when
    /// `mixmdate == 1`, the substream is independent, AND the
    /// mixing-metadata block selects `mixdef == 0x1` ("mixing option
    /// 2"); `None` otherwise (a `mixdef ∈ {0, 2, 3}` block does not
    /// carry the three fields as a standalone 5-bit group, and a
    /// `mixmdate == 0` syncframe skips the whole block). See
    /// [`PremixCompression`] for the field semantics + Table E2.7
    /// scale lookup. The single-stream decode path does not consult
    /// it.
    pub premix_compression: Option<PremixCompression>,
    /// Heavy compression gain word (`compr`, §E.2.3.1.x / §5.4.2.10 +
    /// §7.7.2.2 reused per Annex E). Identical semantics + wire format
    /// to base AC-3 — see [`CompressionGain`] for the X/Y decode. For
    /// 1+1 dual-mono (`acmod == 0`) this is the Ch1 word; Ch2 is
    /// surfaced separately as [`Bsi::compr_ch2`]. `Some` when
    /// `compre == 1`; `None` otherwise.
    pub compr: Option<CompressionGain>,
    /// Ch2 heavy compression gain word for 1+1 dual-mono only. `None`
    /// outside `acmod == 0`, or inside `acmod == 0` when `compr2e == 0`.
    pub compr_ch2: Option<CompressionGain>,
    /// Bit-stream mode (`bsmod`, 3 bits, Table 5.7 semantics). In the
    /// Annex E syntax this word lives inside the informational-metadata
    /// block, so it is `Some` only when `infomdate == 1`; `None` means
    /// "not transmitted" (treat as `0` = complete main).
    pub bsmod: Option<u8>,
    /// Dolby Surround EX mode (§E.2.3.1.x informational metadata, gated
    /// by `infomdate==1` AND `acmod >= 6`). Carries the same semantics
    /// as Annex D §2.3.1.8 / Table D2.7 — see
    /// [`crate::bsi::DolbySurroundExMode`]. `None` when the informational
    /// metadata block was absent or when `acmod < 6` (no stereo
    /// surround pair to drive the EX matrix).
    pub dsurexmod: Option<DolbySurroundExMode>,
    /// Dolby Headphone mode (§E.2.3.1.x informational metadata, gated
    /// by `infomdate==1` AND `acmod == 2`). Same semantics as Annex D
    /// §2.3.1.9 / Table D2.8 — see
    /// [`crate::bsi::DolbyHeadphoneMode`]. `None` when the
    /// informational metadata block was absent or when `acmod != 2`.
    pub dheadphonmod: Option<DolbyHeadphoneMode>,
    /// Base-syntax Dolby Surround mode reused inside the Annex E
    /// informational-metadata block. `Some` only when `infomdate == 1`
    /// AND `acmod == 2` (2/0 stereo — the only channel layout that
    /// carries the codeword on the wire per Table 5.11 / §5.4.2.6,
    /// reused verbatim by Annex E §E.2.3.1.x); `None` otherwise. Same
    /// semantics as [`crate::bsi::Bsi::dolby_surround_mode`] — see
    /// [`crate::bsi::DolbySurroundMode`] for the typed surface. Single
    /// source of truth across base + Annex E so a chain consumer can
    /// route both syntaxes through one branch on
    /// [`Bsi::dolby_surround_mode`].
    pub dolby_surround_mode: Option<DolbySurroundMode>,
    /// A/D converter type for the Ch1 audio production (§E.2.3.1.x
    /// informational metadata, gated by `infomdate==1` AND
    /// `audprodie==1`). Same semantics as Annex D §2.3.1.10 / Table
    /// D2.9 — see [`crate::bsi::AdConverterType`]. `None` when the
    /// audio-production block was absent.
    pub adconvtyp: Option<AdConverterType>,
    /// A/D converter type for the Ch2 audio production in 1+1
    /// dual-mono (`acmod == 0` AND `audprodi2e == 1`). `None` outside
    /// 1+1 mode or when `audprodi2e == 0`.
    pub adconvtyp_ch2: Option<AdConverterType>,
    /// §E.2.3.1.x audio production information (`mixlevel` + `roomtyp`)
    /// for the main channel, gated by `infomdate == 1` AND
    /// `audprodie == 1`. Same semantics as base AC-3 §5.4.2.13-15 — see
    /// [`AudioProductionInfo`]. `None` when the informational metadata
    /// block was absent or when the encoder did not emit the production
    /// chain.
    pub audio_production: Option<AudioProductionInfo>,
    /// §E.2.3.1.x Ch2 audio production information for 1+1 dual-mono
    /// streams (`acmod == 0` AND `audprodi2e == 1`). `None` outside
    /// 1+1 mode or when the Ch2 production chain was absent.
    pub audio_production_ch2: Option<AudioProductionInfo>,
    /// §E.2.3.1.62-65 distribution-control hint pair — same
    /// `copyrightb` (§5.4.2.24) + `origbs` (§5.4.2.25) semantics as
    /// base AC-3, sitting inside the informational-metadata block
    /// gated by `infomdate == 1`. `None` when the encoder set
    /// `infomdate == 0`. The decoder does not act on either bit; a
    /// chain consumer can enforce a distribution / archival policy
    /// without re-parsing the BSI. See [`crate::bsi::CopyrightInfo`]
    /// for the typed surface.
    pub copyright_info: Option<CopyrightInfo>,
    /// §5.4.2.29-31 additional bit-stream information payload reused
    /// verbatim by Annex E (Table E1.2 closes the BSI walk with
    /// `addbsie + addbsil + addbsi` exactly as base AC-3 does at
    /// §5.3.2). `Some` when `addbsie == 1`; `None` when the encoder
    /// did not emit the trailer. Same semantics as
    /// [`crate::bsi::Bsi::addbsi`] — see [`AdditionalBitStreamInfo`].
    pub addbsi: Option<AdditionalBitStreamInfo>,
    /// Frame size in bytes — `(frmsiz + 1) * 2`. Cached so the
    /// dispatcher can range-check the packet without re-doing
    /// arithmetic.
    pub frame_bytes: u32,
    /// Total number of bits the parser consumed out of the input
    /// slice. Callers seek the audfrm parser to exactly this offset.
    pub bits_consumed: u64,
}

impl Bsi {
    /// Typed view over [`Bsi::dialnorm`] per §5.4.2.8 (reused by
    /// Annex E). Identical surface to
    /// [`crate::bsi::Bsi::dialogue_normalization`] — see that
    /// accessor for the §7.6 reproduction-gain derivation.
    pub fn dialogue_normalization(&self) -> DialNorm {
        DialNorm::from_wire(self.dialnorm)
    }

    /// Typed view over [`Bsi::dialnorm_ch2`] per §5.4.2.16 — the
    /// Ch2 mirror in Annex E 1+1 dual-mono streams. `None` outside
    /// `acmod == 0`.
    pub fn dialogue_normalization_ch2(&self) -> Option<DialNorm> {
        self.dialnorm_ch2.map(DialNorm::from_wire)
    }

    /// Typed view over [`Bsi::dmixmod_preference`] — the Annex E
    /// mixmdata preferred stereo downmix mode (Table E1.2 §E.1.2.2
    /// reusing Annex D §2.3.1.2 / Table D2.2). `Some` only when
    /// `mixmdate == 1` AND `acmod > 2`; `None` otherwise. Identical
    /// surface to [`crate::bsi::Bsi::stereo_downmix_preference`] so
    /// a §3.1.1 auto-mode two-channel-downmix router can be shared
    /// between the base AC-3 and Annex E paths.
    pub fn stereo_downmix_preference(&self) -> Option<StereoDownmixPreference> {
        self.dmixmod_preference
    }

    /// Typed view over [`Bsi::dolby_surround_mode`] — the §E.2.3.1.x
    /// Dolby Surround mode reused verbatim from base AC-3 Table 5.11.
    /// `Some` only when `infomdate == 1` AND `acmod == 2` (2/0 stereo
    /// — the only channel layout that carries the codeword on the
    /// wire); `None` otherwise. Identical surface to
    /// [`crate::bsi::Bsi::dolby_surround_mode`] so a chain consumer
    /// can route both base and Annex E syntaxes through one branch.
    pub fn dolby_surround_mode(&self) -> Option<DolbySurroundMode> {
        self.dolby_surround_mode
    }
}

/// Parse the E-AC-3 BSI starting at byte 0 of `data` (the byte *just
/// after* the 16-bit syncword — i.e. the third byte of the syncframe).
///
/// Returns `Err(Error::Invalid)` for reserved / illegal field
/// combinations (`fscod2 == 0x3`, `bsid == 9 || bsid == 10` per
/// §E.2.3.1.6, or a malformed `addbsi` length).
pub fn parse(data: &[u8]) -> Result<Bsi> {
    let mut br = BitReader::new(data);
    parse_with(&mut br)
}

/// Variant that reads from an externally-managed [`BitReader`] so a
/// caller already positioned past the syncword can share its cursor.
pub fn parse_with(br: &mut BitReader<'_>) -> Result<Bsi> {
    let start_bits = br.bit_position();

    // §E.2.3.1.1
    let strmtyp_raw = br.read_u32(2)? as u8;
    let strmtyp = StreamType::from_u8(strmtyp_raw);
    if matches!(strmtyp, StreamType::Reserved) {
        return Err(Error::invalid("eac3 bsi: strmtyp '11' is reserved"));
    }

    // §E.2.3.1.2
    let substreamid = br.read_u32(3)? as u8;

    // §E.2.3.1.3 — 11-bit value, frame_size_in_words = frmsiz + 1.
    let frmsiz = br.read_u32(11)? as u16;
    let frame_words = (frmsiz as u32) + 1;
    let frame_bytes = frame_words * 2;
    if !(64..=4096).contains(&frame_bytes) {
        // Spec note in §E.2.3.1.3: "values at the lower end of this
        // range do not occur as they do not represent enough words to
        // convey a complete syncframe". We still accept anything that
        // can plausibly fit a syncinfo + bsi + crc2; downstream sanity
        // checks reject runts.
        if frame_bytes < 8 {
            return Err(Error::invalid(format!(
                "eac3 bsi: frmsiz {frmsiz} → frame {frame_bytes} bytes is too small"
            )));
        }
    }

    // §E.2.3.1.4
    let fscod = br.read_u32(2)? as u8;
    // §E.2.3.1.5 — fscod2 OR numblkscod
    let (fscod2, numblkscod) = if fscod == 0x3 {
        let f2 = br.read_u32(2)? as u8;
        if f2 == 0x3 {
            return Err(Error::invalid(
                "eac3 bsi: fscod2 '11' is reserved (Table E2.3)",
            ));
        }
        // numblkscod is implicitly 0x3 (six blocks per syncframe) when
        // fscod indicates a reduced-rate stream.
        (f2, 0x3u8)
    } else {
        (0xFFu8, br.read_u32(2)? as u8)
    };
    let num_blocks = match numblkscod {
        0 => 1u8,
        1 => 2,
        2 => 3,
        _ => 6,
    };
    let sample_rate = match (fscod, fscod2) {
        (0, _) => 48_000,
        (1, _) => 44_100,
        (2, _) => 32_000,
        (3, 0) => 24_000,
        (3, 1) => 22_050,
        (3, 2) => 16_000,
        _ => unreachable!("fscod/fscod2 combos covered above"),
    };

    // §E.2.3.1.x acmod / lfeon
    let acmod = br.read_u32(3)? as u8;
    let lfeon = br.read_u32(1)? != 0;
    let nfchans = acmod_nfchans(acmod);
    let nchans = nfchans + u8::from(lfeon);

    // §E.2.3.1.6
    let bsid = br.read_u32(5)? as u8;
    if bsid == 9 || bsid == 10 || bsid > 16 {
        return Err(Error::Unsupported(format!(
            "eac3 bsi: bsid {bsid} is reserved/illegal per §E.2.3.1.6"
        )));
    }
    if bsid <= BSID_BASE_AC3_MAX {
        // Caller should have routed this packet to the base AC-3
        // parser (which itself handles bsid ≤ 8 + the tolerated 9..=10
        // safety margin we permit elsewhere). Surfacing a clear error
        // protects against double-dispatch bugs in the decoder loop.
        return Err(Error::Unsupported(format!(
            "eac3 bsi: bsid {bsid} routes through the base AC-3 parser, not Annex E"
        )));
    }

    // §5.4.2.8 (reused) — dialnorm 0 maps to 31.
    let dialnorm_raw = br.read_u32(5)? as u8;
    let dialnorm = if dialnorm_raw == 0 { 31 } else { dialnorm_raw };

    let compre = br.read_u32(1)? != 0;
    let compr = if compre {
        Some(CompressionGain::from_byte(br.read_u32(8)? as u8))
    } else {
        None
    };

    // 1+1 dual-mono (acmod == 0): second copy of dialnorm + compr.
    let (dialnorm_ch2, compr_ch2) = if acmod == 0 {
        // §5.4.2.16 (reused by Annex E) — dialnorm2 has the same
        // meaning as dialnorm; the `0` codepoint is reserved and
        // remaps to `31` per §5.4.2.8.
        let dialnorm2_raw = br.read_u32(5)? as u8;
        let dialnorm2 = if dialnorm2_raw == 0 {
            31
        } else {
            dialnorm2_raw
        };
        let compr2e = br.read_u32(1)? != 0;
        let c2 = if compr2e {
            Some(CompressionGain::from_byte(br.read_u32(8)? as u8))
        } else {
            None
        };
        (Some(dialnorm2), c2)
    } else {
        (None, None)
    };

    // §E.2.3.1.7-8 — chanmape / chanmap, dependent substream only.
    let chanmap = if matches!(strmtyp, StreamType::Dependent) {
        let chanmape = br.read_u32(1)? != 0;
        if chanmape {
            Some(br.read_u32(16)? as u16)
        } else {
            None
        }
    } else {
        None
    };

    // §E.2.3.1.9-61 — mixing meta-data block.
    let mixmdate = br.read_u32(1)? != 0;
    let MixingMetadata {
        annex_e_mix_levels,
        dmixmod,
        dmixmod_preference,
        lfemixlevcod,
        pgmscl,
        pgmscl2,
        extpgmscl,
        paninfo,
        paninfo2,
        premix_compression,
    } = if mixmdate {
        parse_mixing_metadata(br, acmod, lfeon, strmtyp, numblkscod)?
    } else {
        MixingMetadata::ABSENT
    };

    // §E.2.3.1.62 ff — informational meta-data.
    let infomdate = br.read_u32(1)? != 0;
    let (
        bsmod,
        dsurexmod,
        dheadphonmod,
        dolby_surround_mode,
        adconvtyp,
        adconvtyp_ch2,
        audio_production,
        audio_production_ch2,
        copyright_info,
    ) = if infomdate {
        let info = parse_informational_metadata(br, acmod, fscod, strmtyp, numblkscod)?;
        (
            Some(info.bsmod),
            info.dsurexmod,
            info.dheadphonmod,
            info.dolby_surround_mode,
            info.adconvtyp,
            info.adconvtyp_ch2,
            info.audio_production,
            info.audio_production_ch2,
            Some(info.copyright_info),
        )
    } else {
        (None, None, None, None, None, None, None, None, None)
    };

    // addbsi — §5.4.2.29-31 trailer of 1..=64 encoder-defined bytes
    // (Table E1.2 reuses base AC-3's syntax). Per §5.4.2.30 the
    // decoder PCM path does not consult the payload, but it is
    // surfaced verbatim for chain consumers (encoder-private
    // metadata, OAMD packetisation) and the cursor is advanced
    // exactly `7 + 8 × (addbsil + 1)` bits.
    let addbsie = br.read_u32(1)? != 0;
    let addbsi = if addbsie {
        let addbsil = br.read_u32(6)? as u8;
        let nbytes = addbsil as usize + 1;
        let mut payload = Vec::with_capacity(nbytes);
        for _ in 0..nbytes {
            payload.push(br.read_u32(8)? as u8);
        }
        AdditionalBitStreamInfo::from_addbsil_and_payload(addbsil, payload)
    } else {
        None
    };

    let bits_consumed = br.bit_position() - start_bits;

    Ok(Bsi {
        strmtyp,
        substreamid,
        frmsiz,
        fscod,
        fscod2,
        sample_rate,
        numblkscod,
        num_blocks,
        acmod,
        nfchans,
        lfeon,
        nchans,
        bsid,
        dialnorm,
        dialnorm_ch2,
        chanmap,
        annex_e_mix_levels,
        dmixmod,
        dmixmod_preference,
        lfemixlevcod,
        pgmscl,
        pgmscl2,
        extpgmscl,
        paninfo,
        paninfo2,
        premix_compression,
        compr,
        compr_ch2,
        bsmod,
        dsurexmod,
        dheadphonmod,
        dolby_surround_mode,
        adconvtyp,
        adconvtyp_ch2,
        audio_production,
        audio_production_ch2,
        copyright_info,
        addbsi,
        frame_bytes,
        bits_consumed,
    })
}

/// Fields the §E.2.3.1.9-61 mixing-metadata walk surfaces to the
/// public [`Bsi`]. Per the spec's guards (Table E1.2):
///  * `dmixmod` is only present when `acmod > 2` (more than 2 channels).
///  * `ltrtcmixlev` / `lorocmixlev` only when 3 front channels exist
///    (`acmod & 0x1 != 0 && acmod > 2`).
///  * `ltrtsurmixlev` / `lorosurmixlev` only when a surround channel
///    exists (`acmod & 0x4 != 0`).
///  * `lfemixlevcod` only when `lfeon && lfemixlevcode == 1`.
///  * `pgmscl` / `pgmscl2` / `extpgmscl` only on independent
///    substreams (`strmtyp == 0x0`) when the respective exists-flag
///    is set (`pgmscl2` additionally requires `acmod == 0`).
///  * `paninfo` / `paninfo2` only on independent substreams with
///    `acmod < 0x2` (mono or 1+1 dual-mono source) when the
///    respective exists-flag is set (`paninfo2` additionally
///    requires `acmod == 0`).
///
/// Codewords whose guards fail read back as `0xFF` inside
/// [`AnnexDMixLevels`] so callers can distinguish "spec-absent" from a
/// legitimate `0b000` (1.414×) code. The `annex_e_mix_levels` `Option`
/// itself is `None` only when none of the four center/surround fields
/// were present (mono / 2/0 stereo with no surrounds) — those layouts
/// have no downmix to refine.
struct MixingMetadata {
    annex_e_mix_levels: Option<AnnexDMixLevels>,
    dmixmod: u8,
    dmixmod_preference: Option<StereoDownmixPreference>,
    lfemixlevcod: Option<u8>,
    pgmscl: Option<ProgramScaleFactor>,
    pgmscl2: Option<ProgramScaleFactor>,
    extpgmscl: Option<ProgramScaleFactor>,
    paninfo: Option<PanInfo>,
    paninfo2: Option<PanInfo>,
    premix_compression: Option<PremixCompression>,
}

impl MixingMetadata {
    /// The `mixmdate == 0` state — every surfaced field absent (the
    /// program scale factors default to "0 dB, no scaling" per
    /// §E.2.3.1.12/.14/.16, and the pan position words default to
    /// "center" per §E.2.3.1.53/.56, all represented as `None`).
    const ABSENT: Self = Self {
        annex_e_mix_levels: None,
        dmixmod: 0xFF,
        dmixmod_preference: None,
        lfemixlevcod: None,
        pgmscl: None,
        pgmscl2: None,
        extpgmscl: None,
        paninfo: None,
        paninfo2: None,
        premix_compression: None,
    };
}

fn parse_mixing_metadata(
    br: &mut BitReader<'_>,
    acmod: u8,
    lfeon: bool,
    strmtyp: StreamType,
    numblkscod: u8,
) -> Result<MixingMetadata> {
    // §E.2.3.1 mixing metadata — Table E1.2.
    // dmixmod (2) when acmod > 0x2 (more than 2 channels).
    let (dmixmod, dmixmod_preference) = if acmod > 0x2 {
        let raw = br.read_u32(2)? as u8;
        (raw, Some(StereoDownmixPreference::from_code(raw)))
    } else {
        (0xFFu8, None)
    };
    // ltrtcmixlev (3) + lorocmixlev (3) when 3 front channels exist.
    let (ltrtcmixlev, lorocmixlev) = if (acmod & 0x1) != 0 && acmod > 0x2 {
        (br.read_u32(3)? as u8, br.read_u32(3)? as u8)
    } else {
        (0xFFu8, 0xFFu8)
    };
    // ltrtsurmixlev (3) + lorosurmixlev (3) when a surround channel exists.
    let (ltrtsurmixlev, lorosurmixlev) = if (acmod & 0x4) != 0 {
        (br.read_u32(3)? as u8, br.read_u32(3)? as u8)
    } else {
        (0xFFu8, 0xFFu8)
    };
    let annex_e_mix_levels = if ltrtcmixlev != 0xFF
        || lorocmixlev != 0xFF
        || ltrtsurmixlev != 0xFF
        || lorosurmixlev != 0xFF
    {
        Some(AnnexDMixLevels {
            ltrtcmixlev,
            ltrtsurmixlev,
            lorocmixlev,
            lorosurmixlev,
        })
    } else {
        None
    };
    // lfemixlevcode (1) + lfemixlevcod (5) when LFE on.
    let lfemixlevcod = if lfeon {
        let lfemixlevcode = br.read_u32(1)? != 0;
        if lfemixlevcode {
            Some(br.read_u32(5)? as u8)
        } else {
            None
        }
    } else {
        None
    };
    // strmtyp == 0x0 (independent) emits pgmscle/pgmscl + extpgmscle/
    // extpgmscl + mixdef + (mixdef-dependent body).
    let mut pgmscl = None;
    let mut pgmscl2 = None;
    let mut extpgmscl = None;
    let mut paninfo = None;
    let mut paninfo2 = None;
    let mut premix_compression = None;
    if matches!(strmtyp, StreamType::Independent) {
        // §E.2.3.1.12-13 — program scale factor for this substream's
        // own program. Absent ⇒ 0 dB (no scaling).
        let pgmscle = br.read_u32(1)? != 0;
        if pgmscle {
            pgmscl = Some(ProgramScaleFactor::from_code(br.read_u32(6)? as u8));
        }
        if acmod == 0 {
            // §E.2.3.1.14-15 — same meaning as pgmscl, applied to the
            // second channel of a 1+1 dual-mono program.
            let pgmscl2e = br.read_u32(1)? != 0;
            if pgmscl2e {
                pgmscl2 = Some(ProgramScaleFactor::from_code(br.read_u32(6)? as u8));
            }
        }
        // §E.2.3.1.16-17 — scale factor for an *external* program
        // (carried in a different bit stream / independent substream),
        // same wire scale as pgmscl.
        let extpgmscle = br.read_u32(1)? != 0;
        if extpgmscle {
            extpgmscl = Some(ProgramScaleFactor::from_code(br.read_u32(6)? as u8));
        }
        let mixdef = br.read_u32(2)?;
        match mixdef {
            0 => { /* no additional bits */ }
            1 => {
                // §E.2.3.1.19-21 — premixcmpsel(1) + drcsrc(1) +
                // premixcmpscl(3) = 5 bits ("mixing option 2").
                let premixcmpsel = br.read_u32(1)? != 0;
                let drcsrc = br.read_u32(1)? != 0;
                let premixcmpscl = br.read_u32(3)? as u8;
                premix_compression = Some(PremixCompression::from_fields(
                    premixcmpsel,
                    drcsrc,
                    premixcmpscl,
                ));
            }
            2 => {
                // mixdata = 12 bits (Table E1.2 — "mixing option 3, 12 bits reserved").
                let _ = br.read_u32(12)?;
            }
            _ => {
                // mixdef == 3: variable-length mixing parameter block.
                // mixdeflen(5), mixdata2e(1), if mixdata2e {…}, mixdata3e(1),
                // if mixdata3e {…}, mixdata field (8*(mixdeflen+2) - num_mixdata_bits),
                // mixdatafill (0..7 bits to round to a byte).
                let mixdeflen = br.read_u32(5)?;
                let mut bits_used: u32 = 5; // mixdeflen itself
                let mixdata2e = br.read_u32(1)? != 0;
                bits_used += 1;
                if mixdata2e {
                    bits_used += parse_mixdata2_block(br, acmod, lfeon)?;
                }
                let mixdata3e = br.read_u32(1)? != 0;
                bits_used += 1;
                if mixdata3e {
                    bits_used += parse_mixdata3_block(br)?;
                }
                let mixdata_bits_total = 8 * (mixdeflen + 2);
                if bits_used >= mixdata_bits_total {
                    // Spec note: bits_used must be ≤ mixdata_bits_total.
                    // If we've already consumed more than the budget,
                    // the bit stream is malformed — bail.
                    return Err(Error::invalid(format!(
                        "eac3 bsi: mixdata overrun (used {bits_used} bits, budget {mixdata_bits_total})"
                    )));
                }
                let pad = mixdata_bits_total - bits_used;
                br.skip(pad)?;
                // mixdatafill rounds the field to a whole byte. After
                // the fixed 8*(mixdeflen+2) bits the field is byte-aligned
                // by construction; nothing more to do.
            }
        }
        // §E.2.3.1.53-58 — paninfoe / panmean / paninfo (+ the #2
        // copies in 1+1 dual mono) — only when acmod < 2 (a §E.3.10.8
        // mixer pans a *mono* associated program across the main
        // service's channels). Note the Table E1.2 row prints the
        // 6-bit reserved field as "paninfoe"; the §E.2.3.1.55 heading
        // names it `paninfo`.
        if acmod < 0x2 {
            let paninfoe = br.read_u32(1)? != 0;
            if paninfoe {
                let panmean = br.read_u32(8)? as u8;
                let reserved = br.read_u32(6)? as u8;
                paninfo = Some(PanInfo::from_fields(panmean, reserved));
            }
            if acmod == 0 {
                let paninfo2e = br.read_u32(1)? != 0;
                if paninfo2e {
                    let panmean2 = br.read_u32(8)? as u8;
                    let reserved2 = br.read_u32(6)? as u8;
                    paninfo2 = Some(PanInfo::from_fields(panmean2, reserved2));
                }
            }
        }
        // frmmixcfginfoe — and per-block blkmixcfginfo.
        let frmmixcfginfoe = br.read_u32(1)? != 0;
        if frmmixcfginfoe {
            if numblkscod == 0 {
                let _blkmixcfginfo0 = br.read_u32(5)?;
            } else {
                let nblks = match numblkscod {
                    1 => 2u32,
                    2 => 3,
                    _ => 6,
                };
                for _ in 0..nblks {
                    let blkmixcfginfoe = br.read_u32(1)? != 0;
                    if blkmixcfginfoe {
                        let _blkmixcfginfo = br.read_u32(5)?;
                    }
                }
            }
        }
    }
    Ok(MixingMetadata {
        annex_e_mix_levels,
        dmixmod,
        dmixmod_preference,
        lfemixlevcod,
        pgmscl,
        pgmscl2,
        extpgmscl,
        paninfo,
        paninfo2,
        premix_compression,
    })
}

/// Parses the body of `mixdata2e` (mixing option 4 with extra channel
/// scale factors). Returns the number of bits consumed.
fn parse_mixdata2_block(br: &mut BitReader<'_>, _acmod: u8, _lfeon: bool) -> Result<u32> {
    let mut bits = 0u32;
    // premixcmpsel(1) + drcsrc(1) + premixcmpscl(3) = 5 bits.
    let _ = br.read_u32(5)?;
    bits += 5;
    // For each of L/C/R/Ls/Rs/LFE: presence(1) + (if set) scale(4) = 5 bits.
    // Plus dmixscle(1) + (if set) dmixscl(4).
    // Plus addche(1) + (if set) extpgmaux1scle(1)+(...4) + extpgmaux2scle(1)+(...4).
    for _ in 0..6 {
        let p = br.read_u32(1)? != 0;
        bits += 1;
        if p {
            let _ = br.read_u32(4)?;
            bits += 4;
        }
    }
    let dmixscle = br.read_u32(1)? != 0;
    bits += 1;
    if dmixscle {
        let _ = br.read_u32(4)?;
        bits += 4;
    }
    let addche = br.read_u32(1)? != 0;
    bits += 1;
    if addche {
        let p1 = br.read_u32(1)? != 0;
        bits += 1;
        if p1 {
            let _ = br.read_u32(4)?;
            bits += 4;
        }
        let p2 = br.read_u32(1)? != 0;
        bits += 1;
        if p2 {
            let _ = br.read_u32(4)?;
            bits += 4;
        }
    }
    Ok(bits)
}

/// Parses the body of `mixdata3e` (speech enhancement processing).
/// Returns the number of bits consumed.
fn parse_mixdata3_block(br: &mut BitReader<'_>) -> Result<u32> {
    let mut bits = 0u32;
    // spchdat(5) + addspchdate(1) + (if set) spchdat1(5) + spchan1att(2) +
    //   addspchdat1e(1) + (if set) spchdat2(5) + spchan2att(3).
    let _ = br.read_u32(5)?;
    bits += 5;
    let addspchdate = br.read_u32(1)? != 0;
    bits += 1;
    if addspchdate {
        let _ = br.read_u32(5)?;
        let _ = br.read_u32(2)?;
        bits += 7;
        let addspchdat1e = br.read_u32(1)? != 0;
        bits += 1;
        if addspchdat1e {
            let _ = br.read_u32(5)?;
            let _ = br.read_u32(3)?;
            bits += 8;
        }
    }
    Ok(bits)
}

/// Decoded informational metadata fields surfaced to the public BSI.
/// Layout mirrors §E.2.3.1.62 ff one-for-one — every field is `None`
/// when the spec's per-acmod / per-audprodie guard kept its codepoint
/// off the wire.
struct InformationalMetadata {
    bsmod: u8,
    dsurexmod: Option<DolbySurroundExMode>,
    dheadphonmod: Option<DolbyHeadphoneMode>,
    dolby_surround_mode: Option<DolbySurroundMode>,
    adconvtyp: Option<AdConverterType>,
    adconvtyp_ch2: Option<AdConverterType>,
    audio_production: Option<AudioProductionInfo>,
    audio_production_ch2: Option<AudioProductionInfo>,
    copyright_info: CopyrightInfo,
}

/// Walk the §E.2.3.1.62 ff informational metadata block. The body is
/// the same structural shape as base AC-3's `bsmod`/`copyrightb`/
/// `origbs`/`audprodie` chain plus a few Annex E additions
/// (`sourcefscod`, `convsync`, `blkid`/`frmsizecod` for AC-3-converted
/// streams).
///
/// Surfaces `dsurexmod` (§E.2.3.1.x, acmod ∈ {6, 7} guard),
/// `dheadphonmod` (acmod == 2 guard), per-channel `adconvtyp` /
/// `adconvtyp_ch2` (inside the `audprodie` / `audprodi2e` chain), and
/// the §5.4.2.13-15 audio-production info (`mixlevel` + `roomtyp`,
/// reused verbatim per §E.2.3.1.x) for the main channel and the Ch2
/// 1+1 dual-mono mirror. The remaining service-metadata fields
/// (`bsmod` is parsed in the body `Bsi`, source fscod, conv sync,
/// AC-3-convert blkid / frmsizecod) are still parsed bit-accurately
/// and discarded — they do not drive playback policy.
fn parse_informational_metadata(
    br: &mut BitReader<'_>,
    acmod: u8,
    fscod: u8,
    strmtyp: StreamType,
    numblkscod: u8,
) -> Result<InformationalMetadata> {
    let bsmod = br.read_u32(3)? as u8;
    let copyrightb = br.read_u32(1)? != 0;
    let origbs = br.read_u32(1)? != 0;
    let copyright_info = CopyrightInfo::from_bits(copyrightb, origbs);
    let (dolby_surround_mode, dheadphonmod) = if acmod == 0x2 {
        let dsm_raw = br.read_u32(2)? as u8;
        let dhpm_raw = br.read_u32(2)? as u8;
        (
            Some(DolbySurroundMode::from_code(dsm_raw)),
            Some(DolbyHeadphoneMode::from_code(dhpm_raw)),
        )
    } else {
        (None, None)
    };
    let dsurexmod = if acmod >= 0x6 {
        let dsex_raw = br.read_u32(2)? as u8;
        Some(DolbySurroundExMode::from_code(dsex_raw))
    } else {
        None
    };
    let audprodie = br.read_u32(1)? != 0;
    let (audio_production, adconvtyp) = if audprodie {
        let mixlevel = br.read_u32(5)? as u8;
        let roomtyp_raw = br.read_u32(2)? as u8;
        let adcv_raw = br.read_u32(1)? as u8;
        (
            Some(AudioProductionInfo {
                mixlevel,
                roomtyp: RoomType::from_code(roomtyp_raw),
            }),
            Some(AdConverterType::from_code(adcv_raw)),
        )
    } else {
        (None, None)
    };
    let (audio_production_ch2, adconvtyp_ch2) = if acmod == 0 {
        let audprodi2e = br.read_u32(1)? != 0;
        if audprodi2e {
            let mixlevel2 = br.read_u32(5)? as u8;
            let roomtyp2_raw = br.read_u32(2)? as u8;
            let adcv2_raw = br.read_u32(1)? as u8;
            (
                Some(AudioProductionInfo {
                    mixlevel: mixlevel2,
                    roomtyp: RoomType::from_code(roomtyp2_raw),
                }),
                Some(AdConverterType::from_code(adcv2_raw)),
            )
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };
    if fscod < 0x3 {
        let _sourcefscod = br.read_u32(1)?;
    }
    // convsync is present only for indep substream (strmtyp == 0) when
    // numblkscod != 0x3 (i.e. fewer than 6 blocks per syncframe).
    if matches!(strmtyp, StreamType::Independent) && numblkscod != 0x3 {
        let _convsync = br.read_u32(1)?;
    }
    // strmtyp == 0x2 → AC-3 wrapped in E-AC-3 syncframe; carries
    // `blkid` (only if numblkscod != 0x3) + `frmsizecod` (6 bits).
    if matches!(strmtyp, StreamType::Ac3Convert) {
        if numblkscod != 0x3 {
            let _blkid = br.read_u32(1)?;
        }
        let _frmsizecod = br.read_u32(6)?;
    }
    Ok(InformationalMetadata {
        bsmod,
        dsurexmod,
        dheadphonmod,
        dolby_surround_mode,
        adconvtyp,
        adconvtyp_ch2,
        audio_production,
        audio_production_ch2,
        copyright_info,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper — pack a sequence of (n_bits, value) pairs MSB-first
    /// into a fresh byte buffer, padded with zeros to the next byte
    /// boundary.
    fn pack_msb(bits: &[(u32, u32)]) -> (Vec<u8>, u64) {
        let total: u32 = bits.iter().map(|(n, _)| *n).sum();
        let nbytes = total.div_ceil(8);
        let mut out = vec![0u8; nbytes as usize];
        let mut bitpos = 0u32;
        for &(n, v) in bits {
            for i in (0..n).rev() {
                let bit = ((v >> i) & 1) as u8;
                let byte = (bitpos / 8) as usize;
                let shift = 7 - (bitpos % 8);
                out[byte] |= bit << shift;
                bitpos += 1;
            }
        }
        (out, total as u64)
    }

    /// Independent substream, 2/0 stereo, 48 kHz, 6 blocks, 768 byte
    /// frame. dialnorm=27, no compr, no chanmape (indep), no mixmdate,
    /// no infomdate, no addbsi. Mirrors the validator-encoded fixture
    /// `eac3-stereo-48000-192kbps`.
    #[test]
    fn parses_192kbps_indep_stereo() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = 0
            (3, 0),    // substreamid = 0
            (11, 383), // frmsiz = 383 → 768 bytes
            (2, 0),    // fscod = 0 → 48 kHz
            (2, 3),    // numblkscod = 3 → 6 blocks
            (3, 2),    // acmod = 2 (2/0)
            (1, 0),    // lfeon
            (5, 16),   // bsid = 16
            (5, 27),   // dialnorm = 27 → -27 dB
            (1, 0),    // compre
            (1, 0),    // mixmdate
            (1, 0),    // infomdate
            (1, 0),    // addbsie
        ];
        let (buf, total_bits) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert_eq!(bsi.strmtyp, StreamType::Independent);
        assert_eq!(bsi.substreamid, 0);
        assert_eq!(bsi.frmsiz, 383);
        assert_eq!(bsi.frame_bytes, 768);
        assert_eq!(bsi.sample_rate, 48_000);
        assert_eq!(bsi.num_blocks, 6);
        assert_eq!(bsi.acmod, 2);
        assert_eq!(bsi.nfchans, 2);
        assert_eq!(bsi.nchans, 2);
        assert!(!bsi.lfeon);
        assert_eq!(bsi.bsid, 16);
        assert_eq!(bsi.dialnorm, 27);
        assert!(bsi.chanmap.is_none());
        assert_eq!(bsi.bits_consumed, total_bits);
    }

    /// Reduced-rate (24 kHz) variant — fscod=3, fscod2=0, numblkscod
    /// implicit at 6 blocks.
    #[test]
    fn parses_reduced_rate_24khz() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp
            (3, 0),    // substreamid
            (11, 100), // frmsiz
            (2, 3),    // fscod = 0x3 → reduced
            (2, 0),    // fscod2 = 0 → 24 kHz
            (3, 2),    // acmod = 2
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 31),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // mixmdate
            (1, 0),    // infomdate
            (1, 0),    // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert_eq!(bsi.fscod, 3);
        assert_eq!(bsi.fscod2, 0);
        assert_eq!(bsi.sample_rate, 24_000);
        assert_eq!(bsi.num_blocks, 6); // implicit numblkscod=3
        assert_eq!(bsi.numblkscod, 3);
    }

    /// 1-block-per-syncframe variant (`eac3-256-coeff-block` fixture
    /// shape).
    #[test]
    fn parses_one_block_per_syncframe() {
        let bits: &[(u32, u32)] = &[
            (2, 0),   // strmtyp
            (3, 0),   // substreamid
            (11, 50), // frmsiz
            (2, 0),   // fscod = 48 kHz
            (2, 0),   // numblkscod = 0 → 1 block
            (3, 2),   // acmod
            (1, 0),   // lfeon
            (5, 16),  // bsid
            (5, 31),  // dialnorm
            (1, 0),   // compre
            (1, 0),   // mixmdate
            (1, 0),   // infomdate
            (1, 0),   // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert_eq!(bsi.numblkscod, 0);
        assert_eq!(bsi.num_blocks, 1);
    }

    #[test]
    fn rejects_reserved_bsid_9_10() {
        for bad in [9u32, 10] {
            let bits: &[(u32, u32)] = &[
                (2, 0),
                (3, 0),
                (11, 100),
                (2, 0),
                (2, 3),
                (3, 2),
                (1, 0),
                (5, bad),
                (5, 27),
                (1, 0),
                (1, 0),
                (1, 0),
                (1, 0),
            ];
            let (buf, _) = pack_msb(bits);
            let r = parse(&buf);
            assert!(r.is_err(), "expected reject for bsid={bad}, got {r:?}");
        }
    }

    /// Dependent substream with chanmape=1 and chanmap = bit 6
    /// (Lrs/Rrs pair) — matches our 7.1 encoder output's dep payload.
    #[test]
    fn parses_dependent_substream_chanmap() {
        let chanmap_val = 1u32 << (15 - 6); // bit 6 (Table E2.5)
        let bits: &[(u32, u32)] = &[
            (2, 1),    // strmtyp = dependent
            (3, 0),    // substreamid = 0 (first dep)
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod = 3
            (3, 2),    // acmod = 2 (2 channels: Lb, Rb)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 1),    // chanmape = 1
            (16, chanmap_val),
            (1, 0), // mixmdate
            (1, 0), // infomdate
            (1, 0), // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert_eq!(bsi.strmtyp, StreamType::Dependent);
        assert_eq!(bsi.chanmap, Some(0x0200));
    }

    #[test]
    fn rejects_strmtyp_reserved() {
        let bits: &[(u32, u32)] = &[
            (2, 3), // strmtyp = '11' reserved
            (3, 0),
            (11, 100),
            (2, 0),
            (2, 3),
            (3, 2),
            (1, 0),
            (5, 16),
            (5, 27),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
        ];
        let (buf, _) = pack_msb(bits);
        assert!(parse(&buf).is_err());
    }

    #[test]
    fn rejects_fscod2_reserved() {
        let bits: &[(u32, u32)] = &[
            (2, 0),
            (3, 0),
            (11, 100),
            (2, 3), // fscod = 0x3 → reduced rate
            (2, 3), // fscod2 = 0x3 reserved
            (3, 2),
            (1, 0),
            (5, 16),
            (5, 27),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
        ];
        let (buf, _) = pack_msb(bits);
        assert!(parse(&buf).is_err());
    }

    /// 5.1 indep with `mixmdate == 1` and all four mix-level fields
    /// present (acmod=7 → 3 front + surround channels). Verifies the
    /// captured codewords match the bit-stream and that `dmixmod` is
    /// also surfaced.
    #[test]
    fn captures_mixmdata_5_1_full_mix_levels() {
        // dmixmod=01 (LtRt preferred), ltrtcmixlev=010 (1.000),
        // lorocmixlev=100 (0.707), ltrtsurmixlev=011 (0.841),
        // lorosurmixlev=101 (0.595), no LFE refinement, indep flag
        // off the rest of mixmdata.
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod = 3 → 6 blocks
            (3, 7),    // acmod = 7 (3/2)
            (1, 1),    // lfeon = 1
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 1),    // mixmdate
            // mixmdata body:
            (2, 1),  // dmixmod = 01 (LtRt preferred)
            (3, 2),  // ltrtcmixlev = 010 (1.000)
            (3, 4),  // lorocmixlev = 100 (0.707)
            (3, 3),  // ltrtsurmixlev = 011 (0.841)
            (3, 5),  // lorosurmixlev = 101 (0.595)
            (1, 1),  // lfemixlevcode = 1
            (5, 15), // lfemixlevcod = 15
            // indep substream extras (strmtyp == 0):
            (1, 0), // pgmscle = 0
            (1, 0), // extpgmscle = 0
            (2, 0), // mixdef = 0 (no extra)
            (1, 0), // frmmixcfginfoe = 0
            (1, 0), // infomdate = 0
            (1, 0), // addbsie = 0
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        let mix = bsi
            .annex_e_mix_levels
            .expect("mix levels should be surfaced when mixmdate==1 and acmod=7");
        assert_eq!(mix.ltrtcmixlev, 0b010);
        assert_eq!(mix.lorocmixlev, 0b100);
        assert_eq!(mix.ltrtsurmixlev, 0b011);
        assert_eq!(mix.lorosurmixlev, 0b101);
        assert_eq!(bsi.dmixmod, 0b01);
        assert_eq!(bsi.lfemixlevcod, Some(15));
    }

    /// 2/0 stereo indep with `mixmdate == 1` — none of the per-channel
    /// guards fire (no third front channel, no surround), so the
    /// `annex_e_mix_levels` accessor returns `None` even though the
    /// `mixmdate` flag was set. `dmixmod` is also absent (guarded by
    /// `acmod > 2`).
    #[test]
    fn mixmdate_on_stereo_yields_no_mix_levels() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod = 3
            (3, 2),    // acmod = 2 (2/0)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 1),    // mixmdate = 1
            // mixmdata body for 2/0 indep: no dmixmod, no ltrt/loro
            // codes, no LFE code. Just the indep tail:
            (1, 0), // pgmscle = 0
            (1, 0), // extpgmscle = 0
            (2, 0), // mixdef = 0
            (1, 0), // frmmixcfginfoe = 0
            (1, 0), // infomdate = 0
            (1, 0), // addbsie = 0
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert!(
            bsi.annex_e_mix_levels.is_none(),
            "2/0 stereo should not surface any mix-level codes"
        );
        assert_eq!(bsi.dmixmod, 0xFF);
        assert_eq!(bsi.lfemixlevcod, None);
    }

    /// No-mixmdata baseline — the four fields default to `None` and
    /// `dmixmod` / `lfemixlevcod` return the absent sentinels.
    #[test]
    fn no_mixmdate_yields_none() {
        let bits: &[(u32, u32)] = &[
            (2, 0),
            (3, 0),
            (11, 383),
            (2, 0),
            (2, 3),
            (3, 7), // acmod = 7
            (1, 1), // lfeon = 1
            (5, 16),
            (5, 27),
            (1, 0), // compre
            (1, 0), // mixmdate = 0
            (1, 0), // infomdate = 0
            (1, 0), // addbsie = 0
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert!(bsi.annex_e_mix_levels.is_none());
        assert_eq!(bsi.dmixmod, 0xFF);
        assert_eq!(bsi.lfemixlevcod, None);
    }

    /// 3/1 indep — surround codes present, center codes present, no
    /// dual surround. Verifies the partial-mix-levels case where only
    /// the four 3-bit codes are read (no LFE refinement).
    #[test]
    fn captures_mixmdata_3_1_no_lfe() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp
            (3, 0),    // substreamid
            (11, 200), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod = 3
            (3, 5),    // acmod = 5 (3/1)
            (1, 0),    // lfeon = 0
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 1),    // mixmdate
            // mixmdata body:
            (2, 2), // dmixmod = 10 (LoRo preferred)
            (3, 1), // ltrtcmixlev = 001 (1.189)
            (3, 2), // lorocmixlev = 010 (1.000)
            (3, 6), // ltrtsurmixlev = 110 (0.500)
            (3, 7), // lorosurmixlev = 111 (0.000 - silent surrounds)
            // no LFE bits (lfeon=0).
            // indep extras:
            (1, 0), // pgmscle = 0
            (1, 0), // extpgmscle = 0
            (2, 0), // mixdef = 0
            (1, 0), // frmmixcfginfoe = 0
            (1, 0), // infomdate = 0
            (1, 0), // addbsie = 0
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        let mix = bsi.annex_e_mix_levels.unwrap();
        assert_eq!(mix.ltrtcmixlev, 0b001);
        assert_eq!(mix.lorocmixlev, 0b010);
        assert_eq!(mix.ltrtsurmixlev, 0b110);
        assert_eq!(mix.lorosurmixlev, 0b111);
        assert_eq!(bsi.dmixmod, 0b10);
        assert_eq!(bsi.lfemixlevcod, None);
    }

    /// E-AC-3 `compre=1` surfaces a `CompressionGain` byte verbatim
    /// — the Annex E syntax reuses the base AC-3 §7.7.2.2 + Table 7.30
    /// semantics unchanged.
    #[test]
    fn parses_compr_when_compre_set() {
        // 2/0 indep stereo with compre=1, compr=0b0100_0001 (X=4, Y=1).
        // Linear = 2^5 * (16+1)/32 = 32 * 17/32 = 17.0; dB = 24.61 dB.
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod = 3
            (3, 2),    // acmod = 2
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 1),    // compre = 1
            (8, 0b0100_0001),
            (1, 0), // mixmdate
            (1, 0), // infomdate
            (1, 0), // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        let cg = bsi.compr.expect("compre=1");
        assert_eq!(cg.raw(), 0b0100_0001);
        assert_eq!(cg.x(), 4);
        assert_eq!(cg.y(), 1);
        assert!((cg.linear() - 17.0).abs() < 1e-5);
        // Ch2 word stays None outside acmod==0.
        assert!(bsi.compr_ch2.is_none());
    }

    /// `infomdate == 0` keeps the three Annex D playback hints at
    /// `None` even though the BSI is otherwise fully formed. Reuses
    /// the round-1 192 kbps stereo fixture shape — every existing
    /// fixture builder sets `infomdate=0`.
    #[test]
    fn no_infomdate_yields_no_playback_hints() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod
            (3, 2),    // acmod
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // mixmdate
            (1, 0),    // infomdate = 0
            (1, 0),    // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert!(bsi.dsurexmod.is_none());
        assert!(bsi.dheadphonmod.is_none());
        assert!(bsi.adconvtyp.is_none());
        assert!(bsi.adconvtyp_ch2.is_none());
    }

    /// 3/2 indep with `infomdate == 1` and `audprodie == 1` — the
    /// `dsurexmod` slot (acmod ≥ 6 gate fires) and the `adconvtyp`
    /// slot (inside the audprodie chain) both surface; `dheadphonmod`
    /// stays `None` because the acmod == 2 gate doesn't fire.
    /// `dsurexmod = 0b10` (Dolby Surround EX / PLIIx), `adconvtyp = 1`
    /// (HDCD).
    #[test]
    fn infomdate_surfaces_dsurexmod_and_adconvtyp_on_3_2() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = indep
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod = 0 (48 kHz)
            (2, 3),    // numblkscod = 3 (6 blocks → convsync absent)
            (3, 7),    // acmod = 7 (3/2)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // mixmdate
            (1, 1),    // infomdate = 1
            // informational metadata body:
            (3, 0), // bsmod
            (1, 0), // copyrightb
            (1, 0), // origbs
            // acmod != 2 → no dsurmod/dheadphonmod
            // acmod >= 6 → dsurexmod present
            (2, 0b10),    // dsurexmod = Surround EX / PLIIx
            (1, 1),       // audprodie = 1
            (5, 0b10101), // mixlevel
            (2, 0b10),    // roomtyp
            (1, 1),       // adconvtyp = 1 (HDCD)
            // acmod != 0 → no audprodi2e block
            (1, 0), // sourcefscod (fscod < 3)
            // strmtyp == Indep AND numblkscod == 3 → no convsync
            // strmtyp != Ac3Convert → no blkid/frmsizecod
            (1, 0), // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert_eq!(
            bsi.dsurexmod,
            Some(crate::bsi::DolbySurroundExMode::SurroundExOrProLogicIIx)
        );
        // acmod != 2 → dheadphonmod gate didn't fire.
        assert!(bsi.dheadphonmod.is_none());
        assert_eq!(bsi.adconvtyp, Some(crate::bsi::AdConverterType::Hdcd));
        assert!(bsi.adconvtyp_ch2.is_none());
        // §E.2.3.1.x reuses §5.4.2.13-15 audio production verbatim —
        // `audprodie == 1` surfaces (mixlevel=21 → 101 dB SPL,
        // roomtyp=SmallFlat) and the Ch2 mirror stays None outside
        // 1+1 mode.
        let ap = bsi
            .audio_production
            .expect("audprodie=1 should surface audio_production");
        assert_eq!(ap.mixlevel, 0b10101);
        assert_eq!(ap.peak_mix_level_db_spl(), 101);
        assert_eq!(ap.roomtyp, crate::bsi::RoomType::SmallFlat);
        assert!(bsi.audio_production_ch2.is_none());
    }

    /// 2/0 indep with `infomdate == 1` — the `dheadphonmod` slot
    /// (acmod == 2 gate fires) surfaces; `dsurexmod` and `adconvtyp`
    /// stay `None` because their respective gates do not fire
    /// (acmod < 6, audprodie == 0).
    #[test]
    fn infomdate_surfaces_dheadphonmod_on_2_0() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod = 3
            (3, 2),    // acmod = 2 (2/0)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // mixmdate
            (1, 1),    // infomdate = 1
            // info body:
            (3, 0),    // bsmod
            (1, 0),    // copyrightb
            (1, 0),    // origbs
            (2, 0b10), // dsurmod (table-D2-style, distinct from dsurexmod)
            (2, 0b10), // dheadphonmod = Encoded
            // acmod < 6 → no dsurexmod
            (1, 0), // audprodie = 0
            // acmod != 0 → no audprodi2e
            (1, 0), // sourcefscod
            // strmtyp == Indep && numblkscod == 3 → no convsync
            (1, 0), // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert!(bsi.dsurexmod.is_none());
        assert_eq!(
            bsi.dheadphonmod,
            Some(crate::bsi::DolbyHeadphoneMode::Encoded)
        );
        // The 2-bit `dsurmod` slot inside the acmod==2 branch surfaces
        // as a typed `dolby_surround_mode` per Table 5.11 (`0b10` =
        // Encoded). The fixture sets it alongside `dheadphonmod`.
        assert_eq!(
            bsi.dolby_surround_mode,
            Some(crate::bsi::DolbySurroundMode::Encoded)
        );
        assert!(bsi.adconvtyp.is_none());
        assert!(bsi.adconvtyp_ch2.is_none());
    }

    /// 2/0 indep with `infomdate == 1` — walk all four Table 5.11
    /// `dsurmod` codepoints through `parse()` and confirm the typed
    /// `dolby_surround_mode` field matches each. `dheadphonmod`
    /// surfaces alongside it (same acmod==2 gate fires both reads).
    #[test]
    fn infomdate_surfaces_dolby_surround_mode_all_codepoints_on_2_0() {
        use crate::bsi::DolbySurroundMode::*;
        let expected = [NotIndicated, NotEncoded, Encoded, Reserved];
        for code in 0u32..4 {
            let bits: &[(u32, u32)] = &[
                (2, 0),    // strmtyp
                (3, 0),    // substreamid
                (11, 383), // frmsiz
                (2, 0),    // fscod
                (2, 3),    // numblkscod = 3
                (3, 2),    // acmod = 2 (2/0)
                (1, 0),    // lfeon
                (5, 16),   // bsid
                (5, 27),   // dialnorm
                (1, 0),    // compre
                (1, 0),    // mixmdate
                (1, 1),    // infomdate = 1
                // info body:
                (3, 0),    // bsmod
                (1, 0),    // copyrightb
                (1, 0),    // origbs
                (2, code), // dsurmod walks 0..=3
                (2, 0),    // dheadphonmod = NotIndicated
                // acmod < 6 → no dsurexmod
                (1, 0), // audprodie = 0
                // acmod != 0 → no audprodi2e
                (1, 0), // sourcefscod
                // strmtyp == Indep && numblkscod == 3 → no convsync
                (1, 0), // addbsie
            ];
            let (buf, _) = pack_msb(bits);
            let bsi = parse(&buf).unwrap();
            assert_eq!(bsi.dolby_surround_mode, Some(expected[code as usize]));
            assert_eq!(bsi.dolby_surround_mode(), Some(expected[code as usize]));
        }
    }

    /// `acmod != 2` (here 3/2 with acmod=7) inside the
    /// informational-metadata block — the `dsurmod` slot is skipped
    /// per Table E1.2, so the typed `dolby_surround_mode` resolves to
    /// `None` even with `infomdate == 1`.
    #[test]
    fn infomdate_skips_dolby_surround_mode_when_acmod_not_2_0() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod = 3
            (3, 7),    // acmod = 7 (3/2)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // mixmdate
            (1, 1),    // infomdate = 1
            // info body:
            (3, 0), // bsmod
            (1, 0), // copyrightb
            (1, 0), // origbs
            // acmod != 2 → skip dsurmod + dheadphonmod
            // acmod >= 6 → consume dsurexmod
            (2, 0b00), // dsurexmod = NotIndicated
            (1, 0),    // audprodie = 0
            (1, 0),    // sourcefscod
            (1, 0),    // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert!(bsi.dolby_surround_mode.is_none());
        assert!(bsi.dolby_surround_mode().is_none());
        // dsurexmod gate fires though (acmod >= 6).
        assert_eq!(
            bsi.dsurexmod,
            Some(crate::bsi::DolbySurroundExMode::NotIndicated)
        );
    }

    /// `infomdate == 0` baseline — the entire informational metadata
    /// block is skipped, so `dolby_surround_mode` is `None` even on a
    /// 2/0 stream that would otherwise carry the codeword.
    #[test]
    fn infomdate_zero_leaves_dolby_surround_mode_none() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod = 3
            (3, 2),    // acmod = 2 (2/0)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // mixmdate
            (1, 0),    // infomdate = 0
            (1, 0),    // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert!(bsi.dolby_surround_mode.is_none());
    }

    /// 1+1 dual-mono indep with `infomdate == 1` and both
    /// `audprodie == 1` (Ch1) AND `audprodi2e == 1` (Ch2). Both
    /// `adconvtyp` (Ch1, HDCD) and `adconvtyp_ch2` (Ch2, Standard)
    /// surface independently. `dsurexmod` / `dheadphonmod` stay `None`
    /// because their acmod gates (≥6 and ==2 respectively) do not
    /// fire for acmod=0.
    #[test]
    fn infomdate_surfaces_per_channel_adconvtyp_in_dual_mono() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod = 3
            (3, 0),    // acmod = 0 (1+1)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm (Ch1)
            (1, 0),    // compre (Ch1) = 0
            // 1+1 second-block dialnorm/compr2
            (5, 27), // dialnorm2
            (1, 0),  // compr2e
            (1, 0),  // mixmdate
            (1, 1),  // infomdate = 1
            // info body:
            (3, 0), // bsmod
            (1, 0), // copyrightb
            (1, 0), // origbs
            // acmod != 2 → no dsurmod/dheadphonmod
            // acmod < 6 → no dsurexmod
            (1, 1),       // audprodie = 1
            (5, 0b10000), // mixlevel
            (2, 0b00),    // roomtyp
            (1, 1),       // adconvtyp = 1 (Hdcd)
            // acmod == 0 → audprodi2e block
            (1, 1),       // audprodi2e = 1
            (5, 0b00001), // mixlevel2
            (2, 0b11),    // roomtyp2
            (1, 0),       // adconvtyp2 = 0 (Standard)
            (1, 0),       // sourcefscod
            // strmtyp == Indep && numblkscod == 3 → no convsync
            (1, 0), // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert_eq!(bsi.acmod, 0);
        assert_eq!(bsi.adconvtyp, Some(crate::bsi::AdConverterType::Hdcd));
        assert_eq!(
            bsi.adconvtyp_ch2,
            Some(crate::bsi::AdConverterType::Standard)
        );
        assert!(bsi.dsurexmod.is_none());
        assert!(bsi.dheadphonmod.is_none());
        // §5.4.2.13-15 audio-production block decodes independently
        // for Ch1 (mixlevel=16 → 96 dB SPL, roomtyp=NotIndicated) and
        // Ch2 (mixlevel=1 → 81 dB SPL, roomtyp=Reserved). The 1+1
        // mirror is the canonical test for the audprodi2e chain.
        let ap1 = bsi
            .audio_production
            .expect("audprodie=1 should surface Ch1 audio_production");
        assert_eq!(ap1.mixlevel, 0b10000);
        assert_eq!(ap1.peak_mix_level_db_spl(), 96);
        assert_eq!(ap1.roomtyp, crate::bsi::RoomType::NotIndicated);
        let ap2 = bsi
            .audio_production_ch2
            .expect("audprodi2e=1 should surface Ch2 audio_production");
        assert_eq!(ap2.mixlevel, 0b00001);
        assert_eq!(ap2.peak_mix_level_db_spl(), 81);
        assert_eq!(ap2.roomtyp, crate::bsi::RoomType::Reserved);
    }

    /// `infomdate == 0` short-circuits the whole §E.2.3.1.x
    /// informational block: every typed surface stays `None` including
    /// the freshly-lifted [`crate::bsi::AudioProductionInfo`] mirror.
    /// Matches the round-208 `no_infomdate_yields_no_playback_hints`
    /// shape but extended for the round-214 production fields.
    #[test]
    fn no_infomdate_yields_no_audio_production() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod
            (3, 7),    // acmod = 3/2
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // mixmdate
            (1, 0),    // infomdate = 0
            (1, 0),    // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert!(bsi.audio_production.is_none());
        assert!(bsi.audio_production_ch2.is_none());
    }

    /// `infomdate == 0` → the `copyrightb` / `origbs` pair is
    /// definitionally absent from the wire (they live inside the
    /// §E.2.3.1.62 informational metadata block, gated on
    /// `infomdate == 1`). Surface must stay `None`.
    #[test]
    fn no_infomdate_yields_no_copyright_info() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = indep
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod
            (3, 2),    // acmod = 2/0
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // mixmdate
            (1, 0),    // infomdate = 0
            (1, 0),    // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert!(bsi.copyright_info.is_none());
    }

    /// `infomdate == 1` on a 3/2 indep frame surfaces the
    /// `(copyrightb, origbs)` pair through `copyright_info`. Walk a
    /// "protected, original" pattern (1, 1) to confirm both flags
    /// land on the typed surface independently.
    #[test]
    fn infomdate_surfaces_copyright_info_protected_original_on_3_2() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = indep
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod = 48 kHz
            (2, 3),    // numblkscod = 6 blocks
            (3, 7),    // acmod = 7 (3/2)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // mixmdate
            (1, 1),    // infomdate = 1
            // info body — 3/2 with copyrightb=1, origbs=1:
            (3, 0), // bsmod
            (1, 1), // copyrightb
            (1, 1), // origbs
            // acmod >= 6 → dsurexmod present; acmod != 2 → no dheadphonmod
            (2, 0), // dsurexmod
            (1, 0), // audprodie = 0
            // acmod != 0 → no audprodi2e
            (1, 0), // sourcefscod
            // strmtyp == Indep AND numblkscod == 3 → no convsync
            (1, 0), // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        let ci = bsi
            .copyright_info
            .expect("infomdate=1 should surface copyright_info");
        assert!(ci.is_copyright_protected());
        assert!(ci.is_original_bitstream());
        assert_eq!(ci.copyrightb_bit(), 1);
        assert_eq!(ci.origbs_bit(), 1);
    }

    /// `infomdate == 1` on a 2/0 indep frame with the "unprotected
    /// copy" pattern `(copyrightb=0, origbs=0)`. Distinct from the
    /// 3/2 case above (different acmod, different bit layout after
    /// `origbs`) so a single shared bit-cursor bug would surface as a
    /// disagreement between the two tests.
    #[test]
    fn infomdate_surfaces_copyright_info_unprotected_copy_on_2_0() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod = 6 blocks
            (3, 2),    // acmod = 2/0
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // mixmdate
            (1, 1),    // infomdate = 1
            (3, 0),    // bsmod
            (1, 0),    // copyrightb
            (1, 0),    // origbs
            // acmod == 2 fires the dheadphonmod gate (consumes dsurmod+dhpm).
            (2, 0), // dsurmod
            (2, 0), // dheadphonmod
            // acmod < 6 → no dsurexmod
            (1, 0), // audprodie = 0
            // acmod != 0 → no audprodi2e
            (1, 0), // sourcefscod
            (1, 0), // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        let ci = bsi
            .copyright_info
            .expect("infomdate=1 should surface copyright_info");
        assert!(!ci.is_copyright_protected());
        assert!(!ci.is_original_bitstream());
    }

    // ---------------------------------------------------------------
    // §5.4.2.8 / §5.4.2.16 (reused) — Annex E dialogue-normalization
    // typed surface.
    // ---------------------------------------------------------------

    /// On a stereo (acmod=2) indep substream the typed
    /// `dialogue_normalization()` accessor returns a [`DialNorm`] over
    /// the post-remap [`Bsi::dialnorm`] field, exposing `db()` and the
    /// §7.6 reproduction-gain derivation. `acmod != 0` → no
    /// `dialnorm_ch2`.
    #[test]
    fn parse_surfaces_dialogue_normalization_on_stereo_indep() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = independent
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod = 3 (6 blocks)
            (3, 2),    // acmod = 2 (2/0 stereo)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 20),   // dialnorm = -20 dB
            (1, 0),    // compre
            (1, 0),    // mixmdate
            (1, 0),    // infomdate
            (1, 0),    // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert_eq!(bsi.dialnorm, 20);
        assert!(bsi.dialnorm_ch2.is_none());
        let dn = bsi.dialogue_normalization();
        assert_eq!(dn.codepoint(), 20);
        assert_eq!(dn.db(), -20);
        assert_eq!(dn.level_below_full_scale_db(), 20);
        assert!(bsi.dialogue_normalization_ch2().is_none());
    }

    /// 1+1 dual-mono (acmod == 0) Annex E indep substream surfaces a
    /// separate `dialnorm_ch2` per §5.4.2.16 ("This 5-bit code has the
    /// same meaning as dialnorm, except that it applies to the second
    /// audio channel"). The typed accessor mirrors the AC-3 base
    /// surface.
    #[test]
    fn parse_surfaces_dialnorm_ch2_in_dual_mono() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod
            (3, 0),    // acmod = 0 (1+1 dual mono)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm (Ch1) = -27 dB
            (1, 0),    // compre (Ch1) = 0
            (5, 11),   // dialnorm2 (Ch2) = -11 dB
            (1, 0),    // compr2e
            (1, 0),    // mixmdate
            (1, 0),    // infomdate
            (1, 0),    // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert_eq!(bsi.acmod, 0);
        assert_eq!(bsi.dialnorm, 27);
        let dn_ch2_raw = bsi
            .dialnorm_ch2
            .expect("acmod == 0 should surface dialnorm_ch2");
        assert_eq!(dn_ch2_raw, 11);
        let typed = bsi
            .dialogue_normalization_ch2()
            .expect("dialnorm_ch2 surfaced");
        assert_eq!(typed.codepoint(), 11);
        assert_eq!(typed.db(), -11);
        // Ch1 surface is independent.
        assert_eq!(bsi.dialogue_normalization().db(), -27);
    }

    /// Annex E reuses §5.4.2.8 reserved-codepoint semantics for
    /// `dialnorm2` per §5.4.2.16 ("This 5-bit code has the same meaning
    /// as dialnorm"): wire `0` remaps to `31`. The parser stores the
    /// post-remap value on `dialnorm_ch2`.
    #[test]
    fn parse_remaps_dialnorm2_zero_codepoint_to_31_annex_e() {
        let bits: &[(u32, u32)] = &[
            (2, 0),
            (3, 0),
            (11, 383),
            (2, 0),
            (2, 3),
            (3, 0), // acmod = 0
            (1, 0),
            (5, 16),
            (5, 27), // dialnorm = -27 dB
            (1, 0),  // compre
            (5, 0),  // dialnorm2 = reserved 0 → remaps to 31
            (1, 0),  // compr2e
            (1, 0),  // mixmdate
            (1, 0),  // infomdate
            (1, 0),  // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        let dn_ch2_raw = bsi.dialnorm_ch2.expect("acmod == 0");
        assert_eq!(dn_ch2_raw, 31);
        let typed = bsi
            .dialogue_normalization_ch2()
            .expect("dialnorm_ch2 surfaced");
        assert_eq!(typed.codepoint(), 31);
        assert_eq!(typed.db(), -31);
    }

    // -----------------------------------------------------------------
    // AdditionalBitStreamInfo (Annex E reuse of §5.4.2.29-31)
    // -----------------------------------------------------------------

    /// Encoder-default `addbsie == 0` leaves `addbsi == None` on the
    /// Annex E surface — mirrors the base-AC-3 short-circuit.
    #[test]
    fn no_addbsie_yields_no_addbsi_eac3() {
        let bits: &[(u32, u32)] = &[
            (2, 0),
            (3, 0),
            (11, 383),
            (2, 0),
            (2, 3),
            (3, 2),
            (1, 0),
            (5, 16),
            (5, 27),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert!(bsi.addbsi.is_none());
    }

    /// Annex E independent substream with `addbsie == 1` and a 1-byte
    /// payload (the minimum). Confirms the parser walks the addbsi
    /// trailer correctly on E-AC-3 streams.
    #[test]
    fn parses_addbsi_single_byte_payload_eac3() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod
            (3, 2),    // acmod = 2/0
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // mixmdate
            (1, 0),    // infomdate
            (1, 1),    // addbsie
            (6, 0),    // addbsil = 0 → 1 byte
            (8, 0x5A), // payload
        ];
        let (buf, total) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        let info = bsi.addbsi.expect("addbsie == 1 surfaces a payload");
        assert_eq!(info.addbsil(), 0);
        assert_eq!(info.len(), 1);
        assert_eq!(info.payload(), &[0x5A]);
        assert_eq!(bsi.bits_consumed, total);
    }

    /// Annex E independent substream with the maximum-length 64-byte
    /// addbsi payload — confirms the parser walks all 519 trailer
    /// bits without slipping.
    #[test]
    fn parses_addbsi_max_length_payload_eac3() {
        let mut bits: Vec<(u32, u32)> = vec![
            (2, 0),
            (3, 0),
            (11, 383),
            (2, 0),
            (2, 3),
            (3, 2),
            (1, 0),
            (5, 16),
            (5, 27),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 1),  // addbsie
            (6, 63), // addbsil = 63 → 64 bytes
        ];
        for k in 0..64u32 {
            bits.push((8, (k * 7) ^ 0xA5));
        }
        let (buf, total) = pack_msb(&bits);
        let bsi = parse(&buf).unwrap();
        let info = bsi.addbsi.expect("addbsie == 1 surfaces a payload");
        assert_eq!(info.addbsil(), 63);
        assert_eq!(info.len(), 64);
        let expected: Vec<u8> = (0u32..64).map(|k| ((k * 7) ^ 0xA5) as u8).collect();
        assert_eq!(info.payload(), expected.as_slice());
        assert_eq!(bsi.bits_consumed, total);
        // Annex E wire_bits sanity: the trailer block alone spans
        // 1 (addbsie) + 6 (addbsil) + 64 × 8 (payload) = 519 bits.
        assert_eq!(info.wire_bits(), 7 + 8 * 64);
    }

    /// Dependent-substream Annex E (`strmtyp == Dependent` with
    /// `chanmape == 0`) followed by a 4-byte addbsi payload — confirms
    /// the addbsi cursor is unaffected by the upstream dependent-
    /// substream branch.
    #[test]
    fn parses_addbsi_on_dependent_substream_eac3() {
        let bits: &[(u32, u32)] = &[
            (2, 1),    // strmtyp = Dependent
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod
            (3, 7),    // acmod = 3/2 5.0 (no LFE)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // chanmape (dependent-only flag)
            (1, 0),    // mixmdate
            (1, 0),    // infomdate
            (1, 1),    // addbsie
            (6, 3),    // addbsil = 3 → 4 bytes
            (8, 0xDE),
            (8, 0xAD),
            (8, 0xBE),
            (8, 0xEF),
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert_eq!(bsi.strmtyp, StreamType::Dependent);
        let info = bsi.addbsi.expect("addbsie == 1 surfaces a payload");
        assert_eq!(info.payload(), &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    /// Round 243 — Annex E mixmdata (§E.1.2.2 reusing Annex D
    /// §2.3.1.2 / Table D2.2) surfaces the typed
    /// [`StereoDownmixPreference`] when `mixmdate == 1` AND `acmod > 2`.
    /// Cover all four wire codepoints round-tripping through `parse()`.
    #[test]
    fn parse_surfaces_dmixmod_preference_annex_e_all_codepoints() {
        for (code, expected) in [
            (0b00u32, StereoDownmixPreference::NotIndicated),
            (0b01u32, StereoDownmixPreference::LtRtPreferred),
            (0b10u32, StereoDownmixPreference::LoRoPreferred),
            (0b11u32, StereoDownmixPreference::Reserved),
        ] {
            let bits: &[(u32, u32)] = &[
                (2, 0),    // strmtyp = independent
                (3, 0),    // substreamid
                (11, 383), // frmsiz
                (2, 0),    // fscod
                (2, 3),    // numblkscod = 3 → 6 blocks
                (3, 7),    // acmod = 7 (3/2 — Annex E mixmdata dmixmod slot present)
                (1, 1),    // lfeon = 1
                (5, 16),   // bsid
                (5, 27),   // dialnorm
                (1, 0),    // compre
                (1, 1),    // mixmdate = 1
                // mixmdata body:
                (2, code), // dmixmod codepoint under test
                (3, 2),    // ltrtcmixlev
                (3, 4),    // lorocmixlev
                (3, 3),    // ltrtsurmixlev
                (3, 5),    // lorosurmixlev
                (1, 0),    // lfemixlevcode = 0
                // indep substream extras (strmtyp == 0):
                (1, 0), // pgmscle
                (1, 0), // extpgmscle
                (2, 0), // mixdef = 0
                (1, 0), // frmmixcfginfoe
                (1, 0), // infomdate
                (1, 0), // addbsie
            ];
            let (buf, _) = pack_msb(bits);
            let bsi = parse(&buf).unwrap();
            assert_eq!(bsi.dmixmod, code as u8, "raw dmixmod for {expected:?}");
            assert_eq!(bsi.dmixmod_preference, Some(expected));
            assert_eq!(bsi.stereo_downmix_preference(), Some(expected));
        }
    }

    /// Annex E 2/0 stereo with `mixmdate == 1` — the §E.1.2.2 guard
    /// skips the 2-bit `dmixmod` slot when `acmod <= 2`, so the typed
    /// preference is `None` even though the mixing-metadata block
    /// was emitted.
    #[test]
    fn parse_leaves_dmixmod_preference_none_when_acmod_le_2_annex_e() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = independent
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod = 3
            (3, 2),    // acmod = 2 (2/0 — no dmixmod slot per Table E1.2 guard)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 1),    // mixmdate = 1
            // mixmdata body for 2/0 indep: no dmixmod, no ltrt/loro
            // codes, no LFE code. Just the indep tail:
            (1, 0), // pgmscle
            (1, 0), // extpgmscle
            (2, 0), // mixdef
            (1, 0), // frmmixcfginfoe
            (1, 0), // infomdate
            (1, 0), // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert_eq!(bsi.acmod, 2);
        assert_eq!(bsi.dmixmod, 0xFF);
        assert!(bsi.dmixmod_preference.is_none());
        assert!(bsi.stereo_downmix_preference().is_none());
    }

    /// Annex E syncframe without a mixing-metadata block
    /// (`mixmdate == 0`) — the typed preference is `None` regardless
    /// of `acmod`.
    #[test]
    fn parse_leaves_dmixmod_preference_none_when_mixmdate_clear_annex_e() {
        let bits: &[(u32, u32)] = &[
            (2, 0),
            (3, 0),
            (11, 383),
            (2, 0),
            (2, 3),
            (3, 7), // acmod = 7 (3/2 — slot would be present if mixmdate == 1)
            (1, 1), // lfeon
            (5, 16),
            (5, 27),
            (1, 0), // compre
            (1, 0), // mixmdate = 0 — entire mixing-metadata block skipped
            (1, 0), // infomdate
            (1, 0), // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert!(bsi.dmixmod_preference.is_none());
        assert_eq!(bsi.dmixmod, 0xFF);
    }

    /// Non-1+1 Annex E streams (`acmod != 0`) never carry `dialnorm2`
    /// — the `dialnorm_ch2` field stays `None`. Mirrors the AC-3 base
    /// short-circuit.
    #[test]
    fn parse_leaves_dialnorm_ch2_none_outside_dual_mono_annex_e() {
        let bits: &[(u32, u32)] = &[
            (2, 0),
            (3, 0),
            (11, 383),
            (2, 0),
            (2, 3),
            (3, 7), // acmod = 7 (3/2 5.0)
            (1, 0), // lfeon
            (5, 16),
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // mixmdate
            (1, 0),  // infomdate
            (1, 0),  // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert_eq!(bsi.acmod, 7);
        assert!(bsi.dialnorm_ch2.is_none());
        assert!(bsi.dialogue_normalization_ch2().is_none());
    }

    /// Round 278 — §E.2.3.1.13 wire scale: the `0` codepoint is mute
    /// (no finite dB value, linear gain 0.0).
    #[test]
    fn program_scale_factor_mute_codepoint() {
        let psf = ProgramScaleFactor::from_code(0);
        assert!(psf.is_mute());
        assert_eq!(psf.raw(), 0);
        assert_eq!(psf.decibels(), None);
        assert_eq!(psf.linear(), 0.0);
    }

    /// §E.2.3.1.13 — codepoints `1..=63` map to `-50..=+12 dB` in
    /// 1 dB steps (`code - 51`; `51` is unity). Check every codepoint
    /// plus the spec's stated endpoints and the linear derivations.
    #[test]
    fn program_scale_factor_db_mapping_all_codepoints() {
        for code in 1u8..=63 {
            let psf = ProgramScaleFactor::from_code(code);
            assert!(!psf.is_mute());
            assert_eq!(psf.raw(), code);
            assert_eq!(psf.decibels(), Some(code as i8 - 51), "code {code}");
        }
        // Spec endpoints: "the values 1–63 shall be interpreted as a
        // scale factor of –50 dB to +12 dB in 1 dB steps".
        assert_eq!(ProgramScaleFactor::from_code(1).decibels(), Some(-50));
        assert_eq!(ProgramScaleFactor::from_code(63).decibels(), Some(12));
        // Unity at the 0 dB codepoint.
        let unity = ProgramScaleFactor::from_code(51);
        assert_eq!(unity.decibels(), Some(0));
        assert!((unity.linear() - 1.0).abs() < 1e-6);
        // Linear endpoints: 10^(-50/20) ≈ 0.0031623, 10^(12/20) ≈ 3.9811.
        assert!((ProgramScaleFactor::from_code(1).linear() - 0.003_162_3).abs() < 1e-6);
        assert!((ProgramScaleFactor::from_code(63).linear() - 3.981_07).abs() < 1e-4);
    }

    /// `from_code` masks to the 6-bit wire width so a caller can pass
    /// a wider word verbatim.
    #[test]
    fn program_scale_factor_from_code_masks_upper_bits() {
        assert_eq!(ProgramScaleFactor::from_code(0xFF).raw(), 0x3F);
        assert!(ProgramScaleFactor::from_code(0x40).is_mute());
        assert_eq!(
            ProgramScaleFactor::from_code(0x73),
            ProgramScaleFactor::from_code(0x33)
        );
    }

    /// Round 278 — an independent 3/2+LFE syncframe with
    /// `mixmdate == 1`, `pgmscle == 1` (-3 dB — the §E.3.10.1 worked
    /// example) and `extpgmscle == 1` (-10 dB — the §E.3.10.2 worked
    /// example) surfaces both typed scale factors; `pgmscl2` stays
    /// `None` outside 1+1 dual mono.
    #[test]
    fn parse_surfaces_pgmscl_and_extpgmscl_annex_e() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = independent
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod = 3 → 6 blocks
            (3, 7),    // acmod = 7 (3/2)
            (1, 1),    // lfeon = 1
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 1),    // mixmdate = 1
            // mixmdata body (acmod = 7, lfeon = 1, independent):
            (2, 0),  // dmixmod
            (3, 2),  // ltrtcmixlev
            (3, 4),  // lorocmixlev
            (3, 3),  // ltrtsurmixlev
            (3, 5),  // lorosurmixlev
            (1, 0),  // lfemixlevcode = 0
            (1, 1),  // pgmscle = 1
            (6, 48), // pgmscl = 48 → -3 dB (§E.3.10.1 example)
            (1, 1),  // extpgmscle = 1
            (6, 41), // extpgmscl = 41 → -10 dB (§E.3.10.2 example)
            (2, 0),  // mixdef = 0
            (1, 0),  // frmmixcfginfoe
            (1, 0),  // infomdate
            (1, 0),  // addbsie
        ];
        let (buf, total_bits) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        let pgmscl = bsi.pgmscl.expect("pgmscle == 1 surfaces pgmscl");
        assert_eq!(pgmscl.raw(), 48);
        assert_eq!(pgmscl.decibels(), Some(-3));
        assert!(bsi.pgmscl2.is_none(), "no pgmscl2 outside 1+1 dual mono");
        let ext = bsi.extpgmscl.expect("extpgmscle == 1 surfaces extpgmscl");
        assert_eq!(ext.raw(), 41);
        assert_eq!(ext.decibels(), Some(-10));
        assert_eq!(bsi.bits_consumed, total_bits);
    }

    /// 1+1 dual-mono (`acmod == 0`) independent substream — the
    /// §E.2.3.1.14-15 `pgmscl2` slot is on the wire and surfaces
    /// independently of `pgmscl`; the mute codepoint (`0`) survives
    /// the round-trip.
    #[test]
    fn parse_surfaces_pgmscl2_on_dual_mono_annex_e() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = independent
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod
            (3, 0),    // acmod = 0 (1+1 dual mono)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (5, 28),   // dialnorm2 (acmod == 0)
            (1, 0),    // compr2e
            (1, 1),    // mixmdate = 1
            // mixmdata body (acmod = 0 → no dmixmod / mix levels;
            // lfeon = 0 → no lfemixlevcode):
            (1, 1),  // pgmscle = 1
            (6, 51), // pgmscl = 51 → 0 dB unity
            (1, 1),  // pgmscl2e = 1
            (6, 0),  // pgmscl2 = 0 → mute
            (1, 0),  // extpgmscle = 0
            (2, 0),  // mixdef = 0
            (1, 0),  // paninfoe (acmod < 2)
            (1, 0),  // paninfo2e (acmod == 0)
            (1, 0),  // frmmixcfginfoe
            (1, 0),  // infomdate
            (1, 0),  // addbsie
        ];
        let (buf, total_bits) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        let pgmscl = bsi.pgmscl.expect("pgmscle == 1");
        assert_eq!(pgmscl.decibels(), Some(0));
        let pgmscl2 = bsi.pgmscl2.expect("pgmscl2e == 1");
        assert!(pgmscl2.is_mute());
        assert!(bsi.extpgmscl.is_none(), "extpgmscle == 0 ⇒ 0 dB default");
        assert_eq!(bsi.bits_consumed, total_bits);
    }

    /// `mixmdate == 1` with all three exists-flags clear — per
    /// §E.2.3.1.12/.14/.16 the scale factors default to "0 dB (no
    /// scaling)", represented as `None` on every field.
    #[test]
    fn parse_leaves_program_scale_factors_none_when_exists_flags_clear() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = independent
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod
            (3, 2),    // acmod = 2 (2/0)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 1),    // mixmdate = 1
            // mixmdata body for 2/0 indep:
            (1, 0), // pgmscle = 0
            (1, 0), // extpgmscle = 0
            (2, 0), // mixdef
            (1, 0), // frmmixcfginfoe
            (1, 0), // infomdate
            (1, 0), // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert!(bsi.pgmscl.is_none());
        assert!(bsi.pgmscl2.is_none());
        assert!(bsi.extpgmscl.is_none());
    }

    /// `mixmdate == 0` skips the whole mixing-metadata block — all
    /// three program scale factors stay `None` regardless of layout.
    #[test]
    fn parse_leaves_program_scale_factors_none_when_mixmdate_clear() {
        let bits: &[(u32, u32)] = &[
            (2, 0),
            (3, 0),
            (11, 383),
            (2, 0),
            (2, 3),
            (3, 7), // acmod = 7
            (1, 1), // lfeon
            (5, 16),
            (5, 27),
            (1, 0), // compre
            (1, 0), // mixmdate = 0
            (1, 0), // infomdate
            (1, 0), // addbsie
        ];
        let (buf, _) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert!(bsi.pgmscl.is_none());
        assert!(bsi.pgmscl2.is_none());
        assert!(bsi.extpgmscl.is_none());
    }

    /// Dependent substreams never carry the program-scale-factor
    /// chain — Table E1.2 emits `pgmscle` … `extpgmscl` under
    /// `strmtyp == 0x0` only. A dependent substream with
    /// `mixmdate == 1` still surfaces the mix-level codewords but
    /// leaves all three scale factors `None`.
    #[test]
    fn parse_skips_program_scale_factors_on_dependent_substream() {
        let bits: &[(u32, u32)] = &[
            (2, 1),    // strmtyp = dependent
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod
            (3, 7),    // acmod = 7 (3/2)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // chanmape (dependent-only flag)
            (1, 1),    // mixmdate = 1
            // mixmdata body for dependent 3/2: dmixmod + four mix
            // levels only — no indep pgmscl/extpgmscl/mixdef tail.
            (2, 1), // dmixmod = LtRt preferred
            (3, 2), // ltrtcmixlev
            (3, 4), // lorocmixlev
            (3, 3), // ltrtsurmixlev
            (3, 5), // lorosurmixlev
            (1, 0), // infomdate
            (1, 0), // addbsie
        ];
        let (buf, total_bits) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        assert_eq!(bsi.strmtyp, StreamType::Dependent);
        assert!(bsi.annex_e_mix_levels.is_some());
        assert!(bsi.pgmscl.is_none());
        assert!(bsi.pgmscl2.is_none());
        assert!(bsi.extpgmscl.is_none());
        assert_eq!(bsi.bits_consumed, total_bits);
    }

    /// Round 281 — §E.2.3.1.54 index scale: 1.5-degree clockwise
    /// steps from the center speaker location, indices `0..=239`
    /// spanning `0..=358.5` degrees, `240..=255` reserved.
    #[test]
    fn pan_info_degrees_mapping() {
        assert_eq!(PanInfo::from_fields(0, 0).degrees(), Some(0.0));
        assert_eq!(PanInfo::from_fields(1, 0).degrees(), Some(1.5));
        assert_eq!(PanInfo::from_fields(239, 0).degrees(), Some(358.5));
        for idx in 240..=255u16 {
            let pi = PanInfo::from_fields(idx as u8, 0);
            assert!(pi.is_reserved_index(), "index {idx}");
            assert_eq!(pi.degrees(), None, "index {idx}");
            assert_eq!(pi.stereo_scale_factors(), None, "index {idx}");
            assert_eq!(pi.surround_scale_factors(), None, "index {idx}");
        }
        assert!(!PanInfo::from_fields(239, 0).is_reserved_index());
        // §E.2.3.1.53 default: center, index 0.
        assert_eq!(PanInfo::CENTER.panmean(), 0);
        assert_eq!(PanInfo::CENTER.degrees(), Some(0.0));
        // The 6-bit reserved trailer is masked + preserved verbatim.
        assert_eq!(PanInfo::from_fields(10, 0xFF).reserved(), 0x3F);
        assert_eq!(PanInfo::from_fields(10, 0x2A).reserved(), 0x2A);
    }

    /// Table E3.15 — stereo-output panning scale factors `(AL, AR)`
    /// at the range boundaries: center (index 0) is the equal-power
    /// `cos/sin(π/4)` split, the `20..=99` range is fully right, the
    /// `140..=219` range fully left, and the trig ranges are
    /// continuous with their flat neighbours.
    #[test]
    fn pan_info_stereo_scale_factors_table_e3_15() {
        let eps = 1e-6f32;
        // Index 0 (center): AL = cos(π/2 · 20/40) = cos(π/4) = AR.
        let (al, ar) = PanInfo::from_fields(0, 0).stereo_scale_factors().unwrap();
        assert!((al - std::f32::consts::FRAC_1_SQRT_2).abs() < eps);
        assert!((ar - std::f32::consts::FRAC_1_SQRT_2).abs() < eps);
        // Flat ranges: 20..=99 fully right, 140..=219 fully left.
        for idx in [20u8, 60, 99] {
            assert_eq!(
                PanInfo::from_fields(idx, 0).stereo_scale_factors(),
                Some((0.0, 1.0)),
                "index {idx}"
            );
        }
        for idx in [140u8, 180, 219] {
            assert_eq!(
                PanInfo::from_fields(idx, 0).stereo_scale_factors(),
                Some((1.0, 0.0)),
                "index {idx}"
            );
        }
        // Range starts are continuous with the preceding flat range:
        // index 100 → (sin 0, cos 0) = (0, 1); index 220 → (cos 0,
        // sin 0) = (1, 0).
        let (al, ar) = PanInfo::from_fields(100, 0).stereo_scale_factors().unwrap();
        assert!((al - 0.0).abs() < eps && (ar - 1.0).abs() < eps);
        let (al, ar) = PanInfo::from_fields(220, 0).stereo_scale_factors().unwrap();
        assert!((al - 1.0).abs() < eps && (ar - 0.0).abs() < eps);
    }

    /// Table E3.15 is power-preserving over every non-reserved index:
    /// `AL² + AR² == 1` (each range is a single sin/cos pair or a
    /// degenerate 0/1 split).
    #[test]
    fn pan_info_stereo_scale_factors_power_preserving() {
        for idx in 0..=239u8 {
            let (al, ar) = PanInfo::from_fields(idx, 0).stereo_scale_factors().unwrap();
            let power = al * al + ar * ar;
            assert!((power - 1.0).abs() < 1e-5, "index {idx}: power {power}");
            assert!((0.0..=1.0).contains(&al), "index {idx}: AL {al}");
            assert!((0.0..=1.0).contains(&ar), "index {idx}: AR {ar}");
        }
    }

    /// Tables E3.16 + E3.17 — 5.1-output panning at the five range
    /// starts, each landing the source fully on a single speaker:
    /// index 0 → Center, 20 → Right, 73 → Right Surround, 167 → Left
    /// Surround, 220 → Left (order `[AL, AC, AR, ALS, ARS]`).
    #[test]
    fn pan_info_surround_scale_factors_cardinal_points() {
        let eps = 1e-6f32;
        for (idx, expect) in [
            (0u8, [0.0f32, 1.0, 0.0, 0.0, 0.0]),
            (20, [0.0, 0.0, 1.0, 0.0, 0.0]),
            (73, [0.0, 0.0, 0.0, 0.0, 1.0]),
            (167, [0.0, 0.0, 0.0, 1.0, 0.0]),
            (220, [1.0, 0.0, 0.0, 0.0, 0.0]),
        ] {
            let got = PanInfo::from_fields(idx, 0)
                .surround_scale_factors()
                .unwrap();
            for (ch, (g, e)) in got.iter().zip(expect.iter()).enumerate() {
                assert!((g - e).abs() < eps, "index {idx} ch {ch}: {g} vs {e}");
            }
        }
        // Mid-range check: index 10 splits Center/Right at
        // cos/sin(π/4) per the Table E3.16 first row.
        let got = PanInfo::from_fields(10, 0)
            .surround_scale_factors()
            .unwrap();
        assert!((got[1] - std::f32::consts::FRAC_1_SQRT_2).abs() < eps);
        assert!((got[2] - std::f32::consts::FRAC_1_SQRT_2).abs() < eps);
        assert_eq!((got[0], got[3], got[4]), (0.0, 0.0, 0.0));
    }

    /// Tables E3.16 + E3.17 are jointly power-preserving over every
    /// non-reserved index — each range pans between exactly two
    /// adjacent speakers with a sin/cos pair, so the five squared
    /// factors sum to 1.
    #[test]
    fn pan_info_surround_scale_factors_power_preserving() {
        for idx in 0..=239u8 {
            let sf = PanInfo::from_fields(idx, 0)
                .surround_scale_factors()
                .unwrap();
            let power: f32 = sf.iter().map(|s| s * s).sum();
            assert!((power - 1.0).abs() < 1e-5, "index {idx}: power {power}");
            assert!(
                sf.iter().all(|s| (0.0..=1.0).contains(s)),
                "index {idx}: {sf:?}"
            );
        }
    }

    /// Round 281 — an independent mono (`acmod == 1`) syncframe with
    /// `mixmdate == 1` and `paninfoe == 1` surfaces the typed
    /// [`PanInfo`] (index 10 → 15°, reserved trailer preserved);
    /// `paninfo2` stays `None` outside 1+1 dual mono.
    #[test]
    fn parse_surfaces_paninfo_on_mono_annex_e() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = independent
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod = 3 → 6 blocks
            (3, 1),    // acmod = 1 (1/0 mono — pan chain present)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 1),    // mixmdate = 1
            // mixmdata body (acmod = 1 indep: no dmixmod, no mix
            // levels, no LFE code):
            (1, 0),    // pgmscle
            (1, 0),    // extpgmscle
            (2, 0),    // mixdef = 0
            (1, 1),    // paninfoe = 1 (acmod < 2)
            (8, 10),   // panmean = 10 → 15.0 degrees
            (6, 0x2A), // paninfo (reserved trailer)
            (1, 0),    // frmmixcfginfoe
            (1, 0),    // infomdate
            (1, 0),    // addbsie
        ];
        let (buf, total_bits) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        let pan = bsi.paninfo.expect("paninfoe == 1 surfaces paninfo");
        assert_eq!(pan.panmean(), 10);
        assert_eq!(pan.reserved(), 0x2A);
        assert_eq!(pan.degrees(), Some(15.0));
        assert!(bsi.paninfo2.is_none(), "no paninfo2 outside 1+1 dual mono");
        assert_eq!(bsi.bits_consumed, total_bits);
    }

    /// 1+1 dual-mono (`acmod == 0`) independent substream — the
    /// §E.2.3.1.56-58 `paninfo2` slot is on the wire and surfaces
    /// independently of `paninfo`; a reserved index (240) survives
    /// the round-trip with `degrees() == None`.
    #[test]
    fn parse_surfaces_paninfo2_on_dual_mono_annex_e() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = independent
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod
            (3, 0),    // acmod = 0 (1+1 dual mono)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (5, 28),   // dialnorm2 (acmod == 0)
            (1, 0),    // compr2e
            (1, 1),    // mixmdate = 1
            // mixmdata body (acmod = 0 → no dmixmod / mix levels;
            // lfeon = 0 → no lfemixlevcode):
            (1, 0),    // pgmscle
            (1, 0),    // pgmscl2e (acmod == 0)
            (1, 0),    // extpgmscle
            (2, 0),    // mixdef = 0
            (1, 1),    // paninfoe = 1
            (8, 170),  // panmean = 170 → fully-left flat range
            (6, 0),    // paninfo
            (1, 1),    // paninfo2e = 1 (acmod == 0)
            (8, 240),  // panmean2 = 240 → reserved index
            (6, 0x3F), // paninfo2
            (1, 0),    // frmmixcfginfoe
            (1, 0),    // infomdate
            (1, 0),    // addbsie
        ];
        let (buf, total_bits) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        let pan = bsi.paninfo.expect("paninfoe == 1");
        assert_eq!(pan.panmean(), 170);
        assert_eq!(pan.stereo_scale_factors(), Some((1.0, 0.0)));
        let pan2 = bsi.paninfo2.expect("paninfo2e == 1");
        assert_eq!(pan2.panmean(), 240);
        assert!(pan2.is_reserved_index());
        assert_eq!(pan2.degrees(), None);
        assert_eq!(pan2.reserved(), 0x3F);
        assert_eq!(bsi.bits_consumed, total_bits);
    }

    /// The Table E1.2 guards keep the pan chain off the wire: a 2/0
    /// stereo (`acmod == 2`) indep substream with `mixmdate == 1`
    /// has no `paninfoe` slot at all, and a mono substream with
    /// `paninfoe == 0` defaults to "center" — both read back as
    /// `None` per §E.2.3.1.53.
    #[test]
    fn parse_leaves_paninfo_none_when_guards_fail() {
        // acmod = 2 — pan chain skipped entirely.
        let stereo: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = independent
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod
            (3, 2),    // acmod = 2 (2/0 — no pan chain per Table E1.2)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 1),    // mixmdate = 1
            (1, 0),    // pgmscle
            (1, 0),    // extpgmscle
            (2, 0),    // mixdef
            (1, 0),    // frmmixcfginfoe
            (1, 0),    // infomdate
            (1, 0),    // addbsie
        ];
        let (buf, _) = pack_msb(stereo);
        let bsi = parse(&buf).unwrap();
        assert!(bsi.paninfo.is_none());
        assert!(bsi.paninfo2.is_none());

        // acmod = 1 with paninfoe = 0 — defaulted to center (None).
        let mono: &[(u32, u32)] = &[
            (2, 0),
            (3, 0),
            (11, 383),
            (2, 0),
            (2, 3),
            (3, 1), // acmod = 1
            (1, 0),
            (5, 16),
            (5, 27),
            (1, 0), // compre
            (1, 1), // mixmdate = 1
            (1, 0), // pgmscle
            (1, 0), // extpgmscle
            (2, 0), // mixdef
            (1, 0), // paninfoe = 0 — pan word defaults to center
            (1, 0), // frmmixcfginfoe
            (1, 0), // infomdate
            (1, 0), // addbsie
        ];
        let (buf, total_bits) = pack_msb(mono);
        let bsi = parse(&buf).unwrap();
        assert!(bsi.paninfo.is_none());
        assert!(bsi.paninfo2.is_none());
        assert_eq!(bsi.bits_consumed, total_bits);
    }

    // ---- §E.2.3.1.19-21 premix-compression (PremixCompression) ----

    /// Table E2.7 — every listed `premixcmpscl` code maps to its
    /// `n/6` compression gain-reduction ratio, the unlisted `0b110`
    /// codepoint surfaces as reserved (`scale_ratio() == None`), and
    /// the percentage captions in the spec round to the `n/6` values.
    #[test]
    fn premixcmpscl_scale_ratio_table_e2_7() {
        // (code, expected sixths)
        let listed: &[(u8, u8)] = &[
            (0b000, 0),
            (0b001, 1),
            (0b010, 2),
            (0b011, 3),
            (0b100, 4),
            (0b101, 5),
            (0b111, 6),
        ];
        for &(code, sixths) in listed {
            let pc = PremixCompression::from_fields(false, false, code);
            assert!(!pc.is_premixcmpscl_reserved(), "code {code:#05b} is listed");
            let ratio = pc.scale_ratio().expect("listed code has a ratio");
            assert!(
                (ratio - sixths as f32 / 6.0).abs() < 1e-6,
                "code {code:#05b} → {ratio}, want {}/6",
                sixths
            );
        }
        // 0b110: no Table E2.7 row.
        let reserved = PremixCompression::from_fields(false, false, 0b110);
        assert!(reserved.is_premixcmpscl_reserved());
        assert!(reserved.scale_ratio().is_none());
        // Spec percentage captions match the n/6 ratios.
        let pct = |code: u8| PremixCompression::from_fields(false, false, code).scale_ratio();
        assert!((pct(0b001).unwrap() - 0.167).abs() < 0.001); // 16.7%
        assert!((pct(0b010).unwrap() - 0.333).abs() < 0.001); // 33.3%
        assert!((pct(0b011).unwrap() - 0.5).abs() < 1e-6); //    50%
        assert!((pct(0b100).unwrap() - 0.667).abs() < 0.001); // 66.7%
        assert!((pct(0b101).unwrap() - 0.833).abs() < 0.001); // 83.3%
    }

    /// `premixcmpsel` / `drcsrc` typed views (§E.2.3.1.19-20) and the
    /// recommended-default predicate (§E.2.3.1.21 note).
    #[test]
    fn premix_compression_typed_views_and_default() {
        let default = PremixCompression::from_fields(false, false, 0b000);
        assert_eq!(default.compression_word(), PremixCompressionWord::DynRng);
        assert_eq!(default.drc_source(), DrcSource::ExternalProgram);
        assert!(default.is_recommended_default());

        let custom = PremixCompression::from_fields(true, true, 0b011);
        assert_eq!(custom.compression_word(), PremixCompressionWord::Compr);
        assert_eq!(custom.drc_source(), DrcSource::CurrentSubstream);
        assert!(!custom.is_recommended_default());
        // Raw round-trip.
        assert!(custom.premixcmpsel());
        assert!(custom.drcsrc());
        assert_eq!(custom.premixcmpscl(), 0b011);
        // Each non-default field alone breaks the recommended default.
        assert!(!PremixCompression::from_fields(true, false, 0).is_recommended_default());
        assert!(!PremixCompression::from_fields(false, true, 0).is_recommended_default());
        assert!(!PremixCompression::from_fields(false, false, 1).is_recommended_default());
        // Upper bits of premixcmpscl are masked to 3 bits.
        assert_eq!(
            PremixCompression::from_fields(false, false, 0b1111).premixcmpscl(),
            0b111
        );
    }

    /// `mixdef == 0x1` body on an independent 3/2 substream surfaces
    /// the typed `premix_compression` field; the parse cursor lands
    /// exactly at `audfrm()` (`bits_consumed == total`).
    #[test]
    fn parse_surfaces_premix_compression_mixdef1_annex_e() {
        let bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = independent
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod = 3 → 6 blocks
            (3, 7),    // acmod = 7 (3/2)
            (1, 1),    // lfeon = 1
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 1),    // mixmdate = 1
            // mixmdata body (acmod = 7, lfeon = 1, independent):
            (2, 0), // dmixmod
            (3, 0), // ltrtcmixlev
            (3, 0), // lorocmixlev
            (3, 0), // ltrtsurmixlev
            (3, 0), // lorosurmixlev
            (1, 0), // lfemixlevcode = 0
            (1, 0), // pgmscle = 0
            (1, 0), // extpgmscle = 0
            (2, 1), // mixdef = 1 ("mixing option 2")
            // §E.2.3.1.19-21 body:
            (1, 1), // premixcmpsel = 1 → use compr
            (1, 0), // drcsrc = 0 → external program (recommended)
            (3, 4), // premixcmpscl = 0b100 → 66.7%
            (1, 0), // frmmixcfginfoe
            (1, 0), // infomdate
            (1, 0), // addbsie
        ];
        let (buf, total_bits) = pack_msb(bits);
        let bsi = parse(&buf).unwrap();
        let pc = bsi
            .premix_compression
            .expect("mixdef == 0x1 surfaces premix_compression");
        assert!(pc.premixcmpsel());
        assert!(!pc.drcsrc());
        assert_eq!(pc.premixcmpscl(), 0b100);
        assert_eq!(pc.compression_word(), PremixCompressionWord::Compr);
        assert_eq!(pc.drc_source(), DrcSource::ExternalProgram);
        assert!((pc.scale_ratio().unwrap() - 4.0 / 6.0).abs() < 1e-6);
        assert!(!pc.is_recommended_default());
        assert_eq!(bsi.bits_consumed, total_bits);
    }

    /// A `mixdef ∈ {0, 2}` block leaves `premix_compression` `None`
    /// (the three fields are not carried as a standalone group), and a
    /// `mixmdate == 0` syncframe also reports `None`.
    #[test]
    fn parse_leaves_premix_compression_none_for_other_mixdef() {
        // mixdef = 0 on a 2/0 independent substream.
        let mixdef0: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = independent
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod
            (3, 2),    // acmod = 2 (2/0)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 1),    // mixmdate = 1
            (1, 0),    // pgmscle
            (1, 0),    // extpgmscle
            (2, 0),    // mixdef = 0 → no premix body
            (1, 0),    // frmmixcfginfoe
            (1, 0),    // infomdate
            (1, 0),    // addbsie
        ];
        let (buf, total_bits) = pack_msb(mixdef0);
        let bsi = parse(&buf).unwrap();
        assert!(bsi.premix_compression.is_none());
        assert_eq!(bsi.bits_consumed, total_bits);

        // mixmdate = 0 → whole mixing-metadata block skipped.
        let no_mix: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = independent
            (3, 0),    // substreamid
            (11, 383), // frmsiz
            (2, 0),    // fscod
            (2, 3),    // numblkscod
            (3, 2),    // acmod = 2
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // mixmdate = 0
            (1, 0),    // infomdate
            (1, 0),    // addbsie
        ];
        let (buf2, total2) = pack_msb(no_mix);
        let bsi2 = parse(&buf2).unwrap();
        assert!(bsi2.premix_compression.is_none());
        assert_eq!(bsi2.bits_consumed, total2);
    }
}

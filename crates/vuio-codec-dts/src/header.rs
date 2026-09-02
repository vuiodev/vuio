//! DTS Coherent Acoustics frame-sync header parser.
//!
//! All field layouts and value-range bounds in this module come
//! verbatim from the mirrored multimedia.cx snapshot at
//! `docs/audio/dts/wiki/DTS.wiki`, which in turn mirrors the ETSI
//! TS 102 114 §5.3 frame-header description. The wiki notes four
//! sync encodings:
//!
//! ```text
//!   7F FE 80 01           — raw big-endian
//!   FE 7F 01 80           — raw little-endian (byte-swapped)
//!   1F FF E8 00 07 Fx     — 14-bit packed big-endian
//!   FF 1F 00 E8 Fx 07     — 14-bit packed little-endian
//! ```
//!
//! Round 1 fully parses the two 16-bit raw variants and returns
//! [`Error::UnsupportedFourteenBit`] for the 14-bit variants from
//! [`parse_frame_header`]. Round 2 adds [`parse_frame_header_14bit`]
//! plus a [`crate::unpack_14bit_to_16bit`] primitive that converts a
//! 14-bit-packed buffer into its 16-bit-equivalent raw-BE form so the
//! existing parser can consume both encodings uniformly.
//!
//! ## Field layout (after the 32-bit sync, MSB-first)
//!
//! | Bits | Name                  | Notes                              |
//! | ---- | --------------------- | ---------------------------------- |
//! | 1    | FTYPE                 | 0 = termination, 1 = normal        |
//! | 5    | SHORT (sample count)  | raw value; samples-in-block = +1   |
//! | 1    | CRC_PRESENT           |                                    |
//! | 7    | NBLKS (block count)   | raw 5..=127                        |
//! | 14   | FSIZE-1               | frame size in bytes = +1, 95..=16384 |
//! | 6    | AMODE (channel cfg)   | 0..=15 standard, 16..=63 user      |
//! | 4    | SFREQ                 | sample-freq index (tables missing) |
//! | 5    | RATE                  | bitrate index (tables missing)     |
//! | 1    | DOWNMIX               | embedded downmix-coefficients flag |
//! | 1    | DYNRANGE              | embedded dynamic-range data flag   |
//! | 1    | TIMSTP                | timestamp-field-present flag       |
//! | 1    | AUXDATA               | auxiliary-data-field-present flag  |
//! | 1    | HDCD                  | HDCD-encoded-source flag           |
//! | 3    | EXT_DESCR             | extension-audio-descriptor (0..=7) |
//! | 1    | EXT_CODING            | extension-audio-coding flag        |
//! | 1    | ASPF                  | audio-sync-word in subframes flag  |
//! | 2    | LFE                   | LFE channel mode (0..=3)           |
//! | 1    | PRED_HISTORY          | predictor-history-enabled flag     |
//! | 16   | HEADER_CRC            | only present when CRC_PRESENT == 1 |
//! | 1    | MULTIRATE_INTER       | multirate-interpolation-filter selector |
//! | 4    | VERSION               | encoder version (raw 0..=15)       |
//! | 2    | COPY_HISTORY          | copy-history code (0..=3)          |
//! | 3    | PCMR                  | source-PCM-resolution index (0..=7) |
//! | 1    | FRONT_SUM             | front-channel sum/difference flag  |
//! | 1    | SURROUND_SUM          | surround-channel sum/difference flag |
//! | 4    | DIALNORM              | dialog normalization (dB of recovery) |
//!
//! Round 3 (2026-05-21) surfaced the first batch through
//! [`DtsFrameHeader`]. Round 5 (2026-05-25) extends the typed
//! header through the seven additional post-CRC fields the wiki
//! enumerates (MULTIRATE_INTER, VERSION, COPY_HISTORY, PCMR,
//! FRONT_SUM, SURROUND_SUM, DIALNORM). These 16 bits always
//! follow the HEADER_CRC slot (or the predictor-history bit when
//! `crc_present == 0`), so the parser consumes them
//! unconditionally. The value-table fields (DIALNORM dB, COPY_HISTORY
//! provenance, PCMR resolution mapping) are surfaced as raw indices
//! because the wiki snapshot enumerates the bit widths but not the
//! per-code semantic mapping — those tables remain a `docs/`
//! follow-up.

use crate::bitreader::BitReader;
use crate::filter_bank::FilterBankSelection;
use crate::unpack14::{FourteenBitByteOrder, unpack_14bit_to_16bit};
use crate::{Error, Result};

/// The four documented DTS Core syncword encodings (per the wiki
/// snapshot's "How to distinguish different versions" table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SyncWordEncoding {
    /// `7F FE 80 01` — native big-endian raw 16-bit-per-word DTS.
    /// The wiki notes this is the **native** DTS byte order.
    RawBigEndian,
    /// `FE 7F 01 80` — byte-swapped little-endian raw 16-bit-per-word
    /// DTS. Commonly seen inside DTS-in-WAV / CD-DA encapsulation.
    RawLittleEndian,
    /// `1F FF E8 00 07 Fx` — 14-bit big-endian packed DTS. The
    /// `unpack14` module (round 2) converts this into the raw-BE
    /// form for [`parse_frame_header_14bit`].
    FourteenBitBigEndian,
    /// `FF 1F 00 E8 Fx 07` — 14-bit little-endian packed DTS. The
    /// `unpack14` module (round 2) converts this into the raw-BE
    /// form for [`parse_frame_header_14bit`].
    FourteenBitLittleEndian,
}

impl SyncWordEncoding {
    /// Byte length of the on-wire sync sequence for this encoding,
    /// directly read from the wiki snapshot's
    /// "How to distinguish different versions" table
    /// (`docs/audio/dts/wiki/DTS.wiki`):
    ///
    /// | Encoding                  | Sync sequence            | Bytes |
    /// | ------------------------- | ------------------------ | ----- |
    /// | `RawBigEndian`            | `7F FE 80 01`            | 4     |
    /// | `RawLittleEndian`         | `FE 7F 01 80`            | 4     |
    /// | `FourteenBitBigEndian`    | `1F FF E8 00 07 Fx`      | 6     |
    /// | `FourteenBitLittleEndian` | `FF 1F 00 E8 Fx 07`      | 6     |
    ///
    /// The 14-bit variants are 6 bytes because the last container
    /// (`07 Fx` / `Fx 07`) carries the upper bits of the 32-bit
    /// syncword inside a 14-bit-payload container, and matching the
    /// full sync requires inspecting the four high bits of that
    /// trailing container per [`crate::parse_frame_header_14bit`]'s
    /// detection rule.
    ///
    /// This accessor is the wiki-derived counterpart to a
    /// [`crate::SyncMatch::sync_byte_length`] call. It does not
    /// reflect the **frame** byte length (that is
    /// [`crate::DtsFrameHeader::frame_size_bytes`]) — only the bytes
    /// the sync sequence itself occupies on the wire.
    #[inline]
    pub fn sync_byte_length(self) -> usize {
        match self {
            SyncWordEncoding::RawBigEndian | SyncWordEncoding::RawLittleEndian => 4,
            SyncWordEncoding::FourteenBitBigEndian | SyncWordEncoding::FourteenBitLittleEndian => 6,
        }
    }

    /// Whether this encoding is one of the two raw 16-bit-per-word
    /// forms (the native DTS encodings per the wiki).
    ///
    /// Equivalent to `matches!(self, RawBigEndian | RawLittleEndian)`
    /// and provided so demuxer / re-muxer code can branch on the
    /// "raw vs 14-bit container" distinction without spelling the
    /// `matches!` out at every call site.
    #[inline]
    pub fn is_raw_16bit(self) -> bool {
        matches!(
            self,
            SyncWordEncoding::RawBigEndian | SyncWordEncoding::RawLittleEndian
        )
    }

    /// Whether this encoding is one of the two 14-bit-packed
    /// container forms (the wiki's "DTS Music CD" / "DTS-in-WAV"
    /// 14-bit forms).
    ///
    /// Equivalent to `!self.is_raw_16bit()` but spelled out
    /// affirmatively for readability at call sites that need the
    /// 14-bit branch (e.g. the [`crate::FrameIterator`]'s
    /// `UnsupportedFourteenBit` guard).
    #[inline]
    pub fn is_14bit_packed(self) -> bool {
        matches!(
            self,
            SyncWordEncoding::FourteenBitBigEndian | SyncWordEncoding::FourteenBitLittleEndian
        )
    }
}

/// Frame-type flag (FTYPE bit, 1 bit wide).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameType {
    /// `FTYPE == 0` — termination frame. Per the wiki this marks the
    /// last frame in a continuous stream.
    Termination,
    /// `FTYPE == 1` — normal frame.
    Normal,
}

/// LFE-channel mode (`LFE`, 2 bits wide).
///
/// The wiki snapshot lists the field as a 2-bit code without naming
/// the four values. ETSI TS 102 114 §5.3.1 documents the codes as
/// "no LFE channel" (0), "128-sample-decimated LFE" (1),
/// "64-sample-decimated LFE" (2), and "reserved/invalid" (3); the
/// wiki snapshot itself does not include those labels, so this enum
/// keeps the names neutral — `code` is the raw 2-bit value and
/// [`Self::is_present`] discriminates "no LFE" (code 0) from the
/// three present-LFE codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LfeMode {
    /// Raw LFE code 0. The wiki implies this is "no LFE channel"
    /// because the LFE field is the gate to the LFE-stream
    /// subblocks; this implementation does not assert it.
    None,
    /// Raw LFE code 1 — present, mode-1 (see `docs/audio/dts/wiki/`).
    Mode1,
    /// Raw LFE code 2 — present, mode-2.
    Mode2,
    /// Raw LFE code 3 — reserved-or-mode-3 per the wiki snapshot.
    Mode3,
}

impl LfeMode {
    /// Construct from the raw 2-bit code (`0..=3`).
    fn from_raw(code: u8) -> Self {
        match code & 0b11 {
            0 => LfeMode::None,
            1 => LfeMode::Mode1,
            2 => LfeMode::Mode2,
            _ => LfeMode::Mode3,
        }
    }

    /// Recover the raw 2-bit LFE code.
    pub fn code(self) -> u8 {
        match self {
            LfeMode::None => 0,
            LfeMode::Mode1 => 1,
            LfeMode::Mode2 => 2,
            LfeMode::Mode3 => 3,
        }
    }

    /// Whether *any* LFE channel is present. Codes 1..=3 all signal a
    /// present LFE channel per the wiki; only code 0 marks its
    /// absence.
    pub fn is_present(self) -> bool {
        !matches!(self, LfeMode::None)
    }
}

/// Targeted transmission bit-rate decoded from the 5-bit `RATE`
/// header field.
///
/// The mapping is **ETSI TS 102 114 V1.3.1 §5.3.1, Table 5-7**
/// ("RATE parameter versus targeted bit-rate"), transcribed in
/// `docs/audio/dts/dts-core-extracts.md` §1. Table 5-7 enumerates 25
/// fixed targeted rates (codes `0b00000`..=`0b11000`), one *open*-mode
/// code (`0b11101`), and marks every other code invalid. Per the
/// spec the field names the *targeted* transmission rate, which may
/// be greater than or equal to the actual coded bit-rate; *open* mode
/// permits rates that are not table entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TargetedBitRate {
    /// One of the 25 fixed targeted rates from Table 5-7, in bits per
    /// second (e.g. code `0b01111` → `768_000`). The `1 411,2 kbit/s`
    /// entry (code `0b10110`) is represented exactly as `1_411_200`.
    Fixed(u32),
    /// `RATE == 0b11101` — *open* mode. The frame's targeted bit-rate
    /// is not constrained to a Table 5-7 entry, so no fixed bps value
    /// applies.
    Open,
    /// A reserved / invalid `RATE` code: any code Table 5-7 does not
    /// list among the 25 fixed values or the open code.
    Invalid,
}

/// Bits-per-second values for the 25 fixed `RATE` codes
/// (`0b00000`..=`0b11000`), in code order, per ETSI TS 102 114
/// §5.3.1 Table 5-7 (`docs/audio/dts/dts-core-extracts.md` §1).
/// ETSI lists the rates in kbit/s; this table converts each to bits
/// per second (the decimal-comma `1 411,2 kbit/s` entry → `1_411_200`).
const RATE_TABLE_BPS: [u32; 25] = [
    32_000, 56_000, 64_000, 96_000, 112_000, 128_000, 192_000, 224_000, 256_000, 320_000, 384_000,
    448_000, 512_000, 576_000, 640_000, 768_000, 960_000, 1_024_000, 1_152_000, 1_280_000,
    1_344_000, 1_408_000, 1_411_200, 1_472_000, 1_536_000,
];

/// Resolve a raw 5-bit `RATE` index to its [`TargetedBitRate`] per
/// Table 5-7. Codes `0..=24` are the fixed rates; `29` (`0b11101`)
/// is the open code; everything else is invalid.
fn targeted_bit_rate_from_index(rate_index: u8) -> TargetedBitRate {
    match rate_index {
        0..=24 => TargetedBitRate::Fixed(RATE_TABLE_BPS[rate_index as usize]),
        29 => TargetedBitRate::Open,
        _ => TargetedBitRate::Invalid,
    }
}

// ---------------------------------------------------------------
// Core audio sampling frequency — ETSI TS 102 114 V1.3.1 §5.3.1
// Table 5-5 (PDF p.19).
// ---------------------------------------------------------------
//
// SFREQ is a 4-bit field; six of the sixteen codes are documented as
// "Invalid". The "Source Sampling Frequency" column of Table 5-5 is
// the rate of the *original* PCM input to the encoder; for the
// resampled-base-band core (>48 kHz inputs are split into core +
// extended bands) the spec further notes that the encoder can only
// process Fs_core ≤ 48 kHz, so the table's >48 kHz rows describe the
// source rate, not the core-stream rate. This module surfaces the
// SFREQ→Hz mapping exactly as Table 5-5 lists it.

/// Core-audio sampling frequency decoded from the 4-bit `SFREQ`
/// header field per **ETSI TS 102 114 V1.3.1 §5.3.1, Table 5-5**
/// (PDF p.19). Seven of the sixteen codes are documented as
/// *Invalid*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SampleFrequency {
    /// One of the nine fixed source-sampling-frequency codes from
    /// Table 5-5 (in Hertz; e.g. `SFREQ == 0b1101` → `48_000`).
    Fixed(u32),
    /// A reserved / invalid `SFREQ` code: codes `0b0000`, `0b0100`,
    /// `0b0101`, `0b1001`, `0b1010`, `0b1110`, `0b1111` per Table 5-5.
    Invalid,
}

/// Sampling-frequency-in-Hertz values for the nine fixed `SFREQ` codes
/// of ETSI §5.3.1 Table 5-5. Indexed by `SFREQ` directly; `Invalid`
/// entries are `0` and never reached through [`sample_frequency_from_index`].
/// The nine non-zero values are (in code order, excluding invalid
/// rows): 8 000 / 16 000 / 32 000 / 11 025 / 22 050 / 44 100 / 12 000
/// / 24 000 / 48 000 — matching the spec's listed `Source Sampling
/// Frequency` column verbatim.
const SAMPLE_FREQUENCY_TABLE: [u32; 16] = [
    0,      // 0b0000 Invalid
    8_000,  // 0b0001 8 kHz
    16_000, // 0b0010 16 kHz
    32_000, // 0b0011 32 kHz
    0,      // 0b0100 Invalid
    0,      // 0b0101 Invalid
    11_025, // 0b0110 11,025 kHz
    22_050, // 0b0111 22,05 kHz
    44_100, // 0b1000 44,1 kHz
    0,      // 0b1001 Invalid
    0,      // 0b1010 Invalid
    12_000, // 0b1011 12 kHz
    24_000, // 0b1100 24 kHz
    48_000, // 0b1101 48 kHz
    0,      // 0b1110 Invalid
    0,      // 0b1111 Invalid
];

/// Resolve a raw 4-bit `SFREQ` index to its [`SampleFrequency`] per
/// Table 5-5. The ten fixed rows map to `Fixed(hz)`; the six other
/// codes map to `Invalid`.
fn sample_frequency_from_index(sfreq_index: u8) -> SampleFrequency {
    if sfreq_index >= 16 {
        return SampleFrequency::Invalid;
    }
    let hz = SAMPLE_FREQUENCY_TABLE[sfreq_index as usize];
    if hz == 0 {
        SampleFrequency::Invalid
    } else {
        SampleFrequency::Fixed(hz)
    }
}

// ---------------------------------------------------------------
// Audio Channel Arrangement (AMODE) — ETSI TS 102 114 V1.3.1
// §5.3.1 Table 5-4 (PDF p.18).
// ---------------------------------------------------------------
//
// AMODE is a 6-bit field. Codes `0b000000..=0b001111` (0..=15) are
// the sixteen standard arrangements; codes `0b010000..=0b111111`
// (16..=63) are *User defined* (per Table 5-4's last row). The
// arrangement column names channels using the spec's NOTE legend
// (L = left, R = right, C = centre, S = surround, F = front,
// R = rear, T = total, OV = overhead, A = first mono, B = second
// mono). The CHS column is the channel count for each row.
//
// The LFE channel is **not** part of AMODE — it is gated by the
// separate 2-bit LFE field (already surfaced via [`LfeMode`]).
//
// The CHS-by-AMODE-code mapping below is transcribed verbatim from
// Table 5-4 (in code order, 0..=15):
//   0=1, 1=2, 2=2, 3=2, 4=2, 5=3, 6=3, 7=4,
//   8=4, 9=5, 10=6, 11=6, 12=6, 13=7, 14=8, 15=8.

/// Standard audio channel arrangement decoded from the 6-bit `AMODE`
/// header field per **ETSI TS 102 114 V1.3.1 §5.3.1, Table 5-4**
/// (PDF p.18). Codes `0..=15` are the sixteen standard arrangements;
/// `16..=63` are *User defined* and surfaced as
/// [`Self::UserDefined`] with the raw code preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AmodeArrangement {
    /// `0b000000` — A (mono, 1 channel).
    Mono,
    /// `0b000001` — A + B (dual mono, 2 channels).
    DualMono,
    /// `0b000010` — L + R (stereo, 2 channels).
    Stereo,
    /// `0b000011` — (L+R) + (L-R) (sum-difference, 2 channels).
    SumDifference,
    /// `0b000100` — LT + RT (left/right total, 2 channels).
    LtRt,
    /// `0b000101` — C + L + R (3 channels).
    ClR,
    /// `0b000110` — L + R + S (3 channels).
    LrS,
    /// `0b000111` — C + L + R + S (4 channels).
    ClRS,
    /// `0b001000` — L + R + SL + SR (4 channels).
    LrSlSr,
    /// `0b001001` — C + L + R + SL + SR (5 channels).
    ClRSlSr,
    /// `0b001010` — CL + CR + L + R + SL + SR (6 channels).
    ClCrLRSlSr,
    /// `0b001011` — C + L + R + LR + RR + OV (6 channels).
    ClRLrRrOv,
    /// `0b001100` — CF + CR + LF + RF + LR + RR (6 channels).
    CfCrLfRfLrRr,
    /// `0b001101` — CL + C + CR + L + R + SL + SR (7 channels).
    ClCCrLRSlSr,
    /// `0b001110` — CL + CR + L + R + SL1 + SL2 + SR1 + SR2
    /// (8 channels).
    ClCrLRSl1Sl2Sr1Sr2,
    /// `0b001111` — CL + C + CR + L + R + SL + S + SR (8 channels).
    ClCCrLRSlSSr,
    /// `0b010000..=0b111111` — user-defined arrangement. The raw
    /// 6-bit AMODE code is preserved; the channel count is not
    /// derivable from the spec table.
    UserDefined(u8),
}

impl AmodeArrangement {
    /// Channel count (CHS column of Table 5-4) for the sixteen
    /// standard arrangements. Returns `None` for [`Self::UserDefined`]
    /// codes (the spec does not enumerate a CHS for those).
    pub fn channel_count(self) -> Option<u8> {
        match self {
            AmodeArrangement::Mono => Some(1),
            AmodeArrangement::DualMono
            | AmodeArrangement::Stereo
            | AmodeArrangement::SumDifference
            | AmodeArrangement::LtRt => Some(2),
            AmodeArrangement::ClR | AmodeArrangement::LrS => Some(3),
            AmodeArrangement::ClRS | AmodeArrangement::LrSlSr => Some(4),
            AmodeArrangement::ClRSlSr => Some(5),
            AmodeArrangement::ClCrLRSlSr
            | AmodeArrangement::ClRLrRrOv
            | AmodeArrangement::CfCrLfRfLrRr => Some(6),
            AmodeArrangement::ClCCrLRSlSr => Some(7),
            AmodeArrangement::ClCrLRSl1Sl2Sr1Sr2 | AmodeArrangement::ClCCrLRSlSSr => Some(8),
            AmodeArrangement::UserDefined(_) => None,
        }
    }

    /// Bitstream-order channel indices of the **front left / right** pair
    /// carrying the §C.2.4 `(L+R, L-R)` sum/difference encoding when the
    /// `SUMF` flag is set (or unconditionally for
    /// [`Self::SumDifference`]). Derived from the Table 5-4 channel
    /// ordering documented on each variant: the first tuple element is
    /// the channel that decodes to front-left (`L' = L + R`), the second
    /// decodes to front-right (`R' = L - R`).
    ///
    /// Returns `None` for arrangements with no distinct front L/R pair
    /// ([`Self::Mono`], [`Self::DualMono`]) or whose channel layout is
    /// not enumerated by the spec ([`Self::UserDefined`]). The
    /// [`AmodeArrangement::ClRLrRrOv`] / [`AmodeArrangement::CfCrLfRfLrRr`]
    /// six-channel arrangements are also `None` — their front-pair
    /// labelling is not an unambiguous `L`/`R` in the Table 5-4 ordering.
    #[must_use]
    pub fn front_lr_channels(self) -> Option<(usize, usize)> {
        match self {
            // L, R lead the arrangement.
            AmodeArrangement::Stereo
            | AmodeArrangement::SumDifference
            | AmodeArrangement::LtRt
            | AmodeArrangement::LrS
            | AmodeArrangement::LrSlSr => Some((0, 1)),
            // A leading centre channel offsets L, R by one.
            AmodeArrangement::ClR | AmodeArrangement::ClRS | AmodeArrangement::ClRSlSr => {
                Some((1, 2))
            }
            // CL + CR lead, then L, R.
            AmodeArrangement::ClCrLRSlSr => Some((2, 3)),
            // CL + C + CR lead, then L, R.
            AmodeArrangement::ClCCrLRSlSr | AmodeArrangement::ClCCrLRSlSSr => Some((3, 4)),
            // CL + CR lead, then L, R (8-channel SL1/SL2/SR1/SR2 form).
            AmodeArrangement::ClCrLRSl1Sl2Sr1Sr2 => Some((2, 3)),
            AmodeArrangement::Mono
            | AmodeArrangement::DualMono
            | AmodeArrangement::ClRLrRrOv
            | AmodeArrangement::CfCrLfRfLrRr
            | AmodeArrangement::UserDefined(_) => None,
        }
    }

    /// Bitstream-order channel indices of the **left / right surround**
    /// pair carrying the §C.2.4 `(L+R, L-R)` sum/difference encoding when
    /// the `SUMS` flag is set. First element decodes to surround-left,
    /// second to surround-right.
    ///
    /// Returns `None` for arrangements without a distinct surround L/R
    /// pair (mono/stereo, single-surround arrangements such as
    /// [`Self::LrS`] / [`Self::ClRS`], and [`Self::UserDefined`]).
    #[must_use]
    pub fn surround_lr_channels(self) -> Option<(usize, usize)> {
        match self {
            // L + R + SL + SR.
            AmodeArrangement::LrSlSr => Some((2, 3)),
            // C + L + R + SL + SR.
            AmodeArrangement::ClRSlSr => Some((3, 4)),
            // CL + CR + L + R + SL + SR.
            AmodeArrangement::ClCrLRSlSr => Some((4, 5)),
            // CL + C + CR + L + R + SL + SR.
            AmodeArrangement::ClCCrLRSlSr => Some((5, 6)),
            AmodeArrangement::Mono
            | AmodeArrangement::DualMono
            | AmodeArrangement::Stereo
            | AmodeArrangement::SumDifference
            | AmodeArrangement::LtRt
            | AmodeArrangement::ClR
            | AmodeArrangement::LrS
            | AmodeArrangement::ClRS
            | AmodeArrangement::ClRLrRrOv
            | AmodeArrangement::CfCrLfRfLrRr
            // The 8-channel SL1/SL2/SR1/SR2 and C-plus-single-S forms have
            // no single unambiguous surround L/R pair in Table 5-4.
            | AmodeArrangement::ClCrLRSl1Sl2Sr1Sr2
            | AmodeArrangement::ClCCrLRSlSSr
            | AmodeArrangement::UserDefined(_) => None,
        }
    }
}

/// Resolve a raw 6-bit `AMODE` index to its [`AmodeArrangement`] per
/// Table 5-4. Codes `0..=15` are the sixteen standard arrangements;
/// codes `16..=63` are user-defined (preserved as
/// [`AmodeArrangement::UserDefined`]).
fn amode_arrangement_from_index(amode: u8) -> AmodeArrangement {
    match amode {
        0 => AmodeArrangement::Mono,
        1 => AmodeArrangement::DualMono,
        2 => AmodeArrangement::Stereo,
        3 => AmodeArrangement::SumDifference,
        4 => AmodeArrangement::LtRt,
        5 => AmodeArrangement::ClR,
        6 => AmodeArrangement::LrS,
        7 => AmodeArrangement::ClRS,
        8 => AmodeArrangement::LrSlSr,
        9 => AmodeArrangement::ClRSlSr,
        10 => AmodeArrangement::ClCrLRSlSr,
        11 => AmodeArrangement::ClRLrRrOv,
        12 => AmodeArrangement::CfCrLfRfLrRr,
        13 => AmodeArrangement::ClCCrLRSlSr,
        14 => AmodeArrangement::ClCrLRSl1Sl2Sr1Sr2,
        15 => AmodeArrangement::ClCCrLRSlSSr,
        // Codes 16..=63 are the user-defined range per Table 5-4's
        // final row. Codes >63 cannot reach this function because the
        // AMODE field is only 6 bits wide; we mask defensively.
        code => AmodeArrangement::UserDefined(code & 0b0011_1111),
    }
}

// ---------------------------------------------------------------
// Source PCM Resolution (PCMR) — ETSI TS 102 114 V1.3.1
// §5.3.1 Table 5-17 (PDF p.23).
// ---------------------------------------------------------------
//
// PCMR is a 3-bit field. Table 5-17 lists six valid codes plus an
// "Others" row marked invalid. Each valid code carries a (bits, ES)
// pair where ES is a single auxiliary flag indicating that the L/R
// surround channels of the source were mastered in DTS-ES format.
// Code-to-(bits, ES) (from Table 5-17, in code order 0..=7):
//   0b000=(16,0), 0b001=(16,1), 0b010=(20,0), 0b011=(20,1),
//   0b110=(24,0), 0b101=(24,1), others=Invalid.

/// Source-PCM-resolution decoded from the 3-bit `PCMR` header field
/// per **ETSI TS 102 114 V1.3.1 §5.3.1, Table 5-17** (PDF p.23).
/// The `es` flag indicates that the L/R surround channels were
/// mastered in DTS-ES format (ES=1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SourcePcmResolution {
    /// One of the six valid Table 5-17 rows: `bits` per source PCM
    /// sample plus the auxiliary `es` flag.
    Valid {
        /// Source-PCM bits-per-sample (16, 20, or 24).
        bits: u8,
        /// DTS-ES indicator (ES column of Table 5-17).
        es: bool,
    },
    /// A reserved / invalid `PCMR` code: codes `0b100` (4) and
    /// `0b111` (7) per Table 5-17's "Others" row.
    Invalid,
}

/// Resolve a raw 3-bit `PCMR` index to its [`SourcePcmResolution`]
/// per Table 5-17. The six valid codes map to `Valid { bits, es }`;
/// codes `4` and `7` map to `Invalid`.
fn source_pcm_resolution_from_index(pcmr_index: u8) -> SourcePcmResolution {
    match pcmr_index & 0b111 {
        0b000 => SourcePcmResolution::Valid {
            bits: 16,
            es: false,
        },
        0b001 => SourcePcmResolution::Valid { bits: 16, es: true },
        0b010 => SourcePcmResolution::Valid {
            bits: 20,
            es: false,
        },
        0b011 => SourcePcmResolution::Valid { bits: 20, es: true },
        0b110 => SourcePcmResolution::Valid {
            bits: 24,
            es: false,
        },
        0b101 => SourcePcmResolution::Valid { bits: 24, es: true },
        _ => SourcePcmResolution::Invalid,
    }
}

// ---------------------------------------------------------------
// Dialog Normalization Gain (DIALNORM/UNSPEC) — ETSI TS 102 114
// V1.3.1 §5.3.1 Table 5-20 (PDF p.24).
// ---------------------------------------------------------------
//
// The 4-bit field that follows SURROUND_SUM in the post-CRC header
// window is named `DIALNORM` when `VERNUM` is 6 or 7, and `UNSPEC`
// otherwise. Table 5-20 documents the (VERNUM, DIALNORM) → Dialog
// Normalization Gain (DNG, in decibels) mapping for the two named
// VERNUM rows:
//
//   VERNUM=7 → codes 0..15 → DNG dB  0, -1, -2, -3, -4, -5, -6, -7,
//                                    -8, -9,-10,-11,-12,-13,-14,-15
//   VERNUM=6 → codes 0..15 → DNG dB-16,-17,-18,-19,-20,-21,-22,-23,
//                                   -24,-25,-26,-27,-28,-29,-30,-31
//
// For every other VERNUM (`0,1,2,3,4,5,8,9,...,15`), §5.3.1 specifies
// that the 4-bit field is `UNSPEC`, the decoder must still extract
// the bits, and the Dialog Normalization Gain is fixed at 0 dB
// (the spec's "DNG=0 indicates No Dialog Normalization" sentence on
// PDF p.23). The two rows + the "all other VERNUM => DNG=0"
// convention give a total function on every (VERNUM, DIALNORM) pair
// the 4-bit fields can encode.

/// Dialog Normalization Gain decoded from the 4-bit `DIALNORM`/`UNSPEC`
/// header field per **ETSI TS 102 114 V1.3.1 §5.3.1, Table 5-20**
/// (PDF p.24), routed through the 4-bit `VERNUM` field that precedes
/// it in the post-CRC window.
///
/// The dB value is always non-positive: Table 5-20 enumerates 0 dB
/// down to −31 dB across the two named VERNUM rows, and the spec's
/// `DNG = 0` convention for all other VERNUM values makes 0 dB the
/// only other reachable value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DialogNormalization {
    /// `VERNUM` was 6 or 7: the 4-bit field is `DIALNORM` per
    /// Table 5-20. The contained value is the Dialog Normalization
    /// Gain in decibels (always ≤ 0). For example
    /// `(VERNUM=7, DIALNORM=0)` → `Fixed(0)`,
    /// `(VERNUM=7, DIALNORM=15)` → `Fixed(-15)`,
    /// `(VERNUM=6, DIALNORM=0)` → `Fixed(-16)`,
    /// `(VERNUM=6, DIALNORM=15)` → `Fixed(-31)`.
    Fixed(i8),
    /// `VERNUM` was outside {6, 7}: the 4-bit field is `UNSPEC` per
    /// §5.3.1. The spec says the decoder must still extract the bits
    /// (the parser does, into [`DtsFrameHeader::dialog_normalization`])
    /// but must apply no dialog-normalization gain. Equivalent to
    /// `Fixed(0)` for playback purposes — the variant preserves the
    /// `UNSPEC` distinction for callers that care about the original
    /// field meaning.
    Unspecified,
}

impl DialogNormalization {
    /// Dialog Normalization Gain in decibels.
    ///
    /// Returns the spec's `DNG` value: the contained `i8` for the
    /// [`Self::Fixed`] variant, and `0` for [`Self::Unspecified`]
    /// (per §5.3.1's "DNG=0 indicates No Dialog Normalization" for
    /// non-{6,7} VERNUM values).
    pub fn gain_db(self) -> i8 {
        match self {
            DialogNormalization::Fixed(db) => db,
            DialogNormalization::Unspecified => 0,
        }
    }
}

/// Resolve a `(VERNUM, DIALNORM)` pair to its [`DialogNormalization`]
/// per Table 5-20.
///
/// Only the low 4 bits of each argument are consulted — both fields
/// are 4-bit wires in the post-CRC header window.
fn dialog_normalization_from_codes(vernum: u8, dialnorm: u8) -> DialogNormalization {
    let dialnorm = dialnorm & 0b1111;
    match vernum & 0b1111 {
        7 => DialogNormalization::Fixed(-(dialnorm as i8)),
        6 => DialogNormalization::Fixed(-(dialnorm as i8) - 16),
        _ => DialogNormalization::Unspecified,
    }
}

/// Parsed DTS Core frame-sync header.
///
/// Round 1 surfaces only the structural fields whose semantics are
/// unambiguous in the wiki snapshot. The sample-rate / channel-count
/// *value* tables are not in `docs/` yet — see [`Self::sample_rate_hz`]
/// and [`Self::channel_count`] for the `Option` semantics. The
/// bitrate table (ETSI §5.3.1 Table 5-7) landed in round 185, so
/// [`Self::bit_rate_bps`] / [`Self::targeted_bit_rate`] now resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DtsFrameHeader {
    /// Which of the four documented sync encodings was found at
    /// offset zero.
    pub sync_word_encoding: SyncWordEncoding,
    /// Decoded frame type (termination vs normal).
    pub frame_type: FrameType,
    /// Samples per sub-block (the wiki's "Deficit sample count + 1",
    /// nominally 32 for a normal frame).
    pub sample_count_per_block: u8,
    /// Whether the header CRC field is present (the 16-bit field
    /// that follows the predictor-history bit). Round 1 does not
    /// verify it; the flag is exposed so a future round can.
    pub crc_present: bool,
    /// Number of sub-blocks in the frame (raw NBLKS, 5..=127).
    pub blocks_per_frame: u8,
    /// Frame size in bytes (`FSIZE-1 + 1`, 95..=16384).
    pub frame_size_bytes: u16,
    /// Channel-configuration code (AMODE, 0..=63). 0..=15 are
    /// standard layouts (per ETSI §5.3.1 Table 5-4 — resolved by
    /// [`Self::amode_arrangement`] / [`Self::channel_count`]);
    /// 16..=63 are user-defined.
    pub amode: u8,
    /// Sample-frequency index (SFREQ, 0..=15) per ETSI §5.3.1
    /// Table 5-5. Resolved by [`Self::sample_rate_hz`] /
    /// [`Self::sample_frequency`] (six codes are documented as
    /// `Invalid` per the spec).
    pub sfreq_index: u8,
    /// Transmission-bitrate index (RATE, 0..=31). Resolves to a
    /// targeted bit-rate via ETSI §5.3.1 Table 5-7 — see
    /// [`Self::bit_rate_bps`] / [`Self::targeted_bit_rate`].
    pub rate_index: u8,
    /// Embedded-downmix-coefficients flag (`DOWNMIX`, 1 bit).
    pub downmix: bool,
    /// Embedded-dynamic-range-data flag (`DYNF` / `DYNRANGE`, 1 bit).
    /// Per ETSI §5.3.1 Table 5-8 (`docs/audio/dts/dts-core-extracts.md`
    /// §1): `false` → dynamic-range coefficients not present;
    /// `true` → present at the start of each subframe.
    pub dynamic_range: bool,
    /// Timestamp-field-present flag (`TIMEF` / `TIMSTP`, 1 bit). Per
    /// ETSI §5.3.1 Table 5-9 (`docs/audio/dts/dts-core-extracts.md`
    /// §1): `false` → time stamps not present; `true` → present at
    /// the end of the core audio data. Round 3 surfaces the flag but
    /// does not interpret the optional timestamp payload itself.
    pub time_stamp: bool,
    /// Auxiliary-data-field-present flag (`AUXDATA`, 1 bit).
    pub aux_data: bool,
    /// HDCD-encoded-source flag (`HDCD`, 1 bit).
    pub hdcd: bool,
    /// Extension-audio-descriptor (`EXT_DESCR`, 3 bits, 0..=7). The
    /// wiki snapshot does not enumerate the value semantics; the raw
    /// 3-bit code is preserved verbatim.
    pub ext_descr: u8,
    /// Extension-audio-coding flag (`EXT_CODING`, 1 bit). Indicates
    /// whether an extension substream (X96 / XCH / XXCH / EXSS) is
    /// muxed alongside the Core stream.
    pub ext_coding: bool,
    /// Audio-sync-word-in-subframes flag (`ASPF`, 1 bit).
    pub aspf: bool,
    /// LFE-channel mode (`LFE`, 2 bits). See [`LfeMode`].
    pub lfe: LfeMode,
    /// Predictor-history-enabled flag (`PRED_HISTORY`, 1 bit).
    pub predictor_history: bool,
    /// 16-bit header-CRC value (`HEADER_CRC`, the spec's `HCRC`).
    /// Present iff [`Self::crc_present`] is `true`; `None` otherwise.
    /// The algorithm is the Annex B CRC-CCITT ([`crate::dts_crc16`],
    /// `docs/audio/dts/dts-crc16.md`), but for the core `HCRC` /
    /// `AHCRC` / `SICRC` / `OCRC` fields the spec explicitly states
    /// "The CRC value test **shall not be applied**" — they are
    /// informational placeholders, unlike the genuinely verified
    /// aux / Rev2-aux / extension-substream check words. The field is
    /// therefore surfaced raw for pass-through callers and
    /// [`Self::verify_header_crc`] keeps returning `None`.
    pub header_crc: Option<u16>,
    /// Multirate-interpolation-filter selector (`MULTIRATE_INTER`,
    /// 1 bit). This bit **is** the spec's `FILTS` ("Multirate
    /// Interpolator Switch") field of §5.3.1: per ETSI TS 102 114
    /// V1.3.1 §5.3.1 Table 5-15 (resolved in
    /// `docs/audio/dts/dts-qmf-driver.md` §1) it selects which of the
    /// two §D.8 32-band interpolation FIR coefficient sets the §C.2.5
    /// `QMFInterpolation()` driver convolves against:
    ///
    /// | `multirate_inter` / `FILTS` | 32-band interpolation filter |
    /// |-----------------------------|------------------------------|
    /// | `0` (`false`) | Non-Perfect Reconstruction (`raCoeffLossy`)    |
    /// | `1` (`true`)  | Perfect Reconstruction (`raCoeffLossLess`)     |
    ///
    /// The header-field table (§5.3.1 Table 5-15) and the §C.2.5
    /// driver pseudocode agree bit-for-bit — there is no inverted
    /// convention. [`Self::filter_bank_selection`] bridges this bit
    /// directly to [`crate::FilterBankSelection`] for the §C.2.5
    /// FIR step.
    pub multirate_inter: bool,
    /// Encoder version code (`VERSION`, 4 bits, 0..=15). The wiki
    /// snapshot does not enumerate which integer values correspond
    /// to which encoder revisions; round 5 surfaces the raw 4-bit
    /// code for pass-through callers.
    pub version: u8,
    /// Copy-history code (`COPY_HISTORY`, 2 bits, 0..=3). The wiki
    /// snapshot does not document the per-code semantics; raw value
    /// preserved.
    pub copy_history: u8,
    /// Source-PCM-resolution index (`PCMR`, 3 bits, 0..=7) per ETSI
    /// §5.3.1 Table 5-17. Resolved by
    /// [`Self::source_pcm_bits_per_sample`] /
    /// [`Self::source_pcm_resolution`] (six codes are valid, two
    /// (`0b100` / `0b111`) are documented as `Invalid`).
    pub source_pcm_resolution_index: u8,
    /// Front-channel sum/difference flag (`FRONT_SUM`, 1 bit). For
    /// stereo encodings, signals that the front L/R channels were
    /// transmitted as a sum/difference pair rather than discrete
    /// channels. The semantic interpretation is documented in the
    /// spec; the bit itself is surfaced verbatim.
    pub front_sum: bool,
    /// Surround-channel sum/difference flag (`SURROUND_SUM`, 1 bit).
    /// Same convention as [`Self::front_sum`] but for the surround
    /// channel pair.
    pub surround_sum: bool,
    /// Dialog-normalization code (`DIALNORM` for `VERNUM ∈ {6, 7}`,
    /// otherwise `UNSPEC`; 4 bits, 0..=15). Resolved to a Dialog
    /// Normalization Gain in dB via [`Self::dialog_normalization_db`]
    /// /[`Self::dialog_normalization_gain`] per ETSI TS 102 114 V1.3.1
    /// §5.3.1 Table 5-20 (PDF p.24), with the spec's `UNSPEC` → 0 dB
    /// convention applied for `VERNUM ∉ {6, 7}`.
    pub dialog_normalization: u8,
}

impl DtsFrameHeader {
    /// Resolve [`Self::sfreq_index`] to a sample-rate in Hertz per
    /// ETSI TS 102 114 V1.3.1 §5.3.1 Table 5-5 (PDF p.19).
    ///
    /// Returns `Some(hz)` for the nine valid `SFREQ` codes (e.g.
    /// code `0b1101` → `Some(48_000)`), and `None` for the seven
    /// codes Table 5-5 lists as *Invalid* (`0b0000`, `0b0100`,
    /// `0b0101`, `0b1001`, `0b1010`, `0b1110`, `0b1111`). Callers
    /// that need to distinguish "valid rate" from "invalid code"
    /// should use [`Self::sample_frequency`].
    ///
    /// Per Table 5-5's note, the value is the **source** sampling
    /// frequency. For inputs ≤ 48 kHz the source rate equals the
    /// core rate; for >48 kHz inputs the encoder splits the spectrum
    /// into a core band ≤ 48 kHz plus extended bands carrying the
    /// remainder.
    pub fn sample_rate_hz(&self) -> Option<u32> {
        match self.sample_frequency() {
            SampleFrequency::Fixed(hz) => Some(hz),
            SampleFrequency::Invalid => None,
        }
    }

    /// Resolve [`Self::sfreq_index`] to its [`SampleFrequency`] per
    /// ETSI TS 102 114 §5.3.1 Table 5-5.
    ///
    /// Richer counterpart to [`Self::sample_rate_hz`]: preserves the
    /// `Fixed` / `Invalid` distinction the `Option<u32>` accessor
    /// collapses to `None`.
    pub fn sample_frequency(&self) -> SampleFrequency {
        sample_frequency_from_index(self.sfreq_index)
    }

    /// Resolve [`Self::rate_index`] to its targeted transmission
    /// bit-rate per ETSI TS 102 114 §5.3.1 Table 5-7.
    ///
    /// This is the richer counterpart to [`Self::bit_rate_bps`]: it
    /// preserves the *open*-mode (`RATE == 0b11101`) and *invalid*
    /// distinctions that the `Option<u32>` accessor collapses to
    /// `None`. See [`TargetedBitRate`].
    pub fn targeted_bit_rate(&self) -> TargetedBitRate {
        targeted_bit_rate_from_index(self.rate_index)
    }

    /// Resolve [`Self::rate_index`] to a targeted transmission
    /// bit-rate in bits per second.
    ///
    /// Returns `Some(bps)` for the 25 fixed `RATE` codes of ETSI
    /// §5.3.1 Table 5-7 (e.g. code `0b01111` → `Some(768_000)`), and
    /// `None` for the *open*-mode code (`0b11101`, where no fixed rate
    /// applies) and for any reserved / invalid code. Callers that need
    /// to distinguish open from invalid should use
    /// [`Self::targeted_bit_rate`].
    pub fn bit_rate_bps(&self) -> Option<u32> {
        match self.targeted_bit_rate() {
            TargetedBitRate::Fixed(bps) => Some(bps),
            TargetedBitRate::Open | TargetedBitRate::Invalid => None,
        }
    }

    /// Resolve [`Self::amode`] to a count of audio channels (LFE
    /// excluded; the LFE field is surfaced separately via
    /// [`Self::lfe`] / [`LfeMode::is_present`]) per ETSI TS 102 114
    /// V1.3.1 §5.3.1 Table 5-4 (PDF p.18).
    ///
    /// Returns `Some(chs)` for the sixteen standard AMODE codes
    /// (e.g. code `2` → `Some(2)` for L+R stereo), and `None` for
    /// codes `16..=63` which Table 5-4's final row marks *User
    /// defined* — those codes carry no fixed channel-count in the
    /// spec table. Callers that want to inspect the full arrangement
    /// (per-channel placement, sum/difference encoding, etc.) should
    /// use [`Self::amode_arrangement`].
    pub fn channel_count(&self) -> Option<u8> {
        self.amode_arrangement().channel_count()
    }

    /// Resolve [`Self::amode`] to its [`AmodeArrangement`] per ETSI
    /// TS 102 114 §5.3.1 Table 5-4.
    ///
    /// Richer counterpart to [`Self::channel_count`]: returns the
    /// full arrangement enum (named per Table 5-4) so callers can
    /// branch on the playback layout (mono / dual-mono / stereo /
    /// sum-difference / LtRt / various multichannel arrangements)
    /// rather than just the channel count. User-defined codes
    /// (`16..=63`) round-trip the raw 6-bit value through
    /// [`AmodeArrangement::UserDefined`].
    pub fn amode_arrangement(&self) -> AmodeArrangement {
        amode_arrangement_from_index(self.amode)
    }

    /// Resolve [`Self::source_pcm_resolution_index`] to the source
    /// PCM bits-per-sample value the encoder declared per ETSI
    /// TS 102 114 V1.3.1 §5.3.1 Table 5-17 (PDF p.23).
    ///
    /// Returns `Some(bits)` for the six valid `PCMR` codes (e.g.
    /// code `0b000` → `Some(16)`), and `None` for the two codes
    /// Table 5-17's "Others" row marks *Invalid* (`0b100` and
    /// `0b111`). The auxiliary DTS-ES flag is dropped by this
    /// accessor; callers that need both halves should use
    /// [`Self::source_pcm_resolution`].
    pub fn source_pcm_bits_per_sample(&self) -> Option<u8> {
        match self.source_pcm_resolution() {
            SourcePcmResolution::Valid { bits, .. } => Some(bits),
            SourcePcmResolution::Invalid => None,
        }
    }

    /// Resolve [`Self::source_pcm_resolution_index`] to its
    /// [`SourcePcmResolution`] per ETSI TS 102 114 §5.3.1 Table 5-17.
    ///
    /// Richer counterpart to [`Self::source_pcm_bits_per_sample`]:
    /// preserves the DTS-ES (`es`) flag that Table 5-17 stores
    /// alongside the bits-per-sample column.
    pub fn source_pcm_resolution(&self) -> SourcePcmResolution {
        source_pcm_resolution_from_index(self.source_pcm_resolution_index)
    }

    /// Resolve [`Self::multirate_inter`] (the `FILTS` "Multirate
    /// Interpolator Switch" bit) to the §D.8 32-band interpolation
    /// FIR coefficient set the §C.2.5 `QMFInterpolation()` driver
    /// must convolve against.
    ///
    /// Per **ETSI TS 102 114 V1.3.1 §5.3.1 Table 5-15** (resolved in
    /// `docs/audio/dts/dts-qmf-driver.md` §1, which establishes that
    /// the `MULTIRATE_INTER` header field *is* the spec's `FILTS`
    /// field and that the header table and the §C.2.5 driver
    /// pseudocode agree bit-for-bit):
    ///
    /// - `multirate_inter == false` (`FILTS == 0`) →
    ///   [`FilterBankSelection::NonPerfectReconstruction`]
    ///   (`raCoeffLossy`);
    /// - `multirate_inter == true` (`FILTS == 1`) →
    ///   [`FilterBankSelection::PerfectReconstruction`]
    ///   (`raCoeffLossLess`).
    ///
    /// This is the bridge from a parsed header to the typed
    /// [`crate::FilterBankSelection`] consumed by the §C.2.5 FIR
    /// step (`crate::QmfSynthesis::synthesize`); it is
    /// equivalent to
    /// `FilterBankSelection::from_filts(u8::from(self.multirate_inter))`.
    #[must_use]
    pub fn filter_bank_selection(&self) -> FilterBankSelection {
        FilterBankSelection::from_filts(u8::from(self.multirate_inter))
    }

    /// The §C.2.5 `QMFInterpolation()` output gain `rScale` — the
    /// single post-filterbank float→PCM full-scale conversion factor
    /// applied at the driver's `naCh[nChIndex++] = int(rScale*raZ[i])`
    /// step.
    ///
    /// Per `docs/audio/dts/dts-qmf-driver.md` §2, the §C.2.5 output
    /// `rScale` is **not** a normatively-fixed numeric constant
    /// (§C.2.5 is one informative implementation among many). What the
    /// spec pins down is its purpose: it brings the normalized
    /// floating-point filterbank output `raZ[i]` (nominal range ≈
    /// ±1.0, with the QMF `1/N` normalization already folded into the
    /// §C.2.5 `raCosMod` scalers) up to the signed-integer full-scale
    /// range of the source PCM resolution declared by `PCMR` (§5.3.1
    /// Table 5-17). For a real-valued implementation that keeps `raZ`
    /// at unit scale, `rScale = 2^(PCMR_bits − 1)` (e.g. 32768.0 for
    /// 16-bit source PCM).
    ///
    /// This accessor returns that canonical derivation
    /// `2^(bits − 1)` for the six valid `PCMR` codes (16/20/24-bit →
    /// `Some(32768.0 / 524288.0 / 8388608.0)`), and `None` for the
    /// two reserved/invalid codes ([`SourcePcmResolution::Invalid`]),
    /// matching the `Option` semantics of
    /// [`Self::source_pcm_bits_per_sample`]. Implementations that
    /// apply their own headroom/clip-guard factor or carry a
    /// different internal `raZ` normalization should pass their own
    /// `rScale` to `crate::QmfSynthesis::synthesize` instead.
    #[must_use]
    pub fn output_r_scale(&self) -> Option<f64> {
        self.source_pcm_bits_per_sample()
            .map(|bits| (2.0_f64).powi(i32::from(bits) - 1))
    }

    /// The §5.3.1 `SHORT` (Deficit Sample Count) padding a
    /// **termination frame** asks the decoder to append, in PCM
    /// samples per channel — or `None` for a normal frame.
    ///
    /// Per ETSI TS 102 114 §5.3.1 (PDF p.18): a termination frame
    /// "carries n×32 core audio samples where block length n is
    /// adjusted to just fall short of the video end point", and "On
    /// completion of a termination frame, (SHORT+1) PCM core samples
    /// must be padded to the output buffers of each channel. The
    /// padded samples may be zeros or they may be copies of adjacent
    /// samples." `SHORT` is valid in `[0, 30]` for termination frames
    /// (Table 5-3; `31` indicates a normal frame), so the returned
    /// pad is `1..=31` samples.
    ///
    /// The decode chain ([`crate::decode_core_frame`] /
    /// [`crate::CoreStreamDecoder`]) returns only the **decoded**
    /// samples; appending the presentation pad (and choosing its
    /// fill) is the caller's output-layer decision, made with this
    /// accessor. [`Self::sample_count_per_block`] stores the wire
    /// field as `SHORT + 1`.
    pub fn termination_pad_samples(&self) -> Option<u8> {
        match self.frame_type {
            FrameType::Termination => Some(self.sample_count_per_block),
            FrameType::Normal => None,
        }
    }

    /// Resolve [`Self::dialog_normalization`] to a Dialog
    /// Normalization Gain in decibels per **ETSI TS 102 114 V1.3.1
    /// §5.3.1, Table 5-20** (PDF p.24), routed through
    /// [`Self::version`] (the `VERNUM` field that precedes DIALNORM
    /// in the post-CRC header window).
    ///
    /// Always returns `Some`:
    /// - `VERNUM == 7` → `Some(-(DIALNORM as i8))` (codes 0..=15 → 0 dB
    ///   down to −15 dB).
    /// - `VERNUM == 6` → `Some(-(DIALNORM as i8) - 16)` (codes 0..=15 →
    ///   −16 dB down to −31 dB).
    /// - Any other `VERNUM` → `Some(0)` per §5.3.1's "DNG=0 indicates
    ///   No Dialog Normalization" for VERNUM ∉ {6, 7}. The field is
    ///   `UNSPEC` in this branch (the parser still surfaces the raw
    ///   bits via [`Self::dialog_normalization`]).
    ///
    /// Callers that need to distinguish the `Fixed` Table-5-20 mapping
    /// from the `Unspecified` zero-gain convention should use
    /// [`Self::dialog_normalization_gain`].
    pub fn dialog_normalization_db(&self) -> Option<i8> {
        Some(self.dialog_normalization_gain().gain_db())
    }

    /// Resolve [`Self::dialog_normalization`] to its
    /// [`DialogNormalization`] per ETSI TS 102 114 §5.3.1 Table 5-20.
    ///
    /// Richer counterpart to [`Self::dialog_normalization_db`]:
    /// preserves the distinction between the [`DialogNormalization::Fixed`]
    /// row of Table 5-20 (`VERNUM ∈ {6, 7}`) and the
    /// [`DialogNormalization::Unspecified`] `UNSPEC` convention
    /// (all other `VERNUM` values).
    pub fn dialog_normalization_gain(&self) -> DialogNormalization {
        dialog_normalization_from_codes(self.version, self.dialog_normalization)
    }

    /// Verify the 16-bit [`Self::header_crc`] against the bits
    /// covered by the DTS Core header-CRC contract.
    ///
    /// Always returns `None`, for two spec-mandated reasons
    /// (`docs/audio/dts/dts-crc16.md` "Where CRC-16 is applied"):
    ///
    /// - the Annex B algorithm itself is now documented (and
    ///   available as [`crate::dts_crc16`]), but §5.3.1 explicitly
    ///   states "The CRC value test **shall not be applied**" for the
    ///   core `HCRC` field (likewise `AHCRC` / `SICRC` / `OCRC`) —
    ///   the core check words are informational placeholders, not
    ///   integrity gates;
    /// - the exact protected span for `HCRC` is not normatively
    ///   pinned by the spec text ("core frame-header data"), so no
    ///   `Some(bool)` verdict could be computed defensibly anyway.
    ///
    /// The caller can use [`Self::header_crc`] directly for
    /// pass-through scenarios (e.g. re-muxing), and
    /// [`crate::dts_crc16`] to checksum any span it chooses. The
    /// genuinely verified DTS check words are the aux
    /// ([`crate::AuxData::crc_valid`]) and Rev2-aux
    /// ([`crate::Rev2AuxChunk::crc_valid`]) CRCs.
    pub fn verify_header_crc(&self) -> Option<bool> {
        // Normatively "shall not be applied"; see the doc comment.
        let _ = self.header_crc?;
        None
    }

    /// Total bit-length of the frame-sync header window, counted from
    /// the first bit of the syncword to the first bit of the SUBFRAMES
    /// region the wiki marks as `'''TODO'''`.
    ///
    /// The value is fully derived from the bit-table in the wiki
    /// snapshot (`docs/audio/dts/wiki/DTS.wiki`):
    ///
    /// | Region                              | Bits                |
    /// | ----------------------------------- | ------------------- |
    /// | Sync (32-bit raw / 14-bit packed)   | 32                  |
    /// | Base: FTYPE..RATE                   | 1+5+1+7+14+6+4+5=43 |
    /// | Trailing flags: DOWNMIX..PRED_HIST  | 1+1+1+1+1+3+1+1+2+1=13 |
    /// | Optional HEADER_CRC                 | 16 (iff `crc_present`) |
    /// | Post-CRC: MULTIRATE_INTER..DIALNORM | 1+4+2+3+1+1+4=16    |
    ///
    /// Total: 32 + 43 + 13 + 16 + (16 if `crc_present`) =
    /// `104` bits when `crc_present == 0`, `120` bits when
    /// `crc_present == 1`. Both totals are exact multiples of 8, so
    /// the SUBFRAMES region (the wiki's `'''TODO'''` cell) starts on
    /// a byte boundary.
    ///
    /// The value is in **raw 16-bit-stream bits** for raw-BE / raw-LE
    /// encodings. For the 14-bit-packed encodings the value still
    /// reflects the unpacked-bitstream count (i.e. what the parser
    /// consumed *after* [`crate::unpack_14bit_to_16bit`] has run); the
    /// container-byte advance for 14-bit input is a separate quantity
    /// (see `README.md`'s round-6 docs gap #7).
    pub fn header_bit_length(&self) -> u32 {
        // Sync(32) + base(43) + trailing(13) + post_crc(16) = 104.
        // Plus optional HEADER_CRC(16) when crc_present == true.
        const BASE_BITS: u32 = 32 + 43 + 13 + 16;
        if self.crc_present {
            BASE_BITS + 16
        } else {
            BASE_BITS
        }
    }

    /// Total byte-length of the frame-sync header window — the
    /// byte offset within the (raw-16-bit-equivalent) frame buffer at
    /// which the SUBFRAMES region the wiki marks `'''TODO'''`
    /// begins.
    ///
    /// Equivalent to `header_bit_length() / 8`. Always 13
    /// (`crc_present == false`) or 15 (`crc_present == true`)
    /// because both totals are exact multiples of 8 by construction.
    ///
    /// Useful for downstream subframe / payload decoders that need to
    /// know where the header ends and the SUBFRAMES region begins
    /// within a frame slice obtained from
    /// [`crate::iter_frames`] or directly from
    /// [`crate::parse_frame_header`].
    ///
    /// For 14-bit-packed input the value reflects the unpacked-stream
    /// byte count, not the container-byte count.
    pub fn header_byte_length(&self) -> usize {
        // header_bit_length() is always a multiple of 8 by the
        // arithmetic above; the assertion is for documentation /
        // debug builds only.
        let bits = self.header_bit_length();
        debug_assert_eq!(bits % 8, 0, "DTS header window must be byte-aligned");
        (bits / 8) as usize
    }

    /// Container-byte distance from this frame's syncword to the next
    /// frame's syncword, for a given wire encoding.
    ///
    /// Derived from **ETSI TS 102 114 V1.3.1 §5.3.1** (the `FSIZE`
    /// definition) and the 14-bit container-byte advance rule
    /// transcribed in `docs/audio/dts/dts-core-extracts.md` §3.3:
    ///
    /// - For the raw 16-bit encodings (`RawBigEndian` /
    ///   `RawLittleEndian`) the answer is just
    ///   [`Self::frame_size_bytes`]: `FSIZE+1` already counts bytes of
    ///   the on-wire 16-bit-word stream, which is the same as the
    ///   container-byte stream when no 14-bit re-packing is in effect.
    /// - For the 14-bit-packed encodings (`FourteenBitBigEndian` /
    ///   `FourteenBitLittleEndian`) the same `FSIZE+1` logical bytes
    ///   are carried in 14-bit-payload containers. Per §3.3 each
    ///   container word (= 16 container bits = **2 container bytes**)
    ///   carries 14 logical bits, so the span occupies
    ///   `ceil((FSIZE+1) * 8 / 14)` container **words** =
    ///   `2 * ceil((FSIZE+1) * 8 / 14)` container bytes (the partial
    ///   final word is padded out — the ETSI "28-bit-word boundary"
    ///   invariant in §6.1.3.1 / §6.3.x guarantees the next syncword
    ///   re-aligns on a 28-bit (i.e. two-container-word) boundary).
    ///
    /// The return type is [`u32`] because `frame_size_bytes` tops out
    /// at 16 384, the 14-bit scaling factor is 16 / 14 ≈ 1.143, and
    /// `16_384 * 16 / 14 + 1` comfortably fits.
    ///
    /// This accessor is the analytical half of round-6 docs gap #7
    /// (see `README.md`'s "Docs gaps"): it gives a multi-frame iterator
    /// the byte-count it needs to step from one 14-bit-packed sync to
    /// the next. The empirical half — actually walking a 14-bit
    /// container stream through [`crate::FrameIterator`] — is a
    /// follow-up that needs a streaming 14-bit-to-16-bit unpacker for
    /// the header window of each frame (because the parser reads its
    /// fields from the unpacked stream); this accessor lets that
    /// follow-up land without the formula having to be re-derived
    /// against `dts-core-extracts.md` §3.3 from scratch.
    ///
    /// # Examples
    ///
    /// ```
    /// use vuio_codec_dts::{parse_frame_header, SyncWordEncoding};
    ///
    /// // A 1024-byte raw-BE frame: container advance equals
    /// // frame_size_bytes exactly.
    /// # let bytes: &[u8] = &[];
    /// # if let Ok(hdr) = parse_frame_header(bytes) {
    /// assert_eq!(
    ///     hdr.frame_size_container_bytes(SyncWordEncoding::RawBigEndian),
    ///     hdr.frame_size_bytes as u32,
    /// );
    /// // The same frame's 14-bit-packed container distance is
    /// // ceil(1024 * 8 / 14) words = 586 words = 1172 bytes.
    /// # }
    /// ```
    pub fn frame_size_container_bytes(&self, encoding: SyncWordEncoding) -> u32 {
        let logical_bytes = self.frame_size_bytes as u32;
        if encoding.is_raw_16bit() {
            // FSIZE+1 already counts on-wire container bytes for the
            // raw encodings (the wiki notes raw-LE is the
            // 16-bit-word-swap of raw-BE; byte count is preserved).
            return logical_bytes;
        }
        // 14-bit-packed: 14 logical bits per 16 container bits.
        // ceil(logical_bytes * 8 / 14) container words; one word = 2
        // container bytes. Equivalent integer form: round the
        // logical-bit count up to the next multiple of 14, then
        // multiply by 2/14 = 1/7.
        let logical_bits = logical_bytes * 8;
        let container_words = logical_bits.div_ceil(14);
        container_words * 2
    }
}

/// Serialise a [`DtsFrameHeader`] back into the raw **little-endian**
/// on-wire byte representation of the frame-sync header window.
///
/// The output always begins with the canonical raw-LE sync `FE 7F 01
/// 80` regardless of the [`DtsFrameHeader::sync_word_encoding`] field
/// — the encoder emits the raw-LE form the parser already accepts via
/// the [`SyncWordEncoding::RawLittleEndian`] branch.
///
/// The wiki snapshot describes raw little-endian as "byte-swapped at
/// the 16-bit-word level" of the raw big-endian stream (see
/// `docs/audio/dts/wiki/DTS.wiki`'s sync table — `7F FE 80 01` ↔
/// `FE 7F 01 80`). This encoder therefore:
///
/// 1. calls [`encode_frame_header_be`] to build the 13 or 15 raw-BE
///    bytes,
/// 2. zero-pads them to **16 bytes** (the parser's minimum input
///    length for the raw-LE branch — the parser word-swaps a 16-byte
///    window and consumes 104 or 120 bits from it),
/// 3. byte-swaps each 16-bit word in place to produce the raw-LE
///    output.
///
/// The output is therefore always exactly 16 bytes long (regardless
/// of `header.crc_present`). The trailing 3 or 1 zero bytes correspond
/// to the first 24 or 8 bits of the SUBFRAMES region of a real DTS
/// frame; a caller muxing the encoder output back into a stream
/// should overwrite those bytes with the actual SUBFRAMES content
/// (after byte-swapping their 16-bit-word view to match).
///
/// The round-trip property:
///
/// ```text
///   parse_frame_header(&encode_frame_header_le(&hdr)) == Ok(hdr')
///   where hdr'.sync_word_encoding == SyncWordEncoding::RawBigEndian
///         hdr' == hdr on every other field
/// ```
///
/// holds exactly (no padding step needed by the caller). The parser
/// reports `RawBigEndian` because its normalisation step word-swaps
/// the raw-LE input back into a raw-BE scratch buffer before reading
/// the bit-table; the `sync_word_encoding` field is therefore the only
/// field that does not round-trip through the BE encoder, just as
/// `encode_frame_header_be` already documents.
///
/// Returns the same [`Error`] variants as [`encode_frame_header_be`]
/// for invalid headers (`BlockCountOutOfRange`, `FrameSizeOutOfRange`,
/// `FieldOutOfRange`).
pub fn encode_frame_header_le(header: &DtsFrameHeader) -> Result<Vec<u8>> {
    let mut be = encode_frame_header_be(header)?;
    // Zero-pad to the parser's minimum raw-LE input length (16). The
    // BE encoder returns 13 or 15 bytes; the LE branch of the parser
    // requires a 16-byte window so it can word-swap it before reading
    // the bit-table. Pad with zeros — the parser only consumes the
    // first `header_bit_length()` bits.
    be.resize(16, 0);
    debug_assert_eq!(be.len() % 2, 0, "raw-LE encoder works on 16-bit words");
    // Word-swap pairs in place.
    for pair in be.chunks_exact_mut(2) {
        pair.swap(0, 1);
    }
    Ok(be)
}

/// Serialise a [`DtsFrameHeader`] back into the raw-BE on-wire byte
/// representation of the frame-sync header window.
///
/// The output is exactly [`DtsFrameHeader::header_byte_length`] bytes
/// long (13 or 15, depending on [`DtsFrameHeader::crc_present`]), and
/// always begins with the 4-byte raw-BE sync `7F FE 80 01` regardless
/// of the [`DtsFrameHeader::sync_word_encoding`] field — the encoder
/// emits the canonical raw-BE form the parser already understands, so
/// a caller that needs the raw-LE / 14-bit-BE / 14-bit-LE encoding can
/// post-process the output (byte-swap pairs for raw-LE via
/// [`encode_frame_header_le`], repack 16→14-bit for the 14-bit
/// variants via [`crate::pack_16bit_to_14bit`]).
///
/// The bit layout is the wiki bit-table from
/// `docs/audio/dts/wiki/DTS.wiki`, MSB-first, in the same order
/// [`parse_frame_header`] consumes:
///
/// 1. 32-bit sync `0x7FFE_8001`.
/// 2. Base block (43 bits): FTYPE(1), SHORT(5), CRC_PRESENT(1),
///    NBLKS(7), FSIZE-1(14), AMODE(6), SFREQ(4), RATE(5).
/// 3. Trailing flags (13 bits): DOWNMIX(1), DYNRANGE(1), TIMSTP(1),
///    AUXDATA(1), HDCD(1), EXT_DESCR(3), EXT_CODING(1), ASPF(1),
///    LFE(2), PRED_HISTORY(1).
/// 4. Optional HEADER_CRC (16 bits) iff `header.crc_present` is set.
/// 5. Post-CRC window (16 bits): MULTIRATE_INTER(1), VERSION(4),
///    COPY_HISTORY(2), PCMR(3), FRONT_SUM(1), SURROUND_SUM(1),
///    DIALNORM(4).
///
/// The encoder validates the same field bounds [`parse_frame_header`]
/// enforces and is otherwise the inverse of [`parse_frame_header`].
/// The round-trip property:
///
/// ```text
///   parse_frame_header(&pad15(encode_frame_header_be(&hdr))) == Ok(hdr')
///   where hdr'.sync_word_encoding == SyncWordEncoding::RawBigEndian
///         hdr' == hdr on every other field
///   and   pad15(v) = v padded with zero bytes to length 15
/// ```
///
/// holds because the parser conservatively requires 15 bytes of input
/// (the worst-case `crc_present == 1` window) regardless of the
/// `crc_present` bit, while the encoder emits the actual
/// [`DtsFrameHeader::header_byte_length`] bytes (13 or 15). Callers
/// muxing the encoder output back into a stream should append the
/// SUBFRAMES region they already had (the parser tolerates any
/// trailing bytes); callers re-parsing a bare header should pad with
/// up to two zero bytes.
///
/// Returns:
/// - [`Error::BlockCountOutOfRange`] if `header.blocks_per_frame < 5`
///   or > 127 (NBLKS is a 7-bit field).
/// - [`Error::FrameSizeOutOfRange`] if `header.frame_size_bytes < 95`
///   or > 16384 (FSIZE-1 is a 14-bit field).
/// - [`Error::FieldOutOfRange`] if any other field is too large for
///   its documented bit width (AMODE > 63, SFREQ > 15, RATE > 31,
///   EXT_DESCR > 7, VERSION > 15, COPY_HISTORY > 3, PCMR > 7,
///   DIALNORM > 15, sample_count_per_block == 0 or > 32).
///
/// The encoder is the bounded primitive added in round 141; it
/// closes the parse/encode round-trip the wiki bit-table enables
/// without needing any of the docs-blocked value tables. Payload /
/// SUBFRAMES content remains the caller's responsibility — this
/// helper only owns the frame-sync header window.
pub fn encode_frame_header_be(header: &DtsFrameHeader) -> Result<Vec<u8>> {
    // Field-width validation. The parser enforces NBLKS and FSIZE
    // bounds; this encoder additionally enforces every field fits its
    // declared bit width so a caller cannot smuggle bits past the
    // boundary into the next field.
    if header.blocks_per_frame < 5 || header.blocks_per_frame > 127 {
        return Err(Error::BlockCountOutOfRange {
            blocks: header.blocks_per_frame,
        });
    }
    if !(95..=16384).contains(&header.frame_size_bytes) {
        return Err(Error::FrameSizeOutOfRange {
            frame_size: header.frame_size_bytes,
        });
    }
    // sample_count_per_block is stored as +1 of the SHORT field. The
    // SHORT field is 5 bits so the valid range is 0..=31, and the
    // stored value must be 1..=32.
    if header.sample_count_per_block == 0 || header.sample_count_per_block > 32 {
        return Err(Error::FieldOutOfRange {
            field: "sample_count_per_block",
            value: header.sample_count_per_block as u32,
            max: 32,
        });
    }
    if header.amode > 63 {
        return Err(Error::FieldOutOfRange {
            field: "amode",
            value: header.amode as u32,
            max: 63,
        });
    }
    if header.sfreq_index > 15 {
        return Err(Error::FieldOutOfRange {
            field: "sfreq_index",
            value: header.sfreq_index as u32,
            max: 15,
        });
    }
    if header.rate_index > 31 {
        return Err(Error::FieldOutOfRange {
            field: "rate_index",
            value: header.rate_index as u32,
            max: 31,
        });
    }
    if header.ext_descr > 7 {
        return Err(Error::FieldOutOfRange {
            field: "ext_descr",
            value: header.ext_descr as u32,
            max: 7,
        });
    }
    if header.version > 15 {
        return Err(Error::FieldOutOfRange {
            field: "version",
            value: header.version as u32,
            max: 15,
        });
    }
    if header.copy_history > 3 {
        return Err(Error::FieldOutOfRange {
            field: "copy_history",
            value: header.copy_history as u32,
            max: 3,
        });
    }
    if header.source_pcm_resolution_index > 7 {
        return Err(Error::FieldOutOfRange {
            field: "source_pcm_resolution_index",
            value: header.source_pcm_resolution_index as u32,
            max: 7,
        });
    }
    if header.dialog_normalization > 15 {
        return Err(Error::FieldOutOfRange {
            field: "dialog_normalization",
            value: header.dialog_normalization as u32,
            max: 15,
        });
    }
    // header_crc presence must agree with crc_present (encoder is
    // strict: silently dropping the value or silently emitting a
    // garbage 16-bit field would defeat the round-trip property).
    if header.crc_present != header.header_crc.is_some() {
        return Err(Error::FieldOutOfRange {
            field: "header_crc",
            value: header.header_crc.unwrap_or(0) as u32,
            max: 0,
        });
    }

    // Walk the same bit-table the parser consumes. We accumulate into
    // a small bit-vector and chunk to bytes MSB-first; the layout is
    // identical to the test helper `build_be_header` used in this
    // module's existing test grid (the helper now lives in
    // `#[cfg(test)]`, so externalising the logic as a public
    // primitive does not duplicate runtime code).
    let mut bits: Vec<bool> = Vec::with_capacity(120);

    fn push(bv: &mut Vec<bool>, value: u32, width: u32) {
        for i in (0..width).rev() {
            bv.push(((value >> i) & 1) == 1);
        }
    }

    // Sync (32 bits) — canonical raw-BE 0x7FFE_8001.
    push(&mut bits, 0x7FFE_8001, 32);
    // Base 43 bits.
    push(
        &mut bits,
        match header.frame_type {
            FrameType::Termination => 0,
            FrameType::Normal => 1,
        },
        1,
    );
    // SHORT = sample_count_per_block - 1.
    push(&mut bits, (header.sample_count_per_block - 1) as u32, 5);
    push(&mut bits, header.crc_present as u32, 1);
    push(&mut bits, header.blocks_per_frame as u32, 7);
    // FSIZE-1.
    push(&mut bits, (header.frame_size_bytes - 1) as u32, 14);
    push(&mut bits, header.amode as u32, 6);
    push(&mut bits, header.sfreq_index as u32, 4);
    push(&mut bits, header.rate_index as u32, 5);
    // Trailing 13 bits.
    push(&mut bits, header.downmix as u32, 1);
    push(&mut bits, header.dynamic_range as u32, 1);
    push(&mut bits, header.time_stamp as u32, 1);
    push(&mut bits, header.aux_data as u32, 1);
    push(&mut bits, header.hdcd as u32, 1);
    push(&mut bits, header.ext_descr as u32, 3);
    push(&mut bits, header.ext_coding as u32, 1);
    push(&mut bits, header.aspf as u32, 1);
    push(&mut bits, header.lfe.code() as u32, 2);
    push(&mut bits, header.predictor_history as u32, 1);
    // Optional HEADER_CRC.
    if let Some(crc) = header.header_crc {
        push(&mut bits, crc as u32, 16);
    }
    // Post-CRC 16 bits.
    push(&mut bits, header.multirate_inter as u32, 1);
    push(&mut bits, header.version as u32, 4);
    push(&mut bits, header.copy_history as u32, 2);
    push(&mut bits, header.source_pcm_resolution_index as u32, 3);
    push(&mut bits, header.front_sum as u32, 1);
    push(&mut bits, header.surround_sum as u32, 1);
    push(&mut bits, header.dialog_normalization as u32, 4);

    // The bit-table sums to 104 or 120 bits — both exact multiples of
    // 8 by the same arithmetic `header_bit_length()` documents. Assert
    // we wrote exactly `header_byte_length()` * 8 bits.
    debug_assert_eq!(
        bits.len() as u32,
        header.header_bit_length(),
        "encoder wrote a different bit-count than header_bit_length() reports"
    );

    let mut bytes = Vec::with_capacity(bits.len() / 8);
    for chunk in bits.chunks(8) {
        let mut b: u8 = 0;
        for (i, bit) in chunk.iter().enumerate() {
            if *bit {
                b |= 1 << (7 - i);
            }
        }
        bytes.push(b);
    }
    Ok(bytes)
}

/// Serialise a [`DtsFrameHeader`] back into the 14-bit-packed
/// **big-endian** on-wire byte representation of the frame-sync header
/// window.
///
/// This is the natural composition of [`encode_frame_header_be`] (which
/// produces the canonical raw-BE header bytes) and
/// [`crate::pack_16bit_to_14bit`] (which re-packs an MSB-first 16-bit
/// bit stream into 14-bit-payload containers per the wiki's "sign bit
/// extension" rule). The output always begins with the wiki-documented
/// 14-bit-BE sync prefix `1F FF E8 00 …` and represents the same
/// bit-content as [`encode_frame_header_be`] would, repacked into
/// 14-bit containers.
///
/// ## Output length
///
/// The output is always **18 bytes** long — the minimum input length
/// [`parse_frame_header_14bit`] accepts (nine 14-bit containers =
/// 126 payload bits, unpacking to 16 raw-BE bytes which covers the
/// worst-case 120-bit `crc_present == 1` header window). Both
/// `crc_present` states emit the same length so callers can mux the
/// output into a 14-bit container stream without branching on the
/// flag. The encoder pads the raw-BE header to 15 bytes (= 120 bits =
/// 9 × 14-bit containers minus 6 padding bits per container) before
/// packing:
///
/// | `crc_present` | raw-BE bytes | padded raw-BE | 14-bit containers | output bytes |
/// | ------------- | ------------ | ------------- | ----------------- | ------------ |
/// | `false`       | 13           | 15            | 9                 | 18           |
/// | `true`        | 15           | 15            | 9                 | 18           |
///
/// For the no-CRC case the two trailing zero bytes of the padded
/// raw-BE input land in what would be the first 16 bits of the
/// SUBFRAMES region of a real DTS frame; the parser only consumes
/// `header.header_bit_length()` bits from the unpacked stream, so the
/// padded zeros are inert for parsing purposes. A caller muxing the
/// encoder output back into a stream should overwrite them (after
/// unpacking) with the actual SUBFRAMES bytes.
///
/// ## Round-trip
///
/// `parse_frame_header_14bit(&encode_frame_header_14bit_be(&hdr))` recovers
/// `hdr` on every field except [`DtsFrameHeader::sync_word_encoding`],
/// which the parser reports as
/// [`SyncWordEncoding::FourteenBitBigEndian`] regardless of the input
/// header's value. This is the same round-trip behaviour
/// [`encode_frame_header_be`] / [`encode_frame_header_le`] document for
/// their respective sync encodings.
///
/// ## Errors
///
/// Returns the same [`Error`] variants as [`encode_frame_header_be`]
/// for invalid headers (`BlockCountOutOfRange`, `FrameSizeOutOfRange`,
/// `FieldOutOfRange`).
pub fn encode_frame_header_14bit_be(header: &DtsFrameHeader) -> Result<Vec<u8>> {
    let mut raw_be = encode_frame_header_be(header)?;
    // Pad the raw-BE bytes to 15 bytes (120 bits) so the pack step
    // emits exactly 9 containers = 18 bytes — the parser's minimum
    // 14-bit input length. 15 bytes is also the maximum
    // `header_byte_length()` value (the `crc_present == true` case), so
    // no header bits are dropped. For the no-CRC case the BE encoder
    // emits 13 bytes; we extend with 2 zero bytes that land in what
    // would be the first 16 bits of the SUBFRAMES region of a real
    // frame, which the parser does not consume.
    raw_be.resize(15, 0);
    let (packed, _payload_bit_count) =
        crate::unpack14::pack_16bit_to_14bit(&raw_be, FourteenBitByteOrder::BigEndian);
    debug_assert_eq!(
        packed.len(),
        18,
        "14-bit-BE encoded header must be exactly 18 bytes (9 containers)"
    );
    Ok(packed)
}

/// Serialise a [`DtsFrameHeader`] back into the 14-bit-packed
/// **little-endian** on-wire byte representation of the frame-sync
/// header window.
///
/// Same composition as [`encode_frame_header_14bit_be`] but with
/// [`FourteenBitByteOrder::LittleEndian`] selected for the pack step,
/// so each 16-bit container is emitted in little-endian byte order. The
/// output always begins with the wiki-documented 14-bit-LE sync prefix
/// `FF 1F 00 E8 …`.
///
/// Output length is the same as [`encode_frame_header_14bit_be`]: always
/// 18 bytes (regardless of `crc_present`). The 14-bit-LE output is
/// exactly the pairwise byte-swap of the 14-bit-BE output (each
/// two-byte container swapped independently), matching the wiki's
/// relationship between `1F FF E8 00 …` (BE) and `FF 1F 00 E8 …` (LE).
///
/// ## Round-trip
///
/// `parse_frame_header_14bit(&encode_frame_header_14bit_le(&hdr))`
/// recovers `hdr` on every field except
/// [`DtsFrameHeader::sync_word_encoding`], which the parser reports as
/// [`SyncWordEncoding::FourteenBitLittleEndian`] regardless of the
/// input header's value.
///
/// ## Errors
///
/// Returns the same [`Error`] variants as [`encode_frame_header_be`]
/// for invalid headers.
pub fn encode_frame_header_14bit_le(header: &DtsFrameHeader) -> Result<Vec<u8>> {
    let mut raw_be = encode_frame_header_be(header)?;
    // Same 15-byte padding rule as `encode_frame_header_14bit_be`.
    raw_be.resize(15, 0);
    let (packed, _payload_bit_count) =
        crate::unpack14::pack_16bit_to_14bit(&raw_be, FourteenBitByteOrder::LittleEndian);
    debug_assert_eq!(
        packed.len(),
        18,
        "14-bit-LE encoded header must be exactly 18 bytes (9 containers)"
    );
    Ok(packed)
}

/// Parse a single DTS Core frame-sync header from the start of
/// `bytes`.
///
/// The buffer must begin with one of the two **raw 16-bit** sync
/// sequences (`7F FE 80 01` or its byte-swapped form
/// `FE 7F 01 80`) and contain at least 15 bytes total: a 4-byte
/// sync plus the worst-case 88-bit header (= 11 bytes), which
/// applies when `CRC_PRESENT == 1` and the 16 round-5 post-CRC
/// bits are included. Returns:
/// - [`Error::UnexpectedEof`] on a short buffer.
/// - [`Error::NoSync`] if no documented sync sequence matches at
///   offset zero.
/// - [`Error::UnsupportedFourteenBit`] if a 14-bit-packed sync is
///   found at offset zero — callers with 14-bit input should use
///   [`parse_frame_header_14bit`] (or pre-unpack with
///   [`crate::unpack_14bit_to_16bit`]) instead.
///
/// The parser is non-allocating and side-effect free.
pub fn parse_frame_header(bytes: &[u8]) -> Result<DtsFrameHeader> {
    let sync = detect_sync(bytes)?;
    match sync {
        SyncWordEncoding::FourteenBitBigEndian | SyncWordEncoding::FourteenBitLittleEndian => {
            return Err(Error::UnsupportedFourteenBit);
        }
        _ => {}
    }

    // Normalise the buffer so that we always read the header from
    // a slice whose first 4 bytes are the big-endian sync. For
    // RawLittleEndian we byte-swap each 16-bit word in a small
    // scratch buffer; only the first ~16 bytes are needed.
    let normalised: Vec<u8>;
    let header_bytes: &[u8] = match sync {
        SyncWordEncoding::RawBigEndian => bytes,
        SyncWordEncoding::RawLittleEndian => {
            // We need 4 sync bytes + ceil(82 / 8) = 11 header bytes.
            // Round up to 16 (eight 16-bit words) so any 16-bit
            // word straddle stays inside the slice.
            let needed = 16;
            if bytes.len() < needed {
                return Err(Error::UnexpectedEof);
            }
            let mut scratch = Vec::with_capacity(needed);
            for chunk in bytes[..needed].chunks_exact(2) {
                scratch.push(chunk[1]);
                scratch.push(chunk[0]);
            }
            normalised = scratch;
            &normalised
        }
        // unreachable: 14-bit branches returned above.
        SyncWordEncoding::FourteenBitBigEndian | SyncWordEncoding::FourteenBitLittleEndian => {
            unreachable!()
        }
    };

    // Need at least 4 sync + 11 header bytes = 15 bytes to read the
    // worst-case header bits the round-5 parser consumes
    // (32 sync + 43 base + 13 trailing + 16 optional CRC + 16
    // post-CRC = 120 bits = exactly 15 bytes when CRC_PRESENT == 1;
    // 104 bits = 13 bytes otherwise). We accept 15.
    if header_bytes.len() < 15 {
        return Err(Error::UnexpectedEof);
    }

    let mut br = BitReader::from_byte_offset(header_bytes, 4);

    let ftype_raw = br.read_bit()?;
    let frame_type = if ftype_raw {
        FrameType::Normal
    } else {
        FrameType::Termination
    };
    let sample_count_minus_one = br.read_bits(5)? as u8;
    let sample_count_per_block = sample_count_minus_one + 1;
    let crc_present = br.read_bit()?;
    let nblks = br.read_bits(7)? as u8;
    if nblks < 5 {
        return Err(Error::BlockCountOutOfRange { blocks: nblks });
    }
    let fsize_minus_one = br.read_bits(14)? as u16;
    let frame_size_bytes = fsize_minus_one + 1;
    if frame_size_bytes < 95 {
        return Err(Error::FrameSizeOutOfRange {
            frame_size: frame_size_bytes,
        });
    }
    let amode = br.read_bits(6)? as u8;
    let sfreq_index = br.read_bits(4)? as u8;
    let rate_index = br.read_bits(5)? as u8;

    // Round 3: 13 bits of trailing single-bit / small-field flags.
    // Per the wiki snapshot, in this order:
    //   1 DOWNMIX, 1 DYNRANGE, 1 TIMSTP, 1 AUXDATA, 1 HDCD,
    //   3 EXT_DESCR, 1 EXT_CODING, 1 ASPF, 2 LFE, 1 PRED_HISTORY.
    let downmix = br.read_bit()?;
    let dynamic_range = br.read_bit()?;
    let time_stamp = br.read_bit()?;
    let aux_data = br.read_bit()?;
    let hdcd = br.read_bit()?;
    let ext_descr = br.read_bits(3)? as u8;
    let ext_coding = br.read_bit()?;
    let aspf = br.read_bit()?;
    let lfe_raw = br.read_bits(2)? as u8;
    let lfe = LfeMode::from_raw(lfe_raw);
    let predictor_history = br.read_bit()?;

    // Round 3: optional 16-bit HEADER_CRC field — present iff
    // CRC_PRESENT was set above.
    let header_crc = if crc_present {
        Some(br.read_bits(16)? as u16)
    } else {
        None
    };

    // Round 5: 16 bits of post-CRC trailing fields. Per the wiki,
    // in MSB-first order:
    //   1 MULTIRATE_INTER, 4 VERSION, 2 COPY_HISTORY,
    //   3 PCMR, 1 FRONT_SUM, 1 SURROUND_SUM, 4 DIALNORM.
    // These bits always follow the predictor-history (when
    // crc_present == 0) or the HEADER_CRC field (when set), so they
    // are consumed unconditionally.
    let multirate_inter = br.read_bit()?;
    let version = br.read_bits(4)? as u8;
    let copy_history = br.read_bits(2)? as u8;
    let source_pcm_resolution_index = br.read_bits(3)? as u8;
    let front_sum = br.read_bit()?;
    let surround_sum = br.read_bit()?;
    let dialog_normalization = br.read_bits(4)? as u8;

    Ok(DtsFrameHeader {
        sync_word_encoding: sync,
        frame_type,
        sample_count_per_block,
        crc_present,
        blocks_per_frame: nblks,
        frame_size_bytes,
        amode,
        sfreq_index,
        rate_index,
        downmix,
        dynamic_range,
        time_stamp,
        aux_data,
        hdcd,
        ext_descr,
        ext_coding,
        aspf,
        lfe,
        predictor_history,
        header_crc,
        multirate_inter,
        version,
        copy_history,
        source_pcm_resolution_index,
        front_sum,
        surround_sum,
        dialog_normalization,
    })
}

/// Parse a single DTS Core frame-sync header from a 14-bit-packed
/// buffer.
///
/// The buffer must start with one of the two 14-bit sync sequences
/// documented in `docs/audio/dts/wiki/DTS.wiki`
/// (`1F FF E8 00 07 Fx` for big-endian containers,
/// `FF 1F 00 E8 Fx 07` for little-endian containers). The function
/// runs [`crate::unpack_14bit_to_16bit`] to convert the input into
/// the raw-BE 16-bit form and then delegates to
/// [`parse_frame_header`].
///
/// Returns:
/// - [`Error::NoSync`] if the buffer does not start with a 14-bit
///   sync (callers should route raw 16-bit inputs to
///   [`parse_frame_header`] instead).
/// - [`Error::UnexpectedEof`] if the buffer has an odd length, or
///   if the unpacked stream is shorter than the 15 bytes the
///   header parser requires.
/// - the same out-of-range / EOF errors as [`parse_frame_header`]
///   once the unpack succeeds.
///
/// The unpacker output is byte-aligned every four containers
/// (4 × 14 = 56 bits); the header parser walks at most
/// sync + 56 header bits + 16 CRC bits = 104 bits → 13 bytes for
/// raw-BE input. The 14-bit-packed input therefore needs at least
/// `ceil(104 / 14) * 2 = 16` bytes (= eight 14-bit containers =
/// 112 bits ≥ 104). We require 18 bytes to keep a small margin and
/// to ensure the unpacked stream meets the 15-byte minimum the
/// raw-BE parser asserts up-front.
pub fn parse_frame_header_14bit(bytes: &[u8]) -> Result<DtsFrameHeader> {
    let sync = detect_sync(bytes)?;
    let order = match FourteenBitByteOrder::from_sync(sync) {
        Some(o) => o,
        None => {
            // Caller supplied a raw 16-bit sync to the 14-bit entry
            // point. Report NoSync to keep the two entry points'
            // accepted-input sets disjoint and unambiguous.
            return Err(Error::NoSync);
        }
    };
    // Need at least 18 input bytes (= 9 containers = 126 payload
    // bits = 15.75 unpacked bytes, rounded up to 16) so the parser
    // can read its 15-byte header window.
    if bytes.len() < 18 {
        return Err(Error::UnexpectedEof);
    }
    let unpacked = unpack_14bit_to_16bit(bytes, order)?;
    if unpacked.len() < 15 {
        return Err(Error::UnexpectedEof);
    }
    // After unpacking, the stream is raw-BE; delegate to the
    // existing parser. We override the returned sync_word_encoding
    // so callers see the original 14-bit variant rather than the
    // synthesised RawBigEndian one.
    let mut hdr = parse_frame_header(&unpacked)?;
    hdr.sync_word_encoding = sync;
    Ok(hdr)
}

/// Detect which of the four documented sync sequences (if any)
/// appears at the start of `bytes`. Public to the crate so tests can
/// exercise sync detection independently of header decoding.
///
/// For the two raw (16-bit) variants this is a literal byte-pattern
/// match against the wiki's documented prefixes.
///
/// For the two 14-bit variants the detector matches on the **lower
/// 14 bits** of each of the first three 16-bit containers, ignoring
/// the upper 2 bits of each container. This mirrors the unpacker
/// semantics (`docs/audio/dts/wiki/DTS.wiki` says the upper 2 bits
/// are sign-extension, which is informative-only when interpreting
/// the bytes as audio samples). The wiki's literal documented
/// prefixes (`1F FF E8 00 07 Fx` BE and `FF 1F 00 E8 Fx 07` LE) are
/// one specific instantiation of those payloads; sign-extended
/// instantiations encoding the same payloads are also valid 14-bit
/// DTS sync.
pub(crate) fn detect_sync(bytes: &[u8]) -> Result<SyncWordEncoding> {
    if bytes.len() < 4 {
        return Err(Error::UnexpectedEof);
    }
    // Raw 16-bit sequences (4 bytes).
    if bytes[..4] == [0x7F, 0xFE, 0x80, 0x01] {
        return Ok(SyncWordEncoding::RawBigEndian);
    }
    if bytes[..4] == [0xFE, 0x7F, 0x01, 0x80] {
        return Ok(SyncWordEncoding::RawLittleEndian);
    }
    // 14-bit sequences (6 bytes = three 16-bit containers carrying
    // 42 payload bits). The DTS syncword is 32 payload bits
    // (0x7FFE8001); a 14-bit-packed stream encodes those 32 bits
    // across containers 0/1 in full (14 + 14 = 28 bits) and the top
    // 4 bits of container 2 (28..32). Container 2's bottom 10 bits
    // carry frame-header data (FTYPE..NBLKS_high) and must NOT
    // participate in sync detection — earlier round-1 code matched
    // them too, which incidentally only accepted frames whose
    // FTYPE/deficit/CRC/NBLKS_high happened to be `1/31/1/000`.
    //
    // We confirm bits 0..31 of the unpacked payload equal
    // 0x7FFE_8001 by:
    //   container 0 lower 14 bits == 0x1FFF (covers bits 0..13)
    //   container 1 lower 14 bits == 0x2800 (covers bits 14..27)
    //   container 2 lower 14 bits, top 4 == 0b0001 (covers bits 28..31)
    if bytes.len() >= 6 {
        let c0_be = u16::from_be_bytes([bytes[0], bytes[1]]) & 0x3FFF;
        let c1_be = u16::from_be_bytes([bytes[2], bytes[3]]) & 0x3FFF;
        let c2_be = u16::from_be_bytes([bytes[4], bytes[5]]) & 0x3FFF;
        // c2's top 4 bits within its 14-bit payload: shift right 10
        // and mask to 4 bits.
        if c0_be == 0x1FFF && c1_be == 0x2800 && ((c2_be >> 10) & 0xF) == 0x1 {
            return Ok(SyncWordEncoding::FourteenBitBigEndian);
        }
        let c0_le = u16::from_le_bytes([bytes[0], bytes[1]]) & 0x3FFF;
        let c1_le = u16::from_le_bytes([bytes[2], bytes[3]]) & 0x3FFF;
        let c2_le = u16::from_le_bytes([bytes[4], bytes[5]]) & 0x3FFF;
        if c0_le == 0x1FFF && c1_le == 0x2800 && ((c2_le >> 10) & 0xF) == 0x1 {
            return Ok(SyncWordEncoding::FourteenBitLittleEndian);
        }
    }
    Err(Error::NoSync)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic raw-BE DTS frame header with explicit field
    /// values, in the bit order documented above.
    ///
    /// `extra_bits` are the 13 trailing header bits the parser
    /// consumes after RATE in round 3 (downmix .. predictor history),
    /// passed as a `u32` (only the bottom 13 bits used) so callers
    /// can spell the bit-pattern out literally. If
    /// `header_crc` is `Some`, the 16-bit CRC is emitted after the
    /// 13 trailing bits and `crc_present` should be `1`. `post_crc`
    /// (round 5) carries the 16 bits the wiki documents after the
    /// optional CRC field (multirate_inter, version, copy_history,
    /// PCMR, front_sum, surround_sum, dialnorm) MSB-first.
    #[allow(clippy::too_many_arguments)]
    fn build_be_header(
        ftype: u32,
        sample_count_m1: u32,    // 5 bits
        crc_present: u32,        // 1 bit
        nblks: u32,              // 7 bits
        fsize_m1: u32,           // 14 bits
        amode: u32,              // 6 bits
        sfreq: u32,              // 4 bits
        rate: u32,               // 5 bits
        extra_bits: u32,         // 13 bits (downmix..predictor)
        header_crc: Option<u32>, // 16 bits, only when crc_present == 1
        post_crc: u32,           // 16 bits (round-5 trailing window)
    ) -> Vec<u8> {
        // We will accumulate a bit-vector MSB-first and then chunk to
        // bytes.
        let mut bv: Vec<bool> = Vec::new();

        fn push(bv: &mut Vec<bool>, value: u32, width: u32) {
            for i in (0..width).rev() {
                bv.push(((value >> i) & 1) == 1);
            }
        }

        // 32-bit sync = 0x7FFE8001
        push(&mut bv, 0x7FFE_8001, 32);
        push(&mut bv, ftype, 1);
        push(&mut bv, sample_count_m1, 5);
        push(&mut bv, crc_present, 1);
        push(&mut bv, nblks, 7);
        push(&mut bv, fsize_m1, 14);
        push(&mut bv, amode, 6);
        push(&mut bv, sfreq, 4);
        push(&mut bv, rate, 5);
        push(&mut bv, extra_bits, 13);
        if let Some(crc) = header_crc {
            push(&mut bv, crc, 16);
        }
        // Round 5: 16 post-CRC bits always emitted (the wiki shows
        // them following the HEADER_CRC slot whether or not CRC is
        // present).
        push(&mut bv, post_crc, 16);
        // pad to whole bytes
        while !bv.len().is_multiple_of(8) {
            bv.push(false);
        }
        // pad to 16 bytes so the LE byte-swap path always has 16
        // bytes too if a caller chooses to reuse this builder.
        let mut bytes = Vec::with_capacity(bv.len() / 8);
        for chunk in bv.chunks(8) {
            let mut b: u8 = 0;
            for (i, bit) in chunk.iter().enumerate() {
                if *bit {
                    b |= 1 << (7 - i);
                }
            }
            bytes.push(b);
        }
        while bytes.len() < 16 {
            bytes.push(0);
        }
        bytes
    }

    #[test]
    fn detect_raw_be_sync() {
        let mut buf = vec![0; 16];
        buf[0] = 0x7F;
        buf[1] = 0xFE;
        buf[2] = 0x80;
        buf[3] = 0x01;
        assert_eq!(detect_sync(&buf).unwrap(), SyncWordEncoding::RawBigEndian);
    }

    #[test]
    fn detect_raw_le_sync() {
        let buf = [0xFE, 0x7F, 0x01, 0x80];
        assert_eq!(
            detect_sync(&buf).unwrap(),
            SyncWordEncoding::RawLittleEndian
        );
    }

    #[test]
    fn detect_14bit_be_sync() {
        let buf = [0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xFA];
        assert_eq!(
            detect_sync(&buf).unwrap(),
            SyncWordEncoding::FourteenBitBigEndian
        );
    }

    #[test]
    fn detect_14bit_le_sync() {
        let buf = [0xFF, 0x1F, 0x00, 0xE8, 0xF3, 0x07];
        assert_eq!(
            detect_sync(&buf).unwrap(),
            SyncWordEncoding::FourteenBitLittleEndian
        );
    }

    #[test]
    fn detect_no_sync_returns_error() {
        let buf = [0xDE, 0xAD, 0xBE, 0xEF];
        assert_eq!(detect_sync(&buf).unwrap_err(), Error::NoSync);
    }

    #[test]
    fn detect_short_buffer_returns_eof() {
        assert_eq!(detect_sync(&[0x7F]).unwrap_err(), Error::UnexpectedEof);
    }

    #[test]
    fn parse_normal_frame_be_typical() {
        // Typical values seen on a 48 kHz 1509 kbps 5.1 frame
        // (per the wiki's general bit-layout description; we do
        // not yet know the actual SFREQ/RATE/AMODE *codes* for
        // those Hz/bps/channels — pick arbitrary codes since the
        // parser only roundtrips the raw indices).
        let bytes = build_be_header(
            1,                  // FTYPE = normal
            31,                 // sample_count_m1 = 31 → 32 samples/block
            1,                  // CRC present
            15,                 // NBLKS = 15  (16 blocks)
            1023,               // FSIZE-1 = 1023 → frame size = 1024 bytes
            9,                  // AMODE = 9 (raw index)
            13,                 // SFREQ = 13
            25,                 // RATE = 25
            0b1_0100_1010_0011, // extra trailing 13 bits
            Some(0xC0DE),       // CRC field present
            0,                  // round-5 post-CRC bits (all zero)
        );

        let hdr = parse_frame_header(&bytes).unwrap();
        assert_eq!(hdr.sync_word_encoding, SyncWordEncoding::RawBigEndian);
        assert_eq!(hdr.frame_type, FrameType::Normal);
        assert_eq!(hdr.sample_count_per_block, 32);
        assert!(hdr.crc_present);
        assert_eq!(hdr.blocks_per_frame, 15);
        assert_eq!(hdr.frame_size_bytes, 1024);
        assert_eq!(hdr.amode, 9);
        assert_eq!(hdr.sfreq_index, 13);
        assert_eq!(hdr.rate_index, 25);
        // Round 3: trailing-13-bit flags decoded MSB-first from
        // 0b1_0100_1010_0011 → downmix=1, dyn=0, time=1, aux=0,
        // hdcd=0, ext_descr=101=5, ext_coding=0, aspf=0, lfe=01,
        // predictor=1.
        assert!(hdr.downmix);
        assert!(!hdr.dynamic_range);
        assert!(hdr.time_stamp);
        assert!(!hdr.aux_data);
        assert!(!hdr.hdcd);
        assert_eq!(hdr.ext_descr, 0b101);
        assert!(!hdr.ext_coding);
        assert!(!hdr.aspf);
        assert_eq!(hdr.lfe, LfeMode::Mode1);
        assert!(hdr.predictor_history);
        // CRC field present.
        assert_eq!(hdr.header_crc, Some(0xC0DE));
        // Round 5: post-CRC window all zeros.
        assert!(!hdr.multirate_inter);
        assert_eq!(hdr.version, 0);
        assert_eq!(hdr.copy_history, 0);
        assert_eq!(hdr.source_pcm_resolution_index, 0);
        assert!(!hdr.front_sum);
        assert!(!hdr.surround_sum);
        assert_eq!(hdr.dialog_normalization, 0);
    }

    #[test]
    fn parse_termination_frame_be() {
        let bytes = build_be_header(0, 0, 0, 5, 94, 0, 0, 0, 0, None, 0);
        let hdr = parse_frame_header(&bytes).unwrap();
        assert_eq!(hdr.frame_type, FrameType::Termination);
        assert_eq!(hdr.sample_count_per_block, 1);
        assert!(!hdr.crc_present);
        assert_eq!(hdr.blocks_per_frame, 5);
        assert_eq!(hdr.frame_size_bytes, 95);
        // All trailing flags zero by construction.
        assert!(!hdr.downmix);
        assert!(!hdr.dynamic_range);
        assert!(!hdr.time_stamp);
        assert!(!hdr.aux_data);
        assert!(!hdr.hdcd);
        assert_eq!(hdr.ext_descr, 0);
        assert!(!hdr.ext_coding);
        assert!(!hdr.aspf);
        assert_eq!(hdr.lfe, LfeMode::None);
        assert!(!hdr.predictor_history);
        // crc_present == 0 means no CRC field follows.
        assert_eq!(hdr.header_crc, None);
        // Round 5: post-CRC window all zeros.
        assert!(!hdr.multirate_inter);
        assert_eq!(hdr.version, 0);
        assert_eq!(hdr.copy_history, 0);
        assert_eq!(hdr.source_pcm_resolution_index, 0);
        assert!(!hdr.front_sum);
        assert!(!hdr.surround_sum);
        assert_eq!(hdr.dialog_normalization, 0);
    }

    #[test]
    fn parse_rejects_nblks_below_5() {
        let bytes = build_be_header(1, 31, 1, 4, 1023, 0, 0, 0, 0, Some(0), 0);
        assert_eq!(
            parse_frame_header(&bytes).unwrap_err(),
            Error::BlockCountOutOfRange { blocks: 4 }
        );
    }

    #[test]
    fn parse_rejects_frame_size_below_95() {
        let bytes = build_be_header(1, 31, 1, 16, 93, 0, 0, 0, 0, Some(0), 0);
        assert_eq!(
            parse_frame_header(&bytes).unwrap_err(),
            Error::FrameSizeOutOfRange { frame_size: 94 }
        );
    }

    #[test]
    fn parse_accepts_largest_documented_values() {
        // NBLKS = 127, FSIZE-1 = 16383 → 16384 bytes, AMODE = 63,
        // SFREQ = 15, RATE = 31 — all the max-index values the
        // wiki allows for these fields. Also exercises the
        // largest documented trailing-field codes: ext_descr=7,
        // lfe code 3 (Mode3), and all flag bits set.
        let bytes = build_be_header(
            1,
            31,
            1,
            127,
            16383,
            63,
            15,
            31,
            0b1_1111_1111_1111,
            Some(0xFFFF),
            0xFFFF,
        );
        let hdr = parse_frame_header(&bytes).unwrap();
        assert_eq!(hdr.blocks_per_frame, 127);
        assert_eq!(hdr.frame_size_bytes, 16384);
        assert_eq!(hdr.amode, 63);
        assert_eq!(hdr.sfreq_index, 15);
        assert_eq!(hdr.rate_index, 31);
        assert!(hdr.downmix);
        assert!(hdr.dynamic_range);
        assert!(hdr.time_stamp);
        assert!(hdr.aux_data);
        assert!(hdr.hdcd);
        assert_eq!(hdr.ext_descr, 0b111);
        assert!(hdr.ext_coding);
        assert!(hdr.aspf);
        assert_eq!(hdr.lfe, LfeMode::Mode3);
        assert!(hdr.predictor_history);
        assert_eq!(hdr.header_crc, Some(0xFFFF));
        // Round 5: max-value post-CRC window decodes to ext fields
        // at their max codes.
        assert!(hdr.multirate_inter);
        assert_eq!(hdr.version, 0b1111);
        assert_eq!(hdr.copy_history, 0b11);
        assert_eq!(hdr.source_pcm_resolution_index, 0b111);
        assert!(hdr.front_sum);
        assert!(hdr.surround_sum);
        assert_eq!(hdr.dialog_normalization, 0b1111);
    }

    #[test]
    fn parse_short_buffer_returns_eof() {
        let mut bytes = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, Some(0), 0);
        bytes.truncate(8);
        assert_eq!(
            parse_frame_header(&bytes).unwrap_err(),
            Error::UnexpectedEof
        );
    }

    #[test]
    fn parse_le_byteswapped_matches_be() {
        // Build BE bytes then byte-swap each 16-bit word; the
        // parsed structural fields must match the BE version
        // exactly (only the sync_word_encoding differs).
        let be = build_be_header(
            1,
            31,
            1,
            16,
            1023,
            9,
            13,
            25,
            0b1_0100_1010_0011,
            Some(0xBEEF),
            0xCAFE,
        );
        let mut le = Vec::with_capacity(be.len());
        for chunk in be.chunks_exact(2) {
            le.push(chunk[1]);
            le.push(chunk[0]);
        }
        // Sanity-check the sync was swapped to the LE variant.
        assert_eq!(&le[..4], &[0xFE, 0x7F, 0x01, 0x80]);
        let hdr_be = parse_frame_header(&be).unwrap();
        let hdr_le = parse_frame_header(&le).unwrap();
        assert_eq!(hdr_le.sync_word_encoding, SyncWordEncoding::RawLittleEndian);
        assert_eq!(hdr_le.frame_type, hdr_be.frame_type);
        assert_eq!(hdr_le.sample_count_per_block, hdr_be.sample_count_per_block);
        assert_eq!(hdr_le.crc_present, hdr_be.crc_present);
        assert_eq!(hdr_le.blocks_per_frame, hdr_be.blocks_per_frame);
        assert_eq!(hdr_le.frame_size_bytes, hdr_be.frame_size_bytes);
        assert_eq!(hdr_le.amode, hdr_be.amode);
        assert_eq!(hdr_le.sfreq_index, hdr_be.sfreq_index);
        assert_eq!(hdr_le.rate_index, hdr_be.rate_index);
        // Round 3 fields must also round-trip identically through
        // the LE byte-swap path.
        assert_eq!(hdr_le.downmix, hdr_be.downmix);
        assert_eq!(hdr_le.dynamic_range, hdr_be.dynamic_range);
        assert_eq!(hdr_le.time_stamp, hdr_be.time_stamp);
        assert_eq!(hdr_le.aux_data, hdr_be.aux_data);
        assert_eq!(hdr_le.hdcd, hdr_be.hdcd);
        assert_eq!(hdr_le.ext_descr, hdr_be.ext_descr);
        assert_eq!(hdr_le.ext_coding, hdr_be.ext_coding);
        assert_eq!(hdr_le.aspf, hdr_be.aspf);
        assert_eq!(hdr_le.lfe, hdr_be.lfe);
        assert_eq!(hdr_le.predictor_history, hdr_be.predictor_history);
        assert_eq!(hdr_le.header_crc, hdr_be.header_crc);
        // Round 5: post-CRC fields must also round-trip identically
        // through the LE byte-swap path.
        assert_eq!(hdr_le.multirate_inter, hdr_be.multirate_inter);
        assert_eq!(hdr_le.version, hdr_be.version);
        assert_eq!(hdr_le.copy_history, hdr_be.copy_history);
        assert_eq!(
            hdr_le.source_pcm_resolution_index,
            hdr_be.source_pcm_resolution_index
        );
        assert_eq!(hdr_le.front_sum, hdr_be.front_sum);
        assert_eq!(hdr_le.surround_sum, hdr_be.surround_sum);
        assert_eq!(hdr_le.dialog_normalization, hdr_be.dialog_normalization);
    }

    #[test]
    fn parse_14bit_be_returns_unsupported() {
        let mut buf = vec![0; 16];
        buf[..6].copy_from_slice(&[0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xFA]);
        assert_eq!(
            parse_frame_header(&buf).unwrap_err(),
            Error::UnsupportedFourteenBit
        );
    }

    #[test]
    fn parse_14bit_le_returns_unsupported() {
        let mut buf = vec![0; 16];
        buf[..6].copy_from_slice(&[0xFF, 0x1F, 0x00, 0xE8, 0xF3, 0x07]);
        assert_eq!(
            parse_frame_header(&buf).unwrap_err(),
            Error::UnsupportedFourteenBit
        );
    }

    /// Build a 14-bit BE-packed buffer carrying the same DTS frame
    /// the `build_be_header` helper produces in raw-BE form.
    #[allow(clippy::too_many_arguments)]
    fn build_14bit_packed_header(
        order: FourteenBitByteOrder,
        ftype: u32,
        sample_count_m1: u32,
        crc_present: u32,
        nblks: u32,
        fsize_m1: u32,
        amode: u32,
        sfreq: u32,
        rate: u32,
        extra_bits: u32,
        header_crc: Option<u32>,
        post_crc: u32,
    ) -> Vec<u8> {
        // Step 1: build the equivalent raw-BE byte buffer using the
        // existing helper.
        let raw_be = build_be_header(
            ftype,
            sample_count_m1,
            crc_present,
            nblks,
            fsize_m1,
            amode,
            sfreq,
            rate,
            extra_bits,
            header_crc,
            post_crc,
        );
        // Step 2: walk the raw bit stream MSB-first, emitting 14-bit
        // payloads packed into 16-bit containers in the requested
        // byte order.
        let mut packed: Vec<u8> = Vec::new();
        let mut bit_pos: usize = 0;
        let total_bits = raw_be.len() * 8;
        while bit_pos + 14 <= total_bits {
            let mut payload: u16 = 0;
            for i in 0..14 {
                let abs = bit_pos + i;
                let bit = (raw_be[abs / 8] >> (7 - (abs % 8))) & 1;
                payload = (payload << 1) | bit as u16;
            }
            // Sign-extend bit 13 into bits 14..16 per the wiki's
            // "upper two bits are sign bit extension" rule.
            let container = if payload & 0x2000 != 0 {
                payload | 0xC000
            } else {
                payload & 0x3FFF
            };
            let bytes = match order {
                FourteenBitByteOrder::BigEndian => container.to_be_bytes(),
                FourteenBitByteOrder::LittleEndian => container.to_le_bytes(),
            };
            packed.extend_from_slice(&bytes);
            bit_pos += 14;
        }
        packed
    }

    #[test]
    fn parse_frame_header_14bit_be_matches_raw_be() {
        let raw = build_be_header(
            1,
            31,
            1,
            16,
            1023,
            9,
            13,
            25,
            0b1_0100_1010_0011,
            Some(0xC0DE),
            0x9876,
        );
        let packed = build_14bit_packed_header(
            FourteenBitByteOrder::BigEndian,
            1,
            31,
            1,
            16,
            1023,
            9,
            13,
            25,
            0b1_0100_1010_0011,
            Some(0xC0DE),
            0x9876,
        );
        let hdr_raw = parse_frame_header(&raw).unwrap();
        let hdr_packed = parse_frame_header_14bit(&packed).unwrap();
        assert_eq!(
            hdr_packed.sync_word_encoding,
            SyncWordEncoding::FourteenBitBigEndian,
        );
        // Every structural field must agree with the raw-BE parse.
        assert_eq!(hdr_packed.frame_type, hdr_raw.frame_type);
        assert_eq!(
            hdr_packed.sample_count_per_block,
            hdr_raw.sample_count_per_block,
        );
        assert_eq!(hdr_packed.crc_present, hdr_raw.crc_present);
        assert_eq!(hdr_packed.blocks_per_frame, hdr_raw.blocks_per_frame);
        assert_eq!(hdr_packed.frame_size_bytes, hdr_raw.frame_size_bytes);
        assert_eq!(hdr_packed.amode, hdr_raw.amode);
        assert_eq!(hdr_packed.sfreq_index, hdr_raw.sfreq_index);
        assert_eq!(hdr_packed.rate_index, hdr_raw.rate_index);
        // Round 3: trailing flags + optional CRC must round-trip
        // identically through 14-bit packing.
        assert_eq!(hdr_packed.downmix, hdr_raw.downmix);
        assert_eq!(hdr_packed.dynamic_range, hdr_raw.dynamic_range);
        assert_eq!(hdr_packed.time_stamp, hdr_raw.time_stamp);
        assert_eq!(hdr_packed.aux_data, hdr_raw.aux_data);
        assert_eq!(hdr_packed.hdcd, hdr_raw.hdcd);
        assert_eq!(hdr_packed.ext_descr, hdr_raw.ext_descr);
        assert_eq!(hdr_packed.ext_coding, hdr_raw.ext_coding);
        assert_eq!(hdr_packed.aspf, hdr_raw.aspf);
        assert_eq!(hdr_packed.lfe, hdr_raw.lfe);
        assert_eq!(hdr_packed.predictor_history, hdr_raw.predictor_history);
        assert_eq!(hdr_packed.header_crc, hdr_raw.header_crc);
        // Round 5: post-CRC fields equivalent through 14-bit
        // packing too.
        assert_eq!(hdr_packed.multirate_inter, hdr_raw.multirate_inter);
        assert_eq!(hdr_packed.version, hdr_raw.version);
        assert_eq!(hdr_packed.copy_history, hdr_raw.copy_history);
        assert_eq!(
            hdr_packed.source_pcm_resolution_index,
            hdr_raw.source_pcm_resolution_index
        );
        assert_eq!(hdr_packed.front_sum, hdr_raw.front_sum);
        assert_eq!(hdr_packed.surround_sum, hdr_raw.surround_sum);
        assert_eq!(
            hdr_packed.dialog_normalization,
            hdr_raw.dialog_normalization
        );
    }

    #[test]
    fn parse_frame_header_14bit_le_matches_raw_be() {
        let raw = build_be_header(0, 0, 0, 5, 94, 0, 0, 0, 0, None, 0);
        let packed = build_14bit_packed_header(
            FourteenBitByteOrder::LittleEndian,
            0,
            0,
            0,
            5,
            94,
            0,
            0,
            0,
            0,
            None,
            0,
        );
        let hdr_raw = parse_frame_header(&raw).unwrap();
        let hdr_packed = parse_frame_header_14bit(&packed).unwrap();
        assert_eq!(
            hdr_packed.sync_word_encoding,
            SyncWordEncoding::FourteenBitLittleEndian,
        );
        assert_eq!(hdr_packed.frame_type, FrameType::Termination);
        assert_eq!(hdr_packed.frame_type, hdr_raw.frame_type);
        assert_eq!(hdr_packed.blocks_per_frame, hdr_raw.blocks_per_frame);
        assert_eq!(hdr_packed.frame_size_bytes, hdr_raw.frame_size_bytes);
        // No CRC when crc_present == 0.
        assert_eq!(hdr_packed.header_crc, None);
        // Round 5: post-CRC bits all zero by construction.
        assert!(!hdr_packed.multirate_inter);
        assert_eq!(hdr_packed.version, 0);
        assert_eq!(hdr_packed.dialog_normalization, 0);
    }

    /// `parse_frame_header_14bit` must reject a raw-16-bit buffer
    /// with `NoSync` so the two entry points stay disjoint.
    #[test]
    fn parse_frame_header_14bit_rejects_raw_be_input() {
        let raw = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, None, 0);
        assert_eq!(parse_frame_header_14bit(&raw).unwrap_err(), Error::NoSync,);
    }

    #[test]
    fn parse_frame_header_14bit_short_buffer_returns_eof() {
        // Just the 6-byte sync prefix is below the 18-byte minimum.
        let buf = [0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xF0];
        assert_eq!(
            parse_frame_header_14bit(&buf).unwrap_err(),
            Error::UnexpectedEof,
        );
    }

    #[test]
    fn parse_frame_header_14bit_value_resolvers_resolve_through_14bit_path() {
        let packed = build_14bit_packed_header(
            FourteenBitByteOrder::BigEndian,
            1,
            31,
            1,
            16,
            1023,
            9,
            13,
            25,
            0,
            Some(0),
            0,
        );
        let hdr = parse_frame_header_14bit(&packed).unwrap();
        // Round 202: SFREQ=13 → 48 kHz; AMODE=9 → ClRSlSr (5 channels);
        // PCMR=0 → 16-bit. RATE code 25 (0b11001) is documented as
        // invalid per Table 5-7, so bit_rate_bps() stays None (for a
        // now-documented reason — invalid code, not a missing table).
        assert_eq!(hdr.sample_rate_hz(), Some(48_000));
        assert_eq!(hdr.bit_rate_bps(), None);
        assert_eq!(hdr.targeted_bit_rate(), TargetedBitRate::Invalid);
        assert_eq!(hdr.channel_count(), Some(5));
        assert_eq!(hdr.amode_arrangement(), AmodeArrangement::ClRSlSr);
        assert_eq!(hdr.source_pcm_bits_per_sample(), Some(16));
        // Round 241: Table 5-20. `post_crc == 0` puts VERNUM = 0 (UNSPEC)
        // and DIALNORM = 0, so the gain is 0 dB by the §5.3.1 convention.
        assert_eq!(hdr.dialog_normalization_db(), Some(0));
        assert_eq!(
            hdr.dialog_normalization_gain(),
            DialogNormalization::Unspecified,
        );
    }

    #[test]
    fn parse_no_sync_returns_no_sync() {
        let buf = [0xDE, 0xAD, 0xBE, 0xEF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(parse_frame_header(&buf).unwrap_err(), Error::NoSync);
    }

    #[test]
    fn value_resolvers_resolve_per_round_202_tables() {
        let bytes = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, Some(0), 0);
        let hdr = parse_frame_header(&bytes).unwrap();
        // Round 202: SFREQ=13 → 48 kHz (Table 5-5); AMODE=9 →
        // ClRSlSr / 5-channel arrangement (Table 5-4); PCMR=0 →
        // 16-bit (Table 5-17).
        assert_eq!(hdr.sample_rate_hz(), Some(48_000));
        assert_eq!(hdr.channel_count(), Some(5));
        assert_eq!(hdr.amode_arrangement(), AmodeArrangement::ClRSlSr);
        assert_eq!(hdr.source_pcm_bits_per_sample(), Some(16));
        // RATE code 25 (0b11001) is an invalid Table 5-7 code →
        // bit_rate_bps() None, targeted_bit_rate() Invalid.
        assert_eq!(hdr.bit_rate_bps(), None);
        assert_eq!(hdr.targeted_bit_rate(), TargetedBitRate::Invalid);
        // Round 241: Table 5-20. `post_crc == 0` puts VERNUM = 0 (UNSPEC)
        // and DIALNORM = 0, so the gain is 0 dB by the §5.3.1 convention.
        assert_eq!(hdr.dialog_normalization_db(), Some(0));
        assert_eq!(
            hdr.dialog_normalization_gain(),
            DialogNormalization::Unspecified,
        );
    }

    /// Every fixed `RATE` code (0..=24) resolves to the Table 5-7
    /// bit-rate; the open code (29) and all reserved codes resolve to
    /// `Open` / `Invalid` respectively. The expected bps values are
    /// transcribed from ETSI §5.3.1 Table 5-7
    /// (`docs/audio/dts/dts-core-extracts.md` §1).
    #[test]
    fn rate_table_5_7_resolves_every_code() {
        // (code, expected fixed bps) for the 25 documented rates.
        let fixed: [(u8, u32); 25] = [
            (0, 32_000),
            (1, 56_000),
            (2, 64_000),
            (3, 96_000),
            (4, 112_000),
            (5, 128_000),
            (6, 192_000),
            (7, 224_000),
            (8, 256_000),
            (9, 320_000),
            (10, 384_000),
            (11, 448_000),
            (12, 512_000),
            (13, 576_000),
            (14, 640_000),
            (15, 768_000),
            (16, 960_000),
            (17, 1_024_000),
            (18, 1_152_000),
            (19, 1_280_000),
            (20, 1_344_000),
            (21, 1_408_000),
            (22, 1_411_200),
            (23, 1_472_000),
            (24, 1_536_000),
        ];
        for (code, bps) in fixed {
            // Build a minimal valid header carrying this RATE code.
            let bytes = build_be_header(1, 31, 0, 16, 1023, 2, 13, code as u32, 0, None, 0);
            let hdr = parse_frame_header(&bytes).unwrap();
            assert_eq!(hdr.rate_index, code);
            assert_eq!(
                hdr.targeted_bit_rate(),
                TargetedBitRate::Fixed(bps),
                "RATE code {code} should map to {bps} bps",
            );
            assert_eq!(hdr.bit_rate_bps(), Some(bps));
        }
        // Open code (0b11101 = 29).
        let open = build_be_header(1, 31, 0, 16, 1023, 2, 13, 29, 0, None, 0);
        let hdr = parse_frame_header(&open).unwrap();
        assert_eq!(hdr.targeted_bit_rate(), TargetedBitRate::Open);
        assert_eq!(hdr.bit_rate_bps(), None);
        // Every reserved code (25..=28, 30, 31) is Invalid.
        for code in [25u8, 26, 27, 28, 30, 31] {
            let bytes = build_be_header(1, 31, 0, 16, 1023, 2, 13, code as u32, 0, None, 0);
            let hdr = parse_frame_header(&bytes).unwrap();
            assert_eq!(
                hdr.targeted_bit_rate(),
                TargetedBitRate::Invalid,
                "RATE code {code} should be Invalid",
            );
            assert_eq!(hdr.bit_rate_bps(), None);
        }
    }

    // ---------------------------------------------------------------
    // Round 3 — trailing-13-bit field + optional 16-bit HEADER_CRC.
    // ---------------------------------------------------------------

    /// Walk every 2-bit LFE code (0..=3) and verify the [`LfeMode`]
    /// round-trips through the parser.
    #[test]
    fn lfe_mode_codes_round_trip() {
        for code in 0..=3u32 {
            // extra_bits layout (13 bits MSB-first): 11 leading
            // zeros + 2-bit LFE code + 0 predictor.
            //   bits 0..10 = 0  (downmix..aspf, 11 bits total)
            //   bits 11..12 = lfe code (we shift left 1 so
            //                 predictor bit stays 0)
            let extra = (code & 0b11) << 1;
            let bytes = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, extra, None, 0);
            let hdr = parse_frame_header(&bytes).unwrap();
            assert_eq!(hdr.lfe.code(), code as u8, "code {code}");
            assert_eq!(hdr.lfe.is_present(), code != 0, "is_present({code})");
            // Spot-check the typed enum mapping.
            let expected = match code {
                0 => LfeMode::None,
                1 => LfeMode::Mode1,
                2 => LfeMode::Mode2,
                _ => LfeMode::Mode3,
            };
            assert_eq!(hdr.lfe, expected, "enum mapping for code {code}");
        }
    }

    /// When `crc_present == 0` the parser must NOT consume the
    /// optional 16-bit CRC field; `header_crc` must be `None`.
    #[test]
    fn header_crc_absent_when_crc_present_bit_is_zero() {
        let bytes = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        let hdr = parse_frame_header(&bytes).unwrap();
        assert!(!hdr.crc_present);
        assert_eq!(hdr.header_crc, None);
        // verify_header_crc returns None when there is nothing to
        // verify.
        assert_eq!(hdr.verify_header_crc(), None);
    }

    /// When `crc_present == 1` the parser captures the 16-bit field
    /// verbatim; verification still returns `None` because §5.3.1
    /// mandates "The CRC value test shall not be applied" for the
    /// core HCRC (the Annex B algorithm itself is available as
    /// `dts_crc16` for callers that checksum their own spans).
    #[test]
    fn header_crc_present_returns_raw_field_and_unverified() {
        let bytes = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, Some(0x1234), 0);
        let hdr = parse_frame_header(&bytes).unwrap();
        assert!(hdr.crc_present);
        assert_eq!(hdr.header_crc, Some(0x1234));
        // Normative "shall not be applied" -> None by design.
        assert_eq!(hdr.verify_header_crc(), None);
    }

    /// All-zeros 13-bit trailing window decodes to all-false flags,
    /// `ext_descr == 0`, and `LfeMode::None`.
    #[test]
    fn trailing_bits_all_zero_decodes_clean() {
        let bytes = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        let hdr = parse_frame_header(&bytes).unwrap();
        assert!(!hdr.downmix);
        assert!(!hdr.dynamic_range);
        assert!(!hdr.time_stamp);
        assert!(!hdr.aux_data);
        assert!(!hdr.hdcd);
        assert_eq!(hdr.ext_descr, 0);
        assert!(!hdr.ext_coding);
        assert!(!hdr.aspf);
        assert_eq!(hdr.lfe, LfeMode::None);
        assert!(!hdr.predictor_history);
    }

    /// All-ones 13-bit trailing window decodes to all-true flags,
    /// `ext_descr == 7`, and `LfeMode::Mode3`.
    #[test]
    fn trailing_bits_all_one_decodes_max() {
        let bytes = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0b1_1111_1111_1111, None, 0);
        let hdr = parse_frame_header(&bytes).unwrap();
        assert!(hdr.downmix);
        assert!(hdr.dynamic_range);
        assert!(hdr.time_stamp);
        assert!(hdr.aux_data);
        assert!(hdr.hdcd);
        assert_eq!(hdr.ext_descr, 0b111);
        assert!(hdr.ext_coding);
        assert!(hdr.aspf);
        assert_eq!(hdr.lfe, LfeMode::Mode3);
        assert!(hdr.predictor_history);
    }

    // ---------------------------------------------------------------
    // Round 5 — post-CRC 16-bit trailing field window:
    // MULTIRATE_INTER + VERSION + COPY_HISTORY + PCMR + FRONT_SUM
    // + SURROUND_SUM + DIALNORM.
    // ---------------------------------------------------------------

    /// The bit packing of the post-CRC window is (MSB-first):
    ///   bit 15 = MULTIRATE_INTER, bits 14..11 = VERSION,
    ///   bits 10..9 = COPY_HISTORY, bits 8..6 = PCMR,
    ///   bit 5 = FRONT_SUM, bit 4 = SURROUND_SUM,
    ///   bits 3..0 = DIALNORM.
    ///
    /// Pick a value that exercises every sub-field at a non-extreme
    /// code and confirm the parser decomposes it correctly:
    /// MULTIRATE_INTER = 1, VERSION = 0b1010 = 10,
    /// COPY_HISTORY = 0b01 = 1, PCMR = 0b011 = 3, FRONT_SUM = 1,
    /// SURROUND_SUM = 0, DIALNORM = 0b1100 = 12.
    /// Packed: 1 1010 01 011 1 0 1100 = 0b1101001011101100 = 0xD2EC.
    #[test]
    fn post_crc_window_decomposes_into_individual_fields() {
        let bytes = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, Some(0), 0xD2EC);
        let hdr = parse_frame_header(&bytes).unwrap();
        assert!(hdr.multirate_inter);
        assert_eq!(hdr.version, 0b1010);
        assert_eq!(hdr.copy_history, 0b01);
        assert_eq!(hdr.source_pcm_resolution_index, 0b011);
        assert!(hdr.front_sum);
        assert!(!hdr.surround_sum);
        assert_eq!(hdr.dialog_normalization, 0b1100);
    }

    /// All-zero post-CRC window decodes to all-zero / all-false
    /// across every sub-field.
    #[test]
    fn post_crc_window_all_zero_decodes_to_zero_fields() {
        let bytes = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        let hdr = parse_frame_header(&bytes).unwrap();
        assert!(!hdr.multirate_inter);
        assert_eq!(hdr.version, 0);
        assert_eq!(hdr.copy_history, 0);
        assert_eq!(hdr.source_pcm_resolution_index, 0);
        assert!(!hdr.front_sum);
        assert!(!hdr.surround_sum);
        assert_eq!(hdr.dialog_normalization, 0);
    }

    /// All-ones post-CRC window decodes to every sub-field at its
    /// maximum code.
    #[test]
    fn post_crc_window_all_one_decodes_to_max_fields() {
        let bytes = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0xFFFF);
        let hdr = parse_frame_header(&bytes).unwrap();
        assert!(hdr.multirate_inter);
        assert_eq!(hdr.version, 0b1111);
        assert_eq!(hdr.copy_history, 0b11);
        assert_eq!(hdr.source_pcm_resolution_index, 0b111);
        assert!(hdr.front_sum);
        assert!(hdr.surround_sum);
        assert_eq!(hdr.dialog_normalization, 0b1111);
    }

    /// Walk every 3-bit PCMR code (0..=7) and confirm the parser
    /// preserves the raw index and that the round-202 resolver
    /// follows Table 5-17.
    #[test]
    fn pcmr_index_round_trips_for_every_3bit_code() {
        for code in 0..=7u32 {
            // Pack PCMR into the post-CRC window with every other
            // bit cleared so we isolate the field under test.
            let post_crc = (code & 0b111) << 6;
            let bytes = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, post_crc);
            let hdr = parse_frame_header(&bytes).unwrap();
            assert_eq!(
                hdr.source_pcm_resolution_index, code as u8,
                "PCMR code {code}"
            );
            // Round 202: resolver follows Table 5-17 (codes 4 and 7
            // are Invalid; the other six map to (bits, es) pairs).
            let exp = source_pcm_resolution_from_index(code as u8);
            assert_eq!(hdr.source_pcm_resolution(), exp);
            match exp {
                SourcePcmResolution::Valid { bits, .. } => {
                    assert_eq!(hdr.source_pcm_bits_per_sample(), Some(bits));
                }
                SourcePcmResolution::Invalid => {
                    assert_eq!(hdr.source_pcm_bits_per_sample(), None);
                }
            }
        }
    }

    /// Walk every 4-bit DIALNORM code (0..=15) and confirm the
    /// parser preserves the raw index. With `post_crc == code`, the
    /// VERNUM nibble at bits 14..11 of the post-CRC word is 0
    /// (UNSPEC) and the DIALNORM nibble at bits 3..0 carries `code`.
    /// Per Table 5-20's UNSPEC branch (§5.3.1), the gain is 0 dB for
    /// every code.
    #[test]
    fn dialnorm_code_round_trips_for_every_4bit_value() {
        for code in 0..=15u32 {
            let post_crc = code & 0b1111;
            let bytes = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, post_crc);
            let hdr = parse_frame_header(&bytes).unwrap();
            assert_eq!(hdr.dialog_normalization, code as u8, "DIALNORM code {code}");
            assert_eq!(hdr.version, 0, "VERNUM nibble must be 0 by construction");
            // VERNUM=0 → UNSPEC → DNG = 0 dB regardless of DIALNORM.
            assert_eq!(
                hdr.dialog_normalization_db(),
                Some(0),
                "UNSPEC (VERNUM ∉ {{6,7}}) → DNG = 0 dB per §5.3.1",
            );
            assert_eq!(
                hdr.dialog_normalization_gain(),
                DialogNormalization::Unspecified,
            );
        }
    }

    /// Round 241: every Table 5-20 row, exhaustively. For each
    /// `VERNUM ∈ {6, 7}` and `DIALNORM ∈ 0..=15`, build the post-CRC
    /// word that places `VERNUM` at bits 14..11 and `DIALNORM` at
    /// bits 3..0, then parse and assert that the resolver returns the
    /// Table 5-20 row's DNG (dB) verbatim. The remaining fourteen
    /// `VERNUM` values are exercised by
    /// [`dialnorm_unspec_branch_is_zero_db_for_every_other_vernum`].
    #[test]
    fn dialnorm_resolver_covers_table_5_20_verbatim() {
        // VERNUM == 7: codes 0..=15 → 0 dB down to -15 dB.
        for code in 0u32..=15 {
            let post_crc = (7u32 << 11) | code;
            let bytes = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, post_crc);
            let hdr = parse_frame_header(&bytes).unwrap();
            assert_eq!(hdr.version, 7);
            assert_eq!(hdr.dialog_normalization, code as u8);
            let expected = -(code as i8);
            assert_eq!(
                hdr.dialog_normalization_db(),
                Some(expected),
                "VERNUM=7 DIALNORM={code} → DNG {expected} dB",
            );
            assert_eq!(
                hdr.dialog_normalization_gain(),
                DialogNormalization::Fixed(expected),
            );
        }
        // VERNUM == 6: codes 0..=15 → -16 dB down to -31 dB.
        for code in 0u32..=15 {
            let post_crc = (6u32 << 11) | code;
            let bytes = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, post_crc);
            let hdr = parse_frame_header(&bytes).unwrap();
            assert_eq!(hdr.version, 6);
            assert_eq!(hdr.dialog_normalization, code as u8);
            let expected = -(code as i8) - 16;
            assert_eq!(
                hdr.dialog_normalization_db(),
                Some(expected),
                "VERNUM=6 DIALNORM={code} → DNG {expected} dB",
            );
            assert_eq!(
                hdr.dialog_normalization_gain(),
                DialogNormalization::Fixed(expected),
            );
        }
    }

    /// Round 241: the §5.3.1 UNSPEC branch. For every `VERNUM`
    /// outside `{6, 7}` and every 4-bit `DIALNORM` code, the resolver
    /// returns `Some(0)` / `Unspecified` (the spec's "DNG=0 indicates
    /// No Dialog Normalization" convention for non-named VERNUM
    /// values, PDF p.23).
    #[test]
    fn dialnorm_unspec_branch_is_zero_db_for_every_other_vernum() {
        for vernum in 0u32..=15 {
            if vernum == 6 || vernum == 7 {
                continue;
            }
            for code in 0u32..=15 {
                let post_crc = (vernum << 11) | code;
                let bytes = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, post_crc);
                let hdr = parse_frame_header(&bytes).unwrap();
                assert_eq!(hdr.version, vernum as u8);
                assert_eq!(hdr.dialog_normalization, code as u8);
                assert_eq!(
                    hdr.dialog_normalization_db(),
                    Some(0),
                    "VERNUM={vernum} DIALNORM={code} → DNG = 0 dB (UNSPEC)",
                );
                assert_eq!(
                    hdr.dialog_normalization_gain(),
                    DialogNormalization::Unspecified,
                );
            }
        }
    }

    /// Round 241: pure-function check on
    /// [`dialog_normalization_from_codes`]. Confirms the helper
    /// reproduces Table 5-20's boundary rows exactly and that only
    /// the low 4 bits of each input are consulted.
    #[test]
    fn dialog_normalization_from_codes_boundary_rows() {
        // VERNUM=7 boundary corners: (7, 0) → 0 dB; (7, 15) → -15 dB.
        assert_eq!(
            dialog_normalization_from_codes(7, 0),
            DialogNormalization::Fixed(0),
        );
        assert_eq!(
            dialog_normalization_from_codes(7, 15),
            DialogNormalization::Fixed(-15),
        );
        // VERNUM=6 boundary corners: (6, 0) → -16 dB; (6, 15) → -31 dB.
        assert_eq!(
            dialog_normalization_from_codes(6, 0),
            DialogNormalization::Fixed(-16),
        );
        assert_eq!(
            dialog_normalization_from_codes(6, 15),
            DialogNormalization::Fixed(-31),
        );
        // UNSPEC branch: a sample of non-{6,7} codes.
        for v in [0u8, 1, 2, 3, 4, 5, 8, 9, 10, 11, 12, 13, 14, 15] {
            assert_eq!(
                dialog_normalization_from_codes(v, 0),
                DialogNormalization::Unspecified,
            );
            assert_eq!(
                dialog_normalization_from_codes(v, 15),
                DialogNormalization::Unspecified,
            );
        }
        // High bits of either input are masked off — the resolver
        // consults only the documented 4-bit wire widths.
        assert_eq!(
            dialog_normalization_from_codes(0xF7, 0xF0),
            DialogNormalization::Fixed(0),
        );
        assert_eq!(
            dialog_normalization_from_codes(0xF6, 0xFF),
            DialogNormalization::Fixed(-31),
        );
    }

    /// Round 241: [`DialogNormalization::gain_db`] returns the spec's
    /// DNG value for both variants — the contained `i8` for `Fixed`,
    /// and `0` for `Unspecified`.
    #[test]
    fn dialog_normalization_gain_db_is_zero_for_unspecified() {
        assert_eq!(DialogNormalization::Unspecified.gain_db(), 0);
        for db in -31i8..=0 {
            assert_eq!(DialogNormalization::Fixed(db).gain_db(), db);
        }
    }

    /// Round 241: the resolver's range across every reachable
    /// `(VERNUM, DIALNORM)` pair is exactly `{0, -1, ..., -31}`.
    /// Cross-checks the Table 5-20 + UNSPEC implementation against
    /// the spec's stated dynamic range
    /// (§5.3.1: "Dialog Normalization Gain ... in dB").
    #[test]
    fn dialnorm_resolver_range_is_0_down_to_minus_31() {
        let mut seen = [false; 32];
        for v in 0u8..=15 {
            for d in 0u8..=15 {
                let db = match dialog_normalization_from_codes(v, d) {
                    DialogNormalization::Fixed(db) => db,
                    DialogNormalization::Unspecified => 0,
                };
                assert!((-31..=0).contains(&db), "DNG out of spec range");
                let bucket = (-db) as usize;
                seen[bucket] = true;
            }
        }
        // Every dB value in [-31..=0] must be reachable.
        for (i, hit) in seen.iter().enumerate() {
            assert!(*hit, "DNG {} dB not reachable", -(i as i32));
        }
    }

    /// Walk every 4-bit VERSION code (0..=15) and confirm the parser
    /// preserves the raw index.
    #[test]
    fn version_code_round_trips_for_every_4bit_value() {
        for code in 0..=15u32 {
            let post_crc = (code & 0b1111) << 11;
            let bytes = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, post_crc);
            let hdr = parse_frame_header(&bytes).unwrap();
            assert_eq!(hdr.version, code as u8, "VERSION code {code}");
        }
    }

    /// Walk every 2-bit COPY_HISTORY code (0..=3) and confirm the
    /// parser preserves the raw index.
    #[test]
    fn copy_history_code_round_trips_for_every_2bit_value() {
        for code in 0..=3u32 {
            let post_crc = (code & 0b11) << 9;
            let bytes = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, post_crc);
            let hdr = parse_frame_header(&bytes).unwrap();
            assert_eq!(hdr.copy_history, code as u8, "COPY_HISTORY code {code}");
        }
    }

    /// The post-CRC bits are consumed unconditionally — both when
    /// the optional HEADER_CRC slot is emitted (`crc_present == 1`)
    /// and when it is skipped (`crc_present == 0`). Build the same
    /// post-CRC payload twice with crc_present flipped and confirm
    /// every post-CRC field matches.
    #[test]
    fn post_crc_window_decodes_regardless_of_crc_present_flag() {
        let payload = 0xD2EC;
        let with_crc = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, Some(0xBEEF), payload);
        let without_crc = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, payload);
        let hdr_with = parse_frame_header(&with_crc).unwrap();
        let hdr_without = parse_frame_header(&without_crc).unwrap();
        assert_eq!(hdr_with.header_crc, Some(0xBEEF));
        assert_eq!(hdr_without.header_crc, None);
        // Post-CRC sub-fields must agree.
        assert_eq!(hdr_with.multirate_inter, hdr_without.multirate_inter);
        assert_eq!(hdr_with.version, hdr_without.version);
        assert_eq!(hdr_with.copy_history, hdr_without.copy_history);
        assert_eq!(
            hdr_with.source_pcm_resolution_index,
            hdr_without.source_pcm_resolution_index
        );
        assert_eq!(hdr_with.front_sum, hdr_without.front_sum);
        assert_eq!(hdr_with.surround_sum, hdr_without.surround_sum);
        assert_eq!(
            hdr_with.dialog_normalization,
            hdr_without.dialog_normalization
        );
    }

    // ---------------------------------------------------------------
    // Round 138 — header_bit_length() / header_byte_length()
    //
    // The wiki bit-table sums to 104 bits when `crc_present == 0` and
    // 120 bits when `crc_present == 1`. Both totals are exact
    // multiples of 8 by construction, so the SUBFRAMES region marked
    // `'''TODO'''` in the wiki begins on a byte boundary either way.
    // ---------------------------------------------------------------

    /// `header_bit_length()` returns exactly 104 bits when the
    /// optional HEADER_CRC slot is NOT present.
    #[test]
    fn header_bit_length_104_when_crc_absent() {
        let bytes = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        let hdr = parse_frame_header(&bytes).unwrap();
        assert!(!hdr.crc_present);
        assert_eq!(hdr.header_bit_length(), 104);
        assert_eq!(hdr.header_byte_length(), 13);
    }

    /// `header_bit_length()` returns exactly 120 bits when the
    /// optional HEADER_CRC slot IS present.
    #[test]
    fn header_bit_length_120_when_crc_present() {
        let bytes = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, Some(0xBEEF), 0);
        let hdr = parse_frame_header(&bytes).unwrap();
        assert!(hdr.crc_present);
        assert_eq!(hdr.header_bit_length(), 120);
        assert_eq!(hdr.header_byte_length(), 15);
    }

    /// The header-length value matches the byte position the parser's
    /// internal bit reader is left at after a full parse. We confirm
    /// this by walking the same bit-table independently and observing
    /// the sum agrees with the public accessor.
    #[test]
    fn header_bit_length_matches_manual_wiki_table_sum() {
        // Wiki sub-totals from `docs/audio/dts/wiki/DTS.wiki`.
        let sync = 32u32;
        let base = 1 + 5 + 1 + 7 + 14 + 6 + 4 + 5; // 43
        let trailing = 1 + 1 + 1 + 1 + 1 + 3 + 1 + 1 + 2 + 1; // 13
        let post_crc = 1 + 4 + 2 + 3 + 1 + 1 + 4; // 16
        let crc_slot = 16; // optional

        let absent = build_be_header(1, 31, 0, 5, 94, 0, 0, 0, 0, None, 0);
        let hdr_absent = parse_frame_header(&absent).unwrap();
        assert_eq!(
            hdr_absent.header_bit_length(),
            sync + base + trailing + post_crc,
        );

        let present = build_be_header(1, 31, 1, 5, 94, 0, 0, 0, 0, Some(0), 0);
        let hdr_present = parse_frame_header(&present).unwrap();
        assert_eq!(
            hdr_present.header_bit_length(),
            sync + base + trailing + post_crc + crc_slot,
        );
    }

    /// `header_byte_length()` is always a multiple of 8 bits (i.e.
    /// the SUBFRAMES region starts on a byte boundary), for both
    /// CRC-absent and CRC-present frames and for every combination of
    /// the structural fields surfaced through `build_be_header`.
    #[test]
    fn header_byte_length_is_always_byte_aligned() {
        for crc in [0, 1] {
            for nblks in [5u32, 16, 127] {
                for fsize_m1 in [94u32, 1023, 16383] {
                    let crc_payload = if crc == 1 { Some(0) } else { None };
                    let bytes =
                        build_be_header(1, 31, crc, nblks, fsize_m1, 0, 0, 0, 0, crc_payload, 0);
                    let hdr = parse_frame_header(&bytes).unwrap();
                    assert_eq!(
                        hdr.header_bit_length() % 8,
                        0,
                        "bit length must be byte-aligned (crc={crc} nblks={nblks} fsize_m1={fsize_m1})"
                    );
                    assert_eq!(
                        hdr.header_byte_length() * 8,
                        hdr.header_bit_length() as usize,
                    );
                }
            }
        }
    }

    /// The 14-bit-packed entry point exposes the same
    /// `header_bit_length()` value as the equivalent raw-BE frame:
    /// the byte-length is in unpacked-bitstream bits per the doc
    /// comment, not in 14-bit container bits.
    #[test]
    fn header_bit_length_14bit_matches_raw_be() {
        let raw = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, Some(0xC0DE), 0);
        let packed = build_14bit_packed_header(
            FourteenBitByteOrder::BigEndian,
            1,
            31,
            1,
            16,
            1023,
            9,
            13,
            25,
            0,
            Some(0xC0DE),
            0,
        );
        let hdr_raw = parse_frame_header(&raw).unwrap();
        let hdr_packed = parse_frame_header_14bit(&packed).unwrap();
        assert_eq!(hdr_raw.header_bit_length(), 120);
        assert_eq!(hdr_packed.header_bit_length(), 120);
        assert_eq!(
            hdr_raw.header_byte_length(),
            hdr_packed.header_byte_length()
        );
    }

    // ---------------------------------------------------------------
    // Round 189 — frame_size_container_bytes(): 14-bit container-byte
    // advance rule per ETSI TS 102 114 V1.3.1 §5.3.1 + the
    // 14↔16-bit advance synthesis in
    // `docs/audio/dts/dts-core-extracts.md` §3.3.
    //
    // For the raw encodings the advance equals `frame_size_bytes`
    // verbatim (FSIZE+1 already counts container bytes). For the
    // 14-bit encodings the same logical span occupies
    // `ceil(frame_size_bytes * 8 / 14)` container words = twice that
    // many container bytes (one container word = 2 container bytes
    // = 16 container bits = 14 logical bits).
    // ---------------------------------------------------------------

    /// Returning the bare `frame_size_bytes` for the two raw 16-bit
    /// encodings is the explicit `FSIZE+1` contract from
    /// `docs/audio/dts/dts-core-extracts.md` §3.1.
    #[test]
    fn frame_size_container_bytes_raw_equals_frame_size_bytes() {
        let bytes = build_be_header(1, 31, 0, 16, 1023, 0, 0, 0, 0, None, 0);
        let hdr = parse_frame_header(&bytes).unwrap();
        assert_eq!(hdr.frame_size_bytes, 1024);
        assert_eq!(
            hdr.frame_size_container_bytes(SyncWordEncoding::RawBigEndian),
            1024,
        );
        assert_eq!(
            hdr.frame_size_container_bytes(SyncWordEncoding::RawLittleEndian),
            1024,
        );
    }

    /// 14-bit container advance for a 1024-byte logical frame is
    /// `ceil(1024 * 8 / 14)` container words = `ceil(8192 / 14)` =
    /// 586 words = 1172 container bytes (the ETSI §6.1.3.1 /§6.3.x
    /// "28-bit-word boundary" invariant — two container words per
    /// 28 logical bits — guarantees the next syncword re-aligns).
    #[test]
    fn frame_size_container_bytes_14bit_1024_logical_is_1172_container() {
        let bytes = build_be_header(1, 31, 0, 16, 1023, 0, 0, 0, 0, None, 0);
        let hdr = parse_frame_header(&bytes).unwrap();
        assert_eq!(hdr.frame_size_bytes, 1024);
        assert_eq!(
            hdr.frame_size_container_bytes(SyncWordEncoding::FourteenBitBigEndian),
            1172,
        );
        assert_eq!(
            hdr.frame_size_container_bytes(SyncWordEncoding::FourteenBitLittleEndian),
            1172,
        );
    }

    /// Minimum-frame (FSIZE+1 = 95) and maximum-frame (FSIZE+1 =
    /// 16384) advances exercise the formula's bottom and top.
    /// `ceil(95 * 8 / 14)` = `ceil(760 / 14)` = 55 words = 110
    /// container bytes; `ceil(16384 * 8 / 14)` = `ceil(131072 / 14)`
    /// = 9363 words = 18726 container bytes (the last container word
    /// carries 760 mod 14 = 4 leftover bits in the minimum case
    /// and 131072 mod 14 = 10 leftover bits in the maximum case;
    /// each leftover bit-count is non-zero so the closed-form
    /// ceiling rounds up to a full container word).
    #[test]
    fn frame_size_container_bytes_14bit_min_and_max() {
        let bytes_min = build_be_header(1, 31, 0, 5, 94, 0, 0, 0, 0, None, 0);
        let hdr_min = parse_frame_header(&bytes_min).unwrap();
        assert_eq!(hdr_min.frame_size_bytes, 95);
        assert_eq!(
            hdr_min.frame_size_container_bytes(SyncWordEncoding::FourteenBitBigEndian),
            110,
        );

        let bytes_max = build_be_header(1, 31, 0, 127, 16383, 0, 0, 0, 0, None, 0);
        let hdr_max = parse_frame_header(&bytes_max).unwrap();
        assert_eq!(hdr_max.frame_size_bytes, 16384);
        assert_eq!(
            hdr_max.frame_size_container_bytes(SyncWordEncoding::FourteenBitBigEndian),
            18726,
        );
    }

    /// Encoding-agnostic invariant: the 14-bit container advance is
    /// always strictly greater than the raw advance (because 16
    /// container bits carry only 14 logical bits, so any non-zero
    /// logical span needs at least one extra container byte) and the
    /// difference is bounded by `ceil(frame_size_bytes * 2 / 14) +
    /// 1` — i.e. the 14/16 scaling overhead plus at most one
    /// rounding-up word.
    #[test]
    fn frame_size_container_bytes_14bit_is_strictly_greater_than_raw() {
        for fsize_m1 in [94u32, 511, 1023, 2047, 4095, 8191, 16383] {
            let bytes = build_be_header(1, 31, 0, 16, fsize_m1, 0, 0, 0, 0, None, 0);
            let hdr = parse_frame_header(&bytes).unwrap();
            let raw = hdr.frame_size_container_bytes(SyncWordEncoding::RawBigEndian);
            let packed = hdr.frame_size_container_bytes(SyncWordEncoding::FourteenBitBigEndian);
            assert!(
                packed > raw,
                "14-bit container advance must exceed raw (fsize_bytes={}, raw={}, packed={})",
                hdr.frame_size_bytes,
                raw,
                packed,
            );
            // Upper bound: scaling factor is exactly 16/14, plus
            // at most one extra container word (2 bytes) of
            // round-up.
            let ub = (raw * 16).div_ceil(14) + 2;
            assert!(
                packed <= ub,
                "14-bit advance overshoots scaling bound (fsize_bytes={}, raw={}, packed={}, ub={})",
                hdr.frame_size_bytes,
                raw,
                packed,
                ub,
            );
        }
    }

    /// BE vs LE container-byte advance is identical: 14-bit-LE is the
    /// pairwise byte-swap of 14-bit-BE per the wiki, so the
    /// container-byte count is invariant under the BE/LE flip (and
    /// likewise for the raw pair, where raw-LE = 16-bit-word-swap
    /// of raw-BE).
    #[test]
    fn frame_size_container_bytes_be_le_equivalence() {
        for fsize_m1 in [94u32, 1023, 16383] {
            let bytes = build_be_header(1, 31, 0, 16, fsize_m1, 0, 0, 0, 0, None, 0);
            let hdr = parse_frame_header(&bytes).unwrap();
            assert_eq!(
                hdr.frame_size_container_bytes(SyncWordEncoding::RawBigEndian),
                hdr.frame_size_container_bytes(SyncWordEncoding::RawLittleEndian),
            );
            assert_eq!(
                hdr.frame_size_container_bytes(SyncWordEncoding::FourteenBitBigEndian),
                hdr.frame_size_container_bytes(SyncWordEncoding::FourteenBitLittleEndian),
            );
        }
    }

    /// The container-byte advance is always even for the 14-bit
    /// encodings because a partial 14-bit word is padded out to a
    /// full 16-bit (two-byte) container by the §3.3 / §6.1.3.1
    /// "28-bit-word boundary" invariant — the next syncword
    /// re-aligns on a two-container-word boundary so the per-frame
    /// step lands on an even byte count.
    #[test]
    fn frame_size_container_bytes_14bit_is_even() {
        for fsize_m1 in [94u32, 100, 511, 1023, 1535, 16383] {
            let bytes = build_be_header(1, 31, 0, 16, fsize_m1, 0, 0, 0, 0, None, 0);
            let hdr = parse_frame_header(&bytes).unwrap();
            let n = hdr.frame_size_container_bytes(SyncWordEncoding::FourteenBitBigEndian);
            assert_eq!(
                n % 2,
                0,
                "14-bit container advance must be even (fsize_bytes={}, advance={})",
                hdr.frame_size_bytes,
                n,
            );
        }
    }

    /// Manual closed-form cross-check: `frame_size_container_bytes`
    /// for a 14-bit encoding must equal
    /// `2 * ceil(frame_size_bytes * 8 / 14)` (i.e. the rounded-up
    /// container-word count times two container bytes per word).
    #[test]
    fn frame_size_container_bytes_14bit_matches_closed_form() {
        for fsize_m1 in [94u32, 200, 400, 1023, 8192, 16383] {
            let bytes = build_be_header(1, 31, 0, 16, fsize_m1, 0, 0, 0, 0, None, 0);
            let hdr = parse_frame_header(&bytes).unwrap();
            let logical_bits = (hdr.frame_size_bytes as u32) * 8;
            let expected = 2 * logical_bits.div_ceil(14);
            let actual = hdr.frame_size_container_bytes(SyncWordEncoding::FourteenBitBigEndian);
            assert_eq!(
                expected, actual,
                "closed-form mismatch (fsize_bytes={}, expected={}, actual={})",
                hdr.frame_size_bytes, expected, actual,
            );
        }
    }

    // ---------------------------------------------------------------
    // Round 141 — encode_frame_header_be(): parse ↔ encode round-trip.
    //
    // The encoder is the inverse of `parse_frame_header` against the
    // wiki bit-table. Every structural field round-trips bit-exact;
    // the encoder's output is always exactly `header_byte_length()`
    // bytes long and starts with the canonical raw-BE sync regardless
    // of the source `sync_word_encoding`.
    // ---------------------------------------------------------------

    /// A synthesised non-trivial header with every structural field
    /// set to a distinctive value round-trips through encode → parse
    /// with bit-exact equality on every field except
    /// `sync_word_encoding` (the encoder always emits raw-BE).
    #[test]
    fn encode_round_trip_non_trivial_with_crc() {
        let bytes_in = build_be_header(
            1,                  // FTYPE
            31,                 // SHORT
            1,                  // CRC present
            16,                 // NBLKS
            1023,               // FSIZE-1
            9,                  // AMODE
            13,                 // SFREQ
            25,                 // RATE
            0b1_0100_1010_0011, // 13 trailing bits
            Some(0xC0DE),       // HEADER_CRC
            0xD2EC,             // 16 post-CRC bits
        );
        let hdr = parse_frame_header(&bytes_in).unwrap();

        let encoded = encode_frame_header_be(&hdr).expect("encode must succeed");
        // header_byte_length() reports 15 because crc_present is set.
        assert_eq!(encoded.len(), hdr.header_byte_length());
        assert_eq!(encoded.len(), 15);

        // The encoded output begins with the canonical raw-BE sync.
        assert_eq!(&encoded[..4], &[0x7F, 0xFE, 0x80, 0x01]);

        // The header bytes byte-for-byte match the synthesised input
        // (build_be_header pads to 16 bytes; encode_frame_header_be
        // emits exactly 15 because crc_present is set).
        assert_eq!(&encoded[..], &bytes_in[..encoded.len()]);

        // Re-parse the encoded bytes and confirm every field is
        // identical except `sync_word_encoding`.
        let mut hdr_round = parse_frame_header(&encoded).unwrap();
        assert_eq!(hdr_round.sync_word_encoding, SyncWordEncoding::RawBigEndian);
        hdr_round.sync_word_encoding = hdr.sync_word_encoding;
        assert_eq!(hdr_round, hdr);
    }

    /// Termination frame with no CRC: encoder emits exactly 13 bytes.
    /// The 13-byte header window is shorter than the 15-byte minimum
    /// the parser requires (the parser always reads up to the
    /// worst-case CRC-present 120-bit window before discriminating);
    /// for the round-trip we pad the encoder output with two
    /// scratch-SUBFRAMES bytes (the actual SUBFRAMES region begins
    /// immediately after the 13-byte header anyway).
    #[test]
    fn encode_round_trip_termination_no_crc_minimal() {
        let bytes_in = build_be_header(0, 0, 0, 5, 94, 0, 0, 0, 0, None, 0);
        let hdr = parse_frame_header(&bytes_in).unwrap();

        let encoded = encode_frame_header_be(&hdr).unwrap();
        assert_eq!(encoded.len(), 13);
        assert_eq!(encoded.len(), hdr.header_byte_length());

        // Bytes 0..13 must match the synthesised input (build_be_header
        // pads to 16 but the meaningful header window is 13 bytes).
        assert_eq!(&encoded[..], &bytes_in[..13]);

        let mut padded = encoded.clone();
        padded.extend_from_slice(&[0u8; 2]);
        let hdr_round = parse_frame_header(&padded).unwrap();
        assert_eq!(hdr_round.frame_type, FrameType::Termination);
        assert_eq!(hdr_round.sample_count_per_block, 1);
        assert!(!hdr_round.crc_present);
        assert_eq!(hdr_round.blocks_per_frame, 5);
        assert_eq!(hdr_round.frame_size_bytes, 95);
        // Sync_word_encoding is the only differing field by design.
        let mut hdr_norm = hdr_round;
        hdr_norm.sync_word_encoding = hdr.sync_word_encoding;
        assert_eq!(hdr_norm, hdr);
    }

    /// Field-bound enforcement: NBLKS < 5 is rejected by the encoder
    /// (mirrors the parser bound).
    #[test]
    fn encode_rejects_nblks_below_5() {
        let hdr = synth_hdr(|h| h.blocks_per_frame = 4);
        let err = encode_frame_header_be(&hdr).unwrap_err();
        assert_eq!(err, Error::BlockCountOutOfRange { blocks: 4 });
    }

    /// Field-bound enforcement: NBLKS > 127 cannot fit the 7-bit
    /// field.
    #[test]
    fn encode_rejects_nblks_above_127() {
        let hdr = synth_hdr(|h| h.blocks_per_frame = 128);
        let err = encode_frame_header_be(&hdr).unwrap_err();
        assert_eq!(err, Error::BlockCountOutOfRange { blocks: 128 });
    }

    /// Field-bound enforcement: FSIZE < 95 is rejected (parser bound).
    #[test]
    fn encode_rejects_frame_size_below_95() {
        let hdr = synth_hdr(|h| h.frame_size_bytes = 94);
        let err = encode_frame_header_be(&hdr).unwrap_err();
        assert_eq!(err, Error::FrameSizeOutOfRange { frame_size: 94 });
    }

    /// Field-bound enforcement: FSIZE > 16384 cannot fit the 14-bit
    /// FSIZE-1 field (max 16383+1 = 16384).
    #[test]
    fn encode_rejects_frame_size_above_16384() {
        let hdr = synth_hdr(|h| h.frame_size_bytes = 16385);
        let err = encode_frame_header_be(&hdr).unwrap_err();
        assert_eq!(err, Error::FrameSizeOutOfRange { frame_size: 16385 });
    }

    /// Field-bound enforcement: AMODE > 63 cannot fit the 6-bit field.
    #[test]
    fn encode_rejects_amode_above_63() {
        let hdr = synth_hdr(|h| h.amode = 64);
        let err = encode_frame_header_be(&hdr).unwrap_err();
        assert_eq!(
            err,
            Error::FieldOutOfRange {
                field: "amode",
                value: 64,
                max: 63
            }
        );
    }

    /// Field-bound enforcement: PCMR > 7 cannot fit the 3-bit field.
    #[test]
    fn encode_rejects_pcmr_above_7() {
        let hdr = synth_hdr(|h| h.source_pcm_resolution_index = 8);
        let err = encode_frame_header_be(&hdr).unwrap_err();
        assert_eq!(
            err,
            Error::FieldOutOfRange {
                field: "source_pcm_resolution_index",
                value: 8,
                max: 7,
            }
        );
    }

    /// Field-bound enforcement: VERSION > 15 cannot fit the 4-bit
    /// field.
    #[test]
    fn encode_rejects_version_above_15() {
        let hdr = synth_hdr(|h| h.version = 16);
        let err = encode_frame_header_be(&hdr).unwrap_err();
        assert_eq!(
            err,
            Error::FieldOutOfRange {
                field: "version",
                value: 16,
                max: 15,
            }
        );
    }

    /// Field-bound enforcement: `header_crc.is_some()` must match
    /// `crc_present`. A `Some(_)` payload with `crc_present == false`
    /// is rejected so a silent emit-or-drop bug cannot break the
    /// round-trip.
    #[test]
    fn encode_rejects_crc_payload_without_crc_present() {
        let hdr = synth_hdr(|h| {
            h.crc_present = false;
            h.header_crc = Some(0x1234);
        });
        let err = encode_frame_header_be(&hdr).unwrap_err();
        match err {
            Error::FieldOutOfRange { field, .. } => assert_eq!(field, "header_crc"),
            other => panic!("expected FieldOutOfRange{{field: header_crc}}, got {other:?}"),
        }
    }

    /// Mirror: `crc_present == true` with `header_crc == None` is
    /// also rejected (no silent zeroing of the field).
    #[test]
    fn encode_rejects_crc_present_without_payload() {
        let hdr = synth_hdr(|h| {
            h.crc_present = true;
            h.header_crc = None;
        });
        let err = encode_frame_header_be(&hdr).unwrap_err();
        match err {
            Error::FieldOutOfRange { field, .. } => assert_eq!(field, "header_crc"),
            other => panic!("expected FieldOutOfRange{{field: header_crc}}, got {other:?}"),
        }
    }

    /// Encoding a header parsed from the raw-LE input still emits the
    /// canonical raw-BE on-wire bytes — only `sync_word_encoding`
    /// differs in the re-parsed result.
    #[test]
    fn encode_normalises_le_input_to_raw_be_output() {
        let raw_be = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        // Byte-swap each pair to obtain the raw-LE form.
        let raw_le: Vec<u8> = raw_be.chunks_exact(2).flat_map(|c| [c[1], c[0]]).collect();
        let hdr_le = parse_frame_header(&raw_le).unwrap();
        assert_eq!(hdr_le.sync_word_encoding, SyncWordEncoding::RawLittleEndian);

        let encoded = encode_frame_header_be(&hdr_le).unwrap();
        // First 4 bytes are the canonical raw-BE sync, NOT the
        // byte-swapped LE form.
        assert_eq!(&encoded[..4], &[0x7F, 0xFE, 0x80, 0x01]);

        // Pad to the parser's 15-byte minimum (encoded.len() is 13
        // because crc_present is false here).
        let mut padded = encoded.clone();
        padded.extend_from_slice(&[0u8; 2]);
        let hdr_round = parse_frame_header(&padded).unwrap();
        assert_eq!(hdr_round.sync_word_encoding, SyncWordEncoding::RawBigEndian);
        // Every other field is preserved.
        let mut hdr_norm = hdr_round;
        hdr_norm.sync_word_encoding = hdr_le.sync_word_encoding;
        assert_eq!(hdr_norm, hdr_le);
    }

    /// Exhaustive grid: for every documented LFE code, both CRC
    /// states, and a representative {NBLKS, FSIZE} pair, the encoded
    /// output round-trips back through the parser.
    #[test]
    fn encode_round_trip_grid_lfe_crc_states() {
        for crc_state in [false, true] {
            for lfe_code in 0u8..=3 {
                for &(nblks, fsize) in &[(5u8, 95u16), (16u8, 1024u16), (127u8, 16384u16)] {
                    let crc_arg = if crc_state { Some(0xBEEF) } else { None };
                    let bytes = build_be_header(
                        1,
                        31,
                        crc_state as u32,
                        nblks as u32,
                        (fsize - 1) as u32,
                        9,
                        13,
                        25,
                        // Stuff in the LFE code at the right offset
                        // within the 13-bit trailing slot: positions
                        // MSB-first are 1+1+1+1+1+3+1+1+(2)+1 = LFE at
                        // bit-offset 9..11 (0-indexed from MSB), so
                        // the 2-bit field sits at bit (12 - 9 .. 12 -
                        // 9 + 2) within the 13-bit value, i.e. shift
                        // left by 1. Easier: encode through the
                        // accessor route rather than hand-bitfiddling.
                        ((lfe_code as u32) & 0b11) << 1,
                        crc_arg,
                        0,
                    );
                    let hdr = parse_frame_header(&bytes).unwrap();
                    assert_eq!(hdr.lfe.code(), lfe_code);
                    assert_eq!(hdr.crc_present, crc_state);

                    let encoded = encode_frame_header_be(&hdr).unwrap();
                    assert_eq!(encoded.len(), hdr.header_byte_length());

                    // Pad the encoded output to the parser's 15-byte
                    // minimum: for crc_present=false the encoder
                    // emits 13 bytes, while the parser conservatively
                    // requires the 120-bit worst-case window.
                    let mut padded = encoded.clone();
                    while padded.len() < 15 {
                        padded.push(0);
                    }
                    let hdr_round = parse_frame_header(&padded).unwrap();
                    let mut hdr_norm = hdr_round;
                    hdr_norm.sync_word_encoding = hdr.sync_word_encoding;
                    assert_eq!(hdr_norm, hdr);
                }
            }
        }
    }

    /// `encode_frame_header_be(parse(b))` reproduces the prefix of the
    /// real ffmpeg fixture byte-for-byte (the public FFMPEG fixture
    /// lives in `tests/black_box_ffmpeg.rs`; we re-inline the first
    /// 16 bytes here for a unit-test-level assertion).
    #[test]
    fn encode_reproduces_ffmpeg_fixture_header_prefix() {
        // Same bytes as in tests/black_box_ffmpeg.rs.
        let ffmpeg_bytes: [u8; 16] = [
            0x7f, 0xfe, 0x80, 0x01, 0xfc, 0x3c, 0x3f, 0xf0, 0xb5, 0xe0, 0x01, 0x38, 0x00, 0x03,
            0xef, 0x7f,
        ];
        let hdr = parse_frame_header(&ffmpeg_bytes).unwrap();
        // ffmpeg's frame has crc_present == false, so the header
        // window is 13 bytes long.
        assert!(!hdr.crc_present);
        assert_eq!(hdr.header_byte_length(), 13);

        let encoded = encode_frame_header_be(&hdr).unwrap();
        assert_eq!(encoded.len(), 13);
        assert_eq!(&encoded[..], &ffmpeg_bytes[..13]);
    }

    /// Helper for the bounds tests: build a baseline well-formed
    /// header from `build_be_header` defaults and then let the caller
    /// mutate a single field before encoding.
    fn synth_hdr(mutate: impl FnOnce(&mut DtsFrameHeader)) -> DtsFrameHeader {
        let bytes = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        let mut h = parse_frame_header(&bytes).unwrap();
        mutate(&mut h);
        h
    }

    // ---------------------------------------------------------------
    // Round 145 — encode_frame_header_le(): raw-LE encoder variant.
    //
    // The raw-LE encoder is `encode_frame_header_be` + zero-pad to 16
    // bytes + word-swap pairs. The output always starts with the
    // canonical raw-LE sync `FE 7F 01 80` and is exactly 16 bytes
    // long; the parser's raw-LE branch consumes the first
    // `header_bit_length()` bits (104 or 120) and ignores the trailing
    // zero padding.
    // ---------------------------------------------------------------

    /// The first 4 bytes of the encoder output are the canonical
    /// raw-LE sync regardless of the input header's
    /// `sync_word_encoding`.
    #[test]
    fn encode_le_emits_canonical_raw_le_sync() {
        let bytes_in = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        let hdr = parse_frame_header(&bytes_in).unwrap();
        let encoded = encode_frame_header_le(&hdr).unwrap();
        assert_eq!(&encoded[..4], &[0xFE, 0x7F, 0x01, 0x80]);
    }

    /// Encoder output is always exactly 16 bytes regardless of
    /// `crc_present` (the parser's raw-LE branch reads a 16-byte
    /// window).
    #[test]
    fn encode_le_is_always_16_bytes() {
        // crc_present == false: BE encoder emits 13 bytes; LE pads to
        // 16.
        let bytes_no_crc = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        let hdr_no_crc = parse_frame_header(&bytes_no_crc).unwrap();
        assert!(!hdr_no_crc.crc_present);
        assert_eq!(encode_frame_header_le(&hdr_no_crc).unwrap().len(), 16);

        // crc_present == true: BE encoder emits 15 bytes; LE pads to
        // 16.
        let bytes_crc = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, Some(0xCAFE), 0xD2EC);
        let hdr_crc = parse_frame_header(&bytes_crc).unwrap();
        assert!(hdr_crc.crc_present);
        assert_eq!(encode_frame_header_le(&hdr_crc).unwrap().len(), 16);
    }

    /// Bit-for-bit equivalence with the manual word-swap of the BE
    /// encoder output: `LE == swap16(BE.padded_to_16())`.
    #[test]
    fn encode_le_equals_word_swapped_be_padded() {
        let bytes_in = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, Some(0xCAFE), 0xD2EC);
        let hdr = parse_frame_header(&bytes_in).unwrap();
        let be = encode_frame_header_be(&hdr).unwrap();
        let le = encode_frame_header_le(&hdr).unwrap();
        // Pad BE to 16 and word-swap.
        let mut expected = be.clone();
        expected.resize(16, 0);
        for pair in expected.chunks_exact_mut(2) {
            pair.swap(0, 1);
        }
        assert_eq!(le, expected);
    }

    /// `parse_frame_header(&encode_frame_header_le(&hdr))` round-trips
    /// every field except `sync_word_encoding` (which always reports
    /// `RawBigEndian` after parsing because the parser word-swaps the
    /// LE input back into raw-BE scratch — but the input's first 4
    /// bytes were the raw-LE sync, so the parser reports
    /// `RawLittleEndian`).
    #[test]
    fn encode_le_round_trips_through_parser_no_crc() {
        let bytes_in = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        let hdr = parse_frame_header(&bytes_in).unwrap();
        let encoded_le = encode_frame_header_le(&hdr).unwrap();
        let hdr_round = parse_frame_header(&encoded_le).unwrap();
        // The parser reports RawLittleEndian because that's the sync
        // it detected at the start of the input.
        assert_eq!(
            hdr_round.sync_word_encoding,
            SyncWordEncoding::RawLittleEndian
        );
        // Every other field is preserved.
        let mut hdr_norm = hdr_round;
        hdr_norm.sync_word_encoding = hdr.sync_word_encoding;
        assert_eq!(hdr_norm, hdr);
    }

    /// Same as above with crc_present == true.
    #[test]
    fn encode_le_round_trips_through_parser_with_crc() {
        let bytes_in = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, Some(0xCAFE), 0xD2EC);
        let hdr = parse_frame_header(&bytes_in).unwrap();
        let encoded_le = encode_frame_header_le(&hdr).unwrap();
        let hdr_round = parse_frame_header(&encoded_le).unwrap();
        assert_eq!(
            hdr_round.sync_word_encoding,
            SyncWordEncoding::RawLittleEndian
        );
        let mut hdr_norm = hdr_round;
        hdr_norm.sync_word_encoding = hdr.sync_word_encoding;
        assert_eq!(hdr_norm, hdr);
    }

    /// `encode_frame_header_le` inherits the same field-bound checks
    /// as `encode_frame_header_be` (they share the underlying call).
    /// Spot-check NBLKS bound here so a future refactor can't drop
    /// the validation silently.
    #[test]
    fn encode_le_rejects_nblks_below_5() {
        let hdr = synth_hdr(|h| h.blocks_per_frame = 4);
        let err = encode_frame_header_le(&hdr).unwrap_err();
        assert_eq!(err, Error::BlockCountOutOfRange { blocks: 4 });
    }

    /// Spot-check the `header_crc` / `crc_present` mismatch is also
    /// rejected by the LE wrapper.
    #[test]
    fn encode_le_rejects_crc_payload_mismatch() {
        let hdr = synth_hdr(|h| {
            h.crc_present = false;
            h.header_crc = Some(0xBEEF);
        });
        let err = encode_frame_header_le(&hdr).unwrap_err();
        match err {
            Error::FieldOutOfRange { field, .. } => assert_eq!(field, "header_crc"),
            other => panic!("expected FieldOutOfRange{{field: header_crc}}, got {other:?}"),
        }
    }

    /// Exhaustive grid: every documented LFE code × both CRC states ×
    /// representative {NBLKS, FSIZE} pairs round-trip through the LE
    /// encoder.
    #[test]
    fn encode_le_round_trip_grid_lfe_crc_states() {
        for crc_state in [false, true] {
            for lfe_code in 0u8..=3 {
                for &(nblks, fsize) in &[(5u8, 95u16), (16u8, 1024u16), (127u8, 16384u16)] {
                    let crc_arg = if crc_state { Some(0xBEEF) } else { None };
                    let bytes = build_be_header(
                        1,
                        31,
                        crc_state as u32,
                        nblks as u32,
                        (fsize - 1) as u32,
                        9,
                        13,
                        25,
                        ((lfe_code as u32) & 0b11) << 1,
                        crc_arg,
                        0,
                    );
                    let hdr = parse_frame_header(&bytes).unwrap();
                    let encoded = encode_frame_header_le(&hdr).unwrap();
                    assert_eq!(encoded.len(), 16);
                    assert_eq!(&encoded[..4], &[0xFE, 0x7F, 0x01, 0x80]);
                    let hdr_round = parse_frame_header(&encoded).unwrap();
                    assert_eq!(
                        hdr_round.sync_word_encoding,
                        SyncWordEncoding::RawLittleEndian
                    );
                    let mut hdr_norm = hdr_round;
                    hdr_norm.sync_word_encoding = hdr.sync_word_encoding;
                    assert_eq!(hdr_norm, hdr);
                }
            }
        }
    }

    /// Reproducing the real ffmpeg fixture's first 16 bytes as a
    /// raw-LE on-wire payload: byte-swap the BE bytes pairwise and
    /// confirm the encoder matches.
    #[test]
    fn encode_le_reproduces_ffmpeg_fixture_byte_swapped() {
        let ffmpeg_be: [u8; 16] = [
            0x7f, 0xfe, 0x80, 0x01, 0xfc, 0x3c, 0x3f, 0xf0, 0xb5, 0xe0, 0x01, 0x38, 0x00, 0x03,
            0xef, 0x7f,
        ];
        let hdr = parse_frame_header(&ffmpeg_be).unwrap();
        assert!(!hdr.crc_present);

        // Manual byte-swap of the BE fixture (the on-wire raw-LE form
        // a Wave-container-encapsulated DTS-on-CD would carry).
        let mut expected = [0u8; 16];
        for i in 0..8 {
            expected[i * 2] = ffmpeg_be[i * 2 + 1];
            expected[i * 2 + 1] = ffmpeg_be[i * 2];
        }
        let encoded = encode_frame_header_le(&hdr).unwrap();
        // The BE encoder for this header returns 13 bytes (crc absent),
        // padded to 16 with three zero bytes. The trailing 3 bytes of
        // the BE-padded-to-16 buffer are `00 00 00`; after word-swap
        // the trailing 3 bytes of the LE output are also `00 00 00`.
        // The ffmpeg fixture's bytes 13..16 are `00 03 ef 7f` (real
        // SUBFRAMES content) — those bytes won't match our zero
        // padding. Compare only the first 13 bytes (the header window
        // proper) plus byte 13 of `expected`... actually the LE
        // encoder pads bytes 13..16 with zeros, so word-swap puts
        // zeros at LE bytes 12..16 only if BE bytes 12..16 were also
        // zero — which they aren't (BE byte 12 is `0x00`, byte 13 is
        // `0x03` from real fixture). So only compare the first 12
        // bytes (6 full 16-bit words) which are unambiguous.
        assert_eq!(&encoded[..12], &expected[..12]);
        // Byte 12 of BE is part of the header (it's BE byte 12 = `0x00`
        // = first byte of post-CRC window's continuation, the header's
        // last byte). Encoder padded BE to 16 with zeros at indices
        // 13..16, so LE encoder's index 13 corresponds to BE's index
        // 12. expected[13] is the byte-swap of (BE[12], BE[13]) at
        // index 1 = BE[12] = 0x00; encoded[13] is the byte-swap of
        // (BE[12], 0) at index 1 = BE[12] = 0x00. They match.
        assert_eq!(encoded[13], expected[13]);
    }

    // ---------------------------------------------------------------
    // Round 148 — encode_frame_header_14bit_{be,le}(): 14-bit-packed
    // encoder variants.
    //
    // The 14-bit encoders compose `encode_frame_header_be` with the
    // round-145 `pack_16bit_to_14bit` primitive. The raw-BE 13- or
    // 15-byte header window is zero-padded to 16 bytes so the pack
    // step emits 9 14-bit containers = 18 bytes — the parser's
    // minimum input length for the 14-bit branch. Both encoders emit
    // exactly 18 bytes regardless of `crc_present`; the 14-bit-LE
    // output is the pairwise byte-swap of the 14-bit-BE output.
    // ---------------------------------------------------------------

    use crate::header::{encode_frame_header_14bit_be, encode_frame_header_14bit_le};

    /// 14-bit-BE encoder output is exactly 18 bytes regardless of
    /// `crc_present` (matches the parser's minimum 14-bit input length).
    #[test]
    fn encode_14bit_be_is_always_18_bytes() {
        let bytes_no_crc = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        let hdr_no_crc = parse_frame_header(&bytes_no_crc).unwrap();
        assert!(!hdr_no_crc.crc_present);
        assert_eq!(encode_frame_header_14bit_be(&hdr_no_crc).unwrap().len(), 18);

        let bytes_crc = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, Some(0xCAFE), 0xD2EC);
        let hdr_crc = parse_frame_header(&bytes_crc).unwrap();
        assert!(hdr_crc.crc_present);
        assert_eq!(encode_frame_header_14bit_be(&hdr_crc).unwrap().len(), 18);
    }

    /// 14-bit-LE encoder output is also always 18 bytes.
    #[test]
    fn encode_14bit_le_is_always_18_bytes() {
        let bytes_no_crc = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        let hdr_no_crc = parse_frame_header(&bytes_no_crc).unwrap();
        assert_eq!(encode_frame_header_14bit_le(&hdr_no_crc).unwrap().len(), 18);

        let bytes_crc = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, Some(0xCAFE), 0xD2EC);
        let hdr_crc = parse_frame_header(&bytes_crc).unwrap();
        assert_eq!(encode_frame_header_14bit_le(&hdr_crc).unwrap().len(), 18);
    }

    /// The first 4 bytes of the 14-bit-BE output match the wiki's
    /// `1F FF E8 00` sync-prefix. The wiki documents the sync as
    /// `1F FF E8 00 07 Fx` (6 bytes); the trailing `Fx` byte is
    /// the upper 4 bits of the FTYPE/SHORT/CRC_PRESENT/NBLKS_high
    /// continuation, which depends on the frame's specific header.
    #[test]
    fn encode_14bit_be_starts_with_wiki_sync_prefix() {
        let bytes_in = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        let hdr = parse_frame_header(&bytes_in).unwrap();
        let encoded = encode_frame_header_14bit_be(&hdr).unwrap();
        // Wiki's first 4 bytes are unambiguous (they're the sign-
        // extended 14-bit re-packing of the 32-bit raw-BE sync).
        assert_eq!(&encoded[..4], &[0x1F, 0xFF, 0xE8, 0x00]);
    }

    /// The first 4 bytes of the 14-bit-LE output match the wiki's
    /// `FF 1F 00 E8` sync-prefix.
    #[test]
    fn encode_14bit_le_starts_with_wiki_sync_prefix() {
        let bytes_in = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        let hdr = parse_frame_header(&bytes_in).unwrap();
        let encoded = encode_frame_header_14bit_le(&hdr).unwrap();
        assert_eq!(&encoded[..4], &[0xFF, 0x1F, 0x00, 0xE8]);
    }

    /// 14-bit-LE output is the pairwise byte-swap of the 14-bit-BE
    /// output (each 16-bit container is swapped independently — the
    /// payload bits are identical, only the container byte order
    /// differs).
    #[test]
    fn encode_14bit_le_equals_pairwise_byte_swap_of_be() {
        let bytes_in = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, Some(0xCAFE), 0xD2EC);
        let hdr = parse_frame_header(&bytes_in).unwrap();
        let be = encode_frame_header_14bit_be(&hdr).unwrap();
        let le = encode_frame_header_14bit_le(&hdr).unwrap();
        assert_eq!(be.len(), le.len());
        assert_eq!(be.len() % 2, 0, "container-aligned");
        let mut expected = be.clone();
        for pair in expected.chunks_exact_mut(2) {
            pair.swap(0, 1);
        }
        assert_eq!(le, expected);
    }

    /// `parse_frame_header_14bit(&encode_frame_header_14bit_be(&hdr))`
    /// round-trips every field except `sync_word_encoding`.
    #[test]
    fn encode_14bit_be_round_trips_through_parser_no_crc() {
        let bytes_in = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        let hdr = parse_frame_header(&bytes_in).unwrap();
        let encoded = encode_frame_header_14bit_be(&hdr).unwrap();
        assert_eq!(encoded.len(), 18);
        let hdr_round = parse_frame_header_14bit(&encoded).unwrap();
        assert_eq!(
            hdr_round.sync_word_encoding,
            SyncWordEncoding::FourteenBitBigEndian
        );
        let mut hdr_norm = hdr_round;
        hdr_norm.sync_word_encoding = hdr.sync_word_encoding;
        assert_eq!(hdr_norm, hdr);
    }

    /// Same as above with crc_present == true (15-byte raw-BE header
    /// re-packed into 18-byte 14-bit container window).
    #[test]
    fn encode_14bit_be_round_trips_through_parser_with_crc() {
        let bytes_in = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, Some(0xCAFE), 0xD2EC);
        let hdr = parse_frame_header(&bytes_in).unwrap();
        let encoded = encode_frame_header_14bit_be(&hdr).unwrap();
        assert_eq!(encoded.len(), 18);
        let hdr_round = parse_frame_header_14bit(&encoded).unwrap();
        assert_eq!(
            hdr_round.sync_word_encoding,
            SyncWordEncoding::FourteenBitBigEndian
        );
        let mut hdr_norm = hdr_round;
        hdr_norm.sync_word_encoding = hdr.sync_word_encoding;
        assert_eq!(hdr_norm, hdr);
    }

    /// 14-bit-LE round-trip through `parse_frame_header_14bit`.
    #[test]
    fn encode_14bit_le_round_trips_through_parser_no_crc() {
        let bytes_in = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        let hdr = parse_frame_header(&bytes_in).unwrap();
        let encoded = encode_frame_header_14bit_le(&hdr).unwrap();
        let hdr_round = parse_frame_header_14bit(&encoded).unwrap();
        assert_eq!(
            hdr_round.sync_word_encoding,
            SyncWordEncoding::FourteenBitLittleEndian
        );
        let mut hdr_norm = hdr_round;
        hdr_norm.sync_word_encoding = hdr.sync_word_encoding;
        assert_eq!(hdr_norm, hdr);
    }

    /// Same with crc_present == true.
    #[test]
    fn encode_14bit_le_round_trips_through_parser_with_crc() {
        let bytes_in = build_be_header(1, 31, 1, 16, 1023, 9, 13, 25, 0, Some(0xCAFE), 0xD2EC);
        let hdr = parse_frame_header(&bytes_in).unwrap();
        let encoded = encode_frame_header_14bit_le(&hdr).unwrap();
        let hdr_round = parse_frame_header_14bit(&encoded).unwrap();
        assert_eq!(
            hdr_round.sync_word_encoding,
            SyncWordEncoding::FourteenBitLittleEndian
        );
        let mut hdr_norm = hdr_round;
        hdr_norm.sync_word_encoding = hdr.sync_word_encoding;
        assert_eq!(hdr_norm, hdr);
    }

    /// Bound-validation inheritance: NBLKS<5 from the BE encoder also
    /// rejects the 14-bit-BE wrapper.
    #[test]
    fn encode_14bit_be_rejects_nblks_below_5() {
        let hdr = synth_hdr(|h| h.blocks_per_frame = 4);
        let err = encode_frame_header_14bit_be(&hdr).unwrap_err();
        assert_eq!(err, Error::BlockCountOutOfRange { blocks: 4 });
    }

    /// Bound-validation inheritance: FSIZE above 16384 also rejects
    /// the 14-bit-LE wrapper.
    #[test]
    fn encode_14bit_le_rejects_frame_size_above_16384() {
        let hdr = synth_hdr(|h| h.frame_size_bytes = 16385);
        let err = encode_frame_header_14bit_le(&hdr).unwrap_err();
        assert_eq!(err, Error::FrameSizeOutOfRange { frame_size: 16385 });
    }

    /// Bound-validation inheritance: the crc-present-without-payload
    /// mismatch is also rejected by the 14-bit-BE wrapper.
    #[test]
    fn encode_14bit_be_rejects_crc_payload_mismatch() {
        let hdr = synth_hdr(|h| {
            h.crc_present = false;
            h.header_crc = Some(0xBEEF);
        });
        let err = encode_frame_header_14bit_be(&hdr).unwrap_err();
        match err {
            Error::FieldOutOfRange { field, .. } => assert_eq!(field, "header_crc"),
            other => panic!("expected FieldOutOfRange{{field: header_crc}}, got {other:?}"),
        }
    }

    /// Exhaustive grid: every documented LFE code × both CRC states ×
    /// representative {NBLKS, FSIZE} pairs round-trip through each of
    /// the 14-bit encoders.
    #[test]
    fn encode_14bit_round_trip_grid_lfe_crc_states() {
        for crc_state in [false, true] {
            for lfe_code in 0u8..=3 {
                for &(nblks, fsize) in &[(5u8, 95u16), (16u8, 1024u16), (127u8, 16384u16)] {
                    let crc_arg = if crc_state { Some(0xBEEF) } else { None };
                    let bytes = build_be_header(
                        1,
                        31,
                        crc_state as u32,
                        nblks as u32,
                        (fsize - 1) as u32,
                        9,
                        13,
                        25,
                        ((lfe_code as u32) & 0b11) << 1,
                        crc_arg,
                        0,
                    );
                    let hdr = parse_frame_header(&bytes).unwrap();

                    // BE variant.
                    let encoded_be = encode_frame_header_14bit_be(&hdr).unwrap();
                    assert_eq!(encoded_be.len(), 18);
                    assert_eq!(&encoded_be[..4], &[0x1F, 0xFF, 0xE8, 0x00]);
                    let hdr_round_be = parse_frame_header_14bit(&encoded_be).unwrap();
                    assert_eq!(
                        hdr_round_be.sync_word_encoding,
                        SyncWordEncoding::FourteenBitBigEndian
                    );
                    let mut hdr_norm_be = hdr_round_be;
                    hdr_norm_be.sync_word_encoding = hdr.sync_word_encoding;
                    assert_eq!(hdr_norm_be, hdr);

                    // LE variant.
                    let encoded_le = encode_frame_header_14bit_le(&hdr).unwrap();
                    assert_eq!(encoded_le.len(), 18);
                    assert_eq!(&encoded_le[..4], &[0xFF, 0x1F, 0x00, 0xE8]);
                    let hdr_round_le = parse_frame_header_14bit(&encoded_le).unwrap();
                    assert_eq!(
                        hdr_round_le.sync_word_encoding,
                        SyncWordEncoding::FourteenBitLittleEndian
                    );
                    let mut hdr_norm_le = hdr_round_le;
                    hdr_norm_le.sync_word_encoding = hdr.sync_word_encoding;
                    assert_eq!(hdr_norm_le, hdr);

                    // Cross-check: BE and LE outputs are pairwise byte-
                    // swaps of each other.
                    let mut swapped = encoded_be.clone();
                    for pair in swapped.chunks_exact_mut(2) {
                        pair.swap(0, 1);
                    }
                    assert_eq!(encoded_le, swapped);
                }
            }
        }
    }

    /// Cross-reference with the existing `unpack_14bit_to_16bit` round-
    /// trip: unpacking the 14-bit-BE encoder output and reading the
    /// first `header_byte_length()` bytes equals the BE encoder output
    /// (padded to the multiple-of-8 boundary the unpacker emits).
    #[test]
    fn encode_14bit_be_unpacks_back_to_raw_be_header_prefix() {
        use crate::FourteenBitByteOrder;
        let bytes_in = build_be_header(1, 31, 0, 16, 1023, 9, 13, 25, 0, None, 0);
        let hdr = parse_frame_header(&bytes_in).unwrap();
        let encoded_14 = encode_frame_header_14bit_be(&hdr).unwrap();
        let unpacked =
            crate::unpack_14bit_to_16bit(&encoded_14, FourteenBitByteOrder::BigEndian).unwrap();
        let raw_be = encode_frame_header_be(&hdr).unwrap();
        // Unpacked stream starts with the canonical raw-BE sync and the
        // first 13 bytes equal the BE encoder output (since BE encoder
        // emits exactly 13 bytes for crc_present == false).
        assert_eq!(&unpacked[..raw_be.len()], &raw_be[..]);
        assert_eq!(&unpacked[..4], &[0x7F, 0xFE, 0x80, 0x01]);
    }

    // ---------------------------------------------------------------
    // Round 202 — SFREQ / AMODE / PCMR resolvers
    // (ETSI §5.3.1 Tables 5-5 / 5-4 / 5-17).
    // ---------------------------------------------------------------

    #[test]
    fn sample_frequency_from_index_covers_table_5_5_verbatim() {
        // Per ETSI TS 102 114 §5.3.1 Table 5-5: nine valid rows, seven
        // invalid rows. Each row checked individually so any future
        // table edit fails this test loudly.
        assert_eq!(
            sample_frequency_from_index(0b0000),
            SampleFrequency::Invalid
        );
        assert_eq!(
            sample_frequency_from_index(0b0001),
            SampleFrequency::Fixed(8_000)
        );
        assert_eq!(
            sample_frequency_from_index(0b0010),
            SampleFrequency::Fixed(16_000)
        );
        assert_eq!(
            sample_frequency_from_index(0b0011),
            SampleFrequency::Fixed(32_000)
        );
        assert_eq!(
            sample_frequency_from_index(0b0100),
            SampleFrequency::Invalid
        );
        assert_eq!(
            sample_frequency_from_index(0b0101),
            SampleFrequency::Invalid
        );
        assert_eq!(
            sample_frequency_from_index(0b0110),
            SampleFrequency::Fixed(11_025)
        );
        assert_eq!(
            sample_frequency_from_index(0b0111),
            SampleFrequency::Fixed(22_050)
        );
        assert_eq!(
            sample_frequency_from_index(0b1000),
            SampleFrequency::Fixed(44_100)
        );
        assert_eq!(
            sample_frequency_from_index(0b1001),
            SampleFrequency::Invalid
        );
        assert_eq!(
            sample_frequency_from_index(0b1010),
            SampleFrequency::Invalid
        );
        assert_eq!(
            sample_frequency_from_index(0b1011),
            SampleFrequency::Fixed(12_000)
        );
        assert_eq!(
            sample_frequency_from_index(0b1100),
            SampleFrequency::Fixed(24_000)
        );
        assert_eq!(
            sample_frequency_from_index(0b1101),
            SampleFrequency::Fixed(48_000)
        );
        assert_eq!(
            sample_frequency_from_index(0b1110),
            SampleFrequency::Invalid
        );
        assert_eq!(
            sample_frequency_from_index(0b1111),
            SampleFrequency::Invalid
        );
    }

    #[test]
    fn sample_frequency_table_has_exactly_nine_valid_codes() {
        let valid: usize = (0u8..16)
            .filter(|&code| matches!(sample_frequency_from_index(code), SampleFrequency::Fixed(_)))
            .count();
        // Table 5-5 enumerates nine valid rows: 8/16/32/11.025/22.05/44.1/12/24/48 kHz.
        assert_eq!(valid, 9);
    }

    #[test]
    fn sample_rate_hz_returns_some_for_valid_and_none_for_invalid() {
        // Build a synthetic header for each of the sixteen SFREQ codes
        // and verify sample_rate_hz() round-trips through the parser.
        for code in 0..16u32 {
            let header_bytes = build_be_header(
                /* ftype */ 1, /* sample_count_m1 */ 31, /* crc_present */ 0,
                /* nblks */ 15, /* fsize_m1 */ 1023, /* amode */ 2,
                /* sfreq */ code, /* rate */ 15, /* extra_bits */ 0,
                /* header_crc */ None, /* post_crc */ 0,
            );
            let hdr = parse_frame_header(&header_bytes).unwrap_or_else(|e| {
                panic!("parse_frame_header failed for sfreq={code:04b}: {e:?}")
            });
            assert_eq!(hdr.sfreq_index, code as u8);
            match sample_frequency_from_index(code as u8) {
                SampleFrequency::Fixed(hz) => {
                    assert_eq!(hdr.sample_rate_hz(), Some(hz));
                    assert_eq!(hdr.sample_frequency(), SampleFrequency::Fixed(hz));
                }
                SampleFrequency::Invalid => {
                    assert_eq!(hdr.sample_rate_hz(), None);
                    assert_eq!(hdr.sample_frequency(), SampleFrequency::Invalid);
                }
            }
        }
    }

    #[test]
    fn amode_arrangement_from_index_covers_table_5_4_verbatim() {
        // Per ETSI TS 102 114 §5.3.1 Table 5-4: sixteen standard
        // arrangements at codes 0..=15, plus a user-defined band at
        // codes 16..=63.
        assert_eq!(amode_arrangement_from_index(0), AmodeArrangement::Mono);
        assert_eq!(amode_arrangement_from_index(1), AmodeArrangement::DualMono);
        assert_eq!(amode_arrangement_from_index(2), AmodeArrangement::Stereo);
        assert_eq!(
            amode_arrangement_from_index(3),
            AmodeArrangement::SumDifference
        );
        assert_eq!(amode_arrangement_from_index(4), AmodeArrangement::LtRt);
        assert_eq!(amode_arrangement_from_index(5), AmodeArrangement::ClR);
        assert_eq!(amode_arrangement_from_index(6), AmodeArrangement::LrS);
        assert_eq!(amode_arrangement_from_index(7), AmodeArrangement::ClRS);
        assert_eq!(amode_arrangement_from_index(8), AmodeArrangement::LrSlSr);
        assert_eq!(amode_arrangement_from_index(9), AmodeArrangement::ClRSlSr);
        assert_eq!(
            amode_arrangement_from_index(10),
            AmodeArrangement::ClCrLRSlSr
        );
        assert_eq!(
            amode_arrangement_from_index(11),
            AmodeArrangement::ClRLrRrOv
        );
        assert_eq!(
            amode_arrangement_from_index(12),
            AmodeArrangement::CfCrLfRfLrRr
        );
        assert_eq!(
            amode_arrangement_from_index(13),
            AmodeArrangement::ClCCrLRSlSr
        );
        assert_eq!(
            amode_arrangement_from_index(14),
            AmodeArrangement::ClCrLRSl1Sl2Sr1Sr2
        );
        assert_eq!(
            amode_arrangement_from_index(15),
            AmodeArrangement::ClCCrLRSlSSr
        );
        // Codes 16..=63 are user-defined; spot-check a few.
        for code in [16u8, 17, 31, 32, 47, 62, 63] {
            assert_eq!(
                amode_arrangement_from_index(code),
                AmodeArrangement::UserDefined(code),
                "user-defined code {code} must round-trip"
            );
        }
    }

    #[test]
    fn front_lr_channels_follow_table_5_4_ordering() {
        use AmodeArrangement::*;
        // L, R lead the arrangement → (0, 1).
        for a in [Stereo, SumDifference, LtRt, LrS, LrSlSr] {
            assert_eq!(a.front_lr_channels(), Some((0, 1)), "{a:?}");
        }
        // A leading centre offsets L, R by one → (1, 2).
        for a in [ClR, ClRS, ClRSlSr] {
            assert_eq!(a.front_lr_channels(), Some((1, 2)), "{a:?}");
        }
        // CL + CR lead → L, R at (2, 3).
        assert_eq!(ClCrLRSlSr.front_lr_channels(), Some((2, 3)));
        assert_eq!(ClCrLRSl1Sl2Sr1Sr2.front_lr_channels(), Some((2, 3)));
        // CL + C + CR lead → L, R at (3, 4).
        assert_eq!(ClCCrLRSlSr.front_lr_channels(), Some((3, 4)));
        assert_eq!(ClCCrLRSlSSr.front_lr_channels(), Some((3, 4)));
        // No distinct front L/R pair.
        for a in [Mono, DualMono, ClRLrRrOv, CfCrLfRfLrRr, UserDefined(20)] {
            assert_eq!(a.front_lr_channels(), None, "{a:?}");
        }
    }

    #[test]
    fn surround_lr_channels_follow_table_5_4_ordering() {
        use AmodeArrangement::*;
        assert_eq!(LrSlSr.surround_lr_channels(), Some((2, 3)));
        assert_eq!(ClRSlSr.surround_lr_channels(), Some((3, 4)));
        assert_eq!(ClCrLRSlSr.surround_lr_channels(), Some((4, 5)));
        assert_eq!(ClCCrLRSlSr.surround_lr_channels(), Some((5, 6)));
        // Arrangements with no distinct surround L/R pair.
        for a in [
            Mono,
            DualMono,
            Stereo,
            SumDifference,
            LtRt,
            ClR,
            LrS,
            ClRS,
            ClRLrRrOv,
            CfCrLfRfLrRr,
            ClCrLRSl1Sl2Sr1Sr2,
            ClCCrLRSlSSr,
            UserDefined(33),
        ] {
            assert_eq!(a.surround_lr_channels(), None, "{a:?}");
        }
    }

    #[test]
    fn amode_channel_count_matches_table_5_4_chs_column() {
        // Per Table 5-4 CHS column (in code order 0..=15):
        // 1,2,2,2,2,3,3,4,4,5,6,6,6,7,8,8.
        let expected: [u8; 16] = [1, 2, 2, 2, 2, 3, 3, 4, 4, 5, 6, 6, 6, 7, 8, 8];
        for (code, exp) in expected.iter().enumerate() {
            assert_eq!(
                amode_arrangement_from_index(code as u8).channel_count(),
                Some(*exp),
                "channel_count for AMODE={code:02} (binary={code:06b})"
            );
        }
        // User-defined codes have no fixed CHS in the spec.
        for code in [16u8, 32, 63] {
            assert_eq!(
                amode_arrangement_from_index(code).channel_count(),
                None,
                "user-defined AMODE={code} must report None CHS"
            );
        }
    }

    #[test]
    fn channel_count_returns_some_for_standard_and_none_for_user_defined() {
        // Walk all 64 possible AMODE codes through the parser and
        // check the channel_count() round-trip.
        for code in 0..64u32 {
            let header_bytes = build_be_header(
                /* ftype */ 1, /* sample_count_m1 */ 31, /* crc_present */ 0,
                /* nblks */ 15, /* fsize_m1 */ 1023, /* amode */ code,
                /* sfreq */ 13, /* rate */ 15, /* extra_bits */ 0,
                /* header_crc */ None, /* post_crc */ 0,
            );
            let hdr = parse_frame_header(&header_bytes)
                .unwrap_or_else(|e| panic!("parse_frame_header failed for amode={code}: {e:?}"));
            assert_eq!(hdr.amode, code as u8);
            assert_eq!(
                hdr.amode_arrangement(),
                amode_arrangement_from_index(code as u8)
            );
            let exp = amode_arrangement_from_index(code as u8).channel_count();
            assert_eq!(hdr.channel_count(), exp, "channel_count for amode={code}");
        }
    }

    #[test]
    fn source_pcm_resolution_from_index_covers_table_5_17_verbatim() {
        // Per ETSI TS 102 114 §5.3.1 Table 5-17: six valid (bits, es)
        // pairs at codes {0,1,2,3,5,6}; codes {4,7} are invalid.
        assert_eq!(
            source_pcm_resolution_from_index(0b000),
            SourcePcmResolution::Valid {
                bits: 16,
                es: false
            }
        );
        assert_eq!(
            source_pcm_resolution_from_index(0b001),
            SourcePcmResolution::Valid { bits: 16, es: true }
        );
        assert_eq!(
            source_pcm_resolution_from_index(0b010),
            SourcePcmResolution::Valid {
                bits: 20,
                es: false
            }
        );
        assert_eq!(
            source_pcm_resolution_from_index(0b011),
            SourcePcmResolution::Valid { bits: 20, es: true }
        );
        assert_eq!(
            source_pcm_resolution_from_index(0b100),
            SourcePcmResolution::Invalid
        );
        assert_eq!(
            source_pcm_resolution_from_index(0b101),
            SourcePcmResolution::Valid { bits: 24, es: true }
        );
        assert_eq!(
            source_pcm_resolution_from_index(0b110),
            SourcePcmResolution::Valid {
                bits: 24,
                es: false
            }
        );
        assert_eq!(
            source_pcm_resolution_from_index(0b111),
            SourcePcmResolution::Invalid
        );
    }

    #[test]
    fn source_pcm_bits_per_sample_returns_some_for_valid_and_none_for_invalid() {
        // The PCMR field lives 6 bits deep in the 16-bit post-CRC
        // window (the wiki layout: MSB→LSB is
        // multirate_inter[1] | version[4] | copy_history[2] |
        // PCMR[3] | front_sum[1] | surround_sum[1] | dialnorm[4]).
        // PCMR therefore occupies bits 6..=8 (1-indexed from LSB:
        // positions 8..=6 from MSB).
        for code in 0..8u32 {
            let post_crc = code << 6; // place PCMR at bits 8..=6 (MSB→LSB).
            let header_bytes = build_be_header(
                /* ftype */ 1, /* sample_count_m1 */ 31, /* crc_present */ 0,
                /* nblks */ 15, /* fsize_m1 */ 1023, /* amode */ 2,
                /* sfreq */ 13, /* rate */ 15, /* extra_bits */ 0,
                /* header_crc */ None, /* post_crc */ post_crc,
            );
            let hdr = parse_frame_header(&header_bytes)
                .unwrap_or_else(|e| panic!("parse_frame_header failed for pcmr={code}: {e:?}"));
            assert_eq!(hdr.source_pcm_resolution_index, code as u8);
            assert_eq!(
                hdr.source_pcm_resolution(),
                source_pcm_resolution_from_index(code as u8)
            );
            match source_pcm_resolution_from_index(code as u8) {
                SourcePcmResolution::Valid { bits, .. } => {
                    assert_eq!(hdr.source_pcm_bits_per_sample(), Some(bits));
                }
                SourcePcmResolution::Invalid => {
                    assert_eq!(hdr.source_pcm_bits_per_sample(), None);
                }
            }
        }
    }

    #[test]
    fn ffmpeg_fixture_resolves_to_48k_stereo_16bit() {
        // The bundled ffmpeg-encoded fixture in tests/black_box_ffmpeg.rs
        // has sfreq_index=13 (48 kHz), amode=2 (stereo), and
        // source_pcm_resolution_index=0 (16-bit, ES=0). Mirror the
        // resolution path here so the unit-test layer fails as loudly
        // as the integration-test layer would.
        let header_bytes = build_be_header(
            /* ftype */ 1, /* sample_count_m1 */ 31, /* crc_present */ 0,
            /* nblks */ 15, /* fsize_m1 */ 1023,
            /* amode */ 2, // Stereo (L+R).
            /* sfreq */ 13, // 48 kHz.
            /* rate */ 15, // 768 kb/s.
            /* extra_bits */ 0, /* header_crc */ None,
            /* post_crc */ 0, // PCMR=0 → 16-bit, ES=0.
        );
        let hdr = parse_frame_header(&header_bytes).unwrap();
        assert_eq!(hdr.sample_rate_hz(), Some(48_000));
        assert_eq!(hdr.channel_count(), Some(2));
        assert_eq!(hdr.amode_arrangement(), AmodeArrangement::Stereo);
        assert_eq!(hdr.source_pcm_bits_per_sample(), Some(16));
        assert_eq!(
            hdr.source_pcm_resolution(),
            SourcePcmResolution::Valid {
                bits: 16,
                es: false
            }
        );
    }

    // ---------------------------------------------------------------
    // Round 335 — §C.2.5 QMF driver wiring: filter_bank_selection()
    // (FILTS / MULTIRATE_INTER polarity per §5.3.1 Table 5-15,
    // dts-qmf-driver.md §1) and output_r_scale() (post-filterbank
    // rScale derivation per dts-qmf-driver.md §2).
    // ---------------------------------------------------------------

    /// `multirate_inter == false` (`FILTS == 0`) resolves to the
    /// Non-Perfect Reconstruction §D.8 set per §5.3.1 Table 5-15.
    #[test]
    fn filter_bank_selection_false_is_non_perfect_reconstruction() {
        let h = synth_hdr(|h| h.multirate_inter = false);
        assert_eq!(
            h.filter_bank_selection(),
            FilterBankSelection::NonPerfectReconstruction
        );
        // It picks the lossy §D.8 column.
        assert!(core::ptr::eq(
            h.filter_bank_selection().coefficients(),
            &crate::fir_coeff::RA_COEFF_LOSSY,
        ));
    }

    /// `multirate_inter == true` (`FILTS == 1`) resolves to the
    /// Perfect Reconstruction §D.8 set per §5.3.1 Table 5-15.
    #[test]
    fn filter_bank_selection_true_is_perfect_reconstruction() {
        let h = synth_hdr(|h| h.multirate_inter = true);
        assert_eq!(
            h.filter_bank_selection(),
            FilterBankSelection::PerfectReconstruction
        );
        assert!(core::ptr::eq(
            h.filter_bank_selection().coefficients(),
            &crate::fir_coeff::RA_COEFF_LOSSLESS,
        ));
    }

    /// `filter_bank_selection()` is exactly the bit-for-bit bridge the
    /// driver doc §1 establishes: equivalent to
    /// `from_filts(u8::from(multirate_inter))` for both polarities,
    /// and round-trips through the canonical `filts()` value.
    #[test]
    fn filter_bank_selection_is_the_from_filts_bridge() {
        for bit in [false, true] {
            let h = synth_hdr(|h| h.multirate_inter = bit);
            assert_eq!(
                h.filter_bank_selection(),
                FilterBankSelection::from_filts(u8::from(bit))
            );
            // The selection's canonical FILTS value mirrors the bit.
            assert_eq!(h.filter_bank_selection().filts(), u8::from(bit));
        }
    }

    /// `output_r_scale()` returns `2^(bits-1)` for each valid PCMR
    /// resolution per the dts-qmf-driver.md §2 derivation, and `None`
    /// for the two reserved/invalid PCMR codes.
    #[test]
    fn output_r_scale_is_full_scale_for_each_valid_pcmr() {
        // Drive every PCMR code 0..=7 and check the rScale derivation
        // tracks source_pcm_bits_per_sample().
        for code in 0..8u8 {
            let h = synth_hdr(|h| h.source_pcm_resolution_index = code);
            match h.source_pcm_bits_per_sample() {
                Some(16) => assert_eq!(h.output_r_scale(), Some(32768.0)),
                Some(20) => assert_eq!(h.output_r_scale(), Some(524_288.0)),
                Some(24) => assert_eq!(h.output_r_scale(), Some(8_388_608.0)),
                Some(other) => panic!("unexpected PCMR bits {other} for code {code}"),
                None => assert_eq!(h.output_r_scale(), None),
            }
        }
    }

    /// The two reserved PCMR codes (`0b100`, `0b111`) yield no rScale,
    /// mirroring `source_pcm_bits_per_sample()`'s `None`.
    #[test]
    fn output_r_scale_is_none_for_reserved_pcmr_codes() {
        for code in [0b100u8, 0b111u8] {
            let h = synth_hdr(|h| h.source_pcm_resolution_index = code);
            assert_eq!(h.source_pcm_bits_per_sample(), None);
            assert_eq!(h.output_r_scale(), None);
        }
    }

    /// End-to-end: a parsed header drives `QmfSynthesis::synthesize`
    /// directly through `filter_bank_selection()` + `output_r_scale()`
    /// — the §C.2.5 driver's two header-sourced parameters — producing
    /// the same PCM as feeding the resolved values manually.
    #[test]
    fn header_drives_qmf_synthesis_end_to_end() {
        use crate::cos_mod::NUM_SUBBAND;
        use crate::qmf_synth::QmfSynthesis;

        // PCMR=0 → 16-bit → rScale 32768; multirate_inter=true →
        // perfect reconstruction.
        let h = synth_hdr(|h| {
            h.multirate_inter = true;
            h.source_pcm_resolution_index = 0;
        });
        let filter = h.filter_bank_selection();
        let r_scale = h.output_r_scale().expect("valid PCMR yields rScale");
        assert_eq!(filter, FilterBankSelection::PerfectReconstruction);
        assert_eq!(r_scale, 32768.0);

        // A large impulse across several rows so the §D.8 perfect-
        // reconstruction FIR tail (its leading taps are ~1e-10)
        // truncates to non-zero integer PCM at the 16-bit gain.
        let mut row0 = [0.0_f64; NUM_SUBBAND];
        row0[0] = 1.0e6;
        let mut rows = vec![[0.0_f64; NUM_SUBBAND]; 16];
        rows[0] = row0;

        let mut via_header = QmfSynthesis::new();
        let mut hdr_out = Vec::new();
        via_header
            .synthesize(&rows, 4, filter, r_scale, &mut hdr_out)
            .unwrap();

        // Manually with the values the driver doc resolves them to.
        let mut manual = QmfSynthesis::new();
        let mut man_out = Vec::new();
        manual
            .synthesize(
                &rows,
                4,
                FilterBankSelection::PerfectReconstruction,
                32768.0,
                &mut man_out,
            )
            .unwrap();

        assert_eq!(hdr_out, man_out);
        assert!(hdr_out.iter().any(|&s| s != 0));
    }

    // ---------------------------------------------------------------
    // Round 337 — MultiChannelQmf header convenience: the per-frame
    // multi-channel driver sources its two frame-wide §C.2.5 parameters
    // (FILTS via filter_bank_selection(), output rScale via
    // output_r_scale()) directly from a parsed header.
    // ---------------------------------------------------------------

    /// `MultiChannelQmf::synthesize_planar_from_header` sources `FILTS`
    /// and `rScale` from a parsed header and matches a manual planar
    /// call with the resolved values.
    #[test]
    fn multichannel_from_header_matches_manual_resolution() {
        use crate::cos_mod::NUM_SUBBAND;
        use crate::qmf_multichannel::MultiChannelQmf;

        // multirate_inter=true → perfect; PCMR=0 → 16-bit → rScale 32768.
        let h = synth_hdr(|h| {
            h.multirate_inter = true;
            h.source_pcm_resolution_index = 0;
        });

        let rows: Vec<[f64; NUM_SUBBAND]> = (0..6)
            .map(|s| {
                let mut r = [0.0; NUM_SUBBAND];
                r[0] = (s as f64 + 1.0) * 1e6;
                r
            })
            .collect();
        let refs: Vec<&[[f64; NUM_SUBBAND]]> = vec![rows.as_slice()];
        let n_subs = [3usize];

        let mut mc_h = MultiChannelQmf::new(1);
        let mut via_header = vec![Vec::new()];
        let outcome = mc_h
            .synthesize_planar_from_header(&h, &refs, &n_subs, &mut via_header)
            .unwrap();
        assert_eq!(outcome, Some(()));

        let mut mc_m = MultiChannelQmf::new(1);
        let mut manual = vec![Vec::new()];
        mc_m.synthesize_planar(
            &refs,
            &n_subs,
            FilterBankSelection::PerfectReconstruction,
            32768.0,
            &mut manual,
        )
        .unwrap();

        assert_eq!(via_header, manual);
        assert!(via_header[0].iter().any(|&s| s != 0));
    }

    /// A reserved PCMR code yields `Ok(None)` from the multi-channel
    /// header convenience (no full-scale gain defined) and leaves the
    /// output untouched.
    #[test]
    fn multichannel_from_header_reserved_pcmr_yields_none() {
        use crate::cos_mod::NUM_SUBBAND;
        use crate::qmf_multichannel::MultiChannelQmf;

        let h = synth_hdr(|h| h.source_pcm_resolution_index = 0b100); // reserved

        let rows = vec![[0.0_f64; NUM_SUBBAND]; 2];
        let refs: Vec<&[[f64; NUM_SUBBAND]]> = vec![rows.as_slice()];
        let n_subs = [4usize];

        let mut mc = MultiChannelQmf::new(1);
        let mut out = vec![Vec::new()];
        let outcome = mc
            .synthesize_planar_from_header(&h, &refs, &n_subs, &mut out)
            .unwrap();
        assert_eq!(outcome, None);
        assert!(out[0].is_empty());
    }
}

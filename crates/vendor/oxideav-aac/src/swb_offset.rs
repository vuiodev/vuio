//! Scalefactor-band offset tables — ISO/IEC 14496-3 §4.5.4.1 / Tables
//! 4.129–4.141.
//!
//! Each `swb_offset_long_window[fs_index]` / `swb_offset_short_window[fs_index]`
//! table lists the *index of the lowest spectral coefficient* of each
//! scalefactor band, plus a trailing sentinel at the spectrum length
//! (1024 for long, 128 for short). The per-band width is therefore
//! `offset[i + 1] - offset[i]`, and the total entry count is
//! `num_swb + 1`.
//!
//! ## What this module covers
//!
//! * [`SWB_OFFSET_LONG_WINDOW`] — 13-entry lookup of long-window
//!   offset slices, keyed by `samplingFrequencyIndex` (Table 1.18).
//!   Slots `0..=11` cover the 12 sampling rates that have defined
//!   SWB tables; slot `12` (7350 Hz) is an empty slice (no SWB
//!   table is defined). Sourced from Tables 4.129 (44.1 / 48 kHz,
//!   fs 3/4), 4.131 (32 kHz, fs 5), 4.132 (8 kHz, fs 11), 4.134
//!   (11.025 / 12 / 16 kHz, fs 8/9/10), 4.136 (22.05 / 24 kHz, fs 6/7),
//!   4.138 (64 kHz, fs 2), 4.140 (88.2 / 96 kHz, fs 0/1).
//! * [`SWB_OFFSET_SHORT_WINDOW`] — 13-entry lookup of 128-line
//!   short-window offset slices (same fs-index layout as the long
//!   table). Sourced from Tables 4.130 (32 / 44.1 / 48 kHz,
//!   fs 3/4/5), 4.133 (8 kHz, fs 11), 4.135 (11.025 / 12 / 16 kHz,
//!   fs 8/9/10), 4.137 (22.05 / 24 kHz, fs 6/7), 4.139 (64 kHz,
//!   fs 2), 4.141 (88.2 / 96 kHz, fs 0/1).
//! * [`long_window_offsets`] / [`short_window_offsets`] — safe
//!   bounds-checked accessors.
//! * [`apply_pulse_data`] — the §4.6.13 pulse-escape reconstruction
//!   loop. Given a quantised long-window spectrum `x_quant` and a
//!   parsed [`crate::pulse_data::PulseData`] block, applies the
//!   per-pulse offset / amplitude fix-up in place.
//!
//! * [`FrameFamily`] + [`long_window_offsets_family`] /
//!   [`short_window_offsets_family`] — the §4.5.1.1 frame-length
//!   families: the 960/120-line variant (`frameLengthFlag == 1`, the
//!   bracketed "values for 1920 / 240" columns of Tables 4.129–4.141)
//!   and the ER AAC LD 512/480-line variants (§4.6.17.2.1, Tables
//!   4.142–4.147 with the §4.5.1.1 nearest-defined-table rule for
//!   rates those tables omit).
//!
//! ## What this module does *not* cover
//!
//! * `sampling_frequency_index == 12` (7350 Hz) has no
//!   scalefactor-band table in the spec; accessors return
//!   [`Error::IcsInfoUnsupportedSampleRateIndex`] for that index.
//! * The 24-bit explicit-rate escape (`samplingFrequencyIndex
//!   == 0xf`) does not select an SWB table directly — the caller must
//!   resolve the explicit rate to the nearest standard index before
//!   invoking these accessors.

use crate::pulse_data::PulseData;
use crate::{Error, Result};

/// Total number of spectral coefficients in a long-window frame
/// (1024). The sentinel of every long-window table equals this value.
pub const LONG_WINDOW_LEN: u16 = 1024;

/// Total number of spectral coefficients in a short-window frame
/// (128). The sentinel of every short-window table equals this value.
pub const SHORT_WINDOW_LEN: u16 = 128;

/// `swb_offset_long_window[3]` / `swb_offset_long_window[4]` — Table
/// 4.129 (44.1 and 48 kHz, 49 SWB). 50 entries (49 bands + sentinel).
const SWB_OFFSET_LONG_44100_48000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 48, 56, 64, 72, 80, 88, 96, 108, 120, 132, 144, 160,
    176, 196, 216, 240, 264, 292, 320, 352, 384, 416, 448, 480, 512, 544, 576, 608, 640, 672, 704,
    736, 768, 800, 832, 864, 896, 928, 1024,
];

/// `swb_offset_long_window[5]` — Table 4.131 (32 kHz, 51 SWB). 52
/// entries.
const SWB_OFFSET_LONG_32000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 48, 56, 64, 72, 80, 88, 96, 108, 120, 132, 144, 160,
    176, 196, 216, 240, 264, 292, 320, 352, 384, 416, 448, 480, 512, 544, 576, 608, 640, 672, 704,
    736, 768, 800, 832, 864, 896, 928, 960, 992, 1024,
];

/// `swb_offset_long_window[11]` — Table 4.132 (8 kHz, 40 SWB). 41
/// entries.
const SWB_OFFSET_LONG_8000: &[u16] = &[
    0, 12, 24, 36, 48, 60, 72, 84, 96, 108, 120, 132, 144, 156, 172, 188, 204, 220, 236, 252, 268,
    288, 308, 328, 348, 372, 396, 420, 448, 476, 508, 544, 580, 620, 664, 712, 764, 820, 880, 944,
    1024,
];

/// `swb_offset_long_window[8]` / `[9]` / `[10]` — Table 4.134
/// (11.025, 12 and 16 kHz, 43 SWB). 44 entries.
const SWB_OFFSET_LONG_11025_12000_16000: &[u16] = &[
    0, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 100, 112, 124, 136, 148, 160, 172, 184, 196, 212,
    228, 244, 260, 280, 300, 320, 344, 368, 396, 424, 456, 492, 532, 572, 616, 664, 716, 772, 832,
    896, 960, 1024,
];

/// `swb_offset_long_window[6]` / `[7]` — Table 4.136 (22.05 and 24 kHz,
/// 47 SWB). 48 entries.
const SWB_OFFSET_LONG_22050_24000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 52, 60, 68, 76, 84, 92, 100, 108, 116, 124, 136,
    148, 160, 172, 188, 204, 220, 240, 260, 284, 308, 336, 364, 396, 432, 468, 508, 552, 600, 652,
    704, 768, 832, 896, 960, 1024,
];

/// `swb_offset_long_window[2]` — Table 4.138 (64 kHz, 47 SWB). 48
/// entries.
const SWB_OFFSET_LONG_64000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 64, 72, 80, 88, 100, 112, 124, 140,
    156, 172, 192, 216, 240, 268, 304, 344, 384, 424, 464, 504, 544, 584, 624, 664, 704, 744, 784,
    824, 864, 904, 944, 984, 1024,
];

/// `swb_offset_long_window[0]` / `[1]` — Table 4.140 (88.2 and 96 kHz,
/// 41 SWB). 42 entries.
const SWB_OFFSET_LONG_88200_96000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 64, 72, 80, 88, 96, 108, 120, 132,
    144, 156, 172, 188, 212, 240, 276, 320, 384, 448, 512, 576, 640, 704, 768, 832, 896, 960, 1024,
];

/// `swb_offset_short_window[3]` / `[4]` / `[5]` — Table 4.130
/// (32, 44.1, 48 kHz, 14 SWB). 15 entries.
const SWB_OFFSET_SHORT_32000_44100_48000: &[u16] =
    &[0, 4, 8, 12, 16, 20, 28, 36, 44, 56, 68, 80, 96, 112, 128];

/// `swb_offset_short_window[11]` — Table 4.133 (8 kHz, 15 SWB). 16
/// entries.
const SWB_OFFSET_SHORT_8000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 36, 44, 52, 60, 72, 88, 108, 128,
];

/// `swb_offset_short_window[8]` / `[9]` / `[10]` — Table 4.135
/// (11.025, 12, 16 kHz, 15 SWB). 16 entries.
const SWB_OFFSET_SHORT_11025_12000_16000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 40, 48, 60, 72, 88, 108, 128,
];

/// `swb_offset_short_window[6]` / `[7]` — Table 4.137 (22.05, 24 kHz,
/// 15 SWB). 16 entries.
const SWB_OFFSET_SHORT_22050_24000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 36, 44, 52, 64, 76, 92, 108, 128,
];

/// `swb_offset_short_window[2]` — Table 4.139 (64 kHz, 12 SWB). 13
/// entries.
const SWB_OFFSET_SHORT_64000: &[u16] = &[0, 4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 92, 128];

/// `swb_offset_short_window[0]` / `[1]` — Table 4.141 (88.2, 96 kHz,
/// 12 SWB). 13 entries.
const SWB_OFFSET_SHORT_88200_96000: &[u16] = &[0, 4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 92, 128];

/// `swb_offset_long_window` keyed by `samplingFrequencyIndex`
/// (ISO/IEC 14496-3 Table 1.18). Slot `12` (7350 Hz) carries an empty
/// slice — no SWB table is defined for that rate.
///
/// Each slot is `num_swb + 1` entries long (the trailing entry is the
/// spectrum-length sentinel `1024`).
pub const SWB_OFFSET_LONG_WINDOW: [&[u16]; 13] = [
    SWB_OFFSET_LONG_88200_96000,       // 0 = 96 kHz
    SWB_OFFSET_LONG_88200_96000,       // 1 = 88.2 kHz
    SWB_OFFSET_LONG_64000,             // 2 = 64 kHz
    SWB_OFFSET_LONG_44100_48000,       // 3 = 48 kHz
    SWB_OFFSET_LONG_44100_48000,       // 4 = 44.1 kHz
    SWB_OFFSET_LONG_32000,             // 5 = 32 kHz
    SWB_OFFSET_LONG_22050_24000,       // 6 = 24 kHz
    SWB_OFFSET_LONG_22050_24000,       // 7 = 22.05 kHz
    SWB_OFFSET_LONG_11025_12000_16000, // 8 = 16 kHz
    SWB_OFFSET_LONG_11025_12000_16000, // 9 = 12 kHz
    SWB_OFFSET_LONG_11025_12000_16000, // 10 = 11.025 kHz
    SWB_OFFSET_LONG_8000,              // 11 = 8 kHz
    &[],                               // 12 = 7350 Hz (no SWB table)
];

/// `swb_offset_short_window` keyed by `samplingFrequencyIndex`
/// (ISO/IEC 14496-3 Table 1.18). Slot `12` (7350 Hz) carries an
/// empty slice.
///
/// Each slot is `num_swb + 1` entries long (the trailing entry is the
/// short-spectrum-length sentinel `128`).
pub const SWB_OFFSET_SHORT_WINDOW: [&[u16]; 13] = [
    SWB_OFFSET_SHORT_88200_96000,       // 0 = 96 kHz
    SWB_OFFSET_SHORT_88200_96000,       // 1 = 88.2 kHz
    SWB_OFFSET_SHORT_64000,             // 2 = 64 kHz
    SWB_OFFSET_SHORT_32000_44100_48000, // 3 = 48 kHz
    SWB_OFFSET_SHORT_32000_44100_48000, // 4 = 44.1 kHz
    SWB_OFFSET_SHORT_32000_44100_48000, // 5 = 32 kHz
    SWB_OFFSET_SHORT_22050_24000,       // 6 = 24 kHz
    SWB_OFFSET_SHORT_22050_24000,       // 7 = 22.05 kHz
    SWB_OFFSET_SHORT_11025_12000_16000, // 8 = 16 kHz
    SWB_OFFSET_SHORT_11025_12000_16000, // 9 = 12 kHz
    SWB_OFFSET_SHORT_11025_12000_16000, // 10 = 11.025 kHz
    SWB_OFFSET_SHORT_8000,              // 11 = 8 kHz
    &[],                                // 12 = 7350 Hz (no SWB table)
];

/// Look up `swb_offset_long_window[fs_index]`.
///
/// Returns the slice of `num_swb + 1` per-band lowest-coefficient
/// indices (with the trailing `1024` sentinel) for the requested
/// `samplingFrequencyIndex`.
///
/// Returns [`Error::IcsInfoUnsupportedSampleRateIndex`] if `fs_index`
/// is outside `0..=11`. Index 12 (7350 Hz) has no defined long-window
/// SWB table.
pub fn long_window_offsets(fs_index: u8) -> Result<&'static [u16]> {
    let idx = fs_index as usize;
    if idx >= SWB_OFFSET_LONG_WINDOW.len() {
        return Err(Error::IcsInfoUnsupportedSampleRateIndex(fs_index));
    }
    let slice = SWB_OFFSET_LONG_WINDOW[idx];
    if slice.is_empty() {
        return Err(Error::IcsInfoUnsupportedSampleRateIndex(fs_index));
    }
    Ok(slice)
}

/// Look up `swb_offset_short_window[fs_index]`.
///
/// Returns the slice of `num_swb + 1` per-band lowest-coefficient
/// indices (with the trailing `128` sentinel) for the requested
/// `samplingFrequencyIndex`.
///
/// Returns [`Error::IcsInfoUnsupportedSampleRateIndex`] if `fs_index`
/// is outside `0..=11`. Index 12 (7350 Hz) has no defined short-window
/// SWB table.
pub fn short_window_offsets(fs_index: u8) -> Result<&'static [u16]> {
    let idx = fs_index as usize;
    if idx >= SWB_OFFSET_SHORT_WINDOW.len() {
        return Err(Error::IcsInfoUnsupportedSampleRateIndex(fs_index));
    }
    let slice = SWB_OFFSET_SHORT_WINDOW[idx];
    if slice.is_empty() {
        return Err(Error::IcsInfoUnsupportedSampleRateIndex(fs_index));
    }
    Ok(slice)
}

// ---------------------------------------------------------------------------
// Frame-length families — §4.5.1.1 `frameLengthFlag` / §4.6.17.2.1.
// ---------------------------------------------------------------------------

/// The four spectral-line frame families a General-Audio payload can
/// select — ISO/IEC 14496-3 §4.5.1.1 (`frameLengthFlag`) and
/// §4.6.17.2.1 (the ER AAC LD frame sizes).
///
/// * For every GA AOT except AAC SSR and ER AAC LD,
///   `frameLengthFlag == 0` selects the 1024/128-line IMDCT family
///   and `frameLengthFlag == 1` the 960/120-line family.
/// * For ER AAC LD (AOT 23), `frameLengthFlag == 0` selects a single
///   512-line IMDCT and `frameLengthFlag == 1` a single 480-line
///   IMDCT; there is no block switching (§4.6.17.2.2), hence no
///   short-window geometry at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameFamily {
    /// 1024 spectral lines per long frame, 128 per short window
    /// (`frameLengthFlag == 0`, all GA AOTs except SSR / LD).
    #[default]
    Lc1024,
    /// 960 spectral lines per long frame, 120 per short window
    /// (`frameLengthFlag == 1`).
    Lc960,
    /// ER AAC LD, 512 spectral lines (`frameLengthFlag == 0`);
    /// long-only.
    Ld512,
    /// ER AAC LD, 480 spectral lines (`frameLengthFlag == 1`);
    /// long-only.
    Ld480,
}

impl FrameFamily {
    /// Resolve the family from the stream's `audioObjectType` and
    /// `frameLengthFlag` per §4.5.1.1.
    pub fn from_aot_and_flag(aot: u8, frame_length_flag: bool) -> Self {
        match (aot == 23, frame_length_flag) {
            (false, false) => FrameFamily::Lc1024,
            (false, true) => FrameFamily::Lc960,
            (true, false) => FrameFamily::Ld512,
            (true, true) => FrameFamily::Ld480,
        }
    }

    /// Spectral lines per long window == PCM samples per frame per
    /// channel (1024 / 960 / 512 / 480).
    pub fn frame_len(self) -> usize {
        match self {
            FrameFamily::Lc1024 => 1024,
            FrameFamily::Lc960 => 960,
            FrameFamily::Ld512 => 512,
            FrameFamily::Ld480 => 480,
        }
    }

    /// `N_l` — the long IMDCT transform length (`2 × frame_len`):
    /// 2048 / 1920 / 1024 / 960.
    pub fn long_transform_len(self) -> usize {
        2 * self.frame_len()
    }

    /// Spectral lines per short window (128 / 120), or [`None`] for
    /// the long-only LD families (§4.6.17.2.2 — no block switching).
    pub fn short_window_len(self) -> Option<usize> {
        match self {
            FrameFamily::Lc1024 => Some(128),
            FrameFamily::Lc960 => Some(120),
            FrameFamily::Ld512 | FrameFamily::Ld480 => None,
        }
    }

    /// `N_s` — the short IMDCT transform length (256 / 240), or
    /// [`None`] for the LD families.
    pub fn short_transform_len(self) -> Option<usize> {
        self.short_window_len().map(|w| 2 * w)
    }

    /// `true` for the ER AAC LD families (§4.6.17): long-only frames,
    /// low-overlap window in place of KBD, LD LTP lag semantics.
    pub fn is_ld(self) -> bool {
        matches!(self, FrameFamily::Ld512 | FrameFamily::Ld480)
    }
}

// ---------------------------------------------------------------------------
// 960/120-line tables — the bracketed "values for 1920 / 240" columns
// of Tables 4.129–4.141.
// ---------------------------------------------------------------------------
//
// Each long table prints the 1920-transform variant as bracketed
// values on the shared rows: the band starts are identical to the
// 2048-transform column and only the tail changes — the sentinel
// becomes 960 and any offsets at or above 960 are dropped (`(-)`).
// Each short table only re-brackets the sentinel (`128 (120)`).

/// `swb_offset_long_window[3]` / `[4]` for the 960-line family —
/// Table 4.129 bracketed column (44.1 / 48 kHz, 49 SWB). 50 entries.
const SWB_OFFSET_LONG_960_44100_48000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 48, 56, 64, 72, 80, 88, 96, 108, 120, 132, 144, 160,
    176, 196, 216, 240, 264, 292, 320, 352, 384, 416, 448, 480, 512, 544, 576, 608, 640, 672, 704,
    736, 768, 800, 832, 864, 896, 928, 960,
];

/// `swb_offset_long_window[5]` for the 960-line family — Table 4.131
/// bracketed column (32 kHz; the 992 / 1024 rows are `(-)`, so 49
/// SWB). 50 entries.
const SWB_OFFSET_LONG_960_32000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 48, 56, 64, 72, 80, 88, 96, 108, 120, 132, 144, 160,
    176, 196, 216, 240, 264, 292, 320, 352, 384, 416, 448, 480, 512, 544, 576, 608, 640, 672, 704,
    736, 768, 800, 832, 864, 896, 928, 960,
];

/// `swb_offset_long_window[11]` for the 960-line family — Table 4.132
/// bracketed column (8 kHz, 40 SWB). 41 entries.
const SWB_OFFSET_LONG_960_8000: &[u16] = &[
    0, 12, 24, 36, 48, 60, 72, 84, 96, 108, 120, 132, 144, 156, 172, 188, 204, 220, 236, 252, 268,
    288, 308, 328, 348, 372, 396, 420, 448, 476, 508, 544, 580, 620, 664, 712, 764, 820, 880, 944,
    960,
];

/// `swb_offset_long_window[8]` / `[9]` / `[10]` for the 960-line
/// family — Table 4.134 bracketed column (11.025 / 12 / 16 kHz; the
/// 1024 row is `(-)`, so 42 SWB). 43 entries.
const SWB_OFFSET_LONG_960_11025_12000_16000: &[u16] = &[
    0, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 100, 112, 124, 136, 148, 160, 172, 184, 196, 212,
    228, 244, 260, 280, 300, 320, 344, 368, 396, 424, 456, 492, 532, 572, 616, 664, 716, 772, 832,
    896, 960,
];

/// `swb_offset_long_window[6]` / `[7]` for the 960-line family —
/// Table 4.136 bracketed column (22.05 / 24 kHz; the 1024 row is
/// `(-)`, so 46 SWB). 47 entries.
const SWB_OFFSET_LONG_960_22050_24000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 52, 60, 68, 76, 84, 92, 100, 108, 116, 124, 136,
    148, 160, 172, 188, 204, 220, 240, 260, 284, 308, 336, 364, 396, 432, 468, 508, 552, 600, 652,
    704, 768, 832, 896, 960,
];

/// `swb_offset_long_window[2]` for the 960-line family — Table 4.138
/// bracketed column (64 kHz, `num_swb 47 (46)`: the 984 row brackets
/// to 960 and the 1024 row is `(-)`). 47 entries.
const SWB_OFFSET_LONG_960_64000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 64, 72, 80, 88, 100, 112, 124, 140,
    156, 172, 192, 216, 240, 268, 304, 344, 384, 424, 464, 504, 544, 584, 624, 664, 704, 744, 784,
    824, 864, 904, 944, 960,
];

/// `swb_offset_long_window[0]` / `[1]` for the 960-line family —
/// Table 4.140 bracketed column (88.2 / 96 kHz; the 1024 row is
/// `(-)`, so 40 SWB). 41 entries.
const SWB_OFFSET_LONG_960_88200_96000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 64, 72, 80, 88, 96, 108, 120, 132,
    144, 156, 172, 188, 212, 240, 276, 320, 384, 448, 512, 576, 640, 704, 768, 832, 896, 960,
];

/// Table 4.130 bracketed column — 120-line short window at 32 / 44.1 /
/// 48 kHz (14 SWB). 15 entries.
const SWB_OFFSET_SHORT_120_32000_44100_48000: &[u16] =
    &[0, 4, 8, 12, 16, 20, 28, 36, 44, 56, 68, 80, 96, 112, 120];

/// Table 4.133 bracketed column — 120-line short window at 8 kHz
/// (15 SWB). 16 entries.
const SWB_OFFSET_SHORT_120_8000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 36, 44, 52, 60, 72, 88, 108, 120,
];

/// Table 4.135 bracketed column — 120-line short window at 11.025 /
/// 12 / 16 kHz (15 SWB). 16 entries.
const SWB_OFFSET_SHORT_120_11025_12000_16000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 40, 48, 60, 72, 88, 108, 120,
];

/// Table 4.137 bracketed column — 120-line short window at 22.05 /
/// 24 kHz (15 SWB). 16 entries.
const SWB_OFFSET_SHORT_120_22050_24000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 36, 44, 52, 64, 76, 92, 108, 120,
];

/// Table 4.139 bracketed column — 120-line short window at 64 kHz
/// (12 SWB). 13 entries.
const SWB_OFFSET_SHORT_120_64000: &[u16] = &[0, 4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 92, 120];

/// Table 4.141 bracketed column — 120-line short window at 88.2 /
/// 96 kHz (12 SWB). 13 entries.
const SWB_OFFSET_SHORT_120_88200_96000: &[u16] =
    &[0, 4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 92, 120];

/// 960-line long-window offset tables keyed by
/// `samplingFrequencyIndex` (the bracketed Tables 4.129–4.140
/// columns). Same slot layout as [`SWB_OFFSET_LONG_WINDOW`].
pub const SWB_OFFSET_LONG_WINDOW_960: [&[u16]; 13] = [
    SWB_OFFSET_LONG_960_88200_96000,       // 0 = 96 kHz
    SWB_OFFSET_LONG_960_88200_96000,       // 1 = 88.2 kHz
    SWB_OFFSET_LONG_960_64000,             // 2 = 64 kHz
    SWB_OFFSET_LONG_960_44100_48000,       // 3 = 48 kHz
    SWB_OFFSET_LONG_960_44100_48000,       // 4 = 44.1 kHz
    SWB_OFFSET_LONG_960_32000,             // 5 = 32 kHz
    SWB_OFFSET_LONG_960_22050_24000,       // 6 = 24 kHz
    SWB_OFFSET_LONG_960_22050_24000,       // 7 = 22.05 kHz
    SWB_OFFSET_LONG_960_11025_12000_16000, // 8 = 16 kHz
    SWB_OFFSET_LONG_960_11025_12000_16000, // 9 = 12 kHz
    SWB_OFFSET_LONG_960_11025_12000_16000, // 10 = 11.025 kHz
    SWB_OFFSET_LONG_960_8000,              // 11 = 8 kHz
    &[],                                   // 12 = 7350 Hz (no SWB table)
];

/// 120-line short-window offset tables keyed by
/// `samplingFrequencyIndex` (the bracketed Tables 4.130–4.141
/// columns). Same slot layout as [`SWB_OFFSET_SHORT_WINDOW`].
pub const SWB_OFFSET_SHORT_WINDOW_120: [&[u16]; 13] = [
    SWB_OFFSET_SHORT_120_88200_96000,       // 0 = 96 kHz
    SWB_OFFSET_SHORT_120_88200_96000,       // 1 = 88.2 kHz
    SWB_OFFSET_SHORT_120_64000,             // 2 = 64 kHz
    SWB_OFFSET_SHORT_120_32000_44100_48000, // 3 = 48 kHz
    SWB_OFFSET_SHORT_120_32000_44100_48000, // 4 = 44.1 kHz
    SWB_OFFSET_SHORT_120_32000_44100_48000, // 5 = 32 kHz
    SWB_OFFSET_SHORT_120_22050_24000,       // 6 = 24 kHz
    SWB_OFFSET_SHORT_120_22050_24000,       // 7 = 22.05 kHz
    SWB_OFFSET_SHORT_120_11025_12000_16000, // 8 = 16 kHz
    SWB_OFFSET_SHORT_120_11025_12000_16000, // 9 = 12 kHz
    SWB_OFFSET_SHORT_120_11025_12000_16000, // 10 = 11.025 kHz
    SWB_OFFSET_SHORT_120_8000,              // 11 = 8 kHz
    &[],                                    // 12 = 7350 Hz (no SWB table)
];

// ---------------------------------------------------------------------------
// ER AAC LD tables — §4.5.4 Tables 4.142–4.147 (window lengths 960
// and 1024, i.e. LD frame sizes 480 and 512).
// ---------------------------------------------------------------------------

/// Table 4.143 — LD 512-line frame at 44.1 / 48 kHz (36 SWB). 37
/// entries.
const SWB_OFFSET_LD_512_44100_48000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60, 68, 76, 84, 92, 100, 112, 124,
    136, 148, 164, 184, 208, 236, 268, 300, 332, 364, 396, 428, 460, 512,
];

/// Table 4.145 — LD 512-line frame at 32 kHz (37 SWB). 38 entries.
const SWB_OFFSET_LD_512_32000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 64, 72, 80, 88, 96, 108, 120, 132,
    144, 160, 176, 192, 212, 236, 260, 288, 320, 352, 384, 416, 448, 480, 512,
];

/// Table 4.147 — LD 512-line frame at 22.05 / 24 kHz (31 SWB). 32
/// entries.
const SWB_OFFSET_LD_512_22050_24000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 52, 60, 68, 80, 92, 104, 120, 140, 164, 192, 224,
    256, 288, 320, 352, 384, 416, 448, 480, 512,
];

/// Table 4.142 — LD 480-line frame at 44.1 / 48 kHz (35 SWB). 36
/// entries.
const SWB_OFFSET_LD_480_44100_48000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 64, 72, 80, 88, 96, 108, 120, 132,
    144, 156, 172, 188, 212, 240, 272, 304, 336, 368, 400, 432, 480,
];

/// Table 4.144 — LD 480-line frame at 32 kHz (37 SWB). 38 entries.
const SWB_OFFSET_LD_480_32000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60, 64, 72, 80, 88, 96, 104, 112, 124,
    136, 148, 164, 180, 200, 224, 256, 288, 320, 352, 384, 416, 448, 480,
];

/// Table 4.146 — LD 480-line frame at 22.05 / 24 kHz (30 SWB). 31
/// entries.
const SWB_OFFSET_LD_480_22050_24000: &[u16] = &[
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 52, 60, 68, 80, 92, 104, 120, 140, 164, 192, 224,
    256, 288, 320, 352, 384, 416, 448, 480,
];

/// Map a `samplingFrequencyIndex` onto the LD table column that
/// covers it.
///
/// Tables 4.142–4.147 only define the 48 / 44.1 / 32 / 24 / 22.05 kHz
/// rates. Per §4.5.1.1 ("if in a certain sampling frequency dependent
/// table a sampling frequency stated in the right column of Table
/// 4.82 is not defined, the nearest defined table shall be used"),
/// every higher rate resolves to the 48 kHz table (48 000 is the
/// nearest defined rate for 96 / 88.2 / 64 kHz) and every lower rate
/// to the 22.05 kHz table (22 050 is the nearest defined rate for
/// 16 / 12 / 11.025 / 8 kHz).
fn ld_table_slot(fs_index: u8) -> Result<usize> {
    match fs_index {
        0..=4 => Ok(0),  // 96 / 88.2 / 64 / 48 / 44.1 kHz → 44.1/48 table
        5 => Ok(1),      // 32 kHz
        6..=11 => Ok(2), // 24 / 22.05 kHz + nearest-rule lower rates
        other => Err(Error::IcsInfoUnsupportedSampleRateIndex(other)),
    }
}

/// LD 512-line tables in [`ld_table_slot`] order.
const SWB_OFFSET_LD_512: [&[u16]; 3] = [
    SWB_OFFSET_LD_512_44100_48000,
    SWB_OFFSET_LD_512_32000,
    SWB_OFFSET_LD_512_22050_24000,
];

/// LD 480-line tables in [`ld_table_slot`] order.
const SWB_OFFSET_LD_480: [&[u16]; 3] = [
    SWB_OFFSET_LD_480_44100_48000,
    SWB_OFFSET_LD_480_32000,
    SWB_OFFSET_LD_480_22050_24000,
];

/// Family-aware `swb_offset_long_window[fs_index]` lookup.
///
/// Dispatches on the [`FrameFamily`]: `Lc1024` reads the Tables
/// 4.129–4.140 primary columns (== [`long_window_offsets`]), `Lc960`
/// their bracketed 1920-transform columns, and the LD families the
/// dedicated Tables 4.142–4.147 (with the §4.5.1.1 nearest-defined-
/// table rule for rates those tables omit).
pub fn long_window_offsets_family(family: FrameFamily, fs_index: u8) -> Result<&'static [u16]> {
    match family {
        FrameFamily::Lc1024 => long_window_offsets(fs_index),
        FrameFamily::Lc960 => {
            let idx = fs_index as usize;
            let slice = SWB_OFFSET_LONG_WINDOW_960
                .get(idx)
                .copied()
                .unwrap_or(&[][..]);
            if slice.is_empty() {
                return Err(Error::IcsInfoUnsupportedSampleRateIndex(fs_index));
            }
            Ok(slice)
        }
        FrameFamily::Ld512 => Ok(SWB_OFFSET_LD_512[ld_table_slot(fs_index)?]),
        FrameFamily::Ld480 => Ok(SWB_OFFSET_LD_480[ld_table_slot(fs_index)?]),
    }
}

/// Family-aware `swb_offset_short_window[fs_index]` lookup.
///
/// `Lc1024` reads the Tables 4.130–4.141 primary columns
/// (== [`short_window_offsets`]), `Lc960` their bracketed
/// 240-transform columns. The LD families have no short windows at
/// all (§4.6.17.2.2 — no block switching), so the lookup itself is
/// invalid and surfaces [`Error::LdShortWindow`].
pub fn short_window_offsets_family(family: FrameFamily, fs_index: u8) -> Result<&'static [u16]> {
    match family {
        FrameFamily::Lc1024 => short_window_offsets(fs_index),
        FrameFamily::Lc960 => {
            let idx = fs_index as usize;
            let slice = SWB_OFFSET_SHORT_WINDOW_120
                .get(idx)
                .copied()
                .unwrap_or(&[][..]);
            if slice.is_empty() {
                return Err(Error::IcsInfoUnsupportedSampleRateIndex(fs_index));
            }
            Ok(slice)
        }
        FrameFamily::Ld512 | FrameFamily::Ld480 => Err(Error::LdShortWindow),
    }
}

/// Apply the §4.6.13 pulse-escape reconstruction to a long-window
/// quantised spectrum.
///
/// The decoder pseudocode in ISO/IEC 14496-3 §4.6.13 is:
///
/// ```text
/// if (pulse_data_present) {
///     k = swb_offset_long_window[fs_index][pulse_start_sfb];
///     for (i = 0; i < number_pulse + 1; i++) {
///         k += pulse_offset[i];
///         if (x_quant[k] > 0)
///             x_quant[k] += pulse_amp[i];
///         else
///             x_quant[k] -= pulse_amp[i];
///     }
///  }
/// ```
///
/// `x_quant` is the per-coefficient quantised spectrum from
/// `spectral_data()`; pulse fix-ups overwrite the residual the encoder
/// shaved off the literal escape codeword.
///
/// ## Inputs
///
/// * `x_quant` — `&mut [i32]`, length must be at least
///   [`LONG_WINDOW_LEN`] (1024). Note: §4.4.6.3 normatively forbids
///   `pulse_data_present` on `EIGHT_SHORT_SEQUENCE` frames, so the
///   only window-sequence context this loop runs on is long
///   (long / long_start / long_stop). The short-window spectrum is
///   never touched.
/// * `fs_index` — `samplingFrequencyIndex` (Table 1.18, 0..=11). Selects
///   `swb_offset_long_window[fs_index]`.
/// * `pulse_data` — the parsed [`PulseData`] block. `pulses` must be in
///   `1..=4`; `pulse_start_sfb` must be in
///   `0..long_window_offsets(fs_index).len() - 1` (i.e. addressable
///   without going past the last real band).
///
/// ## Errors
///
/// * [`Error::IcsInfoUnsupportedSampleRateIndex`] if `fs_index` has no
///   long-window SWB table.
/// * [`Error::PulseDataEncodeInvalid`] if:
///   * `pulse_data.pulses.is_empty()` or `> 4` (Table 4.7 cap),
///   * `pulse_data.pulse_start_sfb` indexes past the last real
///     scalefactor band (`>= long_offsets.len() - 1`),
///   * the running coefficient index `k` reaches or exceeds
///     [`LONG_WINDOW_LEN`] (the per-pulse offset accumulation runs off
///     the end of the spectrum). All three checks correspond to
///     conditions a conforming AAC encoder will never produce — this
///     surfaces malformed bitstreams or caller bugs.
/// * `x_quant.len() < LONG_WINDOW_LEN` panics in debug, saturates the
///   slice length in release. (The caller is expected to pass a
///   correctly-sized buffer; misuse here is a programming error, not
///   a wire-format violation.)
pub fn apply_pulse_data(x_quant: &mut [i32], fs_index: u8, pulse_data: &PulseData) -> Result<()> {
    apply_pulse_data_family(x_quant, FrameFamily::Lc1024, fs_index, pulse_data)
}

/// [`apply_pulse_data`] generalized to any [`FrameFamily`]: the band
/// start `k` is read from the family's own long-window table and the
/// running index is bounded by the family's long spectrum length.
pub fn apply_pulse_data_family(
    x_quant: &mut [i32],
    family: FrameFamily,
    fs_index: u8,
    pulse_data: &PulseData,
) -> Result<()> {
    // A corrupted stream can pair a pulse_data_present flag with a
    // group buffer shorter than the family frame length (e.g. a
    // flipped window_sequence bit) — reject rather than assert.
    if x_quant.len() < family.frame_len() {
        return Err(Error::PulseDataEncodeInvalid);
    }

    if pulse_data.pulses.is_empty() || pulse_data.pulses.len() > crate::pulse_data::MAX_PULSES {
        return Err(Error::PulseDataEncodeInvalid);
    }

    let offsets = long_window_offsets_family(family, fs_index)?;
    let start_sfb = pulse_data.pulse_start_sfb as usize;
    // Last entry of the offsets slice is the sentinel; bands are
    // addressable at indices 0..offsets.len() - 1.
    if start_sfb >= offsets.len() - 1 {
        return Err(Error::PulseDataEncodeInvalid);
    }

    let mut k = offsets[start_sfb] as usize;
    let len = x_quant.len().min(family.frame_len());
    for pulse in &pulse_data.pulses {
        k += pulse.offset as usize;
        if k >= len {
            return Err(Error::PulseDataEncodeInvalid);
        }
        let amp = pulse.amp as i32;
        if x_quant[k] > 0 {
            x_quant[k] += amp;
        } else {
            x_quant[k] -= amp;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num_swb_long_window() -> [u8; 12] {
        // Mirror NUM_SWB_LONG_WINDOW in src/ics_info.rs without
        // referencing it from outside the module under test.
        [41, 41, 47, 49, 49, 51, 47, 47, 43, 43, 43, 40]
    }

    fn num_swb_short_window() -> [u8; 12] {
        [12, 12, 12, 14, 14, 14, 15, 15, 15, 15, 15, 15]
    }

    #[test]
    fn long_offset_lengths_match_num_swb() {
        let counts = num_swb_long_window();
        for fs_index in 0..12_u8 {
            let offsets = long_window_offsets(fs_index).unwrap();
            assert_eq!(
                offsets.len(),
                counts[fs_index as usize] as usize + 1,
                "fs_index {} long-window offset table length",
                fs_index,
            );
        }
    }

    #[test]
    fn short_offset_lengths_match_num_swb() {
        let counts = num_swb_short_window();
        for fs_index in 0..12_u8 {
            let offsets = short_window_offsets(fs_index).unwrap();
            assert_eq!(
                offsets.len(),
                counts[fs_index as usize] as usize + 1,
                "fs_index {} short-window offset table length",
                fs_index,
            );
        }
    }

    #[test]
    fn long_tables_start_at_zero_and_end_at_1024() {
        for fs_index in 0..12_u8 {
            let offsets = long_window_offsets(fs_index).unwrap();
            assert_eq!(offsets[0], 0, "fs_index {} first offset", fs_index);
            assert_eq!(
                *offsets.last().unwrap(),
                LONG_WINDOW_LEN,
                "fs_index {} sentinel",
                fs_index
            );
        }
    }

    #[test]
    fn short_tables_start_at_zero_and_end_at_128() {
        for fs_index in 0..12_u8 {
            let offsets = short_window_offsets(fs_index).unwrap();
            assert_eq!(offsets[0], 0, "fs_index {} first offset", fs_index);
            assert_eq!(
                *offsets.last().unwrap(),
                SHORT_WINDOW_LEN,
                "fs_index {} sentinel",
                fs_index
            );
        }
    }

    #[test]
    fn long_offsets_are_strictly_monotonic() {
        for fs_index in 0..12_u8 {
            let offsets = long_window_offsets(fs_index).unwrap();
            for w in offsets.windows(2) {
                assert!(
                    w[0] < w[1],
                    "fs_index {} non-monotonic at {} -> {}",
                    fs_index,
                    w[0],
                    w[1]
                );
            }
        }
    }

    #[test]
    fn short_offsets_are_strictly_monotonic() {
        for fs_index in 0..12_u8 {
            let offsets = short_window_offsets(fs_index).unwrap();
            for w in offsets.windows(2) {
                assert!(
                    w[0] < w[1],
                    "fs_index {} non-monotonic at {} -> {}",
                    fs_index,
                    w[0],
                    w[1]
                );
            }
        }
    }

    #[test]
    fn fs_index_7350_returns_unsupported() {
        assert!(matches!(
            long_window_offsets(12),
            Err(Error::IcsInfoUnsupportedSampleRateIndex(12))
        ));
        assert!(matches!(
            short_window_offsets(12),
            Err(Error::IcsInfoUnsupportedSampleRateIndex(12))
        ));
    }

    #[test]
    fn fs_index_out_of_range_returns_unsupported() {
        assert!(matches!(
            long_window_offsets(13),
            Err(Error::IcsInfoUnsupportedSampleRateIndex(13))
        ));
        assert!(matches!(
            long_window_offsets(15),
            Err(Error::IcsInfoUnsupportedSampleRateIndex(15))
        ));
        assert!(matches!(
            short_window_offsets(15),
            Err(Error::IcsInfoUnsupportedSampleRateIndex(15))
        ));
    }

    #[test]
    fn table_4_129_spot_check_48k() {
        // Table 4.129 — 44.1 / 48 kHz long window. 50 entries.
        let offsets = long_window_offsets(3).unwrap();
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[1], 4);
        assert_eq!(offsets[10], 40);
        assert_eq!(offsets[11], 48);
        assert_eq!(offsets[24], 196);
        assert_eq!(offsets[49], 1024);
        assert_eq!(offsets.len(), 50);
    }

    #[test]
    fn table_4_131_spot_check_32k() {
        // Table 4.131 — 32 kHz long window. 52 entries.
        let offsets = long_window_offsets(5).unwrap();
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[50], 992);
        assert_eq!(offsets[51], 1024);
        assert_eq!(offsets.len(), 52);
    }

    #[test]
    fn table_4_132_spot_check_8k() {
        // Table 4.132 — 8 kHz long window. 41 entries.
        let offsets = long_window_offsets(11).unwrap();
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[1], 12);
        assert_eq!(offsets[20], 268);
        assert_eq!(offsets[40], 1024);
        assert_eq!(offsets.len(), 41);
    }

    #[test]
    fn table_4_134_spot_check_16k() {
        // Table 4.134 — 11.025 / 12 / 16 kHz long window. 44 entries.
        let offsets = long_window_offsets(8).unwrap();
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[1], 8);
        assert_eq!(offsets[21], 212);
        assert_eq!(offsets[22], 228);
        assert_eq!(offsets[43], 1024);
        assert_eq!(offsets.len(), 44);
        // Same table also covers 12 kHz (fs 9) and 11.025 kHz (fs 10).
        assert_eq!(long_window_offsets(9).unwrap(), offsets);
        assert_eq!(long_window_offsets(10).unwrap(), offsets);
    }

    #[test]
    fn table_4_136_spot_check_24k() {
        // Table 4.136 — 22.05 / 24 kHz long window. 48 entries.
        let offsets = long_window_offsets(6).unwrap();
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[11], 44);
        assert_eq!(offsets[12], 52);
        assert_eq!(offsets[23], 148);
        assert_eq!(offsets[24], 160);
        assert_eq!(offsets[47], 1024);
        assert_eq!(offsets.len(), 48);
        assert_eq!(long_window_offsets(7).unwrap(), offsets);
    }

    #[test]
    fn table_4_138_spot_check_64k() {
        // Table 4.138 — 64 kHz long window. 48 entries.
        let offsets = long_window_offsets(2).unwrap();
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[14], 56);
        assert_eq!(offsets[15], 64);
        assert_eq!(offsets[22], 140);
        assert_eq!(offsets[46], 984);
        assert_eq!(offsets[47], 1024);
        assert_eq!(offsets.len(), 48);
    }

    #[test]
    fn table_4_140_spot_check_96k() {
        // Table 4.140 — 88.2 / 96 kHz long window. 42 entries.
        let offsets = long_window_offsets(0).unwrap();
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[20], 108);
        assert_eq!(offsets[21], 120);
        assert_eq!(offsets[41], 1024);
        assert_eq!(offsets.len(), 42);
        assert_eq!(long_window_offsets(1).unwrap(), offsets);
    }

    #[test]
    fn table_4_130_spot_check_48k_short() {
        // Table 4.130 — 32 / 44.1 / 48 kHz short window. 15 entries.
        let offsets = short_window_offsets(3).unwrap();
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[5], 20);
        assert_eq!(offsets[6], 28);
        assert_eq!(offsets[14], 128);
        assert_eq!(offsets.len(), 15);
        // Shared with 44.1 and 32 kHz.
        assert_eq!(short_window_offsets(4).unwrap(), offsets);
        assert_eq!(short_window_offsets(5).unwrap(), offsets);
    }

    #[test]
    fn table_4_133_spot_check_8k_short() {
        // Table 4.133 — 8 kHz short window. 16 entries.
        let offsets = short_window_offsets(11).unwrap();
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[7], 28);
        assert_eq!(offsets[8], 36);
        assert_eq!(offsets[15], 128);
        assert_eq!(offsets.len(), 16);
    }

    #[test]
    fn table_4_135_spot_check_16k_short() {
        // Table 4.135 — 11.025 / 12 / 16 kHz short window. 16 entries.
        let offsets = short_window_offsets(8).unwrap();
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[8], 32);
        assert_eq!(offsets[9], 40);
        assert_eq!(offsets[15], 128);
        assert_eq!(offsets.len(), 16);
        assert_eq!(short_window_offsets(9).unwrap(), offsets);
        assert_eq!(short_window_offsets(10).unwrap(), offsets);
    }

    #[test]
    fn table_4_137_spot_check_24k_short() {
        // Table 4.137 — 22.05 / 24 kHz short window. 16 entries.
        let offsets = short_window_offsets(6).unwrap();
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[7], 28);
        assert_eq!(offsets[8], 36);
        assert_eq!(offsets[11], 64);
        assert_eq!(offsets[15], 128);
        assert_eq!(offsets.len(), 16);
        assert_eq!(short_window_offsets(7).unwrap(), offsets);
    }

    #[test]
    fn table_4_139_spot_check_64k_short() {
        // Table 4.139 — 64 kHz short window. 13 entries.
        let offsets = short_window_offsets(2).unwrap();
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[6], 24);
        assert_eq!(offsets[7], 32);
        assert_eq!(offsets[11], 92);
        assert_eq!(offsets[12], 128);
        assert_eq!(offsets.len(), 13);
    }

    #[test]
    fn table_4_141_spot_check_96k_short() {
        // Table 4.141 — 88.2 / 96 kHz short window. 13 entries.
        let offsets = short_window_offsets(0).unwrap();
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[7], 32);
        assert_eq!(offsets[12], 128);
        assert_eq!(offsets.len(), 13);
        assert_eq!(short_window_offsets(1).unwrap(), offsets);
    }

    #[test]
    fn apply_pulse_data_single_positive_pulse_48k() {
        use crate::pulse_data::{Pulse, PulseData};
        // 48 kHz long, swb_offset_long[3] = 12, then a single pulse
        // with offset=5 (k = 12 + 5 = 17), amp=3, on a positive
        // x_quant: x_quant[17] += 3.
        let mut x_quant = vec![0_i32; 1024];
        x_quant[17] = 7;
        let pd = PulseData {
            pulse_start_sfb: 3,
            pulses: vec![Pulse { offset: 5, amp: 3 }],
        };
        apply_pulse_data(&mut x_quant, 3, &pd).unwrap();
        assert_eq!(x_quant[17], 10);
    }

    #[test]
    fn apply_pulse_data_single_negative_pulse_48k() {
        use crate::pulse_data::{Pulse, PulseData};
        // x_quant <= 0 (incl. 0): amp is subtracted.
        let mut x_quant = vec![0_i32; 1024];
        x_quant[17] = -7;
        let pd = PulseData {
            pulse_start_sfb: 3,
            pulses: vec![Pulse { offset: 5, amp: 3 }],
        };
        apply_pulse_data(&mut x_quant, 3, &pd).unwrap();
        assert_eq!(x_quant[17], -10);
    }

    #[test]
    fn apply_pulse_data_zero_coefficient_subtracts_amp() {
        use crate::pulse_data::{Pulse, PulseData};
        // Zero is not > 0, so it falls into the else branch and amp
        // is subtracted (matching the §4.6.13 pseudocode).
        let mut x_quant = vec![0_i32; 1024];
        let pd = PulseData {
            pulse_start_sfb: 0,
            pulses: vec![Pulse { offset: 1, amp: 4 }],
        };
        apply_pulse_data(&mut x_quant, 3, &pd).unwrap();
        assert_eq!(x_quant[1], -4);
    }

    #[test]
    fn apply_pulse_data_four_pulses_accumulate_k() {
        use crate::pulse_data::{Pulse, PulseData};
        // 48 kHz long, swb_offset_long[10] = 40. Four pulses with
        // offsets 1/2/3/4 land at k = 41, 43, 46, 50. All four target
        // coefficients are set positive so each is incremented by its
        // amplitude.
        let mut x_quant = vec![1_i32; 1024];
        let pd = PulseData {
            pulse_start_sfb: 10,
            pulses: vec![
                Pulse { offset: 1, amp: 1 },
                Pulse { offset: 2, amp: 2 },
                Pulse { offset: 3, amp: 3 },
                Pulse { offset: 4, amp: 4 },
            ],
        };
        apply_pulse_data(&mut x_quant, 3, &pd).unwrap();
        assert_eq!(x_quant[41], 2);
        assert_eq!(x_quant[43], 3);
        assert_eq!(x_quant[46], 4);
        assert_eq!(x_quant[50], 5);
    }

    #[test]
    fn apply_pulse_data_overrun_rejected() {
        use crate::pulse_data::{Pulse, PulseData};
        // Pulse offset that drives k past 1024 is rejected.
        let mut x_quant = vec![0_i32; 1024];
        let pd = PulseData {
            pulse_start_sfb: 48, // 48 kHz, swb_offset_long[48] = 928
            pulses: vec![Pulse { offset: 31, amp: 0 }; 4], // 928 + 4*31 = 1052
        };
        assert!(matches!(
            apply_pulse_data(&mut x_quant, 3, &pd),
            Err(Error::PulseDataEncodeInvalid)
        ));
    }

    #[test]
    fn apply_pulse_data_start_sfb_past_last_band_rejected() {
        use crate::pulse_data::{Pulse, PulseData};
        // 48 kHz long has 49 SWB; addressable band indices are 0..=48.
        let mut x_quant = vec![0_i32; 1024];
        let pd = PulseData {
            pulse_start_sfb: 49,
            pulses: vec![Pulse { offset: 1, amp: 1 }],
        };
        assert!(matches!(
            apply_pulse_data(&mut x_quant, 3, &pd),
            Err(Error::PulseDataEncodeInvalid)
        ));
    }

    #[test]
    fn apply_pulse_data_empty_pulses_rejected() {
        use crate::pulse_data::PulseData;
        let mut x_quant = vec![0_i32; 1024];
        let pd = PulseData {
            pulse_start_sfb: 0,
            pulses: vec![],
        };
        assert!(matches!(
            apply_pulse_data(&mut x_quant, 3, &pd),
            Err(Error::PulseDataEncodeInvalid)
        ));
    }

    #[test]
    fn apply_pulse_data_too_many_pulses_rejected() {
        use crate::pulse_data::{Pulse, PulseData};
        let mut x_quant = vec![0_i32; 1024];
        let pd = PulseData {
            pulse_start_sfb: 0,
            pulses: vec![Pulse { offset: 1, amp: 1 }; 5],
        };
        assert!(matches!(
            apply_pulse_data(&mut x_quant, 3, &pd),
            Err(Error::PulseDataEncodeInvalid)
        ));
    }

    #[test]
    fn apply_pulse_data_unsupported_fs_index_rejected() {
        use crate::pulse_data::{Pulse, PulseData};
        let mut x_quant = vec![0_i32; 1024];
        let pd = PulseData {
            pulse_start_sfb: 0,
            pulses: vec![Pulse { offset: 1, amp: 1 }],
        };
        assert!(matches!(
            apply_pulse_data(&mut x_quant, 12, &pd),
            Err(Error::IcsInfoUnsupportedSampleRateIndex(12))
        ));
    }

    #[test]
    fn long_widths_match_num_swb_sums() {
        // sum of per-band widths must equal LONG_WINDOW_LEN for every
        // table.
        for fs_index in 0..12_u8 {
            let offsets = long_window_offsets(fs_index).unwrap();
            let total: u32 = offsets.windows(2).map(|w| (w[1] - w[0]) as u32).sum();
            assert_eq!(total, LONG_WINDOW_LEN as u32);
        }
    }

    #[test]
    fn short_widths_match_num_swb_sums() {
        for fs_index in 0..12_u8 {
            let offsets = short_window_offsets(fs_index).unwrap();
            let total: u32 = offsets.windows(2).map(|w| (w[1] - w[0]) as u32).sum();
            assert_eq!(total, SHORT_WINDOW_LEN as u32);
        }
    }

    // -- FrameFamily geometry ------------------------------------------------

    #[test]
    fn family_resolution_follows_4_5_1_1() {
        assert_eq!(
            FrameFamily::from_aot_and_flag(2, false),
            FrameFamily::Lc1024
        );
        assert_eq!(FrameFamily::from_aot_and_flag(2, true), FrameFamily::Lc960);
        assert_eq!(FrameFamily::from_aot_and_flag(17, true), FrameFamily::Lc960);
        assert_eq!(
            FrameFamily::from_aot_and_flag(23, false),
            FrameFamily::Ld512
        );
        assert_eq!(FrameFamily::from_aot_and_flag(23, true), FrameFamily::Ld480);
    }

    #[test]
    fn family_lengths() {
        assert_eq!(FrameFamily::Lc1024.frame_len(), 1024);
        assert_eq!(FrameFamily::Lc1024.long_transform_len(), 2048);
        assert_eq!(FrameFamily::Lc1024.short_window_len(), Some(128));
        assert_eq!(FrameFamily::Lc1024.short_transform_len(), Some(256));
        assert_eq!(FrameFamily::Lc960.frame_len(), 960);
        assert_eq!(FrameFamily::Lc960.long_transform_len(), 1920);
        assert_eq!(FrameFamily::Lc960.short_window_len(), Some(120));
        assert_eq!(FrameFamily::Lc960.short_transform_len(), Some(240));
        assert_eq!(FrameFamily::Ld512.frame_len(), 512);
        assert_eq!(FrameFamily::Ld512.long_transform_len(), 1024);
        assert_eq!(FrameFamily::Ld512.short_window_len(), None);
        assert_eq!(FrameFamily::Ld480.frame_len(), 480);
        assert_eq!(FrameFamily::Ld480.long_transform_len(), 960);
        assert_eq!(FrameFamily::Ld480.short_window_len(), None);
        assert!(FrameFamily::Ld512.is_ld());
        assert!(FrameFamily::Ld480.is_ld());
        assert!(!FrameFamily::Lc1024.is_ld());
        assert!(!FrameFamily::Lc960.is_ld());
    }

    #[test]
    fn lc1024_family_lookup_matches_legacy_accessors() {
        for fs_index in 0..12_u8 {
            assert_eq!(
                long_window_offsets_family(FrameFamily::Lc1024, fs_index).unwrap(),
                long_window_offsets(fs_index).unwrap()
            );
            assert_eq!(
                short_window_offsets_family(FrameFamily::Lc1024, fs_index).unwrap(),
                short_window_offsets(fs_index).unwrap()
            );
        }
    }

    #[test]
    fn lc960_long_tables_are_the_bracketed_columns() {
        // Tables 4.129–4.140: the 1920-transform column shares every
        // band start with the 2048 column; the sentinel becomes 960
        // and any offsets >= 960 are dropped. So each 960 table must
        // be a strict prefix of its 1024 sibling with the sentinel
        // replaced by 960.
        for fs_index in 0..12_u8 {
            let long1024 = long_window_offsets(fs_index).unwrap();
            let long960 = long_window_offsets_family(FrameFamily::Lc960, fs_index).unwrap();
            let n = long960.len();
            assert_eq!(*long960.last().unwrap(), 960, "fs {} sentinel", fs_index);
            assert_eq!(
                &long960[..n - 1],
                &long1024[..n - 1],
                "fs {} shared band starts",
                fs_index
            );
            // Everything dropped from the 1024 table must be >= 960.
            for &off in &long1024[n - 1..] {
                assert!(off >= 960, "fs {} dropped offset {}", fs_index, off);
            }
            // Strictly monotonic, starts at zero.
            assert_eq!(long960[0], 0);
            for w in long960.windows(2) {
                assert!(w[0] < w[1], "fs {} non-monotonic", fs_index);
            }
        }
    }

    #[test]
    fn lc960_expected_num_swb() {
        // Bracket-derived band counts: 44.1/48 keeps all 49 bands
        // (only the sentinel shrinks); 32 kHz drops from 51 to 49;
        // 64 kHz prints `47 (46)` in Table 4.138; the rest drop
        // exactly the bands whose start would be >= 960.
        let expected: [usize; 12] = [40, 40, 46, 49, 49, 49, 46, 46, 42, 42, 42, 40];
        for fs_index in 0..12_u8 {
            let long960 = long_window_offsets_family(FrameFamily::Lc960, fs_index).unwrap();
            assert_eq!(
                long960.len() - 1,
                expected[fs_index as usize],
                "fs {} num_swb",
                fs_index
            );
        }
    }

    #[test]
    fn lc960_short_tables_only_rescale_the_sentinel() {
        for fs_index in 0..12_u8 {
            let short128 = short_window_offsets(fs_index).unwrap();
            let short120 = short_window_offsets_family(FrameFamily::Lc960, fs_index).unwrap();
            assert_eq!(short120.len(), short128.len(), "fs {}", fs_index);
            let n = short120.len();
            assert_eq!(&short120[..n - 1], &short128[..n - 1]);
            assert_eq!(short120[n - 1], 120);
            for w in short120.windows(2) {
                assert!(w[0] < w[1], "fs {} non-monotonic", fs_index);
            }
        }
    }

    #[test]
    fn ld_tables_match_spec_counts_and_sentinels() {
        // Table 4.143 / 4.145 / 4.147 — LD 512: 36 / 37 / 31 SWB.
        for (fs, num) in [(3u8, 36usize), (4, 36), (5, 37), (6, 31), (7, 31)] {
            let t = long_window_offsets_family(FrameFamily::Ld512, fs).unwrap();
            assert_eq!(t.len() - 1, num, "LD512 fs {}", fs);
            assert_eq!(t[0], 0);
            assert_eq!(*t.last().unwrap(), 512);
            for w in t.windows(2) {
                assert!(w[0] < w[1]);
            }
        }
        // Table 4.142 / 4.144 / 4.146 — LD 480: 35 / 37 / 30 SWB.
        for (fs, num) in [(3u8, 35usize), (4, 35), (5, 37), (6, 30), (7, 30)] {
            let t = long_window_offsets_family(FrameFamily::Ld480, fs).unwrap();
            assert_eq!(t.len() - 1, num, "LD480 fs {}", fs);
            assert_eq!(t[0], 0);
            assert_eq!(*t.last().unwrap(), 480);
            for w in t.windows(2) {
                assert!(w[0] < w[1]);
            }
        }
    }

    #[test]
    fn ld_512_spot_checks() {
        // Table 4.143 spot rows: swb 16 -> 68, swb 21 -> 112,
        // swb 27 -> 208, swb 35 -> 460.
        let t = long_window_offsets_family(FrameFamily::Ld512, 3).unwrap();
        assert_eq!(t[16], 68);
        assert_eq!(t[21], 112);
        assert_eq!(t[27], 208);
        assert_eq!(t[35], 460);
        // Table 4.145 spot rows: swb 15 -> 64, swb 20 -> 108,
        // swb 30 -> 288, swb 36 -> 480.
        let t = long_window_offsets_family(FrameFamily::Ld512, 5).unwrap();
        assert_eq!(t[15], 64);
        assert_eq!(t[20], 108);
        assert_eq!(t[30], 288);
        assert_eq!(t[36], 480);
        // Table 4.147 spot rows: swb 12 -> 52, swb 18 -> 120,
        // swb 25 -> 320, swb 30 -> 480.
        let t = long_window_offsets_family(FrameFamily::Ld512, 6).unwrap();
        assert_eq!(t[12], 52);
        assert_eq!(t[18], 120);
        assert_eq!(t[25], 320);
        assert_eq!(t[30], 480);
    }

    #[test]
    fn ld_480_spot_checks() {
        // Table 4.142 spot rows: swb 15 -> 64, swb 20 -> 108,
        // swb 27 -> 212, swb 34 -> 432.
        let t = long_window_offsets_family(FrameFamily::Ld480, 4).unwrap();
        assert_eq!(t[15], 64);
        assert_eq!(t[20], 108);
        assert_eq!(t[27], 212);
        assert_eq!(t[34], 432);
        // Table 4.144 spot rows: swb 17 -> 72, swb 23 -> 124,
        // swb 29 -> 224, swb 36 -> 448.
        let t = long_window_offsets_family(FrameFamily::Ld480, 5).unwrap();
        assert_eq!(t[17], 72);
        assert_eq!(t[23], 124);
        assert_eq!(t[29], 224);
        assert_eq!(t[36], 448);
        // Table 4.146 spot rows: swb 12 -> 52, swb 16 -> 92,
        // swb 22 -> 224, swb 29 -> 448.
        let t = long_window_offsets_family(FrameFamily::Ld480, 7).unwrap();
        assert_eq!(t[12], 52);
        assert_eq!(t[16], 92);
        assert_eq!(t[22], 224);
        assert_eq!(t[29], 448);
    }

    #[test]
    fn ld_nearest_defined_table_rule() {
        // §4.5.1.1: rates the LD tables omit resolve to the nearest
        // defined rate — 96/88.2/64 kHz to the 48 kHz table, 16 kHz
        // and below to the 22.05 kHz table.
        let t48 = long_window_offsets_family(FrameFamily::Ld512, 3).unwrap();
        for fs in [0u8, 1, 2, 4] {
            assert_eq!(
                long_window_offsets_family(FrameFamily::Ld512, fs).unwrap(),
                t48
            );
        }
        let t22 = long_window_offsets_family(FrameFamily::Ld512, 7).unwrap();
        for fs in [6u8, 8, 9, 10, 11] {
            assert_eq!(
                long_window_offsets_family(FrameFamily::Ld512, fs).unwrap(),
                t22
            );
        }
        assert!(matches!(
            long_window_offsets_family(FrameFamily::Ld512, 12),
            Err(Error::IcsInfoUnsupportedSampleRateIndex(12))
        ));
    }

    #[test]
    fn ld_short_lookup_is_rejected() {
        assert!(matches!(
            short_window_offsets_family(FrameFamily::Ld512, 3),
            Err(Error::LdShortWindow)
        ));
        assert!(matches!(
            short_window_offsets_family(FrameFamily::Ld480, 3),
            Err(Error::LdShortWindow)
        ));
    }

    #[test]
    fn family_widths_sum_to_family_lengths() {
        for family in [FrameFamily::Lc960, FrameFamily::Ld512, FrameFamily::Ld480] {
            for fs_index in 0..12_u8 {
                let long = long_window_offsets_family(family, fs_index).unwrap();
                assert_eq!(
                    *long.last().unwrap() as usize,
                    family.frame_len(),
                    "{:?} fs {} long sentinel",
                    family,
                    fs_index
                );
            }
        }
        for fs_index in 0..12_u8 {
            let short = short_window_offsets_family(FrameFamily::Lc960, fs_index).unwrap();
            assert_eq!(*short.last().unwrap(), 120);
        }
    }

    #[test]
    fn apply_pulse_data_family_uses_family_bounds() {
        use crate::pulse_data::{Pulse, PulseData};
        // LD512 at 48 kHz: swb_offset[35] == 460 is the last band.
        // A pulse landing at 460 + 40 = 500 stays inside the 512-line
        // spectrum, while the same pulse under a 1024-line check
        // would also pass — so also verify the overrun at >= 512.
        let mut x_quant = vec![1_i32; 512];
        let pd = PulseData {
            pulse_start_sfb: 35,
            pulses: vec![Pulse { offset: 31, amp: 2 }],
        };
        apply_pulse_data_family(&mut x_quant, FrameFamily::Ld512, 3, &pd).unwrap();
        assert_eq!(x_quant[491], 3);

        let mut x_quant = vec![1_i32; 512];
        let pd = PulseData {
            pulse_start_sfb: 35,
            pulses: vec![Pulse { offset: 31, amp: 2 }; 2], // 460+62 = 522 >= 512
        };
        assert!(matches!(
            apply_pulse_data_family(&mut x_quant, FrameFamily::Ld512, 3, &pd),
            Err(Error::PulseDataEncodeInvalid)
        ));
    }
}

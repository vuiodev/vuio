//! Static lookup tables taken directly from ATSC A/52:2018.
//!
//! Every table here is line-for-line from a numbered table in the spec —
//! call sites always cite the section/table so it stays greppable.

/// Table 5.6 — sampling frequency code (fscod) → sample rate in Hz.
///
/// fscod `'11'` is **reserved** and the decoder must mute on receipt
/// (spec §5.4.1.3); this table returns `None` for that case.
pub fn sample_rate_hz(fscod: u8) -> Option<u32> {
    match fscod {
        0 => Some(48_000),
        1 => Some(44_100),
        2 => Some(32_000),
        _ => None,
    }
}

/// Table 5.18 — Frame Size Code Table (1 word = 16 bits).
///
/// Each entry is `(nominal_kbps, words_32k, words_44k, words_48k)`.
/// frmsizecod ranges 0..=37 (6 bits, but values 38..=63 are reserved /
/// invalid); each nominal bitrate has two neighbouring frmsizecod values
/// because 44.1 kHz encoding must alternate frame sizes to hit the
/// declared bit-rate on average (Table 5.18 shows both sizes).
pub const FRAME_SIZE_TABLE: &[(u32, u32, u32, u32)] = &[
    (32, 96, 69, 64),
    (32, 96, 70, 64),
    (40, 120, 87, 80),
    (40, 120, 88, 80),
    (48, 144, 104, 96),
    (48, 144, 105, 96),
    (56, 168, 121, 112),
    (56, 168, 122, 112),
    (64, 192, 139, 128),
    (64, 192, 140, 128),
    (80, 240, 174, 160),
    (80, 240, 175, 160),
    (96, 288, 208, 192),
    (96, 288, 209, 192),
    (112, 336, 243, 224),
    (112, 336, 244, 224),
    (128, 384, 278, 256),
    (128, 384, 279, 256),
    (160, 480, 348, 320),
    (160, 480, 349, 320),
    (192, 576, 417, 384),
    (192, 576, 418, 384),
    (224, 672, 487, 448),
    (224, 672, 488, 448),
    (256, 768, 557, 512),
    (256, 768, 558, 512),
    (320, 960, 696, 640),
    (320, 960, 697, 640),
    (384, 1152, 835, 768),
    (384, 1152, 836, 768),
    (448, 1344, 975, 896),
    (448, 1344, 976, 896),
    (512, 1536, 1114, 1024),
    (512, 1536, 1115, 1024),
    (576, 1728, 1253, 1152),
    (576, 1728, 1254, 1152),
    (640, 1920, 1393, 1280),
    (640, 1920, 1394, 1280),
];

/// Compute the total frame length in **bytes** from (fscod, frmsizecod).
///
/// Returns `None` for reserved fscod (=3) or out-of-range frmsizecod
/// (>= 38). The table is indexed in "16-bit words per syncframe"; we
/// multiply by 2 so the result is the byte length the demuxer / parser
/// expects to consume before the next syncword.
pub fn frame_length_bytes(fscod: u8, frmsizecod: u8) -> Option<u32> {
    let frmsizecod = frmsizecod as usize;
    if frmsizecod >= FRAME_SIZE_TABLE.len() {
        return None;
    }
    let (_, w32, w44, w48) = FRAME_SIZE_TABLE[frmsizecod];
    let words = match fscod {
        0 => w48,
        1 => w44,
        2 => w32,
        _ => return None,
    };
    Some(words * 2)
}

/// Return the nominal bit rate in kbps for a given frmsizecod. The two
/// frmsizecod entries per bitrate are collapsed here since the bitrate
/// is identical. Returns `None` when out of range.
pub fn nominal_bitrate_kbps(frmsizecod: u8) -> Option<u32> {
    let i = frmsizecod as usize;
    if i >= FRAME_SIZE_TABLE.len() {
        return None;
    }
    Some(FRAME_SIZE_TABLE[i].0)
}

/// Table 5.8 — Audio Coding Mode (acmod) → (nfchans, channel ordering).
///
/// `nfchans` here is the number of *full-bandwidth* channels; the total
/// channel count (`nchans`) is `nfchans + 1` when the LFE channel is on.
pub fn acmod_nfchans(acmod: u8) -> u8 {
    match acmod {
        0 => 2, // 1+1 (Ch1, Ch2 — dual mono)
        1 => 1, // 1/0 (C)
        2 => 2, // 2/0 (L, R)
        3 => 3, // 3/0 (L, C, R)
        4 => 3, // 2/1 (L, R, S)
        5 => 4, // 3/1 (L, C, R, S)
        6 => 4, // 2/2 (L, R, SL, SR)
        7 => 5, // 3/2 (L, C, R, SL, SR)
        _ => 0,
    }
}

// ===========================================================================
// Bit allocation tables (§7.2.3) — all values reproduced exactly from the
// spec. Addresses marked in comments.
// ===========================================================================

/// Table 7.6 — Slow Decay Table.
pub const SLOWDEC: [i32; 4] = [0x0f, 0x11, 0x13, 0x15];

/// Table 7.7 — Fast Decay Table.
pub const FASTDEC: [i32; 4] = [0x3f, 0x53, 0x67, 0x7b];

/// Table 7.8 — Slow Gain Table.
pub const SLOWGAIN: [i32; 4] = [0x540, 0x4d8, 0x478, 0x410];

/// Table 7.9 — dB/Bit Table.
pub const DBPBTAB: [i32; 4] = [0x000, 0x700, 0x900, 0xb00];

/// Table 7.10 — Floor Table. Entries are 16-bit signed integers in the
/// spec; address 7 (0xf800) represents –2048.
pub const FLOORTAB: [i32; 8] = [0x2f0, 0x2b0, 0x270, 0x230, 0x1f0, 0x170, 0x0f0, -2048];

/// Table 7.11 — Fast Gain Table.
pub const FASTGAIN: [i32; 8] = [0x080, 0x100, 0x180, 0x200, 0x280, 0x300, 0x380, 0x400];

/// Table 7.12 — Banding Structure Tables. First column is `bndtab[band]`
/// (first mantissa bin in each band), second is `bndsz[band]` (width in
/// mantissa bins). 50 entries.
pub const BNDTAB: [u32; 50] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 31, 34, 37, 40, 43, 46, 49, 55, 61, 67, 73, 79, 85, 97, 109, 121, 133, 157, 181,
    205, 229,
];

pub const BNDSZ: [u32; 50] = [
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 3, 3, 3,
    3, 3, 3, 6, 6, 6, 6, 6, 6, 12, 12, 12, 12, 24, 24, 24, 24, 24,
];

/// Table 7.13 — Bin Number to Band Number Table. masktab[bin] = band.
/// Indexed as `(10 * A) + B` with A=0..25, B=0..9. We unroll the full
/// 256-entry mapping for easy bin→band lookup.
pub const MASKTAB: [u8; 256] = {
    let mut t = [0u8; 256];
    // Row 0 — 10 entries
    let rows: &[(usize, [u8; 10])] = &[
        (0, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
        (1, [10, 11, 12, 13, 14, 15, 16, 17, 18, 19]),
        (2, [20, 21, 22, 23, 24, 25, 26, 27, 28, 28]),
        (3, [28, 29, 29, 29, 30, 30, 30, 31, 31, 31]),
        (4, [32, 32, 32, 33, 33, 33, 34, 34, 34, 35]),
        (5, [35, 35, 35, 35, 35, 36, 36, 36, 36, 36]),
        (6, [36, 37, 37, 37, 37, 37, 37, 38, 38, 38]),
        (7, [38, 38, 38, 39, 39, 39, 39, 39, 39, 40]),
        (8, [40, 40, 40, 40, 40, 41, 41, 41, 41, 41]),
        (9, [41, 41, 41, 41, 41, 41, 41, 42, 42, 42]),
        (10, [42, 42, 42, 42, 42, 42, 42, 42, 42, 43]),
        (11, [43, 43, 43, 43, 43, 43, 43, 43, 43, 43]),
        (12, [43, 44, 44, 44, 44, 44, 44, 44, 44, 44]),
        (13, [44, 44, 44, 45, 45, 45, 45, 45, 45, 45]),
        (14, [45, 45, 45, 45, 45, 45, 45, 45, 45, 45]),
        (15, [45, 45, 45, 45, 45, 45, 45, 46, 46, 46]),
        (16, [46, 46, 46, 46, 46, 46, 46, 46, 46, 46]),
        (17, [46, 46, 46, 46, 46, 46, 46, 46, 46, 46]),
        (18, [46, 47, 47, 47, 47, 47, 47, 47, 47, 47]),
        (19, [47, 47, 47, 47, 47, 47, 47, 47, 47, 47]),
        (20, [47, 47, 47, 47, 47, 48, 48, 48, 48, 48]),
        (21, [48, 48, 48, 48, 48, 48, 48, 48, 48, 48]),
        (22, [48, 48, 48, 48, 48, 48, 48, 48, 48, 49]),
        (23, [49, 49, 49, 49, 49, 49, 49, 49, 49, 49]),
        (24, [49, 49, 49, 49, 49, 49, 49, 49, 49, 49]),
        (25, [49, 49, 49, 0, 0, 0, 0, 0, 0, 0]),
    ];
    let mut r = 0;
    while r < rows.len() {
        let (a, row) = rows[r];
        let mut b = 0;
        while b < 10 {
            let bin = 10 * a + b;
            if bin < 256 {
                t[bin] = row[b];
            }
            b += 1;
        }
        r += 1;
    }
    t
};

/// Table 7.14 — Log-Addition Table, `latab[val]` with val = 10*A + B.
///
/// 256 spec entries (A=0..25, B=0..9; A=25 only B=0..5), transcribed
/// row-for-row from ATSC A/52:2018 Table 7.14 — one A-row per source line.
/// The array is length 260 so that indices 256..259 (never reached:
/// `logadd` clamps `address = min(|c|>>1, 255)`) are padded with 0x0000.
pub const LATAB: [u16; 260] = [
    // A=0
    0x0040, 0x003f, 0x003e, 0x003d, 0x003c, 0x003b, 0x003a, 0x0039, 0x0038, 0x0037,
    // A=1
    0x0036, 0x0035, 0x0034, 0x0034, 0x0033, 0x0032, 0x0031, 0x0030, 0x002f, 0x002f,
    // A=2
    0x002e, 0x002d, 0x002c, 0x002c, 0x002b, 0x002a, 0x0029, 0x0029, 0x0028, 0x0027,
    // A=3
    0x0026, 0x0026, 0x0025, 0x0024, 0x0024, 0x0023, 0x0023, 0x0022, 0x0021, 0x0021,
    // A=4
    0x0020, 0x0020, 0x001f, 0x001e, 0x001e, 0x001d, 0x001d, 0x001c, 0x001c, 0x001b,
    // A=5
    0x001b, 0x001a, 0x001a, 0x0019, 0x0019, 0x0018, 0x0018, 0x0017, 0x0017, 0x0016,
    // A=6
    0x0016, 0x0015, 0x0015, 0x0015, 0x0014, 0x0014, 0x0013, 0x0013, 0x0013, 0x0012,
    // A=7
    0x0012, 0x0012, 0x0011, 0x0011, 0x0011, 0x0010, 0x0010, 0x0010, 0x000f, 0x000f,
    // A=8
    0x000f, 0x000e, 0x000e, 0x000e, 0x000d, 0x000d, 0x000d, 0x000d, 0x000c, 0x000c,
    // A=9
    0x000c, 0x000c, 0x000b, 0x000b, 0x000b, 0x000b, 0x000a, 0x000a, 0x000a, 0x000a,
    // A=10
    0x000a, 0x0009, 0x0009, 0x0009, 0x0009, 0x0009, 0x0008, 0x0008, 0x0008, 0x0008,
    // A=11
    0x0008, 0x0008, 0x0007, 0x0007, 0x0007, 0x0007, 0x0007, 0x0007, 0x0006, 0x0006,
    // A=12
    0x0006, 0x0006, 0x0006, 0x0006, 0x0006, 0x0006, 0x0005, 0x0005, 0x0005, 0x0005,
    // A=13
    0x0005, 0x0005, 0x0005, 0x0005, 0x0004, 0x0004, 0x0004, 0x0004, 0x0004, 0x0004,
    // A=14
    0x0004, 0x0004, 0x0004, 0x0004, 0x0004, 0x0003, 0x0003, 0x0003, 0x0003, 0x0003,
    // A=15
    0x0003, 0x0003, 0x0003, 0x0003, 0x0003, 0x0003, 0x0003, 0x0003, 0x0003, 0x0002,
    // A=16
    0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002,
    // A=17
    0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0001, 0x0001,
    // A=18
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    // A=19
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    // A=20
    0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
    // A=21
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    // A=22
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    // A=23
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    // A=24
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    // A=25 (B=0..5)
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    // pad to length 260 (indices 256..259, never indexed)
    0x0000, 0x0000, 0x0000, 0x0000,
];

/// Table 7.15 — Hearing Threshold Table, `hth[fscod][band]`, 50 bands × 3 fscods.
/// Row i column c gives hth[c=fscod][i=band].
pub const HTH: [[u16; 50]; 3] = [
    // fscod = 0 (48 kHz)
    [
        0x04d0, 0x04d0, 0x0440, 0x0400, 0x03e0, 0x03c0, 0x03b0, 0x03b0, 0x03a0, 0x03a0, 0x03a0,
        0x03a0, 0x03a0, 0x0390, 0x0390, 0x0390, 0x0380, 0x0380, 0x0370, 0x0370, 0x0360, 0x0360,
        0x0350, 0x0350, 0x0340, 0x0340, 0x0330, 0x0320, 0x0310, 0x0300, 0x02f0, 0x02f0, 0x02f0,
        0x02f0, 0x0300, 0x0310, 0x0340, 0x0390, 0x03e0, 0x0420, 0x0460, 0x0490, 0x04a0, 0x0460,
        0x0440, 0x0440, 0x0520, 0x0800, 0x0840, 0x0840,
    ],
    // fscod = 1 (44.1 kHz)
    [
        0x04f0, 0x04f0, 0x0460, 0x0410, 0x03e0, 0x03d0, 0x03c0, 0x03b0, 0x03b0, 0x03a0, 0x03a0,
        0x03a0, 0x03a0, 0x03a0, 0x0390, 0x0390, 0x0390, 0x0380, 0x0380, 0x0380, 0x0370, 0x0370,
        0x0360, 0x0360, 0x0350, 0x0340, 0x0340, 0x0340, 0x0310, 0x0310, 0x0300, 0x02f0, 0x02f0,
        0x02f0, 0x0300, 0x0300, 0x0320, 0x0350, 0x0390, 0x03e0, 0x0420, 0x0450, 0x04a0, 0x0490,
        0x0460, 0x0440, 0x0480, 0x0630, 0x0840, 0x0840,
    ],
    // fscod = 2 (32 kHz)
    [
        0x0580, 0x0580, 0x04b0, 0x0450, 0x0420, 0x03f0, 0x03e0, 0x03d0, 0x03c0, 0x03b0, 0x03b0,
        0x03b0, 0x03a0, 0x03a0, 0x03a0, 0x03a0, 0x03a0, 0x03a0, 0x03a0, 0x03a0, 0x0390, 0x0390,
        0x0390, 0x0390, 0x0380, 0x0380, 0x0380, 0x0370, 0x0360, 0x0350, 0x0340, 0x0330, 0x0320,
        0x0310, 0x0300, 0x02f0, 0x02f0, 0x02f0, 0x0300, 0x0310, 0x0330, 0x0350, 0x03c0, 0x0410,
        0x0470, 0x04a0, 0x0460, 0x0440, 0x0450, 0x04e0,
    ],
];

/// Table 7.16 — Bit Allocation Pointer Table (`baptab`), 64 entries.
/// Spec intervals: 0→0, 1..=5→1, 6..=7→2, 8..=10→3, 11..=12→4, 13..=14→5,
/// 15..=18→6, 19..=22→7, 23..=26→8, 27..=30→9, 31..=34→10, 35..=38→11,
/// 39..=42→12, 43..=46→13, 47..=54→14, 55..=63→15.
pub const BAPTAB: [u8; 64] = [
    0, 1, 1, 1, 1, 1, 2, 2, 3, 3, 3, 4, 4, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7, 8, 8, 8, 8, 9, 9, 9, 9,
    10, 10, 10, 10, 11, 11, 11, 11, 12, 12, 12, 12, 13, 13, 13, 13, 14, 14, 14, 14, 14, 14, 14, 14,
    15, 15, 15, 15, 15, 15, 15, 15, 15,
];

/// Table 7.17/7.18 — Quantizer levels and mantissa bits per bap.
pub const QUANTIZATION_BITS: [u8; 16] = [0, 5, 7, 3, 7, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14, 16];

/// Table 7.19 — bap=1 (3-level) mantissa map: -2/3, 0, 2/3.
pub const MANT_LEVEL_3: [f32; 3] = [-2.0 / 3.0, 0.0, 2.0 / 3.0];

/// Table 7.20 — bap=2 (5-level) mantissa map.
pub const MANT_LEVEL_5: [f32; 5] = [-4.0 / 5.0, -2.0 / 5.0, 0.0, 2.0 / 5.0, 4.0 / 5.0];

/// Table 7.21 — bap=3 (7-level) mantissa map.
pub const MANT_LEVEL_7: [f32; 7] = [
    -6.0 / 7.0,
    -4.0 / 7.0,
    -2.0 / 7.0,
    0.0,
    2.0 / 7.0,
    4.0 / 7.0,
    6.0 / 7.0,
];

/// Table 7.22 — bap=4 (11-level) mantissa map.
pub const MANT_LEVEL_11: [f32; 11] = [
    -10.0 / 11.0,
    -8.0 / 11.0,
    -6.0 / 11.0,
    -4.0 / 11.0,
    -2.0 / 11.0,
    0.0,
    2.0 / 11.0,
    4.0 / 11.0,
    6.0 / 11.0,
    8.0 / 11.0,
    10.0 / 11.0,
];

/// Table 7.23 — bap=5 (15-level) mantissa map.
pub const MANT_LEVEL_15: [f32; 15] = [
    -14.0 / 15.0,
    -12.0 / 15.0,
    -10.0 / 15.0,
    -8.0 / 15.0,
    -6.0 / 15.0,
    -4.0 / 15.0,
    -2.0 / 15.0,
    0.0,
    2.0 / 15.0,
    4.0 / 15.0,
    6.0 / 15.0,
    8.0 / 15.0,
    10.0 / 15.0,
    12.0 / 15.0,
    14.0 / 15.0,
];

/// Table 7.33 — Transform Window Sequence w[n], 256 entries, symmetric.
/// The table lists only the first 256 samples; the full 512-sample
/// window is symmetric with w[511-n] = w[n] for an MDCT window. AC-3's
/// KBD window meets the TDAC constraint that w[n]^2 + w[n+256]^2 = 1.
/// Values given to 5 decimals — accurate enough for 16-bit PCM output.
// reason: one KBD sample (0.78530) coincidentally matches FRAC_PI_4 to five
// decimals but is a tabulated window coefficient, not π/4.
#[allow(clippy::approx_constant)]
pub const WINDOW: [f32; 256] = [
    0.00014, 0.00024, 0.00037, 0.00051, 0.00067, 0.00086, 0.00107, 0.00130, 0.00157, 0.00187,
    0.00220, 0.00256, 0.00297, 0.00341, 0.00390, 0.00443, 0.00501, 0.00564, 0.00632, 0.00706,
    0.00785, 0.00871, 0.00962, 0.01061, 0.01166, 0.01279, 0.01399, 0.01526, 0.01662, 0.01806,
    0.01959, 0.02121, 0.02292, 0.02472, 0.02662, 0.02863, 0.03073, 0.03294, 0.03527, 0.03770,
    0.04025, 0.04292, 0.04571, 0.04862, 0.05165, 0.05481, 0.05810, 0.06153, 0.06508, 0.06878,
    0.07261, 0.07658, 0.08069, 0.08495, 0.08935, 0.09389, 0.09859, 0.10343, 0.10842, 0.11356,
    0.11885, 0.12429, 0.12988, 0.13563, 0.14152, 0.14757, 0.15376, 0.16011, 0.16661, 0.17325,
    0.18005, 0.18699, 0.19407, 0.20130, 0.20867, 0.21618, 0.22382, 0.23161, 0.23952, 0.24757,
    0.25574, 0.26404, 0.27246, 0.28100, 0.28965, 0.29841, 0.30729, 0.31626, 0.32533, 0.33450,
    0.34376, 0.35311, 0.36253, 0.37204, 0.38161, 0.39126, 0.40096, 0.41072, 0.42054, 0.43040,
    0.44030, 0.45023, 0.46020, 0.47019, 0.48020, 0.49022, 0.50025, 0.51028, 0.52031, 0.53033,
    0.54033, 0.55031, 0.56026, 0.57019, 0.58007, 0.58991, 0.59970, 0.60944, 0.61912, 0.62873,
    0.63827, 0.64774, 0.65713, 0.66643, 0.67564, 0.68476, 0.69377, 0.70269, 0.71150, 0.72019,
    0.72877, 0.73723, 0.74557, 0.75378, 0.76186, 0.76981, 0.77762, 0.78530, 0.79283, 0.80022,
    0.80747, 0.81457, 0.82151, 0.82831, 0.83496, 0.84145, 0.84779, 0.85398, 0.86001, 0.86588,
    0.87160, 0.87716, 0.88257, 0.88782, 0.89291, 0.89785, 0.90264, 0.90728, 0.91176, 0.91610,
    0.92028, 0.92432, 0.92822, 0.93197, 0.93558, 0.93906, 0.94240, 0.94560, 0.94867, 0.95162,
    0.95444, 0.95713, 0.95971, 0.96217, 0.96451, 0.96674, 0.96887, 0.97089, 0.97281, 0.97463,
    0.97635, 0.97799, 0.97953, 0.98099, 0.98236, 0.98366, 0.98488, 0.98602, 0.98710, 0.98811,
    0.98905, 0.98994, 0.99076, 0.99153, 0.99225, 0.99291, 0.99353, 0.99411, 0.99464, 0.99513,
    0.99558, 0.99600, 0.99639, 0.99674, 0.99706, 0.99736, 0.99763, 0.99788, 0.99811, 0.99831,
    0.99850, 0.99867, 0.99882, 0.99895, 0.99908, 0.99919, 0.99929, 0.99938, 0.99946, 0.99953,
    0.99959, 0.99965, 0.99969, 0.99974, 0.99978, 0.99981, 0.99984, 0.99986, 0.99988, 0.99990,
    0.99992, 0.99993, 0.99994, 0.99995, 0.99996, 0.99997, 0.99998, 0.99998, 0.99998, 0.99999,
    0.99999, 0.99999, 0.99999, 1.00000, 1.00000, 1.00000, 1.00000, 1.00000, 1.00000, 1.00000,
    1.00000, 1.00000, 1.00000, 1.00000, 1.00000, 1.00000,
];

/// Table 5.9 — Center Mix Level coefficients: cmixlev → clev.
pub const CENTER_MIX_LEVEL: [f32; 4] = [0.707, 0.595, 0.500, 0.595];

/// Table 5.10 — Surround Mix Level coefficients: surmixlev → slev.
pub const SURROUND_MIX_LEVEL: [f32; 4] = [0.707, 0.500, 0.0, 0.500];

#[cfg(test)]
mod tests {
    use super::LATAB;

    /// Independently-transcribed oracle of ATSC A/52:2018 Table 7.14
    /// (`latab[val]`, val = 10*A + B, A=0..25, B=0..9; A=25 only B=0..5).
    /// 256 spec entries. Guards `LATAB[0..256]` against run-length
    /// regressions like issue #10 (understated mid-table runs at A=12/13/14/17
    /// caused railed full-scale bursts on multi-tone content).
    #[rustfmt::skip]
    const LATAB_SPEC: [u16; 256] = [
        // A=0
        0x0040, 0x003f, 0x003e, 0x003d, 0x003c, 0x003b, 0x003a, 0x0039, 0x0038, 0x0037,
        // A=1
        0x0036, 0x0035, 0x0034, 0x0034, 0x0033, 0x0032, 0x0031, 0x0030, 0x002f, 0x002f,
        // A=2
        0x002e, 0x002d, 0x002c, 0x002c, 0x002b, 0x002a, 0x0029, 0x0029, 0x0028, 0x0027,
        // A=3
        0x0026, 0x0026, 0x0025, 0x0024, 0x0024, 0x0023, 0x0023, 0x0022, 0x0021, 0x0021,
        // A=4
        0x0020, 0x0020, 0x001f, 0x001e, 0x001e, 0x001d, 0x001d, 0x001c, 0x001c, 0x001b,
        // A=5
        0x001b, 0x001a, 0x001a, 0x0019, 0x0019, 0x0018, 0x0018, 0x0017, 0x0017, 0x0016,
        // A=6
        0x0016, 0x0015, 0x0015, 0x0015, 0x0014, 0x0014, 0x0013, 0x0013, 0x0013, 0x0012,
        // A=7
        0x0012, 0x0012, 0x0011, 0x0011, 0x0011, 0x0010, 0x0010, 0x0010, 0x000f, 0x000f,
        // A=8
        0x000f, 0x000e, 0x000e, 0x000e, 0x000d, 0x000d, 0x000d, 0x000d, 0x000c, 0x000c,
        // A=9
        0x000c, 0x000c, 0x000b, 0x000b, 0x000b, 0x000b, 0x000a, 0x000a, 0x000a, 0x000a,
        // A=10
        0x000a, 0x0009, 0x0009, 0x0009, 0x0009, 0x0009, 0x0008, 0x0008, 0x0008, 0x0008,
        // A=11
        0x0008, 0x0008, 0x0007, 0x0007, 0x0007, 0x0007, 0x0007, 0x0007, 0x0006, 0x0006,
        // A=12
        0x0006, 0x0006, 0x0006, 0x0006, 0x0006, 0x0006, 0x0005, 0x0005, 0x0005, 0x0005,
        // A=13
        0x0005, 0x0005, 0x0005, 0x0005, 0x0004, 0x0004, 0x0004, 0x0004, 0x0004, 0x0004,
        // A=14
        0x0004, 0x0004, 0x0004, 0x0004, 0x0004, 0x0003, 0x0003, 0x0003, 0x0003, 0x0003,
        // A=15
        0x0003, 0x0003, 0x0003, 0x0003, 0x0003, 0x0003, 0x0003, 0x0003, 0x0003, 0x0002,
        // A=16
        0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002,
        // A=17
        0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0002, 0x0001, 0x0001,
        // A=18
        0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
        // A=19
        0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
        // A=20
        0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001,
        // A=21
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        // A=22
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        // A=23
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        // A=24
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        // A=25 (B=0..5)
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    ];

    #[test]
    fn latab_matches_spec_table_7_14() {
        for (val, &expected) in LATAB_SPEC.iter().enumerate() {
            assert_eq!(
                LATAB[val],
                expected,
                "LATAB[{val}] (A={}, B={}) = 0x{:04x}, spec Table 7.14 = 0x{:04x}",
                val / 10,
                val % 10,
                LATAB[val],
                expected,
            );
        }
        // Tail padding (indices 256..259) is never indexed by `logadd`.
        for &v in &LATAB[256..260] {
            assert_eq!(v, 0x0000);
        }
    }
}

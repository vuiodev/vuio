//! Numeric tables for the ER BSAC noiseless coder — ISO/IEC
//! 14496-3:2009 §4.A.5 (Tables 4.A.31–4.A.77), transcribed from the
//! staged specification PDF.
//!
//! Three table families live here:
//!
//! * **General arithmetic models** — 14-bit cumulative-frequency
//!   arrays consumed by the §4.5.2.6.2.7.4 `decode_symbol()`
//!   procedure: the scalefactor models (Tables 4.A.37–4.A.43,
//!   selected via Table 4.A.32), the `cband_si` models (Tables
//!   4.A.44–4.A.50 for coding bands past the 0th, Table 4.A.51 for
//!   the 0th, selected via Table 4.A.31), and the stereo / PNS
//!   side-info models (Tables 4.A.52–4.A.55). Every array is
//!   strictly decreasing and ends in 0 (the `cum_freq[sym] > cum`
//!   symbol search walks it in order).
//! * **Binary probability tables** — the 22 spectral bit-slice
//!   tables (Tables 4.A.56–4.A.77), each a set of `p0` rows (14-bit
//!   probability of the "0" symbol) indexed by the significance
//!   distance from the coding band's MSB plane, the neighbouring
//!   lines' context (Table 4.A.34 position), and the line's own
//!   decoded higher bits. Tables 11–22 are normative aliases of
//!   tables 9 / 10 at higher MSB planes; tables 9 / 10 alias their
//!   zero-context sub-MSB rows onto tables 7 / 8 (the spec states
//!   the aliases verbatim). [`spectral_p0`] resolves the whole
//!   scheme.
//! * **Context / clamp tables** — the Table 4.A.34 position map
//!   ([`context_position`]), and the Table 4.A.35 / 4.A.36
//!   `min_p0` / `max_p0` clamps applied when a layer's remaining
//!   budget drops under 14 bits ([`clamp_p0`]).

/// Table 4.A.31 row: parameters of one `cband_si_type`.
#[derive(Debug, Clone, Copy)]
pub struct CbandSiTypeParams {
    /// `max_cband_si_len` — the side-info bit allowance used by the
    /// §4.5.2.6.2.5 `layer_si_maxlen` accumulation.
    pub max_len: u8,
    /// Largest decodable `cband_si` for the 0th coding band.
    pub largest_cband0: u8,
    /// Largest decodable `cband_si` for every other coding band.
    pub largest_other: u8,
    /// Index into [`CBAND_SI_MODELS`] for the non-0th coding bands
    /// (the 0th band always uses [`CBAND_SI_MODEL_CBAND0`]).
    pub other_model: u8,
}

/// Table 4.A.31 — `cband_si_type` parameters (32 rows).
pub const CBAND_SI_TYPES: [CbandSiTypeParams; 32] = [
    CbandSiTypeParams {
        max_len: 6,
        largest_cband0: 6,
        largest_other: 4,
        other_model: 0,
    },
    CbandSiTypeParams {
        max_len: 5,
        largest_cband0: 6,
        largest_other: 6,
        other_model: 1,
    },
    CbandSiTypeParams {
        max_len: 6,
        largest_cband0: 8,
        largest_other: 4,
        other_model: 0,
    },
    CbandSiTypeParams {
        max_len: 5,
        largest_cband0: 8,
        largest_other: 6,
        other_model: 1,
    },
    CbandSiTypeParams {
        max_len: 6,
        largest_cband0: 8,
        largest_other: 8,
        other_model: 2,
    },
    CbandSiTypeParams {
        max_len: 6,
        largest_cband0: 10,
        largest_other: 4,
        other_model: 0,
    },
    CbandSiTypeParams {
        max_len: 5,
        largest_cband0: 10,
        largest_other: 6,
        other_model: 1,
    },
    CbandSiTypeParams {
        max_len: 6,
        largest_cband0: 10,
        largest_other: 8,
        other_model: 2,
    },
    CbandSiTypeParams {
        max_len: 5,
        largest_cband0: 10,
        largest_other: 10,
        other_model: 3,
    },
    CbandSiTypeParams {
        max_len: 6,
        largest_cband0: 12,
        largest_other: 4,
        other_model: 0,
    },
    CbandSiTypeParams {
        max_len: 5,
        largest_cband0: 12,
        largest_other: 6,
        other_model: 1,
    },
    CbandSiTypeParams {
        max_len: 6,
        largest_cband0: 12,
        largest_other: 8,
        other_model: 2,
    },
    CbandSiTypeParams {
        max_len: 8,
        largest_cband0: 12,
        largest_other: 12,
        other_model: 4,
    },
    CbandSiTypeParams {
        max_len: 6,
        largest_cband0: 14,
        largest_other: 4,
        other_model: 0,
    },
    CbandSiTypeParams {
        max_len: 5,
        largest_cband0: 14,
        largest_other: 6,
        other_model: 1,
    },
    CbandSiTypeParams {
        max_len: 6,
        largest_cband0: 14,
        largest_other: 8,
        other_model: 2,
    },
    CbandSiTypeParams {
        max_len: 8,
        largest_cband0: 14,
        largest_other: 12,
        other_model: 4,
    },
    CbandSiTypeParams {
        max_len: 9,
        largest_cband0: 14,
        largest_other: 14,
        other_model: 5,
    },
    CbandSiTypeParams {
        max_len: 6,
        largest_cband0: 15,
        largest_other: 4,
        other_model: 0,
    },
    CbandSiTypeParams {
        max_len: 5,
        largest_cband0: 15,
        largest_other: 6,
        other_model: 1,
    },
    CbandSiTypeParams {
        max_len: 6,
        largest_cband0: 15,
        largest_other: 8,
        other_model: 2,
    },
    CbandSiTypeParams {
        max_len: 8,
        largest_cband0: 15,
        largest_other: 12,
        other_model: 4,
    },
    CbandSiTypeParams {
        max_len: 10,
        largest_cband0: 15,
        largest_other: 15,
        other_model: 6,
    },
    CbandSiTypeParams {
        max_len: 8,
        largest_cband0: 16,
        largest_other: 12,
        other_model: 4,
    },
    CbandSiTypeParams {
        max_len: 10,
        largest_cband0: 16,
        largest_other: 16,
        other_model: 6,
    },
    CbandSiTypeParams {
        max_len: 9,
        largest_cband0: 17,
        largest_other: 14,
        other_model: 5,
    },
    CbandSiTypeParams {
        max_len: 10,
        largest_cband0: 17,
        largest_other: 17,
        other_model: 6,
    },
    CbandSiTypeParams {
        max_len: 10,
        largest_cband0: 18,
        largest_other: 18,
        other_model: 6,
    },
    CbandSiTypeParams {
        max_len: 12,
        largest_cband0: 19,
        largest_other: 19,
        other_model: 6,
    },
    CbandSiTypeParams {
        max_len: 12,
        largest_cband0: 20,
        largest_other: 20,
        other_model: 6,
    },
    CbandSiTypeParams {
        max_len: 12,
        largest_cband0: 21,
        largest_other: 21,
        other_model: 6,
    },
    CbandSiTypeParams {
        max_len: 12,
        largest_cband0: 22,
        largest_other: 22,
        other_model: 6,
    },
];

/// Table 4.A.32 — largest differential value decodable under each
/// `scf_model` (model 0 is "not used": no scalefactor decoding).
pub const SCF_MODEL_LARGEST: [u8; 8] = [0, 3, 7, 15, 15, 31, 31, 63];

/// Table 4.A.33 — MSB plane per `cband_si` (0..=22). The MSB plane
/// is the highest bit-slice a coefficient in the coding band
/// carries; `cband_si == 0` means the band decodes to all zeros.
pub const CBAND_SI_MSB_PLANE: [u8; 23] = [
    0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 9, 10, 11, 12, 13, 14, 15,
];

/// Table 4.A.35 — minimum `p0` when the layer's available length is
/// `1..=13` bits (index 0 unused).
pub const MIN_P0: [u16; 14] = [
    0, 0x2000, 0x1000, 0x0800, 0x0400, 0x0200, 0x0100, 0x0080, 0x0040, 0x0020, 0x0010, 0x0008,
    0x0004, 0x0002,
];

/// Table 4.A.36 — maximum `p0` when the layer's available length is
/// `1..=13` bits (index 0 unused).
pub const MAX_P0: [u16; 14] = [
    0, 0x2000, 0x3000, 0x3800, 0x3c00, 0x3e00, 0x3f00, 0x3f80, 0x3fc0, 0x3fe0, 0x3ff0, 0x3ff8,
    0x3ffc, 0x3ffe,
];

/// §4.6.4.2.3: clamp a spectral-bit `p0` onto the Table 4.A.35 /
/// 4.A.36 band when fewer than 14 bits remain in the layer.
pub fn clamp_p0(p0: u16, available_len: i64) -> u16 {
    if (1..14).contains(&available_len) {
        let i = available_len as usize;
        p0.clamp(MIN_P0[i], MAX_P0[i])
    } else {
        p0
    }
}

/// The §4.5.2.6.2.2.13 sign-bit probability: `p0 = 0.5` as a 14-bit
/// fixed-point number.
pub const SIGN_P0: u16 = 0x2000;

/// Scalefactor arithmetic model 1 (Table 4.A.37).
pub const SCF_MODEL_1: [u16; 4] = [0x0752, 0x03cd, 0x014d, 0x0000];

/// Scalefactor arithmetic model 2 (Table 4.A.38).
pub const SCF_MODEL_2: [u16; 8] = [
    0x112f, 0x0de7, 0x0a8b, 0x07c1, 0x047a, 0x023a, 0x00d4, 0x0000,
];

/// Scalefactor arithmetic model 3 (Table 4.A.39).
pub const SCF_MODEL_3: [u16; 16] = [
    0x1f67, 0x1c5f, 0x18d8, 0x1555, 0x1215, 0x0eb4, 0x0adc, 0x0742, 0x0408, 0x01e6, 0x00df, 0x0052,
    0x0032, 0x0023, 0x000c, 0x0000,
];

/// Scalefactor arithmetic model 4 (Table 4.A.40).
pub const SCF_MODEL_4: [u16; 16] = [
    0x250f, 0x22b8, 0x2053, 0x1deb, 0x1b05, 0x186d, 0x15df, 0x12d9, 0x0f77, 0x0c01, 0x0833, 0x050d,
    0x0245, 0x008c, 0x0033, 0x0000,
];

/// Scalefactor arithmetic model 5 (Table 4.A.41).
pub const SCF_MODEL_5: [u16; 32] = [
    0x08a8, 0x074e, 0x0639, 0x0588, 0x048c, 0x03cf, 0x032e, 0x0272, 0x01bc, 0x013e, 0x00e4, 0x0097,
    0x0069, 0x0043, 0x002f, 0x0029, 0x0020, 0x001b, 0x0018, 0x0015, 0x0012, 0x000f, 0x000d, 0x000c,
    0x000a, 0x0009, 0x0007, 0x0006, 0x0004, 0x0003, 0x0001, 0x0000,
];

/// Scalefactor arithmetic model 6 (Table 4.A.42).
pub const SCF_MODEL_6: [u16; 32] = [
    0x0c2a, 0x099f, 0x0809, 0x06ec, 0x0603, 0x053d, 0x0491, 0x040e, 0x0394, 0x030a, 0x02a5, 0x0259,
    0x0202, 0x01bc, 0x0170, 0x0133, 0x0102, 0x00c9, 0x0097, 0x0073, 0x004f, 0x0037, 0x0022, 0x0016,
    0x000f, 0x000b, 0x0009, 0x0007, 0x0005, 0x0003, 0x0001, 0x0000,
];

/// Scalefactor arithmetic model 7 (Table 4.A.43).
pub const SCF_MODEL_7: [u16; 64] = [
    0x3b5e, 0x3a90, 0x39d3, 0x387c, 0x3702, 0x3566, 0x33a7, 0x321c, 0x2f90, 0x2cf2, 0x29fe, 0x26fa,
    0x23e4, 0x20df, 0x1e0d, 0x1ac4, 0x1804, 0x159a, 0x131e, 0x10e7, 0x0e5b, 0x0c9c, 0x0b78, 0x0a21,
    0x08fd, 0x07b7, 0x06b5, 0x062c, 0x055d, 0x04f6, 0x04d4, 0x044b, 0x038e, 0x02e2, 0x029d, 0x0236,
    0x0225, 0x01f2, 0x01cf, 0x01ad, 0x019c, 0x0179, 0x0168, 0x0157, 0x0146, 0x0135, 0x0123, 0x0112,
    0x0101, 0x00f0, 0x00df, 0x00ce, 0x00bc, 0x00ab, 0x009a, 0x0089, 0x0078, 0x0067, 0x0055, 0x0044,
    0x0033, 0x0022, 0x0011, 0x0000,
];

/// cband_si arithmetic model 0 (Table 4.A.44).
pub const CBAND_SI_MODEL_0: [u16; 5] = [0x3ef6, 0x3b59, 0x1b12, 0x12a3, 0x0000];

/// cband_si arithmetic model 1 (Table 4.A.45).
pub const CBAND_SI_MODEL_1: [u16; 7] = [0x3d51, 0x33ae, 0x1cff, 0x0fb7, 0x07e4, 0x022b, 0x0000];

/// cband_si arithmetic model 2 (Table 4.A.46).
pub const CBAND_SI_MODEL_2: [u16; 9] = [
    0x3a47, 0x2aec, 0x1e05, 0x1336, 0x0e7d, 0x0860, 0x05e0, 0x044a, 0x0000,
];

/// cband_si arithmetic model 3 (Table 4.A.47).
pub const CBAND_SI_MODEL_3: [u16; 11] = [
    0x36be, 0x27ae, 0x20f4, 0x1749, 0x14d5, 0x0d46, 0x0ad3, 0x0888, 0x0519, 0x020b, 0x0000,
];

/// cband_si arithmetic model 4 (Table 4.A.48).
pub const CBAND_SI_MODEL_4: [u16; 13] = [
    0x3983, 0x2e77, 0x2b03, 0x1ee8, 0x1df9, 0x1307, 0x11e4, 0x0b4d, 0x094c, 0x0497, 0x0445, 0x0040,
    0x0000,
];

/// cband_si arithmetic model 5 (Table 4.A.49).
pub const CBAND_SI_MODEL_5: [u16; 15] = [
    0x306f, 0x249e, 0x1f56, 0x1843, 0x161a, 0x102d, 0x0f6c, 0x0c81, 0x0af2, 0x07a8, 0x071a, 0x0454,
    0x0413, 0x0016, 0x0000,
];

/// cband_si arithmetic model 6 (Table 4.A.50).
pub const CBAND_SI_MODEL_6: [u16; 23] = [
    0x31af, 0x2001, 0x162d, 0x127e, 0x0f05, 0x0c34, 0x0b8f, 0x0a61, 0x0955, 0x0825, 0x07dd, 0x06a9,
    0x0688, 0x055b, 0x054b, 0x02f7, 0x0198, 0x0077, 0x0010, 0x000c, 0x0008, 0x0004, 0x0000,
];

/// cband_si arithmetic model for the 0th coding band (Table 4.A.51).
pub const CBAND_SI_MODEL_CBAND0: [u16; 23] = [
    0x3ff8, 0x3ff0, 0x3fe8, 0x3fe0, 0x3fd7, 0x3f31, 0x3cd7, 0x3bc9, 0x3074, 0x2bcf, 0x231b, 0x13db,
    0x0d51, 0x0603, 0x044c, 0x0080, 0x0030, 0x0028, 0x0020, 0x0018, 0x0010, 0x0008, 0x0000,
];

/// MS_used model (Table 4.A.52).
pub const MS_USED_MODEL: [u16; 2] = [0x2ccd, 0x0000];

/// stereo_info model (Table 4.A.53).
pub const STEREO_INFO_MODEL: [u16; 4] = [0x3666, 0x1000, 0x0666, 0x0000];

/// noise_flag arithmetic model (Table 4.A.54).
pub const NOISE_FLAG_MODEL: [u16; 2] = [0x2000, 0x0000];

/// noise_mode arithmetic model (Table 4.A.55).
pub const NOISE_MODE_MODEL: [u16; 4] = [0x3000, 0x2000, 0x1000, 0x0000];

/// BSAC probability table 1 (MSB plane 1), MSB row (Table 4.A.56).
pub const PROB_T1_MSB: [u16; 15] = [
    0x3900, 0x3a00, 0x2f00, 0x3b00, 0x2f00, 0x3700, 0x2c00, 0x3b00, 0x3000, 0x3600, 0x2d00, 0x3900,
    0x2f00, 0x3700, 0x2c00,
];

/// BSAC probability table 2 (MSB plane 1), MSB row (Table 4.A.57).
pub const PROB_T2_MSB: [u16; 15] = [
    0x2800, 0x2800, 0x2500, 0x2900, 0x2600, 0x2700, 0x2300, 0x2a00, 0x2700, 0x2800, 0x2400, 0x2800,
    0x2500, 0x2600, 0x2200,
];

/// BSAC probability table 3 (MSB plane 2), MSB row (Table 4.A.58).
pub const PROB_T3_MSB: [u16; 15] = [
    0x3d00, 0x3d00, 0x3300, 0x3d00, 0x3300, 0x3b00, 0x3300, 0x3d00, 0x3200, 0x3b00, 0x3100, 0x3e00,
    0x3700, 0x3c00, 0x3300,
];

/// BSAC probability table 3, MSB-1, zero higher bits (Table 4.A.58).
pub const PROB_T3_ZERO_1: [u16; 65] = [
    0x3700, 0x3a00, 0x2800, 0x3b00, 0x2600, 0x2c00, 0x2400, 0x3a00, 0x2500, 0x2b00, 0x2400, 0x3100,
    0x2300, 0x2900, 0x2300, 0x3000, 0x2c00, 0x1d00, 0x2200, 0x1a00, 0x1c00, 0x1600, 0x2700, 0x2200,
    0x1a00, 0x1d00, 0x1900, 0x1c00, 0x1e00, 0x2c00, 0x2400, 0x1900, 0x1e00, 0x1f00, 0x1c00, 0x2b00,
    0x2400, 0x2900, 0x2700, 0x2400, 0x1300, 0x1a00, 0x2000, 0x1800, 0x2300, 0x2500, 0x1f00, 0x2c00,
    0x2300, 0x3600, 0x2800, 0x3100, 0x2500, 0x1400, 0x1200, 0x1800, 0x1400, 0x2100, 0x2200, 0x1000,
    0x1e00, 0x3000, 0x2600, 0x1200, 0x2200,
];

/// BSAC probability table 3, MSB-1, non-zero higher bits (Table 4.A.58).
pub const PROB_T3_NZ_1: [u16; 1] = [0x3100];

/// BSAC probability table 4 (MSB plane 2), MSB row (Table 4.A.59).
pub const PROB_T4_MSB: [u16; 15] = [
    0x3900, 0x3a00, 0x2e00, 0x3a00, 0x2f00, 0x3400, 0x2a00, 0x3a00, 0x3000, 0x3500, 0x2c00, 0x3600,
    0x2b00, 0x3100, 0x2500,
];

/// BSAC probability table 4, MSB-1, zero higher bits (Table 4.A.59).
pub const PROB_T4_ZERO_1: [u16; 65] = [
    0x1e00, 0x1d00, 0x1c00, 0x1d00, 0x1c00, 0x1d00, 0x1b00, 0x1d00, 0x1e00, 0x1e00, 0x1a00, 0x1e00,
    0x1c00, 0x1d00, 0x1b00, 0x1a00, 0x1a00, 0x1800, 0x1800, 0x1800, 0x1700, 0x1700, 0x1800, 0x1a00,
    0x1700, 0x1700, 0x1900, 0x1800, 0x1600, 0x1700, 0x1600, 0x1500, 0x1700, 0x1800, 0x1600, 0x1c00,
    0x1700, 0x1900, 0x1700, 0x1500, 0x1c00, 0x1500, 0x1600, 0x0f00, 0x1800, 0x1400, 0x1700, 0x1a00,
    0x1a00, 0x1e00, 0x1800, 0x1c00, 0x1b00, 0x1500, 0x1300, 0x1500, 0x1400, 0x1600, 0x1500, 0x1700,
    0x1600, 0x1b00, 0x1800, 0x1400, 0x1400,
];

/// BSAC probability table 4, MSB-1, non-zero higher bits (Table 4.A.59).
pub const PROB_T4_NZ_1: [u16; 1] = [0x3600];

/// BSAC probability table 5 (MSB plane 3), MSB row (Table 4.A.60).
pub const PROB_T5_MSB: [u16; 15] = [
    0x3d00, 0x3d00, 0x3200, 0x3d00, 0x3300, 0x3d00, 0x3600, 0x3d00, 0x3500, 0x3c00, 0x3500, 0x3f00,
    0x3b00, 0x3f00, 0x3d00,
];

/// BSAC probability table 5, MSB-1, zero higher bits (Table 4.A.60).
pub const PROB_T5_ZERO_1: [u16; 65] = [
    0x3c00, 0x3d00, 0x2b00, 0x3d00, 0x2900, 0x3500, 0x2c00, 0x3d00, 0x2b00, 0x3400, 0x2b00, 0x3800,
    0x2b00, 0x3700, 0x2a00, 0x3900, 0x3400, 0x2400, 0x2a00, 0x1c00, 0x1f00, 0x1600, 0x3500, 0x2500,
    0x1a00, 0x2a00, 0x2200, 0x2b00, 0x2a00, 0x3500, 0x2600, 0x1a00, 0x2600, 0x2500, 0x2700, 0x3500,
    0x2d00, 0x3800, 0x3200, 0x2e00, 0x1800, 0x1600, 0x2900, 0x2500, 0x3100, 0x2c00, 0x2300, 0x3600,
    0x3000, 0x3c00, 0x3300, 0x3b00, 0x3400, 0x1700, 0x1a00, 0x1c00, 0x1900, 0x2900, 0x2a00, 0x2400,
    0x2700, 0x3c00, 0x3600, 0x1d00, 0x3100,
];

/// BSAC probability table 5, MSB-1, non-zero higher bits (Table 4.A.60).
pub const PROB_T5_NZ_1: [u16; 1] = [0x3100];

/// BSAC probability table 5, MSB-2, zero higher bits (Table 4.A.60).
pub const PROB_T5_ZERO_2: [u16; 65] = [
    0x3400, 0x3800, 0x2700, 0x3900, 0x2700, 0x2f00, 0x2200, 0x3800, 0x2500, 0x2d00, 0x2000, 0x3300,
    0x2000, 0x2900, 0x1e00, 0x2b00, 0x2300, 0x1a00, 0x1a00, 0x1b00, 0x1800, 0x1700, 0x1e00, 0x1c00,
    0x1b00, 0x1c00, 0x1b00, 0x1a00, 0x1800, 0x1d00, 0x1b00, 0x1800, 0x1900, 0x1b00, 0x1a00, 0x1d00,
    0x1e00, 0x1f00, 0x1b00, 0x1e00, 0x1200, 0x1400, 0x1a00, 0x1300, 0x1c00, 0x1b00, 0x1900, 0x2000,
    0x1e00, 0x3000, 0x2900, 0x2d00, 0x2500, 0x1300, 0x1700, 0x1400, 0x1300, 0x1e00, 0x1f00, 0x1100,
    0x1900, 0x2100, 0x1e00, 0x1500, 0x1a00,
];

/// BSAC probability table 5, MSB-2, non-zero higher bits (Table 4.A.60).
pub const PROB_T5_NZ_2: [u16; 3] = [0x2a00, 0x2b00, 0x2800];

/// BSAC probability table 6 (MSB plane 3), MSB row (Table 4.A.61).
pub const PROB_T6_MSB: [u16; 15] = [
    0x3800, 0x3a00, 0x2d00, 0x3a00, 0x2d00, 0x3600, 0x2d00, 0x3a00, 0x2d00, 0x3600, 0x2b00, 0x3a00,
    0x2800, 0x3600, 0x2700,
];

/// BSAC probability table 6, MSB-1, zero higher bits (Table 4.A.61).
pub const PROB_T6_ZERO_1: [u16; 65] = [
    0x2b00, 0x3000, 0x2500, 0x2f00, 0x2600, 0x2d00, 0x2400, 0x3000, 0x2500, 0x2b00, 0x2400, 0x2d00,
    0x2500, 0x2800, 0x2500, 0x2a00, 0x2900, 0x2300, 0x2200, 0x1e00, 0x1b00, 0x1900, 0x2600, 0x2300,
    0x1f00, 0x1d00, 0x2200, 0x1b00, 0x1800, 0x2100, 0x2100, 0x1d00, 0x1d00, 0x1f00, 0x1f00, 0x2900,
    0x2600, 0x2a00, 0x2100, 0x2300, 0x1800, 0x1a00, 0x1d00, 0x2000, 0x1c00, 0x1a00, 0x1e00, 0x2900,
    0x2800, 0x2f00, 0x2300, 0x2f00, 0x2600, 0x1d00, 0x1700, 0x1d00, 0x1c00, 0x1e00, 0x2100, 0x1700,
    0x2200, 0x2300, 0x2300, 0x1400, 0x1a00,
];

/// BSAC probability table 6, MSB-1, non-zero higher bits (Table 4.A.61).
pub const PROB_T6_NZ_1: [u16; 1] = [0x3000];

/// BSAC probability table 6, MSB-2, zero higher bits (Table 4.A.61).
pub const PROB_T6_ZERO_2: [u16; 65] = [
    0x1900, 0x1900, 0x1900, 0x1b00, 0x1700, 0x1b00, 0x1a00, 0x1000, 0x1900, 0x1600, 0x1800, 0x1e00,
    0x1900, 0x1a00, 0x1700, 0x1b00, 0x1700, 0x1500, 0x1500, 0x1500, 0x1700, 0x1400, 0x1900, 0x1700,
    0x1600, 0x1600, 0x1200, 0x1300, 0x1200, 0x1600, 0x1500, 0x1500, 0x1300, 0x1600, 0x1600, 0x1c00,
    0x1400, 0x1700, 0x1600, 0x1400, 0x1400, 0x1400, 0x1500, 0x1400, 0x1300, 0x1300, 0x1500, 0x1800,
    0x1600, 0x1f00, 0x1a00, 0x1e00, 0x1800, 0x1700, 0x1600, 0x1600, 0x1300, 0x1400, 0x1300, 0x1100,
    0x1500, 0x1600, 0x1500, 0x1200, 0x1300,
];

/// BSAC probability table 6, MSB-2, non-zero higher bits (Table 4.A.61).
pub const PROB_T6_NZ_2: [u16; 3] = [0x2b00, 0x2800, 0x2700];

/// BSAC probability table 7 (MSB plane 4), MSB row (Table 4.A.62).
pub const PROB_T7_MSB: [u16; 15] = [
    0x3d00, 0x3d00, 0x3500, 0x3e00, 0x3500, 0x3f00, 0x3b00, 0x3e00, 0x3200, 0x3f00, 0x3a00, 0x3f00,
    0x3d00, 0x3f00, 0x3b00,
];

/// BSAC probability table 7, MSB-1, zero higher bits (Table 4.A.62).
pub const PROB_T7_ZERO_1: [u16; 65] = [
    0x3f00, 0x3f00, 0x3200, 0x3f00, 0x3500, 0x3e00, 0x3700, 0x3f00, 0x2d00, 0x3c00, 0x3000, 0x3f00,
    0x3700, 0x3e00, 0x3400, 0x3f00, 0x3900, 0x2600, 0x2f00, 0x1e00, 0x2400, 0x1500, 0x3700, 0x3100,
    0x1b00, 0x2600, 0x2300, 0x3a00, 0x3900, 0x3e00, 0x2b00, 0x2200, 0x2800, 0x2f00, 0x2500, 0x3e00,
    0x3700, 0x3e00, 0x3d00, 0x3900, 0x1a00, 0x3300, 0x2500, 0x2800, 0x3c00, 0x3800, 0x2c00, 0x3d00,
    0x3800, 0x3f00, 0x3b00, 0x3f00, 0x3a00, 0x1e00, 0x1b00, 0x1800, 0x1800, 0x3b00, 0x3a00, 0x1200,
    0x2f00, 0x3f00, 0x3b00, 0x1b00, 0x3500,
];

/// BSAC probability table 7, MSB-1, non-zero higher bits (Table 4.A.62).
pub const PROB_T7_NZ_1: [u16; 1] = [0x2f00];

/// BSAC probability table 7, MSB-2, zero higher bits (Table 4.A.62).
pub const PROB_T7_ZERO_2: [u16; 65] = [
    0x3c00, 0x3e00, 0x3000, 0x3e00, 0x3100, 0x3a00, 0x3100, 0x3d00, 0x2c00, 0x3900, 0x2e00, 0x3c00,
    0x2d00, 0x3c00, 0x3100, 0x3d00, 0x3100, 0x2100, 0x2c00, 0x2600, 0x2800, 0x1d00, 0x2b00, 0x2800,
    0x2800, 0x2400, 0x2200, 0x2100, 0x2300, 0x2d00, 0x2500, 0x1f00, 0x2100, 0x2b00, 0x2700, 0x3200,
    0x2d00, 0x3400, 0x2a00, 0x3500, 0x1800, 0x1800, 0x1f00, 0x1e00, 0x2e00, 0x2a00, 0x2400, 0x3000,
    0x2b00, 0x3e00, 0x3d00, 0x3d00, 0x3a00, 0x1e00, 0x2b00, 0x2600, 0x1900, 0x3400, 0x3500, 0x1c00,
    0x2600, 0x3300, 0x2a00, 0x1c00, 0x2b00,
];

/// BSAC probability table 7, MSB-2, non-zero higher bits (Table 4.A.62).
pub const PROB_T7_NZ_2: [u16; 3] = [0x2800, 0x2900, 0x2400];

/// BSAC probability table 7, MSB-3 (others), zero higher bits (Table 4.A.62).
pub const PROB_T7_ZERO_3: [u16; 65] = [
    0x3500, 0x3b00, 0x2900, 0x3b00, 0x2a00, 0x3100, 0x2700, 0x3b00, 0x2600, 0x2f00, 0x2400, 0x3400,
    0x2300, 0x2d00, 0x2000, 0x3300, 0x2700, 0x1c00, 0x2400, 0x1c00, 0x1c00, 0x1900, 0x2700, 0x2800,
    0x1b00, 0x1d00, 0x2000, 0x1b00, 0x1a00, 0x2300, 0x1d00, 0x1700, 0x1e00, 0x2400, 0x2100, 0x2b00,
    0x2100, 0x2800, 0x2000, 0x2300, 0x1b00, 0x1500, 0x1b00, 0x1400, 0x1a00, 0x1a00, 0x2000, 0x2a00,
    0x2200, 0x3700, 0x2f00, 0x3200, 0x2a00, 0x1700, 0x1700, 0x1600, 0x1900, 0x2500, 0x2300, 0x1500,
    0x1900, 0x2500, 0x2200, 0x1400, 0x1b00,
];

/// BSAC probability table 7, MSB-3 (others), non-zero higher bits (Table 4.A.62).
pub const PROB_T7_NZ_3: [u16; 7] = [0x2d00, 0x2500, 0x2300, 0x2500, 0x2500, 0x2600, 0x2400];

/// BSAC probability table 8 (MSB plane 4), MSB row (Table 4.A.63).
pub const PROB_T8_MSB: [u16; 15] = [
    0x3b00, 0x3c00, 0x3400, 0x3c00, 0x3400, 0x3a00, 0x3000, 0x3c00, 0x3200, 0x3a00, 0x3100, 0x3c00,
    0x3000, 0x3900, 0x2f00,
];

/// BSAC probability table 8, MSB-1, zero higher bits (Table 4.A.63).
pub const PROB_T8_ZERO_1: [u16; 65] = [
    0x3500, 0x3800, 0x2c00, 0x3900, 0x2c00, 0x3400, 0x2b00, 0x3800, 0x2e00, 0x3400, 0x2d00, 0x3600,
    0x2a00, 0x3300, 0x2800, 0x3100, 0x3100, 0x2600, 0x2900, 0x2000, 0x2300, 0x1f00, 0x2d00, 0x2600,
    0x2000, 0x2600, 0x2300, 0x2500, 0x2100, 0x2c00, 0x2400, 0x1d00, 0x2500, 0x2400, 0x2400, 0x3000,
    0x2800, 0x3000, 0x2900, 0x2200, 0x1e00, 0x1c00, 0x2500, 0x1d00, 0x2300, 0x2300, 0x2500, 0x3300,
    0x2c00, 0x3700, 0x2b00, 0x3400, 0x2c00, 0x1e00, 0x1c00, 0x2100, 0x1b00, 0x2900, 0x2a00, 0x1d00,
    0x2600, 0x3200, 0x2a00, 0x2000, 0x2400,
];

/// BSAC probability table 8, MSB-1, non-zero higher bits (Table 4.A.63).
pub const PROB_T8_NZ_1: [u16; 1] = [0x3200];

/// BSAC probability table 8, MSB-2, zero higher bits (Table 4.A.63).
pub const PROB_T8_ZERO_2: [u16; 65] = [
    0x2900, 0x2e00, 0x2600, 0x2f00, 0x2600, 0x2d00, 0x2600, 0x2e00, 0x2500, 0x2b00, 0x2600, 0x2f00,
    0x2300, 0x2a00, 0x2300, 0x2800, 0x2800, 0x2100, 0x2400, 0x2000, 0x2000, 0x1b00, 0x2400, 0x1f00,
    0x1c00, 0x2100, 0x2200, 0x1d00, 0x1c00, 0x1f00, 0x1c00, 0x1900, 0x1e00, 0x2100, 0x2100, 0x2900,
    0x2200, 0x2300, 0x2100, 0x1c00, 0x1a00, 0x1a00, 0x2100, 0x2100, 0x1c00, 0x1c00, 0x1f00, 0x2700,
    0x2500, 0x2d00, 0x2700, 0x2a00, 0x2300, 0x1c00, 0x1d00, 0x1a00, 0x1a00, 0x1b00, 0x1d00, 0x1800,
    0x2000, 0x2300, 0x1f00, 0x1900, 0x1c00,
];

/// BSAC probability table 8, MSB-2, non-zero higher bits (Table 4.A.63).
pub const PROB_T8_NZ_2: [u16; 3] = [0x2b00, 0x2900, 0x2800];

/// BSAC probability table 8, MSB-3 (others), zero higher bits (Table 4.A.63).
pub const PROB_T8_ZERO_3: [u16; 65] = [
    0x1c00, 0x1e00, 0x1b00, 0x1e00, 0x1c00, 0x1e00, 0x1900, 0x1a00, 0x1f00, 0x1f00, 0x1900, 0x2000,
    0x1a00, 0x1f00, 0x1700, 0x1b00, 0x1a00, 0x1900, 0x1800, 0x1900, 0x1800, 0x1600, 0x1900, 0x1a00,
    0x1900, 0x1700, 0x1800, 0x1700, 0x1800, 0x1600, 0x1700, 0x1400, 0x1600, 0x1800, 0x1a00, 0x1c00,
    0x1c00, 0x1c00, 0x1700, 0x1700, 0x1500, 0x1500, 0x1600, 0x1600, 0x1500, 0x1400, 0x1700, 0x1b00,
    0x1a00, 0x2300, 0x1c00, 0x1d00, 0x1a00, 0x1600, 0x1600, 0x1500, 0x1400, 0x1800, 0x1500, 0x1300,
    0x1700, 0x1900, 0x1600, 0x1400, 0x1400,
];

/// BSAC probability table 8, MSB-3 (others), non-zero higher bits (Table 4.A.63).
pub const PROB_T8_NZ_3: [u16; 7] = [0x2800, 0x2500, 0x2500, 0x2700, 0x2500, 0x2600, 0x2500];

/// BSAC probability table 9 (MSB plane 5), MSB row (Table 4.A.64).
pub const PROB_T9_MSB: [u16; 15] = [
    0x3d00, 0x3e00, 0x3300, 0x3e00, 0x3500, 0x3e00, 0x3700, 0x3e00, 0x3400, 0x3e00, 0x3500, 0x3f00,
    0x3d00, 0x3f00, 0x3c00,
];

/// BSAC probability table 9, MSB-1, non-zero higher bits (Table 4.A.64).
pub const PROB_T9_NZ_1: [u16; 1] = [0x2e00];

/// BSAC probability table 9, MSB-2, non-zero higher bits (Table 4.A.64).
pub const PROB_T9_NZ_2: [u16; 3] = [0x2900, 0x2a00, 0x2700];

/// BSAC probability table 9, MSB-3, non-zero higher bits (Table 4.A.64).
pub const PROB_T9_NZ_3: [u16; 7] = [0x2d00, 0x2500, 0x2400, 0x2500, 0x2400, 0x2500, 0x2300];

/// BSAC probability table 9, others, non-zero higher bits (Table 4.A.64).
pub const PROB_T9_NZ_4: [u16; 16] = [
    0x2800, 0x2500, 0x2300, 0x2300, 0x2200, 0x2200, 0x2200, 0x2200, 0x2200, 0x2200, 0x2200, 0x2100,
    0x2000, 0x2200, 0x2100, 0x2000,
];

/// BSAC probability table 10 (MSB plane 5), MSB row (Table 4.A.65).
pub const PROB_T10_MSB: [u16; 15] = [
    0x3b00, 0x3c00, 0x3400, 0x3c00, 0x3200, 0x3900, 0x2e00, 0x3d00, 0x3400, 0x3900, 0x2f00, 0x3c00,
    0x2d00, 0x3700, 0x2d00,
];

/// BSAC probability table 10, MSB-1, non-zero higher bits (Table 4.A.65).
pub const PROB_T10_NZ_1: [u16; 1] = [0x3100];

/// BSAC probability table 10, MSB-2, non-zero higher bits (Table 4.A.65).
pub const PROB_T10_NZ_2: [u16; 3] = [0x2b00, 0x2a00, 0x2900];

/// BSAC probability table 10, MSB-3, non-zero higher bits (Table 4.A.65).
pub const PROB_T10_NZ_3: [u16; 7] = [0x2700, 0x2600, 0x2500, 0x2500, 0x2500, 0x2200, 0x2200];

/// BSAC probability table 10, others, non-zero higher bits (Table 4.A.65).
pub const PROB_T10_NZ_4: [u16; 16] = [
    0x2200, 0x2300, 0x2300, 0x2300, 0x2200, 0x2300, 0x2200, 0x2300, 0x2200, 0x2200, 0x2200, 0x2200,
    0x2200, 0x2000, 0x2100, 0x2200,
];

/// The seven Table 4.A.44–4.A.50 `cband_si` models, indexed by the
/// Table 4.A.31 `other_model` column.
pub const CBAND_SI_MODELS: [&[u16]; 7] = [
    &CBAND_SI_MODEL_0,
    &CBAND_SI_MODEL_1,
    &CBAND_SI_MODEL_2,
    &CBAND_SI_MODEL_3,
    &CBAND_SI_MODEL_4,
    &CBAND_SI_MODEL_5,
    &CBAND_SI_MODEL_6,
];

/// The Table 4.A.37–4.A.43 scalefactor models, indexed by
/// `scf_model` (Table 4.A.32; model 0 has no table).
pub const SCF_MODELS: [Option<&[u16]>; 8] = [
    None,
    Some(&SCF_MODEL_1),
    Some(&SCF_MODEL_2),
    Some(&SCF_MODEL_3),
    Some(&SCF_MODEL_4),
    Some(&SCF_MODEL_5),
    Some(&SCF_MODEL_6),
    Some(&SCF_MODEL_7),
];

/// Table 4.A.34 — position of the probability value inside a
/// zero-higher-bits row, from the neighbour context.
///
/// * `a = i % 4` — the line's offset in its aligned 4-line group.
/// * `b`, `c`, `d` — the current-plane sliced bits already decoded
///   for lines `i-3`, `i-2`, `i-1` (only the in-group ones apply:
///   `d` from `a >= 1`, `c` from `a >= 2`, `b` from `a >= 3`).
/// * `e`, `f`, `g`, `h` — whether the higher bits of lines
///   `i-a+3`, `i-a+2`, `i-a+1`, `i-a` are non-zero. Flags of lines
///   at or after `i` are 0 by construction (their higher bits for
///   the *current* plane are what is being decoded), which is
///   exactly how the table's absent cells are shaped.
///
/// Returns the row position `0..=64`.
pub fn context_position(a: usize, prev_bits: [u8; 3], group_higher_nonzero: [u8; 4]) -> usize {
    debug_assert!(a < 4);
    // `prev_bits = [b, c, d]` — the current-plane bits of lines
    // i-3, i-2, i-1; `group_higher_nonzero = [h, g, f, e]` — the
    // higher-bits-non-zero flags of the aligned group lines
    // i-a .. i-a+3 in line order.
    let [b, c, d] = prev_bits;
    let [h, g, f, e] = group_higher_nonzero;
    // Column index within the printed table: (h, g, f, e) walked as
    // a 4-bit number h·8 + g·4 + f·2 + e.
    let col =
        (usize::from(h) << 3) | (usize::from(g) << 2) | (usize::from(f) << 1) | usize::from(e);
    match a {
        0 => {
            // h refers to line i itself: always 0 here. 8 columns.
            const ROW: [usize; 8] = [0, 15, 22, 29, 32, 39, 42, 45];
            ROW[col & 7]
        }
        1 => {
            // g refers to line i: 0. Columns h∈{0,1} × f,e.
            const ROW_D0: [[usize; 4]; 2] = [[1, 16, 23, 30], [46, 53, 56, 59]];
            const ROW_D1: [[usize; 4]; 2] = [[2, 17, 24, 31], [46, 53, 56, 59]];
            let h_i = usize::from(h);
            let fe = col & 3;
            if d == 0 {
                ROW_D0[h_i][fe]
            } else {
                ROW_D1[h_i][fe]
            }
        }
        2 => {
            // f refers to line i: 0. Columns (h, g) × e.
            // Row selected by (c, d).
            const ROWS: [[usize; 8]; 4] = [
                // (h,g,e) order: 000,001,010,011,100,101,110,111
                [3, 18, 33, 40, 47, 54, 60, 63], // c=0, d=0
                [4, 19, 33, 40, 48, 55, 60, 63], // c=0, d=1
                [5, 20, 34, 41, 47, 54, 60, 63], // c=1, d=0
                [6, 21, 34, 41, 48, 55, 60, 63], // c=1, d=1
            ];
            let row = ((c as usize) << 1) | d as usize;
            let hge = ((usize::from(h)) << 2) | ((usize::from(g)) << 1) | usize::from(e);
            ROWS[row][hge]
        }
        _ => {
            // a == 3: e refers to line i: 0. Columns (h, g, f).
            // Row selected by (b, c, d).
            const ROWS: [[usize; 8]; 8] = [
                [7, 25, 35, 43, 49, 57, 61, 64],  // 000
                [8, 25, 36, 43, 50, 57, 62, 64],  // 001
                [9, 26, 35, 43, 51, 58, 61, 64],  // 010
                [10, 26, 36, 43, 52, 58, 62, 64], // 011
                [11, 27, 37, 44, 49, 57, 61, 64], // 100
                [12, 27, 38, 44, 50, 57, 62, 64], // 101
                [13, 28, 37, 44, 51, 58, 61, 64], // 110
                [14, 28, 38, 44, 52, 58, 62, 64], // 111
            ];
            let row = ((b as usize) << 2) | ((c as usize) << 1) | d as usize;
            let hgf = ((usize::from(h)) << 2) | ((usize::from(g)) << 1) | usize::from(f);
            ROWS[row][hgf]
        }
    }
}

/// One probability table's explicit rows (base tables 1..=10; the
/// aliased tables 11..=22 resolve onto 9 / 10 in [`spectral_p0`]).
struct ProbTable {
    /// The MSB-plane row (15 positions — higher-bit flags are all
    /// zero at the MSB by construction).
    msb: &'static [u16],
    /// Zero-higher-bits rows for `rel = 1..` (65 positions each).
    /// Tables 9 / 10 leave this empty and alias tables 7 / 8.
    zero: &'static [&'static [u16]],
    /// Non-zero-higher-bits rows for `rel = 1..` (sizes
    /// `min(2^rel - 1, 16)`).
    nz: &'static [&'static [u16]],
}

const PROB_TABLES: [ProbTable; 10] = [
    ProbTable {
        msb: &PROB_T1_MSB,
        zero: &[],
        nz: &[],
    },
    ProbTable {
        msb: &PROB_T2_MSB,
        zero: &[],
        nz: &[],
    },
    ProbTable {
        msb: &PROB_T3_MSB,
        zero: &[&PROB_T3_ZERO_1],
        nz: &[&PROB_T3_NZ_1],
    },
    ProbTable {
        msb: &PROB_T4_MSB,
        zero: &[&PROB_T4_ZERO_1],
        nz: &[&PROB_T4_NZ_1],
    },
    ProbTable {
        msb: &PROB_T5_MSB,
        zero: &[&PROB_T5_ZERO_1, &PROB_T5_ZERO_2],
        nz: &[&PROB_T5_NZ_1, &PROB_T5_NZ_2],
    },
    ProbTable {
        msb: &PROB_T6_MSB,
        zero: &[&PROB_T6_ZERO_1, &PROB_T6_ZERO_2],
        nz: &[&PROB_T6_NZ_1, &PROB_T6_NZ_2],
    },
    ProbTable {
        msb: &PROB_T7_MSB,
        zero: &[&PROB_T7_ZERO_1, &PROB_T7_ZERO_2, &PROB_T7_ZERO_3],
        nz: &[&PROB_T7_NZ_1, &PROB_T7_NZ_2, &PROB_T7_NZ_3],
    },
    ProbTable {
        msb: &PROB_T8_MSB,
        zero: &[&PROB_T8_ZERO_1, &PROB_T8_ZERO_2, &PROB_T8_ZERO_3],
        nz: &[&PROB_T8_NZ_1, &PROB_T8_NZ_2, &PROB_T8_NZ_3],
    },
    ProbTable {
        msb: &PROB_T9_MSB,
        zero: &[],
        nz: &[&PROB_T9_NZ_1, &PROB_T9_NZ_2, &PROB_T9_NZ_3, &PROB_T9_NZ_4],
    },
    ProbTable {
        msb: &PROB_T10_MSB,
        zero: &[],
        nz: &[
            &PROB_T10_NZ_1,
            &PROB_T10_NZ_2,
            &PROB_T10_NZ_3,
            &PROB_T10_NZ_4,
        ],
    },
];

/// Resolve a `cband_si` (1..=22) to `(base probability table 1..=10,
/// MSB plane)` per Table 4.A.33 and the Table 4.A.66–4.A.77 alias
/// notes ("Same as BSAC probability Table 9/10, but MSB plane = M").
fn resolve_table(cband_si: u8) -> (usize, u8) {
    debug_assert!((1..=22).contains(&cband_si));
    let plane = CBAND_SI_MSB_PLANE[cband_si as usize];
    // 2009 alias scheme. NOTE: the 2001 edition prints a different
    // scheme (tables 11..=22 all onto table 10, and the sub-MSB
    // zero rows of 9/10 onto table 8) — both readings were tested
    // against the 14496-26 conformance streams and neither matches
    // the deployed encoder's selection; see the crate README's
    // BSAC divergence note.
    let base = match cband_si {
        1..=10 => cband_si,
        11 | 13 => 9,
        12 | 14 => 10,
        _ => 9, // tables 15..=22 alias table 9 at planes 8..=15
    };
    (base as usize, plane)
}

/// The spectral bit-slice `p0` — the probability of the "0" symbol
/// for one sliced bit, per §4.6.4.2.3.
///
/// * `cband_si` — the coding band's side info (1..=22; 0 never
///   decodes spectral bits).
/// * `snf` — the significance (bit plane, 1-based) being decoded.
/// * `hbv` — the line's own decoded higher bits (the
///   `higher_bit_vector`, bits above `snf` as an integer).
/// * `pos` — the Table 4.A.34 context position (only consulted when
///   `hbv == 0`).
pub fn spectral_p0(cband_si: u8, snf: u8, hbv: u32, pos: usize) -> u16 {
    let (base, plane) = resolve_table(cband_si);
    let t = &PROB_TABLES[base - 1];
    debug_assert!(snf >= 1 && snf <= plane);
    let rel = usize::from(plane - snf);
    if hbv != 0 {
        // Non-zero decoded higher bits: index by min(hbv, 16) - 1.
        let rows = if t.nz.is_empty() { &[] } else { t.nz };
        let row = rows[rel.min(rows.len()) - 1];
        let idx = (hbv.min(16) as usize - 1).min(row.len() - 1);
        row[idx]
    } else if rel == 0 {
        t.msb[pos.min(t.msb.len() - 1)]
    } else {
        // Zero rows: tables 9 / 10 (and the 11..=22 aliases on
        // them) borrow tables 7 / 8 for the sub-MSB rows.
        let (rows, cap) = if t.zero.is_empty() {
            let borrowed = if base == 9 {
                &PROB_TABLES[6]
            } else {
                &PROB_TABLES[7]
            };
            (borrowed.zero, borrowed.zero.len())
        } else {
            (t.zero, t.zero.len())
        };
        rows[rel.min(cap) - 1][pos]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cumulative_frequency_models_are_well_formed() {
        let mut all: Vec<&[u16]> = vec![
            &MS_USED_MODEL,
            &STEREO_INFO_MODEL,
            &NOISE_FLAG_MODEL,
            &NOISE_MODE_MODEL,
            &CBAND_SI_MODEL_CBAND0,
        ];
        all.extend(CBAND_SI_MODELS.iter().copied());
        all.extend(SCF_MODELS.iter().flatten().copied());
        for model in all {
            assert!(model[0] < 0x4000, "cum freq must sit under 2^14");
            assert!(
                model.windows(2).all(|w| w[0] > w[1]),
                "cum freqs strictly decreasing"
            );
            assert_eq!(*model.last().unwrap(), 0, "last cum freq is 0");
        }
    }

    #[test]
    fn model_sizes_cover_their_largest_symbols() {
        for (i, p) in CBAND_SI_TYPES.iter().enumerate() {
            assert!(
                CBAND_SI_MODELS[p.other_model as usize].len() > p.largest_other as usize,
                "type {i}: other model too small"
            );
            assert!(
                CBAND_SI_MODEL_CBAND0.len() > p.largest_cband0 as usize,
                "type {i}: cband0 model too small"
            );
        }
        for (m, largest) in SCF_MODEL_LARGEST.iter().enumerate() {
            if let Some(model) = SCF_MODELS[m] {
                assert_eq!(
                    model.len(),
                    usize::from(*largest) + 1,
                    "scf model {m} size vs Table 4.A.32 largest"
                );
            }
        }
    }

    /// Every Table 4.A.34 position 0..=64 is reachable, and every
    /// reachable context yields a position <= 64.
    #[test]
    fn context_positions_cover_the_table() {
        let mut seen = [false; 65];
        for a in 0..4usize {
            for bits in 0..8u8 {
                let (b, c, d) = ((bits >> 2) & 1, (bits >> 1) & 1, bits & 1);
                // Only the in-group predecessors apply; zero the rest
                // like the decoder does.
                let (b, c, d) = match a {
                    0 => (0, 0, 0),
                    1 => (0, 0, d),
                    2 => (0, c, d),
                    _ => (b, c, d),
                };
                for flags in 0..16u8 {
                    let (h, g, f, _e) = (
                        (flags >> 3) & 1,
                        (flags >> 2) & 1,
                        (flags >> 1) & 1,
                        flags & 1,
                    );
                    // Flags at or after line i are structurally 0
                    // (the last group line's flag e never survives
                    // the mask below).
                    let (h, g, f, e) = match a {
                        0 => (0, 0, 0, 0),
                        1 => (h, 0, 0, 0),
                        2 => (h, g, 0, 0),
                        _ => (h, g, f, 0),
                    };
                    let pos = context_position(a, [b, c, d], [h, g, f, e]);
                    assert!(pos <= 64);
                    seen[pos] = true;
                }
            }
        }
        // The e..h flags of *later* in-group lines can be non-zero
        // too (their hbv from earlier planes) — walk the full flag
        // space for coverage.
        for a in 0..4usize {
            for bits in 0..8u8 {
                let (b, c, d) = ((bits >> 2) & 1, (bits >> 1) & 1, bits & 1);
                for flags in 0..16u8 {
                    let (h, g, f, e) = (
                        (flags >> 3) & 1,
                        (flags >> 2) & 1,
                        (flags >> 1) & 1,
                        flags & 1,
                    );
                    let pos = context_position(a, [b, c, d], [h, g, f, e]);
                    assert!(pos <= 64);
                    seen[pos] = true;
                }
            }
        }
        assert!(seen.iter().all(|&s| s), "all 65 positions reachable");
    }

    /// [`spectral_p0`] resolves every `(cband_si, snf, hbv, pos)`
    /// combination without panicking, always inside (0, 2^14).
    #[test]
    fn spectral_p0_covers_every_context() {
        for cband_si in 1u8..=22 {
            let plane = CBAND_SI_MSB_PLANE[cband_si as usize];
            for snf in 1..=plane {
                let rel = plane - snf;
                let max_hbv: u32 = if rel >= 31 { u32::MAX } else { (1 << rel) - 1 };
                for hbv in 0..=max_hbv.min(40) {
                    let poss: &[usize] = if rel == 0 { &[0, 7, 14] } else { &[0, 32, 64] };
                    for &pos in poss {
                        let p0 = spectral_p0(cband_si, snf, hbv, pos);
                        assert!(
                            p0 > 0 && p0 < 0x4000,
                            "cband_si {cband_si} snf {snf} hbv {hbv} pos {pos}: {p0:#x}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn p0_clamps_are_consistent() {
        for len in 1..14usize {
            assert!(MIN_P0[len] <= MAX_P0[len]);
        }
        assert_eq!(clamp_p0(0x3fff, 1), 0x2000);
        assert_eq!(clamp_p0(0x0001, 1), 0x2000);
        assert_eq!(clamp_p0(0x1234, 14), 0x1234);
    }

    /// Spot-check transcription anchors against the printed spec
    /// listings.
    #[test]
    fn transcription_anchors() {
        assert_eq!(CBAND_SI_MODEL_0[0], 0x3ef6); // Table 4.A.44
        assert_eq!(CBAND_SI_MODEL_6[0], 0x31af); // Table 4.A.50
        assert_eq!(CBAND_SI_MODEL_CBAND0[0], 0x3ff8); // Table 4.A.51
        assert_eq!(MS_USED_MODEL[0], 0x2ccd); // Table 4.A.52
        assert_eq!(STEREO_INFO_MODEL, [0x3666, 0x1000, 0x0666, 0]); // 4.A.53
        assert_eq!(NOISE_FLAG_MODEL[0], 0x2000); // Table 4.A.54
        assert_eq!(SCF_MODEL_7[0], 0x3b5e); // Table 4.A.43
        assert_eq!(SCF_MODEL_7[63], 0);
        assert_eq!(PROB_T1_MSB[0], 0x3900); // Table 4.A.56
        assert_eq!(PROB_T1_MSB[14], 0x2c00);
        assert_eq!(PROB_T7_MSB[0], 0x3d00); // Table 4.A.62
        assert_eq!(PROB_T7_NZ_1[0], 0x2f00); // the 2F00 uppercase cell
        assert_eq!(PROB_T9_NZ_4[15], 0x2000); // Table 4.A.64 last cell
        assert_eq!(PROB_T10_NZ_4[15], 0x2200); // Table 4.A.65 last cell
    }
}

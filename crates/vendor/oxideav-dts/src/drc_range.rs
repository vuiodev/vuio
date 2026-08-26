//! DTS Dynamic Range Control: the signed-Q2 `dts_dynrng_to_db()`
//! code→dB mapping (§5.4.1 / §5.7.2) plus the §D.4 "Dynamic Range
//! Control" presentation table.
//!
//! ## The wire format: an 8-bit signed Q2 code (round 408)
//!
//! Per §5.4.1 (`RANGE` field description, recovered as a closed-form
//! function in the freshly staged `docs/audio/dts/dts-drc-dynrng.md`):
//!
//! > "Each coefficient is **8-bit signed fractional Q2 binary** and
//! > represents a logarithmic gain value as shown in clause D.4
//! > giving a range of **±31,75 dB in steps of 0,25 dB**. Dynamic
//! > range compression is affected by multiplying the decoded audio
//! > samples by the linear coefficient."
//!
//! So the byte the bitstream carries (`ExtractBits(8)` in the §5.4.1
//! `DYNF != 0` tail, and each `subsubFrameDRC_Rev2AUX[]` byte of the
//! §5.7.2 Rev2AUX chunk) is a **two's-complement signed Q2** value:
//!
//! ```text
//! dB     = (int8_t)code × 0.25          // dts_dynrng_to_db()
//! linear = 10^(dB / 20)                 // multiplies each sample
//! ```
//!
//! Code `0` is therefore **unity** (0 dB), and the applied gain is
//! `10^(dB/20)`, post-QMF ([`dts_dynrng_to_db`] /
//! [`dts_dynrng_to_linear`]).
//!
//! ## The §D.4 table is an offset-binary *presentation* — do not
//! index it with the raw code
//!
//! Annex D §D.4 (PDF p.195-197) prints the same mapping as a 256-row
//! `Index | Q18 binary | Multiplier | Log Multiplier (dB)` table,
//! where `dB(Index) = (Index − 127) × 0.25` — i.e. the printed
//! `Index` column is **offset-binary** (`Index = signed_code + 127`;
//! Index `127` = code `0` = 0 dB). [`DRC_RANGE_MULTIPLIER`] preserves
//! the printed `Multiplier` column keyed by that printed Index.
//! Indexing it directly with a raw wire code (`table[code]`) is off
//! by 127 steps — code `0` would wrongly yield −31.75 dB — which is
//! exactly the correctness trap `docs/audio/dts/dts-drc-dynrng.md`
//! documents ("Why the §D.4 table was reverted"). Decoders must use
//! the signed-Q2 function; the table stays available as the §D.4
//! reference data and is cross-checked against the function in the
//! tests below.
//!
//! §5.4.1 Table 5-28 application pseudocode (the `RANGE` multiply
//! runs **after** the §C.2.5 QMF synthesis):
//!
//! ```text
//! if ( DYNF != 0 ) {
//!   nIndex = ExtractBits(8);
//!   RANGEtbl.LookUp(nIndex, RANGE);
//!   for (ch=0; ch<nPCHS; ch++)
//!     for (n=0; n<nNumSamples; n++)
//!       AudioCh[ch].ReconstructedSamples[n] *= RANGE;
//! }
//! ```
//!
//! This module is feature-independent (no `oxideav-core` dep), so it
//! is available under both the default and `--no-default-features`
//! build modes.

/// The §5.4.1 / §5.7.2 `dts_dynrng_to_db()` function: interpret the
/// 8-bit DRC code from the bitstream as **two's-complement signed Q2**
/// and return its dB gain (`0.25` dB per LSB).
///
/// Per `docs/audio/dts/dts-drc-dynrng.md`: input spans the full byte
/// (`0x00..=0xFF` → signed `−128..=+127`), output spans
/// `[−32.00 dB, +31.75 dB]` in 0.25 dB steps (the spec's nominal
/// usable range is ±31.75 dB; the extreme `0x80` = −32.0 dB sits just
/// outside it). Code `0` is unity (0 dB).
#[must_use]
pub fn dts_dynrng_to_db(code: u8) -> f64 {
    // Sign-extend: two's complement, then Q2 -> 0.25 dB / LSB.
    f64::from(code as i8) * 0.25
}

/// The linear gain a §5.4.1 `RANGE` / §5.7.2 Rev2AUX DRC code applies
/// to every reconstructed PCM sample: `10^(dts_dynrng_to_db(code)/20)`
/// (§5.4.1: "Dynamic range compression is affected by multiplying the
/// decoded audio samples by the linear coefficient").
///
/// Code `0` returns exactly `1.0`.
#[must_use]
pub fn dts_dynrng_to_linear(code: u8) -> f64 {
    if code == 0 {
        return 1.0;
    }
    10f64.powf(dts_dynrng_to_db(code) / 20.0)
}

/// Number of rows in the printed §D.4 table (`Index` column `0..=255`).
pub const DRC_RANGE_LEN: usize = 256;

/// The §D.4 unity-gain **printed Index** (`RANGE == 1.0000`,
/// `0.0000` dB). Note this is the offset-binary presentation index,
/// not a wire code — the wire code for unity is `0`
/// ([`dts_dynrng_to_linear`]).
pub const DRC_RANGE_UNITY_INDEX: usize = 127;

/// §D.4 Dynamic Range Control multiplier table, keyed by the table's
/// **printed offset-binary `Index` column** (`Index = signed_code +
/// 127`; row 127 = 0 dB). Row `i` is `10^((i − 127)·0.25 / 20)` to the
/// spec's 4-decimal rounding.
///
/// **Do not index this with the raw 8-bit wire code** — the §5.4.1 /
/// §5.7.2 DRC byte is two's-complement signed Q2 and must go through
/// [`dts_dynrng_to_db`] / [`dts_dynrng_to_linear`] (see the module
/// docs and `docs/audio/dts/dts-drc-dynrng.md`).
///
/// Transcribed from ETSI TS 102 114 V1.3.1 §D.4, "Multiplier" column,
/// indices `0..=255`.
pub static DRC_RANGE_MULTIPLIER: [f64; DRC_RANGE_LEN] = [
    0.0259, 0.0266, 0.0274, 0.0282, 0.029, 0.0299, 0.0307, 0.0316, 0.0325, 0.0335, 0.0345, 0.0355,
    0.0365, 0.0376, 0.0387, 0.0398, 0.041, 0.0422, 0.0434, 0.0447, 0.046, 0.0473, 0.0487, 0.0501,
    0.0516, 0.0531, 0.0546, 0.0562, 0.0579, 0.0596, 0.0613, 0.0631, 0.0649, 0.0668, 0.0688, 0.0708,
    0.0729, 0.075, 0.0772, 0.0794, 0.0818, 0.0841, 0.0866, 0.0891, 0.0917, 0.0944, 0.0972, 0.1,
    0.1029, 0.1059, 0.109, 0.1122, 0.1155, 0.1189, 0.1223, 0.1259, 0.1296, 0.1334, 0.1372, 0.1413,
    0.1454, 0.1496, 0.154, 0.1585, 0.1631, 0.1679, 0.1728, 0.1778, 0.183, 0.1884, 0.1939, 0.1995,
    0.2054, 0.2113, 0.2175, 0.2239, 0.2304, 0.2371, 0.2441, 0.2512, 0.2585, 0.2661, 0.2738, 0.2818,
    0.2901, 0.2985, 0.3073, 0.3162, 0.3255, 0.335, 0.3447, 0.3548, 0.3652, 0.3758, 0.3868, 0.3981,
    0.4097, 0.4217, 0.434, 0.4467, 0.4597, 0.4732, 0.487, 0.5012, 0.5158, 0.5309, 0.5464, 0.5623,
    0.5788, 0.5957, 0.6131, 0.631, 0.6494, 0.6683, 0.6879, 0.7079, 0.7286, 0.7499, 0.7718, 0.7943,
    0.8175, 0.8414, 0.866, 0.8913, 0.9173, 0.9441, 0.9716, 1.0, 1.0292, 1.0593, 1.0902, 1.122,
    1.1548, 1.1885, 1.2232, 1.2589, 1.2957, 1.3335, 1.3725, 1.4125, 1.4538, 1.4962, 1.5399, 1.5849,
    1.6312, 1.6788, 1.7278, 1.7783, 1.8302, 1.8836, 1.9387, 1.9953, 2.0535, 2.1135, 2.1752, 2.2387,
    2.3041, 2.3714, 2.4406, 2.5119, 2.5852, 2.6607, 2.7384, 2.8184, 2.9007, 2.9854, 3.0726, 3.1623,
    3.2546, 3.3497, 3.4475, 3.5481, 3.6517, 3.7584, 3.8681, 3.9811, 4.0973, 4.217, 4.3401, 4.4668,
    4.5973, 4.7315, 4.8697, 5.0119, 5.1582, 5.3088, 5.4639, 5.6234, 5.7876, 5.9566, 6.1306, 6.3096,
    6.4938, 6.6834, 6.8786, 7.0795, 7.2862, 7.4989, 7.7179, 7.9433, 8.1752, 8.414, 8.6596, 8.9125,
    9.1728, 9.4406, 9.7163, 10.0, 10.292, 10.5925, 10.9018, 11.2202, 11.5478, 11.885, 12.2321,
    12.5893, 12.9569, 13.3352, 13.7246, 14.1254, 14.5378, 14.9624, 15.3993, 15.8489, 16.3117,
    16.788, 17.2783, 17.7828, 18.3021, 18.8365, 19.3865, 19.9526, 20.5353, 21.1349, 21.752,
    22.3872, 23.0409, 23.7137, 24.4062, 25.1189, 25.8523, 26.6073, 27.3842, 28.1838, 29.0068,
    29.8538, 30.7256, 31.6228, 32.5462, 33.4965, 34.4747, 35.4813, 36.5174, 37.5837, 38.6812,
    39.8107,
];

/// Look up a row of the printed §D.4 table by its **offset-binary
/// `Index` column** (`Index = signed_code + 127`).
///
/// This is a reference-data accessor for the table as printed, *not*
/// the wire-code resolution: feeding the raw §5.4.1 / §5.7.2 DRC byte
/// here is off by 127 steps (code `0` would wrongly yield −31.75 dB).
/// Decode wire codes with [`dts_dynrng_to_linear`] /
/// [`dts_dynrng_to_db`] instead (`docs/audio/dts/dts-drc-dynrng.md`,
/// "Why the §D.4 table was reverted").
#[must_use]
pub fn drc_range(index: u8) -> f64 {
    DRC_RANGE_MULTIPLIER[index as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_full_8bit_range() {
        assert_eq!(DRC_RANGE_MULTIPLIER.len(), 256);
        assert_eq!(DRC_RANGE_LEN, 256);
    }

    #[test]
    fn unity_at_index_127() {
        // §D.4: index 127 -> Multiplier 1.0000 (0.0000 dB).
        assert_eq!(drc_range(127), 1.0);
        assert_eq!(DRC_RANGE_UNITY_INDEX, 127);
    }

    #[test]
    fn anchor_rows_match_spec() {
        // Verbatim §D.4 anchor values from the staged PDF.
        assert_eq!(drc_range(0), 0.0259); //  -31.75 dB
        assert_eq!(drc_range(47), 0.1); //    -20.00 dB
        assert_eq!(drc_range(80), 0.2585); // -11.75 dB
        assert_eq!(drc_range(127), 1.0); //     0.00 dB
        assert_eq!(drc_range(128), 1.0292); //  0.25 dB
        assert_eq!(drc_range(207), 10.0); //   20.00 dB
        assert_eq!(drc_range(255), 39.8107); // 32.00 dB
    }

    #[test]
    fn table_is_strictly_monotone_increasing() {
        // The §D.4 multiplier rises monotonically with the index (the
        // dB column is an exact 0.25 dB ramp), so every successor is
        // strictly larger.
        for i in 1..DRC_RANGE_LEN {
            assert!(
                DRC_RANGE_MULTIPLIER[i] > DRC_RANGE_MULTIPLIER[i - 1],
                "entry {i} not greater than predecessor"
            );
        }
    }

    // -----------------------------------------------------------
    // dts_dynrng_to_db / dts_dynrng_to_linear (signed Q2, round 408)
    // -----------------------------------------------------------

    /// The signed-Q2 anchors from `docs/audio/dts/dts-drc-dynrng.md`:
    /// code 0 = 0 dB (unity), 0.25 dB per LSB in both directions, and
    /// the two's-complement extremes.
    #[test]
    fn dynrng_signed_q2_anchors() {
        assert_eq!(dts_dynrng_to_db(0), 0.0);
        assert_eq!(dts_dynrng_to_linear(0), 1.0);
        assert_eq!(dts_dynrng_to_db(1), 0.25);
        assert_eq!(dts_dynrng_to_db(0xFF), -0.25); // signed -1
        assert_eq!(dts_dynrng_to_db(0x7F), 31.75); // +127
        assert_eq!(dts_dynrng_to_db(0x80), -32.0); // -128 (outside nominal)
        assert_eq!(dts_dynrng_to_db(0x81), -31.75); // -127
                                                    // +20 dB = 80 quarter-dB steps -> linear 10.0.
        assert_eq!(dts_dynrng_to_db(80), 20.0);
        assert!((dts_dynrng_to_linear(80) - 10.0).abs() < 1e-12);
        // -20 dB -> linear 0.1.
        let minus_20 = 0u8.wrapping_sub(80);
        assert_eq!(dts_dynrng_to_db(minus_20), -20.0);
        assert!((dts_dynrng_to_linear(minus_20) - 0.1).abs() < 1e-12);
    }

    /// The closed-form function and the printed §D.4 table describe
    /// the same mapping, related by `Index = signed_code + 127`: for
    /// every signed code in the table's domain (−127..=+127, i.e.
    /// printed Index 0..=254) the function's linear gain matches the
    /// table row to the spec's 4-decimal rounding.
    #[test]
    fn dynrng_function_matches_d4_table_via_offset_binary_index() {
        for signed in -127i32..=127 {
            let code = signed as i8 as u8;
            let index = (signed + 127) as usize;
            let from_fn = dts_dynrng_to_linear(code);
            let from_table = DRC_RANGE_MULTIPLIER[index];
            // The printed Multiplier column is rounded to 4 decimals,
            // so the absolute disagreement is bounded by half an ULP
            // of that rounding.
            assert!(
                (from_fn - from_table).abs() < 6e-5,
                "code {signed}: fn {from_fn} vs table[{index}] {from_table}"
            );
        }
        // The extremes that do NOT correspond: table row 255 is
        // +32 dB (no reachable signed code maps there via the offset),
        // and code -128 (-32 dB) has no table row.
        assert_eq!(drc_range(255), 39.8107);
        assert!((dts_dynrng_to_linear(0x80) - 10f64.powf(-32.0 / 20.0)).abs() < 1e-12);
    }

    /// Demonstrate the documented off-by-127 trap: raw-code indexing
    /// of the §D.4 table disagrees with the signed-Q2 function at
    /// code 0 (the most common wire value).
    #[test]
    fn raw_code_table_indexing_is_the_documented_trap() {
        assert_eq!(drc_range(0), 0.0259); // table row 0 = -31.75 dB
        assert_eq!(dts_dynrng_to_linear(0), 1.0); // wire code 0 = 0 dB
    }

    #[test]
    fn multiplier_tracks_log_db_column() {
        // Cross-check the transcribed Multiplier column against the
        // informative Log-Multiplier(dB) column: dB[i] = -31.75 + 0.25*i,
        // and Multiplier ≈ 10^(dB/20) to within the spec's 4-decimal
        // rounding.
        for (i, &actual) in DRC_RANGE_MULTIPLIER.iter().enumerate() {
            let db = -31.75 + 0.25 * i as f64;
            let predicted = 10f64.powf(db / 20.0);
            let rel = (predicted - actual).abs() / actual;
            assert!(
                rel < 0.01,
                "index {i}: rel err {rel} (pred {predicted}, got {actual})"
            );
        }
    }
}

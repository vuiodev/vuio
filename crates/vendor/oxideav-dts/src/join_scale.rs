//! §D.3 Scale Factor table for Joint Intensity Coding (`JScaleTbl`),
//! the look-up the DTS Core §5.4.1 side-information `JOIN_SCALES` walk
//! feeds when a channel enables joint-intensity coding (`JOINX[ch] > 0`).
//!
//! Transcribed verbatim from ETSI TS 102 114 V1.3.1 (2011-08) Annex D
//! §D.3 "Scale Factor for Joint Intensity Coding" (staged at
//! `docs/audio/dts/etsi-ts-102114-dts-coherent-acoustics.pdf`, PDF
//! p.195). The printed table is laid out as four index/value groups per
//! row (indices `0..=31`, `32..=63`, `64..=95`, `96..=128`); this module
//! preserves the single `Scale Factor` column, re-indexed row-major over
//! the full `0..=128` range.
//!
//! Per the §5.4.1 Table 5-28 pseudocode the `JOIN_SCALES` walk decodes
//! one `QSCALES` quantization index per joint sub-band, biases it by 64,
//! and looks the biased index up in this table:
//!
//! ```text
//! nQSelect = JOIN_SHUFF[ch];              // 3-bit code-book selector
//! for (n = nSUBS[ch]; n < nSUBS[nSourceCh]; n++) {
//!     QSCALES.ppQ[nQSelect]->InverseQ(InputFrame, nJScale);
//!     nJScale = nJScale + 64;             // bias
//!     JScaleTbl.LookUp(nJScale, JOIN_SCALES[ch][n]);
//! }
//! ```
//!
//! The resulting `JOIN_SCALES[ch][n]` scalar multiplies the sub-band
//! samples copied from the source channel (`JOINX[ch] - 1`) to the
//! current channel during the §C.2.3 joint-subband reconstruction.
//!
//! Index `64` maps to the unity scale factor `1.0`, confirming the bias:
//! a `QSCALES` quantization index of `0` (the differential zero) resolves
//! to `1.0`, i.e. "copy the source sub-band unchanged".
//!
//! This module is feature-independent (no `oxideav-core` dep), so it is
//! available under both the default and `--no-default-features` builds.

/// Number of entries in the §D.3 joint-intensity scale table —
/// `0..=128`, i.e. 129 entries. The `JOIN_SCALES` walk biases a
/// `QSCALES` index by 64 before indexing, so the reachable index range
/// depends on the code-book symbol range.
pub const JOIN_SCALE_LEN: usize = 129;

/// The §D.3 unity-gain index (`JOIN_SCALES == 1.0`). A `QSCALES` symbol
/// of `0` biased by 64 lands here, meaning "copy the source sub-band
/// with no scaling".
pub const JOIN_SCALE_UNITY_INDEX: usize = 64;

/// §D.3 "Scale Factor for Joint Intensity Coding" table (`JScaleTbl`),
/// indexed by the biased `nJScale` (`InverseQ` symbol + 64). Entry `i`
/// is the linear scale factor applied to a sub-band sample copied from
/// the source channel to a jointly-coded channel (see [`join_scale`]).
///
/// Transcribed from ETSI TS 102 114 V1.3.1 §D.3, "Scale Factor" column,
/// indices `0..=128`.
pub static JOIN_SCALE_FACTOR: [f64; JOIN_SCALE_LEN] = [
    0.025088, 0.026624, 0.02816, 0.029824, 0.031616, 0.033472, 0.035456, 0.037568, 0.039808,
    0.042176, 0.044672, 0.047296, 0.050112, 0.05312, 0.056256, 0.059584, 0.063104, 0.066816,
    0.070784, 0.075008, 0.079424, 0.08416, 0.089152, 0.0944, 0.099968, 0.10592, 0.112192, 0.118848,
    0.125888, 0.133376, 0.141248, 0.149632, 0.158464, 0.167872, 0.177856, 0.188352, 0.199552,
    0.211328, 0.223872, 0.23712, 0.2512, 0.266048, 0.281856, 0.29856, 0.316224, 0.334976, 0.354816,
    0.375808, 0.39808, 0.421696, 0.446656, 0.473152, 0.501184, 0.53088, 0.562368, 0.595648,
    0.630976, 0.668352, 0.707968, 0.749888, 0.794304, 0.841408, 0.891264, 0.944064, 1.0, 1.05926,
    1.12205, 1.18848, 1.25894, 1.3335, 1.41254, 1.49626, 1.5849, 1.67878, 1.7783, 1.88365, 1.99526,
    2.11347, 2.23872, 2.37139, 2.51187, 2.66074, 2.81837, 2.98541, 3.1623, 3.34963, 3.54816,
    3.7584, 3.98106, 4.21696, 4.46682, 4.73152, 5.0119, 5.30886, 5.62342, 5.95661, 6.30957,
    6.68346, 7.07949, 7.49894, 7.9433, 8.41395, 8.91251, 9.44064, 10.0, 10.5925, 11.2202, 11.885,
    12.5892, 13.3352, 14.1254, 14.9624, 15.849, 16.788, 17.7828, 18.8365, 19.9526, 21.1349,
    22.3872, 23.7137, 25.1188, 26.6072, 28.1838, 29.8538, 31.6228, 33.4965, 35.4813, 37.5837,
    39.8107,
];

/// Look up the §D.3 joint-intensity scale factor for a biased index
/// `nJScale` (the `QSCALES` `InverseQ` symbol plus the fixed `+64`
/// bias), returning the linear scale factor `JOIN_SCALES[ch][n]`.
///
/// Returns `None` when `n_j_scale` falls outside `0..=128` — a
/// well-formed stream keeps the biased index inside the table by
/// construction, so an out-of-range index signals a corrupt or
/// misaligned bit stream rather than a silent clamp.
#[must_use]
pub fn join_scale(n_j_scale: i32) -> Option<f64> {
    if !(0..JOIN_SCALE_LEN as i32).contains(&n_j_scale) {
        return None;
    }
    Some(JOIN_SCALE_FACTOR[n_j_scale as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_129_entries() {
        assert_eq!(JOIN_SCALE_FACTOR.len(), 129);
        assert_eq!(JOIN_SCALE_LEN, 129);
    }

    #[test]
    fn unity_at_index_64() {
        // §D.3: index 64 -> Scale Factor 1.0. The +64 bias means a
        // QSCALES symbol of 0 resolves to unity ("copy unchanged").
        assert_eq!(join_scale(64), Some(1.0));
        assert_eq!(JOIN_SCALE_UNITY_INDEX, 64);
    }

    #[test]
    fn anchor_rows_match_spec() {
        // Verbatim §D.3 anchor values from the staged PDF.
        assert_eq!(join_scale(0), Some(0.025088));
        assert_eq!(join_scale(32), Some(0.158464));
        assert_eq!(join_scale(64), Some(1.0));
        assert_eq!(join_scale(96), Some(6.30957));
        assert_eq!(join_scale(104), Some(10.0));
        assert_eq!(join_scale(128), Some(39.8107));
    }

    #[test]
    fn out_of_range_returns_none() {
        assert_eq!(join_scale(-1), None);
        assert_eq!(join_scale(129), None);
        assert_eq!(join_scale(1000), None);
    }

    #[test]
    fn table_is_strictly_monotone_increasing() {
        // The §D.3 scale factor rises monotonically with the index
        // (each successor is a fixed ~+0.5 dB step), so every entry is
        // strictly larger than its predecessor.
        for i in 1..JOIN_SCALE_LEN {
            assert!(
                JOIN_SCALE_FACTOR[i] > JOIN_SCALE_FACTOR[i - 1],
                "entry {i} not greater than predecessor"
            );
        }
    }

    #[test]
    fn scale_tracks_half_db_ramp() {
        // Cross-check the transcribed column against its implied dB
        // ramp: the anchor at index 64 is unity (0 dB) and index 104 is
        // 10x (+20 dB), so the step is 0.5 dB per index. Verify each
        // entry ~= 10^((i-64)*0.5/20) to within the spec's rounding.
        for (i, &actual) in JOIN_SCALE_FACTOR.iter().enumerate() {
            let db = (i as f64 - 64.0) * 0.5;
            let predicted = 10f64.powf(db / 20.0);
            let rel = (predicted - actual).abs() / actual;
            assert!(
                rel < 0.02,
                "index {i}: rel err {rel} (pred {predicted}, got {actual})"
            );
        }
    }
}

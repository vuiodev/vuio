//! Typed selector for the §C.2.5 `QMFInterpolation()` 512-tap FIR
//! coefficient set.
//!
//! `QMFInterpolation()` (ETSI TS 102 114 V1.3.1 Annex C §C.2.5, PDF
//! p.185, per `docs/audio/dts/dts-core-extracts.md` §2.4) opens with
//! a one-bit `FILTS` parameter that selects between two named §D.8
//! coefficient sets, transcribed verbatim from the staged §2.4
//! pseudocode (lines 174-178):
//!
//! ```text
//!     QMFInterpolation(FILTS, int nSUBS) {
//!         // Select filter
//!         if (FILTS==0)        prCoeff = raCoeffLossy;      // Non-perfect
//!         else                 prCoeff = raCoeffLossLess;   // Perfect
//!         …
//!     }
//! ```
//!
//! The two coefficient sets (`raCoeffLossy`, the *non-perfect
//! reconstruction* 512-tap interpolation FIR, and `raCoeffLossLess`,
//! the *perfect reconstruction* 512-tap interpolation FIR) are
//! defined in §D.8 "32-Band Interpolation and LFE Interpolation FIR"
//! (staged PDF p.238-246) and transcribed at the crate
//! root as [`crate::RA_COEFF_LOSSY`] / [`crate::RA_COEFF_LOSSLESS`].
//!
//! This module exposes the §C.2.5 selection step as a typed
//! [`FilterBankSelection`] enum plus a [`FilterBankSelection::from_filts`]
//! resolver that mirrors the spec's `if (FILTS==0) … else …` branch,
//! and [`FilterBankSelection::coefficients`] resolves the selection
//! to the matching §D.8 512-tap table for the §C.2.5 FIR step
//! ([`crate::fir_step`]).
//!
//! ## Relationship to the frame-header `multirate_inter` bit
//!
//! The DTS Core frame header carries a one-bit `MULTIRATE_INTER`
//! field, surfaced as [`crate::DtsFrameHeader::multirate_inter`].
//! Per ETSI TS 102 114 §5.3 (cited in `wiki/DTS.wiki` line 87) the
//! `MULTIRATE_INTER` bit selects between the same two filter modes
//! the §C.2.5 `FILTS` parameter selects, but the precise polarity
//! mapping (`multirate_inter == 0` → `FILTS == 0` or the inverse)
//! is **not** documented in the staged extracts under
//! `docs/audio/dts/` — neither the `dts-core-extracts.md` §1 header
//! tables (which cover RATE / DYNF / TIMEF only) nor the §2.x
//! filterbank extracts (which cover the §C.2.5 / Annex D side) make
//! the polarity claim. Until that mapping is staged, this module
//! does **not** expose a `DtsFrameHeader::filter_bank_selection()`
//! accessor; callers that need the FIR coefficient set from a parsed
//! header must read [`DtsFrameHeader::multirate_inter`] directly,
//! resolve the polarity from their own out-of-band source, and pass
//! the resulting `FILTS` value (`0` for lossy, non-zero for
//! lossless) to [`FilterBankSelection::from_filts`].

/// The two named 512-tap interpolation-FIR coefficient sets
/// referenced by `QMFInterpolation()` per ETSI TS 102 114 V1.3.1
/// Annex C §C.2.5 (staged in `docs/audio/dts/dts-core-extracts.md`
/// §2.4 lines 175-178).
///
/// Each variant names exactly one of the two §D.8 "32-Band
/// Interpolation and LFE Interpolation FIR" coefficient tables
/// (PDF p.238-246), transcribed as [`crate::RA_COEFF_LOSSY`] /
/// [`crate::RA_COEFF_LOSSLESS`] and reachable through
/// [`Self::coefficients`]. The variant names
/// mirror the spec pseudocode's identifiers (`raCoeffLossy` for
/// the non-perfect set, `raCoeffLossLess` for the perfect set)
/// rendered in idiomatic Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FilterBankSelection {
    /// The §C.2.5 `raCoeffLossy` 512-tap **non-perfect**
    /// reconstruction interpolation FIR (§D.8). Selected by
    /// `FILTS == 0` per the §C.2.5 pseudocode's
    /// `if (FILTS==0) prCoeff = raCoeffLossy;` branch.
    NonPerfectReconstruction,
    /// The §C.2.5 `raCoeffLossLess` 512-tap **perfect**
    /// reconstruction interpolation FIR (§D.8). Selected by any
    /// non-zero `FILTS` value per the §C.2.5 pseudocode's
    /// `else prCoeff = raCoeffLossLess;` branch.
    PerfectReconstruction,
}

impl FilterBankSelection {
    /// Resolve a §C.2.5 `FILTS` flag value to the named §D.8
    /// coefficient set it picks, per the pseudocode's
    /// `if (FILTS==0) prCoeff = raCoeffLossy; else prCoeff = raCoeffLossLess;`
    /// branch (`dts-core-extracts.md` §2.4 lines 175-178).
    ///
    /// Per the spec the `FILTS` parameter is one bit (the §C.2.5
    /// pseudocode treats every non-zero value the same — only the
    /// `== 0` branch is distinguished). This resolver therefore
    /// accepts an arbitrary `u8` and groups all non-zero inputs
    /// into the `PerfectReconstruction` variant, matching the
    /// spec's `if (FILTS==0) ... else ...` semantics exactly.
    #[must_use]
    pub fn from_filts(filts: u8) -> Self {
        if filts == 0 {
            FilterBankSelection::NonPerfectReconstruction
        } else {
            FilterBankSelection::PerfectReconstruction
        }
    }

    /// Inverse of [`Self::from_filts`]: the **canonical** `FILTS`
    /// flag value the §C.2.5 pseudocode reads to select this
    /// coefficient set.
    ///
    /// Returns `0` for [`FilterBankSelection::NonPerfectReconstruction`]
    /// (the `FILTS == 0` branch) and `1` for
    /// [`FilterBankSelection::PerfectReconstruction`] (the canonical
    /// "any non-zero value" representative; the spec collapses the
    /// entire non-zero range to the same `else` branch, so `1` is
    /// the smallest equally-valid choice).
    #[must_use]
    pub fn filts(self) -> u8 {
        match self {
            FilterBankSelection::NonPerfectReconstruction => 0,
            FilterBankSelection::PerfectReconstruction => 1,
        }
    }

    /// The §C.2.5 coefficient-table identifier this selection
    /// names, as written in the staged §2.4 pseudocode
    /// (`raCoeffLossy` or `raCoeffLossLess`).
    ///
    /// Returned as a `&'static str` so callers can format spec-
    /// referencing diagnostics without reaching into the enum
    /// variants; the strings match the pseudocode's identifiers
    /// verbatim.
    #[must_use]
    pub fn spec_table_name(self) -> &'static str {
        match self {
            FilterBankSelection::NonPerfectReconstruction => "raCoeffLossy",
            FilterBankSelection::PerfectReconstruction => "raCoeffLossLess",
        }
    }

    /// The §D.8 512-tap coefficient table this selection picks —
    /// the spec's `prCoeff` after the §C.2.5
    /// `if (FILTS==0) prCoeff = raCoeffLossy; else prCoeff = raCoeffLossLess;`
    /// assignment, ready for the FIR step ([`crate::fir_step`]).
    ///
    /// Returns [`crate::RA_COEFF_LOSSY`] (the §D.8 "Non-Perfect
    /// Reconstruction" column) for
    /// [`FilterBankSelection::NonPerfectReconstruction`] and
    /// [`crate::RA_COEFF_LOSSLESS`] (the "Perfect Reconstruction"
    /// column) for [`FilterBankSelection::PerfectReconstruction`],
    /// both transcribed verbatim from the staged PDF p.238-246.
    #[must_use]
    pub fn coefficients(self) -> &'static [f64; crate::fir_coeff::FIR_COEFF_LEN] {
        match self {
            FilterBankSelection::NonPerfectReconstruction => &crate::fir_coeff::RA_COEFF_LOSSY,
            FilterBankSelection::PerfectReconstruction => &crate::fir_coeff::RA_COEFF_LOSSLESS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------
    // from_filts — selection per §C.2.5 pseudocode.
    // -----------------------------------------------------------

    #[test]
    fn from_filts_zero_picks_non_perfect_reconstruction() {
        // Spec line 176: `if (FILTS==0) prCoeff = raCoeffLossy;`
        assert_eq!(
            FilterBankSelection::from_filts(0),
            FilterBankSelection::NonPerfectReconstruction
        );
    }

    #[test]
    fn from_filts_one_picks_perfect_reconstruction() {
        // Spec line 177: `else prCoeff = raCoeffLossLess;`
        assert_eq!(
            FilterBankSelection::from_filts(1),
            FilterBankSelection::PerfectReconstruction
        );
    }

    #[test]
    fn from_filts_treats_every_non_zero_value_identically() {
        // §C.2.5 uses `if (FILTS==0) … else …` with no further
        // discrimination — every non-zero `FILTS` value picks the
        // lossless set. Verify across the full u8 range.
        for filts in 1u16..=255 {
            assert_eq!(
                FilterBankSelection::from_filts(filts as u8),
                FilterBankSelection::PerfectReconstruction,
                "FILTS={filts} should pick PerfectReconstruction per the §C.2.5 else branch"
            );
        }
    }

    // -----------------------------------------------------------
    // filts — canonical inverse.
    // -----------------------------------------------------------

    #[test]
    fn filts_round_trips_non_perfect_reconstruction() {
        let sel = FilterBankSelection::NonPerfectReconstruction;
        assert_eq!(sel.filts(), 0);
        assert_eq!(FilterBankSelection::from_filts(sel.filts()), sel);
    }

    #[test]
    fn filts_round_trips_perfect_reconstruction() {
        let sel = FilterBankSelection::PerfectReconstruction;
        assert_eq!(sel.filts(), 1);
        assert_eq!(FilterBankSelection::from_filts(sel.filts()), sel);
    }

    #[test]
    fn from_filts_after_filts_is_identity_for_canonical_values() {
        // The canonical `filts()` values 0 and 1 are the spec's two
        // distinguishable inputs; round-trip must be the identity.
        for sel in [
            FilterBankSelection::NonPerfectReconstruction,
            FilterBankSelection::PerfectReconstruction,
        ] {
            assert_eq!(FilterBankSelection::from_filts(sel.filts()), sel);
        }
    }

    // -----------------------------------------------------------
    // spec_table_name — pseudocode identifier passthrough.
    // -----------------------------------------------------------

    #[test]
    fn spec_table_name_for_non_perfect_is_ra_coeff_lossy() {
        // Spec line 176: `prCoeff = raCoeffLossy;`
        assert_eq!(
            FilterBankSelection::NonPerfectReconstruction.spec_table_name(),
            "raCoeffLossy"
        );
    }

    #[test]
    fn spec_table_name_for_perfect_is_ra_coeff_loss_less() {
        // Spec line 177: `prCoeff = raCoeffLossLess;`
        assert_eq!(
            FilterBankSelection::PerfectReconstruction.spec_table_name(),
            "raCoeffLossLess"
        );
    }

    #[test]
    fn spec_table_names_are_distinct() {
        // Sanity: the two §C.2.5 identifiers are not aliases for
        // the same string — they refer to two different §D.8
        // coefficient sets.
        assert_ne!(
            FilterBankSelection::NonPerfectReconstruction.spec_table_name(),
            FilterBankSelection::PerfectReconstruction.spec_table_name()
        );
    }

    // -----------------------------------------------------------
    // coefficients — §D.8 table resolution.
    // -----------------------------------------------------------

    #[test]
    fn coefficients_for_non_perfect_is_the_lossy_table() {
        // Spec line 176: `if (FILTS==0) prCoeff = raCoeffLossy;` —
        // the non-perfect variant resolves to the §D.8 "Non-Perfect
        // Reconstruction" column.
        assert!(core::ptr::eq(
            FilterBankSelection::NonPerfectReconstruction.coefficients(),
            &crate::fir_coeff::RA_COEFF_LOSSY,
        ));
    }

    #[test]
    fn coefficients_for_perfect_is_the_lossless_table() {
        // Spec line 177: `else prCoeff = raCoeffLossLess;` — the
        // perfect variant resolves to the §D.8 "Perfect
        // Reconstruction" column.
        assert!(core::ptr::eq(
            FilterBankSelection::PerfectReconstruction.coefficients(),
            &crate::fir_coeff::RA_COEFF_LOSSLESS,
        ));
    }

    #[test]
    fn coefficients_composed_with_from_filts_reproduces_the_spec_branch() {
        // from_filts + coefficients together are the §C.2.5
        // two-line `prCoeff` assignment.
        assert!(core::ptr::eq(
            FilterBankSelection::from_filts(0).coefficients(),
            &crate::fir_coeff::RA_COEFF_LOSSY,
        ));
        assert!(core::ptr::eq(
            FilterBankSelection::from_filts(1).coefficients(),
            &crate::fir_coeff::RA_COEFF_LOSSLESS,
        ));
    }

    // -----------------------------------------------------------
    // Trait derives.
    // -----------------------------------------------------------

    #[test]
    fn variants_are_copyable_and_comparable() {
        // The enum is Copy + PartialEq + Eq + Hash by derive; make
        // sure those land and behave as expected (a sibling crate
        // using this enum in a HashMap key needs Hash + Eq).
        let a = FilterBankSelection::NonPerfectReconstruction;
        let b = a;
        assert_eq!(a, b);
        assert_ne!(a, FilterBankSelection::PerfectReconstruction);

        // Hash collision check: hash both variants into the same
        // hasher and confirm the resulting digests differ — not a
        // proof of correctness but a sanity check that the derive
        // hashes the discriminant.
        use core::hash::{Hash, Hasher};
        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        FilterBankSelection::NonPerfectReconstruction.hash(&mut h1);
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        FilterBankSelection::PerfectReconstruction.hash(&mut h2);
        assert_ne!(h1.finish(), h2.finish());
    }

    #[test]
    fn variants_have_stable_debug_output() {
        // Debug-format the variants so downstream test failures
        // print recognisable enum-variant names rather than opaque
        // discriminant integers.
        let s = format!("{:?}", FilterBankSelection::NonPerfectReconstruction);
        assert!(
            s.contains("NonPerfectReconstruction"),
            "Debug should name the variant, got {s:?}"
        );
        let s = format!("{:?}", FilterBankSelection::PerfectReconstruction);
        assert!(
            s.contains("PerfectReconstruction"),
            "Debug should name the variant, got {s:?}"
        );
    }
}

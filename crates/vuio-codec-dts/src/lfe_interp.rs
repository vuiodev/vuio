//! Typed selector for the §C.2.6 `InterpolationFIR()` 512-tap LFE
//! interpolation FIR coefficient set.
//!
//! `InterpolationFIR(int nDecimationSelect)` (ETSI TS 102 114 V1.3.1
//! Annex C §C.2.6, PDF p.186, per `docs/audio/dts/dts-qmf-driver.md`
//! §3) drives the DTS Core low-frequency-effects (LFE) reconstruction
//! path. It takes an `nDecimationSelect` parameter (the LFE decimation
//! factor) that selects between two named §D.8 coefficient sets, per
//! the resolution in `dts-qmf-driver.md` §3:
//!
//! ```text
//!     | nDecimationSelect | Decimation factor | Coefficient set |
//!     | 1                 | 128               | raCoeff128      |
//!     | else (0)          | 64                | raCoeff64       |
//! ```
//!
//! The two coefficient sets (`raCoeff64`, the 64x-interpolation LFE
//! FIR, and `raCoeff128`, the 128x-interpolation LFE FIR) are defined
//! in §D.8 "32-Band Interpolation and LFE Interpolation FIR" (staged
//! PDF p.238-246) and transcribed at the crate root as
//! [`crate::RA_COEFF_LFE64`] / [`crate::RA_COEFF_LFE128`]. Both are
//! 512 taps (`NumFIRCoef = 512`, §C.2.6).
//!
//! This module exposes the §C.2.6 selection step as a typed
//! [`LfeInterpolationSelection`] enum plus a
//! [`LfeInterpolationSelection::from_decimation_select`] resolver that
//! mirrors the spec's `if (nDecimationSelect == 1) … else …` branch,
//! and [`LfeInterpolationSelection::coefficients`] resolves the
//! selection to the matching §D.8 512-tap table — exactly the
//! companion of [`crate::FilterBankSelection`] for the LFE path.
//!
//! # Driver-body scope
//!
//! This module lands the table *selection* and the table *data*. The
//! §C.2.6 `InterpolationFIR()` per-sample polyphase convolution loop
//! **body** is implemented in [`crate::LfeInterpolator`]
//! (`src/lfe_synth.rs`), transcribed from
//! `docs/audio/dts/dts-lfe-interpolation-and-audio-walker.md` §1. LFE
//! samples are pre-scaled at dequant time
//! (`LFECh.rLFE[k] = LFE[n]*rScale` with `rScale = nScale*0.035`), so
//! the convolution body carries no §C.2.5-style output `rScale`.

/// The two named 512-tap LFE-interpolation FIR coefficient sets
/// referenced by `InterpolationFIR()` per ETSI TS 102 114 V1.3.1
/// Annex C §C.2.6 (resolved in `docs/audio/dts/dts-qmf-driver.md`
/// §3).
///
/// Each variant names exactly one of the two §D.8 LFE-interpolation
/// columns ("64 x Interpolation" / "128 x Interpolation",
/// PDF p.238-246), transcribed as [`crate::RA_COEFF_LFE64`] /
/// [`crate::RA_COEFF_LFE128`] and reachable through
/// [`Self::coefficients`]. The variant names mirror the spec
/// pseudocode's identifiers (`raCoeff64` for the 64x set,
/// `raCoeff128` for the 128x set) rendered in idiomatic Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LfeInterpolationSelection {
    /// The §C.2.6 `raCoeff64` 512-tap **64x-interpolation** LFE FIR
    /// (§D.8). Selected by `nDecimationSelect == 0` (decimation
    /// factor 64) per the §C.2.6 driver's `else` branch.
    Decimation64,
    /// The §C.2.6 `raCoeff128` 512-tap **128x-interpolation** LFE FIR
    /// (§D.8). Selected by `nDecimationSelect == 1` (decimation
    /// factor 128) per the §C.2.6 driver's `if` branch.
    Decimation128,
}

impl LfeInterpolationSelection {
    /// Resolve a §C.2.6 `nDecimationSelect` value to the named §D.8
    /// LFE coefficient set it picks, per the driver mapping in
    /// `dts-qmf-driver.md` §3:
    /// `if (nDecimationSelect == 1) raCoeff128; else raCoeff64;`.
    ///
    /// The resolved table mapping distinguishes only the
    /// `nDecimationSelect == 1` (128x) case from everything else
    /// (64x), matching the spec's `1` / `else` split exactly — so this
    /// resolver accepts an arbitrary `u8` and groups every non-`1`
    /// value into [`LfeInterpolationSelection::Decimation64`].
    #[must_use]
    pub fn from_decimation_select(n_decimation_select: u8) -> Self {
        if n_decimation_select == 1 {
            LfeInterpolationSelection::Decimation128
        } else {
            LfeInterpolationSelection::Decimation64
        }
    }

    /// Inverse of [`Self::from_decimation_select`]: the **canonical**
    /// `nDecimationSelect` value the §C.2.6 driver reads to select
    /// this coefficient set.
    ///
    /// Returns `0` for [`LfeInterpolationSelection::Decimation64`]
    /// (the `else` branch's canonical representative; the driver
    /// collapses every non-`1` value to the same branch, so `0` is the
    /// natural choice) and `1` for
    /// [`LfeInterpolationSelection::Decimation128`].
    #[must_use]
    pub fn decimation_select(self) -> u8 {
        match self {
            LfeInterpolationSelection::Decimation64 => 0,
            LfeInterpolationSelection::Decimation128 => 1,
        }
    }

    /// The LFE decimation factor this selection corresponds to — `64`
    /// for [`LfeInterpolationSelection::Decimation64`] and `128` for
    /// [`LfeInterpolationSelection::Decimation128`], per the
    /// `dts-qmf-driver.md` §3 mapping.
    #[must_use]
    pub fn decimation_factor(self) -> u32 {
        match self {
            LfeInterpolationSelection::Decimation64 => 64,
            LfeInterpolationSelection::Decimation128 => 128,
        }
    }

    /// The §C.2.6 coefficient-table identifier this selection names,
    /// as written in the staged driver resolution (`raCoeff64` or
    /// `raCoeff128`).
    ///
    /// Returned as a `&'static str` so callers can format spec-
    /// referencing diagnostics without reaching into the enum
    /// variants; the strings match the pseudocode's identifiers
    /// verbatim.
    #[must_use]
    pub fn spec_table_name(self) -> &'static str {
        match self {
            LfeInterpolationSelection::Decimation64 => "raCoeff64",
            LfeInterpolationSelection::Decimation128 => "raCoeff128",
        }
    }

    /// The §D.8 512-tap LFE coefficient table this selection picks —
    /// the §C.2.6 driver's `prCoeff` after the
    /// `if (nDecimationSelect == 1) … else …` selection, ready for the
    /// LFE interpolation step.
    ///
    /// Returns [`crate::RA_COEFF_LFE64`] (the §D.8 "64 x
    /// Interpolation" column) for
    /// [`LfeInterpolationSelection::Decimation64`] and
    /// [`crate::RA_COEFF_LFE128`] (the "128 x Interpolation" column)
    /// for [`LfeInterpolationSelection::Decimation128`], both
    /// transcribed verbatim from the staged PDF p.238-246.
    #[must_use]
    pub fn coefficients(self) -> &'static [f64; crate::lfe_fir_coeff::LFE_FIR_COEFF_LEN] {
        match self {
            LfeInterpolationSelection::Decimation64 => &crate::lfe_fir_coeff::RA_COEFF_LFE64,
            LfeInterpolationSelection::Decimation128 => &crate::lfe_fir_coeff::RA_COEFF_LFE128,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------
    // from_decimation_select — selection per §C.2.6 mapping.
    // -----------------------------------------------------------

    #[test]
    fn from_decimation_select_one_picks_128x() {
        // dts-qmf-driver.md §3: nDecimationSelect == 1 → raCoeff128.
        assert_eq!(
            LfeInterpolationSelection::from_decimation_select(1),
            LfeInterpolationSelection::Decimation128
        );
    }

    #[test]
    fn from_decimation_select_zero_picks_64x() {
        // dts-qmf-driver.md §3: else (0) → raCoeff64.
        assert_eq!(
            LfeInterpolationSelection::from_decimation_select(0),
            LfeInterpolationSelection::Decimation64
        );
    }

    #[test]
    fn from_decimation_select_treats_every_non_one_value_as_64x() {
        // The §C.2.6 mapping is `1` vs `else`; every value other than
        // 1 picks the 64x set. Verify across the full u8 range.
        for n in 0u16..=255 {
            if n == 1 {
                continue;
            }
            assert_eq!(
                LfeInterpolationSelection::from_decimation_select(n as u8),
                LfeInterpolationSelection::Decimation64,
                "nDecimationSelect={n} should pick the 64x else branch"
            );
        }
    }

    // -----------------------------------------------------------
    // decimation_select — canonical inverse.
    // -----------------------------------------------------------

    #[test]
    fn decimation_select_round_trips_64x() {
        let sel = LfeInterpolationSelection::Decimation64;
        assert_eq!(sel.decimation_select(), 0);
        assert_eq!(
            LfeInterpolationSelection::from_decimation_select(sel.decimation_select()),
            sel
        );
    }

    #[test]
    fn decimation_select_round_trips_128x() {
        let sel = LfeInterpolationSelection::Decimation128;
        assert_eq!(sel.decimation_select(), 1);
        assert_eq!(
            LfeInterpolationSelection::from_decimation_select(sel.decimation_select()),
            sel
        );
    }

    // -----------------------------------------------------------
    // decimation_factor — the spec's decimation factor.
    // -----------------------------------------------------------

    #[test]
    fn decimation_factor_is_64_or_128() {
        assert_eq!(
            LfeInterpolationSelection::Decimation64.decimation_factor(),
            64
        );
        assert_eq!(
            LfeInterpolationSelection::Decimation128.decimation_factor(),
            128
        );
    }

    // -----------------------------------------------------------
    // spec_table_name — pseudocode identifier passthrough.
    // -----------------------------------------------------------

    #[test]
    fn spec_table_name_matches_the_c26_identifiers() {
        assert_eq!(
            LfeInterpolationSelection::Decimation64.spec_table_name(),
            "raCoeff64"
        );
        assert_eq!(
            LfeInterpolationSelection::Decimation128.spec_table_name(),
            "raCoeff128"
        );
        assert_ne!(
            LfeInterpolationSelection::Decimation64.spec_table_name(),
            LfeInterpolationSelection::Decimation128.spec_table_name()
        );
    }

    // -----------------------------------------------------------
    // coefficients — §D.8 LFE table resolution.
    // -----------------------------------------------------------

    #[test]
    fn coefficients_for_64x_is_the_lfe64_table() {
        assert!(core::ptr::eq(
            LfeInterpolationSelection::Decimation64.coefficients(),
            &crate::lfe_fir_coeff::RA_COEFF_LFE64,
        ));
    }

    #[test]
    fn coefficients_for_128x_is_the_lfe128_table() {
        assert!(core::ptr::eq(
            LfeInterpolationSelection::Decimation128.coefficients(),
            &crate::lfe_fir_coeff::RA_COEFF_LFE128,
        ));
    }

    #[test]
    fn coefficients_composed_with_from_decimation_select_reproduces_the_mapping() {
        // from_decimation_select + coefficients together are the
        // §C.2.6 driver's two-line table selection.
        assert!(core::ptr::eq(
            LfeInterpolationSelection::from_decimation_select(0).coefficients(),
            &crate::lfe_fir_coeff::RA_COEFF_LFE64,
        ));
        assert!(core::ptr::eq(
            LfeInterpolationSelection::from_decimation_select(1).coefficients(),
            &crate::lfe_fir_coeff::RA_COEFF_LFE128,
        ));
    }

    // -----------------------------------------------------------
    // Trait derives.
    // -----------------------------------------------------------

    #[test]
    fn variants_are_copyable_and_comparable() {
        let a = LfeInterpolationSelection::Decimation64;
        let b = a;
        assert_eq!(a, b);
        assert_ne!(a, LfeInterpolationSelection::Decimation128);

        use core::hash::{Hash, Hasher};
        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        LfeInterpolationSelection::Decimation64.hash(&mut h1);
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        LfeInterpolationSelection::Decimation128.hash(&mut h2);
        assert_ne!(h1.finish(), h2.finish());
    }

    #[test]
    fn variants_have_stable_debug_output() {
        let s = format!("{:?}", LfeInterpolationSelection::Decimation64);
        assert!(
            s.contains("Decimation64"),
            "Debug should name the variant, got {s:?}"
        );
        let s = format!("{:?}", LfeInterpolationSelection::Decimation128);
        assert!(
            s.contains("Decimation128"),
            "Debug should name the variant, got {s:?}"
        );
    }
}

//! Encoder-side enhanced coupling — ATSC A/52:2018 Annex E
//! §E.2.3.3.16-26 (syntax) / §E.3.5.5 (decode model), encode direction.
//!
//! Enhanced coupling replaces the waveform coding of every coupled
//! channel's high-frequency region with **one** shared carrier channel
//! plus per-band amplitude / angle / chaos coordinates (§E.3.5.5). The
//! encoder therefore has three jobs:
//!
//! 1. **Carrier construction** — produce the enhanced-coupling channel's
//!    MDCT coefficients over the active region. The first coupled
//!    channel's angle and chaos are spec-fixed to `0` (§E.3.5.5.2-3:
//!    they are never transmitted for `firstchincpl`), which pins the
//!    carrier's per-band *phase* to that channel: any carrier whose
//!    phase deviates from the first coupled channel injects an
//!    uncorrectable phase error into its reconstruction. The carrier is
//!    therefore built from the first coupled channel's own MDCT
//!    coefficients, scaled **per band** by [`carrier_band_gains`] so
//!    that every other coupled channel's amplitude coordinate stays
//!    within the Table E3.10 representable ceiling of `1.0` (the
//!    loudest channel in each band maps to amp ≈ 1). Per-band scaling
//!    of real MDCT coefficients is exactly linear through the
//!    §E.3.5.5.1 analysis, so the carrier stays self-consistent.
//! 2. **Coordinate measurement** — for each coupled channel and band,
//!    measure the amplitude ratio and phase difference against the
//!    carrier **in the §E.3.5.5.1 complex analysis domain** (the same
//!    `Z[k]` the decoder synthesises from), via
//!    [`band_cross_stats`] over [`super::ecpl::reconstruct_carrier`]
//!    outputs. Measuring against the decoder-faithful carrier (with its
//!    zero next-block spectrum at the frame edge) folds every analysis
//!    imperfection into the transmitted coordinates.
//! 3. **Coordinate quantisation** — [`quantise_amp`] (inverse of
//!    Table E3.10, ~1.5 dB grid, code 31 = -∞ dB) and
//!    [`quantise_angle`] (inverse of Table E3.11, π/32 grid with wrap).
//!
//! The bitstream emission (strategy + coordinate blocks, carrier
//! exponents / bit allocation / mantissas) lives in
//! [`super::encoder`]; this module is the pure DSP + quantiser layer,
//! unit-tested standalone. The chaos coordinate is transmitted as `0`
//! (fully correlated model) and `ecpltrans` as `0` — the §E.3.5.5.3
//! random de-correlation then contributes nothing, keeping the decode
//! deterministic and the amplitude modification factor at unity.

use super::ecpl::{ampbnd, band_bin_counts, begin_bin, end_bin, EcplCarrier, N_ECPL_SUBBND};
use crate::audblk::N_COEFFS;

/// Encoder-facing enhanced-coupling parameters (§E.2.3.3.16-17 codes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EcplParams {
    /// `ecplbegf` (4 bits) — begin-frequency code, Table E3.8. The
    /// encoder restricts it to `2..=13` (begin sub-band ≥ 4, i.e. the
    /// region starts at transform coefficient 37 or above) so the
    /// carrier grid aligns with the shared coupling-channel exponent /
    /// bit-allocation machinery.
    pub ecplbegf: u8,
    /// `ecplendf` (4 bits) — end-frequency code, Table E3.8:
    /// `ecpl_end_subbnd = ecplendf + 7`.
    pub ecplendf: u8,
    /// Signal per-band **chaos** coordinates (§E.2.3.3.25 /
    /// Table E3.12) derived from the measured band coherence, so the
    /// decoder's §E.3.5.5.3 random de-correlation restores the
    /// statistical character of channel content the shared carrier
    /// cannot represent (inter-channel width). The transmitted
    /// amplitude is pre-divided by the decoder's `1 + 0.38·chaosval`
    /// modification so band energies stay matched. `false` transmits
    /// chaos 0 everywhere (fully correlated model — the decode is then
    /// a deterministic pure amplitude+angle reconstruction).
    pub chaos: bool,
}

impl Default for EcplParams {
    /// Default geometry: begin sub-band 4 (tc 37 ≈ 3.5 kHz at 48 kHz)
    /// through end sub-band 22 (tc 253) — the full coupling-eligible
    /// region above the independently-coded low band. Coherence-driven
    /// chaos coordinates are on.
    fn default() -> Self {
        Self {
            ecplbegf: 2,
            ecplendf: 15,
            chaos: true,
        }
    }
}

/// Resolved enhanced-coupling geometry the encoder codes against.
#[derive(Clone, Debug)]
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub struct EcplGeometry {
    /// Raw begin-frequency code (re-emitted in the strategy block).
    pub ecplbegf: u8,
    /// Raw end-frequency code (re-emitted in the strategy block). Not
    /// transmitted when the region is SPX-bounded (`spx_bounded`).
    pub ecplendf: u8,
    /// The region's end is derived from the SPX begin frequency
    /// (§E.2.3.3.17 SPX-in-use arm) — `ecplendf` is NOT transmitted.
    pub spx_bounded: bool,
    /// `ecpl_begin_subbnd` (Table E3.8).
    pub begin_subbnd: usize,
    /// `ecpl_end_subbnd` (Table E3.8).
    pub end_subbnd: usize,
    /// Banding structure in force (`ecplbndstrce = 0` → the Table E2.14
    /// default), indexed by absolute sub-band.
    pub bndstrc: [bool; N_ECPL_SUBBND],
    /// Number of coordinate bands (§E.2.3.3.19).
    pub necplbnd: usize,
    /// First transform coefficient of the region (`ecplstartmant`).
    pub start_bin: usize,
    /// One-past-the-last transform coefficient (`ecplendmant`).
    pub end_bin: usize,
    /// Per-band bin counts (§E.3.5.5.1 `nbins_per_bnd_array[]`).
    pub band_bins: Vec<usize>,
}

impl EcplGeometry {
    /// Derive the geometry from the raw codes + a banding structure
    /// (pass [`super::ecpl::DEFAULT_ECPL_BNDSTRC`] for the
    /// `ecplbndstrce = 0` default). Errors mirror the decoder-side
    /// `parse_strategy` guards plus the encoder's own grid restriction.
    pub fn derive(
        params: &EcplParams,
        bndstrc: &[bool; N_ECPL_SUBBND],
    ) -> oxideav_core::Result<Self> {
        if params.ecplbegf > 15 || params.ecplendf > 15 {
            return Err(oxideav_core::Error::invalid(
                "eac3 ecpl encoder: ecplbegf/ecplendf are 4-bit codes (0..=15)",
            ));
        }
        let begin = super::ecpl::begin_subbnd(params.ecplbegf);
        let end = super::ecpl::end_subbnd(false, params.ecplendf, 0);
        if begin < 4 {
            return Err(oxideav_core::Error::invalid(
                "eac3 ecpl encoder: ecplbegf < 2 (begin sub-band < 4) is not \
                 supported — the region must start at transform coefficient 37 \
                 or above",
            ));
        }
        if begin >= end || end > N_ECPL_SUBBND {
            return Err(oxideav_core::Error::invalid(
                "eac3 ecpl encoder: empty or out-of-grid sub-band range \
                 (need begin < end <= 22)",
            ));
        }
        // §E.2.3.3.19: bndstrc entries up to and including
        // max(begin, 8) are always zero — mask the (default) table so
        // a region beginning at sub-band 9+ cannot carry a phantom
        // leading merge (necplbnd vs the band walk would disagree).
        let mut bndstrc = *bndstrc;
        for slot in bndstrc
            .iter_mut()
            .take((begin.max(8) + 1).min(N_ECPL_SUBBND))
        {
            *slot = false;
        }
        let necplbnd = super::ecpl::necplbnd(begin, end, &bndstrc);
        let band_bins = band_bin_counts(begin, end, &bndstrc);
        Ok(Self {
            ecplbegf: params.ecplbegf,
            ecplendf: params.ecplendf,
            begin_subbnd: begin,
            end_subbnd: end,
            bndstrc,
            necplbnd,
            start_bin: begin_bin(begin),
            end_bin: end_bin(end),
            band_bins,
            spx_bounded: false,
        })
    }

    /// Derive the geometry for the **SPX co-active** configuration
    /// (§E.2.3.3.17 SPX-in-use arm / §3.6.1 "coupling for a mid-range
    /// portion ... spectral extension for the higher-range portion"):
    /// the enhanced-coupling region ends where the SPX region begins,
    /// `ecplendf` is not transmitted, and `spxbegf` (the raw 3-bit SPX
    /// begin code) drives the end sub-band — `spxbegf + 5` for
    /// `spxbegf < 6`, else `spxbegf * 2` — so the two regions abut
    /// exactly (`ecplsubbndtab[end] == spxbandtable[spx_begin]`).
    pub fn derive_with_spx(
        params: &EcplParams,
        spxbegf: u8,
        bndstrc: &[bool; N_ECPL_SUBBND],
    ) -> oxideav_core::Result<Self> {
        if params.ecplbegf > 15 {
            return Err(oxideav_core::Error::invalid(
                "eac3 ecpl encoder: ecplbegf is a 4-bit code (0..=15)",
            ));
        }
        let begin = super::ecpl::begin_subbnd(params.ecplbegf);
        let end = super::ecpl::end_subbnd(true, 0, spxbegf as usize);
        if begin < 4 {
            return Err(oxideav_core::Error::invalid(
                "eac3 ecpl encoder: ecplbegf < 2 (begin sub-band < 4) is not \
                 supported — the region must start at transform coefficient 37 \
                 or above",
            ));
        }
        if begin >= end || end > N_ECPL_SUBBND {
            return Err(oxideav_core::Error::invalid(
                "eac3 ecpl encoder: SPX begin frequency leaves an empty or \
                 out-of-grid enhanced-coupling region (need begin < end <= 22)",
            ));
        }
        // §E.2.3.3.19: bndstrc entries up to and including
        // max(begin, 8) are always zero — mask the (default) table so
        // a region beginning at sub-band 9+ cannot carry a phantom
        // leading merge (necplbnd vs the band walk would disagree).
        let mut bndstrc = *bndstrc;
        for slot in bndstrc
            .iter_mut()
            .take((begin.max(8) + 1).min(N_ECPL_SUBBND))
        {
            *slot = false;
        }
        let necplbnd = super::ecpl::necplbnd(begin, end, &bndstrc);
        let band_bins = band_bin_counts(begin, end, &bndstrc);
        Ok(Self {
            ecplbegf: params.ecplbegf,
            ecplendf: 0,
            begin_subbnd: begin,
            end_subbnd: end,
            bndstrc,
            necplbnd,
            start_bin: begin_bin(begin),
            end_bin: end_bin(end),
            band_bins,
            spx_bounded: true,
        })
    }
}

/// Quantise a linear amplitude to the nearest Table E3.10 `ecplamp`
/// code (log-domain nearest neighbour over the ~1.5 dB grid).
///
/// Values at or above `1.0` saturate to code 0 (0 dB — the table's
/// ceiling); values more than half a grid step below the code-30 floor
/// (≈ -45 dB) map to code 31 (-∞ dB, amplitude 0), as do non-positive
/// inputs.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn quantise_amp(a: f32) -> u8 {
    if a.is_nan() || a <= 0.0 {
        return 31;
    }
    if a >= 1.0 {
        return 0;
    }
    let target = a.ln();
    let mut best = 0u8;
    let mut best_err = f32::INFINITY;
    for code in 0..=30u8 {
        let v = ampbnd(code);
        let err = (v.ln() - target).abs();
        if err < best_err {
            best_err = err;
            best = code;
        }
    }
    // Below the representable floor: if the input is further below
    // code 30 than half the local grid step (~1.4 dB), -∞ is closer.
    let floor = ampbnd(30);
    if a < floor && (floor / a) > 1.09 {
        // 1.09 ≈ 10^(0.75/20) — half of the ~1.5 dB step.
        return 31;
    }
    best
}

/// Quantise an angle in the spec's normalised units (`-1.0 ..= 1.0`
/// representing `-π ..= π`) to the nearest Table E3.11 `ecplangle`
/// code (π/32 grid). The value wraps modulo 2.0 first, so any real
/// phase difference maps onto the table's principal interval.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn quantise_angle(units: f32) -> u8 {
    if !units.is_finite() {
        return 0;
    }
    // Round to the nearest 1/32 step, then wrap into [-32, 31].
    let mut m = (units * 32.0).round() as i64;
    m = m.rem_euclid(64);
    if m >= 32 {
        m -= 64;
    }
    if m >= 0 {
        m as u8 // codes 0..=31: 0.0 .. 0.96875
    } else {
        (m + 64) as u8 // codes 32..=63: -1.0 .. -0.03125
    }
}

/// Per-band cross statistics between a coupled channel's complex
/// analysis spectrum `X[k]` and the carrier's `Z[k]` (both §E.3.5.5.1
/// outputs), accumulated over one or more blocks.
#[derive(Clone, Copy, Debug, Default)]
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub struct BandStats {
    /// `Σ |X[k]|²` — channel energy in the band.
    pub ex: f64,
    /// `Σ |Z[k]|²` — carrier energy in the band.
    pub ez: f64,
    /// `Σ Re(X · conj(Z))`.
    pub cr: f64,
    /// `Σ Im(X · conj(Z))`.
    pub ci: f64,
}

impl BandStats {
    /// Energy-matching amplitude coordinate: `sqrt(ex / ez)`
    /// (§E.3.5.5.2's amplitude semantics — the reconstruction
    /// `amp · Z` carries the channel's band energy). Zero-carrier
    /// bands yield 0 (nothing to scale).
    pub fn amp(&self) -> f32 {
        if self.ez <= 0.0 || self.ex <= 0.0 {
            return 0.0;
        }
        (self.ex / self.ez).sqrt() as f32
    }

    /// Band phase difference in Table E3.11 units (`-1.0 ..= 1.0` =
    /// `-π ..= π`): the argument of the band-summed cross spectrum.
    /// A zero cross spectrum (uncorrelated or silent) yields 0.
    pub fn angle_units(&self) -> f32 {
        if self.cr == 0.0 && self.ci == 0.0 {
            return 0.0;
        }
        (self.ci.atan2(self.cr) / std::f64::consts::PI) as f32
    }

    /// Band **coherence** `|Σ X·conj(Z)| / sqrt(ΣE_X · ΣE_Z)` in
    /// `0.0 ..= 1.0`: how much of the channel's band content is a
    /// (rotated, scaled) copy of the carrier. `1.0` = fully coherent
    /// (a pure amplitude+angle relation reconstructs it exactly);
    /// low values mean the channel carries content the carrier does
    /// not — the §E.3.5.5.3 chaos de-correlation restores its
    /// statistical character. Silent bands report full coherence
    /// (nothing to de-correlate).
    pub fn coherence(&self) -> f32 {
        if self.ex <= 0.0 || self.ez <= 0.0 {
            return 1.0;
        }
        let num = (self.cr * self.cr + self.ci * self.ci).sqrt();
        (num / (self.ex * self.ez).sqrt()).clamp(0.0, 1.0) as f32
    }
}

/// Map a band coherence to a Table E3.12 `ecplchaos` code (0..=7,
/// value `-code/7`): the incoherent fraction `1 - γ` scaled onto the
/// 8-step grid. Fully coherent → code 0 (no de-correlation); fully
/// incoherent → code 7 (angle jitter up to ±π, uniform phase).
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn chaos_code_for(coherence: f32) -> u8 {
    let incoherent = (1.0 - coherence).clamp(0.0, 1.0);
    (incoherent * 7.0).round() as u8
}

/// The §E.3.5.5.2 amplitude-modification factor the decoder applies to
/// a non-transient, non-first coupled channel: `1 + 0.38 · chaosval`
/// with `chaosval = -code/7` (Table E3.12) — always in `0.62 ..= 1.0`.
/// The encoder divides its measured amplitude by this factor before
/// quantisation so the decoded band energy stays matched.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn chaos_amp_factor(chaos_code: u8) -> f32 {
    let chaosval = -(chaos_code.min(7) as f32) / 7.0;
    1.0 + 0.38 * chaosval
}

/// Accumulate per-band cross statistics for one block: `X[k]` is the
/// coupled channel's analysis spectrum, `Z[k]` the carrier's, both from
/// [`super::ecpl::reconstruct_carrier`]. Only the first-half bins
/// (`k < 256`, the MDCT-aligned half the §E.3.5.5.4 synthesis reads)
/// inside the active region contribute. `stats` has one slot per band
/// (`geom.necplbnd`), accumulated in place so multi-block coordinate
/// spans sum their statistics before quantisation.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn band_cross_stats(
    x: &EcplCarrier,
    z: &EcplCarrier,
    geom: &EcplGeometry,
    stats: &mut [BandStats],
) {
    let mut bin = geom.start_bin;
    for (bnd, &nbins) in geom.band_bins.iter().enumerate() {
        if bnd >= stats.len() {
            break;
        }
        let s = &mut stats[bnd];
        for k in bin..(bin + nbins).min(256) {
            let xr = x.zr[k] as f64;
            let xi = x.zi[k] as f64;
            let zr = z.zr[k] as f64;
            let zi = z.zi[k] as f64;
            s.ex += xr * xr + xi * xi;
            s.ez += zr * zr + zi * zi;
            // X · conj(Z)
            s.cr += xr * zr + xi * zi;
            s.ci += xi * zr - xr * zi;
        }
        bin += nbins;
    }
}

/// Per-band MDCT-domain energies of one channel over a span of blocks
/// (the coordinate-refresh span), used to size the carrier gains.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn band_mdct_energies(blocks: &[&[f32; N_COEFFS]], geom: &EcplGeometry) -> Vec<f64> {
    let mut out = vec![0.0f64; geom.necplbnd];
    for mdct in blocks {
        let mut bin = geom.start_bin;
        for (bnd, &nbins) in geom.band_bins.iter().enumerate() {
            for k in bin..(bin + nbins).min(N_COEFFS) {
                out[bnd] += (mdct[k] as f64) * (mdct[k] as f64);
            }
            bin += nbins;
        }
    }
    out
}

/// Per-band carrier gains: the carrier is the first coupled channel
/// scaled up (never down) so the loudest coupled channel in each band
/// lands at an amplitude coordinate ≈ 1.0 — the Table E3.10 ceiling.
///
/// `energies[ch][bnd]` are the [`band_mdct_energies`] of every coupled
/// channel over the span, with `energies[0]` the first coupled channel
/// (the carrier source). Gains are clamped to `[1, 32]` (+30 dB): a
/// band where the carrier source is silent but another channel is loud
/// cannot be represented losslessly by a phase-locked carrier anyway,
/// and an unbounded gain would blow up the carrier's exponent envelope.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn carrier_band_gains(energies: &[Vec<f64>], nbands: usize) -> Vec<f32> {
    let mut gains = vec![1.0f32; nbands];
    let Some(base) = energies.first() else {
        return gains;
    };
    for bnd in 0..nbands {
        let e0 = base.get(bnd).copied().unwrap_or(0.0);
        let mut emax = 0.0f64;
        for ch in energies {
            emax = emax.max(ch.get(bnd).copied().unwrap_or(0.0));
        }
        if e0 > 0.0 && emax > e0 {
            gains[bnd] = ((emax / e0).sqrt() as f32).clamp(1.0, 32.0);
        }
    }
    gains
}

/// Build one block's carrier MDCT buffer: the first coupled channel's
/// coefficients restricted to the active region, scaled per band by
/// `gains`. Bins outside `[start_bin, end_bin)` are zero (the
/// §E.3.5.5.1 step-1 `XCURR` definition).
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn build_carrier_block(
    src_mdct: &[f32; N_COEFFS],
    gains: &[f32],
    geom: &EcplGeometry,
) -> [f32; 256] {
    let mut out = [0.0f32; 256];
    let mut bin = geom.start_bin;
    for (bnd, &nbins) in geom.band_bins.iter().enumerate() {
        let g = gains.get(bnd).copied().unwrap_or(1.0);
        for k in bin..(bin + nbins).min(256) {
            out[k] = src_mdct[k] * g;
        }
        bin += nbins;
    }
    out
}

/// Restrict one block's MDCT coefficients to the active region (zero
/// outside) — the per-channel analysis input mirroring the carrier's
/// step-1 buffer definition.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn region_restrict(mdct: &[f32; N_COEFFS], geom: &EcplGeometry) -> [f32; 256] {
    let mut out = [0.0f32; 256];
    let end = geom.end_bin.min(256);
    out[geom.start_bin..end].copy_from_slice(&mdct[geom.start_bin..end]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eac3::ecpl::{
        generate_channel_coeffs, reconstruct_carrier, DEFAULT_ECPL_BNDSTRC, ECPL_ANGLE_TAB,
    };
    use crate::mdct::mdct_512;
    use crate::tables::WINDOW;

    fn geom_default() -> EcplGeometry {
        EcplGeometry::derive(&EcplParams::default(), &DEFAULT_ECPL_BNDSTRC).unwrap()
    }

    #[test]
    fn geometry_default_full_region() {
        let g = geom_default();
        assert_eq!(g.begin_subbnd, 4);
        assert_eq!(g.end_subbnd, 22);
        assert_eq!(g.start_bin, 37);
        assert_eq!(g.end_bin, 253);
        // Default banding: 18 sub-bands in span, 9 merges (Table E2.14
        // rows 9, 11, 13, 15, 16, 17, 19, 20, 21) → 9 bands.
        assert_eq!(g.necplbnd, 9);
        assert_eq!(g.band_bins.iter().sum::<usize>(), 253 - 37);
    }

    #[test]
    fn geometry_rejects_low_begin_and_inverted_range() {
        // ecplbegf 0/1 → begin sub-band 0/2 < 4 → rejected.
        for begf in [0u8, 1] {
            let p = EcplParams {
                ecplbegf: begf,
                ecplendf: 15,
                ..EcplParams::default()
            };
            assert!(EcplGeometry::derive(&p, &DEFAULT_ECPL_BNDSTRC).is_err());
        }
        // begin >= end.
        let p = EcplParams {
            ecplbegf: 13,
            ecplendf: 0,
            ..EcplParams::default()
        };
        // begin_subbnd(13) = 16, end = 7 → inverted.
        assert!(EcplGeometry::derive(&p, &DEFAULT_ECPL_BNDSTRC).is_err());
    }

    #[test]
    fn quantise_amp_roundtrips_every_code() {
        for code in 0..=31u8 {
            let v = ampbnd(code);
            assert_eq!(
                quantise_amp(v),
                code,
                "code {code} (amp {v}) did not round-trip"
            );
        }
    }

    #[test]
    fn quantise_amp_edges() {
        assert_eq!(quantise_amp(0.0), 31);
        assert_eq!(quantise_amp(-1.0), 31);
        assert_eq!(quantise_amp(f32::NAN), 31);
        assert_eq!(quantise_amp(2.0), 0);
        // Just below the code-30 floor stays 30; far below → 31.
        assert_eq!(quantise_amp(ampbnd(30) * 0.95), 30);
        assert_eq!(quantise_amp(ampbnd(30) * 0.25), 31);
        // Midpoints land on a neighbour, never off-grid.
        let mid = (ampbnd(3) * ampbnd(4)).sqrt();
        let q = quantise_amp(mid);
        assert!(q == 3 || q == 4);
    }

    #[test]
    fn quantise_angle_roundtrips_every_code() {
        for code in 0..64u8 {
            let v = ECPL_ANGLE_TAB[code as usize];
            assert_eq!(
                quantise_angle(v),
                code,
                "code {code} (angle {v}) did not round-trip"
            );
        }
    }

    #[test]
    fn chaos_code_and_amp_factor() {
        // Full coherence → no chaos; full incoherence → code 7.
        assert_eq!(chaos_code_for(1.0), 0);
        assert_eq!(chaos_code_for(0.0), 7);
        // Grid midpoints round to the nearest step.
        assert_eq!(chaos_code_for(0.5), 4); // (1-0.5)*7 = 3.5 → 4
        assert_eq!(chaos_code_for(1.5), 0); // clamped
                                            // Amplitude-modification factor: 1 + 0.38·(-code/7).
        assert!((chaos_amp_factor(0) - 1.0).abs() < 1e-6);
        assert!((chaos_amp_factor(7) - 0.62).abs() < 1e-6);
        assert!((chaos_amp_factor(3) - (1.0 - 0.38 * 3.0 / 7.0)).abs() < 1e-6);
        // Round-trip with the decoder's chaos modification: pre-divided
        // amp × decoder factor ≈ measured amp.
        for code in 0..=7u8 {
            let measured = 0.5f32;
            let sent = measured / chaos_amp_factor(code);
            let decoded = sent * chaos_amp_factor(code);
            assert!((decoded - measured).abs() < 1e-6);
        }
    }

    #[test]
    fn coherence_measures_correlation() {
        // Identical spectra → coherence 1; orthogonal (Re vs Im) → the
        // cross terms cancel per-bin only if constructed so; use the
        // direct accumulator forms.
        let full = BandStats {
            ex: 4.0,
            ez: 1.0,
            cr: 2.0,
            ci: 0.0,
        };
        assert!((full.coherence() - 1.0).abs() < 1e-6);
        let none = BandStats {
            ex: 4.0,
            ez: 1.0,
            cr: 0.0,
            ci: 0.0,
        };
        assert!(none.coherence() < 1e-6);
        let silent = BandStats::default();
        assert!((silent.coherence() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn quantise_angle_wraps() {
        // +1.0 (= +π) wraps to the -1.0 slot (code 32) — same angle.
        assert_eq!(quantise_angle(1.0), 32);
        // 2.0 ≡ 0.0.
        assert_eq!(quantise_angle(2.0), 0);
        // -1.03125 ≡ 0.96875 (code 31).
        assert_eq!(quantise_angle(-1.0 - 1.0 / 32.0), 31);
        assert_eq!(quantise_angle(f32::NAN), 0);
    }

    /// Windowed-MDCT a 3-block excerpt of a time signal (256-sample
    /// hop), mirroring the encoder DSP.
    fn mdct_blocks(signal: &dyn Fn(usize) -> f32, nblocks: usize) -> Vec<[f32; N_COEFFS]> {
        let mut out = Vec::with_capacity(nblocks);
        for blk in 0..nblocks {
            let mut buf = [0.0f32; 512];
            for (n, slot) in buf.iter_mut().enumerate() {
                *slot = signal(blk * 256 + n);
            }
            let mut win = [0.0f32; 512];
            for n in 0..256 {
                win[n] = buf[n] * WINDOW[n];
                win[511 - n] = buf[511 - n] * WINDOW[n];
            }
            let mut c = [0.0f32; N_COEFFS];
            mdct_512(&win, &mut c);
            out.push(c);
        }
        out
    }

    /// The property the whole encoder model rests on: running a
    /// region-restricted MDCT block through the §E.3.5.5.1 analysis
    /// (with its true neighbours) and back through the §E.3.5.5.4
    /// synthesis with amplitude 1 / angle 0 reproduces the original
    /// MDCT coefficients in the region interior. This is the spec's
    /// "non-aliased carrier" design intent; the test pins our
    /// primitives to it (and pins the identity's scale factor at 1).
    #[test]
    fn analysis_synthesis_identity_amp1_angle0() {
        let geom = geom_default();
        // Multi-tone with energy well inside the region (bins ~60-200 of
        // a 48 kHz 512-point MDCT: 5.6-18.8 kHz).
        let sig = |n: usize| -> f32 {
            let t = n as f32;
            0.4 * (2.0 * std::f32::consts::PI * 0.24 * t).sin()
                + 0.3 * (2.0 * std::f32::consts::PI * 0.31 * t).sin()
                + 0.2 * (2.0 * std::f32::consts::PI * 0.47 * t + 0.7).sin()
        };
        let blocks = mdct_blocks(&sig, 3);
        let prev = region_restrict(&blocks[0], &geom);
        let curr = region_restrict(&blocks[1], &geom);
        let next = region_restrict(&blocks[2], &geom);

        let z = reconstruct_carrier(&prev, &curr, &next);
        let amp = vec![1.0f32; geom.end_bin - geom.start_bin];
        let ang = vec![0.0f32; geom.end_bin - geom.start_bin];
        let mut out = [0.0f32; 256];
        generate_channel_coeffs(&z.zr, &z.zi, &amp, &ang, geom.start_bin, 512, &mut out);

        // Compare in the region interior (skip 12 bins at each edge —
        // the region restriction itself is a brick-wall filter whose
        // ringing lands at the edges).
        let lo = geom.start_bin + 12;
        let hi = geom.end_bin - 12;
        let mut err = 0.0f64;
        let mut ref_e = 0.0f64;
        for bin in lo..hi {
            let d = (out[bin] - curr[bin]) as f64;
            err += d * d;
            ref_e += (curr[bin] as f64) * (curr[bin] as f64);
        }
        assert!(ref_e > 1e-6, "test signal has no in-region energy");
        let snr_db = 10.0 * (ref_e / err.max(1e-30)).log10();
        assert!(
            snr_db > 40.0,
            "analysis→synthesis identity too lossy: {snr_db:.1} dB"
        );
    }

    #[test]
    fn band_cross_stats_measures_scale_and_phase() {
        let geom = geom_default();
        let sig = |n: usize| -> f32 {
            let t = n as f32;
            0.5 * (2.0 * std::f32::consts::PI * 0.30 * t).sin()
                + 0.3 * (2.0 * std::f32::consts::PI * 0.55 * t).sin()
        };
        let blocks = mdct_blocks(&sig, 3);
        let prev = region_restrict(&blocks[0], &geom);
        let curr = region_restrict(&blocks[1], &geom);
        let next = region_restrict(&blocks[2], &geom);
        // Channel = 0.5 × carrier → amp 0.5, angle 0 in every band with
        // energy.
        let half = |b: &[f32; 256]| -> [f32; 256] {
            let mut o = *b;
            for v in o.iter_mut() {
                *v *= 0.5;
            }
            o
        };
        let z = reconstruct_carrier(&prev, &curr, &next);
        let x = reconstruct_carrier(&half(&prev), &half(&curr), &half(&next));
        let mut stats = vec![BandStats::default(); geom.necplbnd];
        band_cross_stats(&x, &z, &geom, &mut stats);
        for (bnd, s) in stats.iter().enumerate() {
            if s.ez < 1e-4 {
                continue; // silent band
            }
            let amp = s.amp();
            assert!((amp - 0.5).abs() < 0.01, "band {bnd}: amp {amp} != 0.5");
            let ang = s.angle_units();
            assert!(ang.abs() < 0.01, "band {bnd}: angle {ang} != 0");
        }
    }

    #[test]
    fn carrier_band_gains_cover_loudest_channel() {
        // ch0 quiet in band 1, ch1 4× energy there → gain 2 in band 1.
        let e0 = vec![1.0f64, 1.0, 1.0];
        let e1 = vec![0.25f64, 4.0, 1.0];
        let gains = carrier_band_gains(&[e0, e1], 3);
        assert!((gains[0] - 1.0).abs() < 1e-6); // never scales down
        assert!((gains[1] - 2.0).abs() < 1e-6);
        assert!((gains[2] - 1.0).abs() < 1e-6);
        // Silent carrier source → gain stays 1 (nothing to scale).
        let gains = carrier_band_gains(&[vec![0.0], vec![9.0]], 1);
        assert!((gains[0] - 1.0).abs() < 1e-6);
        // Clamp at 32.
        let gains = carrier_band_gains(&[vec![1e-9], vec![1.0]], 1);
        assert!((gains[0] - 32.0).abs() < 1e-3);
    }

    #[test]
    fn build_carrier_scales_per_band_and_zeroes_outside() {
        let geom = geom_default();
        let mut mdct = [0.0f32; N_COEFFS];
        for (k, v) in mdct.iter_mut().enumerate() {
            *v = (k as f32) / 256.0;
        }
        let mut gains = vec![1.0f32; geom.necplbnd];
        gains[0] = 3.0;
        let carrier = build_carrier_block(&mdct, &gains, &geom);
        // Outside the region: zero.
        assert_eq!(carrier[geom.start_bin - 1], 0.0);
        assert_eq!(carrier[geom.end_bin.min(255)], 0.0);
        // First band scaled by 3.
        assert!((carrier[geom.start_bin] - mdct[geom.start_bin] * 3.0).abs() < 1e-6);
        // A later band keeps gain 1.
        let bin2 = geom.start_bin + geom.band_bins[0];
        assert!((carrier[bin2] - mdct[bin2]).abs() < 1e-6);
    }
}

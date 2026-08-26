//! Encoder-side Spectral Extension (SPX) — ATSC A/52:2018 Annex E
//! §E.2.3.3.1-13 (bitstream syntax) + §E.3.6 (decode model the encoder
//! must invert).
//!
//! SPX is E-AC-3's parametric high-frequency reconstruction: the
//! encoder stops coding transform coefficients at the SPX begin
//! frequency and instead sends, per channel per band, a coordinate
//! that scales a *translated copy* of the channel's own low-frequency
//! spectrum (optionally blended with noise) so that — per §3.6.4.3 —
//!
//! > "the banded energy of the synthesized high frequency transform
//! > coefficients should match the banded energy of the high
//! > frequency transform coefficients of the original signal."
//!
//! That sentence is the whole encoder contract. This module provides
//! the pieces the bitstream emitter (`eac3::encoder`) needs:
//!
//! * [`SpxParams`] — the user-facing configuration (begin / end / copy
//!   start frequency codes, noise-blend offset, explicit-vs-default
//!   band structure).
//! * [`SpxGeometry`] — the derived sub-band / band / transform-
//!   coefficient geometry (§E.2.3.3.5-8 + Table E3.13), validated the
//!   same way the decoder validates it.
//! * [`band_coord_targets`] — the §3.6.4.3 energy-matching coordinate
//!   for each band: `spxco = rms(original HF band) /
//!   (rms(translated band) · 32)`, using the *same* translation plan
//!   the decoder executes ([`crate::audblk::spx_translation_plan`]).
//! * [`quantise_coord`] / [`choose_mstrspxco`] — the §E.2.3.3.11-13
//!   exponent / mantissa / master-coordinate quantiser, with
//!   [`decode_coord`] as the exact decoder-side inverse for tests.

use crate::audblk::{spx_translation_plan, N_COEFFS, SPX_ATTEN_TABLE};
use oxideav_core::{Error, Result};

/// Encoder-facing SPX configuration.
///
/// Field ranges mirror the §E.2.3.3.4-6 / §E.2.3.3.10 bitstream
/// fields; [`SpxGeometry::derive`] validates the combination.
#[derive(Clone, Copy, Debug)]
pub struct SpxParams {
    /// `spxbegf` (§E.2.3.3.5, 3 bits, 0..=7) — SPX begin frequency
    /// code. `spx_begin_subbnd = spxbegf + 2` for `spxbegf < 6`, else
    /// `spxbegf·2 − 3`; coded bandwidth ends (and the synthesized
    /// region starts) at tc# `25 + 12·spx_begin_subbnd`.
    pub spxbegf: u8,
    /// `spxendf` (§E.2.3.3.6, 3 bits, 0..=7) — SPX end frequency code.
    /// `spx_end_subbnd = spxendf + 5` for `spxendf < 3`, else
    /// `spxendf·2 + 3` (7 → 17 → tc# 229, the Table E3.13 top).
    pub spxendf: u8,
    /// `spxstrtf` (§E.2.3.3.4, 2 bits, 0..=3) — translation copy start
    /// frequency code; the copy region begins at tc# `25 + 12·spxstrtf`.
    pub spxstrtf: u8,
    /// `spxblnd` (§E.2.3.3.10, 5 bits, 0..=31) — noise blend offset.
    /// The decoder computes `nratio = (band centre)/(spx end tc) −
    /// spxblnd/32` (clamped to [0, 1]); larger values mean less noise.
    /// The default 24 keeps the lower extension bands translation-
    /// dominated while admitting some §E.3.6.4.2.4 noise fill in the
    /// top bands.
    pub spxblnd: u8,
    /// When true the encoder re-evaluates `spxstrtf` per frame: it
    /// scores every valid copy-start candidate (0..=3, copy region
    /// non-empty) by the total coordinate saturation the frame's
    /// spectra would incur (a §3.6.4.3 target above the representable
    /// `0.875` ceiling means the translated source region is too weak
    /// to reach the original band energy) and picks the least-saturated
    /// one, preferring lower codes (larger copy regions) on ties. The
    /// configured [`SpxParams::spxstrtf`] is ignored when set. This
    /// matters for spectra with a hole just above the first copy
    /// sub-band: a fixed copy start would translate near-silence into
    /// a loud extension band and pin its coordinate at the ceiling.
    pub adaptive_copy_start: bool,
    /// Optional §3.6.4.2.3 spectral-extension attenuation: `Some(code)`
    /// signals `spxattene = 1` in audfrm with `chinspxatten[ch] = 1` +
    /// `spxattencod[ch] = code` (0..=31, a Table E3.14 row) for every
    /// fbw channel. The decoder then applies the 5-tap symmetric notch
    /// filter at the baseband/extension border and at every
    /// translation-copy wrap point; the encoder folds the notch into
    /// its §3.6.4.3 energy computation so the coordinates compensate
    /// (band energy still matches). Larger codes attenuate harder.
    pub atten_code: Option<u8>,
    /// When true the encoder emits `spxbndstrce = 1` with an explicit
    /// band structure (identical content to the Table E2.11 default);
    /// when false it emits `spxbndstrce = 0` and relies on the
    /// decoder's default banding. Both decode identically — the flag
    /// exists so tests can pin the two syntax paths against each other.
    pub explicit_band_structure: bool,
    /// Mixed per-channel SPX (§E.2.3.3.3 `chinspx[ch]`): bit `ch` set
    /// means fbw channel `ch` is in spectral extension. `None` (the
    /// default) puts every fbw channel in SPX. Channels NOT in SPX are
    /// waveform-coded to their full chbwcod-derived bandwidth (they
    /// emit `chinspx[ch] = 0`, keep their `chbwcod`, and carry no SPX
    /// coordinates); the SNR tuner budgets each channel at its own
    /// coded bandwidth. Constraints: at least one bit for a coded
    /// channel must be set, and mono (`acmod == 0x1`) cannot exclude
    /// its only channel (the spec makes `chinspx[0]` implicit there).
    pub channel_mask: Option<u8>,
}

impl Default for SpxParams {
    fn default() -> Self {
        SpxParams {
            // begin sub-band 7 → coded bandwidth ends at tc# 109
            // (≈ 10.2 kHz at 48 kHz); extension runs to tc# 229
            // (≈ 21.5 kHz) — a conventional low-rate operating point.
            spxbegf: 5,
            spxendf: 7,
            spxstrtf: 0,
            spxblnd: 24,
            adaptive_copy_start: false,
            atten_code: None,
            explicit_band_structure: false,
            channel_mask: None,
        }
    }
}

/// Derived SPX geometry (§E.2.3.3.5-8 + Table E3.13), the encoder-side
/// mirror of the decoder's strategy-parse state.
#[derive(Clone, Debug)]
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub struct SpxGeometry {
    /// First SPX sub-band (`spx_begin_subbnd`).
    pub begin_subbnd: usize,
    /// One-past-last SPX sub-band (`spx_end_subbnd`).
    pub end_subbnd: usize,
    /// First synthesized tc# (= coded-bandwidth end for SPX channels,
    /// §E.3.3.3 `endmant = spxbandtable[spx_begin_subbnd]`).
    pub begin_tc: usize,
    /// One-past-last synthesized tc#.
    pub end_tc: usize,
    /// Translation copy region start tc# (`spxbandtable[spxstrtf]`).
    pub copy_start_tc: usize,
    /// Band structure over absolute sub-bands (`spxbndstrc[]`,
    /// §E.2.3.3.8) — `true` merges the sub-band into the previous band.
    pub bndstrc: [bool; 18],
    /// Number of coordinate bands (`nspxbnds`).
    pub nbnds: usize,
    /// Per-band bin counts (`spxbndsztab[]`).
    pub bndsztab: [usize; 18],
}

/// Table E3.13 — lowest tc# of SPX sub-band `subbnd` (`25 + 12·s`).
#[inline]
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn spx_bandtable(subbnd: usize) -> usize {
    25 + 12 * subbnd
}

impl SpxGeometry {
    /// Derive and validate the geometry for a parameter set, using the
    /// given band structure (pass
    /// [`crate::eac3::dsp::DEFAULT_SPX_BNDSTRC`] for the Table E2.11
    /// default the decoder assumes when `spxbndstrce == 0`).
    pub fn derive(params: &SpxParams, bndstrc: &[bool; 18]) -> Result<SpxGeometry> {
        if params.spxbegf > 7 || params.spxendf > 7 || params.spxstrtf > 3 || params.spxblnd > 31 {
            return Err(Error::invalid(
                "spxenc: field out of range (spxbegf/spxendf 0..=7, spxstrtf 0..=3, spxblnd 0..=31)",
            ));
        }
        if params.atten_code.is_some_and(|c| c > 31) {
            return Err(Error::invalid(
                "spxenc: spxattencod out of range (0..=31, Table E3.14)",
            ));
        }
        // §E.2.3.3.5-6 sub-band derivations (same arms as the decoder).
        let begin_subbnd = if params.spxbegf < 6 {
            params.spxbegf as usize + 2
        } else {
            params.spxbegf as usize * 2 - 3
        };
        let end_subbnd = if params.spxendf < 3 {
            params.spxendf as usize + 5
        } else {
            params.spxendf as usize * 2 + 3
        };
        if end_subbnd <= begin_subbnd || end_subbnd > 17 {
            return Err(Error::invalid(
                "spxenc: SPX sub-band range invalid (end <= begin or > 17)",
            ));
        }
        let begin_tc = spx_bandtable(begin_subbnd);
        let end_tc = spx_bandtable(end_subbnd);
        let copy_start_tc = spx_bandtable(params.spxstrtf as usize);
        // The decoder rejects an empty copy region (`copyend <=
        // copystart`); require it up front so the translation plan is
        // well-defined.
        if copy_start_tc >= begin_tc {
            return Err(Error::invalid(
                "spxenc: copy region empty (spxstrtf sub-band >= spx begin sub-band)",
            ));
        }
        // §E.3.6.2 band sizing — identical loop to the decoder's.
        let mut nbnds = 1usize;
        let mut bndsztab = [0usize; 18];
        bndsztab[0] = 12;
        for bnd in (begin_subbnd + 1)..end_subbnd {
            if !bndstrc[bnd] {
                bndsztab[nbnds] = 12;
                nbnds += 1;
            } else {
                bndsztab[nbnds - 1] += 12;
            }
        }
        Ok(SpxGeometry {
            begin_subbnd,
            end_subbnd,
            begin_tc,
            end_tc,
            copy_start_tc,
            bndstrc: *bndstrc,
            nbnds,
            bndsztab,
        })
    }
}

/// §3.6.4.3 energy-matching coordinate targets for one channel-block.
///
/// `coeffs` is the channel's full-bandwidth MDCT spectrum (the encoder
/// keeps the true HF bins even though it won't code them). For each
/// SPX band this computes
///
/// ```text
/// spxco[bnd] = rms(original coeffs over band) /
///              (rms(translated coeffs over band) · 32)
/// ```
///
/// using the exact decoder translation plan, so that after the decoder
/// runs translation → noise blend (which preserves band energy:
/// `sblend² + nblend² = 1` and the noise is scaled to the translated
/// band RMS) → `·spxco·32`, the synthesized band energy equals the
/// original band energy.
///
/// Bands whose translated energy is (near) zero get coordinate 0 —
/// there is nothing to scale; the decoder will synthesize silence
/// (plus noise-blend fill scaled by the same zero RMS).
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn band_coord_targets(coeffs: &[f32; N_COEFFS], geom: &SpxGeometry) -> [f32; 18] {
    band_coord_targets_span(&[coeffs], geom)
}

/// Multi-block variant of [`band_coord_targets`]: accumulates the
/// original / translated band energies across all supplied blocks
/// before forming the ratio. The encoder signals one coordinate set
/// per refresh span (`spxcoe[ch] == 1` block; the following
/// `spxcoe == 0` blocks reuse it, §E.2.3.3.9), so the coordinate must
/// energy-match the *whole span*, not just the refresh block.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn band_coord_targets_span(blocks: &[&[f32; N_COEFFS]], geom: &SpxGeometry) -> [f32; 18] {
    band_coord_targets_span_atten(blocks, geom, None)
}

/// [`band_coord_targets_span`] with the §3.6.4.2.3 attenuation folded
/// in. The decoder computes the banded RMS (which scales both the
/// noise fill and — through the coordinate — the final output) *after*
/// applying the border/wrap notch filters to the translated
/// coefficients, so when attenuation is signalled the encoder must
/// derive its coordinates from the *notched* translated energy or every
/// filtered band would come out low by the notch loss. Only the
/// extension-side taps affect the translated energy; the two
/// coded-region bins below the border are attenuated in the decoder's
/// output but are not part of any SPX band.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn band_coord_targets_span_atten(
    blocks: &[&[f32; N_COEFFS]],
    geom: &SpxGeometry,
    atten_code: Option<u8>,
) -> [f32; 18] {
    let (copy_map, wrapflag) = spx_translation_plan(
        geom.copy_start_tc,
        geom.begin_tc,
        geom.nbnds,
        &geom.bndsztab,
    );
    let total: usize = geom.bndsztab[..geom.nbnds].iter().sum();
    // Extension-relative notch gain profile (unity when no attenuation):
    // the 5-tap kernel [T0, T1, T2, T1, T0] sits centred one bin below
    // each border/wrap bin (filter start = site - 2), so extension bins
    // site+0/+1/+2 receive T2/T1/T0. Sites: the baseband/extension
    // border (offset 0, unconditional) and every wrapped band start.
    let mut gain = vec![1.0f64; total];
    if let Some(code) = atten_code {
        let row = SPX_ATTEN_TABLE[(code & 0x1F) as usize];
        let taps = [
            row[0] as f64,
            row[1] as f64,
            row[2] as f64,
            row[1] as f64,
            row[0] as f64,
        ];
        let mut apply = |site: usize| {
            for (i, tap) in taps.iter().enumerate() {
                // Filter start is 2 bins below the site; extension-side
                // indices are site - 2 + i (negative → coded region).
                let idx = site as isize - 2 + i as isize;
                if idx >= 0 && (idx as usize) < total {
                    gain[idx as usize] *= *tap;
                }
            }
        };
        apply(0); // baseband / extension border (§3.6.4.2.3)
        let mut band_start = 0usize;
        for bnd in 0..geom.nbnds {
            if bnd > 0 && wrapflag[bnd] {
                apply(band_start);
            }
            band_start += geom.bndsztab[bnd];
        }
    }
    let mut out = [0.0f32; 18];
    let mut offset = 0usize;
    for bnd in 0..geom.nbnds {
        let bandsize = geom.bndsztab[bnd];
        let mut e_orig = 0.0f64;
        let mut e_trans = 0.0f64;
        for coeffs in blocks {
            for i in 0..bandsize {
                let tc = geom.begin_tc + offset + i;
                if tc < N_COEFFS {
                    let v = coeffs[tc] as f64;
                    e_orig += v * v;
                }
                let src = copy_map[offset + i];
                if src < N_COEFFS {
                    let v = coeffs[src] as f64 * gain[offset + i];
                    e_trans += v * v;
                }
            }
        }
        out[bnd] = if e_trans > f64::MIN_POSITIVE && e_orig > f64::MIN_POSITIVE {
            ((e_orig / e_trans).sqrt() / 32.0) as f32
        } else {
            0.0
        };
        offset += bandsize;
    }
    out
}

/// Decoder-side coordinate reconstruction (§E.2.3.3.11-13 pseudo-code)
/// — the exact inverse of [`quantise_coord`], used by the emitter (to
/// know the value the decoder will apply) and by the quantiser tests.
///
/// ```text
/// temp = (spxcoexp == 15) ? spxcomant / 4 : (spxcomant + 4) / 8
/// spxco = temp >> (spxcoexp + 3·mstrspxco)
/// ```
#[inline]
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn decode_coord(exp: u8, mant: u8, mstr: u8) -> f32 {
    let temp = if exp == 15 {
        mant as f32 / 4.0
    } else {
        (mant as f32 + 4.0) / 8.0
    };
    temp * 2f32.powi(-(exp as i32 + 3 * mstr as i32))
}

/// Quantise one §3.6.4.3 coordinate target into the (`spxcoexp`,
/// `spxcomant`) pair for a channel whose master coordinate is `mstr`.
///
/// The normal-form representation covers `temp ∈ {4..7}/8` (the
/// implicit-msb form: values in [0.5, 0.875] stepped by 1/8) shifted by
/// `2^-(exp + 3·mstr)` for `exp ∈ 0..=14`; `exp == 15` is the denormal
/// escape (`temp = mant/4`, admitting exact zero). Values too large to
/// represent saturate at (0, 3) — `0.875·2^(−3·mstr)` — and values too
/// small collapse into the denormal form (rounding to zero when even
/// that underflows).
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn quantise_coord(target: f32, mstr: u8) -> (u8, u8) {
    let base_shift = 3 * mstr as i32;
    if !target.is_finite() || target <= 0.0 {
        return (15, 0);
    }
    // Normalise: target = m · 2^-s with m ∈ [0.5, 1).
    let mut m = target;
    let mut s = 0i32;
    while m >= 1.0 {
        m *= 0.5;
        s -= 1;
    }
    while m < 0.5 && s < 64 {
        m *= 2.0;
        s += 1;
    }
    let exp_needed = s - base_shift;
    if exp_needed < 0 {
        // Larger than the representable maximum for this mstr —
        // saturate at temp = 7/8, exp = 0.
        return (0, 3);
    }
    if exp_needed >= 15 {
        // Denormal escape: spxco = mant/4 · 2^-(15 + 3·mstr).
        let scaled = target * 4.0 * 2f32.powi(15 + base_shift);
        let mant = scaled.round().clamp(0.0, 3.0) as u8;
        return (15, mant);
    }
    // Normal form: temp = (mant + 4)/8 ∈ {0.5, 0.625, 0.75, 0.875}.
    let rounded = (m * 8.0).round() as i32; // 4..=8
    if rounded >= 8 {
        // m ≈ 1.0 rounds past the top of the mantissa range; the value
        // 1.0·2^-exp equals 0.5·2^-(exp-1), representable one exponent
        // up (or saturating at exp 0).
        if exp_needed == 0 {
            return (0, 3);
        }
        return ((exp_needed - 1) as u8, 0);
    }
    ((exp_needed) as u8, (rounded - 4).clamp(0, 3) as u8)
}

/// Choose the per-channel `mstrspxco` (§E.2.3.3.11) for a set of band
/// coordinate targets.
///
/// `mstrspxco` adds `3·mstr` to every band exponent, extending reach
/// toward *smaller* coordinates (up to 54 dB) at the cost of lowering
/// the representable maximum (`0.875·2^-3·mstr`). Pick the largest
/// `mstr ∈ 0..=3` that (a) the largest target still fits under without
/// saturating and (b) actually helps the smallest non-zero target
/// escape the low-precision `exp == 15` denormal form.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn choose_mstrspxco(targets: &[f32]) -> u8 {
    let mut max_t = 0.0f32;
    let mut min_shift: Option<i32> = None; // shift s of the smallest non-zero target
    let mut max_shift: i32 = 0; // shift s of the largest target
    for &t in targets {
        if t > 0.0 && t.is_finite() {
            let s = -t.log2().ceil() as i32; // t ∈ (2^-(s+1), 2^-s]
            if t > max_t {
                max_t = t;
                max_shift = s.max(0);
            }
            min_shift = Some(min_shift.map_or(s, |m: i32| m.max(s)));
        }
    }
    let Some(min_shift) = min_shift else {
        return 0; // all-zero coordinates — mstr is irrelevant
    };
    // (b) how much extra shift would the smallest target like, to land
    // its exponent at <= 14 (normal form)? ceil((min_shift - 14)/3).
    let wanted = ((min_shift - 14) + 2) / 3;
    // (a) the largest target must keep exp_needed >= 0: 3·mstr <= max_shift.
    let cap = max_shift / 3;
    wanted.clamp(0, 3).min(cap.max(0)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eac3::dsp::DEFAULT_SPX_BNDSTRC;

    // ---- geometry (§E.2.3.3.5-8 / Table E3.13) ----

    #[test]
    fn geometry_default_params() {
        let p = SpxParams::default();
        let g = SpxGeometry::derive(&p, &DEFAULT_SPX_BNDSTRC).expect("default geometry");
        // spxbegf=5 < 6 → begin_subbnd 7 → tc 109.
        assert_eq!(g.begin_subbnd, 7);
        assert_eq!(g.begin_tc, 109);
        // spxendf=7 ≥ 3 → end_subbnd 17 → tc 229 (Table E3.13 top).
        assert_eq!(g.end_subbnd, 17);
        assert_eq!(g.end_tc, 229);
        assert_eq!(g.copy_start_tc, 25);
        // Default banding merges sub-bands 8/10/12/14/16 into their
        // predecessors: sub-bands 7..17 = 10 sub-bands → 5 bands of 24.
        assert_eq!(g.nbnds, 5);
        assert_eq!(&g.bndsztab[..5], &[24, 24, 24, 24, 24]);
        assert_eq!(g.bndsztab[..g.nbnds].iter().sum::<usize>(), 229 - 109);
    }

    #[test]
    fn geometry_subband_derivation_arms() {
        // spxbegf ≥ 6 arm: 6 → 9, 7 → 11.
        for (begf, want) in [(6u8, 9usize), (7, 11)] {
            let p = SpxParams {
                spxbegf: begf,
                spxendf: 7,
                ..SpxParams::default()
            };
            let g = SpxGeometry::derive(&p, &DEFAULT_SPX_BNDSTRC).unwrap();
            assert_eq!(g.begin_subbnd, want, "spxbegf={begf}");
        }
        // spxendf < 3 arm: 0 → 5, 2 → 7; ≥ 3 arm: 3 → 9.
        for (endf, want) in [(0u8, 5usize), (2, 7), (3, 9)] {
            let p = SpxParams {
                spxbegf: 0,
                spxendf: endf,
                ..SpxParams::default()
            };
            let g = SpxGeometry::derive(&p, &DEFAULT_SPX_BNDSTRC).unwrap();
            assert_eq!(g.end_subbnd, want, "spxendf={endf}");
        }
    }

    #[test]
    fn geometry_rejects_invalid_combinations() {
        // Inverted range: begf=7 (sub-band 11) with endf=0 (sub-band 5).
        let p = SpxParams {
            spxbegf: 7,
            spxendf: 0,
            ..SpxParams::default()
        };
        assert!(SpxGeometry::derive(&p, &DEFAULT_SPX_BNDSTRC).is_err());
        // Empty copy region: strtf=3 (tc 61) with begf=0 (begin tc 49).
        let p = SpxParams {
            spxbegf: 0,
            spxendf: 7,
            spxstrtf: 3,
            ..SpxParams::default()
        };
        assert!(SpxGeometry::derive(&p, &DEFAULT_SPX_BNDSTRC).is_err());
        // Out-of-range raw fields.
        let p = SpxParams {
            spxbegf: 8,
            ..SpxParams::default()
        };
        assert!(SpxGeometry::derive(&p, &DEFAULT_SPX_BNDSTRC).is_err());
        let p = SpxParams {
            spxblnd: 32,
            ..SpxParams::default()
        };
        assert!(SpxGeometry::derive(&p, &DEFAULT_SPX_BNDSTRC).is_err());
    }

    #[test]
    fn geometry_explicit_unmerged_structure() {
        // All-false structure: every sub-band its own 12-bin band.
        let p = SpxParams {
            spxbegf: 5,
            spxendf: 7,
            ..SpxParams::default()
        };
        let g = SpxGeometry::derive(&p, &[false; 18]).unwrap();
        assert_eq!(g.nbnds, 10);
        assert!(g.bndsztab[..10].iter().all(|&s| s == 12));
    }

    // ---- coordinate quantiser (§E.2.3.3.11-13) ----

    #[test]
    fn quantise_coord_round_trips_through_decoder_formula() {
        // Sweep coordinate magnitudes across the full normal range for
        // each mstr; the decoded value must sit within the quantiser's
        // worst-case relative error (mantissa step 1/8 over m ∈
        // [0.5, 1) → ≤ 1/16 / 0.5 = 12.5%).
        for mstr in 0..=3u8 {
            let mut c = 0.875f32 * 2f32.powi(-(3 * mstr as i32));
            while c > 2f32.powi(-(14 + 3 * mstr as i32)) {
                let (exp, mant) = quantise_coord(c, mstr);
                let dec = decode_coord(exp, mant, mstr);
                let rel = (dec - c).abs() / c;
                assert!(
                    rel <= 0.126,
                    "mstr={mstr} c={c:e}: decoded {dec:e} rel err {rel:.3}"
                );
                c *= 0.83; // irregular step to hit varied mantissas
            }
        }
    }

    #[test]
    fn quantise_coord_edge_cases() {
        // Zero / negative / non-finite → exact zero via the exp-15 form.
        for bad in [0.0f32, -1.0, f32::NAN, f32::INFINITY] {
            let (exp, mant) = quantise_coord(bad, 0);
            assert_eq!((exp, mant), (15, 0));
            assert_eq!(decode_coord(exp, mant, 0), 0.0);
        }
        // Oversized target saturates at the representable maximum.
        let (exp, mant) = quantise_coord(3.0, 0);
        assert_eq!((exp, mant), (0, 3));
        assert!((decode_coord(0, 3, 0) - 0.875).abs() < 1e-6);
        // Tiny target lands in the denormal exp==15 form, not zero.
        // (Smallest non-zero denormal at mstr=0 is 1/4·2^-15 ≈ 7.6e-6;
        // pick a value above half that step so it rounds to mant 1.)
        let tiny = 6.0e-6f32; // ~2^-17.3
        let (exp, mant) = quantise_coord(tiny, 0);
        assert_eq!(exp, 15);
        assert!(mant > 0, "denormal form must retain a non-zero mantissa");
        let dec = decode_coord(exp, mant, 0);
        assert!((dec - tiny).abs() / tiny < 0.5, "coarse but non-zero");
        // A value that underflows even the denormal form rounds to 0.
        let (exp, mant) = quantise_coord(1e-9, 0);
        assert_eq!((exp, mant), (15, 0));
        // m ≈ 1.0 rounding carry: 0.99·2^-4 rounds to mant 8 → one
        // exponent up with mant 0 (temp 0.5): 0.5·2^-3 = 0.0625.
        let c = 0.99f32 / 16.0;
        let (exp, mant) = quantise_coord(c, 0);
        assert_eq!((exp, mant), (3, 0));
        let dec = decode_coord(exp, mant, 0);
        assert!((dec - c).abs() / c < 0.02);
    }

    #[test]
    fn choose_mstrspxco_balances_range() {
        // Ordinary coordinates (~1/32 scale) need no master shift.
        assert_eq!(choose_mstrspxco(&[0.03, 0.01, 0.005]), 0);
        // All-zero → 0.
        assert_eq!(choose_mstrspxco(&[0.0, 0.0]), 0);
        // Very small coordinates want shift — but only as much as the
        // largest coordinate tolerates. All-tiny set: full shift.
        let m = choose_mstrspxco(&[2e-6, 1e-6]);
        assert!(m >= 1, "tiny coordinates should engage mstrspxco, got {m}");
        // A large coordinate caps the shift at 0 regardless of small
        // companions (saturation would cost more than denormal loss).
        assert_eq!(choose_mstrspxco(&[0.6, 1e-6]), 0);
    }

    #[test]
    fn choose_mstrspxco_never_saturates_largest_target() {
        // Whatever mstr is chosen, quantising the largest target must
        // not hit the (0, 3) saturation clamp unless the target really
        // exceeds 0.875.
        for targets in [
            &[0.4f32, 1e-5, 3e-6][..],
            &[0.05, 4e-6][..],
            &[0.8, 0.2][..],
            &[9e-4, 2e-7][..],
        ] {
            let mstr = choose_mstrspxco(targets);
            let max = targets.iter().cloned().fold(0.0f32, f32::max);
            let (exp, mant) = quantise_coord(max, mstr);
            let dec = decode_coord(exp, mant, mstr);
            let rel = (dec - max).abs() / max;
            assert!(
                rel <= 0.126,
                "mstr={mstr} saturated the largest target {max:e} → {dec:e}"
            );
        }
    }

    // ---- §3.6.4.2.3 attenuation folding ----

    #[test]
    fn atten_fold_raises_border_band_coordinate() {
        // Uniform copy region + uniform HF: without attenuation every
        // band's target is (B/A)/32. With attenuation, band 0's first
        // three translated bins are notched by [T2, T1, T0], so its
        // translated energy drops by exactly
        //   (T2² + T1² + T0² + (n-3)) / n
        // and the target must rise by the inverse square root. Bands
        // that neither border nor wrap are unchanged.
        let p = SpxParams::default(); // copy 25..109 (84 bins), spx 109..229
        let g = SpxGeometry::derive(&p, &DEFAULT_SPX_BNDSTRC).unwrap();
        let mut coeffs = [0.0f32; N_COEFFS];
        for tc in g.copy_start_tc..g.begin_tc {
            coeffs[tc] = 0.02;
        }
        for tc in g.begin_tc..g.end_tc {
            coeffs[tc] = 0.005;
        }
        let code = 14u8; // Table E3.14 row [0.5, 0.25, 0.125]
        let plain = band_coord_targets_span(&[&coeffs], &g);
        let atten = band_coord_targets_span_atten(&[&coeffs], &g, Some(code));
        let row = SPX_ATTEN_TABLE[code as usize];
        let n = g.bndsztab[0] as f64;
        let notched =
            ((row[2] as f64).powi(2) + (row[1] as f64).powi(2) + (row[0] as f64).powi(2) + n - 3.0)
                / n;
        let want = plain[0] as f64 / notched.sqrt();
        assert!(
            (atten[0] as f64 - want).abs() / want < 1e-4,
            "band 0: atten target {:.6e}, want {want:.6e}",
            atten[0]
        );
        // Band 1 starts at translated offset 24 with copy region 84
        // bins — no wrap yet — so it is unchanged.
        assert!(
            (atten[1] - plain[1]).abs() / plain[1] < 1e-6,
            "band 1 must be unaffected (no border, no wrap)"
        );
        // Band 3 (offset 72; 72 + 24 > 84 → pre-band wrap) is notched
        // at its start → its target must exceed the plain one.
        assert!(
            atten[3] > plain[3] * 1.001,
            "wrapped band 3 must compensate the wrap-site notch"
        );
    }

    #[test]
    fn geometry_rejects_bad_atten_code() {
        let p = SpxParams {
            atten_code: Some(32),
            ..SpxParams::default()
        };
        assert!(SpxGeometry::derive(&p, &DEFAULT_SPX_BNDSTRC).is_err());
        let p = SpxParams {
            atten_code: Some(31),
            ..SpxParams::default()
        };
        assert!(SpxGeometry::derive(&p, &DEFAULT_SPX_BNDSTRC).is_ok());
    }

    // ---- §3.6.4.3 energy-matching coordinate targets ----

    #[test]
    fn band_coord_targets_closed_form() {
        // Construct a spectrum where the copy region is a constant
        // amplitude A and the HF region a constant amplitude B: every
        // band's translated RMS is A, original RMS is B, so the target
        // is (B/A)/32 exactly.
        let p = SpxParams::default(); // begin tc 109, end tc 229, copy from 25
        let g = SpxGeometry::derive(&p, &DEFAULT_SPX_BNDSTRC).unwrap();
        let mut coeffs = [0.0f32; N_COEFFS];
        let a = 0.02f32;
        let b = 0.005f32;
        for tc in g.copy_start_tc..g.begin_tc {
            coeffs[tc] = a;
        }
        for tc in g.begin_tc..g.end_tc {
            coeffs[tc] = b;
        }
        let targets = band_coord_targets(&coeffs, &g);
        for bnd in 0..g.nbnds {
            let want = (b / a) / 32.0;
            let got = targets[bnd];
            assert!(
                (got - want).abs() / want < 1e-4,
                "band {bnd}: target {got:e}, want {want:e}"
            );
        }
        // Bands beyond nbnds stay zero.
        assert_eq!(targets[g.nbnds], 0.0);
    }

    #[test]
    fn band_coord_targets_zero_translated_energy() {
        // Silent copy region → coordinate 0 (nothing to scale).
        let p = SpxParams::default();
        let g = SpxGeometry::derive(&p, &DEFAULT_SPX_BNDSTRC).unwrap();
        let mut coeffs = [0.0f32; N_COEFFS];
        for tc in g.begin_tc..g.end_tc {
            coeffs[tc] = 0.01;
        }
        let targets = band_coord_targets(&coeffs, &g);
        for bnd in 0..g.nbnds {
            assert_eq!(targets[bnd], 0.0, "band {bnd}");
        }
    }

    #[test]
    fn band_coord_targets_use_decoder_translation_plan() {
        // A copy region smaller than the SPX region forces wraps; put
        // all the LF energy in the FIRST copy sub-band so bands whose
        // translated content wraps back to it get non-zero targets while
        // bands sourced from the silent tail would get zero if the wrap
        // walk diverged from the decoder's.
        //
        // begf=0 → begin sub-band 2 (tc 49); copy region [25, 49) = 24
        // bins; strtf=0. SPX region 49..229 with default banding.
        let p = SpxParams {
            spxbegf: 0,
            spxendf: 7,
            spxstrtf: 0,
            ..SpxParams::default()
        };
        let g = SpxGeometry::derive(&p, &DEFAULT_SPX_BNDSTRC).unwrap();
        let mut coeffs = [0.0f32; N_COEFFS];
        for tc in 25..49 {
            coeffs[tc] = 0.03; // uniform copy region
        }
        for tc in g.begin_tc..g.end_tc {
            coeffs[tc] = 0.006; // uniform HF
        }
        let targets = band_coord_targets(&coeffs, &g);
        // Uniform energy everywhere → every band's translated RMS is
        // 0.03 regardless of wrap positions → target (0.006/0.03)/32.
        let want = (0.006f32 / 0.03) / 32.0;
        for bnd in 0..g.nbnds {
            assert!(
                (targets[bnd] - want).abs() / want < 1e-4,
                "band {bnd}: {:.6e} vs {want:.6e}",
                targets[bnd]
            );
        }
    }
}

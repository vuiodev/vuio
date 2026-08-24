//! Low-power SBR aliasing detection and reduction — ISO/IEC 14496-3
//! §4.6.18.8.3 / §4.6.18.8.5.
//!
//! The low-power SBR tool processes real-valued subband signals, so
//! the per-subband gains applied by the §4.6.18.7 envelope adjuster
//! can introduce audible aliasing between adjacent QMF subbands. This
//! module implements the countermeasure:
//!
//! * **Aliasing degree** (§4.6.18.8.3 / Figure 4.53) — from the
//!   per-subband reflection coefficients
//!   [`crate::sbr_hf_gen::reflection_coefficient`], the `deg` vector
//!   marking low-band subband pairs whose spectral orientation makes
//!   gain steps alias.
//! * **Patched degree** — `degPatched`, the low-band degrees carried
//!   onto the SBR range through the §4.6.18.6 patch mapping (zero at
//!   every patch start and beyond the patch coverage).
//! * **Gain grouping** (Figure 4.54) — the per-envelope `FGroup`
//!   start/stop index pairs bracketing runs of aliasing-prone,
//!   sinusoid-free subbands.
//! * **Aliasing reduction** (§4.6.18.8.5) — the `GLimBoost → GA` gain
//!   re-calculation: per group, a target gain from the group energies,
//!   the `α(m)`-weighted blend, and the exact energy-restoring
//!   normalization.
//!
//! ## Provenance
//!
//! Every formula and branch is from the §4.6.18.8.3 / §4.6.18.8.5 text
//! and the Figure 4.53 / 4.54 flowcharts of the staged spec. No part
//! of this implementation is derived from any external decoder.

use crate::sbr_env_adjust::EPS0;
use crate::sbr_hf_gen::Patches;
use crate::{Error, Result};

/// §4.6.18.8.3 / Figure 4.53 — the aliasing degree `deg(k)` of every
/// low-band subband, from the reflection coefficients `ref(k)`
/// (`0 ≤ k < k0`). Entries 0 and 1 are always zero (the flowchart
/// starts at `k = 2` after forcing `ref(0) = 0`, `deg(1) = 0`).
#[must_use]
pub fn aliasing_degree(refl: &[f64]) -> Vec<f64> {
    let k0 = refl.len();
    let mut deg = vec![0.0f64; k0];
    let mut refl = refl.to_vec();
    if !refl.is_empty() {
        refl[0] = 0.0;
    }
    let mut k = 2usize;
    while k < k0 {
        deg[k] = 0.0;
        // Even subbands alias on a negative reflection, odd subbands
        // on a positive one; other orientations are alias-free.
        let sign = if k % 2 == 0 && refl[k] < 0.0 {
            1.0
        } else if k % 2 == 1 && refl[k] > 0.0 {
            -1.0
        } else {
            k += 1;
            continue;
        };
        if sign * refl[k - 1] < 0.0 {
            deg[k] = 1.0;
            if sign * refl[k - 2] > 0.0 {
                deg[k - 1] = 1.0 - refl[k - 1] * refl[k - 1];
            }
        } else if sign * refl[k - 2] > 0.0 {
            deg[k] = 1.0 - refl[k - 1] * refl[k - 1];
        }
        k += 1;
    }
    deg
}

/// §4.6.18.8.3 — `degPatched(k)` over the SBR range, `kx`-relative
/// (`m` entries): each patch carries the source subband's degree, the
/// first subband of every patch (`x == 0`) and the region beyond the
/// patch coverage are zero.
pub fn deg_patched(deg: &[f64], patches: &Patches, k_x: i32, m: i32) -> Result<Vec<f64>> {
    let m_cnt = usize::try_from(m).map_err(|_| Error::SbrFreqBandInvalid)?;
    if k_x < 0 {
        return Err(Error::SbrFreqBandInvalid);
    }
    let mut dp = vec![0.0f64; m_cnt];
    let mut k_off = 0usize;
    for (&start, &num) in patches.start.iter().zip(patches.num.iter()) {
        for x in 0..num {
            let rel = k_off + x;
            if rel >= m_cnt {
                break;
            }
            let p = start + x;
            dp[rel] = if x == 0 {
                0.0
            } else {
                deg.get(p).copied().ok_or(Error::SbrFreqBandInvalid)?
            };
        }
        k_off += num;
    }
    Ok(dp)
}

/// Figure 4.54 — the gain groups of one SBR envelope: `(start, stop)`
/// absolute-QMF-subband pairs (`stop` exclusive), bracketing runs
/// where the *next* subband boundary is aliasing-prone
/// (`degPatched(k+1) ≠ 0`) and no sinusoid is mapped.
///
/// `dp` is the `kx`-relative `degPatched` (length `M`), `s_mapped` the
/// envelope's `SMapped` row (length `M`).
#[must_use]
pub fn gain_groups(dp: &[f64], s_mapped: &[bool], k_x: i32) -> Vec<(usize, usize)> {
    let m_cnt = dp.len().min(s_mapped.len());
    let kx = k_x.max(0) as usize;
    let mut groups: Vec<(usize, usize)> = Vec::new();
    let mut open: Option<usize> = None;
    // k walks kx .. kx + M − 1 (exclusive), exactly the flowchart loop.
    for rel in 0..m_cnt.saturating_sub(1) {
        let k = kx + rel;
        if dp[rel + 1] != 0.0 && !s_mapped[rel] {
            if open.is_none() {
                open = Some(k);
            }
        } else if let Some(start) = open.take() {
            // Close the group: past the current subband when it is
            // sinusoid-free, before it otherwise.
            let stop = if s_mapped[rel] { k } else { k + 1 };
            groups.push((start, stop));
        }
    }
    if let Some(start) = open {
        groups.push((start, kx + m_cnt));
    }
    groups
}

/// §4.6.18.8.5 — recompute the limiter/boost gains `GLimBoost` of one
/// envelope into the aliasing-reduced `GA`, in place.
///
/// `g` is the envelope's `GLimBoost` row and `e_curr` its `ECurr` row
/// (both `kx`-relative, length `M`); `dp` the `kx`-relative
/// `degPatched`; `groups` the Figure 4.54 gain groups (absolute
/// subband indices). Subbands outside every group keep `GLimBoost`.
pub fn aliasing_reduction(
    g: &mut [f64],
    e_curr: &[f64],
    dp: &[f64],
    groups: &[(usize, usize)],
    k_x: i32,
) -> Result<()> {
    let m_cnt = g.len();
    if e_curr.len() != m_cnt || dp.len() != m_cnt || k_x < 0 {
        return Err(Error::SbrFreqBandInvalid);
    }
    let kx = k_x as usize;
    for &(start, stop) in groups {
        if start < kx || stop > kx + m_cnt || start >= stop {
            return Err(Error::SbrFreqBandInvalid);
        }
        let lo = start - kx;
        let hi = stop - kx;
        // ETotal: the group energy the GLimBoost gains would produce.
        let mut e_total = 0.0f64;
        let mut e_curr_sum = 0.0f64;
        for i in lo..hi {
            e_total += g[i] * g[i] * e_curr[i];
            e_curr_sum += e_curr[i];
        }
        // GTarget²: the group-equalized gain.
        let g_target2 = e_total / (EPS0 + e_curr_sum);
        // α(m)-weighted blend into G²ARtemp.
        let mut g_ar2 = vec![0.0f64; hi - lo];
        for i in lo..hi {
            let alpha = if i + 1 < m_cnt {
                dp[i].max(dp[i + 1])
            } else {
                dp[i]
            };
            g_ar2[i - lo] = alpha * g_target2 + (1.0 - alpha) * g[i] * g[i];
        }
        // Restore the exact group output energy.
        let mut e_total_new = 0.0f64;
        for (i, &ga2) in (lo..hi).zip(g_ar2.iter()) {
            e_total_new += ga2 * e_curr[i];
        }
        let scale2 = e_total / (EPS0 + e_total_new);
        for (i, &ga2) in (lo..hi).zip(g_ar2.iter()) {
            g[i] = (ga2 * scale2).sqrt();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Figure 4.53 hand-walked: an even subband with a negative
    /// reflection whose left neighbour also reflects negatively is a
    /// full-degree alias pair, and a positive `ref(k−2)` marks the
    /// neighbour too.
    #[test]
    fn aliasing_degree_even_subband_cases() {
        // k = 2: sign = 1 (ref[2] < 0); sign·ref[1] < 0 → deg[2] = 1;
        // sign·ref[0] forced 0 → no deg[1] update.
        let deg = aliasing_degree(&[0.9, -0.6, -0.5, 0.0]);
        assert_eq!(deg, vec![0.0, 0.0, 1.0, 0.0]);

        // k = 4: sign = 1; ref[3] = −0.4 < 0 → deg[4] = 1, and ref[2]
        // = 0.5 > 0 marks the neighbour: deg[3] = 1 − 0.4² = 0.84
        // (k = 2 and k = 3 fire on neither orientation).
        let deg = aliasing_degree(&[0.0, 0.0, 0.5, -0.4, -0.3]);
        assert_eq!(deg[4], 1.0);
        assert!((deg[3] - 0.84).abs() < 1e-12);
        assert_eq!(&deg[..3], &[0.0, 0.0, 0.0]);

        // k = 2 with ref[1] < 0 and ref[0]... ref[0] is forced to 0 by
        // the flowchart even when transmitted non-zero.
        let deg = aliasing_degree(&[0.9, 0.2, -0.5, 0.0]);
        assert_eq!(deg[2], 0.0, "no alias: sign·ref[1] > 0, ref[0] forced 0");
    }

    /// Figure 4.53 odd-subband orientation: positive reflection at an
    /// odd `k` with a positive left neighbour (sign = −1 →
    /// sign·ref[k−1] < 0) is a full-degree alias, and `ref(k−2) < 0`
    /// marks the neighbour.
    #[test]
    fn aliasing_degree_odd_subband_cases() {
        // k = 3: ref[3] > 0 → sign = −1; −ref[2] < 0 (ref[2] > 0) →
        // deg[3] = 1; −ref[1] > 0 (ref[1] < 0) → deg[2] = 1 − ref[2]².
        let deg = aliasing_degree(&[0.0, -0.8, 0.3, 0.7]);
        assert_eq!(deg[3], 1.0);
        assert!((deg[2] - (1.0 - 0.09)).abs() < 1e-12);

        // Odd-k else-branch: k = 2 fires first (ref[2] < 0, ref[1] <
        // 0 → deg[2] = 1), then k = 3: −ref[2] = 0.3 ≥ 0 → else;
        // −ref[1] = 0.6 > 0 → deg[3] = 1 − ref[2]² = 0.91.
        let deg = aliasing_degree(&[0.0, -0.6, -0.3, 0.7]);
        assert_eq!(deg[2], 1.0);
        assert!((deg[3] - 0.91).abs() < 1e-12);
    }

    /// `degPatched`: the source degrees ride the patch mapping, patch
    /// starts and uncovered tail are zero.
    #[test]
    fn deg_patched_rides_patches() {
        let patches = Patches {
            start: vec![2, 4],
            num: vec![3, 2],
        };
        // deg over the low band 0..k0.
        let deg = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7];
        // M = 7: patches cover 5 subbands, tail of 2 stays zero.
        let dp = deg_patched(&deg, &patches, 10, 7).unwrap();
        // Patch 0 (src 2..5): x=0 → 0, then deg[3], deg[4].
        // Patch 1 (src 4..6): x=0 → 0, then deg[5].
        assert_eq!(dp, vec![0.0, 0.3, 0.4, 0.0, 0.5, 0.0, 0.0]);
    }

    /// Figure 4.54 hand-walk: a run of alias-prone boundaries opens a
    /// group at its first subband and closes it past the last one; a
    /// mapped sinusoid closes the group *before* the sinusoid subband;
    /// a run reaching the loop end closes at `kx + M`.
    #[test]
    fn gain_groups_hand_walk() {
        let kx = 8;
        // dp[1], dp[2] non-zero → boundaries after subbands 0 and 1.
        let dp = [0.0, 1.0, 0.5, 0.0, 0.0, 0.0];
        let sm = [false; 6];
        assert_eq!(gain_groups(&dp, &sm, kx), vec![(8, 11)]);

        // A sinusoid at rel 1 blocks the group from covering it: the
        // open condition fails at rel 1, and the close lands at k
        // (the sinusoid subband) rather than k + 1.
        let sm = [false, true, false, false, false, false];
        let dp = [0.0, 1.0, 1.0, 1.0, 0.0, 0.0];
        assert_eq!(gain_groups(&dp, &sm, kx), vec![(8, 9), (10, 12)]);

        // A run whose alias boundaries reach the end of the SBR range
        // closes at kx + M.
        let sm = [false; 6];
        let dp = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0];
        assert_eq!(gain_groups(&dp, &sm, kx), vec![(11, 14)]);
    }

    /// §4.6.18.8.5: the group output energy under GA equals the
    /// GLimBoost energy exactly (the ETotal/ETotalNew normalization),
    /// and a full-degree group equalizes the gains.
    #[test]
    fn aliasing_reduction_preserves_group_energy() {
        let kx = 8;
        let e_curr = [4.0, 1.0, 9.0, 2.0];
        let mut g = [3.0, 0.5, 1.0, 2.0];
        let dp = [0.0, 1.0, 1.0, 1.0];
        let groups = vec![(8usize, 12usize)];
        let e_before: f64 = g
            .iter()
            .zip(e_curr.iter())
            .map(|(gi, ei)| gi * gi * ei)
            .sum();
        aliasing_reduction(&mut g, &e_curr, &dp, &groups, kx).unwrap();
        let e_after: f64 = g
            .iter()
            .zip(e_curr.iter())
            .map(|(gi, ei)| gi * gi * ei)
            .sum();
        assert!(
            (e_after - e_before).abs() < 1e-6 * e_before,
            "group energy {e_after} vs {e_before}"
        );
        // α = 1 on every interior subband → gains equalize to the
        // target (the last subband blends with α = dp[3] = 1 too).
        for w in g.windows(2) {
            assert!((w[0] - w[1]).abs() < 1e-9, "gains not equalized: {g:?}");
        }
    }

    /// Subbands outside every group keep their GLimBoost value.
    #[test]
    fn aliasing_reduction_leaves_ungrouped_gains() {
        let kx = 0;
        let e_curr = [1.0, 1.0, 1.0, 1.0];
        let mut g = [1.0, 2.0, 3.0, 4.0];
        let dp = [0.0, 0.6, 0.0, 0.0];
        let groups = vec![(0usize, 2usize)];
        aliasing_reduction(&mut g, &e_curr, &dp, &groups, kx).unwrap();
        assert_eq!(g[2], 3.0);
        assert_eq!(g[3], 4.0);
        // The grouped pair's energy is preserved.
        let e = g[0] * g[0] + g[1] * g[1];
        assert!((e - 5.0).abs() < 1e-9, "group energy {e}");
    }
}

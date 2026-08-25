//! Enhanced-coupling sub-band / band geometry — ATSC A/52:2018 Annex E
//! §E.2.3.3.16-19 + §E.3.5.2.
//!
//! Enhanced coupling (`ecplinu == 1`) reconstructs the high-frequency
//! transform coefficients of the fbw channels from a single shared
//! *enhanced coupling channel* plus per-band amplitude / angle / chaos
//! parameters (§E.3.5.5). Before any of that synthesis can run, the
//! decoder has to know the **band geometry**: which transform
//! coefficients (bins) belong to the enhanced-coupling region, how the
//! 22 fixed sub-bands of Table E3.7 are grouped into the variable
//! coupling *bands* that carry one coordinate each, and how many such
//! bands (`necplbnd`) exist for the current block.
//!
//! This module is the pure, spec-tabulated geometry layer. It carries:
//!
//! * [`begin_subbnd`] / [`end_subbnd`] — the Table E3.8 derivations of
//!   `ecpl_begin_subbnd` (from `ecplbegf`) and `ecpl_end_subbnd` (from
//!   `ecplendf`, or from the SPX begin when SPX is co-active).
//! * [`ECPL_SUBBND_TAB`] — Table E3.9 `ecplsubbndtab[]`, the starting
//!   transform-coefficient number of each of the 22 sub-bands (plus the
//!   one-past-the-end sentinel at index 22).
//! * [`DEFAULT_ECPL_BNDSTRC`] — Table E2.14 `defecplbndstrc[]`, the
//!   default banding used the first time enhanced coupling is active in
//!   a frame when `ecplbndstrce == 0`.
//! * [`necplbnd`] — §E.2.3.3.19 band-count derivation from the
//!   per-sub-band `ecplbndstrc[]` merge bits.
//! * [`band_bin_counts`] — the §E.3.5.5.1 `nbins_per_bnd_array[]`
//!   population: how many transform coefficients each enhanced coupling
//!   band spans.
//!
//! The geometry layer is kept isolated and unit-tested so the synthesis
//! steps that build on it (parameter processing, carrier reconstruction,
//! per-channel coefficient generation) start from a verified foundation.
//! The full §E.3.5.5 per-block synthesis is orchestrated by
//! [`synthesize_block`]; the cross-block random sources live on
//! [`EcplState`].
//!
//! As of round 300 the module also carries the **bitstream-syntax** layer
//! that sits on top of the geometry: [`parse_strategy`] reads the
//! §E.2.3.3.16-19 strategy fields and [`parse_coords`] reads the
//! §E.2.3.3.20-26 per-band amplitude / angle / chaos coordinates. These
//! advance the bit cursor exactly per the reference syntax so an
//! enhanced-coupling block can be walked without desync.
//!
//! Round 306 adds the **§E.3.5.5.2 / §E.3.5.5.3 parameter-processing**
//! layer: [`ampbnd`] / [`angle_value`] / [`chaos_value`] decode the
//! Table E3.10-E3.12 index triples, [`process_band_amplitudes`] applies
//! the §E.3.5.5.2 chaos amplitude modification, [`expand_bands_to_bins`]
//! fans the per-band values out to per-bin `ampbin[]`, and
//! [`interpolate_bin_angles`] implements the `ecplangleintrp == 1`
//! linear-interpolation path (§E.3.5.5.3). These turn the decoded index
//! triples into the per-bin amplitude / angle arrays the synthesis
//! consumes — pure tabulated arithmetic with no multi-block state.
//!
//! The next layer (added later) closes the §E.3.5.5.3 angle path with the
//! chaos × random de-correlation term and implements the §E.3.5.5.4
//! channel transform-coefficient generation (the per-bin complex product
//! against the reconstructed coupling channel `Z[k]`):
//! [`apply_decorrelation`], [`generate_channel_coeffs`], the
//! [`RandNoTrans`] init-once non-transient random array, [`gen_rand_trans`]
//! per-block transient random, and the [`synthesis_window`] `y[bin]`
//! factor. These are pure tabulated arithmetic over the carrier `Z[k]`.
//!
//! The final layer ([`reconstruct_carrier`]) implements §E.3.5.5.1: the
//! prev/curr/next windowed IMDCT + overlap-add + forward-DFT that produces
//! the non-aliased complex coupling channel `Z[k]` from the de-normalised
//! enhanced-coupling mantissas. The function takes all three blocks'
//! coefficient buffers (the cross-block state is owned by the caller), so
//! this whole module stays a pure, unit-tested layer end to end.
//!
//! The decoder-level *integration* is provided by [`synthesize_block`]
//! plus the cross-block [`EcplState`] (the §E.3.5.5.3 random
//! de-correlation sources): given the previous / current / next blocks'
//! de-normalised enhanced-coupling coefficients ([`EcplBlock`]), it runs
//! the full §E.3.5.5 "for each block" procedure and writes each coupled
//! channel's transform coefficients. The E-AC-3 dsp layer
//! (`super::dsp`) owns the bitstream extraction + per-block buffering and
//! calls into it.
//!
//! **Spec note (erratum):** the default-banding table is captioned
//! "Table E2.14" in the document's table-of-contents (and list of
//! tables) but is cross-referenced as "Table E2.13" from the body of
//! §E.2.3.3.18 — the latter collides with the *standard* coupling
//! default at the genuine Table E2.13. The two tables hold different
//! values; the enhanced-coupling values used here are those listed in
//! full under the §E.2.3.3.18 heading.

use crate::imdct::{dft_512_forward, imdct_512_fft};
use crate::tables::WINDOW;
use oxideav_core::bits::BitReader;
use oxideav_core::{Error, Result};

/// Table E3.9 — `ecplsubbndtab[]`. The starting transform-coefficient
/// number of each of the 22 enhanced-coupling sub-bands, with a
/// one-past-the-end sentinel (`253`) at index 22 so the half-open span
/// of sub-band `s` is `ECPL_SUBBND_TAB[s] .. ECPL_SUBBND_TAB[s + 1]`.
///
/// Sub-bands 0..=3 are 6 bins wide (13..18, 19..24, 25..30, 31..36);
/// sub-bands 4..=21 are 12 bins wide. The enhanced-coupling region thus
/// spans transform coefficients 13..=252.
pub const ECPL_SUBBND_TAB: [usize; 23] = [
    13, 19, 25, 31, 37, 49, 61, 73, 85, 97, 109, 121, 133, 145, 157, 169, 181, 193, 205, 217, 229,
    241, 253,
];

/// Number of enhanced-coupling sub-bands defined by Table E3.7.
pub const N_ECPL_SUBBND: usize = 22;

/// Table E2.14 — `defecplbndstrc[]`. The default enhanced-coupling
/// banding structure, indexed by absolute sub-band number. A `true`
/// ('1') entry means "merge sub-band `s` into the previous band". Per
/// §E.2.3.3.19 the merge bits for sub-bands `<= max(ecpl_begin_subbnd,
/// 8)` are always zero (and not transmitted); the table reflects that —
/// sub-bands 0..=8 are all `false`, with the first merge at sub-band 9.
pub const DEFAULT_ECPL_BNDSTRC: [bool; N_ECPL_SUBBND] = {
    let mut t = [false; N_ECPL_SUBBND];
    // §E.2.3.3.18 default: sub-bands 0..8 → 0; then the per-row values.
    t[9] = true;
    // t[10] = false (12 → 0)
    t[11] = true;
    // t[12] = false
    t[13] = true;
    // t[14] = false
    t[15] = true;
    t[16] = true;
    t[17] = true;
    // t[18] = false
    t[19] = true;
    t[20] = true;
    t[21] = true;
    t
};

/// §E.2.3.3.16 — derive `ecpl_begin_subbnd` from the 4-bit `ecplbegf`
/// code (Table E3.8).
///
/// ```text
/// if (ecplbegf < 3)       ecpl_begin_subbnd = ecplbegf * 2
/// else if (ecplbegf < 13) ecpl_begin_subbnd = ecplbegf + 2
/// else                    ecpl_begin_subbnd = ecplbegf * 2 - 10
/// ```
#[inline]
pub fn begin_subbnd(ecplbegf: u8) -> usize {
    let f = ecplbegf as usize;
    if f < 3 {
        f * 2
    } else if f < 13 {
        f + 2
    } else {
        f * 2 - 10
    }
}

/// §E.2.3.3.17 — derive `ecpl_end_subbnd` (one greater than the highest
/// active enhanced-coupling sub-band), per Table E3.8.
///
/// When spectral extension is **not** in use the end sub-band is taken
/// directly from the 4-bit `ecplendf` code (`ecplendf + 7`). When SPX
/// **is** co-active the enhanced-coupling region is instead bounded by
/// the SPX begin so the two regions abut: `spxbegf + 5` for
/// `spxbegf < 6`, else `spxbegf * 2`. In the SPX-active case `ecplendf`
/// is not transmitted.
#[inline]
pub fn end_subbnd(spxinu: bool, ecplendf: u8, spxbegf: usize) -> usize {
    if !spxinu {
        ecplendf as usize + 7
    } else if spxbegf < 6 {
        spxbegf + 5
    } else {
        spxbegf * 2
    }
}

/// §E.2.3.3.19 — number of enhanced-coupling bands.
///
/// ```text
/// necplbnd  = ecpl_end_subbnd - ecpl_begin_subbnd;
/// necplbnd -= sum(ecplbndstrc[ecpl_begin_subbnd ..= ecpl_end_subbnd-1])
/// ```
///
/// Each set merge bit collapses one sub-band into the previous band, so
/// the count is the number of sub-bands in the active span minus the
/// number of merges. `bndstrc` is indexed by absolute sub-band number.
#[inline]
pub fn necplbnd(begin: usize, end: usize, bndstrc: &[bool; N_ECPL_SUBBND]) -> usize {
    let span = end.saturating_sub(begin);
    // Clamp the slice bounds so an inverted or out-of-grid `begin..end`
    // (only reachable on malformed input — `parse_strategy` rejects it
    // upstream) cannot panic: `lo > hi` would be an invalid range.
    let lo = begin.min(N_ECPL_SUBBND);
    let hi = end.min(N_ECPL_SUBBND).max(lo);
    let merges = bndstrc[lo..hi].iter().filter(|&&b| b).count();
    span - merges
}

/// §E.3.5.5.1 — populate `nbins_per_bnd_array[]`: the number of
/// transform-coefficient bins spanned by each enhanced-coupling band.
///
/// Walking the active sub-band span, a `0` merge bit opens a new band
/// and a `1` merge bit extends the current band; each sub-band
/// contributes `ecplsubbndtab[s+1] - ecplsubbndtab[s]` bins (6 for the
/// narrow low sub-bands, 12 for the rest). The returned vector has
/// length [`necplbnd`].
pub fn band_bin_counts(begin: usize, end: usize, bndstrc: &[bool; N_ECPL_SUBBND]) -> Vec<usize> {
    let mut counts: Vec<usize> = Vec::new();
    // `begin..end` is validated `< N_ECPL_SUBBND` and non-empty by
    // `parse_strategy`; clamp here too so a stray inverted range from a
    // future caller degrades to an empty walk instead of a panic.
    let lo = begin.min(ECPL_SUBBND_TAB.len().saturating_sub(1));
    let hi = end.min(N_ECPL_SUBBND).max(lo);
    for sbnd in lo..hi {
        let bins = ECPL_SUBBND_TAB[sbnd + 1] - ECPL_SUBBND_TAB[sbnd];
        if !bndstrc[sbnd] {
            // New band.
            counts.push(bins);
        } else if let Some(last) = counts.last_mut() {
            // Merge into the current band.
            *last += bins;
        } else {
            // Defensive: a leading merge bit with no open band. The spec
            // guarantees `ecplbndstrc[begin] == 0`, so this can only be
            // reached on malformed input; treat it as a new band.
            counts.push(bins);
        }
    }
    counts
}

/// First transform-coefficient number of the enhanced-coupling region
/// for the given begin sub-band (Table E3.9 lookup). This is where the
/// shared enhanced-coupling channel starts; below it every channel is
/// independently coded.
#[inline]
pub fn begin_bin(begin: usize) -> usize {
    ECPL_SUBBND_TAB[begin.min(N_ECPL_SUBBND)]
}

/// One-past-the-last transform-coefficient number of the
/// enhanced-coupling region for the given end sub-band (Table E3.9
/// lookup).
#[inline]
pub fn end_bin(end: usize) -> usize {
    ECPL_SUBBND_TAB[end.min(N_ECPL_SUBBND)]
}

/// Maximum number of enhanced-coupling bands. Equals [`N_ECPL_SUBBND`]
/// (no merge bits → every sub-band is its own band), so a fixed-size
/// per-band buffer never overflows.
pub const MAX_ECPL_BND: usize = N_ECPL_SUBBND;

/// The decoded **enhanced-coupling strategy** for a block (§E.2.3.3.16-19).
///
/// This is the resolved geometry the coordinate parse + the eventual
/// §E.3.5.5 synthesis consume: the active sub-band span, the per-sub-band
/// merge structure, and the derived band count. It is produced by
/// [`parse_strategy`] from the `cplstre[blk] && cplinu[blk] && ecplinu`
/// branch of Table E1.4, or carried forward unchanged on a block whose
/// `cplstre[blk]` is `0` (strategy reuse).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EcplStrategy {
    /// Raw `ecplbegf` (§E.2.3.3.16) — the 4-bit begin-frequency code as
    /// transmitted, before the [`begin_subbnd`] mapping. Retained because
    /// the §E.3.3.2 `nrematbd` derivation thresholds the raw code directly
    /// (0/1/2/<5), not the derived sub-band index.
    pub ecplbegf: u8,
    /// `ecpl_begin_subbnd` — index of the first active sub-band
    /// (§E.2.3.3.16).
    pub begin_subbnd: usize,
    /// `ecpl_end_subbnd` — one greater than the highest active sub-band
    /// (§E.2.3.3.17).
    pub end_subbnd: usize,
    /// Resolved `ecplbndstrc[]`, indexed by absolute sub-band number: a
    /// `true` entry merges that sub-band into the previous band.
    pub bndstrc: [bool; N_ECPL_SUBBND],
    /// `necplbnd` — number of enhanced-coupling bands (§E.2.3.3.19).
    pub necplbnd: usize,
}

impl EcplStrategy {
    /// First transform-coefficient (bin) of the enhanced-coupling region.
    #[inline]
    pub fn begin_bin(&self) -> usize {
        begin_bin(self.begin_subbnd)
    }

    /// One-past-the-last transform-coefficient (bin) of the region.
    #[inline]
    pub fn end_bin(&self) -> usize {
        end_bin(self.end_subbnd)
    }

    /// Per-band bin counts (`nbins_per_bnd_array[]`, §E.3.5.5.1).
    #[inline]
    pub fn band_bin_counts(&self) -> Vec<usize> {
        band_bin_counts(self.begin_subbnd, self.end_subbnd, &self.bndstrc)
    }
}

/// Parse the enhanced-coupling **strategy** block (§E.2.3.3.16-19 / the
/// `ecplinu` arm of Table E1.4, reached only when
/// `cplstre[blk] && cplinu[blk] && ecplinu`).
///
/// Field order (each `read_u32` advances the cursor exactly as the
/// reference syntax does):
///
/// 1. `ecplbegf` (4 bits) → `ecpl_begin_subbnd` via [`begin_subbnd`].
/// 2. `ecplendf` (4 bits) **only when SPX is off**; when SPX is active
///    `ecpl_end_subbnd` is derived from `spxbegf` and `ecplendf` is *not*
///    transmitted ([`end_subbnd`]).
/// 3. `ecplbndstrce` (1 bit). When set, the per-sub-band merge bits
///    `ecplbndstrc[sbnd]` follow for
///    `sbnd in [max(9, ecpl_begin_subbnd + 1), ecpl_end_subbnd)` — the
///    sub-bands up to and including `max(8, ecpl_begin_subbnd)` are known
///    to be `0` and are never sent (§E.2.3.3.19).
///
/// `ecplbndstrce == 0` means *use the default / reuse the previous*
/// structure. The caller supplies `prev_bndstrc`: pass
/// [`DEFAULT_ECPL_BNDSTRC`] on the first block of the frame that enables
/// enhanced coupling, or the previously-decoded structure on a later
/// block (§E.2.3.3.18). When `ecplbndstrce == 1` the supplied default is
/// ignored and a fresh all-`false` base is populated from the wire bits.
pub fn parse_strategy(
    br: &mut BitReader<'_>,
    spxinu: bool,
    spxbegf: usize,
    prev_bndstrc: &[bool; N_ECPL_SUBBND],
) -> Result<EcplStrategy> {
    let ecplbegf = br.read_u32(4)? as u8;
    let begin = begin_subbnd(ecplbegf);
    let ecplendf = if spxinu { 0 } else { br.read_u32(4)? as u8 };
    let end = end_subbnd(spxinu, ecplendf, spxbegf);

    // §E.2.3.3.16-17: the enhanced-coupling region must be a non-empty
    // span within the valid sub-band grid. `ecplbegf`/`ecplendf` are raw
    // 4-bit codes, so a malformed frame can decode to `begin >= end` or
    // `end > N_ECPL_SUBBND` — e.g. `ecplbegf == 15` (begin = 20) paired
    // with a low `ecplendf`. Left unchecked, the inverted `begin..end`
    // range panics the band-structure slicing in `necplbnd` /
    // `band_bin_counts`. Reject here (mirroring the §E.2.3.3.4-6 SPX
    // sub-band-range guard in `eac3::dsp`) so adversarial input yields a
    // recoverable decode error instead of a process abort.
    if begin >= end || end > N_ECPL_SUBBND {
        return Err(Error::invalid(
            "eac3 ecpl: enhanced-coupling sub-band range invalid \
             (begin >= end or end > N_ECPL_SUBBND)",
        ));
    }

    let ecplbndstrce = br.read_u32(1)? != 0;
    let mut bndstrc = if ecplbndstrce {
        // Fresh structure from the wire. Sub-bands up to and including
        // max(8, begin) are implicitly 0 and not transmitted; the loop
        // starts at max(9, begin + 1). The merge bit for the very first
        // active sub-band is therefore never sent — it always starts a
        // band (§E.2.3.3.19).
        let mut t = [false; N_ECPL_SUBBND];
        let lo = (begin + 1).max(9);
        for sbnd in lo..end.min(N_ECPL_SUBBND) {
            t[sbnd] = br.read_u32(1)? != 0;
        }
        t
    } else {
        // Reuse the default (first block) or the previous block's
        // structure (later block) — no bits consumed.
        *prev_bndstrc
    };
    // §E.2.3.3.19: "the elements of the array corresponding to the
    // sub-bands up to and including ecpl_begin_subbnd or 8 (whichever
    // is greater), are always zero". This must be enforced on the
    // RESOLVED structure too: the Table E2.14 default carries a merge
    // bit at sub-band 9, and a region beginning there (ecplbegf == 7)
    // would otherwise count a phantom merge — `necplbnd` and the
    // §E.3.5.5.1 band walk would disagree by one band, desyncing every
    // coordinate field that follows.
    for slot in bndstrc
        .iter_mut()
        .take((begin.max(8) + 1).min(N_ECPL_SUBBND))
    {
        *slot = false;
    }

    let necplbnd = necplbnd(begin, end, &bndstrc);
    Ok(EcplStrategy {
        ecplbegf,
        begin_subbnd: begin,
        end_subbnd: end,
        bndstrc,
        necplbnd,
    })
}

/// Per-channel decoded enhanced-coupling **parameters** for a block
/// (§E.2.3.3.21-26). Only channels in coupling carry an entry; the angle
/// and chaos arrays of the *first* coupled channel are spec-fixed to `0`
/// and not transmitted, so they stay empty on that channel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EcplChannelParams {
    /// `ecplparam1e[ch]` — amplitudes present this block.
    pub param1e: bool,
    /// `ecplparam2e[ch]` — angle + chaos present this block.
    pub param2e: bool,
    /// `ecplamp[ch][bnd]` — 5-bit amplitude index per band (present iff
    /// `param1e`).
    pub amp: Vec<u8>,
    /// `ecplangle[ch][bnd]` — 6-bit angle index per band (present iff
    /// `param2e` and not the first coupled channel).
    pub angle: Vec<u8>,
    /// `ecplchaos[ch][bnd]` — 3-bit chaos index per band (present iff
    /// `param2e` and not the first coupled channel).
    pub chaos: Vec<u8>,
    /// `ecpltrans[ch]` — transient-present flag (not transmitted for the
    /// first coupled channel, where it is `false`).
    pub trans: bool,
}

/// The decoded enhanced-coupling **coordinate** block for one audio block
/// (§E.2.3.3.20-26 / the `ecplinu` arm of the coupling-coordinate loop in
/// Table E1.4, reached when `cplinu[blk] && ecplinu`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EcplCoords {
    /// `ecplangleintrp` — angle-interpolation flag (§E.2.3.3.20).
    pub angleintrp: bool,
    /// Per-front-channel parameters, indexed by channel number. Channels
    /// not in coupling carry a default (all-`false`) entry.
    pub channels: Vec<EcplChannelParams>,
}

/// Parse the enhanced-coupling **coordinate** block (§E.2.3.3.20-26).
///
/// `nfchans` is the number of full-bandwidth channels; `chincpl[ch]`
/// flags which of them are in coupling. `firstcplcos[ch]` is the
/// per-channel "first coupling block this frame" marker (the same one the
/// standard-coupling `cplcoe` gate uses): on the first block a channel
/// enters coupling, its `ecplparam1e`/`ecplparam2e` are *implicit* (not
/// read from the wire) and all parameters are forced present — the spec
/// guarantees every channel transmits its full parameter set the first
/// time enhanced coupling is enabled. The marker is cleared in place so
/// later blocks read the explicit exist bits.
///
/// `necplbnd` is the band count from the active [`EcplStrategy`]. The
/// first coupled channel (`firstchincpl`) never carries `ecplparam2e`,
/// `ecplangle`, `ecplchaos`, or `ecpltrans` — its angle/chaos are
/// spec-fixed to `0` (§E.2.3.3.24-26).
pub fn parse_coords(
    br: &mut BitReader<'_>,
    nfchans: usize,
    chincpl: &[bool],
    firstcplcos: &mut [bool],
    necplbnd: usize,
) -> Result<EcplCoords> {
    let angleintrp = br.read_u32(1)? != 0;
    let mut channels = vec![EcplChannelParams::default(); nfchans];

    // firstchincpl = -1 → the first channel actually in coupling.
    let mut firstchincpl: Option<usize> = None;

    for ch in 0..nfchans {
        if !chincpl[ch] {
            // §E.2.3.3 "!chincpl[ch]" arm: re-arm the first-coupling
            // marker so a later block re-entering coupling treats its
            // parameters as implicit-present again.
            firstcplcos[ch] = true;
            continue;
        }
        let is_first = firstchincpl.is_none();
        if is_first {
            firstchincpl = Some(ch);
        }

        let (param1e, param2e) = if firstcplcos[ch] {
            // First block this channel is in coupling: parameters are
            // implicit. param1e is always 1; param2e is 1 only for
            // channels after the first coupled channel.
            firstcplcos[ch] = false;
            (true, !is_first)
        } else {
            let p1 = br.read_u32(1)? != 0;
            // param2e is transmitted only for channels after the first
            // coupled channel; the first coupled channel's angle/chaos
            // are fixed to 0, so it has no param2e bit.
            let p2 = if !is_first {
                br.read_u32(1)? != 0
            } else {
                false
            };
            (p1, p2)
        };

        let mut params = EcplChannelParams {
            param1e,
            param2e,
            ..Default::default()
        };

        if param1e {
            params.amp.reserve_exact(necplbnd);
            for _ in 0..necplbnd {
                params.amp.push(br.read_u32(5)? as u8);
            }
        }
        if param2e {
            params.angle.reserve_exact(necplbnd);
            params.chaos.reserve_exact(necplbnd);
            for _ in 0..necplbnd {
                params.angle.push(br.read_u32(6)? as u8);
                params.chaos.push(br.read_u32(3)? as u8);
            }
        }
        // ecpltrans[ch] is transmitted only for channels after the first
        // coupled channel.
        if !is_first {
            params.trans = br.read_u32(1)? != 0;
        }

        channels[ch] = params;
    }

    Ok(EcplCoords {
        angleintrp,
        channels,
    })
}

/// §2.3.3.21-22 reuse semantics: when `ecplparam1e[ch] == 0` the
/// previously transmitted amplitudes for that channel are reused, and
/// when `ecplparam2e[ch] == 0` the previously transmitted angle + chaos
/// values are reused. [`parse_coords`] leaves the corresponding vectors
/// empty on a reuse block; this merges the prior block's values into the
/// fresh set so the §E.3.5.5 synthesis sees the persistent coordinates.
///
/// Values are only carried when the prior vector's length matches the
/// active band count (`necplbnd`) — a mid-frame strategy change that
/// alters the banding invalidates stale coordinates (the spec requires
/// retransmission in that case; degrading to silence is the defensive
/// fallback for a malformed stream).
pub fn merge_reused_params(curr: &mut EcplCoords, prev: &EcplCoords, necplbnd: usize) {
    for (ch, params) in curr.channels.iter_mut().enumerate() {
        let Some(prior) = prev.channels.get(ch) else {
            continue;
        };
        if !params.param1e && params.amp.is_empty() && prior.amp.len() == necplbnd {
            params.amp = prior.amp.clone();
        }
        if !params.param2e && params.angle.is_empty() && prior.angle.len() == necplbnd {
            params.angle = prior.angle.clone();
            params.chaos = prior.chaos.clone();
        }
    }
}

// ===========================================================================
// §E.3.5.5.2 / §E.3.5.5.3 — enhanced-coupling parameter processing
// ===========================================================================
//
// This layer turns the decoded per-band index triples (`ecplamp` /
// `ecplangle` / `ecplchaos` carried in [`EcplChannelParams`]) into the
// per-*bin* amplitude and angle arrays the §E.3.5.5.4 complex-product
// synthesis consumes. It is pure tabulated arithmetic over the active
// band geometry — no multi-block state, no FFT.
//
// The §E.3.5.5.3 closing de-correlation step, the §E.3.5.5.4 complex
// synthesis, and the §E.3.5.5.1 carrier reconstruction
// ([`reconstruct_carrier`]) are all added below this block. The remaining
// work outside this pure layer is the decoder-level integration that
// supplies the prev/curr/next mantissa buffers and consumes the per-channel
// coefficients.

/// Table E3.10 — `ecplampexptab[]`. The binary exponent (right-shift
/// count) applied to the mantissa for each 5-bit `ecplamp` index. Index
/// 31 (minus-infinity dB) has no exponent and is handled specially in
/// [`ampbnd`]; its slot here is `0` and never read on that path.
pub const ECPL_AMP_EXP_TAB: [u8; 32] = [
    0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 0,
];

/// Table E3.10 — `ecplampmanttab[]`. The 6-bit mantissa for each 5-bit
/// `ecplamp` index (the `/32` in [`ampbnd`] applies the implicit binary
/// point). Index 31 is `0x00` (minus-infinity dB → amplitude 0).
pub const ECPL_AMP_MANT_TAB: [u8; 32] = [
    0x20, 0x1b, 0x17, 0x13, 0x10, 0x1b, 0x17, 0x13, 0x10, 0x1b, 0x17, 0x13, 0x10, 0x1b, 0x17, 0x13,
    0x10, 0x1b, 0x17, 0x13, 0x10, 0x1b, 0x17, 0x13, 0x10, 0x1b, 0x17, 0x13, 0x10, 0x1b, 0x17, 0x00,
];

/// Table E3.11 — `ecplangletab[]`. The band phase angle (in the spec's
/// normalised `-1.0 ..= 1.0` units, representing `-pi ..= pi`) for each
/// 6-bit `ecplangle` index. Codes 0..=31 are the non-negative angles
/// `0.0, 0.03125, … 0.96875`; codes 32..=63 are the negative angles
/// `-1.0, -0.96875, … -0.03125`.
pub const ECPL_ANGLE_TAB: [f32; 64] = {
    let mut t = [0.0f32; 64];
    let mut i = 0;
    while i < 32 {
        t[i] = i as f32 / 32.0;
        i += 1;
    }
    let mut i = 32;
    while i < 64 {
        t[i] = -1.0 + (i - 32) as f32 / 32.0;
        i += 1;
    }
    t
};

/// Table E3.12 — `ecplchaostab[]`. The chaos scaling factor (in
/// `0.0 ..= -1.0`) for each 3-bit `ecplchaos` index. The values are the
/// eighths `-k/7` for `k = 0..=7`.
pub const ECPL_CHAOS_TAB: [f32; 8] = [
    0.0, -0.142857, -0.285714, -0.428571, -0.571429, -0.714286, -0.857143, -1.0,
];

/// §E.3.5.5.2 — the band amplitude `ampbnd[ch][bnd]` for a single 5-bit
/// `ecplamp` index, *before* the chaos modification.
///
/// ```text
/// if (ecplamp == 31)  ampbnd = 0
/// else                ampbnd = (ecplampmanttab[ecplamp] / 32) >> ecplampexptab[ecplamp]
/// ```
///
/// The spec's `>> exp` is a fixed-point right shift, i.e. division by
/// `2^exp`; we evaluate it in floating point as `(mant / 32) / 2^exp`.
/// Index 0 (`mant = 0x20`, `exp = 0`) yields `1.0` (0 dB); index 30
/// (`mant = 0x17`, `exp = 7`) yields `≈ 0.005615` (≈ -45.01 dB); index
/// 31 yields `0.0` (minus-infinity dB).
#[inline]
pub fn ampbnd(ecplamp: u8) -> f32 {
    let idx = (ecplamp & 0x1f) as usize;
    if idx == 31 {
        return 0.0;
    }
    let mant = ECPL_AMP_MANT_TAB[idx] as f32 / 32.0;
    let exp = ECPL_AMP_EXP_TAB[idx] as i32;
    mant / 2f32.powi(exp)
}

/// §E.3.5.5.2 — the chaos value `chaos[ch][bnd]` for a single 3-bit
/// `ecplchaos` index. The first coupled channel always reads `0.0`
/// regardless of the index (its chaos/angle are spec-fixed to zero).
#[inline]
pub fn chaos_value(ecplchaos: u8, is_first_coupled: bool) -> f32 {
    if is_first_coupled {
        0.0
    } else {
        ECPL_CHAOS_TAB[(ecplchaos & 0x07) as usize]
    }
}

/// §E.3.5.5.3 — the band angle `angle[ch][bnd]` for a single 6-bit
/// `ecplangle` index. The first coupled channel always reads `0.0`.
#[inline]
pub fn angle_value(ecplangle: u8, is_first_coupled: bool) -> f32 {
    if is_first_coupled {
        0.0
    } else {
        ECPL_ANGLE_TAB[(ecplangle & 0x3f) as usize]
    }
}

/// §E.3.5.5.2 — the fully-processed per-band amplitudes for one channel,
/// including the chaos modification.
///
/// For each band, [`ampbnd`] converts the `ecplamp` index to a linear
/// gain, then the chaos modification
///
/// ```text
/// if (ecpltrans == 0 && ch != firstchincpl)
///     ampbnd[bnd] *= 1 + 0.38 * chaos[bnd]
/// ```
///
/// scales it (note `chaos` is `<= 0`, so this *reduces* the amplitude of
/// non-transient, non-first coupled channels). `is_first_coupled` carries
/// the `ch == firstchincpl` test; `trans` carries `ecpltrans[ch]`.
///
/// Returns a vector of length `necplbnd` (the length of `params.amp`).
pub fn process_band_amplitudes(params: &EcplChannelParams, is_first_coupled: bool) -> Vec<f32> {
    let mut out = Vec::with_capacity(params.amp.len());
    for (bnd, &amp_idx) in params.amp.iter().enumerate() {
        let mut a = ampbnd(amp_idx);
        // The chaos modification applies only to non-first coupled
        // channels with no transient. `chaos`/`angle` are only present
        // for those channels (param2e), so a missing entry means 0.
        if !params.trans && !is_first_coupled {
            let chaos_idx = params.chaos.get(bnd).copied().unwrap_or(0);
            let chaos = chaos_value(chaos_idx, is_first_coupled);
            a *= 1.0 + 0.38 * chaos;
        }
        out.push(a);
    }
    out
}

/// §E.3.5.5.2 — expand per-band values to per-sub-band then per-bin.
///
/// `band_vals` holds one value per enhanced-coupling *band*; this fans
/// each band's value out across every transform-coefficient bin it spans,
/// using the merge structure `bndstrc[]` to walk the same band boundaries
/// the parse used. The returned vector is indexed by *bin offset from
/// `begin_bin`* (i.e. element `0` is transform coefficient
/// `ecplsubbndtab[begin_subbnd]`), with length
/// `end_bin(end) - begin_bin(begin)`.
///
/// This is the §E.3.5.5.2 `ampbin[ch][bin]` reconstruction; the same
/// fan-out applies to the no-interpolation angle path (§E.3.5.5.3) and to
/// the per-bin chaos / random expansion.
pub fn expand_bands_to_bins(
    begin: usize,
    end: usize,
    bndstrc: &[bool; N_ECPL_SUBBND],
    band_vals: &[f32],
) -> Vec<f32> {
    let total = end_bin(end).saturating_sub(begin_bin(begin));
    let mut out = Vec::with_capacity(total);
    // `bnd` tracks the current band index into `band_vals`. The first
    // active sub-band always opens band 0 (its merge bit is never sent).
    let mut bnd: isize = -1;
    for sbnd in begin..end.min(N_ECPL_SUBBND) {
        if !bndstrc[sbnd] {
            bnd += 1;
        }
        let val = if bnd >= 0 {
            band_vals.get(bnd as usize).copied().unwrap_or(0.0)
        } else {
            // Defensive: a leading merge bit (spec guarantees the first
            // active sub-band starts a band, so this is malformed input).
            band_vals.first().copied().unwrap_or(0.0)
        };
        let nbins = ECPL_SUBBND_TAB[sbnd + 1] - ECPL_SUBBND_TAB[sbnd];
        for _ in 0..nbins {
            out.push(val);
        }
    }
    out
}

/// §E.3.5.5.3 — per-bin angle reconstruction with linear interpolation
/// between band centres (the `ecplangleintrp == 1` path).
///
/// `band_angles` holds one angle per band (the [`angle_value`] outputs);
/// `nbins_per_bnd` holds the bin count of each band (from
/// [`band_bin_counts`]). The result is one angle per bin across the
/// active region, length `sum(nbins_per_bnd)`. Angles wrap into the
/// `-1.0 ..= 1.0` interval after each interpolation step, exactly as the
/// reference pseudo-code's `while` guards do.
///
/// The single-band case (`nbands < 2`) has no inter-band slope, so the
/// one band's angle is simply fanned across all its bins.
pub fn interpolate_bin_angles(band_angles: &[f32], nbins_per_bnd: &[usize]) -> Vec<f32> {
    let nbands = band_angles.len().min(nbins_per_bnd.len());
    let total: usize = nbins_per_bnd.iter().take(nbands).sum();
    let mut out = vec![0.0f32; total];
    if nbands == 0 {
        return out;
    }
    if nbands == 1 {
        for slot in out.iter_mut() {
            *slot = band_angles[0];
        }
        return out;
    }

    let mut bin: usize = 0;
    for bnd in 1..nbands {
        let nbins_prev = nbins_per_bnd[bnd - 1];
        let nbins_curr = nbins_per_bnd[bnd];
        let angle_prev = band_angles[bnd - 1];
        let mut angle_curr = band_angles[bnd];
        // Unwrap the current band angle to within one step of the prev.
        while (angle_curr - angle_prev) > 1.0 {
            angle_curr -= 2.0;
        }
        while (angle_prev - angle_curr) > 1.0 {
            angle_curr += 2.0;
        }
        let slope = (angle_curr - angle_prev) / ((nbins_curr + nbins_prev) as f32 / 2.0);

        // Lower half of the first band (walks downward from the centre).
        if bnd == 1 && nbins_prev > 1 {
            let (mut y, mut down_bin): (f32, usize);
            if nbins_prev % 2 == 0 {
                y = angle_prev - slope / 2.0;
                down_bin = nbins_prev / 2 - 1;
            } else {
                y = angle_prev - slope;
                down_bin = (nbins_prev - 3) / 2;
            }
            let count = down_bin + 1;
            for _ in 0..count {
                let ytmp = y;
                while y > 1.0 {
                    y -= 2.0;
                }
                while y < -1.0 {
                    y += 2.0;
                }
                out[down_bin] = y;
                down_bin = down_bin.saturating_sub(1);
                y = ytmp - slope;
            }
            bin = count;
        }

        let (mut y, count): (f32, usize) = if nbins_prev % 2 == 0 {
            (angle_prev + slope / 2.0, nbins_curr / 2 + nbins_prev / 2)
        } else {
            (angle_prev, nbins_curr / 2 + nbins_prev.div_ceil(2))
        };
        for _ in 0..count {
            let ytmp = y;
            while y > 1.0 {
                y -= 2.0;
            }
            while y < -1.0 {
                y += 2.0;
            }
            if bin < total {
                out[bin] = y;
                bin += 1;
            }
            y = ytmp + slope;
        }

        // Finish the last band when this is the final iteration. The
        // reference carries `y`/`slope` out of the loop and runs one
        // closing pass; we mirror that by detecting the last band here.
        if bnd == nbands - 1 {
            let last_count = if nbins_curr % 2 == 0 {
                nbins_curr / 2
            } else {
                nbins_curr / 2 + 1
            };
            for _ in 0..last_count {
                let ytmp = y;
                while y > 1.0 {
                    y -= 2.0;
                }
                while y < -1.0 {
                    y += 2.0;
                }
                if bin < total {
                    out[bin] = y;
                    bin += 1;
                }
                y = ytmp + slope;
            }
        }
    }

    out
}

// ===========================================================================
// §E.3.5.5.3 (closing) / §E.3.5.5.4 — de-correlation + complex synthesis
// ===========================================================================
//
// This layer closes the §E.3.5.5.3 angle path (the chaos × random
// de-correlation added to each bin angle) and implements the §E.3.5.5.4
// channel transform-coefficient generation (the per-bin complex product
// against the reconstructed enhanced-coupling channel `Z[k]`).
//
// It consumes the per-bin amplitude / angle arrays produced by the r306
// parameter-processing layer plus the per-bin `chaos` array (the same
// `expand_bands_to_bins` fan-out applied to the band chaos values) and a
// per-bin `rand` array (this module's [`RandNoTrans`] for non-transient
// channels, [`gen_rand_trans`] expanded for transient channels). The
// §E.3.5.5.1 carrier reconstruction that turns the de-normalised
// enhanced-coupling mantissas into the complex carrier `Z[k]` is
// implemented further below in [`reconstruct_carrier`]; the caller owns the
// cross-block mantissa buffers and feeds them in.

/// §E.3.5.5.4 — the post-FFT MDCT synthesis window factor
/// `y[bin] = cos(2π · (N/4 + 0.5) / N · (bin + 0.5))` for the 512-point
/// transform (`N = 512`). The final real coefficient combines `y[bin]`
/// with the mirror `y[N/2 - 1 - bin]` per the spec's
/// `chmant = -2·(y[bin]·Zr + y[N/2-1-bin]·Zi)`.
#[inline]
pub fn synthesis_window(bin: usize, n: usize) -> f32 {
    let nf = n as f32;
    let arg = 2.0 * std::f32::consts::PI * (nf / 4.0 + 0.5) / nf * (bin as f32 + 0.5);
    arg.cos()
}

/// §E.3.5.5.3 (closing) — the de-correlation random sequence for a
/// **non-transient** channel: `rand_notrans[ch][bin]`.
///
/// Per spec these values must be (a) uniformly distributed on `-1.0 ..=
/// 1.0`, (b) unique for each bin and channel, and (c) generated **once**
/// (e.g. at decoder init) and held constant for every block of every
/// frame. The generator itself is non-normative ("a scaled array of
/// random values"); a deterministic xorshift seeded per channel satisfies
/// all three properties while keeping decodes reproducible.
///
/// The struct caches one full array of `N/2 = 256` values per channel so a
/// repeated block lookup is a cheap index, matching the spec's
/// "generated once" requirement.
#[derive(Clone, Debug)]
pub struct RandNoTrans {
    /// One `[-1, 1]` value per transform-coefficient bin (`0 .. N/2`).
    vals: Vec<f32>,
}

impl RandNoTrans {
    /// Build the per-bin random array for one channel. `n` is the
    /// transform size (512); the array holds `n / 2` values. `ch` seeds
    /// the generator so each channel gets a distinct, stable sequence.
    pub fn new(ch: usize, n: usize) -> Self {
        let half = n / 2;
        // Seed per channel; a non-zero seed is required for xorshift.
        let mut lfsr: u32 = 0x9E37_79B9 ^ (ch as u32).wrapping_mul(0x0100_0193).wrapping_add(1);
        if lfsr == 0 {
            lfsr = 1;
        }
        let mut vals = Vec::with_capacity(half);
        for _ in 0..half {
            vals.push(uniform_pm1(&mut lfsr));
        }
        Self { vals }
    }

    /// The cached `rand_notrans` value for `bin` (0-based from
    /// transform-coefficient 0). Out-of-range bins return `0.0`.
    #[inline]
    pub fn get(&self, bin: usize) -> f32 {
        self.vals.get(bin).copied().unwrap_or(0.0)
    }
}

/// §E.3.5.5.3 (closing) — the de-correlation random sequence for a
/// **transient** channel: `rand_trans[ch][bnd]`, one fresh value per
/// **band** generated for every block (the band values are afterward fanned
/// out to bins by [`expand_bands_to_bins`], exactly like the chaos array).
///
/// Per spec these are uniform on `-1.0 ..= 1.0`, unique per band and
/// channel, and **new for each block** (in contrast to the non-transient
/// init-once array). The caller threads a mutable LFSR state so successive
/// blocks advance the sequence; `nbands` band values are produced.
pub fn gen_rand_trans(lfsr: &mut u32, nbands: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(nbands);
    for _ in 0..nbands {
        out.push(uniform_pm1(lfsr));
    }
    out
}

/// One xorshift step mapped to a uniform value on `[-1, 1]`. Shared by the
/// non-transient and transient de-correlation generators.
#[inline]
fn uniform_pm1(lfsr: &mut u32) -> f32 {
    let mut x = *lfsr;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    if x == 0 {
        x = 1;
    }
    *lfsr = x;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// §E.3.5.5.3 (closing) — add the chaos-scaled random de-correlation term
/// to each per-bin angle and re-wrap into `-1.0 ..= 1.0`.
///
/// ```text
/// angle[ch][bin] += chaos[ch][bin] * rand[ch][bin];
/// if (angle < -1.0)       angle += 2.0;
/// else if (angle >= 1.0)  angle -= 2.0;
/// ```
///
/// `angles`, `chaos`, and `rand` are all per-bin arrays of equal length
/// (the `chaos` band values fanned out by [`expand_bands_to_bins`]; the
/// `rand` array is either the [`RandNoTrans`] cache for a non-transient
/// channel or the fanned-out [`gen_rand_trans`] band values for a transient
/// channel). The wrap is the spec's single-step fold — `chaos` is
/// `0.0 ..= -1.0` and `rand` is `-1.0 ..= 1.0`, so the product is within
/// `±1.0` and one fold suffices.
pub fn apply_decorrelation(angles: &mut [f32], chaos: &[f32], rand: &[f32]) {
    for (bin, a) in angles.iter_mut().enumerate() {
        let c = chaos.get(bin).copied().unwrap_or(0.0);
        let r = rand.get(bin).copied().unwrap_or(0.0);
        *a += c * r;
        if *a < -1.0 {
            *a += 2.0;
        } else if *a >= 1.0 {
            *a -= 2.0;
        }
    }
}

/// §E.3.5.5.4 — generate the individual-channel transform coefficients
/// from the reconstructed enhanced-coupling channel.
///
/// For each bin in the active region the spec forms the complex product of
/// the carrier `Z[bin] = Zr + j·Zi` with the per-channel coordinate
/// `ampbin · e^{jπ·angle}`:
///
/// ```text
/// Zr_ch = Zr·amp·cos(π·angle) − Zi·amp·sin(π·angle)
/// Zi_ch = Zi·amp·cos(π·angle) + Zr·amp·sin(π·angle)
/// chmant[bin] = -2 · ( y[bin]·Zr_ch + y[N/2-1-bin]·Zi_ch )
/// ```
///
/// `zr` / `zi` are the per-bin real / imaginary parts of the carrier `Z`
/// over `bin = 0 .. N/2` (the §E.3.5.5.1 FFT output, supplied by the
/// caller); `ampbin` / `bin_angle` are the per-bin amplitude / angle
/// arrays for this channel, indexed by *offset from `begin_bin`*. The
/// result `chmant` is written at the absolute transform-coefficient index
/// `begin_bin + offset`. `n` is the transform size (512).
///
/// Bins outside `[begin_bin, begin_bin + ampbin.len())` are left
/// untouched, so the caller can pre-zero or pre-fill the low/independent
/// region.
#[allow(clippy::too_many_arguments)]
pub fn generate_channel_coeffs(
    zr: &[f32],
    zi: &[f32],
    ampbin: &[f32],
    bin_angle: &[f32],
    begin_bin: usize,
    n: usize,
    out: &mut [f32],
) {
    let half = n / 2;
    let span = ampbin.len().min(bin_angle.len());
    for offset in 0..span {
        let bin = begin_bin + offset;
        if bin >= half {
            break;
        }
        let amp = ampbin[offset];
        let angle = bin_angle[offset];
        let (s, c) = (std::f32::consts::PI * angle).sin_cos();
        let zr_bin = zr.get(bin).copied().unwrap_or(0.0);
        let zi_bin = zi.get(bin).copied().unwrap_or(0.0);
        let zr_ch = zr_bin * amp * c - zi_bin * amp * s;
        let zi_ch = zi_bin * amp * c + zr_bin * amp * s;
        let y_bin = synthesis_window(bin, n);
        let y_mirror = synthesis_window(half - 1 - bin, n);
        let chmant = -2.0 * (y_bin * zr_ch + y_mirror * zi_ch);
        if let Some(slot) = out.get_mut(bin) {
            *slot = chmant;
        }
    }
}

// ===========================================================================
// §E.3.5.5.1 — enhanced-coupling channel processing (carrier `Z[k]`)
// ===========================================================================
//
// This closes the last deferred piece of §E.3.5.5: turning the
// de-normalised enhanced-coupling mantissas of the previous / current /
// next blocks into the non-aliased complex carrier `Z[k]` the
// §E.3.5.5.4 per-channel synthesis multiplies against. The procedure is
// stateful across blocks (it needs the prev + next block's coefficients),
// so the caller supplies all three buffers; the function itself is pure.
//
// The five spec steps:
//   1) zero-pad each block's ecpl mantissas into a length-N/2 MDCT buffer,
//   2) 512-sample IMDCT (§7.9.4.1 steps 1-5, windowed) of each buffer,
//   3) overlap-add prev's 2nd half + next's 1st half with curr,
//   4) apply the analysis window + xcos3/xsin3 oddly-stacked rotation,
//   5) forward DFT to obtain Z[k], k = 0..N-1.

/// §E.3.5.5.1 step 4 — `xcos3[n] = cos(π·n/N)` for `N = 512`.
#[inline]
fn xcos3(n: usize) -> f32 {
    (std::f32::consts::PI * n as f32 / 512.0).cos()
}

/// §E.3.5.5.1 step 4 — `xsin3[n] = -sin(π·n/N)` for `N = 512`.
#[inline]
fn xsin3(n: usize) -> f32 {
    -(std::f32::consts::PI * n as f32 / 512.0).sin()
}

/// §E.3.5.5.1 step 5 output — the non-aliased complex enhanced-coupling
/// carrier `Z[k]`, `k = 0 .. N-1` (`N = 512`), as parallel real/imag
/// arrays. Element `k` is `Zr[k] + j·Zi[k]`.
#[derive(Clone, Debug)]
pub struct EcplCarrier {
    /// Real part `Zr[k]`, `k = 0 .. 512`.
    pub zr: [f32; 512],
    /// Imaginary part `Zi[k]`, `k = 0 .. 512`.
    pub zi: [f32; 512],
}

/// §E.3.5.5.1 — reconstruct the non-aliased complex enhanced-coupling
/// channel `Z[k]` from the de-normalised enhanced-coupling mantissas of
/// the previous, current and next blocks.
///
/// `prev` / `curr` / `next` are each the 256 (`= N/2`) MDCT transform
/// coefficients of the enhanced-coupling channel for that block, already
/// de-normalised and **zero outside** the active region
/// `[ecplstartmant, ecplendmant)` (step 1's `XPREV`/`XCURR`/`XNEXT`
/// definition). When enhanced coupling is not in use in the previous or
/// next block, the caller passes an all-zero buffer there (per the spec's
/// "set to zero" rule).
///
/// The procedure (spec steps 2-5):
///
/// 2. Each buffer is run through the windowed 512-sample IMDCT
///    (§7.9.4.1 steps 1-5): [`imdct_512_fft`] gives the bare time samples,
///    then the [`WINDOW`] (Table 7.33) multiply `x[n]·w[n]` /
///    `x[511-n]·w[n]` completes step 5. The result is the 512-sample
///    `xPREV` / `xCURR` / `xNEXT`.
/// 3. Overlap-add: `pcm[n] = xPREV[n+N/2] + xCURR[n]` and
///    `pcm[n+N/2] = xCURR[n+N/2] + xNEXT[n]` for `n = 0 .. N/2`.
/// 4. The complex analysis input is `pcm[n]·w·xcos3[n]` (real) and
///    `pcm[n]·w·xsin3[n]` (imag), with `w = w[n]` over the lower half and
///    `w = w[N/2-n-1]` over the upper half, mirroring the spec's two-arm
///    windowing.
/// 5. A normalised forward DFT ([`dft_512_forward`]) produces `Z[k]`.
pub fn reconstruct_carrier(prev: &[f32; 256], curr: &[f32; 256], next: &[f32; 256]) -> EcplCarrier {
    // Step 2 — windowed IMDCT of each block.
    let windowed = |x: &[f32; 256]| -> [f32; 512] {
        let mut t = [0.0f32; 512];
        imdct_512_fft(x, &mut t);
        for n in 0..256 {
            t[n] *= WINDOW[n];
            t[511 - n] *= WINDOW[n];
        }
        t
    };
    let xprev = windowed(prev);
    let xcurr = windowed(curr);
    let xnext = windowed(next);

    // Step 3 — overlap-add into a single 512-sample pcm buffer.
    //
    // **Spec erratum (codified as `docs/audio/ac3/ac3-errata.md` entry
    // E3):** as printed, §E.3.5.5.1 step 3 omits the §7.9.4.1 step-6
    // factor of 2 ("the factor of 2 scaling undoes headroom scaling
    // performed in the encoder"): step 2 references only "steps 1 to 5"
    // of §7.9.4.1, and step 3 — the direct analogue of step 6, an
    // overlap-add over the same windowed samples — prints
    // `pcm[n] = xPREV[n+N/2] + xCURR[n]` with no multiplier. Taken
    // literally, the whole analysis → §E.3.5.5.4 synthesis chain
    // returns exactly HALF the original MDCT coefficients (measured
    // identity gain exactly 0.5 — every enhanced-coupling channel would
    // decode 6 dB low), while the Table E3.10 amplitude ceiling of
    // exactly 1.0 (`ecplamp = 0` is 0 dB per §E.3.5.4) pins the design
    // intent at a unity identity. The corrected reading applies the
    // same factor of 2 as §7.9.4.1 step 6. (The ETSI TS 102 366 V1.4.1
    // copy carries no counterpart clause — its §E.2.5.5 is
    // amplitude-only and omits the channel-processing subclause
    // entirely — so A/52:2018 §E.3.5.5.1 is the operative text.)
    // Pinned by `tests::carrier_overlap_add_factor2_erratum_identity`
    // (corrected fit gain 1.0 vs exactly 0.5 as printed) and
    // `ecplenc::tests::analysis_synthesis_identity_amp1_angle0`.
    let mut pcm = [0.0f32; 512];
    for n in 0..256 {
        pcm[n] = 2.0 * (xprev[256 + n] + xcurr[n]);
        pcm[256 + n] = 2.0 * (xcurr[256 + n] + xnext[n]);
    }

    // Step 4 — oddly-stacked complex rotation. The window is `w[n]` over
    // the lower half and `w[N/2-n-1]` over the upper half; `WINDOW[n]`
    // holds `w[n]` for `n = 0 .. 256`.
    let mut re = [0.0f32; 512];
    let mut im = [0.0f32; 512];
    for n in 0..256 {
        let wl = WINDOW[n]; // w[n]
        let wu = WINDOW[256 - n - 1]; // w[N/2-n-1]
        re[n] = pcm[n] * wl * xcos3(n);
        re[256 + n] = pcm[256 + n] * wu * xcos3(256 + n);
        im[n] = pcm[n] * wl * xsin3(n);
        im[256 + n] = pcm[256 + n] * wu * xsin3(256 + n);
    }

    // Step 5 — forward DFT to the complex carrier `Z[k]`.
    let (zr, zi) = dft_512_forward(&re, &im);
    EcplCarrier { zr, zi }
}

// ===========================================================================
// §E.3.5.5 — decoder-level enhanced-coupling synthesis orchestration
// ===========================================================================
//
// The pieces above are pure per-step primitives. This layer stitches them
// into the full §E.3.5.5 "for each block" procedure the decoder runs once
// the enhanced-coupling channel mantissas/exponents have been decoded and
// de-normalised:
//
//   1. Process the enhanced-coupling channel → carrier `Z[k]`
//      ([`reconstruct_carrier`], from prev/curr/next mantissa buffers).
//   2. Prepare per-bin amplitudes for each coupled channel
//      ([`process_band_amplitudes`] → [`expand_bands_to_bins`]).
//   3. Prepare per-bin angles for each coupled channel ([`angle_value`] →
//      either [`expand_bands_to_bins`] or [`interpolate_bin_angles`], then
//      [`apply_decorrelation`] with the chaos × random term).
//   4. Generate each coupled channel's transform coefficients from the
//      carrier, amplitudes and angles ([`generate_channel_coeffs`]).
//
// The §E.3.5.5.1 carrier needs the *previous*, *current* and *next*
// block's enhanced-coupling mantissas; the decoder owns those buffers and
// passes all three in. The de-correlation random sources have cross-block
// lifetime (the non-transient array is generated once; the transient LFSR
// advances every block), so they live on the persistent [`EcplState`].

/// Maximum number of full-bandwidth channels that can be in coupling.
/// Matches the AC-3 `MAX_FBW` (5 fbw channels in 3/2 mode).
pub const ECPL_MAX_FBW: usize = 5;

/// Cross-block enhanced-coupling synthesis state. Persisted on the decoder
/// for the lifetime of a stream so the §E.3.5.5.3 random de-correlation
/// sources keep their spec-required lifetimes:
///
/// * `rand_notrans[ch]` — the non-transient random array, "generated once
///   (for example during decoder initialization) and … the same for every
///   block of every frame" (§E.3.5.5.3). Built lazily the first time a
///   channel needs it and then reused.
/// * `trans_lfsr` — the transient random generator state; the transient
///   values "must be new for each block", so this LFSR threads across
///   blocks/frames advancing the sequence.
/// * `prev_frame_last_mant` — the de-normalised enhanced-coupling mantissa
///   buffer of the *last* block of the immediately preceding frame, when
///   that block used enhanced coupling. The §E.3.5.5.1 carrier of block 0
///   needs its "previous block" spectrum; block numbering is continuous
///   across the stream, so the previous frame's final enhanced-coupling
///   block is that neighbour. The spec's "set to zero" rule fires only
///   when enhanced coupling was *not* in use in that previous block, which
///   `None` represents (no carried spectrum → zero `prev`).
#[derive(Clone, Debug, Default)]
pub struct EcplState {
    rand_notrans: [Option<RandNoTrans>; ECPL_MAX_FBW],
    trans_lfsr: u32,
    prev_frame_last_mant: Option<[f32; 256]>,
}

impl EcplState {
    /// Fresh state (no random arrays generated yet, transient LFSR seeded).
    pub fn new() -> Self {
        Self {
            rand_notrans: Default::default(),
            // Non-zero seed required for the xorshift transient generator.
            trans_lfsr: 0x2545_F491,
            prev_frame_last_mant: None,
        }
    }

    /// The carried-over enhanced-coupling mantissa buffer of the previous
    /// frame's last block, or `None` when that block did not use enhanced
    /// coupling (the §E.3.5.5.1 "set to zero" boundary case for block 0's
    /// `previous block`).
    pub fn prev_frame_last_mant(&self) -> Option<&[f32; 256]> {
        self.prev_frame_last_mant.as_ref()
    }

    /// Record this frame's final enhanced-coupling mantissa buffer so the
    /// next frame's block 0 carrier can consult it as its "previous block"
    /// (§E.3.5.5.1). Pass `None` when the frame's last block did not use
    /// enhanced coupling, which resets the carry to the zero boundary case.
    pub fn set_prev_frame_last_mant(&mut self, mant: Option<[f32; 256]>) {
        self.prev_frame_last_mant = mant;
    }

    /// The cached non-transient random array for channel `ch`, building it
    /// on first use (§E.3.5.5.3 "generated once"). `n` is the transform
    /// size (512).
    fn rand_notrans(&mut self, ch: usize, n: usize) -> &RandNoTrans {
        if self.rand_notrans[ch].is_none() {
            self.rand_notrans[ch] = Some(RandNoTrans::new(ch, n));
        }
        self.rand_notrans[ch].as_ref().unwrap()
    }
}

/// One block's decoded enhanced-coupling inputs for [`synthesize_block`].
///
/// `mant` holds the de-normalised enhanced-coupling channel transform
/// coefficients for this block, indexed by absolute bin `0 .. N/2`, zero
/// outside the active region `[ecplstartmant, ecplendmant)` (the §E.3.5.5.1
/// step-1 `XCURR` definition). `strategy` and `coords` are the decoded
/// geometry + per-channel parameters. `chincpl[ch]` flags the coupled
/// channels.
#[derive(Clone, Debug)]
pub struct EcplBlock {
    /// De-normalised ecpl-channel MDCT coefficients, length `N/2 = 256`.
    pub mant: [f32; 256],
    /// Active strategy (band geometry) for this block.
    pub strategy: EcplStrategy,
    /// Per-channel decoded coordinates (angle-interp flag + per-band
    /// amplitude/angle/chaos triples).
    pub coords: EcplCoords,
    /// Whether each fbw channel is in coupling this block.
    pub chincpl: [bool; ECPL_MAX_FBW],
}

/// §E.3.5.5 — synthesise the individual-channel transform coefficients for
/// one block of enhanced coupling, writing each coupled channel's result
/// into `out[ch]`.
///
/// `prev` / `curr` / `next` are the three blocks' de-normalised
/// enhanced-coupling mantissa buffers; pass an all-zero `mant` (e.g. an
/// [`EcplBlock`] whose `mant` is `[0.0; 256]`) for a neighbour where
/// enhanced coupling is not in use, per the §E.3.5.5.1 "set to zero" rule.
/// The synthesis applies to `curr`'s strategy/coords; `prev`/`next` are
/// consulted only for their mantissa buffers (the carrier needs the
/// neighbouring spectra to suppress time-domain aliasing).
///
/// `out[ch]` is the absolute-bin coefficient buffer for fbw channel `ch`
/// (length `N/2 = 256`); only bins in the active region
/// `[begin_bin, end_bin)` are written, so the caller pre-fills the
/// independently-coded low region. `n` is the transform size (512).
///
/// The first coupled channel (`firstchincpl`) has angle/chaos fixed to `0`
/// (§E.3.5.5.2-3), so its synthesis is the pure carrier scaled by its
/// per-bin amplitude.
pub fn synthesize_block(
    state: &mut EcplState,
    prev: &EcplBlock,
    curr: &EcplBlock,
    next: &EcplBlock,
    out: &mut [[f32; 256]],
    n: usize,
) {
    // Step 1-5 — reconstruct the non-aliased complex carrier `Z[k]`.
    let carrier = reconstruct_carrier(&prev.mant, &curr.mant, &next.mant);

    let strat = &curr.strategy;
    let begin = strat.begin_subbnd;
    let end = strat.end_subbnd;
    let begin_bin_abs = strat.begin_bin();
    let nbins_per_bnd = strat.band_bin_counts();

    // The first coupled channel anchors the angle/chaos = 0 rule.
    let firstchincpl = (0..ECPL_MAX_FBW).find(|&ch| curr.chincpl[ch]);

    for ch in 0..ECPL_MAX_FBW {
        if !curr.chincpl[ch] {
            continue;
        }
        let is_first = Some(ch) == firstchincpl;
        let params = curr.coords.channels.get(ch);

        // Step 2 — per-bin amplitudes. Missing params (a channel whose
        // coordinates were not re-sent this block) would normally reuse the
        // prior block's values; the decoder threads the persisted params in
        // via `coords`, so an empty `amp` means "all bands zero".
        let band_amps = match params {
            Some(p) if !p.amp.is_empty() => process_band_amplitudes(p, is_first),
            _ => vec![0.0; nbins_per_bnd.len()],
        };
        let ampbin = expand_bands_to_bins(begin, end, &strat.bndstrc, &band_amps);

        // Step 3 — per-bin angles. The first coupled channel and any channel
        // without param2e have all-zero band angles.
        let band_angles: Vec<f32> = if is_first {
            vec![0.0; nbins_per_bnd.len()]
        } else {
            match params {
                Some(p) if !p.angle.is_empty() => {
                    p.angle.iter().map(|&a| angle_value(a, false)).collect()
                }
                _ => vec![0.0; nbins_per_bnd.len()],
            }
        };
        let mut bin_angle = if curr.coords.angleintrp {
            interpolate_bin_angles(&band_angles, &nbins_per_bnd)
        } else {
            expand_bands_to_bins(begin, end, &strat.bndstrc, &band_angles)
        };

        // Step 3 (closing) — chaos × random de-correlation. The first
        // coupled channel has chaos 0 (no de-correlation); other channels
        // fan their per-band chaos to bins, pick the transient/non-transient
        // random source, and add the term.
        if !is_first {
            let trans = params.map(|p| p.trans).unwrap_or(false);
            let band_chaos: Vec<f32> = match params {
                Some(p) if !p.chaos.is_empty() => {
                    p.chaos.iter().map(|&c| chaos_value(c, false)).collect()
                }
                _ => vec![0.0; nbins_per_bnd.len()],
            };
            let chaos_bin = expand_bands_to_bins(begin, end, &strat.bndstrc, &band_chaos);
            let rand_bin: Vec<f32> = if trans {
                // Transient: one fresh random value per band, fanned to bins.
                let band_rand = gen_rand_trans(&mut state.trans_lfsr, nbins_per_bnd.len());
                expand_bands_to_bins(begin, end, &strat.bndstrc, &band_rand)
            } else {
                // Non-transient: the init-once per-bin array, sliced to the
                // active region.
                let rnt = state.rand_notrans(ch, n);
                (0..bin_angle.len())
                    .map(|off| rnt.get(begin_bin_abs + off))
                    .collect()
            };
            apply_decorrelation(&mut bin_angle, &chaos_bin, &rand_bin);
        }

        // Step 4 — generate this channel's transform coefficients from the
        // carrier and the per-bin amplitude/angle arrays.
        generate_channel_coeffs(
            &carrier.zr,
            &carrier.zi,
            &ampbin,
            &bin_angle,
            begin_bin_abs,
            n,
            &mut out[ch],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxideav_core::bits::{BitReader, BitWriter};

    #[test]
    fn ecplsubbndtab_matches_table_e3_9() {
        // Spot-check the Table E3.9 values + the 6/12-bin widths.
        assert_eq!(ECPL_SUBBND_TAB[0], 13);
        assert_eq!(ECPL_SUBBND_TAB[4], 37);
        assert_eq!(ECPL_SUBBND_TAB[21], 241);
        assert_eq!(ECPL_SUBBND_TAB[22], 253); // sentinel
                                              // Sub-bands 0..=3 are 6 bins wide.
        for s in 0..4 {
            assert_eq!(ECPL_SUBBND_TAB[s + 1] - ECPL_SUBBND_TAB[s], 6);
        }
        // Sub-bands 4..=21 are 12 bins wide.
        for s in 4..N_ECPL_SUBBND {
            assert_eq!(ECPL_SUBBND_TAB[s + 1] - ECPL_SUBBND_TAB[s], 12);
        }
        // Region spans tc 13..=252.
        assert_eq!(begin_bin(0), 13);
        assert_eq!(end_bin(22), 253);
    }

    #[test]
    fn begin_subbnd_table_e3_8() {
        // ecplbegf < 3 → *2.
        assert_eq!(begin_subbnd(0), 0);
        assert_eq!(begin_subbnd(1), 2);
        assert_eq!(begin_subbnd(2), 4);
        // 3..=12 → +2.
        assert_eq!(begin_subbnd(3), 5);
        assert_eq!(begin_subbnd(7), 9);
        assert_eq!(begin_subbnd(12), 14);
        // >=13 → *2 - 10.
        assert_eq!(begin_subbnd(13), 16);
        assert_eq!(begin_subbnd(14), 18);
        assert_eq!(begin_subbnd(15), 20);
    }

    #[test]
    fn end_subbnd_table_e3_8_no_spx() {
        // SPX off: ecplendf + 7.
        assert_eq!(end_subbnd(false, 0, 0), 7);
        assert_eq!(end_subbnd(false, 8, 0), 15);
        assert_eq!(end_subbnd(false, 15, 0), 22);
    }

    #[test]
    fn end_subbnd_table_e3_8_with_spx() {
        // SPX on, spxbegf < 6 → spxbegf + 5.
        assert_eq!(end_subbnd(true, 0, 0), 5);
        assert_eq!(end_subbnd(true, 0, 5), 10);
        // SPX on, spxbegf >= 6 → spxbegf * 2.
        assert_eq!(end_subbnd(true, 0, 6), 12);
        assert_eq!(end_subbnd(true, 0, 7), 14);
        // ecplendf is ignored when SPX is active.
        assert_eq!(end_subbnd(true, 15, 6), 12);
    }

    #[test]
    fn default_bndstrc_table_e2_14() {
        // Sub-bands 0..=8 are all zero (never transmitted; always merge=0).
        for s in 0..=8 {
            assert!(!DEFAULT_ECPL_BNDSTRC[s], "sbnd {s} should be 0");
        }
        // The Table E2.14 merge rows.
        assert!(DEFAULT_ECPL_BNDSTRC[9]);
        assert!(!DEFAULT_ECPL_BNDSTRC[10]);
        assert!(DEFAULT_ECPL_BNDSTRC[11]);
        assert!(!DEFAULT_ECPL_BNDSTRC[12]);
        assert!(DEFAULT_ECPL_BNDSTRC[13]);
        assert!(!DEFAULT_ECPL_BNDSTRC[14]);
        assert!(DEFAULT_ECPL_BNDSTRC[15]);
        assert!(DEFAULT_ECPL_BNDSTRC[16]);
        assert!(DEFAULT_ECPL_BNDSTRC[17]);
        assert!(!DEFAULT_ECPL_BNDSTRC[18]);
        assert!(DEFAULT_ECPL_BNDSTRC[19]);
        assert!(DEFAULT_ECPL_BNDSTRC[20]);
        assert!(DEFAULT_ECPL_BNDSTRC[21]);
    }

    #[test]
    fn necplbnd_no_merges() {
        // All-zero banding → every sub-band is its own band.
        let bndstrc = [false; N_ECPL_SUBBND];
        // begin=9, end=22 → 13 sub-bands, no merges → 13 bands.
        assert_eq!(necplbnd(9, 22, &bndstrc), 13);
        // A small span: begin=5, end=8 → 3 sub-bands.
        assert_eq!(necplbnd(5, 8, &bndstrc), 3);
    }

    #[test]
    fn necplbnd_with_default_banding() {
        // Default banding over the full high span begin=9, end=22.
        // 13 sub-bands; merges at 9,11,13,15,16,17,19,20,21 that fall in
        // [9,22) → 9 merges → 13 - 9 = 4 bands.
        let n = necplbnd(9, 22, &DEFAULT_ECPL_BNDSTRC);
        assert_eq!(n, 4);
    }

    #[test]
    fn band_bin_counts_no_merges_low_subbands() {
        // begin=0, end=4 → narrow 6-bin sub-bands, no merges → four
        // 6-bin bands.
        let bndstrc = [false; N_ECPL_SUBBND];
        let counts = band_bin_counts(0, 4, &bndstrc);
        assert_eq!(counts, vec![6, 6, 6, 6]);
    }

    #[test]
    fn band_bin_counts_default_banding_high() {
        // begin=9, end=22 under the default banding. Sub-bands here are
        // all 12-bin wide. Bands open at sub-bands with merge==0
        // (9 opens? no — 9 has merge=1 but it is the first sub-band of
        // the span, so per band_bin_counts the leading merge starts a new
        // band defensively). To match the spec's guarantee that the
        // first sub-band of the active span is never a merge, test a span
        // whose first sub-band has merge=0.
        //
        // begin=10, end=22 → first sub-band 10 has merge=0 (new band).
        // Merge bits in [10,22): 11,13,15,16,17,19,20,21 → 8 merges.
        // 12 sub-bands → 4 bands. Each band's bin count = 12 * (sub-bands
        // in band).
        let counts = band_bin_counts(10, 22, &DEFAULT_ECPL_BNDSTRC);
        // Bands: [10,11]=24, [12,13]=24, [14,15,16,17]=48, [18,19,20,21]=48.
        assert_eq!(counts, vec![24, 24, 48, 48]);
        // Total bins == span bins (12 each * 12 sub-bands = 144).
        let total: usize = counts.iter().sum();
        assert_eq!(total, 144);
        assert_eq!(total, end_bin(22) - begin_bin(10));
    }

    #[test]
    fn band_bin_counts_length_equals_necplbnd() {
        // The vector length must equal necplbnd for any banding.
        let begin = 10;
        let end = 22;
        let n = necplbnd(begin, end, &DEFAULT_ECPL_BNDSTRC);
        let counts = band_bin_counts(begin, end, &DEFAULT_ECPL_BNDSTRC);
        assert_eq!(counts.len(), n);
    }

    // ---- §E.2.3.3.16-19 strategy parse ----

    #[test]
    fn parse_strategy_no_spx_default_banding() {
        // ecplbegf = 7 → begin = 9; SPX off, ecplendf = 15 → end = 22;
        // ecplbndstrce = 0 → reuse the supplied default banding (no merge
        // bits on the wire).
        let mut w = BitWriter::new();
        w.write_u32(7, 4); // ecplbegf
        w.write_u32(15, 4); // ecplendf
        w.write_u32(0, 1); // ecplbndstrce = 0
        let bytes = w.finish();

        let mut br = BitReader::new(&bytes);
        let strat = parse_strategy(&mut br, false, 0, &DEFAULT_ECPL_BNDSTRC).unwrap();
        assert_eq!(strat.begin_subbnd, 9);
        assert_eq!(strat.end_subbnd, 22);
        // §E.2.3.3.19: the resolved structure masks the default table's
        // merge bit AT the first active sub-band (entries up to and
        // including max(begin, 8) are always zero) — sub-band 9 starts
        // band 0 here, so the region has 5 bands, not 4. (The pre-r406
        // code counted the phantom merge in necplbnd while the
        // §E.3.5.5.1 band walk opened a band anyway — a one-band
        // coordinate-count desync on any default-banded region
        // beginning at sub-band 9+.)
        let mut expected = DEFAULT_ECPL_BNDSTRC;
        expected[9] = false;
        assert_eq!(strat.bndstrc, expected);
        assert_eq!(strat.necplbnd, 5);
        assert_eq!(strat.band_bin_counts().len(), 5);
        assert_eq!(strat.begin_bin(), 97);
        assert_eq!(strat.end_bin(), 253);
        // Exactly 9 bits consumed (4 + 4 + 1).
        assert_eq!(br.bit_position(), 9);
    }

    #[test]
    fn parse_strategy_spx_active_skips_ecplendf() {
        // SPX on, spxbegf = 5 → end = spxbegf + 5 = 10; ecplendf is NOT
        // transmitted. ecplbegf = 5 → begin = 7. ecplbndstrce = 0.
        let mut w = BitWriter::new();
        w.write_u32(5, 4); // ecplbegf
        w.write_u32(0, 1); // ecplbndstrce = 0 (no ecplendf field)
        let bytes = w.finish();

        let mut br = BitReader::new(&bytes);
        let strat = parse_strategy(&mut br, true, 5, &DEFAULT_ECPL_BNDSTRC).unwrap();
        assert_eq!(strat.begin_subbnd, 7);
        assert_eq!(strat.end_subbnd, 10);
        // Only 5 bits consumed because ecplendf is omitted under SPX.
        assert_eq!(br.bit_position(), 5);
    }

    #[test]
    fn parse_strategy_explicit_banding_bits() {
        // ecplbegf = 7 → begin = 9; ecplendf = 15 → end = 22.
        // ecplbndstrce = 1 → merge bits for sbnd in [max(9,10), 22) =
        // [10, 22): that's 12 bits. Set every other bit so we can verify
        // placement: 10=1, 11=0, 12=1, ... merge bits transmitted from
        // sbnd 10 upward. The first active sub-band (9) is never a merge
        // bit (not transmitted) and stays false.
        let mut w = BitWriter::new();
        w.write_u32(7, 4); // ecplbegf
        w.write_u32(15, 4); // ecplendf
        w.write_u32(1, 1); // ecplbndstrce = 1
        let pattern = [
            true, false, true, false, true, false, true, false, true, false, true, false,
        ];
        for &b in &pattern {
            w.write_u32(b as u32, 1);
        }
        let bytes = w.finish();

        let mut br = BitReader::new(&bytes);
        let strat = parse_strategy(&mut br, false, 0, &DEFAULT_ECPL_BNDSTRC).unwrap();
        // Sub-band 9 is never transmitted → false.
        assert!(!strat.bndstrc[9]);
        // Sub-bands 10..22 match the pattern.
        for (i, &b) in pattern.iter().enumerate() {
            assert_eq!(strat.bndstrc[10 + i], b, "sbnd {}", 10 + i);
        }
        // 6 merges in the pattern → 13 sub-bands − 6 = 7 bands.
        assert_eq!(strat.necplbnd, 13 - 6);
        // 4 + 4 + 1 + 12 = 21 bits.
        assert_eq!(br.bit_position(), 21);
    }

    // ---- §E.2.3.3.20-26 coordinate parse ----

    #[test]
    fn parse_coords_first_block_implicit_present() {
        // 2/0: both channels in coupling, both firstcplcos (first block).
        // necplbnd = 2 for a compact test. ch0 = first coupled channel:
        // param1e implicit 1 (amps follow), param2e = 0 (angle/chaos fixed
        // to 0, not sent), no ecpltrans. ch1: param1e + param2e implicit
        // 1, then ecpltrans (1 bit).
        let necplbnd = 2usize;
        let mut w = BitWriter::new();
        w.write_u32(1, 1); // ecplangleintrp = 1
                           // ch0: implicit param1e=1 → 2 amps (5 bits each).
        w.write_u32(3, 5);
        w.write_u32(7, 5);
        // ch1: implicit param1e=1 → 2 amps; implicit param2e=1 → 2×(angle
        // 6 + chaos 3); ecpltrans = 1.
        w.write_u32(11, 5);
        w.write_u32(15, 5);
        w.write_u32(20, 6);
        w.write_u32(4, 3);
        w.write_u32(33, 6);
        w.write_u32(2, 3);
        w.write_u32(1, 1); // ecpltrans[1]
        let bytes = w.finish();

        let mut br = BitReader::new(&bytes);
        let chincpl = [true, true];
        let mut firstcplcos = [true, true];
        let c = parse_coords(&mut br, 2, &chincpl, &mut firstcplcos, necplbnd).unwrap();
        assert!(c.angleintrp);
        // firstcplcos cleared for both.
        assert_eq!(firstcplcos, [false, false]);

        // ch0 (first coupled): param1e, no param2e, no trans.
        assert!(c.channels[0].param1e);
        assert!(!c.channels[0].param2e);
        assert_eq!(c.channels[0].amp, vec![3, 7]);
        assert!(c.channels[0].angle.is_empty());
        assert!(c.channels[0].chaos.is_empty());
        assert!(!c.channels[0].trans);

        // ch1: param1e + param2e + trans.
        assert!(c.channels[1].param1e);
        assert!(c.channels[1].param2e);
        assert_eq!(c.channels[1].amp, vec![11, 15]);
        assert_eq!(c.channels[1].angle, vec![20, 33]);
        assert_eq!(c.channels[1].chaos, vec![4, 2]);
        assert!(c.channels[1].trans);

        // 1 + (2×5) + (2×5 + 2×(6+3) + 1) = 1 + 10 + 29 = 40 bits.
        assert_eq!(br.bit_position(), 40);
    }

    #[test]
    fn parse_coords_later_block_explicit_exist_bits() {
        // Later block (firstcplcos already cleared): exist bits are read
        // from the wire. ch0 first coupled: param1e bit only (no param2e,
        // no trans). ch1: param1e + param2e bits + trans.
        let necplbnd = 1usize;
        let mut w = BitWriter::new();
        w.write_u32(0, 1); // ecplangleintrp = 0
                           // ch0: param1e = 1 → 1 amp.
        w.write_u32(1, 1); // param1e
        w.write_u32(9, 5); // amp
                           // ch1: param1e = 0 (reuse), param2e = 1 → angle+chaos; trans = 0.
        w.write_u32(0, 1); // param1e
        w.write_u32(1, 1); // param2e
        w.write_u32(42, 6); // angle
        w.write_u32(5, 3); // chaos
        w.write_u32(0, 1); // ecpltrans[1]
        let bytes = w.finish();

        let mut br = BitReader::new(&bytes);
        let chincpl = [true, true];
        let mut firstcplcos = [false, false];
        let c = parse_coords(&mut br, 2, &chincpl, &mut firstcplcos, necplbnd).unwrap();
        assert!(!c.angleintrp);
        assert!(c.channels[0].param1e);
        assert!(!c.channels[0].param2e);
        assert_eq!(c.channels[0].amp, vec![9]);
        assert!(!c.channels[1].param1e);
        assert!(c.channels[1].param2e);
        assert!(c.channels[1].amp.is_empty());
        assert_eq!(c.channels[1].angle, vec![42]);
        assert_eq!(c.channels[1].chaos, vec![5]);
        assert!(!c.channels[1].trans);
        // 1 + (1 + 5) + (1 + 1 + 6 + 3 + 1) = 1 + 6 + 12 = 19 bits.
        assert_eq!(br.bit_position(), 19);
    }

    #[test]
    fn parse_coords_channel_not_in_coupling_rearms_marker() {
        // 3/0: ch1 not in coupling. firstcplcos[1] must be re-armed to
        // true; its params stay default. ch0 + ch2 are coupled.
        let necplbnd = 1usize;
        let mut w = BitWriter::new();
        w.write_u32(0, 1); // ecplangleintrp
                           // ch0 (first coupled, first block): implicit param1e → 1 amp.
        w.write_u32(8, 5);
        // ch1 skipped (not in coupling).
        // ch2 (second coupled, first block): implicit param1e + param2e +
        // trans.
        w.write_u32(12, 5); // amp
        w.write_u32(30, 6); // angle
        w.write_u32(6, 3); // chaos
        w.write_u32(0, 1); // trans
        let bytes = w.finish();

        let mut br = BitReader::new(&bytes);
        let chincpl = [true, false, true];
        let mut firstcplcos = [true, true, true];
        let c = parse_coords(&mut br, 3, &chincpl, &mut firstcplcos, necplbnd).unwrap();
        // ch1 re-armed, ch0 + ch2 cleared.
        assert_eq!(firstcplcos, [false, true, false]);
        assert_eq!(c.channels[0].amp, vec![8]);
        assert_eq!(c.channels[1], EcplChannelParams::default());
        assert_eq!(c.channels[2].amp, vec![12]);
        assert_eq!(c.channels[2].angle, vec![30]);
        // ch2 is the *second* coupled channel → carries param2e + trans.
        assert!(c.channels[2].param2e);
        // 1 + 5 + (5 + 6 + 3 + 1) = 21 bits.
        assert_eq!(br.bit_position(), 21);
    }

    #[test]
    fn merge_reused_params_threads_prior_block_values() {
        // Prior block: full coordinate sets for a 2-band geometry.
        let prev = EcplCoords {
            angleintrp: false,
            channels: vec![
                EcplChannelParams {
                    param1e: true,
                    param2e: false,
                    amp: vec![3, 4],
                    angle: vec![],
                    chaos: vec![],
                    trans: false,
                },
                EcplChannelParams {
                    param1e: true,
                    param2e: true,
                    amp: vec![5, 6],
                    angle: vec![10, 20],
                    chaos: vec![1, 2],
                    trans: false,
                },
            ],
        };
        // Current block: ch0 reuses amps; ch1 reuses angle/chaos but
        // retransmits amps.
        let mut curr = EcplCoords {
            angleintrp: false,
            channels: vec![
                EcplChannelParams::default(),
                EcplChannelParams {
                    param1e: true,
                    param2e: false,
                    amp: vec![7, 8],
                    angle: vec![],
                    chaos: vec![],
                    trans: true,
                },
            ],
        };
        merge_reused_params(&mut curr, &prev, 2);
        assert_eq!(curr.channels[0].amp, vec![3, 4]); // §2.3.3.21 reuse
        assert_eq!(curr.channels[1].amp, vec![7, 8]); // fresh, kept
        assert_eq!(curr.channels[1].angle, vec![10, 20]); // §2.3.3.22 reuse
        assert_eq!(curr.channels[1].chaos, vec![1, 2]);
        assert!(curr.channels[1].trans); // trans is per-block, kept
    }

    #[test]
    fn merge_reused_params_ignores_stale_band_count() {
        // Prior coords sized for 3 bands; the active strategy has 2 —
        // stale values must NOT be carried.
        let prev = EcplCoords {
            angleintrp: false,
            channels: vec![EcplChannelParams {
                param1e: true,
                param2e: false,
                amp: vec![3, 4, 5],
                angle: vec![],
                chaos: vec![],
                trans: false,
            }],
        };
        let mut curr = EcplCoords {
            angleintrp: false,
            channels: vec![EcplChannelParams::default()],
        };
        merge_reused_params(&mut curr, &prev, 2);
        assert!(curr.channels[0].amp.is_empty());
    }

    // ---- §E.3.5.5.2 / §E.3.5.5.3 parameter processing ----

    fn db(linear: f32) -> f32 {
        20.0 * linear.log10()
    }

    #[test]
    fn ampbnd_endpoints_table_e3_10() {
        // Index 0: mant 0x20 (=32) / 32 = 1.0, exp 0 → 0 dB.
        assert!((ampbnd(0) - 1.0).abs() < 1e-6);
        assert!(db(ampbnd(0)).abs() < 1e-4);
        // Index 31: minus-infinity dB → amplitude 0.
        assert_eq!(ampbnd(31), 0.0);
        // Index 30: mant 0x17 (=23)/32 = 0.71875, exp 7 → /128 ≈ 0.005615
        // → ≈ -45.01 dB (the spec's documented lowest finite gain).
        let a30 = ampbnd(30);
        assert!((a30 - (23.0 / 32.0) / 128.0).abs() < 1e-7);
        assert!((db(a30) - (-45.01)).abs() < 0.05);
        // Monotone non-increasing across the finite range 0..=30.
        for i in 1..=30u8 {
            assert!(
                ampbnd(i) <= ampbnd(i - 1) + 1e-7,
                "amp[{i}] not <= amp[{}]",
                i - 1
            );
        }
        // Each exponent step (every 4 indices) approximately halves the
        // mantissa-1.0 anchor: index 4 (mant 0x10/32 = 0.5, exp 0) and
        // index 8 (mant 0x10/32, exp 1 → 0.25).
        assert!((ampbnd(4) - 0.5).abs() < 1e-6);
        assert!((ampbnd(8) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn angle_and_chaos_tables() {
        // Table E3.11: code 0 → 0.0; code 32 → -1.0; code 63 → -0.03125.
        assert_eq!(ECPL_ANGLE_TAB[0], 0.0);
        assert!((ECPL_ANGLE_TAB[31] - 0.96875).abs() < 1e-6);
        assert!((ECPL_ANGLE_TAB[32] - (-1.0)).abs() < 1e-6);
        assert!((ECPL_ANGLE_TAB[63] - (-0.03125)).abs() < 1e-6);
        // Table E3.12: eighths -k/7.
        assert_eq!(ECPL_CHAOS_TAB[0], 0.0);
        assert!((ECPL_CHAOS_TAB[7] - (-1.0)).abs() < 1e-6);
        assert!((ECPL_CHAOS_TAB[3] - (-3.0 / 7.0)).abs() < 1e-5);
        // First-coupled channel forces angle/chaos to 0 regardless of code.
        assert_eq!(angle_value(40, true), 0.0);
        assert_eq!(chaos_value(5, true), 0.0);
        assert!((angle_value(40, false) - (-0.75)).abs() < 1e-6);
    }

    #[test]
    fn chaos_modification_reduces_amplitude() {
        // A non-first, non-transient channel with chaos applied: the
        // §E.3.5.5.2 modification `*= 1 + 0.38*chaos` with chaos <= 0
        // reduces the gain.
        let params = EcplChannelParams {
            param1e: true,
            param2e: true,
            amp: vec![0, 4],   // gains 1.0, 0.5
            chaos: vec![7, 0], // chaos -1.0, 0.0
            angle: vec![0, 0],
            trans: false,
        };
        let amps = process_band_amplitudes(&params, false);
        // band 0: 1.0 * (1 + 0.38 * -1.0) = 0.62.
        assert!((amps[0] - 0.62).abs() < 1e-6);
        // band 1: 0.5 * (1 + 0.38 * 0.0) = 0.5.
        assert!((amps[1] - 0.5).abs() < 1e-6);

        // Same params but transient present → no chaos modification.
        let mut tp = params.clone();
        tp.trans = true;
        let amps_t = process_band_amplitudes(&tp, false);
        assert!((amps_t[0] - 1.0).abs() < 1e-6);

        // First coupled channel → no chaos modification even without
        // transient (its chaos is fixed to 0).
        let amps_first = process_band_amplitudes(&params, true);
        assert!((amps_first[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn expand_bands_to_bins_fans_out_per_subband() {
        // begin sub-band 0, end sub-band 2: sub-bands 0 (6 bins) and 1
        // (6 bins). No merge → two bands of 6 bins each. begin_bin = 13,
        // end_bin = ECPL_SUBBND_TAB[2] = 25 → 12 bins total.
        let bndstrc = [false; N_ECPL_SUBBND];
        let out = expand_bands_to_bins(0, 2, &bndstrc, &[2.0, 5.0]);
        assert_eq!(out.len(), 12);
        assert!(out[..6].iter().all(|&v| (v - 2.0).abs() < 1e-9));
        assert!(out[6..].iter().all(|&v| (v - 5.0).abs() < 1e-9));

        // Merge sub-band 1 into 0 → single band of 12 bins, one value.
        let mut merged = [false; N_ECPL_SUBBND];
        merged[1] = true;
        let out2 = expand_bands_to_bins(0, 2, &merged, &[3.0]);
        assert_eq!(out2.len(), 12);
        assert!(out2.iter().all(|&v| (v - 3.0).abs() < 1e-9));
    }

    #[test]
    fn interpolate_single_band_is_constant() {
        // One band → angle fanned across all bins, no slope.
        let out = interpolate_bin_angles(&[0.25], &[6]);
        assert_eq!(out.len(), 6);
        assert!(out.iter().all(|&v| (v - 0.25).abs() < 1e-9));
    }

    #[test]
    fn interpolate_two_bands_wraps_and_fills() {
        // Two equal-size bands; verify output length == sum of bin counts
        // and every angle stays within the wrapped -1.0 ..= 1.0 interval.
        let band_angles = [0.0, 0.5];
        let nbins = [6usize, 6usize];
        let out = interpolate_bin_angles(&band_angles, &nbins);
        assert_eq!(out.len(), 12);
        for &v in &out {
            assert!((-1.0..=1.0).contains(&v), "angle {v} out of range");
        }
        // The interpolation walks from below the first band centre up
        // through the second; values should be (weakly) increasing in the
        // interior where no wrap occurs.
        // First band centre region near 0.0, last band region near 0.5.
        assert!(out[0] < out[out.len() - 1] + 1e-6);
    }

    #[test]
    fn interpolate_wrap_across_pi() {
        // angle_prev = 0.9, angle_curr = -0.9: the raw difference is
        // -1.8 but the spec unwraps (-0.9 + 2.0 = 1.1) so the slope is
        // small + positive, crossing the +1/-1 boundary which the wrap
        // guards fold back. Output must stay in range.
        let out = interpolate_bin_angles(&[0.9, -0.9], &[6, 6]);
        assert_eq!(out.len(), 12);
        for &v in &out {
            assert!((-1.0..=1.0).contains(&v), "wrapped angle {v} out of range");
        }
    }

    // ---- §E.3.5.5.3 (closing) / §E.3.5.5.4 synthesis ----

    #[test]
    fn rand_notrans_properties() {
        // Per spec: uniform on [-1, 1], unique per bin, generated once and
        // stable across calls, and distinct between channels.
        let n = 512;
        let r0 = RandNoTrans::new(0, n);
        let r0_again = RandNoTrans::new(0, n);
        let r1 = RandNoTrans::new(1, n);
        // 256 values for a 512-point transform.
        for bin in 0..n / 2 {
            let v = r0.get(bin);
            assert!((-1.0..=1.0).contains(&v), "value {v} out of range");
            // Stable across construction (the "generated once" requirement).
            assert_eq!(v, r0_again.get(bin));
        }
        // Out-of-range → 0.0.
        assert_eq!(r0.get(n / 2), 0.0);
        // Channels differ (overwhelmingly likely with distinct seeds; check
        // the first few bins differ between ch0 and ch1).
        let differ = (0..8).any(|b| (r0.get(b) - r1.get(b)).abs() > 1e-9);
        assert!(differ, "channel 0 and 1 random arrays are identical");
        // Roughly zero-mean over the full array.
        let mean: f32 = (0..n / 2).map(|b| r0.get(b)).sum::<f32>() / (n / 2) as f32;
        assert!(mean.abs() < 0.15, "mean {mean} not near zero");
    }

    #[test]
    fn rand_trans_advances_per_block() {
        // Transient random: new values each block (the LFSR advances), each
        // in [-1, 1]. Two consecutive draws of the same band count differ.
        let mut lfsr = 0x1234_5678u32;
        let blk0 = gen_rand_trans(&mut lfsr, 4);
        let blk1 = gen_rand_trans(&mut lfsr, 4);
        assert_eq!(blk0.len(), 4);
        assert_eq!(blk1.len(), 4);
        for &v in blk0.iter().chain(blk1.iter()) {
            assert!((-1.0..=1.0).contains(&v), "value {v} out of range");
        }
        assert_ne!(blk0, blk1, "consecutive blocks produced identical noise");
    }

    #[test]
    fn apply_decorrelation_wraps_single_step() {
        // chaos in [0, -1], rand in [-1, 1] → product in [-1, 1]; a single
        // fold keeps the result in [-1, 1).
        let mut angles = vec![0.9, -0.9, 0.0];
        let chaos = vec![-1.0, -1.0, -0.5];
        let rand = vec![0.5, -0.5, 1.0];
        apply_decorrelation(&mut angles, &chaos, &rand);
        // 0.9 + (-1.0 * 0.5) = 0.4.
        assert!((angles[0] - 0.4).abs() < 1e-6);
        // -0.9 + (-1.0 * -0.5) = -0.4.
        assert!((angles[1] - (-0.4)).abs() < 1e-6);
        // 0.0 + (-0.5 * 1.0) = -0.5.
        assert!((angles[2] - (-0.5)).abs() < 1e-6);
        for &a in &angles {
            assert!((-1.0..1.0).contains(&a), "angle {a} not wrapped");
        }

        // Force a positive overflow: 0.95 + (-1.0 * -0.5) = 1.45 → -0.55.
        let mut a = vec![0.95];
        apply_decorrelation(&mut a, &[-1.0], &[-0.5]);
        assert!((a[0] - (-0.55)).abs() < 1e-6);
        // Force a negative underflow: -0.95 + (-1.0 * 0.5) = -1.45 → 0.55.
        let mut b = vec![-0.95];
        apply_decorrelation(&mut b, &[-1.0], &[0.5]);
        assert!((b[0] - 0.55).abs() < 1e-6);
    }

    #[test]
    fn synthesis_window_mirror_symmetry() {
        // y[bin] = cos(2π·(N/4+0.5)/N·(bin+0.5)). For N=512 the factor is
        // bounded by 1, and the spec pairs y[bin] with y[N/2-1-bin].
        let n = 512;
        for bin in [0usize, 1, 64, 128, 255] {
            let y = synthesis_window(bin, n);
            assert!((-1.0..=1.0).contains(&y), "y[{bin}]={y} out of range");
        }
        // bin 0 of the 512-point window: cos(2π·128.5/512·0.5) =
        // cos(π·128.5/512).
        let expected0 = (std::f32::consts::PI * 128.5 / 512.0).cos();
        assert!((synthesis_window(0, n) - expected0).abs() < 1e-6);
    }

    #[test]
    fn generate_channel_coeffs_unity_passthrough() {
        // amp = 1, angle = 0 → coordinate is real unity → the channel
        // coefficient is the spec's pure-carrier MDCT synthesis:
        // chmant = -2·(y[bin]·Zr + y[N/2-1-bin]·Zi). Verify one bin.
        let n = 512;
        let half = n / 2;
        let begin_bin = 13;
        let span = 6;
        // Carrier: distinctive per-bin real/imag.
        let mut zr = vec![0.0f32; half];
        let mut zi = vec![0.0f32; half];
        for bin in begin_bin..begin_bin + span {
            zr[bin] = (bin as f32) * 0.01;
            zi[bin] = (bin as f32) * -0.02;
        }
        let ampbin = vec![1.0f32; span];
        let bin_angle = vec![0.0f32; span];
        let mut out = vec![0.0f32; half];
        generate_channel_coeffs(&zr, &zi, &ampbin, &bin_angle, begin_bin, n, &mut out);

        for offset in 0..span {
            let bin = begin_bin + offset;
            let y_bin = synthesis_window(bin, n);
            let y_mirror = synthesis_window(half - 1 - bin, n);
            // angle 0 → cos=1, sin=0 → Zr_ch=Zr, Zi_ch=Zi.
            let expected = -2.0 * (y_bin * zr[bin] + y_mirror * zi[bin]);
            assert!(
                (out[bin] - expected).abs() < 1e-5,
                "bin {bin}: {} != {expected}",
                out[bin]
            );
        }
        // Below the region untouched.
        assert_eq!(out[begin_bin - 1], 0.0);
        // Above the region untouched.
        assert_eq!(out[begin_bin + span], 0.0);
    }

    #[test]
    fn generate_channel_coeffs_amplitude_scales() {
        // Halving amp halves the magnitude of the complex coordinate, and
        // since the synthesis is linear in amp, the output halves too.
        let n = 512;
        let half = n / 2;
        let begin_bin = 13;
        let span = 4;
        let mut zr = vec![0.0f32; half];
        let mut zi = vec![0.0f32; half];
        for bin in begin_bin..begin_bin + span {
            zr[bin] = 0.3;
            zi[bin] = 0.1;
        }
        let angle = vec![0.25f32; span]; // π/4
        let mut full = vec![0.0f32; half];
        generate_channel_coeffs(&zr, &zi, &vec![1.0; span], &angle, begin_bin, n, &mut full);
        let mut halved = vec![0.0f32; half];
        generate_channel_coeffs(
            &zr,
            &zi,
            &vec![0.5; span],
            &angle,
            begin_bin,
            n,
            &mut halved,
        );
        for bin in begin_bin..begin_bin + span {
            assert!(
                (halved[bin] * 2.0 - full[bin]).abs() < 1e-5,
                "bin {bin}: amplitude scaling not linear"
            );
        }
    }

    #[test]
    fn generate_channel_coeffs_angle_rotation() {
        // A pure-real carrier with angle = 0.5 (π/2) rotates the coordinate
        // to pure-imaginary: Zr_ch = -Zi·amp·sin, Zi_ch = Zr·amp·cos... at
        // angle 0.5, cos(π·0.5)=0, sin(π·0.5)=1 → Zr_ch = -Zi, Zi_ch = Zr.
        let n = 512;
        let half = n / 2;
        let begin_bin = 13;
        let mut zr = vec![0.0f32; half];
        let mut zi = vec![0.0f32; half];
        zr[begin_bin] = 0.4;
        zi[begin_bin] = 0.0;
        let mut out = vec![0.0f32; half];
        generate_channel_coeffs(&zr, &zi, &[1.0], &[0.5], begin_bin, n, &mut out);
        // Zr_ch = 0.4·0 - 0·1 = 0; Zi_ch = 0·0 + 0.4·1 = 0.4.
        let y_mirror = synthesis_window(half - 1 - begin_bin, n);
        let expected = -2.0 * (0.0 + y_mirror * 0.4);
        assert!((out[begin_bin] - expected).abs() < 1e-5);
    }

    // ---- §E.3.5.5.1 carrier reconstruction ----

    #[test]
    fn reconstruct_carrier_all_zero_is_zero() {
        // No enhanced-coupling energy in any block → the carrier is zero
        // everywhere (the spec's "set to zero" boundary case).
        let z = [0.0f32; 256];
        let car = reconstruct_carrier(&z, &z, &z);
        let max = car
            .zr
            .iter()
            .chain(car.zi.iter())
            .fold(0.0f32, |a, &b| a.max(b.abs()));
        assert!(max < 1e-6, "zero input must give zero carrier, got {max}");
    }

    #[test]
    fn reconstruct_carrier_steady_state_energy_lives_in_active_region() {
        // A steady-state single MDCT bin in the active enhanced-coupling
        // region across all three blocks. With perfect overlap (prev =
        // curr = next) the time-domain reconstruction is non-aliased, and
        // the carrier should carry meaningful (non-zero) energy. We assert
        // the carrier is non-trivial and finite — the exact spectrum is the
        // full §E.3.5.5.1 chain, validated structurally here.
        let mut x = [0.0f32; 256];
        // Bin 100 sits inside the ecpl region (13..=252).
        x[100] = 1.0;
        let car = reconstruct_carrier(&x, &x, &x);
        let energy: f32 = car
            .zr
            .iter()
            .zip(car.zi.iter())
            .map(|(&r, &i)| r * r + i * i)
            .sum();
        assert!(energy > 1e-3, "steady tone produced no carrier energy");
        for (&r, &i) in car.zr.iter().zip(car.zi.iter()) {
            assert!(r.is_finite() && i.is_finite(), "carrier has non-finite bin");
        }
    }

    #[test]
    fn reconstruct_carrier_linear_in_input() {
        // The whole §E.3.5.5.1 chain (IMDCT, overlap-add, window, DFT) is
        // linear, so scaling the input mantissas by k scales Z[k] by k.
        let mut x = [0.0f32; 256];
        let mut s: u32 = 0x00C0_FFEE;
        for v in x.iter_mut().take(253).skip(13) {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            *v = (s as i32 as f32) / (i32::MAX as f32);
        }
        let mut x2 = [0.0f32; 256];
        for (a, b) in x2.iter_mut().zip(x.iter()) {
            *a = b * 2.0;
        }
        let c1 = reconstruct_carrier(&x, &x, &x);
        let c2 = reconstruct_carrier(&x2, &x2, &x2);
        for k in 0..512 {
            assert!(
                (c1.zr[k] * 2.0 - c2.zr[k]).abs() < 2e-4,
                "Zr[{k}] non-linear"
            );
            assert!(
                (c1.zi[k] * 2.0 - c2.zi[k]).abs() < 2e-4,
                "Zi[{k}] non-linear"
            );
        }
    }

    /// `docs/audio/ac3/ac3-errata.md` entry E3 discriminator — the
    /// §E.3.5.5.1 step-3 overlap-add must carry the §7.9.4.1 step-6
    /// factor of 2. Drives a continuous multi-tone through the full
    /// analysis (steps 2-5) → §E.3.5.5.4 synthesis chain at amplitude 1
    /// / angle 0 and least-squares-fits the identity gain over the
    /// region interior:
    ///
    /// * the corrected chain ([`reconstruct_carrier`], with the factor
    ///   of 2) fits gain `1.0` — a unity coordinate reproduces the
    ///   coupling channel at unit scale, matching the Table E3.10
    ///   `ecplamp = 0` = 0 dB ceiling; and
    /// * a local re-implementation of the overlap-add *as printed*
    ///   (steps 2-5 identical, step 3 without the multiplier) fits gain
    ///   exactly `0.5` — the erratum's measured −6 dB defect.
    #[test]
    fn carrier_overlap_add_factor2_erratum_identity() {
        use crate::mdct::mdct_512;

        // Three consecutive windowed-MDCT blocks (256-sample hop) of a
        // continuous multi-tone with energy across the ecpl region
        // (bins ~40-240 of a 512-point MDCT).
        let sig = |n: usize| -> f32 {
            let t = n as f32;
            0.4 * (2.0 * std::f32::consts::PI * 0.08 * t).sin()
                + 0.3 * (2.0 * std::f32::consts::PI * 0.24 * t).sin()
                + 0.2 * (2.0 * std::f32::consts::PI * 0.31 * t + 0.4).sin()
                + 0.15 * (2.0 * std::f32::consts::PI * 0.45 * t + 1.1).sin()
        };
        let mut blocks = Vec::new();
        for blk in 0..3usize {
            let mut win = [0.0f32; 512];
            for n in 0..512 {
                win[n] = sig(blk * 256 + n);
            }
            for n in 0..256 {
                win[n] *= WINDOW[n];
                win[511 - n] *= WINDOW[n];
            }
            let mut c = [0.0f32; 256];
            mdct_512(&win, &mut c);
            // Region-restrict to the full ecpl span (step 1's XCURR
            // definition: zero outside [ecplstartmant, ecplendmant)).
            c[..ECPL_SUBBND_TAB[0]].fill(0.0);
            blocks.push(c);
        }
        let (prev, curr, next) = (&blocks[0], &blocks[1], &blocks[2]);

        // Steps 2-5 with the overlap-add AS PRINTED (no factor of 2);
        // everything else identical to `reconstruct_carrier`.
        let windowed = |x: &[f32; 256]| -> [f32; 512] {
            let mut t = [0.0f32; 512];
            imdct_512_fft(x, &mut t);
            for n in 0..256 {
                t[n] *= WINDOW[n];
                t[511 - n] *= WINDOW[n];
            }
            t
        };
        let (xprev, xcurr, xnext) = (windowed(prev), windowed(curr), windowed(next));
        let mut pcm = [0.0f32; 512];
        for n in 0..256 {
            pcm[n] = xprev[256 + n] + xcurr[n]; // as printed: no `2 *`
            pcm[256 + n] = xcurr[256 + n] + xnext[n];
        }
        let mut re = [0.0f32; 512];
        let mut im = [0.0f32; 512];
        for n in 0..256 {
            let wl = WINDOW[n];
            let wu = WINDOW[256 - n - 1];
            re[n] = pcm[n] * wl * xcos3(n);
            re[256 + n] = pcm[256 + n] * wu * xcos3(256 + n);
            im[n] = pcm[n] * wl * xsin3(n);
            im[256 + n] = pcm[256 + n] * wu * xsin3(256 + n);
        }
        let (zr_p, zi_p) = dft_512_forward(&re, &im);

        // Corrected carrier (the shipping implementation).
        let z = reconstruct_carrier(prev, curr, next);

        // §E.3.5.5.4 synthesis at amp = 1 / angle = 0 for both.
        let begin = ECPL_SUBBND_TAB[0]; // 13
        let end = ECPL_SUBBND_TAB[N_ECPL_SUBBND]; // 253
        let amp = vec![1.0f32; end - begin];
        let ang = vec![0.0f32; end - begin];
        let mut out_corr = [0.0f32; 256];
        generate_channel_coeffs(&z.zr, &z.zi, &amp, &ang, begin, 512, &mut out_corr);
        let mut out_printed = [0.0f32; 256];
        generate_channel_coeffs(&zr_p, &zi_p, &amp, &ang, begin, 512, &mut out_printed);

        // Least-squares identity-gain fit over the region interior
        // (skip 12 bins at each edge: the brick-wall region restriction
        // rings there).
        let fit = |out: &[f32; 256]| -> f64 {
            let mut num = 0.0f64;
            let mut den = 0.0f64;
            for bin in (begin + 12)..(end - 12) {
                num += out[bin] as f64 * curr[bin] as f64;
                den += curr[bin] as f64 * curr[bin] as f64;
            }
            assert!(den > 1e-3, "test signal has no in-region energy");
            num / den
        };
        let g_corr = fit(&out_corr);
        let g_printed = fit(&out_printed);
        assert!(
            (g_corr - 1.0).abs() < 5e-3,
            "corrected overlap-add identity gain {g_corr:.4} != 1.0"
        );
        assert!(
            (g_printed - 0.5).abs() < 2.5e-3,
            "as-printed overlap-add identity gain {g_printed:.4} != 0.5"
        );
        // The ONLY processing difference is the step-3 multiplier, so
        // the two fits differ by exactly that factor.
        assert!(
            (g_corr - 2.0 * g_printed).abs() < 1e-6,
            "printed/corrected chains differ by more than the factor of 2"
        );
    }

    // ---- §E.3.5.5 deferred-synthesis orchestration ----

    /// Build a non-trivial single-block ecpl input over a small active
    /// region (sub-bands 0..2 = bins 13..25) with `nfchans` coupled
    /// channels, channel `0` first-coupled (amp index 0 → unit amplitude,
    /// no angle/chaos), channel `1` carrying explicit angle/chaos.
    fn sample_block(nfchans: usize) -> EcplBlock {
        let begin = 0usize;
        let end = 2usize; // sub-bands 0,1 → bins 13..25
        let necpl = necplbnd(begin, end, &[false; N_ECPL_SUBBND]);
        let strategy = EcplStrategy {
            ecplbegf: 0,
            begin_subbnd: begin,
            end_subbnd: end,
            bndstrc: [false; N_ECPL_SUBBND],
            necplbnd: necpl,
        };
        let mut chincpl = [false; ECPL_MAX_FBW];
        let mut channels = vec![EcplChannelParams::default(); nfchans];
        for ch in 0..nfchans {
            chincpl[ch] = true;
            let p = &mut channels[ch];
            p.param1e = true;
            p.amp = vec![0u8; necpl]; // index 0 → unit amplitude
            if ch > 0 {
                p.param2e = true;
                p.angle = vec![4u8; necpl];
                p.chaos = vec![1u8; necpl];
            }
        }
        let mut mant = [0.0f32; 256];
        let mut s: u32 = 0x1357_9BDF;
        for v in mant.iter_mut().take(end_bin(end)).skip(begin_bin(begin)) {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            *v = (s as i32 as f32) / (i32::MAX as f32);
        }
        EcplBlock {
            mant,
            strategy,
            coords: EcplCoords {
                angleintrp: false,
                channels,
            },
            chincpl,
        }
    }

    #[test]
    fn synthesize_block_zero_carrier_is_zero() {
        let mut st = EcplState::new();
        let blk = sample_block(2);
        let zero = EcplBlock {
            mant: [0.0; 256],
            ..blk.clone()
        };
        let mut out = vec![[0.0f32; 256]; ECPL_MAX_FBW];
        // prev = curr = next = zero mantissas → zero carrier → zero output.
        synthesize_block(&mut st, &zero, &zero, &zero, &mut out, 512);
        for ch in 0..2 {
            for &v in out[ch].iter() {
                assert!(v.abs() < 1e-6, "ch{ch} non-zero from zero carrier");
            }
        }
    }

    #[test]
    fn synthesize_block_leaves_uncoupled_channel_region_untouched() {
        let mut st = EcplState::new();
        let blk = sample_block(2); // chans 0,1 coupled; chan 2 not
        let mut out = vec![[0.0f32; 256]; ECPL_MAX_FBW];
        // Pre-seed channel 2 (not coupled) with a marker in the active bins.
        for bin in begin_bin(0)..end_bin(2) {
            out[2][bin] = 42.0;
        }
        synthesize_block(&mut st, &blk, &blk, &blk, &mut out, 512);
        for bin in begin_bin(0)..end_bin(2) {
            assert_eq!(out[2][bin], 42.0, "uncoupled ch2 bin{bin} overwritten");
        }
        // Coupled channels DID get written somewhere in the region.
        let any0 = (begin_bin(0)..end_bin(2)).any(|b| out[0][b].abs() > 1e-9);
        assert!(any0, "first coupled channel produced no coefficients");
    }

    #[test]
    fn synthesize_block_is_linear_in_carrier() {
        // The whole synthesis chain is linear in the ecpl-channel mantissas
        // (carrier reconstruction + complex product), so scaling all three
        // neighbour buffers by k scales every output coefficient by k.
        let mut st1 = EcplState::new();
        let mut st2 = EcplState::new();
        let blk = sample_block(2);
        let blk2 = EcplBlock {
            mant: std::array::from_fn(|i| blk.mant[i] * 3.0),
            ..blk.clone()
        };
        let mut out1 = vec![[0.0f32; 256]; ECPL_MAX_FBW];
        let mut out2 = vec![[0.0f32; 256]; ECPL_MAX_FBW];
        synthesize_block(&mut st1, &blk, &blk, &blk, &mut out1, 512);
        synthesize_block(&mut st2, &blk2, &blk2, &blk2, &mut out2, 512);
        for ch in 0..2 {
            for bin in begin_bin(0)..end_bin(2) {
                assert!(
                    (out1[ch][bin] * 3.0 - out2[ch][bin]).abs() < 1e-3,
                    "ch{ch} bin{bin} not linear in carrier"
                );
            }
        }
    }

    #[test]
    fn ecpl_state_rand_notrans_generated_once() {
        // §E.3.5.5.3: the non-transient random array is generated once and
        // stays identical across blocks. Two synthesis calls on a
        // non-transient channel must reuse the same cached array.
        let mut st = EcplState::new();
        let _ = st.rand_notrans(1, 512);
        let snapshot: Vec<f32> = (0..256).map(|b| st.rand_notrans(1, 512).get(b)).collect();
        // A second access yields the identical sequence.
        for (b, &want) in snapshot.iter().enumerate() {
            assert_eq!(st.rand_notrans(1, 512).get(b), want);
        }
        // Distinct channels get distinct sequences (spec: unique per ch).
        let ch1_first = st.rand_notrans(1, 512).get(0);
        let ch2_first = st.rand_notrans(2, 512).get(0);
        assert!(ch1_first != ch2_first, "channels share a random sequence");
    }

    #[test]
    fn ecpl_state_prev_frame_mant_roundtrips() {
        // §E.3.5.5.1 cross-frame carry: a fresh state has no carried
        // previous-frame spectrum (block 0's "previous block" defaults to
        // the zero boundary case); set/get round-trips, and a `None` reset
        // restores the boundary case.
        let mut st = EcplState::new();
        assert!(
            st.prev_frame_last_mant().is_none(),
            "fresh state must carry no previous-frame spectrum"
        );
        let carried: [f32; 256] = std::array::from_fn(|i| (i as f32) * 0.5);
        st.set_prev_frame_last_mant(Some(carried));
        assert_eq!(
            st.prev_frame_last_mant().copied(),
            Some(carried),
            "carried spectrum must round-trip"
        );
        st.set_prev_frame_last_mant(None);
        assert!(
            st.prev_frame_last_mant().is_none(),
            "None reset must restore the zero boundary case"
        );
    }

    #[test]
    fn synthesize_block_consults_prev_neighbour() {
        // The §E.3.5.5.1 carrier of the current block depends on the
        // *previous* block's spectrum (it suppresses time-domain aliasing).
        // A non-zero `prev` (as the cross-frame carry supplies for block 0)
        // must therefore change the synthesised output versus a zero `prev`
        // — proving the previous-frame edge is actually threaded through.
        let curr = sample_block(2);
        let mut zero_prev = curr.clone();
        zero_prev.mant = [0.0; 256];
        let mut nonzero_prev = curr.clone();
        nonzero_prev.mant = std::array::from_fn(|i| sample_block(2).mant[i] * 0.75);

        let mut st_zero = EcplState::new();
        let mut st_carry = EcplState::new();
        let mut out_zero = vec![[0.0f32; 256]; ECPL_MAX_FBW];
        let mut out_carry = vec![[0.0f32; 256]; ECPL_MAX_FBW];
        // next = zero in both (the last-block boundary case) isolates the
        // contribution of the previous-block spectrum.
        let zero_next = zero_prev.clone();
        synthesize_block(
            &mut st_zero,
            &zero_prev,
            &curr,
            &zero_next,
            &mut out_zero,
            512,
        );
        synthesize_block(
            &mut st_carry,
            &nonzero_prev,
            &curr,
            &zero_next,
            &mut out_carry,
            512,
        );

        let mut diff = 0.0f32;
        for ch in 0..2 {
            for bin in begin_bin(0)..end_bin(2) {
                diff += (out_zero[ch][bin] - out_carry[ch][bin]).abs();
            }
        }
        assert!(
            diff > 1e-3,
            "non-zero previous-block spectrum did not affect synthesis (diff={diff})"
        );
    }
}

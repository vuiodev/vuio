//! E-AC-3 audio-block DSP pipeline — rounds 2 / 4-stub / 5 / 6 / 7-SPX.
//!
//! Translates the parsed [`super::bsi::Bsi`] + [`super::audfrm::AudFrm`]
//! into the existing AC-3 [`crate::audblk::Ac3State`] shape so the §7
//! DSP helpers (`decode_exponents`, `run_bit_allocation`,
//! `unpack_mantissas`, `dsp_block`) can be reused without modification.
//!
//! ## Round 7 (this commit) — Spectral Extension (SPX) decode
//!
//! The audblk parser now decodes the full §E.2.3.3 SPX strategy +
//! coordinate syntax (replacing the round-4 `spxinu == 1` mute):
//! `chinspx[ch]`, `spxstrtf`, `spxbegf`, `spxendf`, `spxbndstrce` +
//! `spxbndstrc[]` (with the Table E2.11 default banding), and the
//! per-channel coordinate block `spxcoe` / `spxblnd` / `mstrspxco` /
//! `spxcoexp` / `spxcomant`. SPX-channel `endmant` is set to the SPX
//! begin frequency (§E.3.3.3), `chbwcod` is skipped for SPX channels,
//! `cplendf` is derived from `spxbegf` when SPX is in use (§E.3.3.1),
//! and `nrematbd` folds in SPX (§E.3.3.2) — three derivations that
//! previously drifted the bit cursor on SPX frames. The §E.3.6
//! high-frequency regeneration itself (coefficient translation, noise
//! blending, banded RMS scaling, coordinate scaling) runs in
//! [`crate::audblk::dsp_block`] via `apply_spectral_extension`.
//!
//! ## Adaptive Hybrid Transform (AHT) — multichannel fbw (round 110)
//!
//! Multichannel full-bandwidth AHT decode. The audblk loop keeps a
//! per-channel AHT-coefficient cache (`aht_coeffs[ch][blk][bin]`)
//! populated by [`unpack_mixed_mantissas`] on the FIRST AHT-active block
//! per channel; subsequent blocks load coefficients from the cache and
//! emit zero mantissa bits for that channel. The audfrm parser is split
//! into phase A ([`super::audfrm::parse_with`]) + phase B
//! ([`super::audfrm::parse_phase_b`]) so the dsp can hand it the §3.4.2
//! helper variables `nchregs[ch]` / `ncplregs` / `nlferegs`. Round 6
//! shipped mono-only by hardcoding `nchregs[0] = 1`; round 110 computes
//! all three regs directly from the already-parsed per-block exponent
//! strategies ([`compute_aht_regs`]) — no real audblk pre-walk needed —
//! so every fbw channel with `nchregs[ch] == 1` takes the AHT path.
//! The non-AHT (standard scalar) channels in a mixed frame now share
//! the bap-1/2/4 grouping buffers across channels in frequency-then-
//! channel order via the canonical [`crate::audblk::fetch_mantissa`],
//! matching base AC-3 §7.3.5 (round 6's per-channel grouping was correct
//! only for the mono case). LFE-AHT (`lfeahtinu`) synthesis decodes as of
//! round 113 (and the previously-skipped standard LFE mantissa read in
//! the AHT path is fixed). Coupling-AHT (`cplahtinu`) synthesis decodes
//! as of round 117: the coupling-channel AHT mantissa block (the
//! `cplgaqmod` word, gain words, 6×ncplmant VQ/GAQ mantissas, then IDCT
//! per §3.4) is read inline with the first coupled fbw channel — gated by
//! `got_cplchan` exactly as the base-AC-3 mantissa loop — over the
//! coupling range `[cpl_begf_mant, cpl_endf_mant)`, and its per-block
//! coefficients are loaded into the coupling pseudo-channel slot before
//! the §7.4 decouple step. No AHT flag is rejected by the dsp any more.
//!
//! Per-bin AHT decode flow:
//!
//! 1. Derive `hebap[bin]` from `psd[bin]` / `mask[masktab[bin]]` via
//!    [`super::aht::hebap_from_address`] (Table E3.1).
//! 2. Read 2-bit `chgaqmod` + `chgaqsections` 1- or 5-bit gain words.
//! 3. Per bin:
//!    * `hebap == 0` → zero coefficients across all 6 blocks.
//!    * `1 ≤ hebap ≤ 7` → 2..9-bit VQ codeword indexes into Tables
//!      E4.1..E4.7, returns 6 dequantised mantissas.
//!    * `hebap ≥ 8` → 6 scalar / GAQ-tagged mantissa reads (with the
//!      Gk gain factor applied per Table E3.5).
//! 4. Apply the §3.4.5 inverse DCT-II over the 6 mantissas to
//!    recover per-block C(k, m).
//! 5. Multiply by `2^-exp` and stash in `aht_coeffs[ch][blk][bin]`.
//!
//! ## Round 5 — standard coupling
//!
//! The audblk syntax for **standard** (non-enhanced) coupling per
//! Table E1.4 is wired end-to-end:
//!
//! * `cplstre[blk]` + `cplinu[blk]` come from [`AudFrm::cplstre_blk`]
//!   / [`AudFrm::cplinu_blk`] (audfrm-resident in Annex E, vs.
//!   audblk-resident in base AC-3).
//! * When `cplstre[blk] && cplinu[blk]`: parse `ecplinu` (1 bit),
//!   `chincpl[ch]` (per fbw channel, 1 bit each — implicit `1` for
//!   2/0), `phsflginu` (1 bit, only in 2/0), `cplbegf`/`cplendf`
//!   (4 bits each), `cplbndstrce` (1 bit) + per-subband
//!   `cplbndstrc[bnd]` (1 bit).
//! * When `cplinu[blk]`: parse `cplcoe[ch]` (1 bit, implicit `1` if
//!   `firstcplcos[ch]`) + `mstrcplco`/`cplcoexp`/`cplcomant` (4+4
//!   bits per band) + `phsflg[bnd]` (1 bit, only in 2/0).
//! * Coupling-channel exponents (`cplabsexp` 4 bits + grouped exps),
//!   `chbwcod[ch]` only for un-coupled channels, `cplleake` +
//!   `cplfleak`/`cplsleak`, `cpldeltbae` (delta-BA for the cpl
//!   channel) — all wired through to the existing [`Ac3State`] slots
//!   so [`audblk::dsp_block`] runs the §7.4 decouple step unchanged.
//!
//! `ecplinu == 1` (enhanced coupling, §E.3.5.5) decodes through a
//! deferred two-pass path: the per-block loop parses the strategy +
//! coordinates and decodes the enhanced-coupling channel into the
//! `MAX_FBW` slot (pinning the coupling region at the ecpl bins), then
//! [`run_deferred_ecpl_dsp`] reconstructs the §E.3.5.5.1 carrier from
//! prev/curr/next blocks and emits each coupled channel's coefficients
//! via [`super::ecpl::synthesize_block`] before the §7.4 decouple is
//! skipped. None of the validator-encoded corpus fixtures exercise the
//! ecpl path (the corpus encoder emits only standard coupling), so
//! the synthesis is covered by [`super::ecpl`] unit tests; standard
//! coupling covers all four 5.1 / low-rate fixtures
//! (eac3-5.1-48000-384kbps, eac3-5.1-side-768kbps,
//! eac3-low-rate-stereo-64kbps, eac3-from-ac3-bitstream-recombination).
//!
//! Other newly-handled fields:
//! * `convsnroffste` (1 bit, always present for `strmtyp == 0`) +
//!   optional 10-bit `convsnroffst` — was silently missing from
//!   round 2. Most fixtures had it = 0 so the missing bit aliased
//!   onto the next field cleanly, but coupled fixtures hit `cplleake`
//!   right after which made the misalignment visible.
//!
//! ## Round 4 (prior commit)
//!
//! AHT and SPX are E-AC-3-specific psychoacoustic features that gate
//! decode for any fixture using them. The round-4 **stub** in this
//! commit:
//!
//! * Surfaces `ahte == 1` as a clear `Error::Unsupported` from the
//!   audfrm parser instead of silently skipping bits (round 1
//!   incorrectly consumed AHT bits as if they were always-emitted).
//! * Tightens the `spxinu == 1` rejection in the audblk parser with
//!   a spec citation (§E.2.2.5.4).
//! * Documents the path forward: a real round-4-bis lands the §E.2.2.4
//!   Karhunen-Loeve VQ codebooks (AHT) and the §E.2.2.5.4 SBR-style
//!   parametric high-frequency reconstruction (SPX). Both are
//!   substantial: AHT requires a 2-pass audblk decode (scan
//!   chexpstr, then re-walk for AHT in-use bits) plus the VQ
//!   codebook tables themselves; SPX needs the spxcoexp/spxcomant
//!   coordinate decoding + the noise-blend / amplitude-fold
//!   reconstruction pipeline.
//!
//! Fixtures unblocked by round 4-bis: `eac3-low-bitrate-32kbps` (AHT
//! at low bit budgets per its `notes.md`). No corpus fixture
//! exercises SPX (the validator-encoded corpus omits it).
//!
//! ## Scope (round 2)
//!
//! The round-2 DSP path covers the **simple-encoder happy path** that
//! the bulk of the corpus fixtures actually exercise:
//!
//! * `expstre == 1`  — per-block per-channel chexpstr (no frame-level
//!   strategy run; we don't currently translate the §E.1.3.4.4
//!   chexpstr-codeword runs from frmchexpstr into per-block strategies).
//! * `bamode  == 1`  — per-block bit-allocation parametric info.
//! * `dithflage == 1` — per-block per-channel dithflag (otherwise spec
//!   says implicit 1).
//! * `blkswe   == 1` — per-block blksw (otherwise implicit 0).
//! * `dbaflde  == 1` — delta-bit-allocation may appear per block.
//! * `skipflde == 1` — skip-field may appear per block.
//! * `snroffststr == 0` — single frame-level (csnroffst, fsnroffst).
//! * Standard coupling (round 5) + enhanced coupling (§E.3.5.5,
//!   deferred two-pass synthesis).
//! * No spectral extension (`spxinu == 0` always).
//! * No AHT (`ahte == 0` — gated upstream in audfrm parser).
//! * Transient pre-noise processing (`transproce == 1`) is decoded via
//!   the §E.3.7.2 PCM-domain synthesis (round 103); no longer rejected.
//!
//! When any of these conditions is violated the parser returns
//! [`oxideav_core::Error::Unsupported`] and the caller (the decoder
//! in [`super::decoder`]) falls back to silent emit for that frame.
//!
//! ## Bit syntax — Table E.1.4 (audblk()) verbatim, simplified
//!
//! Per ETSI TS 102 366 V1.4.1 §E.1.2.4 / ATSC A/52:2018 Table E1.4 the
//! per-block-per-channel `chexpstr`, per-block `cplexpstr`, and
//! per-block `lfeexpstr` strategy codes are emitted in **audfrm**
//! (Table E.1.3, gated by `expstre`), NOT in audblk. Audblk merely
//! consumes them as state via the `chexpstr[blk][ch] != reuse` /
//! `cplexpstr[blk] != reuse` / `lfeexpstr[blk] != reuse` gates that
//! decide whether the bandwidth code + exponent payload follow.
//!
//! ```text
//!   if (blkswe)   for (ch=0..nfchans) blksw[ch]    1
//!   if (dithflage) for (ch=0..nfchans) dithflag[ch] 1
//!   dynrnge                                          1
//!   if (dynrnge) dynrng                              8
//!   if (acmod==0) {
//!     dynrng2e                                       1
//!     if (dynrng2e) dynrng2                          8
//!   }
//!   if (blk == 0) spxstre = 1
//!   else          spxstre                            1
//!   if (spxstre)  spxinu (+ spx fields when set)     1+
//!   if (cplstre[blk])  ... coupling strategy fields ...   (cplstre/cplinu in audfrm)
//!   if (cplinu[blk])   ... coupling coordinates ...
//!   if (acmod==2) {
//!     if (blk == 0) rematstr = 1                       (implicit)
//!     else          rematstr                          1
//!     if (rematstr) rematflg[0..n]                    1 each
//!   }
//!   /* §E.1.2.4 chbwcod — gated by audfrm-supplied chexpstr */
//!   for (ch) if (chexpstr[blk][ch] != reuse && !chincpl && !chinspx)
//!                       chbwcod[ch]                  6
//!   /* exponents */
//!   if (cplinu[blk] && cplexpstr[blk] != reuse) — coupling exponents (cplabsexp + groups)
//!   for (ch) if (chexpstr[blk][ch] != reuse)    — exps[ch][0] + groups + gainrng[ch] (2)
//!   if (lfeon && lfeexpstr[blk] != reuse)       — LFE exponents (no gainrng)
//!   /* bit-allocation parametric */
//!   if (bamode) {
//!     baie                                            1
//!     if (baie) sdcycod(2) fdcycod(2) sgaincod(2) dbpbcod(2) floorcod(3)
//!   }
//!   if (snroffststr==0) — uses frame-level (csnroffst, fsnroffst).
//!   else                — per-block snroffste etc. (NOT IN ROUND 2)
//!   if (frmfgaincode)   — fgaincode (1 bit) per block, then 3 bits per channel if set.
//!   if (strmtyp == 0)   — convsnroffste (1 bit) [+10-bit convsnroffst when set]
//!   if (cplinu[blk]) — cplleak block (cplleake + cplfleak/cplsleak).
//!   /* dba */
//!   if (dbaflde) {
//!     deltbaie                                        1
//!     ... (same as AC-3) ...
//!   }
//!   if (skipflde) {
//!     skiple                                          1
//!     if (skiple) skipl(9) + skipfld(skipl*8)
//!   }
//!   /* mantissas — bap-driven, identical to AC-3 unpack_mantissas. */
//! ```

use oxideav_core::bits::BitReader;
use oxideav_core::{Error, Result};

use crate::audblk::{self, Ac3State, BLOCKS_PER_FRAME, MAX_FBW, N_COEFFS, SAMPLES_PER_BLOCK};
use crate::bsi::Bsi as Ac3Bsi;
use crate::syncinfo::SyncInfo;

use super::aht::{self, AHT_BLOCKS};
use super::audfrm::{self, AhtRegsHints, AudFrm};
use super::bsi::{Bsi as Eac3Bsi, StreamType};

/// Default spectral-extension banding structure `defspxbndstrc[]` per
/// Table E2.11. Indexed by absolute SPX sub-band number; a `true` (the
/// '1' entries at sub-bands 8, 10, 12, 14, 16) means "merge into the
/// previous band". Used the first time SPX is active in a frame when
/// `spxbndstrce == 0`.
pub const DEFAULT_SPX_BNDSTRC: [bool; 18] = {
    let mut t = [false; 18];
    t[8] = true;
    t[10] = true;
    t[12] = true;
    t[14] = true;
    t[16] = true;
    t
};

/// §E.2.3.3.15 Table E2.12 — Default Coupling Banding Structure
/// `defcplbndstrc[]`. Indexed by the **absolute** coupling sub-band
/// number (0..17). A `true` entry means that sub-band is merged into
/// the previous band rather than starting a new band. Used when
/// `cplbndstrce == 0` in the first coupling block of a frame (Annex E
/// standard coupling); base AC-3 always transmits the structure
/// explicitly and never consults this table.
const DEFCPLBNDSTRC: [bool; 18] = {
    let mut t = [false; 18];
    t[8] = true;
    t[10] = true;
    t[11] = true;
    t[13] = true;
    t[14] = true;
    t[15] = true;
    t[16] = true;
    t[17] = true;
    t
};

/// Decode one E-AC-3 independent substream's audblks into interleaved
/// f32 PCM. Returns `Ok(())` on a successful clean walk, or `Err(...)`
/// if any block hits a feature we don't support (caller substitutes
/// silence).
///
/// `out` length must equal `bsi.num_blocks * 256 * bsi.nchans`.
pub fn decode_indep_audblks(
    bsi: &Eac3Bsi,
    audfrm: &AudFrm,
    br: &mut BitReader<'_>,
    state: &mut Ac3State,
    out: &mut [f32],
) -> Result<()> {
    // Phase-B audfrm finalisation when AHT is in use. The audfrm parser
    // stopped at the AHT anchor so the dsp can compute the §3.4.2 helper
    // variables `nchregs[ch]` / `ncplregs` / `nlferegs` — the number of
    // times each channel transmits exponents in the 6-block frame. These
    // are NOT in the bitstream; they are derived from the per-block
    // exponent strategies that audfrm already parsed (`chexpstr_blk_ch`,
    // `cplexpstr_blk` + `cplstre_blk`, `lfeexpstr`), so no real audblk
    // pre-walk is needed — every input is available on `AudFrm`.
    //
    // `parse_phase_b` then reads `chahtinu[ch]` for every fbw channel
    // with `nchregs[ch] == 1` (multichannel-capable as of round 110),
    // plus `cplahtinu` / `lfeahtinu` when their regs gate fires.
    // fbw AHT (round 110), LFE AHT (round 113), and coupling-AHT
    // (round 117) all decode now, so no AHT flag is rejected here.
    let mut audfrm_local;
    let audfrm: &AudFrm = if audfrm.aht_phase_b_pending {
        audfrm_local = audfrm.clone();
        let hints = compute_aht_regs(&audfrm_local, bsi);
        audfrm::parse_phase_b(br, &mut audfrm_local, bsi, &hints)?;
        &audfrm_local
    } else {
        audfrm
    };

    // Reject cases the parser does not handle.
    reject_unsupported(bsi, audfrm)?;

    // Build a "shim" AC-3 BSI + SyncInfo so the reused helpers see the
    // shape they expect.
    let ac3_bsi = build_ac3_bsi_shim(bsi);
    let si = build_syncinfo_shim(bsi);

    // Top-level frame init mirrors §7.2.2.6: clear delta-segment counts.
    for n in state.deltnseg.iter_mut() {
        *n = 0;
    }

    let nfchans = bsi.nfchans as usize;
    let nchans = bsi.nchans as usize;
    let lfeon = bsi.lfeon;
    let num_blocks = bsi.num_blocks as usize;
    let strmtyp_indep = matches!(bsi.strmtyp, StreamType::Independent);
    let _ = BLOCKS_PER_FRAME; // unused once cplinu_blk migrated to audfrm

    // §E.2.3.2 / §E.1.2.4 — per-frame syntax-state initialisation. The
    // audfrm finishes by setting `firstcplcos[ch] = 1` for every fbw
    // channel and `firstcplleak = 1`; both are stateful "have we seen
    // a coupling-coordinate / leak-init block yet this frame" flags
    // that gate whether the audblk reads the explicit `cplcoe[ch]` /
    // `cplleake` bit or substitutes an implicit `1`. They reset every
    // syncframe so we keep them as locals here rather than on `state`.
    let mut firstcplcos: [bool; MAX_FBW] = [true; MAX_FBW];
    let mut firstcplleak = true;
    // §E.2.3.3.15 — "have we parsed coupling-banding-structure in any
    // earlier block of THIS frame yet". The default-band-structure
    // substitution (Table E2.12) only applies when `cplbndstrce == 0`
    // in the FIRST coupling block of the frame; a later `cplbndstrce ==
    // 0` reuses the previous block's structure instead.
    let mut first_cpl_strategy_block = true;
    // §E.2.3.3.9 firstspxcos[ch] — "have we seen explicit SPX
    // coordinates for channel ch yet this frame". Resets every
    // syncframe; the first block in which a channel is in SPX carries an
    // implicit `spxcoe[ch] = 1`.
    let mut firstspxcos: [bool; MAX_FBW] = [true; MAX_FBW];

    // ---- §E.3.5.5 enhanced-coupling per-frame state ----
    //
    // `ecpl_in_use` tracks whether the active coupling strategy is the
    // enhanced (`ecplinu == 1`) variant; it persists across `cplstre == 0`
    // reuse blocks exactly like the standard-coupling strategy fields.
    // `ecpl_strategy` holds the band geometry; `ecpl_coords` the per-block
    // amplitude/angle/chaos triples. `ecpl_prev_bndstrc` carries the
    // previous block's banding so a `ecplbndstrce == 0` block reuses it
    // (default on the first ecpl block of the frame, §E.2.3.3.18).
    //
    // When any block uses enhanced coupling the per-block DSP is deferred:
    // each block's de-normalised ecpl-channel coefficients are snapshotted
    // into `ecpl_blocks[blk]` and the per-channel synthesis (§E.3.5.5.1
    // carrier from prev/curr/next + §E.3.5.5.4 product) runs in a second
    // pass after the block loop, because the §E.3.5.5.1 carrier needs the
    // *next* block's coefficients which are not yet decoded mid-loop.
    let mut ecpl_in_use = false;
    let mut ecpl_strategy: Option<super::ecpl::EcplStrategy> = None;
    let mut ecpl_coords: Option<super::ecpl::EcplCoords> = None;
    let mut ecpl_prev_bndstrc = super::ecpl::DEFAULT_ECPL_BNDSTRC;
    // Per-block deferred-synthesis snapshots; `Some` only for ecpl blocks.
    let mut ecpl_blocks: Vec<Option<super::ecpl::EcplBlock>> = vec![None; num_blocks];
    // Whether the frame used enhanced coupling in any block — once true,
    // ALL remaining blocks defer their DSP so the overlap-add delay line
    // renders in order across the second pass.
    let mut frame_has_ecpl = false;
    // Absolute block index where deferral began (= first ecpl block). The
    // deferred pass maps snapshot index `i` to block `first_deferred_blk + i`.
    let mut first_deferred_blk = 0usize;
    // Per-deferred-block full channel-state snapshots + the decouple-skip
    // flag (true for ecpl blocks). Only populated once `frame_has_ecpl`.
    let mut deferred_channels: Vec<[crate::audblk::ChannelState; crate::audblk::MAX_CHANNELS]> =
        Vec::new();
    let mut deferred_skip_decouple: Vec<bool> = Vec::new();

    // §7.2.2.6 — clear leftover coupling state at the top of every
    // frame so a previous frame's `cpl_in_use` doesn't leak forward
    // when this frame's blk 0 has `cplinu == 0`.
    state.cpl_in_use = false;
    for ch in 0..MAX_FBW {
        state.channels[ch].in_coupling = false;
    }

    // §E.3.6 — clear SPX state at the top of every frame so a previous
    // frame's spxinu / band structure doesn't leak forward.
    state.spx_in_use = false;
    for ch in 0..MAX_FBW {
        state.channels[ch].in_spx = false;
    }

    // §3.6.4.2.3 — propagate the frame-scoped `chinspxatten[ch]` +
    // `spxattencod[ch]` (read by the audfrm parser, Table E1.3) onto
    // the per-channel state. The SPX synthesis step
    // (`audblk::apply_spectral_extension`) consults these to apply the
    // 5-tap border notch filter at the baseband/extension boundary and
    // every translation-copy wrap point. When `spxattene == 0` for the
    // frame, both flags stay cleared (no notch applied).
    for ch in 0..nfchans {
        if audfrm.spxattene {
            state.channels[ch].spx_atten_active = audfrm.chinspxatten[ch];
            state.channels[ch].spx_atten_code = audfrm.spxattencod[ch];
        } else {
            state.channels[ch].spx_atten_active = false;
            state.channels[ch].spx_atten_code = 0;
        }
    }

    // ---- §3.4 AHT pre-buffered coefficients ----
    //
    // When `chahtinu[ch] == 1`, the audblk that decodes the FIRST
    // non-`REUSE` exponent strategy for channel `ch` reads ALL 6×nmant
    // AHT mantissas + GAQ side info up front. Subsequent audblks for
    // that channel emit no mantissa bits — they pull their per-block
    // coefficient values from this `aht_coeffs[ch][blk][bin]` cache,
    // which holds the post-IDCT / post-`*2^-exp` floating coefficients.
    //
    // `aht_pending[ch] == true` for channels that have `chahtinu == 1`
    // (or `lfeahtinu == 1` at the LFE slot) AND haven't emitted their
    // AHT mantissa block yet this frame. `aht_filled[ch] == true` once
    // the cache is populated.
    //
    // The arrays carry one slot per `state.channels` index — fbw 0..5,
    // the coupling pseudo-channel at `MAX_FBW`, and the LFE channel at
    // `MAX_FBW + 1` — so the LFE-AHT path (round 113) and the
    // coupling-AHT path (round 117) share the same cache+flag machinery
    // as the fbw channels. The `MAX_FBW` (coupling) slot is armed when
    // `cplahtinu == 1`; the coupling mantissa block is read interleaved
    // with the first coupled fbw channel inside `unpack_mixed_mantissas`.
    // ~42 KB total (7 slots × 6 blks × 256 bins × 4 B). Heap-allocate so
    // we don't blow the audio thread's modest stack budget.
    const AHT_SLOTS: usize = MAX_FBW + 2;
    let lfe_slot = MAX_FBW + 1;
    let mut aht_coeffs: Vec<[[f32; N_COEFFS]; AHT_BLOCKS]> =
        vec![[[0.0; N_COEFFS]; AHT_BLOCKS]; AHT_SLOTS];
    let mut aht_pending: [bool; AHT_SLOTS] = [false; AHT_SLOTS];
    let mut aht_filled: [bool; AHT_SLOTS] = [false; AHT_SLOTS];
    if audfrm.ahte {
        aht_pending[..nfchans].copy_from_slice(&audfrm.chahtinu[..nfchans]);
        aht_pending[MAX_FBW] = audfrm.cplahtinu;
        if lfeon {
            aht_pending[lfe_slot] = audfrm.lfeahtinu;
        }
    }

    for blk in 0..num_blocks {
        state.blkidx = blk;

        // ---- §E.1.3.1 blksw[ch] ----
        if audfrm.blkswe {
            for ch in 0..nfchans {
                let v = br.read_u32(1)? != 0;
                state.channels[ch].blksw = v;
            }
        } else {
            for ch in 0..nfchans {
                state.channels[ch].blksw = false;
            }
        }

        // ---- §E.1.3.2 dithflag[ch] ----
        if audfrm.dithflage {
            for ch in 0..nfchans {
                let v = br.read_u32(1)? != 0;
                state.channels[ch].dithflag = v;
            }
        } else {
            for ch in 0..nfchans {
                state.channels[ch].dithflag = true;
            }
        }

        // ---- dynrng ----
        // The §6.1.9 / §7.7 DRC control surface maps the dynrng word
        // (line-out / partial-compression) or the frame-level compr word
        // (RF mode, §7.7.2.1) to the applied linear gain. Default is
        // line-out (full dynrng), so existing E-AC-3 decodes are
        // unaffected unless the caller opts in.
        let compr_ch1 = bsi.compr.map(|c| c.raw());
        let compr_ch2 = bsi.compr_ch2.map(|c| c.raw());
        let dynrnge = br.read_u32(1)? != 0;
        if dynrnge {
            let dynrng = br.read_u32(8)? as u8;
            let g = state.drc.resolve_block_gain(dynrng, compr_ch1);
            for ch in 0..nfchans {
                state.channels[ch].dynrng = g;
            }
        } else if blk == 0 {
            let g = state.drc.resolve_block_gain(0x00, compr_ch1);
            for ch in 0..nfchans {
                state.channels[ch].dynrng = g;
            }
        }
        if bsi.acmod == 0 {
            let dynrng2e = br.read_u32(1)? != 0;
            if dynrng2e {
                let d2 = br.read_u32(8)? as u8;
                state.channels[1].dynrng = state.drc.resolve_block_gain(d2, compr_ch2);
            } else if blk == 0 {
                state.channels[1].dynrng = state.drc.resolve_block_gain(0x00, compr_ch2);
            }
        }

        // ---- spectral extension strategy block (§E.1.3.5 / §E.3.6) ----
        //
        // Per Table E1.4, blk 0 has implicit `spxstre = 1` with the
        // 1-bit `spxinu[0]` emitted directly; subsequent blocks emit
        // `spxstre[blk]` (1 bit) + (only if spxstre[blk]) `spxinu[blk]`.
        //
        // The §E.3.6 SPX decode is a parametric high-frequency
        // reconstruction (the E-AC-3 analogue of SBR): the band
        // [spx_begin .. spx_end) is copied from low-frequency bins,
        // blended with banded noise, and scaled by per-band coordinates.
        // The strategy fields here set up the sub-band → band geometry;
        // the per-channel coordinate block (below) supplies spxco /
        // spxblnd; the synthesis itself runs in `audblk::dsp_block`.
        let spxstre = if blk == 0 { true } else { br.read_u32(1)? != 0 };
        if spxstre {
            let spxinu = br.read_u32(1)? != 0;
            state.spx_in_use = spxinu;
            if spxinu {
                // §E.2.3.3.3 chinspx[ch].
                if bsi.acmod == 0x1 {
                    state.channels[0].in_spx = true;
                } else {
                    for ch in 0..nfchans {
                        state.channels[ch].in_spx = br.read_u32(1)? != 0;
                    }
                }
                // §E.2.3.3.4-6 spxstrtf (2) + spxbegf (3) + spxendf (3).
                state.spx_strtf = br.read_u32(2)? as u8;
                let spxbegf = br.read_u32(3)? as usize;
                let spxendf = br.read_u32(3)? as usize;
                state.spx_begin_subbnd = if spxbegf < 6 {
                    spxbegf + 2
                } else {
                    spxbegf * 2 - 3
                };
                state.spx_end_subbnd = if spxendf < 3 {
                    spxendf + 5
                } else {
                    spxendf * 2 + 3
                };
                if state.spx_end_subbnd <= state.spx_begin_subbnd || state.spx_end_subbnd > 17 {
                    return Err(Error::invalid(
                        "eac3 audblk: SPX sub-band range invalid (end <= begin or > 17)",
                    ));
                }
                // §E.2.3.3.7-8 spxbndstrce + spxbndstrc[bnd]. When the
                // exist bit is 0 in the first SPX block use the default
                // banding (Table E2.11); in later blocks reuse the prior
                // structure (already on `state.spx_bndstrc`).
                let spxbndstrce = br.read_u32(1)? != 0;
                if spxbndstrce {
                    state.spx_bndstrc = [false; 18];
                    for bnd in (state.spx_begin_subbnd + 1)..state.spx_end_subbnd {
                        state.spx_bndstrc[bnd] = br.read_u32(1)? != 0;
                    }
                } else if firstspxcos.iter().take(nfchans).all(|&f| f) {
                    // First SPX block this frame, no explicit structure →
                    // default banding per Table E2.11 (merge bit set on
                    // odd sub-bands 8,10,12,14,16).
                    state.spx_bndstrc = DEFAULT_SPX_BNDSTRC;
                }
                // Derive nspxbnds + per-band size (§E.3.6.2).
                let mut nspxbnds = 1usize;
                let mut sztab = [0usize; 18];
                sztab[0] = 12;
                for bnd in (state.spx_begin_subbnd + 1)..state.spx_end_subbnd {
                    if !state.spx_bndstrc[bnd] {
                        sztab[nspxbnds] = 12;
                        nspxbnds += 1;
                    } else {
                        sztab[nspxbnds - 1] += 12;
                    }
                }
                state.spx_nbnds = nspxbnds;
                state.spx_bndsztab = sztab;
            } else {
                // §E.2.3.3.2 — SPX not in use this block; clear per-ch
                // flags and arm firstspxcos for the next active block.
                for ch in 0..nfchans {
                    state.channels[ch].in_spx = false;
                    firstspxcos[ch] = true;
                }
            }
        }

        // ---- spectral extension coordinates (§E.1.3.5 / §E.3.6.3) ----
        if state.spx_in_use {
            for ch in 0..nfchans {
                if state.channels[ch].in_spx {
                    // §E.2.3.3.9 spxcoe[ch] — implicit 1 on the first SPX
                    // block for this channel; explicit thereafter.
                    let spxcoe = if firstspxcos[ch] {
                        firstspxcos[ch] = false;
                        true
                    } else {
                        br.read_u32(1)? != 0
                    };
                    if spxcoe {
                        // §E.2.3.3.10-11 spxblnd (5) + mstrspxco (2).
                        let spxblnd = br.read_u32(5)? as f32;
                        let mstrspxco = br.read_u32(2)? as i32;
                        let noffset = spxblnd / 32.0;
                        // §E.3.6.4.2.1 blend factors per band.
                        let spx_begin_tc = 25 + 12 * state.spx_begin_subbnd;
                        let spx_end_tc = 25 + 12 * state.spx_end_subbnd;
                        let mut spxmant = spx_begin_tc as f32;
                        for bnd in 0..state.spx_nbnds {
                            let bandsize = state.spx_bndsztab[bnd] as f32;
                            let mut nratio =
                                (spxmant + 0.5 * bandsize) / spx_end_tc as f32 - noffset;
                            nratio = nratio.clamp(0.0, 1.0);
                            state.channels[ch].spx_nblend[bnd] = nratio.sqrt();
                            state.channels[ch].spx_sblend[bnd] = (1.0 - nratio).sqrt();
                            spxmant += bandsize;
                        }
                        // §E.2.3.3.12-13 + §E.3.6.3 per-band coordinate.
                        for bnd in 0..state.spx_nbnds {
                            let spxcoexp = br.read_u32(4)? as i32;
                            let spxcomant = br.read_u32(2)? as f32;
                            let temp = if spxcoexp == 15 {
                                spxcomant / 4.0
                            } else {
                                (spxcomant + 4.0) / 8.0
                            };
                            let shift = spxcoexp + 3 * mstrspxco;
                            state.channels[ch].spx_coord[bnd] = temp * 2f32.powi(-shift);
                        }
                    }
                    // spxcoe == 0 → reuse the prior block's coordinates +
                    // blend factors (already on `state.channels[ch]`).
                } else {
                    firstspxcos[ch] = true;
                }
            }
        }

        // ---- coupling strategy block (Table E1.4 + §E.1.3.3.5) ----
        //
        // Per §E.1.2 / Table E1.3, `cplstre[blk]` + `cplinu[blk]` are
        // emitted in **audfrm** (already parsed; surfaced as
        // [`AudFrm::cplstre_blk`] / [`AudFrm::cplinu_blk`]). The audblk
        // only carries the **strategy details** (chincpl, cplbegf,
        // cplendf, cplbndstrc) when `cplstre[blk] && cplinu[blk]`,
        // and the **coordinate block** (cplcoe, mstrcplco, cplcoexp,
        // cplcomant, phsflg) whenever `cplinu[blk]`.
        let cplinu = audfrm.cplinu_blk[blk];
        let cplstre = audfrm.cplstre_blk[blk];
        if cplstre {
            if cplinu {
                // §E.1.3.3.6 ecplinu — enhanced coupling flag.
                let ecplinu = br.read_u32(1)? != 0;
                // §E.1.3.3.7 chincpl[ch] — per the Annex E audblk syntax
                // the per-channel in-coupling flags follow `ecplinu`
                // immediately, BEFORE the standard/enhanced arm split:
                // implicit 1 for both channels in 2/0, explicit 1-bit
                // field per fbw channel otherwise. (An earlier revision
                // read them after the enhanced-coupling begin/end/banding
                // fields, which desynced the cursor on any multichannel
                // — acmod > 2 — enhanced-coupling frame; harmless in 2/0
                // where no bits are transmitted. Pinned by the 5.1
                // enhanced-coupling encoder round-trip.)
                if bsi.acmod == 0x2 {
                    state.channels[0].in_coupling = true;
                    state.channels[1].in_coupling = true;
                } else {
                    for ch in 0..nfchans {
                        let v = br.read_u32(1)? != 0;
                        state.channels[ch].in_coupling = v;
                    }
                }
                if ecplinu {
                    // §E.2.3.3.16-19 enhanced-coupling strategy. The
                    // §E.3.5.5 carrier synthesis runs on the deferred path
                    // after the block loop; here we parse the strategy
                    // fields and pin the coupling region
                    // [cpl_begf_mant, cpl_endf_mant) at the ecpl region so
                    // the shared exponent / bit-allocation / mantissa
                    // machinery decodes the enhanced-coupling channel into
                    // the `MAX_FBW` slot exactly like standard coupling.
                    // Recover the raw 3-bit `spxbegf` from the derived
                    // `spx_begin_subbnd` (§E.2.3.3.5 inverse):
                    //   spxbegf < 6 → spx_begin_subbnd = spxbegf + 2
                    //   spxbegf ≥ 6 → spx_begin_subbnd = spxbegf*2 - 3
                    // so the inverse splits at spx_begin_subbnd == 7.
                    // `ecpl::end_subbnd` only consults `spxbegf` when SPX is
                    // co-active (it bounds the ecpl region just below SPX).
                    let spx_begf_for_ecpl = if state.spx_begin_subbnd <= 7 {
                        state.spx_begin_subbnd.saturating_sub(2)
                    } else {
                        (state.spx_begin_subbnd + 3) / 2
                    };
                    let strat = super::ecpl::parse_strategy(
                        br,
                        state.spx_in_use,
                        spx_begf_for_ecpl,
                        &ecpl_prev_bndstrc,
                    )?;
                    ecpl_prev_bndstrc = strat.bndstrc;
                    ecpl_in_use = true;
                    state.cpl_begf_mant = strat.begin_bin();
                    state.cpl_endf_mant = strat.end_bin();
                    state.cpl_in_use = true;
                    state.cpl_nsubbnd = strat.end_subbnd - strat.begin_subbnd;
                    state.cpl_nbnd = strat.necplbnd;
                    ecpl_strategy = Some(strat);
                } else {
                    // §E.1.3.3.8 phsflginu — only in 2/0.
                    state.phsflginu = if bsi.acmod == 0x2 {
                        br.read_u32(1)? != 0
                    } else {
                        false
                    };
                    // §E.1.3.3.9 cplbegf (4 bits).
                    state.cpl_begf = br.read_u32(4)? as u8;
                    // §E.1.3.3.10 cplendf. Read 4 bits only when SPX is OFF;
                    // when SPX is in use the spec derives cplendf from the
                    // SPX begin so the coupled region ends one bin below the
                    // SPX region (spxbegf < 6 → cplendf = spxbegf − 2, else
                    // spxbegf·2 − 7 — both equal spx_begin_subbnd − 4).
                    state.cpl_endf = if state.spx_in_use {
                        (state.spx_begin_subbnd as i32 - 4).max(0) as u8
                    } else {
                        br.read_u32(4)? as u8
                    };
                    // §5.4.3.12 spec envelope: the upper sub-band index is
                    // `cplendf + 2`, so `ncplsubnd = 3 + cplendf - cplbegf
                    // >= 1` is the actual validity test (equivalently
                    // `cplbegf <= cplendf + 2`). The earlier "cplbegf >
                    // cplendf" rejection mirrored the AC-3 round-7 bug —
                    // valid corpus bitstreams use narrow configs like
                    // `(cplbegf=11, cplendf=10)` for high-bandwidth
                    // multichannel frames. Use signed arithmetic so the
                    // `3 + cplendf - cplbegf` term can't underflow.
                    let ncplsubnd_signed = 3i32 + state.cpl_endf as i32 - state.cpl_begf as i32;
                    if ncplsubnd_signed < 1 {
                        return Err(Error::invalid(
                            "eac3 audblk: cplbegf > cplendf+2 — malformed coupling range",
                        ));
                    }
                    state.cpl_nsubbnd = ncplsubnd_signed as usize;
                    // §E.2.3.3.15 cplbndstrce — gates the explicit
                    // `cplbndstrc[]` array. When it is 1, the per-subband
                    // merge flags follow. When it is 0 in the FIRST
                    // coupling block of the frame, the **default coupling
                    // banding structure** `defcplbndstrc[]` (Table E2.12)
                    // applies — it is NOT all-zeros. When it is 0 in any
                    // later block, the previous block's structure is
                    // reused (handled by leaving `cpl_bndstrc` untouched).
                    //
                    // `defcplbndstrc[]` is indexed by the **absolute**
                    // coupling sub-band number; our `cpl_bndstrc[]` is
                    // indexed by the sub-band offset relative to
                    // `cplbegf`, so the lookup is
                    // `defcplbndstrc[cplbegf + offset]`.
                    let cplbndstrce = br.read_u32(1)? != 0;
                    if cplbndstrce {
                        state.cpl_bndstrc[0] = false;
                        for bnd in 1..state.cpl_nsubbnd.min(18) {
                            let v = br.read_u32(1)? != 0;
                            state.cpl_bndstrc[bnd] = v;
                        }
                        // Any remaining (in case nsubbnd capped at 18) stay
                        // at the default 0.
                        for bnd in state.cpl_nsubbnd.min(18)..18 {
                            state.cpl_bndstrc[bnd] = false;
                        }
                    } else if first_cpl_strategy_block {
                        // §E.2.3.3.15 first-block default structure.
                        state.cpl_bndstrc[0] = false;
                        for bnd in 1..state.cpl_nsubbnd.min(18) {
                            let abs_sbnd = state.cpl_begf as usize + bnd;
                            state.cpl_bndstrc[bnd] =
                                DEFCPLBNDSTRC.get(abs_sbnd).copied().unwrap_or(false);
                        }
                        for bnd in state.cpl_nsubbnd.min(18)..18 {
                            state.cpl_bndstrc[bnd] = false;
                        }
                    }
                    // else (cplbndstrce == 0 in a later block): keep the
                    // previous block's `cpl_bndstrc[]` untouched.
                    first_cpl_strategy_block = false;
                    // Mantissa-domain coupling range: bins [37+12·begf,
                    // 37+12·(endf+3)) per §7.4.2.
                    state.cpl_begf_mant = 37 + 12 * state.cpl_begf as usize;
                    state.cpl_endf_mant = 37 + 12 * (state.cpl_endf as usize + 3);
                    // Derive ncplbnd by merging sub-bands whose
                    // cplbndstrc=1 (same algorithm as base AC-3).
                    // `cpl_bndstrc` is sized 18 (§7.4.2 max sub-bands); clamp
                    // the merge walk so a malformed `cpl_nsubbnd > 18` can't
                    // index past it.
                    let mut n = state.cpl_nsubbnd;
                    for bnd in 1..state.cpl_nsubbnd.min(18) {
                        if state.cpl_bndstrc[bnd] {
                            n -= 1;
                        }
                    }
                    state.cpl_nbnd = n;
                    state.cpl_in_use = true;
                    ecpl_in_use = false;
                } // end standard-coupling (ecplinu == 0) strategy parse
            } else {
                // !cplinu[blk] — clear all per-channel coupling flags
                // and reset the per-frame state-init markers per
                // Table E1.4 ("if !cplinu[blk] { firstcplcos[ch] = 1;
                // firstcplleak = 1; phsflginu = 0; ecplinu = 0; }").
                for ch in 0..nfchans {
                    state.channels[ch].in_coupling = false;
                    firstcplcos[ch] = true;
                }
                firstcplleak = true;
                state.phsflginu = false;
                state.cpl_in_use = false;
                ecpl_in_use = false;
                ecpl_strategy = None;
            }
        }
        // When !cplstre[blk], every persistent coupling-strategy field
        // (cpl_begf/cpl_endf/cpl_nsubbnd/cpl_nbnd/cpl_begf_mant/
        // cpl_endf_mant/cpl_in_use/in_coupling) keeps its prior value
        // from the last block where cplstre[blk] == 1 (that's the
        // whole point of `cplstre` — strategy reuse). Coordinates
        // (cplcoe, mstrcplco, cplcoexp, cplcomant) ARE re-emitted per
        // block via the cplcoe[ch] gate below.

        // ---- coupling coordinates (Table E1.4) ----
        let mut any_cplcoe_this_block = false;
        if cplinu && ecpl_in_use {
            // §E.2.3.3.20-26 enhanced-coupling coordinate block. The
            // per-channel amplitude / angle / chaos triples are read by
            // [`super::ecpl::parse_coords`] (which advances the bit cursor
            // exactly per the reference syntax); the result is stashed for
            // the deferred §E.3.5.5 synthesis after the block loop.
            let chincpl: Vec<bool> = (0..nfchans)
                .map(|ch| state.channels[ch].in_coupling)
                .collect();
            let mut coords = super::ecpl::parse_coords(
                br,
                nfchans,
                &chincpl,
                &mut firstcplcos[..nfchans],
                state.cpl_nbnd,
            )?;
            // §2.3.3.21-22: `ecplparam1e[ch] == 0` / `ecplparam2e[ch] == 0`
            // mean "the previously transmitted amplitudes / angle+chaos for
            // this channel shall be reused" — thread the prior block's
            // values into the fresh coordinate set (an earlier revision
            // replaced the whole set each block, silencing every band of a
            // reusing channel).
            if let Some(prev) = &ecpl_coords {
                super::ecpl::merge_reused_params(&mut coords, prev, state.cpl_nbnd);
            }
            ecpl_coords = Some(coords);
        } else if cplinu {
            // ecplinu == 0 path (standard coupling coordinates).
            for ch in 0..nfchans {
                if state.channels[ch].in_coupling {
                    // §E.1.3.3.13 cplcoe[ch] — implicit 1 on the very
                    // first block this channel enters coupling per
                    // frame (firstcplcos[ch]); explicit 1-bit field
                    // otherwise.
                    let cplcoe = if firstcplcos[ch] {
                        firstcplcos[ch] = false;
                        true
                    } else {
                        br.read_u32(1)? != 0
                    };
                    if cplcoe {
                        any_cplcoe_this_block = true;
                        // §E.1.3.3.14 mstrcplco[ch] — 2 bits.
                        let mstrcplco = br.read_u32(2)? as i32;
                        for bnd in 0..state.cpl_nbnd {
                            // §E.1.3.3.15-16 cplcoexp + cplcomant.
                            let cplcoexp = br.read_u32(4)? as i32;
                            let cplcomant = br.read_u32(4)? as i32;
                            let mant = if cplcoexp == 15 {
                                cplcomant as f32 / 16.0
                            } else {
                                (cplcomant + 16) as f32 / 32.0
                            };
                            let shift = cplcoexp + 3 * mstrcplco;
                            state.cpl_coord[ch][bnd] = mant * 2f32.powi(-shift);
                        }
                        state.cpl_coord_valid[ch] = true;
                    }
                } else {
                    // Channel is not part of the coupling group; reset
                    // the firstcplcos marker so a later block that
                    // brings this channel back into coupling treats
                    // its first cplcoe as implicit 1.
                    firstcplcos[ch] = true;
                }
            }
            // §E.1.3.3.17 phsflg[bnd] — only in 2/0 + phsflginu + at
            // least one channel emitted coordinates this block (the
            // spec's `cplcoe[0] || cplcoe[1]` test, which is THIS
            // block's cplcoe — not a sticky any-block flag).
            if bsi.acmod == 0x2 && state.phsflginu && any_cplcoe_this_block {
                for bnd in 0..state.cpl_nbnd {
                    state.cpl_phsflg[bnd] = br.read_u32(1)? != 0;
                }
            }
        }

        // ---- §E.1.3.4 / §7.5 rematrixing — only for 2/0 (acmod==2) ----
        // Block 0 is special: encoder emits rematflg directly without
        // a rematstr gate; subsequent blocks emit rematstr first and
        // only if set do rematflg follow. AC-3 §5.4.3.19 has the same
        // shape for base AC-3.
        if bsi.acmod == 0x2 {
            let rematstr = if blk == 0 { true } else { br.read_u32(1)? != 0 };
            if rematstr {
                // §E.3.3.2 nrematbd — folds in spectral extension AND
                // enhanced coupling. When SPX is in use without coupling
                // the band count drops to 3 for spxbegf < 2
                // (spx_begin_subbnd < 4). When enhanced coupling is in use
                // the count is sized from the raw `ecplbegf` (carried on
                // the persistent `ecpl_strategy`, so a `cplstre == 0` reuse
                // block keeps the prior strategy's begin code), NOT from
                // `cpl_begf` (which the ecpl path never sets). Using the
                // wrong arm drifts the bit cursor on ecpl / SPX 2/0 frames.
                let ecplbegf = ecpl_strategy.as_ref().map_or(0, |s| s.ecplbegf);
                let n_remat = crate::audblk::remat_band_count_spx(
                    cplinu,
                    state.cpl_begf,
                    ecpl_in_use,
                    ecplbegf,
                    state.spx_in_use,
                    state.spx_begin_subbnd,
                );
                for rbnd in 0..n_remat {
                    let v = br.read_u32(1)? != 0;
                    state.rematflg[rbnd] = v;
                }
            }
        }

        // ---- §E.1.2.4 exponent strategy lookup ----
        //
        // §E.1.2.3 / Table E.1.3 emit `chexpstr[blk][ch]` (2 bits),
        // `cplexpstr[blk]` (2 bits when cplinu[blk]), and per-block
        // `lfeexpstr[blk]` (1 bit, when lfeon) IN audfrm — NOT in
        // audblk. The audblk only carries the bandwidth code +
        // exponent payload that those strategies gate. Round-29.5
        // moves the strategy reads back to where the spec puts them
        // (audfrm); audblk just looks them up.
        let cplexpstr = if cplinu {
            audfrm.cplexpstr_blk[blk]
        } else {
            0u8
        };
        let mut chexpstr = [0u8; MAX_FBW];
        chexpstr[..nfchans].copy_from_slice(&audfrm.chexpstr_blk_ch[blk][..nfchans]);
        let lfeexpstr = if lfeon { audfrm.lfeexpstr[blk] } else { 0u8 };

        // §E.1.3.4.5 chbwcod — only when chexpstr != REUSE AND the
        // channel is neither in coupling NOR in spectral extension
        // (per the audblk syntax: `if((!chincpl[ch]) && (!chinspx[ch]))
        // {chbwcod[ch]}`). SPX channels derive their bandwidth from the
        // SPX begin frequency instead.
        let mut chbwcod = [0u8; MAX_FBW];
        for ch in 0..nfchans {
            if chexpstr[ch] != 0 && !state.channels[ch].in_coupling && !state.channels[ch].in_spx {
                chbwcod[ch] = br.read_u32(6)? as u8;
                if chbwcod[ch] > 60 {
                    return Err(Error::invalid(
                        "eac3 audblk: chbwcod > 60 (E.1.3.4.6 invalid)",
                    ));
                }
            }
        }

        // ---- coupling-channel exponents (§E.1.3.4.4) ----
        if cplinu && cplexpstr != 0 {
            let cplabsexp = br.read_u32(4)? as i32;
            let cpl_start = state.cpl_begf_mant;
            let cpl_end = state.cpl_endf_mant;
            let grpsize = match cplexpstr {
                1 => 1,
                2 => 2,
                3 => 4,
                _ => 1,
            };
            // Number of groups: (cpl_end - cpl_start) / (grpsize · 3).
            // cpl_end - cpl_start = 12 · (cplendf + 3 - cplbegf) =
            // 12 · ncplsubnd which is divisible by 3 for grpsize=1
            // and by 6/12 for grpsize=2/4 only when ncplsubnd is even
            // (D2/D4). Spec-conformant encoders generally pick a
            // strategy that makes this divisible; if not we'd round
            // down and miss bins, but this is the spec's
            // `(cpl_end - cpl_start) / (grpsize × 3)` formula verbatim.
            let ncplgrps = (cpl_end - cpl_start) / (grpsize * 3);
            let mut raw_exp = vec![0i32; ncplgrps * 3];
            audblk::decode_exponents(
                br,
                cplabsexp << 1,
                ncplgrps,
                cplexpstr as usize,
                &mut raw_exp,
            )?;
            let cpl_ch = MAX_FBW;
            for (i, e) in raw_exp.iter().enumerate() {
                let idx = cpl_start + i * grpsize;
                for j in 0..grpsize {
                    if idx + j < N_COEFFS {
                        state.channels[cpl_ch].exp[idx + j] = (*e).clamp(0, 24) as u8;
                    }
                }
            }
        }

        // ---- fbw exponents ----
        for ch in 0..nfchans {
            if chexpstr[ch] != 0 {
                // Coupled channels stop at cpl_begf_mant; SPX channels
                // stop at the SPX begin frequency (§E.3.3.3:
                // endmant = spxbandtable[spx_begin_subbnd] =
                // 25 + 12·spx_begin_subbnd); un-coupled / un-SPX channels
                // go up to 37 + 3·(chbwcod+12).
                let end = if state.channels[ch].in_coupling {
                    state.cpl_begf_mant
                } else if state.channels[ch].in_spx {
                    25 + 12 * state.spx_begin_subbnd
                } else {
                    37 + 3 * (chbwcod[ch] as usize + 12)
                };
                state.channels[ch].end_mant = end;
                let absexp = br.read_u32(4)? as i32;
                let grpsize = match chexpstr[ch] {
                    1 => 1,
                    2 => 2,
                    3 => 4,
                    _ => 1,
                };
                // §7.1.3 nchgrps[ch]:
                //   D15 → (end-1)/3, D25 → (end-1+3)/6, D45 → (end-1+9)/12
                // (all truncated). The D25 form here previously used
                // `div_ceil(6)` = `(end-1+5)/6`, which over-counts groups
                // by one when `(end-1) mod 6 ∈ {2,3}` — that reads an
                // extra 7-bit exponent word and drifts the bit cursor on
                // D25 channels (the AC-3 path already uses the +3 form).
                let nchgrps = match chexpstr[ch] {
                    1 => (end - 1) / 3,
                    2 => (end - 1 + 3) / 6,
                    3 => (end - 1 + 9) / 12,
                    _ => 0,
                };
                let mut raw_exp = vec![0i32; nchgrps * 3];
                audblk::decode_exponents(br, absexp, nchgrps, chexpstr[ch] as usize, &mut raw_exp)?;
                state.channels[ch].exp[0] = absexp.clamp(0, 24) as u8;
                for (i, e) in raw_exp.iter().enumerate() {
                    let base = i * grpsize + 1;
                    for j in 0..grpsize {
                        if base + j < end {
                            state.channels[ch].exp[base + j] = (*e).clamp(0, 24) as u8;
                        }
                    }
                }
                // §E.1.2.4 / Table E.1.4 — `gainrng[ch]` (2 bits)
                // immediately after the per-channel exponent payload.
                // We don't currently use it in the DSP path (the round-2
                // bit-allocation reuses base-AC-3 sgain logic that
                // doesn't consult gainrng), but the bit MUST be
                // consumed or every subsequent field slides. The
                // earlier "Annex E dropped gainrng" comment was wrong;
                // Table E.1.4 emits it for every fbw channel whose
                // strategy this block is non-REUSE.
                let _gainrng = br.read_u32(2)?;
            } else if blk == 0 {
                return Err(Error::invalid(
                    "eac3 audblk: chexpstr == 0 in block 0 (no prior exponents to reuse)",
                ));
            } else if state.channels[ch].in_coupling {
                // Reuse path: end_mant follows the coupled channel's
                // bandwidth, which is cpl_begf_mant. Re-set in case a
                // prior block had this channel un-coupled with a
                // different end_mant.
                state.channels[ch].end_mant = state.cpl_begf_mant;
            } else if state.channels[ch].in_spx {
                // Reuse path for an SPX channel: coded mantissas stop at
                // the SPX begin frequency (§E.3.3.3).
                state.channels[ch].end_mant = 25 + 12 * state.spx_begin_subbnd;
            }
        }
        if lfeon && lfeexpstr != 0 {
            let lfe_ch = MAX_FBW + 1;
            state.channels[lfe_ch].end_mant = 7;
            let absexp = br.read_u32(4)? as i32;
            let nlfegrps = 2usize;
            let mut raw_exp = vec![0i32; nlfegrps * 3];
            audblk::decode_exponents(br, absexp, nlfegrps, 1, &mut raw_exp)?;
            state.channels[lfe_ch].exp[0] = absexp.clamp(0, 24) as u8;
            for (i, e) in raw_exp.iter().enumerate() {
                if i + 1 < 7 {
                    state.channels[lfe_ch].exp[i + 1] = (*e).clamp(0, 24) as u8;
                }
            }
        }

        // ---- §E.1.3.5 bit-allocation parametric info ----
        if audfrm.bamode {
            let baie = br.read_u32(1)? != 0;
            if baie {
                state.sdcycod = br.read_u32(2)? as u8;
                state.fdcycod = br.read_u32(2)? as u8;
                state.sgaincod = br.read_u32(2)? as u8;
                state.dbpbcod = br.read_u32(2)? as u8;
                state.floorcod = br.read_u32(3)? as u8;
            } else if blk == 0 {
                // §E.2.2.4 — "if bamode == 0 the encoder uses default
                // BA params". We use the spec's default codewords
                // (Table E1.4 footnote / §E.2.2.4).
                state.sdcycod = 0x2;
                state.fdcycod = 0x1;
                state.sgaincod = 0x1;
                state.dbpbcod = 0x2;
                state.floorcod = 0x7;
            }
        } else if blk == 0 {
            // Same defaults when bamode == 0.
            state.sdcycod = 0x2;
            state.fdcycod = 0x1;
            state.sgaincod = 0x1;
            state.dbpbcod = 0x2;
            state.floorcod = 0x7;
        }

        // ---- SNR offset (§E.1.3.5.2 / Annex E audblk §2.3.3.27) ----
        // Two strategies (Table E1.4 / spec §2.3.3.27):
        //   * `snroffststr == 0` — a single (frmcsnroffst, frmfsnroffst)
        //     pair carried once in audfrm applies to every block of the
        //     frame (the frame-level path).
        //   * `snroffststr == 0x1` / `0x2` — per-block SNR offsets carried
        //     in the audblk itself. Block 0 always emits its offsets
        //     (implicit `snroffste = 1`); later blocks emit a 1-bit
        //     `snroffste` and reuse the prior block's values when it is 0.
        // The fast-gain (`fgaincod`) defaults are independent of
        // `snroffststr`, so they are initialised at block 0 either way.
        if blk == 0 {
            // §E.2.2.4: "If bamode == 0 the encoder uses fast-gain
            // codeword 0x4 (mid)". Carry through for fgaincod when
            // frmfgaincode == 0 (no per-block fgaincod).
            for ch in 0..nfchans {
                state.fgaincod[ch] = 0x4;
            }
            if lfeon {
                state.lfefgaincod = 0x4;
            }
            state.cpl_fgaincod = 0x4;
        }
        if audfrm.snroffststr == 0 {
            // Frame-level: block 0 latches the audfrm values; later
            // blocks reuse them (they never change within the frame).
            if blk == 0 {
                state.snroffst_coarse = audfrm.frmcsnroffst;
                for ch in 0..nfchans {
                    state.fsnroffst[ch] = audfrm.frmfsnroffst;
                }
                if lfeon {
                    state.lfefsnroffst = audfrm.frmfsnroffst;
                }
                state.cpl_fsnroffst = audfrm.frmfsnroffst;
            }
        } else {
            // Per-block SNR offsets (§2.3.3.27). `snroffste` is implicit
            // 1 in block 0 and explicit thereafter; when 0 the prior
            // block's `(csnroffst, *fsnroffst)` values are reused (they
            // already live on `state`).
            let snroffste = if blk == 0 { true } else { br.read_u32(1)? != 0 };
            if snroffste {
                state.snroffst_coarse = br.read_u32(6)? as u8;
                if audfrm.snroffststr == 0x1 {
                    // One shared fine offset for coupling + every fbw
                    // channel + LFE.
                    let blkfsnroffst = br.read_u32(4)? as u8;
                    state.cpl_fsnroffst = blkfsnroffst;
                    for ch in 0..nfchans {
                        state.fsnroffst[ch] = blkfsnroffst;
                    }
                    if lfeon {
                        state.lfefsnroffst = blkfsnroffst;
                    }
                } else {
                    // snroffststr == 0x2 — independent fine offset per
                    // coupling / channel / LFE slot.
                    if cplinu {
                        state.cpl_fsnroffst = br.read_u32(4)? as u8;
                    }
                    for ch in 0..nfchans {
                        state.fsnroffst[ch] = br.read_u32(4)? as u8;
                    }
                    if lfeon {
                        state.lfefsnroffst = br.read_u32(4)? as u8;
                    }
                }
            }
        }

        // ---- §E.1.3.5.4 fgaincode (per-block fgain override) ----
        // Per Table E1.4, when `frmfgaincode == 1` a 1-bit `fgaincode`
        // field follows; if set, the per-channel fgaincod (3 bits each)
        // are emitted including the cpl-channel slot (only when
        // cplinu[blk]).
        if audfrm.frmfgaincode {
            let fgaincode = br.read_u32(1)? != 0;
            if fgaincode {
                if cplinu {
                    state.cpl_fgaincod = br.read_u32(3)? as u8;
                }
                for ch in 0..nfchans {
                    state.fgaincod[ch] = br.read_u32(3)? as u8;
                }
                if lfeon {
                    state.lfefgaincod = br.read_u32(3)? as u8;
                }
            }
        }

        // ---- §E.1.3.5.3 convsnroffste (always present for strmtyp == 0) ----
        // Optional 10-bit `convsnroffst` follows; we don't use it for
        // playback (it adjusts the SNR offset for downstream AC-3
        // converter modes), but we must consume the bit(s).
        if strmtyp_indep {
            let convsnroffste = br.read_u32(1)? != 0;
            if convsnroffste {
                let _convsnroffst = br.read_u32(10)?;
            }
        }

        // ---- §E.1.3.5.4 cplleake (only when cplinu[blk]) ----
        // First-block (per frame) emits `cplleake = 1` implicitly; later
        // blocks emit it explicitly. When set, `cplfleak` + `cplsleak`
        // (3 bits each) follow.
        if cplinu {
            let cplleake = if firstcplleak {
                firstcplleak = false;
                true
            } else {
                br.read_u32(1)? != 0
            };
            if cplleake {
                state.cpl_fleak = br.read_u32(3)? as u8;
                state.cpl_sleak = br.read_u32(3)? as u8;
            }
        }

        // ---- §E.1.3.5.5 dba ----
        if audfrm.dbaflde {
            let dbaie = br.read_u32(1)? != 0;
            if dbaie {
                let cpl_idx = MAX_FBW;
                let mut cpldeltbae = 0u32;
                if cplinu {
                    cpldeltbae = br.read_u32(2)?;
                }
                let mut deltbae = [0u32; MAX_FBW];
                for ch in 0..nfchans {
                    deltbae[ch] = br.read_u32(2)?;
                }
                if cplinu {
                    match cpldeltbae {
                        1 => {
                            let nseg = (br.read_u32(3)? + 1) as usize;
                            state.deltnseg[cpl_idx] = nseg.min(8);
                            for seg in 0..state.deltnseg[cpl_idx] {
                                state.deltoffst[cpl_idx][seg] = br.read_u32(5)? as u8;
                                state.deltlen[cpl_idx][seg] = br.read_u32(4)? as u8;
                                state.deltba[cpl_idx][seg] = br.read_u32(3)? as u8;
                            }
                        }
                        2 => {
                            state.deltnseg[cpl_idx] = 0;
                        }
                        _ => {}
                    }
                }
                for ch in 0..nfchans {
                    match deltbae[ch] {
                        1 => {
                            let nseg = (br.read_u32(3)? + 1) as usize;
                            state.deltnseg[ch] = nseg.min(8);
                            for seg in 0..state.deltnseg[ch] {
                                state.deltoffst[ch][seg] = br.read_u32(5)? as u8;
                                state.deltlen[ch][seg] = br.read_u32(4)? as u8;
                                state.deltba[ch][seg] = br.read_u32(3)? as u8;
                            }
                        }
                        2 => {
                            state.deltnseg[ch] = 0;
                        }
                        _ => {}
                    }
                }
            } else if blk == 0 {
                for ch in 0..MAX_FBW + 1 {
                    state.deltnseg[ch] = 0;
                }
            }
        } else if blk == 0 {
            for ch in 0..MAX_FBW + 1 {
                state.deltnseg[ch] = 0;
            }
        }

        // ---- §E.1.3.5.6 skip ----
        if audfrm.skipflde {
            let skiple = br.read_u32(1)? != 0;
            if skiple {
                let skipl = br.read_u32(9)?;
                br.skip(skipl * 8)?;
            }
        }

        // ---- bit allocation ----
        for ch in 0..nfchans {
            let end = state.channels[ch].end_mant;
            audblk::run_bit_allocation(
                state,
                ch,
                0,
                end,
                si.fscod,
                state.fsnroffst[ch],
                state.fgaincod[ch],
                false,
            );
        }
        if cplinu {
            let start = state.cpl_begf_mant;
            let end = state.cpl_endf_mant;
            audblk::run_bit_allocation(
                state,
                MAX_FBW,
                start,
                end,
                si.fscod,
                state.cpl_fsnroffst,
                state.cpl_fgaincod,
                true,
            );
        }
        if lfeon {
            let lfe_ch = MAX_FBW + 1;
            audblk::run_bit_allocation(
                state,
                lfe_ch,
                0,
                7,
                si.fscod,
                state.lfefsnroffst,
                state.lfefgaincod,
                false,
            );
        }

        // ---- mantissas ----
        //
        // Standard path: walk every (ch, bin) pair reading bap-coded
        // mantissas (`audblk::unpack_mantissas`).
        //
        // AHT path: when chahtinu[ch] == 1, the FIRST audblk that
        // would emit channel exponents (i.e. the block where chexpstr
        // != REUSE — for AHT-eligible streams that's always block 0)
        // reads instead chgaqmod + chgaqgain + 6×nmant AHT mantissas,
        // applies the §3.4.5 IDCT-II to recover per-block transform
        // coefficients, and caches them in `aht_coeffs[ch][blk][bin]`.
        // Subsequent blocks for AHT channels skip the mantissa read
        // and load coefficients from the cache.
        if audfrm.ahte && (aht_pending.iter().any(|&p| p) || aht_filled.iter().any(|&p| p)) {
            // AHT in use for at least one channel in this frame.
            // unpack_mixed_mantissas walks per-channel: AHT-active
            // channels skip bit reads on blocks 1..5 (their mantissas
            // were front-loaded in block 0); standard channels are
            // walked as usual via the per-channel scalar fallback.
            unpack_mixed_mantissas(
                state,
                br,
                &mut aht_coeffs,
                &mut aht_pending,
                &mut aht_filled,
                nfchans,
                lfeon,
                cplinu,
            )?;
        } else {
            audblk::unpack_mantissas(state, &ac3_bsi, br)?;
        }
        // For AHT-active channels, overwrite coeffs[bin] with the
        // pre-cached value for THIS block index. The LFE channel
        // (`lfe_slot`) is included so an AHT-coded LFE loads its 7
        // per-block coefficients from the cache too (round 113).
        for ch in (0..nfchans).chain(lfeon.then_some(lfe_slot)) {
            if aht_filled[ch] {
                let end = state.channels[ch].end_mant;
                for bin in 0..end {
                    state.channels[ch].coeffs[bin] = aht_coeffs[ch][blk][bin];
                }
                // Clear bins past end_mant so stale data can't leak.
                for bin in end..N_COEFFS {
                    state.channels[ch].coeffs[bin] = 0.0;
                }
            }
        }
        #[cfg(debug_assertions)]
        if std::env::var("EAC3_DUMP_BLK").is_ok() {
            let ch0end = state.channels[0].end_mant;
            let ch1end = state.channels[1].end_mant;
            let c0excpl = state.channels[0].in_coupling;
            let c1excpl = state.channels[1].in_coupling;
            let cplc00 = state.cpl_coord[0][0];
            let c00 = state.channels[0].coeffs[0];
            let c060 = state.channels[0].coeffs[60];
            let c0133 = state.channels[0].coeffs[133];
            let c10 = state.channels[1].coeffs[0];
            eprintln!(
                "DBG blk={blk} cplinu={cplinu} cpl_in_use={} begf_m={} endf_m={} nbnd={} ch0[end={ch0end} excpl={c0excpl}] ch1[end={ch1end} excpl={c1excpl}] cplco0={cplc00:.4} c0[0]={c00:.4} c0[60]={c060:.4} c0[133]={c0133:.4} c1[0]={c10:.4}",
                state.cpl_in_use, state.cpl_begf_mant, state.cpl_endf_mant, state.cpl_nbnd,
            );
        }

        // Coupling pseudo-channel (round 117): when coupling-AHT is in
        // use, load this block's cached coupling coefficients into the
        // `MAX_FBW` slot BEFORE `dsp_block` runs §7.4 decouple — the
        // decouple step reads `channels[MAX_FBW].coeffs[bin]` and scatters
        // it into the fbw channels via the cplco coordinates. The valid
        // span is the coupling range `[cpl_begf_mant, cpl_endf_mant)`, not
        // the `end_mant` window the fbw/LFE channels use.
        if cplinu && aht_filled[MAX_FBW] {
            let start = state.cpl_begf_mant;
            let end_c = state.cpl_endf_mant.min(N_COEFFS);
            for bin in 0..start {
                state.channels[MAX_FBW].coeffs[bin] = 0.0;
            }
            for bin in start..end_c {
                state.channels[MAX_FBW].coeffs[bin] = aht_coeffs[MAX_FBW][blk][bin];
            }
            for bin in end_c..N_COEFFS {
                state.channels[MAX_FBW].coeffs[bin] = 0.0;
            }
        }

        // ---- DSP (decouple+rematrix+dynrng+IMDCT+overlap-add) ----
        //
        // Enhanced coupling (§E.3.5.5) defers per-channel synthesis + DSP
        // to a second pass: the §E.3.5.5.1 carrier for block `blk` needs
        // the *next* block's enhanced-coupling coefficients, which are not
        // decoded until the next loop iteration. When ecpl is active for
        // this block we snapshot the de-normalised ecpl-channel
        // coefficients (the carrier source) + strategy + coords + the
        // coupled-channel set, plus the full per-channel state needed to
        // run DSP later, and skip the immediate `dsp_block` + PCM emit.
        if ecpl_in_use {
            if !frame_has_ecpl {
                // First ecpl block of the frame — remember where deferral
                // begins so the second pass maps `deferred_channels[i]` back
                // to the right absolute block (and PCM offset). Real
                // encoders enable enhanced coupling from block 0; the rare
                // mid-frame onset is handled by deferring only from here on
                // (blocks before this already emitted via the immediate
                // path, with their delay lines correctly advanced on
                // `state`).
                first_deferred_blk = blk;
            }
            frame_has_ecpl = true;
            if let Some(strat) = &ecpl_strategy {
                let mut chincpl = [false; super::ecpl::ECPL_MAX_FBW];
                for (ch, slot) in chincpl.iter_mut().enumerate().take(nfchans) {
                    *slot = state.channels[ch].in_coupling;
                }
                let mut mant = [0.0f32; 256];
                let start = state.cpl_begf_mant.min(256);
                let end_c = state.cpl_endf_mant.min(256);
                mant[start..end_c].copy_from_slice(&state.channels[MAX_FBW].coeffs[start..end_c]);
                ecpl_blocks[blk] = Some(super::ecpl::EcplBlock {
                    mant,
                    strategy: strat.clone(),
                    coords: ecpl_coords.clone().unwrap_or_default(),
                    chincpl,
                });
            }
        }
        if frame_has_ecpl {
            // Snapshot the full per-channel state for the deferred DSP pass
            // so blocks render in order with a correct overlap-add delay
            // line (mixing ecpl + non-ecpl blocks within one frame stays
            // correct). The `delay` field is intentionally re-threaded in
            // pass 2, not from the snapshot.
            deferred_channels.push(state.channels.clone());
            deferred_skip_decouple.push(ecpl_in_use);
        } else {
            audblk::dsp_block(state, &si, &ac3_bsi);

            // Write block PCM into `out`.
            let base = blk * SAMPLES_PER_BLOCK * nchans;
            for n in 0..SAMPLES_PER_BLOCK {
                for ch in 0..nfchans {
                    let s = state.channels[ch].coeffs[n];
                    out[base + n * nchans + ch] = s;
                }
                if lfeon {
                    let s = state.channels[MAX_FBW + 1].coeffs[n];
                    out[base + n * nchans + nfchans] = s;
                }
            }
        }
    }

    // ---- Deferred enhanced-coupling DSP pass (§E.3.5.5) ----
    //
    // Runs only when the frame used enhanced coupling. For each block in
    // order: reconstruct the carrier from prev/curr/next ecpl coefficients
    // (zero buffers at the frame edges per §E.3.5.5.1), synthesise each
    // coupled channel's transform coefficients into the snapshot's
    // `coeffs`, then run `dsp_block` (with decouple skipped on ecpl blocks
    // because the coefficients are already per-channel) and emit PCM. The
    // overlap-add delay line is threaded through `state.channels[ch].delay`
    // across the pass exactly as the single-pass loop would have.
    if frame_has_ecpl {
        run_deferred_ecpl_dsp(
            state,
            &si,
            &ac3_bsi,
            first_deferred_blk,
            &ecpl_blocks,
            &deferred_channels,
            &deferred_skip_decouple,
            nfchans,
            nchans,
            lfeon,
            out,
        );
    }

    // ---- Transient pre-noise processing (§E.3.7.2) ----
    // After overlap-add, each fbw channel that carries TPNP data has its
    // pre-transient region overwritten with a time-scaled copy of the
    // cleaner audio that precedes it, removing the smeared pre-noise a
    // low-rate transform coder leaves ahead of a sharp onset. LFE never
    // carries TPNP. Operates in place on the interleaved `out` buffer.
    if audfrm.transproce {
        let total_samples = num_blocks * SAMPLES_PER_BLOCK;
        for ch in 0..nfchans {
            if !audfrm.chintransproc[ch] {
                continue;
            }
            apply_transient_prenoise(
                out,
                nchans,
                ch,
                total_samples,
                audfrm.transprocloc[ch],
                audfrm.transproclen[ch],
            );
        }
    }
    Ok(())
}

/// §E.3.5.5 — the deferred enhanced-coupling DSP second pass.
///
/// Re-renders every audio block of a frame that used enhanced coupling, in
/// order, so the §E.3.5.5.1 carrier reconstruction can consult the *next*
/// block's enhanced-coupling coefficients (unavailable mid-decode). For
/// each block:
///
/// 1. The block's snapshot channel-state is restored into `state.channels`,
///    preserving the evolving overlap-add `delay` lines (carried from the
///    previous block's DSP, not from the snapshot).
/// 2. For an enhanced-coupling block, [`super::ecpl::synthesize_block`]
///    reconstructs the carrier from the previous / current / next block's
///    de-normalised coefficients (zero buffers at the frame edges, per the
///    §E.3.5.5.1 "set to zero" rule) and writes each coupled channel's
///    transform coefficients; `skip_decouple` is set so `dsp_block` does
///    not run the standard §7.4 scalar decouple over them.
/// 3. `dsp_block` runs the remaining stages (rematrix / SPX / dynrng /
///    IMDCT / overlap-add) and the PCM is emitted to `out`.
///
/// The "previous block" of frame block 0 is the *last* block of the prior
/// frame (block numbering is continuous across the stream). When that block
/// used enhanced coupling its de-normalised mantissa buffer is carried over
/// on [`super::ecpl::EcplState`] and used as block 0's `prev`; otherwise the
/// §E.3.5.5.1 "set to zero" rule applies and a zero buffer is used. This
/// frame's final enhanced-coupling block is in turn recorded for the next
/// frame. The "next block" of the frame's last block lives in a frame not
/// yet decoded, so it remains zero (true streaming lookahead is out of
/// scope).
#[allow(clippy::too_many_arguments)]
fn run_deferred_ecpl_dsp(
    state: &mut Ac3State,
    si: &SyncInfo,
    ac3_bsi: &Ac3Bsi,
    first_deferred_blk: usize,
    ecpl_blocks: &[Option<super::ecpl::EcplBlock>],
    deferred_channels: &[[crate::audblk::ChannelState; crate::audblk::MAX_CHANNELS]],
    deferred_skip_decouple: &[bool],
    nfchans: usize,
    nchans: usize,
    lfeon: bool,
    out: &mut [f32],
) {
    let n_deferred = deferred_channels.len();
    // Zero neighbour for frame-edge blocks (§E.3.5.5.1 "set to zero").
    let zero_block = super::ecpl::EcplBlock {
        mant: [0.0; 256],
        strategy: super::ecpl::EcplStrategy {
            ecplbegf: 0,
            begin_subbnd: 0,
            end_subbnd: 0,
            bndstrc: [false; super::ecpl::N_ECPL_SUBBND],
            necplbnd: 0,
        },
        coords: super::ecpl::EcplCoords::default(),
        chincpl: [false; super::ecpl::ECPL_MAX_FBW],
    };

    // Cross-frame "previous block" for frame block 0: the prior frame's last
    // enhanced-coupling block, carried over on `EcplState`. Only its mantissa
    // buffer is consulted by the carrier (strategy/coords/chincpl come from
    // `curr`), so wrap the carried spectrum in an otherwise-zero block. When
    // the prior frame had no trailing enhanced coupling this stays the zero
    // block (the §E.3.5.5.1 boundary case).
    let mut prev_frame_block = zero_block.clone();
    if let Some(carried) = state.ecpl_state.prev_frame_last_mant() {
        prev_frame_block.mant = *carried;
    }

    for i in 0..n_deferred {
        // Snapshot index `i` maps to absolute block `abs_blk`.
        let abs_blk = first_deferred_blk + i;
        // Restore this block's parsed state. For the first deferred block
        // the snapshot's own `delay` is the correct starting overlap-add
        // tail (captured before this block's DSP ran). For later blocks the
        // live delay produced by the previous block's `dsp_block` is the
        // correct one — carry it over the snapshot's stale value.
        let carry_delay: [[f32; SAMPLES_PER_BLOCK]; crate::audblk::MAX_CHANNELS] =
            std::array::from_fn(|ch| state.channels[ch].delay);
        state.channels = deferred_channels[i].clone();
        if i > 0 {
            for ch in 0..crate::audblk::MAX_CHANNELS {
                state.channels[ch].delay = carry_delay[ch];
            }
        }
        state.blkidx = abs_blk;

        let skip = deferred_skip_decouple[i];
        state.skip_decouple = skip;
        if skip {
            if let Some(curr) = &ecpl_blocks[abs_blk] {
                let prev = if abs_blk > 0 {
                    ecpl_blocks[abs_blk - 1].as_ref().unwrap_or(&zero_block)
                } else {
                    // Frame block 0 — its "previous block" is the prior
                    // frame's last enhanced-coupling block (§E.3.5.5.1).
                    &prev_frame_block
                };
                let next = if abs_blk + 1 < ecpl_blocks.len() {
                    ecpl_blocks[abs_blk + 1].as_ref().unwrap_or(&zero_block)
                } else {
                    &zero_block
                };
                // Per-channel transform-coefficient buffers; pre-seed with
                // the already-decoded independent (low-frequency) region so
                // synthesis only overwrites the enhanced-coupling bins.
                let mut chcoef: Vec<[f32; 256]> = (0..super::ecpl::ECPL_MAX_FBW)
                    .map(|ch| {
                        let mut b = [0.0f32; 256];
                        if ch < nfchans {
                            b.copy_from_slice(&state.channels[ch].coeffs);
                        }
                        b
                    })
                    .collect();
                super::ecpl::synthesize_block(
                    &mut state.ecpl_state,
                    prev,
                    curr,
                    next,
                    &mut chcoef,
                    512,
                );
                for ch in 0..nfchans {
                    state.channels[ch].coeffs.copy_from_slice(&chcoef[ch]);
                }
            }
        }

        audblk::dsp_block(state, si, ac3_bsi);
        state.skip_decouple = false;

        let base = abs_blk * SAMPLES_PER_BLOCK * nchans;
        for n in 0..SAMPLES_PER_BLOCK {
            for ch in 0..nfchans {
                out[base + n * nchans + ch] = state.channels[ch].coeffs[n];
            }
            if lfeon {
                out[base + n * nchans + nfchans] = state.channels[MAX_FBW + 1].coeffs[n];
            }
        }
    }

    // Carry this frame's last block's enhanced-coupling spectrum to the next
    // frame so its block 0 carrier can use it as the "previous block"
    // (§E.3.5.5.1). When the final frame block did not use enhanced coupling
    // the carry resets to `None` (the spec's "set to zero" boundary case).
    let last_mant = ecpl_blocks.last().and_then(|b| b.as_ref()).map(|b| b.mant);
    state.ecpl_state.set_prev_frame_last_mant(last_mant);
}

/// Transient pre-noise time-scaling synthesis for one full-bandwidth
/// channel (§E.3.7.2). Operates in place on the interleaved f32 frame
/// buffer `out` (stride `nchans`, channel slot `ch`).
///
/// The encoder transmits, relative to the first decoded PCM sample of
/// the frame, the transient location `transprocloc` (in 4-sample units;
/// multiply by 4) and the time-scaling length `transproclen` (samples).
/// The decoder reconstructs the pre-transient region from a synthesis
/// buffer copied from earlier (cleaner) audio and cross-fades it over
/// the noisy original per the spec pseudo-code:
///
/// ```text
/// transloc      = 4 * transprocloc
/// translen      = transproclen
/// pnlen         = transloc - aud_blk_samp_loc      // pre-noise length
/// tot_corr_len  = pnlen + translen + TC1
/// synth_buf[s]  = pcm_out[transloc - (2*TC1 + 2*pnlen) + s]   // 0..2*TC1+pnlen
/// start_samp    = transloc - tot_corr_len
///   [start .. start+TC1)            : fade out original, fade in synth
///   [start+TC1 .. start+corr-TC2)   : overwrite with synth
///   [start+corr-TC2 .. start+corr)  : fade in original, fade out synth
/// ```
///
/// `TC1 = 256`, `TC2 = 128` are the spec's fixed time-scaling constants.
/// `aud_blk_samp_loc` is the first-sample index of the 256-sample audio
/// block that contains the transient — the decoder derives it directly
/// (the block boundary at or below `transloc`).
///
/// Cross-fades use complementary Hann windows (§E.3.7.2 permits "nearly
/// any pair of constant-amplitude cross-fade windows"; Hann is the
/// spec's recommended choice). Reads that fall before the start of the
/// frame buffer (the spec allows a frame-N transient to reference
/// frame-(N-1) tail samples — §E.3.7.1) are clamped to index 0, the
/// conservative single-frame behaviour; a future round can thread the
/// previous frame's tail through `Eac3DecoderState` for the exact
/// cross-frame case.
fn apply_transient_prenoise(
    out: &mut [f32],
    nchans: usize,
    ch: usize,
    total_samples: usize,
    transprocloc: u16,
    transproclen: u16,
) {
    const TC1: usize = 256;
    const TC2: usize = 128;

    let transloc = 4 * transprocloc as usize;
    let translen = transproclen as usize;
    // A transient at/after the frame end (or a degenerate zero location)
    // leaves nothing to correct.
    if transloc == 0 || transloc >= total_samples {
        return;
    }
    // First sample of the 256-sample audio block containing the transient.
    let aud_blk_samp_loc = (transloc / SAMPLES_PER_BLOCK) * SAMPLES_PER_BLOCK;
    let pnlen = transloc.saturating_sub(aud_blk_samp_loc);
    if pnlen == 0 {
        // Transient sits exactly on a block boundary → no pre-noise gap.
        return;
    }
    let tot_corr_len = pnlen + translen + TC1;
    let synth_len = 2 * TC1 + pnlen;

    // Build the synthesis buffer from earlier PCM. `src0` is the first
    // source index; the spec uses `transloc - (2*TC1 + 2*pnlen)`. When
    // that is negative the samples come from the previous frame — clamp
    // to 0 (single-frame conservative path).
    let want_src0 = transloc as isize - (2 * TC1 + 2 * pnlen) as isize;
    let read = |samp_idx: isize| -> f32 {
        let idx = samp_idx.max(0) as usize;
        if idx < total_samples {
            out[idx * nchans + ch]
        } else {
            0.0
        }
    };
    let mut synth_buf = vec![0.0f32; synth_len];
    for (s, slot) in synth_buf.iter_mut().enumerate() {
        *slot = read(want_src0 + s as isize);
    }

    // start_samp = transloc - tot_corr_len. Clamp the overwrite window to
    // the valid buffer range so cross-frame underflow never panics.
    let start_isize = transloc as isize - tot_corr_len as isize;

    // Complementary Hann cross-fade windows.
    let hann_in = |i: usize, len: usize| -> f32 {
        if len <= 1 {
            return 1.0;
        }
        let x = std::f32::consts::PI * i as f32 / len as f32;
        0.5 - 0.5 * x.cos()
    };

    // Region 1: [start .. start+TC1) — fade out original, fade in synth.
    for s in 0..TC1.min(tot_corr_len) {
        let dst = start_isize + s as isize;
        if dst < 0 || dst as usize >= total_samples {
            continue;
        }
        let fi = hann_in(s, TC1);
        let fo = 1.0 - fi;
        let orig = out[dst as usize * nchans + ch];
        out[dst as usize * nchans + ch] = orig * fo + synth_buf[s] * fi;
    }
    // Region 2: [start+TC1 .. start+corr-TC2) — full synth overwrite.
    let r2_end = tot_corr_len.saturating_sub(TC2);
    for s in TC1..r2_end {
        let dst = start_isize + s as isize;
        if dst < 0 || dst as usize >= total_samples || s >= synth_len {
            continue;
        }
        out[dst as usize * nchans + ch] = synth_buf[s];
    }
    // Region 3: [start+corr-TC2 .. start+corr) — fade in original, fade
    // out synth.
    for (j, s) in (r2_end..tot_corr_len).enumerate() {
        let dst = start_isize + s as isize;
        if dst < 0 || dst as usize >= total_samples || s >= synth_len {
            continue;
        }
        let fi = hann_in(j, TC2);
        let fo = 1.0 - fi;
        let orig = out[dst as usize * nchans + ch];
        out[dst as usize * nchans + ch] = orig * fi + synth_buf[s] * fo;
    }
}

/// AHT-aware mantissa unpacker.
///
/// Mirrors [`audblk::unpack_mantissas`] but routes per-channel reads
/// through the AHT path when `aht_pending[ch] == true`. For AHT-active
/// channels we read 6×nmant mantissas + GAQ side info, dequantise via
/// VQ (Tables E4.1..E4.7) or scalar/GAQ (Table E3.5), apply the
/// §3.4.5 inverse DCT-II to recover per-block coefficients, multiply
/// by `2^-exp`, and cache the per-block coefficients in
/// `aht_coeffs[ch][blk][bin]` for the per-block dispatch loop above.
///
/// Coupling AHT (`cplahtinu`, round 117) **is** handled: the coupling
/// pseudo-channel slot `MAX_FBW` is read interleaved INSIDE the fbw
/// channel loop, right after the first coupled channel's mantissas (the
/// `got_cplchan` gate, matching Table E1.4). When `cplahtinu == 1` the
/// front-loaded coupling-AHT block (`cplgaqmod` + gains + 6×ncplmant +
/// IDCT) fills the cache over `[cpl_begf_mant, cpl_endf_mant)`; when
/// `cplahtinu == 0` the standard coupling mantissas are read there
/// instead (never dithered, §7.3.4 para 1). The LFE channel **is**
/// handled (round 113): after the fbw loop, slot `MAX_FBW + 1` runs
/// either the standard 7-mantissa LFE read (`lfeahtinu == 0`) or the
/// front-loaded LFE-AHT block (`lfeahtinu == 1`, `aht_pending[lfe] ==
/// true`), matching the §E.1.3.2 `if(lfeon)` tail of the audblk loop.
///
/// Multichannel note (round 110): the non-AHT (standard scalar) channels
/// share the bap-1/2/4 triplet/pair grouping buffers across channels in
/// frequency-then-channel order, exactly as the base AC-3
/// [`audblk::unpack_mantissas`] does — a started bap=1 group is consumed
/// by the next bap=1 mantissa even if it belongs to a later channel.
/// AHT channels read their mantissas in a separate front-loaded block, so
/// they never touch these shared buffers; the grouping threads only
/// across the standard channels present in this audblk's mantissa stream.
/// The standard LFE read shares the same grouping buffers (the base path
/// also threads LFE bap-1/2/4 mantissas through the fbw groups).
#[allow(clippy::too_many_arguments)]
fn unpack_mixed_mantissas(
    state: &mut Ac3State,
    br: &mut BitReader<'_>,
    aht_coeffs: &mut [[[f32; N_COEFFS]; AHT_BLOCKS]],
    aht_pending: &mut [bool],
    aht_filled: &mut [bool],
    nfchans: usize,
    lfeon: bool,
    cplinu: bool,
) -> Result<()> {
    // Clear per-block transform-coefficient state for every channel —
    // standard mantissas overwrite bins 0..end_mant, AHT mantissas
    // populate via the cache below; bins outside those ranges must
    // read as zero (matches the base AC-3 unpacker).
    for ch in 0..crate::audblk::MAX_CHANNELS {
        for v in state.channels[ch].coeffs.iter_mut() {
            *v = 0.0;
        }
    }

    // Shared bap-1/2/4 grouping buffers, threaded across every standard
    // (non-AHT) channel in this audblk — see the function docstring.
    // Declared once outside the channel loop so a triplet/pair started by
    // one channel is consumed by the next channel that needs it, matching
    // base AC-3 [`audblk::unpack_mantissas`].
    let mut grp1: [f32; 3] = [0.0; 3];
    let mut grp1_n = 0usize;
    let mut grp2: [f32; 3] = [0.0; 3];
    let mut grp2_n = 0usize;
    let mut grp4: [f32; 2] = [0.0; 2];
    let mut grp4_n = 0usize;

    // Standard channels (and the AHT-skip blocks for AHT channels)
    // pull from the bit stream; AHT channels on their FIRST appearance
    // pull the mantissa block and IDCT it. We walk channels in order
    // (matching the spec's `for ch in 0..nfchans` loop) so the bit
    // cursor advances in the same order whether AHT is in use or not.
    //
    // The coupling-channel mantissas (standard or AHT) are read
    // interleaved INSIDE this loop, right after the FIRST coupled
    // channel's mantissas, gated by `got_cplchan` — exactly as the base
    // AC-3 [`audblk::unpack_mantissas`] does (Table E1.4:
    // `if(cplinu[blk] && chincpl[ch] && !got_cplchan)`).
    let cpl = MAX_FBW;
    let mut got_cplchan = false;
    for ch in 0..nfchans {
        let end = state.channels[ch].end_mant;
        if aht_filled[ch] {
            // AHT cache populated on a prior block — no bits to read
            // here. The per-block dispatch loop in
            // `decode_indep_audblks` will load coefficients from
            // `aht_coeffs[ch][blk]` after this function returns.
        } else if aht_pending[ch] {
            // First AHT-active block for this channel — read GAQ side
            // info + 6×nmant mantissas + IDCT into the coefficient
            // cache. AHT reads a self-contained VQ/GAQ codeword stream
            // and never touches the shared grouping buffers above.
            let snroffset =
                (((state.snroffst_coarse as i32 - 15) << 4) + state.fsnroffst[ch] as i32) << 2;
            decode_aht_channel_mantissas(state, ch, 0, end, snroffset, br, &mut aht_coeffs[ch])?;
            aht_filled[ch] = true;
            aht_pending[ch] = false;
        } else {
            // Standard scalar mantissa path — uses the canonical base-AC-3
            // `fetch_mantissa` so bap-1/2/4 grouping shares the buffers above
            // across all standard channels (§7.3.5) and bap=0 dither matches
            // the base path's LFSR (§7.3.4).
            let dith = state.channels[ch].dithflag;
            for bin in 0..end {
                let bap = state.channels[ch].bap[bin];
                let val = audblk::fetch_mantissa(
                    br,
                    bap,
                    &mut grp1,
                    &mut grp1_n,
                    &mut grp2,
                    &mut grp2_n,
                    &mut grp4,
                    &mut grp4_n,
                    false,
                )?;
                let final_val = if bap == 0 && dith {
                    audblk::dither_lfsr(&mut state.dither_lfsr_state)
                } else {
                    val
                };
                let e = state.channels[ch].exp[bin] as i32;
                state.channels[ch].coeffs[bin] = final_val * 2f32.powi(-e);
            }
        }

        // ---- coupling-channel mantissas (Table E1.4) ----
        // Read once per block, immediately after the first coupled
        // channel, before moving on to later channels. Both the standard
        // (`cplahtinu == 0`) and AHT (`cplahtinu == 1`, round 117)
        // branches land here so the bit cursor stays aligned.
        if cplinu && state.channels[ch].in_coupling && !got_cplchan {
            got_cplchan = true;
            let start = state.cpl_begf_mant;
            let end_c = state.cpl_endf_mant;
            if aht_filled[cpl] {
                // Coupling-AHT cache populated on a prior block — no bits.
            } else if aht_pending[cpl] {
                // First (and only) coupling-AHT block this frame: §3.4.3.1
                // hebap masking uses the coupling fine-SNR offset
                // (`cpl_fsnroffst`). The bins span the coupling range
                // [cpl_begf_mant, cpl_endf_mant); the cache is loaded into
                // the cpl pseudo-channel slot for the per-block decouple
                // step in `dsp_block`.
                let snroffset =
                    (((state.snroffst_coarse as i32 - 15) << 4) + state.cpl_fsnroffst as i32) << 2;
                decode_aht_channel_mantissas(
                    state,
                    cpl,
                    start,
                    end_c,
                    snroffset,
                    br,
                    &mut aht_coeffs[cpl],
                )?;
                aht_filled[cpl] = true;
                aht_pending[cpl] = false;
            } else {
                // Standard coupling-channel read. Coupling mantissas are
                // never dithered (§7.3.4 para 1: dither is applied after a
                // channel is extracted from the coupling channel), so the
                // bap=0 LFSR substitution is skipped here.
                for bin in start..end_c {
                    let bap = state.channels[cpl].bap[bin];
                    let val = audblk::fetch_mantissa(
                        br,
                        bap,
                        &mut grp1,
                        &mut grp1_n,
                        &mut grp2,
                        &mut grp2_n,
                        &mut grp4,
                        &mut grp4_n,
                        false,
                    )?;
                    let e = state.channels[cpl].exp[bin] as i32;
                    state.channels[cpl].coeffs[bin] = val * 2f32.powi(-e);
                }
            }
        }
    }

    // ---- LFE channel (§E.1.3.2 `if(lfeon)` mantissa tail) ----
    //
    // LFE mantissas follow all fbw (+ coupling) mantissas in the audblk
    // bit stream. The base AC-3 `unpack_mantissas` reads them here too;
    // the round-110 mixed path skipped this entirely, so any AHT frame
    // carrying an LFE channel desynced the bit cursor. Round 113 wires
    // both LFE branches:
    //   * `lfeahtinu == 0` (or AHT not in use for LFE): standard 7-bin
    //     read sharing the fbw bap-1/2/4 grouping buffers (§7.3.5).
    //   * `lfeahtinu == 1` (`aht_pending[lfe_slot]`): the front-loaded
    //     LFE-AHT block — `lfegaqmod` + gains + 6×7 mantissas + IDCT,
    //     cached for the per-block dispatch loop.
    if lfeon {
        let lfe = MAX_FBW + 1;
        let end = state.channels[lfe].end_mant; // 7 per §5.4.3.63
        if aht_filled[lfe] {
            // LFE-AHT cache populated on a prior block — no bits to read.
        } else if aht_pending[lfe] {
            // First (and only) LFE-AHT block: §3.4.3.1 hebap masking uses
            // the LFE fine-SNR offset (`lfefsnroffst`) in place of the
            // per-channel `fsnroffst[ch]`.
            let snroffset =
                (((state.snroffst_coarse as i32 - 15) << 4) + state.lfefsnroffst as i32) << 2;
            decode_aht_channel_mantissas(state, lfe, 0, end, snroffset, br, &mut aht_coeffs[lfe])?;
            aht_filled[lfe] = true;
            aht_pending[lfe] = false;
        } else {
            // Standard LFE mantissa read — shares the fbw grouping buffers
            // exactly as the base AC-3 path does. LFE never participates in
            // coupling, so there is no coupling-channel read interleaved.
            let dith = state.channels[lfe].dithflag;
            for bin in 0..end {
                let bap = state.channels[lfe].bap[bin];
                let val = audblk::fetch_mantissa(
                    br,
                    bap,
                    &mut grp1,
                    &mut grp1_n,
                    &mut grp2,
                    &mut grp2_n,
                    &mut grp4,
                    &mut grp4_n,
                    false,
                )?;
                let final_val = if bap == 0 && dith {
                    audblk::dither_lfsr(&mut state.dither_lfsr_state)
                } else {
                    val
                };
                let e = state.channels[lfe].exp[bin] as i32;
                state.channels[lfe].coeffs[bin] = final_val * 2f32.powi(-e);
            }
        }
    }
    Ok(())
}

/// Decode the AHT mantissa block for one channel (fbw or LFE) and fill
/// its 6×N coefficient cache. Per §3.4 / §3.4.4 / §3.4.5 / §3.4.4.2.
///
/// 1. **hebap** — per-bin high-efficiency bap, derived in the §3.4.3.1
///    pseudo-code from psd/mask. Reuses the masking curve already
///    computed by [`audblk::run_bit_allocation`] (so we walk
///    `state.channels[ch].psd`/`mask` rather than re-derive them). The
///    masking `snroffset` for this channel is passed in (`gaqmod`'s
///    siblings `chgaqmod` / `lfegaqmod` differ only by which fine-SNR
///    offset feeds the mask — `state.fsnroffst[ch]` for fbw,
///    `state.lfefsnroffst` for LFE), so this routine serves both.
/// 2. **gaqmod** (`chgaqmod` / `lfegaqmod`): 2 bits.
/// 3. **gaqbin[bin]**: derived from hebap (Table E3.3 logic).
/// 4. **gaqgain[n]** for `n in 0..gaqsections`: 1 or 5 bits each
///    depending on gaqmod (mode 3 packs 3 gains in 5 bits).
/// 5. **mantissas**: per bin, per AHT-block (j in 0..6):
///    * `hebap == 0`           → mantissa = 0 (zero bin).
///    * `1 <= hebap <= 7`      → 6-element VQ codeword shared across all 6 j's.
///    * `hebap >= 8`           → scalar/GAQ per j (with optional gain word).
/// 6. **IDCT-II §3.4.5**       → reconstruct per-block C(k, m) from X(k, j).
/// 7. **`coeff = mant · 2^-exp`** stored in `cache[blk][bin]`.
fn decode_aht_channel_mantissas(
    state: &mut Ac3State,
    ch: usize,
    start: usize,
    end: usize,
    snroffset: i32,
    br: &mut BitReader<'_>,
    cache: &mut [[f32; N_COEFFS]; AHT_BLOCKS],
) -> Result<()> {
    // ---- 1. derive hebap[] from psd / mask via §3.4.3.1 ----
    //
    // The masking curve `state.channels[ch].mask` is in the spec's
    // banded representation (50 entries indexed by masktab[bin]). We
    // reproduce the per-band post-processing inline (`mask_after_floor`)
    // so our hebap lookup matches the encoder's choice exactly. The
    // mantissa range is `[start, end)`: fbw/LFE channels start at bin 0,
    // the coupling pseudo-channel (round 117) starts at `cpl_begf_mant`
    // and ends at `cpl_endf_mant`. `hebap` is indexed by absolute bin so
    // it lines up with `psd`/`exp`/the per-block coefficient cache.
    let mut hebap = vec![0u8; end.max(1)];
    if start < end {
        use crate::tables::{BNDSZ, BNDTAB, FLOORTAB, MASKTAB};
        let floor = FLOORTAB[state.floorcod as usize];
        let mut i = start;
        let mut j = MASKTAB[start] as usize;
        while i < end {
            let lastbin = (BNDTAB[j] as usize + BNDSZ[j] as usize).min(end);
            let mut m = state.channels[ch].mask[j] as i32;
            m -= snroffset;
            m -= floor;
            if m < 0 {
                m = 0;
            }
            m &= 0x1fe0;
            m += floor;
            while i < lastbin {
                hebap[i] = aht::hebap_from_address(state.channels[ch].psd[i], m);
                i += 1;
            }
            j += 1;
        }
    }

    // ---- 2. gaqmod (chgaqmod / cplgaqmod / lfegaqmod, 2 bits) ----
    let gaqmod = br.read_u32(2)? as u8;

    // ---- 3. compute gaqbin[bin] (Table E3.3 logic) ----
    // `fill_gaqbin` walks `hebap[..]` from index 0; bins below `start`
    // have `hebap == 0` (left zero above) so they classify as non-GAQ
    // and never consume a gain word, matching the spec's
    // `for(bin = cplstrtmant; bin < cplendmant; ...)` GAQ-active scan.
    let mut gaqbin = vec![0i8; end.max(1)];
    let active = aht::fill_gaqbin(&hebap, gaqmod, &mut gaqbin);

    // ---- 4. read gaqgain[n] for nsections sections ----
    let nsections = aht::gaq_sections(gaqmod, active);
    let mut gain_words = vec![0u8; active];
    aht::read_gaq_gains(br, gaqmod, nsections, &mut gain_words)?;

    // ---- 5/6/7. per-bin mantissa decode + IDCT + scale ----
    let mut gain_iter = gain_words.into_iter();
    let mut x = [0.0f32; 6];
    for bin in start..end {
        let h = hebap[bin];
        if h == 0 {
            // Zero-mantissa bin — coefficients are 0 across all blocks.
            for blk in 0..AHT_BLOCKS {
                cache[blk][bin] = 0.0;
            }
            continue;
        }
        if (1..=7).contains(&h) {
            // VQ regime — single codeword shared across the 6 AHT blocks.
            let nb = aht::VQ_BITS[h as usize] as u32;
            let idx = br.read_u32(nb)? as usize;
            x = aht::vq_lookup(h, idx);
        } else {
            // Scalar / GAQ regime.
            let gain_code = if gaqbin[bin] == 1 {
                gain_iter.next().unwrap_or(0)
            } else {
                0
            };
            aht::read_scalar_aht_mantissas(br, h, gaqmod, gaqbin[bin], gain_code, &mut x)?;
        }
        // Inverse DCT-II to recover per-block C(k, m) (§3.4.5).
        let c = aht::idct_ii_6(x);
        // `coeff = mantissa · 2^(-exp)` for each block.
        let exp = state.channels[ch].exp[bin] as i32;
        let scale = 2f32.powi(-exp);
        for blk in 0..AHT_BLOCKS {
            cache[blk][bin] = c[blk] * scale;
        }
    }

    Ok(())
}

/// Compute the §3.4.2 AHT helper variables `nchregs[ch]` / `ncplregs` /
/// `nlferegs` from the per-block exponent strategies already parsed onto
/// `AudFrm`. Each variable counts the number of audio blocks in the
/// 6-block frame that transmit fresh exponents for that channel (i.e. a
/// strategy other than REUSE); coupling additionally counts blocks that
/// re-declare the coupling strategy (`cplstre[blk] == 1`).
///
/// These are NOT in the bitstream — the spec derives them so the decoder
/// knows which `chahtinu` / `cplahtinu` / `lfeahtinu` presence bits the
/// `audfrm()` AHT block actually emitted (a flag is only present when its
/// regs count is exactly 1, meaning exponents are sent once per frame and
/// the channel is AHT-eligible). All inputs (`chexpstr_blk_ch`,
/// `cplexpstr_blk`, `cplstre_blk`, `lfeexpstr`) are filled by
/// `audfrm::parse_with` for both the `expstre == 1` and `expstre == 0`
/// (Table E2.10) paths, so no real audblk pre-walk is required.
fn compute_aht_regs(audfrm: &AudFrm, bsi: &Eac3Bsi) -> AhtRegsHints {
    const REUSE: u8 = 0;
    let nfchans = (bsi.nfchans as usize).min(MAX_FBW);

    // nchregs[ch] — §3.4.2: count blocks where chexpstr[blk][ch] != reuse.
    let mut nchregs = [0u8; MAX_FBW];
    for (ch, regs) in nchregs.iter_mut().enumerate().take(nfchans) {
        let mut n = 0u8;
        for blk in 0..AHT_BLOCKS {
            if audfrm.chexpstr_blk_ch[blk][ch] != REUSE {
                n += 1;
            }
        }
        *regs = n;
    }

    // ncplregs — §3.4.2: only meaningful when coupling is in use for all
    // 6 blocks (the AHT eligibility gate also checks `ncplblks == 6`).
    // Count blocks where cplstre[blk] == 1 OR cplexpstr[blk] != reuse.
    let mut ncplregs = 0u8;
    for blk in 0..AHT_BLOCKS {
        if audfrm.cplstre_blk[blk] || audfrm.cplexpstr_blk[blk] != REUSE {
            ncplregs += 1;
        }
    }

    // nlferegs — §3.4.2: count blocks where lfeexpstr[blk] != reuse.
    let mut nlferegs = 0u8;
    if bsi.lfeon {
        for blk in 0..AHT_BLOCKS {
            if audfrm.lfeexpstr[blk] != REUSE {
                nlferegs += 1;
            }
        }
    }

    AhtRegsHints {
        nchregs,
        ncplregs,
        nlferegs,
    }
}

/// Decide whether the round-2 DSP path can handle this frame.
fn reject_unsupported(bsi: &Eac3Bsi, audfrm: &AudFrm) -> Result<()> {
    // expstre handling — both per-block (expstre==1) and frame-based
    // (expstre==0) strategies are supported. Round 72 (this commit)
    // landed the frame-based path: `audfrm::parse_with` expands the
    // 5-bit `frmcplexpstr` + per-channel `frmchexpstr[ch]` codewords
    // via Table E2.10 into `cplexpstr_blk[]` + `chexpstr_blk_ch[]` so
    // the dsp body sees the same per-block-per-channel shape it
    // already consumes for the expstre==1 case. Every validator-produced
    // E-AC-3 fixture in the corpus picks expstre==0.
    // snroffststr handling — all three strategies are supported. `0`
    // uses the single frame-level (frmcsnroffst, frmfsnroffst) pair from
    // audfrm; `0x1` / `0x2` read per-block SNR offsets out of each audblk
    // (`snroffste` + `csnroffst` + the shared/per-channel fine offsets)
    // per the Annex E audblk §2.3.3.27 syntax. See the SNR-offset block
    // in `decode_indep_audblks`.
    // Transient pre-noise processing (`transproce`) is no longer a
    // whole-frame reject: the per-channel time-scaling synthesis runs in
    // `apply_transient_prenoise` after overlap-add (§E.3.7.2). The
    // baseband decode is unaffected by TPNP — it is a PCM-domain quality
    // enhancement layered on top of already-valid samples.
    //
    // Spectral-extension attenuation (`spxattene`) is no longer a
    // whole-frame reject either: the per-channel `chinspxatten[ch]` +
    // `spxattencod[ch]` fields propagate onto state at the top of
    // `decode_indep_audblks`, and `audblk::apply_spectral_extension`
    // applies the §3.6.4.2.3 5-tap border notch filter when the flag
    // is set for a channel. When `spxattene == 0` (every validator-
    // encoded E-AC-3 fixture in the corpus carries this) the SPX
    // synthesis path is byte-identical to the round-100 implementation.
    // ahte is now handled by the round-6 phase-B path (mono-only).
    // Defensive: reject any case where phase B was supposed to run
    // but didn't get a chance (caller forgot to call parse_phase_b).
    if audfrm.aht_phase_b_pending {
        return Err(Error::invalid(
            "eac3 dsp: audfrm phase-B AHT bits not consumed — caller must \
             invoke audfrm::parse_phase_b before decode_indep_audblks",
        ));
    }
    if bsi.frmsiz == 0 {
        return Err(Error::invalid("eac3 dsp: frmsiz=0 (would be 2-byte frame)"));
    }
    Ok(())
}

/// Build a synthetic [`Ac3Bsi`] from the parsed [`Eac3Bsi`] so the
/// AC-3 helpers (which read `bsi.nfchans`/`bsi.nchans`/`bsi.lfeon`) see
/// the shape they expect.
fn build_ac3_bsi_shim(bsi: &Eac3Bsi) -> Ac3Bsi {
    Ac3Bsi {
        bsid: bsi.bsid,
        bsmod: 0,
        acmod: bsi.acmod,
        nfchans: bsi.nfchans,
        lfeon: bsi.lfeon,
        nchans: bsi.nchans,
        dialnorm: bsi.dialnorm,
        dialnorm_ch2: bsi.dialnorm_ch2,
        // Annex E (E-AC-3) removes the base §5.4.2.4-5 2-bit
        // `cmixlev` / `surmixlev` slots in favour of the refined
        // 3-bit `ltrtcmixlev` / `lorocmixlev` / `ltrtsurmixlev` /
        // `lorosurmixlev` codewords carried in the `mixmdata` block.
        // The shim therefore hands the base helpers the "absent"
        // sentinel `0xFF` plus the `None` typed surface unconditionally;
        // any consumer that wants the refined coefficients should consult
        // the Annex E `annex_d_mix_levels` instead.
        cmixlev: 0xFF,
        center_mix: None,
        surmixlev: 0xFF,
        surround_mix: None,
        dsurmod: 0xFF,
        // Forward the Annex E informational-metadata Dolby Surround mode
        // (§E.2.3.1.x reusing §5.4.2.6 / Table 5.11) when present so the
        // base AC-3 downmix helpers can consult the matrix-encode hint
        // through the shim; `None` when the upstream BSI did not surface
        // it (`infomdate == 0` or `acmod != 2`).
        dolby_surround_mode: bsi.dolby_surround_mode,
        annex_d_mix_levels: None,
        dmixmod: 0xFF,
        // Forward the Annex E mixmdata preferred stereo downmix mode
        // (§E.1.2.2 reusing Annex D §2.3.1.2 / Table D2.2) when
        // present so the base AC-3 downmix helpers can consult the
        // hint through the shim; `None` short-circuits to the spec
        // default branch.
        dmixmod_preference: bsi.dmixmod_preference,
        compr: bsi.compr,
        compr_ch2: bsi.compr_ch2,
        // Annex E does not carry a §5.4.2.11-12 `langcod` slot — the
        // E-AC-3 BSI does not have a deprecated language-code field —
        // so the shim hands the base helpers `None` unconditionally.
        language_code: None,
        language_code_ch2: None,
        dsurexmod: None,
        dheadphonmod: None,
        adconvtyp: None,
        // Annex E never carries the §2.3.1.11-12 reserved trailer — the
        // E-AC-3 BSI does not have an `xbsi2e` block at all — so the
        // shim hands the base helpers `None` unconditionally.
        extra_bsi: None,
        audio_production: None,
        audio_production_ch2: None,
        timecod1: None,
        timecod2: None,
        timecode_presence: crate::bsi::TimeCodePresence::NotPresent,
        // Forward the Annex E informational-metadata `copyrightb` /
        // `origbs` pair when present; default to the encoder-default
        // unset pair (no policy hint) when the upstream BSI did not
        // surface them (`infomdate == 0`).
        copyright_info: bsi
            .copyright_info
            .unwrap_or(crate::bsi::CopyrightInfo::from_bits(false, false)),
        // Forward the Annex E `addbsi` payload (or leave `None` when the
        // upstream substream did not carry one) so downstream callers
        // that route through the shim still observe the chain hint.
        addbsi: bsi.addbsi.clone(),
        bits_consumed: 0,
    }
}

/// Build a [`SyncInfo`] shim — only `fscod` is consumed downstream.
fn build_syncinfo_shim(bsi: &Eac3Bsi) -> SyncInfo {
    let fscod_for_ba = match bsi.sample_rate {
        48_000 => 0,
        44_100 => 1,
        32_000 => 2,
        // Reduced-rate (24/22.05/16 kHz) — round 2 maps these to the
        // closest base-AC-3 fscod for the masking-curve table HTH lookup.
        // §E.2.2.4 says "the masking model uses the (fscod, fscod2)
        // pair to index a doubled-row HTH"; we approximate with the
        // closest non-reduced row. PSNR will be a bit off on reduced
        // streams; round-3 follow-up to fix.
        24_000 | 22_050 | 16_000 => 2,
        _ => 0,
    };
    SyncInfo {
        crc1: 0,
        fscod: fscod_for_ba,
        frmsizecod: 0,
        sample_rate: bsi.sample_rate,
        frame_length: bsi.frame_bytes,
    }
}

#[cfg(test)]
mod tpnp_tests {
    use super::*;

    // Mono helper: build an interleaved (stride 1) frame buffer.
    fn frame(samples: usize) -> Vec<f32> {
        vec![0.0; samples]
    }

    /// A transient at or past the frame end is a no-op (nothing to
    /// correct ahead of it within this frame).
    #[test]
    fn transient_at_or_after_frame_end_is_noop() {
        let total = 6 * SAMPLES_PER_BLOCK; // 1536
        let mut buf = frame(total);
        for (i, v) in buf.iter_mut().enumerate() {
            *v = i as f32;
        }
        let before = buf.clone();
        // transprocloc * 4 == total → transient at the frame end.
        apply_transient_prenoise(&mut buf, 1, 0, total, (total / 4) as u16, 50);
        assert_eq!(buf, before, "transient at frame end must not modify PCM");
        // Past the end.
        apply_transient_prenoise(&mut buf, 1, 0, total, (total / 4 + 100) as u16, 50);
        assert_eq!(buf, before, "transient past frame end must not modify PCM");
    }

    /// A transient sitting exactly on a 256-sample block boundary has
    /// zero pre-noise length → no correction window.
    #[test]
    fn transient_on_block_boundary_is_noop() {
        let total = 6 * SAMPLES_PER_BLOCK;
        let mut buf = frame(total);
        for (i, v) in buf.iter_mut().enumerate() {
            *v = (i as f32).sin();
        }
        let before = buf.clone();
        // transloc = 4 * 256 = 1024 → exactly block 4's leading edge.
        apply_transient_prenoise(&mut buf, 1, 0, total, (1024 / 4) as u16, 32);
        assert_eq!(buf, before, "block-aligned transient → no pre-noise gap");
    }

    /// The corrected window must overwrite ONLY the pre-transient region
    /// `[start .. transloc)` and leave samples at/after the transient (and
    /// well before `start`) untouched. Uses a constant-1.0 baseband so the
    /// synth buffer is also all-1.0, which keeps cross-faded values at 1.0
    /// (complementary windows sum to 1) — making the "unchanged" assertion
    /// exact for every corrected sample too.
    #[test]
    fn correction_is_bounded_and_preserves_constant_signal() {
        let total = 6 * SAMPLES_PER_BLOCK; // 1536
        let mut buf = vec![1.0f32; total];
        // transloc = 4 * 300 = 1200 (inside block 4: 1024..1280).
        let transprocloc = 300u16;
        let transloc = 4 * transprocloc as usize; // 1200
        let translen = 40usize;
        apply_transient_prenoise(&mut buf, 1, 0, total, transprocloc, translen as u16);
        // A constant signal is its own time-scaled copy: every sample must
        // remain 1.0 within fp tolerance (the cross-fade windows are
        // complementary and the synth buffer is all 1.0).
        for (i, &v) in buf.iter().enumerate() {
            assert!(
                (v - 1.0).abs() < 1e-5,
                "sample {i} drifted to {v} (constant signal must survive TPNP)"
            );
        }
        // Sanity: the transient sample itself and everything after it is
        // strictly outside the overwrite window.
        let pnlen = transloc - 1024; // 176
        let tot_corr_len = pnlen + translen + 256; // 472
        let start = transloc - tot_corr_len; // 728
        assert!(start < transloc, "correction window must precede transient");
        assert!(
            tot_corr_len > 256 + 128,
            "window must span all three §E.3.7.2 cross-fade/overwrite regions"
        );
    }

    /// With distinct earlier audio, the middle (full-overwrite) region of
    /// the corrected window must equal the copied synthesis samples — i.e.
    /// the pre-noise is genuinely replaced, not merely attenuated.
    #[test]
    fn middle_region_overwrites_with_synthesis_samples() {
        const TC1: usize = 256;
        const TC2: usize = 128;
        let total = 6 * SAMPLES_PER_BLOCK;
        // Ramp so each sample is uniquely identifiable.
        let mut buf: Vec<f32> = (0..total).map(|i| i as f32).collect();
        let transprocloc = 300u16;
        let transloc = 4 * transprocloc as usize; // 1200
        let translen = 40usize;
        let pnlen = transloc - 1024; // 176
        let tot_corr_len = pnlen + translen + TC1; // 472
        let start = transloc - tot_corr_len; // 728
        let want_src0 = transloc as isize - (2 * TC1 + 2 * pnlen) as isize; // 1200-864=336
        let orig = buf.clone();
        apply_transient_prenoise(&mut buf, 1, 0, total, transprocloc, translen as u16);
        // Check a sample firmly inside region 2 [start+TC1 .. start+corr-TC2).
        let s = TC1 + 10; // within [256 .. 472-128=344)
        assert!(s < tot_corr_len - TC2);
        let dst = start + s;
        let expected = orig[(want_src0 + s as isize) as usize];
        assert!(
            (buf[dst] - expected).abs() < 1e-4,
            "region-2 sample {dst} should equal synth source {expected}, got {}",
            buf[dst]
        );
        // A sample at/after the transient is untouched.
        assert_eq!(buf[transloc], orig[transloc], "transient sample untouched");
        assert_eq!(
            buf[transloc + 5],
            orig[transloc + 5],
            "post-transient untouched"
        );
    }
}

#[cfg(test)]
mod aht_regs_tests {
    use super::*;

    /// Build a minimal Annex-E BSI for the regs tests. `acmod` drives
    /// `nfchans`; `lfeon` toggles the LFE; `num_blocks` is fixed at 6
    /// because AHT is only available in 6-block mode (§3.4.2).
    fn bsi(acmod: u8, lfeon: bool) -> Eac3Bsi {
        let nfchans = crate::tables::acmod_nfchans(acmod);
        Eac3Bsi {
            strmtyp: StreamType::Independent,
            substreamid: 0,
            frmsiz: 383,
            fscod: 0,
            fscod2: 0xFF,
            sample_rate: 48_000,
            numblkscod: 3,
            num_blocks: 6,
            acmod,
            nfchans,
            lfeon,
            nchans: nfchans + u8::from(lfeon),
            bsid: 16,
            dialnorm: 27,
            dialnorm_ch2: None,
            bsmod: None,
            chanmap: None,
            annex_e_mix_levels: None,
            dmixmod: 0xFF,
            dmixmod_preference: None,
            lfemixlevcod: None,
            pgmscl: None,
            pgmscl2: None,
            extpgmscl: None,
            paninfo: None,
            paninfo2: None,
            premix_compression: None,
            compr: None,
            compr_ch2: None,
            dsurexmod: None,
            dheadphonmod: None,
            dolby_surround_mode: None,
            adconvtyp: None,
            adconvtyp_ch2: None,
            audio_production: None,
            audio_production_ch2: None,
            copyright_info: None,
            addbsi: None,
            frame_bytes: 768,
            bits_consumed: 0,
        }
    }

    /// REUSE = 0; D15 = 1 etc. nchregs[ch] counts the non-REUSE blocks.
    #[test]
    fn nchregs_counts_non_reuse_blocks_per_channel() {
        let b = bsi(2, false); // 2/0 stereo, 2 fbw channels.
        let mut af = AudFrm::new();
        // Channel 0: AHT-eligible — block 0 fresh (D15), blocks 1..5 REUSE.
        af.chexpstr_blk_ch[0][0] = 1;
        // Channel 1: NOT AHT-eligible — fresh on block 0 and block 3.
        af.chexpstr_blk_ch[0][1] = 2;
        af.chexpstr_blk_ch[3][1] = 1;

        let regs = compute_aht_regs(&af, &b);
        assert_eq!(regs.nchregs[0], 1, "ch0 sends exponents once → eligible");
        assert_eq!(
            regs.nchregs[1], 2,
            "ch1 sends exponents twice → not eligible"
        );
        // Channels beyond nfchans must stay zero.
        assert_eq!(regs.nchregs[2], 0);
        assert_eq!(regs.ncplregs, 0, "no coupling strategy set → 0");
        assert_eq!(regs.nlferegs, 0, "lfeon=false → 0");
    }

    /// ncplregs counts blocks with `cplstre[blk] == 1` OR a non-REUSE
    /// coupling exponent strategy (§3.4.2 first pseudo-code block).
    #[test]
    fn ncplregs_counts_cplstre_or_non_reuse_cplexpstr() {
        let b = bsi(7, false); // 3/2, coupling-capable.
        let mut af = AudFrm::new();
        // Block 0: cplstre set (always true for block 0 when coupling
        // is in use) → counts. Block 2: fresh cplexpstr only. Block 4:
        // both. Others REUSE / no strategy.
        af.cplstre_blk[0] = true;
        af.cplexpstr_blk[2] = 2; // D25, non-REUSE → counts
        af.cplstre_blk[4] = true;
        af.cplexpstr_blk[4] = 1; // counted once (single block)

        let regs = compute_aht_regs(&af, &b);
        assert_eq!(
            regs.ncplregs, 3,
            "blocks 0, 2, 4 transmit coupling exponents"
        );
    }

    /// nlferegs counts non-REUSE LFE exponent strategy blocks, and is
    /// only computed when `lfeon` is set.
    #[test]
    fn nlferegs_counts_non_reuse_lfe_blocks() {
        let mut af = AudFrm::new();
        af.lfeexpstr[0] = 1; // D15 fresh
        af.lfeexpstr[3] = 1; // D15 fresh

        // lfeon=false → always 0 regardless of lfeexpstr contents.
        let no_lfe = compute_aht_regs(&af, &bsi(7, false));
        assert_eq!(no_lfe.nlferegs, 0, "lfeon=false suppresses nlferegs");

        // lfeon=true → counts the two non-REUSE blocks.
        let with_lfe = compute_aht_regs(&af, &bsi(7, true));
        assert_eq!(with_lfe.nlferegs, 2, "two fresh LFE strategy blocks");
    }

    /// A channel that transmits exponents only once across the frame is
    /// AHT-eligible (`nchregs == 1`); a single fresh block 0 with all
    /// reuse afterwards is the canonical eligible pattern.
    #[test]
    fn single_fresh_block_zero_is_aht_eligible() {
        let b = bsi(1, false); // mono
        let mut af = AudFrm::new();
        af.chexpstr_blk_ch[0][0] = 3; // D45 fresh, rest REUSE
        let regs = compute_aht_regs(&af, &b);
        assert_eq!(regs.nchregs[0], 1);
    }
}

/// Round-113 tests for the LFE branch of [`unpack_mixed_mantissas`].
///
/// Before round 113 the AHT-aware mantissa unpacker walked only the fbw
/// channels and never touched the LFE channel, so any AHT syncframe that
/// carried an LFE channel desynced the bit cursor (standard `lfeahtinu ==
/// 0` LFE) or hit the blanket coupling/LFE reject (`lfeahtinu == 1`).
/// These tests exercise both LFE branches directly through
/// `unpack_mixed_mantissas` with a hand-built `Ac3State` so the new path
/// is covered without depending on a full syncframe fixture.
#[cfg(test)]
mod lfe_aht_tests {
    use super::*;

    const LFE: usize = MAX_FBW + 1;
    const AHT_SLOTS: usize = MAX_FBW + 2;

    /// Empty AHT cache + flag arrays (one slot per `state.channels` index).
    fn empty_aht() -> (
        Vec<[[f32; N_COEFFS]; AHT_BLOCKS]>,
        [bool; AHT_SLOTS],
        [bool; AHT_SLOTS],
    ) {
        (
            vec![[[0.0; N_COEFFS]; AHT_BLOCKS]; AHT_SLOTS],
            [false; AHT_SLOTS],
            [false; AHT_SLOTS],
        )
    }

    /// Standard LFE (`lfeahtinu == 0`) in an AHT frame: the 7 LFE
    /// mantissas MUST be consumed from the bit stream. bap=5 reads exactly
    /// 4 bits per bin (no grouping), so 7 bins → 28 bits, and each bin
    /// reconstructs `MANT_LEVEL_15[code] · 2^-exp`. This is the regression
    /// guard for the pre-round-113 cursor desync.
    #[test]
    fn standard_lfe_mantissas_are_consumed_in_aht_frame() {
        let mut state = Ac3State::new();
        // No fbw channels in the mantissa stream; only LFE present.
        state.channels[LFE].end_mant = 7;
        state.channels[LFE].dithflag = false;
        for bin in 0..7 {
            state.channels[LFE].bap[bin] = 5; // 4-bit fixed quantiser
            state.channels[LFE].exp[bin] = 3;
        }

        // 7 × 4-bit LFE mantissa codewords, MSB-first.
        let codes = [0u32, 1, 2, 7, 8, 14, 15];
        let mut w = oxideav_core::bits::BitWriter::new();
        for c in codes {
            w.write_u32(c, 4);
        }
        let bytes = w.into_bytes();
        let mut br = BitReader::new(&bytes);

        let (mut cache, mut pending, mut filled) = empty_aht();
        // nfchans = 0 (no fbw), lfeon = true.
        unpack_mixed_mantissas(
            &mut state,
            &mut br,
            &mut cache,
            &mut pending,
            &mut filled,
            0,
            true,
            false,
        )
        .expect("decode");

        assert_eq!(
            br.bit_position(),
            28,
            "exactly 7 × 4-bit LFE mantissas must be consumed"
        );
        // LFE coeffs are non-zero where the codeword is non-zero (code 0
        // maps to MANT_LEVEL_15[0] which is non-zero for the symmetric
        // mid-tread quantiser, so just check finiteness + that the high
        // codewords differ from the low ones).
        let c = &state.channels[LFE].coeffs;
        assert!(c[..7].iter().all(|v| v.is_finite()));
        assert_ne!(
            c[0], c[6],
            "different codewords must reconstruct different coeffs"
        );
        assert!(!filled[LFE], "standard LFE never fills the AHT cache");
    }

    /// LFE-AHT (`lfeahtinu == 1`) with every bin driven to `hebap == 0`
    /// (zero-mantissa): the front-loaded block reads only the 2-bit
    /// `lfegaqmod`, fills the 6-block cache with zeros, sets
    /// `aht_filled[LFE]`, and the SECOND call (block 1) reads zero bits.
    #[test]
    fn lfe_aht_zero_mantissa_frontloads_and_caches() {
        let mut state = Ac3State::new();
        state.channels[LFE].end_mant = 7;
        // floorcod 0 → floor 0x2f0; mask 0 + positive snroffset clamps the
        // band floor so `mask_after_floor == floor`. psd 0 → address
        // `((0 - 0x2f0) >> 5)` is negative → clamps to 0 → hebap 0.
        state.floorcod = 0;
        state.snroffst_coarse = 15; // coarse term zero
        state.lfefsnroffst = 0;
        for bin in 0..7 {
            state.channels[LFE].psd[bin] = 0;
            state.channels[LFE].exp[bin] = 3;
        }
        for m in state.channels[LFE].mask.iter_mut() {
            *m = 0;
        }

        // Bit stream: lfegaqmod = 0 (2 bits) then nothing (all bins zero).
        let mut w = oxideav_core::bits::BitWriter::new();
        w.write_u32(0, 2);
        let bytes = w.into_bytes();
        let mut br = BitReader::new(&bytes);

        let (mut cache, mut pending, mut filled) = empty_aht();
        pending[LFE] = true; // lfeahtinu == 1

        // Block 0 — front-load.
        unpack_mixed_mantissas(
            &mut state,
            &mut br,
            &mut cache,
            &mut pending,
            &mut filled,
            0,
            true,
            false,
        )
        .expect("block 0 decode");
        assert_eq!(br.bit_position(), 2, "only lfegaqmod consumed");
        assert!(filled[LFE], "LFE-AHT cache filled after block 0");
        assert!(!pending[LFE], "pending cleared after front-load");
        for blk in 0..AHT_BLOCKS {
            for bin in 0..7 {
                assert_eq!(cache[LFE][blk][bin], 0.0, "zero-hebap → zero coeffs");
            }
        }

        // Block 1 — cached, must read no further bits.
        let pos_before = br.bit_position();
        unpack_mixed_mantissas(
            &mut state,
            &mut br,
            &mut cache,
            &mut pending,
            &mut filled,
            0,
            true,
            false,
        )
        .expect("block 1 decode");
        assert_eq!(
            br.bit_position(),
            pos_before,
            "subsequent LFE-AHT blocks read no bits"
        );
    }

    /// LFE-AHT with bins driven to a VQ regime (`hebap == 1`): the
    /// front-loaded block reads `lfegaqmod` + one 2-bit VQ index per bin,
    /// runs the §3.4.5 IDCT-II, and caches non-trivial per-block
    /// coefficients (a single VQ codeword yields six distinct block
    /// values via the inverse DCT-II, so the cache is not flat).
    #[test]
    fn lfe_aht_vq_regime_runs_idct_and_caches_nonflat() {
        let mut state = Ac3State::new();
        state.channels[LFE].end_mant = 7;
        // floorcod 7 → floor -2048; mask 0, snroffset 0. The §3.4.3.1
        // band-floor post-process `(0 - 0 - (-2048)) & 0x1fe0 + (-2048)`
        // collapses to `mask_after_floor == 0`, so the hebap address is
        // `(psd >> 5).clamp(0, 63)`. psd = 48 → `(48 >> 5) = 1` →
        // HEBAPTAB[1] = 1 → VQ regime, VQ_BITS[1] = 2 bits.
        state.floorcod = 7;
        state.snroffst_coarse = 15;
        state.lfefsnroffst = 0;
        for bin in 0..7 {
            state.channels[LFE].psd[bin] = 48;
            state.channels[LFE].exp[bin] = 0; // 2^0 = 1, leave VQ value as-is
        }
        for m in state.channels[LFE].mask.iter_mut() {
            *m = 0;
        }

        // lfegaqmod = 0 (2 bits), then 7 × 2-bit VQ indices (all index 0).
        let mut w = oxideav_core::bits::BitWriter::new();
        w.write_u32(0, 2);
        for _ in 0..7 {
            w.write_u32(0, 2);
        }
        let bytes = w.into_bytes();
        let mut br = BitReader::new(&bytes);

        let (mut cache, mut pending, mut filled) = empty_aht();
        pending[LFE] = true;

        unpack_mixed_mantissas(
            &mut state,
            &mut br,
            &mut cache,
            &mut pending,
            &mut filled,
            0,
            true,
            false,
        )
        .expect("decode");
        assert_eq!(
            br.bit_position(),
            2 + 7 * 2,
            "lfegaqmod + 7 × 2-bit VQ indices consumed"
        );
        assert!(filled[LFE]);
        // The inverse DCT-II of a fixed VQ 6-tuple produces six distinct
        // block coefficients for at least one bin (the codeword is not a
        // pure DC vector), so the per-block cache must vary across blocks.
        let varies = (0..7).any(|bin| {
            let first = cache[LFE][0][bin];
            (0..AHT_BLOCKS).any(|blk| (cache[LFE][blk][bin] - first).abs() > 1e-9)
        });
        assert!(
            varies,
            "IDCT-II of a VQ codeword must yield block-varying coefficients"
        );
    }
}

/// Round-117 tests for the coupling branch of [`unpack_mixed_mantissas`].
///
/// Before round 117 the dsp rejected any AHT syncframe with `cplahtinu ==
/// 1`. These tests drive the new coupling-AHT path (and the interleaved
/// standard coupling read inside an AHT frame) directly through
/// `unpack_mixed_mantissas` with a hand-built `Ac3State`, mirroring the
/// round-113 LFE tests. The coupling pseudo-channel lives at slot
/// `MAX_FBW`; its mantissas span `[cpl_begf_mant, cpl_endf_mant)`.
#[cfg(test)]
mod cpl_aht_tests {
    use super::*;

    const CPL: usize = MAX_FBW;
    const AHT_SLOTS: usize = MAX_FBW + 2;

    fn empty_aht() -> (
        Vec<[[f32; N_COEFFS]; AHT_BLOCKS]>,
        [bool; AHT_SLOTS],
        [bool; AHT_SLOTS],
    ) {
        (
            vec![[[0.0; N_COEFFS]; AHT_BLOCKS]; AHT_SLOTS],
            [false; AHT_SLOTS],
            [false; AHT_SLOTS],
        )
    }

    /// One fbw channel in coupling + a standard (`cplahtinu == 0`)
    /// coupling read, all inside an AHT frame (a different channel uses
    /// AHT). The coupling mantissas MUST be consumed right after the fbw
    /// channel's mantissas (the `got_cplchan` interleave). The fbw channel
    /// here has `end_mant == cpl_begf_mant` (fully coupled), so it reads no
    /// mantissas of its own; the only bits in the stream are the coupling
    /// mantissas. This is the regression guard for the interleave order.
    #[test]
    fn standard_coupling_mantissas_consumed_in_aht_frame() {
        let mut state = Ac3State::new();
        // 1 fbw channel, fully coupled from bin 0.
        let cpl_begf_mant = 0usize;
        let cpl_endf_mant = 6usize; // 6 coupling bins
        state.cpl_begf_mant = cpl_begf_mant;
        state.cpl_endf_mant = cpl_endf_mant;
        state.channels[0].end_mant = cpl_begf_mant; // fully coupled
        state.channels[0].in_coupling = true;
        // Coupling channel quantiser: bap=5 → 4 bits/bin, no grouping.
        for bin in cpl_begf_mant..cpl_endf_mant {
            state.channels[CPL].bap[bin] = 5;
            state.channels[CPL].exp[bin] = 2;
        }

        // 6 × 4-bit coupling mantissa codewords, MSB-first.
        let codes = [1u32, 3, 5, 9, 12, 15];
        let mut w = oxideav_core::bits::BitWriter::new();
        for c in codes {
            w.write_u32(c, 4);
        }
        let bytes = w.into_bytes();
        let mut br = BitReader::new(&bytes);

        let (mut cache, mut pending, mut filled) = empty_aht();
        // nfchans = 1, lfeon = false, cplinu = true. No AHT-pending
        // channel here, but the function is still exercised on the
        // coupling interleave path (the dispatch arms it whenever ahte).
        unpack_mixed_mantissas(
            &mut state,
            &mut br,
            &mut cache,
            &mut pending,
            &mut filled,
            1,
            false,
            true,
        )
        .expect("decode");

        assert_eq!(
            br.bit_position(),
            6u64 * 4,
            "exactly 6 × 4-bit coupling mantissas must be consumed"
        );
        assert!(!filled[CPL], "standard coupling never fills the AHT cache");
        let c = &state.channels[CPL].coeffs;
        assert!(c[..cpl_endf_mant].iter().all(|v| v.is_finite()));
        assert_ne!(
            c[0], c[5],
            "different coupling codewords reconstruct different coeffs"
        );
    }

    /// Coupling-AHT (`cplahtinu == 1`) with every coupling bin driven to
    /// `hebap == 0`: the front-loaded block reads only the 2-bit
    /// `cplgaqmod`, fills the 6-block cache with zeros over the coupling
    /// range, sets `aht_filled[CPL]`, and the SECOND call reads no bits.
    #[test]
    fn cpl_aht_zero_mantissa_frontloads_and_caches() {
        let mut state = Ac3State::new();
        let start = 37usize; // cpl_begf_mant for cplbegf=0
        let end = 49usize; // 12 coupling bins
        state.cpl_begf_mant = start;
        state.cpl_endf_mant = end;
        state.channels[0].end_mant = start; // fully coupled fbw 0
        state.channels[0].in_coupling = true;

        // floorcod 0 → floor 0x2f0; mask 0, snroffset 0 → mask_after_floor
        // == floor, psd 0 → negative address → clamps to 0 → hebap 0.
        state.floorcod = 0;
        state.snroffst_coarse = 15;
        state.cpl_fsnroffst = 0;
        for bin in start..end {
            state.channels[CPL].psd[bin] = 0;
            state.channels[CPL].exp[bin] = 2;
        }
        for m in state.channels[CPL].mask.iter_mut() {
            *m = 0;
        }

        // cplgaqmod = 0 (2 bits), then nothing (all bins zero).
        let mut w = oxideav_core::bits::BitWriter::new();
        w.write_u32(0, 2);
        let bytes = w.into_bytes();
        let mut br = BitReader::new(&bytes);

        let (mut cache, mut pending, mut filled) = empty_aht();
        pending[CPL] = true; // cplahtinu == 1

        // Block 0 — front-load.
        unpack_mixed_mantissas(
            &mut state,
            &mut br,
            &mut cache,
            &mut pending,
            &mut filled,
            1,
            false,
            true,
        )
        .expect("block 0 decode");
        assert_eq!(br.bit_position(), 2, "only cplgaqmod consumed");
        assert!(filled[CPL], "coupling-AHT cache filled after block 0");
        assert!(!pending[CPL], "pending cleared after front-load");
        for blk in 0..AHT_BLOCKS {
            for bin in start..end {
                assert_eq!(cache[CPL][blk][bin], 0.0, "zero-hebap → zero coeffs");
            }
        }

        // Block 1 — cached, must read no further bits.
        let pos_before = br.bit_position();
        unpack_mixed_mantissas(
            &mut state,
            &mut br,
            &mut cache,
            &mut pending,
            &mut filled,
            1,
            false,
            true,
        )
        .expect("block 1 decode");
        assert_eq!(
            br.bit_position(),
            pos_before,
            "subsequent coupling-AHT blocks read no bits"
        );
    }

    /// Coupling-AHT with bins in the VQ regime (`hebap == 1`): the
    /// front-loaded block reads `cplgaqmod` + one 2-bit VQ index per
    /// coupling bin, runs the §3.4.5 IDCT-II, and caches block-varying
    /// per-bin coefficients only across the coupling range — bins below
    /// `cpl_begf_mant` stay zero (the encoder never codes them).
    #[test]
    fn cpl_aht_vq_regime_runs_idct_and_zero_below_begf() {
        let mut state = Ac3State::new();
        let start = 37usize;
        let end = 43usize; // 6 coupling bins
        state.cpl_begf_mant = start;
        state.cpl_endf_mant = end;
        state.channels[0].end_mant = start;
        state.channels[0].in_coupling = true;

        // floorcod 7 → floor -2048; mask 0, snroffset 0 → mask_after_floor
        // == 0. psd 48 → (48 >> 5) = 1 → HEBAPTAB[1] = 1 → VQ, 2 bits.
        state.floorcod = 7;
        state.snroffst_coarse = 15;
        state.cpl_fsnroffst = 0;
        for bin in start..end {
            state.channels[CPL].psd[bin] = 48;
            state.channels[CPL].exp[bin] = 0;
        }
        for m in state.channels[CPL].mask.iter_mut() {
            *m = 0;
        }

        // cplgaqmod = 0 (2 bits), then 6 × 2-bit VQ indices (all index 0).
        let mut w = oxideav_core::bits::BitWriter::new();
        w.write_u32(0, 2);
        for _ in start..end {
            w.write_u32(0, 2);
        }
        let bytes = w.into_bytes();
        let mut br = BitReader::new(&bytes);

        let (mut cache, mut pending, mut filled) = empty_aht();
        pending[CPL] = true;

        unpack_mixed_mantissas(
            &mut state,
            &mut br,
            &mut cache,
            &mut pending,
            &mut filled,
            1,
            false,
            true,
        )
        .expect("decode");
        assert_eq!(
            br.bit_position(),
            2 + (end - start) as u64 * 2,
            "cplgaqmod + one 2-bit VQ index per coupling bin"
        );
        assert!(filled[CPL]);
        // Bins below cpl_begf_mant are never coded → cache stays zero.
        for blk in 0..AHT_BLOCKS {
            for bin in 0..start {
                assert_eq!(cache[CPL][blk][bin], 0.0, "no coupling coeffs below begf");
            }
        }
        // The IDCT-II of a VQ codeword yields six distinct block values for
        // at least one coupling bin.
        let varies = (start..end).any(|bin| {
            let first = cache[CPL][0][bin];
            (0..AHT_BLOCKS).any(|blk| (cache[CPL][blk][bin] - first).abs() > 1e-9)
        });
        assert!(
            varies,
            "IDCT-II of a coupling VQ codeword must yield block-varying coeffs"
        );
    }
}

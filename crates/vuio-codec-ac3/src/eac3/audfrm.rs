//! E-AC-3 audio frame element — `audfrm()` per Table E1.3.
//!
//! `audfrm()` sits between `bsi()` and the first `audblk()`. It
//! carries frame-level strategy flags (`expstre`, `ahte`,
//! `snroffststr`, `transproce`, `blkswe`, `dithflage`, `bamode`,
//! `frmfgaincode`, `dbaflde`, `skipflde`, `spxattene`) plus, when the
//! corresponding flag is *cleared*, the per-channel frame-level
//! strategy values that would otherwise have been emitted per block.
//!
//! Round-1 scope (consumed-but-not-acted-upon for everything except
//! the strategy flags themselves):
//!
//! * Strategy flags: stored in [`AudFrm`].
//! * Frame-level exponent strategies: parsed when `expstre == 0`. The
//!   spec packs these as a per-channel run of fixed-width codes (2
//!   bits for fbw, 5 bits for converter exponents on `strmtyp == 0`,
//!   1 bit for LFE).
//! * AHT in-use flags: parsed when `ahte == 1`.
//! * Frame-level SNR offsets: parsed when `snroffststr == 0`.
//! * Transient pre-noise processing: parsed when `transproce == 1`.
//! * Spectral-extension attenuation parameters: parsed when
//!   `spxattene == 1`.
//! * Block-start info: parsed when `numblkscod != 0` and the
//!   `blkstrtinfoe` bit is set.
//!
//! All consumed values are surfaced on [`AudFrm`] so the audblk
//! parser can use them as defaults when the per-block flag says
//! "frame-level value reused".

use oxideav_core::bits::BitReader;
use oxideav_core::Result;

use super::bsi::{Bsi, StreamType};

/// Maximum coded channels in a single substream (5 fbw + LFE = 6, but
/// the parser indexes fbw and converter strategies independently).
const MAX_FBW: usize = 5;

/// Maximum blocks per syncframe (Annex E).
pub const MAX_BLOCKS_PER_FRAME: usize = 6;

/// Strategy codes: 0 = REUSE, 1 = D15, 2 = D25, 3 = D45 (matching the
/// AC-3 `chexpstr` 2-bit encoding so the audblk-side gates can be
/// reused unchanged for the `expstre == 0` path).
const R: u8 = 0;
const D15: u8 = 1;
const D25: u8 = 2;
const D45: u8 = 3;

/// **Table E2.10** — Frame Exponent Strategy Combinations (ATSC
/// A/52:2018 Annex E §2.3.2.12 / §2.3.2.13). 32 rows × 6 blocks. Each
/// row is the 6-block strategy run encoded by one 5-bit
/// `frmcplexpstr` / `frmchexpstr[ch]` / `convexpstr[ch]` value.
/// Position 0 (block 0) is never `REUSE` — every row begins with a
/// concrete D15 / D25 / D45 strategy so the decoder always has fresh
/// exponents to reuse from.
pub(crate) const FRAME_EXP_STRAT_TABLE: [[u8; MAX_BLOCKS_PER_FRAME]; 32] = [
    [D15, R, R, R, R, R],           // 0
    [D15, R, R, R, R, D45],         // 1
    [D15, R, R, R, D25, R],         // 2
    [D15, R, R, R, D45, D45],       // 3
    [D25, R, R, D25, R, R],         // 4
    [D25, R, R, D25, R, D45],       // 5
    [D25, R, R, D45, D25, R],       // 6
    [D25, R, R, D45, D45, D45],     // 7
    [D25, R, D15, R, R, R],         // 8
    [D25, R, D25, R, R, D45],       // 9
    [D25, R, D25, R, D25, R],       // 10
    [D25, R, D25, R, D45, D45],     // 11
    [D25, R, D45, D25, R, R],       // 12
    [D25, R, D45, D25, R, D45],     // 13
    [D25, R, D45, D45, D25, R],     // 14
    [D25, R, D45, D45, D45, D45],   // 15
    [D45, D15, R, R, R, R],         // 16
    [D45, D15, R, R, R, D45],       // 17
    [D45, D25, R, R, D25, R],       // 18
    [D45, D25, R, R, D45, D45],     // 19
    [D45, D25, R, D25, R, R],       // 20
    [D45, D25, R, D25, R, D45],     // 21
    [D45, D25, R, D45, D25, R],     // 22
    [D45, D25, R, D45, D45, D45],   // 23
    [D45, D45, D15, R, R, R],       // 24
    [D45, D45, D25, R, R, D45],     // 25
    [D45, D45, D25, R, D25, R],     // 26
    [D45, D45, D25, R, D45, D45],   // 27
    [D45, D45, D45, D25, R, R],     // 28
    [D45, D45, D45, D25, R, D45],   // 29
    [D45, D45, D45, D45, D25, R],   // 30
    [D45, D45, D45, D45, D45, D45], // 31
];

/// Parsed `audfrm()` snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudFrm {
    /// `expstre` (1 bit, only present when `numblkscod == 0x3`).
    /// `true` means each block carries its own exponent strategy
    /// (mirroring AC-3 base behaviour); `false` means the frame
    /// emitted per-channel `frmchexpstr` codes that drive a 6-block
    /// strategy run via Table E2.10.
    pub expstre: bool,
    /// `ahte` (1 bit) — Adaptive Hybrid Transform in use this frame.
    pub ahte: bool,
    /// `snroffststr` (2 bits) — frame-level SNR offset packing mode.
    /// 0 = single frame value; 1/2 = per-block sub-modes (see Table
    /// E1.3 for the exact bit layout).
    pub snroffststr: u8,
    /// `transproce` (1 bit) — transient pre-noise processing in use.
    pub transproce: bool,
    /// `blkswe` (1 bit) — per-block block-switch flags emitted (when
    /// false, `blksw[ch] = 0` for every block of every channel).
    pub blkswe: bool,
    /// `dithflage` (1 bit) — per-block per-channel dither flags
    /// emitted (when false, `dithflag[ch] = 1` always).
    pub dithflage: bool,
    /// `bamode` (1 bit) — per-block bit-allocation parametric info
    /// emitted (when false, the BA params take fixed defaults from
    /// §E.2.2.4 / Table E1.4).
    pub bamode: bool,
    /// `frmfgaincode` (1 bit) — `fgaincode` is per-block (when true)
    /// versus implicit (`fgaincode = 0`) when false.
    pub frmfgaincode: bool,
    /// `dbaflde` (1 bit) — delta bit-allocation info may appear in any
    /// block when true; absent when false.
    pub dbaflde: bool,
    /// `skipflde` (1 bit) — skip-field-exists flag emitted per block
    /// when true.
    pub skipflde: bool,
    /// `spxattene` (1 bit) — spectral-extension attenuation parameters
    /// follow.
    pub spxattene: bool,
    /// Frame-level coupling exponent strategy (`frmcplexpstr`, 5
    /// bits, from Table E2.10). 0xFF when not applicable. Applies
    /// only when `expstre == 0` AND coupling is in use across the
    /// whole syncframe.
    pub frmcplexpstr: u8,
    /// Frame-level per-fbw-channel exponent strategy (`frmchexpstr`,
    /// 5 bits each). 0xFF when not applicable.
    pub frmchexpstr: [u8; MAX_FBW],
    /// `lfeexpstr[blk]` — LFE exponent strategy per block (1 bit each).
    /// Only valid when `lfeon == true`. Per Table E.1.3 / §E.1.2.3 the
    /// LFE per-block strategy is emitted in audfrm UNCONDITIONALLY of
    /// `expstre` (the `if(lfeon)` block sits OUTSIDE the
    /// `if(expstre)` branch).
    pub lfeexpstr: [u8; MAX_BLOCKS_PER_FRAME],
    /// Per-block per-channel exponent strategy code (`chexpstr[blk][ch]`,
    /// 2 bits each). Populated when `expstre == 1` (the per-block path,
    /// which is what every reasonable encoder picks). Each value is
    /// the AC-3 strategy code: 0 = REUSE, 1 = D15, 2 = D25, 3 = D45.
    /// When `expstre == 0`, this field stays zeroed and the audblk
    /// path must derive per-block strategies from `frmchexpstr` via
    /// Table E2.10.
    pub chexpstr_blk_ch: [[u8; MAX_FBW]; MAX_BLOCKS_PER_FRAME],
    /// Per-block coupling-channel exponent strategy code
    /// (`cplexpstr[blk]`, 2 bits each), populated when `expstre == 1`
    /// AND `cplinu[blk] == 1` for that block. 0 = REUSE, 1 = D15,
    /// 2 = D25, 3 = D45.
    pub cplexpstr_blk: [u8; MAX_BLOCKS_PER_FRAME],
    /// Frame-level coarse SNR offset (`frmcsnroffst`, 6 bits). Only
    /// when `snroffststr == 0`.
    pub frmcsnroffst: u8,
    /// Frame-level fine SNR offset (`frmfsnroffst`, 4 bits). Only
    /// when `snroffststr == 0`.
    pub frmfsnroffst: u8,
    /// Total bits the parser consumed (handy for callers that share
    /// a bit cursor and want to seek to the start of `audblk[0]`).
    pub bits_consumed: u64,
    /// Per-block `cplinu[blk]` — surfaced from audfrm so the audblk
    /// parser knows whether to read the coupling-coordinate block.
    /// `false` for blocks with no coupling. Always `false` when
    /// `acmod ≤ 1` (no coupling possible).
    pub cplinu_blk: [bool; MAX_BLOCKS_PER_FRAME],
    /// Per-block `cplstre[blk]` — coupling-strategy-exists flag per
    /// block. Block 0 is always implicit `1` per §E.1.2.2 / Table E1.3
    /// (the spec only transmits `cplinu[0]`); subsequent blocks
    /// transmit `cplstre[blk]` explicitly. Surfaced so the round-5
    /// audblk DSP path knows when to expect the coupling-strategy
    /// fields (chincpl[], cplbegf, …) versus reusing the prior block's
    /// strategy with fresh coordinates.
    pub cplstre_blk: [bool; MAX_BLOCKS_PER_FRAME],
    /// Number of blocks with `cplinu[blk] == 1`. Convenient summary
    /// for the round-2 DSP path which rejects any non-zero value.
    pub ncplblks: u32,
    /// Bit position immediately before the AHT block in audfrm. Set
    /// by `parse_with` regardless of whether `ahte` was true. The
    /// dsp module uses this anchor to reseek into audfrm after a
    /// pre-walk of audblks has produced `nchregs[ch]`/`ncplregs`/
    /// `nlferegs` — see [`super::dsp::decode_indep_audblks`].
    pub aht_anchor_bits: u64,
    /// `true` when `parse_with` returned with `ahte == 1` BEFORE
    /// consuming the AHT bits or the SNR/transient/SPX/blkstrtinfo
    /// tail. The caller must invoke [`parse_phase_b`] with the
    /// computed nchregs hints to finish parsing audfrm.
    pub aht_phase_b_pending: bool,
    /// Per-fbw-channel `chahtinu[ch]` flag from §3.4.2 / Table E1.3.
    /// `true` means channel `ch` is AHT-coded for this syncframe.
    /// Populated by [`parse_phase_b`].
    pub chahtinu: [bool; MAX_FBW],
    /// `cplahtinu` flag (single coupling channel). Populated by
    /// [`parse_phase_b`] when `ncplregs == 1 && ncplblks == 6`.
    pub cplahtinu: bool,
    /// `lfeahtinu` flag (single LFE channel). Populated by
    /// [`parse_phase_b`] when `nlferegs == 1` and `lfeon == true`.
    pub lfeahtinu: bool,
    /// Per-fbw-channel `chintransproc[ch]` (§2.3.2.21 / Table E1.3) —
    /// `true` when channel `ch` carries transient pre-noise time-scaling
    /// synthesis data this frame. Only meaningful when `transproce`.
    pub chintransproc: [bool; MAX_FBW],
    /// Per-fbw-channel `transprocloc[ch]` (10 bits, §2.3.2.22). The
    /// transient location relative to the first decoded PCM sample of
    /// the frame, in units of 4 samples — multiply by 4 to get the
    /// sample index (§E.3.7.2). Only valid when `chintransproc[ch]`.
    pub transprocloc: [u16; MAX_FBW],
    /// Per-fbw-channel `transproclen[ch]` (8 bits, §2.3.2.23) — the time
    /// scaling length in samples. Only valid when `chintransproc[ch]`.
    pub transproclen: [u16; MAX_FBW],
    /// Per-fbw-channel `chinspxatten[ch]` (§2.3.2.24 / Table E1.3) —
    /// `true` when channel `ch` carries spectral-extension attenuation
    /// data this frame. Only meaningful when `spxattene`.
    pub chinspxatten: [bool; MAX_FBW],
    /// Per-fbw-channel `spxattencod[ch]` (5 bits, §2.3.2.25) — index
    /// into Table E3.14 (`SPX_ATTEN_TABLE`) for the border-notch filter
    /// taps. Only valid when `chinspxatten[ch]`.
    pub spxattencod: [u8; MAX_FBW],
}

impl AudFrm {
    pub(crate) fn new() -> Self {
        Self {
            expstre: true,
            ahte: false,
            snroffststr: 0,
            transproce: false,
            blkswe: true,
            dithflage: true,
            bamode: true,
            frmfgaincode: false,
            dbaflde: true,
            skipflde: true,
            spxattene: false,
            frmcplexpstr: 0xFF,
            frmchexpstr: [0xFF; MAX_FBW],
            lfeexpstr: [0; MAX_BLOCKS_PER_FRAME],
            chexpstr_blk_ch: [[0; MAX_FBW]; MAX_BLOCKS_PER_FRAME],
            cplexpstr_blk: [0; MAX_BLOCKS_PER_FRAME],
            frmcsnroffst: 0,
            frmfsnroffst: 0,
            bits_consumed: 0,
            cplinu_blk: [false; MAX_BLOCKS_PER_FRAME],
            cplstre_blk: [false; MAX_BLOCKS_PER_FRAME],
            ncplblks: 0,
            aht_anchor_bits: 0,
            aht_phase_b_pending: false,
            chahtinu: [false; MAX_FBW],
            cplahtinu: false,
            lfeahtinu: false,
            chintransproc: [false; MAX_FBW],
            transprocloc: [0; MAX_FBW],
            transproclen: [0; MAX_FBW],
            chinspxatten: [false; MAX_FBW],
            spxattencod: [0; MAX_FBW],
        }
    }
}

/// Per-channel hint set produced by the dsp pre-walk before invoking
/// [`parse_phase_b`]. Each `nchregs[ch]` value is the number of
/// non-`REUSE` exponent strategies emitted across the 6 audblks for
/// that channel; `chahtinu[ch]` is present in the bitstream only when
/// `nchregs[ch] == 1` (and likewise for `ncplregs` / `nlferegs`).
#[derive(Clone, Copy, Debug, Default)]
pub struct AhtRegsHints {
    pub nchregs: [u8; MAX_FBW],
    pub ncplregs: u8,
    pub nlferegs: u8,
}

/// Parse `audfrm()` per Table E1.3.
///
/// `bsi.num_blocks` must equal the number of audio blocks per
/// syncframe (1, 2, 3, or 6). The parser uses it to walk the
/// per-block lfeexpstr / blkstrtinfoe runs.
pub fn parse_with(br: &mut BitReader<'_>, bsi: &Bsi) -> Result<AudFrm> {
    let start_bits = br.bit_position();
    let mut a = AudFrm::new();

    let num_blocks = bsi.num_blocks as usize;
    let nfchans = bsi.nfchans as usize;
    let lfeon = bsi.lfeon;

    // §E.2.2.3 / §E.2.3.2 — 6-block syncframes carry expstre+ahte; for
    // smaller frames the spec hard-codes expstre=1 and ahte=0.
    if num_blocks == MAX_BLOCKS_PER_FRAME {
        a.expstre = br.read_u32(1)? != 0;
        a.ahte = br.read_u32(1)? != 0;
    } else {
        a.expstre = true;
        a.ahte = false;
    }
    a.snroffststr = br.read_u32(2)? as u8;
    a.transproce = br.read_u32(1)? != 0;
    a.blkswe = br.read_u32(1)? != 0;
    a.dithflage = br.read_u32(1)? != 0;
    a.bamode = br.read_u32(1)? != 0;
    a.frmfgaincode = br.read_u32(1)? != 0;
    a.dbaflde = br.read_u32(1)? != 0;
    a.skipflde = br.read_u32(1)? != 0;
    a.spxattene = br.read_u32(1)? != 0;

    // ---- coupling data ----
    //
    // For acmod > 0x1, block 0 carries an explicit `cplinu[0]` flag
    // and subsequent blocks emit (cplstre[blk], optional cplinu[blk])
    // pairs. The pure parser doesn't need to remember which blocks
    // had coupling — it just consumes the bits.
    //
    // For acmod ≤ 0x1, every block has coupling implicitly off and
    // there is nothing to consume.
    let mut ncplblks = 0u32;
    if bsi.acmod > 0x1 {
        // cplstre[0] is fixed at 1 (not transmitted); cplinu[0] is 1
        // bit.
        let cplinu0 = br.read_u32(1)?;
        ncplblks += cplinu0;
        a.cplinu_blk[0] = cplinu0 != 0;
        a.cplstre_blk[0] = true;
        let mut last_cplinu = cplinu0;
        for blk in 1..num_blocks {
            let cplstre = br.read_u32(1)? != 0;
            a.cplstre_blk[blk] = cplstre;
            if cplstre {
                let v = br.read_u32(1)?;
                last_cplinu = v;
            }
            ncplblks += last_cplinu;
            a.cplinu_blk[blk] = last_cplinu != 0;
        }
    }
    a.ncplblks = ncplblks;

    // ---- exponent strategy data ----
    //
    // §E.1.2.3 / Table E.1.3 (ETSI TS 102 366 V1.4.1):
    //
    //   if(expstre) {
    //       for(blk = 0..6) {
    //           if(cplinu[blk] == 1) cplexpstr[blk]   2 bits
    //           for(ch = 0..nfchans) chexpstr[blk][ch] 2 bits
    //       }
    //   } else {
    //       if((acmod>1) && (ncplblks>0)) frmcplexpstr 5 bits
    //       for(ch = 0..nfchans) frmchexpstr[ch]      5 bits
    //   }
    //   if(lfeon) {
    //       for(blk = 0..6) lfeexpstr[blk]            1 bit each
    //   }
    //
    // The `if(lfeon)` block sits OUTSIDE the `if(expstre)` branch —
    // per-block lfeexpstr is emitted unconditionally of expstre.
    //
    // ETSI §E.1.3.2.1 / ATSC §E.2.3.2.1 text: "If the expstre bit is
    // set to '1', the fields that carry the full exponent strategy
    // syntax shall be present in **each audio block**." This wording
    // refers to the per-block-indexed fields enumerated by the
    // syntax table — they LIVE in audfrm, indexed by `[blk]`. Audblk
    // (Table E.1.4) does NOT re-emit chexpstr/cplexpstr/lfeexpstr; it
    // merely consumes them as state via gates like
    // `if(chexpstr[blk][ch] != reuse) {chbwcod[ch]; ...}`.
    //
    // Earlier rounds inverted this — moving the bits into audblk —
    // and the round-2 comment doubled down. The validator binary
    // rejects every frame our encoder emitted under the inverted
    // layout, surfacing as the cascade "new bit allocation info must
    // be present in block 0" / "delta bit allocation strategy
    // reserved" / "error in bit allocation".
    if a.expstre {
        for blk in 0..num_blocks {
            if a.cplinu_blk[blk] {
                a.cplexpstr_blk[blk] = br.read_u32(2)? as u8;
            }
            for ch in 0..nfchans {
                a.chexpstr_blk_ch[blk][ch] = br.read_u32(2)? as u8;
            }
        }
    } else {
        // §E.2.3.2.12 / §E.2.3.2.13 — frame-based exponent strategy. A
        // 5-bit `frmcplexpstr` (when coupling is in use anywhere in the
        // frame) and one 5-bit `frmchexpstr[ch]` per fbw channel index
        // into Table E2.10 to expand into 6 per-block strategies (each
        // value is REUSE / D15 / D25 / D45). The 6-block expansion is
        // also used to populate `cplexpstr_blk[]` on blocks where
        // coupling is in use; entries for non-cplinu blocks are
        // harmlessly left at the lookup value (the dsp module only
        // consults `cplexpstr_blk[blk]` when `cplinu_blk[blk]` is true).
        if bsi.acmod > 0x1 && ncplblks > 0 {
            a.frmcplexpstr = br.read_u32(5)? as u8;
            let row = FRAME_EXP_STRAT_TABLE[a.frmcplexpstr as usize];
            a.cplexpstr_blk[..num_blocks].copy_from_slice(&row[..num_blocks]);
        }
        for ch in 0..nfchans {
            a.frmchexpstr[ch] = br.read_u32(5)? as u8;
            let row = FRAME_EXP_STRAT_TABLE[a.frmchexpstr[ch] as usize];
            // chexpstr_blk_ch is `[blk][ch]` so we can't use a single
            // copy_from_slice — walk the blocks for this channel.
            for blk in 0..num_blocks {
                a.chexpstr_blk_ch[blk][ch] = row[blk];
            }
        }
    }
    if lfeon {
        for blk in 0..num_blocks {
            a.lfeexpstr[blk] = br.read_u32(1)? as u8;
        }
    }

    // ---- converter exponent strategy data ----
    //
    // strmtyp == 0 (independent substream): when numblkscod != 0x3 a
    // 1-bit `convexpstre` flag controls whether per-channel 5-bit
    // `convexpstr` codes follow. With numblkscod == 0x3, `convexpstre`
    // is implicit = 1 and the codes are always present.
    if matches!(bsi.strmtyp, StreamType::Independent) {
        let convexpstre_present = if num_blocks != MAX_BLOCKS_PER_FRAME {
            br.read_u32(1)? != 0
        } else {
            true
        };
        if convexpstre_present {
            for _ch in 0..nfchans {
                let _convexpstr = br.read_u32(5)?;
            }
        }
    }

    // ---- AHT data ----
    //
    // Per §E.2.3.5 / Table E1.3, `ahte` is in scope only when
    // `expstre == 1`. When `ahte == 1`, audfrm carries `ahtinu[ch]`
    // (1 bit) for every fbw channel whose 6-block exponent
    // strategies are all REUSE (`nchregs[ch] == 1`), and one
    // `ahtinu_lfe`. The per-channel `nchregs[ch]` determination
    // requires reading the audblk strategy bits **before** we get to
    // the AHT block — but those bits live in audblk[0]..audblk[5],
    // past the audfrm boundary.
    //
    // Round 6 (this commit) splits audfrm parsing into two phases:
    //
    // * `parse_with` (this entry point) walks fields 0..AHT and
    //   captures `aht_anchor_bits` = the bit position immediately
    //   before the AHT block. When `ahte == 0`, the parser proceeds
    //   straight to the SNR/transient/SPX/blkstrtinfo tail (the
    //   classic round-1 path). When `ahte == 1`, it RETURNS HERE
    //   without consuming the variable-length AHT bits OR the tail
    //   fields; the dsp module then pre-walks audblks for chexpstr,
    //   computes nchregs, and calls `parse_phase_b` with the hint
    //   to finish the parse.
    //
    // The `aht_anchor_bits` field is set in both branches so the
    // dsp module can reseek the bit cursor to here before invoking
    // `parse_phase_b`.
    a.aht_anchor_bits = br.bit_position();
    if a.ahte {
        // Phase A only — AHT bits + remaining tail are read by
        // `parse_phase_b` once the dsp pre-walk has produced
        // nchregs[ch] / ncplregs / nlferegs.
        a.bits_consumed = br.bit_position() - start_bits;
        a.aht_phase_b_pending = true;
        return Ok(a);
    }

    parse_tail(br, &mut a, bsi)?;

    a.bits_consumed = br.bit_position() - start_bits;
    Ok(a)
}

/// Parse the SNR-offset / transient / SPX-attenuation / blkstrtinfo
/// tail of `audfrm()`. Shared between `parse_with` (the AHT-off fast
/// path) and [`parse_phase_b`] (the AHT-on staged path).
fn parse_tail(br: &mut BitReader<'_>, a: &mut AudFrm, bsi: &Bsi) -> Result<()> {
    let nfchans = bsi.nfchans as usize;
    let num_blocks = bsi.num_blocks as usize;

    // ---- audio frame SNR offset data ----
    if a.snroffststr == 0 {
        a.frmcsnroffst = br.read_u32(6)? as u8;
        a.frmfsnroffst = br.read_u32(4)? as u8;
    }
    // snroffststr 1 / 2 → per-block values, parsed inside audblk.

    // ---- transient pre-noise processing (§2.3.2.20-23 / Table E1.3) ----
    // Capture the per-channel transient-location / time-scaling-length
    // parameters so the §E.3.7.2 PCM-domain synthesis can run after
    // overlap-add. Previously these were read for cursor alignment only
    // and discarded (the dsp then errored the whole frame).
    if a.transproce {
        for ch in 0..nfchans.min(MAX_FBW) {
            let chintransproc = br.read_u32(1)? != 0;
            a.chintransproc[ch] = chintransproc;
            if chintransproc {
                a.transprocloc[ch] = br.read_u32(10)? as u16;
                a.transproclen[ch] = br.read_u32(8)? as u16;
            }
        }
    }

    // ---- spectral extension attenuation data (§2.3.2.24-25) ----
    //
    // Captured per-fbw-channel so the SPX synthesis step
    // (`audblk::apply_spectral_extension`) can apply the §3.6.4.2.3
    // 5-tap notch filter at the baseband / extension border + every
    // §3.6.4.1 translation-copy wrap point. `chinspxatten[ch]` is a
    // frame-scoped flag (the spec emits it in audfrm, not audblk) and
    // applies identically to every block of the syncframe.
    if a.spxattene {
        for ch in 0..nfchans.min(MAX_FBW) {
            let chinspxatten = br.read_u32(1)? != 0;
            a.chinspxatten[ch] = chinspxatten;
            if chinspxatten {
                a.spxattencod[ch] = br.read_u32(5)? as u8;
            }
        }
    }

    // ---- block start information ----
    //
    // Only present for frames with > 1 block (numblkscod != 0); flagged
    // by `blkstrtinfoe`. When set, `blkstrtinfo` follows with
    // `nblkstrtbits` bits. nblkstrtbits is derived from frmsiz per
    // §2.3.2.27.
    if num_blocks != 1 {
        let blkstrtinfoe = br.read_u32(1)? != 0;
        if blkstrtinfoe {
            // nblkstrtbits = (numblks - 1) * (4 + ceil(log2(frmsiz_bits)))
            // For numblks=6 and frmsiz_bits ≤ 16, log2 ≤ 4 → 8 bits per
            // entry → 5*8 = 40 bits. Spec-correct formula per §2.3.2.27.
            let frame_bits = bsi.frame_bytes * 8;
            let log2 = 32 - frame_bits.leading_zeros();
            let bits_per = 4 + log2;
            let total = (num_blocks as u32 - 1) * bits_per;
            br.skip(total)?;
        }
    }

    // ---- per-channel state initialisation flags ----
    //
    // The spec requires the syntax-state init for every channel in the
    // syncframe (firstspxcos[ch] = 1, firstcplcos[ch] = 1, firstcplleak
    // = 1) — these are stateful initialisers, not bit-field reads.
    Ok(())
}

/// Phase-B audfrm parse — consumes the AHT block (using the
/// pre-walked `nchregs` hints to know which channels emit `chahtinu`
/// bits) and the SNR/transient/SPX-attenuation/blkstrtinfo tail.
///
/// Caller must:
///
/// 1. Have called [`parse_with`] which returned `aht_phase_b_pending == true`.
/// 2. Reseek the bit reader to `audfrm.aht_anchor_bits` (the
///    bit position immediately before the AHT block).
/// 3. Pass `hints` produced by walking all 6 audblks for chexpstr.
///
/// Updates `audfrm.chahtinu`/`cplahtinu`/`lfeahtinu` and clears
/// `aht_phase_b_pending`. The bit reader lands at the start of
/// `audblk[0]` on success.
pub fn parse_phase_b(
    br: &mut BitReader<'_>,
    audfrm: &mut AudFrm,
    bsi: &Bsi,
    hints: &AhtRegsHints,
) -> Result<()> {
    if !audfrm.aht_phase_b_pending {
        return Ok(());
    }
    let nfchans = bsi.nfchans as usize;
    let lfeon = bsi.lfeon;

    // §3.4.2 AHT bit stream syntax (Table E1.3 chunk gated by `ahte`):
    //
    //   if (ncplblks == 6 && ncplregs == 1) cplahtinu  (1 bit)
    //   for ch in 0..nfchans:
    //       if nchregs[ch] == 1               chahtinu[ch]  (1 bit)
    //   if (lfeon && nlferegs == 1)           lfeahtinu  (1 bit)
    if audfrm.ncplblks == 6 && hints.ncplregs == 1 {
        audfrm.cplahtinu = br.read_u32(1)? != 0;
    }
    for ch in 0..nfchans {
        if hints.nchregs[ch] == 1 {
            audfrm.chahtinu[ch] = br.read_u32(1)? != 0;
        }
    }
    if lfeon && hints.nlferegs == 1 {
        audfrm.lfeahtinu = br.read_u32(1)? != 0;
    }

    parse_tail(br, audfrm, bsi)?;
    audfrm.aht_phase_b_pending = false;
    Ok(())
}

/// Convenience parser that creates a fresh [`BitReader`] over `data`.
pub fn parse(data: &[u8], bsi: &Bsi) -> Result<AudFrm> {
    let mut br = BitReader::new(data);
    parse_with(&mut br, bsi)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bsi(acmod: u8, lfeon: bool, num_blocks: u8, strmtyp: StreamType) -> Bsi {
        Bsi {
            strmtyp,
            substreamid: 0,
            frmsiz: 383,
            fscod: 0,
            fscod2: 0xFF,
            sample_rate: 48_000,
            numblkscod: if num_blocks == 6 { 3 } else { num_blocks - 1 },
            num_blocks,
            acmod,
            nfchans: crate::tables::acmod_nfchans(acmod),
            lfeon,
            nchans: crate::tables::acmod_nfchans(acmod) + u8::from(lfeon),
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

    fn pack_msb(bits: &[(u32, u32)]) -> Vec<u8> {
        let total: u32 = bits.iter().map(|(n, _)| *n).sum();
        let nbytes = total.div_ceil(8);
        let mut out = vec![0u8; nbytes as usize];
        let mut bitpos = 0u32;
        for &(n, v) in bits {
            for i in (0..n).rev() {
                let bit = ((v >> i) & 1) as u8;
                let byte = (bitpos / 8) as usize;
                let shift = 7 - (bitpos % 8);
                out[byte] |= bit << shift;
                bitpos += 1;
            }
        }
        out
    }

    /// 2/0 stereo, 6 blocks, all strategy flags at the encoder's
    /// preferred values (expstre=1, blkswe=1, dithflage=1, bamode=1,
    /// dbaflde=1, skipflde=1; ahte=0, transproce=0, spxattene=0,
    /// snroffststr=0, frmfgaincode=0). acmod=2 has no LFE and (without
    /// a coupling block) no per-block coupling bits.
    #[test]
    fn parses_minimal_indep_stereo_audfrm() {
        let bsi = make_bsi(2, false, 6, StreamType::Independent);
        // Strategy flags (numblkscod==3 → 6 blocks)
        let mut bits: Vec<(u32, u32)> = vec![(1, 1)]; // expstre
        bits.push((1, 0)); // ahte
        bits.push((2, 0)); // snroffststr
        bits.push((1, 0)); // transproce
        bits.push((1, 1)); // blkswe
        bits.push((1, 1)); // dithflage
        bits.push((1, 1)); // bamode
        bits.push((1, 0)); // frmfgaincode
        bits.push((1, 1)); // dbaflde
        bits.push((1, 1)); // skipflde
        bits.push((1, 0)); // spxattene
                           // acmod>1 → block 0 cplinu, then 5*(cplstre[, cplinu]) pairs.
        bits.push((1, 0)); // cplinu[0] = 0
        for _ in 1..6 {
            bits.push((1, 0)); // cplstre[blk] = 0
        }
        // expstre==1 + cplinu[blk]=0 for every block → per-block
        // per-channel chexpstr (2 bits each) for 6 blocks × 2 channels
        // = 24 bits live HERE in audfrm (Table E.1.3 / §E.1.2.3).
        // No cplexpstr (no coupling). No lfeexpstr (lfeon=false).
        for _ in 0..(6 * 2) {
            bits.push((2, 0)); // chexpstr[blk][ch] = REUSE
        }
        // strmtyp == 0 + numblkscod == 0x3 → convexpstre implicit = 1,
        // followed by per-channel convexpstr (5 bits each).
        for _ in 0..2 {
            bits.push((5, 0));
        }
        // ahte=0 → no AHT block.
        // snroffststr=0 → frmcsnroffst (6) + frmfsnroffst (4)
        bits.push((6, 15));
        bits.push((4, 0));
        // transproce=0, spxattene=0 → nothing.
        // num_blocks > 1 → blkstrtinfoe (1 bit, 0).
        bits.push((1, 0));

        let buf = pack_msb(&bits);
        let af = parse(&buf, &bsi).unwrap();
        assert!(af.expstre);
        assert!(af.blkswe);
        assert!(af.dithflage);
        assert!(af.bamode);
        assert!(af.dbaflde);
        assert!(af.skipflde);
        assert!(!af.ahte);
        assert!(!af.transproce);
        assert!(!af.spxattene);
        assert_eq!(af.snroffststr, 0);
        assert_eq!(af.frmcsnroffst, 15);
        assert_eq!(af.frmfsnroffst, 0);
    }

    /// Table E2.10 row sanity — the table is the contract between the
    /// audfrm parser (which expands `frmchexpstr` / `frmcplexpstr`)
    /// and the audblk DSP (which consumes per-block strategy codes via
    /// `chexpstr_blk_ch[blk][ch]` / `cplexpstr_blk[blk]`).
    #[test]
    fn frame_exp_strat_table_spot_check_e2_10() {
        // Row 0: D15 R R R R R
        assert_eq!(FRAME_EXP_STRAT_TABLE[0], [D15, R, R, R, R, R]);
        // Row 16: D45 D15 R R R R (the prevailing corpus pattern — every
        // validator-encoded fixture in our corpus picks row 16 for fbw
        // channels).
        assert_eq!(FRAME_EXP_STRAT_TABLE[16], [D45, D15, R, R, R, R]);
        // Row 28: D45 D45 D45 D25 R R (used by the 64kbps low-rate
        // stereo fixture's frmcplexpstr).
        assert_eq!(FRAME_EXP_STRAT_TABLE[28], [D45, D45, D45, D25, R, R]);
        // Row 31: D45 across every block — fully refreshed exponents
        // each block, the most-bits / least-temporal-correlation choice.
        assert_eq!(FRAME_EXP_STRAT_TABLE[31], [D45, D45, D45, D45, D45, D45]);
        // Block 0 column shall never be REUSE per spec design (the decoder
        // needs concrete exponents on every syncframe's first block).
        for row in &FRAME_EXP_STRAT_TABLE {
            assert_ne!(row[0], R, "Table E2.10: block 0 must not be REUSE");
        }
    }

    /// `expstre == 0` path: the parser expands `frmchexpstr` /
    /// `frmcplexpstr` codewords into the per-block-per-channel strategy
    /// arrays via Table E2.10 — the audblk DSP needs them shaped the
    /// same as the `expstre == 1` path.
    #[test]
    fn parses_minimal_indep_stereo_audfrm_with_frame_exp_strat() {
        let bsi = make_bsi(2, false, 6, StreamType::Independent);
        let mut bits: Vec<(u32, u32)> = vec![(1, 0)]; // expstre = 0
        bits.push((1, 0)); // ahte
        bits.push((2, 0)); // snroffststr
        bits.push((1, 0)); // transproce
        bits.push((1, 1)); // blkswe
        bits.push((1, 1)); // dithflage
        bits.push((1, 1)); // bamode
        bits.push((1, 0)); // frmfgaincode
        bits.push((1, 1)); // dbaflde
        bits.push((1, 1)); // skipflde
        bits.push((1, 0)); // spxattene
                           // acmod>1 → cplinu[0]=1, then cplstre[1..5]=0 (sticky reuse keeps cplinu=1)
        bits.push((1, 1));
        for _ in 1..6 {
            bits.push((1, 0));
        }
        // expstre==0 + acmod>1 + ncplblks>0 → frmcplexpstr (5 bits).
        // Pick row 16 to match the prevailing corpus pattern.
        bits.push((5, 16));
        // 2 fbw channels × frmchexpstr (5 bits each).
        bits.push((5, 16));
        bits.push((5, 28));
        // strmtyp==0 + numblkscod==3 → convexpstre implicit, 2 × 5 bits.
        bits.push((5, 0));
        bits.push((5, 0));
        // snroffststr=0 → frmcsnroffst + frmfsnroffst.
        bits.push((6, 15));
        bits.push((4, 0));
        // num_blocks > 1 → blkstrtinfoe (1 bit, 0).
        bits.push((1, 0));

        let buf = pack_msb(&bits);
        let af = parse(&buf, &bsi).unwrap();
        assert!(!af.expstre);
        assert_eq!(af.ncplblks, 6);
        assert_eq!(af.frmcplexpstr, 16);
        assert_eq!(af.frmchexpstr[0], 16);
        assert_eq!(af.frmchexpstr[1], 28);
        // Expansion: row 16 = [D45, D15, R, R, R, R]; row 28 = [D45, D45, D45, D25, R, R].
        let exp16 = FRAME_EXP_STRAT_TABLE[16];
        let exp28 = FRAME_EXP_STRAT_TABLE[28];
        for blk in 0..6 {
            assert_eq!(af.cplexpstr_blk[blk], exp16[blk], "cpl[{blk}]");
            assert_eq!(af.chexpstr_blk_ch[blk][0], exp16[blk], "ch0[{blk}]");
            assert_eq!(af.chexpstr_blk_ch[blk][1], exp28[blk], "ch1[{blk}]");
        }
    }
}

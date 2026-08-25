//! AC-3 audio-block parser + DSP pipeline (§5.4.3, §7).
//!
//! This module walks `audblk()` bit-by-bit, running the full decoder
//! data-flow per Figure 6.1: unpack side-info → decode exponents → bit
//! allocation → unpack mantissas → decouple → rematrix → IMDCT →
//! window+overlap-add. Decoder state that must persist across audio
//! blocks inside a syncframe (and between syncframes for the
//! overlap-add delay line) lives on the `Ac3State` struct handed in by
//! the top-level decoder.
//!
//! The code is intentionally "big function per DSP stage" so each stage
//! matches a section of the spec 1:1.
//!
//! ## Scope
//!
//! - Full-bandwidth channels (fbw) + LFE are decoded.
//! - Coupling is supported for 2-channel streams (Table 7.24 coupling
//!   sub-bands, 7.4.3 coupling-coordinate reconstruction).
//! - Rematrixing (§7.5) is applied in 2/0 mode.
//! - Dynamic range compression (§7.7 dynrng) scales the transform
//!   coefficients.
//! - 512-point IMDCT with KBD window + 50% overlap-add (§7.9.4.1,
//!   §7.9.5). The 256-point short-block pair (§7.9.4.2) is wired
//!   through `crate::imdct::imdct_256_pair_fft`; block-switching
//!   correctness is gated by the `transient_bursts_stereo.ac3` PSNR
//!   test (the sine fixture never exercises `blksw=1`).

use oxideav_core::bits::BitReader;
use oxideav_core::{Error, Result};

use crate::bsi::Bsi;
use crate::syncinfo::SyncInfo;
use crate::tables::{
    BAPTAB, BNDSZ, BNDTAB, DBPBTAB, FASTDEC, FASTGAIN, FLOORTAB, HTH, LATAB, MANT_LEVEL_11,
    MANT_LEVEL_15, MANT_LEVEL_3, MANT_LEVEL_5, MANT_LEVEL_7, MASKTAB, QUANTIZATION_BITS, SLOWDEC,
    SLOWGAIN, WINDOW,
};

/// Maximum fbw channels (3/2 mode).
pub const MAX_FBW: usize = 5;
/// Total channel slots: fbw (5) + coupling pseudo-channel (1) + lfe (1) = 7.
pub const MAX_CHANNELS: usize = 7;
/// Number of transform coefficients per block.
pub const N_COEFFS: usize = 256;
/// Audio blocks per syncframe.
pub const BLOCKS_PER_FRAME: usize = 6;
/// New samples per block per channel after overlap-add.
pub const SAMPLES_PER_BLOCK: usize = 256;

/// Snapshot of every side-info field decoded out of an `audblk()`
/// element per §5.4.3. This is purely the "parse" half of the pipeline
/// — no exponents, no mantissas, no DSP state. It gives tests and the
/// downstream §7 stages a single inspectable record of what the
/// bit-stream actually said, keyed to the spec clause numbers.
///
/// Every field here cites its §5.4.3.x subsection in the doc comment
/// so an auditor can verify the parser against the spec table by table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AudBlkSideInfo {
    // ---- §5.4.3.1 blksw[ch] / §5.4.3.2 dithflag[ch] ----
    /// `blksw[ch]` — per-channel block-switch flag (§5.4.3.1, 1 bit).
    pub blksw: [bool; MAX_FBW],
    /// `dithflag[ch]` — per-channel dither flag (§5.4.3.2, 1 bit).
    pub dithflag: [bool; MAX_FBW],
    // ---- §5.4.3.3-6 dynamic range control ----
    /// `dynrnge` — dynamic-range word present (§5.4.3.3, 1 bit).
    pub dynrnge: bool,
    /// `dynrng` — 8-bit dynamic-range gain word (§5.4.3.4). Only
    /// meaningful when `dynrnge == true`.
    pub dynrng: u8,
    /// `dynrng2e` — dual-mono ch2 dynamic-range present (§5.4.3.5,
    /// 1 bit). Only present when `acmod == 0`.
    pub dynrng2e: bool,
    /// `dynrng2` — dual-mono ch2 dynamic-range word (§5.4.3.6, 8 bits).
    pub dynrng2: u8,
    // ---- §5.4.3.7-18 coupling strategy + coordinates ----
    /// `cplstre` — coupling strategy present in this block (§5.4.3.7).
    pub cplstre: bool,
    /// `cplinu` — coupling in use (§5.4.3.8). Valid only when `cplstre`.
    pub cplinu: bool,
    /// `chincpl[ch]` — channel is part of the coupling group
    /// (§5.4.3.9). Valid only when `cplinu`.
    pub chincpl: [bool; MAX_FBW],
    /// `phsflginu` — coupling phase flags in use for 2/0 (§5.4.3.10).
    pub phsflginu: bool,
    /// `cplbegf` — coupling begin frequency code (§5.4.3.11, 4 bits).
    pub cplbegf: u8,
    /// `cplendf` — coupling end frequency code (§5.4.3.12, 4 bits).
    pub cplendf: u8,
    /// `cplbndstrc[sbnd]` — coupling band structure (§5.4.3.13).
    pub cplbndstrc: [bool; 18],
    /// `cplcoe[ch]` — coupling-coordinates-present flag per channel
    /// (§5.4.3.14).
    pub cplcoe: [bool; MAX_FBW],
    // ---- §5.4.3.19-20 rematrix ----
    /// `rematstr` — rematrix strategy present (§5.4.3.19).
    pub rematstr: bool,
    /// Number of rematrix bands actually carried in `rematflg` per
    /// §5.4.3.19 rules and Table — 2/3/4 depending on `cplbegf`.
    pub rematflg_count: u8,
    /// `rematflg[rbnd]` — per-band rematrix flag (§5.4.3.20).
    pub rematflg: [bool; 4],
    // ---- §5.4.3.21-24 exponent strategy ----
    /// `cplexpstr` — coupling exponent strategy (§5.4.3.21, 2 bits).
    /// 0=reuse, 1=D15, 2=D25, 3=D45.
    pub cplexpstr: u8,
    /// `chexpstr[ch]` — full-bandwidth exponent strategy per channel
    /// (§5.4.3.22, 2 bits).
    pub chexpstr: [u8; MAX_FBW],
    /// `lfeexpstr` — LFE exponent strategy (§5.4.3.23, 1 bit).
    pub lfeexpstr: u8,
    /// `chbwcod[ch]` — channel bandwidth code (§5.4.3.24, 6 bits).
    /// Only meaningful when `chexpstr[ch] != 0 && !chincpl[ch]`.
    pub chbwcod: [u8; MAX_FBW],
    // ---- §5.4.3.30-46 bit-allocation parametric side-info ----
    /// `baie` — bit-allocation-info exists (§5.4.3.30).
    pub baie: bool,
    /// `snroffste` — SNR-offset block-level flag (§5.4.3.36).
    pub snroffste: bool,
    /// `cplleake` — coupling-leak-init flag (§5.4.3.44). Only when
    /// `cplinu`.
    pub cplleake: bool,
    // ---- §5.4.3.47 delta bit allocation ----
    /// `deltbaie` — delta-bit-allocation info exists (§5.4.3.47).
    pub deltbaie: bool,
    // ---- §5.4.3.58-59 skip field ----
    /// `skiple` — skip-field-exists flag (§5.4.3.58).
    pub skiple: bool,
    /// `skipl` — number of skip *bytes* (§5.4.3.59, 9 bits).
    pub skipl: u16,
}

/// Per-channel persistent state: exponents, bit-allocation pointers,
/// bandwidth, gain-range, and the 256-sample overlap-add delay line.
#[derive(Clone)]
pub struct ChannelState {
    pub exp: [u8; N_COEFFS],
    pub bap: [u8; N_COEFFS],
    pub psd: [i16; N_COEFFS],
    pub bndpsd: [i16; 50],
    pub mask: [i16; 50],
    pub deltba: [i16; 50],
    pub end_mant: usize,
    /// 256-sample tail from last block's MDCT, ready to be added into
    /// this block's output.
    pub delay: [f32; SAMPLES_PER_BLOCK],
    /// Raw dequantized transform coefficients for this block.
    pub coeffs: [f32; N_COEFFS],
    pub blksw: bool,
    pub dithflag: bool,
    /// Whether this channel is coupled (set from chincpl[]).
    pub in_coupling: bool,
    /// Dynrng gain multiplier (linear).
    pub dynrng: f32,
    /// E-AC-3 spectral extension (§E.3.6): whether this channel
    /// regenerates high-frequency transform coefficients via SPX this
    /// block. Base AC-3 never sets this (no SPX in the base layer), so
    /// the SPX synthesis step in [`dsp_block`] is a no-op for AC-3.
    pub in_spx: bool,
    /// Per-band SPX coordinate `spxco[ch][bnd]` (§E.3.6.3). Persisted
    /// across blocks so a `spxcoe[ch] == 0` block can reuse the prior
    /// coordinates.
    pub spx_coord: [f32; 18],
    /// Per-band SPX noise / signal blend factors `nblendfact` /
    /// `sblendfact` (§E.3.6.4.2.1). Recomputed when new coordinates
    /// (and hence a new `spxblnd`) arrive; reused otherwise.
    pub spx_nblend: [f32; 18],
    pub spx_sblend: [f32; 18],
    /// E-AC-3 spectral-extension attenuation (§3.6.4.2.3, §2.3.2.24-25).
    /// `spx_atten_active` mirrors the frame-level `chinspxatten[ch]` bit
    /// (a frame-scoped flag — the spec carries it in audfrm, not audblk,
    /// so it stays constant across the 6 blocks of a syncframe). When
    /// set, the SPX synthesis applies a 5-tap notch filter at the
    /// baseband/extension border (and at every wrap point during the
    /// translation copy) using row `spx_atten_code` of Table E3.14.
    pub spx_atten_active: bool,
    /// `spxattencod[ch]` — 5-bit index into Table E3.14
    /// (`SPX_ATTEN_TABLE`). Only meaningful when `spx_atten_active`.
    pub spx_atten_code: u8,
}

impl Default for ChannelState {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelState {
    pub fn new() -> Self {
        Self {
            exp: [24; N_COEFFS],
            bap: [0; N_COEFFS],
            psd: [0; N_COEFFS],
            bndpsd: [0; 50],
            mask: [0; 50],
            deltba: [0; 50],
            end_mant: 0,
            delay: [0.0; SAMPLES_PER_BLOCK],
            coeffs: [0.0; N_COEFFS],
            blksw: false,
            dithflag: false,
            in_coupling: false,
            dynrng: 1.0,
            in_spx: false,
            spx_coord: [0.0; 18],
            spx_nblend: [0.0; 18],
            spx_sblend: [0.0; 18],
            spx_atten_active: false,
            spx_atten_code: 0,
        }
    }
}

/// Per-frame decoder state that survives across audio blocks and across
/// syncframes (delay lines).
#[derive(Clone)]
pub struct Ac3State {
    /// [0..nfchans] = fbw channels, index MAX_FBW = coupling pseudo-channel,
    /// index MAX_FBW+1 = LFE.
    pub channels: [ChannelState; MAX_CHANNELS],

    // ---- Coupling state (§5.4.3.7 ff, §7.4) ----
    pub cpl_in_use: bool,
    pub phsflginu: bool,
    pub cpl_begf: u8,
    pub cpl_endf: u8,
    pub cpl_begf_mant: usize, // 37 + 12*cplbegf
    pub cpl_endf_mant: usize, // 37 + 12*(cplendf+3)
    pub cpl_nsubbnd: usize,
    pub cpl_nbnd: usize,
    /// cplbndstrc[sbnd], 1 when subband merges into previous band.
    pub cpl_bndstrc: [bool; 18],
    /// cplco[ch][bnd] linear coupling coordinate.
    pub cpl_coord: [[f32; 18]; MAX_FBW],
    pub cpl_coord_valid: [bool; MAX_FBW],
    pub cpl_phsflg: [bool; 18],

    // ---- Rematrix ----
    pub rematflg: [bool; 4],

    // ---- Bit-allocation parameters ----
    pub sdcycod: u8,
    pub fdcycod: u8,
    pub sgaincod: u8,
    pub dbpbcod: u8,
    pub floorcod: u8,
    pub snroffst_coarse: u8,
    pub cpl_fsnroffst: u8,
    pub cpl_fgaincod: u8,
    pub cpl_fleak: u8,
    pub cpl_sleak: u8,
    pub fsnroffst: [u8; MAX_FBW],
    pub fgaincod: [u8; MAX_FBW],
    pub lfefsnroffst: u8,
    pub lfefgaincod: u8,

    // ---- Delta bit allocation state (§5.4.3.47-57, §7.2.2.6) ----
    /// Per-channel deltba state: number of segments + per-segment offset/length/value.
    /// Each fbw channel has its own segment list; index `MAX_FBW` is the
    /// coupling channel. `deltnseg` of 0 means no delta-band processing.
    /// State is initialized to all-zero at the top of every syncframe (§7.2.2.6
    /// "initialize the cpldeltnseg and deltnseg[ch] delta bit allocation
    /// variables to 0 at the beginning of each syncframe") and updated by the
    /// per-block parser when `deltbae[ch] == 1` (new info follows) or cleared
    /// when `deltbae[ch] == 2` (perform no delta alloc this block). When
    /// `deltbae[ch] == 0` (reuse) or `deltbaie == 0` for blk > 0, the previous
    /// values are kept.
    pub deltnseg: [usize; MAX_FBW + 1],
    pub deltoffst: [[u8; 8]; MAX_FBW + 1],
    pub deltlen: [[u8; 8]; MAX_FBW + 1],
    pub deltba: [[u8; 8]; MAX_FBW + 1],

    // ---- E-AC-3 spectral extension region state (§E.3.6) ----
    /// Whether SPX is in use in the current block (`spxinu`). When false
    /// the SPX synthesis step in [`dsp_block`] does nothing.
    pub spx_in_use: bool,
    /// `spxstrtf` copy-start sub-band index → first copied tc# is
    /// `spx_bandtable(spxstrtf)`.
    pub spx_strtf: u8,
    /// First / one-past-last SPX sub-band (`spx_begin_subbnd` /
    /// `spx_end_subbnd`, §E.2.3.3.5-6).
    pub spx_begin_subbnd: usize,
    pub spx_end_subbnd: usize,
    /// SPX sub-band → band grouping (`spxbndstrc[]`, §E.2.3.3.8). Index
    /// is the absolute sub-band number; `true` means "merge into the
    /// previous band". Persisted so a `spxbndstrce == 0` block reuses
    /// the prior structure.
    pub spx_bndstrc: [bool; 18],
    /// Number of SPX bands and per-band size in transform coefficients
    /// (`nspxbnds` / `spxbndsztab[]`), derived from the sub-band range
    /// and `spx_bndstrc`.
    pub spx_nbnds: usize,
    pub spx_bndsztab: [usize; 18],
    /// 32-bit LFSR driving the SPX noise generator (§E.3.6.4.2). The
    /// spec leaves the noise sequence non-normative ("any reasonably
    /// random sequence"); a fixed seed keeps decodes reproducible.
    pub spx_noise_lfsr: u32,

    /// Bit position immediately after the BSI (start of block 0 bits).
    pub audblk_start_bits: u64,
    /// Which block we are currently parsing (0..6).
    pub blkidx: usize,
    /// 16-bit LFSR state driving the `bap=0` dither replacement
    /// (§7.3.4). Persisted across audio blocks and syncframes so the
    /// dither sequence has a smooth long-period character.
    pub dither_lfsr_state: u32,
    /// Monotonically increasing syncframe counter used for trace gating
    /// (e.g. `AC3_TRACE_FRAME=14`). Incremented at the top of every
    /// `decode_frame` call. Not part of the spec — diagnostic only.
    pub frame_counter: u64,

    /// E-AC-3 enhanced coupling (§E.3.5.5): when set, the §7.4 standard
    /// decouple step in [`dsp_block`] is skipped because each coupled
    /// channel's transform coefficients in `channels[ch].coeffs` were
    /// already reconstructed from the enhanced-coupling carrier `Z[k]`
    /// (the §E.3.5.5.4 complex product) by the E-AC-3 dsp layer before
    /// `dsp_block` runs. Base AC-3 and standard E-AC-3 coupling leave
    /// this `false` so the normal decouple applies.
    pub skip_decouple: bool,

    /// E-AC-3 enhanced-coupling cross-frame synthesis state (§E.3.5.5.3
    /// random de-correlation sources). The non-transient random arrays are
    /// "generated once … and the same for every block of every frame", and
    /// the transient random generator advances across blocks/frames — both
    /// lifetimes outlive a single syncframe, so the state lives here. Base
    /// AC-3 and standard E-AC-3 coupling never touch it.
    pub ecpl_state: crate::eac3::ecpl::EcplState,

    /// §6.1.9 / §7.7 dynamic-range control settings. Steers how the
    /// per-block `dynrng` word (and, in RF mode, the frame-level `compr`
    /// word) is turned into the linear coefficient gain. [`Default`] is
    /// line-out — the mandatory §7.7.1 full-`dynrng` decode — so existing
    /// behaviour is unchanged unless a caller opts in via the decoder's
    /// DRC API.
    pub drc: crate::drc::DrcSettings,
}

impl Default for Ac3State {
    fn default() -> Self {
        Self::new()
    }
}

impl Ac3State {
    pub fn new() -> Self {
        Self {
            channels: std::array::from_fn(|_| ChannelState::new()),
            cpl_in_use: false,
            phsflginu: false,
            cpl_begf: 0,
            cpl_endf: 0,
            cpl_begf_mant: 0,
            cpl_endf_mant: 0,
            cpl_nsubbnd: 0,
            cpl_nbnd: 0,
            cpl_bndstrc: [false; 18],
            cpl_coord: [[0.0; 18]; MAX_FBW],
            cpl_coord_valid: [false; MAX_FBW],
            cpl_phsflg: [false; 18],
            rematflg: [false; 4],
            sdcycod: 0,
            fdcycod: 0,
            sgaincod: 0,
            dbpbcod: 0,
            floorcod: 0,
            snroffst_coarse: 0,
            cpl_fsnroffst: 0,
            cpl_fgaincod: 0,
            cpl_fleak: 0,
            cpl_sleak: 0,
            fsnroffst: [0; MAX_FBW],
            fgaincod: [0; MAX_FBW],
            lfefsnroffst: 0,
            lfefgaincod: 0,
            deltnseg: [0; MAX_FBW + 1],
            deltoffst: [[0; 8]; MAX_FBW + 1],
            deltlen: [[0; 8]; MAX_FBW + 1],
            deltba: [[0; 8]; MAX_FBW + 1],
            spx_in_use: false,
            spx_strtf: 0,
            spx_begin_subbnd: 0,
            spx_end_subbnd: 0,
            spx_bndstrc: [false; 18],
            spx_nbnds: 0,
            spx_bndsztab: [0; 18],
            spx_noise_lfsr: 0x4A5B_6C7D,
            audblk_start_bits: 0,
            blkidx: 0,
            // Non-zero seed so the LFSR doesn't get stuck on all-zeros.
            // Arbitrary fixed value keeps decodes byte-reproducible.
            dither_lfsr_state: 0x1234,
            frame_counter: 0,
            skip_decouple: false,
            ecpl_state: crate::eac3::ecpl::EcplState::new(),
            drc: crate::drc::DrcSettings::default(),
        }
    }
}

/// Parse (but do not DSP) one syncframe's 6 audio blocks, returning
/// the per-block [`AudBlkSideInfo`] snapshots. Used by tests and
/// introspection tools; the decoder itself calls [`decode_frame`]
/// which fuses parse + DSP.
///
/// Side-info capture mirrors `decode_frame`'s bit-cursor: after each
/// block's side-info `parse_audblk_into` itself consumes the mantissa
/// region via [`unpack_mantissas`], so the cursor naturally lands on
/// block N+1's bits without a second walk here (a previous version
/// double-consumed the mantissas, which made every block N>0 read its
/// side-info bits from somewhere inside the previous block's mantissa
/// region).
/// Blocks whose parse fails (e.g. due to our current bit-allocation
/// approximation consuming a few mantissa bits too many) yield a
/// `Default::default()` snapshot and subsequent blocks restart from
/// the last good cursor — matching the decoder's graceful-degradation
/// policy for the §7 stages.
pub fn parse_frame_side_info(
    si: &SyncInfo,
    bsi: &Bsi,
    frame_bytes: &[u8],
) -> Result<[AudBlkSideInfo; BLOCKS_PER_FRAME]> {
    let mut state = Ac3State::new();
    let post_sync = &frame_bytes[5..];
    let mut br = BitReader::new(post_sync);
    br.skip(bsi.bits_consumed as u32)?;
    let mut out: [AudBlkSideInfo; BLOCKS_PER_FRAME] = Default::default();
    for blk in 0..BLOCKS_PER_FRAME {
        state.blkidx = blk;
        let mut side = AudBlkSideInfo::default();
        if parse_audblk_into(&mut state, si, bsi, &mut br, &mut side).is_ok() {
            out[blk] = side;
        } else {
            // Parse error — stop side-info capture here. Downstream
            // blocks are unreachable without a known bit-position.
            break;
        }
    }
    Ok(out)
}

/// Decode one syncframe of 6 audio blocks into interleaved f32 samples.
/// Output length = 1536 × nchans.
pub fn decode_frame(
    state: &mut Ac3State,
    si: &SyncInfo,
    bsi: &Bsi,
    frame_bytes: &[u8],
    out: &mut [f32],
) -> Result<()> {
    // Slice starting at the beginning of BSI (byte 5 of syncframe).
    let post_sync = &frame_bytes[5..];
    let mut br = BitReader::new(post_sync);
    // Consume the BSI bits so we start exactly at audio block 0.
    br.skip(bsi.bits_consumed as u32)?;
    state.audblk_start_bits = br.bit_position();
    state.blkidx = 0;
    // §7.2.2.6 / A/52 §5.4.3.47: the cpldeltnseg and deltnseg[ch] delta
    // bit-allocation segment counts must be initialised to 0 at the
    // start of every syncframe so a `deltbaie == 0` / `deltbae == reuse`
    // block-0 inherits "no delta", not stale segments left over from the
    // previous frame's dba. Without this reset a frame whose block 0
    // reuses (deltbae == 0) a prior frame's segments applies a phantom
    // mask offset, perturbing bap[] and desynchronising mantissa unpack.
    for d in state.deltnseg.iter_mut() {
        *d = 0;
    }

    let nchans = bsi.nchans as usize; // output channel count (fbw + lfe)
    let nfchans = bsi.nfchans as usize;

    for blk in 0..BLOCKS_PER_FRAME {
        state.blkidx = blk;
        // Tolerate bit-exhaustion in later blocks: if parsing or mantissa
        // unpack runs out of bits, zero-fill this block's coefficients and
        // keep going. This matches the spec's graceful-degradation
        // guidance for corrupt streams and also compensates for the
        // current bit-allocation approximation producing slightly more
        // mantissas than the encoder actually wrote.
        if parse_audblk(state, si, bsi, &mut br).is_err() {
            for ch in 0..MAX_CHANNELS {
                for v in state.channels[ch].coeffs.iter_mut() {
                    *v = 0.0;
                }
            }
        }
        if std::env::var("AC3_TRACE_BITPOS").is_ok() {
            // Round-12 diagnostic: verify per-block bit-cursor lands inside
            // the syncframe (frame_bits ≈ 6104 for a 192 kbps frame; CRC
            // and a few padding bits sit between the last block's end and
            // the frame end). If `end_pos` ever exceeds `frame_bits` the
            // parser is over-consuming and every later block's side-info
            // bits will be misread.
            eprintln!(
                "TRACE-BITPOS frame={} blk={} end_pos={} frame_bits={}",
                state.frame_counter,
                blk,
                br.bit_position(),
                (frame_bytes.len() as u64 - 5) * 8
            );
        }
        dsp_block(state, si, bsi);
        // Write this block's SAMPLES_PER_BLOCK samples per channel
        // interleaved into `out` starting at block offset.
        let base = blk * SAMPLES_PER_BLOCK * nchans;
        for n in 0..SAMPLES_PER_BLOCK {
            for ch in 0..nfchans {
                let s = state.channels[ch].coeffs[n];
                out[base + n * nchans + ch] = s;
            }
            if bsi.lfeon {
                let s = state.channels[MAX_FBW + 1].coeffs[n];
                out[base + n * nchans + nfchans] = s;
            }
        }
    }
    // Diagnostic frame counter (gated by `AC3_TRACE_FRAME=N`). Increment
    // *after* the frame so frame 0 == first decoded frame; not part of
    // the spec.
    state.frame_counter = state.frame_counter.saturating_add(1);
    Ok(())
}

fn parse_audblk(state: &mut Ac3State, si: &SyncInfo, bsi: &Bsi, br: &mut BitReader) -> Result<()> {
    let mut side = AudBlkSideInfo::default();
    parse_audblk_into(state, si, bsi, br, &mut side)
}

/// Parse one `audblk()` element into [`Ac3State`] (for DSP) and
/// [`AudBlkSideInfo`] (for tests / introspection). Every bit field
/// cites its §5.4.3.x clause; the pseudo-code in Table 5.3 was the
/// authoritative reference for bit-order. Consumes exactly as many
/// bits as the spec prescribes, up to the end of the skip field; the
/// tail-end mantissas are then parsed by [`unpack_mantissas`].
pub(crate) fn parse_audblk_into(
    state: &mut Ac3State,
    si: &SyncInfo,
    bsi: &Bsi,
    br: &mut BitReader,
    side: &mut AudBlkSideInfo,
) -> Result<()> {
    let _ = si;
    let nfchans = bsi.nfchans as usize;
    let acmod = bsi.acmod;
    let blk = state.blkidx;

    // §5.4.3.1 blksw[ch] — per-channel block-switch flag (1 bit each).
    for ch in 0..nfchans {
        let v = br.read_u32(1)? != 0;
        state.channels[ch].blksw = v;
        side.blksw[ch] = v;
    }
    // §5.4.3.2 dithflag[ch] — per-channel dither flag (1 bit each).
    for ch in 0..nfchans {
        let v = br.read_u32(1)? != 0;
        state.channels[ch].dithflag = v;
        side.dithflag[ch] = v;
    }

    // The frame-level §5.4.2.10 heavy-compression words, consulted only
    // when the DRC control surface is in RF mode (§7.7.2.1). `compr`
    // drives ch1 (and every fbw channel for acmod != 0); `compr_ch2`
    // drives ch2 in 1+1 dual mono.
    let compr_ch1 = bsi.compr.map(|c| c.raw());
    let compr_ch2 = bsi.compr_ch2.map(|c| c.raw());
    // §5.4.3.3 dynrnge — dynamic-range word present (1 bit).
    let dynrnge = br.read_u32(1)? != 0;
    side.dynrnge = dynrnge;
    if dynrnge {
        // §5.4.3.4 dynrng — 8-bit dynamic-range gain word. The DRC
        // control surface (§7.7.1.2 partial compression / §7.7.2 heavy
        // compression) maps the raw word to the applied linear gain;
        // line-out (the default) reproduces the bare §7.7.1.2 word.
        let dynrng = br.read_u32(8)? as u8;
        side.dynrng = dynrng;
        let g = state.drc.resolve_block_gain(dynrng, compr_ch1);
        for ch in 0..nfchans {
            state.channels[ch].dynrng = g;
        }
    } else if blk == 0 {
        // §7.7.1.2 — block 0 with no dynrng word uses '0000 0000' (0 dB).
        // In RF mode that 0 dB dynrng still yields to the frame's compr
        // word, so route the block-0 default through the same resolver.
        let g = state.drc.resolve_block_gain(0x00, compr_ch1);
        for ch in 0..nfchans {
            state.channels[ch].dynrng = g;
        }
    }
    // §5.4.3.5 dynrng2e — dual-mono ch2 dynamic-range present (1 bit),
    // §5.4.3.6 dynrng2 — dual-mono ch2 dynamic-range word (8 bits).
    if acmod == 0 {
        let dynrng2e = br.read_u32(1)? != 0;
        side.dynrng2e = dynrng2e;
        if dynrng2e {
            let d2 = br.read_u32(8)? as u8;
            side.dynrng2 = d2;
            state.channels[1].dynrng = state.drc.resolve_block_gain(d2, compr_ch2);
        } else if blk == 0 {
            state.channels[1].dynrng = state.drc.resolve_block_gain(0x00, compr_ch2);
        }
    }

    // §5.4.3.7 cplstre — coupling strategy present (1 bit).
    let cplstre = br.read_u32(1)? != 0;
    side.cplstre = cplstre;
    if cplstre {
        // §5.4.3.8 cplinu — coupling in use (1 bit).
        state.cpl_in_use = br.read_u32(1)? != 0;
        side.cplinu = state.cpl_in_use;
        if state.cpl_in_use {
            // §5.4.3.9 chincpl[ch] — per-channel coupling membership (1 bit).
            for ch in 0..nfchans {
                let v = br.read_u32(1)? != 0;
                state.channels[ch].in_coupling = v;
                side.chincpl[ch] = v;
            }
            // §5.4.3.10 phsflginu — phase flags in use (only in 2/0 mode).
            state.phsflginu = if acmod == 0x2 {
                br.read_u32(1)? != 0
            } else {
                false
            };
            side.phsflginu = state.phsflginu;
            // §5.4.3.11 cplbegf (4 bits), §5.4.3.12 cplendf (4 bits).
            state.cpl_begf = br.read_u32(4)? as u8;
            state.cpl_endf = br.read_u32(4)? as u8;
            side.cplbegf = state.cpl_begf;
            side.cplendf = state.cpl_endf;
            // Per A/52 §5.4.3.12 the upper sub-band index is `cplendf+2`,
            // so the spec's validity envelope is `cplbegf <= cplendf+2`
            // (equivalently `ncplsubnd = 3 + cplendf - cplbegf >= 1`).
            // The earlier strict `cplendf < cplbegf` rejection bombed out
            // of valid 5.0 (acmod=7 lfeon=0) frames whose bitstreams pick
            // narrow-coupling configs like (cplbegf=11, cplendf=10), which
            // place coupling on sub-bands 11..=12 (tc bins 169..193) — a
            // perfectly legal 2-sub-band coupling channel. Using signed
            // arithmetic also dodges the usize underflow that the previous
            // branch would have hit before the explicit check.
            let nsub = 3i32 + state.cpl_endf as i32 - state.cpl_begf as i32;
            if nsub < 1 {
                return Err(Error::invalid(
                    "ac3: §5.4.3.11/12 cplbegf > cplendf+2 — malformed coupling range",
                ));
            }
            state.cpl_nsubbnd = nsub as usize;
            // §5.4.3.13 cplbndstrc[sbnd] — 1 bit per subband for sbnd >= 1.
            state.cpl_bndstrc[0] = false;
            for bnd in 1..state.cpl_nsubbnd {
                let v = br.read_u32(1)? != 0;
                state.cpl_bndstrc[bnd] = v;
                side.cplbndstrc[bnd] = v;
            }
            // Mantissa-domain coupling range: bins [37 + 12*cplbegf,
            // 37 + 12*(cplendf+3)) per §7.4.2.
            state.cpl_begf_mant = 37 + 12 * state.cpl_begf as usize;
            state.cpl_endf_mant = 37 + 12 * (state.cpl_endf as usize + 3);
            // Derive ncplbnd by merging sub-bands whose cplbndstrc=1.
            let mut n = state.cpl_nsubbnd;
            for bnd in 1..state.cpl_nsubbnd {
                if state.cpl_bndstrc[bnd] {
                    n -= 1;
                }
            }
            state.cpl_nbnd = n;
        }
    }

    // §5.4.3.14 cplcoe[ch], §5.4.3.15 mstrcplco[ch], §5.4.3.16 cplcoexp,
    // §5.4.3.17 cplcomant, §5.4.3.18 phsflg[bnd].
    if state.cpl_in_use {
        let mut any = false;
        for ch in 0..nfchans {
            if state.channels[ch].in_coupling {
                // §5.4.3.14 cplcoe[ch] — coupling coordinates present (1 bit).
                let cplcoe = br.read_u32(1)? != 0;
                side.cplcoe[ch] = cplcoe;
                if cplcoe {
                    any = true;
                    // §5.4.3.15 mstrcplco[ch] — master coupling coord (2 bits).
                    let mstrcplco = br.read_u32(2)? as i32;
                    for bnd in 0..state.cpl_nbnd {
                        // §5.4.3.16 cplcoexp[ch][bnd] — 4 bits.
                        let cplcoexp = br.read_u32(4)? as i32;
                        // §5.4.3.17 cplcomant[ch][bnd] — 4 bits.
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
            }
        }
        // §5.4.3.18 phsflg[bnd] — only when 2/0, phsflginu, and at
        // least one channel emitted coupling coordinates this block.
        if acmod == 0x2 && state.phsflginu && any {
            for bnd in 0..state.cpl_nbnd {
                state.cpl_phsflg[bnd] = br.read_u32(1)? != 0;
            }
        }
    }

    // §5.4.3.19 rematstr — rematrix strategy (only in 2/0 mode).
    // §5.4.3.20 rematflg[rbnd] — per-band rematrix flag.
    if acmod == 0x2 {
        let rematstr = br.read_u32(1)? != 0;
        side.rematstr = rematstr;
        if rematstr {
            let n_remat = remat_band_count(state.cpl_in_use, state.cpl_begf);
            side.rematflg_count = n_remat as u8;
            for rbnd in 0..n_remat {
                let v = br.read_u32(1)? != 0;
                state.rematflg[rbnd] = v;
                side.rematflg[rbnd] = v;
            }
        }
        if std::env::var("AC3_TRACE_REMAT").is_ok() {
            eprintln!(
                "TRACE-REMAT blk={} rematstr={} rematflg={:?}",
                blk, rematstr, state.rematflg
            );
        }
    }

    // §5.4.3.21 cplexpstr — coupling exponent strategy (2 bits).
    // §5.4.3.22 chexpstr[ch] — fbw channel exponent strategy (2 bits).
    // §5.4.3.23 lfeexpstr — LFE exponent strategy (1 bit).
    // §5.4.3.24 chbwcod[ch] — channel bandwidth code (6 bits).
    let mut cplexpstr = 0u8;
    let mut chexpstr = [0u8; MAX_FBW];
    let mut lfeexpstr = 0u8;
    if state.cpl_in_use {
        cplexpstr = br.read_u32(2)? as u8;
    }
    side.cplexpstr = cplexpstr;
    for ch in 0..nfchans {
        chexpstr[ch] = br.read_u32(2)? as u8;
        side.chexpstr[ch] = chexpstr[ch];
    }
    if bsi.lfeon {
        lfeexpstr = br.read_u32(1)? as u8;
    }
    side.lfeexpstr = lfeexpstr;
    // chbwcod — only for non-coupled independent fbw channels with new exponents.
    let mut chbwcod = [0u8; MAX_FBW];
    for ch in 0..nfchans {
        if chexpstr[ch] != 0 && !state.channels[ch].in_coupling {
            chbwcod[ch] = br.read_u32(6)? as u8;
            side.chbwcod[ch] = chbwcod[ch];
            if chbwcod[ch] > 60 {
                return Err(Error::invalid("ac3: chbwcod > 60"));
            }
        }
    }

    // --- unpack coupling exponents ---
    if state.cpl_in_use && cplexpstr != 0 {
        let cplabsexp = br.read_u32(4)? as i32;
        let cpl_start = state.cpl_begf_mant;
        let cpl_end = state.cpl_endf_mant;
        let grpsize = match cplexpstr {
            1 => 1,
            2 => 2,
            3 => 4,
            _ => 1,
        };
        let ncplgrps = (cpl_end - cpl_start) / (grpsize * 3);
        // Absolute exponent for coupling: cplabsexp << 1 (from 4-bit range 0..15 to full 5-bit 0..30).
        let mut raw_exp = vec![0i32; ncplgrps * 3];
        decode_exponents(
            br,
            cplabsexp << 1,
            ncplgrps,
            cplexpstr as usize,
            &mut raw_exp,
        )?;
        let ch_idx = MAX_FBW;
        // Offset for coupling per spec: cplexp[n + cplstrtmant] = exp[n+1],
        // i.e. the absolute exponent is used as a reference only and the
        // actual exponents start at index 1.
        for (i, e) in raw_exp.iter().enumerate() {
            let idx = cpl_start + i * grpsize;
            for j in 0..grpsize {
                if idx + j < N_COEFFS {
                    state.channels[ch_idx].exp[idx + j] = (*e).clamp(0, 24) as u8;
                }
            }
        }
        if std::env::var("AC3_TRACE_CPL").is_ok() {
            eprintln!(
                "TRACE-CPL blk={} cplexpstr={} cplabsexp={} (<<1={}) ncplgrps={} grpsize={}",
                blk,
                cplexpstr,
                cplabsexp,
                cplabsexp << 1,
                ncplgrps,
                grpsize
            );
            eprintln!(
                "TRACE-CPL raw_exp first 12: {:?}",
                &raw_exp[..raw_exp.len().min(12)]
            );
            eprintln!(
                "TRACE-CPL placed cpl exp[{}..{}]: {:?}",
                cpl_start,
                cpl_end,
                &state.channels[ch_idx].exp[cpl_start..cpl_end.min(cpl_start + 30)]
            );
        }
    }

    // --- unpack fbw channel exponents + gainrng ---
    for ch in 0..nfchans {
        if chexpstr[ch] != 0 {
            let strt = 0usize;
            let end = if state.channels[ch].in_coupling {
                state.cpl_begf_mant
            } else {
                37 + 3 * (chbwcod[ch] as usize + 12)
            };
            state.channels[ch].end_mant = end;

            // reason: kept as a compile-time-disabled debug probe that is flipped
            // to `true` locally when inspecting exponent strategy bit alignment.
            #[allow(clippy::overly_complex_bool_expr)]
            if false && blk == 0 {
                eprintln!(
                    "ch{} exps start at bit {}, end_mant={}, strategy={}",
                    ch,
                    br.bit_position(),
                    end,
                    chexpstr[ch]
                );
            }
            let absexp = br.read_u32(4)? as i32;
            let grpsize = match chexpstr[ch] {
                1 => 1,
                2 => 2,
                3 => 4,
                _ => 1,
            };
            // Guard against `end == 0` (rare: a fully-coupled channel
            // whose cpl_begf_mant lands at 0 has no independent
            // exponents). Without this guard the `(end - 1) / k` term
            // would underflow under debug arithmetic and panic the
            // decoder mid-frame.
            let nchgrps = if end == 0 {
                0usize
            } else {
                match chexpstr[ch] {
                    1 => (end - 1) / 3,
                    2 => (end - 1 + 3) / 6,
                    3 => (end - 1 + 9) / 12,
                    _ => 0,
                }
            };
            let mut raw_exp = vec![0i32; nchgrps * 3];
            decode_exponents(br, absexp, nchgrps, chexpstr[ch] as usize, &mut raw_exp)?;
            // Place: exp[0] = absexp; exp[i*grpsize + 1 + j] = raw_exp[i]
            state.channels[ch].exp[strt] = absexp.clamp(0, 24) as u8;
            for (i, e) in raw_exp.iter().enumerate() {
                let base = i * grpsize + 1;
                for j in 0..grpsize {
                    if base + j < end {
                        state.channels[ch].exp[base + j] = (*e).clamp(0, 24) as u8;
                    }
                }
            }
            let _gainrng = br.read_u32(2)?;
        } else if blk == 0 {
            return Err(Error::invalid("ac3: chexpstr=0 in block 0"));
        }
    }

    // --- unpack LFE exponents ---
    if bsi.lfeon {
        let lfe_ch = MAX_FBW + 1;
        state.channels[lfe_ch].end_mant = 7;
        if lfeexpstr != 0 {
            let absexp = br.read_u32(4)? as i32;
            let nlfegrps = 2usize;
            let mut raw_exp = vec![0i32; nlfegrps * 3];
            decode_exponents(br, absexp, nlfegrps, 1, &mut raw_exp)?;
            state.channels[lfe_ch].exp[0] = absexp.clamp(0, 24) as u8;
            for (i, e) in raw_exp.iter().enumerate() {
                if i + 1 < 7 {
                    state.channels[lfe_ch].exp[i + 1] = (*e).clamp(0, 24) as u8;
                }
            }
        }
    }

    // §5.4.3.30 baie — bit-allocation info exists (1 bit).
    // §5.4.3.31-35 sdcycod/fdcycod/sgaincod/dbpbcod/floorcod parametric
    // masking words, present iff baie.
    let baie = br.read_u32(1)? != 0;
    side.baie = baie;
    if baie {
        state.sdcycod = br.read_u32(2)? as u8;
        state.fdcycod = br.read_u32(2)? as u8;
        state.sgaincod = br.read_u32(2)? as u8;
        state.dbpbcod = br.read_u32(2)? as u8;
        state.floorcod = br.read_u32(3)? as u8;
    }
    // §5.4.3.36 snroffste — SNR-offset block flag (1 bit).
    let snroffste = br.read_u32(1)? != 0;
    side.snroffste = snroffste;
    if snroffste {
        // §5.4.3.37 csnroffst — coarse SNR offset (6 bits).
        state.snroffst_coarse = br.read_u32(6)? as u8;
        if state.cpl_in_use {
            // §5.4.3.38 cplfsnroffst (4 bits), §5.4.3.39 cplfgaincod (3 bits).
            state.cpl_fsnroffst = br.read_u32(4)? as u8;
            state.cpl_fgaincod = br.read_u32(3)? as u8;
        }
        for ch in 0..nfchans {
            // §5.4.3.40 fsnroffst[ch] (4 bits), §5.4.3.41 fgaincod[ch] (3 bits).
            state.fsnroffst[ch] = br.read_u32(4)? as u8;
            state.fgaincod[ch] = br.read_u32(3)? as u8;
        }
        if bsi.lfeon {
            // §5.4.3.42 lfefsnroffst (4 bits), §5.4.3.43 lfefgaincod (3 bits).
            state.lfefsnroffst = br.read_u32(4)? as u8;
            state.lfefgaincod = br.read_u32(3)? as u8;
        }
    }
    // §5.4.3.44 cplleake — coupling leak init flag (1 bit).
    // §5.4.3.45 cplfleak (3 bits), §5.4.3.46 cplsleak (3 bits).
    if state.cpl_in_use {
        let cplleake = br.read_u32(1)? != 0;
        side.cplleake = cplleake;
        if cplleake {
            state.cpl_fleak = br.read_u32(3)? as u8;
            state.cpl_sleak = br.read_u32(3)? as u8;
        }
    }

    // §5.4.3.47 deltbaie — delta bit allocation info exists (1 bit).
    // §5.4.3.48-57 — per-channel + coupling delta bit allocation segments.
    // §7.2.2.6 — apply per-band ±6 dB mask offsets BEFORE final bit
    // allocation. Critical for transient blocks where the encoder uses
    // dba to lift the masking floor in low-energy bands so they don't
    // get assigned mantissa bits the encoder needs for the burst peak.
    let deltbaie = br.read_u32(1)? != 0;
    side.deltbaie = deltbaie;
    if deltbaie {
        let cpl_idx = MAX_FBW;
        let mut cpldeltbae = 0u32;
        if state.cpl_in_use {
            cpldeltbae = br.read_u32(2)?;
        }
        let mut deltbae = [0u32; MAX_FBW];
        for ch in 0..nfchans {
            deltbae[ch] = br.read_u32(2)?;
        }
        // Per Table 5.16: 0=reuse, 1=new info, 2=no delta this block, 3=reserved.
        if state.cpl_in_use {
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
                    // "perform no delta alloc" — clear segments for this block.
                    state.deltnseg[cpl_idx] = 0;
                }
                _ => {
                    // 0 = reuse previous; 3 = reserved (treat as reuse to stay
                    // robust against malformed streams).
                }
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
        // §5.4.3.47 spec: "If deltbaie is '0' in block 0, then cpldeltbae
        // and deltbae[ch] are set to the binary value '10', and no delta
        // bit allocation is applied." This means clear all segments.
        for ch in 0..MAX_FBW + 1 {
            state.deltnseg[ch] = 0;
        }
    }

    // §5.4.3.58 skiple — skip-length-exists flag (1 bit).
    // §5.4.3.59 skipl — skip length in *bytes* (9 bits).
    // §5.4.3.60 skipfld — `skipl × 8` skip-data bits.
    let skiple = br.read_u32(1)? != 0;
    side.skiple = skiple;
    if skiple {
        let skipl = br.read_u32(9)?;
        side.skipl = skipl as u16;
        br.skip(skipl * 8)?;
    }

    // --- run bit allocation per channel ---
    for ch in 0..nfchans {
        let end = state.channels[ch].end_mant;
        run_bit_allocation(
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
    if state.cpl_in_use {
        let start = state.cpl_begf_mant;
        let end = state.cpl_endf_mant;
        run_bit_allocation(
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
    if bsi.lfeon {
        let lfe_ch = MAX_FBW + 1;
        run_bit_allocation(
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

    // --- unpack mantissas ---
    if std::env::var("AC3_DEBUG_FULL").is_ok() && blk == 0 {
        let mut histo = [0u32; 16];
        for ch in 0..nfchans {
            let end = state.channels[ch].end_mant;
            for bin in 0..end {
                histo[state.channels[ch].bap[bin] as usize] += 1;
            }
        }
        if state.cpl_in_use {
            for bin in state.cpl_begf_mant..state.cpl_endf_mant {
                histo[state.channels[MAX_FBW].bap[bin] as usize] += 1;
            }
        }
        eprintln!("block 0 bap histogram (incl cpl): {:?}", histo);
        eprintln!(
            "ch_cpl bap[begf..end]: {:?}",
            &state.channels[MAX_FBW].bap
                [state.cpl_begf_mant..state.cpl_endf_mant.min(state.cpl_begf_mant + 30)]
        );
        eprintln!(
            "ch_cpl exp[begf..end]: {:?}",
            &state.channels[MAX_FBW].exp
                [state.cpl_begf_mant..state.cpl_endf_mant.min(state.cpl_begf_mant + 30)]
        );
        eprintln!("ch0 bap[0..133]: {:?}", &state.channels[0].bap[0..133]);
        eprintln!("ch1 bap[0..133]: {:?}", &state.channels[1].bap[0..133]);
        eprintln!("ch1 exp[0..133]: {:?}", &state.channels[1].exp[0..133]);
        eprintln!("ch0 exp[0..40]: {:?}", &state.channels[0].exp[0..40]);
        eprintln!("ch0 psd[0..10]: {:?}", &state.channels[0].psd[0..10]);
        eprintln!("ch0 bndpsd[0..10]: {:?}", &state.channels[0].bndpsd[0..10]);
        eprintln!(
            "cpl in_use: {} begf: {} endf: {} begf_mant: {} endf_mant: {} nsubbnd: {} nbnd: {}",
            state.cpl_in_use,
            state.cpl_begf,
            state.cpl_endf,
            state.cpl_begf_mant,
            state.cpl_endf_mant,
            state.cpl_nsubbnd,
            state.cpl_nbnd
        );
        eprintln!(
            "ch0 end_mant: {}, ch1 end_mant: {}",
            state.channels[0].end_mant, state.channels[1].end_mant
        );
        eprintln!(
            "snroffst: csnr={} cpl_fsnr={} cpl_fgain={} cpl_fleak={} cpl_sleak={}",
            state.snroffst_coarse,
            state.cpl_fsnroffst,
            state.cpl_fgaincod,
            state.cpl_fleak,
            state.cpl_sleak
        );
        eprintln!(
            "sdcy={} fdcy={} sgain={} dbpb={} floor={}",
            state.sdcycod, state.fdcycod, state.sgaincod, state.dbpbcod, state.floorcod
        );
        eprintln!("ch0 psd[0..20]: {:?}", &state.channels[0].psd[0..20]);
        eprintln!("ch0 bndpsd[0..20]: {:?}", &state.channels[0].bndpsd[0..20]);
        eprintln!("ch0 mask[0..20]: {:?}", &state.channels[0].mask[0..20]);
        let total_bits: u32 = histo
            .iter()
            .enumerate()
            .map(|(b, n)| match b {
                0 => 0,
                1 => 5 * n / 3 + (if n % 3 != 0 { 5 } else { 0 }),
                2 => 7 * n / 3 + (if n % 3 != 0 { 7 } else { 0 }),
                3 => 3 * n,
                4 => 7 * n / 2 + (if n % 2 != 0 { 7 } else { 0 }),
                5 => 4 * n,
                _ => crate::tables::QUANTIZATION_BITS[b] as u32 * n,
            })
            .sum();
        eprintln!(
            "block 0 pre-mantissa bit pos {}, estimated mantissa bits {}",
            br.bit_position(),
            total_bits
        );
    }
    unpack_mantissas(state, bsi, br)?;

    Ok(())
}

/// Number of rematrix bands (Table 5.15).
//
// reason: the two `4` arms are spec-faithful — Table 5.15 lists nrematbnd=4
// for both "coupling not in use" and "cplbegf > 2". Collapsing them would
// obscure the table structure for future audits.
#[allow(clippy::if_same_then_else)]
pub(crate) fn remat_band_count(cplinu: bool, cplbegf: u8) -> usize {
    if !cplinu {
        4
    } else if cplbegf > 2 {
        4
    } else if cplbegf > 0 {
        3
    } else {
        2
    }
}

/// E-AC-3 number-of-rematrix-bands `nrematbd` per §E.3.3.2, which folds
/// in both spectral extension and enhanced coupling.
///
/// The §E.3.3.2 pseudo-code decision tree, transcribed verbatim:
///
/// ```text
/// if (cplinu) {
///     if (ecplinu) {                         // enhanced coupling
///         if      (ecplbegf == 0) nrematbd = 0
///         else if (ecplbegf == 1) nrematbd = 1
///         else if (ecplbegf == 2) nrematbd = 2
///         else if (ecplbegf <  5) nrematbd = 3
///         else                    nrematbd = 4
///     } else {                               // standard coupling
///         if      (cplbegf == 0)  nrematbd = 2
///         else if (cplbegf <  3)  nrematbd = 3
///         else                    nrematbd = 4
///     }
/// } else if (spxinu) {
///     if (spxbegf < 2) nrematbd = 3 else nrematbd = 4
/// } else {
///     nrematbd = 4
/// }
/// ```
///
/// The standard-coupling arm is identical to [`remat_band_count`]. The
/// enhanced-coupling arm is new (an `ecplinu` 2/0 block uses `ecplbegf`,
/// not `cplbegf`, to size the rematrix-flag field — and uniquely admits
/// `nrematbd = 0`, suppressing the field entirely). `spx_in_use` /
/// `spx_begin_subbnd` come from the SPX strategy block; `spxbegf < 2` is
/// equivalent to `spx_begin_subbnd < 4`. `ecplbegf` is the raw 4-bit
/// `ecplbegf` code carried on [`crate::eac3::ecpl::EcplStrategy`].
pub(crate) fn remat_band_count_spx(
    cplinu: bool,
    cplbegf: u8,
    ecpl_in_use: bool,
    ecplbegf: u8,
    spx_in_use: bool,
    spx_begin_subbnd: usize,
) -> usize {
    if cplinu {
        if ecpl_in_use {
            // §E.3.3.2 enhanced-coupling arm — thresholds the raw ecplbegf.
            match ecplbegf {
                0 => 0,
                1 => 1,
                2 => 2,
                3 | 4 => 3,
                _ => 4,
            }
        } else {
            remat_band_count(true, cplbegf)
        }
    } else if spx_in_use {
        if spx_begin_subbnd < 4 {
            3
        } else {
            4
        }
    } else {
        4
    }
}

/// Decode a grouped exponent run (§7.1.3).
pub(crate) fn decode_exponents(
    br: &mut BitReader,
    absexp: i32,
    ngrps: usize,
    _expstr: usize,
    out: &mut [i32],
) -> Result<()> {
    // Spec §7.1.3 pseudo-code: unpack mapped values, convert to dexp
    // (subtract 2), then prefix-sum with the seeding absolute exponent.
    let mut prev = absexp;
    for grp in 0..ngrps {
        let gexp = br.read_u32(7)? as i32;
        let m1 = gexp / 25;
        let m2 = (gexp % 25) / 5;
        let m3 = (gexp % 25) % 5;
        let dexp0 = m1 - 2;
        let dexp1 = m2 - 2;
        let dexp2 = m3 - 2;
        let e0 = prev + dexp0;
        let e1 = e0 + dexp1;
        let e2 = e1 + dexp2;
        out[grp * 3] = e0;
        out[grp * 3 + 1] = e1;
        out[grp * 3 + 2] = e2;
        prev = e2;
    }
    // Post-chain, clamp each exponent to the valid 0..=24 range. Encoders
    // that overshoot (e.g. when encoding an exactly-zero channel after
    // rematrix) rely on this for sane decoder output.
    for v in out.iter_mut() {
        *v = (*v).clamp(0, 24);
    }
    Ok(())
}

/// Parametric bit allocation (§7.2.2) for a single channel range.
/// `start`..`end` is the mantissa-bin range.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_bit_allocation(
    state: &mut Ac3State,
    ch: usize,
    start: usize,
    end: usize,
    fscod: u8,
    fsnroffst: u8,
    fgaincod: u8,
    is_coupling: bool,
) {
    if end <= start {
        return;
    }
    // 1) Map exponents into PSD (§7.2.2.2).
    for bin in start..end {
        let e = state.channels[ch].exp[bin] as i32;
        state.channels[ch].psd[bin] = (3072 - (e << 7)) as i16;
    }
    // 2) PSD integration (§7.2.2.3).
    let bndstrt = MASKTAB[start] as usize;
    let bndend = MASKTAB[end - 1] as usize + 1;
    {
        let mut j = start;
        let mut k = bndstrt;
        loop {
            let lastbin = (BNDTAB[k] as usize + BNDSZ[k] as usize).min(end);
            state.channels[ch].bndpsd[k] = state.channels[ch].psd[j];
            j += 1;
            while j < lastbin {
                let a = state.channels[ch].bndpsd[k] as i32;
                let b = state.channels[ch].psd[j] as i32;
                state.channels[ch].bndpsd[k] = logadd(a, b) as i16;
                j += 1;
            }
            k += 1;
            if end <= lastbin {
                break;
            }
        }
    }

    // 3) Excitation / masking (§7.2.2.4-7.2.2.5).
    let sdecay = SLOWDEC[state.sdcycod as usize];
    let fdecay = FASTDEC[state.fdcycod as usize];
    let sgain = SLOWGAIN[state.sgaincod as usize];
    let dbknee = DBPBTAB[state.dbpbcod as usize];
    let floor = FLOORTAB[state.floorcod as usize];
    let fgain = FASTGAIN[fgaincod as usize];
    let snroffset = (((state.snroffst_coarse as i32 - 15) << 4) + fsnroffst as i32) << 2;

    let (fastleak_init, slowleak_init) = if is_coupling {
        ((state.cpl_fleak as i32) << 8, (state.cpl_sleak as i32) << 8)
    } else {
        (0i32, 0i32)
    };
    let mut fastleak = fastleak_init + 768;
    let mut slowleak = slowleak_init + 768;

    let mut excite = [0i32; 50];
    let mut lowcomp = 0i32;
    let mut begin;

    if is_coupling {
        begin = bndstrt;
        for bin in bndstrt..bndend {
            fastleak -= fdecay;
            fastleak = fastleak.max(state.channels[ch].bndpsd[bin] as i32 - fgain);
            slowleak -= sdecay;
            slowleak = slowleak.max(state.channels[ch].bndpsd[bin] as i32 - sgain);
            excite[bin] = fastleak.max(slowleak);
        }
        let _ = begin;
    } else if bndstrt == 0 {
        // fbw channel path (and LFE with same start=0)
        let lfe_last = end == 7;
        let bpsd = |i: usize| -> i32 { state.channels[ch].bndpsd[i.min(49)] as i32 };
        if 0 < bndend {
            lowcomp = calc_lowcomp(lowcomp, bpsd(0), bpsd(1), 0);
            excite[0] = bpsd(0) - fgain - lowcomp;
        }
        if 1 < bndend {
            lowcomp = calc_lowcomp(lowcomp, bpsd(1), bpsd(2), 1);
            excite[1] = bpsd(1) - fgain - lowcomp;
        }
        begin = 7.min(bndend);
        for bin in 2..7.min(bndend) {
            if !(lfe_last && bin == 6) {
                lowcomp = calc_lowcomp(lowcomp, bpsd(bin), bpsd(bin + 1), bin);
            }
            fastleak = bpsd(bin) - fgain;
            slowleak = bpsd(bin) - sgain;
            excite[bin] = fastleak - lowcomp;
            if !(lfe_last && bin == 6) && bpsd(bin) <= bpsd(bin + 1) {
                begin = bin + 1;
                break;
            }
        }
        for bin in begin..22.min(bndend) {
            if !(lfe_last && bin == 6) {
                lowcomp = calc_lowcomp(lowcomp, bpsd(bin), bpsd(bin + 1), bin);
            }
            fastleak -= fdecay;
            fastleak = fastleak.max(bpsd(bin) - fgain);
            slowleak -= sdecay;
            slowleak = slowleak.max(bpsd(bin) - sgain);
            excite[bin] = (fastleak - lowcomp).max(slowleak);
        }
        // 22..bndend path (with coupling-channel-style rule).
        if bndend > 22 {
            for bin in 22..bndend {
                fastleak -= fdecay;
                fastleak = fastleak.max(bpsd(bin) - fgain);
                slowleak -= sdecay;
                slowleak = slowleak.max(bpsd(bin) - sgain);
                excite[bin] = fastleak.max(slowleak);
            }
        }
    } else {
        // Shouldn't really hit this path for non-coupling in our data, but
        // cover it: behave like decoupled fbw starting at bndstrt.
        begin = bndstrt;
        for bin in begin..bndend {
            fastleak -= fdecay;
            fastleak = fastleak.max(state.channels[ch].bndpsd[bin] as i32 - fgain);
            slowleak -= sdecay;
            slowleak = slowleak.max(state.channels[ch].bndpsd[bin] as i32 - sgain);
            excite[bin] = fastleak.max(slowleak);
        }
    }

    // Compute masking curve (§7.2.2.5).
    let mut mask = [0i32; 50];
    let hth_row = &HTH[fscod as usize];
    for bin in bndstrt..bndend {
        let mut exc = excite[bin];
        if (state.channels[ch].bndpsd[bin] as i32) < dbknee {
            exc += (dbknee - state.channels[ch].bndpsd[bin] as i32) >> 2;
        }
        mask[bin] = exc.max(hth_row[bin] as i32);
    }

    // Apply delta bit allocation (§7.2.2.6). Per-band ±6 dB mask offsets
    // signalled by the encoder. Critical for transient blocks: the §7.2.2.6
    // mechanism BOOSTs the masking floor (less bits assigned) in low-energy
    // bands during a burst frame, freeing bit budget for the burst peak.
    // Without applying these offsets, our decoder ends up with a different
    // mask shape than the encoder used — and therefore a different bap[]
    // assignment, which causes our mantissa unpacking to read wrong-width
    // codes from the bitstream. The downstream symptom is wildly wrong
    // coefficients in burst frames (PSNR ≈ 5–15 dB). The dba code below
    // is the literal §7.2.2.6 pseudocode; the per-block deltba state is
    // maintained by the parser per Table 5.16 semantics.
    // LFE has no delta bit allocation per spec syntax (Table 5.3 lists
    // only cpldeltbae + per-fbw deltbae[ch]). Skip when ch is the LFE
    // pseudo-channel (index MAX_FBW+1 in our channel array).
    let is_lfe = !is_coupling && ch > MAX_FBW;
    let dba_idx = if is_coupling { MAX_FBW } else { ch };
    if !is_lfe && state.deltnseg[dba_idx] > 0 {
        let mut band = 0usize;
        for seg in 0..state.deltnseg[dba_idx] {
            band += state.deltoffst[dba_idx][seg] as usize;
            let dba_raw = state.deltba[dba_idx][seg] as i32;
            let delta = if dba_raw >= 4 {
                (dba_raw - 3) << 7
            } else {
                (dba_raw - 4) << 7
            };
            let len = state.deltlen[dba_idx][seg] as usize;
            for _ in 0..len {
                if band < 50 {
                    mask[band] += delta;
                }
                band += 1;
            }
        }
    }

    // Persist masking curve onto the channel state for diagnostics.
    for bin in bndstrt..bndend {
        state.channels[ch].mask[bin] = mask[bin] as i16;
    }

    // 4) Compute bit allocation pointers (§7.2.2.7).
    {
        let mut i = start;
        let mut j = MASKTAB[start] as usize;
        loop {
            let lastbin = (BNDTAB[j] as usize + BNDSZ[j] as usize).min(end);
            let mut m = mask[j];
            m -= snroffset;
            m -= floor;
            if m < 0 {
                m = 0;
            }
            m &= 0x1fe0;
            m += floor;
            while i < lastbin {
                let addr = ((state.channels[ch].psd[i] as i32 - m) >> 5).clamp(0, 63) as usize;
                state.channels[ch].bap[i] = BAPTAB[addr];
                i += 1;
            }
            if i >= end {
                break;
            }
            j += 1;
        }
    }

    // ---- Diagnostic trace (gated by `AC3_TRACE_FRAME=N` and `AC3_TRACE_BLK=B`) ----
    // Dumps bndpsd / excite / mask / bap for the requested frame+block. Used
    // to compare against the validator binary's decode of the same fixture.
    // Cheap when the env vars aren't set.
    let trace_frame = std::env::var("AC3_TRACE_FRAME")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let trace_blk = std::env::var("AC3_TRACE_BLK")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    if let Some(tf) = trace_frame {
        if tf == state.frame_counter && state.blkidx == trace_blk {
            let label = if is_coupling {
                "cpl".to_string()
            } else if ch == MAX_FBW + 1 {
                "lfe".to_string()
            } else {
                format!("ch{ch}")
            };
            eprintln!(
                "TRACE frame={} blk={} {} bndstrt={} bndend={} start={} end={} fgain={:#x} sgain={:#x} fdecay={:#x} sdecay={:#x} dbknee={:#x} floor={:#x} snroffset={:#x}",
                state.frame_counter,
                state.blkidx,
                label,
                bndstrt,
                bndend,
                start,
                end,
                fgain,
                sgain,
                fdecay,
                sdecay,
                dbknee,
                floor,
                snroffset
            );
            eprintln!(
                "TRACE  exp[0..bndend]: {:?}",
                &state.channels[ch].exp[start..start + bndend.min(20)]
            );
            eprintln!(
                "TRACE  psd[0..bndend]: {:?}",
                &state.channels[ch].psd[start..start + bndend.min(20)]
            );
            eprintln!(
                "TRACE  bndpsd[bndstrt..bndend]: {:?}",
                &state.channels[ch].bndpsd[bndstrt..bndend]
            );
            eprintln!(
                "TRACE  excite[bndstrt..bndend]: {:?}",
                &excite[bndstrt..bndend]
            );
            eprintln!("TRACE  mask[bndstrt..bndend]: {:?}", &mask[bndstrt..bndend]);
            eprintln!(
                "TRACE  bap[start..end]: {:?}",
                &state.channels[ch].bap[start..end.min(start + 30)]
            );
            // Re-derive the lowcomp progression for the first 7 bins so we
            // can audit calc_lowcomp by hand against the spec table.
            if bndstrt == 0 && !is_coupling {
                let mut lc = 0i32;
                let bp = |i: usize| state.channels[ch].bndpsd[i.min(49)] as i32;
                eprint!("TRACE  lowcomp progression: ");
                for bin in 0..7.min(bndend) {
                    if bin + 1 < 50 {
                        lc = calc_lowcomp(lc, bp(bin), bp(bin + 1), bin);
                    }
                    eprint!("[bin={} lc={}] ", bin, lc);
                }
                eprintln!();
            }
        }
    }
}

/// Log-addition (§7.2.2.3 logadd).
fn logadd(a: i32, b: i32) -> i32 {
    let c = a - b;
    let addr = ((c.abs() >> 1) as usize).min(255);
    if c >= 0 {
        a + LATAB[addr] as i32
    } else {
        b + LATAB[addr] as i32
    }
}

/// calc_lowcomp (§7.2.2.4).
fn calc_lowcomp(a: i32, b0: i32, b1: i32, bin: usize) -> i32 {
    let mut a = a;
    if bin < 7 {
        if b0 + 256 == b1 {
            a = 384;
        } else if b0 > b1 {
            a = (a - 64).max(0);
        }
    } else if bin < 20 {
        if b0 + 256 == b1 {
            a = 320;
        } else if b0 > b1 {
            a = (a - 64).max(0);
        }
    } else {
        a = (a - 128).max(0);
    }
    a
}

/// 16-bit Galois LFSR used to generate dither for `bap=0` mantissas
/// (§7.3.4). The spec says "any reasonably random sequence may be
/// used" — we use the classic x^16 + x^14 + x^13 + x^11 + 1 polynomial
/// because it has a maximal 65535-sample period and the output looks
/// like white noise to within a few bits. Seed is arbitrary but
/// fixed so decodes are deterministic.
pub(crate) fn dither_lfsr(state: &mut u32) -> f32 {
    // Advance the 16-bit LFSR one step and return a uniform value in
    // the range `[-0.707, 0.707)` — the spec's "optimum" scaling
    // (0.707 ≈ 1/√2). Uses the classic Fibonacci taps at bits
    // 15, 13, 12, 10 of a 16-bit state.
    let bit = ((*state >> 15) ^ (*state >> 13) ^ (*state >> 12) ^ (*state >> 10)) & 1;
    *state = ((*state << 1) | bit) & 0xFFFF;
    // Center around zero: bit15 of the 16-bit state becomes the sign,
    // lower 15 bits provide magnitude.
    let signed = (*state as i32).wrapping_sub(0x8000) as f32 / 32768.0;
    signed * 0.707
}

/// Unpack + dequantize mantissas for all channels (§7.3).
/// Populates ChannelState.coeffs with dequantized transform coefficients.
pub(crate) fn unpack_mantissas(state: &mut Ac3State, bsi: &Bsi, br: &mut BitReader) -> Result<()> {
    let nfchans = bsi.nfchans as usize;
    // Zero any leftover coefficient slots so stale data from prior blocks
    // can never bleed into the IMDCT input. Unpacked mantissas overwrite
    // bins 0..end_mant, decoupling overwrites bins in the coupling range,
    // but all other bins (e.g. end_mant..N_COEFFS on an uncoupled or
    // narrow-band channel) must read as exactly zero.
    for ch in 0..MAX_CHANNELS {
        for v in state.channels[ch].coeffs.iter_mut() {
            *v = 0.0;
        }
    }
    let mut got_cplchan = false;
    // Grouped-mantissa buffers per bap: values 1,2,4 have triples/pairs
    // shared across channels in frequency order. The spec says groups
    // are *shared across exponent sets*, meaning once a group is started
    // for bap=1 (5-bit triple), subsequent mantissas of bap=1 consume
    // from that group, even if a different channel emits them. We
    // implement this per-bap buffer state.
    let mut grp1: [f32; 3] = [0.0; 3];
    let mut grp1_n = 0usize; // remaining in buffer
    let mut grp2: [f32; 3] = [0.0; 3];
    let mut grp2_n = 0usize;
    let mut grp4: [f32; 2] = [0.0; 2];
    let mut grp4_n = 0usize;

    // Optional per-block mantissa trace gated by `AC3_TRACE_FRAME=N` and
    // `AC3_TRACE_BLK=B` plus `AC3_TRACE_MANT=1`. Used by maintainers when
    // chasing bit-stream alignment issues; off by default to keep the
    // hot path clean.
    let trace_mant_frame = std::env::var("AC3_TRACE_FRAME")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let trace_mant_blk = std::env::var("AC3_TRACE_BLK")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let trace_mant_on = std::env::var("AC3_TRACE_MANT").is_ok();
    for ch in 0..nfchans {
        let end = state.channels[ch].end_mant;
        let dith = state.channels[ch].dithflag;
        let trace_this = trace_mant_on
            && trace_mant_frame == Some(state.frame_counter)
            && state.blkidx == trace_mant_blk;
        if trace_this {
            eprintln!(
                "TRACE-MANT ch{} mantissa decode (end={}, dith={}):",
                ch, end, dith
            );
        }
        for bin in 0..end {
            let bap = state.channels[ch].bap[bin];
            let bit_pos_before = if trace_this { br.bit_position() } else { 0 };
            let val = fetch_mantissa(
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
            // Dither for bap=0 mantissas (§7.3.4): when dithflag is
            // set, replace the zero-level mantissa with an LFSR-driven
            // pseudo-random value scaled by 0.707 before the standard
            // `>> exponent` coefficient reconstruction. This fills
            // inaudible masked bands with near-noise instead of
            // silence, preventing coloration of subsequent DSP stages
            // (especially rematrix and the IMDCT post-chain).
            let final_val = if bap == 0 && dith {
                dither_lfsr(&mut state.dither_lfsr_state)
            } else {
                val
            };
            let e = state.channels[ch].exp[bin] as i32;
            state.channels[ch].coeffs[bin] = final_val * 2f32.powi(-e);
            if trace_this && bin < 32 {
                eprintln!(
                    "TRACE-MANT ch{} bin={:3} bap={:2} exp={:2} bit_pos_before={} mant={:.5} dither_used={} coeff={:.5e}",
                    ch,
                    bin,
                    bap,
                    e,
                    bit_pos_before,
                    val,
                    bap == 0 && dith,
                    state.channels[ch].coeffs[bin]
                );
            }
        }
        if state.cpl_in_use && state.channels[ch].in_coupling && !got_cplchan {
            let start = state.cpl_begf_mant;
            let end_c = state.cpl_endf_mant;
            let cplc = MAX_FBW;
            for bin in start..end_c {
                let bap = state.channels[cplc].bap[bin];
                // Coupling-channel mantissas are never dithered — the
                // spec explicitly says dither is applied after a channel
                // is extracted from the coupling channel (§7.3.4 para 1).
                let val = fetch_mantissa(
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
                let e = state.channels[cplc].exp[bin] as i32;
                state.channels[cplc].coeffs[bin] = val * 2f32.powi(-e);
            }
            got_cplchan = true;
        }
    }
    if bsi.lfeon {
        let lfe_ch = MAX_FBW + 1;
        let dith = state.channels[lfe_ch].dithflag;
        for bin in 0..7 {
            let bap = state.channels[lfe_ch].bap[bin];
            let val = fetch_mantissa(
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
                dither_lfsr(&mut state.dither_lfsr_state)
            } else {
                val
            };
            let e = state.channels[lfe_ch].exp[bin] as i32;
            state.channels[lfe_ch].coeffs[bin] = final_val * 2f32.powi(-e);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn fetch_mantissa(
    br: &mut BitReader,
    bap: u8,
    g1: &mut [f32; 3],
    g1n: &mut usize,
    g2: &mut [f32; 3],
    g2n: &mut usize,
    g4: &mut [f32; 2],
    g4n: &mut usize,
    dithflag: bool,
) -> Result<f32> {
    let _ = dithflag;
    match bap {
        0 => {
            // Dither replacement for bap=0 is handled by the caller
            // (unpack_mantissas) after we return. Here we just signal
            // "no bits consumed" by returning 0.
            Ok(0.0)
        }
        1 => {
            if *g1n == 0 {
                let code = br.read_u32(5)? as i32;
                let m1 = code / 9;
                let m2 = (code % 9) / 3;
                let m3 = code % 3;
                g1[0] = MANT_LEVEL_3[m1.clamp(0, 2) as usize];
                g1[1] = MANT_LEVEL_3[m2.clamp(0, 2) as usize];
                g1[2] = MANT_LEVEL_3[m3.clamp(0, 2) as usize];
                *g1n = 3;
            }
            let v = g1[3 - *g1n];
            *g1n -= 1;
            Ok(v)
        }
        2 => {
            if *g2n == 0 {
                let code = br.read_u32(7)? as i32;
                let m1 = code / 25;
                let m2 = (code % 25) / 5;
                let m3 = code % 5;
                g2[0] = MANT_LEVEL_5[m1.clamp(0, 4) as usize];
                g2[1] = MANT_LEVEL_5[m2.clamp(0, 4) as usize];
                g2[2] = MANT_LEVEL_5[m3.clamp(0, 4) as usize];
                *g2n = 3;
            }
            let v = g2[3 - *g2n];
            *g2n -= 1;
            Ok(v)
        }
        3 => {
            let code = br.read_u32(3)? as usize;
            Ok(MANT_LEVEL_7[code.min(6)])
        }
        4 => {
            if *g4n == 0 {
                let code = br.read_u32(7)? as i32;
                let m1 = code / 11;
                let m2 = code % 11;
                g4[0] = MANT_LEVEL_11[m1.clamp(0, 10) as usize];
                g4[1] = MANT_LEVEL_11[m2.clamp(0, 10) as usize];
                *g4n = 2;
            }
            let v = g4[2 - *g4n];
            *g4n -= 1;
            Ok(v)
        }
        5 => {
            let code = br.read_u32(4)? as usize;
            Ok(MANT_LEVEL_15[code.min(14)])
        }
        b if (6..=15).contains(&b) => {
            let nbits = QUANTIZATION_BITS[b as usize] as u32;
            let raw = br.read_u32(nbits)? as i32;
            // Sign-extend the top bit as a signed two's-complement fraction.
            let shift = 32 - nbits;
            let signed = (raw << shift) >> shift;
            // Normalize to (-1, 1): divide by 2^(nbits-1).
            let scale = 2f32.powi(-(nbits as i32 - 1));
            Ok(signed as f32 * scale)
        }
        _ => Ok(0.0),
    }
}

/// Lowest transform-coefficient number of SPX sub-band `subbnd` per
/// Table E3.13. Sub-bands 0..=16 carry 12 coefficients each starting at
/// tc# 25; the entry for sub-band 17 (tc# 229) is the one-past-last
/// marker used when `spxendf == 7`.
#[inline]
fn spx_bandtable(subbnd: usize) -> usize {
    25 + 12 * subbnd
}

/// Table E3.14 — Spectral Extension Attenuation Table `spxattentab[][]`
/// (§3.6.4.2.3). Indexed by the 5-bit `spxattencod[ch]` codeword
/// (rows 0..=31), each row holds the first 3 attenuation values of a
/// 5-tap symmetric notch filter applied at the baseband / extension
/// border (and at every wrap point during the §3.6.4.1 translation
/// copy). The 5-tap kernel is `[T[0], T[1], T[2], T[1], T[0]]` —
/// the last two taps are derived by symmetry per spec text:
///
/// > "The first 3 attenuation values of the filter are determined by
/// > lookup into Table E3.14 with index `spxattencod[ch]`. The last
/// > two attenuation values of the filter are determined by symmetry
/// > and are not explicitly stored in the table."
#[allow(clippy::excessive_precision)] // spec text values; f32 round suffices
pub(crate) const SPX_ATTEN_TABLE: [[f32; 3]; 32] = [
    [0.954_841_604, 0.911_722_489, 0.870_550_563],
    [0.911_722_489, 0.831_237_896, 0.757_858_283],
    [0.870_550_563, 0.757_858_283, 0.659_753_955],
    [0.831_237_896, 0.690_956_440, 0.574_349_177],
    [0.793_700_526, 0.629_960_525, 0.500_000_000],
    [0.757_858_283, 0.574_349_177, 0.435_275_282],
    [0.723_634_619, 0.523_647_061, 0.378_929_142],
    [0.690_956_440, 0.477_420_802, 0.329_876_978],
    [0.659_753_955, 0.435_275_282, 0.287_174_589],
    [0.629_960_525, 0.396_850_263, 0.250_000_000],
    [0.601_512_518, 0.361_817_309, 0.217_637_641],
    [0.574_349_177, 0.329_876_978, 0.189_464_571],
    [0.548_412_490, 0.300_756_259, 0.164_938_489],
    [0.523_647_061, 0.274_206_245, 0.143_587_294],
    [0.500_000_000, 0.250_000_000, 0.125_000_000],
    [0.477_420_802, 0.227_930_622, 0.108_818_820],
    [0.455_861_244, 0.207_809_474, 0.094_732_285],
    [0.435_275_282, 0.189_464_571, 0.082_469_244],
    [0.415_618_948, 0.172_739_110, 0.071_793_647],
    [0.396_850_263, 0.157_490_131, 0.062_500_000],
    [0.378_929_142, 0.143_587_294, 0.054_409_410],
    [0.361_817_309, 0.130_911_765, 0.047_366_143],
    [0.345_478_220, 0.119_355_200, 0.041_234_622],
    [0.329_876_978, 0.108_818_820, 0.035_896_824],
    [0.314_980_262, 0.099_212_566, 0.031_250_000],
    [0.300_756_259, 0.090_454_327, 0.027_204_705],
    [0.287_174_589, 0.082_469_244, 0.023_683_071],
    [0.274_206_245, 0.075_189_065, 0.020_617_311],
    [0.261_823_531, 0.068_551_561, 0.017_948_412],
    [0.250_000_000, 0.062_500_000, 0.015_625_000],
    [0.238_710_401, 0.056_982_656, 0.013_602_353],
    [0.227_930_622, 0.051_952_369, 0.011_841_536],
];

/// Apply the §3.6.4.2.3 5-tap symmetric notch filter to a 5-bin window
/// centred on the band-border bin. The kernel taps are
/// `[T[0], T[1], T[2], T[1], T[0]]` where `T = SPX_ATTEN_TABLE[code]`.
/// The window starts at `filtbin` (which the caller positions at
/// `border_bin - 2`); bins outside `0..N_COEFFS` are skipped so this is
/// safe near the array tail.
#[inline]
fn apply_spx_atten_notch(coeffs: &mut [f32; N_COEFFS], filtbin: usize, code: u8) {
    let row = SPX_ATTEN_TABLE[(code & 0x1F) as usize];
    let taps = [row[0], row[1], row[2], row[1], row[0]];
    for (i, tap) in taps.iter().enumerate() {
        let idx = filtbin + i;
        if idx < N_COEFFS {
            coeffs[idx] *= *tap;
        }
    }
}

/// One step of the SPX pseudo-random noise generator (§E.3.6.4.2). The
/// spec only requires a "zero-mean, unity-variance" sequence and leaves
/// the exact generator non-normative — AC-3 / E-AC-3 are lossy and the
/// corpus is PSNR-compared, so a deterministic LFSR-derived value is
/// adequate. Returns a value in roughly `[-1, 1)` scaled to ~unit
/// variance (a sign-balanced uniform on (-√3, √3) has unit variance).
#[inline]
fn spx_noise(lfsr: &mut u32) -> f32 {
    // 32-bit xorshift — long period, cheap, deterministic.
    let mut x = *lfsr;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *lfsr = x;
    // Map to (-1, 1) then scale to unit variance: a uniform on (-1, 1)
    // has variance 1/3, so multiply by √3 ≈ 1.7320508.
    let u = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;
    u * 1.732_050_8
}

/// §E.3.6.4.1 transform-coefficient translation plan, shared between
/// the decoder ([`apply_spectral_extension`]) and the encoder
/// (`eac3::spxenc` — which must predict exactly which LF bin the
/// decoder will copy into each SPX bin to compute the §3.6.4.3
/// energy-matching coordinates).
///
/// Returns:
/// * `map` — for each SPX-region bin `i` (absolute tc# =
///   `spx_begin_tc + i`), the LF source tc# the translation copies
///   from. The copy cursor starts at `copystart` and wraps back to it
///   per the spec's dual wrap checks: the pre-band
///   `copyindex + bandsize > copyend` test AND the per-bin
///   `copyindex == copyend` test.
/// * `wrapflag[bnd]` — true when band `bnd >= 1` wrapped (its start is
///   a copy boundary), driving the §3.6.4.2.3 border-notch re-filter
///   sites. Band 0's border site (the baseband / extension border
///   itself) is unconditional and not flagged here.
pub(crate) fn spx_translation_plan(
    copystart: usize,
    copyend: usize,
    nbnds: usize,
    bndsztab: &[usize; 18],
) -> (Vec<usize>, [bool; 18]) {
    let total: usize = bndsztab[..nbnds].iter().sum();
    let mut map = Vec::with_capacity(total);
    let mut wrapflag = [false; 18];
    let mut copyindex = copystart;
    for bnd in 0..nbnds {
        let bandsize = bndsztab[bnd];
        let mut wrapped = false;
        if copyindex + bandsize > copyend {
            copyindex = copystart;
            wrapped = true;
        }
        for _ in 0..bandsize {
            if copyindex == copyend {
                copyindex = copystart;
                wrapped = true;
            }
            map.push(copyindex);
            copyindex += 1;
        }
        if bnd > 0 && wrapped {
            wrapflag[bnd] = true;
        }
    }
    (map, wrapflag)
}

/// E-AC-3 spectral extension high-frequency regeneration (§E.3.6).
///
/// For each fbw channel with `in_spx == true` this synthesizes the SPX
/// region `[spx_begin .. spx_end)` (transform-coefficient indices) from
/// the channel's own low-frequency coefficients:
///
/// 1. **Translation** (§E.3.6.4.1) — copy LF coefficients from the copy
///    region `[copystart .. copyend)` (`copystart = spxbandtable[spxstrtf]`,
///    `copyend = spxbandtable[spx_begin]`) into the SPX region,
///    wrapping the copy cursor when it reaches `copyend`.
/// 2. **Banded RMS energy** (§E.3.6.4.2.2) of the translated bins.
/// 3. **Noise blending** (§E.3.6.4.2.4) — `tc = tc·sblend + noise·rms·nblend`
///    per band, using the precomputed `spx_nblend` / `spx_sblend`.
/// 4. **Coordinate scaling** (§E.3.6.4.3) — `tc *= spxco·32` per band.
///
/// Band sizing (`spx_nbnds` / `spx_bndsztab`), the per-band coordinates
/// (`spx_coord`) and blend factors are computed during the audblk parse
/// (see `eac3::dsp`); this routine consumes that prepared state.
fn apply_spectral_extension(state: &mut Ac3State, nfchans: usize) {
    if !state.spx_in_use {
        return;
    }
    let nbnds = state.spx_nbnds;
    if nbnds == 0 {
        return;
    }
    let copystart = spx_bandtable(state.spx_strtf as usize);
    let copyend = spx_bandtable(state.spx_begin_subbnd);
    let spx_begin_tc = copyend;
    let spx_end_tc = spx_bandtable(state.spx_end_subbnd);
    if copyend <= copystart || spx_end_tc <= spx_begin_tc {
        return;
    }

    // 1. Transform coefficient translation plan (§E.3.6.4.1) —
    //    channel-independent, so compute it once. `wrapflag[bnd]` is
    //    true when the band-relative copy cursor had to wrap back to
    //    `copystart` before consuming this band's `bandsize` samples —
    //    i.e. the band straddles a copy boundary. The spec applies the
    //    §3.6.4.2.3 border notch filter at every such wrap point AND at
    //    the baseband / extension border itself (the bin straddling
    //    `spx_begin_tc`). The plan is shared with the encoder
    //    (`eac3::spxenc`) so both sides agree bin-for-bin.
    let (copy_map, wrapflag) = spx_translation_plan(copystart, copyend, nbnds, &state.spx_bndsztab);

    for ch in 0..nfchans {
        if !state.channels[ch].in_spx {
            continue;
        }

        // Execute the translation copy.
        for (i, &src) in copy_map.iter().enumerate() {
            let insertindex = spx_begin_tc + i;
            if insertindex < N_COEFFS && src < N_COEFFS {
                state.channels[ch].coeffs[insertindex] = state.channels[ch].coeffs[src];
            }
        }

        // 1b. §3.6.4.2.3 Transform Coefficient Band Border Filtering —
        //     after the §3.6.4.1 translation copy AND BEFORE the
        //     §3.6.4.2.2 banded RMS / §3.6.4.2.4 noise scaling. The
        //     5-tap symmetric notch filter sits centred on the first
        //     extension bin, attenuating the 2 bins below and 2 bins
        //     above the border (filter starts at `spx_begin_tc - 2`).
        //     The same filter re-applies at each band-internal wrap
        //     point flagged above (filter starts at the band's start
        //     minus 2 bins).
        if state.channels[ch].spx_atten_active {
            let code = state.channels[ch].spx_atten_code;
            // Baseband / extension region border.
            let border = spx_begin_tc;
            if border >= 2 {
                apply_spx_atten_notch(&mut state.channels[ch].coeffs, border - 2, code);
            }
            // Wrap points at band starts (bnd >= 1).
            let mut band_start = spx_begin_tc;
            for bnd in 0..nbnds {
                if bnd > 0 && wrapflag[bnd] && band_start >= 2 {
                    apply_spx_atten_notch(&mut state.channels[ch].coeffs, band_start - 2, code);
                }
                band_start += state.spx_bndsztab[bnd];
            }
        }

        // 2. Banded RMS energy of the translated coefficients
        //    (§E.3.6.4.2.2), 3. noise blend (§E.3.6.4.2.4), and
        //    4. coordinate scaling (§E.3.6.4.3) — fused per band.
        let mut spxmant = spx_begin_tc;
        for bnd in 0..nbnds {
            let bandsize = state.spx_bndsztab[bnd];
            let band_lo = spxmant;
            let band_hi = (spxmant + bandsize).min(N_COEFFS);

            // Banded RMS.
            let mut accum = 0.0f64;
            for bin in band_lo..band_hi {
                let v = state.channels[ch].coeffs[bin] as f64;
                accum += v * v;
            }
            let rms = if bandsize > 0 {
                (accum / bandsize as f64).sqrt() as f32
            } else {
                0.0
            };

            let nblend = state.channels[ch].spx_nblend[bnd];
            let sblend = state.channels[ch].spx_sblend[bnd];
            let nscale = rms * nblend;
            let coord = state.channels[ch].spx_coord[bnd];

            for bin in band_lo..band_hi {
                let tctemp = state.channels[ch].coeffs[bin];
                let ntemp = spx_noise(&mut state.spx_noise_lfsr);
                let blended = tctemp * sblend + ntemp * nscale;
                // §E.3.6.4.3 final scale by spxco·32.
                state.channels[ch].coeffs[bin] = blended * coord * 32.0;
            }

            spxmant += bandsize;
        }

        // The SPX region is now populated; extend the channel's mantissa
        // count so dynrng + IMDCT process the regenerated bins.
        state.channels[ch].end_mant = state.channels[ch].end_mant.max(spx_end_tc);
    }
}

/// Apply DSP stages to the current block: decouple, rematrix, dynrng,
/// IMDCT, window + overlap-add. Populates `channels[ch].coeffs[0..256]`
/// with time-domain PCM samples ready for emission.
pub(crate) fn dsp_block(state: &mut Ac3State, _si: &SyncInfo, bsi: &Bsi) {
    let nfchans = bsi.nfchans as usize;
    let acmod = bsi.acmod;

    // --- Decoupling (§7.4) ---
    // Enhanced coupling (§E.3.5.5) reconstructs each coupled channel's
    // transform coefficients from the complex carrier `Z[k]` (the
    // §E.3.5.5.4 product) *before* `dsp_block` is called, so `coeffs[bin]`
    // already holds the per-channel result. The standard scalar decouple
    // (`cpl_coord · cplchan`) must NOT run in that case — it would
    // overwrite the carrier-derived coefficients. `skip_decouple` carries
    // that gate; base AC-3 and standard E-AC-3 coupling leave it `false`.
    if state.cpl_in_use && !state.skip_decouple {
        let cpl_ch = MAX_FBW;
        let start = state.cpl_begf_mant;
        let end = state.cpl_endf_mant;
        // Build subband->band lookup. The §7.4.2 coupling region spans at
        // most 18 sub-bands (bins 37..253, 12 bins each), so `cpl_bndstrc`
        // / `cpl_coord` / `cpl_phsflg` / `sbnd2bnd` are all sized 18. A
        // malformed frame can decode `cpl_nsubbnd > 18` (e.g. an
        // enhanced-coupling span of up to 22 sub-bands threaded into the
        // standard decouple path); clamp the walk so it degrades to the
        // valid prefix instead of indexing out of bounds.
        let nsub = state.cpl_nsubbnd.min(18);
        let mut sbnd2bnd = [0usize; 18];
        let mut bnd = 0usize;
        for sbnd in 0..nsub {
            if sbnd > 0 && !state.cpl_bndstrc[sbnd] {
                bnd += 1;
            }
            sbnd2bnd[sbnd] = bnd;
        }
        for ch in 0..nfchans {
            if !state.channels[ch].in_coupling {
                continue;
            }
            for sbnd_off in 0..nsub {
                let band = sbnd2bnd[sbnd_off];
                let coord = state.cpl_coord[ch][band] * 8.0;
                let base = start + sbnd_off * 12;
                let limit = (base + 12).min(end);
                for bin in base..limit {
                    let mut v = state.channels[cpl_ch].coeffs[bin] * coord;
                    // phase flag for right channel in 2/0.
                    if acmod == 0x2 && ch == 1 && state.cpl_phsflg[band] {
                        v = -v;
                    }
                    state.channels[ch].coeffs[bin] = v;
                }
            }
        }
    }

    // --- Rematrixing (§7.5) ---
    //
    // Per Tables 7.25 / 7.26 / 7.27 / 7.28, the upper edge of the LAST
    // rematrix band is NOT a fixed constant — it tracks the lower edge
    // of the coupling region whenever coupling is in use:
    //
    //   • cplinu == 0           : 4 bands, last ends at bin 252 (Table 7.25)
    //   • cplinu == 1, cplbegf > 2: 4 bands, last ends at A = 36 + 12*cplbegf
    //   • cplinu == 1, 2 ≥ cplbegf > 0: 3 bands, last ends at A
    //   • cplinu == 1, cplbegf = 0: 2 bands, last ends at bin 36
    //
    // A previous formulation hard-coded the last band's high coefficient at
    // bin 252 even when coupling was active. On 2/0 frames whose bitstream
    // enables rematrixing AND coupling above bin 132 (cplbegf=8 in our
    // transient fixture), this bled the L+R / L-R operation into the
    // coupling region — bins that had just been re-derived from the
    // coupling pseudo-channel via cplco coords. The downstream symptom
    // was a steady PSNR drift across the 6 audblks of every burst-onset
    // frame: rematrix scrambled the post-decouple coefficients, the
    // IMDCT rendered the wrong waveform, and overlap-add carried the
    // error into the next block. Fixing the upper edge to the spec's
    // cplbegf-dependent boundary restores burst-frame PSNR.
    if acmod == 0x2 {
        let last_high = if state.cpl_in_use {
            // A = 36 + 12 * cplbegf per Tables 7.26 / 7.27.
            // For cplbegf = 0 (Table 7.28), the last band actually ends
            // at bin 36 — but with only 2 rematrix bands it never reaches
            // band index 3, so the value of `last_high` for index 3 is
            // unused. Compute A unconditionally.
            36 + 12 * state.cpl_begf as usize
        } else {
            252 // Table 7.25 fixed last band high.
        };
        // Convert to exclusive upper bound for our `lo..hi` ranges.
        let last_hi_excl = last_high + 1;
        let remat_bands: [(usize, usize); 4] =
            [(13, 25), (25, 37), (37, 61), (61, last_hi_excl.max(61))];
        let n = remat_band_count(state.cpl_in_use, state.cpl_begf);
        for (i, (lo, hi)) in remat_bands.iter().take(n).enumerate() {
            if !state.rematflg[i] {
                continue;
            }
            let end_lo = state.channels[0].end_mant.min(*hi);
            let end_hi = state.channels[1].end_mant.min(*hi);
            let end = end_lo.min(end_hi);
            for bin in *lo..end {
                let l = state.channels[0].coeffs[bin];
                let r = state.channels[1].coeffs[bin];
                state.channels[0].coeffs[bin] = l + r;
                state.channels[1].coeffs[bin] = l - r;
            }
        }
    }

    // --- Spectral extension synthesis (§E.3.6) ---
    //
    // For channels using SPX (`in_spx`, E-AC-3 only) the coded
    // coefficients stop at the SPX begin frequency; this step
    // regenerates the high-frequency band [spx_begin .. spx_end) by
    // copying low-frequency coefficients, blending with banded noise,
    // and scaling by the per-band SPX coordinates. It runs AFTER
    // decouple + rematrix (which reshape the low-frequency copy region)
    // and BEFORE dynrng + IMDCT so the synthesized bins are gain-scaled
    // and transformed together with the baseband. Base AC-3 never sets
    // `in_spx`, so this is a no-op there.
    apply_spectral_extension(state, nfchans);

    // --- Dynrng scaling ---
    for ch in 0..nfchans {
        let g = state.channels[ch].dynrng;
        if (g - 1.0).abs() > 1e-6 {
            let end = state.channels[ch].end_mant;
            for bin in 0..end {
                state.channels[ch].coeffs[bin] *= g;
            }
        }
    }

    // --- IMDCT + window + overlap-add for every output channel ---
    // Uses the §7.9.4 FFT-backed decomposition: pre-twiddle → N/4-point
    // complex IFFT (N/8 for short blocks) → post-twiddle → de-interleave.
    // Matches the direct-form reference within f32 precision on the long
    // path; the short path is validated by the validator-fixture RMS gate.
    let trace_frame_dsp = std::env::var("AC3_TRACE_FRAME")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let trace_blk_dsp = std::env::var("AC3_TRACE_BLK")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    for ch in 0..nfchans {
        let mut coeffs = [0.0f32; 256];
        coeffs.copy_from_slice(&state.channels[ch].coeffs);
        if let Some(tf) = trace_frame_dsp {
            if tf == state.frame_counter && state.blkidx == trace_blk_dsp {
                let max_abs = coeffs.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
                let nonzero = coeffs.iter().filter(|&&v| v != 0.0).count();
                eprintln!(
                    "TRACE-DSP ch{} pre-IMDCT max|coeff|={:.6e} nonzero_bins={} blksw={} dynrng={}",
                    ch, max_abs, nonzero, state.channels[ch].blksw, state.channels[ch].dynrng
                );
                eprint!("TRACE-DSP ch{} coeff[0..16]: ", ch);
                for v in &coeffs[..16] {
                    eprint!("{:.4e} ", v);
                }
                eprintln!();
                eprint!("TRACE-DSP ch{} coeff[16..32]: ", ch);
                for v in &coeffs[16..32] {
                    eprint!("{:.4e} ", v);
                }
                eprintln!();
            }
        }
        let mut time = [0.0f32; 512];
        if state.channels[ch].blksw {
            crate::imdct::imdct_256_pair_fft(&coeffs, &mut time);
        } else {
            crate::imdct::imdct_512_fft(&coeffs, &mut time);
        }
        // Apply window.
        for n in 0..256 {
            time[n] *= WINDOW[n];
            time[511 - n] *= WINDOW[n];
        }
        // Overlap-add: pcm[n] = time[n] + delay[n]; delay[n] = time[256+n]
        let mut out_pcm = [0.0f32; 256];
        for n in 0..256 {
            // Per §7.9.4.1 overlap-add: pcm[n] = 2 * (x[n] + delay[n]).
            out_pcm[n] = 2.0 * (time[n] + state.channels[ch].delay[n]);
            state.channels[ch].delay[n] = time[256 + n];
        }
        state.channels[ch].coeffs[..256].copy_from_slice(&out_pcm);
    }
    if bsi.lfeon {
        let ch = MAX_FBW + 1;
        let mut coeffs = [0.0f32; 256];
        coeffs.copy_from_slice(&state.channels[ch].coeffs);
        let mut time = [0.0f32; 512];
        // LFE is always long-block (spec §5.4.3.3).
        crate::imdct::imdct_512_fft(&coeffs, &mut time);
        for n in 0..256 {
            time[n] *= WINDOW[n];
            time[511 - n] *= WINDOW[n];
        }
        let mut out_pcm = [0.0f32; 256];
        for n in 0..256 {
            // Per §7.9.4.1 overlap-add: pcm[n] = 2 * (x[n] + delay[n]).
            out_pcm[n] = 2.0 * (time[n] + state.channels[ch].delay[n]);
            state.channels[ch].delay[n] = time[256 + n];
        }
        state.channels[ch].coeffs[..256].copy_from_slice(&out_pcm);
    }
}

// Naive reference 512-point IMDCT (§7.9.4.1). Given N/2=256 transform
// coefficients, produces 512 time-domain samples prior to windowing.
//
// IMDCT formula: x[n] = (2/N) * sum_{k=0..N/2-1} X[k] * cos( (π/(2N)) * (2n+1+N/2) * (2k+1) ).
//
// This is the DFT-style reference implementation — not fast, but
// correct and matches the spec's prescribed output polarity /
// scaling so that window+overlap-add reproduces the original PCM.

// ---------------------------------------------------------------------
// `imdct_256_pair`: DEPRECATED reference — NOT the canonical short-block
// IMDCT. Kept behind `cfg(test)` for regression inspection only.
//
// The forward MDCT spec at §8.2.3.2 has an α parameter that picks the
// phase offset: α=-1 for the first short transform, α=0 for the long
// transform, α=+1 for the second short transform. This function tries
// to reconstruct the per-half direct-form from that spec, with X1 using
// phase `π/(2N)·(2n+1)·(2k+1)` (no `+N/2` shift) and X2 using the
// standard `π/(2N)·(2n+1+N/2)·(2k+1)`. In practice this DOES NOT match
// the §7.9.4.2 FFT decomposition output — the two disagree with ~40%
// residual on random input (see `imdct::tests::short_block_direct_form_disagrees`).
// The FFT path is the canonical one per the spec; keep this around only
// so a future audit can bisect which side is wrong.
#[cfg(test)]
fn imdct_256_pair(x: &[f32; 256], out: &mut [f32; 512]) {
    use std::f32::consts::PI;
    let n: usize = 256;
    let scale = -1.0f32;
    let mut x1 = [0.0f32; 128];
    let mut x2 = [0.0f32; 128];
    for k in 0..128 {
        x1[k] = x[2 * k];
        x2[k] = x[2 * k + 1];
    }
    // First short transform: phase offset (1+α)=0 → pure cos(π/(2N)*(2n+1)*(2k+1)).
    for nn in 0..n {
        let mut s = 0.0f32;
        for k in 0..128 {
            let phase = PI / (2.0 * n as f32) * ((2 * nn + 1) as f32) * ((2 * k + 1) as f32);
            s += x1[k] * phase.cos();
        }
        out[nn] = scale * s;
    }
    // Second short transform: phase offset (1+α)=2 → standard IMDCT with +N/2.
    for nn in 0..n {
        let mut s = 0.0f32;
        for k in 0..128 {
            let phase =
                PI / (2.0 * n as f32) * ((2 * nn + 1 + n / 2) as f32) * ((2 * k + 1) as f32);
            s += x2[k] * phase.cos();
        }
        out[256 + nn] = scale * s;
    }
}

pub fn imdct_512(x: &[f32; 256], out: &mut [f32; 512]) {
    // Direct reference implementation of the 512-point IMDCT described
    // in §7.9.4.1 of A/52:2018. The spec provides a fast FFT-based
    // decomposition with pre/post-twiddle, but the mathematical
    // definition — a sum of cosines over the 256 transform bins —
    // produces identical output and is easier to audit.
    //
    //   x[n] = sum_{k=0..N/2-1} X[k] * cos( π/(2N) * (2n+1+N/2) * (2k+1) )
    //
    // The AC-3 reconstruction chain scales by `2 * (x + delay)` in the
    // overlap-add step (spec pseudocode at end of §7.9.4.1), so the
    // IMDCT itself needs no explicit `2/N` normalisation; pairing that
    // with AC-3's windowing produces full-scale PCM for a full-scale
    // transform coefficient.
    use std::f32::consts::PI;
    let n: usize = 512;
    // The AC-3 encoder applies an explicit `-2/N` scale to the forward
    // MDCT (§8.2.3.2). Our decoder undoes that via the IMDCT scale +
    // the `2*(x + delay)` overlap-add (§7.9.4.1 step 6). Calibrated
    // empirically against the validator binary on the 440 Hz @ 192 kbps
    // fixture: peak validator-decoded 2897 int16, our output 2895 with
    // `scale = -1.0`. The sign flip cancels the encoder's `-2/N` sign so
    // positive-amplitude input reconstructs as positive-amplitude PCM.
    let scale = -1.0f32;
    for nn in 0..n {
        let mut s = 0.0f32;
        for k in 0..256 {
            let phase =
                PI / (2.0 * n as f32) * ((2 * nn + 1 + n / 2) as f32) * ((2 * k + 1) as f32);
            s += x[k] * phase.cos();
        }
        out[nn] = scale * s;
    }
}

#[cfg(test)]
mod short_block_tests {
    use super::*;

    /// Regression / bisection fixture. The naive direct-form short-block
    /// IMDCT (derived from §8.2.3.2's α=-1/+1 phase offsets) does NOT
    /// match the §7.9.4.2 FFT decomposition used in production. We
    /// assert the disagreement explicitly here — if a future fix makes
    /// the two align, that's a signal that BOTH the direct form AND the
    /// FFT path changed together, and the test can then be tightened
    /// into a proper equality gate. Until then the FFT path is
    /// considered canonical: its PCM output is byte-equivalent to the
    /// reference S16LE produced by black-box validator-binary decode of
    /// transient fixtures (cross-validated in the transient-fixture
    /// integration tests).
    #[test]
    fn short_block_direct_form_diverges_from_fft() {
        // LCG-based deterministic "random" input — no rand dependency.
        let mut x = [0.0f32; 256];
        let mut s: u32 = 0x1234_5678;
        for v in x.iter_mut() {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            *v = (s as i32 as f32) / (i32::MAX as f32);
        }
        let mut d = [0.0f32; 512];
        let mut f = [0.0f32; 512];
        imdct_256_pair(&x, &mut d);
        crate::imdct::imdct_256_pair_fft(&x, &mut f);
        let sse: f32 = d.iter().zip(f.iter()).map(|(a, b)| (a - b).powi(2)).sum();
        let rmse = (sse / 512.0).sqrt();
        // Currently observed: RMSE ≈ 4-5 on ~unit-magnitude random input.
        // Assert the divergence exists (>0.5) so this test fails loudly if
        // someone accidentally makes both paths compute the same thing.
        assert!(
            rmse > 0.5,
            "direct form and FFT path now match (rmse={rmse:.3}) — promote short_block_direct_form_diverges_from_fft to equality"
        );
    }
}

#[cfg(test)]
mod spx_tests {
    use super::*;

    /// Table E3.13: spx sub-band `s` begins at transform coefficient
    /// `25 + 12·s`. Sub-band 17 is the one-past-last marker at tc# 229.
    #[test]
    fn spx_bandtable_matches_table_e3_13() {
        assert_eq!(spx_bandtable(0), 25);
        assert_eq!(spx_bandtable(1), 37);
        assert_eq!(spx_bandtable(2), 49);
        assert_eq!(spx_bandtable(9), 133);
        assert_eq!(spx_bandtable(16), 217);
        assert_eq!(spx_bandtable(17), 229);
    }

    /// §E.3.6.4.1 translation plan — no-wrap case: the copy region is
    /// at least as large as the SPX region, so the cursor never wraps
    /// and the map is a straight run from `copystart`.
    #[test]
    fn spx_translation_plan_no_wrap() {
        // copy region [25, 133) = 108 bins; SPX region of 2×12-bin
        // bands = 24 bins — fits without wrapping.
        let mut sztab = [0usize; 18];
        sztab[0] = 12;
        sztab[1] = 12;
        let (map, wrap) = spx_translation_plan(25, 133, 2, &sztab);
        assert_eq!(map.len(), 24);
        for (i, &src) in map.iter().enumerate() {
            assert_eq!(src, 25 + i);
        }
        assert!(wrap.iter().all(|&w| !w), "no band may wrap");
    }

    /// §E.3.6.4.1 translation plan — wrap case with the spec's
    /// PRE-BAND check: when the next band would run past `copyend`,
    /// the cursor resets to `copystart` BEFORE the band starts (it does
    /// not consume the tail of the copy region first).
    #[test]
    fn spx_translation_plan_pre_band_wrap() {
        // Copy region [25, 43) = 18 bins; three 12-bin bands.
        // Band 0: bins 25..37 (12 taken, cursor at 37).
        // Band 1: 37 + 12 > 43 → pre-band wrap → bins 25..37 again.
        // Band 2: 37 + 12 > 43 → pre-band wrap → bins 25..37 again.
        let mut sztab = [0usize; 18];
        sztab[0] = 12;
        sztab[1] = 12;
        sztab[2] = 12;
        let (map, wrap) = spx_translation_plan(25, 43, 3, &sztab);
        assert_eq!(map.len(), 36);
        assert_eq!(&map[0..12], &(25..37).collect::<Vec<_>>()[..]);
        assert_eq!(&map[12..24], &(25..37).collect::<Vec<_>>()[..]);
        assert_eq!(&map[24..36], &(25..37).collect::<Vec<_>>()[..]);
        assert!(!wrap[0], "band 0 border site is unconditional, not flagged");
        assert!(wrap[1] && wrap[2], "bands 1/2 wrap");
    }

    /// §E.3.6.4.1 translation plan — wrap case with the spec's PER-BIN
    /// check: a merged (24-bin) band larger than the copy region wraps
    /// mid-band via the `copyindex == copyend` test.
    #[test]
    fn spx_translation_plan_per_bin_wrap() {
        // Copy region [25, 41) = 16 bins; one 24-bin merged band.
        // Pre-band check fires (24 > 16) → start at 25; after 16 bins
        // the per-bin check wraps the cursor back to 25 for the last 8.
        let mut sztab = [0usize; 18];
        sztab[0] = 24;
        let (map, wrap) = spx_translation_plan(25, 41, 1, &sztab);
        assert_eq!(map.len(), 24);
        assert_eq!(&map[0..16], &(25..41).collect::<Vec<_>>()[..]);
        assert_eq!(&map[16..24], &(25..33).collect::<Vec<_>>()[..]);
        assert!(
            !wrap[0],
            "band 0 wraps are not flagged (border site is unconditional)"
        );
    }

    /// §E.3.3.2 `nrematbd` decision tree — every arm of the spec
    /// pseudo-code. The enhanced-coupling arm (`cplinu && ecplinu`) sizes
    /// from the raw `ecplbegf` code and is the one that 2/0 ecpl frames
    /// reach; the rest reproduce the SPX / standard-coupling / no-coupling
    /// behaviour already exercised by the parse loop.
    #[test]
    fn nrematbd_e3_3_2_decision_tree() {
        // No coupling, no SPX → always 4.
        assert_eq!(remat_band_count_spx(false, 0, false, 0, false, 0), 4);
        assert_eq!(remat_band_count_spx(false, 9, true, 7, false, 0), 4);

        // SPX without coupling: 3 for spx_begin_subbnd < 4 (spxbegf < 2),
        // else 4. ecpl flags are ignored when cplinu == false.
        assert_eq!(remat_band_count_spx(false, 0, false, 0, true, 3), 3);
        assert_eq!(remat_band_count_spx(false, 0, false, 0, true, 4), 4);
        assert_eq!(remat_band_count_spx(false, 0, true, 9, true, 2), 3);

        // Standard coupling (cplinu && !ecplinu): identical to
        // `remat_band_count(true, cplbegf)`. The ecplbegf argument is
        // ignored on this arm.
        assert_eq!(remat_band_count_spx(true, 0, false, 9, false, 0), 2);
        assert_eq!(remat_band_count_spx(true, 1, false, 9, false, 0), 3);
        assert_eq!(remat_band_count_spx(true, 2, false, 9, false, 0), 3);
        assert_eq!(remat_band_count_spx(true, 3, false, 9, false, 0), 4);
        assert_eq!(remat_band_count_spx(true, 9, false, 9, false, 0), 4);

        // Enhanced coupling (cplinu && ecplinu): thresholds the raw
        // ecplbegf 0/1/2/<5/else → 0/1/2/3/4. cplbegf is ignored.
        assert_eq!(remat_band_count_spx(true, 9, true, 0, false, 0), 0);
        assert_eq!(remat_band_count_spx(true, 9, true, 1, false, 0), 1);
        assert_eq!(remat_band_count_spx(true, 9, true, 2, false, 0), 2);
        assert_eq!(remat_band_count_spx(true, 9, true, 3, false, 0), 3);
        assert_eq!(remat_band_count_spx(true, 9, true, 4, false, 0), 3);
        assert_eq!(remat_band_count_spx(true, 9, true, 5, false, 0), 4);
        assert_eq!(remat_band_count_spx(true, 0, true, 15, false, 0), 4);
        // SPX co-active with enhanced coupling: the cplinu arm wins (SPX
        // only matters when coupling is off), so ecplbegf still drives it.
        assert_eq!(remat_band_count_spx(true, 0, true, 0, true, 2), 0);
        assert_eq!(remat_band_count_spx(true, 0, true, 5, true, 9), 4);
    }

    /// §E.3.6.3 spectral-extension coordinate decode. For exponent < 15
    /// the mantissa is `(spxcomant + 4) / 8`, shifted right by
    /// `spxcoexp + 3·mstrspxco`. For exponent == 15 it's `spxcomant / 4`.
    /// These mirror the encoder-side math the parse in `eac3::dsp` runs;
    /// duplicated here as an independent oracle.
    #[test]
    fn spx_coordinate_decode_formula() {
        // Mirrors the §E.3.6.3 decode the parse in `eac3::dsp` runs.
        fn spxco(coexp: i32, comant: i32, mstr: i32) -> f32 {
            let temp = if coexp == 15 {
                comant as f32 / 4.0
            } else {
                (comant as f32 + 4.0) / 8.0
            };
            let shift = coexp + 3 * mstr;
            temp * 2f32.powi(-shift)
        }
        // exp = 0, mant = 3, mstr = 0 → (3+4)/8 = 0.875, no shift.
        assert!((spxco(0, 3, 0) - 0.875).abs() < 1e-6);
        // exp = 2, mant = 1, mstr = 1 → (1+4)/8 = 0.625, >> (2 + 3) = 5.
        assert!((spxco(2, 1, 1) - 0.625 / 32.0).abs() < 1e-7);
        // exp = 15 limiting case → mant/4 (= 0.5 for mant 2), no shift
        // beyond the 15 exponent.
        assert!((spxco(15, 2, 0) - 2.0 / 4.0 / 2f32.powi(15)).abs() < 1e-9);
    }

    /// §E.3.6.2 band sizing. With the default Table E2.11 banding and a
    /// full sub-band range (begin=2, end=17) the merge bits at sub-bands
    /// 8/10/12/14/16 fold pairs of 12-coefficient sub-bands into 24-wide
    /// bands. We replicate the pseudo-code here against a hand-built
    /// `Ac3State` and check the resulting band-size table.
    #[test]
    fn spx_band_sizing_default_banding() {
        // begin=2, end=8 (sub-bands 2..7 active): merge bit only at 8
        // doesn't appear in range (8 == end excluded), so 6 bands of 12.
        let begin = 2usize;
        let end = 8usize;
        let mut bndstrc = [false; 18];
        bndstrc[8] = true; // default merge bit, out of [begin+1, end) here.
        let (n, sztab) = derive_bands(begin, end, &bndstrc);
        assert_eq!(n, 6);
        assert!(sztab[..6].iter().all(|&s| s == 12));

        // begin=2, end=17 with default merges at 8,10,12,14,16: the
        // merge bits land inside [3,17) and combine the second of each
        // pair. nspxbnds = 15 sub-bands → 10 bands (5 of width 24,
        // 5 of width 12) by the pseudo-code.
        let mut bndstrc = [false; 18];
        for &b in &[8usize, 10, 12, 14, 16] {
            bndstrc[b] = true;
        }
        let (n, sztab) = derive_bands(2, 17, &bndstrc);
        // 15 sub-bands, 5 merges → 10 bands.
        assert_eq!(n, 10);
        // Total coefficients == 15 sub-bands × 12 == 180.
        let total: usize = sztab[..n].iter().sum();
        assert_eq!(total, 180);
    }

    // Local re-implementation of the §E.3.6.2 nspxbnds / spxbndsztab
    // pseudo-code, used only by the test above.
    fn derive_bands(begin: usize, end: usize, bndstrc: &[bool; 18]) -> (usize, [usize; 18]) {
        let mut n = 1usize;
        let mut t = [0usize; 18];
        t[0] = 12;
        for bnd in (begin + 1)..end {
            if !bndstrc[bnd] {
                t[n] = 12;
                n += 1;
            } else {
                t[n - 1] += 12;
            }
        }
        (n, t)
    }

    /// End-to-end synthesis check for `apply_spectral_extension`. Build a
    /// channel whose low-frequency copy region carries a known ramp, set
    /// up one SPX band with a pure-signal blend (sblend=1, nblend=0,
    /// coord=1/32 so the ·32 scale cancels), and verify the SPX region is
    /// populated with copied values (not left silent) and that `end_mant`
    /// extends to the SPX end.
    #[test]
    fn spx_synthesis_copies_and_scales() {
        let mut state = Ac3State::new();
        let ch = 0usize;
        // SPX geometry: copy from sub-band 0 (tc 25), begin sub-band 2
        // (tc 49), end sub-band 4 (tc 73). One band of 24 (sub-bands
        // 2+3 merged via bndstrc[3]).
        state.spx_in_use = true;
        state.channels[ch].in_spx = true;
        state.spx_strtf = 0; // copystart = 25
        state.spx_begin_subbnd = 2; // copyend / spx_begin = 49
        state.spx_end_subbnd = 4; // spx_end = 73
        state.spx_bndstrc = [false; 18];
        state.spx_bndstrc[3] = true; // merge sub-band 3 into the band
        state.spx_nbnds = 1;
        state.spx_bndsztab = [0; 18];
        state.spx_bndsztab[0] = 24;
        // Pure-signal blend, unity-after-·32 coord.
        state.channels[ch].spx_sblend[0] = 1.0;
        state.channels[ch].spx_nblend[0] = 0.0;
        state.channels[ch].spx_coord[0] = 1.0 / 32.0;
        state.channels[ch].end_mant = 49; // coded mantissas stop at SPX begin.

        // Fill the copy region [25, 49) with a known non-zero ramp.
        for bin in 25..49 {
            state.channels[ch].coeffs[bin] = (bin - 25) as f32 + 1.0;
        }
        // SPX region [49, 73) starts silent.
        for bin in 49..73 {
            state.channels[ch].coeffs[bin] = 0.0;
        }

        apply_spectral_extension(&mut state, 1);

        // The SPX region must now be non-silent (copied + scaled).
        let nonzero = (49..73)
            .filter(|&b| state.channels[ch].coeffs[b] != 0.0)
            .count();
        assert!(
            nonzero >= 23,
            "SPX region should be populated, got {nonzero} non-zero bins"
        );
        // With sblend=1, nblend=0, coord·32=1, the first SPX bin equals
        // the first copied coefficient (copyindex starts at copystart=25).
        assert!(
            (state.channels[ch].coeffs[49] - state.channels[ch].coeffs[25]).abs() < 1e-4,
            "first SPX bin {} should equal first copy bin {}",
            state.channels[ch].coeffs[49],
            state.channels[ch].coeffs[25],
        );
        // end_mant extends to the SPX end so dynrng + IMDCT cover it.
        assert_eq!(state.channels[ch].end_mant, 73);
    }

    /// `apply_spectral_extension` must be a no-op for a channel not in
    /// SPX (and for base AC-3, which never sets `spx_in_use`).
    #[test]
    fn spx_synthesis_noop_when_disabled() {
        let mut state = Ac3State::new();
        for bin in 49..73 {
            state.channels[0].coeffs[bin] = 0.0;
        }
        state.spx_in_use = false;
        apply_spectral_extension(&mut state, 1);
        assert!((49..73).all(|b| state.channels[0].coeffs[b] == 0.0));
    }

    /// Spot-check three rows of Table E3.14 against the spec values. The
    /// scaling rows (0, 14, 29) cover the table's value-doubling
    /// progression (each ~2× step in `binindex=0` halves at the next
    /// power-of-two row) so a transcription typo on any of them stands
    /// out immediately.
    #[test]
    fn spx_atten_table_matches_spec() {
        // Compare against the spec's full 9-decimal-digit values held in
        // f64 (f32 literals would clip and trip `excessive_precision`).
        // f32 precision is ~7 digits so we compare within 1e-6 absolute.
        let check = |row: usize, col: usize, spec: f64| {
            let got = SPX_ATTEN_TABLE[row][col] as f64;
            assert!(
                (got - spec).abs() < 1e-6,
                "row {row} col {col}: got {got}, spec {spec}",
            );
        };
        // Row 0.
        check(0, 0, 0.954_841_604);
        check(0, 1, 0.911_722_489);
        check(0, 2, 0.870_550_563);
        // Row 14 (half-attenuation reference: T[0]=0.5).
        check(14, 0, 0.5);
        check(14, 1, 0.25);
        check(14, 2, 0.125);
        // Row 29 (quarter-attenuation reference: T[0]=0.25).
        check(29, 0, 0.25);
        check(29, 1, 0.0625);
        check(29, 2, 0.015_625);
        // 32 rows total per the 5-bit `spxattencod[ch]` field.
        assert_eq!(SPX_ATTEN_TABLE.len(), 32);
    }

    /// `apply_spx_atten_notch` is the 5-tap symmetric filter
    /// `[T[0], T[1], T[2], T[1], T[0]]`. Drive it on a constant-1 buffer
    /// and read back the filtered bins — they must equal the kernel.
    #[test]
    fn spx_atten_notch_kernel_is_symmetric() {
        let mut coeffs = [0.0f32; N_COEFFS];
        for v in coeffs.iter_mut().take(50) {
            *v = 1.0;
        }
        // Apply at filtbin=10 with code=14 (T = [0.5, 0.25, 0.125]).
        apply_spx_atten_notch(&mut coeffs, 10, 14);
        assert!((coeffs[10] - 0.5).abs() < 1e-6);
        assert!((coeffs[11] - 0.25).abs() < 1e-6);
        assert!((coeffs[12] - 0.125).abs() < 1e-6);
        assert!((coeffs[13] - 0.25).abs() < 1e-6); // mirror
        assert!((coeffs[14] - 0.5).abs() < 1e-6); // mirror
                                                  // Outside the 5-tap window, coefficients are untouched.
        assert_eq!(coeffs[9], 1.0);
        assert_eq!(coeffs[15], 1.0);
    }

    /// `apply_spx_atten_notch` masks the 5-bit code so a malformed
    /// 6-or-7-bit value doesn't index out of bounds.
    #[test]
    fn spx_atten_notch_masks_code_to_5_bits() {
        let mut coeffs = [1.0f32; N_COEFFS];
        // 0x3F & 0x1F == 31 → row 31. Should not panic.
        apply_spx_atten_notch(&mut coeffs, 0, 0x3F);
        assert!((coeffs[0] - SPX_ATTEN_TABLE[31][0]).abs() < 1e-6);
    }

    /// With `spx_atten_active == true` and `spxattencod = 14` (the
    /// half-attenuation row), the 5 bins centred on the baseband /
    /// extension border (i.e. starting at `spx_begin_tc - 2`) must
    /// be attenuated by `[0.5, 0.25, 0.125, 0.25, 0.5]` AFTER the
    /// translation copy and BEFORE the noise/coord blend. We isolate
    /// the filter contribution by setting blend factors to a pure-pass
    /// (sblend=1, nblend=0, coord=1/32).
    #[test]
    fn spx_synthesis_applies_border_notch_when_chinspxatten() {
        let mut state = Ac3State::new();
        let ch = 0usize;
        state.spx_in_use = true;
        state.channels[ch].in_spx = true;
        state.spx_strtf = 0; // copystart = 25
        state.spx_begin_subbnd = 2; // copyend / spx_begin = 49
        state.spx_end_subbnd = 4; // spx_end = 73
        state.spx_bndstrc = [false; 18];
        state.spx_bndstrc[3] = true;
        state.spx_nbnds = 1;
        state.spx_bndsztab = [0; 18];
        state.spx_bndsztab[0] = 24;
        state.channels[ch].spx_sblend[0] = 1.0;
        state.channels[ch].spx_nblend[0] = 0.0;
        state.channels[ch].spx_coord[0] = 1.0 / 32.0;
        state.channels[ch].end_mant = 49;
        // Drive the whole low-frequency region with a constant signal
        // so the copy + notch is easy to read out.
        for bin in 0..49 {
            state.channels[ch].coeffs[bin] = 1.0;
        }
        // Enable the §3.6.4.2.3 notch with the half-attenuation row.
        state.channels[ch].spx_atten_active = true;
        state.channels[ch].spx_atten_code = 14;

        apply_spectral_extension(&mut state, 1);

        // Border is at spx_begin_tc = 49 → filter window [47, 51].
        let expected = [0.5_f32, 0.25, 0.125, 0.25, 0.5];
        for (i, exp) in expected.iter().enumerate() {
            let bin = 47 + i;
            // Bins 47, 48 are in the baseband (constant=1, scaled by tap).
            // Bins 49, 50, 51 are in the SPX region (copied=1, then scaled
            // by tap, then scaled by sblend=1 * coord=1/32 * 32 = 1).
            assert!(
                (state.channels[ch].coeffs[bin] - exp).abs() < 1e-4,
                "border bin {bin} = {} expected {exp}",
                state.channels[ch].coeffs[bin]
            );
        }
        // Untouched neighbour: bin 46 (below the filter window).
        assert!((state.channels[ch].coeffs[46] - 1.0).abs() < 1e-6);
    }

    /// With `spx_atten_active == false` the SPX synthesis is byte-
    /// identical to the round-100 baseline — the border bins stay at the
    /// copied value (no attenuation applied).
    #[test]
    fn spx_synthesis_no_atten_when_chinspxatten_off() {
        let mut state = Ac3State::new();
        let ch = 0usize;
        state.spx_in_use = true;
        state.channels[ch].in_spx = true;
        state.spx_strtf = 0;
        state.spx_begin_subbnd = 2;
        state.spx_end_subbnd = 4;
        state.spx_bndstrc = [false; 18];
        state.spx_bndstrc[3] = true;
        state.spx_nbnds = 1;
        state.spx_bndsztab = [0; 18];
        state.spx_bndsztab[0] = 24;
        state.channels[ch].spx_sblend[0] = 1.0;
        state.channels[ch].spx_nblend[0] = 0.0;
        state.channels[ch].spx_coord[0] = 1.0 / 32.0;
        state.channels[ch].end_mant = 49;
        for bin in 0..49 {
            state.channels[ch].coeffs[bin] = 1.0;
        }
        state.channels[ch].spx_atten_active = false;

        apply_spectral_extension(&mut state, 1);

        // Border bins are NOT attenuated.
        for bin in 47..=51 {
            assert!(
                (state.channels[ch].coeffs[bin] - 1.0).abs() < 1e-4,
                "no-atten border bin {bin} should stay at 1.0, got {}",
                state.channels[ch].coeffs[bin]
            );
        }
    }

    /// §3.6.4.2.3 wrap-point filtering: when band 1's copy cursor wraps
    /// back to `copystart`, a second 5-tap notch must apply at the start
    /// of band 1 (`band_start - 2`). Construct a geometry where the
    /// copy region is smaller than one band so the second band guarantees
    /// a wrap, and verify a second attenuated 5-bin window appears.
    #[test]
    fn spx_synthesis_applies_wrap_notch_on_band_boundary() {
        let mut state = Ac3State::new();
        let ch = 0usize;
        state.spx_in_use = true;
        state.channels[ch].in_spx = true;
        // Copy region = sub-bands 0..1 only (12 bins: [25, 37)). Two SPX
        // bands of 12 each starting at sub-band 1 → spx region [37, 61).
        // Each SPX band consumes 12 bins from a 12-bin copy region, so
        // the second band MUST wrap.
        state.spx_strtf = 0; // copystart = 25
        state.spx_begin_subbnd = 1; // copyend = 37, spx_begin = 37
        state.spx_end_subbnd = 3; // spx_end = 61
        state.spx_bndstrc = [false; 18];
        state.spx_nbnds = 2;
        state.spx_bndsztab = [0; 18];
        state.spx_bndsztab[0] = 12;
        state.spx_bndsztab[1] = 12;
        state.channels[ch].spx_sblend[0] = 1.0;
        state.channels[ch].spx_nblend[0] = 0.0;
        state.channels[ch].spx_coord[0] = 1.0 / 32.0;
        state.channels[ch].spx_sblend[1] = 1.0;
        state.channels[ch].spx_nblend[1] = 0.0;
        state.channels[ch].spx_coord[1] = 1.0 / 32.0;
        state.channels[ch].end_mant = 37;
        for bin in 0..49 {
            state.channels[ch].coeffs[bin] = 1.0;
        }
        state.channels[ch].spx_atten_active = true;
        state.channels[ch].spx_atten_code = 14; // [0.5, 0.25, 0.125]

        apply_spectral_extension(&mut state, 1);

        // Band 1 starts at SPX bin 49 (37 + 12). Wrap notch is centred
        // on the first bin of band 1 → filter window [47, 51].
        // Border notch (always-applied) is centred on spx_begin_tc=37
        // → filter window [35, 39].
        let expected = [0.5_f32, 0.25, 0.125, 0.25, 0.5];
        for (i, exp) in expected.iter().enumerate() {
            let bin = 35 + i;
            assert!(
                (state.channels[ch].coeffs[bin] - exp).abs() < 1e-4,
                "border-notch bin {bin} expected {exp} got {}",
                state.channels[ch].coeffs[bin]
            );
        }
        for (i, exp) in expected.iter().enumerate() {
            let bin = 47 + i;
            assert!(
                (state.channels[ch].coeffs[bin] - exp).abs() < 1e-4,
                "wrap-notch bin {bin} expected {exp} got {}",
                state.channels[ch].coeffs[bin]
            );
        }
        // Untouched between the two notches: bin 40..46 stay at 1.0.
        for bin in 40..=46 {
            assert!(
                (state.channels[ch].coeffs[bin] - 1.0).abs() < 1e-4,
                "bin {bin} between notches should be 1.0, got {}",
                state.channels[ch].coeffs[bin]
            );
        }
    }
}

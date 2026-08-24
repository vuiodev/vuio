//! DTS Coherent Acoustics — §D.2 quantization step-size tables and the
//! §5.5 inverse-quantization scale composition.
//!
//! Round 293 (2026-06-14) lands the dequantization bridge between the
//! quantization-index decode (the §C.2.1 block-code / Annex D Huffman
//! `AUDIO[m]` indices) and the §C.2.2 inverse-ADPCM / §C.2.5 QMF
//! synthesis inputs: the per-subband real scale factor `rScale` and
//! the `aSample[m] = rScale · AUDIO[m]` sample scaling that the §5.5
//! "Primary Audio Data Arrays" `Audio Data` block (Table 5-29)
//! applies once per subsubframe.
//!
//! Source: ETSI TS 102 114 V1.3.1 (2011-08), staged PDF at
//! `docs/audio/dts/etsi-ts-102114-dts-coherent-acoustics.pdf`.
//!
//! Two clauses combine here:
//!
//! * **Annex D §D.2 "Quantization Step Size"** (staged PDF p.193-194)
//!   gives two 32-entry tables indexed by the `ABITS` bit-allocation
//!   index — §D.2.1 *Lossy Quantization* and §D.2.2 *Lossless
//!   Quantization* — each entry tabulated as the integer
//!   `Step-size × 2²²`. The §5.5 pseudocode selects between them by
//!   the frame-header `RATE` field:
//!
//!   ```text
//!   if (RATE == 0x1f) pStepSizeTable = &StepSizeLossLess; // Lossless
//!   else              pStepSizeTable = &StepSizeLossy;    // Lossy
//!   ```
//!
//! * **§5.5 Table 5-29 `Audio Data`** (staged PDF p.31-32) composes
//!   the looked-up `rStepSize` with the §D.1.1 / §D.1.2 RMS
//!   square-root `SCALES[ch][n][0..2]` factor into the per-sample
//!   real multiplier, transient-aware:
//!
//!   ```text
//!   pStepSizeTable->LookUp(nABITS, rStepSize);
//!   nTmode = TMODE[ch][n];
//!   if (nTmode == 0) nTmode = nSSC;            // No transient
//!   if (nSubSubFrame < nTmode)                 // Pre-transient
//!       rScale = rStepSize * SCALES[ch][n][0]; // First scale factor
//!   else                                       // After-transient
//!       rScale = rStepSize * SCALES[ch][n][1]; // Second scale factor
//!   rScale *= arADJ[ch][SEL[ch][nABITS-1]];    // 1 unless Huffman
//!   for (m=0; m<8; m++, nSample++)
//!       aPrmCh[ch].aSubband[n].aSample[nSample] = rScale * AUDIO[m];
//!   ```
//!
//! This module exposes the two §D.2 tables verbatim, real-valued
//! step-size accessors that divide out the §D.2 `× 2²²` scaling, the
//! §5.5 transient-aware `rScale` composition, and the eight-sample
//! `rScale · AUDIO[m]` scaling. The `arADJ[][]` adjustment multiplier
//! (the round-241 [`crate::ScaleFactorAdjustment`]) is a caller-passed
//! factor — the §5.5 pseudocode comments it is "assumed 1 unless
//! changed by bit stream when SEL indicates Huffman code", so callers
//! that did not read an `ADJ` field pass `1.0`.
//!
//! # Scope and follow-ups
//!
//! The §C.2.1 block-code / Huffman decode that *produces* `AUDIO[m]`
//! is already landed ([`crate::decode_block_code`] and the
//! [`crate::side_info`] Huffman decoders); the §D.2 step-size table
//! selection by `RATE` is exposed here as
//! [`StepSizeTable::for_rate`]. The §C.2.2 inverse-ADPCM step that the
//! §5.5 pseudocode runs *after* this scaling (when `PMODE != 0`) is
//! the separate round-228 [`crate::inverse_adpcm_decode_f64`]; this
//! module stops at the `rScale · AUDIO[m]` product, leaving the
//! ADPCM pass and the DSYNC trailer to the per-subsubframe driver
//! that composes them.

use crate::side_info::ScaleFactorAdjustment;
use crate::subframe::ChannelSideInfo;
use crate::{Error, Result};

/// Number of `ABITS` bit-allocation indices the §D.2 step-size tables
/// tabulate (PDF p.193-194: indices `0..=31`, of which `27..=31` are
/// written "invalid").
pub const STEP_SIZE_TABLE_LEN: usize = 32;

/// The `× 2²²` fixed-point scaling the §D.2 tables apply to every
/// tabulated step size (PDF p.193-194 column header
/// "Step-size × 2²²"). A real step size is recovered by
/// `entry as f64 / 2f64.powi(STEP_SIZE_SCALE_SHIFT)`.
pub const STEP_SIZE_SCALE_SHIFT: i32 = 22;

/// Sentinel placed in the `27..=31` slots the §D.2 tables write as
/// "invalid". Reading these indices through a real-valued accessor
/// surfaces [`Error::InvalidStepSize`] rather than returning `0.0`,
/// so structurally-corrupt `ABITS` values fail loudly.
const STEP_SIZE_INVALID: u32 = 0;

/// First `ABITS` index the §D.2 tables mark "invalid" (PDF p.193-194).
/// Indices `0..STEP_SIZE_FIRST_INVALID` carry a defined step size;
/// indices `STEP_SIZE_FIRST_INVALID..32` are reserved.
pub const STEP_SIZE_FIRST_INVALID: usize = 27;

// ---------------------------------------------------------------
// Annex D §D.2.1 — Lossy Quantization (staged PDF p.193).
// Each entry is `Step-size × 2²²`. Indices 27..=31 are "invalid".
// Transcribed verbatim from the staged PDF.
// ---------------------------------------------------------------

/// §D.2.1 lossy-quantization step sizes, tabulated as
/// `Step-size × 2²²` and indexed by the `ABITS` bit-allocation index.
/// Selected when the frame-header `RATE != 0x1f` (§5.5). Slots
/// `27..=31` hold a zero sentinel because the PDF writes them
/// "invalid".
pub const STEP_SIZE_LOSSY: [u32; STEP_SIZE_TABLE_LEN] = [
    0, // ABITS 0 — "0,0" nominal: no bits allocated.
    6710886,
    4194304,
    3355443,
    2474639,
    2097152,
    1761608,
    1426063,
    796918,
    461373,
    251658,
    146801,
    79692,
    46137,
    27263,
    16777,
    10486,
    5872,
    3355,
    1887,
    1258,
    713,
    336,
    168,
    84,
    42,
    21,
    STEP_SIZE_INVALID, // 27 invalid
    STEP_SIZE_INVALID, // 28 invalid
    STEP_SIZE_INVALID, // 29 invalid
    STEP_SIZE_INVALID, // 30 invalid
    STEP_SIZE_INVALID, // 31 invalid
];

// ---------------------------------------------------------------
// Annex D §D.2.2 — Lossless Quantization (staged PDF p.194).
// Each entry is `Step-size × 2²²`. Indices 27..=31 are "invalid".
// Transcribed verbatim from the staged PDF.
// ---------------------------------------------------------------

/// §D.2.2 lossless-quantization step sizes, tabulated as
/// `Step-size × 2²²` and indexed by the `ABITS` bit-allocation index.
/// Selected when the frame-header `RATE == 0x1f` (§5.5). Slots
/// `27..=31` hold a zero sentinel because the PDF writes them
/// "invalid".
pub const STEP_SIZE_LOSSLESS: [u32; STEP_SIZE_TABLE_LEN] = [
    0, // ABITS 0 — "0,0" nominal: no bits allocated.
    4194304,
    2097152,
    1384120,
    1048576,
    696254,
    524288,
    348127,
    262144,
    131072,
    65431,
    33026,
    16450,
    8208,
    4100,
    2049,
    1024,
    512,
    256,
    128,
    64,
    32,
    16,
    8,
    4,
    2,
    1,
    STEP_SIZE_INVALID, // 27 invalid
    STEP_SIZE_INVALID, // 28 invalid
    STEP_SIZE_INVALID, // 29 invalid
    STEP_SIZE_INVALID, // 30 invalid
    STEP_SIZE_INVALID, // 31 invalid
];

/// Which §D.2 step-size table the §5.5 `RATE` test selects.
///
/// ```text
/// if (RATE == 0x1f) pStepSizeTable = &StepSizeLossLess; // Lossless
/// else              pStepSizeTable = &StepSizeLossy;    // Lossy
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepSizeTable {
    /// §D.2.1 lossy quantization ([`STEP_SIZE_LOSSY`]) — `RATE != 0x1f`.
    Lossy,
    /// §D.2.2 lossless quantization ([`STEP_SIZE_LOSSLESS`]) —
    /// `RATE == 0x1f`.
    Lossless,
}

/// The §5.5 lossless-quantization `RATE` sentinel (`0x1f`).
pub const RATE_LOSSLESS: u8 = 0x1f;

impl StepSizeTable {
    /// Resolve the §5.5 `RATE`-driven table selection: `RATE == 0x1f`
    /// (= [`RATE_LOSSLESS`]) selects [`StepSizeTable::Lossless`], every
    /// other `RATE` value selects [`StepSizeTable::Lossy`].
    #[must_use]
    pub fn for_rate(rate: u8) -> Self {
        if rate == RATE_LOSSLESS {
            StepSizeTable::Lossless
        } else {
            StepSizeTable::Lossy
        }
    }

    /// The raw `Step-size × 2²²` integer table this selection points
    /// at (§D.2.1 or §D.2.2).
    #[must_use]
    pub fn raw_table(self) -> &'static [u32; STEP_SIZE_TABLE_LEN] {
        match self {
            StepSizeTable::Lossy => &STEP_SIZE_LOSSY,
            StepSizeTable::Lossless => &STEP_SIZE_LOSSLESS,
        }
    }

    /// Look up the real-valued step size for an `ABITS` index, undoing
    /// the §D.2 `× 2²²` fixed-point scaling.
    ///
    /// Returns [`Error::InvalidStepSize`] when `abits` is one of the
    /// `27..=31` indices the §D.2 tables write "invalid", or when
    /// `abits >= 32`.
    pub fn step_size(self, abits: u8) -> Result<f64> {
        let idx = abits as usize;
        if idx >= STEP_SIZE_TABLE_LEN || idx >= STEP_SIZE_FIRST_INVALID {
            return Err(Error::InvalidStepSize { abits });
        }
        let raw = self.raw_table()[idx];
        Ok(f64::from(raw) / 2f64.powi(STEP_SIZE_SCALE_SHIFT))
    }
}

/// The §5.5 transient-aware scale-factor selection: given the
/// per-subband `TMODE` value, the subframe's `nSSC` subsubframe count,
/// and the current `nSubSubFrame` index, decide which of the two
/// `SCALES[ch][n][0..2]` factors the §5.5 pseudocode multiplies in.
///
/// ```text
/// nTmode = TMODE[ch][n];
/// if (nTmode == 0) nTmode = nSSC;       // No transient
/// if (nSubSubFrame < nTmode) idx = 0;   // Pre-transient: first factor
/// else                       idx = 1;   // After-transient: second factor
/// ```
///
/// Returns `0` to select `SCALES[ch][n][0]` (pre-transient / no
/// transient) or `1` to select `SCALES[ch][n][1]` (post-transient).
#[must_use]
pub fn transient_scale_index(tmode: u8, n_ssc: usize, subsubframe: usize) -> usize {
    let n_tmode = if tmode == 0 {
        n_ssc
    } else {
        usize::from(tmode)
    };
    if subsubframe < n_tmode {
        0
    } else {
        1
    }
}

/// Compose the §5.5 per-subband real scale `rScale` for one
/// subsubframe:
///
/// ```text
/// rScale  = rStepSize * SCALES[ch][n][transient-selected];
/// rScale *= arADJ[ch][SEL[ch][nABITS-1]];   // adjustment multiplier
/// ```
///
/// * `table` is the §5.5 `RATE`-selected step-size table.
/// * `abits` is `ABITS[ch][n]`; the matching §D.2 step size is looked
///   up internally.
/// * `scale` is the §D.1.1 / §D.1.2 RMS square-root value already
///   resolved for `SCALES[ch][n][transient-selected]` — the §5.5
///   pseudocode's `(real)SCALES[ch][n][…]` cast (passed as the raw
///   integer the RMS table returns).
/// * `adj` is the round-241 [`ScaleFactorAdjustment`] multiplier; pass
///   [`ScaleFactorAdjustment::Adj0`] when no `ADJ` field was read
///   (the §5.5 default of `arADJ == 1`).
///
/// Returns [`Error::InvalidStepSize`] when `abits` is an invalid §D.2
/// index.
pub fn dequant_scale(
    table: StepSizeTable,
    abits: u8,
    scale: u32,
    adj: ScaleFactorAdjustment,
) -> Result<f64> {
    let step = table.step_size(abits)?;
    Ok(step * f64::from(scale) * adj.multiplier_f64())
}

/// Apply the §5.5 `aSample[m] = rScale · AUDIO[m]` scaling to one
/// subsubframe's eight quantization indices, writing the resulting
/// subband samples into `out`.
///
/// `audio` carries the eight §C.2.1 / Huffman-decoded `AUDIO[m]`
/// quantization indices for the current `(ch, n, subsubframe)`; `out`
/// receives the eight scaled subband samples. Both slices must hold
/// exactly [`SAMPLES_PER_SUBSUBFRAME`] entries, matching the §5.5
/// `for (m=0; m<8; m++)` loop.
///
/// Returns [`Error::SampleCountMismatch`] when either slice is not
/// exactly eight samples long.
pub fn scale_subsubframe_samples(audio: &[i32], r_scale: f64, out: &mut [f64]) -> Result<()> {
    if audio.len() != SAMPLES_PER_SUBSUBFRAME || out.len() != SAMPLES_PER_SUBSUBFRAME {
        return Err(Error::SampleCountMismatch {
            expected: SAMPLES_PER_SUBSUBFRAME,
            found: audio.len().max(out.len()),
        });
    }
    for (dst, &index) in out.iter_mut().zip(audio.iter()) {
        *dst = r_scale * f64::from(index);
    }
    Ok(())
}

/// Number of subband samples in one subsubframe (one subband analysis
/// subwindow): the §5.5 / §C.1 fixed `8` samples per subsubframe ("A
/// subsubframe consists of eight subband samples … for each
/// subband", PDF p.181).
pub const SAMPLES_PER_SUBSUBFRAME: usize = 8;

/// End-to-end §5.5 dequantization of one `(ch, n, subsubframe)`
/// subsubframe: resolve the transient-aware scale index, look up the
/// matching `SCALES[ch][n][…]` factor from the side-info, compose the
/// §5.5 `rScale`, and write the eight `rScale · AUDIO[m]` subband
/// samples into `out`.
///
/// This composes [`transient_scale_index`], [`dequant_scale`], and
/// [`scale_subsubframe_samples`] against the round-281
/// [`ChannelSideInfo`] plane: `abits = side.abits[n]`,
/// `tmode = side.tmode[n]`, and the scale factor is
/// `side.scales[n][transient-selected]`.
///
/// * `side` is the decoded §5.4.1 side information for channel `ch`.
/// * `n` is the subband index (`0..n_vqsub`; high-frequency VQ
///   subbands take the separate §5.5 VQ path, not this one).
/// * `n_ssc` is the subframe's subsubframe count (`SSC + 1`).
/// * `subsubframe` is the current subsubframe index (`0..n_ssc`).
/// * `table` is the §5.5 `RATE`-selected step-size table.
/// * `adj` is the §5.5 `arADJ` multiplier (unity by default).
/// * `audio` is the eight decoded `AUDIO[m]` quantization indices.
/// * `out` receives the eight scaled subband samples.
///
/// Returns [`Error::InvalidStepSize`] for an invalid `ABITS`, or
/// [`Error::SampleCountMismatch`] when a slice is not eight samples.
#[allow(clippy::too_many_arguments)]
pub fn dequant_subsubframe(
    side: &ChannelSideInfo,
    n: usize,
    n_ssc: usize,
    subsubframe: usize,
    table: StepSizeTable,
    adj: ScaleFactorAdjustment,
    audio: &[i32],
    out: &mut [f64],
) -> Result<()> {
    let abits = side.abits[n];
    let scale_idx = transient_scale_index(side.tmode[n], n_ssc, subsubframe);
    let scale = side.scales[n][scale_idx];
    let r_scale = dequant_scale(table, abits, scale, adj)?;
    scale_subsubframe_samples(audio, r_scale, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_selects_table() {
        assert_eq!(StepSizeTable::for_rate(0x1f), StepSizeTable::Lossless);
        assert_eq!(StepSizeTable::for_rate(0x00), StepSizeTable::Lossy);
        assert_eq!(StepSizeTable::for_rate(0x1e), StepSizeTable::Lossy);
        assert_eq!(StepSizeTable::for_rate(0x0a), StepSizeTable::Lossy);
    }

    #[test]
    fn tables_have_32_entries() {
        assert_eq!(STEP_SIZE_LOSSY.len(), STEP_SIZE_TABLE_LEN);
        assert_eq!(STEP_SIZE_LOSSLESS.len(), STEP_SIZE_TABLE_LEN);
    }

    #[test]
    fn abits_zero_is_zero_step() {
        // PDF p.193-194 both list ABITS 0 -> step-size 0,0.
        assert_eq!(StepSizeTable::Lossy.step_size(0).unwrap(), 0.0);
        assert_eq!(StepSizeTable::Lossless.step_size(0).unwrap(), 0.0);
    }

    #[test]
    fn lossy_step_sizes_match_nominal() {
        // Spot-check the §D.2.1 nominal column: entry / 2^22 must land
        // near the tabulated "Nominal Step-size".
        // ABITS 2 -> 4194304 / 2^22 = 1,0.
        assert!((StepSizeTable::Lossy.step_size(2).unwrap() - 1.0).abs() < 1e-9);
        // ABITS 5 -> 2097152 / 2^22 = 0,50.
        assert!((StepSizeTable::Lossy.step_size(5).unwrap() - 0.5).abs() < 1e-9);
        // ABITS 1 -> 6710886 / 2^22 ≈ 1,6 (nominal).
        assert!((StepSizeTable::Lossy.step_size(1).unwrap() - 1.6).abs() < 1e-3);
        // ABITS 11 -> 146801 / 2^22 ≈ 0,035 (nominal).
        assert!((StepSizeTable::Lossy.step_size(11).unwrap() - 0.035).abs() < 1e-4);
    }

    #[test]
    fn lossless_step_sizes_match_nominal() {
        // §D.2.2: ABITS 1 -> 4194304 / 2^22 = 1,0.
        assert!((StepSizeTable::Lossless.step_size(1).unwrap() - 1.0).abs() < 1e-9);
        // ABITS 4 -> 1048576 / 2^22 = 0,25.
        assert!((StepSizeTable::Lossless.step_size(4).unwrap() - 0.25).abs() < 1e-9);
        // ABITS 8 -> 262144 / 2^22 = 0,0625.
        assert!((StepSizeTable::Lossless.step_size(8).unwrap() - 0.0625).abs() < 1e-9);
        // ABITS 26 -> 1 / 2^22 ≈ 2,384e-7 (the smallest defined).
        assert!((StepSizeTable::Lossless.step_size(26).unwrap() - 2.384e-7).abs() < 1e-10);
    }

    #[test]
    fn invalid_abits_indices_error() {
        for abits in 27u8..=31 {
            assert_eq!(
                StepSizeTable::Lossy.step_size(abits),
                Err(Error::InvalidStepSize { abits })
            );
            assert_eq!(
                StepSizeTable::Lossless.step_size(abits),
                Err(Error::InvalidStepSize { abits })
            );
        }
        // Out-of-range index past the table also errors.
        assert_eq!(
            StepSizeTable::Lossy.step_size(40),
            Err(Error::InvalidStepSize { abits: 40 })
        );
    }

    #[test]
    fn transient_index_no_transient_uses_first_factor() {
        // TMODE == 0 -> nTmode = nSSC, so every subsubframe is
        // pre-transient -> factor 0.
        for ssf in 0..4 {
            assert_eq!(transient_scale_index(0, 4, ssf), 0);
        }
    }

    #[test]
    fn transient_index_splits_at_tmode() {
        // TMODE = 2 -> nTmode = 2: subsubframes 0,1 pre (factor 0),
        // 2,3 post (factor 1). (Spec: transition in subsubframe
        // TMODE+1, i.e. index 2.)
        assert_eq!(transient_scale_index(2, 4, 0), 0);
        assert_eq!(transient_scale_index(2, 4, 1), 0);
        assert_eq!(transient_scale_index(2, 4, 2), 1);
        assert_eq!(transient_scale_index(2, 4, 3), 1);
    }

    #[test]
    fn dequant_scale_composes_step_scale_adj() {
        // Lossy ABITS 2 step = 1.0; SCALES = 200; ADJ unity ->
        // rScale = 1.0 * 200 * 1.0 = 200.
        let r = dequant_scale(StepSizeTable::Lossy, 2, 200, ScaleFactorAdjustment::Adj0).unwrap();
        assert!((r - 200.0).abs() < 1e-9);
        // ABITS 5 step = 0.5; SCALES = 10 -> rScale = 5.0.
        let r = dequant_scale(StepSizeTable::Lossy, 5, 10, ScaleFactorAdjustment::Adj0).unwrap();
        assert!((r - 5.0).abs() < 1e-9);
    }

    #[test]
    fn scale_samples_applies_product() {
        let audio = [1, -1, 2, -2, 4, -4, 8, -8];
        let mut out = [0.0f64; SAMPLES_PER_SUBSUBFRAME];
        scale_subsubframe_samples(&audio, 3.0, &mut out).unwrap();
        let expected = [3.0, -3.0, 6.0, -6.0, 12.0, -12.0, 24.0, -24.0];
        for (got, want) in out.iter().zip(expected.iter()) {
            assert!((got - want).abs() < 1e-9);
        }
    }

    #[test]
    fn scale_samples_zero_scale_zeros_all() {
        // ABITS 0 produces a zero step size -> zero rScale -> all
        // samples zero regardless of AUDIO[m].
        let audio = [100, -100, 50, -50, 25, -25, 12, -12];
        let mut out = [9.9f64; SAMPLES_PER_SUBSUBFRAME];
        let r = dequant_scale(StepSizeTable::Lossy, 0, 500, ScaleFactorAdjustment::Adj0).unwrap();
        assert_eq!(r, 0.0);
        scale_subsubframe_samples(&audio, r, &mut out).unwrap();
        assert!(out.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn scale_samples_length_mismatch_errors() {
        let audio = [1, 2, 3];
        let mut out = [0.0f64; SAMPLES_PER_SUBSUBFRAME];
        assert!(matches!(
            scale_subsubframe_samples(&audio, 1.0, &mut out),
            Err(Error::SampleCountMismatch { .. })
        ));
        let audio8 = [0i32; SAMPLES_PER_SUBSUBFRAME];
        let mut out_short = [0.0f64; 4];
        assert!(matches!(
            scale_subsubframe_samples(&audio8, 1.0, &mut out_short),
            Err(Error::SampleCountMismatch { .. })
        ));
    }

    #[test]
    fn dequant_subsubframe_end_to_end() {
        // Build a side-info plane with one active subband.
        let mut side = ChannelSideInfo::cleared();
        side.abits[0] = 2; // lossy step = 1.0
        side.tmode[0] = 0; // no transient -> factor 0
        side.scales[0][0] = 10;
        side.scales[0][1] = 999; // must be ignored (no transient)

        let audio = [1, 2, 3, 4, 5, 6, 7, 8];
        let mut out = [0.0f64; SAMPLES_PER_SUBSUBFRAME];
        dequant_subsubframe(
            &side,
            0,
            4,
            0,
            StepSizeTable::Lossy,
            ScaleFactorAdjustment::Adj0,
            &audio,
            &mut out,
        )
        .unwrap();
        // rScale = 1.0 * 10 * 1.0 = 10 -> samples = 10*AUDIO[m].
        for (i, &v) in out.iter().enumerate() {
            assert!((v - 10.0 * (i as f64 + 1.0)).abs() < 1e-9);
        }
    }

    #[test]
    fn dequant_subsubframe_transient_picks_second_factor() {
        let mut side = ChannelSideInfo::cleared();
        side.abits[0] = 5; // lossy step = 0.5
        side.tmode[0] = 2; // transient -> nTmode = 2
        side.scales[0][0] = 4; // pre-transient factor
        side.scales[0][1] = 8; // post-transient factor

        let audio = [2i32; SAMPLES_PER_SUBSUBFRAME];
        // Pre-transient subsubframe 0 -> factor 0 (4): rScale = 0.5*4 = 2.
        let mut out = [0.0f64; SAMPLES_PER_SUBSUBFRAME];
        dequant_subsubframe(
            &side,
            0,
            4,
            0,
            StepSizeTable::Lossy,
            ScaleFactorAdjustment::Adj0,
            &audio,
            &mut out,
        )
        .unwrap();
        assert!(out.iter().all(|&v| (v - 4.0).abs() < 1e-9)); // 2 * 2.0

        // Post-transient subsubframe 3 -> factor 1 (8): rScale = 0.5*8 = 4.
        dequant_subsubframe(
            &side,
            0,
            4,
            3,
            StepSizeTable::Lossy,
            ScaleFactorAdjustment::Adj0,
            &audio,
            &mut out,
        )
        .unwrap();
        assert!(out.iter().all(|&v| (v - 8.0).abs() < 1e-9)); // 2 * 4.0
    }
}

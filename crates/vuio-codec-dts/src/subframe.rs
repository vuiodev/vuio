//! DTS Coherent Acoustics — §5.4.1 Primary Audio Coding Side
//! Information subframe walker (ETSI TS 102 114 V1.3.1, Table 5-28).
//!
//! Round 281 (2026-06-12) composes the previously-landed single-field
//! primitives — the round-249 `SSC`/`PSC` prefix, the round-195
//! ABITS / SCALES decoders, and the new round-281 TMODE decoder —
//! into the §5.4.1 side-information decode walk: one call consumes
//! the SSC/PSC prefix, the PMODE plane, the PVQ indices, the ABITS
//! plane, the TMODE plane, and the SCALES plane (including the
//! transient second scale factor and the high-frequency-VQ-subband
//! tail loop) for every primary audio channel, in the exact field
//! order Table 5-28 fixes (staged PDF p.28-29,
//! `docs/audio/dts/etsi-ts-102114-dts-coherent-acoustics.pdf`).
//!
//! # Inputs
//!
//! The per-channel loop bounds and codebook selectors come from the
//! §5.3.2 Primary Audio Coding Header (Table 5-21, staged PDF
//! p.24-25): `nPCHS = PCHS + 1` (≤ 5 primary channels per p.25),
//! `nSUBS[ch] = SUBS[ch] + 2`, `nVQSUB[ch] = VQSUB[ch] + 1`, and the
//! `BHUFF[ch]` / `THUFF[ch]` / `SHUFF[ch]` codebook selectors. The
//! Table 5-21 header decoder itself is a separate follow-up; this
//! walker takes the resolved values as [`ChannelSideInfoParams`].
//!
//! # Scope
//!
//! [`decode_primary_side_info_at`] covers Table 5-28 from
//! `SSC = ExtractBits(2)` through the end of the SCALES block (the
//! high-frequency VQ subband loop); the companion
//! [`decode_primary_side_info_tail_at`] handles the `RANGE` (`DYNF`)
//! and `SICRC` (`CPF`) tail that follows. Two pieces remain
//! follow-ups, each blocked on material outside this round:
//!
//! * the ADPCM prediction-coefficient lookup
//!   (`ADPCMCoeffVQ.LookUp(nVQIndex, PVQ[ch][n])`) needs the clause
//!   D.10.1 vector codebook — the raw 12-bit `nVQIndex` is captured
//!   in [`ChannelSideInfo::pvq_index`] so the lookup can be applied
//!   later without re-walking the bit stream;
//! * the `JOIN_SHUFF` / `JOIN_SCALES` block (transmitted only when
//!   `JOINX[ch] > 0`) needs the joint-scale-factor table to resolve
//!   the biased index into a multiplier — the tail decoder declines
//!   (`Error::InvalidSideInfo { field: "JOINX", .. }`) rather than
//!   guess the variable `JOIN_SCALES` bit count.
//!
//! The `RANGE` code the tail decoder captures is 8-bit signed Q2
//! (`dB = (int8)code × 0.25`, [`crate::dts_dynrng_to_db`] /
//! [`crate::dts_dynrng_to_linear`], per
//! `docs/audio/dts/dts-drc-dynrng.md`); the §5.4.1 pseudocode applies
//! that linear gain to every reconstructed PCM sample *after* QMF
//! synthesis. The `bits_consumed` cursor of the SCALES-block walk
//! points exactly where the JOIN_SHUFF / RANGE / SICRC reads begin.

use crate::bitreader::BitReader;
use crate::cos_mod::NUM_SUBBAND;
use crate::side_info::{
    AbitsCodebook, ScalesCodebook, SubsubframeCount, TmodeCodebook, decode_abits, decode_scales,
    decode_tmode,
};
use crate::{Error, Result};

/// Maximum number of primary audio channels in one core frame, per
/// the §5.3.2 `PCHS` field description (staged PDF p.25): "there are
/// `nPCHS = PCHS+1 ≤ 5` primary audio channels in the current
/// frame". Channels beyond the fifth are extended channels packed in
/// separate extension data arrays, not in the §5.4.1 side-info block.
pub const MAX_PRIMARY_CHANNELS: usize = 5;

/// Per-channel loop bounds + codebook selectors for the §5.4.1 walk,
/// resolved from the §5.3.2 Primary Audio Coding Header (Table 5-21).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelSideInfoParams {
    /// `nSUBS[ch] = SUBS[ch] + 2` — the number of active subbands
    /// for this channel (Table 5-21 / PDF p.25 "Subband Activity
    /// Count"). Must be ≤ [`NUM_SUBBAND`](crate::NUM_SUBBAND) (= 32).
    pub n_subs: usize,
    /// `nVQSUB[ch] = VQSUB[ch] + 1` — the first high-frequency
    /// VQ-encoded subband (PDF p.25 "High Frequency VQ Start
    /// Subband"). Subbands `0..n_vqsub` carry ABITS/TMODE/SCALES;
    /// subbands `n_vqsub..n_subs` are VQ-encoded and carry only the
    /// single SCALES factor of the Table 5-28 "High frequency VQ
    /// subbands" loop. Must be ≤ `n_subs`.
    pub n_vqsub: usize,
    /// `BHUFF[ch]` resolved through Table 5-25
    /// ([`AbitsCodebook::from_bhuff`]).
    pub abits_codebook: AbitsCodebook,
    /// `THUFF[ch]` resolved through Table 5-23
    /// ([`TmodeCodebook::from_thuff`]).
    pub tmode_codebook: TmodeCodebook,
    /// `SHUFF[ch]` resolved through Table 5-24
    /// ([`ScalesCodebook::from_shuff`]).
    pub scales_codebook: ScalesCodebook,
}

/// Decoded §5.4.1 side information for one primary audio channel.
///
/// Every plane is a fixed [`NUM_SUBBAND`](crate::NUM_SUBBAND)-slot array so downstream
/// consumers can index by subband without re-checking the per-channel
/// bounds; slots at or beyond the corresponding loop bound keep the
/// all-zero / `None` initial value (matching the spec's explicit
/// "Clear SCALES" / `TMODE[ch][n] = 0` initialisation and the
/// `ABITS = 0` "no bits allocated" convention of Table 5-26).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChannelSideInfo {
    /// `PMODE[ch][n]` — 1 when ADPCM prediction is active for
    /// subband `n` (PDF p.30). Read for `n < n_subs`; zero beyond.
    pub pmode: [u8; NUM_SUBBAND],
    /// Raw 12-bit `nVQIndex` into the clause D.10.1 ADPCM
    /// prediction-coefficient vector codebook, `Some` exactly for the
    /// subbands whose `PMODE` bit is set ("Transmitted only when
    /// ADPCM active", Table 5-28). The D.10.1 table lookup itself is
    /// a follow-up; the index is preserved so it can be applied
    /// without re-walking the bit stream.
    pub pvq_index: [Option<u16>; NUM_SUBBAND],
    /// `ABITS[ch][n]` — the bit-allocation index selecting the
    /// mid-tread linear quantizer for subband `n` (Table 5-26). Read
    /// for `n < n_vqsub`; zero (= no bits allocated) beyond.
    pub abits: [u8; NUM_SUBBAND],
    /// `TMODE[ch][n]` — 0 for no transient; a non-zero value means
    /// the transition occurred in subsubframe `TMODE[ch][n] + 1`
    /// (PDF p.30). Decoded only when `nSSC > 1`, only for
    /// `n < n_vqsub`, and only where `ABITS[ch][n] > 0`; zero
    /// everywhere else per the spec's explicit clear.
    pub tmode: [u8; NUM_SUBBAND],
    /// `SCALES[ch][n][0..2]` — the resolved scale factors (the
    /// §D.1.1 / §D.1.2 RMS square-root-table values, not the raw
    /// quantisation indexes). `scales[n][0]` is the only factor for
    /// non-transient subbands (and the pre-transient factor
    /// otherwise); `scales[n][1]` is the post-transient factor,
    /// present only where `TMODE[ch][n] > 0`. Slots the spec's
    /// "Clear SCALES" initialisation covers but no decode reaches
    /// stay `0` — unambiguous, because every documented RMS table
    /// value is ≥ 1.
    pub scales: [[u32; 2]; NUM_SUBBAND],
}

impl ChannelSideInfo {
    pub(crate) fn cleared() -> Self {
        Self {
            pmode: [0; NUM_SUBBAND],
            pvq_index: [None; NUM_SUBBAND],
            abits: [0; NUM_SUBBAND],
            tmode: [0; NUM_SUBBAND],
            scales: [[0; 2]; NUM_SUBBAND],
        }
    }
}

/// Decoded §5.4.1 Primary Audio Coding Side Information block (the
/// SSC/PSC prefix plus one [`ChannelSideInfo`] per primary channel).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrimarySideInfo {
    /// The 5-bit `SSC` / `PSC` prefix (round-249
    /// [`SubsubframeCount`]). Its `n_ssc() > 1` value is what gated
    /// the TMODE plane during the walk.
    pub subsubframe_count: SubsubframeCount,
    /// Per-channel planes, in channel order `ch = 0..nPCHS`.
    pub channels: Vec<ChannelSideInfo>,
}

/// Walk the §5.4.1 Primary Audio Coding Side Information block
/// (Table 5-28, staged PDF p.28-29) from `bytes` starting at
/// `bit_offset` (MSB-first from `bytes[0]`), given one
/// [`ChannelSideInfoParams`] per primary channel (`params.len()` =
/// `nPCHS`).
///
/// Field order, exactly as Table 5-28 fixes it:
///
/// 1. `SSC = ExtractBits(2)`, `PSC = ExtractBits(3)`;
/// 2. the PMODE plane — 1 bit per `(ch, n)` for `n < nSUBS[ch]`,
///    all channels before any later field;
/// 3. the PVQ plane — `nVQIndex = ExtractBits(12)` for every
///    `(ch, n)` whose PMODE bit is set;
/// 4. the ABITS plane — `BHUFF[ch]`-codebook decode per `(ch, n)`
///    for `n < nVQSUB[ch]`;
/// 5. the TMODE plane — cleared to zero; when `nSSC > 1`,
///    `THUFF[ch]`-codebook decode per `(ch, n)` for `n < nVQSUB[ch]`
///    where `ABITS[ch][n] > 0`. (The staged listing's outer
///    `for (ch=…)` brace placement re-opens an inner channel loop;
///    the field semantics — clear all channels, then one decode pass
///    per channel — follow the field description on PDF p.30 and the
///    "variable bits" sizing of the single decode pass.)
/// 6. the SCALES plane — per channel: `nScaleSum = 0`, then for
///    `n < nVQSUB[ch]` with `ABITS[ch][n] > 0` one `SHUFF[ch]`-
///    codebook decode (plus a second when `TMODE[ch][n] > 0`), then
///    the "High frequency VQ subbands" loop for
///    `n ∈ [nVQSUB[ch], nSUBS[ch])` — one factor per subband, the
///    running `nScaleSum` accumulator carrying across both loops.
///
/// Returns `(PrimarySideInfo, bits_consumed)`; the cursor
/// `bit_offset + bits_consumed` is the first bit of the Table 5-28
/// tail this walker does not cover (`JOIN_SHUFF` onward — see the
/// module docs for the follow-up boundary).
///
/// # Errors
///
/// * [`Error::InvalidSideInfo`] with field `"nPCHS"` when
///   `params.len() > 5` (PDF p.25: `nPCHS ≤ 5`), `"nSUBS"` when a
///   channel's `n_subs` exceeds [`NUM_SUBBAND`](crate::NUM_SUBBAND), or `"VQSUB"` when
///   `n_vqsub > n_subs`;
/// * [`Error::InvalidSideInfo`] with field `"SCALES"` when a
///   scale-factor index walks outside its RMS table (per the
///   round-195 single-field decoder);
/// * [`Error::UnexpectedEof`] when the buffer ends mid-walk;
/// * [`Error::HuffmanDecodeFailed`] on a corrupt Huffman prefix.
pub fn decode_primary_side_info_at(
    bytes: &[u8],
    bit_offset: usize,
    params: &[ChannelSideInfoParams],
) -> Result<(PrimarySideInfo, usize)> {
    // Validate the per-channel loop bounds before touching the bit
    // stream so a bad header surfaces as a typed error, not as a
    // mis-aligned walk.
    if params.len() > MAX_PRIMARY_CHANNELS {
        return Err(Error::InvalidSideInfo {
            field: "nPCHS",
            value: params.len() as u32,
        });
    }
    for p in params {
        if p.n_subs > NUM_SUBBAND {
            return Err(Error::InvalidSideInfo {
                field: "nSUBS",
                value: p.n_subs as u32,
            });
        }
        if p.n_vqsub > p.n_subs {
            return Err(Error::InvalidSideInfo {
                field: "VQSUB",
                value: p.n_vqsub as u32,
            });
        }
    }

    let byte_offset = bit_offset / 8;
    let intra_byte = bit_offset % 8;
    let mut br = BitReader::from_byte_offset(bytes, byte_offset);
    if intra_byte > 0 {
        br.read_bits(intra_byte as u32)?;
    }

    // 1. SSC = ExtractBits(2); nSSC = SSC + 1; PSC = ExtractBits(3).
    let ssc = br.read_bits(2)? as u8;
    let psc = br.read_bits(3)? as u8;
    let subsubframe_count = SubsubframeCount::new(ssc, psc);

    let mut channels: Vec<ChannelSideInfo> = vec![ChannelSideInfo::cleared(); params.len()];

    // 2. PMODE plane:
    //    for (ch=0; ch<nPCHS; ch++)
    //      for (n=0; n<nSUBS[ch]; n++)
    //        PMODE[ch][n] = ExtractBits(1);
    for (ch, p) in params.iter().enumerate() {
        for n in 0..p.n_subs {
            channels[ch].pmode[n] = br.read_bits(1)? as u8;
        }
    }

    // 3. PVQ plane (transmitted only when ADPCM active):
    //    for (ch…) for (n…) if (PMODE[ch][n]>0) {
    //      nVQIndex = ExtractBits(12);
    //      ADPCMCoeffVQ.LookUp(nVQIndex, PVQ[ch][n])  // 4 coefficients
    //    }
    //    The D.10.1 coefficient lookup is deferred; the raw index is
    //    captured (see module docs).
    for (ch, p) in params.iter().enumerate() {
        for n in 0..p.n_subs {
            if channels[ch].pmode[n] > 0 {
                channels[ch].pvq_index[n] = Some(br.read_bits(12)? as u16);
            }
        }
    }

    // 4. ABITS plane:
    //    for (ch…) { nQSelect = BHUFF[ch];
    //      for (n=0; n<nVQSUB[ch]; n++)  // Not for VQ encoded subbands.
    //        QABITS.ppQ[nQSelect]->InverseQ(InputFrame, ABITS[ch][n]) }
    for (ch, p) in params.iter().enumerate() {
        for n in 0..p.n_vqsub {
            channels[ch].abits[n] = decode_abits(&mut br, p.abits_codebook)?;
        }
    }

    // 5. TMODE plane. Already cleared to zero ("TMODE[ch][n] = 0");
    //    decoded only when more than one subsubframe is present:
    //    if (nSSC>1) for (ch…) { nQSelect = THUFF[ch];
    //      for (n=0; n<nVQSUB[ch]; n++)   // No VQ encoded subbands
    //        if (ABITS[ch][n] > 0)        // Present only if bits allocated
    //          QTMODE.ppQ[nQSelect]->InverseQ(InputFrame, TMODE[ch][n]) }
    if subsubframe_count.n_ssc() > 1 {
        for (ch, p) in params.iter().enumerate() {
            for n in 0..p.n_vqsub {
                if channels[ch].abits[n] > 0 {
                    channels[ch].tmode[n] = decode_tmode(&mut br, p.tmode_codebook)?;
                }
            }
        }
    }

    // 6. SCALES plane. Per channel: clear (done), reset the running
    //    accumulator, decode one factor per bit-allocated subband
    //    (two when a transient splits the subframe), then the high-
    //    frequency VQ subband tail — the accumulator carries across
    //    both loops, exactly as Table 5-28's single `nScaleSum`
    //    variable does.
    for (ch, p) in params.iter().enumerate() {
        let mut n_scale_sum: i32 = 0;
        for n in 0..p.n_vqsub {
            if channels[ch].abits[n] > 0 {
                let (scale, sum) = decode_scales(&mut br, p.scales_codebook, n_scale_sum)?;
                n_scale_sum = sum;
                channels[ch].scales[n][0] = scale;
                // Two scale factors transmitted if there is a transient.
                if channels[ch].tmode[n] > 0 {
                    let (scale, sum) = decode_scales(&mut br, p.scales_codebook, n_scale_sum)?;
                    n_scale_sum = sum;
                    channels[ch].scales[n][1] = scale;
                }
            }
        }
        // High frequency VQ subbands: one factor each, no ABITS /
        // TMODE gate (no transient is permitted for VQ subbands).
        for n in p.n_vqsub..p.n_subs {
            let (scale, sum) = decode_scales(&mut br, p.scales_codebook, n_scale_sum)?;
            n_scale_sum = sum;
            channels[ch].scales[n][0] = scale;
        }
    }

    let bits_consumed = br.absolute_bit_position() - bit_offset;
    Ok((
        PrimarySideInfo {
            subsubframe_count,
            channels,
        },
        bits_consumed,
    ))
}

/// The §5.4.1 Table 5-28 side-information **tail** that follows the
/// SCALES block: the optional `RANGE` (dynamic-range) and `SICRC`
/// (side-info CRC) fields.
///
/// Produced by [`decode_primary_side_info_tail_at`]. The cursor it
/// reports lands on the first bit of the §5.5 Audio Data region.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct SideInfoTail {
    /// The 8-bit code of the §5.4.1 `RANGE` dynamic-range field —
    /// `Some` iff the frame header's `DYNF != 0`, `None` otherwise.
    /// The code is **8-bit signed Q2 two's-complement**
    /// (`dB = (int8)code × 0.25`); feed it to
    /// [`crate::dts_dynrng_to_linear`] to obtain the linear multiplier
    /// the §5.4.1 pseudocode applies to every reconstructed PCM sample
    /// **after** QMF synthesis. (Do not use it to raw-index the
    /// offset-binary §D.4 presentation table — see
    /// [`crate::dts_dynrng_to_db`].)
    pub range_index: Option<u8>,
    /// Whether a 16-bit `SICRC` side-info CRC word was present and
    /// skipped (`true` iff the frame header's `CPF == 1`). Per §5.4.1
    /// "the CRC value test shall not be applied", so the word is
    /// consumed for framing but not verified.
    pub side_info_crc_present: bool,
    /// The §5.4.1 `JOIN_SCALES[ch][n]` factors, one entry per primary
    /// channel. `join_scales[ch]` is the joint-intensity scale-factor
    /// vector for the imported sub-band range
    /// `[nSUBS[ch], nSUBS[nSourceCh])` of a jointly-coded destination
    /// channel (`JOINX[ch] > 0`); it is **empty** for channels whose
    /// `JOINX[ch] == 0` (no joint import). The `k`-th entry corresponds
    /// to imported sub-band `nSUBS[ch] + k`. These multiply the
    /// sub-band samples copied from the source channel
    /// (`JOINX[ch] - 1`) during the §C.2.3 joint-subband
    /// reconstruction.
    pub join_scales: Vec<Vec<f64>>,
}

/// Walk the §5.4.1 Table 5-28 side-information **tail** — the
/// `JOIN_SHUFF` / `JOIN_SCALES` / `RANGE` / `SICRC` fields that follow
/// the SCALES block — starting at `bit_offset` (the cursor
/// [`decode_primary_side_info_at`] leaves a caller at).
///
/// Field order, exactly as Table 5-28 fixes it (staged PDF p.29):
///
/// 1. `for (ch) if (JOINX[ch]>0) JOIN_SHUFF[ch] = ExtractBits(3)`;
/// 2. `for (ch) if (JOINX[ch]>0) { … JOIN_SCALES … }` (variable bits);
/// 3. `if (DYNF != 0) { nIndex = ExtractBits(8); RANGEtbl.LookUp(…) }`;
/// 4. `if (CPF == 1) SICRC = ExtractBits(16)`.
///
/// `joinx` is the §5.3.2 per-channel `JOINX` array (length `nPCHS`);
/// `n_subs` is the per-channel `nSUBS[ch]` active-subband count (same
/// length); `dynf` / `cpf` are the §5.3.1 frame-header `DYNF` / `CPF`
/// flags.
///
/// # Joint-intensity walk
///
/// When a channel carries `JOINX[ch] > 0`, this walker decodes the
/// `JOIN_SHUFF[ch]` (3-bit code-book selector) then, for each imported
/// sub-band `n ∈ [nSUBS[ch], nSUBS[nSourceCh])`, one `JOIN_SCALES[ch][n]`
/// via [`crate::decode_join_scale_at`] (the §D.3 `JScaleTbl` look-up of
/// the biased `QSCALES` index). The resolved factors are returned in
/// [`SideInfoTail::join_scales`]; a channel with `JOINX[ch] == 0`
/// contributes an empty vector.
///
/// `nSourceCh = JOINX[ch] - 1` per §5.4.1; the imported range runs from
/// the destination channel's `nSUBS[ch]` up to (but not including) the
/// source channel's `nSUBS[nSourceCh]`. When the source's `nSUBS` does
/// not exceed the destination's, the range is empty and no
/// `JOIN_SCALES` bits are read for that channel.
///
/// The `RANGE` (DYNF) and `SICRC` (CPF) fields are both fully specified
/// and handled: `RANGE`'s 8-bit signed-Q2 code is captured (the
/// [`crate::dts_dynrng_to_linear`] gain is applied later, post-QMF)
/// and `SICRC`'s 16 bits are consumed for framing.
///
/// Returns `(SideInfoTail, bits_consumed)`.
///
/// # Errors
///
/// * [`Error::InvalidSideInfo`] with field `"JOIN_SHUFF"` when a
///   channel's selector is the reserved value 7, or `"JOINX"` when a
///   source channel index is out of range, or `"JOIN_SCALES"` when a
///   biased index lands outside the §D.3 table;
/// * [`Error::UnexpectedEof`] when the buffer ends mid-walk.
pub fn decode_primary_side_info_tail_at(
    bytes: &[u8],
    bit_offset: usize,
    joinx: &[u8],
    n_subs: &[usize],
    dynf: bool,
    cpf: bool,
) -> Result<(SideInfoTail, usize)> {
    let byte_offset = bit_offset / 8;
    let intra_byte = bit_offset % 8;
    let mut br = BitReader::from_byte_offset(bytes, byte_offset);
    if intra_byte > 0 {
        br.read_bits(intra_byte as u32)?;
    }

    let n_pchs = joinx.len();
    let mut join_scales: Vec<Vec<f64>> = vec![Vec::new(); n_pchs];

    // 1. for (ch) if (JOINX[ch] > 0) JOIN_SHUFF[ch] = ExtractBits(3).
    //    The selectors are read for every joint channel first (Table
    //    5-28 groups all JOIN_SHUFF reads before the JOIN_SCALES loop).
    let mut join_codebook: Vec<Option<crate::ScalesCodebook>> = vec![None; n_pchs];
    for (ch, &j) in joinx.iter().enumerate() {
        if j > 0 {
            let shuff = br.read_bits(3)? as u8;
            join_codebook[ch] = Some(crate::ScalesCodebook::from_shuff(shuff).map_err(|_| {
                Error::InvalidSideInfo {
                    field: "JOIN_SHUFF",
                    value: shuff as u32,
                }
            })?);
        }
    }

    // 2. for (ch) if (JOINX[ch] > 0) decode JOIN_SCALES[ch][n] for each
    //    imported sub-band n ∈ [nSUBS[ch], nSUBS[nSourceCh]).
    for (ch, &j) in joinx.iter().enumerate() {
        let Some(codebook) = join_codebook[ch] else {
            continue;
        };
        let source_ch = (j - 1) as usize;
        // The source channel and both nSUBS bounds must exist. A joint
        // reference to a non-existent source channel is a malformed
        // header.
        if source_ch >= n_subs.len() || ch >= n_subs.len() {
            return Err(Error::InvalidSideInfo {
                field: "JOINX",
                value: j as u32,
            });
        }
        let n_subs_dst = n_subs[ch];
        let n_subs_src = n_subs[source_ch];
        // The import range runs [nSUBS[ch], nSUBS[nSourceCh]); when the
        // source is not wider than the destination it is empty.
        if n_subs_src > n_subs_dst {
            let mut factors = Vec::with_capacity(n_subs_src - n_subs_dst);
            for _ in n_subs_dst..n_subs_src {
                let (factor, _biased) = crate::side_info::decode_join_scale(&mut br, codebook)?;
                factors.push(factor);
            }
            join_scales[ch] = factors;
        }
    }

    // 3. if (DYNF != 0) { nIndex = ExtractBits(8); RANGEtbl.LookUp(…) }
    let range_index = if dynf {
        Some(br.read_bits(8)? as u8)
    } else {
        None
    };

    // 4. if (CPF == 1) SICRC = ExtractBits(16). The CRC value test
    //    shall not be applied (§5.4.1); the 16 bits are consumed for
    //    framing only.
    let side_info_crc_present = cpf;
    if cpf {
        let _sicrc = br.read_bits(16)?;
    }

    let bits_consumed = br.absolute_bit_position() - bit_offset;
    Ok((
        SideInfoTail {
            range_index,
            side_info_crc_present,
            join_scales,
        },
        bits_consumed,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::side_info::{RMS_6BIT, RMS_7BIT};

    /// Pack a series of (value, bit_width) fields into a byte stream
    /// MSB-first. Trailing bits are zero-padded.
    fn pack_fields(fields: &[(u32, u8)]) -> Vec<u8> {
        let total_bits: usize = fields.iter().map(|(_, w)| *w as usize).sum();
        let mut out = vec![0u8; total_bits.div_ceil(8)];
        let mut bit_pos = 0usize;
        for &(value, width) in fields {
            for i in (0..width).rev() {
                let bit = ((value >> i) & 1) as u8;
                out[bit_pos / 8] |= bit << (7 - (bit_pos % 8));
                bit_pos += 1;
            }
        }
        out
    }

    fn linear_params(n_subs: usize, n_vqsub: usize) -> ChannelSideInfoParams {
        ChannelSideInfoParams {
            n_subs,
            n_vqsub,
            abits_codebook: AbitsCodebook::Linear5Bit,
            tmode_codebook: TmodeCodebook::D4,
            scales_codebook: ScalesCodebook::Linear6Bit,
        }
    }

    #[test]
    fn single_channel_linear_walk_decodes_every_plane() {
        // 1 channel, nSUBS=4, nVQSUB=2, all-linear codebooks,
        // nSSC=1 (no TMODE plane), no ADPCM.
        let stream = pack_fields(&[
            (0, 2), // SSC = 0 -> nSSC = 1
            (0, 3), // PSC = 0
            (0, 1),
            (0, 1),
            (0, 1),
            (0, 1),  // PMODE[0][0..4] = 0
            (3, 5),  // ABITS[0][0] = 3 (Linear5Bit)
            (0, 5),  // ABITS[0][1] = 0 -> no SCALES for n=1
            (10, 6), // SCALES[0][0][0] index 10 (Linear6Bit)
            (20, 6), // SCALES[0][2][0] index 20 (HF VQ subband)
            (30, 6), // SCALES[0][3][0] index 30 (HF VQ subband)
        ]);
        let params = [linear_params(4, 2)];
        let (info, bits) = decode_primary_side_info_at(&stream, 0, &params).unwrap();

        assert_eq!(info.subsubframe_count.n_ssc(), 1);
        assert_eq!(info.subsubframe_count.psc, 0);
        assert_eq!(info.channels.len(), 1);
        let ch = &info.channels[0];
        assert_eq!(&ch.pmode[..4], &[0, 0, 0, 0]);
        assert!(ch.pvq_index.iter().all(Option::is_none));
        assert_eq!(&ch.abits[..4], &[3, 0, 0, 0]);
        assert!(ch.tmode.iter().all(|&t| t == 0));
        assert_eq!(ch.scales[0], [RMS_6BIT[10], 0]);
        assert_eq!(ch.scales[1], [0, 0]); // ABITS=0 -> skipped
        assert_eq!(ch.scales[2], [RMS_6BIT[20], 0]);
        assert_eq!(ch.scales[3], [RMS_6BIT[30], 0]);
        // 5 prefix + 4 PMODE + 2*5 ABITS + 3*6 SCALES = 37 bits.
        assert_eq!(bits, 37);
    }

    #[test]
    fn pmode_plane_for_all_channels_precedes_pvq_plane() {
        // Two channels; Table 5-28 reads every channel's PMODE bits
        // before any PVQ index. ch0: PMODE = [1, 0]; ch1: PMODE =
        // [0, 1]. The two 12-bit PVQ indices then follow in channel
        // order: 0xABC for ch0/n=0, 0x123 for ch1/n=1.
        let stream = pack_fields(&[
            (0, 2),
            (0, 3), // SSC/PSC
            (0b10, 2),
            (0b01, 2), // PMODE ch0 = [1,0], ch1 = [0,1]
            (0xABC, 12),
            (0x123, 12), // PVQ ch0/n0, ch1/n1
            (0, 5),
            (0, 5),
            (0, 5),
            (0, 5), // ABITS both ch (nVQSUB=2 each) all 0
                    // ABITS all-zero -> no TMODE, no coded SCALES;
                    // nVQSUB == nSUBS -> no HF SCALES either.
        ]);
        let params = [linear_params(2, 2), linear_params(2, 2)];
        let (info, bits) = decode_primary_side_info_at(&stream, 0, &params).unwrap();
        assert_eq!(&info.channels[0].pmode[..2], &[1, 0]);
        assert_eq!(&info.channels[1].pmode[..2], &[0, 1]);
        assert_eq!(info.channels[0].pvq_index[0], Some(0xABC));
        assert_eq!(info.channels[0].pvq_index[1], None);
        assert_eq!(info.channels[1].pvq_index[0], None);
        assert_eq!(info.channels[1].pvq_index[1], Some(0x123));
        // 5 + 4 PMODE + 24 PVQ + 20 ABITS = 53 bits.
        assert_eq!(bits, 53);
    }

    #[test]
    fn tmode_decoded_when_multiple_subsubframes_and_bits_allocated() {
        // nSSC = 2 (SSC=1) so the TMODE plane is present; one
        // channel, nVQSUB = nSUBS = 3, ABITS = [2, 0, 1]: TMODE is
        // decoded for n=0 and n=2 only (n=1 has no bits allocated).
        // D4 codes equal their symbol, 2 bits each. TMODE[0]=1 means
        // a transient -> a second scale factor for n=0.
        let stream = pack_fields(&[
            (1, 2), // SSC = 1 -> nSSC = 2
            (0, 3), // PSC = 0
            (0, 3), // PMODE[0][0..3] = 0
            (2, 5),
            (0, 5),
            (1, 5), // ABITS = [2, 0, 1]
            (1, 2),
            (0, 2),  // TMODE[0]=1 (D4), TMODE[2]=0
            (5, 6),  // SCALES[0][0] index 5 (pre-transient)
            (7, 6),  // SCALES[0][1] index 7 (post-transient)
            (40, 6), // SCALES[2][0] index 40
        ]);
        let params = [linear_params(3, 3)];
        let (info, bits) = decode_primary_side_info_at(&stream, 0, &params).unwrap();
        let ch = &info.channels[0];
        assert_eq!(info.subsubframe_count.n_ssc(), 2);
        assert_eq!(&ch.abits[..3], &[2, 0, 1]);
        assert_eq!(&ch.tmode[..3], &[1, 0, 0]);
        assert_eq!(ch.scales[0], [RMS_6BIT[5], RMS_6BIT[7]]);
        assert_eq!(ch.scales[1], [0, 0]);
        assert_eq!(ch.scales[2], [RMS_6BIT[40], 0]);
        // 5 + 3 PMODE + 15 ABITS + 4 TMODE + 18 SCALES = 45 bits.
        assert_eq!(bits, 45);
    }

    #[test]
    fn tmode_plane_skipped_when_single_subsubframe() {
        // Same geometry as above but SSC = 0 -> nSSC = 1: per Table
        // 5-28 / PDF p.30 the TMODE plane is not transmitted at all,
        // so the SCALES reads start right after ABITS and only one
        // scale factor per subband is read.
        let stream = pack_fields(&[
            (0, 2),
            (0, 3), // SSC = 0 -> nSSC = 1, PSC = 0
            (0, 3), // PMODE
            (2, 5),
            (0, 5),
            (1, 5),  // ABITS = [2, 0, 1]
            (5, 6),  // SCALES[0][0]
            (40, 6), // SCALES[2][0]
        ]);
        let params = [linear_params(3, 3)];
        let (info, bits) = decode_primary_side_info_at(&stream, 0, &params).unwrap();
        let ch = &info.channels[0];
        assert!(ch.tmode.iter().all(|&t| t == 0));
        assert_eq!(ch.scales[0], [RMS_6BIT[5], 0]);
        assert_eq!(ch.scales[2], [RMS_6BIT[40], 0]);
        // 5 + 3 + 15 + 12 = 35 bits.
        assert_eq!(bits, 35);
    }

    #[test]
    fn huffman_walk_accumulates_scale_sum_across_hf_vq_tail() {
        // BHUFF=A12 / THUFF=A4 / SHUFF=SA129 (difference codebook
        // A5 + 6-bit RMS). One channel, nSUBS=3, nVQSUB=2, nSSC=1.
        // A12: symbol 2 = code 10 (2 bits). A5 differences: +2 is
        // code 1110 (4 bits), +1 is code 10 (2 bits), 0 is code 0
        // (1 bit). nScaleSum walks 0 -> +2 -> +3 -> +3 across the
        // coded subband and the two HF VQ subbands — one running
        // accumulator across both loops, per Table 5-28.
        let stream = pack_fields(&[
            (0, 2),
            (0, 3),      // SSC=0, PSC=0
            (0, 3),      // PMODE[0..3] = 0
            (0b10, 2),   // ABITS[0] = 2 (A12)
            (0b10, 2),   // ABITS[1] = 2 (A12)
            (0b1110, 4), // SCALES[0][0]: diff +2 -> sum 2
            (0b10, 2),   // SCALES[1][0]: diff +1 -> sum 3
            (0b0, 1),    // SCALES[2][0] (HF): diff 0 -> sum 3
        ]);
        let params = [ChannelSideInfoParams {
            n_subs: 3,
            n_vqsub: 2,
            abits_codebook: AbitsCodebook::A12,
            tmode_codebook: TmodeCodebook::A4,
            scales_codebook: ScalesCodebook::Sa129,
        }];
        let (info, bits) = decode_primary_side_info_at(&stream, 0, &params).unwrap();
        let ch = &info.channels[0];
        assert_eq!(&ch.abits[..3], &[2, 2, 0]);
        assert_eq!(ch.scales[0][0], RMS_6BIT[2]);
        assert_eq!(ch.scales[1][0], RMS_6BIT[3]);
        assert_eq!(ch.scales[2][0], RMS_6BIT[3]);
        assert_eq!(bits, 5 + 3 + 4 + 7);
    }

    #[test]
    fn scale_sum_accumulator_resets_per_channel() {
        // Two channels on the SA129 difference codebook. ch0
        // accumulates to +3; ch1's first difference (+1) must be
        // applied to a fresh nScaleSum of 0 (per-channel
        // `nScaleSum = 0;` in Table 5-28), not to ch0's +3.
        let stream = pack_fields(&[
            (0, 2),
            (0, 3),
            (0, 1),      // PMODE ch0[0] (nSUBS = 1)
            (0, 2),      // PMODE ch1[0..2] (nSUBS = 2)
            (0b10, 2),   // ABITS ch0[0] = 2 (A12)
            (0b10, 2),   // ABITS ch1[0] = 2 (A12)
            (0b1110, 4), // ch0 SCALES[0][0]: diff +2 -> sum 2
            (0b10, 2),   // ch1 SCALES[0][0]: diff +1 -> fresh sum 1
            (0b10, 2),   // ch1 SCALES[1][0] (HF VQ): diff +1 -> sum 2
        ]);
        // Layout note: ch0 has nSUBS = nVQSUB = 1 so its SCALES block
        // is the single +2 difference (no HF tail); ch1 (nSUBS = 2,
        // nVQSUB = 1) has one coded subband followed by one HF VQ
        // subband.
        let ch0 = ChannelSideInfoParams {
            n_subs: 1,
            n_vqsub: 1,
            abits_codebook: AbitsCodebook::A12,
            tmode_codebook: TmodeCodebook::A4,
            scales_codebook: ScalesCodebook::Sa129,
        };
        let ch1 = ChannelSideInfoParams {
            n_subs: 2,
            n_vqsub: 1,
            ..ch0
        };
        let (info, _) = decode_primary_side_info_at(&stream, 0, &[ch0, ch1]).unwrap();
        // ch0: sum walked 0 -> 2.
        assert_eq!(info.channels[0].scales[0][0], RMS_6BIT[2]);
        // ch1: fresh accumulator 0 -> +1 -> +2 (not 3 -> 4 -> 5).
        assert_eq!(info.channels[1].scales[0][0], RMS_6BIT[1]);
        assert_eq!(info.channels[1].scales[1][0], RMS_6BIT[2]);
    }

    #[test]
    fn linear7_scales_route_through_7bit_rms_table() {
        // SHUFF=6 (Linear7Bit) reads 7-bit absolute indexes and
        // resolves them through the §D.1.2 7-bit RMS table.
        let stream = pack_fields(&[
            (0, 2),
            (0, 3),
            (0, 1),  // PMODE
            (1, 5),  // ABITS[0] = 1 (Linear5Bit)
            (99, 7), // SCALES[0][0] index 99 (7-bit table)
        ]);
        let params = [ChannelSideInfoParams {
            n_subs: 1,
            n_vqsub: 1,
            abits_codebook: AbitsCodebook::Linear5Bit,
            tmode_codebook: TmodeCodebook::D4,
            scales_codebook: ScalesCodebook::Linear7Bit,
        }];
        let (info, bits) = decode_primary_side_info_at(&stream, 0, &params).unwrap();
        assert_eq!(info.channels[0].scales[0][0], RMS_7BIT[99]);
        assert_eq!(bits, 5 + 1 + 5 + 7);
    }

    #[test]
    fn walk_starts_at_arbitrary_bit_offset() {
        // Prepend 3 filler bits; the walk must produce the same
        // result as the aligned variant and report the same
        // bits_consumed.
        let aligned = pack_fields(&[(0, 2), (0, 3), (0, 1), (1, 5), (10, 6)]);
        let shifted = pack_fields(&[
            (0b101, 3), // filler
            (0, 2),
            (0, 3),
            (0, 1),
            (1, 5),
            (10, 6),
        ]);
        let params = [linear_params(1, 1)];
        let (a, bits_a) = decode_primary_side_info_at(&aligned, 0, &params).unwrap();
        let (b, bits_b) = decode_primary_side_info_at(&shifted, 3, &params).unwrap();
        assert_eq!(a, b);
        assert_eq!(bits_a, bits_b);
        assert_eq!(bits_a, 17);
    }

    #[test]
    fn empty_channel_list_consumes_only_the_prefix() {
        let stream = pack_fields(&[(0b10, 2), (0b011, 3)]);
        let (info, bits) = decode_primary_side_info_at(&stream, 0, &[]).unwrap();
        assert_eq!(bits, SubsubframeCount::WIRE_BITS as usize);
        assert!(info.channels.is_empty());
        assert_eq!(info.subsubframe_count.n_ssc(), 3);
        assert_eq!(info.subsubframe_count.psc, 0b011);
    }

    #[test]
    fn too_many_channels_rejected_as_npchs() {
        // PDF p.25: nPCHS = PCHS + 1 <= 5 primary channels.
        let params = vec![linear_params(1, 1); 6];
        assert_eq!(
            decode_primary_side_info_at(&[0u8; 64], 0, &params).unwrap_err(),
            Error::InvalidSideInfo {
                field: "nPCHS",
                value: 6
            }
        );
    }

    #[test]
    fn out_of_range_subband_bounds_rejected() {
        // n_subs beyond NumSubband = 32.
        assert_eq!(
            decode_primary_side_info_at(&[0u8; 64], 0, &[linear_params(33, 1)]).unwrap_err(),
            Error::InvalidSideInfo {
                field: "nSUBS",
                value: 33
            }
        );
        // n_vqsub beyond n_subs.
        assert_eq!(
            decode_primary_side_info_at(&[0u8; 64], 0, &[linear_params(4, 5)]).unwrap_err(),
            Error::InvalidSideInfo {
                field: "VQSUB",
                value: 5
            }
        );
    }

    #[test]
    fn truncated_stream_surfaces_eof_mid_walk() {
        // The single-channel linear walk needs 17 bits; 2 bytes hold
        // only 16.
        let stream = pack_fields(&[
            (0, 2),
            (0, 3),
            (0, 1),
            (1, 5), // ABITS[0] = 1; SCALES read now needs 6 more bits
            (0, 5), // ...but only 5 remain.
        ]);
        assert_eq!(stream.len(), 2);
        assert_eq!(
            decode_primary_side_info_at(&stream, 0, &[linear_params(1, 1)]).unwrap_err(),
            Error::UnexpectedEof
        );
    }

    #[test]
    fn full_subband_count_walk_uses_all_32_slots() {
        // nSUBS = nVQSUB = 32 (the NumSubband maximum), all-linear,
        // every subband bit-allocated: 32 PMODE bits, 32 × 5 ABITS
        // bits, 32 × 6 SCALES bits.
        let mut fields: Vec<(u32, u8)> = vec![(0, 2), (0, 3)];
        fields.extend(std::iter::repeat_n((0u32, 1u8), 32)); // PMODE
        fields.extend(std::iter::repeat_n((1u32, 5u8), 32)); // ABITS = 1
        fields.extend((0..32).map(|n| (n as u32, 6u8))); // SCALES idx n
        let stream = pack_fields(&fields);
        let params = [linear_params(32, 32)];
        let (info, bits) = decode_primary_side_info_at(&stream, 0, &params).unwrap();
        let ch = &info.channels[0];
        assert!(ch.abits.iter().all(|&a| a == 1));
        for (n, scales) in ch.scales.iter().enumerate() {
            assert_eq!(scales[0], RMS_6BIT[n]);
        }
        assert_eq!(bits, 5 + 32 + 160 + 192);
    }

    #[test]
    fn tail_empty_when_no_dynf_no_cpf() {
        // JOINX all zero, DYNF=0, CPF=0 -> the tail is empty.
        let stream = pack_fields(&[(0, 8)]); // any padding
        let (tail, bits) =
            decode_primary_side_info_tail_at(&stream, 0, &[0, 0], &[3, 3], false, false).unwrap();
        assert_eq!(bits, 0);
        assert_eq!(tail.range_index, None);
        assert!(!tail.side_info_crc_present);
        assert!(tail.join_scales.iter().all(Vec::is_empty));
    }

    #[test]
    fn tail_captures_range_index_when_dynf() {
        // DYNF=1 -> 8-bit RANGE code; CPF=0. Signed-Q2 code 80 =
        // +20 dB = 10.0x linear.
        let stream = pack_fields(&[(80, 8)]);
        let (tail, bits) =
            decode_primary_side_info_tail_at(&stream, 0, &[0], &[3], true, false).unwrap();
        assert_eq!(bits, 8);
        assert_eq!(tail.range_index, Some(80));
        assert!(!tail.side_info_crc_present);
        let gain = crate::dts_dynrng_to_linear(tail.range_index.unwrap());
        assert!((gain - 10.0).abs() < 1e-12);
    }

    #[test]
    fn tail_skips_sicrc_when_cpf() {
        // DYNF=0, CPF=1 -> 16-bit SICRC consumed, value not surfaced.
        let stream = pack_fields(&[(0xABCD, 16)]);
        let (tail, bits) =
            decode_primary_side_info_tail_at(&stream, 0, &[0], &[3], false, true).unwrap();
        assert_eq!(bits, 16);
        assert_eq!(tail.range_index, None);
        assert!(tail.side_info_crc_present);
    }

    #[test]
    fn tail_range_then_sicrc_when_both() {
        // DYNF=1 and CPF=1 -> 8-bit RANGE then 16-bit SICRC, in
        // order. Signed-Q2 code 0 -> unity (0 dB).
        let stream = pack_fields(&[(0, 8), (0x1234, 16)]);
        let (tail, bits) =
            decode_primary_side_info_tail_at(&stream, 0, &[0, 0], &[3, 3], true, true).unwrap();
        assert_eq!(bits, 24);
        assert_eq!(tail.range_index, Some(0));
        assert!(tail.side_info_crc_present);
        assert_eq!(crate::dts_dynrng_to_linear(0), 1.0);
    }

    #[test]
    fn tail_decodes_joint_intensity_scales() {
        // ch0 is the source (JOINX=0, nSUBS=5); ch1 is jointly coded
        // (JOINX=1 -> source ch0, nSUBS=3). The imported range is
        // [nSUBS[1]=3, nSUBS[0]=5) = subbands 3 and 4, so ch1 needs a
        // JOIN_SHUFF selector and 2 JOIN_SCALES values.
        //
        // JOIN_SHUFF[1] = 5 (linear-6-bit), then two 6-bit indices:
        // index 0 -> biased 64 -> §D.3 unity (1.0), index 63 -> biased
        // 127 -> §D.3 entry 127.
        let stream = pack_fields(&[
            (5, 3),  // JOIN_SHUFF[1] = linear-6-bit
            (0, 6),  // JOIN_SCALES[1][3] index 0 -> 1.0
            (63, 6), // JOIN_SCALES[1][4] index 63 -> JOIN_SCALE_FACTOR[127]
        ]);
        let (tail, bits) =
            decode_primary_side_info_tail_at(&stream, 0, &[0, 1], &[5, 3], false, false).unwrap();
        assert_eq!(bits, 3 + 6 + 6);
        assert!(tail.join_scales[0].is_empty()); // source channel: no import
        assert_eq!(tail.join_scales[1].len(), 2);
        assert_eq!(tail.join_scales[1][0], 1.0);
        assert_eq!(tail.join_scales[1][1], crate::JOIN_SCALE_FACTOR[127]);
    }

    #[test]
    fn tail_rejects_reserved_join_shuff() {
        // JOIN_SHUFF = 7 is the reserved/invalid selector.
        let stream = pack_fields(&[(7, 3)]);
        assert_eq!(
            decode_primary_side_info_tail_at(&stream, 0, &[0, 1], &[5, 3], false, false)
                .unwrap_err(),
            Error::InvalidSideInfo {
                field: "JOIN_SHUFF",
                value: 7,
            }
        );
    }

    #[test]
    fn tail_joint_empty_range_reads_no_scales() {
        // ch1 JOINX=1 but its nSUBS (5) is not exceeded by the source
        // ch0 nSUBS (5) -> empty import range -> JOIN_SHUFF is still read
        // but no JOIN_SCALES values follow.
        let stream = pack_fields(&[(0, 3)]); // JOIN_SHUFF[1] = SA129
        let (tail, bits) =
            decode_primary_side_info_tail_at(&stream, 0, &[0, 1], &[5, 5], false, false).unwrap();
        assert_eq!(bits, 3);
        assert!(tail.join_scales[1].is_empty());
    }

    #[test]
    fn tail_honours_nonzero_bit_offset() {
        // The tail decoder must respect a non-byte-aligned start cursor
        // (the SCALES block rarely ends on a byte boundary).
        let stream = pack_fields(&[(0b101, 3), (80, 8)]); // 3-bit prefix, then RANGE
        let (tail, bits) =
            decode_primary_side_info_tail_at(&stream, 3, &[0], &[3], true, false).unwrap();
        assert_eq!(bits, 8);
        assert_eq!(tail.range_index, Some(80));
    }
}

//! DTS Coherent Acoustics — §5.3.2 Primary Audio Coding Header
//! (ETSI TS 102 114 V1.3.1, Table 5-21, staged PDF p.24-28).
//!
//! Round 340 (2026-06-19) lands the Table 5-21 header decoder that the
//! round-281 [`crate::decode_primary_side_info_at`] side-info walker
//! and the §5.5 audio-data array walk both need: it produces the
//! per-channel loop bounds and codebook selectors (`nSUBFS`, `nPCHS`,
//! `nSUBS[ch]`, `nVQSUB[ch]`, `JOINX[ch]`, `THUFF`/`SHUFF`/`BHUFF`),
//! the transposed `SEL[ch][n]` quantization-index codebook plane, and
//! the `arADJ[ch][n]` scale-factor adjustment plane that the §5.5
//! `rScale *= arADJ[ch][SEL[ch][nABITS-1]]` step multiplies in.
//!
//! Field order, exactly as Table 5-21 fixes it (PDF p.24-25):
//!
//! ```text
//! SUBFS = ExtractBits(4);  nSUBFS = SUBFS + 1;          // 4 bits
//! PCHS  = ExtractBits(3);  nPCHS  = PCHS  + 1;          // 3 bits
//! for (ch) SUBS[ch]  = ExtractBits(5);  nSUBS[ch]  = SUBS[ch]  + 2;
//! for (ch) VQSUB[ch] = ExtractBits(5);  nVQSUB[ch] = VQSUB[ch] + 1;
//! for (ch) JOINX[ch] = ExtractBits(3);
//! for (ch) THUFF[ch] = ExtractBits(2);
//! for (ch) SHUFF[ch] = ExtractBits(3);
//! for (ch) BHUFF[ch] = ExtractBits(3);
//! // SEL plane, ABITS-major then channel-minor:
//! for (ch)         SEL[ch][0] = ExtractBits(1);   // ABITS 1
//! for (n=1..5)  for (ch) SEL[ch][n] = ExtractBits(2);   // ABITS 2..5
//! for (n=5..10) for (ch) SEL[ch][n] = ExtractBits(3);   // ABITS 6..10
//! for (n=10..26) for (ch) SEL[ch][n] = 0;        // not transmitted
//! // ADJ plane (only where SEL indicates a Huffman code book):
//! for (ch)        if (SEL[ch][0] == 0) arADJ[ch][0] = AdjTable[ExtractBits(2)];
//! for (n=1..5)  for (ch) if (SEL[ch][n] < 3) arADJ[ch][n] = AdjTable[ExtractBits(2)];
//! for (n=5..10) for (ch) if (SEL[ch][n] < 7) arADJ[ch][n] = AdjTable[ExtractBits(2)];
//! if (CPF == 1) AHCRC = ExtractBits(16);          // header CRC, skipped
//! ```
//!
//! The §5.3.2 `SEL[ch][n]` plane is indexed by the *`ABITS` index*
//! `n` (`0..26`), **not** by the subband index. The §5.5 audio-array
//! walk reads `SEL[ch][nABITS-1]` for each subband's `ABITS[ch][n]`
//! value — see [`AudioCodingHeader::sel`] / [`AudioCodingHeader::adj`].
//!
//! # Scope
//!
//! The `AHCRC` Header CRC tail (transmitted only when `CPF == 1`) is
//! skipped: the algorithm is the Annex B CRC-CCITT
//! ([`crate::dts_crc16`], `docs/audio/dts/dts-crc16.md`), but the
//! spec text states "the CRC value test shall not be applied" for the
//! core check words (`HCRC` / `AHCRC` / `SICRC` / `OCRC`) — they are
//! informational placeholders. The 16-bit field is consumed (so the cursor lands
//! at the first §5.4 subframe bit) but not verified. `CPF` is the
//! §5.3.1 frame-header "Predictor History Flag Switch" companion — it
//! is passed in by the caller (the round-202 header carries it as
//! [`crate::DtsFrameHeader::predictor_history`]).

use crate::bitreader::BitReader;
use crate::side_info::{AbitsCodebook, ScaleFactorAdjustment, ScalesCodebook, TmodeCodebook};
use crate::subframe::{ChannelSideInfoParams, MAX_PRIMARY_CHANNELS};
use crate::{Error, Result};

/// Number of `ABITS` indices the §5.3.2 `SEL[ch][n]` / `arADJ[ch][n]`
/// planes tabulate (Table 5-21: `n = 0..26`, i.e. `ABITS = 1..26`).
/// `SEL`/`ADJ` slots for `n >= SEL_PLANE_LEN` are never transmitted
/// (the spec sets them to zero).
pub const SEL_PLANE_LEN: usize = 26;

/// Decoded §5.3.2 Primary Audio Coding Header (Table 5-21).
///
/// Carries the per-channel loop bounds + codebook selectors the §5.4.1
/// side-info walk and §5.5 audio-data array consume, plus the
/// transposed `SEL`/`arADJ` planes indexed by `ABITS` index.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct AudioCodingHeader {
    /// `nSUBFS = SUBFS + 1` — number of audio subframes in the core
    /// frame (Table 5-21 / PDF p.25). Range `1..=16`.
    pub n_subframes: usize,
    /// `nPCHS = PCHS + 1` — number of primary audio channels
    /// (`1..=5`, PDF p.25).
    pub n_pchs: usize,
    /// `JOINX[ch]` — joint-intensity source-channel index (Table 5-22):
    /// `0` = disabled, `>0` = source channel `JOINX[ch]`.
    pub joinx: Vec<u8>,
    /// Per-channel side-info loop bounds + Huffman codebook selectors,
    /// ready to hand to [`crate::decode_primary_side_info_at`].
    pub channel_params: Vec<ChannelSideInfoParams>,
    /// `SEL[ch][n]` — the quantization-index codebook selector for
    /// `ABITS` index `n` (`0..26` ↔ `ABITS 1..26`), per channel. Slots
    /// `n >= SEL_PLANE_LEN` are zero (not transmitted).
    sel: Vec<[u8; SEL_PLANE_LEN]>,
    /// `arADJ[ch][n]` — the §5.5 scale-factor adjustment multiplier for
    /// `ABITS` index `n`, per channel. Defaults to
    /// [`ScaleFactorAdjustment::Adj0`] (unity) where no `ADJ` field was
    /// transmitted (the spec's `arADJ == 1` default).
    adj: Vec<[ScaleFactorAdjustment; SEL_PLANE_LEN]>,
}

impl AudioCodingHeader {
    /// The `SEL[ch][nABITS-1]` quantization-index codebook selector for
    /// a subband whose bit-allocation index is `abits` (`>= 1`). The
    /// §5.5 walk reads `SEL[ch][nABITS-1]`; this accessor takes the
    /// raw `abits` and applies the `-1` ABITS→plane-index shift.
    ///
    /// Returns `0` for `abits == 0` (no bits allocated; no SEL) and for
    /// `abits > SEL_PLANE_LEN` (PDF p.27: "No SEL is transmitted for
    /// `ABITS > 11`", and `ABITS > 11` carries no further encoding).
    #[must_use]
    pub fn sel(&self, ch: usize, abits: u8) -> u8 {
        if abits == 0 || ch >= self.sel.len() {
            return 0;
        }
        let idx = (abits - 1) as usize;
        if idx >= SEL_PLANE_LEN {
            return 0;
        }
        self.sel[ch][idx]
    }

    /// The `arADJ[ch][SEL[ch][nABITS-1]]` scale-factor adjustment for a
    /// subband whose bit-allocation index is `abits` (`>= 1`), keyed —
    /// like [`Self::sel`] — by the `nABITS-1` ABITS→plane index.
    ///
    /// Returns [`ScaleFactorAdjustment::Adj0`] (unity) for `abits == 0`
    /// or out-of-range channels/indices — the §5.5 default of
    /// `arADJ == 1` when no `ADJ` field was read.
    #[must_use]
    pub fn adj(&self, ch: usize, abits: u8) -> ScaleFactorAdjustment {
        if abits == 0 || ch >= self.adj.len() {
            return ScaleFactorAdjustment::Adj0;
        }
        let idx = (abits - 1) as usize;
        if idx >= SEL_PLANE_LEN {
            return ScaleFactorAdjustment::Adj0;
        }
        self.adj[ch][idx]
    }

    /// The per-channel `nSUBS[ch]` loop bound (number of active subbands)
    /// collected from [`Self::channel_params`], in channel order. This
    /// is the slice the §5.5 [`crate::decode_audio_data_subframe_at`]
    /// walk and the §C.2.5 [`crate::MultiChannelQmf`] driver both take.
    #[must_use]
    pub fn n_subs(&self) -> Vec<usize> {
        self.channel_params.iter().map(|p| p.n_subs).collect()
    }

    /// The per-channel `nVQSUB[ch]` loop bound (the highest
    /// non-high-frequency-VQ subband index) collected from
    /// [`Self::channel_params`], in channel order. The §5.5 walk uses it
    /// as its inner-loop bound and to detect the §D.10.2 high-frequency
    /// VQ region (`nVQSUB < nSUBS`).
    #[must_use]
    pub fn n_vqsub(&self) -> Vec<usize> {
        self.channel_params.iter().map(|p| p.n_vqsub).collect()
    }

    /// Test-only constructor for a single-channel [`AudioCodingHeader`]
    /// with a uniform `sel` across the whole SEL plane, used by the
    /// `subframe_pcm` bridge tests that need a header without parsing a
    /// full §5.3.2 bit stream. `n_subs` / `n_vqsub` set the channel's
    /// loop bounds; `joinx` defaults to 0 (no joint coding).
    #[cfg(test)]
    pub(crate) fn single_channel_for_test(n_subs: usize, n_vqsub: usize, sel: u8) -> Self {
        use crate::subframe::ChannelSideInfoParams;
        AudioCodingHeader {
            n_subframes: 1,
            n_pchs: 1,
            joinx: vec![0],
            channel_params: vec![ChannelSideInfoParams {
                n_subs,
                n_vqsub,
                abits_codebook: AbitsCodebook::from_bhuff(0).unwrap(),
                tmode_codebook: TmodeCodebook::from_thuff(0),
                scales_codebook: ScalesCodebook::from_shuff(0).unwrap(),
            }],
            sel: vec![[sel; SEL_PLANE_LEN]],
            adj: vec![[ScaleFactorAdjustment::Adj0; SEL_PLANE_LEN]],
        }
    }

    /// Test-only setter for `joinx[ch]`, used by the `subframe_pcm`
    /// bridge test that checks the joint-intensity decline path.
    #[cfg(test)]
    pub(crate) fn set_joinx_for_test(&mut self, ch: usize, joinx: u8) {
        self.joinx[ch] = joinx;
    }

    /// Test-only constructor for a two-channel header, used by the
    /// `subframe_pcm` bridge tests that exercise the §C.2.3
    /// joint-intensity sub-band copy. Both channels default to
    /// `JOINX == 0`; per-channel loop bounds come from `n_subs` /
    /// `n_vqsub` pairs.
    #[cfg(test)]
    pub(crate) fn two_channel_for_test(ch0: (usize, usize), ch1: (usize, usize), sel: u8) -> Self {
        use crate::subframe::ChannelSideInfoParams;
        let mk = |n_subs: usize, n_vqsub: usize| ChannelSideInfoParams {
            n_subs,
            n_vqsub,
            abits_codebook: AbitsCodebook::from_bhuff(0).unwrap(),
            tmode_codebook: TmodeCodebook::from_thuff(0),
            scales_codebook: ScalesCodebook::from_shuff(0).unwrap(),
        };
        AudioCodingHeader {
            n_subframes: 1,
            n_pchs: 2,
            joinx: vec![0, 0],
            channel_params: vec![mk(ch0.0, ch0.1), mk(ch1.0, ch1.1)],
            sel: vec![[sel; SEL_PLANE_LEN]; 2],
            adj: vec![[ScaleFactorAdjustment::Adj0; SEL_PLANE_LEN]; 2],
        }
    }
}

/// The §5.3.2 `SEL` plane bit width for `ABITS` index `n` (`0..26`),
/// per Table 5-21: `n == 0` (ABITS 1) is 1 bit, `n ∈ 1..5` (ABITS 2-5)
/// is 2 bits, `n ∈ 5..10` (ABITS 6-10) is 3 bits, `n >= 10` is not
/// transmitted (0 bits).
fn sel_bit_width(n: usize) -> u32 {
    match n {
        0 => 1,
        1..=4 => 2,
        5..=9 => 3,
        _ => 0,
    }
}

/// The §5.3.2 `ADJ`-transmitted predicate for `ABITS` index `n`: the
/// `ADJ` field follows `SEL[ch][n]` only when `SEL` indicates a Huffman
/// code book — i.e. `SEL` is *below* the group's terminal (block /
/// NFE) entry. Table 5-21 spells the per-group bound out as:
/// `n == 0` → `SEL == 0`; `n ∈ 1..5` → `SEL < 3`; `n ∈ 5..10` →
/// `SEL < 7`.
fn adj_transmitted(n: usize, sel: u8) -> bool {
    match n {
        0 => sel == 0,
        1..=4 => sel < 3,
        5..=9 => sel < 7,
        _ => false,
    }
}

/// Decode the §5.3.2 Primary Audio Coding Header (Table 5-21) from
/// `bytes` starting at `bit_offset` (MSB-first from `bytes[0]`).
///
/// `cpf` is the §5.3.1 frame-header `CPF` (CRC Present Flag, Table 5-1)
/// bit — when set, a 16-bit `AHCRC` Header CRC trailer is consumed (but
/// not verified; see the module docs). Pass
/// [`crate::DtsFrameHeader::crc_present`], **not** `predictor_history`:
/// `CPF` is the same flag that gates the §5.3.1 `HCRC`, the §5.4.1
/// `SICRC`, and this `AHCRC`.
///
/// Returns `(AudioCodingHeader, bits_consumed)`; the cursor
/// `bit_offset + bits_consumed` is the first bit of the §5.4 subframe
/// region (the round-281 side-info walk's entry point).
///
/// # Errors
///
/// * [`Error::InvalidSideInfo`] with field `"nPCHS"` when
///   `nPCHS > 5`, `"nSUBS"` when a channel's `nSUBS` exceeds
///   [`NUM_SUBBAND`](crate::NUM_SUBBAND), or `"VQSUB"` when
///   `nVQSUB > nSUBS`;
/// * [`Error::InvalidSideInfo`] with field `"BHUFF"` / `"SHUFF"` when a
///   reserved selector value `7` is read;
/// * [`Error::UnexpectedEof`] when the buffer ends mid-walk.
pub fn decode_audio_coding_header_at(
    bytes: &[u8],
    bit_offset: usize,
    cpf: bool,
) -> Result<(AudioCodingHeader, usize)> {
    let byte_offset = bit_offset / 8;
    let intra_byte = bit_offset % 8;
    let mut br = BitReader::from_byte_offset(bytes, byte_offset);
    if intra_byte > 0 {
        br.read_bits(intra_byte as u32)?;
    }

    // SUBFS / PCHS.
    let n_subframes = br.read_bits(4)? as usize + 1;
    let n_pchs = br.read_bits(3)? as usize + 1;
    if n_pchs > MAX_PRIMARY_CHANNELS {
        return Err(Error::InvalidSideInfo {
            field: "nPCHS",
            value: n_pchs as u32,
        });
    }

    // SUBS[ch] -> nSUBS[ch].
    let mut n_subs = vec![0usize; n_pchs];
    for slot in n_subs.iter_mut() {
        let v = br.read_bits(5)? as usize + 2;
        if v > crate::cos_mod::NUM_SUBBAND {
            return Err(Error::InvalidSideInfo {
                field: "nSUBS",
                value: v as u32,
            });
        }
        *slot = v;
    }

    // VQSUB[ch] -> nVQSUB[ch].
    let mut n_vqsub = vec![0usize; n_pchs];
    for (ch, slot) in n_vqsub.iter_mut().enumerate() {
        let v = br.read_bits(5)? as usize + 1;
        if v > n_subs[ch] {
            return Err(Error::InvalidSideInfo {
                field: "VQSUB",
                value: v as u32,
            });
        }
        *slot = v;
    }

    // JOINX[ch].
    let mut joinx = vec![0u8; n_pchs];
    for slot in joinx.iter_mut() {
        *slot = br.read_bits(3)? as u8;
    }

    // THUFF[ch] / SHUFF[ch] / BHUFF[ch].
    let mut thuff = vec![0u8; n_pchs];
    for slot in thuff.iter_mut() {
        *slot = br.read_bits(2)? as u8;
    }
    let mut shuff = vec![0u8; n_pchs];
    for slot in shuff.iter_mut() {
        *slot = br.read_bits(3)? as u8;
    }
    let mut bhuff = vec![0u8; n_pchs];
    for slot in bhuff.iter_mut() {
        *slot = br.read_bits(3)? as u8;
    }

    // SEL[ch][n] plane, ABITS-major then channel-minor (Table 5-21).
    let mut sel = vec![[0u8; SEL_PLANE_LEN]; n_pchs];
    for n in 0..SEL_PLANE_LEN {
        let width = sel_bit_width(n);
        if width == 0 {
            // ABITS >= 11: not transmitted, already zero.
            continue;
        }
        for sel_ch in sel.iter_mut() {
            sel_ch[n] = br.read_bits(width)? as u8;
        }
    }

    // arADJ[ch][n] plane: an ADJ field follows each SEL that indicates
    // a Huffman code book, in the same ABITS-major / channel-minor
    // order. Default is unity (Adj0) everywhere else.
    let mut adj = vec![[ScaleFactorAdjustment::Adj0; SEL_PLANE_LEN]; n_pchs];
    for n in 0..SEL_PLANE_LEN {
        if sel_bit_width(n) == 0 {
            continue;
        }
        for (ch, adj_ch) in adj.iter_mut().enumerate() {
            if adj_transmitted(n, sel[ch][n]) {
                let raw = br.read_bits(2)? as u8;
                adj_ch[n] = ScaleFactorAdjustment::from_index(raw);
            }
        }
    }

    // AHCRC Header CRC trailer — consumed but not verified.
    if cpf {
        br.read_bits(16)?;
    }

    // Resolve the per-channel codebook selectors into the round-195
    // typed selectors; reserved 7 values surface InvalidSideInfo here.
    let mut channel_params = Vec::with_capacity(n_pchs);
    for ch in 0..n_pchs {
        let abits_codebook = AbitsCodebook::from_bhuff(bhuff[ch])?;
        let tmode_codebook = TmodeCodebook::from_thuff(thuff[ch]);
        let scales_codebook = ScalesCodebook::from_shuff(shuff[ch])?;
        channel_params.push(ChannelSideInfoParams {
            n_subs: n_subs[ch],
            n_vqsub: n_vqsub[ch],
            abits_codebook,
            tmode_codebook,
            scales_codebook,
        });
    }

    let bits_consumed = br.absolute_bit_position() - bit_offset;
    Ok((
        AudioCodingHeader {
            n_subframes,
            n_pchs,
            joinx,
            channel_params,
            sel,
            adj,
        },
        bits_consumed,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pack a series of (value, bit_width) fields MSB-first.
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

    /// Build the fixed-width prefix of a one-channel header up to the
    /// SEL plane: SUBFS, PCHS, SUBS, VQSUB, JOINX, THUFF, SHUFF, BHUFF.
    fn one_channel_prefix(
        subfs: u32,
        subs: u32,
        vqsub: u32,
        joinx: u32,
        thuff: u32,
        shuff: u32,
        bhuff: u32,
    ) -> Vec<(u32, u8)> {
        vec![
            (subfs, 4),
            (0, 3), // PCHS = 0 -> nPCHS = 1
            (subs, 5),
            (vqsub, 5),
            (joinx, 3),
            (thuff, 2),
            (shuff, 3),
            (bhuff, 3),
        ]
    }

    #[test]
    fn single_channel_linear_header_decodes_bounds() {
        // SUBFS=0 -> 1 subframe; nSUBS = 2+2 = 4; nVQSUB = 1+1 = 2;
        // JOINX=0; THUFF=3 (D4); SHUFF=5 (6-bit linear); BHUFF=6
        // (Linear5Bit). SEL: ABITS 1 (n=0) 1 bit = 0; ABITS 2-5 four
        // 2-bit = 0; ABITS 6-10 five 3-bit = 0. With every SEL == 0
        // each transmits an ADJ (Huffman-coded path), so 10 ADJ
        // fields of 2 bits follow.
        let mut fields = one_channel_prefix(0, 2, 1, 0, 3, 5, 6);
        // SEL plane: n=0 1 bit, n=1..4 2 bits, n=5..9 3 bits.
        fields.push((0, 1));
        for _ in 1..5 {
            fields.push((0, 2));
        }
        for _ in 5..10 {
            fields.push((0, 3));
        }
        // ADJ plane: SEL==0 everywhere -> every group transmits ADJ.
        for _ in 0..10 {
            fields.push((0, 2));
        }
        let bytes = pack_fields(&fields);
        let (hdr, _) = decode_audio_coding_header_at(&bytes, 0, false).unwrap();
        assert_eq!(hdr.n_subframes, 1);
        assert_eq!(hdr.n_pchs, 1);
        assert_eq!(hdr.channel_params.len(), 1);
        assert_eq!(hdr.channel_params[0].n_subs, 4);
        assert_eq!(hdr.channel_params[0].n_vqsub, 2);
        assert_eq!(hdr.joinx, vec![0]);
        assert_eq!(
            hdr.channel_params[0].abits_codebook,
            AbitsCodebook::Linear5Bit
        );
        assert_eq!(hdr.channel_params[0].tmode_codebook, TmodeCodebook::D4);
        assert_eq!(
            hdr.channel_params[0].scales_codebook,
            ScalesCodebook::Linear6Bit
        );
    }

    #[test]
    fn sel_plane_routes_by_abits_index() {
        // SEL values that distinguish the three width groups: set
        // SEL[0][0]=1 (ABITS 1), SEL[0][1]=2 (ABITS 2), SEL[0][5]=4
        // (ABITS 6). Choose values so no ADJ is transmitted where we
        // want clarity: ABITS-1 SEL=1 (not 0) -> no ADJ; ABITS-2 SEL=2
        // (not <3? 2<3 true) -> ADJ present; ABITS-6 SEL=4 (<7) -> ADJ
        // present. To keep the stream simple, set every other SEL to
        // its terminal so no ADJ follows.
        let mut fields = one_channel_prefix(0, 2, 2, 0, 0, 5, 6);
        // SEL plane.
        fields.push((1, 1)); // n=0 ABITS1: SEL=1 (terminal, no ADJ)
        fields.push((2, 2)); // n=1 ABITS2: SEL=2 (<3 -> ADJ)
        fields.push((3, 2)); // n=2 ABITS3: SEL=3 (terminal, no ADJ)
        fields.push((3, 2)); // n=3 ABITS4: SEL=3 terminal
        fields.push((3, 2)); // n=4 ABITS5: SEL=3 terminal
        fields.push((4, 3)); // n=5 ABITS6: SEL=4 (<7 -> ADJ)
        fields.push((7, 3)); // n=6 ABITS7: SEL=7 terminal
        fields.push((7, 3)); // n=7 ABITS8: SEL=7 terminal
        fields.push((7, 3)); // n=8 ABITS9: SEL=7 terminal
        fields.push((7, 3)); // n=9 ABITS10: SEL=7 terminal
        // ADJ plane: only ABITS2 (SEL 2<3) and ABITS6 (SEL 4<7) emit.
        fields.push((1, 2)); // ABITS2 ADJ index 1 (1.1250)
        fields.push((2, 2)); // ABITS6 ADJ index 2 (1.2500)
        let bytes = pack_fields(&fields);
        let (hdr, _) = decode_audio_coding_header_at(&bytes, 0, false).unwrap();
        // sel() applies the ABITS-1 shift: sel(ch, abits).
        assert_eq!(hdr.sel(0, 1), 1);
        assert_eq!(hdr.sel(0, 2), 2);
        assert_eq!(hdr.sel(0, 6), 4);
        // abits 0 and out-of-range yield SEL 0.
        assert_eq!(hdr.sel(0, 0), 0);
        assert_eq!(hdr.sel(0, 30), 0);
        // ADJ resolved only where transmitted.
        assert_eq!(hdr.adj(0, 2), ScaleFactorAdjustment::from_index(1));
        assert_eq!(hdr.adj(0, 6), ScaleFactorAdjustment::from_index(2));
        // Non-transmitted ADJ defaults to unity.
        assert_eq!(hdr.adj(0, 1), ScaleFactorAdjustment::Adj0);
        assert_eq!(hdr.adj(0, 7), ScaleFactorAdjustment::Adj0);
    }

    #[test]
    fn two_channel_planes_interleave_by_channel() {
        // nPCHS = 2: every per-channel loop reads ch0 then ch1.
        let fields = vec![
            (0, 4), // SUBFS = 0
            (1, 3), // PCHS = 1 -> nPCHS = 2
            (0, 5), // SUBS[0] = 0 -> nSUBS 2
            (3, 5), // SUBS[1] = 3 -> nSUBS 5
            (0, 5), // VQSUB[0] = 0 -> nVQSUB 1
            (1, 5), // VQSUB[1] = 1 -> nVQSUB 2
            (0, 3), // JOINX[0]
            (1, 3), // JOINX[1] = 1 (source ch 1)
            (0, 2), // THUFF[0] = A4
            (1, 2), // THUFF[1] = B4
            (5, 3), // SHUFF[0] = 6-bit linear
            (6, 3), // SHUFF[1] = 7-bit linear
            (6, 3), // BHUFF[0] = Linear5Bit
            (5, 3), // BHUFF[1] = Linear4Bit
            // SEL plane n=0 (1 bit) ch0,ch1 then n=1..4 (2 bit) ...
            (1, 1),
            (1, 1), // SEL[*][0]=1 terminal, no ADJ
        ];
        // Fill remaining SEL groups with terminal values (no ADJ) to
        // keep the stream self-contained: ABITS2-5 SEL=3, ABITS6-10
        // SEL=7, both channels.
        let mut fields = fields;
        for _ in 1..5 {
            fields.push((3, 2));
            fields.push((3, 2));
        }
        for _ in 5..10 {
            fields.push((7, 3));
            fields.push((7, 3));
        }
        let bytes = pack_fields(&fields);
        let (hdr, _) = decode_audio_coding_header_at(&bytes, 0, false).unwrap();
        assert_eq!(hdr.n_pchs, 2);
        assert_eq!(hdr.channel_params[0].n_subs, 2);
        assert_eq!(hdr.channel_params[1].n_subs, 5);
        assert_eq!(hdr.channel_params[0].n_vqsub, 1);
        assert_eq!(hdr.channel_params[1].n_vqsub, 2);
        assert_eq!(hdr.joinx, vec![0, 1]);
        assert_eq!(hdr.channel_params[0].tmode_codebook, TmodeCodebook::A4);
        assert_eq!(hdr.channel_params[1].tmode_codebook, TmodeCodebook::B4);
        assert_eq!(
            hdr.channel_params[0].abits_codebook,
            AbitsCodebook::Linear5Bit
        );
        assert_eq!(
            hdr.channel_params[1].abits_codebook,
            AbitsCodebook::Linear4Bit
        );
    }

    #[test]
    fn reserved_bhuff_seven_rejected() {
        let mut fields = one_channel_prefix(0, 0, 0, 0, 0, 0, 7);
        // pad SEL plane (all terminal, no ADJ) so the walk doesn't EOF
        // before the BHUFF resolve — but the resolve runs after the
        // full bit walk, so just supply enough bytes.
        fields.push((0, 1));
        for _ in 1..5 {
            fields.push((3, 2));
        }
        for _ in 5..10 {
            fields.push((7, 3));
        }
        // ABITS1 SEL=0 -> one ADJ.
        fields.push((0, 2));
        let bytes = pack_fields(&fields);
        assert_eq!(
            decode_audio_coding_header_at(&bytes, 0, false).unwrap_err(),
            Error::InvalidSideInfo {
                field: "BHUFF",
                value: 7
            }
        );
    }

    #[test]
    fn too_many_channels_rejected() {
        // PCHS = 5 -> nPCHS = 6 > 5.
        let fields = vec![(0, 4), (5, 3)];
        let bytes = pack_fields(&fields);
        assert_eq!(
            decode_audio_coding_header_at(&bytes, 0, false).unwrap_err(),
            Error::InvalidSideInfo {
                field: "nPCHS",
                value: 6
            }
        );
    }

    #[test]
    fn cpf_consumes_ahcrc_trailer() {
        // Build a minimal 1-channel header with all-terminal SEL (no
        // ADJ) and CPF set, then confirm 16 extra bits are consumed.
        let mut fields = one_channel_prefix(0, 0, 0, 0, 0, 5, 6);
        fields.push((1, 1)); // ABITS1 SEL=1 terminal
        for _ in 1..5 {
            fields.push((3, 2));
        }
        for _ in 5..10 {
            fields.push((7, 3));
        }
        let without_crc = fields.clone();
        let mut with_crc = fields;
        with_crc.push((0xABCD, 16)); // AHCRC

        let bytes_no = pack_fields(&without_crc);
        let (_, bits_no) = decode_audio_coding_header_at(&bytes_no, 0, false).unwrap();
        let bytes_crc = pack_fields(&with_crc);
        let (_, bits_crc) = decode_audio_coding_header_at(&bytes_crc, 0, true).unwrap();
        assert_eq!(bits_crc, bits_no + 16);
    }

    #[test]
    fn arbitrary_bit_offset_matches_aligned() {
        let mut fields = one_channel_prefix(0, 0, 0, 0, 0, 5, 6);
        fields.push((1, 1));
        for _ in 1..5 {
            fields.push((3, 2));
        }
        for _ in 5..10 {
            fields.push((7, 3));
        }
        let aligned = pack_fields(&fields);
        let mut shifted_fields = vec![(0b101, 3)];
        shifted_fields.extend_from_slice(&fields);
        let shifted = pack_fields(&shifted_fields);
        let (a, bits_a) = decode_audio_coding_header_at(&aligned, 0, false).unwrap();
        let (b, bits_b) = decode_audio_coding_header_at(&shifted, 3, false).unwrap();
        assert_eq!(a, b);
        assert_eq!(bits_a, bits_b);
    }
}

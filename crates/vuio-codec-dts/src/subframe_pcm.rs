//! DTS Coherent Acoustics — §5.5 + §C.2.5 end-to-end subframe→PCM
//! bridge (ETSI TS 102 114 V1.3.1).
//!
//! Round 346 (2026-06-20) composes the two already-landed halves of the
//! Core reconstruction chain into one per-subframe call:
//!
//! 1. the round-340 §5.5 [`crate::decode_audio_data_subframe_at`] walk, which
//!    turns the §5.4.1 side information + the §5.5 `Audio Data` arrays
//!    into the per-channel subband-sample matrices
//!    `aPrmCh[ch].aSubband[n].aSample[m]`, and
//! 2. the round-330 §C.2.5 [`MultiChannelQmf`] driver, which runs the
//!    per-channel `aPrmCh[ch].QMFInterpolation(FILTS, nSUBS[ch])` 32-band
//!    synthesis filterbank over those matrices to produce PCM.
//!
//! The bridge is the missing composition step the crate README's "Not
//! yet implemented" tail named first: *"The §5.5 `Audio Data` walker
//! that composes the side-info, dispatch, dequantization, ADPCM, and QMF
//! primitives into reconstructed subband samples — and thus PCM
//! output."* The walker (#1) and the synthesis (#2) both landed in
//! prior rounds; this module is the one-call subframe driver that wires
//! the walker's output directly into the synthesis input.
//!
//! # The per-subframe loop (§5.4 + §5.5 + §C.2.5)
//!
//! For one audio subframe the spec runs (PDF p.28-33, then the §C.2.5
//! driver per channel):
//!
//! ```text
//! // §5.5 Audio Data: nSSC subsubframes of 8 samples each ->
//! //   aPrmCh[ch].aSubband[n].aSample[0 .. nSSC*8]
//! decode_audio_data_subframe_at(...);
//! // §C.2.5 Filter Bank Reconstruction, once per channel:
//! for (ch=0; ch<nPCHS; ch++)
//!     aPrmCh[ch].QMFInterpolation(FILTS, nSUBS[ch]);
//! ```
//!
//! Each channel's `nSSC*8` per-sample subband rows synthesise to
//! `nSSC*8*32` PCM samples (the §C.2.5 driver emits 32 PCM samples per
//! subband-sample row). A subframe therefore yields `nSSC * 256` PCM
//! samples per channel — except on a **termination frame** (§5.3.1
//! `FTYPE = 0`) whose subframe signals a §5.4.1 partial subsubframe
//! (`PSC > 0`): its last subsubframe carries `PSC < 8` samples per
//! subband, so the subframe yields `((nSSC-1)*8 + PSC) * 32` PCM
//! samples (see [`SubframePcmDecoder::decode_subframe_partial`]).
//!
//! # Persistence across subframes
//!
//! [`SubframePcmDecoder`] owns one persistent [`MultiChannelQmf`] so a
//! caller decoding a frame's subframes (or a stream's frames) in order
//! carries each channel's inter-subframe filter tail (`raX[]` / `raZ[]`)
//! exactly as the §C.2.5 driver requires. Construct it once for the
//! frame's channel count, then call [`SubframePcmDecoder::decode_subframe`]
//! for each subframe.
//!
//! # Scope
//!
//! The walker's §D.10.1 ADPCM-coefficient-VQ (`PMODE != 0`) and §D.10.2
//! high-frequency-VQ (`nVQSUB < nSUBS`) sub-paths are fully implemented
//! (round 434) and — since round 439 — **enabled by default**: every
//! decoder starts with the real §D.10 books
//! ([`VqCodebooks::builtin`], transcribed from the staged clean-room
//! tables `docs/audio/dts/tables/dts-d10-*.csv`), so those frames
//! reconstruct to PCM end-to-end out of the box (phase-1 HF-VQ fill,
//! §C.2.2 prediction with the persistent [`AdpcmHistory`] and the
//! §5.3.1 `HFLAG` frame gate). A caller may still swap or strip the
//! books ([`SubframePcmDecoder::set_vq_codebooks`]); with
//! [`VqCodebooks::none`] such frames surface the typed
//! [`AudioArrayError::VqCodebookUnavailable`] error before any §5.5
//! bit is read, exactly as in the pre-round-439 bookless state.
//!
//! Joint-intensity subband coding (`JOINX[ch] > 0`) is not applied here:
//! the §C.2.3 joint-subband decode is landed
//! ([`crate::joint_subband_decode_range_f64`]) but it needs the
//! `JOIN_SCALES[ch][n]` Huffman factors, whose §5.4.x bit-stream decode
//! is not yet wired. [`SubframePcmDecoder::decode_subframe`] therefore
//! surfaces [`SubframePcmError::JointSubbandUnsupported`] when any
//! channel carries `JOINX[ch] > 0`, rather than silently skipping the
//! joint step.

use crate::audio_array::{
    AdpcmContext, AdpcmHistory, AudioArrayDecodeError, AudioArrayError, HfVqFill,
    SubbandSampleMatrix, decode_audio_data_subframe_vq_at, decode_lfe_phase_at,
};
use crate::audio_header::AudioCodingHeader;
use crate::cos_mod::NUM_SUBBAND;
use crate::d10_vq::{VqCodebooks, scan_hf_vq_indices_at};
use crate::filter_bank::FilterBankSelection;
use crate::header::{AmodeArrangement, DtsFrameHeader};
use crate::qmf_multichannel::{MultiChannelQmf, MultiChannelQmfError};
use crate::step_size::StepSizeTable;
use crate::subframe::{ChannelSideInfo, SideInfoTail};

/// One subframe's reconstructed PCM, planar (one `Vec<i32>` per
/// channel). Every channel's vec has the same length — `nSSC * 256`
/// samples (`nSSC` subsubframes × 8 samples × 32 PCM samples per
/// subband-sample row), or `((nSSC-1)·8 + PSC) · 32` samples when a
/// termination frame's subframe ends in a §5.4.1 partial subsubframe.
pub type SubframePcm = Vec<Vec<i32>>;

/// PCM samples one §C.2.5 subband-sample row expands to (the driver
/// emits 32 PCM samples per row — the `NumSubband` bands of one
/// polyphase output block).
pub const PCM_PER_SUBBAND_ROW: usize = NUM_SUBBAND;

/// Errors from the §5.5 + §C.2.5 end-to-end subframe→PCM bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SubframePcmError {
    /// The §5.5 [`crate::decode_audio_data_subframe_at`] walk failed: a
    /// bit-stream-level error, or an Annex D VQ-codebook blocker
    /// (`PMODE != 0` / `nVQSUB < nSUBS`). Carries the underlying
    /// [`AudioArrayDecodeError`].
    AudioData(AudioArrayDecodeError),
    /// The §C.2.5 [`MultiChannelQmf`] synthesis failed (a length or
    /// row-count mismatch between the walker's matrices and the driver's
    /// channel count, or a per-channel synthesis error).
    Synthesis(MultiChannelQmfError),
    /// A channel carried `JOINX[ch] > 0` (joint-intensity subband
    /// coding). The §C.2.3 joint-subband decode is landed but its
    /// `JOIN_SCALES` Huffman side-info decode is not yet wired, so the
    /// bridge declines rather than producing incorrect PCM. Carries the
    /// 0-based channel index and the one-based `JOINX` source.
    JointSubbandUnsupported {
        /// 0-based destination channel carrying `JOINX > 0`.
        ch: usize,
        /// The one-based `JOINX[ch]` source-channel selector.
        joinx: u8,
    },
    /// The frame header's `PCMR` source-PCM-resolution code is one of
    /// the two reserved values, so the §C.2.5 output `rScale` (the
    /// post-filterbank float→PCM full-scale gain derived from `PCMR`) is
    /// undefined and no PCM can be produced. Carries the raw `PCMR`
    /// index.
    ReservedPcmResolution {
        /// The raw §5.3.1 Table 5-17 `PCMR` index.
        pcmr: u8,
    },
    /// The caller-supplied per-channel side-info / loop-bound slices did
    /// not all agree on the channel count. Carries the channel count the
    /// driver expected (the [`SubframePcmDecoder`]'s configured count)
    /// and the mismatching slice length.
    ChannelCountMismatch {
        /// The driver's configured channel count.
        expected: usize,
        /// The mismatching supplied slice length.
        got: usize,
    },
    /// The §C.2.3 joint-intensity sub-band copy could not be applied
    /// because the supplied `JOIN_SCALES` factors, source-channel index,
    /// or `nSUBS` bounds were structurally inconsistent with the decoded
    /// sub-band matrices. Carries the 0-based destination channel.
    JointSubbandShape {
        /// 0-based destination channel whose joint import failed.
        ch: usize,
    },
    /// A subframe's §5.4.1 `PSC` (Partial Subsubframe Sample Count) was
    /// non-zero but the §5.3.1 frame header's `FTYPE` says **normal**
    /// frame. Per PDF p.30 a partial subsubframe "exists only in a
    /// termination frame", so a normal frame signalling one is
    /// structurally invalid and the decode declines rather than
    /// truncating a normal frame's audio. Carries the 0-based subframe
    /// index and the offending wire `PSC`.
    PartialSubsubframeInNormalFrame {
        /// 0-based subframe index whose side info carried `PSC > 0`.
        subframe: usize,
        /// The offending 3-bit wire `PSC` value (`1..=7`).
        psc: u8,
    },
}

impl core::fmt::Display for SubframePcmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SubframePcmError::AudioData(e) => write!(f, "audio-data walk failed: {e}"),
            SubframePcmError::Synthesis(e) => write!(f, "QMF synthesis failed: {e}"),
            SubframePcmError::JointSubbandUnsupported { ch, joinx } => write!(
                f,
                "channel {ch} carries JOINX={joinx} (joint-intensity subband \
                 coding); the JOIN_SCALES side-info decode is not yet wired"
            ),
            SubframePcmError::ReservedPcmResolution { pcmr } => write!(
                f,
                "frame header PCMR index {pcmr} is reserved; the output rScale \
                 is undefined"
            ),
            SubframePcmError::ChannelCountMismatch { expected, got } => write!(
                f,
                "channel-count mismatch: driver expects {expected}, slice carries {got}"
            ),
            SubframePcmError::JointSubbandShape { ch } => write!(
                f,
                "channel {ch} joint-intensity sub-band copy failed: JOIN_SCALES / \
                 nSUBS bounds inconsistent with the decoded sub-band matrices"
            ),
            SubframePcmError::PartialSubsubframeInNormalFrame { subframe, psc } => write!(
                f,
                "subframe {subframe} signals a partial subsubframe (PSC={psc}) \
                 but the frame header's FTYPE is normal; a partial subsubframe \
                 exists only in a termination frame (§5.4.1, PDF p.30)"
            ),
        }
    }
}

impl std::error::Error for SubframePcmError {}

impl From<AudioArrayDecodeError> for SubframePcmError {
    fn from(e: AudioArrayDecodeError) -> Self {
        SubframePcmError::AudioData(e)
    }
}

impl From<MultiChannelQmfError> for SubframePcmError {
    fn from(e: MultiChannelQmfError) -> Self {
        SubframePcmError::Synthesis(e)
    }
}

/// Persistent per-frame §5.5 + §C.2.5 subframe→PCM decoder.
///
/// Owns one [`MultiChannelQmf`] for the frame's channel count, so the
/// per-channel filter state (`raX[]` / `raZ[]`) carries across
/// subframes (and across frames if the same decoder instance is reused
/// for a stream). Construct once with the channel count from the frame
/// header, then call [`SubframePcmDecoder::decode_subframe`] for each of
/// the `nSUBFS` subframes the §5.3.2 header declares.
#[derive(Debug, Clone)]
pub struct SubframePcmDecoder {
    qmf: MultiChannelQmf,
    /// The persistent §5.5/§C.2.6 LFE channel (the `LFECh` filter
    /// object). Drives the §5.5 LFE phase (§2.2) when the frame header's
    /// `LFF` is non-zero, carrying the §C.2.6 inter-subframe
    /// interpolation history. Idle (and never read from the bitstream)
    /// for LFF-absent frames.
    lfe: crate::LfeChannel,
    /// The LFE PCM decoded from the most recent [`Self::decode_subframe`]
    /// call (empty when the frame had no LFE channel). Surfaced via
    /// [`Self::take_last_lfe_pcm`] so the primary-channel return tuple is
    /// unchanged.
    last_lfe_pcm: Vec<i32>,
    /// The §D.10 VQ code books. Default: the built-in real books
    /// ([`VqCodebooks::builtin`]); a caller may swap or strip them
    /// ([`Self::set_vq_codebooks`]).
    vq_codebooks: VqCodebooks,
    /// The persistent §C.2.2 per-subband reconstruction history that
    /// primes the inverse-ADPCM predictor across subframe (and, per
    /// the §5.3.1 `HFLAG` gate, frame) boundaries.
    adpcm_history: AdpcmHistory,
}

impl SubframePcmDecoder {
    /// Construct a decoder for `channels` primary audio channels — the
    /// §5.3.2 `nPCHS` (e.g. [`AudioCodingHeader::n_pchs`]). Each
    /// channel's §C.2.5 filter starts with cleared history.
    #[must_use]
    pub fn new(channels: usize) -> Self {
        Self {
            qmf: MultiChannelQmf::new(channels),
            lfe: crate::LfeChannel::new(),
            last_lfe_pcm: Vec::new(),
            vq_codebooks: VqCodebooks::builtin(),
            adpcm_history: AdpcmHistory::new(channels),
        }
    }

    /// Replace the §D.10 VQ code books ([`VqCodebooks`]). The decoder
    /// starts with the built-in real books ([`VqCodebooks::builtin`]),
    /// so the high-frequency-VQ (`nVQSUB < nSUBS`) and inverse-ADPCM
    /// (`PMODE != 0`) §5.5 sub-paths decode by default; supplying
    /// [`VqCodebooks::none`] strips them, restoring the typed
    /// [`AudioArrayError::VqCodebookUnavailable`] blocker on those
    /// sub-paths.
    pub fn set_vq_codebooks(&mut self, books: VqCodebooks) {
        self.vq_codebooks = books;
    }

    /// The currently attached §D.10 books (default: the built-in real
    /// books).
    #[must_use]
    pub fn vq_codebooks(&self) -> &VqCodebooks {
        &self.vq_codebooks
    }

    /// Borrow the persistent §C.2.2 reconstruction history.
    #[must_use]
    pub fn adpcm_history(&self) -> &AdpcmHistory {
        &self.adpcm_history
    }

    /// Zero the §C.2.2 reconstruction history — the §5.3.1
    /// `HFLAG = 0` entry-point state ("these frames can be coded
    /// without the previous frame predictor history … Otherwise, the
    /// history will be ignored"). The frame-level walks
    /// ([`decode_core_frame`] / [`CoreStreamDecoder::decode_frame`])
    /// apply this automatically from the frame header; it is public
    /// for callers driving the per-subframe API directly.
    pub fn reset_adpcm_history(&mut self) {
        self.adpcm_history.clear();
    }

    /// Take the LFE PCM decoded by the most recent
    /// [`Self::decode_subframe`] / [`Self::decode_frame`] call, leaving
    /// the decoder's buffer empty. Returns an empty `Vec` when the last
    /// frame carried no LFE channel (`LFF == 0`). The samples are the
    /// §5.5 LFE phase (§2.2) output: `2·LFF·nSSC·(64 | 128)` interpolated
    /// PCM samples per decoded subframe.
    #[must_use]
    pub fn take_last_lfe_pcm(&mut self) -> Vec<i32> {
        core::mem::take(&mut self.last_lfe_pcm)
    }

    /// The configured channel count (`nPCHS`).
    #[must_use]
    pub fn channel_count(&self) -> usize {
        self.qmf.channel_count()
    }

    /// Borrow the persistent §C.2.5 driver (e.g. to inspect a channel's
    /// inter-subframe filter tail).
    #[must_use]
    pub fn qmf(&self) -> &MultiChannelQmf {
        &self.qmf
    }

    /// Decode one §5.4/§5.5 audio subframe to planar PCM, end to end.
    ///
    /// Runs the §5.5 [`crate::decode_audio_data_subframe_at`] walk to get the
    /// per-channel subband-sample matrices, then the §C.2.5
    /// [`MultiChannelQmf`] synthesis to turn them into PCM. The
    /// per-channel filter state persists into the next call.
    ///
    /// * `bytes` / `bit_offset` — the bit stream positioned at the first
    ///   §5.5 `Audio Data` bit of this subframe (after the subframe's
    ///   §5.4.1 side information).
    /// * `header` — the parsed §5.3.1 [`DtsFrameHeader`]; supplies the
    ///   frame-wide `FILTS` ([`DtsFrameHeader::filter_bank_selection`])
    ///   and the output `rScale` ([`DtsFrameHeader::output_r_scale`]).
    /// * `coding` — the §5.3.2 [`AudioCodingHeader`]; supplies the
    ///   `SEL` / `arADJ` planes and the per-channel `nSUBS` / `nVQSUB`
    ///   loop bounds and `JOINX`.
    /// * `side` — the per-channel decoded §5.4.1 [`ChannelSideInfo`].
    /// * `n_ssc` — this subframe's subsubframe count (`SSC + 1`).
    /// * `aspf` — the §5.3.1 Audio Sync-Word Insertion Flag.
    ///
    /// Returns `(SubframePcm, bits_consumed)`: planar PCM (one
    /// `Vec<i32>` per channel, `n_ssc * 256` samples each) plus the
    /// number of §5.5 bits the audio-data walk consumed (so the caller
    /// can advance to the next subframe).
    ///
    /// # Errors
    ///
    /// * [`SubframePcmError::ChannelCountMismatch`] if `side`'s length
    ///   differs from the configured channel count;
    /// * [`SubframePcmError::JointSubbandUnsupported`] if any channel
    ///   carries `JOINX[ch] > 0`;
    /// * [`SubframePcmError::ReservedPcmResolution`] if the header's
    ///   `PCMR` code is reserved;
    /// * [`SubframePcmError::AudioData`] for any §5.5 walk failure
    ///   (including the §D.10 VQ blockers);
    /// * [`SubframePcmError::Synthesis`] for any §C.2.5 driver failure.
    #[allow(clippy::too_many_arguments)]
    pub fn decode_subframe(
        &mut self,
        bytes: &[u8],
        bit_offset: usize,
        header: &DtsFrameHeader,
        coding: &AudioCodingHeader,
        side: &[ChannelSideInfo],
        n_ssc: usize,
        aspf: bool,
    ) -> Result<(SubframePcm, usize), SubframePcmError> {
        self.decode_subframe_with_joint(bytes, bit_offset, header, coding, side, n_ssc, aspf, &[])
    }

    /// Like [`Self::decode_subframe`] but also applies the §C.2.3
    /// joint-intensity sub-band copy when `join_scales` is non-empty.
    ///
    /// `join_scales[ch]` is the [`crate::SideInfoTail::join_scales`]
    /// vector for destination channel `ch` (empty for channels whose
    /// `JOINX[ch] == 0`). After the §5.5 audio-data walk fills every
    /// channel's sub-band matrix, each jointly-coded channel imports
    /// sub-bands `[nSUBS[ch], nSUBS[nSourceCh])` from its source channel
    /// (`JOINX[ch] - 1`), each scaled by the matching `JOIN_SCALES`
    /// factor, **before** the §C.2.5 QMF synthesis runs — and the QMF's
    /// per-channel active-subband count is widened to the source
    /// channel's `nSUBS` for those channels, per the §C.2.5 driving-call
    /// note ("For joint intensity coded subbands, it must be set to that
    /// of the source channel, in order to reflect the true subband
    /// activity"), so the imported sub-bands actually reach the output.
    #[allow(clippy::too_many_arguments)]
    pub fn decode_subframe_with_joint(
        &mut self,
        bytes: &[u8],
        bit_offset: usize,
        header: &DtsFrameHeader,
        coding: &AudioCodingHeader,
        side: &[ChannelSideInfo],
        n_ssc: usize,
        aspf: bool,
        join_scales: &[Vec<f64>],
    ) -> Result<(SubframePcm, usize), SubframePcmError> {
        self.decode_subframe_partial(
            bytes,
            bit_offset,
            header,
            coding,
            side,
            n_ssc,
            0,
            aspf,
            join_scales,
        )
    }

    /// Like [`Self::decode_subframe_with_joint`] but with the §5.4.1
    /// `PSC` (Partial Subsubframe Sample Count) of a **termination
    /// frame** applied: when `psc ∈ 1..=7`, the last of this
    /// subframe's `n_ssc` subsubframes is *partial* — it carries `psc`
    /// subband samples per active subband instead of 8, so the
    /// subframe reconstructs to `((n_ssc - 1) * 8 + psc) * 32` PCM
    /// samples per channel and the §5.5 bit budget shrinks exactly by
    /// the untransmitted samples (see
    /// [`crate::decode_audio_data_subframe_partial_at`] for the
    /// spec-clause derivation). `psc = 0` is the normal-frame case and
    /// reproduces [`Self::decode_subframe_with_joint`] verbatim.
    ///
    /// The spec ties `PSC > 0` to termination frames only ("It exists
    /// only in a termination frame", PDF p.30); this per-subframe
    /// entry point trusts the caller on that frame-level gate (the
    /// frame walk [`decode_core_frame`] enforces it, surfacing
    /// [`SubframePcmError::PartialSubsubframeInNormalFrame`]).
    ///
    /// When the frame carries an LFE channel, the §5.5 LFE phase is
    /// extracted and interpolated at its spec-literal size (Table
    /// 5-29: `2·LFF·nSSC` decimated samples — the count has no `PSC`
    /// term, so the LFE always covers whole subsubframes) and the
    /// interpolated plane is then truncated to the primary channels'
    /// PCM length, keeping every output plane aligned on the valid
    /// prefix of the terminated subframe.
    ///
    /// # Errors
    ///
    /// See [`Self::decode_subframe_with_joint`].
    #[allow(clippy::too_many_arguments)]
    pub fn decode_subframe_partial(
        &mut self,
        bytes: &[u8],
        bit_offset: usize,
        header: &DtsFrameHeader,
        coding: &AudioCodingHeader,
        side: &[ChannelSideInfo],
        n_ssc: usize,
        psc: u8,
        aspf: bool,
        join_scales: &[Vec<f64>],
    ) -> Result<(SubframePcm, usize), SubframePcmError> {
        let channels = self.qmf.channel_count();
        if side.len() != channels {
            return Err(SubframePcmError::ChannelCountMismatch {
                expected: channels,
                got: side.len(),
            });
        }
        if coding.n_pchs != channels {
            return Err(SubframePcmError::ChannelCountMismatch {
                expected: channels,
                got: coding.n_pchs,
            });
        }

        // The §C.2.5 output rScale must be defined (PCMR not reserved)
        // before any decode work runs, so a reserved-PCMR frame fails
        // cleanly without disturbing the persistent filter state.
        let Some(r_scale) = header.output_r_scale() else {
            return Err(SubframePcmError::ReservedPcmResolution {
                pcmr: header.source_pcm_resolution_index,
            });
        };
        let filter: FilterBankSelection = header.filter_bank_selection();

        // Joint-intensity subband coding (JOINX > 0) is applied below
        // once the §5.5 walk has filled every channel's sub-band matrix,
        // but only when the caller supplied the JOIN_SCALES factors (via
        // decode_subframe_with_joint). A JOINX > 0 channel with no
        // supplied factors is declined rather than silently dropped.
        if join_scales.is_empty() {
            for (ch, &joinx) in coding.joinx.iter().enumerate().take(channels) {
                if joinx > 0 {
                    return Err(SubframePcmError::JointSubbandUnsupported { ch, joinx });
                }
            }
        }

        // Per-channel loop bounds for the §5.5 walk and the §C.2.5
        // driver come straight off the §5.3.2 header.
        let n_subs = coding.n_subs();
        let n_vqsub = coding.n_vqsub();

        let table = StepSizeTable::for_rate(header.rate_index);

        // Subband-sample rows this subframe reconstructs: the last
        // subsubframe is partial (psc rows) on a termination-frame
        // subframe, full (8 rows) otherwise.
        let rows = if psc > 0 {
            (n_ssc - 1) * 8 + usize::from(psc)
        } else {
            n_ssc * 8
        };

        // (0a) §D.10 book-availability gates, checked BEFORE any bit
        // is read so a blocked frame fails cleanly without disturbing
        // the persistent LFE / filter / history state. Both books are
        // present by default (`VqCodebooks::builtin`); the gates fire
        // only when a caller stripped them (`VqCodebooks::none`).
        let has_hf_vq = (0..channels).any(|ch| n_vqsub[ch] < n_subs[ch]);
        if has_hf_vq && self.vq_codebooks.hfreq.is_none() {
            let ch = (0..channels)
                .find(|&ch| n_vqsub[ch] < n_subs[ch])
                .unwrap_or(0);
            return Err(SubframePcmError::AudioData(
                AudioArrayError::VqCodebookUnavailable {
                    ch,
                    n: n_vqsub[ch],
                    high_frequency_vq: true,
                }
                .into(),
            ));
        }
        if self.vq_codebooks.adpcm.is_none() {
            for (ch, ch_side) in side.iter().enumerate() {
                if let Some(n) = ch_side.pmode[..n_vqsub[ch]].iter().position(|&p| p != 0) {
                    return Err(SubframePcmError::AudioData(
                        AudioArrayError::VqCodebookUnavailable {
                            ch,
                            n,
                            high_frequency_vq: false,
                        }
                        .into(),
                    ));
                }
            }
        }

        // (0b) §5.5 phase 1 — high-frequency VQ subbands: the 10-bit
        // `nVQIndex` fields precede the LFE phase (Table 5-29). Empty
        // for the common Core case (`nVQSUB == nSUBS` everywhere).
        let mut cursor = bit_offset;
        let hf_indices: Option<Vec<Vec<u16>>> = if has_hf_vq {
            let (indices, hf_bits) = scan_hf_vq_indices_at(bytes, cursor, &n_vqsub, &n_subs)
                .map_err(|e| SubframePcmError::AudioData(e.into()))?;
            cursor += hf_bits;
            Some(indices)
        } else {
            None
        };

        // (0c) §5.5 LFE phase (§2.2): present only when the header's
        // `LFF` is non-zero; it follows the phase-1 HF-VQ region and
        // precedes the audio-data phase. Its bits count toward the
        // subframe's total so the caller advances correctly. The Table
        // 5-29 sample count (`2·LFF·nSSC`) has no PSC term — the LFE
        // plane always covers whole subsubframes — so on a partial
        // (termination) subframe the interpolated plane is truncated
        // below to the primary channels' PCM length.
        let lff = header.lfe.code();
        if lff != 0 {
            let (mut lfe_pcm, lfe_bits) =
                decode_lfe_phase_at(bytes, cursor, lff, n_ssc, &mut self.lfe)?;
            lfe_pcm.truncate(rows * PCM_PER_SUBBAND_ROW);
            self.last_lfe_pcm = lfe_pcm;
            cursor += lfe_bits;
        } else {
            self.last_lfe_pcm = Vec::new();
        }

        // (1) §5.5 Audio Data -> per-channel subband-sample matrices
        // (`rows` per channel; the §5.4.1 PSC truncation of the last
        // subsubframe is applied inside the walk, bit-exactly), with
        // the recovered-book sub-paths enabled where supplied: the
        // phase-1 HF-VQ fill and the §C.2.2 inverse-ADPCM prediction
        // (whose reconstruction history persists across subframes; the
        // §5.3.1 HFLAG frame gate is applied by the frame-level walk).
        let hf_fill = match (&self.vq_codebooks.hfreq, &hf_indices) {
            (Some(book), Some(indices)) => Some(HfVqFill {
                book: book.as_ref(),
                indices: indices.as_slice(),
            }),
            _ => None,
        };
        let adpcm_ctx = self.vq_codebooks.adpcm.as_ref().map(|book| AdpcmContext {
            book: book.as_ref(),
            history: &mut self.adpcm_history,
        });
        let (matrices, audio_bits): (Vec<SubbandSampleMatrix>, usize) =
            decode_audio_data_subframe_vq_at(
                bytes,
                cursor,
                side,
                |ch, abits| coding.sel(ch, abits),
                |ch, abits| coding.adj(ch, abits),
                &n_vqsub,
                &n_subs,
                n_ssc,
                psc,
                table,
                aspf,
                hf_fill,
                adpcm_ctx,
            )?;
        let bits_consumed = (cursor - bit_offset) + audio_bits;

        // (1b) §C.2.3 joint-intensity sub-band copy. For each
        // destination channel with JOINX[ch] > 0 and supplied
        // JOIN_SCALES factors, overwrite its imported sub-band columns
        // [nSUBS[ch], nSUBS[src]) with the source channel's sub-band
        // samples scaled by the matching JOIN_SCALES factor. This runs
        // on the sub-band matrices before QMF synthesis.
        let mut matrices = matrices;
        if !join_scales.is_empty() {
            apply_joint_subband(&mut matrices, coding, &n_subs, join_scales)?;
        }

        // (1b') Effective per-channel active-subband counts after the
        // joint import. The §C.2.5 driving-call comment is explicit
        // (staged PDF p.184): "nSUBS[ch] indicates the number of active
        // subbands. Subbands above it are all zeros. For joint intensity
        // coded subbands, it must be set to that of the source channel,
        // in order to reflect the true subband activity." A jointly-
        // coded destination channel therefore synthesizes (and, below,
        // sum/difference-matrixes) over the source channel's count —
        // otherwise the §C.2.3 import into [nSUBS[ch], nSUBS[src]) would
        // be zero-filled away by the QMF's inactive-subband clear. The
        // degenerate empty-range joint (source not wider than the
        // destination) keeps the destination's own count.
        let mut eff_n_subs = n_subs.clone();
        for (ch, &joinx) in coding.joinx.iter().enumerate().take(channels) {
            if let Some(src) = crate::joint_source_channel(joinx) {
                let src = usize::from(src);
                if src < n_subs.len() && n_subs[src] > eff_n_subs[ch] {
                    eff_n_subs[ch] = n_subs[src];
                }
            }
        }

        // (1c) §C.2.4 sum/difference decoding. When the front-sum flag
        // (`SUMF`) is set — or unconditionally for AMODE == 3, per the
        // spec's "This decoding is also required when AMODE = 3" — the
        // front L/R channels are stored as (L+R, L-R) and must be matrixed
        // back on the reconstructed sub-band samples (all active subbands,
        // all sub-subframe samples) before QMF synthesis. `SUMS` does the
        // same for the surround L/R pair. This runs after §C.2.3 joint
        // subband and before §C.2.5, matching the Annex C ordering.
        let arrangement = header.amode_arrangement();
        let apply_front =
            header.front_sum || matches!(arrangement, AmodeArrangement::SumDifference);
        if apply_front && let Some((l, r)) = arrangement.front_lr_channels() {
            apply_sum_difference(&mut matrices, l, r, &eff_n_subs)?;
        }
        if header.surround_sum
            && let Some((l, r)) = arrangement.surround_lr_channels()
        {
            apply_sum_difference(&mut matrices, l, r, &eff_n_subs)?;
        }

        // (2) §C.2.5 per-channel 32-band synthesis -> planar PCM.
        let channel_samples: Vec<&[[f64; NUM_SUBBAND]]> =
            matrices.iter().map(|m| m.as_slice()).collect();
        let mut pcm: SubframePcm = vec![Vec::new(); channels];
        self.qmf
            .synthesize_planar(&channel_samples, &eff_n_subs, filter, r_scale, &mut pcm)?;

        Ok((pcm, bits_consumed))
    }

    /// Decode all `nSUBFS` subframes of one core frame to a single block
    /// of planar PCM, appending each subframe's output (in order) onto
    /// the per-channel vectors so the persistent §C.2.5 filter tail
    /// carries across subframe boundaries (§5.3.2 `nSUBFS`; §C.2.5
    /// per-channel filter continuity).
    ///
    /// `bytes` is the frame's bit-stream buffer; `first_audio_bit` is the
    /// bit offset of the **first** subframe's §5.5 `Audio Data` region
    /// (the cursor a caller is left at after the first subframe's §5.4.1
    /// side info). Each [`Subframe`] supplies that subframe's already-
    /// decoded §5.4.1 [`ChannelSideInfo`], its `n_ssc`, and the byte gap
    /// (`side_info_bits`) the caller must skip between this subframe's
    /// §5.5 region and the next subframe's §5.5 region — i.e. the bits of
    /// the *next* subframe's side info, which this driver does not itself
    /// decode (that §5.4.x region — `JOIN_SHUFF` onward — is not yet
    /// transcribed). The last subframe's `side_info_bits` is ignored.
    ///
    /// Returns the concatenated planar PCM (one `Vec<i32>` per channel,
    /// `Σ nSSC · 256` samples each) plus the total bits consumed from
    /// `first_audio_bit`. This driver assumes whole subsubframes
    /// (`PSC = 0` in every supplied subframe); for a termination
    /// frame's partial subsubframe use
    /// [`Self::decode_subframe_partial`] per subframe or the
    /// header-driven [`decode_core_frame`] walk, which reads each
    /// subframe's own `SSC`/`PSC` prefix.
    ///
    /// # Errors
    ///
    /// The same errors as [`SubframePcmDecoder::decode_subframe`], plus
    /// [`SubframePcmError::ChannelCountMismatch`] if a subframe's
    /// side-info channel count disagrees with the driver. A failure on
    /// the *k*-th subframe leaves the PCM from subframes `0..k` already
    /// appended (the §C.2.5 filter state is likewise advanced through
    /// `k-1`); callers that need all-or-nothing semantics should clone
    /// the decoder first.
    pub fn decode_frame(
        &mut self,
        bytes: &[u8],
        first_audio_bit: usize,
        header: &DtsFrameHeader,
        coding: &AudioCodingHeader,
        subframes: &[Subframe<'_>],
        aspf: bool,
    ) -> Result<(SubframePcm, usize), SubframePcmError> {
        let channels = self.qmf.channel_count();
        let mut pcm: SubframePcm = vec![Vec::new(); channels];
        let mut bit = first_audio_bit;
        // Accumulate the per-subframe LFE PCM across the whole frame so
        // take_last_lfe_pcm() returns the frame's full LFE output (each
        // decode_subframe call leaves only its own subframe's LFE PCM).
        let mut frame_lfe: Vec<i32> = Vec::new();

        for (k, sf) in subframes.iter().enumerate() {
            let (block, audio_bits) =
                self.decode_subframe(bytes, bit, header, coding, sf.side, sf.n_ssc, aspf)?;
            frame_lfe.append(&mut self.last_lfe_pcm);
            for (ch, samples) in block.into_iter().enumerate() {
                pcm[ch].extend(samples);
            }
            bit += audio_bits;
            // Skip the next subframe's §5.4.1 side-info region (the bits
            // the caller pre-measured); the last subframe has no
            // successor side info to skip.
            if k + 1 < subframes.len() {
                bit += sf.side_info_bits;
            }
        }

        self.last_lfe_pcm = frame_lfe;
        Ok((pcm, bit - first_audio_bit))
    }
}

/// One subframe's already-decoded inputs for
/// [`SubframePcmDecoder::decode_frame`].
///
/// The driver decodes each subframe's §5.5 `Audio Data` region from the
/// shared bit-stream buffer; this struct carries the per-subframe §5.4.1
/// side information the audio-data walk needs plus the framing offsets
/// the driver uses to step from one subframe's §5.5 region to the next.
#[derive(Debug, Clone, Copy)]
pub struct Subframe<'a> {
    /// This subframe's decoded §5.4.1 per-channel side information (the
    /// round-281 [`crate::decode_primary_side_info_at`] output).
    pub side: &'a [ChannelSideInfo],
    /// This subframe's subsubframe count `nSSC = SSC + 1` (§5.4.1).
    pub n_ssc: usize,
    /// The bit length of the **next** subframe's §5.4.1 side-info region
    /// — the gap the driver skips after this subframe's §5.5 region to
    /// reach the next subframe's §5.5 region. Ignored for the last
    /// subframe of the frame.
    pub side_info_bits: usize,
}

/// Why a Core frame could not be decoded straight from its bytes by
/// [`decode_core_frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CoreFrameDecodeError {
    /// The frame carries a Table 5-28 joint-intensity side-info tail
    /// this crate does not yet decode: some channel has `JOINX > 0`,
    /// so a variable-length `JOIN_SHUFF` / `JOIN_SCALES` block (gated on
    /// the unstaged joint-scale table) sits between a subframe's side
    /// info and its §5.5 `Audio Data` region, and the audio-data bit
    /// offset cannot be located. The `DYNF` (`RANGE`) and `CPF`
    /// (`SICRC`) tail fields are decoded (see [`decode_core_frame`]);
    /// only joint-intensity surfaces here.
    UnsupportedSideInfoTail {
        /// `DYNF != 0` — embedded dynamic-range `RANGE` field present.
        /// Retained for source compatibility; no longer a decline
        /// reason (the `RANGE` field is decoded and applied post-QMF).
        dynamic_range: bool,
        /// `CPF != 0` — a 16-bit `SICRC` side-info CRC trailer present.
        /// Retained for source compatibility; no longer a decline
        /// reason (the `SICRC` word is consumed for framing).
        side_info_crc: bool,
        /// Some channel carries `JOINX > 0` — a `JOIN_SHUFF`/`JOIN_SCALES`
        /// block present. This is the sole remaining decline reason.
        joint_intensity: bool,
    },
    /// A §5.3.2 / §5.4.1 / §5.5 decode step failed. Carries the
    /// underlying [`SubframePcmError`] (or a wrapped bit-stream
    /// [`crate::Error`] for the header/side-info walks).
    Decode(SubframePcmError),
    /// A structural bit-stream error in the §5.3.2 audio-coding-header or
    /// §5.4.1 side-info walk (EOF, reserved selector, …).
    Bitstream(crate::Error),
}

impl core::fmt::Display for CoreFrameDecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CoreFrameDecodeError::UnsupportedSideInfoTail {
                dynamic_range,
                side_info_crc,
                joint_intensity,
            } => write!(
                f,
                "frame carries an undecoded §5.4.x side-info tail \
                 (DYNF={dynamic_range}, CPF/SICRC={side_info_crc}, \
                 JOINX>0={joint_intensity}); only the empty-tail common \
                 Core case is decoded to PCM"
            ),
            CoreFrameDecodeError::Decode(e) => write!(f, "{e}"),
            CoreFrameDecodeError::Bitstream(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CoreFrameDecodeError {}

impl From<SubframePcmError> for CoreFrameDecodeError {
    fn from(e: SubframePcmError) -> Self {
        CoreFrameDecodeError::Decode(e)
    }
}

impl From<crate::Error> for CoreFrameDecodeError {
    fn from(e: crate::Error) -> Self {
        CoreFrameDecodeError::Bitstream(e)
    }
}

/// Decode one whole DTS Core frame to planar PCM straight from its
/// bytes, for the common Core case (§5.3 / §5.4 / §5.5 + §C.2.5).
///
/// This is the top-level orchestrator that chains the landed stages:
///
/// 1. the §5.3.2 [`crate::decode_audio_coding_header_at`] (Table 5-21)
///    from the bit just after the §5.3.1 frame header
///    ([`DtsFrameHeader::header_bit_length`]);
/// 2. for each of the `nSUBFS` subframes, the §5.4.1
///    [`crate::decode_primary_side_info_at`] (Table 5-28) walk, then the
///    §5.5 + §C.2.5 [`SubframePcmDecoder::decode_subframe`] reconstruction;
///    the per-channel §C.2.5 filter tail carries across subframes.
///
/// `header` is the already-parsed §5.3.1 frame header; `bytes` is the
/// frame's unpacked (16-bit-word-domain) bit-stream buffer.
///
/// # Scope
///
/// Joint-intensity sub-band coding (`JOINX > 0`) **is** decoded: each
/// subframe's [`crate::decode_primary_side_info_tail_at`] resolves the
/// Table 5-28 `JOIN_SHUFF` / `JOIN_SCALES` tail (the §D.3 joint-scale
/// table), and the §C.2.3 sub-band copy imports the source channel's
/// sub-bands, scaled by `JOIN_SCALES`, before QMF synthesis.
///
/// The frame header's `DYNF` (embedded dynamic range) and `CPF`
/// (side-info CRC) tail fields are likewise handled: each subframe's
/// tail walk consumes the 8-bit signed-Q2 `RANGE` code (`DYNF != 0`)
/// and the 16-bit `SICRC` word (`CPF == 1`), and the
/// [`crate::dts_dynrng_to_linear`] gain is applied to that subframe's
/// reconstructed PCM after QMF synthesis (per §5.4.1).
///
/// §D.10 frames (`nVQSUB < nSUBS` / `PMODE != 0`) decode through the
/// built-in code books; the typed VQ-book blocker surfaces (as
/// [`CoreFrameDecodeError::Decode`]) only for a caller-stripped
/// decoder ([`VqCodebooks::none`]).
///
/// Returns planar PCM (one `Vec<i32>` per channel, `Σ nSSC · 256`
/// samples each; a termination frame's trailing partial subsubframe
/// shrinks its subframe's contribution to `((nSSC-1)·8 + PSC) · 32` —
/// the frame total is always `(NBLKS + 1) · 32`).
///
/// # Errors
///
/// * [`CoreFrameDecodeError::Bitstream`] for a §5.3.2 / §5.4.1 walk
///   failure;
/// * [`CoreFrameDecodeError::Decode`] for a §5.5 / §C.2.5 failure
///   (including the §D.10 VQ blockers and a reserved `PCMR`).
pub fn decode_core_frame(
    bytes: &[u8],
    header: &DtsFrameHeader,
) -> Result<SubframePcm, CoreFrameDecodeError> {
    // §5.3.2 Primary Audio Coding Header begins right after the §5.3.1
    // frame header; the channel count it declares sizes the per-channel
    // §C.2.5 filter bank. A fresh per-call decoder gives single-frame
    // semantics (cleared filter history) — for a multi-frame elementary
    // stream use [`CoreStreamDecoder`], which persists the per-channel
    // §C.2.5 filter tail across frame boundaries (the spec's filter is a
    // continuous per-channel object, not reset between frames).
    let header_bits = header.header_bit_length() as usize;
    let cpf = header.crc_present;
    let (coding, _ach_bits) = crate::decode_audio_coding_header_at(bytes, header_bits, cpf)?;
    let mut decoder = SubframePcmDecoder::new(coding.n_pchs);
    decoder.decode_core_frame_into(bytes, header)
}

/// [`decode_core_frame`] plus the frame's §5.6 Table 5-30
/// optional-information region: after the last audio-data array the
/// walk continues through the flag-gated `TIMES` (time code stamp,
/// `TIMEF`), `AUXCT`/`AUXD` (auxiliary bytes, `AUXF`), and `OCRC`
/// (`CPF && DYNF`) fields via [`crate::decode_optional_info_at`],
/// returning them alongside the planar PCM.
///
/// Same single-frame semantics (cleared filter history) as
/// [`decode_core_frame`]; use
/// [`CoreStreamDecoder::decode_frame_with_info`] for the multi-frame
/// path.
///
/// # Errors
///
/// See [`decode_core_frame`]; a truncated optional-information region
/// additionally surfaces as [`CoreFrameDecodeError::Bitstream`].
pub fn decode_core_frame_with_info(
    bytes: &[u8],
    header: &DtsFrameHeader,
) -> Result<(SubframePcm, crate::OptionalInfo), CoreFrameDecodeError> {
    let header_bits = header.header_bit_length() as usize;
    let cpf = header.crc_present;
    let (coding, _ach_bits) = crate::decode_audio_coding_header_at(bytes, header_bits, cpf)?;
    let mut decoder = SubframePcmDecoder::new(coding.n_pchs);
    decoder.decode_core_frame_with_info_into(bytes, header)
}

/// Persistent §5.3/§5.4/§5.5 + §C.2.5 Core-stream decoder.
///
/// The §C.2.5 `aPrmCh[ch]` synthesis filter is a **continuous**
/// per-channel object whose 512-tap history (`raX[]`) and output
/// accumulator (`raZ[]`) carry across subframe **and frame**
/// boundaries of a contiguous elementary stream — the decoder does not
/// reset the filter at each frame. [`decode_core_frame`] (a fresh
/// per-call decoder) therefore reconstructs each frame as if it were
/// the first frame of a stream, which produces a filter-warmup
/// transient at every frame boundary instead of only the stream's true
/// start. For multi-frame decode use this type: construct it once for
/// the stream's channel count and feed every frame in order through
/// [`CoreStreamDecoder::decode_frame`], so each channel's inter-frame
/// filter tail carries correctly.
///
/// Validated against a black-box `ffmpeg -c:a dca` reference decode of
/// the bundled 5-frame fixture: carrying the filter state across frames
/// makes our channel-0 PCM **shape-identical** to the reference
/// (Pearson correlation 1.0 over the whole stream), versus 0.73 when
/// the filter is reset per frame. (The two differ only by the
/// implementation-defined output `rScale` constant — see
/// [`DtsFrameHeader::output_r_scale`] and the round-356 report.)
#[derive(Debug, Clone)]
pub struct CoreStreamDecoder {
    decoder: SubframePcmDecoder,
}

impl CoreStreamDecoder {
    /// Construct a stream decoder for `channels` primary audio channels
    /// (the §5.3.2 `nPCHS`). Every channel's §C.2.5 filter starts with a
    /// cleared history; that history then carries across every
    /// [`CoreStreamDecoder::decode_frame`] call.
    #[must_use]
    pub fn new(channels: usize) -> Self {
        Self {
            decoder: SubframePcmDecoder::new(channels),
        }
    }

    /// The configured channel count (`nPCHS`).
    #[must_use]
    pub fn channel_count(&self) -> usize {
        self.decoder.channel_count()
    }

    /// Borrow the persistent per-subframe decoder (e.g. to inspect a
    /// channel's inter-frame §C.2.5 filter tail via
    /// [`SubframePcmDecoder::qmf`]).
    #[must_use]
    pub fn subframe_decoder(&self) -> &SubframePcmDecoder {
        &self.decoder
    }

    /// Replace the §D.10 VQ code books for the whole stream — see
    /// [`SubframePcmDecoder::set_vq_codebooks`] (the built-in real
    /// books are the default). The §C.2.2 reconstruction history
    /// carries across frames per each frame header's `HFLAG` gate
    /// (§5.3.1: history used when `HFLAG = 1`, ignored — zeroed —
    /// otherwise).
    pub fn set_vq_codebooks(&mut self, books: VqCodebooks) {
        self.decoder.set_vq_codebooks(books);
    }

    /// The currently attached §D.10 books (default: the built-in real
    /// books).
    #[must_use]
    pub fn vq_codebooks(&self) -> &VqCodebooks {
        self.decoder.vq_codebooks()
    }

    /// Take the LFE PCM decoded by the most recent
    /// [`Self::decode_frame`] call (empty when the frame carried no LFE
    /// channel, `LFF == 0`). See
    /// [`SubframePcmDecoder::take_last_lfe_pcm`]. The LFE PCM is at the
    /// same per-frame sample rate as the primary channels (the §C.2.6
    /// interpolation expands each decimated sample by exactly the factor
    /// that matches the primary `nSSC·256` per-subframe length).
    #[must_use]
    pub fn take_last_lfe_pcm(&mut self) -> Vec<i32> {
        self.decoder.take_last_lfe_pcm()
    }

    /// Decode one whole Core frame to planar PCM, carrying the
    /// per-channel §C.2.5 filter tail into the next call.
    ///
    /// Identical reconstruction to [`decode_core_frame`] except the
    /// filter state is **not** reset: a frame's first output samples see
    /// the previous frame's filter tail, exactly as the §C.2.5
    /// continuous per-channel filter requires for a contiguous stream.
    ///
    /// `bytes` is one frame's bit-stream buffer; `header` its parsed
    /// §5.3.1 header. The frame's §5.3.2 audio-coding-header channel
    /// count must equal this decoder's configured channel count.
    ///
    /// # Errors
    ///
    /// The same errors as [`decode_core_frame`], plus
    /// [`CoreFrameDecodeError::Decode`] wrapping a
    /// [`SubframePcmError::ChannelCountMismatch`] if the frame's
    /// `nPCHS` disagrees with the configured channel count.
    pub fn decode_frame(
        &mut self,
        bytes: &[u8],
        header: &DtsFrameHeader,
    ) -> Result<SubframePcm, CoreFrameDecodeError> {
        self.decoder.decode_core_frame_into(bytes, header)
    }

    /// [`Self::decode_frame`] plus the frame's §5.6 Table 5-30
    /// optional-information region (`TIMES` / `AUXD` / `OCRC`),
    /// walked from the end-of-audio bit cursor. See
    /// [`SubframePcmDecoder::decode_core_frame_with_info_into`].
    ///
    /// # Errors
    ///
    /// See [`Self::decode_frame`]; a truncated optional-information
    /// region additionally surfaces as
    /// [`CoreFrameDecodeError::Bitstream`].
    pub fn decode_frame_with_info(
        &mut self,
        bytes: &[u8],
        header: &DtsFrameHeader,
    ) -> Result<(SubframePcm, crate::OptionalInfo), CoreFrameDecodeError> {
        self.decoder.decode_core_frame_with_info_into(bytes, header)
    }
}

impl SubframePcmDecoder {
    /// Decode one whole Core frame to planar PCM using this persistent
    /// decoder's per-channel §C.2.5 filter state (carried across calls).
    ///
    /// This is the per-frame body shared by [`decode_core_frame`] (which
    /// calls it on a fresh decoder, giving single-frame semantics) and
    /// [`CoreStreamDecoder::decode_frame`] (which calls it on a
    /// stream-lifetime decoder, carrying the inter-frame filter tail).
    ///
    /// # Errors
    ///
    /// See [`decode_core_frame`].
    pub fn decode_core_frame_into(
        &mut self,
        bytes: &[u8],
        header: &DtsFrameHeader,
    ) -> Result<SubframePcm, CoreFrameDecodeError> {
        self.decode_core_frame_cursor(bytes, header)
            .map(|(pcm, _)| pcm)
    }

    /// [`Self::decode_core_frame_into`] plus the §5.6 Table 5-30
    /// optional-information region that follows the last audio-data
    /// array: the walk continues from the end-of-audio bit cursor
    /// through the flag-gated `TIMES` / `AUXCT` / `AUXD` / `OCRC`
    /// fields ([`crate::decode_optional_info_at`]).
    ///
    /// # Errors
    ///
    /// See [`decode_core_frame`]; a truncated optional-information
    /// region additionally surfaces as
    /// [`CoreFrameDecodeError::Bitstream`].
    pub fn decode_core_frame_with_info_into(
        &mut self,
        bytes: &[u8],
        header: &DtsFrameHeader,
    ) -> Result<(SubframePcm, crate::OptionalInfo), CoreFrameDecodeError> {
        let (pcm, end_bit) = self.decode_core_frame_cursor(bytes, header)?;
        let (info, _info_bits) = crate::decode_optional_info_at(bytes, end_bit, header)
            .map_err(CoreFrameDecodeError::Bitstream)?;
        Ok((pcm, info))
    }

    /// Shared §5.3.2 → §5.4.1 → §5.5 + §C.2.5 frame walk, returning
    /// the planar PCM plus the bit cursor at the end of the last
    /// audio-data array (where the §5.6 Table 5-30 region begins).
    fn decode_core_frame_cursor(
        &mut self,
        bytes: &[u8],
        header: &DtsFrameHeader,
    ) -> Result<(SubframePcm, usize), CoreFrameDecodeError> {
        // §5.3.2 Primary Audio Coding Header begins right after the
        // §5.3.1 frame header. The §5.3.1 CRC-present flag
        // (CPF == `crc_present`) controls the optional 16-bit SICRC
        // trailer of every subframe's §5.4.1 side info.
        let header_bits = header.header_bit_length() as usize;
        let cpf = header.crc_present;
        let (coding, ach_bits) = crate::decode_audio_coding_header_at(bytes, header_bits, cpf)?;

        let channels = coding.n_pchs;
        if channels != self.channel_count() {
            return Err(CoreFrameDecodeError::Decode(
                SubframePcmError::ChannelCountMismatch {
                    expected: self.channel_count(),
                    got: channels,
                },
            ));
        }
        let mut pcm: SubframePcm = vec![Vec::new(); channels];

        // The §5.4.1 side-info walk needs the per-channel
        // ChannelSideInfoParams.
        let params: Vec<_> = coding.channel_params.clone();

        // §5.7.2.2: when a Rev2AUX chunk carries broadcast DRC values,
        // they "should be used instead of any dynamic range control
        // coefficients found in the legacy core stream (indicated by
        // flag DYNF)". Look the chunk up front so the per-subframe
        // legacy RANGE gain can be suppressed; gate on the verified
        // Annex B CRC so a false DWORD-aligned sync alias inside the
        // audio payload cannot hijack the gain path.
        let frame_end = bytes.len().min(usize::from(header.frame_size_bytes));
        let rev2_drc: Option<Vec<f64>> = crate::parse_rev2_aux(&bytes[..frame_end], header)
            .ok()
            .flatten()
            .filter(|chunk| chunk.crc_valid)
            .and_then(|chunk| chunk.drc)
            .filter(|drc| {
                drc.version == crate::REV2_DRC_VERSION_SINGLE_BAND && !drc.codes.is_empty()
            })
            .map(|drc| drc.multipliers());

        // §5.3.1 HFLAG (Predictor History Flag Switch): "When
        // generating ADPCM predictions for current frame, the decoder
        // will use reconstruction history of the previous frame if
        // HFLAG = 1. Otherwise, the history will be ignored" — an
        // entry-point frame is coded without the previous frame's
        // predictor history, so the persistent §C.2.2 history is
        // zeroed before this frame's first subframe. (Within the
        // frame the history always carries across subframes.)
        if !header.predictor_history {
            self.adpcm_history.clear();
        }

        let n_subs = coding.n_subs();
        let mut bit = header_bits + ach_bits;
        for subframe_index in 0..coding.n_subframes {
            // §5.4.1 side info (Table 5-28) through the end of the
            // SCALES block.
            let (side, side_bits) = crate::decode_primary_side_info_at(bytes, bit, &params)?;
            bit += side_bits;

            // The Table 5-28 JOIN_SHUFF / JOIN_SCALES / RANGE (DYNF) /
            // SICRC (CPF) tail sits between the SCALES block and the §5.5
            // region. The joint-intensity JOIN_SCALES factors (if any)
            // feed the §C.2.3 sub-band copy below.
            let (tail, tail_bits): (SideInfoTail, usize) = crate::decode_primary_side_info_tail_at(
                bytes,
                bit,
                &coding.joinx,
                &n_subs,
                header.dynamic_range,
                cpf,
            )?;
            bit += tail_bits;

            let n_ssc = side.subsubframe_count.n_ssc() as usize;
            // §5.4.1 PSC: a partial (fewer-than-8-sample) trailing
            // subsubframe "exists only in a termination frame" (PDF
            // p.30) — a normal frame signalling one is structurally
            // invalid and declines rather than truncating its audio.
            let psc = side.subsubframe_count.psc;
            if psc > 0 && header.frame_type != crate::header::FrameType::Termination {
                return Err(CoreFrameDecodeError::Decode(
                    SubframePcmError::PartialSubsubframeInNormalFrame {
                        subframe: subframe_index,
                        psc,
                    },
                ));
            }
            let (mut block, audio_bits) = self.decode_subframe_partial(
                bytes,
                bit,
                header,
                &coding,
                &side.channels,
                n_ssc,
                psc,
                header.aspf,
                &tail.join_scales,
            )?;

            // §5.4.1: when DYNF != 0, multiply every reconstructed PCM
            // sample of this subframe by the linear DRC gain, applied
            // after QMF synthesis. The 8-bit RANGE code is signed Q2
            // (dB = (int8)code · 0.25; see dts_dynrng_to_db and
            // docs/audio/dts/dts-drc-dynrng.md) — NOT a raw index into
            // the offset-binary §D.4 presentation table. Suppressed
            // (the field is still consumed for framing) when a
            // CRC-verified Rev2AUX DRC payload overrides it (§5.7.2.2).
            if let Some(code) = tail.range_index
                && rev2_drc.is_none()
            {
                apply_range(&mut block, crate::dts_dynrng_to_linear(code));
            }

            for (ch, samples) in block.into_iter().enumerate() {
                pcm[ch].extend(samples);
            }
            bit += audio_bits;
        }

        // §5.7.2 Table 5-34: one Rev2AUX DRC value per 256-sample
        // subsubframe of the frame, each applied to its own window of
        // the reconstructed PCM (replacing the per-subframe legacy
        // gain suppressed above).
        if let Some(multipliers) = &rev2_drc {
            apply_rev2_drc(&mut pcm, multipliers);
        }

        Ok((pcm, bit))
    }
}

/// Apply the §5.7.2 Rev2AUX per-subsubframe DRC multipliers to the
/// frame's reconstructed planar PCM, in place: plane `ch` is split
/// into `multipliers.len()` equal consecutive windows (Table 5-34: one
/// 8-bit DRC value per 256-sample subsubframe) and window `k` is
/// scaled by `multipliers[k]` with the same round-to-nearest /
/// `i32`-saturating convention as the legacy `RANGE` gain.
///
/// A plane whose length is not an exact multiple of the value count
/// (only possible on a malformed stream whose `NBLKS` disagrees with
/// the decoded subframe structure) is left untouched rather than
/// scaled with a guessed window split.
fn apply_rev2_drc(pcm: &mut SubframePcm, multipliers: &[f64]) {
    for channel in pcm.iter_mut() {
        if multipliers.is_empty() || channel.len() % multipliers.len() != 0 {
            continue;
        }
        let window = channel.len() / multipliers.len();
        if window == 0 {
            continue;
        }
        for (chunk, &m) in channel.chunks_mut(window).zip(multipliers) {
            if m == 1.0 {
                continue;
            }
            for sample in chunk.iter_mut() {
                let scaled = (*sample as f64 * m).round();
                *sample = if scaled >= i32::MAX as f64 {
                    i32::MAX
                } else if scaled <= i32::MIN as f64 {
                    i32::MIN
                } else {
                    scaled as i32
                };
            }
        }
    }
}

/// Apply the §5.4.1 `RANGE` dynamic-range multiplier (the signed-Q2
/// [`crate::dts_dynrng_to_linear`] gain) to every reconstructed PCM
/// sample of one subframe, in place, after QMF synthesis. Results are
/// rounded to the nearest integer and saturated to the `i32` range.
fn apply_range(block: &mut SubframePcm, range: f64) {
    if range == 1.0 {
        return;
    }
    for channel in block.iter_mut() {
        for sample in channel.iter_mut() {
            let scaled = (*sample as f64 * range).round();
            *sample = if scaled >= i32::MAX as f64 {
                i32::MAX
            } else if scaled <= i32::MIN as f64 {
                i32::MIN
            } else {
                scaled as i32
            };
        }
    }
}

/// Apply the §C.2.3 joint-intensity sub-band copy to the per-channel
/// sub-band matrices, in place, before QMF synthesis.
///
/// For every destination channel `ch` with `JOINX[ch] > 0` and a
/// non-empty `join_scales[ch]`, each imported sub-band column `n ∈
/// [nSUBS[ch], nSUBS[nSourceCh])` of every sample row is overwritten
/// with the source channel's (`nSourceCh = JOINX[ch] - 1`) sub-band
/// sample scaled by the matching `JOIN_SCALES[ch][n]` factor.
///
/// The `join_scales[ch]` vector supplies one factor per imported
/// sub-band, ordered from `nSUBS[ch]`; its length must equal
/// `nSUBS[nSourceCh] - nSUBS[ch]`. Structural inconsistencies (source
/// channel out of range, mismatched factor count, matrices shorter than
/// `nSUBS[nSourceCh]`) surface as
/// [`SubframePcmError::JointSubbandShape`].
fn apply_joint_subband(
    matrices: &mut [SubbandSampleMatrix],
    coding: &AudioCodingHeader,
    n_subs: &[usize],
    join_scales: &[Vec<f64>],
) -> Result<(), SubframePcmError> {
    let channels = matrices.len();
    for ch in 0..channels {
        let factors = join_scales.get(ch).map(Vec::as_slice).unwrap_or(&[]);
        if factors.is_empty() {
            continue;
        }
        let joinx = coding.joinx.get(ch).copied().unwrap_or(0);
        let Some(source_ch) = crate::joint_source_channel(joinx).map(usize::from) else {
            return Err(SubframePcmError::JointSubbandShape { ch });
        };
        if source_ch >= channels || ch >= n_subs.len() || source_ch >= n_subs.len() {
            return Err(SubframePcmError::JointSubbandShape { ch });
        }
        let n_subs_dst = n_subs[ch];
        let n_subs_src = n_subs[source_ch];
        // The import range must run forward and match the factor count.
        if n_subs_src < n_subs_dst || factors.len() != n_subs_src - n_subs_dst {
            return Err(SubframePcmError::JointSubbandShape { ch });
        }
        if n_subs_src > NUM_SUBBAND {
            return Err(SubframePcmError::JointSubbandShape { ch });
        }
        // Destination and source are distinct channels of one Vec here
        // (a self-referential joint has an empty import range and never
        // reaches this point). Split the slice so the source (immutable)
        // and destination (mutable) borrows do not overlap.
        let rows = matrices[ch].len();
        if matrices[source_ch].len() != rows {
            return Err(SubframePcmError::JointSubbandShape { ch });
        }
        let (lo, hi) = if ch < source_ch {
            (ch, source_ch)
        } else {
            (source_ch, ch)
        };
        let (left, right) = matrices.split_at_mut(hi);
        let (dst_ch, src_ch): (&mut SubbandSampleMatrix, &SubbandSampleMatrix) = if ch == lo {
            (&mut left[ch], &right[0])
        } else {
            (&mut right[0], &left[source_ch])
        };
        for (dst_row, src_row) in dst_ch.iter_mut().zip(src_ch.iter()) {
            for (k, &factor) in factors.iter().enumerate() {
                let n = n_subs_dst + k;
                dst_row[n] = factor * src_row[n];
            }
        }
    }
    Ok(())
}

/// Apply the §C.2.4 sum/difference matrix to one channel pair's
/// reconstructed sub-band samples, in place, before QMF synthesis.
///
/// For each active sub-band (`n ∈ [0, nSUBS)`) and each sub-subframe
/// sample row, the pair `(l, r)` — with `l` the front/surround **left**
/// channel and `r` the **right** — is matrixed as
/// `(L', R') = (L + R, L - R)`, reading the pre-update value of the left
/// sample for both outputs (the §C.2.4 pseudocode's read-old/write-new
/// ordering). The active sub-band bound is the smaller of the two
/// channels' `nSUBS` (they are equal in a well-formed stream) clamped to
/// the 32-band matrix width.
///
/// Structural inconsistencies (channel index out of range, `l == r`, or
/// the two channels' matrices carrying a different sample-row count)
/// surface as [`SubframePcmError::JointSubbandShape`] — the same
/// caller-side sub-band-matrix shape-violation variant the §C.2.3 copy
/// uses.
fn apply_sum_difference(
    matrices: &mut [SubbandSampleMatrix],
    l: usize,
    r: usize,
    n_subs: &[usize],
) -> Result<(), SubframePcmError> {
    let channels = matrices.len();
    if l >= channels || r >= channels || l == r {
        return Err(SubframePcmError::JointSubbandShape { ch: l.min(r) });
    }
    let n_active = n_subs
        .get(l)
        .copied()
        .unwrap_or(0)
        .min(n_subs.get(r).copied().unwrap_or(0))
        .min(NUM_SUBBAND);
    if n_active == 0 {
        return Ok(());
    }
    if matrices[l].len() != matrices[r].len() {
        return Err(SubframePcmError::JointSubbandShape { ch: l.min(r) });
    }
    // Split the backing Vec so the two distinct channel matrices can be
    // borrowed mutably at once (mirrors the §C.2.3 copy's split pattern).
    let (lo, hi) = if l < r { (l, r) } else { (r, l) };
    let (lower, upper) = matrices.split_at_mut(hi);
    // `left_m` is the front/surround-left channel `l`, `right_m` is `r`.
    let (left_m, right_m): (&mut SubbandSampleMatrix, &mut SubbandSampleMatrix) = if l == lo {
        (&mut lower[l], &mut upper[0])
    } else {
        (&mut upper[0], &mut lower[r])
    };
    for (left_row, right_row) in left_m.iter_mut().zip(right_m.iter_mut()) {
        for n in 0..n_active {
            let lv = left_row[n];
            let rv = right_row[n];
            left_row[n] = lv + rv;
            right_row[n] = lv - rv;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::side_info::ScaleFactorAdjustment;
    use crate::step_size::SAMPLES_PER_SUBSUBFRAME;
    use crate::subframe::ChannelSideInfo;

    /// Pack a list of `(value, width)` MSB-first into bytes.
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

    /// A single-channel single-subsubframe NFE subframe reconstructs to
    /// PCM end to end, and the PCM equals running the §5.5 walk + the
    /// §C.2.5 driver by hand.
    #[test]
    fn nfe_subframe_round_trips_to_pcm() {
        // ABITS 8 -> NFE width 5; SEL 7 selects the terminal NFE entry.
        let mut ch = ChannelSideInfo::cleared();
        ch.abits[0] = 8;
        ch.scales[0][0] = 4;
        let side = vec![ch];

        let vals = [3i32, -3, 5, -5, 7, -7, 2, -2];
        let mut fields: Vec<(u32, u8)> = vals.iter().map(|&v| ((v as u32) & 0x1f, 5u8)).collect();
        fields.push((0xffff, 16)); // DSYNC
        let stream = pack_fields(&fields);

        // Build a parsed header carrying FILTS / PCMR via the public
        // parser: reuse the registry test fixture's real BE header
        // (PCMR index 0 -> 16-bit -> rScale 32768, FILTS = 0).
        let hdr_bytes: [u8; 16] = [
            0x7f, 0xfe, 0x80, 0x01, 0xfc, 0x3c, 0x3f, 0xf0, 0xb5, 0xe0, 0x01, 0x38, 0x00, 0x03,
            0xef, 0x7f,
        ];
        let header = crate::parse_frame_header(&hdr_bytes).unwrap();
        assert_eq!(header.output_r_scale(), Some(32768.0));

        // A one-channel AudioCodingHeader with nSUBS=nVQSUB=1, JOINX=0,
        // SEL[ch][ABITS 8-1] = 7 (terminal NFE). Build it through the
        // public test constructor.
        let coding = AudioCodingHeader::single_channel_for_test(1, 1, 7);

        let mut dec = SubframePcmDecoder::new(1);
        let (pcm, bits) = dec
            .decode_subframe(&stream, 0, &header, &coding, &side, 1, false)
            .unwrap();

        // Reference: walk + driver by hand.
        let table = StepSizeTable::for_rate(header.rate_index);
        let (mats, ref_bits) = crate::audio_array::decode_audio_data_subframe_at(
            &stream,
            0,
            &side,
            |_, _| 7,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            1,
            table,
            false,
        )
        .unwrap();
        let refs: Vec<&[[f64; NUM_SUBBAND]]> = mats.iter().map(|m| m.as_slice()).collect();
        let mut mc = MultiChannelQmf::new(1);
        let mut expect = vec![Vec::new(); 1];
        mc.synthesize_planar(
            &refs,
            &[1],
            header.filter_bank_selection(),
            32768.0,
            &mut expect,
        )
        .unwrap();

        assert_eq!(bits, ref_bits);
        assert_eq!(pcm, expect);
        // One subsubframe of 8 rows -> 8 * 32 = 256 PCM samples.
        assert_eq!(pcm[0].len(), SAMPLES_PER_SUBSUBFRAME * PCM_PER_SUBBAND_ROW);
        assert!(pcm[0].iter().any(|&s| s != 0));
    }

    /// A reserved PCMR code fails cleanly without disturbing the filter
    /// state.
    #[test]
    fn reserved_pcmr_declines() {
        // PCMR index 4 (0b100) is one of the reserved codes -> rScale
        // None. Construct a header with that PCMR via the test setter.
        let hdr_bytes: [u8; 16] = [
            0x7f, 0xfe, 0x80, 0x01, 0xfc, 0x3c, 0x3f, 0xf0, 0xb5, 0xe0, 0x01, 0x38, 0x00, 0x03,
            0xef, 0x7f,
        ];
        let mut header = crate::parse_frame_header(&hdr_bytes).unwrap();
        header.source_pcm_resolution_index = 4; // reserved
        assert_eq!(header.output_r_scale(), None);

        let side = vec![ChannelSideInfo::cleared()];
        let coding = AudioCodingHeader::single_channel_for_test(1, 1, 0);
        let mut dec = SubframePcmDecoder::new(1);
        let err = dec
            .decode_subframe(&[0u8; 4], 0, &header, &coding, &side, 1, false)
            .unwrap_err();
        assert!(matches!(
            err,
            SubframePcmError::ReservedPcmResolution { pcmr: 4 }
        ));
        // Filter untouched.
        assert!(
            dec.qmf()
                .channels()
                .iter()
                .all(|q| q.x_history().iter().all(|&v| v == 0.0))
        );
    }

    /// A JOINX > 0 channel is declined.
    #[test]
    fn joint_subband_declined() {
        let hdr_bytes: [u8; 16] = [
            0x7f, 0xfe, 0x80, 0x01, 0xfc, 0x3c, 0x3f, 0xf0, 0xb5, 0xe0, 0x01, 0x38, 0x00, 0x03,
            0xef, 0x7f,
        ];
        let header = crate::parse_frame_header(&hdr_bytes).unwrap();
        let side = vec![ChannelSideInfo::cleared()];
        let mut coding = AudioCodingHeader::single_channel_for_test(1, 1, 0);
        coding.set_joinx_for_test(0, 2);
        let mut dec = SubframePcmDecoder::new(1);
        let err = dec
            .decode_subframe(&[0u8; 4], 0, &header, &coding, &side, 1, false)
            .unwrap_err();
        assert!(matches!(
            err,
            SubframePcmError::JointSubbandUnsupported { ch: 0, joinx: 2 }
        ));
    }

    /// The §C.2.3 joint copy overwrites the destination channel's
    /// imported sub-band columns with the source channel's samples,
    /// scaled by the matching JOIN_SCALES factor, and leaves the
    /// non-imported columns untouched.
    #[test]
    fn apply_joint_subband_scales_imported_columns() {
        // 2 channels, 2 sample rows. ch0 (source) nSUBS=4, ch1 (dst)
        // nSUBS=2 -> import subbands 2 and 3 from ch0 into ch1.
        let mut ch0 = vec![[0.0f64; NUM_SUBBAND]; 2];
        let mut ch1 = vec![[0.0f64; NUM_SUBBAND]; 2];
        // Source subband samples in columns 2 and 3.
        ch0[0][2] = 10.0;
        ch0[0][3] = -4.0;
        ch0[1][2] = 5.0;
        ch0[1][3] = 8.0;
        // Destination pre-existing values in columns 0/1 (kept) and a
        // stray in column 2 (must be overwritten).
        ch1[0][0] = 99.0;
        ch1[0][2] = 7.0;
        let mut matrices = vec![ch0, ch1];

        let mut coding = AudioCodingHeader::two_channel_for_test((4, 4), (2, 2), 0);
        coding.set_joinx_for_test(1, 1); // ch1 sources ch0 (JOINX=1)

        // JOIN_SCALES[1] = [2.0, 3.0] for imported subbands 2 and 3.
        let join_scales = vec![Vec::new(), vec![2.0, 3.0]];
        let n_subs = [4usize, 2usize];

        apply_joint_subband(&mut matrices, &coding, &n_subs, &join_scales).unwrap();

        // Imported columns are source * factor.
        assert_eq!(matrices[1][0][2], 20.0); // 10 * 2
        assert_eq!(matrices[1][0][3], -12.0); // -4 * 3
        assert_eq!(matrices[1][1][2], 10.0); // 5 * 2
        assert_eq!(matrices[1][1][3], 24.0); // 8 * 3
        // Non-imported destination columns untouched.
        assert_eq!(matrices[1][0][0], 99.0);
        // Source channel untouched.
        assert_eq!(matrices[0][0][2], 10.0);
    }

    /// A JOIN_SCALES vector whose length disagrees with the import range
    /// is rejected as a shape error.
    #[test]
    fn apply_joint_subband_rejects_wrong_factor_count() {
        let mut matrices = vec![
            vec![[0.0f64; NUM_SUBBAND]; 1],
            vec![[0.0f64; NUM_SUBBAND]; 1],
        ];
        let mut coding = AudioCodingHeader::two_channel_for_test((4, 4), (2, 2), 0);
        coding.set_joinx_for_test(1, 1);
        // Import range is [2, 4) = 2 subbands, but only 1 factor given.
        let join_scales = vec![Vec::new(), vec![2.0]];
        let n_subs = [4usize, 2usize];
        let err = apply_joint_subband(&mut matrices, &coding, &n_subs, &join_scales).unwrap_err();
        assert!(matches!(err, SubframePcmError::JointSubbandShape { ch: 1 }));
    }

    /// A channel-count mismatch between the decoder and the side-info
    /// slice is rejected before any decode.
    #[test]
    fn channel_count_mismatch_rejected() {
        let hdr_bytes: [u8; 16] = [
            0x7f, 0xfe, 0x80, 0x01, 0xfc, 0x3c, 0x3f, 0xf0, 0xb5, 0xe0, 0x01, 0x38, 0x00, 0x03,
            0xef, 0x7f,
        ];
        let header = crate::parse_frame_header(&hdr_bytes).unwrap();
        let side = vec![ChannelSideInfo::cleared(), ChannelSideInfo::cleared()];
        let coding = AudioCodingHeader::single_channel_for_test(1, 1, 0);
        let mut dec = SubframePcmDecoder::new(1);
        let err = dec
            .decode_subframe(&[0u8; 4], 0, &header, &coding, &side, 1, false)
            .unwrap_err();
        assert!(matches!(
            err,
            SubframePcmError::ChannelCountMismatch {
                expected: 1,
                got: 2
            }
        ));
    }

    /// A no-bits subframe yields all-zero PCM of the right length.
    #[test]
    fn no_bits_subframe_zero_pcm() {
        let hdr_bytes: [u8; 16] = [
            0x7f, 0xfe, 0x80, 0x01, 0xfc, 0x3c, 0x3f, 0xf0, 0xb5, 0xe0, 0x01, 0x38, 0x00, 0x03,
            0xef, 0x7f,
        ];
        let header = crate::parse_frame_header(&hdr_bytes).unwrap();
        let side = vec![ChannelSideInfo::cleared()]; // ABITS all 0
        let coding = AudioCodingHeader::single_channel_for_test(1, 1, 0);
        // nSSC = 2 -> two DSYNC trailers (last subsubframe only; ASPF
        // false means only the final one).
        let stream = pack_fields(&[(0xffff, 16)]);
        let mut dec = SubframePcmDecoder::new(1);
        let (pcm, _) = dec
            .decode_subframe(&stream, 0, &header, &coding, &side, 1, false)
            .unwrap();
        assert_eq!(pcm.len(), 1);
        assert_eq!(pcm[0].len(), SAMPLES_PER_SUBSUBFRAME * PCM_PER_SUBBAND_ROW);
        assert!(pcm[0].iter().all(|&s| s == 0));
    }

    /// An LFE-present frame (`LFF != 0`) consumes the §5.5 LFE phase
    /// before the audio-data phase, and the decoded LFE PCM has the same
    /// per-subframe length as the primary channels (`nSSC·256`): the
    /// §C.2.6 interpolation expands `2·LFF·nSSC` decimated samples by
    /// `nDeciFactor` to exactly that length. The cursor stays aligned, so
    /// the trailing DSYNC still validates.
    #[test]
    fn lfe_present_subframe_consumes_lfe_phase_and_matches_primary_length() {
        let hdr_bytes: [u8; 16] = [
            0x7f, 0xfe, 0x80, 0x01, 0xfc, 0x3c, 0x3f, 0xf0, 0xb5, 0xe0, 0x01, 0x38, 0x00, 0x03,
            0xef, 0x7f,
        ];
        let mut header = crate::parse_frame_header(&hdr_bytes).unwrap();
        header.lfe = crate::LfeMode::Mode1; // LFF == 1 -> 128×
        let side = vec![ChannelSideInfo::cleared()]; // ABITS all 0
        let coding = AudioCodingHeader::single_channel_for_test(1, 1, 0);

        // §5.5 region for nSSC = 1, LFF = 1:
        //   LFE phase: 2·1·1 = 2 sample bytes + 1 scale-index byte
        //   audio-data phase: ABITS all 0 -> just a 16-bit DSYNC.
        let mut fields: Vec<(u32, u8)> = vec![(0, 8), (0, 8), (10, 8)];
        fields.push((0xffff, 16));
        let stream = pack_fields(&fields);

        let mut dec = SubframePcmDecoder::new(1);
        let (pcm, bits) = dec
            .decode_subframe(&stream, 0, &header, &coding, &side, 1, false)
            .unwrap();

        // Primary channel: nSSC·256 samples, all zero (no audio bits).
        assert_eq!(pcm.len(), 1);
        assert_eq!(pcm[0].len(), SAMPLES_PER_SUBSUBFRAME * PCM_PER_SUBBAND_ROW);
        // The cursor consumed LFE (3 bytes) + DSYNC (2 bytes) = 40 bits.
        assert_eq!(bits, (3 + 2) * 8);
        // LFE PCM is the same per-subframe length as the primary channel.
        let lfe = dec.take_last_lfe_pcm();
        assert_eq!(lfe.len(), SAMPLES_PER_SUBSUBFRAME * PCM_PER_SUBBAND_ROW);
        // All-zero LFE samples -> silence.
        assert!(lfe.iter().all(|&s| s == 0));
        // Taking it again yields empty (it was moved out).
        assert!(dec.take_last_lfe_pcm().is_empty());
    }

    /// `decode_frame` over two NFE subframes equals running
    /// `decode_subframe` twice on the same persistent decoder — the
    /// §C.2.5 filter tail carries across the subframe boundary and the
    /// PCM is concatenated in order.
    #[test]
    fn decode_frame_concatenates_and_carries_filter_state() {
        let hdr_bytes: [u8; 16] = [
            0x7f, 0xfe, 0x80, 0x01, 0xfc, 0x3c, 0x3f, 0xf0, 0xb5, 0xe0, 0x01, 0x38, 0x00, 0x03,
            0xef, 0x7f,
        ];
        let header = crate::parse_frame_header(&hdr_bytes).unwrap();
        let coding = AudioCodingHeader::single_channel_for_test(1, 1, 7);

        // Two subframes, each: 8 NFE 5-bit values + a 16-bit DSYNC. The
        // second subframe's §5.5 region directly follows the first (no
        // inter-subframe side info in this synthetic stream, so
        // side_info_bits = 0).
        let mut ch = ChannelSideInfo::cleared();
        ch.abits[0] = 8;
        ch.scales[0][0] = 4;
        let side = vec![ch];

        let mk_sf = |base: i32| -> Vec<(u32, u8)> {
            let mut f: Vec<(u32, u8)> = (0..8).map(|i| (((base + i) as u32) & 0x1f, 5u8)).collect();
            f.push((0xffff, 16));
            f
        };
        let mut fields = mk_sf(1);
        let sf0_bits: usize = fields.iter().map(|(_, w)| *w as usize).sum();
        fields.extend(mk_sf(-4));
        let stream = pack_fields(&fields);

        let subframes = [
            Subframe {
                side: &side,
                n_ssc: 1,
                side_info_bits: 0, // next subframe's §5.5 immediately follows
            },
            Subframe {
                side: &side,
                n_ssc: 1,
                side_info_bits: 0,
            },
        ];

        let mut frame_dec = SubframePcmDecoder::new(1);
        let (frame_pcm, frame_bits) = frame_dec
            .decode_frame(&stream, 0, &header, &coding, &subframes, false)
            .unwrap();

        // Reference: two decode_subframe calls on one persistent decoder.
        let mut seq_dec = SubframePcmDecoder::new(1);
        let (b0, n0) = seq_dec
            .decode_subframe(&stream, 0, &header, &coding, &side, 1, false)
            .unwrap();
        let (b1, n1) = seq_dec
            .decode_subframe(&stream, n0, &header, &coding, &side, 1, false)
            .unwrap();
        let mut expect = b0;
        for (ch, samples) in b1.into_iter().enumerate() {
            expect[ch].extend(samples);
        }

        assert_eq!(frame_pcm, expect);
        assert_eq!(frame_bits, n0 + n1);
        // Each subframe is one subsubframe -> 256 PCM samples; two -> 512.
        assert_eq!(
            frame_pcm[0].len(),
            2 * SAMPLES_PER_SUBSUBFRAME * PCM_PER_SUBBAND_ROW
        );
        // Sanity: the first subframe's §5.5 region was sf0_bits long.
        assert_eq!(n0, sf0_bits);
        assert!(frame_pcm[0].iter().any(|&s| s != 0));
    }

    /// Encode a clean §5.3.1 header (single channel, byte-aligned, with
    /// `dynamic_range`/`predictor_history`/`aspf` as given) by parsing
    /// the fixture, mutating the flags, and re-encoding. Returns the
    /// encoded header bytes (a body packed separately concatenates
    /// straight onto them; the caller parses the assembled buffer).
    fn encode_clean_header(dynf: bool, cpf: bool) -> Vec<u8> {
        let hdr_bytes: [u8; 16] = [
            0x7f, 0xfe, 0x80, 0x01, 0xfc, 0x3c, 0x3f, 0xf0, 0xb5, 0xe0, 0x01, 0x38, 0x00, 0x03,
            0xef, 0x7f,
        ];
        let mut header = crate::parse_frame_header(&hdr_bytes).unwrap();
        header.dynamic_range = dynf;
        // CPF is the §5.3.1 CRC-Present-Flag (`crc_present`), the flag
        // that gates HCRC / AHCRC / SICRC — NOT `predictor_history`.
        header.crc_present = cpf;
        // When CPF is set the header carries a 16-bit HCRC; supply a
        // value so the BE encoder serialises the field (its value is not
        // verified on decode per §5.3.1).
        header.header_crc = if cpf { Some(0) } else { None };
        header.aspf = false;
        crate::encode_frame_header_be(&header).unwrap()
    }

    /// A one-channel, one-subframe, all-`ABITS==0` (NoBits) Core frame
    /// decodes end to end from raw bytes through `decode_core_frame` to
    /// all-zero PCM of the right length.
    #[test]
    fn decode_core_frame_no_bits_round_trips() {
        let mut bytes = encode_clean_header(false, false);

        // §5.3.2 Audio Coding Header (Table 5-21), one channel:
        //   SUBFS=0 -> 1 subframe; PCHS=0 -> 1 channel;
        //   SUBS=0 -> nSUBS=2; VQSUB=1 -> nVQSUB=2 (== nSUBS, no HF VQ);
        //   JOINX=0; THUFF=0; SHUFF=0; BHUFF=0.
        //   SEL plane: ABITS1 1 bit, ABITS2-5 4×2 bits, ABITS6-10 5×3 bits.
        //   With every SEL=0, every group transmits a 2-bit ADJ -> 10 ADJ.
        let mut body: Vec<(u32, u8)> = vec![
            (0, 4), // SUBFS
            (0, 3), // PCHS
            (0, 5), // SUBS -> nSUBS 2
            (1, 5), // VQSUB -> nVQSUB 2
            (0, 3), // JOINX
            (0, 2), // THUFF
            (0, 3), // SHUFF
            (6, 3), // BHUFF=6 -> Linear5Bit (5-bit ABITS reads)
        ];
        body.push((0, 1)); // SEL ABITS1
        for _ in 1..5 {
            body.push((0, 2));
        }
        for _ in 5..10 {
            body.push((0, 3));
        }
        for _ in 0..10 {
            body.push((0, 2)); // ADJ
        }

        // §5.4.1 side info (Table 5-28), one subframe:
        //   SSC=0 -> nSSC=1; PSC=0; PMODE[0][0..2]=0 (2 bits);
        //   no PVQ (PMODE all 0); ABITS[0][0..2]=0 (2× the BHUFF=6
        //   Linear5Bit code -> 5 bits each, value 0); nSSC==1 so no
        //   TMODE plane; all ABITS 0 so no SCALES factors for the two
        //   primary subbands, and nVQSUB==nSUBS so no HF VQ scales.
        body.push((0, 2)); // SSC
        body.push((0, 3)); // PSC
        body.push((0, 1)); // PMODE[0][0]
        body.push((0, 1)); // PMODE[0][1]
        body.push((0, 5)); // ABITS[0][0] (BHUFF=6 Linear5Bit) = 0
        body.push((0, 5)); // ABITS[0][1] = 0

        // §5.5 Audio Data: nSSC=1, all ABITS 0 -> NoBits -> no audio
        // bits, then the single DSYNC trailer.
        body.push((0xffff, 16));

        let body_bytes = pack_fields(&body);
        bytes.extend_from_slice(&body_bytes);
        // A little trailing slack so the header parser's lookahead is
        // always satisfied.
        bytes.extend_from_slice(&[0u8; 4]);

        let header = crate::parse_frame_header(&bytes).unwrap();
        assert!(!header.dynamic_range);
        assert!(!header.crc_present);
        assert_eq!(header.header_bit_length() % 8, 0);

        let pcm = decode_core_frame(&bytes, &header).unwrap();
        assert_eq!(pcm.len(), 1);
        // One subframe, one subsubframe -> 8 rows -> 256 PCM samples.
        assert_eq!(pcm[0].len(), SAMPLES_PER_SUBSUBFRAME * PCM_PER_SUBBAND_ROW);
        assert!(pcm[0].iter().all(|&s| s == 0));
    }

    /// The §5.3.2 one-channel NoBits ACH body shared by the tail tests:
    /// SUBFS=0/PCHS=0/SUBS=0(nSUBS 2)/VQSUB=1(nVQSUB 2)/JOINX=0, all
    /// codebook selectors 0 except BHUFF=6 (Linear5Bit), the SEL plane,
    /// and the 10 ADJ groups. When `cpf` is set a 16-bit AHCRC trailer
    /// is appended (consumed by `decode_audio_coding_header_at`).
    fn nobits_ach_body(cpf: bool) -> Vec<(u32, u8)> {
        let mut body: Vec<(u32, u8)> = vec![
            (0, 4), // SUBFS
            (0, 3), // PCHS
            (0, 5), // SUBS -> nSUBS 2
            (1, 5), // VQSUB -> nVQSUB 2
            (0, 3), // JOINX
            (0, 2), // THUFF
            (0, 3), // SHUFF
            (6, 3), // BHUFF=6 -> Linear5Bit
        ];
        body.push((0, 1)); // SEL ABITS1
        for _ in 1..5 {
            body.push((0, 2));
        }
        for _ in 5..10 {
            body.push((0, 3));
        }
        for _ in 0..10 {
            body.push((0, 2)); // ADJ
        }
        if cpf {
            body.push((0, 16)); // AHCRC
        }
        body
    }

    /// The §5.4.1 one-subframe NoBits side-info SCALES block (SSC/PSC,
    /// 2 PMODE bits, 2 zero ABITS Linear5Bit reads — no SCALES, no HF
    /// VQ since nVQSUB==nSUBS).
    fn nobits_side_info() -> Vec<(u32, u8)> {
        vec![
            (0, 2), // SSC
            (0, 3), // PSC
            (0, 1), // PMODE[0][0]
            (0, 1), // PMODE[0][1]
            (0, 5), // ABITS[0][0] = 0
            (0, 5), // ABITS[0][1] = 0
        ]
    }

    /// A frame whose header sets `CPF` (a 16-bit `SICRC` side-info tail)
    /// now decodes end to end: the `SICRC` word is consumed for framing
    /// (its CRC test is not applied per §5.4.1) and the §5.5 region lands
    /// at the right cursor, yielding all-zero PCM of the right length.
    #[test]
    fn decode_core_frame_consumes_sicrc_tail() {
        let mut bytes = encode_clean_header(false, true); // DYNF=0, CPF=1
        let mut body = nobits_ach_body(true);
        body.extend(nobits_side_info());
        body.push((0xABCD, 16)); // SICRC (CPF=1) — consumed, not verified
        body.push((0xffff, 16)); // §5.5 DSYNC
        let body_bytes = pack_fields(&body);
        bytes.extend_from_slice(&body_bytes);
        bytes.extend_from_slice(&[0u8; 4]);

        let header = crate::parse_frame_header(&bytes).unwrap();
        assert!(header.crc_present);

        let pcm = decode_core_frame(&bytes, &header).unwrap();
        assert_eq!(pcm.len(), 1);
        assert_eq!(pcm[0].len(), SAMPLES_PER_SUBSUBFRAME * PCM_PER_SUBBAND_ROW);
        assert!(pcm[0].iter().all(|&s| s == 0));
    }

    /// A frame whose header sets `DYNF` carries an 8-bit `RANGE` code in
    /// each subframe's side-info tail; `decode_core_frame` consumes it
    /// and (for a non-unity code) the signed-Q2 linear gain scales the
    /// PCM. With an all-zero (NoBits) subframe the PCM is zero
    /// regardless of `RANGE`, which proves only the framing/cursor is
    /// correct — the `apply_range` value is covered by
    /// `range_unity_is_noop` / `range_scales_pcm`.
    #[test]
    fn decode_core_frame_consumes_range_tail() {
        let mut bytes = encode_clean_header(true, false); // DYNF=1, CPF=0
        let mut body = nobits_ach_body(false);
        body.extend(nobits_side_info());
        body.push((0, 8)); // RANGE code 0 -> unity (no SICRC, CPF=0)
        body.push((0xffff, 16)); // §5.5 DSYNC
        let body_bytes = pack_fields(&body);
        bytes.extend_from_slice(&body_bytes);
        bytes.extend_from_slice(&[0u8; 4]);

        let header = crate::parse_frame_header(&bytes).unwrap();
        assert!(header.dynamic_range);

        let pcm = decode_core_frame(&bytes, &header).unwrap();
        assert_eq!(pcm.len(), 1);
        assert_eq!(pcm[0].len(), SAMPLES_PER_SUBSUBFRAME * PCM_PER_SUBBAND_ROW);
        assert!(pcm[0].iter().all(|&s| s == 0));
    }

    /// A frame with a single `JOINX = 1` channel that references itself
    /// (source == destination, equal `nSUBS`) has an empty joint import
    /// range: the JOIN_SHUFF selector is read but no JOIN_SCALES follow,
    /// and the frame decodes normally (no more decline). This exercises
    /// the JOIN_SHUFF read on the decode path without needing a source
    /// channel wider than the destination.
    #[test]
    fn decode_core_frame_joint_self_empty_range_decodes() {
        let mut bytes = encode_clean_header(false, false);
        // ACH mirrors decode_core_frame_no_bits_round_trips but sets
        // JOINX[0] = 1 (source channel 0 == self); nSUBS[0] == nSUBS[src]
        // so the joint import range is empty. The JOIN_SHUFF selector is
        // still read from the side-info tail; no JOIN_SCALES follow.
        let mut body: Vec<(u32, u8)> = vec![
            (0, 4), // SUBFS
            (0, 3), // PCHS
            (0, 5), // SUBS  -> nSUBS = 2
            (1, 5), // VQSUB -> nVQSUB = 2 (== nSUBS, Core case)
            (1, 3), // JOINX = 1 (source channel 0 == self)
            (0, 2), // THUFF
            (0, 3), // SHUFF
            (6, 3), // BHUFF=6 -> Linear5Bit
        ];
        body.push((0, 1)); // SEL ABITS1
        for _ in 1..5 {
            body.push((0, 2));
        }
        for _ in 5..10 {
            body.push((0, 3));
        }
        for _ in 0..10 {
            body.push((0, 2)); // ADJ
        }

        // §5.4.1 side info, one subframe (as in the no-bits round trip).
        body.push((0, 2)); // SSC
        body.push((0, 3)); // PSC
        body.push((0, 1)); // PMODE[0][0]
        body.push((0, 1)); // PMODE[0][1]
        body.push((0, 5)); // ABITS[0][0] = 0
        body.push((0, 5)); // ABITS[0][1] = 0

        // §5.4.1 tail: JOINX[0] > 0 -> a 3-bit JOIN_SHUFF[0] precedes the
        // (absent) RANGE/SICRC. The empty import range emits no
        // JOIN_SCALES.
        body.push((0, 3)); // JOIN_SHUFF[0] = SA129

        // §5.5 Audio Data: all ABITS 0 -> NoBits -> just the DSYNC.
        body.push((0xffff, 16));

        let body_bytes = pack_fields(&body);
        bytes.extend_from_slice(&body_bytes);
        bytes.extend_from_slice(&[0u8; 4]);

        let header = crate::parse_frame_header(&bytes).unwrap();
        // No longer declined: the empty-range joint frame decodes.
        let pcm = decode_core_frame(&bytes, &header).unwrap();
        assert_eq!(pcm.len(), 1);
        // All-zero side info -> all-zero subband samples -> silent PCM.
        assert!(pcm[0].iter().all(|&s| s == 0));
    }

    /// `apply_range` with the unity code (signed-Q2 `0` = 0 dB) leaves
    /// the PCM untouched.
    #[test]
    fn range_unity_is_noop() {
        let mut block: SubframePcm = vec![vec![100, -200, 0, i32::MAX, i32::MIN]];
        apply_range(&mut block, crate::dts_dynrng_to_linear(0)); // 1.0
        assert_eq!(block[0], vec![100, -200, 0, i32::MAX, i32::MIN]);
    }

    /// `apply_range` scales every sample by the signed-Q2 linear gain
    /// with round-to-nearest and `i32` saturation.
    #[test]
    fn range_scales_pcm() {
        // Signed-Q2 code -80 -> -20 dB -> 0.1; code +80 -> +20 dB -> 10.0.
        let minus_20_db = 0u8.wrapping_sub(80);
        let mut down: SubframePcm = vec![vec![1000, -1000, 5]];
        apply_range(&mut down, crate::dts_dynrng_to_linear(minus_20_db)); // 0.1
        assert_eq!(down[0], vec![100, -100, 1]); // 5*0.1=0.5 -> round 1

        let mut up: SubframePcm = vec![vec![10, -10, i32::MAX]];
        apply_range(&mut up, crate::dts_dynrng_to_linear(80)); // 10.0
        assert_eq!(up[0], vec![100, -100, i32::MAX]); // saturates
    }

    /// Build a complete one-channel all-`ABITS==0` (NoBits) raw-BE Core
    /// frame — the same proven layout `decode_core_frame_no_bits_round_trips`
    /// uses — with a signed-Q2 `RANGE` code optionally injected so
    /// the `apply_range` path is exercised even though the §5.5 audio
    /// data is silent. When `dynf` is `false` no `RANGE` field is
    /// present and the frame decodes to all-zero PCM.
    fn build_nobits_frame(dynf: bool, range_index: u8) -> Vec<u8> {
        let mut header = crate::parse_frame_header(&[
            0x7f, 0xfe, 0x80, 0x01, 0xfc, 0x3c, 0x3f, 0xf0, 0xb5, 0xe0, 0x01, 0x38, 0x00, 0x03,
            0xef, 0x7f,
        ])
        .unwrap();
        header.dynamic_range = dynf;
        header.crc_present = false;
        header.header_crc = None;
        header.aspf = false;
        let mut bytes = crate::encode_frame_header_be(&header).unwrap();

        // §5.3.2 ACH: one channel, nSUBS=2/nVQSUB=2, BHUFF=6 Linear5Bit,
        // SEL plane all zero (every group transmits a 2-bit ADJ), 10 ADJ.
        let mut body: Vec<(u32, u8)> = vec![
            (0, 4), // SUBFS -> 1 subframe
            (0, 3), // PCHS -> 1 channel
            (0, 5), // SUBS -> nSUBS 2
            (1, 5), // VQSUB -> nVQSUB 2
            (0, 3), // JOINX
            (0, 2), // THUFF
            (0, 3), // SHUFF
            (6, 3), // BHUFF=6 Linear5Bit
        ];
        body.push((0, 1)); // SEL ABITS1
        for _ in 1..5 {
            body.push((0, 2));
        }
        for _ in 5..10 {
            body.push((0, 3));
        }
        for _ in 0..10 {
            body.push((0, 2)); // ADJ
        }

        // §5.4.1 side info: SSC/PSC, 2 PMODE bits, 2 zero ABITS reads.
        body.push((0, 2)); // SSC -> nSSC 1
        body.push((0, 3)); // PSC
        body.push((0, 1)); // PMODE[0][0]
        body.push((0, 1)); // PMODE[0][1]
        body.push((0, 5)); // ABITS[0][0] = 0
        body.push((0, 5)); // ABITS[0][1] = 0

        // Table 5-28 tail: an 8-bit RANGE index when DYNF (CPF=0 so no
        // SICRC), then the §5.5 DSYNC trailer.
        if dynf {
            body.push((range_index as u32, 8));
        }
        body.push((0xffff, 16)); // DSYNC

        bytes.extend_from_slice(&pack_fields(&body));
        bytes.extend_from_slice(&[0u8; 4]);
        bytes
    }

    /// [`CoreStreamDecoder::decode_frame`] reproduces the standalone
    /// [`decode_core_frame`] result frame-for-frame (the per-frame body
    /// is the shared [`SubframePcmDecoder::decode_core_frame_into`]); the
    /// difference is only in the persistent filter state carried between
    /// calls, which an all-zero stream cannot expose, so this pins the
    /// per-frame equivalence.
    #[test]
    fn core_stream_decode_matches_decode_core_frame_per_frame() {
        let f0 = build_nobits_frame(false, 0);
        let f1 = build_nobits_frame(false, 0);
        let h0 = crate::parse_frame_header(&f0).unwrap();
        let h1 = crate::parse_frame_header(&f1).unwrap();

        let mut stream = CoreStreamDecoder::new(1);
        let s0 = stream.decode_frame(&f0, &h0).unwrap();
        let s1 = stream.decode_frame(&f1, &h1).unwrap();
        assert_eq!(stream.channel_count(), 1);

        // Each frame matches the fresh-per-frame decode (silent stream:
        // the carried filter tail is zero, so the two paths agree).
        assert_eq!(s0, decode_core_frame(&f0, &h0).unwrap());
        assert_eq!(s1, decode_core_frame(&f1, &h1).unwrap());
        assert_eq!(s0[0].len(), SAMPLES_PER_SUBSUBFRAME * PCM_PER_SUBBAND_ROW);
        assert!(s0[0].iter().all(|&v| v == 0));
    }

    /// [`CoreStreamDecoder`] reuses one persistent per-channel §C.2.5
    /// filter across frames rather than resetting it — the structural
    /// precondition for inter-frame filter continuity. (The end-to-end
    /// proof that this makes our PCM shape-identical to a black-box
    /// `ffmpeg -c:a dca` reference decode of a real multi-frame stream is
    /// the `decodes_real_fixture_stream_matching_ffmpeg_shape`
    /// integration test; with non-zero §5.5 audio the carried tail
    /// changes the next frame's leading samples, which an all-`ABITS==0`
    /// synthetic frame cannot exercise.)
    #[test]
    fn core_stream_reuses_persistent_filter_across_frames() {
        let f0 = build_nobits_frame(false, 0);
        let f1 = build_nobits_frame(false, 0);
        let h0 = crate::parse_frame_header(&f0).unwrap();
        let h1 = crate::parse_frame_header(&f1).unwrap();
        let mut stream = CoreStreamDecoder::new(1);

        // The same filter object (and its history) must survive a decode:
        // a silent stream leaves the history all-zero, so we assert the
        // decoder neither panics nor reallocates the channel filters.
        let _ = stream.decode_frame(&f0, &h0).unwrap();
        assert_eq!(stream.subframe_decoder().qmf().channel_count(), 1);
        let _ = stream.decode_frame(&f1, &h1).unwrap();
        assert_eq!(stream.subframe_decoder().qmf().channel_count(), 1);
        assert!(
            stream
                .subframe_decoder()
                .qmf()
                .channels()
                .iter()
                .all(|q| q.x_history().iter().all(|&v| v == 0.0))
        );
    }

    /// A [`CoreStreamDecoder`] built for the wrong channel count rejects
    /// a frame whose `nPCHS` disagrees, without panicking.
    #[test]
    fn core_stream_channel_count_mismatch_rejected() {
        let frame = build_nobits_frame(false, 0);
        let header = crate::parse_frame_header(&frame).unwrap();
        // The frame is one channel; a 2-channel decoder must decline.
        let mut stream = CoreStreamDecoder::new(2);
        let err = stream.decode_frame(&frame, &header).unwrap_err();
        assert!(matches!(
            err,
            CoreFrameDecodeError::Decode(SubframePcmError::ChannelCountMismatch {
                expected: 2,
                got: 1
            })
        ));
    }

    /// A pure unit test of the §C.2.4 matrix: feeding a two-channel pair
    /// whose sub-band samples are `(L+R, L-R)` back through
    /// `apply_sum_difference` recovers `(2L, 2R)` — the matrix is
    /// self-inverse up to the factor of two the encoder's scale factors
    /// absorb. Only the active sub-band columns are touched.
    #[test]
    fn apply_sum_difference_recovers_double_original() {
        // Two "original" channels, 3 sample rows, 2 active sub-bands.
        let l = [[1.0_f64, 2.0], [3.0, 4.0], [5.0, 6.0]];
        let r = [[10.0_f64, 20.0], [30.0, 40.0], [50.0, 60.0]];
        let mut matrices: Vec<SubbandSampleMatrix> = vec![vec![[0.0; NUM_SUBBAND]; 3]; 2];
        for row in 0..3 {
            for n in 0..2 {
                // Encoder side: store (L+R) in ch0, (L-R) in ch1.
                matrices[0][row][n] = l[row][n] + r[row][n];
                matrices[1][row][n] = l[row][n] - r[row][n];
            }
            // A high (inactive) sub-band that must be left untouched.
            matrices[0][row][20] = 7.0;
            matrices[1][row][20] = 9.0;
        }
        apply_sum_difference(&mut matrices, 0, 1, &[2, 2]).unwrap();
        for row in 0..3 {
            for n in 0..2 {
                assert_eq!(
                    matrices[0][row][n],
                    2.0 * l[row][n],
                    "ch0 row {row} sub {n}"
                );
                assert_eq!(
                    matrices[1][row][n],
                    2.0 * r[row][n],
                    "ch1 row {row} sub {n}"
                );
            }
            // Inactive sub-band 20 (>= nSUBS) untouched.
            assert_eq!(matrices[0][row][20], 7.0);
            assert_eq!(matrices[1][row][20], 9.0);
        }
    }

    /// End-to-end: forcing the `SUMF` flag on a real fixture frame routes
    /// the §C.2.4 front sum/difference decode through the full
    /// reconstruction chain. The bundled fixture's two channels are
    /// identical at the sub-band level (`L == R`), so the matrix produces
    /// `(L+R, L-R) = (2L, 0)`: the difference channel decodes to **exact
    /// silence**, and — since the §C.2.5 QMF is linear over cleared
    /// per-frame history — the sum channel is twice the un-summed decode
    /// (within the ±1 truncation of the integer output cast).
    #[test]
    fn sumf_forced_zeros_difference_channel_on_real_fixture() {
        const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/dts_5_frames.bin");
        let frame = &FIXTURE[0..1024];
        let header = crate::parse_frame_header(frame).unwrap();
        // The fixture is AMODE 2 (Stereo) with the sum flags clear.
        assert_eq!(header.amode_arrangement(), AmodeArrangement::Stereo);
        assert!(!header.front_sum && !header.surround_sum);

        // Baseline decode (fresh, cleared history).
        let base = decode_core_frame(frame, &header).unwrap();
        assert_eq!(base.len(), 2);
        assert_eq!(base[0], base[1], "fixture channels are identical");
        assert!(base[0].iter().any(|&s| s != 0), "baseline is non-silent");

        // Force SUMF and decode the same bytes.
        let mut sumf_header = header;
        sumf_header.front_sum = true;
        let sumf = decode_core_frame(frame, &sumf_header).unwrap();

        // Difference channel (ch1 = L - R = 0) is exactly silent.
        assert!(
            sumf[1].iter().all(|&s| s == 0),
            "SUMF difference channel must decode to exact silence when L == R"
        );
        // Sum channel (ch0 = L + R = 2L) ~= twice the baseline, within the
        // integer output cast's ±1 truncation slack.
        for (i, (&s, &b)) in sumf[0].iter().zip(&base[0]).enumerate() {
            let diff = (i64::from(s) - 2 * i64::from(b)).abs();
            assert!(
                diff <= 1,
                "sum channel sample {i}: got {s}, expected ~{} (2x baseline)",
                2 * b
            );
        }
        assert!(sumf[0].iter().any(|&s| s != 0), "sum channel is non-silent");
    }
}

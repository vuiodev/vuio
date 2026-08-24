//! DTS Coherent Acoustics — §5.5 Primary Audio Data Arrays (`Audio
//! Data`) decode walk (ETSI TS 102 114 V1.3.1, Table 5-29, staged PDF
//! p.31-33).
//!
//! Round 340 (2026-06-19) composes the already-landed per-subband
//! primitives into the §5.5 `Audio Data` block: the per-subsubframe
//! nested loop that extracts the eight `AUDIO[m]` quantization indices
//! for every `(ch, n)` subband (dispatching on the round-258
//! [`AudioQuantType`] resolved from the `(ABITS, SEL)` pair), applies
//! the round-293 §5.5 `rScale · AUDIO[m]` transient-aware
//! dequantization, runs the round-228 §C.2.2 inverse-ADPCM predictor
//! where `PMODE != 0`, and consumes the §5.5 `DSYNC` trailers — all the
//! way to the per-channel subband-sample matrix
//! `aPrmCh[ch].aSubband[n].aSample[m]` the §C.2.5 QMF synthesis
//! consumes.
//!
//! The Table 5-29 `Audio Data` pseudocode (staged PDF p.31-32),
//! transcribed verbatim:
//!
//! ```text
//! for (nSubSubFrame=0; nSubSubFrame<nSSC; nSubSubFrame++) {
//!   for (ch=0; ch<nPCHS; ch++)
//!     for (n=0; n<nVQSUB[ch]; n++) {       // Not high-frequency VQ
//!       nABITS = ABITS[ch][n];
//!       nNumQ  = pCQGroupAUDIO[nABITS-1].nNumQ-1;
//!       nSEL   = SEL[ch][nABITS-1];
//!       nQType = 1;                         // Huffman by default
//!       if (nSEL == nNumQ) { nQType = (nABITS<=7) ? 3 : 2; }
//!       if (nABITS == 0)    nQType = 0;
//!       switch (nQType) {
//!         case 0: AUDIO[0..8] = 0;
//!         case 1: AUDIO[m] = Huffman(SEL);                  // ×8
//!         case 2: AUDIO[m] = SignExtension(Binary(width));  // ×8
//!         case 3: for (nBlock=0;nBlock<2;nBlock++)          // 2×4
//!                   BlockCode(nCode) -> AUDIO[m..m+4];
//!       }
//!       // dequant: rScale = rStepSize·SCALES[ch][n][transient];
//!       //          rScale *= arADJ[ch][SEL[ch][nABITS-1]];
//!       nSample = 8*nSubSubFrame;
//!       aSample[nSample+m] = rScale * AUDIO[m];             // m<8
//!       if (PMODE[ch][n] != 0) InverseADPCM();
//!     }
//!     if ((nSubSubFrame==nSSC-1) || (ASPF==1)) {
//!       DSYNC = ExtractBits(16);
//!       if (DSYNC != 0xffff) "DSYNC error";
//!     }
//! }
//! ```
//!
//! # Scope
//!
//! Two §5.5 sub-paths consume the Annex D §D.10 VQ code books, which
//! the ETSI spec deliberately omits ("Due to its extensive size, this
//! table is not included here", §D.10.1 / §D.10.2, PDF p.255) and
//! which are now staged as clean-room data and **built into the
//! crate** ([`crate::VqCodebooks::builtin`], round 439 — see
//! `docs/audio/dts/dts-d10-vq-tables-GAP.md`, CLOSED):
//!
//! * The **high-frequency VQ subbands** loop (`n ∈ [nVQSUB, nSUBS)`,
//!   `nVQIndex = ExtractBits(10); HFreqVQ.LookUp(...)`) uses the §D.10.2
//!   "High Frequency Subbands" 32-sample VQ code book (1024 vectors;
//!   entries decode as two 8-bit signed integers, low byte first,
//!   **each ÷ 2⁴** — [`crate::unpack_hfreq_vq_entry`]). Its 10-bit
//!   indices are captured structurally
//!   ([`crate::scan_hf_vq_indices_at`]) and the book supplied as an
//!   [`HfVqFill`] to [`decode_audio_data_subframe_vq_at`]
//!   reconstructs the subband (`SCALES[ch][n][0] · HFREQ[m]` over the
//!   subframe's rows).
//! * The **inverse-ADPCM coefficient lookup** (`PMODE != 0`, the §5.4.1
//!   `ADPCMCoeffVQ.LookUp(nVQIndex, PVQ[ch][n])`) uses the §D.10.1
//!   ADPCM-coefficient VQ code book (4096 × 4 stored integers, actual
//!   coefficient = entry ÷ 2¹³ — [`crate::adpcm_vq_coeff`]); with the
//!   book supplied as an [`AdpcmContext`] the §C.2.2
//!   predictor runs per subsubframe from the captured 12-bit
//!   `pvq_index`, primed by the persistent [`AdpcmHistory`].
//!
//! Without the matching book (a caller-stripped decoder,
//! [`crate::VqCodebooks::none`]) each sub-path surfaces the typed
//! [`AudioArrayError::VqCodebookUnavailable`] refusal. A frame
//! whose primary channels are all linearly / Huffman / block coded with
//! `PMODE == 0` and `nVQSUB == nSUBS` (the common Core case) decodes to
//! PCM end-to-end with no books at all.

use crate::audio_data::{audio_quant_type, AudioQuantType};
use crate::audio_huff::{decode_audio_huff_at, AudioHuffCodebook};
use crate::bitreader::BitReader;
use crate::block_code::decode_block_code;
use crate::cos_mod::NUM_SUBBAND;
use crate::d10_vq::{AdpcmVqCodebook, HfVqCodebook};
use crate::dsync::DSYNC_WORD;
use crate::inverse_adpcm::{inverse_adpcm_decode_f64, NUM_ADPCM_COEFF};
use crate::side_info::ScaleFactorAdjustment;
use crate::step_size::{transient_scale_index, StepSizeTable, SAMPLES_PER_SUBSUBFRAME};
use crate::subframe::ChannelSideInfo;
use crate::{Error, Result};

/// Errors specific to the §5.5 audio-data array walk that are not
/// already covered by the crate-level [`Error`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AudioArrayError {
    /// A subband required an Annex D §D.10 VQ code book that the
    /// caller stripped from the decoder
    /// ([`crate::VqCodebooks::none`]; the built-in books are the
    /// default since round 439, so this fires only on an explicit
    /// opt-out). Either the §D.10.2 high-frequency VQ book (a
    /// `nVQSUB < nSUBS` subband) or the §D.10.1 ADPCM-coefficient VQ
    /// book (a `PMODE != 0` subband). Carries the channel/subband
    /// that hit the blocker and which book is missing.
    VqCodebookUnavailable {
        /// 0-based channel index.
        ch: usize,
        /// 0-based subband index.
        n: usize,
        /// `true` = high-frequency VQ (§D.10.2); `false` = ADPCM
        /// coefficient VQ (§D.10.1).
        high_frequency_vq: bool,
    },
    /// The §5.5 LFE phase (§2.2) dequant failed — a reserved §D.1.2
    /// `RMS_7BIT` scale index or an absent LFE channel
    /// ([`crate::LfeChannelError`]).
    LfePhase(crate::LfeChannelError),
    /// The caller-supplied §5.5 phase-1 HF-VQ index capture does not
    /// match the per-channel `[nVQSUB, nSUBS)` shape the walk needs
    /// (wrong channel count or wrong per-channel index count).
    HfVqIndexShape {
        /// 0-based channel index whose captured indices mismatched
        /// (equal to the channel count when the outer capture is the
        /// wrong length).
        ch: usize,
    },
    /// A `PMODE != 0` subband carried no captured 12-bit `PVQ` index
    /// (a structurally impossible [`ChannelSideInfo`] — the §5.4.1
    /// walk always captures the index when the PMODE bit is set —
    /// so this only surfaces on hand-built side info).
    MissingPvqIndex {
        /// 0-based channel index.
        ch: usize,
        /// 0-based subband index.
        n: usize,
    },
}

impl core::fmt::Display for AudioArrayError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AudioArrayError::VqCodebookUnavailable {
                ch,
                n,
                high_frequency_vq,
            } => {
                let book = if *high_frequency_vq {
                    "§D.10.2 high-frequency VQ"
                } else {
                    "§D.10.1 ADPCM-coefficient VQ"
                };
                write!(
                    f,
                    "oxideav-dts: channel {ch} subband {n} needs the {book} code \
                     book, which this decoder was configured without \
                     (VqCodebooks::none(); the built-in books are the default)"
                )
            }
            AudioArrayError::LfePhase(e) => write!(f, "oxideav-dts: §5.5 LFE phase: {e}"),
            AudioArrayError::HfVqIndexShape { ch } => write!(
                f,
                "oxideav-dts: §5.5 phase-1 HF-VQ index capture shape mismatch \
                 at channel {ch}"
            ),
            AudioArrayError::MissingPvqIndex { ch, n } => write!(
                f,
                "oxideav-dts: channel {ch} subband {n} has PMODE set but no \
                 captured §5.4.1 PVQ index"
            ),
        }
    }
}

impl std::error::Error for AudioArrayError {}

/// The §D.6 `V…` block-code-book word width (in bits) for the `ABITS`
/// family `1..=7`, read off the §D.6 table titles (staged PDF
/// p.231-236): `V3` 7-bit, `V5` 10-bit, `V7` 12-bit, `V9` 13-bit,
/// `V13` 15-bit, `V17` 17-bit, `V25` 19-bit. Each block-code word
/// expands to four samples.
fn block_code_word_bits(abits: u8) -> Option<u32> {
    Some(match abits {
        1 => 7,
        2 => 10,
        3 => 12,
        4 => 13,
        5 => 15,
        6 => 17,
        7 => 19,
        _ => return None,
    })
}

/// The §5.5 "No Further Encoding" (NFE) binary-code word width (in
/// bits) for an `ABITS` index, sign-extended on read. Table 5-26's
/// even "or 2ⁿ" level forms (PDF p.27) give `2^(ABITS-3)` levels for
/// `ABITS ∈ 8..=26` (e.g. ABITS 8 → 32 = 2⁵, ABITS 26 → 2²³), so the
/// binary code carries `ABITS - 3` bits. For `ABITS > 26` (the
/// no-SEL-transmitted region) the same `ABITS - 3` width holds up to
/// the 32-bit reader bound.
fn nfe_word_bits(abits: u8) -> Option<u32> {
    if abits < 8 {
        return None;
    }
    let bits = u32::from(abits) - 3;
    if (1..=32).contains(&bits) {
        Some(bits)
    } else {
        None
    }
}

/// Sign-extend a `width`-bit two's-complement field read as an
/// unsigned integer (`pCQGroup->ppQ[nSEL]->SignExtension(nCode)`).
fn sign_extend(value: u32, width: u32) -> i32 {
    debug_assert!((1..=32).contains(&width));
    let shift = 32 - width;
    ((value << shift) as i32) >> shift
}

/// Extract one subband's `count` `AUDIO[m]` quantization indices for
/// one subsubframe from `br`, dispatching on the `(abits, sel)` pair
/// per the §5.5 Table 5-29 `switch (nQType)`.
///
/// `count` is 8 for a normal subsubframe and `PSC ∈ 1..=7` for the
/// trailing **partial** subsubframe of a termination frame (§5.4.1
/// PSC, PDF p.30: "PSC indicates the number of subband samples held
/// in a partial subsubframe for each of the active subbands").
///
/// * [`AudioQuantType::NoBits`] — `count` zeros, no bits read.
/// * [`AudioQuantType::Huffman`] — `count` §D.5 Huffman-coded indices
///   (the code is per-sample, so a partial subsubframe extracts
///   exactly `count` codewords).
/// * [`AudioQuantType::NoEncoding`] — `count` sign-extended
///   binary-code fields of [`nfe_word_bits`] width (likewise
///   per-sample).
/// * [`AudioQuantType::BlockCode`] — [`block_code_word_bits`]-wide
///   block-code words, each expanding to **four** samples; a partial
///   subsubframe extracts `ceil(count / 4)` words and keeps the first
///   `count` decoded samples. The four-sample word is indivisible, so
///   the encoder pads the trailing word the same way the spec
///   documents for the other fixed-span carrier (§5.5 HFREQ, PDF
///   p.33: samples beyond the subframe "are padded with either zeros
///   or 'don't care' and then vector-quantized" and the decoder "will
///   only pick" the live ones).
fn extract_subband_audio(
    br: &mut BitReader<'_>,
    abits: u8,
    sel: u8,
    count: usize,
) -> Result<[i32; SAMPLES_PER_SUBSUBFRAME]> {
    debug_assert!((1..=SAMPLES_PER_SUBSUBFRAME).contains(&count));
    let mut audio = [0_i32; SAMPLES_PER_SUBSUBFRAME];
    match audio_quant_type(abits, sel) {
        AudioQuantType::NoBits => {}
        AudioQuantType::Huffman => {
            // SEL selects the §D.5 book within the ABITS group.
            let codebook = AudioHuffCodebook::from_abits_sel(abits, sel)
                .ok_or(Error::HuffmanDecodeFailed { table: "AUDIO" })?;
            for slot in audio.iter_mut().take(count) {
                let level = decode_audio_huff_in(br, codebook)?;
                *slot = i32::from(level);
            }
        }
        AudioQuantType::NoEncoding => {
            let width = nfe_word_bits(abits).ok_or(Error::InvalidStepSize { abits })?;
            for slot in audio.iter_mut().take(count) {
                let raw = br.read_bits(width)?;
                *slot = sign_extend(raw, width);
            }
        }
        AudioQuantType::BlockCode => {
            let width = block_code_word_bits(abits).ok_or(Error::InvalidStepSize { abits })?;
            let n_levels = u32::from(crate::audio_data::QUANT_LEVELS[abits as usize]);
            let mut m = 0usize;
            while m < count {
                let code = br.read_bits(width)?;
                if count - m >= 4 {
                    decode_block_code(code, n_levels, &mut audio[m..m + 4])?;
                } else {
                    // Trailing partial word: decode all four samples,
                    // keep only the live `count - m` (the rest are the
                    // encoder's pad).
                    let mut word = [0_i32; 4];
                    decode_block_code(code, n_levels, &mut word)?;
                    audio[m..count].copy_from_slice(&word[..count - m]);
                }
                m += 4;
            }
        }
    }
    Ok(audio)
}

/// Decode one §D.5 Huffman `AUDIO[m]` index through a `BitReader`
/// already positioned mid-stream (the [`decode_audio_huff_at`]
/// byte-offset entry point re-seeks from a byte boundary, which the
/// per-subsubframe walk cannot do because it shares one running
/// reader). This re-walks the book bit-at-a-time from `br`.
fn decode_audio_huff_in(br: &mut BitReader<'_>, codebook: AudioHuffCodebook) -> Result<i16> {
    // Bridge through the byte-offset API by re-reading from the
    // current absolute bit position over the same backing buffer.
    // `decode_audio_huff_at` borrows the buffer immutably and reports
    // bits_consumed; we then advance `br` by that many bits.
    let pos = br.absolute_bit_position();
    let bytes = br.backing_bytes();
    let (level, consumed) = decode_audio_huff_at(bytes, pos, codebook)?;
    br.skip_bits(consumed as u32)?;
    Ok(level)
}

/// Per-channel decoded subband-sample matrix for one subframe: row `s`
/// (`s ∈ 0..n_ssc*8`) is the §C.2.5 per-sample subband vector
/// `[aSubband[0].aSample[s], …, aSubband[31].aSample[s]]` for one
/// channel. The QMF synthesis consumes this directly.
pub type SubbandSampleMatrix = Vec<[f64; NUM_SUBBAND]>;

/// Decode the §5.5 LFE phase (the `if (LFF > 0) { … }` block of the
/// `docs/audio/dts/dts-lfe-interpolation-and-audio-walker.md` §2.2
/// walker) for one subframe, returning the interpolated LFE PCM and the
/// number of bits consumed.
///
/// The LFE phase sits between the high-frequency-VQ phase (§2.1, empty
/// for the accepted Core case where `nVQSUB == nSUBS`) and the
/// per-subsubframe audio-data phase (§2.3). It reads `2·LFF·nSSC` 8-bit
/// two's-complement decimated LFE samples followed by an 8-bit
/// `LFEscaleIndex`, dequantises (`rLFE[n] = LFE[n]·nScale·0.035` with the
/// §D.1.2 `RMS_7BIT` scale), then upsamples via the §C.2.6
/// `InterpolationFIR(LFF)` polyphase convolution ([`crate::LfeChannel`]).
///
/// * `bytes` / `bit_offset` — positioned at the first LFE-phase bit.
/// * `lff` — the frame header's non-zero `LFF` (1 → 128×, 2 → 64×).
/// * `n_ssc` — the subframe's subsubframe count (`SSC + 1`).
/// * `lfe` — the persistent per-channel [`crate::LfeChannel`] whose
///   §C.2.6 history carries across subframes.
///
/// Returns `(lfe_pcm, bits_consumed)`. The PCM length is
/// `2·LFF·nSSC·(64 | 128)`.
///
/// # Errors
///
/// * [`Error::UnexpectedEof`] on a truncated LFE region;
/// * [`AudioArrayError::LfePhase`] wrapping a [`crate::LfeChannelError`]
///   (a reserved §D.1.2 scale index, or `lff == 0`).
pub fn decode_lfe_phase_at(
    bytes: &[u8],
    bit_offset: usize,
    lff: u8,
    n_ssc: usize,
    lfe: &mut crate::LfeChannel,
) -> core::result::Result<(Vec<i32>, usize), AudioArrayDecodeError> {
    let byte_offset = bit_offset / 8;
    let intra_byte = bit_offset % 8;
    let mut br = BitReader::from_byte_offset(bytes, byte_offset);
    if intra_byte > 0 {
        br.read_bits(intra_byte as u32)?;
    }

    // 2·LFF·nSSC 8-bit two's-complement decimated LFE samples.
    let n_lfe = 2 * (lff as usize) * n_ssc;
    let mut samples: Vec<i8> = Vec::with_capacity(n_lfe);
    for _ in 0..n_lfe {
        // ExtractBits(8) read as a signed char.
        samples.push(br.read_bits(8)? as u8 as i8);
    }

    // 8-bit LFEscaleIndex.
    let scale_index = br.read_bits(8)? as u8;

    let bits_consumed = br.absolute_bit_position() - bit_offset;

    let pcm = lfe
        .decode_subframe(&samples, scale_index, lff)
        .map_err(AudioArrayError::LfePhase)?;

    Ok((pcm, bits_consumed))
}

/// Persistent per-channel, per-subband §C.2.2 reconstruction history
/// — the four most recently reconstructed subband samples that prime
/// the inverse-ADPCM predictor of the next decode block ("history
/// from last subframe or subsubframe", §C.2.2; "the decoder will use
/// reconstruction history of the previous frame if HFLAG = 1",
/// §5.3.1).
///
/// The walk updates it from every decoded subframe's final rows
/// (whether or not any subband was predicted, since any subband may
/// turn `PMODE` on in a later subframe); the frame-level driver
/// clears it at a frame boundary whose header says `HFLAG = 0`
/// (entry-point frames are coded without the previous frame's
/// predictor history).
#[derive(Debug, Clone, PartialEq)]
pub struct AdpcmHistory {
    /// `per_channel[ch][n]` = the §C.2.2 `raSample[-4..0)` slots of
    /// channel `ch`, subband `n`, **oldest first** (slot 0 =
    /// `raSample[-4]`, slot 3 = `raSample[-1]`).
    per_channel: Vec<[[f64; NUM_ADPCM_COEFF]; NUM_SUBBAND]>,
}

impl AdpcmHistory {
    /// Cleared history for `channels` primary channels (the state of
    /// a stream entry point: "the history will be ignored" when
    /// `HFLAG = 0`, i.e. treated as zero).
    #[must_use]
    pub fn new(channels: usize) -> Self {
        Self {
            per_channel: vec![[[0.0; NUM_ADPCM_COEFF]; NUM_SUBBAND]; channels],
        }
    }

    /// The configured channel count.
    #[must_use]
    pub fn channel_count(&self) -> usize {
        self.per_channel.len()
    }

    /// Zero every subband's history (the §5.3.1 `HFLAG = 0` frame
    /// gate: "Otherwise, the history will be ignored").
    pub fn clear(&mut self) {
        for ch in &mut self.per_channel {
            *ch = [[0.0; NUM_ADPCM_COEFF]; NUM_SUBBAND];
        }
    }

    /// The four-sample history of one `(ch, n)` subband, oldest
    /// first.
    #[must_use]
    pub fn subband(&self, ch: usize, n: usize) -> &[f64; NUM_ADPCM_COEFF] {
        &self.per_channel[ch][n]
    }

    /// Slide every subband's history forward over a decoded
    /// subframe's reconstructed sample matrices (`matrices[ch]` with
    /// `rows` rows): the last four rows become the new history, with
    /// the short-subframe (`rows < 4`) shift semantics of
    /// [`crate::update_history_f64`].
    pub fn absorb_matrices(&mut self, matrices: &[SubbandSampleMatrix]) {
        for (ch_hist, matrix) in self.per_channel.iter_mut().zip(matrices) {
            let rows = matrix.len();
            let take = rows.min(NUM_ADPCM_COEFF);
            for (n, hist) in ch_hist.iter_mut().enumerate() {
                if take < NUM_ADPCM_COEFF {
                    hist.copy_within(take.., 0);
                }
                for (k, row) in matrix[rows - take..].iter().enumerate() {
                    hist[NUM_ADPCM_COEFF - take + k] = row[n];
                }
            }
        }
    }
}

/// The §5.5 phase-1 high-frequency-VQ inputs for
/// [`decode_audio_data_subframe_vq_at`]: a recovered §D.10.2 book
/// plus the 10-bit indices captured (in walk order) by
/// [`crate::scan_hf_vq_indices_at`] from the region that precedes the
/// LFE phase.
#[derive(Debug, Clone, Copy)]
pub struct HfVqFill<'a> {
    /// The recovered §D.10.2 `HFreqVQ` book.
    pub book: &'a HfVqCodebook,
    /// `indices[ch]` = the captured `nVQIndex` values for channel
    /// `ch`'s subbands `nVQSUB[ch]..nSUBS[ch]`, in subband order.
    pub indices: &'a [Vec<u16>],
}

/// The §D.10.1 / §C.2.2 inverse-ADPCM inputs for
/// [`decode_audio_data_subframe_vq_at`]: a recovered coefficient book
/// plus the persistent per-subband reconstruction history the
/// predictor primes from (and which the walk advances).
#[derive(Debug)]
pub struct AdpcmContext<'a> {
    /// The recovered §D.10.1 `ADPCMCoeffVQ` book.
    pub book: &'a AdpcmVqCodebook,
    /// The persistent reconstruction history (advanced by the walk
    /// over **all** subbands, predicted or not).
    pub history: &'a mut AdpcmHistory,
}

/// Decode the §5.5 `Audio Data` block for one subframe, given the
/// already-decoded §5.4.1 side information and §5.3.2 header context.
///
/// Walks the Table 5-29 `nSubSubFrame × ch × n` loop, extracting and
/// dequantizing every primary subband, running inverse-ADPCM where
/// `PMODE != 0`, and consuming the `DSYNC` trailers. Returns one
/// [`SubbandSampleMatrix`] per channel (length `n_ssc * 8` rows).
///
/// * `bytes` / `bit_offset` — the bit stream positioned at the first
///   §5.5 `Audio Data` bit (after the §5.4.1 side-info block).
/// * `side` — the per-channel [`ChannelSideInfo`] (round-281).
/// * `sel` — `|ch, abits| -> u8`, the §5.3.2 `SEL[ch][nABITS-1]`
///   selector ([`crate::AudioCodingHeader::sel`]).
/// * `adj` — `|ch, abits| -> ScaleFactorAdjustment`, the §5.5
///   `arADJ[ch][SEL[ch][nABITS-1]]` multiplier
///   ([`crate::AudioCodingHeader::adj`]).
/// * `n_vqsub` / `n_subs` — per-channel loop bounds.
/// * `n_ssc` — the subframe's subsubframe count (`SSC + 1`).
/// * `table` — the §5.5 `RATE`-selected step-size table.
/// * `aspf` — the §5.3.1 Audio Sync-Word Insertion Flag (a `DSYNC`
///   trailer follows every subsubframe when set, else only the last).
///
/// Returns `(Vec<SubbandSampleMatrix>, bits_consumed)`.
///
/// # Errors
///
/// * [`Error::InvalidStepSize`] for an out-of-range `ABITS`;
/// * [`Error::HuffmanDecodeFailed`] on a corrupt audio Huffman prefix
///   or an `(ABITS, SEL)` pair with no §D.5 book;
/// * [`Error::DsyncMismatch`] when a `DSYNC` trailer is not `0xffff`;
/// * [`Error::UnexpectedEof`] on a truncated array.
///
/// VQ / ADPCM-coefficient blockers surface
/// [`AudioArrayError::VqCodebookUnavailable`] wrapped through the
/// [`AudioArrayDecodeError`] return type.
#[allow(clippy::too_many_arguments)]
pub fn decode_audio_data_subframe_at(
    bytes: &[u8],
    bit_offset: usize,
    side: &[ChannelSideInfo],
    sel: impl Fn(usize, u8) -> u8,
    adj: impl Fn(usize, u8) -> ScaleFactorAdjustment,
    n_vqsub: &[usize],
    n_subs: &[usize],
    n_ssc: usize,
    table: StepSizeTable,
    aspf: bool,
) -> core::result::Result<(Vec<SubbandSampleMatrix>, usize), AudioArrayDecodeError> {
    decode_audio_data_subframe_partial_at(
        bytes, bit_offset, side, sel, adj, n_vqsub, n_subs, n_ssc, 0, table, aspf,
    )
}

/// [`decode_audio_data_subframe_at`] with the §5.4.1 `PSC` (Partial
/// Subsubframe Sample Count) semantics of a **termination frame**
/// applied: when `psc ∈ 1..=7`, the **last** of the subframe's `n_ssc`
/// subsubframes is *partial* — it holds `psc` subband samples per
/// active subband instead of 8 (PDF p.30: "PSC indicates the number
/// of subband samples held in a partial subsubframe for each of the
/// active subbands. A partial subsubframe is one which has less than
/// 8 subband samples. It exists only in a termination frame and is
/// always at the end of last normal subsubframe. A DSYNC word will
/// always occur after a partial subsubframe.").
///
/// That the partial subsubframe is the last one **counted by** `nSSC`
/// (rather than an extra, uncounted tail after them) follows from the
/// staged spec's own ranges: a termination frame's `NBLKS` "can take
/// any value in its valid range" `[5, 127]` (PDF p.18), so the
/// minimum legal termination frame carries 6 subband-sample blocks —
/// which is expressible as `nSSC = 1` with a 6-sample partial
/// subsubframe but not as one full subsubframe *plus* a tail (8 + PSC
/// ≥ 8 > 6); and §5.2's frame layout caps a subframe at "up to 4
/// subsubframes" (PDF p.16), which an uncounted fifth tail after
/// `nSSC = 4` would violate.
///
/// The partial subsubframe changes only the last iteration of the
/// Table 5-29 sample loop:
///
/// * per-sample carriers (§D.5 Huffman, NFE binary) extract exactly
///   `psc` codewords per active subband;
/// * the four-sample §D.6 block-code carrier extracts
///   `ceil(psc / 4)` words and keeps the first `psc` samples (see
///   [`extract_subband_audio`]);
/// * `ABITS = 0` subbands extract nothing, as always;
/// * the `DSYNC` trailer placement is unchanged — after the last
///   (here: partial) subsubframe always, and after every subsubframe
///   when `ASPF` is set, which realises the p.30 "A DSYNC word will
///   always occur after a partial subsubframe" clause.
///
/// The returned matrices have `(n_ssc - 1) * 8 + psc` rows per
/// channel when `psc > 0` (the frame-level row budget is `NBLKS + 1`
/// across all subframes), and `bits_consumed` accounts exactly for
/// the truncated extraction.
///
/// `psc = 0` reproduces [`decode_audio_data_subframe_at`] verbatim.
/// `psc` is trusted to be `< 8` (it is a 3-bit wire field); the
/// termination-frame gating ("exists only in a termination frame")
/// is the frame-level caller's to enforce, since this walk does not
/// see the §5.3.1 `FTYPE`.
#[allow(clippy::too_many_arguments)]
pub fn decode_audio_data_subframe_partial_at(
    bytes: &[u8],
    bit_offset: usize,
    side: &[ChannelSideInfo],
    sel: impl Fn(usize, u8) -> u8,
    adj: impl Fn(usize, u8) -> ScaleFactorAdjustment,
    n_vqsub: &[usize],
    n_subs: &[usize],
    n_ssc: usize,
    psc: u8,
    table: StepSizeTable,
    aspf: bool,
) -> core::result::Result<(Vec<SubbandSampleMatrix>, usize), AudioArrayDecodeError> {
    decode_audio_data_subframe_vq_at(
        bytes, bit_offset, side, sel, adj, n_vqsub, n_subs, n_ssc, psc, table, aspf, None, None,
    )
}

/// [`decode_audio_data_subframe_partial_at`] with the two §D.10
/// VQ-book sub-paths **enabled** by caller-supplied recovered books:
///
/// * `hf` — the §5.5 phase-1 high-frequency-VQ reconstruction. The
///   10-bit indices (captured by [`crate::scan_hf_vq_indices_at`]
///   from the region *before* the LFE phase) select 32-element
///   §D.10.2 vectors, and each HF subband's samples are
///   `SCALES[ch][n][0] · HFREQ[ch][n][m]` for the subframe's `m`
///   rows. The Table 5-29 listing assigns
///   `Scale = (real)SCALES[ch][n][0]` and then multiplies by a
///   variable it spells `rScale` — a spec-verbatim naming conflation
///   (re-verified against the staged PDF by the round-9 extraction
///   pass) that the §5.5 HFREQ prose on p.33 resolves: the decoder
///   picks `nSSC × 8` of the 32 samples "and scale[s] them with the
///   scale factor SCALES". On a termination-frame subframe the valid
///   prefix (`(nSSC−1)·8 + PSC` rows) is picked instead — the p.33
///   pad rule ("padded with either zeros or 'don't care' … the
///   decoder will only pick" the live ones) makes the vector tail
///   don't-care.
/// * `adpcm` — the §5.5 `if (PMODE[ch][n] != 0) InverseADPCM()` step:
///   the four §C.2.2 predictor coefficients are looked up from the
///   §D.10.1 book by the subband's captured 12-bit `PVQ` index, and
///   the dequantized residuals of every subsubframe are reconstructed
///   in walk order, primed by the persistent [`AdpcmHistory`] (which
///   the walk advances over the subframe's final rows — for **all**
///   subbands, so a subband that turns `PMODE` on later still finds
///   its reconstruction history; the §5.3.1 `HFLAG` frame gate is the
///   frame-level caller's).
///
/// With `None` for a needed book the corresponding blocker surfaces
/// as before ([`AudioArrayError::VqCodebookUnavailable`]); with both
/// `None` this is exactly [`decode_audio_data_subframe_partial_at`].
///
/// # Errors
///
/// As [`decode_audio_data_subframe_partial_at`], plus
/// [`AudioArrayError::HfVqIndexShape`] when `hf` is supplied with a
/// capture that does not match the per-channel `[nVQSUB, nSUBS)`
/// shape, and [`AudioArrayError::MissingPvqIndex`] for a hand-built
/// `PMODE != 0` subband lacking its captured index.
#[allow(clippy::too_many_arguments)]
pub fn decode_audio_data_subframe_vq_at(
    bytes: &[u8],
    bit_offset: usize,
    side: &[ChannelSideInfo],
    sel: impl Fn(usize, u8) -> u8,
    adj: impl Fn(usize, u8) -> ScaleFactorAdjustment,
    n_vqsub: &[usize],
    n_subs: &[usize],
    n_ssc: usize,
    psc: u8,
    table: StepSizeTable,
    aspf: bool,
    hf: Option<HfVqFill<'_>>,
    mut adpcm: Option<AdpcmContext<'_>>,
) -> core::result::Result<(Vec<SubbandSampleMatrix>, usize), AudioArrayDecodeError> {
    let n_pchs = side.len();
    let psc = usize::from(psc) % SAMPLES_PER_SUBSUBFRAME;

    // Reject the VQ / ADPCM blockers up front so a partially-decoded
    // matrix is never returned. Each blocker is lifted exactly when
    // the matching recovered book is supplied.
    for (ch, ch_side) in side.iter().enumerate() {
        if n_vqsub[ch] < n_subs[ch] && hf.is_none() {
            return Err(AudioArrayError::VqCodebookUnavailable {
                ch,
                n: n_vqsub[ch],
                high_frequency_vq: true,
            }
            .into());
        }
        if let Some(n) = ch_side.pmode[..n_vqsub[ch]].iter().position(|&p| p != 0) {
            match &adpcm {
                None => {
                    return Err(AudioArrayError::VqCodebookUnavailable {
                        ch,
                        n,
                        high_frequency_vq: false,
                    }
                    .into());
                }
                Some(_) if ch_side.pvq_index[n].is_none() => {
                    return Err(AudioArrayError::MissingPvqIndex { ch, n }.into());
                }
                Some(_) => {}
            }
        }
    }
    if let Some(fill) = &hf {
        if fill.indices.len() != n_pchs {
            return Err(AudioArrayError::HfVqIndexShape { ch: n_pchs }.into());
        }
        for (ch, ch_indices) in fill.indices.iter().enumerate() {
            if ch_indices.len() != n_subs[ch] - n_vqsub[ch] {
                return Err(AudioArrayError::HfVqIndexShape { ch }.into());
            }
        }
    }

    // Row budget: the last subsubframe is partial (psc rows) on a
    // termination-frame subframe, full (8 rows) otherwise.
    let rows = if psc > 0 {
        (n_ssc - 1) * SAMPLES_PER_SUBSUBFRAME + psc
    } else {
        n_ssc * SAMPLES_PER_SUBSUBFRAME
    };
    let mut matrices: Vec<SubbandSampleMatrix> = vec![vec![[0.0_f64; NUM_SUBBAND]; rows]; n_pchs];

    // §5.5 phase 1 — high-frequency VQ subbands: fill the HF columns
    // from the recovered §D.10.2 book before the audio-data walk (the
    // indices were extracted from the bit stream ahead of the LFE
    // phase; the fill itself consumes no bits here).
    if let Some(fill) = &hf {
        for (ch, ch_indices) in fill.indices.iter().enumerate() {
            for (k, &index) in ch_indices.iter().enumerate() {
                let n = n_vqsub[ch] + k;
                // The p.33 HFREQ rule: pick the subframe's rows out of
                // the 32-sample vector, scaled by SCALES[ch][n][0].
                let scale = f64::from(side[ch].scales[n][0]);
                let vector = fill.book.vector(index);
                for (row, &element) in matrices[ch].iter_mut().zip(vector.iter().take(rows)) {
                    row[n] = scale * element;
                }
            }
        }
    }

    let byte_offset = bit_offset / 8;
    let intra_byte = bit_offset % 8;
    let mut br = BitReader::from_byte_offset(bytes, byte_offset);
    if intra_byte > 0 {
        br.read_bits(intra_byte as u32)?;
    }

    for subsubframe in 0..n_ssc {
        let base = subsubframe * SAMPLES_PER_SUBSUBFRAME;
        // §5.4.1 PSC: the last subsubframe of a termination-frame
        // subframe holds `psc < 8` samples per active subband.
        let count = if psc > 0 && subsubframe == n_ssc - 1 {
            psc
        } else {
            SAMPLES_PER_SUBSUBFRAME
        };
        for (ch, ch_side) in side.iter().enumerate() {
            let matrix = &mut matrices[ch];
            // `n` is the subband index, used to address ch_side.abits /
            // tmode / scales and matrix[row][n]; an enumerate() over any
            // single one would not capture the cross-array indexing.
            #[allow(clippy::needless_range_loop)]
            for n in 0..n_vqsub[ch] {
                let abits = ch_side.abits[n];
                let sel_val = sel(ch, abits);
                let audio = extract_subband_audio(&mut br, abits, sel_val, count)?;

                // §5.5 transient-aware rScale composition.
                let scale_idx = transient_scale_index(ch_side.tmode[n], n_ssc, subsubframe);
                let scale = ch_side.scales[n][scale_idx];
                let step = table.step_size(abits)?;
                let r_scale = step * f64::from(scale) * adj(ch, abits).multiplier_f64();

                for (m, &index) in audio.iter().enumerate().take(count) {
                    matrix[base + m][n] = r_scale * f64::from(index);
                }

                // §5.5: "if (PMODE[ch][n] != 0)
                // aPrmCh[ch].aSubband[n].InverseADPCM();" — the four
                // §C.2.2 coefficients come from the recovered §D.10.1
                // book via the subband's captured PVQ index; the
                // history is the four samples preceding this
                // subsubframe (earlier rows of this subframe, else
                // the persistent inter-subframe history).
                if ch_side.pmode[n] != 0 {
                    if let Some(ctx) = adpcm.as_mut() {
                        // Checked non-None in the pre-walk validation.
                        let pvq = ch_side.pvq_index[n].unwrap_or_default();
                        let coeffs = ctx.book.coefficients(pvq);
                        let mut hist = [0.0_f64; NUM_ADPCM_COEFF];
                        for (j, slot) in hist.iter_mut().enumerate() {
                            // Logical row `base - 4 + j`.
                            *slot = if base + j >= NUM_ADPCM_COEFF {
                                matrix[base + j - NUM_ADPCM_COEFF][n]
                            } else {
                                ctx.history.subband(ch, n)[j]
                            };
                        }
                        let mut block = [0.0_f64; SAMPLES_PER_SUBSUBFRAME];
                        for (m, slot) in block.iter_mut().enumerate().take(count) {
                            *slot = matrix[base + m][n];
                        }
                        inverse_adpcm_decode_f64(&hist, coeffs, &mut block[..count])?;
                        for (m, &value) in block.iter().enumerate().take(count) {
                            matrix[base + m][n] = value;
                        }
                    }
                }
            }
        }
        // DSYNC trailer: present after the last subsubframe always, and
        // after every subsubframe when ASPF == 1.
        if subsubframe == n_ssc - 1 || aspf {
            let dsync = br.read_bits(16)? as u16;
            if dsync != DSYNC_WORD {
                return Err(Error::DsyncMismatch {
                    found: dsync,
                    n_subsubframe: subsubframe as u8,
                }
                .into());
            }
        }
    }

    // Advance the persistent §C.2.2 reconstruction history over this
    // subframe's final rows (all subbands — see [`AdpcmHistory`]).
    if let Some(ctx) = adpcm.as_mut() {
        ctx.history.absorb_matrices(&matrices);
    }

    let bits_consumed = br.absolute_bit_position() - bit_offset;
    Ok((matrices, bits_consumed))
}

/// Composite error for the §5.5 audio-data walk: either a crate-level
/// bit-stream [`Error`] or an [`AudioArrayError`] VQ/ADPCM blocker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AudioArrayDecodeError {
    /// A bit-stream-level decode error (EOF, bad Huffman prefix,
    /// invalid step size, DSYNC mismatch, …).
    Bitstream(Error),
    /// A subband needed an Annex D VQ code book not yet in `docs/`.
    Blocked(AudioArrayError),
}

impl From<Error> for AudioArrayDecodeError {
    fn from(e: Error) -> Self {
        AudioArrayDecodeError::Bitstream(e)
    }
}

impl From<AudioArrayError> for AudioArrayDecodeError {
    fn from(e: AudioArrayError) -> Self {
        AudioArrayDecodeError::Blocked(e)
    }
}

impl core::fmt::Display for AudioArrayDecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AudioArrayDecodeError::Bitstream(e) => write!(f, "{e}"),
            AudioArrayDecodeError::Blocked(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for AudioArrayDecodeError {}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn sign_extend_round_trips() {
        assert_eq!(sign_extend(0b011, 3), 3);
        assert_eq!(sign_extend(0b111, 3), -1);
        assert_eq!(sign_extend(0b100, 3), -4);
        assert_eq!(sign_extend(0, 5), 0);
    }

    #[test]
    fn nfe_and_block_widths() {
        assert_eq!(nfe_word_bits(8), Some(5)); // 32 levels
        assert_eq!(nfe_word_bits(11), Some(8)); // 256 levels
        assert_eq!(nfe_word_bits(26), Some(23));
        assert_eq!(nfe_word_bits(7), None);
        assert_eq!(block_code_word_bits(1), Some(7)); // V3
        assert_eq!(block_code_word_bits(7), Some(19)); // V25
        assert_eq!(block_code_word_bits(8), None);
    }

    /// A single-channel, single-subsubframe, no-bits subband stream
    /// decodes to an all-zero matrix and a single DSYNC trailer.
    #[test]
    fn no_bits_subband_zeroes_matrix() {
        // nSSC = 1, one channel, nVQSUB = nSUBS = 1, ABITS = 0.
        let side = vec![ChannelSideInfo::cleared()];
        let stream = pack_fields(&[(0xffff, 16)]); // just the DSYNC
        let (mats, bits) = decode_audio_data_subframe_at(
            &stream,
            0,
            &side,
            |_, _| 0,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            1,
            StepSizeTable::Lossy,
            false,
        )
        .unwrap();
        assert_eq!(mats.len(), 1);
        assert_eq!(mats[0].len(), 8);
        assert!(mats[0].iter().all(|row| row.iter().all(|&v| v == 0.0)));
        assert_eq!(bits, 16);
    }

    /// A NoEncoding (NFE) subband with ABITS 8 reads eight 5-bit
    /// sign-extended fields and scales them by the dequant rScale.
    #[test]
    fn nfe_subband_dequantizes() {
        let mut ch = ChannelSideInfo::cleared();
        ch.abits[0] = 8; // NFE width 5; lossy step for 8 = 796918/2^22
        ch.scales[0][0] = 4;
        let side = vec![ch];

        // Eight 5-bit values: 1,-1,2,-2,3,-3,4,-4 (two's complement).
        let vals = [1i32, -1, 2, -2, 3, -3, 4, -4];
        let mut fields: Vec<(u32, u8)> = vals.iter().map(|&v| ((v as u32) & 0x1f, 5u8)).collect();
        fields.push((0xffff, 16)); // DSYNC
        let stream = pack_fields(&fields);

        // SEL must select the terminal NFE entry for ABITS 8 (group of
        // 8 -> top SEL 7).
        let (mats, _) = decode_audio_data_subframe_at(
            &stream,
            0,
            &side,
            |_, _| 7,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            1,
            StepSizeTable::Lossy,
            false,
        )
        .unwrap();
        let step = StepSizeTable::Lossy.step_size(8).unwrap();
        let r = step * 4.0;
        for (m, &v) in vals.iter().enumerate() {
            assert!((mats[0][m][0] - r * f64::from(v)).abs() < 1e-9);
        }
    }

    /// A bad DSYNC surfaces a typed mismatch.
    #[test]
    fn bad_dsync_rejected() {
        let side = vec![ChannelSideInfo::cleared()];
        let stream = pack_fields(&[(0x1234, 16)]);
        let err = decode_audio_data_subframe_at(
            &stream,
            0,
            &side,
            |_, _| 0,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            1,
            StepSizeTable::Lossy,
            false,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            AudioArrayDecodeError::Bitstream(Error::DsyncMismatch { found: 0x1234, .. })
        ));
    }

    /// A subband with high-frequency VQ (nVQSUB < nSUBS) is blocked.
    #[test]
    fn high_frequency_vq_blocked() {
        let side = vec![ChannelSideInfo::cleared()];
        let err = decode_audio_data_subframe_at(
            &[0u8; 8],
            0,
            &side,
            |_, _| 0,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1], // nVQSUB
            &[3], // nSUBS > nVQSUB -> VQ subbands
            1,
            StepSizeTable::Lossy,
            false,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            AudioArrayDecodeError::Blocked(AudioArrayError::VqCodebookUnavailable {
                high_frequency_vq: true,
                ..
            })
        ));
    }

    /// A PMODE-active subband is blocked on the §D.10.1 coefficient VQ.
    #[test]
    fn adpcm_subband_blocked() {
        let mut ch = ChannelSideInfo::cleared();
        ch.pmode[0] = 1;
        let side = vec![ch];
        let err = decode_audio_data_subframe_at(
            &[0u8; 8],
            0,
            &side,
            |_, _| 0,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            1,
            StepSizeTable::Lossy,
            false,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            AudioArrayDecodeError::Blocked(AudioArrayError::VqCodebookUnavailable {
                high_frequency_vq: false,
                ..
            })
        ));
    }

    /// ASPF == 1 inserts a DSYNC after every subsubframe; two
    /// subsubframes therefore carry two trailers.
    #[test]
    fn aspf_inserts_dsync_each_subsubframe() {
        let side = vec![ChannelSideInfo::cleared()];
        // nSSC = 2, ABITS 0 -> no audio bits, two DSYNC trailers.
        let stream = pack_fields(&[(0xffff, 16), (0xffff, 16)]);
        let (_, bits) = decode_audio_data_subframe_at(
            &stream,
            0,
            &side,
            |_, _| 0,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            2,
            StepSizeTable::Lossy,
            true,
        )
        .unwrap();
        assert_eq!(bits, 32);
    }

    // -----------------------------------------------------------
    // §5.4.1 PSC — termination-frame partial subsubframe.
    // -----------------------------------------------------------

    /// `psc = 0` through the partial entry point is bit-for-bit the
    /// normal walk: same matrices, same bit count.
    #[test]
    fn psc_zero_is_identity_with_normal_walk() {
        let mut ch = ChannelSideInfo::cleared();
        ch.abits[0] = 8;
        ch.scales[0][0] = 4;
        let side = vec![ch];
        let vals = [1i32, -1, 2, -2, 3, -3, 4, -4];
        let mut fields: Vec<(u32, u8)> = vals.iter().map(|&v| ((v as u32) & 0x1f, 5u8)).collect();
        fields.push((0xffff, 16));
        let stream = pack_fields(&fields);

        let normal = decode_audio_data_subframe_at(
            &stream,
            0,
            &side,
            |_, _| 7,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            1,
            StepSizeTable::Lossy,
            false,
        )
        .unwrap();
        let partial = decode_audio_data_subframe_partial_at(
            &stream,
            0,
            &side,
            |_, _| 7,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            1,
            0,
            StepSizeTable::Lossy,
            false,
        )
        .unwrap();
        assert_eq!(normal, partial);
    }

    /// NFE (per-sample binary) partial subsubframe: `nSSC = 2`,
    /// `PSC = 3` extracts 8 + 3 five-bit fields, returns 11 rows, and
    /// the bit budget is exactly `11·5 + 16` (one DSYNC).
    #[test]
    fn psc_nfe_truncates_rows_and_bits_exactly() {
        let mut ch = ChannelSideInfo::cleared();
        ch.abits[0] = 8;
        ch.scales[0][0] = 4;
        let side = vec![ch];

        let vals = [1i32, -1, 2, -2, 3, -3, 4, -4, 5, -5, 6];
        let mut fields: Vec<(u32, u8)> = vals.iter().map(|&v| ((v as u32) & 0x1f, 5u8)).collect();
        fields.push((0xffff, 16)); // DSYNC after the partial subsubframe
        let stream = pack_fields(&fields);

        let (mats, bits) = decode_audio_data_subframe_partial_at(
            &stream,
            0,
            &side,
            |_, _| 7,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            2,
            3,
            StepSizeTable::Lossy,
            false,
        )
        .unwrap();
        assert_eq!(mats[0].len(), 11, "(nSSC-1)*8 + PSC rows");
        assert_eq!(bits, 11 * 5 + 16, "bit budget exact through truncation");
        let step = StepSizeTable::Lossy.step_size(8).unwrap();
        let r = step * 4.0;
        for (m, &v) in vals.iter().enumerate() {
            assert!((mats[0][m][0] - r * f64::from(v)).abs() < 1e-9);
        }
    }

    /// §D.6 block-code partial subsubframe, `PSC ≤ 4`: one four-sample
    /// word is extracted (`ceil(3/4) = 1`), the first three decoded
    /// samples are kept, the fourth (encoder pad) is discarded.
    #[test]
    fn psc_block_code_single_word_keeps_live_samples() {
        let mut ch = ChannelSideInfo::cleared();
        ch.abits[0] = 1; // V3: 3 levels, 7-bit word, 4 samples/word
        ch.scales[0][0] = 1;
        let side = vec![ch];

        // Base-3 digits LSD-first (element i = code%3 - 1): live
        // samples (+1, -1, 0), pad digit 2 (= +1, must be ignored).
        // code = 2 + 3·0 + 9·1 + 27·2 = 65.
        let stream = pack_fields(&[(65, 7), (0xffff, 16)]);

        let (mats, bits) = decode_audio_data_subframe_partial_at(
            &stream,
            0,
            &side,
            |_, _| 1, // terminal SEL for the ABITS=1 group -> block code
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            1,
            3,
            StepSizeTable::Lossy,
            false,
        )
        .unwrap();
        assert_eq!(mats[0].len(), 3);
        assert_eq!(bits, 7 + 16, "one 7-bit V3 word + DSYNC");
        let step = StepSizeTable::Lossy.step_size(1).unwrap();
        let got: Vec<f64> = (0..3).map(|m| mats[0][m][0]).collect();
        let want = [step, -step, 0.0];
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 1e-9, "got {got:?}");
        }
    }

    /// §D.6 block-code partial subsubframe, `PSC = 5`: two words
    /// (`ceil(5/4) = 2`), the second word contributes one live sample.
    #[test]
    fn psc_block_code_two_words_for_five_samples() {
        let mut ch = ChannelSideInfo::cleared();
        ch.abits[0] = 1;
        ch.scales[0][0] = 1;
        let side = vec![ch];

        // Word 1: (+1, +1, -1, -1) -> digits (2,2,0,0) -> 2 + 6 = 8.
        // Word 2: live (-1), pads 0 -> digits (0,1,1,1) -> 3+9+27 = 39.
        let stream = pack_fields(&[(8, 7), (39, 7), (0xffff, 16)]);

        let (mats, bits) = decode_audio_data_subframe_partial_at(
            &stream,
            0,
            &side,
            |_, _| 1,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            1,
            5,
            StepSizeTable::Lossy,
            false,
        )
        .unwrap();
        assert_eq!(mats[0].len(), 5);
        assert_eq!(bits, 2 * 7 + 16, "two 7-bit V3 words + DSYNC");
        let step = StepSizeTable::Lossy.step_size(1).unwrap();
        let want = [step, step, -step, -step, -step];
        for (m, w) in want.iter().enumerate() {
            assert!((mats[0][m][0] - w).abs() < 1e-9);
        }
    }

    /// Find the `(code, len)` pair a §D.5 book decodes to `level`, by
    /// scanning prefixes through the decoder itself (test-side encode
    /// for books whose encode direction is not otherwise needed).
    fn huff_codeword(book: AudioHuffCodebook, level: i16) -> (u32, u8) {
        for len in 1..=16u8 {
            for code in 0..(1u32 << len) {
                // Lay the candidate at the front of a padded buffer.
                let padded = (code << (32 - len)) | ((1 << (32 - len)) - 1) >> 1;
                let bytes = padded.to_be_bytes();
                if let Ok((got, consumed)) = decode_audio_huff_at(&bytes, 0, book) {
                    if consumed == usize::from(len) && got == level {
                        return (code, len);
                    }
                }
            }
        }
        panic!("no codeword for level {level}");
    }

    /// §D.5 Huffman partial subsubframe: the per-sample carrier
    /// extracts exactly `PSC` codewords — verified with the 3-level
    /// `ABITS = 1` book, `nSSC = 1`, `PSC = 3`, bit budget exact.
    #[test]
    fn psc_huffman_extracts_exactly_psc_codewords() {
        let book = AudioHuffCodebook::from_abits_sel(1, 0).expect("ABITS=1 SEL=0 book exists");
        let levels = [1i16, -1, 0];
        let mut fields: Vec<(u32, u8)> = Vec::new();
        let mut audio_bits = 0usize;
        for &level in &levels {
            let (code, len) = huff_codeword(book, level);
            fields.push((code, len));
            audio_bits += usize::from(len);
        }
        fields.push((0xffff, 16)); // DSYNC after the partial subsubframe
        let stream = pack_fields(&fields);

        let mut ch = ChannelSideInfo::cleared();
        ch.abits[0] = 1;
        ch.scales[0][0] = 1;
        let side = vec![ch];

        let (mats, bits) = decode_audio_data_subframe_partial_at(
            &stream,
            0,
            &side,
            |_, _| 0, // SEL = 0 -> Huffman book A3
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            1,
            3,
            StepSizeTable::Lossy,
            false,
        )
        .unwrap();
        assert_eq!(mats[0].len(), 3);
        assert_eq!(bits, audio_bits + 16, "exactly PSC codewords + DSYNC");
        let step = StepSizeTable::Lossy.step_size(1).unwrap();
        for (m, &level) in levels.iter().enumerate() {
            assert!((mats[0][m][0] - step * f64::from(level)).abs() < 1e-9);
        }
    }

    /// ASPF on a partial subframe: a DSYNC follows the full
    /// subsubframe *and* the partial one (the p.30 "A DSYNC word will
    /// always occur after a partial subsubframe" clause composes with
    /// the per-subsubframe ASPF rule).
    #[test]
    fn psc_with_aspf_places_dsync_after_both_subsubframes() {
        let mut ch = ChannelSideInfo::cleared();
        ch.abits[0] = 8;
        ch.scales[0][0] = 4;
        let side = vec![ch];

        let mut fields: Vec<(u32, u8)> = (0..8).map(|_| (0u32, 5u8)).collect();
        fields.push((0xffff, 16)); // ASPF DSYNC after subsubframe 0
        fields.extend((0..2).map(|_| (0u32, 5u8))); // partial: PSC = 2
        fields.push((0xffff, 16)); // DSYNC after the partial subsubframe
        let stream = pack_fields(&fields);

        let (mats, bits) = decode_audio_data_subframe_partial_at(
            &stream,
            0,
            &side,
            |_, _| 7,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            2,
            2,
            StepSizeTable::Lossy,
            true,
        )
        .unwrap();
        assert_eq!(mats[0].len(), 10);
        assert_eq!(bits, 10 * 5 + 2 * 16);
    }

    /// A truncated partial subsubframe (stream ends inside the PSC
    /// samples) surfaces a typed EOF, not a panic or a padded matrix.
    #[test]
    fn psc_truncated_stream_is_typed_eof() {
        let mut ch = ChannelSideInfo::cleared();
        ch.abits[0] = 8;
        ch.scales[0][0] = 4;
        let side = vec![ch];

        // 8 full samples then only 1 of the 3 partial samples.
        let fields: Vec<(u32, u8)> = (0..9).map(|_| (0u32, 5u8)).collect();
        let stream = pack_fields(&fields);

        let err = decode_audio_data_subframe_partial_at(
            &stream,
            0,
            &side,
            |_, _| 7,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            2,
            3,
            StepSizeTable::Lossy,
            false,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            AudioArrayDecodeError::Bitstream(Error::UnexpectedEof)
        ));
    }

    // -----------------------------------------------------------
    // §5.5 LFE phase walker (§2.2).
    // -----------------------------------------------------------

    /// The LFE phase consumes `2·LFF·nSSC` 8-bit samples + an 8-bit
    /// scale index, and reports exactly that many bits.
    #[test]
    fn lfe_phase_consumes_samples_plus_scale_index() {
        let lff = 1u8; // 128×
        let n_ssc = 2usize;
        let n_lfe = 2 * (lff as usize) * n_ssc; // 4 samples
                                                // 4 sample bytes (all 0) + 1 scale-index byte (10).
        let mut fields: Vec<(u32, u8)> = vec![(0, 8); n_lfe];
        fields.push((10, 8));
        let stream = pack_fields(&fields);
        let mut lfe = crate::LfeChannel::new();
        let (pcm, bits) = decode_lfe_phase_at(&stream, 0, lff, n_ssc, &mut lfe).unwrap();
        assert_eq!(bits, (n_lfe + 1) * 8);
        // Each decimated sample expands to 128 PCM samples.
        assert_eq!(pcm.len(), n_lfe * 128);
        // All-zero LFE samples decode to silence.
        assert!(pcm.iter().all(|&s| s == 0));
    }

    /// 8-bit two's-complement LFE samples are read as signed: a 0xFF byte
    /// is -1, which (with a non-zero scale) produces non-zero PCM of the
    /// correct sign at phase 0.
    #[test]
    fn lfe_phase_reads_signed_samples() {
        let lff = 2u8; // 64×
        let n_ssc = 1usize;
        let n_lfe = 2 * (lff as usize) * n_ssc; // 4 samples
        let scale_index = 60u8;
        // First sample = 0xFF (= -1), rest 0.
        let mut fields: Vec<(u32, u8)> = vec![(0xFF, 8)];
        fields.extend(vec![(0, 8); n_lfe - 1]);
        fields.push((u32::from(scale_index), 8));
        let stream = pack_fields(&fields);

        let mut lfe = crate::LfeChannel::new();
        let (pcm, _) = decode_lfe_phase_at(&stream, 0, lff, n_ssc, &mut lfe).unwrap();

        // Reference: phase-0 first output = (int)((-1)·nScale·0.035·c0).
        let n_scale = crate::side_info::RMS_7BIT[scale_index as usize] as f64;
        let r_scale = n_scale * crate::LFE_SCALE_STEP;
        let sel = crate::LfeInterpolationSelection::Decimation64;
        let c0 = sel.coefficients()[0];
        let expected0 = (-(r_scale * c0)) as i32;
        assert_eq!(pcm[0], expected0);
    }

    // -----------------------------------------------------------
    // §D.10 recovered-book walk (round 434).
    // -----------------------------------------------------------

    fn tiny_hf_book() -> crate::HfVqCodebook {
        // Vector v, element m: (v + m) / 24 — small, distinct, exact.
        let vectors: Vec<[f64; 32]> = (0i32..1024)
            .map(|v| core::array::from_fn(|m| f64::from(v + m as i32) / 24.0))
            .collect();
        crate::HfVqCodebook::from_elements(&vectors).unwrap()
    }

    fn tiny_adpcm_book() -> crate::AdpcmVqCodebook {
        // Vector i: coefficients (i mod 5 − 2) / 16 in every tap.
        let vectors: Vec<[f64; 4]> = (0i32..4096)
            .map(|i| [(f64::from(i % 5) - 2.0) / 16.0; 4])
            .collect();
        crate::AdpcmVqCodebook::from_coefficients(&vectors).unwrap()
    }

    /// A supplied HF fill whose capture shape disagrees with the
    /// per-channel `[nVQSUB, nSUBS)` bounds surfaces the typed shape
    /// error (wrong outer length and wrong per-channel count).
    #[test]
    fn hf_fill_shape_mismatch_is_typed() {
        let book = tiny_hf_book();
        let side = vec![ChannelSideInfo::cleared()];
        let stream = pack_fields(&[(0xffff, 16)]);

        // Outer capture length 2 for a 1-channel walk.
        let indices = vec![vec![0u16], vec![]];
        let err = decode_audio_data_subframe_vq_at(
            &stream,
            0,
            &side,
            |_, _| 0,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[2],
            1,
            0,
            StepSizeTable::Lossy,
            false,
            Some(HfVqFill {
                book: &book,
                indices: &indices,
            }),
            None,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            AudioArrayDecodeError::Blocked(AudioArrayError::HfVqIndexShape { ch: 1 })
        ));

        // Right outer length, wrong per-channel count (2 for 1 HF
        // subband).
        let indices = vec![vec![0u16, 1]];
        let err = decode_audio_data_subframe_vq_at(
            &stream,
            0,
            &side,
            |_, _| 0,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[2],
            1,
            0,
            StepSizeTable::Lossy,
            false,
            Some(HfVqFill {
                book: &book,
                indices: &indices,
            }),
            None,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            AudioArrayDecodeError::Blocked(AudioArrayError::HfVqIndexShape { ch: 0 })
        ));
    }

    /// A hand-built `PMODE != 0` subband with no captured PVQ index is
    /// rejected with the typed error even when the book is present.
    #[test]
    fn missing_pvq_index_is_typed() {
        let book = tiny_adpcm_book();
        let mut ch = ChannelSideInfo::cleared();
        ch.pmode[0] = 1; // pvq_index stays None — impossible via decode
        let side = vec![ch];
        let mut history = AdpcmHistory::new(1);
        let err = decode_audio_data_subframe_vq_at(
            &[0u8; 8],
            0,
            &side,
            |_, _| 0,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            1,
            0,
            StepSizeTable::Lossy,
            false,
            None,
            Some(AdpcmContext {
                book: &book,
                history: &mut history,
            }),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            AudioArrayDecodeError::Blocked(AudioArrayError::MissingPvqIndex { ch: 0, n: 0 })
        ));
    }

    /// The HF fill populates exactly the `[nVQSUB, nSUBS)` columns
    /// with `SCALES[ch][n][0] · vector[m]`, consumes no §5.5 bits,
    /// and lifts the bookless blocker.
    #[test]
    fn hf_fill_populates_hf_columns() {
        let book = tiny_hf_book();
        let mut ch = ChannelSideInfo::cleared();
        ch.scales[1][0] = 3; // HF subband n=1: SCALES[ch][1][0] = 3
        ch.scales[2][0] = 5; // HF subband n=2
        let side = vec![ch];
        let stream = pack_fields(&[(0xffff, 16)]); // just the DSYNC
        let indices = vec![vec![7u16, 100]];
        let (mats, bits) = decode_audio_data_subframe_vq_at(
            &stream,
            0,
            &side,
            |_, _| 0,
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[3],
            1,
            0,
            StepSizeTable::Lossy,
            false,
            Some(HfVqFill {
                book: &book,
                indices: &indices,
            }),
            None,
        )
        .unwrap();
        assert_eq!(bits, 16, "the fill itself reads no bits");
        for (m, row) in mats[0].iter().enumerate() {
            assert_eq!(row[0], 0.0, "coded subband (ABITS=0) stays 0");
            assert_eq!(row[1], 3.0 * (f64::from(7 + m as i32) / 24.0));
            assert_eq!(row[2], 5.0 * (f64::from(100 + m as i32) / 24.0));
        }
    }

    /// [`AdpcmHistory::absorb_matrices`] slides the last four rows in
    /// (oldest first), with the short-subframe shift semantics for
    /// fewer than four rows.
    #[test]
    fn adpcm_history_absorb_semantics() {
        let mut hist = AdpcmHistory::new(1);
        // 5 rows: subband 0 carries 1..=5.
        let mut m: SubbandSampleMatrix = vec![[0.0; NUM_SUBBAND]; 5];
        for (k, row) in m.iter_mut().enumerate() {
            row[0] = (k + 1) as f64;
        }
        hist.absorb_matrices(std::slice::from_ref(&m));
        assert_eq!(hist.subband(0, 0), &[2.0, 3.0, 4.0, 5.0]);

        // A 2-row (short) subframe shifts and appends.
        let mut m2: SubbandSampleMatrix = vec![[0.0; NUM_SUBBAND]; 2];
        m2[0][0] = 10.0;
        m2[1][0] = 11.0;
        hist.absorb_matrices(std::slice::from_ref(&m2));
        assert_eq!(hist.subband(0, 0), &[4.0, 5.0, 10.0, 11.0]);
    }

    /// The ADPCM context reconstructs a predicted subband: residuals
    /// plus the 4-tap dot product over the priming history, history
    /// advanced to the block's final rows.
    #[test]
    fn adpcm_context_predicts_and_advances_history() {
        let book = tiny_adpcm_book();
        // PVQ index 1 -> coefficients [-1/16; 4].
        let mut ch = ChannelSideInfo::cleared();
        ch.pmode[0] = 1;
        ch.pvq_index[0] = Some(1);
        ch.abits[0] = 8;
        ch.scales[0][0] = 1;
        let side = vec![ch];

        // One subsubframe of NFE residuals: 16, 0, 0, 0, 0, 0, 0, 0
        // — but NFE range for ABITS=8 is 5 bits, so use 8.
        let vals = [8i32, 0, 0, 0, 0, 0, 0, 0];
        let mut fields: Vec<(u32, u8)> = vals.iter().map(|&v| ((v as u32) & 0x1f, 5u8)).collect();
        fields.push((0xffff, 16));
        let stream = pack_fields(&fields);

        let mut history = AdpcmHistory::new(1);
        let (mats, _) = decode_audio_data_subframe_vq_at(
            &stream,
            0,
            &side,
            |_, _| 7, // terminal NFE SEL for ABITS=8
            |_, _| ScaleFactorAdjustment::Adj0,
            &[1],
            &[1],
            1,
            0,
            StepSizeTable::Lossy,
            false,
            None,
            Some(AdpcmContext {
                book: &book,
                history: &mut history,
            }),
        )
        .unwrap();

        // Analytic: r[0] = 8·step; r[m] = Σ c·r[m-1..m-4], c = -1/16.
        let step = StepSizeTable::Lossy.step_size(8).unwrap();
        let c = -1.0 / 16.0;
        let mut expect = [0.0f64; 8];
        let residual = [8.0 * step, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        for m in 0..8 {
            let mut acc = residual[m];
            for t in 0..4usize {
                if m > t {
                    acc += c * expect[m - t - 1];
                }
            }
            expect[m] = acc;
        }
        for m in 0..8 {
            assert!((mats[0][m][0] - expect[m]).abs() < 1e-12, "row {m}");
        }
        // History advanced to rows 4..8.
        let h = history.subband(0, 0);
        for k in 0..4 {
            assert!((h[k] - expect[4 + k]).abs() < 1e-12);
        }
    }

    /// A reserved §D.1.2 scale index surfaces the typed LFE-phase blocker.
    #[test]
    fn lfe_phase_rejects_reserved_scale_index() {
        let lff = 1u8;
        let n_ssc = 1usize;
        let n_lfe = 2 * (lff as usize) * n_ssc;
        let mut fields: Vec<(u32, u8)> = vec![(0, 8); n_lfe];
        fields.push((126, 8)); // reserved
        let stream = pack_fields(&fields);
        let mut lfe = crate::LfeChannel::new();
        let err = decode_lfe_phase_at(&stream, 0, lff, n_ssc, &mut lfe).unwrap_err();
        assert!(matches!(
            err,
            AudioArrayDecodeError::Blocked(AudioArrayError::LfePhase(
                crate::LfeChannelError::ReservedScaleIndex { index: 126 }
            ))
        ));
    }
}

# oxideav-dts

[![CI](https://github.com/OxideAV/oxideav-dts/actions/workflows/ci.yml/badge.svg)](https://github.com/OxideAV/oxideav-dts/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/oxideav-dts.svg)](https://crates.io/crates/oxideav-dts) [![docs.rs](https://docs.rs/oxideav-dts/badge.svg)](https://docs.rs/oxideav-dts) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A pure-Rust DTS (DTS Coherent Acoustics) decoder for the
[oxideav](https://github.com/OxideAV/oxideav) framework, built clean-room
from a locally-staged copy of ETSI TS 102 114 V1.3.1.

## Status

This crate is a **Core-profile decoder with the complete decode chain
landed** (extensions — EXSS / XCH / XXCH / X96 / XLL — are out of
scope). The frame container, structural parsing, and the full DSP
reconstruction chain are in place: the registry `Decoder` **decodes
raw 16-bit *and* 14-bit container DTS Core frames to PCM end to end**
(§5.3 → §5.4 → §5.5 → §C.2.5), emitting a planar S32 `AudioFrame` —
including, since round 439, §D.10 VQ/ADPCM frames through the built-in
Annex D code books. A
14-bit-packed frame (either container byte order) is unpacked to the
raw-16-bit-word domain in `send_packet` and decodes through the identical
chain, producing **bit-exact** PCM to the equivalent raw-16-bit frame
(asserted byte-for-byte in the registry test suite). The decoder also
**carries the §C.2.5
per-channel QMF filter tail across frames** (`CoreStreamDecoder`) so a
multi-frame elementary stream reconstructs without a per-frame
filter-warmup transient. This full-chain output is **validated against
black-box `ffmpeg` reference decodes** of three bundled
fixtures: the 5-frame stereo stream (Pearson correlation 1.0, 100 %
sign agreement on both channels), a 10-frame
**5.1 stream** (`AMODE 9` = C L R SL SR + `LFE Mode2`), where **all
five primary channels and the LFE channel** are shape-identical to
the reference (correlation 1.000000 per plane;
`tests/black_box_ffmpeg_lfe.rs`), and — new in round 429 — the
5-frame spec-built **joint-intensity stream** (`JOINX = 1`),
shape-identical on both channels
(`tests/black_box_joint_intensity.rs`), confirming the
reconstruction chain — including the §5.5 LFE phase and §C.2.6 64×
interpolation — is correct up to the implementation-defined output
`rScale` gain (the spec leaves §C.2.5 `rScale` non-normative). The
§5.4.1 Table 5-28 side-info tail is handled for **dynamic range**
(`DYNF`: the 8-bit `RANGE` code is read as signed Q2 —
`dB = (int8)code × 0.25`, `dts_dynrng_to_db` — and the linear gain
applied to the reconstructed PCM after QMF synthesis) and the **side-info CRC**
(`CPF`: the 16-bit `SICRC` is consumed for framing, not verified).
**LFE-bearing frames** (`LFF != 0`) now decode correctly: the §5.5 LFE
phase (`2·LFF·nSSC` 8-bit samples + `LFEscaleIndex`) is consumed before
the audio-data phase so the audio-data cursor stays aligned, and the LFE
samples are dequantised (§D.1.2 `RMS_7BIT` scale + `0.035` step) and
upsampled through the §C.2.6 `InterpolationFIR()` polyphase convolution
(`LfeChannel`); the registry `Decoder` emits the decoded LFE channel as
a trailing equal-length plane of the planar S32 `AudioFrame` (the
interpolation lands exactly the primary `nSSC·256` per-frame length).
**Joint-intensity frames** (`JOINX > 0`) decode and are **validated**:
the §5.4.1 Table 5-28 `JOIN_SHUFF` / `JOIN_SCALES` side-info tail is
walked (the per-channel 3-bit `QSCALES` selector then one biased
quantization index per imported sub-band, resolved through the §D.3
joint-scale table `JScaleTbl`), the §C.2.3 sub-band copy imports the
source channel's sub-band samples — scaled by the matching
`JOIN_SCALES` factor — before QMF synthesis, and (round 429) the
§C.2.5 driving call widens each jointly-coded channel's active-subband
count to the **source** channel's `nSUBS` per the spec's driving-call
note ("For joint intensity coded subbands, it must be set to that of
the source channel"), so the imported sub-bands actually reach the
output. Because no reachable black-box encoder emits `JOINX != 0`
(verified by parsing its output across its whole accepted parameter
matrix), the validation streams are **spec-built** field-by-field
(deterministic builder, `tests/common/mod.rs`): every frame is
confirmed by parsing to carry `JOINX == [0, 1]`, decode is bit-exact
against an analytic reconstruction, and the committed 5-frame joint
fixture is accepted cleanly by the black-box `ffmpeg` reference
decoder with our PCM **shape-identical** to its decode on both
channels (correlation 1.000000; `tests/black_box_joint_intensity.rs`
— the jointly-coded channel's upper sixteen sub-bands exist only
through the §C.2.3 import). A boundary battery
(`tests/joint_edge_cases.rs`) covers forward-pointing `JOINX`,
Huffman / Linear7 `JOIN_SHUFF` books, `JOINX`+`DYNF`+`CPF` tail
ordering, `JOINX`+`FRONT_SUM` over the effective range,
multi-subframe joint frames, zero-slack framing, and the three typed
error paths. **Sum/difference frames** are also handled: the §C.2.4
front L/R matrix (`L' = L+R`, `R' = L−R`) is applied on the reconstructed
sub-band samples when the `FRONT_SUM` (`SUMF`) flag is set — or
unconditionally for `AMODE == 3` — and the surround L/R matrix when
`SURROUND_SUM` (`SUMS`) is set, using the Table 5-4 channel ordering to
locate each pair (`AmodeArrangement::front_lr_channels` /
`surround_lr_channels`), between §C.2.3 and §C.2.5. The **§5.7
optional-information chunks** are decoded too: `parse_aux_data` /
`FrameView::aux_data` walk the §5.7.1 Auxiliary Data chunk (decode
time stamp + the dynamic **embedded downmix coefficients**, resolved
through the §D.11 `DmixTable` and applicable to planar PCM via
`DynamicDownmix::apply_planar`), and `parse_rev2_aux` /
`FrameView::rev2_aux` walk the §5.7.2 Rev2 chunk (embedded-ES downmix
scale, per-subsubframe broadcast DRC values, `DIALNORM_rev2aux`).
**§D.10 VQ / ADPCM frames**
(high-frequency VQ sub-bands and ADPCM prediction) **decode out of the
box** (round 439): the two Annex D code books the spec deliberately
omits ("Due to its extensive size, this table is not included here",
§D.10.1/§D.10.2) are staged as clean-room data tables under
`docs/audio/dts/tables/` and **built into the crate**
(`VqCodebooks::builtin()`, the default of every decoder). Our decode
of the §D.10-bearing fixture is shape-identical to the black-box
reference on **every frame class** — HF-VQ, ADPCM, and the combined
`HFLAG = 1` frame — at Pearson 1.000000 and 95-98 dB SNR after the
one implementation-defined output-scale constant
(`tests/black_box_d10.rs`). The last Core-profile decode blocker is
gone.

### What works today

- **Frame-header parsing** (`parse_frame_header` /
  `parse_frame_header_14bit`, typed `DtsFrameHeader`) — the §5.3 Core
  sync header for all four bitstream forms (16-bit big/little-endian and
  the two 14-bit container forms, via the `unpack14` helpers), including
  the trailing single-bit / small-field flags, the optional 16-bit
  `HEADER_CRC` field, and the post-CRC trailing window (multirate-inter,
  version, copy-history, PCMR, front/surround sum, and the §5.3.1
  Table 5-20 `DIALNORM` dialog-normalization gain).
- **Frame framing** — `iter_frames` / `iter_frames_14bit` /
  `FrameIterator` / `FrameView` plus `find_next_sync` walk and resync a
  multi-frame elementary stream (raw and 14-bit container streams are
  routed by encoding).
- **Side-information decode** — the §5.4.1 Primary Audio Coding Side
  Information walker (`decode_primary_side_info_at`) decodes the
  SSC/PSC prefix, PMODE/PVQ/ABITS/TMODE/SCALES planes, and the TMODE
  codebooks end-to-end through SCALES.
- **DSP primitives** — clean-room transcriptions of the building blocks
  the §5.5 audio-data reconstruction needs: the §C.2.1 block-code
  decoder (both the modulus and table-look-up variants), the §C.2.2
  inverse-ADPCM predictor, the §C.2.3 / §C.2.4 sum-difference and
  joint-subband steps, the §C.2.5 32-band synthesis QMF
  (`QmfSynthesis`), the §D.2 quantization step-size tables and §5.5
  inverse-quantization scale composition, the §D.8 512-tap 32-band
  interpolation FIR coefficient sets plus the two §D.8 512-tap **LFE**
  interpolation FIR sets (`RA_COEFF_LFE64` / `RA_COEFF_LFE128`) with the
  typed §C.2.6 `LfeInterpolationSelection` (`nDecimationSelect`) driver
  selector **and the §C.2.6 `InterpolationFIR()` polyphase convolution
  driver body** (`LfeInterpolator`, `src/lfe_synth.rs`: each decimated
  LFE sample expands to 64/128 interpolated PCM samples, carrying the
  `taps_per_phase − 1` inter-sub-frame history) and the **§5.5 LFE phase
  dequant** (`LfeChannel`: 8-bit `LFE[n]` → `rLFE[n] = LFE[n]·nScale·
  0.035` with the §D.1.2 `RMS_7BIT` scale, then `InterpolationFIR(LFF)`),
  the §5.5 `nQType` dispatch, the
  §D.6 block code books, the
  §D.5.1/§D.5.3/§D.5.4/§D.5.5/§D.5.7/§D.5.8/§D.5.9 audio-data
  quantization-index Huffman code books (the seven lowest `ABITS`
  families — 3/5/7/9/13/17/25-level; the 17-level group is the seven
  §D.5.8 books `A17`…`G17` and the 25-level group the seven §D.5.9
  books `A25`…`G25` whose deepest codeword reaches 14 bits — feeding
  the `nQType == 1` path, decoding to signed `AUDIO[m]` levels via
  `AudioHuffCodebook` / `decode_audio_huff_at` with a per-book
  `max_code_len` walk bound), and the §5.5 `DSYNC` subsubframe check
  word.
- **Header → §C.2.5 QMF-driver bridge** — `DtsFrameHeader` now resolves
  the two header-sourced parameters of the §C.2.5 `QMFInterpolation()`
  driver directly: `filter_bank_selection()` maps the `MULTIRATE_INTER`
  bit (the spec's `FILTS` "Multirate Interpolator Switch" of §5.3.1
  Table 5-15) to the §D.8 coefficient set (`false`/`FILTS==0` →
  non-perfect `raCoeffLossy`, `true`/`FILTS==1` → perfect
  `raCoeffLossLess`), and `output_r_scale()` derives the post-filterbank
  output gain `rScale = 2^(PCMR_bits−1)` from the §5.3.1 Table 5-17
  source-PCM resolution (`Some(32768/524288/8388608)` for 16/20/24-bit,
  `None` for the two reserved PCMR codes). A parsed header now feeds
  `QmfSynthesis::synthesize` end-to-end with no out-of-band parameters.
- **Per-frame multi-channel synthesis** — `MultiChannelQmf` owns one
  persistent `QmfSynthesis` per channel (the §C.2.5 `aPrmCh[ch]` filter
  objects) and runs the per-channel driving call
  `aPrmCh[ch].QMFInterpolation(FILTS, nSUBS[ch])` for every channel of a
  frame in one step, with the frame-wide `FILTS` and output `rScale`
  shared across channels. It reconstructs a whole frame's PCM either
  **planar** (per-channel `Vec<i32>`) or **interleaved** (sample-major),
  takes per-channel `nSUBS`, persists every channel's inter-frame filter
  tail across calls, and offers a `synthesize_planar_from_header`
  convenience that sources `FILTS`/`rScale` straight from a parsed
  `DtsFrameHeader` (returning `Ok(None)` for the reserved PCMR codes).

- **End-to-end frame decode** — `decode_core_frame(bytes, &header)`
  chains the §5.3.2 Audio Coding Header (Table 5-21), the per-subframe
  §5.4.1 side-info walk (Table 5-28) **including the `RANGE`/`SICRC`
  tail**, and the §5.5 + §C.2.5 reconstruction into one raw-bytes-to-PCM
  call. It decodes normal **and termination** frames — including
  `JOINX > 0` (joint-intensity, see below), `DYNF != 0` frames (the
  signed-Q2 dynamic-range gain is applied to each subframe's PCM after
  synthesis), `CPF == 1` frames (the `SICRC` word is consumed), and
  §5.4.1 `PSC > 0` partial subsubframes. `SubframePcmDecoder` (with
  `decode_subframe` / `decode_frame`) is the lower-level composition of
  the §5.5 `decode_audio_data_subframe_at` walk and the §C.2.5
  `MultiChannelQmf` synthesis, owning a persistent per-channel filter so
  the inter-subframe filter tail carries across subframes.
- **Streaming decode** — `CoreStreamDecoder` wraps a stream-lifetime
  `SubframePcmDecoder` so the §C.2.5 per-channel filter tail (`raX[]` /
  `raZ[]`) carries across **frame** boundaries of a contiguous
  elementary stream — the spec's QMF filter is a continuous per-channel
  object, not reset between frames. `decode_core_frame` (a fresh
  per-call decoder) keeps single-frame semantics; `CoreStreamDecoder` is
  the multi-frame path. The registry `Decoder::receive_frame` holds a
  persistent `CoreStreamDecoder` so multi-packet streams carry the
  filter tail across packets, and emits a planar S32 `AudioFrame`;
  joint-intensity frames decode (see above) and §D.10 VQ/ADPCM frames
  decode through the built-in code books (round 439; round 446 sweeps
  **every index of both books** through this path bit-exactly) — no
  Core frame class maps to `Unsupported` for missing book data. Carrying the
  inter-frame tail is what makes the decode match the `ffmpeg` reference
  (correlation 1.0 vs 0.73 with a per-frame reset — see
  `tests/black_box_ffmpeg_pcm.rs`).
- **§5.4.1 side-info tail** — `decode_primary_side_info_tail_at` /
  `SideInfoTail` walk the full Table 5-28 tail after the SCALES block:
  the per-channel `JOIN_SHUFF[ch]` (3-bit `QSCALES` selector) and the
  `JOIN_SCALES[ch][n]` loop (one biased quantization index per imported
  sub-band `n ∈ [nSUBS[ch], nSUBS[nSourceCh])`, resolved through the
  §D.3 joint-scale table), the 8-bit `RANGE` dynamic-range code
  (`DYNF`, resolved as **8-bit signed Q2** via `dts_dynrng_to_db` /
  `dts_dynrng_to_linear` — `dB = (int8)code × 0.25`, per the staged
  `docs/audio/dts/dts-drc-dynrng.md`; the §D.4 table stays available
  as reference data keyed by its offset-binary printed Index), and
  the 16-bit `SICRC` (`CPF`). The resolved `JOIN_SCALES`
  factors are carried in `SideInfoTail::join_scales`.
- **§D.3 joint-intensity scale table** — `join_scale` /
  `JOIN_SCALE_FACTOR` transcribe the §D.3 `JScaleTbl` (129 entries,
  index 64 → unity), the look-up the biased `JOIN_SCALES` index feeds.
- **§C.2.3 joint-intensity sub-band copy** — `decode_core_frame` /
  `SubframePcmDecoder::decode_subframe_with_joint` import a jointly-coded
  channel's high sub-bands from its source channel
  (`nSourceCh = JOINX[ch] − 1`), each scaled by the matching
  `JOIN_SCALES` factor, on the decoded sub-band matrices **before** QMF
  synthesis — and both the §C.2.4 sum/difference matrix and the §C.2.5
  synthesis then run over the **effective** active-subband counts
  (widened to the source channel's `nSUBS` for jointly-coded channels,
  per the §C.2.5 driving-call note). `JOINX > 0` frames decode end to
  end, bit-exact against an analytic reconstruction and shape-identical
  to a black-box reference decode of the bundled spec-built joint
  fixture (`tests/fixtures/dts_joint_5_frames.bin`, re-derived
  byte-for-byte from its deterministic builder in CI).

- **§5.3.1 termination frames (`FTYPE = 0`) + §5.4.1 partial
  subsubframe (`PSC`)** — a termination-frame subframe whose
  `SSC`/`PSC` prefix signals `PSC ∈ 1..=7` decodes its **last**
  subsubframe as partial (`PSC` subband samples per active subband
  instead of 8), yielding the valid-prefix PCM
  (`((nSSC−1)·8 + PSC) · 32` samples per channel; frame total always
  `(NBLKS+1) · 32`) with the §5.5 bit budget exact through the
  truncation — per-sample carriers extract `PSC` codewords, the
  §D.6 block-code carrier extracts `ceil(PSC/4)` four-sample words
  keeping the first `PSC`, and the DSYNC trailer follows the partial
  subsubframe. `PSC > 0` on a *normal* frame declines with the typed
  `PartialSubsubframeInNormalFrame` ("It exists only in a
  termination frame", PDF p.30). The `SHORT` deficit surfaces as
  `DtsFrameHeader::termination_pad_samples` (the `1..=31`-sample
  output pad; the decode chain returns decoded samples only). LFE
  planes are truncated to the valid prefix (the §5.5 LFE count
  `2·LFF·nSSC` has no `PSC` term). Validated by a full `SSC × PSC`
  grid, JOINX/DYNF/CPF/ASPF/LFE/multi-subframe interaction and
  corruption batteries over a spec-built termination fixture
  (`tests/fixtures/dts_term_5_frames.bin`, re-derived in CI); the
  black-box reference decoder was observed to *skip* `FTYPE = 0`
  frames at the parser level, so the reference comparison pins the
  normal-frame prefix shape-exactly and the termination tail is
  validated in-crate (`tests/black_box_termination.rs`).
- **§5.6 Unpack Optional Information (Table 5-30)** —
  `decode_optional_info_at` walks the flag-gated region after the
  last audio-data array (`TIMES` time code stamp when `TIMEF`,
  `AUXCT`/`AUXD` auxiliary bytes when `AUXF`, `OCRC` when
  `CPF && DYNF` — surfaced raw per the spec's "shall not be
  applied"), and the `*_with_info` decode entry points
  (`decode_core_frame_with_info`,
  `SubframePcmDecoder::decode_core_frame_with_info_into`,
  `CoreStreamDecoder::decode_frame_with_info`) run it from the real
  end-of-audio bit cursor so callers get PCM + optional info in one
  pass (validated bit-identical to the plain decode on the bundled
  fixture).
- **§D.11 downmix scale-factor tables** — `DMIX_TABLE` (241 × u16,
  the Q15 `DmixTable` column, `-60 dB` … unity) and `INV_DMIX_TABLE`
  (201 × u32, the Q16 `InvDmixTbl` column for `DmixTblIndex >= 40`),
  with `dmix_scale` / `inv_dmix_scale` look-ups and
  `decode_dmix_code` (the §5.7.1 Table 5-31 9-bit coefficient-code
  resolution: phase MSB, one-biased low byte, `0` → exact `0.0`).
  Every entry of both columns is unit-verified against the spec's own
  closed-form dB-ramp derivation (including the deliberate index-216
  half-power point `1/sqrt(2)`).
- **§5.7.1 Auxiliary Data chunk** — `find_aux_data` (the spec's
  suggested backward search for the DWORD-aligned `nSYNCAUX`
  `0x9A1105A0`), `parse_aux_data` / `parse_aux_data_at` /
  `FrameView::aux_data`: the 36-bit decode time stamp (nibble
  realignment + both `0b1011` marker validations) and the dynamic
  downmix coefficient table (`DownmixType`, Table 5-32;
  `DeriveNumDwnMixCodeCoeffs()` from `anNumCh[AMODE]` + LFE;
  `DynamicDownmix::coefficient_matrix` through §D.11;
  `DynamicDownmix::apply_planar` folds planar PCM through the table
  with the §C.2.5 `int()` truncation convention). The `nAUXCRC16` is
  **verified** with the Annex B CRC-16 over its documented coverage
  span (`AuxData::crc_valid`).
- **§5.7.2 Rev2 Auxiliary Data Chunk** — `find_rev2_aux` /
  `parse_rev2_aux` / `FrameView::rev2_aux`: `nRev2AUXDataByteSize`
  (validated `3..=128`), the embedded-ES downmix scale index
  (validated `40..=240`, resolved via §D.11
  `Rev2AuxChunk::es_downmix_scale`), the size-gated broadcast
  metadata — per-subsubframe 8-bit DRC values for
  `DRCversion_Rev2AUX == 1` (one per `32·(NBLKS+1)/256` subsubframe,
  Table 5-34; unsupported versions are skipped per the spec's ignore
  rule) and the 5-bit `DIALNORM_rev2aux` (`DNG = −value` dB,
  Table 5-36) — and the `nRev2AUXCRC16` read at its size-located
  offset (also skipping the reserved field of "unspecified
  duration") and **verified** with the Annex B CRC-16 over the
  `nRev2AUXDataByteSize − 2` covered bytes
  (`Rev2AuxChunk::crc_valid`). `Rev2Drc::gains_db` /
  `Rev2Drc::multipliers` resolve the DRC codes through the §5.7.2
  `dts_dynrng_to_db()` signed-Q2 function (the legacy-core
  coefficient space the spec says these values replace; the raw
  codes stay exposed). On the decode path, a CRC-verified version-1
  Rev2AUX DRC payload **overrides** the legacy `DYNF` gain per
  §5.7.2.2: `decode_core_frame` / `CoreStreamDecoder` suppress the
  per-subframe `RANGE` multiply and scale each Table 5-34 256-sample
  subsubframe window by its own Rev2 gain instead
  (`tests/rev2_drc_override.rs`).
- **§D.10 VQ decode with the built-in code books (rounds 434 + 439)**
  — the two §5.5 sub-paths that long sat behind the spec-omitted
  §D.10 code books decode by default. The books themselves are staged
  clean-room data (`docs/audio/dts/tables/dts-d10-1-adpcm-coeff-vq.csv`,
  4096 × 4 signed Q13; `dts-d10-2-hfreq-vq.csv`, 1024 × 32 int8;
  chain of custody in `docs/audio/dts/provenance/11-extractor-d10-vq.md`,
  two independent sources agreeing on every value), transcribed
  SHA-256-pinned into `d10_tables` and exposed as
  `HfVqCodebook::builtin()` / `AdpcmVqCodebook::builtin()` /
  `VqCodebooks::builtin()` — the default of every decoder;
  `VqCodebooks::none()` (via `set_vq_codebooks`) restores the typed
  bookless blocker. The staged recovery record also settled two
  §D.10.2 facts the spec left open: the element divisor is
  **`2^4 = 16`** (the printed "24" is a typo — a `2^4` with a lost
  superscript; the literal reading costs a constant 2/3 gain on every
  VQ-coded HF subband) and element `2k` is entry `k`'s **low** byte.
  On an HF-VQ frame the phase-1 10-bit `nVQIndex` region (ahead of
  the LFE phase) is walked and each HF subband's rows are
  `SCALES[ch][n][0] · HFREQ[m]` (the Table 5-29 `Scale`/`rScale`
  naming conflation is spec-verbatim; the p.33 HFREQ prose resolves
  it and gives the termination-frame valid-prefix pick rule); on a
  `PMODE != 0` frame the §C.2.2 inverse-ADPCM predictor runs from the
  captured 12-bit `PVQ` index, the per-subband reconstruction history
  (`AdpcmHistory`) carried across subsubframes/subframes and gated at
  frame boundaries by the §5.3.1 `HFLAG` Predictor History Flag
  Switch. Validated bit-exactly against analytic reconstructions
  (`tests/d10_vq_decode.rs`: HFLAG carry/reset grid, PSC × HF-VQ,
  PSC × ADPCM, an HF+ADPCM+LFE+JOINX+DYNF kitchen-sink frame) and
  black-box (`tests/black_box_d10.rs` +
  `tests/fixtures/dts_d10_5_frames.bin`): with the built-in books our
  decode is **shape-identical to the reference on all five frames**
  (Pearson 1.000000 per frame per channel; 95-98 dB SNR after the √2
  output-scale constant), and the registry surface decodes the same
  stream by default, bit-identical to the direct path. Round 446
  closes the index space: the **full-book sweeps** drive all 1024
  §D.10.2 and all 4096 §D.10.1 vectors through the real bitstream
  decode path, each frame bit-exact against an analytic
  reconstruction recomputed from the built-in books
  (`tests/d10_vq_decode.rs`), and the committed 12-frame
  **book-coverage stream** (`tests/black_box_d10_coverage.rs`)
  black-box-confirms 480 swept vectors — the §D.10.2
  duplicate-codeword cluster, both book heads and tails, four frames
  predicting all 32 subbands of both channels, and two `HFLAG = 1`
  history-chained frames — shape-identical to the reference decode
  (Pearson 1.000000; 90.5-95.9 dB SNR after the same √2 constant).
- **Annex B CRC-16** — `dts_crc16` / `dts_crc16_update` /
  `DTS_CRC16_TABLE`: the single normative DTS CRC (CRC-CCITT,
  polynomial `0x1021`, init `0xFFFF`, MSB-first, no reflection, no
  final XOR — the CRC-16/CCITT-FALSE parameter set), per the staged
  `docs/audio/dts/dts-crc16.md`. Drives the aux / Rev2-aux
  verification above; the core `HCRC` / `AHCRC` / `SICRC` / `OCRC`
  stay unverified **by spec mandate** ("The CRC value test shall not
  be applied" — they are informational placeholders).

### Not yet implemented

- Extensions (EXSS / XCH / XXCH / X96 / XLL) are out of scope for the
  current Core-profile effort.
- `DtsFrameHeader::verify_header_crc` returns `None` **by design**,
  not because of a docs gap: the Annex B CRC algorithm is documented
  and implemented (`dts_crc16`), but §5.3.1 states "The CRC value
  test shall not be applied" for the core `HCRC` (likewise `AHCRC` /
  `SICRC` / `OCRC`), and the spec does not normatively pin the
  `HCRC` coverage span. The raw 16-bit field stays surfaced for
  pass-through callers; the genuinely testable check words
  (`nAUXCRC16`, `nRev2AUXCRC16`) *are* verified.

## Usage

```rust
use oxideav_dts::{parse_frame_header, iter_frames};

let bytes: &[u8] = b""; // a DTS Core (raw 16-bit) elementary stream

// Parse a single Core frame header.
if let Ok(_hdr) = parse_frame_header(bytes) {
    // inspect channel layout, sample-rate code, frame size, ...
}

// Walk a multi-frame stream.
for frame in iter_frames(bytes) {
    let _payload = frame.payload();
}

// Decode one whole Core frame to planar PCM (common Core case).
use oxideav_dts::decode_core_frame;
if let Ok(hdr) = parse_frame_header(bytes) {
    match decode_core_frame(bytes, &hdr) {
        Ok(pcm) => { /* pcm[ch] is a Vec<i32> of reconstructed samples */ }
        Err(_unsupported_tail_or_vq) => { /* not the common Core case */ }
    }
}
```

The DSP primitives are public crate-root re-exports
(`decode_block_code`, `QmfSynthesis`, `fir_step`, `dequant_subsubframe`,
…) for callers experimenting with the reconstruction chain directly.

## Cargo features

| Feature    | Default | Effect |
|------------|---------|--------|
| `registry` | yes     | Pulls in `oxideav-core` and registers the codec via `register`, exposing the `Decoder` trait surface and `probe_dts`. Disable (`default-features = false`, build `--no-default-features --lib`) for a standalone build that exposes only the header parser, framing, and DSP primitives without the framework dependency. |

## Clean-room provenance

Implemented entirely from a locally-staged copy of ETSI TS 102 114
V1.3.1 under `docs/audio/dts/`. No external decoder or library source
was consulted; binaries are used only as black-box fixture generators
and validators, never as a source of constants or layout.

## License

MIT — see [LICENSE](LICENSE).

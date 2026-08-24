# oxideav-aac

[![CI](https://github.com/OxideAV/oxideav-aac/actions/workflows/ci.yml/badge.svg)](https://github.com/OxideAV/oxideav-aac/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/oxideav-aac.svg)](https://crates.io/crates/oxideav-aac) [![docs.rs](https://docs.rs/oxideav-aac/badge.svg)](https://docs.rs/oxideav-aac) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A pure-Rust **AAC** (Advanced Audio Coding) codec for the
[oxideav](https://github.com/OxideAV/oxideav-workspace) framework.

Every numeric constant, bit layout, and clause reference is sourced from
the staged ISO/IEC 13818-7 and ISO/IEC 14496-3 specifications under
`docs/audio/aac/`.

## Status

The crate implements the full AAC-LC decode chain end to end — from
ADTS bitstream parse through the per-tool reconstruction to interleaved
16-bit PCM — **plus the complete §4.6.18 SBR back-end (HE-AAC v1) and
the subpart-8 Parametric Stereo tool (HE-AAC v2)**, and **wires them
into the framework's runtime `Decoder` trait** (`register()` installs
an AAC decoder under id `"aac"`; see `codec_decoder` below). The PCM
is validated byte-exactly (within the 1-LSB IMDCT-rounding bound)
against the staged `expected.wav` corpus — **including the HE-AAC v1
SBR fixture, which decodes 99.98% sample-exact with a max error of
1 LSB** at the doubled output rate, and the **HE-AAC v2 fixture, whose
PS stereo reconstruction lands at a 5e-5 per-channel error-to-signal
RMS** against the reference decode. The §4.6.18.4.3 **downsampled**
output mode (core-rate SBR, auto-selected from a core-rate
`extensionSamplingFrequency` ASC) and the §4.6.18.8 **low power** SBR
tool (real-valued filterbanks + aliasing detection/reduction) are
selectable on every decode entry point.
The crate also ships an **end-to-end AAC-LC encoder** (`encoder` /
`codec_encoder`): PCM → §4.6.11.3.1 forward-MDCT analysis with
§4.6.11.3.2 block switching (transient-driven
`ONLY_LONG → LONG_START → EIGHT_SHORT → LONG_STOP`, short frames
grouped per §4.5.2.3.4 on the band-envelope similarity of adjacent
windows), the exact §4.6.2 inverse quantizer under a masking-spread
psychoacoustics-lite model with a bidirectional rate loop,
measured-bit-cost codebook/section choice (a DP over section
boundaries priced with the real tuple writer), per-band §4.6.8.1
M/S joint stereo (long frames per sfb, short frames per
`(window group, sfb)` under the pair's joint grouping), and
**every Table 1.19 default channel layout** — 1–6 and 8 (7.1)
channels as SCE / `common_window`-CPE / §4.5.2.1.3-conforming LFE
element plans — assembled into ADTS through the Phase-2 bit-exact
wire writers. Every stream is round-tripped through the crate's own
decoder (multitone 128 kbps at 0.016 err/sig RMS; staged-fixture
transcodes at 0.0008–0.003; multichannel layouts pinned with one
distinct tone per speaker); `register()` installs the encoder
alongside the decoder under id `"aac"`.

### ISO/IEC 14496-26 conformance (normative corpus)

`tests/iso_14496_26_conformance.rs` decodes members of the normative
MPEG-4 Audio conformance corpus end to end against their reference
waveforms (corpus located via `OXIDEAV_ISO_14496_26_DIR`,
skip-if-absent; sourcing, per-member checksums and the member-level
fetch recipe are in `docs/audio/aac/iso-14496-26-conformance.md` — the
ISO-copyright bitstreams are never committed). Measured state:

* **ER AAC LD** — 15 vectors across 22.05/24/32/44.1/48 kHz at both
  frame lengths: **47 003 / 47 004 access units decode** (the single
  residual is `er_ad1103_22_ep0` AU 367, which the staged corpus
  screen records as failing under every width hypothesis). PCM: the
  LD-512 `er_ad1000*` family is reference-exact at err/sig ≈ 4.4e-5;
  the LD-480 `er_ad1103np*` family lands at ≈ 1.3e-4 outside its
  TNS/PNS access units (PNS noise phase is generator-defined; the
  deployed LD TNS record is a still-untraced extra-spec wire — see
  "Not yet supported"). The 32 kHz members pin the §4.5.4
  Tables 4.144/4.145 band tables end to end.
* **CCE** — the twelve `am05_*` vectors (AAC Main + one
  `coupling_channel_element()` in every access unit): **1 370 / 1 370
  AUs decode**, and all six `am05_48` output channels match their
  per-speaker references at ≈ 1e-4 err/sig — pinning the CCE gain
  path (conformance-settled `cc_scale^(−ge)` exponent), the §4.6.6
  Main-profile predictor at its normative fixed-precision arithmetic,
  M/S + intensity + TNS interplay, and the §8.5.2.2 PCE reorder.
* **SBR-CRC** — the four `al_sbr_{e,i}_32_*` vectors (the corpus's
  only `EXT_SBR_DATA_CRC` carriers): **1 600 / 1 600 payload CRCs
  verify** during a full decode, including the §4.5.2.8.1 pre-header
  prefix (upsampling-only state) and the whole-payload coverage
  region (`bs_fill_bits` included).

### Bitstream parsing

- **ADTS fixed header** (`adts`) — ISO/IEC 13818-7 §1.A.2: sync,
  profile, sampling-frequency index, channel configuration, frame
  length, raw-data-block count, CRC presence flag.
- **ADTS `error_check()` + SBR CRC verification** (`adts_crc`) — the
  ISO/IEC 13818-7:2004 §8.1.1.1 protected-bit region walk (all 56
  header bits; the first 192 bits of every SCE / CPE / CCE / LFE with
  the 3-bit `id_syn_ele` excluded and zero-padding of short elements;
  the additional first-128-bits of every CPE's *second*
  `individual_channel_stream`; all PCE / DSE bits) fed into the
  ISO/IEC 11172-3 §2.4.3.1 CRC-16 (`0x8005`, all-ones init) that
  13818-7 §8.1.1.2 cites. Both frame forms verify: the Table 1.A.8
  single-raw-data-block `crc_check` and the Table 1.A.9 / 1.A.10
  multi-RDB split (headers + `raw_data_block_position` table under
  one CRC, one CRC per block). Wired into
  `StreamDecoder::decode_adts_frame` / `decode_all` and the runtime
  `Decoder`; `protect_adts_frame` / `protect_adts_stream` produce the
  protected form (a protected rewrite of a staged fixture decodes
  byte-identically through a black-box validator binary — which,
  notably, does not verify the CRC *value*, so the code convention is
  pinned to the documented §2.4.3.1 parameters). The same module
  hosts the SBR `bs_sbr_crc_bits` CRC-10 (`G10`, zero init) computed
  over the Table 4.62 coverage region; the FIL walk verifies every
  `EXT_SBR_DATA_CRC` payload. Corruption of any covered bit surfaces
  `Error::AdtsCrcMismatch` / `Error::SbrCrcMismatch`; fill bits and
  the beyond-window element bits are provably uncovered.
- **AudioSpecificConfig** (`asc`) — ISO/IEC 14496-3 §1.6.2.1 including
  the §4.4.1 GASpecificConfig body for all General Audio object types,
  the hierarchical SBR (AOT 5) / PS (AOT 29) wrappers, the
  `extensionFlag` subtree, the `epConfig` field, and the Table 1.15
  trailing `syncExtensionType == 0x2b7` implicit-SBR probe. A
  carrier-bounded `parse_bits_bounded` entry point is exposed for LATM
  `StreamMuxConfig` callers.
- **program_config_element** (`pce`) — §4.4.1.1, used standalone and
  inline inside `asc`.
- **raw_data_block()** walker (`raw_data_block`) — §4.4.2.1: visits each
  `id_syn_ele` and stops at `END`. FIL / DSE / PCE bodies are fully
  consumed; the channel-element body is composed by the modules below.
- **Channel-element body** (`ics_body`) — Table 4.50: `global_gain` →
  `ics_info` → `section_data` → `scale_factor_data` → optional
  `pulse_data` / `tns_data` / `gain_control_data`, surfacing the start
  bit-offset for the spectral data.
- **spectral_data()** (`spectral_data`) — Table 4.56 wire walker and
  bit-exact writer, dispatching onto the Huffman codebooks.
- **extension_payload()** (`extension_payload`) — §4.4.2.7 / Table 4.51
  parser + encoder for the `EXT_FILL`, `EXT_FILL_DATA`, and
  `EXT_DYNAMIC_RANGE` branches. The two SBR-data extension types
  decode through the `parse_with_sbr` entry (feeding the §4.6.18
  back-end); the plain `parse` entry without an SBR context rejects
  them (`Error::UnsupportedExtensionSbr`).
- **Error-protection CRC generator** (`crc`) — §1.8.4.5: the full
  family of MPEG-4 Audio CRC generation polynomials (`CRC4`..`CRC32`,
  including the `CRC8` LATM `StreamMuxConfig()` `crcCheckSum` and the
  16-bit `x¹⁶+x¹⁵+x²+1`), a zero-init MSB-first shift-register
  (`crc_bits` / `crc_bytes`) implementing the §1.8.4.5
  `M(x)·xᵏ = Q(x)·G(x) + R(x)` remainder with the normative
  output-bit inversion ("written in a reversed manner, i.e. each bit
  is inverted"), and the [`crc::stream_mux_config_crc`] LATM helper.
  Cross-checked against an independent GF(2) long-division reference
  and the codeword-divisibility property. The ADTS
  `adts_error_check()` region-selection CRC uses a different code
  convention (ISO/IEC 11172-3 §2.4.3.1, all-ones init, no output
  inversion) and lives in the dedicated `adts_crc` module above.
- **RVLC error-resilient scalefactor coding** (`rvlc`,
  `scale_factor_data::ErScaleFactorData`) — §4.6.16.2 the
  reversible-variable-length-coding replacement for the §4.6.3
  noiseless coding of scalefactors, used when
  `aacScalefactorDataResilienceFlag == 1`. The `rvlc` module
  transcribes the symmetric (bit-palindrome) RVLC codebook
  (Table 4.166, deltas `-7..=+7` with `±7` the `ESC_FLAG`), the eight
  asymmetric *forbidden* codewords (Table 4.167) whose appearance is
  surfaced as the §4.6.16.2.1 in-band error-detection event, and the
  54-entry RVLC-ESC Huffman codebook (Table 4.168) — every codebook
  proven prefix-free and round-tripping, and independently
  cross-validated against the staged packed binary-tree node tables.
  `ErScaleFactorData::parse` / `::write` decode and re-encode the whole
  Table 4.53 RVLC branch: the `sf_concealment` / `rev_global_gain` /
  `length_of_rvlc_sf` (11 bits for `EIGHT_SHORT_SEQUENCE`, else 9)
  header, the RVLC base-delta band loop (first PNS band keeping the
  9-bit PCM seed), the optional `sf_escapes_present` /
  `length_of_rvlc_escapes` second pass folding each escape into its
  `±ESC_FLAG` base (`+7 + esc` / `-7 - esc` per §4.6.16.2.1), and the
  `dpcm_is_last_position` / `dpcm_noise_last_position` backward seeds.
  Both `length_of_*` fields are validated against the bits actually
  consumed. The reconstructed records share the non-resilient
  `ScaleFactorData` shape, so the §4.6.2.3.2 forward DPCM
  `accumulate()` pass consumes them unchanged — pinned by a test that
  an RVLC stream and the Huffman stream carrying the same deltas
  accumulate to identical absolute scalefactors. The
  resilience-flag dispatch from `ics_body` is now wired (see the
  error-resilient ICS body below); the RVLC bitstream path itself is
  decoded end to end.
- **Error-resilient channel-element body**
  (`ics_body::IcsBody::parse_er` / `::parse_with_ics_info_er`,
  `section_data::SectionData::parse_er` / `::write_er`) — ISO/IEC
  14496-3 §4.4.6 Tables 4.50 / 4.52, the ER General-Audio object types
  (AOTs 17 / 19 / 20 / 23). Drives all three resilience branches off
  the `AacResilienceFlags` triplet: `section_data()` through the 5-bit
  `sect_cb` branch (carrying the §4.6.16.4 virtual codebooks 16..=31,
  whose `ESC_HCB` / `>= 16` runs take the fixed `sect_len_incr = 1`
  single-band coding) when `aacSectionDataResilienceFlag` is set;
  `scale_factor_data()` through the RVLC `ErScaleFactorData` branch
  (its reconstruction mirrored into the shared `scale_factor_data`
  field so the §4.6.2.3.2 accumulate pass is branch-agnostic, with the
  RVLC seeds retained in `er_scale_factor_data`) when
  `aacScalefactorDataResilienceFlag` is set; and the
  `length_of_reordered_spectral_data` (14-bit) +
  `length_of_longest_codeword` (6-bit) HCR length fields in
  `reordered_spectral_lengths` when `aacSpectralDataResilienceFlag` is
  set. The trailing `reordered_spectral_data()` (HCR) payload is the
  caller's responsibility, exactly as `spectral_data()` is on the
  non-resilient path.
- **HCR segmentation / pre-sorting scaffold** (`hcr`) — ISO/IEC
  14496-3 §4.6.16.3.3 / §4.6.16.3.5. The deterministic, header-only
  half of Huffman codeword reordering: the Table 4.170 `maxCwLen`
  table, the §4.6.16.3.3.1 `codebookPriority[32]` table + the
  `assignedUnitNr` pre-sorting metric, the
  `segmentWidth = min(maxCwLen, length_of_longest_codeword)`
  derivation, the §4.6.16.3.2 length-field clamps, and the
  `Segmentation` layout that instantiates PCW segments until the
  `length_of_reordered_spectral_data` buffer is exhausted (folding the
  trailing bits into the last segment). `ReorderPlan::build` then runs
  the §4.6.16.3.3.4 `ReorderSpectralData()` writing scheme (PCWs
  forward from each segment start, then the non-PCW set / trial loop
  with the per-set `ToggleWriteDirection()` and the modulo-shift
  `segment = (trial + codewordBase) % numberOfSegments`) to resolve,
  for each codeword, the ordered global buffer bit positions
  (MSB-first) that carry its bits — pinned by a bijection invariant
  (every buffer bit covered exactly once).
- **HCR payload codec** (`hcr_decode`) — §4.6.16.3.3.4 / §4.6.16.3.4,
  both directions of the `reordered_spectral_data()` payload itself.
  `encode_reordered_spectral_data` enumerates the frame's codeword
  units (the §4.5.2.3.2 unit: Huffman codeword + sign bits + escape
  sequences, two or four lines) in the §4.6.16.3.3.1 pre-sorted order
  — the unit-based window interleave (Table 4.169; the §4.5.2.3.5
  grouping interleave does not apply under HCR) stably ordered by
  `assignedUnitNr` — encodes each unit and scatters the bits over the
  segment grid via `ReorderPlan`. `decode_reordered_spectral_data`
  inverts the walk without transmitted lengths: PCWs decode forward
  from their own segment starts, then the non-PCW sets run the same
  direction-toggling modulo-shift trial loop, each codeword consuming
  segment free-region bits until its Huffman unit completes (prefix
  codes make "incomplete" exactly detectable, so lengths are
  discovered where the writer defined them). The §4.6.16.4 virtual
  codebooks (16..=31) decode as book 11 with their own `maxCwLen`
  segment widths. Round-tripped bit-exactly over long / eight-short
  (two window groups), spectrum-less-band mixes, escape-bearing book
  11, virtual codebooks, and slack-padded buffers; corrupt payloads
  surface errors, never panics. Threading the ER triplet from
  `GASpecificConfig` through the stream drivers (the ER top-level
  payloads) remains open, and an HCR-bearing conformance stream is
  still wanted as an external cross-check.

### Numeric reconstruction (AAC-LC tool chain)

- **Spectrum Huffman codebooks 1..=11** (`spectrum_huffman`,
  `spectral_codebook`) — the complete Annex 4.A spectrum book set,
  including the ESC book 11, with §4.6.3.3 index↔tuple translation and
  sign-bit / escape-sequence handling.
- **Inverse quantization + scalefactors** (`dequant`) — §4.6.1.3
  non-uniform inverse quantizer and §4.6.2.3.3 scalefactor gain.
- **Decoded spectrum** (`decoded_spectrum`) — §4.6.3.3 `quant_to_spec()`
  de-interleaver plus the per-channel pipeline composing pulse fix-up →
  scalefactor accumulation → inverse quantization + rescale →
  de-interleave → TNS.
- **TNS** (`tns_data`, `tns_coef`, `tns_frame`, `tns_max`,
  `swb_offset`) — §4.6.9 Temporal Noise Shaping: wire parse,
  coefficient inverse-quantisation + conversion to LPC, the all-pole IIR
  pass, and the per-frame region-slicing orchestration.
- **Filterbank** (`filterbank`) — §4.6.11 stateful per-channel IMDCT
  with sine / KBD windows, all four `window_sequence` shapes, eight-short
  internal overlap-add, and inter-frame overlap-add. Pinned by streaming
  TDAC perfect-reconstruction tests. Covers **all four §4.5.1.1
  frame-length families**: the 1024/128- and 960/120-line
  block-switching families (`N = 2048/256` and `1920/240`) and the
  long-only ER AAC LD 512/480-line families (`N = 1024/960`), where
  the `window_shape == 1` bit selects the §4.6.17.2.3 Table 4.171
  **low-overlap window** in place of KBD (power-complementarity and
  streaming TDAC pinned per family).
- **Channel-pair / noise tools** — M/S stereo de-matrix (`ms_stereo`,
  §4.6.8.1), intensity stereo (`intensity_stereo`, §4.6.8.2), and
  Perceptual Noise Substitution (`pns`, §4.6.13). PNS produces
  energy-exact bands; only the per-coefficient phase is RNG-defined per
  §4.6.13.3, so its output is not byte-exact against any one decoder —
  the staged `docs/audio/aac/pns-gen-rand-vector.md` analysis pins the
  normative half (band selection, energy DPCM, measured-energy
  normalisation, correlated-CPE same-vector rule — all implemented)
  and shows the generator recurrence/seed/threading to be deliberately
  unspecified, so cross-decoder PNS checks are energy-domain by
  design.
- **Coupling channel element** (`cce`) — §4.6.8.3 / Table 4.8. The CCE
  coupling header (`CouplingHeader`: `ind_sw_cce_flag`,
  `num_coupled_elements`, the per-target `cc_target_is_cpe` /
  `cc_target_tag_select` / `cc_l` / `cc_r` list with the Table 4.153
  shared-vs-split `num_gain_element_lists` derivation, `cc_domain` /
  `gain_element_sign` / `gain_element_scale`) and the trailing gain-list
  block (`CouplingGains`: per-target `common_gain_element` or per-`(g,
  sfb)` `dpcm_gain_element` running-sum lists — the §4.6.8.3.3
  `ind_sw_cce_flag ⇒ common-gain-only` constraint and the embedded-SCE
  `sfb_cb` `ZERO_HCB` skip — reusing the §4.A.1 scalefactor Huffman
  codebook `hcod_sf`). `CouplingChannelElement` ties the whole Table 4.8
  element (header → embedded `individual_channel_stream(0,0)` body +
  spectrum → gain lists) together, and `CouplingGains::cc_gain` computes
  the §4.6.8.3.3 `couple_channel()` factor `cc_gain = cc_sign ·
  cc_scale^gain` (Table 4.154 `cc_scale_table`, implicit list-0 natural
  scaling). `CouplingGains::couple_channel` applies the §4.6.8.3.3
  per-band scale-and-add — the spec's group / window-group / sfb /
  coefficient loop multiplying the embedded-SCE spectrum by the
  per-`(g, sfb)` `cc_gain` and adding it onto a target channel's
  window-major spectrum (implicit list 0 in natural scaling, `ZERO_HCB`
  bands skipped). **The cross-element application is wired into the
  stream decoder**: the raw-data-block walk is two-pass (parse every
  channel element, then decode), each CCE's embedded SCE is decoded
  through a per-instance-tag `CceDecoder` slot (its own pulse / dequant
  / PNS / TNS, plus its own persistent §4.6.11 filterbank for the
  independently-switched case), and the `decode_coupling_channel()`
  target walk matches `cc_target_is_cpe` / `cc_target_tag_select`,
  assigns the Table 4.153 gain lists (shared / left / right / both),
  and injects the scaled spectrum at the signalled `cc_domain` stage
  (before / after the target's TNS; window-state match enforced) or —
  for an independently switched CCE — the scaled time signal after the
  target's filterbank. Validated end to end with writer-assembled CCE
  streams against the filterbank-linearity identity
  `decode([target, CCE]) = decode([target]) + cc_gain·decode([embedded])`
  (natural scaling, gain-list ×2, independently-switched, and
  CPE-left-only layouts, ≤ 2 LSB stacked-rounding deviation). A
  writer-assembled CCE fixture cycling all three coupling shapes
  (dpcm + sign split / ind-switched / shared natural, both domains,
  PCE-declared `valid_cc_elements`) is staged in the docs corpus
  (`aac-cce-writer-assembled`). The two long-standing §4.6.8.3.3
  wire questions are both settled: the **exponent** is negated —
  `cc_gain = cc_sign · cc_scale^(−gain_element)` — confirmed by the
  ISO/IEC 14496-26 `am05_*` conformance vectors (all three editions
  print the positive exponent, which misses every coupled target by
  ~1e-1 err/sig), and the **`gain_element_sign` split** follows the
  2001 / 13818-7:2004 `couple_channel()` text as ruled in
  `docs/audio/aac/cce-gain-sign-split.md` §3 — `cc_sign` off **each
  transmitted dpcm delta** (`1 − 2·(dpcm & 1)`), accumulator fed with
  `dpcm >> 1`, and a `common_gain_element` **never** sign-split (the
  14496-3:2009 page prints two conflicting fragments; its
  accumulated-value variant is an editorial defect of that edition).
- **Frequency-domain prediction** (`predictor`) — §4.6.6 MPEG-2
  backward-adaptive intra-channel predictor for the AAC **Main** object
  type (AOT 1). A bank of second-order lattice predictors (one per MDCT
  line up to the §4.6.6.2 `PRED_SFB_MAX` limit) reconstructs
  `x_rec = x_est + y_rec` on the signalled bands. Implements the
  §4.6.6.3.2.1 lattice `predict()` + LMS adaptation
  (`α = 0.90625`, `a = b = 0.953125`), the §4.6.6.3.2.3
  `flt_round_inf()` 16-bit-float rounding applied to every stored state
  variable and the predicted value, and the §4.6.6.3.3 reset (the 30
  Table 4.97 cyclic groups + the short-block reset-all). Wired into
  `element_decode`: the bank runs every long frame *before* TNS (and is
  mutually exclusive with LTP by object type), persisting the
  backward-adaptive state across frames.
- **Long-Term Prediction** (`ltp`) — §4.6.7 long-window LTP: the
  Table 4.98 coefficient codebook, the §4.6.7.3 `predict()` single-tap
  time-domain predictor (`x_est(i) = ltp_coef·x_rec(i − ltp_lag)`) over
  a per-channel `x_rec` reconstruction history, the windowed analysis
  `MDCT(x_est)` (the §4.6.15.3.3 / §4.6.11.3.1 forward transform, now a
  reusable `filterbank` primitive), and the per-sfb
  `X_rec = X_est + Y_rec` combination on the bands flagged by
  `ltp_long_used`. LTP is restricted to long windows for the AAC LTP
  object type (§4.6.7.1, 2009 edition). The ISO/IEC 14496-3:**2001**
  short-window synthesis (`LtpState::apply_short_2001`) is also
  implemented per the 2001 §4.6.7.3 pseudo-code — per flagged
  subwindow, `lag_w = ltp_lag + ltp_short_lag[w]`, the 256-point
  windowed `MDCT(x_est)`, and the `X_rec = X_est + Y_rec` add on the
  first 8 SFBs — with the one quantity the 2001 text never fixes
  (the per-subwindow `x_rec` index origin; see the staged
  `docs/audio/aac/short-window-ltp-blocked.md` §5) taken as an
  explicit caller parameter rather than an invented convention. The
  **ER AAC LD branch is implemented**: the 10-bit lag with the
  `ltp_lag_update` repeat state (`ltp_prev_lag`) and the §4.6.7.3
  `M = N/2` lag offset, applied at the LD transform lengths.
- **TNS analysis filter** (`tns_coef::tns_ma_filter`,
  `tns_frame::tns_analysis_frame`) — §4.6.7.4.1 / Figure 4.30: the
  all-zero (moving-average, FIR) inverse of the §4.6.9.3 all-pole
  synthesis filter, `y(n) = x(n) + Σ lpc[k]·x(n−k)`. Run over the same
  per-window region walk as `tns_decode_frame`; analysis ∘ synthesis is
  the identity over a shared region, which is the §4.6.7.4.1
  noise-shaping invariant.
- **Element decode driver** (`element_decode`) — `ElementDecoder` chains
  the whole stack per element: `decode_sce` for SCE / LFE and
  `decode_cpe` for a CPE (pulse → dequant → `quant_to_spec()` → M/S →
  intensity → PNS → **LTP → TNS** → filterbank), carrying the
  per-channel overlap-add tail **and the §4.6.7.3 LTP reconstruction
  history** across frames. LTP runs in the §4.6.7.4.1 / Figure 4.30
  block order — long-term synthesis (with the all-zero TNS analysis
  filter applied to `X_est`) *before* the §4.6.9 TNS synthesis filter,
  so the single synthesis pass shapes the residual while undoing the
  analysis on the LTP contribution.

### Frame-length families — §4.5.1.1 / §4.6.17 (960, LD 512/480)

All four `frameLengthFlag` frame geometries decode end to end, keyed
by `swb_offset::FrameFamily` (resolved from the AOT + flag; the
LATM/LOAS driver installs it per layer from the ASC,
`StreamDecoder::set_frame_family` serves raw callers — ADTS can only
carry the default 1024-line family):

- **AAC-LC at 960/120 lines** (`frameLengthFlag == 1`) — the
  bracketed "values for 1920/240" columns of Tables 4.129–4.141, the
  `N = 1920/240` transform pair with all four window sequences, both
  window shapes, grouping, TNS and the full joint-stereo/noise tool
  chain; 960 PCM samples per channel per frame. Verified **bit-exact
  against a black-box decoder binary** on writer-assembled streams,
  and staged with mutation coverage as `aac-lc-960-writer-loas`.
- **ER AAC LD at 512/480 lines** (AOT 23, §4.6.17) — Tables
  4.142–4.147 (with the §4.5.1.1 nearest-defined-table rule for the
  rates those tables omit), long-only frames (a non-`ONLY_LONG`
  `window_sequence` is rejected, §4.6.17.2.2), the §4.6.17.2.3
  Table 4.171 **low-overlap window** on the `window_shape == 1` bit,
  the §4.6.17.2.5 LD `TNS_MAX_BANDS` tables, the §4.6.7 **LD LTP**
  branch (10-bit lag, `ltp_lag_update` repeat via a per-channel
  `ltp_prev_lag`, `M = N/2` lag offset at the LD transform lengths),
  and the Table 4.19 `er_raw_data_block()` element walk shared with
  ER AAC LC; 512/480 PCM samples per channel per frame. The LD-512
  geometry (every swb band boundary, both window shapes, the
  transform/overlap-add) is verified **bit-exact against two
  independent black-box decoder binaries**; LD-480 against one (the
  other binary decodes 480-line streams on the wrong 512-line
  frequency grid — probed and documented in the fixture notes).
  Staged fixtures: `aac-ld-512-writer-loas`, `aac-ld-480-writer-loas`.
- **LD TNS wire — RESOLVED against the conformance corpus**: the
  divergence between the literal Table 4.54 / Table 4.155 field
  widths and the deployed LD TNS wire was settled by the ISO/IEC
  14496-26 screen recorded in `docs/audio/aac/er-ld-tns-divergence.md`
  §0 — the normative LD wire transmits `n_filt` in **1 bit** (the
  reduced Table 4.155 column; the literal 2-bit keying hard-fails 792
  of the corpus's 2 017 TNS-bearing AUs). The LD families read the
  1 / 4 / 3 column via `TnsData::parse_family` / `write_family`;
  because the corpus never transmits a `length` / `order` field
  (`n_filt == 0` throughout, so 4 / 3 vs 6 / 5 is undetermined), the
  §0.6 configurability recommendation is kept via the explicit-width
  `TnsData::parse_widths` / `write_widths` entry points.
- An SBR payload on a 960-line or LD stream is rejected before its
  body is parsed (`Error::SbrUnsupportedFrameFamily`) — the §4.6.18
  tool here is defined over the 1024-line core, and the §4.6.19 LD
  SBR tool belongs to ELD (out of scope).

### Scalable AAC (AOT 6) / ER AAC scalable (AOT 20) — §4.4.2.2 / §4.5.2.2

The AAC-only scalable combinations decode end to end (`scalable`):
one `aac_scalable_main_element()` plus up to seven extension
elements, each on its own elementary stream / LATM layer
(mono-only, stereo-only and mixed mono→stereo stacks, Table 4.87).

- **Syntax** — Tables 4.13–4.18: the main/extension headers
  (window geometry hoisted out of `ics_info()`, per-channel TNS on
  the first mono and first stereo layer, per-channel LTP on the main
  layer, the §4.6.8.1.4 *incremental* `ms_data()` over
  `last_max_sfb_ms..max_sfb`, per-channel `diff_control_data_lr()`
  with the Table 4.18 `ms_used` gating), and the Table 4.50
  `scale_flag == 1` ICS form (`IcsBody::parse_scale` — no inline
  `ics_info()`, no tool dispatch trio). For AOT 20 the §4.4.6
  resilience triplet selects the ER wire branches per channel
  (5-bit-`sect_cb` sections, RVLC scalefactors, inline HCR
  `reordered_spectral_data()`); the element syntax itself is
  unchanged (§4.5.2.4), pinned bit-identical to the AOT-6 decode of
  the same spectra. `ScalableFrame::parse` / `::write` round-trip
  the whole per-layer payload stack byte-exactly.
- **Layer combination** (§4.5.2.2.4 SIAQ, `ScalableDecoder`): the
  dequantized spectra of all layers sum per output path under the
  Table 4.91–4.93 per-band tool rules — a lower layer's PNS band
  survives only while every higher layer decodes the band to zero
  (§4.6.13.6), intensity accumulates the M/L channel with positions
  from the highest layer, invalid combinations surface
  `Error::ScalableLayerCombination` — then the §4.6.14.2.1 FSS merges
  the combined mono spectrum into the stereo pair (`L/R += 2·M''` on
  clear `diff_control_lr` bits, `M = M'' + M'` on M/S bands; long
  and short windows), the cumulative-mask M/S butterfly, the
  scalable-invariant intensity reconstruction
  (`invert_intensity() = +1`), correlated PNS via `ms_used`, the
  §4.6.9.5 / Table 4.158 **serial TNS** layout (first mono layer's
  filter serves the low bands up to the highest mono `max_sfb`,
  first stereo layer's filters serve L/R, with the lower-boundary
  override rule) and the §4.6.11 filterbank. §4.6.7.5 **base-layer
  LTP** runs on the lowest layer only, its reconstruction history
  fed by a parallel first-layer-alone synthesis chain (pinned by a
  history-isolation test). Both the 1024- and 960-line families.
- **Transport**: the LATM/LOAS driver recognises AOT-6/20 layers,
  collects each program's layer payloads per access unit and decodes
  them combined through a persistent per-program `ScalableDecoder`
  (`ScalableConfig::from_layer_ascs` validates the layer stack;
  `dependsOnCoreCoder == 1` — a CELP core — and TwinVQ lower layers
  are out of scope, `Error::ScalableUnsupportedCore`).
- Single-layer scalable streams are pinned **bit-identical** to the
  equivalent SCE / common-window CPE decodes; multi-layer stacks are
  pinned against references composed from the crate's own
  reconstruction primitives; every branch carries a deterministic
  bit-flip / truncation battery (`tests/scalable_*.rs`).

### Error protection (EP) tool — §1.8

The MPEG-4 Audio unequal-error-protection layer, from the out-of-band
configuration to the LOAS EP carrier (`ep_config` / `ep_fec` /
`ep_rs` / `ep_frame`):

- **`ErrorProtectionSpecificConfig()`** (Table 1.49) — parse +
  bit-exact write with reserved-field rejection, the §1.8.4.2
  `class_optional` expansion (pinned against the spec's own
  Table 1.57/1.58 example), and ASC integration: `epConfig == 2 / 3`
  now parse the inline config and the `directMapping` bit.
- **SRCPC** (§1.8.4.6) — the rate-1/4 systematic recursive
  convolutional encoder (Figure 1.10 equations), the Table 1.61
  puncture family 8/8..8/32, §1.8.4.6.2 termination (the `u = d`
  tail rule, proven identical to the whole Table 1.60 listing) and a
  hard-decision 16-state Viterbi decoder correcting channel errors.
- **In-band header FEC** (§1.8.4.3 Table 1.59) — majority, BCH(7,4),
  BCH(15,7), Golay(23,12), BCH(31,16) with the normative generators
  and bounded-distance correction; CRC4 + terminated SRCPC 8/16 for
  17+ bits; the extended `header_protection` path.
- **Shortened Reed-Solomon** (§1.8.4.7) — `SRS(255−l, 255−2k−l)`
  over the spec's GF(2⁸) (`m(x) = x⁸+x⁴+x³+x²+1`; the generated
  antilog table is pinned against Table 1.62 rows), the part split
  with zero-padded last part, lowest-order-first parity, and the
  syndrome / Berlekamp-Massey / Chien / Forney correction chain
  (`k` byte errors per part corrected, `k+1` rejected).
- **`ep_frame()`** (§1.8.2.2, `EpFrameCodec`) — the FEC-protected
  `choice_of_pred` + `class_attrib()` header (in-band Table 1.55
  rate / Table 1.56 CRC escapes, `num_stuffing_bits`), per-class
  §1.8.4.5 CRC (the family now reaches down to CRC1) + SRCPC / SRS
  protection, §1.8.4.4 RS chains, the "until the end" class-length
  recovery (§1.8.4.1), §1.8.4.9 class-reordered transmission, and
  the §1.8.4.8 recursive interleaver (`k = m·D + min(m, d) + n`;
  bitwise for SRCPC, bytewise for RS) in modes 0 / 1 / 2 with the
  per-class mode-2 `interleave_switch`. Encode ↔ decode round-trips
  across the configuration matrix; errors are corrected through the
  whole frame; a full bit-flip battery never panics. Two
  under-specified corners (an escaped rate on an RS class; byte-wise
  interleave over a non-octet-aligned Y stream) are rejected rather
  than guessed.
- **LOAS EP carrier** (§1.7) — the `EPAudioSyncStream()` BCH(36,18)
  `headerParity` (§1.7.2.2.2 generator; generate + verify),
  `EPMuxElement(1, 1)` (majority-protected `epUsePreviousMuxConfig`,
  Golay-protected `epSpecificConfigLength`, Table 1.59-protected
  inline config with threaded reuse), and
  `LoasDecoder::decode_all_ep` — the recovered `ep_frame()` class
  concatenation is the plain `AudioMuxElement()` bit stream
  (§1.7.3.2.1: the sensitivity-category instances ride in syntax
  order), so payloads (scalable programs included) ride the
  existing decode paths. A writer-assembled EP stream decodes
  **byte-identical** to its plain LOAS equivalent and survives
  correctable channel errors.

### SSR gain control (§4.6.12) — complete decode pipeline

The §4.6.12 SSR (Scalable Sample Rate, AOT 3) gain-control tool is
implemented **end to end** — front-half filterbank, gain
reconstruction, and IPQF synthesis — and wired into the decode driver
(ADTS profile 2 routes every SCE / CPE channel through it), validated
independent of any external SSR implementation:

- **Gain-control reconstruction** (`gain_control`) — §4.6.12.3.1–3. The
  §4.6.12.3.1 gain-control data decoding (the Table 4.108 `AdjLoc()` =
  `8·AC` and Table 4.109 `AdjLev()` = `AV − 4` tables, the `NADW` /
  `ALOC` / `ALEV` ladder with the step-(3) `ALOC(0)=0` / `ALEV(0)` rule
  and the step-(4) per-window-sequence endpoint), the §4.6.12.3.2
  gain-control function setting (the `M_{W,B,j}` index, the `FMD`
  fragment-modification function with the `Inter(a,b,j)` geometric-blend
  ramp, the per-sequence `GMF` composition threading the cross-frame
  `PFMD`, and the inversion `AD(j) = 1/GMF(j)`), and the §4.6.12.3.3
  windowing + overlapping (`GainBandState::window_overlap` applies
  `T = AD·U` then overlap-adds per `window_sequence` into the band sample
  data `V_B`, threading the cross-frame `PT_B` tail). All four
  `window_sequence` shapes are covered; the spec initial values
  `PFMD ≡ 1.0` / `PT ≡ 0.0` are honoured, and the input-read vs
  produced `PFMD` lengths (which differ per sequence) are tracked
  separately with a persistent 256-entry carry.
- **IPQF synthesis filter** (`ipqf`) — §4.6.12.3.4. The Table 4.110
  length-96 prototype `Q(j)` (the symmetric `Q(j) = Q(95 − j)` half
  mirrored to 96), the cosine modulation
  `Q_B(j) = Q(j)·cos((2B+1)(2j−3)π/16)`, the 4× upsampling
  `Ṽ_B(j) = V_B(j/4)`, and the streaming convolution
  `AS(n) = Σ_B Σ_j Q_B(j)·Ṽ_B(n−j)` as a polyphase bank (`Ipqf`) that
  retains a 24-deep per-band history across frames — pinned by an
  impulse-response test against the direct §4.6.12.3.4 convolution
  (`AS(n) = Q_0(n)`).
- **Per-channel driver** (`ssr`) — `SsrGainControl::decode_frame`
  composes the four-band `GainBandState` and the `Ipqf` into one
  persistent per-channel pipeline: the four per-band IMDCT outputs
  `U_{W,B}` plus the decoded `gain_control_data()` → the §4.6.12.3.3
  per-band windowing/overlap → the §4.6.12.3.4 IPQF synthesis → the PCM
  `AS(n)` (1024 samples/frame for the steady `ONLY_LONG` / `EIGHT_SHORT`
  case). PQF band 0 is never gain-controlled.

- **Front-half filterbank** (`ssr_filterbank`) — §4.6.12.1 /
  13818-7 §16.1, closing the previously docs-gapped spectrum→band
  mapping: the frequency-ascending spectrum splits into four
  *contiguous* PQF-band quarters (the PQF's band `B` covers the `B`-th
  quarter, Annex C.2.1.1), the "even" bands — the spec's ordinal
  2nd/4th, i.e. 0-based 1 and 3, exactly the bands the ×4 decimation
  spectrally inverts — are reversed, and each band runs a 256-line
  (long) / 8 × 32-line (short) IMDCT under the quarter-scale
  §4.6.11.3.2 window geometry (`N_l/N_s = 512/64`; the KBD windows are
  generated with the α = 4 / α = 6 running-sum construction and pinned
  against the normative Table 4.A.14 / 4.A.13 listings). The split +
  reversal convention is pinned by a tone-placement test against the
  Annex C.2.1.1 analysis-PQF definition.
- **Per-channel pipeline + driver wiring** (`ssr::SsrChannelDecoder`,
  `element_decode`) — the complete spectrum → PCM chain (front half →
  §4.6.12.3 gain compensation/overlap → IPQF), replacing the §4.6.11
  filterbank for AOT 3 in the decode driver (per-channel-slot state,
  `gain_control_data()` from the channel body; note the §4.6.12.3.3
  variable frame lengths — 1472 / 576 PCM samples for `LONG_START` /
  `LONG_STOP`). Validated by full round-trip tests against the Annex
  C.2.1.1 analysis PQF: steady long frames and a complete
  window-transition chain reconstruct at err/sig < 1e-3 (the PQF
  pair's near-perfect-reconstruction bound, both window shapes), gain
  ladders applied encoder-side cancel end to end, and an
  ADTS-profile-2 stream decodes through the public `StreamDecoder`
  (mono + stereo).

### SBR bitstream decode (HE-AAC)

The full SBR side-info path is now decoded from the `extension_payload`
SBR element down to the reconstructed quantized envelope / noise-floor
scalefactors — every numeric table sourced from the ISO/IEC 14496-3
spec PDF (the §4.A normative Huffman grids and the §4.4.2.8 syntax
tables), independent of any external SBR table extraction.

- **SBR Huffman codebooks** (`sbr_huffman`) — §4.A.6.1, all ten
  normative envelope / noise codebooks (Tables 4.A.79–4.A.88)
  transcribed from the spec codeword grids and validated complete +
  prefix-free. `sbr_huff_dec()` reads MSB-first and returns the signed
  DPCM delta (`index − LAV`); `env_tables()` / `noise_tables()` pick the
  `(t_huff, f_huff)` pair from the §4.6.18.3 coupling / channel /
  `bs_amp_res` selection (the freq-direction noise tables alias the
  3.0 dB envelope freq tables per Table 4.A.78 Note 2).
- **`sbr_header()`** (`sbr_header`) — §4.4.2.8 Table 4.63: the
  fixed-width header plus the two optional extra blocks, with the
  Table 4.63 Note 3 defaults (Tables 4.105–4.111) applied when an extra
  flag is clear. `band_geometry_changed()` flags a §4.6.18.3.3 reset,
  and `derive_bands()` chains into the band-setup pipeline below.
- **`sbr_grid()` / `sbr_dtdf()` / `sbr_invf()`** (`sbr_grid`) —
  §4.4.2.8 Tables 4.69–4.71: all four `bs_frame_class` layouts (FIXFIX
  / FIXVAR / VARFIX / VARVAR) with the envelope count, variable /
  relative borders, `ptr_bits = ceil(log2(num_env + 1))` pointer,
  reversed FIXVAR freq-res order, single-envelope FIXFIX `bs_amp_res`
  override, and `bs_num_noise` derivation; the delta-direction flags;
  and the per-noise-band 2-bit inverse-filtering modes.
- **`sbr_envelope()` / `sbr_noise()`** (`sbr_envelope`) — §4.4.2.8
  Tables 4.72–4.73: the raw `bs_data_*` delta arrays, with the
  fixed-width absolute start value (5/6/7-bit per the coupling /
  channel / `bs_amp_res` context; noise always 5-bit) and the
  frequency- vs time-direction Huffman deltas, over `NHigh` / `NLow`
  envelope bands and `NQ` noise bands.
- **Envelope / noise DPCM reconstruction** (`sbr_reconstruct`) —
  §4.6.18.3.5: inverts the delta coding to the quantized scalefactors
  `E_Q(k,l)` / `Q(k,l)`. Frequency deltas accumulate from the start
  value; time deltas add to the reference envelope (previous in-frame,
  or the prior frame's last envelope for `l == 0`) with the `i(k)`
  high↔low band remap when the reference resolution differs; the
  coupled second channel's `δ = 0.5` is applied as an integer ×2 on the
  even transmitted values, threading cross-frame state.
- **Element framing** (`sbr_element`) — §4.4.2.8 Tables 4.65 / 4.66 /
  4.74: `SbrElement::parse_single` / `parse_pair` decode a whole SBR
  data element in spec order — the optional `bs_data_extra` field, the
  per-channel grid / dtdf / invf / envelope / noise blocks (coupled
  shared-grid vs. independent-grid layouts, second coupled channel in
  balance mode), the `sbr_sinusoidal_coding()` add-harmonic flags, and
  the `bs_extended_data` block (id + raw body captured for a later PS
  pass). The single-envelope FIXFIX `bs_amp_res` override is applied
  before envelope decode.
- **`sbr_extension_data()`** (`sbr_extension`) — §4.4.2.8 Table 4.62:
  the top-level walker that ties the header + element framing into a
  whole SBR extension payload, in spec order — the optional 10-bit
  `bs_sbr_crc_bits` (for the `EXT_SBR_DATA_CRC` type), the
  `bs_header_flag` + `sbr_header()`, then `sbr_data(id_aac, bs_amp_res)`
  dispatching onto `parse_single` (ID_SCE) / `parse_pair` (ID_CPE) with
  the band tables derived from the active header at the SBR internal
  rate (`FsSBR = 2·core`), and the trailing `bs_fill_bits` alignment
  (`num_align_bits = (8·cnt − 4 − num_sbr_bits) % 8`). A clear
  `bs_header_flag` reuses the threaded previous header (the
  non-scalable core fixes `sbr_layer == SBR_NOT_SCALABLE`, so the flag
  is always present). Reachable from the natural FIL entry point via
  `extension_payload::ExtensionPayload::parse_with_sbr`, which routes
  the SBR extension types here (the default `parse` still rejects them,
  keeping the byte-exact AAC-LC corpus path untouched).

The SBR *bitstream* side info is decoded end to end — CRC field,
header, element framing, band tables, and envelope / noise DPCM
reconstruction — and the **back-end DSP is now implemented too** (see
the next section).

### SBR back-end (HE-AAC v1) — §4.6.18

The complete SBR reconstruction chain, from the core decoder's time
signal to dual-rate PCM, validated **99.98% sample-exact (max error
1 LSB)** against the staged HE-AAC v1 `expected.wav`:

- **QMF filterbanks** (`sbr_qmf`) — §4.6.18.4 / Figures 4.42–4.44: the
  Table 4.A.89 640-tap prototype window (transcribed digit-for-digit
  from the spec PDF), the 32-band complex analysis bank, the 64-band
  real-output synthesis bank (dual-rate), and the downsampled
  32-channel synthesis variant. Pinned by near-perfect-reconstruction
  properties (< 1e-4 error ratios).
- **Dequantization + stereo decoding** (`sbr_dequant`) — §4.6.18.3.5:
  `EOrig = 64·2^(E/a)`, `QOrig = 2^(6 − Q)`, and the coupled-pair pan
  split with `panOffset = [24, 12]` (energy-sum-preserving).
- **Time / frequency grid** (`sbr_time_grid`) — §4.6.18.3.3: the
  `tE` / `tQ` border vectors for all four frame classes, the
  Table 4.174 `middleBorder` and the Table 4.176 `lA`.
- **HF generation** (`sbr_hf_gen`) — §4.6.18.6: the Figure 4.48 patch
  construction, the covariance-method second-order inverse filtering
  (`εInv = 1e-6`, `|α| ≥ 4` reset), the Table 4.175 chirp-factor
  blend, and the patched `XHigh` generator.
- **Limiter band table** (`sbr_limiter`) — §4.6.18.3.2.3 /
  Figure 4.41, fed by the patch borders (closing the previously
  deferred limiter-table item).
- **Envelope adjustment** (`sbr_env_adjust` + `sbr_noise_table`) —
  §4.6.18.7: mapping, `ECurr` estimation (both `bs_interpol_freq`
  regimes), amplitude-domain gains (the spec PDF's typeset equations
  carry square roots the plain text layer drops), the limiter /
  boost compensation, `hSmooth` smoothing with cross-frame tails, the
  Table 4.A.91 noise table with the running `fIndexNoise`, and the
  sinusoid injection with the `(−1)^(m+kx)` alternation.
- **Frame driver** (`sbr_decoder`) — §4.6.18.5 / Figure 4.47: the
  `tHFGen = 8`-slot `XLow` history, header-reset handling, the
  `lTemp` splice of the previous frame's `Y'`, the coupled-pair invf
  sharing, and the pure-upsampling path for SBR-less frames.
- **Stream wiring** (`decode`) — the ADTS `StreamDecoder` walks FIL
  extension payloads via `extension_payload::parse_with_sbr`, attaches
  each SBR payload to its preceding SCE / CPE, threads the
  `sbr_header()` reuse state per element slot, and (once SBR-active)
  emits every frame at the doubled rate — 2048 samples/channel — with
  SBR-less frames upsampled so the output rate never flaps. The
  runtime `Decoder` trait surfaces the dual-rate frames unchanged
  (pinned byte-identical to the raw `StreamDecoder`).
- **Downsampled output mode** (§4.6.18.4.3) — selectable end to end:
  `SbrDecoder::set_downsampled` / `StreamDecoder::set_sbr_downsampled`
  / the `sbr_downsampled` codec option run the 32-channel synthesis
  bank so an SBR-active stream is emitted at the *core* rate (1024
  samples per channel per frame; the SBR range above the core Nyquist
  is discarded by construction, the bands below it are kept). The
  LATM driver selects the mode automatically when an explicitly
  signalled ASC carries `extensionSamplingFrequency ==
  samplingFrequency` (the §4.6.18.2.6 in-band core-rate declaration),
  and PS composes (stereo through two downsampled banks). Validated
  on the HE-AAC v1 fixture at **1.8e-4** per-channel err/sig RMS
  against a band-limited 2:1 decimation of the reference decode
  (v2 PS at 1.95e-4), byte-identical between the LATM and forced-ADTS
  paths.
- **Low power SBR tool** (§4.6.18.8) — selectable end to end
  (`SbrDecoder::set_low_power` / `StreamDecoder::set_sbr_low_power` /
  the `sbr_low_power` codec option; composes with the downsampled
  output): the §4.6.18.8.2 real-valued filterbank trio (`sbr_qmf`),
  the §4.6.18.8.3 aliasing detection (`sbr_lp` + the reflection
  coefficients in `sbr_hf_gen`: the Figure 4.53 degree walk, the
  patch-carried `degPatched`, the Figure 4.54 gain groups), the
  §4.6.18.8.4 ×2 energy estimation, and the §4.6.18.8.5 aliasing
  reduction (`GLimBoost → GA`, exact group-energy restoration),
  no-smoothing rule, modified real-valued sinusoid injection
  (−0.00815 neighbour correction, first-16 rule, `kx − 1` / `kx + M`
  spill) and modified `X` assembly. Validated on the HE-AAC v1
  fixture: sub-crossover content at **9e-5** err/sig RMS against the
  reference with per-frame full-band energy within 0.05% (the
  real-valued HF path is energy-normative, not phase-normative). A
  PS payload in this mode is rejected (`Error::SbrLowPowerPs` — the
  subpart-8 tool needs the complex QMF domain). All four mode
  combinations survive a deterministic corruption battery
  (`tests/sbr_mode_mutations.rs`).

### SBR frequency band setup (HE-AAC)

- **SBR frequency band tables** (`sbr_freq_bands`) — §4.6.18.3.2 the
  static, header-only half of the Spectral Band Replication band setup,
  computed directly from the closed-form spec algorithm (no QMF back-end
  required):
  - `k0` / `k2` — §4.6.18.3.2.1 the low and high QMF subband
    boundaries. `k0 = startMin + offset(bs_start_freq)` with the
    per-`FsSBR` `offset` table and the `startMin = NINT(c·128/FsSBR)`
    thresholds; `k2` covers the `bs_stop_freq < 14` `stopDkSort`
    accumulation path and the `bs_stop_freq == 14 / 15`
    `min(64, 2·k0)` / `min(64, 3·k0)` shortcuts.
  - `master_table` — §4.6.18.3.2.1 `fMaster`, both the Figure 4.39
    linear path (`bs_freq_scale == 0`, the `dk`/`vDk`/`k2Diff`
    away-from-zero correction walk) and the Figure 4.40 warped path
    (`bs_freq_scale > 0`, the `bands`/`warp` log-spaced regions with
    the single-/two-region split at `k2/k0 > 2.2449` and the
    `min(vDk1) < max(vDk0)` smoothing step).
  - `HiLoTables::derive` — §4.6.18.3.2.2 the derived `fTableHigh`,
    `fTableLow` (the `i(k) = 2k − (1−(−1)^NHigh)/2` decimation), and
    `fTableNoise` (the `NQ = max(1, NINT(bs_noise_bands·log2(k2/kx)))`
    band count plus its `i(k)` recursion), along with the `M` and
    `k_x` outputs every later SBR stage keys off.
  - The §4.6.18.3.6 requirements are enforced (`k2 > k0`,
    `numBands > 0`, `vDk > 0`, `bs_xover_band < NMaster`), surfacing
    `Error::SbrFreqBandInvalid` on violation. The §4.6.18.3.2.3
    limiter band table is out of scope here — its `bs_limiter_bands >
    0` path consumes the §4.6.18.6 patch borders that need the QMF
    patching back-end.

### Parametric Stereo (HE-AAC v2) — subpart 8 / Annex 8.A

The complete §8.6.4 PS tool, reconstructing a stereo image from the
mono SBR signal, validated **5e-5 per-channel error-to-signal RMS**
against the staged HE-AAC v2 MP4 fixture (filterbank-rounding level):

- **Bitstream** (`ps_data`, `ps_huffman`) — §8.4.2 Tables 8.9–8.14:
  the persistent `enable_ps_header` configuration, FIX/VAR framing
  (Table 8.29), per-envelope IID/ICC/IPD/OPD delta rows on all ten
  Annex 8.B codebooks (each verified a complete prefix code; the six
  IID/ICC books cross-checked leaf-for-leaf against the staged
  `ps-huffbook-*.csv` trees), and the §8.5.2 time/frequency DPCM
  resolution with range checks and modulo-8 phase wrap.
- **Hybrid filterbank** (`ps_hybrid`) — §8.6.4.3: both configurations
  (71 / 91 sub-subbands) on the Table 8.37/8.38 13-tap prototypes,
  with the Figure 8.20 merge/reorder, the odd-QMF-band inversion, and
  the Annex 8.A.3 zero-delay alignment (6 look-ahead `XLow` slots + 6
  history slots). Analysis→synthesis reconstructs exactly.
- **De-correlation** (`ps_decorr`) — §8.6.4.5: the 3-link complex
  all-pass chain behind `z⁻²·φ_fract`, the Table 8.40/8.41 centre
  frequencies, the 14-/1-slot delays above `NR_ALLPASS_BANDS`, and
  the transient duck (peak decay / smoothing / γ = 1.5) per stereo
  band, with the Annex 8.A.3 partial + full resets.
- **Stereo processing** (`ps_stereo`, `ps_map`) — §8.6.4.6: Table
  8.25/8.26/8.28 dequantization (cross-validated against the staged
  Q30 tables), mixing procedures Ra and Rb, IPD/OPD three-position
  smoothing, the Table 8.48/8.49 `b(k)` maps + conjugate channels,
  the Table 8.45/8.46 10↔20↔34 re-mappings, and the §8.6.4.6.4
  border interpolation with hold semantics.
- **Frame driver + wiring** (`ps_decoder`, `sbr_decoder`) — Annex
  8.A: inactive (mono) until the first header'd `ps_data()`,
  parameter hold over payload-less frames, band-count switches, and
  the per-frame de-correlator reset above `k_x + M`. A PS-carrying
  SCE renders stereo through two synthesis banks in both the
  SBR-processed and pure-upsampling paths, end to end through the
  ADTS / LATM / raw `StreamDecoder` entries and the runtime
  `Decoder`.

### LATM / LOAS transport framing

The §1.7 low-overhead transport layer is now decoded from the LOAS
sync frame down to the recovered MPEG-4 Audio access units — every
field sourced from the ISO/IEC 14496-3 §1.7 syntax tables.

- **`StreamMuxConfig()`** (`latm::StreamMuxConfig`) — §1.7.3.1
  Table 1.42 plus `LatmGetValue()` (Table 1.43). Decodes the whole
  multiplex configuration: the `audioMuxVersion` / `audioMuxVersionA`
  version flags (with the `audioMuxVersion == 1` `taraBufferFullness`
  and per-ASC length-prefix + `fillBits` extensions),
  `allStreamsSameTimeFraming`, `numSubFrames` / `numProgram` /
  per-program `numLayer`, and the per-`streamID[prog][lay]`
  `LayerConfig` table — each layer carrying its inline
  `AudioSpecificConfig()` (parsed via the `asc` module's
  `parse_bits` / `parse_bits_bounded` entry points) or the resolved
  `useSameConfig` inheritance, the `frameLengthType`, and the type-0
  `latmBufferFullness` / CELP-core `coreFrameOffset` or type-1
  `frameLength`. The `crcCheckSum` is recomputed against the
  configuration prefix via the §1.8.4.5 `CRC8` generator and
  validated. The reserved `audioMuxVersionA == 1` branch and the
  CELP (`3`/`4`/`5`) / HVXC (`6`/`7`) `frameLengthType` values index
  frame-length tables for object types this AAC-focused crate does not
  decode, so they surface dedicated errors.
- **`AudioMuxElement()`** (`latm::AudioMuxElement`) — §1.7.3.1
  Tables 1.41 / 1.44 / 1.45. Recovers a whole multiplexed element: the
  `muxConfigPresent` `useSameStreamMux` branch (inline
  `StreamMuxConfig()` vs. inherited previous config), the per-subframe
  `PayloadLengthInfo()` + `PayloadMux()` loop over `numSubFrames + 1`
  frames (both the `allStreamsSameTimeFraming` program/layer walk and
  the `numChunk` chunk layout with its `streamIndx` + `AuEndFlag`), the
  `frameLengthType`-0 `MuxSlotLengthBytes` 8-bit-escape byte count and
  the `frameLengthType`-1 fixed `(frameLength + 20) * 8` bits, the
  `otherData` skip, and the trailing `ByteAlign()`. Each access unit is
  returned as a `MuxPayload` carrying the raw §4.4.2.1
  `raw_data_block()` bytes.
- **`AudioSyncStream()` / `EPAudioSyncStream()`**
  (`latm::AudioSyncStream`, `latm::EpAudioSyncHeader`) — §1.7.2.1
  Tables 1.36 / 1.37. `AudioSyncStream` scans a LOAS byte buffer for
  the 11-bit `0x2B7` syncword, reads the 13-bit `audioMuxLengthBytes`,
  and decodes the byte-aligned `AudioMuxElement(1)` body, exposing an
  `Iterator` of `LoasFrame`s with the `StreamMuxConfig` threaded across
  frames for `useSameStreamMux` inheritance. `EpAudioSyncHeader`
  decodes the `EPAudioSyncStream` FEC header (`0x4DE1` syncword,
  `futureUse`, `audioMuxLengthBytes`, `frameCounter`, `headerParity`)
  and reports the byte-aligned `EPMuxElement` body offset.

- **`LoasDecoder`** (`latm::LoasDecoder`) — the end-to-end LATM/LOAS →
  PCM driver. `decode_all` walks the `AudioSyncStream`, and for every
  recovered `MuxPayload` drives the payload's §4.4.2.1 `raw_data_block()`
  through the shared `decode::StreamDecoder::decode_raw_data_block` core,
  configuring the decode from the layer's `AudioSpecificConfig` (AOT /
  `samplingFrequencyIndex` / resolved sample rate). One `StreamDecoder`
  is held per `streamID[prog][lay]` so each multiplexed stream's
  §4.6.11 overlap / §4.6.7 LTP / §4.6.6 predictor state threads
  independently. An SBR-signalling ASC (explicit AOT-5 wrapper or
  implicit AAC-LC-only) rides the same §4.6.18 auto-detect the ADTS
  path uses and emits dual-rate output, and a PS payload synthesizes
  stereo through the Annex 8.A tool. Pinned against the
  `aac-latm-stream` fixture
  (stereo, 44.1 kHz) to a §8 PCM-RMS error ratio of 0.0004, proven
  bit-identical to a hand-fed `decode_raw_data_block` pass, and — for
  a re-multiplexed HE-AAC v1 stream (both signalling modes) —
  byte-identical to the ADTS decode.

The runtime `Decoder` (`codec_decoder::AacDecoder`) auto-detects its
carrier on the first packet and routes LOAS packets through `LoasDecoder`
(see "Runtime `Decoder` registration" below). The `EPMuxElement()` EP-tool
payload de-interleave decodes via `LoasDecoder::decode_all_ep` (see
the EP section below).

### Stream decode + PCM output

- **Integer-PCM rendering** (`pcm`) — §4.6.11 filterbank `f64` time
  signal → 16-bit signed PCM: `nint` (the §1.3 `NINT()` round-half-
  away-from-zero operator), `to_s16` (round + saturate), `channel_to_s16`,
  and `interleave_s16` (element-order interleave). The conversion is the
  only output-rendering step (no resampler / dither), so it is fully
  spec-determined. The **canonical channel reorder** for default
  `channelConfiguration` layouts (Table 1.19, see `channel_map` below) is
  applied to the per-channel buffers *before* this interleave.
- **Default-config channel reorder** (`channel_map`) — ISO/IEC 14496-3
  §1.6.3.5 / Table 1.19. A `raw_data_block()` lists its channel elements
  in bitstream order, so the decoder produces channels in element order
  (e.g. a 5.1 stream as `C, L, R, Ls, Rs, LFE` for `SCE, CPE, CPE, LFE`);
  `channel_map::reorder_channels` permutes them into the canonical
  interleaved order that `oxideav_core::ChannelLayout` adopts (the
  WAVE_FORMAT_EXTENSIBLE / BS.775 convention — 5.1 becomes
  `L, R, C, LFE, Ls, Rs`). The driver threads the signalled
  `channelConfiguration` through `decode_raw_data_block` and applies the
  reorder for every default config **1–7** — mono / stereo are identity
  permutations and config 7 (the Table 1.19 7.1 arrangement: centre +
  inner Lc/Rc centre-front pair + outer L/R front pair + surround pair
  + LFE) rank-sorts to `L, R, C, LFE, Lc, Rc, Ls, Rs`. **Config 0
  (PCE-defined) layouts are also mapped**: `channel_map::pce_speaker_assignment` implements the
  ISO/IEC 13818-7 §8.5.2.2 rules — the front list center-outward
  (lone SCE = center, SCE pairs L-then-R, two front pairs = the
  Table 42 inner Lc/Rc + outer L/R arrangement), the side list front
  to back, the back list outside-in (outer pair = side surround,
  inner = rear; a final unpaired SCE = rear center), one LFE — keyed
  by `(element kind, instance tag)` so the block's element order is
  irrelevant; unmappable shapes fall back to element order. The
  decode driver captures an in-band PCE (§8.5.2.2 persistence),
  `StreamDecoder::set_program_config` installs an out-of-band
  (ASC-inline) one, and the LATM driver does so automatically. The
  whole path is validated end to end in `tests/multichannel_mp4.rs`
  (a minimal ISO 14496-12 sample-table + esds walk): the 5.1
  config-6 fixture at 2.4e-4–8.9e-4 per-channel err/sig RMS, the
  **7.1 PCE fixture at 2e-5–2.9e-4**, and the **hexagonal custom
  6.0 PCE fixture at 2.2e-4–7.8e-4** — every speaker carries a
  distinct source tone, so the per-channel ratios pin the mapping
  (silent LFEs reproduced exactly).
- **Stream-level ADTS decode driver** (`decode`) — `StreamDecoder` walks
  the §4.4.2.1 `raw_data_block()` of each ADTS frame above the
  per-element driver, keying one `ElementDecoder` per
  `(syntactic-element-id, element_instance_tag)` slot so every element's
  §4.6.11 overlap / §4.6.7 LTP / §4.6.6 predictor state persists across
  frames, and renders to element-order interleaved s16 PCM. `decode_all`
  walks a whole raw-ADTS buffer (ID3v2-skip + `aac_frame_length`
  framing). **The decoded PCM is validated against the staged
  `expected.wav` corpus**: the two PNS-free ADTS fixtures
  (`aac-lc-mono-8000-16kbps-adts`, `aac-lc-intensity-stereo`) are
  **99.9% byte-exact** to the reference s16 output with a **max error of
  1 LSB** — the residual is purely the difference between this crate's
  `f64` direct-sum IMDCT and a `float32` fast transform. The PNS-bearing
  fixtures are compared in the PCM RMS domain (per the fixtures-doc §8),
  where the error-to-signal RMS ratio stays below 0.1%; full
  byte-exactness on those is precluded by the §4.6.13.3 spec-undefined
  noise-phase RNG (energy is normative, phase is not). A
  `coupling_channel_element()` (CCE) carried in the block is **decoded
  and applied**: the block walk is two-pass, so the §4.6.8.3.3
  coupling contribution lands on the addressed SCE / CPE channels at
  the signalled `cc_domain` stage whether the CCE precedes or follows
  its targets (see the `cce` bullet above). Multi-`raw_data_block`
  frames decode as consecutive 1024-sample blocks (per-block channel
  render + time concatenation), and `decode_adts_frame` verifies the
  whole §8.1.1 `error_check()` CRC layer when
  `protection_absent == 0` (see `adts_crc` above).
- **Runtime `Decoder` registration** (`codec_decoder`) — `AacDecoder`
  adapts the persistent `StreamDecoder` / `LoasDecoder` into the
  framework's packet-in / frame-out `oxideav_core::Decoder` trait. It
  **auto-detects the carrier** on the first packet — the `0xFFF` ADTS
  syncword vs. the `0x2B7` LOAS `AudioSyncStream` syncword — and then
  routes every later packet the same way: ADTS frames through
  `StreamDecoder::decode_frame` (ID3v2-skip + `aac_frame_length`
  framing; one or many frames per packet), LOAS packets through
  `LoasDecoder::decode_all` (one or many sync frames per packet, with
  the `StreamMuxConfig` and per-stream state threaded across packets).
  `receive_frame` returns one interleaved-S16 `AudioFrame` (1024
  samples/channel) per decoded access unit, `flush` drains to `Eof`,
  and `reset` drops both backends and re-arms carrier detection for a
  clean post-seek restart. `register()` installs it under id `"aac"`,
  claiming the MP4 object-type `0x40`, WAVEFORMATEX `0x00FF` / `0x1601`,
  the `mp4a` / `aac ` FourCCs, and the Matroska `A_AAC` CodecID; the
  probe scores a structurally-confirmed ADTS header at 1.0 and a bare
  LOAS syncword at 0.9 to win shared tags. Both carrier outputs are
  pinned byte-identical to their underlying `StreamDecoder` /
  `LoasDecoder`.

### AAC-LC encoder

- **`encoder`** — the §4.5/§4.6-written-forward AAC-LC encode chain.
  `StreamEncoder` consumes interleaved S16 PCM hop by hop
  (`encode_frame` / `finish` / one-shot `encode_all`) and emits one
  complete ADTS frame per 1024-sample hop with a 1024-sample encoder
  delay. Per hop: an energy-jump transient detector over the
  `[hist | cur]` subblock grid drives the §4.6.11.3.2
  `ONLY_LONG → LONG_START → EIGHT_SHORT → LONG_STOP` state machine;
  the §4.6.11.3.1 forward MDCT runs under the same composite windows
  the decoder synthesizes with (one 2048-point transform for long
  sequences, eight 256-point transforms at `448 + j·128` for short);
  `EIGHT_SHORT` frames merge envelope-alike adjacent windows into
  §4.5.2.3.4 window groups (one scalefactor/section track per group,
  §4.5.2.3.5 interleaved transmission order; the emitted 7-bit mask
  is pinned as the exact inverse of the decoder-side derivation);
  per-band scalefactors follow a masking-spread rule (band target
  magnitude `42·(peak_b/peak_frame)^½`, sub-step bands culled to
  `ZERO_HCB`) with the DPCM ±60 track threaded across window groups;
  a bidirectional rate loop (±4 scalefactor ladder + ±1..3 fine pass)
  fits each frame to the bitrate-derived byte budget; codebooks and
  sections come from a **measured-bit-cost dynamic program** (every
  candidate same-class run priced at its `section_data()` header
  overhead plus the cheapest Table 4.95 book, actual codeword +
  sign + escape bits measured with the real tuple writer — pinned
  never-larger than the classic smallest-LAV + merge rule), and
  long frames additionally price a §4.4.6.3 `pulse_data()` variant
  (band outliers reduced to the rest-of-band floor, restored
  bit-exactly by the decoder's §4.6.3.3 fix-up; kept only when the
  measured channel stream is smaller).
  Stereo pairs code per-band §4.6.8.1 M/S (`m=(l+r)/2`, `s=(l−r)/2`
  where the transform concentrates band energy, emitted as
  `ms_mask_present` 2 / 1+mask / 0) inside a `common_window` CPE —
  long frames per sfb, short frames per `(window group, sfb)` under
  the pair's **joint** grouping (decided once on the pair envelope;
  independent decisions would desync the shared `ics_info`).
  **Every Table 1.19 default channel layout encodes** — 1–6 and 8
  (7.1) channels: the element-plan `raw_data_block()` assembly (SCE
  / `common_window` CPE / LFE with per-kind instance tags),
  canonical-order input permuted by the exact inverse of the
  decoder's §1.6.3.5 reorder, and §4.5.2.1.3-conforming LFE elements
  (always ONLY_LONG / sine through the frame's block switching, no
  TNS, only the lowest 12 spectral lines transmitted).
  Round-trips through the crate's own decoder: multitone 128 kbps at
  0.016 err/sig RMS, staged-fixture transcodes at 0.0008–0.003,
  identical-channel stereo at 1.02× the mono stream size (short-run
  identical channels decode L exactly equal to R), multichannel
  layouts pinned with one distinct tone per speaker, and the
  wire-level window-sequence walk pins the exact
  `LongStart → EightShort → LongStop` pattern around a percussive
  burst; a deterministic bit-flip/truncation battery covers the
  multichannel and grouped-short streams.
- **`codec_encoder`** — the frame-in / packet-out
  `oxideav_core::Encoder` adaptor (`make_encoder`, honouring
  `sample_rate` / `channels` (1–6, 8) / `bit_rate`, default
  64 kbps/channel); registered alongside the decoder under id
  `"aac"`, and re-exported as `encoder::make_encoder` per the
  workspace dual-API convention.

### ER BSAC (AOT 22) — noiseless-coder bring-up (§4.4.2.6 / §4.5.2.6 / §4.6.4)

The Bit-Sliced Arithmetic Coding decoder roster is implemented and
its front half is **conformance-pinned against the ISO/IEC 14496-26
`er_bs*` corpus**; the spectral bit-slice probability *selection*
of the deployed encoder diverges from the printed spec and is the
component still open (see below):

- **Numeric tables** (`bsac_tables`) — Tables 4.A.31–4.A.77
  transcribed from the staged spec PDF: the `cband_si_type`
  parameter matrix, the scalefactor / `cband_si` / stereo / PNS
  cumulative-frequency models, the Table 4.A.34 context-position
  map, the Table 4.A.35/36 `min_p0`/`max_p0` budget clamps, and the
  22 spectral probability tables with the printed alias scheme
  resolved. (The 2001 and 2009 editions print *different* alias
  schemes — tables 11–22 onto 9/10 alternating with sub-MSB zero
  rows from 7/8 in 2009, everything onto 10 with zero rows from 8
  in 2001 — plus one conflicting cell in table 7; both were
  transcribed and tested.)
- **Arithmetic decoder** (`bsac_arith`) — the §4.5.2.6.2.7.4
  procedure exactly as listed (`decode_symbol` over 14-bit cumfreq
  models, binary `decode_bit`, the `half[]` renorm schedule, 30-bit
  init, zero-stuffing segment reader), round-tripped against a
  spec-inverse test encoder over every model.
- **Layer geometry** (`bsac_layer`) — the §4.5.2.6.2.4/5 roster:
  base sub-layer split, per-layer coding-band / spectral / sfb
  coverage (the literal `end_sfb = sfb + 1` one-band lookahead,
  corpus-confirmed), `layer_si_maxlen`, the rate-anchored
  `layer_bit_offset` derivation with the overflow/underflow
  redistribution, and the SBA `terminal_layer` marks.
- **Block decode + reconstruction** (`bsac_decode`) — the
  `bsac_header()` / `general_header()` raw-bit parse, the full
  layer walk (side info, first-pass spectra, the
  `bsac_lower_spectra()` refinement, budget carry between layers),
  bit-slice reassembly with interleaved sign decode, and the AAC
  back end (§4.6.2 dequant, group de-interleave, §4.6.8 stereo
  hooks, §4.6.9 TNS, §4.6.11 filterbank) behind a persistent
  `BsacDecoder`.
- **What the corpus pins** (`tests/bsac_bringup.rs`, corpus-gated):
  on `er_bs01_48_ep0` the silent access units decode
  **sample-exact** against the reference waveform (headers, layer
  roster, arithmetic side-info decode all in sync), and on content
  frames a TDAC oracle (the §4.6.11 perfect-reconstruction
  property recovers each frame's exact transmitted spectrum from
  the reference PCM) confirms the decoded `cband_si` MSB plane and
  scalefactor gains match the deployed encoder precisely.
- **The open divergence**: the §4.6.4.2.3 spectral-bit probability
  selection as printed decodes the wrong sliced bits partway into
  the first coding band (both editions' alias readings, several
  structural variants, and every printed row under every position
  mapping tried were tested against the oracle truth). A
  constraint solver over the real stream proves a consistent
  context→p0 dictionary *exists* — the symbol order, sign
  interleave and context classes are right — but its values match
  no printed row (the all-zero-context position demands
  `p0 ∈ {0x3700..=0x3a00}`; every plausible row prints `0x3b00+`
  there). A clean-room behavioural trace of the deployed p0
  selection (or the corrigendum text) is the standing docs ask;
  `tests/bsac_bringup.rs` carries the divergence locator and the
  solver instrument, and `tests/iso_14496_26_conformance.rs`
  reports the structural decode rate (618/703 AUs on
  `er_bs01_48_ep0`) without asserting PCM until the rule lands.

## Not yet supported

- **The deployed ER AAC LD `tns_data()` filter record.** The
  ISO/IEC 14496-26 LD conformance bitstreams transmit an
  extra-spec TNS record: the corpus-resolved 1-bit-`n_filt` reading
  (`docs/audio/aac/er-ld-tns-divergence.md` §0, implemented here)
  reconciles the *structure* — every AU parses to its boundary — but
  the reference waveforms show the record carries a real
  variable-length filter (per-AU record lengths ≈ 19–61 bits, in
  3-bit increments) whose layout matches no Table 4.54/4.155
  reading (a grammar search over length 4/6 × order 3/5 × 1–2
  filters × optional direction/compress fields, decoded *and*
  applied, reconciles none of it). TNS-bearing LD AUs (~6 % of the
  `er_ad1103*` family) therefore decode with wrong PCM until a
  behavioural trace of the deployed record lands; the conformance
  harness masks them (and bounds the LTP-setup vectors coarsely,
  since LD LTP history includes those AUs).
- Encoder-side tool remainders — the end-to-end AAC-LC encoder (see
  `encoder` below) covers block switching with §4.5.2.3.4 short-frame
  grouping, M/S on both frame shapes, the scalefactor/quantizer rate
  loop with measured-bit-cost codebook/section choice, every
  Table 1.19 default channel layout, and opt-in §4.6.13 PNS emission
  (`StreamEncoder::set_pns` — off by default because a single-frame
  spectral statistic cannot tell true noise from noise-shaped
  deterministic content such as sweeps; default-on awaits a
  cross-frame tonality measure) and default-on §4.6.9 TNS emission
  (`encoder_tns`: per-window Levinson-Durbin prediction-gain decision
  under a time-domain temporal-envelope gate, PARCOR quantised on
  the §4.6.9.3 4-bit arcsine grid, and the §4.6.7.4.1 all-zero
  analysis pass derived from the *wire* coefficients so the
  decoder's all-pole synthesis is its exact inverse) and opt-in
  §4.6.8.2 intensity-stereo emission
  (`StreamEncoder::set_intensity_stereo` — correlated high bands
  transmitted once with codebook 15/14 + `is_pos` on the §4.6.8.1.4
  track; off by default because intensity coding discards the
  pair's side information) and measured §4.4.6.3 `pulse_data()`
  emission (long-frame outlier-over-floor bands, kept only when the
  whole channel stream prices smaller; the decode-side fix-up
  restores the identical quantized spectrum), keeps
  PNS and IS long-frame-only (CPE PNS emits the §4.6.13.3 `ms_used`
  correlated-noise signalling — shared random vector — for
  both-channels-noise bands correlating above 0.5), and has no
  PCE-driven custom layouts (7-channel and beyond-7.1 shapes; the
  Table 1.19 defaults 1–6 and 8 all encode).
- SSR remainders — the §4.6.12 gain-control tool is now implemented
  and wired **end to end** (front-half filterbank, gain
  reconstruction, IPQF — see the "SSR gain control" section above),
  and a writer-assembled AOT-3 fixture driving non-unity gain
  ladders through all four window sequences (with the §4.6.12.3.3
  variable 1024/1472/576 frame lengths) is staged in the docs corpus
  (`aac-ssr-gain-control-adts`; no encoder for AOT 3 exists anywhere,
  so a captured conformance stream remains welcome — a black-box
  validator binary reports SSR gain control unimplemented, so there
  is no external oracle for the ladders). Still open: the 13818-7
  SSR-profile *bandwidth-scalable* output modes (decoding only 1–3
  PQF bands at a reduced rate) are not selectable — the decoder
  always reconstructs the full-rate signal. (The Main frequency-domain predictor,
  §4.6.6, is now
  fully wired into `element_decode` for the AAC Main object type on long
  windows — see `predictor` above. LTP, §4.6.7, is likewise wired in
  with the §4.6.7.4.1 / Figure 4.30 TNS-analysis-in-loop ordering.
  The ISO/IEC 14496-3:**2001** Table 4.55 short-window LTP *syntax* —
  the per-short-window `ltp_short_used` / `ltp_short_lag_present` /
  `ltp_short_lag` loop that the 2009 edition removed ("LTP is
  restricted to long windows only", §4.6.7.1 2009) — is parsed and
  re-encoded under the explicit `LtpEdition::Iso2001` selector
  (`parse_ltp_data_edition` / `write_ltp_data_edition`; the two
  editions are wire-incompatible there and nothing in-band signals
  which one a stream follows). The short-window *synthesis* stays
  unimplemented: the 2001 §4.6.7.3 text defines the `x_rec` buffer
  arrangement once but never fixes the per-subframe index origin for
  the eight windows, and no LTP fixture exists to disambiguate. The
  ER AAC LD long-window LTP — 10-bit lag, `ltp_lag_update` repeat,
  `M = N/2` — is implemented and exercised by the staged LD
  fixtures.)
- SBR/PS remainders — the §4.6.18 SBR tool **and** the subpart-8 PS
  tool are **implemented end to end** (see the sections above) and
  wired into the ADTS / LATM `StreamDecoder` paths and the runtime
  `Decoder`, validated against the HE-AAC v1 (99.98% sample-exact)
  and HE-AAC v2 (5e-5 RMS) fixtures, with the 10-bit
  `bs_sbr_crc_bits` of every `EXT_SBR_DATA_CRC` payload now
  **verified** (§4.4.2.8.1 `G10`, zero init, over the Table 4.62
  region — see `adts_crc`; a derived type-14 fixture is staged as
  `he-aac-v1-sbrcrc-adts`). The §4.6.18.4.3 downsampled-output mode
  and the §4.6.18.8 low-power variant are now **both selectable end
  to end** (see the SBR back-end section above). Still open: SBR is
  defined here over the 1024-line core only — an SBR payload on a
  960-line or LD stream is rejected
  (`Error::SbrUnsupportedFrameFamily`; the §4.6.19 LD SBR tool is
  ELD's and stays out of scope) — and low-power PS is undefined by
  design (the subpart-8 tool needs the complex QMF domain, so LP +
  PS is rejected). The coupling-channel (CCE) tool is
  decoded **and applied** end to end (`cce` + the two-pass stream walk;
  see the tool-chain section above), validated against the
  filterbank-linearity identity on writer-assembled CCE streams; a
  third-party CCE-bearing conformance fixture would still be a welcome
  external cross-check.
- **ER BSAC (AOT 22) PCM** — the noiseless-coder front half
  (headers, layer geometry, arithmetic side-info decode) is
  conformance-pinned, but the deployed encoder's spectral bit-slice
  probability selection diverges from every reading of the printed
  §4.6.4.2.3 / Table 4.A.34 selection (see the BSAC section above),
  so reconstructed PCM does not yet match the reference waveforms.
  Also out of scope until then: SBA-mode segment scheduling
  (`sba_mode == 1`), BSAC LTP, BSAC PNS (the noise-energy PCM
  conventions need a working spectral decode to pin), the
  `zero_code` extended part (channel / SBR / MPEG-Surround
  extensions), and the §4.5.2.6.1 multi-ES `bsac_payload()`
  large-step-layer reassembly (the `er_bs02`-style carriage).
- Error-resilience remainders — the ER story is now wired end to end
  for ER AAC LC (AOT 17), **ER AAC LTP (AOT 19)** and ER AAC LD
  (AOT 23): the ER channel-element body
  (`ics_body::IcsBody::parse_er`) selects all three §4.4.6
  resilience branches, the `reordered_spectral_data()` payload is
  decoded and encoded (`hcr_decode`), and the §4.4.2.3 Table 4.19
  `er_raw_data_block()` driver
  (`StreamDecoder::decode_er_raw_data_block`) walks the fixed
  per-`channelConfiguration` element sequence for all three AOTs —
  reachable from LATM (the LOAS driver routes AOT-17/19/23 layers
  there with the ASC's resilience triplet and the ASC-resolved
  §4.5.1.1 frame family). AOT 17 is pinned bit-identical to the
  equivalent non-resilient decode of the same spectra
  (`aac-er-hcr-loas`); AOT 19 — the §4.6.7 LTP tool over the
  Table 4.19 walk (11-bit lag, `M = 0`, per-element `x_rec` history
  across frames) — is pinned bit-identical to the equivalent AOT-4
  decode with LTP active (SCE and pair-LTP CPE, plain and HCR
  spectra); AOT 23 is pinned by the staged LD fixtures (see the
  frame-length families section above). **ER AAC scalable (AOT 20)
  now decodes end to end** (see the scalable section above), and the
  §1.8 EP tool — `ErrorProtectionSpecificConfig()`, SRCPC / RS /
  interleaving, the `ep_frame()` codec and the LOAS `EPMuxElement` /
  `EPAudioSyncStream` carrier — is implemented (see the EP section
  above). Still open: the §4.5.2.4 Table 4.148/4.149 per-element
  category *split* of the codec payloads themselves (reassembling an
  er_raw_data_block whose bits arrive as separate
  error-sensitivity-category instances under `epConfig == 1` /
  `directMapping` — the ep_frame class concatenation covers the
  in-order case), and an encoder-produced HCR conformance stream as
  an external cross-check.
- LATM/LOAS transport framing (§1.7) — the `StreamMuxConfig()`,
  `AudioMuxElement()`, `PayloadLengthInfo()`, `PayloadMux()`,
  `LatmGetValue()`, `AudioSyncStream()` and `EPAudioSyncStream()`
  bitstream walkers are now implemented and tested (see the
  LATM / LOAS transport section above), with the `crcCheckSum`
  recomputed against the §1.8.4.5 `CRC8` generator in the `crc`
  module. The runtime `Decoder` LOAS entry point that routes the
  recovered `MuxPayload` raw-data-blocks into a `StreamDecoder` is now
  wired (`latm::LoasDecoder` + the `codec_decoder` carrier
  auto-detection above). The `EPMuxElement()` EP-tool payload
  de-interleave is now implemented (`LoasDecoder::decode_all_ep`; see
  the EP section above). (ADTS `adts_error_check()` CRC validation —
  the 192/128-bit region selection with the double-protection edge
  cases plus the ISO/IEC 11172-3 §2.4.3.1 code — landed in the
  dedicated `adts_crc` module; see the bitstream-parsing section.)

## License

MIT — see [LICENSE](./LICENSE).

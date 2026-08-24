# oxideav-ac3

[![CI](https://github.com/OxideAV/oxideav-ac3/actions/workflows/ci.yml/badge.svg)](https://github.com/OxideAV/oxideav-ac3/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/oxideav-ac3.svg)](https://crates.io/crates/oxideav-ac3) [![docs.rs](https://docs.rs/oxideav-ac3/badge.svg)](https://docs.rs/oxideav-ac3) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Pure-Rust **AC-3 (Dolby Digital)** + **E-AC-3 (Enhanced AC-3 / Dolby
Digital Plus)** audio decoder + encoder — elementary streams per
ATSC A/52:2018 (= ETSI TS 102 366). Zero C dependencies.

Part of the [oxideav](https://github.com/OxideAV/oxideav-workspace)
framework but usable standalone.

## Architecture

The pipeline follows the spec's natural ordering; each module owns one
slice of §5..§7 (base AC-3) or §E (E-AC-3):

1. [`syncinfo`] — sync word 0x0B77, crc1, fscod, frmsizecod,
   frame-length lookup (§5.3.1 / §5.4.1 / Table 5.18).
2. [`bsi`] — Bit Stream Information: bsid, bsmod, acmod → channel layout
   + lfeon + dialnorm + the optional timecode / Annex D alternate-syntax
   metadata blocks (§5.4.2).
3. [`audblk`] — per-block exponent decode (§7.1), parametric bit
   allocation (§7.2 with §7.2.2.6 delta-bit-allocation), mantissa decode
   (§7.3), channel coupling (§7.4), rematrixing (§7.5), dynamic-range
   compression (§7.7).
4. [`imdct`] + [`mdct`] — §7.9.4 FFT-backed 512-point IMDCT and
   256-point short-block pair, plus the forward transforms the encoder
   uses.
5. [`downmix`] — §7.8 LoRo + §7.8.2 LtRt downmix matrices for every
   source acmod, with Annex D / E-AC-3 mix-level extension routing.
6. [`wave_order`] — channel reorder for front-centre-bearing layouts
   (`acmod ∈ {3, 5, 7}`).
7. [`encoder`] — base AC-3 encoder.
8. [`eac3`] — Annex E decoder + encoder.
9. [`crc`] — §7.10.1 CRC-16 over poly 0x8005, shared between the encoder
   and the opt-in `decoder::verify_packet_crc` residue check.
10. [`drc`] — §6.1.9 / §7.6 / §7.7 dynamic-range-control + dialogue-
    normalisation control surface (`DrcSettings`: partial-compression
    cut/boost, heavy-compression "RF mode", dialnorm playback target).

## Capabilities

### AC-3 decoder

- Sync frame + BSI parse (§5.3 / §5.4). All §5.4.2 metadata words —
  bit-stream mode, compression gain, dialogue normalisation, mix
  levels, Dolby Surround mode, timecodes, copyright/original flags,
  language code, audio-production info, the Annex D alternate-syntax
  informational blocks, and the `addbsi` trailer — are parsed and
  surfaced as typed accessors (advisory metadata; the PCM path is
  unchanged).
- Audio-block parse (§5.4.3), exponent decode (§7.1) + parametric bit
  allocation (§7.2), mantissa decode (§7.3) with bap=0 dither (§7.3.4),
  delta bit allocation (§7.2.2.6).
- IMDCT synthesis (§7.9) — both 512-point long-block and 256-point
  short-block paths.
- Channel coupling (§7.4) + rematrix (§7.5) + dynrng (§7.7).
- **Dynamic-range-control + dialogue-normalisation control surface**
  (§6.1.9 / §7.6 / §7.7, [`drc`]). The mandatory §7.7.1 full-`dynrng`
  decode is the default ("line out"); a listener-facing
  [`DrcSettings`] steers the §7.7.1.2 *partial-compression* cut/boost
  factors (apply a fraction of each gain reduction / increase, with
  independent directions), §7.7.2 *heavy compression* ("RF mode" —
  substitutes the BSI `compr` word, ±48 dB, falling back to `dynrng`
  when a frame carries no `compr` per §7.7.2.1), and §7.6 dialogue
  normalisation (an opt-in playback scalar `10^((target − dialnorm)/20)`
  toward a chosen headroom target). Build a configured decoder with
  `decoder::make_decoder_with_drc(params, DrcSettings)`. Applies to both
  the AC-3 and E-AC-3 paths.
- Downmix (§7.8) — LoRo and LtRt 2-channel.
- Bitstream → WAV channel reorder for multichannel layouts.

### AC-3 encoder

- Multichannel encode — 1/0, 2/0, 2/0+LFE (2.1), 3/0, 2/2, 3/2, 3/2.1
  (5.1) and other acmod layouts, with per-channel D15/D25/D45 exponent
  strategy selection (§7.1.3), 5-fbw channel coupling within the
  §5.4.3.12 narrow-coupling validity envelope, a §8.2.2 transient
  detector (4th-order Butterworth 8 kHz split for short-block
  switching), per-channel `fsnroffst[ch]` tuning (§5.4.3.40), per-block
  SNR-offset bit-pool redistribution, and §7.10.1 dual-CRC emission.
- **Bitstream-metadata surface** (`encoder::MetadataParams` /
  `make_encoder_with_metadata`, or the registry options `dialnorm`,
  `compr`, `dynrng`, `bsmod`, `cmixlev`, `surmixlev`, `dsurmod`,
  `langcod`, `mixlevel`+`roomtyp`, `copyright`, `origbs`): every
  §5.4.2 BSI advisory word plus the §5.4.3.3-4 per-block `dynrng`
  dynamic-range word. Round-tripped through the typed `bsi::parse`
  surface and black-box validated: the external decoder binary
  reproduces the authored `dynrng` / `compr` gains exactly
  (Δ0.000 dB at −12 dB words) and a 10 dB `dialnorm` delta measures
  −10.03 dB under target-level normalisation.

### E-AC-3 (Annex E)

- Decoder — BSI, audfrm (Tables E1.2 / E1.3), audblk DSP, the §3.4
  Adaptive Hybrid Transform on fbw / LFE / coupling channels, §3.6
  spectral extension with the §3.6.4.2.3 SPXATTEN border notch, and
  §3.7.2 transient pre-noise processing. All three §2.3.2.3 SNR-offset
  strategies decode: the frame-level `snroffststr == 0` pair plus the
  §2.3.3.27 per-block modes `0x1` (one shared `blkfsnroffst`) and `0x2`
  (independent per-coupling/channel/LFE fine offsets), each gated by the
  per-block `snroffste` reuse flag. Enhanced coupling
  (`ecplinu == 1`, §E.2.3.3.16-26 / §E.3.5.5) decodes end-to-end: the
  audblk parser reads the strategy + per-channel amplitude/angle/chaos
  coordinates, decodes the enhanced-coupling channel through the shared
  exponent / bit-allocation / mantissa path, and a deferred second pass
  reconstructs the non-aliased complex carrier `Z[k]` from the
  previous / current / next blocks (§E.3.5.5.1), processes the per-bin
  amplitudes + de-correlated angles, and emits each coupled channel's
  transform coefficients via the §E.3.5.5.4 complex product — replacing
  the standard §7.4 decouple. Block 0's "previous block" carrier source
  is threaded across the frame boundary from the prior frame's last
  enhanced-coupling block (carried on `EcplState`, §E.3.5.5.1), so the
  prior-frame edge no longer collapses to a zero carrier; the frame's
  last block's "next block" still uses a zero carrier (it lives in a
  not-yet-decoded frame — streaming lookahead is out of scope). Three
  enhanced-coupling conformance defects fixed in r406, pinned by the
  new encoder round-trips: `chincpl[ch]` is read directly after
  `ecplinu` (BEFORE the standard/enhanced strategy split — the prior
  order desynced every multichannel ecpl frame, invisibly in 2/0 where
  the flags are implicit); `ecplparam1e/2e == 0` now REUSE the
  previously transmitted amplitudes / angle+chaos values per
  §2.3.3.21-22 (previously each block's coordinate set was replaced
  wholesale, silencing every band of a reusing channel); and the
  resolved banding structure masks its entries up to and including
  `max(ecpl_begin_subbnd, 8)` per §E.2.3.3.19 — the Table E2.14
  default carries a merge bit at sub-band 9, so a default-banded
  region beginning there previously made `necplbnd` disagree with the
  §E.3.5.5.1 band walk by one band (a coordinate-count desync). The
  §E.3.3.2 `nrematbd` derivation now folds in enhanced coupling: a 2/0
  `ecplinu` block sizes its rematrix-flag field from the raw `ecplbegf`
  code (0/1/2/<5 → 0/1/2/3 bands, else 4) rather than `cplbegf`, keeping
  the bit cursor aligned on enhanced-coupling 2/0 frames. Standard coupling
  now applies the §E.2.3.3.15 **default coupling banding structure**
  (`defcplbndstrc[]`, Table E2.12, indexed by absolute sub-band) when
  `cplbndstrce == 0` in a frame's first coupling block, instead of leaving
  every sub-band un-merged — the prior all-zeros behaviour collapsed a
  7-subband region to 7 bands instead of 3, corrupting the §7.4
  coupling-coordinate scatter on every basic stereo-coupled frame. Three
  corpus stereo fixtures (`eac3-stereo-48000-192kbps`, `eac3-256-coeff-block`,
  `eac3-from-ac3-bitstream-recombination`) jumped from ~8-14 dB to ~91 dB
  PSNR and are now CI-gated at an 80 dB floor (`Tier::MinPsnr` in
  `tests/docs_corpus.rs`). Dependent-substream channel combination follows
  the §E.3.8.2 replace-or-extend rule: each dep coded channel is routed by
  its Table E2.5 location (or natural `acmod` order when `chanmape == 0`),
  *replacing* the matching independent-substream channel in place when the
  location is shared (e.g. a dep substream re-coding Center / LFE, or L/R
  via a custom `chanmap`) and *extending* the output only for genuinely new
  locations — so a real greater-than-5.1 broadcast program reassembles
  spatially correctly rather than duplicating and decorrelating the shared
  channels a blind append would have appended.
- Encoder — independent + dependent substream pairs for 1.0 / 2.0 / 5.1
  / 7.1 layouts, with adaptive / frame-based exponent strategies.
  **Bitstream-metadata surface** (`eac3::Eac3Metadata` /
  `eac3::make_encoder_with_metadata` + registry options): fixed-BSI
  `dialnorm` and `compr`, the per-block `dynrng` word (every block of
  every substream), the Table E1.2 **mixing-metadata block**
  (`dmixmod`, LtRt/LoRo centre + surround mix levels, `lfemixlevcod`,
  `pgmscl`, `extpgmscl`) and the §E.2.3.1.62+ **informational block**
  (`bsmod`, copyright/original, 2/0 `dsurmod`+`dheadphonmod`,
  ≥6-channel `dsurexmod`, audio-production info, `sourcefscod`) —
  emitted on the independent substream (a 7.1 pair's dependent
  substream keeps the blocks absent while sharing dialnorm/compr).
  Round-tripped through the typed Annex E `bsi::parse` surface (which
  now also surfaces `bsmod`); black-box validated: the external
  decoder binary accepts the block-bearing syntax (decode level
  within 0.004 dB of a block-less encode) and reproduces the authored
  `dynrng` gain exactly. Metadata-bearing AC-3 and E-AC-3 streams are
  swept through the corruption families in `tests/robustness.rs`.
  **Spectral extension is now available on the encoder side**
  (`eac3::make_encoder_with_spx(params, SpxParams)`, §E.2.3.3 / §E.3.6):
  every fbw channel is coded only up to the SPX begin frequency
  (default tc# 109 ≈ 10.2 kHz at 48 kHz) and the decoder regenerates
  the extension region (default up to tc# 229 ≈ 21.5 kHz) from a
  translated copy of the channel's own low band, noise-blended and
  scaled by per-band coordinates the encoder derives from the
  §3.6.4.3 energy-matching rule (`spxco = rms(original HF band) /
  (rms(translated band)·32)`, quantised through the §E.2.3.3.11-13
  exponent/mantissa/master-coordinate forms). Coordinates refresh on
  the exponent anchor blocks and are reused between (`spxcoe`);
  geometry (begin/end/copy-start codes, noise blend, default
  Table E2.11 vs explicit band structure) is configurable via
  `SpxParams`; the freed high-frequency bits are re-spent on the coded
  low band by the SNR-offset tuner. Optional extras: §3.6.4.2.3
  **attenuation** (`spxattene`/`chinspxatten`/`spxattencod` in audfrm,
  with the border/wrap notch folded into the encoder's coordinate
  computation so band energies still match) and **adaptive copy-start**
  (per-frame `spxstrtf` re-selection that scores every candidate by
  coordinate saturation — a spectrum with a hole above the first copy
  sub-band would otherwise pin a band's coordinate at the 0.875
  ceiling). Validated three ways: in-tree round-trips gate per-SPX-band
  decoded energy within ±3 dB of the original (mono / stereo / 5.1 /
  narrow non-default geometry), default-vs-explicit band structure
  decodes bit-identically, and an external decoder binary
  cross-validates the emitted syntax + banded energies (±4 dB) for the
  plain, mono, and attenuated variants. SPX-encoded streams are also
  swept through the truncation / bit-flip / garbage corruption
  families. Both entry points build the same encoder: the typed
  `make_encoder_with_spx` and the registry path via
  `CodecParameters::options` `spx*` keys (`spx`, `spx_begf`/`endf`/
  `strtf`/`blnd`, `spx_atten`, `spx_adaptive_copy_start`,
  `spx_explicit_band_structure`) — pinned byte-identical. Stationary
  coordinate refreshes are thrifted (`spxcoe = 0`) with a mid-frame
  level-step gate proving per-span refresh still tracks moving spectra.
  **Mixed per-channel membership** (§E.2.3.3.3) is supported via
  `SpxParams::channel_mask` / the `spx_chmask` option: excluded
  channels emit `chinspx[ch] = 0`, keep their `chbwcod`, and are
  waveform-coded to full bandwidth while member channels stop at the
  SPX begin frequency — the SNR tuner budgets every channel at its own
  coded bandwidth (per-channel `end_mant` plumbed through
  `tune_snroffst_with_plan_ends` / `overhead_bits_for_ends` /
  `mantissa_bits_total_ends`). The mixed split is validated in-tree
  (SPX band-energy contract on the member channel AND full-bandwidth
  HF fidelity on the excluded one) and through the external decoder.

  **The Adaptive Hybrid Transform is now on the encoder side too**
  (`eac3::make_encoder_with_aht(params)` / the `aht` option, §3.4):
  every fbw channel — and the LFE (`lfeahtinu`) — moves to a single
  block-0 exponent anchor (`nchregs[ch] == nlferegs == 1`, per-bin
  6-block-max exponents), long transforms are forced, and block 0
  front-loads the §3.4.4 mantissa stream: `chgaqmod` + gain words +
  per-bin codewords against the §3.4.3.1 `hebap[]` (the shared
  psd/excitation/mask pipeline with the Table E3.1 pointer table).
  Quantiser stack per Tables E3.2/E3.5/E3.6: minimum-Euclidean-
  distance VQ over Tables E4.1..E4.7 for `hebap` 1..7, and
  gain-adaptive quantisation for `hebap` >= 8 — the per-channel
  `gaqmod` (all four Table E3.3 modes, incl. the 5-bit composite gain
  triplets) is chosen by exact bit accounting, with per-bin Gk in
  {1, 2, 4} splitting short small-mantissa codewords from
  tag + dead-zone large escapes. The SNR-offset tuner costs AHT
  channels by their exact front-loaded payload and binary-searches
  the monotone `csnroffst·16 + fsnroffst` axis. Measured on a
  stationary stereo two-tone (in-tree decode): 42.0 dB @ 96 kbps
  rising to 75.6 dB @ 448 kbps versus a flat ~23.9 dB for the
  standard path (+18 to +52 dB); on a multitone+noise bed fixture
  +7 to +22 dB. Black-box: mono / stereo / 5.1 AHT streams decode
  through an external decoder binary at 28.1 / 28.1 / 33.4 dB
  (vs 22.3 dB non-AHT baseline through the same harness).
  `examples/eac3_rate_curves.rs` prints the full
  standard/AHT/SPX/enhanced-coupling rate ladder;
  `aht_quality_scales_with_rate` gates the curve shape in CI.

  **Enhanced coupling is now on the encoder side too**
  (`eac3::make_encoder_with_ecpl(params, EcplParams)` / the `ecpl`,
  `ecpl_begf`, `ecpl_endf` options; §E.2.3.3.16-26 / §E.3.5.5) — the
  last Annex E encoder tool. Every fbw channel of the independent
  substream is coupled: below the begin frequency (default tc# 37 ≈
  3.5 kHz) channels are waveform-coded as usual; above it a single
  shared **carrier** channel is coded through the standard coupling-
  channel exponent / bit-allocation / mantissa path (cplexpstr anchors
  on blocks 0/3, `cplabsexp` + D15 groups, implicit first-block
  `cplleake` with zero leak inits, mantissas interleaved after the
  first coupled channel) and each coupled channel is rebuilt from it
  via per-band Table E3.10 amplitude + Table E3.11 angle coordinates
  (chaos 0, `ecpltrans` 0 — deterministic decode). The carrier is the
  first coupled channel's MDCT scaled per band ~3 dB above the loudest
  coupled channel — phase-locked to channel 0, whose angle is
  spec-fixed to 0 and never transmitted, with the margin letting the
  1.0 amplitude ceiling absorb band-level carrier coding loss.
  Coordinates are measured in the §E.3.5.5.1 complex analysis domain
  against the carrier the decoder will actually reconstruct (each bin
  through the final exponent + bap quantiser; the previous frame's
  carried quantised last block at the frame head, zero after the
  tail), refreshed on blocks 0/3 with §2.3.3.21-22 reuse thrift when
  the block-3 refresh quantises identically. **Chaos coordinates**
  (Table E3.12, on by default via `EcplParams::chaos` / the
  `ecpl_chaos` option) are derived from the measured per-band
  coherence — the incoherent fraction maps onto the 8-step grid,
  engaging the decoder's §E.3.5.5.3 per-bin random de-correlation for
  content the shared carrier cannot represent, with the transmitted
  amplitude pre-divided by the decoder's `1 + 0.38·chaosval`
  modification (chaos backs off when the pre-compensated amplitude
  would exceed the 1.0 ceiling — band energy wins over width). A
  partial-coherence stereo fixture gates the effect: decoded
  in-region inter-channel coherence 0.914 chaos-less → 0.796 with
  chaos (source 0.706), band energies still matched. Validated in-tree:
  stereo band-energy round-trip (±3 dB per signal band, ±1.5 dB coded
  low band, 20 dB waveform floor), a stereo **quadrature** fixture
  (channel 1's tones 90° off the carrier — pins the angle path at an
  18 dB waveform floor; a broken angle path collapses to ~3 dB),
  5.1 with explicit `chincpl` bits, a 7.1 indep(coupled)+dep(plain)
  pair walk, registry-vs-typed byte-identical construction, and the
  corruption families in `tests/robustness.rs`. Measured interior
  PSNR 25.4-30.4 dB across channels. Black-box cross-validation is
  **not possible for this tool**: the external validator binary
  reports enhanced coupling as not implemented and mutes (probed
  r406) — our decoder is ahead of the validator here, so validation
  is round-trip + spec-text only. **SPX and enhanced coupling can be
  co-active** (`make_encoder_with_spx_ecpl` / options `spx=1` +
  `ecpl=1`) per §3.6.1: channels are waveform-coded below the
  coupling begin, carried by the shared carrier + coordinates from
  there to the SPX begin frequency (the coupling region is
  SPX-bounded — `ecplendf` is not transmitted, §E.2.3.3.17), and
  SPX-synthesized above it; a stereo three-region round-trip gates
  all three regions' energies (coupling bands ±3 dB, SPX bands
  ±3.5 dB, coded low band ±1.5 dB). AHT remains mutually exclusive
  with both.

  Three spec-fidelity notes from this work: (1) GAQ dequantisation now
  uses the literal Table E3.5/E3.6 characteristics — the `Gk = 2`
  large mantissa is an `(m-1)`-bit codeword (2^(m-1) output points),
  and all scalar quantisers apply the exact Q15 `y = x + ax + b`
  remap. (2) The §3.4.5 IDCT's printed leading constant `2` measures
  as `√2` against an independent production decoder (a pure-DC and a
  40 Hz-modulated fixture both fit `external = ours(2·Σ)/√2` with
  ~89 dB residual); with `√2` the DC basis weight is exactly 1, and
  both transforms follow the deployed constant. (3) §E.3.5.5.1's
  step-3 overlap-add omits the §7.9.4.1 step-6 headroom-restoring
  factor of 2 (step 2 references only "steps 1 to 5") — taken
  literally the analysis→synthesis chain returns exactly half the
  original coefficients, so every enhanced-coupling channel would
  decode 6 dB low and the loudest coupled channel would need the
  unrepresentable amplitude 2.0; the Table E3.10 ceiling of exactly
  1.0 pins the intended identity at unity, and the factor of 2 is
  applied in the carrier reconstruction. All three notes are codified
  as clean-room errata entries (`docs/audio/ac3/ac3-errata.md` E2 /
  E1 / E3 respectively; E3 also records that the ETSI TS 102 366
  V1.4.1 copy omits the enhanced-coupling channel-processing clause
  entirely, so A/52:2018 is the operative text). The E3 correction is
  regression-pinned in both directions: a decode-side least-squares
  identity fit (corrected chain gain 1.0 vs exactly 0.5 as printed)
  and a full encode→decode bitstream round-trip gating the aggregate
  coupling-region energy within ±1.5 dB of unity (the as-printed
  reading sits at −6.02 dB). A real Dolby-encoded `ecplinu = 1`
  stream remains a recorded fixture GAP
  (`docs/audio/ac3/fixtures/eac3-ecpl-enhanced-coupling/GAP.md`), so
  ecpl validation stays in-tree round-trip + spec-text.

### CRC

§7.10.1 CRC-16 (poly 0x8005), shared between the encoder (forward
generation, augmented form for crc2) and the opt-in decoder residue
check.

## Conformance corpus

`tests/docs_corpus.rs` decodes the AC-3 / E-AC-3 fixture set under
`docs/audio/ac3/fixtures/` (each a raw elementary stream paired with a
reference PCM decode) and scores per-channel PSNR. The decode is
floating-point in the IMDCT, so it is not bit-exact against the
reference, but it is **deterministic** (identical PSNR run-to-run).

Fixtures whose decode is known-good are gated at a `Tier::MinPsnr`
floor so a regression fails CI; the rest log deltas without gating:

| Tier | AC-3 | E-AC-3 |
| --- | --- | --- |
| `MinPsnr` (CI-gated) | 11 fixtures — mono / stereo / 2/1 / 3/0 / 3/2 (±LFE) at 48 / 44.1 / 32 kHz, 32-448 kbps, ~86-92 dB (80 dB floor, 96 kbps mono at 78), plus the torture-grade `ac3-low-bitrate-32kbps-mono` (~62 dB — the 32 kbps lossy / onset-overlap floor — at a loose 50 dB gross-regression floor) | 6 fixtures — stereo + 5.1 + 256-coeff at ~91 dB (80 dB floor), plus the low-rate `eac3-low-bitrate-32kbps` (~66 dB, 60 dB floor) and `eac3-low-rate-stereo-64kbps` (~72 dB, 65 dB floor) as gross-regression guards |

Every corpus fixture is now CI-gated; none remain `ReportOnly`. The
decoder is additionally fuzzed for panic-safety against
truncation / bit-flip / sync-prefixed-garbage corruption of every
fixture (`tests/robustness.rs`).

## Installation

```toml
[dependencies]
oxideav-core = "0.1"
oxideav-codec = "0.1"
oxideav-ac3 = "0.0"
```

## Codec ID

- Codecs: `"ac3"` (decoder + encoder) and `"eac3"` (decoder + encoder);
  output sample format `S16` interleaved.

## License

MIT — see [LICENSE](LICENSE).

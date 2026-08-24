//! Enhanced AC-3 (E-AC-3 / Dolby Digital Plus) — ATSC A/52 Annex E.
//!
//! E-AC-3 is **not** backwards-compatible with AC-3 at the bit-stream
//! level: the syncinfo loses crc1, the bsi grows new fields
//! (`strmtyp`, `substreamid`, `frmsiz`, `numblkscod`), the audio frame
//! gains a new `audfrm()` element with frame-level strategy flags, and
//! every audio block carries SPX, AHT, and enhanced-coupling fields
//! that don't exist in the base spec. The bsid value (16, or 11..15
//! for backward-compatible variants) selects the syntax — base-AC-3
//! decoders MUST mute on bsid > 10 per A/52 §E.2.3.1.6.
//!
//! ## Module layout
//!
//! * **[`bsi`]** — Table E1.2 parser: stream type, substream id,
//!   frame size, sample-rate code (incl. fscod2 reduced rates),
//!   number of blocks, channel layout, dialnorm, compression,
//!   `chanmape`/`chanmap` for dependent substreams, plus the full
//!   `mixmdate`/`infomdate`/`addbsi` opt-in chain.
//! * **[`audfrm`]** — Table E1.3 parser: the 11 strategy flags,
//!   frame-level exponent strategies (`frmcplexpstr`,
//!   `frmchexpstr`, `lfeexpstr` runs), AHT in-use flags, frame-level
//!   SNR offsets, transient pre-noise + spectral-extension attenuation
//!   parameters, per-block start info. Two-phase: [`audfrm::parse_with`]
//!   stops at the AHT anchor when `ahte == 1`; once the dsp pre-walk has
//!   produced `nchregs[ch]` / `ncplregs` / `nlferegs` from the per-block
//!   exponent strategies, [`audfrm::parse_phase_b`] consumes the
//!   variable-width `chahtinu` / `cplahtinu` / `lfeahtinu` bits.
//! * **[`aht`]** — Adaptive Hybrid Transform decode (§3.4). VQ
//!   codebooks E4.1..E4.7 (956 × 6 i16) + `hebap` pointer table
//!   (E3.1) + quantiser-bit table (E3.2) + the literal Table E3.6
//!   Q15 remap constants. [`aht::vq_lookup`] /
//!   [`aht::read_scalar_aht_mantissas`] plus the §3.4.5 inverse
//!   DCT-II ([`aht::idct_ii_6`] — leading constant √2 per black-box
//!   validation; the printed `2` is an erratum).
//! * **[`ahtenc`]** — encoder-side AHT (§3.4, encode direction):
//!   forward DCT-II, §3.4.4.1 minimum-distance VQ search, the
//!   Tables E3.5/E3.6 scalar+GAQ quantiser (exact inverse of the
//!   decoder's remap), per-channel `gaqmod` planning by exact bit
//!   accounting, the §3.4.4 emission order, and the §3.4.3.1
//!   `hebap[]` derivation through the shared bit-allocation core.
//! * **[`ecpl`]** — enhanced-coupling sub-band / band geometry
//!   (§E.2.3.3.16-19 + §E.3.5.2): the Table E3.8 begin/end sub-band
//!   derivations, Table E3.9 `ecplsubbndtab[]`, Table E2.14 default
//!   banding, the §E.2.3.3.19 `necplbnd` band count, and the
//!   §E.3.5.5.1 per-band bin counts; the §E.2.3.3.16-26 bitstream-syntax
//!   parse; the §E.3.5.5.2 / §E.3.5.5.3 parameter-processing layer
//!   (Table E3.10-E3.12 amplitude / angle / chaos decode, the chaos
//!   amplitude modification, per-band→per-bin expansion, the
//!   angle-interpolation path); and the full §E.3.5.5.1 carrier
//!   reconstruction + §E.3.5.5.4 complex synthesis
//!   ([`ecpl::synthesize_block`] / [`ecpl::EcplState`]).
//! * **[`ecplenc`]** — encoder-side enhanced coupling (§E.2.3.3.16-26 /
//!   §E.3.5.5, encode direction): Table E3.10 / E3.11 inverse
//!   quantisers, §E.3.5.5.1 band cross-spectrum statistics, and the
//!   first-coupled-channel-phase-locked carrier construction with
//!   per-band gains.
//! * **[`dsp`]** — per-frame DSP: §7.4 decouple, AHT mantissa cache,
//!   §3.6 spectral extension (translate → noise-blend → coordinate
//!   scale + §3.6.4.2.3 SPXATTEN border notch), §3.7.2 transient
//!   pre-noise processing (PCM-domain time-scaling synthesis).
//! * **[`decoder`]** — top-level per-substream decode. Routes packets
//!   with `bsid ∈ {11..=16}` through BSI → audfrm phase-A → dsp
//!   pre-walk → audfrm phase-B → audblk DSP → IMDCT → overlap-add →
//!   §7.8 downmix.
//! * **[`encoder`]** — Annex E encoder. Indep substream for
//!   1.0 / 2.0 / 5.1 layouts (acmod ∈ {1, 2, 7} with `lfeon=1` for
//!   5.1); 7.1 input emits an indep+dep substream pair (indep
//!   carries the 5.1 program, dep 0 carries Lb/Rb back surrounds
//!   with chanmap bit 6 set per §E.2.3.1.7-8 / §E.3.8.2). Spectral
//!   extension is available opt-in via [`encoder::make_encoder_with_spx`]
//!   (incl. mixed per-channel `chinspx` via
//!   `SpxParams::channel_mask`), the Adaptive Hybrid Transform
//!   via [`encoder::make_encoder_with_aht`] (fbw + LFE channels,
//!   §3.4 — mutually exclusive with the others), enhanced coupling
//!   via [`encoder::make_encoder_with_ecpl`], and the §3.6.1
//!   mid-range-coupling + high-range-SPX combination via
//!   [`encoder::make_encoder_with_spx_ecpl`].
//! * **[`spxenc`]** — encoder-side spectral extension (§E.2.3.3 /
//!   §E.3.6): geometry derivation + validation, the §3.6.4.3
//!   energy-matching coordinate targets (through the decoder-shared
//!   translation plan, with the §3.6.4.2.3 attenuation notch folded
//!   in when signalled), and the §E.2.3.3.11-13 exponent / mantissa /
//!   master-coordinate quantiser.
//!
//! ## Known decoder gaps
//!
//! * **Enhanced coupling** (`ecplinu == 1`, §E.1.3.3.7-26 /
//!   §E.2.3.3.16-26 / §E.3.5.5) decodes end-to-end. The audblk parser
//!   reads the strategy + per-channel amplitude/angle/chaos coordinates
//!   and decodes the enhanced-coupling channel through the shared
//!   exponent / bit-allocation / mantissa path; a deferred second pass
//!   (see [`dsp`]) reconstructs the §E.3.5.5.1 complex carrier `Z[k]`
//!   from the previous / current / next blocks, processes the per-bin
//!   amplitudes + de-correlated angles, and emits each coupled channel's
//!   transform coefficients via the §E.3.5.5.4 complex product. The
//!   per-step primitives + the [`ecpl::synthesize_block`] orchestration
//!   are spec-derived and unit-tested in [`ecpl`]. Block 0's "previous
//!   block" carrier source is now threaded from the prior frame's last
//!   enhanced-coupling block (carried on [`ecpl::EcplState`], §E.3.5.5.1);
//!   the prior-frame edge no longer collapses to a zero carrier. The
//!   frame's last block's "next block" still uses a zero carrier (it lives
//!   in a not-yet-decoded frame — streaming lookahead is out of scope).
//!   Standard coupling is fully in.
//! * **Cross-frame transient pre-noise reference** (§E.3.7.1) is
//!   clamped to the current frame; intra-frame transients (§E.3.7.2)
//!   are fully synthesised.
//! * **Standard-coupling default banding** (§E.2.3.3.15 Table E2.12):
//!   when `cplbndstrce == 0` in the first coupling block of a frame the
//!   decoder now applies the `defcplbndstrc[]` default structure
//!   (indexed by absolute sub-band number) instead of leaving every
//!   sub-band un-merged. This was the root cause of the three
//!   previously floor-bound stereo fixtures; they now decode at
//!   ~91 dB PSNR (gated `MinPsnr` floors in `tests/docs_corpus.rs`).
//! * Corpus status: every multichannel / stereo E-AC-3 fixture in the
//!   `tests/docs_corpus.rs` set now decodes at ~88-92 dB PSNR and is
//!   CI-gated at a `MinPsnr(80.0)` floor — including
//!   `eac3-5.1-side-768kbps` (~91.7 dB, promoted from the earlier
//!   side-channel-glitch floor in round 365). The only remaining
//!   `ReportOnly` E-AC-3 fixtures are the deliberately torture-grade
//!   low-rate cases (`eac3-low-bitrate-32kbps` ~66 dB,
//!   `eac3-low-rate-stereo-64kbps` ~72 dB), whose error is confined to
//!   the signal attack/release blocks (the steady-state interior
//!   decodes at ~85 dB+) and reflects the lossy floor at those budgets
//!   rather than a decode defect (see crate `README.md`).

// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod aht;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod ahtenc;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod audfrm;
pub mod bsi;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod chanmap;
pub mod decoder;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod dsp;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod ecpl;
pub mod ecplenc;
pub mod encoder;
pub mod spxenc;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod tables;

// Re-exports — keep the public surface identical to the old single-
// file `eac3.rs` so external callers (the encoder integration test in
// `tests/eac3_ffmpeg.rs` and the workspace registration in
// `crate::lib::register`) don't need to change.
pub use bsi::{
    Bsi as Eac3Bsi, DrcSource, PanInfo, PremixCompression, PremixCompressionWord,
    ProgramScaleFactor, BSID_BASE_AC3_MAX, EAC3_BSID,
};
pub use decoder::{decode_eac3_packet, Eac3DecoderState};
pub use ecplenc::EcplParams;
pub use encoder::{
    make_encoder, make_encoder_with_aht, make_encoder_with_ecpl, make_encoder_with_spx,
    make_encoder_with_spx_ecpl, CODEC_ID_STR,
};
pub use spxenc::SpxParams;

//! Pure-Rust **AC-3 (Dolby Digital)** + **E-AC-3 (Enhanced AC-3,
//! a.k.a. Dolby Digital Plus)** audio decoder + encoder.
//!
//! Implements ATSC A/52:2018 (= ETSI TS 102 366) elementary streams:
//! base AC-3 (`bsid ≤ 10`) and Annex E (`bsid ∈ {11..=16}`).
//!
//! # Architecture
//!
//! The pipeline follows the spec's natural ordering. Each stage is a
//! module that owns one slice of §5..§7 (base AC-3) or §E (E-AC-3):
//!
//! 1. [`syncinfo`] — 16-bit sync word 0x0B77, crc1, fscod, frmsizecod,
//!    frame-length table lookup (§5.3.1 / §5.4.1 / Table 5.18).
//! 2. [`bsi`] — Bit Stream Information: bsid, bsmod, acmod →
//!    channel-layout + lfeon + dialnorm + optional timecodes /
//!    Annex D §2.3 alternate-syntax mix-level params (§5.4.2).
//! 3. [`audblk`] — per-block exponent decode (§7.1), parametric bit
//!    allocation (§7.2 with §7.2.2.6 delta-bit-allocation), mantissa
//!    decode (§7.3), channel coupling (§7.4), rematrixing (§7.5),
//!    dynamic-range compression (§7.7).
//! 4. [`imdct`] + [`mdct`] — §7.9.4 FFT-backed 512-point IMDCT and
//!    256-point short-block pair plus the forward transforms used by
//!    the encoder; the direct-form references in `audblk` are kept
//!    only as test oracles.
//! 5. [`downmix`] — §7.8 LoRo + §7.8.2 LtRt (Dolby Surround
//!    matrix-encoded) downmix matrices for every source acmod, with
//!    Annex D §2.3 / E-AC-3 mixmdata mix-level extension routing.
//! 6. [`wave_order`] — WAVE_FORMAT_EXTENSIBLE dwChannelMask channel
//!    reorder for front-centre-bearing layouts (`acmod ∈ {3, 5, 7}`).
//! 7. [`encoder`] — full base AC-3 encoder (acmod 1/0..3/2 + LFE,
//!    per-channel D15/D25/D45 exponent strategies, DBA, 5-fbw
//!    coupling, per-channel `fsnroffst[ch]` tuning, per-block
//!    snroffst redistribution, §7.10.1 dual-CRC emission).
//! 8. [`eac3`] — Annex E decoder + encoder. Decoder covers BSI,
//!    audfrm (Tables E1.2 / E1.3), audblk DSP, §3.4 Adaptive Hybrid
//!    Transform on fbw / LFE / coupling channels, §3.6 spectral
//!    extension with §3.6.4.2.3 SPXATTEN border notch, and §3.7.2
//!    transient pre-noise processing. Encoder covers
//!    indep+dep-substream pairs for 1.0 / 2.0 / 5.1 / 7.1 layouts;
//!    SPX and AHT are out of scope on the encoder side.
//! 9. [`crc`] — §7.10.1 CRC-16 over poly 0x8005, shared between
//!    encoder (forward generation, augmented form for crc2) and the
//!    opt-in [`decoder::verify_packet_crc`] residue check.
//!
//! See `README.md` for the round-by-round status checklist and the
//! per-feature dB measurements.

#![allow(clippy::needless_range_loop)]

// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod audblk;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod bsi;
pub mod crc;
pub mod decoder;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod downmix;
pub mod drc;
pub mod eac3;
pub mod encoder;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod imdct;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod mdct;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod syncinfo;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod tables;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod wave_order;

use oxideav_core::{CodecCapabilities, CodecId, CodecParameters, CodecTag, Result};
use oxideav_core::{CodecInfo, CodecRegistry, Decoder, Encoder};

pub const CODEC_ID_STR: &str = "ac3";
pub const CODEC_ID_STR_EAC3: &str = "eac3";

/// Re-export the §6.1.9 / §7.6 / §7.7 dynamic-range-control + dialogue-
/// normalisation control surface at the crate root so callers can build a
/// DRC-configured decoder via [`decoder::make_decoder_with_drc`] without
/// reaching into the [`drc`] submodule.
pub use crate::drc::{DrcMode, DrcSettings};

/// Register the AC-3 + E-AC-3 decoder + encoder with the supplied codec
/// registry.
pub fn register_codecs(reg: &mut CodecRegistry) {
    let cid = CodecId::new(CODEC_ID_STR);
    let dec_caps = CodecCapabilities::audio("ac3_sw_dec")
        .with_lossy(true)
        .with_intra_only(true)
        .with_max_channels(6)
        .with_max_sample_rate(48_000);
    let enc_caps = CodecCapabilities::audio("ac3_sw_enc")
        .with_lossy(true)
        .with_intra_only(true)
        .with_max_channels(6)
        .with_max_sample_rate(48_000);
    // Container tag claims. AC-3 is identified by:
    //   - WAVEFORMATEX::wFormatTag = 0x2000 (AVI / WAV)
    //   - MP4 ObjectTypeIndication 0xA5 (for MP4/ISOBMFF carriage)
    //   - Matroska CodecID "A_AC3"
    reg.register(
        CodecInfo::new(cid.clone())
            .capabilities(dec_caps.clone())
            .decoder(make_decoder)
            .tag(CodecTag::wave_format(0x2000)),
    );
    reg.register(
        CodecInfo::new(cid.clone())
            .capabilities(dec_caps.clone())
            .decoder(make_decoder)
            .tag(CodecTag::mp4_object_type(0xA5)),
    );
    reg.register(
        CodecInfo::new(cid.clone())
            .capabilities(dec_caps)
            .decoder(make_decoder)
            .tag(CodecTag::matroska("A_AC3")),
    );
    // Encoder registration — keyed on codec id only (no container tag
    // so it gets picked up regardless of output muxer).
    reg.register(
        CodecInfo::new(cid)
            .capabilities(enc_caps)
            .encoder(make_encoder),
    );

    // E-AC-3 (Annex E). Accepts mono, stereo, 5.1, and 7.1 input on
    // the encoder side. 7.1 (8 ch) emits an independent substream
    // pair: indep substream carries a 5.1 program (acmod=7, lfeon=1)
    // and dep substream 0 carries Lb/Rb back surrounds with chanmap
    // bit 6 set (Lrs/Rrs pair) per ATSC A/52 Annex E §E.2.3.1.7-8 /
    // §E.3.8.2. Encoder-side SPX and AHT are out of scope (decoder
    // implements both).
    //
    // Decoder side: full Annex E DSP — §3.4 Adaptive Hybrid
    // Transform on fbw / LFE / coupling channels, §3.6 spectral
    // extension with §3.6.4.2.3 SPXATTEN border notch, §3.7.2
    // transient pre-noise processing, and §7.8 LoRo / LtRt downmix
    // including mixmdata mix-level routing.
    let eac3_cid = CodecId::new(CODEC_ID_STR_EAC3);
    let eac3_dec_caps = CodecCapabilities::audio("eac3_sw_dec")
        .with_lossy(true)
        .with_intra_only(true)
        .with_max_channels(8)
        .with_max_sample_rate(48_000);
    let eac3_enc_caps = CodecCapabilities::audio("eac3_sw_enc")
        .with_lossy(true)
        .with_intra_only(true)
        .with_max_channels(8)
        .with_max_sample_rate(48_000);
    // Container tag claims for E-AC-3:
    //   - WAVEFORMATEX::wFormatTag = 0xA7 (DD+ in WAV/AVI)
    //   - MP4 ObjectTypeIndication 0xA6 (ISOBMFF carriage)
    //   - Matroska CodecID "A_EAC3"
    reg.register(
        CodecInfo::new(eac3_cid.clone())
            .capabilities(eac3_dec_caps.clone())
            .decoder(make_eac3_decoder)
            .tag(CodecTag::wave_format(0xA7)),
    );
    reg.register(
        CodecInfo::new(eac3_cid.clone())
            .capabilities(eac3_dec_caps.clone())
            .decoder(make_eac3_decoder)
            .tag(CodecTag::mp4_object_type(0xA6)),
    );
    reg.register(
        CodecInfo::new(eac3_cid.clone())
            .capabilities(eac3_dec_caps)
            .decoder(make_eac3_decoder)
            .tag(CodecTag::matroska("A_EAC3")),
    );
    reg.register(
        CodecInfo::new(eac3_cid)
            .capabilities(eac3_enc_caps)
            .encoder(make_eac3_encoder),
    );
}

/// Unified registration entry point — installs AC-3 + E-AC-3 into the
/// codec sub-registry of the supplied [`oxideav_core::RuntimeContext`].
pub fn register(ctx: &mut oxideav_core::RuntimeContext) {
    register_codecs(&mut ctx.codecs);
}

oxideav_core::register!("ac3", register);

fn make_decoder(params: &CodecParameters) -> Result<Box<dyn Decoder>> {
    decoder::make_decoder(params)
}

fn make_encoder(params: &CodecParameters) -> Result<Box<dyn Encoder>> {
    encoder::make_encoder(params)
}

fn make_eac3_encoder(params: &CodecParameters) -> Result<Box<dyn Encoder>> {
    eac3::make_encoder(params)
}

fn make_eac3_decoder(params: &CodecParameters) -> Result<Box<dyn Decoder>> {
    decoder::make_eac3_decoder(params)
}

#[cfg(test)]
mod register_tests {
    use super::*;

    #[test]
    fn register_via_runtime_context_installs_codec_factory() {
        let mut ctx = oxideav_core::RuntimeContext::new();
        register(&mut ctx);
        let ac3 = CodecId::new(CODEC_ID_STR);
        let eac3 = CodecId::new(CODEC_ID_STR_EAC3);
        assert!(
            ctx.codecs.has_decoder(&ac3),
            "AC-3 decoder factory not installed via RuntimeContext"
        );
        assert!(
            ctx.codecs.has_encoder(&ac3),
            "AC-3 encoder factory not installed via RuntimeContext"
        );
        assert!(
            ctx.codecs.has_decoder(&eac3),
            "E-AC-3 decoder factory not installed via RuntimeContext"
        );
        assert!(
            ctx.codecs.has_encoder(&eac3),
            "E-AC-3 encoder factory not installed via RuntimeContext"
        );
    }
}

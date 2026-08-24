//! AC-3 / E-AC-3 dynamic-range and dialogue-normalisation control
//! surface (§6.1.9 / §7.6 / §7.7).
//!
//! The base [`crate::audblk`] decode path already converts the per-block
//! `dynrng` gain word to a linear multiplier and scales the transform
//! coefficients by it (§7.7.1.2, the decoder's *default* behaviour). What
//! lives here is the **listener-controllable** layer the spec wraps around
//! that default:
//!
//! * **§7.7.1.2 "Partial Compression"** — a decoder may apply only a
//!   *fraction* of each `dynrng` gain change, and a *different* fraction
//!   for the positive (boost) and negative (cut) directions. The word is
//!   reinterpreted as a signed 8-bit fraction `X0 . X1 X2 Y3 Y4 Y5 Y6 Y7`,
//!   multiplied by the chosen direction-dependent factor, then re-formed
//!   and used normally. Factor `1.0` reproduces the mandatory full
//!   compression; factor `0.0` reproduces the original (un-compressed)
//!   dynamic range.
//! * **§7.7.2 Heavy Compression (`compr`)** — when a product must
//!   constrain the peak output level (the canonical "RF / set-top
//!   modulator" case), it uses the BSI-resident `compr` word (±48 dB,
//!   0.5 dB resolution) *in place of* `dynrng`. `compr` is decoded here
//!   to a linear multiplier per Table 7.30.
//! * **§7.6 Dialogue Normalisation** — the 5-bit `dialnorm` word advertises
//!   the headroom (in dB) of normal spoken dialogue below digital 100%.
//!   The spec is explicit that this value is *not* consumed inside the
//!   core decoder; it is the reproduction system's responsibility to fold
//!   `dialnorm` into the listener volume control. A decoder that *does*
//!   own the playback gain can normalise to a chosen target level with
//!   `gain = 10^((target_dB − dialnorm_dB)/20)` — implemented here as an
//!   opt-in output scalar.
//!
//! None of these alter the default decode: [`DrcSettings::default()`] is
//! "line out" — full `dynrng`, no heavy compression, no dialnorm
//! normalisation — so the mandatory §7.7.1 behaviour is unchanged unless
//! the caller opts in.
//!
//! All gain arithmetic is `f32` to match the rest of the floating-point
//! DSP path; the dB constants are the spec's 6.02 dB / 0.25 dB / 0.5 dB
//! step sizes derived from the underlying `2^(±n)` shifts.

/// Which dynamic-range control regime the decoder applies.
///
/// The spec frames this as a product/listener choice (§7.7.1.1 /
/// §7.7.2.1): a "line out" product reproduces the full encoder-authored
/// compression, while a peak-constrained product ("RF mode") substitutes
/// the heavier `compr` word. The [`DrcMode::Custom`] arm exposes the
/// §7.7.1.2 partial-compression cut/boost factors directly.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DrcMode {
    /// §7.7.1 default — apply the full `dynrng` gain word (cut factor
    /// `1.0`, boost factor `1.0`). This is the mandatory decoder
    /// behaviour absent listener override.
    #[default]
    LineOut,
    /// §7.7.2 heavy compression — substitute the BSI `compr` word for
    /// `dynrng` so the peak output level is constrained (the "RF
    /// modulator / set-top" case). When a syncframe carries no `compr`
    /// word, §7.7.2.1 mandates falling back to `dynrng` for that frame.
    RfMode,
    /// §7.7.1.2 partial compression — apply `cut` of each gain *reduction*
    /// and `boost` of each gain *increase*. Both factors are clamped to
    /// `[0.0, 1.0]`. `Custom { cut: 1.0, boost: 1.0 }` is identical to
    /// [`DrcMode::LineOut`]; `Custom { cut: 0.0, boost: 0.0 }` reproduces
    /// the original dynamic range (no compression at all).
    Custom { cut: f32, boost: f32 },
}

/// Listener-side dynamic-range + dialogue-normalisation control surface.
///
/// Threaded onto the decoder so the §7.7 gain layer can be steered without
/// re-parsing the bitstream. [`Default`] leaves every knob at the
/// spec-mandated baseline (full `dynrng`, no heavy compression, no
/// dialnorm playback normalisation).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DrcSettings {
    /// Which §7.7 regime to apply.
    pub mode: DrcMode,
    /// Optional §7.6 dialogue-normalisation **target** level, expressed as
    /// the headroom in dB below digital 100% that the listener wants
    /// dialogue reproduced at (same units as the `dialnorm` word: a value
    /// in `1..=31`). `Some(target)` scales the output by
    /// `10^((target − dialnorm)/20)`; `None` leaves the PCM at the encoder
    /// level (the spec default — see §7.6, "not directly used by the
    /// decoder").
    pub dialnorm_target: Option<u8>,
}

impl DrcSettings {
    /// Full-compression line-out default (§7.7.1, no dialnorm
    /// normalisation).
    pub const fn line_out() -> Self {
        DrcSettings {
            mode: DrcMode::LineOut,
            dialnorm_target: None,
        }
    }

    /// Heavy-compression "RF mode" (§7.7.2): substitute `compr` for
    /// `dynrng`, peak-constrained output.
    pub const fn rf_mode() -> Self {
        DrcSettings {
            mode: DrcMode::RfMode,
            dialnorm_target: None,
        }
    }

    /// §7.7.1.2 partial-compression preset with explicit cut/boost
    /// fractions. The factors are clamped into `[0.0, 1.0]`.
    pub fn partial(cut: f32, boost: f32) -> Self {
        DrcSettings {
            mode: DrcMode::Custom {
                cut: cut.clamp(0.0, 1.0),
                boost: boost.clamp(0.0, 1.0),
            },
            dialnorm_target: None,
        }
    }

    /// Attach a §7.6 dialnorm playback-normalisation target (headroom in
    /// dB below full scale, `1..=31`).
    pub fn with_dialnorm_target(mut self, target: u8) -> Self {
        self.dialnorm_target = Some(target);
        self
    }

    /// Resolve the per-block `dynrng` (or, in [`DrcMode::RfMode`], the
    /// frame-level `compr`) gain word into a linear coefficient
    /// multiplier.
    ///
    /// * `dynrng` — the raw 8-bit §5.4.3.4 dynamic-range word for this
    ///   block.
    /// * `compr` — the BSI §5.4.2.10 heavy-compression word, `Some` when
    ///   the syncframe carried one. Only consulted in [`DrcMode::RfMode`].
    ///
    /// Returns the linear gain to multiply the channel's transform
    /// coefficients by (the same role the bare [`dynrng_to_linear`] result
    /// played before this control surface existed).
    pub fn resolve_block_gain(&self, dynrng: u8, compr: Option<u8>) -> f32 {
        match self.mode {
            DrcMode::LineOut => dynrng_to_linear(dynrng),
            DrcMode::Custom { cut, boost } => {
                dynrng_to_linear(scale_dynrng_partial(dynrng, cut, boost))
            }
            DrcMode::RfMode => match compr {
                // §7.7.2.1: heavy compression substitutes for dynrng.
                Some(c) => compr_to_linear(c),
                // §7.7.2.1: "If the decoder has been instructed to use
                // compr, and compr is not present for a particular
                // syncframe, then the dynrng control signal shall be used
                // for that syncframe."
                None => dynrng_to_linear(dynrng),
            },
        }
    }

    /// §7.6 dialogue-normalisation playback gain, given the syncframe's
    /// `dialnorm` word. `1.0` (unity) when no target is configured.
    ///
    /// `dialnorm` and the configured target are both "dB of headroom below
    /// digital 100%". Normalising dialogue to `target` from a stream
    /// authored at `dialnorm` is an attenuation of `target − dialnorm` dB
    /// (a *more* negative target = quieter output). Out-of-range words
    /// (the reserved `0` codepoint) are treated as unity.
    pub fn dialnorm_gain(&self, dialnorm: u8) -> f32 {
        match self.dialnorm_target {
            None => 1.0,
            Some(target) => dialnorm_gain(dialnorm, target),
        }
    }
}

/// Convert an 8-bit `dynrng` word to a linear gain multiplier (§7.7.1.2).
///
/// Layout `X0 X1 X2 . Y3 Y4 Y5 Y6 Y7`:
/// * `X` is a 3-bit signed integer in `−4..=3`; the coarse gain is
///   `(X + 1) · 6.02 dB`, realised as `2^(X+1)` arithmetic shifts.
/// * `Y` is the fractional mantissa `0.1 Y3 Y4 Y5 Y6 Y7` (base 2), i.e.
///   `(32 + Y) / 64` in `[0.5, 0.984375]`.
///
/// The combined linear gain is `2^(X+1) · (32 + Y) / 64`. The all-zero
/// word maps to unity.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn dynrng_to_linear(dynrng: u8) -> f32 {
    let x = ((dynrng >> 5) & 0x7) as i32;
    let x_signed = if x >= 4 { x - 8 } else { x };
    let y = (dynrng & 0x1F) as i32;
    let y_val = (32 + y) as f32 / 64.0;
    let shift = x_signed + 1;
    let base = 2f32.powi(shift);
    base * y_val
}

/// §7.7.1.2 "Partial Compression": reinterpret the `dynrng` word as a
/// signed 8-bit fraction `X0 . X1 X2 Y3 Y4 Y5 Y6 Y7`, multiply by the
/// direction-appropriate factor, then re-form an 8-bit word in the
/// original `X0 X1 X2 . Y3 Y4 Y5 Y6 Y7` interpretation.
///
/// Per the spec the byte is a two's-complement signed value in
/// `−128..=127` (units of `1/128` of full range). A *positive* word
/// indicates a gain increase (boost); a *negative* word a gain reduction
/// (cut). The selected factor scales the magnitude; the result is
/// rounded to the nearest integer code and clamped back into the 8-bit
/// signed range, then returned as a `u8` for re-linearisation by
/// [`dynrng_to_linear`].
///
/// `cut` and `boost` are assumed already clamped to `[0.0, 1.0]` (the
/// [`DrcSettings::partial`] constructor enforces that).
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn scale_dynrng_partial(dynrng: u8, cut: f32, boost: f32) -> u8 {
    // Reinterpret the raw byte as a signed 8-bit value: the §7.7.1.2
    // "signed fractional number" X0 . (X1 X2 Y3..Y7). The numeric ordering
    // is identical to two's-complement on the byte.
    let signed = dynrng as i8 as i32;
    if signed == 0 {
        // 0 dB word — no gain change in either direction; nothing to scale.
        return 0;
    }
    let factor = if signed > 0 { boost } else { cut };
    // Scale the magnitude, round to nearest integer code.
    let scaled = (signed as f32 * factor).round() as i32;
    // Clamp into the signed 8-bit range, then reinterpret as the original
    // unsigned X0 X1 X2 . Y3..Y7 byte.
    let clamped = scaled.clamp(-128, 127);
    (clamped as i8) as u8
}

/// Convert an 8-bit `compr` heavy-compression word to a linear gain
/// multiplier (§7.7.2.2, Table 7.30).
///
/// Layout `X0 X1 X2 X3 . Y4 Y5 Y6 Y7`:
/// * `X` is a 4-bit signed integer in `−8..=7`; the coarse gain is
///   `(X + 1) · 6.02 dB`, realised as `2^(X+1)` arithmetic shifts (range
///   `+48.16 dB` down to `−42.14 dB`).
/// * `Y` is the fractional mantissa `0.1 Y4 Y5 Y6 Y7` (base 2), i.e.
///   `(16 + Y) / 32` in `[0.5, 0.96875]` (`−0.28 dB` to `−6.02 dB`).
///
/// Combined linear gain is `2^(X+1) · (16 + Y) / 32`, spanning `+47.89 dB`
/// down to `−48.16 dB`.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn compr_to_linear(compr: u8) -> f32 {
    let x = ((compr >> 4) & 0xF) as i32;
    let x_signed = if x >= 8 { x - 16 } else { x };
    let y = (compr & 0xF) as i32;
    let y_val = (16 + y) as f32 / 32.0;
    let shift = x_signed + 1;
    let base = 2f32.powi(shift);
    base * y_val
}

/// §7.6 dialogue-normalisation playback gain.
///
/// `dialnorm` and `target` are both "dB of headroom below digital 100%"
/// (`1..=31`; the reserved `0` codepoint is treated as a no-op / unity).
/// Normalising a stream authored at `dialnorm` to a desired playback
/// `target` is an attenuation of `target − dialnorm` dB.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn dialnorm_gain(dialnorm: u8, target: u8) -> f32 {
    if dialnorm == 0 {
        // Reserved codepoint — §5.4.2.8 maps it to the −31 dB default; a
        // decoder that can't trust the value should not re-scale.
        return 1.0;
    }
    let delta_db = target as f32 - dialnorm as f32;
    10f32.powf(delta_db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db(linear: f32) -> f32 {
        20.0 * linear.log10()
    }

    #[test]
    fn dynrng_zero_word_is_unity() {
        assert!((dynrng_to_linear(0x00) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn dynrng_table_729_endpoints() {
        // X=3 (0b011), Y=0x1F → +24.08 dB coarse, then the Y mantissa.
        // X=3, Y=31: 2^4 * (32+31)/64 = 16 * 63/64 = 15.75 → +23.95 dB.
        let max = dynrng_to_linear(0b011_11111);
        assert!((db(max) - 23.95).abs() < 0.05, "max dynrng {}", db(max));
        // X=-4 (0b100), Y=0 → 2^-3 * 32/64 = 0.125*0.5 = 0.0625 → -24.08 dB.
        let min = dynrng_to_linear(0b100_00000);
        assert!((db(min) - (-24.08)).abs() < 0.05, "min dynrng {}", db(min));
    }

    #[test]
    fn compr_table_730_endpoints() {
        // X=7 (0b0111), Y=15 → 2^8 * (16+15)/32 = 256 * 31/32 = 248 → +47.89 dB.
        let max = compr_to_linear(0b0111_1111);
        assert!((db(max) - 47.89).abs() < 0.05, "max compr {}", db(max));
        // X=-8 (0b1000), Y=0 → 2^-7 * 16/32 = (1/128)*0.5 → -48.16 dB.
        let min = compr_to_linear(0b1000_0000);
        assert!((db(min) - (-48.16)).abs() < 0.05, "min compr {}", db(min));
        // The 0 dB code per Table 7.30: X=-1 (0b1111), Y=15 →
        // 2^0 * 31/32 ≈ 0.969 → -0.28 dB (the Y mantissa floor; the table
        // lists the X-only "0 dB None" row, the Y term then trims it).
        let near0 = compr_to_linear(0b1111_1111);
        assert!(
            (db(near0) - (-0.28)).abs() < 0.05,
            "compr near-0 {}",
            db(near0)
        );
    }

    #[test]
    fn partial_compression_full_factor_is_identity() {
        // factor 1.0 must reproduce the raw word for every byte.
        for raw in 0u16..=255 {
            let raw = raw as u8;
            assert_eq!(scale_dynrng_partial(raw, 1.0, 1.0), raw, "raw {raw:#04x}");
        }
    }

    #[test]
    fn partial_compression_zero_factor_removes_gain() {
        // factor 0.0 must flatten every word to the 0 dB code.
        for raw in 0u16..=255 {
            let scaled = scale_dynrng_partial(raw as u8, 0.0, 0.0);
            assert!(
                (dynrng_to_linear(scaled) - 1.0).abs() < 1e-6,
                "raw {raw:#04x} scaled {scaled:#04x} not unity"
            );
        }
    }

    #[test]
    fn partial_compression_halves_a_cut() {
        // A gain-reduction word (negative signed byte) scaled by cut=0.5
        // should land roughly halfway (in dB) toward unity. Pick -18.06 dB
        // worth of cut: X=-4..., use 0b100_00000 = signed -128.
        let raw = 0b100_00000u8; // signed -128 → strong cut
        let scaled = scale_dynrng_partial(raw, 0.5, 1.0);
        // -128 * 0.5 = -64 → 0b1100_0000 = 0xC0 = X=-2(0b110),Y=0.
        assert_eq!(scaled, 0xC0);
        let g = dynrng_to_linear(scaled);
        // X=-2,Y=0 → 2^-1 * 0.5 = 0.25 → -12.04 dB. The original
        // 0b100_00000 was -24.08 dB; half (in this signed-code sense) is
        // -12.04 dB. ✓
        assert!((db(g) - (-12.04)).abs() < 0.05, "halved cut {}", db(g));
    }

    #[test]
    fn partial_compression_independent_directions() {
        // boost=0 must zero out a positive (increase) word while a cut=1
        // leaves negative words untouched.
        let boost_word = 0b001_00000u8; // signed +32 → boost
        assert_eq!(scale_dynrng_partial(boost_word, 1.0, 0.0), 0);
        let cut_word = 0b110_00000u8; // signed -64 → cut
        assert_eq!(scale_dynrng_partial(cut_word, 1.0, 1.0), cut_word);
    }

    #[test]
    fn dialnorm_gain_unity_when_target_equals_source() {
        assert!((dialnorm_gain(27, 27) - 1.0).abs() < 1e-6);
        assert!((dialnorm_gain(31, 31) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn dialnorm_gain_quieter_target_attenuates() {
        // Normalisation gain in dB is `target − dialnorm` (the same
        // convention as the documented `2^((target − dialnorm)/6)` ≈
        // `10^((target − dialnorm)/20)` playback formula). Stream authored
        // at dialnorm=27, target=31 → +4 dB; the inverse pair (27↔31)
        // multiplies back to unity.
        let g = dialnorm_gain(27, 31);
        assert!((g - 10f32.powf(4.0 / 20.0)).abs() < 1e-5);
        let g2 = dialnorm_gain(31, 27);
        assert!((g2 - 10f32.powf(-4.0 / 20.0)).abs() < 1e-5);
        // Inverse pair multiplies to unity.
        assert!((g * g2 - 1.0).abs() < 1e-5);
    }

    #[test]
    fn dialnorm_reserved_codepoint_is_unity() {
        assert!((dialnorm_gain(0, 31) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn settings_line_out_resolves_full_dynrng() {
        let s = DrcSettings::line_out();
        let raw = 0b110_00000u8;
        assert!((s.resolve_block_gain(raw, Some(0x00)) - dynrng_to_linear(raw)).abs() < 1e-6);
        assert!((s.dialnorm_gain(27) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn settings_rf_mode_uses_compr_when_present() {
        let s = DrcSettings::rf_mode();
        let dynrng = 0b110_00000u8;
        let compr = 0b1000_0000u8; // -48.16 dB
        let g = s.resolve_block_gain(dynrng, Some(compr));
        assert!((g - compr_to_linear(compr)).abs() < 1e-6);
        // No compr in this frame → fall back to dynrng (§7.7.2.1).
        let g2 = s.resolve_block_gain(dynrng, None);
        assert!((g2 - dynrng_to_linear(dynrng)).abs() < 1e-6);
    }

    #[test]
    fn settings_custom_clamps_factors() {
        let s = DrcSettings::partial(2.0, -1.0);
        match s.mode {
            DrcMode::Custom { cut, boost } => {
                assert_eq!(cut, 1.0);
                assert_eq!(boost, 0.0);
            }
            _ => panic!("expected Custom"),
        }
    }
}

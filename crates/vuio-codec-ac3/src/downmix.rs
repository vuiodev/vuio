//! AC-3 channel-layout downmix (§7.8).
//!
//! Maps an arbitrary source `acmod` channel layout onto a 2- or 1-channel
//! output using the §7.8.1 / §7.8.2 matrix equations. The per-channel
//! weights come from §5.4.2.4 `cmixlev` / §5.4.2.5 `surmixlev` (Tables
//! 5.9 / 5.10), remapped to linear gains via the existing
//! [`crate::tables::CENTER_MIX_LEVEL`] / [`SURROUND_MIX_LEVEL`] LUTs.
//!
//! ## Scope
//!
//! - **Target layouts:** 2-channel `LoRo` (conventional stereo),
//!   2-channel `LtRt` (Dolby Surround matrix-encoded stereo per
//!   §7.8.2's `Lt = L + 0.707·C − 0.707·Ls − 0.707·Rs` / `Rt = R +
//!   0.707·C + 0.707·Ls + 0.707·Rs`), and 1-channel mono. Per §7.8.2
//!   LoRo is the preferred downmix when the ultimate target is mono;
//!   LtRt is selected only when the consumer wants a matrix-encoded
//!   pair to feed a surround decoder downstream.
//! - **Source layouts:** every `acmod` ≥ 1 (1/0, 2/0, 3/0, 2/1, 3/1,
//!   2/2, 3/2). `acmod = 0` (dual-mono 1+1) is handled by routing Ch1
//!   into the Left output and Ch2 into the Right, which is the "Stereo"
//!   dualmode path from §7.8.1.
//! - **LFE:** always dropped. §7.8 leaves the LFE downmix coefficient
//!   implementation-defined; adding LFE to the stereo sum is risky
//!   (speakers crossed-over to a sub bus will double-tap the bass), so
//!   this decoder routes LFE to nothing — a spec-permitted default
//!   matching the absence of LFE in mainstream stereo downmix output.
//! - **Overload scaling:** §7.8.2 mandates attenuating the matrix so
//!   `sum-of-coefficients ≤ 1`. We compute the per-output sum exactly
//!   and divide — for stereo 3/2 this lands at the spec's 0.4143 worst
//!   case; for narrower layouts the scaling is looser, preserving
//!   envelope.
//!
//! Construction is cheap (just a 5×2 matrix of `f32`) so callers can
//! cache a `Downmix` across syncframes or build one per block.

use crate::bsi::{annex_d_center_mix_gain, annex_d_surround_mix_gain, AnnexDMixLevels, Bsi};
use crate::eac3::Eac3Bsi;
use crate::tables::{CENTER_MIX_LEVEL, SURROUND_MIX_LEVEL};

/// Output layout requested from the decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownmixMode {
    /// Leave the source channels untouched.
    Passthrough,
    /// Mix every source channel into a 2-channel LoRo pair (§7.8.2's
    /// conventional stereo equations).
    Stereo,
    /// Mix every source channel into a 2-channel LtRt pair — the
    /// §7.8.2 Dolby Surround matrix-encoded stereo form. Surrounds
    /// fold in with opposite signs into Lt vs Rt at a fixed 0.707
    /// coefficient so a downstream matrix decoder (Pro Logic et al.)
    /// can recover them. Spec equations:
    ///   `Lt = L + 0.707·C − 0.707·Ls − 0.707·Rs`
    ///   `Rt = R + 0.707·C + 0.707·Ls + 0.707·Rs`
    StereoLtRt,
    /// Mix every source channel into a single mono channel.
    Mono,
}

impl DownmixMode {
    /// Resolve from a user-requested output channel count (`None`
    /// meaning "pass through"). A requested count that matches the
    /// source `nfchans` also becomes `Passthrough`, even when LFE is
    /// on — AC-3 never downmixes LFE explicitly. `Some(2)` resolves
    /// to [`Self::Stereo`] (LoRo); selecting LtRt requires explicit
    /// API (decoder setter) since the wire `dsurmod` field advertises
    /// whether the program *was* matrix-encoded but does not mandate
    /// a particular downmix target.
    pub fn resolve(requested: Option<u16>, source_nfchans: u8) -> Self {
        match requested {
            None => Self::Passthrough,
            Some(1) => Self::Mono,
            Some(2) if source_nfchans > 2 || source_nfchans == 0 => Self::Stereo,
            Some(2) if source_nfchans == 2 => Self::Passthrough,
            Some(2) if source_nfchans == 1 => Self::Stereo,
            _ => Self::Passthrough,
        }
    }
}

/// Pre-computed per-channel coefficients for one syncframe. The source
/// layout slot order matches `acmod`'s Table 5.8 ordering
/// `[L, C, R, Ls/S, Rs]`, with missing channels having a zero weight.
///
/// Each `DownmixGains` row lists the weight applied to *that source slot*
/// when summed into an output channel. For stereo the two rows are
/// Left-output-coeffs and Right-output-coeffs. For mono there is one
/// row (Center-output-coeffs).
#[derive(Clone, Debug)]
pub struct Downmix {
    mode: DownmixMode,
    /// `out_coeffs[out_ch][src_slot]` — up to 5 slots per output.
    out_coeffs: [[f32; 5]; 2],
    /// Number of active output channels (1 or 2).
    out_channels: u8,
    /// Source acmod for spec diagnostics.
    src_acmod: u8,
    /// Source nfchans for arithmetic in `apply`.
    src_nfchans: u8,
    /// Whether LFE exists on the source (never mixed in).
    src_lfe: bool,
}

impl Downmix {
    /// Build a downmix matrix from BSI state and a target mode. When
    /// `mode` is [`DownmixMode::Passthrough`] the returned `Downmix`
    /// still records the source layout but `out_channels` is set to
    /// `nfchans + lfe`; [`Downmix::apply`] short-circuits.
    pub fn from_bsi(bsi: &Bsi, mode: DownmixMode) -> Self {
        // §5.4.2.4 / §5.4.2.5 — reserved code 0b11 maps to the
        // "intermediate" coefficient per spec. Our `CENTER_MIX_LEVEL`
        // table already repeats the middle value at index 3 so the
        // reserved code resolves to 0.595 / 0.500.
        let base_clev = if bsi.cmixlev == 0xFF {
            0.707
        } else {
            CENTER_MIX_LEVEL[(bsi.cmixlev & 0x3) as usize]
        };
        let base_slev = if bsi.surmixlev == 0xFF {
            0.707
        } else {
            SURROUND_MIX_LEVEL[(bsi.surmixlev & 0x3) as usize]
        };
        Self::build(
            mode,
            bsi.acmod,
            bsi.nfchans,
            bsi.nchans,
            bsi.lfeon,
            base_clev,
            base_slev,
            bsi.annex_d_mix_levels,
        )
    }

    /// E-AC-3 field-by-field constructor — equivalent to
    /// [`Self::from_eac3_bsi`] but lets callers that hold a
    /// [`crate::eac3::DecodedFrame`] (not the parser-internal `Bsi`)
    /// build a matrix without re-parsing the syncframe. `acmod` /
    /// `nfchans` / `nchans` / `lfeon` / `mix` mirror the
    /// `from_eac3_bsi` fields exactly.
    pub fn from_eac3_fields(
        acmod: u8,
        nfchans: u8,
        nchans: u8,
        lfeon: bool,
        mix: Option<AnnexDMixLevels>,
        mode: DownmixMode,
    ) -> Self {
        Self::build(mode, acmod, nfchans, nchans, lfeon, 0.707, 0.707, mix)
    }

    /// E-AC-3 (Annex E) counterpart to [`Self::from_bsi`]. Annex E
    /// removes the body-spec 2-bit `cmixlev` / `surmixlev` fields and
    /// instead carries refined 3-bit `ltrtcmixlev` / `lorocmixlev` /
    /// `ltrtsurmixlev` / `lorosurmixlev` codewords inside the
    /// `mixmdata` block (§E.2.3.1.3-6, Tables E1.13-16 = D2.3-D2.6).
    ///
    /// When the producer set `mixmdate == 1` the four 3-bit codes
    /// override the §7.8 defaults for the LtRt and LoRo targets
    /// respectively — exactly the same override that base AC-3's
    /// `bsid == 6` xbsi1 block provides. Without `mixmdate` (or in
    /// reduced-channel layouts where the per-channel guards skip the
    /// field), the fixed §7.8.2 LtRt 0.707 and the 0.707 LoRo defaults
    /// apply (mono / 2/0 stereo never have a downmix to refine).
    ///
    /// The Mono target keeps the §7.8.2 fixed 0.707 defaults (Annex E
    /// mixmdata, like Annex D xbsi1, has no mono-specific mix levels).
    pub fn from_eac3_bsi(bsi: &Eac3Bsi, mode: DownmixMode) -> Self {
        // Annex E has no body-spec cmixlev/surmixlev; default to 0.707
        // (§7.8.2 "if not otherwise specified") for the mono path and
        // any LoRo/LtRt downmix on a stream that elected to skip the
        // mixmdata refinement.
        Self::build(
            mode,
            bsi.acmod,
            bsi.nfchans,
            bsi.nchans,
            bsi.lfeon,
            0.707,
            0.707,
            bsi.annex_e_mix_levels,
        )
    }

    /// Shared matrix-fill path for the AC-3 (`from_bsi`) and E-AC-3
    /// (`from_eac3_bsi`) constructors. Resolves the per-target
    /// (`clev`, `slev`) pair from the Annex D / Annex E mix-level
    /// codewords when present, falling back to the supplied
    /// `base_clev` / `base_slev` (which are themselves either the
    /// AC-3 body 2-bit codes or the §7.8.2 fixed 0.707 defaults for
    /// Annex E). Then dispatches to `fill_stereo` / `fill_stereo_ltrt`
    /// / `fill_mono` per `mode`.
    #[allow(clippy::too_many_arguments)]
    fn build(
        mode: DownmixMode,
        acmod: u8,
        nfchans: u8,
        nchans: u8,
        lfeon: bool,
        base_clev: f32,
        base_slev: f32,
        mix: Option<AnnexDMixLevels>,
    ) -> Self {
        let mut out = Self {
            mode,
            out_coeffs: [[0.0; 5]; 2],
            out_channels: nchans,
            src_acmod: acmod,
            src_nfchans: nfchans,
            src_lfe: lfeon,
        };
        if matches!(mode, DownmixMode::Passthrough) {
            return out;
        }

        // Annex D §2.3.1.3-6 (and Annex E mixmdata) provide a refined
        // 3-bit mix level per downmix target. When present they
        // override the body / default for that target's downmix —
        // missing center / surround codes (0xFF sentinel) fall back to
        // the base value so a partial mixmdata block still works.
        let (loro_clev, loro_slev) = match mix {
            Some(m) => (
                if m.lorocmixlev == 0xFF {
                    base_clev
                } else {
                    annex_d_center_mix_gain(m.lorocmixlev)
                },
                if m.lorosurmixlev == 0xFF {
                    base_slev
                } else {
                    annex_d_surround_mix_gain(m.lorosurmixlev)
                },
            ),
            None => (base_clev, base_slev),
        };
        let (ltrt_clev, ltrt_slev) = match mix {
            Some(m) => (
                if m.ltrtcmixlev == 0xFF {
                    // §7.8.2 fixed default when only the LoRo codes
                    // were carried (acmod-guard mismatch).
                    0.707
                } else {
                    annex_d_center_mix_gain(m.ltrtcmixlev)
                },
                if m.ltrtsurmixlev == 0xFF {
                    0.707
                } else {
                    annex_d_surround_mix_gain(m.ltrtsurmixlev)
                },
            ),
            // Body §7.8.2: LtRt uses a fixed 0.707 for the C and
            // surround coefficients regardless of `cmixlev`/`surmixlev`.
            None => (0.707, 0.707),
        };

        match mode {
            DownmixMode::Stereo => Self::fill_stereo(&mut out, acmod, loro_clev, loro_slev),
            DownmixMode::StereoLtRt => {
                Self::fill_stereo_ltrt(&mut out, acmod, ltrt_clev, ltrt_slev)
            }
            DownmixMode::Mono => Self::fill_mono(&mut out, acmod, base_clev, base_slev),
            DownmixMode::Passthrough => unreachable!(),
        }
        out
    }

    /// LoRo 2-channel downmix per §7.8.2. Source slots are indexed as
    /// `L=0, C=1, R=2, Ls/S=3, Rs=4` per Table 5.8; missing channels
    /// leave their coefficient at zero. Output slot 0 = Lo, slot 1 = Ro.
    fn fill_stereo(out: &mut Self, acmod: u8, clev: f32, slev: f32) {
        // Start with the §7.8.2 3/2 LoRo equations:
        //     Lo = 1·L + clev·C + slev·Ls
        //     Ro = 1·R + clev·C + slev·Rs
        // with Table 5.8 dropping channels as the acmod narrows.

        // Source-channel presence per Table 5.8.
        // acmod 0 (1+1 dual mono) is handled separately below — for all
        // other acmods the bit pattern is:
        //   bit 0 (center): acmod == 1, 3, 5, 7
        //   bit 2 (surround): acmod >= 4
        //   two surround channels: acmod == 6, 7
        //   two front (L/R): acmod != 1 (i.e. acmod != 1/0 mono)
        match acmod {
            0 => {
                // 1+1 dual mono — §7.8.1 'dualmode == Stereo' path:
                // Ch1 → Lo, Ch2 → Ro. Slot layout [Ch1, _, Ch2, _, _]:
                // Ch1 at slot 0, Ch2 at slot 2.
                out.out_coeffs[0][0] = 1.0;
                out.out_coeffs[1][2] = 1.0;
            }
            1 => {
                // 1/0 — pure center → both outputs get -3 dB of C.
                // §7.8.1 `output_nfront == 2` path with `input_nfront==1`:
                //   mix center into left with –3 dB
                //   mix center into right with –3 dB
                out.out_coeffs[0][0] = 0.707;
                out.out_coeffs[1][0] = 0.707;
            }
            2 => {
                // 2/0 — pass through. L at slot 0, R at slot 2 per
                // the [L, C, R, Ls/S, Rs] layout.
                out.out_coeffs[0][0] = 1.0;
                out.out_coeffs[1][2] = 1.0;
            }
            3 => {
                // 3/0 — L, C, R into Lo, Ro using clev.
                out.out_coeffs[0][0] = 1.0;
                out.out_coeffs[0][1] = clev;
                out.out_coeffs[1][1] = clev;
                out.out_coeffs[1][2] = 1.0;
            }
            4 => {
                // 2/1 — L, R, S. Single surround is folded into both
                // outputs with slev and, per spec "0.7 * slev" factor
                // for single-surround LoRo.
                out.out_coeffs[0][0] = 1.0;
                out.out_coeffs[0][3] = 0.7 * slev;
                out.out_coeffs[1][2] = 1.0;
                out.out_coeffs[1][3] = 0.7 * slev;
            }
            5 => {
                // 3/1 — L, C, R, S.
                out.out_coeffs[0][0] = 1.0;
                out.out_coeffs[0][1] = clev;
                out.out_coeffs[0][3] = 0.7 * slev;
                out.out_coeffs[1][1] = clev;
                out.out_coeffs[1][2] = 1.0;
                out.out_coeffs[1][3] = 0.7 * slev;
            }
            6 => {
                // 2/2 — L, R, Ls, Rs.
                out.out_coeffs[0][0] = 1.0;
                out.out_coeffs[0][3] = slev;
                out.out_coeffs[1][2] = 1.0;
                out.out_coeffs[1][4] = slev;
            }
            _ => {
                // 3/2 (acmod=7) and any defensive fall-through — full
                // L, C, R, Ls, Rs.
                out.out_coeffs[0][0] = 1.0;
                out.out_coeffs[0][1] = clev;
                out.out_coeffs[0][3] = slev;
                out.out_coeffs[1][1] = clev;
                out.out_coeffs[1][2] = 1.0;
                out.out_coeffs[1][4] = slev;
            }
        }

        out.out_channels = 2;
        // Normalise per §7.8.2: divide each row so its sum is ≤ 1.
        Self::normalise(&mut out.out_coeffs);
    }

    /// LtRt 2-channel matrix-encoded stereo downmix per §7.8.2. The
    /// 3/2 base equations are
    ///   `Lt = 1.0·L + clev·C − slev·Ls − slev·Rs`
    ///   `Rt = 1.0·R + clev·C + slev·Ls + slev·Rs`
    /// and for 3/1 (single surround S folded in):
    ///   `Lt = 1.0·L + clev·C − slev·S`
    ///   `Rt = 1.0·R + clev·C + slev·S`
    /// where `clev` / `slev` default to the §7.8.2 fixed 0.707
    /// coefficients but may be overridden via Annex D §2.3.1.3-4
    /// (`ltrtcmixlev` / `ltrtsurmixlev`, `bsid == 6` streams) — those
    /// fields refine the C / surround gain per encoder authoring.
    ///
    /// Worst-case sum of |coeffs| is `1 + clev + 2·slev`. With the
    /// default 0.707/0.707 it lands at 3.121 → §7.8.2 normalisation
    /// scales every coefficient by 1/3.121 = 0.3204 (9.89 dB
    /// attenuation; Table 7.32 headline). Stronger mix levels just
    /// move the normalisation factor — the surround-channel sign
    /// discipline is invariant, which is what makes the downstream
    /// matrix decoder recoverable.
    fn fill_stereo_ltrt(out: &mut Self, acmod: u8, clev: f32, slev: f32) {
        // Slot layout [L, C, R, Ls/S, Rs] per Table 5.8. Surround
        // channels enter with -slev on Lt and +slev on Rt. Center
        // is symmetric (+clev on both).
        let c = clev;
        let k = slev;
        match acmod {
            0 => {
                // 1+1 dual mono — no matrix encoding makes sense
                // (no surround information to preserve), so fall back
                // to the §7.8.1 'Stereo' dualmode path: Ch1 → Lt,
                // Ch2 → Rt. This is a sentinel choice; an LtRt request
                // on a dual-mono source is degenerate but should still
                // produce something playable.
                out.out_coeffs[0][0] = 1.0;
                out.out_coeffs[1][2] = 1.0;
            }
            1 => {
                // 1/0 — pure center → both outputs get the C gain
                // (default -3 dB = 0.707). Symmetric with the LoRo case
                // (no surround sign play).
                out.out_coeffs[0][0] = c;
                out.out_coeffs[1][0] = c;
            }
            2 => {
                // 2/0 — pass through. No surround to matrix-encode.
                out.out_coeffs[0][0] = 1.0;
                out.out_coeffs[1][2] = 1.0;
            }
            3 => {
                // 3/0 — L, C, R. No surround info; symmetric with LoRo.
                out.out_coeffs[0][0] = 1.0;
                out.out_coeffs[0][1] = c;
                out.out_coeffs[1][1] = c;
                out.out_coeffs[1][2] = 1.0;
            }
            4 => {
                // 2/1 — L, R, S. Single surround folds in with -k on
                // Lt and +k on Rt (spec drops the C term).
                out.out_coeffs[0][0] = 1.0;
                out.out_coeffs[0][3] = -k;
                out.out_coeffs[1][2] = 1.0;
                out.out_coeffs[1][3] = k;
            }
            5 => {
                // 3/1 — L, C, R, S. Single surround folds with opposite
                // signs into Lt/Rt; C symmetric.
                out.out_coeffs[0][0] = 1.0;
                out.out_coeffs[0][1] = c;
                out.out_coeffs[0][3] = -k;
                out.out_coeffs[1][1] = c;
                out.out_coeffs[1][2] = 1.0;
                out.out_coeffs[1][3] = k;
            }
            6 => {
                // 2/2 — L, R, Ls, Rs. Both surrounds fold with
                // opposite signs into Lt/Rt. C term dropped per §7.8.2
                // 'if center is missing'.
                out.out_coeffs[0][0] = 1.0;
                out.out_coeffs[0][3] = -k;
                out.out_coeffs[0][4] = -k;
                out.out_coeffs[1][2] = 1.0;
                out.out_coeffs[1][3] = k;
                out.out_coeffs[1][4] = k;
            }
            _ => {
                // 3/2 (acmod=7) and defensive fall-through — the
                // canonical §7.8.2 LtRt equations.
                out.out_coeffs[0][0] = 1.0;
                out.out_coeffs[0][1] = c;
                out.out_coeffs[0][3] = -k;
                out.out_coeffs[0][4] = -k;
                out.out_coeffs[1][1] = c;
                out.out_coeffs[1][2] = 1.0;
                out.out_coeffs[1][3] = k;
                out.out_coeffs[1][4] = k;
            }
        }

        out.out_channels = 2;
        // Normalise per §7.8.2 — Self::normalise uses |coeff| so the
        // negative surround weights count correctly toward the bound.
        // For 3/2 with clev=slev=0.707 the row-sum-of-|coeffs| is
        // 3.121, so the normalised L-coefficient is 1/3.121 = 0.3204
        // (Table 7.32's headline value); Annex D mix levels just move
        // the normalisation factor without changing the sign discipline.
        Self::normalise(&mut out.out_coeffs);
    }

    /// Mono downmix: derive from the stereo LoRo pair and sum to mono
    /// per §7.8.2 ("a simple summation of the 2 channels"). The spec
    /// also gives the explicit 3/2 mono formula:
    ///     M = L + 2·clev·C + R + slev·Ls + slev·Rs
    /// and the 3/1 form:
    ///     M = L + 2·clev·C + R + 1.4·slev·S
    /// which we build from scratch rather than composing stereo, so we
    /// can apply spec's "further scaling of 1/2" exactly once.
    fn fill_mono(out: &mut Self, acmod: u8, clev: f32, slev: f32) {
        // Slot layout [L, C, R, Ls/S, Rs].
        let (l, c, r, ls, rs) = match acmod {
            0 => {
                // 1+1 dual mono — sum Ch1 + Ch2 with -6 dB each per
                // §7.8.1 (dualmode=Stereo 1-front path).
                (0.5, 0.0, 0.5, 0.0, 0.0)
            }
            1 => (0.0, 1.0, 0.0, 0.0, 0.0),               // 1/0
            2 => (1.0, 0.0, 1.0, 0.0, 0.0),               // 2/0
            3 => (1.0, 2.0 * clev, 1.0, 0.0, 0.0),        // 3/0
            4 => (1.0, 0.0, 1.0, 1.4 * slev, 0.0),        // 2/1
            5 => (1.0, 2.0 * clev, 1.0, 1.4 * slev, 0.0), // 3/1
            6 => (1.0, 0.0, 1.0, slev, slev),             // 2/2
            _ => (1.0, 2.0 * clev, 1.0, slev, slev),      // 3/2 (acmod=7)
        };
        out.out_coeffs[0][0] = l;
        out.out_coeffs[0][1] = c;
        out.out_coeffs[0][2] = r;
        out.out_coeffs[0][3] = ls;
        out.out_coeffs[0][4] = rs;
        out.out_channels = 1;

        // Normalise so the sum of coefficients is ≤ 1 (§7.8.2 overload
        // guard — effectively the "further scaling of 1/2" for mono
        // when the stereo sum already saturates).
        let sum: f32 = out.out_coeffs[0].iter().sum();
        if sum > 1.0 {
            let k = 1.0 / sum;
            for v in out.out_coeffs[0].iter_mut() {
                *v *= k;
            }
        }
    }

    /// Normalise each output row so `sum(|coeff|) ≤ 1`. §7.8.2 calls
    /// this "attenuating all downmix coefficients equally". For the 3/2
    /// LoRo case with default clev=slev=0.707 this matches the spec's
    /// worst-case 0.4143 factor (1 / 2.414) to 3-sig-fig.
    fn normalise(rows: &mut [[f32; 5]; 2]) {
        for row in rows.iter_mut() {
            let sum: f32 = row.iter().map(|c| c.abs()).sum();
            if sum > 1.0 {
                let k = 1.0 / sum;
                for v in row.iter_mut() {
                    *v *= k;
                }
            }
        }
    }

    /// How many channels the downmix produces, including LFE if any.
    /// Passthrough returns the source count.
    pub fn output_channels(&self) -> u8 {
        match self.mode {
            DownmixMode::Passthrough => self.src_nfchans + u8::from(self.src_lfe),
            _ => self.out_channels,
        }
    }

    /// Whether this downmix will touch the samples at all.
    pub fn is_passthrough(&self) -> bool {
        matches!(self.mode, DownmixMode::Passthrough)
    }

    /// Apply this downmix to one block of per-source-channel PCM.
    ///
    /// `src` is indexed as `src[ch][n]` with `ch` in the decoder's
    /// internal fbw order (Table 5.8) — so for 3/2 the channels are
    /// `[L, C, R, Ls, Rs]`. `lfe` is the optional 7th channel;
    /// currently ignored.
    ///
    /// Writes `nsamples × output_channels()` samples into `dst` in
    /// channel-interleaved layout. `dst.len()` must be at least
    /// `nsamples × output_channels()` — extra trailing space is left
    /// untouched.
    pub fn apply(&self, src: &[[f32; 256]; 5], nsamples: usize, dst: &mut [f32]) {
        debug_assert!(self.mode != DownmixMode::Passthrough);
        // Map the decoder's fbw channel index to our [L, C, R, Ls/S, Rs]
        // slot layout. For 1+1 dual-mono Ch1 and Ch2 live at fbw 0 / 1
        // — we alias them to slots L and R since the matrix was built
        // with L/R gains for them in `fill_stereo`/`fill_mono`. For all
        // other acmods we follow Table 5.8 directly.
        let slot_of: [Option<usize>; 5] = match self.src_acmod {
            0 => [Some(0), None, Some(1), None, None], // Ch1, Ch2 → L, R
            1 => [Some(0), None, None, None, None],    // C only at fbw 0 → slot 1? see below
            2 => [Some(0), None, Some(1), None, None], // L, R
            3 => [Some(0), Some(1), Some(2), None, None], // L, C, R
            4 => [Some(0), None, Some(1), Some(2), None], // L, R, S
            5 => [Some(0), Some(1), Some(2), Some(3), None], // L, C, R, S
            6 => [Some(0), None, Some(1), Some(2), Some(3)], // L, R, Ls, Rs
            _ => [Some(0), Some(1), Some(2), Some(3), Some(4)], // 3/2 (acmod=7)
        };
        // Special case acmod=1 (1/0): our single source channel is the
        // center, so its matrix weights sit in slot 1. Rebuild the
        // mapping with slot 1 pointing to fbw 0 and slot 0 nothing.
        let slot_of = if self.src_acmod == 1 {
            [None, Some(0), None, None, None]
        } else {
            slot_of
        };

        let nch = self.out_channels as usize;
        for n in 0..nsamples {
            for out_ch in 0..nch {
                let coeffs = &self.out_coeffs[out_ch];
                let mut acc = 0.0f32;
                for (slot, fbw) in slot_of.iter().enumerate() {
                    let Some(fbw_idx) = *fbw else { continue };
                    let c = coeffs[slot];
                    if c == 0.0 {
                        continue;
                    }
                    acc += c * src[fbw_idx][n];
                }
                dst[n * nch + out_ch] = acc;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_bsi(acmod: u8, cmixlev: u8, surmixlev: u8, lfeon: bool) -> Bsi {
        let nfchans = crate::tables::acmod_nfchans(acmod);
        // Mirror the typed surfaces from the raw codepoints so the
        // fixture exercises both views; `0xFF` collapses to `None`.
        let center_mix = if cmixlev == 0xFF {
            None
        } else {
            Some(crate::bsi::CenterMixLevel::from_code(cmixlev))
        };
        let surround_mix = if surmixlev == 0xFF {
            None
        } else {
            Some(crate::bsi::SurroundMixLevel::from_code(surmixlev))
        };
        Bsi {
            bsid: 8,
            bsmod: 0,
            acmod,
            nfchans,
            lfeon,
            nchans: nfchans + u8::from(lfeon),
            dialnorm: 27,
            dialnorm_ch2: None,
            cmixlev,
            center_mix,
            surmixlev,
            surround_mix,
            dsurmod: 0xFF,
            dolby_surround_mode: None,
            annex_d_mix_levels: None,
            dmixmod: 0xFF,
            dmixmod_preference: None,
            compr: None,
            compr_ch2: None,
            language_code: None,
            language_code_ch2: None,
            dsurexmod: None,
            dheadphonmod: None,
            adconvtyp: None,
            extra_bsi: None,
            audio_production: None,
            audio_production_ch2: None,
            timecod1: None,
            timecod2: None,
            timecode_presence: crate::bsi::TimeCodePresence::NotPresent,
            copyright_info: crate::bsi::CopyrightInfo::from_bits(false, true),
            addbsi: None,
            bits_consumed: 0,
        }
    }

    fn fake_bsi_annex_d(
        acmod: u8,
        lfeon: bool,
        mix: crate::bsi::AnnexDMixLevels,
        dmixmod: u8,
    ) -> Bsi {
        let nfchans = crate::tables::acmod_nfchans(acmod);
        Bsi {
            bsid: 6,
            bsmod: 0,
            acmod,
            nfchans,
            lfeon,
            nchans: nfchans + u8::from(lfeon),
            dialnorm: 27,
            dialnorm_ch2: None,
            cmixlev: 0xFF,
            center_mix: None,
            surmixlev: 0xFF,
            surround_mix: None,
            dsurmod: 0xFF,
            dolby_surround_mode: None,
            annex_d_mix_levels: Some(mix),
            dmixmod,
            // Mirror the typed view from the raw codepoint so the
            // fixture exercises both surfaces.
            dmixmod_preference: Some(crate::bsi::StereoDownmixPreference::from_code(dmixmod)),
            compr: None,
            compr_ch2: None,
            language_code: None,
            language_code_ch2: None,
            dsurexmod: None,
            dheadphonmod: None,
            adconvtyp: None,
            extra_bsi: None,
            audio_production: None,
            audio_production_ch2: None,
            timecod1: None,
            timecod2: None,
            timecode_presence: crate::bsi::TimeCodePresence::NotPresent,
            copyright_info: crate::bsi::CopyrightInfo::from_bits(false, true),
            addbsi: None,
            bits_consumed: 0,
        }
    }

    #[test]
    fn stereo_downmix_3_2_sum_bounded() {
        // acmod=7, default clev/slev.
        let bsi = fake_bsi(7, 0, 0, true);
        let d = Downmix::from_bsi(&bsi, DownmixMode::Stereo);
        // Both output rows must sum to ≤ 1.
        let sum_l: f32 = d.out_coeffs[0].iter().map(|c| c.abs()).sum();
        let sum_r: f32 = d.out_coeffs[1].iter().map(|c| c.abs()).sum();
        assert!(sum_l <= 1.0 + 1e-6);
        assert!(sum_r <= 1.0 + 1e-6);
        // §7.8.2 says the 3/2 LoRo worst-case scale is 1/2.414 ≈ 0.4143
        // when clev=slev=0.707. The unnormalised L-row is
        // [1, 0.707, 0, 0.707, 0] summing to 2.414, so after normalise
        // the L-coeff (originally 1.0) should read ≈ 0.4143.
        assert!((d.out_coeffs[0][0] - 0.4143).abs() < 1e-3);
    }

    #[test]
    fn stereo_downmix_2_0_is_identity() {
        let bsi = fake_bsi(2, 0xFF, 0xFF, false);
        let d = Downmix::from_bsi(&bsi, DownmixMode::Stereo);
        assert_eq!(d.output_channels(), 2);
        // Lo-row picks up source slot 0 (L); Ro-row picks up slot 2 (R).
        assert_eq!(d.out_coeffs[0][0], 1.0);
        assert_eq!(d.out_coeffs[1][2], 1.0);
        // Other slots zero on both rows.
        for slot in 1..5 {
            assert_eq!(d.out_coeffs[0][slot], 0.0);
        }
        for slot in [0, 1, 3, 4] {
            assert_eq!(d.out_coeffs[1][slot], 0.0);
        }
    }

    #[test]
    fn stereo_downmix_3_1() {
        // 3/1 with cmixlev=0 (0.707), surmixlev=0 (0.707).
        let bsi = fake_bsi(5, 0, 0, false);
        let d = Downmix::from_bsi(&bsi, DownmixMode::Stereo);
        assert_eq!(d.output_channels(), 2);
        // Each row should have L/R, C (clev) and S (0.7·slev) slots populated.
        // Pre-normalise the row is [1, 0.707, 0, 0.4949, 0] summing to 2.2019.
        let sum: f32 = d.out_coeffs[0].iter().map(|c| c.abs()).sum();
        assert!(sum <= 1.0 + 1e-6);
        // Centre weight is non-zero on both rows; surround weight too.
        assert!(d.out_coeffs[0][1] > 0.0);
        assert!(d.out_coeffs[1][1] > 0.0);
        assert!(d.out_coeffs[0][3] > 0.0);
        assert!(d.out_coeffs[1][3] > 0.0);
    }

    #[test]
    fn mono_from_3_2_has_all_sources() {
        let bsi = fake_bsi(7, 0, 0, false);
        let d = Downmix::from_bsi(&bsi, DownmixMode::Mono);
        assert_eq!(d.output_channels(), 1);
        // All five source slots must contribute.
        for slot in 0..5 {
            assert!(
                d.out_coeffs[0][slot] > 0.0,
                "mono slot {} should be >0",
                slot
            );
        }
        // Sum bounded.
        let sum: f32 = d.out_coeffs[0].iter().sum();
        assert!(sum <= 1.0 + 1e-6);
    }

    #[test]
    fn mono_from_mono_routes_center() {
        let bsi = fake_bsi(1, 0xFF, 0xFF, false);
        let d = Downmix::from_bsi(&bsi, DownmixMode::Mono);
        assert_eq!(d.output_channels(), 1);
        // Only the center slot carries weight.
        assert_eq!(d.out_coeffs[0][1], 1.0);
    }

    #[test]
    fn apply_stereo_passes_identity_for_2_0() {
        let bsi = fake_bsi(2, 0xFF, 0xFF, false);
        let d = Downmix::from_bsi(&bsi, DownmixMode::Stereo);
        let mut src: [[f32; 256]; 5] = [[0.0; 256]; 5];
        for n in 0..256 {
            src[0][n] = 0.5;
            src[1][n] = -0.25;
        }
        let mut out = vec![0.0f32; 256 * 2];
        d.apply(&src, 256, &mut out);
        for n in 0..256 {
            assert!((out[n * 2] - 0.5).abs() < 1e-6);
            assert!((out[n * 2 + 1] - -0.25).abs() < 1e-6);
        }
    }

    #[test]
    fn apply_stereo_on_3_2_full_scale_clamps() {
        // Hand a full-scale signal on every source channel; worst-case
        // normalisation must keep the output within ±1.
        let bsi = fake_bsi(7, 0, 0, false);
        let d = Downmix::from_bsi(&bsi, DownmixMode::Stereo);
        let mut src: [[f32; 256]; 5] = [[1.0; 256]; 5];
        // Make R (slot 2) negative so its path doesn't fully phase-cancel.
        for n in 0..256 {
            src[2][n] = 1.0;
        }
        let mut out = vec![0.0f32; 256 * 2];
        d.apply(&src, 256, &mut out);
        for n in 0..256 {
            assert!(out[n * 2].abs() <= 1.0 + 1e-6);
            assert!(out[n * 2 + 1].abs() <= 1.0 + 1e-6);
        }
    }

    #[test]
    fn ltrt_3_2_matches_table_7_32() {
        // 3/2 LtRt: Lt = L + 0.707 C − 0.707 Ls − 0.707 Rs.
        // Unscaled row-sum-of-|coeffs| = 1 + 3·0.707 = 3.121.
        // After §7.8.2 normalisation each coeff is divided by 3.121, so
        // the L term lands at 1/3.121 = 0.3204 (Table 7.32, headline).
        let bsi = fake_bsi(7, 0, 0, false);
        let d = Downmix::from_bsi(&bsi, DownmixMode::StereoLtRt);
        assert_eq!(d.output_channels(), 2);
        assert!((d.out_coeffs[0][0] - 0.3204).abs() < 1e-3);
        // The 0.707 terms (C, Ls, Rs) scale to 0.707/3.121 = 0.2265
        // (Table 7.32's second row).
        assert!((d.out_coeffs[0][1] - 0.2265).abs() < 1e-3);
        assert!((d.out_coeffs[0][3] + 0.2265).abs() < 1e-3); // -K
        assert!((d.out_coeffs[0][4] + 0.2265).abs() < 1e-3); // -K
                                                             // Rt mirrors with +K for both surrounds.
        assert!((d.out_coeffs[1][2] - 0.3204).abs() < 1e-3);
        assert!((d.out_coeffs[1][1] - 0.2265).abs() < 1e-3);
        assert!((d.out_coeffs[1][3] - 0.2265).abs() < 1e-3); // +K
        assert!((d.out_coeffs[1][4] - 0.2265).abs() < 1e-3); // +K
    }

    #[test]
    fn ltrt_surround_sign_discipline() {
        // The whole point of LtRt vs LoRo is that the surround folds
        // in with OPPOSITE signs into Lt vs Rt — that's what a Pro Logic
        // matrix decoder pulls out. Verify the sign pattern across every
        // surround-bearing acmod.
        for &acmod in &[4u8, 5, 6, 7] {
            let bsi = fake_bsi(acmod, 0, 0, false);
            let d = Downmix::from_bsi(&bsi, DownmixMode::StereoLtRt);
            // Surround slot 3 (S or Ls): negative on Lt, positive on Rt.
            assert!(
                d.out_coeffs[0][3] < 0.0,
                "acmod={} Lt slot 3 should be negative, got {}",
                acmod,
                d.out_coeffs[0][3]
            );
            assert!(
                d.out_coeffs[1][3] > 0.0,
                "acmod={} Rt slot 3 should be positive, got {}",
                acmod,
                d.out_coeffs[1][3]
            );
            // The two surround terms must be equal-magnitude in opposite
            // signs at the same slot — that's what makes the matrix
            // decoder's subtraction recover the surround source.
            assert!((d.out_coeffs[0][3] + d.out_coeffs[1][3]).abs() < 1e-6);
        }
    }

    #[test]
    fn ltrt_2_2_drops_center() {
        // §7.8.2: 'if the center channel is missing (2/2 or 2/1 mode)
        // the C term is dropped.' acmod=6 (2/2) has no center.
        let bsi = fake_bsi(6, 0, 0, false);
        let d = Downmix::from_bsi(&bsi, DownmixMode::StereoLtRt);
        assert_eq!(d.out_coeffs[0][1], 0.0, "Lt center weight must be zero");
        assert_eq!(d.out_coeffs[1][1], 0.0, "Rt center weight must be zero");
        // The two surrounds still ride in with opposite signs.
        assert!(d.out_coeffs[0][3] < 0.0);
        assert!(d.out_coeffs[0][4] < 0.0);
        assert!(d.out_coeffs[1][3] > 0.0);
        assert!(d.out_coeffs[1][4] > 0.0);
    }

    #[test]
    fn ltrt_3_1_uses_single_surround_form() {
        // §7.8.2: 3/1 form is Lt = L + 0.707 C − 0.707 S; Rt mirror.
        // acmod=5 (3/1). Single surround S sits at slot 3; slot 4 must
        // stay zero.
        let bsi = fake_bsi(5, 0, 0, false);
        let d = Downmix::from_bsi(&bsi, DownmixMode::StereoLtRt);
        assert_eq!(d.out_coeffs[0][4], 0.0);
        assert_eq!(d.out_coeffs[1][4], 0.0);
        assert!(d.out_coeffs[0][3] < 0.0);
        assert!(d.out_coeffs[1][3] > 0.0);
        // Center is present (acmod 5 has C).
        assert!(d.out_coeffs[0][1] > 0.0);
        assert!(d.out_coeffs[1][1] > 0.0);
    }

    #[test]
    fn ltrt_2_0_passes_no_surround_through() {
        // 2/0 has no surround to matrix-encode; the LtRt path falls back
        // to plain L→Lt, R→Rt. Sums equal 1 so no normalisation kicks in.
        let bsi = fake_bsi(2, 0xFF, 0xFF, false);
        let d = Downmix::from_bsi(&bsi, DownmixMode::StereoLtRt);
        assert_eq!(d.out_coeffs[0][0], 1.0);
        assert_eq!(d.out_coeffs[1][2], 1.0);
        for slot in 1..5 {
            assert_eq!(d.out_coeffs[0][slot], 0.0);
        }
    }

    #[test]
    fn ltrt_apply_preserves_surround_phase_inversion() {
        // Push a +1.0 signal on Ls only (fbw index 3 on acmod=7 source
        // layout). Lt should come out negative; Rt positive; same
        // magnitude. This is the matrix encoder's defining behaviour.
        let bsi = fake_bsi(7, 0, 0, false);
        let d = Downmix::from_bsi(&bsi, DownmixMode::StereoLtRt);
        let mut src: [[f32; 256]; 5] = [[0.0; 256]; 5];
        for n in 0..256 {
            src[3][n] = 1.0; // Ls
        }
        let mut out = vec![0.0f32; 256 * 2];
        d.apply(&src, 256, &mut out);
        for n in 0..256 {
            let lt = out[n * 2];
            let rt = out[n * 2 + 1];
            assert!(lt < 0.0, "Lt should be negative, got {}", lt);
            assert!(rt > 0.0, "Rt should be positive, got {}", rt);
            assert!(
                (lt + rt).abs() < 1e-6,
                "Lt + Rt should cancel, got {}",
                lt + rt
            );
        }
    }

    #[test]
    fn ltrt_3_2_full_scale_does_not_clip() {
        // Worst case: every source channel at full scale. Even with sign
        // flips the row-sum-of-|coeffs| ≤ 1 invariant means the result
        // stays within ±1.
        let bsi = fake_bsi(7, 0, 0, false);
        let d = Downmix::from_bsi(&bsi, DownmixMode::StereoLtRt);
        let mut src: [[f32; 256]; 5] = [[1.0; 256]; 5];
        // Make R negative so the Rt row's L=0 / R=1 / C+Ls+Rs at +K
        // does not phase-cancel and we hit the true magnitude bound.
        for n in 0..256 {
            src[2][n] = 1.0;
        }
        let mut out = vec![0.0f32; 256 * 2];
        d.apply(&src, 256, &mut out);
        for n in 0..256 {
            assert!(out[n * 2].abs() <= 1.0 + 1e-6);
            assert!(out[n * 2 + 1].abs() <= 1.0 + 1e-6);
        }
    }

    #[test]
    fn ltrt_vs_loro_differ_on_surround() {
        // LoRo sums surrounds with the SAME sign into Lt and Rt; LtRt
        // inverts. Plant +1 on Ls and Rs simultaneously and verify the
        // difference: LoRo doubles up, LtRt mostly cancels.
        let bsi = fake_bsi(7, 0, 0, false);
        let loro = Downmix::from_bsi(&bsi, DownmixMode::Stereo);
        let ltrt = Downmix::from_bsi(&bsi, DownmixMode::StereoLtRt);
        let mut src: [[f32; 256]; 5] = [[0.0; 256]; 5];
        for n in 0..256 {
            src[3][n] = 1.0; // Ls
            src[4][n] = 1.0; // Rs
        }
        let mut loro_out = vec![0.0f32; 256 * 2];
        let mut ltrt_out = vec![0.0f32; 256 * 2];
        loro.apply(&src, 256, &mut loro_out);
        ltrt.apply(&src, 256, &mut ltrt_out);
        // LoRo Lt gets +slev·Ls and Lt's slot-4 is zero (LoRo only puts
        // Rs into Ro, not Lo). LtRt Lt gets -K·Ls + -K·Rs, summing to
        // a strongly negative number. Whatever the exact magnitudes,
        // the SIGN of Lt differs between LoRo and LtRt for this input.
        let loro_lt = loro_out[0];
        let ltrt_lt = ltrt_out[0];
        assert!(loro_lt > 0.0, "LoRo Lt should be positive, got {}", loro_lt);
        assert!(ltrt_lt < 0.0, "LtRt Lt should be negative, got {}", ltrt_lt);
    }

    #[test]
    fn resolve_common_cases() {
        assert_eq!(DownmixMode::resolve(None, 5), DownmixMode::Passthrough);
        assert_eq!(DownmixMode::resolve(Some(2), 5), DownmixMode::Stereo);
        assert_eq!(DownmixMode::resolve(Some(2), 2), DownmixMode::Passthrough);
        assert_eq!(DownmixMode::resolve(Some(1), 2), DownmixMode::Mono);
        assert_eq!(DownmixMode::resolve(Some(1), 5), DownmixMode::Mono);
        assert_eq!(DownmixMode::resolve(Some(6), 5), DownmixMode::Passthrough);
    }

    /// Annex D §2.3.1.3 — `ltrtcmixlev` overrides the §7.8.2 fixed
    /// 0.707 center gain. Use 1.000 (code `010`) so the post-
    /// normalisation Lt center weight exceeds the default-0.707 case
    /// by a measurable margin.
    #[test]
    fn ltrt_3_2_honours_annex_d_ltrtcmixlev_override() {
        use crate::bsi::AnnexDMixLevels;
        let mix = AnnexDMixLevels {
            ltrtcmixlev: 0b010,   // 1.000
            ltrtsurmixlev: 0b100, // 0.707 (default)
            lorocmixlev: 0b100,   // 0.707
            lorosurmixlev: 0b100, // 0.707
        };
        let bsi = fake_bsi_annex_d(7, false, mix, 0xFF);
        let d = Downmix::from_bsi(&bsi, DownmixMode::StereoLtRt);
        // Pre-normalise: |row| = 1 + 1.0 + 2*0.707 = 3.414, so the
        // normalised L weight is 1/3.414 ≈ 0.2929, C weight is the
        // same. Both bigger than the 0.707-clev would produce on the
        // C slot (0.2265) and smaller on the L slot (0.3204).
        let l = d.out_coeffs[0][0];
        let c = d.out_coeffs[0][1];
        assert!(
            (l - 0.2929).abs() < 1e-3,
            "Lt L weight: want 0.2929, got {}",
            l
        );
        assert!(
            (c - 0.2929).abs() < 1e-3,
            "Lt C weight (ltrtcmixlev=010 → 1.0): want 0.2929, got {}",
            c
        );
        // Surround sign discipline preserved.
        assert!(d.out_coeffs[0][3] < 0.0);
        assert!(d.out_coeffs[1][3] > 0.0);
    }

    /// Annex D §2.3.1.4 — reserved `ltrtsurmixlev` codes (000/001/010)
    /// substitute 0.841 per spec note. Verify the coefficient ends up
    /// at 0.841 / row-sum, not the default 0.707 / row-sum.
    #[test]
    fn ltrt_reserved_surround_code_substitutes_0_841() {
        use crate::bsi::AnnexDMixLevels;
        let mix = AnnexDMixLevels {
            ltrtcmixlev: 0b100,   // 0.707
            ltrtsurmixlev: 0b001, // reserved → 0.841
            lorocmixlev: 0b100,
            lorosurmixlev: 0b100,
        };
        let bsi = fake_bsi_annex_d(7, false, mix, 0xFF);
        let d = Downmix::from_bsi(&bsi, DownmixMode::StereoLtRt);
        // Pre-normalise row sum: 1 + 0.707 + 2*0.841 = 3.389.
        // Surround weight after normalise = -0.841/3.389 ≈ -0.2482.
        let s = d.out_coeffs[0][3];
        assert!(
            (s.abs() - 0.2482).abs() < 1e-3,
            "Lt surround weight: want 0.2482, got {}",
            s
        );
        assert!(s < 0.0, "Lt surround weight must still be negative");
    }

    /// Annex D §2.3.1.5 — `lorocmixlev` overrides the body `cmixlev`
    /// for the LoRo downmix specifically. Verify a non-default
    /// override propagates into the LoRo C weight.
    #[test]
    fn loro_honours_annex_d_lorocmixlev_override() {
        use crate::bsi::AnnexDMixLevels;
        let mix = AnnexDMixLevels {
            ltrtcmixlev: 0b100,
            ltrtsurmixlev: 0b100,
            lorocmixlev: 0b010,   // 1.000 — louder than the 0.707 default
            lorosurmixlev: 0b100, // 0.707
        };
        let bsi = fake_bsi_annex_d(7, false, mix, 0xFF);
        let d = Downmix::from_bsi(&bsi, DownmixMode::Stereo);
        // Default LoRo (clev=0.707, slev=0.707) row = [1, 0.707, 0,
        // 0.707, 0], sum=2.414 → C weight = 0.707/2.414 = 0.2928.
        // With lorocmixlev=010 → clev=1.0, row = [1, 1.0, 0, 0.707, 0],
        // sum=2.707 → C weight = 1.0/2.707 = 0.3694.
        let c = d.out_coeffs[0][1];
        assert!(
            (c - 0.3694).abs() < 1e-3,
            "LoRo C weight (lorocmixlev=010 → 1.0): want 0.3694, got {}",
            c
        );
    }

    /// When `bsid != 6` (no Annex D extension) the §7.8.2 base form
    /// applies — LtRt is the fixed-0.707 case, completely uninfluenced
    /// by `cmixlev` / `surmixlev`. Regression guard for the round-126
    /// refactor that introduced parameterisation.
    #[test]
    fn ltrt_without_annex_d_uses_fixed_0_707() {
        // Set body cmixlev to 0.500 (code 0b10) just to confirm it
        // does NOT bleed into the LtRt path. Behaviour must match the
        // pre-round-126 baseline.
        let bsi = fake_bsi(7, 0b10, 0b10, false);
        let d = Downmix::from_bsi(&bsi, DownmixMode::StereoLtRt);
        assert!((d.out_coeffs[0][0] - 0.3204).abs() < 1e-3);
        assert!((d.out_coeffs[0][1] - 0.2265).abs() < 1e-3);
        assert!((d.out_coeffs[0][3] + 0.2265).abs() < 1e-3);
    }

    /// E-AC-3 (Annex E) field-based constructor with full mixmdata —
    /// matrix matches the AC-3 / Annex D path for the same codeword
    /// set. Regression guard against the shared-fill `build` helper
    /// diverging between the two parsers.
    #[test]
    fn eac3_fields_match_annex_d_for_same_mix_codes() {
        use crate::bsi::AnnexDMixLevels;
        let mix = AnnexDMixLevels {
            ltrtcmixlev: 0b010,   // 1.000
            ltrtsurmixlev: 0b100, // 0.707
            lorocmixlev: 0b100,   // 0.707
            lorosurmixlev: 0b101, // 0.595
        };
        let d_e = Downmix::from_eac3_fields(7, 5, 6, true, Some(mix), DownmixMode::StereoLtRt);
        let bsi = fake_bsi_annex_d(7, true, mix, 0xFF);
        let d_d = Downmix::from_bsi(&bsi, DownmixMode::StereoLtRt);
        // Compare both rows coefficient-by-coefficient.
        for row in 0..2 {
            for col in 0..5 {
                assert!(
                    (d_e.out_coeffs[row][col] - d_d.out_coeffs[row][col]).abs() < 1e-6,
                    "mismatch row {row} col {col}: e-ac3 {} vs annex-d {}",
                    d_e.out_coeffs[row][col],
                    d_d.out_coeffs[row][col],
                );
            }
        }
        assert_eq!(d_e.output_channels(), 2);
    }

    /// E-AC-3 without mixmdata — LtRt falls back to the §7.8.2 fixed
    /// 0.707 defaults exactly like base AC-3 without xbsi1. Matrix is
    /// byte-identical to the `ltrt_without_annex_d_uses_fixed_0_707`
    /// baseline (the Eac3 path has no body cmixlev/surmixlev so this
    /// is the only sensible default).
    #[test]
    fn eac3_fields_without_mixmdata_uses_fixed_0_707() {
        let d = Downmix::from_eac3_fields(7, 5, 6, true, None, DownmixMode::StereoLtRt);
        assert!((d.out_coeffs[0][0] - 0.3204).abs() < 1e-3);
        assert!((d.out_coeffs[0][1] - 0.2265).abs() < 1e-3);
        assert!((d.out_coeffs[0][3] + 0.2265).abs() < 1e-3);
    }

    /// E-AC-3 LoRo with `lorocmixlev` override — verifies the Annex E
    /// mix-level codeword takes effect on the LoRo path. Mirror of
    /// `loro_honours_annex_d_lorocmixlev_override` but via the
    /// `from_eac3_fields` constructor.
    #[test]
    fn eac3_loro_honours_lorocmixlev_override() {
        use crate::bsi::AnnexDMixLevels;
        let mix = AnnexDMixLevels {
            ltrtcmixlev: 0b100,
            ltrtsurmixlev: 0b100,
            lorocmixlev: 0b010,   // 1.000
            lorosurmixlev: 0b100, // 0.707
        };
        let d = Downmix::from_eac3_fields(7, 5, 5, false, Some(mix), DownmixMode::Stereo);
        let c = d.out_coeffs[0][1];
        // sum = 1 + 1.0 + 0 + 0.707 + 0 = 2.707 → C weight = 1/2.707 = 0.3694.
        assert!(
            (c - 0.3694).abs() < 1e-3,
            "Eac3 LoRo C weight: want 0.3694, got {}",
            c
        );
    }
}

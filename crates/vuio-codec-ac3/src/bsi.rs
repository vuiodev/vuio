//! AC-3 Bit Stream Information — `bsi()` (§5.3.2 / §5.4.2).
//!
//! The BSI immediately follows the 5-byte syncinfo and describes the
//! service characteristics: stream identification, channel layout,
//! dialogue normalization, compression, language, timecode, etc.
//!
//! This module parses the base (bsid ≤ 8) layout. Annex E (E-AC-3,
//! bsid=16) is a separate syntax not handled here — that's a future
//! `oxideav-eac3` crate.

use oxideav_core::bits::BitReader;
use oxideav_core::{Error, Result};

use crate::tables::acmod_nfchans;

/// Largest `bsid` value accepted by the base AC-3 BSI parser. Streams
/// at higher `bsid` values use the Annex E (E-AC-3) syntax — the
/// top-level decoder dispatches them to [`crate::eac3::decoder`].
///
/// The spec mandates muting for `bsid > 8` in pure AC-3 decoders
/// (§5.4.2.7) but accepts up to 10 as a small safety margin for
/// near-compatible streams (legacy bsid=9..=10 variants of base AC-3
/// that still parse the same syntax). bsid 11..=16 is canonical
/// E-AC-3 territory.
pub const MAX_BSID_BASE: u8 = 10;

/// Parsed BSI — just the fields a decoder actually needs. Optional
/// service-metadata (compression gain, language code, timecodes,
/// `addbsi`) is also surfaced for chain consumers but does not drive
/// the decoder PCM path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bsi {
    /// bsid — bit stream identification. Spec mandates decoders built
    /// to A/52 mute for `bsid > 8` (Annex E E-AC-3 is a different
    /// syntax). We surface the raw value and let the decoder decide.
    pub bsid: u8,
    pub bsmod: u8,
    pub acmod: u8,
    /// Number of full-bandwidth channels per Table 5.8.
    pub nfchans: u8,
    /// `true` when the low-frequency-effects channel is on.
    pub lfeon: bool,
    /// Total channel count — `nfchans + lfeon`.
    pub nchans: u8,
    /// Dialogue normalization, 1..=31 dB below reference. 0 is reserved
    /// and spec says to treat it as 31. For a typed surface that
    /// exposes the `1..=31`-dB-below-reference semantics + the §7.6
    /// reproduction-gain derivation, use
    /// [`Bsi::dialogue_normalization`].
    pub dialnorm: u8,
    /// §5.4.2.16 dialogue normalization for Ch2 in 1+1 dual-mono
    /// streams (`acmod == 0`). `None` outside `acmod == 0`. Stored as
    /// the same post-remap `1..=31` codepoint that
    /// [`Bsi::dialnorm`] carries — the reserved `0` wire codepoint
    /// is collapsed to `31` per §5.4.2.8 (the spec note on
    /// §5.4.2.16 reads "This 5-bit code has the same meaning as
    /// dialnorm").
    ///
    /// For the typed surface, see
    /// [`Bsi::dialogue_normalization_ch2`].
    pub dialnorm_ch2: Option<u8>,
    /// Center mix-level coefficient code (cmixlev) for acmod with 3
    /// front channels; 0xFF when absent.
    pub cmixlev: u8,
    /// §5.4.2.4 center mix level (Table 5.9), surfaced as a typed
    /// [`CenterMixLevel`]. `Some` only when the encoder emitted the
    /// 2-bit codeword — i.e. the stream has 3 front channels
    /// (`(acmod & 0x1) != 0 && acmod != 0x1`, equivalently `acmod ∈ {3,
    /// 5, 7}`); `None` for every other channel mode where the wire
    /// field is definitionally absent. Equivalent to the typed view of
    /// [`Bsi::cmixlev`] where the `0xFF` "absent" sentinel becomes
    /// `None`; the raw `cmixlev` field stays authoritative for
    /// bit-stream round-trip and the typed surface is a thin
    /// convenience over it. Lets a §7.8 downmix consumer pick the
    /// per-codepoint center-channel attenuation (0.707 / 0.595 / 0.500)
    /// without re-walking Table 5.9, and the spec's reserved-code
    /// fallback ("the decoder should still reproduce audio. The
    /// intermediate value of cmixlev (-4.5 dB) may be used in this
    /// case") is exposed as
    /// [`CenterMixLevel::coefficient_with_reserved_fallback`].
    pub center_mix: Option<CenterMixLevel>,
    /// Surround mix-level coefficient code (surmixlev); 0xFF when absent.
    pub surmixlev: u8,
    /// §5.4.2.5 surround mix level (Table 5.10), surfaced as a typed
    /// [`SurroundMixLevel`]. `Some` only when the encoder emitted the
    /// 2-bit codeword — i.e. the stream has a surround channel
    /// (`(acmod & 0x4) != 0`, equivalently `acmod ∈ {4, 5, 6, 7}`);
    /// `None` for every other channel mode where the wire field is
    /// definitionally absent. Equivalent to the typed view of
    /// [`Bsi::surmixlev`] where the `0xFF` "absent" sentinel becomes
    /// `None`; the raw `surmixlev` field stays authoritative for
    /// bit-stream round-trip and the typed surface is a thin
    /// convenience over it. Lets a §7.8 downmix consumer pick the
    /// per-codepoint surround-channel attenuation (0.707 / 0.500 / 0)
    /// without re-walking Table 5.10, and the spec's reserved-code
    /// fallback ("the decoder should still reproduce audio. The
    /// intermediate value of surmixlev (-6 dB) may be used in this
    /// case") is exposed as
    /// [`SurroundMixLevel::coefficient_with_reserved_fallback`].
    pub surround_mix: Option<SurroundMixLevel>,
    /// Dolby-Surround flag for 2/0 stereo streams; 0xFF when absent.
    pub dsurmod: u8,
    /// §5.4.2.6 Dolby Surround mode (Table 5.11), surfaced as a typed
    /// [`DolbySurroundMode`]. `Some` only when `acmod == 2` (2/0 stereo
    /// — the only channel layout that carries the codeword on the wire);
    /// `None` otherwise. Equivalent to the typed view of [`Bsi::dsurmod`]
    /// where the `0xFF` "absent" sentinel becomes `None`; the raw
    /// `dsurmod` field stays public for bit-stream round-trip and the
    /// typed surface is a thin convenience over it. Lets a Pro Logic-aware
    /// receiver arm its matrix decoder without consulting a magic-number
    /// sentinel — paired with [`Bsi::dolby_surround_mode`].
    pub dolby_surround_mode: Option<DolbySurroundMode>,
    /// Annex D §2.3 "alternate bit stream syntax" mix-level extensions.
    /// `Some` only when `bsid == 6` AND the encoder set `xbsi1e == 1`;
    /// `None` otherwise. The four 3-bit codewords (`ltrtcmixlev` /
    /// `ltrtsurmixlev` / `lorocmixlev` / `lorosurmixlev`) refine the
    /// 2-bit `cmixlev` / `surmixlev` defaults specifically for the
    /// LtRt vs LoRo downmix targets — see [`crate::downmix`].
    pub annex_d_mix_levels: Option<AnnexDMixLevels>,
    /// Annex D §2.3.1.2 preferred-stereo-downmix-mode (`dmixmod`); 0xFF
    /// when absent. `00` = not indicated, `01` = LtRt preferred,
    /// `10` = LoRo preferred, `11` = reserved.
    pub dmixmod: u8,
    /// Annex D §2.3.1.2 preferred stereo downmix mode (Table D2.2),
    /// surfaced as a typed [`StereoDownmixPreference`]. `Some` only
    /// when `bsid == 6` AND the encoder set `xbsi1e == 1`; `None`
    /// otherwise (base §5.3.2 timecode syntax reuses the bit slot for
    /// `timecod*e/timecod*`). Equivalent to the typed view of [`Bsi::dmixmod`]
    /// where the `0xFF` "absent" sentinel becomes `None`; the raw
    /// `dmixmod` field stays authoritative for bit-stream round-trip
    /// and the typed surface is a thin convenience over it. Lets a
    /// §3.1.1 auto-mode two-channel-downmix router pick LtRt vs
    /// LoRo without consulting a magic-number sentinel.
    pub dmixmod_preference: Option<StereoDownmixPreference>,
    /// Heavy compression gain word (`compr`, §5.4.2.10 / §7.7.2.2). For
    /// 1+1 dual-mono (`acmod == 0`) this is the Ch1 word; Ch2 is
    /// surfaced separately as [`Bsi::compr_ch2`]. `Some` when
    /// `compre == 1` in the bitstream; `None` when the encoder did not
    /// emit a heavy-compression word for this syncframe (the spec's
    /// "use `dynrng` instead for this frame" branch).
    pub compr: Option<CompressionGain>,
    /// Ch2 heavy compression gain word for 1+1 dual-mono only. `None`
    /// outside `acmod == 0`, or inside `acmod == 0` when `compr2e == 0`.
    pub compr_ch2: Option<CompressionGain>,
    /// Annex D §2.3.1.8 Dolby Surround EX mode (`dsurexmod`, 2 bits,
    /// Table D2.7). `Some` only when `bsid == 6` and the `xbsi2e` block
    /// is present; `None` otherwise. Per the spec note the field's
    /// semantics are only defined for `acmod ∈ {6, 7}` (2/2 or 3/2) —
    /// the parser still surfaces the raw decoded variant for other
    /// `acmod` values so a caller can decide whether to honour the
    /// hint (encoders treat reserved-combination codes as advisory).
    pub dsurexmod: Option<DolbySurroundExMode>,
    /// Annex D §2.3.1.9 Dolby Headphone mode (`dheadphonmod`, 2 bits,
    /// Table D2.8). `Some` only when `bsid == 6` and the `xbsi2e`
    /// block is present; `None` otherwise. Per the spec note the
    /// field's semantics are only defined for `acmod == 2` (2/0
    /// stereo); the parser still surfaces the raw decoded variant for
    /// other `acmod` values.
    pub dheadphonmod: Option<DolbyHeadphoneMode>,
    /// Annex D §2.3.1.10 A/D converter type (`adconvtyp`, 1 bit, Table
    /// D2.9). `Some` only when `bsid == 6` and the `xbsi2e` block is
    /// present; `None` otherwise. `Standard` = generic 24-bit PCM
    /// converter; `Hdcd` = HDCD-encoded source.
    pub adconvtyp: Option<AdConverterType>,
    /// Annex D §2.3.1.11-12 reserved-for-future-assignment + encoder-
    /// private trailer of the `xbsi2` block. `Some` only when `bsid ==
    /// 6` AND the encoder set `xbsi2e == 1`; `None` otherwise (the §5.3.2
    /// base syntax reuses the bit slot for `timecod2e/timecod2` and the
    /// trailer is definitionally absent). See [`ExtraBsi2`] — the
    /// surface exposes the raw 8-bit `xbsi2` codepoint, the 1-bit
    /// `encinfo` flag, and an `is_spec_reserved_value()` predicate that
    /// flags non-conformant `xbsi2 != 0x00` encoder output.
    pub extra_bsi: Option<ExtraBsi2>,
    /// §5.4.2.11-12 deprecated 8-bit `langcod` slot, surfaced as a
    /// typed [`LanguageCode`]. `Some` only when the encoder set
    /// `langcode == 1` in the bitstream; `None` when `langcode == 0`
    /// (no `langcod` word follows). Per §5.4.2.12 the slot is an
    /// "8 bit reserved value that shall be set to `0xFF` if present"
    /// — the original 1995 mapping to a table-lookup language id was
    /// removed in 2001, and modern delivery systems carry the ISO
    /// 639-2 language code in the signaling layer instead. Surfacing
    /// the raw byte plus the spec-mandated `is_spec_reserved_value()`
    /// predicate lets a probe / archive tool flag legacy streams that
    /// still carry a non-`0xFF` deprecated value without re-parsing
    /// the BSI. The decoder PCM path is unchanged — the word does not
    /// affect audio reproduction.
    pub language_code: Option<LanguageCode>,
    /// §5.4.2.19-20 Ch2 deprecated `langcod2` slot, surfaced as a
    /// typed [`LanguageCode`]. `Some` only when `acmod == 0` (1+1
    /// dual-mono) AND the encoder set `langcod2e == 1`; `None`
    /// otherwise. Same semantics as [`Self::language_code`] but
    /// routed to the Ch2 reproduction chain — the §5.4.2.20 spec
    /// note reads "See lancod, Section 5.4.2.12 above" so the
    /// wire-conformance check is identical.
    pub language_code_ch2: Option<LanguageCode>,
    /// §5.4.2.13-15 audio production information for the main channel
    /// (Ch1 in a 1+1 dual-mono stream). `Some` only when `audprodie ==
    /// 1` in the bitstream; `None` otherwise. Carries the `mixlevel`
    /// (peak mixing-session SPL hint per §5.4.2.14) and the `roomtyp`
    /// (mixing-room calibration per §5.4.2.15 / Table 5.12). The base
    /// AC-3 decoder does not act on these fields ("not typically used
    /// within the AC-3 decoder, but may be used by other parts of the
    /// audio reproduction equipment") — surfacing them lets a chain
    /// consumer route the hint without re-parsing the BSI.
    pub audio_production: Option<AudioProductionInfo>,
    /// §5.4.2.21-23 audio production information for Ch2 in a 1+1
    /// dual-mono stream (`acmod == 0` AND `audprodi2e == 1`). `None`
    /// outside 1+1 mode or when `audprodi2e == 0`. Same semantics as
    /// [`Bsi::audio_production`] but routed to the Ch2 reproduction
    /// chain.
    pub audio_production_ch2: Option<AudioProductionInfo>,
    /// §5.4.2.27 low-resolution timecode half. `Some` only when the
    /// base syntax is in use (`bsid != 6` — the alternate Annex D
    /// syntax reuses these wire bits for the `xbsi1` block) AND the
    /// encoder set `timecod1e == 1` in the bitstream. Covers hours +
    /// minutes + 8-second increments per §5.4.2.27; combine with
    /// [`Self::timecod2`] for a full ~521 µs-resolution offset.
    ///
    /// Per Annex D §1 / §3.2 the timecode "does not affect the
    /// decoding process in legacy decoders"; surfacing it lets a chain
    /// consumer recover a playback offset for editorial workflows that
    /// pre-date out-of-band timecode.
    pub timecod1: Option<TimeCode1>,
    /// §5.4.2.28 high-resolution timecode half. `Some` only when the
    /// base syntax is in use (`bsid != 6`) AND the encoder set
    /// `timecod2e == 1`. Covers residual seconds + frames +
    /// fractional-frames per §5.4.2.28; can stand alone (sync to
    /// out-of-band wall-clock) or pair with [`Self::timecod1`] for the
    /// full 28-bit code.
    pub timecod2: Option<TimeCode2>,
    /// §5.4.2.26 Table 5.13 presence pattern. Always present —
    /// [`TimeCodePresence::NotPresent`] when both flags are clear (or
    /// when the alternate Annex D syntax is in use, in which case the
    /// `timecod*e` slots carry `xbsi*e` instead and the timecode is
    /// definitionally absent).
    pub timecode_presence: TimeCodePresence,
    /// §5.4.2.24-25 distribution-control hint pair (`copyrightb` +
    /// `origbs`). Always present — every base AC-3 syncframe carries
    /// both 1-bit fields unconditionally per the BSI bit layout
    /// (`bit_stream_info()` syntax in §5.3.2). The decoder PCM path
    /// does not consult these bits; surfacing them lets a chain
    /// consumer enforce a distribution / archive policy without
    /// re-parsing the BSI.
    pub copyright_info: CopyrightInfo,
    /// §5.4.2.29-31 additional bit-stream information payload. `Some`
    /// when the encoder set `addbsie == 1`; `None` when `addbsie == 0`.
    /// The decoder per §5.4.2.30 "is not required to interpret this
    /// information, and thus shall skip over this number of bytes" —
    /// surfacing the payload bytes lets a chain consumer recover an
    /// encoder-private metadata block (Dolby reserved-payload routing,
    /// OAMD packetisation, encoder watermark) without re-parsing the
    /// BSI. See [`AdditionalBitStreamInfo`].
    pub addbsi: Option<AdditionalBitStreamInfo>,
    /// Absolute bit position (in bits, measured from the first byte of
    /// `bsi()` input) where the BSI ended. Callers use this to skip
    /// straight to the audio-block area.
    pub bits_consumed: u64,
}

impl Bsi {
    /// Decode the raw `bsmod` value into a typed [`BitStreamMode`]
    /// per Table 5.7. `bsmod == 0b111` is overloaded by `acmod` and
    /// returns either [`BitStreamMode::VoiceOver`] (acmod=0b001) or
    /// [`BitStreamMode::Karaoke`] (acmod ∈ {0b010..=0b111}); the
    /// `bsmod==0b111 && acmod==0b000` combination is not defined by
    /// the spec and maps to [`BitStreamMode::Reserved`].
    ///
    /// This is a thin convenience over [`Bsi::bsmod`] + [`Bsi::acmod`]
    /// — the raw fields stay authoritative and an unmatched value
    /// never panics here. A player can use the typed result to drive
    /// service-routing (e.g. mute the dialogue-only `Dialogue` track
    /// when also playing a main service, or surface the
    /// `VisuallyImpaired` track to a screen-reader bus).
    pub fn service_type(&self) -> BitStreamMode {
        BitStreamMode::from_bsmod_acmod(self.bsmod, self.acmod)
    }

    /// Typed view over [`Bsi::dialnorm`] per §5.4.2.8. The wrapper
    /// exposes both `db()` (signed, in `-31..=-1` dB) and
    /// `reproduction_gain_linear()` (the §7.6 playback-gain
    /// derivation) so a reproduction system can apply the dialnorm
    /// without re-parsing the BSI.
    ///
    /// Because [`Bsi::dialnorm`] has already been remapped (the
    /// reserved `0` codepoint becomes `31`), the returned
    /// [`DialNorm::is_reserved_wire_codepoint`] always reports
    /// `false` on the value built from this accessor — callers who
    /// need to detect the reserved-wire-code path should consult the
    /// raw [`Bsi::dialnorm`] value directly.
    pub fn dialogue_normalization(&self) -> DialNorm {
        DialNorm::from_wire(self.dialnorm)
    }

    /// Typed view over [`Bsi::dialnorm_ch2`] per §5.4.2.16 — the
    /// Ch2 mirror in 1+1 dual-mono streams. `None` outside
    /// `acmod == 0`.
    pub fn dialogue_normalization_ch2(&self) -> Option<DialNorm> {
        self.dialnorm_ch2.map(DialNorm::from_wire)
    }

    /// Typed view over [`Bsi::dmixmod_preference`] — the Annex D
    /// §2.3.1.2 preferred stereo downmix mode. `Some` only when
    /// `bsid == 6` and the encoder set `xbsi1e == 1`; `None`
    /// otherwise. A §3.1.1 auto-mode two-channel-downmix router
    /// should consult this hint to pick LtRt vs LoRo and fall back
    /// to the §7.8 LoRo defaults when this returns `None` (or when
    /// the hint reports
    /// [`StereoDownmixPreference::is_not_indicated`]).
    pub fn stereo_downmix_preference(&self) -> Option<StereoDownmixPreference> {
        self.dmixmod_preference
    }

    /// Typed view over [`Bsi::dolby_surround_mode`] — the §5.4.2.6
    /// base-syntax Dolby Surround mode (Table 5.11). `Some` only when
    /// `acmod == 2` (2/0 stereo); `None` otherwise. A Pro Logic-aware
    /// receiver can consult this hint to arm its matrix decoder for a
    /// program that was matrix-encoded for surround-from-stereo
    /// recovery. The decoder PCM path is unchanged — per §5.4.2.6 the
    /// field "is not used by the AC-3 decoder, but may be used by
    /// other portions of the audio reproduction equipment".
    pub fn dolby_surround_mode(&self) -> Option<DolbySurroundMode> {
        self.dolby_surround_mode
    }

    /// Typed view over [`Bsi::center_mix`] — the §5.4.2.4 center mix
    /// level (Table 5.9). `Some` only when the stream has 3 front
    /// channels (`acmod ∈ {3, 5, 7}`); `None` otherwise. A §7.8
    /// downmix consumer can use the returned
    /// [`CenterMixLevel::coefficient_with_reserved_fallback`] to pick
    /// the per-codepoint center-channel attenuation without re-walking
    /// Table 5.9 or consulting the raw [`Bsi::cmixlev`] sentinel.
    pub fn center_mix(&self) -> Option<CenterMixLevel> {
        self.center_mix
    }

    /// Typed view over [`Bsi::surround_mix`] — the §5.4.2.5 surround
    /// mix level (Table 5.10). `Some` only when the stream has a
    /// surround channel (`acmod ∈ {4, 5, 6, 7}`); `None` otherwise. A
    /// §7.8 downmix consumer can use the returned
    /// [`SurroundMixLevel::coefficient_with_reserved_fallback`] to pick
    /// the per-codepoint surround-channel attenuation without
    /// re-walking Table 5.10 or consulting the raw [`Bsi::surmixlev`]
    /// sentinel.
    pub fn surround_mix(&self) -> Option<SurroundMixLevel> {
        self.surround_mix
    }

    /// Typed view over [`Bsi::language_code`] — the §5.4.2.11-12
    /// deprecated 8-bit `langcod` slot. `Some` only when the encoder
    /// set `langcode == 1` in the bitstream; `None` when
    /// `langcode == 0` (no `langcod` word follows). Per §5.4.2.12 the
    /// slot is an "8 bit reserved value that shall be set to `0xFF`
    /// if present" — use [`LanguageCode::is_spec_reserved_value`] to
    /// flag legacy streams that still carry a non-`0xFF` value.
    pub fn language_code(&self) -> Option<LanguageCode> {
        self.language_code
    }

    /// Typed view over [`Bsi::language_code_ch2`] — the §5.4.2.19-20
    /// Ch2 `langcod2` slot in 1+1 dual-mono streams. `Some` only when
    /// `acmod == 0` AND `langcod2e == 1`; `None` otherwise. Same
    /// per-§5.4.2.20 semantics as [`Self::language_code`].
    pub fn language_code_ch2(&self) -> Option<LanguageCode> {
        self.language_code_ch2
    }
}

/// Service-type classification of an AC-3 bit stream — Table 5.7
/// "Bit Stream Mode". The encoding is keyed on `bsmod`; the `'111'`
/// codepoint is overloaded and resolves with `acmod`'s help.
///
/// Spec §5.4.2.2: `bsmod` indicates whether the bit stream carries a
/// main audio service (CM, ME, karaoke), an associated service
/// (VI, HI, D, C, E, VO), or — for the unused `bsmod==0b111`
/// /`acmod==0b000` combination — nothing defined.
///
/// Routing recommendations (from §5.4.2.2 and Table 5.7):
///
/// * **Main** services (`CompleteMain` / `MusicAndEffects` / `Karaoke`):
///   the primary playback target. A receiver normally selects exactly
///   one main service at a time.
/// * **Associated** services may be mixed *on top of* a main service
///   (e.g. `VisuallyImpaired` and `HearingImpaired` are descriptive
///   narration / cleaned-dialogue mixes intended to substitute or
///   augment the main mix; `Commentary` / `Emergency` / `VoiceOver`
///   typically mix on top of a separate main).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitStreamMode {
    /// `bsmod=0b000` — main audio service: complete main (CM).
    CompleteMain,
    /// `bsmod=0b001` — main audio service: music and effects (ME).
    MusicAndEffects,
    /// `bsmod=0b010` — associated service: visually impaired (VI).
    VisuallyImpaired,
    /// `bsmod=0b011` — associated service: hearing impaired (HI).
    HearingImpaired,
    /// `bsmod=0b100` — associated service: dialogue (D).
    Dialogue,
    /// `bsmod=0b101` — associated service: commentary (C).
    Commentary,
    /// `bsmod=0b110` — associated service: emergency (E).
    Emergency,
    /// `bsmod=0b111` + `acmod=0b001` (mono) — associated service:
    /// voice over (VO).
    VoiceOver,
    /// `bsmod=0b111` + `acmod ∈ {0b010..=0b111}` — main audio
    /// service: karaoke.
    Karaoke,
    /// `bsmod=0b111` + `acmod=0b000` — undefined by Table 5.7
    /// (`bsmod==0b111` collides with the 1+1 dual-mono `acmod`).
    /// Decoders should treat this as malformed metadata, not error.
    Reserved,
}

impl BitStreamMode {
    /// Resolve a `(bsmod, acmod)` pair into a typed service-type per
    /// Table 5.7. Only the low 3 bits of each input are consulted.
    pub fn from_bsmod_acmod(bsmod: u8, acmod: u8) -> Self {
        match bsmod & 0x7 {
            0b000 => BitStreamMode::CompleteMain,
            0b001 => BitStreamMode::MusicAndEffects,
            0b010 => BitStreamMode::VisuallyImpaired,
            0b011 => BitStreamMode::HearingImpaired,
            0b100 => BitStreamMode::Dialogue,
            0b101 => BitStreamMode::Commentary,
            0b110 => BitStreamMode::Emergency,
            0b111 => match acmod & 0x7 {
                0b000 => BitStreamMode::Reserved,
                0b001 => BitStreamMode::VoiceOver,
                _ => BitStreamMode::Karaoke,
            },
            _ => unreachable!(),
        }
    }

    /// `true` for a main audio service (CM, ME, or karaoke). A
    /// receiver picking a default playback target should normally
    /// route a main service first.
    pub fn is_main(self) -> bool {
        matches!(
            self,
            BitStreamMode::CompleteMain | BitStreamMode::MusicAndEffects | BitStreamMode::Karaoke
        )
    }

    /// `true` for an associated service (VI / HI / D / C / E / VO).
    /// These are typically mixed on top of a separately-decoded main
    /// service.
    pub fn is_associated(self) -> bool {
        matches!(
            self,
            BitStreamMode::VisuallyImpaired
                | BitStreamMode::HearingImpaired
                | BitStreamMode::Dialogue
                | BitStreamMode::Commentary
                | BitStreamMode::Emergency
                | BitStreamMode::VoiceOver
        )
    }

    /// Short ASCII mnemonic per Table 5.7 (e.g. "CM", "ME", "VI",
    /// "HI", "D", "C", "E", "VO", "K"). Stable for UI / logging.
    /// Returns "?" for [`BitStreamMode::Reserved`].
    pub fn mnemonic(self) -> &'static str {
        match self {
            BitStreamMode::CompleteMain => "CM",
            BitStreamMode::MusicAndEffects => "ME",
            BitStreamMode::VisuallyImpaired => "VI",
            BitStreamMode::HearingImpaired => "HI",
            BitStreamMode::Dialogue => "D",
            BitStreamMode::Commentary => "C",
            BitStreamMode::Emergency => "E",
            BitStreamMode::VoiceOver => "VO",
            BitStreamMode::Karaoke => "K",
            BitStreamMode::Reserved => "?",
        }
    }
}

/// §5.4.2.8 dialogue normalization word — the 5-bit `dialnorm`
/// codepoint, lifted into a typed surface.
///
/// Per spec the 5-bit value indicates "how far the average dialogue
/// level is below digital 100 percent": valid codepoints `1..=31`
/// map to `-1 dB`..=`-31 dB`. The `0` codepoint is reserved; a
/// spec-compliant decoder treats it as `31` (the `-31 dB` floor).
///
/// Per §7.6 the `dialnorm` value is **not** consumed inside the
/// AC-3 decoder itself — it is forwarded to the reproduction
/// system's volume controller, which combines it with the
/// listener's chosen playback SPL. With `dialnorm` advertised, a
/// system volume control calibrated in dB SPL stays consistent
/// across programs of different mixing loudness (the spec example
/// describes a listener set to 67 dB SPL receiving a -25 dB
/// program then a -15 dB commercial — the system gain
/// auto-adjusts so the dialogue stays at 67 dB SPL across the
/// boundary). The §7.6 prose closes with "It is mandatory that
/// the dialnorm value and the user selected volume setting both
/// be used to set the reproduction system gain."
///
/// `oxideav-ac3`'s decoder PCM path does not apply the value
/// (the field is forwarded raw on [`crate::bsi::Bsi::dialnorm`])
/// — surfacing the typed value lets a downstream volume
/// controller carry out the §7.6 normalisation without
/// re-parsing the BSI.
///
/// For 1+1 dual-mono streams (`acmod == 0`) the bitstream carries
/// a second copy of the word for Ch2; see
/// [`crate::bsi::Bsi::dialnorm_ch2`] / [`Bsi::dialogue_normalization_ch2`]
/// for that surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DialNorm {
    /// Stored as the post-remap codepoint in `1..=31`. The `0`
    /// wire codepoint is collapsed to `31` per §5.4.2.8; the
    /// `wire_value` accessor recovers the on-the-wire byte if
    /// needed.
    raw: u8,
    /// Records whether the on-the-wire codepoint was the reserved
    /// `0` value (which the parser remaps to `31`). Lets a
    /// careful consumer distinguish "encoder emitted the
    /// reserved code" from a legitimate `31` codepoint.
    was_reserved: bool,
}

impl DialNorm {
    /// Wrap a 5-bit wire codepoint. The reserved `0` codepoint is
    /// remapped to `31` per §5.4.2.8; the original wire value is
    /// preserved for [`Self::wire_value`] and
    /// [`Self::is_reserved_wire_codepoint`].
    ///
    /// Only the low 5 bits of `wire` are consulted.
    pub fn from_wire(wire: u8) -> Self {
        let masked = wire & 0x1F;
        if masked == 0 {
            Self {
                raw: 31,
                was_reserved: true,
            }
        } else {
            Self {
                raw: masked,
                was_reserved: false,
            }
        }
    }

    /// Post-remap codepoint in `1..=31`. This is the value the
    /// reproduction system should use for §7.6 normalisation —
    /// the reserved `0` wire codepoint has already been collapsed
    /// to `31`.
    pub fn codepoint(self) -> u8 {
        self.raw
    }

    /// On-the-wire 5-bit codepoint as it appeared in the
    /// bitstream (`0..=31`). Recovers `0` for the reserved code;
    /// callers re-emitting the BSI byte-for-byte should use this
    /// rather than [`Self::codepoint`].
    pub fn wire_value(self) -> u8 {
        if self.was_reserved {
            0
        } else {
            self.raw
        }
    }

    /// `true` when the on-the-wire codepoint was the reserved
    /// `0` value (which the parser remapped to `31` per
    /// §5.4.2.8). Use to flag malformed encoders without
    /// rejecting the stream — per the spec text "If the reserved
    /// value of 0 is received, the decoder shall use -31 dB."
    pub fn is_reserved_wire_codepoint(self) -> bool {
        self.was_reserved
    }

    /// Dialogue level below digital 100 percent, in dB. Returns
    /// a negative integer in `-31..=-1` per §5.4.2.8 (the spec's
    /// "interpreted as -1 dB to -31 dB" wording — codepoint `N`
    /// maps to `-N dB`).
    pub fn db(self) -> i8 {
        -(self.raw as i8)
    }

    /// Magnitude of the dialogue level below full scale, in dB
    /// (`1..=31`). Equivalent to `-self.db()` — kept as a
    /// separate accessor since the §7.6 prose phrases the value
    /// both ways ("headroom in dB above the subjective dialogue
    /// level" / "how many dB the subjective dialogue level is
    /// below digital 100 percent").
    pub fn level_below_full_scale_db(self) -> u8 {
        self.raw
    }

    /// Linear-domain attenuation factor — multiply a full-scale
    /// digital signal by this to land at the dialogue reference
    /// level. Equivalent to `10^(dialnorm.db() / 20.0)`. Range:
    /// `10^(-31/20) ≈ 0.0282` at codepoint 31 (the `-31 dB`
    /// floor) up to `10^(-1/20) ≈ 0.891` at codepoint 1
    /// (the `-1 dB` ceiling).
    ///
    /// This is the *reverse* of the gain the reproduction system
    /// applies — the spec mandates the system *boost* the signal
    /// by `(listener_target_db + dialnorm.db()) dB`, not attenuate
    /// it by `-dialnorm.db() dB`. Use
    /// [`Self::reproduction_gain_linear`] for the playback gain
    /// derivation.
    pub fn attenuation_linear(self) -> f32 {
        10.0f32.powf(self.db() as f32 / 20.0)
    }

    /// Linear playback gain to bring dialogue at the encoded
    /// level to the listener's target dialogue SPL — per §7.6
    /// "reproduction system gain becomes a function of both the
    /// listeners desired reproduction sound pressure level for
    /// dialogue, and the dialnorm value".
    ///
    /// Given a listener target dialogue level (in dB SPL) and an
    /// assumed full-scale reproduction SPL (`reference_full_scale_db`),
    /// the playback gain in dB is
    /// `listener_target_db - (reference_full_scale_db + self.db())`
    /// — equivalent to
    /// `listener_target_db - reference_full_scale_db + level_below_full_scale_db`.
    /// Returned as a linear multiplier.
    ///
    /// Example (from §7.6): `listener_target_db = 67`,
    /// `reference_full_scale_db = 105` (typical cinema
    /// calibration), `level_below_full_scale_db = 25` →
    /// `67 - 105 + 25 = -13 dB` of attenuation from full scale,
    /// matching the spec example's "full scale digital signals
    /// reproduce at a sound pressure level of 92 dB".
    pub fn reproduction_gain_linear(
        self,
        listener_target_db: f32,
        reference_full_scale_db: f32,
    ) -> f32 {
        let gain_db =
            listener_target_db - reference_full_scale_db + self.level_below_full_scale_db() as f32;
        10.0f32.powf(gain_db / 20.0)
    }
}

/// Heavy compression gain word per Table 7.30 + §7.7.2.2.
///
/// The wire field is 8 bits, split as `X0 X1 X2 X3 . Y4 Y5 Y6 Y7`:
///
/// * The upper nibble `X` is a 4-bit signed integer in the range
///   `-8..=+7` (transmitted MSB-first). It contributes a gain of
///   `(X + 1) * 6.02 dB` — i.e. an arithmetic shift on the PCM
///   sample. The 16 `X` codepoints span `+48.16 dB` (`X=7`) down to
///   `-42.14 dB` (`X=-8`).
/// * The lower nibble `Y` is an unsigned fractional value with an
///   implicit leading `1`, read as `0.1 Y4 Y5 Y6 Y7` in base 2 — i.e.
///   `(16 + Y) / 32`, ranging from `16/32 = 0.5` to `31/32`. It
///   represents a linear *attenuation* between `0` dB and `-6.02` dB.
///
/// The combined linear gain is `linear = 2^(X+1) * (16 + Y) / 32`;
/// the combined dB gain runs from `-48.16 dB` (`X=-8`, `Y=0`,
/// linear `0.5 * 0.5 = 0.25`) up to `+47.89 dB` (`X=7`, `Y=15`,
/// linear `256 * 31/32`).
///
/// Per §7.7.2 the `compr` element is intended to bound the **peak**
/// playback level for downstream feeds with restricted dynamic range
/// (RF modulators, hotel-room feeds, etc.). Decoders that have been
/// instructed to "compress on" SHOULD apply `compr` when present, and
/// fall back to `dynrng` for syncframes that omit it (§7.7.2.1).
/// `oxideav-ac3`'s current PCM path does neither — both `compr` and
/// `dynrng` are left for the application to apply downstream — but
/// surfacing the typed value here lets a player implement the policy
/// without re-parsing the BSI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompressionGain {
    raw: u8,
}

impl CompressionGain {
    /// Wrap the 8-bit wire value verbatim. Every byte pattern is valid
    /// per Table 7.30 (all 256 codepoints map to a defined gain).
    pub fn from_byte(raw: u8) -> Self {
        Self { raw }
    }

    /// Underlying 8-bit wire value — `X0 X1 X2 X3 Y4 Y5 Y6 Y7` packed
    /// MSB-first.
    pub fn raw(self) -> u8 {
        self.raw
    }

    /// Signed `X` field, in `-8..=+7`. Per the §7.7.2.2 description
    /// the four upper bits encode `X` as a 4-bit signed integer
    /// (two's-complement convention: `0b1111 → -1`, `0b1000 → -8`).
    pub fn x(self) -> i8 {
        let x4 = (self.raw >> 4) & 0xF;
        // Sign-extend the 4-bit field.
        if x4 & 0x8 != 0 {
            (x4 as i16 - 16) as i8
        } else {
            x4 as i8
        }
    }

    /// Unsigned `Y` field, in `0..=15`. Combined with the implicit
    /// leading `1`, it represents `(16 + Y) / 32` per §7.7.2.2.
    pub fn y(self) -> u8 {
        self.raw & 0xF
    }

    /// Linear-domain gain coefficient — multiply the decoded PCM by
    /// this scalar. Equals `2^(X+1) * (16 + Y) / 32`.
    pub fn linear(self) -> f32 {
        let x_shift = (self.x() as i32) + 1; // -7..=+8
        let y_frac = (16.0 + self.y() as f32) / 32.0; // 0.5..=31/32
                                                      // 2^x_shift via direct floating multiply: x_shift fits in i32 well
                                                      // within f32 exponent range (-7..=+8).
        let two_pow = 2.0f32.powi(x_shift);
        two_pow * y_frac
    }

    /// dB-domain gain — `20 * log10(linear())`. Range
    /// `-48.16 dB ..= +47.89 dB` per Table 7.30 + §7.7.2.2.
    pub fn decibels(self) -> f32 {
        20.0 * self.linear().log10()
    }
}

/// Annex D §2.3.1.8 Dolby Surround EX mode (Table D2.7).
///
/// Surfaced on [`Bsi::dsurexmod`] when `bsid == 6` and the `xbsi2e`
/// block is present. The spec note constrains the meaningful range of
/// the field to `acmod ∈ {6, 7}` (2/2 and 3/2 — the only layouts that
/// carry a stereo surround pair); for other `acmod` values the field
/// is "reserved" but encoders still emit one of the four codepoints,
/// so the parser surfaces the raw decoded variant and leaves the
/// caller to honour the spec gating.
///
/// "Dolby Pro Logic IIx" is a back-compatible matrix decoder that
/// recovers a 5.1 or 6.1/7.1 program from a Dolby Surround EX-encoded
/// stream; "Dolby Pro Logic IIz" is the matrix variant that recovers a
/// front-height pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DolbySurroundExMode {
    /// `'00'` — encoding not indicated.
    NotIndicated,
    /// `'01'` — explicitly NOT Dolby Surround EX, Pro Logic IIx, or
    /// Pro Logic IIz encoded.
    NotEncoded,
    /// `'10'` — Dolby Surround EX or Pro Logic IIx encoded.
    SurroundExOrProLogicIIx,
    /// `'11'` — Dolby Pro Logic IIz encoded.
    ProLogicIIz,
}

impl DolbySurroundExMode {
    /// Decode the 2-bit wire value verbatim per Table D2.7.
    pub fn from_code(code: u8) -> Self {
        match code & 0x3 {
            0 => DolbySurroundExMode::NotIndicated,
            1 => DolbySurroundExMode::NotEncoded,
            2 => DolbySurroundExMode::SurroundExOrProLogicIIx,
            _ => DolbySurroundExMode::ProLogicIIz,
        }
    }

    /// Raw 2-bit code as it appeared on the wire.
    pub fn raw(self) -> u8 {
        match self {
            DolbySurroundExMode::NotIndicated => 0,
            DolbySurroundExMode::NotEncoded => 1,
            DolbySurroundExMode::SurroundExOrProLogicIIx => 2,
            DolbySurroundExMode::ProLogicIIz => 3,
        }
    }
}

/// Annex D §2.3.1.9 Dolby Headphone mode (Table D2.8).
///
/// Surfaced on [`Bsi::dheadphonmod`] when `bsid == 6` and the `xbsi2e`
/// block is present. The spec note constrains the meaningful range of
/// the field to `acmod == 2` (2/0 stereo); for other `acmod` values
/// the field is "reserved" but the parser still surfaces the raw
/// decoded variant.
///
/// The `'11'` reserved codepoint is mapped to [`DolbyHeadphoneMode::Reserved`];
/// per the spec a decoder receiving the reserved code "should still
/// reproduce audio" and is encouraged to treat it as `NotIndicated`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DolbyHeadphoneMode {
    /// `'00'` — encoding not indicated.
    NotIndicated,
    /// `'01'` — explicitly NOT Dolby Headphone encoded.
    NotEncoded,
    /// `'10'` — Dolby Headphone encoded.
    Encoded,
    /// `'11'` — reserved (treat as [`NotIndicated`](Self::NotIndicated)
    /// per §2.3.1.9; the decoder must still reproduce audio).
    Reserved,
}

impl DolbyHeadphoneMode {
    /// Decode the 2-bit wire value verbatim per Table D2.8.
    pub fn from_code(code: u8) -> Self {
        match code & 0x3 {
            0 => DolbyHeadphoneMode::NotIndicated,
            1 => DolbyHeadphoneMode::NotEncoded,
            2 => DolbyHeadphoneMode::Encoded,
            _ => DolbyHeadphoneMode::Reserved,
        }
    }

    /// Raw 2-bit code as it appeared on the wire.
    pub fn raw(self) -> u8 {
        match self {
            DolbyHeadphoneMode::NotIndicated => 0,
            DolbyHeadphoneMode::NotEncoded => 1,
            DolbyHeadphoneMode::Encoded => 2,
            DolbyHeadphoneMode::Reserved => 3,
        }
    }
}

/// Base-syntax §5.4.2.6 Dolby Surround mode (Table 5.11). A 2-bit
/// advisory carried in 2/0 stereo streams (`acmod == 2`) that flags
/// whether the program was matrix-encoded for a Dolby Surround
/// (Pro Logic-decodable) playback path.
///
/// Surfaced on [`Bsi::dolby_surround_mode`] when `acmod == 2`; the
/// parser returns `None` for every other channel mode because the
/// `dsurmod` slot is not on the wire there (§5.3.2 only emits the
/// 2-bit codeword inside the `acmod == 0x2` guard). The Annex E
/// informational-metadata block (§E.2.3.1.x) carries the same field
/// under the same `acmod == 2` guard, so the Annex E `Bsi` reuses
/// this enum verbatim — single source of truth for both syntaxes.
///
/// Per §5.4.2.6: "This information is not used by the AC-3 decoder,
/// but may be used by other portions of the audio reproduction
/// equipment. If `dsurmod` is set to the reserved code, the decoder
/// should still reproduce audio. The reserved code may be
/// interpreted as 'not indicated'." A receiver that has a Pro Logic
/// matrix decoder available can pre-arm it when this surface reports
/// [`DolbySurroundMode::Encoded`].
///
/// This is the base-syntax cousin of the Annex D §2.3.1.8
/// [`DolbySurroundExMode`] (which extends the same advisory pattern
/// to 6/7-channel `acmod` layouts for the EX / Pro Logic IIx / IIz
/// matrix variants). The two surfaces are independent — a 2/0 stream
/// can only carry `dsurmod`, a 5.1 stream can only carry `dsurexmod`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DolbySurroundMode {
    /// `'00'` — encoding not indicated.
    NotIndicated,
    /// `'01'` — explicitly NOT Dolby Surround encoded.
    NotEncoded,
    /// `'10'` — Dolby Surround encoded (matrix-encoded for a
    /// Pro Logic-decodable playback path).
    Encoded,
    /// `'11'` — reserved (per §5.4.2.6 the decoder must keep
    /// reproducing audio and may interpret this codepoint as
    /// [`NotIndicated`](Self::NotIndicated)).
    Reserved,
}

impl DolbySurroundMode {
    /// Decode the 2-bit wire value verbatim per Table 5.11.
    pub fn from_code(code: u8) -> Self {
        match code & 0x3 {
            0 => DolbySurroundMode::NotIndicated,
            1 => DolbySurroundMode::NotEncoded,
            2 => DolbySurroundMode::Encoded,
            _ => DolbySurroundMode::Reserved,
        }
    }

    /// Raw 2-bit code as it appeared on the wire.
    pub fn raw(self) -> u8 {
        match self {
            DolbySurroundMode::NotIndicated => 0,
            DolbySurroundMode::NotEncoded => 1,
            DolbySurroundMode::Encoded => 2,
            DolbySurroundMode::Reserved => 3,
        }
    }

    /// `true` when the field collapses to "no useful information" per
    /// the §5.4.2.6 spec note ("the reserved code may be interpreted
    /// as 'not indicated'") — both [`NotIndicated`](Self::NotIndicated)
    /// and [`Reserved`](Self::Reserved) fold into one branch for a
    /// receiver that wants to apply a matrix-decode default.
    pub fn is_not_indicated(self) -> bool {
        matches!(
            self,
            DolbySurroundMode::NotIndicated | DolbySurroundMode::Reserved
        )
    }

    /// `true` only for [`Encoded`](Self::Encoded) — a Pro Logic-aware
    /// receiver can use this predicate to arm its matrix decoder.
    pub fn is_dolby_surround_encoded(self) -> bool {
        matches!(self, DolbySurroundMode::Encoded)
    }
}

/// §5.4.2.4 center mix level (Table 5.9). A 2-bit codeword carried
/// only when the stream has 3 front channels (`acmod ∈ {3, 5, 7}`)
/// that picks the nominal down-mix attenuation applied to the centre
/// channel when it is folded into the left / right channels by a
/// stereo downmix.
///
/// Surfaced on [`Bsi::center_mix`] when the wire codeword is present;
/// `None` for every other channel layout because the 2-bit slot is
/// not on the wire there (§5.3.2 only emits the codeword inside the
/// `(acmod & 0x1) != 0 && acmod != 0x1` guard).
///
/// Per §5.4.2.4: "If `cmixlev` is set to the reserved code, decoders
/// should still reproduce audio. The intermediate value of `cmixlev`
/// (-4.5 dB) may be used in this case." That fallback is applied by
/// [`Self::coefficient_with_reserved_fallback`] — callers who want
/// to detect "encoder explicitly emitted the reserved code" should
/// match on the [`Reserved`](Self::Reserved) variant directly.
///
/// Annex E (E-AC-3) removes this 2-bit slot in favour of the refined
/// 3-bit `ltrtcmixlev` / `lorocmixlev` codewords (Tables D2.3 /
/// D2.5), so this enum is base-AC-3 only and is not mirrored on the
/// Annex E `Bsi`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CenterMixLevel {
    /// `'00'` — 0.707 (≈ -3.0 dB).
    Minus3Db,
    /// `'01'` — 0.595 (≈ -4.5 dB). Per §5.4.2.4 this intermediate
    /// value is also the spec's recommended fallback when the
    /// [`Reserved`](Self::Reserved) codepoint is received.
    Minus4Point5Db,
    /// `'10'` — 0.500 (≈ -6.0 dB).
    Minus6Db,
    /// `'11'` — reserved. Per §5.4.2.4 the decoder must still
    /// reproduce audio and may treat this codepoint as equivalent to
    /// [`Minus4Point5Db`](Self::Minus4Point5Db);
    /// [`Self::coefficient_with_reserved_fallback`] applies that
    /// substitution.
    Reserved,
}

impl CenterMixLevel {
    /// Decode the 2-bit wire value verbatim per Table 5.9.
    pub fn from_code(code: u8) -> Self {
        match code & 0x3 {
            0 => CenterMixLevel::Minus3Db,
            1 => CenterMixLevel::Minus4Point5Db,
            2 => CenterMixLevel::Minus6Db,
            _ => CenterMixLevel::Reserved,
        }
    }

    /// Raw 2-bit code as it appeared on the wire.
    pub fn raw(self) -> u8 {
        match self {
            CenterMixLevel::Minus3Db => 0,
            CenterMixLevel::Minus4Point5Db => 1,
            CenterMixLevel::Minus6Db => 2,
            CenterMixLevel::Reserved => 3,
        }
    }

    /// Linear attenuation coefficient per Table 5.9 — `0.707` for
    /// `'00'`, `0.595` for `'01'`, `0.500` for `'10'`, and `None` for
    /// the `'11'` reserved codepoint (the spec leaves the choice to
    /// the decoder; see
    /// [`Self::coefficient_with_reserved_fallback`] for the standard
    /// "intermediate value" substitution).
    pub fn coefficient(self) -> Option<f32> {
        match self {
            CenterMixLevel::Minus3Db => Some(0.707),
            CenterMixLevel::Minus4Point5Db => Some(0.595),
            CenterMixLevel::Minus6Db => Some(0.500),
            CenterMixLevel::Reserved => None,
        }
    }

    /// Linear attenuation coefficient with the §5.4.2.4 reserved-code
    /// substitution applied — for the [`Reserved`](Self::Reserved)
    /// codepoint, returns the intermediate value `0.595` (-4.5 dB) so
    /// a downmix consumer can pick the centre-channel gain in a
    /// single call.
    pub fn coefficient_with_reserved_fallback(self) -> f32 {
        match self {
            CenterMixLevel::Minus3Db => 0.707,
            CenterMixLevel::Minus4Point5Db | CenterMixLevel::Reserved => 0.595,
            CenterMixLevel::Minus6Db => 0.500,
        }
    }

    /// `true` only for [`Reserved`](Self::Reserved) — lets a probe
    /// tool flag streams that picked the reserved codepoint without
    /// re-walking Table 5.9.
    pub fn is_reserved(self) -> bool {
        matches!(self, CenterMixLevel::Reserved)
    }
}

/// §5.4.2.5 surround mix level (Table 5.10). A 2-bit codeword carried
/// only when the stream has a surround channel (`acmod ∈ {4, 5, 6,
/// 7}`) that picks the nominal down-mix attenuation applied to the
/// surround channel(s) when they are folded into the left / right
/// channels by a stereo downmix.
///
/// Surfaced on [`Bsi::surround_mix`] when the wire codeword is
/// present; `None` for every other channel layout because the 2-bit
/// slot is not on the wire there (§5.3.2 only emits the codeword
/// inside the `(acmod & 0x4) != 0` guard).
///
/// Per §5.4.2.5: "If `surmixlev` is set to the reserved code, the
/// decoder should still reproduce audio. The intermediate value of
/// `surmixlev` (-6 dB) may be used in this case." That fallback is
/// applied by [`Self::coefficient_with_reserved_fallback`] — callers
/// who want to detect "encoder explicitly emitted the reserved code"
/// should match on the [`Reserved`](Self::Reserved) variant
/// directly.
///
/// Annex E (E-AC-3) removes this 2-bit slot in favour of the refined
/// 3-bit `ltrtsurmixlev` / `lorosurmixlev` codewords (Tables D2.4 /
/// D2.6), so this enum is base-AC-3 only and is not mirrored on the
/// Annex E `Bsi`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurroundMixLevel {
    /// `'00'` — 0.707 (≈ -3.0 dB).
    Minus3Db,
    /// `'01'` — 0.500 (≈ -6.0 dB). Per §5.4.2.5 this intermediate
    /// value is also the spec's recommended fallback when the
    /// [`Reserved`](Self::Reserved) codepoint is received.
    Minus6Db,
    /// `'10'` — 0 (surround channels muted in the downmix).
    Mute,
    /// `'11'` — reserved. Per §5.4.2.5 the decoder must still
    /// reproduce audio and may treat this codepoint as equivalent to
    /// [`Minus6Db`](Self::Minus6Db);
    /// [`Self::coefficient_with_reserved_fallback`] applies that
    /// substitution.
    Reserved,
}

impl SurroundMixLevel {
    /// Decode the 2-bit wire value verbatim per Table 5.10.
    pub fn from_code(code: u8) -> Self {
        match code & 0x3 {
            0 => SurroundMixLevel::Minus3Db,
            1 => SurroundMixLevel::Minus6Db,
            2 => SurroundMixLevel::Mute,
            _ => SurroundMixLevel::Reserved,
        }
    }

    /// Raw 2-bit code as it appeared on the wire.
    pub fn raw(self) -> u8 {
        match self {
            SurroundMixLevel::Minus3Db => 0,
            SurroundMixLevel::Minus6Db => 1,
            SurroundMixLevel::Mute => 2,
            SurroundMixLevel::Reserved => 3,
        }
    }

    /// Linear attenuation coefficient per Table 5.10 — `0.707` for
    /// `'00'`, `0.500` for `'01'`, `0.000` for `'10'`, and `None` for
    /// the `'11'` reserved codepoint (the spec leaves the choice to
    /// the decoder; see
    /// [`Self::coefficient_with_reserved_fallback`] for the standard
    /// "intermediate value" substitution).
    pub fn coefficient(self) -> Option<f32> {
        match self {
            SurroundMixLevel::Minus3Db => Some(0.707),
            SurroundMixLevel::Minus6Db => Some(0.500),
            SurroundMixLevel::Mute => Some(0.000),
            SurroundMixLevel::Reserved => None,
        }
    }

    /// Linear attenuation coefficient with the §5.4.2.5 reserved-code
    /// substitution applied — for the [`Reserved`](Self::Reserved)
    /// codepoint, returns the intermediate value `0.500` (-6 dB) so
    /// a downmix consumer can pick the surround-channel gain in a
    /// single call.
    pub fn coefficient_with_reserved_fallback(self) -> f32 {
        match self {
            SurroundMixLevel::Minus3Db => 0.707,
            SurroundMixLevel::Minus6Db | SurroundMixLevel::Reserved => 0.500,
            SurroundMixLevel::Mute => 0.000,
        }
    }

    /// `true` only for [`Reserved`](Self::Reserved) — lets a probe
    /// tool flag streams that picked the reserved codepoint without
    /// re-walking Table 5.10.
    pub fn is_reserved(self) -> bool {
        matches!(self, SurroundMixLevel::Reserved)
    }

    /// `true` only for [`Mute`](Self::Mute) — surround channels are
    /// dropped from the stereo downmix. Lets a downmix consumer
    /// short-circuit the surround mix-in step without computing the
    /// coefficient.
    pub fn is_mute(self) -> bool {
        matches!(self, SurroundMixLevel::Mute)
    }
}

/// Annex D §2.3.1.10 A/D converter type (Table D2.9). A single bit:
/// `'0'` indicates a generic / standard PCM A/D converter; `'1'`
/// indicates an HDCD-encoded source (HDCD packs a "hidden" 4 bits in
/// the 16-bit PCM LSBs, and downstream equipment may decode them for a
/// 20-bit dynamic range). The AC-3 decoder treats both identically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdConverterType {
    /// `'0'` — Standard (generic 24-bit PCM).
    Standard,
    /// `'1'` — HDCD-encoded source.
    Hdcd,
}

impl AdConverterType {
    /// Decode the 1-bit wire value verbatim per Table D2.9.
    pub fn from_code(code: u8) -> Self {
        if code & 0x1 == 0 {
            AdConverterType::Standard
        } else {
            AdConverterType::Hdcd
        }
    }

    /// Raw 1-bit code as it appeared on the wire.
    pub fn raw(self) -> u8 {
        match self {
            AdConverterType::Standard => 0,
            AdConverterType::Hdcd => 1,
        }
    }
}

/// Annex D §2.3.1.2 preferred stereo downmix mode (Table D2.2).
///
/// Surfaced on [`Bsi::dmixmod_preference`] when `bsid == 6` and the
/// `xbsi1e` block is present; mirrored on
/// [`crate::eac3::Bsi::dmixmod_preference`] when the Annex E
/// `mixmdate == 1` mixing-metadata block is present and `acmod > 2`.
/// `None` outside those gates — base AC-3 streams with the §5.3.2
/// timecode syntax (`bsid != 6`) cannot carry this hint, and the
/// Annex D / Annex E spec note states the field is meaningful only
/// for the multi-channel audio coding modes (3/0, 2/1, 3/1, 2/2,
/// 3/2); for 1+1 / 1/0 / 2/0 the wire field is reserved and not
/// transmitted.
///
/// Per §2.3.1.2 / §3.1.1 a compliant two-channel-downmix decoder
/// "should allow the end user to specify which two-channel downmix
/// is chosen" with an "automatic selection of either Lt/Rt or Lo/Ro
/// based on the preferred downmix mode parameter dmixmod" as one of
/// the three options — so the typed value is consulted by an
/// auto-mode downmix router. The Reserved codepoint is per spec to
/// be treated as `NotIndicated` ("the decoder should still reproduce
/// audio. The reserved code may be interpreted as 'not indicated'").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StereoDownmixPreference {
    /// `'00'` — not indicated by the encoder. The downmix router
    /// falls back to the LoRo equations from the original §7.8
    /// specification (which the AC-3 decoder defaults to when no
    /// preference is signalled).
    NotIndicated,
    /// `'01'` — Lt/Rt downmix preferred. A matrix-encoded stereo
    /// pair suitable for Dolby Pro Logic / Pro Logic II / IIx
    /// recovery of the surround field; consult the `ltrtcmixlev`
    /// / `ltrtsurmixlev` codewords for the per-channel gains.
    LtRtPreferred,
    /// `'10'` — Lo/Ro downmix preferred. A non-matrix-encoded
    /// stereo pair suitable for conventional two-speaker playback;
    /// consult the `lorocmixlev` / `lorosurmixlev` codewords for
    /// the per-channel gains.
    LoRoPreferred,
    /// `'11'` — reserved. Per spec the decoder should treat the
    /// reserved codepoint as equivalent to
    /// [`NotIndicated`](Self::NotIndicated) and "should still
    /// reproduce audio"; we surface it as its own variant so a
    /// chain consumer can distinguish "encoder explicitly emitted
    /// the reserved code" from "encoder emitted 'not indicated'".
    Reserved,
}

impl StereoDownmixPreference {
    /// Decode the 2-bit wire value verbatim per Table D2.2.
    pub fn from_code(code: u8) -> Self {
        match code & 0x3 {
            0 => StereoDownmixPreference::NotIndicated,
            1 => StereoDownmixPreference::LtRtPreferred,
            2 => StereoDownmixPreference::LoRoPreferred,
            _ => StereoDownmixPreference::Reserved,
        }
    }

    /// Raw 2-bit code as it appeared on the wire.
    pub fn raw(self) -> u8 {
        match self {
            StereoDownmixPreference::NotIndicated => 0,
            StereoDownmixPreference::LtRtPreferred => 1,
            StereoDownmixPreference::LoRoPreferred => 2,
            StereoDownmixPreference::Reserved => 3,
        }
    }

    /// Whether the encoder signalled an explicit Lt/Rt preference.
    ///
    /// Equivalent to `matches!(self, Self::LtRtPreferred)` but
    /// expressed as a method so a downmix router can short-circuit
    /// on the typed predicate.
    pub fn prefers_lt_rt(self) -> bool {
        matches!(self, StereoDownmixPreference::LtRtPreferred)
    }

    /// Whether the encoder signalled an explicit Lo/Ro preference.
    ///
    /// Equivalent to `matches!(self, Self::LoRoPreferred)` but
    /// expressed as a method so a downmix router can short-circuit
    /// on the typed predicate.
    pub fn prefers_lo_ro(self) -> bool {
        matches!(self, StereoDownmixPreference::LoRoPreferred)
    }

    /// Whether the codepoint should be treated as "not indicated"
    /// — covers both the explicit [`NotIndicated`](Self::NotIndicated)
    /// codepoint and the [`Reserved`](Self::Reserved) codepoint
    /// (per §2.3.1.2: "the reserved code may be interpreted as
    /// 'not indicated'"). Lets an auto-mode downmix router collapse
    /// both into the "fall back to §7.8 LoRo defaults" branch with
    /// a single check.
    pub fn is_not_indicated(self) -> bool {
        matches!(
            self,
            StereoDownmixPreference::NotIndicated | StereoDownmixPreference::Reserved
        )
    }
}

/// §5.4.2.15 / Table 5.12 mixing-room type. A 2-bit code describing
/// the calibration of the mixing room used during the final audio
/// mixing session.
///
/// Per spec the value "is not typically used by the AC-3 decoder, but
/// may be used by other parts of the audio reproduction equipment".
/// The reserved code may be interpreted as "not indicated"; we keep
/// it as its own variant so a careful consumer can still distinguish
/// "encoder explicitly left the field blank" from "encoder emitted an
/// invalid codepoint".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomType {
    /// `'00'` — not indicated.
    NotIndicated,
    /// `'01'` — large room, X-curve monitor calibration.
    LargeXCurve,
    /// `'10'` — small room, flat monitor calibration.
    SmallFlat,
    /// `'11'` — reserved (treat as
    /// [`NotIndicated`](Self::NotIndicated) per §5.4.2.15; the
    /// decoder must still reproduce audio).
    Reserved,
}

impl RoomType {
    /// Decode the 2-bit wire value verbatim per Table 5.12.
    pub fn from_code(code: u8) -> Self {
        match code & 0x3 {
            0 => RoomType::NotIndicated,
            1 => RoomType::LargeXCurve,
            2 => RoomType::SmallFlat,
            _ => RoomType::Reserved,
        }
    }

    /// Raw 2-bit code as it appeared on the wire.
    pub fn raw(self) -> u8 {
        match self {
            RoomType::NotIndicated => 0,
            RoomType::LargeXCurve => 1,
            RoomType::SmallFlat => 2,
            RoomType::Reserved => 3,
        }
    }
}

/// §5.4.2.11-12 deprecated language-code word — the optional 8-bit
/// `langcod` slot that follows `langcode == 1` in the §5.3.2 base
/// AC-3 BSI syntax (and its Ch2 `langcod2` mirror at §5.4.2.19-20 in
/// 1+1 dual-mono streams).
///
/// The original 1995 A/52 specification defined `langcod` as an 8-bit
/// table-lookup index into a language identifier table; the 2001
/// revision retired the table-lookup semantics. Per the current
/// §5.4.2.12 wire-conformance rule the slot "is an 8 bit reserved
/// value that shall be set to `0xFF` if present" — modern delivery
/// systems carry the ISO 639-2 language code in the signaling layer,
/// and the in-stream slot is kept only for bitstream-format
/// backwards-compatibility.
///
/// A typed surface over the raw byte lets a probe / archive tool flag
/// legacy streams that still carry a non-`0xFF` value (and may
/// therefore be addressable to the obsolete 1995 lookup table) — via
/// [`Self::is_spec_reserved_value`] — without re-parsing the BSI.
/// The AC-3 decoder ignores this word; the slot exists in the BSI
/// surface only as an informational hint.
///
/// Per §5.4.2.12 the field only exists when `langcode == 1`; the
/// typed surface lives behind `Bsi::language_code:
/// Option<LanguageCode>` so the absence (`langcode == 0`) case
/// short-circuits to `None`. The Ch2 mirror at §5.4.2.20 reads "See
/// lancod, Section 5.4.2.12 above" so the same type backs both
/// slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LanguageCode {
    raw: u8,
}

impl LanguageCode {
    /// Wrap the 8-bit wire byte verbatim. No remap or rejection —
    /// every value `0x00..=0xFF` is representable, and the
    /// spec-conformance check is exposed separately as
    /// [`Self::is_spec_reserved_value`].
    pub fn from_raw(raw: u8) -> Self {
        LanguageCode { raw }
    }

    /// Raw 8-bit code as it appeared on the wire. For a
    /// spec-conforming stream this is always `0xFF`; legacy 1995-era
    /// streams may carry a different value pointing into the retired
    /// language-id table.
    pub fn raw(self) -> u8 {
        self.raw
    }

    /// `true` when the carried byte equals the §5.4.2.12 mandated
    /// reserved value (`0xFF`). For a spec-conforming stream this
    /// predicate is always `true` when the slot is present; a probe
    /// tool can branch on `false` to surface a non-conforming legacy
    /// stream to a chain-of-custody log.
    pub fn is_spec_reserved_value(self) -> bool {
        self.raw == 0xFF
    }
}

/// §5.4.2.13-15 audio production information block — the
/// `audprodie==1` payload (and its Ch2 `audprodi2e==1` mirror in 1+1
/// dual-mono streams). Carries a peak mixing-level hint and the
/// mixing-room calibration.
///
/// Neither field affects AC-3 PCM decoding, but a downstream
/// SPL-calibrated reproduction chain (cinema / mastering monitor)
/// can use them to re-target the playback level back to the absolute
/// SPL the mixing engineer was monitoring at. Per §5.4.2.14 the peak
/// mixing level is `80 + mixlevel` dB SPL, in the documented range
/// 80..=111 dB SPL.
///
/// The Annex E (E-AC-3) `infomdata` informational block reuses the
/// same two fields with identical semantics (§E.2.3.1.x) so the type
/// is shared between the AC-3 and E-AC-3 BSI surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioProductionInfo {
    /// Raw 5-bit `mixlevel` codepoint, in the spec-documented range
    /// 0..=31. The peak mixing-session SPL is
    /// `80 + mixlevel` dB SPL — use [`Self::peak_mix_level_db_spl`]
    /// for the resolved value.
    pub mixlevel: u8,
    /// Typed `roomtyp` decode (Table 5.12).
    pub roomtyp: RoomType,
}

impl AudioProductionInfo {
    /// Resolve the [`Self::mixlevel`] codepoint into its absolute
    /// peak SPL value per §5.4.2.14: the peak mixing level is
    /// `80 + mixlevel` dB SPL, i.e. in the range 80..=111 dB SPL for
    /// a 5-bit codepoint.
    pub fn peak_mix_level_db_spl(self) -> u32 {
        80 + (self.mixlevel as u32 & 0x1F)
    }
}

/// §5.4.2.27 base-syntax `timecod1` field — the **low-resolution** half
/// of the 28-bit SMPTE-style time code. Surfaced on
/// [`Bsi::timecod1`] only when the base syntax is in use (`bsid != 6`,
/// equivalently when the alternate Annex D syntax is *not* selected)
/// AND the encoder set `timecod1e == 1`.
///
/// The 14 wire bits split per §5.4.2.27 as `H H H H H . M M M M M M .
/// S S S` (MSB-first):
///
/// * 5-bit `hours` field — valid range `0..=23` (§5.4.2.27 says values
///   24..=31 are illegal but spec-compliant decoders should still
///   reproduce audio; the parser accepts the raw codepoint and lets
///   the caller decide).
/// * 6-bit `minutes` field — valid range `0..=59`.
/// * 3-bit `eight_second_increments` field — valid range `0..=7`,
///   each step representing 8 seconds (i.e. `0, 8, 16, 24, 32, 40,
///   48, 56` seconds within the current minute).
///
/// The combined resolution is 8 seconds and the addressable range is
/// 24 hours (`24 × 3600 = 86 400 s`). The high-resolution remainder
/// lives in [`TimeCode2`].
///
/// Per §5.4.2.26 and Annex D §1 these slots have "never been applied
/// for their originally anticipated purpose" — modern delivery uses
/// out-of-band timecode (e.g. PTP, SMPTE 12M MTC) — but legacy AC-3
/// streams may still carry them, and a careful consumer can recover a
/// frame-accurate playback offset for editorial workflows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeCode1 {
    raw: u16,
}

impl TimeCode1 {
    /// Wrap the 14-bit wire value verbatim. Only the low 14 bits of
    /// `raw` are consulted; the parser hands the field in already
    /// masked.
    pub fn from_raw(raw: u16) -> Self {
        Self { raw: raw & 0x3FFF }
    }

    /// Underlying 14-bit wire value — `HHHHH MMMMMM SSS` packed
    /// MSB-first.
    pub fn raw(self) -> u16 {
        self.raw
    }

    /// 5-bit `hours` field, in `0..=31` (spec-valid range `0..=23`).
    pub fn hours(self) -> u8 {
        ((self.raw >> 9) & 0x1F) as u8
    }

    /// 6-bit `minutes` field, in `0..=63` (spec-valid range `0..=59`).
    pub fn minutes(self) -> u8 {
        ((self.raw >> 3) & 0x3F) as u8
    }

    /// 3-bit `eight_second_increments` field, in `0..=7`. Each step
    /// represents 8 seconds within the current minute.
    pub fn eight_second_increments(self) -> u8 {
        (self.raw & 0x7) as u8
    }

    /// Total whole-second offset within the 24-hour day represented by
    /// this half — `hours·3600 + minutes·60 + eight_second_increments·8`.
    /// Maxes at `23·3600 + 59·60 + 7·8 = 86 336 s` for spec-valid input;
    /// the raw 5+6+3 bit ranges can push the result up to `122 296 s`
    /// when the encoder emits out-of-range values (the parser still
    /// passes those through verbatim).
    pub fn seconds_in_day(self) -> u32 {
        (self.hours() as u32) * 3600
            + (self.minutes() as u32) * 60
            + (self.eight_second_increments() as u32) * 8
    }

    /// `true` when every field is inside its spec-documented range
    /// (`hours ≤ 23`, `minutes ≤ 59`). The `eight_second_increments`
    /// field cannot overflow its spec range (its 3-bit width caps it at
    /// 7). Use this to flag malformed encoders without rejecting the
    /// stream — per §5.4.2.27 a decoder need not act on the timecode.
    pub fn is_spec_valid(self) -> bool {
        self.hours() <= 23 && self.minutes() <= 59
    }
}

/// §5.4.2.28 base-syntax `timecod2` field — the **high-resolution**
/// half of the 28-bit SMPTE-style time code. Surfaced on
/// [`Bsi::timecod2`] only when the base syntax is in use (`bsid != 6`)
/// AND the encoder set `timecod2e == 1`.
///
/// The 14 wire bits split per §5.4.2.28 as `S S S . F F F F F . f f f
/// f f f` (MSB-first):
///
/// * 3-bit `seconds` field — valid range `0..=7`, the residual whole
///   seconds beyond the [`TimeCode1::eight_second_increments`]
///   quantum (i.e. `tc1.eight_second_increments·8 + tc2.seconds`
///   recovers the absolute second-within-minute).
/// * 5-bit `frames` field — valid range `0..=29` (assumes a 30 fps
///   reference per §5.4.2.26 "one frame = 1/30th of a second"; the
///   parser accepts codepoints up to 31).
/// * 6-bit `frame_fractions` field — valid range `0..=63`, each step
///   representing 1/64 of a frame.
///
/// The combined resolution is `1 / (30 × 64) ≈ 521 µs` and the
/// addressable range covers 8 seconds (the quantum of
/// [`TimeCode1::eight_second_increments`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeCode2 {
    raw: u16,
}

impl TimeCode2 {
    /// Wrap the 14-bit wire value verbatim. Only the low 14 bits of
    /// `raw` are consulted.
    pub fn from_raw(raw: u16) -> Self {
        Self { raw: raw & 0x3FFF }
    }

    /// Underlying 14-bit wire value — `SSS FFFFF ffffff` packed
    /// MSB-first.
    pub fn raw(self) -> u16 {
        self.raw
    }

    /// 3-bit `seconds` field, in `0..=7`. Combine with
    /// [`TimeCode1::eight_second_increments`] for the absolute
    /// second-within-minute (`tc1.eight_second_increments · 8 +
    /// tc2.seconds`).
    pub fn seconds(self) -> u8 {
        ((self.raw >> 11) & 0x7) as u8
    }

    /// 5-bit `frames` field, in `0..=31` (spec-valid range `0..=29`
    /// for the 30 fps reference assumed by §5.4.2.26).
    pub fn frames(self) -> u8 {
        ((self.raw >> 6) & 0x1F) as u8
    }

    /// 6-bit `frame_fractions` field, in `0..=63`. Each step represents
    /// 1/64 of a frame; at the 30 fps reference that is `≈ 521 µs`.
    pub fn frame_fractions(self) -> u8 {
        (self.raw & 0x3F) as u8
    }

    /// `true` when the `frames` field is inside its spec-documented
    /// 30 fps range (`≤ 29`). The 3-bit `seconds` and 6-bit
    /// `frame_fractions` fields cannot exceed their spec ranges.
    pub fn is_spec_valid(self) -> bool {
        self.frames() <= 29
    }
}

/// §5.4.2.26 Table 5.13 presence pattern for the
/// `timecod2e, timecod1e` pair. A receiver typically inspects
/// [`Bsi::timecode_presence`] before deciding whether to consult
/// [`Bsi::timecod1`] / [`Bsi::timecod2`].
///
/// Wire codes per Table 5.13:
///
/// * `(timecod2e=0, timecod1e=0)` → [`Self::NotPresent`]
/// * `(timecod2e=0, timecod1e=1)` → [`Self::FirstHalfOnly`]
/// * `(timecod2e=1, timecod1e=0)` → [`Self::SecondHalfOnly`]
/// * `(timecod2e=1, timecod1e=1)` → [`Self::BothHalves`]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeCodePresence {
    /// Neither half present — `timecod2e=0, timecod1e=0`.
    NotPresent,
    /// First (low-resolution) half only — `timecod2e=0, timecod1e=1`.
    /// Resolves coarse playback offset to 8-second granularity.
    FirstHalfOnly,
    /// Second (high-resolution) half only — `timecod2e=1, timecod1e=0`.
    /// Resolves to ≈ 521 µs but only within the implicit `0..=8 s`
    /// quantum — typically paired with out-of-band sync to pin the
    /// minute / hour position.
    SecondHalfOnly,
    /// Both halves present — `timecod2e=1, timecod1e=1`. Full 28-bit
    /// SMPTE-style timecode addressing 24 h at ≈ 521 µs resolution.
    BothHalves,
}

impl TimeCodePresence {
    /// Resolve the `(timecod2e, timecod1e)` pair into a presence
    /// pattern per Table 5.13. Only the low bit of each input is
    /// consulted.
    pub fn from_flags(timecod2e: bool, timecod1e: bool) -> Self {
        match (timecod2e, timecod1e) {
            (false, false) => TimeCodePresence::NotPresent,
            (false, true) => TimeCodePresence::FirstHalfOnly,
            (true, false) => TimeCodePresence::SecondHalfOnly,
            (true, true) => TimeCodePresence::BothHalves,
        }
    }
}

/// §5.4.2.24-25 distribution-control hint pair — the `copyrightb`
/// (Copyright Bit) + `origbs` (Original Bit Stream) flags. Both are
/// 1-bit fields placed back-to-back in every BSI's mandatory section
/// (§5.3.2 — they live just after the optional `audprodie` / `roomtyp2`
/// chain and just before the `timecod*e` / `xbsi*e` slots, with no
/// per-acmod gate).
///
/// Per spec text:
///
/// * `copyrightb == 1` — the bitstream is indicated as
///   copyright-protected (§5.4.2.24). `0` — not indicated as
///   protected.
/// * `origbs == 1` — this is an original bitstream (§5.4.2.25). `0` —
///   this is a copy of another bitstream.
///
/// The decoder does not act on either bit; surfacing them lets a chain
/// consumer enforce a distribution / archival policy (e.g. refuse to
/// re-encode a `copyrightb == 1` stream, or tag a `origbs == 0` copy
/// for downstream-only routing) without re-parsing the BSI.
///
/// On the Annex E (E-AC-3) side the same `copyrightb` / `origbs` pair
/// is carried inside the §E.2.3.1.62 informational-metadata block
/// (gated by `infomdate == 1`) and surfaces as
/// `eac3::Bsi::copyright_info` — see [`crate::eac3::bsi::Bsi`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CopyrightInfo {
    copyrightb: bool,
    origbs: bool,
}

impl CopyrightInfo {
    /// Build a [`CopyrightInfo`] from the raw 1-bit `copyrightb` /
    /// `origbs` values. Inputs are taken as booleans so the call site
    /// stays clean — the BSI parser passes the bit-shift result of
    /// each `read_u32(1)?` cast through `!= 0`.
    pub fn from_bits(copyrightb: bool, origbs: bool) -> Self {
        Self { copyrightb, origbs }
    }

    /// `true` when the encoder set the `copyrightb` bit
    /// (§5.4.2.24 — "the information in the bit stream is indicated as
    /// protected by copyright").
    pub fn is_copyright_protected(self) -> bool {
        self.copyrightb
    }

    /// `true` when the encoder set the `origbs` bit (§5.4.2.25 —
    /// "this is an original bit stream"). `false` indicates this is a
    /// copy of another bitstream.
    pub fn is_original_bitstream(self) -> bool {
        self.origbs
    }

    /// Raw 1-bit `copyrightb` codepoint, useful for re-emission /
    /// bit-exact mirroring of the wire field.
    pub fn copyrightb_bit(self) -> u8 {
        u8::from(self.copyrightb)
    }

    /// Raw 1-bit `origbs` codepoint, useful for re-emission /
    /// bit-exact mirroring of the wire field.
    pub fn origbs_bit(self) -> u8 {
        u8::from(self.origbs)
    }
}

/// §5.4.2.29-31 additional bit-stream information (`addbsi`) payload.
///
/// Carries between 1 and 64 bytes of encoder-defined trailing data,
/// gated on `addbsie == 1`. The bit-stream syntax (§5.3.2 / Table 5.1)
/// places the field right before the audio blocks at the end of
/// `bit_stream_info()`; on Annex E (E-AC-3) streams the same field
/// closes Table E1.2's BSI walk at the same logical position.
///
/// Per §5.4.2.30 — "the decoder is not required to interpret this
/// information, and thus shall skip over this number of bytes" — the
/// PCM decode is unchanged; surfacing the payload bytes lets a chain
/// consumer reach an encoder-private metadata block without
/// re-walking the BSI.
///
/// The wire format is:
///
/// ```text
///   addbsie    1 bit     // 1 = field present
///   if (addbsie) {
///     addbsil  6 bits    // 0..=63 ⇒ 1..=64 payload bytes
///     addbsi   (addbsil + 1) × 8 bits
///   }
/// ```
///
/// The payload bytes are stored verbatim in transmission order — bit 7
/// of the first byte is the bit immediately after the `addbsil` field.
/// The bit-stream cursor is not required to be byte-aligned at the
/// start of `addbsi` (and in practice rarely is, since `addbsi` follows
/// a 6-bit length field rather than padding); the bytes here are the
/// MSB-first bit-stream view in 8-bit groups, matching the wire-order
/// reads §5.4.2.31 prescribes.
///
/// The `addbsil` codepoint is preserved verbatim so a caller can
/// distinguish between an empty payload (`addbsil == 0`, payload = `[0]`
/// — a single byte) and a long payload at the codepoint endpoint
/// (`addbsil == 63`, payload = 64 bytes). The length-byte relationship
/// is `payload.len() == addbsil + 1`.
///
/// On the Annex E (E-AC-3) side the same payload is surfaced on
/// `eac3::Bsi::addbsi` — see [`crate::eac3::bsi::Bsi`] — using the same
/// type. The base + Annex E syntax tables (§5.3.2 / Table E1.2) carry
/// `addbsie + addbsil + addbsi` verbatim, so a single typed surface
/// covers both.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdditionalBitStreamInfo {
    addbsil: u8,
    payload: Vec<u8>,
}

impl AdditionalBitStreamInfo {
    /// Build an [`AdditionalBitStreamInfo`] from the raw 6-bit
    /// `addbsil` codepoint + the (addbsil + 1)-byte payload. Returns
    /// `None` when `addbsil >= 64` (the wire field is 6 bits, so any
    /// caller-supplied value above `63` is outside the codepoint range)
    /// or when `payload.len() != addbsil as usize + 1` (the spec is
    /// strict about the length-byte relationship — a violation here
    /// would not round-trip back through the bit-stream parser).
    pub fn from_addbsil_and_payload(addbsil: u8, payload: Vec<u8>) -> Option<Self> {
        if addbsil > 63 {
            return None;
        }
        if payload.len() != addbsil as usize + 1 {
            return None;
        }
        Some(Self { addbsil, payload })
    }

    /// Raw 6-bit `addbsil` codepoint (§5.4.2.30). Range `0..=63`,
    /// indicating `1..=64` payload bytes (the codepoint is the byte
    /// count *minus one*).
    pub fn addbsil(&self) -> u8 {
        self.addbsil
    }

    /// Number of payload bytes — `addbsil + 1`. Always within `1..=64`
    /// per the §5.4.2.30 codepoint range.
    pub fn len(&self) -> usize {
        self.addbsil as usize + 1
    }

    /// Convenience: `false` always per the spec — the field is at
    /// least 1 byte whenever it exists. Provided for Clippy
    /// `len_without_is_empty` and for caller idiomatic checks.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Borrowed view of the payload bytes in wire order (bit 7 of byte
    /// 0 is the bit immediately after `addbsil`).
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Total wire-field width in bits — `7 + 8 × (addbsil + 1)`. Useful
    /// for callers that need to mirror the BSI verbatim back into a
    /// bit-stream writer (6 bits for `addbsil` + 8 × payload bytes +
    /// 1 bit for the `addbsie` flag that gates the block).
    pub fn wire_bits(&self) -> u32 {
        7 + 8 * (self.addbsil as u32 + 1)
    }
}

/// Annex D §2.3.1.11-12 reserved-for-future-assignment + encoder-private
/// trailer of the `xbsi2` block. The §2.3 alternate bit-stream syntax
/// reserves 9 bits at the tail of the `xbsi2e == 1` block:
///
/// ```text
///   xbsi2    8 bits   // §2.3.1.11 — reserved for future assignment;
///                     //              encoders shall set to all 0s.
///   encinfo  1 bit    // §2.3.1.12 — reserved for encoder-private use;
///                     //              decoders do not interpret.
/// ```
///
/// Both fields sit definitionally after the §2.3.1.8-10 informational
/// metadata (`dsurexmod` / `dheadphonmod` / `adconvtyp`) inside the
/// `xbsi2e == 1` block — see §D Table D2.1 syntax. They are decoder
/// no-ops by spec but a conformant decoder still has to walk the bits,
/// and a chain consumer that re-emits the stream verbatim needs the raw
/// codepoints to round-trip the BSI without loss.
///
/// Surfacing the typed pair lets:
///
/// * a conformance probe verify that `xbsi2 == 0x00` per §2.3.1.11
///   (a non-zero codepoint flags either a future-spec extension or a
///   non-conformant encoder) without re-parsing the BSI,
/// * an encoder-watermark / encoder-identification consumer recover the
///   §2.3.1.12 `encinfo` bit without consulting a magic-number sentinel,
/// * a verbatim re-encoder route the BSI back into a bit-stream writer
///   bit-exactly without re-walking the wire.
///
/// `xbsi2` is preserved as a raw `u8` rather than parsed into a
/// codepoint enum — §2.3.1.11 deliberately leaves the bits unassigned,
/// so any partition would be premature. The single accessor
/// [`Self::is_spec_reserved_value`] flags whether the carried byte
/// matches the spec-conformance `0x00` value.
///
/// On the Annex E (E-AC-3) side this block does not exist — the Annex E
/// BSI never carries an `xbsi2e == 1` slot — so the typed surface stays
/// on the base BSI struct only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExtraBsi2 {
    xbsi2: u8,
    encinfo: bool,
}

impl ExtraBsi2 {
    /// Build an [`ExtraBsi2`] from the raw 8-bit `xbsi2` codepoint and
    /// the 1-bit `encinfo` flag. No validation — both fields are
    /// reserved (`xbsi2` for future assignment, `encinfo` for encoder
    /// private use) so any 8-bit + 1-bit combination is wire-legal.
    pub fn from_raw(xbsi2: u8, encinfo: bool) -> Self {
        Self { xbsi2, encinfo }
    }
    /// Raw 8-bit `xbsi2` codepoint (§2.3.1.11). Per spec encoders shall
    /// set this to `0x00`; a non-zero codepoint flags either a
    /// future-spec extension or a non-conformant encoder.
    pub fn xbsi2(&self) -> u8 {
        self.xbsi2
    }
    /// 1-bit `encinfo` flag (§2.3.1.12). Reserved for encoder-private
    /// use; the decoder does not interpret it.
    pub fn encinfo(&self) -> bool {
        self.encinfo
    }
    /// `true` when the carried `xbsi2` byte matches the spec-mandated
    /// `0x00` wire-conformance value (§2.3.1.11). Lets a probe / archive
    /// tool route streams that carry a non-conformant codepoint without
    /// re-parsing the BSI. The `encinfo` bit is excluded from the
    /// check — it is reserved for encoder-private use and any value is
    /// wire-legal.
    pub fn is_spec_reserved_value(&self) -> bool {
        self.xbsi2 == 0x00
    }
    /// Total wire-field width in bits — `9` (8 bits for `xbsi2` + 1 bit
    /// for `encinfo`). Useful for callers that need to mirror the BSI
    /// verbatim back into a bit-stream writer; does **not** include the
    /// gating `xbsi2e` flag that sits ahead of the informational
    /// metadata block.
    pub fn wire_bits(&self) -> u32 {
        9
    }
}

/// Annex D §2.3.1.3-6 alternate-syntax mix-level codewords. Each is a
/// 3-bit value; Tables D2.3 / D2.4 / D2.5 / D2.6 map them to linear
/// gains via [`annex_d_lt_rt_clev`] / [`annex_d_lt_rt_slev`] /
/// [`annex_d_lo_ro_clev`] / [`annex_d_lo_ro_slev`].
///
/// These supersede the body-spec 2-bit `cmixlev` / `surmixlev` defaults
/// for the LtRt / LoRo downmix targets specifically — the body fields
/// are still parsed (they sit ahead of the xbsi1 block in the bit
/// stream) but a §7.8 downmix on a `bsid == 6` Annex D stream should
/// prefer the Annex D refinements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnnexDMixLevels {
    /// `ltrtcmixlev` (Table D2.3). Defined for acmod ∈ {3, 5, 7}.
    pub ltrtcmixlev: u8,
    /// `ltrtsurmixlev` (Table D2.4). Codes 000..010 are reserved → use
    /// 0.841. Defined for acmod ∈ {4, 5, 6, 7}.
    pub ltrtsurmixlev: u8,
    /// `lorocmixlev` (Table D2.5). Defined for acmod ∈ {3, 5, 7}.
    pub lorocmixlev: u8,
    /// `lorosurmixlev` (Table D2.6). Codes 000..010 are reserved → use
    /// 0.841. Defined for acmod ∈ {4, 5, 6, 7}.
    pub lorosurmixlev: u8,
}

/// Map an Annex D 3-bit "center-channel" mix-level code to a linear
/// gain per Tables D2.3 / D2.5 (the two tables are identical).
///
/// Code → gain (dB):
///  `000` 1.414 (+3.0), `001` 1.189 (+1.5), `010` 1.000 ( 0.0),
///  `011` 0.841 (−1.5), `100` 0.707 (−3.0), `101` 0.595 (−4.5),
///  `110` 0.500 (−6.0), `111` 0.000 (−∞).
pub fn annex_d_center_mix_gain(code: u8) -> f32 {
    match code & 0x7 {
        0 => 1.414,
        1 => 1.189,
        2 => 1.000,
        3 => 0.841,
        4 => 0.707,
        5 => 0.595,
        6 => 0.500,
        _ => 0.000,
    }
}

/// Map an Annex D 3-bit "surround-channel" mix-level code to a linear
/// gain per Tables D2.4 / D2.6 (identical). Codes `000..010` are
/// reserved; per §2.3.1.4 / §2.3.1.6 the decoder shall substitute
/// 0.841 (the next defined code).
pub fn annex_d_surround_mix_gain(code: u8) -> f32 {
    match code & 0x7 {
        0..=3 => 0.841, // 000/001/010 reserved → 0.841; 011 = 0.841
        4 => 0.707,
        5 => 0.595,
        6 => 0.500,
        _ => 0.000,
    }
}

/// Parse the BSI starting at the beginning of `data`. The slice *must*
/// point at the byte immediately following `syncinfo` (i.e. byte 5 of
/// the syncframe).
///
/// On success the returned `Bsi` describes the stream and carries the
/// exact number of bits the parser consumed, so the caller can resume
/// an MSB-first `BitReader` at the right place for the first audio
/// block.
pub fn parse(data: &[u8]) -> Result<Bsi> {
    let mut br = BitReader::new(data);

    let bsid = br.read_u32(5)? as u8;
    let bsmod = br.read_u32(3)? as u8;
    let acmod = br.read_u32(3)? as u8;
    let nfchans = acmod_nfchans(acmod);

    // cmixlev — only present when there are 3 front channels, i.e.
    // the two LSBs of acmod include '1' for centre *and* acmod!=1
    // (the spec's "if ((acmod & 0x1) && (acmod != 0x1))" guard).
    let (cmixlev, center_mix) = if (acmod & 0x1) != 0 && acmod != 0x1 {
        let raw = br.read_u32(2)? as u8;
        (raw, Some(CenterMixLevel::from_code(raw)))
    } else {
        (0xFF, None)
    };

    // surmixlev — present when a surround channel exists (acmod & 0x4).
    let (surmixlev, surround_mix) = if (acmod & 0x4) != 0 {
        let raw = br.read_u32(2)? as u8;
        (raw, Some(SurroundMixLevel::from_code(raw)))
    } else {
        (0xFF, None)
    };

    // dsurmod — present only in 2/0 mode (acmod == 0x2).
    let (dsurmod, dolby_surround_mode) = if acmod == 0x2 {
        let raw = br.read_u32(2)? as u8;
        (raw, Some(DolbySurroundMode::from_code(raw)))
    } else {
        (0xFF, None)
    };

    let lfeon = br.read_u32(1)? != 0;
    let nchans = nfchans + u8::from(lfeon);

    let dialnorm_raw = br.read_u32(5)? as u8;
    // §5.4.2.8: dialnorm=0 is reserved; decoder shall use -31 dB.
    let dialnorm = if dialnorm_raw == 0 { 31 } else { dialnorm_raw };

    // Optional service metadata (§5.4.2.9 ff). `compr` is surfaced
    // (Table 7.30); `audprodie` carries the §5.4.2.13-15 mixing-room
    // hints and is surfaced as a typed [`AudioProductionInfo`]. The
    // §5.4.2.11-12 `langcod` slot — once a table-lookup language id,
    // now a wire-conformance reserved `0xFF` per the 2001 revision —
    // is surfaced as a typed [`LanguageCode`] so a probe / archive
    // tool can flag legacy non-conforming streams without re-parsing
    // the BSI.
    let compre = br.read_u32(1)? != 0;
    let compr = if compre {
        Some(CompressionGain::from_byte(br.read_u32(8)? as u8))
    } else {
        None
    };
    let langcode_flag = br.read_u32(1)? != 0;
    let language_code = if langcode_flag {
        Some(LanguageCode::from_raw(br.read_u32(8)? as u8))
    } else {
        None
    };
    let audprodie = br.read_u32(1)? != 0;
    let audio_production = if audprodie {
        let mixlevel = br.read_u32(5)? as u8;
        let roomtyp_raw = br.read_u32(2)? as u8;
        Some(AudioProductionInfo {
            mixlevel,
            roomtyp: RoomType::from_code(roomtyp_raw),
        })
    } else {
        None
    };

    // 1+1 mode (dual mono) carries a second copy of the metadata for Ch2.
    let (dialnorm_ch2, compr_ch2, language_code_ch2, audio_production_ch2) = if acmod == 0 {
        // §5.4.2.16 — dialnorm2 has the same meaning as dialnorm; the
        // `0` codepoint is reserved and remaps to `31` per §5.4.2.8.
        let dialnorm2_raw = br.read_u32(5)? as u8;
        let dialnorm2 = if dialnorm2_raw == 0 {
            31
        } else {
            dialnorm2_raw
        };
        let compr2e = br.read_u32(1)? != 0;
        let c2 = if compr2e {
            Some(CompressionGain::from_byte(br.read_u32(8)? as u8))
        } else {
            None
        };
        // §5.4.2.19-20 Ch2 `langcod2` slot — same reserved-`0xFF`
        // wire-conformance semantics as the Ch1 `langcod`.
        let langcod2e = br.read_u32(1)? != 0;
        let lc2 = if langcod2e {
            Some(LanguageCode::from_raw(br.read_u32(8)? as u8))
        } else {
            None
        };
        let audprodi2e = br.read_u32(1)? != 0;
        let ap2 = if audprodi2e {
            let mixlevel2 = br.read_u32(5)? as u8;
            let roomtyp2_raw = br.read_u32(2)? as u8;
            Some(AudioProductionInfo {
                mixlevel: mixlevel2,
                roomtyp: RoomType::from_code(roomtyp2_raw),
            })
        } else {
            None
        };
        (Some(dialnorm2), c2, lc2, ap2)
    } else {
        (None, None, None, None)
    };

    let copyrightb = br.read_u32(1)? != 0;
    let origbs = br.read_u32(1)? != 0;
    let copyright_info = CopyrightInfo::from_bits(copyrightb, origbs);

    // §5.3.2 base syntax has `timecod1e/timecod2e` here; Annex D
    // §2.3 / Table D2.1 reuses the same two 1+14-bit slots as
    // `xbsi1e/xbsi2e` and is identified by `bsid == 6` (§2.1).
    // Both shapes occupy the same fixed 30 bits maximum so the
    // surrounding parse is unchanged.
    let (
        annex_d_mix_levels,
        dmixmod,
        dmixmod_preference,
        dsurexmod,
        dheadphonmod,
        adconvtyp,
        extra_bsi,
        timecod1,
        timecod2,
        timecode_presence,
    ) = if bsid == 6 {
        // Annex D xbsi1 block.
        let xbsi1e = br.read_u32(1)? != 0;
        let (mix, dmm, dmm_pref) = if xbsi1e {
            let dmm = br.read_u32(2)? as u8;
            let ltrtc = br.read_u32(3)? as u8;
            let ltrts = br.read_u32(3)? as u8;
            let loroc = br.read_u32(3)? as u8;
            let loros = br.read_u32(3)? as u8;
            (
                Some(AnnexDMixLevels {
                    ltrtcmixlev: ltrtc,
                    ltrtsurmixlev: ltrts,
                    lorocmixlev: loroc,
                    lorosurmixlev: loros,
                }),
                dmm,
                Some(StereoDownmixPreference::from_code(dmm)),
            )
        } else {
            (None, 0xFFu8, None)
        };
        // xbsi2 block — §2.3.1.7-12. 14 bits total: dsurexmod(2) +
        // dheadphonmod(2) + adconvtyp(1) + xbsi2(8) + encinfo(1). The
        // last two are reserved-for-future-assignment / encoder-private
        // respectively; they are captured into [`ExtraBsi2`] so a chain
        // consumer can verify spec-conformance (`xbsi2 == 0x00`) and
        // recover the encoder-private `encinfo` bit without re-walking
        // the BSI.
        let xbsi2e = br.read_u32(1)? != 0;
        let (dsex, dhpm, adcv, xbsi2_tail) = if xbsi2e {
            let dsex_raw = br.read_u32(2)? as u8;
            let dhpm_raw = br.read_u32(2)? as u8;
            let adcv_raw = br.read_u32(1)? as u8;
            let xbsi2_raw = br.read_u32(8)? as u8;
            let encinfo_raw = br.read_u32(1)? != 0;
            (
                Some(DolbySurroundExMode::from_code(dsex_raw)),
                Some(DolbyHeadphoneMode::from_code(dhpm_raw)),
                Some(AdConverterType::from_code(adcv_raw)),
                Some(ExtraBsi2::from_raw(xbsi2_raw, encinfo_raw)),
            )
        } else {
            (None, None, None, None)
        };
        // Annex D syntax replaces both `timecod*` slots with
        // `xbsi*e` blocks — by definition the timecode is absent.
        (
            mix,
            dmm,
            dmm_pref,
            dsex,
            dhpm,
            adcv,
            xbsi2_tail,
            None,
            None,
            TimeCodePresence::NotPresent,
        )
    } else {
        // §5.3.2 base syntax — timecod1/timecod2 surfaced as typed
        // [`TimeCode1`] / [`TimeCode2`] when the encoder set the
        // respective `timecod*e` flag. Both halves are independently
        // gated per §5.4.2.26 Table 5.13.
        let timecod1e = br.read_u32(1)? != 0;
        let tc1 = if timecod1e {
            Some(TimeCode1::from_raw(br.read_u32(14)? as u16))
        } else {
            None
        };
        let timecod2e = br.read_u32(1)? != 0;
        let tc2 = if timecod2e {
            Some(TimeCode2::from_raw(br.read_u32(14)? as u16))
        } else {
            None
        };
        let presence = TimeCodePresence::from_flags(timecod2e, timecod1e);
        (
            None, 0xFFu8, None, None, None, None, None, tc1, tc2, presence,
        )
    };

    // addbsi — §5.4.2.29-31 trailer of 1..=64 encoder-defined bytes.
    // The decoder PCM path does not consult these bits ("the decoder is
    // not required to interpret this information") so the payload is
    // surfaced verbatim for chain consumers (encoder-private metadata,
    // OAMD packetisation, distribution-tagging) and the cursor is
    // advanced exactly `7 + 8 × (addbsil + 1)` bits.
    let addbsie = br.read_u32(1)? != 0;
    let addbsi = if addbsie {
        let addbsil = br.read_u32(6)? as u8; // 0..=63, meaning 1..=64 bytes
        let nbytes = addbsil as usize + 1;
        let mut payload = Vec::with_capacity(nbytes);
        for _ in 0..nbytes {
            payload.push(br.read_u32(8)? as u8);
        }
        // `from_addbsil_and_payload` returns `Some` here unconditionally
        // — the 6-bit field cannot exceed 63 and the payload length is
        // built to match `addbsil + 1` exactly — but route through the
        // safe constructor so the invariant is checked rather than
        // asserted.
        AdditionalBitStreamInfo::from_addbsil_and_payload(addbsil, payload)
    } else {
        None
    };

    let bits_consumed = br.bit_position();

    if bsid > 10 {
        // Per spec, base decoders mute for bsid > 8; we accept ≤10 as
        // a small safety margin for near-compatible streams and defer
        // a hard rejection to the decoder loop so probing still
        // succeeds.
        return Err(Error::Unsupported(format!(
            "ac3: bsid {bsid} > 8 — Annex E E-AC-3 bitstream needs a separate parser"
        )));
    }

    Ok(Bsi {
        bsid,
        bsmod,
        acmod,
        nfchans,
        lfeon,
        nchans,
        dialnorm,
        dialnorm_ch2,
        cmixlev,
        center_mix,
        surmixlev,
        surround_mix,
        dsurmod,
        dolby_surround_mode,
        annex_d_mix_levels,
        dmixmod,
        dmixmod_preference,
        compr,
        compr_ch2,
        language_code,
        language_code_ch2,
        dsurexmod,
        dheadphonmod,
        adconvtyp,
        extra_bsi,
        audio_production,
        audio_production_ch2,
        timecod1,
        timecod2,
        timecode_presence,
        copyright_info,
        addbsi,
        bits_consumed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal BSI byte sequence for 2/0 stereo, LFE off, no
    /// optional fields. acmod=2 → surmixlev/cmixlev absent, dsurmod
    /// present.
    ///
    ///   bsid=8 (5 bits)  : 0b01000
    ///   bsmod=0 (3 bits) : 0b000
    ///   acmod=2 (3 bits) : 0b010
    ///   dsurmod=0 (2)    : 0b00
    ///   lfeon=0 (1)      : 0
    ///   dialnorm=27 (5)  : 0b11011
    ///   compre=0         : 0
    ///   langcode=0       : 0
    ///   audprodie=0      : 0
    ///   copyrightb=0     : 0
    ///   origbs=0         : 0
    ///   timecod1e=0      : 0
    ///   timecod2e=0      : 0
    ///   addbsie=0        : 0
    ///
    /// Total = 5+3+3+2+1+5+1+1+1+1+1+1+1+1 = 27 bits → 4 bytes with 5
    /// trailing pad bits.
    #[test]
    fn parses_minimal_2_0_stereo_bsi() {
        // Build via a BitWriter-style manual pack.
        let bits: [(u8, u32); 14] = [
            (5, 0b01000),
            (3, 0b000),
            (3, 0b010),
            (2, 0b00),
            (1, 0),
            (5, 27),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
        ];
        let mut out = vec![0u8; 8];
        let mut bitpos = 0usize;
        for (n, v) in bits.iter().copied() {
            for i in (0..n).rev() {
                let bit = ((v >> i) & 1) as u8;
                let byte = bitpos / 8;
                let shift = 7 - (bitpos % 8);
                out[byte] |= bit << shift;
                bitpos += 1;
            }
        }

        let b = parse(&out).unwrap();
        assert_eq!(b.bsid, 8);
        assert_eq!(b.bsmod, 0);
        assert_eq!(b.acmod, 2);
        assert_eq!(b.nfchans, 2);
        assert!(!b.lfeon);
        assert_eq!(b.nchans, 2);
        assert_eq!(b.dialnorm, 27);
        assert_eq!(b.dsurmod, 0);
        assert_eq!(b.cmixlev, 0xFF);
        assert_eq!(b.surmixlev, 0xFF);
        assert!(b.annex_d_mix_levels.is_none());
        assert_eq!(b.dmixmod, 0xFF);
        assert_eq!(b.bits_consumed, bitpos as u64);
    }

    #[test]
    fn dialnorm_zero_remaps_to_31() {
        // bsid=8, bsmod=0, acmod=1 (1/0 mono — no cmix / surmix / dsurmod),
        // lfeon=0, dialnorm=0 → should remap.
        let bits: [(u8, u32); 11] = [
            (5, 8),
            (3, 0),
            (3, 1),
            (1, 0), // lfeon
            (5, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
        ];
        let mut last4 = vec![0u8; 8];
        let mut bitpos = 0usize;
        for (n, v) in bits.iter().copied() {
            for i in (0..n).rev() {
                let bit = ((v >> i) & 1) as u8;
                let byte = bitpos / 8;
                let shift = 7 - (bitpos % 8);
                last4[byte] |= bit << shift;
                bitpos += 1;
            }
        }
        // Need addbsie + timecodes bits too — add three trailing zero bits
        // to cover (timecod1e, timecod2e, addbsie) — wait, already in list.
        // Actually this packs 5+3+3+1+5+... re-count:
        //   5+3+3+1+5+1+1+1+1+1+1 = 23 bits. Missing nothing structural?
        // For acmod=1 there's no cmix, surmix, dsurmod. After lfeon and
        // dialnorm it goes compre/langcode/audprodie/copyrightb/origbs/
        // timecod1e/timecod2e/addbsie = 8 flags but we have 6. Add two
        // more zero bits so the addbsie fires false.
        let bits2: [(u8, u32); 2] = [(1, 0), (1, 0)];
        for (n, v) in bits2.iter().copied() {
            for i in (0..n).rev() {
                let bit = ((v >> i) & 1) as u8;
                let byte = bitpos / 8;
                let shift = 7 - (bitpos % 8);
                last4[byte] |= bit << shift;
                bitpos += 1;
            }
        }

        let b = parse(&last4).unwrap();
        assert_eq!(b.dialnorm, 31);
        assert_eq!(b.nfchans, 1);
        assert_eq!(b.nchans, 1);
    }

    /// Annex D §2 / Table D2.1 — `bsid == 6` activates the alternate
    /// syntax: the body's `timecod1e/timecod2e` slots become
    /// `xbsi1e/xbsi2e`. Verify the xbsi1 mix-level fields surface on
    /// [`Bsi::annex_d_mix_levels`] / [`Bsi::dmixmod`].
    #[test]
    fn parses_annex_d_bsid_6_xbsi1_mix_levels() {
        // 3/2 (acmod=7), lfe on. cmixlev = 0b00 (0.707), surmixlev = 0b00
        // (0.707). dialnorm=27. No compre / langcode / audprodie /
        // copyrightb / origbs. xbsi1e = 1 with:
        //   dmixmod        = 0b01 (LtRt preferred)
        //   ltrtcmixlev    = 0b011 (0.841)
        //   ltrtsurmixlev  = 0b100 (0.707)
        //   lorocmixlev    = 0b100 (0.707)
        //   lorosurmixlev  = 0b101 (0.595)
        // xbsi2e = 0. addbsie = 0.
        //
        // Bit layout:
        //   bsid=6 (5)         00110
        //   bsmod=0 (3)        000
        //   acmod=7 (3)        111
        //   cmixlev (2)        00
        //   surmixlev (2)      00
        //   lfeon (1)          1
        //   dialnorm (5)       11011
        //   compre (1)         0
        //   langcode (1)       0
        //   audprodie (1)      0
        //   copyrightb (1)     0
        //   origbs (1)         0
        //   xbsi1e (1)         1
        //   dmixmod (2)        01
        //   ltrtcmixlev (3)    011
        //   ltrtsurmixlev (3)  100
        //   lorocmixlev (3)    100
        //   lorosurmixlev (3)  101
        //   xbsi2e (1)         0
        //   addbsie (1)        0
        let bits: &[(u8, u32)] = &[
            (5, 6),
            (3, 0),
            (3, 7),
            (2, 0),
            (2, 0),
            (1, 1),
            (5, 27),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 1),
            (2, 0b01),
            (3, 0b011),
            (3, 0b100),
            (3, 0b100),
            (3, 0b101),
            (1, 0),
            (1, 0),
        ];
        let mut out = vec![0u8; 8];
        let mut bitpos = 0usize;
        for (n, v) in bits.iter().copied() {
            for i in (0..n).rev() {
                let bit = ((v >> i) & 1) as u8;
                let byte = bitpos / 8;
                let shift = 7 - (bitpos % 8);
                out[byte] |= bit << shift;
                bitpos += 1;
            }
        }

        let b = parse(&out).unwrap();
        assert_eq!(b.bsid, 6);
        assert_eq!(b.acmod, 7);
        assert!(b.lfeon);
        assert_eq!(b.dmixmod, 0b01);
        let mix = b.annex_d_mix_levels.expect("xbsi1 set → mix levels");
        assert_eq!(mix.ltrtcmixlev, 0b011);
        assert_eq!(mix.ltrtsurmixlev, 0b100);
        assert_eq!(mix.lorocmixlev, 0b100);
        assert_eq!(mix.lorosurmixlev, 0b101);
        assert_eq!(b.bits_consumed, bitpos as u64);
    }

    /// `bsid == 6` with `xbsi1e == 0` should leave the mix-level
    /// payload absent. The xbsi2e slot still needs to be consumed.
    #[test]
    fn parses_annex_d_bsid_6_no_xbsi1() {
        // 2/0 (acmod=2), no LFE. cmixlev absent (acmod & 1 == 0).
        // surmixlev absent (acmod & 4 == 0). dsurmod=0 (2 bits).
        // dialnorm=20. xbsi1e=0. xbsi2e=0. addbsie=0.
        let bits: &[(u8, u32)] = &[
            (5, 6),
            (3, 0),
            (3, 2),
            (2, 0),  // dsurmod
            (1, 0),  // lfeon
            (5, 20), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 0),  // copyrightb
            (1, 0),  // origbs
            (1, 0),  // xbsi1e
            (1, 0),  // xbsi2e
            (1, 0),  // addbsie
        ];
        let mut out = vec![0u8; 8];
        let mut bitpos = 0usize;
        for (n, v) in bits.iter().copied() {
            for i in (0..n).rev() {
                let bit = ((v >> i) & 1) as u8;
                let byte = bitpos / 8;
                let shift = 7 - (bitpos % 8);
                out[byte] |= bit << shift;
                bitpos += 1;
            }
        }

        let b = parse(&out).unwrap();
        assert_eq!(b.bsid, 6);
        assert!(b.annex_d_mix_levels.is_none());
        assert_eq!(b.dmixmod, 0xFF);
        assert_eq!(b.bits_consumed, bitpos as u64);
    }

    /// Table D2.3 / D2.5 — the 3-bit center mix-level codewords map to
    /// the exact gains the spec tabulates.
    #[test]
    fn annex_d_center_mix_gain_matches_table_d2_3() {
        let expected: [(u8, f32); 8] = [
            (0b000, 1.414),
            (0b001, 1.189),
            (0b010, 1.000),
            (0b011, 0.841),
            (0b100, 0.707),
            (0b101, 0.595),
            (0b110, 0.500),
            (0b111, 0.000),
        ];
        for (code, gain) in expected {
            assert!(
                (annex_d_center_mix_gain(code) - gain).abs() < 1e-6,
                "code 0b{code:03b}: want {gain}, got {}",
                annex_d_center_mix_gain(code)
            );
        }
    }

    /// Table D2.4 / D2.6 — the 3-bit surround mix-level codewords. The
    /// reserved codes `000/001/010` substitute 0.841 per spec.
    #[test]
    fn annex_d_surround_mix_gain_substitutes_reserved_with_0_841() {
        // Reserved codes all map to 0.841.
        for code in 0u8..=2 {
            let g = annex_d_surround_mix_gain(code);
            assert!(
                (g - 0.841).abs() < 1e-6,
                "reserved code 0b{code:03b} should resolve to 0.841, got {g}"
            );
        }
        let expected: [(u8, f32); 5] = [
            (0b011, 0.841),
            (0b100, 0.707),
            (0b101, 0.595),
            (0b110, 0.500),
            (0b111, 0.000),
        ];
        for (code, gain) in expected {
            assert!(
                (annex_d_surround_mix_gain(code) - gain).abs() < 1e-6,
                "code 0b{code:03b}: want {gain}, got {}",
                annex_d_surround_mix_gain(code)
            );
        }
    }

    /// Table 5.7 — every `bsmod` codepoint except `0b111` resolves to a
    /// fixed service type independent of `acmod`. Spot-check each row
    /// with a couple of `acmod` values to confirm the resolver doesn't
    /// peek at `acmod` when `bsmod != 0b111`.
    #[test]
    fn bsmod_table_5_7_fixed_codepoints() {
        use BitStreamMode::*;
        let rows: [(u8, BitStreamMode); 7] = [
            (0b000, CompleteMain),
            (0b001, MusicAndEffects),
            (0b010, VisuallyImpaired),
            (0b011, HearingImpaired),
            (0b100, Dialogue),
            (0b101, Commentary),
            (0b110, Emergency),
        ];
        for (bsmod, want) in rows {
            for acmod in 0u8..=7 {
                let got = BitStreamMode::from_bsmod_acmod(bsmod, acmod);
                assert_eq!(
                    got, want,
                    "bsmod=0b{bsmod:03b} acmod=0b{acmod:03b}: want {want:?}, got {got:?}"
                );
            }
        }
    }

    /// Table 5.7 — `bsmod==0b111` is overloaded: acmod=0b001 → VoiceOver,
    /// acmod ∈ {0b010..=0b111} → Karaoke, acmod=0b000 (the 1+1 dual-mono
    /// slot) → Reserved (no Table 5.7 row defines it).
    #[test]
    fn bsmod_0b111_resolves_with_acmod() {
        assert_eq!(
            BitStreamMode::from_bsmod_acmod(0b111, 0b000),
            BitStreamMode::Reserved
        );
        assert_eq!(
            BitStreamMode::from_bsmod_acmod(0b111, 0b001),
            BitStreamMode::VoiceOver
        );
        for acmod in 0b010u8..=0b111 {
            assert_eq!(
                BitStreamMode::from_bsmod_acmod(0b111, acmod),
                BitStreamMode::Karaoke,
                "acmod=0b{acmod:03b}"
            );
        }
    }

    /// `is_main` / `is_associated` partition Table 5.7 cleanly. CM, ME,
    /// and karaoke are main; VI/HI/D/C/E/VO are associated; the unused
    /// `bsmod=0b111 acmod=0b000` cell is neither.
    #[test]
    fn main_vs_associated_partition() {
        use BitStreamMode::*;
        let main = [CompleteMain, MusicAndEffects, Karaoke];
        let assoc = [
            VisuallyImpaired,
            HearingImpaired,
            Dialogue,
            Commentary,
            Emergency,
            VoiceOver,
        ];
        for m in main {
            assert!(m.is_main(), "{m:?} should be main");
            assert!(!m.is_associated(), "{m:?} should not be associated");
        }
        for a in assoc {
            assert!(a.is_associated(), "{a:?} should be associated");
            assert!(!a.is_main(), "{a:?} should not be main");
        }
        assert!(!Reserved.is_main());
        assert!(!Reserved.is_associated());
    }

    /// Mnemonics are stable per Table 5.7 — used in CLI / log output.
    /// "?" is reserved for the Reserved case so downstream code can
    /// rely on a single sentinel for "no service type".
    #[test]
    fn mnemonics_are_table_5_7_short_forms() {
        use BitStreamMode::*;
        let rows: [(BitStreamMode, &str); 10] = [
            (CompleteMain, "CM"),
            (MusicAndEffects, "ME"),
            (VisuallyImpaired, "VI"),
            (HearingImpaired, "HI"),
            (Dialogue, "D"),
            (Commentary, "C"),
            (Emergency, "E"),
            (VoiceOver, "VO"),
            (Karaoke, "K"),
            (Reserved, "?"),
        ];
        for (mode, mnem) in rows {
            assert_eq!(mode.mnemonic(), mnem, "{mode:?}");
        }
    }

    /// `Bsi::service_type()` round-trips the raw bsmod/acmod into the
    /// typed enum. Reuses the minimal 2/0 stereo fixture (acmod=2,
    /// bsmod=0) and a custom 1/0 mono bsmod=0b111 builder to cover
    /// both the simple and overloaded branches end-to-end through the
    /// `Bsi` accessor.
    #[test]
    fn bsi_service_type_accessor_routes_through_table_5_7() {
        // The minimal 2/0 stereo fixture sets bsmod=0, acmod=2 →
        // CompleteMain. Re-built locally so the test stays
        // self-contained.
        let stereo_bits: [(u8, u32); 14] = [
            (5, 0b01000),
            (3, 0b000),
            (3, 0b010),
            (2, 0b00),
            (1, 0),
            (5, 27),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
        ];
        let stereo_bytes = pack_bits(&stereo_bits);
        let bsi = parse(&stereo_bytes).expect("parse minimal 2/0");
        assert_eq!(bsi.bsmod, 0b000);
        assert_eq!(bsi.acmod, 0b010);
        assert_eq!(bsi.service_type(), BitStreamMode::CompleteMain);

        // 1/0 mono BSI with bsmod=0b111 + acmod=0b001 → VoiceOver.
        // acmod=1 means no cmix / no surmix / no dsurmod optional fields.
        //   bsid=8       (5)  : 0b01000
        //   bsmod=0b111  (3)  : 0b111
        //   acmod=0b001  (3)  : 0b001
        //   lfeon=0      (1)  : 0
        //   dialnorm=27  (5)  : 0b11011
        //   compre=0     (1)  : 0
        //   langcode=0   (1)  : 0
        //   audprodie=0  (1)  : 0
        //   copyrightb=0 (1)  : 0
        //   origbs=0     (1)  : 0
        //   timecod1e=0  (1)  : 0
        //   timecod2e=0  (1)  : 0
        //   addbsie=0    (1)  : 0
        let voiceover_bits: [(u8, u32); 13] = [
            (5, 0b01000),
            (3, 0b111),
            (3, 0b001),
            (1, 0),
            (5, 27),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
        ];
        let voiceover_bytes = pack_bits(&voiceover_bits);
        let bsi = parse(&voiceover_bytes).expect("parse 1/0 voiceover");
        assert_eq!(bsi.bsmod, 0b111);
        assert_eq!(bsi.acmod, 0b001);
        assert_eq!(bsi.service_type(), BitStreamMode::VoiceOver);
    }

    /// MSB-first bit packer matching the AC-3 `BitReader` order — used
    /// by the Table 5.7 service-type tests to build synthetic BSIs.
    fn pack_bits(bits: &[(u8, u32)]) -> Vec<u8> {
        let total_bits: usize = bits.iter().map(|(n, _)| *n as usize).sum();
        let mut out = vec![0u8; total_bits.div_ceil(8) + 1];
        let mut bitpos = 0usize;
        for (n, v) in bits.iter().copied() {
            for i in (0..n).rev() {
                let bit = ((v >> i) & 1) as u8;
                let byte = bitpos / 8;
                let shift = 7 - (bitpos % 8);
                out[byte] |= bit << shift;
                bitpos += 1;
            }
        }
        out
    }

    // ---------------------------------------------------------------
    // Heavy compression gain (`compr`) — Table 7.30 / §7.7.2.2.
    // ---------------------------------------------------------------

    /// X is a 4-bit signed integer with values in `-8..=+7`. Walk every
    /// `X` codepoint with `Y = 0b1111` (max-Y) and assert the decoded
    /// `(X, Y)` round-trip matches the bit layout described in §7.7.2.2.
    #[test]
    fn compression_gain_x_field_sign_extends_correctly() {
        // (raw_x_nibble, expected signed value) — every codepoint.
        let cases = [
            (0b0000u8, 0i8),
            (0b0001, 1),
            (0b0010, 2),
            (0b0011, 3),
            (0b0100, 4),
            (0b0101, 5),
            (0b0110, 6),
            (0b0111, 7),
            (0b1000, -8),
            (0b1001, -7),
            (0b1010, -6),
            (0b1011, -5),
            (0b1100, -4),
            (0b1101, -3),
            (0b1110, -2),
            (0b1111, -1),
        ];
        for (xn, x) in cases {
            // Y = 0b1010 (arbitrary) — verify X decoding is independent of Y.
            let cg = CompressionGain::from_byte((xn << 4) | 0b1010);
            assert_eq!(cg.x(), x, "X mismatch for raw nibble {xn:#06b}");
            assert_eq!(cg.y(), 0b1010);
            assert_eq!(cg.raw(), (xn << 4) | 0b1010);
        }
    }

    /// Table 7.30 row checks: the dB gain of each `(X, Y=0)` codepoint
    /// must match the table's "Gain Indicated" column to within 0.005 dB.
    /// At `Y=0`, the contribution from `Y` is exactly `-6.02 dB`, so the
    /// table's "X alone = (X+1)*6.02 dB" sums with the Y attenuation to
    /// `linear = 2^(X+1) * 0.5`, i.e. `(X+1)*6.02 - 6.02 = X*6.02 dB`.
    /// Therefore the dB at `Y=0` equals `X * 6.02` (Table 7.30 minus
    /// 6.02 dB across the board).
    ///
    /// Equivalently the table's headline rows (e.g. `X=7 → +48.16 dB`)
    /// describe the X contribution **without** the Y attenuation; the
    /// effective decoder gain when `Y = 0b1111` (`(16+15)/32 = 31/32 ≈
    /// -0.28 dB`) drops the headline by 0.276 dB. This test checks both
    /// the headline (max-Y) and the bottom (Y=0) of every X row.
    #[test]
    fn compression_gain_table_7_30_db_endpoints() {
        // (X, Y=15 dB ≈ headline - 0.276; Y=0 dB = headline - 6.02).
        let cases = [
            (7i8, 48.16f32),
            (6, 42.14),
            (5, 36.12),
            (4, 30.10),
            (3, 24.08),
            (2, 18.06),
            (1, 12.04),
            (0, 6.02),
            (-1, 0.0),
            (-2, -6.02),
            (-3, -12.04),
            (-4, -18.06),
            (-5, -24.08),
            (-6, -30.10),
            (-7, -36.12),
            (-8, -42.14),
        ];
        for (x, headline_db) in cases {
            // Pack X into the upper nibble (two's-complement 4-bit).
            let xn = (x as i16 & 0xF) as u8;
            // Y = 0b1111 → top of row, dB ≈ headline - 0.276.
            let max_y = CompressionGain::from_byte((xn << 4) | 0b1111);
            let max_y_db = max_y.decibels();
            assert!(
                (max_y_db - (headline_db - 0.276)).abs() < 0.01,
                "X={x} Y=15: got {max_y_db:.3} dB, want {:.3} dB",
                headline_db - 0.276
            );
            // Y = 0b0000 → bottom of row, dB = headline - 6.02.
            let min_y = CompressionGain::from_byte(xn << 4);
            let min_y_db = min_y.decibels();
            assert!(
                (min_y_db - (headline_db - 6.02)).abs() < 0.01,
                "X={x} Y=0: got {min_y_db:.3} dB, want {:.3} dB",
                headline_db - 6.02
            );
        }
    }

    /// Y is a 4-bit unsigned mantissa with an implicit leading 1, read
    /// as `(16 + Y) / 32`. Spot-check the four boundary values per
    /// §7.7.2.2 ("Y can represent values between 0.111112 (or 31/32) and
    /// 0.100002 (or 1/2)").
    #[test]
    fn compression_gain_y_field_is_fractional_with_leading_one() {
        // With X = -1 (= 0b1111, gain = 0 dB), linear = 1.0 * (16+Y)/32.
        let cases = [
            (0u8, 16.0 / 32.0), // 0.5
            (1, 17.0 / 32.0),   // 0.53125
            (15, 31.0 / 32.0),  // 0.96875
            (8, 24.0 / 32.0),   // 0.75
        ];
        for (y, expected) in cases {
            let cg = CompressionGain::from_byte(0b1111_0000 | y);
            let lin = cg.linear();
            assert!(
                (lin - expected).abs() < 1e-6,
                "X=-1 Y={y}: got linear={lin}, want {expected}"
            );
        }
    }

    /// Combined-range sanity per §7.7.2.2:
    /// "The combination of X and Y values allows compr to indicate gain
    /// changes from 48.16 – 0.28 = +47.89 dB, to –42.14 – 6.02 =
    /// –48.16 dB."
    #[test]
    fn compression_gain_extreme_codepoints_match_spec_range() {
        let top = CompressionGain::from_byte(0b0111_1111); // X=7, Y=15
        let bottom = CompressionGain::from_byte(0b1000_0000); // X=-8, Y=0

        assert_eq!(top.x(), 7);
        assert_eq!(top.y(), 15);
        // Linear = 2^8 * 31/32 = 248.
        assert!((top.linear() - 248.0).abs() < 1e-3);
        // dB = 20*log10(248) ≈ +47.884 dB.
        assert!((top.decibels() - 47.884).abs() < 0.01);

        assert_eq!(bottom.x(), -8);
        assert_eq!(bottom.y(), 0);
        // Linear = 2^-7 * 0.5 = 1/256.
        assert!((bottom.linear() - 1.0 / 256.0).abs() < 1e-6);
        // dB = 20*log10(1/256) ≈ -48.165 dB.
        assert!((bottom.decibels() - (-48.165)).abs() < 0.01);
    }

    /// `parse()` surfaces `compr` as `Some(CompressionGain)` when the
    /// `compre` flag is set, and `None` otherwise. Build a 1/0 mono
    /// BSI with `compre=1` and `compr=0b0001_0000` (X=1, Y=0, linear
    /// `2^2 * 0.5 = 2.0`, ≈ +6.02 dB), then verify the parser routes
    /// the byte verbatim into the typed surface.
    #[test]
    fn parse_surfaces_compr_when_compre_set() {
        // 1/0 mono (acmod=1) → no cmixlev / surmixlev / dsurmod.
        // bsid=8, bsmod=0, acmod=1, lfeon=0, dialnorm=27,
        //   compre=1, compr=0b0001_0000, langcode=0, audprodie=0,
        //   copyrightb=0, origbs=0, timecod1e=0, timecod2e=0,
        //   addbsie=0.
        let bits: [(u8, u32); 13] = [
            (5, 8),
            (3, 0),
            (3, 1),
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 1),  // compre
            (8, 0b0001_0000),
            (1, 0), // langcode
            (1, 0), // audprodie
            (1, 0), // copyrightb
            (1, 0), // origbs
            (1, 0), // timecod1e
            (1, 0), // timecod2e + addbsie folded as separate bits below
        ];
        let mut bytes = pack_bits(&bits);
        // Append one more zero bit for addbsie.
        bytes.push(0);
        let bsi = parse(&bytes).unwrap();
        assert_eq!(bsi.acmod, 1);
        let cg = bsi.compr.expect("compre=1 should surface compr");
        assert_eq!(cg.raw(), 0b0001_0000);
        assert_eq!(cg.x(), 1);
        assert_eq!(cg.y(), 0);
        assert!((cg.linear() - 2.0).abs() < 1e-6);
        // 1+1 mode is acmod==0; for acmod==1 the Ch2 word stays None.
        assert!(bsi.compr_ch2.is_none());
    }

    /// `parse()` leaves `compr` as `None` when `compre == 0`.
    #[test]
    fn parse_leaves_compr_none_when_compre_clear() {
        // Reuse the minimal 2/0 BSI from `parses_minimal_2_0_stereo_bsi`
        // — it has compre=0 by construction.
        let bits: [(u8, u32); 14] = [
            (5, 0b01000),
            (3, 0b000),
            (3, 0b010),
            (2, 0b00),
            (1, 0),
            (5, 27),
            (1, 0), // compre
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
        ];
        let bytes = pack_bits(&bits);
        let bsi = parse(&bytes).unwrap();
        assert!(bsi.compr.is_none());
        assert!(bsi.compr_ch2.is_none());
    }

    /// 1+1 dual-mono (`acmod == 0`) carries a second `compr2` word for
    /// Ch2 with identical Table 7.30 semantics per §5.4.2.18 ("This
    /// 8-bit word has the same meaning as compr, except that it applies
    /// to the second audio channel"). Build a 1+1 BSI with `compre=1`
    /// (X=-1, Y=15 ≈ -0.276 dB on Ch1) and `compr2e=1` (X=-8, Y=0 ≈
    /// -48.16 dB on Ch2) and verify both surface independently.
    #[test]
    fn parse_surfaces_compr_ch2_in_dual_mono() {
        // acmod=0 (1+1 dual mono): no cmix/surmix/dsurmod, lfeon possible.
        //   bsid=8, bsmod=0, acmod=0, lfeon=0, dialnorm=27,
        //     compre=1, compr=0b1111_1111,
        //     langcode=0, audprodie=0,
        //   /* 1+1 second block */
        //     dialnorm2=27, compr2e=1, compr2=0b1000_0000,
        //     langcod2e=0, audprodi2e=0,
        //   copyrightb=0, origbs=0, timecod1e=0, timecod2e=0, addbsie=0.
        let bits: [(u8, u32); 18] = [
            (5, 8),
            (3, 0),
            (3, 0),
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 1),  // compre
            (8, 0b1111_1111),
            (1, 0), // langcode
            (1, 0), // audprodie
            (5, 27),
            (1, 1), // compr2e
            (8, 0b1000_0000),
            (1, 0), // langcod2e
            (1, 0), // audprodi2e
            (1, 0), // copyrightb
            (1, 0), // origbs
            (1, 0), // timecod1e
            (1, 0), // timecod2e
        ];
        let mut bytes = pack_bits(&bits);
        bytes.push(0); // addbsie + pad
        let bsi = parse(&bytes).unwrap();
        assert_eq!(bsi.acmod, 0);
        let c1 = bsi.compr.expect("compre=1");
        assert_eq!(c1.raw(), 0b1111_1111);
        assert!((c1.decibels() - (-0.276)).abs() < 0.01);
        let c2 = bsi.compr_ch2.expect("compr2e=1");
        assert_eq!(c2.raw(), 0b1000_0000);
        assert!((c2.decibels() - (-48.165)).abs() < 0.01);
    }

    // ---------------------------------------------------------------
    // §5.4.2.8 / §5.4.2.16 — dialogue normalization typed surface.
    // ---------------------------------------------------------------

    /// Every legal wire codepoint `1..=31` maps to itself unchanged
    /// and to `-N dB` per §5.4.2.8 ("interpreted as -1 dB to -31 dB").
    #[test]
    fn dialnorm_decodes_every_legal_wire_codepoint() {
        for wire in 1u8..=31u8 {
            let dn = DialNorm::from_wire(wire);
            assert_eq!(dn.codepoint(), wire);
            assert_eq!(dn.wire_value(), wire);
            assert!(!dn.is_reserved_wire_codepoint());
            assert_eq!(dn.db(), -(wire as i8));
            assert_eq!(dn.level_below_full_scale_db(), wire);
        }
    }

    /// The reserved `0` wire codepoint remaps to `31` per §5.4.2.8
    /// ("If the reserved value of 0 is received, the decoder shall
    /// use -31 dB"). The remap is observable via
    /// [`DialNorm::is_reserved_wire_codepoint`] so a careful consumer
    /// can distinguish a legitimate `31` codepoint from the reserved-
    /// remap path; [`DialNorm::wire_value`] recovers the original `0`
    /// for byte-exact re-emission.
    #[test]
    fn dialnorm_zero_wire_codepoint_remaps_to_31_with_reserved_flag() {
        let dn = DialNorm::from_wire(0);
        assert_eq!(dn.codepoint(), 31);
        assert_eq!(dn.wire_value(), 0);
        assert!(dn.is_reserved_wire_codepoint());
        assert_eq!(dn.db(), -31);
        assert_eq!(dn.level_below_full_scale_db(), 31);
        // A "real" 31 codepoint reports the same dB / codepoint but is
        // distinguishable from the reserved path.
        let dn31 = DialNorm::from_wire(31);
        assert_eq!(dn31.codepoint(), 31);
        assert_eq!(dn31.wire_value(), 31);
        assert!(!dn31.is_reserved_wire_codepoint());
        assert_ne!(dn, dn31);
    }

    /// Bits above the 5-bit field are masked off — `from_wire` consumes
    /// only the low 5 bits per the BSI bit-reader contract.
    #[test]
    fn dialnorm_only_consumes_low_5_bits() {
        let dn = DialNorm::from_wire(0b1110_1011); // low5=01011=11
        assert_eq!(dn.codepoint(), 11);
        assert_eq!(dn.db(), -11);
    }

    /// Linear attenuation matches `10^(dB/20)` per the standard
    /// dB-to-linear conversion. `-1 dB` ≈ 0.8913, `-31 dB` ≈ 0.02818,
    /// `-25 dB` ≈ 0.05623.
    #[test]
    fn dialnorm_attenuation_linear_matches_dbgain() {
        let cases: [(u8, f32); 3] = [(1, 0.8913), (25, 0.0562), (31, 0.0282)];
        for (wire, expected) in cases {
            let got = DialNorm::from_wire(wire).attenuation_linear();
            assert!(
                (got - expected).abs() < 1e-3,
                "wire={wire}: got {got}, expected {expected}"
            );
        }
    }

    /// §7.6 worked example — listener target 67 dB SPL, reference
    /// full-scale 105 dB SPL, dialnorm = -25 dB → playback gain
    /// `67 - 105 + 25 = -13 dB` (so full-scale digital reproduces at
    /// `105 - 13 = 92 dB SPL`, matching the spec text "full scale
    /// digital signals reproduce at a sound pressure level of 92 dB").
    /// Linear gain `10^(-13/20) ≈ 0.2239`.
    #[test]
    fn dialnorm_reproduction_gain_matches_spec_7_6_example() {
        let dn = DialNorm::from_wire(25);
        let gain = dn.reproduction_gain_linear(67.0, 105.0);
        let expected = 10.0f32.powf(-13.0 / 20.0);
        assert!(
            (gain - expected).abs() < 1e-4,
            "got {gain}, expected {expected}"
        );
    }

    /// `parse()` surfaces [`Bsi::dialnorm`] as the post-remap u8 (kept
    /// for backward compatibility) AND exposes the typed
    /// [`Bsi::dialogue_normalization`] accessor. The typed view loses
    /// the reserved-wire-codepoint distinction since the post-remap
    /// `dialnorm` field has already collapsed it; callers needing the
    /// raw codepoint check the field directly.
    #[test]
    fn parse_surfaces_dialogue_normalization_accessor() {
        // 2/0 stereo, dialnorm=20, all optional metadata off.
        let bits: [(u8, u32); 14] = [
            (5, 8),  // bsid
            (3, 0),  // bsmod
            (3, 2),  // acmod=2 (2/0 stereo)
            (2, 0),  // dsurmod
            (1, 0),  // lfeon
            (5, 20), // dialnorm = -20 dB
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 0),  // copyrightb
            (1, 0),  // origbs
            (1, 0),  // timecod1e
            (1, 0),  // timecod2e
            (1, 0),  // addbsie
        ];
        let mut bytes = pack_bits(&bits);
        bytes.push(0);
        let bsi = parse(&bytes).unwrap();
        assert_eq!(bsi.dialnorm, 20);
        assert!(bsi.dialnorm_ch2.is_none());
        let dn = bsi.dialogue_normalization();
        assert_eq!(dn.codepoint(), 20);
        assert_eq!(dn.db(), -20);
        assert_eq!(dn.level_below_full_scale_db(), 20);
        assert!(bsi.dialogue_normalization_ch2().is_none());
    }

    /// 1+1 dual-mono (`acmod == 0`) carries a second `dialnorm2` word
    /// for Ch2 with identical §5.4.2.8 semantics per §5.4.2.16
    /// ("This 5-bit code has the same meaning as dialnorm"). Build a
    /// 1+1 BSI with Ch1 dialnorm=27 (-27 dB) and Ch2 dialnorm2=11
    /// (-11 dB) and verify both surface independently — Ch2 via the
    /// new `dialnorm_ch2` field + `dialogue_normalization_ch2()`
    /// accessor.
    #[test]
    fn parse_surfaces_dialnorm_ch2_in_dual_mono() {
        let bits: [(u8, u32); 18] = [
            (5, 8),
            (3, 0),
            (3, 0),  // acmod=0 (1+1 dual mono)
            (1, 0),  // lfeon
            (5, 27), // dialnorm = -27 dB
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (5, 11), // dialnorm2 = -11 dB
            (1, 0),  // compr2e
            (1, 0),  // langcod2e
            (1, 0),  // audprodi2e
            (1, 0),  // copyrightb
            (1, 0),  // origbs
            (1, 0),  // timecod1e
            (1, 0),  // timecod2e
            (1, 0),  // addbsie
            (1, 0),  // pad
        ];
        let mut bytes = pack_bits(&bits);
        bytes.push(0);
        let bsi = parse(&bytes).unwrap();
        assert_eq!(bsi.acmod, 0);
        assert_eq!(bsi.dialnorm, 27);
        let dn_ch2 = bsi
            .dialnorm_ch2
            .expect("acmod == 0 should surface dialnorm_ch2");
        assert_eq!(dn_ch2, 11);
        let typed = bsi
            .dialogue_normalization_ch2()
            .expect("dialnorm_ch2 surfaced");
        assert_eq!(typed.codepoint(), 11);
        assert_eq!(typed.db(), -11);
        // Ch1 surface is independent.
        assert_eq!(bsi.dialogue_normalization().codepoint(), 27);
    }

    /// 1+1 dual-mono Ch2 with the reserved `dialnorm2 = 0` wire
    /// codepoint remaps to `31` per §5.4.2.8 (reused by §5.4.2.16),
    /// matching the Ch1 remap. The post-remap `31` is what the parser
    /// stores; the wire-reserved-bit distinction is only available
    /// via `DialNorm::from_wire(0)` on a freshly built value, not via
    /// the BSI surface (since the BSI field is the remapped value
    /// only — same shape as Ch1's existing `dialnorm: u8`).
    #[test]
    fn parse_remaps_dialnorm2_zero_codepoint_to_31() {
        let bits: [(u8, u32); 18] = [
            (5, 8),
            (3, 0),
            (3, 0), // acmod=0
            (1, 0),
            (5, 27), // dialnorm = -27 dB (legitimate)
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (5, 0),  // dialnorm2 = reserved 0 → remaps to 31
            (1, 0),  // compr2e
            (1, 0),  // langcod2e
            (1, 0),  // audprodi2e
            (1, 0),  // copyrightb
            (1, 0),  // origbs
            (1, 0),  // timecod1e
            (1, 0),  // timecod2e
            (1, 0),  // addbsie
            (1, 0),
        ];
        let mut bytes = pack_bits(&bits);
        bytes.push(0);
        let bsi = parse(&bytes).unwrap();
        let dn_ch2 = bsi.dialnorm_ch2.expect("acmod == 0");
        assert_eq!(dn_ch2, 31);
        let typed = bsi
            .dialogue_normalization_ch2()
            .expect("dialnorm_ch2 surfaced");
        assert_eq!(typed.codepoint(), 31);
        assert_eq!(typed.db(), -31);
    }

    /// Non-1+1 streams (`acmod != 0`) never carry `dialnorm2` per
    /// §5.4.2.16's "applies to the second audio channel when acmod
    /// indicates two independent channels (dual mono 1+1 mode)". The
    /// `dialnorm_ch2` field stays `None` for every other `acmod`.
    #[test]
    fn parse_leaves_dialnorm_ch2_none_outside_dual_mono() {
        // 2/0 stereo (acmod=2) baseline.
        let bits: [(u8, u32); 14] = [
            (5, 8),
            (3, 0),
            (3, 2), // acmod=2
            (2, 0), // dsurmod
            (1, 0), // lfeon
            (5, 27),
            (1, 0), // compre
            (1, 0), // langcode
            (1, 0), // audprodie
            (1, 0), // copyrightb
            (1, 0), // origbs
            (1, 0), // timecod1e
            (1, 0), // timecod2e
            (1, 0), // addbsie
        ];
        let mut bytes = pack_bits(&bits);
        bytes.push(0);
        let bsi = parse(&bytes).unwrap();
        assert_eq!(bsi.acmod, 2);
        assert!(bsi.dialnorm_ch2.is_none());
        assert!(bsi.dialogue_normalization_ch2().is_none());
    }

    // ---------------------------------------------------------------
    // Annex D §2.3.1.7-10 — xbsi2 informational metadata.
    // ---------------------------------------------------------------

    /// Table D2.7 — `dsurexmod` decodes verbatim across all 4 codepoints.
    #[test]
    fn dsurexmod_decodes_all_4_codepoints() {
        use DolbySurroundExMode::*;
        assert_eq!(DolbySurroundExMode::from_code(0b00), NotIndicated);
        assert_eq!(DolbySurroundExMode::from_code(0b01), NotEncoded);
        assert_eq!(
            DolbySurroundExMode::from_code(0b10),
            SurroundExOrProLogicIIx
        );
        assert_eq!(DolbySurroundExMode::from_code(0b11), ProLogicIIz);
        // raw() round-trip.
        for code in 0u8..4 {
            assert_eq!(DolbySurroundExMode::from_code(code).raw(), code);
        }
    }

    /// Table D2.8 — `dheadphonmod` decodes verbatim. The `'11'`
    /// codepoint is `Reserved`; the spec instructs decoders to keep
    /// reproducing audio when it appears.
    #[test]
    fn dheadphonmod_decodes_all_4_codepoints() {
        use DolbyHeadphoneMode::*;
        assert_eq!(DolbyHeadphoneMode::from_code(0b00), NotIndicated);
        assert_eq!(DolbyHeadphoneMode::from_code(0b01), NotEncoded);
        assert_eq!(DolbyHeadphoneMode::from_code(0b10), Encoded);
        assert_eq!(DolbyHeadphoneMode::from_code(0b11), Reserved);
        for code in 0u8..4 {
            assert_eq!(DolbyHeadphoneMode::from_code(code).raw(), code);
        }
    }

    /// Table D2.9 — `adconvtyp` is a single bit (`Standard` vs `Hdcd`).
    #[test]
    fn adconvtyp_decodes_both_codepoints() {
        assert_eq!(AdConverterType::from_code(0), AdConverterType::Standard);
        assert_eq!(AdConverterType::from_code(1), AdConverterType::Hdcd);
        // Defensive — `from_code` masks the low bit.
        assert_eq!(AdConverterType::from_code(2), AdConverterType::Standard);
        assert_eq!(AdConverterType::from_code(3), AdConverterType::Hdcd);
        assert_eq!(AdConverterType::Standard.raw(), 0);
        assert_eq!(AdConverterType::Hdcd.raw(), 1);
    }

    /// Annex D §2.3.1.7 — `bsid == 6` with `xbsi2e == 1` surfaces the
    /// three typed playback hints on the parsed [`Bsi`]. Build a 3/2
    /// frame (acmod=7) with `xbsi1e == 0` (mix-level extensions
    /// absent), `xbsi2e == 1`, and Table D2.7 / D2.8 / D2.9 codepoints
    /// `(0b10, 0b00, 0b1)` — Dolby Surround EX on, headphone hint not
    /// indicated, HDCD source. The body `xbsi2(8)` + `encinfo(1)`
    /// reserved fields are populated with non-zero bits to verify the
    /// parser skips them but still surfaces the three typed fields.
    #[test]
    fn parse_surfaces_xbsi2_dsurexmod_dheadphonmod_adconvtyp() {
        // bsid=6 (5), bsmod=0 (3), acmod=7 (3), cmixlev=0 (2),
        // surmixlev=0 (2), lfeon=0 (1), dialnorm=27 (5),
        //   compre=0, langcode=0, audprodie=0, copyrightb=0, origbs=0,
        //   xbsi1e=0,
        //   xbsi2e=1, dsurexmod=0b10 (Surround EX / PLIIx),
        //              dheadphonmod=0b00 (NotIndicated),
        //              adconvtyp=0b1 (Hdcd),
        //              xbsi2=0b1010_1010 (reserved garbage — must be
        //                                 parsed-and-discarded),
        //              encinfo=0b1,
        //   addbsie=0.
        let bits: [(u8, u32); 20] = [
            (5, 6),
            (3, 0),
            (3, 7),
            (2, 0),
            (2, 0),
            (1, 0),           // lfeon
            (5, 27),          // dialnorm
            (1, 0),           // compre
            (1, 0),           // langcode
            (1, 0),           // audprodie
            (1, 0),           // copyrightb
            (1, 0),           // origbs
            (1, 0),           // xbsi1e=0
            (1, 1),           // xbsi2e=1
            (2, 0b10),        // dsurexmod = Surround EX / PLIIx
            (2, 0b00),        // dheadphonmod = NotIndicated
            (1, 0b1),         // adconvtyp = HDCD
            (8, 0b1010_1010), // xbsi2 (reserved garbage)
            (1, 0b1),         // encinfo (encoder-private)
            (1, 0),           // addbsie
        ];
        let bytes = pack_bits(&bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.bsid, 6);
        assert_eq!(b.acmod, 7);
        assert!(b.annex_d_mix_levels.is_none());
        assert_eq!(
            b.dsurexmod,
            Some(DolbySurroundExMode::SurroundExOrProLogicIIx)
        );
        assert_eq!(b.dheadphonmod, Some(DolbyHeadphoneMode::NotIndicated));
        assert_eq!(b.adconvtyp, Some(AdConverterType::Hdcd));
    }

    /// `bsid != 6` falls through the §5.3.2 base syntax — the
    /// `xbsi2e` block doesn't exist, so the three Annex D fields stay
    /// `None`. Use the round-202 `parses_minimal_2_0_stereo_bsi`
    /// fixture (bsid=8, 2/0 stereo) and just assert the new fields.
    #[test]
    fn parse_leaves_xbsi2_fields_none_outside_bsid_6() {
        // Identical layout to `parses_minimal_2_0_stereo_bsi`.
        let bits: [(u8, u32); 14] = [
            (5, 0b01000),
            (3, 0b000),
            (3, 0b010),
            (2, 0b00),
            (1, 0),
            (5, 27),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
        ];
        let bytes = pack_bits(&bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.bsid, 8);
        assert!(b.dsurexmod.is_none());
        assert!(b.dheadphonmod.is_none());
        assert!(b.adconvtyp.is_none());
        assert!(b.extra_bsi.is_none());
    }

    // ---------------------------------------------------------------
    // ExtraBsi2 — §2.3.1.11-12 reserved + encoder-private trailer.
    // ---------------------------------------------------------------

    /// `from_raw` preserves the `xbsi2` byte and `encinfo` bit verbatim
    /// across every combination — no validation per §2.3.1.11-12 (both
    /// fields are reserved, so any 8+1-bit combination is wire-legal).
    /// Also exercises the `Eq` + `Copy` semantics by passing the value
    /// to a sibling helper after the implicit move.
    #[test]
    fn extra_bsi2_from_raw_round_trips_every_byte() {
        // Every 8-bit `xbsi2` codepoint × both `encinfo` values.
        for xbsi2 in 0u8..=255u8 {
            for &enc in &[false, true] {
                let x = ExtraBsi2::from_raw(xbsi2, enc);
                assert_eq!(x.xbsi2(), xbsi2);
                assert_eq!(x.encinfo(), enc);
                assert_eq!(x.wire_bits(), 9);
                // Copy: `x` survives the implicit move.
                let y = x;
                assert_eq!(x, y);
            }
        }
    }

    /// `is_spec_reserved_value` flags only the §2.3.1.11 wire-conformance
    /// `xbsi2 == 0x00` byte; `encinfo` is excluded from the check per
    /// §2.3.1.12 (encoder-private, any value is wire-legal).
    #[test]
    fn extra_bsi2_predicate_only_accepts_zero_xbsi2() {
        // `xbsi2 == 0x00` is the only spec-conformant codepoint.
        assert!(ExtraBsi2::from_raw(0x00, false).is_spec_reserved_value());
        assert!(ExtraBsi2::from_raw(0x00, true).is_spec_reserved_value());
        // Any non-zero `xbsi2` byte fails the conformance check.
        for xbsi2 in 1u8..=255u8 {
            assert!(!ExtraBsi2::from_raw(xbsi2, false).is_spec_reserved_value());
            assert!(!ExtraBsi2::from_raw(xbsi2, true).is_spec_reserved_value());
        }
    }

    /// Annex D `bsid == 6` with `xbsi2e == 1` surfaces the typed
    /// [`ExtraBsi2`] alongside the pre-existing
    /// `dsurexmod`/`dheadphonmod`/`adconvtyp` triplet. Confirms the
    /// raw `xbsi2 == 0xAA` byte + `encinfo == 1` round-trip through
    /// `parse()` and that `is_spec_reserved_value` reports `false`
    /// for the non-conformant byte. Layout cloned from the existing
    /// `parse_surfaces_xbsi2_dsurexmod_dheadphonmod_adconvtyp` so the
    /// new field is the only behaviour change.
    #[test]
    fn parse_surfaces_extra_bsi2_nonzero_codepoint() {
        let bits: [(u8, u32); 20] = [
            (5, 6),           // bsid
            (3, 0),           // bsmod
            (3, 7),           // acmod (3/2)
            (2, 0),           // cmixlev
            (2, 0),           // surmixlev
            (1, 0),           // lfeon
            (5, 27),          // dialnorm
            (1, 0),           // compre
            (1, 0),           // langcode
            (1, 0),           // audprodie
            (1, 0),           // copyrightb
            (1, 0),           // origbs
            (1, 0),           // xbsi1e
            (1, 1),           // xbsi2e = 1
            (2, 0b10),        // dsurexmod
            (2, 0b00),        // dheadphonmod
            (1, 0b1),         // adconvtyp = HDCD
            (8, 0b1010_1010), // xbsi2 (non-conformant codepoint)
            (1, 0b1),         // encinfo = 1
            (1, 0),           // addbsie
        ];
        let bytes = pack_bits(&bits);
        let b = parse(&bytes).unwrap();
        let extra = b.extra_bsi.expect("xbsi2e == 1 surfaces an ExtraBsi2");
        assert_eq!(extra.xbsi2(), 0b1010_1010);
        assert!(extra.encinfo());
        assert!(!extra.is_spec_reserved_value());
    }

    /// Spec-conformant Annex D xbsi2 — `xbsi2 == 0x00` per §2.3.1.11,
    /// `encinfo == 0` (encoder did not stash a private bit). The
    /// `is_spec_reserved_value` predicate reports `true`. The typed
    /// surface is `Some`, distinguishing "conformant emitted block"
    /// from "block absent" (which is `None`).
    #[test]
    fn parse_surfaces_extra_bsi2_conformant_zero_codepoint() {
        let bits: [(u8, u32); 20] = [
            (5, 6),    // bsid
            (3, 0),    // bsmod
            (3, 2),    // acmod (2/0 stereo)
            (2, 0),    // dsurmod
            (1, 0),    // lfeon
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // langcode
            (1, 0),    // audprodie
            (1, 0),    // copyrightb
            (1, 0),    // origbs
            (1, 0),    // xbsi1e
            (1, 1),    // xbsi2e = 1
            (2, 0b00), // dsurexmod = NotIndicated
            (2, 0b10), // dheadphonmod = Encoded
            (1, 0b0),  // adconvtyp = Standard
            (8, 0x00), // xbsi2 = spec-conformant zero
            (1, 0b0),  // encinfo = 0
            (1, 0),    // addbsie
            (1, 0),    // padding (keeps the [(u8,u32); 20] cardinality)
        ];
        let bytes = pack_bits(&bits);
        let b = parse(&bytes).unwrap();
        let extra = b.extra_bsi.expect("xbsi2e == 1 surfaces an ExtraBsi2");
        assert_eq!(extra.xbsi2(), 0x00);
        assert!(!extra.encinfo());
        assert!(extra.is_spec_reserved_value());
        // Cross-check the sibling typed fields surface correctly too —
        // a 2/0 frame is the only mode where `dheadphonmod` is semantically
        // defined per §2.3.1.9.
        assert_eq!(b.dheadphonmod, Some(DolbyHeadphoneMode::Encoded));
        assert_eq!(b.adconvtyp, Some(AdConverterType::Standard));
    }

    /// `bsid == 6` with `xbsi2e == 0` short-circuits the xbsi2 block —
    /// `extra_bsi` stays `None` even though the gating flag bit is on
    /// the wire. Confirms the parser distinguishes "block absent"
    /// (`None`) from "block present, all-zero" (`Some(0x00, false)`).
    #[test]
    fn parse_leaves_extra_bsi2_none_when_xbsi2e_zero() {
        let bits: [(u8, u32); 14] = [
            (5, 6),  // bsid
            (3, 0),  // bsmod
            (3, 2),  // acmod (2/0)
            (2, 0),  // dsurmod
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 0),  // copyrightb
            (1, 0),  // origbs
            (1, 0),  // xbsi1e
            (1, 0),  // xbsi2e = 0
            (1, 0),  // addbsie
        ];
        let bytes = pack_bits(&bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.bsid, 6);
        assert!(b.extra_bsi.is_none());
        // Companion fields also stay None per the gate.
        assert!(b.dsurexmod.is_none());
        assert!(b.dheadphonmod.is_none());
        assert!(b.adconvtyp.is_none());
    }

    /// §5.4.2.15 / Table 5.12 — every codepoint of `roomtyp` decodes
    /// to its named variant and round-trips through `raw()`.
    #[test]
    fn room_type_table_5_12_round_trip() {
        for (code, want) in [
            (0u8, RoomType::NotIndicated),
            (1, RoomType::LargeXCurve),
            (2, RoomType::SmallFlat),
            (3, RoomType::Reserved),
        ] {
            let got = RoomType::from_code(code);
            assert_eq!(got, want, "code={code:02b}");
            assert_eq!(got.raw(), code, "raw round-trip: code={code:02b}");
        }
        // Upper 6 bits of input are ignored — only the low 2 bits matter.
        assert_eq!(RoomType::from_code(0b1111_1110), RoomType::SmallFlat);
    }

    /// §5.4.2.14 — `mixlevel` is the 5-bit code, peak SPL is
    /// `80 + mixlevel` dB. Spot the endpoints (`0` → 80 dB SPL,
    /// `31` → 111 dB SPL) and a typical mid-range value
    /// (`mixlevel = 5` → 85 dB SPL, ITU-R BS.775 reference monitor).
    #[test]
    fn audio_production_info_peak_db_spl_endpoints() {
        let lo = AudioProductionInfo {
            mixlevel: 0,
            roomtyp: RoomType::NotIndicated,
        };
        assert_eq!(lo.peak_mix_level_db_spl(), 80);
        let mid = AudioProductionInfo {
            mixlevel: 5,
            roomtyp: RoomType::LargeXCurve,
        };
        assert_eq!(mid.peak_mix_level_db_spl(), 85);
        let hi = AudioProductionInfo {
            mixlevel: 31,
            roomtyp: RoomType::SmallFlat,
        };
        assert_eq!(hi.peak_mix_level_db_spl(), 111);
    }

    /// `parse()` surfaces `audprodie==1` into a typed
    /// [`AudioProductionInfo`] with the 5-bit `mixlevel` and Table 5.12
    /// `roomtyp` taken verbatim from the wire. Build a 1/0 mono BSI
    /// (`acmod=1`) with `audprodie=1`, mixlevel=0b10101 (85 dB SPL),
    /// roomtyp=0b01 (`LargeXCurve`), and verify both decode correctly.
    /// `audio_production_ch2` stays `None` because the stream is not
    /// 1+1 dual-mono.
    #[test]
    fn parse_surfaces_audio_production_when_audprodie_set() {
        let bits: [(u8, u32); 16] = [
            (5, 8),       // bsid
            (3, 0),       // bsmod
            (3, 1),       // acmod = 1/0 mono → no cmix/surmix/dsurmod
            (1, 0),       // lfeon
            (5, 27),      // dialnorm
            (1, 0),       // compre
            (1, 0),       // langcode
            (1, 1),       // audprodie = 1
            (5, 0b10101), // mixlevel = 21 → 101 dB SPL
            (2, 0b01),    // roomtyp = LargeXCurve
            // No 1+1 mirror — acmod != 0.
            (1, 0), // copyrightb
            (1, 0), // origbs
            (1, 0), // timecod1e
            (1, 0), // timecod2e
            (1, 0), // addbsie
            (1, 0), // pad
        ];
        let bytes = pack_bits(&bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.acmod, 1);
        let ap = b
            .audio_production
            .expect("audprodie=1 should surface audio_production");
        assert_eq!(ap.mixlevel, 0b10101);
        assert_eq!(ap.peak_mix_level_db_spl(), 80 + 21);
        assert_eq!(ap.roomtyp, RoomType::LargeXCurve);
        // Not 1+1 dual-mono → no Ch2 mirror.
        assert!(b.audio_production_ch2.is_none());
    }

    /// `audprodie==0` leaves [`Bsi::audio_production`] as `None`. The
    /// existing `parses_minimal_2_0_stereo_bsi` fixture exercises this
    /// case (it clears `audprodie`), so just re-pack a minimal 2/0
    /// stream and assert.
    #[test]
    fn parse_leaves_audio_production_none_when_audprodie_clear() {
        let bits: [(u8, u32); 14] = [
            (5, 0b01000),
            (3, 0b000),
            (3, 0b010),
            (2, 0b00),
            (1, 0),
            (5, 27),
            (1, 0), // compre
            (1, 0), // langcode
            (1, 0), // audprodie = 0
            (1, 0), // copyrightb
            (1, 0), // origbs
            (1, 0),
            (1, 0),
            (1, 0),
        ];
        let bytes = pack_bits(&bits);
        let b = parse(&bytes).unwrap();
        assert!(b.audio_production.is_none());
        assert!(b.audio_production_ch2.is_none());
    }

    /// 1+1 dual-mono (`acmod == 0`) emits an independent `audprodi2e`
    /// chain for Ch2 per §5.4.2.21-23. Build a stream with Ch1
    /// audprodie=1 (mixlevel=8, roomtyp=SmallFlat) AND Ch2
    /// audprodi2e=1 (mixlevel=0, roomtyp=NotIndicated) and verify both
    /// fields surface independently.
    #[test]
    fn parse_surfaces_audio_production_ch2_in_dual_mono() {
        let bits: [(u8, u32); 20] = [
            (5, 8),       // bsid
            (3, 0),       // bsmod
            (3, 0),       // acmod = 0 (1+1 dual-mono)
            (1, 0),       // lfeon
            (5, 27),      // dialnorm
            (1, 0),       // compre
            (1, 0),       // langcode
            (1, 1),       // audprodie = 1
            (5, 0b01000), // mixlevel = 8 → 88 dB SPL
            (2, 0b10),    // roomtyp = SmallFlat
            // 1+1 second block.
            (5, 27),      // dialnorm2
            (1, 0),       // compr2e
            (1, 0),       // langcod2e
            (1, 1),       // audprodi2e = 1
            (5, 0b00000), // mixlevel2 = 0 → 80 dB SPL
            (2, 0b00),    // roomtyp2 = NotIndicated
            (1, 0),       // copyrightb
            (1, 0),       // origbs
            (1, 0),       // timecod1e
            (1, 0),       // timecod2e
        ];
        let mut bytes = pack_bits(&bits);
        bytes.push(0); // addbsie pad
        let b = parse(&bytes).unwrap();
        assert_eq!(b.acmod, 0);
        let ap1 = b
            .audio_production
            .expect("audprodie=1 should surface Ch1 production");
        assert_eq!(ap1.mixlevel, 8);
        assert_eq!(ap1.peak_mix_level_db_spl(), 88);
        assert_eq!(ap1.roomtyp, RoomType::SmallFlat);
        let ap2 = b
            .audio_production_ch2
            .expect("audprodi2e=1 should surface Ch2 production");
        assert_eq!(ap2.mixlevel, 0);
        assert_eq!(ap2.peak_mix_level_db_spl(), 80);
        assert_eq!(ap2.roomtyp, RoomType::NotIndicated);
    }

    // ---------------------------------------------------------------
    // Time code (`timecod1` / `timecod2`) — §5.4.2.26-28 / Table 5.13.
    // ---------------------------------------------------------------

    /// [`TimeCode1`] splits its 14 wire bits as 5+6+3 (hours, minutes,
    /// 8-second increments). Walk a few hand-packed codepoints and
    /// verify each accessor lifts the right slice.
    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn timecode1_field_decomposition_matches_spec() {
        // (raw14, hours, minutes, eight_second_increments)
        let cases: [(u16, u8, u8, u8); 6] = [
            // All zeroes → 00:00:00.
            (0b00000_000000_000, 0, 0, 0),
            // Maximum spec-valid: 23 h, 59 m, 56 s (7×8).
            (0b10111_111011_111, 23, 59, 7),
            // Minimal hour bump: 01:00:00.
            (0b00001_000000_000, 1, 0, 0),
            // Minute boundary: 00:59:00.
            (0b00000_111011_000, 0, 59, 0),
            // Eight-second boundary at 00:00:48 (8×6).
            (0b00000_000000_110, 0, 0, 6),
            // Out-of-range hour codepoint (24..=31) per §5.4.2.27 — the
            // wire layout reserves these values; the parser still
            // surfaces them so a careful consumer can decide.
            (0b11111_111111_111, 31, 63, 7),
        ];
        for (raw, h, m, s8) in cases {
            let tc = TimeCode1::from_raw(raw);
            assert_eq!(tc.raw(), raw & 0x3FFF, "raw mask, raw={raw:#018b}");
            assert_eq!(tc.hours(), h, "hours, raw={raw:#018b}");
            assert_eq!(tc.minutes(), m, "minutes, raw={raw:#018b}");
            assert_eq!(
                tc.eight_second_increments(),
                s8,
                "8-second increments, raw={raw:#018b}"
            );
            // seconds_in_day = h·3600 + m·60 + s8·8.
            let want_secs = (h as u32) * 3600 + (m as u32) * 60 + (s8 as u32) * 8;
            assert_eq!(tc.seconds_in_day(), want_secs);
        }
    }

    /// `TimeCode1::is_spec_valid()` flags out-of-range hours / minutes.
    /// The eight-second-increment field cannot escape its 3-bit
    /// 0..=7 range.
    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn timecode1_spec_valid_checks_hours_and_minutes() {
        // 23:59:56 is the maximum valid combination.
        assert!(TimeCode1::from_raw(0b10111_111011_111).is_spec_valid());
        // hours = 24 (reserved).
        assert!(!TimeCode1::from_raw(0b11000_111011_111).is_spec_valid());
        // minutes = 60 (reserved).
        assert!(!TimeCode1::from_raw(0b10111_111100_111).is_spec_valid());
        // hours = 0, minutes = 0, s8 = 0 is also valid.
        assert!(TimeCode1::from_raw(0).is_spec_valid());
    }

    /// [`TimeCode2`] splits its 14 wire bits as 3+5+6 (seconds, frames,
    /// frame fractions).
    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn timecode2_field_decomposition_matches_spec() {
        // (raw14, seconds, frames, frame_fractions)
        let cases: [(u16, u8, u8, u8); 5] = [
            // All zeroes.
            (0b000_00000_000000, 0, 0, 0),
            // Maximum spec-valid: s=7, f=29, ff=63.
            (0b111_11101_111111, 7, 29, 63),
            // Seconds boundary: s=7, f=0, ff=0.
            (0b111_00000_000000, 7, 0, 0),
            // Frames boundary at 30 fps (frames=29 is the max valid).
            (0b000_11101_000000, 0, 29, 0),
            // Out-of-range frames (30, 31) per §5.4.2.28 — codepoints
            // beyond the 30 fps reference; pass-through for caller
            // inspection.
            (0b000_11111_111111, 0, 31, 63),
        ];
        for (raw, s, f, ff) in cases {
            let tc = TimeCode2::from_raw(raw);
            assert_eq!(tc.raw(), raw & 0x3FFF, "raw mask, raw={raw:#018b}");
            assert_eq!(tc.seconds(), s, "seconds, raw={raw:#018b}");
            assert_eq!(tc.frames(), f, "frames, raw={raw:#018b}");
            assert_eq!(tc.frame_fractions(), ff, "frame fractions, raw={raw:#018b}");
        }
    }

    /// `TimeCode2::is_spec_valid()` rejects out-of-range frame
    /// codepoints (≥ 30 at the 30 fps reference assumed by §5.4.2.26).
    #[test]
    #[allow(clippy::unusual_byte_groupings)]
    fn timecode2_spec_valid_checks_frames() {
        assert!(TimeCode2::from_raw(0b111_11101_111111).is_spec_valid()); // f=29
        assert!(!TimeCode2::from_raw(0b000_11110_000000).is_spec_valid()); // f=30
        assert!(!TimeCode2::from_raw(0b000_11111_000000).is_spec_valid()); // f=31
        assert!(TimeCode2::from_raw(0).is_spec_valid()); // all zero is valid
    }

    /// Table 5.13 — the `(timecod2e, timecod1e)` pair maps to a
    /// presence-pattern enum. Walk every codepoint.
    #[test]
    fn timecode_presence_table_5_13_round_trip() {
        use TimeCodePresence::*;
        let rows: [(bool, bool, TimeCodePresence); 4] = [
            (false, false, NotPresent),
            (false, true, FirstHalfOnly),
            (true, false, SecondHalfOnly),
            (true, true, BothHalves),
        ];
        for (tc2e, tc1e, want) in rows {
            let got = TimeCodePresence::from_flags(tc2e, tc1e);
            assert_eq!(
                got, want,
                "(timecod2e={tc2e}, timecod1e={tc1e}): want {want:?}, got {got:?}"
            );
        }
    }

    /// `parse()` surfaces `timecod1` / `timecod2` independently when
    /// each `timecod*e` flag is set. Build a 1/0 mono BSI carrying
    /// `(h=12, m=34, s8=5, s=3, f=15, ff=42)` and verify the parser
    /// routes both halves into the typed surface plus
    /// `timecode_presence == BothHalves`.
    #[test]
    fn parse_surfaces_both_timecode_halves() {
        // tc1 raw = h(5)·512 + m(6)·8 + s8(3) packed MSB-first as
        //         (12 << 9) | (34 << 3) | 5 = 0x18 << 9 | 0x22 << 3 | 5
        //         = 0b01100_100010_101
        // tc2 raw = s(3)·2048 + f(5)·64 + ff(6) packed MSB-first as
        //         (3 << 11) | (15 << 6) | 42
        //         = 0b011_01111_101010
        let tc1_raw: u32 = (12u32 << 9) | (34u32 << 3) | 5u32;
        let tc2_raw: u32 = (3u32 << 11) | (15u32 << 6) | 42u32;
        let bits: [(u8, u32); 15] = [
            (5, 8),  // bsid (base syntax)
            (3, 0),  // bsmod
            (3, 1),  // acmod = 1 (1/0 mono, no cmix / surmix / dsurmod)
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 0),  // copyrightb
            (1, 0),  // origbs
            (1, 1),  // timecod1e
            (14, tc1_raw),
            (1, 1), // timecod2e
            (14, tc2_raw),
            (1, 0), // addbsie
        ];
        let bytes = pack_bits(&bits);
        let bsi = parse(&bytes).unwrap();
        assert_eq!(bsi.bsid, 8);
        assert_eq!(bsi.acmod, 1);
        assert_eq!(bsi.timecode_presence, TimeCodePresence::BothHalves);
        let tc1 = bsi.timecod1.expect("timecod1e=1 should surface tc1");
        assert_eq!(tc1.hours(), 12);
        assert_eq!(tc1.minutes(), 34);
        assert_eq!(tc1.eight_second_increments(), 5);
        assert_eq!(tc1.raw(), tc1_raw as u16);
        assert_eq!(tc1.seconds_in_day(), 12 * 3600 + 34 * 60 + 5 * 8);
        assert!(tc1.is_spec_valid());
        let tc2 = bsi.timecod2.expect("timecod2e=1 should surface tc2");
        assert_eq!(tc2.seconds(), 3);
        assert_eq!(tc2.frames(), 15);
        assert_eq!(tc2.frame_fractions(), 42);
        assert_eq!(tc2.raw(), tc2_raw as u16);
        assert!(tc2.is_spec_valid());
    }

    /// Only the first half present per Table 5.13 row
    /// `(timecod2e=0, timecod1e=1)`. `parse()` should surface
    /// `timecod1` and leave `timecod2 == None` with
    /// `timecode_presence == FirstHalfOnly`.
    #[test]
    fn parse_surfaces_only_first_timecode_half_when_only_timecod1e() {
        let tc1_raw: u32 = (1u32 << 9) | (2u32 << 3) | 3u32; // 01:02:24
        let bits: [(u8, u32); 14] = [
            (5, 8),  // bsid
            (3, 0),  // bsmod
            (3, 1),  // acmod = 1
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 0),  // copyrightb
            (1, 0),  // origbs
            (1, 1),  // timecod1e=1
            (14, tc1_raw),
            (1, 0), // timecod2e=0
            (1, 0), // addbsie=0
        ];
        let bytes = pack_bits(&bits);
        let bsi = parse(&bytes).unwrap();
        assert_eq!(bsi.timecode_presence, TimeCodePresence::FirstHalfOnly);
        let tc1 = bsi.timecod1.expect("tc1 should surface");
        assert_eq!(tc1.hours(), 1);
        assert_eq!(tc1.minutes(), 2);
        assert_eq!(tc1.eight_second_increments(), 3);
        assert!(bsi.timecod2.is_none(), "tc2 should not surface");
    }

    /// Only the second half present per Table 5.13 row
    /// `(timecod2e=1, timecod1e=0)`. `parse()` should leave
    /// `timecod1 == None` and surface `timecod2`.
    #[test]
    fn parse_surfaces_only_second_timecode_half_when_only_timecod2e() {
        let tc2_raw: u32 = (4u32 << 11) | (10u32 << 6) | 20u32;
        let bits: [(u8, u32); 14] = [
            (5, 8),  // bsid
            (3, 0),  // bsmod
            (3, 1),  // acmod = 1
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 0),  // copyrightb
            (1, 0),  // origbs
            (1, 0),  // timecod1e=0
            (1, 1),  // timecod2e=1
            (14, tc2_raw),
            (1, 0), // addbsie=0
        ];
        let bytes = pack_bits(&bits);
        let bsi = parse(&bytes).unwrap();
        assert_eq!(bsi.timecode_presence, TimeCodePresence::SecondHalfOnly);
        assert!(bsi.timecod1.is_none(), "tc1 should not surface");
        let tc2 = bsi.timecod2.expect("tc2 should surface");
        assert_eq!(tc2.seconds(), 4);
        assert_eq!(tc2.frames(), 10);
        assert_eq!(tc2.frame_fractions(), 20);
    }

    /// `(timecod2e=0, timecod1e=0)` per Table 5.13 leaves both halves
    /// at `None` and `timecode_presence == NotPresent`. Reuses the
    /// minimal 2/0 stereo fixture which has both flags clear.
    #[test]
    fn parse_leaves_timecode_none_when_both_flags_zero() {
        let bits: [(u8, u32); 14] = [
            (5, 8),
            (3, 0),
            (3, 2),
            (2, 0),
            (1, 0),
            (5, 27),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0), // timecod1e=0
            (1, 0), // timecod2e=0
            (1, 0), // addbsie=0
        ];
        let bytes = pack_bits(&bits);
        let bsi = parse(&bytes).unwrap();
        assert!(bsi.timecod1.is_none());
        assert!(bsi.timecod2.is_none());
        assert_eq!(bsi.timecode_presence, TimeCodePresence::NotPresent);
    }

    /// `bsid == 6` activates the Annex D alternate bit stream syntax;
    /// the `timecod*e` slots carry `xbsi*e` instead so the timecode is
    /// definitionally absent regardless of how the slots were set.
    /// Verify the parser leaves the timecode surface untouched on an
    /// `xbsi1e == 1` stream (the Annex D mix-levels fixture).
    #[test]
    fn parse_leaves_timecode_none_on_annex_d_bsid_6() {
        // Reuse the Annex D xbsi1 fixture from
        // `parses_annex_d_bsid_6_xbsi1_mix_levels` — xbsi1e=1 sets the
        // bits that under the base syntax would mean "timecode present".
        let bits: &[(u8, u32)] = &[
            (5, 6),
            (3, 0),
            (3, 7),
            (2, 0),
            (2, 0),
            (1, 1),
            (5, 27),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 1),    // xbsi1e=1 (would be timecod1e=1 under base syntax)
            (2, 0b01), // dmixmod
            (3, 0b011),
            (3, 0b100),
            (3, 0b100),
            (3, 0b101),
            (1, 0), // xbsi2e=0
            (1, 0), // addbsie=0
        ];
        let bytes = pack_bits(bits);
        let bsi = parse(&bytes).unwrap();
        assert_eq!(bsi.bsid, 6);
        assert!(
            bsi.annex_d_mix_levels.is_some(),
            "Annex D mix-levels surfaced"
        );
        assert!(
            bsi.timecod1.is_none(),
            "Annex D syntax must NOT surface timecod1"
        );
        assert!(
            bsi.timecod2.is_none(),
            "Annex D syntax must NOT surface timecod2"
        );
        assert_eq!(bsi.timecode_presence, TimeCodePresence::NotPresent);
    }

    /// `bsid == 6` with `xbsi2e == 0` keeps the three Annex D fields at
    /// `None` even though the alternate syntax is active — the encoder
    /// chose to omit the playback metadata. The xbsi1 block is also
    /// disabled here to keep the bit string short.
    #[test]
    fn parse_leaves_xbsi2_fields_none_when_xbsi2e_zero() {
        // bsid=6, acmod=2 (2/0 stereo): no cmix, surmixlev=0xFF guard,
        // dsurmod present. Skip xbsi1e/xbsi2e/addbsie.
        let bits: [(u8, u32); 16] = [
            (5, 6),
            (3, 0),
            (3, 2),
            (2, 0),  // dsurmod
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 0),  // copyrightb
            (1, 0),  // origbs
            (1, 0),  // xbsi1e=0
            (1, 0),  // xbsi2e=0
            (1, 0),  // addbsie=0
            (1, 0),  // pad
            (1, 0),  // pad
        ];
        let bytes = pack_bits(&bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.bsid, 6);
        assert!(b.dsurexmod.is_none());
        assert!(b.dheadphonmod.is_none());
        assert!(b.adconvtyp.is_none());
    }

    // ---------------------------------------------------------------
    // DolbySurroundMode — §5.4.2.6 / Table 5.11 (2/0 stereo `acmod==2`).
    // ---------------------------------------------------------------

    /// Table 5.11 — `dsurmod` decodes verbatim across all 4 codepoints
    /// and the raw round-trip is byte-stable.
    #[test]
    fn dsurmod_decodes_all_4_codepoints() {
        use DolbySurroundMode::*;
        assert_eq!(DolbySurroundMode::from_code(0b00), NotIndicated);
        assert_eq!(DolbySurroundMode::from_code(0b01), NotEncoded);
        assert_eq!(DolbySurroundMode::from_code(0b10), Encoded);
        assert_eq!(DolbySurroundMode::from_code(0b11), Reserved);
        for code in 0u8..4 {
            assert_eq!(DolbySurroundMode::from_code(code).raw(), code);
        }
    }

    /// §5.4.2.6 spec note: the reserved code "may be interpreted as
    /// 'not indicated'" — `is_not_indicated()` collapses both
    /// codepoints into one branch.
    #[test]
    fn dsurmod_is_not_indicated_collapses_reserved() {
        use DolbySurroundMode::*;
        assert!(NotIndicated.is_not_indicated());
        assert!(Reserved.is_not_indicated());
        assert!(!NotEncoded.is_not_indicated());
        assert!(!Encoded.is_not_indicated());
        // Encoded is the only codepoint that should arm a matrix decoder.
        assert!(Encoded.is_dolby_surround_encoded());
        assert!(!NotIndicated.is_dolby_surround_encoded());
        assert!(!NotEncoded.is_dolby_surround_encoded());
        assert!(!Reserved.is_dolby_surround_encoded());
    }

    /// 2/0 stereo (`acmod == 2`) syncframe surfaces the typed
    /// `dolby_surround_mode` view of the on-wire codepoint. Walk all
    /// four Table 5.11 rows through `parse()` and check both the raw
    /// `dsurmod` byte and the typed `Option<DolbySurroundMode>` line
    /// up.
    #[test]
    fn parse_surfaces_dolby_surround_mode_on_2_0() {
        for code in 0u32..4 {
            // bsid=8, bsmod=0, acmod=2, cmixlev absent, surmixlev absent,
            // dsurmod=code, lfeon=0, dialnorm=27, then the optional
            // chain clipped down to a tail of zeros (compre=0, langcode=0,
            // audprodie=0, copyrightb=0, origbs=0, timecod1e=0,
            // timecod2e=0, addbsie=0).
            let bits: [(u8, u32); 14] = [
                (5, 8), // bsid
                (3, 0), // bsmod
                (3, 2), // acmod
                // cmixlev absent (acmod & 1 == 0 → guard false)
                // surmixlev absent (acmod & 4 == 0 → guard false)
                (2, code), // dsurmod
                (1, 0),    // lfeon
                (5, 27),   // dialnorm
                (1, 0),    // compre
                (1, 0),    // langcode
                (1, 0),    // audprodie
                (1, 0),    // copyrightb
                (1, 0),    // origbs
                (1, 0),    // timecod1e
                (1, 0),    // timecod2e
                (1, 0),    // addbsie
            ];
            let bytes = pack_bits(&bits);
            let b = parse(&bytes).unwrap();
            assert_eq!(b.bsid, 8);
            assert_eq!(b.acmod, 2);
            assert_eq!(b.dsurmod, code as u8);
            let expected = DolbySurroundMode::from_code(code as u8);
            assert_eq!(b.dolby_surround_mode, Some(expected));
            // Accessor matches the field.
            assert_eq!(b.dolby_surround_mode(), Some(expected));
        }
    }

    /// `acmod != 2` syncframe — the §5.3.2 guard skips the 2-bit
    /// `dsurmod` slot entirely, so the typed `dolby_surround_mode`
    /// resolves to `None` and the raw byte stays at the `0xFF`
    /// "absent" sentinel.
    #[test]
    fn parse_dolby_surround_mode_none_when_acmod_not_2_0() {
        // bsid=8, bsmod=0, acmod=7 (3/2 — no dsurmod slot), cmixlev=0,
        // surmixlev=0, lfeon=0, dialnorm=27, tail zeros.
        let bits: [(u8, u32); 15] = [
            (5, 8),
            (3, 0),
            (3, 7), // acmod = 3/2 (5-channel)
            (2, 0), // cmixlev (acmod & 1 != 0)
            (2, 0), // surmixlev (acmod & 4 != 0)
            // dsurmod absent (acmod != 2)
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 0),  // copyrightb
            (1, 0),  // origbs
            (1, 0),  // timecod1e
            (1, 0),  // timecod2e
            (1, 0),  // addbsie
        ];
        let bytes = pack_bits(&bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.acmod, 7);
        assert_eq!(b.dsurmod, 0xFF);
        assert!(b.dolby_surround_mode.is_none());
        assert!(b.dolby_surround_mode().is_none());
    }

    // ---------------------------------------------------------------
    // CopyrightInfo — §5.4.2.24-25 distribution-control hint pair.
    // ---------------------------------------------------------------

    /// Walk all four `(copyrightb, origbs)` codepoints and assert the
    /// raw 1-bit values + the semantic accessors line up with the spec
    /// text (§5.4.2.24 / §5.4.2.25).
    #[test]
    fn copyright_info_four_codepoints_round_trip() {
        let cases: [(bool, bool, bool, bool); 4] = [
            (false, false, false, false),
            (false, true, false, true),
            (true, false, true, false),
            (true, true, true, true),
        ];
        for (c, o, exp_protected, exp_original) in cases {
            let ci = CopyrightInfo::from_bits(c, o);
            assert_eq!(ci.is_copyright_protected(), exp_protected);
            assert_eq!(ci.is_original_bitstream(), exp_original);
            assert_eq!(ci.copyrightb_bit(), u8::from(c));
            assert_eq!(ci.origbs_bit(), u8::from(o));
        }
    }

    /// Equality + Copy semantics — two `CopyrightInfo` built from the
    /// same bits compare equal, derive `Copy` so the typed surface can
    /// be passed by value alongside the other small typed fields on
    /// the BSI without ref-counting.
    #[test]
    fn copyright_info_eq_and_copy() {
        let a = CopyrightInfo::from_bits(true, false);
        let b = CopyrightInfo::from_bits(true, false);
        let c = CopyrightInfo::from_bits(false, true);
        assert_eq!(a, b);
        assert_ne!(a, c);
        // Copy: `a` survives the `let _ = a;` use after the implicit move.
        let moved = a;
        assert_eq!(moved, a);
    }

    /// Parse a minimal 2/0 BSI with `copyrightb=0, origbs=1` (the
    /// base-AC-3 encoder's default — "not protected, original
    /// bitstream") and confirm the typed surface decodes the pair.
    #[test]
    fn parses_copyright_info_encoder_default() {
        // 2/0 stereo, dialnorm=27. All metadata flags off. `copyrightb=0`,
        // `origbs=1` matches the in-tree base-AC-3 encoder's emit.
        let bits: &[(u8, u32)] = &[
            (5, 8),  // bsid
            (3, 0),  // bsmod
            (3, 2),  // acmod
            (2, 0),  // dsurmod
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 0),  // copyrightb
            (1, 1),  // origbs
            (1, 0),  // timecod1e
            (1, 0),  // timecod2e
            (1, 0),  // addbsie
        ];
        let bytes = pack_bits(bits);
        let b = parse(&bytes).unwrap();
        assert!(!b.copyright_info.is_copyright_protected());
        assert!(b.copyright_info.is_original_bitstream());
        assert_eq!(b.copyright_info.copyrightb_bit(), 0);
        assert_eq!(b.copyright_info.origbs_bit(), 1);
    }

    /// Parse a minimal 2/0 BSI with `copyrightb=1, origbs=0` (the
    /// "protected copy" pattern — a downstream re-distribution should
    /// honour the copyright tag and the "this is a copy" flag).
    #[test]
    fn parses_copyright_info_protected_copy() {
        let bits: &[(u8, u32)] = &[
            (5, 8),  // bsid
            (3, 0),  // bsmod
            (3, 2),  // acmod
            (2, 0),  // dsurmod
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 1),  // copyrightb
            (1, 0),  // origbs
            (1, 0),  // timecod1e
            (1, 0),  // timecod2e
            (1, 0),  // addbsie
        ];
        let bytes = pack_bits(bits);
        let b = parse(&bytes).unwrap();
        assert!(b.copyright_info.is_copyright_protected());
        assert!(!b.copyright_info.is_original_bitstream());
    }

    /// Parse a 1+1 dual-mono BSI (acmod=0) and confirm `copyrightb` /
    /// `origbs` decode correctly even when the 1+1 chain pushes the
    /// pair further down the bit cursor (extra `dialnorm2` + Ch2
    /// `compr2e` + `langcod2e` + `audprodi2e` flags sit between the
    /// Ch1 metadata block and the `copyrightb`/`origbs` slots).
    #[test]
    fn parses_copyright_info_dual_mono_acmod_0() {
        // acmod=0 (1+1). Ch1 metadata flags off, Ch2 metadata flags
        // off, copyrightb=1, origbs=1.
        let bits: &[(u8, u32)] = &[
            (5, 8), // bsid
            (3, 0), // bsmod
            (3, 0), // acmod = 1+1
            // cmixlev / surmixlev / dsurmod all absent for acmod=0.
            (1, 0),  // lfeon
            (5, 27), // dialnorm (Ch1)
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            // 1+1 Ch2 service-metadata block
            (5, 27), // dialnorm2
            (1, 0),  // compr2e
            (1, 0),  // langcod2e
            (1, 0),  // audprodi2e
            (1, 1),  // copyrightb
            (1, 1),  // origbs
            (1, 0),  // timecod1e
            (1, 0),  // timecod2e
            (1, 0),  // addbsie
        ];
        let bytes = pack_bits(bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.acmod, 0);
        assert!(b.copyright_info.is_copyright_protected());
        assert!(b.copyright_info.is_original_bitstream());
    }

    // -----------------------------------------------------------------
    // AdditionalBitStreamInfo (§5.4.2.29-31)
    // -----------------------------------------------------------------

    /// `from_addbsil_and_payload` rejects `addbsil > 63` (the wire field
    /// is 6 bits) and rejects a payload-length mismatch — these would
    /// not round-trip back through the bit-stream parser.
    #[test]
    fn additional_bsi_constructor_rejects_invalid_inputs() {
        assert!(AdditionalBitStreamInfo::from_addbsil_and_payload(64, vec![0u8; 65]).is_none());
        assert!(AdditionalBitStreamInfo::from_addbsil_and_payload(255, vec![0u8; 1]).is_none());
        // Length mismatch — addbsil=0 means 1 byte, payload has 2.
        assert!(AdditionalBitStreamInfo::from_addbsil_and_payload(0, vec![0u8, 0u8]).is_none());
        // Length mismatch — addbsil=3 means 4 bytes, payload has 3.
        assert!(
            AdditionalBitStreamInfo::from_addbsil_and_payload(3, vec![0u8, 0u8, 0u8]).is_none()
        );
    }

    /// Minimum-length payload: `addbsil == 0` ⇒ payload is 1 byte;
    /// surface exposes `len() == 1`, `is_empty() == false`,
    /// `wire_bits() == 7 + 8 == 15`.
    #[test]
    fn additional_bsi_min_length_payload() {
        let info = AdditionalBitStreamInfo::from_addbsil_and_payload(0, vec![0xA5]).unwrap();
        assert_eq!(info.addbsil(), 0);
        assert_eq!(info.len(), 1);
        assert!(!info.is_empty());
        assert_eq!(info.payload(), &[0xA5]);
        assert_eq!(info.wire_bits(), 15);
    }

    /// Maximum-length payload: `addbsil == 63` ⇒ payload is 64 bytes;
    /// surface exposes `len() == 64`, `wire_bits() == 7 + 8 × 64 ==
    /// 519`. Payload is preserved verbatim.
    #[test]
    fn additional_bsi_max_length_payload() {
        let payload: Vec<u8> = (0..64).collect();
        let info = AdditionalBitStreamInfo::from_addbsil_and_payload(63, payload.clone()).unwrap();
        assert_eq!(info.addbsil(), 63);
        assert_eq!(info.len(), 64);
        assert_eq!(info.payload(), payload.as_slice());
        assert_eq!(info.wire_bits(), 7 + 8 * 64);
    }

    /// Round-trip through `parse()` on a synthetic 1/0 mono BSI where
    /// the encoder set `addbsie == 1` with a 1-byte payload (the
    /// minimum). Confirms the parser recovers the payload byte
    /// verbatim and that `addbsil == 0 ⇒ len() == 1`.
    #[test]
    fn parses_addbsi_single_byte_payload() {
        // 1/0 mono, no optional service metadata, addbsie=1,
        // addbsil=0 (⇒ 1 payload byte), payload = 0b1011_0100.
        let bits: &[(u8, u32)] = &[
            (5, 8),    // bsid
            (3, 0),    // bsmod
            (3, 1),    // acmod (1/0 mono)
            (1, 0),    // lfeon
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // langcode
            (1, 0),    // audprodie
            (1, 0),    // copyrightb
            (1, 1),    // origbs
            (1, 0),    // timecod1e
            (1, 0),    // timecod2e
            (1, 1),    // addbsie
            (6, 0),    // addbsil (0 ⇒ 1 byte payload)
            (8, 0xB4), // addbsi payload
        ];
        let bytes = pack_bits(bits);
        let b = parse(&bytes).unwrap();
        let info = b.addbsi.expect("addbsie == 1 surfaces a payload");
        assert_eq!(info.addbsil(), 0);
        assert_eq!(info.len(), 1);
        assert_eq!(info.payload(), &[0xB4]);
    }

    /// Round-trip through `parse()` on a synthetic 1/0 mono BSI where
    /// the encoder set `addbsie == 1` with the maximum-length 64-byte
    /// payload. Confirms the parser walks all 64 bytes correctly and
    /// `bits_consumed` advances by `7 + 8 × 64 == 519` over the BSI
    /// tail block.
    #[test]
    fn parses_addbsi_max_length_payload() {
        let total_prefix_bits: u32 = 5 + 3 + 3 + 1 + 5 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1;
        let mut bits: Vec<(u8, u32)> = vec![
            (5, 8),  // bsid
            (3, 0),  // bsmod
            (3, 1),  // acmod
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 0),  // copyrightb
            (1, 1),  // origbs
            (1, 0),  // timecod1e
            (1, 0),  // timecod2e
            (1, 1),  // addbsie
            (6, 63), // addbsil = 63 ⇒ 64 payload bytes
        ];
        // 64 distinct payload bytes — easy to detect any cursor slippage.
        for k in 0..64u32 {
            bits.push((8, k ^ 0x55));
        }
        let bytes = pack_bits(&bits);
        let b = parse(&bytes).unwrap();
        let info = b.addbsi.expect("addbsie == 1 surfaces a payload");
        assert_eq!(info.addbsil(), 63);
        assert_eq!(info.len(), 64);
        let expected: Vec<u8> = (0u32..64).map(|k| (k ^ 0x55) as u8).collect();
        assert_eq!(info.payload(), expected.as_slice());
        let expected_bits = total_prefix_bits as u64 + 6 + 8 * 64;
        assert_eq!(b.bits_consumed, expected_bits);
    }

    /// `addbsie == 0` yields `addbsi == None` and the parser stops
    /// after the flag bit — `bits_consumed` should match the
    /// pre-addbsi byte count + 1 bit.
    #[test]
    fn parses_addbsi_absent_when_addbsie_zero() {
        // 1/0 mono, no optional fields, addbsie = 0.
        let bits: &[(u8, u32)] = &[
            (5, 8),  // bsid
            (3, 0),  // bsmod
            (3, 1),  // acmod
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 0),  // copyrightb
            (1, 1),  // origbs
            (1, 0),  // timecod1e
            (1, 0),  // timecod2e
            (1, 0),  // addbsie = 0
        ];
        let bytes = pack_bits(bits);
        let b = parse(&bytes).unwrap();
        assert!(b.addbsi.is_none());
        let expected_bits: u64 = 5 + 3 + 3 + 1 + 5 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1;
        assert_eq!(b.bits_consumed, expected_bits);
    }

    /// Annex D `bsid == 6` shares the addbsi position with the base
    /// syntax — only the `timecod*e` slots flip to `xbsi*e` upstream.
    /// Confirm the addbsi surface still decodes independently of the
    /// `bsid == 6` switch.
    #[test]
    fn parses_addbsi_on_annex_d_bsid_6() {
        // bsid=6, acmod=2 (2/0). xbsi1e=0, xbsi2e=0. addbsie=1 with a
        // 2-byte payload (addbsil=1).
        let bits: &[(u8, u32)] = &[
            (5, 6),  // bsid
            (3, 0),  // bsmod
            (3, 2),  // acmod (2/0 stereo)
            (2, 0),  // dsurmod
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 0),  // copyrightb
            (1, 1),  // origbs
            (1, 0),  // xbsi1e
            (1, 0),  // xbsi2e
            (1, 1),  // addbsie
            (6, 1),  // addbsil = 1 ⇒ 2 payload bytes
            (8, 0xCA),
            (8, 0xFE),
        ];
        let bytes = pack_bits(bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.bsid, 6);
        let info = b.addbsi.expect("addbsie == 1 surfaces a payload");
        assert_eq!(info.addbsil(), 1);
        assert_eq!(info.len(), 2);
        assert_eq!(info.payload(), &[0xCA, 0xFE]);
    }

    /// 1+1 dual-mono (`acmod == 0`) routes through the Ch2 service-
    /// metadata block before reaching the addbsi position. Confirm the
    /// addbsi surface still decodes correctly downstream of the longer
    /// Ch2 chain.
    #[test]
    fn parses_addbsi_on_1plus1_dual_mono() {
        let bits: &[(u8, u32)] = &[
            (5, 8),  // bsid
            (3, 0),  // bsmod
            (3, 0),  // acmod=0 (1+1)
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            // Ch2 service-metadata block
            (5, 27), // dialnorm2
            (1, 0),  // compr2e
            (1, 0),  // langcod2e
            (1, 0),  // audprodi2e
            (1, 0),  // copyrightb
            (1, 1),  // origbs
            (1, 0),  // timecod1e
            (1, 0),  // timecod2e
            (1, 1),  // addbsie
            (6, 2),  // addbsil = 2 ⇒ 3 payload bytes
            (8, 0xDE),
            (8, 0xAD),
            (8, 0xBE),
        ];
        let bytes = pack_bits(bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.acmod, 0);
        let info = b.addbsi.expect("addbsie == 1 surfaces a payload");
        assert_eq!(info.addbsil(), 2);
        assert_eq!(info.payload(), &[0xDE, 0xAD, 0xBE]);
    }

    /// Annex D `bsid == 6` keeps the same `copyrightb` / `origbs`
    /// position in the BSI — only the post-`origbs` slots flip from
    /// `timecod*e` to `xbsi*e`. Confirm the typed surface decodes
    /// independently of the `bsid == 6` switch.
    #[test]
    fn parses_copyright_info_annex_d_bsid_6() {
        // bsid=6, acmod=2 (2/0). xbsi1e=0, xbsi2e=0, copyrightb=0,
        // origbs=0 — distinct from the base-syntax default to confirm
        // the parser isn't reading a stale value.
        let bits: &[(u8, u32)] = &[
            (5, 6),  // bsid
            (3, 0),  // bsmod
            (3, 2),  // acmod
            (2, 0),  // dsurmod
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 0),  // copyrightb
            (1, 0),  // origbs
            (1, 0),  // xbsi1e
            (1, 0),  // xbsi2e
            (1, 0),  // addbsie
        ];
        let bytes = pack_bits(bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.bsid, 6);
        assert!(!b.copyright_info.is_copyright_protected());
        assert!(!b.copyright_info.is_original_bitstream());
    }

    /// Round 243 — Table D2.2 / §2.3.1.2 typed surface. The 2-bit
    /// codepoint decode is a direct lookup: `00` → `NotIndicated`,
    /// `01` → `LtRtPreferred`, `10` → `LoRoPreferred`, `11` →
    /// `Reserved`. `raw()` round-trips back to the original
    /// codepoint.
    #[test]
    fn stereo_downmix_preference_decodes_all_four_codepoints() {
        let cases = [
            (0b00u8, StereoDownmixPreference::NotIndicated),
            (0b01u8, StereoDownmixPreference::LtRtPreferred),
            (0b10u8, StereoDownmixPreference::LoRoPreferred),
            (0b11u8, StereoDownmixPreference::Reserved),
        ];
        for (code, expected) in cases {
            let decoded = StereoDownmixPreference::from_code(code);
            assert_eq!(decoded, expected, "code = {code:#04b}");
            assert_eq!(decoded.raw(), code, "raw round-trip for {decoded:?}");
        }
    }

    /// Spec §2.3.1.2 notes "the reserved code may be interpreted as
    /// 'not indicated'" — confirm
    /// [`StereoDownmixPreference::is_not_indicated`] collapses both
    /// codepoints into one branch.
    #[test]
    fn stereo_downmix_preference_treats_reserved_as_not_indicated() {
        assert!(StereoDownmixPreference::NotIndicated.is_not_indicated());
        assert!(StereoDownmixPreference::Reserved.is_not_indicated());
        assert!(!StereoDownmixPreference::LtRtPreferred.is_not_indicated());
        assert!(!StereoDownmixPreference::LoRoPreferred.is_not_indicated());
    }

    /// Confirm the LtRt / LoRo predicates short-circuit only on
    /// the explicit-preference variants.
    #[test]
    fn stereo_downmix_preference_predicates_match_explicit_variants() {
        assert!(StereoDownmixPreference::LtRtPreferred.prefers_lt_rt());
        assert!(!StereoDownmixPreference::LtRtPreferred.prefers_lo_ro());
        assert!(StereoDownmixPreference::LoRoPreferred.prefers_lo_ro());
        assert!(!StereoDownmixPreference::LoRoPreferred.prefers_lt_rt());
        assert!(!StereoDownmixPreference::NotIndicated.prefers_lt_rt());
        assert!(!StereoDownmixPreference::NotIndicated.prefers_lo_ro());
        assert!(!StereoDownmixPreference::Reserved.prefers_lt_rt());
        assert!(!StereoDownmixPreference::Reserved.prefers_lo_ro());
    }

    /// Base §5.3.2 timecode syntax (`bsid != 6`) reuses the bit slot
    /// for `timecod*e/timecod*`, so the preferred-downmix-mode hint
    /// stays absent. The typed surface must return `None` even when
    /// the raw `dmixmod` field carries the `0xFF` "absent" sentinel.
    #[test]
    fn parse_leaves_dmixmod_preference_none_in_base_syntax() {
        // bsid=8 (base syntax), acmod=7 (3/2 — the §2.3.1.2 note's
        // meaningful range, were the hint actually carried). Confirm
        // the parser reports `None` and the raw sentinel.
        let bits: &[(u8, u32)] = &[
            (5, 8),    // bsid (base syntax, no Annex D xbsi1 slot)
            (3, 0),    // bsmod
            (3, 7),    // acmod = 3/2
            (2, 0b10), // cmixlev
            (2, 0b01), // surmixlev
            (1, 0),    // lfeon
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // langcode
            (1, 0),    // audprodie
            (1, 0),    // copyrightb
            (1, 0),    // origbs
            (1, 0),    // timecod1e
            (1, 0),    // timecod2e
            (1, 0),    // addbsie
        ];
        let bytes = pack_bits(bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.bsid, 8);
        assert_eq!(b.dmixmod, 0xFF);
        assert!(b.dmixmod_preference.is_none());
        assert!(b.stereo_downmix_preference().is_none());
    }

    /// Annex D `bsid == 6` with `xbsi1e == 1` surfaces the typed
    /// preference. Cover all four wire codepoints round-tripping
    /// through `parse()`.
    #[test]
    fn parse_surfaces_dmixmod_preference_annex_d_all_codepoints() {
        for (code, expected) in [
            (0b00u8, StereoDownmixPreference::NotIndicated),
            (0b01u8, StereoDownmixPreference::LtRtPreferred),
            (0b10u8, StereoDownmixPreference::LoRoPreferred),
            (0b11u8, StereoDownmixPreference::Reserved),
        ] {
            let bits: &[(u8, u32)] = &[
                (5, 6),           // bsid = 6 (Annex D alt syntax)
                (3, 0),           // bsmod
                (3, 7),           // acmod = 3/2 (multi-channel — Annex D xbsi1 slot present)
                (2, 0b10),        // cmixlev
                (2, 0b01),        // surmixlev
                (1, 0),           // lfeon
                (5, 27),          // dialnorm
                (1, 0),           // compre
                (1, 0),           // langcode
                (1, 0),           // audprodie
                (1, 0),           // copyrightb
                (1, 0),           // origbs
                (1, 1),           // xbsi1e
                (2, code as u32), // dmixmod codepoint under test
                (3, 0b010),       // ltrtcmixlev
                (3, 0b010),       // ltrtsurmixlev
                (3, 0b010),       // lorocmixlev
                (3, 0b010),       // lorosurmixlev
                (1, 0),           // xbsi2e
                (1, 0),           // addbsie
            ];
            let bytes = pack_bits(bits);
            let b = parse(&bytes).unwrap();
            assert_eq!(b.bsid, 6);
            assert_eq!(b.dmixmod, code, "raw dmixmod for {expected:?}");
            assert_eq!(b.dmixmod_preference, Some(expected));
            assert_eq!(b.stereo_downmix_preference(), Some(expected));
        }
    }

    /// Annex D `bsid == 6` with `xbsi1e == 0` skips the xbsi1
    /// block — the typed preference is `None` even though the
    /// alternate-syntax wire slot exists in the BSI layout.
    #[test]
    fn parse_leaves_dmixmod_preference_none_when_xbsi1e_clear() {
        let bits: &[(u8, u32)] = &[
            (5, 6),    // bsid = 6
            (3, 0),    // bsmod
            (3, 7),    // acmod = 3/2
            (2, 0b10), // cmixlev
            (2, 0b01), // surmixlev
            (1, 0),    // lfeon
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // langcode
            (1, 0),    // audprodie
            (1, 0),    // copyrightb
            (1, 0),    // origbs
            (1, 0),    // xbsi1e (cleared — block absent)
            (1, 0),    // xbsi2e
            (1, 0),    // addbsie
        ];
        let bytes = pack_bits(bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.bsid, 6);
        assert_eq!(b.dmixmod, 0xFF);
        assert!(b.dmixmod_preference.is_none());
    }

    // ---------------------------------------------------------------
    // Language code (`langcod` / `langcod2`) — §5.4.2.11-12 / 19-20.
    // ---------------------------------------------------------------

    /// [`LanguageCode::from_raw`] is a verbatim wrapper — every byte
    /// `0x00..=0xFF` is representable, and `raw()` returns the same
    /// value back.
    #[test]
    fn language_code_from_raw_round_trips_every_byte() {
        for raw in 0u8..=255 {
            let lc = LanguageCode::from_raw(raw);
            assert_eq!(lc.raw(), raw, "raw={raw:#04x}");
        }
    }

    /// Per §5.4.2.12 the spec-mandated wire value is `0xFF`. The
    /// `is_spec_reserved_value` predicate must hold only for that
    /// byte and reject everything else.
    #[test]
    fn language_code_is_spec_reserved_only_for_0xff() {
        for raw in 0u8..=255 {
            let lc = LanguageCode::from_raw(raw);
            let want = raw == 0xFF;
            assert_eq!(
                lc.is_spec_reserved_value(),
                want,
                "raw={raw:#04x} should be spec_reserved={want}"
            );
        }
    }

    /// `parse()` surfaces `langcode == 1` into a typed
    /// [`LanguageCode`] with the 8-bit `langcod` byte taken verbatim
    /// from the wire. Build a 1/0 mono BSI (`acmod=1`) with
    /// `langcode=1`, `langcod=0xFF` (spec-conforming), and verify the
    /// typed surface holds and `is_spec_reserved_value` matches.
    #[test]
    fn parse_surfaces_language_code_when_langcode_flag_set() {
        let bits: [(u8, u32); 16] = [
            (5, 8),    // bsid
            (3, 0),    // bsmod
            (3, 1),    // acmod = 1/0 mono → no cmix/surmix/dsurmod
            (1, 0),    // lfeon
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 1),    // langcode = 1
            (8, 0xFF), // langcod = 0xFF (spec-conforming)
            (1, 0),    // audprodie = 0
            (1, 0),    // copyrightb
            (1, 0),    // origbs
            (1, 0),    // timecod1e
            (1, 0),    // timecod2e
            (1, 0),    // addbsie
            (1, 0),    // pad
            (1, 0),    // pad
        ];
        let bytes = pack_bits(&bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.acmod, 1);
        let lc = b
            .language_code
            .expect("langcode=1 should surface language_code");
        assert_eq!(lc.raw(), 0xFF);
        assert!(lc.is_spec_reserved_value());
        // Same value reachable via the typed accessor.
        assert_eq!(b.language_code(), Some(lc));
        // Not 1+1 dual-mono → no Ch2 mirror.
        assert!(b.language_code_ch2.is_none());
        assert!(b.language_code_ch2().is_none());
    }

    /// `parse()` surfaces a non-`0xFF` `langcod` legacy-encoded byte
    /// verbatim and `is_spec_reserved_value` reports `false` so a
    /// probe / archive tool can flag the stream. Build a 1/0 mono
    /// BSI with `langcode=1`, `langcod=0x42` (a 1995-era table-lookup
    /// codepoint).
    #[test]
    fn parse_surfaces_non_reserved_language_code_byte() {
        let bits: [(u8, u32); 16] = [
            (5, 8),    // bsid
            (3, 0),    // bsmod
            (3, 1),    // acmod = 1/0 mono
            (1, 0),    // lfeon
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 1),    // langcode = 1
            (8, 0x42), // langcod = legacy codepoint
            (1, 0),    // audprodie
            (1, 0),    // copyrightb
            (1, 0),    // origbs
            (1, 0),    // timecod1e
            (1, 0),    // timecod2e
            (1, 0),    // addbsie
            (1, 0),    // pad
            (1, 0),    // pad
        ];
        let bytes = pack_bits(&bits);
        let b = parse(&bytes).unwrap();
        let lc = b
            .language_code
            .expect("langcode=1 should surface language_code");
        assert_eq!(lc.raw(), 0x42);
        assert!(!lc.is_spec_reserved_value());
    }

    /// `langcode == 0` leaves [`Bsi::language_code`] as `None` — no
    /// `langcod` byte follows on the wire. Use a minimal 2/0 stereo
    /// shape (the same fixture used by
    /// `parses_minimal_2_0_stereo_bsi`).
    #[test]
    fn parse_leaves_language_code_none_when_langcode_flag_clear() {
        let bits: [(u8, u32); 14] = [
            (5, 0b01000),
            (3, 0b000),
            (3, 0b010),
            (2, 0b00),
            (1, 0),
            (5, 27),
            (1, 0), // compre
            (1, 0), // langcode = 0 → no langcod byte
            (1, 0), // audprodie
            (1, 0), // copyrightb
            (1, 0), // origbs
            (1, 0),
            (1, 0),
            (1, 0),
        ];
        let bytes = pack_bits(&bits);
        let b = parse(&bytes).unwrap();
        assert!(b.language_code.is_none());
        assert!(b.language_code_ch2.is_none());
        assert!(b.language_code().is_none());
        assert!(b.language_code_ch2().is_none());
    }

    /// 1+1 dual-mono (`acmod == 0`) emits an independent `langcod2e`
    /// flag and (when set) the Ch2 `langcod2` byte. Build a 1+1
    /// stream with `langcode=1` / `langcod=0xFF` and
    /// `langcod2e=1` / `langcod2=0xFF`, and verify both typed
    /// surfaces hold.
    #[test]
    fn parse_surfaces_language_code_ch2_in_1_plus_1() {
        let bits: &[(u8, u32)] = &[
            (5, 8),    // bsid
            (3, 0),    // bsmod
            (3, 0),    // acmod = 0 (1+1 dual mono) — no cmix/surmix/dsurmod
            (1, 0),    // lfeon
            (5, 27),   // dialnorm (Ch1)
            (1, 0),    // compre
            (1, 1),    // langcode = 1
            (8, 0xFF), // langcod = 0xFF
            (1, 0),    // audprodie = 0
            (5, 27),   // dialnorm2 (Ch2)
            (1, 0),    // compr2e
            (1, 1),    // langcod2e = 1
            (8, 0xFF), // langcod2 = 0xFF
            (1, 0),    // audprodi2e
            (1, 0),    // copyrightb
            (1, 0),    // origbs
            (1, 0),    // timecod1e
            (1, 0),    // timecod2e
            (1, 0),    // addbsie
        ];
        let bytes = pack_bits(bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.acmod, 0);
        let lc1 = b
            .language_code
            .expect("langcode=1 should surface language_code");
        assert_eq!(lc1.raw(), 0xFF);
        assert!(lc1.is_spec_reserved_value());
        let lc2 = b
            .language_code_ch2
            .expect("langcod2e=1 should surface language_code_ch2");
        assert_eq!(lc2.raw(), 0xFF);
        assert!(lc2.is_spec_reserved_value());
    }

    /// In a 1+1 dual-mono stream with `langcode=0` / `langcod2e=0`,
    /// both typed surfaces stay `None`. Confirms the per-channel
    /// gates are independent of each other.
    #[test]
    fn parse_leaves_language_code_ch2_none_when_langcod2e_clear() {
        let bits: &[(u8, u32)] = &[
            (5, 8),  // bsid
            (3, 0),  // bsmod
            (3, 0),  // acmod = 0 (1+1)
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (5, 27), // dialnorm2
            (1, 0),  // compr2e
            (1, 0),  // langcod2e
            (1, 0),  // audprodi2e
            (1, 0),  // copyrightb
            (1, 0),  // origbs
            (1, 0),  // timecod1e
            (1, 0),  // timecod2e
            (1, 0),  // addbsie
        ];
        let bytes = pack_bits(bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.acmod, 0);
        assert!(b.language_code.is_none());
        assert!(b.language_code_ch2.is_none());
    }

    // ---------------------------------------------------------------
    // CenterMixLevel / SurroundMixLevel — §5.4.2.4-5 typed surfaces.
    // ---------------------------------------------------------------

    /// Every Table 5.9 codepoint round-trips through `from_code` /
    /// `raw` and resolves to its spec-documented linear coefficient.
    /// The reserved codepoint returns `None` from `coefficient()` and
    /// the spec's intermediate-value substitution from
    /// `coefficient_with_reserved_fallback`.
    #[test]
    fn center_mix_level_table_5_9_round_trip() {
        let cases: [(u8, CenterMixLevel, Option<f32>, f32); 4] = [
            (0b00, CenterMixLevel::Minus3Db, Some(0.707), 0.707),
            (0b01, CenterMixLevel::Minus4Point5Db, Some(0.595), 0.595),
            (0b10, CenterMixLevel::Minus6Db, Some(0.500), 0.500),
            (0b11, CenterMixLevel::Reserved, None, 0.595),
        ];
        for (raw, want_variant, want_coef, want_fallback) in cases {
            let v = CenterMixLevel::from_code(raw);
            assert_eq!(v, want_variant, "raw {raw:02b}");
            assert_eq!(v.raw(), raw, "raw round-trip {raw:02b}");
            assert_eq!(v.coefficient(), want_coef);
            assert!(
                (v.coefficient_with_reserved_fallback() - want_fallback).abs() < 1e-6,
                "fallback for {raw:02b}: {} vs {want_fallback}",
                v.coefficient_with_reserved_fallback()
            );
            assert_eq!(v.is_reserved(), raw == 0b11);
        }
        // `from_code` truncates anything outside the 2-bit codespace
        // to its low 2 bits — `0xFF & 0x3 == 0b11` is reserved.
        assert_eq!(CenterMixLevel::from_code(0xFF), CenterMixLevel::Reserved);
    }

    /// Every Table 5.10 codepoint round-trips through `from_code` /
    /// `raw` and resolves to its spec-documented linear coefficient.
    /// The reserved codepoint returns `None` from `coefficient()` and
    /// the spec's intermediate-value substitution from
    /// `coefficient_with_reserved_fallback`. The mute codepoint
    /// `'10'` collapses to coefficient `0.0` and is flagged by
    /// `is_mute()`.
    #[test]
    fn surround_mix_level_table_5_10_round_trip() {
        let cases: [(u8, SurroundMixLevel, Option<f32>, f32); 4] = [
            (0b00, SurroundMixLevel::Minus3Db, Some(0.707), 0.707),
            (0b01, SurroundMixLevel::Minus6Db, Some(0.500), 0.500),
            (0b10, SurroundMixLevel::Mute, Some(0.000), 0.000),
            (0b11, SurroundMixLevel::Reserved, None, 0.500),
        ];
        for (raw, want_variant, want_coef, want_fallback) in cases {
            let v = SurroundMixLevel::from_code(raw);
            assert_eq!(v, want_variant, "raw {raw:02b}");
            assert_eq!(v.raw(), raw, "raw round-trip {raw:02b}");
            assert_eq!(v.coefficient(), want_coef);
            assert!(
                (v.coefficient_with_reserved_fallback() - want_fallback).abs() < 1e-6,
                "fallback for {raw:02b}: {} vs {want_fallback}",
                v.coefficient_with_reserved_fallback()
            );
            assert_eq!(v.is_reserved(), raw == 0b11);
            assert_eq!(v.is_mute(), raw == 0b10);
        }
        // 2-bit truncation: low 2 bits of `0xFF` are `0b11` (reserved).
        assert_eq!(
            SurroundMixLevel::from_code(0xFF),
            SurroundMixLevel::Reserved
        );
    }

    /// A 3/2 (acmod=7) syncframe carries both `cmixlev` (3 front
    /// channels gate satisfied) and `surmixlev` (surround gate
    /// satisfied), so both typed surfaces become `Some` after
    /// `parse()` and match the raw codepoint round-trip.
    #[test]
    fn parse_surfaces_center_and_surround_mix_on_3_2() {
        // Exercise the non-default codepoints to prove the bit
        // positions are correct: cmixlev = 0b10 (-6 dB),
        // surmixlev = 0b01 (-6 dB).
        let bits: &[(u8, u32)] = &[
            (5, 8),    // bsid
            (3, 0),    // bsmod
            (3, 7),    // acmod = 3/2
            (2, 0b10), // cmixlev = -6 dB (0.500)
            (2, 0b01), // surmixlev = -6 dB (0.500)
            // dsurmod absent (acmod != 2)
            (1, 1),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 0),  // copyrightb
            (1, 0),  // origbs
            (1, 0),  // timecod1e
            (1, 0),  // timecod2e
            (1, 0),  // addbsie
        ];
        let bytes = pack_bits(bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.acmod, 7);
        assert_eq!(b.cmixlev, 0b10);
        assert_eq!(b.surmixlev, 0b01);
        let center = b.center_mix.expect("3/2 carries cmixlev");
        assert_eq!(center, CenterMixLevel::Minus6Db);
        assert_eq!(center.raw(), 0b10);
        assert_eq!(center.coefficient(), Some(0.500));
        // Accessor view matches the field view.
        assert_eq!(b.center_mix(), Some(CenterMixLevel::Minus6Db));
        let surround = b.surround_mix.expect("3/2 carries surmixlev");
        assert_eq!(surround, SurroundMixLevel::Minus6Db);
        assert_eq!(surround.raw(), 0b01);
        assert_eq!(surround.coefficient(), Some(0.500));
        assert_eq!(b.surround_mix(), Some(SurroundMixLevel::Minus6Db));
    }

    /// A 2/0 stereo (acmod=2) syncframe carries neither `cmixlev` nor
    /// `surmixlev` — the §5.3.2 guards skip both 2-bit slots — so both
    /// typed surfaces stay `None` and the raw fields keep the
    /// "absent" sentinel `0xFF`. This is the same fixture as
    /// `parses_minimal_2_0_stereo_bsi` extended with the typed
    /// assertions.
    #[test]
    fn parse_center_and_surround_mix_none_when_acmod_2_0() {
        let bits: [(u8, u32); 14] = [
            (5, 0b01000),
            (3, 0b000),
            (3, 0b010), // acmod = 2/0
            (2, 0b00),  // dsurmod
            (1, 0),
            (5, 27),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
        ];
        let bytes = pack_bits(&bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.acmod, 2);
        assert_eq!(b.cmixlev, 0xFF);
        assert_eq!(b.surmixlev, 0xFF);
        assert!(b.center_mix.is_none());
        assert!(b.surround_mix.is_none());
        assert!(b.center_mix().is_none());
        assert!(b.surround_mix().is_none());
    }

    /// A 1/0 mono (acmod=1) syncframe carries neither slot. The
    /// `(acmod & 0x1) != 0 && acmod != 0x1` cmixlev guard rejects
    /// `acmod==1` and the `(acmod & 0x4) != 0` surmixlev guard
    /// rejects it too — so both typed surfaces stay `None`.
    #[test]
    fn parse_center_and_surround_mix_none_when_acmod_1_0_mono() {
        let bits: &[(u8, u32)] = &[
            (5, 8),
            (3, 0),
            (3, 1), // acmod = 1/0 mono
            // no cmixlev, no surmixlev, no dsurmod
            (1, 0),  // lfeon
            (5, 27), // dialnorm
            (1, 0),  // compre
            (1, 0),  // langcode
            (1, 0),  // audprodie
            (1, 0),  // copyrightb
            (1, 0),  // origbs
            (1, 0),  // timecod1e
            (1, 0),  // timecod2e
            (1, 0),  // addbsie
        ];
        let bytes = pack_bits(bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.acmod, 1);
        assert_eq!(b.cmixlev, 0xFF);
        assert_eq!(b.surmixlev, 0xFF);
        assert!(b.center_mix.is_none());
        assert!(b.surround_mix.is_none());
    }

    /// A 2/2 (acmod=6) syncframe is the asymmetric case — the
    /// `cmixlev` guard fails (no centre channel, `acmod & 0x1 == 0`)
    /// but the `surmixlev` guard passes (surround present,
    /// `acmod & 0x4 != 0`). Confirms each typed surface is gated
    /// independently and the surround codeword is decoded
    /// correctly.
    #[test]
    fn parse_surfaces_only_surround_mix_on_2_2() {
        let bits: &[(u8, u32)] = &[
            (5, 8),
            (3, 0),
            (3, 6), // acmod = 2/2 (L, R, Ls, Rs)
            // cmixlev absent (acmod & 0x1 == 0)
            (2, 0b10), // surmixlev = Mute
            (1, 0),    // lfeon
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // langcode
            (1, 0),    // audprodie
            (1, 0),    // copyrightb
            (1, 0),    // origbs
            (1, 0),    // timecod1e
            (1, 0),    // timecod2e
            (1, 0),    // addbsie
        ];
        let bytes = pack_bits(bits);
        let b = parse(&bytes).unwrap();
        assert_eq!(b.acmod, 6);
        assert_eq!(b.cmixlev, 0xFF);
        assert_eq!(b.surmixlev, 0b10);
        assert!(b.center_mix.is_none());
        let surround = b.surround_mix.expect("2/2 carries surmixlev");
        assert_eq!(surround, SurroundMixLevel::Mute);
        assert!(surround.is_mute());
        assert_eq!(surround.coefficient(), Some(0.0));
    }
}

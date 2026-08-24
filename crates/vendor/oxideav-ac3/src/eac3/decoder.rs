//! E-AC-3 syncframe decoder — rounds 1 + 2 + 3.
//!
//! Round-1 path:
//!
//! 1. Verify the 16-bit syncword (`0x0B77`).
//! 2. Parse the [`super::bsi`] BSI — channel layout, sample rate,
//!    frame size, dialnorm, etc.
//! 3. Parse the [`super::audfrm`] audio-frame element — strategy
//!    flags only (no DSP yet).
//! 4. Emit `bsi.num_blocks × 256 × nchans` interleaved S16 zeros.
//!
//! Round-2 path:
//!
//! 5. Hand the BitReader to [`super::dsp::decode_indep_audblks`],
//!    which walks per-block side-info, runs §7 bit allocation +
//!    mantissa unpack, and applies IMDCT + window + overlap-add via
//!    the AC-3 helpers. On any "unsupported feature" error
//!    (coupling, SPX, AHT, transient processing, …) the decoder
//!    falls back to silent emit so the corpus driver keeps decoding.
//!
//! Round-3 path (this commit):
//!
//! 6. Walk dependent substreams in the same packet, decode each via
//!    the round-2 DSP, and **splice** the resulting channels into
//!    [`Eac3DecoderState::indep_pcm_f32`] per the §E.3.8.2 channel-
//!    and-program-extension rule: each dep coded channel is routed by
//!    its Table E2.5 location (or natural `acmod` order when
//!    `chanmape == 0`); a location already present in the indep
//!    program *replaces* that channel in place, a new location
//!    *extends* the output. The replace-vs-extend partition is what
//!    keeps a real greater-than-5.1 broadcast stream spatially
//!    correct — a dep substream commonly re-codes Center / LFE (or,
//!    with a custom chanmap, even L/R) that the indep 5.1 downmix
//!    already carries, and a naive blind-append would duplicate and
//!    decorrelate those channels.
//!    None of the validator-encoded corpus fixtures exercise dep
//!    substreams (the corpus omits them per
//!    `eac3-5.1-side-768kbps/notes.md`); the in-tree 7.1 encoder
//!    (indep 5.1 + dep [Lrs/Rrs] pair) and the
//!    `splice_*_replace`/`_extend` unit tests cover the path.

use oxideav_core::bits::BitReader;
use oxideav_core::{Error, Result};

use crate::audblk::{Ac3State, SAMPLES_PER_BLOCK};

use super::audfrm::{self, AudFrm};
use super::bsi::{self, Bsi as Eac3Bsi, StreamType};
use super::chanmap::{self, ChannelLocation};
use super::dsp;

/// E-AC-3 syncword — same value as base AC-3 (§E.2.2.1).
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub const SYNCWORD: u16 = 0x0B77;

/// Per-decoder state that persists across packets.
///
/// Round 2 adds [`Ac3State`] — the per-channel exponent / bap /
/// overlap-add-delay state shared with the AC-3 decoder. We carry one
/// `Ac3State` per substream id so dependent + independent substreams
/// don't trample each other's delay lines (round 3 wires this).
#[derive(Default, Clone)]
pub struct Eac3DecoderState {
    /// Last successfully-decoded indep substream parameters. Used by
    /// dependent substreams to know how many channels to extend.
    pub last_indep: Option<IndepProgramShape>,
    /// Per-channel persistent DSP state for the **independent**
    /// substream. Carries exponent reuse + 256-sample IMDCT delay line
    /// across blocks and frames.
    indep_state: Ac3State,
    /// Per-channel persistent DSP state for the **dependent** substream.
    /// Held separately so a dep-substream block's reuse-exponent path
    /// doesn't read indep exponents.
    dep_state: Ac3State,
    /// Per-frame f32 PCM scratch — the indep substream's PCM in its
    /// native layout immediately after [`decode_indep_substream`]; if
    /// any dep substreams follow in the same packet, their channels
    /// are spliced in by [`decode_dep_substream`], growing this slot
    /// to `indep_nchans + dep_nchans`.
    indep_pcm_f32: Vec<f32>,
    /// Current channel count of [`Self::indep_pcm_f32`] — starts at
    /// the indep BSI's `nchans` and grows as dep substreams splice
    /// their channels in.
    indep_nchans: u16,
    /// Physical channel locations of the **independent** substream's
    /// own channels, in `indep_pcm_f32` slot order (natural
    /// `acmod`/`lfeon` order, Table 5.8). Populated by
    /// [`decode_indep_substream`]. Used by [`splice_dep_into_indep`]
    /// to implement the §E.3.8.2 dependent-substream combination rule:
    /// a dep channel whose location already exists in this list
    /// *replaces* the indep channel in place rather than being appended.
    indep_locations: Vec<ChannelLocation>,
    /// Samples-per-frame (per channel) of [`Self::indep_pcm_f32`].
    indep_samples_per_frame: u32,
    /// Per-frame error string (last seen). Diagnostic only.
    pub last_error: Option<String>,
    /// Physical channel locations of dep-substream channels that were
    /// spliced into [`Self::indep_pcm_f32`] during the most recent
    /// packet, in the order they were appended. Always one entry per
    /// dep coded channel, matching the §E.2.3.1.8 chanmap expansion
    /// (Table E2.5). Empty when no dep substream contributed, or when
    /// the dep substream's `chanmape == 0` (in which case the dep
    /// channels are routed by the spec's "channel locations apply in
    /// the natural order" default for the dep's `acmod` / `lfeon` —
    /// see [`Self::default_dep_locations`]).
    pub dep_locations: Vec<ChannelLocation>,
}

impl Eac3DecoderState {
    /// Read-only view of the indep substream's per-frame interleaved
    /// f32 PCM (range -1..1). After [`decode_eac3_packet`] this is the
    /// indep substream's PCM in `(acmod, lfeon)` bitstream order; if
    /// any dep substreams contributed in the same packet, their
    /// channels have been combined in per §E.3.8.2 — replacing the
    /// matching indep slot when the location is shared, or extending
    /// the buffer when it is new (see
    /// [`crate::eac3::decoder::splice_dep_into_indep`]).
    ///
    /// `process_eac3_frame` consumes this directly when running the
    /// §7.8 downmix matrix so it can apply per-channel coefficients
    /// before quantising to S16LE — matrix×S16 would lose 6+ dB of
    /// headroom per cascade step on negatively-signed weights.
    pub fn indep_pcm_f32(&self) -> &[f32] {
        &self.indep_pcm_f32
    }

    /// Current channel count of [`Self::indep_pcm_f32`].
    pub fn indep_nchans(&self) -> u16 {
        self.indep_nchans
    }

    /// Propagate §7.7 DRC control settings into both the independent and
    /// dependent substream DSP states so the next packet's `dynrng` /
    /// `compr` decode (in [`crate::eac3::dsp`]) applies the requested
    /// regime. Called by the decoder whenever the caller changes its DRC
    /// configuration.
    pub fn set_drc(&mut self, drc: crate::drc::DrcSettings) {
        self.indep_state.drc = drc;
        self.dep_state.drc = drc;
    }

    /// Channel-location list assigned to the dep substream when
    /// `chanmape == 0` (no custom channel map present), per
    /// §E.2.3.1.7: "the channel map for a dependent substream shall
    /// be defined by the audio coding mode" (the dep substream's
    /// `acmod` + `lfeon`).
    ///
    /// Maps the dep substream's `acmod` to the natural Table 5.8
    /// channel order (L, C, R, Ls, Rs, …) plus LFE when `lfeon`.
    /// `acmod == 0` (1+1 dual mono) is treated as two anonymous
    /// channels assigned to `Left` / `Right` slots.
    pub fn default_dep_locations(acmod: u8, lfeon: bool) -> Vec<ChannelLocation> {
        let mut out: Vec<ChannelLocation> = Vec::with_capacity(6);
        match acmod {
            0 => {
                // 1+1 dual mono — two independent channels with no
                // canonical assignment. Default to L / R.
                out.push(ChannelLocation::Left);
                out.push(ChannelLocation::Right);
            }
            1 => {
                // 1/0 mono — Center.
                out.push(ChannelLocation::Center);
            }
            2 => {
                // 2/0 stereo — L, R.
                out.push(ChannelLocation::Left);
                out.push(ChannelLocation::Right);
            }
            3 => {
                // 3/0 — L, C, R.
                out.push(ChannelLocation::Left);
                out.push(ChannelLocation::Center);
                out.push(ChannelLocation::Right);
            }
            4 => {
                // 2/1 — L, R, S (treated as center-surround per
                // Table E2.5 bit 7).
                out.push(ChannelLocation::Left);
                out.push(ChannelLocation::Right);
                out.push(ChannelLocation::CenterSurround);
            }
            5 => {
                // 3/1 — L, C, R, S.
                out.push(ChannelLocation::Left);
                out.push(ChannelLocation::Center);
                out.push(ChannelLocation::Right);
                out.push(ChannelLocation::CenterSurround);
            }
            6 => {
                // 2/2 — L, R, Ls, Rs.
                out.push(ChannelLocation::Left);
                out.push(ChannelLocation::Right);
                out.push(ChannelLocation::LeftSurround);
                out.push(ChannelLocation::RightSurround);
            }
            7 => {
                // 3/2 — L, C, R, Ls, Rs.
                out.push(ChannelLocation::Left);
                out.push(ChannelLocation::Center);
                out.push(ChannelLocation::Right);
                out.push(ChannelLocation::LeftSurround);
                out.push(ChannelLocation::RightSurround);
            }
            _ => {}
        }
        if lfeon {
            out.push(ChannelLocation::Lfe);
        }
        out
    }
}

/// Snapshot of the most recent independent substream's output shape
/// — channels, sample rate, samples per frame.
#[derive(Clone, Debug)]
pub struct IndepProgramShape {
    pub nchans: u16,
    pub sample_rate: u32,
    pub samples_per_frame: u32,
}

/// Result of decoding one E-AC-3 packet.
#[derive(Clone, Debug)]
pub struct DecodedFrame {
    pub sample_rate: u32,
    pub channels: u16,
    /// PCM samples per channel (= `bsi.num_blocks * 256`).
    pub samples: u32,
    /// Interleaved S16LE bytes — `samples * channels * 2` total.
    pub pcm_s16le: Vec<u8>,
    /// Independent substream's `acmod` field (Table E1.2 §E.1.2.1).
    /// Surfaced so callers that need to reorder bitstream-order
    /// multichannel layouts into WAV-mask order can pick the right
    /// permutation via [`crate::wave_order`]. For dep-substream-extended
    /// programs (e.g. 7.1 emitted as indep 5.1 + dep [Lb,Rb]) this
    /// reflects the **indep** acmod only — any dep channels at *new*
    /// locations are appended at the end of the PCM buffer per the
    /// §E.3.8.2 rule in `splice_dep_into_indep` and are not covered by
    /// Table 5.8 (dep channels at a *shared* location replace the
    /// matching indep slot in place and do not grow the layout).
    pub acmod: u8,
    /// Independent substream's `lfeon` flag (1 bit, §E.1.2.1).
    pub lfeon: bool,
    /// Independent substream's `nfchans` (= `acmod_nfchans(acmod)`).
    /// Surfaced so callers building a [`crate::downmix::Downmix`] don't
    /// have to re-derive it from `channels - lfeon as u16`.
    pub nfchans: u8,
    /// Independent substream's Annex E mixmdata refinement
    /// (§E.2.3.1.3-6). `Some` only when `mixmdate == 1` AND at least
    /// one of the four 3-bit codes' per-channel guards fired
    /// (`acmod > 2` for center codes, `acmod & 0x4` for surround
    /// codes). When `Some`, the [`crate::downmix::Downmix::from_eac3_bsi`]
    /// constructor uses these as overrides for the §7.8.2 fixed-0.707
    /// LtRt defaults and the 0.707 LoRo defaults.
    pub annex_e_mix_levels: Option<crate::bsi::AnnexDMixLevels>,
    /// Physical channel locations for the dep-substream channels that
    /// *extended* the indep program (i.e. were appended because their
    /// location was new), in append order (§E.3.8.2 / §E.2.3.1.7-8).
    /// One entry per extending dep coded channel, derived from
    /// `chanmap` (Table E2.5) when `chanmape == 1` or from the dep
    /// substream's `acmod`/`lfeon` natural order when `chanmape == 0`.
    /// Empty when no dep substream contributed, or when every dep
    /// channel *replaced* a shared indep location (§E.3.8.2 replace).
    ///
    /// The indep program's channels occupy slots `0..indep_nchans`
    /// (replacements overwrite the matching slot in place); the
    /// extending dep channels occupy the appended slots in this order.
    pub dep_locations: Vec<ChannelLocation>,
    /// Independent substream's 5-bit `dialnorm` word (§E.2.3.1.x, headroom
    /// in dB below digital 100%, `1..=31`). Surfaced so a caller that owns
    /// the playback gain can apply §7.6 dialogue normalisation toward a
    /// chosen target level. Advisory — the core decode does not scale by
    /// it (the spec leaves dialnorm to the reproduction system).
    pub dialnorm: u8,
}

/// Decode one or more concatenated E-AC-3 syncframes contained in a
/// single packet (a "transport syncframe" pair: indep + dep).
///
/// The packet must begin with a 16-bit `0x0B77` syncword. Subsequent
/// syncframes are located via the BSI's `frame_bytes` field. Returns
/// the **independent** substream's program PCM. Dependent substreams
/// are parsed; round 3 will splice their channels into the indep PCM.
pub fn decode_eac3_packet(state: &mut Eac3DecoderState, data: &[u8]) -> Result<DecodedFrame> {
    if data.len() < 4 {
        return Err(Error::invalid("eac3: packet too short for syncinfo"));
    }
    let mut indep_pcm: Option<DecodedFrame> = None;
    let mut off = 0usize;
    state.last_error = None;
    state.dep_locations.clear();
    while off + 4 <= data.len() {
        // §E.2.2.1 — syncword.
        let sync = u16::from_be_bytes([data[off], data[off + 1]]);
        if sync != SYNCWORD {
            return Err(Error::invalid(format!(
                "eac3: bad syncword 0x{sync:04X} at offset {off} (expected 0x0B77)"
            )));
        }
        // BSI starts at byte off+2.
        let bsi_data = &data[off + 2..];
        let mut br = BitReader::new(bsi_data);
        let bsi = bsi::parse_with(&mut br)?;
        let frame_bytes = bsi.frame_bytes as usize;
        if off + frame_bytes > data.len() {
            return Err(Error::invalid(format!(
                "eac3: syncframe at offset {off} claims {frame_bytes} bytes but packet has only {}",
                data.len() - off
            )));
        }

        // Audfrm follows BSI in the same bit cursor. AHT (audfrm
        // returning Unsupported) is recoverable: the silent emit path
        // handles it by emitting zeros for this frame instead of
        // bailing the whole packet.
        let audfrm = match audfrm::parse_with(&mut br, &bsi) {
            Ok(a) => a,
            Err(e) => {
                state.last_error = Some(format!("{e}"));
                let pcm = build_silent_indep(&bsi)?;
                if matches!(
                    bsi.strmtyp,
                    StreamType::Independent | StreamType::Ac3Convert
                ) {
                    state.last_indep = Some(IndepProgramShape {
                        nchans: pcm.channels,
                        sample_rate: pcm.sample_rate,
                        samples_per_frame: pcm.samples,
                    });
                    indep_pcm = Some(pcm);
                }
                off += frame_bytes;
                continue;
            }
        };

        match bsi.strmtyp {
            StreamType::Independent | StreamType::Ac3Convert => {
                let pcm = decode_indep_substream(state, &bsi, &audfrm, &mut br)?;
                state.last_indep = Some(IndepProgramShape {
                    nchans: pcm.channels,
                    sample_rate: pcm.sample_rate,
                    samples_per_frame: pcm.samples,
                });
                indep_pcm = Some(pcm);
            }
            StreamType::Dependent => {
                // Round 3: decode + splice into indep_pcm_f32. On
                // failure (Unsupported / parse error) we keep the
                // indep PCM as-is so output is still meaningful.
                let _ = decode_dep_substream(state, &bsi, &audfrm, &mut br);
            }
            StreamType::Reserved => {
                return Err(Error::invalid("eac3: strmtyp '11' is reserved"));
            }
        }

        off += frame_bytes;
    }

    let mut pcm = indep_pcm.ok_or_else(|| {
        Error::invalid(
            "eac3: packet contains no independent substream (only dependent or ac3-convert frames)",
        )
    })?;

    // Rebuild the final S16 buffer from the (possibly extended)
    // [`indep_pcm_f32`] scratch. When no dep substream was seen, this
    // is the indep PCM unchanged; when one or more dep substreams
    // contributed, the scratch has grown to `indep_nchans + Σ
    // dep_nchans` channels.
    if state.indep_nchans != pcm.channels {
        pcm.channels = state.indep_nchans;
        pcm.pcm_s16le = pack_f32_to_s16le(&state.indep_pcm_f32);
    }
    // Surface the accumulated dep-channel locations so callers know
    // the physical assignment of the appended channels without
    // re-parsing the chanmap. Empty when no dep substream
    // contributed.
    pcm.dep_locations.clone_from(&state.dep_locations);
    Ok(pcm)
}

/// Decode one independent substream's audblks. Tries the round-2 DSP
/// path first; on `Error::Unsupported` (or any other DSP error) falls
/// back to silent PCM of the right shape so the corpus driver sees
/// a frame-count match.
fn decode_indep_substream(
    state: &mut Eac3DecoderState,
    bsi: &Eac3Bsi,
    audfrm: &AudFrm,
    br: &mut BitReader<'_>,
) -> Result<DecodedFrame> {
    let samples = bsi.num_blocks as u32 * SAMPLES_PER_BLOCK as u32;
    let nchans = bsi.nchans as usize;
    let mut floats = vec![0.0f32; samples as usize * nchans];

    let dsp_result =
        dsp::decode_indep_audblks(bsi, audfrm, br, &mut state.indep_state, &mut floats);
    if let Err(e) = &dsp_result {
        // Silent fallback. Reset the per-channel exponent reuse state
        // so the next frame's reuse-strategy blocks don't pick up
        // garbage from a half-decoded prior frame.
        state.last_error = Some(format!("{e}"));
        state.indep_state = Ac3State::new();
        for v in floats.iter_mut() {
            *v = 0.0;
        }
    }

    // Cache the indep f32 PCM in its native (acmod, lfeon) layout
    // so any subsequent dep substream in the same packet can append
    // its chanmap-routed channels (round 3).
    state.indep_pcm_f32 = floats;
    state.indep_nchans = nchans as u16;
    state.indep_samples_per_frame = samples;
    // Record the indep substream's own per-channel locations (natural
    // acmod/lfeon order, Table 5.8) so a following dep substream can
    // tell which of its channels *replace* an indep channel (shared
    // location, §E.3.8.2) versus *extend* the program (new location).
    state.indep_locations = Eac3DecoderState::default_dep_locations(bsi.acmod, bsi.lfeon);

    // Pack indep PCM only. If a dep substream follows, the packet-
    // level driver will rebuild the final S16 buffer in
    // `decode_eac3_packet` after all substreams are walked.
    let pcm_s16le = pack_f32_to_s16le(&state.indep_pcm_f32);

    Ok(DecodedFrame {
        sample_rate: bsi.sample_rate,
        channels: bsi.nchans as u16,
        samples,
        pcm_s16le,
        acmod: bsi.acmod,
        lfeon: bsi.lfeon,
        nfchans: bsi.nfchans,
        annex_e_mix_levels: bsi.annex_e_mix_levels,
        // Indep-substream-only emit: no dep channels yet. The
        // packet-level driver overwrites this with the accumulated
        // `state.dep_locations` if dep substreams follow.
        dep_locations: Vec::new(),
        dialnorm: bsi.dialnorm,
    })
}

/// Decode one dependent substream and splice its `chanmap`-routed
/// channels into the indep substream's PCM scratch
/// [`Eac3DecoderState::indep_pcm_f32`].
///
/// Returns `Ok(extended_channel_count)` on success; `Err(...)` if
/// the dep substream uses a feature we can't decode (round-2-style
/// silent-fallback applied to the dep audio, leaving the indep PCM
/// untouched).
fn decode_dep_substream(
    state: &mut Eac3DecoderState,
    bsi: &Eac3Bsi,
    audfrm: &AudFrm,
    br: &mut BitReader<'_>,
) -> Result<u16> {
    let samples = bsi.num_blocks as u32 * SAMPLES_PER_BLOCK as u32;
    let dep_nchans = bsi.nchans as usize;

    // Without an indep substream in the same packet there is nothing
    // to extend. Per §E.2.3.1.1 a dep substream must follow an
    // indep substream; flag it but don't error.
    if state.last_indep.is_none() {
        return Err(Error::invalid(
            "eac3 dep: dependent substream with no preceding independent substream",
        ));
    }
    if state.indep_samples_per_frame != samples {
        return Err(Error::invalid(format!(
            "eac3 dep: dep substream sample-count {samples} differs from indep {}",
            state.indep_samples_per_frame
        )));
    }

    // Resolve the dep substream's per-channel location list per
    // §E.2.3.1.7-8 BEFORE decoding the DSP. When `chanmape == 1` the
    // chanmap field selects locations from Table E2.5 and the
    // expanded count MUST equal `dep_nchans` (spec invariant). When
    // `chanmape == 0` the locations default to the natural-acmod
    // order (e.g. acmod=2 → [Left, Right]).
    let dep_locations = match bsi.chanmap {
        Some(map) => match chanmap::expand_chanmap_locations(map, dep_nchans as u8) {
            Ok(locs) => locs,
            Err(e) => {
                state.last_error = Some(format!("eac3 dep chanmap: {e}"));
                return Err(Error::invalid(format!("eac3 dep chanmap: {e}")));
            }
        },
        None => Eac3DecoderState::default_dep_locations(bsi.acmod, bsi.lfeon),
    };

    // Decode the dep substream into its own f32 buffer. The dep
    // substream's audblks have the same syntax (Table E1.4 doesn't
    // branch on strmtyp).
    let mut dep_floats = vec![0.0f32; samples as usize * dep_nchans];
    if let Err(e) =
        dsp::decode_indep_audblks(bsi, audfrm, br, &mut state.dep_state, &mut dep_floats)
    {
        // Silent fallback for the dep substream — leave indep PCM
        // untouched so the indep program is still audible.
        state.last_error = Some(format!("{e}"));
        state.dep_state = Ac3State::new();
        return Err(e);
    }

    // Splice channels per §E.3.8.2: dep channels whose location already
    // exists in the indep program *replace* the indep channel in place;
    // dep channels with a new location *extend* the output. The
    // `dep_locations` list (resolved above from chanmap / acmod) gives
    // each dep coded channel its physical location.
    let extended_locations =
        splice_dep_into_indep(state, dep_nchans, bsi.chanmap, &dep_floats, &dep_locations);

    // Record only the *extending* dep-channel locations (the ones that
    // grew the output past the indep program) so callers (e.g. a future
    // WAV-mask reorderer or the §7.8 downmix when extended to 7.1) can
    // find Lb/Rb without re-parsing the chanmap. Replaced channels are
    // not appended, so they do not appear here.
    state.dep_locations.extend(extended_locations);

    Ok(state.indep_nchans)
}

/// Pack interleaved f32 PCM (range -1..1) into S16LE bytes.
fn pack_f32_to_s16le(floats: &[f32]) -> Vec<u8> {
    let mut out = vec![0u8; floats.len() * 2];
    for (i, s) in floats.iter().enumerate() {
        let clamped = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
        let le = clamped.to_le_bytes();
        out[i * 2] = le[0];
        out[i * 2 + 1] = le[1];
    }
    out
}

/// Splice the dep substream's channels into the indep PCM scratch per
/// the §E.3.8.2 channel-and-program-extension combination rule.
///
/// A/52:2018 §E.3.8.2 (Decoding a Single Program with Greater than 5.1
/// Channels):
///
/// > If channels are present in the dependent substream that
/// > correspond to channels in the associated independent substream,
/// > then the dependent substream data for those channels replaces the
/// > independent substream data for the corresponding channels. All
/// > channels present in the dependent substream that do not correspond
/// > to channels in the independent substream are used to enable output
/// > for speaker configurations with greater than 5.1 channels.
///
/// The same paragraph also covers `chanmape == 0`, where the dep
/// substream's `acmod`/`lfeon` identify its channels and "the
/// corresponding audio channels in the independent substream are
/// overwritten with the dependent audio channel data". Both cases
/// therefore reduce to: for each dep coded channel, *replace in place*
/// when its [`ChannelLocation`] already exists in the indep program,
/// otherwise *append* it as a new output channel.
///
/// Replacing rather than blind-appending is what keeps a real
/// greater-than-5.1 broadcast stream spatially correct — a dep
/// substream commonly re-codes the Center / LFE (or, with a custom
/// chanmap, even L/R; Table E2.5 shaded entries) that the indep
/// program already carries. A naive append duplicates and reorders
/// those channels, decorrelating the rendered layout.
///
/// `dep_locations[ch]` gives the physical location of dep coded
/// channel `ch` (resolved from the chanmap, or the natural acmod
/// order when `chanmape == 0`). Returns the locations of only the
/// channels that *extended* the program (the ones that were appended),
/// in append order, so the caller can record them on
/// [`Eac3DecoderState::dep_locations`].
fn splice_dep_into_indep(
    state: &mut Eac3DecoderState,
    dep_nchans: usize,
    chanmap: Option<u16>,
    dep_floats: &[f32],
    dep_locations: &[ChannelLocation],
) -> Vec<ChannelLocation> {
    let samples = state.indep_samples_per_frame as usize;
    let indep_nchans = state.indep_nchans as usize;

    // Partition the dep channels into "replace existing indep slot"
    // versus "extend". For replacement, find the indep slot index that
    // carries the same location. The indep_locations list is in
    // indep_pcm_f32 slot order, so its index *is* the slot index.
    // Defensive: if locations weren't recorded (shouldn't happen on a
    // real decode), fall back to extend-only so we never lose audio.
    let mut replace_slot: Vec<Option<usize>> = Vec::with_capacity(dep_nchans);
    let mut extend_locs: Vec<ChannelLocation> = Vec::new();
    for loc in dep_locations.iter().copied().take(dep_nchans) {
        match state.indep_locations.iter().position(|&l| l == loc) {
            Some(slot) => replace_slot.push(Some(slot)),
            None => {
                replace_slot.push(None);
                extend_locs.push(loc);
            }
        }
    }
    // Any dep channels beyond the resolved location list (malformed /
    // missing locations) are extended with an unknown location to
    // preserve their audio rather than drop it.
    for _ in dep_locations.len().min(dep_nchans)..dep_nchans {
        replace_slot.push(None);
        extend_locs.push(ChannelLocation::Left);
    }

    let num_extend = extend_locs.len();
    let new_nchans = indep_nchans + num_extend;

    let mut grown = vec![0.0f32; samples * new_nchans];
    for n in 0..samples {
        // Start from the indep program (which the in-place replacements
        // below overwrite slot-by-slot).
        for ch in 0..indep_nchans {
            grown[n * new_nchans + ch] = state.indep_pcm_f32[n * indep_nchans + ch];
        }
        let mut extend_cursor = 0usize;
        for (dch, slot) in replace_slot.iter().copied().enumerate() {
            let sample = dep_floats[n * dep_nchans + dch];
            match slot {
                // §E.3.8.2 replace: overwrite the matching indep slot.
                Some(s) => grown[n * new_nchans + s] = sample,
                // §E.3.8.2 extend: append in extend order.
                None => {
                    grown[n * new_nchans + indep_nchans + extend_cursor] = sample;
                    extend_cursor += 1;
                }
            }
        }
    }
    state.indep_pcm_f32 = grown;
    state.indep_nchans = new_nchans as u16;
    // The extended channels grow the location list; replaced channels
    // already have their location in indep_locations and keep their slot.
    state.indep_locations.extend(extend_locs.iter().copied());

    // Diagnostic: log the chanmap for any future bug hunts.
    if let Some(map) = chanmap {
        if std::env::var("EAC3_TRACE_CHANMAP").is_ok() {
            eprintln!(
                "TRACE-CHANMAP dep substream: chanmap=0x{map:04X} dep_nchans={dep_nchans} \
                 replaced={} extended={num_extend} → indep grown to {new_nchans} channels",
                dep_nchans - num_extend,
            );
        }
    }

    extend_locs
}

/// Build a silent S16 PCM buffer of the right shape for one
/// substream. Used as a fallback when a frame can't be DSP-decoded.
fn build_silent_indep(bsi: &Eac3Bsi) -> Result<DecodedFrame> {
    // Annex E does not change the §2.2 transform: each audio block
    // produces 256 PCM samples per channel. Total per syncframe =
    // num_blocks × 256.
    let samples = bsi.num_blocks as u32 * 256;
    let nchans = bsi.nchans as usize;
    let total_bytes = samples as usize * nchans * 2;
    let pcm_s16le = vec![0u8; total_bytes];
    Ok(DecodedFrame {
        sample_rate: bsi.sample_rate,
        channels: bsi.nchans as u16,
        samples,
        pcm_s16le,
        acmod: bsi.acmod,
        lfeon: bsi.lfeon,
        nfchans: bsi.nfchans,
        annex_e_mix_levels: bsi.annex_e_mix_levels,
        // Silent-fallback path emits the indep program only — no
        // dep channels were spliced.
        dep_locations: Vec::new(),
        dialnorm: bsi.dialnorm,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxideav_core::bits::BitWriter;

    /// Build a minimal valid E-AC-3 syncframe of `frame_bytes` bytes:
    /// syncword + the BSI from `bsi_bits` + zero-padding through the
    /// payload. Used by parser smoke tests; not bit-stream conformant
    /// past the BSI/audfrm boundary.
    fn build_syncframe(frame_bytes: usize, bsi_bits: &[(u32, u32)]) -> Vec<u8> {
        let mut bw = BitWriter::with_capacity(frame_bytes);
        bw.write_u32(SYNCWORD as u32, 16);
        for &(n, v) in bsi_bits {
            bw.write_u32(v, n);
        }
        let mut buf = bw.into_bytes();
        if buf.len() < frame_bytes {
            buf.resize(frame_bytes, 0);
        } else {
            buf.truncate(frame_bytes);
        }
        buf
    }

    /// Build a stereo 6-block 768-byte indep syncframe + a stripped
    /// audfrm with zero strategy flags — enough that the round-1
    /// decoder produces a silent buffer of the right shape.
    fn stereo_768_indep() -> Vec<u8> {
        let bsi_bits: &[(u32, u32)] = &[
            (2, 0),    // strmtyp = indep
            (3, 0),    // substreamid
            (11, 383), // frmsiz → 768 bytes
            (2, 0),    // fscod = 48 kHz
            (2, 3),    // numblkscod = 3 → 6 blocks
            (3, 2),    // acmod = 2 (2/0)
            (1, 0),    // lfeon
            (5, 16),   // bsid
            (5, 27),   // dialnorm
            (1, 0),    // compre
            (1, 0),    // mixmdate
            (1, 0),    // infomdate
            (1, 0),    // addbsie
            // ----- audfrm -----
            (1, 1), // expstre
            (1, 0), // ahte
            (2, 0), // snroffststr
            (1, 0), // transproce
            (1, 1), // blkswe
            (1, 1), // dithflage
            (1, 1), // bamode
            (1, 0), // frmfgaincode
            (1, 1), // dbaflde
            (1, 1), // skipflde
            (1, 0), // spxattene
            // acmod>1 → cplinu[0] (1) + 5 × cplstre (1 each)
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            (1, 0),
            // expstre==1 → per-block chexpstr lives in audblk(), NOT
            // audfrm. (Round-2 fix: round 1 over-consumed these bits.)
            // strmtyp=0 + numblkscod=3 → convexpstre implicit 1
            // → 2 × convexpstr (5 bits)
            (5, 0),
            (5, 0),
            // snroffststr=0 → frmcsnroffst (6) + frmfsnroffst (4)
            (6, 15),
            (4, 0),
            // num_blocks > 1 → blkstrtinfoe (1, 0)
            (1, 0),
        ];
        build_syncframe(768, bsi_bits)
    }

    #[test]
    fn decode_silent_indep_smoke() {
        let pkt = stereo_768_indep();
        let mut st = Eac3DecoderState::default();
        let frm = decode_eac3_packet(&mut st, &pkt).unwrap();
        assert_eq!(frm.sample_rate, 48_000);
        assert_eq!(frm.channels, 2);
        assert_eq!(frm.samples, 6 * 256);
        assert_eq!(frm.pcm_s16le.len(), (6 * 256) * 2 * 2);
        assert!(frm.pcm_s16le.iter().all(|&b| b == 0));
    }

    #[test]
    fn rejects_bad_syncword() {
        let mut data = stereo_768_indep();
        data[0] = 0xFF;
        let mut st = Eac3DecoderState::default();
        assert!(decode_eac3_packet(&mut st, &data).is_err());
    }

    /// Set up a decoder state with a 1-sample, 3-channel indep program
    /// at the given locations and the given per-channel constant values.
    fn indep_state_3ch(locs: [ChannelLocation; 3], vals: [f32; 3]) -> Eac3DecoderState {
        Eac3DecoderState {
            indep_samples_per_frame: 1,
            indep_nchans: 3,
            indep_pcm_f32: vals.to_vec(),
            indep_locations: locs.to_vec(),
            ..Default::default()
        }
    }

    /// §E.3.8.2: a dep channel whose location is NOT in the indep
    /// program *extends* the output (grows the channel count). The 7.1
    /// case: indep 5.1 (here shrunk to L,C,R for the test) + a dep
    /// [Lrs, Rrs] pair → 5 channels, the two new ones appended at the
    /// end, and `dep_locations` reports exactly [Lrs, Rrs].
    #[test]
    fn splice_extend_appends_new_locations() {
        use ChannelLocation::*;
        let mut st = indep_state_3ch([Left, Center, Right], [1.0, 2.0, 3.0]);
        // Dep substream: 2 coded channels at the rear-surround pair.
        // Table E2.5 bit 6 = Lrs/Rrs.
        let chanmap = Some(1u16 << (15 - 6));
        let dep_floats = [4.0f32, 5.0f32]; // 1 sample × 2 dep channels
        let locs = vec![LeftRearSurround, RightRearSurround];
        let extended = splice_dep_into_indep(&mut st, 2, chanmap, &dep_floats, &locs);
        assert_eq!(st.indep_nchans, 5, "indep 3 + 2 new dep = 5 channels");
        // Indep channels untouched; dep pair appended in order.
        assert_eq!(st.indep_pcm_f32, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(extended, vec![LeftRearSurround, RightRearSurround]);
        assert_eq!(
            st.indep_locations,
            vec![Left, Center, Right, LeftRearSurround, RightRearSurround]
        );
    }

    /// §E.3.8.2: a dep channel whose location ALREADY exists in the
    /// indep program *replaces* the matching indep slot in place — it
    /// does NOT grow the channel count, and the original indep sample
    /// at that slot is overwritten with the dep data. This is the case
    /// the spec spells out explicitly ("the center channel audio data
    /// carried in the dependent stream will replace the center channel
    /// audio data carried in the independent stream"). A naive append
    /// would have produced [L,C,R, C'] (4 channels, duplicated +
    /// decorrelated center) — the broadcast-decode defect this fixes.
    #[test]
    fn splice_replace_overwrites_shared_location_in_place() {
        use ChannelLocation::*;
        let mut st = indep_state_3ch([Left, Center, Right], [1.0, 2.0, 3.0]);
        // Dep substream: a single channel re-coding the Center.
        // Table E2.5 bit 1 = Center.
        let chanmap = Some(1u16 << (15 - 1));
        let dep_floats = [9.0f32]; // 1 sample × 1 dep channel
        let locs = vec![Center];
        let extended = splice_dep_into_indep(&mut st, 1, chanmap, &dep_floats, &locs);
        assert_eq!(
            st.indep_nchans, 3,
            "replace must NOT grow the channel count"
        );
        // Center slot (index 1) overwritten; L and R unchanged.
        assert_eq!(st.indep_pcm_f32, vec![1.0, 9.0, 3.0]);
        assert!(
            extended.is_empty(),
            "a replaced channel does not extend the program"
        );
        assert_eq!(st.indep_locations, vec![Left, Center, Right]);
    }

    /// §E.3.8.2 mixed case: a dep substream carrying both a shared
    /// location (Center, replaced in place) and a new location (Cs,
    /// appended). Verifies the partition is per-channel, not all-or-
    /// nothing, and that the appended channel lands after the indep
    /// program while the replaced one stays in its original slot.
    #[test]
    fn splice_mixed_replace_and_extend() {
        use ChannelLocation::*;
        let mut st = indep_state_3ch([Left, Center, Right], [1.0, 2.0, 3.0]);
        // Table E2.5 bit 1 = Center, bit 7 = Cs.
        let chanmap = Some((1u16 << (15 - 1)) | (1u16 << (15 - 7)));
        let dep_floats = [7.0f32, 8.0f32]; // Center', Cs
        let locs = vec![Center, CenterSurround];
        let extended = splice_dep_into_indep(&mut st, 2, chanmap, &dep_floats, &locs);
        assert_eq!(st.indep_nchans, 4, "1 replace + 1 extend → 3 + 1 = 4");
        // Center (slot 1) replaced with 7.0; Cs appended at slot 3.
        assert_eq!(st.indep_pcm_f32, vec![1.0, 7.0, 3.0, 8.0]);
        assert_eq!(extended, vec![CenterSurround]);
        assert_eq!(
            st.indep_locations,
            vec![Left, Center, Right, CenterSurround]
        );
    }
}

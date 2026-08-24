//! Stream-level ADTS decode driver — raw_data_block walk to interleaved
//! 16-bit PCM.
//!
//! [`crate::element_decode::ElementDecoder`] decodes *one* channel
//! element per call and carries that element's §4.6.11 overlap-add tail
//! across frames. This module is the layer above it: it walks the
//! §4.4.2.1 `raw_data_block()` of one ADTS frame
//! ([`crate::raw_data_block::Walker`]), dispatches each `id_syn_ele`
//! onto a per-element-slot [`ElementDecoder`] (keyed by `(syntactic
//! element id, element_instance_tag)` so each element's filterbank state
//! is independent), composes the channel-element bodies via
//! [`crate::ics_body`] / [`crate::spectral_data`], and renders the
//! frame's per-channel time signals to the element-order interleaved
//! 16-bit PCM layout via [`crate::pcm`].
//!
//! Scope: AAC-LC (and the other General-Audio object types the
//! per-tool chain covers) carried in ADTS — including
//! multi-`raw_data_block` frames (each block renders one consecutive
//! 1024-sample hop) and the `error_check()` CRC layer (verified by
//! [`StreamDecoder::decode_adts_frame`] via [`crate::adts_crc`]) —
//! with the channel elements the staged-fixture encoders emit
//! (SCE / LFE / CPE, plus the consumed-and-ignored FIL / DSE /
//! PCE). A `coupling_channel_element()` (CCE) is parsed via
//! [`crate::cce::CouplingChannelElement`] **and applied**: the walk is
//! two-pass — every channel element of a block is parsed first, each
//! CCE's embedded `single_channel_element()` is decoded through its
//! per-instance-tag [`CceDecoder`] slot, and the §4.6.8.3.3
//! `decode_coupling_channel()` target walk then injects the scaled
//! spectra (or, for an independently switched CCE, the time signal)
//! into the addressed SCE / CPE channels at the signalled `cc_domain`
//! stage. The CCE contributes no output channel of its own. SBR / PS
//! up-sampling ride the FIL extension walk (§4.6.18 back-end).
//!
//! ## Provenance
//!
//! The §4.4.2.1 `raw_data_block()` walk, the §4.4.2.3 `channel_pair_
//! element()` `common_window` / `ms_mask_present` header, and the
//! §4.6.11 PCM output contract are from ISO/IEC 14496-3 / 13818-7 staged
//! under `docs/audio/aac/`. No part of the byte ordering or the element
//! dispatch comes from any external decoder.

use std::collections::HashMap;

use oxideav_core::bits::BitReader;

use crate::adts::AdtsHeader;
use crate::asc::AacResilienceFlags;
use crate::cce::CouplingChannelElement;
use crate::channel_map::PceElementKind;
use crate::element_decode::{
    CceDecoder, ChannelInput, CouplingApply, CpeJointStereo, DecodedCce, ElementDecoder,
};
use crate::extension_payload::{ExtensionPayload, ExtensionPayloadOrSbr};
use crate::ics_body::IcsBody;
use crate::ics_info::IcsInfo;
use crate::ms_stereo::MsMaskPresent;
use crate::pce::Pce;
use crate::pcm::interleave_s16;
use crate::raw_data_block::{Element, IdSynEle, Walker};
use crate::sbr_decoder::SbrDecoder;
use crate::sbr_extension::SbrExtensionData;
use crate::sbr_header::SbrHeader;
use crate::spectral_data::SpectralData;
use crate::swb_offset::FrameFamily;
use crate::{Error, Result};

/// Map a channel element's [`IdSynEle`] to its §8.5.2.2 PCE reference
/// kind. `None` for elements a PCE never addresses as an output
/// channel (CCE contributes no output channel here).
fn pce_kind(kind: IdSynEle) -> Option<PceElementKind> {
    match kind {
        IdSynEle::Sce => Some(PceElementKind::Sce),
        IdSynEle::Cpe => Some(PceElementKind::Cpe),
        IdSynEle::Lfe => Some(PceElementKind::Lfe),
        _ => None,
    }
}

/// The §4.6.11 per-frame sample count for the default 1024-line
/// transform family. The other §4.5.1.1 families emit 960 / 512 /
/// 480 samples per frame per channel
/// ([`crate::swb_offset::FrameFamily::frame_len`]).
pub const FRAME_LEN: usize = 1024;

/// One decoded ADTS frame: the interleaved 16-bit PCM plus the geometry
/// needed to interpret it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    /// Interleaved 16-bit PCM, `channels` samples per time index. For a
    /// default `channelConfiguration` (Table 1.19, values 1–6) the
    /// channels are in the canonical [`crate::channel_map`] output order
    /// (e.g. 5.1 is `L, R, C, LFE, Ls, Rs`); for the unmapped configs
    /// (`0` PCE-defined, `7`) they stay in `raw_data_block` element order
    /// (an SCE/LFE contributes one channel, a CPE two). Length is
    /// `FRAME_LEN * channels` for the plain AAC path, or
    /// `2 * FRAME_LEN * channels` once the stream is SBR-active
    /// (HE-AAC dual-rate output; `FRAME_LEN * channels` again when
    /// the §4.6.18.4.3 downsampled SBR mode is selected).
    pub pcm: Vec<i16>,
    /// Number of interleaved channels this frame produced.
    pub channels: usize,
    /// The frame's sampling rate in Hz: the ADTS-signalled core rate,
    /// doubled once the stream is SBR-active (kept at the core rate
    /// in the §4.6.18.4.3 downsampled SBR mode).
    pub sample_rate: u32,
}

/// Stateful whole-stream ADTS decoder.
///
/// Holds one [`ElementDecoder`] per `(element-id, instance-tag)` slot so
/// every channel element's §4.6.11 overlap-add tail, §4.6.7 LTP history,
/// and §4.6.6 predictor state persist across the frames of the stream.
/// Construct one [`StreamDecoder`] per stream and feed it ADTS frames in
/// order via [`Self::decode_frame`], or hand it the whole byte buffer
/// via [`Self::decode_all`].
#[derive(Debug, Default)]
pub struct StreamDecoder {
    decoders: HashMap<(u8, u8), ElementDecoder>,
    /// One §4.6.8.3.3 CCE decoder per coupling-element instance tag
    /// (its independently-switched filterbank overlap and PNS state
    /// persist across frames).
    cce_decoders: HashMap<u8, CceDecoder>,
    /// One §4.6.18 SBR back-end per channel-element slot (HE-AAC).
    sbr: HashMap<(u8, u8), SbrDecoder>,
    /// The threaded previous `sbr_header()` per slot (the
    /// `bs_header_flag == 0` reuse path).
    sbr_prev_header: HashMap<(u8, u8), SbrHeader>,
    /// Latched once any frame carries SBR data: from then on every
    /// frame is emitted at the SBR output rate (doubled, or the core
    /// rate in downsampled mode) — SBR-less frames go through the
    /// §4.6.18.5 pure-upsampling path so the output rate never flaps.
    sbr_active: bool,
    /// §4.6.18.4.3 downsampled SBR output mode: SBR frames are
    /// synthesized through the 32-channel bank and emitted at the
    /// *core* rate (1024 samples per channel per block). Installed on
    /// every SBR back-end this decoder creates
    /// ([`Self::set_sbr_downsampled`]).
    sbr_downsampled: bool,
    /// §4.6.18.8 low-power SBR mode: real-valued filterbanks and the
    /// LP adjustment chain on every SBR back-end this decoder creates
    /// ([`Self::set_sbr_low_power`]).
    sbr_low_power: bool,
    /// The active `program_config_element()` for
    /// `channelConfiguration == 0` streams — captured from an in-band
    /// PCE (§8.5.2.2: it takes effect at the block carrying it and
    /// persists) or installed by [`Self::set_program_config`] when the
    /// PCE rides inline in an out-of-band `AudioSpecificConfig`.
    program_config: Option<Pce>,
    /// The §4.5.1.1 frame-length family every block of this stream
    /// decodes under. ADTS cannot signal anything but the 1024-line
    /// family (the default); a LATM / raw caller with an
    /// `AudioSpecificConfig` installs the ASC-resolved family via
    /// [`Self::set_frame_family`] before the first block.
    family: FrameFamily,
}

impl StreamDecoder {
    /// A fresh stream decoder with no element state.
    #[must_use]
    pub fn new() -> Self {
        StreamDecoder::default()
    }

    /// Install the program configuration of a
    /// `channelConfiguration == 0` stream whose
    /// `program_config_element()` rides *outside* the AAC payload —
    /// inline in the `AudioSpecificConfig` (the MP4 / LATM case,
    /// [`crate::asc::GaSpecificConfig::pce`]) or an `adif_header()`.
    /// An in-band PCE inside a later `raw_data_block()` replaces it
    /// (§8.5.2.2 persistence). The active PCE drives the §8.5.2.2
    /// element→speaker canonical output reorder; without one, a
    /// config-0 stream is emitted in bitstream element order.
    pub fn set_program_config(&mut self, pce: Pce) {
        self.program_config = Some(pce);
    }

    /// Select the §4.6.18.4.3 downsampled SBR output mode: every SBR
    /// back-end runs the 32-channel synthesis bank, so an SBR-active
    /// stream is emitted at the *core* sampling rate (1024 samples per
    /// channel per block) instead of the doubled `fs_sbr` rate. The
    /// reconstructed SBR bands below the core Nyquist are kept; the
    /// range above it is discarded by construction. An explicitly
    /// signalled `AudioSpecificConfig` whose `extensionSamplingFrequency`
    /// equals the core rate is the in-band request for this mode
    /// (§4.6.18.2.6, `FsSBR` definition).
    ///
    /// Select the mode before decoding: back-ends already created for
    /// earlier frames keep their rate (the QMF history is
    /// rate-specific).
    pub fn set_sbr_downsampled(&mut self, downsampled: bool) {
        self.sbr_downsampled = downsampled;
    }

    /// Install the §4.5.1.1 frame-length family (from
    /// `GASpecificConfig.frameLengthFlag` + the AOT) for every later
    /// block. Affects the SWB tables, transform lengths and the
    /// per-frame PCM sample count (1024 / 960 / 512 / 480). ADTS
    /// cannot signal anything but the default 1024-line family; a
    /// LATM / raw caller with an `AudioSpecificConfig` selects the
    /// ASC-resolved family before the first block (the per-element
    /// state is keyed to the family at slot creation).
    pub fn set_frame_family(&mut self, family: FrameFamily) {
        self.family = family;
    }

    /// The active §4.5.1.1 frame-length family.
    pub fn frame_family(&self) -> FrameFamily {
        self.family
    }

    /// Select the §4.6.18.8 low-power SBR mode: every SBR back-end
    /// runs the real-valued filterbanks with the LP adjustment chain
    /// (×2 energy estimation, aliasing detection/reduction, modified
    /// sinusoid injection, no gain smoothing). Composable with
    /// [`Self::set_sbr_downsampled`]. An HE-AAC v2 (PS) stream is
    /// rejected in this mode ([`crate::Error::SbrLowPowerPs`]) — the
    /// subpart-8 tool needs the complex QMF domain. Select before
    /// decoding.
    pub fn set_sbr_low_power(&mut self, low_power: bool) {
        self.sbr_low_power = low_power;
    }

    /// Decode one ADTS frame's `raw_data_block()` payload to interleaved
    /// 16-bit PCM.
    ///
    /// `header` is the parsed [`AdtsHeader`]; `payload` is the
    /// `raw_data_block()` bytes (the frame body *after* the
    /// fixed/variable header and the optional CRC — i.e. starting at the
    /// header's `payload_offset`). The channel elements update this
    /// decoder's per-slot state, so frames must be fed in stream order.
    ///
    /// A frame that yields no channel element (e.g. fill-only) returns a
    /// [`DecodedFrame`] with `channels == 0` and an empty `pcm`.
    pub fn decode_frame(&mut self, header: &AdtsHeader, payload: &[u8]) -> Result<DecodedFrame> {
        self.decode_raw_data_block(
            header.audio_object_type(),
            header.sampling_frequency_index,
            header.sample_rate(),
            header.channel_configuration,
            header.number_of_raw_data_blocks_in_frame,
            payload,
        )
    }

    /// Decode one `raw_data_block()` payload to interleaved 16-bit PCM,
    /// driven by an explicit `(audioObjectType, samplingFrequencyIndex,
    /// sampleRate)` configuration rather than an ADTS header.
    ///
    /// This is the transport-independent core that [`Self::decode_frame`]
    /// (ADTS) and the LATM/LOAS driver
    /// ([`crate::latm::LoasDecoder`]) both call: each recovers the AAC
    /// configuration from its own framing (the ADTS fixed header, or the
    /// LATM `AudioSpecificConfig`) and hands the same §4.4.2.1
    /// `raw_data_block()` bytes here. `aot` is the §1.6.2.1
    /// `audioObjectType` (already escaped past the ADTS `profile + 1`
    /// adjustment), `fs_index` is the Table 1.18
    /// `samplingFrequencyIndex`, `sample_rate` is the resolved rate the
    /// returned [`DecodedFrame`] reports, `channel_configuration` is the
    /// Table 1.19 default-layout selector that drives the §1.6.3.5
    /// element→speaker output reorder (see [`crate::channel_map`]), and
    /// `num_raw_data_blocks` is the resolved block count `N` (ADTS carries
    /// `N - 1`; LATM carries one block per payload, i.e. `N == 1`).
    pub fn decode_raw_data_block(
        &mut self,
        aot: u8,
        fs_index: u8,
        sample_rate: u32,
        channel_configuration: u8,
        num_raw_data_blocks: u8,
        payload: &[u8],
    ) -> Result<DecodedFrame> {
        let fs = fs_index;
        let family = self.family;
        let mut reader = BitReader::new(payload);

        // Per channel-element outputs in element order: the decoded
        // core time signals plus any SBR extension payload that
        // followed the element in a FIL.
        struct ElementOut {
            key: (u8, u8),
            kind: IdSynEle,
            channels: Vec<Vec<f64>>,
            sbr: Option<Box<SbrExtensionData>>,
        }
        // A channel element parsed off the bitstream but not yet
        // decoded. Decoding is deferred until the whole block is
        // walked so §4.6.8.3.3 coupling channel elements — which may
        // appear before or after the SCE / CPE targets they address —
        // can contribute at the right stage of every target's chain.
        struct ParsedSce {
            body: IcsBody,
            ics: IcsInfo,
            spectral: SpectralData,
        }
        enum ParsedChannel {
            Single(Box<ParsedSce>),
            Pair(Box<ParsedCpe>),
        }
        struct PendingElement {
            key: (u8, u8),
            kind: IdSynEle,
            block: u8,
            parsed: ParsedChannel,
            sbr: Option<Box<SbrExtensionData>>,
        }
        let mut pending: Vec<PendingElement> = Vec::new();
        let mut cces: Vec<(u8, CouplingChannelElement)> = Vec::new();
        let fs_sbr = sample_rate.saturating_mul(2);

        // `num_raw_data_blocks` is the resolved count `N`. The walker
        // returns `None` when the payload is exhausted before an explicit
        // END (real-world encoders pad the frame but do not always
        // round-trip a trailing END marker after the last element); treat
        // that as end-of-block, the same as an `Element::End`.
        'blocks: for block in 0..num_raw_data_blocks {
            while let Some(elem) = Walker::new(&mut reader).next_element_keep_fill()? {
                match elem {
                    Element::ChannelElement {
                        kind: kind @ (IdSynEle::Sce | IdSynEle::Lfe),
                        element_instance_tag,
                    } => {
                        let body = IcsBody::parse_family(&mut reader, family, aot, fs, false)?;
                        let ics = body.ics_info.clone().ok_or(Error::ElementDecodeInvalid)?;
                        let spectral =
                            SpectralData::parse(&mut reader, &ics, &body.section_data, fs)?;
                        pending.push(PendingElement {
                            key: (kind_id(kind), element_instance_tag),
                            kind,
                            block,
                            parsed: ParsedChannel::Single(Box::new(ParsedSce {
                                body,
                                ics,
                                spectral,
                            })),
                            sbr: None,
                        });
                    }
                    Element::ChannelElement {
                        kind: IdSynEle::Cpe,
                        element_instance_tag,
                    } => {
                        let parsed = parse_cpe_family(&mut reader, family, aot, fs)?;
                        pending.push(PendingElement {
                            key: (kind_id(IdSynEle::Cpe), element_instance_tag),
                            kind: IdSynEle::Cpe,
                            block,
                            parsed: ParsedChannel::Pair(Box::new(parsed)),
                            sbr: None,
                        });
                    }
                    Element::ChannelElement {
                        kind: IdSynEle::Cce,
                        element_instance_tag,
                    } => {
                        // §4.6.8.3 / Table 4.8: parse the whole coupling
                        // channel element (header + embedded
                        // single_channel_element + gain lists). Its
                        // embedded spectrum is decoded once per block
                        // below and coupled onto the addressed SCE / CPE
                        // targets per §4.6.8.3.3.
                        let cce = CouplingChannelElement::parse_after_tag_family(
                            &mut reader,
                            family,
                            element_instance_tag,
                            aot,
                            fs,
                        )?;
                        cces.push((block, cce));
                    }
                    Element::ChannelElement { kind, .. } => {
                        // Any other channel-element id has no decode path.
                        return Err(unsupported_element(kind));
                    }
                    Element::Fill { payload_bytes } => {
                        // The FIL body was left unconsumed: walk the
                        // Table 4.51 extension_payload() chain, routing
                        // any SBR payload onto the preceding channel
                        // element (§4.4.2.7: an SBR FIL directly follows
                        // the SCE/CPE it extends).
                        let target = pending
                            .last()
                            .filter(|el| matches!(el.kind, IdSynEle::Sce | IdSynEle::Cpe))
                            .map(|el| (el.kind, el.key));
                        if let Some(ext) =
                            self.consume_fill(&mut reader, payload, payload_bytes, fs_sbr, target)?
                        {
                            if let Some(el) = pending.last_mut() {
                                el.sbr = Some(ext);
                            }
                        }
                    }
                    Element::Data { .. } => {}
                    Element::ProgramConfig(pce) => {
                        // §8.5.2.2: the configuration takes effect at
                        // the raw_data_block() containing the PCE and
                        // persists until a new PCE arrives.
                        self.program_config = Some(pce);
                    }
                    Element::End => continue 'blocks,
                }
            }
        }

        // §4.6.8.3.3 — decode each CCE's embedded
        // single_channel_element() into its cc_spectrum (and, for an
        // independently switched CCE, its time signal), through the
        // per-instance-tag persistent CCE decoder slot.
        let mut decoded_cces: Vec<DecodedCce> = Vec::with_capacity(cces.len());
        for (_, cce) in &cces {
            let dec = self
                .cce_decoders
                .entry(cce.element_instance_tag)
                .or_insert_with(|| CceDecoder::new_family(family));
            decoded_cces.push(dec.decode(cce, aot, fs)?);
        }

        // Decode the pending channel elements in element order, with
        // each channel's coupling contributions injected at the
        // §4.6.8.3.3 cc_domain stage. The elements stay tagged with
        // their raw_data_block index: a multi-RDB ADTS frame carries N
        // *consecutive* 1024-sample blocks of the same program, so
        // each block renders its own channel set and the per-block PCM
        // is concatenated in time below.
        let mut elements: Vec<(u8, ElementOut)> = Vec::new();
        for pe in pending {
            let channels = match &pe.parsed {
                ParsedChannel::Single(sce) => {
                    let coupling =
                        coupling_for(&cces, &decoded_cces, pe.block, pe.kind, pe.key.1, 0);
                    let ch = ChannelInput {
                        body: &sce.body,
                        ics_info: &sce.ics,
                        spectral: &sce.spectral,
                    };
                    let dec = self
                        .decoders
                        .entry(pe.key)
                        .or_insert_with(|| ElementDecoder::new_family(family));
                    vec![dec.decode_sce_coupled(&ch, aot, fs, &coupling)?]
                }
                ParsedChannel::Pair(cpe) => {
                    let left_coupling =
                        coupling_for(&cces, &decoded_cces, pe.block, pe.kind, pe.key.1, 0);
                    let right_coupling =
                        coupling_for(&cces, &decoded_cces, pe.block, pe.kind, pe.key.1, 1);
                    let (left, right, joint) = cpe.channel_inputs();
                    let dec = self
                        .decoders
                        .entry(pe.key)
                        .or_insert_with(|| ElementDecoder::new_family(family));
                    let (l, r) = dec.decode_cpe_coupled(
                        &left,
                        &right,
                        joint,
                        aot,
                        fs,
                        &left_coupling,
                        &right_coupling,
                    )?;
                    vec![l, r]
                }
            };
            elements.push((
                pe.block,
                ElementOut {
                    key: pe.key,
                    kind: pe.kind,
                    channels,
                    sbr: pe.sbr,
                },
            ));
        }

        // HE-AAC: once any frame carries SBR data the stream is emitted
        // at the doubled rate; frames without SBR go through the pure
        // upsampling path so the rate never flaps.
        if elements.iter().any(|(_, e)| e.sbr.is_some()) {
            self.sbr_active = true;
        }
        let out_rate = if self.sbr_active && !self.sbr_downsampled {
            fs_sbr
        } else {
            sample_rate
        };

        // Render block by block; each block contributes one hop of
        // interleaved PCM (all blocks of a frame must agree on the
        // channel count).
        let mut pcm: Vec<i16> = Vec::new();
        let mut frame_channels: Option<usize> = None;
        for block in 0..num_raw_data_blocks {
            let mut channels: Vec<Vec<f64>> = Vec::new();
            // Per decoded element: (kind, instance tag, contributed
            // channel count) — the descriptor list the §8.5.2.2 PCE
            // reorder keys on for `channelConfiguration == 0`.
            let mut element_desc: Vec<(PceElementKind, u8, usize)> = Vec::new();
            for (_, el) in elements.iter().filter(|(b, _)| *b == block) {
                if self.sbr_active {
                    let n_ch = el.channels.len();
                    let dec = match self.sbr.entry(el.key) {
                        std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                        std::collections::hash_map::Entry::Vacant(v) => {
                            let mut d = SbrDecoder::new(fs_sbr, n_ch)?;
                            d.set_downsampled(self.sbr_downsampled)?;
                            d.set_low_power(self.sbr_low_power)?;
                            v.insert(d)
                        }
                    };
                    let core: Vec<&[f64]> = el.channels.iter().map(Vec::as_slice).collect();
                    let up = match &el.sbr {
                        Some(ext) => dec.process_frame(ext, &core)?,
                        None => dec.upsample_frame(&core)?,
                    };
                    if let Some(kind) = pce_kind(el.kind) {
                        element_desc.push((kind, el.key.1, up.len()));
                    }
                    channels.extend(up);
                } else {
                    if let Some(kind) = pce_kind(el.kind) {
                        element_desc.push((kind, el.key.1, el.channels.len()));
                    }
                    channels.extend(el.channels.iter().cloned());
                }
            }

            // §1.6.3.5 / Table 1.19: a default `channelConfiguration`
            // (1–7) fixes which loudspeaker each decoded element feeds.
            // Reorder the element-order channel buffers into the
            // canonical interleaved layout (a no-op for mono/stereo). A
            // `channelConfiguration == 0` block is reordered by the
            // active §8.5.2.2 PCE instead, when one is installed and it
            // maps onto canonical positions; otherwise element order is
            // kept.
            let channels =
                if channel_configuration == 0 {
                    match self.program_config.as_ref().and_then(|pce| {
                        crate::channel_map::pce_reorder_permutation(pce, &element_desc)
                    }) {
                        Some(perm) => crate::channel_map::apply_permutation(&perm, channels),
                        None => channels,
                    }
                } else {
                    crate::channel_map::reorder_channels(channel_configuration, channels)
                };

            match frame_channels {
                None => frame_channels = Some(channels.len()),
                Some(n) if n != channels.len() => {
                    // The blocks of one ADTS frame carry the same
                    // program; a channel-count flip mid-frame is
                    // structurally inconsistent.
                    return Err(Error::ElementDecodeInvalid);
                }
                Some(_) => {}
            }
            pcm.extend(interleave_s16(&channels)?);
        }

        Ok(DecodedFrame {
            pcm,
            channels: frame_channels.unwrap_or(0),
            sample_rate: out_rate,
        })
    }

    /// Walk a FIL element's Table 4.51 `extension_payload()` chain
    /// (the body was left unconsumed by
    /// [`Walker::next_element_keep_fill`]). `payload` is the byte
    /// buffer `reader` was constructed over (needed to recompute the
    /// §4.4.2.8.1 SBR CRC over its coverage region); `target` is the
    /// preceding SCE / CPE this FIL would extend (its `id_syn_ele` +
    /// slot key), or `None` when the FIL follows no channel element.
    /// Returns the decoded SBR payload, if any; the threaded
    /// `sbr_header()` reuse state is updated per slot. An
    /// `EXT_SBR_DATA_CRC` payload whose recomputed CRC-10 disagrees
    /// with the transmitted `bs_sbr_crc_bits` is rejected with
    /// [`Error::SbrCrcMismatch`].
    fn consume_fill(
        &mut self,
        reader: &mut BitReader<'_>,
        payload: &[u8],
        payload_bytes: u32,
        fs_sbr: u32,
        target: Option<(IdSynEle, (u8, u8))>,
    ) -> Result<Option<Box<SbrExtensionData>>> {
        let mut remaining = payload_bytes;
        let mut result = None;
        while remaining > 0 {
            match target {
                None => {
                    // No preceding channel element: only the non-SBR
                    // payload types are meaningful here.
                    let p = ExtensionPayload::parse(reader, remaining)?;
                    let n = p.byte_length().max(1);
                    remaining = remaining.saturating_sub(n);
                }
                Some(_) if self.family != FrameFamily::Lc1024 => {
                    // The §4.6.18 SBR tool in this crate is defined
                    // over the 1024-line core frame (32-subband
                    // analysis / 2048-sample output); a 960-line or
                    // LD core cannot feed it, so an SBR extension
                    // type is rejected before its body is even
                    // parsed. Non-SBR payload types stay usable.
                    match ExtensionPayload::parse(reader, remaining) {
                        Ok(p) => {
                            let n = p.byte_length().max(1);
                            remaining = remaining.saturating_sub(n);
                        }
                        Err(Error::UnsupportedExtensionSbr(_)) => {
                            return Err(Error::SbrUnsupportedFrameFamily);
                        }
                        Err(e) => return Err(e),
                    }
                }
                Some((id_aac, slot)) => {
                    let prev = self.sbr_prev_header.get(&slot).copied();
                    match ExtensionPayload::parse_with_sbr(reader, remaining, id_aac, fs_sbr, prev)?
                    {
                        ExtensionPayloadOrSbr::Payload(p) => {
                            let n = p.byte_length().max(1);
                            remaining = remaining.saturating_sub(n);
                        }
                        ExtensionPayloadOrSbr::Sbr(ext) => {
                            ext.verify_crc(payload)?;
                            self.sbr_prev_header.insert(slot, ext.header);
                            result = Some(ext);
                            remaining = 0;
                        }
                        ExtensionPayloadOrSbr::SbrPreHeader { crc, crc_region } => {
                            // §4.5.2.8.1: SBR payloads before the first
                            // sbr_header() — verify the CRC over the
                            // whole-payload region, then run upsampling
                            // and delay adjustment only (the None SBR
                            // slot below selects the §4.6.18.5 pure
                            // upsampling path). No header is threaded.
                            if let (Some(crc), Some((s, e))) = (crc, crc_region) {
                                if crate::adts_crc::sbr_crc(payload, s, e) != crc {
                                    return Err(Error::SbrCrcMismatch);
                                }
                            }
                            self.sbr_active = true;
                            remaining = 0;
                        }
                    }
                }
            }
        }
        Ok(result)
    }

    /// Decode one whole ADTS frame — fixed/variable header, the
    /// optional `error_check()` CRC layer, and the `raw_data_block()`
    /// payload(s) — to interleaved 16-bit PCM.
    ///
    /// `frame` must start at the ADTS syncword and carry at least
    /// `aac_frame_length` bytes (trailing bytes are ignored). Unlike
    /// [`Self::decode_frame`] (which receives the payload with the CRC
    /// layer already stripped and therefore cannot verify it), this
    /// entry point *verifies* the ISO/IEC 13818-7:2004 §8.1.1 CRCs
    /// when `protection_absent == 0`:
    ///
    /// * single raw data block — the Table 1.A.8 `adts_error_check()`
    ///   16-bit `crc_check` over the 56 header bits plus every
    ///   §8.1.1.1 protected element region;
    /// * multiple raw data blocks — the Table 1.A.9
    ///   `adts_header_error_check()` (headers + the 16-bit
    ///   `raw_data_block_position` table) followed by one Table 1.A.10
    ///   `adts_raw_data_block_error_check()` per block, each read from
    ///   its byte-aligned slot after the block it protects.
    ///
    /// A mismatch surfaces [`Error::AdtsCrcMismatch`] before any
    /// decoder state is touched.
    pub fn decode_adts_frame(&mut self, frame: &[u8]) -> Result<DecodedFrame> {
        let (header, payload_offset) = AdtsHeader::parse(frame)?;
        let frame_len = header.aac_frame_length as usize;
        if frame_len < payload_offset || frame.len() < frame_len {
            return Err(Error::UnexpectedEnd);
        }
        let frame = &frame[..frame_len];
        if header.protection_absent {
            return self.decode_frame(&header, &frame[payload_offset..]);
        }
        let aot = header.audio_object_type();
        let fs = header.sampling_frequency_index;
        if header.number_of_raw_data_blocks_in_frame == 1 {
            // Table 1.A.8 adts_error_check(): one 16-bit crc_check at
            // bytes 7..9 covering headers + the block's regions.
            let crc = u16::from_be_bytes([frame[7], frame[8]]);
            let payload = &frame[crate::adts::ADTS_HEADER_BYTES_WITH_CRC..];
            let mut reader = BitReader::new(payload);
            let regions = crate::adts_crc::collect_block_regions(&mut reader, aot, fs)?;
            if crate::adts_crc::adts_single_crc(&frame[..7], payload, &regions) != crc {
                return Err(Error::AdtsCrcMismatch);
            }
            return self.decode_frame(&header, payload);
        }
        // Multi-RDB form (Tables 1.A.9 / 1.A.10): N − 1 16-bit
        // raw_data_block_position entries + the 16-bit header CRC,
        // then each raw_data_block() followed by its own 16-bit CRC.
        let n = usize::from(header.number_of_raw_data_blocks_in_frame);
        let after_positions = 7 + 2 * (n - 1);
        if frame.len() < after_positions + 2 {
            return Err(Error::UnexpectedEnd);
        }
        let positions: Vec<u16> = (0..n - 1)
            .map(|i| u16::from_be_bytes([frame[7 + 2 * i], frame[8 + 2 * i]]))
            .collect();
        let header_crc = u16::from_be_bytes([frame[after_positions], frame[after_positions + 1]]);
        if crate::adts_crc::adts_header_crc(&frame[..7], &positions) != header_crc {
            return Err(Error::AdtsCrcMismatch);
        }
        let payload = &frame[after_positions + 2..];
        let mut reader = BitReader::new(payload);
        // Verify each block's CRC, splicing the CRC fields out so the
        // block walk below sees the contiguous raw_data_block()
        // sequence it expects.
        let mut clean = Vec::with_capacity(payload.len());
        for _ in 0..n {
            let start_byte = (reader.bit_position() / 8) as usize;
            let regions = crate::adts_crc::collect_block_regions(&mut reader, aot, fs)?;
            let end_bit = reader.bit_position();
            if end_bit % 8 != 0 {
                // A block that did not end on its §4.4.2.1
                // byte_alignment() cannot be followed by the
                // byte-aligned CRC slot.
                return Err(Error::UnexpectedEnd);
            }
            let rdb_crc = reader.read_u32(16).map_err(|_| Error::UnexpectedEnd)? as u16;
            if crate::adts_crc::adts_rdb_crc(payload, &regions) != rdb_crc {
                return Err(Error::AdtsCrcMismatch);
            }
            clean.extend_from_slice(&payload[start_byte..(end_bit / 8) as usize]);
        }
        self.decode_raw_data_block(
            aot,
            fs,
            header.sample_rate(),
            header.channel_configuration,
            header.number_of_raw_data_blocks_in_frame,
            &clean,
        )
    }

    /// Decode a whole raw-ADTS byte buffer to a vector of per-frame
    /// interleaved PCM.
    ///
    /// Skips a leading ID3v2 tag if present, then walks consecutive ADTS
    /// frames (`aac_frame_length`-delimited) to exhaustion, verifying
    /// the `error_check()` CRC layer of every `protection_absent == 0`
    /// frame (see [`Self::decode_adts_frame`]). A truncated trailing
    /// frame (fewer bytes than its `aac_frame_length`) is rejected with
    /// [`Error::UnexpectedEnd`].
    pub fn decode_all(&mut self, data: &[u8]) -> Result<Vec<DecodedFrame>> {
        let data = skip_id3v2(data);
        let mut frames = Vec::new();
        let mut pos = 0usize;
        while pos + crate::adts::ADTS_HEADER_BYTES_NO_CRC <= data.len() {
            let (header, payload_offset) = AdtsHeader::parse(&data[pos..])?;
            let frame_len = header.aac_frame_length as usize;
            if frame_len < payload_offset || pos + frame_len > data.len() {
                return Err(Error::UnexpectedEnd);
            }
            frames.push(self.decode_adts_frame(&data[pos..pos + frame_len])?);
            pos += frame_len;
        }
        Ok(frames)
    }

    /// Decode one §4.4.2.3 Table 4.19 `er_raw_data_block()` payload
    /// (the ER General-Audio top-level payload) to interleaved 16-bit
    /// PCM.
    ///
    /// The ER object types do not use the tagged `raw_data_block()`
    /// element walk: the channel-element sequence is fixed by
    /// `channelConfiguration` (1..=7). Each element body is parsed
    /// through the error-resilient Table 4.50 branches selected by the
    /// ASC's [`AacResilienceFlags`] triplet, and — when
    /// `aacSpectralDataResilienceFlag` is set — the spectrum arrives
    /// as the two HCR length fields plus the
    /// `reordered_spectral_data()` payload decoded by
    /// [`crate::hcr_decode::decode_reordered_spectral_data`].
    ///
    /// Scope: the ER AAC LC (AOT 17), ER AAC LTP (AOT 19) and ER AAC
    /// LD (AOT 23) object types — the three §4.4.2.3 Table 4.19
    /// payloads. ER AAC scalable (AOT 20) rides its own layered
    /// `aac_scalable_main_element()` walk (see [`crate::scalable`])
    /// and is rejected here with [`Error::NotImplemented`]. For
    /// AOT 19 the §4.6.7 LTP tool is live: `ics_info()` carries the
    /// Table 4.55 non-LD `ltp_data()` branch (11-bit lag, `M = 0`),
    /// and the per-element [`crate::element_decode::ElementDecoder`]
    /// slots persist the §4.6.7.3 `x_rec` reconstruction history
    /// across frames exactly as the non-ER AOT-4 walk does. The
    /// trailing
    /// `extension_payload()` loop is consumed permissively (ignored),
    /// matching the FIL handling of the non-ER walk; `epConfig` 2 / 3
    /// physical-payload preprocessing (§4.5.2.4) is out of scope (the
    /// ASC parser already rejects those configurations).
    pub fn decode_er_raw_data_block(
        &mut self,
        aot: u8,
        fs_index: u8,
        sample_rate: u32,
        channel_configuration: u8,
        resilience: AacResilienceFlags,
        payload: &[u8],
    ) -> Result<DecodedFrame> {
        // AOT 17 (ER AAC LC), AOT 19 (ER AAC LTP) and AOT 23 (ER AAC
        // LD) share the Table 4.19 er_raw_data_block(). LD differs in
        // the 512/480-line frame family this decoder was configured
        // with (§4.6.17) and its delta-coded ltp_data() branch; AOT 19
        // adds the plain §4.6.7 LTP tool (Table 4.55 non-LD branch)
        // whose per-element reconstruction history the decoder slots
        // below already thread. ER AAC scalable (AOT 20) uses the
        // layered aac_scalable_main_element() walk instead and stays
        // out of this entry point.
        if aot != 17 && aot != 19 && aot != 23 {
            return Err(Error::NotImplemented);
        }
        // An LD stream must run an LD family and vice versa — a
        // mismatch means the caller never installed the ASC-resolved
        // family, which would silently mis-decode every band.
        if (aot == 23) != self.family.is_ld() {
            return Err(Error::ElementDecodeInvalid);
        }
        let fs = fs_index;
        let family = self.family;
        // Table 4.19: the fixed element sequence per channelConfiguration.
        let sequence: &[IdSynEle] = match channel_configuration {
            1 => &[IdSynEle::Sce],
            2 => &[IdSynEle::Cpe],
            3 => &[IdSynEle::Sce, IdSynEle::Cpe],
            4 => &[IdSynEle::Sce, IdSynEle::Cpe, IdSynEle::Sce],
            5 => &[IdSynEle::Sce, IdSynEle::Cpe, IdSynEle::Cpe],
            6 => &[IdSynEle::Sce, IdSynEle::Cpe, IdSynEle::Cpe, IdSynEle::Lfe],
            7 => &[
                IdSynEle::Sce,
                IdSynEle::Cpe,
                IdSynEle::Cpe,
                IdSynEle::Cpe,
                IdSynEle::Lfe,
            ],
            _ => return Err(Error::ElementDecodeInvalid),
        };

        let mut reader = BitReader::new(payload);
        let mut channels: Vec<Vec<f64>> = Vec::new();
        for &kind in sequence {
            let element_instance_tag = reader.read_u32(4).map_err(|_| Error::UnexpectedEnd)? as u8;
            let key = (kind_id(kind), element_instance_tag);
            match kind {
                IdSynEle::Sce | IdSynEle::Lfe => {
                    let body =
                        IcsBody::parse_er_family(&mut reader, family, aot, fs, false, resilience)?;
                    let ics = body.ics_info.clone().ok_or(Error::ElementDecodeInvalid)?;
                    let spectral =
                        parse_er_spectral(&mut reader, &body, &ics, fs, resilience, false)?;
                    let ch = ChannelInput {
                        body: &body,
                        ics_info: &ics,
                        spectral: &spectral,
                    };
                    let dec = self
                        .decoders
                        .entry(key)
                        .or_insert_with(|| ElementDecoder::new_family(family));
                    channels.push(dec.decode_sce(&ch, aot, fs)?);
                }
                IdSynEle::Cpe => {
                    let common_window = reader.read_bit().map_err(|_| Error::UnexpectedEnd)?;
                    let dec_out = if common_window {
                        // §4.4.2.3 shared ics_info + Table 4.4 ms_mask.
                        let ics = IcsInfo::parse_family(&mut reader, family, aot, fs, true)?;
                        let ms_bits = reader.read_u32(2).map_err(|_| Error::UnexpectedEnd)? as u8;
                        let ms_mask_present = MsMaskPresent::from_bits(ms_bits)?;
                        let mut ms_used: Vec<Vec<bool>> = Vec::new();
                        if ms_mask_present == MsMaskPresent::Mask {
                            for _g in 0..usize::from(ics.num_window_groups) {
                                let mut row = Vec::with_capacity(usize::from(ics.max_sfb));
                                for _sfb in 0..usize::from(ics.max_sfb) {
                                    row.push(reader.read_bit().map_err(|_| Error::UnexpectedEnd)?);
                                }
                                ms_used.push(row);
                            }
                        }
                        let left_body = IcsBody::parse_with_ics_info_er(
                            &mut reader,
                            &ics,
                            aot,
                            false,
                            resilience,
                        )?;
                        let left_spectral =
                            parse_er_spectral(&mut reader, &left_body, &ics, fs, resilience, true)?;
                        let right_body = IcsBody::parse_with_ics_info_er(
                            &mut reader,
                            &ics,
                            aot,
                            false,
                            resilience,
                        )?;
                        let right_spectral = parse_er_spectral(
                            &mut reader,
                            &right_body,
                            &ics,
                            fs,
                            resilience,
                            true,
                        )?;
                        let left = ChannelInput {
                            body: &left_body,
                            ics_info: &ics,
                            spectral: &left_spectral,
                        };
                        let right = ChannelInput {
                            body: &right_body,
                            ics_info: &ics,
                            spectral: &right_spectral,
                        };
                        let joint = CpeJointStereo {
                            ms_mask_present,
                            ms_used,
                        };
                        let dec = self
                            .decoders
                            .entry(key)
                            .or_insert_with(|| ElementDecoder::new_family(family));
                        dec.decode_cpe(&left, &right, &joint, aot, fs)?
                    } else {
                        let left_body = IcsBody::parse_er_family(
                            &mut reader,
                            family,
                            aot,
                            fs,
                            false,
                            resilience,
                        )?;
                        let left_ics = left_body
                            .ics_info
                            .clone()
                            .ok_or(Error::ElementDecodeInvalid)?;
                        let left_spectral = parse_er_spectral(
                            &mut reader,
                            &left_body,
                            &left_ics,
                            fs,
                            resilience,
                            true,
                        )?;
                        let right_body = IcsBody::parse_er_family(
                            &mut reader,
                            family,
                            aot,
                            fs,
                            false,
                            resilience,
                        )?;
                        let right_ics = right_body
                            .ics_info
                            .clone()
                            .ok_or(Error::ElementDecodeInvalid)?;
                        let right_spectral = parse_er_spectral(
                            &mut reader,
                            &right_body,
                            &right_ics,
                            fs,
                            resilience,
                            true,
                        )?;
                        let left = ChannelInput {
                            body: &left_body,
                            ics_info: &left_ics,
                            spectral: &left_spectral,
                        };
                        let right = ChannelInput {
                            body: &right_body,
                            ics_info: &right_ics,
                            spectral: &right_spectral,
                        };
                        let dec = self
                            .decoders
                            .entry(key)
                            .or_insert_with(|| ElementDecoder::new_family(family));
                        dec.decode_cpe(&left, &right, &CpeJointStereo::default(), aot, fs)?
                    };
                    channels.push(dec_out.0);
                    channels.push(dec_out.1);
                }
                _ => return Err(Error::ElementDecodeInvalid),
            }
        }
        // Trailing extension_payload() loop + byte_alignment(): consumed
        // permissively (nothing this decoder acts on rides there yet).

        // Table 1.19 canonical output reorder, same as the non-ER walk.
        let channels = crate::channel_map::reorder_channels(channel_configuration, channels);
        let pcm = interleave_s16(&channels)?;
        Ok(DecodedFrame {
            pcm,
            channels: channels.len(),
            sample_rate,
        })
    }

    // (the per-CPE parse lives in the free `parse_cpe` below so the
    // two-pass §4.6.8.3.3 coupling walk can defer decoding)
}

/// A parsed `channel_pair_element()` awaiting decode: both channels'
/// bodies + spectra and the Table 4.4 joint-stereo header. For the
/// `common_window == 1` form the shared `ics_info` is cloned into both
/// per-channel slots (the clone carries `ltp_data_pair`, so the
/// channel-1 LTP selection is unaffected).
pub(crate) struct ParsedCpe {
    joint: CpeJointStereo,
    left_body: IcsBody,
    left_ics: IcsInfo,
    left_spectral: SpectralData,
    right_body: IcsBody,
    right_ics: IcsInfo,
    right_spectral: SpectralData,
    /// Absolute bit position (in the reader's buffer) where the second
    /// `individual_channel_stream()` begins — the anchor of the
    /// 13818-7:2004 §8.1.1.1 128-bit second-ICS ADTS-CRC region.
    pub(crate) second_ics_start_bit: u64,
}

impl ParsedCpe {
    /// Borrow the two [`ChannelInput`]s plus the joint-stereo header.
    fn channel_inputs(&self) -> (ChannelInput<'_>, ChannelInput<'_>, &CpeJointStereo) {
        (
            ChannelInput {
                body: &self.left_body,
                ics_info: &self.left_ics,
                spectral: &self.left_spectral,
            },
            ChannelInput {
                body: &self.right_body,
                ics_info: &self.right_ics,
                spectral: &self.right_spectral,
            },
            &self.joint,
        )
    }
}

/// Parse one CPE body (after the walker consumed its element-instance
/// tag): the §4.4.2.3 `common_window` fork, the Table 4.4
/// `ms_mask_present` / `ms_used` joint-stereo header (shared form), and
/// both channels' `individual_channel_stream()` + `spectral_data()`.
pub(crate) fn parse_cpe(reader: &mut BitReader<'_>, aot: u8, fs: u8) -> Result<ParsedCpe> {
    parse_cpe_family(reader, FrameFamily::Lc1024, aot, fs)
}

/// [`parse_cpe`] under an explicit §4.5.1.1 frame-length family.
pub(crate) fn parse_cpe_family(
    reader: &mut BitReader<'_>,
    family: FrameFamily,
    aot: u8,
    fs: u8,
) -> Result<ParsedCpe> {
    let common_window = reader.read_bit().map_err(|_| Error::UnexpectedEnd)?;
    if common_window {
        // §4.4.2.3: shared ics_info, then the Table 4.4 ms_mask.
        let ics = IcsInfo::parse_family(reader, family, aot, fs, true)?;
        let ms_bits = reader.read_u32(2).map_err(|_| Error::UnexpectedEnd)? as u8;
        let ms_mask_present = MsMaskPresent::from_bits(ms_bits)?;
        let mut ms_used: Vec<Vec<bool>> = Vec::new();
        if ms_mask_present == MsMaskPresent::Mask {
            for _g in 0..usize::from(ics.num_window_groups) {
                let mut row = Vec::with_capacity(usize::from(ics.max_sfb));
                for _sfb in 0..usize::from(ics.max_sfb) {
                    row.push(reader.read_bit().map_err(|_| Error::UnexpectedEnd)?);
                }
                ms_used.push(row);
            }
        }
        let left_body = IcsBody::parse_with_ics_info(reader, &ics, aot, false)?;
        let left_spectral = SpectralData::parse(reader, &ics, &left_body.section_data, fs)?;
        let second_ics_start_bit = reader.bit_position();
        let right_body = IcsBody::parse_with_ics_info(reader, &ics, aot, false)?;
        let right_spectral = SpectralData::parse(reader, &ics, &right_body.section_data, fs)?;
        Ok(ParsedCpe {
            joint: CpeJointStereo {
                ms_mask_present,
                ms_used,
            },
            left_body,
            left_ics: ics.clone(),
            left_spectral,
            right_body,
            right_ics: ics,
            right_spectral,
            second_ics_start_bit,
        })
    } else {
        // Non-shared CPE: each channel carries its own ics_info; no
        // M/S mask, so the joint-stereo tools do not run.
        let left_body = IcsBody::parse_family(reader, family, aot, fs, false)?;
        let left_ics = left_body
            .ics_info
            .clone()
            .ok_or(Error::ElementDecodeInvalid)?;
        let left_spectral = SpectralData::parse(reader, &left_ics, &left_body.section_data, fs)?;
        let second_ics_start_bit = reader.bit_position();
        let right_body = IcsBody::parse_family(reader, family, aot, fs, false)?;
        let right_ics = right_body
            .ics_info
            .clone()
            .ok_or(Error::ElementDecodeInvalid)?;
        let right_spectral = SpectralData::parse(reader, &right_ics, &right_body.section_data, fs)?;
        Ok(ParsedCpe {
            joint: CpeJointStereo::default(),
            left_body,
            left_ics,
            left_spectral,
            right_body,
            right_ics,
            right_spectral,
            second_ics_start_bit,
        })
    }
}

/// Parse one ER channel's spectrum: the plain Table 4.56
/// `spectral_data()` when `aacSpectralDataResilienceFlag` is clear, or
/// the §4.6.16.3 `reordered_spectral_data()` payload (whose two length
/// fields the ER body already captured) decoded through
/// [`crate::hcr_decode::decode_reordered_spectral_data`].
fn parse_er_spectral(
    reader: &mut BitReader<'_>,
    body: &IcsBody,
    ics: &IcsInfo,
    fs: u8,
    resilience: AacResilienceFlags,
    is_cpe: bool,
) -> Result<SpectralData> {
    if !resilience.spectral_data {
        return SpectralData::parse(reader, ics, &body.section_data, fs);
    }
    let (len_reordered, len_longest) = body
        .reordered_spectral_lengths
        .ok_or(Error::ElementDecodeInvalid)?;
    let len = crate::hcr::clamp_reordered_length(len_reordered, is_cpe);
    // Gather the (not necessarily byte-aligned) payload bits.
    let mut buf = vec![0u8; usize::from(len).div_ceil(8)];
    for i in 0..usize::from(len) {
        if reader.read_bit().map_err(|_| Error::UnexpectedEnd)? {
            buf[i / 8] |= 0x80 >> (i % 8);
        }
    }
    crate::hcr_decode::decode_reordered_spectral_data(
        &buf,
        len,
        len_longest,
        ics,
        &body.section_data,
        fs,
    )
}

/// §4.6.8.3.3 `decode_coupling_channel()` — collect the coupling
/// contributions addressed at one target channel.
///
/// Walks every CCE of the same raw data block, replaying the spec's
/// target loop to assign `list_index` values: an SCE target consumes
/// one gain list; a CPE target consumes one shared list (`cc_l == cc_r
/// == 0`, applied to both channels), or one list per flagged channel.
/// `channel` selects the target channel of a CPE (`0` left, `1`
/// right); an SCE target only ever matches `channel == 0`.
fn coupling_for<'a>(
    cces: &'a [(u8, CouplingChannelElement)],
    decoded: &'a [DecodedCce],
    block: u8,
    kind: IdSynEle,
    tag: u8,
    channel: usize,
) -> Vec<CouplingApply<'a>> {
    let mut out = Vec::new();
    for ((cce_block, cce), dec) in cces.iter().zip(decoded.iter()) {
        if *cce_block != block {
            continue;
        }
        let mut list_index = 0usize;
        for t in &cce.header.targets {
            if !t.is_cpe {
                if kind == IdSynEle::Sce && tag == t.tag_select && channel == 0 {
                    out.push(CouplingApply {
                        cce,
                        decoded: dec,
                        list_index,
                    });
                }
                list_index += 1;
            } else {
                let addressed = kind == IdSynEle::Cpe && tag == t.tag_select;
                if !t.cc_l && !t.cc_r {
                    // Table 4.153 shared list: both channels couple
                    // with the same gain list.
                    if addressed {
                        out.push(CouplingApply {
                            cce,
                            decoded: dec,
                            list_index,
                        });
                    }
                    list_index += 1;
                }
                if t.cc_l {
                    if addressed && channel == 0 {
                        out.push(CouplingApply {
                            cce,
                            decoded: dec,
                            list_index,
                        });
                    }
                    list_index += 1;
                }
                if t.cc_r {
                    if addressed && channel == 1 {
                        out.push(CouplingApply {
                            cce,
                            decoded: dec,
                            list_index,
                        });
                    }
                    list_index += 1;
                }
            }
        }
    }
    out
}

/// Map a channel-element `id_syn_ele` to the slot key's first component
/// (the element decoders are keyed independently per syntactic-element
/// id so an SCE tag 0 and a CPE tag 0 never collide).
fn kind_id(kind: IdSynEle) -> u8 {
    match kind {
        IdSynEle::Sce => 0,
        IdSynEle::Cpe => 1,
        IdSynEle::Lfe => 3,
        _ => 9,
    }
}

fn unsupported_element(kind: IdSynEle) -> Error {
    // CCE (coupling) has no decode path; surface the element-decode
    // failure mode rather than a parse error so the caller can tell a
    // structural-OK-but-unsupported element apart from a malformed one.
    let _ = kind;
    Error::ElementDecodeInvalid
}

/// Skip a leading ID3v2 tag (`"ID3"` + 6-byte header + syncsafe size +
/// optional footer) if present; otherwise return the input unchanged.
fn skip_id3v2(data: &[u8]) -> &[u8] {
    if data.len() < 10 || &data[..3] != b"ID3" {
        return data;
    }
    let size = data[6..10]
        .iter()
        .fold(0usize, |acc, &b| (acc << 7) | usize::from(b & 0x7f));
    let footer = if data[5] & 0x10 != 0 { 10 } else { 0 };
    let total = 10 + size + footer;
    if total >= data.len() {
        data
    } else {
        &data[total..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_id3v2_passes_through_non_id3() {
        let data = [0xFFu8, 0xF1, 0x00, 0x00];
        assert_eq!(skip_id3v2(&data), &data);
    }

    #[test]
    fn skip_id3v2_strips_a_tag() {
        // "ID3", ver 4.0, no flags, syncsafe size = 4 → 10 + 4 = 14
        // bytes of tag, then a sentinel payload byte.
        let mut data = vec![b'I', b'D', b'3', 4, 0, 0, 0, 0, 0, 4];
        data.extend_from_slice(&[0; 4]);
        data.push(0xAB);
        assert_eq!(skip_id3v2(&data), &[0xABu8]);
    }

    #[test]
    fn skip_id3v2_keeps_tag_when_size_overruns() {
        // A declared size larger than the buffer leaves the data as-is
        // rather than panicking.
        let data = vec![b'I', b'D', b'3', 4, 0, 0, 0x7f, 0x7f, 0x7f, 0x7f];
        assert_eq!(skip_id3v2(&data), &data[..]);
    }

    #[test]
    fn kind_id_separates_sce_and_cpe() {
        assert_ne!(kind_id(IdSynEle::Sce), kind_id(IdSynEle::Cpe));
        assert_ne!(kind_id(IdSynEle::Lfe), kind_id(IdSynEle::Cpe));
    }
}

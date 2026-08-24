//! Matroska (.mkv) track inspection and sample packet demuxing via Symphonia.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackKind {
    Video,
    Audio,
    Other,
}

/// What one track is in, as far as this pipeline is concerned.
///
/// Video is passthrough — AVC and HEVC go into fMP4 as the bytes they already
/// are — and so is AAC audio. AC-3, E-AC-3 and DTS are the three the vendored
/// decoders handle: named here rather than lumped into `Unsupported` even in a
/// build with no decoder compiled in, because "AC-3, which this build cannot
/// decode" is a diagnostic and "unsupported" is a shrug. Whether a *named*
/// codec can actually be produced is a separate question, asked through
/// [`TrackCodec::is_playable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TrackCodec {
    Avc,
    Hevc,
    Aac,
    /// AC-3, "Dolby Digital" (ATSC A/52).
    Ac3,
    /// E-AC-3, "Dolby Digital Plus" (A/52 Annex E).
    Eac3,
    /// DTS Coherent Acoustics.
    Dts,
    #[default]
    Unsupported,
}

impl TrackCodec {
    /// The decoder this track needs, if it needs one at all.
    ///
    /// `None` covers both ends: a codec that is already playable everywhere
    /// (AAC), and one nothing here can do anything with.
    #[cfg(feature = "transcode")]
    pub fn transcode_codec(self) -> Option<crate::media::transcode::TranscodeCodec> {
        use crate::media::transcode::TranscodeCodec;
        match self {
            Self::Ac3 => Some(TranscodeCodec::Ac3),
            Self::Eac3 => Some(TranscodeCodec::Eac3),
            Self::Dts => Some(TranscodeCodec::Dts),
            _ => None,
        }
    }

    /// Whether this build can put this track in front of a browser or a TV —
    /// either by passing it through untouched, or by decoding it.
    ///
    /// The answer is a compile-time constant reached at runtime, so the
    /// playlist writer and the segment handler agree without either carrying a
    /// `#[cfg]`: a build without `transcode-dts` drops a DTS rendition rather
    /// than offering one it would then fail to produce.
    pub fn is_playable(self) -> bool {
        match self {
            Self::Avc | Self::Hevc | Self::Aac => true,
            #[cfg(feature = "transcode")]
            Self::Ac3 | Self::Eac3 | Self::Dts => self
                .transcode_codec()
                .is_some_and(|codec| codec.is_decodable() && cfg!(feature = "transcode-aac")),
            #[cfg(not(feature = "transcode"))]
            Self::Ac3 | Self::Eac3 | Self::Dts => false,
            Self::Unsupported => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    pub id: u32,
    pub track_kind: TrackKind,
    pub codec: String,
    pub codec_kind: TrackCodec,
    pub language: Option<String>,
    pub name: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Whether the container marks this the default track of its kind.
    ///
    /// Which of a film's audio tracks to carry, when it has several. Language is
    /// deliberately not consulted: a wrong guess is worse than a predictable
    /// one, and the fix if it turns out to matter is one resource per track.
    #[serde(default)]
    pub is_default: bool,
    /// Raw decoder config record for `codec_kind`: an AVC/HEVCDecoderConfigurationRecord
    /// for video (Matroska's CodecPrivate for these codecs already *is* this record), or
    /// the raw AudioSpecificConfig for AAC. Empty when `codec_kind` is `Unsupported`.
    pub extra_data: Vec<u8>,
}

/// Container-agnostic media packet with plain integer timestamps.
///
/// Timestamp units depend on the source container's timescale; callers must
/// know (or query) the timescale to interpret `pts`/`dts`/`duration` correctly.
#[derive(Debug, Clone)]
pub struct MediaPacket {
    pub track_id: u32,
    pub pts: u64,
    pub dts: u64,
    pub duration: u64,
    pub is_keyframe: bool,
    pub data: Vec<u8>,
}

/// File-level metadata extracted during probing.
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub tracks: Vec<TrackInfo>,
    /// Total duration of the file in seconds (if available from the container).
    pub duration_secs: Option<f64>,
}

/// The first video track this remuxer can pass through into browser-playable fMP4.
pub fn browser_video_track(tracks: &[TrackInfo]) -> Option<&TrackInfo> {
    tracks.iter().find(|t| {
        t.track_kind == TrackKind::Video && matches!(t.codec_kind, TrackCodec::Avc | TrackCodec::Hevc)
    })
}

/// Audio tracks this remuxer can pass through into browser-playable fMP4, in the same
/// order used for their `audio/{idx}/...` HLS rendition URLs. Both the playlist side
/// (`HlsGenerator`) and the segment-serving side (`web::remux_streaming`) must apply
/// this exact filter/order, or `idx` stops meaning the same track on both ends.
pub fn browser_audio_tracks(tracks: &[TrackInfo]) -> Vec<&TrackInfo> {
    tracks
        .iter()
        .filter(|t| t.track_kind == TrackKind::Audio && t.codec_kind.is_playable())
        .collect()
}

pub struct MkvDemuxer;

impl MkvDemuxer {
    /// Inspect tracks in a media file, returning track metadata and file-level
    /// info such as duration.
    #[cfg(feature = "casting")]
    pub fn inspect(path: &Path) -> Result<FileInfo> {
        use symphonia::core::codecs::audio::well_known::{
            CODEC_ID_AAC, CODEC_ID_AC3, CODEC_ID_DCA, CODEC_ID_EAC3, CODEC_ID_TRUEHD,
        };
        use symphonia::core::codecs::video::well_known::extra_data::{
            VIDEO_EXTRA_DATA_ID_AVC_DECODER_CONFIG, VIDEO_EXTRA_DATA_ID_HEVC_DECODER_CONFIG,
        };
        use symphonia::core::codecs::video::well_known::{CODEC_ID_H264, CODEC_ID_HEVC};
        use symphonia::core::formats::probe::Hint;
        use symphonia::core::formats::{FormatOptions, TrackFlags};
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::meta::MetadataOptions;
        use symphonia::core::units::Timestamp;

        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open MKV file: {}", path.display()))?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let format = symphonia::default::get_probe()
            .probe(
                &hint,
                stream,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .with_context(|| format!("Failed to probe media file: {}", path.display()))?;

        // The container's own duration (e.g. Matroska's Segment > Info > Duration),
        // rather than something derived per-track — this is what drives HLS segment
        // counts, so a missing/wrong value here truncates playback.
        let media_info = format.media_info();
        let duration_secs = match (media_info.time_base, media_info.duration) {
            (Some(time_base), Some(duration)) => time_base
                .calc_time(Timestamp::new(duration.get() as i64))
                .map(|t| t.as_secs_f64()),
            _ => None,
        };

        let mut tracks = Vec::new();

        for t in format.tracks() {
            let params = match t.codec_params.as_ref() {
                Some(p) => p,
                None => continue,
            };

            if let Some(v) = params.video() {
                let (codec_kind, codec_name, extra_data_id) = if v.codec == CODEC_ID_H264 {
                    (
                        TrackCodec::Avc,
                        "H.264",
                        Some(VIDEO_EXTRA_DATA_ID_AVC_DECODER_CONFIG),
                    )
                } else if v.codec == CODEC_ID_HEVC {
                    (
                        TrackCodec::Hevc,
                        "HEVC",
                        Some(VIDEO_EXTRA_DATA_ID_HEVC_DECODER_CONFIG),
                    )
                } else {
                    (TrackCodec::Unsupported, "Video", None)
                };
                let extra_data = extra_data_id
                    .and_then(|id| v.extra_data.iter().find(|e| e.id == id))
                    .map(|e| e.data.to_vec())
                    .unwrap_or_default();

                tracks.push(TrackInfo {
                    id: t.id,
                    track_kind: TrackKind::Video,
                    codec: codec_name.to_string(),
                    codec_kind,
                    language: t.language.clone(),
                    name: None,
                    sample_rate: None,
                    channels: None,
                    width: v.width.map(u32::from),
                    height: v.height.map(u32::from),
                    is_default: t.flags.contains(TrackFlags::DEFAULT),
                    extra_data,
                });
            } else if let Some(a) = params.audio() {
                let (codec_kind, codec_name) = match a.codec {
                    CODEC_ID_AAC => (TrackCodec::Aac, "AAC"),
                    CODEC_ID_AC3 => (TrackCodec::Ac3, "AC-3"),
                    CODEC_ID_EAC3 => (TrackCodec::Eac3, "E-AC-3"),
                    CODEC_ID_DCA => (TrackCodec::Dts, "DTS"),
                    CODEC_ID_TRUEHD => (TrackCodec::Unsupported, "TrueHD"),
                    _ => (TrackCodec::Unsupported, "Audio"),
                };
                // Only AAC carries a decoder config worth keeping. AC-3 and DTS
                // frames describe themselves in their own headers, so their
                // decoders need nothing from the container — and the AAC track
                // re-encoded *from* one of them gets its `AudioSpecificConfig`
                // from the encoder's shape, not from the source.
                let extra_data = if codec_kind == TrackCodec::Aac {
                    a.extra_data.as_ref().map(|d| d.to_vec()).unwrap_or_default()
                } else {
                    Vec::new()
                };

                tracks.push(TrackInfo {
                    id: t.id,
                    track_kind: TrackKind::Audio,
                    codec: codec_name.to_string(),
                    codec_kind,
                    language: t.language.clone(),
                    name: None,
                    sample_rate: a.sample_rate,
                    channels: a.channels.clone().map(|c| c.count() as u8),
                    width: None,
                    height: None,
                    is_default: t.flags.contains(TrackFlags::DEFAULT),
                    extra_data,
                });
            }
            // Subtitle and other track kinds aren't part of the browser remux path.
        }

        Ok(FileInfo {
            tracks,
            duration_secs,
        })
    }

    #[cfg(not(feature = "casting"))]
    pub fn inspect(_path: &Path) -> Result<FileInfo> {
        Err(anyhow::anyhow!("Casting/Symphonia feature not enabled"))
    }

    /// Legacy convenience wrapper — returns just the track list.
    pub fn inspect_tracks(path: &Path) -> Result<Vec<TrackInfo>> {
        Self::inspect(path).map(|fi| fi.tracks)
    }

    /// Extract packets for `target_track_id` covering roughly `target_duration_secs` of
    /// content, starting at (or just before) `start_secs` into the stream. Timestamps
    /// are rescaled from the track's native timebase (Matroska's `TimestampScale`,
    /// typically 1ms ticks) into `output_timescale` — the fMP4 writer's `mdhd`/`trun`
    /// boxes declare `output_timescale` as the track's timescale, so packet ticks must
    /// already be expressed in it, or every sample's duration ends up off by that scale
    /// factor.
    ///
    /// Stopping by accumulated duration rather than a fixed packet count matters: a
    /// fixed count tuned for one frame rate silently produces overlapping (too long) or
    /// gapped (too short) segments at any other frame rate, and either can make MSE
    /// reject appended segments or show visible stutter/duplication at segment
    /// boundaries.
    #[cfg(feature = "casting")]
    pub fn extract_track_packets(
        path: &Path,
        target_track_id: u32,
        codec: TrackCodec,
        output_timescale: u32,
        start_secs: f64,
        target_duration_secs: f64,
    ) -> Result<Vec<MediaPacket>> {
        // A safety valve, not a tuning knob: guards against runaway loops if a track's
        // packets never accumulate to `target_duration_secs` (e.g. a corrupt duration).
        const MAX_PACKETS_PER_SEGMENT: usize = 4096;
        use symphonia::core::formats::probe::Hint;
        use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::meta::MetadataOptions;
        use symphonia::core::units::Time;

        let file = std::fs::File::open(path)?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let mut format = symphonia::default::get_probe().probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )?;

        let track_time_base = format
            .tracks()
            .iter()
            .find(|t| t.id == target_track_id)
            .and_then(|t| t.time_base);

        // Seek to (at or before) the segment's start time instead of walking every
        // packet from the beginning of the file on every request — re-scanning from
        // zero for each of a few hundred segments is O(n^2) over a playback session.
        // A failed seek (e.g. `start_secs` is 0 and there's nothing to seek past) just
        // leaves the reader at the start, which is already correct.
        //
        // Coarse rather than Accurate: an HLS segment has to begin at a random-access
        // point or a player starting there has no reference frame to decode against.
        // Coarse lands on the container's own cue point (a keyframe) at or before the
        // requested time, where Accurate lands on the nearest sample — which is usually
        // mid-GOP, and produced segments a player could not start on.
        let target_time = Time::try_from_secs_f64(start_secs.max(0.0)).unwrap_or(Time::ZERO);
        let _ = format.seek(
            SeekMode::Coarse,
            SeekTo::Time {
                time: target_time,
                track_id: Some(target_track_id),
            },
        );

        let target_ticks =
            (target_duration_secs.max(0.0) * output_timescale as f64).round() as u64;

        let mut packets = Vec::new();
        let mut accumulated_ticks: u64 = 0;
        // Where the segment began on the presentation timeline, for the elapsed
        // check below.
        let mut first_pts: Option<u64> = None;

        loop {
            if packets.len() >= MAX_PACKETS_PER_SEGMENT {
                break;
            }
            // Never stop on an empty segment — always take at least one packet so a
            // segment request near the end of the stream doesn't come back empty.
            if accumulated_ticks >= target_ticks && !packets.is_empty() {
                break;
            }
            match format.next_packet() {
                Ok(Some(packet)) => {
                    if packet.track_id != target_track_id {
                        continue;
                    }
                    let rescale = |ticks: i64| -> u64 {
                        match track_time_base {
                            Some(tb) => rescale_ticks(ticks, tb, output_timescale),
                            None => ticks.max(0) as u64,
                        }
                    };
                    let pts = rescale(packet.pts.get());
                    let dts = rescale(packet.dts.get());
                    let dur = rescale(packet.dur.get() as i64);
                    let is_keyframe = packet_is_keyframe(&packet.data, codec);
                    // Guarantee the segment opens on a random-access point even where
                    // the container's cue index is sparse enough that the seek above
                    // landed mid-GOP. Dropping these leading frames loses nothing: they
                    // depend on references the player would not have when starting here,
                    // and the previous segment already covers their span.
                    if packets.is_empty() && !is_keyframe {
                        continue;
                    }
                    // Matroska stores no per-block duration: a `SimpleBlock` is a
                    // timestamp and a payload, and symphonia can only report a
                    // duration where the track declares `DefaultDuration` or the
                    // codec implies one. Accumulating durations alone therefore
                    // runs to the packet ceiling on any track that declares
                    // neither — which is a segment holding the whole film. The
                    // elapsed presentation time is the check that does not depend
                    // on the container being generous.
                    let elapsed = pts.saturating_sub(*first_pts.get_or_insert(pts));
                    if elapsed >= target_ticks && !packets.is_empty() {
                        break;
                    }
                    accumulated_ticks += dur;
                    packets.push(MediaPacket {
                        track_id: packet.track_id,
                        pts,
                        dts,
                        duration: dur,
                        is_keyframe,
                        data: packet.data.to_vec(),
                    });
                }
                Ok(None) => break,
                Err(symphonia::core::errors::Error::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(_) => break,
            }
        }

        derive_decode_timestamps(&mut packets);

        Ok(packets)
    }

    #[cfg(not(feature = "casting"))]
    pub fn extract_track_packets(
        _path: &Path,
        _target_track_id: u32,
        _codec: TrackCodec,
        _output_timescale: u32,
        _start_secs: f64,
        _target_duration_secs: f64,
    ) -> Result<Vec<MediaPacket>> {
        Err(anyhow::anyhow!("Casting/Symphonia feature not enabled"))
    }
}

/// Fill in each packet's decode timestamp for a run of packets that are in decode order
/// but carry only presentation timestamps.
///
/// Matroska stores a single timestamp per block — the *presentation* time — with blocks
/// laid out in decode order, and no decode timestamps anywhere. ISO-BMFF needs the
/// opposite: a monotonically increasing decode timeline (`tfdt` plus per-sample
/// durations) with a separate composition offset per sample. Symphonia passes the block
/// timestamp through as both `pts` and `dts`, so without this every sample would claim
/// `pts == dts` and B-frame content would be handed to the player in decode order —
/// visibly the wrong frame order, since nothing else in the fragment carries
/// presentation timing.
///
/// Recovering the decode timeline needs no lookahead or bitstream parsing: each frame is
/// decoded exactly once and presented exactly once, so the multiset of decode timestamps
/// is precisely the multiset of presentation timestamps. Sorting this run's presentation
/// timestamps therefore yields the decode timeline directly, and `pts - dts` recovers
/// each frame's composition offset (negative for frames presented before their decode
/// position, which `trun` version 1 stores as a signed value).
#[cfg(feature = "casting")]
pub(crate) fn derive_decode_timestamps(packets: &mut [MediaPacket]) {
    let mut decode_times: Vec<u64> = packets.iter().map(|p| p.pts).collect();
    decode_times.sort_unstable();
    for (packet, dts) in packets.iter_mut().zip(decode_times) {
        packet.dts = dts;
    }
}

/// Convert a tick count from a track's native timebase into `output_timescale` ticks
/// per second (e.g. Matroska's default 1ms-per-tick timebase into fMP4's 90kHz video
/// timescale). Uses 128-bit arithmetic since `ticks * numer * output_timescale` can
/// exceed 64 bits for a multi-hour file's later timestamps.
#[cfg(feature = "casting")]
pub(crate) fn rescale_ticks(ticks: i64, time_base: symphonia::core::units::TimeBase, output_timescale: u32) -> u64 {
    let ticks = i128::from(ticks.max(0));
    let numer = i128::from(time_base.numer.get());
    let denom = i128::from(time_base.denom.get());
    let output_timescale = i128::from(output_timescale);
    (ticks * numer * output_timescale / denom).max(0) as u64
}

/// Whether a length-prefixed AVC/HEVC sample contains a random-access (IDR/IRAP) NAL
/// unit. Matroska already stores AVC/HEVC samples length-prefixed (matching ISO-BMFF),
/// so no Annex-B start-code conversion is needed here — just walk the NAL units.
///
/// Assumes a 4-byte NAL length prefix, which is what Matroska muxers use in practice
/// (the AVC/HEVCDecoderConfigurationRecord's `lengthSizeMinusOne` is almost always 3).
#[cfg(feature = "casting")]
pub(crate) fn packet_is_keyframe(data: &[u8], codec: TrackCodec) -> bool {
    match codec {
        TrackCodec::Avc => nal_units(data, 4).any(|nal| !nal.is_empty() && (nal[0] & 0x1F) == 5),
        TrackCodec::Hevc => {
            nal_units(data, 4).any(|nal| nal.len() >= 2 && matches!((nal[0] >> 1) & 0x3F, 16..=23))
        }
        // Every AAC frame (and anything else passed through unexamined) is
        // independently decodable, so `true` is correct here, not a placeholder.
        _ => true,
    }
}

#[cfg(feature = "casting")]
fn nal_units(data: &[u8], length_size: usize) -> impl Iterator<Item = &[u8]> + '_ {
    let mut pos = 0usize;
    std::iter::from_fn(move || {
        if pos + length_size > data.len() {
            return None;
        }
        let mut len = 0usize;
        for &b in &data[pos..pos + length_size] {
            len = (len << 8) | (b as usize);
        }
        pos += length_size;
        if pos + len > data.len() {
            return None;
        }
        let nal = &data[pos..pos + len];
        pos += len;
        Some(nal)
    })
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    pub id: u32,
    pub track_kind: TrackKind,
    pub codec: String,
    pub language: Option<String>,
    pub name: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub width: Option<u32>,
    pub height: Option<u32>,
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

pub struct MkvDemuxer;

impl MkvDemuxer {
    /// Inspect tracks in a media file, returning track metadata and file-level
    /// info such as duration.
    #[cfg(feature = "casting")]
    pub fn inspect(path: &Path) -> Result<FileInfo> {
        use symphonia::core::formats::probe::Hint;
        use symphonia::core::formats::FormatOptions;
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::meta::MetadataOptions;

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

        let mut tracks = Vec::new();

        // Try to get file duration from the first track's time base + n_frames,
        // or from the format's default track.
        let duration_secs: Option<f64> = None;

        for t in format.tracks() {
            let params = match t.codec_params.as_ref() {
                Some(p) => p,
                None => continue,
            };

            let kind = if params.video().is_some() {
                TrackKind::Video
            } else if params.audio().is_some() {
                TrackKind::Audio
            } else {
                TrackKind::Other
            };

            if kind == TrackKind::Other {
                continue;
            }

            let codec = format!("{:?}", params);
            let sample_rate = params.audio().and_then(|a| a.sample_rate);
            let channels = params
                .audio()
                .and_then(|a| a.channels.clone())
                .map(|c| c.count() as u8);
            let width = params.video().and_then(|v| v.width).map(u32::from);
            let height = params.video().and_then(|v| v.height).map(u32::from);
            let extra_data = Vec::new();

            let language = t.language.clone();

            tracks.push(TrackInfo {
                id: t.id,
                track_kind: kind,
                codec,
                language,
                name: None,
                sample_rate,
                channels,
                width,
                height,
                extra_data,
            });
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

    /// Extract packets for a given track, skipping the first `skip_packets` and
    /// returning at most `max_packets`.
    #[cfg(feature = "casting")]
    pub fn extract_track_packets(
        path: &Path,
        target_track_id: u32,
        skip_packets: usize,
        max_packets: usize,
    ) -> Result<Vec<MediaPacket>> {
        use symphonia::core::formats::probe::Hint;
        use symphonia::core::formats::FormatOptions;
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::meta::MetadataOptions;

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

        let mut packets = Vec::new();
        let mut skipped = 0;

        loop {
            if packets.len() >= max_packets {
                break;
            }
            match format.next_packet() {
                Ok(Some(packet)) => {
                    if packet.track_id == target_track_id {
                        if skipped < skip_packets {
                            skipped += 1;
                            continue;
                        }
                        // Convert symphonia Timestamp/Duration to u64 via
                        // format!+parse — these newtypes have private inner
                        // fields but implement Display.
                        let pts = format!("{}", packet.pts).parse::<u64>().unwrap_or(0);
                        let dts = format!("{}", packet.dts).parse::<u64>().unwrap_or(0);
                        let dur = format!("{}", packet.dur).parse::<u64>().unwrap_or(0);
                        packets.push(MediaPacket {
                            track_id: packet.track_id,
                            pts,
                            dts,
                            duration: dur,
                            is_keyframe: true,
                            data: packet.data.to_vec(),
                        });
                    }
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

        Ok(packets)
    }

    #[cfg(not(feature = "casting"))]
    pub fn extract_track_packets(
        _path: &Path,
        _target_track_id: u32,
        _skip_packets: usize,
        _max_packets: usize,
    ) -> Result<Vec<MediaPacket>> {
        Err(anyhow::anyhow!("Casting/Symphonia feature not enabled"))
    }
}

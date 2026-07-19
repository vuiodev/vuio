//! Codec and container compatibility checking for playback targets.
//!
//! Before casting a media file to a target device, we verify that the
//! file's container format and (when known) codecs are compatible with
//! the target's capabilities.

use super::{PlaybackTarget, TargetCapabilities};
use serde::{Deserialize, Serialize};
use tracing::debug;

/// Result of a compatibility check.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum CompatResult {
    /// The media file is compatible with the target.
    Compatible,
    /// The media file is not compatible.
    Incompatible { reason: String },
    /// Compatibility could not be determined (missing metadata).
    Unknown { note: String },
}

impl CompatResult {
    pub fn is_compatible(&self) -> bool {
        matches!(
            self,
            CompatResult::Compatible | CompatResult::Unknown { .. }
        )
    }
}

/// Check whether a media file is compatible with a target device.
///
/// # Arguments
/// * `target` - The playback target to check against.
/// * `extension` - The file extension (e.g., "mp4", "mkv").
/// * `mime_type` - The MIME type of the file (e.g., "video/mp4").
pub fn check_compatibility(
    target: &PlaybackTarget,
    extension: &str,
    mime_type: &str,
) -> CompatResult {
    let ext = extension.to_lowercase();
    let mime = mime_type.to_lowercase();

    // Check container format
    let container_ok = check_container(&target.capabilities, &ext, &mime);
    if !container_ok {
        let result = CompatResult::Incompatible {
            reason: format!("{} does not support .{} container format", target.kind, ext),
        };
        debug!(
            target_id = %target.id,
            target_kind = %target.kind,
            extension = %ext,
            "Compatibility check failed: container not supported"
        );
        return result;
    }

    // For audio files, check audio codec compatibility via MIME type
    if mime.starts_with("audio/") {
        let audio_codec = audio_codec_from_mime(&mime);
        if let Some(codec) = audio_codec {
            if !target.capabilities.audio_codecs.iter().any(|c| c == &codec) {
                return CompatResult::Incompatible {
                    reason: format!("{} does not support {} audio codec", target.kind, codec),
                };
            }
        }
    }

    // For video files, we can infer the likely video codec from the container.
    // Without probing the file, we give a best-effort check.
    if mime.starts_with("video/") {
        let likely_codecs = likely_video_codecs_for_container(&ext);
        if !likely_codecs.is_empty() {
            let any_supported = likely_codecs
                .iter()
                .any(|codec| target.capabilities.video_codecs.iter().any(|c| c == codec));
            if !any_supported {
                return CompatResult::Unknown {
                    note: format!(
                        ".{} files may contain codecs not supported by {}; playback may fail",
                        ext, target.kind
                    ),
                };
            }
        }
    }

    debug!(
        target_id = %target.id,
        target_kind = %target.kind,
        extension = %ext,
        mime_type = %mime,
        "Compatibility check passed"
    );
    CompatResult::Compatible
}

/// Check if the container format is supported by the target.
fn check_container(caps: &TargetCapabilities, ext: &str, mime: &str) -> bool {
    // Direct extension match
    if caps.containers.iter().any(|c| c == ext) {
        return true;
    }

    // MIME-type based fallback
    let container_from_mime = match mime {
        "video/mp4" | "audio/mp4" | "audio/aac" => Some("mp4"),
        "video/x-matroska" | "audio/x-matroska" => Some("mkv"),
        "video/webm" | "audio/webm" => Some("webm"),
        "video/x-msvideo" => Some("avi"),
        "video/quicktime" => Some("mov"),
        "video/mp2t" => Some("ts"),
        "audio/mpeg" => Some("mp3"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        "audio/wav" | "audio/x-wav" => Some("wav"),
        "audio/ogg" | "video/ogg" => Some("ogg"),
        _ => None,
    };

    if let Some(container) = container_from_mime {
        return caps.containers.iter().any(|c| c == container);
    }

    false
}

/// Infer audio codec from MIME type.
fn audio_codec_from_mime(mime: &str) -> Option<String> {
    match mime {
        "audio/aac" | "audio/mp4" => Some("aac".into()),
        "audio/mpeg" => Some("mp3".into()),
        "audio/flac" | "audio/x-flac" => Some("flac".into()),
        "audio/wav" | "audio/x-wav" => Some("lpcm".into()),
        "audio/ogg" => Some("vorbis".into()),
        "audio/opus" => Some("opus".into()),
        "audio/webm" => Some("opus".into()),
        _ => None,
    }
}

/// Return likely video codecs for a given container extension.
fn likely_video_codecs_for_container(ext: &str) -> Vec<String> {
    match ext {
        "mp4" | "m4v" => vec!["h264".into(), "hevc".into()],
        "mkv" => vec!["h264".into(), "hevc".into(), "vp9".into(), "av1".into()],
        "webm" => vec!["vp8".into(), "vp9".into(), "av1".into()],
        "avi" => vec!["mpeg4".into(), "h264".into()],
        "mov" => vec!["h264".into(), "hevc".into()],
        "ts" | "m2ts" => vec!["h264".into(), "mpeg2".into()],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::{TargetCapabilities, TargetKind};
    use std::net::SocketAddr;

    fn make_target(kind: TargetKind) -> PlaybackTarget {
        PlaybackTarget {
            id: "test".into(),
            friendly_name: "Test Device".into(),
            kind,
            address: SocketAddr::from(([192, 168, 1, 100], 8009)),
            model: None,
            control_url: None,
            capabilities: match kind {
                TargetKind::Chromecast => TargetCapabilities::chromecast(),
                TargetKind::AirPlay => TargetCapabilities::airplay(),
                TargetKind::Dlna => TargetCapabilities::dlna(),
                TargetKind::Dial => TargetCapabilities::dial(),
            },
        }
    }

    #[test]
    fn chromecast_supports_mp4_video() {
        let target = make_target(TargetKind::Chromecast);
        let result = check_compatibility(&target, "mp4", "video/mp4");
        assert!(result.is_compatible());
    }

    #[test]
    fn chromecast_supports_webm_video() {
        let target = make_target(TargetKind::Chromecast);
        let result = check_compatibility(&target, "webm", "video/webm");
        assert!(result.is_compatible());
    }

    #[test]
    fn airplay_rejects_webm() {
        let target = make_target(TargetKind::AirPlay);
        let result = check_compatibility(&target, "webm", "video/webm");
        assert!(!result.is_compatible());
    }

    #[test]
    fn airplay_supports_mov() {
        let target = make_target(TargetKind::AirPlay);
        let result = check_compatibility(&target, "mov", "video/quicktime");
        assert!(result.is_compatible());
    }

    #[test]
    fn chromecast_supports_mp3_audio() {
        let target = make_target(TargetKind::Chromecast);
        let result = check_compatibility(&target, "mp3", "audio/mpeg");
        assert!(result.is_compatible());
    }

    #[test]
    fn airplay_rejects_opus_audio() {
        let target = make_target(TargetKind::AirPlay);
        let result = check_compatibility(&target, "opus", "audio/opus");
        assert!(!result.is_compatible());
    }

    #[test]
    fn dlna_supports_avi() {
        let target = make_target(TargetKind::Dlna);
        let result = check_compatibility(&target, "avi", "video/x-msvideo");
        assert!(result.is_compatible());
    }

    #[test]
    fn compat_result_serialize() {
        let result = CompatResult::Compatible;
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"status\":\"compatible\""));
    }
}

//! HLS playlist generator supporting RFC 8216 multi-audio tracks.

use super::mkv_demuxer::{browser_audio_tracks, browser_video_track, TrackCodec, TrackInfo};

pub struct HlsGenerator;

impl HlsGenerator {
    /// Generate HLS Master Playlist (`master.m3u8`) for an MKV file containing multi-audio tracks.
    ///
    /// Only tracks this remuxer can actually pass through into fMP4 are offered: a video
    /// track must be AVC/HEVC, and only AAC audio tracks are listed as selectable
    /// renditions (E-AC-3/AC-3/DTS/TrueHD/etc. audio has no in-browser decoder, so
    /// serving it would just be a silent/broken rendition).
    pub fn build_master_playlist(_media_id: &str, tracks: &[TrackInfo]) -> String {
        let Some(video_track) = browser_video_track(tracks) else {
            // No browser-playable video track: a variant-less master playlist fails
            // hls.js's manifest parse, which routes the player to its existing
            // "can't play this" fallback instead of trying to decode something that
            // can't work.
            return "#EXTM3U\n#EXT-X-VERSION:6\n".to_string();
        };

        let audio_tracks = browser_audio_tracks(tracks);

        let mut playlist = String::new();
        playlist.push_str("#EXTM3U\n");
        playlist.push_str("#EXT-X-VERSION:6\n");
        playlist.push_str("#EXT-X-INDEPENDENT-SEGMENTS\n\n");

        let video_codec = video_codec_string(video_track);

        if !audio_tracks.is_empty() {
            for (idx, track) in audio_tracks.iter().enumerate() {
                let name = track
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Audio Track {}", idx + 1));
                let lang = track.language.as_deref().unwrap_or("und");
                let is_default = if idx == 0 { "YES" } else { "NO" };

                playlist.push_str(&format!(
                    "#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",NAME=\"{}\",DEFAULT={},AUTOSELECT=YES,LANGUAGE=\"{}\",URI=\"audio/{}/index.m3u8\"\n",
                    name, is_default, lang, idx
                ));
            }
            playlist.push('\n');
            let audio_codec = aac_codec_string(&audio_tracks[0].extra_data);
            playlist.push_str(&format!(
                "#EXT-X-STREAM-INF:BANDWIDTH=5000000,CODECS=\"{},{}\",AUDIO=\"audio\"\n",
                video_codec, audio_codec
            ));
            playlist.push_str("video/index.m3u8\n");
        } else {
            // Video-only: either the file has no audio, or none of its audio tracks
            // are browser-playable.
            playlist.push_str(&format!(
                "#EXT-X-STREAM-INF:BANDWIDTH=5000000,CODECS=\"{}\"\n",
                video_codec
            ));
            playlist.push_str("video/index.m3u8\n");
        }

        playlist
    }

    /// Generate an HLS Media Playlist (`index.m3u8`) for a video or audio rendition.
    ///
    /// Every rendition's init segment and segments live alongside its own playlist
    /// (`init.mp4`, `segment/{n}`), so this is identical for video and audio callers.
    pub fn build_media_playlist(total_duration_secs: f64, segment_duration_secs: u32) -> String {
        let segment_duration_secs = segment_duration_secs.max(1);
        let total_duration_secs = total_duration_secs.max(0.0);
        let segment_count = (total_duration_secs / segment_duration_secs as f64)
            .ceil()
            .max(1.0) as usize;

        let mut playlist = String::new();
        playlist.push_str("#EXTM3U\n");
        playlist.push_str("#EXT-X-VERSION:6\n");
        playlist.push_str(&format!(
            "#EXT-X-TARGETDURATION:{}\n",
            segment_duration_secs
        ));
        playlist.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
        playlist.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");
        playlist.push_str("#EXT-X-MAP:URI=\"init.mp4\"\n\n");

        let mut remaining = total_duration_secs;
        for i in 0..segment_count {
            // Every segment is `segment_duration_secs` except the last, which is
            // whatever real time is left — declaring the nominal duration for a
            // shorter final segment drifts the reported vs. actual timeline and can
            // make players stall waiting for content that was never coming.
            let this_duration = if i + 1 == segment_count {
                remaining.max(0.0)
            } else {
                segment_duration_secs as f64
            };
            remaining -= this_duration;

            playlist.push_str(&format!("#EXTINF:{:.3},\nsegment/{}\n", this_duration, i));
        }

        playlist.push_str("#EXT-X-ENDLIST\n");
        playlist
    }
}

/// Derive an `avc1.PPCCLL`/generic `hvc1...` CODECS string. hls.js/MSE only use this for
/// a `MediaSource.isTypeSupported` capability pre-check — actual decode relies on the
/// real VPS/SPS/PPS carried in the init segment's `avcC`/`hvcC` box, not this string.
fn video_codec_string(track: &TrackInfo) -> String {
    match track.codec_kind {
        TrackCodec::Avc if track.extra_data.len() >= 4 => format!(
            "avc1.{:02x}{:02x}{:02x}",
            track.extra_data[1], track.extra_data[2], track.extra_data[3]
        ),
        TrackCodec::Hevc => "hvc1.1.6.L93.B0".to_string(),
        _ => "avc1.640028".to_string(),
    }
}

/// Derive an `mp4a.40.{audioObjectType}` CODECS string from the leading bits of the raw
/// AAC `AudioSpecificConfig` (audioObjectType is the top 5 bits of the first byte).
fn aac_codec_string(extra_data: &[u8]) -> String {
    let audio_object_type = extra_data
        .first()
        .map(|b| b >> 3)
        .filter(|&t| t != 0)
        .unwrap_or(2); // 2 == AAC-LC, the common/safe default.
    format!("mp4a.40.{}", audio_object_type)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::mkv_demuxer::TrackKind;

    fn video_track(codec_kind: TrackCodec) -> TrackInfo {
        TrackInfo {
            id: 1,
            track_kind: TrackKind::Video,
            codec: "H264".into(),
            codec_kind,
            language: None,
            name: None,
            sample_rate: None,
            channels: None,
            width: Some(1920),
            height: Some(1080),
            extra_data: vec![0x01, 0x64, 0x00, 0x28],
        }
    }

    fn audio_track(id: u32, codec_kind: TrackCodec, name: &str, lang: &str) -> TrackInfo {
        TrackInfo {
            id,
            track_kind: TrackKind::Audio,
            codec: "AAC".into(),
            codec_kind,
            language: Some(lang.into()),
            name: Some(name.into()),
            sample_rate: Some(48000),
            channels: Some(if codec_kind == TrackCodec::Aac { 2 } else { 6 }),
            width: None,
            height: None,
            extra_data: vec![0x11, 0x90],
        }
    }

    #[test]
    fn test_master_playlist_multi_audio() {
        let tracks = vec![
            video_track(TrackCodec::Avc),
            audio_track(2, TrackCodec::Aac, "English", "eng"),
            audio_track(3, TrackCodec::Aac, "Spanish", "spa"),
        ];

        let master = HlsGenerator::build_master_playlist("test-id", &tracks);
        assert!(master.contains("#EXT-X-MEDIA:TYPE=AUDIO"));
        assert!(master.contains("NAME=\"English\""));
        assert!(master.contains("NAME=\"Spanish\""));
        assert!(master.contains("audio/0/index.m3u8"));
        assert!(master.contains("audio/1/index.m3u8"));
    }

    #[test]
    fn test_master_playlist_excludes_unsupported_audio_codecs() {
        // Mirrors a real WEB-DL release: AVC video with only E-AC-3/AC-3 audio tracks —
        // none of those tracks can be decoded in-browser, so none should be offered.
        let tracks = vec![
            video_track(TrackCodec::Avc),
            audio_track(2, TrackCodec::Unsupported, "5.1 Atmos", "eng"),
            audio_track(3, TrackCodec::Unsupported, "Stereo", "eng"),
        ];

        let master = HlsGenerator::build_master_playlist("test-id", &tracks);
        assert!(!master.contains("#EXT-X-MEDIA:TYPE=AUDIO"));
        assert!(!master.contains("AUDIO=\"audio\""));
        assert!(master.contains("video/index.m3u8"));
    }

    #[test]
    fn test_master_playlist_no_supported_video_is_variant_less() {
        let tracks = vec![video_track(TrackCodec::Unsupported)];
        let master = HlsGenerator::build_master_playlist("test-id", &tracks);
        assert!(!master.contains("#EXT-X-STREAM-INF"));
    }

    #[test]
    fn test_media_playlist_final_segment_uses_real_remainder() {
        // 10 seconds of content at a 4-second target: 4, 4, then a 2-second remainder —
        // not another 4-second entry that overruns the real content.
        let playlist = HlsGenerator::build_media_playlist(10.0, 4);
        assert!(playlist.contains("#EXTINF:4.000,\nsegment/0\n"));
        assert!(playlist.contains("#EXTINF:4.000,\nsegment/1\n"));
        assert!(playlist.contains("#EXTINF:2.000,\nsegment/2\n"));
        assert!(!playlist.contains("segment/3"));
        assert!(playlist.contains("#EXT-X-MAP:URI=\"init.mp4\""));
    }

    #[test]
    fn test_video_codec_string_uses_real_avcc_bytes() {
        let track = video_track(TrackCodec::Avc);
        assert_eq!(video_codec_string(&track), "avc1.640028");
    }
}

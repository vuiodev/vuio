//! HLS playlist generator supporting RFC 8216 multi-audio tracks.

use super::mkv_demuxer::{TrackInfo, TrackKind};

pub struct HlsGenerator;

impl HlsGenerator {
    /// Generate HLS Master Playlist (`master.m3u8`) for an MKV file containing multi-audio tracks.
    pub fn build_master_playlist(_media_id: &str, tracks: &[TrackInfo]) -> String {
        let mut playlist = String::new();
        playlist.push_str("#EXTM3U\n");
        playlist.push_str("#EXT-X-VERSION:6\n");
        playlist.push_str("#EXT-X-INDEPENDENT-SEGMENTS\n\n");

        let audio_tracks: Vec<&TrackInfo> = tracks
            .iter()
            .filter(|t| t.track_kind == TrackKind::Audio)
            .collect();

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
            playlist.push_str(
                "#EXT-X-STREAM-INF:BANDWIDTH=5000000,CODECS=\"avc1.640028,mp4a.40.2\",AUDIO=\"audio\"\n",
            );
            playlist.push_str("video/index.m3u8\n");
        } else {
            // Video-only fallback
            playlist.push_str(
                "#EXT-X-STREAM-INF:BANDWIDTH=5000000,CODECS=\"avc1.640028\"\n",
            );
            playlist.push_str("video/index.m3u8\n");
        }

        playlist
    }

    /// Generate HLS Media Playlist (`index.m3u8`) for video or audio stream.
    pub fn build_media_playlist(
        init_segment_name: &str,
        segment_count: usize,
        target_duration_secs: u32,
    ) -> String {
        let mut playlist = String::new();
        playlist.push_str("#EXTM3U\n");
        playlist.push_str("#EXT-X-VERSION:6\n");
        playlist.push_str(&format!(
            "#EXT-X-TARGETDURATION:{}\n",
            target_duration_secs
        ));
        playlist.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
        playlist.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");
        playlist.push_str(&format!(
            "#EXT-X-MAP:URI=\"{}\"\n\n",
            init_segment_name
        ));

        for i in 0..segment_count {
            playlist.push_str(&format!(
                "#EXTINF:{:.3},\nsegment/{}\n",
                target_duration_secs as f64, i
            ));
        }

        playlist.push_str("#EXT-X-ENDLIST\n");
        playlist
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_master_playlist_multi_audio() {
        let tracks = vec![
            TrackInfo {
                id: 1,
                track_kind: TrackKind::Video,
                codec: "H264".into(),
                language: None,
                name: None,
                sample_rate: None,
                channels: None,
                width: Some(1920),
                height: Some(1080),
                extra_data: vec![],
            },
            TrackInfo {
                id: 2,
                track_kind: TrackKind::Audio,
                codec: "AAC".into(),
                language: Some("eng".into()),
                name: Some("English".into()),
                sample_rate: Some(48000),
                channels: Some(6),
                width: None,
                height: None,
                extra_data: vec![],
            },
            TrackInfo {
                id: 3,
                track_kind: TrackKind::Audio,
                codec: "AAC".into(),
                language: Some("spa".into()),
                name: Some("Spanish".into()),
                sample_rate: Some(48000),
                channels: Some(2),
                width: None,
                height: None,
                extra_data: vec![],
            },
        ];

        let master = HlsGenerator::build_master_playlist("test-id", &tracks);
        assert!(master.contains("#EXT-X-MEDIA:TYPE=AUDIO"));
        assert!(master.contains("NAME=\"English\""));
        assert!(master.contains("NAME=\"Spanish\""));
        assert!(master.contains("audio/0/index.m3u8"));
        assert!(master.contains("audio/1/index.m3u8"));
    }
}

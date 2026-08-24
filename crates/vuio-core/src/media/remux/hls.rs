//! HLS playlist generator supporting RFC 8216 multi-audio tracks.

use super::mkv_demuxer::{browser_audio_tracks, browser_video_track, TrackCodec, TrackInfo};

pub struct HlsGenerator;

impl HlsGenerator {
    /// Generate HLS Master Playlist (`master.m3u8`) for an MKV file containing multi-audio tracks.
    ///
    /// Only tracks this build can actually produce are offered: a video track must be
    /// AVC/HEVC, and an audio track must be either AAC (passed through) or one of the
    /// three codecs the vendored decoders handle, re-encoded to AAC on the way out. A
    /// build compiled without the matching decoder — or a codec nothing here decodes,
    /// TrueHD being the one that turns up in real libraries — drops the rendition
    /// rather than offering one that would arrive silent.
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
                let name = format_audio_track_name(track, idx);
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

fn language_name(code: &str) -> &'static str {
    match code.to_ascii_lowercase().as_str() {
        "eng" | "en" => "English",
        "spa" | "es" => "Spanish",
        "fra" | "fre" | "fr" => "French",
        "deu" | "ger" | "de" => "German",
        "ita" | "it" => "Italian",
        "jpn" | "ja" => "Japanese",
        "rus" | "ru" => "Russian",
        "zho" | "chi" | "zh" => "Chinese",
        "kor" | "ko" => "Korean",
        "por" | "pt" => "Portuguese",
        "hin" | "hi" => "Hindi",
        "ara" | "ar" => "Arabic",
        "pol" | "pl" => "Polish",
        "ukr" | "uk" => "Ukrainian",
        "vie" | "vi" => "Vietnamese",
        "tur" | "tr" => "Turkish",
        "nld" | "dut" | "nl" => "Dutch",
        "swe" | "sv" => "Swedish",
        "nor" | "no" => "Norwegian",
        "dan" | "da" => "Danish",
        "fin" | "fi" => "Finnish",
        "ces" | "cze" | "cs" => "Czech",
        "hun" | "hu" => "Hungarian",
        "ron" | "rum" | "ro" => "Romanian",
        "ell" | "gre" | "el" => "Greek",
        "heb" | "he" => "Hebrew",
        "tha" | "th" => "Thai",
        _ => "",
    }
}

pub fn format_audio_track_name(track: &TrackInfo, idx: usize) -> String {
    if let Some(ref name) = track.name {
        let trimmed = name.trim();
        if !trimmed.is_empty()
            && !trimmed.eq_ignore_ascii_case("und")
            && !trimmed.starts_with("Audio Track")
        {
            return trimmed.to_string();
        }
    }

    let lang_code = track.language.as_deref().unwrap_or("");
    let lang_display = if !lang_code.is_empty() && lang_code != "und" {
        let name = language_name(lang_code);
        if !name.is_empty() {
            name
        } else {
            lang_code
        }
    } else {
        ""
    };

    let channels_str = match track.channels {
        Some(6) => "5.1",
        Some(8) => "7.1",
        Some(2) => "Stereo",
        Some(1) => "Mono",
        Some(c) if c > 2 => return format!("Track {} ({}ch {})", idx + 1, c, track.codec),
        _ => "",
    };

    let main_label = if !lang_display.is_empty() {
        lang_display.to_string()
    } else {
        format!("Audio Track {}", idx + 1)
    };

    let mut details = Vec::new();
    if !channels_str.is_empty() {
        details.push(channels_str);
    }
    if !track.codec.is_empty() {
        details.push(&track.codec);
    }

    if !details.is_empty() {
        format!("{} ({})", main_label, details.join(" "))
    } else {
        main_label
    }
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
            is_default: false,
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
            is_default: false,
            // Only an AAC source carries an AudioSpecificConfig; a decoded track's
            // config comes from the encoder, and the writer must cope with neither.
            extra_data: if codec_kind == TrackCodec::Aac {
                vec![0x11, 0x90]
            } else {
                Vec::new()
            },
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
    fn test_master_playlist_excludes_audio_this_build_cannot_produce() {
        // TrueHD: identified, named, and decoded by nothing vendored. A rendition
        // pointing at it would arrive silent, so it must not be listed — in any build.
        let tracks = vec![
            video_track(TrackCodec::Avc),
            audio_track(2, TrackCodec::Unsupported, "TrueHD Atmos", "eng"),
        ];

        let master = HlsGenerator::build_master_playlist("test-id", &tracks);
        assert!(!master.contains("#EXT-X-MEDIA:TYPE=AUDIO"));
        assert!(!master.contains("AUDIO=\"audio\""));
        assert!(master.contains("video/index.m3u8"));
    }

    /// The contract that replaced "AAC only": a rendition is offered exactly when
    /// this build can produce it. Compiled with the decoder, an AC-3 track becomes a
    /// selectable rendition; compiled without, it disappears rather than being
    /// advertised and then failing.
    #[test]
    fn test_master_playlist_offers_ac3_only_when_this_build_can_decode_it() {
        let tracks = vec![
            video_track(TrackCodec::Avc),
            audio_track(2, TrackCodec::Ac3, "5.1 English", "eng"),
        ];
        let master = HlsGenerator::build_master_playlist("test-id", &tracks);

        if TrackCodec::Ac3.is_playable() {
            assert!(master.contains("#EXT-X-MEDIA:TYPE=AUDIO"));
            assert!(master.contains("NAME=\"5.1 English\""));
            assert!(master.contains("audio/0/index.m3u8"));
            // Re-encoded, so the rendition really is AAC-LC whatever the source was.
            assert!(master.contains("mp4a.40.2"), "{master}");
        } else {
            assert!(!master.contains("#EXT-X-MEDIA:TYPE=AUDIO"), "{master}");
        }
    }

    #[test]
    fn test_master_playlist_offers_dts_only_when_this_build_can_decode_it() {
        let tracks = vec![
            video_track(TrackCodec::Avc),
            audio_track(2, TrackCodec::Dts, "DTS", "eng"),
        ];
        let master = HlsGenerator::build_master_playlist("test-id", &tracks);
        assert_eq!(
            master.contains("#EXT-X-MEDIA:TYPE=AUDIO"),
            TrackCodec::Dts.is_playable(),
            "a DTS rendition must appear exactly when this build can decode DTS"
        );
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

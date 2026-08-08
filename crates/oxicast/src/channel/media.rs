//! Cast media channel — LOAD, PLAY, PAUSE, SEEK, STOP, QUEUE operations.

use super::ns;
use crate::client::framing::build_message;
use crate::proto::CastMessage;
use crate::types::*;

/// Build a LOAD media request.
pub fn load(
    request_id: u32,
    transport_id: &str,
    session_id: &str,
    media: &MediaInfo,
    autoplay: bool,
    current_time: f64,
    custom_data: Option<&serde_json::Value>,
) -> CastMessage {
    let mut media_obj = serde_json::json!({
        "contentId": media.content_id,
        "contentType": media.content_type,
        "streamType": match media.stream_type {
            StreamType::Buffered => "BUFFERED",
            StreamType::Live => "LIVE",
            StreamType::None => "NONE",
        },
    });

    if let Some(dur) = media.duration {
        media_obj["duration"] = serde_json::json!(dur);
    }
    if let Some(ref meta) = media.metadata {
        media_obj["metadata"] = serialize_metadata(meta);
    }

    let payload = serde_json::json!({
        "type": ns::MSG_LOAD,
        "requestId": request_id,
        "sessionId": session_id,
        "media": media_obj,
        "autoplay": autoplay,
        "currentTime": current_time,
        "customData": custom_data.unwrap_or(&serde_json::json!({})),
    });

    build_message(ns::NS_MEDIA, ns::SENDER_ID, transport_id, &payload.to_string())
}

/// Build a PLAY (resume) request.
pub fn play(request_id: u32, transport_id: &str, media_session_id: i32) -> CastMessage {
    let payload = serde_json::json!({
        "type": ns::MSG_PLAY,
        "requestId": request_id,
        "mediaSessionId": media_session_id,
    });
    build_message(ns::NS_MEDIA, ns::SENDER_ID, transport_id, &payload.to_string())
}

/// Build a PAUSE request.
pub fn pause(request_id: u32, transport_id: &str, media_session_id: i32) -> CastMessage {
    let payload = serde_json::json!({
        "type": ns::MSG_PAUSE,
        "requestId": request_id,
        "mediaSessionId": media_session_id,
    });
    build_message(ns::NS_MEDIA, ns::SENDER_ID, transport_id, &payload.to_string())
}

/// Build a STOP media request (ends the media session).
pub fn stop(request_id: u32, transport_id: &str, media_session_id: i32) -> CastMessage {
    let payload = serde_json::json!({
        "type": ns::MSG_MEDIA_STOP,
        "requestId": request_id,
        "mediaSessionId": media_session_id,
        "customData": {},
    });
    build_message(ns::NS_MEDIA, ns::SENDER_ID, transport_id, &payload.to_string())
}

/// Build a SEEK request.
pub fn seek(
    request_id: u32,
    transport_id: &str,
    media_session_id: i32,
    position: f64,
) -> CastMessage {
    let payload = serde_json::json!({
        "type": ns::MSG_SEEK,
        "requestId": request_id,
        "mediaSessionId": media_session_id,
        "currentTime": position,
    });
    build_message(ns::NS_MEDIA, ns::SENDER_ID, transport_id, &payload.to_string())
}

/// Build a GET_STATUS request for the media channel.
pub fn get_status(request_id: u32, transport_id: &str) -> CastMessage {
    let payload = serde_json::json!({
        "type": ns::MSG_GET_STATUS,
        "requestId": request_id,
    });
    build_message(ns::NS_MEDIA, ns::SENDER_ID, transport_id, &payload.to_string())
}

/// Serialize a QueueItem's media into the wire JSON format (reuses load's logic).
fn serialize_queue_item(item: &QueueItem) -> serde_json::Value {
    let mut media_obj = serde_json::json!({
        "contentId": item.media.content_id,
        "contentType": item.media.content_type,
        "streamType": match item.media.stream_type {
            StreamType::Buffered => "BUFFERED",
            StreamType::Live => "LIVE",
            StreamType::None => "NONE",
        },
    });
    if let Some(dur) = item.media.duration {
        media_obj["duration"] = serde_json::json!(dur);
    }
    if let Some(ref meta) = item.media.metadata {
        media_obj["metadata"] = serialize_metadata(meta);
    }
    serde_json::json!({
        "media": media_obj,
        "autoplay": item.autoplay,
        "startTime": item.start_time,
    })
}

/// Build a QUEUE_LOAD request.
pub fn queue_load(
    request_id: u32,
    transport_id: &str,
    session_id: &str,
    items: &[QueueItem],
    start_index: u32,
    repeat_mode: RepeatMode,
) -> CastMessage {
    let queue_items: Vec<serde_json::Value> = items.iter().map(serialize_queue_item).collect();

    let repeat = match repeat_mode {
        RepeatMode::RepeatOff => "REPEAT_OFF",
        RepeatMode::RepeatAll => "REPEAT_ALL",
        RepeatMode::RepeatSingle => "REPEAT_SINGLE",
        RepeatMode::RepeatAllAndShuffle => "REPEAT_ALL_AND_SHUFFLE",
    };

    let payload = serde_json::json!({
        "type": ns::MSG_QUEUE_LOAD,
        "requestId": request_id,
        "sessionId": session_id,
        "items": queue_items,
        "startIndex": start_index,
        "repeatMode": repeat,
    });

    build_message(ns::NS_MEDIA, ns::SENDER_ID, transport_id, &payload.to_string())
}

/// Build a QUEUE_INSERT request.
pub fn queue_insert(
    request_id: u32,
    transport_id: &str,
    media_session_id: i32,
    items: &[QueueItem],
    insert_before: Option<u32>,
) -> CastMessage {
    let queue_items: Vec<serde_json::Value> = items.iter().map(serialize_queue_item).collect();

    let mut payload = serde_json::json!({
        "type": ns::MSG_QUEUE_INSERT,
        "requestId": request_id,
        "mediaSessionId": media_session_id,
        "items": queue_items,
    });

    if let Some(before) = insert_before {
        payload["insertBefore"] = serde_json::json!(before);
    }

    build_message(ns::NS_MEDIA, ns::SENDER_ID, transport_id, &payload.to_string())
}

/// Insert a field into a JSON map only if the value is Some.
fn insert_opt<V: Into<serde_json::Value> + Clone>(
    map: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: &Option<V>,
) {
    if let Some(v) = value {
        map.insert(key.into(), v.clone().into());
    }
}

fn serialize_metadata(meta: &MediaMetadata) -> serde_json::Value {
    let mut m = serde_json::Map::new();

    match meta {
        MediaMetadata::Generic { title, subtitle, images } => {
            m.insert("metadataType".into(), 0.into());
            insert_opt(&mut m, "title", title);
            insert_opt(&mut m, "subtitle", subtitle);
            m.insert("images".into(), serialize_images(images));
        }
        MediaMetadata::Movie { title, subtitle, studio, images } => {
            m.insert("metadataType".into(), 1.into());
            insert_opt(&mut m, "title", title);
            insert_opt(&mut m, "subtitle", subtitle);
            insert_opt(&mut m, "studio", studio);
            m.insert("images".into(), serialize_images(images));
        }
        MediaMetadata::TvShow { series_title, episode_title, season, episode, images } => {
            m.insert("metadataType".into(), 2.into());
            insert_opt(&mut m, "seriesTitle", series_title);
            insert_opt(&mut m, "title", episode_title);
            insert_opt(&mut m, "season", season);
            insert_opt(&mut m, "episode", episode);
            m.insert("images".into(), serialize_images(images));
        }
        MediaMetadata::MusicTrack {
            title,
            artist,
            album_name,
            composer,
            track_number,
            disc_number,
            images,
        } => {
            m.insert("metadataType".into(), 3.into());
            insert_opt(&mut m, "title", title);
            insert_opt(&mut m, "artist", artist);
            insert_opt(&mut m, "albumName", album_name);
            insert_opt(&mut m, "composer", composer);
            insert_opt(&mut m, "trackNumber", track_number);
            insert_opt(&mut m, "discNumber", disc_number);
            m.insert("images".into(), serialize_images(images));
        }
        MediaMetadata::Photo {
            title,
            artist,
            location,
            latitude,
            longitude,
            width,
            height,
            images,
        } => {
            m.insert("metadataType".into(), 4.into());
            insert_opt(&mut m, "title", title);
            insert_opt(&mut m, "artist", artist);
            insert_opt(&mut m, "location", location);
            insert_opt(&mut m, "latitude", latitude);
            insert_opt(&mut m, "longitude", longitude);
            insert_opt(&mut m, "width", width);
            insert_opt(&mut m, "height", height);
            m.insert("images".into(), serialize_images(images));
        }
        MediaMetadata::AudiobookChapter {
            book_title,
            chapter_title,
            chapter_number,
            subtitle,
            images,
        } => {
            m.insert("metadataType".into(), 5.into());
            insert_opt(&mut m, "bookTitle", book_title);
            insert_opt(&mut m, "chapterTitle", chapter_title);
            insert_opt(&mut m, "chapterNumber", chapter_number);
            insert_opt(&mut m, "subtitle", subtitle);
            m.insert("images".into(), serialize_images(images));
        }
    }

    serde_json::Value::Object(m)
}

fn serialize_images(images: &[Image]) -> serde_json::Value {
    serde_json::json!(
        images
            .iter()
            .map(|i| {
                let mut obj = serde_json::json!({"url": i.url});
                if let Some(w) = i.width {
                    obj["width"] = serde_json::json!(w);
                }
                if let Some(h) = i.height {
                    obj["height"] = serde_json::json!(h);
                }
                obj
            })
            .collect::<Vec<_>>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_payload(msg: &CastMessage) -> serde_json::Value {
        serde_json::from_str(msg.payload_utf8.as_deref().unwrap()).unwrap()
    }

    #[test]
    fn test_load_with_metadata() {
        let media = MediaInfo::new("url", "video/mp4").metadata(MediaMetadata::Movie {
            title: Some("Test Movie".into()),
            subtitle: None,
            studio: Some("Studio X".into()),
            images: vec![Image { url: "http://img.jpg".into(), width: Some(800), height: None }],
        });
        let msg = load(1, "t", "s", &media, true, 0.0, None);
        let p = parse_payload(&msg);
        assert_eq!(p["media"]["metadata"]["metadataType"], 1);
        assert_eq!(p["media"]["metadata"]["title"], "Test Movie");
        assert_eq!(p["media"]["metadata"]["studio"], "Studio X");
        assert_eq!(p["media"]["metadata"]["images"][0]["url"], "http://img.jpg");
        assert_eq!(p["media"]["metadata"]["images"][0]["width"], 800);
    }

    #[test]
    fn test_load_with_duration() {
        let media = MediaInfo::new("url", "video/mp4").duration(120.5);
        let msg = load(1, "t", "s", &media, false, 10.0, None);
        let p = parse_payload(&msg);
        assert_eq!(p["media"]["duration"], 120.5);
        assert_eq!(p["autoplay"], false);
        assert_eq!(p["currentTime"], 10.0);
    }

    #[test]
    fn test_load_live_stream_type() {
        let media = MediaInfo::live("url", "application/x-mpegURL");
        let msg = load(1, "t", "s", &media, true, 0.0, None);
        let p = parse_payload(&msg);
        assert_eq!(p["media"]["streamType"], "LIVE");
    }

    #[test]
    fn test_load_with_custom_data() {
        let media = MediaInfo::new("url", "video/mp4");
        let custom = serde_json::json!({
            "positionOffset": 300.0,
            "realDuration": 7200.0,
            "isLive": false
        });
        let msg = load(1, "t", "s", &media, true, 0.0, Some(&custom));
        let p = parse_payload(&msg);
        assert_eq!(p["customData"]["positionOffset"], 300.0);
        assert_eq!(p["customData"]["realDuration"], 7200.0);
        assert_eq!(p["customData"]["isLive"], false);
    }

    #[test]
    fn test_load_without_custom_data() {
        let media = MediaInfo::new("url", "video/mp4");
        let msg = load(1, "t", "s", &media, true, 0.0, None);
        let p = parse_payload(&msg);
        assert_eq!(p["customData"], serde_json::json!({}));
    }

    #[test]
    fn test_queue_load_includes_metadata_and_duration() {
        let items = vec![QueueItem {
            media: MediaInfo::new("url1", "video/mp4").duration(300.0).metadata(
                MediaMetadata::Movie {
                    title: Some("Movie 1".into()),
                    subtitle: None,
                    studio: None,
                    images: vec![],
                },
            ),
            autoplay: true,
            start_time: 0.0,
        }];
        let msg = queue_load(1, "t", "s", &items, 0, RepeatMode::RepeatOff);
        let p = parse_payload(&msg);
        assert_eq!(p["type"], "QUEUE_LOAD");
        assert_eq!(p["repeatMode"], "REPEAT_OFF");
        assert_eq!(p["startIndex"], 0);
        // Verify metadata and duration are included
        let item = &p["items"][0];
        assert_eq!(item["media"]["duration"], 300.0);
        assert_eq!(item["media"]["metadata"]["metadataType"], 1);
        assert_eq!(item["media"]["metadata"]["title"], "Movie 1");
    }

    #[test]
    fn test_queue_load_repeat_modes() {
        let items = vec![];
        for (mode, expected) in [
            (RepeatMode::RepeatOff, "REPEAT_OFF"),
            (RepeatMode::RepeatAll, "REPEAT_ALL"),
            (RepeatMode::RepeatSingle, "REPEAT_SINGLE"),
            (RepeatMode::RepeatAllAndShuffle, "REPEAT_ALL_AND_SHUFFLE"),
        ] {
            let msg = queue_load(1, "t", "s", &items, 0, mode);
            let p = parse_payload(&msg);
            assert_eq!(p["repeatMode"], expected);
        }
    }

    #[test]
    fn test_queue_insert_with_insert_before() {
        let items = vec![QueueItem {
            media: MediaInfo::new("url", "video/mp4"),
            autoplay: true,
            start_time: 0.0,
        }];
        let msg = queue_insert(1, "t", 42, &items, Some(3));
        let p = parse_payload(&msg);
        assert_eq!(p["type"], "QUEUE_INSERT");
        assert_eq!(p["mediaSessionId"], 42);
        assert_eq!(p["insertBefore"], 3);
    }

    #[test]
    fn test_queue_insert_without_insert_before() {
        let items = vec![];
        let msg = queue_insert(1, "t", 42, &items, None);
        let p = parse_payload(&msg);
        assert!(p.get("insertBefore").is_none());
    }

    #[test]
    fn test_serialize_metadata_all_types() {
        // Generic (type 0)
        let meta =
            MediaMetadata::Generic { title: Some("Title".into()), subtitle: None, images: vec![] };
        let json = serialize_metadata(&meta);
        assert_eq!(json["metadataType"], 0);

        // TvShow (type 2)
        let meta = MediaMetadata::TvShow {
            series_title: Some("Series".into()),
            episode_title: Some("Ep".into()),
            season: Some(2),
            episode: Some(5),
            images: vec![],
        };
        let json = serialize_metadata(&meta);
        assert_eq!(json["metadataType"], 2);
        assert_eq!(json["seriesTitle"], "Series");
        assert_eq!(json["season"], 2);

        // MusicTrack (type 3)
        let meta = MediaMetadata::MusicTrack {
            title: Some("Song".into()),
            artist: Some("Artist".into()),
            album_name: Some("Album".into()),
            composer: None,
            track_number: Some(7),
            disc_number: Some(1),
            images: vec![],
        };
        let json = serialize_metadata(&meta);
        assert_eq!(json["metadataType"], 3);
        assert_eq!(json["trackNumber"], 7);

        // Photo (type 4)
        let meta = MediaMetadata::Photo {
            title: None,
            artist: None,
            location: Some("NYC".into()),
            latitude: Some(40.7),
            longitude: Some(-74.0),
            width: Some(1920),
            height: Some(1080),
            images: vec![],
        };
        let json = serialize_metadata(&meta);
        assert_eq!(json["metadataType"], 4);
        assert_eq!(json["latitude"], 40.7);

        // AudiobookChapter (type 5)
        let meta = MediaMetadata::AudiobookChapter {
            book_title: Some("Book".into()),
            chapter_title: Some("Ch 1".into()),
            chapter_number: Some(1),
            subtitle: None,
            images: vec![],
        };
        let json = serialize_metadata(&meta);
        assert_eq!(json["metadataType"], 5);
        assert_eq!(json["bookTitle"], "Book");
    }

    #[test]
    fn test_serialize_metadata_omits_none_fields() {
        // None fields should be absent, not null
        let meta = MediaMetadata::Movie {
            title: Some("Movie".into()),
            subtitle: None,
            studio: None,
            images: vec![],
        };
        let json = serialize_metadata(&meta);
        let obj = json.as_object().unwrap();
        assert_eq!(obj["title"], "Movie");
        assert!(!obj.contains_key("subtitle"), "None subtitle should be omitted");
        assert!(!obj.contains_key("studio"), "None studio should be omitted");
        assert!(obj.contains_key("metadataType"));
        assert!(obj.contains_key("images"));
    }

    #[test]
    fn test_serialize_images_with_dimensions() {
        let images = vec![
            Image { url: "http://a.jpg".into(), width: Some(100), height: Some(200) },
            Image { url: "http://b.jpg".into(), width: None, height: None },
        ];
        let json = serialize_images(&images);
        let arr = json.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["width"], 100);
        assert_eq!(arr[0]["height"], 200);
        assert!(arr[1].get("width").is_none());
    }

    #[test]
    fn test_serialize_images_empty() {
        let json = serialize_images(&[]);
        assert_eq!(json.as_array().unwrap().len(), 0);
    }
}

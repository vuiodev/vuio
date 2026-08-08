//! Message router — dispatches inbound Cast messages to the right handler.

use tokio::sync::mpsc;

use crate::channel::ns;
use crate::client::heartbeat;
use crate::client::request_tracker::RequestTracker;
use crate::event::CastEvent;

/// Known media message types for structured matching.
enum MediaMessageType {
    Status,
    LoadFailed,
    LoadCancelled,
    InvalidRequest,
    Unknown,
}

fn classify_media_message(msg_type: &str) -> MediaMessageType {
    match msg_type {
        ns::MSG_MEDIA_STATUS => MediaMessageType::Status,
        ns::MSG_LOAD_FAILED => MediaMessageType::LoadFailed,
        ns::MSG_LOAD_CANCELLED => MediaMessageType::LoadCancelled,
        ns::MSG_INVALID_REQUEST => MediaMessageType::InvalidRequest,
        _ => MediaMessageType::Unknown,
    }
}
use crate::proto::CastMessage;
use crate::state::StateHolder;
use crate::types::*;

/// Route an inbound message to the appropriate handler.
pub(crate) async fn route(
    msg: &CastMessage,
    request_tracker: &RequestTracker,
    event_tx: &mpsc::Sender<CastEvent>,
    state: &StateHolder,
    write_tx: &mpsc::Sender<CastMessage>,
) {
    let namespace = &msg.namespace;
    let payload_str = msg.payload_utf8.as_deref().unwrap_or("");

    match namespace.as_str() {
        x if x == ns::NS_HEARTBEAT => {
            if heartbeat::is_ping(msg) {
                tracing::trace!("received PING, sending PONG");
                let _ = write_tx.send(heartbeat::pong()).await;
            }
        }

        x if x == ns::NS_CONNECTION => {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(payload_str) {
                if json.get("type").and_then(|t| t.as_str()) == Some("CLOSE") {
                    // CLOSE is a virtual-channel close (e.g. app stopped), NOT a full
                    // TCP/TLS disconnection. Emit as a raw event so consumers can
                    // decide how to handle it. Do NOT emit Disconnected here — that
                    // should only come from actual I/O failures in the reader task.
                    let source = &msg.source_id;
                    let dest = &msg.destination_id;
                    tracing::info!("received CLOSE from {source} to {dest}");
                    let _ = event_tx.try_send(CastEvent::RawMessage {
                        namespace: ns::NS_CONNECTION.to_string(),
                        source: source.clone(),
                        destination: dest.clone(),
                        payload: payload_str.to_string(),
                    });
                }
            }
        }

        x if x == ns::NS_RECEIVER => {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(payload_str) {
                // Check for request-response correlation
                if let Some(request_id) = json.get("requestId").and_then(|r| r.as_u64()) {
                    if request_id > 0 {
                        request_tracker.resolve(request_id as u32, json.clone()).await;
                    }
                }

                let msg_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if msg_type == ns::MSG_RECEIVER_STATUS {
                    if let Some(status) = parse_receiver_status(&json) {
                        let _ = state.receiver_tx.send(Some(status.clone()));
                        let _ = event_tx.try_send(CastEvent::ReceiverStatusChanged(status));
                    }
                }
            }
        }

        x if x == ns::NS_MEDIA => {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(payload_str) {
                let msg_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");
                let classified = classify_media_message(msg_type);

                // Only resolve request correlation for MEDIA_STATUS.
                // Error types (LOAD_FAILED, etc.) are resolved in their own
                // match arms to avoid consuming the oneshot before the specific handler.
                if matches!(classified, MediaMessageType::Status) {
                    if let Some(request_id) = json.get("requestId").and_then(|r| r.as_u64()) {
                        if request_id > 0 {
                            request_tracker.resolve(request_id as u32, json.clone()).await;
                        }
                    }
                }

                match classified {
                    MediaMessageType::Status => {
                        if let Some(status) = parse_media_status(&json) {
                            // Update tracked media session ID from broadcasts
                            if status.media_session_id > 0 {
                                state.set_media_session_id(status.media_session_id);
                            }

                            // Only emit session-ended for real sessions (not session 0)
                            if status.player_state == PlayerState::Idle
                                && status.media_session_id > 0
                            {
                                if let Some(reason) = &status.idle_reason {
                                    let _ = event_tx.try_send(CastEvent::MediaSessionEnded {
                                        media_session_id: status.media_session_id,
                                        idle_reason: *reason,
                                    });
                                }
                            }

                            let _ = state.media_tx.send(Some(status.clone()));
                            let _ = event_tx.try_send(CastEvent::MediaStatusChanged(status));
                        }
                    }
                    MediaMessageType::LoadFailed => {
                        let item_id = json.get("itemId").and_then(|v| v.as_i64()).unwrap_or(0);
                        tracing::debug!(
                            "LOAD_FAILED for item {item_id}, requestId={}",
                            json.get("requestId").and_then(|r| r.as_u64()).unwrap_or(0)
                        );
                        // Resolve the pending request so load_media() returns an error
                        if let Some(request_id) = json.get("requestId").and_then(|r| r.as_u64()) {
                            if request_id > 0 {
                                request_tracker.resolve(request_id as u32, json.clone()).await;
                            }
                        }
                    }
                    MediaMessageType::LoadCancelled => {
                        tracing::debug!("LOAD_CANCELLED");
                        if let Some(request_id) = json.get("requestId").and_then(|r| r.as_u64()) {
                            if request_id > 0 {
                                request_tracker.resolve(request_id as u32, json.clone()).await;
                            }
                        }
                    }
                    MediaMessageType::InvalidRequest => {
                        let reason =
                            json.get("reason").and_then(|r| r.as_str()).unwrap_or("unknown");
                        tracing::debug!("invalid request: {reason}");
                        if let Some(request_id) = json.get("requestId").and_then(|r| r.as_u64()) {
                            if request_id > 0 {
                                request_tracker.resolve(request_id as u32, json.clone()).await;
                            }
                        }
                    }
                    MediaMessageType::Unknown => {}
                }
            }
        }

        _ => {
            // Attempt request-response correlation for custom namespaces
            // (send_raw() injects requestId into the payload)
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(payload_str) {
                if let Some(request_id) = json.get("requestId").and_then(|r| r.as_u64()) {
                    if request_id > 0 {
                        request_tracker.resolve(request_id as u32, json).await;
                        return; // resolved — don't also emit as raw event
                    }
                }
            }

            let _ = event_tx.try_send(CastEvent::RawMessage {
                namespace: namespace.clone(),
                source: msg.source_id.clone(),
                destination: msg.destination_id.clone(),
                payload: payload_str.to_string(),
            });
        }
    }
}

/// Parse a RECEIVER_STATUS JSON — public for CastClient.
pub fn parse_receiver_status_from_json(json: &serde_json::Value) -> Option<ReceiverStatus> {
    parse_receiver_status(json)
}

/// Parse a MEDIA_STATUS JSON — public for CastClient.
pub fn parse_media_status_from_json(json: &serde_json::Value) -> Option<MediaStatus> {
    parse_media_status(json)
}

fn parse_receiver_status(json: &serde_json::Value) -> Option<ReceiverStatus> {
    let status = json.get("status")?;
    let volume_obj = status.get("volume")?;

    let applications = status
        .get("applications")
        .and_then(|a| a.as_array())
        .map(|apps| {
            apps.iter()
                .filter_map(|app| {
                    Some(Application {
                        app_id: app.get("appId")?.as_str()?.to_string(),
                        display_name: app
                            .get("displayName")
                            .and_then(|d| d.as_str())
                            .unwrap_or("")
                            .to_string(),
                        session_id: app.get("sessionId")?.as_str()?.to_string(),
                        transport_id: app.get("transportId")?.as_str()?.to_string(),
                        namespaces: app
                            .get("namespaces")
                            .and_then(|n| n.as_array())
                            .map(|ns| {
                                ns.iter()
                                    .filter_map(|n| {
                                        n.get("name").and_then(|n| n.as_str()).map(String::from)
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                        status_text: app
                            .get("statusText")
                            .and_then(|s| s.as_str())
                            .unwrap_or("")
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Some(ReceiverStatus {
        volume: Volume {
            level: volume_obj.get("level").and_then(|l| l.as_f64()).unwrap_or(1.0) as f32,
            muted: volume_obj.get("muted").and_then(|m| m.as_bool()).unwrap_or(false),
        },
        applications,
        is_active_input: status.get("isActiveInput").and_then(|v| v.as_bool()).unwrap_or(false),
        is_stand_by: status.get("isStandBy").and_then(|v| v.as_bool()).unwrap_or(false),
    })
}

fn parse_media_status(json: &serde_json::Value) -> Option<MediaStatus> {
    let entries = json.get("status")?.as_array()?;
    let entry = entries.first()?;

    let player_state = match entry.get("playerState")?.as_str()? {
        "IDLE" => PlayerState::Idle,
        "PLAYING" => PlayerState::Playing,
        "PAUSED" => PlayerState::Paused,
        "BUFFERING" => PlayerState::Buffering,
        _ => return None,
    };

    let idle_reason = entry.get("idleReason").and_then(|r| r.as_str()).and_then(|r| match r {
        "CANCELLED" => Some(IdleReason::Cancelled),
        "INTERRUPTED" => Some(IdleReason::Interrupted),
        "FINISHED" => Some(IdleReason::Finished),
        "ERROR" => Some(IdleReason::Error),
        _ => None,
    });

    let volume_obj = entry.get("volume");

    Some(MediaStatus {
        media_session_id: entry.get("mediaSessionId")?.as_i64()? as i32,
        player_state,
        idle_reason,
        current_time: entry.get("currentTime").and_then(|t| t.as_f64()).unwrap_or(0.0),
        duration: entry
            .get("media")
            .and_then(|m| m.get("duration"))
            .and_then(|d| d.as_f64())
            .filter(|d| *d > 0.0),
        volume: Volume {
            level: volume_obj.and_then(|v| v.get("level")).and_then(|l| l.as_f64()).unwrap_or(1.0)
                as f32,
            muted: volume_obj
                .and_then(|v| v.get("muted"))
                .and_then(|m| m.as_bool())
                .unwrap_or(false),
        },
        media: entry.get("media").and_then(|m| {
            Some(MediaInfo {
                content_id: m.get("contentId")?.as_str()?.to_string(),
                content_type: m
                    .get("contentType")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string(),
                stream_type: match m.get("streamType").and_then(|s| s.as_str()).unwrap_or("") {
                    "BUFFERED" => StreamType::Buffered,
                    "LIVE" => StreamType::Live,
                    _ => StreamType::None,
                },
                duration: m.get("duration").and_then(|d| d.as_f64()).filter(|d| *d > 0.0),
                metadata: m.get("metadata").and_then(parse_metadata),
            })
        }),
    })
}

fn parse_metadata(meta: &serde_json::Value) -> Option<MediaMetadata> {
    let metadata_type = meta.get("metadataType").and_then(|t| t.as_u64()).unwrap_or(0);
    let images = parse_images(meta);

    match metadata_type {
        ns::METADATA_GENERIC => Some(MediaMetadata::Generic {
            title: meta.get("title").and_then(|t| t.as_str()).map(String::from),
            subtitle: meta.get("subtitle").and_then(|s| s.as_str()).map(String::from),
            images,
        }),
        ns::METADATA_MOVIE => Some(MediaMetadata::Movie {
            title: meta.get("title").and_then(|t| t.as_str()).map(String::from),
            subtitle: meta.get("subtitle").and_then(|s| s.as_str()).map(String::from),
            studio: meta.get("studio").and_then(|s| s.as_str()).map(String::from),
            images,
        }),
        ns::METADATA_TV_SHOW => Some(MediaMetadata::TvShow {
            series_title: meta.get("seriesTitle").and_then(|t| t.as_str()).map(String::from),
            episode_title: meta
                .get("episodeTitle")
                .or_else(|| meta.get("title"))
                .and_then(|t| t.as_str())
                .map(String::from),
            season: meta.get("season").and_then(|s| s.as_u64()).map(|s| s as u32),
            episode: meta.get("episode").and_then(|e| e.as_u64()).map(|e| e as u32),
            images,
        }),
        ns::METADATA_MUSIC_TRACK => Some(MediaMetadata::MusicTrack {
            title: meta.get("title").and_then(|t| t.as_str()).map(String::from),
            artist: meta
                .get("artist")
                .or_else(|| meta.get("albumArtist"))
                .and_then(|a| a.as_str())
                .map(String::from),
            album_name: meta.get("albumName").and_then(|a| a.as_str()).map(String::from),
            composer: meta.get("composer").and_then(|c| c.as_str()).map(String::from),
            track_number: meta.get("trackNumber").and_then(|t| t.as_u64()).map(|t| t as u32),
            disc_number: meta.get("discNumber").and_then(|d| d.as_u64()).map(|d| d as u32),
            images,
        }),
        ns::METADATA_PHOTO => Some(MediaMetadata::Photo {
            title: meta.get("title").and_then(|t| t.as_str()).map(String::from),
            artist: meta.get("artist").and_then(|a| a.as_str()).map(String::from),
            location: meta.get("location").and_then(|l| l.as_str()).map(String::from),
            latitude: meta.get("latitude").and_then(|l| l.as_f64()),
            longitude: meta.get("longitude").and_then(|l| l.as_f64()),
            width: meta.get("width").and_then(|w| w.as_u64()).map(|w| w as u32),
            height: meta.get("height").and_then(|h| h.as_u64()).map(|h| h as u32),
            images,
        }),
        ns::METADATA_AUDIOBOOK_CHAPTER => Some(MediaMetadata::AudiobookChapter {
            book_title: meta.get("bookTitle").and_then(|t| t.as_str()).map(String::from),
            chapter_title: meta.get("chapterTitle").and_then(|t| t.as_str()).map(String::from),
            chapter_number: meta.get("chapterNumber").and_then(|n| n.as_u64()).map(|n| n as u32),
            subtitle: meta.get("subtitle").and_then(|s| s.as_str()).map(String::from),
            images,
        }),
        _ => Some(MediaMetadata::Generic {
            title: meta.get("title").and_then(|t| t.as_str()).map(String::from),
            subtitle: meta.get("subtitle").and_then(|s| s.as_str()).map(String::from),
            images,
        }),
    }
}

fn parse_images(meta: &serde_json::Value) -> Vec<Image> {
    meta.get("images")
        .and_then(|i| i.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|img| {
                    Some(Image {
                        url: img.get("url")?.as_str()?.to_string(),
                        width: img.get("width").and_then(|w| w.as_u64()).map(|w| w as u32),
                        height: img.get("height").and_then(|h| h.as_u64()).map(|h| h as u32),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::framing::build_message;
    use crate::client::request_tracker::RequestTracker;
    use crate::state;
    use std::sync::Arc;
    use std::time::Duration;

    async fn setup() -> (
        Arc<RequestTracker>,
        mpsc::Sender<CastEvent>,
        mpsc::Receiver<CastEvent>,
        Arc<StateHolder>,
        mpsc::Sender<CastMessage>,
        mpsc::Receiver<CastMessage>,
    ) {
        let tracker = Arc::new(RequestTracker::new(Duration::from_secs(5)));
        let (event_tx, event_rx) = mpsc::channel(64);
        let (state_holder, _watchers) = state::new_state();
        let (write_tx, write_rx) = mpsc::channel(64);
        (tracker, event_tx, event_rx, state_holder, write_tx, write_rx)
    }

    #[tokio::test]
    async fn test_ping_auto_reply() {
        let (tracker, event_tx, _event_rx, state, write_tx, mut write_rx) = setup().await;
        let msg = build_message(ns::NS_HEARTBEAT, "receiver-0", "sender-0", r#"{"type":"PING"}"#);
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        // Should send PONG
        let reply = write_rx.try_recv().unwrap();
        assert_eq!(reply.namespace, ns::NS_HEARTBEAT);
        let payload: serde_json::Value =
            serde_json::from_str(reply.payload_utf8.as_deref().unwrap()).unwrap();
        assert_eq!(payload["type"], "PONG");
    }

    #[tokio::test]
    async fn test_pong_does_not_reply() {
        let (tracker, event_tx, _event_rx, state, write_tx, mut write_rx) = setup().await;
        let msg = build_message(ns::NS_HEARTBEAT, "receiver-0", "sender-0", r#"{"type":"PONG"}"#);
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        // Should NOT send anything
        assert!(write_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_close_emits_raw_message() {
        let (tracker, event_tx, mut event_rx, state, write_tx, _write_rx) = setup().await;
        let msg = build_message(ns::NS_CONNECTION, "web-123", "sender-0", r#"{"type":"CLOSE"}"#);
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        let event = event_rx.try_recv().unwrap();
        match event {
            CastEvent::RawMessage { namespace, source, .. } => {
                assert_eq!(namespace, ns::NS_CONNECTION);
                assert_eq!(source, "web-123");
            }
            other => panic!("expected RawMessage, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_close_does_not_emit_disconnected() {
        let (tracker, event_tx, mut event_rx, state, write_tx, _write_rx) = setup().await;
        let msg = build_message(ns::NS_CONNECTION, "web-123", "sender-0", r#"{"type":"CLOSE"}"#);
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        let event = event_rx.try_recv().unwrap();
        assert!(!event.is_disconnected());
    }

    #[tokio::test]
    async fn test_receiver_status_resolves_request() {
        let (tracker, event_tx, _event_rx, state, write_tx, _write_rx) = setup().await;
        let (id, rx) = tracker.register().await;

        let payload = serde_json::json!({
            "type": "RECEIVER_STATUS",
            "requestId": id,
            "status": {
                "volume": {"level": 0.5, "muted": false},
                "applications": []
            }
        });
        let msg = build_message(ns::NS_RECEIVER, "receiver-0", "sender-0", &payload.to_string());
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        let value = rx.await.unwrap();
        assert_eq!(value["requestId"], id);
    }

    #[tokio::test]
    async fn test_receiver_status_emits_event() {
        let (tracker, event_tx, mut event_rx, state, write_tx, _write_rx) = setup().await;

        let payload = serde_json::json!({
            "type": "RECEIVER_STATUS",
            "requestId": 0,
            "status": {
                "volume": {"level": 0.8, "muted": true},
                "applications": []
            }
        });
        let msg = build_message(ns::NS_RECEIVER, "receiver-0", "sender-0", &payload.to_string());
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        let event = event_rx.try_recv().unwrap();
        match event {
            CastEvent::ReceiverStatusChanged(status) => {
                assert_eq!(status.volume.level, 0.8);
                assert!(status.volume.muted);
            }
            other => panic!("expected ReceiverStatusChanged, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_media_status_resolves_request() {
        let (tracker, event_tx, _event_rx, state, write_tx, _write_rx) = setup().await;
        let (id, rx) = tracker.register().await;

        let payload = serde_json::json!({
            "type": "MEDIA_STATUS",
            "requestId": id,
            "status": [{
                "mediaSessionId": 42,
                "playerState": "PLAYING",
                "currentTime": 10.5,
                "volume": {"level": 1.0, "muted": false}
            }]
        });
        let msg = build_message(ns::NS_MEDIA, "web-5", "sender-0", &payload.to_string());
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        let value = rx.await.unwrap();
        assert_eq!(value["status"][0]["mediaSessionId"], 42);
    }

    #[tokio::test]
    async fn test_media_status_updates_session_id() {
        let (tracker, event_tx, _event_rx, state, write_tx, _write_rx) = setup().await;

        let payload = serde_json::json!({
            "type": "MEDIA_STATUS",
            "requestId": 0,
            "status": [{
                "mediaSessionId": 77,
                "playerState": "PLAYING",
                "currentTime": 0.0,
                "volume": {"level": 1.0, "muted": false}
            }]
        });
        let msg = build_message(ns::NS_MEDIA, "web-5", "sender-0", &payload.to_string());
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        assert_eq!(state.media_session_id.load(std::sync::atomic::Ordering::Relaxed), 77);
    }

    #[tokio::test]
    async fn test_media_session_ended_event() {
        let (tracker, event_tx, mut event_rx, state, write_tx, _write_rx) = setup().await;

        let payload = serde_json::json!({
            "type": "MEDIA_STATUS",
            "requestId": 0,
            "status": [{
                "mediaSessionId": 5,
                "playerState": "IDLE",
                "idleReason": "FINISHED",
                "currentTime": 120.0,
                "volume": {"level": 1.0, "muted": false}
            }]
        });
        let msg = build_message(ns::NS_MEDIA, "web-5", "sender-0", &payload.to_string());
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        // First event: MediaSessionEnded
        let event = event_rx.try_recv().unwrap();
        match event {
            CastEvent::MediaSessionEnded { media_session_id, idle_reason } => {
                assert_eq!(media_session_id, 5);
                assert_eq!(idle_reason, IdleReason::Finished);
            }
            other => panic!("expected MediaSessionEnded, got {other:?}"),
        }

        // Second event: MediaStatusChanged
        let event2 = event_rx.try_recv().unwrap();
        assert!(event2.is_media_status());
    }

    #[tokio::test]
    async fn test_idle_session_zero_no_session_ended() {
        let (tracker, event_tx, mut event_rx, state, write_tx, _write_rx) = setup().await;

        let payload = serde_json::json!({
            "type": "MEDIA_STATUS",
            "requestId": 0,
            "status": [{
                "mediaSessionId": 0,
                "playerState": "IDLE",
                "idleReason": "FINISHED",
                "currentTime": 0.0,
                "volume": {"level": 1.0, "muted": false}
            }]
        });
        let msg = build_message(ns::NS_MEDIA, "web-5", "sender-0", &payload.to_string());
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        // Should only get MediaStatusChanged, NOT MediaSessionEnded (session 0 is not real)
        let event = event_rx.try_recv().unwrap();
        assert!(event.is_media_status());
        // No more events
        assert!(event_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_load_failed_resolves_request() {
        let (tracker, event_tx, _event_rx, state, write_tx, _write_rx) = setup().await;
        let (id, rx) = tracker.register().await;

        let payload = serde_json::json!({
            "type": "LOAD_FAILED",
            "requestId": id,
            "itemId": 1
        });
        let msg = build_message(ns::NS_MEDIA, "web-5", "sender-0", &payload.to_string());
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        let value = rx.await.unwrap();
        assert_eq!(value["type"], "LOAD_FAILED");
    }

    #[tokio::test]
    async fn test_load_cancelled_resolves_request() {
        let (tracker, event_tx, _event_rx, state, write_tx, _write_rx) = setup().await;
        let (id, rx) = tracker.register().await;

        let payload = serde_json::json!({
            "type": "LOAD_CANCELLED",
            "requestId": id,
        });
        let msg = build_message(ns::NS_MEDIA, "web-5", "sender-0", &payload.to_string());
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        let value = rx.await.unwrap();
        assert_eq!(value["type"], "LOAD_CANCELLED");
    }

    #[tokio::test]
    async fn test_invalid_request_resolves_request() {
        let (tracker, event_tx, _event_rx, state, write_tx, _write_rx) = setup().await;
        let (id, rx) = tracker.register().await;

        let payload = serde_json::json!({
            "type": "INVALID_REQUEST",
            "requestId": id,
            "reason": "bad stuff"
        });
        let msg = build_message(ns::NS_MEDIA, "web-5", "sender-0", &payload.to_string());
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        let value = rx.await.unwrap();
        assert_eq!(value["type"], "INVALID_REQUEST");
    }

    #[tokio::test]
    async fn test_unknown_media_message_ignored() {
        let (tracker, event_tx, mut event_rx, state, write_tx, _write_rx) = setup().await;

        let payload = serde_json::json!({
            "type": "SOME_UNKNOWN_THING",
            "requestId": 0,
        });
        let msg = build_message(ns::NS_MEDIA, "web-5", "sender-0", &payload.to_string());
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        // No events emitted for unknown media message types
        assert!(event_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_custom_namespace_with_request_id_resolves() {
        let (tracker, event_tx, mut event_rx, state, write_tx, _write_rx) = setup().await;
        let (id, rx) = tracker.register().await;

        let payload = serde_json::json!({
            "requestId": id,
            "data": "hello"
        });
        let msg = build_message(
            "urn:x-cast:com.example.custom",
            "web-5",
            "sender-0",
            &payload.to_string(),
        );
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        let value = rx.await.unwrap();
        assert_eq!(value["data"], "hello");
        // Should NOT also emit RawMessage when resolved
        assert!(event_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_custom_namespace_no_request_id_emits_raw() {
        let (tracker, event_tx, mut event_rx, state, write_tx, _write_rx) = setup().await;

        let payload = r#"{"data":"broadcast"}"#;
        let msg = build_message("urn:x-cast:com.example.custom", "web-5", "sender-0", payload);
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        let event = event_rx.try_recv().unwrap();
        match event {
            CastEvent::RawMessage { namespace, source, payload: p, .. } => {
                assert_eq!(namespace, "urn:x-cast:com.example.custom");
                assert_eq!(source, "web-5");
                assert!(p.contains("broadcast"));
            }
            other => panic!("expected RawMessage, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_invalid_json_payload_ignored() {
        let (tracker, event_tx, mut event_rx, state, write_tx, mut write_rx) = setup().await;

        // Invalid JSON on receiver namespace — should not panic
        let msg = build_message(ns::NS_RECEIVER, "receiver-0", "sender-0", "not valid json");
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;

        // No events, no replies
        assert!(event_rx.try_recv().is_err());
        assert!(write_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_receiver_status_request_id_zero_not_resolved() {
        let (tracker, event_tx, _event_rx, state, write_tx, _write_rx) = setup().await;
        // Register request id 1 to verify it stays pending
        let (_id, _rx) = tracker.register().await;

        let payload = serde_json::json!({
            "type": "RECEIVER_STATUS",
            "requestId": 0,
            "status": {
                "volume": {"level": 1.0, "muted": false},
                "applications": []
            }
        });
        let msg = build_message(ns::NS_RECEIVER, "receiver-0", "sender-0", &payload.to_string());
        route(&msg, &tracker, &event_tx, &state, &write_tx).await;
        // requestId 0 should not consume the pending entry for id 1
        assert!(tracker.resolve(1, serde_json::json!({})).await);
    }

    // ── Parsing tests ────────────────────────────────────────

    #[test]
    fn test_parse_receiver_status_missing_status() {
        let json = serde_json::json!({"type": "RECEIVER_STATUS"});
        assert!(parse_receiver_status(&json).is_none());
    }

    #[test]
    fn test_parse_receiver_status_missing_volume() {
        let json = serde_json::json!({
            "status": {"applications": []}
        });
        assert!(parse_receiver_status(&json).is_none());
    }

    #[test]
    fn test_parse_media_status_empty_array() {
        let json = serde_json::json!({"status": []});
        assert!(parse_media_status(&json).is_none());
    }

    #[test]
    fn test_parse_media_status_missing_player_state() {
        let json = serde_json::json!({
            "status": [{"mediaSessionId": 1, "currentTime": 0.0}]
        });
        assert!(parse_media_status(&json).is_none());
    }

    #[test]
    fn test_classify_media_message_all_types() {
        assert!(matches!(classify_media_message("MEDIA_STATUS"), MediaMessageType::Status));
        assert!(matches!(classify_media_message("LOAD_FAILED"), MediaMessageType::LoadFailed));
        assert!(matches!(
            classify_media_message("LOAD_CANCELLED"),
            MediaMessageType::LoadCancelled
        ));
        assert!(matches!(
            classify_media_message("INVALID_REQUEST"),
            MediaMessageType::InvalidRequest
        ));
        assert!(matches!(classify_media_message("WHATEVER"), MediaMessageType::Unknown));
    }
}

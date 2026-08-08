//! Cast receiver channel — app launch, status, volume control.

use super::ns;
use crate::client::framing::build_message;
use crate::proto::CastMessage;

/// Build a GET_STATUS request.
pub fn get_status(request_id: u32) -> CastMessage {
    let payload = serde_json::json!({
        "type": ns::MSG_GET_STATUS,
        "requestId": request_id,
    });
    build_message(ns::NS_RECEIVER, ns::SENDER_ID, ns::RECEIVER_ID, &payload.to_string())
}

/// Build a LAUNCH request for an application.
pub fn launch_app(request_id: u32, app_id: &str) -> CastMessage {
    let payload = serde_json::json!({
        "type": ns::MSG_LAUNCH,
        "requestId": request_id,
        "appId": app_id,
    });
    build_message(ns::NS_RECEIVER, ns::SENDER_ID, ns::RECEIVER_ID, &payload.to_string())
}

/// Build a STOP request for an application.
pub fn stop_app(request_id: u32, session_id: &str) -> CastMessage {
    let payload = serde_json::json!({
        "type": ns::MSG_STOP,
        "requestId": request_id,
        "sessionId": session_id,
    });
    build_message(ns::NS_RECEIVER, ns::SENDER_ID, ns::RECEIVER_ID, &payload.to_string())
}

/// Build a SET_VOLUME request.
pub fn set_volume(request_id: u32, level: Option<f32>, muted: Option<bool>) -> CastMessage {
    let mut volume = serde_json::Map::new();
    if let Some(l) = level {
        volume.insert("level".into(), serde_json::json!(l));
    }
    if let Some(m) = muted {
        volume.insert("muted".into(), serde_json::json!(m));
    }
    let payload = serde_json::json!({
        "type": ns::MSG_SET_VOLUME,
        "requestId": request_id,
        "volume": volume,
    });
    build_message(ns::NS_RECEIVER, ns::SENDER_ID, ns::RECEIVER_ID, &payload.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_payload(msg: &CastMessage) -> serde_json::Value {
        serde_json::from_str(msg.payload_utf8.as_deref().unwrap()).unwrap()
    }

    #[test]
    fn test_set_volume_level_only() {
        let msg = set_volume(1, Some(0.5), None);
        let p = parse_payload(&msg);
        assert_eq!(p["type"], "SET_VOLUME");
        assert_eq!(p["volume"]["level"], 0.5);
        assert!(p["volume"].get("muted").is_none());
    }

    #[test]
    fn test_set_volume_muted_only() {
        let msg = set_volume(1, None, Some(true));
        let p = parse_payload(&msg);
        assert!(p["volume"].get("level").is_none());
        assert_eq!(p["volume"]["muted"], true);
    }

    #[test]
    fn test_set_volume_both() {
        let msg = set_volume(1, Some(0.3), Some(false));
        let p = parse_payload(&msg);
        assert_eq!(p["volume"]["level"], 0.30000001192092896_f64); // f32 → f64 precision
        assert_eq!(p["volume"]["muted"], false);
    }
}

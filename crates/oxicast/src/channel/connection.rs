//! Cast connection channel — virtual connection setup (CONNECT/CLOSE).

use super::ns;
use crate::client::framing::build_message;
use crate::proto::CastMessage;

/// Build a CONNECT message to a destination (receiver or app transport).
pub fn connect_msg(destination: &str) -> CastMessage {
    let payload = serde_json::json!({
        "type": ns::MSG_CONNECT,
        "userAgent": ns::USER_AGENT,
    });
    build_message(ns::NS_CONNECTION, ns::SENDER_ID, destination, &payload.to_string())
}

/// Build a CLOSE message to a destination.
pub fn close_msg(destination: &str) -> CastMessage {
    let payload = serde_json::json!({ "type": ns::MSG_CLOSE });
    build_message(ns::NS_CONNECTION, ns::SENDER_ID, destination, &payload.to_string())
}

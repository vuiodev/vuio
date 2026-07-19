//! Castv2 protobuf message definitions.
//!
//! The Cast channel protocol uses a simple protobuf message envelope
//! (`CastMessage`) that wraps JSON payloads for different namespaces.

/// Protocol version for Castv2 messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, prost::Enumeration)]
#[repr(i32)]
pub enum ProtocolVersion {
    Castv210 = 0,
}

/// Payload type for Castv2 messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, prost::Enumeration)]
#[repr(i32)]
pub enum PayloadType {
    String = 0,
    Binary = 1,
}

/// The Cast channel protocol buffer message.
///
/// Each message has a source/destination ID, a namespace that determines
/// the handler, and a payload (usually JSON-encoded UTF-8).
#[derive(Clone, PartialEq, prost::Message)]
pub struct CastMessage {
    /// Protocol version (always CASTV2_1_0).
    #[prost(enumeration = "ProtocolVersion", tag = "1")]
    pub protocol_version: i32,

    /// Sender identifier (usually "sender-0").
    #[prost(string, tag = "2")]
    pub source_id: String,

    /// Receiver identifier ("receiver-0" for the device, or a transport ID).
    #[prost(string, tag = "3")]
    pub destination_id: String,

    /// The namespace determines which handler processes this message.
    #[prost(string, tag = "4")]
    pub namespace: String,

    /// Whether the payload is a UTF-8 string or binary data.
    #[prost(enumeration = "PayloadType", tag = "5")]
    pub payload_type: i32,

    /// UTF-8 string payload (JSON for most namespaces).
    #[prost(string, optional, tag = "6")]
    pub payload_utf8: Option<String>,

    /// Binary payload (rarely used).
    #[prost(bytes = "vec", optional, tag = "7")]
    pub payload_binary: Option<Vec<u8>>,
}

/// Cast protocol namespaces.
pub mod namespace {
    /// Connection management (CONNECT/CLOSE).
    pub const CONNECTION: &str = "urn:x-cast:com.google.cast.tp.connection";
    /// Heartbeat (PING/PONG keepalive).
    pub const HEARTBEAT: &str = "urn:x-cast:com.google.cast.tp.heartbeat";
    /// Receiver control (LAUNCH/STOP apps, GET_STATUS).
    pub const RECEIVER: &str = "urn:x-cast:com.google.cast.receiver";
    /// Media control (LOAD/PLAY/PAUSE/STOP/SEEK).
    pub const MEDIA: &str = "urn:x-cast:com.google.cast.media";
}

/// Default Media Receiver app ID on Chromecast.
pub const DEFAULT_MEDIA_RECEIVER_APP_ID: &str = "CC1AD845";

/// Default sender ID.
pub const DEFAULT_SENDER_ID: &str = "sender-0";

/// Default receiver ID.
pub const DEFAULT_RECEIVER_ID: &str = "receiver-0";

impl CastMessage {
    /// Create a new string-payload message.
    pub fn new_string(
        source_id: impl Into<String>,
        destination_id: impl Into<String>,
        namespace: impl Into<String>,
        payload: impl Into<String>,
    ) -> Self {
        Self {
            protocol_version: ProtocolVersion::Castv210 as i32,
            source_id: source_id.into(),
            destination_id: destination_id.into(),
            namespace: namespace.into(),
            payload_type: PayloadType::String as i32,
            payload_utf8: Some(payload.into()),
            payload_binary: None,
        }
    }

    /// Create a CONNECT message for a virtual connection.
    pub fn connect(destination_id: &str) -> Self {
        Self::new_string(
            DEFAULT_SENDER_ID,
            destination_id,
            namespace::CONNECTION,
            r#"{"type":"CONNECT"}"#,
        )
    }

    /// Create a CLOSE message.
    pub fn close(destination_id: &str) -> Self {
        Self::new_string(
            DEFAULT_SENDER_ID,
            destination_id,
            namespace::CONNECTION,
            r#"{"type":"CLOSE"}"#,
        )
    }

    /// Create a PING message.
    pub fn ping() -> Self {
        Self::new_string(
            DEFAULT_SENDER_ID,
            DEFAULT_RECEIVER_ID,
            namespace::HEARTBEAT,
            r#"{"type":"PING"}"#,
        )
    }

    /// Create a PONG message.
    pub fn pong() -> Self {
        Self::new_string(
            DEFAULT_SENDER_ID,
            DEFAULT_RECEIVER_ID,
            namespace::HEARTBEAT,
            r#"{"type":"PONG"}"#,
        )
    }

    /// Create a GET_STATUS message for the receiver.
    pub fn get_receiver_status() -> Self {
        Self::new_string(
            DEFAULT_SENDER_ID,
            DEFAULT_RECEIVER_ID,
            namespace::RECEIVER,
            r#"{"type":"GET_STATUS","requestId":1}"#,
        )
    }

    /// Create a LAUNCH message for the Default Media Receiver.
    pub fn launch_default_media_receiver() -> Self {
        Self::new_string(
            DEFAULT_SENDER_ID,
            DEFAULT_RECEIVER_ID,
            namespace::RECEIVER,
            format!(
                r#"{{"type":"LAUNCH","appId":"{}","requestId":2}}"#,
                DEFAULT_MEDIA_RECEIVER_APP_ID,
            ),
        )
    }

    /// Create a media LOAD message.
    pub fn load_media(
        transport_id: &str,
        media_url: &str,
        mime_type: &str,
        title: &str,
        request_id: i32,
    ) -> Self {
        let payload = serde_json::json!({
            "type": "LOAD",
            "requestId": request_id,
            "media": {
                "contentId": media_url,
                "contentType": mime_type,
                "streamType": "BUFFERED",
                "metadata": {
                    "metadataType": 0,
                    "title": title
                }
            },
            "autoplay": true
        });
        Self::new_string(
            DEFAULT_SENDER_ID,
            transport_id,
            namespace::MEDIA,
            payload.to_string(),
        )
    }

    /// Create a media control message (PLAY, PAUSE, STOP).
    pub fn media_command(
        transport_id: &str,
        command: &str,
        media_session_id: i32,
        request_id: i32,
    ) -> Self {
        let payload = serde_json::json!({
            "type": command,
            "requestId": request_id,
            "mediaSessionId": media_session_id
        });
        Self::new_string(
            DEFAULT_SENDER_ID,
            transport_id,
            namespace::MEDIA,
            payload.to_string(),
        )
    }

    /// Create a SEEK message.
    pub fn seek(
        transport_id: &str,
        media_session_id: i32,
        position_secs: f64,
        request_id: i32,
    ) -> Self {
        let payload = serde_json::json!({
            "type": "SEEK",
            "requestId": request_id,
            "mediaSessionId": media_session_id,
            "currentTime": position_secs
        });
        Self::new_string(
            DEFAULT_SENDER_ID,
            transport_id,
            namespace::MEDIA,
            payload.to_string(),
        )
    }

    /// Create a SET_VOLUME message.
    pub fn set_volume(level: f64, request_id: i32) -> Self {
        let payload = serde_json::json!({
            "type": "SET_VOLUME",
            "requestId": request_id,
            "volume": {
                "level": level,
                "muted": false
            }
        });
        Self::new_string(
            DEFAULT_SENDER_ID,
            DEFAULT_RECEIVER_ID,
            namespace::RECEIVER,
            payload.to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_message_has_correct_namespace() {
        let msg = CastMessage::connect("receiver-0");
        assert_eq!(msg.namespace, namespace::CONNECTION);
        let payload = msg.payload_utf8.unwrap();
        assert!(payload.contains("CONNECT"));
    }

    #[test]
    fn ping_message_is_heartbeat_namespace() {
        let msg = CastMessage::ping();
        assert_eq!(msg.namespace, namespace::HEARTBEAT);
    }

    #[test]
    fn load_media_includes_content_id() {
        let msg = CastMessage::load_media(
            "transport-123",
            "http://192.168.1.2:8080/media/42",
            "video/mp4",
            "Test Movie",
            3,
        );
        let payload = msg.payload_utf8.unwrap();
        assert!(payload.contains("http://192.168.1.2:8080/media/42"));
        assert!(payload.contains("video/mp4"));
        assert!(payload.contains("Test Movie"));
        assert!(payload.contains("LOAD"));
    }

    #[test]
    fn seek_message_includes_position() {
        let msg = CastMessage::seek("transport-1", 1, 120.5, 5);
        let payload = msg.payload_utf8.unwrap();
        assert!(payload.contains("SEEK"));
        assert!(payload.contains("120.5"));
    }

    #[test]
    fn protocol_version_is_castv2() {
        let msg = CastMessage::ping();
        assert_eq!(msg.protocol_version, ProtocolVersion::Castv210 as i32);
    }
}

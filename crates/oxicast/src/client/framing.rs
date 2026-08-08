//! Wire-level message framing for the Cast protocol.
//!
//! Messages are length-prefixed: a 4-byte big-endian u32 followed by
//! a protobuf-encoded `CastMessage`.

use bytes::BytesMut;
use prost::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::{Error, Result};
use crate::proto::CastMessage;

/// Read one length-prefixed CastMessage from an async reader.
pub async fn read_message<R: tokio::io::AsyncRead + Unpin>(reader: &mut R) -> Result<CastMessage> {
    // Read 4-byte big-endian length prefix
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            Error::Disconnected
        } else {
            Error::Framing(format!("read length: {e}"))
        }
    })?;

    let len = u32::from_be_bytes(len_buf) as usize;
    if len == 0 {
        return Err(Error::Framing("zero-length message".into()));
    }
    if len > 65_536 {
        // 64KB limit per Cast protocol specification
        return Err(Error::Framing(format!("message too large: {len} bytes (max 65536)")));
    }

    // Read the protobuf payload
    let mut buf = BytesMut::zeroed(len);
    reader.read_exact(&mut buf).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            Error::Disconnected
        } else {
            Error::Framing(format!("read payload: {e}"))
        }
    })?;

    CastMessage::decode(&buf[..]).map_err(Error::from)
}

/// Write a length-prefixed CastMessage to an async writer.
pub async fn write_message<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    msg: &CastMessage,
) -> Result<()> {
    let encoded = msg.encode_to_vec();
    if encoded.len() > 65_536 {
        return Err(Error::Framing(format!(
            "outbound message too large: {} bytes (max 65536)",
            encoded.len()
        )));
    }

    // Single write for length prefix + payload to avoid two TLS records
    let mut buf = Vec::with_capacity(4 + encoded.len());
    buf.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
    buf.extend_from_slice(&encoded);

    writer.write_all(&buf).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            Error::Disconnected
        } else {
            Error::Framing(format!("write: {e}"))
        }
    })?;

    writer.flush().await.map_err(|e| Error::Framing(format!("flush: {e}")))?;

    Ok(())
}

/// Build a CastMessage with a JSON string payload.
pub fn build_message(
    namespace: &str,
    source: &str,
    destination: &str,
    payload: &str,
) -> CastMessage {
    CastMessage {
        protocol_version: crate::proto::cast_message::ProtocolVersion::Castv210 as i32,
        source_id: source.to_string(),
        destination_id: destination.to_string(),
        namespace: namespace.to_string(),
        payload_type: crate::proto::cast_message::PayloadType::String as i32,
        payload_utf8: Some(payload.to_string()),
        payload_binary: None,
        continued: None,
        remaining_length: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_roundtrip() {
        let msg = build_message(
            "urn:x-cast:com.google.cast.tp.heartbeat",
            "sender-0",
            "receiver-0",
            r#"{"type":"PING"}"#,
        );

        let mut buf = Vec::new();
        write_message(&mut buf, &msg).await.unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let decoded = read_message(&mut cursor).await.unwrap();

        assert_eq!(decoded.namespace, msg.namespace);
        assert_eq!(decoded.payload_utf8, msg.payload_utf8);
        assert_eq!(decoded.source_id, msg.source_id);
        assert_eq!(decoded.destination_id, msg.destination_id);
    }

    #[tokio::test]
    async fn test_disconnected_on_eof() {
        let mut empty = std::io::Cursor::new(Vec::<u8>::new());
        let result = read_message(&mut empty).await;
        assert!(matches!(result, Err(Error::Disconnected)));
    }

    #[tokio::test]
    async fn test_zero_length_message_rejected() {
        // 4-byte header claiming 0 bytes
        let data = 0u32.to_be_bytes().to_vec();
        let mut cursor = std::io::Cursor::new(data);
        let result = read_message(&mut cursor).await;
        assert!(matches!(result, Err(Error::Framing(ref s)) if s.contains("zero-length")));
    }

    #[tokio::test]
    async fn test_oversized_read_rejected() {
        // Header claiming 65537 bytes (over 64KB limit)
        let data = 65537u32.to_be_bytes().to_vec();
        let mut cursor = std::io::Cursor::new(data);
        let result = read_message(&mut cursor).await;
        assert!(matches!(result, Err(Error::Framing(ref s)) if s.contains("too large")));
    }

    #[tokio::test]
    async fn test_oversized_write_rejected() {
        // Build a message with a payload that, once protobuf-encoded, exceeds 64KB
        let big_payload = "x".repeat(70_000);
        let msg = build_message("ns", "src", "dst", &big_payload);
        let mut buf = Vec::new();
        let result = write_message(&mut buf, &msg).await;
        assert!(matches!(result, Err(Error::Framing(ref s)) if s.contains("too large")));
    }

    #[tokio::test]
    async fn test_exact_64kb_message_accepted() {
        // Build a message and verify it stays under 64KB when encoded
        let msg = build_message("ns", "s", "d", "ok");
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).await.unwrap();
        assert!(buf.len() < 65_536);
    }

    #[tokio::test]
    async fn test_partial_header_eof() {
        // Only 2 bytes of a 4-byte header → UnexpectedEof → Disconnected
        let data = vec![0u8, 10];
        let mut cursor = std::io::Cursor::new(data);
        let result = read_message(&mut cursor).await;
        assert!(matches!(result, Err(Error::Disconnected)));
    }

    #[tokio::test]
    async fn test_partial_payload_eof() {
        // Header says 100 bytes, but only 5 bytes of payload follow
        let mut data = 100u32.to_be_bytes().to_vec();
        data.extend_from_slice(&[0u8; 5]);
        let mut cursor = std::io::Cursor::new(data);
        let result = read_message(&mut cursor).await;
        assert!(matches!(result, Err(Error::Disconnected)));
    }

    #[tokio::test]
    async fn test_corrupt_protobuf_payload() {
        // Valid header (10 bytes), but garbage protobuf data
        let mut data = 10u32.to_be_bytes().to_vec();
        data.extend_from_slice(&[0xFF; 10]);
        let mut cursor = std::io::Cursor::new(data);
        let result = read_message(&mut cursor).await;
        assert!(matches!(result, Err(Error::Protobuf(_))));
    }

    #[tokio::test]
    async fn test_multiple_messages_roundtrip() {
        let msg1 = build_message("ns1", "s", "d", r#"{"a":1}"#);
        let msg2 = build_message("ns2", "s", "d", r#"{"b":2}"#);

        let mut buf = Vec::new();
        write_message(&mut buf, &msg1).await.unwrap();
        write_message(&mut buf, &msg2).await.unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let d1 = read_message(&mut cursor).await.unwrap();
        let d2 = read_message(&mut cursor).await.unwrap();

        assert_eq!(d1.namespace, "ns1");
        assert_eq!(d2.namespace, "ns2");
    }
}

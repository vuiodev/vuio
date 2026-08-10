use anyhow::{Context, Result};
use hap_crypto::SessionKeys;
use hap_transport::record_test_support::{decrypt_frame, encrypt_frame, NonceCounter};
use std::{collections::HashMap, net::SocketAddr, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
const DATA_STREAM_HEADER_BYTES: usize = 32;

pub struct AirplayConnection {
    stream: tokio::net::TcpStream,
    secure: Option<SecureState>,
    wire_buffer: Vec<u8>,
    plain_buffer: Vec<u8>,
}

struct SecureState {
    keys: SessionKeys,
    read_counter: NonceCounter,
    write_counter: NonceCounter,
}

pub struct Response {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl AirplayConnection {
    pub async fn connect(address: SocketAddr) -> Result<Self> {
        let stream = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::net::TcpStream::connect(address),
        )
        .await
        .context("AirPlay connection timed out")??;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream,
            secure: None,
            wire_buffer: Vec::new(),
            plain_buffer: Vec::new(),
        })
    }

    pub fn secure(&mut self, keys: SessionKeys) {
        self.secure = Some(SecureState {
            keys,
            read_counter: NonceCounter::new(),
            write_counter: NonceCounter::new(),
        });
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.stream
            .local_addr()
            .context("reading the local AirPlay socket address")
    }

    pub async fn request(
        &mut self,
        method: &str,
        path: &str,
        protocol: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response> {
        self.write_request(method, path, protocol, headers, body)
            .await?;
        self.read_response().await
    }

    pub async fn request_while_serving_events(
        &mut self,
        method: &str,
        path: &str,
        protocol: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<Response> {
        self.write_request(method, path, protocol, headers, body)
            .await?;
        loop {
            // A buffered message is only ours when it opens with a status line.
            // Anything else the receiver pushes here is an event request that has
            // to be answered before our response can arrive.
            if self.plain_buffer.starts_with(b"HTTP/") || self.plain_buffer.starts_with(b"RTSP/") {
                if let Some(response) = take_response(&mut self.plain_buffer)? {
                    return Ok(response);
                }
            } else if complete_message(&self.plain_buffer)? {
                self.handle_http_event_request().await?;
                continue;
            }
            anyhow::ensure!(
                self.plain_buffer.len() <= MAX_MESSAGE_BYTES,
                "AirPlay response exceeded {MAX_MESSAGE_BYTES} bytes"
            );
            self.read_plain_chunk().await?;
        }
    }

    async fn write_request(
        &mut self,
        method: &str,
        path: &str,
        protocol: &str,
        headers: &[(&str, String)],
        body: &[u8],
    ) -> Result<()> {
        let mut message = format!("{method} {path} {protocol}\r\n").into_bytes();
        let mut has_length = false;
        for (name, value) in headers {
            anyhow::ensure!(
                !name.contains(['\r', '\n']) && !value.contains(['\r', '\n']),
                "invalid AirPlay header"
            );
            has_length |= name.eq_ignore_ascii_case("content-length");
            message.extend_from_slice(name.as_bytes());
            message.extend_from_slice(b": ");
            message.extend_from_slice(value.as_bytes());
            message.extend_from_slice(b"\r\n");
        }
        if !has_length {
            message.extend_from_slice(format!("Content-Length: {}\r\n", body.len()).as_bytes());
        }
        message.extend_from_slice(b"\r\n");
        message.extend_from_slice(body);
        self.write_plain(&message).await
    }

    pub async fn serve_events(
        mut self,
        replies: tokio::sync::mpsc::UnboundedSender<(u64, Vec<u8>)>,
    ) -> Result<()> {
        loop {
            while self.plain_buffer.len() < 4 {
                self.read_plain_chunk().await?;
            }
            if starts_data_stream_message(&self.plain_buffer) {
                let message = loop {
                    if let Some(message) = take_data_stream_message(&mut self.plain_buffer)? {
                        break message;
                    }
                    self.read_plain_chunk().await?;
                };
                if message.message_type.starts_with(b"sync") {
                    let reply = data_stream_frame(b"rply", b"\0\0\0\0", message.sequence, &[])?;
                    self.write_plain(&reply).await?;
                } else if message.message_type.starts_with(b"rply") {
                    let _ = replies.send((message.sequence, message.body));
                } else {
                    tracing::debug!(
                        sequence = message.sequence,
                        message_type = ?String::from_utf8_lossy(&message.message_type),
                        "ignored unrelated AirPlay event data-stream message"
                    );
                }
                continue;
            }
            self.handle_http_event_request().await?;
        }
    }

    async fn handle_http_event_request(&mut self) -> Result<()> {
        while !complete_message(&self.plain_buffer)? {
            anyhow::ensure!(
                self.plain_buffer.len() <= MAX_MESSAGE_BYTES,
                "AirPlay event exceeded {MAX_MESSAGE_BYTES} bytes"
            );
            self.read_plain_chunk().await?;
        }
        let header_end = self
            .plain_buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .context("AirPlay event omitted its headers")?;
        let header = std::str::from_utf8(&self.plain_buffer[..header_end])?;
        let length = content_length(header)?;
        let request_line = header
            .lines()
            .next()
            .unwrap_or("unknown AirPlay event")
            .to_string();
        let protocol = request_line
            .split_whitespace()
            .nth(2)
            .unwrap_or("HTTP/1.1")
            .to_string();
        let cseq = header_value(header, "cseq").map(str::to_string);
        let stream_id = header_value(header, "x-apple-stream-id").map(str::to_string);
        let content_type = header_value(header, "content-type").map(str::to_string);
        let body = &self.plain_buffer[header_end + 4..header_end + 4 + length];
        if let Ok(value) = plist::Value::from_reader(std::io::Cursor::new(body)) {
            let dictionary = value.as_dictionary();
            let keys = dictionary
                .map(|dictionary| dictionary.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            let nested = dictionary
                .and_then(|dictionary| dictionary.get("params"))
                .and_then(plist::Value::as_dictionary)
                .and_then(|parameters| parameters.get("data"))
                .and_then(plist::Value::as_data)
                .and_then(|data| plist::Value::from_reader(std::io::Cursor::new(data)).ok());
            let nested_dictionary = nested.as_ref().and_then(plist::Value::as_dictionary);
            let nested_keys = nested_dictionary
                .map(|dictionary| dictionary.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            let nested_type = nested_dictionary
                .and_then(|dictionary| dictionary.get("type"))
                .and_then(plist::Value::as_string);
            let nested_error = nested_dictionary
                .and_then(|dictionary| dictionary.get("errorCode"))
                .and_then(plist::Value::as_signed_integer);
            let nested_name = nested_dictionary
                .and_then(|dictionary| dictionary.get("name"))
                .and_then(plist::Value::as_string);
            let nested_reason = nested_dictionary.and_then(|dictionary| dictionary.get("reason"));
            let nested_item_uuid = nested_dictionary
                .and_then(|dictionary| dictionary.get("itemCurrent"))
                .and_then(plist::Value::as_dictionary)
                .and_then(|item| item.get("uuid"))
                .and_then(plist::Value::as_string);
            tracing::debug!(
                request_line,
                cseq,
                stream_id,
                content_type,
                ?keys,
                ?nested_keys,
                nested_type,
                nested_error,
                nested_name,
                ?nested_reason,
                nested_item_uuid,
                "received AirPlay event"
            );
        } else {
            tracing::debug!(request_line, body_length = length, "received AirPlay event");
        }
        self.plain_buffer.drain(..header_end + 4 + length);
        let mut response =
            format!("{protocol} 200 OK\r\nContent-Length: 0\r\nAudio-Latency: 0\r\n");
        if let Some(cseq) = cseq {
            response.push_str(&format!("CSeq: {cseq}\r\n"));
        }
        response.push_str("\r\n");
        self.write_plain(response.as_bytes()).await
    }

    async fn write_plain(&mut self, bytes: &[u8]) -> Result<()> {
        if let Some(secure) = &mut self.secure {
            for block in bytes.chunks(1024) {
                let frame =
                    encrypt_frame(&secure.keys.write_key, &mut secure.write_counter, block)?;
                self.stream.write_all(&frame).await?;
            }
        } else {
            self.stream.write_all(bytes).await?;
        }
        self.stream.flush().await?;
        Ok(())
    }

    async fn read_response(&mut self) -> Result<Response> {
        loop {
            if let Some(response) = take_response(&mut self.plain_buffer)? {
                return Ok(response);
            }
            anyhow::ensure!(
                self.plain_buffer.len() <= MAX_MESSAGE_BYTES,
                "AirPlay response exceeded {MAX_MESSAGE_BYTES} bytes"
            );
            self.read_plain_chunk().await?;
        }
    }

    async fn read_plain_chunk(&mut self) -> Result<()> {
        if let Some(secure) = &mut self.secure {
            loop {
                if let Some(block) = decrypt_frame(
                    &secure.keys.read_key,
                    &mut secure.read_counter,
                    &self.wire_buffer,
                )? {
                    let frame_len = 2 + block.len() + 16;
                    self.wire_buffer.drain(..frame_len);
                    self.plain_buffer.extend_from_slice(&block);
                    return Ok(());
                }
                let read = self.stream.read_buf(&mut self.wire_buffer).await?;
                anyhow::ensure!(read != 0, "AirPlay receiver closed the secure connection");
            }
        }
        let read = self.stream.read_buf(&mut self.plain_buffer).await?;
        anyhow::ensure!(read != 0, "AirPlay receiver closed the connection");
        Ok(())
    }
}

struct DataStreamMessage {
    message_type: [u8; 12],
    sequence: u64,
    body: Vec<u8>,
}

fn data_stream_frame(
    message_type: &[u8; 4],
    command: &[u8; 4],
    sequence: u64,
    body: &[u8],
) -> Result<Vec<u8>> {
    let size = DATA_STREAM_HEADER_BYTES
        .checked_add(body.len())
        .context("AirPlay data-stream message is too large")?;
    anyhow::ensure!(
        size <= MAX_MESSAGE_BYTES,
        "AirPlay data-stream message is too large"
    );
    let size = u32::try_from(size).context("AirPlay data-stream message is too large")?;
    let mut frame = Vec::with_capacity(size as usize);
    frame.extend_from_slice(&size.to_be_bytes());
    frame.extend_from_slice(message_type);
    frame.extend_from_slice(&[0; 8]);
    frame.extend_from_slice(command);
    frame.extend_from_slice(&sequence.to_be_bytes());
    frame.extend_from_slice(&0u32.to_be_bytes());
    frame.extend_from_slice(body);
    Ok(frame)
}

fn take_data_stream_message(buffer: &mut Vec<u8>) -> Result<Option<DataStreamMessage>> {
    if buffer.len() < DATA_STREAM_HEADER_BYTES {
        return Ok(None);
    }
    let size = u32::from_be_bytes(buffer[..4].try_into()?) as usize;
    anyhow::ensure!(
        (DATA_STREAM_HEADER_BYTES..=MAX_MESSAGE_BYTES).contains(&size),
        "invalid AirPlay data-stream message size"
    );
    if buffer.len() < size {
        return Ok(None);
    }
    let message_type = buffer[4..16].try_into()?;
    let sequence = u64::from_be_bytes(buffer[20..28].try_into()?);
    let body = buffer[DATA_STREAM_HEADER_BYTES..size].to_vec();
    buffer.drain(..size);
    Ok(Some(DataStreamMessage {
        message_type,
        sequence,
        body,
    }))
}

fn starts_data_stream_message(buffer: &[u8]) -> bool {
    buffer
        .get(..4)
        .and_then(|size| <[u8; 4]>::try_from(size).ok())
        .map(u32::from_be_bytes)
        .is_some_and(|size| {
            (DATA_STREAM_HEADER_BYTES as u32..=MAX_MESSAGE_BYTES as u32).contains(&size)
        })
}

fn take_response(buffer: &mut Vec<u8>) -> Result<Option<Response>> {
    let Some(header_end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Ok(None);
    };
    let header_bytes = &buffer[..header_end];
    let header = std::str::from_utf8(header_bytes).context("invalid AirPlay response headers")?;
    let mut lines = header.split("\r\n");
    let status_line = lines.next().context("missing AirPlay status line")?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .context("missing AirPlay status")?
        .parse::<u16>()
        .context("invalid AirPlay status")?;
    let mut headers = HashMap::new();
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .context("malformed AirPlay response header")?;
        headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
    }
    let length = headers
        .get("content-length")
        .map(|value| value.parse::<usize>())
        .transpose()
        .context("invalid AirPlay content length")?
        .unwrap_or(0);
    anyhow::ensure!(
        length <= MAX_MESSAGE_BYTES,
        "AirPlay response body is too large"
    );
    let total = header_end + 4 + length;
    if buffer.len() < total {
        return Ok(None);
    }
    let body = buffer[header_end + 4..total].to_vec();
    buffer.drain(..total);
    Ok(Some(Response {
        status,
        headers,
        body,
    }))
}

fn content_length(header: &str) -> Result<usize> {
    for line in header.split("\r\n").skip(1) {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                return value
                    .trim()
                    .parse()
                    .context("invalid AirPlay content length");
            }
        }
    }
    Ok(0)
}

fn header_value<'a>(header: &'a str, expected_name: &str) -> Option<&'a str> {
    header.split("\r\n").skip(1).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case(expected_name)
            .then(|| value.trim())
    })
}

fn complete_message(buffer: &[u8]) -> Result<bool> {
    let Some(header_end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Ok(false);
    };
    let header = std::str::from_utf8(&buffer[..header_end])?;
    let length = content_length(header)?;
    anyhow::ensure!(
        length <= MAX_MESSAGE_BYTES,
        "AirPlay event body is too large"
    );
    Ok(buffer.len() >= header_end + 4 + length)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_parser_preserves_following_message() {
        let mut bytes = b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nX-Test: yes\r\n\r\noneRTSP/1.0 204 OK\r\nContent-Length: 0\r\n\r\n".to_vec();
        let first = take_response(&mut bytes).unwrap().unwrap();
        assert_eq!(first.status, 200);
        assert_eq!(first.body, b"one");
        assert_eq!(first.headers.get("x-test").map(String::as_str), Some("yes"));
        assert_eq!(take_response(&mut bytes).unwrap().unwrap().status, 204);
        assert!(bytes.is_empty());
    }

    #[tokio::test]
    async fn request_answers_an_event_that_arrives_before_the_response() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let receiver = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0u8; 1024];
            let read = stream.read(&mut buffer).await.unwrap();
            let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
            // Push an event request ahead of the response we owe the sender.
            stream
                .write_all(b"POST /event RTSP/1.0\r\nCSeq: 9\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
            stream
                .write_all(b"RTSP/1.0 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .await
                .unwrap();
            stream.flush().await.unwrap();
            let mut acknowledgement = vec![0u8; 1024];
            let read = stream.read(&mut acknowledgement).await.unwrap();
            (
                request,
                String::from_utf8_lossy(&acknowledgement[..read]).into_owned(),
            )
        });

        let mut connection = AirplayConnection::connect(address).await.unwrap();
        let response = connection
            .request_while_serving_events(
                "RECORD",
                "rtsp://127.0.0.1/1",
                "RTSP/1.0",
                &[("CSeq", "1".to_string())],
                &[],
            )
            .await
            .unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"ok");

        let (request, acknowledgement) = receiver.await.unwrap();
        assert!(
            request.starts_with("RECORD rtsp://127.0.0.1/1 RTSP/1.0\r\n"),
            "{request}"
        );
        assert!(
            acknowledgement.starts_with("RTSP/1.0 200 OK\r\n"),
            "{acknowledgement}"
        );
        assert!(acknowledgement.contains("CSeq: 9"), "{acknowledgement}");
    }

    #[test]
    fn data_stream_parser_preserves_following_frame() {
        let mut bytes = data_stream_frame(b"sync", b"comm", 7, b"one").unwrap();
        bytes.extend(data_stream_frame(b"rply", b"\0\0\0\0", 8, b"two").unwrap());
        let first = take_data_stream_message(&mut bytes).unwrap().unwrap();
        assert_eq!(&first.message_type[..4], b"sync");
        assert_eq!(first.sequence, 7);
        assert_eq!(first.body, b"one");
        let second = take_data_stream_message(&mut bytes).unwrap().unwrap();
        assert_eq!(&second.message_type[..4], b"rply");
        assert_eq!(second.sequence, 8);
        assert_eq!(second.body, b"two");
        assert!(bytes.is_empty());
    }
}

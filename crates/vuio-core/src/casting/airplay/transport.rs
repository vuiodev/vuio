use anyhow::{Context, Result};
use hap_crypto::SessionKeys;
use hap_transport::record_test_support::{decrypt_frame, encrypt_frame, NonceCounter};
use std::{collections::HashMap, net::SocketAddr, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

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
    pub _headers: HashMap<String, String>,
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
        self.write_plain(&message).await?;
        self.read_response().await
    }

    pub async fn serve_events(mut self) -> Result<()> {
        loop {
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
            self.plain_buffer.drain(..header_end + 4 + length);
            self.write_plain(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nAudio-Latency: 0\r\n\r\n")
                .await?;
        }
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
        _headers: headers,
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
        assert_eq!(
            first._headers.get("x-test").map(String::as_str),
            Some("yes")
        );
        assert_eq!(take_response(&mut bytes).unwrap().unwrap().status, 204);
        assert!(bytes.is_empty());
    }
}

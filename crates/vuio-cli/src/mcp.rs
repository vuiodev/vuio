//! `vuio mcp` — speak MCP over stdio, on behalf of a running server.
//!
//! Some clients will only launch a local process and talk to it over pipes;
//! Claude Desktop is the one that matters here. This is that process. It reads
//! JSON-RPC from stdin, forwards each message to a VuIO server's `/mcp`
//! endpoint, and writes the answer to stdout.
//!
//! A **proxy**, not a second server. Running the tools in-process would mean a
//! second writer on a single-writer SQLite file, and the casting tools need the
//! renderer cache and SSDP state that only the running server has. So this owns
//! nothing: every request is answered by the server that already has the
//! library open.

use std::io::Write as _;

use anyhow::{Context, Result};

/// Where the mirrored request headers come from, so an intermediary can route
/// without parsing the body. Derived here rather than demanded of the client,
/// because a stdio client has no idea it is talking to HTTP.
const PROTOCOL_VERSION_HEADER: &str = "MCP-Protocol-Version";
const METHOD_HEADER: &str = "Mcp-Method";
const NAME_HEADER: &str = "Mcp-Name";

/// The revision this proxy speaks upstream.
///
/// Fixed rather than negotiated: both ends are this project, and pinning it
/// means a stdio client's own version — whatever era it is from — never has to
/// agree with the server's.
const PROTOCOL_VERSION: &str = "2026-07-28";

pub struct Options {
    pub url: String,
    pub token: Option<String>,
    pub token_file: Option<String>,
}

pub async fn run(options: Options) -> Result<()> {
    let endpoint = endpoint_url(&options.url);
    let token = resolve_token(&options)?;

    let client = reqwest::Client::builder()
        .user_agent(concat!("vuio-mcp-proxy/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("failed to build the HTTP client")?;

    // Everything the client says arrives on stdin, one JSON-RPC message per
    // line, which is what the stdio transport specifies.
    let stdin = tokio::io::stdin();
    let mut lines = tokio::io::AsyncBufReadExt::lines(tokio::io::BufReader::new(stdin));

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // The handshake is answered here rather than forwarded. This proxy is
        // the boundary between the two eras of the protocol: it speaks the
        // stateless revision upstream, where `initialize` no longer exists, and
        // whatever the client speaks downstream.
        if let Some(local) = handle_locally(line) {
            // A notification carries no id and gets no answer, here as upstream.
            if local["id"].is_null() {
                continue;
            }
            emit(&local)?;
            continue;
        }
        match forward(&client, &endpoint, token.as_deref(), line).await {
            Ok(Some(response)) => emit(&response)?,
            // A notification has no id and therefore no answer; the server
            // acknowledged it and there is nothing to write back.
            Ok(None) => {}
            Err(error) => {
                // Never let a transport failure be silence: a client waiting on
                // an id would wait forever. Answer with the id it used, so it
                // can fail the one call rather than the session.
                let id = serde_json::from_str::<serde_json::Value>(line)
                    .ok()
                    .and_then(|message| message.get("id").cloned());
                if let Some(id) = id {
                    emit(&serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32603,
                            "message": format!("vuio mcp proxy: {error:#}")
                        }
                    }))?;
                } else {
                    // Diagnostics go to stderr: stdout carries the protocol and
                    // nothing else.
                    eprintln!("vuio mcp proxy: {error:#}");
                }
            }
        }
    }
    Ok(())
}

/// The messages this proxy answers itself, because they do not exist upstream.
///
/// `initialize` and `ping` belong to the revisions before `2026-07-28`, and the
/// server implements only what a stateless transport needs. A stdio client is
/// very likely to be from that older era — that is why it wants a pipe rather
/// than an HTTP endpoint — so its handshake ends here.
///
/// Returns `None` for anything that should go upstream.
fn handle_locally(line: &str) -> Option<serde_json::Value> {
    let message: serde_json::Value = serde_json::from_str(line).ok()?;
    let method = message.get("method")?.as_str()?;
    let id = message.get("id").cloned();

    match method {
        "initialize" => {
            let requested = message
                .get("params")
                .and_then(|params| params.get("protocolVersion"))
                .and_then(|version| version.as_str())
                .unwrap_or("2025-11-25")
                .to_owned();
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    // Echo whatever the client asked for. This proxy translates
                    // rather than negotiates: the shape it speaks upstream is
                    // fixed, so there is nothing for the client to lose by
                    // keeping its own.
                    "protocolVersion": requested,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": {
                        "name": "vuio-media-server",
                        "title": "VuIO Media Server",
                        "version": env!("CARGO_PKG_VERSION"),
                    }
                }
            }))
        }
        "ping" => Some(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {}
        })),
        _ => None,
    }
}

/// Send one message upstream and return the response, if it has one.
async fn forward(
    client: &reqwest::Client,
    endpoint: &str,
    token: Option<&str>,
    line: &str,
) -> Result<Option<serde_json::Value>> {
    let message: serde_json::Value =
        serde_json::from_str(line).context("the client sent something that is not JSON")?;

    let method = message
        .get("method")
        .and_then(|value| value.as_str())
        .context("the client sent a message with no method")?;

    let mut request = client
        .post(endpoint)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header(PROTOCOL_VERSION_HEADER, PROTOCOL_VERSION)
        .header(METHOD_HEADER, method);

    // `tools/call` mirrors the tool name. A name that is not plain ASCII is
    // carried in the sentinel encoding the transport defines for exactly that.
    if method == "tools/call" {
        if let Some(name) = message
            .get("params")
            .and_then(|params| params.get("name"))
            .and_then(|name| name.as_str())
        {
            request = request.header(NAME_HEADER, encode_header_value(name));
        }
    }
    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }

    // The body is the client's message with the protocol version stamped into
    // `_meta`, which is where the modern revision carries it. The client may be
    // from an era that has never heard of `_meta`; that is the point of a proxy.
    let body = with_protocol_version(message);
    let response = request
        .json(&body)
        .send()
        .await
        .with_context(|| format!("could not reach {endpoint}"))?;

    if response.status() == reqwest::StatusCode::ACCEPTED {
        return Ok(None);
    }
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    match serde_json::from_str::<serde_json::Value>(&text) {
        // A JSON-RPC error body is a real answer, whatever the HTTP status: it
        // tells the client what was wrong with its request.
        Ok(value) => Ok(Some(value)),
        Err(_) if status.is_success() => Ok(None),
        Err(_) if status == reqwest::StatusCode::UNAUTHORIZED => {
            anyhow::bail!("{endpoint} requires a management token. Pass --token or --token-file.")
        }
        Err(_) => anyhow::bail!("{status} from {endpoint}: {}", text.trim()),
    }
}

/// Stamp `_meta.io.modelcontextprotocol/protocolVersion` into a message.
fn with_protocol_version(mut message: serde_json::Value) -> serde_json::Value {
    let params = message
        .as_object_mut()
        .map(|object| {
            object
                .entry("params")
                .or_insert_with(|| serde_json::json!({}))
        })
        .filter(|params| params.is_object());
    if let Some(params) = params {
        if let Some(params) = params.as_object_mut() {
            let meta = params
                .entry("_meta")
                .or_insert_with(|| serde_json::json!({}));
            if let Some(meta) = meta.as_object_mut() {
                meta.insert(
                    "io.modelcontextprotocol/protocolVersion".to_owned(),
                    serde_json::json!(PROTOCOL_VERSION),
                );
            }
        }
    }
    message
}

/// A header value, Base64-wrapped when it cannot be sent as plain ASCII.
///
/// HTTP field values are visible ASCII only, so a tool name outside that set —
/// or one that happens to look like the sentinel — is encoded rather than sent
/// raw. The server decodes before comparing it to the body.
fn encode_header_value(value: &str) -> String {
    let safe = !value.is_empty()
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
        && !(value.starts_with("=?base64?") && value.ends_with("?="));
    if safe {
        return value.to_owned();
    }
    format!("=?base64?{}?=", base64_encode(value.as_bytes()))
}

fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let bytes = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let bits = (u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2]);
        out.push(ALPHABET[(bits >> 18) as usize & 0x3f] as char);
        out.push(ALPHABET[(bits >> 12) as usize & 0x3f] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(bits >> 6) as usize & 0x3f] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[bits as usize & 0x3f] as char
        } else {
            '='
        });
    }
    out
}

/// Write one message to stdout and flush it.
///
/// Flushing every line matters: the client is blocked on this pipe, and a
/// buffered response is a hung session.
fn emit(message: &serde_json::Value) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, message)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

/// Accept either a base URL or the endpoint itself, so `--url http://host:8080`
/// and `--url http://host:8080/mcp` both work.
fn endpoint_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    if trimmed.ends_with("/mcp") {
        trimmed.to_owned()
    } else {
        format!("{trimmed}/mcp")
    }
}

/// The admin token, from the flag or the file it names.
///
/// `--token-file` exists because a token on a command line is visible in the
/// process table, and a desktop client's configuration is the usual caller here.
fn resolve_token(options: &Options) -> Result<Option<String>> {
    if let Some(token) = options.token.as_deref().map(str::trim) {
        if !token.is_empty() {
            return Ok(Some(token.to_owned()));
        }
    }
    let Some(path) = options.token_file.as_deref().map(str::trim) else {
        return Ok(None);
    };
    if path.is_empty() {
        return Ok(None);
    }
    let token = std::fs::read_to_string(path)
        .with_context(|| format!("could not read the token file {path}"))?;
    let token = token.trim();
    if token.is_empty() {
        anyhow::bail!("the token file {path} is empty");
    }
    Ok(Some(token.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_base_url_and_an_endpoint_url_both_resolve_to_the_endpoint() {
        for input in [
            "http://nas.local:8080",
            "http://nas.local:8080/",
            "http://nas.local:8080/mcp",
            "http://nas.local:8080/mcp/",
        ] {
            assert_eq!(endpoint_url(input), "http://nas.local:8080/mcp", "{input}");
        }
    }

    #[test]
    fn the_protocol_version_is_stamped_into_meta() {
        let stamped = with_protocol_version(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/list"
        }));
        assert_eq!(
            stamped["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"],
            PROTOCOL_VERSION
        );

        // Existing params survive.
        let stamped = with_protocol_version(serde_json::json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "search_media", "arguments": { "query": "x" } }
        }));
        assert_eq!(stamped["params"]["name"], "search_media");
        assert_eq!(stamped["params"]["arguments"]["query"], "x");
        assert_eq!(
            stamped["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"],
            PROTOCOL_VERSION
        );
    }

    #[test]
    fn header_values_are_encoded_only_when_they_have_to_be() {
        assert_eq!(encode_header_value("search_media"), "search_media");
        assert_eq!(
            encode_header_value("Hello, 世界"),
            "=?base64?SGVsbG8sIOS4lueVjA==?="
        );
        // A value that merely looks like the sentinel is encoded too, so the
        // server cannot mistake it for one.
        assert_eq!(
            encode_header_value("=?base64?literal?="),
            "=?base64?PT9iYXNlNjQ/bGl0ZXJhbD89?="
        );
    }

    /// The handshake ends at the proxy: `initialize` does not exist upstream,
    /// and a stdio client is very likely to open with one.
    #[test]
    fn the_handshake_is_answered_locally() {
        let answer = handle_locally(
            r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,
        )
        .expect("initialize is answered here");
        assert_eq!(answer["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(answer["result"]["serverInfo"]["name"], "vuio-media-server");
        assert_eq!(answer["id"], 0);

        // A client on a different revision keeps its own.
        let answer = handle_locally(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
        )
        .unwrap();
        assert_eq!(answer["result"]["protocolVersion"], "2025-06-18");

        assert!(handle_locally(r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#).is_some());

        // Everything that does real work goes upstream.
        for forwarded in [
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"search_media"}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        ] {
            assert!(handle_locally(forwarded).is_none(), "{forwarded}");
        }
    }

    #[test]
    fn a_token_file_outranks_nothing_and_a_flag_outranks_the_file() {
        let dir = std::env::temp_dir().join("vuio-mcp-proxy-token-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("admin.token");
        std::fs::write(&path, "  from-the-file\n").unwrap();
        let file = path.to_string_lossy().into_owned();

        let from_file = resolve_token(&Options {
            url: String::new(),
            token: None,
            token_file: Some(file.clone()),
        })
        .unwrap();
        assert_eq!(from_file.as_deref(), Some("from-the-file"));

        let from_flag = resolve_token(&Options {
            url: String::new(),
            token: Some("from-the-flag".to_owned()),
            token_file: Some(file),
        })
        .unwrap();
        assert_eq!(from_flag.as_deref(), Some("from-the-flag"));

        let neither = resolve_token(&Options {
            url: String::new(),
            token: None,
            token_file: None,
        })
        .unwrap();
        assert_eq!(neither, None);

        let _ = std::fs::remove_dir_all(&dir);
    }
}

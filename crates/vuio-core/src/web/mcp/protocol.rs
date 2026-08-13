//! JSON-RPC 2.0 types and the constants of MCP revision `2026-07-28`.

use serde::{Deserialize, Serialize};

/// The protocol revision this server prefers and implements fully.
///
/// `2026-07-28` is stateless: there is no `initialize` handshake and no session,
/// so every request carries its own version in `_meta` and in the
/// `MCP-Protocol-Version` header, and the two must agree.
pub const PROTOCOL_VERSION: &str = "2026-07-28";

/// Revisions that still negotiate with `initialize`, newest first.
///
/// `2026-07-28` split MCP into two eras. Everything from `2025-03-26` to
/// `2025-11-25` opens with an `initialize` handshake and carries no `_meta`
/// version, and that is still what shipping clients send — Claude Code 2.1.x
/// asks for `2025-11-25`. Refusing them would mean refusing the clients this
/// server exists to be driven by, so both eras are answered on the one endpoint.
pub const LEGACY_PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];

/// Which era a request belongs to.
///
/// Resolved per request rather than per connection, because there is no
/// connection: the modern transport is stateless, so the era is a property of
/// the message in hand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Era {
    /// `2026-07-28`: `_meta` carries the version, headers mirror the body, and
    /// results are stamped with `resultType`.
    Modern,
    /// `2025-03-26` … `2025-11-25`: `initialize` handshake, no mirrored
    /// headers, and results that must not carry fields the client's schema
    /// will not recognise.
    Legacy,
}

pub const SERVER_NAME: &str = "vuio-media-server";
pub const SERVER_TITLE: &str = "VuIO Media Server";

// `_meta` keys. Namespaced by the specification, so they are written out in full
// rather than assembled, and live here so a typo is a compile error at one site.
pub const META_PROTOCOL_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
pub const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

// Error codes. `-32000..=-32019` is the implementation-defined range;
// `-32020..=-32099` is reserved for the specification, which is where the two
// transport errors below come from.
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const HEADER_MISMATCH: i64 = -32020;
pub const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;

/// How long a client may cache `tools/list`.
///
/// The catalog is compiled in, so it cannot change while the process runs — but
/// a restart can change it, because the tool set depends on cargo features and
/// on `[mcp].read_only`. Five minutes bounds how long a client can be wrong.
pub const TOOLS_LIST_TTL_MS: u64 = 300_000;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[serde(default)]
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub id: Option<serde_json::Value>,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    /// Whether this message is a notification rather than a request.
    ///
    /// MCP forbids a null request id, so a null `id` is treated the same as an
    /// absent one — which is what serde gives us anyway, since `Option` cannot
    /// tell the two apart.
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }

    /// The value the `Mcp-Name` header has to mirror, if this method has one.
    ///
    /// Only `tools/call` applies here: `resources/read` and `prompts/get` are
    /// the other two the specification names, and this server implements
    /// neither.
    pub fn mirrored_name(&self) -> Option<&str> {
        if self.method != "tools/call" {
            return None;
        }
        self.params.as_ref()?.get("name")?.as_str()
    }

    /// The protocol version the body claims, from `params._meta`.
    pub fn declared_protocol_version(&self) -> Option<&str> {
        self.params
            .as_ref()?
            .get("_meta")?
            .get(META_PROTOCOL_VERSION)?
            .as_str()
    }

    /// The version an `initialize` request asks for, from `params`.
    pub fn requested_protocol_version(&self) -> Option<&str> {
        self.params.as_ref()?.get("protocolVersion")?.as_str()
    }
}

/// Which legacy revision to answer an `initialize` with.
///
/// Echo the client's own choice when it is one this server understands;
/// otherwise offer the newest legacy revision and let the client decide whether
/// it can live with it. That is the negotiation those revisions specify.
pub(super) fn negotiated_legacy_version(requested: Option<&str>) -> &'static str {
    requested
        .and_then(|requested| {
            LEGACY_PROTOCOL_VERSIONS
                .iter()
                .find(|supported| **supported == requested)
                .copied()
        })
        .unwrap_or(LEGACY_PROTOCOL_VERSIONS[0])
}

/// Every revision this server answers, newest first.
pub(super) fn supported_versions() -> Vec<&'static str> {
    std::iter::once(PROTOCOL_VERSION)
        .chain(LEGACY_PROTOCOL_VERSIONS.iter().copied())
        .collect()
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    /// A successful result, stamped with what the caller's era expects.
    ///
    /// `_meta` is a field in every revision, so the server's identity goes in
    /// unconditionally. `resultType` was introduced by `2026-07-28` and is
    /// withheld from older clients, whose result schemas predate it.
    pub(super) fn success(
        era: Era,
        id: Option<serde_json::Value>,
        mut result: serde_json::Value,
    ) -> Self {
        if let Some(object) = result.as_object_mut() {
            if era == Era::Modern {
                object.insert("resultType".to_owned(), serde_json::json!("complete"));
            }
            let meta = object
                .entry("_meta")
                .or_insert_with(|| serde_json::json!({}));
            if let Some(meta) = meta.as_object_mut() {
                meta.insert(META_SERVER_INFO.to_owned(), server_info());
            }
        }
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub(super) fn error(id: Option<serde_json::Value>, code: i64, message: String) -> Self {
        Self::error_with_data(id, code, message, None)
    }

    pub(super) fn error_with_data(
        id: Option<serde_json::Value>,
        code: i64,
        message: String,
        data: Option<serde_json::Value>,
    ) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data,
            }),
        }
    }
}

pub(super) fn server_info() -> serde_json::Value {
    serde_json::json!({
        "name": SERVER_NAME,
        "title": SERVER_TITLE,
        "version": env!("CARGO_PKG_VERSION"),
    })
}

// ──────────────────────────────────────────
// Header value encoding
// ──────────────────────────────────────────

/// The marker wrapping a header value that could not be sent as plain ASCII.
const BASE64_PREFIX: &str = "=?base64?";
const BASE64_SUFFIX: &str = "?=";

/// Whether a mirrored header value matches the body value it claims to carry.
///
/// A client that cannot represent the value as plain ASCII — or whose value
/// happens to look like the sentinel — sends it Base64-encoded instead. Rather
/// than decode the header, this encodes the body value and compares: the
/// encoding is total, so a malformed header simply fails to match instead of
/// needing its own error path.
pub(super) fn header_matches(header: &str, body: &str) -> bool {
    match header
        .strip_prefix(BASE64_PREFIX)
        .and_then(|rest| rest.strip_suffix(BASE64_SUFFIX))
    {
        Some(encoded) => encoded == base64_encode(body.as_bytes()),
        None => header == body,
    }
}

/// Standard Base64 with padding, which is what the sentinel format specifies.
///
/// Written out rather than taken from a crate because the `mcp` feature is
/// documented as adding no dependencies, and this is the only encoding it needs.
fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let bits = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_specifications_examples() {
        assert_eq!(base64_encode("Hello, 世界".as_bytes()), "SGVsbG8sIOS4lueVjA==");
        assert_eq!(base64_encode(" padded ".as_bytes()), "IHBhZGRlZCA=");
        assert_eq!(base64_encode("line1\nline2".as_bytes()), "bGluZTEKbGluZTI=");
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn plain_header_values_compare_directly() {
        assert!(header_matches("search_media", "search_media"));
        assert!(!header_matches("search_media", "list_media"));
    }

    #[test]
    fn encoded_header_values_are_compared_against_the_encoded_body() {
        assert!(header_matches("=?base64?SGVsbG8sIOS4lueVjA==?=", "Hello, 世界"));
        assert!(!header_matches("=?base64?SGVsbG8sIOS4lueVjA==?=", "Hello"));
        // A value that merely looks like the sentinel is still encoded by a
        // conforming client, so the literal form must not match.
        assert!(!header_matches("=?base64?literal?=", "=?base64?literal?="));
        assert!(header_matches(
            "=?base64?PT9iYXNlNjQ/bGl0ZXJhbD89?=",
            "=?base64?literal?="
        ));
    }

    #[test]
    fn a_malformed_encoded_header_fails_to_match_rather_than_erroring() {
        assert!(!header_matches("=?base64?not valid base64!?=", "anything"));
    }

    #[test]
    fn success_stamps_result_type_and_server_info() {
        let response =
            JsonRpcResponse::success(Era::Modern, Some(serde_json::json!(1)), serde_json::json!({}));
        let result = response.result.expect("a result");
        assert_eq!(result["resultType"], "complete");
        assert_eq!(result["_meta"][META_SERVER_INFO]["name"], SERVER_NAME);
    }

    /// `resultType` arrived with `2026-07-28`. A client on an older revision
    /// validates the result against a schema that has never heard of it.
    #[test]
    fn a_legacy_result_carries_no_result_type() {
        let response =
            JsonRpcResponse::success(Era::Legacy, Some(serde_json::json!(1)), serde_json::json!({}));
        let result = response.result.expect("a result");
        assert!(result.get("resultType").is_none());
        assert_eq!(result["_meta"][META_SERVER_INFO]["name"], SERVER_NAME);
    }

    #[test]
    fn initialize_echoes_a_version_the_client_asked_for_when_we_have_it() {
        assert_eq!(negotiated_legacy_version(Some("2025-06-18")), "2025-06-18");
        assert_eq!(negotiated_legacy_version(Some("2025-03-26")), "2025-03-26");
        // Anything unrecognised gets this server's newest legacy revision, and
        // the client decides whether it can live with it.
        assert_eq!(negotiated_legacy_version(Some("1999-01-01")), "2025-11-25");
        assert_eq!(negotiated_legacy_version(None), "2025-11-25");
    }

    #[test]
    fn the_supported_list_leads_with_the_preferred_revision() {
        let supported = supported_versions();
        assert_eq!(supported[0], PROTOCOL_VERSION);
        assert!(supported.contains(&"2025-11-25"));
    }

    #[test]
    fn a_null_id_is_treated_as_a_notification() {
        // MCP forbids a null request id, so the only sender producing one is a
        // client that meant to send a notification.
        let request: JsonRpcRequest =
            serde_json::from_str(r#"{"jsonrpc":"2.0","method":"x","id":null}"#).unwrap();
        assert!(request.is_notification());
    }
}

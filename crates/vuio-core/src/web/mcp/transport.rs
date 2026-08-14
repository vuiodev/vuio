//! The Streamable HTTP transport of MCP revision `2026-07-28`.
//!
//! One endpoint, `POST /mcp`, one JSON-RPC message per request. There is no
//! session, no `Mcp-Session-Id`, no GET stream and no resumability — all four
//! were removed from the transport in this revision, which is what lets the
//! whole thing be a plain request handler with no state behind it.

use super::*;

/// Header names the transport mirrors from the body, so an intermediary can
/// route on them without parsing JSON. All are case-insensitive on the wire;
/// `HeaderMap` lookups already are.
const HEADER_PROTOCOL_VERSION: &str = "mcp-protocol-version";
const HEADER_METHOD: &str = "mcp-method";
const HEADER_NAME: &str = "mcp-name";

/// `GET` and `DELETE` on the MCP endpoint belonged to the session-based
/// revisions. A server that implements only this one answers `405`, which is
/// how an older client learns to stop asking.
pub async fn method_not_allowed() -> axum::response::Response {
    StatusCode::METHOD_NOT_ALLOWED.into_response()
}

pub async fn mcp_handler<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    headers: HeaderMap,
    body: Bytes,
) -> axum::response::Response {
    // 1. Origin, before anything else touches the body. A browser page on
    //    another origin must not be able to drive a media server on the LAN,
    //    which is the DNS-rebinding attack this closes. A non-browser client
    //    sends no Origin at all, and that is fine — absence is not a claim.
    if !origin_permitted(&headers) {
        return rejection(
            StatusCode::FORBIDDEN,
            None,
            INVALID_REQUEST,
            "Origin is not permitted".to_owned(),
        );
    }

    // 2. `[mcp].require_auth` demands a token for this endpoint even on a
    //    server that leaves the dashboard open. `require_management` has
    //    already run and waved the request through in that case, so this is a
    //    second, narrower gate rather than a duplicate of it.
    if state.current_config().mcp.require_auth && !state.auth.bearer_valid(&headers) {
        return rejection(
            StatusCode::UNAUTHORIZED,
            None,
            INVALID_REQUEST,
            "This MCP endpoint requires a bearer token".to_owned(),
        );
    }

    // 3. Parse.
    let request: JsonRpcRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(error) => {
            return rejection(
                StatusCode::BAD_REQUEST,
                None,
                PARSE_ERROR,
                format!("Parse error: {error}"),
            );
        }
    };

    // A notification carries no id, so there is nothing to answer it with. The
    // core protocol defines none over this transport; accepting and discarding
    // is the specified behaviour.
    if request.is_notification() {
        return (StatusCode::ACCEPTED, "").into_response();
    }
    let id = request.id.clone();

    // 4. Which era is this? Resolving it before validation is what lets one
    //    endpoint serve both: the mirrored headers the modern revision requires
    //    did not exist in the older one, so demanding them of every request
    //    would refuse every client that still opens with `initialize`.
    let era = match resolve_era(&headers, &request) {
        Ok(era) => era,
        Err(declared) => {
            return (
                StatusCode::BAD_REQUEST,
                axum::Json(JsonRpcResponse::error_with_data(
                    id,
                    UNSUPPORTED_PROTOCOL_VERSION,
                    format!("Unsupported protocol version: {declared}"),
                    Some(serde_json::json!({ "supported": supported_versions() })),
                )),
            )
                .into_response();
        }
    };

    // 5. The mirrored headers must agree with the body — a load balancer
    //    routing on the header while this server executes the body is exactly
    //    the split this check exists to prevent. Modern only: the older
    //    revisions never sent `Mcp-Method` or `Mcp-Name` at all.
    if era == Era::Modern {
        if let Err(message) = validate_headers(&headers, &request) {
            return rejection(StatusCode::BAD_REQUEST, id, HEADER_MISMATCH, message);
        }
    }

    debug!("MCP request: method={} era={era:?}", request.method);

    // 6. Dispatch. Every tool this server exposes answers from the local
    //    database or from a cached renderer list, so none of them is slow
    //    enough to need an SSE response stream; a single JSON body is the
    //    whole response.
    match handle_method(&state, &request, era).await {
        Some(response) => (StatusCode::OK, axum::Json(response)).into_response(),
        None => rejection(
            StatusCode::NOT_FOUND,
            request.id.clone(),
            METHOD_NOT_FOUND,
            format!("Method not found: {}", request.method),
        ),
    }
}

/// Decide which revision of the protocol a request is speaking.
///
/// Returns the offending version string when it is one this server does not
/// implement, so the caller can answer with the list that it does.
fn resolve_era(headers: &HeaderMap, request: &JsonRpcRequest) -> Result<Era, String> {
    // The body's `_meta` is the modern era's own declaration and outranks the
    // header, which merely mirrors it.
    if let Some(declared) = request.declared_protocol_version() {
        return if declared == PROTOCOL_VERSION {
            Ok(Era::Modern)
        } else {
            Err(declared.to_owned())
        };
    }

    match headers
        .get(HEADER_PROTOCOL_VERSION)
        .and_then(|value| value.to_str().ok())
    {
        Some(PROTOCOL_VERSION) => Ok(Era::Modern),
        Some(version) if LEGACY_PROTOCOL_VERSIONS.contains(&version) => Ok(Era::Legacy),
        Some(version) => Err(version.to_owned()),
        // `initialize` is how the older era opens, and it predates the header
        // entirely — there is nothing else it could be.
        None if request.method == "initialize" => Ok(Era::Legacy),
        // A request with neither is from a client that has not identified
        // itself at all. Treating it as modern gives it the header-mismatch
        // error naming exactly what is missing.
        None => Ok(Era::Modern),
    }
}

/// Whether the request's `Origin`, if it has one, belongs to this server.
///
/// [`crate::web::auth::AuthState::origin_valid`] answers a different question —
/// it guards cookie-authenticated writes, where a missing `Origin` is itself
/// suspicious. Here a missing `Origin` is the normal case, because the clients
/// are not browsers.
fn origin_permitted(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return true;
    };
    let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    origin == format!("http://{host}") || origin == format!("https://{host}")
}

fn validate_headers(headers: &HeaderMap, request: &JsonRpcRequest) -> Result<(), String> {
    let value = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());

    let Some(version) = value(HEADER_PROTOCOL_VERSION) else {
        return Err(format!("Missing required header {HEADER_PROTOCOL_VERSION}"));
    };
    if let Some(declared) = request.declared_protocol_version() {
        if version != declared {
            return Err(format!(
                "Header mismatch: {HEADER_PROTOCOL_VERSION} is '{version}' but the body declares '{declared}'"
            ));
        }
    }

    let Some(method) = value(HEADER_METHOD) else {
        return Err(format!("Missing required header {HEADER_METHOD}"));
    };
    if method != request.method {
        return Err(format!(
            "Header mismatch: {HEADER_METHOD} is '{method}' but the body method is '{}'",
            request.method
        ));
    }

    if let Some(expected) = request.mirrored_name() {
        let Some(name) = value(HEADER_NAME) else {
            return Err(format!("Missing required header {HEADER_NAME}"));
        };
        if !header_matches(name, expected) {
            return Err(format!(
                "Header mismatch: {HEADER_NAME} does not match the body value '{expected}'"
            ));
        }
    }

    Ok(())
}

/// A transport-level refusal: an HTTP status the client can act on, with a
/// JSON-RPC error in the body so it can tell this apart from a proxy's 404.
fn rejection(
    status: StatusCode,
    id: Option<serde_json::Value>,
    code: i64,
    message: String,
) -> axum::response::Response {
    (
        status,
        axum::Json(JsonRpcResponse::error(id, code, message)),
    )
        .into_response()
}

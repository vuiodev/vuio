//! The Model Context Protocol server.
//!
//! One endpoint — `POST /mcp` — speaking revision `2026-07-28`. That revision
//! is stateless: no handshake, no session id, no server-initiated requests. So
//! there is nothing here but a request handler, a method table and the tools,
//! and none of it holds state between calls.
//!
//! The tools themselves are transport-agnostic: they take `&AppState` and JSON
//! arguments and return JSON, which is what lets `vuio mcp` proxy the same
//! catalog over stdio.

use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
};
use tracing::debug;

use crate::web::format::format_bytes;
use crate::{
    database::{
        DatabaseManager, DatabaseReadSession, DirectoryView, MediaFileQuery, MediaFileView,
    },
    state::AppState,
};

mod protocol;
pub use protocol::{JsonRpcRequest, JsonRpcResponse};
use protocol::{
    header_matches, negotiated_legacy_version, server_info, supported_versions, Era,
    HEADER_MISMATCH, INVALID_PARAMS, INVALID_REQUEST, LEGACY_PROTOCOL_VERSIONS, METHOD_NOT_FOUND,
    PARSE_ERROR, PROTOCOL_VERSION, TOOLS_LIST_TTL_MS, UNSUPPORTED_PROTOCOL_VERSION,
};

mod catalog;
mod dispatch;
mod tools;
mod transport;

use catalog::*;
use dispatch::*;
pub use transport::{mcp_handler, method_not_allowed};
// The cast helpers live with casting and call back for path resolution, so the
// containment check has one implementation rather than one per caller.
#[cfg(feature = "casting")]
pub(crate) use tools::canonical_media_path;

#[cfg(test)]
mod tests;

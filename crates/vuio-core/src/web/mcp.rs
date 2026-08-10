use axum::{
    extract::{ConnectInfo, Json, Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
};
use futures_util::StreamExt;
use serde::Deserialize;
use std::{
    convert::Infallible,
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::web::format::format_bytes;
use crate::{
    casting::{PlaybackAction, PlaybackItem, PlaybackState},
    database::{
        DatabaseManager, DatabaseReadSession, DirectoryView, FileLocation, MediaFileQuery,
        MediaFileView,
    },
    state::{AppState, McpClient},
};

const MCP_MAX_CLIENTS: usize = 64;
const MCP_MAX_CLIENTS_PER_PEER: usize = 4;
const MCP_CLIENT_TTL: Duration = Duration::from_secs(30 * 60);
const MCP_MAX_RESPONSE_BYTES: usize = 1024 * 1024;

mod protocol;
pub use protocol::{JsonRpcRequest, JsonRpcResponse};

// ──────────────────────────────────────────
// JSON-RPC 2.0 types
// ──────────────────────────────────────────

// ──────────────────────────────────────────
// MCP tool definitions
// ──────────────────────────────────────────
mod catalog;
mod dispatch;
mod tools;
mod transport;

use catalog::*;
use dispatch::*;
pub use tools::{cast_file_helper, cast_playlist_helper, cast_tracks_helper};
// MessageQuery is exercised by the MCP integration tests.
#[allow(unused_imports)]
pub use transport::{message_handler, sse_handler, MessageQuery};

#[cfg(test)]
mod tests;

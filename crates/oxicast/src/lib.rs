#![warn(missing_docs)]
#![deny(unsafe_code)]

//! # oxicast
//!
//! Async Google Cast (Chromecast) client for Rust, built on [tokio](https://tokio.rs).
//!
//! Discover, connect to, and control Cast devices. Handles TLS, heartbeats,
//! reconnection, and request-response correlation automatically.
//!
//! ## Quick Start
//!
//! ```no_run
//! use oxicast::{CastClient, CastApp, MediaInfo};
//! use std::time::Duration;
//!
//! # async fn example() -> oxicast::Result<()> {
//! // Connect by IP (discovery is optional)
//! let client = CastClient::connect("192.168.1.100", 8009).await?;
//!
//! // Launch an app and play media
//! client.launch_app(&CastApp::DefaultMediaReceiver).await?;
//! client.load_media(
//!     &MediaInfo::new("https://example.com/video.mp4", "video/mp4"),
//!     true,
//!     0.0,
//!     None, // custom_data for Custom Web Receiver
//! ).await?;
//!
//! // Control playback
//! client.pause().await?;
//! client.seek(60.0).await?;
//! client.play().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! Three background tokio tasks handle the Cast protocol:
//! - A **reader task** decodes inbound messages and dispatches them
//! - A **writer task** serializes outbound commands without blocking reads
//! - A **heartbeat task** sends PING and detects connection loss
//!
//! Commands respond instantly — they never wait for the next heartbeat cycle.
//!
//! ## Consuming status updates
//!
//! Two options — use whichever fits your architecture:
//!
//! - [`CastClient::next_event()`] — event stream in a `tokio::select!` loop.
//!   Bounded channel; events are dropped (not blocked) if the buffer fills.
//! - [`CastClient::watch_media_status()`] / [`CastClient::watch_receiver_status()`]
//!   — always-fresh latest state via `tokio::sync::watch`. No draining needed.
//!
//! ## Feature flags
//!
//! - **`discovery`** (default) — mDNS device scanning via [`discovery`] module
//! - **`serve`** — HTTP file server for casting local files via the `serve` module

pub(crate) mod channel;
pub(crate) mod client;
pub mod error;
pub mod event;
pub(crate) mod state;
pub mod types;

// Re-export test utilities for integration tests
#[cfg(feature = "testing")]
#[doc(hidden)]
pub mod __test_util {
    pub use crate::channel::ns;
    pub use crate::client::router::{
        parse_media_status_from_json, parse_receiver_status_from_json,
    };
}

mod proto {
    #![allow(dead_code)]
    include!("proto_gen.rs");
}

#[cfg(feature = "discovery")]
pub mod discovery;

#[cfg(feature = "serve")]
pub mod serve;

pub use client::CastClient;
pub use error::{Error, Result};
pub use event::CastEvent;
pub use types::{
    Application, CastApp, DeviceInfo, IdleReason, Image, MediaInfo, MediaMetadata, MediaStatus,
    PlayerState, QueueItem, ReceiverStatus, RepeatMode, StreamType, Volume,
};

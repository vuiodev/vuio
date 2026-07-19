//! Chromecast (Castv2) protocol support.
//!
//! Implements the Cast V2 protocol for controlling Chromecast devices:
//! TLS connection on port 8009, protobuf message framing, and JSON
//! payloads for receiver/media control.

pub mod client;
pub mod proto;

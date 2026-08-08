//! Cast protocol constants — namespaces, message types, IDs.
//!
//! All magic strings from the Cast v2 protocol in one place.

// ── Namespaces ───────────────────────────────────────────────

/// Virtual connection management.
pub const NS_CONNECTION: &str = "urn:x-cast:com.google.cast.tp.connection";
/// Heartbeat keep-alive.
pub const NS_HEARTBEAT: &str = "urn:x-cast:com.google.cast.tp.heartbeat";
/// Receiver/device control (apps, volume).
pub const NS_RECEIVER: &str = "urn:x-cast:com.google.cast.receiver";
/// Media playback control.
pub const NS_MEDIA: &str = "urn:x-cast:com.google.cast.media";
/// Device authentication.
#[allow(dead_code)]
pub const NS_DEVICE_AUTH: &str = "urn:x-cast:com.google.cast.tp.deviceauth";

// ── Sender / Receiver IDs ────────────────────────────────────

/// Default sender identifier.
pub const SENDER_ID: &str = "sender-0";
/// Default receiver identifier.
pub const RECEIVER_ID: &str = "receiver-0";

// ── Connection Messages ──────────────────────────────────────

/// Open a virtual connection.
pub const MSG_CONNECT: &str = "CONNECT";
/// Close a virtual connection.
pub const MSG_CLOSE: &str = "CLOSE";

// ── Heartbeat Messages ───────────────────────────────────────

/// Heartbeat ping.
pub const MSG_PING: &str = "PING";
/// Heartbeat pong.
pub const MSG_PONG: &str = "PONG";

// ── Receiver Messages ────────────────────────────────────────

/// Launch an application.
pub const MSG_LAUNCH: &str = "LAUNCH";
/// Stop an application.
pub const MSG_STOP: &str = "STOP";
/// Request receiver status.
pub const MSG_GET_STATUS: &str = "GET_STATUS";
/// Set device volume.
pub const MSG_SET_VOLUME: &str = "SET_VOLUME";
/// Receiver status response.
pub const MSG_RECEIVER_STATUS: &str = "RECEIVER_STATUS";

// ── Media Messages ───────────────────────────────────────────

/// Load media for playback.
pub const MSG_LOAD: &str = "LOAD";
/// Resume playback.
pub const MSG_PLAY: &str = "PLAY";
/// Pause playback.
pub const MSG_PAUSE: &str = "PAUSE";
/// Seek to a position.
pub const MSG_SEEK: &str = "SEEK";
/// Stop media playback.
pub const MSG_MEDIA_STOP: &str = "STOP";
/// Media status response.
pub const MSG_MEDIA_STATUS: &str = "MEDIA_STATUS";
/// Load failed error.
pub const MSG_LOAD_FAILED: &str = "LOAD_FAILED";
/// Load was cancelled.
pub const MSG_LOAD_CANCELLED: &str = "LOAD_CANCELLED";
/// Invalid request error.
pub const MSG_INVALID_REQUEST: &str = "INVALID_REQUEST";
/// Load a media queue.
pub const MSG_QUEUE_LOAD: &str = "QUEUE_LOAD";
/// Insert items into the queue.
pub const MSG_QUEUE_INSERT: &str = "QUEUE_INSERT";

// ── Well-Known App IDs ───────────────────────────────────────

/// Google Default Media Receiver.
pub const APP_DEFAULT_MEDIA_RECEIVER: &str = "CC1AD845";
/// Backdrop / ambient screen.
pub const APP_BACKDROP: &str = "E8C28D3C";
/// YouTube.
pub const APP_YOUTUBE: &str = "233637DE";

// ── Metadata Types (from Google Cast SDK) ────────────────────

/// Generic metadata type (0).
pub const METADATA_GENERIC: u64 = 0;
/// Movie metadata type (1).
pub const METADATA_MOVIE: u64 = 1;
/// TV show metadata type (2).
pub const METADATA_TV_SHOW: u64 = 2;
/// Music track metadata type (3).
pub const METADATA_MUSIC_TRACK: u64 = 3;
/// Photo metadata type (4).
pub const METADATA_PHOTO: u64 = 4;
/// Audiobook chapter metadata type (5).
pub const METADATA_AUDIOBOOK_CHAPTER: u64 = 5;

// ── User Agent ───────────────────────────────────────────────

/// User-agent string sent in CONNECT messages.
pub const USER_AGENT: &str = "oxicast";

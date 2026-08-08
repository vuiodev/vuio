//! Shared types for Cast protocol communication.

use serde::{Deserialize, Serialize};

/// Information about a discovered Cast device on the local network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Human-readable device name (e.g., "Living Room TV").
    pub name: String,
    /// The device's IP address.
    pub ip: std::net::IpAddr,
    /// The Cast protocol port (typically 8009).
    pub port: u16,
    /// Device model name (e.g., "Chromecast", "Google Home").
    pub model: Option<String>,
    /// Device UUID.
    pub uuid: Option<String>,
}

impl DeviceInfo {
    /// Connect to this device with default settings.
    ///
    /// Convenience for `CastClient::connect(&device.ip.to_string(), device.port)`.
    pub async fn connect(&self) -> crate::Result<crate::CastClient> {
        crate::CastClient::connect(&self.ip.to_string(), self.port).await
    }
}

/// Volume state of a Cast device.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Volume {
    /// Volume level from 0.0 (silent) to 1.0 (maximum).
    pub level: f32,
    /// Whether the device is muted.
    pub muted: bool,
}

/// Media stream type, sent in the LOAD command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum StreamType {
    /// Unknown or unspecified.
    None,
    /// On-demand content with a known duration (movies, episodes).
    Buffered,
    /// Live content with no fixed end (live TV, radio).
    Live,
}

/// Player state as reported by the Cast device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlayerState {
    /// No media is loaded or the session has ended.
    Idle,
    /// Media is actively playing.
    Playing,
    /// Playback is paused.
    Paused,
    /// The device is buffering data before it can play.
    Buffering,
}

/// The reason the player entered the Idle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IdleReason {
    /// The user or sender cancelled playback.
    Cancelled,
    /// Playback was interrupted (e.g., by loading new media).
    Interrupted,
    /// The media played to completion.
    Finished,
    /// An error occurred during playback.
    Error,
}

/// Description of media to be loaded onto the Cast device.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    /// The URL or content identifier.
    pub content_id: String,
    /// MIME type (e.g., "video/mp4", "application/x-mpegURL").
    pub content_type: String,
    /// Whether this is buffered (VOD) or live content.
    pub stream_type: StreamType,
    /// Total duration in seconds (if known).
    pub duration: Option<f64>,
    /// Optional metadata (title, images, etc.).
    pub metadata: Option<MediaMetadata>,
}

impl MediaInfo {
    /// Create a new `MediaInfo` with the minimum required fields.
    #[must_use]
    pub fn new(content_id: impl Into<String>, content_type: impl Into<String>) -> Self {
        Self {
            content_id: content_id.into(),
            content_type: content_type.into(),
            stream_type: StreamType::Buffered,
            duration: None,
            metadata: None,
        }
    }

    /// Set the stream type.
    pub fn stream_type(mut self, stream_type: StreamType) -> Self {
        self.stream_type = stream_type;
        self
    }

    /// Set the duration in seconds.
    pub fn duration(mut self, duration: f64) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Set the duration if `Some`, no-op if `None`.
    ///
    /// Avoids the `let mut` + `if let` dance when the duration is optional:
    /// ```
    /// # use oxicast::MediaInfo;
    /// let dur: Option<f64> = Some(120.0);
    /// let media = MediaInfo::new("url", "video/mp4").maybe_duration(dur);
    /// ```
    pub fn maybe_duration(self, duration: Option<f64>) -> Self {
        match duration {
            Some(d) => self.duration(d),
            None => self,
        }
    }

    /// Set the metadata.
    pub fn metadata(mut self, metadata: MediaMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set the metadata if `Some`, no-op if `None`.
    pub fn maybe_metadata(self, metadata: Option<MediaMetadata>) -> Self {
        match metadata {
            Some(m) => self.metadata(m),
            None => self,
        }
    }
}

/// Metadata about the media being played.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum MediaMetadata {
    /// Generic metadata.
    Generic {
        /// Title of the media.
        title: Option<String>,
        /// Subtitle of the media.
        subtitle: Option<String>,
        /// Associated images.
        images: Vec<Image>,
    },
    /// Movie metadata.
    Movie {
        /// Movie title.
        title: Option<String>,
        /// Subtitle or tagline.
        subtitle: Option<String>,
        /// Production studio.
        studio: Option<String>,
        /// Associated images.
        images: Vec<Image>,
    },
    /// TV show episode metadata.
    TvShow {
        /// Series title.
        series_title: Option<String>,
        /// Episode title.
        episode_title: Option<String>,
        /// Season number.
        season: Option<u32>,
        /// Episode number.
        episode: Option<u32>,
        /// Associated images.
        images: Vec<Image>,
    },
    /// Music track metadata.
    MusicTrack {
        /// Track title.
        title: Option<String>,
        /// Artist name.
        artist: Option<String>,
        /// Album name.
        album_name: Option<String>,
        /// Composer name.
        composer: Option<String>,
        /// Track number on the album.
        track_number: Option<u32>,
        /// Disc number.
        disc_number: Option<u32>,
        /// Associated images.
        images: Vec<Image>,
    },
    /// Photo metadata.
    Photo {
        /// Photo title.
        title: Option<String>,
        /// Photographer or artist.
        artist: Option<String>,
        /// Location where the photo was taken.
        location: Option<String>,
        /// Latitude coordinate.
        latitude: Option<f64>,
        /// Longitude coordinate.
        longitude: Option<f64>,
        /// Image width in pixels.
        width: Option<u32>,
        /// Image height in pixels.
        height: Option<u32>,
        /// Associated images.
        images: Vec<Image>,
    },
    /// Audiobook chapter metadata.
    AudiobookChapter {
        /// Book title.
        book_title: Option<String>,
        /// Chapter title.
        chapter_title: Option<String>,
        /// Chapter number.
        chapter_number: Option<u32>,
        /// Subtitle.
        subtitle: Option<String>,
        /// Associated images.
        images: Vec<Image>,
    },
}

/// An image (poster, thumbnail, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    /// The image URL.
    pub url: String,
    /// Image width in pixels.
    pub width: Option<u32>,
    /// Image height in pixels.
    pub height: Option<u32>,
}

/// A snapshot of the current media playback state.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaStatus {
    /// The media session identifier.
    pub media_session_id: i32,
    /// Current player state.
    pub player_state: PlayerState,
    /// Why the player is idle (if applicable).
    pub idle_reason: Option<IdleReason>,
    /// Current playback position in seconds.
    pub current_time: f64,
    /// Total duration in seconds (if known).
    pub duration: Option<f64>,
    /// Current volume state.
    pub volume: Volume,
    /// The media that is loaded (if any).
    pub media: Option<MediaInfo>,
}

/// Status of the Cast receiver device.
#[derive(Debug, Clone, PartialEq)]
pub struct ReceiverStatus {
    /// Current volume state.
    pub volume: Volume,
    /// Running applications.
    pub applications: Vec<Application>,
    /// Whether the device has an active input source.
    pub is_active_input: bool,
    /// Whether the device is in standby mode.
    pub is_stand_by: bool,
}

/// A running application on the Cast device.
#[derive(Debug, Clone, PartialEq)]
pub struct Application {
    /// The application ID (e.g., "CC1AD845" for Default Media Receiver).
    pub app_id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// The session ID for this app instance.
    pub session_id: String,
    /// The transport ID for sending messages to this app.
    pub transport_id: String,
    /// Namespaces supported by this app.
    pub namespaces: Vec<String>,
    /// Current status text.
    pub status_text: String,
}

/// Well-known Cast application identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CastApp {
    /// Google's Default Media Receiver — plays HLS, MP4, WebM, etc.
    DefaultMediaReceiver,
    /// The backdrop/ambient screen.
    Backdrop,
    /// YouTube.
    YouTube,
    /// A custom application by ID.
    Custom(String),
}

impl CastApp {
    /// Get the application ID string.
    pub fn app_id(&self) -> &str {
        match self {
            CastApp::DefaultMediaReceiver => crate::channel::ns::APP_DEFAULT_MEDIA_RECEIVER,
            CastApp::Backdrop => crate::channel::ns::APP_BACKDROP,
            CastApp::YouTube => crate::channel::ns::APP_YOUTUBE,
            CastApp::Custom(id) => id,
        }
    }
}

/// Queue repeat mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum RepeatMode {
    /// No repeat.
    RepeatOff,
    /// Repeat the entire queue.
    RepeatAll,
    /// Repeat the current item.
    RepeatSingle,
    /// Repeat all and shuffle.
    RepeatAllAndShuffle,
}

/// An item in a media queue.
#[derive(Debug, Clone, PartialEq)]
pub struct QueueItem {
    /// The media to play.
    pub media: MediaInfo,
    /// Whether to start playing automatically.
    pub autoplay: bool,
    /// Start position in seconds.
    pub start_time: f64,
}

// ── Display Implementations ────────────────────────────────────

impl std::fmt::Display for PlayerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayerState::Idle => write!(f, "idle"),
            PlayerState::Playing => write!(f, "playing"),
            PlayerState::Paused => write!(f, "paused"),
            PlayerState::Buffering => write!(f, "buffering"),
        }
    }
}

impl std::fmt::Display for IdleReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdleReason::Cancelled => write!(f, "cancelled"),
            IdleReason::Interrupted => write!(f, "interrupted"),
            IdleReason::Finished => write!(f, "finished"),
            IdleReason::Error => write!(f, "error"),
        }
    }
}

impl std::fmt::Display for StreamType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamType::None => write!(f, "none"),
            StreamType::Buffered => write!(f, "buffered"),
            StreamType::Live => write!(f, "live"),
        }
    }
}

impl std::fmt::Display for CastApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CastApp::DefaultMediaReceiver => write!(f, "Default Media Receiver"),
            CastApp::Backdrop => write!(f, "Backdrop"),
            CastApp::YouTube => write!(f, "YouTube"),
            CastApp::Custom(id) => write!(f, "Custom({id})"),
        }
    }
}

// ── Convenience Constructors ───────────────────────────────────

impl Volume {
    /// Create a new volume with muted=false. Level is clamped to [0.0, 1.0].
    #[must_use]
    pub fn new(level: f32) -> Self {
        Self { level: level.clamp(0.0, 1.0), muted: false }
    }

    /// Create a muted volume at level 0.
    ///
    /// Note: On real Cast devices, muting preserves the current level.
    /// Use `set_muted(true)` on the client to mute without changing the level.
    #[must_use]
    pub fn muted() -> Self {
        Self { level: 0.0, muted: true }
    }
}

impl MediaInfo {
    /// Create a movie `MediaInfo` with title metadata.
    #[must_use]
    pub fn movie(
        content_id: impl Into<String>,
        content_type: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        Self {
            content_id: content_id.into(),
            content_type: content_type.into(),
            stream_type: StreamType::Buffered,
            duration: None,
            metadata: Some(MediaMetadata::Movie {
                title: Some(title.into()),
                subtitle: None,
                studio: None,
                images: vec![],
            }),
        }
    }

    /// Create a live stream `MediaInfo`.
    #[must_use]
    pub fn live(content_id: impl Into<String>, content_type: impl Into<String>) -> Self {
        Self {
            content_id: content_id.into(),
            content_type: content_type.into(),
            stream_type: StreamType::Live,
            duration: None,
            metadata: None,
        }
    }
}

//! Fetching titles, synopses, ratings and artwork from public metadata APIs.
//!
//! Everything local metadata cannot supply lives here. Symphonia reads whatever an
//! audio file carries in its tags, but a video file carries nothing VuIO reads at
//! all — a movie is a filename until something looks it up — and even a well-tagged
//! album has no synopsis or cover unless the artwork happens to sit next to it on
//! disk.
//!
//! This is the only part of VuIO that talks to anything off the local network, and
//! the split in this module is drawn around that fact: [`provider`] is static data
//! and always compiles, while everything that opens a socket is behind the
//! `mediainfo` feature. A build without the feature still parses a `[mediainfo]`
//! config section and still knows what a provider id means; it just has no way to
//! act on either.

pub mod provider;

// `DEFAULT_PROVIDER_IDS` is needed by the config layer either way; the lookup is
// only reached by the endpoints, which the feature gates.
pub use provider::DEFAULT_PROVIDER_IDS;
#[cfg(feature = "mediainfo")]
pub use provider::provider_info;

// Submodules stay public rather than being re-exported item by item: the module
// is already crate-internal, and the integration tests reach the parser and the
// scorer by path. Only the handful of names used elsewhere in the crate are
// lifted to the top.
#[cfg(feature = "mediainfo")]
pub mod artwork;
#[cfg(feature = "mediainfo")]
pub mod client;
#[cfg(feature = "mediainfo")]
pub mod credentials;
#[cfg(feature = "mediainfo")]
pub mod env_keys;
#[cfg(feature = "mediainfo")]
pub mod job;
#[cfg(feature = "mediainfo")]
pub mod matching;
#[cfg(feature = "mediainfo")]
mod providers;
#[cfg(feature = "mediainfo")]
mod rate_limit;

#[cfg(feature = "mediainfo")]
pub use artwork::{content_type_for as artwork_content_type, ArtworkCache};
#[cfg(feature = "mediainfo")]
pub use credentials::CredentialStore;
#[cfg(feature = "mediainfo")]
pub use job::{run_library_fetch, MediaInfoJobState};

/// Bumping this marks every stored row stale, so a later run re-fetches instead of
/// skipping what it already has. The same lever as `TAGS_VERSION` for local tags:
/// raise it when the matching or the fields we keep change enough that old rows are
/// worth discarding.
pub const MEDIAINFO_VERSION: u32 = 1;

pub mod provider;

pub use provider::DEFAULT_PROVIDER_IDS;
#[cfg(feature = "mediainfo")]
#[allow(unused_imports)]
pub use provider::provider_info;

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
#[allow(unused_imports)]
pub use artwork::{content_type_for as artwork_content_type, ArtworkCache};
#[cfg(feature = "mediainfo")]
#[allow(unused_imports)]
pub use credentials::CredentialStore;
#[cfg(feature = "mediainfo")]
#[allow(unused_imports)]
pub use job::{run_library_fetch, MediaInfoJobState};

pub const MEDIAINFO_VERSION: u32 = 1;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use thiserror::Error;
use tokio::fs;
use tracing::debug;

use crate::database::MediaFile;

#[cfg(target_os = "windows")]
pub mod windows;

/// Path normalization trait for consistent path handling across platforms
mod manager;
#[cfg(feature = "metadata")]
mod metadata;
mod normalization;

pub use manager::*;
#[cfg(feature = "metadata")]
pub(crate) use metadata::*;
pub use normalization::*;

/// Without the `metadata` feature nothing reads tags, so a scanned file keeps
/// the title derived from its filename and carries no artist, album or
/// duration. Indexing and streaming are unaffected, which is why this is a
/// silent no-op rather than an error.
#[cfg(not(feature = "metadata"))]
pub(crate) async fn extract_audio_metadata(
    _media_file: &mut MediaFile,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Ok(())
}

/// Likewise for video: with no probe there is no codec to record, and the
/// browse path simply never offers a decoded alternative for a film.
#[cfg(not(feature = "metadata"))]
pub(crate) async fn extract_stream_info(
    _media_file: &mut MediaFile,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Ok(())
}

/// Records written without a tag reader carry version 0, so enabling the
/// feature later re-reads them on the next scan.
#[cfg(not(feature = "metadata"))]
pub(crate) const TAGS_VERSION: u32 = 0;

#[cfg(test)]
mod tests;

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
mod metadata;
mod normalization;

pub use manager::*;
pub(crate) use metadata::*;
pub use normalization::*;

#[cfg(test)]
mod tests;

use anyhow::Result;
use futures_util::StreamExt as _;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{debug, info, warn};

use crate::config::{AppConfig, MonitoredDirectoryConfig};
use crate::database::{ActiveDatabase, DatabaseManager, FileFingerprint, MediaFile};
use crate::platform::filesystem::{create_platform_filesystem_manager, FileSystemManager};

/// Batch size for database operations during parallel scanning
const BATCH_SIZE: usize = 1000;

/// Cheap identity for the current contents of a file.
///
/// File size plus whole-second mtime is not enough: replacing a file with
/// another of the same size inside one timestamp second reused cached indexes,
/// HLS segments, and browser URLs. `marker` folds the highest-resolution times
/// the platform exposes, and on Unix also the inode and change time. Change
/// time still moves when a copying tool deliberately restores the mtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ContentVersion {
    pub size: u64,
    pub marker: i64,
}

impl ContentVersion {
    pub(crate) async fn for_file(path: &Path) -> Option<Self> {
        let metadata = tokio::fs::metadata(path).await.ok()?;
        let mut marker = 0xcbf2_9ce4_8422_2325u64;
        let mut mix = |value: u64| {
            for byte in value.to_le_bytes() {
                marker ^= u64::from(byte);
                marker = marker.wrapping_mul(0x1000_0000_01b3);
            }
        };

        mix(metadata.len());
        if let Ok(modified) = metadata.modified() {
            match modified.duration_since(std::time::UNIX_EPOCH) {
                Ok(value) => {
                    mix(value.as_secs());
                    mix(u64::from(value.subsec_nanos()));
                }
                Err(value) => {
                    mix(value.duration().as_secs());
                    mix(u64::from(value.duration().subsec_nanos()) | (1 << 63));
                }
            }
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            mix(metadata.dev());
            mix(metadata.ino());
            mix(metadata.ctime() as u64);
            mix(metadata.ctime_nsec() as u64);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            mix(metadata.creation_time());
            mix(metadata.last_write_time());
            mix(u64::from(metadata.file_attributes()));
        }
        #[cfg(not(any(unix, windows)))]
        if let Ok(created) = metadata.created() {
            if let Ok(value) = created.duration_since(std::time::UNIX_EPOCH) {
                mix(value.as_secs());
                mix(u64::from(value.subsec_nanos()));
            }
        }

        Some(Self {
            size: metadata.len(),
            marker: marker as i64,
        })
    }

    pub(crate) fn token(self) -> String {
        format!("{:x}-{:016x}", self.size, self.marker as u64)
    }
}

/// Immutable rules for one configured media root.  The same value is shared by
/// startup scans, reconciliation and watcher filtering so those paths cannot
/// disagree about what belongs in the catalog.
mod policy;
pub mod remux;
mod result;
mod scanner;
#[cfg(feature = "transcode")]
pub mod transcode;

pub use policy::ScanPolicy;
#[cfg(test)]
use policy::{path_components_equal, swap_one_ascii_case};
pub use result::{ScanError, ScanResult};
pub use scanner::MediaScanner;

#[cfg(test)]
mod tests;

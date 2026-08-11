use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{debug, info, warn};

use crate::config::{AppConfig, MonitoredDirectoryConfig};
use crate::database::{redb::RedbDatabase, DatabaseManager, FileFingerprint, MediaFile};
use crate::platform::filesystem::{create_platform_filesystem_manager, FileSystemManager};

/// Batch size for database operations during parallel scanning
const BATCH_SIZE: usize = 1000;

/// Immutable rules for one configured media root.  The same value is shared by
/// startup scans, reconciliation and watcher filtering so those paths cannot
/// disagree about what belongs in the catalog.
mod policy;
pub mod remux;
mod result;
mod scanner;

pub use policy::ScanPolicy;
#[cfg(test)]
use policy::{path_components_equal, swap_one_ascii_case};
pub use result::{ScanError, ScanResult};
pub use scanner::MediaScanner;

#[cfg(test)]
mod tests;

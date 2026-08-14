use super::*;

/// Result of a media scanning operation
///
/// Every outcome is a count. The records themselves live in the database by the
/// time a scan returns, and nothing has ever read them back off this struct —
/// `.len()` and `.is_empty()` were the only consumers. Retaining them meant a
/// whole [`MediaFile`] per added file, which on a large library is most of the
/// index built and dropped for a log line, and worse on real music where
/// `extra_tags` is populated.
///
/// [`MediaFile`]: crate::database::MediaFile
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// How many files were newly added to the database.
    pub new: usize,

    /// How many files were updated in the database.
    pub updated: usize,

    /// How many files were removed from the database.
    pub removed: usize,

    /// How many files were found to be unchanged.
    pub unchanged: usize,

    /// Total number of files scanned from the file system
    pub total_scanned: usize,

    /// How many files this scan actually opened and read.
    ///
    /// Distinct from `total_scanned`, which counts everything the walk saw. A
    /// scan of a library that has not changed should read nothing: each file
    /// costs one `stat`, and the difference between the two numbers is the work
    /// avoided.
    pub files_read: usize,

    /// Errors encountered during scanning
    pub errors: Vec<ScanError>,

    /// True only when the whole requested root was enumerated without uncertainty.
    pub complete: bool,
}

impl ScanResult {
    /// Create a new empty scan result
    pub fn new() -> Self {
        Self {
            new: 0,
            updated: 0,
            removed: 0,
            unchanged: 0,
            total_scanned: 0,
            files_read: 0,
            errors: Vec::new(),
            complete: true,
        }
    }

    /// Merge another scan result into this one
    pub fn merge(&mut self, other: ScanResult) {
        self.new += other.new;
        self.updated += other.updated;
        self.removed += other.removed;
        self.unchanged += other.unchanged;
        self.total_scanned += other.total_scanned;
        self.files_read += other.files_read;
        self.errors.extend(other.errors);
        self.complete &= other.complete;
    }

    /// Get the total number of changes (new + updated + removed)
    pub fn total_changes(&self) -> usize {
        self.new + self.updated + self.removed
    }

    /// Get a summary string of the scan results
    pub fn summary(&self) -> String {
        format!(
            "Scanned {} files ({} read): {} new, {} updated, {} removed, {} unchanged, {} errors",
            self.total_scanned,
            self.files_read,
            self.new,
            self.updated,
            self.removed,
            self.unchanged,
            self.errors.len()
        )
    }
}

/// Error that occurred during scanning
#[derive(Debug, Clone)]
pub struct ScanError {
    /// Path where the error occurred
    pub path: PathBuf,

    /// Error description
    pub error: String,
}

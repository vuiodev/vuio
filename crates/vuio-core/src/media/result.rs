use super::*;

/// Result of a media scanning operation
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// Files that were newly added to the database
    pub new_files: Vec<MediaFile>,

    /// Files that were updated in the database
    pub updated_files: Vec<MediaFile>,

    /// Files that were removed from the database
    pub removed_files: Vec<FileFingerprint>,

    /// Files that were unchanged
    pub unchanged_files: Vec<FileFingerprint>,

    /// Total number of files scanned from the file system
    pub total_scanned: usize,

    /// Errors encountered during scanning
    pub errors: Vec<ScanError>,

    /// True only when the whole requested root was enumerated without uncertainty.
    pub complete: bool,
}

impl ScanResult {
    /// Create a new empty scan result with pre-allocated capacity
    pub fn new() -> Self {
        Self {
            new_files: Vec::with_capacity(100),
            updated_files: Vec::with_capacity(50),
            removed_files: Vec::with_capacity(50),
            unchanged_files: Vec::with_capacity(1000),
            total_scanned: 0,
            errors: Vec::with_capacity(10),
            complete: true,
        }
    }

    /// Merge another scan result into this one
    pub fn merge(&mut self, other: ScanResult) {
        self.new_files.extend(other.new_files);
        self.updated_files.extend(other.updated_files);
        self.removed_files.extend(other.removed_files);
        self.unchanged_files.extend(other.unchanged_files);
        self.total_scanned += other.total_scanned;
        self.errors.extend(other.errors);
        self.complete &= other.complete;
    }

    /// Get the total number of changes (new + updated + removed)
    pub fn total_changes(&self) -> usize {
        self.new_files.len() + self.updated_files.len() + self.removed_files.len()
    }


    /// Get a summary string of the scan results
    pub fn summary(&self) -> String {
        format!(
            "Scanned {} files: {} new, {} updated, {} removed, {} unchanged, {} errors",
            self.total_scanned,
            self.new_files.len(),
            self.updated_files.len(),
            self.removed_files.len(),
            self.unchanged_files.len(),
            self.errors.len()
        )
    }
}

impl Default for ScanResult {
    fn default() -> Self {
        Self::new()
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

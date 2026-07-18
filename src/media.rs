use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{debug, info, warn};

use crate::database::{redb::RedbDatabase, DatabaseManager, FileFingerprint, MediaFile};
use crate::platform::filesystem::{create_platform_filesystem_manager, FileSystemManager};

/// Batch size for database operations during parallel scanning
const BATCH_SIZE: usize = 1000;

/// Media scanner that uses the file system manager and database for efficient scanning
pub struct MediaScanner<D: DatabaseManager = RedbDatabase> {
    filesystem_manager: Box<dyn FileSystemManager>,
    database_manager: Arc<D>,
}

impl<D: DatabaseManager> MediaScanner<D> {
    fn fingerprint(file: &MediaFile) -> FileFingerprint {
        FileFingerprint {
            id: file.id.unwrap_or_default(),
            path: file.path.clone(),
            size: file.size,
            modified: file.modified,
            created_at: file.created_at,
        }
    }

    /// Create a new media scanner with database manager
    pub fn with_database(database_manager: Arc<D>) -> Self {
        Self {
            filesystem_manager: create_platform_filesystem_manager(),
            database_manager,
        }
    }

    /// Simple directory scan that returns files without database operations
    pub async fn scan_directory_simple(&self, directory: &Path) -> Result<Vec<MediaFile>> {
        // Use canonical path normalization for consistency
        let canonical_dir = match self.filesystem_manager.get_canonical_path(directory) {
            Ok(canonical) => PathBuf::from(canonical),
            Err(e) => {
                tracing::warn!(
                    "Failed to get canonical path for {}: {}, using basic normalization",
                    directory.display(),
                    e
                );
                self.filesystem_manager.normalize_path(directory)
            }
        };

        // Validate the directory path
        self.filesystem_manager.validate_path(&canonical_dir)?;

        if !self.filesystem_manager.is_accessible(&canonical_dir).await {
            return Err(anyhow::anyhow!(
                "Directory is not accessible: {}",
                canonical_dir.display()
            ));
        }

        // Scan the file system for current files
        let fs_files = self
            .filesystem_manager
            .scan_media_directory(&canonical_dir)
            .await
            .map_err(|e| anyhow::anyhow!("File system scan failed: {}", e))?;

        Ok(fs_files)
    }

    /// Create a media scanner with a custom file system manager (for testing)
    pub fn with_filesystem_manager(
        filesystem_manager: Box<dyn FileSystemManager>,
        database_manager: Arc<D>,
    ) -> Self {
        Self {
            filesystem_manager,
            database_manager,
        }
    }

    /// Perform a full scan of a directory, updating the database with new/changed files
    pub async fn scan_directory(&self, directory: &Path) -> Result<ScanResult> {
        self.scan_directory_with_existing_files(directory, None)
            .await
    }

    /// Internal method that allows passing existing files to avoid repeated database queries during recursive scans
    async fn scan_directory_with_existing_files(
        &self,
        directory: &Path,
        all_existing_files: Option<&[MediaFile]>,
    ) -> Result<ScanResult> {
        // Use canonical path normalization for consistency
        let canonical_dir = match self.filesystem_manager.get_canonical_path(directory) {
            Ok(canonical) => PathBuf::from(canonical),
            Err(e) => {
                tracing::warn!(
                    "Failed to get canonical path for {}: {}, using basic normalization",
                    directory.display(),
                    e
                );
                self.filesystem_manager.normalize_path(directory)
            }
        };

        // Validate the directory path
        self.filesystem_manager.validate_path(&canonical_dir)?;

        if !self.filesystem_manager.is_accessible(&canonical_dir).await {
            return Err(anyhow::anyhow!(
                "Directory is not accessible: {}",
                canonical_dir.display()
            ));
        }

        // Get existing files from database for this directory
        let existing_files = if let Some(all_files) = all_existing_files {
            // Filter existing files to only those in this directory
            all_files
                .iter()
                .filter(|file| {
                    let file_parent = file
                        .path
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new(""));
                    file_parent == canonical_dir
                })
                .cloned()
                .collect()
        } else {
            self.database_manager
                .get_files_in_directory(&canonical_dir)
                .await?
        };

        // Scan the file system for current files
        let current_files = self
            .filesystem_manager
            .scan_media_directory(&canonical_dir)
            .await
            .map_err(|e| anyhow::anyhow!("File system scan failed: {}", e))?;

        // Perform incremental update
        self.perform_incremental_update(&canonical_dir, existing_files, current_files)
            .await
    }

    /// Perform an incremental update by comparing database state with file system state
    /// **OPTIMIZED FOR REDB DATABASE WITH BULK OPERATIONS**
    async fn perform_incremental_update(
        &self,
        _directory: &Path,
        existing_files: Vec<MediaFile>,
        current_files: Vec<MediaFile>,
    ) -> Result<ScanResult> {
        let mut result = ScanResult::new();

        let existing_by_normalized: std::collections::HashMap<PathBuf, MediaFile> = existing_files
            .into_iter()
            .map(|file| (file.path.clone(), file))
            .collect();
        let current_normalized: std::collections::HashMap<PathBuf, MediaFile> = current_files
            .into_iter()
            .map(|file| (file.path.clone(), file))
            .collect();

        let current_paths: HashSet<PathBuf> = current_normalized.keys().cloned().collect();

        // **REDB BULK OPERATIONS - Collect files for batch processing**
        let mut files_to_insert = Vec::new();
        let mut files_to_update = Vec::new();
        let mut files_to_remove = Vec::new();

        // Process current files - collect new ones and changed ones for bulk operations
        for (normalized_current_path, current_file) in &current_normalized {
            let existing_file = existing_by_normalized.get(normalized_current_path);

            match existing_file {
                Some(existing_file) => {
                    // File exists in database, check if it needs updating
                    if self.file_needs_update(existing_file, current_file) {
                        tracing::debug!(
                            "File needs update: {} (modified: {:?} vs {:?}, size: {} vs {})",
                            existing_file.path.display(),
                            existing_file.modified,
                            current_file.modified,
                            existing_file.size,
                            current_file.size
                        );

                        // Use the canonical path from current_file (already normalized above)
                        let mut updated_file = current_file.clone();
                        updated_file.id = existing_file.id; // Preserve database ID
                        updated_file.created_at = existing_file.created_at; // Preserve creation time
                        updated_file.updated_at = SystemTime::now();

                        files_to_update.push(updated_file);
                    } else {
                        result
                            .unchanged_files
                            .push(Self::fingerprint(existing_file));
                    }
                }
                None => {
                    // New file, add to bulk insert list with canonical path format
                    // The current_file already has the canonical path from normalization above
                    files_to_insert.push(current_file.clone());
                }
            }
        }

        // Find files that were removed from the file system
        for (normalized_existing_path, existing_file) in existing_by_normalized {
            if !current_paths.contains(&normalized_existing_path) {
                // File was removed from file system, add to bulk removal list
                files_to_remove.push(existing_file.path.clone());
                result.removed_files.push(Self::fingerprint(&existing_file));
            }
        }

        // **EXECUTE BULK OPERATIONS WITH REDB DATABASE**

        // Bulk insert new files
        if !files_to_insert.is_empty() {
            tracing::info!(
                "Bulk inserting {} new files using ReDB database",
                files_to_insert.len()
            );
            for file in &files_to_insert {
                tracing::debug!(
                    "Inserting file: path='{}', mime_type='{}', size={}",
                    file.path.display(),
                    file.mime_type,
                    file.size
                );
            }
            let insert_ids = self
                .database_manager
                .bulk_store_canonical_media_files(&files_to_insert)
                .await?;

            // Update result with inserted files and their IDs
            for (i, mut file) in files_to_insert.into_iter().enumerate() {
                if let Some(id) = insert_ids.get(i) {
                    file.id = Some(*id);
                }
                result.new_files.push(file);
            }
        }

        // Bulk update changed files
        if !files_to_update.is_empty() {
            tracing::info!(
                "Bulk updating {} changed files using ReDB database",
                files_to_update.len()
            );
            self.database_manager
                .bulk_update_canonical_media_files(&files_to_update)
                .await?;
            result.updated_files.extend(files_to_update);
        }

        // Bulk remove deleted files
        if !files_to_remove.is_empty() {
            tracing::info!(
                "Bulk removing {} deleted files using ReDB database",
                files_to_remove.len()
            );
            let removed_count = self
                .database_manager
                .bulk_remove_media_files(&files_to_remove)
                .await?;
            tracing::debug!(
                "Successfully removed {} out of {} requested files",
                removed_count,
                files_to_remove.len()
            );
        }

        result.total_scanned = current_paths.len();

        // Log bulk operation summary
        tracing::info!(
            "ReDB bulk operations completed: {} inserted, {} updated, {} removed, {} unchanged",
            result.new_files.len(),
            result.updated_files.len(),
            result.removed_files.len(),
            result.unchanged_files.len()
        );

        Ok(result)
    }

    /// Check if a file needs to be updated in the database
    fn file_needs_update(&self, existing: &MediaFile, current: &MediaFile) -> bool {
        // Compare file sizes first (most reliable)
        if existing.size != current.size {
            return true;
        }

        // Compare MIME type and filename
        if existing.mime_type != current.mime_type || existing.filename != current.filename {
            return true;
        }

        // Compare modification times with tolerance for Windows timestamp precision issues
        // Windows can have different precision depending on filesystem and access method
        let time_diff = if existing.modified > current.modified {
            existing.modified.duration_since(current.modified)
        } else {
            current.modified.duration_since(existing.modified)
        };

        // Allow up to 10 seconds difference to account for timestamp precision issues
        match time_diff {
            Ok(diff) => diff.as_secs() > 10,
            Err(_) => true, // If we can't calculate the difference, assume it needs updating
        }
    }

    fn fingerprint_needs_update(&self, existing: &FileFingerprint, current: &MediaFile) -> bool {
        if existing.size != current.size {
            return true;
        }
        let time_diff = if existing.modified > current.modified {
            existing.modified.duration_since(current.modified)
        } else {
            current.modified.duration_since(existing.modified)
        };
        time_diff.map_or(true, |difference| difference.as_secs() > 10)
    }

    /// Scan multiple directories and return combined results
    pub async fn scan_directories(&self, directories: &[PathBuf]) -> Result<ScanResult> {
        let mut combined_result = ScanResult::new();

        for directory in directories {
            match self.scan_directory(directory).await {
                Ok(result) => {
                    combined_result.merge(result);
                }
                Err(e) => {
                    tracing::warn!("Failed to scan directory {}: {}", directory.display(), e);
                    combined_result.errors.push(ScanError {
                        path: directory.clone(),
                        error: e.to_string(),
                    });
                }
            }
        }

        Ok(combined_result)
    }

    /// Perform a recursive scan of a directory using parallel multi-threaded traversal (jwalk)
    ///
    /// This method uses jwalk for fast parallel directory traversal, collecting all media files
    /// in a single pass. Files are then batched for efficient database operations.
    pub async fn scan_directory_recursive(&self, directory: &Path) -> Result<ScanResult> {
        use jwalk::WalkDir;

        // Use canonical path normalization for consistent database storage
        let canonical_root = match self.filesystem_manager.get_canonical_path(directory) {
            Ok(canonical) => PathBuf::from(canonical),
            Err(e) => {
                warn!(
                    "Failed to get canonical path for {}: {}, using basic normalization",
                    directory.display(),
                    e
                );
                self.filesystem_manager.normalize_path(directory)
            }
        };

        info!(
            "Starting parallel directory scan of: {}",
            canonical_root.display()
        );

        // Load all existing files from database once at the start (for incremental updates)
        debug!("Loading existing files from database...");
        let existing_files_map: HashMap<PathBuf, FileFingerprint> = self
            .database_manager
            .load_file_fingerprints()
            .await?
            .into_iter()
            .map(|fingerprint| (fingerprint.path.clone(), fingerprint))
            .collect();
        debug!(
            "Loaded {} existing files from database",
            existing_files_map.len()
        );

        // Use jwalk for parallel directory traversal - runs in a blocking thread pool
        let root_clone = canonical_root.clone();

        let file_paths: Vec<PathBuf> = tokio::task::spawn_blocking(move || {
            WalkDir::new(&root_clone)
                .skip_hidden(true)
                .into_iter()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().is_file())
                .filter(|entry| {
                    if let Some(ext) = entry.path().extension() {
                        if let Some(ext_str) = ext.to_str() {
                            return crate::platform::filesystem::is_supported_media_extension(
                                ext_str,
                            );
                        }
                    }
                    false
                })
                .map(|entry| entry.path())
                .collect()
        })
        .await?;

        let total_files = file_paths.len();
        info!(
            "Found {} media files, processing in batches of {}",
            total_files, BATCH_SIZE
        );

        // Process files in batches
        let mut result = ScanResult::new();
        let mut files_to_insert: Vec<MediaFile> = Vec::with_capacity(BATCH_SIZE);
        let mut files_to_update: Vec<MediaFile> = Vec::with_capacity(BATCH_SIZE);
        let mut current_paths: HashSet<PathBuf> = HashSet::with_capacity(total_files);
        let mut processed = 0;

        for path in file_paths {
            // jwalk descendants inherit the already-canonical root. It does not
            // follow file symlinks, so ordinary entries require no syscall here.
            current_paths.insert(path.clone());

            // Create MediaFile from path
            let current_file = match self.create_media_file_from_path(&path).await {
                Ok(f) => f,
                Err(e) => {
                    debug!("Failed to create MediaFile for {}: {}", path.display(), e);
                    result.errors.push(ScanError {
                        path: path.clone(),
                        error: e.to_string(),
                    });
                    continue;
                }
            };

            // Check if file exists in database
            if let Some(existing) = existing_files_map.get(&path) {
                if self.fingerprint_needs_update(existing, &current_file) {
                    let mut updated = current_file;
                    updated.id = Some(existing.id);
                    updated.created_at = existing.created_at;
                    updated.updated_at = SystemTime::now();
                    files_to_update.push(updated);
                } else {
                    result.unchanged_files.push(existing.clone());
                }
            } else {
                files_to_insert.push(current_file);
            }

            processed += 1;

            // Process batch when full
            if files_to_insert.len() >= BATCH_SIZE {
                info!(
                    "Inserting batch of {} files ({}/{})",
                    files_to_insert.len(),
                    processed,
                    total_files
                );
                let ids = self
                    .database_manager
                    .bulk_store_canonical_media_files(&files_to_insert)
                    .await?;
                for (i, mut file) in files_to_insert.drain(..).enumerate() {
                    if let Some(id) = ids.get(i) {
                        file.id = Some(*id);
                    }
                    result.new_files.push(file);
                }
            }

            if files_to_update.len() >= BATCH_SIZE {
                info!(
                    "Updating batch of {} files ({}/{})",
                    files_to_update.len(),
                    processed,
                    total_files
                );
                self.database_manager
                    .bulk_update_canonical_media_files(&files_to_update)
                    .await?;
                result.updated_files.append(&mut files_to_update);
            }

            // Progress logging every 1000 files
            if processed % 1000 == 0 {
                info!("Progress: {}/{} files processed", processed, total_files);
            }
        }

        // Process remaining files in last batch
        if !files_to_insert.is_empty() {
            info!("Inserting final batch of {} files", files_to_insert.len());
            let ids = self
                .database_manager
                .bulk_store_canonical_media_files(&files_to_insert)
                .await?;
            for (i, mut file) in files_to_insert.into_iter().enumerate() {
                if let Some(id) = ids.get(i) {
                    file.id = Some(*id);
                }
                result.new_files.push(file);
            }
        }

        if !files_to_update.is_empty() {
            info!("Updating final batch of {} files", files_to_update.len());
            self.database_manager
                .bulk_update_canonical_media_files(&files_to_update)
                .await?;
            result.updated_files.extend(files_to_update);
        }

        // Find and remove deleted files
        let files_to_remove: Vec<PathBuf> = existing_files_map
            .iter()
            .filter(|(path, _)| !current_paths.contains(*path))
            .filter(|(path, _)| path.starts_with(&canonical_root)) // Only remove files under scanned directory
            .map(|(_, file)| file.path.clone())
            .collect();

        if !files_to_remove.is_empty() {
            info!(
                "Removing {} deleted files from database",
                files_to_remove.len()
            );
            self.database_manager
                .bulk_remove_media_files(&files_to_remove)
                .await?;
            for (path, file) in existing_files_map.iter() {
                if !current_paths.contains(path) && path.starts_with(&canonical_root) {
                    result.removed_files.push(file.clone());
                }
            }
        }

        result.total_scanned = total_files;

        info!(
            "Scan completed: {} new, {} updated, {} removed, {} unchanged",
            result.new_files.len(),
            result.updated_files.len(),
            result.removed_files.len(),
            result.unchanged_files.len()
        );

        Ok(result)
    }

    /// Create a MediaFile from a path by reading file metadata
    async fn create_media_file_from_path(&self, path: &Path) -> Result<MediaFile> {
        let metadata = tokio::fs::metadata(path).await?;
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let mime_type = crate::platform::filesystem::get_mime_type_for_extension(ext);
        let size = metadata.len();
        let modified = metadata.modified().unwrap_or_else(|_| SystemTime::now());

        let mut media_file = MediaFile {
            id: None,
            path: path.to_path_buf(),
            filename,
            size,
            modified,
            mime_type,
            duration: None,
            title: None,
            artist: None,
            album: None,
            genre: None,
            track_number: None,
            year: None,
            album_artist: None,
            subtitle_available: tokio::fs::try_exists(path.with_extension("srt"))
                .await
                .unwrap_or(false),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        };

        if media_file.mime_type.starts_with("audio/") {
            let _ = crate::platform::filesystem::extract_audio_metadata(&mut media_file).await;
        }

        Ok(media_file)
    }

    /// Get the file system manager (for testing or advanced usage)
    pub fn filesystem_manager(&self) -> &dyn FileSystemManager {
        self.filesystem_manager.as_ref()
    }
}

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
    }

    /// Get the total number of changes (new + updated + removed)
    pub fn total_changes(&self) -> usize {
        self.new_files.len() + self.updated_files.len() + self.removed_files.len()
    }

    /// Check if any changes were made
    pub fn has_changes(&self) -> bool {
        self.total_changes() > 0
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::redb::RedbDatabase;
    use crate::database::MediaRepository;
    use crate::platform::filesystem::BaseFileSystemManager;
    #[cfg(target_os = "windows")]
    use crate::platform::filesystem::WindowsPathNormalizer;
    use futures_util::StreamExt;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_media_scanner_basic_functionality() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.redb");

        // Create Redb database
        let db = Arc::new(RedbDatabase::new(db_path).await.unwrap());
        db.initialize().await.unwrap();

        // Create scanner with base filesystem manager
        let filesystem_manager = Box::new(BaseFileSystemManager::new(true));
        let scanner = MediaScanner::with_filesystem_manager(filesystem_manager, db);

        // Test directory validation
        let invalid_path = Path::new("/nonexistent/directory");
        let result = scanner.scan_directory(invalid_path).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_media_scanner_path_normalization() {
        // Create temp directory and keep it alive
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path().to_path_buf();

        // Use synchronous std::fs to ensure file is created before any async ops
        let db_path = temp_path.join("test.redb");
        let test_file_path = temp_path.join("test.mp4");

        // Create test file synchronously to avoid race conditions
        std::fs::write(&test_file_path, b"fake video content").unwrap();

        // Verify directory and file exist before proceeding
        assert!(temp_path.exists(), "Temp directory should exist");
        assert!(test_file_path.exists(), "Test file should exist");

        // Create Redb database
        let db = Arc::new(RedbDatabase::new(db_path).await.unwrap());
        db.initialize().await.unwrap();

        // Create scanner with platform-appropriate path normalizer
        #[cfg(target_os = "windows")]
        let path_normalizer = Box::new(WindowsPathNormalizer::new());
        #[cfg(not(target_os = "windows"))]
        let path_normalizer = Box::new(crate::platform::filesystem::UnixPathNormalizer::new());

        // On Windows we want case-insensitivity (false), on Linux usually true but for this test we match the normalizer
        let case_sensitive = !cfg!(target_os = "windows");
        let filesystem_manager = Box::new(BaseFileSystemManager::with_normalizer(
            case_sensitive,
            path_normalizer,
        ));
        let scanner = MediaScanner::with_filesystem_manager(filesystem_manager, db.clone());

        // Verify directory still exists before scanning
        assert!(
            temp_path.exists(),
            "Temp directory should still exist before scan"
        );

        // Scan the directory
        let result = scanner.scan_directory(&temp_path).await.unwrap();

        // Verify that files were found and processed
        assert_eq!(result.new_files.len(), 1);
        let scanned_file = &result.new_files[0];

        // Verify that the path was normalized (should be canonical format)
        let expected_canonical = scanner
            .filesystem_manager()
            .get_canonical_path(&test_file_path)
            .unwrap();
        assert_eq!(scanned_file.path.to_string_lossy(), expected_canonical);

        // Verify the file was stored in the database with canonical path
        let stored_file = db.get_file_by_path(&scanned_file.path).await.unwrap();
        assert!(stored_file.is_some());
        let stored_file = stored_file.unwrap();
        assert_eq!(stored_file.path.to_string_lossy(), expected_canonical);

        // temp_dir dropped here, auto-cleanup
    }

    #[tokio::test]
    async fn test_scan_result_operations() {
        let mut result1 = ScanResult::new();
        result1.total_scanned = 5;
        result1.new_files.push(MediaFile {
            id: Some(1),
            path: PathBuf::from("/test1.mp4"),
            filename: "test1.mp4".to_string(),
            size: 1024,
            modified: SystemTime::now(),
            mime_type: "video/mp4".to_string(),
            duration: None,
            title: None,
            artist: None,
            album: None,
            genre: None,
            track_number: None,
            year: None,
            album_artist: None,
            subtitle_available: false,
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        });

        let mut result2 = ScanResult::new();
        result2.total_scanned = 3;
        result2.updated_files.push(MediaFile {
            id: Some(2),
            path: PathBuf::from("/test2.mp4"),
            filename: "test2.mp4".to_string(),
            size: 2048,
            modified: SystemTime::now(),
            mime_type: "video/mp4".to_string(),
            duration: None,
            title: None,
            artist: None,
            album: None,
            genre: None,
            track_number: None,
            year: None,
            album_artist: None,
            subtitle_available: false,
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        });

        // Test merge
        result1.merge(result2);
        assert_eq!(result1.total_scanned, 8);
        assert_eq!(result1.new_files.len(), 1);
        assert_eq!(result1.updated_files.len(), 1);

        // Test summary
        let summary = result1.summary();
        assert!(summary.contains("8 files"));
        assert!(summary.contains("1 new"));
        assert!(summary.contains("1 updated"));
    }

    #[tokio::test]
    async fn test_recursive_scan_optimization() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.redb");

        // Create Redb database
        let db = Arc::new(RedbDatabase::new(db_path).await.unwrap());
        db.initialize().await.unwrap();

        // Create scanner with base filesystem manager
        let filesystem_manager = Box::new(BaseFileSystemManager::new(true));
        let scanner = MediaScanner::with_filesystem_manager(filesystem_manager, db.clone());

        // Create a nested directory structure with test files
        let root_dir = temp_dir.path().join("media");
        let sub_dir1 = root_dir.join("videos");
        let sub_dir2 = root_dir.join("music");
        let sub_sub_dir = sub_dir1.join("movies");

        tokio::fs::create_dir_all(&sub_sub_dir).await.unwrap();
        tokio::fs::create_dir_all(&sub_dir2).await.unwrap();

        // Create test files in different directories
        tokio::fs::write(root_dir.join("root.mp4"), b"root video")
            .await
            .unwrap();
        tokio::fs::write(sub_dir1.join("video1.mp4"), b"video content")
            .await
            .unwrap();
        tokio::fs::write(sub_dir2.join("song1.mp3"), b"audio content")
            .await
            .unwrap();
        tokio::fs::write(sub_sub_dir.join("movie1.mkv"), b"movie content")
            .await
            .unwrap();

        // First scan to populate database
        let initial_result = scanner.scan_directory_recursive(&root_dir).await.unwrap();
        assert_eq!(initial_result.new_files.len(), 4);
        assert_eq!(initial_result.total_changes(), 4);

        // Verify all files were stored in database
        let mut all_files_stream = db.stream_all_media_files();
        let mut stored_files = Vec::new();
        while let Some(result) = all_files_stream.next().await {
            stored_files.push(result.unwrap());
        }
        assert_eq!(stored_files.len(), 4);

        // Second scan should find no changes (tests that optimization works correctly)
        let second_result = scanner.scan_directory_recursive(&root_dir).await.unwrap();
        assert_eq!(second_result.new_files.len(), 0);
        assert_eq!(second_result.updated_files.len(), 0);
        assert_eq!(second_result.unchanged_files.len(), 4);
        assert_eq!(second_result.total_changes(), 0);

        // Verify the optimization is working by checking that we can handle the recursive scan
        // without making individual database queries for each directory
        assert!(second_result.errors.is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn direct_scan_resolves_only_symlinked_media_entries() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().unwrap();
        let media_root = temp.path().join("media");
        tokio::fs::create_dir(&media_root).await.unwrap();
        let target = temp.path().join("target.mp4");
        tokio::fs::write(&target, b"video").await.unwrap();
        let link = media_root.join("visible-name.mp4");
        symlink(&target, &link).unwrap();

        let database = Arc::new(
            RedbDatabase::new(temp.path().join("symlink.redb"))
                .await
                .unwrap(),
        );
        database.initialize().await.unwrap();
        let scanner = MediaScanner::with_filesystem_manager(
            Box::new(BaseFileSystemManager::new(true)),
            database,
        );

        let result = scanner.scan_directory(&media_root).await.unwrap();
        assert_eq!(result.new_files.len(), 1);
        assert_eq!(result.new_files[0].filename, "visible-name.mp4");
        assert_eq!(result.new_files[0].path, target.canonicalize().unwrap());
    }
}

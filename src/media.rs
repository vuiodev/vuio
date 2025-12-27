use anyhow::Result;
use futures_util::StreamExt;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tracing::warn;

use crate::database::{DatabaseManager, MediaFile, redb::RedbDatabase};
use crate::platform::filesystem::{create_platform_filesystem_manager, FileSystemManager};

/// Media scanner that uses the file system manager and database for efficient scanning
pub struct MediaScanner {
    filesystem_manager: Box<dyn FileSystemManager>,
    database_manager: Arc<dyn DatabaseManager>,
}

impl MediaScanner {
    /// Create a new media scanner with platform-specific file system manager
    pub async fn new() -> anyhow::Result<Self> {
        // Create a temporary Redb database for basic scanning
        let temp_path = std::env::temp_dir().join("temp_scanner.redb");
        let redb_db = RedbDatabase::new(temp_path).await?;
        
        // Initialize the database
        redb_db.initialize().await?;
        
        let database_manager = Arc::new(redb_db) as Arc<dyn DatabaseManager>;
        
        Ok(Self {
            filesystem_manager: create_platform_filesystem_manager(),
            database_manager,
        })
    }
    
    /// Create a new media scanner with database manager
    pub fn with_database(database_manager: Arc<dyn DatabaseManager>) -> Self {
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
                tracing::warn!("Failed to get canonical path for {}: {}, using basic normalization", directory.display(), e);
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
        let mut fs_files = self.filesystem_manager
            .scan_media_directory(&canonical_dir)
            .await
            .map_err(|e| anyhow::anyhow!("File system scan failed: {}", e))?;
        
        // Apply canonical path normalization to all scanned files
        for file in &mut fs_files {
            if let Ok(canonical_path) = self.filesystem_manager.get_canonical_path(&file.path) {
                file.path = PathBuf::from(canonical_path);
            } else {
                // Fallback to basic normalization if canonical fails
                file.path = self.filesystem_manager.normalize_path(&file.path);
            }
        }
        
        Ok(fs_files)
    }

    /// Simple recursive directory scan that returns files without database operations
    pub async fn scan_directory_recursively_simple(&self, directory: &Path) -> Result<Vec<MediaFile>> {
        let mut all_files = Vec::with_capacity(1000); // Pre-allocate capacity
        let mut dirs_to_scan = Vec::with_capacity(100); // Pre-allocate capacity
        
        // Start with canonical path normalization
        let canonical_root = match self.filesystem_manager.get_canonical_path(directory) {
            Ok(canonical) => PathBuf::from(canonical),
            Err(e) => {
                tracing::warn!("Failed to get canonical path for {}: {}, using basic normalization", directory.display(), e);
                self.filesystem_manager.normalize_path(directory)
            }
        };
        
        dirs_to_scan.push(canonical_root);

        while let Some(current_dir) = dirs_to_scan.pop() {
            // Scan current directory for files
            match self.filesystem_manager.scan_media_directory(&current_dir).await {
                Ok(mut fs_files) => {
                    // Apply canonical path normalization to all scanned files
                    for file in &mut fs_files {
                        if let Ok(canonical_path) = self.filesystem_manager.get_canonical_path(&file.path) {
                            file.path = PathBuf::from(canonical_path);
                        } else {
                            // Fallback to basic normalization if canonical fails
                            file.path = self.filesystem_manager.normalize_path(&file.path);
                        }
                    }
                    all_files.extend(fs_files);
                }
                Err(e) => warn!("Failed to scan directory {}: {}", current_dir.display(), e),
            }

            // Find subdirectories and add to the queue
            if let Ok(mut entries) = tokio::fs::read_dir(&current_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.is_dir() {
                        // Apply canonical normalization to subdirectory paths too
                        let canonical_subdir = match self.filesystem_manager.get_canonical_path(&path) {
                            Ok(canonical) => PathBuf::from(canonical),
                            Err(_) => self.filesystem_manager.normalize_path(&path)
                        };
                        dirs_to_scan.push(canonical_subdir);
                    }
                }
            }
        }
        Ok(all_files)
    }
    
    /// Create a media scanner with a custom file system manager (for testing)
    pub fn with_filesystem_manager(
        filesystem_manager: Box<dyn FileSystemManager>,
        database_manager: Arc<dyn DatabaseManager>,
    ) -> Self {
        Self {
            filesystem_manager,
            database_manager,
        }
    }
    
    /// Perform a full scan of a directory, updating the database with new/changed files
    pub async fn scan_directory(&self, directory: &Path) -> Result<ScanResult> {
        self.scan_directory_with_existing_files(directory, None).await
    }
    
    /// Internal method that allows passing existing files to avoid repeated database queries during recursive scans
    async fn scan_directory_with_existing_files(&self, directory: &Path, all_existing_files: Option<&[MediaFile]>) -> Result<ScanResult> {
        // Use canonical path normalization for consistency
        let canonical_dir = match self.filesystem_manager.get_canonical_path(directory) {
            Ok(canonical) => PathBuf::from(canonical),
            Err(e) => {
                tracing::warn!("Failed to get canonical path for {}: {}, using basic normalization", directory.display(), e);
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
            all_files.iter()
                .filter(|file| {
                    let file_parent = file.path.parent().unwrap_or_else(|| std::path::Path::new(""));
                    // Use canonical normalization for consistent comparison
                    let canonical_file_parent = match self.filesystem_manager.get_canonical_path(file_parent) {
                        Ok(canonical) => PathBuf::from(canonical),
                        Err(_) => self.filesystem_manager.normalize_path(file_parent)
                    };
                    canonical_file_parent == canonical_dir
                })
                .cloned()
                .collect()
        } else {
            self.database_manager
                .get_files_in_directory(&canonical_dir)
                .await?
        };
        
        // Scan the file system for current files
        let mut current_files = self.filesystem_manager
            .scan_media_directory(&canonical_dir)
            .await
            .map_err(|e| anyhow::anyhow!("File system scan failed: {}", e))?;
        
        // Apply canonical path normalization to all scanned files before database operations
        for file in &mut current_files {
            let original_path = file.path.clone();
            if let Ok(canonical_path) = self.filesystem_manager.get_canonical_path(&file.path) {
                file.path = PathBuf::from(canonical_path);
                tracing::debug!("Path normalization: '{}' -> '{}'", original_path.display(), file.path.display());
            } else {
                // Fallback to basic normalization if canonical fails
                file.path = self.filesystem_manager.normalize_path(&file.path);
                tracing::debug!("Path normalization (fallback): '{}' -> '{}'", original_path.display(), file.path.display());
            }
        }
        
        // Perform incremental update
        self.perform_incremental_update(&canonical_dir, existing_files, current_files).await
    }
    
    /// Perform an incremental update by comparing database state with file system state
    /// **OPTIMIZED FOR ZEROCOPY DATABASE WITH BULK OPERATIONS**
    async fn perform_incremental_update(
        &self,
        _directory: &Path,
        existing_files: Vec<MediaFile>,
        current_files: Vec<MediaFile>,
    ) -> Result<ScanResult> {
        let mut result = ScanResult::new();
        
        // Create lookup maps for efficient comparison with pre-allocated capacity
        // Use both original and normalized paths to handle legacy database entries
        let mut existing_by_original: std::collections::HashMap<PathBuf, MediaFile> = std::collections::HashMap::with_capacity(existing_files.len());
        let mut existing_by_normalized: std::collections::HashMap<PathBuf, MediaFile> = std::collections::HashMap::with_capacity(existing_files.len());
        
        for existing_file in existing_files {
            // Use canonical path normalization for database consistency
            let canonical_path = match self.filesystem_manager.get_canonical_path(&existing_file.path) {
                Ok(canonical) => PathBuf::from(canonical),
                Err(e) => {
                    tracing::warn!("Failed to get canonical path for {}: {}", existing_file.path.display(), e);
                    self.filesystem_manager.normalize_path(&existing_file.path)
                }
            };
            
            existing_by_original.insert(existing_file.path.clone(), existing_file.clone());
            existing_by_normalized.insert(canonical_path, existing_file);
        }
        
        // Current files paths - normalize for consistent comparison with pre-allocated capacity
        let mut current_normalized: std::collections::HashMap<PathBuf, MediaFile> = std::collections::HashMap::with_capacity(current_files.len());
        for f in &current_files {
            // Apply canonical path normalization to current files before database operations
            let canonical_path = match self.filesystem_manager.get_canonical_path(&f.path) {
                Ok(canonical) => PathBuf::from(canonical),
                Err(e) => {
                    tracing::warn!("Failed to get canonical path for {}: {}", f.path.display(), e);
                    self.filesystem_manager.normalize_path(&f.path)
                }
            };
            
            // Create a normalized version of the MediaFile for database storage
            let mut normalized_file = f.clone();
            normalized_file.path = canonical_path.clone();
            
            current_normalized.insert(canonical_path, normalized_file);
        }
        
        let current_paths: HashSet<PathBuf> = current_normalized.keys().cloned().collect();
        
        // **ZEROCOPY BULK OPERATIONS - Collect files for batch processing**
        let mut files_to_insert = Vec::new();
        let mut files_to_update = Vec::new();
        let mut files_to_remove = Vec::new();
        
        // Process current files - collect new ones and changed ones for bulk operations
        for (normalized_current_path, current_file) in &current_normalized {
            // Try to find existing file by normalized path first, then by original path
            let existing_file = existing_by_normalized.get(normalized_current_path)
                .or_else(|| existing_by_original.get(&current_file.path));
            
            match existing_file {
                Some(existing_file) => {
                    // File exists in database, check if it needs updating
                    if self.file_needs_update(existing_file, current_file) {
                        tracing::debug!("File needs update: {} (modified: {:?} vs {:?}, size: {} vs {})", 
                            existing_file.path.display(), 
                            existing_file.modified, current_file.modified,
                            existing_file.size, current_file.size);
                        
                        // Use the canonical path from current_file (already normalized above)
                        let mut updated_file = current_file.clone();
                        updated_file.id = existing_file.id; // Preserve database ID
                        updated_file.created_at = existing_file.created_at; // Preserve creation time
                        updated_file.updated_at = SystemTime::now();
                        
                        files_to_update.push(updated_file);
                    } else {
                        // Check if the existing file path needs canonical normalization
                        let existing_canonical = match self.filesystem_manager.get_canonical_path(&existing_file.path) {
                            Ok(canonical) => PathBuf::from(canonical),
                            Err(e) => {
                                tracing::warn!("Failed to get canonical path for {}: {}", existing_file.path.display(), e);
                                self.filesystem_manager.normalize_path(&existing_file.path)
                            }
                        };
                        
                        if existing_file.path != existing_canonical {
                            // Path needs canonical normalization - update it in the database
                            tracing::debug!("Normalizing path to canonical format: '{}' -> '{}'", existing_file.path.display(), existing_canonical.display());
                            let mut normalized_existing = existing_file.clone();
                            normalized_existing.path = existing_canonical;
                            normalized_existing.updated_at = SystemTime::now();
                            
                            files_to_update.push(normalized_existing);
                        } else {
                            result.unchanged_files.push(existing_file.clone());
                        }
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
        // Check both normalized and original paths to handle legacy entries
        for (normalized_existing_path, existing_file) in existing_by_normalized {
            if !current_paths.contains(&normalized_existing_path) {
                // File was removed from file system, add to bulk removal list
                files_to_remove.push(existing_file.path.clone());
                result.removed_files.push(existing_file);
            }
        }
        
        // **EXECUTE BULK OPERATIONS WITH ZEROCOPY DATABASE**
        
        // Bulk insert new files
        if !files_to_insert.is_empty() {
            tracing::info!("Bulk inserting {} new files using ZeroCopy database", files_to_insert.len());
            for file in &files_to_insert {
                tracing::debug!("Inserting file: path='{}', mime_type='{}', size={}", 
                    file.path.display(), file.mime_type, file.size);
            }
            let insert_ids = self.database_manager.bulk_store_media_files(&files_to_insert).await?;
            
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
            tracing::info!("Bulk updating {} changed files using ZeroCopy database", files_to_update.len());
            self.database_manager.bulk_update_media_files(&files_to_update).await?;
            result.updated_files.extend(files_to_update);
        }
        
        // Bulk remove deleted files
        if !files_to_remove.is_empty() {
            tracing::info!("Bulk removing {} deleted files using ZeroCopy database", files_to_remove.len());
            let removed_count = self.database_manager.bulk_remove_media_files(&files_to_remove).await?;
            tracing::debug!("Successfully removed {} out of {} requested files", removed_count, files_to_remove.len());
        }
        
        result.total_scanned = current_paths.len();
        
        // Log bulk operation summary
        tracing::info!(
            "ZeroCopy bulk operations completed: {} inserted, {} updated, {} removed, {} unchanged",
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
    
    /// Perform a recursive scan of a directory and its subdirectories
    /// 
    /// This method is optimized to avoid N+1 query problems by loading all existing files
    /// from the database once at the start, then passing this collection down to each
    /// directory scan. This significantly improves performance for large directory structures.
    /// 
    /// The method also implements batch processing to handle very large directory structures
    /// efficiently without blocking the async runtime.
    pub async fn scan_directory_recursive(&self, directory: &Path) -> Result<ScanResult> {
        // Use canonical path normalization for consistent database storage
        let canonical_root = match self.filesystem_manager.get_canonical_path(directory) {
            Ok(canonical) => PathBuf::from(canonical),
            Err(e) => {
                tracing::warn!("Failed to get canonical path for {}: {}, using basic normalization", directory.display(), e);
                self.filesystem_manager.normalize_path(directory)
            }
        };
        
        // Fix N+1 query problem: Load all existing files from database once at the start
        tracing::debug!("Loading all existing files from database for recursive scan optimization");
        let all_existing_files = {
            let mut files = Vec::with_capacity(1000);
            let mut stream = self.database_manager.stream_all_media_files();
            while let Some(result) = stream.next().await {
                match result {
                    Ok(file) => files.push(file),
                    Err(e) => {
                        tracing::warn!("Error reading file from database during recursive scan: {}", e);
                        // Continue processing other files
                    }
                }
            }
            files
        };
        
        tracing::debug!("Loaded {} existing files from database for recursive scan optimization", all_existing_files.len());
        
        // Perform recursive scan with batch processing
        let result = self.scan_directory_recursive_with_existing_files(&canonical_root, &all_existing_files).await?;
        
        tracing::debug!("Recursive scan completed: {}", result.summary());
        Ok(result)
    }
    
    /// Internal optimized recursive scan that uses pre-loaded existing files to avoid N+1 queries
    async fn scan_directory_recursive_with_existing_files(
        &self, 
        directory: &Path, 
        all_existing_files: &[MediaFile]
    ) -> Result<ScanResult> {
        let mut combined_result = ScanResult::new();
        let mut directories_to_scan = Vec::with_capacity(100);
        directories_to_scan.push(directory.to_path_buf());
        
        // Process directories in batches to handle large directory structures efficiently
        const BATCH_SIZE: usize = 50; // Process up to 50 directories at a time
        
        while !directories_to_scan.is_empty() {
            // Take a batch of directories to process
            let batch_size = std::cmp::min(BATCH_SIZE, directories_to_scan.len());
            let current_batch: Vec<_> = directories_to_scan.drain(0..batch_size).collect();
            
            // Process each directory in the current batch
            for current_dir in current_batch {
                // Use the optimized scan method that accepts pre-loaded existing files
                match self.scan_directory_with_existing_files(&current_dir, Some(all_existing_files)).await {
                    Ok(result) => {
                        combined_result.merge(result);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to scan directory {}: {}", current_dir.display(), e);
                        combined_result.errors.push(ScanError {
                            path: current_dir.clone(),
                            error: e.to_string(),
                        });
                        continue; // Skip subdirectory scanning if parent failed
                    }
                }
                
                // Find subdirectories to add to the scan queue
                if let Ok(mut entries) = tokio::fs::read_dir(&current_dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        let entry_path = entry.path();
                        if entry_path.is_dir() {
                            // Skip hidden directories and common system directories
                            if let Some(dir_name) = entry_path.file_name().and_then(|n| n.to_str()) {
                                if !dir_name.starts_with('.') && 
                                   !matches!(dir_name.to_lowercase().as_str(), 
                                       "system volume information" | "$recycle.bin" | "recycler" | 
                                       "windows" | "program files" | "program files (x86)"
                                   ) {
                                    directories_to_scan.push(entry_path);
                                }
                            }
                        }
                    }
                }
            }
            
            // Yield control periodically during large scans to prevent blocking
            if directories_to_scan.len() > BATCH_SIZE * 2 {
                tracing::debug!("Processing large directory structure: {} directories remaining", directories_to_scan.len());
                tokio::task::yield_now().await;
            }
        }
        
        Ok(combined_result)
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
    pub removed_files: Vec<MediaFile>,
    
    /// Files that were unchanged
    pub unchanged_files: Vec<MediaFile>,
    
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

/// Legacy function for backward compatibility - performs a simple directory scan
/// 
/// This function is deprecated in favor of using MediaScanner directly
#[deprecated(note = "Use MediaScanner::scan_directory instead")]
pub async fn scan_media_files(dir: &Path) -> Result<Vec<MediaFile>> {
    let filesystem_manager = create_platform_filesystem_manager();
    
    let fs_files = filesystem_manager
        .scan_media_directory(dir)
        .await
        .map_err(|e| anyhow::anyhow!("Scan failed: {}", e))?;
    
    Ok(fs_files)
}

/// Get MIME type for a file based on its extension
pub fn get_mime_type(path: &std::path::Path) -> String {
    let extension = path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    match extension.as_str() {
        // Video formats
        "mp4" => "video/mp4",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "mov" => "video/quicktime",
        "wmv" => "video/x-ms-wmv",
        "flv" => "video/x-flv",
        "webm" => "video/webm",
        "m4v" => "video/x-m4v",
        "3gp" => "video/3gpp",
        "mpg" | "mpeg" => "video/mpeg",
        
        // Audio formats
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        "aac" => "audio/aac",
        "ogg" => "audio/ogg",
        "wma" => "audio/x-ms-wma",
        "m4a" => "audio/mp4",
        "opus" => "audio/opus",
        "aiff" => "audio/aiff",
        
        // Image formats
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tiff" => "image/tiff",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        
        _ => "application/octet-stream",
    }.to_string()
}

/// Get MIME type for a file based on its extension (legacy function)
/// 
/// This function is deprecated in favor of using the filesystem module directly
#[deprecated(note = "Use crate::platform::filesystem::get_mime_type_for_extension instead")]
pub fn get_mime_type_legacy(path: &std::path::Path) -> String {
    get_mime_type(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::redb::RedbDatabase;
    use crate::platform::filesystem::{BaseFileSystemManager, WindowsPathNormalizer};
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
        
        // Create scanner with custom path normalizer for testing
        let path_normalizer = Box::new(WindowsPathNormalizer::new());
        let filesystem_manager = Box::new(BaseFileSystemManager::with_normalizer(false, path_normalizer));
        let scanner = MediaScanner::with_filesystem_manager(filesystem_manager, db.clone());
        
        // Verify directory still exists before scanning
        assert!(temp_path.exists(), "Temp directory should still exist before scan");
        
        // Scan the directory
        let result = scanner.scan_directory(&temp_path).await.unwrap();
        
        // Verify that files were found and processed
        assert_eq!(result.new_files.len(), 1);
        let scanned_file = &result.new_files[0];
        
        // Verify that the path was normalized (should be canonical format)
        let expected_canonical = scanner.filesystem_manager().get_canonical_path(&test_file_path).unwrap();
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
        tokio::fs::write(root_dir.join("root.mp4"), b"root video").await.unwrap();
        tokio::fs::write(sub_dir1.join("video1.mp4"), b"video content").await.unwrap();
        tokio::fs::write(sub_dir2.join("song1.mp3"), b"audio content").await.unwrap();
        tokio::fs::write(sub_sub_dir.join("movie1.mkv"), b"movie content").await.unwrap();
        
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
}
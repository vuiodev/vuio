use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tracing::warn;

use crate::database::{DatabaseManager, MediaFile};
use crate::platform::filesystem::{create_platform_filesystem_manager, FileSystemManager};

/// Media scanner that uses the file system manager and database for efficient scanning
pub struct MediaScanner {
    filesystem_manager: Box<dyn FileSystemManager>,
    database_manager: Arc<dyn DatabaseManager>,
}

impl MediaScanner {
    /// Create a new media scanner with platform-specific file system manager
    pub async fn new() -> anyhow::Result<Self> {
        // Create a temporary in-memory database for basic scanning
        let temp_path = std::env::temp_dir().join("temp_scanner.db");
        let database_manager = Arc::new(crate::database::SqliteDatabase::new(temp_path).await?) as Arc<dyn DatabaseManager>;
        
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
        let normalized_dir = self.filesystem_manager.normalize_path(directory);
        
        // Validate the directory path
        self.filesystem_manager.validate_path(&normalized_dir)?;
        
        if !self.filesystem_manager.is_accessible(&normalized_dir).await {
            return Err(anyhow::anyhow!(
                "Directory is not accessible: {}",
                normalized_dir.display()
            ));
        }
        
        // Scan the file system for current files
        let fs_files = self.filesystem_manager
            .scan_media_directory(&normalized_dir)
            .await
            .map_err(|e| anyhow::anyhow!("File system scan failed: {}", e))?;
        
        Ok(fs_files)
    }

    /// Simple recursive directory scan that returns files without database operations
    pub async fn scan_directory_recursively_simple(&self, directory: &Path) -> Result<Vec<MediaFile>> {
        let mut all_files = Vec::new();
        let mut dirs_to_scan = vec![directory.to_path_buf()];

        while let Some(current_dir) = dirs_to_scan.pop() {
            // Scan current directory for files
            match self.filesystem_manager.scan_media_directory(&current_dir).await {
                Ok(fs_files) => {
                    all_files.extend(fs_files);
                }
                Err(e) => warn!("Failed to scan directory {}: {}", current_dir.display(), e),
            }

            // Find subdirectories and add to the queue
            if let Ok(mut entries) = tokio::fs::read_dir(&current_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.is_dir() {
                        dirs_to_scan.push(path);
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
        let normalized_dir = self.filesystem_manager.normalize_path(directory);
        
        // Validate the directory path
        self.filesystem_manager.validate_path(&normalized_dir)?;
        
        if !self.filesystem_manager.is_accessible(&normalized_dir).await {
            return Err(anyhow::anyhow!(
                "Directory is not accessible: {}",
                normalized_dir.display()
            ));
        }
        
        // Get existing files from database for this directory
        let existing_files = if let Some(all_files) = all_existing_files {
            // Filter existing files to only those in this directory
            all_files.iter()
                .filter(|file| {
                    let file_parent = file.path.parent().unwrap_or_else(|| std::path::Path::new(""));
                    let normalized_file_parent = self.filesystem_manager.normalize_path(file_parent);
                    normalized_file_parent == normalized_dir
                })
                .cloned()
                .collect()
        } else {
            self.database_manager
                .get_files_in_directory(&normalized_dir)
                .await?
        };
        
        // Scan the file system for current files
        let current_files = self.filesystem_manager
            .scan_media_directory(&normalized_dir)
            .await
            .map_err(|e| anyhow::anyhow!("File system scan failed: {}", e))?;
        
        // Perform incremental update
        self.perform_incremental_update(&normalized_dir, existing_files, current_files).await
    }
    
    /// Perform an incremental update by comparing database state with file system state
    async fn perform_incremental_update(
        &self,
        _directory: &Path,
        existing_files: Vec<MediaFile>,
        current_files: Vec<MediaFile>,
    ) -> Result<ScanResult> {
        let mut result = ScanResult::new();
        
        // Create lookup maps for efficient comparison
        // Use both original and normalized paths to handle legacy database entries
        let mut existing_by_original: std::collections::HashMap<PathBuf, MediaFile> = std::collections::HashMap::new();
        let mut existing_by_normalized: std::collections::HashMap<PathBuf, MediaFile> = std::collections::HashMap::new();
        
        for existing_file in existing_files {
            let normalized_path = self.filesystem_manager.normalize_path(&existing_file.path);
            

            
            existing_by_original.insert(existing_file.path.clone(), existing_file.clone());
            existing_by_normalized.insert(normalized_path, existing_file);
        }
        
        // Current files paths - normalize for consistent comparison
        let current_normalized: std::collections::HashMap<PathBuf, MediaFile> = current_files
            .iter()
            .map(|f| {
                let normalized_path = self.filesystem_manager.normalize_path(&f.path);
                

                
                (normalized_path, f.clone())
            })
            .collect();
        
        let current_paths: HashSet<PathBuf> = current_normalized.keys().cloned().collect();
        

        
        // Process current files - add new ones or update changed ones
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
                        let mut updated_file = current_file.clone();
                        updated_file.path = normalized_current_path.clone(); // Use normalized path
                        updated_file.id = existing_file.id; // Preserve database ID
                        updated_file.created_at = existing_file.created_at; // Preserve creation time
                        updated_file.updated_at = SystemTime::now();
                        
                        self.database_manager.update_media_file(&updated_file).await?;
                        result.updated_files.push(updated_file);
                    } else {
                        // Check if the existing file path needs normalization
                        let existing_normalized = self.filesystem_manager.normalize_path(&existing_file.path);
                        if existing_file.path != existing_normalized {
                            // Path needs normalization - update it in the database
                            tracing::debug!("Normalizing path: '{}' -> '{}'", existing_file.path.display(), existing_normalized.display());
                            let mut normalized_existing = existing_file.clone();
                            normalized_existing.path = existing_normalized;
                            normalized_existing.updated_at = SystemTime::now();
                            
                            self.database_manager.update_media_file(&normalized_existing).await?;
                            result.updated_files.push(normalized_existing);
                        } else {
                            result.unchanged_files.push(existing_file.clone());
                        }
                    }
                }
                None => {
                    // New file, add to database with normalized path
                    let mut normalized_file = current_file.clone();
                    normalized_file.path = normalized_current_path.clone();
                    let id = self.database_manager.store_media_file(&normalized_file).await?;
                    normalized_file.id = Some(id);
                    result.new_files.push(normalized_file);
                }
            }
        }
        
        // Find files that were removed from the file system
        // Check both normalized and original paths to handle legacy entries
        for (normalized_existing_path, existing_file) in existing_by_normalized {
            if !current_paths.contains(&normalized_existing_path) {
                // File was removed from file system, remove from database using original path
                if self.database_manager.remove_media_file(&existing_file.path).await? {
                    result.removed_files.push(existing_file);
                }
            }
        }
        
        result.total_scanned = current_paths.len();
        
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
    pub async fn scan_directory_recursive(&self, directory: &Path) -> Result<ScanResult> {
        let normalized_root = self.filesystem_manager.normalize_path(directory);
        
        // Get all existing files from database once at the beginning
        let all_existing_files = self.database_manager.get_all_media_files().await?;
        
        let mut combined_result = ScanResult::new();
        let mut directories_to_scan = vec![normalized_root.clone()];
        
        while let Some(current_dir) = directories_to_scan.pop() {
            // Scan current directory with the pre-loaded existing files
            match self.scan_directory_with_existing_files(&current_dir, Some(&all_existing_files)).await {
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
            
            // Find subdirectories to scan
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
    /// Create a new empty scan result
    pub fn new() -> Self {
        Self {
            new_files: Vec::new(),
            updated_files: Vec::new(),
            removed_files: Vec::new(),
            unchanged_files: Vec::new(),
            total_scanned: 0,
            errors: Vec::new(),
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
pub async fn scan_media_files(dir: &PathBuf) -> Result<Vec<MediaFile>> {
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
    use crate::database::SqliteDatabase;
    use crate::platform::filesystem::BaseFileSystemManager;
    use std::sync::Arc;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_media_scanner_basic_functionality() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        
        // Create database
        let db = Arc::new(SqliteDatabase::new(db_path).await.unwrap());
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
}
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use thiserror::Error;
use tokio::fs;

use crate::database::MediaFile;

#[cfg(target_os = "windows")]
pub mod windows;

/// File system manager trait for cross-platform file operations
#[async_trait::async_trait]
pub trait FileSystemManager: Send + Sync {
    /// Scan a media directory and return all media files
    async fn scan_media_directory(&self, path: &Path) -> Result<Vec<MediaFile>, FileSystemError>;
    
    /// Normalize a path for the current platform
    fn normalize_path(&self, path: &Path) -> PathBuf;
    
    /// Check if a path is accessible with current permissions
    async fn is_accessible(&self, path: &Path) -> bool;
    
    /// Get detailed file information
    async fn get_file_info(&self, path: &Path) -> Result<FileInfo, FileSystemError>;
    
    /// Check if two paths refer to the same file (handles case sensitivity)
    fn paths_equal(&self, path1: &Path, path2: &Path) -> bool;
    
    /// Validate that a path is safe to access (security check)
    fn validate_path(&self, path: &Path) -> Result<(), FileSystemError>;
    
    /// Get the canonical form of a path
    async fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, FileSystemError>;
    
    /// Check if a file matches the given extensions (case-insensitive on Windows)
    fn matches_extension(&self, path: &Path, extensions: &[String]) -> bool;
}

/// File system specific errors
#[derive(Error, Debug)]
pub enum FileSystemError {
    #[error("Path not found: {path}")]
    PathNotFound { path: String },
    
    #[error("Access denied: {path} - {reason}")]
    AccessDenied { path: String, reason: String },
    
    #[error("Invalid path: {path} - {reason}")]
    InvalidPath { path: String, reason: String },
    
    #[error("Path contains invalid Windows character '{character}': {path} - {reason}")]
    InvalidWindowsCharacter {
        path: String,
        character: char,
        reason: String,
    },
    
    #[error("Invalid colon usage in Windows path: {path} - {details}")]
    InvalidColonUsage {
        path: String,
        details: String,
    },
    
    #[error("Path exceeds maximum length: {path} - {details}")]
    PathTooLong {
        path: String,
        details: String,
    },
    
    #[error("Path contains reserved name: {path} - {reserved_name}")]
    ReservedName {
        path: String,
        reserved_name: String,
    },
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Permission error: {path} - {details}")]
    Permission { path: String, details: String },
    
    #[error("Encoding error: {path} - {details}")]
    Encoding { path: String, details: String },
    
    #[error("Platform-specific error: {0}")]
    Platform(String),
}

impl FileSystemError {
    /// Get a user-friendly error message with detailed explanation
    pub fn user_message(&self) -> String {
        match self {
            FileSystemError::PathNotFound { path } => {
                format!(
                    "Path not found: {}\n\nWhat this means: The specified file or directory does not exist.\nSuggestion: Verify the path is correct and the file/directory exists.",
                    path
                )
            }
            FileSystemError::AccessDenied { path, reason } => {
                format!(
                    "Access denied: {}\nReason: {}\n\nWhat this means: You don't have permission to access this location.\nSuggestion: Check file permissions or run with appropriate privileges.",
                    path, reason
                )
            }
            FileSystemError::InvalidPath { path, reason } => {
                format!(
                    "Invalid path: {}\nReason: {}\n\nWhat this means: The path format is not valid for this system.\nSuggestion: Use a valid path format for your operating system.",
                    path, reason
                )
            }
            FileSystemError::InvalidWindowsCharacter { path, character, reason } => {
                format!(
                    "Invalid Windows path character '{}': {}\nReason: {}\n\nWhat this means: Windows paths cannot contain certain special characters.\nSuggestion: Remove or replace the invalid character '{}' from your path.",
                    character, path, reason, character
                )
            }
            FileSystemError::InvalidColonUsage { path, details } => {
                format!(
                    "Invalid colon usage in Windows path: {}\nDetails: {}\n\nWhat this means: Colons (:) are only allowed in specific positions in Windows paths.\nValid examples:\n  - Drive letters: C:\\Users\\Documents\n  - UNC network paths: \\\\server:port\\share\nSuggestion: Check that colons are only used in drive letters or network addresses.",
                    path, details
                )
            }
            FileSystemError::PathTooLong { path, details } => {
                format!(
                    "Path too long: {}\nDetails: {}\n\nWhat this means: The path exceeds the maximum length allowed by Windows.\nSuggestion: Use a shorter path or enable long path support in Windows.",
                    path, details
                )
            }
            FileSystemError::ReservedName { path, reserved_name } => {
                format!(
                    "Reserved name in path: {}\nReserved name: {}\n\nWhat this means: Windows reserves certain names that cannot be used as file or directory names.\nReserved names include: CON, PRN, AUX, NUL, COM1-9, LPT1-9\nSuggestion: Choose a different name that is not reserved by Windows.",
                    path, reserved_name
                )
            }
            FileSystemError::Io(err) => {
                format!(
                    "System I/O error: {}\n\nWhat this means: A low-level system operation failed.\nSuggestion: Check disk space, file permissions, and system resources.",
                    err
                )
            }
            FileSystemError::Permission { path, details } => {
                format!(
                    "Permission error: {}\nDetails: {}\n\nWhat this means: The application doesn't have the required permissions.\nSuggestion: Check file/directory permissions or run with elevated privileges.",
                    path, details
                )
            }
            FileSystemError::Encoding { path, details } => {
                format!(
                    "Text encoding error: {}\nDetails: {}\n\nWhat this means: The path contains characters that cannot be properly encoded.\nSuggestion: Use standard ASCII characters in file paths when possible.",
                    path, details
                )
            }
            FileSystemError::Platform(msg) => {
                format!(
                    "Platform-specific error: {}\n\nWhat this means: A system-specific operation failed.\nSuggestion: Check system logs and ensure the operation is supported on your platform.",
                    msg
                )
            }
        }
    }
    
    /// Get recovery suggestions for the error
    pub fn recovery_suggestions(&self) -> Vec<String> {
        match self {
            FileSystemError::PathNotFound { .. } => vec![
                "Verify the path exists".to_string(),
                "Check for typos in the path".to_string(),
                "Create the missing directory if needed".to_string(),
                "Use an absolute path instead of relative".to_string(),
            ],
            FileSystemError::AccessDenied { .. } => vec![
                "Check file/directory permissions".to_string(),
                "Run the application with administrator privileges".to_string(),
                "Ensure the file is not locked by another application".to_string(),
                "Verify you have read/write access to the parent directory".to_string(),
            ],
            FileSystemError::InvalidPath { .. } => vec![
                "Use forward slashes (/) or double backslashes (\\\\) in paths".to_string(),
                "Avoid special characters in file names".to_string(),
                "Use absolute paths when possible".to_string(),
                "Check path format for your operating system".to_string(),
            ],
            FileSystemError::InvalidWindowsCharacter { character, .. } => vec![
                format!("Remove the '{}' character from the path", character),
                format!("Replace '{}' with an underscore or dash", character),
                "Use only alphanumeric characters and standard separators".to_string(),
                "Avoid these characters in Windows paths: < > : \" | ? *".to_string(),
            ],
            FileSystemError::InvalidColonUsage { .. } => vec![
                "Use colons only in drive letters (C:, D:, etc.)".to_string(),
                "For network paths, use \\\\server:port\\share format".to_string(),
                "Remove colons from file and directory names".to_string(),
                "Use underscores or dashes instead of colons in names".to_string(),
            ],
            FileSystemError::PathTooLong { .. } => vec![
                "Use shorter file and directory names".to_string(),
                "Move files to a location with a shorter path".to_string(),
                "Enable long path support in Windows 10/11".to_string(),
                "Use the \\\\?\\ prefix for very long paths".to_string(),
            ],
            FileSystemError::ReservedName { reserved_name, .. } => vec![
                format!("Rename '{}' to something else", reserved_name),
                "Add a suffix or prefix to make the name unique".to_string(),
                "Avoid these reserved names: CON, PRN, AUX, NUL, COM1-9, LPT1-9".to_string(),
                "Use descriptive names that don't conflict with system names".to_string(),
            ],
            FileSystemError::Io(_) => vec![
                "Check available disk space".to_string(),
                "Verify file permissions".to_string(),
                "Close other applications that might be using the file".to_string(),
                "Restart the application and try again".to_string(),
            ],
            FileSystemError::Permission { .. } => vec![
                "Run as administrator or with elevated privileges".to_string(),
                "Check and modify file/directory permissions".to_string(),
                "Ensure the file is not read-only".to_string(),
                "Verify you own the file or have appropriate access".to_string(),
            ],
            FileSystemError::Encoding { .. } => vec![
                "Use standard ASCII characters in paths".to_string(),
                "Avoid special Unicode characters in file names".to_string(),
                "Rename files with problematic characters".to_string(),
                "Check system locale and encoding settings".to_string(),
            ],
            FileSystemError::Platform(_) => vec![
                "Check system documentation for platform-specific requirements".to_string(),
                "Verify the operation is supported on your system".to_string(),
                "Update your operating system if needed".to_string(),
                "Contact support with system details".to_string(),
            ],
        }
    }
    
    /// Check if the error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            FileSystemError::PathNotFound { .. } => true,  // Path might be created
            FileSystemError::AccessDenied { .. } => true,  // Permissions might be fixed
            FileSystemError::InvalidPath { .. } => false, // Path format is fundamentally wrong
            FileSystemError::InvalidWindowsCharacter { .. } => false, // Character is invalid
            FileSystemError::InvalidColonUsage { .. } => false, // Colon usage is invalid
            FileSystemError::PathTooLong { .. } => true,  // Path might be shortened
            FileSystemError::ReservedName { .. } => false, // Name is reserved
            FileSystemError::Io(_) => true,               // I/O issues might be temporary
            FileSystemError::Permission { .. } => true,   // Permissions might be fixed
            FileSystemError::Encoding { .. } => false,    // Encoding issues are fundamental
            FileSystemError::Platform(_) => false,        // Platform issues are usually permanent
        }
    }
    
    /// Get the severity level of the error
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            FileSystemError::PathNotFound { .. } => ErrorSeverity::Warning,
            FileSystemError::AccessDenied { .. } => ErrorSeverity::Error,
            FileSystemError::InvalidPath { .. } => ErrorSeverity::Error,
            FileSystemError::InvalidWindowsCharacter { .. } => ErrorSeverity::Error,
            FileSystemError::InvalidColonUsage { .. } => ErrorSeverity::Error,
            FileSystemError::PathTooLong { .. } => ErrorSeverity::Warning,
            FileSystemError::ReservedName { .. } => ErrorSeverity::Error,
            FileSystemError::Io(_) => ErrorSeverity::Error,
            FileSystemError::Permission { .. } => ErrorSeverity::Error,
            FileSystemError::Encoding { .. } => ErrorSeverity::Warning,
            FileSystemError::Platform(_) => ErrorSeverity::Critical,
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Warning,
    Error,
    Critical,
}

/// Detailed file information
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// File size in bytes
    pub size: u64,
    
    /// Last modified time
    pub modified: SystemTime,
    
    /// File permissions information
    pub permissions: FilePermissions,
    
    /// MIME type based on file extension
    pub mime_type: String,
    
    /// Whether the file is hidden
    pub is_hidden: bool,
    
    /// Platform-specific metadata
    pub metadata: HashMap<String, String>,
}

/// Cross-platform file permissions representation
#[derive(Debug, Clone)]
pub struct FilePermissions {
    /// Whether the file is readable
    pub readable: bool,
    
    /// Whether the file is writable
    pub writable: bool,
    
    /// Whether the file is executable
    pub executable: bool,
    
    /// Platform-specific permission details
    pub platform_details: HashMap<String, String>,
}

/// Supported media file extensions and their MIME types
pub const SUPPORTED_MEDIA_TYPES: &[(&str, &str)] = &[
    // Video formats
    ("mkv", "video/x-matroska"),
    ("mp4", "video/mp4"),
    ("avi", "video/x-msvideo"),
    ("mov", "video/quicktime"),
    ("wmv", "video/x-ms-wmv"),
    ("flv", "video/x-flv"),
    ("webm", "video/webm"),
    ("m4v", "video/x-m4v"),
    ("3gp", "video/3gpp"),
    ("mpg", "video/mpeg"),
    ("mpeg", "video/mpeg"),
    // Audio formats
    ("mp3", "audio/mpeg"),
    ("flac", "audio/flac"),
    ("wav", "audio/wav"),
    ("aac", "audio/aac"),
    ("ogg", "audio/ogg"),
    ("wma", "audio/x-ms-wma"),
    ("m4a", "audio/mp4"),
    ("opus", "audio/opus"),
    ("aiff", "audio/aiff"),
    // Image formats
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("png", "image/png"),
    ("gif", "image/gif"),
    ("bmp", "image/bmp"),
    ("tiff", "image/tiff"),
    ("webp", "image/webp"),
    ("svg", "image/svg+xml"),
];

/// Get MIME type for a file based on its extension
pub fn get_mime_type_for_extension(extension: &str) -> String {
    let ext_lower = extension.to_lowercase();
    SUPPORTED_MEDIA_TYPES
        .iter()
        .find(|(ext, _)| *ext == ext_lower)
        .map(|(_, mime)| mime.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string())
}

/// Check if a file extension is supported for media serving
pub fn is_supported_media_extension(extension: &str) -> bool {
    let ext_lower = extension.to_lowercase();
    SUPPORTED_MEDIA_TYPES
        .iter()
        .any(|(ext, _)| *ext == ext_lower)
}

/// Base implementation of FileSystemManager with common functionality
pub struct BaseFileSystemManager {
    /// Whether the file system is case-sensitive
    pub case_sensitive: bool,
}

impl BaseFileSystemManager {
    /// Create a new base file system manager
    pub fn new(case_sensitive: bool) -> Self {
        Self { case_sensitive }
    }
    
    /// Common path validation logic
    pub fn validate_path_common(&self, path: &Path) -> Result<(), FileSystemError> {
        // Check for null bytes
        if path.to_string_lossy().contains('\0') {
            return Err(FileSystemError::InvalidPath {
                path: path.display().to_string(),
                reason: "Path contains null bytes".to_string(),
            });
        }
        
        // Check for excessively long paths
        if path.to_string_lossy().len() > 4096 {
            return Err(FileSystemError::InvalidPath {
                path: path.display().to_string(),
                reason: "Path is too long".to_string(),
            });
        }
        
        // Check for directory traversal attempts
        let path_str = path.to_string_lossy();
        if path_str.contains("..") {
            return Err(FileSystemError::InvalidPath {
                path: path.display().to_string(),
                reason: "Path contains directory traversal".to_string(),
            });
        }
        
        Ok(())
    }
    
    /// Common file info extraction
    pub async fn get_file_info_common(&self, path: &Path) -> Result<FileInfo, FileSystemError> {
        let metadata = fs::metadata(path).await?;
        
        let permissions = FilePermissions {
            readable: !metadata.permissions().readonly(),
            writable: !metadata.permissions().readonly(),
            executable: false, // Will be overridden by platform-specific implementations
            platform_details: HashMap::new(),
        };
        
        let mime_type = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(get_mime_type_for_extension)
            .unwrap_or_else(|| "application/octet-stream".to_string());
        
        let is_hidden = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with('.'))
            .unwrap_or(false);
        
        Ok(FileInfo {
            size: metadata.len(),
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            permissions,
            mime_type,
            is_hidden,
            metadata: HashMap::new(),
        })
    }
    
    /// Common media file scanning logic
    pub async fn scan_directory_common(&self, path: &Path) -> Result<Vec<MediaFile>, FileSystemError> {
        let mut media_files = Vec::new();
        let mut entries = fs::read_dir(path).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            
            // Skip directories
            if entry_path.is_dir() {
                continue;
            }
            
            // Check if it's a supported media file
            if let Some(extension) = entry_path.extension().and_then(|ext| ext.to_str()) {
                if !is_supported_media_extension(extension) {
                    continue;
                }
                
                // Get file metadata
                let metadata = entry.metadata().await?;
                let filename = entry_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                
                let mime_type = get_mime_type_for_extension(extension);
                let now = SystemTime::now();
                
                media_files.push(MediaFile {
                    id: None,
                    path: entry_path,
                    filename,
                    size: metadata.len(),
                    modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    mime_type,
                    duration: None, // TODO: Extract from metadata
                    title: None,    // TODO: Extract from metadata
                    artist: None,   // TODO: Extract from metadata
                    album: None,    // TODO: Extract from metadata
                    created_at: now,
                    updated_at: now,
                });
            }
        }
        
        Ok(media_files)
    }
}

#[async_trait::async_trait]
impl FileSystemManager for BaseFileSystemManager {
    async fn scan_media_directory(&self, path: &Path) -> Result<Vec<MediaFile>, FileSystemError> {
        self.validate_path_common(path)?;
        
        if !self.is_accessible(path).await {
            return Err(FileSystemError::AccessDenied {
                path: path.display().to_string(),
                reason: "Directory is not accessible".to_string(),
            });
        }
        
        self.scan_directory_common(path).await
    }
    
    fn normalize_path(&self, path: &Path) -> PathBuf {
        // Basic normalization - platform-specific implementations will override
        path.to_path_buf()
    }
    
    async fn is_accessible(&self, path: &Path) -> bool {
        fs::metadata(path).await.is_ok()
    }
    
    async fn get_file_info(&self, path: &Path) -> Result<FileInfo, FileSystemError> {
        self.get_file_info_common(path).await
    }
    
    fn paths_equal(&self, path1: &Path, path2: &Path) -> bool {
        if self.case_sensitive {
            path1 == path2
        } else {
            path1.to_string_lossy().to_lowercase() == path2.to_string_lossy().to_lowercase()
        }
    }
    
    fn validate_path(&self, path: &Path) -> Result<(), FileSystemError> {
        self.validate_path_common(path)
    }
    
    async fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, FileSystemError> {
        fs::canonicalize(path).await.map_err(FileSystemError::from)
    }
    
    fn matches_extension(&self, path: &Path, extensions: &[String]) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_to_check = if self.case_sensitive {
                ext.to_string()
            } else {
                ext.to_lowercase()
            };
            
            extensions.iter().any(|allowed| {
                let allowed_ext = if self.case_sensitive {
                    allowed.clone()
                } else {
                    allowed.to_lowercase()
                };
                ext_to_check == allowed_ext
            })
        } else {
            false
        }
    }
}

/// Create a platform-specific file system manager
pub fn create_platform_filesystem_manager() -> Box<dyn FileSystemManager> {
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsFileSystemManager::new())
    }
    
    #[cfg(target_os = "macos")]
    {
        Box::new(BaseFileSystemManager::new(true)) // macOS is case-sensitive
    }
    
    #[cfg(target_os = "linux")]
    {
        Box::new(BaseFileSystemManager::new(true)) // Linux is case-sensitive
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    #[test]
    fn test_mime_type_detection() {
        assert_eq!(get_mime_type_for_extension("mp4"), "video/mp4");
        assert_eq!(get_mime_type_for_extension("MP4"), "video/mp4");
        assert_eq!(get_mime_type_for_extension("mp3"), "audio/mpeg");
        assert_eq!(get_mime_type_for_extension("unknown"), "application/octet-stream");
    }
    
    #[test]
    fn test_supported_extension_check() {
        assert!(is_supported_media_extension("mp4"));
        assert!(is_supported_media_extension("MP4"));
        assert!(is_supported_media_extension("mp3"));
        assert!(!is_supported_media_extension("txt"));
        assert!(!is_supported_media_extension("unknown"));
    }
    
    #[test]
    fn test_path_validation() {
        let manager = BaseFileSystemManager::new(true);
        
        // Valid paths
        assert!(manager.validate_path_common(Path::new("/valid/path")).is_ok());
        assert!(manager.validate_path_common(Path::new("relative/path")).is_ok());
        
        // Invalid paths
        assert!(manager.validate_path_common(Path::new("path/with/\0/null")).is_err());
        assert!(manager.validate_path_common(Path::new("path/../traversal")).is_err());
    }
    
    #[test]
    fn test_case_sensitivity() {
        let case_sensitive = BaseFileSystemManager::new(true);
        let case_insensitive = BaseFileSystemManager::new(false);
        
        let path1 = Path::new("/Test/Path");
        let path2 = Path::new("/test/path");
        
        assert!(!case_sensitive.paths_equal(path1, path2));
        assert!(case_insensitive.paths_equal(path1, path2));
    }
    
    #[test]
    fn test_extension_matching() {
        let case_sensitive = BaseFileSystemManager::new(true);
        let case_insensitive = BaseFileSystemManager::new(false);
        
        let path = Path::new("test.MP4");
        let extensions = vec!["mp4".to_string(), "avi".to_string()];
        
        assert!(!case_sensitive.matches_extension(path, &extensions));
        assert!(case_insensitive.matches_extension(path, &extensions));
    }
}
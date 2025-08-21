use super::{BaseFileSystemManager, FileInfo, FilePermissions, FileSystemError, FileSystemManager, MediaFile};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::fs;

/// Windows-specific file system manager
pub struct WindowsFileSystemManager {
    base: BaseFileSystemManager,
}

impl WindowsFileSystemManager {
    /// Create a new Windows file system manager
    pub fn new() -> Self {
        Self {
            base: BaseFileSystemManager::new(false), // Windows NTFS is case-insensitive by default
        }
    }
    
    /// Check if a path is a UNC path (\\server\share\path)
    fn is_unc_path(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        path_str.starts_with(r"\\") && path_str.len() > 2
    }
    
    /// Check if a path contains a drive letter (C:\path)
    fn has_drive_letter(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        path_str.len() >= 3 && 
        path_str.chars().nth(1) == Some(':') && 
        path_str.chars().nth(2) == Some('\\')
    }
    
    /// Normalize Windows path separators, case, and handle drive letters
    fn normalize_windows_path(&self, path: &Path) -> PathBuf {
        let path_str = path.to_string_lossy();
        
        // Convert forward slashes to backslashes and path to lowercase
        let normalized = path_str.replace('/', r"\").to_lowercase();
        
        // Handle UNC paths - don't modify server/share case
        if self.is_unc_path(path) {
            if let Some(first_sep) = normalized.find('\\').and_then(|p| normalized[p+1..].find('\\')) {
                let server_share_part_end = 1 + first_sep + 1;
                 if let Some(second_sep) = normalized[server_share_part_end..].find('\\') {
                    let server_share = &normalized[..server_share_part_end + second_sep];
                    let rest_of_path = &normalized[server_share_part_end + second_sep..];
                    return PathBuf::from(format!("{}{}", server_share, rest_of_path.to_lowercase()));
                }
            }
            return PathBuf::from(normalized);
        }
        
        PathBuf::from(normalized)
    }
    
    /// Validate Windows-specific path constraints
    fn validate_windows_path(&self, path: &Path) -> Result<(), FileSystemError> {
        // First run common validation
        self.base.validate_path_common(path)?;
        
        let path_str = path.to_string_lossy();
        
        // Check for invalid Windows characters (excluding colon which is handled separately)
        let invalid_chars = ['<', '>', '"', '|', '?', '*'];
        for &invalid_char in &invalid_chars {
            if path_str.contains(invalid_char) {
                return Err(FileSystemError::InvalidWindowsCharacter {
                    path: path.display().to_string(),
                    character: invalid_char,
                    reason: format!("Windows paths cannot contain the '{}' character. This character is reserved by the Windows file system.", invalid_char),
                });
            }
        }
        
        // Handle colon validation separately with proper logic
        if path_str.contains(':') {
            if !self.is_valid_colon_usage(path) {
                let colon_details = self.get_colon_validation_details(path);
                return Err(FileSystemError::InvalidColonUsage {
                    path: path.display().to_string(),
                    details: colon_details,
                });
            }
        }
        
        // Check for reserved Windows names
        let reserved_names = [
            "CON", "PRN", "AUX", "NUL",
            "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
            "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ];
        
        if let Some(filename) = path.file_name().and_then(|name| name.to_str()) {
            let filename_upper = filename.to_uppercase();
            let name_without_ext = filename_upper.split('.').next().unwrap_or(&filename_upper);
            
            if reserved_names.contains(&name_without_ext) {
                return Err(FileSystemError::ReservedName {
                    path: path.display().to_string(),
                    reserved_name: name_without_ext.to_string(),
                });
            }
        }
        
        // Check path length limits
        if path_str.len() > 260 && !path_str.starts_with(r"\\?\") {
            return Err(FileSystemError::PathTooLong {
                path: path.display().to_string(),
                details: format!("Path length is {} characters, which exceeds the Windows MAX_PATH limit of 260 characters. Consider using shorter names or enabling long path support.", path_str.len()),
            });
        }
        
        Ok(())
    }
    
    /// Get Windows-specific file permissions
    async fn get_windows_permissions(&self, path: &Path) -> Result<FilePermissions, FileSystemError> {
        let metadata = fs::metadata(path).await?;
        let std_permissions = metadata.permissions();
        
        let mut platform_details = HashMap::new();
        
        // On Windows, we can check basic read-only status
        let readonly = std_permissions.readonly();
        platform_details.insert("readonly".to_string(), readonly.to_string());
        
        // For more detailed Windows ACL information, we would need to use Windows APIs
        // This is a simplified implementation
        let permissions = FilePermissions {
            readable: true, // If we can read metadata, we can likely read the file
            writable: !readonly,
            executable: path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| {
                    let ext_lower = ext.to_lowercase();
                    matches!(ext_lower.as_str(), "exe" | "bat" | "cmd" | "com" | "scr" | "msi")
                })
                .unwrap_or(false),
            platform_details,
        };
        
        Ok(permissions)
    }
    
    /// Check if a file is hidden on Windows
    fn is_hidden_windows(&self, path: &Path) -> bool {
        // On Windows, files starting with '.' are not necessarily hidden
        // The hidden attribute is set via file attributes, but we'll use a simple heuristic
        if let Some(filename) = path.file_name().and_then(|name| name.to_str()) {
            // Common Windows hidden files
            matches!(filename.to_lowercase().as_str(), 
                "thumbs.db" | "desktop.ini" | ".ds_store" | "hiberfil.sys" | "pagefile.sys"
            ) || filename.starts_with('.')
        } else {
            false
        }
    }
    
    /// Handle case-insensitive file matching for Windows
    fn find_actual_case(&self, path: &Path) -> Option<PathBuf> {
        // This is a simplified implementation
        // In a full implementation, we would use Windows APIs to find the actual case
        // For now, we'll just return the path as-is since Windows is case-insensitive
        Some(path.to_path_buf())
    }
    
    /// Check if colon usage in the path is valid for Windows
    fn is_valid_colon_usage(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        
        // UNC paths can have colons in network addresses (\\server:port\share)
        if self.is_unc_path(path) {
            return self.validate_unc_colon_usage(&path_str);
        }
        
        // Drive letter paths should only have colon at position 1
        if self.has_drive_letter(path) || self.looks_like_drive_letter(&path_str) {
            return self.validate_drive_letter_colon_usage(&path_str);
        }
        
        // Relative paths should not contain colons
        !path_str.contains(':')
    }
    
    /// Check if a path string looks like it starts with a drive letter (more flexible than has_drive_letter)
    fn looks_like_drive_letter(&self, path_str: &str) -> bool {
        path_str.len() >= 2 && 
        path_str.chars().nth(1) == Some(':') &&
        path_str.chars().nth(0).map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
    }
    
    /// Validate colon usage in drive letter paths
    fn validate_drive_letter_colon_usage(&self, path_str: &str) -> bool {
        // Find all colon positions
        let colon_positions: Vec<usize> = path_str.match_indices(':').map(|(i, _)| i).collect();
        
        // Should only have one colon at position 1 (after drive letter)
        colon_positions.len() == 1 && colon_positions[0] == 1
    }
    
    /// Validate colon usage in UNC paths
    fn validate_unc_colon_usage(&self, path_str: &str) -> bool {
        // UNC paths: \\server:port\share or \\server\share
        // Colons are allowed in the server:port portion
        if !path_str.starts_with(r"\\") {
            return false;
        }
        
        // Split into components: ["", "", "server:port", "share", ...]
        let components: Vec<&str> = path_str.split('\\').collect();
        if components.len() < 4 {
            return false; // Invalid UNC path structure
        }
        
        // Check if colons only appear in the server component (index 2)
        for (i, component) in components.iter().enumerate() {
            if component.contains(':') && i != 2 {
                return false; // Colon in wrong position
            }
        }
        
        true
    }
    
    /// Get detailed information about why colon validation failed
    fn get_colon_validation_details(&self, path: &Path) -> String {
        let path_str = path.to_string_lossy();
        let colon_positions: Vec<usize> = path_str.match_indices(':').map(|(i, _)| i).collect();
        
        if colon_positions.is_empty() {
            return "No colons found in path".to_string();
        }
        
        let mut details = Vec::new();
        
        // Check if it's a UNC path
        if path_str.starts_with(r"\\") {
            details.push("Detected UNC network path format".to_string());
            
            let components: Vec<&str> = path_str.split('\\').collect();
            if components.len() < 4 {
                details.push("Invalid UNC path structure - should be \\\\server\\share or \\\\server:port\\share".to_string());
            } else {
                for (i, component) in components.iter().enumerate() {
                    if component.contains(':') {
                        if i == 2 {
                            details.push(format!("Valid colon usage in server component: '{}'", component));
                        } else {
                            details.push(format!("Invalid colon in component {} ('{}'): colons only allowed in server component", i, component));
                        }
                    }
                }
            }
        }
        // Check if it looks like a drive letter path
        else if self.looks_like_drive_letter(&path_str) {
            details.push("Detected drive letter path format".to_string());
            
            if colon_positions.len() == 1 && colon_positions[0] == 1 {
                details.push("Valid drive letter colon usage".to_string());
            } else if colon_positions.len() > 1 {
                details.push(format!("Multiple colons found at positions: {:?} - only one colon allowed at position 1", colon_positions));
            } else if colon_positions[0] != 1 {
                details.push(format!("Colon at wrong position {} - drive letter colon must be at position 1", colon_positions[0]));
            }
        }
        // Relative path or other format
        else {
            details.push("Relative or non-standard path format".to_string());
            details.push(format!("Found colons at positions: {:?} - colons not allowed in relative paths", colon_positions));
            details.push("Suggestion: Remove colons from file/directory names or use absolute paths with proper drive letters".to_string());
        }
        
        // Add examples of valid usage
        if details.iter().any(|d| d.contains("Invalid") || d.contains("not allowed")) {
            details.push("Valid colon usage examples:".to_string());
            details.push("  - Drive letters: C:\\Users\\Documents, D:\\Media".to_string());
            details.push("  - UNC paths: \\\\server\\share, \\\\192.168.1.100:8080\\media".to_string());
        }
        
        details.join("\n")
    }
}

#[async_trait::async_trait]
impl FileSystemManager for WindowsFileSystemManager {
    async fn scan_media_directory(&self, path: &Path) -> Result<Vec<MediaFile>, FileSystemError> {
        self.validate_windows_path(path)?;
        
        if !self.is_accessible(path).await {
            return Err(FileSystemError::AccessDenied {
                path: path.display().to_string(),
                reason: "Directory is not accessible or requires elevated permissions".to_string(),
            });
        }
        
        // Use the base implementation for scanning, but with Windows-specific path handling
        let normalized_path = self.normalize_windows_path(path);
        self.base.scan_directory_common(&normalized_path).await
    }
    
    fn normalize_path(&self, path: &Path) -> PathBuf {
        self.normalize_windows_path(path)
    }
    
    async fn is_accessible(&self, path: &Path) -> bool {
        let normalized_path = self.normalize_windows_path(path);
        
        // Try to access the path
        match fs::metadata(&normalized_path).await {
            Ok(_) => true,
            Err(err) => {
                // Log the specific error for debugging
                tracing::debug!("Path not accessible: {} - {}", normalized_path.display(), err);
                false
            }
        }
    }
    
    async fn get_file_info(&self, path: &Path) -> Result<FileInfo, FileSystemError> {
        let normalized_path = self.normalize_windows_path(path);
        let metadata = fs::metadata(&normalized_path).await?;
        
        let permissions = self.get_windows_permissions(&normalized_path).await?;
        
        let mime_type = normalized_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(super::get_mime_type_for_extension)
            .unwrap_or_else(|| "application/octet-stream".to_string());
        
        let is_hidden = self.is_hidden_windows(&normalized_path);
        
        let mut platform_metadata = HashMap::new();
        platform_metadata.insert("is_unc_path".to_string(), self.is_unc_path(&normalized_path).to_string());
        platform_metadata.insert("has_drive_letter".to_string(), self.has_drive_letter(&normalized_path).to_string());
        
        Ok(FileInfo {
            size: metadata.len(),
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            permissions,
            mime_type,
            is_hidden,
            metadata: platform_metadata,
        })
    }
    
    fn paths_equal(&self, path1: &Path, path2: &Path) -> bool {
        // Windows paths are case-insensitive
        let norm1 = self.normalize_windows_path(path1);
        let norm2 = self.normalize_windows_path(path2);
        
        norm1.to_string_lossy().to_lowercase() == norm2.to_string_lossy().to_lowercase()
    }
    
    fn validate_path(&self, path: &Path) -> Result<(), FileSystemError> {
        self.validate_windows_path(path)
    }
    
    async fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, FileSystemError> {
        let normalized_path = self.normalize_windows_path(path);
        
        match fs::canonicalize(&normalized_path).await {
            Ok(canonical) => Ok(canonical),
            Err(err) => {
                // If canonicalization fails, try to provide a helpful error
                if err.kind() == std::io::ErrorKind::NotFound {
                    Err(FileSystemError::PathNotFound {
                        path: normalized_path.display().to_string(),
                    })
                } else if err.kind() == std::io::ErrorKind::PermissionDenied {
                    Err(FileSystemError::AccessDenied {
                        path: normalized_path.display().to_string(),
                        reason: "Permission denied when accessing path".to_string(),
                    })
                } else {
                    Err(FileSystemError::Platform(format!(
                        "Windows canonicalization failed: {}", err
                    )))
                }
            }
        }
    }
    
    fn matches_extension(&self, path: &Path, extensions: &[String]) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            // Windows is case-insensitive
            let ext_lower = ext.to_lowercase();
            extensions.iter().any(|allowed| {
                allowed.to_lowercase() == ext_lower
            })
        } else {
            false
        }
    }
}

impl Default for WindowsFileSystemManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;
    
    #[test]
    fn test_unc_path_detection() {
        let manager = WindowsFileSystemManager::new();
        
        assert!(manager.is_unc_path(Path::new(r"\\server\share\path")));
        assert!(manager.is_unc_path(Path::new(r"\\192.168.1.100\media")));
        assert!(!manager.is_unc_path(Path::new(r"C:\local\path")));
        assert!(!manager.is_unc_path(Path::new(r"relative\path")));
    }
    
    #[test]
    fn test_drive_letter_detection() {
        let manager = WindowsFileSystemManager::new();
        
        assert!(manager.has_drive_letter(Path::new(r"C:\path")));
        assert!(manager.has_drive_letter(Path::new(r"D:\another\path")));
        assert!(!manager.has_drive_letter(Path::new(r"\\server\share")));
        assert!(!manager.has_drive_letter(Path::new(r"relative\path")));
    }
    
    #[test]
    fn test_path_normalization() {
        let manager = WindowsFileSystemManager::new();
        
        // Test forward slash conversion and case
        let normalized = manager.normalize_windows_path(Path::new("C:/Path/To/File"));
        assert_eq!(normalized, PathBuf::from(r"c:\path\to\file"));
        
        // Test UNC path preservation (server/share part)
        let unc_path = Path::new(r"\\Server\Share\SubFolder");
        let normalized = manager.normalize_windows_path(unc_path);
        assert_eq!(normalized, PathBuf::from(r"\\server\share\subfolder"));
    }
    
    #[test]
    fn test_reserved_name_validation() {
        let manager = WindowsFileSystemManager::new();
        
        // Test reserved names
        assert!(manager.validate_windows_path(Path::new(r"C:\CON")).is_err());
        assert!(manager.validate_windows_path(Path::new(r"C:\PRN.txt")).is_err());
        assert!(manager.validate_windows_path(Path::new(r"C:\COM1")).is_err());
        assert!(manager.validate_windows_path(Path::new(r"C:\LPT1.dat")).is_err());
        
        // Test valid names
        assert!(manager.validate_windows_path(Path::new(r"C:\CONSOLE")).is_ok());
        assert!(manager.validate_windows_path(Path::new(r"C:\PRINTER.txt")).is_ok());
    }
    
    #[test]
    fn test_invalid_character_validation() {
        let manager = WindowsFileSystemManager::new();
        
        // Test invalid characters (excluding drive letters and UNC paths)
        assert!(manager.validate_windows_path(Path::new(r"C:\path\file<name")).is_err());
        assert!(manager.validate_windows_path(Path::new(r"C:\path\file>name")).is_err());
        assert!(manager.validate_windows_path(Path::new(r"C:\path\file|name")).is_err());
        assert!(manager.validate_windows_path(Path::new(r"C:\path\file?name")).is_err());
        assert!(manager.validate_windows_path(Path::new(r"C:\path\file*name")).is_err());
        
        // Test valid characters
        assert!(manager.validate_windows_path(Path::new(r"C:\path\filename")).is_ok());
        assert!(manager.validate_windows_path(Path::new(r"C:\path\file-name_123")).is_ok());
    }
    
    #[test]
    fn test_case_insensitive_comparison() {
        let manager = WindowsFileSystemManager::new();
        
        let path1 = Path::new(r"C:\Path\To\File.txt");
        let path2 = Path::new(r"c:\path\to\file.TXT");
        
        assert!(manager.paths_equal(path1, path2));
    }
    
    #[test]
    fn test_extension_matching() {
        let manager = WindowsFileSystemManager::new();
        
        let path = Path::new(r"C:\path\file.MP4");
        let extensions = vec!["mp4".to_string(), "avi".to_string()];
        
        // Should match case-insensitively on Windows
        assert!(manager.matches_extension(path, &extensions));
    }
    
    #[test]
    fn test_hidden_file_detection() {
        let manager = WindowsFileSystemManager::new();
        
        assert!(manager.is_hidden_windows(Path::new(r"C:\path\Thumbs.db")));
        assert!(manager.is_hidden_windows(Path::new(r"C:\path\desktop.ini")));
        assert!(manager.is_hidden_windows(Path::new(r"C:\path\.hidden")));
        assert!(!manager.is_hidden_windows(Path::new(r"C:\path\normal.txt")));
    }
    
    #[test]
    fn test_valid_drive_letter_paths() {
        let manager = WindowsFileSystemManager::new();
        
        // Valid drive letter paths
        assert!(manager.validate_windows_path(Path::new(r"C:\Users\Welcome\Videos")).is_ok());
        assert!(manager.validate_windows_path(Path::new(r"D:\Media\Movies")).is_ok());
        assert!(manager.validate_windows_path(Path::new(r"Z:\")).is_ok());
        assert!(manager.validate_windows_path(Path::new(r"C:")).is_ok());
        assert!(manager.validate_windows_path(Path::new(r"C:/path/with/forward/slashes")).is_ok());
    }
    
    #[test]
    fn test_valid_unc_paths() {
        let manager = WindowsFileSystemManager::new();
        
        // Valid UNC paths
        assert!(manager.validate_windows_path(Path::new(r"\\server\share")).is_ok());
        assert!(manager.validate_windows_path(Path::new(r"\\192.168.1.100\media")).is_ok());
        assert!(manager.validate_windows_path(Path::new(r"\\server:8080\share")).is_ok());
        assert!(manager.validate_windows_path(Path::new(r"\\server:443\share\subfolder")).is_ok());
    }
    
    #[test]
    fn test_invalid_colon_usage() {
        let manager = WindowsFileSystemManager::new();
        
        // Invalid colon usage
        assert!(manager.validate_windows_path(Path::new(r"C:\path\file:name")).is_err());
        assert!(manager.validate_windows_path(Path::new(r"relative\path:name")).is_err());
        assert!(manager.validate_windows_path(Path::new(r"\\server\share:invalid")).is_err());
        assert!(manager.validate_windows_path(Path::new(r"C:D:\invalid")).is_err());
        assert!(manager.validate_windows_path(Path::new(r"path:with:multiple:colons")).is_err());
    }
    
    #[test]
    fn test_colon_validation_details() {
        let manager = WindowsFileSystemManager::new();
        
        // Test drive letter colon validation
        assert!(manager.validate_drive_letter_colon_usage("C:\\path"));
        assert!(manager.validate_drive_letter_colon_usage("D:"));
        assert!(!manager.validate_drive_letter_colon_usage("C:\\path:invalid"));
        assert!(!manager.validate_drive_letter_colon_usage("C:D:\\invalid"));
        assert!(!manager.validate_drive_letter_colon_usage("path:invalid"));
        
        // Test UNC colon validation
        assert!(manager.validate_unc_colon_usage(r"\\server:8080\share"));
        assert!(manager.validate_unc_colon_usage(r"\\server\share"));
        assert!(manager.validate_unc_colon_usage(r"\\192.168.1.100:443\share\subfolder"));
        assert!(!manager.validate_unc_colon_usage(r"\\server\share:invalid"));
        assert!(!manager.validate_unc_colon_usage(r"\\server:8080\share:invalid"));
        assert!(!manager.validate_unc_colon_usage(r"not\unc\path:invalid"));
        
        // Test looks_like_drive_letter helper
        assert!(manager.looks_like_drive_letter("C:"));
        assert!(manager.looks_like_drive_letter("C:\\path"));
        assert!(manager.looks_like_drive_letter("D:/path"));
        assert!(!manager.looks_like_drive_letter("\\\\server"));
        assert!(!manager.looks_like_drive_letter("relative"));
        assert!(!manager.looks_like_drive_letter("1:invalid"));
    }
    
    #[test]
    fn test_is_valid_colon_usage() {
        let manager = WindowsFileSystemManager::new();
        
        // Valid colon usage
        assert!(manager.is_valid_colon_usage(Path::new(r"C:\path")));
        assert!(manager.is_valid_colon_usage(Path::new(r"D:")));
        assert!(manager.is_valid_colon_usage(Path::new(r"\\server:8080\share")));
        assert!(manager.is_valid_colon_usage(Path::new(r"\\server\share")));
        assert!(manager.is_valid_colon_usage(Path::new(r"relative\path\without\colons")));
        
        // Invalid colon usage
        assert!(!manager.is_valid_colon_usage(Path::new(r"C:\path\file:name")));
        assert!(!manager.is_valid_colon_usage(Path::new(r"relative\path:name")));
        assert!(!manager.is_valid_colon_usage(Path::new(r"\\server\share:invalid")));
        assert!(!manager.is_valid_colon_usage(Path::new(r"C:D:\invalid")));
    }
}
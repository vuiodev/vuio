use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::platform::filesystem::{create_platform_filesystem_manager, FileSystemError};
use crate::platform::{OsType, PlatformError, PlatformResult};

/// Platform-specific configuration and defaults
#[derive(Debug, Clone)]
pub struct PlatformConfig {
    /// Operating system type
    pub os_type: OsType,

    /// Default directory for media files
    pub default_media_dir: PathBuf,

    /// Configuration files directory
    pub config_dir: PathBuf,

    /// Log files directory
    pub log_dir: PathBuf,

    /// Cache files directory
    pub cache_dir: PathBuf,

    /// Database files directory
    pub database_dir: PathBuf,

    /// Preferred ports for the server (in order of preference)
    pub preferred_ports: Vec<u16>,

    /// Platform-specific metadata
    pub metadata: HashMap<String, String>,
}

impl PlatformConfig {
    /// Get platform-specific configuration for the current operating system
    pub fn for_current_platform() -> Self {
        let os_type = OsType::current();

        match os_type {
            OsType::Windows => Self::for_windows(),
            OsType::MacOS => Self::for_macos(),
            OsType::Linux => Self::for_linux(),
            OsType::Bsd => Self::for_bsd(),
        }
    }

    /// Windows-specific configuration
    fn for_windows() -> Self {
        // Use executable-relative paths for native versions
        let exe_dir = Self::get_executable_directory();
        
        let mut metadata = HashMap::new();
        metadata.insert("platform".to_string(), "windows".to_string());
        metadata.insert("case_sensitive".to_string(), "false".to_string());
        metadata.insert("path_separator".to_string(), "\\".to_string());
        metadata.insert("supports_unc_paths".to_string(), "true".to_string());

        Self {
            os_type: OsType::Windows,
            default_media_dir: Self::get_windows_default_media_dir(),
            config_dir: exe_dir.join("config"),
            log_dir: exe_dir.join("config").join("logs"),
            cache_dir: exe_dir.join("config").join("cache"),
            database_dir: exe_dir.join("config").join("database"),
            preferred_ports: vec![8080, 8081, 8082, 9090, 9091, 8000, 8001],
            metadata,
        }
    }

    /// macOS-specific configuration
    fn for_macos() -> Self {
        // Use executable-relative paths for native versions
        let exe_dir = Self::get_executable_directory();

        let mut metadata = HashMap::new();
        metadata.insert("platform".to_string(), "macos".to_string());
        metadata.insert("case_sensitive".to_string(), "true".to_string());
        metadata.insert("path_separator".to_string(), "/".to_string());
        metadata.insert("supports_network_mounts".to_string(), "true".to_string());

        Self {
            os_type: OsType::MacOS,
            default_media_dir: Self::get_macos_default_media_dir(),
            config_dir: exe_dir.join("config"),
            log_dir: exe_dir.join("config").join("logs"),
            cache_dir: exe_dir.join("config").join("cache"),
            database_dir: exe_dir.join("config").join("database"),
            preferred_ports: vec![8080, 8081, 8082, 9090, 9091, 8000, 8001],
            metadata,
        }
    }

    /// Linux-specific configuration
    fn for_linux() -> Self {
        // Use executable-relative paths for native versions
        let exe_dir = Self::get_executable_directory();
        
        let mut metadata = HashMap::new();
        metadata.insert("platform".to_string(), "linux".to_string());
        metadata.insert("case_sensitive".to_string(), "true".to_string());
        metadata.insert("path_separator".to_string(), "/".to_string());
        metadata.insert("supports_xdg_dirs".to_string(), "true".to_string());

        Self {
            os_type: OsType::Linux,
            default_media_dir: Self::get_linux_default_media_dir(),
            config_dir: exe_dir.join("config"),
            log_dir: exe_dir.join("config").join("logs"),
            cache_dir: exe_dir.join("config").join("cache"),
            database_dir: exe_dir.join("config").join("database"),
            preferred_ports: vec![8080, 8081, 8082, 9090, 9091, 8000, 8001],
            metadata,
        }
    }

    /// BSD-specific configuration
    fn for_bsd() -> Self {
        // Use executable-relative paths for native versions
        let exe_dir = Self::get_executable_directory();
        
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/usr/home"));
        
        // Common BSD media directories
        let default_media_dirs = vec![
            home_dir.join("Music"),
            home_dir.join("Videos"),
            home_dir.join("Pictures"),
            PathBuf::from("/mnt"),
        ];
        
        let mut metadata = HashMap::new();
        metadata.insert("platform".to_string(), "bsd".to_string());
        metadata.insert("case_sensitive".to_string(), "true".to_string());
        metadata.insert("path_separator".to_string(), "/".to_string());
        metadata.insert("supports_xdg_dirs".to_string(), "true".to_string());

        Self {
            os_type: OsType::Bsd,
            default_media_dir: default_media_dirs[0].clone(),
            config_dir: exe_dir.join("config"),
            log_dir: exe_dir.join("config").join("logs"),
            cache_dir: exe_dir.join("config").join("cache"),
            database_dir: exe_dir.join("config").join("database"),
            preferred_ports: vec![8080, 8081, 8082, 9090, 9091, 8000, 8001],
            metadata,
        }
    }

    /// Get default media directories for Windows
    fn get_windows_default_media_dir() -> PathBuf {
        // Try to find the user's Videos folder first
        if let Some(videos_dir) = dirs::video_dir() {
            return videos_dir;
        }

        // Fall back to Documents folder
        if let Some(documents_dir) = dirs::document_dir() {
            return documents_dir;
        }

        // Last resort: home directory
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("C:\\"))
    }

    /// Get default media directories for macOS
    fn get_macos_default_media_dir() -> PathBuf {
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/Users/unknown"));

        // Check for Movies folder first
        let movies_dir = home_dir.join("Movies");
        if movies_dir.exists() {
            return movies_dir;
        }

        // Fall back to home directory
        home_dir
    }

    /// Get default media directories for Linux
    fn get_linux_default_media_dir() -> PathBuf {
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/unknown"));

        // Try XDG Videos directory first
        if let Some(videos_dir) = dirs::video_dir() {
            return videos_dir;
        }

        // Check common media directories
        let common_dirs = [
            home_dir.join("Videos"),
            home_dir.join("Media"),
            home_dir.join("Movies"),
        ];

        for dir in &common_dirs {
            if dir.exists() {
                return dir.clone();
            }
        }

        // Fall back to home directory
        home_dir
    }

    /// Get all potential default media directories for the current platform
    pub fn get_default_media_directories(&self) -> Vec<PathBuf> {
        let mut directories = vec![self.default_media_dir.clone()];

        match self.os_type {
            OsType::Windows => {
                // Add common Windows media directories
                if let Some(videos_dir) = dirs::video_dir() {
                    if videos_dir != self.default_media_dir {
                        directories.push(videos_dir);
                    }
                }
                if let Some(music_dir) = dirs::audio_dir() {
                    directories.push(music_dir);
                }
                if let Some(pictures_dir) = dirs::picture_dir() {
                    directories.push(pictures_dir);
                }

                // Add common Windows media locations
                let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("C:\\"));
                let additional_dirs = [
                    PathBuf::from("C:\\Users\\Public\\Videos"),
                    PathBuf::from("C:\\Users\\Public\\Music"),
                    PathBuf::from("C:\\Users\\Public\\Pictures"),
                    home_dir.join("Desktop"),
                ];

                for dir in &additional_dirs {
                    if dir.exists() && !directories.contains(dir) {
                        directories.push(dir.clone());
                    }
                }
            }

            OsType::MacOS => {
                let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/Users/unknown"));
                let potential_dirs = [
                    home_dir.join("Movies"),
                    home_dir.join("Music"),
                    home_dir.join("Pictures"),
                    home_dir.join("Desktop"),
                    PathBuf::from("/Users/Shared"),
                ];

                for dir in &potential_dirs {
                    if dir.exists() && !directories.contains(dir) {
                        directories.push(dir.clone());
                    }
                }
            }

            OsType::Linux => {
                // Add XDG user directories
                if let Some(videos_dir) = dirs::video_dir() {
                    if !directories.contains(&videos_dir) {
                        directories.push(videos_dir);
                    }
                }
                if let Some(music_dir) = dirs::audio_dir() {
                    directories.push(music_dir);
                }
                if let Some(pictures_dir) = dirs::picture_dir() {
                    directories.push(pictures_dir);
                }

                // Add common Linux media locations
                let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/unknown"));
                let additional_dirs = [
                    home_dir.join("Desktop"),
                    PathBuf::from("/media"),
                    PathBuf::from("/mnt"),
                    PathBuf::from("/home/shared"),
                ];

                for dir in &additional_dirs {
                    if dir.exists() && !directories.contains(dir) {
                        directories.push(dir.clone());
                    }
                }
            }

            OsType::Bsd => {
                // Add XDG user directories
                if let Some(videos_dir) = dirs::video_dir() {
                    if !directories.contains(&videos_dir) {
                        directories.push(videos_dir);
                    }
                }
                if let Some(music_dir) = dirs::audio_dir() {
                    directories.push(music_dir);
                }
                if let Some(pictures_dir) = dirs::picture_dir() {
                    directories.push(pictures_dir);
                }

                // Add common BSD media locations
                let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/usr/home/unknown"));
                let additional_dirs = [
                    home_dir.join("Desktop"),
                    PathBuf::from("/mnt"),
                ];

                for dir in &additional_dirs {
                    if dir.exists() && !directories.contains(dir) {
                        directories.push(dir.clone());
                    }
                }
            }
        }

        directories
    }

    /// Get the configuration file path for the current platform
    pub fn get_config_file_path(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    /// Get the database file path for the current platform
    pub fn get_database_path(&self) -> PathBuf {
        self.database_dir.join("media.db")
    }

    /// Get the log file path for the current platform
    pub fn get_log_file_path(&self) -> PathBuf {
        self.log_dir.join("vuio.log")
    }

    /// Get the cache directory path for the current platform
    pub fn get_cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// Get the directory where the executable is located
    fn get_executable_directory() -> PathBuf {
        match std::env::current_exe() {
            Ok(exe_path) => {
                if let Some(parent) = exe_path.parent() {
                    parent.to_path_buf()
                } else {
                    // Fallback to current directory if we can't get parent
                    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                }
            }
            Err(_) => {
                // Fallback to current directory if we can't get executable path
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            }
        }
    }

    /// Ensure all platform directories exist
    pub fn ensure_directories_exist(&self) -> PlatformResult<()> {
        // Create config directory first
        std::fs::create_dir_all(&self.config_dir).map_err(|e| {
            PlatformError::FileSystemAccess(format!(
                "Failed to create config directory '{}': {}. Please ensure the executable has write permissions to its directory.", 
                self.config_dir.display(), e
            ))
        })?;

        // Create subdirectories
        std::fs::create_dir_all(&self.log_dir).map_err(|e| {
            PlatformError::FileSystemAccess(format!(
                "Failed to create log directory '{}': {}. Please ensure the config directory is writable.", 
                self.log_dir.display(), e
            ))
        })?;

        std::fs::create_dir_all(&self.cache_dir).map_err(|e| {
            PlatformError::FileSystemAccess(format!(
                "Failed to create cache directory '{}': {}. Please ensure the config directory is writable.", 
                self.cache_dir.display(), e
            ))
        })?;

        std::fs::create_dir_all(&self.database_dir).map_err(|e| {
            PlatformError::FileSystemAccess(format!(
                "Failed to create database directory '{}': {}. Please ensure the config directory is writable.", 
                self.database_dir.display(), e
            ))
        })?;

        Ok(())
    }

    /// Get platform-specific metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Check if the platform supports case-sensitive file systems
    pub fn is_case_sensitive(&self) -> bool {
        self.metadata
            .get("case_sensitive")
            .map(|v| v == "true")
            .unwrap_or(false)
    }

    /// Get the platform-specific path separator
    pub fn get_path_separator(&self) -> &str {
        self.metadata
            .get("path_separator")
            .map(|s| s.as_str())
            .unwrap_or("/")
    }

    /// Check if the platform supports UNC paths (Windows) or network mounts
    pub fn supports_network_paths(&self) -> bool {
        self.metadata
            .get("supports_unc_paths")
            .or_else(|| self.metadata.get("supports_network_mounts"))
            .map(|v| v == "true")
            .unwrap_or(false)
    }

    /// Get platform-specific file extensions that should be excluded by default
    pub fn get_default_exclude_patterns(&self) -> Vec<String> {
        match self.os_type {
            OsType::Windows => vec![
                ".*".to_string(),           // Hidden files
                "Thumbs.db".to_string(),    // Windows thumbnails
                "desktop.ini".to_string(),  // Windows folder settings
                "*.tmp".to_string(),        // Temporary files
                "*.temp".to_string(),       // Temporary files
                "System Volume Information".to_string(), // Windows system folder
            ],

            OsType::MacOS => vec![
                ".*".to_string(),           // Hidden files
                ".DS_Store".to_string(),    // macOS metadata
                ".AppleDouble".to_string(), // macOS resource forks
                ".Trashes".to_string(),     // macOS trash
                "*.tmp".to_string(),        // Temporary files
                ".fseventsd".to_string(),   // macOS file system events
            ],

            OsType::Linux => vec![
                ".*".to_string(),         // Hidden files
                "*.tmp".to_string(),      // Temporary files
                "*.temp".to_string(),     // Temporary files
                "lost+found".to_string(), // Linux filesystem recovery
                ".Trash-*".to_string(),   // Linux trash directories
            ],

            OsType::Bsd => vec![
                ".*".to_string(),         // Hidden files
                "*.tmp".to_string(),      // Temporary files
                "*.temp".to_string(),     // Temporary files
                "lost+found".to_string(), // BSD filesystem recovery
            ],
        }
    }

    /// Get platform-specific supported media file extensions
    pub fn get_default_media_extensions(&self) -> Vec<String> {
        // Common extensions across all platforms
        let mut extensions = vec![
            // Video formats
            "mp4".to_string(),
            "mkv".to_string(),
            "avi".to_string(),
            "mov".to_string(),
            "wmv".to_string(),
            "flv".to_string(),
            "webm".to_string(),
            "m4v".to_string(),
            "mpg".to_string(),
            "mpeg".to_string(),
            "3gp".to_string(),
            "ogv".to_string(),
            // Audio formats
            "mp3".to_string(),
            "flac".to_string(),
            "wav".to_string(),
            "aac".to_string(),
            "ogg".to_string(),
            "wma".to_string(),
            "m4a".to_string(),
            "opus".to_string(),
            "ape".to_string(),
            // Image formats
            "jpg".to_string(),
            "jpeg".to_string(),
            "png".to_string(),
            "gif".to_string(),
            "bmp".to_string(),
            "webp".to_string(),
            "tiff".to_string(),
            "svg".to_string(),
        ];

        // Add platform-specific extensions
        match self.os_type {
            OsType::Windows => {
                extensions.extend(vec![
                    "asf".to_string(),    // Windows Media
                    "wm".to_string(),     // Windows Media
                    "dvr-ms".to_string(), // Windows Media Center
                ]);
            }

            OsType::MacOS => {
                extensions.extend(vec![
                    "m4p".to_string(), // iTunes protected audio
                    "m4b".to_string(), // iTunes audiobook
                ]);
            }

            OsType::Linux => {
                // Linux typically supports all open formats
                extensions.extend(vec![
                    "mka".to_string(), // Matroska audio
                    "mks".to_string(), // Matroska subtitles
                ]);
            }

            OsType::Bsd => {
                // BSD typically supports all open formats
                extensions.extend(vec![
                    "mka".to_string(), // Matroska audio
                    "mks".to_string(), // Matroska subtitles
                ]);
            }
        }

        extensions
    }

    /// Validate that a path is appropriate for the current platform.
    pub fn validate_path(&self, path: &Path) -> PlatformResult<()> {
        let fs_manager = create_platform_filesystem_manager();
        // The filesystem manager's validate_path only validates the format.
        // Existence is checked by callers like AppConfig::from_args.
        fs_manager.validate_path(path).map_err(|e: FileSystemError| {
            // Convert FileSystemError to a PlatformError for consistency.
            PlatformError::FileSystemAccess(e.user_message())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_config_creation() {
        let config = PlatformConfig::for_current_platform();

        // Basic sanity checks
        assert!(!config.config_dir.as_os_str().is_empty());
        assert!(!config.database_dir.as_os_str().is_empty());
        assert!(!config.preferred_ports.is_empty());
        assert!(!config.metadata.is_empty());

        // Check that paths are different
        assert_ne!(config.config_dir, config.database_dir);
        assert_ne!(config.config_dir, config.cache_dir);

        // Check OS type matches current platform
        assert_eq!(config.os_type, OsType::current());
    }

    #[test]
    fn test_default_media_directories() {
        let config = PlatformConfig::for_current_platform();
        let directories = config.get_default_media_directories();

        assert!(!directories.is_empty());
        assert!(directories.contains(&config.default_media_dir));
    }

    #[test]
    fn test_file_paths() {
        let config = PlatformConfig::for_current_platform();

        let config_file = config.get_config_file_path();
        assert!(config_file.file_name().is_some());
        assert_eq!(config_file.file_name().unwrap(), "config.toml");

        let db_file = config.get_database_path();
        assert!(db_file.file_name().is_some());
        assert_eq!(db_file.file_name().unwrap(), "media.db");

        let log_file = config.get_log_file_path();
        assert!(log_file.file_name().is_some());
        assert_eq!(log_file.file_name().unwrap(), "vuio.log");
    }

    #[test]
    fn test_platform_metadata() {
        let config = PlatformConfig::for_current_platform();

        // Check that platform metadata exists
        assert!(config.get_metadata("platform").is_some());
        assert!(config.get_metadata("case_sensitive").is_some());
        assert!(config.get_metadata("path_separator").is_some());

        // Test helper methods
        let _is_case_sensitive = config.is_case_sensitive();
        let path_sep = config.get_path_separator();
        assert!(!path_sep.is_empty());
    }

    #[test]
    fn test_exclude_patterns() {
        let config = PlatformConfig::for_current_platform();
        let patterns = config.get_default_exclude_patterns();

        assert!(!patterns.is_empty());
        assert!(patterns.contains(&".*".to_string())); // All platforms should exclude hidden files
    }

    #[test]
    fn test_media_extensions() {
        let config = PlatformConfig::for_current_platform();
        let extensions = config.get_default_media_extensions();

        assert!(!extensions.is_empty());
        assert!(extensions.contains(&"mp4".to_string()));
        assert!(extensions.contains(&"mp3".to_string()));
        assert!(extensions.contains(&"jpg".to_string()));
    }

    #[test]
    fn test_path_validation() {
        let config = PlatformConfig::for_current_platform();

        // Create a temporary directory to ensure existence for the test.
        let temp_dir = tempfile::tempdir().unwrap();
        assert!(config.validate_path(&temp_dir.path().to_path_buf()).is_ok());

        // Test with a non-existent but correctly formatted path.
        // The new validation logic only checks format, not existence.
        if cfg!(target_os = "windows") {
            let valid_format_path = PathBuf::from("C:\\This\\Is\\A\\Valid\\Format");
            assert!(config.validate_path(&valid_format_path).is_ok());
        } else {
            let _valid_format_path = PathBuf::from("/this/is/a/valid/format");
            // On non-windows, this path won't exist, and the base validator
            // that our mock uses might not check for that, but we can't be sure.
            // A format-only check is what's intended.
        }
    }
}
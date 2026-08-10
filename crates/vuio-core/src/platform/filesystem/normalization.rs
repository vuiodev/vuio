use super::*;

pub trait PathNormalizer: Send + Sync {
    /// Convert any path to canonical database format (lowercase, absolute paths with forward slashes)
    fn to_canonical(&self, path: &Path) -> Result<String, PathNormalizationError>;

    /// Convert canonical format back to platform path
    fn canonical_to_platform(&self, canonical: &str) -> Result<PathBuf, PathNormalizationError>;

    /// Normalize path for database queries (same as to_canonical but with explicit intent)
    fn normalize_for_query(&self, path: &Path) -> Result<String, PathNormalizationError>;
}

/// Path normalization specific errors
#[derive(Error, Debug, Clone, PartialEq)]
pub enum PathNormalizationError {
    #[error("Invalid path format: {path}")]
    InvalidFormat { path: String },



    #[error("Path contains invalid characters: {path}")]
    InvalidCharacters { path: String },

    #[error("Path is too long: {path}")]
    PathTooLong { path: String },
}

/// Windows-specific path normalizer implementation
pub struct WindowsPathNormalizer;

impl WindowsPathNormalizer {
    pub fn new() -> Self {
        Self
    }

    /// Convert Windows path to canonical format (lowercase, forward slashes)
    fn normalize_to_canonical(&self, path: &Path) -> Result<String, PathNormalizationError> {
        let path_str = path.to_string_lossy();

        // Validate path length
        if path_str.len() > 4096 {
            return Err(PathNormalizationError::PathTooLong {
                path: path_str.to_string(),
            });
        }

        // Check for invalid characters
        let invalid_chars = ['\0', '<', '>', '"', '|', '?', '*'];
        for &invalid_char in &invalid_chars {
            if path_str.contains(invalid_char) {
                return Err(PathNormalizationError::InvalidCharacters {
                    path: path_str.to_string(),
                });
            }
        }

        // Convert to lowercase and use forward slashes
        let mut canonical = path_str.to_lowercase();
        canonical = canonical.replace('\\', "/");

        // Deduplicate slashes
        if canonical.starts_with("//") {
            // UNC path: preserve leading double slash, clean the rest
            let rest = canonical[2..].replace("//", "/");
            // Iterate until stable to handle multiple consecutive slashes
            let mut cleaned = rest;
            while cleaned.contains("//") {
                cleaned = cleaned.replace("//", "/");
            }
            canonical = format!("//{}", cleaned);
        } else {
            // Standard path: clean all double slashes
            while canonical.contains("//") {
                canonical = canonical.replace("//", "/");
            }
        }

        // Handle UNC paths - preserve the leading double slash
        if canonical.starts_with("//") {
            // UNC path: //server/share/path
            Ok(canonical)
        } else if canonical.len() >= 2 && canonical.chars().nth(1) == Some(':') {
            // Drive letter path: c:/path/to/file
            Ok(canonical)
        } else if canonical.starts_with('/') {
            // Already absolute Unix-style path
            Ok(canonical)
        } else {
            // Relative path - this might need to be made absolute
            // For now, return as-is but this could be enhanced
            Ok(canonical)
        }
    }

    /// Convert canonical format back to Windows path format
    fn canonical_to_windows(&self, canonical: &str) -> Result<PathBuf, PathNormalizationError> {
        if canonical.is_empty() {
            return Err(PathNormalizationError::InvalidFormat {
                path: canonical.to_string(),
            });
        }

        // Convert forward slashes back to backslashes
        let windows_path = canonical.replace('/', "\\");

        // Handle UNC paths
        if windows_path.starts_with("\\\\") {
            return Ok(PathBuf::from(windows_path));
        }

        // Handle drive letter paths
        if windows_path.len() >= 2 && windows_path.chars().nth(1) == Some(':') {
            // Ensure drive letter is uppercase for consistency
            let mut chars: Vec<char> = windows_path.chars().collect();
            if let Some(first_char) = chars.get_mut(0) {
                *first_char = first_char.to_ascii_uppercase();
            }
            let normalized_windows: String = chars.into_iter().collect();
            return Ok(PathBuf::from(normalized_windows));
        }

        // For other paths, return as-is
        Ok(PathBuf::from(windows_path))
    }
}

impl PathNormalizer for WindowsPathNormalizer {
    fn to_canonical(&self, path: &Path) -> Result<String, PathNormalizationError> {
        self.normalize_to_canonical(path)
    }

    fn canonical_to_platform(&self, canonical: &str) -> Result<PathBuf, PathNormalizationError> {
        self.canonical_to_windows(canonical)
    }

    fn normalize_for_query(&self, path: &Path) -> Result<String, PathNormalizationError> {
        // Same as to_canonical - explicit method for query context
        self.normalize_to_canonical(path)
    }
}

/// Unix/macOS path normalizer that preserves case sensitivity
#[derive(Debug, Clone)]
pub struct UnixPathNormalizer;

impl UnixPathNormalizer {
    pub fn new() -> Self {
        Self
    }

    /// Convert Unix path to canonical format (preserves case, uses forward slashes, and resolves symlinks/private paths)
    fn normalize_to_canonical(&self, path: &Path) -> Result<String, PathNormalizationError> {
        let path_str = path.to_string_lossy();

        // Validate path length
        if path_str.len() > 4096 {
            return Err(PathNormalizationError::PathTooLong {
                path: path_str.to_string(),
            });
        }

        // Check for null bytes (invalid on Unix)
        if path_str.contains('\0') {
            return Err(PathNormalizationError::InvalidCharacters {
                path: path_str.to_string(),
            });
        }

        // On Unix/macOS, we resolve symlinks (e.g. /var -> /private/var) to ensure consistency
        // between the initial scanner paths and directory watcher events.
        // If the path itself does not exist (e.g., a Deleted event), we resolve the longest
        // existing parent directory prefix and append the remaining components.
        let mut resolved_path = std::path::PathBuf::new();
        let mut components = Vec::new();
        let mut current = path;

        while !current.as_os_str().is_empty() {
            if let Ok(canonical) = std::fs::canonicalize(current) {
                resolved_path = canonical;
                break;
            }
            if let Some(parent) = current.parent() {
                if let Some(file_name) = current.file_name() {
                    components.push(file_name);
                }
                current = parent;
            } else {
                break;
            }
        }

        let final_path = if resolved_path.as_os_str().is_empty() {
            path.to_path_buf()
        } else {
            let mut p = resolved_path;
            for comp in components.into_iter().rev() {
                p.push(comp);
            }
            p
        };

        let canonical = final_path.to_string_lossy().to_string();
        Ok(canonical)
    }

    /// Convert canonical format back to Unix path format
    fn canonical_to_unix(&self, canonical: &str) -> Result<PathBuf, PathNormalizationError> {
        if canonical.is_empty() {
            return Err(PathNormalizationError::InvalidFormat {
                path: canonical.to_string(),
            });
        }

        // Unix canonical format is already the correct format
        Ok(PathBuf::from(canonical))
    }
}

impl Default for UnixPathNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl PathNormalizer for UnixPathNormalizer {
    fn to_canonical(&self, path: &Path) -> Result<String, PathNormalizationError> {
        self.normalize_to_canonical(path)
    }

    fn canonical_to_platform(&self, canonical: &str) -> Result<PathBuf, PathNormalizationError> {
        self.canonical_to_unix(canonical)
    }

    fn normalize_for_query(&self, path: &Path) -> Result<String, PathNormalizationError> {
        // Same as to_canonical - explicit method for query context
        self.normalize_to_canonical(path)
    }
}

impl Default for WindowsPathNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a platform-specific path normalizer
pub fn create_platform_path_normalizer() -> Box<dyn PathNormalizer> {
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsPathNormalizer::new())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Box::new(UnixPathNormalizer::new())
    }
}

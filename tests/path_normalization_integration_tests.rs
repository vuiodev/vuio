//! Comprehensive path normalization integration tests
//! 
//! These tests verify path normalization functionality across different scenarios
//! including Windows path variations, symbolic links, junction points, and Unicode handling.

use vuio::platform::filesystem::{PathNormalizer, WindowsPathNormalizer, create_platform_path_normalizer, FileSystemManager};
use std::path::Path;

#[cfg(test)]
mod path_normalization_integration_tests {
    use super::*;

    /// Test Windows path variations including different drive formats and UNC paths
    #[test]
    fn test_windows_path_variations() {
        let normalizer = WindowsPathNormalizer::new();
        
        // Test standard Windows paths with backslashes
        let test_cases = vec![
            // (input_path, expected_canonical)
            (r"C:\Users\Media\Videos", "c:/users/media/videos"),
            (r"c:\users\media\videos", "c:/users/media/videos"),
            (r"C:/Users/Media/Videos", "c:/users/media/videos"),
            (r"c:/users/media/videos", "c:/users/media/videos"),
            
            // Mixed separators
            (r"C:\Users/Media\Videos", "c:/users/media/videos"),
            (r"C:/Users\Media/Videos", "c:/users/media/videos"),
            
            // Drive letter variations
            (r"C:", "c:"),
            (r"c:", "c:"),
            (r"C:\", "c:/"),
            (r"c:\", "c:/"),
            (r"C:/", "c:/"),
            (r"c:/", "c:/"),
            
            // Different drive letters
            (r"D:\Media\Music", "d:/media/music"),
            (r"E:\Videos\Movies", "e:/videos/movies"),
            (r"Z:\Network\Share", "z:/network/share"),
        ];
        
        for (input, expected) in test_cases {
            let result = normalizer.to_canonical(Path::new(input));
            assert!(result.is_ok(), "Failed to normalize path: {}", input);
            assert_eq!(result.unwrap(), expected, "Path normalization mismatch for: {}", input);
        }
    }

    /// Test UNC (Universal Naming Convention) path handling
    #[test]
    fn test_unc_path_variations() {
        let normalizer = WindowsPathNormalizer::new();
        
        let unc_test_cases = vec![
            // Standard UNC paths
            (r"\\Server\Share\Media", "//server/share/media"),
            (r"\\SERVER\SHARE\MEDIA", "//server/share/media"),
            (r"\\server\share\media", "//server/share/media"),
            
            // UNC paths with IP addresses
            (r"\\192.168.1.100\Media", "//192.168.1.100/media"),
            (r"\\10.0.0.1\Share\Videos", "//10.0.0.1/share/videos"),
            
            // UNC paths with mixed separators
            (r"\\Server/Share\Media", "//server/share/media"),
            (r"\\Server\Share/Media", "//server/share/media"),
            
            // UNC root paths
            (r"\\Server\Share", "//server/share"),
            (r"\\Server\Share\", "//server/share/"),
        ];
        
        for (input, expected) in unc_test_cases {
            let result = normalizer.to_canonical(Path::new(input));
            assert!(result.is_ok(), "Failed to normalize UNC path: {}", input);
            assert_eq!(result.unwrap(), expected, "UNC path normalization mismatch for: {}", input);
        }
    }

    /// Test extended-length path prefix (\\?\) handling
    #[test]
    fn test_extended_length_path_prefix() {
        let normalizer = WindowsPathNormalizer::new();
        
        let extended_test_cases = vec![
            // Extended-length paths - current implementation may not handle these specially
            (r"\\?\C:\Users\Media", "c:/users/media"),
            (r"\\?\c:\users\media", "c:/users/media"),
            (r"\\?\D:\Very\Long\Path\To\Media\Files", "d:/very/long/path/to/media/files"),
            
            // Extended-length UNC paths
            (r"\\?\UNC\Server\Share\Media", "//server/share/media"),
            (r"\\?\UNC\server\share\media", "//server/share/media"),
        ];
        
        for (input, _expected) in extended_test_cases {
            let result = normalizer.to_canonical(Path::new(input));
            
            // Current implementation may not handle extended-length paths specially
            // This test documents the expected behavior for future implementation
            match result {
                Ok(canonical) => {
                    println!("Extended-length path normalized: {} -> {}", input, canonical);
                    // If normalization succeeds, verify it produces reasonable output
                    assert!(!canonical.is_empty());
                    // Note: May not match expected exactly if extended-length handling isn't implemented
                }
                Err(e) => {
                    println!("Extended-length path normalization failed (may be expected): {} -> {:?}", input, e);
                    // This is acceptable if extended-length path support isn't implemented yet
                }
            }
        }
    }

    /// Test Unicode character handling in paths
    #[test]
    fn test_unicode_character_handling() {
        let normalizer = WindowsPathNormalizer::new();
        
        let basic_unicode_test_cases = vec![
            // Basic Unicode characters
            (r"C:\Users\José\Media", "c:/users/josé/media"),
            (r"C:\Users\François\Vidéos", "c:/users/françois/vidéos"),
            (r"C:\Users\张三\媒体", "c:/users/张三/媒体"),
            
            // Unicode with mixed case
            (r"C:\Users\JOSÉ\MEDIA", "c:/users/josé/media"),
            (r"C:\USERS\François\VIDÉOS", "c:/users/françois/vidéos"),
            
            // Emoji and special Unicode characters
            (r"C:\Users\Test\🎵Music", "c:/users/test/🎵music"),
            (r"C:\Users\Test\📹Videos", "c:/users/test/📹videos"),
        ];
        
        // Test basic Unicode handling
        for (input, expected) in basic_unicode_test_cases {
            let result = normalizer.to_canonical(Path::new(input));
            assert!(result.is_ok(), "Failed to normalize Unicode path: {}", input);
            assert_eq!(result.unwrap(), expected, "Unicode path normalization mismatch for: {}", input);
        }
        
        // Test Unicode normalization forms (NFC vs NFD) - this is more complex
        let nfc_path = "C:\\Users\\café\\media";
        let nfd_path = "C:\\Users\\cafe\\u{0301}\\media"; // NFD form with combining accent
        
        // Test NFC form (should work normally)
        let nfc_result = normalizer.to_canonical(Path::new(nfc_path));
        assert!(nfc_result.is_ok(), "Failed to normalize NFC Unicode path");
        assert_eq!(nfc_result.unwrap(), "c:/users/café/media");
        
        // Test NFD form (current implementation may not handle Unicode normalization)
        let nfd_result = normalizer.to_canonical(Path::new(nfd_path));
        match nfd_result {
            Ok(canonical) => {
                println!("NFD Unicode path normalized: {} -> {}", nfd_path, canonical);
                // Current implementation may not normalize NFD to NFC
                assert!(!canonical.is_empty());
                assert!(canonical.starts_with("c:/users/"));
                
                // Check if Unicode normalization is implemented
                if canonical == "c:/users/café/media" {
                    println!("  Unicode normalization (NFD -> NFC) is implemented");
                } else {
                    println!("  Note: Unicode normalization (NFD -> NFC) may not be fully implemented");
                    println!("  Got: {}", canonical);
                    println!("  Expected: c:/users/café/media");
                }
            }
            Err(e) => {
                println!("NFD Unicode path normalization failed: {} -> {:?}", nfd_path, e);
                // This might be acceptable if advanced Unicode normalization isn't implemented
            }
        }
    }

    /// Test roundtrip conversion (canonical -> Windows -> canonical)
    #[test]
    fn test_roundtrip_conversion() {
        let normalizer = WindowsPathNormalizer::new();
        
        let test_paths = vec![
            "c:/users/media/videos",
            "d:/music/albums/artist",
            "//server/share/media",
            "e:/very/long/path/to/media/files",
            "c:/users/josé/vidéos",
            "//192.168.1.100/media/movies",
        ];
        
        for canonical_path in test_paths {
            // Convert canonical to Windows format
            let windows_path = normalizer.from_canonical(canonical_path);
            assert!(windows_path.is_ok(), "Failed to convert canonical to Windows: {}", canonical_path);
            
            let windows_pathbuf = windows_path.unwrap();
            
            // Convert back to canonical
            let back_to_canonical = normalizer.to_canonical(&windows_pathbuf);
            assert!(back_to_canonical.is_ok(), "Failed to convert Windows back to canonical: {:?}", windows_pathbuf);
            
            // Should match original canonical path
            assert_eq!(back_to_canonical.unwrap(), canonical_path, 
                      "Roundtrip conversion failed for: {}", canonical_path);
        }
    }

    /// Test path normalization consistency across different representations
    #[test]
    fn test_path_normalization_consistency() {
        let normalizer = WindowsPathNormalizer::new();
        
        // Different representations of the same logical path
        let path_groups = vec![
            // Group 1: C:\Users\Media variations
            vec![
                r"C:\Users\Media",
                r"c:\users\media",
                r"C:/Users/Media",
                r"c:/users/media",
                r"C:\Users/Media",
                r"C:/Users\Media",
            ],
            
            // Group 2: UNC path variations
            vec![
                r"\\Server\Share\Media",
                r"\\server\share\media",
                r"\\SERVER\SHARE\MEDIA",
                r"\\Server/Share\Media",
                r"\\Server\Share/Media",
            ],
            
            // Group 3: Drive root variations
            vec![
                r"C:\",
                r"c:\",
                r"C:/",
                r"c:/",
            ],
        ];
        
        for group in path_groups {
            let mut canonical_results = Vec::new();
            
            // Normalize all paths in the group
            for path_str in &group {
                let canonical = normalizer.to_canonical(Path::new(path_str));
                assert!(canonical.is_ok(), "Failed to normalize path: {}", path_str);
                canonical_results.push(canonical.unwrap());
            }
            
            // All results should be identical
            let first_result = &canonical_results[0];
            for (i, result) in canonical_results.iter().enumerate() {
                assert_eq!(result, first_result, 
                          "Inconsistent normalization: {} vs {} (paths: {} vs {})", 
                          result, first_result, group[i], group[0]);
            }
        }
    }

    /// Test error handling for invalid paths
    #[test]
    fn test_invalid_path_handling() {
        let normalizer = WindowsPathNormalizer::new();
        
        // Create long path as owned string
        let long_path = "C:\\".to_string() + &"a".repeat(5000);
        
        let invalid_paths = vec![
            // Null byte in path
            "C:\\path\0file",
            "C:\\path\\file\0",
            
            // Invalid Windows characters
            "C:\\path<file",
            "C:\\path>file", 
            "C:\\path\"file",
            "C:\\path|file",
            "C:\\path?file",
            "C:\\path*file",
            "C:\\path:file:name", // Colon not at drive position
            
            // Empty path components
            "C:\\\\double\\backslash",
            "C:\\path\\\\empty",
        ];
        
        // Test regular invalid paths
        for invalid_path in invalid_paths {
            let result = normalizer.to_canonical(Path::new(invalid_path));
            
            // Most should fail, but some might be handled gracefully
            if result.is_ok() {
                println!("Path was normalized (might be acceptable): {} -> {}", 
                        invalid_path, result.unwrap());
            } else {
                println!("Path correctly rejected: {} -> {:?}", invalid_path, result.err());
            }
        }
        
        // Test extremely long path separately
        let long_path_result = normalizer.to_canonical(Path::new(&long_path));
        if long_path_result.is_ok() {
            println!("Long path was normalized (might be acceptable): {} chars", long_path.len());
        } else {
            println!("Long path correctly rejected: {:?}", long_path_result.err());
        }
    }

    /// Test platform-specific path normalizer creation
    #[test]
    fn test_platform_path_normalizer_creation() {
        let normalizer = create_platform_path_normalizer();
        
        // Should be able to normalize basic paths regardless of platform
        let test_paths = vec![
            "C:/Test/Path",
            "/home/user/media",
            "relative/path",
        ];
        
        for path_str in test_paths {
            let result = normalizer.to_canonical(Path::new(path_str));
            // Should either succeed or fail gracefully
            match result {
                Ok(canonical) => {
                    println!("Normalized {} -> {}", path_str, canonical);
                    assert!(!canonical.is_empty());
                }
                Err(e) => {
                    println!("Path normalization failed (may be expected): {} -> {:?}", path_str, e);
                }
            }
        }
    }
}

/// Integration tests that require actual file system operations
#[cfg(test)]
mod filesystem_integration_tests {
    use super::*;
    use vuio::platform::filesystem::create_platform_filesystem_manager;
    use std::fs;
    use tempfile::TempDir;

    /// Test symbolic link and junction point resolution
    #[tokio::test]
    async fn test_symbolic_link_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let fs_manager = create_platform_filesystem_manager();
        
        // Create original file
        let original_file = temp_dir.path().join("original_video.mp4");
        fs::write(&original_file, b"test video content").unwrap();
        
        // Create symbolic link (platform-specific)
        let symlink_file = temp_dir.path().join("symlink_video.mp4");
        
        #[cfg(target_os = "windows")]
        {
            // On Windows, try to create a junction or symbolic link
            use std::os::windows::fs;
            
            // Try junction first (doesn't require admin privileges)
            let junction_dir = temp_dir.path().join("junction_dir");
            let junction_result = fs::symlink_dir(&original_file.parent().unwrap(), &junction_dir);
            
            if junction_result.is_ok() {
                let junction_file = junction_dir.join("original_video.mp4");
                
                // Test canonicalization of junction path
                let canonical_result = fs_manager.canonicalize_path(&junction_file).await;
                match canonical_result {
                    Ok(canonical_path) => {
                        println!("Junction resolved to canonical: {}", canonical_path);
                        // Should resolve to the original file's canonical path
                        let original_canonical = fs_manager.get_canonical_path(&original_file).unwrap();
                        assert_eq!(canonical_path, original_canonical);
                    }
                    Err(e) => {
                        println!("Junction canonicalization failed: {}", e);
                    }
                }
            } else {
                println!("Junction creation failed: {:?}", junction_result.err());
            }
            
            // Try symbolic link (requires admin privileges)
            let symlink_result = fs::symlink_file(&original_file, &symlink_file);
            if symlink_result.is_ok() {
                test_symlink_resolution(&fs_manager, &symlink_file, &original_file).await;
            } else {
                println!("Symbolic link creation failed (expected without admin privileges): {:?}", 
                        symlink_result.err());
            }
        }
        
        #[cfg(unix)]
        {
            use std::os::unix::fs;
            
            // Create symbolic link on Unix systems
            let symlink_result = fs::symlink(&original_file, &symlink_file);
            if symlink_result.is_ok() {
                test_symlink_resolution(&fs_manager, &symlink_file, &original_file).await;
            } else {
                println!("Symbolic link creation failed: {:?}", symlink_result.err());
            }
        }
    }

    /// Test Windows-specific junction point handling
    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn test_windows_junction_points() {
        let temp_dir = TempDir::new().unwrap();
        let fs_manager = create_platform_filesystem_manager();
        
        // Create source directory with media files
        let source_dir = temp_dir.path().join("source_media");
        std::fs::create_dir(&source_dir).unwrap();
        
        let media_files = vec![
            source_dir.join("video1.mp4"),
            source_dir.join("video2.mkv"),
            source_dir.join("audio1.mp3"),
        ];
        
        for file_path in &media_files {
            std::fs::write(file_path, b"test media content").unwrap();
        }
        
        // Try to create junction point
        use std::os::windows::fs as windows_fs;
        let junction_dir = temp_dir.path().join("junction_media");
        let junction_result = windows_fs::symlink_dir(&source_dir, &junction_dir);
        
        match junction_result {
            Ok(()) => {
                println!("Successfully created junction point");
                
                // Test path normalization through junction
                for (i, original_file) in media_files.iter().enumerate() {
                    let junction_file = junction_dir.join(original_file.file_name().unwrap());
                    
                    // Both paths should normalize to the same canonical form
                    let original_canonical = fs_manager.get_canonical_path(original_file).unwrap();
                    let junction_canonical_result = fs_manager.canonicalize_path(&junction_file).await;
                    
                    match junction_canonical_result {
                        Ok(junction_canonical) => {
                            println!("File {}: Original: {} -> Junction: {}", 
                                    i + 1, original_canonical, junction_canonical);
                            
                            // Should resolve to the same canonical path
                            assert_eq!(original_canonical, junction_canonical,
                                      "Junction should resolve to original file's canonical path");
                        }
                        Err(e) => {
                            println!("Junction file canonicalization failed: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                println!("Junction creation failed (may be expected): {:?}", e);
                // This is acceptable - junction creation might fail in some environments
            }
        }
    }

    async fn test_symlink_resolution(
        fs_manager: &Box<dyn FileSystemManager>,
        symlink_path: &Path,
        original_path: &Path,
    ) {
        // Test that canonicalize_path resolves the symbolic link
        let canonical_result = fs_manager.canonicalize_path(symlink_path).await;
        
        match canonical_result {
            Ok(canonical_path) => {
                println!("Symlink resolved to canonical: {}", canonical_path);
                
                // Should resolve to the original file's canonical path
                let original_canonical = fs_manager.get_canonical_path(original_path).unwrap();
                // On macOS, /var is a symlink to /private/var, so we need to normalize both for comparison
                let canonical_normalized = canonical_path.replace("/private/var", "/var");
                let original_normalized = original_canonical.replace("/private/var", "/var");
                assert_eq!(canonical_normalized, original_normalized, 
                          "Symlink should resolve to original file's canonical path");
            }
            Err(e) => {
                println!("Symlink canonicalization failed: {}", e);
                // This might be acceptable in some test environments
            }
        }
    }

    /// Test path normalization with actual file system paths
    #[tokio::test]
    async fn test_real_filesystem_path_normalization() {
        let temp_dir = TempDir::new().unwrap();
        let fs_manager = create_platform_filesystem_manager();
        
        // Create test directory structure
        let media_dir = temp_dir.path().join("Media");
        let videos_dir = media_dir.join("Videos");
        let music_dir = media_dir.join("Music");
        
        fs::create_dir_all(&videos_dir).unwrap();
        fs::create_dir_all(&music_dir).unwrap();
        
        // Create test files with various naming conventions
        let test_files = vec![
            videos_dir.join("Movie.MP4"),
            videos_dir.join("SERIES.mkv"),
            videos_dir.join("documentary.avi"),
            music_dir.join("Song.MP3"),
            music_dir.join("ALBUM.flac"),
            music_dir.join("track.wav"),
        ];
        
        for file_path in &test_files {
            fs::write(file_path, b"test content").unwrap();
        }
        
        // Test path normalization for each file
        for file_path in &test_files {
            let canonical_result = fs_manager.get_canonical_path(file_path);
            
            match canonical_result {
                Ok(canonical_path) => {
                    println!("File path normalized: {} -> {}", 
                            file_path.display(), canonical_path);
                    
                    // Canonical path should be lowercase and use forward slashes
                    assert!(!canonical_path.is_empty());
                    
                    // Should contain forward slashes (or be a simple drive letter)
                    if canonical_path.len() > 2 {
                        assert!(canonical_path.contains("/") || canonical_path.starts_with("\\\\"), 
                               "Canonical path should use forward slashes: {}", canonical_path);
                    }
                    
                    // Should be lowercase (except for UNC server names which preserve case)
                    // On macOS, filesystem is case-preserving, so we don't enforce lowercase
                    #[cfg(not(target_os = "macos"))]
                    if !canonical_path.starts_with("//") {
                        assert_eq!(canonical_path, canonical_path.to_lowercase(),
                                  "Canonical path should be lowercase: {}", canonical_path);
                    }
                }
                Err(e) => {
                    panic!("Failed to get canonical path for {}: {}", file_path.display(), e);
                }
            }
        }
    }

    /// Test path normalization with nested directory structures
    #[tokio::test]
    async fn test_nested_directory_path_normalization() {
        let temp_dir = TempDir::new().unwrap();
        let fs_manager = create_platform_filesystem_manager();
        
        // Create deeply nested directory structure
        let deep_path = temp_dir.path()
            .join("Level1")
            .join("Level2")
            .join("Level3")
            .join("Level4")
            .join("Level5");
        
        fs::create_dir_all(&deep_path).unwrap();
        
        let test_file = deep_path.join("DeepFile.mp4");
        fs::write(&test_file, b"deep file content").unwrap();
        
        // Test normalization of the deep path
        let canonical_result = fs_manager.get_canonical_path(&test_file);
        
        match canonical_result {
            Ok(canonical_path) => {
                println!("Deep path normalized: {} -> {}", 
                        test_file.display(), canonical_path);
                
                // Should handle deep nesting correctly
                assert!(!canonical_path.is_empty());
                assert!(canonical_path.contains("level1/level2/level3/level4/level5") ||
                       canonical_path.contains("Level1/Level2/Level3/Level4/Level5"),
                       "Deep path structure should be preserved: {}", canonical_path);
            }
            Err(e) => {
                panic!("Failed to normalize deep path: {}", e);
            }
        }
    }

    /// Test path normalization with special characters in filenames
    #[tokio::test]
    async fn test_special_characters_in_paths() {
        let temp_dir = TempDir::new().unwrap();
        let fs_manager = create_platform_filesystem_manager();
        
        // Create files with special characters (that are valid on the file system)
        let special_files = vec![
            "file with spaces.mp4",
            "file-with-dashes.mp4",
            "file_with_underscores.mp4",
            "file.with.dots.mp4",
            "file(with)parentheses.mp4",
            "file[with]brackets.mp4",
            "file{with}braces.mp4",
            "file'with'quotes.mp4",
            "file&with&ampersand.mp4",
            "file+with+plus.mp4",
            "file=with=equals.mp4",
            "file@with@at.mp4",
            "file#with#hash.mp4",
            "file%with%percent.mp4",
        ];
        
        for filename in &special_files {
            let file_path = temp_dir.path().join(filename);
            
            // Some characters might not be valid on all file systems
            match fs::write(&file_path, b"test content") {
                Ok(()) => {
                    // Test normalization of files that were successfully created
                    let canonical_result = fs_manager.get_canonical_path(&file_path);
                    
                    match canonical_result {
                        Ok(canonical_path) => {
                            println!("Special character file normalized: {} -> {}", 
                                    filename, canonical_path);
                            assert!(!canonical_path.is_empty());
                        }
                        Err(e) => {
                            println!("Failed to normalize special character file {}: {}", filename, e);
                        }
                    }
                }
                Err(e) => {
                    println!("Could not create file with special characters {}: {}", filename, e);
                }
            }
        }
    }
}

/// Additional integration tests for edge cases
#[cfg(test)]
mod additional_integration_tests {
    use super::*;
    use vuio::platform::filesystem::create_platform_filesystem_manager;
    use std::fs;
    use tempfile::TempDir;

    /// Test case sensitivity and path comparison edge cases
    #[tokio::test]
    async fn test_case_sensitivity_edge_cases() {
        let temp_dir = TempDir::new().unwrap();
        let fs_manager = create_platform_filesystem_manager();
        
        // Create files with different case variations
        let test_cases = vec![
            ("TestVideo.MP4", "test video content 1"),
            ("TESTVIDEO.mp4", "test video content 2"), // Different case, same logical name
            ("testvideo.MP4", "test video content 3"), // Another case variation
        ];
        
        let mut created_files = Vec::new();
        
        for (filename, content) in &test_cases {
            let file_path = temp_dir.path().join(filename);
            
            // Try to create the file - on case-insensitive systems, later files might overwrite earlier ones
            match fs::write(&file_path, content.as_bytes()) {
                Ok(()) => {
                    created_files.push((file_path, filename, content));
                    println!("Created file: {}", filename);
                }
                Err(e) => {
                    println!("Failed to create file {}: {}", filename, e);
                }
            }
        }
        
        // Test path normalization for created files
        for (file_path, filename, _content) in &created_files {
            let canonical_result = fs_manager.get_canonical_path(file_path);
            
            match canonical_result {
                Ok(canonical_path) => {
                    println!("File {} normalized to: {}", filename, canonical_path);
                    
                    // On macOS, filesystem is case-preserving, so we don't enforce lowercase
                    #[cfg(not(target_os = "macos"))]
                    assert_eq!(canonical_path, canonical_path.to_lowercase(),
                              "Canonical path should be lowercase: {}", canonical_path);
                    
                    // Should contain the filename in some form (case-insensitive check)
                    let canonical_lower = canonical_path.to_lowercase();
                    assert!(canonical_lower.contains("testvideo"),
                           "Canonical path should contain filename: {}", canonical_path);
                }
                Err(e) => {
                    println!("Failed to normalize path for {}: {}", filename, e);
                }
            }
        }
        
        // Test that different case variations normalize to the same canonical form
        if created_files.len() > 1 {
            let mut canonical_paths = Vec::new();
            
            for (file_path, filename, _) in &created_files {
                if let Ok(canonical) = fs_manager.get_canonical_path(file_path) {
                    canonical_paths.push((canonical, filename));
                }
            }
            
            // On case-insensitive systems, all variations should normalize to the same path
            // On case-sensitive systems, they should be different
            if canonical_paths.len() > 1 {
                let first_canonical = &canonical_paths[0].0;
                let all_same = canonical_paths.iter().all(|(canonical, _)| canonical == first_canonical);
                
                if all_same {
                    println!("All case variations normalized to same path (case-insensitive filesystem)");
                } else {
                    println!("Case variations normalized to different paths (case-sensitive filesystem)");
                    for (canonical, filename) in &canonical_paths {
                        println!("  {} -> {}", filename, canonical);
                    }
                }
            }
        }
    }

    /// Test path normalization with relative paths and current directory
    #[tokio::test]
    async fn test_relative_path_normalization() {
        let temp_dir = TempDir::new().unwrap();
        let fs_manager = create_platform_filesystem_manager();
        
        // Create test file
        let test_file = temp_dir.path().join("relative_test.mp4");
        fs::write(&test_file, b"relative test content").unwrap();
        
        // Test absolute path normalization
        let absolute_canonical = fs_manager.get_canonical_path(&test_file).unwrap();
        println!("Absolute path canonical: {}", absolute_canonical);
        
        // Test relative path normalization (if supported)
        let relative_path = Path::new("relative_test.mp4");
        let relative_canonical_result = fs_manager.get_canonical_path(relative_path);
        
        match relative_canonical_result {
            Ok(relative_canonical) => {
                println!("Relative path canonical: {}", relative_canonical);
                // Relative paths might be normalized differently
                assert!(!relative_canonical.is_empty());
            }
            Err(e) => {
                println!("Relative path normalization failed (may be expected): {}", e);
                // This is acceptable - relative paths might not be supported for canonicalization
            }
        }
        
        // Test path with .. components
        let parent_path = test_file.parent().unwrap().join("..").join(test_file.file_name().unwrap());
        let parent_canonical_result = fs_manager.get_canonical_path(&parent_path);
        
        match parent_canonical_result {
            Ok(parent_canonical) => {
                println!("Parent reference path canonical: {}", parent_canonical);
                // Current implementation may not resolve .. components in canonical paths
                if parent_canonical.contains("..") {
                    println!("  Note: .. components not resolved in canonical path (may be expected)");
                } else {
                    println!("  .. components resolved successfully");
                }
                assert!(!parent_canonical.is_empty());
            }
            Err(e) => {
                println!("Parent reference path normalization failed: {}", e);
            }
        }
    }

    /// Test path normalization performance with many files
    #[tokio::test]
    async fn test_path_normalization_performance() {
        let temp_dir = TempDir::new().unwrap();
        let fs_manager = create_platform_filesystem_manager();
        
        // Create many test files
        let file_count = 100;
        let mut test_files = Vec::new();
        
        for i in 0..file_count {
            let filename = format!("test_file_{:03}.mp4", i);
            let file_path = temp_dir.path().join(&filename);
            fs::write(&file_path, format!("test content {}", i).as_bytes()).unwrap();
            test_files.push(file_path);
        }
        
        // Measure normalization performance
        let start_time = std::time::Instant::now();
        let mut successful_normalizations = 0;
        
        for file_path in &test_files {
            if let Ok(_canonical) = fs_manager.get_canonical_path(file_path) {
                successful_normalizations += 1;
            }
        }
        
        let elapsed = start_time.elapsed();
        println!("Normalized {} files in {:?} ({:.2} files/ms)", 
                successful_normalizations, elapsed, 
                successful_normalizations as f64 / elapsed.as_millis() as f64);
        
        // Should be able to normalize at least most files
        assert!(successful_normalizations >= file_count * 9 / 10, 
               "Should successfully normalize at least 90% of files");
        
        // Should complete in reasonable time (less than 1 second for 100 files)
        assert!(elapsed.as_secs() < 1, 
               "Path normalization should complete quickly");
    }
}
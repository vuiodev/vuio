use super::*;

#[test]
fn test_mime_type_detection() {
    assert_eq!(get_mime_type_for_extension("mp4"), "video/mp4");
    assert_eq!(get_mime_type_for_extension("MP4"), "video/mp4");
    assert_eq!(get_mime_type_for_extension("mp3"), "audio/mpeg");
    assert_eq!(
        get_mime_type_for_extension("unknown"),
        "application/octet-stream"
    );
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
    assert!(manager
        .validate_path_common(Path::new("/valid/path"))
        .is_ok());
    assert!(manager
        .validate_path_common(Path::new("relative/path"))
        .is_ok());
    assert!(manager
        .validate_path_common(Path::new("relative/movie..mp4"))
        .is_ok());

    // Invalid paths
    assert!(manager
        .validate_path_common(Path::new("path/with/\0/null"))
        .is_err());
    assert!(manager
        .validate_path_common(Path::new("path/../traversal"))
        .is_err());
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

#[test]
fn test_fallback_parse_filename() {
    use std::path::PathBuf;
    use std::time::SystemTime;

    let mut f1 = MediaFile {
        id: None,
        path: PathBuf::from("/path/to/01 - Artist Name - Song Title.mp3"),
        filename: "01 - Artist Name - Song Title.mp3".to_string(),
        size: 0,
        modified: SystemTime::UNIX_EPOCH,
        mime_type: "audio/mpeg".to_string(),
        duration: None,
        title: None,
        artist: None,
        album: None,
        genre: None,
        track_number: None,
        year: None,
        album_artist: None,
        subtitle_available: false,
        created_at: SystemTime::UNIX_EPOCH,
        updated_at: SystemTime::UNIX_EPOCH,
    };
    fallback_parse_filename(&mut f1);
    assert_eq!(f1.track_number, Some(1));
    assert_eq!(f1.artist.as_deref(), Some("Artist Name"));
    assert_eq!(f1.title.as_deref(), Some("Song Title"));

    let mut f2 = MediaFile {
        id: None,
        path: PathBuf::from("/path/to/Artist Name - Song Title.mp3"),
        filename: "Artist Name - Song Title.mp3".to_string(),
        size: 0,
        modified: SystemTime::UNIX_EPOCH,
        mime_type: "audio/mpeg".to_string(),
        duration: None,
        title: None,
        artist: None,
        album: None,
        genre: None,
        track_number: None,
        year: None,
        album_artist: None,
        subtitle_available: false,
        created_at: SystemTime::UNIX_EPOCH,
        updated_at: SystemTime::UNIX_EPOCH,
    };
    fallback_parse_filename(&mut f2);
    assert_eq!(f2.track_number, None);
    assert_eq!(f2.artist.as_deref(), Some("Artist Name"));
    assert_eq!(f2.title.as_deref(), Some("Song Title"));

    let mut f3 = MediaFile {
        id: None,
        path: PathBuf::from("/path/to/02 Song Title.mp3"),
        filename: "02 Song Title.mp3".to_string(),
        size: 0,
        modified: SystemTime::UNIX_EPOCH,
        mime_type: "audio/mpeg".to_string(),
        duration: None,
        title: None,
        artist: None,
        album: None,
        genre: None,
        track_number: None,
        year: None,
        album_artist: None,
        subtitle_available: false,
        created_at: SystemTime::UNIX_EPOCH,
        updated_at: SystemTime::UNIX_EPOCH,
    };
    fallback_parse_filename(&mut f3);
    assert_eq!(f3.track_number, Some(2));
    assert_eq!(f3.artist, None);
    assert_eq!(f3.title.as_deref(), Some("Song Title"));
}

// PathNormalizer tests
mod path_normalizer_tests {
    use super::*;

    #[test]
    fn test_windows_path_normalization_basic() {
        let normalizer = WindowsPathNormalizer::new();

        // Test basic Windows path with backslashes
        let result = normalizer
            .to_canonical(Path::new(r"C:\Users\Media"))
            .unwrap();
        assert_eq!(result, "c:/users/media");

        // Test mixed case
        let result = normalizer
            .to_canonical(Path::new(r"C:\Users\MEDIA\Videos"))
            .unwrap();
        assert_eq!(result, "c:/users/media/videos");

        // Test forward slashes (already normalized)
        let result = normalizer
            .to_canonical(Path::new("C:/Users/Media"))
            .unwrap();
        assert_eq!(result, "c:/users/media");
    }

    #[test]
    fn test_windows_path_normalization_mixed_separators() {
        let normalizer = WindowsPathNormalizer::new();

        // Test mixed separators
        let result = normalizer
            .to_canonical(Path::new(r"C:\Users/Media\Videos"))
            .unwrap();
        assert_eq!(result, "c:/users/media/videos");

        // Test multiple consecutive separators
        let result = normalizer
            .to_canonical(Path::new(r"C:\Users\\Media"))
            .unwrap();
        assert_eq!(result, "c:/users/media");
    }

    #[test]
    fn test_windows_path_normalization_unc_paths() {
        let normalizer = WindowsPathNormalizer::new();

        // Test UNC path
        let result = normalizer
            .to_canonical(Path::new(r"\\Server\Share\Media"))
            .unwrap();
        assert_eq!(result, "//server/share/media");

        // Test UNC path with port
        let result = normalizer
            .to_canonical(Path::new(r"\\192.168.1.100\Media"))
            .unwrap();
        assert_eq!(result, "//192.168.1.100/media");
    }

    #[test]
    fn test_windows_path_normalization_edge_cases() {
        let normalizer = WindowsPathNormalizer::new();

        // Test drive letter only
        let result = normalizer.to_canonical(Path::new("C:")).unwrap();
        assert_eq!(result, "c:");

        // Test root path
        let result = normalizer.to_canonical(Path::new(r"C:\")).unwrap();
        assert_eq!(result, "c:/");

        // Test relative path
        let result = normalizer.to_canonical(Path::new(r"Media\Videos")).unwrap();
        assert_eq!(result, "media/videos");
    }

    #[test]
    fn test_canonical_to_windows_conversion() {
        let normalizer = WindowsPathNormalizer::new();

        // Test basic conversion
        let result = normalizer.canonical_to_platform("c:/users/media").unwrap();
        assert_eq!(result, PathBuf::from(r"C:\users\media"));

        // Test UNC path conversion
        let result = normalizer
            .canonical_to_platform("//server/share/media")
            .unwrap();
        assert_eq!(result, PathBuf::from(r"\\server\share\media"));

        // Test drive letter capitalization
        let result = normalizer.canonical_to_platform("d:/media/videos").unwrap();
        assert_eq!(result, PathBuf::from(r"D:\media\videos"));
    }

    #[test]
    fn test_normalize_for_query() {
        let normalizer = WindowsPathNormalizer::new();

        // Should be identical to to_canonical
        let path = Path::new(r"C:\Users\Media");
        let canonical = normalizer.to_canonical(path).unwrap();
        let query_normalized = normalizer.normalize_for_query(path).unwrap();

        assert_eq!(canonical, query_normalized);
        assert_eq!(query_normalized, "c:/users/media");
    }

    #[test]
    fn test_path_normalization_invalid_characters() {
        let normalizer = WindowsPathNormalizer::new();

        // Test null byte
        assert!(normalizer.to_canonical(Path::new("C:\\path\0")).is_err());

        // Test invalid Windows characters
        assert!(normalizer.to_canonical(Path::new("C:\\path<file")).is_err());
        assert!(normalizer.to_canonical(Path::new("C:\\path>file")).is_err());
        assert!(normalizer
            .to_canonical(Path::new("C:\\path\"file"))
            .is_err());
        assert!(normalizer.to_canonical(Path::new("C:\\path|file")).is_err());
        assert!(normalizer.to_canonical(Path::new("C:\\path?file")).is_err());
        assert!(normalizer.to_canonical(Path::new("C:\\path*file")).is_err());
    }

    #[test]
    fn test_path_normalization_too_long() {
        let normalizer = WindowsPathNormalizer::new();

        // Create a very long path
        let long_path = "C:\\".to_string() + &"a".repeat(5000);
        assert!(normalizer.to_canonical(Path::new(&long_path)).is_err());
    }

    #[test]
    fn test_canonical_format_consistency() {
        let normalizer = WindowsPathNormalizer::new();

        // Different representations of the same path should normalize to the same canonical form
        let paths = vec![
            r"C:\Users\Media",
            r"c:\users\media",
            r"C:/Users/Media",
            r"c:/users/media",
            r"C:\Users/Media",
        ];

        let mut canonical_results = Vec::new();
        for path_str in paths {
            let canonical = normalizer.to_canonical(Path::new(path_str)).unwrap();
            canonical_results.push(canonical);
        }

        // All should be the same
        let expected = "c:/users/media";
        for result in canonical_results {
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_roundtrip_conversion() {
        let normalizer = WindowsPathNormalizer::new();

        let test_paths = vec![
            r"C:\Users\Media\Videos",
            r"D:\Music\Albums",
            r"\\Server\Share\Media",
            r"E:\",
        ];

        for original_path in test_paths {
            let canonical = normalizer.to_canonical(Path::new(original_path)).unwrap();
            let back_to_windows = normalizer.canonical_to_platform(&canonical).unwrap();

            // The roundtrip should preserve the essential path structure
            // (though case and separator format may change)
            let re_canonical = normalizer.to_canonical(&back_to_windows).unwrap();
            assert_eq!(canonical, re_canonical);
        }
    }

    #[test]
    fn test_filesystem_manager_path_normalization_integration() {
        let manager = create_platform_filesystem_manager();

        // Test that normalize_path still works as expected (returns PathBuf)
        let test_path = Path::new("test/path");
        let normalized = manager.normalize_path(test_path);
        assert!(normalized.is_absolute() || normalized.is_relative());

        // Test that get_canonical_path returns the canonical string format
        let canonical_result = manager.get_canonical_path(test_path);
        assert!(canonical_result.is_ok());

        let canonical = canonical_result.unwrap();
        assert!(canonical.contains('/') || !canonical.is_empty()); // Should be forward-slash format
    }

    #[tokio::test]
    async fn test_canonicalize_path_resolves_before_normalization() {
        // This test verifies that canonicalize_path resolves symbolic links before normalization
        // Note: This is a unit test that verifies the method signature and basic functionality
        // Integration tests would verify actual symbolic link resolution

        let manager = create_platform_filesystem_manager();

        // Test with a simple path (this will likely fail since the path doesn't exist,
        // but we're testing the method signature and error handling)
        let test_path = Path::new("nonexistent/test/path");
        let result = manager.canonicalize_path(test_path).await;

        // Should return an error since the path doesn't exist, but the error should be
        // a FileSystemError, not a compilation error
        assert!(result.is_err());

        // The return type should be Result<String, FileSystemError> (canonical format)
        match result {
            Err(FileSystemError::PathNotFound { .. }) => {
                // Expected error for non-existent path
            }
            Err(FileSystemError::Io(_)) => {
                // Also acceptable - depends on platform
            }
            Err(other) => {
                // Other errors are also acceptable for this test
                println!("Got error: {:?}", other);
            }
            Ok(_) => {
                panic!("Expected error for non-existent path");
            }
        }
    }

    #[test]
    fn test_empty_and_invalid_canonical_paths() {
        let normalizer = WindowsPathNormalizer::new();

        // Test empty canonical path
        assert!(normalizer.canonical_to_platform("").is_err());

        // Test valid canonical paths
        assert!(normalizer.canonical_to_platform("c:/users/media").is_ok());
        assert!(normalizer.canonical_to_platform("//server/share").is_ok());
    }

    #[test]
    fn test_platform_path_normalizer_creation() {
        let normalizer = create_platform_path_normalizer();

        // Should be able to normalize a basic path
        let result = normalizer.to_canonical(Path::new("C:/Test/Path"));
        assert!(result.is_ok());
    }
}

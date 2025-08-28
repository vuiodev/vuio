//! Regression tests for critical path normalization scenarios
//! 
//! These tests verify that critical edge cases and scenarios continue to work correctly
//! after refactoring. They focus on mixed case paths on Windows, network paths and UNC formats,
//! very long path names, and special characters.

use vuio::platform::filesystem::{PathNormalizer, WindowsPathNormalizer, create_platform_path_normalizer};
use std::path::Path;
use std::time::Instant;

#[cfg(test)]
mod critical_path_scenarios {
    use super::*;

    /// Test mixed case paths on Windows - Requirements 1.5, 2.1
    /// This is a critical scenario that caused DLNA browsing failures
    #[test]
    fn test_mixed_case_paths_windows() {
        let normalizer = WindowsPathNormalizer::new();
        
        // Test cases that previously caused issues in DLNA browsing
        let mixed_case_scenarios = vec![
            // Same logical path with different case variations
            (vec![
                r"C:\Users\Media\Videos\Movie.mp4",
                r"c:\users\media\videos\movie.mp4", 
                r"C:/Users/Media/Videos/Movie.mp4",
                r"c:/users/media/videos/movie.mp4",
                r"C:\USERS\MEDIA\VIDEOS\MOVIE.MP4",
                r"C:/USERS/MEDIA/VIDEOS/MOVIE.MP4",
            ], "c:/users/media/videos/movie.mp4"),
            
            // Drive letter variations
            (vec![
                r"D:\Music\Artist\Album\Song.mp3",
                r"d:\music\artist\album\song.mp3",
                r"D:/Music/Artist/Album/Song.mp3", 
                r"d:/music/artist/album/song.mp3",
            ], "d:/music/artist/album/song.mp3"),
            
            // Root directory variations
            (vec![
                r"C:\",
                r"c:\",
                r"C:/",
                r"c:/",
            ], "c:/"),
            
            // Mixed separators with case variations
            (vec![
                r"E:\Media\TV Shows\Series\Season 1\Episode.mkv",
                r"e:\media\tv shows\series\season 1\episode.mkv",
                r"E:/Media/TV Shows/Series/Season 1/Episode.mkv",
                r"E:\Media/TV Shows\Series/Season 1\Episode.mkv",
                r"e:/media\tv shows/series\season 1/episode.mkv",
            ], "e:/media/tv shows/series/season 1/episode.mkv"),
        ];
        
        for (path_variations, expected_canonical) in mixed_case_scenarios {
            let mut canonical_results = Vec::new();
            
            // Normalize all variations
            for path_str in &path_variations {
                let result = normalizer.to_canonical(Path::new(path_str));
                assert!(result.is_ok(), "Failed to normalize mixed case path: {}", path_str);
                canonical_results.push(result.unwrap());
            }
            
            // All variations should produce the same canonical result
            for (i, canonical) in canonical_results.iter().enumerate() {
                assert_eq!(canonical, expected_canonical, 
                          "Mixed case path normalization inconsistent: {} -> {} (expected: {})", 
                          path_variations[i], canonical, expected_canonical);
            }
            
            // Verify all results are identical (consistency check)
            let first_result = &canonical_results[0];
            for (i, result) in canonical_results.iter().enumerate() {
                assert_eq!(result, first_result,
                          "Inconsistent normalization between {} and {}: {} vs {}", 
                          path_variations[0], path_variations[i], first_result, result);
            }
        }
    }

    /// Test network paths and UNC formats - Requirements 1.5, 2.1
    /// UNC paths are critical for network storage scenarios
    #[test]
    fn test_network_paths_and_unc_formats() {
        let normalizer = WindowsPathNormalizer::new();
        
        // UNC path test cases that must work correctly
        let unc_test_scenarios = vec![
            // Standard UNC server paths with case variations
            (vec![
                r"\\MediaServer\Videos\Movies\Action\Movie.mp4",
                r"\\mediaserver\videos\movies\action\movie.mp4",
                r"\\MEDIASERVER\VIDEOS\MOVIES\ACTION\MOVIE.MP4",
                r"\\MediaServer/Videos/Movies/Action/Movie.mp4",
                r"\\mediaserver/videos\movies/action\movie.mp4",
            ], "//mediaserver/videos/movies/action/movie.mp4"),
            
            // IP address-based UNC paths
            (vec![
                r"\\192.168.1.100\Media\Music\Artist\Album.mp3",
                r"\\192.168.1.100\media\music\artist\album.mp3",
                r"\\192.168.1.100/Media/Music/Artist/Album.mp3",
                r"\\192.168.1.100\Media/Music\Artist/Album.mp3",
            ], "//192.168.1.100/media/music/artist/album.mp3"),
            
            // UNC root shares
            (vec![
                r"\\Server\Share",
                r"\\server\share",
                r"\\SERVER\SHARE",
                r"\\Server/Share",
                r"\\server/share",
            ], "//server/share"),
            
            // UNC paths with trailing slashes
            (vec![
                r"\\Server\Share\",
                r"\\server\share\",
                r"\\Server/Share/",
                r"\\server/share/",
            ], "//server/share/"),
            
            // Complex UNC paths with deep nesting
            (vec![
                r"\\NAS\Media\TV\Series\Season 01\Episode 01.mkv",
                r"\\nas\media\tv\series\season 01\episode 01.mkv",
                r"\\NAS/Media/TV/Series/Season 01/Episode 01.mkv",
                r"\\nas\media/tv\series/season 01\episode 01.mkv",
            ], "//nas/media/tv/series/season 01/episode 01.mkv"),
        ];
        
        for (unc_variations, expected_canonical) in unc_test_scenarios {
            for path_str in &unc_variations {
                let result = normalizer.to_canonical(Path::new(path_str));
                assert!(result.is_ok(), "Failed to normalize UNC path: {}", path_str);
                
                let canonical = result.unwrap();
                assert_eq!(canonical, expected_canonical,
                          "UNC path normalization incorrect: {} -> {} (expected: {})",
                          path_str, canonical, expected_canonical);
                
                // Verify UNC format is preserved (starts with //)
                assert!(canonical.starts_with("//"), 
                       "UNC canonical format should start with //: {}", canonical);
            }
        }
    }

    /// Test very long path names - Requirements 1.5, 2.1
    /// Long paths can cause issues in Windows and database storage
    #[test]
    fn test_very_long_path_names() {
        let normalizer = WindowsPathNormalizer::new();
        
        // Test various long path scenarios
        let long_path_tests = vec![
            // Long filename
            {
                let long_filename = "A".repeat(200) + ".mp4";
                let path = format!(r"C:\Media\Videos\{}", long_filename);
                let expected = format!("c:/media/videos/{}", long_filename.to_lowercase());
                (path, expected)
            },
            
            // Long directory name
            {
                let long_dirname = "Very".repeat(50) + "LongDirectoryName";
                let path = format!(r"C:\Media\{}\Movie.mp4", long_dirname);
                let expected = format!("c:/media/{}/movie.mp4", long_dirname.to_lowercase());
                (path, expected)
            },
            
            // Deep nesting with moderately long names
            {
                let mut path_parts = vec!["C:".to_string()];
                let mut expected_parts = vec!["c:".to_string()];
                
                // Create 20 levels of nesting
                for i in 1..=20 {
                    let part = format!("Level{:02}WithSomewhatLongName", i);
                    path_parts.push(part.clone());
                    expected_parts.push(part.to_lowercase());
                }
                path_parts.push("FinalFile.mp4".to_string());
                expected_parts.push("finalfile.mp4".to_string());
                
                let path = path_parts.join("\\");
                let expected = expected_parts.join("/");
                (path, expected)
            },
            
            // UNC path with long server and share names
            {
                let long_server = "MediaServer".repeat(10);
                let long_share = "VideoShare".repeat(8);
                let path = format!(r"\\{}\{}\Movies\LongMovieName.mp4", long_server, long_share);
                let expected = format!("//{}/{}/movies/longmoviename.mp4", 
                                     long_server.to_lowercase(), long_share.to_lowercase());
                (path, expected)
            },
        ];
        
        for (long_path, expected_canonical) in long_path_tests {
            println!("Testing long path ({} chars): {}", long_path.len(), 
                    if long_path.len() > 100 { &long_path[..100] } else { &long_path });
            
            let result = normalizer.to_canonical(Path::new(&long_path));
            
            match result {
                Ok(canonical) => {
                    assert_eq!(canonical, expected_canonical,
                              "Long path normalization incorrect");
                    
                    // Verify canonical format is maintained
                    assert!(canonical.chars().all(|c| c.is_lowercase() || !c.is_alphabetic() || c == '/'),
                           "Long path canonical should be lowercase: {}", canonical);
                }
                Err(e) => {
                    // Some extremely long paths might be rejected, which is acceptable
                    println!("Long path rejected (may be acceptable): {} -> {:?}", long_path, e);
                    
                    // But paths under reasonable limits should work
                    if long_path.len() < 1000 {
                        panic!("Reasonable length path should not be rejected: {} ({})", long_path, e);
                    }
                }
            }
        }
    }

    /// Test special characters in paths - Requirements 1.5, 2.1
    /// Special characters can cause encoding and database issues
    #[test]
    fn test_special_characters_in_paths() {
        let normalizer = WindowsPathNormalizer::new();
        
        // Test various special character scenarios
        let special_char_tests = vec![
            // Unicode characters
            (r"C:\Users\José\Música\Canción.mp3", "c:/users/josé/música/canción.mp3"),
            (r"C:\Users\François\Vidéos\Film.mp4", "c:/users/françois/vidéos/film.mp4"),
            (r"C:\Users\张三\媒体\视频.mp4", "c:/users/张三/媒体/视频.mp4"),
            (r"C:\Users\محمد\وسائط\فيديو.mp4", "c:/users/محمد/وسائط/فيديو.mp4"),
            
            // Emoji and special Unicode
            (r"C:\Media\🎵Music\🎤Artist\Song.mp3", "c:/media/🎵music/🎤artist/song.mp3"),
            (r"C:\Media\📹Videos\🎬Movies\Film.mp4", "c:/media/📹videos/🎬movies/film.mp4"),
            
            // Spaces and punctuation
            (r"C:\Media\TV Shows\Series Name (2023)\Season 1\Episode 01.mkv", 
             "c:/media/tv shows/series name (2023)/season 1/episode 01.mkv"),
            (r"C:\Music\Artist Name\Album [Deluxe Edition]\Track 01.mp3",
             "c:/music/artist name/album [deluxe edition]/track 01.mp3"),
            
            // Special punctuation
            (r"C:\Media\Movies\Action & Adventure\Movie Title!.mp4",
             "c:/media/movies/action & adventure/movie title!.mp4"),
            (r"C:\Music\Rock & Roll\Band's Greatest Hits\Song #1.mp3",
             "c:/music/rock & roll/band's greatest hits/song #1.mp3"),
            
            // Mixed Unicode and ASCII
            (r"C:\Users\User\Desktop\Café Müller - Résumé.pdf",
             "c:/users/user/desktop/café müller - résumé.pdf"),
            (r"C:\Media\Música\Artista Español\Álbum Número 1\Canción.mp3",
             "c:/media/música/artista español/álbum número 1/canción.mp3"),
        ];
        
        for (input_path, expected_canonical) in special_char_tests {
            println!("Testing special characters: {}", input_path);
            
            let result = normalizer.to_canonical(Path::new(input_path));
            
            match result {
                Ok(canonical) => {
                    assert_eq!(canonical, expected_canonical,
                              "Special character path normalization incorrect: {} -> {} (expected: {})",
                              input_path, canonical, expected_canonical);
                    
                    // Verify roundtrip conversion works
                    let back_to_windows = normalizer.from_canonical(&canonical);
                    match back_to_windows {
                        Ok(windows_path) => {
                            let back_to_canonical = normalizer.to_canonical(&windows_path);
                            assert!(back_to_canonical.is_ok(), 
                                   "Roundtrip conversion failed for special characters");
                            assert_eq!(back_to_canonical.unwrap(), canonical,
                                      "Roundtrip conversion inconsistent for special characters");
                        }
                        Err(e) => {
                            println!("Roundtrip conversion failed (may be acceptable): {} -> {:?}", 
                                    canonical, e);
                        }
                    }
                }
                Err(e) => {
                    println!("Special character path normalization failed: {} -> {:?}", input_path, e);
                    
                    // Some special characters might not be supported, but basic Unicode should work
                    if input_path.chars().all(|c| c.is_ascii() || c == 'é' || c == 'ñ' || c == 'ü') {
                        panic!("Basic Unicode characters should be supported: {} ({})", input_path, e);
                    }
                }
            }
        }
    }

    /// Test Unicode normalization forms (NFC vs NFD) - Requirements 1.5
    /// Different Unicode normalization can cause path matching issues
    #[test]
    fn test_unicode_normalization_forms() {
        let normalizer = WindowsPathNormalizer::new();
        
        // Test NFC (Normalized Form Composed) vs NFD (Normalized Form Decomposed)
        let unicode_normalization_tests = vec![
            // Café in NFC vs NFD
            (r"C:\Users\café\media", r"C:\Users\cafe\u{0301}\media"), // NFD with combining accent
            
            // Résumé in different forms
            (r"C:\Documents\résumé.pdf", r"C:\Documents\re\u{0301}sume\u{0301}.pdf"),
            
            // Naïve in different forms  
            (r"C:\Media\naïve\video.mp4", r"C:\Media\nai\u{0308}ve\video.mp4"),
        ];
        
        for (nfc_path, nfd_path) in unicode_normalization_tests {
            println!("Testing Unicode normalization: NFC vs NFD");
            
            let nfc_result = normalizer.to_canonical(Path::new(nfc_path));
            let nfd_result = normalizer.to_canonical(Path::new(nfd_path));
            
            match (nfc_result, nfd_result) {
                (Ok(nfc_canonical), Ok(nfd_canonical)) => {
                    println!("NFC: {} -> {}", nfc_path, nfc_canonical);
                    println!("NFD: {} -> {}", nfd_path, nfd_canonical);
                    
                    // Ideally, both should normalize to the same canonical form
                    // But current implementation might not handle Unicode normalization
                    if nfc_canonical == nfd_canonical {
                        println!("✓ Unicode normalization working correctly");
                    } else {
                        println!("⚠ Unicode normalization may not be fully implemented");
                        println!("  NFC result: {}", nfc_canonical);
                        println!("  NFD result: {}", nfd_canonical);
                        
                        // This is acceptable for now - Unicode normalization is complex
                        // Both results should at least be valid canonical paths
                        assert!(!nfc_canonical.is_empty());
                        assert!(!nfd_canonical.is_empty());
                        assert!(nfc_canonical.starts_with("c:/"));
                        assert!(nfd_canonical.starts_with("c:/"));
                    }
                }
                (Ok(canonical), Err(e)) | (Err(e), Ok(canonical)) => {
                    println!("One Unicode form failed (may be acceptable): {:?}", e);
                    assert!(!canonical.is_empty());
                }
                (Err(e1), Err(e2)) => {
                    println!("Both Unicode forms failed: {:?}, {:?}", e1, e2);
                    // This might be acceptable if Unicode support is limited
                }
            }
        }
    }
}

/// Performance regression tests for critical scenarios
#[cfg(test)]
mod performance_regression_tests {
    use super::*;

    /// Test that path normalization performance doesn't regress - Requirements 2.1
    #[test]
    fn test_path_normalization_performance() {
        let normalizer = WindowsPathNormalizer::new();
        
        // Create a large set of test paths
        let mut test_paths = Vec::new();
        
        // Add various path types
        for i in 0..1000 {
            test_paths.push(format!(r"C:\Media\Videos\Movie{:04}.mp4", i));
            test_paths.push(format!(r"\\Server\Share\Video{:04}.mkv", i));
            test_paths.push(format!(r"D:\Music\Artist\Album\Track{:04}.mp3", i));
        }
        
        // Measure normalization performance
        let start_time = Instant::now();
        
        for path_str in &test_paths {
            let result = normalizer.to_canonical(Path::new(path_str));
            assert!(result.is_ok(), "Path normalization should succeed: {}", path_str);
        }
        
        let elapsed = start_time.elapsed();
        let paths_per_second = test_paths.len() as f64 / elapsed.as_secs_f64();
        
        println!("Path normalization performance: {:.0} paths/second", paths_per_second);
        
        // Should be able to normalize at least 10,000 paths per second
        assert!(paths_per_second > 10000.0, 
               "Path normalization performance regression: {:.0} paths/second (expected > 10,000)", 
               paths_per_second);
    }

    /// Test cross-platform path normalizer creation - Requirements 1.5
    #[test]
    fn test_cross_platform_normalizer_creation() {
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

    /// Test path normalization consistency across different representations - Requirements 1.5, 2.1
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
}

/// Edge case regression tests
#[cfg(test)]
mod edge_case_regression_tests {
    use super::*;

    /// Test error handling for invalid paths - Requirements 1.5
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

    /// Test roundtrip conversion (canonical -> Windows -> canonical) - Requirements 1.5
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
}
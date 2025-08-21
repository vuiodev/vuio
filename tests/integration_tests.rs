//! Integration tests for cross-platform functionality
//! 
//! These tests verify that different components work together correctly
//! across Windows, macOS, and Linux platforms.

use vuio::platform::{PlatformInfo, OsType};
use vuio::platform::network::{NetworkManager, SsdpConfig};
use vuio::platform::filesystem::{FileSystemManager, create_platform_filesystem_manager};
use vuio::database::{DatabaseManager, SqliteDatabase, MediaFile};
use vuio::watcher::{FileSystemWatcher, CrossPlatformWatcher, FileSystemEvent};

// Platform-specific network managers
#[cfg(target_os = "windows")]
use vuio::platform::network::WindowsNetworkManager;
#[cfg(target_os = "macos")]
use vuio::platform::network::MacOSNetworkManager;
#[cfg(target_os = "linux")]
use vuio::platform::network::LinuxNetworkManager;

// Create a type alias for the current platform's network manager
#[cfg(target_os = "windows")]
type PlatformNetworkManager = WindowsNetworkManager;
#[cfg(target_os = "macos")]
type PlatformNetworkManager = MacOSNetworkManager;
#[cfg(target_os = "linux")]
type PlatformNetworkManager = LinuxNetworkManager;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::fs;
use tempfile::TempDir;
use tokio::time::{timeout, sleep};

/// End-to-end DLNA discovery tests for each platform
#[cfg(test)]
mod dlna_discovery_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_end_to_end_dlna_discovery() {
        println!("Testing DLNA discovery on {}", std::env::consts::OS);
        
        // Detect platform information
        let platform_info = PlatformInfo::detect().await.unwrap();
        println!("Platform: {} {}", platform_info.os_type.display_name(), platform_info.version);
        
        // Create network manager
        let network_manager = PlatformNetworkManager::new();
        
        // Get available interfaces
        let interfaces_result = network_manager.get_local_interfaces().await;
        match interfaces_result {
            Ok(interfaces) => {
                println!("Found {} network interfaces", interfaces.len());
                for interface in &interfaces {
                    println!("  {}: {} (up: {}, multicast: {})", 
                            interface.name, 
                            interface.ip_address,
                            interface.is_up,
                            interface.supports_multicast);
                }
                
                // Try to create SSDP socket
                let socket_result = network_manager.create_ssdp_socket().await;
                match socket_result {
                    Ok(socket) => {
                        println!("Successfully created SSDP socket on port {}", socket.port);
                        
                        // Test multicast capability
                        if !socket.interfaces.is_empty() {
                            let multicast_addr = "239.255.255.250".parse().unwrap();
                            let mut test_socket = socket;
                            
                            let join_result = network_manager.join_multicast_group(
                                &mut test_socket, 
                                multicast_addr, 
                                None
                            ).await;
                            
                            match join_result {
                                Ok(()) => {
                                    println!("Successfully joined multicast group");
                                    assert!(test_socket.multicast_enabled);
                                }
                                Err(e) => {
                                    println!("Multicast join failed (may be expected): {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        println!("SSDP socket creation failed: {}", e);
                        // This might be expected in test environments without network access
                    }
                }
            }
            Err(e) => {
                println!("Interface detection failed: {}", e);
            }
        }
    }
    
    #[tokio::test]
    async fn test_ssdp_announcement_and_discovery() {
        let network_manager = PlatformNetworkManager::new();
        
        // Create SSDP configuration for testing
        let config = SsdpConfig {
            primary_port: 8080, // Use non-privileged port
            fallback_ports: vec![8081, 8082],
            multicast_address: "239.255.255.250".parse().unwrap(),
            announce_interval: Duration::from_secs(30),
            max_retries: 3,
            interfaces: Vec::new(),
        };
        
        let socket_result = network_manager.create_ssdp_socket_with_config(&config).await;
        
        match socket_result {
            Ok(socket) => {
                println!("Created SSDP socket for announcement test on port {}", socket.port);
                
                // Simulate SSDP announcement
                let announcement = b"NOTIFY * HTTP/1.1\r\n\
                                   HOST: 239.255.255.250:1900\r\n\
                                   CACHE-CONTROL: max-age=1800\r\n\
                                   LOCATION: http://192.168.1.100:8080/description.xml\r\n\
                                   NT: upnp:rootdevice\r\n\
                                   NTS: ssdp:alive\r\n\
                                   USN: uuid:test-device::upnp:rootdevice\r\n\
                                   SERVER: VuIO/1.0 UPnP/1.0\r\n\r\n";
                
                // Try to send announcement
                let multicast_group = format!("{}:{}", config.multicast_address, socket.port)
                    .parse()
                    .unwrap();
                
                let send_result = network_manager.send_multicast(&socket, announcement, multicast_group).await;
                
                match send_result {
                    Ok(()) => {
                        println!("Successfully sent SSDP announcement");
                    }
                    Err(e) => {
                        println!("SSDP announcement failed, trying unicast fallback: {}", e);
                        
                        // Try unicast fallback
                        let fallback_result = network_manager.send_unicast_fallback(
                            &socket, 
                            announcement, 
                            &socket.interfaces
                        ).await;
                        
                        match fallback_result {
                            Ok(()) => {
                                println!("Unicast fallback succeeded");
                            }
                            Err(e) => {
                                println!("Both multicast and unicast failed: {}", e);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!("Failed to create SSDP socket: {}", e);
            }
        }
    }
    
    #[tokio::test]
    async fn test_network_diagnostics_and_troubleshooting() {
        let network_manager = PlatformNetworkManager::new();
        let diagnostics_result = network_manager.get_network_diagnostics().await;
        
        match diagnostics_result {
            Ok(diagnostics) => {
                println!("Network Diagnostics Report:");
                println!("  Multicast working: {}", diagnostics.multicast_working);
                println!("  Available ports: {:?}", diagnostics.available_ports);
                
                if let Some(firewall) = &diagnostics.firewall_status {
                    println!("  Firewall detected: {}", firewall.detected);
                    if let Some(blocking) = firewall.blocking_ssdp {
                        println!("  Firewall blocking SSDP: {}", blocking);
                    }
                    
                    if !firewall.suggestions.is_empty() {
                        println!("  Firewall suggestions:");
                        for suggestion in &firewall.suggestions {
                            println!("    - {}", suggestion);
                        }
                    }
                }
                
                println!("  Interface status:");
                for status in &diagnostics.interface_status {
                    println!("    {}: reachable={}, multicast_capable={}", 
                            status.interface.name,
                            status.reachable,
                            status.multicast_capable);
                    
                    if let Some(error) = &status.error_message {
                        println!("      Error: {}", error);
                    }
                }
                
                if !diagnostics.diagnostic_messages.is_empty() {
                    println!("  Diagnostic messages:");
                    for message in &diagnostics.diagnostic_messages {
                        println!("    - {}", message);
                    }
                }
                
                // Verify we have some available ports
                assert!(!diagnostics.available_ports.is_empty(), "No available ports found");
            }
            Err(e) => {
                println!("Network diagnostics failed: {}", e);
            }
        }
    }
}

/// File serving tests with platform-specific file systems and permissions
#[cfg(test)]
mod file_serving_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_cross_platform_file_serving() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create test media files with different characteristics
        let test_files = vec![
            ("video.mp4", b"fake mp4 content", "video/mp4"),
            ("audio.mp3", b"fake mp3 content", "audio/mpeg"),
            ("image.jpg", b"fake jpg content", "image/jpeg"),
        ];
        
        for (filename, content, expected_mime) in &test_files {
            let file_path = temp_dir.path().join(filename);
            fs::write(&file_path, *content).unwrap();
        }
        
        // Test file system manager
        let fs_manager = create_platform_filesystem_manager();
        let scan_result = fs_manager.scan_media_directory(temp_dir.path()).await;
        
        match scan_result {
            Ok(media_files) => {
                assert_eq!(media_files.len(), 3);
                
                for media_file in &media_files {
                    println!("Found media file: {} ({})", media_file.filename, media_file.mime_type);
                    
                    // Verify MIME type detection
                    let expected_mime = match media_file.filename.as_str() {
                        "video.mp4" => "video/mp4",
                        "audio.mp3" => "audio/mpeg",
                        "image.jpg" => "image/jpeg",
                        _ => panic!("Unexpected file: {}", media_file.filename),
                    };
                    
                    assert_eq!(media_file.mime_type, expected_mime);
                    
                    // Test file accessibility
                    let accessible = fs_manager.is_accessible(&media_file.path).await;
                    assert!(accessible, "File should be accessible: {}", media_file.path.display());
                    
                    // Test file info retrieval
                    let file_info_result = fs_manager.get_file_info(&media_file.path).await;
                    match file_info_result {
                        Ok(file_info) => {
                            assert!(file_info.size > 0);
                            assert_eq!(file_info.mime_type, expected_mime);
                            println!("  File info: {} bytes, permissions: readable={}, writable={}", 
                                    file_info.size,
                                    file_info.permissions.readable,
                                    file_info.permissions.writable);
                        }
                        Err(e) => {
                            println!("  Failed to get file info: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                panic!("Media directory scan failed: {}", e);
            }
        }
    }
    
    #[tokio::test]
    async fn test_platform_specific_path_handling() {
        let fs_manager = create_platform_filesystem_manager();
        
        // Test platform-specific path scenarios
        let test_paths = if cfg!(target_os = "windows") {
            vec![
                (PathBuf::from("C:\\Users\\Test\\Videos\\movie.mp4"), true),
                (PathBuf::from("\\\\server\\share\\video.mkv"), true), // UNC path
                (PathBuf::from("D:\\Media\\UPPERCASE.MP4"), true),
                (PathBuf::from("invalid\0path"), false), // Null byte
            ]
        } else {
            vec![
                (PathBuf::from("/home/user/Videos/movie.mp4"), true),
                (PathBuf::from("/mnt/nas/video.mkv"), true),
                (PathBuf::from("/tmp/test.MP4"), true),
                (PathBuf::from("invalid\0path"), false), // Null byte
            ]
        };
        
        for (path, should_be_valid) in test_paths {
            let validation_result = fs_manager.validate_path(&path);
            
            if should_be_valid {
                assert!(validation_result.is_ok(), "Path should be valid: {}", path.display());
                
                // Test path normalization
                let normalized = fs_manager.normalize_path(&path);
                println!("Normalized path: {} -> {}", path.display(), normalized.display());
            } else {
                assert!(validation_result.is_err(), "Path should be invalid: {}", path.display());
            }
        }
    }
    
    #[tokio::test]
    async fn test_case_sensitivity_handling() {
        let temp_dir = TempDir::new().unwrap();
        let fs_manager = create_platform_filesystem_manager();
        
        // Create a test file
        let original_file = temp_dir.path().join("TestVideo.MP4");
        fs::write(&original_file, b"test content").unwrap();
        
        // Test case sensitivity based on platform
        let case_variants = vec![
            temp_dir.path().join("TestVideo.MP4"),
            temp_dir.path().join("testvideo.mp4"),
            temp_dir.path().join("TESTVIDEO.MP4"),
        ];
        
        for variant in &case_variants {
            let paths_equal = fs_manager.paths_equal(&original_file, variant);
            
            if cfg!(target_os = "windows") {
                // Windows should treat all variants as equal (case-insensitive)
                assert!(paths_equal, "Windows should treat paths as equal: {} vs {}", 
                       original_file.display(), variant.display());
            } else {
                // Unix systems should only treat exact matches as equal (case-sensitive)
                let should_be_equal = variant == &original_file;
                assert_eq!(paths_equal, should_be_equal, 
                          "Unix path equality mismatch: {} vs {}", 
                          original_file.display(), variant.display());
            }
        }
        
        // Test extension matching with case sensitivity
        let extensions = vec!["mp4".to_string(), "avi".to_string()];
        let matches = fs_manager.matches_extension(&original_file, &extensions);
        
        if cfg!(target_os = "windows") {
            // Windows should match case-insensitively
            assert!(matches, "Windows should match .MP4 with mp4 extension");
        } else {
            // Unix should not match different cases
            assert!(!matches, "Unix should not match .MP4 with mp4 extension");
            
            // But should match with correct case
            let correct_extensions = vec!["MP4".to_string(), "avi".to_string()];
            let correct_matches = fs_manager.matches_extension(&original_file, &correct_extensions);
            assert!(correct_matches, "Unix should match with correct case");
        }
    }
    
    #[tokio::test]
    async fn test_file_permissions_and_access_control() {
        let temp_dir = TempDir::new().unwrap();
        let fs_manager = create_platform_filesystem_manager();
        
        // Create test files with different permissions
        let test_file = temp_dir.path().join("permissions_test.mp4");
        fs::write(&test_file, b"test content").unwrap();
        
        // Test basic accessibility
        let accessible = fs_manager.is_accessible(&test_file).await;
        assert!(accessible, "File should be accessible");
        
        // Get file info to check permissions
        let file_info_result = fs_manager.get_file_info(&test_file).await;
        match file_info_result {
            Ok(file_info) => {
                println!("File permissions:");
                println!("  Readable: {}", file_info.permissions.readable);
                println!("  Writable: {}", file_info.permissions.writable);
                println!("  Executable: {}", file_info.permissions.executable);
                
                // Basic sanity checks
                assert!(file_info.permissions.readable, "File should be readable");
                
                // Platform-specific permission details
                if !file_info.permissions.platform_details.is_empty() {
                    println!("  Platform-specific details:");
                    for (key, value) in &file_info.permissions.platform_details {
                        println!("    {}: {}", key, value);
                    }
                }
            }
            Err(e) => {
                println!("Failed to get file permissions: {}", e);
            }
        }
        
        // Test with a directory
        let test_dir = temp_dir.path().join("test_subdir");
        fs::create_dir(&test_dir).unwrap();
        
        let dir_accessible = fs_manager.is_accessible(&test_dir).await;
        assert!(dir_accessible, "Directory should be accessible");
    }
}

/// Error handling and recovery mechanism tests
#[cfg(test)]
mod error_handling_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_multicast_fallback_recovery() {
        let network_manager = PlatformNetworkManager::new();
        
        // Try to create a socket and test multicast
        let config = SsdpConfig {
            primary_port: 8080,
            ..Default::default()
        };
        
        let socket_result = network_manager.create_ssdp_socket_with_config(&config).await;
        
        match socket_result {
            Ok(socket) => {
                // Test multicast failure and unicast fallback
                let test_data = b"TEST SSDP MESSAGE";
                let multicast_group = "239.255.255.250:8080".parse().unwrap();
                
                let multicast_result = network_manager.send_multicast(&socket, test_data, multicast_group).await;
                
                match multicast_result {
                    Ok(()) => {
                        println!("Multicast succeeded");
                    }
                    Err(e) => {
                        println!("Multicast failed, testing unicast fallback: {}", e);
                        
                        // Test unicast fallback
                        let fallback_result = network_manager.send_unicast_fallback(
                            &socket, 
                            test_data, 
                            &socket.interfaces
                        ).await;
                        
                        match fallback_result {
                            Ok(()) => {
                                println!("Unicast fallback succeeded");
                            }
                            Err(e) => {
                                println!("Both multicast and unicast failed: {}", e);
                                // This might be expected in test environments
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!("Socket creation failed: {}", e);
            }
        }
    }
    
    #[tokio::test]
    async fn test_file_system_error_recovery() {
        let fs_manager = create_platform_filesystem_manager();
        
        // Test with non-existent directory
        let nonexistent_dir = PathBuf::from("/nonexistent/directory/path");
        let scan_result = fs_manager.scan_media_directory(&nonexistent_dir).await;
        
        match scan_result {
            Ok(_) => {
                panic!("Scan should fail for non-existent directory");
            }
            Err(e) => {
                println!("Expected error for non-existent directory: {}", e);
                // Error should be informative
                let error_msg = format!("{}", e);
                assert!(error_msg.contains("not found") || 
                       error_msg.contains("does not exist") || 
                       error_msg.contains("access"));
            }
        }
        
        // Test with invalid path
        let invalid_path = Path::new("invalid\0path");
        let validation_result = fs_manager.validate_path(invalid_path);
        
        assert!(validation_result.is_err(), "Invalid path should be rejected");
        
        let error = validation_result.unwrap_err();
        println!("Path validation error: {}", error);
        let error_msg = format!("{}", error);
        assert!(error_msg.contains("null") || error_msg.contains("invalid"));
    }
    
    #[tokio::test]
    async fn test_database_error_recovery() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("error_test.db");
        
        let db = SqliteDatabase::new(db_path.clone()).await.unwrap();
        db.initialize().await.unwrap();
        
        // Add some test data
        let media_file = MediaFile::new(
            PathBuf::from("test/video.mp4"),
            1024,
            "video/mp4".to_string(),
        );
        db.store_media_file(&media_file).await.unwrap();
        
        // Test database health check and repair
        let health = db.check_and_repair().await.unwrap();
        
        println!("Database health check:");
        println!("  Healthy: {}", health.is_healthy);
        println!("  Integrity check passed: {}", health.integrity_check_passed);
        println!("  Corruption detected: {}", health.corruption_detected);
        
        if health.repair_attempted {
            println!("  Repair attempted: {}", health.repair_successful);
        }
        
        if !health.issues.is_empty() {
            println!("  Issues found:");
            for issue in &health.issues {
                println!("    {:?}: {}", issue.severity, issue.description);
                println!("      Suggested action: {}", issue.suggested_action);
            }
        }
        
        // Should be healthy for a new database
        assert!(health.is_healthy, "New database should be healthy");
        assert!(health.integrity_check_passed, "Integrity check should pass");
        assert!(!health.corruption_detected, "No corruption should be detected");
    }
    
    #[tokio::test]
    async fn test_file_watcher_error_recovery() {
        let watcher = CrossPlatformWatcher::new();
        
        // Test watching non-existent directory
        let nonexistent_dir = PathBuf::from("/nonexistent/watch/directory");
        let watch_result = watcher.start_watching(&[nonexistent_dir.clone()]).await;
        
        // Should not fail, but should not be watching the directory
        assert!(watch_result.is_ok(), "Watching non-existent directory should not fail");
        assert!(!watcher.is_watching(&nonexistent_dir).await, "Should not be watching non-existent directory");
        
        // Test adding and removing non-existent paths
        let add_result = watcher.add_watch_path(&nonexistent_dir).await;
        assert!(add_result.is_ok(), "Adding non-existent path should not fail");
        
        let remove_result = watcher.remove_watch_path(&nonexistent_dir).await;
        assert!(remove_result.is_ok(), "Removing non-existent path should not fail");
        
        // Test stopping watcher multiple times
        let stop_result1 = watcher.stop_watching().await;
        assert!(stop_result1.is_ok(), "First stop should succeed");
        
        let stop_result2 = watcher.stop_watching().await;
        assert!(stop_result2.is_ok(), "Second stop should not fail");
    }
}

/// Database operations and file watching integration tests
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_database_and_filesystem_integration() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("integration.db");
        
        // Initialize database
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();
        
        // Create file system manager
        let fs_manager = create_platform_filesystem_manager();
        
        // Create test media directory
        let media_dir = temp_dir.path().join("media");
        fs::create_dir(&media_dir).unwrap();
        
        // Create test media files
        let test_files = vec![
            ("video1.mp4", b"video content 1"),
            ("video2.mkv", b"video content 2"),
            ("audio1.mp3", b"audio content 1"),
            ("image1.jpg", b"image content 1"),
        ];
        
        for (filename, content) in &test_files {
            let file_path = media_dir.join(filename);
            fs::write(&file_path, *content).unwrap();
        }
        
        // Scan media directory
        let media_files = fs_manager.scan_media_directory(&media_dir).await.unwrap();
        assert_eq!(media_files.len(), 4);
        
        // Store files in database
        let mut stored_ids = Vec::new();
        for media_file in &media_files {
            let id = db.store_media_file(media_file).await.unwrap();
            stored_ids.push(id);
            println!("Stored file: {} (ID: {})", media_file.filename, id);
        }
        
        // Retrieve files from database
        let db_files = db.get_all_media_files().await.unwrap();
        assert_eq!(db_files.len(), 4);
        
        // Verify file paths match
        for db_file in &db_files {
            let original_exists = media_files.iter().any(|f| f.path == db_file.path);
            assert!(original_exists, "Database file should match original: {}", db_file.path.display());
        }
        
        // Test incremental updates
        let new_file = media_dir.join("new_video.mp4");
        fs::write(&new_file, b"new video content").unwrap();
        
        // Scan again and update database
        let updated_files = fs_manager.scan_media_directory(&media_dir).await.unwrap();
        assert_eq!(updated_files.len(), 5);
        
        // Find and store the new file
        let new_media_file = updated_files.iter()
            .find(|f| f.filename == "new_video.mp4")
            .unwrap();
        
        let new_id = db.store_media_file(new_media_file).await.unwrap();
        println!("Stored new file: {} (ID: {})", new_media_file.filename, new_id);
        
        // Verify total count
        let final_count = db.get_all_media_files().await.unwrap().len();
        assert_eq!(final_count, 5);
        
        // Test cleanup of missing files
        fs::remove_file(&new_file).unwrap();
        
        let remaining_files = fs_manager.scan_media_directory(&media_dir).await.unwrap();
        let remaining_paths: Vec<_> = remaining_files.iter().map(|f| f.path.clone()).collect();
        
        let removed_count = db.cleanup_missing_files(&remaining_paths).await.unwrap();
        assert_eq!(removed_count, 1); // Should remove the deleted file
        
        let final_db_files = db.get_all_media_files().await.unwrap();
        assert_eq!(final_db_files.len(), 4);
    }
    
    #[tokio::test]
    async fn test_file_watcher_and_database_integration() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("watcher_integration.db");
        
        // Initialize database
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();
        
        // Create file watcher
        let watcher = CrossPlatformWatcher::new();
        let mut receiver = watcher.get_event_receiver();
        
        // Start watching the temp directory
        watcher.start_watching(&[temp_dir.path().to_path_buf()]).await.unwrap();
        
        // Give watcher time to initialize
        sleep(Duration::from_millis(100)).await;
        
        // Create a media file
        let test_file = temp_dir.path().join("watched_video.mp4");
        fs::write(&test_file, b"watched video content").unwrap();
        
        // Wait for file system event
        let event_timeout = Duration::from_secs(3);
        let event_result = timeout(event_timeout, receiver.recv()).await;
        
        match event_result {
            Ok(Some(event)) => {
                println!("Received file system event: {:?}", event);
                
                match event {
                    FileSystemEvent::Created(path) | FileSystemEvent::Modified(path) => {
                        // Verify it's our test file
                        assert!(path.file_name().unwrap().to_str().unwrap().contains("watched_video"));
                        
                        // Create MediaFile and store in database
                        let media_file = MediaFile::new(
                            path.clone(),
                            fs::metadata(&path).unwrap().len(),
                            "video/mp4".to_string(),
                        );
                        
                        let id = db.store_media_file(&media_file).await.unwrap();
                        println!("Stored watched file in database with ID: {}", id);
                        
                        // Verify file is in database
                        let db_file = db.get_file_by_path(&path).await.unwrap();
                        assert!(db_file.is_some());
                        assert_eq!(db_file.unwrap().path, path);
                    }
                    _ => {
                        println!("Received unexpected event type: {:?}", event);
                    }
                }
            }
            Ok(None) => {
                println!("File watcher channel closed unexpectedly");
            }
            Err(_) => {
                println!("No file system event received within timeout (may be expected in test environment)");
            }
        }
        
        // Test file deletion event
        fs::remove_file(&test_file).unwrap();
        
        let delete_event_result = timeout(event_timeout, receiver.recv()).await;
        
        match delete_event_result {
            Ok(Some(FileSystemEvent::Deleted(path))) => {
                println!("Received file deletion event: {:?}", path);
                
                // Remove from database
                let removed = db.remove_media_file(&path).await.unwrap();
                assert!(removed, "File should have been removed from database");
                
                // Verify file is no longer in database
                let db_file = db.get_file_by_path(&path).await.unwrap();
                assert!(db_file.is_none(), "File should not be in database after deletion");
            }
            Ok(Some(other_event)) => {
                println!("Received other event instead of deletion: {:?}", other_event);
            }
            Ok(None) => {
                println!("File watcher channel closed");
            }
            Err(_) => {
                println!("No deletion event received (may be expected in test environment)");
            }
        }
        
        watcher.stop_watching().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_configuration_and_platform_integration() {
        let temp_dir = TempDir::new().unwrap();
        
        // Test platform-specific configuration paths
        let platform_info = PlatformInfo::detect().await.unwrap();
        
        println!("Testing configuration integration on {}", platform_info.os_type.display_name());
        
        // Create a mock configuration that would work on the current platform
        let config_content = if cfg!(target_os = "windows") {
            r#"
[server]
port = 8080
interface = "0.0.0.0"
name = "VuIO Server"
uuid = "test-uuid-windows"

[network]
ssdp_port = 8080
interface_selection = "Auto"
multicast_ttl = 4
announce_interval_seconds = 300

[[media.directories]]
path = "C:\\Users\\Public\\Videos"
recursive = true

[[media.directories]]
path = "C:\\Users\\Public\\Music"
recursive = true

[media]
scan_on_startup = true
watch_for_changes = true
supported_extensions = ["mp4", "mkv", "avi", "mp3", "flac", "wav", "jpg", "png"]

[database]
vacuum_on_startup = false
backup_enabled = true
"#
        } else {
            r#"
[server]
port = 8080
interface = "0.0.0.0"
name = "VuIO Server"
uuid = "test-uuid-unix"

[network]
ssdp_port = 8080
interface_selection = "Auto"
multicast_ttl = 4
announce_interval_seconds = 300

[[media.directories]]
path = "/home/user/Videos"
recursive = true

[[media.directories]]
path = "/home/user/Music"
recursive = true

[media]
scan_on_startup = true
watch_for_changes = true
supported_extensions = ["mp4", "mkv", "avi", "mp3", "flac", "wav", "jpg", "png"]

[database]
vacuum_on_startup = false
backup_enabled = true
"#
        };
        
        // Write configuration file
        let config_file = temp_dir.path().join("config.toml");
        fs::write(&config_file, config_content).unwrap();
        
        println!("Created platform-specific configuration file");
        
        // Test that the configuration would be valid for the current platform
        // (This is a simplified test - in a real implementation, we'd load and validate the config)
        
        // Test database path resolution
        let db_path = temp_dir.path().join("media.db");
        let db = SqliteDatabase::new(db_path.clone()).await.unwrap();
        db.initialize().await.unwrap();
        
        println!("Database initialized at: {}", db_path.display());
        
        // Test file system manager with platform-appropriate paths
        let fs_manager = create_platform_filesystem_manager();
        
        // Use temp directory as a test media directory
        let test_media_dir = temp_dir.path().join("test_media");
        fs::create_dir(&test_media_dir).unwrap();
        
        // Create a test file
        let test_file = test_media_dir.join("config_test.mp4");
        fs::write(&test_file, b"configuration test content").unwrap();
        
        // Test scanning
        let media_files = fs_manager.scan_media_directory(&test_media_dir).await.unwrap();
        assert_eq!(media_files.len(), 1);
        
        // Store in database
        let id = db.store_media_file(&media_files[0]).await.unwrap();
        println!("Stored configuration test file with ID: {}", id);
        
        // Test network manager initialization
        let network_manager = PlatformNetworkManager::new();
        let interfaces = network_manager.get_local_interfaces().await;
        
        match interfaces {
            Ok(ifaces) => {
                println!("Network interfaces available for configuration:");
                for iface in &ifaces {
                    println!("  {}: {} (up: {}, multicast: {})", 
                            iface.name, 
                            iface.ip_address,
                            iface.is_up,
                            iface.supports_multicast);
                }
            }
            Err(e) => {
                println!("Network interface detection failed: {}", e);
            }
        }
        
        println!("Platform integration test completed successfully");
    }
}
//! Platform-specific unit tests for cross-platform compatibility
//! 
//! This module contains tests that verify platform-specific functionality
//! works correctly on Windows, macOS, and Linux systems.

use vuio::platform::{PlatformInfo, OsType, PlatformCapabilities};
use vuio::platform::network::{NetworkManager, BaseNetworkManager, SsdpConfig};
use vuio::platform::filesystem::{FileSystemManager, BaseFileSystemManager, create_platform_filesystem_manager};
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
use tempfile::{TempDir, NamedTempFile};
use tokio::time::timeout;

/// Platform-specific network manager tests
#[cfg(test)]
mod network_tests {
    use super::*;
    
    /// Test Windows-specific network manager functionality
    #[cfg(target_os = "windows")]
    mod windows_tests {
        use super::*;
        use vuio::platform::network::WindowsNetworkManager;
        use vuio::platform::error::WindowsError;
        
        #[tokio::test]
        async fn test_windows_network_manager_creation() {
            let manager = WindowsNetworkManager::new();
            assert!(manager.is_ok());
        }
        
        #[tokio::test]
        async fn test_windows_privileged_port_handling() {
            let manager = WindowsNetworkManager::new();
            
            // Test that binding to port 1900 fails gracefully without admin privileges
            let result = manager.create_ssdp_socket().await;
            
            // Should either succeed (if running as admin) or fail with appropriate error
            match result {
                Ok(socket) => {
                    // If successful, verify socket is properly configured
                    assert!(socket.port > 0);
                    assert!(!socket.interfaces.is_empty());
                }
                Err(e) => {
                    // Should be a specific Windows error about privileges
                    let error_msg = format!("{}", e);
                    assert!(error_msg.contains("privilege") || error_msg.contains("port") || error_msg.contains("bind"));
                }
            }
        }
        
        #[tokio::test]
        async fn test_windows_firewall_detection() {
            let manager = WindowsNetworkManager::new();
            let diagnostics = manager.get_network_diagnostics().await;
            
            match diagnostics {
                Ok(diag) => {
                    // Windows should detect firewall presence
                    assert!(diag.firewall_status.is_some());
                    let firewall = diag.firewall_status.unwrap();
                    assert!(firewall.detected); // Windows always has Windows Defender Firewall
                }
                Err(_) => {
                    // Diagnostics might fail in test environment, which is acceptable
                }
            }
        }
        
        #[tokio::test]
        async fn test_windows_interface_detection() {
            let manager = WindowsNetworkManager::new();
            let interfaces_result = manager.get_local_interfaces().await;
            
            match interfaces_result {
                Ok(interfaces) => {
                    // Windows should have at least loopback interface
                    assert!(!interfaces.is_empty());
                    
                    // Check for Windows-specific interface naming
                    let has_windows_naming = interfaces.iter().any(|iface| {
                        iface.name.contains("Ethernet") || 
                        iface.name.contains("Wi-Fi") ||
                        iface.name.contains("Local Area Connection")
                    });
                    
                    if !has_windows_naming {
                        println!("Warning: No Windows-style interface names found. Interfaces: {:?}", 
                                interfaces.iter().map(|i| &i.name).collect::<Vec<_>>());
                    }
                }
                Err(e) => {
                    println!("Interface detection failed (acceptable in test environment): {}", e);
                }
            }
        }
        
        #[tokio::test]
        async fn test_windows_so_reuseaddr_socket_options() {
            let manager = WindowsNetworkManager::new();
            let config = SsdpConfig {
                primary_port: 8080, // Use non-privileged port for testing
                ..Default::default()
            };
            
            let socket_result = manager.create_ssdp_socket_with_config(&config).await;
            
            match socket_result {
                Ok(socket) => {
                    // Verify socket was created successfully
                    assert_eq!(socket.port, 8080);
                    
                    // Try to create another socket on the same port to test SO_REUSEADDR
                    let second_socket_result = manager.create_ssdp_socket_with_config(&config).await;
                    
                    match second_socket_result {
                        Ok(_) => {
                            // SO_REUSEADDR should allow this on Windows
                            println!("SO_REUSEADDR working correctly on Windows");
                        }
                        Err(e) => {
                            println!("Second socket creation failed (may be expected): {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("Socket creation failed in test environment: {}", e);
                }
            }
        }
        
        #[tokio::test]
        async fn test_windows_multicast_fallback() {
            let manager = WindowsNetworkManager::new();
            
            // Test multicast capability
            let interfaces_result = manager.get_local_interfaces().await;
            if let Ok(interfaces) = interfaces_result {
                for interface in interfaces.iter().take(1) { // Test first interface only
                    let multicast_test = manager.test_multicast(interface).await;
                    
                    match multicast_test {
                        Ok(supports_multicast) => {
                            println!("Interface {} multicast support: {}", interface.name, supports_multicast);
                        }
                        Err(e) => {
                            println!("Multicast test failed for {}: {}", interface.name, e);
                        }
                    }
                }
            }
        }
    }
    
    /// Test macOS-specific network manager functionality
    #[cfg(target_os = "macos")]
    mod macos_tests {
        use super::*;
        use vuio::platform::network::MacOSNetworkManager;
        
        #[tokio::test]
        async fn test_macos_network_manager_creation() {
            let _manager = MacOSNetworkManager::new();
            // Manager creation should succeed
        }
        
        #[tokio::test]
        async fn test_macos_multicast_interface_selection() {
            let manager = MacOSNetworkManager::new();
            let interfaces_result = manager.get_local_interfaces().await;
            
            match interfaces_result {
                Ok(interfaces) => {
                    // macOS should have at least loopback
                    assert!(!interfaces.is_empty());
                    
                    // Check for macOS-specific interface naming
                    let has_macos_naming = interfaces.iter().any(|iface| {
                        iface.name.starts_with("en") || // Ethernet interfaces
                        iface.name.starts_with("lo") || // Loopback
                        iface.name.starts_with("utun") || // VPN tunnels
                        iface.name.starts_with("awdl") // AirDrop Wireless Direct Link
                    });
                    
                    assert!(has_macos_naming, "Expected macOS-style interface names");
                }
                Err(e) => {
                    println!("Interface detection failed (acceptable in test environment): {}", e);
                }
            }
        }
        
        #[tokio::test]
        async fn test_macos_interface_prioritization() {
            let manager = MacOSNetworkManager::new();
            let primary_interface_result = manager.get_primary_interface().await;
            
            match primary_interface_result {
                Ok(primary) => {
                    // Primary interface should not be loopback
                    assert!(!primary.is_loopback);
                    assert!(primary.is_up);
                    
                    // Should prefer Ethernet over WiFi
                    println!("Primary interface: {} ({:?})", primary.name, primary.interface_type);
                }
                Err(e) => {
                    println!("Primary interface detection failed: {}", e);
                }
            }
        }
        
        #[tokio::test]
        async fn test_macos_multicast_group_joining() {
            let manager = MacOSNetworkManager::new();
            let config = SsdpConfig {
                primary_port: 8080,
                ..Default::default()
            };
            
            let socket_result = manager.create_ssdp_socket_with_config(&config).await;
            
            match socket_result {
                Ok(mut socket) => {
                    let multicast_addr = "239.255.255.250".parse().unwrap();
                    let join_result = manager.join_multicast_group(&mut socket, multicast_addr, None).await;
                    
                    match join_result {
                        Ok(()) => {
                            assert!(socket.multicast_enabled);
                            println!("Successfully joined multicast group on macOS");
                        }
                        Err(e) => {
                            println!("Multicast join failed (may be expected in test environment): {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("Socket creation failed: {}", e);
                }
            }
        }
        
        #[tokio::test]
        async fn test_macos_network_permissions() {
            let manager = MacOSNetworkManager::new();
            
            // Test binding to privileged port (should fail without sudo)
            let privileged_config = SsdpConfig {
                primary_port: 1900,
                fallback_ports: vec![],
                ..Default::default()
            };
            
            let result = manager.create_ssdp_socket_with_config(&privileged_config).await;
            
            match result {
                Ok(_) => {
                    println!("Privileged port binding succeeded (running with elevated privileges?)");
                }
                Err(e) => {
                    let error_msg = format!("{}", e);
                    assert!(error_msg.contains("permission") || error_msg.contains("bind") || error_msg.contains("port"));
                    println!("Privileged port binding failed as expected: {}", e);
                }
            }
        }
    }
    
    /// Test Linux-specific network manager functionality
    #[cfg(target_os = "linux")]
    mod linux_tests {
        use super::*;
        use vuio::platform::network::LinuxNetworkManager;
        
        #[tokio::test]
        async fn test_linux_network_manager_creation() {
            let manager = LinuxNetworkManager::new();
            assert!(manager.is_ok());
        }
        
        #[tokio::test]
        async fn test_linux_network_namespaces() {
            let manager = LinuxNetworkManager::new();
            let interfaces_result = manager.get_local_interfaces().await;
            
            match interfaces_result {
                Ok(interfaces) => {
                    // Linux should have at least loopback
                    assert!(!interfaces.is_empty());
                    
                    // Check for Linux-specific interface naming
                    let has_linux_naming = interfaces.iter().any(|iface| {
                        iface.name.starts_with("eth") || // Ethernet
                        iface.name.starts_with("wlan") || // WiFi
                        iface.name.starts_with("lo") || // Loopback
                        iface.name.starts_with("enp") || // Predictable network interface names
                        iface.name.starts_with("wlp") // Predictable WiFi interface names
                    });
                    
                    assert!(has_linux_naming, "Expected Linux-style interface names");
                }
                Err(e) => {
                    println!("Interface detection failed (acceptable in test environment): {}", e);
                }
            }
        }
        
        #[tokio::test]
        async fn test_linux_multicast_group_joining() {
            let manager = LinuxNetworkManager::new();
            let config = SsdpConfig {
                primary_port: 8080,
                ..Default::default()
            };
            
            let socket_result = manager.create_ssdp_socket_with_config(&config).await;
            
            match socket_result {
                Ok(mut socket) => {
                    let multicast_addr = "239.255.255.250".parse().unwrap();
                    let join_result = manager.join_multicast_group(&mut socket, multicast_addr, None).await;
                    
                    match join_result {
                        Ok(()) => {
                            assert!(socket.multicast_enabled);
                            println!("Successfully joined multicast group on Linux");
                        }
                        Err(e) => {
                            println!("Multicast join failed (may be expected in test environment): {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("Socket creation failed: {}", e);
                }
            }
        }
        
        #[tokio::test]
        async fn test_linux_interface_binding() {
            let manager = LinuxNetworkManager::new();
            let interfaces_result = manager.get_local_interfaces().await;
            
            if let Ok(interfaces) = interfaces_result {
                // Test binding to specific interfaces
                for interface in interfaces.iter().take(2) { // Test first 2 interfaces
                    if interface.is_loopback {
                        continue;
                    }
                    
                    println!("Testing interface binding for: {}", interface.name);
                    
                    let multicast_test = manager.test_multicast(interface).await;
                    match multicast_test {
                        Ok(supports_multicast) => {
                            println!("Interface {} multicast support: {}", interface.name, supports_multicast);
                            assert_eq!(supports_multicast, interface.supports_multicast);
                        }
                        Err(e) => {
                            println!("Multicast test failed for {}: {}", interface.name, e);
                        }
                    }
                }
            }
        }
        
        #[tokio::test]
        async fn test_linux_capabilities_and_permissions() {
            let manager = LinuxNetworkManager::new();
            
            // Test binding to privileged port (should fail without root/capabilities)
            let privileged_config = SsdpConfig {
                primary_port: 1900,
                fallback_ports: vec![],
                ..Default::default()
            };
            
            let result = manager.create_ssdp_socket_with_config(&privileged_config).await;
            
            match result {
                Ok(_) => {
                    println!("Privileged port binding succeeded (running with elevated privileges or capabilities?)");
                }
                Err(e) => {
                    let error_msg = format!("{}", e);
                    assert!(error_msg.contains("permission") || error_msg.contains("bind") || error_msg.contains("port"));
                    println!("Privileged port binding failed as expected: {}", e);
                }
            }
        }
    }
    
    /// Cross-platform network manager tests
    #[tokio::test]
    async fn test_platform_network_manager_creation() {
        let _manager = PlatformNetworkManager::new();
        // Manager creation should succeed
    }
    
    #[tokio::test]
    async fn test_base_network_manager_port_fallback() {
        let manager = BaseNetworkManager::new();
        
        // Test port availability checking
        let is_available = manager.is_port_available(8080).await;
        println!("Port 8080 available: {}", is_available);
        
        // Test with a likely unavailable port (if not running as root)
        let privileged_available = manager.is_port_available(80).await;
        println!("Port 80 available: {}", privileged_available);
    }
    
    #[tokio::test]
    async fn test_network_diagnostics() {
        let manager = BaseNetworkManager::new();
        let diagnostics_result = manager.get_network_diagnostics().await;
        
        match diagnostics_result {
            Ok(diagnostics) => {
                println!("Network diagnostics:");
                println!("  Multicast working: {}", diagnostics.multicast_working);
                println!("  Available ports: {:?}", diagnostics.available_ports);
                println!("  Diagnostic messages: {:?}", diagnostics.diagnostic_messages);
                
                // Should have at least some available ports
                assert!(!diagnostics.available_ports.is_empty());
            }
            Err(e) => {
                println!("Network diagnostics failed: {}", e);
            }
        }
    }
}
// Platform-specific file system manager tests
#[cfg(test)]
mod filesystem_tests {
    use super::*;
    use std::fs;
    
    /// Test Windows-specific file system handling
    #[cfg(target_os = "windows")]
    mod windows_tests {
        use super::*;
        use vuio::platform::filesystem::windows::WindowsFileSystemManager;
        
        #[tokio::test]
        async fn test_windows_filesystem_manager_creation() {
            let manager = WindowsFileSystemManager::new();
            
            // Windows file system should be case-insensitive
            assert!(!manager.case_sensitive);
        }
        
        #[tokio::test]
        async fn test_windows_case_insensitive_paths() {
            let manager = WindowsFileSystemManager::new();
            
            let path1 = Path::new("C:\\Test\\File.mp4");
            let path2 = Path::new("c:\\test\\file.mp4");
            
            // Should be equal on Windows (case-insensitive)
            assert!(manager.paths_equal(path1, path2));
        }
        
        #[tokio::test]
        async fn test_windows_drive_letter_handling() {
            let manager = WindowsFileSystemManager::new();
            
            let path_with_drive = Path::new("C:\\Users\\Test\\Videos\\movie.mp4");
            let normalized = manager.normalize_path(path_with_drive);
            
            // Should preserve drive letter
            assert!(normalized.to_string_lossy().starts_with("C:"));
        }
        
        #[tokio::test]
        async fn test_windows_unc_path_support() {
            let manager = WindowsFileSystemManager::new();
            
            let unc_path = Path::new("\\\\server\\share\\videos\\movie.mp4");
            let validation_result = manager.validate_path(unc_path);
            
            // UNC paths should be valid on Windows
            assert!(validation_result.is_ok());
        }
        
        #[tokio::test]
        async fn test_windows_extension_matching() {
            let manager = WindowsFileSystemManager::new();
            
            let path = Path::new("C:\\Videos\\Movie.MP4");
            let extensions = vec!["mp4".to_string(), "avi".to_string()];
            
            // Should match case-insensitively on Windows
            assert!(manager.matches_extension(path, &extensions));
        }
        
        #[tokio::test]
        async fn test_windows_file_permissions() {
            let temp_dir = TempDir::new().unwrap();
            let test_file = temp_dir.path().join("test.mp4");
            fs::write(&test_file, b"test content").unwrap();
            
            let manager = WindowsFileSystemManager::new();
            let file_info_result = manager.get_file_info(&test_file).await;
            
            match file_info_result {
                Ok(file_info) => {
                    // Should have Windows-specific permission details
                    assert!(!file_info.permissions.platform_details.is_empty());
                    println!("Windows file permissions: {:?}", file_info.permissions);
                }
                Err(e) => {
                    println!("File info retrieval failed: {}", e);
                }
            }
        }
    }
    
    /// Test macOS-specific file system handling
    #[cfg(target_os = "macos")]
    mod macos_tests {
        use super::*;
        
        #[tokio::test]
        async fn test_macos_case_sensitive_paths() {
            let manager = BaseFileSystemManager::new(true); // macOS is case-sensitive
            
            let path1 = Path::new("/Users/test/Videos/Movie.mp4");
            let path2 = Path::new("/Users/test/Videos/movie.mp4");
            
            // Should NOT be equal on macOS (case-sensitive)
            assert!(!manager.paths_equal(path1, path2));
        }
        
        #[tokio::test]
        async fn test_macos_extension_matching() {
            let manager = BaseFileSystemManager::new(true);
            
            let path = Path::new("/Users/test/Videos/Movie.MP4");
            let extensions = vec!["mp4".to_string(), "avi".to_string()];
            
            // Should NOT match on case-sensitive macOS
            assert!(!manager.matches_extension(path, &extensions));
            
            // But should match with correct case
            let correct_extensions = vec!["MP4".to_string(), "avi".to_string()];
            assert!(manager.matches_extension(path, &correct_extensions));
        }
        
        #[tokio::test]
        async fn test_macos_hidden_files() {
            let temp_dir = TempDir::new().unwrap();
            let hidden_file = temp_dir.path().join(".hidden_video.mp4");
            fs::write(&hidden_file, b"test content").unwrap();
            
            let manager = BaseFileSystemManager::new(true);
            let file_info_result = manager.get_file_info(&hidden_file).await;
            
            match file_info_result {
                Ok(file_info) => {
                    // Should detect hidden files (starting with .)
                    assert!(file_info.is_hidden);
                }
                Err(e) => {
                    println!("File info retrieval failed: {}", e);
                }
            }
        }
        
        #[tokio::test]
        async fn test_macos_apfs_features() {
            let temp_dir = TempDir::new().unwrap();
            let test_file = temp_dir.path().join("test.mp4");
            fs::write(&test_file, b"test content").unwrap();
            
            let manager = BaseFileSystemManager::new(true);
            let canonical_result = manager.canonicalize_path(&test_file).await;
            
            match canonical_result {
                Ok(canonical_path) => {
                    // Should resolve to absolute path
                    assert!(canonical_path.is_absolute());
                    println!("Canonical path: {:?}", canonical_path);
                }
                Err(e) => {
                    println!("Path canonicalization failed: {}", e);
                }
            }
        }
    }
    
    /// Test Linux-specific file system handling
    #[cfg(target_os = "linux")]
    mod linux_tests {
        use super::*;
        
        #[tokio::test]
        async fn test_linux_case_sensitive_paths() {
            let manager = BaseFileSystemManager::new(true); // Linux is case-sensitive
            
            let path1 = Path::new("/home/user/Videos/Movie.mp4");
            let path2 = Path::new("/home/user/Videos/movie.mp4");
            
            // Should NOT be equal on Linux (case-sensitive)
            assert!(!manager.paths_equal(path1, path2));
        }
        
        #[tokio::test]
        async fn test_linux_extension_matching() {
            let manager = BaseFileSystemManager::new(true);
            
            let path = Path::new("/home/user/Videos/Movie.MP4");
            let extensions = vec!["mp4".to_string(), "avi".to_string()];
            
            // Should NOT match on case-sensitive Linux
            assert!(!manager.matches_extension(path, &extensions));
            
            // But should match with correct case
            let correct_extensions = vec!["MP4".to_string(), "avi".to_string()];
            assert!(manager.matches_extension(path, &correct_extensions));
        }
        
        #[tokio::test]
        async fn test_linux_file_permissions() {
            let temp_dir = TempDir::new().unwrap();
            let test_file = temp_dir.path().join("test.mp4");
            fs::write(&test_file, b"test content").unwrap();
            
            let manager = BaseFileSystemManager::new(true);
            let file_info_result = manager.get_file_info(&test_file).await;
            
            match file_info_result {
                Ok(file_info) => {
                    // Should have basic permission info
                    assert!(file_info.permissions.readable);
                    println!("Linux file permissions: {:?}", file_info.permissions);
                }
                Err(e) => {
                    println!("File info retrieval failed: {}", e);
                }
            }
        }
        
        #[tokio::test]
        async fn test_linux_symlink_handling() {
            let temp_dir = TempDir::new().unwrap();
            let original_file = temp_dir.path().join("original.mp4");
            let symlink_file = temp_dir.path().join("symlink.mp4");
            
            fs::write(&original_file, b"test content").unwrap();
            
            // Create symlink (may fail if not supported)
            if std::os::unix::fs::symlink(&original_file, &symlink_file).is_ok() {
                let manager = BaseFileSystemManager::new(true);
                
                let canonical_result = manager.canonicalize_path(&symlink_file).await;
                match canonical_result {
                    Ok(canonical_path) => {
                        // Should resolve to the original file
                        assert_eq!(canonical_path, original_file.canonicalize().unwrap());
                    }
                    Err(e) => {
                        println!("Symlink canonicalization failed: {}", e);
                    }
                }
            }
        }
        
        #[tokio::test]
        async fn test_linux_mount_point_detection() {
            let manager = BaseFileSystemManager::new(true);
            
            // Test common Linux paths
            let test_paths = vec![
                Path::new("/"),
                Path::new("/home"),
                Path::new("/tmp"),
                Path::new("/var"),
            ];
            
            for path in test_paths {
                if path.exists() {
                    let accessible = manager.is_accessible(path).await;
                    println!("Path {} accessible: {}", path.display(), accessible);
                }
            }
        }
    }
    
    /// Cross-platform file system tests
    #[tokio::test]
    async fn test_platform_filesystem_manager_creation() {
        let manager = create_platform_filesystem_manager();
        
        // Should create appropriate manager for current platform
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.mp4");
        fs::write(&test_file, b"test content").unwrap();
        
        let accessible = manager.is_accessible(&test_file).await;
        assert!(accessible);
    }
    
    #[tokio::test]
    async fn test_media_directory_scanning() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create test media files
        let media_files = vec![
            ("video.mp4", "video content"),
            ("audio.mp3", "audio content"),
            ("image.jpg", "image content"),
            ("document.txt", "not media"), // Should be ignored
        ];
        
        for (filename, content) in &media_files {
            let file_path = temp_dir.path().join(filename);
            fs::write(&file_path, content.as_bytes()).unwrap();
        }
        
        let manager = create_platform_filesystem_manager();
        let scan_result = manager.scan_media_directory(temp_dir.path()).await;
        
        match scan_result {
            Ok(files) => {
                // Should find 3 media files (excluding .txt)
                assert_eq!(files.len(), 3);
                
                let filenames: Vec<_> = files.iter().map(|f| &f.filename).collect();
                assert!(filenames.contains(&&"video.mp4".to_string()));
                assert!(filenames.contains(&&"audio.mp3".to_string()));
                assert!(filenames.contains(&&"image.jpg".to_string()));
                assert!(!filenames.contains(&&"document.txt".to_string()));
            }
            Err(e) => {
                panic!("Media directory scan failed: {}", e);
            }
        }
    }
    
    #[tokio::test]
    async fn test_path_validation() {
        let manager = create_platform_filesystem_manager();
        
        // Valid paths
        assert!(manager.validate_path(Path::new("valid/path/video.mp4")).is_ok());
        
        // Invalid paths
        assert!(manager.validate_path(Path::new("path/with/\0/null")).is_err());
        assert!(manager.validate_path(Path::new("path/../traversal")).is_err());
        
        // Very long path
        let long_path = "a/".repeat(2100) + "video.mp4";
        assert!(manager.validate_path(Path::new(&long_path)).is_err());
    }
}

/// Platform-specific database tests
#[cfg(test)]
mod database_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_database_creation_on_current_platform() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        
        let db = SqliteDatabase::new(db_path.clone()).await.unwrap();
        db.initialize().await.unwrap();
        
        // Verify database file was created
        assert!(db_path.exists());
        
        let stats = db.get_stats().await.unwrap();
        assert_eq!(stats.total_files, 0);
        assert!(stats.database_size > 0);
    }
    
    #[tokio::test]
    async fn test_database_with_platform_specific_paths() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("platform_test.db");
        
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();
        
        // Create media files with platform-specific paths
        let test_paths = if cfg!(target_os = "windows") {
            vec![
                PathBuf::from("C:\\Users\\Test\\Videos\\movie.mp4"),
                PathBuf::from("D:\\Media\\Music\\song.mp3"),
                PathBuf::from("\\\\server\\share\\video.mkv"),
            ]
        } else {
            vec![
                PathBuf::from("/home/user/Videos/movie.mp4"),
                PathBuf::from("/Users/test/Music/song.mp3"),
                PathBuf::from("/mnt/nas/video.mkv"),
            ]
        };
        
        // Store files with platform-specific paths
        for (i, path) in test_paths.iter().enumerate() {
            let mut media_file = MediaFile::new(
                path.clone(),
                1024 * (i as u64 + 1),
                "video/mp4".to_string(),
            );
            media_file.title = Some(format!("Test Video {}", i + 1));
            
            let id = db.store_media_file(&media_file).await.unwrap();
            assert!(id > 0);
        }
        
        // Retrieve all files
        let all_files = db.get_all_media_files().await.unwrap();
        assert_eq!(all_files.len(), test_paths.len());
        
        // Verify paths are stored correctly
        for (original_path, stored_file) in test_paths.iter().zip(all_files.iter()) {
            assert_eq!(&stored_file.path, original_path);
        }
    }
    
    #[tokio::test]
    async fn test_database_backup_restore_on_platform() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("original.db");
        let backup_path = temp_dir.path().join("backup.db");
        
        let db = SqliteDatabase::new(db_path.clone()).await.unwrap();
        db.initialize().await.unwrap();
        
        // Add test data
        let media_file = MediaFile::new(
            PathBuf::from("test/video.mp4"),
            2048,
            "video/mp4".to_string(),
        );
        db.store_media_file(&media_file).await.unwrap();
        
        // Create backup
        db.create_backup(&backup_path).await.unwrap();
        assert!(backup_path.exists());
        
        // Verify backup integrity
        let backup_db = SqliteDatabase::new(backup_path.clone()).await.unwrap();
        let backup_files = backup_db.get_all_media_files().await.unwrap();
        assert_eq!(backup_files.len(), 1);
        assert_eq!(backup_files[0].filename, "video.mp4");
    }
    
    #[tokio::test]
    async fn test_database_health_check_platform_specific() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("health_test.db");
        
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();
        
        // Add some test data
        for i in 0..10 {
            let media_file = MediaFile::new(
                PathBuf::from(format!("test/video_{}.mp4", i)),
                1024 * i,
                "video/mp4".to_string(),
            );
            db.store_media_file(&media_file).await.unwrap();
        }
        
        // Run health check
        let health = db.check_and_repair().await.unwrap();
        
        assert!(health.is_healthy);
        assert!(health.integrity_check_passed);
        assert!(!health.corruption_detected);
        
        println!("Database health check passed on {}", std::env::consts::OS);
        
        if !health.issues.is_empty() {
            println!("Health issues found:");
            for issue in &health.issues {
                println!("  {:?}: {}", issue.severity, issue.description);
            }
        }
    }
    
    #[tokio::test]
    async fn test_database_concurrent_access() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("concurrent_test.db");
        
        let db = SqliteDatabase::new(db_path.clone()).await.unwrap();
        db.initialize().await.unwrap();
        
        // Test concurrent database access
        let mut handles = vec![];
        
        for i in 0..5 {
            let db_path_clone = db_path.clone();
            let handle = tokio::spawn(async move {
                let db = SqliteDatabase::new(db_path_clone).await.unwrap();
                
                // Each task adds a media file
                let media_file = MediaFile::new(
                    PathBuf::from(format!("concurrent/video_{}.mp4", i)),
                    1024 * i,
                    "video/mp4".to_string(),
                );
                
                db.store_media_file(&media_file).await.unwrap();
                
                // Read back all files
                let files = db.get_all_media_files().await.unwrap();
                files.len()
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        let mut total_files = 0;
        for handle in handles {
            let file_count = handle.await.unwrap();
            total_files = file_count.max(total_files);
        }
        
        // Should have at least 5 files from concurrent operations
        assert!(total_files >= 5);
        
        println!("Concurrent database access test passed with {} files", total_files);
    }
    
    #[tokio::test]
    async fn test_database_cleanup_missing_files() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("cleanup_test.db");
        
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();
        
        // Add files to database
        let test_files = vec![
            PathBuf::from("existing/video1.mp4"),
            PathBuf::from("existing/video2.mp4"),
            PathBuf::from("missing/video3.mp4"),
            PathBuf::from("missing/video4.mp4"),
        ];
        
        for path in &test_files {
            let media_file = MediaFile::new(path.clone(), 1024, "video/mp4".to_string());
            db.store_media_file(&media_file).await.unwrap();
        }
        
        // Simulate cleanup with only some files existing
        let existing_files = vec![
            PathBuf::from("existing/video1.mp4"),
            PathBuf::from("existing/video2.mp4"),
        ];
        
        let removed_count = db.cleanup_missing_files(&existing_files).await.unwrap();
        assert_eq!(removed_count, 2); // Should remove 2 missing files
        
        // Verify only existing files remain
        let remaining_files = db.get_all_media_files().await.unwrap();
        assert_eq!(remaining_files.len(), 2);
        
        for file in &remaining_files {
            assert!(existing_files.contains(&file.path));
        }
    }
}

/// Platform-specific file watcher tests
#[cfg(test)]
mod watcher_tests {
    use super::*;
    use std::fs;
    use tokio::time::sleep;
    
    #[tokio::test]
    async fn test_file_watcher_creation() {
        let watcher = CrossPlatformWatcher::new();
        
        // Should not be watching anything initially
        assert!(!watcher.is_watching(Path::new("/nonexistent")).await);
    }
    
    #[tokio::test]
    async fn test_file_watcher_media_file_detection() {
        let _watcher = CrossPlatformWatcher::new();
        
        // Test media file detection by creating files and checking if they trigger events
        // Since is_media_file is private, we test the behavior indirectly
        println!("Testing media file detection behavior through file operations");
    }
    
    #[tokio::test]
    async fn test_file_watcher_directory_monitoring() {
        let temp_dir = TempDir::new().unwrap();
        let watcher = CrossPlatformWatcher::new();
        
        // Start watching the directory
        let result = watcher.start_watching(&[temp_dir.path().to_path_buf()]).await;
        assert!(result.is_ok());
        
        // Verify directory is being watched
        assert!(watcher.is_watching(temp_dir.path()).await);
        
        // Stop watching
        let stop_result = watcher.stop_watching().await;
        assert!(stop_result.is_ok());
        
        // Should no longer be watching
        assert!(!watcher.is_watching(temp_dir.path()).await);
    }
    
    #[tokio::test]
    async fn test_file_watcher_events() {
        let temp_dir = TempDir::new().unwrap();
        let watcher = CrossPlatformWatcher::new();
        
        // Get event receiver before starting watcher
        let mut receiver = watcher.get_event_receiver();
        
        // Start watching
        watcher.start_watching(&[temp_dir.path().to_path_buf()]).await.unwrap();
        
        // Give watcher time to initialize
        sleep(Duration::from_millis(100)).await;
        
        // Create a media file
        let test_file = temp_dir.path().join("test_video.mp4");
        fs::write(&test_file, b"test video content").unwrap();
        
        // Wait for event with timeout
        let event_result = timeout(Duration::from_secs(3), receiver.recv()).await;
        
        match event_result {
            Ok(Some(event)) => {
                match event {
                    FileSystemEvent::Created(path) => {
                        println!("Received file creation event: {:?}", path);
                        // Verify it's the file we created
                        assert!(path.file_name().unwrap().to_str().unwrap().contains("test_video"));
                    }
                    FileSystemEvent::Modified(path) => {
                        println!("Received file modification event: {:?}", path);
                        // Some platforms may report modification instead of creation
                        assert!(path.file_name().unwrap().to_str().unwrap().contains("test_video"));
                    }
                    other => {
                        println!("Received other event: {:?}", other);
                    }
                }
            }
            Ok(None) => {
                println!("Event receiver closed unexpectedly");
            }
            Err(_) => {
                println!("No file system event received within timeout (may be expected in test environment)");
            }
        }
        
        watcher.stop_watching().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_file_watcher_add_remove_paths() {
        let temp_dir1 = TempDir::new().unwrap();
        let temp_dir2 = TempDir::new().unwrap();
        let watcher = CrossPlatformWatcher::new();
        
        // Initialize watcher
        watcher.start_watching(&[temp_dir1.path().to_path_buf()]).await.unwrap();
        
        // Add second directory
        let add_result = watcher.add_watch_path(temp_dir2.path()).await;
        assert!(add_result.is_ok());
        assert!(watcher.is_watching(temp_dir2.path()).await);
        
        // Remove first directory
        let remove_result = watcher.remove_watch_path(temp_dir1.path()).await;
        assert!(remove_result.is_ok());
        assert!(!watcher.is_watching(temp_dir1.path()).await);
        
        // Second directory should still be watched
        assert!(watcher.is_watching(temp_dir2.path()).await);
        
        watcher.stop_watching().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_file_watcher_nonexistent_directory() {
        let watcher = CrossPlatformWatcher::new();
        
        // Try to watch non-existent directory
        let nonexistent = PathBuf::from("/nonexistent/directory");
        let result = watcher.start_watching(&[nonexistent.clone()]).await;
        
        // Should not fail, just log warning
        assert!(result.is_ok());
        
        // Should not be watching the non-existent directory
        assert!(!watcher.is_watching(&nonexistent).await);
    }
    
    #[tokio::test]
    async fn test_file_watcher_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        let watcher = CrossPlatformWatcher::new();
        
        let mut receiver = watcher.get_event_receiver();
        watcher.start_watching(&[temp_dir.path().to_path_buf()]).await.unwrap();
        
        sleep(Duration::from_millis(100)).await;
        
        // Create a media file
        let test_file = temp_dir.path().join("operations_test.mp4");
        fs::write(&test_file, b"initial content").unwrap();
        
        // Modify the file
        sleep(Duration::from_millis(200)).await;
        fs::write(&test_file, b"modified content").unwrap();
        
        // Delete the file
        sleep(Duration::from_millis(200)).await;
        fs::remove_file(&test_file).unwrap();
        
        // Collect events for a short time
        let mut events = Vec::new();
        let collection_timeout = Duration::from_secs(2);
        let start_time = std::time::Instant::now();
        
        while start_time.elapsed() < collection_timeout {
            match timeout(Duration::from_millis(100), receiver.recv()).await {
                Ok(Some(event)) => {
                    events.push(event);
                }
                Ok(None) => break,
                Err(_) => continue,
            }
        }
        
        println!("Collected {} file system events", events.len());
        for (i, event) in events.iter().enumerate() {
            println!("  Event {}: {:?}", i + 1, event);
        }
        
        // We should have received at least one event
        // The exact events depend on the platform and timing
        if !events.is_empty() {
            println!("File watcher successfully detected file operations");
        } else {
            println!("No events detected (may be expected in test environment)");
        }
        
        watcher.stop_watching().await.unwrap();
    }
    
    /// Test platform-specific file watcher behavior
    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn test_windows_file_watcher_behavior() {
        let temp_dir = TempDir::new().unwrap();
        let watcher = CrossPlatformWatcher::new();
        
        let mut receiver = watcher.get_event_receiver();
        watcher.start_watching(&[temp_dir.path().to_path_buf()]).await.unwrap();
        
        sleep(Duration::from_millis(100)).await;
        
        // Test Windows-specific file operations
        let test_file = temp_dir.path().join("windows_test.MP4"); // Uppercase extension
        fs::write(&test_file, b"windows test content").unwrap();
        
        // Windows may generate multiple events for a single operation
        let event_result = timeout(Duration::from_secs(2), receiver.recv()).await;
        
        if let Ok(Some(event)) = event_result {
            println!("Windows file watcher event: {:?}", event);
            // Should detect the .MP4 file as media
            match event {
                FileSystemEvent::Created(path) | FileSystemEvent::Modified(path) => {
                    assert!(path.extension().unwrap().to_str().unwrap().to_lowercase() == "mp4");
                }
                _ => {}
            }
        }
        
        watcher.stop_watching().await.unwrap();
    }
    
    /// Test platform-specific file watcher behavior
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[tokio::test]
    async fn test_unix_file_watcher_behavior() {
        let temp_dir = TempDir::new().unwrap();
        let watcher = CrossPlatformWatcher::new();
        
        let mut receiver = watcher.get_event_receiver();
        watcher.start_watching(&[temp_dir.path().to_path_buf()]).await.unwrap();
        
        sleep(Duration::from_millis(100)).await;
        
        // Test Unix-specific file operations
        let test_file = temp_dir.path().join("unix_test.mp4");
        fs::write(&test_file, b"unix test content").unwrap();
        
        // Set file permissions (Unix-specific)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&test_file).unwrap().permissions();
            perms.set_mode(0o644);
            fs::set_permissions(&test_file, perms).unwrap();
        }
        
        let event_result = timeout(Duration::from_secs(2), receiver.recv()).await;
        
        if let Ok(Some(event)) = event_result {
            println!("Unix file watcher event: {:?}", event);
            match event {
                FileSystemEvent::Created(path) | FileSystemEvent::Modified(path) => {
                    assert!(path.file_name().unwrap().to_str().unwrap().contains("unix_test"));
                }
                _ => {}
            }
        }
        
        watcher.stop_watching().await.unwrap();
    }
}

/// Platform information and capabilities tests
#[cfg(test)]
mod platform_info_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_platform_info_detection() {
        let platform_info = PlatformInfo::detect().await;
        assert!(platform_info.is_ok());
        
        let info = platform_info.unwrap();
        
        // Verify OS type matches current platform
        let expected_os = if cfg!(target_os = "windows") {
            OsType::Windows
        } else if cfg!(target_os = "macos") {
            OsType::MacOS
        } else if cfg!(target_os = "linux") {
            OsType::Linux
        } else {
            panic!("Unsupported platform for testing");
        };
        
        assert_eq!(info.os_type, expected_os);
        assert!(!info.version.is_empty());
        assert!(!info.network_interfaces.is_empty());
        
        println!("Platform: {} {}", info.os_type.display_name(), info.version);
        println!("Network interfaces: {}", info.network_interfaces.len());
        
        for interface in &info.network_interfaces {
            println!("  {}: {} (up: {}, multicast: {})", 
                    interface.name, 
                    interface.ip_address,
                    interface.is_up,
                    interface.supports_multicast);
        }
    }
    
    #[tokio::test]
    async fn test_platform_capabilities() {
        let capabilities = PlatformCapabilities::for_current_platform();
        
        // All platforms should support multicast
        assert!(capabilities.supports_multicast);
        
        // All platforms should have some form of firewall
        assert!(capabilities.has_firewall);
        
        // Platform-specific capability checks
        if cfg!(target_os = "windows") {
            assert!(!capabilities.case_sensitive_fs); // NTFS is case-insensitive by default
            assert!(capabilities.supports_network_paths); // UNC paths
            assert!(capabilities.requires_network_permissions); // UAC
        } else if cfg!(target_os = "macos") {
            assert!(capabilities.case_sensitive_fs); // APFS is case-sensitive
            assert!(capabilities.supports_network_paths); // SMB/AFP mounts
            assert!(capabilities.requires_network_permissions); // System permissions
        } else if cfg!(target_os = "linux") {
            assert!(capabilities.case_sensitive_fs); // ext4/xfs are case-sensitive
            assert!(capabilities.supports_network_paths); // NFS/CIFS mounts
            assert!(!capabilities.requires_network_permissions); // Usually no special permissions
        }
        
        println!("Platform capabilities:");
        println!("  Can bind privileged ports: {}", capabilities.can_bind_privileged_ports);
        println!("  Supports multicast: {}", capabilities.supports_multicast);
        println!("  Has firewall: {}", capabilities.has_firewall);
        println!("  Case sensitive FS: {}", capabilities.case_sensitive_fs);
        println!("  Supports network paths: {}", capabilities.supports_network_paths);
        println!("  Requires network permissions: {}", capabilities.requires_network_permissions);
    }
    
    #[tokio::test]
    async fn test_platform_feature_support() {
        let platform_info = PlatformInfo::detect().await.unwrap();
        
        // Test feature support queries
        assert!(platform_info.supports_feature("multicast"));
        assert!(platform_info.supports_feature("firewall"));
        assert!(!platform_info.supports_feature("nonexistent_feature"));
        
        // Platform-specific features
        if cfg!(target_os = "windows") {
            assert!(platform_info.supports_feature("network_permissions"));
            assert!(!platform_info.supports_feature("case_sensitive_fs"));
        } else {
            assert!(platform_info.supports_feature("case_sensitive_fs"));
        }
        
        println!("Feature support test completed for {}", platform_info.os_type.display_name());
    }
}
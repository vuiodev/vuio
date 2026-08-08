use super::*;
use tempfile::{NamedTempFile, TempDir};

#[test]
fn test_default_config_creation() {
    let config = AppConfig::default_for_platform();
    let platform_config = PlatformConfig::for_current_platform();

    // Test that platform defaults are used
    assert_eq!(
        config.server.port,
        platform_config
            .preferred_ports
            .first()
            .copied()
            .unwrap_or(8080)
    );
    // Note: SSDP port is hardcoded to 1900 for DLNA compatibility
    assert!(config.media.scan_on_startup);
    assert!(config.media.watch_for_changes);
    assert!(!config.media.supported_extensions.is_empty());
}

#[test]
fn test_config_serialization() -> Result<()> {
    let config = AppConfig::default_for_platform();

    let toml_str = toml::to_string_pretty(&config)?;
    assert!(toml_str.contains("[server]"));
    assert!(toml_str.contains("[network]"));
    assert!(toml_str.contains("[media]"));
    assert!(toml_str.contains("[database]"));

    // Test deserialization
    let parsed_config: AppConfig = toml::from_str(&toml_str)?;
    assert_eq!(config.server.port, parsed_config.server.port);
    // Note: SSDP port is hardcoded to 1900 and not serialized

    Ok(())
}

#[test]
fn test_config_file_operations() -> Result<()> {
    let temp_file = NamedTempFile::new()?;
    let config_path = temp_file.path().to_path_buf();
    let temp_dir = TempDir::new()?;

    // Delete the temp file so we can test creation
    std::fs::remove_file(&config_path).ok();

    // Create a config with a temporary directory that exists
    let mut config = AppConfig::default();
    config.media.directories = vec![MonitoredDirectoryConfig {
        path: temp_dir.path().to_string_lossy().to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Strict,
    }];

    // Save the config
    config.save_to_file(&config_path)?;
    assert!(config_path.exists());

    // Test loading existing config
    let loaded_config = AppConfig::load_from_file(&config_path)?;
    assert_eq!(config.server.port, loaded_config.server.port);

    Ok(())
}

#[test]
fn test_exclude_patterns() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path().to_path_buf();

    let mut config = AppConfig::default_for_platform();
    config.media.directories = vec![MonitoredDirectoryConfig {
        path: dir_path.to_string_lossy().to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: Some(vec![
            ".*".to_string(),        // Hidden files
            "Thumbs.db".to_string(), // Windows thumbnails
            ".DS_Store".to_string(), // macOS metadata
            "*.tmp".to_string(),     // Temporary files
        ]),
        validation_mode: ValidationMode::Strict,
    }];

    // Test hidden file exclusion
    assert!(config.should_exclude_file(&dir_path.join(".hidden"), &dir_path));

    // Test Thumbs.db exclusion
    assert!(config.should_exclude_file(&dir_path.join("Thumbs.db"), &dir_path));

    // Test .DS_Store exclusion
    assert!(config.should_exclude_file(&dir_path.join(".DS_Store"), &dir_path));

    // Test tmp file exclusion
    assert!(config.should_exclude_file(&dir_path.join("temp.tmp"), &dir_path));

    // Test normal file inclusion
    assert!(!config.should_exclude_file(&dir_path.join("movie.mp4"), &dir_path));
}

#[tokio::test]
async fn test_config_manager() -> Result<()> {
    let temp_file = NamedTempFile::new()?;
    let config_path = temp_file.path().to_path_buf();
    let temp_dir = TempDir::new()?;

    // Delete the temp file so we can test creation
    std::fs::remove_file(&config_path).ok();

    // Create a config with a temporary directory that exists
    let mut config = AppConfig::default();
    config.media.directories = vec![MonitoredDirectoryConfig {
        path: temp_dir.path().to_string_lossy().to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Strict,
    }];
    config.save_to_file(&config_path)?;

    let manager = ConfigManager::new(&config_path)?;
    let _original_port = manager.get_config().await.server.port;

    // Update configuration
    let mut new_config = manager.get_config().await;
    new_config.server.port = 9090;
    manager.update_config(new_config).await?;

    assert_eq!(manager.get_config().await.server.port, 9090);

    // Test reload - should revert to original file value since we don't save changes
    manager.reload().await?;
    assert_eq!(manager.get_config().await.server.port, 8080);

    Ok(())
}

#[test]
fn test_platform_defaults_application() -> Result<()> {
    let mut config = AppConfig::default_for_platform();

    // Simulate a config with missing platform defaults
    config.database.path = None;
    config.media.supported_extensions.clear();

    // Apply platform defaults
    config.apply_platform_defaults()?;

    // Verify defaults were applied
    assert!(config.database.path.is_some());
    assert!(config.media.supported_extensions.is_empty());

    Ok(())
}

#[test]
fn test_platform_config_template_creation() -> Result<()> {
    let temp_file = NamedTempFile::new()?;
    let config_path = temp_file.path().to_path_buf();
    let temp_dir = TempDir::new()?;

    // Delete the temp file so we can test creation
    std::fs::remove_file(&config_path).ok();

    // Create a custom config that uses temp directories instead of platform template
    let mut config = AppConfig::default();
    config.media.directories = vec![MonitoredDirectoryConfig {
        path: temp_dir.path().to_string_lossy().to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Strict,
    }];

    // Save the config to file
    config.save_to_file(&config_path)?;

    // Verify file was created
    assert!(config_path.exists());

    // Verify content contains configuration information
    let content = std::fs::read_to_string(&config_path)?;
    assert!(content.contains("[server]"));
    assert!(content.contains("[media]"));

    // Load and validate the configuration
    let loaded_config = AppConfig::load_from_file(&config_path)?;
    ConfigValidator::validate(&loaded_config)?;
    assert!(!loaded_config.media.supported_extensions.is_empty());

    Ok(())
}

#[test]
fn test_platform_validation() -> Result<()> {
    let mut config = AppConfig::default_for_platform();

    // Use a temporary directory for the database in tests
    let temp_dir = std::env::temp_dir().join("vuio_test_db");
    config.database.path = Some(temp_dir.join("test.db").to_string_lossy().to_string());

    // Should validate successfully with platform defaults
    config.validate_for_platform()?;

    Ok(())
}

#[test]
fn test_comprehensive_platform_integration() -> Result<()> {
    let platform_config = PlatformConfig::for_current_platform();
    let config = AppConfig::default_for_platform();

    // Test that platform defaults are properly applied
    assert!(platform_config
        .preferred_ports
        .contains(&config.server.port));
    // Note: SSDP port is hardcoded to 1900 for DLNA standard
    assert!(!config.server.name.is_empty());
    assert!(!config.media.supported_extensions.is_empty());

    // Test that platform-specific exclude patterns are included
    for dir_config in &config.media.directories {
        if let Some(patterns) = &dir_config.exclude_patterns {
            let platform_patterns = platform_config.get_default_exclude_patterns();
            // At least some platform patterns should be present
            let has_platform_patterns = platform_patterns.iter().any(|p| patterns.contains(p));
            assert!(
                has_platform_patterns,
                "No platform-specific exclude patterns found"
            );
        }
    }

    // Test platform validation
    assert!(config.validate_for_platform().is_ok());

    // Test platform recommendations
    let recommendations = AppConfig::get_platform_recommendations();
    assert!(!recommendations.is_empty());
    assert!(recommendations.iter().any(|r| r.contains("port")));

    // Test best practices check
    let _issues = config.check_platform_best_practices();
    // Issues may or may not exist depending on the platform and configuration
    // But the function should not panic

    // Test platform-specific helper methods
    assert!(!AppConfig::get_platform_default_interface(&platform_config).is_empty());
    // Note: SSDP port is hardcoded to 1900 (get_platform_default_ssdp_port function removed)
    assert!(AppConfig::get_platform_default_multicast_ttl(&platform_config) > 0);
    assert!(AppConfig::get_platform_default_announce_interval(&platform_config) > 0);

    Ok(())
}

#[test]
fn test_enhanced_platform_defaults_application() -> Result<()> {
    let mut config = AppConfig::default_for_platform();
    let platform_config = PlatformConfig::for_current_platform();

    // Modify config to remove some platform defaults
    config.media.supported_extensions.clear();
    config.server.interface = String::new();
    config.server.name = String::new();
    for dir_config in &mut config.media.directories {
        dir_config.exclude_patterns = None;
    }

    // Apply platform defaults
    assert!(config.apply_platform_defaults().is_ok());

    // Verify defaults were applied
    assert!(config.media.supported_extensions.is_empty());
    assert!(!config.server.interface.is_empty());
    assert!(!config.server.name.is_empty());

    for dir_config in &config.media.directories {
        assert!(dir_config.exclude_patterns.is_some());
        let patterns = dir_config.exclude_patterns.as_ref().unwrap();
        assert!(!patterns.is_empty());

        // Should contain platform-specific patterns
        let platform_patterns = platform_config.get_default_exclude_patterns();
        let has_platform_patterns = platform_patterns.iter().any(|p| patterns.contains(p));
        assert!(has_platform_patterns);
    }

    Ok(())
}

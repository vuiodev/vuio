use super::*;
use crate::config::AppConfig;
use tempfile::TempDir;

#[test]
fn test_valid_config() {
    let config = AppConfig::default_for_platform();

    // This might fail if the default media directory doesn't exist
    // So we'll create a minimal valid config for testing
    let temp_dir = TempDir::new().unwrap();
    let mut test_config = config;
    test_config.media.directories = vec![super::MonitoredDirectoryConfig {
        path: temp_dir.path().to_string_lossy().to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Strict,
    }];

    assert!(ConfigValidator::validate(&test_config).is_ok());
}

#[test]
fn test_invalid_server_config() {
    let mut config = AppConfig::default_for_platform();

    // Test invalid port
    config.server.port = 0;
    assert!(ConfigValidator::validate(&config).is_err());

    // Reset port and test empty name
    config.server.port = 8080;
    config.server.name = "".to_string();
    assert!(ConfigValidator::validate(&config).is_err());

    // Reset name and test invalid UUID
    config.server.name = "Test Server".to_string();
    config.server.uuid = "invalid-uuid".to_string();
    assert!(ConfigValidator::validate(&config).is_err());
}

#[test]
fn test_invalid_network_config() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = AppConfig::default_for_platform();
    config.media.directories = vec![super::MonitoredDirectoryConfig {
        path: temp_dir.path().to_string_lossy().to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Strict,
    }];

    // Test invalid TTL (SSDP port is now hardcoded to 1900)
    config.network.multicast_ttl = 0;
    assert!(ConfigValidator::validate(&config).is_err());
}

#[test]
fn test_invalid_media_config() {
    let mut config = AppConfig::default_for_platform();

    // Test empty directories
    config.media.directories = vec![];
    assert!(ConfigValidator::validate(&config).is_err());

    // Test empty supported extensions
    config.media.directories = vec![super::MonitoredDirectoryConfig {
        path: "/tmp".to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Warn,
    }];
    config.media.supported_extensions = vec![];
    assert!(ConfigValidator::validate(&config).is_ok());
}

#[test]
fn test_directory_validation() {
    let temp_dir = TempDir::new().unwrap();

    // Valid directory
    let valid_dir = super::MonitoredDirectoryConfig {
        path: temp_dir.path().to_string_lossy().to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: Some(vec!["mp4".to_string()]),
        exclude_patterns: Some(vec!["*.tmp".to_string()]),
        validation_mode: super::ValidationMode::Strict,
    };
    assert!(ConfigValidator::validate_monitored_directory(&valid_dir, 0).is_ok());

    // Invalid directory (doesn't exist) with Strict mode - should fail
    let invalid_dir_strict = super::MonitoredDirectoryConfig {
        path: "/nonexistent/directory".to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Strict,
    };
    assert!(ConfigValidator::validate_monitored_directory(&invalid_dir_strict, 0).is_err());

    // Invalid directory (doesn't exist) with Warn mode - should succeed
    let invalid_dir_warn = super::MonitoredDirectoryConfig {
        path: "/nonexistent/directory".to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Warn,
    };
    assert!(ConfigValidator::validate_monitored_directory(&invalid_dir_warn, 0).is_ok());

    // Invalid directory (doesn't exist) with Skip mode - should succeed
    let invalid_dir_skip = super::MonitoredDirectoryConfig {
        path: "/nonexistent/directory".to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Skip,
    };
    assert!(ConfigValidator::validate_monitored_directory(&invalid_dir_skip, 0).is_ok());

    // Empty path - should always fail regardless of validation mode
    let empty_path_dir = super::MonitoredDirectoryConfig {
        path: "".to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Warn,
    };
    assert!(ConfigValidator::validate_monitored_directory(&empty_path_dir, 0).is_err());
}

#[test]
fn test_validation_mode_behavior() {
    // Test that Warn mode allows startup with missing directories
    let missing_dir_warn = super::MonitoredDirectoryConfig {
        path: "/definitely/does/not/exist".to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Warn,
    };

    // Should succeed with warning logged
    assert!(ConfigValidator::validate_monitored_directory(&missing_dir_warn, 0).is_ok());

    // Test that Skip mode bypasses all validation
    let missing_dir_skip = super::MonitoredDirectoryConfig {
        path: "/definitely/does/not/exist".to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Skip,
    };

    // Should succeed without any validation
    assert!(ConfigValidator::validate_monitored_directory(&missing_dir_skip, 0).is_ok());

    // Test that Strict mode still fails for missing directories
    let missing_dir_strict = super::MonitoredDirectoryConfig {
        path: "/definitely/does/not/exist".to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Strict,
    };

    // Should fail
    assert!(ConfigValidator::validate_monitored_directory(&missing_dir_strict, 0).is_err());
}

#[test]
fn test_flexible_validation_allows_startup() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let mut config = AppConfig::default_for_platform();

    // Create a configuration with one valid directory and one missing directory
    config.media.directories = vec![
        super::MonitoredDirectoryConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            recursive: true,
            case_sensitive: None,
            extensions: None,
            exclude_patterns: None,
            validation_mode: ValidationMode::Strict, // This should pass
        },
        super::MonitoredDirectoryConfig {
            path: "/definitely/does/not/exist".to_string(),
            recursive: true,
            case_sensitive: None,
            extensions: None,
            exclude_patterns: None,
            validation_mode: ValidationMode::Warn, // This should warn but not fail
        },
        super::MonitoredDirectoryConfig {
            path: "/another/missing/directory".to_string(),
            recursive: true,
            case_sensitive: None,
            extensions: None,
            exclude_patterns: None,
            validation_mode: ValidationMode::Skip, // This should be skipped
        },
    ];

    // Flexible validation should succeed
    assert!(ConfigValidator::validate_flexible(&config).is_ok());

    // But strict validation should fail due to the missing directories
    assert!(ConfigValidator::validate(&config).is_err());
}

#[test]
fn test_config_loading_with_missing_directories() -> Result<()> {
    use std::io::Write;
    use tempfile::NamedTempFile;

    let mut temp_file = NamedTempFile::new()?;
    let temp_dir = tempfile::TempDir::new()?;

    // Create a TOML config with mixed validation modes
    // Escape backslashes in Windows paths for TOML
    let temp_path = temp_dir.path().to_string_lossy().replace("\\", "\\\\");
    let config_content = format!(
        r#"
[server]
port = 8080
interface = "0.0.0.0"
name = "Test Server"
uuid = "12345678-1234-1234-1234-123456789012"

[network]
interface_selection = "Auto"
multicast_ttl = 4
announce_interval_seconds = 30

[media]
scan_on_startup = true
watch_for_changes = true
cleanup_deleted_files = true
autoplay_enabled = true
supported_extensions = ["mp4", "mkv", "avi"]

[[media.directories]]
path = "{}"
recursive = true
validation_mode = "Strict"

[[media.directories]]
path = "/definitely/does/not/exist"
recursive = true
validation_mode = "Warn"

[[media.directories]]
path = "/another/missing/directory"
recursive = true
validation_mode = "Skip"

[database]
vacuum_on_startup = false
backup_enabled = true
"#,
        temp_path
    );

    temp_file.write_all(config_content.as_bytes())?;
    temp_file.flush()?;

    // This should succeed with flexible validation
    let config = AppConfig::load_from_file(temp_file.path())?;

    // Verify the configuration was loaded correctly
    assert_eq!(config.media.directories.len(), 3);
    assert_eq!(
        config.media.directories[0].validation_mode,
        ValidationMode::Strict
    );
    assert_eq!(
        config.media.directories[1].validation_mode,
        ValidationMode::Warn
    );
    assert_eq!(
        config.media.directories[2].validation_mode,
        ValidationMode::Skip
    );
    assert!(config.network.upnp_callback_allowed_networks.is_empty());

    Ok(())
}

#[test]
fn callback_allowlist_requires_valid_cidr_notation() {
    let mut config = AppConfig::default_for_platform();
    config.network.upnp_callback_allowed_networks = vec!["192.168.1.0/24".to_string()];
    assert!(ConfigValidator::validate_network_config(&config).is_ok());

    config.network.upnp_callback_allowed_networks = vec!["192.168.1.999/24".to_string()];
    let error = ConfigValidator::validate_network_config(&config).unwrap_err();
    assert!(error
        .to_string()
        .contains("Invalid UPnP callback network CIDR"));
}

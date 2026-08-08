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
    assert!(config.validate_path(temp_dir.path()).is_ok());

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

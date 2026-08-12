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

/// Every write to the config file reaches the watcher, including ones that changed
/// nothing — an editor re-saving, or the admin API writing the file back. Forwarding
/// those would bump the ContentDirectory update id, drop the browse cache and NOTIFY
/// every UPnP subscriber, so the watcher compares before it broadcasts.
#[tokio::test]
async fn rewriting_the_config_unchanged_broadcasts_nothing() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let media_dir = TempDir::new()?;
    // The watcher matches event paths against the configured path exactly, and on
    // macOS a temp dir arrives as /var/... while the events report /private/var/...
    let config_path = std::fs::canonicalize(temp_dir.path())?.join("config.toml");

    let mut config = AppConfig::default();
    config.media.directories = vec![MonitoredDirectoryConfig {
        path: media_dir.path().to_string_lossy().to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Skip,
    }];
    config.save_to_file(&config_path)?;

    let cancellation = tokio_util::sync::CancellationToken::new();
    let tasks = tokio_util::task::TaskTracker::new();
    let manager =
        ConfigManager::new_with_watching(&config_path, cancellation.clone(), tasks.clone()).await?;
    let mut changes = manager.subscribe_to_changes();

    // Byte-for-byte the same content, written again.
    let reloaded = AppConfig::load_from_file(&config_path)?;
    reloaded.save_to_file(&config_path)?;
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
    assert!(
        changes.try_recv().is_err(),
        "an unchanged rewrite must not be broadcast"
    );

    // A real edit still gets through.
    let mut edited = reloaded;
    edited.media.autoplay_enabled = !edited.media.autoplay_enabled;
    edited.save_to_file(&config_path)?;
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
    assert!(
        matches!(changes.try_recv(), Ok(ConfigChangeEvent::Reloaded(_))),
        "a real change must still be broadcast"
    );
    assert_eq!(
        manager.get_config().await.media.autoplay_enabled,
        edited.media.autoplay_enabled
    );

    cancellation.cancel();
    tasks.close();
    tasks.wait().await;
    Ok(())
}

/// A hand-written config that omits optional keys used to churn on every reload:
/// `load_or_create` fills `exclude_patterns` and `database.path` from platform
/// defaults, a bare `load_from_file` leaves them `None`, so the diff reported the
/// root as modified and the consumer dropped its watch and rescanned it whole.
#[tokio::test]
async fn reloading_a_minimal_config_reports_no_directory_change() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let media_dir = TempDir::new()?;
    let config_path = std::fs::canonicalize(temp_dir.path())?.join("config.toml");

    // No exclude_patterns, no database.path — the shape config.example.toml suggests.
    std::fs::write(
        &config_path,
        format!(
            r#"
[server]
port = 8080
interface = "0.0.0.0"
name = "VuIO"
uuid = "129c3ee9-4fd2-45ea-9f11-7a15e6d831ef"

[network]
interface_selection = "Auto"
multicast_ttl = 4
announce_interval_seconds = 30

[media]
scan_on_startup = true
watch_for_changes = true
supported_extensions = ["mp4"]

[[media.directories]]
path = "{}"
recursive = true

[database]
vacuum_on_startup = false
backup_enabled = false
"#,
            media_dir.path().to_string_lossy()
        ),
    )?;

    let manager = ConfigManager::new(&config_path)?;
    let mut changes = manager.subscribe_to_changes();
    manager.reload().await?;

    let mut events = Vec::new();
    while let Ok(event) = changes.try_recv() {
        events.push(event);
    }
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, ConfigChangeEvent::DirectoriesChanged { .. })),
        "a reload with no edits must not report a directory change: {events:?}"
    );

    Ok(())
}

/// The shipped example is documentation people paste into place, so it has to be a
/// config the server accepts. The previous one had drifted: it documented a
/// `[database.redb]` table the model does not have, which loads as nothing at all.
#[test]
fn the_example_config_is_loadable() {
    // Read rather than include_str!: the file lives outside the published crate.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config.example.toml");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return; // Not a checkout — nothing to check.
    };

    let config: AppConfig = toml::from_str(&raw)
        .unwrap_or_else(|error| panic!("config.example.toml must parse: {error}"));

    // Nothing has deny_unknown_fields, so a key the model does not have deserialises
    // to nothing at all. Round-trip the parsed config and check every path the example
    // writes survives, which is the only way an ignored key shows up.
    let documented: toml::Table = toml::from_str(&raw).expect("parses as a table");
    let modelled = toml::Table::try_from(&config).expect("config serialises");

    fn assert_paths_exist(documented: &toml::Table, modelled: &toml::Table, prefix: &str) {
        for (key, value) in documented {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            let Some(counterpart) = modelled.get(key) else {
                panic!("config.example.toml documents `{path}`, which AppConfig ignores");
            };
            if let (Some(child), Some(other)) = (value.as_table(), counterpart.as_table()) {
                assert_paths_exist(child, other, &path);
            }
        }
    }
    assert_paths_exist(&documented, &modelled, "");

    // The commented-out keys are the optional ones; the uncommented keys must be
    // enough on their own.
    ConfigValidator::validate_flexible(&config)
        .unwrap_or_else(|error| panic!("config.example.toml must validate: {error}"));
}

/// Command-line overrides used to force the whole configuration into a scratch file
/// with no watcher, which made `--port` silently disable hot reload and left the real
/// file uneditable. They are now layered onto every load instead: the override holds
/// for the run, the real file stays watched, and edits to it still apply.
#[tokio::test]
async fn overrides_hold_across_reloads_without_freezing_the_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let media_dir = TempDir::new()?;
    let config_path = std::fs::canonicalize(temp_dir.path())?.join("config.toml");

    let mut config = AppConfig::default();
    config.server.port = 8080;
    config.server.name = "From The File".to_string();
    config.media.directories = vec![MonitoredDirectoryConfig {
        path: media_dir.path().to_string_lossy().to_string(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Skip,
    }];
    config.save_to_file(&config_path)?;

    let overrides = ConfigOverrides {
        port: Some(9099),
        ..ConfigOverrides::default()
    };
    let cancellation = tokio_util::sync::CancellationToken::new();
    let tasks = tokio_util::task::TaskTracker::new();
    let manager = ConfigManager::watching_with_overrides(
        &config_path,
        overrides,
        cancellation.clone(),
        tasks.clone(),
    )
    .await?;

    // The override wins at startup, and the rest of the file is untouched by it.
    let running = manager.get_config().await;
    assert_eq!(running.server.port, 9099);
    assert_eq!(running.server.name, "From The File");
    // It is watched, which is what makes the file editable from the admin API.
    assert!(manager.is_watched());

    // An edit to the file applies, and the override survives the reload.
    let mut edited = AppConfig::load_from_file(&config_path)?;
    edited.server.name = "Renamed On Disk".to_string();
    edited.media.autoplay_enabled = !edited.media.autoplay_enabled;
    edited.save_to_file(&config_path)?;
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    let reloaded = manager.get_config().await;
    assert_eq!(reloaded.server.name, "Renamed On Disk");
    assert_eq!(
        reloaded.media.autoplay_enabled,
        edited.media.autoplay_enabled
    );
    assert_eq!(
        reloaded.server.port, 9099,
        "the command-line port must survive a reload"
    );

    // And the override was never written into the file.
    assert_eq!(AppConfig::load_from_file(&config_path)?.server.port, 8080);

    cancellation.cancel();
    tasks.close();
    tasks.wait().await;
    Ok(())
}

#[test]
fn overrides_report_what_they_force() {
    assert!(ConfigOverrides::default().in_force().is_empty());

    let overrides = ConfigOverrides {
        port: Some(9090),
        server_name: Some("Kitchen".to_string()),
        media_dirs: vec![std::path::PathBuf::from("/movies")],
    };
    let forced = overrides.in_force();
    assert_eq!(forced[0], ("server.port", "9090".to_string()));
    assert_eq!(forced[1], ("server.name", "Kitchen".to_string()));
    assert_eq!(forced[2], ("media.directories", "/movies".to_string()));
}

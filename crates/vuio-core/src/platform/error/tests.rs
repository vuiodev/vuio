use super::*;

#[test]
fn test_windows_error_recovery() {
    let error = WindowsError::PrivilegedPortAccess { port: 1900 };
    assert!(error.is_recoverable());
    assert!(!error.recovery_actions().is_empty());
}

#[test]
fn test_database_error_recovery() {
    let error = DatabaseError::CorruptionDetected {
        location: "table_media".to_string(),
    };
    assert!(error.is_recoverable());
    assert_eq!(
        error.recovery_strategy(),
        "Database will be automatically rebuilt from media scan"
    );
}

#[test]
fn test_platform_error_user_message() {
    let error = PlatformError::Windows(WindowsError::FirewallBlocked);
    let message = error.user_message();
    assert!(message.contains("Windows Error"));
    assert!(message.contains("Troubleshooting"));
}

#[test]
fn test_configuration_error_recovery() {
    let error = ConfigurationError::FileNotFound {
        path: PathBuf::from("/etc/vuio/config.toml"),
    };
    assert!(error.is_recoverable());
    assert!(!error.recovery_actions().is_empty());
}

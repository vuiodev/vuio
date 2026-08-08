use super::*;

#[test]
fn test_unc_path_detection() {
    let manager = WindowsFileSystemManager::new();

    assert!(manager.is_unc_path(Path::new(r"\\server\share\path")));
    assert!(manager.is_unc_path(Path::new(r"\\192.168.1.100\media")));
    assert!(!manager.is_unc_path(Path::new(r"C:\local\path")));
    assert!(!manager.is_unc_path(Path::new(r"relative\path")));
}

#[test]
fn test_drive_letter_detection() {
    let manager = WindowsFileSystemManager::new();

    assert!(manager.has_drive_letter(Path::new(r"C:\path")));
    assert!(manager.has_drive_letter(Path::new(r"D:\another\path")));
    assert!(!manager.has_drive_letter(Path::new(r"\\server\share")));
    assert!(!manager.has_drive_letter(Path::new(r"relative\path")));
}

#[test]
fn test_path_normalization() {
    let manager = WindowsFileSystemManager::new();

    // Test forward slash conversion and case - drive letter should remain uppercase
    let normalized = manager.normalize_windows_path(Path::new("C:/Path/To/File"));
    assert_eq!(normalized, PathBuf::from(r"C:\path\to\file"));

    // Test UNC path preservation (server/share part)
    let unc_path = Path::new(r"\\Server\Share\SubFolder");
    let normalized = manager.normalize_windows_path(unc_path);
    assert_eq!(normalized, PathBuf::from(r"\\server\share\subfolder"));
}

#[test]
fn test_reserved_name_validation() {
    let manager = WindowsFileSystemManager::new();

    // Test reserved names
    assert!(manager.validate_windows_path(Path::new(r"C:\CON")).is_err());
    assert!(manager
        .validate_windows_path(Path::new(r"C:\PRN.txt"))
        .is_err());
    assert!(manager
        .validate_windows_path(Path::new(r"C:\COM1"))
        .is_err());
    assert!(manager
        .validate_windows_path(Path::new(r"C:\LPT1.dat"))
        .is_err());

    // Test valid names
    assert!(manager
        .validate_windows_path(Path::new(r"C:\CONSOLE"))
        .is_ok());
    assert!(manager
        .validate_windows_path(Path::new(r"C:\PRINTER.txt"))
        .is_ok());
}

#[test]
fn test_invalid_character_validation() {
    let manager = WindowsFileSystemManager::new();

    // Test invalid characters (excluding drive letters and UNC paths)
    assert!(manager
        .validate_windows_path(Path::new(r"C:\path\file<name"))
        .is_err());
    assert!(manager
        .validate_windows_path(Path::new(r"C:\path\file>name"))
        .is_err());
    assert!(manager
        .validate_windows_path(Path::new(r"C:\path\file|name"))
        .is_err());
    assert!(manager
        .validate_windows_path(Path::new(r"C:\path\file?name"))
        .is_err());
    assert!(manager
        .validate_windows_path(Path::new(r"C:\path\file*name"))
        .is_err());

    // Test valid characters
    assert!(manager
        .validate_windows_path(Path::new(r"C:\path\filename"))
        .is_ok());
    assert!(manager
        .validate_windows_path(Path::new(r"C:\path\file-name_123"))
        .is_ok());
}

#[test]
fn test_case_insensitive_comparison() {
    let manager = WindowsFileSystemManager::new();

    let path1 = Path::new(r"C:\Path\To\File.txt");
    let path2 = Path::new(r"c:\path\to\file.TXT");

    assert!(manager.paths_equal(path1, path2));
}

#[test]
fn test_extension_matching() {
    let manager = WindowsFileSystemManager::new();

    let path = Path::new(r"C:\path\file.MP4");
    let extensions = vec!["mp4".to_string(), "avi".to_string()];

    // Should match case-insensitively on Windows
    assert!(manager.matches_extension(path, &extensions));
}

#[test]
fn test_hidden_file_detection() {
    let manager = WindowsFileSystemManager::new();

    assert!(manager.is_hidden_windows(Path::new(r"C:\path\Thumbs.db")));
    assert!(manager.is_hidden_windows(Path::new(r"C:\path\desktop.ini")));
    assert!(manager.is_hidden_windows(Path::new(r"C:\path\.hidden")));
    assert!(!manager.is_hidden_windows(Path::new(r"C:\path\normal.txt")));
}

#[test]
fn test_valid_drive_letter_paths() {
    let manager = WindowsFileSystemManager::new();

    // Valid drive letter paths
    assert!(manager
        .validate_windows_path(Path::new(r"C:\Users\Welcome\Videos"))
        .is_ok());
    assert!(manager
        .validate_windows_path(Path::new(r"D:\Media\Movies"))
        .is_ok());
    assert!(manager.validate_windows_path(Path::new(r"Z:\")).is_ok());
    assert!(manager.validate_windows_path(Path::new(r"C:")).is_ok());
    assert!(manager
        .validate_windows_path(Path::new(r"C:/path/with/forward/slashes"))
        .is_ok());
}

#[test]
fn test_valid_unc_paths() {
    let manager = WindowsFileSystemManager::new();

    // Valid UNC paths
    assert!(manager
        .validate_windows_path(Path::new(r"\\server\share"))
        .is_ok());
    assert!(manager
        .validate_windows_path(Path::new(r"\\192.168.1.100\media"))
        .is_ok());
    assert!(manager
        .validate_windows_path(Path::new(r"\\server:8080\share"))
        .is_ok());
    assert!(manager
        .validate_windows_path(Path::new(r"\\server:443\share\subfolder"))
        .is_ok());
}

#[test]
fn test_invalid_colon_usage() {
    let manager = WindowsFileSystemManager::new();

    // Invalid colon usage
    assert!(manager
        .validate_windows_path(Path::new(r"C:\path\file:name"))
        .is_err());
    assert!(manager
        .validate_windows_path(Path::new(r"relative\path:name"))
        .is_err());
    assert!(manager
        .validate_windows_path(Path::new(r"\\server\share:invalid"))
        .is_err());
    assert!(manager
        .validate_windows_path(Path::new(r"C:D:\invalid"))
        .is_err());
    assert!(manager
        .validate_windows_path(Path::new(r"path:with:multiple:colons"))
        .is_err());
}

#[test]
fn test_colon_validation_details() {
    let manager = WindowsFileSystemManager::new();

    // Test drive letter colon validation
    assert!(manager.validate_drive_letter_colon_usage("C:\\path"));
    assert!(manager.validate_drive_letter_colon_usage("D:"));
    assert!(!manager.validate_drive_letter_colon_usage("C:\\path:invalid"));
    assert!(!manager.validate_drive_letter_colon_usage("C:D:\\invalid"));
    assert!(!manager.validate_drive_letter_colon_usage("path:invalid"));

    // Test UNC colon validation
    assert!(manager.validate_unc_colon_usage(r"\\server:8080\share"));
    assert!(manager.validate_unc_colon_usage(r"\\server\share"));
    assert!(manager.validate_unc_colon_usage(r"\\192.168.1.100:443\share\subfolder"));
    assert!(!manager.validate_unc_colon_usage(r"\\server\share:invalid"));
    assert!(!manager.validate_unc_colon_usage(r"\\server:8080\share:invalid"));
    assert!(!manager.validate_unc_colon_usage(r"not\unc\path:invalid"));

    // Test looks_like_drive_letter helper
    assert!(manager.looks_like_drive_letter("C:"));
    assert!(manager.looks_like_drive_letter("C:\\path"));
    assert!(manager.looks_like_drive_letter("D:/path"));
    assert!(!manager.looks_like_drive_letter("\\\\server"));
    assert!(!manager.looks_like_drive_letter("relative"));
    assert!(!manager.looks_like_drive_letter("1:invalid"));
}

#[test]
fn test_is_valid_colon_usage() {
    let manager = WindowsFileSystemManager::new();

    // Valid colon usage
    assert!(manager.is_valid_colon_usage(Path::new(r"C:\path")));
    assert!(manager.is_valid_colon_usage(Path::new(r"D:")));
    assert!(manager.is_valid_colon_usage(Path::new(r"\\server:8080\share")));
    assert!(manager.is_valid_colon_usage(Path::new(r"\\server\share")));
    assert!(manager.is_valid_colon_usage(Path::new(r"relative\path\without\colons")));

    // Invalid colon usage
    assert!(!manager.is_valid_colon_usage(Path::new(r"C:\path\file:name")));
    assert!(!manager.is_valid_colon_usage(Path::new(r"relative\path:name")));
    assert!(!manager.is_valid_colon_usage(Path::new(r"\\server\share:invalid")));
    assert!(!manager.is_valid_colon_usage(Path::new(r"C:D:\invalid")));
}

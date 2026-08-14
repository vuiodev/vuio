use super::*;
use crate::database::sqlite::SqliteDatabase;
use crate::database::MediaRepository;
use crate::platform::filesystem::BaseFileSystemManager;
#[cfg(target_os = "windows")]
use crate::platform::filesystem::WindowsPathNormalizer;
use futures_util::StreamExt;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_media_scanner_basic_functionality() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create database
    let db = Arc::new(SqliteDatabase::new(db_path).await.unwrap());
    db.initialize().await.unwrap();

    // Create scanner with base filesystem manager
    let filesystem_manager = Box::new(BaseFileSystemManager::new(true));
    let scanner = MediaScanner::with_filesystem_manager(filesystem_manager, db);

    // Test directory validation
    let invalid_path = Path::new("/nonexistent/directory");
    let result = scanner.scan_directory(invalid_path).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_media_scanner_path_normalization() {
    // Create temp directory and keep it alive
    let temp_dir = tempdir().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    // Use synchronous std::fs to ensure file is created before any async ops
    let db_path = temp_path.join("test.db");
    let test_file_path = temp_path.join("test.mp4");

    // Create test file synchronously to avoid race conditions
    std::fs::write(&test_file_path, b"fake video content").unwrap();

    // Verify directory and file exist before proceeding
    assert!(temp_path.exists(), "Temp directory should exist");
    assert!(test_file_path.exists(), "Test file should exist");

    // Create database
    let db = Arc::new(SqliteDatabase::new(db_path).await.unwrap());
    db.initialize().await.unwrap();

    // Create scanner with platform-appropriate path normalizer
    #[cfg(target_os = "windows")]
    let path_normalizer = Box::new(WindowsPathNormalizer::new());
    #[cfg(not(target_os = "windows"))]
    let path_normalizer = Box::new(crate::platform::filesystem::UnixPathNormalizer::new());

    // On Windows we want case-insensitivity (false), on Linux usually true but for this test we match the normalizer
    let case_sensitive = !cfg!(target_os = "windows");
    let filesystem_manager = Box::new(BaseFileSystemManager::with_normalizer(
        case_sensitive,
        path_normalizer,
    ));
    let scanner = MediaScanner::with_filesystem_manager(filesystem_manager, db.clone());

    // Verify directory still exists before scanning
    assert!(
        temp_path.exists(),
        "Temp directory should still exist before scan"
    );

    // Scan the directory
    let result = scanner.scan_directory(&temp_path).await.unwrap();

    // Verify that files were found and processed
    assert_eq!(result.new_files.len(), 1);
    let scanned_file = &result.new_files[0];

    // Verify that the path was normalized (should be canonical format)
    let expected_canonical = scanner
        .filesystem_manager()
        .get_canonical_path(&test_file_path)
        .unwrap();
    assert_eq!(scanned_file.path.to_string_lossy(), expected_canonical);

    // Verify the file was stored in the database with canonical path
    let stored_file = db.get_file_by_path(&scanned_file.path).await.unwrap();
    assert!(stored_file.is_some());
    let stored_file = stored_file.unwrap();
    assert_eq!(stored_file.path.to_string_lossy(), expected_canonical);

    // temp_dir dropped here, auto-cleanup
}

#[tokio::test]
async fn test_scan_result_operations() {
    let mut result1 = ScanResult::new();
    result1.total_scanned = 5;
    result1.new_files.push(MediaFile {
        id: Some(1),
        path: PathBuf::from("/test1.mp4"),
        filename: "test1.mp4".to_string(),
        size: 1024,
        modified: SystemTime::now(),
        mime_type: "video/mp4".to_string(),
        duration: None,
        title: None,
        artist: None,
        album: None,
        genre: None,
        track_number: None,
        year: None,
        album_artist: None,
        tags: Default::default(),
        stream: Default::default(),
        extra_tags: Vec::new(),
        tags_version: 0,
        subtitle_available: false,
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
    });

    let mut result2 = ScanResult::new();
    result2.total_scanned = 3;
    result2.updated_files.push(MediaFile {
        id: Some(2),
        path: PathBuf::from("/test2.mp4"),
        filename: "test2.mp4".to_string(),
        size: 2048,
        modified: SystemTime::now(),
        mime_type: "video/mp4".to_string(),
        duration: None,
        title: None,
        artist: None,
        album: None,
        genre: None,
        track_number: None,
        year: None,
        album_artist: None,
        tags: Default::default(),
        stream: Default::default(),
        extra_tags: Vec::new(),
        tags_version: 0,
        subtitle_available: false,
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
    });

    // Test merge
    result1.merge(result2);
    assert_eq!(result1.total_scanned, 8);
    assert_eq!(result1.new_files.len(), 1);
    assert_eq!(result1.updated_files.len(), 1);

    // Test summary
    let summary = result1.summary();
    assert!(summary.contains("8 files"));
    assert!(summary.contains("1 new"));
    assert!(summary.contains("1 updated"));
}

#[tokio::test]
async fn test_recursive_scan_optimization() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create database
    let db = Arc::new(SqliteDatabase::new(db_path).await.unwrap());
    db.initialize().await.unwrap();

    // Create scanner with base filesystem manager
    let filesystem_manager = Box::new(BaseFileSystemManager::new(true));
    let scanner = MediaScanner::with_filesystem_manager(filesystem_manager, db.clone());

    // Create a nested directory structure with test files
    let root_dir = temp_dir.path().join("media");
    let sub_dir1 = root_dir.join("videos");
    let sub_dir2 = root_dir.join("music");
    let sub_sub_dir = sub_dir1.join("movies");

    tokio::fs::create_dir_all(&sub_sub_dir).await.unwrap();
    tokio::fs::create_dir_all(&sub_dir2).await.unwrap();

    // Create test files in different directories
    tokio::fs::write(root_dir.join("root.mp4"), b"root video")
        .await
        .unwrap();
    tokio::fs::write(sub_dir1.join("video1.mp4"), b"video content")
        .await
        .unwrap();
    tokio::fs::write(sub_dir2.join("song1.mp3"), b"audio content")
        .await
        .unwrap();
    tokio::fs::write(sub_sub_dir.join("movie1.mkv"), b"movie content")
        .await
        .unwrap();

    // First scan to populate database
    let initial_result = scanner.scan_directory_recursive(&root_dir).await.unwrap();
    assert_eq!(initial_result.new_files.len(), 4);
    assert_eq!(initial_result.total_changes(), 4);

    // Verify all files were stored in database
    let mut all_files_stream = db.stream_all_media_files();
    let mut stored_files = Vec::new();
    while let Some(result) = all_files_stream.next().await {
        stored_files.push(result.unwrap());
    }
    assert_eq!(stored_files.len(), 4);

    // Second scan should find no changes (tests that optimization works correctly)
    let second_result = scanner.scan_directory_recursive(&root_dir).await.unwrap();
    assert_eq!(second_result.new_files.len(), 0);
    assert_eq!(second_result.updated_files.len(), 0);
    assert_eq!(second_result.unchanged, 4);
    assert_eq!(second_result.total_changes(), 0);
    // The point of the second scan: it still visited all four files, and opened
    // none of them. Reading a file means canonicalizing its path, probing for a
    // subtitle sidecar and, for audio, parsing the whole container — which used
    // to happen on every scan of an unchanged library, and there is one every
    // five minutes.
    assert_eq!(second_result.total_scanned, 4);
    assert_eq!(
        second_result.files_read, 0,
        "a rescan of an unchanged library must not open any file"
    );

    // Verify the optimization is working by checking that we can handle the recursive scan
    // without making individual database queries for each directory
    assert!(second_result.errors.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn direct_scan_resolves_only_symlinked_media_entries() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().unwrap();
    let media_root = temp.path().join("media");
    tokio::fs::create_dir(&media_root).await.unwrap();
    let target = temp.path().join("target.mp4");
    tokio::fs::write(&target, b"video").await.unwrap();
    let link = media_root.join("visible-name.mp4");
    symlink(&target, &link).unwrap();

    let database = Arc::new(
        SqliteDatabase::new(temp.path().join("symlink.db"))
            .await
            .unwrap(),
    );
    database.initialize().await.unwrap();
    let scanner =
        MediaScanner::with_filesystem_manager(Box::new(BaseFileSystemManager::new(true)), database);

    let result = scanner.scan_directory(&media_root).await.unwrap();
    assert_eq!(result.new_files.len(), 1);
    assert_eq!(result.new_files[0].filename, "visible-name.mp4");
    assert_eq!(result.new_files[0].path, target.canonicalize().unwrap());
}
#[test]
fn case_policy_compares_path_components_without_changing_boundaries() {
    assert!(path_components_equal(
        Path::new("/Media/Movies"),
        Path::new("/media/movies"),
        false
    ));
    assert!(!path_components_equal(
        Path::new("/Media/Movies"),
        Path::new("/media/movies"),
        true
    ));
    assert_eq!(swap_one_ascii_case("Movies"), Some("movies".to_string()));
}

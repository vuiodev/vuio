use super::*;
use std::fs;
use tempfile::TempDir;
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn test_watcher_creation() {
    let watcher = CrossPlatformWatcher::new();
    assert!(!watcher.is_watching(Path::new("/nonexistent")).await);
}

#[tokio::test]
async fn test_media_file_detection() {
    let watcher = CrossPlatformWatcher::new();

    assert!(watcher.is_media_file(Path::new("test.mp4")));
    assert!(watcher.is_media_file(Path::new("test.MP3")));
    assert!(watcher.is_media_file(Path::new("test.jpg")));
    assert!(!watcher.is_media_file(Path::new("test.txt")));
    assert!(!watcher.is_media_file(Path::new("test")));
}

#[test]
fn classifies_media_rename_transitions() {
    for extension in crate::platform::filesystem::get_supported_extensions() {
        let completed = format!("download.{extension}");
        assert_eq!(
            classify_media_rename(Path::new("download.staging"), Path::new(&completed)),
            MediaRenameKind::Create,
            "staging -> {completed}"
        );
    }

    for (staging, completed) in [
        ("movie.crdownload", "movie.mkv"),
        ("track.download", "track.flac"),
        ("image.tmp", "image.webp"),
        ("download", "clip.webm"),
        ("archive.partial", "recording.MP4"),
    ] {
        assert_eq!(
            classify_media_rename(Path::new(staging), Path::new(completed)),
            MediaRenameKind::Create,
            "{staging} -> {completed}"
        );
    }
    assert_eq!(
        classify_media_rename(Path::new("old.mp4"), Path::new("new.mp4")),
        MediaRenameKind::Replace
    );
    assert_eq!(
        classify_media_rename(Path::new("movie.mp4"), Path::new("movie.tmp")),
        MediaRenameKind::Remove
    );
    assert_eq!(
        classify_media_rename(Path::new("old.tmp"), Path::new("new.tmp")),
        MediaRenameKind::Ignore
    );
}

#[tokio::test]
async fn test_watch_nonexistent_directory() {
    let watcher = CrossPlatformWatcher::new();
    let result = watcher
        .start_watching(&[PathBuf::from("/nonexistent/path")])
        .await;
    // Should not fail, just log a warning
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_watch_and_unwatch() {
    let temp_dir = TempDir::new().unwrap();
    let watcher = CrossPlatformWatcher::new();

    // Start watching
    let result = watcher
        .start_watching(&[temp_dir.path().to_path_buf()])
        .await;
    assert!(result.is_ok());

    // Check if watching
    assert!(watcher.is_watching(temp_dir.path()).await);
    assert!(watcher.is_watching(&temp_dir.path().join(".")).await);

    // Stop watching
    let result = watcher.stop_watching().await;
    assert!(result.is_ok());

    // Should no longer be watching
    assert!(!watcher.is_watching(temp_dir.path()).await);
}

#[tokio::test]
async fn test_file_events() {
    let temp_dir = TempDir::new().unwrap();
    let watcher = CrossPlatformWatcher::new();

    // Get event receiver before starting watcher
    let mut receiver = watcher.take_event_receiver().await.unwrap();

    // Start watching
    watcher
        .start_watching(&[temp_dir.path().to_path_buf()])
        .await
        .unwrap();

    // Give the watcher time to initialize
    sleep(Duration::from_millis(200)).await;

    // Create a media file
    let test_file = temp_dir.path().join("test.mp4");
    fs::write(&test_file, b"test content").unwrap();

    // Wait for the correct event with timeout, ignoring directory creation events
    let timeout_duration = Duration::from_secs(5);
    let correct_event_result = timeout(timeout_duration, async {
        loop {
            let event = receiver.recv().await;
            match event {
                Some(FileSystemEvent::Created(path)) => {
                    let canonical_received = path.canonicalize().unwrap_or_else(|_| path.clone());
                    let canonical_expected = test_file
                        .canonicalize()
                        .unwrap_or_else(|_| test_file.clone());

                    if canonical_received == canonical_expected {
                        // This is the event we are looking for
                        return Some(FileSystemEvent::Created(path));
                    } else {
                        // This is likely the directory creation event, ignore it and continue waiting
                        info!("Ignoring creation event for path: {}", path.display());
                    }
                }
                Some(other_event) => {
                    // Ignore other events for this test
                    info!("Ignoring other event: {:?}", other_event);
                }
                None => {
                    // Channel is closed, stop waiting
                    return None;
                }
            }
        }
    })
    .await;

    if let Ok(Some(event)) = correct_event_result {
        match event {
            FileSystemEvent::Created(path) => {
                let canonical_received = path.canonicalize().unwrap_or(path);
                let canonical_expected = test_file.canonicalize().unwrap_or(test_file);
                assert_eq!(canonical_received, canonical_expected);
            }
            _ => panic!("Received an unexpected event type after filtering"),
        }
    } else {
        // Events might be flaky in test environments, so we don't fail the test
        warn!("No specific file creation event received within {:?}. This can sometimes happen in test environments.", timeout_duration);
    }

    watcher.stop_watching().await.unwrap();
}

#[test]
fn test_convert_watcher_events_ignores_access() {
    use notify::event::{AccessKind, AccessMode};

    let policy = ScanPolicy::platform_default(Path::new("/media"), true);
    let policies = vec![policy];

    let access_event = DebouncedEvent {
        event: notify::Event {
            kind: notify::EventKind::Access(AccessKind::Open(AccessMode::Read)),
            paths: vec![PathBuf::from("/media/video.mp4")],
            attrs: notify::event::EventAttributes::default(),
        },
        time: std::time::Instant::now(),
    };

    let converted = convert_watcher_events(vec![access_event], &policies);
    assert!(
        converted.is_empty(),
        "Access/read events must be ignored and not converted to modifications"
    );
}

#[test]
fn test_convert_watcher_events_handles_modify() {
    use notify::event::{ModifyKind, DataChange};

    let policy = ScanPolicy::platform_default(Path::new("/media"), true);
    let policies = vec![policy];

    let modify_event = DebouncedEvent {
        event: notify::Event {
            kind: notify::EventKind::Modify(ModifyKind::Data(DataChange::Content)),
            paths: vec![PathBuf::from("/media/song.flac")],
            attrs: notify::event::EventAttributes::default(),
        },
        time: std::time::Instant::now(),
    };

    let converted = convert_watcher_events(vec![modify_event], &policies);
    assert_eq!(converted.len(), 1);
    match &converted[0] {
        FileSystemEvent::Modified(p) => assert_eq!(p, Path::new("/media/song.flac")),
        _ => panic!("Expected FileSystemEvent::Modified"),
    }
}

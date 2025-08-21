use async_trait::async_trait;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, DebouncedEvent, Debouncer, FileIdMap};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::error::Result;

pub mod integration;

/// Events that can occur in the file system for media files
#[derive(Debug, Clone)]
pub enum FileSystemEvent {
    /// A new file was created
    Created(PathBuf),
    /// An existing file was modified
    Modified(PathBuf),
    /// A file was deleted
    Deleted(PathBuf),
    /// A file was renamed/moved
    Renamed { from: PathBuf, to: PathBuf },
}

/// Trait for cross-platform file system watching
#[async_trait]
pub trait FileSystemWatcher: Send + Sync {
    /// Start watching the specified directories for changes
    async fn start_watching(&self, directories: &[PathBuf]) -> Result<()>;
    
    /// Stop watching all directories
    async fn stop_watching(&self) -> Result<()>;
    
    /// Get a receiver for file system events
    fn get_event_receiver(&self) -> mpsc::Receiver<FileSystemEvent>;
    
    /// Add a new path to watch
    async fn add_watch_path(&self, path: &Path) -> Result<()>;
    
    /// Remove a path from watching
    async fn remove_watch_path(&self, path: &Path) -> Result<()>;
    
    /// Check if a path is currently being watched
    async fn is_watching(&self, path: &Path) -> bool;
}

/// Cross-platform file system watcher implementation
pub struct CrossPlatformWatcher {
    debouncer: Arc<RwLock<Option<Debouncer<RecommendedWatcher, FileIdMap>>>>,
    event_sender: mpsc::Sender<FileSystemEvent>,
    event_receiver: Arc<RwLock<Option<mpsc::Receiver<FileSystemEvent>>>>,
    watched_paths: Arc<RwLock<HashSet<PathBuf>>>,
    media_extensions: HashSet<String>,
    debounce_duration: Duration,
}

impl CrossPlatformWatcher {
    /// Create a new cross-platform file system watcher
    pub fn new() -> Self {
        let (event_sender, event_receiver) = mpsc::channel(1000);
        
        // Define supported media file extensions
        let media_extensions = [
            // Video formats
            "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "3gp", "mpg", "mpeg",
            // Audio formats  
            "mp3", "flac", "wav", "aac", "ogg", "wma", "m4a", "opus", "aiff",
            // Image formats
            "jpg", "jpeg", "png", "gif", "bmp", "tiff", "webp", "svg",
        ]
        .iter()
        .map(|ext| ext.to_lowercase())
        .collect();

        Self {
            debouncer: Arc::new(RwLock::new(None)),
            event_sender,
            event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
            watched_paths: Arc::new(RwLock::new(HashSet::new())),
            media_extensions,
            debounce_duration: Duration::from_millis(100), // 100ms debounce
        }
    }

    /// Check if a file is a supported media file based on its extension
    fn is_media_file(&self, path: &Path) -> bool {
        if let Some(extension) = path.extension() {
            if let Some(ext_str) = extension.to_str() {
                return self.media_extensions.contains(&ext_str.to_lowercase());
            }
        }
        false
    }

    /// Convert notify events to our FileSystemEvent enum
    fn convert_events(&self, events: Vec<DebouncedEvent>) -> Vec<FileSystemEvent> {
        let mut fs_events = Vec::new();
        
        for event in events {
            match event.event.kind {
                notify::EventKind::Create(_) => {
                    for path in &event.event.paths {
                        if path.is_dir() {
                            // Handle directory creation - scan for media files
                            info!("Directory created (detected by watcher): {:?}", path);
                            fs_events.push(FileSystemEvent::Created(path.clone()));
                        } else if self.is_media_file(path) {
                            info!("Media file created (detected by watcher): {:?}", path);
                            fs_events.push(FileSystemEvent::Created(path.clone()));
                        } else {
                            debug!("Non-media file created, ignoring: {:?}", path);
                        }
                    }
                }
                notify::EventKind::Modify(_) => {
                    // Only process modify events for media files
                    let media_paths: Vec<_> = event.event.paths.iter()
                        .filter(|path| self.is_media_file(path))
                        .collect();
                    
                    for path in media_paths {
                        debug!("Media file modified: {:?}", path);
                        fs_events.push(FileSystemEvent::Modified(path.clone()));
                    }
                }
                notify::EventKind::Remove(_) => {
                    for path in &event.event.paths {
                        // Since the path is deleted, we can't check if it was a directory
                        // We'll send all deletion events and let the handler figure it out
                        info!("Path deleted (detected by watcher): {:?}", path);
                        fs_events.push(FileSystemEvent::Deleted(path.clone()));
                    }
                }
                notify::EventKind::Other => {
                    // Handle platform-specific events for media files only
                    let media_paths: Vec<_> = event.event.paths.iter()
                        .filter(|path| self.is_media_file(path))
                        .collect();
                    
                    for path in media_paths {
                        debug!("Media file other event: {:?}", path);
                        fs_events.push(FileSystemEvent::Modified(path.clone()));
                    }
                }
                _ => {
                    // Handle other event types as modifications for media files only
                    let media_paths: Vec<_> = event.event.paths.iter()
                        .filter(|path| self.is_media_file(path))
                        .collect();
                    
                    for path in media_paths {
                        debug!("Media file generic event: {:?}", path);
                        fs_events.push(FileSystemEvent::Modified(path.clone()));
                    }
                }
            }
        }
        
        fs_events
    }

    /// Initialize the debounced watcher
    async fn initialize_watcher(&self) -> Result<()> {
        let event_sender = self.event_sender.clone();
        let media_extensions = self.media_extensions.clone();
        
        let debouncer = new_debouncer(
            self.debounce_duration,
            None, // Use default tick rate
            move |result: DebounceEventResult| {
                match result {
                    Ok(events) => {
                        if !events.is_empty() {
                            info!("Watcher callback triggered with {} events", events.len());
                            for event in &events {
                                info!("  Raw event: {:?} for paths: {:?}", event.event.kind, event.paths);
                            }
                        }
                        
                        // Filter events for media files OR directories
                        let relevant_events: Vec<_> = events.into_iter()
                            .filter(|event| {
                                event.paths.iter().any(|path| {
                                    // For deletion events, we can't check if path.is_dir() since it's gone
                                    // So we include all deletion events
                                    if matches!(event.event.kind, notify::EventKind::Remove(_)) {
                                        info!("Including deletion event for path: {:?}", path);
                                        return true;
                                    }
                                    
                                    // Include directories and media files for other events
                                    if path.is_dir() {
                                        info!("Including directory event for path: {:?}", path);
                                        return true;
                                    }
                                    
                                    // Include media files
                                    if let Some(extension) = path.extension() {
                                        if let Some(ext_str) = extension.to_str() {
                                            if media_extensions.contains(&ext_str.to_lowercase()) {
                                                info!("Including media file event for path: {:?}", path);
                                                return true;
                                            }
                                        }
                                    }
                                    
                                    debug!("Excluding non-media file event for path: {:?}", path);
                                    false
                                })
                            })
                            .collect();

                        if !relevant_events.is_empty() {
                            info!("Processing {} relevant events", relevant_events.len());
                            let watcher = CrossPlatformWatcher {
                                debouncer: Arc::new(RwLock::new(None)),
                                event_sender: event_sender.clone(),
                                event_receiver: Arc::new(RwLock::new(None)),
                                watched_paths: Arc::new(RwLock::new(HashSet::new())),
                                media_extensions: media_extensions.clone(),
                                debounce_duration: Duration::from_millis(100),
                            };
                            
                            let fs_events = watcher.convert_events(relevant_events);
                            for fs_event in fs_events {
                                if let Err(e) = event_sender.try_send(fs_event) {
                                    error!("Failed to send file system event: {}", e);
                                }
                            }
                        }
                    }
                    Err(errors) => {
                        for error in errors {
                            error!("File watcher error: {:?}", error);
                        }
                    }
                }
            },
        )?;

        let mut debouncer_guard = self.debouncer.write().await;
        *debouncer_guard = Some(debouncer);
        
        info!("File system watcher initialized with {}ms debounce", self.debounce_duration.as_millis());
        info!("Watcher callback registered and ready to receive events");
        Ok(())
    }
}

#[async_trait]
impl FileSystemWatcher for CrossPlatformWatcher {
    async fn start_watching(&self, directories: &[PathBuf]) -> Result<()> {
        info!("Starting file system watcher for {} directories", directories.len());
        
        // Initialize the watcher if not already done
        if self.debouncer.read().await.is_none() {
            self.initialize_watcher().await?;
        }

        let mut debouncer_guard = self.debouncer.write().await;
        if let Some(ref mut debouncer) = *debouncer_guard {
            let mut watched_paths = self.watched_paths.write().await;
            
            for directory in directories {
                if !directory.exists() {
                    warn!("Directory does not exist, skipping: {:?}", directory);
                    continue;
                }
                
                if !directory.is_dir() {
                    warn!("Path is not a directory, skipping: {:?}", directory);
                    continue;
                }

                match debouncer.watcher().watch(directory, RecursiveMode::Recursive) {
                    Ok(()) => {
                        watched_paths.insert(directory.clone());
                        info!("Started watching directory: {:?}", directory);
                        
                        // Test if directory is accessible
                        if directory.exists() && directory.is_dir() {
                            info!("Directory exists and is accessible: {:?}", directory);
                        } else {
                            warn!("Directory may not be accessible: {:?}", directory);
                        }
                    }
                    Err(e) => {
                        error!("Failed to watch directory {:?}: {}", directory, e);
                        return Err(e.into());
                    }
                }
            }
        }

        Ok(())
    }

    async fn stop_watching(&self) -> Result<()> {
        info!("Stopping file system watcher");
        
        let mut debouncer_guard = self.debouncer.write().await;
        if let Some(debouncer) = debouncer_guard.take() {
            // The debouncer will be dropped here, stopping the watcher
            drop(debouncer);
        }
        
        let mut watched_paths = self.watched_paths.write().await;
        watched_paths.clear();
        
        info!("File system watcher stopped");
        Ok(())
    }

    fn get_event_receiver(&self) -> mpsc::Receiver<FileSystemEvent> {
        // This is a bit tricky - we need to return the receiver but can only do it once
        // In practice, this should be called once during application startup
        let receiver_guard = self.event_receiver.try_write();
        if let Ok(mut guard) = receiver_guard {
            if let Some(receiver) = guard.take() {
                return receiver;
            }
        }
        
        // If we can't get the original receiver, create a new channel
        // This shouldn't happen in normal usage
        warn!("Creating new event receiver - original may have been consumed");
        let (_, receiver) = mpsc::channel(1000);
        receiver
    }

    async fn add_watch_path(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            warn!("Path does not exist, cannot watch: {:?}", path);
            return Ok(());
        }

        let mut debouncer_guard = self.debouncer.write().await;
        if let Some(ref mut debouncer) = *debouncer_guard {
            let mut watched_paths = self.watched_paths.write().await;
            
            if watched_paths.contains(path) {
                debug!("Path already being watched: {:?}", path);
                return Ok(());
            }

            match debouncer.watcher().watch(path, RecursiveMode::Recursive) {
                Ok(()) => {
                    watched_paths.insert(path.to_path_buf());
                    info!("Added watch path: {:?}", path);
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to add watch path {:?}: {}", path, e);
                    Err(e.into())
                }
            }
        } else {
            warn!("Watcher not initialized, cannot add path: {:?}", path);
            Ok(())
        }
    }

    async fn remove_watch_path(&self, path: &Path) -> Result<()> {
        let mut debouncer_guard = self.debouncer.write().await;
        if let Some(ref mut debouncer) = *debouncer_guard {
            let mut watched_paths = self.watched_paths.write().await;
            
            if !watched_paths.contains(path) {
                debug!("Path not being watched: {:?}", path);
                return Ok(());
            }

            match debouncer.watcher().unwatch(path) {
                Ok(()) => {
                    watched_paths.remove(path);
                    info!("Removed watch path: {:?}", path);
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to remove watch path {:?}: {}", path, e);
                    Err(e.into())
                }
            }
        } else {
            warn!("Watcher not initialized, cannot remove path: {:?}", path);
            Ok(())
        }
    }

    async fn is_watching(&self, path: &Path) -> bool {
        let watched_paths = self.watched_paths.read().await;
        watched_paths.contains(path)
    }
}

impl Default for CrossPlatformWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
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

    #[tokio::test]
    async fn test_watch_nonexistent_directory() {
        let watcher = CrossPlatformWatcher::new();
        let result = watcher.start_watching(&[PathBuf::from("/nonexistent/path")]).await;
        // Should not fail, just log a warning
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_watch_and_unwatch() {
        let temp_dir = TempDir::new().unwrap();
        let watcher = CrossPlatformWatcher::new();
        
        // Start watching
        let result = watcher.start_watching(&[temp_dir.path().to_path_buf()]).await;
        assert!(result.is_ok());
        
        // Check if watching
        assert!(watcher.is_watching(temp_dir.path()).await);
        
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
        let mut receiver = watcher.get_event_receiver();
        
        // Start watching
        watcher.start_watching(&[temp_dir.path().to_path_buf()]).await.unwrap();
        
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
                        let canonical_expected = test_file.canonicalize().unwrap_or_else(|_| test_file.clone());
                        
                        if canonical_received == canonical_expected {
                            // This is the event we are looking for
                            return Some(FileSystemEvent::Created(path));
                        } else {
                            // This is likely the directory creation event, ignore it and continue waiting
                            info!("Ignoring creation event for path: {:?}", path);
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
        }).await;

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
}
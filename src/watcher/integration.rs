use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::database::{DatabaseManager, MediaFile};
use crate::error::Result;
use crate::media;
use crate::watcher::{FileSystemEvent, FileSystemWatcher};

/// Service that integrates file system watching with database updates
pub struct WatcherDatabaseIntegration<D, W>
where
    D: DatabaseManager,
    W: FileSystemWatcher,
{
    database: Arc<D>,
    watcher: Arc<W>,
    event_receiver: Option<mpsc::Receiver<FileSystemEvent>>,
    processing_queue: Arc<RwLock<HashMap<PathBuf, QueuedOperation>>>,
    batch_interval: Duration,
    is_running: Arc<RwLock<bool>>,
}

/// Operations that can be queued for batch processing
#[derive(Debug, Clone)]
enum QueuedOperation {
    Add(PathBuf),
    Update(PathBuf),
    Remove(PathBuf),
    Move { from: PathBuf, to: PathBuf },
}

impl<D, W> WatcherDatabaseIntegration<D, W>
where
    D: DatabaseManager + 'static,
    W: FileSystemWatcher + 'static,
{
    /// Create a new watcher-database integration service
    pub fn new(database: Arc<D>, watcher: Arc<W>) -> Self {
        Self {
            database,
            watcher,
            event_receiver: None,
            processing_queue: Arc::new(RwLock::new(HashMap::new())),
            batch_interval: Duration::from_millis(1000), // Process batches every second
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the integration service
    pub async fn start(&mut self, monitored_directories: &[PathBuf]) -> Result<()> {
        info!("Starting watcher-database integration service");

        // Start watching directories
        self.watcher.start_watching(monitored_directories).await?;

        // Get event receiver from watcher
        self.event_receiver = Some(self.watcher.get_event_receiver());

        // Set running flag
        {
            let mut running = self.is_running.write().await;
            *running = true;
        }

        // Start event processing task
        let database = Arc::clone(&self.database);
        let processing_queue = Arc::clone(&self.processing_queue);
        let is_running = Arc::clone(&self.is_running);
        let batch_interval = self.batch_interval;

        if let Some(receiver) = self.event_receiver.take() {
            let processing_queue_events = Arc::clone(&processing_queue);
            tokio::spawn(async move {
                Self::process_events(receiver, processing_queue_events).await;
            });
        }

        // Start batch processing task
        tokio::spawn(async move {
            Self::process_batches(database, processing_queue, is_running, batch_interval).await;
        });

        info!("Watcher-database integration service started");
        Ok(())
    }

    /// Stop the integration service
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping watcher-database integration service");

        // Set running flag to false
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }

        // Stop the file watcher
        self.watcher.stop_watching().await?;

        // Process any remaining queued operations
        self.flush_queue().await?;

        info!("Watcher-database integration service stopped");
        Ok(())
    }

    /// Process file system events and queue database operations
    async fn process_events(
        mut receiver: mpsc::Receiver<FileSystemEvent>,
        processing_queue: Arc<RwLock<HashMap<PathBuf, QueuedOperation>>>,
    ) {
        while let Some(event) = receiver.recv().await {
            debug!("Processing file system event: {:?}", event);

            let operation = match event {
                FileSystemEvent::Created(path) => {
                    info!("Media file created: {:?}", path);
                    Some(QueuedOperation::Add(path))
                }
                FileSystemEvent::Modified(path) => {
                    info!("Media file modified: {:?}", path);
                    Some(QueuedOperation::Update(path))
                }
                FileSystemEvent::Deleted(path) => {
                    info!("Media file deleted: {:?}", path);
                    Some(QueuedOperation::Remove(path))
                }
                FileSystemEvent::Renamed { from, to } => {
                    info!("Media file renamed: {:?} -> {:?}", from, to);
                    Some(QueuedOperation::Move { from, to })
                }
            };

            if let Some(op) = operation {
                let mut queue = processing_queue.write().await;
                let key = match &op {
                    QueuedOperation::Add(path) => path.clone(),
                    QueuedOperation::Update(path) => path.clone(),
                    QueuedOperation::Remove(path) => path.clone(),
                    QueuedOperation::Move { to, .. } => to.clone(),
                };
                queue.insert(key, op);
            }
        }
    }

    /// Process queued operations in batches
    async fn process_batches(
        database: Arc<D>,
        processing_queue: Arc<RwLock<HashMap<PathBuf, QueuedOperation>>>,
        is_running: Arc<RwLock<bool>>,
        batch_interval: Duration,
    ) {
        let mut interval = interval(batch_interval);

        loop {
            interval.tick().await;

            // Check if we should continue running
            {
                let running = is_running.read().await;
                if !*running {
                    break;
                }
            }

            // Get and clear the current queue
            let operations = {
                let mut queue = processing_queue.write().await;
                let ops: Vec<_> = queue.drain().collect();
                ops
            };

            if operations.is_empty() {
                continue;
            }

            debug!("Processing batch of {} operations", operations.len());

            // Process each operation
            for (_, operation) in operations {
                if let Err(e) = Self::process_operation(&database, operation).await {
                    error!("Failed to process database operation: {}", e);
                }
            }
        }
    }

    /// Process a single database operation
    async fn process_operation(database: &Arc<D>, operation: QueuedOperation) -> Result<()> {
        match operation {
            QueuedOperation::Add(path) => {
                Self::handle_file_added(database, &path).await
            }
            QueuedOperation::Update(path) => {
                Self::handle_file_updated(database, &path).await
            }
            QueuedOperation::Remove(path) => {
                Self::handle_file_removed(database, &path).await
            }
            QueuedOperation::Move { from, to } => {
                Self::handle_file_moved(database, &from, &to).await
            }
        }
    }

    /// Handle a new file or directory being added
    async fn handle_file_added(database: &Arc<D>, path: &Path) -> Result<()> {
        // Check if path still exists (might have been deleted quickly)
        if !path.exists() {
            debug!("Path no longer exists, skipping add: {:?}", path);
            return Ok(());
        }

        if path.is_dir() {
            // Handle directory creation by scanning for media files
            info!("New directory detected, scanning for media files: {:?}", path);
            match Self::scan_directory_recursive(path).await {
                Ok(media_files) => {
                    info!("Found {} media files in new directory: {:?}", media_files.len(), path);
                    
                    // Add each media file found in the new directory
                    for media_file_path in media_files {
                        if let Err(e) = Box::pin(Self::handle_file_added(database, &media_file_path)).await {
                            error!("Failed to add media file from new directory {:?}: {}", media_file_path, e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to scan new directory {:?}: {}", path, e);
                }
            }
            return Ok(());
        }

        // Handle regular file addition
        if !Self::is_media_file(path) {
            debug!("Not a media file, skipping: {:?}", path);
            return Ok(());
        }

        // Check if file is already in database
        if let Ok(Some(_)) = database.get_file_by_path(path).await {
            debug!("File already in database, updating instead: {:?}", path);
            return Box::pin(Self::handle_file_updated(database, path)).await;
        }

        // Create MediaFile from path
        match Self::create_media_file_from_path(path).await {
            Ok(media_file) => {
                match database.store_media_file(&media_file).await {
                    Ok(id) => {
                        info!("Added media file to database: {:?} (ID: {})", path, id);
                    }
                    Err(e) => {
                        error!("Failed to store media file {:?}: {}", path, e);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to create MediaFile from path {:?}: {}", path, e);
            }
        }

        Ok(())
    }

    /// Handle a file being updated
    async fn handle_file_updated(database: &Arc<D>, path: &Path) -> Result<()> {
        // Check if file still exists
        if !path.exists() {
            debug!("File no longer exists, removing from database: {:?}", path);
            return Self::handle_file_removed(database, path).await;
        }

        // Get existing file from database
        match database.get_file_by_path(path).await? {
            Some(mut existing_file) => {
                // Update file metadata
                match Self::update_media_file_metadata(&mut existing_file).await {
                    Ok(()) => {
                        match database.update_media_file(&existing_file).await {
                            Ok(()) => {
                                info!("Updated media file in database: {:?}", path);
                            }
                            Err(e) => {
                                error!("Failed to update media file {:?}: {}", path, e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to update metadata for {:?}: {}", path, e);
                    }
                }
            }
            None => {
                // File not in database, add it
                debug!("File not in database, adding: {:?}", path);
                Box::pin(Self::handle_file_added(database, path)).await?;
            }
        }

        Ok(())
    }

    /// Handle a file or directory being removed
    async fn handle_file_removed(database: &Arc<D>, path: &Path) -> Result<()> {
        // For directory removal, we need to remove all media files that were in that directory
        // Since the directory no longer exists, we can't check if it was a directory,
        // so we'll try to remove it as both a file and check for files in that path prefix
        
        // First, try to remove as a single file
        let mut removed_any = false;
        match database.remove_media_file(path).await {
            Ok(removed) => {
                if removed {
                    info!("Removed media file from database: {:?}", path);
                    removed_any = true;
                }
            }
            Err(e) => {
                debug!("Failed to remove as single file {:?}: {}", path, e);
            }
        }

        // Also check if this was a directory by looking for files with this path as prefix
        // This handles the case where a directory was deleted
        match database.get_all_media_files().await {
            Ok(all_files) => {
                let files_in_deleted_path: Vec<_> = all_files
                    .iter()
                    .filter(|file| file.path.starts_with(path))
                    .collect();

                if !files_in_deleted_path.is_empty() {
                    info!("Removing {} media files from deleted directory: {:?}", files_in_deleted_path.len(), path);
                    
                    for file in files_in_deleted_path {
                        match database.remove_media_file(&file.path).await {
                            Ok(removed) => {
                                if removed {
                                    debug!("Removed media file from deleted directory: {:?}", file.path);
                                    removed_any = true;
                                }
                            }
                            Err(e) => {
                                error!("Failed to remove media file from deleted directory {:?}: {}", file.path, e);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to get all media files for directory cleanup: {}", e);
            }
        }

        if !removed_any {
            debug!("No files were removed for path: {:?}", path);
        }

        Ok(())
    }

    /// Handle a file being moved/renamed
    async fn handle_file_moved(database: &Arc<D>, from: &Path, to: &Path) -> Result<()> {
        // Check if destination file exists
        if !to.exists() {
            debug!("Destination file doesn't exist, treating as deletion: {:?}", from);
            return Self::handle_file_removed(database, from).await;
        }

        // Get existing file from database
        match database.get_file_by_path(from).await? {
            Some(mut existing_file) => {
                // Update the path and metadata
                existing_file.path = to.to_path_buf();
                existing_file.filename = to
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                existing_file.updated_at = SystemTime::now();

                // Update file metadata
                if let Err(e) = Self::update_media_file_metadata(&mut existing_file).await {
                    warn!("Failed to update metadata for moved file {:?}: {}", to, e);
                }

                // Remove old record and add new one (SQLite doesn't support path updates easily)
                if let Err(e) = database.remove_media_file(from).await {
                    error!("Failed to remove old media file record {:?}: {}", from, e);
                }

                match database.store_media_file(&existing_file).await {
                    Ok(id) => {
                        info!("Updated media file path in database: {:?} -> {:?} (ID: {})", from, to, id);
                    }
                    Err(e) => {
                        error!("Failed to store moved media file {:?}: {}", to, e);
                        // Try to re-add the original file if the move failed
                        let _ = database.store_media_file(&existing_file).await;
                    }
                }
            }
            None => {
                // File wasn't in database, treat as new file
                debug!("Moved file wasn't in database, adding as new: {:?}", to);
                Self::handle_file_added(database, to).await?;
            }
        }

        Ok(())
    }

    /// Create a MediaFile from a file path
    async fn create_media_file_from_path(path: &Path) -> Result<MediaFile> {
        let metadata = tokio::fs::metadata(path).await?;
        let size = metadata.len();
        let modified = metadata.modified().unwrap_or(SystemTime::now());

        // Get MIME type
        let mime_type = media::get_mime_type(path);

        let mut media_file = MediaFile::new(path.to_path_buf(), size, mime_type);
        media_file.modified = modified;

        // Try to extract additional metadata (title, artist, etc.)
        if let Err(e) = Self::extract_media_metadata(&mut media_file).await {
            debug!("Failed to extract metadata for {:?}: {}", path, e);
        }

        Ok(media_file)
    }

    /// Update metadata for an existing MediaFile
    async fn update_media_file_metadata(media_file: &mut MediaFile) -> Result<()> {
        let metadata = tokio::fs::metadata(&media_file.path).await?;
        media_file.size = metadata.len();
        media_file.modified = metadata.modified().unwrap_or(SystemTime::now());
        media_file.updated_at = SystemTime::now();

        // Try to extract additional metadata
        if let Err(e) = Self::extract_media_metadata(media_file).await {
            debug!("Failed to extract metadata for {:?}: {}", media_file.path, e);
        }

        Ok(())
    }

    /// Extract media metadata (title, artist, duration, etc.)
    async fn extract_media_metadata(media_file: &mut MediaFile) -> Result<()> {
        // For now, this is a placeholder. In a real implementation, you would use
        // libraries like `ffprobe`, `taglib`, or similar to extract metadata
        
        // Extract basic info from filename
        if let Some(stem) = media_file.path.file_stem() {
            let stem_str = stem.to_string_lossy();
            
            // Simple heuristic: if filename contains " - ", split into artist and title
            if let Some(dash_pos) = stem_str.find(" - ") {
                let (artist, title) = stem_str.split_at(dash_pos);
                media_file.artist = Some(artist.trim().to_string());
                media_file.title = Some(title[3..].trim().to_string()); // Skip " - "
            } else {
                media_file.title = Some(stem_str.to_string());
            }
        }

        // For video/audio files, you could extract duration here
        // This would require additional dependencies like ffprobe-rs or similar
        
        Ok(())
    }

    /// Flush any remaining operations in the queue
    async fn flush_queue(&self) -> Result<()> {
        let operations = {
            let mut queue = self.processing_queue.write().await;
            let ops: Vec<_> = queue.drain().collect();
            ops
        };

        if !operations.is_empty() {
            info!("Flushing {} remaining operations", operations.len());
            
            for (_, operation) in operations {
                if let Err(e) = Self::process_operation(&self.database, operation).await {
                    error!("Failed to process queued operation during flush: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Perform an initial scan of directories and sync with database
    pub async fn initial_sync(&self, directories: &[PathBuf]) -> Result<()> {
        info!("Starting initial sync of {} directories", directories.len());

        for directory in directories {
            if !directory.exists() || !directory.is_dir() {
                warn!("Skipping non-existent or non-directory path: {:?}", directory);
                continue;
            }

            info!("Scanning directory: {:?}", directory);
            
            match Self::scan_directory_recursive(directory).await {
                Ok(found_files) => {
                    info!("Found {} media files in {:?}", found_files.len(), directory);
                    
                    // Get existing files from database for this directory
                    let existing_files = self.database.get_files_in_directory(directory).await?;
                    let existing_paths: std::collections::HashSet<_> = existing_files
                        .iter()
                        .map(|f| &f.path)
                        .collect();

                    // Add new files
                    for file_path in &found_files {
                        if !existing_paths.contains(file_path) {
                            if let Err(e) = Self::handle_file_added(&self.database, file_path).await {
                                error!("Failed to add file during initial sync {:?}: {}", file_path, e);
                            }
                        }
                    }

                    // Remove files that no longer exist
                    let found_paths: std::collections::HashSet<_> = found_files.iter().collect();
                    for existing_file in existing_files {
                        if !found_paths.contains(&existing_file.path) {
                            if let Err(e) = Self::handle_file_removed(&self.database, &existing_file.path).await {
                                error!("Failed to remove file during initial sync {:?}: {}", existing_file.path, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to scan directory {:?}: {}", directory, e);
                }
            }
        }

        info!("Initial sync completed");
        Ok(())
    }

    /// Recursively scan a directory for media files
    async fn scan_directory_recursive(directory: &Path) -> Result<Vec<PathBuf>> {
        let mut media_files = Vec::new();
        let mut entries = tokio::fs::read_dir(directory).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if path.is_dir() {
                // Recursively scan subdirectories
                match Box::pin(Self::scan_directory_recursive(&path)).await {
                    Ok(mut sub_files) => {
                        media_files.append(&mut sub_files);
                    }
                    Err(e) => {
                        warn!("Failed to scan subdirectory {:?}: {}", path, e);
                    }
                }
            } else if Self::is_media_file(&path) {
                media_files.push(path);
            }
        }

        Ok(media_files)
    }

    /// Check if a file is a supported media file
    fn is_media_file(path: &Path) -> bool {
        if let Some(extension) = path.extension() {
            if let Some(ext_str) = extension.to_str() {
                let ext_lower = ext_str.to_lowercase();
                return matches!(
                    ext_lower.as_str(),
                    "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "3gp" | "mpg" | "mpeg" |
                    "mp3" | "flac" | "wav" | "aac" | "ogg" | "wma" | "m4a" | "opus" | "aiff" |
                    "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "webp" | "svg"
                );
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::database::MediaFile;
    use crate::watcher::CrossPlatformWatcher;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::RwLock;

    // Mock database for testing
    struct MockDatabase {
        files: Arc<RwLock<HashMap<PathBuf, MediaFile>>>,
    }

    impl MockDatabase {
        fn new() -> Self {
            Self {
                files: Arc::new(RwLock::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl DatabaseManager for MockDatabase {
        async fn initialize(&self) -> anyhow::Result<()> {
            Ok(())
        }

        async fn store_media_file(&self, file: &MediaFile) -> anyhow::Result<i64> {
            let mut files = self.files.write().await;
            files.insert(file.path.clone(), file.clone());
            Ok(1)
        }

        async fn get_all_media_files(&self) -> anyhow::Result<Vec<MediaFile>> {
            let files = self.files.read().await;
            Ok(files.values().cloned().collect())
        }

        async fn remove_media_file(&self, path: &Path) -> anyhow::Result<bool> {
            let mut files = self.files.write().await;
            Ok(files.remove(path).is_some())
        }

        async fn update_media_file(&self, file: &MediaFile) -> anyhow::Result<()> {
            let mut files = self.files.write().await;
            files.insert(file.path.clone(), file.clone());
            Ok(())
        }

        async fn get_files_in_directory(&self, _dir: &Path) -> anyhow::Result<Vec<MediaFile>> {
            let files = self.files.read().await;
            Ok(files.values().cloned().collect())
        }

        async fn cleanup_missing_files(&self, _existing_paths: &[PathBuf]) -> anyhow::Result<usize> {
            Ok(0)
        }

        async fn get_file_by_path(&self, path: &Path) -> anyhow::Result<Option<MediaFile>> {
            let files = self.files.read().await;
            Ok(files.get(path).cloned())
        }

        async fn get_stats(&self) -> anyhow::Result<crate::database::DatabaseStats> {
            let files = self.files.read().await;
            Ok(crate::database::DatabaseStats {
                total_files: files.len(),
                total_size: files.values().map(|f| f.size).sum(),
                database_size: 0,
            })
        }

        async fn check_and_repair(&self) -> anyhow::Result<crate::database::DatabaseHealth> {
            Ok(crate::database::DatabaseHealth {
                is_healthy: true,
                corruption_detected: false,
                integrity_check_passed: true,
                issues: vec![],
                repair_attempted: false,
                repair_successful: false,
            })
        }

        async fn create_backup(&self, _backup_path: &Path) -> anyhow::Result<()> {
            Ok(())
        }

        async fn restore_from_backup(&self, _backup_path: &Path) -> anyhow::Result<()> {
            Ok(())
        }

        async fn vacuum(&self) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_integration_creation() {
        let database = Arc::new(MockDatabase::new());
        let watcher = Arc::new(CrossPlatformWatcher::new());
        let integration = WatcherDatabaseIntegration::new(database, watcher);
        
        // Just test that we can create the integration
        assert!(!*integration.is_running.read().await);
    }

    #[tokio::test]
    async fn test_media_file_detection() {
        assert!(WatcherDatabaseIntegration::<MockDatabase, CrossPlatformWatcher>::is_media_file(Path::new("test.mp4")));
        assert!(WatcherDatabaseIntegration::<MockDatabase, CrossPlatformWatcher>::is_media_file(Path::new("test.MP3")));
        assert!(!WatcherDatabaseIntegration::<MockDatabase, CrossPlatformWatcher>::is_media_file(Path::new("test.txt")));
    }

    #[tokio::test]
    async fn test_initial_sync() {
        let temp_dir = TempDir::new().unwrap();
        let database = Arc::new(MockDatabase::new());
        let watcher = Arc::new(CrossPlatformWatcher::new());
        let integration = WatcherDatabaseIntegration::new(database.clone(), watcher);

        // Create a test media file
        let test_file = temp_dir.path().join("test.mp4");
        tokio::fs::write(&test_file, b"test content").await.unwrap();

        // Run initial sync
        let result = integration.initial_sync(&[temp_dir.path().to_path_buf()]).await;
        assert!(result.is_ok());

        // Check that file was added to database
        let files = database.get_all_media_files().await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, test_file);
    }
}
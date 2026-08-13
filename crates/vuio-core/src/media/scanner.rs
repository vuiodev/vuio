use super::policy::TraversalReport;
use super::*;

/// Media scanner that uses the file system manager and database for efficient scanning.
pub struct MediaScanner<D: DatabaseManager = ActiveDatabase> {
    filesystem_manager: Box<dyn FileSystemManager>,
    database_manager: Arc<D>,
}

impl<D: DatabaseManager> MediaScanner<D> {
    fn fingerprint(file: &MediaFile) -> FileFingerprint {
        FileFingerprint {
            id: file.id.unwrap_or_default(),
            path: file.path.clone(),
            size: file.size,
            modified: file.modified,
            created_at: file.created_at,
            tags_version: file.tags_version,
        }
    }

    /// Create a new media scanner with database manager
    pub fn with_database(database_manager: Arc<D>) -> Self {
        Self {
            filesystem_manager: create_platform_filesystem_manager(),
            database_manager,
        }
    }


    /// Create a media scanner with a custom file system manager (for testing)
    pub fn with_filesystem_manager(
        filesystem_manager: Box<dyn FileSystemManager>,
        database_manager: Arc<D>,
    ) -> Self {
        Self {
            filesystem_manager,
            database_manager,
        }
    }

    /// Perform a full scan of a directory, updating the database with new/changed files
    pub async fn scan_directory(&self, directory: &Path) -> Result<ScanResult> {
        let policy = ScanPolicy::platform_default(directory, false);
        self.scan_directory_with_policy(&policy).await
    }

    pub async fn scan_directory_with_policy(&self, policy: &ScanPolicy) -> Result<ScanResult> {
        let directory = &policy.root;
        let canonical_dir = match self.filesystem_manager.get_canonical_path(directory) {
            Ok(canonical) => PathBuf::from(canonical),
            Err(error) => {
                warn!("Failed to canonicalize {}: {error}", directory.display());
                self.filesystem_manager.normalize_path(directory)
            }
        };
        self.filesystem_manager.validate_path(&canonical_dir)?;
        let mut effective_policy = policy.clone();
        effective_policy.root = canonical_dir.clone();
        let mut entries = tokio::fs::read_dir(&canonical_dir).await?;
        let existing_files = self
            .database_manager
            .get_files_in_directory(&canonical_dir)
            .await?;
        let mut current_files = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if effective_policy.allows_media(&path) && tokio::fs::metadata(&path).await?.is_file() {
                current_files.push(self.create_media_file_from_path(&path).await?);
            }
        }
        if current_files.is_empty() && !existing_files.is_empty() {
            let mut result = ScanResult::new();
            result.complete = false;
            result.total_scanned = 0;
            result.errors.push(ScanError {
                path: canonical_dir,
                error: "previously populated root is unexpectedly empty; destructive reconciliation deferred"
                    .to_owned(),
            });
            result
                .unchanged_files
                .extend(existing_files.iter().map(Self::fingerprint));
            return Ok(result);
        }
        self.perform_incremental_update(&canonical_dir, existing_files, current_files)
            .await
    }

    /// Perform an incremental update by comparing database state with file system state
    /// **OPTIMIZED FOR DATABASE WITH BULK OPERATIONS**
    async fn perform_incremental_update(
        &self,
        _directory: &Path,
        existing_files: Vec<MediaFile>,
        current_files: Vec<MediaFile>,
    ) -> Result<ScanResult> {
        let mut result = ScanResult::new();

        let existing_by_normalized: std::collections::HashMap<PathBuf, MediaFile> = existing_files
            .into_iter()
            .map(|file| (file.path.clone(), file))
            .collect();
        let current_normalized: std::collections::HashMap<PathBuf, MediaFile> = current_files
            .into_iter()
            .map(|file| (file.path.clone(), file))
            .collect();

        let current_paths: HashSet<PathBuf> = current_normalized.keys().cloned().collect();

        // **BULK OPERATIONS - Collect files for batch processing**
        let mut files_to_insert = Vec::new();
        let mut files_to_update = Vec::new();
        let mut files_to_remove = Vec::new();

        // Process current files - collect new ones and changed ones for bulk operations
        for (normalized_current_path, current_file) in &current_normalized {
            let existing_file = existing_by_normalized.get(normalized_current_path);

            match existing_file {
                Some(existing_file) => {
                    // File exists in database, check if it needs updating
                    if self.file_needs_update(existing_file, current_file) {
                        tracing::debug!(
                            "File needs update: {} (modified: {:?} vs {:?}, size: {} vs {})",
                            existing_file.path.display(),
                            existing_file.modified,
                            current_file.modified,
                            existing_file.size,
                            current_file.size
                        );

                        // Use the canonical path from current_file (already normalized above)
                        let mut updated_file = current_file.clone();
                        updated_file.id = existing_file.id; // Preserve database ID
                        updated_file.created_at = existing_file.created_at; // Preserve creation time
                        updated_file.updated_at = SystemTime::now();

                        files_to_update.push(updated_file);
                    } else {
                        result
                            .unchanged_files
                            .push(Self::fingerprint(existing_file));
                    }
                }
                None => {
                    // New file, add to bulk insert list with canonical path format
                    // The current_file already has the canonical path from normalization above
                    files_to_insert.push(current_file.clone());
                }
            }
        }

        // Find files that were removed from the file system
        for (normalized_existing_path, existing_file) in existing_by_normalized {
            if !current_paths.contains(&normalized_existing_path) {
                // File was removed from file system, add to bulk removal list
                files_to_remove.push(existing_file.path.clone());
                result.removed_files.push(Self::fingerprint(&existing_file));
            }
        }

        // **EXECUTE BULK OPERATIONS**

        // Bulk insert new files
        if !files_to_insert.is_empty() {
            tracing::info!(
                "Bulk inserting {} new files using database",
                files_to_insert.len()
            );
            for file in &files_to_insert {
                tracing::debug!(
                    "Inserting file: path='{}', mime_type='{}', size={}",
                    file.path.display(),
                    file.mime_type,
                    file.size
                );
            }
            let insert_ids = self
                .database_manager
                .bulk_store_canonical_media_files(&files_to_insert)
                .await?;

            // Update result with inserted files and their IDs
            for (i, mut file) in files_to_insert.into_iter().enumerate() {
                if let Some(id) = insert_ids.get(i) {
                    file.id = Some(*id);
                }
                result.new_files.push(file);
            }
        }

        // Bulk update changed files
        if !files_to_update.is_empty() {
            tracing::info!(
                "Bulk updating {} changed files using database",
                files_to_update.len()
            );
            self.database_manager
                .bulk_update_canonical_media_files(&files_to_update)
                .await?;
            result.updated_files.extend(files_to_update);
        }

        // Bulk remove deleted files
        if !files_to_remove.is_empty() {
            tracing::info!(
                "Bulk removing {} deleted files using database",
                files_to_remove.len()
            );
            let removed_count = self
                .database_manager
                .bulk_remove_media_files(&files_to_remove)
                .await?;
            tracing::debug!(
                "Successfully removed {} out of {} requested files",
                removed_count,
                files_to_remove.len()
            );
        }

        result.total_scanned = current_paths.len();

        // Log bulk operation summary
        tracing::info!(
            "bulk operations completed: {} inserted, {} updated, {} removed, {} unchanged",
            result.new_files.len(),
            result.updated_files.len(),
            result.removed_files.len(),
            result.unchanged_files.len()
        );

        Ok(result)
    }

    /// Check if a file needs to be updated in the database
    fn file_needs_update(&self, existing: &MediaFile, current: &MediaFile) -> bool {
        // Compare file sizes first (most reliable)
        if existing.size != current.size {
            return true;
        }

        // Compare MIME type and filename
        if existing.mime_type != current.mime_type || existing.filename != current.filename {
            return true;
        }

        // A record written by an older tag reader is stale even though its file
        // is not. Non-recursive roots come through here rather than through
        // `fingerprint_needs_update`, and without this they would never pick up
        // an improved extractor.
        if existing.tags_version < current.tags_version {
            return true;
        }

        // Compare modification times with tolerance for Windows timestamp precision issues
        // Windows can have different precision depending on filesystem and access method
        let time_diff = if existing.modified > current.modified {
            existing.modified.duration_since(current.modified)
        } else {
            current.modified.duration_since(existing.modified)
        };

        // Allow up to 10 seconds difference to account for timestamp precision issues
        match time_diff {
            Ok(diff) => diff.as_secs() > 10,
            Err(_) => true, // If we can't calculate the difference, assume it needs updating
        }
    }

    fn fingerprint_needs_update(&self, existing: &FileFingerprint, current: &MediaFile) -> bool {
        if existing.size != current.size {
            return true;
        }
        // A record written by an older tag reader is stale even though its file
        // is not. The file has already been parsed by the time we get here, so
        // rewriting it costs one database write and no extra I/O.
        if existing.tags_version < current.tags_version {
            return true;
        }
        let time_diff = if existing.modified > current.modified {
            existing.modified.duration_since(current.modified)
        } else {
            current.modified.duration_since(existing.modified)
        };
        time_diff.map_or(true, |difference| difference.as_secs() > 10)
    }


    /// Perform a recursive scan of a directory using parallel multi-threaded traversal (jwalk)
    ///
    /// This method uses jwalk for fast parallel directory traversal, collecting all media files
    /// in a single pass. Files are then batched for efficient database operations.
    pub async fn scan_directory_recursive(&self, directory: &Path) -> Result<ScanResult> {
        let policy = ScanPolicy::platform_default(directory, true);
        self.scan_directory_recursive_with_policy(&policy).await
    }

    pub async fn scan_directory_recursive_with_policy(
        &self,
        policy: &ScanPolicy,
    ) -> Result<ScanResult> {
        use jwalk::WalkDir;

        let directory = &policy.root;

        // Use canonical path normalization for consistent database storage
        let canonical_root = match self.filesystem_manager.get_canonical_path(directory) {
            Ok(canonical) => PathBuf::from(canonical),
            Err(e) => {
                warn!(
                    "Failed to get canonical path for {}: {}, using basic normalization",
                    directory.display(),
                    e
                );
                self.filesystem_manager.normalize_path(directory)
            }
        };

        info!(
            "Starting parallel directory scan of: {}",
            canonical_root.display()
        );

        // Load all existing files from database once at the start (for incremental updates)
        debug!("Loading existing files from database...");
        let existing_files_map: HashMap<PathBuf, FileFingerprint> = self
            .database_manager
            .load_file_fingerprints()
            .await?
            .into_iter()
            .map(|fingerprint| (fingerprint.path.clone(), fingerprint))
            .collect();
        debug!(
            "Loaded {} existing files from database",
            existing_files_map.len()
        );

        // Use jwalk for parallel directory traversal - runs in a blocking thread pool
        let root_clone = canonical_root.clone();
        let mut traversal_policy = policy.clone();
        traversal_policy.root = canonical_root.clone();

        let traversal = tokio::task::spawn_blocking(move || {
            let mut report = TraversalReport {
                file_paths: Vec::new(),
                uncertain_prefixes: Vec::new(),
                errors: Vec::new(),
                root_complete: true,
            };
            for entry in WalkDir::new(&root_clone).skip_hidden(false) {
                match entry {
                    Ok(entry) if entry.file_type().is_file() => {
                        let path = entry.path();
                        if traversal_policy.allows_media(&path) {
                            report.file_paths.push(path);
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        let failed_path = error
                            .path()
                            .map(Path::to_path_buf)
                            .unwrap_or_else(|| root_clone.clone());
                        if failed_path == root_clone {
                            report.root_complete = false;
                        }
                        report.uncertain_prefixes.push(failed_path.clone());
                        report.errors.push(ScanError {
                            path: failed_path,
                            error: error.to_string(),
                        });
                    }
                }
            }
            report
        })
        .await?;

        let file_paths = traversal.file_paths;

        let total_files = file_paths.len();
        let existing_in_root = existing_files_map
            .keys()
            .filter(|path| path.starts_with(&canonical_root))
            .count();
        let suspect_empty_root = total_files == 0 && existing_in_root > 0;
        info!(
            "Found {} media files, processing in batches of {}",
            total_files, BATCH_SIZE
        );

        // Process files in batches
        let mut result = ScanResult::new();
        result.errors.extend(traversal.errors);
        result.complete = traversal.root_complete
            && traversal.uncertain_prefixes.is_empty()
            && !suspect_empty_root;
        if suspect_empty_root {
            result.errors.push(ScanError {
                path: canonical_root.clone(),
                error: "previously populated root is unexpectedly empty; destructive reconciliation deferred"
                    .to_owned(),
            });
        }
        let mut files_to_insert: Vec<MediaFile> = Vec::with_capacity(BATCH_SIZE);
        let mut files_to_update: Vec<MediaFile> = Vec::with_capacity(BATCH_SIZE);
        let mut current_paths: HashSet<PathBuf> = HashSet::with_capacity(total_files);
        let mut processed = 0;

        for path in file_paths {
            // jwalk descendants inherit the already-canonical root. It does not
            // follow file symlinks, so ordinary entries require no syscall here.
            current_paths.insert(path.clone());

            // Create MediaFile from path
            let current_file = match self.create_media_file_from_path(&path).await {
                Ok(f) => f,
                Err(e) => {
                    debug!("Failed to create MediaFile for {}: {}", path.display(), e);
                    result.errors.push(ScanError {
                        path: path.clone(),
                        error: e.to_string(),
                    });
                    continue;
                }
            };

            // Check if file exists in database
            if let Some(existing) = existing_files_map.get(&path) {
                if self.fingerprint_needs_update(existing, &current_file) {
                    let mut updated = current_file;
                    updated.id = Some(existing.id);
                    updated.created_at = existing.created_at;
                    updated.updated_at = SystemTime::now();
                    files_to_update.push(updated);
                } else {
                    result.unchanged_files.push(existing.clone());
                }
            } else {
                files_to_insert.push(current_file);
            }

            processed += 1;

            // Process batch when full
            if files_to_insert.len() >= BATCH_SIZE {
                info!(
                    "Inserting batch of {} files ({}/{})",
                    files_to_insert.len(),
                    processed,
                    total_files
                );
                let ids = self
                    .database_manager
                    .bulk_store_canonical_media_files(&files_to_insert)
                    .await?;
                for (i, mut file) in files_to_insert.drain(..).enumerate() {
                    if let Some(id) = ids.get(i) {
                        file.id = Some(*id);
                    }
                    result.new_files.push(file);
                }
            }

            if files_to_update.len() >= BATCH_SIZE {
                info!(
                    "Updating batch of {} files ({}/{})",
                    files_to_update.len(),
                    processed,
                    total_files
                );
                self.database_manager
                    .bulk_update_canonical_media_files(&files_to_update)
                    .await?;
                result.updated_files.append(&mut files_to_update);
            }

            // Progress logging every 1000 files
            if processed % 1000 == 0 {
                info!("Progress: {}/{} files processed", processed, total_files);
            }
        }

        // Process remaining files in last batch
        if !files_to_insert.is_empty() {
            info!("Inserting final batch of {} files", files_to_insert.len());
            let ids = self
                .database_manager
                .bulk_store_canonical_media_files(&files_to_insert)
                .await?;
            for (i, mut file) in files_to_insert.into_iter().enumerate() {
                if let Some(id) = ids.get(i) {
                    file.id = Some(*id);
                }
                result.new_files.push(file);
            }
        }

        if !files_to_update.is_empty() {
            info!("Updating final batch of {} files", files_to_update.len());
            self.database_manager
                .bulk_update_canonical_media_files(&files_to_update)
                .await?;
            result.updated_files.extend(files_to_update);
        }

        // Find and remove deleted files
        let files_to_remove: Vec<PathBuf> = existing_files_map
            .iter()
            .filter(|(path, _)| !current_paths.contains(*path))
            .filter(|(path, _)| path.starts_with(&canonical_root)) // Only remove files under scanned directory
            .filter(|(path, _)| {
                traversal.root_complete
                    && !suspect_empty_root
                    && !traversal
                        .uncertain_prefixes
                        .iter()
                        .any(|prefix| path.starts_with(prefix))
            })
            .map(|(_, file)| file.path.clone())
            .collect();

        if !files_to_remove.is_empty() {
            info!(
                "Removing {} deleted files from database",
                files_to_remove.len()
            );
            self.database_manager
                .bulk_remove_media_files(&files_to_remove)
                .await?;
            let removed_paths = files_to_remove.iter().collect::<HashSet<_>>();
            for (path, file) in existing_files_map.iter() {
                if removed_paths.contains(path) {
                    result.removed_files.push(file.clone());
                }
            }
        }

        result.total_scanned = total_files;

        info!(
            "Scan completed: {} new, {} updated, {} removed, {} unchanged",
            result.new_files.len(),
            result.updated_files.len(),
            result.removed_files.len(),
            result.unchanged_files.len()
        );

        Ok(result)
    }

    /// Create a MediaFile from a path by reading file metadata
    async fn create_media_file_from_path(&self, path: &Path) -> Result<MediaFile> {
        let metadata = tokio::fs::metadata(path).await?;
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let mut mime_type = crate::platform::filesystem::get_mime_type_for_extension(ext);
        if let Some(parent) = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()) {
            if parent.eq_ignore_ascii_case("radio") {
                mime_type = "audio/radio".to_string();
            }
        }
        let size = metadata.len();
        let modified = metadata.modified().unwrap_or_else(|_| SystemTime::now());
        let storage_path = self
            .filesystem_manager
            .get_canonical_path(path)
            .map(PathBuf::from)
            .unwrap_or_else(|_| path.to_path_buf());

        let mut media_file = MediaFile {
            id: None,
            path: storage_path,
            filename,
            size,
            modified,
            mime_type,
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
            subtitle_available: tokio::fs::try_exists(path.with_extension("srt"))
                .await
                .unwrap_or(false),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        };

        if media_file.mime_type.starts_with("audio/") {
            let _ = crate::platform::filesystem::extract_audio_metadata(&mut media_file).await;
        }

        if matches!(ext.to_lowercase().as_str(), "m3u" | "m3u8" | "pls") || media_file.mime_type == "audio/radio" {
            if let Ok(content) = tokio::fs::read_to_string(path).await {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("#EXTINF:") {
                        if let Some((_, title)) = trimmed.split_once(',') {
                            let clean = title.trim();
                            if !clean.is_empty() {
                                media_file.title = Some(clean.to_string());
                                break;
                            }
                        }
                    } else if trimmed.starts_with("Title") && trimmed.contains('=') {
                        if let Some((_, title)) = trimmed.split_once('=') {
                            let clean = title.trim();
                            if !clean.is_empty() {
                                media_file.title = Some(clean.to_string());
                                break;
                            }
                        }
                    }
                }
            }
            if media_file.title.is_none() {
                media_file.title = Some(
                    path.file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| media_file.filename.clone()),
                );
            }
        }

        Ok(media_file)
    }

    /// Get the file system manager (for testing or advanced usage)
    pub fn filesystem_manager(&self) -> &dyn FileSystemManager {
        self.filesystem_manager.as_ref()
    }
}

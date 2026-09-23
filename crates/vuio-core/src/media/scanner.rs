use super::policy::TraversalReport;
use super::*;

/// Media scanner that uses the file system manager and database for efficient scanning.
pub struct MediaScanner<D: DatabaseManager = ActiveDatabase> {
    filesystem_manager: Box<dyn FileSystemManager>,
    database_manager: Arc<D>,
}

/// How many paths the walker may run ahead of the classifier.
///
/// Bounded so that a walk of a large library does not become a list of the
/// library: past this the walking thread blocks until the consumer catches up.
const WALK_QUEUE: usize = 4096;

/// How many changed files to read before writing them out.
///
/// Only files that actually changed reach this, so on an unchanged library it
/// never fills. On a first scan it is what keeps peak memory flat instead of
/// proportional to the library. Matched to [`BATCH_SIZE`] so one window is one
/// write.
const READ_WINDOW: usize = BATCH_SIZE;

/// How many indexed records to load at a time.
///
/// Each page is converted into the scan's own compact form before the next is
/// read, so this — about a megabyte of rows — rather than the size of the
/// library is what the load costs on top of the map it builds.
const FINGERPRINT_PAGE: usize = 4096;

/// How large a scan has to be before the memory it freed is handed back to the
/// system. See [`crate::platform::release_free_memory`].
const RELEASE_AFTER_FILES: usize = 10_000;

/// What a scan needs to know about a file it may already have indexed.
///
/// Deliberately not [`FileFingerprint`]: that carries the path, and this lives
/// in a map keyed by the path, so storing it again doubled the largest
/// allocation a scan makes.
///
/// The map holds one of these for every file under the root, so it is the one
/// allocation a scan makes that grows with the library, and it is packed to
/// match. Times are whole seconds since the epoch because that is all the index
/// stores — a `SystemTime` spent sixteen bytes carrying nanoseconds that were
/// always zero — and the key is a boxed path below the root rather than an
/// owned path from it (see [`IndexedTree`]). At 100,000 files that took the
/// table from 10.1 MB to 7.1 MB, and every key is shorter by the root.
struct IndexedFile {
    id: i64,
    size: u64,
    modified_secs: u64,
    created_at_secs: u64,
    tags_version: u32,
    /// Set when the walk produced this path. What is left unset is what has been
    /// deleted from disk — which is why the scan needs no second collection of
    /// every path it saw.
    seen: bool,
}

/// Everything indexed under the root being scanned, keyed by the path below
/// that root.
///
/// Every file under one root repeats that root's prefix, and there is one key
/// per file. Stripping it is free — the walk only ever produces paths under the
/// root, and a deletion puts it back with a `join`. A record that somehow lies
/// outside the root keeps its absolute path, which cannot collide with a
/// relative one.
struct IndexedTree<'a> {
    root: &'a Path,
    files: HashMap<Box<Path>, IndexedFile>,
}

impl<'a> IndexedTree<'a> {
    fn new(root: &'a Path) -> Self {
        Self {
            root,
            files: HashMap::new(),
        }
    }

    /// Key one page of loaded records, moving each path rather than copying it.
    fn extend(&mut self, fingerprints: Vec<FileFingerprint>) {
        let root = self.root;
        let files = fingerprints
            .into_iter()
            .map(|fingerprint| {
                let FileFingerprint {
                    id,
                    path,
                    size,
                    modified,
                    created_at,
                    tags_version,
                } = fingerprint;
                let key = match path.strip_prefix(root) {
                    Ok(relative) => relative.into(),
                    Err(_) => path.into_boxed_path(),
                };
                let file = IndexedFile {
                    id,
                    size,
                    modified_secs: epoch_secs(modified),
                    created_at_secs: epoch_secs(created_at),
                    tags_version,
                    seen: false,
                };
                (key, file)
            });
        self.files.extend(files);
    }

    fn key<'p>(&self, path: &'p Path) -> &'p Path {
        path.strip_prefix(self.root).unwrap_or(path)
    }

    fn get(&self, path: &Path) -> Option<&IndexedFile> {
        self.files.get(self.key(path))
    }

    fn get_mut(&mut self, path: &Path) -> Option<&mut IndexedFile> {
        let key = self.key(path);
        self.files.get_mut(key)
    }

    fn len(&self) -> usize {
        self.files.len()
    }

    /// Every record the walk did not produce, as absolute paths.
    fn unseen(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.files
            .iter()
            .filter(|(_, indexed)| !indexed.seen)
            .map(|(key, _)| self.root.join(key))
    }
}

fn epoch_secs(time: SystemTime) -> u64 {
    time.duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn from_epoch_secs(secs: u64) -> SystemTime {
    std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs)
}

impl IndexedFile {
    fn modified(&self) -> SystemTime {
        from_epoch_secs(self.modified_secs)
    }

    fn created_at(&self) -> SystemTime {
        from_epoch_secs(self.created_at_secs)
    }
}

impl<D: DatabaseManager> MediaScanner<D> {
    /// Read a window of changed files, several at a time, and sort them into
    /// inserts and updates.
    ///
    /// Each read ends in `spawn_blocking`, so the work already lands on the
    /// blocking pool — but awaiting them one after another meant only ever one
    /// was in flight, and a scan used a single core however many the machine had.
    async fn read_window(
        &self,
        paths: &mut Vec<PathBuf>,
        indexed: &IndexedTree<'_>,
        files_to_insert: &mut Vec<MediaFile>,
        files_to_update: &mut Vec<MediaFile>,
        result: &mut ScanResult,
        concurrency: usize,
    ) -> Result<()> {
        let mut built = futures_util::stream::iter(paths.drain(..))
            .map(|path| async move { (path.clone(), self.create_media_file_from_path(&path).await) })
            .buffer_unordered(concurrency);

        while let Some((path, outcome)) = built.next().await {
            let current_file = match outcome {
                Ok(file) => file,
                Err(e) => {
                    debug!("Failed to create MediaFile for {}: {}", path.display(), e);
                    result.errors.push(ScanError {
                        path,
                        error: e.to_string(),
                    });
                    continue;
                }
            };
            result.files_read += 1;

            match indexed.get(&path) {
                Some(existing) => {
                    if self.fingerprint_needs_update(existing, &current_file) {
                        let mut updated = current_file;
                        updated.id = Some(existing.id);
                        updated.created_at = existing.created_at();
                        updated.updated_at = SystemTime::now();
                        files_to_update.push(updated);
                    } else {
                        result.unchanged += 1;
                    }
                }
                None => files_to_insert.push(current_file),
            }
        }

        Ok(())
    }

    /// Write out whatever a window produced.
    async fn flush_batches(
        &self,
        files_to_insert: &mut Vec<MediaFile>,
        files_to_update: &mut Vec<MediaFile>,
        result: &mut ScanResult,
    ) -> Result<()> {
        if !files_to_insert.is_empty() {
            info!("Inserting batch of {} files", files_to_insert.len());
            self.database_manager
                .bulk_store_canonical_media_files(files_to_insert)
                .await?;
            result.new += files_to_insert.len();
            files_to_insert.clear();
        }

        if !files_to_update.is_empty() {
            info!("Updating batch of {} files", files_to_update.len());
            self.database_manager
                .bulk_update_canonical_media_files(files_to_update)
                .await?;
            result.updated += files_to_update.len();
            files_to_update.clear();
        }

        Ok(())
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
            result.unchanged += existing_files.len();
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
                        result.unchanged += 1;
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
                result.removed += 1;
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
            self.database_manager
                .bulk_store_canonical_media_files(&files_to_insert)
                .await?;
            result.new += files_to_insert.len();
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
            result.updated += files_to_update.len();
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
            result.new,
            result.updated,
            result.removed,
            result.unchanged
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

    /// Whether a record is stale, judged from `stat` alone.
    ///
    /// The same three rules as [`Self::fingerprint_needs_update`], asked before
    /// the file is opened rather than after. Every rule is answerable from
    /// metadata the filesystem already has, which is what makes an unchanged
    /// library nearly free to re-scan.
    #[allow(clippy::absurd_extreme_comparisons)]
    fn stat_needs_update(existing: &IndexedFile, metadata: &std::fs::Metadata) -> bool {
        if existing.size != metadata.len() {
            return true;
        }
        // A record written by an older tag reader is stale even though its file
        // is not, so it is re-read to pick up the fields the new reader knows.
        if existing.tags_version < crate::platform::filesystem::TAGS_VERSION {
            return true;
        }
        let Ok(modified) = metadata.modified() else {
            return true;
        };
        let indexed = existing.modified();
        let difference = if indexed > modified {
            indexed.duration_since(modified)
        } else {
            modified.duration_since(indexed)
        };
        difference.map_or(true, |difference| difference.as_secs() > 10)
    }

    fn fingerprint_needs_update(&self, existing: &IndexedFile, current: &MediaFile) -> bool {
        if existing.size != current.size {
            return true;
        }
        // A record written by an older tag reader is stale even though its file
        // is not. The file has already been parsed by the time we get here, so
        // rewriting it costs one database write and no extra I/O.
        if existing.tags_version < current.tags_version {
            return true;
        }
        let indexed = existing.modified();
        let time_diff = if indexed > current.modified {
            indexed.duration_since(current.modified)
        } else {
            current.modified.duration_since(indexed)
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

        // Load the index for this subtree only. A scan compares one root against
        // one root; the rest of the library would be held for nothing, and a
        // watcher event that rescans a single folder used to load every row.
        debug!("Loading existing files from database...");
        let canonical_root_str = canonical_root.to_string_lossy().into_owned();
        let mut indexed = IndexedTree::new(&canonical_root);
        let mut after: Option<String> = None;
        loop {
            let page = self
                .database_manager
                .load_file_fingerprints_under(&canonical_root_str, after.as_deref(), FINGERPRINT_PAGE)
                .await?;
            let last_page = page.len() < FINGERPRINT_PAGE;
            after = page
                .last()
                .map(|fingerprint| fingerprint.path.to_string_lossy().into_owned());
            indexed.extend(page);
            if last_page {
                break;
            }
        }
        let existing_in_root = indexed.len();
        debug!("Loaded {existing_in_root} existing files from database");

        // Walk on a blocking thread, handing paths over as they are found rather
        // than collecting the library into a `Vec` first. The channel is bounded,
        // so a slow consumer backs the walker up instead of buffering.
        //
        // Serially, because the channel is the only bound there is. jwalk's
        // parallel mode reads directories on the rayon pool as fast as it can
        // and holds each one's entries until the iterator reaches it, so a
        // walker blocked on a full channel stopped nothing: at 100,000 files it
        // was 25 MB of entries read ahead — nearly the whole tree — at the scan's
        // peak, and that peak is what the allocator keeps afterwards. One
        // directory at a time costs no measurable time, because every path it
        // yields is then `stat`ed, and that stage is already concurrent.
        let root_clone = canonical_root.clone();
        let mut traversal_policy = policy.clone();
        traversal_policy.root = canonical_root.clone();
        let (path_sender, path_receiver) = tokio::sync::mpsc::channel::<PathBuf>(WALK_QUEUE);

        let traversal_task = tokio::task::spawn_blocking(move || {
            let mut report = TraversalReport {
                uncertain_prefixes: Vec::new(),
                errors: Vec::new(),
                root_complete: true,
            };
            for entry in WalkDir::new(&root_clone)
                .skip_hidden(false)
                .parallelism(jwalk::Parallelism::Serial)
            {
                match entry {
                    Ok(entry) if entry.file_type().is_file() => {
                        let path = entry.path();
                        if traversal_policy.allows_media(&path) && path_sender.blocking_send(path).is_err() {
                            // The consumer is gone, so the scan is over.
                            break;
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
        });

        let mut result = ScanResult::new();
        // Empty until something changes: a `MediaFile` is over six hundred
        // bytes, so reserving a batch of each up front held 1.3 MB through
        // every scan of a library that turned out not to have changed.
        let mut files_to_insert: Vec<MediaFile> = Vec::new();
        let mut files_to_update: Vec<MediaFile> = Vec::new();
        let mut processed = 0_usize;

        // Classify each path with a single `stat`, several at a time.
        //
        // Building a `MediaFile` canonicalizes the path, probes for a subtitle
        // sidecar and, for audio, parses the entire container with symphonia. Ask
        // the cheap question first, or a library that has not changed is fully
        // re-read on every scan — and there is one every five minutes.
        //
        // Concurrent because a `stat` blocks, on a network filesystem as readily
        // as a local one, and because the answers are independent.
        let concurrency = std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(4);
        let mut pending_reads: Vec<PathBuf> = Vec::with_capacity(READ_WINDOW);
        {
            let mut classified = tokio_stream::wrappers::ReceiverStream::new(path_receiver)
                .map(|path| async move {
                    let metadata = tokio::fs::metadata(&path).await.ok();
                    (path, metadata)
                })
                .buffer_unordered(concurrency);

            while let Some((path, metadata)) = classified.next().await {
                processed += 1;

                // jwalk descendants inherit the already-canonical root. It does
                // not follow file symlinks, so no syscall is needed here.
                let unchanged = match (indexed.get_mut(&path), metadata) {
                    (Some(existing), Some(metadata)) => {
                        // Marking the record is what replaces a second set of every
                        // path on disk: whatever is left unmarked is what is gone.
                        existing.seen = true;
                        !Self::stat_needs_update(existing, &metadata)
                    }
                    (Some(existing), None) => {
                        existing.seen = true;
                        false
                    }
                    // A new file, or one whose `stat` failed —
                    // `create_media_file_from_path` makes the same call and
                    // reports the error properly.
                    (None, _) => false,
                };

                if unchanged {
                    result.unchanged += 1;
                } else {
                    pending_reads.push(path);
                    if pending_reads.len() >= READ_WINDOW {
                        self.read_window(
                            &mut pending_reads,
                            &indexed,
                            &mut files_to_insert,
                            &mut files_to_update,
                            &mut result,
                            concurrency,
                        )
                        .await?;
                        self.flush_batches(
                            &mut files_to_insert,
                            &mut files_to_update,
                            &mut result,
                        )
                        .await?;
                    }
                }

                if processed.is_multiple_of(10_000) {
                    debug!("Examined {processed} files");
                }
            }
        }

        if !pending_reads.is_empty() {
            self.read_window(
                &mut pending_reads,
                &indexed,
                &mut files_to_insert,
                &mut files_to_update,
                &mut result,
                concurrency,
            )
            .await?;
        }
        self.flush_batches(&mut files_to_insert, &mut files_to_update, &mut result)
            .await?;

        // The walker's own findings — which prefixes it could not read — only
        // arrive once the channel has closed, and deletion depends on them.
        let traversal = traversal_task.await?;

        let total_files = processed;
        let suspect_empty_root = total_files == 0 && existing_in_root > 0;
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

        // Whatever the walk never produced is gone from disk.
        let reconcile_deletions =
            traversal.root_complete && !suspect_empty_root && traversal.uncertain_prefixes.is_empty();
        let files_to_remove: Vec<PathBuf> = if reconcile_deletions {
            indexed.unseen().collect()
        } else {
            // A partial walk cannot tell "absent" from "unreadable". Where only
            // some prefixes are in doubt, everything outside them is still
            // decidable.
            indexed
                .unseen()
                .filter(|_| traversal.root_complete && !suspect_empty_root)
                .filter(|path| {
                    !traversal
                        .uncertain_prefixes
                        .iter()
                        .any(|prefix| path.starts_with(prefix))
                })
                .collect()
        };

        if !files_to_remove.is_empty() {
            info!(
                "Removing {} deleted files from database",
                files_to_remove.len()
            );
            self.database_manager
                .bulk_remove_media_files(&files_to_remove)
                .await?;
            result.removed += files_to_remove.len();
        }

        result.total_scanned = total_files;

        info!(
            "Scan completed: {} new, {} updated, {} removed, {} unchanged, {} files read",
            result.new,
            result.updated,
            result.removed,
            result.unchanged,
            result.files_read
        );

        // The index of the root is the scan's largest allocation and is gone
        // now. After a large scan the allocator would otherwise keep the pages
        // it freed for the rest of the run; a watcher's rescan of one folder
        // leaves too little behind to be worth the call.
        drop(indexed);
        if total_files.max(existing_in_root) >= RELEASE_AFTER_FILES {
            tokio::task::spawn_blocking(crate::platform::release_free_memory);
        }

        Ok(result)
    }

    /// Create a MediaFile from a path by reading file metadata.
    ///
    /// Public because the file watcher indexes single files too, and a record
    /// it builds by hand is a record with no tags, no duration, no codec and no
    /// subtitle flag — which a later scan then takes for up to date, because
    /// the fingerprint it committed says the file has already been read.
    pub async fn create_media_file_from_path(&self, path: &Path) -> Result<MediaFile> {
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
            // Which reader has examined this record, not which one found
            // something. A file with no readable tags — a video, or audio whose
            // container will not parse — still counts as examined, or it would
            // be opened again on every scan for as long as it exists.
            tags_version: crate::platform::filesystem::TAGS_VERSION,
            subtitle_available: tokio::fs::try_exists(path.with_extension("srt"))
                .await
                .unwrap_or(false),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        };

        if media_file.mime_type.starts_with("audio/") {
            let _ = crate::platform::filesystem::extract_audio_metadata(&mut media_file).await;
        } else if media_file.mime_type.starts_with("video/") {
            // Films get a header probe too, for one field: the audio track's
            // codec. Deciding "does this need a decoded alternative?" has to be
            // a database read — a folder of 400 films would otherwise open 400
            // files to render one Browse response. Stream properties only: a
            // video's titling stays with the filename.
            let _ = crate::platform::filesystem::extract_stream_info(&mut media_file).await;
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

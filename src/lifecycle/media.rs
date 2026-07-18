/// Validate cached files and remove any that no longer exist on disk
///
/// Uses two-phase approach to avoid RwLock deadlock:
/// 1. Stream all files and collect paths to delete (read lock)
/// 2. Drop stream, then bulk delete (write lock)
async fn validate_and_cleanup_deleted_files(
    database: Arc<database::redb::RedbDatabase>,
    monitored_roots: &[PathBuf],
) -> anyhow::Result<usize> {
    use futures_util::StreamExt;

    info!("Validating cached media files...");

    // Phase 1: Collect paths to delete (holds read lock)
    let mut paths_to_delete = Vec::new();
    let mut total_checked = 0;

    {
        let mut stream = database.stream_all_media_files();

        while let Some(media_file_result) = stream.next().await {
            let media_file =
                media_file_result.context("Failed to read media file from database stream")?;

            total_checked += 1;

            let unavailable_root = monitored_roots
                .iter()
                .any(|root| media_file.path.starts_with(root) && !root.is_dir());
            if !unavailable_root && !media_file.path.exists() {
                paths_to_delete.push(media_file.path.clone());
            }

            // Log progress every 1000 files
            if total_checked % 1000 == 0 {
                info!("Validated {} files so far...", total_checked);
            }
        }
    } // Stream dropped here, read lock released

    // Phase 2: Bulk delete (acquires write lock)
    let removed_count = paths_to_delete.len();
    if !paths_to_delete.is_empty() {
        info!("Removing {} deleted files from database", removed_count);
        database.bulk_remove_media_files(&paths_to_delete).await?;
    }

    if removed_count > 0 {
        info!(
            "Cleaned up {} deleted files from database (checked {} total)",
            removed_count, total_checked
        );
    } else {
        info!(
            "All {} cached files are still present on disk",
            total_checked
        );
    }

    Ok(removed_count)
}

async fn hide_unavailable_media_roots(
    database: &Arc<database::redb::RedbDatabase>,
    roots: &[PathBuf],
) -> anyhow::Result<usize> {
    let mut removed = 0;
    for root in roots {
        if root.is_dir() {
            continue;
        }
        removed += database.remove_derived_content_by_source(root).await?;
        removed += database.remove_media_under_path(root).await?.removed_files;
    }
    Ok(removed)
}

/// Perform initial media scan, using database cache when possible
async fn perform_initial_media_scan(
    config: &AppConfig,
    database: &Arc<database::redb::RedbDatabase>,
) -> anyhow::Result<()> {
    info!("Performing initial media scan...");

    let configured_roots = config
        .media
        .directories
        .iter()
        .map(|directory| PathBuf::from(&directory.path))
        .collect::<Vec<_>>();
    let hidden = hide_unavailable_media_roots(database, &configured_roots).await?;
    if hidden > 0 {
        info!(
            "Removed {} cached items belonging to unavailable media roots",
            hidden
        );
    }

    let database_is_empty = database.get_stats().await?.total_files == 0;
    if config.media.scan_on_startup || database_is_empty {
        if database_is_empty && !config.media.scan_on_startup {
            warn!("Database is empty; forcing a full media scan despite scan_on_startup=false");
        }
        info!("Full media scan enabled - scanning all directories");

        let scanner = media::MediaScanner::with_database(database.clone());
        let mut total_changes = 0;
        let mut total_files_scanned = 0;

        for dir_config in &config.media.directories {
            let dir_path = std::path::PathBuf::from(&dir_config.path);

            if !dir_path.exists() {
                warn!("Media directory does not exist: {}", dir_config.path);
                continue;
            }

            info!("Scanning directory: {}", dir_config.path);

            let scan_result = if dir_config.recursive {
                scanner
                    .scan_directory_recursive(&dir_path)
                    .await
                    .with_context(|| {
                        format!("Failed to recursively scan directory: {}", dir_config.path)
                    })?
            } else {
                scanner
                    .scan_directory(&dir_path)
                    .await
                    .with_context(|| format!("Failed to scan directory: {}", dir_config.path))?
            };

            info!(
                "Scan of {} completed: {}",
                dir_path.display(),
                scan_result.summary()
            );
            if !scan_result.errors.is_empty() {
                // FIX: Iterate over a reference to avoid moving scan_result.errors
                for err in &scan_result.errors {
                    warn!("Scan error in {}: {}", err.path.display(), err.error);
                }
            }
            total_changes += scan_result.total_changes();
            total_files_scanned += scan_result.total_scanned;
        }

        info!(
            "Initial media scan completed - total files scanned: {}, total changes: {}",
            total_files_scanned, total_changes
        );

        // Validate files to catch any that were deleted while app was offline
        if config.media.cleanup_deleted_files {
            let roots: Vec<_> = config
                .media
                .directories
                .iter()
                .map(|d| PathBuf::from(&d.path))
                .collect();
            validate_and_cleanup_deleted_files(database.clone(), &roots).await?;
        }

        Ok(())
    } else {
        info!("Skipping full scan (scan on startup disabled)");

        // Validate that cached files still exist on disk and remove any that don't (if enabled)
        if config.media.cleanup_deleted_files {
            let roots: Vec<_> = config
                .media
                .directories
                .iter()
                .map(|d| PathBuf::from(&d.path))
                .collect();
            validate_and_cleanup_deleted_files(database.clone(), &roots).await?;
        }

        Ok(())
    }
}

/// Perform initial playlist file scan
async fn perform_initial_playlist_scan(
    config: &AppConfig,
    database: &Arc<database::redb::RedbDatabase>,
) -> anyhow::Result<()> {
    if !config.media.scan_playlists {
        info!("Playlist scanning disabled in configuration");
        return Ok(());
    }

    info!("Scanning for playlist files...");

    let mut total_playlists = 0;

    for dir_config in &config.media.directories {
        let dir_path = std::path::PathBuf::from(&dir_config.path);

        if !dir_path.exists() {
            warn!(
                "Media directory does not exist, skipping playlist scan: {}",
                dir_config.path
            );
            continue;
        }

        info!("Scanning for playlists in: {}", dir_config.path);

        let playlist_ids = if dir_config.recursive {
            database
                .scan_and_import_playlists_recursive(&dir_path)
                .await
                .with_context(|| format!("Failed to scan playlists in: {}", dir_config.path))?
        } else {
            database
                .scan_and_import_playlists(&dir_path)
                .await
                .with_context(|| format!("Failed to scan playlists in: {}", dir_config.path))?
        };

        if !playlist_ids.is_empty() {
            info!(
                "Imported {} playlist(s) from {}",
                playlist_ids.len(),
                dir_config.path
            );
        }

        total_playlists += playlist_ids.len();
    }

    if total_playlists > 0 {
        info!(
            "Playlist scan completed: {} playlist(s) imported",
            total_playlists
        );
    } else {
        info!("Playlist scan completed: no playlist files found");
    }

    Ok(())
}

/// Start file system monitoring with database integration
async fn start_file_monitoring(
    watcher: Arc<CrossPlatformWatcher>,
    app_state: AppState,
    cancellation: CancellationToken,
) -> anyhow::Result<Option<tokio::task::JoinHandle<()>>> {
    if !app_state.config.media.watch_for_changes {
        info!("File system monitoring disabled");
        return Ok(None);
    }

    info!("Starting file system monitoring...");

    // Get directories to monitor
    let all_directories: Vec<std::path::PathBuf> = app_state
        .config
        .media
        .directories
        .iter()
        .map(|dir| std::path::PathBuf::from(&dir.path))
        .collect();
    let directories: Vec<_> = all_directories
        .iter()
        .filter(|path| path.is_dir())
        .cloned()
        .collect();

    if directories.is_empty() {
        warn!("No media roots are currently available; recovery probing remains active");
    }

    info!("Starting to monitor {} directories:", directories.len());
    for (i, dir) in directories.iter().enumerate() {
        info!("  {}: {}", i + 1, dir.display());
    }

    // Start watching directories
    watcher
        .start_watching(&directories)
        .await
        .context("Failed to start watching directories")?;

    info!("File system watcher successfully started for all directories");

    // Get event receiver
    let mut event_receiver = watcher.get_event_receiver();

    // Spawn task to handle file system events
    let app_state_clone = app_state.clone();
    let watcher_clone = watcher.clone();

    let handle = tokio::spawn(async move {
        info!("File system event handler started");

        let mut reconciliation = tokio::time::interval(std::time::Duration::from_secs(300));
        reconciliation.tick().await;
        loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    info!("File system event handler received cancellation");
                    break;
                }
                event = event_receiver.recv() => {
                    let Some(event) = event else { break; };
                    if let Err(e) = handle_file_system_event(event, &app_state_clone).await {
                        error!("Failed to handle file system event; reconciling all roots: {}", e);
                        let configured_directories = app_state_clone
                            .media_directories
                            .read()
                            .await
                            .iter()
                            .map(|directory| PathBuf::from(&directory.path))
                            .collect::<Vec<_>>();
                        for root in &configured_directories {
                            if root.is_dir() {
                                let scanner = media::MediaScanner::with_database(app_state_clone.database.clone());
                                if scanner.scan_directory_recursive(root).await.is_ok() {
                                    increment_content_update_id(&app_state_clone).await;
                                }
                            }
                        }
                    }
                }
                _ = reconciliation.tick() => {
                    let configured_roots = app_state_clone
                        .media_directories
                        .read()
                        .await
                        .clone();
                    let configured_directories = configured_roots
                        .iter()
                        .map(|root| PathBuf::from(&root.path))
                        .collect::<Vec<_>>();
                    match hide_unavailable_media_roots(
                        &app_state_clone.database,
                        &configured_directories,
                    )
                    .await
                    {
                        Ok(removed) if removed > 0 => {
                            increment_content_update_id(&app_state_clone).await
                        }
                        Ok(_) => {}
                        Err(error) => {
                            error!("Failed to hide unavailable media roots: {}", error)
                        }
                    }
                    for root in &configured_directories {
                        if root.is_dir() && !watcher_clone.is_watching(root).await {
                            if let Err(error) = watcher_clone.add_watch_path(root).await {
                                error!("Failed to restore watch for {}: {}", root.display(), error);
                            }
                        }
                    }

                    let dirty_roots = watcher_clone.take_dirty_roots();
                    if !dirty_roots.is_empty() {
                        warn!(
                            "Reconciling after dropped watcher events in {} root(s)",
                            dirty_roots.len()
                        );
                    }

                    // Watchers are advisory: some network filesystems and backend
                    // overflows can lose every event for a download. Sweep every
                    // configured root so any supported file is eventually found
                    // even when no Create/Rename/Modify event reaches the app.
                    let scanner = media::MediaScanner::with_database(app_state_clone.database.clone());
                    for root in &configured_roots {
                        let path = PathBuf::from(&root.path);
                        if !path.is_dir() {
                            continue;
                        }
                        let result = if root.recursive {
                            scanner.scan_directory_recursive(&path).await
                        } else {
                            scanner.scan_directory(&path).await
                        };
                        match result {
                            Ok(result) if result.total_changes() > 0 => {
                                increment_content_update_id(&app_state_clone).await
                            }
                            Ok(_) => {}
                            Err(error) => error!(
                                "Periodic media discovery failed for {}: {}",
                                path.display(),
                                error
                            ),
                        }
                    }
                    match validate_and_cleanup_deleted_files(app_state_clone.database.clone(), &configured_directories).await {
                        Ok(removed) if removed > 0 => increment_content_update_id(&app_state_clone).await,
                        Ok(_) => {}
                        Err(error) => error!("Periodic missing-file reconciliation failed: {}", error),
                    }
                }
            }
        }

        warn!("File system event handler stopped");
    });

    info!(
        "File system monitoring started for {} directories",
        directories.len()
    );
    Ok(Some(handle))
}

/// Increment the content update ID to notify DLNA clients of changes
async fn increment_content_update_id(app_state: &AppState) {
    let old_id = app_state
        .content_update_id
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let new_id = old_id.wrapping_add(1);
    app_state.browse_cache.lock().await.clear();
    info!(
        "Content update ID incremented from {} to {}",
        old_id, new_id
    );

    let state = app_state.clone();
    tokio::spawn(async move {
        crate::web::eventing::notify_content_change(&state, new_id).await;
    });
}

/// Atomic application statistics for monitoring
#[derive(Debug)]
pub struct ApplicationStats {
    files_processed: AtomicU64,
    directories_scanned: AtomicU64,
    events_handled: AtomicU64,
    errors_encountered: AtomicU64,
    last_activity: AtomicU64,
}

impl ApplicationStats {
    pub fn new() -> Self {
        let initial_secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            files_processed: AtomicU64::new(0),
            directories_scanned: AtomicU64::new(0),
            events_handled: AtomicU64::new(0),
            errors_encountered: AtomicU64::new(0),
            last_activity: AtomicU64::new(initial_secs),
        }
    }

    fn update_last_activity(&self) {
        let secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_activity.store(secs, Ordering::Relaxed);
    }

    fn record_files_processed(&self, count: u64) {
        self.files_processed.fetch_add(count, Ordering::Relaxed);
        self.update_last_activity();
    }

    fn record_directory_scanned(&self) {
        self.directories_scanned.fetch_add(1, Ordering::Relaxed);
        self.update_last_activity();
    }

    fn record_event_handled(&self) {
        self.events_handled.fetch_add(1, Ordering::Relaxed);
        self.update_last_activity();
    }

    fn record_error(&self) {
        self.errors_encountered.fetch_add(1, Ordering::Relaxed);
        self.update_last_activity();
    }

    pub fn snapshot(&self) -> (u64, u64, u64, u64, SystemTime) {
        let last_secs = self.last_activity.load(Ordering::Relaxed);
        let last_activity = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(last_secs);
        (
            self.files_processed.load(Ordering::Relaxed),
            self.directories_scanned.load(Ordering::Relaxed),
            self.events_handled.load(Ordering::Relaxed),
            self.errors_encountered.load(Ordering::Relaxed),
            last_activity,
        )
    }
}

impl Default for ApplicationStats {
    fn default() -> Self {
        Self::new()
    }
}

fn is_srt_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("srt"))
}

async fn update_subtitle_index(
    subtitle_path: &std::path::Path,
    available: bool,
    app_state: &AppState,
) -> anyhow::Result<bool> {
    let Some(parent) = subtitle_path.parent() else {
        return Ok(false);
    };
    let subtitle_stem = subtitle_path.file_stem();
    let mut changed = Vec::new();
    for mut file in app_state.database.get_files_in_directory(parent).await? {
        if file.path.file_stem() == subtitle_stem && file.subtitle_available != available {
            file.subtitle_available = available;
            file.updated_at = SystemTime::now();
            changed.push(file);
        }
    }
    if changed.is_empty() {
        return Ok(false);
    }
    app_state.database.bulk_update_media_files(&changed).await?;
    increment_content_update_id(app_state).await;
    Ok(true)
}

fn is_supported_media_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(crate::platform::filesystem::is_supported_media_extension)
}

/// Upsert a supported media path from its current filesystem metadata.
async fn index_media_file_path<D: DatabaseManager + ?Sized>(
    database: &D,
    path: &Path,
) -> anyhow::Result<i64> {
    let metadata = tokio::fs::metadata(path).await?;
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    let mime_type = crate::platform::filesystem::get_mime_type_for_extension(extension);
    let mut media_file = database::MediaFile::new(path.to_path_buf(), metadata.len(), mime_type);
    media_file.modified = metadata.modified().unwrap_or_else(|_| SystemTime::now());

    database
        .bulk_store_media_files(&[media_file])
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("media upsert returned no ID for {}", path.display()))
}

async fn handle_file_system_event(
    event: FileSystemEvent,
    app_state: &AppState,
) -> anyhow::Result<()> {
    let database = &app_state.database;
    let stats = &app_state.lifecycle_stats;

    // Record event handling with atomic counter
    stats.record_event_handled();

    match event {
        FileSystemEvent::Created(path) => {
            if is_srt_path(&path) {
                update_subtitle_index(&path, true, app_state).await?;
                return Ok(());
            }
            // Check if this is a directory or a file
            if path.is_dir() {
                info!("Directory created: {}", path.display());

                // Scan the new directory for media files using ReDB bulk operations
                let scanner = media::MediaScanner::with_database(database.clone());
                match scanner.scan_directory_recursive(&path).await {
                    Ok(scan_result) => {
                        info!(
                            "Scanned new directory {}: {}",
                            path.display(),
                            scan_result.summary()
                        );

                        // Files are already stored in database by the scanner using bulk operations

                        // Record atomic statistics
                        stats.record_directory_scanned();
                        stats.record_files_processed(scan_result.new_files.len() as u64);

                        info!("Added {} media files from new directory using ReDB bulk operations: {}", 
                              scan_result.new_files.len(), path.display());

                        // Increment update ID to notify DLNA clients
                        if !scan_result.new_files.is_empty() {
                            increment_content_update_id(app_state).await;
                        }
                    }
                    Err(e) => {
                        error!("Failed to scan new directory {}: {}", path.display(), e);
                    }
                }
            } else {
                // Handle individual media file creation using bulk operations (single-item batch)
                info!("Media file created: {}", path.display());

                if !is_supported_media_path(&path) {
                    debug!("Not a supported media file, ignoring: {}", path.display());
                    return Ok(());
                }

                index_media_file_path(database.as_ref(), &path).await?;

                // Record atomic statistics
                stats.record_files_processed(1);

                info!("Added new media file to ReDB database: {}", path.display());

                // Increment update ID to notify DLNA clients
                increment_content_update_id(app_state).await;
            }
        }

        FileSystemEvent::Modified(path) => {
            if is_srt_path(&path) {
                update_subtitle_index(&path, true, app_state).await?;
                return Ok(());
            }
            info!("Media file modified: {}", path.display());

            if !is_supported_media_path(&path) {
                debug!("Not a supported media file, ignoring: {}", path.display());
                return Ok(());
            }

            // A downloader or platform backend may report only Modify/CloseWrite,
            // without a preceding Create. Upsert missing paths so those event
            // shapes cannot leave a completed download absent from the database.
            if let Some(mut existing_file) = database.get_file_by_path(&path).await? {
                let metadata = tokio::fs::metadata(&path).await?;
                existing_file.size = metadata.len();
                existing_file.modified =
                    metadata.modified().unwrap_or(std::time::SystemTime::now());
                existing_file.updated_at = std::time::SystemTime::now();

                // Use ReDB bulk update operation (single-item batch for atomic consistency)
                database.bulk_update_media_files(&[existing_file]).await?;

                // Record atomic statistics
                stats.record_files_processed(1);

                info!("Updated media file in ReDB database: {}", path.display());

                // Increment update ID to notify DLNA clients
                increment_content_update_id(app_state).await;
            } else if path.is_file() {
                index_media_file_path(database.as_ref(), &path).await?;
                stats.record_files_processed(1);
                info!(
                    "Indexed media file first observed through a modification event: {}",
                    path.display()
                );
                increment_content_update_id(app_state).await;
            } else {
                debug!(
                    "Modified media path disappeared before it could be indexed: {}",
                    path.display()
                );
            }
        }

        FileSystemEvent::Deleted { path, is_directory } => {
            if is_srt_path(&path) {
                update_subtitle_index(&path, false, app_state).await?;
                return Ok(());
            }
            info!("Path deleted: {}", path.display());
            let derived_removed = database.remove_derived_content_by_source(&path).await?;
            let summary = database
                .remove_media_under_path(&path)
                .await
                .map_err(|error| {
                    stats.record_error();
                    error
                })?;
            stats.record_files_processed(summary.removed_files as u64);
            info!(
                "Removed {} indexed files and {} derived items below deleted path {}",
                summary.removed_files,
                derived_removed,
                path.display()
            );
            // Publish empty/duplicate directory events because they can retire an
            // older browse generation. Known unrelated file deletions do not churn
            // the library revision unless they removed indexed/derived content.
            if is_directory != Some(false) || summary.removed_files > 0 || derived_removed > 0 {
                increment_content_update_id(app_state).await;
            }
        }

        FileSystemEvent::Renamed { from, to } => {
            if is_srt_path(&from) || is_srt_path(&to) {
                if is_srt_path(&from) {
                    update_subtitle_index(&from, false, app_state).await?;
                }
                if is_srt_path(&to) {
                    update_subtitle_index(&to, true, app_state).await?;
                }
                return Ok(());
            }
            info!("Path renamed: {} -> {}", from.display(), to.display());

            // Check if the destination is a directory or file
            if to.is_dir() {
                // Handle directory rename using ReDB bulk operations
                info!("Directory renamed: {} -> {}", from.display(), to.display());

                // Use efficient path prefix query to find files in the old directory path
                let path_normalizer = create_platform_path_normalizer();
                let canonical_from_prefix = path_normalizer.to_canonical(&from)?;
                let files_in_old_path = database
                    .get_files_with_path_prefix(&canonical_from_prefix)
                    .await?;

                if !files_in_old_path.is_empty() {
                    info!(
                        "Updating {} media files for renamed directory using ReDB bulk operations",
                        files_in_old_path.len()
                    );

                    // Collect paths for bulk removal
                    let old_paths: Vec<PathBuf> =
                        files_in_old_path.iter().map(|f| f.path.clone()).collect();

                    // Remove old files from database using ReDB bulk operation
                    let removed_count = database.bulk_remove_media_files(&old_paths).await?;
                    info!(
                        "ReDB bulk removal: {} files removed for renamed directory",
                        removed_count
                    );

                    // Scan the new directory location using ReDB bulk operations
                    let scanner = media::MediaScanner::with_database(database.clone());
                    match scanner.scan_directory_recursive(&to).await {
                        Ok(scan_result) => {
                            info!(
                                "Rescanned renamed directory {}: {}",
                                to.display(),
                                scan_result.summary()
                            );

                            // Files are already stored in database by the scanner using ReDB bulk operations

                            // Increment update ID to notify DLNA clients
                            increment_content_update_id(app_state).await;
                        }
                        Err(e) => {
                            error!("Failed to rescan renamed directory {}: {}", to.display(), e);
                        }
                    }
                }
            } else {
                // Handle individual file renames based on both endpoints. Any
                // non-media staging name promoted to any supported media type is
                // a create because the staging source is intentionally unindexed.
                info!("File renamed: {} -> {}", from.display(), to.display());

                match classify_media_rename(&from, &to) {
                    MediaRenameKind::Ignore => {
                        debug!(
                            "Rename has no supported media endpoint, ignoring: {} -> {}",
                            from.display(),
                            to.display()
                        );
                    }
                    MediaRenameKind::Create => {
                        index_media_file_path(database.as_ref(), &to).await?;
                        stats.record_files_processed(1);
                        info!(
                            "Indexed completed media file after download rename: {}",
                            to.display()
                        );
                        increment_content_update_id(app_state).await;
                    }
                    MediaRenameKind::Remove => {
                        let removed = database.bulk_remove_media_files(&[from.clone()]).await?;
                        if removed > 0 {
                            stats.record_files_processed(removed as u64);
                            info!(
                                "Removed media file renamed to non-media path: {}",
                                from.display()
                            );
                            increment_content_update_id(app_state).await;
                        } else {
                            debug!("Media rename source was already absent: {}", from.display());
                        }
                    }
                    MediaRenameKind::Replace => {
                        let removed = database.bulk_remove_media_files(&[from.clone()]).await?;
                        if removed == 0 {
                            debug!(
                                "Media rename source was absent; destination will still be indexed: {}",
                                from.display()
                            );
                        }
                        index_media_file_path(database.as_ref(), &to).await?;
                        stats.record_files_processed(1);
                        info!("Renamed media file: {} -> {}", from.display(), to.display());
                        increment_content_update_id(app_state).await;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Media scanning, filesystem-event, and reconciliation lifecycle operations.
pub struct MediaLifecycleService;

impl MediaLifecycleService {
    pub async fn initial_scan(
        config: &AppConfig,
        database: &Arc<database::redb::RedbDatabase>,
    ) -> anyhow::Result<()> {
        perform_initial_media_scan(config, database).await?;
        perform_initial_playlist_scan(config, database).await
    }

    pub async fn handle_event(event: FileSystemEvent, state: &AppState) -> anyhow::Result<()> {
        handle_file_system_event(event, state).await
    }

    pub async fn start_monitoring(
        watcher: Arc<CrossPlatformWatcher>,
        state: AppState,
        cancellation: CancellationToken,
    ) -> anyhow::Result<Option<tokio::task::JoinHandle<()>>> {
        start_file_monitoring(watcher, state, cancellation).await
    }
}

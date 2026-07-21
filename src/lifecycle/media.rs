/// Validate cached files and remove any that no longer exist on disk
///
/// Uses two-phase approach to avoid RwLock deadlock:
/// 1. Stream all files and collect paths to delete (read lock)
/// 2. Drop stream, then bulk delete (write lock)
async fn validate_and_cleanup_deleted_files<D: DatabaseManager>(
    database: Arc<D>,
    monitored_roots: &[PathBuf],
) -> anyhow::Result<usize> {
    use futures_util::{stream, StreamExt};

    info!("Validating cached media files...");

    // Phase 1: Collect paths to delete (holds read lock)
    let unavailable_roots = database
        .list_root_availability()
        .await?
        .into_iter()
        .filter(|root| root.unavailable_since_secs.is_some())
        .map(|root| root.path)
        .collect::<Vec<_>>();

    let unavailable_configured_roots = stream::iter(monitored_roots.iter().cloned())
        .map(|root| async move {
            let unavailable = match tokio::fs::metadata(&root).await {
                Ok(metadata) => !metadata.is_dir(),
                Err(_) => true,
            };
            (root, unavailable)
        })
        .buffer_unordered(32)
        .filter_map(|(root, unavailable)| async move { unavailable.then_some(root) })
        .collect::<Vec<_>>()
        .await;
    let fingerprints = database.load_file_fingerprints().await?;
    let total_checked = fingerprints.len();
    let paths_to_delete: Vec<PathBuf> = stream::iter(fingerprints)
        .map(|media_file| {
            let unavailable_roots = &unavailable_roots;
            let unavailable_configured_roots = &unavailable_configured_roots;
            async move {
                let unavailable = unavailable_configured_roots
                    .iter()
                    .chain(unavailable_roots.iter())
                    .any(|root| media_file.path.starts_with(root));
                if unavailable {
                    return None;
                }
                tokio::fs::symlink_metadata(&media_file.path)
                    .await
                    .is_err()
                    .then_some(media_file.path)
            }
        })
        .buffer_unordered(32)
        .filter_map(std::future::ready)
        .collect()
        .await;

    // Phase 2: Bulk delete (acquires write lock)
    let removed_count = paths_to_delete.len();
    if !paths_to_delete.is_empty() {
        info!("Removing {} deleted files from database", removed_count);
        database
            .bulk_remove_canonical_media_files(&paths_to_delete)
            .await?;
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

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn reconcile_unavailable_media_roots<D: DatabaseManager>(
    database: &Arc<D>,
    roots: &[PathBuf],
    grace_hours: u64,
) -> anyhow::Result<usize> {
    let fingerprints = database.load_file_fingerprints().await?;
    let now = unix_now_secs();
    let grace_secs = grace_hours.saturating_mul(3600);
    let mut removed = 0;
    for root in roots {
        let indexed_count = fingerprints
            .iter()
            .filter(|file| file.path.starts_with(root))
            .count() as u64;
        let probe = tokio::fs::read_dir(root).await;
        let reason = match probe {
            Ok(mut entries) => {
                if indexed_count > 0 && entries.next_entry().await?.is_none() {
                    Some("previously populated root is unexpectedly empty".to_owned())
                } else {
                    database
                        .set_root_availability(&database::RootAvailability {
                            path: root.clone(),
                            last_seen_secs: now,
                            unavailable_since_secs: None,
                            indexed_count,
                            reason: String::new(),
                        })
                        .await?;
                    None
                }
            }
            Err(error) => Some(format!("{}: {error}", error.kind())),
        };
        let Some(reason) = reason else {
            continue;
        };
        let previous = database.get_root_availability(root).await?;
        let unavailable_since = previous
            .as_ref()
            .and_then(|state| state.unavailable_since_secs)
            .unwrap_or(now);
        database
            .set_root_availability(&database::RootAvailability {
                path: root.clone(),
                last_seen_secs: previous.as_ref().map_or(0, |state| state.last_seen_secs),
                unavailable_since_secs: Some(unavailable_since),
                indexed_count: previous
                    .as_ref()
                    .map_or(indexed_count, |state| state.indexed_count.max(indexed_count)),
                reason: reason.clone(),
            })
            .await?;

        let permission_denied = reason.starts_with("permission denied");
        if !permission_denied && now.saturating_sub(unavailable_since) >= grace_secs {
            removed += database.remove_derived_content_by_source(root).await?;
            removed += database.remove_media_under_path(root).await?.removed_files;
        }
    }
    Ok(removed)
}

async fn record_root_scan<D: DatabaseManager>(
    database: &Arc<D>,
    root: &Path,
    result: &media::ScanResult,
) -> anyhow::Result<()> {
    let now = unix_now_secs();
    let previous = database.get_root_availability(root).await?;
    let state = if result.complete {
        database::RootAvailability {
            path: root.to_path_buf(),
            last_seen_secs: now,
            unavailable_since_secs: None,
            indexed_count: result.total_scanned as u64,
            reason: String::new(),
        }
    } else {
        database::RootAvailability {
            path: root.to_path_buf(),
            last_seen_secs: previous.as_ref().map_or(0, |state| state.last_seen_secs),
            unavailable_since_secs: Some(
                previous
                    .as_ref()
                    .and_then(|state| state.unavailable_since_secs)
                    .unwrap_or(now),
            ),
            indexed_count: previous
                .as_ref()
                .map_or(result.total_scanned as u64, |state| state.indexed_count),
            reason: result
                .errors
                .first()
                .map_or_else(|| "incomplete scan".to_owned(), |error| error.error.clone()),
        }
    };
    database.set_root_availability(&state).await
}

async fn reconcile_media_roots<D: DatabaseManager + 'static>(
    app_state: &AppState<D>,
    roots: &[crate::config::MonitoredDirectoryConfig],
) {
    let scanner = media::MediaScanner::with_database(app_state.database.clone());
    for root in roots {
        let path = PathBuf::from(&root.path);
        if !matches!(tokio::fs::metadata(&path).await, Ok(metadata) if metadata.is_dir()) {
            continue;
        }
        let policy = media::ScanPolicy::from_config(&app_state.current_config(), root);
        let scan = if root.recursive {
            scanner.scan_directory_recursive_with_policy(&policy).await
        } else {
            scanner.scan_directory_with_policy(&policy).await
        };
        match scan {
            Ok(result) => {
                if let Err(error) = record_root_scan(&app_state.database, &path, &result).await {
                    error!("Failed to persist root scan state for {}: {}", path.display(), error);
                }
                if result.total_changes() > 0 {
                    increment_content_update_id(app_state).await;
                }
            }
            Err(error) => error!("Media discovery failed for {}: {}", path.display(), error),
        }
    }
}

pub(crate) async fn refresh_unavailable_roots<D: DatabaseManager>(
    app_state: &AppState<D>,
) -> anyhow::Result<()> {
    let unavailable = app_state
        .database
        .list_root_availability()
        .await?
        .into_iter()
        .filter(|state| state.unavailable_since_secs.is_some())
        .map(|state| state.path)
        .collect();
    *app_state.unavailable_roots.write().await = unavailable;
    Ok(())
}

/// Perform initial media scan, using database cache when possible
async fn perform_initial_media_scan<D: DatabaseManager + 'static>(
    config: &AppConfig,
    database: &Arc<D>,
) -> anyhow::Result<()> {
    info!("Performing initial media scan...");

    let configured_roots = config
        .media
        .directories
        .iter()
        .map(|directory| PathBuf::from(&directory.path))
        .collect::<Vec<_>>();
    let hidden = reconcile_unavailable_media_roots(
        database,
        &configured_roots,
        config.media.unavailable_root_grace_hours,
    )
    .await?;
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
            let policy = media::ScanPolicy::from_config(config, dir_config);

            if !dir_path.exists() {
                warn!("Media directory does not exist: {}", dir_config.path);
                continue;
            }

            info!("Scanning directory: {}", dir_config.path);

            let scan_result = if dir_config.recursive {
                scanner
                    .scan_directory_recursive_with_policy(&policy)
                    .await
                    .with_context(|| {
                        format!("Failed to recursively scan directory: {}", dir_config.path)
                    })?
            } else {
                scanner
                    .scan_directory_with_policy(&policy)
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
            record_root_scan(database, &dir_path, &scan_result).await?;
            total_changes += scan_result.total_changes();
            total_files_scanned += scan_result.total_scanned;
        }

        info!(
            "Initial media scan completed - total files scanned: {}, total changes: {}",
            total_files_scanned, total_changes
        );

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
async fn perform_initial_playlist_scan<D: DatabaseManager + 'static>(
    config: &AppConfig,
    database: &Arc<D>,
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
async fn start_file_monitoring<D: DatabaseManager + 'static>(
    watcher: Arc<CrossPlatformWatcher>,
    app_state: AppState<D>,
    cancellation: CancellationToken,
) -> anyhow::Result<Option<tokio::task::JoinHandle<()>>> {
    let watching_enabled = app_state.current_config().media.watch_for_changes;
    info!("Starting file system monitoring controller...");

    // Get directories to monitor
    let all_directories: Vec<std::path::PathBuf> = app_state
        .current_config()
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
    if watching_enabled {
        watcher
            .start_watching(&directories)
            .await
            .context("Failed to start watching directories")?;
        info!("File system watcher successfully started for all directories");
    } else {
        info!("File system watching is disabled; controller remains ready for reload");
    }

    // Get event receiver
    let mut event_receiver = watcher
        .take_event_receiver()
        .await
        .context("File-system event receiver was already consumed")?;

    // Spawn task to handle file system events
    let app_state_clone = app_state.clone();
    let watcher_clone = watcher.clone();

    let handle = tokio::spawn(async move {
        info!("File system event handler started");

        let mut dirty_reconciliation =
            tokio::time::interval(std::time::Duration::from_secs(30));
        let mut full_reconciliation =
            tokio::time::interval(std::time::Duration::from_secs(300));
        dirty_reconciliation.tick().await;
        full_reconciliation.tick().await;
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
                        let configured_roots = app_state_clone
                            .media_directories
                            .read()
                            .await
                            .clone();
                        reconcile_media_roots(&app_state_clone, &configured_roots).await;
                    }
                }
                _ = dirty_reconciliation.tick() => {
                    let dirty_roots = coalesce_roots(watcher_clone.take_dirty_roots());
                    if dirty_roots.is_empty() {
                        continue;
                    }
                    warn!("Reconciling after dropped watcher events in {} dirty path(s)", dirty_roots.len());
                    let configured_roots = app_state_clone.media_directories.read().await.clone();
                    let roots_to_scan = configured_roots
                        .into_iter()
                        .filter(|root| {
                            let path = Path::new(&root.path);
                            dirty_roots
                                .iter()
                                .any(|dirty| dirty.starts_with(path) || path.starts_with(dirty))
                        })
                        .collect::<Vec<_>>();
                    reconcile_media_roots(&app_state_clone, &roots_to_scan).await;
                }
                _ = full_reconciliation.tick() => {
                    let configured_roots = app_state_clone
                        .media_directories
                        .read()
                        .await
                        .clone();
                    let configured_directories = configured_roots
                        .iter()
                        .map(|root| PathBuf::from(&root.path))
                        .collect::<Vec<_>>();
                    match reconcile_unavailable_media_roots(
                        &app_state_clone.database,
                        &configured_directories,
                        app_state_clone.current_config().media.unavailable_root_grace_hours,
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
                            let Some(root_config) = configured_roots
                                .iter()
                                .find(|configured| Path::new(&configured.path) == root)
                            else { continue; };
                            let policy = media::ScanPolicy::from_config(
                                &app_state_clone.current_config(),
                                root_config,
                            );
                            if let Err(error) = watcher_clone.add_watch_policy(policy).await {
                                error!("Failed to restore watch for {}: {}", root.display(), error);
                            }
                        }
                    }

                    // This mandatory sweep is independent of the dirty-root queue, so a noisy
                    // root cannot starve reconciliation of the rest of the library.
                    reconcile_media_roots(&app_state_clone, &configured_roots).await;
                    if let Err(error) = refresh_unavailable_roots(&app_state_clone).await {
                        error!("Failed to refresh unavailable-root visibility: {}", error);
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

/// Coalesce dirty root paths so overlapping subtrees are merged
fn coalesce_roots(mut roots: Vec<PathBuf>) -> Vec<PathBuf> {
    roots.sort_by_key(|p| p.components().count());
    let mut coalesced = Vec::new();
    for root in roots {
        if !coalesced.iter().any(|parent: &PathBuf| root.starts_with(parent)) {
            coalesced.push(root);
        }
    }
    coalesced
}

/// Increment the content update ID to notify DLNA clients of changes
async fn increment_content_update_id<D: DatabaseManager + 'static>(app_state: &AppState<D>) {
    crate::web::eventing::publish_content_change(app_state).await;
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

async fn update_subtitle_index<D: DatabaseManager + 'static>(
    subtitle_path: &std::path::Path,
    available: bool,
    app_state: &AppState<D>,
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

/// Upsert a supported media path from its current filesystem metadata.
async fn index_media_file_path<D: DatabaseManager + ?Sized>(
    database: &D,
    path: &Path,
    policy: &media::ScanPolicy,
    filesystem_manager: &dyn crate::platform::filesystem::FileSystemManager,
) -> anyhow::Result<Option<i64>> {
    let Some(path) = policy
        .secure_canonical_path(path, filesystem_manager)
        .await?
    else {
        return Ok(None);
    };
    let mut media_file = media::build_media_file_from_path(&path, filesystem_manager).await?;
    if let Some(existing) = database.get_file_by_path(&media_file.path).await? {
        media_file.id = existing.id;
        media_file.created_at = existing.created_at;
    }

    database
        .bulk_store_media_files(&[media_file])
        .await?
        .into_iter()
        .next()
        .map(Some)
        .ok_or_else(|| anyhow::anyhow!("media upsert returned no ID for {}", path.display()))
}

async fn reconcile_rejected_watched_path<D: DatabaseManager + 'static>(
    database: &Arc<D>,
    policy: &media::ScanPolicy,
    path: &Path,
) -> anyhow::Result<media::ScanResult> {
    let scope = path.parent().unwrap_or(&policy.root);
    media::MediaScanner::with_database(database.clone())
        .scan_directory_recursive_with_policy(&policy.for_subtree(scope))
        .await
}

async fn import_changed_playlist<D: DatabaseManager + ?Sized>(
    database: &D,
    path: &Path,
) -> anyhow::Result<()> {
    let is_radio = path.parent().is_some_and(|parent| {
        parent.components().any(|component| {
            component
                .as_os_str()
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("radio"))
        })
    });
    if is_radio {
        database::playlist_formats::PlaylistFileManager::import_radio_playlist(database, path)
            .await
    } else {
        database.import_playlist_file(path, None).await.map(|_| ())
    }
}

async fn handle_file_system_event<D: DatabaseManager + 'static>(
    event: FileSystemEvent,
    app_state: &AppState<D>,
) -> anyhow::Result<()> {
    let database = &app_state.database;
    let stats = &app_state.lifecycle_stats;
    let policies = media::ScanPolicy::policies(&app_state.current_config());

    // Record event handling with atomic counter
    stats.record_event_handled();

    match event {
        FileSystemEvent::Created(path) => {
            let policy = media::ScanPolicy::for_path(&policies, &path).cloned();
            let Some(policy) = policy else {
                return Ok(());
            };
            let Some(secure_path) = policy
                .secure_canonical_path(&path, app_state.filesystem_manager.as_ref())
                .await?
            else {
                let scan = reconcile_rejected_watched_path(database, &policy, &path).await?;
                if scan.has_changes() {
                    increment_content_update_id(app_state).await;
                }
                return Ok(());
            };
            if is_srt_path(&path) {
                update_subtitle_index(&secure_path, true, app_state).await?;
                return Ok(());
            }
            // Check if this is a directory or a file
            if path.is_dir() {
                if !policy.recursive || path == policy.root {
                    return Ok(());
                }
                info!("Directory created: {}", path.display());

                // Scan the new directory for media files using ReDB bulk operations
                let scanner = media::MediaScanner::with_database(database.clone());
                match scanner
                    .scan_directory_recursive_with_policy(&policy.for_subtree(&path))
                    .await
                {
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

                if policy.allows_playlist(&path) {
                    import_changed_playlist(database.as_ref(), &secure_path).await?;
                    increment_content_update_id(app_state).await;
                    return Ok(());
                }
                if !policy.allows_media(&path) {
                    debug!("Not a supported media file, ignoring: {}", path.display());
                    return Ok(());
                }

                if index_media_file_path(
                    database.as_ref(),
                    &path,
                    &policy,
                    app_state.filesystem_manager.as_ref(),
                )
                .await?
                .is_none()
                {
                    return Ok(());
                }

                // Record atomic statistics
                stats.record_files_processed(1);

                info!("Added new media file to ReDB database: {}", path.display());

                // Increment update ID to notify DLNA clients
                increment_content_update_id(app_state).await;
            }
        }

        FileSystemEvent::Modified(path) => {
            let policy = media::ScanPolicy::for_path(&policies, &path).cloned();
            let Some(policy) = policy else {
                return Ok(());
            };
            let Some(secure_path) = policy
                .secure_canonical_path(&path, app_state.filesystem_manager.as_ref())
                .await?
            else {
                let scan = reconcile_rejected_watched_path(database, &policy, &path).await?;
                if scan.has_changes() {
                    increment_content_update_id(app_state).await;
                }
                return Ok(());
            };
            if is_srt_path(&path) {
                update_subtitle_index(&secure_path, true, app_state).await?;
                return Ok(());
            }
            info!("Media file modified: {}", path.display());

            if policy.allows_playlist(&path) {
                import_changed_playlist(database.as_ref(), &secure_path).await?;
                increment_content_update_id(app_state).await;
                return Ok(());
            }
            if !policy.allows_media(&path) {
                debug!("Not a supported media file, ignoring: {}", path.display());
                return Ok(());
            }

            // A downloader or platform backend may report only Modify/CloseWrite,
            // without a preceding Create. Upsert missing paths so those event
            // shapes cannot leave a completed download absent from the database.
            if let Some(existing_file) = database.get_file_by_path(&secure_path).await? {
                let mut refreshed = media::build_media_file_from_path(
                    &secure_path,
                    app_state.filesystem_manager.as_ref(),
                )
                .await?;
                refreshed.id = existing_file.id;
                refreshed.created_at = existing_file.created_at;

                // Use ReDB bulk update operation (single-item batch for atomic consistency)
                database.bulk_update_media_files(&[refreshed]).await?;

                // Record atomic statistics
                stats.record_files_processed(1);

                info!("Updated media file in ReDB database: {}", path.display());

                // Increment update ID to notify DLNA clients
                increment_content_update_id(app_state).await;
            } else if secure_path.is_file() {
                if index_media_file_path(
                    database.as_ref(),
                    &path,
                    &policy,
                    app_state.filesystem_manager.as_ref(),
                )
                .await?
                .is_none()
                {
                    return Ok(());
                }
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
                .inspect_err(|_error| {
                    stats.record_error();
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
                    let secure_to = media::ScanPolicy::for_path(&policies, &to)
                        .map(|policy| async {
                            policy
                                .secure_canonical_path(
                                    &to,
                                    app_state.filesystem_manager.as_ref(),
                                )
                                .await
                        });
                    if let Some(result) = secure_to {
                        if let Some(path) = result.await? {
                            update_subtitle_index(&path, true, app_state).await?;
                        }
                    }
                }
                return Ok(());
            }
            info!("Path renamed: {} -> {}", from.display(), to.display());

            let path_normalizer = create_platform_path_normalizer();
            let canonical_from_prefix = path_normalizer.to_canonical(&from)?;
            let files_in_old_path = database
                .get_files_with_path_prefix(&canonical_from_prefix)
                .await?;
            let looks_like_directory = to.is_dir()
                || files_in_old_path
                    .iter()
                    .any(|file| file.path.as_path() != from.as_path());
            if looks_like_directory {
                info!("Directory renamed: {} -> {}", from.display(), to.display());
                let from_policy = media::ScanPolicy::for_path(&policies, &from)
                    .filter(|policy| from == policy.root || policy.recursive)
                    .cloned();
                let to_policy = media::ScanPolicy::for_path(&policies, &to)
                    .filter(|policy| to == policy.root || policy.recursive)
                    .cloned();
                match (from_policy, to_policy) {
                    (None, None) => {}
                    (Some(_), None) => {
                        let removed = database.remove_media_under_path(&from).await?;
                        let derived = database.remove_derived_content_by_source(&from).await?;
                        if removed.removed_files > 0 || derived > 0 {
                            increment_content_update_id(app_state).await;
                        }
                    }
                    (from_policy, Some(policy)) => {
                        // Stage the complete destination before touching old records.
                        let scanner = media::MediaScanner::with_database(database.clone());
                        let mut staged = scanner
                            .stage_directory_with_policy(&policy.for_subtree(&to))
                            .await?;
                        let old_by_relative = files_in_old_path
                            .iter()
                            .filter_map(|file| {
                                file.path
                                    .strip_prefix(&from)
                                    .ok()
                                    .map(|relative| (relative.to_path_buf(), file))
                            })
                            .collect::<HashMap<_, _>>();
                        for file in &mut staged {
                            if let Ok(relative) = file.path.strip_prefix(&to) {
                                if let Some(previous) = old_by_relative.get(relative) {
                                    file.id = previous.id;
                                    file.created_at = previous.created_at;
                                }
                            }
                        }
                        // Reusing IDs makes each matched old->new remap part of
                        // the same ReDB transaction as the staged upsert.
                        database.bulk_store_canonical_media_files(&staged).await?;
                        let staged_ids = staged.iter().filter_map(|file| file.id).collect::<HashSet<_>>();
                        let stale_old = files_in_old_path
                            .iter()
                            .filter(|file| file.id.is_none_or(|id| !staged_ids.contains(&id)))
                            .map(|file| file.path.clone())
                            .collect::<Vec<_>>();
                        if !stale_old.is_empty() {
                            database.bulk_remove_media_files(&stale_old).await?;
                        }
                        if from_policy.is_some() {
                            database.remove_derived_content_by_source(&from).await?;
                        }
                        if !staged.is_empty() || !files_in_old_path.is_empty() {
                            increment_content_update_id(app_state).await;
                        }
                    }
                }
            } else {
                // Handle individual file renames based on both endpoints. Any
                // non-media staging name promoted to any supported media type is
                // a create because the staging source is intentionally unindexed.
                info!("File renamed: {} -> {}", from.display(), to.display());

                let from_playlist = media::ScanPolicy::for_path(&policies, &from)
                    .is_some_and(|policy| policy.allows_playlist(&from));
                let to_playlist = media::ScanPolicy::for_path(&policies, &to)
                    .is_some_and(|policy| policy.allows_playlist(&to));
                if from_playlist || to_playlist {
                    if from_playlist {
                        database.remove_derived_content_by_source(&from).await?;
                    }
                    if to_playlist && to.is_file() {
                        let policy = media::ScanPolicy::for_path(&policies, &to)
                            .expect("playlist rename has a destination policy");
                        if let Some(path) = policy
                            .secure_canonical_path(
                                &to,
                                app_state.filesystem_manager.as_ref(),
                            )
                            .await?
                        {
                            import_changed_playlist(database.as_ref(), &path).await?;
                        }
                    }
                    increment_content_update_id(app_state).await;
                    return Ok(());
                }

                let from_media = media::ScanPolicy::for_path(&policies, &from)
                    .is_some_and(|policy| policy.allows_media(&from));
                let to_media = media::ScanPolicy::for_path(&policies, &to)
                    .is_some_and(|policy| policy.allows_media(&to));
                let rename_kind = match (from_media, to_media) {
                    (false, false) => MediaRenameKind::Ignore,
                    (false, true) => MediaRenameKind::Create,
                    (true, false) => MediaRenameKind::Remove,
                    (true, true) => MediaRenameKind::Replace,
                };
                match rename_kind {
                    MediaRenameKind::Ignore => {
                        debug!(
                            "Rename has no supported media endpoint, ignoring: {} -> {}",
                            from.display(),
                            to.display()
                        );
                    }
                    MediaRenameKind::Create => {
                        if index_media_file_path(
                            database.as_ref(),
                            &to,
                            media::ScanPolicy::for_path(&policies, &to)
                                .expect("create rename has a destination policy"),
                            app_state.filesystem_manager.as_ref(),
                        )
                        .await?
                        .is_none()
                        {
                            return Ok(());
                        }
                        stats.record_files_processed(1);
                        info!(
                            "Indexed completed media file after download rename: {}",
                            to.display()
                        );
                        increment_content_update_id(app_state).await;
                    }
                    MediaRenameKind::Remove => {
                        let removed = database
                            .bulk_remove_media_files(std::slice::from_ref(&from))
                            .await?;
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
                        let removed = database
                            .bulk_remove_media_files(std::slice::from_ref(&from))
                            .await?;
                        if removed == 0 {
                            debug!(
                                "Media rename source was absent; destination will still be indexed: {}",
                                from.display()
                            );
                        }
                        let indexed = index_media_file_path(
                            database.as_ref(),
                            &to,
                            media::ScanPolicy::for_path(&policies, &to)
                                .expect("replace rename has a destination policy"),
                            app_state.filesystem_manager.as_ref(),
                        )
                        .await?
                        .is_some();
                        stats.record_files_processed(indexed as u64);
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
    pub async fn initial_scan<D: DatabaseManager + 'static>(
        config: &AppConfig,
        database: &Arc<D>,
    ) -> anyhow::Result<()> {
        perform_initial_media_scan(config, database).await?;
        perform_initial_playlist_scan(config, database).await
    }

    pub async fn handle_event<D: DatabaseManager + 'static>(event: FileSystemEvent, state: &AppState<D>) -> anyhow::Result<()> {
        handle_file_system_event(event, state).await
    }

    pub async fn start_monitoring<D: DatabaseManager + 'static>(
        watcher: Arc<CrossPlatformWatcher>,
        state: AppState<D>,
        cancellation: CancellationToken,
    ) -> anyhow::Result<Option<tokio::task::JoinHandle<()>>> {
        start_file_monitoring(watcher, state, cancellation).await
    }
}

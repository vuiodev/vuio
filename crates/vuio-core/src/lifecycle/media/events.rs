use super::super::*;

/// Increment the content update ID to notify DLNA clients of changes
pub(in crate::lifecycle) async fn increment_content_update_id<D: DatabaseManager + 'static>(
    app_state: &AppState<D>,
) {
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

pub(in crate::lifecycle) fn is_srt_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("srt"))
}

pub(in crate::lifecycle) async fn update_subtitle_index<D: DatabaseManager + 'static>(
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
pub(in crate::lifecycle) async fn index_media_file_path<D: DatabaseManager + ?Sized>(
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

pub(in crate::lifecycle) async fn import_changed_playlist<D: DatabaseManager + ?Sized>(
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
        database::playlist_formats::PlaylistFileManager::import_radio_playlist(database, path).await
    } else {
        database.import_playlist_file(path, None).await.map(|_| ())
    }
}

pub(in crate::lifecycle) async fn handle_file_system_event<D: DatabaseManager + 'static>(
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
            if is_srt_path(&path) {
                update_subtitle_index(&path, true, app_state).await?;
                return Ok(());
            }
            // Check if this is a directory or a file
            if path.is_dir() {
                if !policy.recursive || path == policy.root {
                    return Ok(());
                }
                info!("Directory created: {}", path.display());

                // Scan the new directory for media files using bulk operations
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
                        stats.record_files_processed(scan_result.new as u64);

                        info!("Added {} media files from new directory using bulk operations: {}",
                              scan_result.new, path.display());

                        // Increment update ID to notify DLNA clients
                        if scan_result.new > 0 {
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
                    import_changed_playlist(database.as_ref(), &path).await?;
                    increment_content_update_id(app_state).await;
                    return Ok(());
                }
                if !policy.allows_media(&path) {
                    debug!("Not a supported media file, ignoring: {}", path.display());
                    return Ok(());
                }

                index_media_file_path(database.as_ref(), &path).await?;

                // Record atomic statistics
                stats.record_files_processed(1);

                info!("Added new media file to database: {}", path.display());

                // Increment update ID to notify DLNA clients
                increment_content_update_id(app_state).await;
            }
        }

        FileSystemEvent::Modified(path) => {
            let policy = media::ScanPolicy::for_path(&policies, &path).cloned();
            let Some(policy) = policy else {
                return Ok(());
            };
            if is_srt_path(&path) {
                update_subtitle_index(&path, true, app_state).await?;
                return Ok(());
            }
            info!("Media file modified: {}", path.display());

            if policy.allows_playlist(&path) {
                import_changed_playlist(database.as_ref(), &path).await?;
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
            if let Some(mut existing_file) = database.get_file_by_path(&path).await? {
                let metadata = tokio::fs::metadata(&path).await?;
                existing_file.size = metadata.len();
                existing_file.modified =
                    metadata.modified().unwrap_or(std::time::SystemTime::now());
                existing_file.updated_at = std::time::SystemTime::now();

                // Use bulk update operation (single-item batch for atomic consistency)
                database.bulk_update_media_files(&[existing_file]).await?;

                // Record atomic statistics
                stats.record_files_processed(1);

                info!("Updated media file in database: {}", path.display());

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
                    update_subtitle_index(&to, true, app_state).await?;
                }
                return Ok(());
            }
            info!("Path renamed: {} -> {}", from.display(), to.display());

            // Check if the destination is a directory or file
            if to.is_dir() {
                let Some(policy) = media::ScanPolicy::for_path(&policies, &to).cloned() else {
                    return Ok(());
                };
                if !policy.recursive || to == policy.root {
                    return Ok(());
                }
                // Handle directory rename using bulk operations
                info!("Directory renamed: {} -> {}", from.display(), to.display());

                // Use efficient path prefix query to find files in the old directory path
                let path_normalizer = create_platform_path_normalizer();
                let canonical_from_prefix = path_normalizer.to_canonical(&from)?;
                let files_in_old_path = database
                    .get_files_with_path_prefix(&canonical_from_prefix)
                    .await?;

                if !files_in_old_path.is_empty() {
                    info!(
                        "Updating {} media files for renamed directory using bulk operations",
                        files_in_old_path.len()
                    );

                    // Collect paths for bulk removal
                    let old_paths: Vec<PathBuf> =
                        files_in_old_path.iter().map(|f| f.path.clone()).collect();

                    // Remove old files from database using bulk operation
                    let removed_count = database.bulk_remove_media_files(&old_paths).await?;
                    info!(
                        "bulk removal: {} files removed for renamed directory",
                        removed_count
                    );

                    // Scan the new directory location using bulk operations
                    let scanner = media::MediaScanner::with_database(database.clone());
                    match scanner
                        .scan_directory_recursive_with_policy(&policy.for_subtree(&to))
                        .await
                    {
                        Ok(scan_result) => {
                            info!(
                                "Rescanned renamed directory {}: {}",
                                to.display(),
                                scan_result.summary()
                            );

                            // Files are already stored in database by the scanner using bulk operations

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

                let from_playlist = media::ScanPolicy::for_path(&policies, &from)
                    .is_some_and(|policy| policy.allows_playlist(&from));
                let to_playlist = media::ScanPolicy::for_path(&policies, &to)
                    .is_some_and(|policy| policy.allows_playlist(&to));
                if from_playlist || to_playlist {
                    if from_playlist {
                        database.remove_derived_content_by_source(&from).await?;
                    }
                    if to_playlist && to.is_file() {
                        import_changed_playlist(database.as_ref(), &to).await?;
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
                        index_media_file_path(database.as_ref(), &to).await?;
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

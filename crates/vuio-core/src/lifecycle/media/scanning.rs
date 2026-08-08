use super::super::*;

/// Validate cached files and remove any that no longer exist on disk
///
/// Uses two-phase approach to avoid RwLock deadlock:
/// 1. Stream all files and collect paths to delete (read lock)
/// 2. Drop stream, then bulk delete (write lock)
pub(in crate::lifecycle) async fn validate_and_cleanup_deleted_files<D: DatabaseManager>(
    database: Arc<D>,
    monitored_roots: &[PathBuf],
) -> anyhow::Result<usize> {
    info!("Validating cached media files...");

    // Phase 1: Collect paths to delete (holds read lock)
    let mut paths_to_delete = Vec::new();
    let mut total_checked = 0;
    let unavailable_roots = database
        .list_root_availability()
        .await?
        .into_iter()
        .filter(|root| root.unavailable_since_secs.is_some())
        .map(|root| root.path)
        .collect::<Vec<_>>();

    {
        for media_file in database.load_file_fingerprints().await? {
            total_checked += 1;

            let unavailable_root = monitored_roots
                .iter()
                .any(|root| media_file.path.starts_with(root) && !root.is_dir())
                || unavailable_roots
                    .iter()
                    .any(|root| media_file.path.starts_with(root));
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

pub(in crate::lifecycle) fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(in crate::lifecycle) async fn reconcile_unavailable_media_roots<D: DatabaseManager>(
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
                indexed_count: previous.as_ref().map_or(indexed_count, |state| {
                    state.indexed_count.max(indexed_count)
                }),
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

pub(in crate::lifecycle) async fn record_root_scan<D: DatabaseManager>(
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
pub(in crate::lifecycle) async fn perform_initial_media_scan<D: DatabaseManager + 'static>(
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
pub(in crate::lifecycle) async fn perform_initial_playlist_scan<D: DatabaseManager + 'static>(
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

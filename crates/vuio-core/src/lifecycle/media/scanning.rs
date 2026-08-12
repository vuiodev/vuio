use super::super::*;

/// A library root in the form the index stores paths in.
///
/// Falls back to the path as written when it cannot be resolved — the caller treats a
/// match against either form as belonging to the library, so a failure here can only
/// make the comparison more conservative, never less.
fn canonical_root(path: &Path) -> PathBuf {
    crate::platform::filesystem::create_platform_path_normalizer()
        .to_canonical(path)
        .map(PathBuf::from)
        .unwrap_or_else(|_| path.to_path_buf())
}

/// Validate cached files and remove the ones that no longer belong in the index:
/// files deleted from disk, and files left behind by a library that is no longer
/// configured.
///
/// `enabled` is `media.cleanup_deleted_files`, and it is taken as an argument rather than
/// checked by each caller so that "do not remove anything from the index automatically"
/// means exactly that. It used to be honoured by the startup scan and ignored by the
/// periodic pass, which removed the same files five minutes later.
///
/// Uses two-phase approach to avoid RwLock deadlock:
/// 1. Stream all files and collect paths to delete (read lock)
/// 2. Drop stream, then bulk delete (write lock)
pub(in crate::lifecycle) async fn validate_and_cleanup_deleted_files<D: DatabaseManager>(
    database: Arc<D>,
    monitored_roots: &[PathBuf],
    enabled: bool,
) -> anyhow::Result<usize> {
    if !enabled {
        debug!("Skipping index cleanup: cleanup_deleted_files is off");
        return Ok(0);
    }

    info!("Validating cached media files...");

    // Phase 1: Collect paths to delete (holds read lock)
    let mut paths_to_delete = Vec::new();
    let mut orphaned_count = 0_usize;
    let mut total_checked = 0;
    let availability = database.list_root_availability().await?;
    let unavailable_roots = availability
        .iter()
        .filter(|root| root.unavailable_since_secs.is_some())
        .map(|root| root.path.clone())
        .collect::<Vec<_>>();

    // Removing a root through the config watcher drops its content, but a root that
    // simply is not in the config at startup never got that treatment: its files stay
    // indexed forever, because the only other check asks whether they still exist on
    // disk, and they do. They then show up in Browse with no folder structure, since
    // nothing can work out a path relative to a library that is not configured.
    //
    // Guarded on a non-empty root list: an empty one would mean discarding the whole
    // index, and validation should have rejected that configuration long before here.
    let prune_orphans = !monitored_roots.is_empty();
    if !prune_orphans {
        warn!("No media libraries are configured; leaving the existing index alone");
    }

    // Paths enter the index canonicalised — symlinks resolved on Unix, lower-cased on
    // Windows — so a root as written in the config often will not prefix-match them.
    // A library at /tmp/media is stored under /private/tmp/media on macOS. Matching
    // against both forms keeps that from reading as "belongs to no library", which for
    // a deletion pass would mean discarding the entire library's index.
    let canonical_roots = monitored_roots
        .iter()
        .map(|root| canonical_root(root))
        .collect::<Vec<_>>();
    let owning_root = |path: &Path| {
        monitored_roots
            .iter()
            .chain(canonical_roots.iter())
            .find(|root| path.starts_with(root))
            .cloned()
    };

    {
        for media_file in database.load_file_fingerprints().await? {
            total_checked += 1;

            let Some(configured_root) = owning_root(&media_file.path) else {
                if prune_orphans {
                    orphaned_count += 1;
                    paths_to_delete.push(media_file.path.clone());
                }
                continue;
            };

            // A configured library that is offline keeps its content: the files are
            // unreachable rather than gone. That grace only applies to a library the
            // configuration still lists, which is why it is checked after the above.
            let unavailable_root = !configured_root.is_dir()
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

    if orphaned_count > 0 {
        info!(
            "Removing {} indexed files from libraries that are no longer configured",
            orphaned_count
        );
        // Forget the roots themselves too, so their derived content and availability
        // records do not outlive the files.
        for root in availability
            .iter()
            .map(|root| &root.path)
            .filter(|root| !monitored_roots.iter().any(|kept| root.starts_with(kept)))
        {
            if let Err(error) = database.remove_derived_content_by_source(root).await {
                warn!(
                    "Failed to remove derived content for unconfigured library {}: {}",
                    root.display(),
                    error
                );
            }
            if let Err(error) = database.remove_root_availability(root).await {
                warn!(
                    "Failed to forget unconfigured library {}: {}",
                    root.display(),
                    error
                );
            }
        }
    }

    // Phase 2: Bulk delete (acquires write lock)
    let removed_count = paths_to_delete.len();
    if !paths_to_delete.is_empty() {
        database.bulk_remove_media_files(&paths_to_delete).await?;
    }

    if removed_count > 0 {
        info!(
            "Cleaned up {} files from the index — {} deleted from disk, {} from libraries \
             no longer configured (checked {} total)",
            removed_count,
            removed_count - orphaned_count,
            orphaned_count,
            total_checked
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
        let roots: Vec<_> = config
            .media
            .directories
            .iter()
            .map(|d| PathBuf::from(&d.path))
            .collect();
        validate_and_cleanup_deleted_files(
            database.clone(),
            &roots,
            config.media.cleanup_deleted_files,
        )
        .await?;

        Ok(())
    } else {
        info!("Skipping full scan (scan on startup disabled)");

        // Validate that cached files still exist on disk and remove any that don't (if enabled)
        let roots: Vec<_> = config
            .media
            .directories
            .iter()
            .map(|d| PathBuf::from(&d.path))
            .collect();
        validate_and_cleanup_deleted_files(
            database.clone(),
            &roots,
            config.media.cleanup_deleted_files,
        )
        .await?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{redb::RedbDatabase, MediaFile, MediaRepository};

    async fn database_with(files: &[PathBuf]) -> (Arc<RedbDatabase>, tempfile::TempDir) {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let database = Arc::new(
            RedbDatabase::new(temp.path().join("test.redb"))
                .await
                .expect("database"),
        );
        for path in files {
            database
                .store_media_file(&MediaFile::new(path.clone(), 1024, "video/mp4".to_string()))
                .await
                .expect("store");
        }
        (database, temp)
    }

    async fn indexed_paths(database: &Arc<RedbDatabase>) -> Vec<PathBuf> {
        let mut paths = database
            .load_file_fingerprints()
            .await
            .expect("fingerprints")
            .into_iter()
            .map(|file| file.path)
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    /// Removing a library through the config watcher drops its content, but a library
    /// that is simply absent at startup never got that treatment. Its files stayed
    /// indexed forever — they still exist on disk, which was the only question asked —
    /// and showed up in Browse with no folder structure, because nothing can express
    /// their path relative to a library that is not configured.
    #[tokio::test]
    async fn files_from_an_unconfigured_library_are_dropped() {
        let kept_root = tempfile::TempDir::new().expect("kept");
        let gone_root = tempfile::TempDir::new().expect("gone");
        let kept_file = kept_root.path().join("keep.mp4");
        let orphan = gone_root.path().join("orphan.mp4");
        std::fs::write(&kept_file, b"x").expect("write");
        // Still present on disk: the point is that existing is no longer sufficient.
        std::fs::write(&orphan, b"x").expect("write");

        let (database, _temp) = database_with(&[kept_file.clone(), orphan.clone()]).await;
        let removed =
            validate_and_cleanup_deleted_files(database.clone(), &[kept_root.path().to_path_buf()], true)
                .await
                .expect("cleanup");

        assert_eq!(removed, 1);
        // The index stores canonical paths, which on macOS resolves /var to /private/var.
        assert_eq!(indexed_paths(&database).await, vec![canonical_root(&kept_file)]);
    }

    /// `cleanup_deleted_files = false` has to mean nothing is removed from the index
    /// automatically. The startup scan honoured it and the 300-second reconciliation tick
    /// did not, so switching it off delayed deletions by five minutes rather than
    /// preventing them — and once the orphan prune started running through the same
    /// ungated call, it removed whole libraries too.
    #[tokio::test]
    async fn cleanup_disabled_removes_nothing() {
        let root = tempfile::TempDir::new().expect("root");
        let gone_root = tempfile::TempDir::new().expect("gone");
        let present = root.path().join("here.mp4");
        let deleted = root.path().join("gone.mp4");
        let orphan = gone_root.path().join("orphan.mp4");
        std::fs::write(&present, b"x").expect("write");
        std::fs::write(&orphan, b"x").expect("write");

        let (database, _temp) =
            database_with(&[present.clone(), deleted.clone(), orphan.clone()]).await;
        let removed = validate_and_cleanup_deleted_files(
            database.clone(),
            &[root.path().to_path_buf()],
            false,
        )
        .await
        .expect("cleanup");

        assert_eq!(removed, 0);
        // Both the file missing from disk and the orphaned library survive.
        assert_eq!(indexed_paths(&database).await.len(), 3);

        // And with it on, the same call removes both.
        let removed = validate_and_cleanup_deleted_files(
            database.clone(),
            &[root.path().to_path_buf()],
            true,
        )
        .await
        .expect("cleanup");
        assert_eq!(removed, 2);
        assert_eq!(indexed_paths(&database).await, vec![canonical_root(&present)]);
    }

    /// A configured library that is offline is not the same as one that was removed:
    /// its files are unreachable, not gone, and must survive until the grace expires.
    #[tokio::test]
    async fn an_offline_library_keeps_its_content() {
        let root = tempfile::TempDir::new().expect("root");
        let file = root.path().join("on-the-nas.mp4");
        std::fs::write(&file, b"x").expect("write");
        let (database, _temp) = database_with(&[file.clone()]).await;

        // The library is still configured, but the path is gone — an unmounted volume.
        let offline = root.path().to_path_buf();
        let stored = canonical_root(&file);
        drop(root);

        let removed = validate_and_cleanup_deleted_files(database.clone(), &[offline], true)
            .await
            .expect("cleanup");

        assert_eq!(removed, 0, "an offline library must not lose its index");
        assert_eq!(indexed_paths(&database).await, vec![stored]);
    }

    /// Files deleted from a library that is still configured and online are still
    /// removed — the behaviour this function had before orphans were considered.
    #[tokio::test]
    async fn files_deleted_from_a_live_library_are_still_dropped() {
        let root = tempfile::TempDir::new().expect("root");
        let present = root.path().join("here.mp4");
        let deleted = root.path().join("gone.mp4");
        std::fs::write(&present, b"x").expect("write");

        let (database, _temp) = database_with(&[present.clone(), deleted]).await;
        let removed =
            validate_and_cleanup_deleted_files(database.clone(), &[root.path().to_path_buf()], true)
                .await
                .expect("cleanup");

        assert_eq!(removed, 1);
        assert_eq!(indexed_paths(&database).await, vec![canonical_root(&present)]);
    }

    /// Nothing configured means nothing to compare against. Treating every file as an
    /// orphan would discard the whole index over what is almost certainly a misload.
    #[tokio::test]
    async fn an_empty_library_list_never_prunes() {
        let root = tempfile::TempDir::new().expect("root");
        let file = root.path().join("keep.mp4");
        std::fs::write(&file, b"x").expect("write");

        let (database, _temp) = database_with(&[file.clone()]).await;
        let removed = validate_and_cleanup_deleted_files(database.clone(), &[], true)
            .await
            .expect("cleanup");

        assert_eq!(removed, 0);
        assert_eq!(indexed_paths(&database).await, vec![canonical_root(&file)]);
    }

    /// The index stores canonical paths, so a library reached through a symlink — or
    /// spelled with different case on Windows — does not prefix-match the root as the
    /// config writes it. Matching only the raw form would have read every one of its
    /// files as belonging to no library and deleted the entire index for it.
    #[tokio::test]
    async fn a_symlinked_library_is_not_mistaken_for_an_orphan() {
        let real = tempfile::TempDir::new().expect("real");
        let link_parent = tempfile::TempDir::new().expect("link parent");
        let link = link_parent.path().join("library");
        #[cfg(unix)]
        std::os::unix::fs::symlink(real.path(), &link).expect("symlink");
        #[cfg(not(unix))]
        return;

        let file = link.join("through-the-link.mp4");
        std::fs::write(&file, b"x").expect("write");

        let (database, _temp) = database_with(&[file.clone()]).await;
        // Configured by the symlinked path, stored under the resolved one.
        let removed = validate_and_cleanup_deleted_files(database.clone(), &[link.clone()], true)
            .await
            .expect("cleanup");

        assert_eq!(removed, 0, "a symlinked library must keep its index");
        assert_eq!(indexed_paths(&database).await.len(), 1);
    }
}

use super::super::*;

/// Start file system monitoring with database integration
pub(in crate::lifecycle) async fn start_file_monitoring<D: DatabaseManager + 'static>(
    watcher: Arc<CrossPlatformWatcher>,
    app_state: AppState<D>,
    cancellation: CancellationToken,
) -> anyhow::Result<Option<tokio::task::JoinHandle<()>>> {
    if !app_state.current_config().media.watch_for_changes {
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
    let mut event_receiver = watcher
        .take_event_receiver()
        .await
        .context("File-system event receiver was already consumed")?;

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
                        let configured_roots = app_state_clone
                            .media_directories
                            .read()
                            .await
                            .clone();
                        for root in &configured_roots {
                            let path = PathBuf::from(&root.path);
                            if path.is_dir() {
                                let scanner = media::MediaScanner::with_database(app_state_clone.database.clone());
                                let policy = media::ScanPolicy::from_config(&app_state_clone.current_config(), root);
                                let scan = if root.recursive {
                                    scanner.scan_directory_recursive_with_policy(&policy).await
                                } else {
                                    scanner.scan_directory_with_policy(&policy).await
                                };
                                if scan.is_ok() {
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
                        let policy = media::ScanPolicy::from_config(&app_state_clone.current_config(), root);
                        if !path.is_dir() {
                            continue;
                        }
                        let result = if root.recursive {
                            scanner.scan_directory_recursive_with_policy(&policy).await
                        } else {
                            scanner.scan_directory_with_policy(&policy).await
                        };
                        match result {
                            Ok(result) => {
                                if let Err(error) = record_root_scan(
                                    &app_state_clone.database,
                                    &path,
                                    &result,
                                )
                                .await
                                {
                                    error!("Failed to persist root scan state for {}: {}", path.display(), error);
                                }
                                if result.total_changes() > 0 {
                                    increment_content_update_id(&app_state_clone).await;
                                }
                            }
                            Err(error) => error!(
                                "Periodic media discovery failed for {}: {}",
                                path.display(),
                                error
                            ),
                        }
                    }
                    if let Err(error) = refresh_unavailable_roots(&app_state_clone).await {
                        error!("Failed to refresh unavailable-root visibility: {}", error);
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

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
    for file in app_state.database.get_files_in_directory(parent).await? {
        if file.path.file_stem() == subtitle_stem && file.subtitle_available != available {
            if let Some(id) = file.id {
                changed.push(id);
            }
        }
    }
    if changed.is_empty() {
        return Ok(false);
    }
    // One column, by id. Writing the whole record back would have taken the
    // empty `extra_tags` of a record read from the database for the truth and
    // deleted every tag the file really has — over a sidecar appearing, which
    // says nothing about the media file at all.
    app_state
        .database
        .set_subtitle_available(&changed, available)
        .await?;
    increment_content_update_id(app_state).await;
    Ok(true)
}

/// Upsert a supported media path, read the way the scanner reads one.
///
/// Through the scanner rather than by hand: a record built from `stat` alone
/// has no tags, no duration, no codec and no subtitle flag, and it commits a
/// fingerprint at the current `tags_version` — so the next scan sees a record
/// that has already been read and leaves the gaps in place. `keep` carries the
/// identifier of a record this one replaces, which is what makes a rename an
/// update of that row rather than a delete and a fresh insert; the row's
/// playlist entries and scraped metadata hang off the identifier.
pub(in crate::lifecycle) async fn index_media_file_path<D: DatabaseManager + 'static>(
    database: &Arc<D>,
    path: &Path,
    keep: Option<i64>,
) -> anyhow::Result<i64> {
    let scanner = media::MediaScanner::with_database(database.clone());
    let mut media_file = scanner.create_media_file_from_path(path).await?;
    media_file.id = keep;

    database
        .bulk_store_media_files(&[media_file])
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("media upsert returned no ID for {}", path.display()))
}

/// Rewrite `path`'s `from` prefix as `to`, at a component boundary.
///
/// `None` when `path` is not under `from`, which for a directory rename means
/// a record the prefix query returned for a reason this cannot account for —
/// better left where it is than moved somewhere invented.
fn repoint(path: &Path, from: &Path, to: &Path) -> Option<PathBuf> {
    let relative = path.strip_prefix(from).ok()?;
    Some(to.join(relative))
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

                index_media_file_path(database, &path, None).await?;

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
            if let Some(existing_file) = database.get_file_by_path(&path).await? {
                // Re-read the file, rather than copying its new size and mtime
                // onto the old record. Committing a fresh fingerprint beside
                // stale tags is the worst of both: the next scan compares that
                // fingerprint against the file, finds them in step, and never
                // looks again — so re-tagged music and a re-encoded film kept
                // their old artist, duration and codec for good. Keeping the
                // identifier makes this an update of the existing row.
                index_media_file_path(database, &path, existing_file.id).await?;

                // Record atomic statistics
                stats.record_files_processed(1);

                info!("Updated media file in database: {}", path.display());

                // Increment update ID to notify DLNA clients
                increment_content_update_id(app_state).await;
            } else if path.is_file() {
                index_media_file_path(database, &path, None).await?;
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
                let canonical_to_prefix = path_normalizer.to_canonical(&to)?;
                let files_in_old_path = database
                    .get_files_with_path_prefix(&canonical_from_prefix)
                    .await?;

                if !files_in_old_path.is_empty() {
                    info!(
                        "Updating {} media files for renamed directory using bulk operations",
                        files_in_old_path.len()
                    );

                    // Move the rows to their new paths, keeping their
                    // identifiers. Removing them and letting the rescan insert
                    // fresh rows would give every file a new identifier, and
                    // playlist entries and scraped metadata cascade off that —
                    // renaming a folder used to empty the playlists that
                    // referenced it. The rescan below then matches these rows
                    // by their new paths and refreshes them in place.
                    let moves: Vec<(i64, PathBuf)> = files_in_old_path
                        .iter()
                        .filter_map(|file| {
                            let id = file.id?;
                            // Both sides canonical: the stored path is, and
                            // the event's is not — /var against /private/var
                            // would strip nothing at all.
                            Some((
                                id,
                                repoint(
                                    &file.path,
                                    Path::new(&canonical_from_prefix),
                                    Path::new(&canonical_to_prefix),
                                )?,
                            ))
                        })
                        .collect();
                    let moved = database.relocate_media_files(&moves).await?;
                    info!("Moved {moved} indexed files with the renamed directory");

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
                        index_media_file_path(database, &to, None).await?;
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
                        // Move the existing row rather than deleting it and
                        // inserting another. Playlist entries and scraped
                        // metadata hang off the identifier by foreign key, so a
                        // delete cascades them away — an ordinary rename in a
                        // file manager used to cost an album's playlist
                        // membership and everything a scraper had found.
                        let keep = database
                            .get_file_by_path(&from)
                            .await?
                            .and_then(|existing| existing.id);
                        if keep.is_none() {
                            debug!(
                                "Media rename source was absent; destination will still be indexed: {}",
                                from.display()
                            );
                        }
                        index_media_file_path(database, &to, keep).await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, MonitoredDirectoryConfig, ValidationMode};
    use crate::database::sqlite::SqliteDatabase;
    use crate::database::{MediaRepository, PlaylistRepository};
    use crate::watcher::FileSystemEvent;

    /// A server watching `root`, over a fresh database.
    async fn state_watching(root: &Path) -> AppState<SqliteDatabase> {
        let database = Arc::new(
            SqliteDatabase::new(root.join(".library.db"))
                .await
                .expect("database"),
        );
        database.initialize().await.expect("schema");

        let mut config = AppConfig::default();
        // AIFF is not in the default extension list, and it is what the fixtures
        // below use: it is the one container these tests can write a real tag
        // into, since the reader in the server only reads.
        config.media.supported_extensions.push("aiff".to_owned());
        config.media.directories = vec![MonitoredDirectoryConfig {
            path: root.to_string_lossy().into_owned(),
            recursive: true,
            case_sensitive: None,
            extensions: None,
            exclude_patterns: None,
            validation_mode: ValidationMode::Skip,
        }];
        let config = Arc::new(config);

        AppState {
            media_directories: Arc::new(tokio::sync::RwLock::new(
                config.media.directories.clone(),
            )),
            unavailable_roots: Arc::new(tokio::sync::RwLock::new(
                std::collections::HashSet::new(),
            )),
            config: config.clone(),
            config_source: Arc::new(Default::default()),
            http_binding: Arc::new(crate::state::HttpBinding::new(8080)),
            live_config: Arc::new(crate::state::LiveConfig::new(config)),
            database,
            auth: Arc::new(crate::web::auth::AuthState::testing()),
            platform_info: Arc::new(
                crate::platform::PlatformInfo::detect().await.expect("platform"),
            ),
            filesystem_manager: Arc::from(
                crate::platform::filesystem::create_platform_filesystem_manager(),
            ),
            content_update_id: Arc::new(std::sync::atomic::AtomicU32::new(1)),
            web_metrics: Arc::new(crate::web::diagnostics::WebHandlerMetrics::new()),
            runtime_diagnostics: Arc::new(
                crate::platform::diagnostics::SystemDiagnosticsSampler::new(),
            ),
            lifecycle_stats: Arc::new(ApplicationStats::new()),
            bookmarks: Arc::new(tokio::sync::Mutex::new(
                crate::runtime_state::BookmarkRegistry::new(
                    crate::runtime_state::BOOKMARK_MAX_ENTRIES,
                ),
            )),
            log_file_path: root.join("vuio.log"),
            browse_cache: Arc::new(tokio::sync::Mutex::new(
                crate::runtime_state::BrowseResponseCache::new(),
            )),
            active_monitors: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            active_casts: Arc::new(tokio::sync::Mutex::new(
                crate::runtime_state::ActiveCastRegistry::new(),
            )),
            #[cfg(feature = "mediainfo")]
            mediainfo_job: Arc::new(tokio::sync::Mutex::new(Default::default())),
            #[cfg(feature = "casting")]
            discovered_tvs: Arc::new(crate::runtime_state::RendererCache::new()),
            upnp_subscriptions: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            radio: Arc::new(crate::radio::RadioManager::new()),
            #[cfg(feature = "transcode")]
            transcode: Arc::new(Default::default()),
            cancellation: CancellationToken::new(),
            background_tasks: tokio_util::task::TaskTracker::new(),
        }
    }

    /// An AIFF carrying an ID3v2.3 tag: a title, one tag with no column of its
    /// own, and `frames` samples at 44.1 kHz.
    ///
    /// Built by hand because the reader only reads — and because the point is a
    /// file whose embedded metadata changes between two watcher events, which
    /// two separate fixtures would not isolate.
    fn tagged_audio(title: &str, mood: &str, frames: u32) -> Vec<u8> {
        fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
            let mut out = Vec::with_capacity(8 + payload.len() + 1);
            out.extend_from_slice(id);
            out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            out.extend_from_slice(payload);
            if payload.len() % 2 == 1 {
                out.push(0); // IFF chunks are word aligned
            }
            out
        }

        let mut body_tag = Vec::new();
        for (id, text) in [(b"TIT2", title), (b"TMOO", mood)] {
            let mut payload = vec![0u8]; // ISO-8859-1 encoding marker
            payload.extend_from_slice(text.as_bytes());
            body_tag.extend_from_slice(id);
            // v2.3 frame sizes are plain big-endian, unlike the synchsafe header.
            body_tag.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            body_tag.extend_from_slice(&[0, 0]); // flags
            body_tag.extend_from_slice(&payload);
        }
        let size = body_tag.len();
        let mut tag = b"ID3".to_vec();
        tag.extend_from_slice(&[3, 0, 0]); // version 2.3, no flags
        tag.extend_from_slice(&[
            ((size >> 21) & 0x7f) as u8,
            ((size >> 14) & 0x7f) as u8,
            ((size >> 7) & 0x7f) as u8,
            (size & 0x7f) as u8,
        ]);
        tag.extend_from_slice(&body_tag);

        // COMM: channels, frame count, bits per sample, then a 10-byte extended
        // float sample rate. 0x400EAC44… is 44100 Hz.
        let mut comm = Vec::new();
        comm.extend_from_slice(&2i16.to_be_bytes());
        comm.extend_from_slice(&frames.to_be_bytes());
        comm.extend_from_slice(&16i16.to_be_bytes());
        comm.extend_from_slice(&[0x40, 0x0e, 0xac, 0x44, 0, 0, 0, 0, 0, 0]);

        // Real silence, as many frames as COMM claims: the reader takes the
        // duration from what the sound chunk actually holds.
        let mut ssnd = vec![0u8; 8]; // offset and block size
        ssnd.extend(std::iter::repeat_n(0u8, frames as usize * 2 * 2));

        let mut body = b"AIFF".to_vec();
        body.extend(chunk(b"COMM", &comm));
        body.extend(chunk(b"ID3 ", &tag));
        body.extend(chunk(b"SSND", &ssnd));

        let mut file = b"FORM".to_vec();
        file.extend_from_slice(&(body.len() as u32).to_be_bytes());
        file.extend_from_slice(&body);
        file
    }

    /// A file the watcher sees appear has to arrive with everything the scanner
    /// would have given it. Built from `stat` alone it had no title, duration or
    /// subtitle flag — and it committed a fingerprint at the current
    /// `tags_version`, so the next scan saw a record already in step with its
    /// file and never looked inside it.
    // Asserts on what the tag reader found, so it belongs to that feature.
    #[cfg(feature = "metadata")]
    #[tokio::test]
    async fn a_created_file_is_indexed_with_its_metadata() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let root = std::fs::canonicalize(temp.path()).expect("canonical root");
        let state = state_watching(&root).await;

        let track = root.join("song.aiff");
        std::fs::write(&track, tagged_audio("Sixteen Tons", "wry", 44_100)).expect("write track");
        std::fs::write(track.with_extension("srt"), b"1\n").expect("write subtitle");

        handle_file_system_event(FileSystemEvent::Created(track.clone()), &state)
            .await
            .expect("created");

        let indexed = state
            .database
            .get_file_by_path(&track)
            .await
            .expect("query")
            .expect("indexed");
        assert_eq!(indexed.title.as_deref(), Some("Sixteen Tons"));
        assert_eq!(indexed.duration.map(|d| d.as_secs()), Some(1));
        assert!(
            indexed.subtitle_available,
            "the sidecar beside it must be recorded"
        );
        let tags = state
            .database
            .get_media_tags(indexed.id.expect("id"))
            .await
            .expect("tags");
        assert!(
            tags.iter().any(|(key, value)| key == "Mood" && value == "wry"),
            "extra tags belong to the record too, got {tags:?}"
        );
    }

    /// Re-tagging a file has to reach the index. Copying the new size and mtime
    /// onto the old record committed a fresh fingerprint beside stale tags, and
    /// a later scan compared that fingerprint against the file, found them in
    /// step, and left the old title and duration in place for good.
    // Asserts on what the tag reader found, so it belongs to that feature.
    #[cfg(feature = "metadata")]
    #[tokio::test]
    async fn a_modified_file_has_its_metadata_re_read() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let root = std::fs::canonicalize(temp.path()).expect("canonical root");
        let state = state_watching(&root).await;

        let track = root.join("song.aiff");
        std::fs::write(&track, tagged_audio("Before", "wry", 44_100)).expect("write track");
        handle_file_system_event(FileSystemEvent::Created(track.clone()), &state)
            .await
            .expect("created");

        // Re-tagged, and a different length so the record is unambiguously the
        // new file rather than the old one.
        std::fs::write(&track, tagged_audio("After", "brisk", 88_200)).expect("retag track");
        handle_file_system_event(FileSystemEvent::Modified(track.clone()), &state)
            .await
            .expect("modified");

        let indexed = state
            .database
            .get_file_by_path(&track)
            .await
            .expect("query")
            .expect("indexed");
        assert_eq!(indexed.title.as_deref(), Some("After"));
        assert_eq!(indexed.duration.map(|d| d.as_secs()), Some(2));
        let tags = state
            .database
            .get_media_tags(indexed.id.expect("id"))
            .await
            .expect("tags");
        assert!(
            tags.iter().any(|(key, value)| key == "Mood" && value == "brisk"),
            "the new tags replace the old ones, got {tags:?}"
        );
    }

    /// A subtitle appearing says nothing about the media file, so it must not
    /// rewrite the record. It used to: the flag was flipped on a `MediaFile`
    /// read back from the database, whose `extra_tags` are never joined in, and
    /// writing that back deleted every tag the file really had.
    // Asserts on what the tag reader found, so it belongs to that feature.
    #[cfg(feature = "metadata")]
    #[tokio::test]
    async fn a_subtitle_appearing_keeps_the_media_file_s_tags() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let root = std::fs::canonicalize(temp.path()).expect("canonical root");
        let state = state_watching(&root).await;

        let track = root.join("song.aiff");
        std::fs::write(&track, tagged_audio("Sixteen Tons", "wry", 44_100)).expect("write track");
        handle_file_system_event(FileSystemEvent::Created(track.clone()), &state)
            .await
            .expect("created");
        let id = state
            .database
            .get_file_by_path(&track)
            .await
            .expect("query")
            .expect("indexed")
            .id
            .expect("id");

        let subtitle = track.with_extension("srt");
        std::fs::write(&subtitle, b"1\n").expect("write subtitle");
        handle_file_system_event(FileSystemEvent::Created(subtitle.clone()), &state)
            .await
            .expect("subtitle created");

        let indexed = state
            .database
            .get_file_by_path(&track)
            .await
            .expect("query")
            .expect("indexed");
        assert!(indexed.subtitle_available);
        assert_eq!(indexed.id, Some(id), "the record itself does not move");
        let tags = state
            .database
            .get_media_tags(id)
            .await
            .expect("tags");
        assert!(
            tags.iter().any(|(key, value)| key == "Mood" && value == "wry"),
            "a sidecar must not take the file's tags with it, got {tags:?}"
        );

        std::fs::remove_file(&subtitle).expect("remove subtitle");
        handle_file_system_event(
            FileSystemEvent::Deleted {
                path: subtitle,
                is_directory: Some(false),
            },
            &state,
        )
        .await
        .expect("subtitle deleted");

        let indexed = state
            .database
            .get_file_by_path(&track)
            .await
            .expect("query")
            .expect("indexed");
        assert!(!indexed.subtitle_available);
        let tags = state.database.get_media_tags(id).await.expect("tags");
        assert!(
            tags.iter().any(|(key, value)| key == "Mood" && value == "wry"),
            "nor must its removal, got {tags:?}"
        );
    }

    /// Renaming a file keeps its identifier, so everything that references it
    /// by identifier survives. It used to be a delete and an insert, and
    /// `playlist_entries` and `mediainfo` reference media files with
    /// `ON DELETE CASCADE` — so renaming a track in a file manager silently
    /// emptied the playlists it was in.
    #[tokio::test]
    async fn renaming_a_file_keeps_its_playlist_membership() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let root = std::fs::canonicalize(temp.path()).expect("canonical root");
        let state = state_watching(&root).await;

        let before = root.join("song.aiff");
        std::fs::write(&before, tagged_audio("Sixteen Tons", "wry", 44_100)).expect("write track");
        handle_file_system_event(FileSystemEvent::Created(before.clone()), &state)
            .await
            .expect("created");
        let id = state
            .database
            .get_file_by_path(&before)
            .await
            .expect("query")
            .expect("indexed")
            .id
            .expect("id");

        let playlist = state
            .database
            .create_playlist("Favourites", None)
            .await
            .expect("playlist");
        state
            .database
            .add_to_playlist(playlist, id, None)
            .await
            .expect("add to playlist");

        let after = root.join("renamed.aiff");
        std::fs::rename(&before, &after).expect("rename");
        handle_file_system_event(
            FileSystemEvent::Renamed {
                from: before.clone(),
                to: after.clone(),
            },
            &state,
        )
        .await
        .expect("renamed");

        let indexed = state
            .database
            .get_file_by_path(&after)
            .await
            .expect("query")
            .expect("indexed");
        assert_eq!(indexed.id, Some(id), "a rename moves the row, it does not replace it");
        assert!(state
            .database
            .get_file_by_path(&before)
            .await
            .expect("query")
            .is_none());
        let entries = state
            .database
            .get_playlist_tracks(playlist)
            .await
            .expect("playlist files");
        assert_eq!(
            entries.iter().filter_map(|file| file.id).collect::<Vec<_>>(),
            vec![id],
            "the playlist still holds the renamed track"
        );
    }

    /// Replacing an existing destination is still a rename of the source. The
    /// destination row describes bytes the filesystem just removed, so it must
    /// not win merely because path lookup happens before identifier lookup.
    #[tokio::test]
    async fn renaming_over_an_indexed_file_keeps_the_source_identity() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let root = std::fs::canonicalize(temp.path()).expect("canonical root");
        let state = state_watching(&root).await;

        let source = root.join("source.aiff");
        let destination = root.join("destination.aiff");
        std::fs::write(&source, tagged_audio("Source", "kept", 44_100)).expect("source");
        std::fs::write(
            &destination,
            tagged_audio("Destination", "displaced", 44_100),
        )
        .expect("destination");
        handle_file_system_event(FileSystemEvent::Created(source.clone()), &state)
            .await
            .expect("index source");
        handle_file_system_event(FileSystemEvent::Created(destination.clone()), &state)
            .await
            .expect("index destination");

        let source_id = state
            .database
            .get_file_by_path(&source)
            .await
            .unwrap()
            .unwrap()
            .id
            .unwrap();
        let destination_id = state
            .database
            .get_file_by_path(&destination)
            .await
            .unwrap()
            .unwrap()
            .id
            .unwrap();
        let playlist = state
            .database
            .create_playlist("Source list", None)
            .await
            .unwrap();
        state
            .database
            .add_to_playlist(playlist, source_id, None)
            .await
            .unwrap();

        // Removing first makes overwrite-rename semantics portable to Windows
        // while deliberately leaving the old destination row in the database,
        // exactly as it is when the watcher handles the combined rename event.
        std::fs::remove_file(&destination).expect("remove destination bytes");
        std::fs::rename(&source, &destination).expect("replace destination");
        handle_file_system_event(
            FileSystemEvent::Renamed {
                from: source.clone(),
                to: destination.clone(),
            },
            &state,
        )
        .await
        .expect("rename event");

        let indexed = state
            .database
            .get_file_by_path(&destination)
            .await
            .unwrap()
            .expect("destination remains indexed");
        assert_eq!(indexed.id, Some(source_id));
        assert!(state.database.get_file_by_path(&source).await.unwrap().is_none());
        assert!(state
            .database
            .get_file_location_by_id(destination_id)
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            state
                .database
                .get_playlist_tracks(playlist)
                .await
                .unwrap()
                .iter()
                .filter_map(|file| file.id)
                .collect::<Vec<_>>(),
            vec![source_id]
        );
    }

    /// The same for a whole folder: every row moves with it, keeping its
    /// identifier and everything hanging off it.
    #[tokio::test]
    async fn renaming_a_folder_keeps_its_files_playlist_membership() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let root = std::fs::canonicalize(temp.path()).expect("canonical root");
        let state = state_watching(&root).await;

        let before = root.join("album");
        std::fs::create_dir_all(before.join("disc 1")).expect("album dir");
        let track = before.join("disc 1").join("song.aiff");
        std::fs::write(&track, tagged_audio("Sixteen Tons", "wry", 44_100)).expect("write track");
        handle_file_system_event(FileSystemEvent::Created(track.clone()), &state)
            .await
            .expect("created");
        let id = state
            .database
            .get_file_by_path(&track)
            .await
            .expect("query")
            .expect("indexed")
            .id
            .expect("id");

        let playlist = state
            .database
            .create_playlist("Favourites", None)
            .await
            .expect("playlist");
        state
            .database
            .add_to_playlist(playlist, id, None)
            .await
            .expect("add to playlist");

        let after = root.join("album (remastered)");
        std::fs::rename(&before, &after).expect("rename");
        handle_file_system_event(
            FileSystemEvent::Renamed {
                from: before.clone(),
                to: after.clone(),
            },
            &state,
        )
        .await
        .expect("renamed");

        let moved = after.join("disc 1").join("song.aiff");
        let indexed = state
            .database
            .get_file_by_path(&moved)
            .await
            .expect("query")
            .expect("indexed");
        assert_eq!(indexed.id, Some(id));
        assert!(state
            .database
            .get_file_by_path(&track)
            .await
            .expect("query")
            .is_none());
        let entries = state
            .database
            .get_playlist_tracks(playlist)
            .await
            .expect("playlist files");
        assert_eq!(
            entries.iter().filter_map(|file| file.id).collect::<Vec<_>>(),
            vec![id],
            "the playlist still holds the track under its new folder"
        );
    }
}

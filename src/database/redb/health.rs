//! Database integrity, index rebuilding, backup, restore, and maintenance.

use super::*;

impl RedbDatabase {
    pub(super) async fn check_and_repair_impl(&self) -> Result<DatabaseHealth> {
        self.rebuild_derived_indexes_impl().await
    }

    pub(super) async fn create_backup_impl(&self, backup_path: &Path) -> Result<()> {
        let _mutation_guard = self.mutation_lock.lock().await;
        let database = Arc::clone(&self.db);
        let destination = backup_path.to_path_buf();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let database_guard = database
                .read()
                .map_err(|_| anyhow!("ReDB handle lock is poisoned"))?;
            let parent = destination
                .parent()
                .ok_or_else(|| anyhow!("backup path has no parent"))?;
            std::fs::create_dir_all(parent)?;
            let temporary =
                destination.with_extension(format!("backup-{}.tmp", uuid::Uuid::new_v4()));
            Self::create_snapshot_file(&database_guard, &temporary)?;
            Self::validate_database_file(&temporary)?;
            let previous = destination
                .with_extension(format!("previous-backup-{}.redb", uuid::Uuid::new_v4()));
            let had_previous = destination.exists();
            if had_previous {
                std::fs::rename(&destination, &previous)?;
            }
            if let Err(error) = std::fs::rename(&temporary, &destination) {
                if had_previous {
                    let _ = std::fs::rename(&previous, &destination);
                }
                return Err(error.into());
            }
            if had_previous {
                std::fs::remove_file(previous)?;
            }
            Ok(())
        })
        .await
        .context("ReDB backup task failed")??;
        info!("Created database backup at {}", backup_path.display());
        Ok(())
    }

    fn validate_database_file(path: &Path) -> Result<()> {
        let database = redb::Database::builder().open_read_only(path)?;
        let transaction = database.begin_read()?;
        let metadata = transaction.open_table(METADATA_TABLE)?;
        let schema = metadata.get("schema_version")?.map(|value| value.value());
        let codec = metadata.get("codec_version")?.map(|value| value.value());
        if schema != Some(SCHEMA_VERSION) || codec != Some(CODEC_VERSION) {
            anyhow::bail!(
                "incompatible backup format: schema={schema:?}, codec={codec:?}, expected schema={SCHEMA_VERSION}, codec={CODEC_VERSION}"
            );
        }
        Ok(())
    }

    fn create_snapshot_file(source: &redb::Database, destination: &Path) -> Result<()> {
        if destination.exists() {
            std::fs::remove_file(destination)?;
        }
        let target = redb::Database::create(destination)?;
        let source_transaction = source.begin_read()?;
        let target_transaction = target.begin_write()?;

        macro_rules! copy_table {
            ($definition:expr) => {{
                let source_table = source_transaction.open_table($definition)?;
                let mut target_table = target_transaction.open_table($definition)?;
                for entry in source_table.iter()? {
                    let (key, value) = entry?;
                    target_table.insert(key.value(), value.value())?;
                }
            }};
        }
        macro_rules! copy_multimap {
            ($definition:expr) => {{
                let source_table = source_transaction.open_multimap_table($definition)?;
                let mut target_table = target_transaction.open_multimap_table($definition)?;
                for entry in source_table.iter()? {
                    let (key, values) = entry?;
                    for value in values {
                        target_table.insert(key.value(), value?.value())?;
                    }
                }
            }};
        }

        copy_table!(FILES_TABLE);
        copy_table!(PATH_INDEX);
        copy_table!(DIRECTORY_PATH_INDEX);
        copy_table!(DIRECTORY_RECORDS);
        copy_multimap!(DIRECTORY_CHILDREN);
        copy_table!(DIRECTORY_CHILDREN_BY_NAME);
        copy_multimap!(DIRECTORY_FILES);
        copy_table!(DIRECTORY_MIME_COUNTS);
        copy_table!(PLAYLISTS_TABLE);
        copy_table!(PLAYLIST_ENTRIES);
        copy_multimap!(FILE_PLAYLIST_ENTRIES);
        copy_table!(PLAYLIST_SOURCES);
        copy_multimap!(SOURCE_PLAYLISTS);
        copy_table!(METADATA_TABLE);
        copy_table!(ROOT_AVAILABILITY);
        copy_multimap!(ARTIST_INDEX);
        copy_multimap!(ALBUM_INDEX);
        copy_multimap!(GENRE_INDEX);
        copy_multimap!(YEAR_INDEX);
        copy_multimap!(ALBUM_ARTIST_INDEX);

        target_transaction.commit()?;
        drop(source_transaction);
        drop(target);
        std::fs::File::open(destination)?.sync_all()?;
        Ok(())
    }

    /// Restore a validated backup before the active database is opened.
    pub async fn restore_backup_file(backup_path: PathBuf, database_path: PathBuf) -> Result<()> {
        tokio::task::spawn_blocking(move || -> Result<()> {
            Self::validate_database_file(&backup_path)?;
            if let Some(parent) = database_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let temporary =
                database_path.with_extension(format!("restore-{}.tmp", uuid::Uuid::new_v4()));
            std::fs::copy(&backup_path, &temporary)?;
            std::fs::File::open(&temporary)?.sync_all()?;
            Self::validate_database_file(&temporary)?;

            let previous =
                database_path.with_extension(format!("pre-restore-{}.redb", uuid::Uuid::new_v4()));
            let had_previous = database_path.exists();
            if had_previous {
                std::fs::rename(&database_path, &previous)?;
            }
            if let Err(error) = std::fs::rename(&temporary, &database_path) {
                if had_previous {
                    let _ = std::fs::rename(&previous, &database_path);
                }
                return Err(error.into());
            }
            if had_previous {
                std::fs::remove_file(previous)?;
            }
            Ok(())
        })
        .await
        .context("ReDB restore task failed")?
    }

    pub(super) async fn vacuum_impl(&self) -> Result<bool> {
        let _mutation_guard = self.mutation_lock.lock().await;
        let database = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let mut database = database
                .write()
                .map_err(|_| anyhow!("ReDB handle lock is poisoned"))?;
            database.compact().map_err(anyhow::Error::from)
        })
        .await
        .context("ReDB compaction task failed")?
    }

    pub(super) async fn rebuild_derived_indexes_impl(&self) -> Result<DatabaseHealth> {
        let next_directory_id = Arc::clone(&self.next_directory_id);
        let (health, total_files, total_size) = self
            .execute_write(move |database| {
                let txn = database.begin_write()?;
                let mut winners: HashMap<String, (i64, u64)> = HashMap::new();
                let mut remap = HashMap::new();
                {
                    let files = txn.open_table(FILES_TABLE)?;
                    for entry in files.iter()? {
                        let (id, bytes) = entry?;
                        let view = RedbReadSession::view(bytes.value())?;
                        let path = view.path().to_owned();
                        if let Some((old_id, old_updated_at)) = winners.get(&path) {
                            if (view.updated_at_secs(), id.value()) > (*old_updated_at, *old_id) {
                                remap.insert(*old_id, id.value());
                                winners.insert(path, (id.value(), view.updated_at_secs()));
                            } else {
                                remap.insert(id.value(), *old_id);
                            }
                        } else {
                            winners.insert(path, (id.value(), view.updated_at_secs()));
                        }
                    }
                }
                {
                    let mut files = txn.open_table(FILES_TABLE)?;
                    for key in remap.keys() {
                        files.remove(key)?;
                    }
                }
                macro_rules! clear_table_str {
                    ($def:expr) => {{
                        let mut table = txn.open_table($def)?;
                        let keys = table
                            .iter()?
                            .map(|entry| entry.map(|(key, _)| key.value().to_string()))
                            .collect::<std::result::Result<Vec<_>, _>>()?;
                        for key in keys {
                            table.remove(key.as_str())?;
                        }
                    }};
                }
                clear_table_str!(PATH_INDEX);
                clear_table_str!(DIRECTORY_PATH_INDEX);
                clear_table_str!(DIRECTORY_MIME_COUNTS);
                {
                    let mut table = txn.open_table(DIRECTORY_RECORDS)?;
                    let keys = table
                        .iter()?
                        .map(|entry| entry.map(|(key, _)| key.value()))
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    for key in keys {
                        table.remove(key)?;
                    }
                }
                macro_rules! clear_multimap_str {
                    ($def:expr) => {{
                        let mut table = txn.open_multimap_table($def)?;
                        let keys = table
                            .iter()?
                            .map(|entry| entry.map(|(key, _)| key.value().to_string()))
                            .collect::<std::result::Result<Vec<_>, _>>()?;
                        for key in keys {
                            let _ = table.remove_all(key.as_str())?;
                        }
                    }};
                }
                clear_multimap_str!(ARTIST_INDEX);
                clear_multimap_str!(ALBUM_INDEX);
                clear_multimap_str!(GENRE_INDEX);
                clear_multimap_str!(ALBUM_ARTIST_INDEX);
                macro_rules! clear_multimap_u64 {
                    ($definition:expr) => {{
                        let mut table = txn.open_multimap_table($definition)?;
                        let keys = table
                            .iter()?
                            .map(|entry| entry.map(|(key, _)| key.value()))
                            .collect::<std::result::Result<Vec<_>, _>>()?;
                        for key in keys {
                            let _ = table.remove_all(key)?;
                        }
                    }};
                }
                clear_multimap_u64!(DIRECTORY_CHILDREN);
                clear_multimap_u64!(DIRECTORY_FILES);
                clear_table_str!(DIRECTORY_CHILDREN_BY_NAME);
                {
                    let mut table = txn.open_multimap_table(YEAR_INDEX)?;
                    let keys = table
                        .iter()?
                        .map(|entry| entry.map(|(key, _)| key.value()))
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    for key in keys {
                        let _ = table.remove_all(key)?;
                    }
                }
                let total_size = {
                    let files = txn.open_table(FILES_TABLE)?;
                    let mut paths = txn.open_table(PATH_INDEX)?;
                    let mut directory_paths = txn.open_table(DIRECTORY_PATH_INDEX)?;
                    let mut directory_records = txn.open_table(DIRECTORY_RECORDS)?;
                    let mut directory_children = txn.open_multimap_table(DIRECTORY_CHILDREN)?;
                    let mut ordered_children = txn.open_table(DIRECTORY_CHILDREN_BY_NAME)?;
                    let mut directory_files = txn.open_multimap_table(DIRECTORY_FILES)?;
                    let mut directory_mime_counts = txn.open_table(DIRECTORY_MIME_COUNTS)?;
                    let mut artists = txn.open_multimap_table(ARTIST_INDEX)?;
                    let mut albums = txn.open_multimap_table(ALBUM_INDEX)?;
                    let mut genres = txn.open_multimap_table(GENRE_INDEX)?;
                    let mut years = txn.open_multimap_table(YEAR_INDEX)?;
                    let mut album_artists = txn.open_multimap_table(ALBUM_ARTIST_INDEX)?;
                    let mut total_size = 0_u64;
                    for entry in files.iter()? {
                        let (id, bytes) = entry?;
                        let view = RedbReadSession::view(bytes.value())?;
                        paths.insert(view.path(), id.value())?;
                        Self::add_directory_membership(
                            &mut directory_paths,
                            &mut directory_records,
                            &mut directory_children,
                            &mut ordered_children,
                            &mut directory_files,
                            &mut directory_mime_counts,
                            &next_directory_id,
                            &view,
                        )?;
                        Self::add_file_indexes(
                            &mut artists,
                            &mut albums,
                            &mut genres,
                            &mut years,
                            &mut album_artists,
                            id.value(),
                            &view,
                        )?;
                        total_size = total_size.saturating_add(view.size());
                    }
                    total_size
                };
                let legacy = {
                    let meta = txn.open_table(METADATA_TABLE)?;
                    let v = meta
                        .get("schema_version")?
                        .is_none_or(|version| version.value() != SCHEMA_VERSION);
                    v
                };
                if legacy {
                    {
                        let mut table = txn.open_table(PLAYLISTS_TABLE)?;
                        let keys = table
                            .iter()?
                            .map(|entry| entry.map(|(key, _)| key.value()))
                            .collect::<std::result::Result<Vec<_>, _>>()?;
                        for key in keys {
                            table.remove(key)?;
                        }
                    }
                    {
                        let mut table = txn.open_table(PLAYLIST_ENTRIES)?;
                        let keys = table
                            .iter()?
                            .map(|entry| entry.map(|(key, _)| key.value()))
                            .collect::<std::result::Result<Vec<_>, _>>()?;
                        for key in keys {
                            table.remove(key)?;
                        }
                    }
                    {
                        let mut table = txn.open_table(PLAYLIST_SOURCES)?;
                        let keys = table
                            .iter()?
                            .map(|entry| entry.map(|(key, _)| key.value()))
                            .collect::<std::result::Result<Vec<_>, _>>()?;
                        for key in keys {
                            table.remove(key)?;
                        }
                    }
                }
                {
                    let mut metadata = txn.open_table(METADATA_TABLE)?;
                    metadata.insert("schema_version", SCHEMA_VERSION)?;
                    metadata.insert("codec_version", CODEC_VERSION)?;
                }
                {
                    let playlists = txn.open_table(PLAYLISTS_TABLE)?;
                    let mut live_playlists = HashSet::new();
                    for entry in playlists.iter()? {
                        let (playlist_id, bytes) = entry?;
                        rkyv::access::<ArchivedPlaylistSerializable, rkyv::rancor::Error>(
                            bytes.value(),
                        )
                        .with_context(|| {
                            format!("corrupt playlist record {}", playlist_id.value())
                        })?;
                        live_playlists.insert(playlist_id.value());
                    }
                    let live = winners.values().map(|(id, _)| *id).collect::<HashSet<_>>();
                    let mut entries = txn.open_table(PLAYLIST_ENTRIES)?;
                    let mut reverse = txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
                    let reverse_keys = reverse
                        .iter()?
                        .map(|entry| entry.map(|(key, _)| key.value()))
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    for key in reverse_keys {
                        reverse.remove_all(key)?;
                    }
                    let snapshot = entries
                        .iter()?
                        .map(|entry| entry.map(|(key, value)| (key.value(), value.value())))
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    for (key, old) in snapshot {
                        let playlist_id = ((key >> 32) as u64) as i64;
                        let id = remap.get(&old).copied().unwrap_or(old);
                        if !live.contains(&id) || !live_playlists.contains(&playlist_id) {
                            entries.remove(key)?;
                        } else if id != old {
                            entries.insert(key, id)?;
                        }
                        if live.contains(&id) {
                            reverse.insert(id, key)?;
                        }
                    }
                }
                {
                    let playlists = txn.open_table(PLAYLISTS_TABLE)?;
                    let live_playlists = playlists
                        .iter()?
                        .map(|entry| entry.map(|(key, _)| key.value()))
                        .collect::<std::result::Result<HashSet<_>, _>>()?;
                    let mut sources = txn.open_table(PLAYLIST_SOURCES)?;
                    let stale_sources = sources
                        .iter()?
                        .map(|entry| entry.map(|(key, _)| key.value()))
                        .collect::<std::result::Result<Vec<_>, _>>()?
                        .into_iter()
                        .filter(|id| !live_playlists.contains(id))
                        .collect::<Vec<_>>();
                    for playlist_id in stale_sources {
                        sources.remove(playlist_id)?;
                    }
                    let mut reverse = txn.open_multimap_table(SOURCE_PLAYLISTS)?;
                    let keys = reverse
                        .iter()?
                        .map(|entry| entry.map(|(key, _)| key.value().to_owned()))
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    for key in keys {
                        reverse.remove_all(key.as_str())?;
                    }
                    for entry in sources.iter()? {
                        let (playlist_id, source) = entry?;
                        reverse.insert(source.value(), playlist_id.value())?;
                    }
                }
                txn.commit()?;
                let total_files = winners.len() as u64;
                let health = DatabaseHealth {
                    is_healthy: true,
                    corruption_detected: !remap.is_empty(),
                    integrity_check_passed: true,
                    issues: Vec::new(),
                    repair_attempted: true,
                    repair_successful: true,
                };
                Ok((health, total_files, total_size))
            })
            .await?;
        self.total_files.store(total_files, Ordering::SeqCst);
        self.total_size.store(total_size, Ordering::SeqCst);
        Ok(health)
    }
}

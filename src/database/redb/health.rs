//! Database integrity, index rebuilding, backup, restore, and maintenance.

use super::*;

impl RedbDatabase {
    pub(super) async fn check_and_repair_impl(&self) -> Result<DatabaseHealth> {
        self.rebuild_derived_indexes_impl().await
    }

    pub(super) async fn create_backup_impl(&self, backup_path: &Path) -> Result<()> {
        tokio::fs::copy(&self.db_path, backup_path).await?;
        info!("Created database backup at {}", backup_path.display());
        Ok(())
    }

    pub(super) async fn restore_from_backup_impl(&self, backup_path: &Path) -> Result<()> {
        tokio::fs::copy(backup_path, &self.db_path).await?;
        info!("Restored database from backup {}", backup_path.display());
        Ok(())
    }

    pub(super) async fn vacuum_impl(&self) -> Result<()> {
        debug!("Compaction requested, skipping to preserve concurrent MVCC access");
        Ok(())
    }

    pub(super) async fn rebuild_derived_indexes_impl(&self) -> Result<DatabaseHealth> {
        let next_directory_id = Arc::clone(&self.next_directory_id);
        let (health, total_files, total_size) = self
            .execute_write(move |database| {
                let txn = database.begin_write()?;
                let mut winners: HashMap<String, (i64, MediaFile)> = HashMap::new();
                let mut remap = HashMap::new();
                {
                    let files = txn.open_table(FILES_TABLE)?;
                    for entry in files.iter()? {
                        let (id, bytes) = entry?;
                        let file =
                            Self::canonical_file(&Self::deserialize_media_file(bytes.value())?)?;
                        let path = file.path.to_string_lossy().to_string();
                        if let Some((old_id, old)) = winners.get(&path) {
                            if (file.updated_at, id.value()) > (old.updated_at, *old_id) {
                                remap.insert(*old_id, id.value());
                                winners.insert(path, (id.value(), file));
                            } else {
                                remap.insert(id.value(), *old_id);
                            }
                        } else {
                            winners.insert(path, (id.value(), file));
                        }
                    }
                }
                {
                    let mut files = txn.open_table(FILES_TABLE)?;
                    let keys = files
                        .iter()?
                        .filter_map(|e| e.ok().map(|(k, _)| k.value()))
                        .collect::<Vec<_>>();
                    for key in keys {
                        files.remove(key)?;
                    }
                    for (id, file) in winners.values_mut() {
                        file.id = Some(*id);
                        let bytes = Self::serialize_media_file(file)?;
                        files.insert(*id, bytes.as_slice())?;
                    }
                }
                macro_rules! clear_table_str {
                    ($def:expr) => {{
                        let mut table = txn.open_table($def)?;
                        let keys = table
                            .iter()?
                            .filter_map(|e| e.ok().map(|(k, _)| k.value().to_string()))
                            .collect::<Vec<_>>();
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
                        .filter_map(|entry| entry.ok().map(|(key, _)| key.value()))
                        .collect::<Vec<_>>();
                    for key in keys {
                        table.remove(key)?;
                    }
                }
                macro_rules! clear_multimap_str {
                    ($def:expr) => {{
                        let mut table = txn.open_multimap_table($def)?;
                        let keys = table
                            .iter()?
                            .filter_map(|entry| entry.ok().map(|(key, _)| key.value().to_string()))
                            .collect::<Vec<_>>();
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
                            .filter_map(|entry| entry.ok().map(|(key, _)| key.value()))
                            .collect::<Vec<_>>();
                        for key in keys {
                            let _ = table.remove_all(key)?;
                        }
                    }};
                }
                clear_multimap_u64!(DIRECTORY_CHILDREN);
                clear_multimap_u64!(DIRECTORY_FILES);
                {
                    let mut table = txn.open_multimap_table(YEAR_INDEX)?;
                    let keys = table
                        .iter()?
                        .filter_map(|e| e.ok().map(|(k, _)| k.value()))
                        .collect::<Vec<_>>();
                    for key in keys {
                        let _ = table.remove_all(key)?;
                    }
                }
                {
                    let mut paths = txn.open_table(PATH_INDEX)?;
                    let mut directory_paths = txn.open_table(DIRECTORY_PATH_INDEX)?;
                    let mut directory_records = txn.open_table(DIRECTORY_RECORDS)?;
                    let mut directory_children = txn.open_multimap_table(DIRECTORY_CHILDREN)?;
                    let mut directory_files = txn.open_multimap_table(DIRECTORY_FILES)?;
                    let mut directory_mime_counts = txn.open_table(DIRECTORY_MIME_COUNTS)?;
                    let mut artists = txn.open_multimap_table(ARTIST_INDEX)?;
                    let mut albums = txn.open_multimap_table(ALBUM_INDEX)?;
                    let mut genres = txn.open_multimap_table(GENRE_INDEX)?;
                    let mut years = txn.open_multimap_table(YEAR_INDEX)?;
                    let mut album_artists = txn.open_multimap_table(ALBUM_ARTIST_INDEX)?;
                    for (path, (id, file)) in &winners {
                        paths.insert(path.as_str(), *id)?;
                        Self::add_directory_membership(
                            &mut directory_paths,
                            &mut directory_records,
                            &mut directory_children,
                            &mut directory_files,
                            &mut directory_mime_counts,
                            &next_directory_id,
                            file,
                        )?;
                        Self::add_file_indexes(
                            &mut artists,
                            &mut albums,
                            &mut genres,
                            &mut years,
                            &mut album_artists,
                            *id,
                            file,
                        )?;
                    }
                }
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
                            .filter_map(|e| e.ok().map(|(k, _)| k.value()))
                            .collect::<Vec<_>>();
                        for key in keys {
                            table.remove(key)?;
                        }
                    }
                    {
                        let mut table = txn.open_table(PLAYLIST_ENTRIES)?;
                        let keys = table
                            .iter()?
                            .filter_map(|entry| entry.ok().map(|(key, _)| key.value()))
                            .collect::<Vec<_>>();
                        for key in keys {
                            table.remove(key)?;
                        }
                    }
                    {
                        let mut table = txn.open_table(PLAYLIST_SOURCES)?;
                        let keys = table
                            .iter()?
                            .filter_map(|e| e.ok().map(|(k, _)| k.value()))
                            .collect::<Vec<_>>();
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
                    let live = winners.values().map(|(id, _)| *id).collect::<HashSet<_>>();
                    let mut entries = txn.open_table(PLAYLIST_ENTRIES)?;
                    let mut reverse = txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
                    let reverse_keys = reverse
                        .iter()?
                        .filter_map(|entry| entry.ok().map(|(key, _)| key.value()))
                        .collect::<Vec<_>>();
                    for key in reverse_keys {
                        reverse.remove_all(key)?;
                    }
                    let snapshot = entries
                        .iter()?
                        .filter_map(|e| e.ok().map(|(k, v)| (k.value(), v.value())))
                        .collect::<Vec<_>>();
                    for (key, old) in snapshot {
                        let id = remap.get(&old).copied().unwrap_or(old);
                        if !live.contains(&id) {
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
                    let sources = txn.open_table(PLAYLIST_SOURCES)?;
                    let mut reverse = txn.open_multimap_table(SOURCE_PLAYLISTS)?;
                    let keys = reverse
                        .iter()?
                        .filter_map(|entry| entry.ok().map(|(key, _)| key.value().to_owned()))
                        .collect::<Vec<_>>();
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
                let total_size = winners.values().map(|(_, file)| file.size).sum();
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

//! Playlist records, ordered entries, and source-derived content.

use super::*;

impl RedbDatabase {
    pub(super) async fn replace_source_content_impl(
        &self,
        source_path: &Path,
        playlist_name: Option<&str>,
        entries: &[SourceMediaEntry],
    ) -> Result<Option<i64>> {
        let source = Self::canonical_path(source_path)?
            .to_string_lossy()
            .into_owned();
        let entries = entries
            .iter()
            .map(|entry| {
                Ok(SourceMediaEntry {
                    location: Self::canonical_path(&entry.location)?,
                    position: entry.position,
                    stream_title: entry.stream_title.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let playlist_name = playlist_name.map(str::to_owned);
        let candidate_playlist_id = self.next_playlist_id.fetch_add(1, Ordering::SeqCst);
        let candidate_file_ids = entries
            .iter()
            .map(|_| self.next_file_id.fetch_add(1, Ordering::SeqCst))
            .collect::<Vec<_>>();
        let next_directory_id = Arc::clone(&self.next_directory_id);

        let (playlist_id, added_files, removed_files, added_size, removed_size) = self
            .execute_write(move |database| {
                let transaction = database.begin_write()?;
                let mut resolved = Vec::with_capacity(entries.len());
                let mut stream_ids = HashSet::new();
                let mut added_files = 0_u64;
                let mut added_size = 0_u64;

                {
                    let mut files = transaction.open_table(FILES_TABLE)?;
                    let mut paths = transaction.open_table(PATH_INDEX)?;
                    let mut directory_paths = transaction.open_table(DIRECTORY_PATH_INDEX)?;
                    let mut directory_records = transaction.open_table(DIRECTORY_RECORDS)?;
                    let mut directory_children =
                        transaction.open_multimap_table(DIRECTORY_CHILDREN)?;
                    let mut ordered_children =
                        transaction.open_table(DIRECTORY_CHILDREN_BY_NAME)?;
                    let mut directory_files = transaction.open_multimap_table(DIRECTORY_FILES)?;
                    let mut ordered_files = transaction.open_table(DIRECTORY_FILES_BY_NAME)?;
                    let mut directory_mime_counts =
                        transaction.open_table(DIRECTORY_MIME_COUNTS)?;
                    let mut artist = transaction.open_multimap_table(ARTIST_INDEX)?;
                    let mut album = transaction.open_multimap_table(ALBUM_INDEX)?;
                    let mut genre = transaction.open_multimap_table(GENRE_INDEX)?;
                    let mut year = transaction.open_multimap_table(YEAR_INDEX)?;
                    let mut album_artist = transaction.open_multimap_table(ALBUM_ARTIST_INDEX)?;

                    for (entry, candidate_id) in entries.iter().zip(candidate_file_ids) {
                        let path = entry.location.to_string_lossy().into_owned();
                        let is_stream = path
                            .get(..7)
                            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("http://"))
                            || path
                                .get(..8)
                                .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"));
                        let existing_id = {
                            let value = paths.get(path.as_str())?.map(|id| id.value());
                            value
                        };
                        let file_id = if let Some(id) = existing_id {
                            id
                        } else if is_stream {
                            let mut stream =
                                MediaFile::new(entry.location.clone(), 0, "audio/radio".to_owned());
                            let title = entry.stream_title.clone().unwrap_or_else(|| path.clone());
                            stream.filename = title.clone();
                            stream.title = Some(title);
                            stream.id = Some(candidate_id);
                            let serialized = rkyv::to_bytes::<rkyv::rancor::Error>(
                                &MediaFileSerializable::from(&stream),
                            )
                            .map_err(|error| anyhow!("Failed to archive stream: {error}"))?;
                            files.insert(candidate_id, serialized.as_slice())?;
                            paths.insert(path.as_str(), candidate_id)?;
                            Self::add_directory_membership(
                                &mut directory_paths,
                                &mut directory_records,
                                &mut directory_children,
                                &mut ordered_children,
                                &mut directory_files,
                                &mut ordered_files,
                                &mut directory_mime_counts,
                                &next_directory_id,
                                &stream,
                            )?;
                            Self::add_file_indexes(
                                &mut artist,
                                &mut album,
                                &mut genre,
                                &mut year,
                                &mut album_artist,
                                candidate_id,
                                &stream,
                            )?;
                            added_files += 1;
                            added_size = added_size.saturating_add(stream.size);
                            candidate_id
                        } else {
                            continue;
                        };
                        if is_stream {
                            stream_ids.insert(file_id);
                        }
                        resolved.push((file_id, entry.position));
                    }
                }

                let playlist_id = if let Some(name) = playlist_name {
                    let existing = {
                        let reverse = transaction.open_multimap_table(SOURCE_PLAYLISTS)?;
                        let ids = reverse
                            .get(source.as_str())?
                            .map(|id| id.map(|id| id.value()))
                            .collect::<std::result::Result<Vec<_>, _>>()?;
                        ids
                    };
                    let playlist_id = existing
                        .iter()
                        .copied()
                        .min()
                        .unwrap_or(candidate_playlist_id);
                    let now = SystemTime::now();
                    let playlist = Playlist {
                        id: Some(playlist_id),
                        name,
                        description: None,
                        created_at: now,
                        updated_at: now,
                    };
                    let serialized = Self::serialize_playlist(&playlist)?;
                    {
                        let mut playlists = transaction.open_table(PLAYLISTS_TABLE)?;
                        let mut playlist_entries = transaction.open_table(PLAYLIST_ENTRIES)?;
                        let mut reverse_entries =
                            transaction.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
                        let mut sources = transaction.open_table(PLAYLIST_SOURCES)?;
                        let mut source_playlists =
                            transaction.open_multimap_table(SOURCE_PLAYLISTS)?;
                        for duplicate_id in existing {
                            let old_entries = playlist_entries
                                .range(Self::playlist_entry_range(duplicate_id))?
                                .map(|entry| entry.map(|(key, file)| (key.value(), file.value())))
                                .collect::<std::result::Result<Vec<_>, _>>()?;
                            for (key, file_id) in old_entries {
                                playlist_entries.remove(key)?;
                                reverse_entries.remove(file_id, key)?;
                            }
                            if duplicate_id != playlist_id {
                                playlists.remove(duplicate_id)?;
                                sources.remove(duplicate_id)?;
                            }
                        }
                        source_playlists.remove_all(source.as_str())?;
                        playlists.insert(playlist_id, serialized.as_slice())?;
                        sources.insert(playlist_id, source.as_str())?;
                        source_playlists.insert(source.as_str(), playlist_id)?;
                        for (file_id, position) in &resolved {
                            let key = Self::playlist_entry_key(playlist_id, *position);
                            playlist_entries.insert(key, *file_id)?;
                            reverse_entries.insert(*file_id, key)?;
                        }
                    }
                    Some(playlist_id)
                } else {
                    let existing = {
                        let reverse = transaction.open_multimap_table(SOURCE_PLAYLISTS)?;
                        let ids = reverse
                            .get(source.as_str())?
                            .map(|id| id.map(|id| id.value()))
                            .collect::<std::result::Result<Vec<_>, _>>()?;
                        ids
                    };
                    {
                        let mut playlists = transaction.open_table(PLAYLISTS_TABLE)?;
                        let mut playlist_entries = transaction.open_table(PLAYLIST_ENTRIES)?;
                        let mut reverse_entries =
                            transaction.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
                        let mut sources = transaction.open_table(PLAYLIST_SOURCES)?;
                        let mut source_playlists =
                            transaction.open_multimap_table(SOURCE_PLAYLISTS)?;
                        for playlist_id in existing {
                            let old_entries = playlist_entries
                                .range(Self::playlist_entry_range(playlist_id))?
                                .map(|entry| entry.map(|(key, file)| (key.value(), file.value())))
                                .collect::<std::result::Result<Vec<_>, _>>()?;
                            for (key, file_id) in old_entries {
                                playlist_entries.remove(key)?;
                                reverse_entries.remove(file_id, key)?;
                            }
                            playlists.remove(playlist_id)?;
                            sources.remove(playlist_id)?;
                        }
                        source_playlists.remove_all(source.as_str())?;
                    }
                    None
                };

                let old_stream_ids = {
                    let source_streams = transaction.open_multimap_table(SOURCE_STREAMS)?;
                    let ids = source_streams
                        .get(source.as_str())?
                        .map(|id| id.map(|id| id.value()))
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    ids
                };
                {
                    let mut source_streams = transaction.open_multimap_table(SOURCE_STREAMS)?;
                    let mut stream_sources = transaction.open_multimap_table(STREAM_SOURCES)?;
                    source_streams.remove_all(source.as_str())?;
                    for file_id in &old_stream_ids {
                        stream_sources.remove(*file_id, source.as_str())?;
                    }
                    for file_id in &stream_ids {
                        source_streams.insert(source.as_str(), *file_id)?;
                        stream_sources.insert(*file_id, source.as_str())?;
                    }
                }

                let orphaned = {
                    let stream_sources = transaction.open_multimap_table(STREAM_SOURCES)?;
                    let playlist_refs = transaction.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
                    let files = transaction.open_table(FILES_TABLE)?;
                    let mut orphaned = Vec::new();
                    for file_id in old_stream_ids {
                        let has_owner = stream_sources.get(file_id)?.next().transpose()?.is_some();
                        let has_playlist =
                            playlist_refs.get(file_id)?.next().transpose()?.is_some();
                        if !has_owner && !has_playlist {
                            if let Some(bytes) = files.get(file_id)? {
                                let view = RedbReadSession::view(bytes.value())?;
                                if view.mime_type() == "audio/radio" {
                                    orphaned.push((
                                        view.path().to_owned(),
                                        file_id,
                                        IndexSnapshot::from_view(&view).ok_or_else(|| {
                                            anyhow!("stream record {file_id} has no ID")
                                        })?,
                                    ));
                                }
                            }
                        }
                    }
                    orphaned
                };
                let (removed_files, removed_size) =
                    Self::remove_files_from_transaction(&transaction, &orphaned)?;
                transaction.commit()?;
                Ok((
                    playlist_id,
                    added_files,
                    removed_files as u64,
                    added_size,
                    removed_size,
                ))
            })
            .await?;

        if added_files >= removed_files {
            self.total_files
                .fetch_add(added_files - removed_files, Ordering::SeqCst);
        } else {
            self.total_files
                .fetch_sub(removed_files - added_files, Ordering::SeqCst);
        }
        if added_size >= removed_size {
            self.total_size
                .fetch_add(added_size - removed_size, Ordering::SeqCst);
        } else {
            self.total_size
                .fetch_sub(removed_size - added_size, Ordering::SeqCst);
        }
        Ok(playlist_id)
    }

    pub(super) async fn create_playlist_impl(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<i64> {
        let playlist_id = self.next_playlist_id.fetch_add(1, Ordering::SeqCst);
        let now = SystemTime::now();

        let playlist = Playlist {
            id: Some(playlist_id),
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        };

        let serialized = Self::serialize_playlist(&playlist)?;

        self.execute_write(move |database| {
            let write_txn = database.begin_write()?;
            {
                write_txn
                    .open_table(PLAYLISTS_TABLE)?
                    .insert(playlist_id, serialized.as_slice())?;
            }
            write_txn.commit()?;
            Ok(())
        })
        .await?;

        info!("Created playlist '{}' with ID {}", name, playlist_id);
        Ok(playlist_id)
    }

    pub(super) async fn get_playlists_impl(&self) -> Result<Vec<Playlist>> {
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let playlists_table = read_txn.open_table(PLAYLISTS_TABLE)?;

            let mut playlists = Vec::new();
            for result in playlists_table.iter()? {
                let (key, value) = result?;
                playlists.push(
                    Self::deserialize_playlist(value.value())
                        .with_context(|| format!("corrupt playlist record {}", key.value()))?,
                );
            }

            Ok(playlists)
        })
        .await
    }

    pub(super) async fn get_playlist_impl(&self, playlist_id: i64) -> Result<Option<Playlist>> {
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let playlists_table = read_txn.open_table(PLAYLISTS_TABLE)?;

            if let Some(data) = playlists_table.get(playlist_id)? {
                return Ok(Some(Self::deserialize_playlist(data.value())?));
            }

            Ok(None)
        })
        .await
    }

    pub(super) async fn update_playlist_impl(&self, playlist: &Playlist) -> Result<()> {
        let Some(playlist_id) = playlist.id else {
            return Err(anyhow!("Cannot update playlist without ID"));
        };

        let serialized = Self::serialize_playlist(playlist)?;

        self.execute_write(move |database| {
            let write_txn = database.begin_write()?;
            {
                let mut playlists = write_txn.open_table(PLAYLISTS_TABLE)?;
                if playlists.get(playlist_id)?.is_none() {
                    return Err(anyhow!("playlist {playlist_id} not found"));
                }
                playlists.insert(playlist_id, serialized.as_slice())?;
            }
            write_txn.commit()?;
            Ok(())
        })
        .await
    }

    pub(super) async fn delete_playlist_impl(&self, playlist_id: i64) -> Result<bool> {
        self.execute_write(move |database| {
            let write_txn = database.begin_write()?;
            let removed = {
                let mut playlists_table = write_txn.open_table(PLAYLISTS_TABLE)?;
                let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
                let mut reverse_entries = write_txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
                let mut playlist_sources = write_txn.open_table(PLAYLIST_SOURCES)?;
                let mut source_playlists = write_txn.open_multimap_table(SOURCE_PLAYLISTS)?;

                let existed = playlists_table.remove(playlist_id)?.is_some();

                // Remove all entries for this playlist
                let entries = playlist_entries
                    .range(Self::playlist_entry_range(playlist_id))?
                    .map(|entry| entry.map(|(key, file)| (key.value(), file.value())))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                for (key, file_id) in entries {
                    playlist_entries.remove(key)?;
                    reverse_entries.remove(file_id, key)?;
                }
                if let Some(source) = playlist_sources
                    .get(playlist_id)?
                    .map(|value| value.value().to_owned())
                {
                    source_playlists.remove(source.as_str(), playlist_id)?;
                }
                playlist_sources.remove(playlist_id)?;

                existed
            };
            write_txn.commit()?;

            Ok(removed)
        })
        .await
    }

    pub(super) async fn set_playlist_source_impl(
        &self,
        playlist_id: i64,
        source_path: &Path,
    ) -> Result<()> {
        let source = Self::canonical_path(source_path)?
            .to_string_lossy()
            .to_string();
        self.execute_write(move |database| {
            let txn = database.begin_write()?;
            {
                if txn.open_table(PLAYLISTS_TABLE)?.get(playlist_id)?.is_none() {
                    return Err(anyhow!("playlist {playlist_id} not found"));
                }
                let mut sources = txn.open_table(PLAYLIST_SOURCES)?;
                let mut reverse = txn.open_multimap_table(SOURCE_PLAYLISTS)?;
                if let Some(old) = sources
                    .get(playlist_id)?
                    .map(|value| value.value().to_owned())
                {
                    reverse.remove(old.as_str(), playlist_id)?;
                }
                sources.insert(playlist_id, source.as_str())?;
                reverse.insert(source.as_str(), playlist_id)?;
            }
            txn.commit()?;
            Ok(())
        })
        .await
    }

    pub(super) async fn replace_playlist_from_source_impl(
        &self,
        source_path: &Path,
        name: &str,
        media_file_ids: &[(i64, u32)],
    ) -> Result<i64> {
        let source = Self::canonical_path(source_path)?
            .to_string_lossy()
            .into_owned();
        let name = name.to_owned();
        let entries = media_file_ids.to_vec();
        let candidate_id = self.next_playlist_id.fetch_add(1, Ordering::SeqCst);
        self.execute_write(move |database| {
            let transaction = database.begin_write()?;
            let playlist_id = {
                let reverse = transaction.open_multimap_table(SOURCE_PLAYLISTS)?;
                let existing = reverse
                    .get(source.as_str())?
                    .map(|value| value.map(|id| id.value()))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                existing.into_iter().min().unwrap_or(candidate_id)
            };

            {
                let files = transaction.open_table(FILES_TABLE)?;
                for (file_id, _) in &entries {
                    if files.get(*file_id)?.is_none() {
                        return Err(anyhow!("media file {file_id} not found"));
                    }
                }
            }

            let now = SystemTime::now();
            let playlist = Playlist {
                id: Some(playlist_id),
                name,
                description: None,
                created_at: now,
                updated_at: now,
            };
            let serialized = Self::serialize_playlist(&playlist)?;
            {
                let mut playlists = transaction.open_table(PLAYLISTS_TABLE)?;
                let mut playlist_entries = transaction.open_table(PLAYLIST_ENTRIES)?;
                let mut reverse_entries = transaction.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
                let mut sources = transaction.open_table(PLAYLIST_SOURCES)?;
                let mut source_playlists = transaction.open_multimap_table(SOURCE_PLAYLISTS)?;

                let old_entries = playlist_entries
                    .range(Self::playlist_entry_range(playlist_id))?
                    .map(|entry| entry.map(|(key, file)| (key.value(), file.value())))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                for (key, file_id) in old_entries {
                    playlist_entries.remove(key)?;
                    reverse_entries.remove(file_id, key)?;
                }

                let duplicate_ids = source_playlists
                    .get(source.as_str())?
                    .map(|value| value.map(|id| id.value()))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                for duplicate_id in duplicate_ids {
                    if duplicate_id == playlist_id {
                        continue;
                    }
                    let duplicate_entries = playlist_entries
                        .range(Self::playlist_entry_range(duplicate_id))?
                        .map(|entry| entry.map(|(key, file)| (key.value(), file.value())))
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    for (key, file_id) in duplicate_entries {
                        playlist_entries.remove(key)?;
                        reverse_entries.remove(file_id, key)?;
                    }
                    playlists.remove(duplicate_id)?;
                    sources.remove(duplicate_id)?;
                }
                source_playlists.remove_all(source.as_str())?;

                playlists.insert(playlist_id, serialized.as_slice())?;
                sources.insert(playlist_id, source.as_str())?;
                source_playlists.insert(source.as_str(), playlist_id)?;
                for (file_id, position) in &entries {
                    let key = Self::playlist_entry_key(playlist_id, *position);
                    playlist_entries.insert(key, *file_id)?;
                    reverse_entries.insert(*file_id, key)?;
                }
            }
            transaction.commit()?;
            Ok(playlist_id)
        })
        .await
    }

    pub(super) async fn remove_derived_content_by_source_impl(
        &self,
        source_path: &Path,
    ) -> Result<usize> {
        let source = Self::canonical_path(source_path)?
            .to_string_lossy()
            .to_string();
        let child_prefix = format!("{}/", source.trim_end_matches('/'));
        let source_for_query = source.clone();
        let child_for_query = child_prefix.clone();
        let sources = self
            .execute_read(move |database| {
                let txn = database.begin_read()?;
                let mut sources = Vec::new();
                for is_playlist in [true, false] {
                    if is_playlist {
                        let table = txn.open_multimap_table(SOURCE_PLAYLISTS)?;
                        for entry in table.range(source_for_query.as_str()..)? {
                            let (key, _) = entry?;
                            if key.value() != source_for_query
                                && !key.value().starts_with(&child_for_query)
                            {
                                break;
                            }
                            sources.push(key.value().to_owned());
                        }
                    } else {
                        let table = txn.open_multimap_table(SOURCE_STREAMS)?;
                        for entry in table.range(source_for_query.as_str()..)? {
                            let (key, _) = entry?;
                            if key.value() != source_for_query
                                && !key.value().starts_with(&child_for_query)
                            {
                                break;
                            }
                            sources.push(key.value().to_owned());
                        }
                    }
                }
                sources.sort();
                sources.dedup();
                Ok(sources)
            })
            .await?;
        let mut removed = 0;
        for derived_source in sources {
            let before = self.total_files.load(Ordering::SeqCst);
            self.replace_source_content_impl(Path::new(&derived_source), None, &[])
                .await?;
            removed += before.saturating_sub(self.total_files.load(Ordering::SeqCst)) as usize;
            removed += 1;
        }
        Ok(removed)
    }

    pub(super) async fn add_to_playlist_impl(
        &self,
        playlist_id: i64,
        media_file_id: i64,
        position: Option<u32>,
    ) -> Result<i64> {
        let pos = position.unwrap_or(0);
        let key = Self::playlist_entry_key(playlist_id, pos);

        self.execute_write(move |database| {
            let write_txn = database.begin_write()?;
            {
                if write_txn
                    .open_table(PLAYLISTS_TABLE)?
                    .get(playlist_id)?
                    .is_none()
                {
                    return Err(anyhow!("playlist {playlist_id} not found"));
                }
                if write_txn
                    .open_table(FILES_TABLE)?
                    .get(media_file_id)?
                    .is_none()
                {
                    return Err(anyhow!("media file {media_file_id} not found"));
                }
                let mut entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
                let old = entries
                    .insert(key, media_file_id)?
                    .map(|value| value.value());
                let mut reverse = write_txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
                if let Some(old) = old {
                    reverse.remove(old, key)?;
                }
                reverse.insert(media_file_id, key)?;
            }
            write_txn.commit()?;
            Ok(media_file_id)
        })
        .await
    }

    pub(super) async fn batch_add_to_playlist_impl(
        &self,
        playlist_id: i64,
        media_file_ids: &[(i64, u32)],
    ) -> Result<Vec<i64>> {
        let media_file_ids = media_file_ids.to_vec();
        self.execute_write(move |database| {
            let write_txn = database.begin_write()?;
            {
                if write_txn
                    .open_table(PLAYLISTS_TABLE)?
                    .get(playlist_id)?
                    .is_none()
                {
                    return Err(anyhow!("playlist {playlist_id} not found"));
                }
                let files = write_txn.open_table(FILES_TABLE)?;
                for (file_id, _) in &media_file_ids {
                    if files.get(*file_id)?.is_none() {
                        return Err(anyhow!("media file {file_id} not found"));
                    }
                }
                let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
                let mut reverse_entries = write_txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
                for (file_id, position) in &media_file_ids {
                    let key = Self::playlist_entry_key(playlist_id, *position);
                    if let Some(old) = playlist_entries
                        .insert(key, *file_id)?
                        .map(|value| value.value())
                    {
                        reverse_entries.remove(old, key)?;
                    }
                    reverse_entries.insert(*file_id, key)?;
                }
            }
            write_txn.commit()?;

            Ok(media_file_ids.iter().map(|(id, _)| *id).collect())
        })
        .await
    }

    pub(super) async fn remove_from_playlist_impl(
        &self,
        playlist_id: i64,
        media_file_id: i64,
    ) -> Result<bool> {
        self.execute_write(move |database| {
            let write_txn = database.begin_write()?;
            let removed = {
                if write_txn
                    .open_table(PLAYLISTS_TABLE)?
                    .get(playlist_id)?
                    .is_none()
                {
                    return Err(anyhow!("playlist {playlist_id} not found"));
                }
                let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
                let mut reverse = write_txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
                let mut key_to_remove = None;
                for entry in playlist_entries.range(Self::playlist_entry_range(playlist_id))? {
                    let (key, value) = entry?;
                    if value.value() == media_file_id {
                        key_to_remove = Some(key.value());
                        break;
                    }
                }

                if let Some(key) = key_to_remove {
                    playlist_entries.remove(key)?;
                    reverse.remove(media_file_id, key)?;
                    true
                } else {
                    false
                }
            };
            write_txn.commit()?;

            Ok(removed)
        })
        .await
    }

    pub(super) async fn get_playlist_tracks_impl(
        &self,
        playlist_id: i64,
    ) -> Result<Vec<MediaFile>> {
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            if read_txn
                .open_table(PLAYLISTS_TABLE)?
                .get(playlist_id)?
                .is_none()
            {
                return Err(anyhow!("playlist {playlist_id} not found"));
            }
            let playlist_entries = read_txn.open_table(PLAYLIST_ENTRIES)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut files = Vec::new();
            for entry in playlist_entries.range(Self::playlist_entry_range(playlist_id))? {
                let (_, file_id) = entry?;
                let file_id = file_id.value();
                let data = files_table.get(file_id)?.ok_or_else(|| {
                    anyhow!("playlist {playlist_id} references missing media file {file_id}")
                })?;
                files.push(Self::deserialize_media_file(data.value())?);
            }

            Ok(files)
        })
        .await
    }

    pub(super) async fn reorder_playlist_impl(
        &self,
        playlist_id: i64,
        track_positions: &[(i64, u32)],
    ) -> Result<()> {
        let track_positions = track_positions.to_vec();
        self.execute_write(move |database| {
            let write_txn = database.begin_write()?;
            {
                if write_txn
                    .open_table(PLAYLISTS_TABLE)?
                    .get(playlist_id)?
                    .is_none()
                {
                    return Err(anyhow!("playlist {playlist_id} not found"));
                }
                let files = write_txn.open_table(FILES_TABLE)?;
                for (file_id, _) in &track_positions {
                    if files.get(*file_id)?.is_none() {
                        return Err(anyhow!("media file {file_id} not found"));
                    }
                }
                let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
                let mut reverse_entries = write_txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;

                // Remove existing entries for this playlist
                let entries = playlist_entries
                    .range(Self::playlist_entry_range(playlist_id))?
                    .map(|entry| entry.map(|(key, file)| (key.value(), file.value())))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                for (key, file_id) in entries {
                    playlist_entries.remove(key)?;
                    reverse_entries.remove(file_id, key)?;
                }

                // Insert new order
                for (file_id, position) in track_positions {
                    let key = Self::playlist_entry_key(playlist_id, position);
                    playlist_entries.insert(key, file_id)?;
                    reverse_entries.insert(file_id, key)?;
                }
            }
            write_txn.commit()?;

            Ok(())
        })
        .await
    }
}

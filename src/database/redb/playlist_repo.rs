//! Playlist records, ordered entries, and source-derived content.

use super::*;

impl RedbDatabase {
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
                let (_, value) = result?;
                if let Ok(playlist) = Self::deserialize_playlist(value.value()) {
                    playlists.push(playlist);
                }
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
                write_txn
                    .open_table(PLAYLISTS_TABLE)?
                    .insert(playlist_id, serialized.as_slice())?;
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
                    .filter_map(|entry| entry.ok().map(|(key, file)| (key.value(), file.value())))
                    .collect::<Vec<_>>();
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

    pub(super) async fn remove_derived_content_by_source_impl(
        &self,
        source_path: &Path,
    ) -> Result<usize> {
        let source = Self::canonical_path(source_path)?
            .to_string_lossy()
            .to_string();
        let child_prefix = format!("{}/", source.trim_end_matches('/'));
        let matches_source =
            |candidate: &str| candidate == source || candidate.starts_with(&child_prefix);
        let source_for_query = source.clone();
        let child_for_query = child_prefix.clone();
        let ids = self
            .execute_read(move |database| {
                let txn = database.begin_read()?;
                let table = txn.open_multimap_table(SOURCE_PLAYLISTS)?;
                let mut ids = Vec::new();
                for entry in table.range(source_for_query.as_str()..)? {
                    let (key, values) = entry?;
                    if key.value() != source_for_query && !key.value().starts_with(&child_for_query)
                    {
                        break;
                    }
                    for value in values {
                        ids.push(value?.value());
                    }
                }
                ids.sort_unstable();
                ids.dedup();
                Ok(ids)
            })
            .await?;
        let mut removed = 0;
        for id in ids {
            removed += usize::from(self.delete_playlist_impl(id).await?);
        }
        let mut radio_paths = Vec::new();
        use futures_util::StreamExt;
        let mut stream = self.stream_all_media_files_impl();
        while let Some(file) = stream.next().await {
            let file = file?;
            if file.mime_type == "audio/radio" && file.album.as_deref().is_some_and(matches_source)
            {
                radio_paths.push(file.path);
            }
        }
        drop(stream);
        removed += self.bulk_remove_media_files_impl(&radio_paths).await?;
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
                let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
                let mut reverse = write_txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;
                let key_to_remove = playlist_entries
                    .range(Self::playlist_entry_range(playlist_id))?
                    .filter_map(|entry| entry.ok())
                    .find(|(_, value)| value.value() == media_file_id)
                    .map(|(key, _)| key.value());

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
            let playlist_entries = read_txn.open_table(PLAYLIST_ENTRIES)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut files = Vec::new();
            for entry in playlist_entries.range(Self::playlist_entry_range(playlist_id))? {
                let (_, file_id) = entry?;
                let file_id = file_id.value();
                if let Some(data) = files_table.get(file_id)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
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
                let mut playlist_entries = write_txn.open_table(PLAYLIST_ENTRIES)?;
                let mut reverse_entries = write_txn.open_multimap_table(FILE_PLAYLIST_ENTRIES)?;

                // Remove existing entries for this playlist
                let entries = playlist_entries
                    .range(Self::playlist_entry_range(playlist_id))?
                    .filter_map(|entry| entry.ok().map(|(key, file)| (key.value(), file.value())))
                    .collect::<Vec<_>>();
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

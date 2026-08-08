use super::super::*;

impl RedbDatabase {
    pub(in crate::database::redb) async fn get_files_by_paths_impl(
        &self,
        paths: &[PathBuf],
    ) -> Result<Vec<MediaFile>> {
        let paths = paths
            .iter()
            .map(|path| {
                Self::canonical_path(path).map(|value| value.to_string_lossy().into_owned())
            })
            .collect::<Result<Vec<_>>>()?;
        self.execute_read(move |database| {
            let mut files = Vec::new();
            let read_txn = database.begin_read()?;
            let path_index = read_txn.open_table(PATH_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            for path_str in paths {
                if let Some(file_id) = path_index.get(path_str.as_str())?.map(|v| v.value()) {
                    if let Some(data) = files_table.get(file_id)? {
                        files.push(Self::deserialize_media_file(data.value())?);
                    }
                }
            }

            Ok(files)
        })
        .await
    }

    pub(in crate::database::redb) async fn bulk_store_media_files_impl(
        &self,
        files: &[MediaFile],
    ) -> Result<Vec<i64>> {
        self.bulk_store_media_files_with_mode(files, false).await
    }

    pub(in crate::database::redb) async fn bulk_store_canonical_media_files_impl(
        &self,
        files: &[MediaFile],
    ) -> Result<Vec<i64>> {
        self.bulk_store_media_files_with_mode(files, true).await
    }

    async fn bulk_store_media_files_with_mode(
        &self,
        files: &[MediaFile],
        paths_are_canonical: bool,
    ) -> Result<Vec<i64>> {
        let inputs = files.to_vec();
        let candidate_ids = inputs
            .iter()
            .map(|file| {
                file.id
                    .unwrap_or_else(|| self.next_file_id.fetch_add(1, Ordering::SeqCst))
            })
            .collect::<Vec<_>>();
        let next_directory_id = Arc::clone(&self.next_directory_id);
        let (ids, added_files, replaced_size, stored_size) = self
            .execute_write(move |database| {
                let mut ids = Vec::with_capacity(inputs.len());
                let mut added_files = 0_u64;
                let mut replaced_size = 0_u64;
                let mut stored_size = 0_u64;

                let write_txn = database.begin_write()?;
                {
                    let mut files_table = write_txn.open_table(FILES_TABLE)?;
                    let mut path_index = write_txn.open_table(PATH_INDEX)?;
                    let mut directory_paths = write_txn.open_table(DIRECTORY_PATH_INDEX)?;
                    let mut directory_records = write_txn.open_table(DIRECTORY_RECORDS)?;
                    let mut directory_children =
                        write_txn.open_multimap_table(DIRECTORY_CHILDREN)?;
                    let mut ordered_children = write_txn.open_table(DIRECTORY_CHILDREN_BY_NAME)?;
                    let mut directory_files = write_txn.open_multimap_table(DIRECTORY_FILES)?;
                    let mut directory_mime_counts = write_txn.open_table(DIRECTORY_MIME_COUNTS)?;

                    let mut artist_index = write_txn.open_multimap_table(ARTIST_INDEX)?;
                    let mut album_index = write_txn.open_multimap_table(ALBUM_INDEX)?;
                    let mut genre_index = write_txn.open_multimap_table(GENRE_INDEX)?;
                    let mut year_index = write_txn.open_multimap_table(YEAR_INDEX)?;
                    let mut album_artist_index =
                        write_txn.open_multimap_table(ALBUM_ARTIST_INDEX)?;
                    let mut archive_scratch: rkyv::util::AlignedVec = rkyv::util::AlignedVec::new();

                    for (input, candidate_id) in inputs.iter().zip(candidate_ids) {
                        let file = if paths_are_canonical {
                            input.clone()
                        } else {
                            Self::canonical_file(input)?
                        };
                        let path_str = file.path.to_string_lossy().to_string();
                        let existing_path_id =
                            path_index.get(path_str.as_str())?.map(|v| v.value());
                        let file_id = existing_path_id.or(file.id).unwrap_or(candidate_id);
                        ids.push(file_id);

                        let mut file_with_id = file.clone();
                        file_with_id.id = Some(file_id);
                        let had_old = if let Some(old_bytes) = files_table.get(file_id)? {
                            let old = RedbReadSession::view(old_bytes.value())?;
                            Self::remove_directory_membership(
                                &mut directory_paths,
                                &mut directory_records,
                                &mut directory_children,
                                &mut ordered_children,
                                &mut directory_files,
                                &mut directory_mime_counts,
                                file_id,
                                &old,
                            )?;
                            Self::remove_file_indexes(
                                &mut artist_index,
                                &mut album_index,
                                &mut genre_index,
                                &mut year_index,
                                &mut album_artist_index,
                                file_id,
                                &old,
                            )?;
                            if old.path() != path_str {
                                path_index.remove(old.path())?;
                            }
                            replaced_size = replaced_size.saturating_add(old.size());
                            true
                        } else {
                            false
                        };
                        if !had_old {
                            added_files = added_files.saturating_add(1);
                        }

                        archive_scratch.clear();
                        archive_scratch = rkyv::api::high::to_bytes_in::<_, rkyv::rancor::Error>(
                            &MediaFileSerializable::from(&file_with_id),
                            archive_scratch,
                        )
                        .map_err(|error| {
                            anyhow!("Failed to archive MediaFile using Rkyv: {error}")
                        })?;
                        files_table.insert(file_id, archive_scratch.as_slice())?;
                        path_index.insert(path_str.as_str(), file_id)?;
                        Self::add_directory_membership(
                            &mut directory_paths,
                            &mut directory_records,
                            &mut directory_children,
                            &mut ordered_children,
                            &mut directory_files,
                            &mut directory_mime_counts,
                            &next_directory_id,
                            &file_with_id,
                        )?;
                        Self::add_file_indexes(
                            &mut artist_index,
                            &mut album_index,
                            &mut genre_index,
                            &mut year_index,
                            &mut album_artist_index,
                            file_id,
                            &file_with_id,
                        )?;
                        stored_size = stored_size.saturating_add(file.size);
                    }
                }
                write_txn.commit()?;
                Ok((ids, added_files, replaced_size, stored_size))
            })
            .await?;
        self.total_files.fetch_add(added_files, Ordering::SeqCst);
        if stored_size >= replaced_size {
            self.total_size
                .fetch_add(stored_size - replaced_size, Ordering::SeqCst);
        } else {
            let decrease = replaced_size - stored_size;
            let _ = self
                .total_size
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                    Some(current.saturating_sub(decrease))
                });
        }

        debug!("Bulk stored {} media files", ids.len());
        Ok(ids)
    }

    pub(in crate::database::redb) async fn bulk_update_media_files_impl(
        &self,
        files: &[MediaFile],
    ) -> Result<()> {
        if files.iter().any(|file| file.id.is_none()) {
            return Err(anyhow!("cannot update a media file without an ID"));
        }
        self.bulk_store_media_files_impl(files).await?;
        Ok(())
    }

    pub(in crate::database::redb) async fn bulk_update_canonical_media_files_impl(
        &self,
        files: &[MediaFile],
    ) -> Result<()> {
        if files.iter().any(|file| file.id.is_none()) {
            return Err(anyhow!("cannot update a media file without an ID"));
        }
        self.bulk_store_canonical_media_files_impl(files).await?;
        Ok(())
    }

    pub(in crate::database::redb) async fn bulk_remove_media_files_impl(
        &self,
        paths: &[PathBuf],
    ) -> Result<usize> {
        let paths = paths
            .iter()
            .map(|path| Self::canonical_path(path).map(|path| path.to_string_lossy().to_string()))
            .collect::<Result<Vec<_>>>()?;
        let (removed, removed_size) = self
            .execute_write(move |database| {
                let transaction = database.begin_write()?;
                let mut files = Vec::new();
                let mut orphan_paths = Vec::new();
                let mut seen_ids = HashSet::new();

                {
                    let path_index = transaction.open_table(PATH_INDEX)?;
                    let files_table = transaction.open_table(FILES_TABLE)?;
                    for path_string in &paths {
                        let Some(id) = path_index
                            .get(path_string.as_str())?
                            .map(|value| value.value())
                        else {
                            continue;
                        };
                        if !seen_ids.insert(id) {
                            continue;
                        }
                        if let Some(data) = files_table.get(id)? {
                            let view = RedbReadSession::view(data.value())?;
                            let snapshot = IndexSnapshot::from_view(&view)
                                .ok_or_else(|| anyhow!("stored media record {id} has no ID"))?;
                            files.push((path_string.clone(), id, snapshot));
                        } else {
                            orphan_paths.push(path_string.clone());
                        }
                    }
                }

                if !orphan_paths.is_empty() {
                    let mut path_index = transaction.open_table(PATH_INDEX)?;
                    for path in orphan_paths {
                        path_index.remove(path.as_str())?;
                    }
                }

                let result = Self::remove_files_from_transaction(&transaction, &files)?;
                transaction.commit()?;
                Ok(result)
            })
            .await?;
        let _ = self
            .total_files
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                Some(current.saturating_sub(removed as u64))
            });
        let _ = self
            .total_size
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                Some(current.saturating_sub(removed_size))
            });

        debug!("Bulk removed {} media files", removed);
        Ok(removed)
    }

    pub(in crate::database::redb) async fn remove_media_under_path_impl(
        &self,
        path: &Path,
    ) -> Result<RemovalSummary> {
        let canonical = Self::canonical_path(path)?;
        let prefix = canonical
            .to_string_lossy()
            .trim_end_matches('/')
            .to_string();
        let (mut summary, removed, removed_size) = self
            .execute_write(move |database| {
                let transaction = database.begin_write()?;
                let mut files = Vec::new();
                let mut seen_ids = HashSet::new();
                let mut summary = RemovalSummary::default();

                {
                    let path_index = transaction.open_table(PATH_INDEX)?;
                    let files_table = transaction.open_table(FILES_TABLE)?;
                    let directory_paths = transaction.open_table(DIRECTORY_PATH_INDEX)?;
                    let directory_children = transaction.open_multimap_table(DIRECTORY_CHILDREN)?;
                    let directory_files = transaction.open_multimap_table(DIRECTORY_FILES)?;

                    let mut file_ids = Vec::new();
                    if let Some(root_id) = directory_paths
                        .get(prefix.as_str())?
                        .map(|value| value.value())
                    {
                        let mut stack = vec![root_id];
                        while let Some(directory_id) = stack.pop() {
                            for child in directory_children.get(directory_id)? {
                                stack.push(child?.value());
                            }
                            for file_id in directory_files.get(directory_id)? {
                                file_ids.push(file_id?.value());
                            }
                        }
                    } else if let Some(file_id) =
                        path_index.get(prefix.as_str())?.map(|value| value.value())
                    {
                        file_ids.push(file_id);
                    }

                    for id in file_ids {
                        if !seen_ids.insert(id) {
                            continue;
                        }
                        if let Some(data) = files_table.get(id)? {
                            let view = RedbReadSession::view(data.value())?;
                            if let Some(parent) = Path::new(view.path()).parent() {
                                summary.affected_parents.push(parent.to_path_buf());
                            }
                            summary
                                .mime_families
                                .insert(Self::mime_family(view.mime_type()));
                            let snapshot = IndexSnapshot::from_view(&view)
                                .ok_or_else(|| anyhow!("stored media record {id} has no ID"))?;
                            files.push((view.path().to_owned(), id, snapshot));
                        }
                    }
                }

                summary.affected_parents.sort();
                summary.affected_parents.dedup();
                let (removed, removed_size) =
                    Self::remove_files_from_transaction(&transaction, &files)?;
                let pruned_directories =
                    Self::prune_directory_subtree(&transaction, prefix.as_str())?;
                if pruned_directories > 0 {
                    if let Some(parent) = Path::new(&prefix).parent() {
                        summary.affected_parents.push(parent.to_path_buf());
                        summary.affected_parents.sort();
                        summary.affected_parents.dedup();
                    }
                    debug!(
                        "Defensively pruned {} directory records under {}",
                        pruned_directories, prefix
                    );
                }
                transaction.commit()?;
                Ok((summary, removed, removed_size))
            })
            .await?;

        let _ = self
            .total_files
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                Some(current.saturating_sub(removed as u64))
            });
        let _ = self
            .total_size
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                Some(current.saturating_sub(removed_size))
            });
        summary.removed_files = removed;
        Ok(summary)
    }
}

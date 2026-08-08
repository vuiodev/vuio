use super::super::*;

// Media CRUD, directory queries, and common read operations.

impl RedbDatabase {
    pub(in crate::database::redb) async fn read_impl<R, F>(
        self: Arc<Self>,
        operation: F,
    ) -> Result<R>
    where
        R: Send + 'static,
        F: FnOnce(&mut RedbReadSession) -> Result<R> + Send + 'static,
    {
        let database = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let database = database
                .read()
                .map_err(|_| anyhow!("ReDB handle lock is poisoned"))?;
            let transaction = database.begin_read()?;
            let mut session = RedbReadSession { transaction };
            operation(&mut session)
        })
        .await
        .context("ReDB read task failed")?
    }

    pub(in crate::database::redb) async fn store_media_file_impl(
        &self,
        file: &MediaFile,
    ) -> Result<i64> {
        self.bulk_store_media_files_impl(std::slice::from_ref(file))
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("media upsert returned no ID"))
    }

    pub(in crate::database::redb) async fn get_file_location_by_id_impl(
        &self,
        id: i64,
    ) -> Result<Option<FileLocation>> {
        self.execute_read(move |database| {
            let transaction = database.begin_read()?;
            let files = transaction.open_table(FILES_TABLE)?;
            files
                .get(id)?
                .map(|bytes| {
                    RedbReadSession::view(bytes.value())?
                        .to_file_location()
                        .ok_or_else(|| anyhow!("stored media record {id} has no ID"))
                })
                .transpose()
        })
        .await
    }

    pub(in crate::database::redb) async fn load_file_fingerprints_impl(
        &self,
    ) -> Result<Vec<FileFingerprint>> {
        let capacity = self.total_files.load(Ordering::Relaxed) as usize;
        self.execute_read(move |database| {
            let transaction = database.begin_read()?;
            let files = transaction.open_table(FILES_TABLE)?;
            let mut fingerprints = Vec::with_capacity(capacity);
            for entry in files.iter()? {
                let (id, bytes) = entry?;
                let view = RedbReadSession::view(bytes.value())?;
                fingerprints.push(FileFingerprint {
                    id: id.value(),
                    path: PathBuf::from(view.path()),
                    size: view.size(),
                    modified: UNIX_EPOCH + Duration::from_secs(view.modified_secs()),
                    created_at: UNIX_EPOCH + Duration::from_secs(view.created_at_secs()),
                });
            }
            Ok(fingerprints)
        })
        .await
    }

    pub(in crate::database::redb) fn stream_all_media_files_impl(
        &self,
    ) -> Pin<Box<dyn futures_util::Stream<Item = Result<MediaFile, DatabaseError>> + Send + '_>>
    {
        let db = self.db.clone();
        let (sender, receiver) = tokio::sync::mpsc::channel(32);
        tokio::task::spawn_blocking(move || {
            let operation = || -> std::result::Result<(), DatabaseError> {
                let db = db.read().map_err(|_| DatabaseError::QueryFailed {
                    query: "database handle".into(),
                    reason: "ReDB handle lock is poisoned".into(),
                })?;
                let read_txn = db
                    .begin_read()
                    .map_err(|error| DatabaseError::QueryFailed {
                        query: "begin_read".into(),
                        reason: error.to_string(),
                    })?;
                let files = read_txn.open_table(FILES_TABLE).map_err(|error| {
                    DatabaseError::QueryFailed {
                        query: "open_table".into(),
                        reason: error.to_string(),
                    }
                })?;
                for entry in files.iter().map_err(|error| DatabaseError::QueryFailed {
                    query: "iter".into(),
                    reason: error.to_string(),
                })? {
                    let (_, bytes) = entry.map_err(|error| DatabaseError::QueryFailed {
                        query: "next".into(),
                        reason: error.to_string(),
                    })?;
                    let file = Self::deserialize_media_file(bytes.value()).map_err(|error| {
                        DatabaseError::QueryFailed {
                            query: "deserialize".into(),
                            reason: error.to_string(),
                        }
                    })?;
                    if sender.blocking_send(Ok(file)).is_err() {
                        return Ok(());
                    }
                }
                Ok(())
            };
            if let Err(error) = operation() {
                let _ = sender.blocking_send(Err(error));
            }
        });
        Box::pin(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub(in crate::database::redb) async fn remove_media_file_impl(
        &self,
        path: &Path,
    ) -> Result<bool> {
        Ok(self
            .bulk_remove_media_files_impl(&[path.to_path_buf()])
            .await?
            > 0)
    }

    pub(in crate::database::redb) async fn update_media_file_impl(
        &self,
        file: &MediaFile,
    ) -> Result<()> {
        if file.id.is_none() {
            return Err(anyhow!("Cannot update file without ID"));
        }
        self.bulk_store_media_files_impl(std::slice::from_ref(file))
            .await?;
        Ok(())
    }

    pub(in crate::database::redb) async fn get_files_in_directory_impl(
        &self,
        dir: &Path,
    ) -> Result<Vec<MediaFile>> {
        let dir_key = Self::canonical_path(dir)?.to_string_lossy().to_string();
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let files_table = read_txn.open_table(FILES_TABLE)?;
            let directory_paths = read_txn.open_table(DIRECTORY_PATH_INDEX)?;
            let directory_files = read_txn.open_multimap_table(DIRECTORY_FILES)?;
            let file_ids = if let Some(directory_id) = directory_paths.get(dir_key.as_str())? {
                directory_files
                    .get(directory_id.value())?
                    .map(|value| value.map(|value| value.value()))
                    .collect::<std::result::Result<Vec<_>, _>>()?
            } else {
                Vec::new()
            };

            let mut files = Vec::new();
            for file_id in file_ids {
                if let Some(data) = files_table.get(file_id)? {
                    let file = Self::deserialize_media_file(data.value())?;
                    files.push(file);
                }
            }

            Ok(files)
        })
        .await
    }

    pub(in crate::database::redb) async fn get_directory_listing_impl(
        &self,
        parent_path: &Path,
        media_type_filter: &str,
    ) -> Result<(Vec<MediaDirectory>, Vec<MediaFile>)> {
        let canonical_parent = Self::canonical_path(parent_path)?;
        let raw_parent_str = canonical_parent.to_string_lossy().to_string();

        // Strip trailing slash if present, unless it's the root path "/"
        let parent_str = if raw_parent_str.len() > 1
            && (raw_parent_str.ends_with('/') || raw_parent_str.ends_with('\\'))
        {
            raw_parent_str[..raw_parent_str.len() - 1].to_string()
        } else {
            raw_parent_str
        };

        debug!(
            "get_directory_listing: querying for parent_path='{}' (raw='{}'), filter='{}'",
            parent_str,
            parent_path.to_string_lossy(),
            media_type_filter
        );
        let media_type_filter = media_type_filter.to_owned();
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let files_table = read_txn.open_table(FILES_TABLE)?;
            let directory_paths = read_txn.open_table(DIRECTORY_PATH_INDEX)?;
            let directory_records = read_txn.open_table(DIRECTORY_RECORDS)?;
            let directory_children = read_txn.open_multimap_table(DIRECTORY_CHILDREN)?;
            let directory_files = read_txn.open_multimap_table(DIRECTORY_FILES)?;
            let directory_mime_counts = read_txn.open_table(DIRECTORY_MIME_COUNTS)?;

            let Some(parent_id) = directory_paths
                .get(parent_str.as_str())?
                .map(|value| value.value())
            else {
                return Ok((Vec::new(), Vec::new()));
            };

            let mut files = Vec::new();

            // Get files in this directory
            let file_ids = directory_files
                .get(parent_id)?
                .map(|value| value.map(|value| value.value()))
                .collect::<std::result::Result<Vec<_>, _>>()?;

            debug!(
                "get_directory_listing: found {} file IDs for dir '{}'",
                file_ids.len(),
                parent_str
            );

            for file_id in file_ids {
                if let Some(data) = files_table.get(file_id)? {
                    let file = Self::deserialize_media_file(data.value())?;
                    if media_type_filter.is_empty()
                        || file.mime_type.starts_with(&media_type_filter)
                    {
                        files.push(file);
                    }
                }
            }

            let count_family = if media_type_filter.is_empty() {
                "*"
            } else {
                media_type_filter.as_str()
            };
            let mut directories = Vec::new();
            for child in directory_children.get(parent_id)? {
                let child_id = child?.value();
                let count_key = Self::mime_count_key(child_id, count_family);
                if directory_mime_counts
                    .get(count_key.as_str())?
                    .is_none_or(|count| count.value() == 0)
                {
                    continue;
                }
                if let Some(path) = directory_records.get(child_id)? {
                    let path = path.value().to_owned();
                    let name = PathBuf::from(&path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    directories.push(MediaDirectory {
                        path: PathBuf::from(&path),
                        name,
                    });
                }
            }

            // Sort subdirectories naturally (so e.g. "Season 2" < "Season 10")
            directories.sort_by(|a, b| crate::natural_cmp(&a.name, &b.name));

            // Sort files by track number if available, then naturally by filename
            files.sort_by(|a, b| match (a.track_number, b.track_number) {
                (Some(ta), Some(tb)) if ta != tb => ta.cmp(&tb),
                _ => crate::natural_cmp(&a.filename, &b.filename),
            });

            Ok((directories, files))
        })
        .await
    }

    pub(in crate::database::redb) async fn cleanup_missing_files_impl(
        &self,
        existing_paths: &[PathBuf],
    ) -> Result<usize> {
        let existing_set: HashSet<String> = existing_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        // First, collect all paths to remove
        let paths_to_remove: Vec<PathBuf> = self
            .execute_read(move |database| {
                let read_txn = database.begin_read()?;
                let path_index = read_txn.open_table(PATH_INDEX)?;

                let mut paths = Vec::new();
                for entry in path_index.iter()? {
                    let (key, _) = entry?;
                    if !existing_set.contains(key.value()) {
                        paths.push(PathBuf::from(key.value()));
                    }
                }
                Ok(paths)
            })
            .await?;

        // Use batch removal
        self.bulk_remove_media_files_impl(&paths_to_remove).await
    }

    pub(in crate::database::redb) async fn get_file_by_path_impl(
        &self,
        path: &Path,
    ) -> Result<Option<MediaFile>> {
        let path_str = Self::canonical_path(path)?.to_string_lossy().to_string();
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let path_index = read_txn.open_table(PATH_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            if let Some(file_id) = path_index.get(path_str.as_str())?.map(|v| v.value()) {
                if let Some(data) = files_table.get(file_id)? {
                    return Ok(Some(Self::deserialize_media_file(data.value())?));
                }
            }

            Ok(None)
        })
        .await
    }

    pub(in crate::database::redb) async fn get_file_by_id_impl(
        &self,
        id: i64,
    ) -> Result<Option<MediaFile>> {
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            if let Some(data) = files_table.get(id)? {
                return Ok(Some(Self::deserialize_media_file(data.value())?));
            }

            Ok(None)
        })
        .await
    }
}

//! Media CRUD, bulk indexing, directory queries, and music categorization.

use super::*;

impl RedbDatabase {
    pub(super) async fn read_impl<R, F>(self: Arc<Self>, operation: F) -> Result<R>
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

    pub(super) async fn store_media_file_impl(&self, file: &MediaFile) -> Result<i64> {
        self.bulk_store_media_files_impl(std::slice::from_ref(file))
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("media upsert returned no ID"))
    }

    pub(super) async fn get_file_location_by_id_impl(
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

    pub(super) async fn load_file_fingerprints_impl(&self) -> Result<Vec<FileFingerprint>> {
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
                    modified: UNIX_EPOCH
                        + Duration::new(
                            view.modified_secs(),
                            view.modified_nanos().min(999_999_999),
                        ),
                    created_at: UNIX_EPOCH + Duration::from_secs(view.created_at_secs()),
                    subtitle_available: view.subtitle_available(),
                });
            }
            Ok(fingerprints)
        })
        .await
    }

    pub(super) async fn load_file_fingerprints_under_root_impl(
        &self,
        root: &Path,
    ) -> Result<Vec<FileFingerprint>> {
        let root_str = root
            .to_string_lossy()
            .trim_end_matches(['/', '\\'])
            .to_string();
        let child_prefix = format!("{}{sep}", root_str, sep = std::path::MAIN_SEPARATOR);
        self.execute_read(move |database| {
            let transaction = database.begin_read()?;
            let files = transaction.open_table(FILES_TABLE)?;
            let paths = transaction.open_table(PATH_INDEX)?;
            let mut fingerprints = Vec::new();
            for entry in paths.range(root_str.as_str()..)? {
                let (path, id) = entry?;
                let path = path.value();
                if path != root_str && !path.starts_with(&child_prefix) {
                    if !path.starts_with(&root_str) {
                        break;
                    }
                    continue;
                }
                let Some(bytes) = files.get(id.value())? else {
                    continue;
                };
                let view = RedbReadSession::view(bytes.value())?;
                fingerprints.push(FileFingerprint {
                    id: id.value(),
                    path: PathBuf::from(view.path()),
                    size: view.size(),
                    modified: UNIX_EPOCH
                        + Duration::new(
                            view.modified_secs(),
                            view.modified_nanos().min(999_999_999),
                        ),
                    created_at: UNIX_EPOCH + Duration::from_secs(view.created_at_secs()),
                    subtitle_available: view.subtitle_available(),
                });
            }
            Ok(fingerprints)
        })
        .await
    }

    pub(super) fn stream_all_media_files_impl(
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

    pub(super) async fn remove_media_file_impl(&self, path: &Path) -> Result<bool> {
        Ok(self
            .bulk_remove_media_files_impl(&[path.to_path_buf()])
            .await?
            > 0)
    }

    pub(super) async fn update_media_file_impl(&self, file: &MediaFile) -> Result<()> {
        if file.id.is_none() {
            return Err(anyhow!("Cannot update file without ID"));
        }
        self.bulk_store_media_files_impl(std::slice::from_ref(file))
            .await?;
        Ok(())
    }

    pub(super) async fn get_files_in_directory_impl(&self, dir: &Path) -> Result<Vec<MediaFile>> {
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

    pub(super) async fn get_directory_listing_impl(
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

            // Sort subdirectories case-insensitively using natural sort
            directories.sort_by_cached_key(|directory| {
                crate::natural_sort::natural_sort_key(&directory.name.to_lowercase())
            });

            // Sort files by track number if available, then case-insensitively by filename using natural sort
            files.sort_by(|a, b| match (a.track_number, b.track_number) {
                (Some(ta), Some(tb)) if ta != tb => ta.cmp(&tb),
                _ => {
                    let a_key = crate::natural_sort::natural_sort_key(&a.filename.to_lowercase());
                    let b_key = crate::natural_sort::natural_sort_key(&b.filename.to_lowercase());
                    a_key.cmp(&b_key)
                }
            });

            Ok((directories, files))
        })
        .await
    }

    pub(super) async fn cleanup_missing_files_impl(
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

    pub(super) async fn get_file_by_path_impl(&self, path: &Path) -> Result<Option<MediaFile>> {
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

    pub(super) async fn get_file_by_id_impl(&self, id: i64) -> Result<Option<MediaFile>> {
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

    pub(super) async fn get_artists_impl(&self) -> Result<Vec<MusicCategory>> {
        self.execute_read(|database| {
            let read_txn = database.begin_read()?;
            let artist_index = read_txn.open_multimap_table(ARTIST_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut categories = Vec::new();
            for result in artist_index.iter()? {
                let (key, value) = result?;
                let artist_name = key.value().to_string();
                let mut count = 0;
                for id in value {
                    if files_table.get(id?.value())?.is_some() {
                        count += 1;
                    }
                }
                if count == 0 {
                    continue;
                }
                categories.push(MusicCategory {
                    id: artist_name.clone(),
                    name: artist_name,
                    category_type: MusicCategoryType::Artist,
                    count,
                });
            }
            Ok(categories)
        })
        .await
    }

    pub(super) async fn get_albums_impl(
        &self,
        artist_filter: Option<&str>,
    ) -> Result<Vec<MusicCategory>> {
        let artist_filter = artist_filter.map(str::to_owned);
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let album_index = read_txn.open_multimap_table(ALBUM_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut categories = Vec::new();
            for result in album_index.iter()? {
                let (key, value) = result?;
                let album_name = key.value().to_string();
                let file_ids = value
                    .map(|id| id.map(|id| id.value()))
                    .collect::<std::result::Result<Vec<_>, _>>()?;

                let count = if let Some(artist) = artist_filter.as_deref() {
                    let mut matched = 0;
                    for fid in file_ids {
                        if let Some(data) = files_table.get(fid)? {
                            let file = RedbReadSession::view(data.value())?;
                            if file.artist() == Some(artist) {
                                matched += 1;
                            }
                        }
                    }
                    matched
                } else {
                    let mut existing = 0;
                    for id in file_ids {
                        if files_table.get(id)?.is_some() {
                            existing += 1;
                        }
                    }
                    existing
                };

                if count > 0 {
                    categories.push(MusicCategory {
                        id: album_name.clone(),
                        name: album_name,
                        category_type: MusicCategoryType::Album,
                        count,
                    });
                }
            }
            Ok(categories)
        })
        .await
    }

    pub(super) async fn get_genres_impl(&self) -> Result<Vec<MusicCategory>> {
        self.execute_read(|database| {
            let read_txn = database.begin_read()?;
            let genre_index = read_txn.open_multimap_table(GENRE_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut categories = Vec::new();
            for result in genre_index.iter()? {
                let (key, value) = result?;
                let name = key.value().to_string();
                let mut count = 0;
                for id in value {
                    if files_table.get(id?.value())?.is_some() {
                        count += 1;
                    }
                }
                if count == 0 {
                    continue;
                }
                categories.push(MusicCategory {
                    id: name.clone(),
                    name,
                    category_type: MusicCategoryType::Genre,
                    count,
                });
            }
            Ok(categories)
        })
        .await
    }

    pub(super) async fn get_years_impl(&self) -> Result<Vec<MusicCategory>> {
        self.execute_read(|database| {
            let read_txn = database.begin_read()?;
            let year_index = read_txn.open_multimap_table(YEAR_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut categories = Vec::new();
            for result in year_index.iter()? {
                let (key, value) = result?;
                let year = key.value();
                let mut count = 0;
                for id in value {
                    if files_table.get(id?.value())?.is_some() {
                        count += 1;
                    }
                }
                if count == 0 {
                    continue;
                }
                categories.push(MusicCategory {
                    id: year.to_string(),
                    name: year.to_string(),
                    category_type: MusicCategoryType::Year,
                    count,
                });
            }
            Ok(categories)
        })
        .await
    }

    pub(super) async fn get_album_artists_impl(&self) -> Result<Vec<MusicCategory>> {
        self.execute_read(|database| {
            let read_txn = database.begin_read()?;
            let album_artist_index = read_txn.open_multimap_table(ALBUM_ARTIST_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut categories = Vec::new();
            for result in album_artist_index.iter()? {
                let (key, value) = result?;
                let name = key.value().to_string();
                let mut count = 0;
                for id in value {
                    if files_table.get(id?.value())?.is_some() {
                        count += 1;
                    }
                }
                if count == 0 {
                    continue;
                }
                categories.push(MusicCategory {
                    id: name.clone(),
                    name,
                    category_type: MusicCategoryType::AlbumArtist,
                    count,
                });
            }
            Ok(categories)
        })
        .await
    }

    pub(super) async fn get_music_by_artist_impl(&self, artist: &str) -> Result<Vec<MediaFile>> {
        let artist = artist.to_owned();
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let artist_index = read_txn.open_multimap_table(ARTIST_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut files = Vec::new();
            for fid in artist_index.get(artist.as_str())? {
                let fid = fid?.value();
                if let Some(data) = files_table.get(fid)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
            Ok(files)
        })
        .await
    }

    pub(super) async fn get_music_by_album_impl(
        &self,
        album: &str,
        artist: Option<&str>,
    ) -> Result<Vec<MediaFile>> {
        let album = album.to_owned();
        let artist = artist.map(str::to_owned);
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let album_index = read_txn.open_multimap_table(ALBUM_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut files = Vec::new();
            for fid in album_index.get(album.as_str())? {
                let fid = fid?.value();
                if let Some(data) = files_table.get(fid)? {
                    let file = Self::deserialize_media_file(data.value())?;
                    if let Some(art) = artist.as_deref() {
                        if file.artist.as_deref() != Some(art) {
                            continue;
                        }
                    }
                    files.push(file);
                }
            }
            Ok(files)
        })
        .await
    }

    pub(super) async fn get_music_by_genre_impl(&self, genre: &str) -> Result<Vec<MediaFile>> {
        let genre = genre.to_owned();
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let genre_index = read_txn.open_multimap_table(GENRE_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut files = Vec::new();
            for fid in genre_index.get(genre.as_str())? {
                let fid = fid?.value();
                if let Some(data) = files_table.get(fid)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
            Ok(files)
        })
        .await
    }

    pub(super) async fn get_music_by_year_impl(&self, year: u32) -> Result<Vec<MediaFile>> {
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let year_index = read_txn.open_multimap_table(YEAR_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut files = Vec::new();
            for fid in year_index.get(year)? {
                let fid = fid?.value();
                if let Some(data) = files_table.get(fid)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
            Ok(files)
        })
        .await
    }

    pub(super) async fn get_music_by_album_artist_impl(
        &self,
        album_artist: &str,
    ) -> Result<Vec<MediaFile>> {
        let album_artist = album_artist.to_owned();
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let album_artist_index = read_txn.open_multimap_table(ALBUM_ARTIST_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut files = Vec::new();
            for fid in album_artist_index.get(album_artist.as_str())? {
                let fid = fid?.value();
                if let Some(data) = files_table.get(fid)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
            Ok(files)
        })
        .await
    }

    pub(super) async fn get_files_by_paths_impl(
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

    pub(super) async fn bulk_store_media_files_impl(
        &self,
        files: &[MediaFile],
    ) -> Result<Vec<i64>> {
        self.bulk_store_media_files_with_mode(files, false).await
    }

    pub(super) async fn bulk_store_canonical_media_files_impl(
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
                    let mut ordered_files = write_txn.open_table(DIRECTORY_FILES_BY_NAME)?;
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
                                &mut ordered_files,
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
                            &mut ordered_files,
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

    pub(super) async fn bulk_update_media_files_impl(&self, files: &[MediaFile]) -> Result<()> {
        if files.iter().any(|file| file.id.is_none()) {
            return Err(anyhow!("cannot update a media file without an ID"));
        }
        self.bulk_store_media_files_impl(files).await?;
        Ok(())
    }

    pub(super) async fn bulk_update_canonical_media_files_impl(
        &self,
        files: &[MediaFile],
    ) -> Result<()> {
        if files.iter().any(|file| file.id.is_none()) {
            return Err(anyhow!("cannot update a media file without an ID"));
        }
        self.bulk_store_canonical_media_files_impl(files).await?;
        Ok(())
    }

    pub(super) async fn bulk_remove_media_files_impl(&self, paths: &[PathBuf]) -> Result<usize> {
        let paths = paths
            .iter()
            .map(|path| Self::canonical_path(path).map(|path| path.to_string_lossy().to_string()))
            .collect::<Result<Vec<_>>>()?;
        self.bulk_remove_canonical_path_strings_impl(paths).await
    }

    pub(super) async fn bulk_remove_canonical_media_files_impl(
        &self,
        paths: &[PathBuf],
    ) -> Result<usize> {
        let paths = paths
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect();
        self.bulk_remove_canonical_path_strings_impl(paths).await
    }

    async fn bulk_remove_canonical_path_strings_impl(&self, paths: Vec<String>) -> Result<usize> {
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

    pub(super) async fn remove_media_under_path_impl(&self, path: &Path) -> Result<RemovalSummary> {
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

    pub(super) async fn get_files_with_path_prefix_impl(
        &self,
        canonical_prefix: &str,
    ) -> Result<Vec<MediaFile>> {
        let mut files = Vec::new();
        let canonical = Self::canonical_path(Path::new(canonical_prefix))?;
        let prefix = canonical
            .to_string_lossy()
            .trim_end_matches('/')
            .to_string();
        let child = format!("{prefix}/");

        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let path_index = read_txn.open_table(PATH_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            for result in path_index.range(prefix.as_str()..)? {
                let (key, value) = result?;
                if key.value() == prefix || key.value().starts_with(&child) {
                    if let Some(data) = files_table.get(value.value())? {
                        files.push(Self::deserialize_media_file(data.value())?);
                    }
                } else {
                    break;
                }
            }

            Ok(files)
        })
        .await
    }

    pub(super) async fn get_direct_subdirectories_impl(
        &self,
        canonical_parent_path: &str,
    ) -> Result<Vec<MediaDirectory>> {
        let canonical = Self::canonical_path(Path::new(canonical_parent_path))?;
        let canonical_parent_path = canonical.to_string_lossy().to_string();

        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let paths = read_txn.open_table(DIRECTORY_PATH_INDEX)?;
            let records = read_txn.open_table(DIRECTORY_RECORDS)?;
            let children = read_txn.open_multimap_table(DIRECTORY_CHILDREN)?;
            let counts = read_txn.open_table(DIRECTORY_MIME_COUNTS)?;
            let Some(parent_id) = paths
                .get(canonical_parent_path.as_str())?
                .map(|value| value.value())
            else {
                return Ok(Vec::new());
            };

            let mut result = Vec::new();
            for child in children.get(parent_id)? {
                let child_id = child?.value();
                let count_key = Self::mime_count_key(child_id, "*");
                if counts
                    .get(count_key.as_str())?
                    .is_none_or(|value| value.value() == 0)
                {
                    continue;
                }
                if let Some(path) = records.get(child_id)? {
                    let path = path.value().to_owned();
                    let name = PathBuf::from(&path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    result.push(MediaDirectory {
                        path: PathBuf::from(&path),
                        name,
                    });
                }
            }
            Ok(result)
        })
        .await
    }

    pub(super) async fn batch_cleanup_missing_files_impl(
        &self,
        existing_canonical_paths: &HashSet<String>,
    ) -> Result<usize> {
        let paths_vec: Vec<PathBuf> = existing_canonical_paths.iter().map(PathBuf::from).collect();
        self.cleanup_missing_files_impl(&paths_vec).await
    }

    pub(super) async fn database_native_cleanup_impl(
        &self,
        existing_canonical_paths: &[String],
    ) -> Result<usize> {
        let existing_set: HashSet<String> = existing_canonical_paths.iter().cloned().collect();
        let paths_vec: Vec<PathBuf> = existing_set.iter().map(PathBuf::from).collect();
        self.cleanup_missing_files_impl(&paths_vec).await
    }

    pub(super) async fn get_filtered_direct_subdirectories_impl(
        &self,
        canonical_parent_path: &str,
        mime_filter: &str,
    ) -> Result<Vec<MediaDirectory>> {
        Ok(self
            .get_directory_listing(Path::new(canonical_parent_path), mime_filter)
            .await?
            .0)
    }
}

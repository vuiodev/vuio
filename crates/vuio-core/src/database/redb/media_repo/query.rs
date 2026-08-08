use super::super::*;

impl RedbDatabase {
    pub(in crate::database::redb) async fn get_files_with_path_prefix_impl(
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

    pub(in crate::database::redb) async fn get_direct_subdirectories_impl(
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

    pub(in crate::database::redb) async fn batch_cleanup_missing_files_impl(
        &self,
        existing_canonical_paths: &HashSet<String>,
    ) -> Result<usize> {
        let paths_vec: Vec<PathBuf> = existing_canonical_paths.iter().map(PathBuf::from).collect();
        self.cleanup_missing_files_impl(&paths_vec).await
    }

    pub(in crate::database::redb) async fn database_native_cleanup_impl(
        &self,
        existing_canonical_paths: &[String],
    ) -> Result<usize> {
        let existing_set: HashSet<String> = existing_canonical_paths.iter().cloned().collect();
        let paths_vec: Vec<PathBuf> = existing_set.iter().map(PathBuf::from).collect();
        self.cleanup_missing_files_impl(&paths_vec).await
    }

    pub(in crate::database::redb) async fn get_filtered_direct_subdirectories_impl(
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

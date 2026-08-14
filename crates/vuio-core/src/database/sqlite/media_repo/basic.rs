//! Single-record reads and writes.

use anyhow::Result;
use futures_util::Stream;
use rusqlite::OptionalExtension;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use crate::database::sqlite::query;
use crate::database::sqlite::schema::{
    self, FILE_LOCATION_COLUMNS, FINGERPRINT_COLUMNS, MEDIA_COLUMNS,
};
use crate::database::sqlite::SqliteDatabase;
use crate::database::{FileFingerprint, FileLocation, MediaDirectory, MediaFile, MediaFileQuery};

impl SqliteDatabase {
    pub(in crate::database::sqlite) async fn store_media_file_impl(
        &self,
        file: &MediaFile,
    ) -> Result<i64> {
        Ok(self
            .bulk_store_media_files_impl(std::slice::from_ref(file), false)
            .await?
            .into_iter()
            .next()
            .expect("storing one record yields one identifier"))
    }

    pub(in crate::database::sqlite) async fn update_media_file_impl(
        &self,
        file: &MediaFile,
    ) -> Result<()> {
        self.bulk_update_media_files_impl(std::slice::from_ref(file), false)
            .await
    }

    pub(in crate::database::sqlite) async fn remove_media_file_impl(
        &self,
        path: &Path,
    ) -> Result<bool> {
        Ok(self
            .bulk_remove_media_files_impl(&[path.to_path_buf()])
            .await?
            > 0)
    }

    pub(in crate::database::sqlite) async fn get_file_by_path_impl(
        &self,
        path: &Path,
    ) -> Result<Option<MediaFile>> {
        let path = Self::canonical_string(path)?;
        self.execute_read(move |connection| {
            Ok(connection
                .prepare_cached(&format!(
                    "SELECT {MEDIA_COLUMNS} FROM media_files WHERE path = ?"
                ))?
                .query_row([&path], schema::media_file_from_row)
                .optional()?)
        })
        .await
    }

    pub(in crate::database::sqlite) async fn get_file_by_id_impl(
        &self,
        id: i64,
    ) -> Result<Option<MediaFile>> {
        self.execute_read(move |connection| {
            Ok(connection
                .prepare_cached(&format!(
                    "SELECT {MEDIA_COLUMNS} FROM media_files WHERE id = ?"
                ))?
                .query_row([id], schema::media_file_from_row)
                .optional()?)
        })
        .await
    }

    pub(in crate::database::sqlite) async fn get_file_location_by_id_impl(
        &self,
        id: i64,
    ) -> Result<Option<FileLocation>> {
        self.execute_read(move |connection| {
            Ok(connection
                .prepare_cached(&format!(
                    "SELECT {FILE_LOCATION_COLUMNS} FROM media_files WHERE id = ?"
                ))?
                .query_row([id], schema::file_location_from_row)
                .optional()?)
        })
        .await
    }

    pub(in crate::database::sqlite) async fn load_file_fingerprints_impl(
        &self,
    ) -> Result<Vec<FileFingerprint>> {
        self.execute_read(move |connection| {
            let mut statement = connection.prepare_cached(&format!(
                "SELECT {FINGERPRINT_COLUMNS} FROM media_files ORDER BY id"
            ))?;
            let fingerprints = statement
                .query_map([], schema::fingerprint_from_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(fingerprints)
        })
        .await
    }

    pub(in crate::database::sqlite) async fn load_file_fingerprints_under_impl(
        &self,
        canonical_prefix: String,
    ) -> Result<Vec<FileFingerprint>> {
        self.execute_read(move |connection| {
            // A range on `path` rather than a `LIKE`, so the primary key index
            // serves it. `subtree_range` stops at a component boundary, which is
            // what keeps `/media/Film` from matching `/media/Films`.
            let (start, end) = SqliteDatabase::subtree_range(&canonical_prefix);
            let mut statement = connection.prepare_cached(&format!(
                "SELECT {FINGERPRINT_COLUMNS} FROM media_files WHERE path >= ? AND path < ?"
            ))?;
            let fingerprints = statement
                .query_map([&start, &end], schema::fingerprint_from_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(fingerprints)
        })
        .await
    }

    pub(in crate::database::sqlite) async fn get_files_by_paths_impl(
        &self,
        paths: &[PathBuf],
    ) -> Result<Vec<MediaFile>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let canonical = paths
            .iter()
            .map(|path| Self::canonical_string(path))
            .collect::<Result<Vec<_>>>()?;

        self.execute_read(move |connection| {
            let mut statement = connection.prepare_cached(&format!(
                "SELECT {MEDIA_COLUMNS} FROM media_files WHERE path = ?"
            ))?;
            let mut files = Vec::with_capacity(canonical.len());
            for path in &canonical {
                if let Some(file) = statement
                    .query_row([path], schema::media_file_from_row)
                    .optional()?
                {
                    files.push(file);
                }
            }
            Ok(files)
        })
        .await
    }

    pub(in crate::database::sqlite) async fn get_files_in_directory_impl(
        &self,
        directory: &Path,
    ) -> Result<Vec<MediaFile>> {
        let directory = Self::canonical_string(directory)?;
        self.query_media(MediaFileQuery::Directory {
            path: directory,
            mime_family: None,
        })
        .await
    }

    pub(in crate::database::sqlite) async fn get_directory_listing_impl(
        &self,
        parent: &Path,
        media_type_filter: &str,
    ) -> Result<(Vec<MediaDirectory>, Vec<MediaFile>)> {
        let parent = Self::canonical_string(parent)?;
        let filter = media_type_filter.to_owned();

        let directories = self
            .get_filtered_direct_subdirectories_impl(&parent, &filter)
            .await?;
        let files = self
            .query_media(MediaFileQuery::Directory {
                path: parent,
                mime_family: Some(filter),
            })
            .await?;
        Ok((directories, files))
    }

    pub(in crate::database::sqlite) async fn cleanup_missing_files_impl(
        &self,
        existing_paths: &[PathBuf],
    ) -> Result<usize> {
        let surviving = existing_paths
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        self.cleanup_missing_impl(surviving).await
    }

    /// Run a planned query and materialize every matching record.
    pub(in crate::database::sqlite) async fn query_media(
        &self,
        query: MediaFileQuery,
    ) -> Result<Vec<MediaFile>> {
        self.execute_read(move |connection| {
            let plan = query::plan(&query);
            let mut statement = connection.prepare_cached(&plan.select_sql())?;
            let files = statement
                .query_map(
                    rusqlite::params_from_iter(plan.params.iter()),
                    schema::media_file_from_row,
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(files)
        })
        .await
    }

    /// Stream every record without holding the whole library in memory.
    ///
    /// Records are paged by identifier rather than held open in one statement,
    /// so a slow consumer cannot pin a read transaction — and with it the WAL —
    /// for the length of the walk.
    pub(in crate::database::sqlite) fn stream_all_media_files_impl(
        &self,
    ) -> Pin<Box<dyn Stream<Item = Result<MediaFile>> + Send + '_>> {
        const PAGE: usize = 512;

        Box::pin(async_stream::try_stream! {
            let mut after = 0_i64;
            loop {
                let page = self
                    .execute_read(move |connection| {
                        let mut statement = connection.prepare_cached(&format!(
                            "SELECT {MEDIA_COLUMNS} FROM media_files \
                             WHERE id > ? ORDER BY id LIMIT ?"
                        ))?;
                        let files = statement
                            .query_map(rusqlite::params![after, PAGE as i64],
                                       schema::media_file_from_row)?
                            .collect::<rusqlite::Result<Vec<_>>>()?;
                        Ok(files)
                    })
                    .await?;

                let Some(last) = page.last().and_then(|file| file.id) else {
                    break;
                };
                after = last;
                for file in page {
                    yield file;
                }
            }
        })
    }
}

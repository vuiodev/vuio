//! Directory browsing and subtree queries.

use anyhow::Result;
use std::path::PathBuf;

use crate::database::sqlite::query::{self, MimeFilter};
use crate::database::sqlite::schema::{self, MEDIA_COLUMNS};
use crate::database::sqlite::session::directory_listing_sql;
use crate::database::sqlite::SqliteDatabase;
use crate::database::{MediaDirectory, MediaFile};

impl SqliteDatabase {
    pub(in crate::database::sqlite) async fn get_direct_subdirectories_impl(
        &self,
        canonical_parent: &str,
    ) -> Result<Vec<MediaDirectory>> {
        self.get_filtered_direct_subdirectories_impl(canonical_parent, "")
            .await
    }

    pub(in crate::database::sqlite) async fn get_filtered_direct_subdirectories_impl(
        &self,
        canonical_parent: &str,
        mime_filter: &str,
    ) -> Result<Vec<MediaDirectory>> {
        let parent = canonical_parent.to_owned();
        let filter = mime_filter.to_owned();

        self.execute_read(move |connection| {
            let (sql, params) = directory_listing_sql(&parent, &MimeFilter::parse(Some(&filter)));
            let mut statement = connection.prepare_cached(&sql)?;
            let directories = statement
                .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                    let path: String = row.get(0)?;
                    let name: String = row.get(1)?;
                    Ok(MediaDirectory {
                        path: PathBuf::from(path),
                        name,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(directories)
        })
        .await
    }

    pub(in crate::database::sqlite) async fn get_files_with_path_prefix_impl(
        &self,
        canonical_prefix: &str,
    ) -> Result<Vec<MediaFile>> {
        let prefix = canonical_prefix.to_owned();

        self.execute_read(move |connection| {
            let (predicate, params) = query::subtree_params(&prefix);
            // The prefix may name a file as easily as a directory.
            let mut statement = connection.prepare_cached(&format!(
                "SELECT {MEDIA_COLUMNS} FROM media_files \
                 WHERE media_files.path = ? OR ({predicate}) \
                 ORDER BY media_files.path"
            ))?;
            let mut bound = vec![rusqlite::types::Value::Text(prefix.clone())];
            bound.extend(params);
            let files = statement
                .query_map(
                    rusqlite::params_from_iter(bound.iter()),
                    schema::media_file_from_row,
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(files)
        })
        .await
    }
}

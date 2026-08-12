//! Scoped read sessions lending borrowed record views.
//!
//! The DLNA layer inspects far more records than it returns, so views borrow
//! straight out of the statement's result buffer instead of materializing a
//! `MediaFile` per candidate row.

use anyhow::{Context, Result};
use rusqlite::Row;

use super::query::{self, MimeFilter};
use super::schema::column;
use super::PooledConnection;
use crate::database::{
    DatabaseReadSession, DirectoryView, MediaFileQuery, MediaFileView, PlaylistView, VisitSummary,
};

/// A read transaction plus the connection it runs on.
///
/// The transaction is opened with raw SQL rather than `rusqlite::Transaction`
/// because that type borrows the connection, which would make this struct
/// self-referential for no gain.
pub struct SqliteReadSession {
    connection: PooledConnection,
    open: bool,
}

impl SqliteReadSession {
    pub(super) fn begin(connection: PooledConnection) -> Result<Self> {
        connection
            .execute_batch("BEGIN DEFERRED")
            .context("Failed to open a SQLite read transaction")?;
        Ok(Self {
            connection,
            open: true,
        })
    }
}

impl Drop for SqliteReadSession {
    fn drop(&mut self) {
        if self.open {
            // A read transaction has nothing to roll back; ending it simply
            // releases the snapshot so the WAL can be checkpointed.
            let _ = self.connection.execute_batch("COMMIT");
            self.open = false;
        }
    }
}

/// A media record borrowed from the current row.
pub struct SqliteMediaFileView<'a> {
    row: &'a Row<'a>,
}

impl SqliteMediaFileView<'_> {
    fn text(&self, index: usize) -> &str {
        self.row
            .get_ref(index)
            .ok()
            .and_then(|value| value.as_str().ok())
            .unwrap_or_default()
    }

    fn optional_text(&self, index: usize) -> Option<&str> {
        self.row
            .get_ref(index)
            .ok()
            .and_then(|value| value.as_str_or_null().ok())
            .flatten()
    }

    fn integer(&self, index: usize) -> i64 {
        self.row
            .get_ref(index)
            .ok()
            .and_then(|value| value.as_i64().ok())
            .unwrap_or_default()
    }

    fn optional_integer(&self, index: usize) -> Option<i64> {
        self.row
            .get_ref(index)
            .ok()
            .and_then(|value| value.as_i64_or_null().ok())
            .flatten()
    }
}

impl MediaFileView for SqliteMediaFileView<'_> {
    fn id(&self) -> Option<i64> {
        Some(self.integer(column::ID))
    }
    fn path(&self) -> &str {
        self.text(column::PATH)
    }
    fn filename(&self) -> &str {
        self.text(column::FILENAME)
    }
    fn size(&self) -> u64 {
        self.integer(column::SIZE).max(0) as u64
    }
    fn modified_secs(&self) -> u64 {
        self.integer(column::MODIFIED_SECS).max(0) as u64
    }
    fn mime_type(&self) -> &str {
        self.text(column::MIME_TYPE)
    }
    fn duration_secs(&self) -> Option<f64> {
        self.row
            .get_ref(column::DURATION_SECS)
            .ok()
            .and_then(|value| value.as_f64_or_null().ok())
            .flatten()
    }
    fn title(&self) -> Option<&str> {
        self.optional_text(column::TITLE)
    }
    fn artist(&self) -> Option<&str> {
        self.optional_text(column::ARTIST)
    }
    fn album(&self) -> Option<&str> {
        self.optional_text(column::ALBUM)
    }
    fn genre(&self) -> Option<&str> {
        self.optional_text(column::GENRE)
    }
    fn track_number(&self) -> Option<u32> {
        self.optional_integer(column::TRACK_NUMBER)
            .map(|value| value as u32)
    }
    fn year(&self) -> Option<u32> {
        self.optional_integer(column::YEAR).map(|value| value as u32)
    }
    fn album_artist(&self) -> Option<&str> {
        self.optional_text(column::ALBUM_ARTIST)
    }
    fn subtitle_available(&self) -> bool {
        self.integer(column::SUBTITLE_AVAILABLE) != 0
    }
    fn created_at_secs(&self) -> u64 {
        self.integer(column::CREATED_AT_SECS).max(0) as u64
    }
    fn updated_at_secs(&self) -> u64 {
        self.integer(column::UPDATED_AT_SECS).max(0) as u64
    }
}

/// A playlist record borrowed from the current row.
pub struct SqlitePlaylistView<'a> {
    row: &'a Row<'a>,
}

impl PlaylistView for SqlitePlaylistView<'_> {
    fn id(&self) -> Option<i64> {
        self.row.get_ref(0).ok().and_then(|v| v.as_i64().ok())
    }
    fn name(&self) -> &str {
        self.row
            .get_ref(1)
            .ok()
            .and_then(|v| v.as_str().ok())
            .unwrap_or_default()
    }
    fn description(&self) -> Option<&str> {
        self.row
            .get_ref(2)
            .ok()
            .and_then(|v| v.as_str_or_null().ok())
            .flatten()
    }
    fn created_at_secs(&self) -> u64 {
        self.row
            .get_ref(3)
            .ok()
            .and_then(|v| v.as_i64().ok())
            .unwrap_or_default()
            .max(0) as u64
    }
    fn updated_at_secs(&self) -> u64 {
        self.row
            .get_ref(4)
            .ok()
            .and_then(|v| v.as_i64().ok())
            .unwrap_or_default()
            .max(0) as u64
    }
}

/// A directory record borrowed from the current row.
pub struct SqliteDirectoryView<'a> {
    row: &'a Row<'a>,
}

impl DirectoryView for SqliteDirectoryView<'_> {
    fn id(&self) -> u64 {
        // Directories are keyed by path; the numeric id exists only because
        // the trait predates that and nothing consumes it.
        0
    }
    fn path(&self) -> &str {
        self.row
            .get_ref(0)
            .ok()
            .and_then(|v| v.as_str().ok())
            .unwrap_or_default()
    }
    fn name(&self) -> &str {
        self.row
            .get_ref(1)
            .ok()
            .and_then(|v| v.as_str().ok())
            .unwrap_or_default()
    }
}

impl DatabaseReadSession for SqliteReadSession {
    type File<'a>
        = SqliteMediaFileView<'a>
    where
        Self: 'a;
    type Playlist<'a>
        = SqlitePlaylistView<'a>
    where
        Self: 'a;
    type Directory<'a>
        = SqliteDirectoryView<'a>
    where
        Self: 'a;

    fn visit_files<F>(
        &mut self,
        query: &MediaFileQuery,
        offset: usize,
        limit: usize,
        mut visitor: F,
    ) -> Result<VisitSummary>
    where
        F: for<'a> FnMut(Self::File<'a>) -> Result<()>,
    {
        let plan = query::plan(query);
        let params = rusqlite::params_from_iter(plan.params.iter());

        // `matched` is the size of the whole result and drives the DLNA
        // TotalMatches field, so it is counted independently of the page.
        let matched: i64 = self
            .connection
            .prepare_cached(&plan.count_sql())?
            .query_row(params, |row| row.get(0))?;

        let mut summary = VisitSummary {
            matched: matched.max(0) as usize,
            visited: 0,
        };
        if limit == 0 || summary.matched <= offset {
            return Ok(summary);
        }

        let mut page_params = plan.params.clone();
        page_params.push(rusqlite::types::Value::Integer(limit as i64));
        page_params.push(rusqlite::types::Value::Integer(offset as i64));

        let mut statement = self.connection.prepare_cached(&plan.page_sql())?;
        let mut rows = statement.query(rusqlite::params_from_iter(page_params.iter()))?;
        while let Some(row) = rows.next()? {
            visitor(SqliteMediaFileView { row })?;
            summary.visited += 1;
        }
        Ok(summary)
    }

    fn visit_direct_subdirectories<F>(
        &mut self,
        canonical_parent: &str,
        mime_family: Option<&str>,
        offset: usize,
        limit: usize,
        mut visitor: F,
    ) -> Result<VisitSummary>
    where
        F: for<'a> FnMut(Self::Directory<'a>) -> Result<()>,
    {
        let filter = MimeFilter::parse(mime_family);
        let (sql, params) = directory_listing_sql(canonical_parent, &filter);

        let count_sql = format!("SELECT COUNT(*) FROM ({sql})");
        let matched: i64 = self
            .connection
            .prepare_cached(&count_sql)?
            .query_row(rusqlite::params_from_iter(params.iter()), |row| row.get(0))?;

        let mut summary = VisitSummary {
            matched: matched.max(0) as usize,
            visited: 0,
        };
        if limit == 0 || summary.matched <= offset {
            return Ok(summary);
        }

        let mut page_params = params.clone();
        page_params.push(rusqlite::types::Value::Integer(limit as i64));
        page_params.push(rusqlite::types::Value::Integer(offset as i64));

        let mut statement = self
            .connection
            .prepare_cached(&format!("{sql} LIMIT ? OFFSET ?"))?;
        let mut rows = statement.query(rusqlite::params_from_iter(page_params.iter()))?;
        while let Some(row) = rows.next()? {
            visitor(SqliteDirectoryView { row })?;
            summary.visited += 1;
        }
        Ok(summary)
    }

    fn visit_playlists<F>(
        &mut self,
        offset: usize,
        limit: usize,
        mut visitor: F,
    ) -> Result<VisitSummary>
    where
        F: for<'a> FnMut(Self::Playlist<'a>) -> Result<()>,
    {
        let matched: i64 = self
            .connection
            .prepare_cached("SELECT COUNT(*) FROM playlists")?
            .query_row([], |row| row.get(0))?;

        let mut summary = VisitSummary {
            matched: matched.max(0) as usize,
            visited: 0,
        };
        if limit == 0 || summary.matched <= offset {
            return Ok(summary);
        }

        let mut statement = self.connection.prepare_cached(
            "SELECT playlists.id, playlists.name, playlists.description, \
             playlists.created_at_secs, playlists.updated_at_secs \
             FROM playlists ORDER BY playlists.id LIMIT ? OFFSET ?",
        )?;
        let mut rows = statement.query([limit as i64, offset as i64])?;
        while let Some(row) = rows.next()? {
            visitor(SqlitePlaylistView { row })?;
            summary.visited += 1;
        }
        Ok(summary)
    }
}

/// Ordered direct children of a directory, filtered by MIME family.
///
/// A family filter is answered from the maintained counters. A longer prefix
/// such as `audio/mpeg` has no counter, so it falls back to probing the
/// subtree for a single matching record — correct, and rare enough that the
/// extra index seek per child does not matter.
pub(super) fn directory_listing_sql(
    parent: &str,
    filter: &MimeFilter,
) -> (String, Vec<rusqlite::types::Value>) {
    use rusqlite::types::Value;

    let mut params = vec![Value::Text(parent.to_owned())];
    let sql = match filter.counter_family() {
        Some(family) => {
            params.push(Value::Text(family.to_owned()));
            "SELECT directories.path, directories.name FROM directories \
             JOIN directory_mime_counts \
               ON directory_mime_counts.dir_path = directories.path \
             WHERE directories.parent_path = ? \
               AND directory_mime_counts.family = ? \
               AND directory_mime_counts.count > 0 \
             ORDER BY directories.name COLLATE natural_order"
                .to_owned()
        }
        None => {
            let MimeFilter::Prefix(prefix) = filter else {
                unreachable!("only a prefix filter lacks a counter family")
            };
            params.push(Value::Text(format!("{}%", query::escape_like(prefix))));
            "SELECT directories.path, directories.name FROM directories \
             WHERE directories.parent_path = ? \
               AND EXISTS ( \
                   SELECT 1 FROM media_files \
                   WHERE media_files.path >= directories.path || '/' \
                     AND media_files.path <  directories.path || '0' \
                     AND media_files.mime_type LIKE ? ESCAPE '\\' \
               ) \
             ORDER BY directories.name COLLATE natural_order"
                .to_owned()
        }
    };
    (sql, params)
}

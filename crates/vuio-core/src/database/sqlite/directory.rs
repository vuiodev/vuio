//! The directory tree implied by stored file paths.
//!
//! A directory is not a record anyone inserts: it exists because some file
//! beneath it does. Browsing needs to list directories, filter them by the
//! kind of media they contain, and report how much is inside — none of which
//! can be derived from `media_files` alone, because a directory whose files
//! all live in grandchildren has no row there to be found by.
//!
//! So counts are maintained here, in the same transaction as the write that
//! changes them, walking from the file's parent up to the root. Every count is
//! recursive; the `*` family counts records of any kind and decides whether
//! the directory exists at all.

use anyhow::Result;
use rusqlite::Transaction;
use std::collections::HashMap;

use super::SqliteDatabase;

/// Family key counting descendants of every type.
pub(super) const ANY_FAMILY: &str = "*";

/// A pending adjustment to one directory's counters.
#[derive(Default)]
pub(super) struct DirectoryDelta {
    counts: HashMap<(String, String), i64>,
}

impl DirectoryDelta {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Record that a file of `family` was added to (or removed from) `path`.
    ///
    /// Accumulating deltas and applying them once per transaction keeps a bulk
    /// scan from touching the same ancestor row once per file.
    pub(super) fn record(&mut self, file_path: &str, family: &str, delta: i64) {
        let mut current = SqliteDatabase::parent_directory(file_path);
        while let Some(directory) = current {
            *self
                .counts
                .entry((directory.clone(), ANY_FAMILY.to_owned()))
                .or_insert(0) += delta;
            *self
                .counts
                .entry((directory.clone(), family.to_owned()))
                .or_insert(0) += delta;
            current = SqliteDatabase::parent_directory(&directory);
        }
    }

    /// Apply the accumulated counts, creating and pruning directories.
    pub(super) fn apply(self, transaction: &Transaction<'_>) -> Result<()> {
        if self.counts.is_empty() {
            return Ok(());
        }

        // Counters reference their directory, so every directory row has to
        // exist before any count does.
        let directories = self
            .counts
            .keys()
            .map(|(path, _)| path.clone())
            .collect::<std::collections::HashSet<_>>();

        {
            let mut insert = transaction.prepare_cached(
                "INSERT INTO directories (path, parent_path, name) VALUES (?, ?, ?) \
                 ON CONFLICT(path) DO NOTHING",
            )?;
            for path in &directories {
                insert.execute(rusqlite::params![
                    path,
                    SqliteDatabase::parent_directory(path).unwrap_or_default(),
                    SqliteDatabase::directory_name(path),
                ])?;
            }
        }

        {
            let mut upsert = transaction.prepare_cached(
                "INSERT INTO directory_mime_counts (dir_path, family, count) VALUES (?, ?, ?) \
                 ON CONFLICT(dir_path, family) DO UPDATE SET count = count + excluded.count",
            )?;
            for ((path, family), delta) in &self.counts {
                if *delta != 0 {
                    upsert.execute(rusqlite::params![path, family, delta])?;
                }
            }
        }

        prune(transaction)?;
        Ok(())
    }
}

/// Drop counters that have fallen to zero, and directories left with none.
///
/// A directory that no longer holds any media must disappear from browsing
/// entirely rather than linger as an empty container.
pub(super) fn prune(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute("DELETE FROM directory_mime_counts WHERE count <= 0", [])?;
    transaction.execute(
        "DELETE FROM directories WHERE NOT EXISTS ( \
             SELECT 1 FROM directory_mime_counts \
             WHERE directory_mime_counts.dir_path = directories.path \
         )",
        [],
    )?;
    Ok(())
}

/// Rebuild the whole tree from `media_files`.
///
/// Used by index repair, and as the definition of correctness the incremental
/// path is measured against.
pub(super) fn rebuild(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute("DELETE FROM directory_mime_counts", [])?;
    transaction.execute("DELETE FROM directories", [])?;

    let mut delta = DirectoryDelta::new();
    {
        let mut statement =
            transaction.prepare("SELECT path, mime_family FROM media_files")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let path: String = row.get(0)?;
            let family: String = row.get(1)?;
            delta.record(&path, &family, 1);
        }
    }
    delta.apply(transaction)
}

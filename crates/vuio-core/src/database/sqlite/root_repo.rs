//! Availability of configured media roots.

use anyhow::Result;
use rusqlite::OptionalExtension;
use std::path::{Path, PathBuf};

use super::SqliteDatabase;
use crate::database::RootAvailability;

const COLUMNS: &str =
    "path, last_seen_secs, unavailable_since_secs, indexed_count, reason";

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RootAvailability> {
    Ok(RootAvailability {
        path: PathBuf::from(row.get::<_, String>(0)?),
        last_seen_secs: row.get::<_, i64>(1)?.max(0) as u64,
        unavailable_since_secs: row
            .get::<_, Option<i64>>(2)?
            .map(|value| value.max(0) as u64),
        indexed_count: row.get::<_, i64>(3)?.max(0) as u64,
        reason: row.get(4)?,
    })
}

impl SqliteDatabase {
    pub(super) async fn get_root_availability_impl(
        &self,
        path: &Path,
    ) -> Result<Option<RootAvailability>> {
        let path = Self::canonical_string(path)?;
        self.execute_read(move |connection| {
            Ok(connection
                .prepare_cached(&format!(
                    "SELECT {COLUMNS} FROM root_availability WHERE path = ?"
                ))?
                .query_row([&path], from_row)
                .optional()?)
        })
        .await
    }

    pub(super) async fn list_root_availability_impl(&self) -> Result<Vec<RootAvailability>> {
        self.execute_read(move |connection| {
            let mut statement = connection.prepare_cached(&format!(
                "SELECT {COLUMNS} FROM root_availability ORDER BY path"
            ))?;
            let states = statement
                .query_map([], from_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(states)
        })
        .await
    }

    pub(super) async fn set_root_availability_impl(&self, state: &RootAvailability) -> Result<()> {
        let path = Self::canonical_string(&state.path)?;
        let last_seen = state.last_seen_secs as i64;
        let unavailable = state.unavailable_since_secs.map(|value| value as i64);
        let indexed = state.indexed_count as i64;
        let reason = state.reason.clone();

        self.transact(move |transaction| {
            transaction.execute(
                "INSERT INTO root_availability \
                 (path, last_seen_secs, unavailable_since_secs, indexed_count, reason) \
                 VALUES (?, ?, ?, ?, ?) \
                 ON CONFLICT(path) DO UPDATE SET \
                     last_seen_secs = excluded.last_seen_secs, \
                     unavailable_since_secs = excluded.unavailable_since_secs, \
                     indexed_count = excluded.indexed_count, \
                     reason = excluded.reason",
                rusqlite::params![path, last_seen, unavailable, indexed, reason],
            )?;
            Ok(())
        })
        .await
    }

    pub(super) async fn remove_root_availability_impl(&self, path: &Path) -> Result<()> {
        let path = Self::canonical_string(path)?;
        self.transact(move |transaction| {
            transaction.execute("DELETE FROM root_availability WHERE path = ?", [&path])?;
            Ok(())
        })
        .await
    }
}

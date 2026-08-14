//! Radio stations: what this server broadcasts, and whether it should be.

use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use std::time::{SystemTime, UNIX_EPOCH};

use super::schema::{radio_station_from_row, RADIO_STATION_COLUMNS};
use super::SqliteDatabase;
use crate::database::{RadioStation, RadioStationInput};

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

/// A starting point for a station's shuffle, drawn once when it is created.
///
/// Stored rather than generated per run so that a station resumed after a
/// restart continues the order its listeners were already hearing.
fn fresh_seed() -> i64 {
    // The clock is enough: the seed only has to differ between stations, not
    // resist prediction.
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.subsec_nanos() as i64 ^ elapsed.as_secs() as i64)
        .unwrap_or(1)
}

impl SqliteDatabase {
    pub(super) async fn list_radio_stations_impl(&self) -> Result<Vec<RadioStation>> {
        self.execute_read(move |connection| {
            let query = format!("SELECT {RADIO_STATION_COLUMNS} FROM radio_stations ORDER BY id");
            let mut statement = connection.prepare_cached(&query)?;
            let rows = statement.query_map([], radio_station_from_row)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
        .await
    }

    pub(super) async fn get_radio_station_impl(&self, id: i64) -> Result<Option<RadioStation>> {
        self.execute_read(move |connection| {
            let query =
                format!("SELECT {RADIO_STATION_COLUMNS} FROM radio_stations WHERE id = ?");
            Ok(connection
                .prepare_cached(&query)?
                .query_row([id], radio_station_from_row)
                .optional()?)
        })
        .await
    }

    pub(super) async fn create_radio_station_impl(
        &self,
        input: &RadioStationInput,
    ) -> Result<RadioStation> {
        let name = input.name.clone();
        let genre = input.genre.clone();
        let folders = serde_json::to_string(&input.folders)?;
        let mode = input.mode.as_str().to_owned();
        let seed = fresh_seed();
        let now = now_secs();

        let id = self
            .transact(move |transaction| {
                transaction.execute(
                    "INSERT INTO radio_stations \
                     (name, genre, folders, mode, enabled, seed, cursor_path, \
                      created_at_secs, updated_at_secs) \
                     VALUES (?, ?, ?, ?, 0, ?, NULL, ?, ?)",
                    params![name, genre, folders, mode, seed, now, now],
                )?;
                Ok(transaction.last_insert_rowid())
            })
            .await?;

        self.get_radio_station_impl(id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("the station was created but could not be read back"))
    }

    pub(super) async fn update_radio_station_impl(
        &self,
        id: i64,
        input: &RadioStationInput,
    ) -> Result<Option<RadioStation>> {
        let name = input.name.clone();
        let genre = input.genre.clone();
        let folders = serde_json::to_string(&input.folders)?;
        let mode = input.mode.as_str().to_owned();
        let now = now_secs();

        let updated = self
            .transact(move |transaction| {
                Ok(transaction.execute(
                    "UPDATE radio_stations \
                     SET name = ?, genre = ?, folders = ?, mode = ?, updated_at_secs = ? \
                     WHERE id = ?",
                    params![name, genre, folders, mode, now, id],
                )? > 0)
            })
            .await?;

        if !updated {
            return Ok(None);
        }
        self.get_radio_station_impl(id).await
    }

    pub(super) async fn set_radio_station_enabled_impl(&self, id: i64, enabled: bool) -> Result<bool> {
        let now = now_secs();
        self.transact(move |transaction| {
            Ok(transaction.execute(
                "UPDATE radio_stations SET enabled = ?, updated_at_secs = ? WHERE id = ?",
                params![i64::from(enabled), now, id],
            )? > 0)
        })
        .await
    }

    pub(super) async fn set_radio_station_cursor_impl(
        &self,
        id: i64,
        cursor_path: Option<&str>,
    ) -> Result<()> {
        let cursor_path = cursor_path.map(str::to_owned);
        self.transact(move |transaction| {
            // Deliberately not touching `updated_at_secs`: the cursor moves
            // every few minutes on its own, and it is not a change an operator
            // made to the station.
            transaction.execute(
                "UPDATE radio_stations SET cursor_path = ? WHERE id = ?",
                params![cursor_path, id],
            )?;
            Ok(())
        })
        .await
    }

    pub(super) async fn delete_radio_station_impl(&self, id: i64) -> Result<bool> {
        self.transact(move |transaction| {
            Ok(transaction.execute("DELETE FROM radio_stations WHERE id = ?", [id])? > 0)
        })
        .await
    }
}

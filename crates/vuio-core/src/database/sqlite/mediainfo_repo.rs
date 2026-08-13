//! Storage for what public metadata services said about a file.

use anyhow::Result;
use rusqlite::{OptionalExtension, Row};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::SqliteDatabase;
use crate::database::{MediaInfoRecord, MediaInfoStats};

/// Column order shared by every read here, so the index constants below stay
/// meaningful.
const COLUMNS: &str = "\
media_file_id, provider, remote_id, kind, title, original_title, overview, \
release_date, year, rating, genres, season, episode, artwork_key, payload, \
confidence, fetched_at, mediainfo_version";

fn seconds_since_epoch(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

fn record_from_row(row: &Row<'_>) -> rusqlite::Result<MediaInfoRecord> {
    let genres: Option<String> = row.get(10)?;
    Ok(MediaInfoRecord {
        media_file_id: row.get(0)?,
        provider: row.get(1)?,
        remote_id: row.get(2)?,
        kind: row.get(3)?,
        title: row.get(4)?,
        original_title: row.get(5)?,
        overview: row.get(6)?,
        release_date: row.get(7)?,
        year: row.get::<_, Option<i64>>(8)?.map(|year| year as u32),
        rating: row.get(9)?,
        // Stored as a JSON array. A row whose genres will not parse is not worth
        // failing the whole listing over — it comes back with none.
        genres: genres
            .and_then(|genres| serde_json::from_str::<Vec<String>>(&genres).ok())
            .unwrap_or_default(),
        season: row.get::<_, Option<i64>>(11)?.map(|season| season as u32),
        episode: row.get::<_, Option<i64>>(12)?.map(|episode| episode as u32),
        artwork_key: row.get(13)?,
        payload: row.get(14)?,
        confidence: row.get::<_, i64>(15)?.clamp(0, 100) as u8,
        fetched_at: UNIX_EPOCH + Duration::from_secs(row.get::<_, i64>(16)?.max(0) as u64),
        mediainfo_version: row.get::<_, i64>(17)?.max(0) as u32,
    })
}

impl SqliteDatabase {
    pub(super) async fn get_mediainfo_impl(
        &self,
        media_file_id: i64,
    ) -> Result<Option<MediaInfoRecord>> {
        self.execute_read(move |connection| {
            Ok(connection
                .prepare_cached(&format!(
                    "SELECT {COLUMNS} FROM mediainfo WHERE media_file_id = ?"
                ))?
                .query_row([media_file_id], record_from_row)
                .optional()?)
        })
        .await
    }

    pub(super) async fn get_mediainfo_batch_impl(
        &self,
        media_file_ids: &[i64],
    ) -> Result<Vec<MediaInfoRecord>> {
        if media_file_ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = media_file_ids.to_vec();
        self.execute_read(move |connection| {
            // A placeholder list rather than a temp table: browse pages are bounded
            // by the requested count, so this stays well inside SQLite's limit.
            let placeholders = std::iter::repeat_n("?", ids.len()).collect::<Vec<_>>().join(",");
            let mut statement = connection.prepare(&format!(
                "SELECT {COLUMNS} FROM mediainfo WHERE media_file_id IN ({placeholders})"
            ))?;
            let rows = statement.query_map(rusqlite::params_from_iter(ids.iter()), record_from_row)?;
            let mut records = Vec::new();
            for record in rows {
                records.push(record?);
            }
            Ok(records)
        })
        .await
    }

    pub(super) async fn bulk_store_mediainfo_impl(&self, records: &[MediaInfoRecord]) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        let records = records.to_vec();
        self.transact(move |transaction| {
            let mut statement = transaction.prepare_cached(
                "INSERT INTO mediainfo (\
                    media_file_id, provider, remote_id, kind, title, original_title, overview, \
                    release_date, year, rating, genres, season, episode, artwork_key, payload, \
                    confidence, fetched_at, mediainfo_version\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18) \
                 ON CONFLICT(media_file_id) DO UPDATE SET \
                    provider = excluded.provider, remote_id = excluded.remote_id, \
                    kind = excluded.kind, title = excluded.title, \
                    original_title = excluded.original_title, overview = excluded.overview, \
                    release_date = excluded.release_date, year = excluded.year, \
                    rating = excluded.rating, genres = excluded.genres, \
                    season = excluded.season, episode = excluded.episode, \
                    artwork_key = excluded.artwork_key, payload = excluded.payload, \
                    confidence = excluded.confidence, fetched_at = excluded.fetched_at, \
                    mediainfo_version = excluded.mediainfo_version",
            )?;
            for record in &records {
                let genres = serde_json::to_string(&record.genres).unwrap_or_else(|_| "[]".into());
                statement.execute(rusqlite::params![
                    record.media_file_id,
                    record.provider,
                    record.remote_id,
                    record.kind,
                    record.title,
                    record.original_title,
                    record.overview,
                    record.release_date,
                    record.year.map(|year| year as i64),
                    record.rating,
                    genres,
                    record.season.map(|season| season as i64),
                    record.episode.map(|episode| episode as i64),
                    record.artwork_key,
                    record.payload,
                    record.confidence as i64,
                    seconds_since_epoch(record.fetched_at),
                    record.mediainfo_version as i64,
                ])?;
            }
            Ok(())
        })
        .await
    }

    pub(super) async fn list_low_confidence_impl(
        &self,
        threshold: u8,
        limit: usize,
    ) -> Result<Vec<MediaInfoRecord>> {
        self.execute_read(move |connection| {
            let mut statement = connection.prepare_cached(&format!(
                "SELECT {COLUMNS} FROM mediainfo WHERE confidence < ? \
                 ORDER BY confidence ASC, media_file_id ASC LIMIT ?"
            ))?;
            let rows = statement
                .query_map(rusqlite::params![threshold as i64, limit as i64], record_from_row)?;
            let mut records = Vec::new();
            for record in rows {
                records.push(record?);
            }
            Ok(records)
        })
        .await
    }

    pub(super) async fn mediainfo_stats_impl(&self, threshold: u8) -> Result<MediaInfoStats> {
        self.execute_read(move |connection| {
            let row = connection
                .prepare_cached(
                    "SELECT COUNT(*), \
                            COALESCE(SUM(confidence >= ?1), 0), \
                            COALESCE(SUM(confidence <  ?1), 0), \
                            COALESCE(SUM(artwork_key IS NOT NULL), 0) \
                     FROM mediainfo",
                )?
                .query_row([threshold as i64], |row| {
                    Ok(MediaInfoStats {
                        total: row.get::<_, i64>(0)?.max(0) as u64,
                        confident: row.get::<_, i64>(1)?.max(0) as u64,
                        low_confidence: row.get::<_, i64>(2)?.max(0) as u64,
                        with_artwork: row.get::<_, i64>(3)?.max(0) as u64,
                    })
                })?;
            Ok(row)
        })
        .await
    }

    pub(super) async fn clear_mediainfo_impl(&self) -> Result<u64> {
        self.transact(move |transaction| {
            Ok(transaction.execute("DELETE FROM mediainfo", [])? as u64)
        })
        .await
    }

    pub(super) async fn media_ids_missing_mediainfo_impl(
        &self,
        version: u32,
        threshold: u8,
    ) -> Result<Vec<i64>> {
        self.execute_read(move |connection| {
            // Three cases count as "needs looking up": never tried, tried by an
            // older reader, or tried and the answer was not good enough to trust.
            // The last is what lets a run with a new provider key improve on a
            // previous run without clearing the table first.
            let mut statement = connection.prepare_cached(
                "SELECT media_files.id FROM media_files \
                 LEFT JOIN mediainfo ON mediainfo.media_file_id = media_files.id \
                 WHERE mediainfo.media_file_id IS NULL \
                    OR mediainfo.mediainfo_version < ?1 \
                    OR mediainfo.confidence < ?2 \
                 ORDER BY media_files.id",
            )?;
            let rows = statement.query_map(
                rusqlite::params![version as i64, threshold as i64],
                |row| row.get::<_, i64>(0),
            )?;
            let mut ids = Vec::new();
            for id in rows {
                ids.push(id?);
            }
            Ok(ids)
        })
        .await
    }
}

//! Playlists and their ordered entries.
//!
//! Entries are a real table keyed by `(playlist_id, position)`, and both of
//! their foreign keys cascade. Deleting a playlist or a media record therefore
//! cannot leave a dangling entry behind — the case a key-value store has to
//! maintain a reverse index to catch.

use anyhow::{anyhow, Result};
use rusqlite::{OptionalExtension, Transaction};
use std::path::Path;
use std::time::SystemTime;

use super::schema::{self, time_to_seconds, PLAYLIST_COLUMNS};
use super::SqliteDatabase;
use crate::database::{MediaFile, MediaFileQuery, Playlist};

/// Fail unless every referenced record exists.
///
/// The foreign keys would reject the write anyway; checking first turns
/// "FOREIGN KEY constraint failed" into a message naming the record.
fn require_files(transaction: &Transaction<'_>, entries: &[(i64, u32)]) -> Result<()> {
    let mut statement =
        transaction.prepare_cached("SELECT 1 FROM media_files WHERE id = ?")?;
    for (file_id, _) in entries {
        if statement
            .query_row([file_id], |_| Ok(()))
            .optional()?
            .is_none()
        {
            return Err(anyhow!("media file {file_id} not found"));
        }
    }
    Ok(())
}

fn require_playlist(transaction: &Transaction<'_>, playlist_id: i64) -> Result<()> {
    if transaction
        .prepare_cached("SELECT 1 FROM playlists WHERE id = ?")?
        .query_row([playlist_id], |_| Ok(()))
        .optional()?
        .is_none()
    {
        return Err(anyhow!("playlist {playlist_id} not found"));
    }
    Ok(())
}

fn replace_entries(
    transaction: &Transaction<'_>,
    playlist_id: i64,
    entries: &[(i64, u32)],
) -> Result<Vec<i64>> {
    transaction.execute(
        "DELETE FROM playlist_entries WHERE playlist_id = ?",
        [playlist_id],
    )?;
    let mut insert = transaction.prepare_cached(
        "INSERT OR REPLACE INTO playlist_entries (playlist_id, media_file_id, position) \
         VALUES (?, ?, ?)",
    )?;
    let mut ids = Vec::with_capacity(entries.len());
    for (file_id, position) in entries {
        insert.execute(rusqlite::params![playlist_id, file_id, i64::from(*position)])?;
        ids.push(*file_id);
    }
    Ok(ids)
}

impl SqliteDatabase {
    pub(super) async fn create_playlist_impl(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<i64> {
        let name = name.to_owned();
        let description = description.map(str::to_owned);
        let now = time_to_seconds(SystemTime::now());

        self.transact(move |transaction| {
            transaction.execute(
                "INSERT INTO playlists (name, description, created_at_secs, updated_at_secs) \
                 VALUES (?, ?, ?, ?)",
                rusqlite::params![name, description, now, now],
            )?;
            Ok(transaction.last_insert_rowid())
        })
        .await
    }

    pub(super) async fn get_playlists_impl(&self) -> Result<Vec<Playlist>> {
        self.execute_read(move |connection| {
            let mut statement = connection.prepare_cached(&format!(
                "SELECT {PLAYLIST_COLUMNS} FROM playlists ORDER BY playlists.id"
            ))?;
            let playlists = statement
                .query_map([], schema::playlist_from_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(playlists)
        })
        .await
    }

    pub(super) async fn get_playlist_impl(&self, playlist_id: i64) -> Result<Option<Playlist>> {
        self.execute_read(move |connection| {
            Ok(connection
                .prepare_cached(&format!(
                    "SELECT {PLAYLIST_COLUMNS} FROM playlists WHERE playlists.id = ?"
                ))?
                .query_row([playlist_id], schema::playlist_from_row)
                .optional()?)
        })
        .await
    }

    pub(super) async fn update_playlist_impl(&self, playlist: &Playlist) -> Result<()> {
        let Some(playlist_id) = playlist.id else {
            return Err(anyhow!("Cannot update playlist without ID"));
        };
        let name = playlist.name.clone();
        let description = playlist.description.clone();
        let updated = time_to_seconds(playlist.updated_at);

        self.transact(move |transaction| {
            let changed = transaction.execute(
                "UPDATE playlists SET name = ?, description = ?, updated_at_secs = ? WHERE id = ?",
                rusqlite::params![name, description, updated, playlist_id],
            )?;
            if changed == 0 {
                return Err(anyhow!("playlist {playlist_id} not found"));
            }
            Ok(())
        })
        .await
    }

    pub(super) async fn delete_playlist_impl(&self, playlist_id: i64) -> Result<bool> {
        self.transact(move |transaction| {
            // Entries go with the playlist through ON DELETE CASCADE.
            Ok(transaction.execute("DELETE FROM playlists WHERE id = ?", [playlist_id])? > 0)
        })
        .await
    }

    pub(super) async fn set_playlist_source_impl(
        &self,
        playlist_id: i64,
        source_path: &Path,
    ) -> Result<()> {
        let source = Self::canonical_string(source_path)?;
        self.transact(move |transaction| {
            let changed = transaction.execute(
                "UPDATE playlists SET source_path = ? WHERE id = ?",
                rusqlite::params![source, playlist_id],
            )?;
            if changed == 0 {
                return Err(anyhow!("playlist {playlist_id} not found"));
            }
            Ok(())
        })
        .await
    }

    pub(super) async fn replace_playlist_from_source_impl(
        &self,
        source_path: &Path,
        name: &str,
        media_file_ids: &[(i64, u32)],
    ) -> Result<i64> {
        let source = Self::canonical_string(source_path)?;
        let name = name.to_owned();
        let entries = media_file_ids.to_vec();
        let now = time_to_seconds(SystemTime::now());

        self.transact(move |transaction| {
            require_files(transaction, &entries)?;

            // Re-importing a source updates the playlist it produced last time
            // rather than adding a second one beside it.
            let existing: Option<i64> = transaction
                .prepare_cached("SELECT MIN(id) FROM playlists WHERE source_path = ?")?
                .query_row([&source], |row| row.get::<_, Option<i64>>(0))
                .optional()?
                .flatten();

            let playlist_id = match existing {
                Some(id) => {
                    transaction.execute(
                        "UPDATE playlists SET name = ?, updated_at_secs = ? WHERE id = ?",
                        rusqlite::params![name, now, id],
                    )?;
                    id
                }
                None => {
                    transaction.execute(
                        "INSERT INTO playlists \
                         (name, description, source_path, created_at_secs, updated_at_secs) \
                         VALUES (?, NULL, ?, ?, ?)",
                        rusqlite::params![name, source, now, now],
                    )?;
                    transaction.last_insert_rowid()
                }
            };

            replace_entries(transaction, playlist_id, &entries)?;
            Ok(playlist_id)
        })
        .await
    }

    pub(super) async fn remove_derived_content_by_source_impl(
        &self,
        source_path: &Path,
    ) -> Result<usize> {
        let source = Self::canonical_string(source_path)?;

        self.transact(move |transaction| {
            let (start, end) = SqliteDatabase::subtree_range(&source);
            let mut removed = transaction.execute(
                "DELETE FROM playlists \
                 WHERE source_path = ?1 OR (source_path >= ?2 AND source_path < ?3)",
                rusqlite::params![&source, &start, &end],
            )?;

            // Radio entries are media records whose album names the playlist
            // file they came from, so they are derived content too.
            let mut delta = crate::database::sqlite::directory::DirectoryDelta::new();
            {
                let mut statement = transaction.prepare(
                    "DELETE FROM media_files \
                     WHERE mime_type = 'audio/radio' \
                       AND (album = ?1 OR (album >= ?2 AND album < ?3)) \
                     RETURNING path, mime_family",
                )?;
                let mut rows = statement.query(rusqlite::params![&source, &start, &end])?;
                while let Some(row) = rows.next()? {
                    delta.record(&row.get::<_, String>(0)?, &row.get::<_, String>(1)?, -1);
                    removed += 1;
                }
            }
            delta.apply(transaction)?;
            Ok(removed)
        })
        .await
    }

    pub(super) async fn add_to_playlist_impl(
        &self,
        playlist_id: i64,
        media_file_id: i64,
        position: Option<u32>,
    ) -> Result<i64> {
        let position = i64::from(position.unwrap_or(0));

        self.transact(move |transaction| {
            require_playlist(transaction, playlist_id)?;
            require_files(transaction, &[(media_file_id, 0)])?;
            transaction.execute(
                "INSERT OR REPLACE INTO playlist_entries (playlist_id, media_file_id, position) \
                 VALUES (?, ?, ?)",
                rusqlite::params![playlist_id, media_file_id, position],
            )?;
            Ok(media_file_id)
        })
        .await
    }

    pub(super) async fn batch_add_to_playlist_impl(
        &self,
        playlist_id: i64,
        media_file_ids: &[(i64, u32)],
    ) -> Result<Vec<i64>> {
        let entries = media_file_ids.to_vec();

        self.transact(move |transaction| {
            require_playlist(transaction, playlist_id)?;
            require_files(transaction, &entries)?;
            let mut insert = transaction.prepare_cached(
                "INSERT OR REPLACE INTO playlist_entries (playlist_id, media_file_id, position) \
                 VALUES (?, ?, ?)",
            )?;
            let mut ids = Vec::with_capacity(entries.len());
            for (file_id, position) in &entries {
                insert.execute(rusqlite::params![playlist_id, file_id, i64::from(*position)])?;
                ids.push(*file_id);
            }
            Ok(ids)
        })
        .await
    }

    pub(super) async fn remove_from_playlist_impl(
        &self,
        playlist_id: i64,
        media_file_id: i64,
    ) -> Result<bool> {
        self.transact(move |transaction| {
            Ok(transaction.execute(
                "DELETE FROM playlist_entries WHERE playlist_id = ? AND media_file_id = ?",
                rusqlite::params![playlist_id, media_file_id],
            )? > 0)
        })
        .await
    }

    pub(super) async fn get_playlist_tracks_impl(&self, playlist_id: i64) -> Result<Vec<MediaFile>> {
        self.query_media(MediaFileQuery::Playlist(playlist_id)).await
    }

    pub(super) async fn reorder_playlist_impl(
        &self,
        playlist_id: i64,
        track_positions: &[(i64, u32)],
    ) -> Result<()> {
        let entries = track_positions.to_vec();

        self.transact(move |transaction| {
            require_playlist(transaction, playlist_id)?;
            require_files(transaction, &entries)?;
            replace_entries(transaction, playlist_id, &entries)?;
            Ok(())
        })
        .await
    }
}

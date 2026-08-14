//! Batched writes, and the single record write they are built from.
//!
//! A library scan is the heaviest thing this server does, so every path here
//! commits one transaction for the whole batch and reuses prepared statements
//! across records.

use anyhow::Result;
use rusqlite::{types::Value, OptionalExtension, Transaction};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::database::sqlite::directory::{prune, DirectoryDelta};
use crate::database::sqlite::schema::time_to_seconds;
use crate::database::sqlite::SqliteDatabase;
use crate::database::{MediaFile, RemovalSummary};

pub(in crate::database::sqlite) const INSERT_MEDIA: &str = "\
INSERT INTO media_files (
    id, path, parent_path, filename, size, modified_secs, mime_type, mime_family,
    duration_secs, title, artist, album, genre, track_number, year, album_artist,
    subtitle_available, created_at_secs, updated_at_secs,
    disc_number, disc_total, track_total, composer, comment, bpm, compilation,
    sort_title, sort_artist, sort_album, release_date,
    musicbrainz_track_id, musicbrainz_album_id, musicbrainz_artist_id,
    codec, sample_rate, channels, bits_per_sample, bit_rate, tags_version
) VALUES (?39, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
          ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35,
          ?36, ?37, ?38)";

pub(in crate::database::sqlite) const UPDATE_MEDIA: &str = "\
UPDATE media_files SET
    path = ?1, parent_path = ?2, filename = ?3, size = ?4, modified_secs = ?5,
    mime_type = ?6, mime_family = ?7, duration_secs = ?8, title = ?9, artist = ?10,
    album = ?11, genre = ?12, track_number = ?13, year = ?14, album_artist = ?15,
    subtitle_available = ?16, created_at_secs = ?17, updated_at_secs = ?18,
    disc_number = ?19, disc_total = ?20, track_total = ?21, composer = ?22,
    comment = ?23, bpm = ?24, compilation = ?25, sort_title = ?26,
    sort_artist = ?27, sort_album = ?28, release_date = ?29,
    musicbrainz_track_id = ?30, musicbrainz_album_id = ?31, musicbrainz_artist_id = ?32,
    codec = ?33, sample_rate = ?34, channels = ?35, bits_per_sample = ?36,
    bit_rate = ?37, tags_version = ?38
WHERE id = ?39";

/// The thirty-eight stored fields, in the order both statements bind them.
pub(in crate::database::sqlite) fn bind_media_file(file: &MediaFile) -> Vec<Value> {
    let path = file.path.to_string_lossy().into_owned();
    let parent = SqliteDatabase::parent_directory(&path).unwrap_or_default();
    let family = SqliteDatabase::mime_family(&file.mime_type).to_owned();

    vec![
        Value::Text(path),
        Value::Text(parent),
        Value::Text(file.filename.clone()),
        Value::Integer(file.size as i64),
        Value::Integer(time_to_seconds(file.modified)),
        Value::Text(file.mime_type.clone()),
        Value::Text(family),
        match file.duration {
            Some(duration) => Value::Real(duration.as_secs_f64()),
            None => Value::Null,
        },
        optional_text(&file.title),
        optional_text(&file.artist),
        optional_text(&file.album),
        optional_text(&file.genre),
        optional_integer(file.track_number),
        optional_integer(file.year),
        optional_text(&file.album_artist),
        Value::Integer(i64::from(file.subtitle_available)),
        Value::Integer(time_to_seconds(file.created_at)),
        Value::Integer(time_to_seconds(file.updated_at)),
        optional_integer(file.tags.disc_number),
        optional_integer(file.tags.disc_total),
        optional_integer(file.tags.track_total),
        optional_text(&file.tags.composer),
        optional_text(&file.tags.comment),
        optional_integer(file.tags.bpm),
        match file.tags.compilation {
            Some(flag) => Value::Integer(i64::from(flag)),
            None => Value::Null,
        },
        optional_text(&file.tags.sort_title),
        optional_text(&file.tags.sort_artist),
        optional_text(&file.tags.sort_album),
        optional_text(&file.tags.release_date),
        optional_text(&file.tags.musicbrainz_track_id),
        optional_text(&file.tags.musicbrainz_album_id),
        optional_text(&file.tags.musicbrainz_artist_id),
        optional_text(&file.stream.codec),
        optional_integer(file.stream.sample_rate),
        optional_integer(file.stream.channels.map(u32::from)),
        optional_integer(file.stream.bits_per_sample.map(u32::from)),
        optional_integer(file.stream.bit_rate),
        Value::Integer(i64::from(file.tags_version)),
    ]
}

/// Replace the long tail of tags for one record.
///
/// For an existing record the clear is unconditional, so that its tag state stays
/// internally consistent: the same write that empties the promoted columns empties
/// the side table with them. Guarding that on `tags_version` would leave a file
/// whose tags became unreadable — a truncated download, a corrupted re-encode —
/// with empty columns but its old `media_tags` rows still answering
/// `get_media_tags`.
///
/// A row that was just inserted has no tags to clear, and saying so saves an index
/// lookup and a statement per file on a first scan of a large library.
fn write_extra_tags(
    transaction: &Transaction<'_>,
    media_file_id: i64,
    file: &MediaFile,
    row_existed: bool,
) -> Result<()> {
    if row_existed {
        transaction.execute(
            "DELETE FROM media_tags WHERE media_file_id = ?",
            [media_file_id],
        )?;
    }
    if file.extra_tags.is_empty() {
        return Ok(());
    }

    let mut insert = transaction.prepare_cached(
        "INSERT OR IGNORE INTO media_tags (media_file_id, key, value) VALUES (?, ?, ?)",
    )?;
    for (key, value) in &file.extra_tags {
        insert.execute(rusqlite::params![media_file_id, key, value])?;
    }
    Ok(())
}

fn optional_text(value: &Option<String>) -> Value {
    match value {
        Some(value) => Value::Text(value.clone()),
        None => Value::Null,
    }
}

fn optional_integer(value: Option<u32>) -> Value {
    match value {
        Some(value) => Value::Integer(i64::from(value)),
        None => Value::Null,
    }
}

/// Insert or replace one record, accumulating the directory counts it changes.
///
/// A record is matched by path first and by identifier second, which is what
/// lets a rename move an existing row instead of duplicating it.
pub(in crate::database::sqlite) fn upsert_media_file(
    transaction: &Transaction<'_>,
    file: &MediaFile,
    delta: &mut DirectoryDelta,
) -> Result<i64> {
    let path = file.path.to_string_lossy().into_owned();
    let family = SqliteDatabase::mime_family(&file.mime_type).to_owned();

    let existing = transaction
        .prepare_cached("SELECT id, path, mime_family FROM media_files WHERE path = ?")?
        .query_row([&path], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .optional()?;

    let existing = match existing {
        Some(found) => Some(found),
        None => match file.id {
            Some(id) => transaction
                .prepare_cached("SELECT id, path, mime_family FROM media_files WHERE id = ?")?
                .query_row([id], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .optional()?,
            None => None,
        },
    };

    let mut params = bind_media_file(file);
    match existing {
        Some((id, old_path, old_family)) => {
            // Withdraw the old contribution and add the new one. When neither
            // path nor family changed these cancel, and the accumulated delta
            // for the directory is zero.
            delta.record(&old_path, &old_family, -1);
            delta.record(&path, &family, 1);
            params.push(Value::Integer(id));
            transaction
                .prepare_cached(UPDATE_MEDIA)?
                .execute(rusqlite::params_from_iter(params.iter()))?;
            write_extra_tags(transaction, id, file, true)?;
            Ok(id)
        }
        None => {
            delta.record(&path, &family, 1);
            params.push(match file.id {
                Some(id) => Value::Integer(id),
                None => Value::Null,
            });
            transaction
                .prepare_cached(INSERT_MEDIA)?
                .execute(rusqlite::params_from_iter(params.iter()))?;
            let id = transaction.last_insert_rowid();
            write_extra_tags(transaction, id, file, false)?;
            Ok(id)
        }
    }
}

impl SqliteDatabase {
    fn prepared_records(files: &[MediaFile], already_canonical: bool) -> Result<Vec<MediaFile>> {
        files
            .iter()
            .map(|file| {
                if already_canonical {
                    Ok(file.clone())
                } else {
                    Self::canonical_file(file)
                }
            })
            .collect()
    }

    pub(in crate::database::sqlite) async fn bulk_store_media_files_impl(
        &self,
        files: &[MediaFile],
        already_canonical: bool,
    ) -> Result<Vec<i64>> {
        if files.is_empty() {
            return Ok(Vec::new());
        }
        let records = Self::prepared_records(files, already_canonical)?;

        self.transact(move |transaction| {
            let mut delta = DirectoryDelta::new();
            let mut ids = Vec::with_capacity(records.len());
            for file in &records {
                ids.push(upsert_media_file(transaction, file, &mut delta)?);
            }
            delta.apply(transaction)?;
            Ok(ids)
        })
        .await
    }

    pub(in crate::database::sqlite) async fn bulk_update_media_files_impl(
        &self,
        files: &[MediaFile],
        already_canonical: bool,
    ) -> Result<()> {
        if files.is_empty() {
            return Ok(());
        }
        self.bulk_store_media_files_impl(files, already_canonical)
            .await?;
        Ok(())
    }

    pub(in crate::database::sqlite) async fn bulk_remove_media_files_impl(
        &self,
        paths: &[PathBuf],
    ) -> Result<usize> {
        if paths.is_empty() {
            return Ok(0);
        }
        let canonical = paths
            .iter()
            .map(|path| Self::canonical_string(path))
            .collect::<Result<Vec<_>>>()?;

        self.transact(move |transaction| {
            let mut delta = DirectoryDelta::new();
            let mut removed = 0;
            {
                let mut statement = transaction.prepare_cached(
                    "DELETE FROM media_files WHERE path = ? RETURNING path, mime_family",
                )?;
                for path in &canonical {
                    let mut rows = statement.query([path])?;
                    while let Some(row) = rows.next()? {
                        delta.record(&row.get::<_, String>(0)?, &row.get::<_, String>(1)?, -1);
                        removed += 1;
                    }
                }
            }
            delta.apply(transaction)?;
            Ok(removed)
        })
        .await
    }

    pub(in crate::database::sqlite) async fn remove_media_under_path_impl(
        &self,
        path: &Path,
    ) -> Result<RemovalSummary> {
        let prefix = Self::canonical_string(path)?;

        self.transact(move |transaction| {
            let (start, end) = SqliteDatabase::subtree_range(&prefix);
            let mut summary = RemovalSummary::default();
            let mut delta = DirectoryDelta::new();
            let mut parents = HashSet::new();

            {
                // The path itself may name a file rather than a directory, so
                // both the exact match and the subtree are removed.
                let mut statement = transaction.prepare_cached(
                    "DELETE FROM media_files \
                     WHERE path = ?1 OR (path >= ?2 AND path < ?3) \
                     RETURNING path, mime_type, mime_family",
                )?;
                let mut rows = statement.query(rusqlite::params![&prefix, &start, &end])?;
                while let Some(row) = rows.next()? {
                    let removed_path: String = row.get(0)?;
                    let mime_type: String = row.get(1)?;
                    let family: String = row.get(2)?;
                    if let Some(parent) = Path::new(&removed_path).parent() {
                        parents.insert(parent.to_path_buf());
                    }
                    summary
                        .mime_families
                        .insert(format!("{}/", SqliteDatabase::mime_family(&mime_type)));
                    delta.record(&removed_path, &family, -1);
                    summary.removed_files += 1;
                }
            }

            delta.apply(transaction)?;

            // Directory rows can outlive their files if a previous run was
            // interrupted; clearing the subtree keeps browsing free of empty
            // containers either way.
            let pruned = transaction.execute(
                "DELETE FROM directories WHERE path = ?1 OR (path >= ?2 AND path < ?3)",
                rusqlite::params![&prefix, &start, &end],
            )?;
            if pruned > 0 {
                if let Some(parent) = Path::new(&prefix).parent() {
                    parents.insert(parent.to_path_buf());
                }
            }
            prune(transaction)?;

            summary.affected_parents = parents.into_iter().collect();
            summary.affected_parents.sort();
            Ok(summary)
        })
        .await
    }

    /// Delete every record whose path is not in `surviving`.
    ///
    /// The comparison happens inside the database against a temporary table
    /// rather than by loading every path into the process, which is what makes
    /// this affordable on a large library.
    pub(in crate::database::sqlite) async fn cleanup_missing_impl(
        &self,
        surviving: Vec<String>,
    ) -> Result<usize> {
        self.transact(move |transaction| {
            transaction.execute_batch(
                "CREATE TEMP TABLE IF NOT EXISTS surviving_paths (path TEXT PRIMARY KEY);
                 DELETE FROM surviving_paths;",
            )?;
            {
                let mut insert = transaction.prepare_cached(
                    "INSERT OR IGNORE INTO surviving_paths (path) VALUES (?)",
                )?;
                for path in &surviving {
                    insert.execute([path])?;
                }
            }

            let mut delta = DirectoryDelta::new();
            let mut removed = 0;
            {
                let mut statement = transaction.prepare(
                    "DELETE FROM media_files \
                     WHERE path NOT IN (SELECT path FROM surviving_paths) \
                     RETURNING path, mime_family",
                )?;
                let mut rows = statement.query([])?;
                while let Some(row) = rows.next()? {
                    delta.record(&row.get::<_, String>(0)?, &row.get::<_, String>(1)?, -1);
                    removed += 1;
                }
            }
            delta.apply(transaction)?;
            transaction.execute_batch("DELETE FROM surviving_paths")?;
            Ok(removed)
        })
        .await
    }
}

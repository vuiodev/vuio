//! Table definitions, connection setup, and row decoding.
//!
//! The schema is deliberately relational: the media index is one table with
//! ordinary indexes, and the directory tree is the only derived state the
//! engine cannot compute for us — a directory whose files all live in
//! grandchildren has no row of its own to be found by.

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags, Row};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::database::{FileFingerprint, FileLocation, MediaFile, Playlist};

/// Bumped only for a change that makes an existing file unreadable.
///
/// Startup treats a mismatch the way it treats corruption: the file is
/// quarantined and rebuilt from a rescan, so this is a last resort rather than
/// a routine migration mechanism.
pub(super) const SCHEMA_VERSION: i64 = 1;

/// Name of the collation that carries the application's natural ordering into
/// SQL. Registered on every connection; see [`register_collations`].
///
/// Not simply `natural`: that is a SQLite keyword (`NATURAL JOIN`), and an
/// index declared with it fails to parse.
pub(super) const NATURAL: &str = "natural_order";

/// The schema, with the collation name substituted so it cannot drift from
/// the name the connections actually register.
pub(super) fn ddl() -> String {
    DDL_TEMPLATE.replace("{NATURAL}", NATURAL)
}

const DDL_TEMPLATE: &str = r#"
CREATE TABLE IF NOT EXISTS media_files (
    id                 INTEGER PRIMARY KEY,
    path               TEXT    NOT NULL UNIQUE,
    parent_path        TEXT    NOT NULL,
    filename           TEXT    NOT NULL,
    size               INTEGER NOT NULL,
    modified_secs      INTEGER NOT NULL,
    mime_type          TEXT    NOT NULL,
    mime_family        TEXT    NOT NULL,
    duration_secs      REAL,
    title              TEXT,
    artist             TEXT,
    album              TEXT,
    genre              TEXT,
    track_number       INTEGER,
    year               INTEGER,
    album_artist       TEXT,
    subtitle_available INTEGER NOT NULL DEFAULT 0,
    created_at_secs    INTEGER NOT NULL,
    updated_at_secs    INTEGER NOT NULL,
    -- Browse ordering is "track number, then natural filename", with untagged
    -- records last. Materializing the rank keeps that an index scan instead of
    -- a sort over the whole directory.
    track_sort         INTEGER GENERATED ALWAYS AS (COALESCE(track_number, 4294967296)) STORED
) STRICT;

CREATE INDEX IF NOT EXISTS idx_media_dir_order
    ON media_files(parent_path, track_sort, filename COLLATE {NATURAL});
CREATE INDEX IF NOT EXISTS idx_media_dir_family
    ON media_files(parent_path, mime_family);
CREATE INDEX IF NOT EXISTS idx_media_album
    ON media_files(album, track_sort, filename COLLATE {NATURAL});
CREATE INDEX IF NOT EXISTS idx_media_artist       ON media_files(artist);
CREATE INDEX IF NOT EXISTS idx_media_genre        ON media_files(genre);
CREATE INDEX IF NOT EXISTS idx_media_year         ON media_files(year);
CREATE INDEX IF NOT EXISTS idx_media_album_artist ON media_files(album_artist);
CREATE INDEX IF NOT EXISTS idx_media_family       ON media_files(mime_family);

-- Directories exist only by implication from the paths of files, so unlike the
-- music indexes they cannot be recomputed by a query at browse time.
CREATE TABLE IF NOT EXISTS directories (
    path        TEXT PRIMARY KEY,
    parent_path TEXT NOT NULL,
    name        TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_directories_parent
    ON directories(parent_path, name COLLATE {NATURAL});

-- Recursive per-family record counts, maintained with the writes that change
-- them. `family = '*'` counts every descendant regardless of type; it is what
-- decides whether a directory exists at all.
CREATE TABLE IF NOT EXISTS directory_mime_counts (
    dir_path TEXT    NOT NULL REFERENCES directories(path) ON DELETE CASCADE,
    family   TEXT    NOT NULL,
    count    INTEGER NOT NULL,
    PRIMARY KEY (dir_path, family)
) STRICT;

CREATE TABLE IF NOT EXISTS playlists (
    id              INTEGER PRIMARY KEY,
    name            TEXT    NOT NULL,
    description     TEXT,
    source_path     TEXT,
    created_at_secs INTEGER NOT NULL,
    updated_at_secs INTEGER NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_playlists_source
    ON playlists(source_path) WHERE source_path IS NOT NULL;

CREATE TABLE IF NOT EXISTS playlist_entries (
    playlist_id   INTEGER NOT NULL REFERENCES playlists(id)   ON DELETE CASCADE,
    media_file_id INTEGER NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    position      INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, position)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_playlist_entries_file
    ON playlist_entries(media_file_id);

CREATE TABLE IF NOT EXISTS root_availability (
    path                   TEXT PRIMARY KEY,
    last_seen_secs         INTEGER NOT NULL,
    unavailable_since_secs INTEGER,
    indexed_count          INTEGER NOT NULL,
    reason                 TEXT    NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS secrets (
    key   TEXT PRIMARY KEY,
    value BLOB NOT NULL
) STRICT;
"#;

/// Columns of `media_files`, qualified so the list can be used inside joins.
pub(super) const MEDIA_COLUMNS: &str = "\
media_files.id, media_files.path, media_files.filename, media_files.size, \
media_files.modified_secs, media_files.mime_type, media_files.duration_secs, \
media_files.title, media_files.artist, media_files.album, media_files.genre, \
media_files.track_number, media_files.year, media_files.album_artist, \
media_files.subtitle_available, media_files.created_at_secs, media_files.updated_at_secs";

/// Positions within [`MEDIA_COLUMNS`], shared by the owned decoder and the
/// borrowed views so the two can never drift apart.
pub(super) mod column {
    pub const ID: usize = 0;
    pub const PATH: usize = 1;
    pub const FILENAME: usize = 2;
    pub const SIZE: usize = 3;
    pub const MODIFIED_SECS: usize = 4;
    pub const MIME_TYPE: usize = 5;
    pub const DURATION_SECS: usize = 6;
    pub const TITLE: usize = 7;
    pub const ARTIST: usize = 8;
    pub const ALBUM: usize = 9;
    pub const GENRE: usize = 10;
    pub const TRACK_NUMBER: usize = 11;
    pub const YEAR: usize = 12;
    pub const ALBUM_ARTIST: usize = 13;
    pub const SUBTITLE_AVAILABLE: usize = 14;
    pub const CREATED_AT_SECS: usize = 15;
    pub const UPDATED_AT_SECS: usize = 16;
}

/// Open one connection and put it in the state every caller expects.
///
/// Every connection needs identical treatment: the collation because indexes
/// are declared with it and SQLite refuses to read them otherwise, and the
/// pragmas because they are per-connection rather than stored in the file.
pub(super) fn open_connection(path: &std::path::Path, cache_mb: usize) -> Result<Connection> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("Failed to open SQLite database {}", path.display()))?;

    register_collations(&connection)?;
    apply_pragmas(&connection, cache_mb)?;
    Ok(connection)
}

/// Teach a connection the application's ordering.
///
/// `natural_cmp` is the same function the Rust code sorts with, so an ordered
/// query and an in-memory sort cannot disagree. The cost is that the indexes
/// declared `COLLATE natural` are unreadable by tools that do not define it —
/// the `sqlite3` shell included.
pub(super) fn register_collations(connection: &Connection) -> Result<()> {
    connection
        .create_collation(NATURAL, |left: &str, right: &str| {
            crate::natural_cmp(left, right)
        })
        .context("Failed to register the natural collation")?;
    Ok(())
}

fn apply_pragmas(connection: &Connection, cache_mb: usize) -> Result<()> {
    // A negative cache_size is a KiB budget rather than a page count, which is
    // what the configuration actually expresses.
    let cache_kib = (cache_mb.max(1) * 1024) as i64;
    connection
        .execute_batch(&format!(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;
             PRAGMA temp_store = MEMORY;
             PRAGMA cache_size = -{cache_kib};"
        ))
        .context("Failed to configure the SQLite connection")?;
    Ok(())
}

/// Create the schema, or reject a file written by an incompatible version.
pub(super) fn initialize_schema(connection: &Connection) -> Result<()> {
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .context("Failed to read the database schema version")?;

    if version == 0 {
        connection
            .execute_batch(&ddl())
            .context("Failed to create the SQLite schema")?;
        connection
            .execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
            .context("Failed to record the database schema version")?;
        return Ok(());
    }

    if version != SCHEMA_VERSION {
        anyhow::bail!(
            "Incompatible database schema {version}; expected {SCHEMA_VERSION}. \
             The file was written by a different version of VuIO and is left untouched."
        );
    }

    // A file at the right version may still predate an additive index, and
    // creating them is idempotent.
    connection
        .execute_batch(&ddl())
        .context("Failed to verify the SQLite schema")?;
    Ok(())
}

/// Confirm a file is a readable database at the expected schema version.
///
/// Used before a restore overwrites anything, so it must not create or modify
/// the file it inspects.
pub(super) fn validate_database_file(path: &std::path::Path) -> Result<()> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("Failed to open {} for validation", path.display()))?;
    register_collations(&connection)?;

    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .with_context(|| format!("{} is not a readable SQLite database", path.display()))?;
    if integrity != "ok" {
        anyhow::bail!("{} failed its integrity check: {integrity}", path.display());
    }

    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .context("Failed to read the schema version")?;
    if version != SCHEMA_VERSION {
        anyhow::bail!(
            "{} has schema version {version}; expected {SCHEMA_VERSION}",
            path.display()
        );
    }

    // `user_version` is just an integer in the header, so a file could carry
    // the right number without the tables that number implies.
    let tables: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN \
             ('media_files', 'directories', 'playlists', 'playlist_entries', 'secrets')",
            [],
            |row| row.get(0),
        )
        .context("Failed to inspect the database schema")?;
    if tables != 5 {
        anyhow::bail!("{} is missing required tables", path.display());
    }
    Ok(())
}

// ── Decoding ───────────────────────────────────────────────────────────────

fn seconds_to_time(seconds: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds.max(0) as u64)
}

pub(super) fn time_to_seconds(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

/// Decode a row selected with [`MEDIA_COLUMNS`].
pub(super) fn media_file_from_row(row: &Row<'_>) -> rusqlite::Result<MediaFile> {
    Ok(MediaFile {
        id: Some(row.get(column::ID)?),
        path: PathBuf::from(row.get::<_, String>(column::PATH)?),
        filename: row.get(column::FILENAME)?,
        size: row.get::<_, i64>(column::SIZE)? as u64,
        modified: seconds_to_time(row.get(column::MODIFIED_SECS)?),
        mime_type: row.get(column::MIME_TYPE)?,
        duration: row
            .get::<_, Option<f64>>(column::DURATION_SECS)?
            .map(Duration::from_secs_f64),
        title: row.get(column::TITLE)?,
        artist: row.get(column::ARTIST)?,
        album: row.get(column::ALBUM)?,
        genre: row.get(column::GENRE)?,
        track_number: row
            .get::<_, Option<i64>>(column::TRACK_NUMBER)?
            .map(|value| value as u32),
        year: row
            .get::<_, Option<i64>>(column::YEAR)?
            .map(|value| value as u32),
        album_artist: row.get(column::ALBUM_ARTIST)?,
        subtitle_available: row.get::<_, i64>(column::SUBTITLE_AVAILABLE)? != 0,
        created_at: seconds_to_time(row.get(column::CREATED_AT_SECS)?),
        updated_at: seconds_to_time(row.get(column::UPDATED_AT_SECS)?),
    })
}

pub(super) fn file_location_from_row(row: &Row<'_>) -> rusqlite::Result<FileLocation> {
    Ok(FileLocation {
        id: row.get(0)?,
        path: PathBuf::from(row.get::<_, String>(1)?),
        filename: row.get(2)?,
        title: row.get(3)?,
        mime_type: row.get(4)?,
        size: row.get::<_, i64>(5)? as u64,
        subtitle_available: row.get::<_, i64>(6)? != 0,
    })
}

pub(super) const FILE_LOCATION_COLUMNS: &str =
    "id, path, filename, title, mime_type, size, subtitle_available";

pub(super) const FINGERPRINT_COLUMNS: &str = "id, path, size, modified_secs, created_at_secs";

pub(super) fn fingerprint_from_row(row: &Row<'_>) -> rusqlite::Result<FileFingerprint> {
    Ok(FileFingerprint {
        id: row.get(0)?,
        path: PathBuf::from(row.get::<_, String>(1)?),
        size: row.get::<_, i64>(2)? as u64,
        modified: seconds_to_time(row.get(3)?),
        created_at: seconds_to_time(row.get(4)?),
    })
}

pub(super) const PLAYLIST_COLUMNS: &str =
    "playlists.id, playlists.name, playlists.description, playlists.created_at_secs, \
     playlists.updated_at_secs";

pub(super) fn playlist_from_row(row: &Row<'_>) -> rusqlite::Result<Playlist> {
    Ok(Playlist {
        id: Some(row.get(0)?),
        name: row.get(1)?,
        description: row.get(2)?,
        created_at: seconds_to_time(row.get(3)?),
        updated_at: seconds_to_time(row.get(4)?),
    })
}

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

use crate::database::{
    AudioTags, BroadcastMode, FileFingerprint, FileLocation, MediaFile, Playlist, RadioStation,
    StreamInfo,
};

/// The schema version this build writes and expects.
///
/// Bumped whenever the tables change. An older file is brought forward by
/// [`migrations`]; only a *newer* file — one written by a build that knows
/// something this one does not — is refused, because there is no way to
/// downgrade a schema without guessing at what to discard.
pub(super) const SCHEMA_VERSION: i64 = 6;

/// Name of the collation that carries the application's natural ordering into
/// SQL. Registered on every connection; see [`register_collations`].
///
/// Not simply `natural`: that is a SQLite keyword (`NATURAL JOIN`), and an
/// index declared with it fails to parse.
pub(super) const NATURAL: &str = "natural_order";

/// The schema, with the collation name substituted so it cannot drift from
/// the name the connections actually register.
pub(super) fn ddl() -> String {
    format!("{}{FTS_DDL}", DDL_TEMPLATE.replace("{NATURAL}", NATURAL))
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
    disc_number        INTEGER,
    disc_total         INTEGER,
    track_total        INTEGER,
    composer           TEXT,
    comment            TEXT,
    bpm                INTEGER,
    compilation        INTEGER,
    sort_title         TEXT,
    sort_artist        TEXT,
    sort_album         TEXT,
    release_date       TEXT,
    musicbrainz_track_id  TEXT,
    musicbrainz_album_id  TEXT,
    musicbrainz_artist_id TEXT,
    codec              TEXT,
    sample_rate        INTEGER,
    channels           INTEGER,
    bits_per_sample    INTEGER,
    bit_rate           INTEGER,
    -- Which tag reader wrote this record. A file whose bytes have not changed
    -- is still re-read when this trails the current reader, which is how a
    -- better extractor reaches records that were already indexed.
    tags_version       INTEGER NOT NULL DEFAULT 0,
    subtitle_available INTEGER NOT NULL DEFAULT 0,
    created_at_secs    INTEGER NOT NULL,
    updated_at_secs    INTEGER NOT NULL,
    -- Browse ordering is "disc, track number, then natural filename", with
    -- untagged records last. Materializing the rank keeps that an index scan
    -- instead of a sort over the whole directory.
    --
    -- `track_sort` is STORED and predates `disc_sort`; SQLite cannot alter a
    -- generated column, and ALTER TABLE only accepts VIRTUAL ones, so the disc
    -- rank is a separate VIRTUAL column that the same indexes cover. Declared
    -- here exactly as the migration adds it, so old and new files agree.
    track_sort         INTEGER GENERATED ALWAYS AS (COALESCE(track_number, 4294967296)) STORED,
    disc_sort          INTEGER GENERATED ALWAYS AS (COALESCE(disc_number, 1)) VIRTUAL
) STRICT;

CREATE INDEX IF NOT EXISTS idx_media_dir_order
    ON media_files(parent_path, disc_sort, track_sort, filename COLLATE {NATURAL});
CREATE INDEX IF NOT EXISTS idx_media_dir_family
    ON media_files(parent_path, mime_family);
CREATE INDEX IF NOT EXISTS idx_media_album
    ON media_files(album, disc_sort, track_sort, filename COLLATE {NATURAL});
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

-- A station the server broadcasts: which folders feed it, in what order, and
-- whether it should be on the air.
--
-- `enabled` is desired state rather than a record of what happened. It is what
-- makes a restart resume the stations an operator left running, and why only an
-- explicit stop takes one off the air. `seed` fixes a shuffled station's order
-- so that resuming continues the same sequence instead of drawing a new one,
-- and `cursor_path` is where in that sequence to pick up.
CREATE TABLE IF NOT EXISTS radio_stations (
    id              INTEGER PRIMARY KEY,
    name            TEXT    NOT NULL,
    genre           TEXT    NOT NULL,
    folders         TEXT    NOT NULL,
    mode            TEXT    NOT NULL,
    enabled         INTEGER NOT NULL,
    seed            INTEGER NOT NULL,
    cursor_path     TEXT,
    created_at_secs INTEGER NOT NULL,
    updated_at_secs INTEGER NOT NULL
) STRICT;

-- Every tag the reader found that has no column of its own, kept verbatim so
-- that using a new one later is a query rather than another migration. The
-- composite key is what makes multi-valued tags — two artists, three genres —
-- representable at all.
CREATE TABLE IF NOT EXISTS media_tags (
    media_file_id INTEGER NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    key           TEXT    NOT NULL,
    value         TEXT    NOT NULL,
    PRIMARY KEY (media_file_id, key, value)
) STRICT;


-- What a public metadata service said about a file: title, synopsis, rating and
-- a pointer into the artwork cache.
--
-- Deliberately not `media_tags`. That table is cleared and rewritten in full
-- every time a record is re-scanned, because it holds what the file itself
-- claims and the file is the authority on that. This holds what somebody else
-- said, which no amount of re-reading the file can reproduce, so it has to
-- survive a scan. It still goes when the file does, via the cascade.
CREATE TABLE IF NOT EXISTS mediainfo (
    media_file_id     INTEGER PRIMARY KEY REFERENCES media_files(id) ON DELETE CASCADE,
    provider          TEXT    NOT NULL,
    remote_id         TEXT    NOT NULL,
    kind              TEXT    NOT NULL,
    title             TEXT,
    original_title    TEXT,
    overview          TEXT,
    release_date      TEXT,
    year              INTEGER,
    rating            REAL,
    genres            TEXT,
    season            INTEGER,
    episode           INTEGER,
    artwork_key       TEXT,
    payload           TEXT    NOT NULL,
    confidence        INTEGER NOT NULL,
    fetched_at        INTEGER NOT NULL,
    mediainfo_version INTEGER NOT NULL
) STRICT;

-- The dashboard lists the least certain matches first, which is a sort over the
-- whole table.
CREATE INDEX IF NOT EXISTS idx_mediainfo_confidence ON mediainfo(confidence);
"#
;

/// The full-text index, defined once and shared by the fresh-file DDL and the
/// migration that adds it to an existing file.
///
/// Search used to be `LIKE '%needle%'` over four columns: a leading wildcard no
/// index can serve, so every query read the whole table, and the results came
/// back in row order because there was no relevance to sort by. FTS5 answers the
/// same question from an index, ranked, across every field worth searching.
///
/// Both tables are **external-content**: the text stays in `media_files` and
/// `mediainfo`, and the index stores only what it needs to find a rowid. That
/// costs nothing in duplicated storage and makes recovery a one-liner
/// (`INSERT INTO … VALUES('rebuild')`).
///
/// Two tables rather than one because external content binds an index to exactly
/// one table, and a film's synopsis lives in `mediainfo` while its filename
/// lives in `media_files`. Searches union the two.
///
/// `remove_diacritics 2` folds accents across the whole Unicode range, so
/// "bjork" finds "Björk" — which is the difference between a search box that
/// works on a real music library and one that does not.
const FTS_DDL: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS media_fts USING fts5(
    filename, title, artist, album, album_artist, genre, composer, comment,
    content='media_files',
    content_rowid='id',
    tokenize='unicode61 remove_diacritics 2'
);

-- Triggers rather than call-site updates: `media_files` is written from the
-- scanner's bulk paths, the watcher's incremental ones, subtree removal and
-- native cleanup. A trigger covers all of them by construction, and cannot be
-- forgotten by the next write path someone adds.
CREATE TRIGGER IF NOT EXISTS media_fts_insert AFTER INSERT ON media_files BEGIN
    INSERT INTO media_fts(rowid, filename, title, artist, album, album_artist,
                          genre, composer, comment)
    VALUES (new.id, new.filename, new.title, new.artist, new.album,
            new.album_artist, new.genre, new.composer, new.comment);
END;

-- External-content tables are updated by inserting a matching 'delete' row that
-- cancels the old one, then inserting the new one. The old *values* have to be
-- given exactly, which is why the delete half spells out `old.`.
CREATE TRIGGER IF NOT EXISTS media_fts_delete AFTER DELETE ON media_files BEGIN
    INSERT INTO media_fts(media_fts, rowid, filename, title, artist, album,
                          album_artist, genre, composer, comment)
    VALUES ('delete', old.id, old.filename, old.title, old.artist, old.album,
            old.album_artist, old.genre, old.composer, old.comment);
END;

CREATE TRIGGER IF NOT EXISTS media_fts_update AFTER UPDATE ON media_files BEGIN
    INSERT INTO media_fts(media_fts, rowid, filename, title, artist, album,
                          album_artist, genre, composer, comment)
    VALUES ('delete', old.id, old.filename, old.title, old.artist, old.album,
            old.album_artist, old.genre, old.composer, old.comment);
    INSERT INTO media_fts(rowid, filename, title, artist, album, album_artist,
                          genre, composer, comment)
    VALUES (new.id, new.filename, new.title, new.artist, new.album,
            new.album_artist, new.genre, new.composer, new.comment);
END;

CREATE VIRTUAL TABLE IF NOT EXISTS mediainfo_fts USING fts5(
    title, original_title, overview,
    content='mediainfo',
    content_rowid='media_file_id',
    tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS mediainfo_fts_insert AFTER INSERT ON mediainfo BEGIN
    INSERT INTO mediainfo_fts(rowid, title, original_title, overview)
    VALUES (new.media_file_id, new.title, new.original_title, new.overview);
END;

CREATE TRIGGER IF NOT EXISTS mediainfo_fts_delete AFTER DELETE ON mediainfo BEGIN
    INSERT INTO mediainfo_fts(mediainfo_fts, rowid, title, original_title, overview)
    VALUES ('delete', old.media_file_id, old.title, old.original_title, old.overview);
END;

CREATE TRIGGER IF NOT EXISTS mediainfo_fts_update AFTER UPDATE ON mediainfo BEGIN
    INSERT INTO mediainfo_fts(mediainfo_fts, rowid, title, original_title, overview)
    VALUES ('delete', old.media_file_id, old.title, old.original_title, old.overview);
    INSERT INTO mediainfo_fts(rowid, title, original_title, overview)
    VALUES (new.media_file_id, new.title, new.original_title, new.overview);
END;
"#;

/// Recompute both full-text indexes from the tables they shadow.
///
/// Cheap enough to be the answer to any doubt about the index: it reads the
/// content tables once. Used by the migration that introduces them and by
/// `rebuild_derived_indexes`, which is the repair path for the same reason.
pub(super) const FTS_REBUILD: &str = r#"
INSERT INTO media_fts(media_fts) VALUES('rebuild');
INSERT INTO mediainfo_fts(mediainfo_fts) VALUES('rebuild');
"#;

/// Schema upgrades, applied in order to any file older than [`SCHEMA_VERSION`].
///
/// Each entry is `(version_it_produces, sql)`. The SQL runs inside the same
/// transaction that bumps `user_version`, so a failure part-way leaves the file
/// exactly as it was.
///
/// Migrations are additive by construction: they add columns, tables and
/// indexes, never drop or rewrite user data. Anything that cannot be expressed
/// that way needs a different plan, not a destructive one — the file holds
/// AirPlay pairings, imported playlists, and the record ids that DIDL hands out
/// as object ids, none of which survive a rebuild.
///
/// Owned rather than borrowed because v4 is assembled from the shared FTS
/// definitions rather than written out a second time.
fn migrations() -> Vec<(i64, String)> {
    vec![
        (2, MIGRATION_V2.to_owned()),
        (3, MIGRATION_V3.to_owned()),
        (4, migration_v4()),
        (5, MIGRATION_V5.to_owned()),
        (6, MIGRATION_V6.to_owned()),
    ]
}

/// v1 → v2: full tag extraction.
///
/// Adds the promoted tag and stream columns, the `media_tags` side table, and
/// disc-aware browse ordering. `tags_version` defaults to 0 on existing rows,
/// which is below the current reader's version, so the next scan re-writes them
/// with the new fields filled in.
const MIGRATION_V2: &str = r#"
ALTER TABLE media_files ADD COLUMN disc_number           INTEGER;
ALTER TABLE media_files ADD COLUMN disc_total            INTEGER;
ALTER TABLE media_files ADD COLUMN track_total           INTEGER;
ALTER TABLE media_files ADD COLUMN composer              TEXT;
ALTER TABLE media_files ADD COLUMN comment               TEXT;
ALTER TABLE media_files ADD COLUMN bpm                   INTEGER;
ALTER TABLE media_files ADD COLUMN compilation           INTEGER;
ALTER TABLE media_files ADD COLUMN sort_title            TEXT;
ALTER TABLE media_files ADD COLUMN sort_artist           TEXT;
ALTER TABLE media_files ADD COLUMN sort_album            TEXT;
ALTER TABLE media_files ADD COLUMN release_date          TEXT;
ALTER TABLE media_files ADD COLUMN musicbrainz_track_id  TEXT;
ALTER TABLE media_files ADD COLUMN musicbrainz_album_id  TEXT;
ALTER TABLE media_files ADD COLUMN musicbrainz_artist_id TEXT;
ALTER TABLE media_files ADD COLUMN codec                 TEXT;
ALTER TABLE media_files ADD COLUMN sample_rate           INTEGER;
ALTER TABLE media_files ADD COLUMN channels              INTEGER;
ALTER TABLE media_files ADD COLUMN bits_per_sample       INTEGER;
ALTER TABLE media_files ADD COLUMN bit_rate              INTEGER;
ALTER TABLE media_files ADD COLUMN tags_version          INTEGER NOT NULL DEFAULT 0;
ALTER TABLE media_files ADD COLUMN disc_sort INTEGER
    GENERATED ALWAYS AS (COALESCE(disc_number, 1)) VIRTUAL;

-- The ordering indexes now lead with the disc rank, and `CREATE INDEX IF NOT
-- EXISTS` will not redefine one that already exists.
DROP INDEX IF EXISTS idx_media_dir_order;
DROP INDEX IF EXISTS idx_media_album;
"#;

/// v2 → v3: online media info.
///
/// Only adds the `mediainfo` table. Existing rows are untouched and the table
/// starts empty, so an upgraded file behaves exactly as before until someone
/// presses Fetch. The idempotent DDL creates the same table on a fresh file.
const MIGRATION_V3: &str = r#"
CREATE TABLE IF NOT EXISTS mediainfo (
    media_file_id     INTEGER PRIMARY KEY REFERENCES media_files(id) ON DELETE CASCADE,
    provider          TEXT    NOT NULL,
    remote_id         TEXT    NOT NULL,
    kind              TEXT    NOT NULL,
    title             TEXT,
    original_title    TEXT,
    overview          TEXT,
    release_date      TEXT,
    year              INTEGER,
    rating            REAL,
    genres            TEXT,
    season            INTEGER,
    episode           INTEGER,
    artwork_key       TEXT,
    payload           TEXT    NOT NULL,
    confidence        INTEGER NOT NULL,
    fetched_at        INTEGER NOT NULL,
    mediainfo_version INTEGER NOT NULL
) STRICT;
"#;

/// v3 → v4: ranked full-text search.
///
/// Adds the two FTS5 indexes and their triggers, then populates them from the
/// rows already on disk. Purely additive: nothing in `media_files` or
/// `mediainfo` is touched, and a file that fails part-way through is rolled back
/// with the rest of the migration transaction.
fn migration_v4() -> String {
    format!("{FTS_DDL}{FTS_REBUILD}")
}

/// v4 → v5: drop two indexes nothing queries.
///
/// `tags_version` never appears in a `WHERE` or `ORDER BY` — it is compared in
/// Rust, against a value the scanner already holds — and `media_tags` is only
/// ever read by `media_file_id`, which its primary key already serves. Both were
/// pure write cost: two more b-tree insertions for every row indexed.
///
/// Dropping an index is not the destructive kind of migration the rule above
/// guards against. An index holds nothing that is not derivable from the table.
const MIGRATION_V5: &str = r#"
DROP INDEX IF EXISTS idx_media_tags_version;
DROP INDEX IF EXISTS idx_media_tags_key;
"#;

/// v5 → v6: radio stations the server broadcasts itself.
///
/// The one station that came before this lived as a JSON blob under the
/// `radio_broadcast_state` key in `secrets`, and described a feature that was
/// really driven from a browser tab. Nothing in it is worth carrying forward —
/// not the folder list, whose meaning changes now that a station is a queue the
/// server plays — so the key is dropped and stations start empty.
const MIGRATION_V6: &str = r#"
CREATE TABLE IF NOT EXISTS radio_stations (
    id              INTEGER PRIMARY KEY,
    name            TEXT    NOT NULL,
    genre           TEXT    NOT NULL,
    folders         TEXT    NOT NULL,
    mode            TEXT    NOT NULL,
    enabled         INTEGER NOT NULL,
    seed            INTEGER NOT NULL,
    cursor_path     TEXT,
    created_at_secs INTEGER NOT NULL,
    updated_at_secs INTEGER NOT NULL
) STRICT;

DELETE FROM secrets WHERE key = 'radio_broadcast_state';
"#;

/// Columns of `media_files`, qualified so the list can be used inside joins.
pub(super) const MEDIA_COLUMNS: &str = "\
media_files.id, media_files.path, media_files.filename, media_files.size, \
media_files.modified_secs, media_files.mime_type, media_files.duration_secs, \
media_files.title, media_files.artist, media_files.album, media_files.genre, \
media_files.track_number, media_files.year, media_files.album_artist, \
media_files.subtitle_available, media_files.created_at_secs, media_files.updated_at_secs, \
media_files.disc_number, media_files.disc_total, media_files.track_total, \
media_files.composer, media_files.comment, media_files.bpm, media_files.compilation, \
media_files.sort_title, media_files.sort_artist, media_files.sort_album, \
media_files.release_date, media_files.musicbrainz_track_id, \
media_files.musicbrainz_album_id, media_files.musicbrainz_artist_id, \
media_files.codec, media_files.sample_rate, media_files.channels, \
media_files.bits_per_sample, media_files.bit_rate, media_files.tags_version";

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
    pub const DISC_NUMBER: usize = 17;
    pub const DISC_TOTAL: usize = 18;
    pub const TRACK_TOTAL: usize = 19;
    pub const COMPOSER: usize = 20;
    pub const COMMENT: usize = 21;
    pub const BPM: usize = 22;
    pub const COMPILATION: usize = 23;
    pub const SORT_TITLE: usize = 24;
    pub const SORT_ARTIST: usize = 25;
    pub const SORT_ALBUM: usize = 26;
    pub const RELEASE_DATE: usize = 27;
    pub const MUSICBRAINZ_TRACK_ID: usize = 28;
    pub const MUSICBRAINZ_ALBUM_ID: usize = 29;
    pub const MUSICBRAINZ_ARTIST_ID: usize = 30;
    pub const CODEC: usize = 31;
    pub const SAMPLE_RATE: usize = 32;
    pub const CHANNELS: usize = 33;
    pub const BITS_PER_SAMPLE: usize = 34;
    pub const BIT_RATE: usize = 35;
    pub const TAGS_VERSION: usize = 36;
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

/// Create the schema, migrate an older file forward, or reject a newer one.
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

    if version > SCHEMA_VERSION {
        anyhow::bail!(
            "Incompatible database schema {version}; expected {SCHEMA_VERSION}. \
             The file was written by a newer version of VuIO and is left untouched."
        );
    }

    if version < SCHEMA_VERSION {
        migrate(connection, version)?;
    }

    // A file at the right version may still predate an additive index, and
    // creating them is idempotent.
    connection
        .execute_batch(&ddl())
        .context("Failed to verify the SQLite schema")?;
    Ok(())
}

/// Apply every migration between `from` and [`SCHEMA_VERSION`].
///
/// Each step runs with its `user_version` bump in one transaction, so an
/// interrupted upgrade leaves the file at a version that describes it.
fn migrate(connection: &Connection, from: i64) -> Result<()> {
    let all = migrations();
    let pending = all
        .iter()
        .filter(|(produces, _)| *produces > from)
        .collect::<Vec<_>>();

    if let Some((unreachable_from, _)) = pending.first().filter(|(first, _)| *first > from + 1) {
        anyhow::bail!(
            "No migration path from database schema {from} to {unreachable_from}; \
             the file is left untouched."
        );
    }

    for (produces, sql) in pending {
        tracing::info!(
            "Migrating the media database to schema version {}",
            produces
        );
        connection
            .execute_batch(&format!(
                "BEGIN;\n{}\nPRAGMA user_version = {produces};\nCOMMIT;",
                sql.replace("{NATURAL}", NATURAL)
            ))
            .with_context(|| format!("Failed to migrate the database to schema {produces}"))?;
    }

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

    // An older file is acceptable: opening it will migrate it forward. Only a
    // newer one has no path back to this build.
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .context("Failed to read the schema version")?;
    if !(1..=SCHEMA_VERSION).contains(&version) {
        anyhow::bail!(
            "{} has schema version {version}; expected 1..={SCHEMA_VERSION}",
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
        tags: AudioTags {
            disc_number: optional_u32(row, column::DISC_NUMBER)?,
            disc_total: optional_u32(row, column::DISC_TOTAL)?,
            track_total: optional_u32(row, column::TRACK_TOTAL)?,
            composer: row.get(column::COMPOSER)?,
            comment: row.get(column::COMMENT)?,
            bpm: optional_u32(row, column::BPM)?,
            compilation: row
                .get::<_, Option<i64>>(column::COMPILATION)?
                .map(|value| value != 0),
            sort_title: row.get(column::SORT_TITLE)?,
            sort_artist: row.get(column::SORT_ARTIST)?,
            sort_album: row.get(column::SORT_ALBUM)?,
            release_date: row.get(column::RELEASE_DATE)?,
            musicbrainz_track_id: row.get(column::MUSICBRAINZ_TRACK_ID)?,
            musicbrainz_album_id: row.get(column::MUSICBRAINZ_ALBUM_ID)?,
            musicbrainz_artist_id: row.get(column::MUSICBRAINZ_ARTIST_ID)?,
        },
        stream: StreamInfo {
            codec: row.get(column::CODEC)?,
            sample_rate: optional_u32(row, column::SAMPLE_RATE)?,
            channels: optional_u32(row, column::CHANNELS)?.map(|value| value as u16),
            bits_per_sample: optional_u32(row, column::BITS_PER_SAMPLE)?.map(|value| value as u16),
            bit_rate: optional_u32(row, column::BIT_RATE)?,
        },
        // Reading a record for a browse response never needs the long tail of
        // tags, so it is not joined in.
        extra_tags: Vec::new(),
        tags_version: row.get::<_, i64>(column::TAGS_VERSION)? as u32,
        subtitle_available: row.get::<_, i64>(column::SUBTITLE_AVAILABLE)? != 0,
        created_at: seconds_to_time(row.get(column::CREATED_AT_SECS)?),
        updated_at: seconds_to_time(row.get(column::UPDATED_AT_SECS)?),
    })
}

fn optional_u32(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u32>> {
    Ok(row.get::<_, Option<i64>>(index)?.map(|value| value as u32))
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

pub(super) const FINGERPRINT_COLUMNS: &str =
    "id, path, size, modified_secs, created_at_secs, tags_version";

pub(super) fn fingerprint_from_row(row: &Row<'_>) -> rusqlite::Result<FileFingerprint> {
    Ok(FileFingerprint {
        id: row.get(0)?,
        path: PathBuf::from(row.get::<_, String>(1)?),
        size: row.get::<_, i64>(2)? as u64,
        modified: seconds_to_time(row.get(3)?),
        created_at: seconds_to_time(row.get(4)?),
        tags_version: row.get::<_, i64>(5)? as u32,
    })
}

pub(super) const PLAYLIST_COLUMNS: &str =
    "playlists.id, playlists.name, playlists.description, playlists.created_at_secs, \
     playlists.updated_at_secs";

pub(super) const RADIO_STATION_COLUMNS: &str =
    "radio_stations.id, radio_stations.name, radio_stations.genre, radio_stations.folders, \
     radio_stations.mode, radio_stations.enabled, radio_stations.seed, \
     radio_stations.cursor_path, radio_stations.created_at_secs, radio_stations.updated_at_secs";

pub(super) fn radio_station_from_row(row: &Row<'_>) -> rusqlite::Result<RadioStation> {
    let folders: String = row.get(3)?;
    let mode: String = row.get(4)?;
    Ok(RadioStation {
        id: row.get(0)?,
        name: row.get(1)?,
        genre: row.get(2)?,
        // A folder list that will not parse leaves a station with no sources,
        // which surfaces as "nothing to broadcast" rather than a failed read of
        // every other station beside it.
        folders: serde_json::from_str(&folders).unwrap_or_default(),
        mode: BroadcastMode::parse(&mode).unwrap_or_default(),
        enabled: row.get::<_, i64>(5)? != 0,
        seed: row.get::<_, i64>(6)? as u64,
        cursor_path: row.get(7)?,
        created_at: seconds_to_time(row.get(8)?),
        updated_at: seconds_to_time(row.get(9)?),
    })
}

pub(super) fn playlist_from_row(row: &Row<'_>) -> rusqlite::Result<Playlist> {
    Ok(Playlist {
        id: Some(row.get(0)?),
        name: row.get(1)?,
        description: row.get(2)?,
        created_at: seconds_to_time(row.get(3)?),
        updated_at: seconds_to_time(row.get(4)?),
    })
}

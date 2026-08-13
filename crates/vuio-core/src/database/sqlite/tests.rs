use super::*;
use tempfile::tempdir;

/// Every backend answers the same questions the same way.
mod conformance {
    use crate::database::conformance::backend_conformance_tests;
    backend_conformance_tests!(crate::database::sqlite::SqliteDatabase);
}

async fn database(temp: &tempfile::TempDir, name: &str) -> Arc<SqliteDatabase> {
    let db = SqliteDatabase::new(temp.path().join(format!("{name}.db")))
        .await
        .unwrap();
    db.initialize().await.unwrap();
    Arc::new(db)
}

#[test]
fn subtree_range_stops_at_a_component_boundary() {
    let (start, end) = SqliteDatabase::subtree_range("/media/Film");
    assert_eq!(start, "/media/Film/");
    // '0' is the byte after '/', so the range covers every path under
    // "/media/Film/" and nothing that merely starts with "/media/Film".
    assert_eq!(end, "/media/Film0");

    let in_range = |path: &str| path >= start.as_str() && path < end.as_str();
    assert!(in_range("/media/Film/a.mkv"));
    assert!(in_range("/media/Film/nested/b.mkv"));
    assert!(!in_range("/media/Films/b.mkv"));
    assert!(!in_range("/media/Film"));

    // A trailing slash must not produce a doubled separator.
    assert_eq!(
        SqliteDatabase::subtree_range("/media/Film/"),
        ("/media/Film/".to_owned(), "/media/Film0".to_owned())
    );
}

#[test]
fn mime_family_is_the_segment_before_the_slash() {
    assert_eq!(SqliteDatabase::mime_family("video/x-matroska"), "video");
    assert_eq!(SqliteDatabase::mime_family("audio/mpeg"), "audio");
    assert_eq!(SqliteDatabase::mime_family("application"), "application");
}

#[tokio::test]
async fn the_natural_collation_orders_embedded_numbers_by_value() {
    let temp = tempdir().unwrap();
    let db = database(&temp, "collation").await;

    // The collation is the same function the rest of the crate sorts with, so
    // an ordered query cannot disagree with an in-memory sort.
    let ordered = db
        .execute_read(|connection| {
            let mut statement = connection.prepare(
                "WITH names(value) AS (VALUES ('s01e10'), ('s01e2'), ('S01E1')) \
                 SELECT value FROM names ORDER BY value COLLATE natural_order",
            )?;
            let values = statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(values)
        })
        .await
        .unwrap();
    assert_eq!(ordered, ["S01E1", "s01e2", "s01e10"]);
}

/// The v1 schema, frozen.
///
/// A migration is only tested by the schema it actually has to upgrade, so this
/// is a copy rather than something derived from the current DDL. It must never
/// be edited to track later changes.
const SCHEMA_V1: &str = r#"
CREATE TABLE media_files (
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
    track_sort         INTEGER GENERATED ALWAYS AS (COALESCE(track_number, 4294967296)) STORED
) STRICT;

CREATE INDEX idx_media_dir_order
    ON media_files(parent_path, track_sort, filename COLLATE natural_order);
CREATE INDEX idx_media_album
    ON media_files(album, track_sort, filename COLLATE natural_order);

CREATE TABLE directories (
    path        TEXT PRIMARY KEY,
    parent_path TEXT NOT NULL,
    name        TEXT NOT NULL
) STRICT;

CREATE TABLE directory_mime_counts (
    dir_path TEXT    NOT NULL REFERENCES directories(path) ON DELETE CASCADE,
    family   TEXT    NOT NULL,
    count    INTEGER NOT NULL,
    PRIMARY KEY (dir_path, family)
) STRICT;

CREATE TABLE playlists (
    id              INTEGER PRIMARY KEY,
    name            TEXT    NOT NULL,
    description     TEXT,
    source_path     TEXT,
    created_at_secs INTEGER NOT NULL,
    updated_at_secs INTEGER NOT NULL
) STRICT;

CREATE TABLE playlist_entries (
    playlist_id   INTEGER NOT NULL REFERENCES playlists(id)   ON DELETE CASCADE,
    media_file_id INTEGER NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    position      INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, position)
) STRICT;

CREATE TABLE root_availability (
    path                   TEXT PRIMARY KEY,
    last_seen_secs         INTEGER NOT NULL,
    unavailable_since_secs INTEGER,
    indexed_count          INTEGER NOT NULL,
    reason                 TEXT    NOT NULL
) STRICT;

CREATE TABLE secrets (
    key   TEXT PRIMARY KEY,
    value BLOB NOT NULL
) STRICT;

PRAGMA user_version = 1;
"#;

/// Opening a v1 file must carry it forward without losing anything.
///
/// The alternative — rebuilding from a rescan — would drop AirPlay pairings and
/// imported playlists, and would renumber every record. Those numbers are the
/// object ids DIDL hands to renderers, so a rebuild breaks every favourite and
/// resume point a TV has saved.
#[tokio::test]
async fn a_v1_database_migrates_forward_without_losing_anything() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("v1.db");

    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        crate::database::sqlite::schema::register_collations(&connection).unwrap();
        connection.execute_batch(SCHEMA_V1).unwrap();
        connection
            .execute_batch(
                "INSERT INTO media_files
                   (id, path, parent_path, filename, size, modified_secs, mime_type,
                    mime_family, title, artist, album, track_number,
                    created_at_secs, updated_at_secs)
                 VALUES (7, '/media/one.mp3', '/media', 'one.mp3', 10, 100, 'audio/mpeg',
                         'audio', 'One', 'Artist', 'Album', 1, 100, 100);
                 INSERT INTO playlists (id, name, created_at_secs, updated_at_secs)
                   VALUES (3, 'Roadtrip', 100, 100);
                 INSERT INTO playlist_entries (playlist_id, media_file_id, position)
                   VALUES (3, 7, 0);
                 INSERT INTO secrets (key, value) VALUES ('airplay.pairings', x'0102');
                 INSERT INTO root_availability
                   (path, last_seen_secs, indexed_count, reason)
                   VALUES ('/media', 100, 1, 'present');",
            )
            .unwrap();
    }

    let db = SqliteDatabase::new(path.clone()).await.unwrap();
    db.initialize().await.unwrap();

    let version: i64 = db
        .execute_read(|connection| {
            Ok(connection.query_row("PRAGMA user_version", [], |row| row.get(0))?)
        })
        .await
        .unwrap();
    assert_eq!(version, super::schema::SCHEMA_VERSION);

    // The record kept its identity, which is what DIDL object ids depend on.
    let file = db
        .get_file_by_path(std::path::Path::new("/media/one.mp3"))
        .await
        .unwrap()
        .expect("the migrated record is still there");
    assert_eq!(file.id, Some(7));
    assert_eq!(file.artist.as_deref(), Some("Artist"));
    // New columns exist and are empty until a scan re-reads the file.
    assert_eq!(file.tags.disc_number, None);
    assert_eq!(file.tags_version, 0);

    // Everything a rebuild would have thrown away.
    assert_eq!(db.get_playlists().await.unwrap().len(), 1);
    assert_eq!(db.get_playlist_tracks(3).await.unwrap().len(), 1);
    assert_eq!(
        db.get_secret("airplay.pairings").await.unwrap().as_deref(),
        Some(&[1u8, 2][..])
    );
    assert_eq!(db.list_root_availability().await.unwrap().len(), 1);

    // A record left at tags_version 0 is stale against any real reader, so the
    // next scan rewrites it even though the file has not changed.
    assert!(file.tags_version < 1);
}

#[tokio::test]
async fn an_incompatible_schema_version_is_refused() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("stale.db");
    {
        let db = SqliteDatabase::new(path.clone()).await.unwrap();
        db.initialize().await.unwrap();
        db.execute_write(|connection| {
            connection.execute_batch("PRAGMA user_version = 99")?;
            Ok(())
        })
        .await
        .unwrap();
    }

    let reopened = SqliteDatabase::new(path.clone()).await.unwrap();
    let error = reopened.initialize().await.unwrap_err();
    assert!(
        error.to_string().contains("Incompatible database schema"),
        "unexpected error: {error:#}"
    );
    assert!(path.exists(), "the file must be left for quarantine");
}

#[tokio::test]
async fn directory_counts_return_to_zero_when_the_last_file_goes() {
    let temp = tempdir().unwrap();
    let db = database(&temp, "counters").await;

    db.bulk_store_media_files(&[
        MediaFile::new(
            PathBuf::from("/media/a/b/one.mkv"),
            1,
            "video/x-matroska".to_owned(),
        ),
        MediaFile::new(
            PathBuf::from("/media/a/b/two.mkv"),
            1,
            "video/x-matroska".to_owned(),
        ),
    ])
    .await
    .unwrap();

    let counts = |db: Arc<SqliteDatabase>| async move {
        db.execute_read(|connection| {
            Ok(connection.query_row(
                "SELECT COALESCE(SUM(count), 0) FROM directory_mime_counts WHERE family = '*'",
                [],
                |row| row.get::<_, i64>(0),
            )?)
        })
        .await
        .unwrap()
    };

    // Two files, each counted by every ancestor: /media/a/b, /media/a, /media, /
    assert_eq!(counts(db.clone()).await, 8);

    db.remove_media_file(Path::new("/media/a/b/one.mkv"))
        .await
        .unwrap();
    assert_eq!(counts(db.clone()).await, 4);

    db.remove_media_file(Path::new("/media/a/b/two.mkv"))
        .await
        .unwrap();
    assert_eq!(counts(db.clone()).await, 0);

    let remaining = db
        .execute_read(|connection| {
            Ok(connection.query_row("SELECT COUNT(*) FROM directories", [], |row| {
                row.get::<_, i64>(0)
            })?)
        })
        .await
        .unwrap();
    assert_eq!(remaining, 0, "empty directories must not linger");
}

#[tokio::test]
async fn rebuilding_the_directory_tree_reproduces_incremental_maintenance() {
    let temp = tempdir().unwrap();
    let db = database(&temp, "rebuild").await;

    db.bulk_store_media_files(&[
        MediaFile::new(PathBuf::from("/media/x/a.mkv"), 1, "video/mp4".to_owned()),
        MediaFile::new(PathBuf::from("/media/x/y/b.mp3"), 1, "audio/mpeg".to_owned()),
        MediaFile::new(PathBuf::from("/media/z/c.jpg"), 1, "image/jpeg".to_owned()),
    ])
    .await
    .unwrap();

    let snapshot = |db: Arc<SqliteDatabase>| async move {
        db.execute_read(|connection| {
            let mut statement = connection.prepare(
                "SELECT dir_path, family, count FROM directory_mime_counts \
                 ORDER BY dir_path, family",
            )?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .await
        .unwrap()
    };

    let incremental = snapshot(db.clone()).await;
    assert!(!incremental.is_empty());

    db.rebuild_derived_indexes().await.unwrap();
    let rebuilt = snapshot(db.clone()).await;

    // Drift between these two is invisible in normal use: it shows up only as
    // a folder missing from a filtered browse.
    assert_eq!(incremental, rebuilt);
}

#[tokio::test]
async fn a_write_ahead_log_left_behind_does_not_resurrect_records() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("wal.db");
    {
        let db = SqliteDatabase::new(path.clone()).await.unwrap();
        db.initialize().await.unwrap();
        db.store_media_file(&MediaFile::new(
            PathBuf::from("/media/only.mkv"),
            1,
            "video/mp4".to_owned(),
        ))
        .await
        .unwrap();
    }

    // A crash can leave committed data in the log rather than the main file.
    assert!(
        SqliteDatabase::file_extension() == "db"
            && SqliteDatabase::sidecar_extensions() == ["db-wal", "db-shm"],
        "the sidecar contract is what makes quarantine and restore safe"
    );

    let reopened = SqliteDatabase::new(path).await.unwrap();
    reopened.initialize().await.unwrap();
    assert!(reopened
        .get_file_by_path(Path::new("/media/only.mkv"))
        .await
        .unwrap()
        .is_some());
}

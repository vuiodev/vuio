use super::*;
use tempfile::tempdir;

/// Every backend answers the same questions the same way.
mod conformance {
    use crate::database::conformance::backend_conformance_tests;
    backend_conformance_tests!(crate::database::sqlite::SqliteDatabase);
}

async fn database_at(path: &std::path::Path) -> Arc<SqliteDatabase> {
    let db = SqliteDatabase::new(path.to_path_buf()).await.unwrap();
    db.initialize().await.unwrap();
    Arc::new(db)
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
    assert_eq!(file.stream.codec, None);
    assert_eq!(file.stream.video_codec, None, "v7's column");
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

    // v3 added the media info table. It arrives empty and usable, and the file it
    // hangs off keeps the id it already had.
    use crate::database::MediaInfoRepository;
    assert!(db.get_mediainfo(7).await.unwrap().is_none());
    db.bulk_store_mediainfo(&[crate::database::MediaInfoRecord {
        media_file_id: 7,
        provider: "musicbrainz".to_string(),
        remote_id: "release-1".to_string(),
        kind: "album".to_string(),
        title: Some("Album".to_string()),
        original_title: None,
        overview: Some("A remarkably hoopy record".to_string()),
        release_date: None,
        year: Some(1971),
        rating: None,
        genres: Vec::new(),
        season: None,
        episode: None,
        artwork_key: None,
        payload: "null".to_string(),
        confidence: 90,
        fetched_at: std::time::SystemTime::now(),
        mediainfo_version: 1,
    }])
    .await
    .unwrap();
    assert_eq!(
        db.get_mediainfo(7).await.unwrap().unwrap().year,
        Some(1971)
    );

    // v4 added full-text search. The rows already on disk have to become
    // searchable during the migration — a record only reaches the index through
    // a trigger, and this one was written years before the trigger existed.
    let db = std::sync::Arc::new(db);
    assert_eq!(
        search_ids(&db, "artist").await,
        vec![Some(7)],
        "rows that predate the index must be backfilled by the migration"
    );

    // And a synopsis fetched after the migration is searchable too — that is
    // the second index, over a different table, with its own triggers. The word
    // appears nowhere in the file's own tags, so only `mediainfo_fts` can
    // answer it.
    assert_eq!(search_ids(&db, "hoopy").await, vec![Some(7)]);
}

async fn search_ids(db: &std::sync::Arc<SqliteDatabase>, text: &str) -> Vec<Option<i64>> {
    use crate::database::{DatabaseReadSession, MediaFileView};
    let text = text.to_string();
    db.clone()
        .read(move |session| {
            let mut ids = Vec::new();
            session.visit_files(
                &crate::database::MediaFileQuery::Search {
                    text,
                    mime_family: None,
                },
                0,
                10,
                |file| {
                    ids.push(file.id());
                    Ok(())
                },
            )?;
            Ok(ids)
        })
        .await
        .unwrap()
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

/// A scan loads its root's records a page at a time. The pages have to tile the
/// subtree exactly — every record once, in order, nothing from a sibling whose
/// name merely starts the same way — and a cursor from outside the range must
/// not widen it.
#[tokio::test]
async fn fingerprints_under_a_root_come_back_in_pages_that_tile_it() {
    let temp = tempdir().unwrap();
    let db = database(&temp, "pages").await;

    let mut inside: Vec<String> = (0..7).map(|i| format!("/media/Film/{i:02}.mkv")).collect();
    inside.push("/media/Film/nested/deep.mkv".to_owned());
    // Either side of the range: '.' sorts just before '/', and "/media/Film0"
    // is where the range ends.
    let outside = [
        "/media/A.mkv",
        "/media/Film.mkv",
        "/media/Film0.mkv",
        "/media/Films/other.mkv",
    ];
    let files: Vec<MediaFile> = inside
        .iter()
        .map(String::as_str)
        .chain(outside)
        .map(|path| MediaFile::new(PathBuf::from(path), 1, "video/x-matroska".to_owned()))
        .collect();
    db.bulk_store_media_files(&files).await.unwrap();

    let mut seen = Vec::new();
    let mut pages = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let page = db
            .load_file_fingerprints_under("/media/Film", after.as_deref(), 3)
            .await
            .unwrap();
        pages.push(page.len());
        after = page.last().map(|f| f.path.to_string_lossy().into_owned());
        let last = page.len() < 3;
        seen.extend(page.into_iter().map(|f| f.path.to_string_lossy().into_owned()));
        if last {
            break;
        }
    }
    inside.sort();
    assert_eq!(seen, inside);
    assert_eq!(pages, [3, 3, 2]);

    let widened = db
        .load_file_fingerprints_under("/media/Film", Some("/a"), 100)
        .await
        .unwrap();
    assert_eq!(widened.len(), inside.len(), "a cursor below the range is ignored");
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

    // More distinct directory counters than one repair batch, with shared
    // ancestors whose counts must add up across every flush.
    for batch in 0..5 {
        let files: Vec<_> = (0..1000)
            .map(|i| {
                MediaFile::new(
                    PathBuf::from(format!("/media/large/{}/song.mp3", batch * 1000 + i)),
                    1,
                    "audio/mpeg".to_owned(),
                )
            })
            .collect();
        db.bulk_store_media_files(&files).await.unwrap();
    }

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

/// macOS hands back `Fu\u{308}\u{df}en` where Windows wrote `F\u{fc}\u{df}en`. The two look
/// identical, share no bytes, and used to be two different libraries as far as
/// search was concerned. Both spellings must reach the same row from either
/// spelling of the query, through the FTS index and the `LIKE` filter alike.
#[tokio::test]
async fn either_normalization_form_finds_the_other() {
    use crate::database::{DatabaseReadSession, MediaFileView, MediaFileQuery};

    const NFC: &str = "Füßen";
    const NFD: &str = "Fu\u{308}\u{df}en";
    assert_ne!(NFC, NFD, "the premise: the two spellings differ byte for byte");

    let temp = tempdir().unwrap();
    let db = database(&temp, "normalization").await;

    // Stored decomposed, as a scan of an HFS+ volume would produce.
    let mut decomposed = MediaFile::new(
        PathBuf::from(format!("/media/{NFD}.flac")),
        1,
        "audio/flac".to_owned(),
    );
    decomposed.title = Some(NFD.to_owned());
    decomposed.artist = Some(NFD.to_owned());
    let id = db.store_media_file(&decomposed).await.unwrap();

    // Found by the composed spelling, which is what a browser or a phone sends.
    assert_eq!(search_ids(&db, NFC).await, vec![Some(id)], "FTS missed NFC");
    assert_eq!(search_ids(&db, NFD).await, vec![Some(id)], "FTS missed NFD");

    let filtered = |text: &str| {
        let db = db.clone();
        let text = text.to_owned();
        async move {
            db.read(move |session| {
                let mut ids = Vec::new();
                session.visit_files(
                    &MediaFileQuery::Filtered {
                        after_id: None,
                        mime_family: None,
                        text: Some(text),
                    },
                    0,
                    10,
                    |file| {
                        ids.push(file.id());
                        Ok(())
                    },
                )?;
                Ok(ids)
            })
            .await
            .unwrap()
        }
    };
    assert_eq!(filtered(NFC).await, vec![Some(id)], "LIKE missed NFC");
    assert_eq!(filtered(NFD).await, vec![Some(id)], "LIKE missed NFD");

    // What we hand a renderer is the composed spelling, whatever was scanned:
    // a TV that cannot place a combining mark shows "Fu" followed by a stray
    // diaeresis otherwise.
    let stored = db.get_file_by_path(&decomposed.path).await.unwrap().unwrap();
    assert_eq!(stored.title.as_deref(), Some(NFC));
    assert_eq!(stored.artist.as_deref(), Some(NFC));
    assert_eq!(stored.filename, format!("{NFC}.flac"));

    // The path is the exception: it is the key the file is opened with, so it
    // keeps the bytes the filesystem reported.
    assert_eq!(stored.path, decomposed.path);
}

/// A library scanned before this release holds whatever form the filesystem gave
/// it. Those rows have to be folded by the migration, not left waiting for a
/// rescan — a user who upgrades and searches for their own music would otherwise
/// find nothing, which is exactly the state the fold is meant to end.
#[tokio::test]
async fn a_legacy_database_has_its_text_folded_by_the_migration() {
    const NFC: &str = "Füßen";
    const NFD: &str = "Fu\u{308}\u{df}en";

    let temp = tempdir().unwrap();
    let path = temp.path().join("legacy.db");

    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        crate::database::sqlite::schema::register_collations(&connection).unwrap();
        connection.execute_batch(SCHEMA_V1).unwrap();
        connection
            .execute(
                "INSERT INTO media_files
                   (id, path, parent_path, filename, size, modified_secs, mime_type,
                    mime_family, title, artist, album, created_at_secs, updated_at_secs)
                 VALUES (11, ?1, '/media', ?2, 10, 100, 'audio/flac', 'audio', ?3, ?3, ?3,
                         100, 100)",
                rusqlite::params![
                    format!("/media/{NFD}.flac"),
                    format!("{NFD}.flac"),
                    NFD,
                ],
            )
            .unwrap();
    }

    let db = SqliteDatabase::new(path.clone()).await.unwrap();
    db.initialize().await.unwrap();

    // The path is untouched — it still has to open the file that is on disk.
    let file = db
        .get_file_by_path(std::path::Path::new(&format!("/media/{NFD}.flac")))
        .await
        .unwrap()
        .expect("the migrated record is still there");
    assert_eq!(file.id, Some(11), "the record kept its DIDL object id");

    // Its text was folded in place.
    assert_eq!(file.title.as_deref(), Some(NFC));
    assert_eq!(file.artist.as_deref(), Some(NFC));
    assert_eq!(file.album.as_deref(), Some(NFC));
    assert_eq!(file.filename, format!("{NFC}.flac"));

    // And the folded text reached the index the migration rebuilt.
    let db = std::sync::Arc::new(db);
    assert_eq!(search_ids(&db, NFC).await, vec![Some(11)]);
    assert_eq!(search_ids(&db, NFD).await, vec![Some(11)]);
}

/// The full-text index's own storage, row by row.
///
/// FTS5 writes a new segment at every commit that changed the index, so two
/// equal snapshots mean nothing between them rewrote an entry.
async fn fts_segments(db: &SqliteDatabase) -> Vec<(i64, Vec<u8>)> {
    db.execute_read(|connection| {
        let mut statement = connection.prepare("SELECT id, block FROM media_fts_data ORDER BY id")?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    })
    .await
    .unwrap()
}

/// FTS5's own check, against `media_files` as well as internally: an external
/// content index that has drifted from its table fails here.
async fn assert_index_matches_media_files(db: &SqliteDatabase) {
    db.execute_write(|connection| {
        connection
            .execute_batch("INSERT INTO media_fts(media_fts, rank) VALUES('integrity-check', 1);")?;
        Ok(())
    })
    .await
    .expect("media_fts no longer matches media_files");
}

/// Flagging a subtitle, or a rescan writing back the tags a record already has,
/// changes nothing the index holds. It used to delete and re-add the record's
/// entry anyway — most of the cost of both writes.
#[tokio::test]
async fn an_update_that_leaves_indexed_text_alone_does_not_touch_the_index() {
    let temp = tempdir().unwrap();
    let db = database(&temp, "fts-untouched").await;
    let mut file = MediaFile::new(
        std::path::PathBuf::from("/media/01.flac"),
        10,
        "audio/flac".to_string(),
    );
    file.title = Some("Moonlight Sonata".to_string());
    file.artist = Some("Beethoven".to_string());
    let id = db.store_media_file(&file).await.unwrap();
    let before = fts_segments(&db).await;

    // A column the trigger does not name at all.
    assert_eq!(db.set_subtitle_available(&[id], true).await.unwrap(), 1);
    assert_eq!(fts_segments(&db).await, before, "set_subtitle_available");

    // The whole-record rewrite, which names every column: the rescan path.
    let mut rescanned = db.get_file_by_id(id).await.unwrap().unwrap();
    rescanned.size = 20;
    rescanned.duration = Some(std::time::Duration::from_secs(180));
    db.update_media_file(&rescanned).await.unwrap();
    assert_eq!(fts_segments(&db).await, before, "rewrite with unchanged text");
    assert_index_matches_media_files(&db).await;

    // A change the index does hold still reaches it.
    let mut retitled = rescanned.clone();
    retitled.title = Some("Breakfast".to_string());
    db.update_media_file(&retitled).await.unwrap();
    assert_ne!(fts_segments(&db).await, before, "a retitle has to reach the index");
    assert_index_matches_media_files(&db).await;
    assert_eq!(search_ids(&db, "breakfast").await, vec![Some(id)]);
    assert_eq!(search_ids(&db, "beethoven").await, vec![Some(id)]);
    assert!(search_ids(&db, "sonata").await.is_empty(), "the old title is gone");
}

/// The trigger lists what it watches, twice, so a column added to the index and
/// forgotten in either list would leave the index stale for exactly that column.
/// Each indexed column is changed on its own, and FTS5 compares the index with
/// the table after every one.
#[tokio::test]
async fn a_change_to_any_indexed_column_alone_reaches_the_index() {
    const INDEXED: [&str; 8] = [
        "filename",
        "title",
        "artist",
        "album",
        "album_artist",
        "genre",
        "composer",
        "comment",
    ];
    let temp = tempdir().unwrap();
    let db = database(&temp, "fts-columns").await;
    let mut file = MediaFile::new(
        std::path::PathBuf::from("/media/one.flac"),
        10,
        "audio/flac".to_string(),
    );
    file.title = Some("before".to_string());
    let id = db.store_media_file(&file).await.unwrap();

    for column in INDEXED {
        db.execute_write(move |connection| {
            connection.execute(
                &format!("UPDATE media_files SET {column} = ?1 WHERE id = ?2"),
                rusqlite::params![format!("changed{column}"), id],
            )?;
            Ok(())
        })
        .await
        .unwrap();
        assert_index_matches_media_files(&db).await;
        assert_eq!(
            search_ids(&db, &format!("changed{column}")).await,
            vec![Some(id)],
            "{column}"
        );
    }
}

/// `media_fts_update` as v8 declared it, frozen: it fired on every UPDATE.
/// Must never be edited to track later changes.
const MEDIA_FTS_UPDATE_V8: &str = r#"
CREATE TRIGGER media_fts_update AFTER UPDATE ON media_files BEGIN
    INSERT INTO media_fts(media_fts, rowid, filename, title, artist, album,
                          album_artist, genre, composer, comment)
    VALUES ('delete', old.id, old.filename, old.title, old.artist, old.album,
            old.album_artist, old.genre, old.composer, old.comment);
    INSERT INTO media_fts(rowid, filename, title, artist, album, album_artist,
                          genre, composer, comment)
    VALUES (new.id, new.filename, new.title, new.artist, new.album,
            new.album_artist, new.genre, new.composer, new.comment);
END;
"#;

/// `CREATE TRIGGER IF NOT EXISTS` would leave a v8 file's trigger as it was, so
/// the migration has to replace it — and must not disturb the index it guards.
#[tokio::test]
async fn a_v8_database_gets_the_narrowed_update_trigger() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("v8.db");
    let id = {
        let db = database_at(&path).await;
        let mut file = MediaFile::new(
            std::path::PathBuf::from("/media/Moon River.flac"),
            10,
            "audio/flac".to_string(),
        );
        file.title = Some("Moon River".to_string());
        let id = db.store_media_file(&file).await.unwrap();
        // Back to how a v8 build left the file.
        db.execute_write(|connection| {
            connection.execute_batch(&format!(
                "DROP TRIGGER media_fts_update;\n{MEDIA_FTS_UPDATE_V8}\nPRAGMA user_version = 8;"
            ))?;
            Ok(())
        })
        .await
        .unwrap();
        id
    };

    let db = database_at(&path).await;
    let (version, trigger): (i64, String) = db
        .execute_read(|connection| {
            Ok((
                connection.query_row("PRAGMA user_version", [], |row| row.get(0))?,
                connection.query_row(
                    "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'media_fts_update'",
                    [],
                    |row| row.get(0),
                )?,
            ))
        })
        .await
        .unwrap();
    assert_eq!(version, super::schema::SCHEMA_VERSION);
    assert!(
        trigger.contains("AFTER UPDATE OF") && trigger.contains("WHEN"),
        "the v8 trigger survived the migration: {trigger}"
    );

    // The record is still found, and the replaced trigger does its job.
    assert_index_matches_media_files(&db).await;
    let before = fts_segments(&db).await;
    assert_eq!(db.set_subtitle_available(&[id], true).await.unwrap(), 1);
    assert_eq!(fts_segments(&db).await, before);
    assert_eq!(search_ids(&db, "moon").await, vec![Some(id)]);
}

/// The database holds the `secrets` table — provider API keys, and the pairing secret
/// an AirPlay receiver hands over once. It was created at the process umask, so on a
/// normal system any local user could read them out of it, while the admin token beside
/// it is written 0600 and the server refuses to start if it is group- or other-readable.
#[cfg(unix)]
#[tokio::test]
async fn a_new_database_is_readable_only_by_its_owner() {
    use crate::database::SecretStore;
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir().unwrap();
    let path = temp.path().join("vuio.db");
    let database = database_at(&path).await;
    // A write, so the write-ahead log and the shared-memory index really exist.
    database.set_secret("tmdb", b"an-api-key").await.unwrap();

    let mode = |path: &std::path::Path| {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    };
    assert_eq!(mode(&path) & 0o077, 0, "the database itself");
    for path in SqliteDatabase::sidecar_paths(&path) {
        if path.exists() {
            assert_eq!(mode(&path) & 0o077, 0, "{}", path.display());
        }
    }
}

/// SQLite appends `-wal` and `-shm` to the complete database filename. Replacing
/// an assumed `.db` extension only works for the default path and left custom
/// paths such as `library.sqlite-wal` at their old permissions.
#[cfg(unix)]
#[test]
fn custom_database_sidecars_are_narrowed_by_their_real_names() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir().unwrap();
    let path = temp.path().join("library.sqlite");
    std::fs::write(&path, b"database").unwrap();
    let sidecars = SqliteDatabase::sidecar_paths(&path);
    for sidecar in &sidecars {
        std::fs::write(sidecar, b"sidecar").unwrap();
        std::fs::set_permissions(sidecar, std::fs::Permissions::from_mode(0o644)).unwrap();
    }

    crate::database::restrict_database_to_owner::<SqliteDatabase>(&path).unwrap();
    for sidecar in sidecars {
        let mode = std::fs::metadata(&sidecar).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode & 0o077, 0, "{}", sidecar.display());
    }
}

/// And an installation that has been running for a year, whose database was created
/// before any of this, is narrowed when it is next opened rather than left as it was.
#[cfg(unix)]
#[tokio::test]
async fn an_existing_database_is_narrowed_when_it_is_opened() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir().unwrap();
    let path = temp.path().join("vuio.db");
    drop(database_at(&path).await);
    // As the umask would have left it.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    let _database = database_at(&path).await;
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode & 0o077, 0, "reopening narrows what it finds");
    assert_eq!(mode & 0o700, 0o600, "and keeps the owner's own bits");
}

/// A backup is a copy of the `secrets` table, so it has to be as private as the
/// database it came from. `VACUUM INTO` creates its target at the process umask, and
/// only the rotating lifecycle backup was narrowed afterwards — the pre-repair backup
/// taken at every start with backups on went straight through `create_backup` and was
/// left readable by every local user.
#[cfg(unix)]
#[tokio::test]
async fn a_backup_is_readable_only_by_its_owner() {
    use crate::database::{HealthRepository, SecretStore};
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir().unwrap();
    let database = database_at(&temp.path().join("vuio.db")).await;
    database.set_secret("tmdb", b"an-api-key").await.unwrap();

    let backup = temp.path().join("backups").join("pre-repair.db");
    database.create_backup(&backup).await.unwrap();

    let mode = std::fs::metadata(&backup).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode & 0o077, 0, "the backup is mode {mode:o}");
    // And it is a real backup, not an empty file left where one should be.
    let restored = database_at(&backup).await;
    assert_eq!(
        restored.get_secret("tmdb").await.unwrap().as_deref(),
        Some(&b"an-api-key"[..])
    );
}

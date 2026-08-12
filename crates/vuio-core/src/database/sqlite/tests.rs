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

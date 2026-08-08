use super::*;
use tempfile::tempdir;

#[tokio::test]
async fn test_redb_database_basic_operations() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.redb");

    let db = RedbDatabase::new(db_path).await.unwrap();
    db.initialize().await.unwrap();

    // Store a file
    let file = MediaFile::new(
        PathBuf::from("/music/test.mp3"),
        1024,
        "audio/mpeg".to_string(),
    );
    let id = db.store_media_file(&file).await.unwrap();
    assert!(id > 0);

    // Retrieve by path
    let retrieved = db
        .get_file_by_path(&PathBuf::from("/music/test.mp3"))
        .await
        .unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().filename, "test.mp3");

    // Retrieve by ID
    let by_id = db.get_file_by_id(id).await.unwrap();
    assert!(by_id.is_some());

    // Remove
    let removed = db
        .remove_media_file(&PathBuf::from("/music/test.mp3"))
        .await
        .unwrap();
    assert!(removed);

    let removed_check = db
        .get_file_by_path(&PathBuf::from("/music/test.mp3"))
        .await
        .unwrap();
    assert!(removed_check.is_none());
}

#[tokio::test]
async fn opening_corrupt_database_does_not_delete_original() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("corrupt.redb");
    let original = b"not a redb database";
    std::fs::write(&path, original).unwrap();

    assert!(RedbDatabase::new(path.clone()).await.is_err());
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[tokio::test]
async fn opening_incompatible_schema_preserves_database_contents() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("old-schema.redb");
    {
        let raw = Database::create(&path).unwrap();
        let write = raw.begin_write().unwrap();
        {
            let mut files = write.open_table(FILES_TABLE).unwrap();
            files.insert(42, &[1_u8, 2, 3][..]).unwrap();
            let mut metadata = write.open_table(METADATA_TABLE).unwrap();
            metadata
                .insert("schema_version", SCHEMA_VERSION - 1)
                .unwrap();
        }
        write.commit().unwrap();
    }

    assert!(RedbDatabase::new(path.clone()).await.is_err());
    assert!(path.exists());

    let raw = Database::open(&path).unwrap();
    let read = raw.begin_read().unwrap();
    let files = read.open_table(FILES_TABLE).unwrap();
    assert_eq!(files.get(42).unwrap().unwrap().value(), &[1_u8, 2, 3]);
    let metadata = read.open_table(METADATA_TABLE).unwrap();
    assert_eq!(
        metadata.get("schema_version").unwrap().unwrap().value(),
        SCHEMA_VERSION - 1
    );
}

#[tokio::test]
async fn test_redb_database_bulk_operations() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test_bulk.redb");

    let db = RedbDatabase::new(db_path).await.unwrap();
    db.initialize().await.unwrap();

    let files: Vec<MediaFile> = (0..100)
        .map(|i| {
            MediaFile::new(
                PathBuf::from(format!("/music/song{}.mp3", i)),
                1024,
                "audio/mpeg".to_string(),
            )
        })
        .collect();

    let ids = db.bulk_store_media_files(&files).await.unwrap();
    assert_eq!(ids.len(), 100);

    let stats = db.get_stats().await.unwrap();
    assert_eq!(stats.total_files, 100);
}

#[tokio::test]
async fn duplicate_upsert_then_delete_does_not_leave_ghost_directory() {
    let temp = tempdir().unwrap();
    let db = RedbDatabase::new(temp.path().join("ghost.redb"))
        .await
        .unwrap();
    db.initialize().await.unwrap();
    let mut file = MediaFile::new(
        PathBuf::from("/media/deleted/movie.mkv"),
        1,
        "video/x-matroska".to_string(),
    );
    file.artist = Some("Ghost Artist".to_string());
    file.album = Some("Ghost Album".to_string());
    file.genre = Some("Ghost Genre".to_string());
    file.year = Some(2026);
    file.album_artist = Some("Ghost Album Artist".to_string());
    let first = db.store_media_file(&file).await.unwrap();
    let second = db
        .bulk_store_media_files(std::slice::from_ref(&file))
        .await
        .unwrap();
    assert_eq!(vec![first], second);
    assert_eq!(
        db.remove_media_under_path(Path::new("/media/deleted"))
            .await
            .unwrap()
            .removed_files,
        1
    );
    let (dirs, files) = db
        .get_directory_listing(Path::new("/media"), "video/")
        .await
        .unwrap();
    assert!(dirs.is_empty(), "ghost directories: {dirs:?}");
    assert!(files.is_empty());
    assert!(db.get_artists().await.unwrap().is_empty());
    assert!(db.get_albums(None).await.unwrap().is_empty());
    assert!(db.get_genres().await.unwrap().is_empty());
    assert!(db.get_years().await.unwrap().is_empty());
    assert!(db.get_album_artists().await.unwrap().is_empty());
}

#[tokio::test]
async fn rebuild_removes_orphan_indexes_and_repairs_counters() {
    let temp = tempdir().unwrap();
    let db = RedbDatabase::new(temp.path().join("repair.redb"))
        .await
        .unwrap();
    db.initialize().await.unwrap();
    db.rebuild_derived_indexes().await.unwrap();

    let mut file = MediaFile::new(
        PathBuf::from("/music/orphan/song.mp3"),
        42,
        "audio/mpeg".to_string(),
    );
    file.artist = Some("Orphan Artist".to_string());
    let id = db.store_media_file(&file).await.unwrap();
    let playlist = db.create_playlist("Kept playlist", None).await.unwrap();
    db.add_to_playlist(playlist, id, Some(0)).await.unwrap();

    {
        let database = db.db.read().unwrap();
        let transaction = database.begin_write().unwrap();
        transaction
            .open_table(FILES_TABLE)
            .unwrap()
            .remove(id)
            .unwrap();
        transaction.commit().unwrap();
    }

    let health = db.rebuild_derived_indexes().await.unwrap();
    assert!(health.is_healthy);
    assert!(db.get_artists().await.unwrap().is_empty());
    assert!(db
        .get_directory_listing(Path::new("/music"), "audio/")
        .await
        .unwrap()
        .0
        .is_empty());
    assert!(db.get_playlist_tracks(playlist).await.unwrap().is_empty());
    let stats = db.get_stats().await.unwrap();
    assert_eq!(stats.total_files, 0);
    assert_eq!(stats.total_size, 0);
}

#[tokio::test]
async fn path_prefix_removal_respects_component_boundaries() {
    let temp = tempdir().unwrap();
    let db = RedbDatabase::new(temp.path().join("prefix.redb"))
        .await
        .unwrap();
    db.initialize().await.unwrap();
    let a = MediaFile::new(PathBuf::from("/media/Film/a.mkv"), 1, "video/x".to_string());
    let b = MediaFile::new(
        PathBuf::from("/media/Films/b.mkv"),
        1,
        "video/x".to_string(),
    );
    db.bulk_store_media_files(&[a, b.clone()]).await.unwrap();
    db.remove_media_under_path(Path::new("/media/Film"))
        .await
        .unwrap();
    assert!(db.get_file_by_path(&b.path).await.unwrap().is_some());
}

#[tokio::test]
async fn targeted_removal_prunes_orphan_directory_artifacts() {
    let temp = tempdir().unwrap();
    let db = RedbDatabase::new(temp.path().join("orphan-subtree.redb"))
        .await
        .unwrap();
    db.initialize().await.unwrap();

    {
        let database = db.db.read().unwrap();
        let transaction = database.begin_write().unwrap();
        {
            let mut paths = transaction.open_table(DIRECTORY_PATH_INDEX).unwrap();
            let mut records = transaction.open_table(DIRECTORY_RECORDS).unwrap();
            for (path, id) in [
                ("/media", 900),
                ("/media/orphan", 901),
                ("/media/orphan/child", 902),
                ("/media/orphans", 903),
            ] {
                paths.insert(path, id).unwrap();
                records.insert(id, path).unwrap();
            }
            let mut children = transaction.open_multimap_table(DIRECTORY_CHILDREN).unwrap();
            children.insert(900, 901).unwrap();
            children.insert(901, 902).unwrap();
            children.insert(900, 903).unwrap();
            let mut ordered = transaction.open_table(DIRECTORY_CHILDREN_BY_NAME).unwrap();
            ordered
                .insert(
                    RedbDatabase::directory_order_key(900, "/media/orphan", 901).as_str(),
                    901,
                )
                .unwrap();
            ordered
                .insert(
                    RedbDatabase::directory_order_key(901, "/media/orphan/child", 902).as_str(),
                    902,
                )
                .unwrap();
            ordered
                .insert(
                    RedbDatabase::directory_order_key(900, "/media/orphans", 903).as_str(),
                    903,
                )
                .unwrap();
            transaction
                .open_table(DIRECTORY_MIME_COUNTS)
                .unwrap()
                .insert("901:*", 99)
                .unwrap();
        }
        transaction.commit().unwrap();
    }

    let summary = db
        .remove_media_under_path(Path::new("/media/orphan"))
        .await
        .unwrap();
    assert_eq!(summary.removed_files, 0);

    let database = db.db.read().unwrap();
    let transaction = database.begin_read().unwrap();
    let paths = transaction.open_table(DIRECTORY_PATH_INDEX).unwrap();
    assert!(paths.get("/media/orphan").unwrap().is_none());
    assert!(paths.get("/media/orphan/child").unwrap().is_none());
    assert_eq!(paths.get("/media/orphans").unwrap().unwrap().value(), 903);
    let children = transaction.open_multimap_table(DIRECTORY_CHILDREN).unwrap();
    let remaining = children
        .get(900)
        .unwrap()
        .map(|value| value.unwrap().value())
        .collect::<Vec<_>>();
    assert_eq!(remaining, vec![903]);
}

#[test]
fn schema_registry_has_unique_names() {
    let mut names = Vec::new();
    macro_rules! collect_schema_name {
        ($kind:ident, $constant:ident, $key:ty, $value:ty, $name:literal, $role:ident) => {
            names.push($name);
        };
    }
    redb_schema!(collect_schema_name);
    let unique = names.iter().copied().collect::<HashSet<_>>();
    assert_eq!(names.len(), unique.len());
    assert_eq!(names.len(), 20);
}

#[tokio::test]
async fn directory_visibility_respects_mime_family() {
    let temp = tempdir().unwrap();
    let db = RedbDatabase::new(temp.path().join("mime-filter.redb"))
        .await
        .unwrap();
    db.initialize().await.unwrap();
    let video = MediaFile::new(
        PathBuf::from("/media/mixed/movie.mkv"),
        1,
        "video/x-matroska".to_string(),
    );
    let audio = MediaFile::new(
        PathBuf::from("/media/mixed/song.mp3"),
        1,
        "audio/mpeg".to_string(),
    );
    db.bulk_store_media_files(&[video.clone(), audio])
        .await
        .unwrap();

    assert_eq!(
        db.get_directory_listing(Path::new("/media"), "video/")
            .await
            .unwrap()
            .0
            .len(),
        1
    );
    db.remove_media_file(&video.path).await.unwrap();
    assert!(db
        .get_directory_listing(Path::new("/media"), "video/")
        .await
        .unwrap()
        .0
        .is_empty());
    assert_eq!(
        db.get_directory_listing(Path::new("/media"), "audio/")
            .await
            .unwrap()
            .0
            .len(),
        1
    );
}

#[tokio::test]
async fn test_redb_database_playlist_operations() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test_playlist.redb");

    let db = RedbDatabase::new(db_path).await.unwrap();
    db.initialize().await.unwrap();

    // Store some test files
    let files: Vec<MediaFile> = (0..5)
        .map(|i| {
            MediaFile::new(
                PathBuf::from(format!("/music/song{}.mp3", i)),
                1024,
                "audio/mpeg".to_string(),
            )
        })
        .collect();
    let file_ids = db.bulk_store_media_files(&files).await.unwrap();

    // Create a playlist
    let playlist_id = db
        .create_playlist("Test Playlist", Some("A test playlist"))
        .await
        .unwrap();
    assert!(playlist_id > 0);

    // Verify playlist was created
    let playlists = db.get_playlists().await.unwrap();
    assert_eq!(playlists.len(), 1);
    assert_eq!(playlists[0].name, "Test Playlist");

    // Add tracks to playlist
    for (i, file_id) in file_ids.iter().enumerate() {
        db.add_to_playlist(playlist_id, *file_id, Some(i as u32))
            .await
            .unwrap();
    }

    // Get playlist tracks
    let tracks = db.get_playlist_tracks(playlist_id).await.unwrap();
    assert_eq!(tracks.len(), 5);
    assert_eq!(tracks[0].filename, "song0.mp3");
    assert_eq!(tracks[4].filename, "song4.mp3");

    // Remove a track
    let removed = db
        .remove_from_playlist(playlist_id, file_ids[2])
        .await
        .unwrap();
    assert!(removed);

    // Verify track was removed
    let tracks_after_remove = db.get_playlist_tracks(playlist_id).await.unwrap();
    assert_eq!(tracks_after_remove.len(), 4);

    // Delete the playlist
    let deleted = db.delete_playlist(playlist_id).await.unwrap();
    assert!(deleted);

    // Verify playlist was deleted
    let playlists_after_delete = db.get_playlists().await.unwrap();
    assert_eq!(playlists_after_delete.len(), 0);
}

#[tokio::test]
async fn deleting_source_tree_removes_derived_playlist_and_radio() {
    let temp = tempdir().unwrap();
    let db = RedbDatabase::new(temp.path().join("playlist-source.redb"))
        .await
        .unwrap();
    db.initialize().await.unwrap();
    db.rebuild_derived_indexes().await.unwrap();

    let source = PathBuf::from("/music/playlists/stations.m3u");
    let playlist = db.create_playlist("Stations", None).await.unwrap();
    db.set_playlist_source(playlist, &source).await.unwrap();
    let mut radio = MediaFile::new(
        PathBuf::from("https://radio.example/stream"),
        0,
        "audio/radio".to_string(),
    );
    radio.album = Some(
        RedbDatabase::canonical_path(&source)
            .unwrap()
            .to_string_lossy()
            .to_string(),
    );
    let radio_id = db.store_media_file(&radio).await.unwrap();

    assert_eq!(
        db.remove_derived_content_by_source(Path::new("/music/playlists"))
            .await
            .unwrap(),
        2
    );
    assert!(db.get_playlist(playlist).await.unwrap().is_none());
    assert!(db.get_file_by_id(radio_id).await.unwrap().is_none());
}

#[tokio::test]
async fn backup_and_offline_restore_preserve_a_valid_snapshot() {
    let temp = tempdir().unwrap();
    let database_path = temp.path().join("active.redb");
    let backup_path = temp.path().join("snapshot.redb");
    let db = RedbDatabase::new(database_path.clone()).await.unwrap();
    db.initialize().await.unwrap();
    db.rebuild_derived_indexes().await.unwrap();
    db.store_media_file(&MediaFile::new(
        PathBuf::from("/media/original.mp4"),
        42,
        "video/mp4".to_owned(),
    ))
    .await
    .unwrap();
    db.create_backup(&backup_path).await.unwrap();
    drop(db);

    let replacement = RedbDatabase::new(database_path.clone()).await.unwrap();
    replacement.initialize().await.unwrap();
    replacement
        .store_media_file(&MediaFile::new(
            PathBuf::from("/media/replacement.mp4"),
            7,
            "video/mp4".to_owned(),
        ))
        .await
        .unwrap();
    drop(replacement);

    RedbDatabase::restore_backup_file(backup_path, database_path.clone())
        .await
        .unwrap();
    let restored = RedbDatabase::new(database_path).await.unwrap();
    restored.initialize().await.unwrap();
    assert!(restored
        .get_file_by_path(Path::new("/media/original.mp4"))
        .await
        .unwrap()
        .is_some());
    assert!(restored
        .get_file_by_path(Path::new("/media/replacement.mp4"))
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn direct_directory_visitor_orders_and_pages_before_loading_records() {
    let temp = tempdir().unwrap();
    let db = Arc::new(
        RedbDatabase::new(temp.path().join("ordered.redb"))
            .await
            .unwrap(),
    );
    db.initialize().await.unwrap();
    db.bulk_store_media_files(&[
        MediaFile::new(
            PathBuf::from("/media/Zeta/z.mp4"),
            1,
            "video/mp4".to_owned(),
        ),
        MediaFile::new(
            PathBuf::from("/media/alpha/a.mp4"),
            1,
            "video/mp4".to_owned(),
        ),
    ])
    .await
    .unwrap();

    let (summary, names) = db
        .clone()
        .read(|session| {
            let mut names = Vec::new();
            let summary = session.visit_direct_subdirectories(
                "/media",
                Some("video/"),
                0,
                1,
                |directory| {
                    names.push(directory.name().to_owned());
                    Ok(())
                },
            )?;
            Ok((summary, names))
        })
        .await
        .unwrap();
    assert_eq!(summary.matched, 2);
    assert_eq!(summary.visited, 1);
    assert_eq!(names, ["alpha"]);
}

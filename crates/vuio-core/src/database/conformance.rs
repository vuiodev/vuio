//! Behaviour every database backend must reproduce.
//!
//! The repository traits say what a backend implements; they cannot say what
//! the answers must be. This suite does, written against `DatabaseBackend` and
//! nothing else, so a new backend inherits the whole of it by invoking
//! [`backend_conformance_tests!`] with its type.
//!
//! Assertions here are deliberately limited to behaviour the application
//! actually depends on. Where two reasonable backends could legitimately
//! differ — the order of records with mixed track-number presence, say — the
//! case is left unasserted and the reason recorded at the call site.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tempfile::TempDir;

use super::{
    DatabaseBackend, DatabaseReadSession, DatabaseSettings, DirectoryView,
    MediaFile, MediaFileQuery, MediaFileView,
};

/// Open an initialized backend inside `directory`.
///
/// The cache budget is deliberately small: a conformance run should exercise
/// eviction paths rather than fit entirely in memory.
async fn open<B: DatabaseBackend>(directory: &TempDir, name: &str) -> Arc<B> {
    let path = directory
        .path()
        .join(format!("{name}.{}", B::file_extension()));
    let database = B::open(&DatabaseSettings::new(path, 4))
        .await
        .expect("backend failed to open");
    database
        .initialize()
        .await
        .expect("backend failed to initialize");
    Arc::new(database)
}

fn audio(path: &str, size: u64) -> MediaFile {
    MediaFile::new(PathBuf::from(path), size, "audio/mpeg".to_string())
}

fn video(path: &str, size: u64) -> MediaFile {
    MediaFile::new(PathBuf::from(path), size, "video/x-matroska".to_string())
}

/// The canonical form of a path as the backend will have stored it.
///
/// Backends normalize incoming paths through the platform normalizer, so a
/// test that needs to name a stored path by string must apply the same rule.
fn canonical(path: &Path) -> String {
    crate::platform::filesystem::create_platform_path_normalizer()
        .to_canonical(path)
        .expect("path normalization failed")
}

// ── Media records ──────────────────────────────────────────────────────────

pub async fn crud_round_trip<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "crud").await;

    let file = audio("/music/test.mp3", 1024);
    let id = database.store_media_file(&file).await.unwrap();
    assert!(id > 0, "a stored record must receive a positive id");

    let by_path = database
        .get_file_by_path(Path::new("/music/test.mp3"))
        .await
        .unwrap()
        .expect("record missing after store");
    assert_eq!(by_path.filename, "test.mp3");
    assert_eq!(by_path.size, 1024);
    assert_eq!(by_path.mime_type, "audio/mpeg");
    assert_eq!(by_path.id, Some(id));

    let by_id = database.get_file_by_id(id).await.unwrap();
    assert_eq!(by_id.map(|file| file.path), Some(by_path.path.clone()));

    let location = database
        .get_file_location_by_id(id)
        .await
        .unwrap()
        .expect("streaming location missing");
    assert_eq!(location.filename, "test.mp3");
    assert_eq!(location.size, 1024);

    assert!(database
        .remove_media_file(Path::new("/music/test.mp3"))
        .await
        .unwrap());
    assert!(database
        .get_file_by_path(Path::new("/music/test.mp3"))
        .await
        .unwrap()
        .is_none());
    assert!(
        !database
            .remove_media_file(Path::new("/music/test.mp3"))
            .await
            .unwrap(),
        "removing an absent record reports no deletion"
    );
}

pub async fn storing_the_same_path_twice_updates_in_place<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "upsert").await;

    let mut file = audio("/music/song.mp3", 10);
    let first = database.store_media_file(&file).await.unwrap();

    file.size = 20;
    file.title = Some("Renamed".to_string());
    let second = database
        .bulk_store_media_files(std::slice::from_ref(&file))
        .await
        .unwrap();
    assert_eq!(
        vec![first],
        second,
        "re-storing a path must reuse its identifier"
    );

    let stored = database
        .get_file_by_path(Path::new("/music/song.mp3"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.size, 20);
    assert_eq!(stored.title.as_deref(), Some("Renamed"));
    assert_eq!(database.get_stats().await.unwrap().total_files, 1);
}

pub async fn bulk_store_and_stats<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "bulk").await;

    let files: Vec<MediaFile> = (0..100)
        .map(|index| audio(&format!("/music/song{index}.mp3"), 1024))
        .collect();
    let ids = database.bulk_store_media_files(&files).await.unwrap();
    assert_eq!(ids.len(), 100);
    assert_eq!(ids.iter().collect::<HashSet<_>>().len(), 100);

    database
        .bulk_store_media_files(&[video("/media/clip.mkv", 2048)])
        .await
        .unwrap();

    let stats = database.get_stats().await.unwrap();
    assert_eq!(stats.total_files, 101);
    assert_eq!(stats.total_size, 100 * 1024 + 2048);
    assert_eq!(stats.audio_files, 100);
    assert_eq!(stats.video_files, 1);
    assert!(
        stats.database_size > 0,
        "a populated database reports a non-zero file size"
    );

    let collected = database.collect_all_media_files().await.unwrap();
    assert_eq!(collected.len(), 101);
}

pub async fn bulk_update_rewrites_metadata<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "bulk-update").await;

    let files: Vec<MediaFile> = (0..5)
        .map(|index| audio(&format!("/music/song{index}.mp3"), 1))
        .collect();
    database.bulk_store_media_files(&files).await.unwrap();

    let mut stored = database.collect_all_media_files().await.unwrap();
    for file in &mut stored {
        file.artist = Some("Updated Artist".to_string());
        file.year = Some(2026);
    }
    database.bulk_update_media_files(&stored).await.unwrap();

    let artists = database.get_artists().await.unwrap();
    assert_eq!(artists.len(), 1);
    assert_eq!(artists[0].name, "Updated Artist");
    assert_eq!(artists[0].count, 5);
    assert_eq!(database.get_music_by_year(2026).await.unwrap().len(), 5);
}

pub async fn fingerprints_match_stored_records<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "fingerprints").await;

    let files: Vec<MediaFile> = (0..3)
        .map(|index| audio(&format!("/music/song{index}.mp3"), 100 + index as u64))
        .collect();
    database.bulk_store_media_files(&files).await.unwrap();

    let mut fingerprints = database.load_file_fingerprints().await.unwrap();
    fingerprints.sort_by(|left, right| left.path.cmp(&right.path));
    assert_eq!(fingerprints.len(), 3);

    for fingerprint in &fingerprints {
        let full = database
            .get_file_by_id(fingerprint.id)
            .await
            .unwrap()
            .expect("fingerprint points at a missing record");
        assert_eq!(full.path, fingerprint.path);
        assert_eq!(full.size, fingerprint.size);
        // Timestamps round-trip through whole seconds in storage.
        assert_eq!(
            full.modified
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            fingerprint
                .modified
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );
    }
}

pub async fn lookup_by_multiple_paths<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "by-paths").await;

    let files: Vec<MediaFile> = (0..4)
        .map(|index| audio(&format!("/music/song{index}.mp3"), 1))
        .collect();
    database.bulk_store_media_files(&files).await.unwrap();

    let wanted = vec![
        PathBuf::from("/music/song1.mp3"),
        PathBuf::from("/music/song3.mp3"),
        PathBuf::from("/music/absent.mp3"),
    ];
    let found = database.get_files_by_paths(&wanted).await.unwrap();
    let names = found
        .iter()
        .map(|file| file.filename.clone())
        .collect::<HashSet<_>>();
    assert_eq!(
        names,
        HashSet::from(["song1.mp3".to_string(), "song3.mp3".to_string()]),
        "absent paths are skipped rather than reported"
    );
}

// ── Directories ────────────────────────────────────────────────────────────

pub async fn directory_listing_filters_by_mime_family<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "mime-filter").await;

    let clip = video("/media/mixed/movie.mkv", 1);
    database
        .bulk_store_media_files(&[clip.clone(), audio("/media/mixed/song.mp3", 1)])
        .await
        .unwrap();

    let (directories, _) = database
        .get_directory_listing(Path::new("/media"), "video/")
        .await
        .unwrap();
    assert_eq!(directories.len(), 1);
    assert_eq!(directories[0].name, "mixed");

    let (_, files) = database
        .get_directory_listing(Path::new("/media/mixed"), "audio/")
        .await
        .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].filename, "song.mp3");

    database.remove_media_file(&clip.path).await.unwrap();
    assert!(
        database
            .get_directory_listing(Path::new("/media"), "video/")
            .await
            .unwrap()
            .0
            .is_empty(),
        "a directory with no remaining video must stop appearing under a video filter"
    );
    assert_eq!(
        database
            .get_directory_listing(Path::new("/media"), "audio/")
            .await
            .unwrap()
            .0
            .len(),
        1
    );
}

pub async fn directories_are_visible_from_deep_descendants<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "deep").await;

    // The only file lives three levels down, so every ancestor exists solely
    // by implication. Each must still be browsable.
    database
        .bulk_store_media_files(&[video("/media/shows/season/episode/pilot.mkv", 1)])
        .await
        .unwrap();

    for (parent, expected) in [
        ("/media", "shows"),
        ("/media/shows", "season"),
        ("/media/shows/season", "episode"),
    ] {
        let children = database
            .get_direct_subdirectories(&canonical(Path::new(parent)))
            .await
            .unwrap();
        assert_eq!(
            children.len(),
            1,
            "expected exactly one child of {parent}, got {children:?}"
        );
        assert_eq!(children[0].name, expected);

        let filtered = database
            .get_filtered_direct_subdirectories(&canonical(Path::new(parent)), "video/")
            .await
            .unwrap();
        assert_eq!(
            filtered.len(),
            1,
            "video filter hid an ancestor of a video file at {parent}"
        );
        assert!(
            database
                .get_filtered_direct_subdirectories(&canonical(Path::new(parent)), "audio/")
                .await
                .unwrap()
                .is_empty(),
            "audio filter must not match a subtree holding only video at {parent}"
        );
    }
}

pub async fn subtree_removal_respects_component_boundaries<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "prefix").await;

    let kept = video("/media/Films/b.mkv", 1);
    database
        .bulk_store_media_files(&[video("/media/Film/a.mkv", 1), kept.clone()])
        .await
        .unwrap();

    let summary = database
        .remove_media_under_path(Path::new("/media/Film"))
        .await
        .unwrap();
    assert_eq!(summary.removed_files, 1);
    assert!(
        database.get_file_by_path(&kept.path).await.unwrap().is_some(),
        "/media/Films must survive removal of /media/Film"
    );
    assert!(
        database
            .get_directory_listing(Path::new("/media"), "video/")
            .await
            .unwrap()
            .0
            .iter()
            .all(|directory| directory.name == "Films"),
        "the emptied directory must not linger in listings"
    );
}

pub async fn path_prefix_query_is_bounded_by_the_prefix<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "path-prefix").await;

    database
        .bulk_store_media_files(&[
            video("/media/Film/a.mkv", 1),
            video("/media/Film/nested/b.mkv", 1),
            video("/media/Films/c.mkv", 1),
        ])
        .await
        .unwrap();

    let matched = database
        .get_files_with_path_prefix(&canonical(Path::new("/media/Film")))
        .await
        .unwrap();
    let names = matched
        .iter()
        .map(|file| file.filename.clone())
        .collect::<HashSet<_>>();
    assert_eq!(
        names,
        HashSet::from(["a.mkv".to_string(), "b.mkv".to_string()]),
        "prefix matching must stop at a path component boundary"
    );
}

pub async fn cleanup_drops_records_missing_from_disk<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "cleanup").await;

    let files: Vec<MediaFile> = (0..4)
        .map(|index| audio(&format!("/music/song{index}.mp3"), 1))
        .collect();
    database.bulk_store_media_files(&files).await.unwrap();

    let surviving = [
        canonical(Path::new("/music/song0.mp3")),
        canonical(Path::new("/music/song2.mp3")),
    ]
    .into_iter()
    .collect::<HashSet<_>>();

    let removed = database
        .batch_cleanup_missing_files(&surviving)
        .await
        .unwrap();
    assert_eq!(removed, 2);
    assert_eq!(database.get_stats().await.unwrap().total_files, 2);
    assert!(database
        .get_file_by_path(Path::new("/music/song0.mp3"))
        .await
        .unwrap()
        .is_some());
    assert!(database
        .get_file_by_path(Path::new("/music/song1.mp3"))
        .await
        .unwrap()
        .is_none());
}

// ── Music categorization ───────────────────────────────────────────────────

pub async fn music_categories_group_and_count<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "music").await;

    let mut records = Vec::new();
    for (index, (artist, album, genre, year)) in [
        ("Artist A", "Album One", "Rock", 2001),
        ("Artist A", "Album One", "Rock", 2001),
        ("Artist A", "Album Two", "Jazz", 2002),
        ("Artist B", "Album Three", "Rock", 2001),
    ]
    .into_iter()
    .enumerate()
    {
        let mut file = audio(&format!("/music/track{index}.mp3"), 1);
        file.artist = Some(artist.to_string());
        file.album = Some(album.to_string());
        file.genre = Some(genre.to_string());
        file.year = Some(year);
        file.album_artist = Some(artist.to_string());
        records.push(file);
    }
    // One record with no tags at all must not create empty categories.
    records.push(audio("/music/untagged.mp3", 1));
    database.bulk_store_media_files(&records).await.unwrap();

    let artists = database.get_artists().await.unwrap();
    assert_eq!(artists.len(), 2);
    let counts = artists
        .iter()
        .map(|category| (category.name.clone(), category.count))
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(counts.get("Artist A"), Some(&3));
    assert_eq!(counts.get("Artist B"), Some(&1));

    assert_eq!(database.get_albums(None).await.unwrap().len(), 3);
    assert_eq!(
        database.get_albums(Some("Artist A")).await.unwrap().len(),
        2,
        "album listing must narrow to the requested artist"
    );
    assert_eq!(database.get_genres().await.unwrap().len(), 2);
    assert_eq!(database.get_years().await.unwrap().len(), 2);
    assert_eq!(database.get_album_artists().await.unwrap().len(), 2);

    assert_eq!(
        database.get_music_by_artist("Artist A").await.unwrap().len(),
        3
    );
    assert_eq!(
        database
            .get_music_by_album("Album One", None)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        database
            .get_music_by_album("Album One", Some("Artist B"))
            .await
            .unwrap()
            .len(),
        0,
        "an album query must respect its artist qualifier"
    );
    assert_eq!(database.get_music_by_genre("Rock").await.unwrap().len(), 3);
    assert_eq!(database.get_music_by_year(2001).await.unwrap().len(), 3);
    assert_eq!(
        database
            .get_music_by_album_artist("Artist B")
            .await
            .unwrap()
            .len(),
        1
    );
}

pub async fn removing_the_last_tagged_record_empties_its_categories<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "categories-empty").await;

    let mut file = video("/media/deleted/movie.mkv", 1);
    file.artist = Some("Ghost Artist".to_string());
    file.album = Some("Ghost Album".to_string());
    file.genre = Some("Ghost Genre".to_string());
    file.year = Some(2026);
    file.album_artist = Some("Ghost Album Artist".to_string());
    database.store_media_file(&file).await.unwrap();

    assert_eq!(
        database
            .remove_media_under_path(Path::new("/media/deleted"))
            .await
            .unwrap()
            .removed_files,
        1
    );

    let (directories, files) = database
        .get_directory_listing(Path::new("/media"), "video/")
        .await
        .unwrap();
    assert!(directories.is_empty(), "stale directories: {directories:?}");
    assert!(files.is_empty());
    assert!(database.get_artists().await.unwrap().is_empty());
    assert!(database.get_albums(None).await.unwrap().is_empty());
    assert!(database.get_genres().await.unwrap().is_empty());
    assert!(database.get_years().await.unwrap().is_empty());
    assert!(database.get_album_artists().await.unwrap().is_empty());
}

// ── Read sessions ──────────────────────────────────────────────────────────

pub async fn directory_visitor_orders_and_pages<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "ordered-dirs").await;

    database
        .bulk_store_media_files(&[
            video("/media/Zeta/z.mkv", 1),
            video("/media/alpha/a.mkv", 1),
        ])
        .await
        .unwrap();

    let (summary, names) = database
        .clone()
        .read(|session| {
            let mut names = Vec::new();
            let summary =
                session.visit_direct_subdirectories("/media", Some("video/"), 0, 1, |directory| {
                    names.push(directory.name().to_owned());
                    Ok(())
                })?;
            Ok((summary, names))
        })
        .await
        .unwrap();

    // `matched` counts the whole result, `visited` only the page — the DLNA
    // layer reports the former as TotalMatches while returning the latter.
    assert_eq!(summary.matched, 2);
    assert_eq!(summary.visited, 1);
    assert_eq!(
        names,
        ["alpha"],
        "directory ordering is case-insensitive and natural"
    );
}

pub async fn file_visitor_pages_a_directory_in_natural_order<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "ordered-files").await;

    // Episode numbering is the case that plain lexical ordering gets wrong.
    database
        .bulk_store_media_files(&[
            video("/media/show/s01e10.mkv", 1),
            video("/media/show/s01e2.mkv", 1),
            video("/media/show/s01e1.mkv", 1),
        ])
        .await
        .unwrap();

    let query = MediaFileQuery::Directory {
        path: canonical(Path::new("/media/show")),
        mime_family: Some("video/".to_string()),
    };

    let (summary, names) = database
        .clone()
        .read(move |session| {
            let mut names = Vec::new();
            let summary = session.visit_files(&query, 0, 2, |file| {
                names.push(file.filename().to_owned());
                Ok(())
            })?;
            Ok((summary, names))
        })
        .await
        .unwrap();
    assert_eq!(summary.matched, 3);
    assert_eq!(summary.visited, 2);
    assert_eq!(names, ["s01e1.mkv", "s01e2.mkv"]);

    let query = MediaFileQuery::Directory {
        path: canonical(Path::new("/media/show")),
        mime_family: Some("video/".to_string()),
    };
    let tail = database
        .clone()
        .read(move |session| {
            let mut names = Vec::new();
            session.visit_files(&query, 2, 2, |file| {
                names.push(file.filename().to_owned());
                Ok(())
            })?;
            Ok(names)
        })
        .await
        .unwrap();
    assert_eq!(tail, ["s01e10.mkv"], "offset must resume where the page ended");
}

pub async fn album_tracks_order_by_track_number<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "track-order").await;

    // Every record carries a track number here. Ordering across a mix of
    // tagged and untagged records is left unasserted: it is not a distinction
    // the application relies on, and backends may resolve it differently.
    let mut records = Vec::new();
    for (track, name) in [(3, "c.mp3"), (1, "a.mp3"), (2, "b.mp3")] {
        let mut file = audio(&format!("/music/album/{name}"), 1);
        file.album = Some("Ordered".to_string());
        file.track_number = Some(track);
        records.push(file);
    }
    database.bulk_store_media_files(&records).await.unwrap();

    let query = MediaFileQuery::Album {
        album: "Ordered".to_string(),
        artist: None,
    };
    let names = database
        .clone()
        .read(move |session| {
            let mut names = Vec::new();
            session.visit_files(&query, 0, 10, |file| {
                names.push(file.filename().to_owned());
                Ok(())
            })?;
            Ok(names)
        })
        .await
        .unwrap();
    assert_eq!(names, ["a.mp3", "b.mp3", "c.mp3"]);
}

/// A multi-disc album plays in the order it was pressed.
///
/// Disc 2 track 1 belongs after disc 1 track 12, not before it, which is what
/// ordering on the track number alone would give.
pub async fn album_tracks_order_by_disc_then_track<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "disc-order").await;

    let mut records = Vec::new();
    for (disc, track, name) in [
        (Some(2), Some(1), "d2t1.mp3"),
        (Some(1), Some(12), "d1t12.mp3"),
        (Some(1), Some(2), "d1t2.mp3"),
        // No disc tag at all belongs with disc one, which is how a
        // single-disc release is tagged.
        (None, Some(1), "d0t1.mp3"),
    ] {
        let mut file = audio(&format!("/music/box/{name}"), 1);
        file.album = Some("Boxed".to_string());
        file.tags.disc_number = disc;
        file.track_number = track;
        records.push(file);
    }
    // An untagged record sorts last, as it does everywhere else.
    let mut untagged = audio("/music/box/zz.mp3", 1);
    untagged.album = Some("Boxed".to_string());
    records.push(untagged);

    database.bulk_store_media_files(&records).await.unwrap();

    let query = MediaFileQuery::Album {
        album: "Boxed".to_string(),
        artist: None,
    };
    let names = database
        .clone()
        .read(move |session| {
            let mut names = Vec::new();
            session.visit_files(&query, 0, 10, |file| {
                names.push(file.filename().to_owned());
                Ok(())
            })?;
            Ok(names)
        })
        .await
        .unwrap();
    assert_eq!(
        names,
        ["d0t1.mp3", "d1t2.mp3", "d1t12.mp3", "zz.mp3", "d2t1.mp3"]
    );
}

/// One query serves both a flat category listing and a nested one.
pub async fn music_categories_narrow_to_their_filter<B: DatabaseBackend>() {
    use crate::database::{MusicCategoryFilter, MusicCategoryType};

    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "nested-categories").await;

    let mut records = Vec::new();
    for (index, (artist, album, genre)) in [
        ("Metallica", "Ride the Lightning", "Metal"),
        ("Metallica", "Load", "Rock"),
        ("Portishead", "Dummy", "Trip Hop"),
        // Two different artists with an identically named album: the reason a
        // nested album listing has to be scoped to its artist.
        ("Artist A", "Greatest Hits", "Rock"),
        ("Artist B", "Greatest Hits", "Rock"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut file = audio(&format!("/music/t{index}.mp3"), 1);
        file.artist = Some(artist.to_string());
        file.album = Some(album.to_string());
        file.genre = Some(genre.to_string());
        records.push(file);
    }
    database.bulk_store_media_files(&records).await.unwrap();

    let albums_of = |artist: &str| {
        let filter = MusicCategoryFilter::artist(artist);
        let database = database.clone();
        async move {
            database
                .get_music_categories(MusicCategoryType::Album, &filter, None)
                .await
                .unwrap()
                .into_iter()
                .map(|category| category.name)
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(albums_of("Metallica").await, ["Load", "Ride the Lightning"]);
    assert_eq!(albums_of("Portishead").await, ["Dummy"]);

    // Same title, two artists: one album container each, not one shared.
    assert_eq!(albums_of("Artist A").await, ["Greatest Hits"]);
    assert_eq!(albums_of("Artist B").await, ["Greatest Hits"]);

    // A genre lists the artists within it, which is the level minidlna puts
    // between a genre and its albums.
    let rock = database
        .get_music_categories(
            MusicCategoryType::Artist,
            &MusicCategoryFilter::genre("Rock"),
            Some(MusicCategoryType::Album),
        )
        .await
        .unwrap();
    let rock_artists = rock
        .iter()
        .map(|category| category.name.clone())
        .collect::<Vec<_>>();
    assert_eq!(rock_artists, ["Artist A", "Artist B", "Metallica"]);

    // A container whose children are containers must count the containers.
    // Metallica has two Rock tracks but only one Rock album, and announcing
    // two would promise a child the browse never returns.
    let metallica = rock
        .iter()
        .find(|category| category.name == "Metallica")
        .unwrap();
    assert_eq!(metallica.count, 1, "one Metallica track is tagged Rock");
    assert_eq!(metallica.child_count, Some(1), "in one album");

    // Without a child tag there is nothing to count, and the caller falls back
    // to the record count.
    assert!(database
        .get_music_categories(
            MusicCategoryType::Artist,
            &MusicCategoryFilter::default(),
            None,
        )
        .await
        .unwrap()
        .iter()
        .all(|category| category.child_count.is_none()));

    // And a genre-and-artist pair narrows to that artist's albums in it.
    let scoped = database
        .get_music_categories(
            MusicCategoryType::Album,
            &MusicCategoryFilter::genre("Rock").with_artist("Metallica"),
            None,
        )
        .await
        .unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].name, "Load");
    assert!(
        scoped[0].sample_id.is_some(),
        "a category must name a record whose cover art can represent it"
    );
}

/// Internet radio is audio but not part of a music library.
///
/// A radio station is stored with its source playlist path as the album, so a
/// category listing that does not exclude it grows a container named after a
/// file path — one that lists nothing when opened, because every track query
/// the browse tree builds does exclude radio.
pub async fn radio_records_do_not_become_music_categories<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "radio-categories").await;

    let mut station = MediaFile::new(
        PathBuf::from("https://radio.example/stream"),
        0,
        "audio/radio".to_string(),
    );
    station.album = Some("/media/radio/stations.m3u".to_string());
    station.artist = Some("Example Radio".to_string());

    let mut track = audio("/music/real.mp3", 1);
    track.album = Some("A Real Album".to_string());
    track.artist = Some("A Real Artist".to_string());

    database
        .bulk_store_media_files(&[station, track])
        .await
        .unwrap();

    let names = |categories: Vec<crate::database::MusicCategory>| {
        categories
            .into_iter()
            .map(|category| category.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(names(database.get_albums(None).await.unwrap()), ["A Real Album"]);
    assert_eq!(
        names(database.get_artists().await.unwrap()),
        ["A Real Artist"]
    );
}

/// Browsing the playlist list needs one child count per playlist.
pub async fn playlist_entry_counts_are_returned_together<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "playlist-counts").await;

    let ids = database
        .bulk_store_media_files(&[
            audio("/music/one.mp3", 1),
            audio("/music/two.mp3", 1),
            audio("/music/three.mp3", 1),
        ])
        .await
        .unwrap();

    let full = database.create_playlist("Full", None).await.unwrap();
    let empty = database.create_playlist("Empty", None).await.unwrap();
    database
        .batch_add_to_playlist(full, &[(ids[0], 0), (ids[1], 1), (ids[2], 2)])
        .await
        .unwrap();

    let counts = database.count_playlist_entries().await.unwrap();
    assert_eq!(counts.get(&full), Some(&3));
    // A playlist with no entries has no row to group, so it is simply absent.
    assert_eq!(counts.get(&empty), None);
}

pub async fn filtered_query_searches_text_and_pages_by_cursor<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "filtered").await;

    let mut tagged = audio("/music/first.mp3", 1);
    tagged.title = Some("Needle In Title".to_string());
    let mut by_artist = audio("/music/second.mp3", 1);
    by_artist.artist = Some("needle artist".to_string());
    let mut by_album = audio("/music/third.mp3", 1);
    by_album.album = Some("NEEDLE album".to_string());
    database
        .bulk_store_media_files(&[
            tagged,
            by_artist,
            by_album,
            audio("/music/needle-in-filename.mp3", 1),
            audio("/music/unrelated.mp3", 1),
            video("/media/needle.mkv", 1),
        ])
        .await
        .unwrap();

    let query = MediaFileQuery::Filtered {
        after_id: None,
        mime_family: Some("audio/".to_string()),
        text: Some("needle".to_string()),
    };
    let (summary, ids) = database
        .clone()
        .read(move |session| {
            let mut ids = Vec::new();
            let summary = session.visit_files(&query, 0, 100, |file| {
                ids.push(file.id().expect("stored records carry an id"));
                Ok(())
            })?;
            Ok((summary, ids))
        })
        .await
        .unwrap();
    assert_eq!(
        summary.matched, 4,
        "search covers filename, title, artist and album, case-insensitively"
    );
    assert_eq!(ids.len(), 4, "the video match is excluded by the mime filter");
    assert!(
        ids.windows(2).all(|pair| pair[0] < pair[1]),
        "filtered results are ordered by id so a cursor can resume: {ids:?}"
    );

    let cursor = ids[1];
    let query = MediaFileQuery::Filtered {
        after_id: Some(cursor),
        mime_family: Some("audio/".to_string()),
        text: Some("needle".to_string()),
    };
    let after = database
        .clone()
        .read(move |session| {
            let mut ids = Vec::new();
            session.visit_files(&query, 0, 100, |file| {
                ids.push(file.id().unwrap());
                Ok(())
            })?;
            Ok(ids)
        })
        .await
        .unwrap();
    assert_eq!(
        after,
        ids[2..].to_vec(),
        "a cursor resumes strictly after the given id"
    );
}

pub async fn read_session_finds_records_by_id_and_path<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "session-lookup").await;

    let id = database
        .store_media_file(&audio("/music/one.mp3", 7))
        .await
        .unwrap();
    let path = canonical(Path::new("/music/one.mp3"));

    let found = database
        .clone()
        .read(move |session| {
            let mut sizes = Vec::new();
            session.visit_files(&MediaFileQuery::Id(id), 0, 10, |file| {
                sizes.push(file.size());
                Ok(())
            })?;
            session.visit_files(&MediaFileQuery::Path(path.clone()), 0, 10, |file| {
                sizes.push(file.size());
                Ok(())
            })?;
            let mut total = 0;
            session.visit_files(&MediaFileQuery::All, 0, 10, |_| {
                total += 1;
                Ok(())
            })?;
            Ok((sizes, total))
        })
        .await
        .unwrap();
    assert_eq!(found, (vec![7, 7], 1));
}

// ── Playlists ──────────────────────────────────────────────────────────────

pub async fn playlist_lifecycle<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "playlists").await;

    let files: Vec<MediaFile> = (0..5)
        .map(|index| audio(&format!("/music/song{index}.mp3"), 1024))
        .collect();
    let file_ids = database.bulk_store_media_files(&files).await.unwrap();

    let playlist_id = database
        .create_playlist("Test Playlist", Some("A test playlist"))
        .await
        .unwrap();
    assert!(playlist_id > 0);

    let playlists = database.get_playlists().await.unwrap();
    assert_eq!(playlists.len(), 1);
    assert_eq!(playlists[0].name, "Test Playlist");
    assert_eq!(playlists[0].description.as_deref(), Some("A test playlist"));

    let fetched = database
        .get_playlist(playlist_id)
        .await
        .unwrap()
        .expect("playlist missing after create");
    assert_eq!(fetched.id, Some(playlist_id));

    for (position, file_id) in file_ids.iter().enumerate() {
        database
            .add_to_playlist(playlist_id, *file_id, Some(position as u32))
            .await
            .unwrap();
    }

    let tracks = database.get_playlist_tracks(playlist_id).await.unwrap();
    assert_eq!(tracks.len(), 5);
    assert_eq!(tracks[0].filename, "song0.mp3");
    assert_eq!(tracks[4].filename, "song4.mp3");

    assert!(database
        .remove_from_playlist(playlist_id, file_ids[2])
        .await
        .unwrap());
    assert_eq!(
        database
            .get_playlist_tracks(playlist_id)
            .await
            .unwrap()
            .len(),
        4
    );

    let mut updated = fetched;
    updated.name = "Renamed".to_string();
    database.update_playlist(&updated).await.unwrap();
    assert_eq!(
        database
            .get_playlist(playlist_id)
            .await
            .unwrap()
            .unwrap()
            .name,
        "Renamed"
    );

    assert!(database.delete_playlist(playlist_id).await.unwrap());
    assert!(database.get_playlists().await.unwrap().is_empty());
    assert!(database.get_playlist(playlist_id).await.unwrap().is_none());
    assert!(
        !database.delete_playlist(playlist_id).await.unwrap(),
        "deleting an absent playlist reports no deletion"
    );
}

pub async fn playlist_batch_add_and_reorder<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "playlist-order").await;

    let files: Vec<MediaFile> = (0..3)
        .map(|index| audio(&format!("/music/song{index}.mp3"), 1))
        .collect();
    let ids = database.bulk_store_media_files(&files).await.unwrap();
    let playlist = database.create_playlist("Ordered", None).await.unwrap();

    let entries = ids
        .iter()
        .enumerate()
        .map(|(position, id)| (*id, position as u32))
        .collect::<Vec<_>>();
    assert_eq!(
        database
            .batch_add_to_playlist(playlist, &entries)
            .await
            .unwrap()
            .len(),
        3
    );

    let names = |tracks: Vec<MediaFile>| {
        tracks
            .into_iter()
            .map(|file| file.filename)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names(database.get_playlist_tracks(playlist).await.unwrap()),
        ["song0.mp3", "song1.mp3", "song2.mp3"]
    );

    let reversed = ids
        .iter()
        .rev()
        .enumerate()
        .map(|(position, id)| (*id, position as u32))
        .collect::<Vec<_>>();
    database
        .reorder_playlist(playlist, &reversed)
        .await
        .unwrap();
    assert_eq!(
        names(database.get_playlist_tracks(playlist).await.unwrap()),
        ["song2.mp3", "song1.mp3", "song0.mp3"]
    );

    let visited = database
        .clone()
        .read(move |session| {
            let mut names = Vec::new();
            session.visit_files(&MediaFileQuery::Playlist(playlist), 0, 10, |file| {
                names.push(file.filename().to_owned());
                Ok(())
            })?;
            Ok(names)
        })
        .await
        .unwrap();
    assert_eq!(
        visited,
        ["song2.mp3", "song1.mp3", "song0.mp3"],
        "a playlist read session must honour stored positions"
    );
}

pub async fn deleting_a_file_removes_it_from_playlists<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "playlist-cascade").await;

    let ids = database
        .bulk_store_media_files(&[audio("/music/a.mp3", 1), audio("/music/b.mp3", 1)])
        .await
        .unwrap();
    let playlist = database.create_playlist("Cascade", None).await.unwrap();
    database
        .batch_add_to_playlist(playlist, &[(ids[0], 0), (ids[1], 1)])
        .await
        .unwrap();

    database
        .remove_media_file(Path::new("/music/a.mp3"))
        .await
        .unwrap();

    let tracks = database.get_playlist_tracks(playlist).await.unwrap();
    assert_eq!(
        tracks.len(),
        1,
        "a deleted record must not remain referenced by a playlist"
    );
    assert_eq!(tracks[0].filename, "b.mp3");
}

pub async fn source_derived_content_is_replaced_and_removed<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "playlist-source").await;

    let source = PathBuf::from("/music/playlists/stations.m3u");
    let ids = database
        .bulk_store_media_files(&[audio("/music/a.mp3", 1), audio("/music/b.mp3", 1)])
        .await
        .unwrap();

    let playlist = database
        .replace_playlist_from_source(&source, "Stations", &[(ids[0], 0)])
        .await
        .unwrap();
    assert_eq!(database.get_playlists().await.unwrap().len(), 1);
    assert_eq!(
        database.get_playlist_tracks(playlist).await.unwrap().len(),
        1
    );

    // Re-importing the same source replaces rather than duplicates.
    let replaced = database
        .replace_playlist_from_source(&source, "Stations", &[(ids[0], 0), (ids[1], 1)])
        .await
        .unwrap();
    assert_eq!(
        database.get_playlists().await.unwrap().len(),
        1,
        "re-importing a source must not create a second playlist"
    );
    assert_eq!(
        database.get_playlist_tracks(replaced).await.unwrap().len(),
        2
    );

    // Radio entries record their source in `album`, and are removed with it.
    let mut radio = MediaFile::new(
        PathBuf::from("https://radio.example/stream"),
        0,
        "audio/radio".to_string(),
    );
    radio.album = Some(canonical(&source));
    let radio_id = database.store_media_file(&radio).await.unwrap();

    assert_eq!(
        database
            .remove_derived_content_by_source(Path::new("/music/playlists"))
            .await
            .unwrap(),
        2
    );
    assert!(database.get_playlist(replaced).await.unwrap().is_none());
    assert!(database.get_file_by_id(radio_id).await.unwrap().is_none());
}

// ── Root availability ──────────────────────────────────────────────────────

pub async fn root_availability_round_trip<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "roots").await;

    let state = super::RootAvailability {
        path: PathBuf::from("/media/external"),
        last_seen_secs: 1_700_000_000,
        unavailable_since_secs: None,
        indexed_count: 42,
        reason: "mounted".to_string(),
    };
    database.set_root_availability(&state).await.unwrap();

    let stored = database
        .get_root_availability(Path::new("/media/external"))
        .await
        .unwrap()
        .expect("availability record missing");
    assert_eq!(stored.indexed_count, 42);
    assert_eq!(stored.reason, "mounted");
    assert_eq!(stored.last_seen_secs, 1_700_000_000);

    let updated = super::RootAvailability {
        unavailable_since_secs: Some(1_700_000_500),
        reason: "unmounted".to_string(),
        ..state
    };
    database.set_root_availability(&updated).await.unwrap();
    assert_eq!(database.list_root_availability().await.unwrap().len(), 1);
    assert_eq!(
        database
            .get_root_availability(Path::new("/media/external"))
            .await
            .unwrap()
            .unwrap()
            .unavailable_since_secs,
        Some(1_700_000_500)
    );

    database
        .remove_root_availability(Path::new("/media/external"))
        .await
        .unwrap();
    assert!(database.list_root_availability().await.unwrap().is_empty());
}

// ── Secrets ────────────────────────────────────────────────────────────────

pub async fn secrets_round_trip<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "secrets").await;

    assert_eq!(database.get_secret("airplay.pairings").await.unwrap(), None);
    database
        .set_secret("airplay.pairings", b"{\"version\":1}")
        .await
        .unwrap();
    assert_eq!(
        database
            .get_secret("airplay.pairings")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"{\"version\":1}"[..])
    );

    database
        .set_secret("airplay.pairings", b"replaced")
        .await
        .unwrap();
    assert_eq!(
        database
            .get_secret("airplay.pairings")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"replaced"[..])
    );

    assert!(database.delete_secret("airplay.pairings").await.unwrap());
    assert!(!database.delete_secret("airplay.pairings").await.unwrap());
    assert_eq!(database.get_secret("airplay.pairings").await.unwrap(), None);
}

// ── Health, durability, maintenance ────────────────────────────────────────

pub async fn records_survive_reopening<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(format!("persist.{}", B::file_extension()));
    let settings = DatabaseSettings::new(path, 4);

    {
        let database = B::open(&settings).await.unwrap();
        database.initialize().await.unwrap();
        let mut file = audio("/music/persisted.mp3", 99);
        file.artist = Some("Kept".to_string());
        database.store_media_file(&file).await.unwrap();
        let playlist = database.create_playlist("Kept", None).await.unwrap();
        database.set_playlist_source(playlist, Path::new("/music/kept.m3u")).await.unwrap();
        database.set_secret("kept", b"secret").await.unwrap();
    }

    let reopened = B::open(&settings).await.unwrap();
    reopened.initialize().await.unwrap();
    assert_eq!(
        reopened
            .get_file_by_path(Path::new("/music/persisted.mp3"))
            .await
            .unwrap()
            .unwrap()
            .size,
        99
    );
    assert_eq!(reopened.get_artists().await.unwrap().len(), 1);
    assert_eq!(reopened.get_playlists().await.unwrap().len(), 1);
    assert_eq!(
        reopened.get_secret("kept").await.unwrap().as_deref(),
        Some(&b"secret"[..])
    );
    assert_eq!(reopened.get_stats().await.unwrap().total_files, 1);
}

pub async fn health_check_reports_a_clean_database<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "health").await;

    database
        .bulk_store_media_files(&[audio("/music/a.mp3", 1), video("/media/b.mkv", 2)])
        .await
        .unwrap();

    let health = database.check_and_repair().await.unwrap();
    assert!(health.is_healthy, "issues: {:?}", health.issues);
    assert!(!health.corruption_detected);
    assert!(health.integrity_check_passed);

    // Rebuilding derived state must be idempotent and must not lose records.
    let rebuilt = database.rebuild_derived_indexes().await.unwrap();
    assert!(rebuilt.is_healthy, "issues: {:?}", rebuilt.issues);
    assert_eq!(database.get_stats().await.unwrap().total_files, 2);
    assert_eq!(
        database
            .get_directory_listing(Path::new("/music"), "audio/")
            .await
            .unwrap()
            .1
            .len(),
        1
    );
}

pub async fn vacuum_leaves_the_database_usable<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let database = open::<B>(&temp, "vacuum").await;

    let files: Vec<MediaFile> = (0..50)
        .map(|index| audio(&format!("/music/song{index}.mp3"), 1024))
        .collect();
    database.bulk_store_media_files(&files).await.unwrap();
    database
        .bulk_remove_media_files(
            &files[..40]
                .iter()
                .map(|file| file.path.clone())
                .collect::<Vec<_>>(),
        )
        .await
        .unwrap();

    database.vacuum().await.unwrap();

    assert_eq!(database.get_stats().await.unwrap().total_files, 10);
    assert!(database
        .get_file_by_path(Path::new("/music/song45.mp3"))
        .await
        .unwrap()
        .is_some());
}

pub async fn backup_and_offline_restore_preserve_a_snapshot<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let extension = B::file_extension();
    let database_path = temp.path().join(format!("active.{extension}"));
    let backup_path = temp.path().join(format!("snapshot.{extension}"));
    let settings = DatabaseSettings::new(database_path.clone(), 4);

    {
        let database = B::open(&settings).await.unwrap();
        database.initialize().await.unwrap();
        database
            .store_media_file(&video("/media/original.mp4", 42))
            .await
            .unwrap();
        database.create_backup(&backup_path).await.unwrap();
    }
    assert!(backup_path.exists(), "backup file was not created");

    {
        let replacement = B::open(&settings).await.unwrap();
        replacement.initialize().await.unwrap();
        replacement
            .store_media_file(&video("/media/replacement.mp4", 7))
            .await
            .unwrap();
    }

    B::restore_backup_file(&backup_path, &database_path)
        .await
        .unwrap();

    let restored = B::open(&settings).await.unwrap();
    restored.initialize().await.unwrap();
    assert!(
        restored
            .get_file_by_path(Path::new("/media/original.mp4"))
            .await
            .unwrap()
            .is_some(),
        "the backup's contents are missing after restore"
    );
    assert!(
        restored
            .get_file_by_path(Path::new("/media/replacement.mp4"))
            .await
            .unwrap()
            .is_none(),
        "writes made after the backup must not survive a restore"
    );
}

pub async fn restoring_a_non_database_leaves_the_original_intact<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let extension = B::file_extension();
    let database_path = temp.path().join(format!("active.{extension}"));
    let settings = DatabaseSettings::new(database_path.clone(), 4);

    {
        let database = B::open(&settings).await.unwrap();
        database.initialize().await.unwrap();
        database
            .store_media_file(&video("/media/keep.mp4", 1))
            .await
            .unwrap();
    }

    let junk = temp.path().join(format!("junk.{extension}"));
    std::fs::write(&junk, b"this is not a database").unwrap();
    assert!(
        B::restore_backup_file(&junk, &database_path).await.is_err(),
        "an unreadable backup must be rejected"
    );

    let survivor = B::open(&settings).await.unwrap();
    survivor.initialize().await.unwrap();
    assert!(
        survivor
            .get_file_by_path(Path::new("/media/keep.mp4"))
            .await
            .unwrap()
            .is_some(),
        "a rejected restore must leave the active database untouched"
    );
}

pub async fn opening_a_non_database_fails_without_deleting_it<B: DatabaseBackend>() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(format!("corrupt.{}", B::file_extension()));
    let original = b"not a database at all";
    std::fs::write(&path, original).unwrap();

    // A backend may reject the file on open or defer validation to first use,
    // but the pair must not succeed: startup relies on one of them failing so
    // the file gets quarantined rather than silently replaced.
    let settings = DatabaseSettings::new(path.clone(), 4);
    let rejected = match B::open(&settings).await {
        Err(_) => true,
        Ok(database) => database.initialize().await.is_err(),
    };
    assert!(rejected, "a file that is not a database was accepted");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "an unusable file must be left for the caller to quarantine, not overwritten"
    );
}

/// Generate the conformance suite as named tests for one backend.
macro_rules! backend_conformance_tests {
    ($backend:ty) => {
        macro_rules! conformance_case {
            ($name:ident) => {
                #[tokio::test]
                async fn $name() {
                    $crate::database::conformance::$name::<$backend>().await;
                }
            };
        }

        conformance_case!(crud_round_trip);
        conformance_case!(storing_the_same_path_twice_updates_in_place);
        conformance_case!(bulk_store_and_stats);
        conformance_case!(bulk_update_rewrites_metadata);
        conformance_case!(fingerprints_match_stored_records);
        conformance_case!(lookup_by_multiple_paths);
        conformance_case!(directory_listing_filters_by_mime_family);
        conformance_case!(directories_are_visible_from_deep_descendants);
        conformance_case!(subtree_removal_respects_component_boundaries);
        conformance_case!(path_prefix_query_is_bounded_by_the_prefix);
        conformance_case!(cleanup_drops_records_missing_from_disk);
        conformance_case!(music_categories_group_and_count);
        conformance_case!(removing_the_last_tagged_record_empties_its_categories);
        conformance_case!(directory_visitor_orders_and_pages);
        conformance_case!(file_visitor_pages_a_directory_in_natural_order);
        conformance_case!(album_tracks_order_by_track_number);
        conformance_case!(album_tracks_order_by_disc_then_track);
        conformance_case!(music_categories_narrow_to_their_filter);
        conformance_case!(radio_records_do_not_become_music_categories);
        conformance_case!(playlist_entry_counts_are_returned_together);
        conformance_case!(filtered_query_searches_text_and_pages_by_cursor);
        conformance_case!(read_session_finds_records_by_id_and_path);
        conformance_case!(playlist_lifecycle);
        conformance_case!(playlist_batch_add_and_reorder);
        conformance_case!(deleting_a_file_removes_it_from_playlists);
        conformance_case!(source_derived_content_is_replaced_and_removed);
        conformance_case!(root_availability_round_trip);
        conformance_case!(secrets_round_trip);
        conformance_case!(records_survive_reopening);
        conformance_case!(health_check_reports_a_clean_database);
        conformance_case!(vacuum_leaves_the_database_usable);
        conformance_case!(backup_and_offline_restore_preserve_a_snapshot);
        conformance_case!(restoring_a_non_database_leaves_the_original_intact);
        conformance_case!(opening_a_non_database_fails_without_deleting_it);
    };
}

pub(crate) use backend_conformance_tests;

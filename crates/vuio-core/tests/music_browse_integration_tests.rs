//! Walking the music tree the way a control point does.
//!
//! These drive the real SOAP endpoint through the real router, so what they
//! assert is what a renderer receives: the containers, their UPnP classes, and
//! the tracks at the leaves.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::{tempdir, TempDir};
use tower::ServiceExt;

use vuio_core::config::AppConfig;
use vuio_core::database::sqlite::SqliteDatabase;
use vuio_core::database::{DatabaseManager, MediaFile, MediaRepository, PlaylistRepository};
use vuio_core::platform::filesystem::create_platform_filesystem_manager;
use vuio_core::platform::PlatformInfo;
use vuio_core::state::AppState;
use vuio_core::web::create_router;
use vuio_core::web::diagnostics::WebHandlerMetrics;

async fn make_test_state() -> (TempDir, AppState) {
    let temp = tempdir().unwrap();
    let database = Arc::new(
        SqliteDatabase::new(temp.path().join("music-browse.db"))
            .await
            .unwrap(),
    );
    database.initialize().await.unwrap();
    let config = Arc::new(AppConfig::default());

    let state = AppState {
        media_directories: Arc::new(tokio::sync::RwLock::new(config.media.directories.clone())),
        unavailable_roots: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
        config: config.clone(),
        config_source: Arc::new(vuio_core::state::ConfigSource::default()),
        http_binding: Arc::new(vuio_core::state::HttpBinding::new(8080)),
        live_config: Arc::new(vuio_core::state::LiveConfig::new(config)),
        database,
        auth: Arc::new(vuio_core::web::auth::AuthState::testing()),
        platform_info: Arc::new(PlatformInfo::detect().await.unwrap()),
        filesystem_manager: Arc::from(create_platform_filesystem_manager()),
        content_update_id: Arc::new(std::sync::atomic::AtomicU32::new(1)),
        web_metrics: Arc::new(WebHandlerMetrics::new()),
        runtime_diagnostics: Arc::new(
            vuio_core::platform::diagnostics::SystemDiagnosticsSampler::new(),
        ),
        lifecycle_stats: Arc::new(vuio_core::lifecycle::ApplicationStats::new()),
        bookmarks: Arc::new(tokio::sync::Mutex::new(
            vuio_core::runtime_state::BookmarkRegistry::new(
                vuio_core::runtime_state::BOOKMARK_MAX_ENTRIES,
            ),
        )),
        log_file_path: temp.path().join("vuio.log"),
        browse_cache: Arc::new(tokio::sync::Mutex::new(
            vuio_core::runtime_state::BrowseResponseCache::new(),
        )),
        mcp_clients: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_monitors: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_casts: Arc::new(tokio::sync::Mutex::new(
            vuio_core::runtime_state::ActiveCastRegistry::new(),
        )),
        discovered_tvs: Arc::new(vuio_core::runtime_state::RendererCache::new()),
        upnp_subscriptions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        cancellation: tokio_util::sync::CancellationToken::new(),
        background_tasks: tokio_util::task::TaskTracker::new(),
    };

    (temp, state)
}

fn track(path: &str, artist: &str, album: &str, genre: &str, track_number: u32) -> MediaFile {
    let mut file = MediaFile::new(PathBuf::from(path), 4096, "audio/mpeg".to_string());
    file.artist = Some(artist.to_string());
    file.album_artist = Some(artist.to_string());
    file.album = Some(album.to_string());
    file.genre = Some(genre.to_string());
    file.year = Some(1984);
    file.track_number = Some(track_number);
    file.title = Some(format!("{album} {track_number}"));
    file.stream.sample_rate = Some(44_100);
    file.stream.channels = Some(2);
    file.stream.bits_per_sample = Some(16);
    file.stream.bit_rate = Some(320_000);
    file
}

/// A library with two artists, three albums, and an artist whose name contains
/// the path separator.
async fn seed(state: &AppState) {
    let records = vec![
        track(
            "/music/m/rtl1.mp3",
            "Metallica",
            "Ride the Lightning",
            "Metal",
            1,
        ),
        track(
            "/music/m/rtl2.mp3",
            "Metallica",
            "Ride the Lightning",
            "Metal",
            2,
        ),
        track("/music/m/load1.mp3", "Metallica", "Load", "Rock", 1),
        // A slash inside a tag value is the case that nesting breaks without
        // encoded object ids.
        track("/music/acdc/bib1.mp3", "AC/DC", "Back in Black", "Rock", 1),
    ];
    state
        .database
        .bulk_store_media_files(&records)
        .await
        .unwrap();
}

async fn browse(state: &AppState, object_id: &str) -> String {
    browse_page(state, object_id, "BrowseDirectChildren", 0, 0).await
}

async fn browse_with_flag(state: &AppState, object_id: &str, flag: &str) -> String {
    browse_page(state, object_id, flag, 0, 0).await
}

async fn browse_page(
    state: &AppState,
    object_id: &str,
    flag: &str,
    starting_index: u32,
    requested_count: u32,
) -> String {
    let body = format!(
        r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
  <s:Body>
    <u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
      <ObjectID>{object_id}</ObjectID>
      <BrowseFlag>{flag}</BrowseFlag>
      <Filter>*</Filter>
      <StartingIndex>{starting_index}</StartingIndex>
      <RequestedCount>{requested_count}</RequestedCount>
      <SortCriteria></SortCriteria>
    </u:Browse>
  </s:Body>
</s:Envelope>"#
    );

    let response = create_router(state.clone())
        .oneshot(
            Request::post("/control/ContentDirectory")
                .header("content-type", "text/xml")
                .header(
                    "soapaction",
                    "\"urn:schemas-upnp-org:service:ContentDirectory:1#Browse\"",
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "browsing {object_id} failed"
    );
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// Container ids in a response, in order.
///
/// The DIDL arrives XML-escaped inside `<Result>`, so the markup this reads is
/// `&lt;container id=&quot;…`.
fn container_ids(response: &str) -> Vec<String> {
    response
        .split("&lt;container id=&quot;")
        .skip(1)
        .filter_map(|rest| rest.split("&quot;").next())
        .map(|id| id.replace("&amp;", "&"))
        .collect()
}

fn container_titles(response: &str) -> Vec<String> {
    response
        .split("&lt;dc:title&gt;")
        .skip(1)
        .filter_map(|rest| rest.split("&lt;/dc:title&gt;").next())
        .map(|title| title.to_string())
        .collect()
}

fn item_count(response: &str) -> usize {
    response.matches("&lt;item id=&quot;").count()
}

#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (integration harness)"
)]
async fn music_root_offers_every_category() {
    let (_temp, state) = make_test_state().await;
    seed(&state).await;

    let response = browse(&state, "audio").await;
    let ids = container_ids(&response);
    assert_eq!(
        ids,
        [
            "audio/!all",
            "audio/artists",
            "audio/albumartists",
            "audio/albums",
            "audio/genres",
            "audio/years",
            "audio/playlists",
            "audio/folders",
        ],
        "the Music container must offer the whole categorization"
    );

    let titles = container_titles(&response);
    assert!(titles.contains(&"All Music".to_string()));
    assert!(titles.contains(&"Album Artists".to_string()));
}

/// The shape the issue asked for: artist, then album, then tracks.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (integration harness)"
)]
async fn artists_descend_through_albums_to_tracks() {
    let (_temp, state) = make_test_state().await;
    seed(&state).await;

    let artists = browse(&state, "audio/artists").await;
    assert_eq!(
        container_ids(&artists),
        ["audio/artists/AC%2FDC", "audio/artists/Metallica"]
    );
    assert!(
        artists.contains("object.container.person.musicArtist"),
        "an artist must announce itself as an artist, not a folder"
    );
    // The name is the artist, with no count spliced into it.
    assert_eq!(
        container_titles(&artists),
        ["AC/DC", "Metallica"],
        "counts belong in childCount, not the title"
    );

    let metallica = browse(&state, "audio/artists/Metallica").await;
    assert_eq!(
        container_ids(&metallica),
        [
            "audio/artists/Metallica/!all",
            "audio/artists/Metallica/Load",
            "audio/artists/Metallica/Ride the Lightning",
        ],
        "an artist holds their albums, with All Songs first"
    );
    assert!(metallica.contains("object.container.album.musicAlbum"));

    let album = browse(&state, "audio/artists/Metallica/Ride the Lightning").await;
    assert_eq!(item_count(&album), 2);
    assert!(album.contains("object.item.audioItem.musicTrack"));
    assert!(album.contains("/media/"), "tracks must carry a res URL");

    // All Songs reaches every track by the artist regardless of album.
    let all_songs = browse(&state, "audio/artists/Metallica/!all").await;
    assert_eq!(item_count(&all_songs), 3);
}

/// The slash case, end to end: an artist named "AC/DC" must be browsable.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (integration harness)"
)]
async fn an_artist_whose_name_contains_a_slash_is_browsable() {
    let (_temp, state) = make_test_state().await;
    seed(&state).await;

    let artist = browse(&state, "audio/artists/AC%2FDC").await;
    assert_eq!(
        container_ids(&artist),
        [
            "audio/artists/AC%2FDC/!all",
            "audio/artists/AC%2FDC/Back in Black",
        ]
    );
    assert_eq!(container_titles(&artist)[1], "Back in Black");

    let album = browse(&state, "audio/artists/AC%2FDC/Back in Black").await;
    assert_eq!(item_count(&album), 1);
}

#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (integration harness)"
)]
async fn genres_descend_through_artists_and_albums() {
    let (_temp, state) = make_test_state().await;
    seed(&state).await;

    let genres = browse(&state, "audio/genres").await;
    assert_eq!(
        container_ids(&genres),
        ["audio/genres/Metal", "audio/genres/Rock"]
    );
    assert!(genres.contains("object.container.genre.musicGenre"));

    let rock = browse(&state, "audio/genres/Rock").await;
    assert_eq!(
        container_ids(&rock),
        [
            "audio/genres/Rock/!all",
            "audio/genres/Rock/AC%2FDC",
            "audio/genres/Rock/Metallica",
        ],
        "a genre holds its artists"
    );

    let metallica_rock = browse(&state, "audio/genres/Rock/Metallica").await;
    assert_eq!(
        container_ids(&metallica_rock),
        [
            "audio/genres/Rock/Metallica/!all",
            "audio/genres/Rock/Metallica/Load",
        ],
        "only the albums this artist has in this genre"
    );

    let tracks = browse(&state, "audio/genres/Rock/Metallica/Load").await;
    assert_eq!(item_count(&tracks), 1);
}

/// Two artists with an identically titled album must not merge.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (integration harness)"
)]
async fn same_titled_albums_stay_separate_under_their_artists() {
    let (_temp, state) = make_test_state().await;
    state
        .database
        .bulk_store_media_files(&[
            track("/music/a/hits.mp3", "Artist A", "Greatest Hits", "Pop", 1),
            track("/music/b/hits1.mp3", "Artist B", "Greatest Hits", "Pop", 1),
            track("/music/b/hits2.mp3", "Artist B", "Greatest Hits", "Pop", 2),
        ])
        .await
        .unwrap();

    let a = browse(&state, "audio/artists/Artist A/Greatest Hits").await;
    assert_eq!(item_count(&a), 1);

    let b = browse(&state, "audio/artists/Artist B/Greatest Hits").await;
    assert_eq!(item_count(&b), 2);

    // The flat Albums view still shows one container for the shared title,
    // holding every track that carries it. That is the flat view's meaning.
    let flat = browse(&state, "audio/albums/Greatest Hits").await;
    assert_eq!(item_count(&flat), 3);
}

#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (integration harness)"
)]
async fn playlists_are_browsable_and_play_in_stored_order() {
    let (_temp, state) = make_test_state().await;
    let ids = state
        .database
        .bulk_store_media_files(&[
            track("/music/p/one.mp3", "Artist", "Album", "Pop", 1),
            track("/music/p/two.mp3", "Artist", "Album", "Pop", 2),
            track("/music/p/three.mp3", "Artist", "Album", "Pop", 3),
        ])
        .await
        .unwrap();

    let playlist = state
        .database
        .create_playlist("Roadtrip", None)
        .await
        .unwrap();
    // Deliberately not track order: a playlist plays in the order it stores.
    state
        .database
        .batch_add_to_playlist(playlist, &[(ids[2], 0), (ids[0], 1), (ids[1], 2)])
        .await
        .unwrap();

    let list = browse(&state, "audio/playlists").await;
    assert_eq!(
        container_ids(&list),
        [format!("audio/playlists/{playlist}")]
    );
    assert_eq!(container_titles(&list), ["Roadtrip"]);
    assert!(
        list.contains("object.container.playlistContainer"),
        "a playlist must announce itself as a playlist so renderers offer play-all"
    );
    assert!(
        list.contains("childCount=&quot;3&quot;"),
        "a playlist reports how many tracks it holds"
    );

    let tracks = browse(&state, &format!("audio/playlists/{playlist}")).await;
    assert_eq!(item_count(&tracks), 3);
    // Stored order, not track-number order: the entries were added 3, 1, 2.
    assert_eq!(
        container_titles(&tracks),
        ["Album 3", "Album 1", "Album 2"],
        "playlist entries must render in stored position order"
    );
}

/// Renderers use these to decide whether they can play a track at all.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (integration harness)"
)]
async fn tracks_advertise_their_stream_properties() {
    let (_temp, state) = make_test_state().await;
    seed(&state).await;

    let album = browse(&state, "audio/albums/Load").await;
    assert!(album.contains("sampleFrequency=&quot;44100&quot;"));
    assert!(album.contains("nrAudioChannels=&quot;2&quot;"));
    assert!(album.contains("bitsPerSample=&quot;16&quot;"));
    // DLNA's res@bitrate is bytes per second, so 320 kbit/s is 40000.
    assert!(
        album.contains("bitrate=&quot;40000&quot;"),
        "res@bitrate is bytes per second, not bits"
    );
}

/// Samsung probes a container before opening it and reads childCount to decide
/// whether it is worth showing.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (integration harness)"
)]
async fn browse_metadata_describes_music_containers() {
    let (_temp, state) = make_test_state().await;
    seed(&state).await;

    let artists = browse_with_flag(&state, "audio/artists", "BrowseMetadata").await;
    assert!(artists.contains("&lt;dc:title&gt;Artists&lt;/dc:title&gt;"));
    assert!(
        artists.contains("childCount=&quot;2&quot;"),
        "two artists were indexed"
    );
    assert!(artists.contains("parentID=&quot;audio&quot;"));

    let artist = browse_with_flag(&state, "audio/artists/Metallica", "BrowseMetadata").await;
    assert!(artist.contains("&lt;dc:title&gt;Metallica&lt;/dc:title&gt;"));
    assert!(artist.contains("object.container.person.musicArtist"));
    assert!(artist.contains("parentID=&quot;audio/artists&quot;"));
    assert!(
        !artist.contains("childCount=&quot;0&quot;"),
        "a container that holds albums must never report itself empty"
    );
}

/// A control point that pages must still learn how much there is to page
/// through, or it stops after the first response.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (integration harness)"
)]
async fn paging_reports_the_full_total_at_every_offset() {
    let (_temp, state) = make_test_state().await;
    state
        .database
        .bulk_store_media_files(&[
            track("/music/p/a.mp3", "Artist A", "Album", "Pop", 1),
            track("/music/p/b.mp3", "Artist B", "Album", "Pop", 1),
            track("/music/p/c.mp3", "Artist C", "Album", "Pop", 1),
            track("/music/p/d.mp3", "Artist D", "Album", "Pop", 1),
        ])
        .await
        .unwrap();

    let field = |response: &str, name: &str| -> usize {
        response
            .split(&format!("<{name}>"))
            .nth(1)
            .and_then(|rest| rest.split(&format!("</{name}>")).next())
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(|| panic!("no <{name}> in response"))
    };

    // Containers: four artists, taken two at a time.
    let first = browse_page(&state, "audio/artists", "BrowseDirectChildren", 0, 2).await;
    assert_eq!(container_ids(&first).len(), 2);
    assert_eq!(field(&first, "NumberReturned"), 2);
    assert_eq!(field(&first, "TotalMatches"), 4);

    let second = browse_page(&state, "audio/artists", "BrowseDirectChildren", 2, 2).await;
    assert_eq!(container_titles(&second), ["Artist C", "Artist D"]);
    assert_eq!(field(&second, "TotalMatches"), 4);

    // Past the end is an empty page, not an error or a wrapped one.
    let past_end = browse_page(&state, "audio/artists", "BrowseDirectChildren", 10, 2).await;
    assert_eq!(container_ids(&past_end).len(), 0);
    assert_eq!(field(&past_end, "NumberReturned"), 0);
    assert_eq!(field(&past_end, "TotalMatches"), 4);

    // Items page the same way, through the read session rather than a slice.
    let tracks = browse_page(&state, "audio/albums/Album", "BrowseDirectChildren", 1, 2).await;
    assert_eq!(item_count(&tracks), 2);
    assert_eq!(field(&tracks, "TotalMatches"), 4);
}

/// A container announcing more children than it returns sends a control point
/// looking for items that are not there.
///
/// The count of an artist is its albums, not its tracks — those are different
/// numbers, and the tracks number is the larger and wrong one.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (integration harness)"
)]
async fn child_counts_match_the_children_actually_returned() {
    let (_temp, state) = make_test_state().await;
    seed(&state).await;
    // Push Metallica to four tracks across two albums, so the track count and
    // the child count are different numbers and the assertion can tell them
    // apart.
    state
        .database
        .bulk_store_media_files(&[track(
            "/music/m/rtl3.mp3",
            "Metallica",
            "Ride the Lightning",
            "Metal",
            3,
        )])
        .await
        .unwrap();

    let child_count = |response: &str, container_id: &str| -> usize {
        // Each container renders as `id="…" parentID="…" … childCount="N"`.
        let anchor = format!("id=&quot;{container_id}&quot;");
        let start = response
            .find(&anchor)
            .unwrap_or_else(|| panic!("{container_id} not in response"));
        response[start..]
            .split("childCount=&quot;")
            .nth(1)
            .and_then(|rest| rest.split("&quot;").next())
            .and_then(|value| value.parse().ok())
            .expect("no childCount")
    };

    // Metallica: 4 tracks but 2 albums, so 3 children (All Songs + 2 albums).
    let artists = browse(&state, "audio/artists").await;
    let announced = child_count(&artists, "audio/artists/Metallica");
    let actual = container_ids(&browse(&state, "audio/artists/Metallica").await).len();
    assert_eq!(
        announced, actual,
        "artist announced {announced} children but returned {actual}"
    );
    assert_eq!(actual, 3);

    // Rock: 2 artists, so 3 children (All Songs + 2 artists).
    let genres = browse(&state, "audio/genres").await;
    let announced = child_count(&genres, "audio/genres/Rock");
    let actual = container_ids(&browse(&state, "audio/genres/Rock").await).len();
    assert_eq!(
        announced, actual,
        "genre announced {announced} children but returned {actual}"
    );

    // An album's children really are tracks, so there the record count is right.
    let metallica = browse(&state, "audio/artists/Metallica").await;
    let announced = child_count(&metallica, "audio/artists/Metallica/Ride the Lightning");
    let actual = item_count(&browse(&state, "audio/artists/Metallica/Ride the Lightning").await);
    assert_eq!(announced, actual);
    assert_eq!(actual, 3);

    // BrowseMetadata must agree with the parent listing about the same object.
    let probed = browse_with_flag(&state, "audio/artists/Metallica", "BrowseMetadata").await;
    assert_eq!(child_count(&probed, "audio/artists/Metallica"), 3);
}

/// A playlist is named the same whether it is listed or probed.
#[tokio::test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "SIGSEGV in FreeBSD CI QEMU guest (integration harness)"
)]
async fn browse_metadata_names_a_playlist_the_way_its_listing_did() {
    let (_temp, state) = make_test_state().await;
    let ids = state
        .database
        .bulk_store_media_files(&[track("/music/p/one.mp3", "Artist", "Album", "Pop", 1)])
        .await
        .unwrap();
    let playlist = state
        .database
        .create_playlist("Roadtrip", None)
        .await
        .unwrap();
    state
        .database
        .batch_add_to_playlist(playlist, &[(ids[0], 0)])
        .await
        .unwrap();

    let listed = browse(&state, "audio/playlists").await;
    assert_eq!(container_titles(&listed), ["Roadtrip"]);

    let probed = browse_with_flag(
        &state,
        &format!("audio/playlists/{playlist}"),
        "BrowseMetadata",
    )
    .await;
    assert_eq!(
        container_titles(&probed),
        ["Roadtrip"],
        "a probe must not rename the container its listing already named"
    );
}

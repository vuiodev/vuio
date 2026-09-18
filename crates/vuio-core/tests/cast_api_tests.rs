//! What casting does to the library, which should be nothing.
//!
//! `POST /api/cast/playlist` used to build a real playlist to do its work —
//! "Web Cast - <folder>", written to the database, handed to the renderer, and
//! then deleted only if the cast had failed. On success it stayed, so every
//! cast left another one behind: visible to every DLNA client under
//! Music → Playlists, duplicated on each repeat, and costing a ContentDirectory
//! revision coming and going. The API reference has always called it temporary.

#![cfg(feature = "casting")]

mod common;

use std::sync::atomic::Ordering;
use std::sync::Arc;
use vuio_core::database::{MediaFile, MediaRepository, PlaylistRepository};

/// A cast that cannot proceed must leave the library exactly as it found it.
///
/// An image is the request that gets refused before anything is discovered,
/// which is what keeps this test off the network: it asserts on the writes that
/// happened on the way to the refusal, and those are the writes that used to
/// happen on every call. The old code created the playlist and announced the
/// change before it ever looked at what it had been given, so the revision below
/// moved twice for a cast that never started.
#[tokio::test]
async fn a_cast_writes_nothing_to_the_library() {
    let temp = tempfile::tempdir().expect("temp dir");
    let root = temp.path().join("library");
    std::fs::create_dir_all(&root).expect("library dir");
    let photo = root.join("holiday.jpg");
    std::fs::write(&photo, b"not really a photo").expect("write");

    let state = common::state_over(temp.path(), &root).await;
    let id = state
        .database
        .store_media_file(&MediaFile::new(photo, 18, "image/jpeg".to_string()))
        .await
        .expect("store");

    let revision_before = state.content_update_id.load(Ordering::SeqCst);
    let response = vuio_core::web::casting::api_cast_playlist(
        axum::extract::State(state.clone()),
        axum::Json(vuio_core::web::casting::ApiCastPlaylistRequest {
            renderer_id: "chromecast:nothing-here".to_string(),
            folder_name: "Photos".to_string(),
            file_ids: vec![id],
        }),
    )
    .await;

    let status = axum::response::IntoResponse::into_response(response).status();
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert!(
        state
            .database
            .get_playlists()
            .await
            .expect("playlists")
            .is_empty(),
        "casting must not leave a playlist behind"
    );
    assert_eq!(
        state.content_update_id.load(Ordering::SeqCst),
        revision_before,
        "casting is not a change to the library, and must not announce one"
    );

    // Keep the tracker honest: `state_over` builds one and nothing here spawns
    // into it, but a cast that got further would.
    let _: Arc<_> = state.database.clone();
}

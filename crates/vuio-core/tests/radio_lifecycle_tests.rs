//! Starting and stopping a station is a sequence of awaits, so two requests
//! that arrive together can interleave inside it. What comes out has to be one
//! station on the air and nothing left running behind it.

mod common;

use std::sync::Arc;
use vuio_core::database::{
    BroadcastMode, MediaFile, MediaRepository, RadioStationInput, RadioStationRepository,
};

/// Two simultaneous starts of one station must leave exactly one instance live
/// and take the other off the air.
///
/// Both used to get past the opening `stop` together and build a `Playout`
/// each. The second `insert` dropped the first from the map, but the playout
/// task holds an `Arc<Station>` of its own, so it kept reading files and
/// writing cursors with nothing left to cancel it — and a later `stop` reached
/// only the instance the map had kept.
#[tokio::test]
async fn two_simultaneous_starts_leave_one_station_on_the_air() {
    let temp = tempfile::tempdir().expect("temp dir");
    let media = temp.path().join("media");
    std::fs::create_dir_all(&media).expect("media dir");
    // The database stores canonical paths; the station's folder has to match,
    // and on macOS /var is a symlink to /private/var.
    let media = std::fs::canonicalize(&media).expect("canonical media dir");

    // A row is all `build_queue` reads; the playout task discovers the file is
    // not really audio on its own time, which is not what this test is about.
    let track = media.join("track.mp3");
    std::fs::write(&track, b"not really an mp3").expect("write track");

    let state = common::state_over(temp.path(), &media).await;
    state
        .database
        .store_media_file(&MediaFile::new(track, 17, "audio/mpeg".to_owned()))
        .await
        .expect("index track");

    let row = state
        .database
        .create_radio_station(&RadioStationInput {
            name: "test".to_owned(),
            genre: String::new(),
            folders: vec![media.to_string_lossy().into_owned()],
            mode: BroadcastMode::Loop,
        })
        .await
        .expect("create station");

    let radio = Arc::clone(&state.radio);
    let (first, second) = tokio::join!(radio.start(&state, &row), radio.start(&state, &row));
    let first = first.expect("first start");
    let second = second.expect("second start");

    assert_eq!(
        state.radio.live_count().await,
        1,
        "one station, one live instance"
    );
    assert_eq!(
        usize::from(!first.is_off_air()) + usize::from(!second.is_off_air()),
        1,
        "the instance the map did not keep must have been taken off the air"
    );

    // And stopping reaches whichever one survived.
    state.radio.stop(row.id).await;
    assert!(first.is_off_air() && second.is_off_air());
    assert_eq!(state.radio.live_count().await, 0);

    state.cancellation.cancel();
    state.background_tasks.close();
    state.background_tasks.wait().await;
}

/// A stop that arrives while a start is in flight must not be overtaken by it:
/// the station is either never installed or installed and then removed, but it
/// is never left broadcasting after a stop has been asked for and completed.
#[tokio::test]
async fn a_start_and_a_stop_do_not_interleave() {
    let temp = tempfile::tempdir().expect("temp dir");
    let media = temp.path().join("media");
    std::fs::create_dir_all(&media).expect("media dir");
    // The database stores canonical paths; the station's folder has to match,
    // and on macOS /var is a symlink to /private/var.
    let media = std::fs::canonicalize(&media).expect("canonical media dir");
    let track = media.join("track.mp3");
    std::fs::write(&track, b"not really an mp3").expect("write track");

    let state = common::state_over(temp.path(), &media).await;
    state
        .database
        .store_media_file(&MediaFile::new(track, 17, "audio/mpeg".to_owned()))
        .await
        .expect("index track");
    let row = state
        .database
        .create_radio_station(&RadioStationInput {
            name: "test".to_owned(),
            genre: String::new(),
            folders: vec![media.to_string_lossy().into_owned()],
            mode: BroadcastMode::Loop,
        })
        .await
        .expect("create station");

    let radio = Arc::clone(&state.radio);
    let (started, ()) = tokio::join!(radio.start(&state, &row), radio.stop(row.id));
    let started = started.expect("start");

    // Whichever order the lock granted, the map and the instance agree.
    assert_eq!(
        state.radio.live_count().await == 1,
        !started.is_off_air(),
        "a station is live exactly when the map holds it"
    );

    state.cancellation.cancel();
    state.background_tasks.close();
    state.background_tasks.wait().await;
}

/// A looping station whose files have gone — an unmounted share, a folder
/// emptied under it — must wait before trying its queue again.
///
/// Every track failed to open, the queue was rebuilt from rows that are all
/// still there, and the loop went straight round again: nothing anywhere on
/// that path waits. It span a core and wrote a warning per track per turn until
/// someone stopped it. The station stays on the air, because a share that comes
/// back should start playing again on its own.
#[tokio::test]
async fn a_station_whose_files_have_gone_waits_instead_of_spinning() {
    let temp = tempfile::tempdir().expect("temp dir");
    let media = temp.path().join("media");
    std::fs::create_dir_all(&media).expect("media dir");
    let media = std::fs::canonicalize(&media).expect("canonical media dir");

    // Indexed, and then removed from disk: exactly the state an unmounted share
    // leaves behind, and the one `build_queue` cannot tell from a healthy queue.
    let mut tracks = Vec::new();
    for index in 0..4 {
        let track = media.join(format!("track{index}.mp3"));
        std::fs::write(&track, b"not really an mp3").expect("write track");
        tracks.push(track);
    }

    let state = common::state_over(temp.path(), &media).await;
    for track in &tracks {
        state
            .database
            .store_media_file(&MediaFile::new(track.clone(), 17, "audio/mpeg".to_owned()))
            .await
            .expect("index track");
    }
    for track in &tracks {
        std::fs::remove_file(track).expect("unmount");
    }

    let row = state
        .database
        .create_radio_station(&RadioStationInput {
            name: "gone".to_owned(),
            genre: String::new(),
            folders: vec![media.to_string_lossy().into_owned()],
            mode: BroadcastMode::Loop,
        })
        .await
        .expect("create station");

    let station = state.radio.start(&state, &row).await.expect("start");

    // Long enough for a spinning task to have gone round hundreds of times, and
    // far short of the five seconds the first wait lasts.
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    assert_eq!(
        station.silent_passes(),
        1,
        "one pass produced nothing and the task is waiting, not looping"
    );
    assert!(
        !station.is_off_air(),
        "a share that comes back should find its station still there"
    );

    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    assert_eq!(station.silent_passes(), 1, "still waiting, still one pass");

    state.radio.stop(row.id).await;
    state.cancellation.cancel();
    state.background_tasks.close();
    state.background_tasks.wait().await;
}

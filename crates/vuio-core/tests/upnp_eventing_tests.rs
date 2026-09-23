//! ContentDirectory eventing against a real subscriber.
//!
//! The throttle and ownership rules are unit-tested beside the code in
//! `src/web/eventing.rs`. What those cannot show is what reaches the television,
//! so these stand up a callback listener and read the NOTIFY bodies it receives.

mod common;

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A UPnP event subscriber on loopback that records the `SystemUpdateID` of every
/// NOTIFY it is sent, and answers each one 200.
async fn subscriber() -> (u16, Arc<Mutex<Vec<u32>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let recorded = recorded.clone();
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut buffer = [0u8; 4096];
                while !String::from_utf8_lossy(&request).contains("</e:propertyset>") {
                    match stream.read(&mut buffer).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => request.extend_from_slice(&buffer[..read]),
                    }
                }
                let request = String::from_utf8_lossy(&request).into_owned();
                if let Some(revision) = request
                    .split("<SystemUpdateID>")
                    .nth(1)
                    .and_then(|rest| rest.split('<').next())
                    .and_then(|value| value.parse().ok())
                {
                    recorded.lock().unwrap().push(revision);
                }
                let _ = stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await;
            });
        }
    });
    (port, seen)
}

/// The last change of a burst has to reach the subscriber.
///
/// The notification worker used to read the revision before it took the lock the
/// pending flags are claimed under. A change published in that gap — revision
/// bumped, then the subscription marked owed under the lock — had its mark consumed
/// by a notification carrying the revision before it, and nothing was owed after
/// that: the television held the stale library until some unrelated change came
/// along, which is the bug the pending flag was introduced to fix. The gap is only
/// as wide as the wait for the lock, so this holds the lock across it to make the
/// interleaving certain rather than likely.
#[tokio::test]
async fn a_change_published_while_the_worker_waits_for_the_lock_is_announced() {
    let (port, seen) = subscriber().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("library");
    std::fs::create_dir_all(&root).unwrap();
    let state = common::state_over(temp.path(), &root).await;
    state.content_update_id.store(5, Ordering::SeqCst);

    let worker = uuid::Uuid::new_v4();
    let now = Instant::now();
    state.upnp_subscriptions.lock().await.insert(
        "uuid:television".to_owned(),
        vuio_core::state::UpnpSubscription {
            callback_url: format!("http://127.0.0.1:{port}/events"),
            peer: "127.0.0.1".parse().unwrap(),
            generation: uuid::Uuid::new_v4(),
            expires_at: now + Duration::from_secs(1800),
            next_sequence: 1,
            consecutive_failures: 0,
            // Outside the throttle window, so nothing waits on it.
            last_notification_at: now.checked_sub(Duration::from_secs(10)).unwrap_or(now),
            // Revision 5 is owed, and this worker is the one delivering it.
            pending_notification: true,
            notification_worker: Some(worker),
        },
    );

    // Someone else holds the lock as the worker starts, so it has to wait for it.
    let mut subscriptions = state.upnp_subscriptions.lock().await;
    let delivery = tokio::spawn({
        let state = state.clone();
        async move { vuio_core::web::eventing::notify_content_change(&state, worker).await }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Revision 6, published the way `publish_content_change` does it: the revision
    // first, then the mark under the lock.
    state.content_update_id.fetch_add(1, Ordering::SeqCst);
    subscriptions
        .get_mut("uuid:television")
        .unwrap()
        .pending_notification = true;
    drop(subscriptions);

    tokio::time::timeout(Duration::from_secs(5), delivery)
        .await
        .expect("the worker finishes once nothing is owed")
        .unwrap();

    let seen = seen.lock().unwrap().clone();
    assert_eq!(
        seen.last().copied(),
        Some(6),
        "the subscriber must end on the latest revision; it was sent {seen:?}"
    );
}

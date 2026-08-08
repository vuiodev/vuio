//! Request-response correlation for the Cast protocol.
//!
//! The Cast protocol uses `requestId` fields in JSON payloads to correlate
//! responses with requests. This module tracks pending requests and resolves
//! them when matching responses arrive.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::sync::{Mutex, oneshot};

use crate::error::{Error, Result};

/// Tracks in-flight requests and resolves them when responses arrive.
pub struct RequestTracker {
    counter: AtomicU32,
    pending: Mutex<HashMap<u32, oneshot::Sender<serde_json::Value>>>,
    timeout: Duration,
}

impl RequestTracker {
    /// Create a new request tracker with the given default timeout.
    pub fn new(timeout: Duration) -> Self {
        Self { counter: AtomicU32::new(0), pending: Mutex::new(HashMap::new()), timeout }
    }

    /// Allocate a new request ID and register a pending response.
    ///
    /// Returns `(request_id, receiver)` — the caller sends the request with
    /// this ID, then awaits the receiver for the response.
    pub async fn register(&self) -> (u32, oneshot::Receiver<serde_json::Value>) {
        // Wrapping add to avoid overflow panic. Skip 0 since the router
        // treats requestId == 0 as a broadcast (not correlated).
        let mut id = self.counter.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        if id == 0 {
            id = self.counter.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        }
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        (id, rx)
    }

    /// Attempt to resolve a pending request with a response payload.
    ///
    /// Returns `true` if the request ID was found and resolved.
    pub async fn resolve(&self, request_id: u32, payload: serde_json::Value) -> bool {
        if let Some(tx) = self.pending.lock().await.remove(&request_id) {
            let _ = tx.send(payload);
            true
        } else {
            false
        }
    }

    /// Wait for a response to a registered request, with timeout.
    ///
    /// On timeout or cancellation, the pending entry is removed to prevent leaks.
    pub async fn wait_for(
        &self,
        request_id: u32,
        rx: oneshot::Receiver<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let result = tokio::time::timeout(self.timeout, rx).await;
        match result {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(_)) => {
                self.pending.lock().await.remove(&request_id);
                Err(Error::Disconnected)
            }
            Err(_) => {
                self.pending.lock().await.remove(&request_id);
                Err(Error::Timeout(self.timeout))
            }
        }
    }

    /// Clean up all pending requests (called on disconnect).
    pub async fn clear(&self) {
        self.pending.lock().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_register_and_resolve() {
        let tracker = RequestTracker::new(Duration::from_secs(5));
        let (id, rx) = tracker.register().await;
        assert_eq!(id, 1);

        let resolved = tracker.resolve(1, json!({"type": "RECEIVER_STATUS"})).await;
        assert!(resolved);

        let value = rx.await.unwrap();
        assert_eq!(value["type"], "RECEIVER_STATUS");
    }

    #[tokio::test]
    async fn test_unmatched_resolve() {
        let tracker = RequestTracker::new(Duration::from_secs(5));
        let resolved = tracker.resolve(999, json!({})).await;
        assert!(!resolved);
    }

    #[tokio::test]
    async fn test_timeout() {
        let tracker = RequestTracker::new(Duration::from_millis(50));
        let (id, rx) = tracker.register().await;

        let result = tracker.wait_for(id, rx).await;
        assert!(matches!(result, Err(Error::Timeout(_))));
        // Verify the pending entry was cleaned up
        assert!(!tracker.resolve(id, json!({})).await);
    }

    #[tokio::test]
    async fn test_sequential_ids() {
        let tracker = RequestTracker::new(Duration::from_secs(5));
        let (id1, _) = tracker.register().await;
        let (id2, _) = tracker.register().await;
        let (id3, _) = tracker.register().await;
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[tokio::test]
    async fn test_id_wrapping_skips_zero() {
        let tracker = RequestTracker::new(Duration::from_secs(5));
        // Set counter to u32::MAX - 1 so next fetch_add returns MAX-1, +1 = MAX
        tracker.counter.store(u32::MAX - 1, std::sync::atomic::Ordering::Relaxed);
        let (id1, _) = tracker.register().await;
        assert_eq!(id1, u32::MAX);
        // Next: fetch_add(1) wraps to MAX, wrapping_add(1) = 0, skip 0 → fetch_add again → 1
        let (id2, _) = tracker.register().await;
        assert_ne!(id2, 0, "request ID 0 must be skipped");
        assert!(id2 > 0);
    }

    #[tokio::test]
    async fn test_double_resolve_returns_false() {
        let tracker = RequestTracker::new(Duration::from_secs(5));
        let (id, _rx) = tracker.register().await;
        assert!(tracker.resolve(id, json!({"first": true})).await);
        // Second resolve for same ID should return false (already consumed)
        assert!(!tracker.resolve(id, json!({"second": true})).await);
    }

    #[tokio::test]
    async fn test_clear_drops_all_pending() {
        let tracker = RequestTracker::new(Duration::from_secs(5));
        let (id1, _) = tracker.register().await;
        let (id2, _) = tracker.register().await;
        tracker.clear().await;
        // Both should be gone
        assert!(!tracker.resolve(id1, json!({})).await);
        assert!(!tracker.resolve(id2, json!({})).await);
    }

    #[tokio::test]
    async fn test_wait_for_receiver_dropped() {
        let tracker = RequestTracker::new(Duration::from_secs(5));
        let (id, rx) = tracker.register().await;
        // Drop the sender by clearing
        tracker.clear().await;
        let result = tracker.wait_for(id, rx).await;
        assert!(matches!(result, Err(Error::Disconnected)));
    }

    #[tokio::test]
    async fn test_concurrent_register_resolve() {
        use std::sync::Arc;
        let tracker = Arc::new(RequestTracker::new(Duration::from_secs(5)));

        let mut handles = Vec::new();
        for _ in 0..20 {
            let t = tracker.clone();
            handles.push(tokio::spawn(async move {
                let (id, rx) = t.register().await;
                // Resolve from another task
                let t2 = t.clone();
                tokio::spawn(async move {
                    t2.resolve(id, json!({"id": id})).await;
                });
                let val = t.wait_for(id, rx).await.unwrap();
                assert_eq!(val["id"], id);
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
    }
}

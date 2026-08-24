//! Per-server transcoding state: the index cache and the concurrency ceiling.
//!
//! Both exist for the same reason — decoding is the only CPU-bound work this
//! server does, and a shared folder can be opened by every renderer in the house
//! at once.
//!
//! The cache matters more than it looks. A renderer typically issues a `HEAD`,
//! then a `GET`, then one or more range requests as someone scrubs, and building
//! an index re-reads the whole track each time. Holding a handful of indexes
//! turns that into one read per file rather than one per request.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, Semaphore};

use super::AudioPlan;

/// How many indexes to keep. A two-hour AC-3 track indexes to roughly 3 MB, so
/// this is single-digit megabytes for a household's worth of open streams.
const MAX_CACHED_INDEXES: usize = 8;

/// Identifies a cached index. The file's size and modification time are part of
/// the key so replacing a file in place invalidates its index rather than
/// serving byte offsets into a file that no longer has them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndexKey {
    /// Database id of the file.
    pub id: i64,
    /// Size in bytes at the time the index was built.
    pub size: u64,
    /// Modification time, as seconds since the epoch.
    pub modified: i64,
}

/// Shared transcoding state, held by `AppState`.
#[derive(Debug)]
pub struct TranscodeState {
    cache: Mutex<Cache>,
    permits: Arc<Semaphore>,
}

#[derive(Debug, Default)]
struct Cache {
    entries: HashMap<IndexKey, Arc<AudioPlan>>,
    /// Insertion order, oldest first. A plain queue rather than a true LRU: with
    /// a cap of eight the difference is not measurable, and this needs no
    /// bookkeeping on the read path.
    order: Vec<IndexKey>,
}

impl Default for TranscodeState {
    fn default() -> Self {
        Self::new(2)
    }
}

impl TranscodeState {
    /// Build state allowing `max_concurrent` simultaneous transcodes.
    ///
    /// Zero is treated as one. Configuration validation rejects it, but this is
    /// reachable from a `Default` and refusing every request would be a strange
    /// way to express "misconfigured".
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            cache: Mutex::new(Cache::default()),
            permits: Arc::new(Semaphore::new(max_concurrent.max(1))),
        }
    }

    /// Take a transcoding slot, or `None` when all of them are in use.
    ///
    /// Deliberately non-blocking: a renderer that waits in a queue for a slot
    /// looks to its user like a file that will not open, and meanwhile the
    /// streams already playing lose CPU to it.
    pub fn try_acquire(&self) -> Option<tokio::sync::OwnedSemaphorePermit> {
        self.permits.clone().try_acquire_owned().ok()
    }

    /// The plan for `key`, if one was built recently.
    pub async fn cached(&self, key: &IndexKey) -> Option<Arc<AudioPlan>> {
        self.cache.lock().await.entries.get(key).cloned()
    }

    /// Remember `index` under `key`, evicting the oldest entry if full.
    pub async fn remember(&self, key: IndexKey, index: Arc<AudioPlan>) {
        let mut cache = self.cache.lock().await;
        if cache.entries.insert(key, index).is_none() {
            cache.order.push(key);
            while cache.order.len() > MAX_CACHED_INDEXES {
                let oldest = cache.order.remove(0);
                cache.entries.remove(&oldest);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::transcode::TranscodeCodec;

    fn index() -> Arc<AudioPlan> {
        Arc::new(AudioPlan {
            source_path: std::path::PathBuf::from("/dev/null"),
            codec: TranscodeCodec::Ac3,
            channels: 2,
            index: crate::media::transcode::FrameIndex {
                codec: TranscodeCodec::Ac3,
                sample_rate: 48_000,
                frames: Vec::new(),
                total_samples: 0,
            },
        })
    }

    fn key(id: i64) -> IndexKey {
        IndexKey {
            id,
            size: 1,
            modified: 1,
        }
    }

    #[tokio::test]
    async fn an_index_survives_until_evicted_by_newer_ones() {
        let state = TranscodeState::new(1);
        state.remember(key(1), index()).await;
        assert!(state.cached(&key(1)).await.is_some());

        for id in 2..=(MAX_CACHED_INDEXES as i64 + 1) {
            state.remember(key(id), index()).await;
        }
        assert!(state.cached(&key(1)).await.is_none(), "oldest is evicted");
        assert!(state.cached(&key(2)).await.is_some());
    }

    #[tokio::test]
    async fn a_file_replaced_in_place_does_not_reuse_its_index() {
        let state = TranscodeState::new(1);
        state.remember(key(1), index()).await;
        let rewritten = IndexKey {
            id: 1,
            size: 999,
            modified: 2,
        };
        assert!(state.cached(&rewritten).await.is_none());
    }

    #[tokio::test]
    async fn slots_are_handed_out_up_to_the_ceiling_and_then_refused() {
        let state = TranscodeState::new(2);
        let a = state.try_acquire().expect("first slot");
        let _b = state.try_acquire().expect("second slot");
        assert!(state.try_acquire().is_none(), "third is refused, not queued");
        drop(a);
        assert!(state.try_acquire().is_some(), "a finished stream frees its slot");
    }

    #[tokio::test]
    async fn a_zero_ceiling_still_serves_one_rather_than_nothing() {
        let state = TranscodeState::new(0);
        assert!(state.try_acquire().is_some());
    }
}

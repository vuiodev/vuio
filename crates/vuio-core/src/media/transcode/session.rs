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

/// Identifies a cached fMP4 segment.
///
/// Segments are cached where the elementary index is not, and for a different
/// reason: a copy is cheap to redo, but a decode-and-re-encode is not, and
/// seeking or re-buffering asks for the same segment again and again. Keyed on
/// the track as well as the file because a film's renditions are built
/// independently and a browser may be pulling two of them at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SegmentKey {
    /// Database id of the file.
    pub id: i64,
    /// Container track id.
    pub track: u32,
    /// Segment index within the rendition.
    pub seq: u32,
}

/// How many segments to keep, and how much memory they may occupy between them.
///
/// A segment of 1080p video is single-digit megabytes, so a count alone would
/// bound the wrong thing on a large file and the wrong thing on a small one.
const MAX_CACHED_SEGMENTS: usize = 24;
const MAX_CACHED_SEGMENT_BYTES: usize = 48 * 1024 * 1024;

/// Shared transcoding state, held by `AppState`.
#[derive(Debug)]
pub struct TranscodeState {
    cache: Mutex<Cache>,
    segments: Mutex<SegmentCache>,
    permits: Arc<Semaphore>,
}

#[derive(Debug, Default)]
struct SegmentCache {
    entries: HashMap<SegmentKey, bytes::Bytes>,
    order: Vec<SegmentKey>,
    bytes: usize,
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
            segments: Mutex::new(SegmentCache::default()),
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

    /// The bytes of segment `key`, if it was built recently.
    pub async fn cached_segment(&self, key: &SegmentKey) -> Option<bytes::Bytes> {
        self.segments.lock().await.entries.get(key).cloned()
    }

    /// Remember a built segment, evicting oldest-first past either ceiling.
    pub async fn remember_segment(&self, key: SegmentKey, segment: bytes::Bytes) {
        let mut cache = self.segments.lock().await;
        let len = segment.len();
        if cache.entries.insert(key, segment).is_none() {
            cache.order.push(key);
            cache.bytes += len;
        }
        while cache.order.len() > MAX_CACHED_SEGMENTS || cache.bytes > MAX_CACHED_SEGMENT_BYTES {
            let Some(oldest) = cache.order.first().copied() else {
                break;
            };
            cache.order.remove(0);
            if let Some(evicted) = cache.entries.remove(&oldest) {
                cache.bytes = cache.bytes.saturating_sub(evicted.len());
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
            source: crate::media::transcode::PacketSource::Elementary(
                crate::media::transcode::FrameIndex {
                    codec: TranscodeCodec::Ac3,
                    sample_rate: 48_000,
                    frames: Vec::new(),
                    total_samples: 0,
                },
            ),
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

    #[tokio::test]
    async fn segments_are_evicted_once_they_outgrow_their_memory_ceiling() {
        let state = TranscodeState::new(1);
        let key = |seq| SegmentKey {
            id: 1,
            track: 2,
            seq,
        };
        // Four segments of 16 MB: the fourth must push the first out, because
        // the byte ceiling binds long before the entry count does.
        for seq in 0..4 {
            state
                .remember_segment(key(seq), bytes::Bytes::from(vec![0u8; 16 * 1024 * 1024]))
                .await;
        }
        assert!(state.cached_segment(&key(0)).await.is_none());
        assert!(state.cached_segment(&key(3)).await.is_some());
    }

    #[tokio::test]
    async fn a_segment_is_found_again_under_the_key_that_stored_it() {
        let state = TranscodeState::new(1);
        let key = SegmentKey {
            id: 7,
            track: 2,
            seq: 3,
        };
        state
            .remember_segment(key, bytes::Bytes::from_static(b"segment"))
            .await;
        assert_eq!(
            state.cached_segment(&key).await.as_deref(),
            Some(&b"segment"[..])
        );
        // A different rendition of the same file is a different segment.
        assert!(state
            .cached_segment(&SegmentKey { track: 3, ..key })
            .await
            .is_none());
    }
}

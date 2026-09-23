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
#[cfg(all(feature = "transcode-aac", feature = "casting"))]
use super::TrackRates;

/// How many indexes to keep. A two-hour AC-3 track indexes to roughly 3 MB.
///
/// An index earns its place across the handful of requests a renderer makes
/// while it opens and scrubs one file, so the count only has to cover the files
/// being opened at the same time. The byte ceiling also covers long recordings
/// and codecs with many more frames per second than AC-3.
const MAX_CACHED_INDEXES: usize = 4;
const MAX_CACHED_INDEX_BYTES: usize = 12 * 1024 * 1024;

/// Identifies a cached index. The file's size and high-resolution metadata
/// marker are part of the key so replacing a file in place invalidates its
/// index rather than serving byte offsets into a file that no longer has them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndexKey {
    /// Database id of the file.
    pub id: i64,
    /// Size in bytes at the time the index was built.
    pub size: u64,
    /// High-resolution, platform-specific metadata marker for the contents.
    pub modified: i64,
}

/// Identifies a cached fMP4 segment.
///
/// Segments are cached where the elementary index is not, and for a different
/// reason: a copy is cheap to redo, but a decode-and-re-encode is not, and
/// seeking or re-buffering asks for the same segment again and again. Keyed on
/// the track as well as the file because a film's renditions are built
/// independently and a browser may be pulling two of them at once.
///
/// The file is an [`IndexKey`] rather than a bare id, for the same reason that
/// one is: replacing a film's contents while it keeps its database row left
/// matching track and segment numbers answering out of the cache with the old
/// film's pictures, possibly alongside the new one's init segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SegmentKey {
    /// The file, with the fingerprint that tells a replacement apart.
    pub file: IndexKey,
    /// Container track id.
    pub track: u32,
    /// Segment index within the rendition.
    pub seq: u32,
}

impl IndexKey {
    /// The file's identity as every cache here keys on it: which record it is,
    /// and whether its contents are still the ones that were read.
    ///
    /// `None` when the file cannot be stat'd, which callers treat as "do not
    /// cache" rather than inventing a fingerprint.
    pub async fn for_file(id: i64, path: &std::path::Path) -> Option<Self> {
        let version = crate::media::ContentVersion::for_file(path).await?;
        Some(Self {
            id,
            size: version.size,
            modified: version.marker,
        })
    }

    /// A short token that changes when the file's contents do.
    ///
    /// Goes in the URLs a playlist hands out, so a browser's own cache — which
    /// keys on the URL and was told these are good for an hour — cannot serve
    /// the replaced film's segments either.
    pub fn version(&self) -> String {
        crate::media::ContentVersion {
            size: self.size,
            marker: self.modified,
        }
        .token()
    }
}

/// How many segments to keep, and how much memory they may occupy between them.
///
/// A segment of 1080p video is single-digit megabytes, so a count alone would
/// bound the wrong thing on a large file and the wrong thing on a small one.
///
/// What gets asked for twice is a segment just played — a seek back, a stall
/// that re-buffers — so the cache only needs the recent end of what is
/// playing: 24 MB is several 1080p segments of each of a few browsers. It was
/// 48 MB, held for the life of the process once anyone had watched a film.
const MAX_CACHED_SEGMENTS: usize = 24;
const MAX_CACHED_SEGMENT_BYTES: usize = 24 * 1024 * 1024;

/// Identifies the first run of packets a seeked response opens with.
///
/// Worth remembering because a television seeking a transport stream
/// binary-searches it — twenty-odd ranged requests, each read only far enough to
/// find one clock value — and a coarse seek snaps every byte offset inside one
/// group of pictures back to the same random-access point. On a film with
/// ten-second groups that is most of the search asking, in different words, for
/// bytes that have already been produced: twenty-nine requests, eight distinct
/// answers, measured.
///
/// The soundtracks are part of the key because `?audio_track=` produces a
/// different stream from the same instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkKey {
    pub file: IndexKey,
    /// Where the demuxer landed, in the source's own units.
    pub origin: u64,
    /// Which soundtracks the response carries.
    pub tracks: u64,
}

/// How many opening runs to keep, and how much memory they may occupy.
///
/// One is a fraction of a second of film, so single-digit megabytes each on a
/// high-bitrate feature — a count alone would bound the wrong thing.
///
/// Eviction is oldest-first, and the repeats come at the end of a seek, when
/// the probes have narrowed to one group of pictures — so what a seek needs
/// back is the few runs it produced last, not all of them. 24 MB keeps those
/// for a high-bitrate film; it was 64 MB.
const MAX_CACHED_CHUNKS: usize = 32;
const MAX_CACHED_CHUNK_BYTES: usize = 24 * 1024 * 1024;

/// How many films' soundtrack measurements to keep.
///
/// Two numbers per soundtrack, so this is bytes rather than megabytes and the
/// only reason there is a ceiling at all is that a library is not a session.
const MAX_CACHED_RATES: usize = 64;

/// Shared transcoding state, held by `AppState`.
#[derive(Debug)]
pub struct TranscodeState {
    cache: Mutex<Cache>,
    segments: Mutex<SegmentCache>,
    #[cfg(all(feature = "transcode-aac", feature = "casting"))]
    rates: Mutex<RateCache>,
    /// A plain lock, not the async one the others use: this is read and written
    /// from the blocking thread that does the muxing, which cannot await.
    chunks: std::sync::Mutex<ChunkCache>,
    permits: Arc<Semaphore>,
}

#[derive(Debug, Default)]
struct ChunkCache {
    entries: HashMap<ChunkKey, Arc<Vec<u8>>>,
    order: Vec<ChunkKey>,
    bytes: usize,
}

/// What each film's tracks were measured to cost.
///
/// Worth caching for the same reason the index is: a renderer opens a film with
/// a `HEAD`, then a `GET`, then a range request per scrub, and every one of them
/// has to state the same promised length or the byte offsets stop meaning the
/// same instants. Measuring once is both cheaper and the only way the answer is
/// guaranteed to be identical each time.
#[cfg(all(feature = "transcode-aac", feature = "casting"))]
#[derive(Debug, Default)]
struct RateCache {
    entries: HashMap<IndexKey, Arc<TrackRates>>,
    order: Vec<IndexKey>,
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
    /// a cap of four the difference is not measurable, and this needs no
    /// bookkeeping on the read path.
    order: Vec<IndexKey>,
    bytes: usize,
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
            #[cfg(all(feature = "transcode-aac", feature = "casting"))]
            rates: Mutex::new(RateCache::default()),
            chunks: std::sync::Mutex::new(ChunkCache::default()),
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
        if let Some(old) = cache.entries.remove(&key) {
            cache.bytes -= old.retained_bytes();
            cache.order.retain(|stored| *stored != key);
        }
        let bytes = index.retained_bytes();
        if bytes > MAX_CACHED_INDEX_BYTES {
            return;
        }
        cache.bytes += bytes;
        cache.entries.insert(key, index);
        cache.order.push(key);
        while cache.order.len() > MAX_CACHED_INDEXES || cache.bytes > MAX_CACHED_INDEX_BYTES {
            let oldest = cache.order.remove(0);
            if let Some(old) = cache.entries.remove(&oldest) {
                cache.bytes -= old.retained_bytes();
            }
        }
    }

    /// What `key`'s soundtracks were measured to cost, if they have been.
    #[cfg(all(feature = "transcode-aac", feature = "casting"))]
    pub async fn cached_rates(&self, key: &IndexKey) -> Option<Arc<TrackRates>> {
        self.rates.lock().await.entries.get(key).cloned()
    }

    /// Remember one film's measurement, evicting the oldest if full.
    #[cfg(all(feature = "transcode-aac", feature = "casting"))]
    pub async fn remember_rates(&self, key: IndexKey, rates: Arc<TrackRates>) {
        let mut cache = self.rates.lock().await;
        if cache.entries.insert(key, rates).is_none() {
            cache.order.push(key);
            while cache.order.len() > MAX_CACHED_RATES {
                let oldest = cache.order.remove(0);
                cache.entries.remove(&oldest);
            }
        }
    }

    /// The opening run of packets for `key`, if it was produced recently.
    ///
    /// Synchronous, because the caller is the blocking thread doing the muxing.
    /// A poisoned lock is treated as a miss: the worst that costs is producing
    /// the run again.
    pub fn cached_chunk(&self, key: &ChunkKey) -> Option<Arc<Vec<u8>>> {
        self.chunks.lock().ok()?.entries.get(key).cloned()
    }

    /// Remember an opening run, evicting oldest-first past either ceiling.
    pub fn remember_chunk(&self, key: ChunkKey, chunk: Arc<Vec<u8>>) {
        let Ok(mut cache) = self.chunks.lock() else {
            return;
        };
        let len = chunk.len();
        if len > MAX_CACHED_CHUNK_BYTES {
            if let Some(old) = cache.entries.remove(&key) {
                cache.bytes -= old.len();
                cache.order.retain(|stored| *stored != key);
            }
            return;
        }
        if let Some(old) = cache.entries.insert(key, chunk) {
            cache.bytes -= old.len();
        } else {
            cache.order.push(key);
        }
        cache.bytes += len;
        while cache.order.len() > MAX_CACHED_CHUNKS || cache.bytes > MAX_CACHED_CHUNK_BYTES {
            let oldest = cache.order.remove(0);
            if let Some(gone) = cache.entries.remove(&oldest) {
                cache.bytes -= gone.len();
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
        if len > MAX_CACHED_SEGMENT_BYTES {
            if let Some(old) = cache.entries.remove(&key) {
                cache.bytes -= old.len();
                cache.order.retain(|stored| *stored != key);
            }
            return;
        }
        if let Some(old) = cache.entries.insert(key, segment) {
            cache.bytes -= old.len();
        } else {
            cache.order.push(key);
        }
        cache.bytes += len;
        while cache.order.len() > MAX_CACHED_SEGMENTS || cache.bytes > MAX_CACHED_SEGMENT_BYTES {
            let Some(oldest) = cache.order.first().copied() else {
                break;
            };
            cache.order.remove(0);
            if let Some(evicted) = cache.entries.remove(&oldest) {
                cache.bytes -= evicted.len();
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
                    frames: Arc::from([]),
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
    async fn large_indexes_are_bounded_by_bytes_including_replacements() {
        fn large_index(bytes: usize) -> Arc<AudioPlan> {
            let mut plan = index();
            let frames = match &mut Arc::get_mut(&mut plan).unwrap().source {
                super::super::PacketSource::Elementary(frames) => frames,
                #[cfg(feature = "demux")]
                super::super::PacketSource::Container(_) => unreachable!(),
            };
            frames.frames = vec![
                super::super::IndexedFrame {
                    offset: 0,
                    len: 768,
                    samples: 1536
                };
                bytes / std::mem::size_of::<super::super::IndexedFrame>()
            ]
            .into();
            plan
        }
        let state = TranscodeState::new(1);
        let large = large_index(MAX_CACHED_INDEX_BYTES / 2);
        state.remember(key(1), large.clone()).await;
        state.remember(key(2), index()).await;
        state.remember(key(2), large).await;
        assert!(state.cached(&key(1)).await.is_none());
        assert!(state.cached(&key(2)).await.is_some());
        assert!(state.cache.lock().await.bytes <= MAX_CACHED_INDEX_BYTES);
        state
            .remember(key(2), large_index(MAX_CACHED_INDEX_BYTES + 1024))
            .await;
        assert!(state.cached(&key(2)).await.is_none());
        assert_eq!(state.cache.lock().await.bytes, 0);
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
            file: key(1),
            track: 2,
            seq,
        };
        // Four segments of 16 MB: the first is pushed out, because the byte
        // ceiling binds long before the entry count does.
        for seq in 0..4 {
            state
                .remember_segment(key(seq), bytes::Bytes::from(vec![0u8; 16 * 1024 * 1024]))
                .await;
        }
        assert!(state.cached_segment(&key(0)).await.is_none());
        assert!(state.cached_segment(&key(3)).await.is_some());
    }

    #[tokio::test]
    async fn replacing_a_segment_recounts_bytes_and_keeps_the_ceiling() {
        let state = TranscodeState::new(1);
        let key = |seq| SegmentKey {
            file: key(1),
            track: 2,
            seq,
        };
        state
            .remember_segment(key(0), bytes::Bytes::from_static(b"x"))
            .await;
        state
            .remember_segment(key(0), bytes::Bytes::from(vec![0; 20 * 1024 * 1024]))
            .await;
        state
            .remember_segment(key(1), bytes::Bytes::from(vec![0; 10 * 1024 * 1024]))
            .await;

        let cache = state.segments.lock().await;
        assert!(cache.bytes <= MAX_CACHED_SEGMENT_BYTES);
        assert_eq!(
            cache.bytes,
            cache.entries.values().map(bytes::Bytes::len).sum::<usize>()
        );
        assert!(
            !cache.entries.contains_key(&key(0)),
            "the older large segment is evicted"
        );
        assert!(cache.entries.contains_key(&key(1)));
    }

    #[test]
    fn replacing_a_chunk_recounts_bytes_and_keeps_the_ceiling() {
        let state = TranscodeState::new(1);
        let key = |origin| ChunkKey {
            file: key(1),
            origin,
            tracks: 1,
        };
        state.remember_chunk(key(0), Arc::new(vec![0]));
        state.remember_chunk(key(0), Arc::new(vec![0; 20 * 1024 * 1024]));
        state.remember_chunk(key(1), Arc::new(vec![0; 10 * 1024 * 1024]));

        let cache = state.chunks.lock().unwrap();
        assert!(cache.bytes <= MAX_CACHED_CHUNK_BYTES);
        assert_eq!(
            cache.bytes,
            cache
                .entries
                .values()
                .map(|chunk| chunk.len())
                .sum::<usize>()
        );
        assert!(
            !cache.entries.contains_key(&key(0)),
            "the older large chunk is evicted"
        );
        assert!(cache.entries.contains_key(&key(1)));
    }

    #[tokio::test]
    async fn an_oversized_replacement_is_not_retained() {
        let state = TranscodeState::new(1);
        let segment = SegmentKey {
            file: key(1),
            track: 2,
            seq: 0,
        };
        let chunk = ChunkKey {
            file: key(1),
            origin: 0,
            tracks: 1,
        };

        state
            .remember_segment(segment, bytes::Bytes::from_static(b"old"))
            .await;
        state.remember_chunk(chunk, Arc::new(b"old".to_vec()));
        state
            .remember_segment(
                segment,
                bytes::Bytes::from(vec![0; MAX_CACHED_SEGMENT_BYTES + 1]),
            )
            .await;
        state.remember_chunk(chunk, Arc::new(vec![0; MAX_CACHED_CHUNK_BYTES + 1]));

        assert!(state.cached_segment(&segment).await.is_none());
        assert!(state.cached_chunk(&chunk).is_none());
        assert_eq!(state.segments.lock().await.bytes, 0);
        assert_eq!(state.chunks.lock().unwrap().bytes, 0);
    }

    #[tokio::test]
    async fn a_segment_is_found_again_under_the_key_that_stored_it() {
        let state = TranscodeState::new(1);
        let segment = SegmentKey {
            file: key(7),
            track: 2,
            seq: 3,
        };
        state
            .remember_segment(segment, bytes::Bytes::from_static(b"segment"))
            .await;
        assert_eq!(
            state.cached_segment(&segment).await.as_deref(),
            Some(&b"segment"[..])
        );
        // A different rendition of the same file is a different segment.
        assert!(state
            .cached_segment(&SegmentKey { track: 3, ..segment })
            .await
            .is_none());
    }

    /// Replacing a film's contents while it keeps its database row must not
    /// leave the old film's pictures answering for the new one's segments.
    ///
    /// The key used to be id, track and sequence — nothing about the file — so
    /// every matching segment number came back out of the cache, potentially
    /// alongside an init segment built from the new file.
    #[tokio::test]
    async fn a_file_replaced_in_place_does_not_reuse_its_segments() {
        let state = TranscodeState::new(1);
        let segment = SegmentKey {
            file: key(7),
            track: 2,
            seq: 3,
        };
        state
            .remember_segment(segment, bytes::Bytes::from_static(b"old film"))
            .await;

        let rewritten = SegmentKey {
            file: IndexKey {
                id: 7,
                size: 999,
                modified: 2,
            },
            ..segment
        };
        assert!(state.cached_segment(&rewritten).await.is_none());
        // And the version a playlist hands out changes with it, so a browser
        // that was told these are good for an hour asks for new URLs.
        assert_ne!(rewritten.file.version(), segment.file.version());
    }
}

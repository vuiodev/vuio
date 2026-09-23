# Memory audit: September 2026

## Measured peak RSS

Measured on macOS in separate processes using `/usr/bin/time -l` and the same
debug-profile core test binary (the workspace optimizes codecs in this profile).
These are isolated workload measurements, **not whole-server or idle-RSS claims**.
The comparison paths reproduce the previous allocation/retention patterns inside
the probes; they are not separate builds of the old revision.

| Workload | Previous pattern | Current implementation | Reduction |
| --- | ---: | ---: | ---: |
| Build a single-track fMP4 segment (every HLS segment) with 64 MiB of payload | 211,550,208 B (201.8 MiB) | 144,310,272 B (137.6 MiB) | 32% |
| Build a multi-track fMP4 fragment (a film with re-encoded audio for a television) with 64 MiB of payload | 278,675,456 B (265.8 MiB) | 144,523,264 B (137.8 MiB) | 48% |
| Read a two-hour AAC audiobook for radio | 164,003,840 B (156.4 MiB) | 14,548,992 B (13.9 MiB) | 91% |

All four fragment paths produced 67,108,976 bytes, with hash
`9318397001415464912`. Both radio paths produced 336,963 frames and 117,404,618
bytes, with hash `15861541014108818885`. Hashes use Rust's `DefaultHasher` and
are useful for comparison within the same build, not as a stable file format.
The radio probe drains frames without real-time pacing; the production channel
also bounds read-ahead when playback is slower.

Both fragment builders now write `mdat` directly into their final output. The
single-track builder, which HLS passthrough and HLS audio go through, used to
keep one additional payload-sized copy; the multi-track builder kept two, which
is the larger saving but the less common path. The radio reader now uses
backpressure with a 32-frame queue instead of retaining the entire audiobook.

## Other fixes and bounds

- Radio queues retain track IDs rather than paths, titles and artists; only the
  current track's metadata is loaded for playout. A track whose row disappears
  mid-pass — deleted and indexed again, which gives it a new ID — makes the
  station read its queue again and carry on from where it was, so the track is
  not lost for the rest of the pass (for a linear station, the whole broadcast).
- Elementary-stream frame indexes share immutable storage across listeners.
  Their cache has a 12 MiB byte budget as well as a four-entry limit. Oversized
  indexes can still be used by an active stream, but are not cached.
- Replacing cached chunks and segments updates byte accounting and evicts as
  needed, including removal of stale entries on oversized replacement.
- Video/TS timestamp estimates stop accumulating after initial placement.
  fMP4 Dolby priming also stops reading ahead after 16 MiB or 512 packets
  (the byte threshold can be exceeded by the final packet).
- All HLS segment builds, including passthrough video, share four slots. A
  request waits up to five seconds for one before it receives 503 with
  `Retry-After: 1`: a seek's abandoned builds keep their slots until they
  finish, and refusing at once turned scrubbing into a run of 503s. Metadata
  probes share one slot per core, at least two and at most eight, across every
  scan and watch event; two slots halved the probe rate on an eight-core
  machine. Blocking workers retain their permits after caller cancellation;
  elementary transcode planning likewise keeps its existing transcode slot.
- Metadata-fetch jobs page pending IDs in groups of 256, using an initial ID
  high-water mark; misses are not retried forever and later insertions do not
  extend the job.
- Directory repair flushes accumulated directory deltas at 4,096 entries;
  folder renames and scan deletions operate in 1,000-file batches. Playlist
  resolution loads at most 512 full metadata records per batch. Folder casting
  queries playback fields rather than full records.
- SQLite temporary sorts/index work can spill to disk (`temp_store = FILE`)
  instead of requiring the entire temporary structure in RAM.
- AirPlay event/command queues are bounded. Diagnostic replies may be dropped
  if their consumer is slow; remote commands use backpressure. Cast request
  registration prunes abandoned waiters. Log-tail reads remain byte-bounded
  even while a log grows.
- Watcher root removal drops its old file-ID hash tables, releasing retained
  capacity while preserving other roots. AAC zeroed-allocation optimization is
  explicitly opted into only by callers whose allocator guarantees zeroed memory.

Regression coverage includes multi-page deletion, directory repair and metadata
jobs, playlist duplicates across batch boundaries, cache replacements, shared
frame storage, radio frame order/cancellation, metadata-probe cancellation,
fragment payload offsets, and long-running timestamp state.

Verification: 845 core unit/integration tests, 75 casting unit tests and four
AAC codec tests passed; ten core tests were ignored (including the two RSS
probes run separately). Strict library Clippy and no-default-feature checks for
transcode-only and casting-only builds passed. Tests used all server features
except `web-ui`, whose generated assets are absent in this worktree. All-target
Clippy with `-D warnings` hits an existing unused web-UI test helper in that
feature configuration; the library lint run is clean.

## Reproducing the RSS probes

Build the unit-test executable (without the separately generated web UI):

```sh
cargo test --locked -p vuio-core --no-default-features \
  --features casting,dashboard,diagnostics,mcp,metadata,mediainfo,transcode-ac3,transcode-dts,transcode-aac \
  --lib --no-run
```

Set `audit_test_binary` to the executable path Cargo prints, then run each case
in a fresh process:

```sh
/usr/bin/time -l "$audit_test_binary" fragment_rss_probe --ignored --nocapture
VUIO_LEGACY_FRAGMENT=1 /usr/bin/time -l "$audit_test_binary" fragment_rss_probe --ignored --nocapture
VUIO_FRAGMENT_SHAPE=single /usr/bin/time -l "$audit_test_binary" fragment_rss_probe --ignored --nocapture
VUIO_FRAGMENT_SHAPE=single VUIO_LEGACY_FRAGMENT=1 \
  /usr/bin/time -l "$audit_test_binary" fragment_rss_probe --ignored --nocapture

audit_media_dir=$(mktemp -d /tmp/vuio-memory-probe.XXXXXX)
ffmpeg -hide_banner -loglevel error -f lavfi \
  -i 'sine=frequency=440:sample_rate=48000:duration=10' \
  -ac 2 -c:a aac -b:a 128k "$audit_media_dir/seed.m4a"
ffmpeg -hide_banner -loglevel error -stream_loop 719 \
  -i "$audit_media_dir/seed.m4a" -t 7200 -c copy "$audit_media_dir/audiobook.m4a"
VUIO_MEMORY_AUDIO="$audit_media_dir/audiobook.m4a" /usr/bin/time -l \
  "$audit_test_binary" radio_rss_probe --ignored --nocapture
VUIO_MEMORY_AUDIO="$audit_media_dir/audiobook.m4a" VUIO_RETAIN_RADIO=1 \
  /usr/bin/time -l "$audit_test_binary" radio_rss_probe --ignored --nocapture
```

On Linux use `/usr/bin/time -v`; its maximum-RSS field is in KiB rather than
macOS's bytes. The generated media remains in the temporary directory for
repeat runs.

## Remaining scaling limits

This is not a guarantee that every allocation in the server is bounded. The
scanner still keeps one compact fingerprint per indexed file under the root
being scanned; very large roots remain a significant RAM workload. Casting and
radio queues also scale with their number of tracks, though their records are
smaller now. Active response bodies and indexes can outlive cache eviction.

Symphonia's metadata/visual byte-limit options do not enforce every parser
allocation. Probe concurrency limits reduce simultaneous allocation peaks,
but do not impose a hard per-file memory ceiling. A malformed file, a very large
container index, or an unusually large video packet still needs separate
parser-level investigation. Measure representative whole-server workloads
before translating the isolated reductions above into an RSS expectation for
a deployment.

Every figure here is a peak measured on macOS. How much of a peak stays
resident afterwards depends on the allocator, and the deployments use others:
glibc keeps freed memory in per-thread arenas for reuse, and
`release_free_memory` (macOS and glibc only) hands it back after a large scan,
not after streaming. The Docker image is built on Alpine; musl has no such
call, so there `release_free_memory` does nothing.

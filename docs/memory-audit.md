# Memory audit: September 2026

## Measured peak RSS

Measured on macOS in separate processes using `/usr/bin/time -l` and the same
debug-profile core test binary (the workspace optimizes codecs in this profile).
These are isolated workload measurements, **not whole-server or idle-RSS claims**.
The comparison paths reproduce the previous allocation/retention patterns inside
the probes; they are not separate builds of the old revision.

| Workload | Previous pattern | Current implementation | Reduction |
| --- | ---: | ---: | ---: |
| Build an fMP4 fragment with 64 MiB of payload | 278,380,544 B (265.5 MiB) | 144,293,888 B (137.6 MiB) | 48% |
| Read a two-hour AAC audiobook for radio | 164,003,840 B (156.4 MiB) | 14,548,992 B (13.9 MiB) | 91% |

Both fragment paths produced 67,108,976 bytes, with hash
`9318397001415464912`. Both radio paths produced 336,963 frames and 117,404,618
bytes, with hash `15861541014108818885`. Hashes use Rust's `DefaultHasher` and
are useful for comparison within the same build, not as a stable file format.
The radio probe drains frames without real-time pacing; the production channel
also bounds read-ahead when playback is slower.

The fragment builder now writes `mdat` directly into its final output instead
of keeping two additional payload-sized copies. The radio reader now uses
backpressure with a 32-frame queue instead of retaining the entire audiobook.

## Other fixes and bounds

- Radio queues retain track IDs rather than paths, titles and artists; only the
  current track's metadata is loaded for playout.
- Elementary-stream frame indexes share immutable storage across listeners.
  Their cache has a 12 MiB byte budget as well as a four-entry limit. Oversized
  indexes can still be used by an active stream, but are not cached.
- Replacing cached chunks and segments updates byte accounting and evicts as
  needed, including removal of stale entries on oversized replacement.
- Video/TS timestamp estimates stop accumulating after initial placement.
  fMP4 Dolby priming also stops reading ahead after 16 MiB or 512 packets
  (the byte threshold can be exceeded by the final packet).
- All HLS segment builds, including passthrough video, share four slots. Busy
  requests receive 503 with `Retry-After: 1`. Metadata probes share two slots.
  Blocking workers retain their permits after caller cancellation; elementary
  transcode planning likewise keeps its existing transcode slot.
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

Verification: 838 core unit/integration tests, 75 casting unit tests and four
AAC codec tests passed; nine core tests were ignored (including the two RSS
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

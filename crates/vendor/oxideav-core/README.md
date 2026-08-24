# oxideav-core

[![CI](https://github.com/OxideAV/oxideav-core/actions/workflows/ci.yml/badge.svg)](https://github.com/OxideAV/oxideav-core/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/oxideav-core.svg)](https://crates.io/crates/oxideav-core) [![docs.rs](https://docs.rs/oxideav-core/badge.svg)](https://docs.rs/oxideav-core) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Core types for the [oxideav](https://github.com/OxideAV/oxideav-workspace)
pure-Rust media framework:

* **`Packet`** — one compressed chunk belonging to one stream, with
  timestamps. Chainable `with_*` builders cover every
  [`PacketFlags`](crate::packet::PacketFlags) field
  (`with_keyframe` / `with_header` / `with_corrupt` / `with_discard` /
  `with_unit_boundary`, plus a bulk `with_flags`) and the
  stream-index / time-base / pts / dts / duration setters used by
  demuxers and remuxers. An `end_pts()` accessor returns the
  overflow-checked `pts + duration` for muxers that need a per-
  packet end timestamp.
* **`Frame`** — one uncompressed audio / video / subtitle chunk.
  `VideoFrame` can carry typed in-band side-channels alongside its
  pixel planes: a palette for palette-indexed (`Pal8`) content
  (`palette()` / `set_palette` / `take_palette`) and a per-plane
  significant-bits record for mixed depths no single `PixelFormat`
  names — e.g. 12-bit luma with 10-bit chroma from a custom signal
  range (`significant_bits()` / `set_significant_bits` /
  `take_significant_bits`, LSB-anchored values). The two records
  compose on one frame; `image_planes()` iterates pixel data
  side-channel-agnostically.
* **`StreamInfo`** / **`CodecParameters`** — what a demuxer advertises and
  what a decoder / encoder consumes.
* **`TimeBase`** / **`Timestamp`** / **`Rational`** — rational time per
  stream; timestamps are integers in that base. Named constants
  (`MILLIS` / `MICROS` / `NANOS` / `MPEG_TS` / `AUDIO_48K` / `AUDIO_44K1`
  / `AUDIO_8K` / `SECONDS`) replace the workspace's `TimeBase::new(1, …)`
  magic-numbers; `TimeBase::from_rate(u32)` constructs the inverse-of-rate
  form, and `ticks_of(seconds: f64)` is the overflow-clamped inverse of
  the existing `seconds_of(ticks)`. `Timestamp::from_seconds` /
  `checked_add_ticks` / `checked_sub_ticks` / `checked_diff` /
  `checked_rescale` cover per-stream timestamp arithmetic (including
  cross-base differences for remux pipelines).

  The whole numeric core is **total — no panic, no silent wrap, even on
  `i64::MIN` terms or zero denominators**. `rescale` computes in 128-bit
  sign+magnitude space, rounds half-away-from-zero, and *saturates* at
  the `i64` boundaries; `rescale_checked` returns `None` instead
  wherever `rescale` would saturate or default; `rescale_rnd` takes an
  explicit `Rounding` mode (`NearestAway` / `Floor` for DTS-safe stamps
  / `Ceil` / `TowardZero`). `Rational` supports `+ - * /` and unary `-`
  (exact via `i128` intermediates, reduced, closest-representable
  approximation when even the reduced result exceeds `i64`),
  `checked_add/sub/mul/div` that report `None` exactly where the
  operators approximate, plus `cmp_value` / `equals_value` for value
  comparison (`30000/1001` vs `30/1`) that doesn't disturb the
  structural `Eq`/`Hash` callers rely on to preserve the on-wire
  fraction. Property-tested against independent `i128` oracles (~200k
  edge-biased cases in `tests/props.rs`).
* **`PixelFormat`** / **`SampleFormat`** — enum of supported raw formats
  (70 pixel variants including 8/10/12/16-bit YUV at
  4:2:0/4:2:2/4:4:4/4:1:1/4:4:0, YUV+alpha at 4:2:0/4:2:2/4:4:4 in both
  8-bit and deep 10/12/16-bit flavours, planar GBR(A) across the full
  8/10/12/14/16-bit depth ladder with an alpha companion at every
  depth, scene-referred 32-bit float gray/RGB(A)/planar-GBR(A) for
  linear-light HDR, packed RGB/RGBA, gray+alpha at 8 and 16 bits, CMYK
  in both ink conventions, NV12/NV21, all common sample layouts), plus
  plane-geometry helpers on every variant: `chroma_subsampling()`
  (log2 shifts per sampling class), `plane_dimensions()`
  (ceil-division subsampled grids), and tightly-packed sizing via
  `plane_row_bytes()` / `plane_size_bytes()` / `frame_size_bytes()`
  with checked arithmetic.
* **`AttachedPicture`** / **`PictureType`** — ID3v2 `APIC` taxonomy
  shared by ID3v2 / FLAC / MP4 / Vorbis cover-art carriage. `PictureType`
  round-trips byte-for-byte through `from_u8` ↔ `to_u8` over the spec-
  assigned `0x00..=0x14` range; unassigned bytes collapse to `Unknown`,
  flagged via `is_known()` so strict writers can refuse to emit the
  `0xFF` sentinel. `AttachedPicture::new(mime, kind)` plus chainable
  `with_description` / `with_data` / `with_picture_type` builders cover
  the producer side (parsers writing into a partially-decoded picture
  as bytes arrive), and `is_external_link()` distinguishes ID3v2's
  `"-->"` URL-sentinel mime from inline image bytes without having to
  hardcode the string at every call site.
* **`CodecTag`** / **`CodecResolver`** — neutral abstraction for mapping
  container-level tags (AVI FourCC, WAVEFORMATEX `wFormatTag`, MP4 OTI,
  Matroska CodecID strings) to oxideav `CodecId`s. Lets codec crates own
  their own tag claims without pulling a codec registry into every
  container. Tag-less identification gets its own container-agnostic
  path: codecs declare the payload magic prefixes they answer to
  (`CodecInfo::payload_magic(b"\x01vorbis")`, `b"OpusHead"`, `b"fLaC"`,
  …) and callers resolve a stream's leading payload bytes with
  `CodecResolver::resolve_payload_magic(first_bytes)` — longest
  matching magic wins, then registration order. Serves Ogg's BOS
  packets and raw elementary-stream sniffing alike.
* **`bits`** — shared MSB-first / LSB-first `BitReader` / `BitWriter`
  plus unary helpers. Used by the FLAC, AAC, H.264, HEVC, Vorbis and a
  dozen other codecs in the workspace. The LSB pair (the Vorbis §2.1.4
  layout) exposes the full MSB surface — `peek_u32` (Huffman lookup
  windows), `skip` / `consume`, `align_to_byte`, `read_bytes`,
  positional bookkeeping, `write_bytes` and the alias set. Criterion
  baselines live in `benches/primitives.rs` (~1.3 GiB/s read,
  ~430 MiB/s write on a mixed-width field schedule).
* **`SourceRegistry`** — URI scheme dispatch for sources. Drivers
  register as one of three shapes — `BytesSource` (file / http), 
  `PacketSource` (transport-layer protocols that pre-demux), or
  `FrameSource` (synthetic generators that emit decoded frames) —
  and `open(uri)` returns a `SourceOutput` enum the pipeline executor
  branches on.
* **`Error`** — one unified error enum used across the ecosystem, with
  a documented caller-action taxonomy (verdict vs starvation vs
  backpressure), constructors for every string variant, and
  `is_eof` / `is_need_more` / `is_starved` / `is_resource_exhausted`
  predicates (the enum can't be `PartialEq` — `Io` wraps
  `std::io::Error`).

Every public item is documented (`#![warn(missing_docs)]` is enforced
at the crate root, promoted to deny by CI's clippy gate) and
`cargo doc` is warning-clean under docs.rs-strict settings.

Zero C dependencies. Zero FFI. Zero `*-sys` crates.

## Usage

```toml
[dependencies]
oxideav-core = "0.1"
```

Everything downstream in oxideav (codec traits, container traits, codec
implementations, the CLI) depends on this crate transitively, so the
surface is kept deliberately small. The 0.1 series is the first stable
semver line — additive changes are `0.1.x` patch bumps; breaking
reshapes go to `0.2.0`.

## License

MIT — see [LICENSE](LICENSE).

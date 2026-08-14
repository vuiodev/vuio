# Architecture Overview

VuIO is designed as a modular, high-concurrency media server built entirely in Rust. It utilizes an asynchronous runtime ([Tokio](https://tokio.rs)), a fast HTTP layer ([Axum](https://github.com/tokio-rs/axum)), and an embedded [SQLite](https://www.sqlite.org) database.

---

## Component Diagram

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Web Server    │    │  SSDP Service   │    │ File Watcher    │
│   (Axum/HTTP)   │    │  (Discovery)    │    │ (Real-time)     │
└────────┬────────┘    └────────┬────────┘    └────────┬────────┘
         │                      │                      │
         └──────────────────────┼──────────────────────┘
                                │
         ┌──────────────────────┴──────────────────────┐
         │              Application Core               │
         │  ┌─────────┐  ┌─────────┐  ┌─────────────┐  │
         │  │ Config  │  │ Database│  │  Platform   │  │
         │  │ Manager │  │ (SQLite)│  │ Abstraction │  │
         │  └─────────┘  └─────────┘  └─────────────┘  │
         └──────────────────────┬──────────────────────┘
                                │
         ┌──────────────────────┴──────────────────────┐
         │            Platform Layer                   │
         │  ┌─────────┐  ┌─────────┐  ┌─────────┐      │
         │  │ Windows │  │  macOS  │  │  Linux  │      │
         │  └─────────┘  └─────────┘  └─────────┘      │
         └─────────────────────────────────────────────┘
```

---

## Core Subsystems

### 1. HTTP & Streaming Engine (`crates/vuio-core/src/web`)
- Built on **Axum** and **Tower**.
- Serves HTTP Range requests with byte-accurate seeking for large 4K/UHD video and lossless audio.
- Direct static file serving for the bundled Svelte application (`crates/vuio-web`).
- Supports zero-restart dynamic port and interface binding.

### 2. SSDP & UPnP Discovery (`crates/vuio-core/src/ssdp`)
- Implements the UPnP Device Architecture 1.0/2.0 discovery specification.
- Periodically multicasts `ssdp:alive` and `ssdp:byebye` packets over UDP `239.255.255.250:1900`.
- Concurrent mDNS (Bonjour / DNS-SD) advertiser for zero-configuration discovery on Apple and local devices.

### 3. File Watcher & Scanner (`crates/vuio-core/src/scanner`)
- Cross-platform recursive filesystem notification engine based on `notify`.
- Debounced event batches to handle mass copying and moving without thrashing disk I/O.
- Automatic metadata extractor parsing ID3, FLAC Vorbis comments, MP4 atoms, and image EXIF data.

### 4. Database & Browse Cache (`crates/vuio-core/src/db`)
- Embedded ACID-compliant SQLite engine with WAL (Write-Ahead Logging) mode.
- Memory-mapped index caching configurable via `database.cache_mb`.
- Thread-safe SOAP Browse response cache storing pre-rendered XML signatures for instant directory listings on DLNA renderers.

### 5. Multi-Protocol Casting (`crates/vuio-cast`)
- Async Google Cast (Chromecast) protocol client with `ring` TLS.
- DLNA / UPnP AVTransport and RenderingControl client.
- Compatible AirPlay video streaming.

### 6. Model Context Protocol (`crates/vuio-core/src/mcp`)
- JSON-RPC 2.0 endpoint implementing MCP version `2026-07-28`.
- Exposes tools for library exploration, metadata inspection, playlist curation, and remote renderer playback.

---

## Related Documentation

- [Development Guide](DEV.md)
- [API Reference](api.md)
- [Configuration Reference](configuration.md)

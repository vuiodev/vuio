# VuIO Media Server

A lightweight, high-performance, cross-platform media server written in Rust. Streams video, audio, and images to **DLNA / UPnP**, **Chromecast / Google TV**, and compatible **AirPlay** video receivers.

- **Low resource usage**: Uses only ~25 MB of RAM for a 5,000-object media library. For all functions listed below.
And Less then 100Mb for 100000 objects.
- **Modern asynchronous core**: Built with Rust, Tokio, Axum, and SQLite for maximum concurrency and stability.
- **Cross-platform**: Native binaries for Linux, macOS,  Windows (x64 and arm), Freebsd (x64), Docker, and Kubernetes. Repo with setup packages for all linux flows, DEB, RPM, APK, Pacman, musl and tar.gz.
- **Single binary, no dependencies**, All is included in the binary, no need to install anything else,  just download and run.

## Features

- **DLNA/UPnP Media Server** – Stream to any Smart TV or DLNA renderer with SSDP and mDNS (Bonjour) discovery.
- **Multi-Protocol Casting** – Play directly to DLNA, Chromecast/Google TV, and AirPlay video receivers from the web UI or API.
- **Dual Web Interfaces** – Modern Svelte web interface (`:8090`) alongside a lightweight built-in dashboard (`:8080`).
- **AI Agent & MCP Integration** – Native Model Context Protocol (MCP 2026-07-28) support for AI assistants (Claude, ChatGPT, autonomous agents) to browse, search, and cast.
- **Instant Search & Fast Indexing** – High-performance SQLite engine with parent-directory indexing and sub-millisecond SOAP browse cache.
- **HTTP Range Streaming** – Full seek support for massive 4K/UHD media files.
- **Broad Format Support** – MKV, MP4, AVI, WebM, MP3, FLAC, WAV, AAC, OGG, JPEG, PNG, and more.
- **Audio Tagging & Playlists** – Automatic extraction of artist, album, genre, and year tags; auto-imports M3U/M3U8 and PLS playlists.
- **Live File Monitoring** – Real-time filesystem watcher updates the library dynamically as files change.
- **Live Configuration** – 21 of 25 settings apply dynamically in place without restarting the server.
- **Live Radio Broadcasting & P2P Tuner** – Broadcast continuous radio stations from folder selections with synchronous playout clock and P2P peer station discovery across local VuIO instances.
- **Multi-Provider Metadata Scraping (MediaInfo)** – Automatic metadata, poster artwork, ratings, and synopsis enrichment via TMDb, OMDb, TVmaze, MusicBrainz, Discogs, Last.fm, Genius, AniList, Jikan, and Kitsu.
- **On-the-Fly WebVTT Subtitle Conversion** – Auto-detects sidecar SRT subtitles and transforms them dynamically to WebVTT for browser and cast playback with zero disk writes.
- **HLS Video Streaming & In-Browser Player** – Master playlist generation with segmented HLS streaming (`/media/{id}/hls/master.m3u8`), Plyr video player, and sticky audio player.
- **Observability & Log Streaming** – Native Kubernetes HA probes (`/healthz`, `/readyz`), Prometheus exposition metrics (`/metrics`), and HTTP log streaming (`/logs`) for Grafana Loki/Alloy.
- **Granular Security & Admin Management** – Optional administrative token protection, secure session management, and CIDR subnet access restrictions (`allowed_networks`).

---

## Quick Start

### Homebrew (macOS & Linux)

```bash
brew tap vuiodev/vuio
brew install vuio
```

### Docker

```bash
docker run -d \
  --name vuio-server \
  --restart unless-stopped \
  --network host \
  -v /path/to/media:/media:ro \
  -v ./vuio-config:/config \
  -e VUIO_IP=192.168.1.100 \
  ghcr.io/vuiodev/vuio:latest
```

### Ubuntu / Debian

```bash
echo "deb [trusted=yes] https://vuiodev.github.io/vuio/apt stable main" | sudo tee /etc/apt/sources.list.d/vuio.list
sudo apt update && sudo apt install vuio
```

---

## Documentation Index

Explore the detailed documentation guides for full installation, configuration, and integration instructions:

| Guide | Description |
|---|---|
| 📖 **[Installation Guide](docs/install.md)** | Complete installation instructions for Linux (APT, DNF, APK, Pacman, musl), macOS, Windows, Docker, Kubernetes, FreeBSD, and Systemd. |
| 🖥️ **[Web Interface & Dashboard](docs/web-ui.md)** | Modern Svelte web UI (`:8090`), built-in dashboard (`:8080`), media players, casting controls, and in-browser settings admin. |
| ⚙️ **[Configuration Guide](docs/configuration.md)** | Full TOML configuration reference (`config.toml`), live hot-reload vs restart behavior, and metadata provider API keys. |
| 🐳 **[Docker Deployment](docs/docker.md)** | Docker Compose setups, single/multi/NFS volume mounting, network modes, and full environment variables table. |
| ☸️ **[Kubernetes & Helm](docs/kubernetes.md)** | Helm 3 chart deployment, host networking for SSDP multicast discovery, and persistent storage. |
| 🤖 **[AI Agent & MCP Integration](docs/mcp.md)** | Model Context Protocol setup, Claude integration (plugin, stdio, Desktop), and full tools reference catalog. |
| 🔒 **[Security & Authentication](docs/security.md)** | Administrative authentication, secure sign-in tokens, session lifetimes, and CIDR network restriction filters. |
| 📊 **[Monitoring & Observability](docs/monitoring.md)** | High-availability probes (`/healthz`, `/readyz`), Prometheus metrics (`/metrics`), JSON telemetry, and DLNA browse caching. |
| 📝 **[Logging & Diagnostics](docs/logging.md)** | Rolling background log files, `--debug` console tracing, custom log destinations, and log streaming endpoint (`/logs`). |
| 🎵 **[Audio & Media Features](docs/audio.md)** | Music metadata extraction, supported lossless/lossy formats, playlist discovery, and recommended directory organization. |
| 🔌 **[API Reference](docs/api.md)** | Comprehensive REST, UPnP/DLNA SOAP, and MCP endpoint specifications with request/response examples. |
| 🏗️ **[Architecture Overview](docs/architecture.md)** | Core system components, concurrency model, multi-protocol casting pipeline, and platform layer. |
| 🛠️ **[Development Guide](docs/DEV.md)** | Building from source, multi-architecture Docker image builds, and local development workflows. |

---

## Command Line Usage

```
Usage: vuio [OPTIONS] [MEDIA_DIR]

Arguments:
  [MEDIA_DIR]  Directory containing media files

Options:
  -p, --port <PORT>        Port to listen on [default: 8080]
  -n, --name <NAME>        DLNA / AirPlay server friendly name
  -c, --config <CONFIG>    Path to configuration file
  -m, --media-dir <DIR>    Additional media directories to scan
      --debug              Enable verbose console debug logging
      --log-file <PATH>    Path to custom log file
      --log-level <LEVEL>  Set log level (off, error, warn, info, debug, trace)
      --update             Update binary to the latest release from GitHub
      --auth               Enable administrative authentication
  -h, --help               Print help information
  -V, --version            Print version information
```

### Self-Updater

Update an installed binary to the latest GitHub release:

```bash
vuio --update
```

---

## Contributing

Contributions are welcome! Please ensure cross-platform compatibility is maintained across Linux, macOS, and Windows.

- **Backend & Core**: Rust workspace (`crates/vuio-core`, `crates/vuio-cli`, `crates/vuio-cast`).
- **Web Interface**: Developed in its own repository at [vuiodev/vuio-web](https://github.com/vuiodev/vuio-web). The built bundle (`crates/vuio-web/dist`) is refreshed via `./scripts/build-web.sh`.

---

## Credits & Third-Party Code

* **[oxicast](https://github.com/denniskribl/oxicast)**: Async Google Cast (Chromecast) client for Rust by Dennis Kribl (MIT / Apache-2.0). Maintained locally in `crates/vuio-cast` with `ring` TLS.

---

## License

Dual-licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

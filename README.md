# VuIO Media Server

A cross-platform DLNA/UPnP media server written in Rust. Streams video, audio, and images to any DLNA-compatible device (smart TVs, receivers, game consoles).
Less than 8Mb of RAM needed

Built with Tokio, Axum, and Redb for high performance and reliability.

**Supported platforms:** Windows, Linux, macOS, Docker (x64 and ARM64)

```bash
git clone https://github.com/vuiodev/vuio.git
cd vuio
cargo build --release
./target/release/vuio /path/to/media
```

## Features

- **DLNA/UPnP Media Server** - Stream to any DLNA device with SSDP discovery
- **Web Interface** - Modern dashboard showing server status, scanned files, and directories
- **AI Agent & MCP Integration** - AI agents (voice assistants, chatbots, and autonomous agents) can interact with your media library and control playback on smart TVs on the local network.
- **Global Search** - Instant search across all indexed filenames and paths
- **HTTP Range Streaming** - Seek support for large media files
- **Multi-format Support** - MKV, MP4, AVI, MP3, FLAC, WAV, AAC, OGG, JPEG, PNG, and more
- **Audio Metadata** - Automatic extraction of artist, album, genre, year from tags
- **Music Browsing** - Browse by Artists, Albums, Genres, Years via DLNA
- **Playlist Support** - Auto-imports M3U/PLS playlists from media directories
- **Real-time Monitoring** - Detects file changes and updates database automatically
- **Cross-platform** - Native integration for Windows, macOS, Linux
- **Redb Database** - Embedded ACID-compliant database with crash recovery

## Web Interface & Search

VuIO features a built-in web dashboard at `http://<server-ip>:<port>` (default: `http://localhost:8080`):
- **Sleek Dashboard**: Real-time server status, monitored directories, and database statistics.
- **Media Explorer**: Browse all scanned videos, music, and pictures directly in your web browser.
- **Instant Search**: Quick client-side filtering/searching across all files and paths as you type.

## Quick Start

```bash
# Run with default settings (scans ~/Videos, ~/Music, ~/Pictures)
./vuio

# Specify media directory
./vuio /path/to/media

# Custom port and name
./vuio -p 9090 -n "My Media Server" /path/to/media

# Multiple directories
./vuio /movies --media-dir /music --media-dir /photos
```

### Command Line Options

```
Usage: vuio [OPTIONS] [MEDIA_DIR]

Arguments:
  [MEDIA_DIR]  Directory containing media files

Options:
  -p, --port <PORT>        Port to listen on [default: 8080]
  -n, --name <NAME>        DLNA server name
  -c, --config <CONFIG>    Path to config file
      --media-dir <DIR>    Additional media directories
      --debug              Enable debug logging
      --log-file <PATH>    Path to custom log file
      --log-level <LEVEL>  Set log level (off, error, warn, info, debug, trace)
  -h, --help               Print help
  -V, --version            Print version
```

## Docker

> Docker does not work on macOS due to multicast limitations.

### Quick Start

```bash
git clone https://github.com/vuiodev/vuio.git
cd vuio
docker-compose -f docker-compose.yml up
```

### Docker Compose

```yaml
services:
  vuio:
    image: vuio:latest
    container_name: vuio-server
    restart: unless-stopped
    network_mode: host  # Required for DLNA multicast
    cap_add:
      - NET_ADMIN
      - NET_RAW
    volumes:
      - ./vuio-config:/config
      - /path/to/media:/media:ro
    environment:
      - VUIO_IP=192.168.1.100      # Your host IP (required)
      - VUIO_PORT=8080
      - VUIO_MEDIA_DIRS=/media
      - VUIO_SERVER_NAME=VuIO
      - VUIO_DB_PATH=/data/vuio.redb
```

### Docker Volume Mounting

**Single directory:**
```yaml
volumes:
  - ./vuio-config:/config
  - /path/to/media:/media:ro
```

**Multiple directories:**
```yaml
volumes:
  - ./vuio-config:/config
  - /home/user/Movies:/media/movies:ro
  - /home/user/Music:/media/music:ro
  - /home/user/Pictures:/media/pictures:ro
  - /mnt/nas/media:/media/nas:ro
environment:
  - VUIO_MEDIA_DIRS=/media/movies,/media/music,/media/pictures,/media/nas
```

**Network storage (NFS/SMB):**
```yaml
volumes:
  - type: bind
    source: /mnt/nas/media
    target: /media
    read_only: true
```

### Docker Run

```bash
docker run -d \
  --name vuio-server \
  --restart unless-stopped \
  --network host \
  --cap-add NET_ADMIN \
  --cap-add NET_RAW \
  -v /path/to/media:/media:ro \
  -v ./vuio-config:/config \
  -e VUIO_IP=192.168.1.100 \
  -e VUIO_PORT=8080 \
  -e VUIO_MEDIA_DIRS=/media \
  -e VUIO_DB_PATH=/data/vuio.redb \
  vuio:latest
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `VUIO_IP` | - | **Required.** Host IP for DLNA announcements |
| `VUIO_PORT` | 8080 | HTTP server port |
| `VUIO_SERVER_NAME` | VuIO | DLNA server name |
| `VUIO_UUID` | random | Device UUID (set for persistence) |
| `VUIO_MEDIA_DIRS` | /media | Comma-separated media paths |
| `VUIO_SCAN_ON_STARTUP` | true | Scan media on startup |
| `VUIO_WATCH_CHANGES` | true | Monitor for file changes |
| `VUIO_CLEANUP_DELETED` | true | Remove deleted files from DB |
| `VUIO_SCAN_PLAYLISTS` | true | Import M3U/PLS playlists |
| `VUIO_DB_PATH` | /data/vuio.redb | Database file path |
| `VUIO_MULTICAST_TTL` | 4 | Multicast TTL |
| `VUIO_ANNOUNCE_INTERVAL` | 30 | SSDP announce interval (seconds) |

**Find your host IP:**
```bash
# Linux
ip route get 1.1.1.1 | grep -oP 'src \K[0-9.]+'

# macOS
ipconfig getifaddr en0

# Windows
ipconfig | findstr "IPv4"
```

**Generate UUID for multiple instances:**
```bash
uuidgen  # Linux/macOS
[System.Guid]::NewGuid()  # Windows PowerShell
```

## Configuration

### Native (TOML Config)

VuIO uses TOML configuration files on native platforms. Config location: `./config/config.toml`

```toml
[server]
port = 8080
interface = "0.0.0.0"
name = "VuIO Server"
uuid = "auto-generated"

[network]
interface_selection = "Auto"
multicast_ttl = 4
announce_interval_seconds = 30

[media]
scan_on_startup = true
watch_for_changes = true
cleanup_deleted_files = true
scan_playlists = true
supported_extensions = ["mp4", "mkv", "avi", "mp3", "flac", "wav", "jpg", "png"]

[[media.directories]]
path = "/home/user/Videos"
recursive = true
extensions = ["mp4", "mkv", "avi", "mov", "wmv", "webm"]
exclude_patterns = ["*.tmp", ".*"]
validation_mode = "Warn"  # Strict, Warn, or Skip

[[media.directories]]
path = "/home/user/Music"
recursive = true
extensions = ["mp3", "flac", "wav", "aac", "ogg", "wma", "m4a", "opus"]
exclude_patterns = ["*.tmp", ".*"]

[[media.directories]]
path = "/home/user/Pictures"
recursive = true
extensions = ["jpg", "jpeg", "png", "gif", "bmp", "webp"]
exclude_patterns = ["*.tmp", ".*"]

[database]
path = "~/.local/share/vuio/media.redb"
vacuum_on_startup = false
backup_enabled = true
```

### Configuration Options

**Server:**
- `port` - HTTP server port
- `interface` - Network interface to bind (0.0.0.0 for all)
- `name` - DLNA server friendly name
- `uuid` - Device UUID (auto-generated if not set)
- `ip` - Specific IP for DLNA announcements (optional)

**Network:**
- `interface_selection` - "Auto", "All", or specific interface name
- `multicast_ttl` - Multicast time-to-live
- `announce_interval_seconds` - SSDP announcement interval

**Media:**
- `scan_on_startup` - Scan directories on startup
- `watch_for_changes` - Real-time file monitoring
- `cleanup_deleted_files` - Auto-remove deleted files from database
- `scan_playlists` - Import M3U/PLS playlist files
- `supported_extensions` - Global list of media extensions

**Media Directories:**
- `path` - Directory path
- `recursive` - Scan subdirectories
- `extensions` - Override extensions for this directory
- `exclude_patterns` - Patterns to exclude (e.g., "*.tmp", ".*")
- `validation_mode` - Path validation: "Strict" (fail if missing), "Warn" (log warning), "Skip" (no validation)

**Database:**
- `path` - Database file location
- `vacuum_on_startup` - Compact database on startup
- `backup_enabled` - Enable automatic backups

## Audio Features (ALPHA)

### Metadata Extraction

VuIO automatically extracts metadata from audio files:
- Title, Artist, Album, Album Artist
- Genre, Year, Track Number
- Duration
- Falls back to filename parsing when tags are missing

### Supported Audio Formats

- **Lossless:** FLAC, WAV, AIFF
- **Lossy:** MP3, AAC, OGG, WMA, OPUS, M4A

### Playlist Support

VuIO automatically discovers and imports playlist files:
- **M3U/M3U8** - Most common format
- **PLS** - WinAmp/iTunes compatible

Playlists are scanned from media directories on startup and made available to DLNA clients.

Configure: `scan_playlists = true` or `VUIO_SCAN_PLAYLISTS=true`

### Music Organization

Recommended directory structure:
```
/music/
├── Artist Name/
│   └── Album Name (Year)/
│       ├── 01 - Track.flac
│       └── folder.jpg
└── playlists/
    ├── favorites.m3u
    └── workout.pls
```

## Database

VuIO uses Redb, an embedded ACID-compliant database.

### Database Location

| Platform | Default Path |
|----------|--------------|
| **Windows** | `[exe dir]\config\database\media.redb` |
| **Linux** | `~/.local/share/vuio/media.redb` |
| **macOS** | `~/Library/Application Support/vuio/media.redb` |
| **Docker** | `/data/vuio.redb` (or `VUIO_DB_PATH`) |

When running from source on Windows:
```
C:\Users\Welcome\Downloads\code\rust\vuio\target\release\config\database\media.redb
```

### Reset Database

```bash
# Windows (PowerShell)
Remove-Item -Force .\target\release\config\database\media.redb

# Linux
rm -f ~/.local/share/vuio/media.redb

# macOS
rm -f ~/Library/Application\ Support/vuio/media.redb

# Docker
docker exec vuio-server rm -f /data/vuio.redb
```

VuIO will create a new database and rescan media on next startup.

## Platform Notes

### Windows
- Supports UNC paths (`\\server\share`)
- Auto-excludes `Thumbs.db`, `desktop.ini`

### macOS
- Supports network volumes under `/Volumes`
- Auto-excludes `.DS_Store`, `.AppleDouble`

### Linux
- Supports mounts under `/media`, `/mnt`
- Auto-excludes `lost+found`, `.Trash-*`

## Architecture

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
         │  │ Manager │  │ (Redb)  │  │ Abstraction │  │
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

## Logging & Diagnostics

VuIO is designed to run cleanly by default. Standard startup displays a clean, visual card containing crucial server information, while silencing verbose background execution traces.

### Default Log File

By default, all detailed logs (`INFO` level and below) are automatically recorded to a rolling background log file so that troubleshooting info is always preserved.

**Default Log Path:**
- **Windows**: `[exe dir]\config\logs\vuio.log`
- **macOS**: `[exe dir]/config/logs/vuio.log` (Native) or platform cache path
- **Linux**: `[exe dir]/config/logs/vuio.log`
- **Docker**: `/data/logs/vuio.log`

### Detailed Console Logs

If you want to view verbose background logs directly on the console, you can use either of the following approaches:

1. **Command Line Flag**:
   Run with `--debug` to enable verbose debug logs on the terminal:
   ```bash
   ./vuio --debug
   ```

2. **Environment Variable**:
   Set `RUST_LOG` env variable:
   ```bash
   RUST_LOG=info ./vuio
   RUST_LOG=debug ./vuio
   ```

### Custom Log Destinations and Levels

You can fully control where logs are written and their severity level using command line options:

- **Specify Custom Log File**:
  ```bash
  ./vuio --log-file /path/to/my-custom.log
  ```
- **Set Log Level**:
  ```bash
  ./vuio --log-level debug
  ./vuio --log-level warn
  ```

---

## Monitoring & Probes (HA & Kubernetes Native)

VuIO contains built-in endpoints optimized for Kubernetes orchestration and observability via Grafana, Prometheus, and Loki.

### Kubernetes Probes
- **Liveness Probe (`/healthz`)**: A lightweight endpoint indicating that the web server is running.
  - Returns: `200 OK` with JSON `{"status": "healthy"}`
- **Readiness Probe (`/readyz`)**: Verifies database connectivity and readiness to serve requests.
  - Returns: `200 OK` with JSON `{"status": "ready"}` if healthy, or `503 Service Unavailable` if database access fails.

### Metrics & Monitoring
To monitor the server health, cache efficiency, and indexing status, you can query the metrics endpoints:
- **Prometheus Exposition Format (`/metrics`)**: Returns raw metrics formatted for Prometheus.
  - Query: `curl http://localhost:8080/metrics`
  - Returns: `200 OK` with `text/plain` Prometheus exposition format.
- **JSON Format (`/metrics/json`)**: Returns JSON telemetry.
  - Query: `curl http://localhost:8080/metrics/json`
  - Returns: `200 OK` with JSON structure like:
    ```json
    {
      "web_handler_metrics": {
        "browse_requests": 12,
        "cache_hits": 9,
        "cache_misses": 3,
        "cache_hit_rate_percent": 75.0,
        "average_response_time_ms": 12,
        "gigabytes_transferred": 0.25,
        "redb_database": "active"
      }
    }
    ```

### DLNA Browse Caching
To support instant directory listings for directories containing 1000+ files, VuIO implements an automatic, thread-safe SOAP response cache:
- **How it works**: The cache stores the fully rendered XML response mapped to a unique signature of `(ObjectID, StartingIndex, RequestedCount, ClientProfile, UpdateID)`. Subsequent scrolls or refreshes from the TV/client are served in sub-milliseconds without hitting the database, resolving paths, or performing memory cloning.
- **Cache Invalidation**: The cache is automatically and immediately cleared whenever a filesystem change or directory scan increments the `UpdateID` counter, ensuring no stale data is ever served.

### Log Streaming (Grafana / Loki / Alloy)
- **Log Scraper Endpoint (`/logs`)**: Stream the last N log entries (default `100`, max `5000`) dynamically over HTTP. Useful for pull-based logs scraping.
  - Query: `curl http://localhost:8080/logs?limit=50`
  - Returns: `200 OK` with raw plaintext log lines.

---

## AI Agent & MCP Integration

VuIO supports the **Model Context Protocol (MCP)**, allowing AI agents (like voice assistants, chatbots, and autonomous agents) to interact with your media library and control playback on smart TVs on the local network.

### Transport Protocols

The MCP server is served over **SSE (Server-Sent Events)** on the existing HTTP port:
- **Establish SSE Session**: `GET http://localhost:8080/sse`
  - When connected, the server will yield an initial `endpoint` event containing the POST message target, e.g. `data: /mcp/message?client_id=<uuid>`
- **Post Messages**: `POST http://localhost:8080/mcp/message?client_id=<uuid>`
  - Used to send standard MCP JSON-RPC 2.0 messages to the server.

### Available MCP Tools

| Tool Name | Parameters | Description |
| :--- | :--- | :--- |
| `search_media` | `query` (string) | Search media files by keyword matching filenames or tags |
| `browse_folder` | `path` (string), `category` (optional string) | Browse files and directories in a specific folder path |
| `get_media_info` | `file_id` (integer) | Fetch detailed metadata for a file by its ID |
| `get_server_stats` | None | Retrieve media counts, library size, and server URL info |
| `list_tvs` | None | Scan the local network for DLNA/UPnP MediaRenderer TVs |
| `cast_media_to_tv` | `file_id` (integer), `tv_name` (string) | Start playing a media file on a discovered smart TV |
| `control_tv` | `tv_name` (string), `action` ("play"\|"pause"\|"stop") | Send playback control commands to a smart TV |
| `list_media` | `category` (optional string), `limit` (optional integer) | Retrieve a flat list of indexed media files (all, audio, video, image) |
| `list_playlists` | None | List all playlists stored on the server |
| `create_playlist` | `name` (string), `description` (optional string) | Create a new media playlist |
| `delete_playlist` | `playlist_id` (integer) | Delete a playlist by ID |
| `add_to_playlist` | `playlist_id` (integer), `media_file_ids` (integer[]) | Add multiple tracks in bulk to a playlist |
| `remove_from_playlist` | `playlist_id` (integer), `media_file_id` (integer) | Remove a specific track from a playlist |
| `get_playlist_tracks` | `playlist_id` (integer) | Retrieve all media files/tracks in a specific playlist |
| `cast_playlist_to_tv` | `playlist_id` (integer), `tv_name` (string) | Cast a playlist to a local smart TV and start playing it |

### Example Usage

1. **Discover TVs**:
   ```bash
   curl -X POST "http://localhost:8080/mcp/message?client_id=agent-1" \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"list_tvs","arguments":{}}}'
   ```

2. **Search for Media**:
   ```bash
   curl -X POST "http://localhost:8080/mcp/message?client_id=agent-1" \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","method":"tools/call","id":2,"params":{"name":"search_media","arguments":{"query":"matrix"}}}'
   ```

3. **Cast to Bedroom TV**:
   ```bash
   curl -X POST "http://localhost:8080/mcp/message?client_id=agent-1" \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","method":"tools/call","id":3,"params":{"name":"cast_media_to_tv","arguments":{"file_id":42,"tv_name":"bedroom"}}}'
   ```

---

## Testing

```bash
cargo test
```

## Contributing

Contributions welcome! Please ensure cross-platform compatibility is maintained.
Input license = output license

## License

- [MIT License](LICENSE-MIT)
or
- [Apache License 2.0](LICENSE-APACHE)

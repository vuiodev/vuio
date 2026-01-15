# VuIO Media Server

A cross-platform DLNA/UPnP media server written in Rust. Streams video, audio, and images to any DLNA-compatible device (smart TVs, receivers, game consoles).

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
- **HTTP Range Streaming** - Seek support for large media files
- **Multi-format Support** - MKV, MP4, AVI, MP3, FLAC, WAV, AAC, OGG, JPEG, PNG, and more
- **Audio Metadata** - Automatic extraction of artist, album, genre, year from tags
- **Music Browsing** - Browse by Artists, Albums, Genres, Years via DLNA
- **Playlist Support** - Auto-imports M3U/PLS playlists from media directories
- **Real-time Monitoring** - Detects file changes and updates database automatically
- **Cross-platform** - Native integration for Windows, macOS, Linux
- **Redb Database** - Embedded ACID-compliant database with crash recovery

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

## Audio Features

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

## Testing

```bash
cargo test
```

Debug logging:
```bash
RUST_LOG=debug ./vuio 2>&1 | tee vuio-debug.log
```

## Contributing

Contributions welcome! Please ensure cross-platform compatibility is maintained.
DCO is required for contributions.

## License

Dual-licensed under either:
- [MIT License](LICENSE-MIT)
or
- [Apache License 2.0](LICENSE-APACHE)
at your choise.

# VuIO Media Server

A comprehensive, cross-platform DLNA/UPnP media server written in Rust with advanced platform integration, real-time file monitoring, and robust database management. Built with Axum, Tokio, and SQLite for high performance and reliability.

Windows, Linux, macOS, and Docker fully supported
x64 and ARM64 supported

```bash
git clone https://github.com/vuiodev/vuio.git
cd vuio
docker-compose -f docker-compose.yml up
```

## 🚀 Features

### Core DLNA/UPnP Functionality
- **Full DLNA/UPnP Media Server** - Streams video, audio, and image files to any DLNA-compatible device
- **SSDP Discovery** - Automatic device discovery with platform-optimized networking
- **HTTP Range Streaming** - Efficient streaming with seek support for large media files
- **Dynamic XML Generation** - Standards-compliant device and service descriptions
- **Multi-format Support** - Handles MKV, MP4, AVI, MP3, FLAC, WAV, AAC, OGG, JPEG, PNG, and many more formats
- **Read-Only Media Support** - Works seamlessly with read-only file systems and network storage

### 🎵 Advanced Audio Features
- **Rich Metadata Extraction** - Automatic extraction of title, artist, album, genre, track number, year, and album artist from audio files
- **Music Categorization** - Browse music by Artists, Albums, Genres, Years, and Album Artists with track counts
- **Playlist Management** - Create, edit, and manage playlists with support for M3U and PLS formats
- **Audio Format Support** - MP3, FLAC, WAV, AAC, OGG, WMA, M4A, OPUS, AIFF with full metadata
- **Smart Music Organization** - Automatic categorization and sorting of music collections
- **DLNA Audio Browsing** - Professional-grade music browsing experience for DLNA clients
- **Playlist Import/Export** - Import existing playlists and export to standard formats
- **Track Management** - Add, remove, and reorder tracks in playlists with position management

### Cross-Platform Integration
- **Windows Support** - Native networking and filesystem integration
- **macOS Support** - Native networking and filesystem integration
- **Linux Support** - Native networking and filesystem integration
- **Platform-Specific Optimizations** - Optimized networking and filesystem handling

### Advanced Database Management
- **SQLite Database** - Persistent media library with metadata caching
- **Health Monitoring** - Automatic integrity checks and repair capabilities
- **Backup System** - Automated backups with cleanup and restoration
- **Performance Optimization** - Database vacuuming and query optimization

### Real-Time File Monitoring
- **Cross-Platform File Watching** - Real-time detection of media file changes
- **Incremental Updates** - Efficient database synchronization on file system changes
- **Smart Filtering** - Platform-specific exclude patterns and media type detection
- **Batch Processing** - Optimized handling of bulk file operations

### Configuration & Management
- **Hot Configuration Reload** - Runtime configuration updates without restart
- **Platform-Aware Defaults** - Intelligent defaults based on operating system
- **TOML Configuration** - Human-readable configuration with comprehensive validation
- **Multiple Media Directories** - Support for monitoring multiple locations

## 🛠️ Installation & Usage

### Prerequisites
- Rust 1.75+ (for building from source)
- SQLite 3.x (bundled with the application)

### Build from Source
```bash
git clone https://github.com/vuiodev/vuio.git
cd vuio
cargo build --release
```

### Docker

Docker will not work on MacOS due to the lack of a compatible multicast implementation.

#### Quick Start with Docker Compose

```bash
# Clone the repository
git clone https://github.com/vuiodev/vuio.git
cd vuio

# Run from remote repo
docker-compose -f docker-compose.yml up

# Build and run with Docker Compose
docker-compose -f docker-compose.local.yml up --build
```

#### Docker Environment Variables

**Essential Configuration:**
```bash
# Server Configuration
VUIO_PORT=8080                                        # HTTP server port
VUIO_SERVER_NAME="VuIO DLNA Server"                   # DLNA server name
VUIO_INTERFACE=0.0.0.0                               # Network interface to bind
VUIO_UUID=550e8400-e29b-41d4-a716-446655440000       # Fixed UUID (CHANGE THIS for multiple instances)

# CRITICAL: Set your host IP for DLNA announcements
VUIO_IP=192.168.1.126                                # Replace with YOUR host IP address

# Media Configuration
VUIO_MEDIA_DIRS=/media                               # Single directory
# VUIO_MEDIA_DIRS=/media/movies,/media/tv,/media/music # Multiple directories (comma-separated)
VUIO_SCAN_ON_STARTUP=true                           # Scan media on startup
VUIO_WATCH_CHANGES=true                             # Enable file system monitoring
VUIO_CLEANUP_DELETED=true                           # Remove deleted files from database

# Network Configuration
VUIO_MULTICAST_TTL=4                                # Multicast TTL
VUIO_ANNOUNCE_INTERVAL=300                          # SSDP announcement interval (seconds)

# Database Configuration
VUIO_DB_PATH=/data/vuio.db                          # Database file path
VUIO_DB_VACUUM=false                                # Vacuum database on startup
VUIO_DB_BACKUP=true                                 # Enable database backups

# ZeroCopy Database Configuration (optional - uses minimal defaults)
ZEROCOPY_CACHE_MB=2                                 # Memory cache size (1-1024 MB, default: 1)
ZEROCOPY_INDEX_SIZE=5000                            # Index cache entries (100-10M, default: 1000)
ZEROCOPY_BATCH_SIZE=500                             # Batch processing size (10-1M, default: 100)
ZEROCOPY_INITIAL_FILE_SIZE_MB=2                     # Initial DB file size (1-1024 MB, default: 1)
ZEROCOPY_MAX_FILE_SIZE_GB=2                         # Max DB file size (1-100 GB, default: 1)
ZEROCOPY_SYNC_FREQUENCY_SECS=120                    # Sync frequency (1-3600 seconds, default: 60)
ZEROCOPY_ENABLE_WAL=true                            # Enable Write-Ahead Logging (default: false)
ZEROCOPY_ENABLE_COMPRESSION=false                   # Enable compression (default: false)
ZEROCOPY_MONITOR_INTERVAL_SECS=900                  # Performance monitoring (30-3600s, default: 600)

# Debugging
RUST_LOG=debug                                      # Enable debug logging
```

**Generate a unique UUID for multiple instances:**
```bash
# On macOS/Linux
uuidgen

# On Windows (PowerShell)
[System.Guid]::NewGuid()

# Or use online UUID generators
```

#### Finding Your Host IP Address

**Linux:**
```bash
# Method 1: Using ip command (Linux)
ip route get 1.1.1.1 | grep -oP 'src \K[0-9.]+'

# Method 2: Using ifconfig
ifconfig | grep 'inet ' | grep -v '127.0.0.1' | head -1 | awk '{print $2}'

# Method 3: Using hostname
hostname -I | awk '{print $1}'
```

**Windows:**
```cmd
# Using ipconfig
ipconfig | findstr "IPv4"
```

#### Docker Volume Mounting for Multiple Media Directories

**Single Media Directory:**
```yaml
volumes:
  - ./test-media:/media
```

**Multiple Media Directories:**
```yaml
volumes:
  # Mount each directory separately
  - /path/to/movies:/media/movies
  - /path/to/tv-shows:/media/tv
  - /path/to/music:/media/music
  
# Then configure via environment variable:
environment:
  - VUIO_MEDIA_DIRS=/media/movies,/media/tv,/media/music
```

**Network Storage (NFS/SMB):**
```yaml
volumes:
  - type: bind
    source: /mnt/nas/media
    target: /media
```

#### Docker Compose Configuration

**Key Settings for DLNA:**
- `network_mode: host` - **Required** for multicast/DLNA discovery
- `cap_add: [NET_ADMIN, NET_RAW]` - Enhanced networking capabilities
- `VUIO_SERVER_IP` - **Must match your host IP address**

```yaml
services:
  vuio:
    build:
      context: .
      dockerfile: Dockerfile
    container_name: vuio-server-local
    restart: unless-stopped
    network_mode: host  # REQUIRED for DLNA
    
    cap_add:
      - NET_ADMIN
      - NET_RAW
    
    volumes:
      - ./vuio-config:/config
      # Recommended: Mount media as read-only for security
      - /path/to/your/media:/media:ro
      
    environment:
      - VUIO_IP=192.168.1.126         # YOUR HOST IP HERE
      - VUIO_PORT=8080
      - VUIO_MEDIA_DIRS=/media
      - VUIO_SERVER_NAME=VuIO
      - VUIO_DB_PATH=/data/vuio.db
      - PUID=1000
      - PGID=1000
```

#### Volume Mapping

**Read-Only Media Support (Recommended):**
VuIO enforces read-only access to all media directories at the application level. This provides several benefits:
- Prevents any modification of your media files
- Works with network storage that may be mounted read-only
- Eliminates permission issues with Docker containers
- Provides better security by design

```bash
# Media directories (application enforces read-only access)
/path/to/your/media:/media

# Configuration and database persistence (read-write required)
./vuio-config:/config
```

**Multiple Directory Examples:**
```yaml
volumes:
  # Configuration and database (read-write)
  - ./vuio-config:/config
  
  # Media directories (read-only recommended)
  - /home/user/Movies:/media/movies:ro
  - /home/user/TV-Shows:/media/tv:ro
  - /home/user/Music:/media/music:ro
  - /home/user/Pictures:/media/pictures:ro
  
  # Network storage example
  - /mnt/nas/media:/media/nas:ro
```

#### Docker Run Command

```bash
# Recommended: Read-only media mount
docker run -d \
  --name vuio-server \
  --restart unless-stopped \
  --network host \
  --cap-add NET_ADMIN \
  --cap-add NET_RAW \
  -v /path/to/your/media:/media:ro \
  -v ./vuio-config:/config \
  -e VUIO_IP=192.168.1.126 \
  -e VUIO_PORT=8080 \
  -e VUIO_MEDIA_DIRS=/media \
  -e VUIO_DB_PATH=/data/vuio.db \
  -e PUID=1000 \
  -e PGID=1000 \
  vuio:latest

# Multiple read-only media directories
docker run -d \
  --name vuio-server \
  --restart unless-stopped \
  --network host \
  --cap-add NET_ADMIN \
  --cap-add NET_RAW \
  -v /path/to/movies:/media/movies:ro \
  -v /path/to/music:/media/music:ro \
  -v /path/to/pictures:/media/pictures:ro \
  -v ./vuio-config:/config \
  -e VUIO_IP=192.168.1.126 \
  -e VUIO_PORT=8080 \
  -e VUIO_MEDIA_DIRS=/media \
  -e VUIO_DB_PATH=/data/vuio.db \
  -e PUID=1000 \
  -e PGID=1000 \
  vuio:latest
```

### Quick Start
```bash
# Run with default settings (scans ~/Videos, ~/Music, ~/Pictures)
./target/release/vuio

# Specify a custom media directory
./target/release/vuio /path/to/your/media

# Custom port and server name
./target/release/vuio -p 9090 -n "My Media Server" /path/to/media
```

### Command Line Options
```
Usage: vuio [OPTIONS] [MEDIA_DIR]

Arguments:
  [MEDIA_DIR]  The directory containing media files to serve

Options:
  -p, --port <PORT>        The network port to listen on [default: 8080]
  -n, --name <NAME>        The friendly name for the DLNA server [default: platform-specific]
  -c, --config <CONFIG>    Path to configuration file
      --media-dir <DIR>    Additional media directories (can be used multiple times)
      --debug              Enable debug logging
  -h, --help               Print help information
  -V, --version            Print version information
```

### Multiple Media Directories

You can serve media from multiple directories using:

```bash
# Single directory
./vuio /path/to/movies

# Multiple directories
./vuio /path/to/movies --media-dir /path/to/music --media-dir /path/to/photos

# Only additional directories (no primary)
./vuio --media-dir /raid1/movies --media-dir /raid2/music --media-dir /nas/photos

# Mixed with other options
./vuio -p 9090 -n "My Media Server" /primary/media --media-dir /secondary/media
```

## ⚙️ Configuration

VuIO supports two configuration modes:

### Docker Configuration (Environment Variables Only)
When running in Docker, VuIO automatically detects the container environment and uses **only environment variables** for configuration. No config files are read or written.

**Required Environment Variables:**
```bash
# Server Configuration
VUIO_PORT=8080                     # HTTP server port (default: 8080)
VUIO_INTERFACE=0.0.0.0             # Network interface to bind (default: 0.0.0.0)
VUIO_SERVER_NAME=VuIO              # DLNA server name (default: VuIO)
VUIO_UUID=550e8400-e29b-41d4-a716-446655440000  # DLNA device UUID (CHANGE THIS if running multiple instances)
VUIO_IP=192.168.1.126              # Optional: Specific IP for DLNA announcements

# Media Configuration
VUIO_MEDIA_DIRS="/media,/movies,/music" # Comma-separated media directories (default: "/media")
VUIO_SCAN_ON_STARTUP=true          # Scan media on startup (default: true)
VUIO_WATCH_CHANGES=true            # Enable file system monitoring (default: true)
VUIO_CLEANUP_DELETED=true          # Remove deleted files from database (default: true)

# Network Configuration
VUIO_MULTICAST_TTL=4               # Multicast TTL (default: 4)
VUIO_ANNOUNCE_INTERVAL=30          # SSDP announcement interval in seconds (default: 30)

# Database Configuration
VUIO_DB_PATH=/data/vuio.db         # Database file path (default: "/data/vuio.db")
VUIO_DB_VACUUM=false               # Vacuum database on startup (default: false)
VUIO_DB_BACKUP=false               # Enable database backups (default: false)
```

**About UUID:** The UUID is required by the DLNA/UPnP specification to uniquely identify your media server on the network. It's used in SSDP announcements and device descriptions. If not provided, VuIO generates a random UUID on startup, but for consistency across container restarts, you should set a fixed UUID. **Important:** If running multiple VuIO instances on the same LAN, each must have a unique UUID to avoid conflicts.

**Generate a unique UUID:**
```bash
# On macOS/Linux
uuidgen

# On Windows (PowerShell)
[System.Guid]::NewGuid()

# Or use online UUID generators
```

### Native Platform Configuration (Config Files)
When running natively (Windows, macOS, Linux), VuIO uses TOML configuration files with platform-specific defaults:

**Configuration Locations:**
- **Native Apps:** `.\config\config.toml`
- **Docker:** `/config/config.toml`

**Multiple Media Directories:**
You can monitor multiple directories by adding multiple `[[media.directories]]` sections to your configuration file. Each directory can have its own settings for recursion, file extensions, and exclude patterns.

**Network Interface Selection:**
- `"Auto"` - Automatically select the best interface
- `"All"` - Use all available interfaces  
- `"eth0"` - Use a specific interface name (replace with actual interface name)

**Media Configuration Options:**
- `scan_on_startup` - Whether to scan all media directories on startup (default: true)
- `watch_for_changes` - Enable real-time file system monitoring (default: true)
- `cleanup_deleted_files` - Remove deleted files from database automatically (default: true)
- `supported_extensions` - Global list of supported media file extensions
- Individual directories can override `extensions` and `exclude_patterns`

### Example Configuration
```toml
[server]
port = 8080
interface = "0.0.0.0"
name = "VuIO Server"
uuid = "auto-generated"
# ip = "192.168.1.100"  # Optional: Set specific IP for DLNA announcements

[network]
ssdp_port = 1900
interface_selection = "Auto"
multicast_ttl = 4
announce_interval_seconds = 30

[media]
scan_on_startup = true
watch_for_changes = true
cleanup_deleted_files = true
supported_extensions = ["mp4", "mkv", "avi", "mp3", "flac", "wav", "aac", "ogg", "jpg", "jpeg", "png"]

# Video directory
[[media.directories]]
path = "/home/user/Videos"
recursive = true
extensions = ["mp4", "mkv", "avi", "mov", "wmv"]
exclude_patterns = ["*.tmp", ".*"]

# Music directory with audio-specific extensions
[[media.directories]]
path = "/home/user/Music"
recursive = true
extensions = ["mp3", "flac", "wav", "aac", "ogg", "wma", "m4a", "opus"]
exclude_patterns = ["*.tmp", ".*", "*.m3u", "*.pls"]  
# Exclude playlist files from scanning

# Photos directory
[[media.directories]]
path = "/home/user/Pictures"
recursive = true
extensions = ["jpg", "jpeg", "png", "gif", "bmp"]
exclude_patterns = ["*.tmp", ".*"]

[database]
path = "~/.local/share/vuio/media.db"
vacuum_on_startup = false
backup_enabled = true
```

## 🎵 Audio Features & Music Management

VuIO provides professional-grade audio features that rival commercial media servers like Plex or Emby, with comprehensive music organization and playlist management.

### Audio Metadata Support

**Automatic Metadata Extraction:**
- **Title, Artist, Album** - Extracted from ID3 tags and other metadata formats
- **Genre, Year, Track Number** - Complete album and track information
- **Album Artist** - Proper handling of compilation albums and various artists
- **Duration** - Accurate playback time for seeking and display
- **Fallback Parsing** - Intelligent filename parsing when metadata is missing

**Supported Audio Formats:**
- **Lossless:** FLAC, WAV, AIFF, APE
- **Lossy:** MP3, AAC, OGG Vorbis, WMA, OPUS
- **Apple:** M4A, M4P (iTunes), M4B (audiobooks)
- **Platform-specific:** ASF, WM (Windows Media)

### Music Categorization & Browsing

VuIO organizes your music collection into intuitive categories accessible through any DLNA client:

### Playlist Management

**Playlist Creation & Management:**
- Create custom playlists through the web API
- Add and remove tracks dynamically
- Reorder tracks with position management
- Delete and modify existing playlists

**Playlist Import & Export:**
```bash
# Supported formats
- M3U/M3U8 playlists (most common)
- PLS playlists (WinAmp/iTunes compatible)

# Import existing playlists
POST /api/playlists/import

# Export playlists
GET /api/playlists/{id}/export?format=m3u
GET /api/playlists/{id}/export?format=pls
```

**Directory Playlist Scanning:**
- Automatically discover existing playlist files
- Import multiple playlists from directory
- Maintain playlist metadata and descriptions

### Web API for Audio Management

**Playlist Operations:**
```bash
# List all playlists
GET /api/playlists

# Create a new playlist
POST /api/playlists
{
  "name": "My Favorite Songs",
  "description": "A collection of my favorite tracks"
}

# Add track to playlist
POST /api/playlists/{id}/tracks
{
  "media_file_id": 123,
  "position": 1
}

# Import playlist file
POST /api/playlists/import
# (multipart/form-data with playlist file)

# Export playlist
GET /api/playlists/{id}/export?format=m3u

# Scan directory for playlists
POST /api/playlists/scan
{
  "directory": "/path/to/playlists"
}
```

### Music Library Organization

**Best Practices:**
```bash
# Recommended directory structure
/music/
├── Artist Name/
│   ├── Album Name (Year)/
│   │   ├── 01 - Track Name.flac
│   │   ├── 02 - Track Name.flac
│   │   └── folder.jpg
│   └── Another Album/
└── Various Artists/
    └── Compilation Album/
        ├── 01 - Artist - Track.mp3
        └── 02 - Artist - Track.mp3

# Playlist storage
/music/playlists/
├── favorites.m3u
├── workout.pls
└── jazz_collection.m3u8
```

**Metadata Tips:**
- Use proper ID3v2.4 tags for MP3 files
- Ensure consistent artist naming (avoid "Artist" vs "The Artist")
- Use "Album Artist" tag for compilations
- Include cover art as embedded metadata or folder.jpg
- Set appropriate genre tags for better categorization

**Configuration for Large Music Libraries:**
```toml
[media]
# Enable for faster startup with large collections
scan_on_startup = false
watch_for_changes = true
cleanup_deleted_files = true

[database]
# Enable database optimization for large libraries
vacuum_on_startup = true
backup_enabled = true

[[media.directories]]
path = "/music"
recursive = true
# Audio-specific extensions
extensions = ["mp3", "flac", "wav", "aac", "ogg", "wma", "m4a", "opus", "aiff"]
# Exclude temporary and playlist files
exclude_patterns = ["*.tmp", ".*", "*.m3u", "*.pls", "*.log"]
```

## 🔧 Platform-Specific Notes

### Windows
- Supports UNC paths (`\\server\share`)
- Excludes `Thumbs.db` and `desktop.ini` files automatically

### macOS
- Supports network mounted volumes
- Excludes `.DS_Store` and `.AppleDouble` files automatically

### Linux
- Supports mounted filesystems under `/media` and `/mnt`
- Excludes `lost+found` and `.Trash-*` directories automatically

## 🏗️ Architecture

VuIO is built with a modular, cross-platform architecture:

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Web Server    │    │  SSDP Service   │    │ File Watcher    │
│   (Axum/HTTP)   │    │  (Discovery)    │    │ (Real-time)     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
         ┌─────────────────────────────────────────────────────┐
         │              Application Core                       │
         │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │
         │  │   Config    │  │  Database   │  │  Platform   │  │
         │  │  Manager    │  │  Manager    │  │ Abstraction │  │
         │  └─────────────┘  └─────────────┘  └─────────────┘  │
         └─────────────────────────────────────────────────────┘
                                 │
         ┌─────────────────────────────────────────────────────┐
         │            Platform Layer                           │
         │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  |
         │  │   Windows   │  │    macOS    │  │    Linux    │  │
         │  │ Integration │  │ Integration │  │ Integration │  │
         │  └─────────────┘  └─────────────┘  └─────────────┘  │
         └─────────────────────────────────────────────────────┘
```

## 🧪 Testing

```bash
cargo test
```

### Diagnostic Information

Generate a diagnostic report:
```bash
RUST_LOG=debug ./vuio 2>&1 | tee vuio-debug.log
```

## 🤝 Contributing

Contributions are welcome! Please read our contributing guidelines and ensure:

Cross-platform compatibility is maintained

## 📄 License

This project is licensed under the [Apache License 2.0](LICENSE).
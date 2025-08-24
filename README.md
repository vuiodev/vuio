# VuIO Media Server

A comprehensive, cross-platform DLNA/UPnP media server written in Rust with advanced platform integration, real-time file monitoring, and robust database management. Built with Axum, Tokio, and SQLite for high performance and reliability.

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
- **Windows Support** - UAC integration, Windows Firewall detection, Windows Defender awareness
- **macOS Support** - Gatekeeper integration, SIP detection, Application Firewall handling
- **Linux Support** - SELinux/AppArmor awareness, capabilities management, firewall detection
- **Platform-Specific Optimizations** - Tailored networking, file system, and security handling

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

### Security & Permissions
- **Security Checks** - Platform-specific privilege and permission validation
- **Secure Defaults** - Minimal privilege operation with graceful degradation
- **Firewall Integration** - Automatic detection and guidance for network access
- **Permission Management** - Proper handling of file system and network permissions

### Diagnostics & Monitoring
- **Comprehensive Diagnostics** - Detailed system and platform information
- **Startup Validation** - Pre-flight checks for optimal operation
- **Network Analysis** - Interface detection and connectivity testing
- **Performance Monitoring** - Resource usage and health metrics

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

# Build and run with Docker Compose
docker-compose -f docker-compose.local.yml up --build
```

#### Docker Environment Variables

**Essential Configuration:**
```bash
# Server Configuration
VUIO_PORT=8080                    # HTTP server port
VUIO_SERVER_NAME="VuIO"           # DLNA server name
VUIO_BIND_INTERFACE=0.0.0.0       # Network interface to bind

# CRITICAL: Set your host IP for DLNA announcements
VUIO_SERVER_IP=192.168.1.126      # Replace with YOUR host IP address

# Media Configuration
VUIO_MEDIA_DIR=/media              # Media directory inside container

# SSDP Configuration (if port 1900 conflicts)
VUIO_SSDP_PORT=1902               # Alternative SSDP port

# User/Group Mapping
PUID=1000                          # Your user ID (run 'id -u')
PGID=1000                          # Your group ID (run 'id -g')

# Debugging
RUST_LOG=debug                     # Enable debug logging
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
      - VUIO_SERVER_IP=192.168.1.126  # YOUR HOST IP HERE
      - VUIO_PORT=8080
      - VUIO_MEDIA_DIR=/media
      - VUIO_SERVER_NAME=VuIO
      - PUID=1000
      - PGID=1000
```

#### Volume Mapping

**Read-Only Media Support (Recommended):**
VuIO fully supports read-only media directories, which is the recommended approach for production deployments. This provides several benefits:
- Prevents accidental modification of your media files
- Works with network storage that may be mounted read-only
- Eliminates permission issues with Docker containers
- Provides better security by reducing write access

```bash
# Recommended: Mount media as read-only
/path/to/your/media:/media:ro

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

**Legacy Read-Write Mounting:**
```bash
# Only use read-write if you need the server to modify files
/home/user/Videos:/media/videos
/home/user/Music:/media/music
/home/user/Pictures:/media/pictures
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
  -e VUIO_SERVER_IP=192.168.1.126 \
  -e VUIO_PORT=8080 \
  -e VUIO_MEDIA_DIR=/media \
  -e VUIO_SERVER_NAME="My DLNA Server" \
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
  -e VUIO_SERVER_IP=192.168.1.126 \
  -e VUIO_PORT=8080 \
  -e VUIO_MEDIA_DIR=/media \
  -e VUIO_SERVER_NAME="My DLNA Server" \
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

VuIO uses a TOML configuration file with platform-specific defaults:

**Configuration Locations:**
- **Windows:** `%APPDATA%\VuIO\config.toml`
- **macOS:** `~/Library/Application Support/VuIO/config.toml`
- **Linux:** `~/.config/vuio/config.toml`

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

**Browse by Artists:**
```
Audio > Artists > [Artist Name] > [All Tracks by Artist]
```
- Lists all unique artists with track counts
- Shows all tracks by selected artist, sorted by album and track number
- Supports various artists and featured artist metadata

**Browse by Albums:**
```
Audio > Albums > [Album Name] > [Album Tracks]
```
- Lists all albums with track counts
- Can be filtered by specific artist
- Proper track ordering by track number

**Browse by Genres:**
```
Audio > Genres > [Genre] > [All Tracks in Genre]
```
- Automatic genre categorization from metadata
- Supports multiple genres per track
- Custom genre organization

**Browse by Years:**
```
Audio > Years > [Year] > [All Tracks from Year]
```
- Organizes music by release year
- Perfect for exploring music by era
- Handles albums spanning multiple years

**Browse by Album Artists:**
```
Audio > Album Artists > [Album Artist] > [All Albums]
```
- Proper handling of compilation albums
- Separates album artists from track artists
- Ideal for classical and soundtrack collections

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

### DLNA Client Compatibility

**Tested DLNA Clients:**
- **VLC Media Player** - Full audio browsing and playback
- **Kodi/XBMC** - Complete music library integration
- **Windows Media Player** - Native Windows DLNA support
- **BubbleUPnP (Android)** - Advanced mobile music browsing
- **Hi-Fi Cast (iOS)** - Premium iOS audio streaming
- **Smart TVs** - Samsung, LG, Sony, and other DLNA-enabled TVs

**Audio Streaming Features:**
- **Gapless Playback** - Seamless album listening experience
- **HTTP Range Requests** - Efficient seeking and resume
- **Multiple Bitrates** - Automatic quality selection

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
- Administrator privileges may be required for ports < 1024
- Windows Firewall may prompt for network access
- Supports UNC paths (`\\server\share`)
- Excludes `Thumbs.db` and `desktop.ini` files automatically

### macOS
- System may prompt for network access permissions
- Supports network mounted volumes
- Excludes `.DS_Store` and `.AppleDouble` files automatically
- Gatekeeper and SIP integration for enhanced security

### Linux
- Root privileges required for ports < 1024 (or use capabilities)
- SELinux/AppArmor policies may affect file access
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
         │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │
         │  │   Windows   │  │    macOS    │  │    Linux    │  │
         │  │ Integration │  │ Integration │  │ Integration │  │
         │  └─────────────┘  └─────────────┘  └─────────────┘  │
         └─────────────────────────────────────────────────────┘
```

## 🧪 Testing

Run the comprehensive test suite:

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test modules
cargo test platform::tests
cargo test database::tests
cargo test config::tests
```

**Test Coverage:**
- ✅ 91 tests passing (including audio features)
- ✅ Platform detection and capabilities
- ✅ Database operations and health checks
- ✅ Configuration management and validation
- ✅ File system monitoring and events
- ✅ Network interface detection
- ✅ SSDP socket creation and binding
- ✅ Media file scanning and metadata
- ✅ Audio metadata extraction and categorization
- ✅ Playlist management (M3U/PLS import/export)
- ✅ Music categorization and browsing
- ✅ Error handling and recovery

## 🐛 Troubleshooting

### Common Issues

**Read-Only File System Errors (Docker)**
If you see errors like `chown: Read-only file system`, this is normal when mounting media directories as read-only:
- ✅ **Recommended**: Mount media directories as read-only (`:ro`) for security
- ✅ The application will detect and handle read-only mounts gracefully
- ✅ Only the `/config` directory needs write access for database and configuration
- ⚠️ Warnings about read-only media directories can be safely ignored

```yaml
# Correct Docker Compose volume configuration
volumes:
  - ./vuio-config:/config          # Read-write for database
  - /path/to/media:/media:ro       # Read-only for media files
```

**Port Already in Use**
```bash
# Check what's using the port
netstat -tulpn | grep :8080  # Linux
netstat -an | grep :8080     # macOS/Windows

# Use a different port
./vuio -p 9090
```

**Permission Denied**
```bash
# Linux: Use capabilities instead of root
sudo setcap 'cap_net_bind_service=+ep' ./target/release/vuio

# Or run on unprivileged port
./vuio -p 8080
```

**No Media Files Found**
- Check directory permissions
- Verify supported file extensions
- Review exclude patterns in configuration
- Check platform-specific file system case sensitivity

**DLNA Clients Can't Find Server**
- ✅ **Docker Users**: The application now works perfectly with Docker host networking mode
- Verify firewall settings
- Check multicast support on network interface  
- Ensure SSDP port (1900) is not blocked
- Try specifying network interface in configuration
- For Docker: Use `network_mode: host` for full multicast support

**Audio Files Not Showing Metadata**
- Verify audio files have embedded ID3 tags or metadata
- Check that file extensions are included in `supported_extensions`
- Enable debug logging to see metadata extraction attempts: `RUST_LOG=debug`
- Supported metadata formats: ID3v1/v2, Vorbis Comments, APE tags, MP4 metadata
- For files without metadata, titles will be extracted from filenames

**Music Categories Are Empty**
- Ensure audio files have proper artist/album/genre metadata
- Check that the media directory scanning completed successfully
- Verify database contains audio files: look for `mime_type LIKE 'audio/%'` entries
- Re-scan the media directory if metadata was added after initial scan

**Playlists Not Importing**
- Verify playlist files are in M3U or PLS format
- Check that file paths in playlists point to actual media files
- Ensure playlist files are not excluded by `exclude_patterns`
- Use absolute paths in playlist files for best compatibility

**Poor DLNA Audio Performance**
- Enable database vacuuming for large music libraries: `vacuum_on_startup = true`
- Use read-only media mounts to improve Docker performance
- Consider disabling `scan_on_startup` for very large collections
- Monitor database size and consider periodic cleanup

### Diagnostic Information

Generate a diagnostic report:
```bash
RUST_LOG=debug ./vuio 2>&1 | tee vuio-debug.log
```

The application provides comprehensive startup diagnostics including:
- Platform capabilities and limitations
- Network interface analysis
- Port availability testing
- File system permissions
- Database health status

## 🤝 Contributing

Contributions are welcome! Please read our contributing guidelines and ensure:

Cross-platform compatibility is maintained

## 📄 License

This project is licensed under the [Apache License 2.0](LICENSE).
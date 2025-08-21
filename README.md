# VuIO Media Server

A comprehensive, cross-platform DLNA/UPnP media server written in Rust with advanced platform integration, real-time file monitoring, and robust database management. Built with Axum, Tokio, and SQLite for high performance and reliability.

## 🚀 Features

### Core DLNA/UPnP Functionality
- **Full DLNA/UPnP Media Server** - Streams video, audio, and image files to any DLNA-compatible device
- **SSDP Discovery** - Automatic device discovery with platform-optimized networking
- **HTTP Range Streaming** - Efficient streaming with seek support for large media files
- **Dynamic XML Generation** - Standards-compliant device and service descriptions
- **Multi-format Support** - Handles MKV, MP4, AVI, MP3, FLAC, JPEG, PNG, and many more formats

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
- Rust 1.70+ (for building from source)
- SQLite 3.x (bundled with the application)

### Build from Source
```bash
git clone https://github.com/vuio/vuio.git
cd vuio
cargo build --release
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
  -p, --port <PORT>    The network port to listen on [default: 8080]
  -n, --name <NAME>    The friendly name for the DLNA server [default: platform-specific]
  -h, --help           Print help information
  -V, --version        Print version information
```

## ⚙️ Configuration

VuIO uses a TOML configuration file with platform-specific defaults:

**Configuration Locations:**
- **Windows:** `%APPDATA%\VuIO\config.toml`
- **macOS:** `~/Library/Application Support/VuIO/config.toml`
- **Linux:** `~/.config/vuio/config.toml`

### Example Configuration
```toml
[server]
port = 8080
interface = "0.0.0.0"
name = "VuIO Server"
uuid = "auto-generated"

[network]
ssdp_port = 1900
interface_selection = "Auto"
multicast_ttl = 4
announce_interval_seconds = 30

[[media.directories]]
path = "/home/user/Videos"
recursive = true
extensions = ["mp4", "mkv", "avi"]
exclude_patterns = ["*.tmp", ".*"]

[database]
path = "~/.local/share/vuio/media.db"
vacuum_on_startup = false
backup_enabled = true
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
- ✅ 81 tests passing
- ✅ Platform detection and capabilities
- ✅ Database operations and health checks
- ✅ Configuration management and validation
- ✅ File system monitoring and events
- ✅ Network interface detection
- ✅ SSDP socket creation and binding
- ✅ Media file scanning and metadata
- ✅ Error handling and recovery

## 🐛 Troubleshooting

### Common Issues

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
- Verify firewall settings
- Check multicast support on network interface
- Ensure SSDP port (1900) is not blocked
- Try specifying network interface in configuration

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
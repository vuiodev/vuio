# Cross-Platform Compatibility Design Document

## Overview

This design document outlines the architecture and implementation strategy for making the VuIO server fully cross-platform compatible. The solution addresses networking, file system, build process, and security challenges across Windows, macOS, and Linux platforms.

The design follows a layered approach with platform abstraction layers, graceful fallbacks, and comprehensive error handling to ensure consistent behavior across all supported platforms.

## Architecture

### Platform Abstraction Layer

```
┌─────────────────────────────────────────────────────────┐
│                    Application Layer                    │
├─────────────────────────────────────────────────────────┤
│                Platform Abstraction Layer              │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐│
│  │  Network    │ │ File System │ │    Build System     ││
│  │  Manager    │ │   Manager   │ │     Manager         ││
│  └─────────────┘ └─────────────┘ └─────────────────────┘│
├─────────────────────────────────────────────────────────┤
│              Platform-Specific Implementations         │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐│
│  │   Windows   │ │    macOS    │ │       Linux         ││
│  │Implementation│ │Implementation│ │   Implementation    ││
│  └─────────────┘ └─────────────┘ └─────────────────────┘│
└─────────────────────────────────────────────────────────┘
```

### Core Components

1. **Platform Detection Module**: Runtime detection of operating system and capabilities
2. **Network Abstraction Layer**: Cross-platform networking with fallback strategies
3. **File System Abstraction**: Unified file access across different file systems
4. **Build System Manager**: Platform-specific build and packaging tools
5. **Configuration Manager**: Platform-aware configuration and defaults
6. **Security Manager**: Platform-specific security and permissions handling

## Components and Interfaces

### 1. Platform Detection Module

```rust
pub struct PlatformInfo {
    pub os_type: OsType,
    pub version: String,
    pub capabilities: PlatformCapabilities,
    pub network_interfaces: Vec<NetworkInterface>,
}

pub enum OsType {
    Windows,
    MacOS,
    Linux,
}

pub struct PlatformCapabilities {
    pub can_bind_privileged_ports: bool,
    pub supports_multicast: bool,
    pub has_firewall: bool,
    pub case_sensitive_fs: bool,
}
```

### 2. Network Manager

```rust
pub trait NetworkManager {
    async fn create_ssdp_socket(&self) -> Result<SsdpSocket>;
    async fn get_local_interfaces(&self) -> Result<Vec<NetworkInterface>>;
    async fn join_multicast_group(&self, socket: &SsdpSocket, group: &str) -> Result<()>;
    async fn send_multicast(&self, socket: &SsdpSocket, data: &[u8]) -> Result<()>;
}

pub struct SsdpSocket {
    pub socket: UdpSocket,
    pub port: u16,
    pub interfaces: Vec<NetworkInterface>,
}
```

**Platform-Specific Implementations:**

- **Windows**: Uses SO_REUSEADDR, handles UAC elevation, implements firewall detection
- **macOS**: Standard multicast with interface selection
- **Linux**: Handles multiple network namespaces and interface binding

### 3. File System Manager

```rust
pub trait FileSystemManager {
    async fn scan_media_directory(&self, path: &Path) -> Result<Vec<MediaFile>>;
    fn normalize_path(&self, path: &Path) -> PathBuf;
    fn is_accessible(&self, path: &Path) -> bool;
    fn get_file_info(&self, path: &Path) -> Result<FileInfo>;
}

pub struct FileInfo {
    pub size: u64,
    pub modified: SystemTime,
    pub permissions: FilePermissions,
    pub mime_type: String,
}
```

### 4. Configuration Manager

```rust
pub struct PlatformConfig {
    pub default_media_dir: PathBuf,
    pub config_dir: PathBuf,
    pub log_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub database_dir: PathBuf,
    pub preferred_ports: Vec<u16>,
}

impl PlatformConfig {
    pub fn for_current_platform() -> Self;
    pub fn get_default_media_directories(&self) -> Vec<PathBuf>;
    pub fn get_config_file_path(&self) -> PathBuf;
    pub fn get_database_path(&self) -> PathBuf;
}

pub struct AppConfig {
    pub monitored_directories: Vec<MonitoredDirectory>,
    pub network_interface: NetworkInterfaceConfig,
    pub server_port: u16,
    pub ssdp_port: u16,
    pub scan_on_startup: bool,
    pub watch_for_changes: bool,
}

pub struct MonitoredDirectory {
    pub path: PathBuf,
    pub recursive: bool,
    pub file_extensions: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

pub enum NetworkInterfaceConfig {
    Auto,
    Specific(String),
    All,
}
```

### 5. Database Manager

```rust
pub trait DatabaseManager {
    async fn initialize(&self) -> Result<()>;
    async fn store_media_file(&self, file: &MediaFile) -> Result<()>;
    async fn get_all_media_files(&self) -> Result<Vec<MediaFile>>;
    async fn remove_media_file(&self, path: &Path) -> Result<()>;
    async fn update_media_file(&self, file: &MediaFile) -> Result<()>;
    async fn get_files_in_directory(&self, dir: &Path) -> Result<Vec<MediaFile>>;
    async fn cleanup_missing_files(&self, existing_paths: &[PathBuf]) -> Result<()>;
    async fn get_file_by_path(&self, path: &Path) -> Result<Option<MediaFile>>;
}

pub struct SqliteDatabase {
    connection: Arc<Mutex<rusqlite::Connection>>,
    db_path: PathBuf,
}

pub struct MediaFile {
    pub id: Option<i64>,
    pub path: PathBuf,
    pub filename: String,
    pub size: u64,
    pub modified: SystemTime,
    pub mime_type: String,
    pub duration: Option<Duration>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}
```

### 6. File System Watcher

```rust
pub trait FileSystemWatcher {
    async fn start_watching(&self, directories: &[PathBuf]) -> Result<()>;
    async fn stop_watching(&self) -> Result<()>;
    fn get_event_receiver(&self) -> Receiver<FileSystemEvent>;
    async fn add_watch_path(&self, path: &Path) -> Result<()>;
    async fn remove_watch_path(&self, path: &Path) -> Result<()>;
}

pub enum FileSystemEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}

pub struct CrossPlatformWatcher {
    watcher: RecommendedWatcher,
    event_sender: Sender<FileSystemEvent>,
    watched_paths: HashSet<PathBuf>,
}
```

## Data Models

### Network Interface Model

```rust
pub struct NetworkInterface {
    pub name: String,
    pub ip_address: IpAddr,
    pub is_loopback: bool,
    pub is_up: bool,
    pub supports_multicast: bool,
    pub interface_type: InterfaceType,
}

pub enum InterfaceType {
    Ethernet,
    WiFi,
    VPN,
    Loopback,
    Other(String),
}
```

### SSDP Configuration Model

```rust
pub struct SsdpConfig {
    pub primary_port: u16,
    pub fallback_ports: Vec<u16>,
    pub multicast_address: IpAddr,
    pub announce_interval: Duration,
    pub max_retries: u32,
    pub interfaces: Vec<NetworkInterface>,
}
```

### Configuration File Model

```rust
#[derive(Serialize, Deserialize)]
pub struct ConfigFile {
    pub server: ServerConfig,
    pub network: NetworkConfig,
    pub media: MediaConfig,
    pub database: DatabaseConfig,
}

#[derive(Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub interface: String,
    pub name: String,
    pub uuid: String,
}

#[derive(Serialize, Deserialize)]
pub struct NetworkConfig {
    pub ssdp_port: u16,
    pub interface_selection: NetworkInterfaceConfig,
    pub multicast_ttl: u8,
    pub announce_interval_seconds: u64,
}

#[derive(Serialize, Deserialize)]
pub struct MediaConfig {
    pub directories: Vec<MonitoredDirectoryConfig>,
    pub scan_on_startup: bool,
    pub watch_for_changes: bool,
    pub supported_extensions: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct MonitoredDirectoryConfig {
    pub path: String,
    pub recursive: bool,
    pub extensions: Option<Vec<String>>,
    pub exclude_patterns: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: Option<String>,
    pub vacuum_on_startup: bool,
    pub backup_enabled: bool,
}
```

### Build Configuration Model

```rust
pub struct BuildConfig {
    pub target_platforms: Vec<BuildTarget>,
    pub cross_compile_settings: HashMap<String, CrossCompileConfig>,
    pub package_formats: Vec<PackageFormat>,
}

pub struct BuildTarget {
    pub triple: String,
    pub display_name: String,
    pub linker: Option<String>,
    pub additional_flags: Vec<String>,
}

pub enum PackageFormat {
    Msi,      // Windows
    Pkg,      // macOS
    Deb,      // Debian/Ubuntu
    Rpm,      // RedHat/SUSE
    AppImage, // Universal Linux
}
```

## Error Handling

### Platform-Specific Error Types

```rust
#[derive(Error, Debug)]
pub enum PlatformError {
    #[error("Windows-specific error: {0}")]
    Windows(#[from] WindowsError),
    
    #[error("macOS-specific error: {0}")]
    MacOS(#[from] MacOSError),
    
    #[error("Linux-specific error: {0}")]
    Linux(#[from] LinuxError),
    
    #[error("Network configuration error: {0}")]
    NetworkConfig(String),
    
    #[error("File system access error: {0}")]
    FileSystemAccess(String),
}

#[derive(Error, Debug)]
pub enum WindowsError {
    #[error("Administrator privileges required for port {0}")]
    PrivilegedPortAccess(u16),
    
    #[error("Windows Firewall blocking multicast")]
    FirewallBlocked,
    
    #[error("UNC path access denied: {0}")]
    UncPathDenied(String),
}
```

### Error Recovery Strategies

1. **Port Binding Failures**: Automatic fallback to alternative ports (8080, 8081, etc.)
2. **Multicast Failures**: Fall back to unicast discovery with broadcast
3. **File Access Failures**: Provide platform-specific permission guidance
4. **Network Interface Issues**: Retry with different interfaces or manual configuration
5. **Database Corruption**: Automatic database rebuild from fresh media scan
6. **Configuration File Issues**: Fall back to defaults and recreate configuration
7. **File System Watcher Failures**: Fall back to periodic scanning with user notification

## Testing Strategy

### Platform-Specific Test Suites

1. **Unit Tests**: Cross-platform compatibility for core logic
2. **Integration Tests**: Platform-specific networking and file system tests
3. **End-to-End Tests**: Full DLNA discovery and media serving on each platform
4. **Performance Tests**: Network throughput and file serving performance
5. **Security Tests**: Permission handling and privilege escalation

### Test Infrastructure

```rust
#[cfg(test)]
mod platform_tests {
    use super::*;
    
    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn test_windows_ssdp_binding() {
        // Windows-specific SSDP tests
    }
    
    #[tokio::test]
    #[cfg(target_os = "macos")]
    async fn test_macos_multicast() {
        // macOS-specific multicast tests
    }
    
    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn test_linux_interface_detection() {
        // Linux-specific interface tests
    }
}
```

### Continuous Integration Matrix

```yaml
# GitHub Actions matrix for cross-platform testing
strategy:
  matrix:
    os: [windows-latest, macos-latest, ubuntu-latest]
    rust: [stable, beta]
    include:
      - os: windows-latest
        target: x86_64-pc-windows-msvc
      - os: macos-latest
        target: x86_64-apple-darwin
      - os: ubuntu-latest
        target: x86_64-unknown-linux-gnu
```

## Implementation Details

### SSDP Socket Management

**Windows Implementation:**
```rust
impl NetworkManager for WindowsNetworkManager {
    async fn create_ssdp_socket(&self) -> Result<SsdpSocket> {
        // Try standard port first
        match self.try_bind_port(1900).await {
            Ok(socket) => Ok(socket),
            Err(_) => {
                // Check if we need admin privileges
                if self.requires_elevation(1900) {
                    warn!("Port 1900 requires administrator privileges");
                    // Try alternative ports
                    for port in &[8080, 8081, 8082] {
                        if let Ok(socket) = self.try_bind_port(*port).await {
                            return Ok(socket);
                        }
                    }
                }
                Err(PlatformError::Windows(WindowsError::PrivilegedPortAccess(1900)))
            }
        }
    }
}
```

**Cross-Platform Interface Detection:**
```rust
pub async fn detect_network_interfaces() -> Result<Vec<NetworkInterface>> {
    let mut interfaces = Vec::new();
    
    #[cfg(target_os = "windows")]
    {
        interfaces = windows_interface_detection().await?;
    }
    
    #[cfg(target_os = "macos")]
    {
        interfaces = macos_interface_detection().await?;
    }
    
    #[cfg(target_os = "linux")]
    {
        interfaces = linux_interface_detection().await?;
    }
    
    // Filter and prioritize interfaces
    interfaces.retain(|iface| iface.is_up && !iface.is_loopback);
    interfaces.sort_by_key(|iface| match iface.interface_type {
        InterfaceType::Ethernet => 0,
        InterfaceType::WiFi => 1,
        InterfaceType::VPN => 2,
        _ => 3,
    });
    
    Ok(interfaces)
}
```

### Build System Enhancements

**Cross-Platform Build Scripts:**

1. **PowerShell Script (build.ps1)** for Windows:
```powershell
# Cross-platform build script for Windows
param(
    [string[]]$Targets = @("x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"),
    [string]$BuildDir = "builds"
)

# Detect package name from Cargo.toml
$PackageName = (Select-String -Path "Cargo.toml" -Pattern '^name = "(.+)"').Matches[0].Groups[1].Value

# Create build directory
New-Item -ItemType Directory -Force -Path $BuildDir

foreach ($Target in $Targets) {
    Write-Host "Building for $Target..."
    rustup target add $Target
    cargo build --release --target $Target
    
    $SourcePath = "target\$Target\release\$PackageName.exe"
    $DestPath = "$BuildDir\$PackageName-$Target.exe"
    Copy-Item $SourcePath $DestPath
}
```

2. **Enhanced Cargo Configuration (.cargo/config.toml)**:
```toml
[target.x86_64-pc-windows-gnu]
linker = "x86_64-w64-mingw32-gcc"

[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]

[target.x86_64-apple-darwin]
rustflags = ["-C", "link-arg=-undefined", "-C", "link-arg=dynamic_lookup"]

[target.x86_64-unknown-linux-gnu]
linker = "gcc"
```

### Package Generation

**Windows MSI Generation:**
```rust
// Using wix-rs or similar for MSI generation
pub fn create_windows_installer(config: &BuildConfig) -> Result<PathBuf> {
    let wix_config = WixConfig {
        product_name: "VuIO Server",
        manufacturer: "VuIO Project",
        version: env!("CARGO_PKG_VERSION"),
        executable_path: "vuio.exe",
        install_dir: r"ProgramFiles\VuIO",
        create_desktop_shortcut: true,
        create_start_menu_entry: true,
        firewall_rules: vec![
            FirewallRule::new("VuIO HTTP", 8080, Protocol::Tcp),
            FirewallRule::new("VuIO SSDP", 1900, Protocol::Udp),
        ],
    };
    
    wix_config.build()
}
```

### Database Schema and Management

**SQLite Database Schema:**
```sql
CREATE TABLE media_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT UNIQUE NOT NULL,
    filename TEXT NOT NULL,
    size INTEGER NOT NULL,
    modified INTEGER NOT NULL,
    mime_type TEXT NOT NULL,
    duration INTEGER,
    title TEXT,
    artist TEXT,
    album TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_media_files_path ON media_files(path);
CREATE INDEX idx_media_files_modified ON media_files(modified);
CREATE INDEX idx_media_files_mime_type ON media_files(mime_type);
```

**Database Implementation:**
```rust
impl SqliteDatabase {
    pub async fn new(db_path: PathBuf) -> Result<Self> {
        let connection = rusqlite::Connection::open(&db_path)?;
        connection.execute_batch(include_str!("schema.sql"))?;
        
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            db_path,
        })
    }
    
    pub async fn perform_incremental_scan(&self, directory: &Path, existing_files: &[MediaFile]) -> Result<ScanResult> {
        // Compare file system state with database
        // Return only new, modified, or deleted files
    }
}
```

### File System Monitoring Implementation

**Cross-Platform File Watcher:**
```rust
impl CrossPlatformWatcher {
    pub async fn new() -> Result<Self> {
        let (event_sender, _) = mpsc::channel(1000);
        let watcher = notify::recommended_watcher(move |res| {
            match res {
                Ok(event) => {
                    // Convert notify events to our FileSystemEvent enum
                    let fs_event = Self::convert_event(event);
                    let _ = event_sender.try_send(fs_event);
                }
                Err(e) => warn!("File watcher error: {:?}", e),
            }
        })?;
        
        Ok(Self {
            watcher,
            event_sender,
            watched_paths: HashSet::new(),
        })
    }
    
    fn convert_event(event: notify::Event) -> FileSystemEvent {
        match event.kind {
            notify::EventKind::Create(_) => FileSystemEvent::Created(event.paths[0].clone()),
            notify::EventKind::Modify(_) => FileSystemEvent::Modified(event.paths[0].clone()),
            notify::EventKind::Remove(_) => FileSystemEvent::Deleted(event.paths[0].clone()),
            _ => FileSystemEvent::Modified(event.paths[0].clone()),
        }
    }
}
```

### Configuration Management Implementation

**Configuration File Handling:**
```rust
impl AppConfig {
    pub fn load_or_create(config_path: &Path) -> Result<Self> {
        if config_path.exists() {
            let content = std::fs::read_to_string(config_path)?;
            let config: ConfigFile = toml::from_str(&content)?;
            Ok(Self::from_config_file(config))
        } else {
            let default_config = Self::default_for_platform();
            default_config.save_to_file(config_path)?;
            Ok(default_config)
        }
    }
    
    pub fn watch_for_changes(&self, config_path: &Path) -> Result<Receiver<AppConfig>> {
        // Set up file watcher for configuration file
        // Return receiver for configuration updates
    }
    
    fn default_for_platform() -> Self {
        let platform_config = PlatformConfig::for_current_platform();
        Self {
            monitored_directories: vec![
                MonitoredDirectory {
                    path: platform_config.default_media_dir,
                    recursive: true,
                    file_extensions: vec![
                        "mp4".to_string(), "mkv".to_string(), "avi".to_string(),
                        "mp3".to_string(), "flac".to_string(), "wav".to_string(),
                        "jpg".to_string(), "png".to_string(), "gif".to_string(),
                    ],
                    exclude_patterns: vec![
                        ".*".to_string(), // Hidden files
                        "Thumbs.db".to_string(), // Windows thumbnails
                        ".DS_Store".to_string(), // macOS metadata
                    ],
                }
            ],
            network_interface: NetworkInterfaceConfig::Auto,
            server_port: 8080,
            ssdp_port: 1900,
            scan_on_startup: true,
            watch_for_changes: true,
        }
    }
}
```

This design provides a comprehensive foundation for cross-platform compatibility while maintaining clean separation of concerns and robust error handling. The addition of persistent storage, real-time file monitoring, and flexible configuration management ensures the DLNA server can efficiently manage media libraries and adapt to user preferences across all supported platforms.
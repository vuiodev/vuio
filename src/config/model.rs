use serde::{Deserialize, Serialize};

pub(super) fn default_cleanup_deleted_files() -> bool {
    true
}

pub(super) fn default_true() -> bool {
    true
}

pub(super) fn default_false() -> bool {
    false
}

pub(super) fn default_session_ttl_hours() -> u64 {
    12
}

pub(super) fn default_autoplay_enabled() -> bool {
    true
}

pub(super) fn default_scan_playlists() -> bool {
    true
}

pub(super) fn default_unavailable_root_grace_hours() -> u64 {
    168
}

pub(super) fn default_redb_cache_mb() -> usize {
    128
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub network: NetworkConfig,
    pub media: MediaConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub management: ManagementConfig,
    #[serde(default)]
    pub cast: CastConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CastConfig {
    /// Enable Chromecast (Castv2) discovery and control.
    #[serde(default = "default_true")]
    pub chromecast_enabled: bool,
    /// Enable Apple AirPlay discovery and control.
    #[serde(default = "default_true")]
    pub airplay_enabled: bool,
    /// Interval between discovery scans in seconds.
    #[serde(default = "default_discovery_interval")]
    pub discovery_interval_seconds: u64,
}

pub(super) fn default_discovery_interval() -> u64 {
    30
}

impl Default for CastConfig {
    fn default() -> Self {
        Self {
            chromecast_enabled: true,
            airplay_enabled: true,
            discovery_interval_seconds: default_discovery_interval(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManagementConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_false")]
    pub auth_enabled: bool,
    pub token_file: Option<String>,
    #[serde(default = "default_session_ttl_hours")]
    pub session_ttl_hours: u64,
    #[serde(default = "default_allowed_networks")]
    pub allowed_networks: Vec<String>,
}

pub(super) fn default_allowed_networks() -> Vec<String> {
    vec![
        "127.0.0.0/8".to_string(),
        "10.0.0.0/8".to_string(),
        "172.16.0.0/12".to_string(),
        "192.168.0.0/16".to_string(),
        "::1/128".to_string(),
        "fd00::/8".to_string(),
        "fe80::/10".to_string(),
    ]
}

impl Default for ManagementConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auth_enabled: false,
            token_file: None,
            session_ttl_hours: default_session_ttl_hours(),
            allowed_networks: default_allowed_networks(),
        }
    }
}

fn default_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub interface: String,
    pub name: String,
    #[serde(default = "default_uuid")]
    pub uuid: String,
    pub ip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub interface_selection: NetworkInterfaceConfig,
    pub multicast_ttl: u8,
    pub announce_interval_seconds: u64,
    #[serde(default)]
    pub upnp_callback_allowed_networks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum NetworkInterfaceConfig {
    Auto,
    #[serde(rename = "All")]
    All,
    #[serde(untagged)]
    Specific(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaConfig {
    pub directories: Vec<MonitoredDirectoryConfig>,
    pub scan_on_startup: bool,
    pub watch_for_changes: bool,
    #[serde(default = "default_cleanup_deleted_files")]
    pub cleanup_deleted_files: bool,
    #[serde(default = "default_autoplay_enabled")]
    pub autoplay_enabled: bool,
    #[serde(default = "default_scan_playlists")]
    pub scan_playlists: bool,
    #[serde(default = "default_unavailable_root_grace_hours")]
    pub unavailable_root_grace_hours: u64,
    pub supported_extensions: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ValidationMode {
    Strict,
    #[default]
    Warn,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitoredDirectoryConfig {
    pub path: String,
    pub recursive: bool,
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    pub extensions: Option<Vec<String>>,
    pub exclude_patterns: Option<Vec<String>>,
    #[serde(default)]
    pub validation_mode: ValidationMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: Option<String>,
    pub vacuum_on_startup: bool,
    #[serde(default)]
    pub compact_on_shutdown: bool,
    pub backup_enabled: bool,
    #[serde(default = "default_redb_cache_mb")]
    pub redb_cache_mb: usize,
}

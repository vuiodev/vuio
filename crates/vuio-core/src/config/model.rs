use serde::{Deserialize, Serialize};

pub(super) fn default_cleanup_deleted_files() -> bool {
    true
}

pub(super) fn default_mdns_enabled() -> bool {
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

pub(super) fn default_cache_mb() -> usize {
    128
}

pub(super) fn default_true() -> bool {
    true
}

pub(super) fn default_web_ui_port() -> u16 {
    8090
}

/// The providers that need no account. Everything else stays off until the
/// operator supplies a credential, so the default configuration cannot produce a
/// run that fails half its lookups on 401.
pub(super) fn default_mediainfo_providers() -> Vec<String> {
    crate::mediainfo::DEFAULT_PROVIDER_IDS
        .iter()
        .map(|id| (*id).to_string())
        .collect()
}

pub(super) fn default_min_confidence() -> u8 {
    60
}

pub(super) fn default_mediainfo_timeout_seconds() -> u64 {
    15
}

/// Settings the host supplied on the command line, which win over the file for the
/// lifetime of the run.
///
/// These are re-applied after every load rather than baked into the file: the file
/// stays the durable configuration and remains editable, and `--port` stops meaning
/// "silently freeze everything else too".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConfigOverrides {
    pub port: Option<u16>,
    pub server_name: Option<String>,
    pub media_dirs: Vec<std::path::PathBuf>,
}

impl ConfigOverrides {
    pub fn is_empty(&self) -> bool {
        self.port.is_none() && self.server_name.is_none() && self.media_dirs.is_empty()
    }

    /// Apply whatever was set onto an already-loaded configuration.
    pub fn apply(&self, config: &mut AppConfig) {
        if let Some(port) = self.port {
            config.server.port = port;
        }
        if let Some(name) = &self.server_name {
            config.server.name = name.clone();
        }
        if !self.media_dirs.is_empty() {
            config.media.directories = self
                .media_dirs
                .iter()
                .map(|path| {
                    if !path.is_dir() {
                        tracing::warn!("Media directory is not available: {}", path.display());
                    }
                    MonitoredDirectoryConfig {
                        path: path.to_string_lossy().into_owned(),
                        recursive: true,
                        case_sensitive: None,
                        extensions: None,
                        exclude_patterns: None,
                        validation_mode: ValidationMode::Warn,
                    }
                })
                .collect();
        }
    }

    /// The settings this forces, as dotted config keys and the value in force, so a
    /// settings screen can say why an edit will not take hold until the next start.
    pub fn in_force(&self) -> Vec<(&'static str, String)> {
        let mut forced = Vec::new();
        if let Some(port) = self.port {
            forced.push(("server.port", port.to_string()));
        }
        if let Some(name) = &self.server_name {
            forced.push(("server.name", name.clone()));
        }
        if !self.media_dirs.is_empty() {
            forced.push((
                "media.directories",
                self.media_dirs
                    .iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }
        forced
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub network: NetworkConfig,
    pub media: MediaConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub management: ManagementConfig,
    #[serde(default)]
    pub mediainfo: MediaInfoConfig,
    #[serde(default)]
    pub web_ui: WebUiConfig,
    #[serde(default)]
    pub mcp: McpConfig,
}

/// The Model Context Protocol server, which lets an AI agent browse, search and
/// cast the library.
///
/// Defaulted as a whole, like `[management]` and `[web_ui]`: a config file
/// written before this existed has no `[mcp]` table and must keep loading, with
/// the behaviour it had then.
///
/// The tools are not all read-only — playlists can be deleted and casting drives
/// real devices on the network — so `read_only` exists to hand an agent the
/// browsing half without the rest, and `require_auth` exists to demand a token
/// for it even on a server that leaves the dashboard open.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Hide every mutating and casting tool. They vanish from `tools/list` and
    /// are refused by `tools/call`, so an agent cannot reach them by guessing a
    /// name it saw elsewhere.
    #[serde(default = "default_false")]
    pub read_only: bool,
    /// Require `Authorization: Bearer <admin token>` on the MCP endpoint even
    /// when `[management].enabled` is false.
    #[serde(default = "default_false")]
    pub require_auth: bool,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            read_only: false,
            require_auth: false,
        }
    }
}

/// The second HTTP listener, carrying the Svelte browser interface.
///
/// Defaulted as a whole, like `[management]` and `[mediainfo]`: a config file
/// written before this existed has no `[web_ui]` table and must keep loading.
///
/// It is a second front end, not a second server. The listener serves the same
/// router over the same state as the main port, so the interface reaches the
/// database through the same in-process handlers — there is no proxy between
/// the two ports and nothing is duplicated but the socket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebUiConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Deliberately not `server.port`: the two listeners would fight over the
    /// address, and the validator rejects that rather than letting one of them
    /// lose the race at startup.
    #[serde(default = "default_web_ui_port")]
    pub port: u16,
}

impl Default for WebUiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: default_web_ui_port(),
        }
    }
}

/// Fetching titles, synopses and artwork from public metadata APIs.
///
/// Defaulted as a whole, like `[management]`: a config file written before this
/// existed has no `[mediainfo]` table and must keep loading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaInfoConfig {
    /// Whether the feature may run at all. Off until asked for — this is the only
    /// part of VuIO that talks to anything outside the local network.
    #[serde(default = "default_false")]
    pub enabled: bool,
    /// Provider ids to consult, in preference order. Empty means the key-free set.
    #[serde(default = "default_mediainfo_providers")]
    pub providers: Vec<String>,
    #[serde(default = "default_true")]
    pub artwork_enabled: bool,
    /// Where downloaded posters are cached. Filled in from the database directory
    /// by `apply_platform_defaults` when absent.
    pub artwork_path: Option<String>,
    /// Below this score a match is stored but flagged rather than trusted.
    #[serde(default = "default_min_confidence")]
    pub min_confidence: u8,
    /// Whether a fetched title outranks the one read from local tags.
    #[serde(default = "default_true")]
    pub prefer_online_titles: bool,
    #[serde(default = "default_mediainfo_timeout_seconds")]
    pub request_timeout_seconds: u64,
}

impl Default for MediaInfoConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            providers: default_mediainfo_providers(),
            artwork_enabled: true,
            artwork_path: None,
            min_confidence: default_min_confidence(),
            prefer_online_titles: true,
            request_timeout_seconds: default_mediainfo_timeout_seconds(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManagementConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    pub token_file: Option<String>,
    #[serde(default = "default_session_ttl_hours")]
    pub session_ttl_hours: u64,
    #[serde(default)]
    pub allowed_networks: Vec<String>,
}

impl Default for ManagementConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            token_file: None,
            session_ttl_hours: default_session_ttl_hours(),
            allowed_networks: Vec::new(),
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
    /// Advertise the server over mDNS/DNS-SD in addition to SSDP.
    #[serde(default = "default_mdns_enabled")]
    pub mdns_enabled: bool,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub backup_enabled: bool,
    /// Page-cache budget in MiB.
    #[serde(default = "default_cache_mb")]
    pub cache_mb: usize,
}

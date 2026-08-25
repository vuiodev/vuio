//! Reading and writing `config.toml` from the dashboard's Admin tab.
//!
//! The schema table below is the single source of truth for what the tab renders.
//! A test asserts it covers every leaf of `AppConfig`, so a config field added later
//! fails the suite rather than quietly going missing from the UI.
//!
//! Writes edit the live TOML document rather than regenerating it from the template.
//! That keeps the operator's comments and any keys VuIO does not know about, and it
//! leaves directory path strings exactly as written — a re-normalised path reads as a
//! removed root, and removing a root deletes its indexed content.

use crate::{
    config::{validation::ConfigValidator, AppConfig, ConfigChangeImpact},
    database::DatabaseManager,
    state::AppState,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use toml_edit::DocumentMut;

/// How a setting is typed, which is all the UI needs to pick a control.
#[derive(Clone, Copy, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum FieldKind {
    Bool,
    Int {
        min: i64,
        max: i64,
    },
    Text,
    Path,
    /// `free_form` allows a value outside `options` — a named network interface, say.
    Enum {
        options: &'static [&'static str],
        free_form: bool,
    },
    StringList,
}

/// When a change takes hold.
///
/// `Restart` and `NextStart` are not the same thing, and conflating them is what made the
/// old labels untrustworthy: one means the running server is still using the old value,
/// the other means the setting only ever describes what happens at startup and there is
/// nothing to apply now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Impact {
    Live,
    Restart,
    NextStart,
}

#[derive(Clone, Copy, Serialize)]
struct FieldSpec {
    /// Dotted TOML path, e.g. `network.mdns_enabled`.
    key: &'static str,
    label: &'static str,
    #[serde(flatten)]
    kind: FieldKind,
    impact: Impact,
    help: &'static str,
    /// Whether the key may be removed from the file to fall back to a default.
    /// Most cannot: `AppConfig` declares them without a serde default, so a file
    /// missing one fails to load at all. The UI only offers an unset switch here.
    removable: bool,
    /// A caveat about what the setting actually does. Present only where the code
    /// does something narrower than the name suggests; hiding that would make the
    /// screen actively misleading.
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<&'static str>,
}

#[derive(Clone, Copy, Serialize)]
struct SectionSpec {
    id: &'static str,
    title: &'static str,
    blurb: &'static str,
    fields: &'static [FieldSpec],
    /// Marks the section the UI renders as a repeatable directory editor rather
    /// than a list of fields.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    directories: bool,
    /// Marks a section that carries an action panel below its fields — provider
    /// credentials and the Fetch button, which are not settings in the file and
    /// so cannot be described as `FieldSpec`s.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    panel: bool,
}

/// A setting the file must always carry: `AppConfig` has no serde default for it,
/// so a config missing it does not load.
const fn field(
    key: &'static str,
    label: &'static str,
    kind: FieldKind,
    impact: Impact,
    help: &'static str,
) -> FieldSpec {
    FieldSpec {
        key,
        label,
        kind,
        impact,
        help,
        removable: false,
        note: None,
    }
}

/// A setting that may be left out of the file entirely, falling back to a default.
const fn optional(
    key: &'static str,
    label: &'static str,
    kind: FieldKind,
    impact: Impact,
    help: &'static str,
) -> FieldSpec {
    FieldSpec {
        removable: true,
        ..field(key, label, kind, impact, help)
    }
}

const fn noted(spec: FieldSpec, note: &'static str) -> FieldSpec {
    FieldSpec {
        note: Some(note),
        ..spec
    }
}

const SERVER_FIELDS: &[FieldSpec] = &[
    field(
        "server.port",
        "HTTP port",
        FieldKind::Int { min: 1, max: 65535 },
        Impact::Live,
        "Port for the dashboard, media streaming and UPnP control.",
    ),
    field(
        "server.interface",
        "Bind address",
        FieldKind::Text,
        Impact::Live,
        "Address the HTTP server binds. 0.0.0.0 accepts connections on every interface.",
    ),
    field(
        "server.name",
        "Friendly name",
        FieldKind::Text,
        Impact::Live,
        "The name TVs and players show for this server.",
    ),
    noted(
        field(
            "server.uuid",
            "Device UUID",
            FieldKind::Text,
            Impact::Live,
            "Stable identity advertised over UPnP.",
        ),
        "Changing this makes every client treat the server as a brand new device.",
    ),
    noted(
        optional(
            "server.ip",
            "Advertised address",
            FieldKind::Text,
            Impact::Live,
            "Address written into the media URLs handed to clients. Leave unset to detect it.",
        ),
        "Usually only needed in Docker, where the container cannot see its host address.",
    ),
];

const NETWORK_FIELDS: &[FieldSpec] = &[
    noted(
        field(
            "network.interface_selection",
            "Interface selection",
            FieldKind::Enum {
                options: &["Auto", "All"],
                free_form: true,
            },
            Impact::Live,
            "Auto, All, or a specific interface name or address.",
        ),
        "Only affects the address advertised in media URLs. It does not choose which \
         interface SSDP binds to.",
    ),
    noted(
        field(
            "network.multicast_ttl",
            "Multicast TTL",
            FieldKind::Int { min: 1, max: 255 },
            Impact::Live,
            "Hop limit for SSDP discovery packets.",
        ),
        "Raise this only to reach a subnet across a router; most networks need no more \
         than the default.",
    ),
    field(
        "network.announce_interval_seconds",
        "Announce interval",
        FieldKind::Int {
            min: 1,
            max: 86_400,
        },
        Impact::Live,
        "Seconds between SSDP presence announcements.",
    ),
    optional(
        "network.mdns_enabled",
        "Advertise over mDNS",
        FieldKind::Bool,
        Impact::Live,
        "Also announce the server over Bonjour/DNS-SD, alongside SSDP.",
    ),
    optional(
        "network.upnp_callback_allowed_networks",
        "UPnP callback networks",
        FieldKind::StringList,
        Impact::Live,
        "Extra CIDRs accepted as UPnP event callback destinations, beyond the subscriber itself.",
    ),
];

const MEDIA_FIELDS: &[FieldSpec] = &[
    field(
        "media.scan_on_startup",
        "Scan at startup",
        FieldKind::Bool,
        Impact::NextStart,
        "Walk every library at boot to pick up changes made while the server was down.",
    ),
    field(
        "media.watch_for_changes",
        "Watch for changes",
        FieldKind::Bool,
        Impact::Live,
        "Index new and deleted files as they appear, rather than only at startup.",
    ),
    optional(
        "media.cleanup_deleted_files",
        "Remove deleted files",
        FieldKind::Bool,
        Impact::Live,
        "Drop files from the index once they are gone from disk, or once their library is \
         no longer configured. Turn it off and nothing is removed automatically.",
    ),
    optional(
        "media.autoplay_enabled",
        "Autoplay next track",
        FieldKind::Bool,
        Impact::Live,
        "Let renderers continue to the next item in a folder automatically.",
    ),
    optional(
        "media.scan_playlists",
        "Index playlists",
        FieldKind::Bool,
        Impact::Live,
        "Read .m3u and .pls files found in the libraries.",
    ),
    optional(
        "media.unavailable_root_grace_hours",
        "Offline library grace",
        FieldKind::Int {
            min: 1,
            max: 8_760,
        },
        Impact::Live,
        "How long a library that has gone offline keeps its indexed content before it is dropped.",
    ),
    noted(
        optional(
            "media.full_rescan_interval_hours",
            "Full rescan interval",
            FieldKind::Int { min: 0, max: 8_760 },
            Impact::Live,
            "How often every library is swept from scratch, in hours. 0 leaves discovery \
             entirely to the file watcher.",
        ),
        "Changes are normally picked up by the watcher within seconds. This sweep exists for \
         what the watcher cannot see — a network share that drops events, most often — so it \
         costs a full walk of every library each time it runs.",
    ),
    field(
        "media.supported_extensions",
        "File extensions",
        FieldKind::StringList,
        Impact::Live,
        "Extensions indexed across all libraries, unless a library overrides them.",
    ),
];

const DATABASE_FIELDS: &[FieldSpec] = &[
    optional(
        "database.path",
        "Database file",
        FieldKind::Path,
        Impact::Restart,
        "Where the media index lives. Leave unset for the platform default location.",
    ),
    noted(
        field(
            "database.vacuum_on_startup",
            "Compact the index",
            FieldKind::Bool,
            Impact::NextStart,
            "Reclaim free space in the index file when the server starts and stops.",
        ),
        "Compaction rewrites the whole file, so on a large library it adds time to both. \
         It is not needed for durability — only to give back space left by deletions.",
    ),
    noted(
        field(
            "database.backup_enabled",
            "Automatic backups",
            FieldKind::Bool,
            Impact::Live,
            "Back up the index at startup, once a day, and at shutdown.",
        ),
        "Applies from the next daily tick; the startup and shutdown backups need a restart.",
    ),
    noted(
        optional(
            "database.cache_mb",
            "Index cache",
            FieldKind::Int { min: 1, max: 4_096 },
            Impact::Restart,
            "Megabytes of memory the index keeps cached, per database connection.",
        ),
        "A budget per connection — one writer plus two to four readers — but only a \
         connection running a large query fills its share, so resident memory grows by \
         roughly this much rather than a multiple of it. Folder browsing does not depend \
         on it. Search does, and as a step rather than a slope: on a very large library \
         it stays slow until the search index fits and then roughly halves. Raising this \
         is worth it only if it goes past that line — below it, the memory is spent and \
         nothing gets faster.",
    ),
];

const MANAGEMENT_FIELDS: &[FieldSpec] = &[
    noted(
        optional(
            "management.enabled",
            "Require a token",
            FieldKind::Bool,
            Impact::Live,
            "Protect the dashboard and every management endpoint behind the admin token.",
        ),
        "The --auth flag and VUIO_AUTH=1 also turn this on, and neither the file nor this \
         switch can undo that. Turning it on signs out nobody, but everyone will need the \
         token on their next visit.",
    ),
    optional(
        "management.token_file",
        "Token file",
        FieldKind::Path,
        Impact::Live,
        "Where the admin token is read from. Leave unset for admin.token beside this config.",
    ),
    optional(
        "management.session_ttl_hours",
        "Session lifetime",
        FieldKind::Int {
            min: 1,
            max: 8_760,
        },
        Impact::Live,
        "Hours a browser stays signed in after entering the token.",
    ),
    noted(
        optional(
            "management.allowed_networks",
            "Allowed networks",
            FieldKind::StringList,
            Impact::Live,
            "CIDRs permitted to reach management endpoints, in addition to loopback.",
        ),
        "Leave empty to allow loopback and private/link-local addresses only.",
    ),
];

const WEB_UI_FIELDS: &[FieldSpec] = &[
    noted(
        optional(
            "web_ui.enabled",
            "Serve the web interface",
            FieldKind::Bool,
            Impact::Live,
            "Run the modern browser interface on its own port, beside this dashboard.",
        ),
        "Turning this off stops the second listener. It does not affect DLNA, streaming or \
         this page, which are all served on the main port.",
    ),
    noted(
        optional(
            "web_ui.port",
            "Web interface port",
            FieldKind::Int { min: 1, max: 65535 },
            Impact::Live,
            "Port the web interface answers on. It cannot be the same as the HTTP port.",
        ),
        "The interface carries the same API and the same media as the main port, so it is \
         subject to the same access rules as this dashboard.",
    ),
];

const TRANSCODE_FIELDS: &[FieldSpec] = &[
    noted(
        optional(
            "transcode.enabled",
            "Offer decoded audio",
            FieldKind::Bool,
            Impact::Live,
            "For films whose audio is AC-3, Dolby Digital Plus or DTS, list a second, \
             already-decoded version beside the original so a TV without those licences \
             can play it with sound.",
        ),
        "Both versions are offered and the TV picks. One that can already play the original \
         is unaffected.",
    ),
    noted(
        optional(
            "transcode.audio_format",
            "Decoded audio format",
            FieldKind::Enum {
                options: &["ac3", "aac", "lpcm"],
                free_form: false,
            },
            Impact::Live,
            "AC-3 is what a television was built to decode, and the only one that keeps a \
             film's surround channels instead of folding them down to stereo. AAC is about a \
             third of the bitrate, stereo, and cannot be scrubbed. LPCM is uncompressed and \
             seekable but costs about 1.5 Mbps and applies to audio files only.",
        ),
        "AC-3 is the one to leave alone: it is the format the TVs this feature exists for \
         already decode, and it is the only setting under which a 5.1 film reaches them in 5.1.",
    ),
    noted(
        optional(
            "transcode.mode",
            "Transcode Mode",
            FieldKind::Enum {
                options: &["enabled", "forced", "disabled"],
                free_form: false,
            },
            Impact::Live,
            "Operating mode: enabled (auto/standard), forced (transcoded stream listed first for all TVs), or disabled.",
        ),
        "Use \u{201c}forced\u{201d} to force all TVs (even those with native DTS support) to play the transcoded AAC stream.",
    ),
    noted(
        optional(
            "transcode.max_concurrent",
            "Simultaneous decodes",
            FieldKind::Int { min: 1, max: 32 },
            Impact::Live,
            "How many decodes may run at once. Beyond this, a further request is refused \
             rather than queued.",
        ),
        "Decoding is the only CPU-heavy work this server does. Raising it on a small box \
         trades stutter on the streams already playing for the chance to start another.",
    ),
];

const MCP_FIELDS: &[FieldSpec] = &[
    noted(
        optional(
            "mcp.enabled",
            "Enable the MCP server",
            FieldKind::Bool,
            Impact::Restart,
            "Serve the Model Context Protocol at /mcp, so an AI assistant can browse, search \
             and cast your library.",
        ),
        "The endpoint is registered when the server starts, so turning this on or off takes \
         effect after a restart.",
    ),
    noted(
        optional(
            "mcp.read_only",
            "Read-only tools",
            FieldKind::Bool,
            Impact::Live,
            "Offer only the browsing and searching tools, hiding everything that changes \
             playlists or drives a device.",
        ),
        "The hidden tools are also refused if called by name, so an assistant that learned \
         one elsewhere still cannot use it.",
    ),
    noted(
        optional(
            "mcp.require_auth",
            "Require a token",
            FieldKind::Bool,
            Impact::Live,
            "Demand the management token on /mcp even when the dashboard itself is open.",
        ),
        "Worth turning on whenever this server is reachable from more than loopback: the \
         casting tools drive real devices on your network.",
    ),
];

const MEDIAINFO_FIELDS: &[FieldSpec] = &[
    noted(
        optional(
            "mediainfo.enabled",
            "Enable online lookups",
            FieldKind::Bool,
            Impact::Live,
            "Allow VuIO to fetch titles, synopses and artwork from public metadata services.",
        ),
        "This is the only feature that contacts anything outside the local network. Nothing \
         is requested until you press Fetch below.",
    ),
    noted(
        optional(
            "mediainfo.providers",
            "Providers",
            FieldKind::StringList,
            Impact::Live,
            "Which services to consult, one id per line, in the order they should be tried.",
        ),
        "tvmaze, musicbrainz, jikan, anilist and kitsu need no account. tmdb, omdb, discogs, \
         lastfm and genius stay idle until you save a credential for them below.",
    ),
    optional(
        "mediainfo.artwork_enabled",
        "Download artwork",
        FieldKind::Bool,
        Impact::Live,
        "Cache posters and cover art locally so DLNA clients, which usually cannot reach the \
         internet, can still display them.",
    ),
    optional(
        "mediainfo.artwork_path",
        "Artwork cache",
        FieldKind::Path,
        Impact::Restart,
        "Where downloaded artwork is kept. Leave unset to keep it beside the database.",
    ),
    noted(
        optional(
            "mediainfo.min_confidence",
            "Confidence threshold",
            FieldKind::Int { min: 0, max: 100 },
            Impact::Live,
            "How sure a match must be before it is trusted, from 0 to 100.",
        ),
        "Weaker matches are still stored, but are listed below for review instead of being \
         used. Raising this makes a later run reconsider everything below the new value.",
    ),
    optional(
        "mediainfo.prefer_online_titles",
        "Prefer fetched titles",
        FieldKind::Bool,
        Impact::Live,
        "Show the fetched title instead of the one read from the file's own tags.",
    ),
    optional(
        "mediainfo.request_timeout_seconds",
        "Request timeout",
        FieldKind::Int { min: 1, max: 120 },
        Impact::Live,
        "Seconds to wait for a provider before giving up on one lookup.",
    ),
];

const SECTIONS: &[SectionSpec] = &[
    SectionSpec {
        id: "server",
        title: "Server",
        blurb: "Identity and the address this server answers on.",
        fields: SERVER_FIELDS,
        directories: false,
        panel: false,
    },
    SectionSpec {
        id: "library",
        title: "Libraries",
        blurb: "The folders scanned for media. Changes apply without a restart.",
        fields: &[],
        directories: true,
        panel: false,
    },
    SectionSpec {
        id: "media",
        title: "Media",
        blurb: "What gets indexed, and how playback behaves.",
        fields: MEDIA_FIELDS,
        directories: false,
        panel: false,
    },
    SectionSpec {
        id: "network",
        title: "Network",
        blurb: "Discovery and advertisement on the local network.",
        fields: NETWORK_FIELDS,
        directories: false,
        panel: false,
    },
    SectionSpec {
        id: "database",
        title: "Database",
        blurb: "Storage for the media index.",
        fields: DATABASE_FIELDS,
        directories: false,
        panel: false,
    },
    SectionSpec {
        id: "web_ui",
        title: "Web interface",
        blurb: "The modern browser interface, served on its own port beside this dashboard.",
        fields: WEB_UI_FIELDS,
        directories: false,
        panel: false,
    },
    SectionSpec {
        id: "management",
        title: "Access",
        blurb: "Who may reach the dashboard and the management API.",
        fields: MANAGEMENT_FIELDS,
        directories: false,
        panel: false,
    },
    SectionSpec {
        id: "transcode",
        title: "Audio for older TVs",
        blurb: "Films often carry AC-3, Dolby Digital Plus or DTS audio, and a TV sold without \
                those licences plays the picture and nothing else. VuIO can decode them and \
                offer a second, playable version beside the original.",
        fields: TRANSCODE_FIELDS,
        directories: false,
        panel: false,
    },
    SectionSpec {
        id: "mcp",
        title: "AI assistants",
        blurb: "The Model Context Protocol endpoint, which lets an assistant such as Claude \
                browse, search and cast this library.",
        fields: MCP_FIELDS,
        directories: false,
        panel: false,
    },
    SectionSpec {
        id: "mediainfo",
        title: "MediaInfo",
        blurb: "Fetch titles, synopses, ratings and artwork for the library from public \
                metadata services.",
        fields: MEDIAINFO_FIELDS,
        directories: false,
        panel: true,
    },
];

fn spec_for(key: &str) -> Option<&'static FieldSpec> {
    SECTIONS
        .iter()
        .flat_map(|section| section.fields)
        .find(|spec| spec.key == key)
}

fn error(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({ "error": message.into() }))).into_response()
}

/// Walk a dotted path through a serialised config.
fn value_at<'a>(root: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.')
        .try_fold(root, |value, segment| value.get(segment))
}

/// Which of the schema's keys are written in the file, as opposed to filled in from
/// a serde default. `AppConfig` cannot express the difference — an absent key
/// deserialises to its default and becomes indistinguishable from one set to it — so
/// this reads the raw document instead.
fn present_keys(document: &DocumentMut) -> BTreeMap<&'static str, bool> {
    SECTIONS
        .iter()
        .flat_map(|section| section.fields)
        .map(|spec| {
            let mut item = document.as_item();
            let mut found = true;
            for segment in spec.key.split('.') {
                match item.get(segment) {
                    Some(next) => item = next,
                    None => {
                        found = false;
                        break;
                    }
                }
            }
            (spec.key, found)
        })
        .collect()
}

#[derive(Serialize)]
struct RuntimeInfo {
    config_path: String,
    /// False when the config is a scratch file a restart discards, which makes the
    /// whole tab read-only.
    writable: bool,
    read_only_reason: Option<&'static str>,
    auth_enabled: bool,
    is_docker: bool,
    version: &'static str,
    /// Where the server is actually accepting. Differs from `server.port` when a bind
    /// failed, and the settings screen has to keep saying so after a page reload.
    bound_addr: Option<String>,
    desired_addr: Option<String>,
    bind_error: Option<String>,
}

#[derive(Serialize)]
struct AdminConfigResponse {
    sections: &'static [SectionSpec],
    /// What the editor shows and round-trips: the file's value where the file sets one,
    /// and otherwise the default currently in force. Deliberately not the running value,
    /// which a command-line override can differ from — editing the file should not be
    /// able to write back a value that only came from the command line.
    values: Map<String, Value>,
    present: BTreeMap<&'static str, bool>,
    /// Settings the command line is forcing for this run, keyed by config key. A saved
    /// change to one of these lands in the file but does not take effect until restart.
    overrides: BTreeMap<&'static str, String>,
    /// Libraries exactly as the file writes them. Editing and sending these back
    /// leaves keys the operator never set out of the file, rather than freezing this
    /// version's platform defaults into it.
    directories: Value,
    /// The same libraries with defaults filled in, so the UI can show what is actually
    /// in force for a key the file leaves out.
    effective_directories: Value,
    /// What a library added from here will end up with for keys left unset.
    ///
    /// `effective_directories` only describes libraries that already exist, so a
    /// newly added card has no entry to read and its exclusions would show as
    /// "None" — while the server would in fact apply the platform defaults on the
    /// next load. This is that answer, available before the library is saved.
    library_defaults: LibraryDefaults,
    runtime: RuntimeInfo,
}

#[derive(Serialize)]
struct LibraryDefaults {
    exclude_patterns: Vec<String>,
}

impl LibraryDefaults {
    fn for_current_platform() -> Self {
        Self {
            exclude_patterns: crate::platform::config::PlatformConfig::for_current_platform()
                .get_default_exclude_patterns(),
        }
    }
}

/// The `[[media.directories]]` array as written, or `None` if the file cannot be read.
fn raw_directories(document: &DocumentMut) -> Option<Value> {
    let array = document
        .get("media")
        .and_then(|media| media.get("directories"))?
        .as_array_of_tables()?;
    let entries = array
        .iter()
        .map(|table| {
            let rendered = table.to_string();
            toml::from_str::<Value>(&rendered).unwrap_or(Value::Object(Map::new()))
        })
        .collect();
    Some(Value::Array(entries))
}

fn runtime_info<D: DatabaseManager>(state: &AppState<D>) -> RuntimeInfo {
    let is_docker = AppConfig::is_running_in_docker();
    let binding = state.http_binding.detail();
    let writable = state.config_source.durable;
    RuntimeInfo {
        config_path: state.config_source.path.display().to_string(),
        writable,
        read_only_reason: if writable {
            None
        } else if is_docker {
            Some(
                "This server is configured by environment variables. Change those and \
                 recreate the container; edits made here would be discarded.",
            )
        } else {
            Some("This server's configuration cannot be written from here.")
        },
        auth_enabled: state.auth.enabled(),
        is_docker,
        version: env!("CARGO_PKG_VERSION"),
        bound_addr: binding.addr.map(|addr| addr.to_string()),
        desired_addr: binding.desired,
        bind_error: binding.last_error,
    }
}

pub async fn get_config<D: DatabaseManager>(State(state): State<AppState<D>>) -> Response {
    let config = state.current_config();
    let serialised = match serde_json::to_value(config.as_ref()) {
        Ok(value) => value,
        Err(err) => return error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    };

    let effective_directories = serialised
        .get("media")
        .and_then(|media| media.get("directories"))
        .cloned()
        .unwrap_or(Value::Array(Vec::new()));

    // A scratch config still renders; it is simply read-only. Reading the file back is
    // what tells us which keys the operator actually wrote, as opposed to which ones
    // ended up with a value.
    let document = std::fs::read_to_string(&state.config_source.path)
        .map_err(|err| tracing::warn!("Could not read config for presence detection: {err}"))
        .ok()
        .and_then(|raw| {
            raw.parse::<DocumentMut>()
                .map_err(|err| tracing::warn!("Could not parse config for presence: {err}"))
                .ok()
        });

    let present = document.as_ref().map(present_keys).unwrap_or_default();
    let on_file = document
        .as_ref()
        .and_then(|document| toml::from_str::<Value>(&document.to_string()).ok());

    let mut values = Map::new();
    for spec in SECTIONS.iter().flat_map(|section| section.fields) {
        // Where the file sets a value, that is what the editor shows, so a save writes
        // back what was on screen. Only a key the file omits falls back to the running
        // value, which is exactly the default the operator needs to see.
        let value = on_file
            .as_ref()
            .filter(|_| present.get(spec.key).copied().unwrap_or(false))
            .and_then(|file| value_at(file, spec.key))
            .or_else(|| value_at(&serialised, spec.key))
            .cloned()
            .unwrap_or(Value::Null);
        values.insert(spec.key.to_string(), value);
    }

    let directories = document
        .as_ref()
        .and_then(raw_directories)
        .unwrap_or_else(|| effective_directories.clone());

    Json(AdminConfigResponse {
        sections: SECTIONS,
        values,
        present,
        overrides: state.config_source.overrides.in_force().into_iter().collect(),
        directories,
        effective_directories,
        library_defaults: LibraryDefaults::for_current_platform(),
        runtime: runtime_info(&state),
    })
    .into_response()
}

#[derive(Deserialize)]
pub struct ConfigUpdate {
    /// Dotted key to value. `null` removes the key from the file, restoring whatever
    /// default applies.
    #[serde(default)]
    values: BTreeMap<String, Value>,
    /// Present only when the libraries section was edited; replaces the whole array.
    #[serde(default)]
    directories: Option<Vec<Value>>,
}

/// Convert a JSON value from the browser into TOML, rejecting shapes no setting uses.
fn to_toml(value: &Value) -> Result<toml_edit::Value, String> {
    Ok(match value {
        Value::Bool(inner) => (*inner).into(),
        Value::String(inner) => inner.as_str().into(),
        Value::Number(inner) => match inner.as_i64() {
            Some(integer) => integer.into(),
            None => inner
                .as_f64()
                .ok_or_else(|| format!("unsupported number {inner}"))?
                .into(),
        },
        Value::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(to_toml(item)?);
            }
            array.into()
        }
        Value::Null | Value::Object(_) => {
            return Err("unsupported value".to_string());
        }
    })
}

/// Set or remove a dotted key, creating intermediate tables as needed.
fn apply_key(document: &mut DocumentMut, key: &str, value: &Value) -> Result<(), String> {
    let segments: Vec<&str> = key.split('.').collect();
    let (leaf, tables) = segments
        .split_last()
        .ok_or_else(|| "empty key".to_string())?;

    let mut table = document.as_table_mut();
    for segment in tables {
        let entry = table
            .entry(segment)
            .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
        table = entry
            .as_table_mut()
            .ok_or_else(|| format!("{segment} is not a table"))?;
    }

    if !value.is_null() {
        table[*leaf] = toml_edit::Item::Value(to_toml(value)?);
        return Ok(());
    }

    table.remove(leaf);
    // Unsetting the last key of a section should leave the file as it was before that
    // section existed, not with an empty `[management]` header standing over nothing.
    // Config sections are one level deep, so the immediate parent is the only candidate.
    if let [parent] = tables {
        let emptied = document
            .as_table()
            .get(parent)
            .and_then(|item| item.as_table())
            .is_some_and(|section| section.is_empty());
        if emptied {
            document.as_table_mut().remove(parent);
        }
    }
    Ok(())
}

fn apply_directories(document: &mut DocumentMut, directories: &[Value]) -> Result<(), String> {
    let mut array = toml_edit::ArrayOfTables::new();
    for directory in directories {
        let object = directory
            .as_object()
            .ok_or_else(|| "each library must be an object".to_string())?;
        let mut table = toml_edit::Table::new();
        for (key, value) in object {
            // An omitted optional key means "unset"; writing null would be invalid TOML.
            if value.is_null() {
                continue;
            }
            table[key.as_str()] = toml_edit::Item::Value(to_toml(value)?);
        }
        array.push(table);
    }

    let media = document
        .as_table_mut()
        .entry("media")
        .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .ok_or_else(|| "media is not a table".to_string())?;
    media.insert("directories", toml_edit::Item::ArrayOfTables(array));
    Ok(())
}

/// Classify a pending change by the strongest impact among the keys it touches.
fn impact_of(old: &AppConfig, new: &AppConfig) -> ConfigChangeImpact {
    if old == new {
        return ConfigChangeImpact::NoChange;
    }
    let (Ok(old_value), Ok(new_value)) = (serde_json::to_value(old), serde_json::to_value(new))
    else {
        return ConfigChangeImpact::RestartRequired;
    };

    let changed = |impact: Impact| {
        SECTIONS
            .iter()
            .flat_map(|section| section.fields)
            .filter(|spec| spec.impact == impact)
            .any(|spec| value_at(&old_value, spec.key) != value_at(&new_value, spec.key))
    };

    // Reported worst-first. A save that touches both a live setting and a next-start one
    // reports the caveat, because the live half already took effect and the half that did
    // not is the part worth saying out loud.
    if changed(Impact::Restart) {
        ConfigChangeImpact::RestartRequired
    } else if changed(Impact::NextStart) {
        ConfigChangeImpact::NextStart
    } else {
        ConfigChangeImpact::LiveReload
    }
}

/// Replace the file in one step. A truncate-then-write leaves a half-written config
/// behind if the process dies mid-write, and the watcher would try to load it.
fn write_atomically(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    let directory = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let temporary = directory.join(format!(
        ".{}.tmp",
        path.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "config.toml".to_string())
    ));
    std::fs::write(&temporary, contents)?;
    std::fs::rename(&temporary, path)
}

/// Wait for the listener supervisor to act on a bind change, and report what it did.
///
/// Bounded: the supervisor may be busy draining, and a settings save must not hang. A
/// timeout reports `pending` rather than guessing, and the standing state is readable
/// from `runtime_info` on the next page load either way.
async fn await_relocation<D: DatabaseManager>(state: &AppState<D>, before: u64) -> Value {
    let mut moves = state.http_binding.subscribe();
    let settled = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if *moves.borrow_and_update() != before {
                return;
            }
            // A failed bind never bumps the generation, so also stop once the failure
            // has been recorded.
            if state.http_binding.detail().last_error.is_some() {
                return;
            }
            if moves.changed().await.is_err() {
                return;
            }
        }
    })
    .await;

    let detail = state.http_binding.detail();
    let addr = detail.addr.map(|addr| addr.to_string());
    if let Some(error) = detail.last_error {
        return json!({
            "state": "failed",
            "serving": addr,
            "desired": detail.desired,
            "error": error,
        });
    }
    if settled.is_err() {
        return json!({ "state": "pending", "serving": addr });
    }
    json!({
        "state": "moved",
        "serving": addr,
        // The browser builds the URL to follow from its own hostname; a wildcard bind
        // says nothing about which address the operator can actually reach.
        "port": state.http_binding.port(),
    })
}

pub async fn put_config<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    Json(update): Json<ConfigUpdate>,
) -> Response {
    let runtime = runtime_info(&state);
    if !runtime.writable {
        return error(
            StatusCode::CONFLICT,
            runtime.read_only_reason.unwrap_or("Configuration is read-only"),
        );
    }

    for (key, value) in &update.values {
        let Some(spec) = spec_for(key) else {
            return error(StatusCode::BAD_REQUEST, format!("Unknown setting: {key}"));
        };
        // Removing a key with no default leaves a config that will not load. Say so
        // here rather than letting the caller see a raw TOML deserialisation error.
        if value.is_null() && !spec.removable {
            return error(
                StatusCode::BAD_REQUEST,
                format!("{} must always have a value", spec.label),
            );
        }
    }

    let path = state.config_source.path.clone();
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(err) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Could not read {}: {err}", path.display()),
            )
        }
    };
    let mut document = match raw.parse::<DocumentMut>() {
        Ok(document) => document,
        Err(err) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Could not parse {}: {err}", path.display()),
            )
        }
    };

    for (key, value) in &update.values {
        if let Err(message) = apply_key(&mut document, key, value) {
            return error(StatusCode::BAD_REQUEST, format!("{key}: {message}"));
        }
    }
    if let Some(directories) = &update.directories {
        if directories.is_empty() {
            return error(
                StatusCode::BAD_REQUEST,
                "At least one library folder must be configured",
            );
        }
        if let Err(message) = apply_directories(&mut document, directories) {
            return error(StatusCode::BAD_REQUEST, message);
        }
    }

    let rendered = document.to_string();
    let mut candidate = match toml::from_str::<AppConfig>(&rendered) {
        Ok(config) => config,
        Err(err) => return error(StatusCode::BAD_REQUEST, err.to_string()),
    };
    // The running config has platform defaults applied, so the candidate needs them
    // too or every comparison against it reports phantom differences.
    if let Err(err) = candidate.apply_platform_defaults() {
        return error(StatusCode::BAD_REQUEST, err.to_string());
    }
    if let Err(err) = ConfigValidator::validate_flexible(&candidate) {
        return error(StatusCode::BAD_REQUEST, format!("{err:#}"));
    }

    // Impact describes the change the operator made, so it compares the file before
    // against the file after. Comparing against the running config would misreport a
    // run with command-line overrides: those differ from the file on every key they
    // force, which made a one-boolean edit look like it needed a restart.
    let mut before = match toml::from_str::<AppConfig>(&raw) {
        Ok(config) => config,
        // An unparseable file cannot be diffed against; the edit is a rewrite.
        Err(_) => candidate.clone(),
    };
    let _ = before.apply_platform_defaults();
    let impact = impact_of(&before, &candidate);

    // Nothing is written until the result has parsed and validated, so a rejected
    // edit leaves the file exactly as it was.
    if let Err(err) = write_atomically(&path, &rendered) {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Could not write {}: {err}", path.display()),
        );
    }

    // The file watcher picks this up and runs the reload, which is the same path a
    // hand edit takes -- it re-watches directories, rescans, and notifies subscribers.
    // But it is debounced, so a read issued straight after this response would still
    // see the old values. The bytes just written are known to parse and validate, so
    // publish them now and let the watcher's unchanged-file check swallow the echo.
    //
    // Command-line overrides go back on top, exactly as the reload will re-apply them.
    // Publishing the bare file would drop them and hand the running server a port or
    // library set the host explicitly overrode.
    let mut running = candidate;
    state.config_source.overrides.apply(&mut running);
    // Auth settings are held by AuthState rather than read from the config per request,
    // so they need applying explicitly. Doing it here as well as in the watcher's reload
    // handler means a save takes effect immediately rather than after the debounce.
    if state.current_config().management != running.management {
        if let Err(error) = state.auth.apply(&running.management, &path) {
            tracing::error!("Keeping the previous management settings: {error:#}");
        }
    }
    let wants_move = before.server.port != running.server.port
        || before.server.interface != running.server.interface;
    let generation_before = *state.http_binding.subscribe().borrow();
    state.live_config.store(std::sync::Arc::new(running));

    // A move disconnects the browser that asked for it, so the response has to say
    // where the server went rather than leaving a dead page. Waiting for the supervisor
    // to actually rebind is what makes this a report instead of a prediction; the reply
    // still arrives, because the old listener drains in-flight requests before closing.
    let relocation = if wants_move {
        Some(await_relocation(&state, generation_before).await)
    } else {
        None
    };

    Json(json!({
        "saved": true,
        "moved": relocation,
        "impact": match impact {
            ConfigChangeImpact::NoChange => "no_change",
            ConfigChangeImpact::LiveReload => "live",
            ConfigChangeImpact::NextStart => "next_start",
            ConfigChangeImpact::RestartRequired => "restart_required",
        },
    }))
    .into_response()
}

pub async fn restart<D: DatabaseManager>(State(state): State<AppState<D>>) -> Response {
    let cancellation = state.cancellation.clone();
    // Deliberately not on the state's TaskTracker: this task outlives the shutdown
    // it triggers, and the tracker is closed as part of that shutdown.
    tokio::spawn(async move {
        // Long enough for this response to reach the browser before the listener goes.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        tracing::info!("Restart requested from the admin API; shutting down");
        cancellation.cancel();
    });

    (
        StatusCode::ACCEPTED,
        Json(json!({
            "stopping": true,
            // The process exits cleanly; something has to start it again.
            "supervised": AppConfig::is_running_in_docker(),
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::generator::ConfigGenerator;

    /// The schema table is what the Admin tab renders, so a config field missing from
    /// it is a field no operator can see or change. Rather than trusting review to
    /// catch that, walk the serialised config and demand a spec for every leaf.
    #[test]
    fn every_config_field_has_a_spec() {
        let config = AppConfig::default_for_platform();
        let serialised = serde_json::to_value(&config).expect("config serialises");

        let mut leaves = Vec::new();
        collect_leaves(&serialised, String::new(), &mut leaves);

        for leaf in &leaves {
            // The libraries array has its own editor rather than one spec per entry.
            if leaf.starts_with("media.directories") {
                continue;
            }
            assert!(
                spec_for(leaf).is_some(),
                "{leaf} is in AppConfig but has no admin spec, so the Admin tab cannot show it"
            );
        }

        // And nothing in the table refers to a key that no longer exists.
        for spec in SECTIONS.iter().flat_map(|section| section.fields) {
            assert!(
                value_at(&serialised, spec.key).is_some(),
                "{} is specced but not present in AppConfig",
                spec.key
            );
        }
    }

    fn collect_leaves(value: &Value, prefix: String, out: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    collect_leaves(child, path, out);
                }
            }
            _ => out.push(prefix),
        }
    }

    /// `removable` decides whether the UI offers an unset switch, and getting it wrong
    /// is not a cosmetic error: offering to remove a key that `AppConfig` has no default
    /// for writes a config the server can no longer load. Rather than trusting the
    /// annotations, delete each key from a complete document and see what serde says.
    #[test]
    fn removable_matches_what_the_config_will_actually_load_without() {
        let complete = ConfigGenerator::new()
            .expect("generator")
            .generate_config(&AppConfig::default_for_platform())
            .expect("generate");

        for spec in SECTIONS.iter().flat_map(|section| section.fields) {
            let mut document = complete.parse::<DocumentMut>().expect("parses");
            apply_key(&mut document, spec.key, &Value::Null).expect("removes");
            let loads = toml::from_str::<AppConfig>(&document.to_string()).is_ok();

            // The UUID is the one key serde would accept as absent but we still pin:
            // its default mints a fresh one on every load, so an unset switch here
            // would silently hand the server a new identity at each boot.
            if spec.key == "server.uuid" {
                assert!(loads, "server.uuid is expected to have a serde default");
                assert!(!spec.removable, "server.uuid must not be offered as unset");
                continue;
            }

            assert_eq!(
                spec.removable, loads,
                "{} is marked removable={} but the config {} without it",
                spec.key,
                spec.removable,
                if loads { "loads" } else { "fails to load" }
            );
        }
    }

    /// The keys with a default are the only ones that can show the "not set" state,
    /// so between them the two lists have to cover the whole table.
    #[test]
    fn every_removable_key_survives_being_unset() {
        let base = AppConfig::default_for_platform();
        let complete = ConfigGenerator::new()
            .expect("generator")
            .generate_config(&base)
            .expect("generate");

        for spec in SECTIONS
            .iter()
            .flat_map(|section| section.fields)
            .filter(|spec| spec.removable)
        {
            let mut document = complete.parse::<DocumentMut>().expect("parses");
            apply_key(&mut document, spec.key, &Value::Null).expect("removes");
            let mut config = toml::from_str::<AppConfig>(&document.to_string())
                .unwrap_or_else(|error| panic!("{} should still load: {error}", spec.key));
            config.apply_platform_defaults().expect("defaults");
            ConfigValidator::validate_flexible(&config)
                .unwrap_or_else(|error| panic!("{} should still validate: {error}", spec.key));
        }
    }

    #[test]
    fn no_key_is_specced_twice() {
        let mut keys: Vec<&str> = SECTIONS
            .iter()
            .flat_map(|section| section.fields)
            .map(|spec| spec.key)
            .collect();
        let total = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(total, keys.len(), "a setting is listed in two sections");
    }

    fn document(raw: &str) -> DocumentMut {
        raw.parse::<DocumentMut>().expect("parses")
    }

    /// Presence is the whole point of the "not set" state: a key absent from the file
    /// must read as absent even though the loaded config has a value for it.
    #[test]
    fn presence_distinguishes_defaults_from_written_values() {
        let document = document(
            r#"
[server]
port = 8080

[network]
mdns_enabled = true
"#,
        );
        let present = present_keys(&document);
        assert!(present["server.port"]);
        assert!(present["network.mdns_enabled"]);
        assert!(!present["network.multicast_ttl"]);
        assert!(!present["media.autoplay_enabled"]);
    }

    #[test]
    fn setting_a_key_preserves_surrounding_comments() {
        let mut doc = document(
            r#"# Written by hand, and worth keeping.
[server]
# The port everything talks to.
port = 8080
name = "VuIO"
"#,
        );
        apply_key(&mut doc, "server.port", &json!(9090)).expect("applies");
        let rendered = doc.to_string();
        assert!(rendered.contains("port = 9090"));
        assert!(rendered.contains("# Written by hand, and worth keeping."));
        assert!(rendered.contains("# The port everything talks to."));
        assert!(rendered.contains("name = \"VuIO\""));
    }

    #[test]
    fn a_null_removes_the_key_and_creates_missing_tables() {
        let mut doc = document("[server]\nport = 8080\nip = \"10.0.0.5\"\n");
        apply_key(&mut doc, "server.ip", &Value::Null).expect("applies");
        assert!(!doc.to_string().contains("ip ="));

        apply_key(&mut doc, "management.session_ttl_hours", &json!(24)).expect("applies");
        let rendered = doc.to_string();
        assert!(rendered.contains("[management]"));
        assert!(rendered.contains("session_ttl_hours = 24"));
    }

    /// Setting a key in an absent section creates the section; unsetting the last key
    /// should take the section with it, so a set/unset round trip leaves the file as
    /// it was rather than with an empty `[management]` header over nothing.
    #[test]
    fn unsetting_the_last_key_removes_its_empty_section() {
        let original = "[server]\nport = 8080\n";
        let mut doc = document(original);

        apply_key(&mut doc, "management.session_ttl_hours", &json!(24)).expect("applies");
        assert!(doc.to_string().contains("[management]"));

        apply_key(&mut doc, "management.session_ttl_hours", &Value::Null).expect("applies");
        assert!(!doc.to_string().contains("[management]"));
        assert_eq!(doc.to_string(), original);

        // A section that still has other keys stays put.
        let mut kept = document("[management]\nenabled = true\nsession_ttl_hours = 24\n");
        apply_key(&mut kept, "management.session_ttl_hours", &Value::Null).expect("applies");
        assert!(kept.to_string().contains("[management]"));
        assert!(kept.to_string().contains("enabled = true"));
    }

    /// The libraries the UI edits come from the file, not the loaded config, so keys
    /// the operator never wrote stay unwritten. Round-tripping the effective config
    /// instead would freeze this version's platform defaults into their file.
    #[test]
    fn raw_directories_report_only_what_the_file_says() {
        let doc = document(
            r#"
[media]
scan_on_startup = true

[[media.directories]]
path = "/movies"
recursive = true

[[media.directories]]
path = "/music"
recursive = false
extensions = ["mp3"]
"#,
        );
        let directories = raw_directories(&doc).expect("directories");
        let entries = directories.as_array().expect("array");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["path"], json!("/movies"));
        // Absent in the file, so absent here — no exclude_patterns, no validation_mode.
        assert!(entries[0].get("exclude_patterns").is_none());
        assert!(entries[0].get("validation_mode").is_none());
        assert_eq!(entries[1]["extensions"], json!(["mp3"]));
    }

    #[test]
    fn directories_replace_the_whole_array() {
        let mut doc = document(
            r#"
[media]
scan_on_startup = true

[[media.directories]]
path = "/old"
recursive = true
"#,
        );
        apply_directories(
            &mut doc,
            &[
                json!({"path": "/first", "recursive": true, "case_sensitive": null}),
                json!({"path": "/second", "recursive": false, "extensions": ["mp4", "mkv"]}),
            ],
        )
        .expect("applies");

        let rendered = doc.to_string();
        assert!(!rendered.contains("/old"));
        assert!(rendered.contains("/first"));
        assert!(rendered.contains("/second"));
        // A null optional stays out of the file rather than becoming a literal.
        assert!(!rendered.contains("case_sensitive"));
        assert!(rendered.contains("scan_on_startup = true"));
    }

    #[test]
    fn impact_follows_the_specced_field() {
        let base = AppConfig::default_for_platform();
        assert_eq!(impact_of(&base, &base), ConfigChangeImpact::NoChange);

        let mut live = base.clone();
        live.media.autoplay_enabled = !live.media.autoplay_enabled;
        assert_eq!(impact_of(&base, &live), ConfigChangeImpact::LiveReload);

        // The index cannot be reopened under a live server, so this is one of the two
        // settings that genuinely still needs one.
        let mut restart = base.clone();
        restart.database.cache_mb += 1;
        assert_eq!(impact_of(&base, &restart), ConfigChangeImpact::RestartRequired);

        // A change the schema does not cover still has to be reported honestly.
        let mut directories = base.clone();
        directories.media.directories.clear();
        assert_eq!(
            impact_of(&base, &directories),
            ConfigChangeImpact::LiveReload
        );
    }

    /// A setting that only describes startup is not the same as one the running server
    /// is still ignoring, and reporting the first as "restart required" is what taught
    /// operators to distrust the labels.
    #[test]
    fn a_startup_only_change_is_not_reported_as_restart_required() {
        let base = AppConfig::default_for_platform();

        let mut next_start = base.clone();
        next_start.media.scan_on_startup = !next_start.media.scan_on_startup;
        assert_eq!(impact_of(&base, &next_start), ConfigChangeImpact::NextStart);

        // Paired with a live change, the caveat still wins: the live half already
        // applied, and the half that did not is the part worth saying.
        let mut mixed = next_start.clone();
        mixed.media.autoplay_enabled = !mixed.media.autoplay_enabled;
        assert_eq!(impact_of(&base, &mixed), ConfigChangeImpact::NextStart);

        // But a genuine restart-required change outranks both.
        let mut restart = mixed;
        restart.database.cache_mb += 1;
        assert_eq!(
            impact_of(&base, &restart),
            ConfigChangeImpact::RestartRequired
        );
    }

    /// Every field the UI can show has to carry a pill the UI knows how to render, and
    /// every impact has to be reachable — an unreferenced variant means a setting was
    /// silently reclassified into a state nothing displays.
    #[test]
    fn every_impact_variant_is_used() {
        let impacts: Vec<Impact> = SECTIONS
            .iter()
            .flat_map(|section| section.fields)
            .map(|spec| spec.impact)
            .collect();
        for expected in [Impact::Live, Impact::Restart, Impact::NextStart] {
            assert!(
                impacts.contains(&expected),
                "no setting is classified {expected:?}"
            );
        }
    }

    #[test]
    fn json_values_convert_to_the_toml_they_look_like() {
        assert_eq!(to_toml(&json!(true)).unwrap().as_bool(), Some(true));
        assert_eq!(to_toml(&json!(42)).unwrap().as_integer(), Some(42));
        assert_eq!(to_toml(&json!("x")).unwrap().as_str(), Some("x"));
        assert!(to_toml(&json!(["a", "b"])).unwrap().is_array());
        assert!(to_toml(&json!({"a": 1})).is_err());
    }
}

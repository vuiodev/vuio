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

/// Whether a change takes hold in the running server or waits for the next start.
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Impact {
    Live,
    Restart,
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
        Impact::Restart,
        "Port for the dashboard, media streaming and UPnP control.",
    ),
    field(
        "server.interface",
        "Bind address",
        FieldKind::Text,
        Impact::Restart,
        "Address the HTTP server binds. 0.0.0.0 accepts connections on every interface.",
    ),
    field(
        "server.name",
        "Friendly name",
        FieldKind::Text,
        Impact::Restart,
        "The name TVs and players show for this server.",
    ),
    noted(
        field(
            "server.uuid",
            "Device UUID",
            FieldKind::Text,
            Impact::Restart,
            "Stable identity advertised over UPnP.",
        ),
        "Changing this makes every client treat the server as a brand new device.",
    ),
    noted(
        optional(
            "server.ip",
            "Advertised address",
            FieldKind::Text,
            Impact::Restart,
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
            Impact::Restart,
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
            Impact::Restart,
            "Hop limit for SSDP discovery packets.",
        ),
        "Not applied: the SSDP socket TTL is currently fixed at 4.",
    ),
    field(
        "network.announce_interval_seconds",
        "Announce interval",
        FieldKind::Int {
            min: 1,
            max: 86_400,
        },
        Impact::Restart,
        "Seconds between SSDP presence announcements.",
    ),
    optional(
        "network.mdns_enabled",
        "Advertise over mDNS",
        FieldKind::Bool,
        Impact::Restart,
        "Also announce the server over Bonjour/DNS-SD, alongside SSDP.",
    ),
    optional(
        "network.upnp_callback_allowed_networks",
        "UPnP callback networks",
        FieldKind::StringList,
        Impact::Restart,
        "Extra CIDRs accepted as UPnP event callback destinations, beyond the subscriber itself.",
    ),
];

const MEDIA_FIELDS: &[FieldSpec] = &[
    field(
        "media.scan_on_startup",
        "Scan at startup",
        FieldKind::Bool,
        Impact::Restart,
        "Walk every library at boot to pick up changes made while the server was down.",
    ),
    field(
        "media.watch_for_changes",
        "Watch for changes",
        FieldKind::Bool,
        Impact::Live,
        "Index new and deleted files as they appear, without waiting for a restart.",
    ),
    optional(
        "media.cleanup_deleted_files",
        "Remove deleted files",
        FieldKind::Bool,
        Impact::Restart,
        "Drop files from the index when the startup scan finds them gone.",
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
    field(
        "database.vacuum_on_startup",
        "Compact at startup",
        FieldKind::Bool,
        Impact::Restart,
        "Reclaim space in the index file at boot. Slows startup on a large library.",
    ),
    noted(
        field(
            "database.backup_enabled",
            "Automatic backups",
            FieldKind::Bool,
            Impact::Restart,
            "Back up the index at startup, once a day, and at shutdown.",
        ),
        "The daily backup task is only started if this was already on at boot.",
    ),
    optional(
        "database.redb_cache_mb",
        "Index cache",
        FieldKind::Int { min: 1, max: 4_096 },
        Impact::Restart,
        "Megabytes of memory the index keeps cached.",
    ),
];

const MANAGEMENT_FIELDS: &[FieldSpec] = &[
    noted(
        optional(
            "management.enabled",
            "Require a token",
            FieldKind::Bool,
            Impact::Restart,
            "Protect the dashboard and every management endpoint behind the admin token.",
        ),
        "The --auth flag and VUIO_AUTH=1 also turn this on; neither this nor the file can \
         switch off auth the host asked for.",
    ),
    optional(
        "management.token_file",
        "Token file",
        FieldKind::Path,
        Impact::Restart,
        "Where the admin token is read from. Leave unset for admin.token beside this config.",
    ),
    optional(
        "management.session_ttl_hours",
        "Session lifetime",
        FieldKind::Int {
            min: 1,
            max: 8_760,
        },
        Impact::Restart,
        "Hours a browser stays signed in after entering the token.",
    ),
    noted(
        optional(
            "management.allowed_networks",
            "Allowed networks",
            FieldKind::StringList,
            Impact::Restart,
            "CIDRs permitted to reach management endpoints, in addition to loopback.",
        ),
        "Leave empty to allow loopback and private/link-local addresses only.",
    ),
];

const SECTIONS: &[SectionSpec] = &[
    SectionSpec {
        id: "server",
        title: "Server",
        blurb: "Identity and the address this server answers on.",
        fields: SERVER_FIELDS,
        directories: false,
    },
    SectionSpec {
        id: "library",
        title: "Libraries",
        blurb: "The folders scanned for media. Changes apply without a restart.",
        fields: &[],
        directories: true,
    },
    SectionSpec {
        id: "media",
        title: "Media",
        blurb: "What gets indexed, and how playback behaves.",
        fields: MEDIA_FIELDS,
        directories: false,
    },
    SectionSpec {
        id: "network",
        title: "Network",
        blurb: "Discovery and advertisement on the local network.",
        fields: NETWORK_FIELDS,
        directories: false,
    },
    SectionSpec {
        id: "database",
        title: "Database",
        blurb: "Storage for the media index.",
        fields: DATABASE_FIELDS,
        directories: false,
    },
    SectionSpec {
        id: "management",
        title: "Access",
        blurb: "Who may reach the dashboard and the management API.",
        fields: MANAGEMENT_FIELDS,
        directories: false,
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
}

#[derive(Serialize)]
struct AdminConfigResponse {
    sections: &'static [SectionSpec],
    /// Effective values, including those coming from a default rather than the file.
    values: Map<String, Value>,
    present: BTreeMap<&'static str, bool>,
    /// Libraries exactly as the file writes them. Editing and sending these back
    /// leaves keys the operator never set out of the file, rather than freezing this
    /// version's platform defaults into it.
    directories: Value,
    /// The same libraries with defaults filled in, so the UI can show what is actually
    /// in force for a key the file leaves out.
    effective_directories: Value,
    runtime: RuntimeInfo,
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
            Some(
                "This server was started with command-line overrides, which live in a \
                 scratch file. Restart without them to edit configuration here.",
            )
        },
        auth_enabled: state.auth.enabled(),
        is_docker,
        version: env!("CARGO_PKG_VERSION"),
    }
}

pub async fn get_config<D: DatabaseManager>(State(state): State<AppState<D>>) -> Response {
    let config = state.current_config();
    let serialised = match serde_json::to_value(config.as_ref()) {
        Ok(value) => value,
        Err(err) => return error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    };

    let mut values = Map::new();
    for spec in SECTIONS.iter().flat_map(|section| section.fields) {
        let value = value_at(&serialised, spec.key)
            .cloned()
            .unwrap_or(Value::Null);
        values.insert(spec.key.to_string(), value);
    }

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
    let directories = document
        .as_ref()
        .and_then(raw_directories)
        .unwrap_or_else(|| effective_directories.clone());

    Json(AdminConfigResponse {
        sections: SECTIONS,
        values,
        present,
        directories,
        effective_directories,
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

    let restart = SECTIONS
        .iter()
        .flat_map(|section| section.fields)
        .filter(|spec| spec.impact == Impact::Restart)
        .any(|spec| value_at(&old_value, spec.key) != value_at(&new_value, spec.key));

    if restart {
        ConfigChangeImpact::RestartRequired
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

    let impact = impact_of(state.current_config().as_ref(), &candidate);

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
    state.live_config.store(std::sync::Arc::new(candidate));

    Json(json!({
        "saved": true,
        "impact": match impact {
            ConfigChangeImpact::NoChange => "no_change",
            ConfigChangeImpact::LiveReload => "live",
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
        assert_eq!(present["server.port"], true);
        assert_eq!(present["network.mdns_enabled"], true);
        assert_eq!(present["network.multicast_ttl"], false);
        assert_eq!(present["media.autoplay_enabled"], false);
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

        let mut restart = base.clone();
        restart.server.port += 1;
        assert_eq!(impact_of(&base, &restart), ConfigChangeImpact::RestartRequired);

        // A change the schema does not cover still has to be reported honestly.
        let mut directories = base.clone();
        directories.media.directories.clear();
        assert_eq!(
            impact_of(&base, &directories),
            ConfigChangeImpact::LiveReload
        );
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

# Configuration Guide

VuIO uses a declarative TOML configuration file on native platforms.

- **Default path:** `./config/config.toml`
- **Systemd package path:** `/etc/vuio/vuio.toml`

Everything in this file can also be edited directly from the web dashboard's **Admin** tab, which writes back to this file in place while preserving your comments. When running in a Docker container configured by environment variables, the Admin tab is read-only.

Command-line flags (`--port`, `--name`, `-m`) layer on top of the file rather than replacing it. They win for the run they were given in, while the configuration file remains editable.

---

## Live Hot-Reload vs. Restart Requirements

Almost nothing in VuIO requires a restart. Of the 25 configuration settings, 21 apply to the running server dynamically in place:

| Behavior | Settings | Details |
|---|---|---|
| **Live Hot-Reload** (Instant) | 21 settings (Ports, network interfaces, auth settings, file monitoring, SSDP/mDNS announcements, etc.) | Editing `config.toml` or saving from the Admin tab immediately moves HTTP listeners, re-announces over SSDP/mDNS, swaps authentication, or starts/stops file monitors in place. |
| **Next Start** | `scan_on_startup`, `vacuum_on_startup` | These settings describe startup routines and have nothing to apply while running. Marked as **next start** in the UI. |
| **Restart Required** | `database.path`, `database.cache_mb` | The SQLite index database cannot be reopened underneath a running server. The Admin tab marks these and provides a restart button. |

### Dynamic Listener Relocation

Changing `server.port` or `server.interface` moves the listener while the server runs. The new address is bound before the old one is released; if the target port is already in use, the server stays safely on the existing port with an error rather than going offline. In-progress streaming connections to renderers are given a short grace period and then transitioned.

---

## Configuration Options

### `[server]`

General server identification and binding.

```toml
[server]
port = 8080               # HTTP server port
interface = "0.0.0.0"     # Network interface to bind (0.0.0.0 for all interfaces)
name = "VuIO Media Server" # DLNA & AirPlay friendly name
uuid = ""                 # Device UUID (auto-generated if empty)
ip = ""                   # Specific IP for DLNA announcements (optional, overrides auto-detection)
```

### `[network]`

Discovery protocols and LAN advertisement.

```toml
[network]
interface_selection = "Auto" # "Auto", "All", or specific interface name (e.g. "eth0")
announce_interval_seconds = 30 # SSDP announcement interval
mdns_enabled = true       # Also advertise over Bonjour / DNS-SD alongside SSDP
multicast_ttl = 4         # Multicast time-to-live
upnp_callback_allowed_networks = [] # Extra CIDRs allowed as UPnP event callbacks
```

### `[web_ui]`

Modern Svelte web client listener.

```toml
[web_ui]
enabled = true            # Serve the Svelte web interface (default: true)
port = 8090               # Listener port for web UI (must differ from server.port)
```

### Metadata Provider API Keys

Metadata providers that require an account (TheMovieDB, OMDb, etc.) read their API keys from the environment or `.env` file:

```bash
# In .env or shell environment:
VUIO_TMDB_API_KEY="your_themoviedb_api_key"
VUIO_OMDB_API_KEY="your_omdb_api_key"
```

A key can also be configured per-server from the **MediaInfo** tab in either web interface, which takes precedence. Providers without keys are gracefully skipped.

### `[media]`

Library indexing and playback settings.

```toml
[media]
scan_on_startup = true     # Scan directories on initial startup
watch_for_changes = true   # Enable real-time file system monitoring
cleanup_deleted_files = true # Auto-remove missing files from the database
autoplay_enabled = true    # Let renderers continue to next item in folder automatically
scan_playlists = true      # Discover and import M3U/M3U8 and PLS playlists
unavailable_root_grace_hours = 168 # Hours an offline library root keeps its indexed content (default: 7 days)
supported_extensions = ["mp4", "mkv", "avi", "mov", "mp3", "flac", "wav", "m4a", "jpg", "png"]
```

### `[[media_directories]]`

Configured library roots. Multiple directory blocks can be specified.

```toml
[[media_directories]]
path = "/path/to/movies"
recursive = true
validation_mode = "Warn"   # "Strict" (fail if missing), "Warn" (log warning), "Skip" (ignore)
exclude_patterns = ["*.tmp", ".*", "Thumbs.db"]
case_sensitive = false     # Omit to auto-detect filesystem behavior
```

### `[database]`

SQLite embedded database configuration.

```toml
[database]
path = "./config/database/media.db" # Database file location
vacuum_on_startup = false  # Compact SQLite database on startup
backup_enabled = false     # Enable automatic daily and shutdown backups
cache_mb = 128             # Megabytes of database index cached in memory
```

#### Default Database Paths by Platform

| Platform | Default Path |
|---|---|
| **Windows** | `[exe dir]\config\database\media.db` |
| **Linux** | `~/.local/share/vuio/media.db` (or `/var/lib/vuio/media.db` via systemd) |
| **macOS** | `~/Library/Application Support/vuio/media.db` |
| **Docker** | `/data/vuio.db` (or `VUIO_DB_PATH`) |

### `[management]`

Administrative security and access control.

```toml
[management]
enabled = false            # Require admin token for dashboard and management APIs
token_file = "admin.token" # File path to read admin token from
session_ttl_hours = 12     # Browser authentication session lifetime
allowed_networks = []      # Allowed CIDR blocks (empty restricts to private/loopback)
```

### `[transcode]`

Audio for renderers that cannot decode AC-3, Dolby Digital Plus or DTS. Those
codecs are licensed, and a television sold without the licence plays the picture
and nothing else.

When this is on, an item whose audio is one of the three is listed **twice** in
the browse response — the file as stored, and a decoded alternative at
`/media/{id}/transcode/audio.wav`. Both are offered and the renderer picks the
one it can play. A renderer that was already fine is unaffected.

```toml
[transcode]
enabled = true         # Offer the decoded alternative
audio_format = "lpcm"  # "lpcm" or "aac"
prefer = "original"    # Which resource is listed first: "original" or "transcoded"
max_concurrent = 2     # Simultaneous decodes; further requests are refused, not queued
```

| Key | Effect |
| --- | --- |
| `enabled` | Live. Off leaves the browse response exactly as it was. |
| `audio_format` | `lpcm` is uncompressed, carries an exact `Content-Length` and supports byte-range seeking, and costs about 1.5 Mbps. `aac` is roughly a tenth of that, at the price of a lossy re-encode, no `Content-Length` and no scrubbing. |
| `prefer` | Some renderers take the first resource without checking whether they can decode it. The default keeps the original first, which cannot make anything worse. Switch to `transcoded` if a set still plays silently. |
| `max_concurrent` | Decoding is the only CPU-bound work this server does. Past this ceiling a request gets `503` with `Retry-After` rather than joining a queue that would starve the streams already playing. |

Environment variables: `VUIO_TRANSCODE_ENABLED`, `VUIO_TRANSCODE_AUDIO_FORMAT`,
`VUIO_TRANSCODE_PREFER`, `VUIO_TRANSCODE_MAX_CONCURRENT`.

The section is read in every build, including one compiled without the decoders,
so a config file moved between builds is never rejected by the leaner one. A
build without them simply never advertises the alternative — see the feature
table in `crates/vuio-core/README.md`.

---

## Related Documentation

- [Security & Authentication Guide](security.md)
- [Docker Configuration & Environment Variables](docker.md)
- [API Reference](api.md)

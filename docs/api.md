# VuIO Server API Reference

The VuIO Media Server exposes a variety of endpoints for Web UI rendering, media streaming, UPnP/DLNA service discovery, Prometheus metrics, and Model Context Protocol (MCP) clients.

---

## 1. Frontend & Client REST APIs

These APIs are used by the Web UI to interact with media rendering devices.

### Discover local playback devices
Discovers DLNA, Chromecast, and compatible AirPlay renderers on the local network.
* **Endpoint**: `GET /api/renderers`
* **Response**: `200 OK`
  ```json
  [
    {
      "id": "chromecast:01234567",
      "friendly_name": "Living Room TV",
      "model_name": "Chromecast",
      "protocol": "chromecast",
      "capabilities": {
        "video": true,
        "audio": true,
        "image": true,
        "playlists": true,
        "controls": ["play", "pause", "stop"]
      }
    }
  ]
  ```

### Cast playlist to TV
Creates a temporary playlist and starts casting it to the selected playback device.
* **Endpoint**: `POST /api/cast/playlist`
* **Content-Type**: `application/json`
* **Request Payload**:
  ```json
  {
    "renderer_id": "chromecast:01234567",
    "folder_name": "Season 5",
    "file_ids": [343, 330, 331]
  }
  ```
* **Response**: `200 OK`
  ```json
  {
    "status": "playing",
    "playlist_id": 12,
    "tracks_count": 3,
    "current_index": 0,
    "current_file": "Kuhnya.s05.e01.tahiy.mkv",
    "queued_next_file": "Kuhnya.s05.e02.tahiy.mkv",
    "renderer": "Living Room TV",
    "renderer_id": "chromecast:01234567",
    "protocol": "chromecast",
    "media_url": "http://192.168.1.170:8080/media/343"
  }
  ```

### Read the configuration
Returns the full settings schema, the values currently in force, and which keys the
configuration file actually writes. Backs the dashboard's Admin tab.
* **Endpoint**: `GET /api/admin/config`
* **Response**: `200 OK`
  ```jsonc
  {
    "sections": [
      {
        "id": "network",
        "title": "Network",
        "blurb": "Discovery and advertisement on the local network.",
        "fields": [
          {
            "key": "network.mdns_enabled",
            "label": "Advertise over mDNS",
            "type": "bool",
            "impact": "live",
            "removable": true,
            "help": "Also announce the server over Bonjour/DNS-SD, alongside SSDP."
          }
        ]
      }
    ],
    // The file's value where the file sets one; otherwise the default in force.
    "values": { "network.mdns_enabled": true },
    // False means the key is absent from config.toml and `values` is showing a default.
    "present": { "network.mdns_enabled": false },
    // Settings the command line forces for this run, which the file cannot change until restart.
    "overrides": { "server.port": "9090" },
    // Libraries as the file writes them; absent optional keys stay absent.
    "directories": [{ "path": "/media", "recursive": true }],
    // The same libraries with defaults filled in, for display only.
    "effective_directories": [{ "path": "/media", "recursive": true, "validation_mode": "Warn" }],
    "runtime": {
      "config_path": "/opt/vuio/config/config.toml",
      "writable": true,
      "read_only_reason": null,
      "auth_enabled": false,
      "is_docker": false,
      "version": "0.0.47",
      // Where the server is actually accepting, which is what every advertised URL uses.
      "bound_addr": "0.0.0.0:8080",
      "desired_addr": null,
      "bind_error": null
    }
  }
  ```

Field `type` is one of `bool`, `int` (with `min`/`max`), `text`, `path`, `enum` (with
`options` and `free_form`), or `string_list`. `impact` is `live`, `next_start` — the setting
only describes startup, so there is nothing to apply now — or `restart`, meaning the running
server is still using the old value. Only `database.path` and `database.cache_mb` are
`restart`.
`removable: false` marks a key that must always carry a value, because `AppConfig`
declares no default for it. Some fields carry a `note` describing a caveat in what the
setting actually does.

### Write the configuration
Applies a set of changes to `config.toml`. The file is edited in place, so comments and
unrecognised keys survive. The result is parsed and validated before anything is written,
and the write is atomic, so a rejected change leaves the file untouched. The file watcher
then reloads it — the same path a hand edit takes.
* **Endpoint**: `POST /api/admin/config`
* **Request Payload**:
  ```jsonc
  {
    // Dotted key to value. `null` removes the key, restoring its default.
    "values": { "media.autoplay_enabled": false, "server.ip": null },
    // Optional. Replaces the whole [[media.directories]] array.
    "directories": [{ "path": "/media", "recursive": true }]
  }
  ```
* **Response**: `200 OK` — `{"saved": true, "impact": "live"}`, where `impact` is
  `no_change`, `live`, `next_start`, or `restart_required`.
* A save that changes `server.port` or `server.interface` also carries `moved`, reporting
  what the listener actually did rather than predicting it:
  ```jsonc
  { "state": "moved",  "serving": "0.0.0.0:9090", "port": 9090 }
  { "state": "failed", "serving": "0.0.0.0:8080", "desired": "0.0.0.0:80",
    "error": "Failed to bind to 0.0.0.0:80: Permission denied (os error 13)" }
  { "state": "pending", "serving": "0.0.0.0:8080" }
  ```
  On `failed` the server keeps serving on the old address and every advertised URL keeps
  naming it; `runtime.bound_addr`, `runtime.desired_addr` and `runtime.bind_error` report
  the same disagreement on subsequent `GET`s, so it survives a page reload.
* `400 Bad Request` — `{"error": "..."}` for an unknown key, a value of the wrong type, a
  failed validation, or an attempt to unset a key that has no default.
* `409 Conflict` — the configuration is not editable. This means a container configured by
  environment variables, whose config is a scratch file that a restart discards.

Command-line overrides (`--port`, `--name`, `-m`) do **not** make the configuration
read-only. They are layered over the file on every load, so they hold for the run while
the file stays editable. `overrides` in the `GET` response reports what they force, keyed
by config key, so a value saved for one of those keys can be shown as taking effect at the
next start rather than looking as though it failed. `values` deliberately reports the
file's value, not the running one, so a save cannot write an override back into the file.

### Restart the server
Runs the normal graceful shutdown and exits. The process only returns if something
supervises it — Docker, systemd or launchd.
* **Endpoint**: `POST /api/admin/restart`
* **Response**: `202 Accepted` — `{"stopping": true, "supervised": false}`

### Online media info

Fetches titles, synopses, ratings and artwork from public metadata services. This is the
only part of VuIO that contacts anything outside the local network, and it is only reached
by an explicit request to `/run`. Requires the `mediainfo` cargo feature (on by default)
and `mediainfo.enabled` in the configuration.

Five providers answer without an account — `tvmaze`, `musicbrainz` (with Cover Art Archive
for artwork), `jikan`, `anilist` and `kitsu`. Five more work once a credential is saved:
`tmdb`, `omdb`, `discogs`, `lastfm` and `genius`. Requests are paced per provider to the
rate limits each publisher documents; MusicBrainz's one-per-second ceiling makes a large
music library slow by design.

#### Status and progress
* **Endpoint**: `GET /api/admin/mediainfo`
* **Response**: `200 OK`
  ```jsonc
  {
    "enabled": true,
    "min_confidence": 60,
    "providers": [
      {
        "id": "tmdb", "label": "TheMovieDB", "group": "Movies & TV",
        "provides": "Movies, TV, posters, trailers and ratings.",
        "credential_label": "API key",
        "signup_url": "https://developer.themoviedb.org",
        "needs_credential": true,
        "has_credential": true,   // whether one is stored — never the value
        "enabled": true           // whether it is in mediainfo.providers
      }
    ],
    "job": {
      "running": true, "total": 1240, "processed": 318,
      "matched": 290, "low_confidence": 22, "failed": 6,
      "cancelled": false, "current": "Arrival.2016.1080p.mkv",
      "started_at": 1765000000
    },
    "stats": { "total": 318, "confident": 290, "low_confidence": 28, "with_artwork": 271 },
    "flagged": [
      { "media_file_id": 91, "confidence": 35, "provider": "tvmaze",
        "matched_title": "Some Show", "filename": "unknown.s01e02.mkv" }
    ]
  }
  ```
  Stored credentials are never returned by this or any other endpoint; `has_credential`
  is the only thing reported about them.

#### Save or clear a provider credential
* **Endpoint**: `POST /api/admin/mediainfo/credentials`
* **Body**: `{"provider": "tmdb", "token": "…"}` — an empty `token` clears the stored one.
* **Response**: `200 OK` — `{"saved": true, "has_credential": true}`
* **Errors**: `400 Bad Request` for an unknown provider, or for one that needs no account.

Credentials are kept in the database's `secrets` table rather than `config.toml`. Under
Docker the configuration is built from environment variables and `PUT /api/admin/config`
returns `409`, so a credential in the file would be unsettable in exactly the deployment
most likely to need one.

#### Start a library fetch
Walks every file with no usable record — never looked up, looked up by an older reader
version, or matched too weakly to trust — and returns as soon as the run is scheduled.
* **Endpoint**: `POST /api/admin/mediainfo/run`
* **Response**: `200 OK` — `{"started": true, "total": 1240}`
* **Errors**: `409 Conflict` if a run is already going, if the feature is off, or if no
  provider is enabled.

#### Cancel a running fetch
Stops after the item currently in flight; whatever was already matched stays.
* **Endpoint**: `POST /api/admin/mediainfo/cancel`
* **Response**: `200 OK` — `{"cancelled": true}`
* **Errors**: `409 Conflict` when nothing is running.

When a run finishes it publishes a ContentDirectory revision, so DLNA clients and the
dashboard both pick up the new titles, synopses and artwork without further prompting.

---

## 2. Media Streaming APIs

Endpoints for playing back video/audio and retrieving subtitles.

### Serve Media File
Streams the requested media file. Supports HTTP range requests (essential for scrubbing/seeking in video players).
* **Endpoint**: `GET /media/{id}`
* **Response Headers**:
  - `Content-Type`: Matching media file mime type (e.g. `video/x-matroska`, `audio/mpeg`)
  - `Accept-Ranges`: `bytes`
  - `TransferMode.dlna.org`: `Streaming`

### Serve Cover Art
Returns artwork for an item, trying three sources in order: an image file sitting beside
the media (`cover.jpg`, `folder.png`, `<basename>.webp`, …), artwork embedded in the file's
own tags, and finally a poster cached by the media info fetch. The local sources apply to
audio only; video reaches this endpoint through the cache, which is what gives a movie or
an episode a poster at all.
* **Endpoint**: `GET /media/{id}/cover`
* **Response**: `200 OK` with the image, or `404 Not Found` when no source has one.
* Also advertised to DLNA clients as `upnp:albumArtURI`.

### Serve Subtitles
Serves the sidecar subtitle track (`<media basename>.srt`) if one exists, in either of two
formats. Both return `404 Not Found` when there is no sidecar file.

* **Endpoint**: `GET /media/{id}/subtitle`
* **Response**: `200 OK`, `Content-Type: text/srt` — the file verbatim. This is what DLNA
  renderers consume (Samsung via the `CaptionInfo.sec` response header, LG and Panasonic via
  `pv:subtitleFileUri`).

* **Endpoint**: `GET /media/{id}/subtitle.vtt`
* **Response**: `200 OK`, `Content-Type: text/vtt; charset=utf-8` — the same file converted to
  WebVTT on the fly. Browsers' `<track>` element accepts WebVTT only, so this is the endpoint
  the dashboard player uses. Non-UTF-8 sidecars are decoded lossily.

---

## 3. Monitoring & System Health

Endpoints for health monitoring, log scraping, and metrics.

### System Metrics (Prometheus)
* **Endpoint**: `GET /metrics`
* **Response**: Prometheus exposition text format.

### Web Handler Metrics (JSON)
* **Endpoint**: `GET /metrics/json`
* **Response**: `200 OK` (JSON statistics)

### Health Check (Liveness)
* **Endpoint**: `GET /healthz`
* **Response**: `200 OK` `"OK"`

### Readiness Check
* **Endpoint**: `GET /readyz`
* **Response**: `200 OK` `"OK"`

### Loki Log Scraping
* **Endpoint**: `GET /logs?limit={num_lines}`
* **Response**: `200 OK` (plain text log lines)

---

## 4. Model Context Protocol (MCP) API

One endpoint, on the main port, letting an AI assistant browse, search and cast
the library. See the [MCP Integration Guide](mcp.md)
for how to connect a client, and `mcp/reference.json` for the tool schemas.

### MCP endpoint
* **Endpoint**: `POST /mcp`
* **Content-Type**: `application/json`
* **Protocol**: MCP `2026-07-28`. Clients that open with an `initialize`
  handshake are answered too, for `2025-11-25`, `2025-06-18` and `2025-03-26`.
* **Methods**: `server/discover`, `tools/list`, `tools/call`. Plus `initialize`
  and `ping` for the handshake-based revisions.
* **Required headers**:
  * `MCP-Protocol-Version` — must equal `params._meta`'s
    `io.modelcontextprotocol/protocolVersion`
  * `Mcp-Method` — must equal the body's `method`
  * `Mcp-Name` — on `tools/call`, must equal `params.name`

  A mismatch returns `400` with JSON-RPC error `-32020`; an unsupported version
  returns `400` with `-32022` and the list of versions the server does speak.
* **Origin**: validated when present, to prevent DNS rebinding. Clients that
  send no `Origin` — which is most of them — are unaffected.
* **Auth**: behind the management middleware, plus `[mcp].require_auth` for a
  bearer token even when management authentication is off.
* **`GET` / `DELETE`**: `405`. Both belonged to the session-based revisions of
  the transport; this one has no sessions.
* **Body limit**: 256 KiB, shared with the other JSON endpoints.

Notifications (a message with no `id`) are answered `202 Accepted` with no body.
Everything else returns `200` with the JSON-RPC response in the body.

### stdio bridge

For clients that launch a local process instead of calling an endpoint:

```bash
vuio mcp --url http://nas.local:8080 --token-file ~/.vuio/admin.token
```

It reads JSON-RPC on stdin and writes answers to stdout, forwarding to the
server's `/mcp`. It serves nothing itself.

---

## 5. UPnP / DLNA Core Services

These endpoints implement the UPnP MediaServer:1 and ContentDirectory:1 protocols for TV/Receiver client discovery.

* `GET /description.xml` - Device XML definition.
* `GET /ContentDirectory.xml` - ContentDirectory SCPD.
* `POST /control/ContentDirectory` - ContentDirectory control endpoint (SOAP actions).
* `GET /ConnectionManager.xml` - ConnectionManager SCPD.
* `POST /control/ConnectionManager` - ConnectionManager control endpoint.
* `GET /X_MS_MediaReceiverRegistrar.xml` - MediaReceiverRegistrar SCPD.
* `POST /control/X_MS_MediaReceiverRegistrar` - MediaReceiverRegistrar control endpoint.

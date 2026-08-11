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
            "impact": "restart",
            "removable": true,
            "help": "Also announce the server over Bonjour/DNS-SD, alongside SSDP."
          }
        ]
      }
    ],
    "values": { "network.mdns_enabled": true },
    // False means the key is absent from config.toml and `values` is showing a default.
    "present": { "network.mdns_enabled": false },
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
      "version": "0.0.42"
    }
  }
  ```

Field `type` is one of `bool`, `int` (with `min`/`max`), `text`, `path`, `enum` (with
`options` and `free_form`), or `string_list`. `impact` is `live` or `restart`.
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
  `no_change`, `live`, or `restart_required`.
* `400 Bad Request` — `{"error": "..."}` for an unknown key, a value of the wrong type, a
  failed validation, or an attempt to unset a key that has no default.
* `409 Conflict` — the configuration is not editable: a container configured by
  environment variables, or a run started with command-line overrides. Both use a scratch
  file that a restart discards.

### Restart the server
Runs the normal graceful shutdown and exits. The process only returns if something
supervises it — Docker, systemd or launchd.
* **Endpoint**: `POST /api/admin/restart`
* **Response**: `202 Accepted` — `{"stopping": true, "supervised": false}`

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

## 4. Model Context Protocol (MCP) APIs

Endpoints used by AI agents (e.g. LM Studio, Claude Desktop) to connect to the server.

### SSE Session Stream
Establishes the Server-Sent Events stream, which sends back a unique `client_id`.
* **Endpoint**: `GET /sse`
* **Response**: `text/event-stream`

### MCP JSON-RPC Endpoint
Post JSON-RPC messages to command the server.
* **Endpoint**: `POST /mcp/message?client_id={uuid}`
* **Content-Type**: `application/json`

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

# VuIO Server API Reference

The VuIO Media Server exposes a variety of endpoints for Web UI rendering, media streaming, UPnP/DLNA service discovery, Prometheus metrics, and Model Context Protocol (MCP) clients.

---

## 1. Frontend & Client REST APIs

These APIs are used by the Web UI to interact with media rendering devices.

### Discover local smart TVs
Discovers UPnP/DLNA MediaRenderer devices on the local network.
* **Endpoint**: `GET /api/tvs`
* **Response**: `200 OK`
  ```json
  [
    "Bedroom TV",
    "Living Room TV"
  ]
  ```

### Cast playlist to TV
Creates a temporary playlist and starts casting it to the selected TV screen.
* **Endpoint**: `POST /api/cast/playlist`
* **Content-Type**: `application/json`
* **Request Payload**:
  ```json
  {
    "tv_name": "Bedroom TV",
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
    "tv": "Bedroom TV",
    "media_url": "http://192.168.1.170:8080/media/343"
  }
  ```

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
Serves subtitle tracks if available for the given media file.
* **Endpoint**: `GET /media/{id}/subtitle`
* **Response**: `200 OK` (WebVTT format)

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

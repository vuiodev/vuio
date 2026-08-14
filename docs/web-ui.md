# Web Interface & Dashboard Guide

VuIO serves two browser interfaces from one server. They are two front ends, not two servers: both execute over the same state and in-process database handle, so nothing is duplicated or reverse-proxied.

---

## Interface Overview

| Interface | Default Address | Description |
|---|---|---|
| **Modern Web UI** | `http://<server-ip>:8090` | A modern Svelte-based interface featuring folder navigation, inline video/audio players, cast control, and administrative controls. Built from [vuiodev/vuio-web](https://github.com/vuiodev/vuio-web). |
| **Built-in Dashboard** | `http://<server-ip>:8080` | Lightweight built-in dashboard hosted directly on the main DLNA/streaming port. |

---

## Key Features

### 1. High-Performance Folder Browsing

Folder browsing queries are answered by the server from an indexed parent directory cache (`/api/browse`). Opening a directory with millions of files incurs negligible overhead compared to a directory of ten items.

### 2. Built-in Media Player & Explorer

- **Video Player**: Stream MP4, MKV, and WebM video with subtitle track selection and HTTP Range seek support.
- **Audio Player**: High-fidelity playback with cover art rendering, metadata display, and playlist queueing.
- **Image Gallery**: Fast image viewer for scanned photo collections.

### 3. Discovered Renderers & Casting

Control playback on smart devices across your home network directly from your browser:
- Cast videos, audio tracks, and full playlists to **Chromecast / Google TV**, **DLNA / UPnP MediaRenderers**, and compatible **AirPlay** devices.
- Control volume, seek positions, play, pause, and stop commands in real time.

### 4. Instant Search

Client-side filtering and server-side full-text search across all indexed filenames, directories, and metadata tags with instant debounced updates as you type.

### 5. In-Browser Admin Tab

The **Admin** tab allows you to view and edit every setting accepted by `config.toml`:
- Changes are written back to `config.toml` in place while preserving file comments.
- 21 of 25 settings apply immediately without restarting the server (including moving listeners, swapping auth, or updating monitoring directories).
- Non-configured options display the active default value.

---

## Configuration

In `config.toml`:

```toml
[web_ui]
enabled = true  # Set to false to disable the Svelte interface
port = 8090     # Port for the web interface (must differ from server.port)
```

To configure via environment variables (e.g. in Docker):
- `VUIO_WEB_UI=true` / `VUIO_WEB_UI=false`
- `VUIO_WEB_PORT=8090`

---

## Related Documentation

- [Configuration Reference](configuration.md)
- [AI Agent & MCP Integration](mcp.md)
- [API Reference](api.md)

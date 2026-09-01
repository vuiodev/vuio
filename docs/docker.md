# Docker Deployment Guide

VuIO publishes official multi-architecture container images to GitHub Container Registry (`ghcr.io/vuiodev/vuio`) supporting `linux/amd64` and `linux/arm64`.

> [!NOTE]
> SSDP/UPnP auto-discovery relies on multicast UDP (239.255.255.250:1900). For local devices (Smart TVs, AV receivers, Chromecast) to discover the server, container `--network host` mode is required.
> VuIO can share UDP port 1900 with other multicast-aware SSDP services that enable compatible socket reuse. A service that reserves the port exclusively will still prevent SSDP discovery from starting.
> Docker for macOS runs inside a lightweight virtual machine that blocks host multicast routing; on macOS, running the native binary (`brew install vuio`) is recommended for DLNA discovery.

---

## Quick Start

### Docker Compose

Create a `docker-compose.yml` file:

```yaml
version: '3.8'

services:
  vuio:
    image: ghcr.io/vuiodev/vuio:latest
    container_name: vuio-server
    restart: unless-stopped
    network_mode: host
    cap_add:
      - NET_ADMIN
      - NET_RAW
    environment:
      - VUIO_IP=192.168.1.100       # Set to your host IP
      - VUIO_PORT=8080
      - VUIO_WEB_PORT=8090
      - VUIO_MEDIA_DIRS=/media/movies,/media/music,/media/pictures
    volumes:
      - ./vuio-config:/config
      - /path/to/movies:/media/movies:ro
      - /path/to/music:/media/music:ro
      - /path/to/pictures:/media/pictures:ro
```

Run with:
```bash
docker-compose up -d
```

### Docker CLI (`docker run`)

```bash
docker run -d \
  --name vuio-server \
  --restart unless-stopped \
  --network host \
  --cap-add NET_ADMIN \
  --cap-add NET_RAW \
  -v /path/to/media:/media:ro \
  -v ./vuio-config:/config \
  -e VUIO_IP=192.168.1.100 \
  -e VUIO_PORT=8080 \
  -e VUIO_WEB_PORT=8090 \
  -e VUIO_MEDIA_DIRS=/media \
  -e VUIO_DB_PATH=/data/vuio.db \
  ghcr.io/vuiodev/vuio:latest
```

---

## Volume Mounting Patterns

### Single Directory
```yaml
volumes:
  - ./vuio-config:/config
  - /path/to/media:/media:ro
```

### Multiple Directories
```yaml
volumes:
  - ./vuio-config:/config
  - /home/user/Movies:/media/movies:ro
  - /home/user/Music:/media/music:ro
  - /home/user/Pictures:/media/pictures:ro
  - /mnt/nas/media:/media/nas:ro
environment:
  - VUIO_MEDIA_DIRS=/media/movies,/media/music,/media/pictures,/media/nas
```

### Network Storage (NFS / SMB)
```yaml
volumes:
  - type: bind
    source: /mnt/nas/media
    target: /media
    read_only: true
```

---

## Environment Variables Reference

A container is configured via environment variables. In Docker environments, the dashboard's Admin tab is read-only so that configuration changes are made declaratively in your container definition.

| Variable | Default | Description |
|---|---|---|
| `VUIO_IP` | - | **Required.** Host IP for DLNA announcements and stream URLs |
| `VUIO_PORT` | `8080` | HTTP streaming and SOAP server port |
| `VUIO_WEB_PORT` | `8090` | Modern Svelte web interface port |
| `VUIO_WEB_UI` | `true` | Serve the web UI (`false`/`0`/`off` to disable) |
| `VUIO_INTERFACE` | `0.0.0.0` | IP address the HTTP server binds |
| `VUIO_SERVER_NAME` | `VuIO` | DLNA & AirPlay server friendly name |
| `VUIO_UUID` | random | Device UUID (set a fixed UUID for persistence across restarts) |
| `VUIO_MEDIA_DIRS` | `/media` | Comma-separated list of media directories to scan |
| `VUIO_SUPPORTED_EXTENSIONS` | Built-in media formats | Comma-separated global list of file extensions to scan (for example `mp4,mkv,flac`; leading dots are optional) |
| `VUIO_SCAN_ON_STARTUP` | `true` | Automatically scan media directories on container startup |
| `VUIO_WATCH_CHANGES` | `true` | Monitor file system changes in real time |
| `VUIO_CLEANUP_DELETED` | `true` | Automatically remove deleted files from database |
| `VUIO_SCAN_PLAYLISTS` | `true` | Auto-import M3U/M3U8 and PLS playlists |
| `VUIO_DB_PATH` | `/data/vuio.db` | SQLite database file location |
| `VUIO_DB_VACUUM` | `false` | Compact SQLite database index on startup |
| `VUIO_DB_BACKUP` | `false` | Back up the index at startup, daily, and at shutdown |
| `VUIO_DB_CACHE_MB` | `128` | Memory cache allocated for the SQLite index (in MB) |
| `VUIO_MULTICAST_TTL` | `4` | Multicast socket time-to-live |
| `VUIO_ANNOUNCE_INTERVAL`| `30` | SSDP alive announcement interval (seconds) |
| `VUIO_MDNS` | `true` | Advertise over Bonjour/DNS-SD as well as SSDP |
| `VUIO_AUTOPLAY` | `true` | Let renderers continue to the next item in a folder |
| `VUIO_UNAVAILABLE_ROOT_GRACE_HOURS` | `168` | Hours an offline root keeps indexed content before pruning |
| `VUIO_AUTH` | `false` | Enable web dashboard administrative authentication |
| `VUIO_MANAGEMENT_ENABLED` | `false` | Same as `VUIO_AUTH`; requires admin token |
| `VUIO_ADMIN_TOKEN` | - | Pre-configured admin password token for sign in |
| `VUIO_ADMIN_TOKEN_FILE`| - | Read token from file instead of `admin.token` |
| `VUIO_ADMIN_SESSION_TTL_HOURS` | `12` | Web admin session lifetime (hours) |
| `VUIO_MANAGEMENT_ALLOWED_NETWORKS` | - | Comma-separated CIDRs allowed to reach management endpoints |
| `VUIO_UPNP_CALLBACK_ALLOWED_NETWORKS`| - | Extra CIDRs allowed as UPnP event callbacks |
| `VUIO_TMDB_API_KEY` | - | TheMovieDB API key for automatic metadata enrichment |
| `VUIO_MCP_ENABLED` | `true` | Enable Model Context Protocol AI assistant endpoint (`POST /mcp`) |
| `VUIO_MCP_READ_ONLY` | `false` | Restrict MCP tools to browsing only (disable casting/mutations) |
| `VUIO_MCP_REQUIRE_AUTH`| `false` | Require admin token for MCP endpoint even when auth is off |

---

## Helpful Helper Commands

### Find Your Host IP

```bash
# Linux
ip route get 1.1.1.1 | grep -oP 'src \K[0-9.]+'

# macOS
ipconfig getifaddr en0

# Windows PowerShell
(Get-NetIPAddress -AddressFamily IPv4 -InterfaceAlias "Wi-Fi*","Ethernet*").IPAddress
```

### Generate a Persistent UUID

```bash
# Linux / macOS
uuidgen

# Windows PowerShell
[System.Guid]::NewGuid()
```

---

## Related Documentation

- [Kubernetes & Helm Deployment](kubernetes.md)
- [Configuration Reference](configuration.md)
- [Security & Authentication Guide](security.md)

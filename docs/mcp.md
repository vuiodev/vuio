# AI Agent & Model Context Protocol (MCP) Integration

VuIO implements the **Model Context Protocol (MCP)** specification, allowing AI assistants (such as Claude, ChatGPT, and autonomous agents) to browse and search your media library, construct playlists, and cast media to TVs and smart speakers on your local network.

---

## Connecting Claude

### 1. Claude CLI Plugin (Recommended)

The quickest route is the official plugin, which provides the server connection, an interactive skill, and `/cast`, `/playlist`, and `/library` slash commands:

```bash
claude plugin marketplace add vuiodev/vuio
claude plugin install vuio@vuio
```

Configure your server target with environment variables:
- `VUIO_URL`: Server address (default: `http://localhost:8080`)
- `VUIO_TOKEN`: Administration token (required if authentication is enabled)

### 2. Direct HTTP MCP Transport

You can connect Claude directly to the HTTP MCP endpoint:

```bash
claude mcp add --transport http vuio http://localhost:8080/mcp \
  --header "Authorization: Bearer $(cat admin.token)"
```

### 3. Claude Desktop Application

Claude Desktop runs MCP servers via local stdio processes using a bundle:

```bash
# Build the .mcpb bundle
cargo build --release
./claude/mcpb/build.sh          # Generates claude/mcpb/dist/vuio.mcpb
```

Double-click the generated `.mcpb` bundle to install it into Claude Desktop, then specify your server URL. The bundle invokes `vuio mcp`, which bridges stdio JSON-RPC requests to a running VuIO server.

### 4. CLI Stdio Bridge (`vuio mcp`)

Any MCP client that supports stdio commands can interface with a remote or local VuIO instance:

```bash
vuio mcp --url http://nas.local:8080 --token-file ~/.vuio/admin.token
```

---

## The `/mcp` HTTP Endpoint

VuIO serves a single HTTP endpoint at `POST /mcp` speaking MCP version **2026-07-28** (with backward compatibility for `2025-11-25`, `2025-06-18`, and `2025-03-26`).

- **Version validation**: Requests must supply the protocol version in both `_meta` and the `MCP-Protocol-Version` HTTP header.
- **Method headers**: `Mcp-Method` must match the JSON-RPC `method`, and `Mcp-Name` must match the tool name on `tools/call`.
- **Stateless transport**: Sessionless HTTP transport (GET and DELETE return `405 Method Not Allowed`).

### Example: Discover Tools

```bash
curl -X POST http://localhost:8080/mcp \
  -H 'Content-Type: application/json' \
  -H 'MCP-Protocol-Version: 2026-07-28' \
  -H 'Mcp-Method: server/discover' \
  -d '{"jsonrpc":"2.0","id":1,"method":"server/discover",
       "params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}'
```

### Example: Search Media

```bash
curl -X POST http://localhost:8080/mcp \
  -H 'Content-Type: application/json' \
  -H 'MCP-Protocol-Version: 2026-07-28' \
  -H 'Mcp-Method: tools/call' \
  -H 'Mcp-Name: search_media' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call",
       "params":{"name":"search_media","arguments":{"query":"blade runner"},
                 "_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28"}}}'
```

*(Add `-H "Authorization: Bearer <token>"` if authentication is enabled)*

---

## Configuration

In `config.toml`:

```toml
[mcp]
enabled = true        # Enable the MCP endpoint (VUIO_MCP_ENABLED)
read_only = false     # Restrict to browsing only; hide casting and playlist creation (VUIO_MCP_READ_ONLY)
require_auth = false  # Require token even when management auth is disabled (VUIO_MCP_REQUIRE_AUTH)
```

> [!WARNING]
> MCP tools can delete playlists and trigger playback on physical devices. If your server is reachable over a public or untrusted network, set `require_auth = true` or enable `[management]` authentication.

---

## Available MCP Tools Reference

Every query result returns fully qualified `stream_url`, `cover_url`, and `subtitle_url` fields so AI agents can immediately return playable URLs.

### Media Library Tools

| Tool | Parameters | Description |
|---|---|---|
| `get_server_stats` | — | Total counts by media type, library byte size, playlist count, base URL |
| `list_library_roots` | — | Configured media roots and their online/offline availability status |
| `search_media` | `query`, `category?`, `limit?`, `cursor?` | Ranked full-text search across filenames, tags, and fetched synopses |
| `list_media` | `category?`, `limit?`, `cursor?` | Paginate through all indexed files |
| `browse_folder` | `path`, `category?`, `offset?`, `limit?` | Inspect subfolders and media files in a specific directory |
| `get_media_info` | `file_id` | Full audio/video metadata, playable URLs, and enriched synopsis/ratings |
| `list_music_categories` | `kind`, `artist?`, `genre?` | Aggregated artists, albums, album artists, genres, or years with counts |
| `find_music` | `artist?`, `album_artist?`, `album?`, `genre?`, `year?`, `limit?` | Search tracks matching exact tag parameters |

### Playlist Tools

| Tool | Parameters | Description |
|---|---|---|
| `list_playlists` | — | List all custom playlists with track counts |
| `get_playlist_tracks` | `playlist_id` | Retrieve tracks for a playlist in playback order |
| `create_playlist` | `name`, `description?` | Create a new empty playlist |
| `add_to_playlist` | `playlist_id`, `media_file_ids` | Append media file IDs to a playlist |
| `reorder_playlist` | `playlist_id`, `media_file_ids` | Replace the running order of a playlist |
| `remove_from_playlist`| `playlist_id`, `media_file_id` | Remove a single track from a playlist |
| `delete_playlist` | `playlist_id` | Permanently delete a playlist and its associations |

### Device & Casting Tools

| Tool | Parameters | Description |
|---|---|---|
| `list_renderers` | — | Discovered DLNA, Chromecast, and AirPlay devices with stable IDs |
| `get_playback_status` | `renderer_id?` | Current playback state, track info, and active renderer metadata |
| `cast_media_to_renderer` | `file_id`, `renderer_id` | Stream an individual media file to a target renderer |
| `cast_playlist_to_renderer` | `playlist_id`, `renderer_id`, `track_index?` | Cast an entire playlist with automatic track advancement |
| `cast_folder_to_renderer` | `path`, `renderer_id`, `media?` | Cast all media within a directory sequentially without creating a playlist |
| `control_renderer` | `renderer_id`, `action` | Control playback state (`play`, `pause`, `stop`) |

---

## Related Documentation

- [API Reference](api.md)
- [Security & Authentication Guide](security.md)
- [Web Interface & Dashboard Guide](web-ui.md)

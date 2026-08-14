# Audio & Music Features

VuIO includes built-in music indexing, tag parsing, playlist management, and DLNA audio streaming capabilities.

---

## Metadata Extraction

When scanning audio files, VuIO automatically reads native container tags:
- **Track Information**: Title, Artist, Album, Album Artist
- **Classification**: Genre, Release Year, Track Number, Disc Number
- **Audio Properties**: Duration, Sample Rate, Bit Depth, Channels
- **Fallback Handling**: If ID3 or Vorbis tags are missing, VuIO intelligently parses title and artist names from the file path.

---

## Supported Audio Formats

| Category | Formats / Codecs |
|---|---|
| **Lossless** | FLAC (`.flac`), WAV (`.wav`), AIFF (`.aiff`, `.aif`), ALAC (`.m4a`) |
| **Lossy** | MP3 (`.mp3`), AAC / M4A (`.aac`, `.m4a`), Ogg Vorbis (`.ogg`), Opus (`.opus`), WMA (`.wma`) |

---

## Playlist Support

VuIO automatically detects and imports playlists located within your indexed media roots:
- **M3U / M3U8**: Standard and extended UTF-8 playlists.
- **PLS**: Winamp and iTunes compatible playlist format.

Playlists are exposed over DLNA under the **Playlists** folder and can be queried or cast directly via REST and MCP tools.

To configure playlist scanning in `config.toml`:
```toml
[media]
scan_playlists = true
```

---

## Recommended Directory Structure

For optimal tag aggregation and browsing by Artist / Album:

```
/music/
├── Pink Floyd/
│   └── The Dark Side of the Moon (1973)/
│       ├── 01 - Speak to Me.flac
│       ├── 02 - Breathe.flac
│       └── cover.jpg
└── Playlists/
    ├── Favorites.m3u8
    └── Chill.pls
```

---

## Related Documentation

- [Web Interface & Dashboard Guide](web-ui.md)
- [Model Context Protocol (MCP) Integration](mcp.md)
- [API Reference](api.md)

---
name: vuio
description: Browse, search and cast a VuIO media library, build playlists, and control DLNA, Chromecast and AirPlay devices on the local network. Use when the user asks about their media library, wants to play or cast something to a TV or speaker, asks what is playing, or wants a playlist built.
---

# Driving a VuIO media server

VuIO indexes a local media library and can push playback to devices on the same
network. The tools are the whole interface; this file is the part the schemas
cannot tell you — which order to call things in, and what the answers mean.

## Two facts that shape everything

**`renderer_id` is stable; `friendly_name` is not.** People rename their TVs, and
two speakers in a house are often called the same thing. Always resolve a name to
an id with `list_renderers` first, and pass the id everywhere. When the user says
"the living room TV", match it against `friendly_name` yourself and say which
device you picked.

**`file_id` is how media is addressed.** Every listing and search result carries
one, along with a `stream_url` that plays in a browser. When the user wants a
link rather than playback, give them the `stream_url`.

## Finding something

Start with `get_server_stats` if you have no idea what the library holds — it
answers "how much of what" in one call.

Then pick the right tool for the question:

| The user asks | Use |
| --- | --- |
| "do I have anything by X", "find the Blade Runner one" | `search_media` |
| "what artists do I have", "which albums by X" | `list_music_categories` |
| "play everything by X", "all the jazz" | `find_music` |
| "what's in my movies folder" | `list_library_roots` then `browse_folder` |
| "tell me about this file" | `get_media_info` |

`search_media` is ranked full-text across filenames, tags and any synopsis
fetched from a metadata provider. It matches every word, and the last word also
matches as a prefix — so `beethov sym` finds Beethoven symphonies. Punctuation is
ignored, so search for `AC/DC` exactly as written.

`list_music_categories` and `find_music` match tag values **exactly**. Use
`list_music_categories` first to learn the spelling, then `find_music` with it.
Going straight to `find_music` with a guessed spelling usually returns nothing.

`browse_folder` needs a real path. Never invent one: call `list_library_roots`,
then follow the `path` of each folder in the result. Paths outside the configured
roots are refused.

## Casting

The sequence is always: **discover → resolve → cast → verify.**

1. `list_renderers` — devices are discovered on the network, so this can be empty
   or slow the first time. If it comes back empty, say so plainly: the device may
   be off, on another network segment, or not yet announced. Do not retry in a
   loop.
2. Resolve the file or playlist to an id.
3. `cast_media_to_renderer`, `cast_playlist_to_renderer` or
   `cast_folder_to_renderer`.
4. `get_playback_status` to confirm it actually started. A cast that returns
   successfully has been *accepted*; it has not necessarily begun.

`control_renderer` takes `play`, `pause` and `stop` only. There is no seek and no
volume — say so rather than pretending otherwise.

Casting a playlist or a folder advances automatically as each track finishes; the
server keeps the queue moving without further calls.

**Casting is real-world action.** It starts something playing on a device in
someone's home, possibly a room they are not in. Confirm the device before
casting when the user was vague about which one, and never cast speculatively to
"see if it works".

## Playlists

`create_playlist` → `add_to_playlist` → optionally `cast_playlist_to_renderer`.

Pass every track to `add_to_playlist` in one call, in the order you want them
played — the array order is the running order. Calling it once per track is
slower and gets the order wrong if any call fails.

To build a playlist from a description ("something calm for dinner", "90s rock"),
resolve the description into tracks first with `find_music` or `search_media`,
show the user what you found, and only then create the playlist. A playlist built
from a wrong guess is tedious to unpick.

`delete_playlist` and `remove_from_playlist` destroy stored data and cannot be
undone. They do not touch the media files themselves — say so, because people
assume otherwise and hesitate.

## When a tool refuses

Errors are written to be acted on. A refusal naming another tool means call that
tool: `browse_folder` pointing at `list_library_roots` means the path was outside
the library, not that the library is empty.

If a whole category of tool is missing from the tool list, the server was built
or configured without it — `[mcp].read_only` hides everything that changes
anything, and a build without casting has no renderer tools. Tell the user that
rather than reporting a failure.

## Connecting

The tools reach the server over HTTP at `${VUIO_URL:-http://localhost:8080}/mcp`.
If every call fails to connect, the server is not running or is on another
address; `VUIO_URL` and `VUIO_TOKEN` are the two environment variables that
configure it.

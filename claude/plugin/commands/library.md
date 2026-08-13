---
description: Summarise what is in the media library
---

Give the user a picture of their library.

1. `get_server_stats` for the totals: how many videos, tracks and images, how
   much disk they take, and how many playlists exist.
2. `list_library_roots` for where it all lives, and whether any root is currently
   unavailable — an unplugged drive stays indexed but cannot be played, which is
   worth flagging.
3. `list_music_categories` with `kind: "artist"` and `kind: "genre"` for a sense
   of the music, if there is any.
4. `list_playlists` if any exist.

Summarise it in prose, not as a dump of every call's output. Mention anything
that looks wrong — a root marked unavailable, a library with no tags at all — and
what could be done about it.

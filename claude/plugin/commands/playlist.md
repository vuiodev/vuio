---
description: Build a playlist in the media library from a description
argument-hint: <what the playlist should contain>
---

Build a playlist from this brief: **$ARGUMENTS**

1. Turn the brief into tracks. `find_music` for exact artists, albums, genres or
   years; `search_media` when the brief is loose. Use `list_music_categories`
   first if you need the library's own spelling of an artist or genre.
2. Show the user the tracks you found, in the order you propose to play them,
   before creating anything. A playlist built from a wrong reading of the brief
   is tedious to unpick.
3. `create_playlist` with a name that reflects the brief, then one
   `add_to_playlist` call with every id in playback order.
4. Offer to cast it: `list_renderers`, then `cast_playlist_to_renderer`.

If the library has too little to satisfy the brief, say what you found and how
short it falls rather than padding it with loosely related tracks.

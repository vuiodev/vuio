---
description: Cast something from the media library to a device on the network
argument-hint: <what to play> [to <device>]
---

Cast what the user asked for to a device on their network.

The request is: **$ARGUMENTS**

Work in this order, and report what you picked at each step so the user can stop
you if you guessed wrong:

1. `list_renderers` to see what is reachable. If the user named a device, match
   it against `friendly_name` and use that device's `id`. If they did not, and
   there is more than one, ask which — do not pick for them.
2. Resolve what to play. `search_media` for a title, `find_music` for an artist,
   album or genre, `browse_folder` for a folder they named. Show the match before
   casting it.
3. Cast: `cast_media_to_renderer` for one file, `cast_playlist_to_renderer` for a
   playlist, `cast_folder_to_renderer` for a whole folder.
4. `get_playback_status` to confirm it started, and say what is now playing where.

If nothing matches, say what you searched for and suggest `list_music_categories`
to see what the library actually contains. Do not cast something approximate.

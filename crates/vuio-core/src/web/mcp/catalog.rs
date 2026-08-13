//! The tool catalog.
//!
//! One table, built once per `tools/list`, filtered by what this build and this
//! configuration can actually do. A client must never be shown a tool it cannot
//! call: an agent that discovers `cast_media_to_renderer` on a server compiled
//! without casting has no way to learn that from the error it eventually gets.

/// What a tool does to the world, which decides both its annotations and
/// whether `[mcp].read_only` hides it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Effect {
    /// Answers from the index. Safe to call speculatively and safe to repeat.
    Read,
    /// Changes stored data, reversibly.
    Write,
    /// Destroys stored data.
    Destroy,
    /// Drives a device on the local network.
    Device,
}

impl Effect {
    fn annotations(self) -> serde_json::Value {
        let (read_only, destructive, idempotent, open_world) = match self {
            Self::Read => (true, false, true, false),
            Self::Write => (false, false, false, false),
            Self::Destroy => (false, true, true, false),
            // `openWorldHint`: the effect lands on a TV or speaker, not in the
            // database, so it is neither undoable nor confined to this server.
            Self::Device => (false, false, false, true),
        };
        serde_json::json!({
            "readOnlyHint": read_only,
            "destructiveHint": destructive,
            "idempotentHint": idempotent,
            "openWorldHint": open_world,
        })
    }

    /// Whether `[mcp].read_only` leaves this tool visible.
    fn survives_read_only(self) -> bool {
        self == Self::Read
    }

    /// Whether this tool needs a compiled-in cast provider to work at all.
    fn needs_casting(self) -> bool {
        self == Self::Device
    }
}

struct ToolSpec {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    effect: Effect,
    input: fn() -> serde_json::Value,
    output: fn() -> serde_json::Value,
}

/// Whether a tool is advertised by this build and this configuration.
///
/// `tools/call` consults it before dispatching, so a name learned from another
/// server — or guessed — cannot reach a handler that `tools/list` deliberately
/// withheld.
pub(super) fn tool_is_available(name: &str, read_only: bool) -> bool {
    TOOLS
        .iter()
        .any(|tool| tool.name == name && is_visible(tool, read_only))
}

fn is_visible(tool: &ToolSpec, read_only: bool) -> bool {
    if read_only && !tool.effect.survives_read_only() {
        return false;
    }
    if tool.effect.needs_casting() && !cfg!(feature = "casting") {
        return false;
    }
    true
}

pub(super) fn get_tools_list(read_only: bool) -> serde_json::Value {
    let tools: Vec<serde_json::Value> = TOOLS
        .iter()
        .filter(|tool| is_visible(tool, read_only))
        .map(|tool| {
            serde_json::json!({
                "name": tool.name,
                "title": tool.title,
                "description": tool.description,
                "inputSchema": (tool.input)(),
                "outputSchema": (tool.output)(),
                "annotations": tool.effect.annotations(),
            })
        })
        .collect();
    serde_json::json!({ "tools": tools })
}

// ──────────────────────────────────────────
// Schema fragments
// ──────────────────────────────────────────

/// A tool that takes nothing.
///
/// `additionalProperties: false` rather than an empty `properties` map: it says
/// "only an empty object", which is what these mean, instead of "any object".
fn no_input() -> serde_json::Value {
    serde_json::json!({ "type": "object", "additionalProperties": false })
}

/// The shape `media_file_view_to_json` produces.
///
/// Described loosely on purpose — every field but `id` can be absent, because
/// the record is whatever the tag reader managed to find in the file.
fn media_file_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "id": { "type": "integer" },
            "filename": { "type": "string" },
            "path": { "type": "string" },
            "mime_type": { "type": "string" },
            "size_bytes": { "type": "integer" },
            "size_human": { "type": "string" },
            "duration_seconds": { "type": ["number", "null"] },
            "title": { "type": ["string", "null"] },
            "artist": { "type": ["string", "null"] },
            "album": { "type": ["string", "null"] },
            "album_artist": { "type": ["string", "null"] },
            "genre": { "type": ["string", "null"] },
            "composer": { "type": ["string", "null"] },
            "track_number": { "type": ["integer", "null"] },
            "disc_number": { "type": ["integer", "null"] },
            "year": { "type": ["integer", "null"] },
            "codec": { "type": ["string", "null"] },
            "sample_rate": { "type": ["integer", "null"] },
            "channels": { "type": ["integer", "null"] },
            "bits_per_sample": { "type": ["integer", "null"] },
            "bit_rate": { "type": ["integer", "null"] },
            "subtitle_available": { "type": "boolean" },
            "stream_url": {
                "type": "string",
                "description": "Direct HTTP URL for playback, with range support"
            },
            "cover_url": { "type": ["string", "null"] },
            "subtitle_url": { "type": ["string", "null"] }
        },
        "required": ["id", "filename", "path", "mime_type", "stream_url"]
    })
}

fn file_page_schema(count_key: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            count_key: { "type": "integer" },
            "files": { "type": "array", "items": media_file_schema() },
            "next_cursor": {
                "type": ["string", "null"],
                "description": "Pass back as `cursor` for the next page; null when exhausted"
            }
        },
        "required": [count_key, "files"]
    })
}

fn category_enum(description: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "string",
        "enum": ["all", "audio", "video", "image", "radio"],
        "description": description
    })
}

fn status_schema(properties: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "type": "object", "properties": properties })
}

// ──────────────────────────────────────────
// The catalog
// ──────────────────────────────────────────

/// Ordering is deliberate and stable: discovery first, then the library, then
/// playlists, then devices. `tools/list` results are cached and fed to a model,
/// so a stable order is both a caching property and a prompt-cache one.
static TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "get_server_stats",
        title: "Server statistics",
        description: "Get server name, base URL, and totals for the indexed library: \
                      file counts by type, total size, playlist count and database size. \
                      A good first call to learn what this server holds.",
        effect: Effect::Read,
        input: no_input,
        output: || {
            status_schema(serde_json::json!({
                "server_name": { "type": "string" },
                "server_url": { "type": "string" },
                "total_files": { "type": "integer" },
                "total_size_bytes": { "type": "integer" },
                "total_size_human": { "type": "string" },
                "video_files": { "type": "integer" },
                "audio_files": { "type": "integer" },
                "image_files": { "type": "integer" },
                "playlists": { "type": "integer" },
                "database_size_bytes": { "type": "integer" }
            }))
        },
    },
    ToolSpec {
        name: "list_library_roots",
        title: "List library roots",
        description: "List the configured media directories this server indexes. \
                      Call this before browse_folder: the roots are the only paths \
                      guaranteed to be browsable, and they cannot be guessed.",
        effect: Effect::Read,
        input: no_input,
        output: || {
            status_schema(serde_json::json!({
                "roots": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" },
                            "name": { "type": "string" },
                            "file_count": { "type": "integer" },
                            "available": { "type": "boolean" }
                        }
                    }
                }
            }))
        },
    },
    ToolSpec {
        name: "search_media",
        title: "Search the library",
        description: "Full-text search across filenames, titles, artists, albums, genres, \
                      composers and — where available — fetched synopses. Results are ranked \
                      by relevance. Multi-word queries match all words; the last word also \
                      matches as a prefix, so \"beethov sym\" finds \"Beethoven Symphony\". \
                      This is the right tool when you know roughly what you are looking for.",
        effect: Effect::Read,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Words to search for. Punctuation is ignored."
                    },
                    "category": category_enum("Restrict to one media type. Defaults to 'all'."),
                    "cursor": {
                        "type": "string",
                        "description": "Opaque cursor returned by the previous page"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Page size (defaults to 50, maximum 500)"
                    }
                },
                "required": ["query"]
            })
        },
        output: || file_page_schema("total_matches"),
    },
    ToolSpec {
        name: "list_media",
        title: "List library files",
        description: "Page through every indexed file, optionally filtered by category. \
                      Use search_media when you know what you want; this is for surveying \
                      the whole library.",
        effect: Effect::Read,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "category": category_enum("Restrict to one media type. Defaults to 'all'."),
                    "limit": {
                        "type": "integer",
                        "description": "Page size (defaults to 100, maximum 500)"
                    },
                    "cursor": {
                        "type": "string",
                        "description": "Opaque cursor returned by the previous page"
                    }
                }
            })
        },
        output: || file_page_schema("total_files"),
    },
    ToolSpec {
        name: "browse_folder",
        title: "Browse a folder",
        description: "List one directory's immediate subfolders and files. Call \
                      list_library_roots first to get a valid starting path. Results are \
                      paged: a folder with thousands of files returns them a page at a time.",
        effect: Effect::Read,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute folder path, from list_library_roots or from \
                                        the `path` of a folder in a previous result"
                    },
                    "category": category_enum("Restrict files to one media type. Defaults to 'all'."),
                    "offset": {
                        "type": "integer",
                        "description": "Files to skip (defaults to 0)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Page size (defaults to 200, maximum 500)"
                    }
                },
                "required": ["path"]
            })
        },
        output: || {
            status_schema(serde_json::json!({
                "path": { "type": "string" },
                "parent": { "type": ["string", "null"] },
                "directories": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "path": { "type": "string" },
                            "file_count": { "type": "integer" }
                        }
                    }
                },
                "files": { "type": "array", "items": media_file_schema() },
                "total": { "type": "integer" },
                "offset": { "type": "integer" }
            }))
        },
    },
    ToolSpec {
        name: "get_media_info",
        title: "Media file details",
        description: "Full metadata for one file: tags, audio stream properties, playable \
                      URLs, and any synopsis, rating or artwork fetched from an online \
                      metadata provider.",
        effect: Effect::Read,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "file_id": {
                        "type": "integer",
                        "description": "Numeric id from any listing or search result"
                    }
                },
                "required": ["file_id"]
            })
        },
        output: media_file_schema,
    },
    ToolSpec {
        name: "list_music_categories",
        title: "List music categories",
        description: "List the distinct artists, albums, album artists, genres or years in \
                      the music library, each with a track count. Answered from an index, so \
                      it is cheap on a large library — prefer it over listing every file and \
                      grouping them yourself.",
        effect: Effect::Read,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "kind": {
                        "type": "string",
                        "enum": ["artist", "album", "album_artist", "genre", "year"],
                        "description": "Which category to enumerate"
                    },
                    "artist": {
                        "type": "string",
                        "description": "With kind='album', list only this artist's albums"
                    },
                    "genre": {
                        "type": "string",
                        "description": "Restrict the results to one genre"
                    }
                },
                "required": ["kind"]
            })
        },
        output: || {
            status_schema(serde_json::json!({
                "kind": { "type": "string" },
                "categories": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "track_count": { "type": "integer" },
                            "child_count": { "type": "integer" }
                        }
                    }
                }
            }))
        },
    },
    ToolSpec {
        name: "find_music",
        title: "Find music by tag",
        description: "Find tracks by exact tag values, combining any of artist, album artist, \
                      album, genre and year. Use list_music_categories first to learn the \
                      exact spellings — these match exactly, unlike search_media.",
        effect: Effect::Read,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "artist": { "type": "string" },
                    "album_artist": { "type": "string" },
                    "album": { "type": "string" },
                    "genre": { "type": "string" },
                    "year": { "type": "integer" },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum tracks to return (defaults to 200, maximum 500)"
                    }
                }
            })
        },
        output: || file_page_schema("total_matches"),
    },
    ToolSpec {
        name: "list_playlists",
        title: "List playlists",
        description: "List every playlist on the server with its id, name and description.",
        effect: Effect::Read,
        input: no_input,
        output: || {
            status_schema(serde_json::json!({
                "playlists": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "integer" },
                            "name": { "type": "string" },
                            "description": { "type": ["string", "null"] },
                            "created_at": { "type": "integer" },
                            "updated_at": { "type": "integer" }
                        }
                    }
                }
            }))
        },
    },
    ToolSpec {
        name: "get_playlist_tracks",
        title: "Playlist tracks",
        description: "List a playlist's tracks in playback order.",
        effect: Effect::Read,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": { "playlist_id": { "type": "integer" } },
                "required": ["playlist_id"]
            })
        },
        output: || {
            status_schema(serde_json::json!({
                "playlist_id": { "type": "integer" },
                "tracks_count": { "type": "integer" },
                "tracks": { "type": "array", "items": media_file_schema() }
            }))
        },
    },
    ToolSpec {
        name: "create_playlist",
        title: "Create a playlist",
        description: "Create an empty playlist and return its id. Add tracks with \
                      add_to_playlist.",
        effect: Effect::Write,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "description": { "type": "string" }
                },
                "required": ["name"]
            })
        },
        output: || {
            status_schema(serde_json::json!({
                "playlist_id": { "type": "integer" },
                "status": { "type": "string" },
                "name": { "type": "string" }
            }))
        },
    },
    ToolSpec {
        name: "add_to_playlist",
        title: "Add tracks to a playlist",
        description: "Append media files to a playlist, in the order given. Pass every id in \
                      one call rather than calling repeatedly.",
        effect: Effect::Write,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "playlist_id": { "type": "integer" },
                    "media_file_ids": {
                        "type": "array",
                        "items": { "type": "integer" },
                        "description": "File ids, in the order they should play"
                    }
                },
                "required": ["playlist_id", "media_file_ids"]
            })
        },
        output: || {
            status_schema(serde_json::json!({
                "playlist_id": { "type": "integer" },
                "tracks_added": { "type": "integer" },
                "status": { "type": "string" }
            }))
        },
    },
    ToolSpec {
        name: "reorder_playlist",
        title: "Reorder a playlist",
        description: "Replace a playlist's running order. The ids given become positions \
                      0, 1, 2 and so on.",
        effect: Effect::Write,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "playlist_id": { "type": "integer" },
                    "media_file_ids": {
                        "type": "array",
                        "items": { "type": "integer" },
                        "description": "Every track id, in the new order"
                    }
                },
                "required": ["playlist_id", "media_file_ids"]
            })
        },
        output: || {
            status_schema(serde_json::json!({
                "playlist_id": { "type": "integer" },
                "tracks": { "type": "integer" },
                "status": { "type": "string" }
            }))
        },
    },
    ToolSpec {
        name: "remove_from_playlist",
        title: "Remove a track from a playlist",
        description: "Remove one media file from a playlist. The file itself is untouched.",
        effect: Effect::Destroy,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "playlist_id": { "type": "integer" },
                    "media_file_id": { "type": "integer" }
                },
                "required": ["playlist_id", "media_file_id"]
            })
        },
        output: || {
            status_schema(serde_json::json!({
                "playlist_id": { "type": "integer" },
                "media_file_id": { "type": "integer" },
                "status": { "type": "string" }
            }))
        },
    },
    ToolSpec {
        name: "delete_playlist",
        title: "Delete a playlist",
        description: "Delete a playlist and all of its entries. The media files themselves \
                      are untouched. This cannot be undone.",
        effect: Effect::Destroy,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": { "playlist_id": { "type": "integer" } },
                "required": ["playlist_id"]
            })
        },
        output: || {
            status_schema(serde_json::json!({
                "playlist_id": { "type": "integer" },
                "status": { "type": "string" }
            }))
        },
    },
    ToolSpec {
        name: "list_renderers",
        title: "List playback devices",
        description: "List the DLNA, Chromecast and AirPlay devices discovered on the local \
                      network. Use the `id` for every other renderer tool — friendly names \
                      are not unique and can change, ids do not.",
        effect: Effect::Device,
        input: no_input,
        output: || {
            status_schema(serde_json::json!({
                "renderers_found": { "type": "integer" },
                "renderers": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "friendly_name": { "type": "string" },
                            "model": { "type": ["string", "null"] },
                            "location": { "type": ["string", "null"] },
                            "protocol": { "type": "string" },
                            "capabilities": { "type": "object" }
                        }
                    }
                }
            }))
        },
    },
    ToolSpec {
        name: "get_playback_status",
        title: "What is playing",
        description: "Ask a renderer what it is currently playing, and report what this \
                      server last sent it. Call this after casting to confirm playback \
                      actually started.",
        effect: Effect::Device,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "renderer_id": {
                        "type": "string",
                        "description": "Stable id from list_renderers. Omit to report every \
                                        renderer this server has an active cast on."
                    }
                }
            })
        },
        output: || {
            status_schema(serde_json::json!({
                "renderers": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "renderer_id": { "type": "string" },
                            "friendly_name": { "type": "string" },
                            "state": {
                                "type": "string",
                                "enum": ["playing", "paused", "stopped", "finished", "error", "unknown"]
                            },
                            "current_url": { "type": ["string", "null"] },
                            "current_file": { "type": ["object", "null"] },
                            "queue_position": { "type": ["integer", "null"] },
                            "queue_length": { "type": ["integer", "null"] }
                        }
                    }
                }
            }))
        },
    },
    ToolSpec {
        name: "cast_media_to_renderer",
        title: "Cast a file to a device",
        description: "Start playing one media file on a renderer. Get `file_id` from a \
                      search or listing and `renderer_id` from list_renderers.",
        effect: Effect::Device,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "file_id": { "type": "integer" },
                    "renderer_id": {
                        "type": "string",
                        "description": "Stable renderer id from list_renderers"
                    }
                },
                "required": ["file_id", "renderer_id"]
            })
        },
        output: || {
            status_schema(serde_json::json!({
                "status": { "type": "string" },
                "renderer": { "type": "string" },
                "media_url": { "type": "string" }
            }))
        },
    },
    ToolSpec {
        name: "cast_playlist_to_renderer",
        title: "Cast a playlist to a device",
        description: "Play a playlist on a renderer, starting at `track_index` and advancing \
                      automatically as each track finishes.",
        effect: Effect::Device,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "playlist_id": { "type": "integer" },
                    "renderer_id": { "type": "string" },
                    "track_index": {
                        "type": "integer",
                        "description": "0-based track to start from (defaults to 0)"
                    }
                },
                "required": ["playlist_id", "renderer_id"]
            })
        },
        output: || {
            status_schema(serde_json::json!({
                "status": { "type": "string" },
                "renderer": { "type": "string" },
                "tracks_count": { "type": "integer" },
                "current_index": { "type": "integer" }
            }))
        },
    },
    ToolSpec {
        name: "cast_folder_to_renderer",
        title: "Cast a folder to a device",
        description: "Play everything in a folder on a renderer, in natural filename order, \
                      without creating a permanent playlist.",
        effect: Effect::Device,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Folder path, as returned by browse_folder"
                    },
                    "renderer_id": { "type": "string" },
                    "media": {
                        "type": "string",
                        "enum": ["audio", "video"],
                        "description": "Restrict to one kind. Omit to cast everything castable."
                    }
                },
                "required": ["path", "renderer_id"]
            })
        },
        output: || {
            status_schema(serde_json::json!({
                "status": { "type": "string" },
                "renderer": { "type": "string" },
                "tracks_count": { "type": "integer" }
            }))
        },
    },
    ToolSpec {
        name: "control_renderer",
        title: "Control playback",
        description: "Play, pause or stop what is already on a renderer. Stopping tears the \
                      session down, which is what a push protocol like AirPlay audio needs.",
        effect: Effect::Device,
        input: || {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "renderer_id": { "type": "string" },
                    "action": {
                        "type": "string",
                        "enum": ["play", "pause", "stop"]
                    }
                },
                "required": ["renderer_id", "action"]
            })
        },
        output: || {
            status_schema(serde_json::json!({
                "status": { "type": "string" },
                "action": { "type": "string" },
                "renderer": { "type": "string" }
            }))
        },
    },
];

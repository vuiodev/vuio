pub(super) fn get_tools_list() -> serde_json::Value {
    #[allow(unused_mut)]
    let mut tools = serde_json::json!({
        "tools": [
            {
                "name": "search_media",
                "description": "Search media files (video, audio, images) by keyword in filename or title. Returns matching files with their IDs, paths, types and metadata.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search keyword to match against filenames and titles"
                        },
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
                }
            },
            {
                "name": "browse_folder",
                "description": "Browse files and subdirectories in a specific folder path. Returns directories and files at that location.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "The folder path to browse (relative to media root, or absolute path)"
                        },
                        "category": {
                            "type": "string",
                            "enum": ["all", "audio", "video", "image"],
                            "description": "Optional media type filter. Defaults to 'all'."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "get_media_info",
                "description": "Get detailed metadata for a specific media file by its numeric ID. Returns title, artist, album, duration, size, mime type and more.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_id": {
                            "type": "integer",
                            "description": "The numeric ID of the media file"
                        }
                    },
                    "required": ["file_id"]
                }
            },
            {
                "name": "get_server_stats",
                "description": "Get server statistics including total media file counts by type (video, audio, image), total library size, and database size.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "list_renderers",
                "description": "List cached DLNA, Chromecast, and AirPlay renderers with their stable IDs and capabilities.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "cast_media_to_renderer",
                "description": "Cast a media file to a renderer by stable ID. First use list_renderers.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_id": {
                            "type": "integer",
                            "description": "The numeric ID of the media file to cast"
                        },
                        "renderer_id": {
                            "type": "string",
                            "description": "Stable renderer ID returned by list_renderers"
                        }
                    },
                    "required": ["file_id", "renderer_id"]
                }
            },
            {
                "name": "control_renderer",
                "description": "Send a playback control command to a smart TV or media renderer. Use after casting media to control playback.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "renderer_id": {
                            "type": "string",
                            "description": "Stable renderer ID returned by list_renderers"
                        },
                        "action": {
                            "type": "string",
                            "enum": ["play", "pause", "stop"],
                            "description": "The playback action to perform"
                        }
                    },
                    "required": ["renderer_id", "action"]
                }
            },
            {
                "name": "list_media",
                "description": "List all media files indexed on the server, optionally filtered by category (video, audio, image). Returns a flat list containing IDs, filenames, titles, paths, size and mime type. Useful for getting an overview of what files exist.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "category": {
                            "type": "string",
                            "enum": ["all", "audio", "video", "image"],
                            "description": "Optional category filter. Defaults to 'all'."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of files to return (defaults to 100)"
                        },
                        "cursor": {
                            "type": "string",
                            "description": "Opaque cursor returned by the previous page"
                        }
                    }
                }
            },
            {
                "name": "list_playlists",
                "description": "List all playlists currently stored on the server.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "create_playlist",
                "description": "Create a new media playlist.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The name of the new playlist"
                        },
                        "description": {
                            "type": "string",
                            "description": "Optional description for the playlist"
                        }
                    },
                    "required": ["name"]
                }
            },
            {
                "name": "delete_playlist",
                "description": "Delete a playlist by its ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "playlist_id": {
                            "type": "integer",
                            "description": "The numeric ID of the playlist to delete"
                        }
                    },
                    "required": ["playlist_id"]
                }
            },
            {
                "name": "add_to_playlist",
                "description": "Add one or more media files to a playlist in bulk.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "playlist_id": {
                            "type": "integer",
                            "description": "The numeric ID of the target playlist"
                        },
                        "media_file_ids": {
                            "type": "array",
                            "items": {
                                "type": "integer"
                            },
                            "description": "An array of media file IDs to add to the playlist"
                        }
                    },
                    "required": ["playlist_id", "media_file_ids"]
                }
            },
            {
                "name": "remove_from_playlist",
                "description": "Remove a specific media file from a playlist.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "playlist_id": {
                            "type": "integer",
                            "description": "The numeric ID of the playlist"
                        },
                        "media_file_id": {
                            "type": "integer",
                            "description": "The numeric ID of the media file to remove"
                        }
                    },
                    "required": ["playlist_id", "media_file_id"]
                }
            },
            {
                "name": "get_playlist_tracks",
                "description": "Get all media files/tracks in a specific playlist.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "playlist_id": {
                            "type": "integer",
                            "description": "The numeric ID of the playlist"
                        }
                    },
                    "required": ["playlist_id"]
                }
            },
            {
                "name": "cast_playlist_to_renderer",
                "description": "Cast a playlist to a renderer by stable ID.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "playlist_id": {
                            "type": "integer",
                            "description": "The numeric ID of the playlist to cast"
                        },
                        "renderer_id": {
                            "type": "string",
                            "description": "Stable renderer ID returned by list_renderers"
                        },
                        "track_index": {
                            "type": "integer",
                            "description": "Optional 0-based index of the track in the playlist to start playing from (defaults to 0)"
                        }
                    },
                    "required": ["playlist_id", "renderer_id"]
                }
            }
        ]
    });

    // Without a cast provider these four cannot work, so they are not
    // advertised: an MCP client should see the tools it can actually call.
    #[cfg(not(feature = "casting"))]
    {
        const CASTING_TOOLS: [&str; 4] = [
            "list_renderers",
            "cast_media_to_renderer",
            "control_renderer",
            "cast_playlist_to_renderer",
        ];
        if let Some(list) = tools.get_mut("tools").and_then(|value| value.as_array_mut()) {
            list.retain(|tool| {
                !tool
                    .get("name")
                    .and_then(|name| name.as_str())
                    .is_some_and(|name| CASTING_TOOLS.contains(&name))
            });
        }
    }

    tools
}
use std::sync::Arc;
use tempfile::tempdir;
use std::path::PathBuf;
use axum::extract::{State, Query};

use vuio::database::{DatabaseManager, MediaFile};
use vuio::database::redb::RedbDatabase;
use vuio::config::AppConfig;
use vuio::platform::filesystem::create_platform_filesystem_manager;
use vuio::platform::PlatformInfo;
use vuio::state::AppState;
use vuio::web::mcp::{message_handler, MessageQuery};
use vuio::web::handlers::WebHandlerMetrics;

#[tokio::test]
async fn test_mcp_initialize_and_tools_list() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test_mcp.redb");
    
    // 1. Initialize DB and insert some sample files
    let db = Arc::new(RedbDatabase::new(db_path).await.unwrap());
    db.initialize().await.unwrap();

    let audio_file = MediaFile {
        id: None,
        path: PathBuf::from("/media/music/song.mp3"),
        filename: "song.mp3".to_string(),
        size: 5000,
        modified: std::time::SystemTime::now(),
        mime_type: "audio/mpeg".to_string(),
        duration: None,
        title: Some("Stairway to Heaven".to_string()),
        artist: Some("Led Zeppelin".to_string()),
        album: Some("Led Zeppelin IV".to_string()),
        genre: Some("Rock".to_string()),
        track_number: Some(4),
        year: Some(1971),
        album_artist: None,
        created_at: std::time::SystemTime::now(),
        updated_at: std::time::SystemTime::now(),
    };
    db.store_media_file(&audio_file).await.unwrap();

    // 2. Setup mock AppState
    let config = Arc::new(AppConfig::default());
    let platform_info = Arc::new(PlatformInfo::detect().await.unwrap());
    let filesystem_manager = Arc::from(create_platform_filesystem_manager());
    let content_update_id = Arc::new(std::sync::atomic::AtomicU32::new(1));
    let web_metrics = Arc::new(WebHandlerMetrics::new());

    let app_state = AppState {
        config,
        database: db,
        platform_info,
        filesystem_manager,
        content_update_id,
        web_metrics,
        bookmarks: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        log_file_path: temp_dir.path().join("vuio.log"),
        browse_cache: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        mcp_clients: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_monitors: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
    };

    // Add a fake client channel so message handler can send back to the SSE receiver
    let client_id = "test-client-123".to_string();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(10);
    app_state.mcp_clients.lock().await.insert(client_id.clone(), tx);

    // 3. Test `initialize` method
    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "id": 1
    });

    let _resp = message_handler(
        State(app_state.clone()),
        Query(MessageQuery { client_id: client_id.clone() }),
        init_req.to_string()
    ).await;

    // Check message sent through channel
    let response_str = rx.recv().await.unwrap();
    let init_res: serde_json::Value = serde_json::from_str(&response_str).unwrap();
    assert_eq!(init_res["jsonrpc"], "2.0");
    assert_eq!(init_res["id"], 1);
    assert_eq!(init_res["result"]["protocolVersion"], "2025-03-26");

    // 4. Test `tools/list` method
    let list_req = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "id": 2
    });

    let _resp = message_handler(
        State(app_state.clone()),
        Query(MessageQuery { client_id: client_id.clone() }),
        list_req.to_string()
    ).await;

    let response_str2 = rx.recv().await.unwrap();
    let list_res: serde_json::Value = serde_json::from_str(&response_str2).unwrap();
    assert_eq!(list_res["id"], 2);
    let tools = list_res["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|t| t["name"] == "search_media"));
    assert!(tools.iter().any(|t| t["name"] == "list_tvs"));

    // 5. Test `tools/call` for `search_media`
    let search_req = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "id": 3,
        "params": {
            "name": "search_media",
            "arguments": {
                "query": "stairway"
            }
        }
    });

    let _resp = message_handler(
        State(app_state.clone()),
        Query(MessageQuery { client_id: client_id.clone() }),
        search_req.to_string()
    ).await;

    let response_str3 = rx.recv().await.unwrap();
    let search_res: serde_json::Value = serde_json::from_str(&response_str3).unwrap();
    assert_eq!(search_res["id"], 3);
    
    // Parse the inner text block from tool response
    let text = search_res["result"]["content"][0]["text"].as_str().unwrap();
    let search_data: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(search_data["total_matches"], 1);
    assert_eq!(search_data["files"][0]["title"], "Stairway to Heaven");
    assert_eq!(search_data["files"][0]["artist"], "Led Zeppelin");

    // 6. Test `tools/call` for `list_media`
    let list_media_req = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "id": 4,
        "params": {
            "name": "list_media",
            "arguments": {
                "category": "audio",
                "limit": 10
            }
        }
    });

    let _resp = message_handler(
        State(app_state.clone()),
        Query(MessageQuery { client_id: client_id.clone() }),
        list_media_req.to_string()
    ).await;

    let response_str4 = rx.recv().await.unwrap();
    let list_media_res: serde_json::Value = serde_json::from_str(&response_str4).unwrap();
    assert_eq!(list_media_res["id"], 4);

    let text_list = list_media_res["result"]["content"][0]["text"].as_str().unwrap();
    let list_data: serde_json::Value = serde_json::from_str(text_list).unwrap();
    assert_eq!(list_data["total_files"], 1);
    assert_eq!(list_data["files"][0]["filename"], "song.mp3");

    // 7. Test playlist creation via MCP
    let create_pl_req = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "id": 5,
        "params": {
            "name": "create_playlist",
            "arguments": {
                "name": "My Favorites",
                "description": "Best tracks"
            }
        }
    });

    let _resp = message_handler(
        State(app_state.clone()),
        Query(MessageQuery { client_id: client_id.clone() }),
        create_pl_req.to_string()
    ).await;

    let response_str5 = rx.recv().await.unwrap();
    let create_pl_res: serde_json::Value = serde_json::from_str(&response_str5).unwrap();
    assert_eq!(create_pl_res["id"], 5);

    let create_text = create_pl_res["result"]["content"][0]["text"].as_str().unwrap();
    let create_data: serde_json::Value = serde_json::from_str(create_text).unwrap();
    let playlist_id = create_data["playlist_id"].as_i64().unwrap();
    assert!(playlist_id > 0);

    // 8. Test adding a media file to the playlist in bulk
    let add_track_req = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "id": 6,
        "params": {
            "name": "add_to_playlist",
            "arguments": {
                "playlist_id": playlist_id,
                "media_file_ids": [1] // The song we added has ID 1 (inserted into database)
            }
        }
    });

    let _resp = message_handler(
        State(app_state.clone()),
        Query(MessageQuery { client_id: client_id.clone() }),
        add_track_req.to_string()
    ).await;

    let response_str6 = rx.recv().await.unwrap();
    let add_track_res: serde_json::Value = serde_json::from_str(&response_str6).unwrap();
    assert_eq!(add_track_res["id"], 6);

    // 9. Test getting tracks in the playlist
    let get_tracks_req = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "id": 7,
        "params": {
            "name": "get_playlist_tracks",
            "arguments": {
                "playlist_id": playlist_id
            }
        }
    });

    let _resp = message_handler(
        State(app_state.clone()),
        Query(MessageQuery { client_id: client_id.clone() }),
        get_tracks_req.to_string()
    ).await;

    let response_str7 = rx.recv().await.unwrap();
    let get_tracks_res: serde_json::Value = serde_json::from_str(&response_str7).unwrap();
    assert_eq!(get_tracks_res["id"], 7);

    let tracks_text = get_tracks_res["result"]["content"][0]["text"].as_str().unwrap();
    let tracks_data: serde_json::Value = serde_json::from_str(tracks_text).unwrap();
    assert_eq!(tracks_data["tracks_count"], 1);
    assert_eq!(tracks_data["tracks"][0]["filename"], "song.mp3");
}

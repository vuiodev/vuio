pub mod handlers;
pub mod xml;
pub mod playlist_api;

use crate::state::AppState;
use axum::{routing::{get, post, delete}, Router};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(handlers::root_handler))
        .route("/description.xml", get(handlers::description_handler))
        .route(
            "/ContentDirectory.xml",
            get(handlers::content_directory_scpd),
        )
        .route(
            "/control/ContentDirectory",
            get(handlers::content_directory_control).post(handlers::content_directory_control),
        )
        .route(
            "/event/ContentDirectory",
            axum::routing::any(handlers::content_directory_subscribe),
        )
        // Corrected route syntax from "/media/:id" to "/media/{id}"
        .route("/media/{id}", get(handlers::serve_media))
        // Playlist management API routes
        .route("/api/playlists", get(playlist_api::list_playlists).post(playlist_api::create_playlist))
        .route("/api/playlists/import", post(playlist_api::import_playlist))
        .route("/api/playlists/scan", post(playlist_api::scan_and_import_playlists))
        .route("/api/playlists/{id}", delete(playlist_api::delete_playlist))
        .route("/api/playlists/{id}/export", get(playlist_api::export_playlist))
        .with_state(state)
}
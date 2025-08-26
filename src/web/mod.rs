pub mod handlers;
pub mod xml;

use crate::state::AppState;
use axum::{routing::get, Router};

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
        .route("/media/{id}", get(handlers::serve_media))
        .with_state(state)
}
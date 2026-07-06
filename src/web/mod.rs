pub mod handlers;
pub mod xml;
pub mod client;

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
        .route(
            "/ConnectionManager.xml",
            get(handlers::connection_manager_scpd),
        )
        .route(
            "/control/ConnectionManager",
            get(handlers::connection_manager_control).post(handlers::connection_manager_control),
        )
        .route(
            "/X_MS_MediaReceiverRegistrar.xml",
            get(handlers::media_receiver_registrar_scpd),
        )
        .route(
            "/control/X_MS_MediaReceiverRegistrar",
            get(handlers::media_receiver_registrar_control).post(handlers::media_receiver_registrar_control),
        )
        .route("/media/{id}", get(handlers::serve_media).head(handlers::serve_media))
        .route("/media/{id}/subtitle", get(handlers::serve_subtitle))
        .route("/metrics", get(handlers::get_prometheus_metrics))
        .route("/metrics/json", get(handlers::get_web_metrics))
        .route("/healthz", get(handlers::healthz_handler))
        .route("/readyz", get(handlers::readyz_handler))
        .route("/logs", get(handlers::get_logs_handler))
        .with_state(state)
}
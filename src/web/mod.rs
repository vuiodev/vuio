pub mod casting;
pub mod client;
pub mod diagnostics;
pub mod eventing;
pub mod mcp;
pub mod soap;
pub mod streaming;
pub mod ui;
pub mod xml;

use crate::state::AppState;
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};

const SOAP_BODY_LIMIT: usize = 1024 * 1024;
const JSON_BODY_LIMIT: usize = 256 * 1024;

pub fn create_router(state: AppState) -> Router {
    let soap_routes = Router::new()
        .route(
            "/control/ContentDirectory",
            get(soap::content_directory_control).post(soap::content_directory_control),
        )
        .route(
            "/control/ConnectionManager",
            get(soap::connection_manager_control).post(soap::connection_manager_control),
        )
        .route(
            "/control/X_MS_MediaReceiverRegistrar",
            get(soap::media_receiver_registrar_control)
                .post(soap::media_receiver_registrar_control),
        )
        .layer(DefaultBodyLimit::max(SOAP_BODY_LIMIT));

    let json_routes = Router::new()
        .route("/api/cast/playlist", post(casting::api_cast_playlist))
        .route("/mcp/message", post(mcp::message_handler))
        .layer(DefaultBodyLimit::max(JSON_BODY_LIMIT));

    Router::new()
        .route("/", get(ui::root_handler))
        .route("/description.xml", get(soap::description_handler))
        .route("/ContentDirectory.xml", get(soap::content_directory_scpd))
        .route(
            "/event/ContentDirectory",
            axum::routing::any(eventing::content_directory_subscribe),
        )
        .route("/ConnectionManager.xml", get(soap::connection_manager_scpd))
        .route(
            "/X_MS_MediaReceiverRegistrar.xml",
            get(soap::media_receiver_registrar_scpd),
        )
        .route(
            "/media/{id}",
            get(streaming::serve_media).head(streaming::serve_media),
        )
        .route("/media/{id}/cover", get(streaming::serve_cover))
        .route("/media/{id}/subtitle", get(streaming::serve_subtitle))
        .route("/metrics", get(diagnostics::get_prometheus_metrics))
        .route("/metrics/json", get(diagnostics::get_web_metrics))
        .route("/healthz", get(diagnostics::healthz_handler))
        .route("/readyz", get(diagnostics::readyz_handler))
        .route("/logs", get(diagnostics::get_logs_handler))
        .route("/api/tvs", get(casting::api_list_tvs))
        .route("/sse", get(mcp::sse_handler))
        .merge(soap_routes)
        .merge(json_routes)
        .with_state(state)
}

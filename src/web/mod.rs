pub mod casting;
pub mod client;
pub mod diagnostics;
pub mod eventing;
mod format;
pub mod mcp;
pub mod soap;
pub mod streaming;
pub mod ui;
pub mod xml;

use crate::{database::DatabaseManager, state::AppState};
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};

const SOAP_BODY_LIMIT: usize = 1024 * 1024;
const JSON_BODY_LIMIT: usize = 256 * 1024;

pub fn create_router<D: DatabaseManager + 'static>(state: AppState<D>) -> Router {
    let soap_routes = Router::new()
        .route(
            "/control/ContentDirectory",
            get(soap::content_directory_control::<D>).post(soap::content_directory_control::<D>),
        )
        .route(
            "/control/ConnectionManager",
            get(soap::connection_manager_control::<D>).post(soap::connection_manager_control::<D>),
        )
        .route(
            "/control/X_MS_MediaReceiverRegistrar",
            get(soap::media_receiver_registrar_control::<D>)
                .post(soap::media_receiver_registrar_control::<D>),
        )
        .layer(DefaultBodyLimit::max(SOAP_BODY_LIMIT));

    let json_routes = Router::new()
        .route("/api/cast/playlist", post(casting::api_cast_playlist::<D>))
        .route("/mcp/message", post(mcp::message_handler::<D>))
        .layer(DefaultBodyLimit::max(JSON_BODY_LIMIT));

    Router::new()
        .route("/", get(ui::root_handler))
        .route("/api/server-info", get(ui::server_info_handler::<D>))
        .route("/api/media", get(ui::media_page_handler::<D>))
        .route("/description.xml", get(soap::description_handler::<D>))
        .route("/ContentDirectory.xml", get(soap::content_directory_scpd))
        .route(
            "/event/ContentDirectory",
            axum::routing::any(eventing::content_directory_subscribe::<D>),
        )
        .route("/ConnectionManager.xml", get(soap::connection_manager_scpd))
        .route(
            "/X_MS_MediaReceiverRegistrar.xml",
            get(soap::media_receiver_registrar_scpd),
        )
        .route(
            "/media/{id}",
            get(streaming::serve_media::<D>).head(streaming::serve_media::<D>),
        )
        .route("/media/{id}/cover", get(streaming::serve_cover::<D>))
        .route("/media/{id}/subtitle", get(streaming::serve_subtitle::<D>))
        .route("/metrics", get(diagnostics::get_prometheus_metrics::<D>))
        .route("/metrics/json", get(diagnostics::get_web_metrics::<D>))
        .route("/healthz", get(diagnostics::healthz_handler))
        .route("/readyz", get(diagnostics::readyz_handler::<D>))
        .route("/logs", get(diagnostics::get_logs_handler::<D>))
        .route("/api/renderers", get(casting::api_list_renderers::<D>))
        .route("/sse", get(mcp::sse_handler::<D>))
        .merge(soap_routes)
        .merge(json_routes)
        .with_state(state)
}

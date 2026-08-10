pub mod auth;
#[cfg(feature = "casting")]
pub mod casting;
pub mod client;
pub mod diagnostics;
pub mod eventing;
mod format;
#[cfg(feature = "mcp")]
pub mod mcp;
pub mod soap;
pub mod streaming;
#[cfg(feature = "dashboard")]
pub mod ui;
pub mod xml;

use crate::{database::DatabaseManager, state::AppState};
use axum::{
    extract::DefaultBodyLimit,
    middleware,
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

    // Routes are added conditionally rather than declared in one chain: a
    // feature that is off must take its endpoints with it, so a caller gets a
    // 404 instead of a handler that cannot work.
    #[allow(unused_mut)]
    let mut json_routes = Router::new();
    #[cfg(feature = "casting")]
    {
        json_routes = json_routes
            .route("/api/cast", post(casting::api_cast::<D>))
            .route("/api/cast/control", post(casting::api_cast_control::<D>))
            .route("/api/cast/playlist", post(casting::api_cast_playlist::<D>))
            .route(
                "/api/renderers/pair/start",
                post(casting::api_pairing_start::<D>),
            )
            .route(
                "/api/renderers/pair/finish",
                post(casting::api_pairing_finish::<D>),
            )
            .route(
                "/api/renderers/pair/forget",
                post(casting::api_pairing_forget::<D>),
            );
    }
    #[cfg(feature = "mcp")]
    {
        json_routes = json_routes.route("/mcp/message", post(mcp::message_handler::<D>));
    }
    let json_routes = json_routes.layer(DefaultBodyLimit::max(JSON_BODY_LIMIT));

    #[allow(unused_mut)]
    let mut management_routes = Router::new()
        .route("/metrics", get(diagnostics::get_prometheus_metrics::<D>))
        .route("/metrics/json", get(diagnostics::get_web_metrics::<D>))
        .route("/logs", get(diagnostics::get_logs_handler::<D>))
        .route("/logout", post(auth::logout::<D>));
    #[cfg(feature = "dashboard")]
    {
        management_routes = management_routes
            .route("/", get(ui::root_handler))
            .route("/api/server-info", get(ui::server_info_handler::<D>))
            .route("/api/media", get(ui::media_page_handler::<D>));
    }
    #[cfg(feature = "casting")]
    {
        management_routes =
            management_routes.route("/api/renderers", get(casting::api_list_renderers::<D>));
    }
    #[cfg(feature = "mcp")]
    {
        management_routes = management_routes.route("/sse", get(mcp::sse_handler::<D>));
    }
    let management_routes = management_routes
        .merge(json_routes)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_management::<D>,
        ));

    Router::new()
        .route("/login", get(auth::login_page::<D>).post(auth::login::<D>))
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
        .route("/healthz", get(diagnostics::healthz_handler))
        .route("/readyz", get(diagnostics::readyz_handler::<D>))
        .merge(soap_routes)
        .merge(management_routes)
        .with_state(state)
}

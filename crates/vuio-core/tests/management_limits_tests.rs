//! Resource limits protect management routes even when no token is required.

mod common;

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware,
    routing::get,
    Router,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tower::ServiceExt;
use vuio_core::{config::ManagementConfig, database::sqlite::SqliteDatabase, web::auth};

fn request() -> Request<Body> {
    let mut request = Request::builder().uri("/").body(Body::empty()).unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("127.0.0.1:1234".parse::<SocketAddr>().unwrap()));
    request
}

#[tokio::test]
async fn management_requests_are_rate_limited_without_authentication() {
    for allowed_networks in [vec![], vec!["127.0.0.0/8".to_owned()]] {
        let temp = tempfile::tempdir().unwrap();
        let mut state = common::state_over(temp.path(), temp.path()).await;
        state.auth = Arc::new(
            auth::AuthState::load(
                &ManagementConfig {
                    enabled: false,
                    allowed_networks,
                    ..Default::default()
                },
                &temp.path().join("config.toml"),
                false,
            )
            .unwrap(),
        );
        assert!(!state.auth.enabled());
        let app = Router::new()
            .route("/", get(|| async { StatusCode::OK }))
            .layer(middleware::from_fn_with_state(
                state,
                auth::require_management::<SqliteDatabase>,
            ));
        for _ in 0..120 {
            assert_eq!(
                app.clone().oneshot(request()).await.unwrap().status(),
                StatusCode::OK
            );
        }
        assert_eq!(
            app.oneshot(request()).await.unwrap().status(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }
}

#[tokio::test]
async fn management_concurrency_is_limited_without_authentication() {
    let temp = tempfile::tempdir().unwrap();
    let mut state = common::state_over(temp.path(), temp.path()).await;
    state.auth = Arc::new(
        auth::AuthState::load(
            &ManagementConfig {
                enabled: false,
                ..Default::default()
            },
            &temp.path().join("config.toml"),
            false,
        )
        .unwrap(),
    );
    assert!(!state.auth.enabled());
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let (entered, mut arrivals) = tokio::sync::mpsc::unbounded_channel();
    let app = Router::new()
        .route(
            "/",
            get({
                let gate = gate.clone();
                move || {
                    let gate = gate.clone();
                    let entered = entered.clone();
                    async move {
                        entered.send(()).unwrap();
                        gate.acquire().await.unwrap().forget();
                        StatusCode::OK
                    }
                }
            }),
        )
        .layer(middleware::from_fn_with_state(
            state,
            auth::require_management::<SqliteDatabase>,
        ));
    let mut requests = Vec::new();
    for _ in 0..32 {
        requests.push(tokio::spawn(app.clone().oneshot(request())));
        tokio::time::timeout(Duration::from_secs(5), arrivals.recv())
            .await
            .unwrap()
            .unwrap();
    }
    let response = tokio::time::timeout(Duration::from_secs(5), app.oneshot(request()))
        .await
        .expect("excess requests must be rejected immediately")
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    gate.add_permits(32);
    for request in requests {
        assert_eq!(request.await.unwrap().unwrap().status(), StatusCode::OK);
    }
}

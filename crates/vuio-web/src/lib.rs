//! The VuIO browser interface, compiled into the server binary.
//!
//! The app itself is a Svelte single-page application developed in its own
//! repository, `github.com/vuiodev/vuio-web`, with its own toolchain and its
//! own dev server. What lives here is its build output, `dist/`, committed so
//! that compiling VuIO — including the Docker builder stage, which has no Node
//! — never needs npm. `scripts/build-web.sh` refreshes it from a checkout of
//! that repository, and `BUILD_INFO.toml` records the commit it came from.
//!
//! This crate is the whole of the Rust side: a table of bytes fixed at compile
//! time by `build.rs`, and a [`Router`] that hands them out. It holds no state,
//! opens no socket, and knows nothing about the server it is merged into. The
//! app talks to that server the way any browser does, over the HTTP API on the
//! same origin.
//!
//! ```no_run
//! # use axum::Router;
//! let app: Router<()> = Router::new().merge(vuio_web::routes());
//! ```

#![deny(missing_docs)]

use axum::{
    body::Body,
    http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// The shell every client-side route is served from. `adapter-static` is
/// configured with `fallback: 'index.html'`, so this is the app's only entry
/// point regardless of the URL the browser asked for.
const INDEX: &str = "/index.html";

/// Cache forever. SvelteKit puts a content hash in every name under this
/// prefix, so a changed body always arrives at a new URL and a stale copy can
/// never be the wrong one.
const IMMUTABLE_PREFIX: &str = "/_app/immutable/";
const IMMUTABLE_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
/// Everything else — the shell, `version.json`, the vendored player assets —
/// keeps its name across builds, so it has to be revalidated. The ETag makes
/// that cost one 304 rather than a re-download.
const REVALIDATE_CACHE_CONTROL: &str = "no-cache";

/// Paths that belong to the server rather than to the app.
///
/// The fallback answers an unknown path with the HTML shell, which is what
/// makes client-side routing work. Doing that for an API path would turn a
/// typo'd endpoint into a 200 full of HTML that `res.json()` fails on somewhere
/// far away, so those get an honest 404 instead.
const SERVER_PREFIXES: &[&str] = &[
    "/api/",
    "/media/",
    "/control/",
    "/event/",
    "/metrics",
    "/logs",
    "/mcp",
    "/login",
    "/logout",
    "/healthz",
    "/readyz",
];

/// Routes serving the single-page app: its shell, its bundled assets, and the
/// fallback that makes client-side routing work.
///
/// Generic over the state of the router this is merged into, because it needs
/// none of its own. The app's routes arrive as a fallback rather than as a
/// `/{*path}` route so that every route of the router it joins keeps priority:
/// merging this into a server cannot shadow an endpoint, whatever the app is
/// later taught to ask for.
pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new().route("/", get(shell)).fallback(fallback)
}

async fn shell(headers: HeaderMap) -> Response {
    serve(INDEX, &headers).unwrap_or_else(|| {
        // Unreachable: build.rs refuses to compile a dist with no index.html.
        (StatusCode::INTERNAL_SERVER_ERROR, "web UI is not built").into_response()
    })
}

async fn fallback(method: Method, uri: Uri, headers: HeaderMap) -> Response {
    // Only GET and HEAD can be answered from a table of static bytes. Anything
    // else that reaches here was aimed at a server route that does not exist.
    if !matches!(method, Method::GET | Method::HEAD) {
        return StatusCode::NOT_FOUND.into_response();
    }

    let path = uri.path();
    if let Some(response) = serve(path, &headers) {
        return response;
    }
    // A directory URL: `adapter-static` writes a nested page as `index.html`
    // inside its own folder.
    if path.ends_with('/') {
        if let Some(response) = serve(&format!("{path}index.html"), &headers) {
            return response;
        }
    }

    if SERVER_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
        || path.ends_with(".xml")
    {
        return StatusCode::NOT_FOUND.into_response();
    }

    shell(headers).await
}

fn serve(route: &str, request_headers: &HeaderMap) -> Option<Response> {
    let (_, content_type, body) = ASSETS
        .iter()
        .find(|(candidate, _, _)| *candidate == route)?;

    let cache_control = if route.starts_with(IMMUTABLE_PREFIX) {
        IMMUTABLE_CACHE_CONTROL
    } else {
        REVALIDATE_CACHE_CONTROL
    };
    let etag = format!("\"{:016x}\"", fnv1a(body));

    if request_headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| value.split(',').any(|candidate| candidate.trim() == etag))
    {
        return Some(
            Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .header(header::CACHE_CONTROL, cache_control)
                .header(header::ETAG, etag.as_str())
                .body(Body::empty())
                .expect("a 304 carries only valid headers"),
        );
    }

    Some(
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
            .header(header::CACHE_CONTROL, cache_control)
            .header(header::ETAG, etag.as_str())
            .body(Body::from(*body))
            .expect("a compiled-in asset carries only valid headers"),
    )
}

/// FNV-1a over the compiled-in body. Every asset is a compile-time constant, so
/// this is an exact content fingerprint: it changes when and only when the file
/// does.
const fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shell_and_its_bundles_are_embedded() {
        assert!(ASSETS.iter().any(|(route, _, _)| *route == INDEX));
        assert!(
            ASSETS
                .iter()
                .any(|(route, _, _)| route.starts_with(IMMUTABLE_PREFIX) && route.ends_with(".js")),
            "no hashed JavaScript bundle was embedded"
        );
    }

    /// A wrong content type here is a page that silently fails to load, so the
    /// mapping is worth asserting rather than eyeballing.
    #[test]
    fn bundles_are_served_as_script_and_style() {
        for (route, content_type, _) in ASSETS {
            if route.ends_with(".js") {
                assert_eq!(*content_type, "text/javascript; charset=utf-8", "{route}");
            }
            if route.ends_with(".css") {
                assert_eq!(*content_type, "text/css; charset=utf-8", "{route}");
            }
        }
    }

    #[test]
    fn hashed_bundles_are_immutable_and_the_shell_is_not() {
        let headers = HeaderMap::new();
        let shell = serve(INDEX, &headers).expect("the shell is embedded");
        assert_eq!(
            shell.headers().get(header::CACHE_CONTROL).unwrap(),
            REVALIDATE_CACHE_CONTROL
        );

        let (bundle, _, _) = ASSETS
            .iter()
            .find(|(route, _, _)| route.starts_with(IMMUTABLE_PREFIX))
            .expect("a hashed bundle is embedded");
        let response = serve(bundle, &headers).expect("the bundle is embedded");
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            IMMUTABLE_CACHE_CONTROL
        );
    }

    #[test]
    fn a_matching_etag_is_answered_with_not_modified() {
        let first = serve(INDEX, &HeaderMap::new()).expect("the shell is embedded");
        let etag = first.headers().get(header::ETAG).unwrap().clone();

        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, etag);
        let second = serve(INDEX, &headers).expect("the shell is embedded");
        assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
    }

    #[test]
    fn an_unknown_path_is_not_mistaken_for_an_asset() {
        assert!(serve("/nothing/here.js", &HeaderMap::new()).is_none());
    }
}

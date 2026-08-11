//! A minimal HTTP/1.1 client for the server's outbound control traffic.
//!
//! VuIO talks to devices on the local network — UPnP GENA callbacks, DLNA SOAP
//! control, AirPlay's `POST /play` — and every one of those is a plain,
//! unencrypted request to a numeric address on the same LAN. No TLS, no
//! redirects, no cookies, no proxy, no name resolution. `hyper` is already in
//! the dependency graph underneath axum, so serving those requests through it
//! directly costs nothing and lets `reqwest` (and the ~46 crates it pulls in
//! for features none of this uses) leave the tree.
//!
//! Bodies are always read with an explicit cap. The peer is an appliance on the
//! network rather than a trusted service, and a control response that never
//! ends must not be able to exhaust memory.

use anyhow::{Context, Result};
use bytes::{Buf, Bytes};
use http::{Request, StatusCode, Uri};
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::time::Duration;

/// The request body type: these requests are small and fully in memory.
pub(crate) type Body = Full<Bytes>;

/// A request body holding `data`.
pub(crate) fn body(data: impl Into<Bytes>) -> Body {
    Full::new(data.into())
}

/// A request with no body, for `GET` and the AirPlay commands that take none.
pub(crate) fn empty_body() -> Body {
    Full::new(Bytes::new())
}

/// A response whose body has already been read, up to the cap that was asked
/// for.
pub(crate) struct HttpResponse {
    pub(crate) status: StatusCode,
    pub(crate) body: Vec<u8>,
    /// The body hit the cap and the rest was discarded.
    pub(crate) truncated: bool,
}

impl HttpResponse {
    /// The body as text, with invalid UTF-8 replaced rather than rejected:
    /// these are diagnostics and SOAP payloads from third-party devices.
    pub(crate) fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// An HTTP/1.1 client for cleartext requests to local devices.
#[derive(Clone)]
pub(crate) struct HttpClient {
    inner: Client<HttpConnector, Body>,
    timeout: Duration,
}

impl HttpClient {
    /// A client whose `timeout` bounds connecting, sending, and reading the
    /// body — the same span `reqwest`'s per-request timeout covered.
    pub(crate) fn new(timeout: Duration) -> Self {
        let mut connector = HttpConnector::new();
        connector.set_connect_timeout(Some(timeout));
        // These peers are TVs and speakers: a pooled connection is usually
        // dead by the time it is reused, and a redirect is not something any
        // of these protocols define.
        let inner = Client::builder(TokioExecutor::new())
            .pool_max_idle_per_host(0)
            .build(connector);
        Self { inner, timeout }
    }

    /// Send a request and read at most `limit` bytes of the response body.
    pub(crate) async fn send(&self, request: Request<Body>, limit: usize) -> Result<HttpResponse> {
        let uri = request.uri().clone();
        let response = tokio::time::timeout(self.timeout, self.inner.request(request))
            .await
            .with_context(|| format!("request to {uri} timed out"))?
            .with_context(|| format!("request to {uri} failed"))?;

        let (parts, incoming) = response.into_parts();
        let (body, truncated) = tokio::time::timeout(self.timeout, read_capped(incoming, limit))
            .await
            .with_context(|| format!("reading the response from {uri} timed out"))??;

        Ok(HttpResponse {
            status: parts.status,
            body,
            truncated,
        })
    }

    /// `GET` a URI, reading at most `limit` bytes.
    pub(crate) async fn get(&self, uri: &Uri, limit: usize) -> Result<HttpResponse> {
        let request = Request::get(uri.clone())
            .body(empty_body())
            .context("could not build the request")?;
        self.send(request, limit).await
    }
}

/// Read a body frame by frame, stopping once `limit` bytes have been kept.
///
/// Returns the bytes and whether anything was dropped. Reading frames rather
/// than calling `collect()` is what makes the cap meaningful: `collect()` would
/// buffer the whole body first and only then let us look at its size.
async fn read_capped<B>(mut body: B, limit: usize) -> Result<(Vec<u8>, bool)>
where
    B: hyper::body::Body + Unpin,
    B::Error: std::error::Error + Send + Sync + 'static,
{
    let mut bytes = Vec::new();
    let mut truncated = false;

    while let Some(frame) = body.frame().await {
        let frame = frame.context("failed while reading the response body")?;
        let Ok(mut chunk) = frame.into_data() else {
            // A trailers frame carries no payload.
            continue;
        };
        let available = chunk.remaining();
        let remaining = limit.saturating_sub(bytes.len());
        if available > remaining {
            bytes.extend_from_slice(&chunk.copy_to_bytes(remaining));
            truncated = true;
            break;
        }
        bytes.extend_from_slice(&chunk.copy_to_bytes(available));
    }

    Ok((bytes, truncated))
}

/// Reject a URL string that carries a fragment.
///
/// [`Uri`] parses `http://host/path#frag` by silently discarding the fragment,
/// where the `url` crate reported it and callers rejected it. The check has to
/// happen on the raw text to keep that refusal.
pub(crate) fn has_fragment(raw: &str) -> bool {
    raw.contains('#')
}

/// Whether a URI carries userinfo (`http://user:password@host/`).
///
/// [`Uri::host`] strips it, so the authority has to be inspected directly.
pub(crate) fn has_credentials(uri: &Uri) -> bool {
    uri.authority()
        .is_some_and(|authority| authority.as_str().contains('@'))
}

/// Resolve a device's control URL, which UPnP allows to be either absolute or
/// relative to the description URL.
///
/// This is RFC 3986 reference resolution, the same rule `Url::join` applied
/// before: a relative reference resolves against the *directory* of the base
/// path, so `ctrl` under `/dev/desc.xml` is `/dev/ctrl` and not `/ctrl`.
pub(crate) fn join_path(base: &Uri, reference: &str) -> Result<Uri> {
    if has_scheme(reference) {
        return reference.parse().context("invalid URL on the device");
    }

    // The reference is deliberately not parsed as a `Uri` first. `Uri` accepts
    // only whole URIs and origin-form paths, so `AVTransport/ctrl` fails to
    // parse outright and a bare `ctrl` is read as an *authority* — a host named
    // "ctrl" — which silently produced the wrong address.
    let authority = base.authority().context("base URL has no host")?.clone();
    let (target_path, query) = match reference.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (reference, None),
    };

    let merged = if target_path.starts_with('/') {
        target_path.to_owned()
    } else if target_path.is_empty() {
        base.path().to_owned()
    } else {
        let base_path = base.path();
        let directory = match base_path.rfind('/') {
            Some(index) => &base_path[..=index],
            None => "/",
        };
        format!("{directory}{target_path}")
    };

    let mut resolved = remove_dot_segments(&merged);
    if let Some(query) = query {
        resolved.push('?');
        resolved.push_str(query);
    }

    Uri::builder()
        .scheme(base.scheme_str().unwrap_or("http"))
        .authority(authority)
        .path_and_query(resolved)
        .build()
        .context("could not resolve the URL against the device address")
}

/// Whether a URL reference is absolute, i.e. carries its own scheme.
///
/// A scheme is everything before the first `:`, and it only counts when that
/// colon comes before any `/` — otherwise the colon belongs to a port or a
/// path segment.
fn has_scheme(reference: &str) -> bool {
    match (reference.find(':'), reference.find('/')) {
        (Some(colon), Some(slash)) => colon < slash,
        (Some(_), None) => true,
        _ => false,
    }
}

/// RFC 3986 §5.2.4: collapse `.` and `..` segments in a path.
fn remove_dot_segments(path: &str) -> String {
    let mut output: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "." => {}
            ".." => {
                output.pop();
            }
            segment => output.push(segment),
        }
    }
    // `split` on a leading slash yields an empty first segment, which rebuilds
    // the leading slash on join. A path that popped past its root loses it.
    let joined = output.join("/");
    if joined.starts_with('/') {
        joined
    } else {
        format!("/{joined}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_body_is_capped_and_reports_that_it_was_cut() {
        let (bytes, truncated) = read_capped(body("0123456789"), 8).await.unwrap();
        assert_eq!(bytes, b"01234567");
        assert!(truncated);
    }

    #[tokio::test]
    async fn a_body_under_the_cap_is_returned_whole() {
        let (bytes, truncated) = read_capped(body("0123"), 8).await.unwrap();
        assert_eq!(bytes, b"0123");
        assert!(!truncated);
    }

    #[test]
    fn fragments_are_still_detectable_after_the_switch_to_uri() {
        // The whole reason this helper exists: Uri drops the fragment.
        let raw = "http://192.168.1.10/x#frag";
        assert!(has_fragment(raw));
        assert_eq!(
            raw.parse::<Uri>().unwrap().to_string(),
            "http://192.168.1.10/x"
        );
    }

    #[test]
    fn credentials_are_detected_in_the_authority() {
        let with = "http://user:pw@192.168.1.10/x".parse::<Uri>().unwrap();
        let without = "http://192.168.1.10/x".parse::<Uri>().unwrap();
        assert!(has_credentials(&with));
        assert!(!has_credentials(&without));
        // Uri::host hides userinfo, which is why the check is not on the host.
        assert_eq!(with.host(), Some("192.168.1.10"));
    }

    #[test]
    fn host_matches_what_the_url_crate_reported() {
        // Both kept the brackets on an IPv6 literal, so address parsing
        // downstream behaves exactly as it did before.
        let uri = "http://[::1]:8080/desc.xml".parse::<Uri>().unwrap();
        assert_eq!(uri.host(), Some("[::1]"));
        assert_eq!(uri.port_u16(), Some(8080));
    }

    #[test]
    fn control_urls_resolve_against_the_description_url() {
        let base = "http://192.168.1.10:2870/desc.xml".parse::<Uri>().unwrap();

        // The two forms UPnP device descriptions actually use.
        assert_eq!(
            join_path(&base, "/AVTransport/ctrl").unwrap().to_string(),
            "http://192.168.1.10:2870/AVTransport/ctrl"
        );
        assert_eq!(
            join_path(&base, "AVTransport/ctrl").unwrap().to_string(),
            "http://192.168.1.10:2870/AVTransport/ctrl"
        );

        // An absolute control URL is taken as given.
        assert_eq!(
            join_path(&base, "http://192.168.1.11/ctrl")
                .unwrap()
                .to_string(),
            "http://192.168.1.11/ctrl"
        );
    }

    #[test]
    fn a_relative_control_url_resolves_against_the_base_directory() {
        // The case that separates real reference resolution from just
        // prefixing a slash: several renderers publish their description
        // under a subdirectory and their control URL relative to it.
        let base = "http://192.168.1.10:2870/dev/desc.xml"
            .parse::<Uri>()
            .unwrap();

        assert_eq!(
            join_path(&base, "ctrl").unwrap().to_string(),
            "http://192.168.1.10:2870/dev/ctrl"
        );
        assert_eq!(
            join_path(&base, "../ctrl").unwrap().to_string(),
            "http://192.168.1.10:2870/ctrl"
        );
        assert_eq!(
            join_path(&base, "/ctrl").unwrap().to_string(),
            "http://192.168.1.10:2870/ctrl"
        );
    }

    #[test]
    fn a_query_string_survives_resolution() {
        let base = "http://192.168.1.10/desc.xml".parse::<Uri>().unwrap();
        assert_eq!(
            join_path(&base, "ctrl?svc=AVTransport").unwrap().to_string(),
            "http://192.168.1.10/ctrl?svc=AVTransport"
        );
    }
}

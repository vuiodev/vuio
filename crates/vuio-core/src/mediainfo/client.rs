//! The outbound HTTP surface, and the types every provider speaks in.
//!
//! `http_client.rs` cannot serve this: it has no TLS, no redirects and no name
//! resolution, all three of which are load-bearing here (every endpoint is HTTPS
//! on a hostname, and Cover Art Archive answers with a redirect to archive.org).
//! So this is the one place `reqwest` is used, kept behind one small wrapper so
//! providers cannot each invent their own timeout, cap or header policy.

use super::provider::ProviderInfo;
use super::rate_limit::RateLimiters;
use anyhow::{bail, Context, Result};
use std::time::Duration;

/// Identifies VuIO to the services it queries.
///
/// MusicBrainz rejects requests that do not identify their client, and blocks
/// clients that share a generic one, so this is a requirement rather than a
/// courtesy. It is sent to every provider for consistency.
pub const USER_AGENT: &str = "MediaServer (http://github/media)";

/// A JSON response is read with an explicit cap. These are third-party services
/// rather than trusted peers, and a body that never ends must not be able to
/// exhaust memory — the same rule `http_client.rs` applies to devices on the LAN.
const MAX_JSON_BYTES: usize = 4 * 1024 * 1024;

/// What kind of thing a file looks like, which decides the providers it is put to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaQueryKind {
    Movie,
    Episode,
    Music,
    Anime,
}

/// What we know about a file before asking anyone.
#[derive(Clone, Debug, Default)]
pub struct MediaQuery {
    pub title: String,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// An exact identifier read out of the file's own tags. When present there is
    /// nothing to guess and no search to run.
    pub musicbrainz_release_id: Option<String>,
    pub musicbrainz_track_id: Option<String>,
}

impl MediaQuery {
    /// What to put in a plain text search box.
    pub fn search_terms(&self) -> String {
        match (&self.artist, &self.album) {
            (Some(artist), Some(album)) => format!("{artist} {album}"),
            (Some(artist), None) => format!("{artist} {}", self.title),
            _ => self.title.clone(),
        }
    }
}

/// One possible answer from one provider, before scoring.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub provider: &'static str,
    pub remote_id: String,
    /// `movie` | `series` | `episode` | `album` | `track` | `anime`
    pub kind: &'static str,
    pub title: String,
    pub original_title: Option<String>,
    pub overview: Option<String>,
    pub release_date: Option<String>,
    pub year: Option<u32>,
    pub rating: Option<f64>,
    pub genres: Vec<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub artwork_url: Option<String>,
    /// The provider's own record, kept whole so a field we did not give a column
    /// to is a query away rather than another migration.
    pub payload: serde_json::Value,
}

impl Candidate {
    pub fn new(provider: &'static str, kind: &'static str, remote_id: String, title: String) -> Self {
        Self {
            provider,
            remote_id,
            kind,
            title,
            original_title: None,
            overview: None,
            release_date: None,
            year: None,
            rating: None,
            genres: Vec::new(),
            season: None,
            episode: None,
            artwork_url: None,
            payload: serde_json::Value::Null,
        }
    }
}

#[async_trait::async_trait]
pub trait MetadataProvider: Send + Sync {
    fn info(&self) -> &'static ProviderInfo;

    /// Ask this provider about `query`. Returning an empty vec means "nothing
    /// matched", which is not an error; `Err` means the provider could not be
    /// reached or refused us.
    async fn search(
        &self,
        http: &Fetcher,
        query: &MediaQuery,
        credential: Option<&str>,
    ) -> Result<Vec<Candidate>>;
}

/// A rate-limited, capped HTTP client shared by every provider.
pub struct Fetcher {
    client: reqwest::Client,
    limiters: RateLimiters,
}

impl Fetcher {
    pub fn new(timeout: Duration) -> Result<Self> {
        // `rustls-no-provider` means the client panics at build time unless a
        // provider is already installed. `Runtime::start` installs one, but this
        // type is also constructed by tests and by hosts embedding the crate
        // without going through `Runtime`. Installation is process-global and
        // returns an error when someone got there first, which is fine.
        let _ = rustls::crypto::ring::default_provider().install_default();

        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(timeout)
            // Cover Art Archive answers a release lookup with a redirect to
            // archive.org, so following them is required, not optional.
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .context("Failed to build the metadata HTTP client")?;
        Ok(Self {
            client,
            limiters: RateLimiters::new(),
        })
    }

    /// GET a JSON document, waiting for the provider's rate limit first.
    pub async fn get_json(
        &self,
        provider: &'static str,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        self.limiters.acquire(provider).await;
        let mut request = self.client.get(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request
            .send()
            .await
            .with_context(|| format!("{provider}: request to {url} failed"))?;
        self.read_json(provider, response).await
    }

    /// POST a JSON body and read a JSON response. Only AniList needs this — it is
    /// a GraphQL endpoint rather than a REST one.
    pub async fn post_json(
        &self,
        provider: &'static str,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.limiters.acquire(provider).await;
        let response = self
            .client
            .post(url)
            .json(body)
            .send()
            .await
            .with_context(|| format!("{provider}: request to {url} failed"))?;
        self.read_json(provider, response).await
    }

    async fn read_json(
        &self,
        provider: &'static str,
        response: reqwest::Response,
    ) -> Result<serde_json::Value> {
        let status = response.status();
        // A 404 is a legitimate "no such record" for several of these APIs, so it
        // is reported as an empty document rather than an error the job would
        // count as a failure.
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(serde_json::Value::Null);
        }
        if !status.is_success() {
            bail!("{provider}: responded {status}");
        }
        let body = read_capped(response, MAX_JSON_BYTES)
            .await
            .with_context(|| format!("{provider}: reading the response body failed"))?;
        if body.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_slice(&body).with_context(|| format!("{provider}: response was not JSON"))
    }

    /// Download an image, returning its content type and bytes.
    pub async fn get_image(
        &self,
        provider: &'static str,
        url: &str,
        limit: usize,
    ) -> Result<(String, Vec<u8>)> {
        self.limiters.acquire(provider).await;
        let response = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("{provider}: image request to {url} failed"))?;
        let status = response.status();
        if !status.is_success() {
            bail!("{provider}: image request responded {status}");
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let bytes = read_capped(response, limit)
            .await
            .with_context(|| format!("{provider}: reading the image body failed"))?;
        Ok((content_type, bytes))
    }
}

/// Read a body, stopping at `limit`.
///
/// `Response::bytes()` would buffer whatever the peer sends, which for an
/// untrusted host is an unbounded allocation, so the body is drained a chunk at a
/// time and abandoned once it exceeds what the caller asked for.
async fn read_capped(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > limit {
            bail!("response exceeded {limit} bytes");
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Percent-encode a value for use in a query string.
///
/// `percent_encoding` is already a dependency for the DLNA paths; QUERY_ENCODE
/// leaves the characters a query component allows and escapes the rest, including
/// the `&` and `=` that would otherwise let a filename forge extra parameters.
pub fn query_escape(value: &str) -> String {
    const QUERY_ENCODE: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~');
    percent_encoding::utf8_percent_encode(value, QUERY_ENCODE).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_escape_neutralises_parameter_injection() {
        // A filename is attacker-controlled as far as this code is concerned.
        let escaped = query_escape("a&api_key=leak b");
        assert!(!escaped.contains('&'));
        assert!(!escaped.contains('='));
        assert!(!escaped.contains(' '));
    }

    #[test]
    fn search_terms_prefer_artist_and_album() {
        let query = MediaQuery {
            title: "Black Dog".to_string(),
            artist: Some("Led Zeppelin".to_string()),
            album: Some("Led Zeppelin IV".to_string()),
            ..MediaQuery::default()
        };
        assert_eq!(query.search_terms(), "Led Zeppelin Led Zeppelin IV");
    }

    #[test]
    fn search_terms_fall_back_to_the_title() {
        let query = MediaQuery {
            title: "Arrival".to_string(),
            ..MediaQuery::default()
        };
        assert_eq!(query.search_terms(), "Arrival");
    }
}

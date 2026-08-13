//! MusicBrainz, with artwork from the Cover Art Archive. No account required.
//!
//! Two rules here are not negotiable. The service demands a User-Agent that
//! identifies the client and blocks those that do not supply one — [`USER_AGENT`]
//! is set on every request in `client.rs`. And it enforces one request per second
//! per IP at the server, which `rate_limit.rs` honours by serialising this
//! provider; this is the slowest provider VuIO has, by design rather than by
//! accident.
//!
//! Cover Art Archive has no search of its own: artwork is addressed by release id,
//! so it is reachable only once MusicBrainz has answered. That is why it is folded
//! in here instead of being a provider the operator can switch on alone.

use super::super::client::{query_escape, Candidate, Fetcher, MediaQuery, MetadataProvider};
use super::super::provider::{provider_info, ProviderInfo};
use super::{text, year_of};
use anyhow::Result;

const ID: &str = "musicbrainz";
const BASE: &str = "https://musicbrainz.org/ws/2";

pub struct MusicBrainz;

/// The front cover for a release. This URL redirects to archive.org, which is why
/// the shared client follows redirects.
fn cover_art_url(release_id: &str) -> String {
    format!("https://coverartarchive.org/release/{release_id}/front")
}

#[async_trait::async_trait]
impl MetadataProvider for MusicBrainz {
    fn info(&self) -> &'static ProviderInfo {
        provider_info(ID).expect("musicbrainz is in the registry")
    }

    async fn search(
        &self,
        http: &Fetcher,
        query: &MediaQuery,
        _credential: Option<&str>,
    ) -> Result<Vec<Candidate>> {
        // A release id read out of the file's own tags is an exact answer. Looking
        // it up directly skips the search, the guessing and the scoring.
        if let Some(release_id) = query.musicbrainz_release_id.as_deref() {
            let url = format!(
                "{BASE}/release/{}?fmt=json&inc=artist-credits+release-groups+genres",
                query_escape(release_id)
            );
            let body = http.get_json(ID, &url, &[]).await?;
            if let Some(candidate) = parse_release(&body) {
                return Ok(vec![candidate]);
            }
            return Ok(Vec::new());
        }

        let lucene = match (&query.artist, &query.album) {
            (Some(artist), Some(album)) => format!("release:\"{album}\" AND artist:\"{artist}\""),
            (Some(artist), None) => format!("artist:\"{artist}\" AND release:\"{}\"", query.title),
            (None, Some(album)) => format!("release:\"{album}\""),
            (None, None) => format!("release:\"{}\"", query.title),
        };
        let url = format!(
            "{BASE}/release?query={}&fmt=json&limit=5",
            query_escape(&lucene)
        );
        let body = http.get_json(ID, &url, &[]).await?;
        Ok(parse_search(&body))
    }
}

/// The artist name from a `artist-credit` array.
fn artist_credit(value: &serde_json::Value) -> Option<String> {
    let credits = value.get("artist-credit")?.as_array()?;
    let joined: String = credits
        .iter()
        .filter_map(|credit| {
            text(credit, "name").or_else(|| credit.get("artist").and_then(|a| text(a, "name")))
        })
        .collect::<Vec<_>>()
        .join(" & ");
    (!joined.is_empty()).then_some(joined)
}

fn release_to_candidate(entry: &serde_json::Value) -> Option<Candidate> {
    let id = text(entry, "id")?;
    let title = text(entry, "title")?;
    let mut candidate = Candidate::new(ID, "album", id.clone(), title);
    candidate.original_title = artist_credit(entry);
    candidate.release_date = text(entry, "date");
    candidate.year = candidate.release_date.as_deref().and_then(year_of);
    candidate.genres = super::named_list(entry, "genres");
    candidate.artwork_url = Some(cover_art_url(&id));
    candidate.payload = entry.clone();
    Some(candidate)
}

fn parse_search(body: &serde_json::Value) -> Vec<Candidate> {
    let Some(releases) = body.get("releases").and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    releases.iter().filter_map(release_to_candidate).collect()
}

fn parse_release(body: &serde_json::Value) -> Option<Candidate> {
    release_to_candidate(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_release_search_response() {
        let body = serde_json::json!({
            "releases": [{
                "id": "f1e2d3c4-0000-0000-0000-000000000001",
                "title": "Led Zeppelin IV",
                "date": "1971-11-08",
                "artist-credit": [{ "name": "Led Zeppelin" }],
                "genres": [{ "name": "hard rock" }]
            }]
        });

        let candidates = parse_search(&body);
        assert_eq!(candidates.len(), 1);
        let release = &candidates[0];
        assert_eq!(release.title, "Led Zeppelin IV");
        assert_eq!(release.original_title.as_deref(), Some("Led Zeppelin"));
        assert_eq!(release.year, Some(1971));
        assert_eq!(release.genres, vec!["hard rock"]);
        assert_eq!(release.kind, "album");
    }

    #[test]
    fn artwork_points_at_the_cover_art_archive_front_cover() {
        let body = serde_json::json!({
            "releases": [{ "id": "abc", "title": "An Album" }]
        });
        assert_eq!(
            parse_search(&body)[0].artwork_url.as_deref(),
            Some("https://coverartarchive.org/release/abc/front")
        );
    }

    #[test]
    fn a_joined_artist_credit_is_flattened() {
        let body = serde_json::json!({
            "id": "abc", "title": "A Split",
            "artist-credit": [{ "name": "One" }, { "artist": { "name": "Two" } }]
        });
        assert_eq!(
            parse_release(&body).unwrap().original_title.as_deref(),
            Some("One & Two")
        );
    }

    #[test]
    fn an_empty_search_yields_nothing() {
        assert!(parse_search(&serde_json::json!({ "releases": [] })).is_empty());
        assert!(parse_search(&serde_json::Value::Null).is_empty());
    }
}

//! Discogs — releases, masters and discographies. Needs a free personal token.

use super::super::client::{query_escape, Candidate, Fetcher, MediaQuery, MetadataProvider};
use super::super::provider::{provider_info, ProviderInfo};
use super::{text, year_of};
use anyhow::{bail, Result};

const ID: &str = "discogs";
const SEARCH: &str = "https://api.discogs.com/database/search";

pub struct Discogs;

#[async_trait::async_trait]
impl MetadataProvider for Discogs {
    fn info(&self) -> &'static ProviderInfo {
        provider_info(ID).expect("discogs is in the registry")
    }

    async fn search(
        &self,
        http: &Fetcher,
        query: &MediaQuery,
        credential: Option<&str>,
    ) -> Result<Vec<Candidate>> {
        let Some(token) = credential else {
            bail!("Discogs needs a personal access token");
        };
        let url = format!(
            "{SEARCH}?q={}&type=release&per_page=5",
            query_escape(&query.search_terms())
        );
        // The token goes in the header rather than the query string so it stays out
        // of any log that records the URL.
        let authorization = format!("Discogs token={token}");
        let body = http
            .get_json(ID, &url, &[("Authorization", authorization.as_str())])
            .await?;
        Ok(parse(&body))
    }
}

fn parse(body: &serde_json::Value) -> Vec<Candidate> {
    let Some(results) = body.get("results").and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|entry| {
            let id = entry.get("id")?.as_i64()?;
            // Discogs titles are "Artist - Album"; the album half is what a tag
            // would have called it, so scoring wants that rather than the pair.
            let full = text(entry, "title")?;
            let (artist, album) = match full.split_once(" - ") {
                Some((artist, album)) => (Some(artist.trim().to_string()), album.trim().to_string()),
                None => (None, full.clone()),
            };
            let mut candidate = Candidate::new(ID, "album", id.to_string(), album);
            candidate.original_title = artist;
            candidate.release_date = text(entry, "released").or_else(|| text(entry, "year"));
            candidate.year = text(entry, "year")
                .as_deref()
                .and_then(year_of)
                .or_else(|| candidate.release_date.as_deref().and_then(year_of));
            candidate.genres = {
                let mut genres = super::named_list(entry, "genre");
                genres.extend(super::named_list(entry, "style"));
                genres
            };
            candidate.artwork_url =
                text(entry, "cover_image").or_else(|| text(entry, "thumb"));
            candidate.payload = entry.clone();
            Some(candidate)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_the_artist_off_the_title() {
        let body = serde_json::json!({
            "results": [{
                "id": 1234, "title": "Led Zeppelin - Led Zeppelin IV", "year": "1971",
                "genre": ["Rock"], "style": ["Hard Rock", "Blues Rock"],
                "cover_image": "https://x.test/c.jpg", "thumb": "https://x.test/t.jpg"
            }]
        });

        let candidates = parse(&body);
        assert_eq!(candidates.len(), 1);
        let release = &candidates[0];
        assert_eq!(release.title, "Led Zeppelin IV");
        assert_eq!(release.original_title.as_deref(), Some("Led Zeppelin"));
        assert_eq!(release.year, Some(1971));
        assert_eq!(release.genres, vec!["Rock", "Hard Rock", "Blues Rock"]);
        assert_eq!(release.artwork_url.as_deref(), Some("https://x.test/c.jpg"));
    }

    #[test]
    fn a_title_without_a_dash_is_kept_whole() {
        let body = serde_json::json!({ "results": [{ "id": 1, "title": "Untitled" }] });
        let candidates = parse(&body);
        assert_eq!(candidates[0].title, "Untitled");
        assert_eq!(candidates[0].original_title, None);
    }

    #[test]
    fn an_empty_result_set_yields_nothing() {
        assert!(parse(&serde_json::json!({ "results": [] })).is_empty());
    }
}

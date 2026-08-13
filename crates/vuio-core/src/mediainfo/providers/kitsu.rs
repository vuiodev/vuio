//! Kitsu — anime, manga and drama. JSON:API, no account required.

use super::super::client::{query_escape, Candidate, Fetcher, MediaQuery, MetadataProvider};
use super::super::provider::{provider_info, ProviderInfo};
use super::{number, text, year_of};
use anyhow::Result;

const ID: &str = "kitsu";
const SEARCH: &str = "https://kitsu.io/api/edge/anime";

pub struct Kitsu;

#[async_trait::async_trait]
impl MetadataProvider for Kitsu {
    fn info(&self) -> &'static ProviderInfo {
        provider_info(ID).expect("kitsu is in the registry")
    }

    async fn search(
        &self,
        http: &Fetcher,
        query: &MediaQuery,
        _credential: Option<&str>,
    ) -> Result<Vec<Candidate>> {
        let url = format!(
            "{SEARCH}?filter%5Btext%5D={}&page%5Blimit%5D=5",
            query_escape(&query.title)
        );
        // JSON:API servers are entitled to refuse a request that does not ask for
        // their media type.
        let body = http
            .get_json(ID, &url, &[("Accept", "application/vnd.api+json")])
            .await?;
        Ok(parse(&body, query.episode))
    }
}

fn parse(body: &serde_json::Value, wanted_episode: Option<u32>) -> Vec<Candidate> {
    let Some(results) = body.get("data").and_then(|data| data.as_array()) else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|entry| {
            let id = text(entry, "id")?;
            let attributes = entry.get("attributes")?;
            let title = text(attributes, "canonicalTitle").or_else(|| {
                attributes
                    .get("titles")
                    .and_then(|titles| text(titles, "en").or_else(|| text(titles, "en_jp")))
            })?;
            let mut candidate = Candidate::new(ID, "anime", id, title);
            candidate.original_title = attributes
                .get("titles")
                .and_then(|titles| text(titles, "ja_jp"));
            candidate.overview = text(attributes, "synopsis");
            candidate.release_date = text(attributes, "startDate");
            candidate.year = candidate.release_date.as_deref().and_then(year_of);
            // Kitsu rates out of 100 like AniList, not out of 10.
            candidate.rating = number(attributes, "averageRating").map(|rating| rating / 10.0);
            candidate.episode = wanted_episode;
            candidate.artwork_url = attributes.get("posterImage").and_then(|poster| {
                text(poster, "original")
                    .or_else(|| text(poster, "large"))
                    .or_else(|| text(poster, "medium"))
            });
            candidate.payload = entry.clone();
            Some(candidate)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_json_api_response() {
        let body = serde_json::json!({
            "data": [{
                "id": "7442",
                "type": "anime",
                "attributes": {
                    "canonicalTitle": "Some Anime",
                    "titles": { "en": "Some Anime", "ja_jp": "アニメ" },
                    "synopsis": "A synopsis.",
                    "startDate": "2013-04-07",
                    "averageRating": "82.53",
                    "posterImage": { "medium": "https://x.test/m.jpg", "original": "https://x.test/o.jpg" }
                }
            }]
        });

        let candidates = parse(&body, Some(3));
        assert_eq!(candidates.len(), 1);
        let anime = &candidates[0];
        // Kitsu ids are strings, not numbers, unlike every other provider here.
        assert_eq!(anime.remote_id, "7442");
        assert_eq!(anime.title, "Some Anime");
        assert_eq!(anime.year, Some(2013));
        assert_eq!(anime.rating, Some(8.253));
        assert_eq!(anime.episode, Some(3));
        assert_eq!(anime.artwork_url.as_deref(), Some("https://x.test/o.jpg"));
    }

    #[test]
    fn an_entry_without_a_usable_title_is_skipped() {
        let body = serde_json::json!({ "data": [{ "id": "1", "attributes": {} }] });
        assert!(parse(&body, None).is_empty());
    }
}

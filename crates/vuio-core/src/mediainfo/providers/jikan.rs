//! Jikan — the unofficial MyAnimeList API. No account required.

use super::super::client::{query_escape, Candidate, Fetcher, MediaQuery, MetadataProvider};
use super::super::provider::{provider_info, ProviderInfo};
use super::{named_list, number, text, year_of};
use anyhow::Result;

const ID: &str = "jikan";
const SEARCH: &str = "https://api.jikan.moe/v4/anime";

pub struct Jikan;

#[async_trait::async_trait]
impl MetadataProvider for Jikan {
    fn info(&self) -> &'static ProviderInfo {
        provider_info(ID).expect("jikan is in the registry")
    }

    async fn search(
        &self,
        http: &Fetcher,
        query: &MediaQuery,
        _credential: Option<&str>,
    ) -> Result<Vec<Candidate>> {
        let url = format!("{SEARCH}?q={}&limit=5", query_escape(&query.title));
        let body = http.get_json(ID, &url, &[]).await?;
        Ok(parse(&body, query.episode))
    }
}

/// `wanted_episode` is carried onto the candidate rather than looked up: Jikan
/// indexes anime at series level, and an episode number from the filename is the
/// only episode information available. Recording it keeps the scorer from docking
/// the match for a missing episode it was never going to find.
fn parse(body: &serde_json::Value, wanted_episode: Option<u32>) -> Vec<Candidate> {
    let Some(results) = body.get("data").and_then(|data| data.as_array()) else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|entry| {
            let id = entry.get("mal_id")?.as_i64()?;
            let title = text(entry, "title")
                .or_else(|| text(entry, "title_english"))
                .or_else(|| text(entry, "title_japanese"))?;
            let mut candidate = Candidate::new(ID, "anime", id.to_string(), title);
            candidate.original_title = text(entry, "title_english")
                .or_else(|| text(entry, "title_japanese"));
            candidate.overview = text(entry, "synopsis");
            candidate.rating = number(entry, "score");
            candidate.genres = named_list(entry, "genres");
            candidate.year = entry
                .get("year")
                .and_then(|year| year.as_u64())
                .map(|year| year as u32)
                .or_else(|| {
                    entry
                        .get("aired")
                        .and_then(|aired| text(aired, "from"))
                        .as_deref()
                        .and_then(year_of)
                });
            candidate.release_date = entry.get("aired").and_then(|aired| text(aired, "from"));
            candidate.episode = wanted_episode;
            candidate.artwork_url = entry
                .get("images")
                .and_then(|images| images.get("jpg"))
                .and_then(|jpg| {
                    text(jpg, "large_image_url").or_else(|| text(jpg, "image_url"))
                });
            candidate.payload = entry.clone();
            Some(candidate)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> serde_json::Value {
        serde_json::json!({
            "data": [{
                "mal_id": 5114,
                "title": "Some Anime",
                "title_english": "Some Anime: English",
                "synopsis": "A synopsis.",
                "score": 9.1,
                "year": 2009,
                "aired": { "from": "2009-04-05T00:00:00+00:00" },
                "genres": [{ "name": "Action" }, { "name": "Drama" }],
                "images": { "jpg": { "image_url": "https://x.test/s.jpg", "large_image_url": "https://x.test/l.jpg" } }
            }]
        })
    }

    #[test]
    fn reads_an_anime_search_response() {
        let candidates = parse(&fixture(), None);
        assert_eq!(candidates.len(), 1);
        let anime = &candidates[0];
        assert_eq!(anime.remote_id, "5114");
        assert_eq!(anime.title, "Some Anime");
        assert_eq!(anime.original_title.as_deref(), Some("Some Anime: English"));
        assert_eq!(anime.year, Some(2009));
        assert_eq!(anime.rating, Some(9.1));
        assert_eq!(anime.genres, vec!["Action", "Drama"]);
        assert_eq!(anime.artwork_url.as_deref(), Some("https://x.test/l.jpg"));
        assert_eq!(anime.kind, "anime");
    }

    #[test]
    fn the_episode_from_the_filename_is_carried_through() {
        // Jikan has no episode-level record, so without this the scorer would
        // penalise every anime episode for a season/episode it cannot supply.
        let candidates = parse(&fixture(), Some(12));
        assert_eq!(candidates[0].episode, Some(12));
    }

    #[test]
    fn a_missing_data_array_yields_nothing() {
        assert!(parse(&serde_json::json!({}), None).is_empty());
        assert!(parse(&serde_json::Value::Null, None).is_empty());
    }
}

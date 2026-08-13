//! AniList — a GraphQL API, free and without an account for public queries.

use super::super::client::{Candidate, Fetcher, MediaQuery, MetadataProvider};
use super::super::provider::{provider_info, ProviderInfo};
use super::{strip_html, text};
use anyhow::Result;

const ID: &str = "anilist";
const ENDPOINT: &str = "https://graphql.anilist.co";

/// Asking for exactly the fields that map onto a candidate. AniList charges rate
/// against complexity, so a narrower query is also a cheaper one.
const QUERY: &str = r#"
query ($search: String) {
  Page(perPage: 5) {
    media(search: $search, type: ANIME) {
      id
      title { romaji english native }
      description
      startDate { year }
      averageScore
      genres
      coverImage { extraLarge large }
    }
  }
}"#;

pub struct AniList;

#[async_trait::async_trait]
impl MetadataProvider for AniList {
    fn info(&self) -> &'static ProviderInfo {
        provider_info(ID).expect("anilist is in the registry")
    }

    async fn search(
        &self,
        http: &Fetcher,
        query: &MediaQuery,
        _credential: Option<&str>,
    ) -> Result<Vec<Candidate>> {
        let body = serde_json::json!({
            "query": QUERY,
            "variables": { "search": query.title },
        });
        let response = http.post_json(ID, ENDPOINT, &body).await?;
        Ok(parse(&response, query.episode))
    }
}

fn parse(body: &serde_json::Value, wanted_episode: Option<u32>) -> Vec<Candidate> {
    let Some(results) = body
        .get("data")
        .and_then(|data| data.get("Page"))
        .and_then(|page| page.get("media"))
        .and_then(|media| media.as_array())
    else {
        return Vec::new();
    };

    results
        .iter()
        .filter_map(|entry| {
            let id = entry.get("id")?.as_i64()?;
            let titles = entry.get("title");
            let title = titles
                .and_then(|title| text(title, "romaji"))
                .or_else(|| titles.and_then(|title| text(title, "english")))
                .or_else(|| titles.and_then(|title| text(title, "native")))?;
            let mut candidate = Candidate::new(ID, "anime", id.to_string(), title);
            candidate.original_title = titles.and_then(|title| text(title, "english"));
            // Descriptions come back as HTML with <br> runs in them.
            candidate.overview = text(entry, "description").map(|text| strip_html(&text));
            candidate.year = entry
                .get("startDate")
                .and_then(|date| date.get("year"))
                .and_then(|year| year.as_u64())
                .map(|year| year as u32);
            // AniList scores out of 100; every other provider is out of 10.
            candidate.rating = entry
                .get("averageScore")
                .and_then(|score| score.as_f64())
                .map(|score| score / 10.0);
            candidate.genres = super::named_list(entry, "genres");
            candidate.episode = wanted_episode;
            candidate.artwork_url = entry.get("coverImage").and_then(|cover| {
                text(cover, "extraLarge").or_else(|| text(cover, "large"))
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
            "data": { "Page": { "media": [{
                "id": 1535,
                "title": { "romaji": "Some Anime", "english": "Some Anime EN", "native": "アニメ" },
                "description": "A <br>synopsis.",
                "startDate": { "year": 2006 },
                "averageScore": 84,
                "genres": ["Psychological", "Thriller"],
                "coverImage": { "large": "https://x.test/l.jpg", "extraLarge": "https://x.test/xl.jpg" }
            }]}}
        })
    }

    #[test]
    fn reads_a_graphql_response() {
        let candidates = parse(&fixture(), None);
        assert_eq!(candidates.len(), 1);
        let anime = &candidates[0];
        assert_eq!(anime.remote_id, "1535");
        assert_eq!(anime.title, "Some Anime");
        assert_eq!(anime.original_title.as_deref(), Some("Some Anime EN"));
        assert_eq!(anime.year, Some(2006));
        assert_eq!(anime.overview.as_deref(), Some("A synopsis."));
        assert_eq!(anime.genres, vec!["Psychological", "Thriller"]);
        assert_eq!(anime.artwork_url.as_deref(), Some("https://x.test/xl.jpg"));
    }

    #[test]
    fn the_score_is_rescaled_to_ten() {
        // Storing 84 next to another provider's 8.4 would make the column
        // meaningless.
        assert_eq!(parse(&fixture(), None)[0].rating, Some(8.4));
    }

    #[test]
    fn a_graphql_error_response_yields_nothing() {
        let body = serde_json::json!({ "errors": [{ "message": "Too Many Requests" }] });
        assert!(parse(&body, None).is_empty());
    }
}

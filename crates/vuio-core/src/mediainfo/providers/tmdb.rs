//! TheMovieDB — movies and TV. Needs a free API key.

use super::super::client::{query_escape, Candidate, Fetcher, MediaQuery, MetadataProvider};
use super::super::provider::{provider_info, ProviderInfo};
use super::{number, strip_html, text, year_of};
use anyhow::{bail, Result};

const ID: &str = "tmdb";
const SEARCH: &str = "https://api.themoviedb.org/3/search/multi";
/// Poster paths come back relative; w500 is large enough for a TV's cover slot
/// without pulling the multi-megabyte original for every item in a library.
const IMAGE_BASE: &str = "https://image.tmdb.org/t/p/w500";

pub struct Tmdb;

#[async_trait::async_trait]
impl MetadataProvider for Tmdb {
    fn info(&self) -> &'static ProviderInfo {
        provider_info(ID).expect("tmdb is in the registry")
    }

    async fn search(
        &self,
        http: &Fetcher,
        query: &MediaQuery,
        credential: Option<&str>,
    ) -> Result<Vec<Candidate>> {
        let Some(key) = credential else {
            bail!("TheMovieDB needs an API key");
        };
        let mut url = format!(
            "{SEARCH}?api_key={}&query={}",
            query_escape(key),
            query_escape(&query.title)
        );
        if let Some(year) = query.year {
            url.push_str(&format!("&year={year}"));
        }
        let body = http.get_json(ID, &url, &[]).await?;
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
            let media_type = text(entry, "media_type").unwrap_or_else(|| "movie".to_string());
            // `search/multi` also returns people, who have no title and are not
            // something a media file can be.
            let kind = match media_type.as_str() {
                "tv" => "series",
                "movie" => "movie",
                _ => return None,
            };
            let id = entry.get("id")?.as_i64()?;
            // Movies carry `title`, TV carries `name`.
            let title = text(entry, "title").or_else(|| text(entry, "name"))?;
            let mut candidate = Candidate::new(ID, kind, id.to_string(), title);
            candidate.original_title =
                text(entry, "original_title").or_else(|| text(entry, "original_name"));
            candidate.overview = text(entry, "overview").map(|text| strip_html(&text));
            candidate.release_date =
                text(entry, "release_date").or_else(|| text(entry, "first_air_date"));
            candidate.year = candidate.release_date.as_deref().and_then(year_of);
            candidate.rating = number(entry, "vote_average");
            candidate.artwork_url = text(entry, "poster_path")
                .or_else(|| text(entry, "backdrop_path"))
                .map(|path| format!("{IMAGE_BASE}{path}"));
            candidate.payload = entry.clone();
            Some(candidate)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_multi_search_response() {
        let body = serde_json::json!({
            "results": [
                {
                    "id": 329865, "media_type": "movie", "title": "Arrival",
                    "original_title": "Arrival", "overview": "A linguist is recruited.",
                    "release_date": "2016-11-10", "vote_average": 7.6,
                    "poster_path": "/poster.jpg"
                },
                {
                    "id": 1399, "media_type": "tv", "name": "Some Show",
                    "first_air_date": "2011-04-17", "vote_average": 8.4,
                    "poster_path": "/show.jpg"
                }
            ]
        });

        let candidates = parse(&body);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].kind, "movie");
        assert_eq!(candidates[0].title, "Arrival");
        assert_eq!(candidates[0].year, Some(2016));
        assert_eq!(
            candidates[0].artwork_url.as_deref(),
            Some("https://image.tmdb.org/t/p/w500/poster.jpg")
        );
        // TV entries name their title differently; both have to be read.
        assert_eq!(candidates[1].kind, "series");
        assert_eq!(candidates[1].title, "Some Show");
        assert_eq!(candidates[1].year, Some(2011));
    }

    #[test]
    fn people_results_are_dropped() {
        let body = serde_json::json!({
            "results": [{ "id": 1, "media_type": "person", "name": "Someone" }]
        });
        assert!(parse(&body).is_empty());
    }

    #[tokio::test]
    async fn searching_without_a_key_is_an_error_rather_than_a_silent_miss() {
        let http = Fetcher::new(std::time::Duration::from_secs(1)).unwrap();
        let result = Tmdb
            .search(&http, &MediaQuery::default(), None)
            .await;
        assert!(result.is_err());
    }
}

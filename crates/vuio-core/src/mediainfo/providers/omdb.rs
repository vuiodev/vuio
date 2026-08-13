//! OMDb — IMDb-sourced ratings, posters and plots. Needs a free API key.

use super::super::client::{query_escape, Candidate, Fetcher, MediaQuery, MetadataProvider};
use super::super::provider::{provider_info, ProviderInfo};
use super::{number, text, year_of};
use anyhow::{bail, Result};

const ID: &str = "omdb";
const BASE: &str = "https://www.omdbapi.com/";

pub struct Omdb;

#[async_trait::async_trait]
impl MetadataProvider for Omdb {
    fn info(&self) -> &'static ProviderInfo {
        provider_info(ID).expect("omdb is in the registry")
    }

    async fn search(
        &self,
        http: &Fetcher,
        query: &MediaQuery,
        credential: Option<&str>,
    ) -> Result<Vec<Candidate>> {
        let Some(key) = credential else {
            bail!("OMDb needs an API key");
        };
        // `t=` returns one fully-populated record — plot, rating, genres — where
        // `s=` returns a list of stubs. One good answer beats five thin ones,
        // especially against a 1000/day budget.
        let mut url = format!(
            "{BASE}?apikey={}&t={}&plot=short",
            query_escape(key),
            query_escape(&query.title)
        );
        if let Some(year) = query.year {
            url.push_str(&format!("&y={year}"));
        }
        if query.season.is_some() {
            url.push_str("&type=series");
        }
        let body = http.get_json(ID, &url, &[]).await?;
        Ok(parse(&body).into_iter().collect())
    }
}

fn parse(body: &serde_json::Value) -> Option<Candidate> {
    // OMDb signals failure in the body with HTTP 200: `{"Response":"False", ...}`.
    if text(body, "Response").as_deref() == Some("False") {
        return None;
    }
    let id = text(body, "imdbID")?;
    let title = text(body, "Title")?;
    let kind = match text(body, "Type").as_deref() {
        Some("series") => "series",
        Some("episode") => "episode",
        _ => "movie",
    };
    let mut candidate = Candidate::new(ID, kind, id, title);
    candidate.overview = text(body, "Plot");
    candidate.release_date = text(body, "Released");
    candidate.year = text(body, "Year").as_deref().and_then(year_of);
    candidate.rating = number(body, "imdbRating");
    candidate.genres = text(body, "Genre")
        .map(|genres| {
            genres
                .split(',')
                .map(str::trim)
                .filter(|genre| !genre.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    candidate.season = text(body, "Season").and_then(|season| season.parse().ok());
    candidate.episode = text(body, "Episode").and_then(|episode| episode.parse().ok());
    candidate.artwork_url = text(body, "Poster");
    candidate.payload = body.clone();
    Some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_title_lookup() {
        let body = serde_json::json!({
            "Title": "Arrival", "Year": "2016", "Released": "11 Nov 2016",
            "Genre": "Drama, Mystery, Sci-Fi", "Plot": "A linguist is recruited.",
            "Poster": "https://x.test/p.jpg", "imdbRating": "7.9",
            "imdbID": "tt2543164", "Type": "movie", "Response": "True"
        });

        let candidate = parse(&body).unwrap();
        assert_eq!(candidate.remote_id, "tt2543164");
        assert_eq!(candidate.title, "Arrival");
        assert_eq!(candidate.year, Some(2016));
        assert_eq!(candidate.rating, Some(7.9));
        assert_eq!(candidate.genres, vec!["Drama", "Mystery", "Sci-Fi"]);
        assert_eq!(candidate.kind, "movie");
    }

    #[test]
    fn a_false_response_is_a_miss_not_a_record() {
        // OMDb reports "not found" with HTTP 200 and this body, so failing to read
        // it would store an empty candidate for every unmatched file.
        let body = serde_json::json!({ "Response": "False", "Error": "Movie not found!" });
        assert!(parse(&body).is_none());
    }

    #[test]
    fn n_a_fields_do_not_become_content() {
        let body = serde_json::json!({
            "Title": "Something", "imdbID": "tt1", "Type": "movie",
            "Plot": "N/A", "Poster": "N/A", "imdbRating": "N/A", "Response": "True"
        });
        let candidate = parse(&body).unwrap();
        assert_eq!(candidate.overview, None);
        assert_eq!(candidate.artwork_url, None);
        assert_eq!(candidate.rating, None);
    }
}

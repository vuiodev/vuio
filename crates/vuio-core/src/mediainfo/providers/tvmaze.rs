//! TVmaze — TV series and episodes, no account required.

use super::super::client::{Candidate, Fetcher, MediaQuery, MetadataProvider};
use super::super::provider::{provider_info, ProviderInfo};
use super::{named_list, number, strip_html, text, year_of};
use anyhow::Result;

const ID: &str = "tvmaze";
const SEARCH: &str = "https://api.tvmaze.com/search/shows";

pub struct TvMaze;

#[async_trait::async_trait]
impl MetadataProvider for TvMaze {
    fn info(&self) -> &'static ProviderInfo {
        provider_info(ID).expect("tvmaze is in the registry")
    }

    async fn search(
        &self,
        http: &Fetcher,
        query: &MediaQuery,
        _credential: Option<&str>,
    ) -> Result<Vec<Candidate>> {
        let url = format!(
            "{SEARCH}?q={}",
            super::super::client::query_escape(&query.title)
        );
        let body = http.get_json(ID, &url, &[]).await?;
        let mut candidates = parse_shows(&body);

        // An episode request wants the episode's own title and summary, which the
        // show search does not carry. Only the best show is followed up: every
        // extra lookup is another second against the rate limit.
        if let (Some(season), Some(episode), Some(best)) =
            (query.season, query.episode, candidates.first().cloned())
        {
            let url = format!(
                "https://api.tvmaze.com/shows/{}/episodebynumber?season={season}&number={episode}",
                super::super::client::query_escape(&best.remote_id),
            );
            match http.get_json(ID, &url, &[]).await {
                Ok(body) => {
                    if let Some(found) = parse_episode(&body, &best) {
                        candidates.insert(0, found);
                    }
                }
                // The show matched but the episode does not exist upstream. That is
                // a miss, not a failure — the show-level candidate still stands.
                Err(error) => tracing::debug!(%error, "TVmaze episode lookup failed"),
            }
        }

        Ok(candidates)
    }
}

fn parse_shows(body: &serde_json::Value) -> Vec<Candidate> {
    let Some(results) = body.as_array() else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|entry| {
            let show = entry.get("show")?;
            let id = show.get("id")?.as_i64()?;
            let name = text(show, "name")?;
            let mut candidate = Candidate::new(ID, "series", id.to_string(), name);
            candidate.overview = text(show, "summary").map(|summary| strip_html(&summary));
            candidate.release_date = text(show, "premiered");
            candidate.year = candidate.release_date.as_deref().and_then(year_of);
            candidate.rating = show.get("rating").and_then(|rating| number(rating, "average"));
            candidate.genres = named_list(show, "genres");
            candidate.artwork_url = show
                .get("image")
                .and_then(|image| text(image, "original").or_else(|| text(image, "medium")));
            candidate.payload = show.clone();
            Some(candidate)
        })
        .collect()
}

/// An episode record, taking the show's artwork when the episode has none of its
/// own — most episodes do not carry a still.
fn parse_episode(body: &serde_json::Value, show: &Candidate) -> Option<Candidate> {
    let id = body.get("id")?.as_i64()?;
    // "Breakage" on its own says nothing about which show it belongs to, and a
    // browse listing is where this title is read. The series leads.
    let episode_name = text(body, "name")?;
    let mut candidate = Candidate::new(
        ID,
        "episode",
        id.to_string(),
        format!("{} — {episode_name}", show.title),
    );
    candidate.overview = text(body, "summary").map(|summary| strip_html(&summary));
    candidate.release_date = text(body, "airdate");
    candidate.year = candidate.release_date.as_deref().and_then(year_of);
    candidate.season = body.get("season").and_then(|value| value.as_u64()).map(|v| v as u32);
    candidate.episode = body.get("number").and_then(|value| value.as_u64()).map(|v| v as u32);
    candidate.rating = body.get("rating").and_then(|rating| number(rating, "average"));
    candidate.genres = show.genres.clone();
    candidate.artwork_url = body
        .get("image")
        .and_then(|image| text(image, "original"))
        .or_else(|| show.artwork_url.clone());
    candidate.payload = body.clone();
    // The episode title is rarely what the filename said; the series name is. Keep
    // the show's name available so scoring can match on it.
    candidate.original_title = Some(show.title.clone());
    Some(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_show_search_response() {
        let body = serde_json::json!([{
            "score": 0.9,
            "show": {
                "id": 143,
                "name": "Some Show",
                "premiered": "2011-04-17",
                "genres": ["Drama", "Fantasy"],
                "rating": { "average": 8.9 },
                "summary": "<p>A <b>tale</b>.</p>",
                "image": { "medium": "https://x.test/m.jpg", "original": "https://x.test/o.jpg" }
            }
        }]);

        let candidates = parse_shows(&body);
        assert_eq!(candidates.len(), 1);
        let show = &candidates[0];
        assert_eq!(show.remote_id, "143");
        assert_eq!(show.title, "Some Show");
        assert_eq!(show.year, Some(2011));
        assert_eq!(show.rating, Some(8.9));
        assert_eq!(show.genres, vec!["Drama", "Fantasy"]);
        assert_eq!(show.overview.as_deref(), Some("A tale."));
        assert_eq!(show.artwork_url.as_deref(), Some("https://x.test/o.jpg"));
        assert_eq!(show.kind, "series");
    }

    #[test]
    fn an_empty_response_yields_nothing_rather_than_failing() {
        assert!(parse_shows(&serde_json::json!([])).is_empty());
        assert!(parse_shows(&serde_json::Value::Null).is_empty());
    }

    #[test]
    fn an_episode_inherits_the_shows_artwork_and_genres() {
        let show = {
            let mut candidate =
                Candidate::new(ID, "series", "143".to_string(), "Some Show".to_string());
            candidate.artwork_url = Some("https://x.test/o.jpg".to_string());
            candidate.genres = vec!["Drama".to_string()];
            candidate
        };
        let body = serde_json::json!({
            "id": 900, "name": "The Episode", "season": 2, "number": 5,
            "airdate": "2012-04-01", "summary": "<p>Things happen.</p>", "image": null
        });

        let episode = parse_episode(&body, &show).unwrap();
        assert_eq!(episode.kind, "episode");
        assert_eq!(episode.season, Some(2));
        assert_eq!(episode.episode, Some(5));
        // Series first: the episode name alone does not identify anything.
        assert_eq!(episode.title, "Some Show — The Episode");
        assert_eq!(episode.original_title.as_deref(), Some("Some Show"));
        assert_eq!(episode.artwork_url.as_deref(), Some("https://x.test/o.jpg"));
        assert_eq!(episode.genres, vec!["Drama"]);
    }
}

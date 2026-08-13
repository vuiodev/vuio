//! Genius — song, artist and album metadata. Needs a free access token.

use super::super::client::{query_escape, Candidate, Fetcher, MediaQuery, MetadataProvider};
use super::super::provider::{provider_info, ProviderInfo};
use super::{text, year_of};
use anyhow::{bail, Result};

const ID: &str = "genius";
const SEARCH: &str = "https://api.genius.com/search";

pub struct Genius;

#[async_trait::async_trait]
impl MetadataProvider for Genius {
    fn info(&self) -> &'static ProviderInfo {
        provider_info(ID).expect("genius is in the registry")
    }

    async fn search(
        &self,
        http: &Fetcher,
        query: &MediaQuery,
        credential: Option<&str>,
    ) -> Result<Vec<Candidate>> {
        let Some(token) = credential else {
            bail!("Genius needs an access token");
        };
        let terms = match &query.artist {
            Some(artist) => format!("{artist} {}", query.title),
            None => query.title.clone(),
        };
        let url = format!("{SEARCH}?q={}", query_escape(&terms));
        let authorization = format!("Bearer {token}");
        let body = http
            .get_json(ID, &url, &[("Authorization", authorization.as_str())])
            .await?;
        Ok(parse(&body))
    }
}

fn parse(body: &serde_json::Value) -> Vec<Candidate> {
    let Some(hits) = body
        .get("response")
        .and_then(|response| response.get("hits"))
        .and_then(|hits| hits.as_array())
    else {
        return Vec::new();
    };

    hits.iter()
        .filter_map(|hit| {
            // Genius returns other hit types alongside songs.
            if text(hit, "type").is_some_and(|kind| kind != "song") {
                return None;
            }
            let result = hit.get("result")?;
            let id = result.get("id")?.as_i64()?;
            let title = text(result, "title")?;
            let mut candidate = Candidate::new(ID, "track", id.to_string(), title);
            candidate.original_title = result
                .get("primary_artist")
                .and_then(|artist| text(artist, "name"));
            candidate.release_date = text(result, "release_date_for_display");
            candidate.year = result
                .get("release_date_components")
                .and_then(|components| components.get("year"))
                .and_then(|year| year.as_u64())
                .map(|year| year as u32)
                .or_else(|| candidate.release_date.as_deref().and_then(year_of));
            candidate.artwork_url = text(result, "song_art_image_url")
                .or_else(|| text(result, "header_image_url"));
            candidate.payload = result.clone();
            Some(candidate)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_song_search_response() {
        let body = serde_json::json!({
            "response": { "hits": [{
                "type": "song",
                "result": {
                    "id": 378195,
                    "title": "Black Dog",
                    "primary_artist": { "name": "Led Zeppelin" },
                    "release_date_for_display": "November 8, 1971",
                    "release_date_components": { "year": 1971, "month": 11, "day": 8 },
                    "song_art_image_url": "https://x.test/a.jpg"
                }
            }]}
        });

        let candidates = parse(&body);
        assert_eq!(candidates.len(), 1);
        let song = &candidates[0];
        assert_eq!(song.remote_id, "378195");
        assert_eq!(song.title, "Black Dog");
        assert_eq!(song.original_title.as_deref(), Some("Led Zeppelin"));
        assert_eq!(song.year, Some(1971));
        assert_eq!(song.kind, "track");
    }

    #[test]
    fn non_song_hits_are_dropped() {
        let body = serde_json::json!({
            "response": { "hits": [{ "type": "artist", "result": { "id": 1, "title": "X" } }] }
        });
        assert!(parse(&body).is_empty());
    }

    #[test]
    fn a_missing_response_yields_nothing() {
        assert!(parse(&serde_json::json!({})).is_empty());
    }
}

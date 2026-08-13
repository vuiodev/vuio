//! Last.fm — album art, artist biographies and tags. Needs a free API key.

use super::super::client::{query_escape, Candidate, Fetcher, MediaQuery, MetadataProvider};
use super::super::provider::{provider_info, ProviderInfo};
use super::text;
use anyhow::{bail, Result};

const ID: &str = "lastfm";
const BASE: &str = "https://ws.audioscrobbler.com/2.0/";

pub struct LastFm;

#[async_trait::async_trait]
impl MetadataProvider for LastFm {
    fn info(&self) -> &'static ProviderInfo {
        provider_info(ID).expect("lastfm is in the registry")
    }

    async fn search(
        &self,
        http: &Fetcher,
        query: &MediaQuery,
        credential: Option<&str>,
    ) -> Result<Vec<Candidate>> {
        let Some(key) = credential else {
            bail!("Last.fm needs an API key");
        };
        let album = query.album.as_deref().unwrap_or(&query.title);
        let url = format!(
            "{BASE}?method=album.search&album={}&api_key={}&format=json&limit=5",
            query_escape(album),
            query_escape(key)
        );
        let body = http.get_json(ID, &url, &[]).await?;
        Ok(parse(&body))
    }
}

/// The largest image in Last.fm's `[{"#text":…,"size":…}]` array.
///
/// The list is ordered smallest-first, but relying on that would silently degrade
/// to a 34px thumbnail if it ever changed, so the size names are ranked.
fn best_image(entry: &serde_json::Value) -> Option<String> {
    let images = entry.get("image")?.as_array()?;
    let rank = |size: Option<&str>| match size {
        Some("mega") => 5,
        Some("extralarge") => 4,
        Some("large") => 3,
        Some("medium") => 2,
        Some("small") => 1,
        _ => 0,
    };
    images
        .iter()
        .filter_map(|image| {
            let url = text(image, "#text")?;
            Some((rank(image.get("size").and_then(|size| size.as_str())), url))
        })
        .max_by_key(|(rank, _)| *rank)
        .map(|(_, url)| url)
}

fn parse(body: &serde_json::Value) -> Vec<Candidate> {
    let Some(matches) = body
        .get("results")
        .and_then(|results| results.get("albummatches"))
        .and_then(|matches| matches.get("album"))
        .and_then(|album| album.as_array())
    else {
        return Vec::new();
    };

    matches
        .iter()
        .filter_map(|entry| {
            let name = text(entry, "name")?;
            // Last.fm has no stable numeric id for an album; the MBID is there when
            // it knows one, otherwise artist/name is the only handle it offers.
            let artist = text(entry, "artist");
            let id = text(entry, "mbid").unwrap_or_else(|| match &artist {
                Some(artist) => format!("{artist} - {name}"),
                None => name.clone(),
            });
            let mut candidate = Candidate::new(ID, "album", id, name);
            candidate.original_title = artist;
            candidate.artwork_url = best_image(entry);
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
            "results": { "albummatches": { "album": [{
                "name": "Led Zeppelin IV",
                "artist": "Led Zeppelin",
                "mbid": "abc-123",
                "image": [
                    { "#text": "https://x.test/s.png", "size": "small" },
                    { "#text": "https://x.test/xl.png", "size": "extralarge" },
                    { "#text": "https://x.test/m.png", "size": "medium" }
                ]
            }]}}
        })
    }

    #[test]
    fn reads_an_album_search_response() {
        let candidates = parse(&fixture());
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title, "Led Zeppelin IV");
        assert_eq!(candidates[0].original_title.as_deref(), Some("Led Zeppelin"));
        assert_eq!(candidates[0].remote_id, "abc-123");
    }

    #[test]
    fn the_largest_image_wins_regardless_of_array_order() {
        assert_eq!(
            parse(&fixture())[0].artwork_url.as_deref(),
            Some("https://x.test/xl.png")
        );
    }

    #[test]
    fn an_album_without_an_mbid_still_gets_an_id() {
        let body = serde_json::json!({
            "results": { "albummatches": { "album": [
                { "name": "An Album", "artist": "An Artist" }
            ]}}
        });
        assert_eq!(parse(&body)[0].remote_id, "An Artist - An Album");
    }

    #[test]
    fn an_error_response_yields_nothing() {
        let body = serde_json::json!({ "error": 6, "message": "Invalid parameters" });
        assert!(parse(&body).is_empty());
    }
}

//! Per-provider request pacing.
//!
//! These limits are not politeness, they are the terms of use. MusicBrainz
//! enforces one request per second at the server and starts returning 503 to an
//! IP that exceeds it; Jikan and AniList do the same on their own schedules. A
//! library fetch walks thousands of files, so without pacing the first hundred
//! lookups would succeed and the rest would be rejected.
//!
//! The gate holds its lock across the sleep. That is deliberate: it serializes
//! the provider rather than merely spacing whoever happens to check, which is
//! what a hard ceiling requires when several files are in flight at once.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Minimum spacing between two requests to the same provider.
///
/// Set from each publisher's documented ceiling with a little headroom, since the
/// limit is enforced on arrival time at their end and not on departure from ours.
fn interval_for(provider: &str) -> Duration {
    let millis = match provider {
        // Documented as a hard 1/s, applied per IP.
        "musicbrainz" | "coverartarchive" => 1_100,
        // 60 requests/minute for an authenticated token.
        "discogs" => 1_100,
        // 3/s and 60/min — the minute budget is the binding one.
        "jikan" => 1_050,
        // 90/min.
        "anilist" => 700,
        "tvmaze" => 500,
        "kitsu" => 300,
        "lastfm" | "omdb" | "genius" => 250,
        // TMDb removed its published per-second cap, but hammering it still
        // invites a block.
        "tmdb" => 60,
        _ => 250,
    };
    Duration::from_millis(millis)
}

/// One gate per provider, created on first use.
#[derive(Default)]
pub struct RateLimiters {
    gates: Mutex<HashMap<&'static str, std::sync::Arc<Mutex<Option<Instant>>>>>,
}

impl RateLimiters {
    pub fn new() -> Self {
        Self::default()
    }

    /// Block until it is this provider's turn.
    pub async fn acquire(&self, provider: &'static str) {
        let gate = {
            let mut gates = self.gates.lock().await;
            gates.entry(provider).or_default().clone()
        };

        let interval = interval_for(provider);
        let mut last = gate.lock().await;
        if let Some(previous) = *last {
            let elapsed = previous.elapsed();
            if elapsed < interval {
                tokio::time::sleep(interval - elapsed).await;
            }
        }
        *last = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_first_request_to_a_provider_is_not_delayed() {
        let limiters = RateLimiters::new();
        let started = Instant::now();
        limiters.acquire("musicbrainz").await;
        assert!(started.elapsed() < Duration::from_millis(100));
    }

    #[tokio::test]
    async fn a_second_request_waits_for_the_interval() {
        let limiters = RateLimiters::new();
        limiters.acquire("musicbrainz").await;
        let started = Instant::now();
        limiters.acquire("musicbrainz").await;
        // MusicBrainz's ceiling is the one that gets an IP blocked, so this is the
        // case worth asserting rather than the general shape.
        assert!(started.elapsed() >= Duration::from_millis(1_000));
    }

    #[tokio::test]
    async fn providers_do_not_wait_on_each_other() {
        let limiters = RateLimiters::new();
        limiters.acquire("musicbrainz").await;
        let started = Instant::now();
        limiters.acquire("tmdb").await;
        assert!(started.elapsed() < Duration::from_millis(100));
    }
}

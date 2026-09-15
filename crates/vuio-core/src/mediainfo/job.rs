//! The library-wide fetch.
//!
//! One run walks every file that has no usable record yet, asks the providers
//! that suit it, scores what comes back and keeps the best answer. It is slow by
//! construction — MusicBrainz alone allows one request a second — so it reports
//! progress as it goes and can be cancelled, rather than being a request that
//! either returns or times out.

use super::artwork::ArtworkCache;
use super::client::{Candidate, Fetcher, MediaQuery, MediaQueryKind};
use super::credentials::CredentialStore;
use super::matching::{parse_media_name, score_candidate};
use super::MEDIAINFO_VERSION;
use crate::database::{DatabaseManager, MediaFile, MediaInfoRecord};
use crate::state::AppState;
use anyhow::{bail, Result};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio_util::sync::CancellationToken;

/// Records are written in batches: one transaction per file would make the write
/// lock the bottleneck in a job that is otherwise waiting on the network.
const WRITE_BATCH: usize = 25;

/// What the dashboard polls while a run is in progress.
#[derive(Clone, Debug, Default)]
pub struct MediaInfoJobState {
    pub running: bool,
    pub total: usize,
    pub processed: usize,
    pub matched: usize,
    pub low_confidence: usize,
    pub failed: usize,
    /// The file being looked at, for the progress line.
    pub current: Option<String>,
    pub started_at: Option<SystemTime>,
    pub finished_at: Option<SystemTime>,
    pub last_error: Option<String>,
    pub cancelled: bool,
    /// Held so a later request can stop the run. Not reported to the client.
    pub cancel: Option<CancellationToken>,
    /// Which run this state describes. Bumped by every [`Self::reserve`].
    ///
    /// A run only writes to the state while the generation it was given is
    /// still the current one, so a worker that has been superseded — or that
    /// finishes after a later run has started — cannot report its own
    /// completion over the run that is actually going.
    pub generation: u64,
}

impl MediaInfoJobState {
    /// Claim the job for a new run, if no run holds it.
    ///
    /// Claiming is separate from [`Self::begin`] because setting a run up takes
    /// a database query, and checking `running` on one side of that await and
    /// claiming it on the other is not a claim at all: two requests arriving
    /// together both saw an idle job, both started a worker, and the second
    /// `begin` overwrote the first's cancellation token — so cancelling reached
    /// only one of them and either could report the run finished while the
    /// other was still going.
    fn reserve(&mut self) -> Option<u64> {
        if self.running {
            return None;
        }
        let generation = self.generation.wrapping_add(1);
        *self = Self {
            running: true,
            started_at: Some(SystemTime::now()),
            generation,
            ..Self::default()
        };
        Some(generation)
    }

    /// Release a reservation whose run never started, so the next request can
    /// claim the job instead of waiting out a run that is not happening.
    fn abandon(&mut self, generation: u64, error: Option<String>) {
        if self.generation != generation {
            return;
        }
        self.running = false;
        self.started_at = None;
        self.finished_at = Some(SystemTime::now());
        if error.is_some() {
            self.last_error = error;
        }
    }

    /// Record what the reserved run turned out to consist of.
    fn begin(&mut self, generation: u64, total: usize, cancel: CancellationToken) -> bool {
        if self.generation != generation {
            return false;
        }
        self.total = total;
        self.cancel = Some(cancel);
        true
    }

    fn finish(&mut self, generation: u64, cancelled: bool, error: Option<String>) {
        if self.generation != generation {
            return;
        }
        self.running = false;
        self.cancelled = cancelled;
        self.finished_at = Some(SystemTime::now());
        self.current = None;
        self.cancel = None;
        if error.is_some() {
            self.last_error = error;
        }
    }
}

fn is_season_folder(folder_name: &str) -> bool {
    let lowered = folder_name.to_ascii_lowercase();
    let trimmed = lowered.trim();
    if trimmed.starts_with("season")
        || trimmed.starts_with("specials")
        || trimmed.starts_with("extra")
        || trimmed.starts_with("extras")
    {
        return true;
    }
    if let Some(rest) = trimmed.strip_prefix('s').filter(|_| trimmed.len() <= 4) {
        if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()) {
            return true;
        }
    }
    false
}

fn extract_title_from_parent_dirs(path: &std::path::Path) -> Option<String> {
    let mut current = path.parent()?;
    while let Some(name_os) = current.file_name() {
        let name = name_os.to_string_lossy();
        if is_season_folder(&name) {
            current = current.parent()?;
            continue;
        }
        let parsed = parse_media_name(&name);
        if !parsed.title.is_empty() {
            return Some(parsed.title);
        }
        if !name.trim().is_empty() {
            return Some(name.to_string());
        }
        current = current.parent()?;
    }
    None
}

/// Turn a media record into something worth searching for.
///
/// Returns `None` for anything there is no point asking about — images, and
/// audio that is really an internet radio stream.
fn query_for(file: &MediaFile) -> Option<(MediaQueryKind, MediaQuery)> {
    if file.mime_type.starts_with("image/") || file.mime_type == "audio/radio" {
        return None;
    }

    if file.mime_type.starts_with("audio/") {
        // Audio already went through a tag reader, so the filename is the worst
        // source available and is only used when the tags gave nothing.
        let title = file
            .title
            .clone()
            .unwrap_or_else(|| file.filename.clone());
        let query = MediaQuery {
            title,
            year: file.year,
            artist: file.artist.clone().or_else(|| file.album_artist.clone()),
            album: file.album.clone(),
            musicbrainz_release_id: file.tags.musicbrainz_album_id.clone(),
            musicbrainz_track_id: file.tags.musicbrainz_track_id.clone(),
            ..MediaQuery::default()
        };
        if query.title.is_empty() && query.album.is_none() {
            return None;
        }
        return Some((MediaQueryKind::Music, query));
    }

    if !file.mime_type.starts_with("video/") {
        return None;
    }

    let stem = std::path::Path::new(&file.filename)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| file.filename.clone());
    let mut parsed = parse_media_name(&stem);
    if parsed.title.is_empty() {
        if let Some(parent_title) = extract_title_from_parent_dirs(&file.path) {
            parsed.title = parent_title;
        }
    }
    if parsed.title.is_empty() {
        return None;
    }
    let query = MediaQuery {
        title: parsed.title,
        year: parsed.year,
        season: parsed.season,
        episode: parsed.episode,
        ..MediaQuery::default()
    };
    Some((parsed.kind, query))
}

fn record_from(
    media_file_id: i64,
    candidate: &Candidate,
    confidence: u8,
    artwork_key: Option<String>,
) -> MediaInfoRecord {
    MediaInfoRecord {
        media_file_id,
        provider: candidate.provider.to_string(),
        remote_id: candidate.remote_id.clone(),
        kind: candidate.kind.to_string(),
        title: Some(candidate.title.clone()),
        original_title: candidate.original_title.clone(),
        overview: candidate.overview.clone(),
        release_date: candidate.release_date.clone(),
        year: candidate.year,
        rating: candidate.rating,
        genres: candidate.genres.clone(),
        season: candidate.season,
        episode: candidate.episode,
        artwork_key,
        payload: serde_json::to_string(&candidate.payload).unwrap_or_else(|_| "null".to_string()),
        confidence,
        fetched_at: SystemTime::now(),
        mediainfo_version: MEDIAINFO_VERSION,
    }
}

/// Run a fetch over the whole library.
///
/// Returns as soon as the run is set up; the work happens on `background_tasks`
/// so the HTTP request that started it does not have to stay open for what may be
/// hours.
pub async fn run_library_fetch<D: DatabaseManager + 'static>(state: AppState<D>) -> Result<usize> {
    let config = state.current_config();
    let settings = config.mediainfo.clone();
    if !settings.enabled {
        bail!("Online media info is turned off");
    }

    // Claimed before the query below, not after: the check and the claim have
    // to be one step, or two requests arriving together each start a run.
    let Some(generation) = state.mediainfo_job.lock().await.reserve() else {
        bail!("A media info fetch is already running");
    };

    let threshold = settings.min_confidence.min(100);
    let pending = match state
        .database
        .media_ids_missing_mediainfo(MEDIAINFO_VERSION, threshold)
        .await
    {
        Ok(pending) => pending,
        Err(error) => {
            // Nothing is going to run, so the job must not be left claimed.
            state
                .mediainfo_job
                .lock()
                .await
                .abandon(generation, Some(error.to_string()));
            return Err(error);
        }
    };
    let total = pending.len();

    // A child of the application token, so shutdown stops the run without the
    // caller having to remember to cancel it.
    let cancel = state.cancellation.child_token();
    if !state
        .mediainfo_job
        .lock()
        .await
        .begin(generation, total, cancel.clone())
    {
        bail!("A media info fetch is already running");
    }

    let tracker = state.background_tasks.clone();
    tracker.spawn(async move {
        let outcome = fetch_all(&state, generation, pending, cancel.clone()).await;
        let cancelled = cancel.is_cancelled();
        let error = outcome.err().map(|error| error.to_string());
        if let Some(error) = error.as_deref() {
            tracing::warn!(error, "Media info fetch ended early");
        }
        {
            let mut job = state.mediainfo_job.lock().await;
            job.finish(generation, cancelled, error);
        }
        // Titles, descriptions and artwork all just changed. This bumps the
        // ContentDirectory revision, drops the browse cache and notifies every
        // UPnP subscriber, which is what makes a TV redraw with the new data.
        crate::web::eventing::publish_content_change(&state).await;
    });

    Ok(total)
}

async fn fetch_all<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    generation: u64,
    pending: Vec<i64>,
    cancel: CancellationToken,
) -> Result<()> {
    let config = state.current_config();
    let settings = &config.mediainfo;
    let threshold = settings.min_confidence.min(100);

    let credentials =
        CredentialStore::load(state.database.clone() as Arc<dyn crate::database::SecretStore>)
            .await?;
    let http = Fetcher::new(Duration::from_secs(settings.request_timeout_seconds.max(1)))?;
    let providers = super::providers::build(&settings.providers);
    if providers.is_empty() {
        bail!("No media info providers are enabled");
    }

    let artwork = settings
        .artwork_enabled
        .then(|| settings.artwork_path.as_ref().map(ArtworkCache::new))
        .flatten();

    let mut batch: Vec<MediaInfoRecord> = Vec::with_capacity(WRITE_BATCH);

    for media_file_id in pending {
        if cancel.is_cancelled() {
            break;
        }

        let Some(file) = state.database.get_file_by_id(media_file_id).await? else {
            continue;
        };
        {
            let mut job = state.mediainfo_job.lock().await;
            if job.generation != generation {
                // Superseded. Its progress is not ours to write.
                return Ok(());
            }
            job.current = Some(file.filename.clone());
        }

        let outcome = match query_for(&file) {
            Some((kind, query)) => {
                fetch_one(
                    &http,
                    &providers,
                    &credentials,
                    artwork.as_ref(),
                    kind,
                    &query,
                    media_file_id,
                )
                .await
            }
            // Nothing worth asking about is not a failure, it is a file this
            // feature does not apply to.
            None => Ok(None),
        };

        let mut job = state.mediainfo_job.lock().await;
        if job.generation != generation {
            return Ok(());
        }
        job.processed += 1;
        match outcome {
            Ok(Some(record)) => {
                if record.confidence >= threshold {
                    job.matched += 1;
                } else {
                    job.low_confidence += 1;
                }
                batch.push(record);
            }
            Ok(None) => {}
            Err(error) => {
                job.failed += 1;
                job.last_error = Some(error.to_string());
                tracing::debug!(file = %file.filename, %error, "Media info lookup failed");
            }
        }
        drop(job);

        if batch.len() >= WRITE_BATCH {
            state.database.bulk_store_mediainfo(&batch).await?;
            batch.clear();
        }
    }

    if !batch.is_empty() {
        state.database.bulk_store_mediainfo(&batch).await?;
    }
    Ok(())
}

/// Ask every suitable provider and keep the best-scoring answer.
///
/// Providers are tried in the order configured, and the search stops as soon as
/// one returns a candidate good enough to be trusted — a second opinion costs a
/// second of rate limit and cannot improve on a match already above the bar.
#[allow(clippy::too_many_arguments)]
async fn fetch_one(
    http: &Fetcher,
    providers: &[Box<dyn super::client::MetadataProvider>],
    credentials: &CredentialStore,
    artwork: Option<&ArtworkCache>,
    kind: MediaQueryKind,
    query: &MediaQuery,
    media_file_id: i64,
) -> Result<Option<MediaInfoRecord>> {
    let mut best: Option<(u8, Candidate)> = None;
    let mut last_error: Option<anyhow::Error> = None;

    for provider in providers {
        let info = provider.info();
        if !super::providers::serves(info.kind, kind) {
            continue;
        }
        let credential = credentials.get(info.id).await;
        // A provider that needs a key it has not been given is skipped silently:
        // it is off, not broken.
        if info.needs_credential() && credential.is_none() {
            continue;
        }

        match provider.search(http, query, credential.as_deref()).await {
            Ok(candidates) => {
                for candidate in candidates {
                    let score = score_candidate(query, &candidate);
                    if best.as_ref().is_none_or(|(current, _)| score > *current) {
                        best = Some((score, candidate));
                    }
                }
            }
            Err(error) => last_error = Some(error),
        }

        if best.as_ref().is_some_and(|(score, _)| *score >= 90) {
            break;
        }
    }

    let Some((confidence, candidate)) = best else {
        // Only report a failure if a provider actually errored. Everyone
        // answering "no such thing" is a miss, and counting it as an error would
        // fill the dashboard with noise for a library of home videos.
        return match last_error {
            Some(error) => Err(error),
            None => Ok(None),
        };
    };

    let artwork_key = match (artwork, candidate.artwork_url.as_deref()) {
        (Some(cache), Some(url)) => match cache.store(http, candidate.provider, url).await {
            Ok(key) => Some(key),
            // A read-only cache directory, or a poster that 404s, must not cost us
            // the metadata we already have.
            Err(error) => {
                tracing::debug!(%error, "Could not cache artwork");
                None
            }
        },
        _ => None,
    };

    Ok(Some(record_from(
        media_file_id,
        &candidate,
        confidence,
        artwork_key,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn media_file(filename: &str, mime: &str) -> MediaFile {
        let mut file = MediaFile::new(
            std::path::PathBuf::from(format!("/media/{filename}")),
            1024,
            mime.to_string(),
        );
        file.filename = filename.to_string();
        file
    }

    #[test]
    fn a_video_query_comes_from_the_filename() {
        let file = media_file("Show.Name.S02E05.1080p.WEB-DL.mkv", "video/x-matroska");
        let (kind, query) = query_for(&file).unwrap();
        assert_eq!(kind, MediaQueryKind::Episode);
        assert_eq!(query.title, "Show Name");
        assert_eq!(query.season, Some(2));
        assert_eq!(query.episode, Some(5));
    }

    #[test]
    fn an_audio_query_prefers_tags_over_the_filename() {
        // The filename here is useless; the tags are not. Parsing the name anyway
        // would throw away the better source.
        let mut file = media_file("01 - track.mp3", "audio/mpeg");
        file.title = Some("Black Dog".to_string());
        file.artist = Some("Led Zeppelin".to_string());
        file.album = Some("Led Zeppelin IV".to_string());

        let (kind, query) = query_for(&file).unwrap();
        assert_eq!(kind, MediaQueryKind::Music);
        assert_eq!(query.title, "Black Dog");
        assert_eq!(query.artist.as_deref(), Some("Led Zeppelin"));
        assert_eq!(query.album.as_deref(), Some("Led Zeppelin IV"));
    }

    #[test]
    fn a_musicbrainz_id_from_the_tags_is_carried_into_the_query() {
        let mut file = media_file("track.flac", "audio/flac");
        file.title = Some("A Song".to_string());
        file.tags.musicbrainz_album_id = Some("release-123".to_string());
        let (_, query) = query_for(&file).unwrap();
        assert_eq!(query.musicbrainz_release_id.as_deref(), Some("release-123"));
    }

    #[test]
    fn images_and_radio_streams_are_not_looked_up() {
        assert!(query_for(&media_file("photo.jpg", "image/jpeg")).is_none());
        assert!(query_for(&media_file("Some Station", "audio/radio")).is_none());
    }

    #[test]
    fn a_record_carries_the_current_version_so_a_bump_invalidates_it() {
        let candidate = Candidate::new("tvmaze", "series", "1".into(), "Show".into());
        let record = record_from(7, &candidate, 88, Some("abc".into()));
        assert_eq!(record.media_file_id, 7);
        assert_eq!(record.confidence, 88);
        assert_eq!(record.mediainfo_version, MEDIAINFO_VERSION);
        assert_eq!(record.artwork_key.as_deref(), Some("abc"));
        assert!(record.fetched_at > UNIX_EPOCH);
    }

    #[test]
    fn beginning_a_run_clears_the_previous_ones_counters() {
        let mut job = MediaInfoJobState {
            processed: 99,
            failed: 4,
            last_error: Some("old".into()),
            ..MediaInfoJobState::default()
        };
        let generation = job.reserve().expect("idle job");
        assert!(job.begin(generation, 10, CancellationToken::new()));
        assert!(job.running);
        assert_eq!(job.total, 10);
        assert_eq!(job.processed, 0);
        assert_eq!(job.failed, 0);
        assert!(job.last_error.is_none());
    }

    #[test]
    fn finishing_records_why_it_stopped() {
        let mut job = MediaInfoJobState::default();
        let generation = job.reserve().expect("idle job");
        assert!(job.begin(generation, 1, CancellationToken::new()));
        job.finish(generation, true, Some("boom".into()));
        assert!(!job.running);
        assert!(job.cancelled);
        assert!(job.cancel.is_none());
        assert_eq!(job.last_error.as_deref(), Some("boom"));
    }

    /// The job is claimed before the run is set up, so a second request that
    /// arrives while the first is still querying the database is refused.
    ///
    /// It used to check `running` and claim the job on opposite sides of that
    /// query, so both requests got through, both started a worker, and the
    /// second claim replaced the first's cancellation token — cancelling then
    /// reached one run and left the other going.
    #[test]
    fn a_second_request_cannot_claim_a_reserved_job() {
        let mut job = MediaInfoJobState::default();
        let first = job.reserve().expect("idle job");
        assert!(job.running, "reserving marks the job taken immediately");
        assert!(
            job.reserve().is_none(),
            "no second run while the first holds the job"
        );

        // And the loser cannot write over the winner's run either.
        let stale = first.wrapping_sub(1);
        assert!(!job.begin(stale, 99, CancellationToken::new()));
        assert_eq!(job.total, 0);
        job.finish(stale, true, Some("not mine".into()));
        assert!(job.running, "a superseded run cannot end the current one");
        assert!(job.last_error.is_none());

        job.finish(first, false, None);
        assert!(!job.running);
    }

    /// A reservation whose setup fails has to be released, or the next request
    /// waits out a run that is not happening.
    #[test]
    fn an_abandoned_reservation_frees_the_job() {
        let mut job = MediaInfoJobState::default();
        let generation = job.reserve().expect("idle job");
        job.abandon(generation, Some("the query failed".into()));
        assert!(!job.running);
        assert_eq!(job.last_error.as_deref(), Some("the query failed"));
        assert!(job.reserve().is_some(), "the job is claimable again");
    }
}

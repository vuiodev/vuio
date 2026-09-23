//! The playout task: one per live station.
//!
//! This is what makes a station a broadcast rather than a download. A single
//! task reads the queue, cuts each track into frames and pushes them out at the
//! speed they are meant to be heard — never faster. Everyone listening is
//! reading the same task's output, so they are all at the same point in the
//! same track, and someone who connects halfway through a song hears the second
//! half of it.
//!
//! The pieces that follow from that:
//!
//! - **Pacing.** After every chunk the task sleeps until the wall clock catches
//!   up with the audio clock, less a couple of seconds of lead so a listener has
//!   something buffered. Without it the whole file would arrive at once and the
//!   stream would end at the last byte.
//! - **One sender, many receivers.** Chunks go to a [`broadcast`] channel. A
//!   listener who cannot keep up is skipped forward rather than stalling the
//!   station, which is the right trade for radio: being behind is worse than
//!   missing a moment.
//! - **A burst on join.** The last few seconds are kept and handed to a new
//!   listener before it starts following the live channel, so playback begins
//!   immediately instead of after the first chunk arrives.
//! - **A cursor.** The track that is playing is written back to the database, so
//!   a station that comes back after a restart continues its queue instead of
//!   starting it again.

use crate::database::{
    BroadcastMode, DatabaseManager, DatabaseReadSession, MediaFileQuery, MediaFileView,
    RadioStation,
};
use crate::radio::frames::{Codec, Frame, TrackReader};
use crate::state::AppState;
use anyhow::{bail, Result};
use bytes::{Bytes, BytesMut};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, watch};
use tokio_util::sync::CancellationToken;

/// How much audio goes out in one chunk. At 128 kbit/s this is about half a
/// second, which is fine-grained enough to pace smoothly and coarse enough that
/// the channel is not doing more work than the disk.
const CHUNK_BYTES: usize = 8 * 1024;

/// How far ahead of the listener's clock the station is allowed to run. This is
/// the buffer a player has to survive a stutter.
const LEAD: Duration = Duration::from_secs(2);

/// How much recent audio is kept for listeners who have just joined.
const BURST_BYTES: usize = 64 * 1024;

/// Chunks a slow listener may fall behind before being skipped forward.
const CHANNEL_DEPTH: usize = 256;

/// What a station is playing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NowPlaying {
    pub title: String,
    pub artist: Option<String>,
    pub path: Option<String>,
    /// When this track started, for a listener that wants to show progress.
    pub started_at_epoch_secs: u64,
}

impl NowPlaying {
    /// The `StreamTitle` an ICY-aware player displays.
    pub fn stream_title(&self) -> String {
        match &self.artist {
            Some(artist) if !artist.is_empty() => format!("{artist} - {}", self.title),
            _ => self.title.clone(),
        }
    }
}

/// Paths are needed only while sorting and resuming a queue. Playout retains
/// just the ids and loads one track's current metadata when it reaches it.
#[derive(Clone, Debug)]
struct Track {
    id: i64,
    path: PathBuf,
}

/// A station that is on the air.
pub struct Station {
    pub id: i64,
    pub name: String,
    pub genre: String,
    pub codec: Codec,
    /// How many files the folders held that cannot be broadcast.
    pub skipped_files: usize,
    pub queue_len: usize,
    started_at: Instant,
    audio: broadcast::Sender<Bytes>,
    now_playing: watch::Sender<NowPlaying>,
    listeners: Arc<AtomicUsize>,
    burst: Arc<Mutex<VecDeque<Bytes>>>,
    /// Bumped to ask the playout task to move on to the next track.
    skip_requests: Arc<AtomicU64>,
    /// Consecutive passes over the queue that produced no audio at all, which is
    /// what a station whose files have gone looks like from in here. Reset by the
    /// first chunk that goes out. See [`Playout::run`].
    silent_passes: Arc<AtomicU32>,
    cancel: CancellationToken,
}

/// A station as the API reports it.
#[derive(Clone, Debug)]
pub struct StationSnapshot {
    pub id: i64,
    pub name: String,
    pub genre: String,
    pub codec: Codec,
    pub listeners: usize,
    pub uptime: Duration,
    pub now_playing: NowPlaying,
    pub queue_len: usize,
    pub skipped_files: usize,
}

/// What a new listener needs: the recent past, then the live feed.
pub struct Attachment {
    pub burst: Vec<Bytes>,
    pub audio: broadcast::Receiver<Bytes>,
    pub now_playing: watch::Receiver<NowPlaying>,
    pub listeners: Arc<AtomicUsize>,
}

impl Station {
    /// Build a station's queue and put it on the air.
    pub async fn start<D: DatabaseManager + 'static>(
        state: AppState<D>,
        row: &RadioStation,
    ) -> Result<Arc<Self>> {
        let plan = build_queue(&state, row).await?;

        let (audio, _) = broadcast::channel(CHANNEL_DEPTH);
        let (now_playing, _) = watch::channel(NowPlaying::default());
        let cancel = state.cancellation.child_token();

        let station = Arc::new(Self {
            id: row.id,
            name: row.name.clone(),
            genre: row.genre.clone(),
            codec: plan.codec,
            skipped_files: plan.skipped,
            queue_len: plan.tracks.len(),
            started_at: Instant::now(),
            audio,
            now_playing,
            listeners: Arc::new(AtomicUsize::new(0)),
            burst: Arc::new(Mutex::new(VecDeque::new())),
            skip_requests: Arc::new(AtomicU64::new(0)),
            silent_passes: Arc::new(AtomicU32::new(0)),
            cancel: cancel.clone(),
        });

        let task = Playout {
            station: station.clone(),
            state: state.clone(),
            row: row.clone(),
            tracks: plan.tracks,
            random: SplitMix64::new(row.seed),
        };

        state.background_tasks.clone().spawn(async move {
            task.run().await;
        });

        Ok(station)
    }

    /// Everything a listener needs to start receiving audio.
    ///
    /// Subscribing and snapshotting the burst both happen under the burst lock;
    /// see [`Station::publish`] for why the two must not be split.
    pub fn attach(&self) -> Attachment {
        let held = self.burst.lock();
        let audio = self.audio.subscribe();
        let burst = held
            .map(|held| held.iter().cloned().collect())
            .unwrap_or_default();
        Attachment {
            burst,
            audio,
            now_playing: self.now_playing.subscribe(),
            listeners: self.listeners.clone(),
        }
    }

    /// Send a chunk to every listener and remember it for the next one to join.
    ///
    /// Both halves happen under the burst lock, which [`Station::attach`] also
    /// holds across subscribing and snapshotting. That pairing is what makes the
    /// burst-to-live handover seamless: a chunk published before a listener
    /// attaches is already in the burst it copies, and one published after it
    /// reaches the receiver it just created. Split either pair and a chunk that
    /// crosses a join lands in both (repeated audio) or in neither (a gap).
    /// Neither call here blocks, so nothing is held across an await.
    fn publish(&self, bytes: Bytes) {
        let burst = self.burst.lock();

        // An error here only means nobody is listening, which is not a problem
        // a station should react to: it stays on the air either way.
        let _ = self.audio.send(bytes.clone());

        if let Ok(mut burst) = burst {
            let mut held: usize = burst.iter().map(Bytes::len).sum();
            held += bytes.len();
            burst.push_back(bytes);
            while held > BURST_BYTES && burst.len() > 1 {
                if let Some(dropped) = burst.pop_front() {
                    held -= dropped.len();
                }
            }
        }
    }

    pub fn snapshot(&self) -> StationSnapshot {
        StationSnapshot {
            id: self.id,
            name: self.name.clone(),
            genre: self.genre.clone(),
            codec: self.codec,
            listeners: self.listeners.load(Ordering::Relaxed),
            uptime: self.started_at.elapsed(),
            now_playing: self.now_playing.borrow().clone(),
            queue_len: self.queue_len,
            skipped_files: self.skipped_files,
        }
    }

    pub fn now_playing(&self) -> NowPlaying {
        self.now_playing.borrow().clone()
    }

    pub fn listeners(&self) -> usize {
        self.listeners.load(Ordering::Relaxed)
    }

    /// How many passes over the queue in a row have produced no audio.
    ///
    /// Zero for a station that is playing. Anything else means every track was
    /// unreadable — an unmounted share, a folder emptied under it — and the
    /// playout task is waiting before trying the queue again.
    pub fn silent_passes(&self) -> u32 {
        self.silent_passes.load(Ordering::Relaxed)
    }

    /// Ask the playout task to move on to the next track.
    pub fn skip(&self) {
        self.skip_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Take the station off the air and wait for the task to notice.
    pub async fn shutdown(&self) {
        self.cancel.cancel();
    }

    /// Whether this instance has been taken off the air. A `Station` the
    /// [`RadioManager`](crate::radio::RadioManager) has replaced or stopped
    /// reads `true` here even though callers may still hold a handle to it.
    pub fn is_off_air(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

/// The queue a station will play, and what had to be left out of it.
struct QueuePlan {
    tracks: Vec<i64>,
    codec: Codec,
    skipped: usize,
}

/// Collect the station's folders into a queue.
///
/// `MediaFileQuery::Directory` matches one parent directory rather than a
/// subtree, so the filtering is done against the borrowed rows inside the read
/// transaction: only files that will actually be broadcast are ever
/// materialised.
async fn build_queue<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    row: &RadioStation,
) -> Result<QueuePlan> {
    if row.folders.is_empty() {
        bail!("this station has no folders to play from");
    }

    let folders: Vec<String> = row
        .folders
        .iter()
        .map(|folder| normalise(folder.trim_end_matches(['/', '\\'])))
        .collect();

    let collected = state
        .database
        .clone()
        .read(move |session| {
            let query = MediaFileQuery::Filtered {
                after_id: None,
                mime_family: Some("audio/".to_string()),
                text: None,
            };
            let mut mp3 = Vec::new();
            let mut aac = Vec::new();
            let mut skipped = 0usize;

            session.visit_files_page(&query, 0, i64::MAX as usize, |file| {
                let path = file.path();
                // Internet radio records are audio rows whose path is a URL.
                // There is no file here to read.
                if file.mime_type() == "audio/radio" {
                    return Ok(());
                }
                if !folders.iter().any(|folder| is_within(path, folder)) {
                    return Ok(());
                }
                let Some(id) = file.id() else { return Ok(()) };
                let destination = match crate::radio::frames::codec_for_path(Path::new(path)) {
                    Some(Codec::Mp3) => &mut mp3,
                    Some(Codec::Aac) => &mut aac,
                    None => { skipped += 1; return Ok(()) }
                };
                destination.push(Track { id, path: PathBuf::from(path) });
                Ok(())
            })?;

            Ok((mp3, aac, skipped))
        })
        .await?;

    let (mp3, aac, mut skipped) = collected;
    if mp3.is_empty() && aac.is_empty() {
        // Two different problems, and an operator fixes them differently: an
        // empty selection means the wrong folders or an unscanned library,
        // while a full one means the right folders in the wrong format.
        if skipped == 0 {
            bail!(
                "no audio was found in those folders — check the paths, \
                 and that the library has been scanned"
            );
        }
        bail!(
            "none of the {skipped} audio file(s) in those folders can be broadcast \
             — a station carries MP3 or AAC, and everything else would have to be re-encoded"
        );
    }

    // One stream, one codec: the response names a content type and a decoder is
    // entitled to believe it. The larger set wins and the other is counted as
    // skipped, which the studio shows.
    let (mut tracks, codec) = if mp3.len() >= aac.len() {
        skipped += aac.len();
        (mp3, Codec::Mp3)
    } else {
        skipped += mp3.len();
        (aac, Codec::Aac)
    };

    tracks.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    if row.mode == BroadcastMode::Shuffle {
        SplitMix64::new(row.seed).shuffle(&mut tracks);
    }

    resume_after(&mut tracks, row.cursor_path.as_deref());

    let mut ids: Vec<_> = tracks.into_iter().map(|track| track.id).collect();
    // collect may reuse the larger Track allocation in place.
    ids.shrink_to_fit();
    Ok(QueuePlan {
        tracks: ids,
        codec,
        skipped,
    })
}

/// Rotate the queue so it carries on from the track after `cursor`.
///
/// This is what a station resumed after a restart picks up from. The order
/// itself is already fixed by the station's seed, so continuing means starting
/// one past the track that was playing rather than at the top again.
///
/// A cursor naming a track that is no longer there — deleted, renamed, moved
/// out of the folders — leaves the queue alone and the station starts at the
/// beginning, which is the only other thing it could sensibly do.
fn resume_after(tracks: &mut [Track], cursor: Option<&str>) {
    let Some(cursor) = cursor else { return };
    if tracks.is_empty() {
        return;
    }
    if let Some(index) = tracks
        .iter()
        .position(|track| track.path.to_string_lossy() == cursor)
    {
        let resume_at = (index + 1) % tracks.len();
        tracks.rotate_left(resume_at);
    }
}

/// Whether `path` sits inside `folder`. Both are compared case-insensitively
/// with separators normalised, because a folder arrives as an operator typed it.
fn is_within(path: &str, folder: &str) -> bool {
    if folder.is_empty() {
        return false;
    }
    let path = normalise(path);
    path.starts_with(&format!("{folder}/")) || path == folder
}

fn normalise(value: &str) -> String {
    value.replace('\\', "/").to_lowercase()
}

/// The task that actually plays a station.
struct Playout<D: DatabaseManager + 'static> {
    station: Arc<Station>,
    state: AppState<D>,
    row: RadioStation,
    tracks: Vec<i64>,
    random: SplitMix64,
}

/// The station's playout clock: when, in real time, the audio emitted so far
/// was due to have been heard.
///
/// One per station rather than one per track. `wait` runs `LEAD` ahead of real
/// time so a player has something buffered, and that lead is the station's to
/// take once. Restarting the clock at every track took it again on each one,
/// so a station drifted a further two seconds ahead of its listeners per track
/// — after enough of them a whole short track could go out inside a burst.
struct PlayoutClock {
    /// Real time at which the stream's audio clock read zero.
    anchor: Instant,
    /// Audio emitted since then.
    audio: Duration,
}

impl PlayoutClock {
    fn new() -> Self {
        Self {
            anchor: Instant::now(),
            audio: Duration::ZERO,
        }
    }

    /// When the audio emitted so far is due to go out, allowing for the lead.
    fn due(&self) -> Instant {
        self.anchor + self.audio.saturating_sub(LEAD)
    }

    /// Re-anchor once reading has fallen more than the lead behind the clock —
    /// a slow disk, a stalled network share. Catching that up would mean
    /// emptying the reader at listeners as fast as it reads, which is what
    /// restarting the clock per track used to avoid; this keeps that protection
    /// without handing out a fresh lead at every track boundary.
    fn resynchronise(&mut self, now: Instant) {
        let behind = now.saturating_duration_since(self.due());
        if behind > LEAD {
            self.anchor += behind;
        }
    }
}

/// How long to wait after the nth consecutive pass that broadcast nothing.
fn silent_pass_backoff(passes: u32) -> Duration {
    const FIRST_SECONDS: u64 = 5;
    const LONGEST_SECONDS: u64 = 60;
    let doublings = passes.clamp(1, 6) - 1;
    Duration::from_secs((FIRST_SECONDS << doublings).min(LONGEST_SECONDS))
}

impl<D: DatabaseManager + 'static> Playout<D> {
    async fn run(mut self) {
        tracing::info!(
            station = %self.row.name,
            tracks = self.tracks.len(),
            codec = self.station.codec.as_str(),
            "Radio station is on the air"
        );

        let mut clock = PlayoutClock::new();

        loop {
            if self.station.cancel.is_cancelled() {
                break;
            }
            if self.tracks.is_empty() {
                tracing::warn!(
                    station = %self.row.name,
                    "Radio station has nothing left to play; going off the air"
                );
                self.stop_permanently().await;
                break;
            }

            let queue = std::mem::take(&mut self.tracks);
            let quiet_so_far = self.station.silent_passes() > 0;
            // The clock advances at emission, including audio sent before a later
            // read fails. A track's final Result cannot tell us whether it was silent.
            let audio_before_pass = clock.audio;
            for &track_id in &queue {
                if self.station.cancel.is_cancelled() {
                    return;
                }
                match self.play(track_id, &mut clock).await {
                    Ok(()) => {}
                    // A track that has been moved or is not what its extension
                    // claims should cost one track, not the station. Once a whole
                    // pass has failed this drops to debug: a station whose share
                    // has gone would otherwise write one warning per track per
                    // pass, for as long as it stays gone.
                    Err(error) if !quiet_so_far => tracing::warn!(
                        station = %self.row.name,
                        track_id,
                        "Skipping a track that could not be broadcast: {error:#}"
                    ),
                    Err(error) => tracing::debug!(
                        station = %self.row.name,
                        track_id,
                        "Skipping a track that could not be broadcast: {error:#}"
                    ),
                }
            }
            let broadcast_something = self.station.finish_pass(audio_before_pass, &clock);

            match self.row.mode {
                BroadcastMode::Linear => {
                    tracing::info!(
                        station = %self.row.name,
                        "Radio station reached the end of its queue"
                    );
                    self.stop_permanently().await;
                    break;
                }
                // Rebuilding rather than replaying picks up anything added to
                // the folders since the station started.
                BroadcastMode::Loop | BroadcastMode::Shuffle => {
                    // A pass that broadcast nothing and a pass that broadcast a
                    // queue are the same shape from here, and going straight
                    // round again is only right for the second. A station whose
                    // files have gone — an unmounted share, a folder emptied
                    // under it — failed to open every track, rebuilt the queue
                    // from rows that are all still there, and started over, with
                    // nothing anywhere on that path that waits. It span a core
                    // and wrote a warning per track per turn until the disk
                    // filled or someone stopped it.
                    if !broadcast_something && !self.wait_out_a_silent_pass().await {
                        return;
                    }
                    self.row.cursor_path = None;
                    match build_queue(&self.state, &self.row).await {
                        Ok(plan) => {
                            let mut tracks = plan.tracks;
                            if self.row.mode == BroadcastMode::Shuffle {
                                self.random.shuffle(&mut tracks);
                            }
                            self.tracks = tracks;
                        }
                        Err(error) => {
                            tracing::warn!(
                                station = %self.row.name,
                                "Could not rebuild the queue, replaying the last one: {error:#}"
                            );
                            self.tracks = queue;
                        }
                    }
                }
            }
        }
    }

    /// Wait before trying a queue that produced nothing again.
    ///
    /// Returns false if the station was taken off the air while waiting. The wait
    /// grows — five seconds, then ten, twenty, forty, and a minute from then on —
    /// so a share that comes back is picked up quickly and one that does not costs
    /// a query a minute.
    async fn wait_out_a_silent_pass(&self) -> bool {
        let passes = self
            .station
            .silent_passes
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        let wait = silent_pass_backoff(passes);
        // Once, on the pass that discovered it, rather than once per track.
        if passes == 1 {
            tracing::warn!(
                station = %self.row.name,
                seconds = wait.as_secs(),
                "Nothing in this station's queue could be broadcast; waiting before trying again"
            );
        }
        tokio::select! {
            _ = tokio::time::sleep(wait) => true,
            _ = self.station.cancel.cancelled() => false,
        }
    }

    /// Take the station off the air for good, so a restart does not resume it.
    async fn stop_permanently(&self) {
        if let Err(error) = self
            .state
            .database
            .set_radio_station_enabled(self.row.id, false)
            .await
        {
            tracing::warn!("Could not record that a station stopped: {error:#}");
        }
        self.state.radio.stop(self.row.id).await;
    }

    /// Play one track, in real time, to everyone listening. `clock` is the
    /// station's, not this track's — see [`PlayoutClock`].
    async fn play(&self, track_id: i64, clock: &mut PlayoutClock) -> Result<()> {
        let track = self
            .state
            .database
            .get_file_by_id(track_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("track {track_id} is no longer in the library"))?;
        let reader = TrackReader::open(&track.path, self.station.codec).await?;

        // `send_replace` rather than `send`: what is playing is state the studio
        // reads whether or not anyone is listening, and `send` refuses to store
        // a value while the channel has no receivers — which is exactly the
        // case for a station nobody has tuned into yet.
        self.station.now_playing.send_replace(NowPlaying {
            title: track.title.as_deref().filter(|title| !title.is_empty())
                .unwrap_or(&track.filename).to_owned(),
            artist: track.artist.as_ref().filter(|artist| !artist.is_empty()).cloned(),
            path: Some(track.path.to_string_lossy().into_owned()),
            started_at_epoch_secs: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|since| since.as_secs())
                .unwrap_or_default(),
        });
        self.record_cursor(&track.path).await;
        drop(track);

        self.station.play_reader(reader, clock).await
    }

    async fn record_cursor(&self, path: &Path) {
        let cursor = path.to_string_lossy().into_owned();
        if let Err(error) = self
            .state
            .database
            .set_radio_station_cursor(self.row.id, Some(&cursor))
            .await
        {
            tracing::debug!("Could not record the station cursor: {error:#}");
        }
    }
}

impl Station {
    fn finish_pass(&self, audio_before_pass: Duration, clock: &PlayoutClock) -> bool {
        let broadcast_something = clock.audio > audio_before_pass;
        if broadcast_something {
            self.silent_passes.store(0, Ordering::Relaxed);
        }
        broadcast_something
    }

    async fn play_reader(&self, mut reader: TrackReader, clock: &mut PlayoutClock) -> Result<()> {
        let skips_at_start = self.skip_requests.load(Ordering::Relaxed);
        let mut chunk = BytesMut::with_capacity(CHUNK_BYTES + 2048);
        let mut chunk_time = Duration::ZERO;

        loop {
            let frame = match reader.next_frame().await? {
                Some(frame) => frame,
                None => break,
            };
            self.accumulate(&mut chunk, &mut chunk_time, frame);

            if chunk.len() >= CHUNK_BYTES {
                self.emit(&mut chunk, &mut chunk_time, clock);
                if !self.wait(clock, skips_at_start).await {
                    return Ok(());
                }
            }
        }

        if !chunk.is_empty() {
            self.emit(&mut chunk, &mut chunk_time, clock);
            self.wait(clock, skips_at_start).await;
        }
        Ok(())
    }

    fn accumulate(&self, chunk: &mut BytesMut, chunk_time: &mut Duration, frame: Frame) {
        chunk.extend_from_slice(&frame.bytes);
        *chunk_time += frame.duration;
    }

    /// Close off the chunk being accumulated, advance the station clock by the
    /// audio it holds, and put it on the air.
    fn emit(&self, chunk: &mut BytesMut, chunk_time: &mut Duration, clock: &mut PlayoutClock) {
        let bytes = chunk.split().freeze();
        clock.audio += *chunk_time;
        *chunk_time = Duration::ZERO;

        self.publish(bytes);
    }

    /// Hold the stream to real time. Returns false if the track should end now.
    async fn wait(&self, clock: &mut PlayoutClock, skips_at_start: u64) -> bool {
        if self.skip_requests.load(Ordering::Relaxed) != skips_at_start {
            return false;
        }

        let target = clock.due();
        let now = Instant::now();
        if target > now {
            tokio::select! {
                _ = tokio::time::sleep(target - now) => {}
                _ = self.cancel.cancelled() => return false,
            }
        } else {
            if self.cancel.is_cancelled() {
                return false;
            }
            clock.resynchronise(now);
        }
        true
    }
}

/// A small deterministic generator, so a shuffled station resumes the order its
/// listeners were already hearing rather than drawing a new one.
struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for index in (1..items.len()).rev() {
            let swap = (self.next() % (index as u64 + 1)) as usize;
            items.swap(index, swap);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wait after a fruitless pass: quick enough that a share coming back is
    /// picked up while someone is still standing at the hi-fi, long enough that a
    /// station whose folder has gone costs one query a minute rather than a core.
    #[test]
    fn the_silent_pass_backoff_grows_and_settles() {
        let seconds = |passes| silent_pass_backoff(passes).as_secs();
        assert_eq!(seconds(1), 5);
        assert_eq!(seconds(2), 10);
        assert_eq!(seconds(3), 20);
        assert_eq!(seconds(4), 40);
        assert_eq!(seconds(5), 60);
        assert_eq!(seconds(100), 60, "and never longer than a minute");
        // Nothing calls it with zero, but a schedule that starts by not waiting
        // at all would be the bug this exists to fix.
        assert_eq!(seconds(0), 5);
    }

    #[test]
    fn a_seed_reproduces_its_shuffle() {
        let mut first: Vec<u32> = (0..64).collect();
        let mut second = first.clone();
        SplitMix64::new(12345).shuffle(&mut first);
        SplitMix64::new(12345).shuffle(&mut second);
        assert_eq!(first, second, "the same seed must give the same order");

        let mut third: Vec<u32> = (0..64).collect();
        SplitMix64::new(54321).shuffle(&mut third);
        assert_ne!(first, third, "a different seed must give a different order");
    }

    #[test]
    fn shuffling_keeps_every_track() {
        let mut items: Vec<u32> = (0..100).collect();
        SplitMix64::new(7).shuffle(&mut items);
        items.sort_unstable();
        assert_eq!(items, (0..100).collect::<Vec<_>>());
    }

    fn queue(names: &[&str]) -> Vec<Track> {
        names
            .iter()
            .enumerate()
            .map(|(id, name)| Track {
                id: id as i64,
                path: PathBuf::from(format!("/music/{name}.mp3")),
            })
            .collect()
    }

    fn order(tracks: &[Track]) -> Vec<String> {
        tracks
            .iter()
            .map(|track| {
                track
                    .path
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[test]
    fn a_resumed_station_carries_on_from_the_next_track() {
        let mut tracks = queue(&["a", "b", "c", "d"]);
        resume_after(&mut tracks, Some("/music/b.mp3"));
        assert_eq!(order(&tracks), ["c", "d", "a", "b"]);
    }

    #[test]
    fn resuming_from_the_last_track_wraps_to_the_first() {
        let mut tracks = queue(&["a", "b", "c"]);
        resume_after(&mut tracks, Some("/music/c.mp3"));
        assert_eq!(order(&tracks), ["a", "b", "c"]);
    }

    #[test]
    fn a_station_that_never_ran_starts_at_the_top() {
        let mut tracks = queue(&["a", "b", "c"]);
        resume_after(&mut tracks, None);
        assert_eq!(order(&tracks), ["a", "b", "c"]);
    }

    #[test]
    fn a_cursor_naming_a_track_that_is_gone_starts_at_the_top() {
        let mut tracks = queue(&["a", "b", "c"]);
        resume_after(&mut tracks, Some("/music/deleted.mp3"));
        assert_eq!(order(&tracks), ["a", "b", "c"]);

        let mut empty: Vec<Track> = Vec::new();
        resume_after(&mut empty, Some("/music/a.mp3"));
        assert!(empty.is_empty(), "an empty queue must not panic");
    }

    #[test]
    fn folder_matching_is_by_subtree() {
        assert!(is_within("/music/rock/a.mp3", "/music"));
        assert!(is_within("/music/rock/deep/a.mp3", "/music/rock"));
        assert!(is_within("/Music/Rock/A.mp3", "/music/rock"));
        assert!(is_within(r"C:\Music\a.mp3", "c:/music"));
        // A sibling whose name merely starts the same must not match.
        assert!(!is_within("/music-2/a.mp3", "/music"));
        assert!(!is_within("/other/a.mp3", "/music"));
        assert!(!is_within("/music/a.mp3", ""));
    }

    #[test]
    fn stream_titles_read_as_a_player_shows_them() {
        let playing = NowPlaying {
            title: "Sixteen Tons".to_owned(),
            artist: Some("Tennessee Ernie Ford".to_owned()),
            ..Default::default()
        };
        assert_eq!(
            playing.stream_title(),
            "Tennessee Ernie Ford - Sixteen Tons"
        );

        let untagged = NowPlaying {
            title: "track01".to_owned(),
            artist: None,
            ..Default::default()
        };
        assert_eq!(untagged.stream_title(), "track01");
    }

    /// A station for exercising the burst/live handover: no queue, no playout
    /// task, just the channel and the burst that `publish` and `attach` share.
    fn silent_station() -> Station {
        let (audio, _) = broadcast::channel(CHANNEL_DEPTH);
        let (now_playing, _) = watch::channel(NowPlaying::default());
        Station {
            id: 1,
            name: "test".to_owned(),
            genre: String::new(),
            codec: Codec::Mp3,
            skipped_files: 0,
            queue_len: 0,
            started_at: Instant::now(),
            audio,
            now_playing,
            listeners: Arc::new(AtomicUsize::new(0)),
            burst: Arc::new(Mutex::new(VecDeque::new())),
            skip_requests: Arc::new(AtomicU64::new(0)),
            silent_passes: Arc::new(AtomicU32::new(0)),
            cancel: CancellationToken::new(),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_read_failure_after_broadcasting_does_not_count_as_a_silent_pass() {
        use super::super::frames::Framing;

        let station = silent_station();
        station.silent_passes.store(3, Ordering::Relaxed);
        let mut listener = station.attach();
        let mut frame = vec![0u8; 417]; // MPEG-1 Layer III, 128 kbps, 44.1 kHz
        frame[..4].copy_from_slice(&[0xff, 0xfb, 0x90, 0]);
        let mut buffer = BytesMut::new();
        for _ in 0..30 {
            buffer.extend_from_slice(&frame);
        }
        // Buffered audio is valid. Once exhausted, reading a directory produces
        // an actual I/O error, like a source becoming unreadable mid-track.
        let temp = tempfile::tempdir().unwrap();
        let reader = TrackReader::Framed {
            file: tokio::fs::File::open(temp.path()).await.unwrap(),
            buffer,
            framing: Framing::Mpeg,
            eof: false,
            first_frame: false,
        };
        let mut clock = PlayoutClock::new();
        let before = clock.audio;
        assert!(station.play_reader(reader, &mut clock).await.is_err());
        assert!(
            listener.audio.try_recv().is_ok(),
            "audio reached the listener"
        );
        assert!(
            station.finish_pass(before, &clock),
            "partial playback must prevent backoff"
        );
        assert_eq!(
            station.silent_passes(),
            0,
            "partial playback resets prior failures"
        );
    }

    /// Whatever order a join and a chunk happen in, the listener hears that
    /// chunk exactly once: once from the burst it copied, or once from the
    /// channel it subscribed to, never both and never neither.
    #[test]
    fn joining_never_repeats_or_drops_a_chunk() {
        // A barrier rather than a bare spawn: the window is a handful of
        // instructions wide, and without lining the two threads up on it the
        // publisher reliably finishes before the join even starts.
        for _ in 0..5_000 {
            let station = Arc::new(silent_station());
            let gate = Arc::new(std::sync::Barrier::new(2));

            let publisher = {
                let station = Arc::clone(&station);
                let gate = Arc::clone(&gate);
                std::thread::spawn(move || {
                    gate.wait();
                    station.publish(Bytes::from_static(b"chunk"));
                })
            };
            gate.wait();
            let mut attachment = station.attach();
            publisher.join().expect("publisher thread");

            let mut heard = attachment.burst.len();
            while let Ok(chunk) = attachment.audio.try_recv() {
                assert_eq!(chunk, Bytes::from_static(b"chunk"));
                heard += 1;
            }
            assert_eq!(heard, 1, "the chunk must be heard exactly once");
        }
    }

    /// The handover is one critical section, not two. Holding the lock
    /// `publish` holds pins `attach` at its start, so publishing a chunk from
    /// here reproduces exactly the interleaving that used to double it: the bug
    /// subscribed before taking the lock, so the waiting listener received this
    /// chunk *and* then copied it out of the burst.
    #[test]
    fn a_chunk_published_across_a_join_is_heard_once() {
        let station = Arc::new(silent_station());
        let (ready, started) = std::sync::mpsc::channel();

        let mut held = station.burst.lock().expect("burst lock");
        let worker = {
            let station = Arc::clone(&station);
            std::thread::spawn(move || {
                let _ = ready.send(());
                station.attach()
            })
        };
        started.recv().expect("attach thread starts");
        // Long enough for that thread to reach the lock. Under the fix there is
        // nothing before it to reach; the bug subscribed on the way. Either way
        // a correct `attach` gives the same answer, so no timing here can make
        // this test fail against code that holds the invariant.
        std::thread::sleep(Duration::from_millis(200));

        // The publishing half by hand, in the order `publish` does it.
        let chunk = Bytes::from_static(b"chunk");
        let _ = station.audio.send(chunk.clone());
        held.push_back(chunk);
        drop(held);

        let mut attachment = worker.join().expect("attach thread");
        let mut heard = attachment.burst.len();
        while attachment.audio.try_recv().is_ok() {
            heard += 1;
        }
        assert_eq!(heard, 1, "the chunk must be heard exactly once");
    }

    /// The lead is the station's to take once. Taking it again at every track
    /// boundary pushed the station a further two seconds ahead of its listeners
    /// per track, so a long enough station ran arbitrarily far ahead of itself.
    #[test]
    fn the_lead_is_taken_once_for_the_station_not_once_per_track() {
        let mut clock = PlayoutClock::new();
        let anchor = clock.anchor;
        let chunk = Duration::from_millis(500);

        // Four tracks of two seconds each. Nothing about a track boundary is
        // visible to the clock, which is the point.
        let tracks = 4u32;
        let chunks_per_track = 4u32;
        for _ in 0..tracks {
            for _ in 0..chunks_per_track {
                clock.audio += chunk;
            }
        }

        let played = chunk * tracks * chunks_per_track;
        assert_eq!(
            clock.due(),
            anchor + played - LEAD,
            "the whole station runs exactly one lead ahead of real time"
        );
        // The shape the bug produced, for contrast: one lead per track.
        assert_ne!(clock.due(), anchor + played - LEAD * tracks);
    }

    /// Reading that falls far behind must not be caught up by dumping audio at
    /// listeners as fast as it comes off the disk.
    #[test]
    fn a_reader_that_stalls_re_anchors_instead_of_racing_to_catch_up() {
        let mut clock = PlayoutClock::new();
        clock.audio = Duration::from_secs(10);
        let due = clock.due();

        // A hiccup inside the lead is absorbed: the station keeps its place.
        clock.resynchronise(due + LEAD);
        assert_eq!(clock.due(), due, "a small stall does not move the anchor");

        // A longer one re-anchors to now, so the next chunk is due immediately
        // and no more than that.
        let now = due + LEAD + Duration::from_secs(30);
        clock.resynchronise(now);
        assert_eq!(clock.due(), now);
    }
}

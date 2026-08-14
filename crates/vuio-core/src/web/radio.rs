//! HTTP for live radio: the stream itself, and the console that runs it.
//!
//! Two surfaces, deliberately split.
//!
//! **Public.** The stream and the list of what is on the air need no login, for
//! the same reason `/media/{id}` does not: a station exists to be played, and
//! the things that play it — a hi-fi, VLC, a phone, another VuIO server
//! building its own local-stations list — have nowhere to put a password. This
//! is also what makes the "Local Radio Stations" tab work across servers.
//!
//! **Management.** Creating, editing, starting and stopping a station is
//! administration and sits behind the same middleware as the rest of the
//! console. Reading the list of stations is one thing; deciding what the
//! speakers in the house play is another.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use bytes::{BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;

use crate::{
    database::{BroadcastMode, DatabaseManager, RadioStation, RadioStationInput},
    error::AppError,
    radio::{engine::Station, peers::PeerServer, peers::PublishedStation},
    state::AppState,
};

/// How many audio bytes sit between ICY metadata blocks. 16 KiB is what every
/// SHOUTcast client has assumed since the format existed.
const ICY_METAINT: usize = 16_000;

/// The path a station's audio is served from, relative to the server root.
fn stream_path(id: i64, codec: crate::radio::frames::Codec) -> String {
    format!("/api/radio/stations/{id}/stream.{}", codec.extension())
}

// ---------------------------------------------------------------------------
// Public: what is on the air
// ---------------------------------------------------------------------------

/// Every station this server is broadcasting.
///
/// Public, and the reason peer discovery works: this is the endpoint another
/// VuIO server calls after finding this one over mDNS.
pub async fn list_public_stations<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
) -> Json<Vec<PublishedStation>> {
    let origin = state.advertised_http_origin();
    Json(
        state
            .radio
            .snapshots()
            .await
            .into_iter()
            .map(|snapshot| {
                let now_playing = snapshot.now_playing;
                PublishedStation {
                    id: snapshot.id,
                    name: snapshot.name,
                    genre: snapshot.genre,
                    codec: snapshot.codec.as_str().to_owned(),
                    stream_url: format!("{origin}{}", stream_path(snapshot.id, snapshot.codec)),
                    listeners: snapshot.listeners,
                    uptime_secs: snapshot.uptime.as_secs(),
                    now_playing: (!now_playing.title.is_empty())
                        .then(|| now_playing.stream_title()),
                    artist: now_playing.artist.clone(),
                    title: (!now_playing.title.is_empty()).then(|| now_playing.title.clone()),
                }
            })
            .collect(),
    )
}

/// Serve a station's live audio.
///
/// The listener joins wherever the station happens to be. What it gets first is
/// the burst — the last few seconds, already sent to everyone else — so playback
/// starts at once; after that it follows the live feed like every other
/// listener.
pub async fn serve_stream<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let station = state.radio.get(id).await.ok_or(AppError::NotFound)?;
    Ok(stream_response(&station, &headers))
}

/// The same, for players that will only open a URL ending in a file extension.
pub async fn serve_stream_with_extension<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    Path((id, _extension)): Path<(i64, String)>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let station = state.radio.get(id).await.ok_or(AppError::NotFound)?;
    Ok(stream_response(&station, &headers))
}

fn stream_response(station: &Arc<Station>, headers: &HeaderMap) -> Response {
    let wants_metadata = headers
        .get("icy-metadata")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim() == "1");

    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(station.codec.content_type()),
    );
    response_headers.insert("icy-name", icy_header(&station.name));
    response_headers.insert("icy-genre", icy_header(&station.genre));
    // Not listed in any public directory: this is a station on someone's LAN.
    response_headers.insert("icy-pub", HeaderValue::from_static("0"));
    response_headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-store"),
    );
    if wants_metadata {
        response_headers.insert(
            "icy-metaint",
            HeaderValue::from_str(&ICY_METAINT.to_string()).expect("a number is a valid header"),
        );
    }

    let attachment = station.attach();
    let guard = ListenerGuard::attach(attachment.listeners);
    let mut audio = attachment.audio;
    let now_playing = attachment.now_playing;
    let burst = attachment.burst;
    let mut icy = IcyWriter::new(wants_metadata.then_some(ICY_METAINT));

    let body = async_stream::stream! {
        // Held for the life of the response: dropping it is what makes the
        // listener count go back down when a player disconnects.
        let _guard = guard;
        let mut out = BytesMut::new();

        for chunk in burst {
            let title = now_playing.borrow().stream_title();
            icy.push(&chunk, &title, &mut out);
            yield Ok::<Bytes, std::io::Error>(out.split().freeze());
        }

        loop {
            match audio.recv().await {
                Ok(chunk) => {
                    let title = now_playing.borrow().stream_title();
                    icy.push(&chunk, &title, &mut out);
                    yield Ok(out.split().freeze());
                }
                // Falling behind is not fatal for radio: catching up matters
                // more than hearing every byte, so the listener is moved
                // forward to the live edge.
                Err(RecvError::Lagged(missed)) => {
                    tracing::debug!(missed, "A radio listener fell behind and was skipped forward");
                }
                Err(RecvError::Closed) => break,
            }
        }
    };

    (StatusCode::OK, response_headers, Body::from_stream(body)).into_response()
}

/// Keeps a station's listener count honest across disconnects.
struct ListenerGuard(Arc<AtomicUsize>);

impl ListenerGuard {
    fn attach(counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self(counter)
    }
}

impl Drop for ListenerGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Splices ICY metadata into an audio stream.
///
/// The audio is cut every `metaint` bytes and a metadata block inserted: one
/// length byte, then that many 16-byte units of text. A block is only worth
/// sending when the title has changed, so the usual case is the single zero
/// byte that says "still the same".
///
/// Each listener has its own writer because each one started at a different
/// point in the stream, and the count that matters is bytes since *this*
/// listener connected.
struct IcyWriter {
    metaint: Option<usize>,
    until_metadata: usize,
    last_title: Option<String>,
}

impl IcyWriter {
    fn new(metaint: Option<usize>) -> Self {
        Self {
            metaint,
            until_metadata: metaint.unwrap_or(0),
            last_title: None,
        }
    }

    fn push(&mut self, mut audio: &[u8], title: &str, out: &mut BytesMut) {
        let Some(metaint) = self.metaint else {
            out.put_slice(audio);
            return;
        };

        while !audio.is_empty() {
            if self.until_metadata == 0 {
                self.write_metadata(title, out);
                self.until_metadata = metaint;
            }
            let take = audio.len().min(self.until_metadata);
            out.put_slice(&audio[..take]);
            audio = &audio[take..];
            self.until_metadata -= take;
        }
    }

    fn write_metadata(&mut self, title: &str, out: &mut BytesMut) {
        if self.last_title.as_deref() == Some(title) {
            // Nothing new to say.
            out.put_u8(0);
            return;
        }
        self.last_title = Some(title.to_owned());

        let payload = format!("StreamTitle='{}';", sanitise_title(title)).into_bytes();
        // The length byte counts 16-byte units, so the payload is padded up.
        let units = payload.len().div_ceil(16).min(255);
        let padded = units * 16;
        out.put_u8(units as u8);
        if payload.len() <= padded {
            out.put_slice(&payload);
            out.put_bytes(0, padded - payload.len());
        } else {
            out.put_slice(&payload[..padded]);
        }
    }
}

/// A title safe to put inside `StreamTitle='…';`.
fn sanitise_title(title: &str) -> String {
    title
        .chars()
        .filter(|character| !character.is_control() && *character != '\'')
        .take(200)
        .collect()
}

/// A header value for text an operator typed.
///
/// Control characters would end the header early and let a station name write
/// headers of its own, so they are dropped. Everything else is kept, including
/// non-ASCII: an ICY name is read as UTF-8 by the players that show it.
fn icy_header(text: &str) -> HeaderValue {
    let cleaned: String = text
        .chars()
        .filter(|character| !character.is_control())
        .take(200)
        .collect();
    HeaderValue::from_bytes(cleaned.as_bytes()).unwrap_or(HeaderValue::from_static("VuIO Radio"))
}

// ---------------------------------------------------------------------------
// Management: running the stations
// ---------------------------------------------------------------------------

/// A station as the studio sees it: its settings, and what it is doing.
#[derive(Debug, Serialize)]
pub struct AdminStation {
    pub id: i64,
    pub name: String,
    pub genre: String,
    pub folders: Vec<String>,
    pub mode: BroadcastMode,
    /// Whether the operator wants this station on the air. Survives a restart.
    pub enabled: bool,
    /// Whether it actually is, right now.
    pub is_live: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    pub listeners: usize,
    pub uptime_secs: u64,
    pub queue_len: usize,
    /// Files in those folders that cannot be broadcast without re-encoding.
    pub skipped_files: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub now_playing: Option<NowPlayingView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NowPlayingView {
    pub title: String,
    pub artist: Option<String>,
    pub path: Option<String>,
    pub started_at_epoch_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct StationInput {
    pub name: Option<String>,
    pub genre: Option<String>,
    pub folders: Option<Vec<String>>,
    pub mode: Option<BroadcastMode>,
}

impl StationInput {
    /// Fill in what the operator left out, and refuse what cannot work.
    fn into_record(self, existing: Option<&RadioStation>) -> Result<RadioStationInput, AppError> {
        let name = self
            .name
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .or_else(|| existing.map(|station| station.name.clone()))
            .unwrap_or_else(|| "VuIO Radio".to_owned());

        let genre = self
            .genre
            .map(|genre| genre.trim().to_owned())
            .or_else(|| existing.map(|station| station.genre.clone()))
            .unwrap_or_else(|| "Variety".to_owned());

        let folders: Vec<String> = self
            .folders
            .or_else(|| existing.map(|station| station.folders.clone()))
            .unwrap_or_default()
            .into_iter()
            .map(|folder| folder.trim().to_owned())
            .filter(|folder| !folder.is_empty())
            .collect();

        if folders.is_empty() {
            return Err(AppError::InvalidInput(
                "A station needs at least one folder to play from".to_owned(),
            ));
        }

        Ok(RadioStationInput {
            name,
            genre,
            folders,
            mode: self
                .mode
                .or_else(|| existing.map(|station| station.mode))
                .unwrap_or_default(),
        })
    }
}

/// Every station, whether or not it is on the air.
pub async fn list_stations<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
) -> Result<Json<Vec<AdminStation>>, AppError> {
    let stations = state
        .database
        .list_radio_stations()
        .await
        .map_err(AppError::Internal)?;

    let mut views = Vec::with_capacity(stations.len());
    for station in stations {
        views.push(admin_view(&state, station).await);
    }
    Ok(Json(views))
}

async fn admin_view<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    station: RadioStation,
) -> AdminStation {
    let live = state.radio.get(station.id).await;
    let snapshot = live.as_ref().map(|running| running.snapshot());
    let origin = state.advertised_http_origin();

    AdminStation {
        id: station.id,
        name: station.name,
        genre: station.genre,
        folders: station.folders,
        mode: station.mode,
        enabled: station.enabled,
        is_live: snapshot.is_some(),
        codec: snapshot
            .as_ref()
            .map(|snapshot| snapshot.codec.as_str().to_owned()),
        listeners: snapshot.as_ref().map_or(0, |snapshot| snapshot.listeners),
        uptime_secs: snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.uptime.as_secs()),
        queue_len: snapshot.as_ref().map_or(0, |snapshot| snapshot.queue_len),
        skipped_files: snapshot
            .as_ref()
            .map_or(0, |snapshot| snapshot.skipped_files),
        now_playing: snapshot.as_ref().and_then(|snapshot| {
            let playing = &snapshot.now_playing;
            (!playing.title.is_empty()).then(|| NowPlayingView {
                title: playing.title.clone(),
                artist: playing.artist.clone(),
                path: playing.path.clone(),
                started_at_epoch_secs: playing.started_at_epoch_secs,
            })
        }),
        stream_url: Some(snapshot.as_ref().map_or_else(
            || format!("{origin}/api/radio/stations/{}/stream", station.id),
            |snapshot| format!("{origin}{}", stream_path(snapshot.id, snapshot.codec)),
        )),
    }
}

pub async fn create_station<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    Json(input): Json<StationInput>,
) -> Result<Json<AdminStation>, AppError> {
    let record = input.into_record(None)?;
    let station = state
        .database
        .create_radio_station(&record)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(admin_view(&state, station).await))
}

pub async fn update_station<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    Path(id): Path<i64>,
    Json(input): Json<StationInput>,
) -> Result<Json<AdminStation>, AppError> {
    let existing = load(&state, id).await?;
    let record = input.into_record(Some(&existing))?;
    let updated = state
        .database
        .update_radio_station(id, &record)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)?;

    // A station that is on the air is playing a queue built from the settings
    // that just changed, so it is rebuilt to match what the operator now sees.
    if state.radio.is_live(id).await {
        if let Err(error) = state.radio.start(&state, &updated).await {
            state.radio.stop(id).await;
            return Err(AppError::InvalidInput(format!(
                "The station was saved but could not restart: {error:#}"
            )));
        }
    }
    Ok(Json(admin_view(&state, updated).await))
}

pub async fn start_station<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    Path(id): Path<i64>,
) -> Result<Json<AdminStation>, AppError> {
    let station = load(&state, id).await?;
    state
        .radio
        .start(&state, &station)
        .await
        // The usual cause is folders holding nothing broadcastable, which is
        // the operator's to fix and so belongs in the response.
        .map_err(|error| AppError::InvalidInput(format!("{error:#}")))?;

    state
        .database
        .set_radio_station_enabled(id, true)
        .await
        .map_err(AppError::Internal)?;

    let station = load(&state, id).await?;
    Ok(Json(admin_view(&state, station).await))
}

pub async fn stop_station<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    Path(id): Path<i64>,
) -> Result<Json<AdminStation>, AppError> {
    state.radio.stop(id).await;
    state
        .database
        .set_radio_station_enabled(id, false)
        .await
        .map_err(AppError::Internal)?;
    let station = load(&state, id).await?;
    Ok(Json(admin_view(&state, station).await))
}

pub async fn skip_track<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    Path(id): Path<i64>,
) -> Result<Json<AdminStation>, AppError> {
    if !state.radio.skip(id).await {
        return Err(AppError::InvalidInput(
            "That station is not on the air".to_owned(),
        ));
    }
    let station = load(&state, id).await?;
    Ok(Json(admin_view(&state, station).await))
}

pub async fn delete_station<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.radio.stop(id).await;
    let deleted = state
        .database
        .delete_radio_station(id)
        .await
        .map_err(AppError::Internal)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

async fn load<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    id: i64,
) -> Result<RadioStation, AppError> {
    state
        .database
        .get_radio_station(id)
        .await
        .map_err(AppError::Internal)?
        .ok_or(AppError::NotFound)
}

// ---------------------------------------------------------------------------
// Management: the neighbours
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PeerQuery {
    /// Skip the mDNS browse and report only this server's own stations.
    #[serde(default)]
    pub local_only: bool,
}

/// Every live station on the network, this server's own included.
///
/// This is what the "Local Radio Stations" tab lists. Discovery is cached for a
/// few seconds, so a tab polling it does not re-browse the network each time.
pub async fn list_peers<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    Query(query): Query<PeerQuery>,
) -> Json<Vec<PeerServer>> {
    let config = state.current_config();
    let Json(own) = list_public_stations(State(state.clone())).await;

    let mut servers = Vec::new();
    if !own.is_empty() {
        servers.push(PeerServer {
            uuid: config.server.uuid.clone(),
            name: config.server.name.clone(),
            address: state
                .advertised_http_origin()
                .trim_start_matches("http://")
                .to_owned(),
            is_self: true,
            stations: own,
        });
    }

    if !query.local_only {
        servers.extend(state.radio.peers(&config.server.uuid).await);
    }
    Json(servers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(writer: &mut IcyWriter, audio: &[u8], title: &str) -> Vec<u8> {
        let mut out = BytesMut::new();
        writer.push(audio, title, &mut out);
        out.to_vec()
    }

    #[test]
    fn without_metadata_the_audio_is_untouched() {
        let mut writer = IcyWriter::new(None);
        let audio = vec![0xAB; 40_000];
        assert_eq!(drain(&mut writer, &audio, "anything"), audio);
    }

    #[test]
    fn a_metadata_block_lands_every_metaint_bytes() {
        let mut writer = IcyWriter::new(Some(16));
        let out = drain(&mut writer, &[0x01; 48], "Artist - Song");

        // 16 audio bytes, a metadata block, 16 more, a "nothing changed" byte,
        // 16 more, and another empty block.
        assert_eq!(&out[..16], &[0x01; 16]);
        let units = out[16] as usize;
        assert!(units > 0, "the first block must carry the title");
        let payload = &out[17..17 + units * 16];
        let text = String::from_utf8_lossy(payload);
        assert!(text.starts_with("StreamTitle='Artist - Song';"), "{text}");
        assert!(
            text[text.find(';').unwrap() + 1..]
                .bytes()
                .all(|byte| byte == 0),
            "the block must be padded with zeroes"
        );

        let after_first = 17 + units * 16;
        assert_eq!(&out[after_first..after_first + 16], &[0x01; 16]);
        assert_eq!(
            out[after_first + 16],
            0,
            "an unchanged title costs one zero byte"
        );
    }

    #[test]
    fn a_new_title_reaches_a_listener_already_connected() {
        let mut writer = IcyWriter::new(Some(16));
        let _ = drain(&mut writer, &[0x01; 16], "First Song");
        let out = drain(&mut writer, &[0x01; 16], "Second Song");

        let units = out[0] as usize;
        assert!(units > 0, "a changed title must be sent, not skipped");
        let text = String::from_utf8_lossy(&out[1..1 + units * 16]);
        assert!(text.starts_with("StreamTitle='Second Song';"), "{text}");
    }

    #[test]
    fn audio_is_never_lost_to_the_interleaver() {
        let mut writer = IcyWriter::new(Some(100));
        let audio: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
        let out = drain(&mut writer, &audio, "Song");

        // Strip the blocks back out and the original audio must remain.
        let mut recovered = Vec::new();
        let mut cursor = 0;
        while cursor < out.len() {
            let take = (out.len() - cursor).min(100);
            recovered.extend_from_slice(&out[cursor..cursor + take]);
            cursor += take;
            if cursor < out.len() {
                let units = out[cursor] as usize;
                cursor += 1 + units * 16;
            }
        }
        assert_eq!(recovered, audio);
    }

    #[test]
    fn a_title_cannot_break_out_of_its_own_field() {
        let hostile = "Bad'; DROP TABLE\nStreamTitle='pwned";
        let cleaned = sanitise_title(hostile);
        assert!(!cleaned.contains('\''));
        assert!(!cleaned.contains('\n'));
    }

    #[test]
    fn a_station_name_cannot_write_headers() {
        let value = icy_header("Evil\r\nX-Injected: yes");
        assert_eq!(value.to_str().unwrap(), "EvilX-Injected: yes");
    }

    #[test]
    fn a_non_ascii_station_name_survives() {
        let value = icy_header("Радио VuIO");
        assert_eq!(value.as_bytes(), "Радио VuIO".as_bytes());
    }

    #[test]
    fn a_station_needs_somewhere_to_play_from() {
        let input = StationInput {
            name: Some("Test".to_owned()),
            genre: None,
            folders: Some(Vec::new()),
            mode: None,
        };
        assert!(matches!(
            input.into_record(None),
            Err(AppError::InvalidInput(_))
        ));
    }

    #[test]
    fn omitted_fields_fall_back_to_sensible_defaults() {
        let input = StationInput {
            name: None,
            genre: None,
            folders: Some(vec!["/music".to_owned(), "  ".to_owned()]),
            mode: None,
        };
        let record = input.into_record(None).expect("a usable station");
        assert_eq!(record.name, "VuIO Radio");
        assert_eq!(record.genre, "Variety");
        assert_eq!(record.folders, vec!["/music".to_owned()]);
        assert_eq!(record.mode, BroadcastMode::Shuffle);
    }

    #[test]
    fn a_stream_path_names_the_codec_it_carries() {
        assert_eq!(
            stream_path(7, crate::radio::frames::Codec::Mp3),
            "/api/radio/stations/7/stream.mp3"
        );
        assert_eq!(
            stream_path(7, crate::radio::frames::Codec::Aac),
            "/api/radio/stations/7/stream.aac"
        );
    }
}

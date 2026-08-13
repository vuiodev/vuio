//! ICY Radio Broadcast Engine & Control Handlers
//!
//! Provides a SHOUTcast / Icecast compatible live HTTP audio broadcasting stream
//! with interleaved ICY `StreamTitle` metadata blocks, SQLite database persistence,
//! and accurate listener tracking.

use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use bytes::{BufMut, Bytes, BytesMut};
use futures_util::stream::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    path::PathBuf,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll},
};
use tokio::sync::RwLock;
use tokio_util::io::ReaderStream;

use crate::{
    database::{DatabaseManager, DatabaseReadSession, MediaFileQuery, MediaFileView},
    error::AppError,
    state::AppState,
};

pub const RADIO_BROADCAST_SECRET_KEY: &str = "radio_broadcast_state";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BroadcastMode {
    Linear,
    Shuffle,
    Loop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioBroadcastPersistedState {
    pub station_name: String,
    pub station_genre: String,
    pub mode: BroadcastMode,
    pub selected_folders: Vec<String>,
    pub is_broadcasting: bool,
    pub current_title: Option<String>,
    pub current_artist: Option<String>,
    pub current_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastStatus {
    pub station_name: String,
    pub station_genre: String,
    pub is_broadcasting: bool,
    pub mode: BroadcastMode,
    pub selected_folders: Vec<String>,
    pub current_title: Option<String>,
    pub current_artist: Option<String>,
    pub current_path: Option<String>,
    pub listeners_count: usize,
    pub icy_metadata: String,
    pub icy_metaint: usize,
    pub elapsed_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BroadcastControlInput {
    pub action: String, // "start", "stop", "next", "prev", "set_mode", "set_folders", "set_track"
    pub mode: Option<BroadcastMode>,
    pub folders: Option<Vec<String>>,
    pub station_name: Option<String>,
    pub station_genre: Option<String>,
    pub track_id: Option<i64>,
    pub track_path: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
}

pub struct RadioBroadcastState {
    pub station_name: RwLock<String>,
    pub station_genre: RwLock<String>,
    pub mode: RwLock<BroadcastMode>,
    pub selected_folders: RwLock<Vec<String>>,
    pub is_broadcasting: RwLock<bool>,
    pub current_title: RwLock<Option<String>>,
    pub current_artist: RwLock<Option<String>>,
    pub current_path: RwLock<Option<PathBuf>>,
    pub listeners_count: Arc<AtomicUsize>,
    pub elapsed_secs: std::sync::atomic::AtomicU64,
}

impl Default for RadioBroadcastState {
    fn default() -> Self {
        Self {
            station_name: RwLock::new("VuIO Live FM Broadcast".to_string()),
            station_genre: RwLock::new("Variety / Live Radio".to_string()),
            mode: RwLock::new(BroadcastMode::Shuffle),
            selected_folders: RwLock::new(Vec::new()),
            is_broadcasting: RwLock::new(false),
            current_title: RwLock::new(None),
            current_artist: RwLock::new(None),
            current_path: RwLock::new(None),
            listeners_count: Arc::new(AtomicUsize::new(0)),
            elapsed_secs: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

impl RadioBroadcastState {
    pub async fn load_from_database<D: DatabaseManager>(database: &D) -> Self {
        let state = Self::default();
        if let Ok(Some(bytes)) = database.get_secret(RADIO_BROADCAST_SECRET_KEY).await {
            if let Ok(persisted) = serde_json::from_slice::<RadioBroadcastPersistedState>(&bytes) {
                *state.station_name.write().await = persisted.station_name;
                *state.station_genre.write().await = persisted.station_genre;
                *state.mode.write().await = persisted.mode;
                *state.selected_folders.write().await = persisted.selected_folders;
                *state.is_broadcasting.write().await = persisted.is_broadcasting;
                *state.current_title.write().await = persisted.current_title;
                *state.current_artist.write().await = persisted.current_artist;
                *state.current_path.write().await = persisted.current_path;
            }
        }
        state
    }

    pub async fn save_to_database<D: DatabaseManager>(&self, database: &D) {
        let persisted = RadioBroadcastPersistedState {
            station_name: self.station_name.read().await.clone(),
            station_genre: self.station_genre.read().await.clone(),
            mode: *self.mode.read().await,
            selected_folders: self.selected_folders.read().await.clone(),
            is_broadcasting: *self.is_broadcasting.read().await,
            current_title: self.current_title.read().await.clone(),
            current_artist: self.current_artist.read().await.clone(),
            current_path: self.current_path.read().await.clone(),
        };

        if let Ok(bytes) = serde_json::to_vec(&persisted) {
            let _ = database.set_secret(RADIO_BROADCAST_SECRET_KEY, &bytes).await;
        }
    }
}

struct ListenerGuard {
    counter: Arc<AtomicUsize>,
}

impl Drop for ListenerGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

pub async fn get_broadcast_status<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
) -> Result<Json<BroadcastStatus>, AppError> {
    let broadcast = &state.radio_broadcast;
    let station_name = broadcast.station_name.read().await.clone();
    let station_genre = broadcast.station_genre.read().await.clone();
    let is_broadcasting = *broadcast.is_broadcasting.read().await;
    let mode = *broadcast.mode.read().await;
    let selected_folders = broadcast.selected_folders.read().await.clone();
    let current_title = broadcast.current_title.read().await.clone();
    let current_artist = broadcast.current_artist.read().await.clone();
    let current_path = broadcast
        .current_path
        .read()
        .await
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());

    let raw_listeners = broadcast.listeners_count.load(Ordering::Relaxed);
    let listeners_count = if is_broadcasting { raw_listeners } else { 0 };
    let elapsed_secs = broadcast.elapsed_secs.load(Ordering::Relaxed);

    let artist = current_artist.as_deref().unwrap_or("Unknown Artist");
    let title = current_title.as_deref().unwrap_or("VuIO Station Intermission");
    let icy_metadata = format!("StreamTitle='{artist} - {title}';StreamUrl='';");

    Ok(Json(BroadcastStatus {
        station_name,
        station_genre,
        is_broadcasting,
        mode,
        selected_folders,
        current_title,
        current_artist,
        current_path,
        listeners_count,
        icy_metadata,
        icy_metaint: 16000,
        elapsed_secs,
    }))
}

pub async fn post_broadcast_control<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    Json(input): Json<BroadcastControlInput>,
) -> Result<Json<BroadcastStatus>, AppError> {
    let broadcast = &state.radio_broadcast;

    if let Some(name) = input.station_name {
        *broadcast.station_name.write().await = name;
    }
    if let Some(genre) = input.station_genre {
        *broadcast.station_genre.write().await = genre;
    }
    if let Some(mode) = input.mode {
        *broadcast.mode.write().await = mode;
    }
    if let Some(folders) = input.folders {
        *broadcast.selected_folders.write().await = folders;
    }
    if let Some(t) = input.title {
        *broadcast.current_title.write().await = Some(t);
    }
    if let Some(a) = input.artist {
        *broadcast.current_artist.write().await = Some(a);
    }
    if let Some(path_str) = input.track_path {
        *broadcast.current_path.write().await = Some(PathBuf::from(path_str));
    } else if let Some(track_id) = input.track_id {
        if let Ok(Some(file_info)) = state.database.get_file_location_by_id(track_id).await {
            *broadcast.current_path.write().await = Some(file_info.path);
        }
    }

    match input.action.as_str() {
        "start" | "set_track" => {
            *broadcast.is_broadcasting.write().await = true;
            if broadcast.current_path.read().await.is_none() {
                let state_clone = state.clone();
                let db_clone = state.database.clone();
                let b = state.radio_broadcast.clone();
                let _ = db_clone
                    .read(move |session| {
                        let query = MediaFileQuery::Filtered {
                            after_id: None,
                            mime_family: Some("audio/".to_string()),
                            text: None,
                        };
                        let _ = session.visit_files(&query, 0, 1, |file| {
                            let artist = file.artist().map(str::to_owned);
                            let title = file
                                .title()
                                .map(str::to_owned)
                                .unwrap_or_else(|| file.filename().to_owned());
                            let path = PathBuf::from(file.path());
                            let b_inner = b.clone();
                            let db_inner = state_clone.database.clone();
                            tokio::spawn(async move {
                                *b_inner.current_artist.write().await = artist;
                                *b_inner.current_title.write().await = Some(title);
                                *b_inner.current_path.write().await = Some(path);
                                b_inner.save_to_database(db_inner.as_ref()).await;
                            });
                            Ok(())
                        });
                        Ok(())
                    })
                    .await;
            }
        }
        "stop" => {
            *broadcast.is_broadcasting.write().await = false;
            *broadcast.current_title.write().await = None;
            *broadcast.current_artist.write().await = None;
            *broadcast.current_path.write().await = None;
            broadcast.listeners_count.store(0, Ordering::Relaxed);
        }
        _ => {}
    }

    // Persist latest state to database
    broadcast.save_to_database(state.database.as_ref()).await;

    get_broadcast_status(State(state)).await
}

/// Serve ICY audio live stream endpoint
pub async fn serve_icy_broadcast_stream<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    ConnectInfo(client_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let broadcast = &state.radio_broadcast;

    if !*broadcast.is_broadcasting.read().await {
        return Err(AppError::NotFound);
    }

    let station_name = broadcast.station_name.read().await.clone();
    let station_genre = broadcast.station_genre.read().await.clone();
    let current_artist = broadcast.current_artist.read().await.clone();
    let current_title = broadcast.current_title.read().await.clone();

    let client_requests_icy = headers
        .get("icy-metadata")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == "1")
        .unwrap_or(false);

    let is_self_preview = client_addr.ip().is_loopback()
        || headers
            .get("user-agent")
            .and_then(|ua| ua.to_str().ok())
            .is_some_and(|ua| ua.contains("VuIO-InternalAdmin"));

    let _guard = if !is_self_preview {
        broadcast.listeners_count.fetch_add(1, Ordering::Relaxed);
        Some(ListenerGuard {
            counter: broadcast.listeners_count.clone(),
        })
    } else {
        None
    };

    let current_path = broadcast.current_path.read().await.clone();
    let path = match current_path {
        Some(p) if p.exists() => p,
        _ => {
            let mut found_path: Option<PathBuf> = None;
            let (tx, rx) = tokio::sync::oneshot::channel();
            let state_clone = state.clone();
            tokio::spawn(async move {
                let _ = state_clone
                    .database
                    .read(move |session| {
                        let query = MediaFileQuery::Filtered {
                            after_id: None,
                            mime_family: Some("audio/".to_string()),
                            text: None,
                        };
                        let mut p = None;
                        let _ = session.visit_files(&query, 0, 1, |file| {
                            p = Some(PathBuf::from(file.path()));
                            Ok(())
                        });
                        let _ = tx.send(p);
                        Ok(())
                    })
                    .await;
            });
            if let Ok(Some(p)) = rx.await {
                found_path = Some(p);
            }
            found_path.ok_or(AppError::NotFound)?
        }
    };

    let file = tokio::fs::File::open(&path).await.map_err(|e| {
        tracing::error!("Failed to open broadcast stream file {:?}: {}", path, e);
        AppError::NotFound
    })?;

    let mime_type = match path.extension().and_then(|ext| ext.to_str()).map(str::to_lowercase).as_deref() {
        Some("flac") => "audio/flac",
        Some("wav") => "audio/wav",
        Some("ogg") => "audio/ogg",
        Some("aac") => "audio/aac",
        Some("m4a") => "audio/mp4",
        _ => "audio/mpeg",
    };

    let mut response_headers = HeaderMap::new();
    response_headers.insert(header::CONTENT_TYPE, mime_type.parse().unwrap());
    response_headers.insert("icy-name", station_name.parse().unwrap());
    response_headers.insert("icy-genre", station_genre.parse().unwrap());
    response_headers.insert("icy-br", "320".parse().unwrap());
    response_headers.insert("icy-pub", "1".parse().unwrap());

    let reader_stream = ReaderStream::new(file);

    if client_requests_icy {
        response_headers.insert("icy-metaint", "16000".parse().unwrap());
        let artist = current_artist.as_deref().unwrap_or("Unknown Artist");
        let title = current_title.as_deref().unwrap_or("VuIO Station Intermission");
        let icy_meta_str = format!("StreamTitle='{artist} - {title}';");

        let icy_stream = IcyStreamAdapter::new(reader_stream, 16000, icy_meta_str, _guard);
        Ok((StatusCode::OK, response_headers, Body::from_stream(icy_stream)).into_response())
    } else {
        let guarded_stream = GuardedStream::new(reader_stream, _guard);
        Ok((StatusCode::OK, response_headers, Body::from_stream(guarded_stream)).into_response())
    }
}

struct GuardedStream<S> {
    inner: S,
    _guard: Option<ListenerGuard>,
}

impl<S> GuardedStream<S> {
    fn new(inner: S, guard: Option<ListenerGuard>) -> Self {
        Self { inner, _guard: guard }
    }
}

impl<S> Stream for GuardedStream<S>
where
    S: Stream<Item = Result<Bytes, std::io::Error>> + Unpin,
{
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.poll_next_unpin(cx)
    }
}

/// Stream adapter that interleaves ICY metadata block every `metaint` bytes
struct IcyStreamAdapter<S> {
    inner: S,
    metaint: usize,
    bytes_until_meta: usize,
    metadata_bytes: Bytes,
    _guard: Option<ListenerGuard>,
}

impl<S> IcyStreamAdapter<S> {
    fn new(inner: S, metaint: usize, metadata_str: String, guard: Option<ListenerGuard>) -> Self {
        let meta_payload = metadata_str.into_bytes();
        let raw_len = meta_payload.len();
        let num_blocks = raw_len.div_ceil(16);
        let padded_len = num_blocks * 16;

        let mut meta_buf = BytesMut::with_capacity(1 + padded_len);
        meta_buf.put_u8(num_blocks as u8);
        meta_buf.put_slice(&meta_payload);
        for _ in raw_len..padded_len {
            meta_buf.put_u8(0);
        }

        Self {
            inner,
            metaint,
            bytes_until_meta: metaint,
            metadata_bytes: meta_buf.freeze(),
            _guard: guard,
        }
    }
}

impl<S> Stream for IcyStreamAdapter<S>
where
    S: Stream<Item = Result<Bytes, std::io::Error>> + Unpin,
{
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.poll_next_unpin(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                let mut out = BytesMut::new();
                let mut chunk_slice = &chunk[..];

                while !chunk_slice.is_empty() {
                    if self.bytes_until_meta == 0 {
                        out.put_slice(&self.metadata_bytes);
                        self.bytes_until_meta = self.metaint;
                    }

                    let take = chunk_slice.len().min(self.bytes_until_meta);
                    out.put_slice(&chunk_slice[..take]);
                    chunk_slice = &chunk_slice[take..];
                    self.bytes_until_meta -= take;
                }

                Poll::Ready(Some(Ok(out.freeze())))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

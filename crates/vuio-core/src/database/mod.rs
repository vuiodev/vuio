use anyhow::Result;
use async_trait::async_trait;
use futures_util::Stream;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{Duration, SystemTime};

#[cfg(test)]
pub mod conformance;
pub mod playlist_formats;
pub mod sqlite;

/// The storage backend the server runs on.
///
/// Backend selection is a compile-time decision made here and nowhere else.
/// Every consumer is generic over `D: DatabaseManager`, so this alias is the
/// only edit needed to swap the engine underneath the whole application.
pub type ActiveDatabase = sqlite::SqliteDatabase;

/// Represents a subdirectory in the media library.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MediaDirectory {
    pub path: PathBuf,
    pub name: String,
}

/// Result of removing one file or a complete directory subtree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RemovalSummary {
    pub removed_files: usize,
    pub affected_parents: Vec<PathBuf>,
    pub mime_families: HashSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct RootAvailability {
    pub path: PathBuf,
    pub last_seen_secs: u64,
    pub unavailable_since_secs: Option<u64>,
    pub indexed_count: u64,
    pub reason: String,
}

/// Represents a playlist
#[derive(Clone, Debug)]
pub struct Playlist {
    pub id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

/// Music categorization container for organizing music content
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MusicCategory {
    pub id: String,
    pub name: String,
    pub category_type: MusicCategoryType,
    /// How many records carry this value.
    pub count: usize,
    /// How many distinct sub-categories it contains, when one was asked for.
    ///
    /// A container whose children are containers cannot report `count` as its
    /// `childCount` — an artist with forty tracks across three albums has three
    /// children, not forty.
    pub child_count: Option<usize>,
    /// One record belonging to this category, used to point a container's
    /// `upnp:albumArtURI` at cover art without a second query.
    pub sample_id: Option<i64>,
}

/// Which records a category listing is drawn from.
///
/// Every field is an `AND`, which is what lets one query serve a flat list of
/// artists and the albums of one artist within one genre alike.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MusicCategoryFilter {
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
}

impl MusicCategoryFilter {
    pub fn artist(artist: impl Into<String>) -> Self {
        Self {
            artist: Some(artist.into()),
            ..Self::default()
        }
    }

    pub fn album_artist(album_artist: impl Into<String>) -> Self {
        Self {
            album_artist: Some(album_artist.into()),
            ..Self::default()
        }
    }

    pub fn genre(genre: impl Into<String>) -> Self {
        Self {
            genre: Some(genre.into()),
            ..Self::default()
        }
    }

    pub fn with_artist(mut self, artist: impl Into<String>) -> Self {
        self.artist = Some(artist.into());
        self
    }
}

/// Types of music categorization
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MusicCategoryType {
    Artist,
    Album,
    Genre,
    AlbumArtist,
    Year,
    Playlist,
}

/// Tag fields promoted to columns of their own.
///
/// These are the ones browsing, sorting or DIDL read directly. Everything else
/// a tag reader finds is kept verbatim in `extra_tags`, so learning to use a
/// new tag later is a query rather than a migration.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AudioTags {
    pub disc_number: Option<u32>,
    pub disc_total: Option<u32>,
    pub track_total: Option<u32>,
    pub composer: Option<String>,
    pub comment: Option<String>,
    pub bpm: Option<u32>,
    pub compilation: Option<bool>,
    pub sort_title: Option<String>,
    pub sort_artist: Option<String>,
    pub sort_album: Option<String>,
    /// The full release date string. `MediaFile::year` keeps the integer the
    /// Years category groups on.
    pub release_date: Option<String>,
    pub musicbrainz_track_id: Option<String>,
    pub musicbrainz_album_id: Option<String>,
    pub musicbrainz_artist_id: Option<String>,
}

/// Stream properties read off the container while its tags are parsed.
///
/// DLNA renderers use these as `res` attributes to decide whether they can play
/// a track before fetching it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StreamInfo {
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub bits_per_sample: Option<u16>,
    /// Average bits per second. DLNA's `res@bitrate` wants *bytes* per second,
    /// so the renderer layer divides by eight.
    pub bit_rate: Option<u32>,
}

/// Enhanced MediaFile structure for database storage
#[derive(Clone, Debug)]
pub struct MediaFile {
    pub id: Option<i64>,
    pub path: PathBuf,
    pub filename: String,
    pub size: u64,
    pub modified: SystemTime,
    pub mime_type: String,
    pub duration: Option<Duration>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub track_number: Option<u32>,
    pub year: Option<u32>,
    pub album_artist: Option<String>,
    pub tags: AudioTags,
    pub stream: StreamInfo,
    /// Every other tag the reader found, as (normalized key, value).
    ///
    /// Write-only: the scanner fills it and the writer persists it to
    /// `media_tags`. Reads that only need to render a browse response leave it
    /// empty rather than pay for the join.
    pub extra_tags: Vec<(String, String)>,
    /// Which version of the tag reader wrote this record. Drives re-reads when
    /// the reader improves; see `platform::filesystem::TAGS_VERSION`.
    pub tags_version: u32,
    pub subtitle_available: bool,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

/// Explicit name for a complete record that must outlive a database read session.
pub type OwnedMediaFile = MediaFile;

/// Minimal persisted state needed to compare a filesystem scan with the index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileFingerprint {
    pub id: i64,
    pub path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
    pub created_at: SystemTime,
    /// Which tag reader wrote the record. A record whose file is unchanged is
    /// still re-read when this trails the current reader.
    pub tags_version: u32,
}

/// Minimal owned state needed after a database session to serve one resource.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileLocation {
    pub id: i64,
    pub path: PathBuf,
    pub filename: String,
    pub title: Option<String>,
    pub mime_type: String,
    pub size: u64,
    pub subtitle_available: bool,
}

/// Small owned copy of fields required after a write invalidates an archived value guard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexSnapshot {
    pub id: i64,
    pub path: String,
    pub size: u64,
    pub mime_type: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub album_artist: Option<String>,
}

impl IndexSnapshot {
    pub fn from_view(view: &impl MediaFileView) -> Option<Self> {
        Some(Self {
            id: view.id()?,
            path: view.path().to_owned(),
            size: view.size(),
            mime_type: view.mime_type().to_owned(),
            artist: view.artist().map(str::to_owned),
            album: view.album().map(str::to_owned),
            genre: view.genre().map(str::to_owned),
            year: view.year(),
            album_artist: view.album_artist().map(str::to_owned),
        })
    }
}

impl MediaFileView for IndexSnapshot {
    fn id(&self) -> Option<i64> {
        Some(self.id)
    }
    fn path(&self) -> &str {
        &self.path
    }
    fn filename(&self) -> &str {
        ""
    }
    fn size(&self) -> u64 {
        self.size
    }
    fn modified_secs(&self) -> u64 {
        0
    }
    fn mime_type(&self) -> &str {
        &self.mime_type
    }
    fn duration_secs(&self) -> Option<f64> {
        None
    }
    fn title(&self) -> Option<&str> {
        None
    }
    fn artist(&self) -> Option<&str> {
        self.artist.as_deref()
    }
    fn album(&self) -> Option<&str> {
        self.album.as_deref()
    }
    fn genre(&self) -> Option<&str> {
        self.genre.as_deref()
    }
    fn track_number(&self) -> Option<u32> {
        None
    }
    fn year(&self) -> Option<u32> {
        self.year
    }
    fn album_artist(&self) -> Option<&str> {
        self.album_artist.as_deref()
    }
    fn subtitle_available(&self) -> bool {
        false
    }
    fn created_at_secs(&self) -> u64 {
        0
    }
    fn updated_at_secs(&self) -> u64 {
        0
    }
}

impl MediaFile {
    pub fn new(path: PathBuf, size: u64, mime_type: String) -> Self {
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let now = SystemTime::now();

        Self {
            id: None,
            path,
            filename,
            size,
            modified: now,
            mime_type,
            duration: None,
            title: None,
            artist: None,
            album: None,
            genre: None,
            track_number: None,
            year: None,
            album_artist: None,
            tags: AudioTags::default(),
            stream: StreamInfo::default(),
            extra_tags: Vec::new(),
            tags_version: 0,
            subtitle_available: false,
            created_at: now,
            updated_at: now,
        }
    }
}

impl MediaFileView for MediaFile {
    fn id(&self) -> Option<i64> {
        self.id
    }
    fn path(&self) -> &str {
        self.path.to_str().unwrap_or_default()
    }
    fn filename(&self) -> &str {
        &self.filename
    }
    fn size(&self) -> u64 {
        self.size
    }
    fn modified_secs(&self) -> u64 {
        self.modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
    fn mime_type(&self) -> &str {
        &self.mime_type
    }
    fn duration_secs(&self) -> Option<f64> {
        self.duration.map(|value| value.as_secs_f64())
    }
    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
    fn artist(&self) -> Option<&str> {
        self.artist.as_deref()
    }
    fn album(&self) -> Option<&str> {
        self.album.as_deref()
    }
    fn genre(&self) -> Option<&str> {
        self.genre.as_deref()
    }
    fn track_number(&self) -> Option<u32> {
        self.track_number
    }
    fn year(&self) -> Option<u32> {
        self.year
    }
    fn album_artist(&self) -> Option<&str> {
        self.album_artist.as_deref()
    }
    fn tags_version(&self) -> u32 {
        self.tags_version
    }
    fn disc_number(&self) -> Option<u32> {
        self.tags.disc_number
    }
    fn composer(&self) -> Option<&str> {
        self.tags.composer.as_deref()
    }
    fn codec(&self) -> Option<&str> {
        self.stream.codec.as_deref()
    }
    fn sample_rate(&self) -> Option<u32> {
        self.stream.sample_rate
    }
    fn channels(&self) -> Option<u16> {
        self.stream.channels
    }
    fn bits_per_sample(&self) -> Option<u16> {
        self.stream.bits_per_sample
    }
    fn bit_rate(&self) -> Option<u32> {
        self.stream.bit_rate
    }
    fn subtitle_available(&self) -> bool {
        self.subtitle_available
    }
    fn created_at_secs(&self) -> u64 {
        self.created_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
    fn updated_at_secs(&self) -> u64 {
        self.updated_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// Borrowed, backend-neutral view of one media record.
///
/// Implementations may point directly into a database page. Callers must not
/// retain any returned string beyond the scoped read-session callback.
pub trait MediaFileView {
    fn id(&self) -> Option<i64>;
    fn path(&self) -> &str;
    fn filename(&self) -> &str;
    fn size(&self) -> u64;
    fn modified_secs(&self) -> u64;
    fn mime_type(&self) -> &str;
    fn duration_secs(&self) -> Option<f64>;
    fn title(&self) -> Option<&str>;
    fn artist(&self) -> Option<&str>;
    fn album(&self) -> Option<&str>;
    fn genre(&self) -> Option<&str>;
    fn track_number(&self) -> Option<u32>;
    fn year(&self) -> Option<u32>;
    fn album_artist(&self) -> Option<&str>;
    fn subtitle_available(&self) -> bool;
    fn created_at_secs(&self) -> u64;
    fn updated_at_secs(&self) -> u64;

    /// Which version of the tag reader wrote this record.
    fn tags_version(&self) -> u32 {
        0
    }

    // The remaining tag and stream fields default to absent so a view that has
    // no use for them — the index snapshot, say — need not carry them.

    fn disc_number(&self) -> Option<u32> {
        None
    }
    fn composer(&self) -> Option<&str> {
        None
    }
    fn codec(&self) -> Option<&str> {
        None
    }
    fn sample_rate(&self) -> Option<u32> {
        None
    }
    fn channels(&self) -> Option<u16> {
        None
    }
    fn bits_per_sample(&self) -> Option<u16> {
        None
    }
    /// Average bits per second; `res@bitrate` is bytes per second.
    fn bit_rate(&self) -> Option<u32> {
        None
    }

    fn to_file_location(&self) -> Option<FileLocation> {
        Some(FileLocation {
            id: self.id()?,
            path: PathBuf::from(self.path()),
            filename: self.filename().to_owned(),
            title: self.title().map(str::to_owned),
            mime_type: self.mime_type().to_owned(),
            size: self.size(),
            subtitle_available: self.subtitle_available(),
        })
    }

}

pub trait PlaylistView {
    fn id(&self) -> Option<i64>;
    fn name(&self) -> &str;
    fn description(&self) -> Option<&str>;
    fn created_at_secs(&self) -> u64;
    fn updated_at_secs(&self) -> u64;
}

/// Borrowed directory record. A backend may implement it directly over a result row.
pub trait DirectoryView {
    fn id(&self) -> u64;
    fn path(&self) -> &str;
    fn name(&self) -> &str;
    /// How many matching files the directory's subtree holds, or 0 when the
    /// backing query cannot say. Recursive, because that is the number the
    /// maintained counters carry and the number a listing wants to show: a
    /// folder whose media all lives in grandchildren is not empty.
    fn file_count(&self) -> u64 {
        0
    }
}

impl DirectoryView for MediaDirectory {
    fn id(&self) -> u64 {
        0
    }

    fn path(&self) -> &str {
        self.path.to_str().unwrap_or_default()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Debug)]
pub enum MediaFileQuery {
    All,
    Id(i64),
    Path(String),
    Directory {
        path: String,
        mime_family: Option<String>,
    },
    Artist(String),
    Album {
        album: String,
        artist: Option<String>,
    },
    Genre(String),
    Year(u32),
    AlbumArtist(String),
    /// Tracks matching any combination of music tags.
    ///
    /// The single-tag variants above stay for the `get_music_by_*` API; this is
    /// what a nested browse tree needs, where an album is only meaningful
    /// alongside the artist or genre it was reached through.
    Music {
        artist: Option<String>,
        album_artist: Option<String>,
        album: Option<String>,
        genre: Option<String>,
        year: Option<u32>,
        /// Leave out internet-radio records, which are audio but not music.
        exclude_radio: bool,
    },
    Playlist(i64),
    /// Cursor-paged library scan. Filtering is performed against borrowed
    /// views inside the database read transaction, so rejected rows are never
    /// materialized as `MediaFile` values.
    Filtered {
        after_id: Option<i64>,
        mime_family: Option<String>,
        text: Option<String>,
    },
    /// Ranked full-text search across filenames, tags and fetched synopses.
    ///
    /// Separate from [`MediaFileQuery::Filtered`] because the two page
    /// differently and cannot share a cursor: `Filtered` scans in id order and
    /// resumes from `after_id`, while these results come back by relevance,
    /// where an id says nothing about position. Search pages by offset instead,
    /// which is affordable because a result set is bounded by how much actually
    /// matched rather than by the size of the library.
    Search {
        text: String,
        mime_family: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct VisitSummary {
    pub matched: usize,
    pub visited: usize,
}

/// Backend-owned read transaction with lending record views.
pub trait DatabaseReadSession {
    type File<'a>: MediaFileView
    where
        Self: 'a;
    type Playlist<'a>: PlaylistView
    where
        Self: 'a;
    type Directory<'a>: DirectoryView
    where
        Self: 'a;

    fn visit_files<F>(
        &mut self,
        query: &MediaFileQuery,
        offset: usize,
        limit: usize,
        visitor: F,
    ) -> Result<VisitSummary>
    where
        F: for<'a> FnMut(Self::File<'a>) -> Result<()>;

    /// One page, without counting the whole result.
    ///
    /// [`Self::visit_files`] reports `matched`, the size of the entire result,
    /// because DLNA's `TotalMatches` requires it — and computing it means
    /// evaluating the query a second time. For a ranked search that is the
    /// expensive half: the engine has to find and rank every hit to count them,
    /// so a page of twenty costs the same as a page of one.
    ///
    /// Callers that page by cursor never look at `matched`. Defaulted so a
    /// backend need not implement it; overriding it is what makes the saving
    /// real.
    fn visit_files_page<F>(
        &mut self,
        query: &MediaFileQuery,
        offset: usize,
        limit: usize,
        visitor: F,
    ) -> Result<usize>
    where
        F: for<'a> FnMut(Self::File<'a>) -> Result<()>,
    {
        self.visit_files(query, offset, limit, visitor)
            .map(|summary| summary.visited)
    }

    fn visit_direct_subdirectories<F>(
        &mut self,
        canonical_parent: &str,
        mime_family: Option<&str>,
        offset: usize,
        limit: usize,
        visitor: F,
    ) -> Result<VisitSummary>
    where
        F: for<'a> FnMut(Self::Directory<'a>) -> Result<()>;

    fn visit_playlists<F>(
        &mut self,
        offset: usize,
        limit: usize,
        visitor: F,
    ) -> Result<VisitSummary>
    where
        F: for<'a> FnMut(Self::Playlist<'a>) -> Result<()>;

    /// The fetched media info for `ids`, for rendering a browse page.
    ///
    /// Inside the session because DIDL is written from a blocking read and cannot
    /// await a lookup mid-row. Defaulted to empty so a backend that stores no
    /// media info still satisfies the trait.
    ///
    /// Rows below `min_confidence` are not returned. They are kept in the table so
    /// the operator can review them, but a guess that was not good enough to trust
    /// must not end up relabelling the library — showing "Dead on Arrival" for a
    /// film called Arrival is worse than showing the filename.
    fn mediainfo_overlays(
        &mut self,
        ids: &[i64],
        min_confidence: u8,
    ) -> Result<std::collections::HashMap<i64, MediaInfoOverlay>> {
        let _ = (ids, min_confidence);
        Ok(std::collections::HashMap::new())
    }
}

/// The few fetched fields a browse response actually renders.
///
/// Deliberately not [`MediaInfoRecord`]: that carries the provider's whole JSON
/// payload, and pulling one of those per row would dwarf the DIDL document it is
/// decorating.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaInfoOverlay {
    pub title: Option<String>,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    pub has_artwork: bool,
}

/// Media-library storage and query operations implemented by a database backend.
#[async_trait]
pub trait MediaRepository: Send + Sync {
    type ReadSession: DatabaseReadSession + Send + 'static;

    /// Execute a complete scoped read operation. Implementations choose their
    /// scheduling strategy; SQLite runs this closure on Tokio's blocking pool.
    async fn read<R, F>(self: std::sync::Arc<Self>, operation: F) -> Result<R>
    where
        Self: Sized + 'static,
        R: Send + 'static,
        F: FnOnce(&mut Self::ReadSession) -> Result<R> + Send + 'static;

    /// Store a new media file record
    async fn store_media_file(&self, file: &MediaFile) -> Result<i64>;

    /// Stream all media files from the database (memory efficient)
    ///
    /// This method provides a memory-efficient way to process all media files without
    /// loading them all into memory at once. Use this instead of `get_all_media_files()`
    /// for large media libraries.
    ///
    /// # Example
    /// ```rust,ignore
    /// use futures_util::StreamExt;
    ///
    /// let mut stream = database.stream_all_media_files();
    /// while let Some(result) = stream.next().await {
    ///     match result {
    ///         Ok(media_file) => {
    ///             // Process each file individually
    ///             println!("Processing: {}", media_file.filename);
    ///         }
    ///         Err(e) => {
    ///             eprintln!("Error reading file: {}", e);
    ///         }
    ///     }
    /// }
    /// ```
    fn stream_all_media_files(&self) -> Pin<Box<dyn Stream<Item = Result<MediaFile>> + Send + '_>>;

    /// Collect all media files from the stream (helper for tests)
    ///
    /// This is a convenience method that collects all files from the stream into a Vec.
    /// Use this only for testing or when you need all files at once.
    async fn collect_all_media_files(&self) -> Result<Vec<MediaFile>> {
        use futures_util::StreamExt;

        let mut stream = self.stream_all_media_files();
        let mut files = Vec::new();

        while let Some(result) = stream.next().await {
            files.push(result?);
        }

        Ok(files)
    }

    /// Remove a media file record by path
    async fn remove_media_file(&self, path: &Path) -> Result<bool>;

    /// Update an existing media file record
    async fn update_media_file(&self, file: &MediaFile) -> Result<()>;

    /// Get all files in a specific directory
    async fn get_files_in_directory(&self, dir: &Path) -> Result<Vec<MediaFile>>;

    /// Get directory listing (subdirectories and files) for a given path and media type
    async fn get_directory_listing(
        &self,
        parent_path: &Path,
        media_type_filter: &str,
    ) -> Result<(Vec<MediaDirectory>, Vec<MediaFile>)>;

    /// Remove media files that no longer exist on disk
    async fn cleanup_missing_files(&self, existing_paths: &[PathBuf]) -> Result<usize>;

    /// Get a specific file by path
    async fn get_file_by_path(&self, path: &Path) -> Result<Option<MediaFile>>;

    /// Get a specific file by ID
    async fn get_file_by_id(&self, id: i64) -> Result<Option<MediaFile>>;

    /// Load only the fields required by media streaming.
    async fn get_file_location_by_id(&self, id: i64) -> Result<Option<FileLocation>>;

    /// Load compact scanner comparison records instead of complete media metadata.
    async fn load_file_fingerprints(&self) -> Result<Vec<FileFingerprint>>;

    /// The same, for one subtree.
    ///
    /// A scan compares what is on disk against what is indexed, and it only ever
    /// scans one root — so loading the whole table means every other library's
    /// rows are held in memory for nothing. That is most of the cost when a
    /// watcher event rescans a single folder.
    async fn load_file_fingerprints_under(
        &self,
        canonical_prefix: &str,
    ) -> Result<Vec<FileFingerprint>>;

    async fn get_root_availability(&self, path: &Path) -> Result<Option<RootAvailability>>;

    async fn list_root_availability(&self) -> Result<Vec<RootAvailability>>;

    async fn set_root_availability(&self, state: &RootAvailability) -> Result<()>;

    async fn remove_root_availability(&self, path: &Path) -> Result<()>;

    // Music categorization methods
    /// Get all unique artists
    async fn get_artists(&self) -> Result<Vec<MusicCategory>>;

    /// Get all albums, optionally filtered by artist
    async fn get_albums(&self, artist: Option<&str>) -> Result<Vec<MusicCategory>>;

    /// Get all genres
    async fn get_genres(&self) -> Result<Vec<MusicCategory>>;

    /// Get all years
    async fn get_years(&self) -> Result<Vec<MusicCategory>>;

    /// Get all album artists
    async fn get_album_artists(&self) -> Result<Vec<MusicCategory>>;

    /// Distinct values of one tag, restricted to the records a filter selects.
    ///
    /// The flat listings above are this with an empty filter; a nested browse
    /// tree is this with the ancestors it descended through.
    ///
    /// `child_of` names the tag one level further down. When given, each result
    /// also reports how many distinct values of *that* tag it contains, which
    /// is what a container whose children are containers must announce as its
    /// `childCount`.
    async fn get_music_categories(
        &self,
        kind: MusicCategoryType,
        filter: &MusicCategoryFilter,
        child_of: Option<MusicCategoryType>,
    ) -> Result<Vec<MusicCategory>>;

    /// Get music files by artist
    async fn get_music_by_artist(&self, artist: &str) -> Result<Vec<MediaFile>>;

    /// Get music files by album
    async fn get_music_by_album(&self, album: &str, artist: Option<&str>)
        -> Result<Vec<MediaFile>>;

    /// Get music files by genre
    async fn get_music_by_genre(&self, genre: &str) -> Result<Vec<MediaFile>>;

    /// Get music files by year
    async fn get_music_by_year(&self, year: u32) -> Result<Vec<MediaFile>>;

    /// Get music files by album artist
    async fn get_music_by_album_artist(&self, album_artist: &str) -> Result<Vec<MediaFile>>;

    /// Every tag stored for one record that has no column of its own.
    ///
    /// Returned as (normalized key, value) pairs, sorted by key. A tag with
    /// several values — two artists, three genres — appears once per value.
    async fn get_media_tags(&self, media_file_id: i64) -> Result<Vec<(String, String)>>;

    /// Get multiple files by their paths in a single query.
    async fn get_files_by_paths(&self, paths: &[PathBuf]) -> Result<Vec<MediaFile>>;

    /// Store multiple media files in a single batch operation.
    async fn bulk_store_media_files(&self, files: &[MediaFile]) -> Result<Vec<i64>>;

    /// Store scanner-owned records whose paths already satisfy the backend's
    /// canonical-path invariant. Backends may override this to skip defensive
    /// path resolution; the default keeps the safe public write behavior.
    async fn bulk_store_canonical_media_files(&self, files: &[MediaFile]) -> Result<Vec<i64>> {
        self.bulk_store_media_files(files).await
    }

    /// Update multiple media files in a single batch operation.
    async fn bulk_update_media_files(&self, files: &[MediaFile]) -> Result<()>;

    /// Update scanner-owned records with already-canonical paths.
    async fn bulk_update_canonical_media_files(&self, files: &[MediaFile]) -> Result<()> {
        self.bulk_update_media_files(files).await
    }

    /// Remove multiple media files by paths in a single batch operation.
    async fn bulk_remove_media_files(&self, paths: &[PathBuf]) -> Result<usize>;

    /// Atomically remove every media file at or below a path component boundary.
    async fn remove_media_under_path(&self, path: &Path) -> Result<RemovalSummary>;

    /// Get multiple files by their paths in a single batch query.
    async fn bulk_get_files_by_paths(&self, paths: &[PathBuf]) -> Result<Vec<MediaFile>> {
        self.get_files_by_paths(paths).await
    }

    /// Get files with a specific canonical path prefix.
    async fn get_files_with_path_prefix(&self, canonical_prefix: &str) -> Result<Vec<MediaFile>>;

    /// Get direct subdirectories using canonical paths.
    async fn get_direct_subdirectories(
        &self,
        canonical_parent_path: &str,
    ) -> Result<Vec<MediaDirectory>>;

    /// Batch cleanup missing files using canonical paths.
    async fn batch_cleanup_missing_files(
        &self,
        existing_canonical_paths: &HashSet<String>,
    ) -> Result<usize>;

    /// Perform cleanup using backend-native operations.
    async fn database_native_cleanup(&self, existing_canonical_paths: &[String]) -> Result<usize>;

    /// Get direct subdirectories containing the requested MIME family.
    async fn get_filtered_direct_subdirectories(
        &self,
        canonical_parent_path: &str,
        mime_filter: &str,
    ) -> Result<Vec<MediaDirectory>>;
}

/// Playlist storage and ordered-entry operations.
#[async_trait]
pub trait PlaylistRepository: Send + Sync {
    /// Create a new playlist
    async fn create_playlist(&self, name: &str, description: Option<&str>) -> Result<i64>;

    /// Get all playlists
    async fn get_playlists(&self) -> Result<Vec<Playlist>>;

    /// Get a specific playlist by ID
    async fn get_playlist(&self, playlist_id: i64) -> Result<Option<Playlist>>;

    /// Update a playlist
    async fn update_playlist(&self, playlist: &Playlist) -> Result<()>;

    /// Delete a playlist
    async fn delete_playlist(&self, playlist_id: i64) -> Result<bool>;

    /// Mark a playlist as derived from an on-disk source file.
    async fn set_playlist_source(&self, playlist_id: i64, source_path: &Path) -> Result<()>;

    /// Atomically create or replace the playlist derived from one source.
    async fn replace_playlist_from_source(
        &self,
        source_path: &Path,
        name: &str,
        media_file_ids: &[(i64, u32)],
    ) -> Result<i64>;

    /// Delete playlists/radio records derived from an on-disk source file.
    async fn remove_derived_content_by_source(&self, source_path: &Path) -> Result<usize>;

    /// Add a track to a playlist
    async fn add_to_playlist(
        &self,
        playlist_id: i64,
        media_file_id: i64,
        position: Option<u32>,
    ) -> Result<i64>;

    /// Add multiple tracks to a playlist in a single transaction (batch operation)
    async fn batch_add_to_playlist(
        &self,
        playlist_id: i64,
        media_file_ids: &[(i64, u32)],
    ) -> Result<Vec<i64>>;

    /// Remove a track from a playlist
    async fn remove_from_playlist(&self, playlist_id: i64, media_file_id: i64) -> Result<bool>;

    /// Get all tracks in a playlist
    async fn get_playlist_tracks(&self, playlist_id: i64) -> Result<Vec<MediaFile>>;

    /// Track counts for every playlist, keyed by playlist id.
    ///
    /// Browsing the playlist list needs one child count per container, which
    /// would otherwise be a query per row.
    async fn count_playlist_entries(&self) -> Result<HashMap<i64, usize>>;

    /// Reorder tracks in a playlist
    async fn reorder_playlist(
        &self,
        playlist_id: i64,
        track_positions: &[(i64, u32)],
    ) -> Result<()>;
}

/// Integrity, recovery, backup, and maintenance operations.
#[async_trait]
pub trait HealthRepository: Send + Sync {
    async fn check_and_repair(&self) -> Result<DatabaseHealth>;
    async fn rebuild_derived_indexes(&self) -> Result<DatabaseHealth>;
    async fn create_backup(&self, backup_path: &Path) -> Result<()>;
    async fn vacuum(&self) -> Result<bool>;
}

/// Database statistics independent of the storage backend.
#[async_trait]
pub trait StatsRepository: Send + Sync {
    async fn get_stats(&self) -> Result<DatabaseStats>;
}

/// Opaque secret storage, kept object-safe on purpose.
///
/// `DatabaseManager` has generic methods and so cannot be made into a trait
/// object; components that only need to persist a blob (AirPlay pairing keys,
/// for example) take `Arc<dyn SecretStore>` instead of being made generic over
/// the whole database.
#[async_trait]
pub trait SecretStore: Send + Sync {
    async fn get_secret(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn set_secret(&self, key: &str, value: &[u8]) -> Result<()>;
    async fn delete_secret(&self, key: &str) -> Result<bool>;
}

/// What a public metadata service said about one file.
///
/// Kept apart from [`MediaFile`] because the two have different lifetimes: a
/// media record is re-derived from the file on every scan, and this is not
/// derivable from the file at all. `payload` holds the provider's own record
/// whole, so a field that turns out to be worth showing is a query rather than
/// another migration — the same bargain `media_tags` makes for local tags.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaInfoRecord {
    pub media_file_id: i64,
    pub provider: String,
    pub remote_id: String,
    /// `movie` | `series` | `episode` | `album` | `track` | `anime`
    pub kind: String,
    pub title: Option<String>,
    pub original_title: Option<String>,
    pub overview: Option<String>,
    pub release_date: Option<String>,
    pub year: Option<u32>,
    pub rating: Option<f64>,
    pub genres: Vec<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    /// Key into the artwork cache, if a poster was downloaded.
    pub artwork_key: Option<String>,
    pub payload: String,
    /// 0–100. Below the configured threshold the row is kept but flagged.
    pub confidence: u8,
    pub fetched_at: SystemTime,
    pub mediainfo_version: u32,
}

/// How much of the library has been looked up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct MediaInfoStats {
    pub total: u64,
    /// Rows at or above the confidence threshold they were stored with.
    pub confident: u64,
    pub low_confidence: u64,
    pub with_artwork: u64,
}

/// Storage for fetched media info.
///
/// Not gated behind the `mediainfo` feature: it is plain SQL with no HTTP in it,
/// and gating it would fragment this trait and the conformance suite for no gain.
/// A build without the feature simply never writes a row.
#[async_trait]
pub trait MediaInfoRepository: Send + Sync {
    async fn get_mediainfo(&self, media_file_id: i64) -> Result<Option<MediaInfoRecord>>;
    /// Look up many at once. The browse path renders a page at a time and must not
    /// issue one query per row.
    async fn get_mediainfo_batch(&self, media_file_ids: &[i64]) -> Result<Vec<MediaInfoRecord>>;
    async fn bulk_store_mediainfo(&self, records: &[MediaInfoRecord]) -> Result<()>;
    /// The least certain matches first, for the operator to review.
    async fn list_low_confidence(&self, threshold: u8, limit: usize)
        -> Result<Vec<MediaInfoRecord>>;
    async fn mediainfo_stats(&self, threshold: u8) -> Result<MediaInfoStats>;
    /// Forget everything, so the next run starts over.
    async fn clear_mediainfo(&self) -> Result<u64>;
    /// Ids of files that have no usable row yet, oldest first.
    ///
    /// `version` is the current reader version: rows written by an older one are
    /// treated as absent so a bumped version re-fetches.
    async fn media_ids_missing_mediainfo(&self, version: u32, threshold: u8) -> Result<Vec<i64>>;
}

/// Aggregate database capability used by the application.
#[async_trait]
pub trait DatabaseManager:
    MediaRepository
    + PlaylistRepository
    + HealthRepository
    + StatsRepository
    + SecretStore
    + MediaInfoRepository
    + Send
    + Sync
{
    /// Initialize the database and create tables if needed.
    async fn initialize(&self) -> Result<()>;

    // Playlist file format operations remain aggregate helpers because importing
    // and exporting spans both media and playlist repositories.
    /// Import a playlist from a file (.m3u or .pls)
    async fn import_playlist_file(
        &self,
        file_path: &Path,
        playlist_name: Option<String>,
    ) -> Result<i64> {
        playlist_formats::PlaylistFileManager::import_playlist(self, file_path, playlist_name).await
    }

    /// Scan directory for playlist files and import them
    async fn scan_and_import_playlists(&self, directory: &Path) -> Result<Vec<i64>> {
        playlist_formats::PlaylistFileManager::scan_and_import_playlists(self, directory).await
    }

    /// Recursively scan directory tree for playlist files and import them
    async fn scan_and_import_playlists_recursive(&self, directory: &Path) -> Result<Vec<i64>> {
        playlist_formats::PlaylistFileManager::scan_and_import_playlists_recursive(self, directory)
            .await
    }
}

/// Backend-neutral open parameters.
///
/// New tuning knobs land here rather than in `DatabaseBackend::open`, so adding
/// one does not churn every backend's signature.
#[derive(Clone, Debug)]
pub struct DatabaseSettings {
    /// Full path to the database file, extension included.
    pub path: PathBuf,
    /// Page-cache budget in mebibytes.
    pub cache_mb: usize,
}

impl DatabaseSettings {
    pub fn new(path: PathBuf, cache_mb: usize) -> Self {
        Self { path, cache_mb }
    }
}

/// Construction boundary for statically dispatched database backends.
/// Backend selection happens once at startup; record loops remain monomorphized.
#[async_trait]
pub trait DatabaseBackend: DatabaseManager + Sized + 'static {
    async fn open(settings: &DatabaseSettings) -> Result<Self>;

    fn backend_name() -> &'static str;

    /// Canonical on-disk extension, without a leading dot.
    ///
    /// Drives the database path, backup filenames, and backup retention, so a
    /// backend never has its file naming decided by a call site.
    fn file_extension() -> &'static str;

    /// Extensions of sidecar files that must travel with the main file whenever
    /// it is moved, quarantined, or replaced.
    ///
    /// Leaving a stale write-ahead log next to a replaced database silently
    /// resurrects records, so this is part of the backend contract rather than
    /// something each call site is expected to remember.
    fn sidecar_extensions() -> &'static [&'static str] {
        &[]
    }

    /// Validate a backup file and install it at `destination`.
    ///
    /// Runs before the database is opened. Implementations must validate first
    /// and leave `destination` untouched when the backup is unusable.
    async fn restore_backup_file(backup: &Path, destination: &Path) -> Result<()>;
}

#[derive(Clone, Debug)]
pub struct DatabaseStats {
    pub total_files: usize,
    pub total_size: u64,
    pub database_size: u64,
    pub video_files: usize,
    pub audio_files: usize,
    pub image_files: usize,
    pub playlists: usize,
}

#[derive(Debug, Clone)]
pub struct DatabaseHealth {
    pub is_healthy: bool,
    pub corruption_detected: bool,
    pub integrity_check_passed: bool,
    pub issues: Vec<DatabaseIssue>,
    pub repair_attempted: bool,
    pub repair_successful: bool,
}

#[derive(Debug, Clone)]
pub struct DatabaseIssue {
    pub severity: IssueSeverity,
    pub description: String,
    pub table_affected: Option<String>,
    pub suggested_action: String,
}

#[derive(Debug, Clone)]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

#[cfg(test)]
mod tests;

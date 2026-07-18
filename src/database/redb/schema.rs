// ReDB table schema and Rkyv archived record views.

// Table definitions for redb
// Primary table: stores MediaFile data keyed by ID
const FILES_TABLE: TableDefinition<i64, &[u8]> = TableDefinition::new("files");
// Index: path -> file ID (for lookups by path)
const PATH_INDEX: TableDefinition<&str, i64> = TableDefinition::new("path_index");
// Native one-to-many indexes. ReDB keeps each ID independently ordered, so
// inserts/removals no longer parse and rewrite an entire comma-separated blob.
const DIRECTORY_PATH_INDEX: TableDefinition<&str, u64> =
    TableDefinition::new("directory_path_index");
const DIRECTORY_RECORDS: TableDefinition<u64, &str> = TableDefinition::new("directory_records");
const DIRECTORY_CHILDREN: MultimapTableDefinition<u64, u64> =
    MultimapTableDefinition::new("directory_children");
const DIRECTORY_FILES: MultimapTableDefinition<u64, i64> =
    MultimapTableDefinition::new("directory_files");
// Composite key `<directory id>:<MIME family>`. Counts are recursive, so a
// direct child can be filtered without opening or decoding any media record.
const DIRECTORY_MIME_COUNTS: TableDefinition<&str, u64> =
    TableDefinition::new("directory_mime_counts");
// Playlists table
const PLAYLISTS_TABLE: TableDefinition<i64, &[u8]> = TableDefinition::new("playlists");
// Packed `(playlist_id, position)` key -> media file, plus a reverse mapping
// used to remove dangling playlist entries without scanning every playlist.
const PLAYLIST_ENTRIES: TableDefinition<u128, i64> = TableDefinition::new("playlist_entries");
const FILE_PLAYLIST_ENTRIES: MultimapTableDefinition<i64, u128> =
    MultimapTableDefinition::new("file_playlist_entries");
const PLAYLIST_SOURCES: TableDefinition<i64, &str> = TableDefinition::new("playlist_sources");
const SOURCE_PLAYLISTS: MultimapTableDefinition<&str, i64> =
    MultimapTableDefinition::new("source_playlists");
const METADATA_TABLE: TableDefinition<&str, u64> = TableDefinition::new("metadata");

const ARTIST_INDEX: MultimapTableDefinition<&str, i64> =
    MultimapTableDefinition::new("artist_index");
const ALBUM_INDEX: MultimapTableDefinition<&str, i64> = MultimapTableDefinition::new("album_index");
const GENRE_INDEX: MultimapTableDefinition<&str, i64> = MultimapTableDefinition::new("genre_index");
const YEAR_INDEX: MultimapTableDefinition<u32, i64> = MultimapTableDefinition::new("year_index");
const ALBUM_ARTIST_INDEX: MultimapTableDefinition<&str, i64> =
    MultimapTableDefinition::new("album_artist_index");
const SCHEMA_VERSION: u64 = 4;
const CODEC_VERSION: u64 = 1;

// Stable storage records. Keep these independent from application structs so
// schema changes are explicit and versioned.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
struct MediaFileSerializable {
    id: Option<i64>,
    path: String,
    filename: String,
    size: u64,
    modified_secs: u64,
    mime_type: String,
    duration_secs: Option<f64>,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,
    track_number: Option<u32>,
    year: Option<u32>,
    album_artist: Option<String>,
    subtitle_available: bool,
    created_at_secs: u64,
    updated_at_secs: u64,
}

impl From<&MediaFile> for MediaFileSerializable {
    fn from(file: &MediaFile) -> Self {
        Self {
            id: file.id,
            path: file.path.to_string_lossy().to_string(),
            filename: file.filename.clone(),
            size: file.size,
            modified_secs: file
                .modified
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            mime_type: file.mime_type.clone(),
            duration_secs: file.duration.map(|d| d.as_secs_f64()),
            title: file.title.clone(),
            artist: file.artist.clone(),
            album: file.album.clone(),
            genre: file.genre.clone(),
            track_number: file.track_number,
            year: file.year,
            album_artist: file.album_artist.clone(),
            subtitle_available: file.subtitle_available,
            created_at_secs: file
                .created_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            updated_at_secs: file
                .updated_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

impl From<MediaFileSerializable> for MediaFile {
    fn from(s: MediaFileSerializable) -> Self {
        Self {
            id: s.id,
            path: PathBuf::from(s.path),
            filename: s.filename,
            size: s.size,
            modified: UNIX_EPOCH + Duration::from_secs(s.modified_secs),
            mime_type: s.mime_type,
            duration: s.duration_secs.map(Duration::from_secs_f64),
            title: s.title,
            artist: s.artist,
            album: s.album,
            genre: s.genre,
            track_number: s.track_number,
            year: s.year,
            album_artist: s.album_artist,
            subtitle_available: s.subtitle_available,
            created_at: UNIX_EPOCH + Duration::from_secs(s.created_at_secs),
            updated_at: UNIX_EPOCH + Duration::from_secs(s.updated_at_secs),
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
struct PlaylistSerializable {
    id: Option<i64>,
    name: String,
    description: Option<String>,
    created_at_secs: u64,
    updated_at_secs: u64,
}

impl From<&Playlist> for PlaylistSerializable {
    fn from(playlist: &Playlist) -> Self {
        Self {
            id: playlist.id,
            name: playlist.name.clone(),
            description: playlist.description.clone(),
            created_at_secs: playlist
                .created_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            updated_at_secs: playlist
                .updated_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

impl From<PlaylistSerializable> for Playlist {
    fn from(s: PlaylistSerializable) -> Self {
        Self {
            id: s.id,
            name: s.name,
            description: s.description,
            created_at: UNIX_EPOCH + Duration::from_secs(s.created_at_secs),
            updated_at: UNIX_EPOCH + Duration::from_secs(s.updated_at_secs),
        }
    }
}

/// Validated borrowed view into one Rkyv value held by a ReDB access guard.
pub struct RkyvMediaFileView<'a> {
    archived: &'a ArchivedMediaFileSerializable,
}

pub struct RkyvPlaylistView<'a> {
    archived: &'a ArchivedPlaylistSerializable,
}

impl PlaylistView for RkyvPlaylistView<'_> {
    fn id(&self) -> Option<i64> {
        self.archived.id.as_ref().map(|value| value.to_native())
    }
    fn name(&self) -> &str {
        self.archived.name.as_str()
    }
    fn description(&self) -> Option<&str> {
        self.archived
            .description
            .as_ref()
            .map(|value| value.as_str())
    }
    fn created_at_secs(&self) -> u64 {
        self.archived.created_at_secs.to_native()
    }
    fn updated_at_secs(&self) -> u64 {
        self.archived.updated_at_secs.to_native()
    }
}

impl MediaFileView for RkyvMediaFileView<'_> {
    fn id(&self) -> Option<i64> {
        self.archived.id.as_ref().map(|value| value.to_native())
    }
    fn path(&self) -> &str {
        self.archived.path.as_str()
    }
    fn filename(&self) -> &str {
        self.archived.filename.as_str()
    }
    fn size(&self) -> u64 {
        self.archived.size.to_native()
    }
    fn modified_secs(&self) -> u64 {
        self.archived.modified_secs.to_native()
    }
    fn mime_type(&self) -> &str {
        self.archived.mime_type.as_str()
    }
    fn duration_secs(&self) -> Option<f64> {
        self.archived
            .duration_secs
            .as_ref()
            .map(|value| value.to_native())
    }
    fn title(&self) -> Option<&str> {
        self.archived.title.as_ref().map(|value| value.as_str())
    }
    fn artist(&self) -> Option<&str> {
        self.archived.artist.as_ref().map(|value| value.as_str())
    }
    fn album(&self) -> Option<&str> {
        self.archived.album.as_ref().map(|value| value.as_str())
    }
    fn genre(&self) -> Option<&str> {
        self.archived.genre.as_ref().map(|value| value.as_str())
    }
    fn track_number(&self) -> Option<u32> {
        self.archived
            .track_number
            .as_ref()
            .map(|value| value.to_native())
    }
    fn year(&self) -> Option<u32> {
        self.archived.year.as_ref().map(|value| value.to_native())
    }
    fn album_artist(&self) -> Option<&str> {
        self.archived
            .album_artist
            .as_ref()
            .map(|value| value.as_str())
    }
    fn subtitle_available(&self) -> bool {
        self.archived.subtitle_available
    }
    fn created_at_secs(&self) -> u64 {
        self.archived.created_at_secs.to_native()
    }
    fn updated_at_secs(&self) -> u64 {
        self.archived.updated_at_secs.to_native()
    }
}

pub struct RedbReadSession {
    transaction: redb::ReadTransaction,
}

impl RedbReadSession {
    fn view(data: &[u8]) -> Result<RkyvMediaFileView<'_>> {
        let archived = rkyv::access::<ArchivedMediaFileSerializable, rkyv::rancor::Error>(data)
            .map_err(|error| anyhow!("Invalid archived MediaFile: {error}"))?;
        Ok(RkyvMediaFileView { archived })
    }
}

impl DatabaseReadSession for RedbReadSession {
    type File<'a> = RkyvMediaFileView<'a>;
    type Playlist<'a> = RkyvPlaylistView<'a>;

    fn visit_files<F>(
        &mut self,
        query: &MediaFileQuery,
        offset: usize,
        limit: usize,
        mut visitor: F,
    ) -> Result<VisitSummary>
    where
        F: for<'a> FnMut(Self::File<'a>) -> Result<()>,
    {
        let files = self.transaction.open_table(FILES_TABLE)?;
        let mut summary = VisitSummary::default();

        macro_rules! emit_id {
            ($id:expr) => {{
                let id = $id;
                summary.matched += 1;
                if summary.matched > offset && summary.visited < limit {
                    if let Some(bytes) = files.get(id)? {
                        visitor(Self::view(bytes.value())?)?;
                        summary.visited += 1;
                    }
                }
            }};
        }

        match query {
            MediaFileQuery::All => {
                for entry in files.iter()? {
                    let (_, bytes) = entry?;
                    summary.matched += 1;
                    if summary.matched > offset && summary.visited < limit {
                        visitor(Self::view(bytes.value())?)?;
                        summary.visited += 1;
                    }
                }
            }
            MediaFileQuery::Id(id) => emit_id!(*id),
            MediaFileQuery::Path(path) => {
                let paths = self.transaction.open_table(PATH_INDEX)?;
                if let Some(id) = paths.get(path.as_str())? {
                    emit_id!(id.value());
                }
            }
            MediaFileQuery::Directory { path, mime_family } => {
                let paths = self.transaction.open_table(DIRECTORY_PATH_INDEX)?;
                if let Some(directory_id) = paths.get(path.as_str())? {
                    let index = self.transaction.open_multimap_table(DIRECTORY_FILES)?;
                    for id in index.get(directory_id.value())? {
                        let id = id?.value();
                        if let Some(family) = mime_family {
                            if let Some(bytes) = files.get(id)? {
                                if !Self::view(bytes.value())?.mime_type().starts_with(family) {
                                    continue;
                                }
                            }
                        }
                        emit_id!(id);
                    }
                }
            }
            MediaFileQuery::Artist(value) => {
                let index = self.transaction.open_multimap_table(ARTIST_INDEX)?;
                for id in index.get(value.as_str())? {
                    emit_id!(id?.value());
                }
            }
            MediaFileQuery::Album { album, artist } => {
                let index = self.transaction.open_multimap_table(ALBUM_INDEX)?;
                for id in index.get(album.as_str())? {
                    let id = id?.value();
                    if let Some(expected_artist) = artist {
                        if let Some(bytes) = files.get(id)? {
                            if Self::view(bytes.value())?.artist() != Some(expected_artist.as_str())
                            {
                                continue;
                            }
                        }
                    }
                    emit_id!(id);
                }
            }
            MediaFileQuery::Genre(value) => {
                let index = self.transaction.open_multimap_table(GENRE_INDEX)?;
                for id in index.get(value.as_str())? {
                    emit_id!(id?.value());
                }
            }
            MediaFileQuery::Year(value) => {
                let index = self.transaction.open_multimap_table(YEAR_INDEX)?;
                for id in index.get(*value)? {
                    emit_id!(id?.value());
                }
            }
            MediaFileQuery::AlbumArtist(value) => {
                let index = self.transaction.open_multimap_table(ALBUM_ARTIST_INDEX)?;
                for id in index.get(value.as_str())? {
                    emit_id!(id?.value());
                }
            }
            MediaFileQuery::Playlist(playlist_id) => {
                let entries = self.transaction.open_table(PLAYLIST_ENTRIES)?;
                for entry in entries.range(RedbDatabase::playlist_entry_range(*playlist_id))? {
                    let (_, id) = entry?;
                    emit_id!(id.value());
                }
            }
        }

        Ok(summary)
    }

    fn direct_subdirectories(
        &mut self,
        canonical_parent: &str,
        mime_family: Option<&str>,
    ) -> Result<Vec<MediaDirectory>> {
        let paths = self.transaction.open_table(DIRECTORY_PATH_INDEX)?;
        let records = self.transaction.open_table(DIRECTORY_RECORDS)?;
        let children = self.transaction.open_multimap_table(DIRECTORY_CHILDREN)?;
        let counts = self.transaction.open_table(DIRECTORY_MIME_COUNTS)?;
        let Some(parent_id) = paths.get(canonical_parent)?.map(|value| value.value()) else {
            return Ok(Vec::new());
        };
        let family = mime_family.filter(|value| !value.is_empty()).unwrap_or("*");
        let mut directories = Vec::new();
        for child in children.get(parent_id)? {
            let child_id = child?.value();
            let key = RedbDatabase::mime_count_key(child_id, family);
            if counts
                .get(key.as_str())?
                .is_none_or(|value| value.value() == 0)
            {
                continue;
            }
            if let Some(path) = records.get(child_id)? {
                let path = path.value().to_owned();
                let name = Path::new(&path)
                    .file_name()
                    .map(|value| value.to_string_lossy().into_owned())
                    .unwrap_or_default();
                directories.push(MediaDirectory {
                    path: PathBuf::from(path),
                    name,
                });
            }
        }
        directories.sort_by_cached_key(|directory| directory.name.to_lowercase());
        Ok(directories)
    }

    fn visit_playlists<F>(
        &mut self,
        offset: usize,
        limit: usize,
        mut visitor: F,
    ) -> Result<VisitSummary>
    where
        F: for<'a> FnMut(Self::Playlist<'a>) -> Result<()>,
    {
        let table = self.transaction.open_table(PLAYLISTS_TABLE)?;
        let mut summary = VisitSummary::default();
        for entry in table.iter()? {
            let (_, bytes) = entry?;
            summary.matched += 1;
            if summary.matched <= offset || summary.visited >= limit {
                continue;
            }
            let archived =
                rkyv::access::<ArchivedPlaylistSerializable, rkyv::rancor::Error>(bytes.value())
                    .map_err(|error| anyhow!("Invalid archived Playlist: {error}"))?;
            visitor(RkyvPlaylistView { archived })?;
            summary.visited += 1;
        }
        Ok(summary)
    }
}

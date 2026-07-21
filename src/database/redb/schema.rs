// ReDB table schema and Rkyv archived record views.

// This is the single registration point for every table. Callers provide a
// small callback macro for initialization, snapshots, or schema auditing.
// The final token documents whether health repair rebuilds the table from
// primary records or treats it as primary state.
macro_rules! redb_schema {
    ($callback:ident) => {
        $callback!(table, FILES_TABLE, i64, &[u8], "files", primary);
        $callback!(table, PATH_INDEX, &str, i64, "path_index", derived);
        $callback!(table, DIRECTORY_PATH_INDEX, &str, u64, "directory_path_index", derived);
        $callback!(table, DIRECTORY_RECORDS, u64, &str, "directory_records", derived);
        $callback!(multimap, DIRECTORY_CHILDREN, u64, u64, "directory_children", derived);
        $callback!(table, DIRECTORY_CHILDREN_BY_NAME, &str, u64, "directory_children_by_name", derived);
        $callback!(multimap, DIRECTORY_FILES, u64, i64, "directory_files", derived);
        $callback!(table, DIRECTORY_MIME_COUNTS, &str, u64, "directory_mime_counts", derived);
        $callback!(table, DIRECTORY_FILES_BY_NAME, &str, i64, "directory_files_by_name", derived);
        $callback!(table, PLAYLISTS_TABLE, i64, &[u8], "playlists", primary);
        $callback!(table, PLAYLIST_ENTRIES, u128, i64, "playlist_entries", primary);
        $callback!(multimap, FILE_PLAYLIST_ENTRIES, i64, u128, "file_playlist_entries", derived);
        $callback!(table, PLAYLIST_SOURCES, i64, &str, "playlist_sources", primary);
        $callback!(multimap, SOURCE_PLAYLISTS, &str, i64, "source_playlists", derived);
        $callback!(table, METADATA_TABLE, &str, u64, "metadata", primary);
        $callback!(table, ROOT_AVAILABILITY, &str, &[u8], "root_availability", primary);
        $callback!(multimap, ARTIST_INDEX, &str, i64, "artist_index", derived);
        $callback!(multimap, ALBUM_INDEX, &str, i64, "album_index", derived);
        $callback!(multimap, GENRE_INDEX, &str, i64, "genre_index", derived);
        $callback!(multimap, YEAR_INDEX, u32, i64, "year_index", derived);
        $callback!(multimap, ALBUM_ARTIST_INDEX, &str, i64, "album_artist_index", derived);
    };
}

macro_rules! declare_schema_entry {
    (table, $constant:ident, $key:ty, $value:ty, $name:literal, $role:ident) => {
        const $constant: TableDefinition<$key, $value> = TableDefinition::new($name);
    };
    (multimap, $constant:ident, $key:ty, $value:ty, $name:literal, $role:ident) => {
        const $constant: MultimapTableDefinition<$key, $value> =
            MultimapTableDefinition::new($name);
    };
}

redb_schema!(declare_schema_entry);
const SCHEMA_VERSION: u64 = 6;
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

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
struct RootAvailabilitySerializable {
    path: String,
    last_seen_secs: u64,
    unavailable_since_secs: Option<u64>,
    indexed_count: u64,
    reason: String,
}

impl From<&RootAvailability> for RootAvailabilitySerializable {
    fn from(state: &RootAvailability) -> Self {
        Self {
            path: state.path.to_string_lossy().into_owned(),
            last_seen_secs: state.last_seen_secs,
            unavailable_since_secs: state.unavailable_since_secs,
            indexed_count: state.indexed_count,
            reason: state.reason.clone(),
        }
    }
}

impl From<RootAvailabilitySerializable> for RootAvailability {
    fn from(state: RootAvailabilitySerializable) -> Self {
        Self {
            path: PathBuf::from(state.path),
            last_seen_secs: state.last_seen_secs,
            unavailable_since_secs: state.unavailable_since_secs,
            indexed_count: state.indexed_count,
            reason: state.reason,
        }
    }
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

pub struct RedbDirectoryView<'a> {
    id: u64,
    path: &'a str,
    name: &'a str,
}

impl DirectoryView for RedbDirectoryView<'_> {
    fn id(&self) -> u64 {
        self.id
    }

    fn path(&self) -> &str {
        self.path
    }

    fn name(&self) -> &str {
        self.name
    }
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
    type Directory<'a> = RedbDirectoryView<'a>;

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
                    let directory_id = directory_id.value();
                    let index = self.transaction.open_table(DIRECTORY_FILES_BY_NAME)?;
                    let (range_start, range_end) = RedbDatabase::directory_file_order_range(directory_id);
                    for entry in index.range(range_start.as_str()..range_end.as_str())? {
                        let (_, id) = entry?;
                        let id = id.value();
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
            MediaFileQuery::Filtered {
                after_id,
                mime_family,
                text,
            } => {
                let first_id = after_id.unwrap_or(i64::MIN).saturating_add(1);
                let needle_lower = text.as_deref().map(|n| n.to_lowercase());
                for entry in files.range(first_id..)? {
                    let (_, bytes) = entry?;
                    let view = Self::view(bytes.value())?;
                    if mime_family
                        .as_deref()
                        .is_some_and(|family| !view.mime_type().starts_with(family))
                    {
                        continue;
                    }
                    if let Some(needle) = needle_lower.as_deref() {
                        let matches_text = contains_ignore_ascii_case(view.filename(), needle)
                            || view.title().is_some_and(|v| contains_ignore_ascii_case(v, needle))
                            || view.artist().is_some_and(|v| contains_ignore_ascii_case(v, needle))
                            || view.album().is_some_and(|v| contains_ignore_ascii_case(v, needle));
                        if !matches_text {
                            continue;
                        }
                    }
                    summary.matched += 1;
                    if summary.matched > offset {
                        if summary.visited < limit {
                            visitor(view)?;
                            summary.visited += 1;
                        }
                        if summary.visited >= limit {
                            break;
                        }
                    }
                }
            }
        }

        Ok(summary)
    }

    fn visit_direct_subdirectories<F>(
        &mut self,
        canonical_parent: &str,
        mime_family: Option<&str>,
        offset: usize,
        limit: usize,
        mut visitor: F,
    ) -> Result<VisitSummary>
    where
        F: for<'a> FnMut(Self::Directory<'a>) -> Result<()>,
    {
        let paths = self.transaction.open_table(DIRECTORY_PATH_INDEX)?;
        let records = self.transaction.open_table(DIRECTORY_RECORDS)?;
        let children = self.transaction.open_table(DIRECTORY_CHILDREN_BY_NAME)?;
        let counts = self.transaction.open_table(DIRECTORY_MIME_COUNTS)?;
        let Some(parent_id) = paths.get(canonical_parent)?.map(|value| value.value()) else {
            return Ok(VisitSummary::default());
        };
        let family = mime_family.filter(|value| !value.is_empty()).unwrap_or("*");
        let (range_start, range_end) = RedbDatabase::directory_order_range(parent_id);
        let mut summary = VisitSummary::default();
        for child in children.range(range_start.as_str()..range_end.as_str())? {
            let (_, child_id) = child?;
            let child_id = child_id.value();
            let key = RedbDatabase::mime_count_key(child_id, family);
            if counts
                .get(key.as_str())?
                .is_none_or(|value| value.value() == 0)
            {
                continue;
            }
            summary.matched += 1;
            if summary.matched <= offset || summary.visited >= limit {
                continue;
            }
            if let Some(path) = records.get(child_id)? {
                let path = path.value();
                visitor(RedbDirectoryView {
                    id: child_id,
                    path,
                    name: RedbDatabase::directory_name(path),
                })?;
                summary.visited += 1;
            }
        }
        Ok(summary)
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

fn contains_ignore_ascii_case(value: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if value.is_ascii() && needle.is_ascii() {
        value
            .as_bytes()
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
    } else {
        value.to_lowercase().contains(&needle.to_lowercase())
    }
}

//! The music browse tree.
//!
//! Everything under `audio` other than the folder view is described by
//! [`MusicNode`]: one enum listing every container the tree can address, one
//! parser turning an object id into it, and one builder turning it back into
//! child ids. Adding a level means adding a variant, not another handler.
//!
//! ## Object ids
//!
//! A node's id is its path through the tree, `audio/artists/Metallica/Ride the
//! Lightning`. Tag values become path segments, so they are percent-encoded:
//! without that, an artist called `AC/DC` is indistinguishable from an artist
//! `AC` holding an album `DC`.
//!
//! Structural segments are spelled `!all` and the encoder escapes `!`, so no
//! tag value can ever collide with one — an album genuinely called `!all`
//! encodes as `%21all` and still resolves to itself.

use super::*;
use crate::database::{MusicCategoryFilter, MusicCategoryType};
use crate::web::xml::{container_class, generate_container_list_response, ContainerSpec};
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};

/// Characters that must not survive into an object id verbatim.
///
/// `/` is the path separator, `%` is the escape itself, and `!` introduces a
/// structural segment. The rest are escaped because they are awkward inside XML
/// attributes and URLs even though the writer already escapes XML.
const ID_SEGMENT: &AsciiSet = &CONTROLS
    .add(b'%')
    .add(b'/')
    .add(b'!')
    .add(b'?')
    .add(b'#')
    .add(b'&')
    .add(b'"')
    .add(b'\'')
    .add(b'<')
    .add(b'>');

/// The structural segment meaning "every track at this level".
const ALL: &str = "!all";

pub(super) fn encode_segment(value: &str) -> String {
    utf8_percent_encode(value, ID_SEGMENT).to_string()
}

fn decode_segment(value: &str) -> String {
    percent_decode_str(value).decode_utf8_lossy().into_owned()
}

/// Every container and track listing the music tree can address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum MusicNode {
    /// `audio` — the six category containers.
    Root,
    /// `audio/!all` — every track in the library.
    AllMusic,

    /// `audio/artists`
    ArtistList,
    /// `audio/artists/{artist}` — that artist's albums, plus All Songs.
    Artist(String),
    /// `audio/artists/{artist}/!all`
    ArtistAll(String),
    /// `audio/artists/{artist}/{album}`
    ArtistAlbum(String, String),

    /// `audio/albumartists`
    AlbumArtistList,
    /// `audio/albumartists/{album_artist}`
    AlbumArtist(String),
    /// `audio/albumartists/{album_artist}/!all`
    AlbumArtistAll(String),
    /// `audio/albumartists/{album_artist}/{album}`
    AlbumArtistAlbum(String, String),

    /// `audio/albums`
    AlbumList,
    /// `audio/albums/{album}`
    Album(String),

    /// `audio/genres`
    GenreList,
    /// `audio/genres/{genre}` — that genre's artists, plus All Songs.
    Genre(String),
    /// `audio/genres/{genre}/!all`
    GenreAll(String),
    /// `audio/genres/{genre}/{artist}`
    GenreArtist(String, String),
    /// `audio/genres/{genre}/{artist}/!all`
    GenreArtistAll(String, String),
    /// `audio/genres/{genre}/{artist}/{album}`
    GenreArtistAlbum(String, String, String),

    /// `audio/years`
    YearList,
    /// `audio/years/{year}`
    Year(u32),

    /// `audio/playlists`
    PlaylistList,
    /// `audio/playlists/{id}`
    Playlist(i64),

    /// `audio/folders/...` — the filesystem view, handled by the folder browse.
    Folders(String),
}

/// Parse the part of an object id after `audio/`.
///
/// Returns `None` for a path that names no node, which the caller answers as a
/// legacy folder browse rather than an error.
pub(super) fn parse_music_path(audio_path: &str) -> Option<MusicNode> {
    let path = audio_path.trim_matches('/');
    if path.is_empty() {
        return Some(MusicNode::Root);
    }

    let mut segments = path.split('/');
    let head = segments.next()?;

    // The folder view keeps raw filesystem paths, which are not encoded.
    if head == "folders" {
        let rest = path.strip_prefix("folders").unwrap_or("").trim_start_matches('/');
        return Some(MusicNode::Folders(rest.to_owned()));
    }

    let rest: Vec<&str> = segments.collect();
    let value = |index: usize| decode_segment(rest[index]);
    let is_all = |index: usize| rest[index] == ALL;

    match (head, rest.len()) {
        (ALL, 0) => Some(MusicNode::AllMusic),

        ("artists", 0) => Some(MusicNode::ArtistList),
        ("artists", 1) if is_all(0) => None,
        ("artists", 1) => Some(MusicNode::Artist(value(0))),
        ("artists", 2) if is_all(1) => Some(MusicNode::ArtistAll(value(0))),
        ("artists", 2) => Some(MusicNode::ArtistAlbum(value(0), value(1))),

        ("albumartists", 0) => Some(MusicNode::AlbumArtistList),
        ("albumartists", 1) if is_all(0) => None,
        ("albumartists", 1) => Some(MusicNode::AlbumArtist(value(0))),
        ("albumartists", 2) if is_all(1) => Some(MusicNode::AlbumArtistAll(value(0))),
        ("albumartists", 2) => Some(MusicNode::AlbumArtistAlbum(value(0), value(1))),

        ("albums", 0) => Some(MusicNode::AlbumList),
        ("albums", 1) if is_all(0) => None,
        ("albums", 1) => Some(MusicNode::Album(value(0))),

        ("genres", 0) => Some(MusicNode::GenreList),
        ("genres", 1) if is_all(0) => None,
        ("genres", 1) => Some(MusicNode::Genre(value(0))),
        ("genres", 2) if is_all(1) => Some(MusicNode::GenreAll(value(0))),
        ("genres", 2) => Some(MusicNode::GenreArtist(value(0), value(1))),
        ("genres", 3) if is_all(2) => Some(MusicNode::GenreArtistAll(value(0), value(1))),
        ("genres", 3) => Some(MusicNode::GenreArtistAlbum(value(0), value(1), value(2))),

        ("years", 0) => Some(MusicNode::YearList),
        ("years", 1) => value(0).parse().ok().map(MusicNode::Year),

        ("playlists", 0) => Some(MusicNode::PlaylistList),
        ("playlists", 1) => value(0).parse().ok().map(MusicNode::Playlist),

        _ => None,
    }
}

impl MusicNode {
    /// The object id that addresses this node.
    pub(super) fn object_id(&self) -> String {
        let join = |parts: &[&str]| {
            let mut id = String::from("audio");
            for part in parts {
                id.push('/');
                id.push_str(part);
            }
            id
        };
        match self {
            Self::Root => "audio".to_owned(),
            Self::AllMusic => join(&[ALL]),

            Self::ArtistList => join(&["artists"]),
            Self::Artist(artist) => join(&["artists", &encode_segment(artist)]),
            Self::ArtistAll(artist) => join(&["artists", &encode_segment(artist), ALL]),
            Self::ArtistAlbum(artist, album) => join(&[
                "artists",
                &encode_segment(artist),
                &encode_segment(album),
            ]),

            Self::AlbumArtistList => join(&["albumartists"]),
            Self::AlbumArtist(who) => join(&["albumartists", &encode_segment(who)]),
            Self::AlbumArtistAll(who) => join(&["albumartists", &encode_segment(who), ALL]),
            Self::AlbumArtistAlbum(who, album) => join(&[
                "albumartists",
                &encode_segment(who),
                &encode_segment(album),
            ]),

            Self::AlbumList => join(&["albums"]),
            Self::Album(album) => join(&["albums", &encode_segment(album)]),

            Self::GenreList => join(&["genres"]),
            Self::Genre(genre) => join(&["genres", &encode_segment(genre)]),
            Self::GenreAll(genre) => join(&["genres", &encode_segment(genre), ALL]),
            Self::GenreArtist(genre, artist) => join(&[
                "genres",
                &encode_segment(genre),
                &encode_segment(artist),
            ]),
            Self::GenreArtistAll(genre, artist) => join(&[
                "genres",
                &encode_segment(genre),
                &encode_segment(artist),
                ALL,
            ]),
            Self::GenreArtistAlbum(genre, artist, album) => join(&[
                "genres",
                &encode_segment(genre),
                &encode_segment(artist),
                &encode_segment(album),
            ]),

            Self::YearList => join(&["years"]),
            Self::Year(year) => join(&["years", &year.to_string()]),

            Self::PlaylistList => join(&["playlists"]),
            Self::Playlist(id) => join(&["playlists", &id.to_string()]),

            Self::Folders(path) if path.is_empty() => join(&["folders"]),
            Self::Folders(path) => format!("audio/folders/{path}"),
        }
    }

    /// The object id of the container this node sits in.
    pub(super) fn parent_id(&self) -> String {
        match self {
            Self::Root => "0".to_owned(),
            Self::AllMusic
            | Self::ArtistList
            | Self::AlbumArtistList
            | Self::AlbumList
            | Self::GenreList
            | Self::YearList
            | Self::PlaylistList => "audio".to_owned(),

            Self::Artist(_) => Self::ArtistList.object_id(),
            Self::ArtistAll(artist) | Self::ArtistAlbum(artist, _) => {
                Self::Artist(artist.clone()).object_id()
            }

            Self::AlbumArtist(_) => Self::AlbumArtistList.object_id(),
            Self::AlbumArtistAll(who) | Self::AlbumArtistAlbum(who, _) => {
                Self::AlbumArtist(who.clone()).object_id()
            }

            Self::Album(_) => Self::AlbumList.object_id(),

            Self::Genre(_) => Self::GenreList.object_id(),
            Self::GenreAll(genre) | Self::GenreArtist(genre, _) => {
                Self::Genre(genre.clone()).object_id()
            }
            Self::GenreArtistAll(genre, artist) | Self::GenreArtistAlbum(genre, artist, _) => {
                Self::GenreArtist(genre.clone(), artist.clone()).object_id()
            }

            Self::Year(_) => Self::YearList.object_id(),
            Self::Playlist(_) => Self::PlaylistList.object_id(),

            Self::Folders(path) => match path.rsplit_once('/') {
                Some((parent, _)) => format!("audio/folders/{parent}"),
                None if path.is_empty() => "audio".to_owned(),
                None => "audio/folders".to_owned(),
            },
        }
    }

    /// The title a control point shows for this node.
    pub(super) fn title(&self) -> String {
        match self {
            Self::Root => "Music".to_owned(),
            Self::AllMusic => "All Music".to_owned(),
            Self::ArtistList => "Artists".to_owned(),
            Self::AlbumArtistList => "Album Artists".to_owned(),
            Self::AlbumList => "Albums".to_owned(),
            Self::GenreList => "Genres".to_owned(),
            Self::YearList => "Years".to_owned(),
            Self::PlaylistList => "Playlists".to_owned(),

            Self::ArtistAll(_)
            | Self::AlbumArtistAll(_)
            | Self::GenreAll(_)
            | Self::GenreArtistAll(_, _) => "All Songs".to_owned(),

            Self::Artist(value)
            | Self::AlbumArtist(value)
            | Self::Album(value)
            | Self::Genre(value) => value.clone(),
            Self::ArtistAlbum(_, album)
            | Self::AlbumArtistAlbum(_, album)
            | Self::GenreArtistAlbum(_, _, album) => album.clone(),
            Self::GenreArtist(_, artist) => artist.clone(),

            Self::Year(year) => year.to_string(),
            // A playlist's name lives in the database, not in its object id.
            // `display_title` resolves it; this is only the fallback for a
            // playlist that has since been deleted.
            Self::Playlist(id) => format!("Playlist {id}"),

            Self::Folders(path) if path.is_empty() => "Folders".to_owned(),
            Self::Folders(path) => path.rsplit('/').next().unwrap_or("Folders").to_owned(),
        }
    }

    /// The UPnP class this node announces itself as.
    pub(super) fn class(&self) -> &'static str {
        match self {
            Self::Artist(_) | Self::AlbumArtist(_) | Self::GenreArtist(_, _) => {
                container_class::MUSIC_ARTIST
            }
            Self::Album(_)
            | Self::ArtistAlbum(_, _)
            | Self::AlbumArtistAlbum(_, _)
            | Self::GenreArtistAlbum(_, _, _) => container_class::MUSIC_ALBUM,
            Self::Genre(_) => container_class::MUSIC_GENRE,
            Self::Playlist(_) => container_class::PLAYLIST,
            _ => container_class::STORAGE_FOLDER,
        }
    }

    /// The tracks this node lists, or `None` if it lists containers instead.
    pub(super) fn track_query(&self) -> Option<crate::database::MediaFileQuery> {
        use crate::database::MediaFileQuery::{Music, Playlist};

        let music = |filter: MusicCategoryFilter| {
            Some(Music {
                artist: filter.artist,
                album_artist: filter.album_artist,
                album: filter.album,
                genre: filter.genre,
                year: filter.year,
                exclude_radio: true,
            })
        };

        match self {
            Self::AllMusic => music(MusicCategoryFilter::default()),
            Self::ArtistAll(artist) => music(MusicCategoryFilter::artist(artist)),
            Self::ArtistAlbum(artist, album) => music(MusicCategoryFilter {
                album: Some(album.clone()),
                ..MusicCategoryFilter::artist(artist)
            }),
            Self::AlbumArtistAll(who) => music(MusicCategoryFilter::album_artist(who)),
            Self::AlbumArtistAlbum(who, album) => music(MusicCategoryFilter {
                album: Some(album.clone()),
                ..MusicCategoryFilter::album_artist(who)
            }),
            Self::Album(album) => music(MusicCategoryFilter {
                album: Some(album.clone()),
                ..MusicCategoryFilter::default()
            }),
            Self::GenreAll(genre) => music(MusicCategoryFilter::genre(genre)),
            Self::GenreArtistAll(genre, artist) => {
                music(MusicCategoryFilter::genre(genre).with_artist(artist))
            }
            Self::GenreArtistAlbum(genre, artist, album) => music(MusicCategoryFilter {
                album: Some(album.clone()),
                ..MusicCategoryFilter::genre(genre).with_artist(artist)
            }),
            Self::Year(year) => music(MusicCategoryFilter {
                year: Some(*year),
                ..MusicCategoryFilter::default()
            }),
            Self::Playlist(id) => Some(Playlist(*id)),
            _ => None,
        }
    }
}

/// The six containers directly under Music.
pub(super) fn root_children() -> Vec<MusicNode> {
    vec![
        MusicNode::AllMusic,
        MusicNode::ArtistList,
        MusicNode::AlbumArtistList,
        MusicNode::AlbumList,
        MusicNode::GenreList,
        MusicNode::YearList,
        MusicNode::PlaylistList,
        MusicNode::Folders(String::new()),
    ]
}

/// The containers a node holds. Empty for a node that lists tracks instead;
/// see [`MusicNode::track_query`].
pub(super) async fn child_containers<D: DatabaseManager + 'static>(
    node: &MusicNode,
    state: &AppState<D>,
) -> anyhow::Result<Vec<ContainerSpec>> {
    use MusicCategoryType as Kind;

    let specs = match node {
        MusicNode::Root => root_children()
            .into_iter()
            .map(|child| spec_for(&child, 1, None))
            .collect(),

        MusicNode::ArtistList => {
            category_specs(
                state,
                Kind::Artist,
                MusicCategoryFilter::default(),
                Some(Kind::Album),
                &MusicNode::Artist,
            )
            .await?
        }

        MusicNode::AlbumArtistList => {
            category_specs(
                state,
                Kind::AlbumArtist,
                MusicCategoryFilter::default(),
                Some(Kind::Album),
                &MusicNode::AlbumArtist,
            )
            .await?
        }

        MusicNode::AlbumList => {
            category_specs(
                state,
                Kind::Album,
                MusicCategoryFilter::default(),
                None,
                &MusicNode::Album,
            )
            .await?
        }

        MusicNode::GenreList => {
            category_specs(
                state,
                Kind::Genre,
                MusicCategoryFilter::default(),
                Some(Kind::Artist),
                &MusicNode::Genre,
            )
            .await?
        }

        // Years are stored as integers, so a value that will not parse is a
        // record with a malformed tag rather than a browsable container.
        MusicNode::YearList => {
            let database = state.database.clone();
            database
                .get_music_categories(Kind::Year, &MusicCategoryFilter::default(), None)
                .await?
                .into_iter()
                .filter_map(|category| {
                    let year = category.name.parse().ok()?;
                    Some(spec_for(
                        &MusicNode::Year(year),
                        category.count,
                        category.sample_id,
                    ))
                })
                .collect()
        }

        // An artist container holds their albums, with All Songs first so a
        // renderer can play everything without descending.
        MusicNode::Artist(artist) => {
            let filter = MusicCategoryFilter::artist(artist);
            album_children(state, MusicNode::ArtistAll(artist.clone()), filter, &|album| {
                MusicNode::ArtistAlbum(artist.clone(), album)
            })
            .await?
        }

        MusicNode::AlbumArtist(who) => {
            let filter = MusicCategoryFilter::album_artist(who);
            album_children(
                state,
                MusicNode::AlbumArtistAll(who.clone()),
                filter,
                &|album| MusicNode::AlbumArtistAlbum(who.clone(), album),
            )
            .await?
        }

        // A genre holds its artists, matching minidlna's genre/artist/album
        // shape, with All Songs at the top.
        MusicNode::Genre(genre) => {
            let filter = MusicCategoryFilter::genre(genre);
            let mut specs = vec![spec_for(&MusicNode::GenreAll(genre.clone()), 1, None)];
            specs.extend(
                category_specs(state, Kind::Artist, filter, Some(Kind::Album), &|artist| {
                    MusicNode::GenreArtist(genre.clone(), artist)
                })
                .await?,
            );
            specs
        }

        MusicNode::GenreArtist(genre, artist) => {
            let filter = MusicCategoryFilter::genre(genre).with_artist(artist);
            album_children(
                state,
                MusicNode::GenreArtistAll(genre.clone(), artist.clone()),
                filter,
                &|album| MusicNode::GenreArtistAlbum(genre.clone(), artist.clone(), album),
            )
            .await?
        }

        MusicNode::PlaylistList => {
            let database = state.database.clone();
            let playlists = database.get_playlists().await?;
            let counts = database.count_playlist_entries().await?;
            playlists
                .into_iter()
                .filter_map(|playlist| {
                    let id = playlist.id?;
                    let count = counts.get(&id).copied().unwrap_or(0);
                    Some(
                        ContainerSpec::folder(MusicNode::Playlist(id).object_id(), playlist.name)
                            .with_class(container_class::PLAYLIST)
                            .with_child_count(count),
                    )
                })
                .collect()
        }

        _ => Vec::new(),
    };

    Ok(specs)
}

/// One container per distinct value of a tag, carrying its real child count.
///
/// `child_of` names the tag one level down. Given it, each container counts its
/// sub-containers — plus the All Songs node the tree inserts — rather than the
/// tracks underneath, which is a different and much larger number.
async fn category_specs<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    kind: MusicCategoryType,
    filter: MusicCategoryFilter,
    child_of: Option<MusicCategoryType>,
    to_node: &(dyn Fn(String) -> MusicNode + Sync),
) -> anyhow::Result<Vec<ContainerSpec>> {
    let database = state.database.clone();
    let found = database
        .get_music_categories(kind, &filter, child_of)
        .await?;
    Ok(found
        .into_iter()
        .map(|category| {
            let child = to_node(category.name.clone());
            let children = match category.child_count {
                Some(count) => count + 1, // the All Songs node
                None => category.count,
            };
            spec_for(&child, children, category.sample_id)
        })
        .collect())
}

/// The albums under an artist-like container, preceded by All Songs.
async fn album_children<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    all_node: MusicNode,
    filter: MusicCategoryFilter,
    to_album: &(dyn Fn(String) -> MusicNode + Sync),
) -> anyhow::Result<Vec<ContainerSpec>> {
    let database = state.database.clone();
    let found = database
        .get_music_categories(MusicCategoryType::Album, &filter, None)
        .await?;
    let mut specs = vec![spec_for(&all_node, 1, None)];
    specs.extend(found.into_iter().map(|category| {
        let child = to_album(category.name.clone());
        spec_for(&child, category.count, category.sample_id)
    }));
    Ok(specs)
}

/// The title to show for a node, resolving the ones the object id cannot carry.
///
/// A control point that probes a container with `BrowseMetadata` must be told
/// the same name its parent's listing used, or the two disagree about one
/// object.
pub(super) async fn display_title<D: DatabaseManager + 'static>(
    node: &MusicNode,
    state: &AppState<D>,
) -> String {
    if let MusicNode::Playlist(id) = node {
        if let Ok(Some(playlist)) = state.database.get_playlist(*id).await {
            return playlist.name;
        }
    }
    node.title()
}

fn spec_for(node: &MusicNode, child_count: usize, album_art_id: Option<i64>) -> ContainerSpec {
    ContainerSpec::folder(node.object_id(), node.title())
        .with_class(node.class())
        .with_child_count(child_count.max(1))
        .with_album_art(album_art_id)
}

pub(super) async fn render_context<D: DatabaseManager>(
    state: &AppState<D>,
) -> crate::web::xml::BrowseRenderContext {
    let client = crate::web::client::CURRENT_CLIENT
        .try_with(|client| *client)
        .unwrap_or(crate::web::client::DlnaClientProfile::Standard);
    let bookmarks = if matches!(
        client,
        crate::web::client::DlnaClientProfile::SamsungTv
            | crate::web::client::DlnaClientProfile::SamsungTvQ
    ) {
        state.bookmarks.lock().await.snapshot()
    } else {
        std::collections::HashMap::new()
    };
    crate::web::xml::BrowseRenderContext {
        client,
        server_ip: state.get_server_ip(),
        server_port: state.http_binding.port(),
        autoplay_enabled: state.current_config().media.autoplay_enabled,
        update_id: state.content_update_id.load(Ordering::SeqCst),
        bookmarks,
    }
}

/// Serve any node of the music tree.
///
/// Containers and tracks differ only in how the body is produced, so the cache
/// lookup, cache insert and metrics around them are written once here.
pub(super) async fn handle_music_node_browse<D: DatabaseManager + 'static>(
    params: &BrowseParams,
    state: &AppState<D>,
    node: MusicNode,
) -> Response {
    let start_time = Instant::now();
    let client = crate::web::client::CURRENT_CLIENT
        .try_with(|c| *c)
        .unwrap_or(crate::web::client::DlnaClientProfile::Standard);

    let current_update_id = state.content_update_id.load(Ordering::SeqCst);
    let browse_epoch = state.browse_cache.lock().await.epoch();
    let cache_key = crate::state::SoapCacheKey {
        object_id: params.object_id.clone(),
        starting_index: params.starting_index,
        requested_count: params.requested_count,
        client_profile: client,
        content_update_id: current_update_id,
        browse_epoch,
    };

    {
        let mut cache = state.browse_cache.lock().await;
        if cache
            .generation()
            .is_some_and(|generation| generation != current_update_id)
        {
            cache.clear();
        }
        if let Some(cached) = cache.get(&cache_key) {
            let elapsed = start_time.elapsed().as_micros() as u64;
            state.web_metrics.record_browse_request(elapsed, true);
            debug!(
                "Browse cache hit for music ObjectID {} ({}us)",
                params.object_id, elapsed
            );
            return xml_response(cached.clone());
        }
    }

    let context = render_context(state).await;
    let object_id = node.object_id();

    let body = match node.track_query() {
        // A leaf lists tracks, paged inside the read transaction so no record
        // is materialized just to be skipped.
        Some(query) => {
            let starting_index = params.starting_index as usize;
            let requested_count = browse_page_limit(params);
            let database = state.database.clone();
            let owned_id = object_id.clone();
            match database
                .read(move |session| {
                    crate::web::xml::generate_indexed_items_response(
                        session,
                        query,
                        &owned_id,
                        starting_index,
                        requested_count,
                        context,
                    )
                })
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    error!(%error, "Music browse failed for {}", params.object_id);
                    state.web_metrics.record_error();
                    state
                        .web_metrics
                        .record_browse_request(start_time.elapsed().as_micros() as u64, false);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
                        .into_response();
                }
            }
        }
        None => match child_containers(&node, state).await {
            Ok(containers) => {
                let total = containers.len();
                let page = browse_page_bounds(params, total);
                generate_container_list_response(&object_id, &containers[page], total, &context)
                    .into()
            }
            Err(error) => {
                error!(%error, "Music category listing failed for {}", params.object_id);
                state.web_metrics.record_error();
                state
                    .web_metrics
                    .record_browse_request(start_time.elapsed().as_micros() as u64, false);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
                    .into_response();
            }
        },
    };

    let elapsed = start_time.elapsed().as_micros() as u64;
    state.web_metrics.record_browse_request(elapsed, false);
    debug!("Served music ObjectID {} in {}us", params.object_id, elapsed);

    // A scan that landed while this response was being built has already
    // invalidated it, so it must not enter the cache.
    if state.content_update_id.load(Ordering::SeqCst) == current_update_id {
        let mut cache = state.browse_cache.lock().await;
        if cache
            .generation()
            .is_some_and(|generation| generation != current_update_id)
        {
            cache.clear();
        }
        cache.insert(cache_key, body.clone());
    }

    xml_response(body)
}

fn xml_response(body: axum::body::Bytes) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
            (header::HeaderName::from_static("ext"), ""),
        ],
        body,
    )
        .into_response()
}

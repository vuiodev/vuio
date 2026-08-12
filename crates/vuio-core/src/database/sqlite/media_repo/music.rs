//! Music categorization.
//!
//! Every category here is a `GROUP BY` over an indexed column. The equivalent
//! in a key-value store is a secondary index table maintained by hand on every
//! write; the point of moving to SQL is that these need no maintenance at all.

use anyhow::Result;

use crate::database::sqlite::SqliteDatabase;
use crate::database::{
    MediaFile, MediaFileQuery, MusicCategory, MusicCategoryFilter, MusicCategoryType,
};

/// The column each category groups on.
fn category_column(kind: &MusicCategoryType) -> &'static str {
    match kind {
        MusicCategoryType::Artist => "artist",
        MusicCategoryType::Album => "album",
        MusicCategoryType::Genre => "genre",
        MusicCategoryType::AlbumArtist => "album_artist",
        MusicCategoryType::Year => "year",
        // Playlists live in their own table and never reach this query.
        MusicCategoryType::Playlist => "album",
    }
}

impl SqliteDatabase {
    /// Distinct values of one tag column, with the number of records carrying each.
    async fn categories(
        &self,
        kind: MusicCategoryType,
        filter: MusicCategoryFilter,
    ) -> Result<Vec<MusicCategory>> {
        let column = category_column(&kind);

        self.execute_read(move |connection| {
            let mut params: Vec<rusqlite::types::Value> = Vec::new();
            let mut extra = String::new();

            let mut restrict = |filter_column: &str, value: rusqlite::types::Value| {
                params.push(value);
                extra.push_str(&format!(" AND {filter_column} = ?"));
            };
            if let Some(artist) = &filter.artist {
                restrict("artist", rusqlite::types::Value::Text(artist.clone()));
            }
            if let Some(album_artist) = &filter.album_artist {
                restrict(
                    "album_artist",
                    rusqlite::types::Value::Text(album_artist.clone()),
                );
            }
            if let Some(album) = &filter.album {
                restrict("album", rusqlite::types::Value::Text(album.clone()));
            }
            if let Some(genre) = &filter.genre {
                restrict("genre", rusqlite::types::Value::Text(genre.clone()));
            }
            if let Some(year) = filter.year {
                restrict("year", rusqlite::types::Value::Integer(i64::from(year)));
            }

            // An empty tag is as absent as a missing one; neither should
            // produce a browsable container.
            //
            // `MIN(id)` picks a stable representative for the container's cover
            // art. It costs nothing extra: the grouping has already visited
            // every row.
            let sql = format!(
                "SELECT {column} AS label, COUNT(*) AS total, MIN(media_files.id) AS sample \
                 FROM media_files \
                 WHERE {column} IS NOT NULL AND {column} <> ''{extra} \
                 GROUP BY label ORDER BY label COLLATE natural_order"
            );
            let mut statement = connection.prepare_cached(&sql)?;
            let categories = statement
                .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                    let name: String = row.get::<_, rusqlite::types::Value>(0).map(stringify)?;
                    let count: i64 = row.get(1)?;
                    Ok(MusicCategory {
                        id: name.clone(),
                        name,
                        category_type: kind.clone(),
                        count: count.max(0) as usize,
                        sample_id: row.get(2)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(categories)
        })
        .await
    }

    pub(in crate::database::sqlite) async fn get_music_categories_impl(
        &self,
        kind: MusicCategoryType,
        filter: &MusicCategoryFilter,
    ) -> Result<Vec<MusicCategory>> {
        self.categories(kind, filter.clone()).await
    }

    pub(in crate::database::sqlite) async fn get_artists_impl(&self) -> Result<Vec<MusicCategory>> {
        self.categories(MusicCategoryType::Artist, MusicCategoryFilter::default())
            .await
    }

    pub(in crate::database::sqlite) async fn get_albums_impl(
        &self,
        artist_filter: Option<&str>,
    ) -> Result<Vec<MusicCategory>> {
        let filter = match artist_filter {
            Some(artist) => MusicCategoryFilter::artist(artist),
            None => MusicCategoryFilter::default(),
        };
        self.categories(MusicCategoryType::Album, filter).await
    }

    pub(in crate::database::sqlite) async fn get_genres_impl(&self) -> Result<Vec<MusicCategory>> {
        self.categories(MusicCategoryType::Genre, MusicCategoryFilter::default())
            .await
    }

    pub(in crate::database::sqlite) async fn get_years_impl(&self) -> Result<Vec<MusicCategory>> {
        self.categories(MusicCategoryType::Year, MusicCategoryFilter::default())
            .await
    }

    pub(in crate::database::sqlite) async fn get_album_artists_impl(
        &self,
    ) -> Result<Vec<MusicCategory>> {
        self.categories(MusicCategoryType::AlbumArtist, MusicCategoryFilter::default())
            .await
    }

    pub(in crate::database::sqlite) async fn get_music_by_artist_impl(
        &self,
        artist: &str,
    ) -> Result<Vec<MediaFile>> {
        self.query_media(MediaFileQuery::Artist(artist.to_owned()))
            .await
    }

    pub(in crate::database::sqlite) async fn get_music_by_album_impl(
        &self,
        album: &str,
        artist: Option<&str>,
    ) -> Result<Vec<MediaFile>> {
        self.query_media(MediaFileQuery::Album {
            album: album.to_owned(),
            artist: artist.map(str::to_owned),
        })
        .await
    }

    pub(in crate::database::sqlite) async fn get_music_by_genre_impl(
        &self,
        genre: &str,
    ) -> Result<Vec<MediaFile>> {
        self.query_media(MediaFileQuery::Genre(genre.to_owned()))
            .await
    }

    pub(in crate::database::sqlite) async fn get_music_by_year_impl(
        &self,
        year: u32,
    ) -> Result<Vec<MediaFile>> {
        self.query_media(MediaFileQuery::Year(year)).await
    }

    pub(in crate::database::sqlite) async fn get_music_by_album_artist_impl(
        &self,
        album_artist: &str,
    ) -> Result<Vec<MediaFile>> {
        self.query_media(MediaFileQuery::AlbumArtist(album_artist.to_owned()))
            .await
    }
}

/// Render a category label as text.
///
/// Year is stored as an integer while the other categories are strings, and
/// the DLNA layer wants a name for all of them alike.
fn stringify(value: rusqlite::types::Value) -> String {
    match value {
        rusqlite::types::Value::Text(text) => text,
        rusqlite::types::Value::Integer(number) => number.to_string(),
        rusqlite::types::Value::Real(number) => number.to_string(),
        _ => String::new(),
    }
}

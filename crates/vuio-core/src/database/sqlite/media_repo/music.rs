//! Music categorization.
//!
//! Every category here is a `GROUP BY` over an indexed column. The equivalent
//! in a key-value store is a secondary index table maintained by hand on every
//! write; the point of moving to SQL is that these need no maintenance at all.

use anyhow::Result;

use crate::database::sqlite::SqliteDatabase;
use crate::database::{MediaFile, MediaFileQuery, MusicCategory, MusicCategoryType};

impl SqliteDatabase {
    /// Distinct values of one tag column, with the number of records carrying each.
    async fn categories(
        &self,
        column: &'static str,
        category_type: MusicCategoryType,
        filter: Option<(&'static str, String)>,
    ) -> Result<Vec<MusicCategory>> {
        self.execute_read(move |connection| {
            let mut params: Vec<rusqlite::types::Value> = Vec::new();
            let extra = match &filter {
                Some((filter_column, value)) => {
                    params.push(rusqlite::types::Value::Text(value.clone()));
                    format!(" AND {filter_column} = ?")
                }
                None => String::new(),
            };

            // An empty tag is as absent as a missing one; neither should
            // produce a browsable container.
            let sql = format!(
                "SELECT {column} AS label, COUNT(*) AS total FROM media_files \
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
                        category_type: category_type.clone(),
                        count: count.max(0) as usize,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(categories)
        })
        .await
    }

    pub(in crate::database::sqlite) async fn get_artists_impl(&self) -> Result<Vec<MusicCategory>> {
        self.categories("artist", MusicCategoryType::Artist, None)
            .await
    }

    pub(in crate::database::sqlite) async fn get_albums_impl(
        &self,
        artist_filter: Option<&str>,
    ) -> Result<Vec<MusicCategory>> {
        self.categories(
            "album",
            MusicCategoryType::Album,
            artist_filter.map(|artist| ("artist", artist.to_owned())),
        )
        .await
    }

    pub(in crate::database::sqlite) async fn get_genres_impl(&self) -> Result<Vec<MusicCategory>> {
        self.categories("genre", MusicCategoryType::Genre, None)
            .await
    }

    pub(in crate::database::sqlite) async fn get_years_impl(&self) -> Result<Vec<MusicCategory>> {
        self.categories("year", MusicCategoryType::Year, None).await
    }

    pub(in crate::database::sqlite) async fn get_album_artists_impl(
        &self,
    ) -> Result<Vec<MusicCategory>> {
        self.categories("album_artist", MusicCategoryType::AlbumArtist, None)
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

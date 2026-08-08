use super::super::*;

impl RedbDatabase {
    pub(in crate::database::redb) async fn get_artists_impl(&self) -> Result<Vec<MusicCategory>> {
        self.execute_read(|database| {
            let read_txn = database.begin_read()?;
            let artist_index = read_txn.open_multimap_table(ARTIST_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut categories = Vec::new();
            for result in artist_index.iter()? {
                let (key, value) = result?;
                let artist_name = key.value().to_string();
                let mut count = 0;
                for id in value {
                    if files_table.get(id?.value())?.is_some() {
                        count += 1;
                    }
                }
                if count == 0 {
                    continue;
                }
                categories.push(MusicCategory {
                    id: artist_name.clone(),
                    name: artist_name,
                    category_type: MusicCategoryType::Artist,
                    count,
                });
            }
            Ok(categories)
        })
        .await
    }

    pub(in crate::database::redb) async fn get_albums_impl(
        &self,
        artist_filter: Option<&str>,
    ) -> Result<Vec<MusicCategory>> {
        let artist_filter = artist_filter.map(str::to_owned);
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let album_index = read_txn.open_multimap_table(ALBUM_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut categories = Vec::new();
            for result in album_index.iter()? {
                let (key, value) = result?;
                let album_name = key.value().to_string();
                let file_ids = value
                    .map(|id| id.map(|id| id.value()))
                    .collect::<std::result::Result<Vec<_>, _>>()?;

                let count = if let Some(artist) = artist_filter.as_deref() {
                    let mut matched = 0;
                    for fid in file_ids {
                        if let Some(data) = files_table.get(fid)? {
                            let file = RedbReadSession::view(data.value())?;
                            if file.artist() == Some(artist) {
                                matched += 1;
                            }
                        }
                    }
                    matched
                } else {
                    let mut existing = 0;
                    for id in file_ids {
                        if files_table.get(id)?.is_some() {
                            existing += 1;
                        }
                    }
                    existing
                };

                if count > 0 {
                    categories.push(MusicCategory {
                        id: album_name.clone(),
                        name: album_name,
                        category_type: MusicCategoryType::Album,
                        count,
                    });
                }
            }
            Ok(categories)
        })
        .await
    }

    pub(in crate::database::redb) async fn get_genres_impl(&self) -> Result<Vec<MusicCategory>> {
        self.execute_read(|database| {
            let read_txn = database.begin_read()?;
            let genre_index = read_txn.open_multimap_table(GENRE_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut categories = Vec::new();
            for result in genre_index.iter()? {
                let (key, value) = result?;
                let name = key.value().to_string();
                let mut count = 0;
                for id in value {
                    if files_table.get(id?.value())?.is_some() {
                        count += 1;
                    }
                }
                if count == 0 {
                    continue;
                }
                categories.push(MusicCategory {
                    id: name.clone(),
                    name,
                    category_type: MusicCategoryType::Genre,
                    count,
                });
            }
            Ok(categories)
        })
        .await
    }

    pub(in crate::database::redb) async fn get_years_impl(&self) -> Result<Vec<MusicCategory>> {
        self.execute_read(|database| {
            let read_txn = database.begin_read()?;
            let year_index = read_txn.open_multimap_table(YEAR_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut categories = Vec::new();
            for result in year_index.iter()? {
                let (key, value) = result?;
                let year = key.value();
                let mut count = 0;
                for id in value {
                    if files_table.get(id?.value())?.is_some() {
                        count += 1;
                    }
                }
                if count == 0 {
                    continue;
                }
                categories.push(MusicCategory {
                    id: year.to_string(),
                    name: year.to_string(),
                    category_type: MusicCategoryType::Year,
                    count,
                });
            }
            Ok(categories)
        })
        .await
    }

    pub(in crate::database::redb) async fn get_album_artists_impl(
        &self,
    ) -> Result<Vec<MusicCategory>> {
        self.execute_read(|database| {
            let read_txn = database.begin_read()?;
            let album_artist_index = read_txn.open_multimap_table(ALBUM_ARTIST_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut categories = Vec::new();
            for result in album_artist_index.iter()? {
                let (key, value) = result?;
                let name = key.value().to_string();
                let mut count = 0;
                for id in value {
                    if files_table.get(id?.value())?.is_some() {
                        count += 1;
                    }
                }
                if count == 0 {
                    continue;
                }
                categories.push(MusicCategory {
                    id: name.clone(),
                    name,
                    category_type: MusicCategoryType::AlbumArtist,
                    count,
                });
            }
            Ok(categories)
        })
        .await
    }

    pub(in crate::database::redb) async fn get_music_by_artist_impl(
        &self,
        artist: &str,
    ) -> Result<Vec<MediaFile>> {
        let artist = artist.to_owned();
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let artist_index = read_txn.open_multimap_table(ARTIST_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut files = Vec::new();
            for fid in artist_index.get(artist.as_str())? {
                let fid = fid?.value();
                if let Some(data) = files_table.get(fid)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
            Ok(files)
        })
        .await
    }

    pub(in crate::database::redb) async fn get_music_by_album_impl(
        &self,
        album: &str,
        artist: Option<&str>,
    ) -> Result<Vec<MediaFile>> {
        let album = album.to_owned();
        let artist = artist.map(str::to_owned);
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let album_index = read_txn.open_multimap_table(ALBUM_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut files = Vec::new();
            for fid in album_index.get(album.as_str())? {
                let fid = fid?.value();
                if let Some(data) = files_table.get(fid)? {
                    let file = Self::deserialize_media_file(data.value())?;
                    if let Some(art) = artist.as_deref() {
                        if file.artist.as_deref() != Some(art) {
                            continue;
                        }
                    }
                    files.push(file);
                }
            }
            Ok(files)
        })
        .await
    }

    pub(in crate::database::redb) async fn get_music_by_genre_impl(
        &self,
        genre: &str,
    ) -> Result<Vec<MediaFile>> {
        let genre = genre.to_owned();
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let genre_index = read_txn.open_multimap_table(GENRE_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut files = Vec::new();
            for fid in genre_index.get(genre.as_str())? {
                let fid = fid?.value();
                if let Some(data) = files_table.get(fid)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
            Ok(files)
        })
        .await
    }

    pub(in crate::database::redb) async fn get_music_by_year_impl(
        &self,
        year: u32,
    ) -> Result<Vec<MediaFile>> {
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let year_index = read_txn.open_multimap_table(YEAR_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut files = Vec::new();
            for fid in year_index.get(year)? {
                let fid = fid?.value();
                if let Some(data) = files_table.get(fid)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
            Ok(files)
        })
        .await
    }

    pub(in crate::database::redb) async fn get_music_by_album_artist_impl(
        &self,
        album_artist: &str,
    ) -> Result<Vec<MediaFile>> {
        let album_artist = album_artist.to_owned();
        self.execute_read(move |database| {
            let read_txn = database.begin_read()?;
            let album_artist_index = read_txn.open_multimap_table(ALBUM_ARTIST_INDEX)?;
            let files_table = read_txn.open_table(FILES_TABLE)?;

            let mut files = Vec::new();
            for fid in album_artist_index.get(album_artist.as_str())? {
                let fid = fid?.value();
                if let Some(data) = files_table.get(fid)? {
                    files.push(Self::deserialize_media_file(data.value())?);
                }
            }
            Ok(files)
        })
        .await
    }
}

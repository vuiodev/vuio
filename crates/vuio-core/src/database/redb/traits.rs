use super::*;

#[async_trait]
impl MediaRepository for RedbDatabase {
    type ReadSession = RedbReadSession;

    async fn read<R, F>(self: Arc<Self>, operation: F) -> Result<R>
    where
        R: Send + 'static,
        F: FnOnce(&mut Self::ReadSession) -> Result<R> + Send + 'static,
    {
        RedbDatabase::read_impl(self, operation).await
    }

    async fn store_media_file(&self, file: &MediaFile) -> Result<i64> {
        RedbDatabase::store_media_file_impl(self, file).await
    }

    fn stream_all_media_files(
        &self,
    ) -> Pin<Box<dyn futures_util::Stream<Item = Result<MediaFile, DatabaseError>> + Send + '_>>
    {
        RedbDatabase::stream_all_media_files_impl(self)
    }

    async fn remove_media_file(&self, path: &Path) -> Result<bool> {
        RedbDatabase::remove_media_file_impl(self, path).await
    }

    async fn update_media_file(&self, file: &MediaFile) -> Result<()> {
        RedbDatabase::update_media_file_impl(self, file).await
    }

    async fn get_files_in_directory(&self, dir: &Path) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_files_in_directory_impl(self, dir).await
    }

    async fn get_directory_listing(
        &self,
        parent_path: &Path,
        media_type_filter: &str,
    ) -> Result<(Vec<MediaDirectory>, Vec<MediaFile>)> {
        RedbDatabase::get_directory_listing_impl(self, parent_path, media_type_filter).await
    }

    async fn cleanup_missing_files(&self, existing_paths: &[PathBuf]) -> Result<usize> {
        RedbDatabase::cleanup_missing_files_impl(self, existing_paths).await
    }

    async fn get_file_by_path(&self, path: &Path) -> Result<Option<MediaFile>> {
        RedbDatabase::get_file_by_path_impl(self, path).await
    }

    async fn get_file_by_id(&self, id: i64) -> Result<Option<MediaFile>> {
        RedbDatabase::get_file_by_id_impl(self, id).await
    }

    async fn get_file_location_by_id(&self, id: i64) -> Result<Option<FileLocation>> {
        RedbDatabase::get_file_location_by_id_impl(self, id).await
    }

    async fn load_file_fingerprints(&self) -> Result<Vec<FileFingerprint>> {
        RedbDatabase::load_file_fingerprints_impl(self).await
    }

    async fn get_root_availability(&self, path: &Path) -> Result<Option<RootAvailability>> {
        RedbDatabase::get_root_availability_impl(self, path).await
    }

    async fn list_root_availability(&self) -> Result<Vec<RootAvailability>> {
        RedbDatabase::list_root_availability_impl(self).await
    }

    async fn set_root_availability(&self, state: &RootAvailability) -> Result<()> {
        RedbDatabase::set_root_availability_impl(self, state).await
    }

    async fn remove_root_availability(&self, path: &Path) -> Result<()> {
        RedbDatabase::remove_root_availability_impl(self, path).await
    }

    async fn get_artists(&self) -> Result<Vec<MusicCategory>> {
        RedbDatabase::get_artists_impl(self).await
    }

    async fn get_albums(&self, artist_filter: Option<&str>) -> Result<Vec<MusicCategory>> {
        RedbDatabase::get_albums_impl(self, artist_filter).await
    }

    async fn get_genres(&self) -> Result<Vec<MusicCategory>> {
        RedbDatabase::get_genres_impl(self).await
    }

    async fn get_years(&self) -> Result<Vec<MusicCategory>> {
        RedbDatabase::get_years_impl(self).await
    }

    async fn get_album_artists(&self) -> Result<Vec<MusicCategory>> {
        RedbDatabase::get_album_artists_impl(self).await
    }

    async fn get_music_by_artist(&self, artist: &str) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_music_by_artist_impl(self, artist).await
    }

    async fn get_music_by_album(
        &self,
        album: &str,
        artist: Option<&str>,
    ) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_music_by_album_impl(self, album, artist).await
    }

    async fn get_music_by_genre(&self, genre: &str) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_music_by_genre_impl(self, genre).await
    }

    async fn get_music_by_year(&self, year: u32) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_music_by_year_impl(self, year).await
    }

    async fn get_music_by_album_artist(&self, album_artist: &str) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_music_by_album_artist_impl(self, album_artist).await
    }

    async fn get_files_by_paths(&self, paths: &[PathBuf]) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_files_by_paths_impl(self, paths).await
    }

    async fn bulk_store_media_files(&self, files: &[MediaFile]) -> Result<Vec<i64>> {
        RedbDatabase::bulk_store_media_files_impl(self, files).await
    }

    async fn bulk_store_canonical_media_files(&self, files: &[MediaFile]) -> Result<Vec<i64>> {
        RedbDatabase::bulk_store_canonical_media_files_impl(self, files).await
    }

    async fn bulk_update_media_files(&self, files: &[MediaFile]) -> Result<()> {
        RedbDatabase::bulk_update_media_files_impl(self, files).await
    }

    async fn bulk_update_canonical_media_files(&self, files: &[MediaFile]) -> Result<()> {
        RedbDatabase::bulk_update_canonical_media_files_impl(self, files).await
    }

    async fn bulk_remove_media_files(&self, paths: &[PathBuf]) -> Result<usize> {
        RedbDatabase::bulk_remove_media_files_impl(self, paths).await
    }

    async fn remove_media_under_path(&self, path: &Path) -> Result<RemovalSummary> {
        RedbDatabase::remove_media_under_path_impl(self, path).await
    }

    async fn get_files_with_path_prefix(&self, canonical_prefix: &str) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_files_with_path_prefix_impl(self, canonical_prefix).await
    }

    async fn get_direct_subdirectories(
        &self,
        canonical_parent_path: &str,
    ) -> Result<Vec<MediaDirectory>> {
        RedbDatabase::get_direct_subdirectories_impl(self, canonical_parent_path).await
    }

    async fn batch_cleanup_missing_files(
        &self,
        existing_canonical_paths: &HashSet<String>,
    ) -> Result<usize> {
        RedbDatabase::batch_cleanup_missing_files_impl(self, existing_canonical_paths).await
    }

    async fn database_native_cleanup(&self, existing_canonical_paths: &[String]) -> Result<usize> {
        RedbDatabase::database_native_cleanup_impl(self, existing_canonical_paths).await
    }

    async fn get_filtered_direct_subdirectories(
        &self,
        canonical_parent_path: &str,
        mime_filter: &str,
    ) -> Result<Vec<MediaDirectory>> {
        RedbDatabase::get_filtered_direct_subdirectories_impl(
            self,
            canonical_parent_path,
            mime_filter,
        )
        .await
    }
}

#[async_trait]
impl PlaylistRepository for RedbDatabase {
    async fn create_playlist(&self, name: &str, description: Option<&str>) -> Result<i64> {
        RedbDatabase::create_playlist_impl(self, name, description).await
    }

    async fn get_playlists(&self) -> Result<Vec<Playlist>> {
        RedbDatabase::get_playlists_impl(self).await
    }

    async fn get_playlist(&self, playlist_id: i64) -> Result<Option<Playlist>> {
        RedbDatabase::get_playlist_impl(self, playlist_id).await
    }

    async fn update_playlist(&self, playlist: &Playlist) -> Result<()> {
        RedbDatabase::update_playlist_impl(self, playlist).await
    }

    async fn delete_playlist(&self, playlist_id: i64) -> Result<bool> {
        RedbDatabase::delete_playlist_impl(self, playlist_id).await
    }

    async fn set_playlist_source(&self, playlist_id: i64, source_path: &Path) -> Result<()> {
        RedbDatabase::set_playlist_source_impl(self, playlist_id, source_path).await
    }

    async fn replace_playlist_from_source(
        &self,
        source_path: &Path,
        name: &str,
        media_file_ids: &[(i64, u32)],
    ) -> Result<i64> {
        RedbDatabase::replace_playlist_from_source_impl(self, source_path, name, media_file_ids)
            .await
    }

    async fn remove_derived_content_by_source(&self, source_path: &Path) -> Result<usize> {
        RedbDatabase::remove_derived_content_by_source_impl(self, source_path).await
    }

    async fn add_to_playlist(
        &self,
        playlist_id: i64,
        media_file_id: i64,
        position: Option<u32>,
    ) -> Result<i64> {
        RedbDatabase::add_to_playlist_impl(self, playlist_id, media_file_id, position).await
    }

    async fn batch_add_to_playlist(
        &self,
        playlist_id: i64,
        media_file_ids: &[(i64, u32)],
    ) -> Result<Vec<i64>> {
        RedbDatabase::batch_add_to_playlist_impl(self, playlist_id, media_file_ids).await
    }

    async fn remove_from_playlist(&self, playlist_id: i64, media_file_id: i64) -> Result<bool> {
        RedbDatabase::remove_from_playlist_impl(self, playlist_id, media_file_id).await
    }

    async fn get_playlist_tracks(&self, playlist_id: i64) -> Result<Vec<MediaFile>> {
        RedbDatabase::get_playlist_tracks_impl(self, playlist_id).await
    }

    async fn reorder_playlist(
        &self,
        playlist_id: i64,
        track_positions: &[(i64, u32)],
    ) -> Result<()> {
        RedbDatabase::reorder_playlist_impl(self, playlist_id, track_positions).await
    }
}

#[async_trait]
impl HealthRepository for RedbDatabase {
    async fn check_and_repair(&self) -> Result<DatabaseHealth> {
        RedbDatabase::check_and_repair_impl(self).await
    }

    async fn create_backup(&self, backup_path: &Path) -> Result<()> {
        RedbDatabase::create_backup_impl(self, backup_path).await
    }

    async fn vacuum(&self) -> Result<bool> {
        RedbDatabase::vacuum_impl(self).await
    }

    async fn rebuild_derived_indexes(&self) -> Result<DatabaseHealth> {
        RedbDatabase::rebuild_derived_indexes_impl(self).await
    }
}

#[async_trait]
impl StatsRepository for RedbDatabase {
    async fn get_stats(&self) -> Result<DatabaseStats> {
        RedbDatabase::get_stats_impl(self).await
    }
}

#[async_trait]
impl SecretStore for RedbDatabase {
    async fn get_secret(&self, key: &str) -> Result<Option<Vec<u8>>> {
        RedbDatabase::get_secret_impl(self, key).await
    }

    async fn set_secret(&self, key: &str, value: &[u8]) -> Result<()> {
        RedbDatabase::set_secret_impl(self, key, value).await
    }

    async fn delete_secret(&self, key: &str) -> Result<bool> {
        RedbDatabase::delete_secret_impl(self, key).await
    }
}

#[async_trait]
impl DatabaseManager for RedbDatabase {
    async fn initialize(&self) -> Result<()> {
        info!("RedbDatabase initialized");
        Ok(())
    }
}

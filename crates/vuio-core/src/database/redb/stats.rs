//! Database statistics.

use super::*;

impl RedbDatabase {
    pub(super) async fn get_stats_impl(&self) -> Result<DatabaseStats> {
        let total_files = self.total_files.load(Ordering::SeqCst) as usize;
        let total_size = self.total_size.load(Ordering::SeqCst);
        let database_size = tokio::fs::metadata(&self.db_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        let (video_files, audio_files, image_files, playlists) = self
            .execute_read(|database| {
                let transaction = database.begin_read()?;
                let files = transaction.open_table(FILES_TABLE)?;
                let mut video = 0;
                let mut audio = 0;
                let mut image = 0;
                for entry in files.iter()? {
                    let (_, bytes) = entry?;
                    let view = RedbReadSession::view(bytes.value())?;
                    if view.mime_type().starts_with("video/") {
                        video += 1;
                    } else if view.mime_type().starts_with("audio/") {
                        audio += 1;
                    } else if view.mime_type().starts_with("image/") {
                        image += 1;
                    }
                }
                let playlists = transaction.open_table(PLAYLISTS_TABLE)?.iter()?.count();
                Ok((video, audio, image, playlists))
            })
            .await?;

        Ok(DatabaseStats {
            total_files,
            total_size,
            database_size,
            video_files,
            audio_files,
            image_files,
            playlists,
        })
    }
}

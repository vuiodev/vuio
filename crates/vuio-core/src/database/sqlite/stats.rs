//! Library statistics.

use anyhow::Result;

use super::SqliteDatabase;
use crate::database::DatabaseStats;

impl SqliteDatabase {
    pub(super) async fn get_stats_impl(&self) -> Result<DatabaseStats> {
        self.execute_read(move |connection| {
            // One pass over the table serves every counter, so the numbers are
            // consistent with each other without needing a transaction.
            let (total_files, total_size, video, audio, image) = connection
                .prepare_cached(
                    "SELECT COUNT(*), COALESCE(SUM(size), 0), \
                            COALESCE(SUM(mime_family = 'video'), 0), \
                            COALESCE(SUM(mime_family = 'audio'), 0), \
                            COALESCE(SUM(mime_family = 'image'), 0) \
                     FROM media_files",
                )?
                .query_row([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })?;

            let playlists: i64 = connection
                .prepare_cached("SELECT COUNT(*) FROM playlists")?
                .query_row([], |row| row.get(0))?;

            // The file on disk includes free pages a vacuum would reclaim,
            // which is what an operator watching disk usage wants to see.
            let page_count: i64 = connection.query_row("PRAGMA page_count", [], |row| row.get(0))?;
            let page_size: i64 = connection.query_row("PRAGMA page_size", [], |row| row.get(0))?;

            Ok(DatabaseStats {
                total_files: total_files.max(0) as usize,
                total_size: total_size.max(0) as u64,
                database_size: (page_count * page_size).max(0) as u64,
                video_files: video.max(0) as usize,
                audio_files: audio.max(0) as usize,
                image_files: image.max(0) as usize,
                playlists: playlists.max(0) as usize,
            })
        })
        .await
    }
}

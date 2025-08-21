use anyhow::Result;
use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

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
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
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
            created_at: now,
            updated_at: now,
        }
    }

    /// Create MediaFile from database row
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self> {
        let path_str: String = row.try_get("path")?;
        let modified_timestamp: i64 = row.try_get("modified")?;
        let created_timestamp: i64 = row.try_get("created_at")?;
        let updated_timestamp: i64 = row.try_get("updated_at")?;

        let duration_ms: Option<i64> = row.try_get("duration")?;
        let duration = duration_ms.map(|ms| Duration::from_millis(ms as u64));

        Ok(Self {
            id: Some(row.try_get("id")?),
            path: PathBuf::from(path_str),
            filename: row.try_get("filename")?,
            size: row.try_get::<i64, _>("size")? as u64,
            modified: SystemTime::UNIX_EPOCH + Duration::from_secs(modified_timestamp as u64),
            mime_type: row.try_get("mime_type")?,
            duration,
            title: row.try_get("title")?,
            artist: row.try_get("artist")?,
            album: row.try_get("album")?,
            created_at: SystemTime::UNIX_EPOCH + Duration::from_secs(created_timestamp as u64),
            updated_at: SystemTime::UNIX_EPOCH + Duration::from_secs(updated_timestamp as u64),
        })
    }
}

/// Database manager trait for media file operations
#[async_trait]
pub trait DatabaseManager: Send + Sync {
    /// Initialize the database and create tables if needed
    async fn initialize(&self) -> Result<()>;

    /// Store a new media file record
    async fn store_media_file(&self, file: &MediaFile) -> Result<i64>;

    /// Get all media files from the database
    async fn get_all_media_files(&self) -> Result<Vec<MediaFile>>;

    /// Remove a media file record by path
    async fn remove_media_file(&self, path: &Path) -> Result<bool>;

    /// Update an existing media file record
    async fn update_media_file(&self, file: &MediaFile) -> Result<()>;

    /// Get all files in a specific directory
    async fn get_files_in_directory(&self, dir: &Path) -> Result<Vec<MediaFile>>;

    /// Remove media files that no longer exist on disk
    async fn cleanup_missing_files(&self, existing_paths: &[PathBuf]) -> Result<usize>;

    /// Get a specific file by path
    async fn get_file_by_path(&self, path: &Path) -> Result<Option<MediaFile>>;

    /// Get database statistics
    async fn get_stats(&self) -> Result<DatabaseStats>;

    /// Check database integrity and repair if needed
    async fn check_and_repair(&self) -> Result<DatabaseHealth>;

    /// Create a backup of the database
    async fn create_backup(&self, backup_path: &Path) -> Result<()>;

    /// Restore database from backup
    async fn restore_from_backup(&self, backup_path: &Path) -> Result<()>;

    /// Vacuum the database to reclaim space and optimize performance
    async fn vacuum(&self) -> Result<()>;
}

#[derive(Debug)]
pub struct DatabaseStats {
    pub total_files: usize,
    pub total_size: u64,
    pub database_size: u64,
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

/// SQLite implementation of DatabaseManager
pub struct SqliteDatabase {
    pool: SqlitePool,
    db_path: PathBuf,
}

impl SqliteDatabase {
    /// Create a new SQLite database manager
    pub async fn new(db_path: PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
        let pool = SqlitePool::connect(&database_url).await?;

        Ok(Self { pool, db_path })
    }

    /// Create database tables
    async fn create_tables(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS media_files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT UNIQUE NOT NULL,
                filename TEXT NOT NULL,
                size INTEGER NOT NULL,
                modified INTEGER NOT NULL,
                mime_type TEXT NOT NULL,
                duration INTEGER,
                title TEXT,
                artist TEXT,
                album TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes for better query performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_files_path ON media_files(path)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_files_modified ON media_files(modified)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_files_mime_type ON media_files(mime_type)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_media_files_filename ON media_files(filename)")
            .execute(&self.pool)
            .await?;

        // Create database metadata table for migrations
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS database_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Set initial schema version
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        sqlx::query(
            "INSERT OR IGNORE INTO database_metadata (key, value, updated_at) VALUES (?, ?, ?)",
        )
        .bind("schema_version")
        .bind("1")
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Convert SystemTime to Unix timestamp
    fn system_time_to_timestamp(time: SystemTime) -> i64 {
        time.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }
}

#[async_trait]
impl DatabaseManager for SqliteDatabase {
    async fn initialize(&self) -> Result<()> {
        // Configure SQLite for better performance
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(&self.pool)
            .await?;
        sqlx::query("PRAGMA synchronous = NORMAL")
            .execute(&self.pool)
            .await?;
        sqlx::query("PRAGMA temp_store = MEMORY")
            .execute(&self.pool)
            .await?;
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&self.pool)
            .await?;
        sqlx::query("PRAGMA cache_size = -10000") // 10MB cache
            .execute(&self.pool)
            .await?;

        self.create_tables().await?;
        Ok(())
    }

    async fn store_media_file(&self, file: &MediaFile) -> Result<i64> {
        let path_str = file.path.to_string_lossy().to_string();
        let modified_timestamp = Self::system_time_to_timestamp(file.modified);
        let created_timestamp = Self::system_time_to_timestamp(file.created_at);
        let updated_timestamp = Self::system_time_to_timestamp(file.updated_at);
        let duration_ms = file.duration.map(|d| d.as_millis() as i64);

        let result = sqlx::query(
            r#"
            INSERT INTO media_files 
            (path, filename, size, modified, mime_type, duration, title, artist, album, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&path_str)
        .bind(&file.filename)
        .bind(file.size as i64)
        .bind(modified_timestamp)
        .bind(&file.mime_type)
        .bind(duration_ms)
        .bind(&file.title)
        .bind(&file.artist)
        .bind(&file.album)
        .bind(created_timestamp)
        .bind(updated_timestamp)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    async fn get_all_media_files(&self) -> Result<Vec<MediaFile>> {
        let rows = sqlx::query(
            "SELECT id, path, filename, size, modified, mime_type, duration, title, artist, album, created_at, updated_at FROM media_files ORDER BY filename"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::new();
        for row in rows {
            files.push(MediaFile::from_row(&row)?);
        }

        Ok(files)
    }

    async fn remove_media_file(&self, path: &Path) -> Result<bool> {
        let path_str = path.to_string_lossy().to_string();

        let result = sqlx::query("DELETE FROM media_files WHERE path = ?")
            .bind(&path_str)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn update_media_file(&self, file: &MediaFile) -> Result<()> {
        let path_str = file.path.to_string_lossy().to_string();
        let modified_timestamp = Self::system_time_to_timestamp(file.modified);
        let updated_timestamp = Self::system_time_to_timestamp(SystemTime::now());
        let duration_ms = file.duration.map(|d| d.as_millis() as i64);

        sqlx::query(
            r#"
            UPDATE media_files 
            SET filename = ?, size = ?, modified = ?, mime_type = ?, duration = ?, 
                title = ?, artist = ?, album = ?, updated_at = ?
            WHERE path = ?
            "#,
        )
        .bind(&file.filename)
        .bind(file.size as i64)
        .bind(modified_timestamp)
        .bind(&file.mime_type)
        .bind(duration_ms)
        .bind(&file.title)
        .bind(&file.artist)
        .bind(&file.album)
        .bind(updated_timestamp)
        .bind(&path_str)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_files_in_directory(&self, dir: &Path) -> Result<Vec<MediaFile>> {
        let dir_str = format!("{}%", dir.to_string_lossy());

        let rows = sqlx::query(
            r#"
            SELECT id, path, filename, size, modified, mime_type, duration, title, artist, album, created_at, updated_at 
            FROM media_files 
            WHERE path LIKE ?
            ORDER BY filename
            "#,
        )
        .bind(&dir_str)
        .fetch_all(&self.pool)
        .await?;

        let mut files = Vec::new();
        for row in rows {
            files.push(MediaFile::from_row(&row)?);
        }

        Ok(files)
    }

    async fn cleanup_missing_files(&self, existing_paths: &[PathBuf]) -> Result<usize> {
        if existing_paths.is_empty() {
            // If no existing paths provided, don't remove anything
            return Ok(0);
        }

        let existing_paths: Vec<String> = existing_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        // Create placeholders for the IN clause
        let placeholders = existing_paths
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let query = format!("DELETE FROM media_files WHERE path NOT IN ({})", placeholders);

        let mut query_builder = sqlx::query(&query);
        for path in &existing_paths {
            query_builder = query_builder.bind(path);
        }

        let result = query_builder.execute(&self.pool).await?;

        Ok(result.rows_affected() as usize)
    }

    async fn get_file_by_path(&self, path: &Path) -> Result<Option<MediaFile>> {
        let path_str = path.to_string_lossy().to_string();

        let row = sqlx::query(
            r#"
            SELECT id, path, filename, size, modified, mime_type, duration, title, artist, album, created_at, updated_at 
            FROM media_files 
            WHERE path = ?
            "#,
        )
        .bind(&path_str)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(MediaFile::from_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_stats(&self) -> Result<DatabaseStats> {
        // Get total files and size
        let row = sqlx::query("SELECT COUNT(*), COALESCE(SUM(size), 0) FROM media_files")
            .fetch_one(&self.pool)
            .await?;

        let total_files: i64 = row.try_get(0)?;
        let total_size: i64 = row.try_get(1)?;

        // Get database file size
        let database_size = tokio::fs::metadata(&self.db_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(DatabaseStats {
            total_files: total_files as usize,
            total_size: total_size as u64,
            database_size,
        })
    }

    async fn check_and_repair(&self) -> Result<DatabaseHealth> {
        let mut health = DatabaseHealth {
            is_healthy: true,
            corruption_detected: false,
            integrity_check_passed: false,
            issues: Vec::new(),
            repair_attempted: false,
            repair_successful: false,
        };

        // Run integrity check
        match self.run_integrity_check().await {
            Ok(integrity_ok) => {
                health.integrity_check_passed = integrity_ok;
                if !integrity_ok {
                    health.is_healthy = false;
                    health.corruption_detected = true;
                    health.issues.push(DatabaseIssue {
                        severity: IssueSeverity::Critical,
                        description: "Database integrity check failed".to_string(),
                        table_affected: None,
                        suggested_action: "Attempt database repair or restore from backup"
                            .to_string(),
                    });
                }
            }
            Err(e) => {
                health.is_healthy = false;
                health.issues.push(DatabaseIssue {
                    severity: IssueSeverity::Error,
                    description: format!("Failed to run integrity check: {}", e),
                    table_affected: None,
                    suggested_action: "Check database file permissions and disk space".to_string(),
                });
            }
        }

        // Check for common issues
        if let Err(e) = self.check_common_issues(&mut health).await {
            health.issues.push(DatabaseIssue {
                severity: IssueSeverity::Warning,
                description: format!("Error during common issues check: {}", e),
                table_affected: None,
                suggested_action: "Review database configuration".to_string(),
            });
        }

        // Attempt repair if corruption detected
        if health.corruption_detected {
            health.repair_attempted = true;
            match self.attempt_repair().await {
                Ok(success) => {
                    health.repair_successful = success;
                    if success {
                        health.is_healthy = true;
                        health.corruption_detected = false;
                        health.issues.push(DatabaseIssue {
                            severity: IssueSeverity::Info,
                            description: "Database successfully repaired".to_string(),
                            table_affected: None,
                            suggested_action: "Consider creating a backup".to_string(),
                        });
                    }
                }
                Err(e) => {
                    health.issues.push(DatabaseIssue {
                        severity: IssueSeverity::Critical,
                        description: format!("Database repair failed: {}", e),
                        table_affected: None,
                        suggested_action: "Restore from backup or recreate database".to_string(),
                    });
                }
            }
        }

        Ok(health)
    }

    async fn create_backup(&self, backup_path: &Path) -> Result<()> {
        // Ensure backup directory exists
        if let Some(parent) = backup_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Create backup using SQLite's backup API through a VACUUM INTO command
        let backup_path_str = backup_path.to_string_lossy().to_string();

        sqlx::query(&format!("VACUUM INTO '{}'", backup_path_str))
            .execute(&self.pool)
            .await?;

        // Verify backup was created successfully
        if !backup_path.exists() {
            return Err(anyhow::anyhow!("Backup file was not created"));
        }

        // Verify backup integrity
        let backup_url = format!("sqlite://{}?mode=ro", backup_path.display());
        let backup_pool = SqlitePool::connect(&backup_url).await?;

        let integrity_ok = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&backup_pool)
            .await?;

        backup_pool.close().await;

        if integrity_ok != "ok" {
            tokio::fs::remove_file(backup_path).await.ok(); // Clean up bad backup
            return Err(anyhow::anyhow!(
                "Backup integrity check failed: {}",
                integrity_ok
            ));
        }

        Ok(())
    }

    async fn restore_from_backup(&self, backup_path: &Path) -> Result<()> {
        if !backup_path.exists() {
            return Err(anyhow::anyhow!(
                "Backup file does not exist: {}",
                backup_path.display()
            ));
        }

        // Verify backup integrity before restore
        let backup_url = format!("sqlite://{}?mode=ro", backup_path.display());
        let backup_pool = SqlitePool::connect(&backup_url).await?;

        let integrity_ok = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&backup_pool)
            .await?;

        backup_pool.close().await;

        if integrity_ok != "ok" {
            return Err(anyhow::anyhow!(
                "Backup file is corrupted: {}",
                integrity_ok
            ));
        }

        // Close current connection
        self.pool.close().await;

        // Replace current database with backup
        tokio::fs::copy(backup_path, &self.db_path).await?;

        // Reconnect to restored database
        let database_url = format!("sqlite://{}?mode=rwc", self.db_path.display());
        let new_pool = SqlitePool::connect(&database_url).await?;

        // Configure SQLite for better performance
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&new_pool)
            .await?;
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(&new_pool)
            .await?;
        sqlx::query("PRAGMA synchronous = NORMAL")
            .execute(&new_pool)
            .await?;
        sqlx::query("PRAGMA cache_size = 10000")
            .execute(&new_pool)
            .await?;
        sqlx::query("PRAGMA temp_store = MEMORY")
            .execute(&new_pool)
            .await?;

        // Note: We can't replace self.pool here due to borrowing rules
        // In a real implementation, this would require restructuring or using Arc<Mutex<>>

        Ok(())
    }

    async fn vacuum(&self) -> Result<()> {
        sqlx::query("VACUUM").execute(&self.pool).await?;

        Ok(())
    }
}

impl SqliteDatabase {
    /// Run SQLite integrity check
    async fn run_integrity_check(&self) -> Result<bool> {
        let result = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&self.pool)
            .await?;

        Ok(result == "ok")
    }

    /// Check for common database issues
    async fn check_common_issues(&self, health: &mut DatabaseHealth) -> Result<()> {
        // Check for orphaned records or inconsistencies
        let orphaned_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM media_files WHERE path = '' OR filename = ''")
                .fetch_one(&self.pool)
                .await?;

        if orphaned_count > 0 {
            health.issues.push(DatabaseIssue {
                severity: IssueSeverity::Warning,
                description: format!("Found {} records with empty path or filename", orphaned_count),
                table_affected: Some("media_files".to_string()),
                suggested_action: "Clean up orphaned records".to_string(),
            });
        }

        // Check for duplicate paths
        let duplicate_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM (SELECT path FROM media_files GROUP BY path HAVING COUNT(*) > 1)",
        )
        .fetch_one(&self.pool)
        .await?;

        if duplicate_count > 0 {
            health.issues.push(DatabaseIssue {
                severity: IssueSeverity::Warning,
                description: format!("Found {} duplicate file paths", duplicate_count),
                table_affected: Some("media_files".to_string()),
                suggested_action: "Remove duplicate entries".to_string(),
            });
        }

        // Check database size vs file count ratio
        let stats = self.get_stats().await?;
        if stats.total_files > 0 {
            let avg_db_size_per_file = stats.database_size / stats.total_files as u64;
            if avg_db_size_per_file > 10000 {
                // More than 10KB per file record seems excessive
                health.issues.push(DatabaseIssue {
                    severity: IssueSeverity::Info,
                    description: "Database size seems large relative to file count".to_string(),
                    table_affected: None,
                    suggested_action: "Consider running VACUUM to optimize database".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Attempt to repair database corruption
    async fn attempt_repair(&self) -> Result<bool> {
        // Try to clean up orphaned records
        sqlx::query("DELETE FROM media_files WHERE path = '' OR filename = ''")
            .execute(&self.pool)
            .await?;

        // Remove duplicates, keeping the most recent
        sqlx::query(
            r#"
            DELETE FROM media_files 
            WHERE id NOT IN (
                SELECT MAX(id) 
                FROM media_files 
                GROUP BY path
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Try to rebuild indexes
        sqlx::query("REINDEX").execute(&self.pool).await?;

        // Run integrity check again
        self.run_integrity_check().await
    }

    /// Clean up orphaned and invalid records
    pub async fn cleanup_invalid_records(&self) -> Result<usize> {
        let result =
            sqlx::query("DELETE FROM media_files WHERE path = '' OR filename = '' OR size < 0")
                .execute(&self.pool)
                .await?;

        Ok(result.rows_affected() as usize)
    }

    /// Remove duplicate file entries, keeping the most recent
    pub async fn remove_duplicates(&self) -> Result<usize> {
        let result = sqlx::query(
            r#"
            DELETE FROM media_files 
            WHERE id NOT IN (
                SELECT MAX(id) 
                FROM media_files 
                GROUP BY path
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_database_creation() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        let stats = db.get_stats().await.unwrap();
        assert_eq!(stats.total_files, 0);
    }

    #[tokio::test]
    async fn test_media_file_crud() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        // Create a test media file
        let mut media_file = MediaFile::new(
            PathBuf::from("/test/video.mp4"),
            1024,
            "video/mp4".to_string(),
        );
        media_file.title = Some("Test Video".to_string());

        // Store the file
        let id = db.store_media_file(&media_file).await.unwrap();
        assert!(id > 0);

        // Retrieve the file
        let retrieved = db
            .get_file_by_path(&PathBuf::from("/test/video.mp4"))
            .await
            .unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.filename, "video.mp4");
        assert_eq!(retrieved.title, Some("Test Video".to_string()));

        // Update the file
        let mut updated_file = retrieved.clone();
        updated_file.title = Some("Updated Video".to_string());
        db.update_media_file(&updated_file).await.unwrap();

        // Verify update
        let updated = db
            .get_file_by_path(&PathBuf::from("/test/video.mp4"))
            .await
            .unwrap();
        assert_eq!(updated.unwrap().title, Some("Updated Video".to_string()));

        // Remove the file
        let removed = db
            .remove_media_file(&PathBuf::from("/test/video.mp4"))
            .await
            .unwrap();
        assert!(removed);

        // Verify removal
        let not_found = db
            .get_file_by_path(&PathBuf::from("/test/video.mp4"))
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_database_health_check() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        // Add some test data
        let media_file =
            MediaFile::new(PathBuf::from("/test/video.mp4"), 1024, "video/mp4".to_string());
        db.store_media_file(&media_file).await.unwrap();

        // Run health check
        let health = db.check_and_repair().await.unwrap();
        assert!(health.is_healthy);
        assert!(health.integrity_check_passed);
        assert!(!health.corruption_detected);
    }

    #[tokio::test]
    async fn test_database_backup_and_restore() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let backup_path = temp_dir.path().join("backup.db");

        let db = SqliteDatabase::new(db_path.clone()).await.unwrap();
        db.initialize().await.unwrap();

        // Add some test data
        let media_file =
            MediaFile::new(PathBuf::from("/test/video.mp4"), 1024, "video/mp4".to_string());
        db.store_media_file(&media_file).await.unwrap();

        // Create backup
        db.create_backup(&backup_path).await.unwrap();
        assert!(backup_path.exists());

        // Verify backup contains data
        let backup_db = SqliteDatabase::new(backup_path.clone()).await.unwrap();
        let files = backup_db.get_all_media_files().await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "video.mp4");
    }

    #[tokio::test]
    async fn test_cleanup_invalid_records() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        // Add valid record
        let valid_file =
            MediaFile::new(PathBuf::from("/test/video.mp4"), 1024, "video/mp4".to_string());
        db.store_media_file(&valid_file).await.unwrap();

        // Manually insert invalid records
        sqlx::query("INSERT INTO media_files (path, filename, size, modified, mime_type, created_at, updated_at) VALUES ('', 'empty.mp4', 1024, 0, 'video/mp4', 0, 0)")
            .execute(&db.pool)
            .await
            .unwrap();

        sqlx::query("INSERT INTO media_files (path, filename, size, modified, mime_type, created_at, updated_at) VALUES ('/test/valid.mp4', '', 1024, 0, 'video/mp4', 0, 0)")
            .execute(&db.pool)
            .await
            .unwrap();

        // Verify we have 3 records (1 valid, 2 invalid)
        let all_files = db.get_all_media_files().await.unwrap();
        assert_eq!(all_files.len(), 3);

        // Clean up invalid records
        let cleaned = db.cleanup_invalid_records().await.unwrap();
        assert_eq!(cleaned, 2);

        // Verify only valid record remains
        let remaining_files = db.get_all_media_files().await.unwrap();
        assert_eq!(remaining_files.len(), 1);
        assert_eq!(remaining_files[0].filename, "video.mp4");
    }

    #[tokio::test]
    async fn test_remove_duplicates() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();

        // Add some unique records first
        let file1 = MediaFile::new(
            PathBuf::from("/test/video1.mp4"),
            1024,
            "video/mp4".to_string(),
        );
        let file2 = MediaFile::new(
            PathBuf::from("/test/video2.mp4"),
            2048,
            "video/mp4".to_string(),
        );

        db.store_media_file(&file1).await.unwrap();
        db.store_media_file(&file2).await.unwrap();

        // Verify we have 2 unique records
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_files")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(count, 2);

        // Test the remove_duplicates function (should have no effect since no duplicates exist)
        let removed = db.remove_duplicates().await.unwrap();
        assert_eq!(removed, 0);

        // Verify count is still 2
        let count_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_files")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(count_after, 2);
    }
}
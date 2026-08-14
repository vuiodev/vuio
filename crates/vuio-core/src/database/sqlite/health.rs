//! Integrity checking, repair, backup, and compaction.
//!
//! Most of what a hand-maintained index needs repairing for cannot happen
//! here: the music categories are queries, and the playlist links are foreign
//! keys the engine enforces. What remains is the directory tree, which is
//! maintained by this crate and so can drift if a write path is wrong.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use super::{directory, schema, SqliteDatabase};
use crate::database::{DatabaseHealth, DatabaseIssue, IssueSeverity};

fn healthy() -> DatabaseHealth {
    DatabaseHealth {
        is_healthy: true,
        corruption_detected: false,
        integrity_check_passed: true,
        issues: Vec::new(),
        repair_attempted: false,
        repair_successful: false,
    }
}

impl SqliteDatabase {
    pub(super) async fn check_and_repair_impl(&self) -> Result<DatabaseHealth> {
        let mut health = self
            .execute_read(move |connection| {
                let mut health = healthy();

                // Verify schema readiness and user_version
                let schema_version: i64 = connection
                    .query_row("PRAGMA user_version", [], |row| row.get(0))?;
                if schema_version != schema::SCHEMA_VERSION {
                    health.is_healthy = false;
                    health.issues.push(DatabaseIssue {
                        severity: IssueSeverity::Error,
                        description: format!(
                            "Schema version mismatch: expected {}, got {}",
                            schema::SCHEMA_VERSION,
                            schema_version
                        ),
                        table_affected: None,
                        suggested_action: "Run migrations".to_owned(),
                    });
                }

                // Verify core tables exist and are queryable
                let tables_ok: bool = connection
                    .query_row(
                        "SELECT COUNT(*) >= 4 FROM sqlite_master WHERE type = 'table' AND name IN ('media_files', 'directories', 'directory_mime_counts', 'playlists')",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap_or(false);

                if !tables_ok {
                    health.is_healthy = false;
                    health.issues.push(DatabaseIssue {
                        severity: IssueSeverity::Critical,
                        description: "Core database tables missing or incomplete".to_owned(),
                        table_affected: None,
                        suggested_action: "Initialize database schema".to_owned(),
                    });
                }

                Ok(health)
            })
            .await?;

        // Derived indexes (directories tree & full-text search) are incrementally
        // maintained during writes. Only rebuild them on startup if corruption was detected
        // or if the directory index is missing while files exist.
        let needs_rebuild = if !health.is_healthy {
            true
        } else {
            self.execute_read(move |connection| {
                let has_dirs: bool = connection
                    .query_row("SELECT 1 FROM directories LIMIT 1", [], |_| Ok(true))
                    .unwrap_or(false);
                let has_files: bool = connection
                    .query_row("SELECT 1 FROM media_files LIMIT 1", [], |_| Ok(true))
                    .unwrap_or(false);
                Ok(has_files && !has_dirs)
            })
            .await?
        };

        if needs_rebuild {
            let rebuilt = self.rebuild_derived_indexes_impl().await?;
            health.repair_attempted = true;
            health.repair_successful = rebuilt.is_healthy;
            health.issues.extend(rebuilt.issues);
            if !rebuilt.is_healthy {
                health.is_healthy = false;
            }
        }

        Ok(health)
    }

    pub(super) async fn rebuild_derived_indexes_impl(&self) -> Result<DatabaseHealth> {
        let mut health = healthy();

        let rebuilt = self
            .transact(move |transaction| {
                directory::rebuild(transaction)?;
                Ok(())
            })
            .await;
        match rebuilt {
            Ok(()) => info!("Rebuilt the directory index"),
            Err(error) => {
                warn!("Failed to rebuild the directory index: {error:#}");
                health.is_healthy = false;
                health.issues.push(DatabaseIssue {
                    severity: IssueSeverity::Error,
                    description: format!("Directory index rebuild failed: {error}"),
                    table_affected: Some("directories".to_owned()),
                    suggested_action: "Restore a backup or rescan the library".to_owned(),
                });
            }
        }

        // The full-text index is maintained by triggers, so it should never
        // need this — but it shadows two tables, and 'rebuild' costs one pass
        // over them. Cheap enough to be the standing answer to any doubt about
        // whether search is showing the whole library.
        let searchable = self
            .execute_write(move |connection| {
                connection
                    .execute_batch(schema::FTS_REBUILD)
                    .context("Failed to rebuild the full-text index")?;
                Ok(())
            })
            .await;
        match searchable {
            Ok(()) => info!("Rebuilt the full-text index"),
            Err(error) => {
                warn!("Failed to rebuild the full-text index: {error:#}");
                health.is_healthy = false;
                health.issues.push(DatabaseIssue {
                    severity: IssueSeverity::Error,
                    description: format!("Full-text index rebuild failed: {error}"),
                    table_affected: Some("media_fts".to_owned()),
                    suggested_action: "Restore a backup or rescan the library".to_owned(),
                });
            }
        }

        Ok(health)
    }

    pub(super) async fn vacuum_impl(&self) -> Result<bool> {
        let before = tokio::fs::metadata(&self.db_path)
            .await
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        self.execute_write(move |connection| {
            connection
                .execute_batch("VACUUM")
                .context("Failed to compact the database")?;
            Ok(())
        })
        .await?;

        let after = tokio::fs::metadata(&self.db_path)
            .await
            .map(|metadata| metadata.len())
            .unwrap_or(before);
        Ok(after < before)
    }

    pub(super) async fn create_backup_impl(&self, backup_path: &Path) -> Result<()> {
        if let Some(parent) = backup_path.parent() {
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!("Failed to create backup directory {}", parent.display())
            })?;
        }
        // `VACUUM INTO` writes a consistent, compacted copy in one statement
        // without blocking writers or needing a second connection.
        let destination = backup_path.to_path_buf();
        if destination.exists() {
            tokio::fs::remove_file(&destination).await.with_context(|| {
                format!("Failed to replace existing backup {}", destination.display())
            })?;
        }

        let target = destination.clone();
        self.execute_read(move |connection| {
            connection
                .execute("VACUUM INTO ?", [target.to_string_lossy().as_ref()])
                .with_context(|| format!("Failed to write backup {}", target.display()))?;
            Ok(())
        })
        .await?;

        info!("Wrote database backup to {}", destination.display());
        Ok(())
    }

    /// Install a validated backup in place of the active database.
    ///
    /// Named apart from `DatabaseBackend::restore_backup_file` so the trait
    /// method can delegate here without the two resolving to each other.
    pub async fn install_backup(backup_path: PathBuf, database_path: PathBuf) -> Result<()> {
        tokio::task::spawn_blocking(move || -> Result<()> {
            // Validate before touching anything: a restore that destroys the
            // working database and then fails is worse than no restore.
            schema::validate_database_file(&backup_path).with_context(|| {
                format!("{} is not a usable backup", backup_path.display())
            })?;

            if let Some(parent) = database_path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }

            let temporary =
                database_path.with_extension(format!("restore-{}.tmp", uuid::Uuid::new_v4()));
            std::fs::copy(&backup_path, &temporary)?;
            std::fs::File::open(&temporary)?.sync_all()?;
            schema::validate_database_file(&temporary)?;

            let previous = database_path.with_extension(format!(
                "pre-restore-{}.db",
                uuid::Uuid::new_v4()
            ));
            let had_previous = database_path.exists();
            if had_previous {
                std::fs::rename(&database_path, &previous)?;
            }
            if let Err(error) = std::fs::rename(&temporary, &database_path) {
                if had_previous {
                    let _ = std::fs::rename(&previous, &database_path);
                }
                return Err(error.into());
            }

            // The replaced database's write-ahead log describes pages that no
            // longer exist. Leaving it would corrupt the restored file.
            for sidecar in ["db-wal", "db-shm"] {
                let stale = database_path.with_extension(sidecar);
                if stale.exists() {
                    std::fs::remove_file(stale)?;
                }
            }

            if had_previous {
                std::fs::remove_file(previous)?;
            }
            Ok(())
        })
        .await
        .context("SQLite restore task failed")?
    }
}

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

                // `integrity_check` reports one row per problem, or the single
                // value "ok".
                let mut statement = connection.prepare("PRAGMA integrity_check")?;
                let findings = statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                for finding in findings.iter().filter(|finding| *finding != "ok") {
                    health.is_healthy = false;
                    health.corruption_detected = true;
                    health.integrity_check_passed = false;
                    health.issues.push(DatabaseIssue {
                        severity: IssueSeverity::Critical,
                        description: format!("Integrity check reported: {finding}"),
                        table_affected: None,
                        suggested_action:
                            "Restore the most recent backup; the file cannot be repaired in place"
                                .to_owned(),
                    });
                }

                let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
                let violations = statement.query_map([], |row| row.get::<_, String>(0))?.count();
                if violations > 0 {
                    health.is_healthy = false;
                    health.issues.push(DatabaseIssue {
                        severity: IssueSeverity::Error,
                        description: format!("{violations} rows reference records that are gone"),
                        table_affected: None,
                        suggested_action: "Rebuild derived indexes".to_owned(),
                    });
                }

                Ok(health)
            })
            .await?;

        // Rebuilding the directory tree is cheap next to the scan that follows
        // a failed startup, so it runs unconditionally rather than only after a
        // problem is detected: drift there is silent, and shows up to the user
        // only as folders missing from a browse.
        let rebuilt = self.rebuild_derived_indexes_impl().await?;
        health.repair_attempted = true;
        health.repair_successful = rebuilt.is_healthy;
        health.issues.extend(rebuilt.issues);
        if !rebuilt.is_healthy {
            health.is_healthy = false;
        }
        Ok(health)
    }

    pub(super) async fn rebuild_derived_indexes_impl(&self) -> Result<DatabaseHealth> {
        let rebuilt = self
            .transact(move |transaction| {
                directory::rebuild(transaction)?;
                Ok(())
            })
            .await;

        let mut health = healthy();
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

//! Opaque blob storage, used for pairing credentials.

use anyhow::Result;
use rusqlite::OptionalExtension;

use super::SqliteDatabase;

impl SqliteDatabase {
    pub(super) async fn get_secret_impl(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let key = key.to_owned();
        self.execute_read(move |connection| {
            Ok(connection
                .prepare_cached("SELECT value FROM secrets WHERE key = ?")?
                .query_row([&key], |row| row.get::<_, Vec<u8>>(0))
                .optional()?)
        })
        .await
    }

    pub(super) async fn set_secret_impl(&self, key: &str, value: &[u8]) -> Result<()> {
        let key = key.to_owned();
        let value = value.to_vec();
        self.transact(move |transaction| {
            transaction.execute(
                "INSERT INTO secrets (key, value) VALUES (?, ?) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                rusqlite::params![key, value],
            )?;
            Ok(())
        })
        .await
    }

    pub(super) async fn delete_secret_impl(&self, key: &str) -> Result<bool> {
        let key = key.to_owned();
        self.transact(move |transaction| {
            Ok(transaction.execute("DELETE FROM secrets WHERE key = ?", [&key])? > 0)
        })
        .await
    }
}

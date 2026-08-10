use super::*;

impl RedbDatabase {
    pub(super) async fn get_secret_impl(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let key = key.to_string();
        self.execute_read(move |database| {
            let transaction = database.begin_read()?;
            let table = transaction.open_table(SECRETS_TABLE)?;
            Ok(table.get(key.as_str())?.map(|value| value.value().to_vec()))
        })
        .await
    }

    pub(super) async fn set_secret_impl(&self, key: &str, value: &[u8]) -> Result<()> {
        let key = key.to_string();
        let value = value.to_vec();
        self.execute_write(move |database| {
            let transaction = database.begin_write()?;
            transaction
                .open_table(SECRETS_TABLE)?
                .insert(key.as_str(), value.as_slice())?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    pub(super) async fn delete_secret_impl(&self, key: &str) -> Result<bool> {
        let key = key.to_string();
        self.execute_write(move |database| {
            let transaction = database.begin_write()?;
            let removed = {
                let mut table = transaction.open_table(SECRETS_TABLE)?;
                let previous = table.remove(key.as_str())?;
                previous.is_some()
            };
            transaction.commit()?;
            Ok(removed)
        })
        .await
    }
}

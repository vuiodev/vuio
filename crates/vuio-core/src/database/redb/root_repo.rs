use super::*;

impl RedbDatabase {
    pub(super) async fn get_root_availability_impl(
        &self,
        path: &Path,
    ) -> Result<Option<RootAvailability>> {
        let key = Self::canonical_path(path)?.to_string_lossy().into_owned();
        self.execute_read(move |database| {
            let transaction = database.begin_read()?;
            let table = transaction.open_table(ROOT_AVAILABILITY)?;
            let Some(value) = table.get(key.as_str())? else {
                return Ok(None);
            };
            let state = rkyv::from_bytes::<RootAvailabilitySerializable, rkyv::rancor::Error>(
                value.value(),
            )
            .map_err(|error| anyhow!("invalid root availability record {key}: {error}"))?;
            Ok(Some(state.into()))
        })
        .await
    }

    pub(super) async fn list_root_availability_impl(&self) -> Result<Vec<RootAvailability>> {
        self.execute_read(move |database| {
            let transaction = database.begin_read()?;
            let table = transaction.open_table(ROOT_AVAILABILITY)?;
            let mut states = Vec::new();
            for entry in table.iter()? {
                let (key, value) = entry?;
                let state = rkyv::from_bytes::<RootAvailabilitySerializable, rkyv::rancor::Error>(
                    value.value(),
                )
                .map_err(|error| {
                    anyhow!("invalid root availability record {}: {error}", key.value())
                })?;
                states.push(state.into());
            }
            Ok(states)
        })
        .await
    }

    pub(super) async fn set_root_availability_impl(&self, state: &RootAvailability) -> Result<()> {
        let mut state = state.clone();
        state.path = Self::canonical_path(&state.path)?;
        let key = state.path.to_string_lossy().into_owned();
        let bytes =
            rkyv::to_bytes::<rkyv::rancor::Error>(&RootAvailabilitySerializable::from(&state))
                .map_err(|error| anyhow!("failed to archive root availability {key}: {error}"))?;
        self.execute_write(move |database| {
            let transaction = database.begin_write()?;
            transaction
                .open_table(ROOT_AVAILABILITY)?
                .insert(key.as_str(), bytes.as_slice())?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    pub(super) async fn remove_root_availability_impl(&self, path: &Path) -> Result<()> {
        let key = Self::canonical_path(path)?.to_string_lossy().into_owned();
        self.execute_write(move |database| {
            let transaction = database.begin_write()?;
            transaction
                .open_table(ROOT_AVAILABILITY)?
                .remove(key.as_str())?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }
}

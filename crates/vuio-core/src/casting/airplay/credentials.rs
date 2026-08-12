use crate::database::SecretStore;
use anyhow::{Context, Result};
use hap_crypto::{AccessoryPairing, ControllerKeypair};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

/// Key under which the whole pairing document lives in the `secrets` table.
const SECRET_KEY: &str = "airplay.pairings";

#[derive(Clone)]
pub struct CredentialStore {
    inner: Arc<Inner>,
}

struct Inner {
    /// `None` keeps the store in memory, which is what tests and the
    /// non-persistent constructor use.
    secrets: Option<Arc<dyn SecretStore>>,
    document: Mutex<Document>,
}

#[derive(Default, Serialize, Deserialize)]
struct Document {
    #[serde(default = "document_version")]
    version: u32,
    controller: Option<ControllerRecord>,
    #[serde(default)]
    receivers: HashMap<String, ReceiverRecord>,
}

#[derive(Serialize, Deserialize)]
struct ControllerRecord {
    id: String,
    seed: String,
}

#[derive(Serialize, Deserialize)]
struct ReceiverRecord {
    pairing_id: String,
    ltpk: String,
}

const fn document_version() -> u32 {
    1
}

impl CredentialStore {
    pub fn memory() -> Self {
        Self {
            inner: Arc::new(Inner {
                secrets: None,
                document: Mutex::new(Document {
                    version: document_version(),
                    ..Document::default()
                }),
            }),
        }
    }

    /// Load pairings from the application database.
    ///
    /// Keys live in the `secrets` table rather than the OS credential vault so
    /// that a VuIO instance is self-contained: the database file is the single
    /// thing to back up, move between hosts, or mount into a container, and
    /// headless and Docker deployments have no system keychain to talk to.
    pub async fn load(secrets: Arc<dyn SecretStore>) -> Result<Self> {
        let stored = secrets
            .get_secret(SECRET_KEY)
            .await
            .context("reading AirPlay credentials")?;
        let document = match stored {
            Some(bytes) => parse_document(&String::from_utf8_lossy(&bytes))
                .context("parsing stored AirPlay credentials")?,
            None => Document {
                version: document_version(),
                ..Document::default()
            },
        };
        Ok(Self {
            inner: Arc::new(Inner {
                secrets: Some(secrets),
                document: Mutex::new(document),
            }),
        })
    }

    pub async fn is_paired(&self, renderer_id: &str) -> bool {
        self.inner
            .document
            .lock()
            .await
            .receivers
            .contains_key(renderer_id)
    }

    pub async fn controller(&self) -> Result<ControllerKeypair> {
        let mut document = self.inner.document.lock().await;
        if let Some(record) = &document.controller {
            return controller_from_record(record);
        }
        let controller = ControllerKeypair::generate(uuid::Uuid::new_v4().to_string());
        document.controller = Some(ControllerRecord {
            id: controller.id.clone(),
            seed: hex::encode(controller.seed()),
        });
        self.persist_locked(&document).await?;
        Ok(controller)
    }

    pub async fn pairing(
        &self,
        renderer_id: &str,
    ) -> Result<Option<(ControllerKeypair, AccessoryPairing)>> {
        let document = self.inner.document.lock().await;
        let Some(controller) = document.controller.as_ref() else {
            return Ok(None);
        };
        let Some(receiver) = document.receivers.get(renderer_id) else {
            return Ok(None);
        };
        let ltpk: [u8; 32] = decode_fixed(&receiver.ltpk, "receiver public key")?;
        Ok(Some((
            controller_from_record(controller)?,
            AccessoryPairing {
                pairing_id: receiver.pairing_id.clone(),
                ltpk,
            },
        )))
    }

    pub async fn save_pairing(&self, renderer_id: &str, pairing: &AccessoryPairing) -> Result<()> {
        let mut document = self.inner.document.lock().await;
        document.receivers.insert(
            renderer_id.to_string(),
            ReceiverRecord {
                pairing_id: pairing.pairing_id.clone(),
                ltpk: hex::encode(pairing.ltpk),
            },
        );
        self.persist_locked(&document).await
    }

    pub async fn forget(&self, renderer_id: &str) -> Result<bool> {
        let mut document = self.inner.document.lock().await;
        let removed = document.receivers.remove(renderer_id).is_some();
        if removed {
            self.persist_locked(&document).await?;
        }
        Ok(removed)
    }

    async fn persist_locked(&self, document: &Document) -> Result<()> {
        let Some(secrets) = &self.inner.secrets else {
            return Ok(());
        };
        let encoded = serde_json::to_string(document).context("encoding AirPlay credentials")?;
        secrets
            .set_secret(SECRET_KEY, encoded.as_bytes())
            .await
            .context("storing AirPlay credentials")
    }
}

fn controller_from_record(record: &ControllerRecord) -> Result<ControllerKeypair> {
    let seed: [u8; 32] = decode_fixed(&record.seed, "controller private key")?;
    Ok(ControllerKeypair::from_seed(record.id.clone(), seed))
}

fn decode_fixed<const N: usize>(value: &str, label: &str) -> Result<[u8; N]> {
    hex::decode(value)
        .with_context(|| format!("decoding {label}"))?
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid {label} length"))
}

fn parse_document(value: &str) -> Result<Document> {
    let document: Document = serde_json::from_str(value)?;
    anyhow::ensure!(
        document.version == document_version(),
        "unsupported credential version"
    );
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_store_round_trips_pairing_without_exposing_keys() {
        let store = CredentialStore::memory();
        let _ = store.controller().await.unwrap();
        let pairing = AccessoryPairing {
            pairing_id: "AA:BB:CC:DD:EE:FF".to_string(),
            ltpk: [7; 32],
        };
        store.save_pairing("airplay:test", &pairing).await.unwrap();
        assert!(store.is_paired("airplay:test").await);
        let (_, loaded) = store.pairing("airplay:test").await.unwrap().unwrap();
        assert_eq!(loaded, pairing);
        assert!(store.forget("airplay:test").await.unwrap());
        assert!(!store.is_paired("airplay:test").await);
    }

    #[tokio::test]
    async fn pairings_survive_a_restart_through_the_database() {
        use crate::database::DatabaseManager as _;

        let temp = tempfile::tempdir().unwrap();
        let database = std::sync::Arc::new(
            crate::database::sqlite::SqliteDatabase::new(temp.path().join("pairings.db"))
                .await
                .unwrap(),
        );
        database.initialize().await.unwrap();
        let secrets: Arc<dyn SecretStore> = database.clone();

        let pairing = AccessoryPairing {
            pairing_id: "7C:58:BC:93:AE:C4".to_string(),
            ltpk: [9; 32],
        };
        {
            let store = CredentialStore::load(secrets.clone()).await.unwrap();
            let controller = store.controller().await.unwrap();
            store.save_pairing("airplay:sony", &pairing).await.unwrap();
            assert!(!controller.id.is_empty());
        }

        // A fresh store over the same database is what a restart looks like.
        let reopened = CredentialStore::load(secrets).await.unwrap();
        assert!(reopened.is_paired("airplay:sony").await);
        let (_, loaded) = reopened.pairing("airplay:sony").await.unwrap().unwrap();
        assert_eq!(loaded, pairing);
        assert!(reopened.forget("airplay:sony").await.unwrap());

        let after_forget = CredentialStore::load(database).await.unwrap();
        assert!(!after_forget.is_paired("airplay:sony").await);
    }
}

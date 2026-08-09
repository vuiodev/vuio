use anyhow::{Context, Result};
use hap_crypto::{AccessoryPairing, ControllerKeypair};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Mutex;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
const KEYRING_SERVICE: &str = "dev.vuio.airplay";
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
const KEYRING_ACCOUNT: &str = "pairings";

#[derive(Clone)]
pub struct CredentialStore {
    inner: Arc<Inner>,
}

struct Inner {
    path: Option<PathBuf>,
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
                path: None,
                document: Mutex::new(Document {
                    version: document_version(),
                    ..Document::default()
                }),
            }),
        }
    }

    pub async fn load(path: PathBuf) -> Result<Self> {
        let document = match load_from_keyring().await {
            Ok(Some(value)) => parse_document(&value).context("parsing AirPlay vault data")?,
            Ok(None) | Err(_) => load_file(&path).await?,
        };
        Ok(Self {
            inner: Arc::new(Inner {
                path: Some(path),
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
        let encoded = serde_json::to_string(document).context("encoding AirPlay credentials")?;
        if self.inner.path.is_none() {
            return Ok(());
        }
        if save_to_keyring(encoded.clone()).await.is_ok() {
            if let Some(path) = &self.inner.path {
                let _ = tokio::fs::remove_file(path).await;
            }
            return Ok(());
        }
        save_file(
            self.inner
                .path
                .as_ref()
                .context("AirPlay credential path is unavailable")?,
            encoded.as_bytes(),
        )
        .await
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

async fn load_file(path: &Path) -> Result<Document> {
    match tokio::fs::read_to_string(path).await {
        Ok(value) => parse_document(&value).context("parsing AirPlay credential file"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Document {
            version: document_version(),
            ..Document::default()
        }),
        Err(error) => Err(error).context("reading AirPlay credential file"),
    }
}

async fn save_file(path: &Path, value: &[u8]) -> Result<()> {
    let path = path.to_path_buf();
    let value = value.to_vec();
    tokio::task::spawn_blocking(move || -> Result<()> {
        use std::io::Write as _;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("json.tmp");
        let mut options = std::fs::OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(&value)?;
        file.sync_all()?;
        std::fs::rename(&temporary, &path)?;
        Ok(())
    })
    .await
    .context("joining AirPlay credential writer")??;
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
async fn load_from_keyring() -> Result<Option<String>> {
    tokio::task::spawn_blocking(|| -> Result<Option<String>> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    })
    .await
    .context("joining AirPlay vault reader")?
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
async fn load_from_keyring() -> Result<Option<String>> {
    Ok(None)
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
async fn save_to_keyring(value: String) -> Result<()> {
    tokio::task::spawn_blocking(move || -> Result<()> {
        keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)?.set_password(&value)?;
        Ok(())
    })
    .await
    .context("joining AirPlay vault writer")?
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
async fn save_to_keyring(_value: String) -> Result<()> {
    anyhow::bail!("system credential vault is unavailable")
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
}

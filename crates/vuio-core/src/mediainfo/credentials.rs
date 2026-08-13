//! API keys and tokens for the providers that require an account.
//!
//! These live in the `secrets` table rather than `config.toml`, following the
//! AirPlay pairing store. That is a functional requirement and not a matter of
//! taste: under Docker the configuration is built from environment variables and
//! the admin API refuses to write the file at all, so a credential kept in the
//! file would be unsettable in exactly the deployment most likely to need one. It
//! also keeps tokens out of the file operators paste into bug reports.

use crate::database::SecretStore;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

/// Key under which the whole credential document lives in the `secrets` table.
const SECRET_KEY: &str = "mediainfo.credentials";

#[derive(Clone)]
pub struct CredentialStore {
    inner: Arc<Inner>,
}

struct Inner {
    /// `None` keeps the store in memory, which is what the tests use.
    secrets: Option<Arc<dyn SecretStore>>,
    document: Mutex<Document>,
}

#[derive(Default, Serialize, Deserialize)]
struct Document {
    #[serde(default = "document_version")]
    version: u32,
    #[serde(default)]
    tokens: HashMap<String, String>,
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
                    tokens: HashMap::new(),
                }),
            }),
        }
    }

    /// Load the document, treating an unreadable one as empty.
    ///
    /// A corrupt secret should cost the operator their saved keys, not the ability
    /// to start the server or to save a replacement.
    pub async fn load(secrets: Arc<dyn SecretStore>) -> Result<Self> {
        let document = match secrets.get_secret(SECRET_KEY).await {
            Ok(Some(bytes)) => serde_json::from_slice::<Document>(&bytes).unwrap_or_else(|error| {
                tracing::warn!(%error, "Stored media info credentials were unreadable, starting empty");
                Document {
                    version: document_version(),
                    tokens: HashMap::new(),
                }
            }),
            Ok(None) => Document {
                version: document_version(),
                tokens: HashMap::new(),
            },
            Err(error) => {
                tracing::warn!(%error, "Could not read media info credentials");
                Document {
                    version: document_version(),
                    tokens: HashMap::new(),
                }
            }
        };

        Ok(Self {
            inner: Arc::new(Inner {
                secrets: Some(secrets),
                document: Mutex::new(document),
            }),
        })
    }

    pub async fn get(&self, provider: &str) -> Option<String> {
        self.inner.document.lock().await.tokens.get(provider).cloned()
    }

    /// Which providers have a credential stored.
    ///
    /// The dashboard is told only this, never the values — a saved token must not
    /// be readable back out of the API that set it.
    pub async fn stored_providers(&self) -> Vec<String> {
        let mut stored: Vec<String> = self
            .inner
            .document
            .lock()
            .await
            .tokens
            .keys()
            .cloned()
            .collect();
        stored.sort();
        stored
    }

    /// Store a token, or remove it when `token` is empty.
    pub async fn set(&self, provider: &str, token: &str) -> Result<()> {
        let mut document = self.inner.document.lock().await;
        let token = token.trim();
        if token.is_empty() {
            document.tokens.remove(provider);
        } else {
            document.tokens.insert(provider.to_string(), token.to_string());
        }
        self.persist(&document).await
    }

    pub async fn clear(&self, provider: &str) -> Result<()> {
        let mut document = self.inner.document.lock().await;
        document.tokens.remove(provider);
        self.persist(&document).await
    }

    async fn persist(&self, document: &Document) -> Result<()> {
        let Some(secrets) = self.inner.secrets.as_ref() else {
            return Ok(());
        };
        let encoded = serde_json::to_vec(document).context("Failed to encode media info credentials")?;
        secrets
            .set_secret(SECRET_KEY, &encoded)
            .await
            .context("Failed to store media info credentials")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_stored_token_can_be_read_back_and_cleared() {
        let store = CredentialStore::memory();
        assert!(store.get("tmdb").await.is_none());

        store.set("tmdb", "abc123").await.unwrap();
        assert_eq!(store.get("tmdb").await.as_deref(), Some("abc123"));
        assert_eq!(store.stored_providers().await, vec!["tmdb".to_string()]);

        store.clear("tmdb").await.unwrap();
        assert!(store.get("tmdb").await.is_none());
        assert!(store.stored_providers().await.is_empty());
    }

    #[tokio::test]
    async fn setting_an_empty_token_removes_it() {
        // The dashboard's Clear button sends an empty string rather than a
        // separate verb, so this is the path that has to erase.
        let store = CredentialStore::memory();
        store.set("omdb", "key").await.unwrap();
        store.set("omdb", "   ").await.unwrap();
        assert!(store.get("omdb").await.is_none());
    }

    #[tokio::test]
    async fn tokens_are_trimmed_before_storing() {
        let store = CredentialStore::memory();
        store.set("lastfm", "  key  ").await.unwrap();
        assert_eq!(store.get("lastfm").await.as_deref(), Some("key"));
    }

    #[test]
    fn an_unreadable_document_decodes_as_empty() {
        // Whatever else happens, a corrupt secret must not stop the server.
        let document: Document = serde_json::from_slice(b"not json").unwrap_or_default();
        assert!(document.tokens.is_empty());
    }
}

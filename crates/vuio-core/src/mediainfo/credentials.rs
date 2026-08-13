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

/// Consulted when nothing is stored for a provider. Production supplies the
/// environment; the tests supply a fixed map, so they neither read nor mutate
/// the process environment — which is shared, and which `cargo test` runs
/// several threads against at once.
type Fallback = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

struct Inner {
    /// `None` keeps the store in memory, which is what the tests use.
    secrets: Option<Arc<dyn SecretStore>>,
    document: Mutex<Document>,
    fallback: Option<Fallback>,
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

/// Where a provider's credential comes from. Serialised for the dashboard, which
/// has to say which one is in force — "no key" and "a key you cannot see here"
/// look identical otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialSource {
    /// Saved from the dashboard, in the secrets table.
    User,
    /// `VUIO_<ID>_API_KEY`, from the environment or `.env`.
    Environment,
    /// The provider is inactive.
    None,
}

impl CredentialStore {
    /// An empty store with no fallback: what the tests use, so nothing they
    /// assert depends on what the machine running them happens to have set.
    pub fn memory() -> Self {
        Self::with_fallback(None, None)
    }

    /// As [`Self::memory`], but answering from `fallback` when nothing is
    /// stored — the in-memory stand-in for the environment.
    #[cfg(test)]
    pub fn memory_with_fallback(
        fallback: impl Fn(&str) -> Option<String> + Send + Sync + 'static,
    ) -> Self {
        Self::with_fallback(None, Some(Arc::new(fallback)))
    }

    fn with_fallback(secrets: Option<Arc<dyn SecretStore>>, fallback: Option<Fallback>) -> Self {
        Self {
            inner: Arc::new(Inner {
                secrets,
                document: Mutex::new(Document {
                    version: document_version(),
                    tokens: HashMap::new(),
                }),
                fallback,
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
                fallback: Some(Arc::new(super::env_keys::env_credential)),
            }),
        })
    }

    /// The credential to use for a provider, or `None` if it has none.
    ///
    /// A token saved from the dashboard wins over one supplied to the process,
    /// so an operator can override a key their container or `.env` sets without
    /// having to find and change it. Clearing theirs falls back rather than
    /// turning the provider off, which is why [`Self::set`] treats an empty
    /// token as a removal.
    pub async fn get(&self, provider: &str) -> Option<String> {
        let stored = self.inner.document.lock().await.tokens.get(provider).cloned();
        stored.or_else(|| self.fallback(provider))
    }

    /// Where a provider's credential is coming from, for the dashboard to
    /// explain. Never the value itself.
    pub async fn source(&self, provider: &str) -> CredentialSource {
        if self.inner.document.lock().await.tokens.contains_key(provider) {
            return CredentialSource::User;
        }
        if self.fallback(provider).is_some() {
            return CredentialSource::Environment;
        }
        CredentialSource::None
    }

    fn fallback(&self, provider: &str) -> Option<String> {
        self.inner
            .fallback
            .as_ref()
            .and_then(|fallback| fallback(provider))
    }

    /// Which providers have a credential *stored*.
    ///
    /// Deliberately not "have a credential": an environment-supplied key is not
    /// stored and must not be reported as though the operator saved it, or the
    /// dashboard's Clear button would appear to do nothing.
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

    /// Stands in for `VUIO_TMDB_API_KEY` being set on the server.
    fn with_env_key() -> CredentialStore {
        CredentialStore::memory_with_fallback(|provider| {
            (provider == "tmdb").then(|| "from-the-environment".to_string())
        })
    }

    #[tokio::test]
    async fn an_environment_key_is_used_when_nothing_is_stored() {
        let store = with_env_key();
        assert_eq!(store.get("tmdb").await.as_deref(), Some("from-the-environment"));
        assert_eq!(store.source("tmdb").await, CredentialSource::Environment);

        // And says nothing about providers it was not given.
        assert!(store.get("omdb").await.is_none());
        assert_eq!(store.source("omdb").await, CredentialSource::None);
    }

    /// The operator's own key has to win, or a key baked into a container image
    /// could not be overridden from the dashboard.
    #[tokio::test]
    async fn a_saved_token_beats_the_environment() {
        let store = with_env_key();
        store.set("tmdb", "mine").await.unwrap();

        assert_eq!(store.get("tmdb").await.as_deref(), Some("mine"));
        assert_eq!(store.source("tmdb").await, CredentialSource::User);
    }

    /// Clearing means "stop using mine", not "turn the provider off" — the
    /// dashboard's Clear button would otherwise be a one-way door.
    #[tokio::test]
    async fn clearing_a_saved_token_falls_back_to_the_environment() {
        let store = with_env_key();
        store.set("tmdb", "mine").await.unwrap();
        store.clear("tmdb").await.unwrap();

        assert_eq!(store.get("tmdb").await.as_deref(), Some("from-the-environment"));
        assert_eq!(store.source("tmdb").await, CredentialSource::Environment);
    }

    /// An environment key is not stored, and must not be reported as though the
    /// operator had saved it — the UI decides whether to offer Clear from this.
    #[tokio::test]
    async fn an_environment_key_is_not_reported_as_stored() {
        let store = with_env_key();
        assert!(store.stored_providers().await.is_empty());

        store.set("tmdb", "mine").await.unwrap();
        assert_eq!(store.stored_providers().await, vec!["tmdb".to_string()]);
    }
}

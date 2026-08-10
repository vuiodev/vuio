//! Small bounded runtime registries. Media records and indexes remain owned by ReDB.

use crate::{casting::RendererDevice, state::SoapCacheKey};
use axum::body::Bytes;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    time::{Duration, Instant},
};

pub const BROWSE_CACHE_MAX_ENTRIES: usize = 256;
pub const BROWSE_CACHE_MAX_BYTES: usize = 16 * 1024 * 1024;
pub const BOOKMARK_MAX_ENTRIES: usize = 10_000;
pub const ACTIVE_CAST_MAX_ENTRIES: usize = 128;
pub const ACTIVE_CAST_TTL: Duration = Duration::from_secs(180);
pub const RENDERER_CACHE_MAX_ENTRIES: usize = 128;
pub const RENDERER_CACHE_FRESH_TTL: Duration = Duration::from_secs(90);
pub const RENDERER_CACHE_STALE_TTL: Duration = Duration::from_secs(600);

struct BrowseEntry {
    value: Bytes,
    last_access: u64,
}

pub struct BrowseResponseCache {
    entries: HashMap<SoapCacheKey, BrowseEntry>,
    total_bytes: usize,
    clock: u64,
    epoch: u64,
}

impl BrowseResponseCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            total_bytes: 0,
            clock: 0,
            epoch: 0,
        }
    }

    fn tick(&mut self) -> u64 {
        self.clock = self.clock.wrapping_add(1);
        self.clock
    }

    pub fn get(&mut self, key: &SoapCacheKey) -> Option<Bytes> {
        let access = self.tick();
        let entry = self.entries.get_mut(key)?;
        entry.last_access = access;
        Some(entry.value.clone())
    }

    pub fn insert(&mut self, key: SoapCacheKey, value: Bytes) {
        let value_size = value.len();
        if value_size > BROWSE_CACHE_MAX_BYTES {
            return;
        }
        if let Some(previous) = self.entries.remove(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(previous.value.len());
        }
        let access = self.tick();
        self.total_bytes = self.total_bytes.saturating_add(value_size);
        self.entries.insert(
            key,
            BrowseEntry {
                value,
                last_access: access,
            },
        );
        while self.entries.len() > BROWSE_CACHE_MAX_ENTRIES
            || self.total_bytes > BROWSE_CACHE_MAX_BYTES
        {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.total_bytes = self.total_bytes.saturating_sub(removed.value.len());
            }
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.total_bytes = 0;
        self.epoch = self.epoch.wrapping_add(1);
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn generation(&self) -> Option<u32> {
        self.entries.keys().next().map(|key| key.content_update_id)
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }
}

impl Default for BrowseResponseCache {
    fn default() -> Self {
        Self::new()
    }
}

struct BoundedEntry<V> {
    value: V,
    last_access: u64,
}

pub struct BoundedRegistry<K, V> {
    entries: HashMap<K, BoundedEntry<V>>,
    max_entries: usize,
    clock: u64,
}

impl<K: Eq + Hash + Clone, V> BoundedRegistry<K, V> {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
            clock: 0,
        }
    }

    fn tick(&mut self) -> u64 {
        self.clock = self.clock.wrapping_add(1);
        self.clock
    }

    pub fn insert(&mut self, key: K, value: V) {
        let access = self.tick();
        self.entries.insert(
            key,
            BoundedEntry {
                value,
                last_access: access,
            },
        );
        while self.entries.len() > self.max_entries {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        let access = self.tick();
        let entry = self.entries.get_mut(key)?;
        entry.last_access = access;
        Some(&entry.value)
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.entries.remove(key).map(|entry| entry.value)
    }

    pub fn snapshot(&self) -> HashMap<K, V>
    where
        V: Clone,
    {
        self.entries
            .iter()
            .map(|(key, entry)| (key.clone(), entry.value.clone()))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

pub type BookmarkRegistry = BoundedRegistry<i64, u32>;

pub struct ActiveCastRegistry {
    entries: HashMap<String, (String, String, Instant)>,
}

impl ActiveCastRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn insert(&mut self, device: String, filename: String) {
        self.insert_labeled(device.clone(), device, filename);
    }

    pub fn insert_labeled(&mut self, key: String, device: String, filename: String) {
        self.prune();
        if self.entries.len() >= ACTIVE_CAST_MAX_ENTRIES && !self.entries.contains_key(&key) {
            if let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, _, seen))| *seen)
                .map(|(key, _)| key.clone())
            {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(key, (device, filename, Instant::now()));
    }

    pub fn remove(&mut self, device: &str) {
        self.entries.remove(device);
    }

    pub fn prune(&mut self) {
        self.entries
            .retain(|_, (_, _, last_seen)| last_seen.elapsed() < ACTIVE_CAST_TTL);
    }

    pub fn snapshot(&mut self) -> HashMap<String, String> {
        self.prune();
        self.entries
            .iter()
            .map(|(_, (device, filename, _))| (device.clone(), filename.clone()))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for ActiveCastRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
struct RendererSnapshot {
    renderers: Vec<RendererDevice>,
    refreshed_at: Option<Instant>,
}

/// A single shared renderer snapshot. The refresh mutex prevents concurrent
/// HTTP and MCP requests from launching duplicate three-second SSDP searches.
pub struct RendererCache {
    snapshot: tokio::sync::RwLock<RendererSnapshot>,
    refresh: tokio::sync::Mutex<()>,
    casting: crate::casting::CastingManager,
}

impl RendererCache {
    pub fn new() -> Self {
        Self {
            snapshot: tokio::sync::RwLock::new(RendererSnapshot::default()),
            refresh: tokio::sync::Mutex::new(()),
            casting: crate::casting::CastingManager::new(),
        }
    }

    pub async fn persistent(
        secrets: std::sync::Arc<dyn crate::database::SecretStore>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            snapshot: tokio::sync::RwLock::new(RendererSnapshot::default()),
            refresh: tokio::sync::Mutex::new(()),
            casting: crate::casting::CastingManager::persistent(secrets).await?,
        })
    }

    pub async fn snapshot(&self) -> Vec<RendererDevice> {
        self.snapshot.read().await.renderers.clone()
    }

    pub async fn name_for_ip(&self, ip: &str) -> Option<String> {
        self.snapshot
            .read()
            .await
            .renderers
            .iter()
            .find(|renderer| {
                renderer
                    .peer_ip()
                    .is_some_and(|address| address.to_string() == ip)
            })
            .map(|renderer| renderer.friendly_name.clone())
    }

    pub async fn replace(&self, mut renderers: Vec<RendererDevice>) {
        renderers.sort_by(|left, right| {
            left.friendly_name
                .to_lowercase()
                .cmp(&right.friendly_name.to_lowercase())
                .then_with(|| left.id.cmp(&right.id))
        });
        let mut device_ids = HashSet::new();
        let mut physical_devices = HashSet::new();
        renderers.retain(|renderer| {
            let id = renderer.id.trim().to_lowercase();
            let physical = format!(
                "{:?}|{}|{}|{}",
                renderer.protocol,
                renderer
                    .peer_ip()
                    .map(|address| address.to_string())
                    .unwrap_or_default(),
                renderer.friendly_name.trim().to_lowercase(),
                renderer.model_name.trim().to_lowercase()
            );
            if (!id.is_empty() && device_ids.contains(&id)) || physical_devices.contains(&physical)
            {
                return false;
            }
            if !id.is_empty() {
                device_ids.insert(id);
            }
            physical_devices.insert(physical);
            true
        });
        renderers.truncate(RENDERER_CACHE_MAX_ENTRIES);
        *self.snapshot.write().await = RendererSnapshot {
            renderers,
            refreshed_at: Some(Instant::now()),
        };
    }

    pub async fn get_or_refresh(&self) -> anyhow::Result<Vec<RendererDevice>> {
        if let Some(renderers) = self.usable_snapshot(RENDERER_CACHE_FRESH_TTL).await {
            return Ok(renderers);
        }

        let _refresh_guard = self.refresh.lock().await;
        if let Some(renderers) = self.usable_snapshot(RENDERER_CACHE_FRESH_TTL).await {
            return Ok(renderers);
        }

        match self.casting.discover(Duration::from_secs(3)).await {
            Ok(discovery) => {
                self.replace_discovery(discovery).await;
                Ok(self.snapshot().await)
            }
            Err(error) => {
                if let Some(renderers) = self.usable_snapshot(RENDERER_CACHE_STALE_TTL).await {
                    tracing::warn!(%error, "Renderer refresh failed; using stale snapshot");
                    Ok(renderers)
                } else {
                    Err(error)
                }
            }
        }
    }

    pub async fn refresh(&self) -> anyhow::Result<Vec<RendererDevice>> {
        let _refresh_guard = self.refresh.lock().await;
        let discovery = self.casting.discover(Duration::from_secs(3)).await?;
        self.replace_discovery(discovery).await;
        Ok(self.snapshot().await)
    }

    async fn replace_discovery(&self, mut discovery: crate::casting::DiscoveryBatch) {
        if !discovery.failed_protocols.is_empty() {
            let snapshot = self.snapshot.read().await;
            if snapshot
                .refreshed_at
                .is_some_and(|time| time.elapsed() <= RENDERER_CACHE_STALE_TTL)
            {
                discovery.devices.extend(
                    snapshot
                        .renderers
                        .iter()
                        .filter(|renderer| discovery.failed_protocols.contains(&renderer.protocol))
                        .cloned(),
                );
            }
        }
        self.replace(discovery.devices).await;
    }

    pub fn validate(
        &self,
        renderer: &RendererDevice,
        item: &crate::casting::PlaybackItem,
    ) -> Result<(), String> {
        self.casting.validate(renderer, item)
    }

    pub async fn play(
        &self,
        renderer: &RendererDevice,
        item: &crate::casting::PlaybackItem,
    ) -> anyhow::Result<()> {
        self.casting.play(renderer, item).await
    }

    pub async fn control(
        &self,
        renderer: &RendererDevice,
        action: crate::casting::PlaybackAction,
    ) -> anyhow::Result<()> {
        self.casting.control(renderer, action).await
    }

    pub async fn status(
        &self,
        renderer: &RendererDevice,
    ) -> anyhow::Result<crate::casting::PlaybackStatus> {
        self.casting.status(renderer).await
    }

    pub async fn begin_pairing(
        &self,
        renderer: &RendererDevice,
    ) -> anyhow::Result<crate::casting::PairingChallenge> {
        self.casting.begin_pairing(renderer).await
    }

    pub async fn finish_pairing(
        &self,
        protocol: crate::casting::RendererProtocol,
        challenge_id: &str,
        pin: &str,
    ) -> anyhow::Result<()> {
        self.casting
            .finish_pairing(protocol, challenge_id, pin)
            .await
    }

    pub async fn forget_pairing(&self, renderer: &RendererDevice) -> anyhow::Result<bool> {
        self.casting.forget_pairing(renderer).await
    }

    pub async fn queue_next(
        &self,
        renderer: &RendererDevice,
        item: &crate::casting::PlaybackItem,
    ) -> anyhow::Result<bool> {
        self.casting.queue_next(renderer, item).await
    }

    pub async fn shutdown(&self) {
        self.casting.shutdown().await;
    }

    async fn usable_snapshot(&self, ttl: Duration) -> Option<Vec<RendererDevice>> {
        let snapshot = self.snapshot.read().await;
        let refreshed_at = snapshot.refreshed_at?;
        if refreshed_at.elapsed() <= ttl {
            Some(snapshot.renderers.clone())
        } else {
            None
        }
    }
}

impl Default for RendererCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_key(epoch: u64) -> SoapCacheKey {
        SoapCacheKey {
            object_id: "audio".to_string(),
            starting_index: 0,
            requested_count: 25,
            client_profile: crate::web::client::DlnaClientProfile::Standard,
            content_update_id: 1,
            browse_epoch: epoch,
        }
    }

    #[test]
    fn cleared_epoch_cannot_reuse_a_late_stale_response() {
        let mut cache = BrowseResponseCache::new();
        let stale_key = cache_key(cache.epoch());
        cache.clear();
        let current_key = cache_key(cache.epoch());

        // Simulate a request that finished after invalidation and inserted its
        // response late. Its old epoch must not match a subsequent request.
        cache.insert(stale_key, Bytes::from_static(b"stale"));
        assert!(cache.get(&current_key).is_none());
    }

    fn renderer(id: &str, name: &str, model: &str, ip: &str) -> RendererDevice {
        let control_url = format!("http://{ip}:1400/control");
        let location_url = format!("http://{ip}:1400/description.xml");
        RendererDevice {
            id: id.to_string(),
            friendly_name: name.to_string(),
            control_url: control_url.clone(),
            location_url: location_url.clone(),
            model_name: model.to_string(),
            protocol: crate::casting::RendererProtocol::Dlna,
            pairing: crate::casting::PairingStatus::NotRequired,
            capabilities: crate::casting::RendererCapabilities {
                video: true,
                audio: true,
                image: true,
                playlists: true,
                controls: vec![
                    crate::casting::PlaybackAction::Play,
                    crate::casting::PlaybackAction::Pause,
                    crate::casting::PlaybackAction::Stop,
                ],
            },
            endpoint: crate::casting::RendererEndpoint::Dlna {
                control_url,
                location_url,
            },
        }
    }

    #[tokio::test]
    async fn renderer_cache_deduplicates_physical_tvs_and_sorts_by_name() {
        let cache = RendererCache::new();
        cache
            .replace(vec![
                renderer("uuid:z", "Bedroom", "TV", "192.168.1.10"),
                renderer("uuid:a", "Living Room", "TV", "192.168.1.20"),
                renderer("uuid:b", "Living Room", "TV", "192.168.1.20"),
            ])
            .await;

        let renderers = cache.snapshot().await;
        assert_eq!(renderers.len(), 2);
        assert_eq!(renderers[0].friendly_name, "Bedroom");
        assert_eq!(renderers[1].friendly_name, "Living Room");
    }
}

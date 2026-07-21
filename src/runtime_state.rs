//! Small bounded runtime registries. Media records and indexes remain owned by ReDB.

use crate::{state::SoapCacheKey, tv_control::DiscoveredTv};
use axum::body::Bytes;
use std::{
    collections::HashMap,
    hash::Hash,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub enum CastTransport {
    Dlna { control_url: String },
    Chromecast(Arc<crate::chromecast::client::ChromecastClient>),
    AirPlay { address: std::net::SocketAddr },
}

#[derive(Clone)]
pub struct CastSession {
    pub transport: CastTransport,
    pub operation: Arc<tokio::sync::Mutex<()>>,
    pub device: String,
    pub filename: String,
    last_activity: Instant,
}

impl CastSession {
    pub fn new(transport: CastTransport, device: String, filename: String) -> Self {
        Self {
            transport,
            operation: Arc::new(tokio::sync::Mutex::new(())),
            device,
            filename,
            last_activity: Instant::now(),
        }
    }
}

pub struct CastSessionRegistry {
    entries: HashMap<String, CastSession>,
}

impl CastSessionRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn insert(&mut self, target_id: String, session: CastSession) -> Option<CastSession> {
        self.prune();
        if self.entries.len() >= ACTIVE_CAST_MAX_ENTRIES && !self.entries.contains_key(&target_id) {
            if let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, session)| session.last_activity)
                .map(|(id, _)| id.clone())
            {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(target_id, session)
    }

    pub fn get(&mut self, target_id: &str) -> Option<CastSession> {
        self.prune();
        let session = self.entries.get_mut(target_id)?;
        session.last_activity = Instant::now();
        Some(session.clone())
    }

    pub fn remove(&mut self, target_id: &str) -> Option<CastSession> {
        self.entries.remove(target_id)
    }

    pub fn prune(&mut self) {
        self.entries
            .retain(|_, session| session.last_activity.elapsed() < ACTIVE_CAST_TTL);
    }

    pub fn drain(&mut self) -> Vec<CastSession> {
        self.entries.drain().map(|(_, session)| session).collect()
    }
}

impl Default for CastSessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

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
    renderers: Vec<DiscoveredTv>,
    refreshed_at: Option<Instant>,
}

/// A single shared renderer snapshot. The refresh mutex prevents concurrent
/// HTTP and MCP requests from launching duplicate three-second SSDP searches.
pub struct RendererCache {
    snapshot: tokio::sync::RwLock<RendererSnapshot>,
    refresh: tokio::sync::Mutex<()>,
    discovery: Option<Arc<crate::discovery::DiscoveryService>>,
}

impl RendererCache {
    pub fn new() -> Self {
        Self {
            snapshot: tokio::sync::RwLock::new(RendererSnapshot::default()),
            refresh: tokio::sync::Mutex::new(()),
            discovery: None,
        }
    }

    pub fn from_discovery(discovery: Arc<crate::discovery::DiscoveryService>) -> Self {
        Self {
            snapshot: tokio::sync::RwLock::new(RendererSnapshot::default()),
            refresh: tokio::sync::Mutex::new(()),
            discovery: Some(discovery),
        }
    }

    pub async fn snapshot(&self) -> Vec<DiscoveredTv> {
        if let Some(discovery) = &self.discovery {
            return discovery_renderers(discovery.targets().await);
        }
        self.snapshot.read().await.renderers.clone()
    }

    pub async fn name_for_ip(&self, ip: &str) -> Option<String> {
        if let Some(discovery) = &self.discovery {
            return discovery.name_for_ip(ip).await;
        }
        self.snapshot
            .read()
            .await
            .renderers
            .iter()
            .find(|renderer| renderer_ip(&renderer.location_url).as_deref() == Some(ip))
            .map(|renderer| renderer.friendly_name.clone())
    }

    pub async fn replace(&self, mut renderers: Vec<DiscoveredTv>) {
        renderers.sort_by(|left, right| left.id.cmp(&right.id));
        renderers.dedup_by(|left, right| left.id == right.id);
        renderers.truncate(RENDERER_CACHE_MAX_ENTRIES);
        *self.snapshot.write().await = RendererSnapshot {
            renderers,
            refreshed_at: Some(Instant::now()),
        };
    }

    pub async fn get_or_refresh(&self) -> anyhow::Result<Vec<DiscoveredTv>> {
        if let Some(discovery) = &self.discovery {
            return Ok(discovery_renderers(
                discovery.targets_or_refresh(RENDERER_CACHE_FRESH_TTL).await,
            ));
        }
        if let Some(renderers) = self.usable_snapshot(RENDERER_CACHE_FRESH_TTL).await {
            return Ok(renderers);
        }

        let _refresh_guard = self.refresh.lock().await;
        if let Some(renderers) = self.usable_snapshot(RENDERER_CACHE_FRESH_TTL).await {
            return Ok(renderers);
        }

        match crate::tv_control::discover_tvs().await {
            Ok(renderers) => {
                self.replace(renderers).await;
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

    pub async fn refresh(&self) -> anyhow::Result<Vec<DiscoveredTv>> {
        if let Some(discovery) = &self.discovery {
            return Ok(discovery_renderers(discovery.refresh().await));
        }
        let _refresh_guard = self.refresh.lock().await;
        let renderers = crate::tv_control::discover_tvs().await?;
        self.replace(renderers).await;
        Ok(self.snapshot().await)
    }

    async fn usable_snapshot(&self, ttl: Duration) -> Option<Vec<DiscoveredTv>> {
        let snapshot = self.snapshot.read().await;
        let refreshed_at = snapshot.refreshed_at?;
        if refreshed_at.elapsed() <= ttl {
            Some(snapshot.renderers.clone())
        } else {
            None
        }
    }
}

fn discovery_renderers(targets: Vec<crate::discovery::PlaybackTarget>) -> Vec<DiscoveredTv> {
    targets
        .into_iter()
        .filter(|target| target.kind == crate::discovery::TargetKind::Dlna)
        .filter_map(|target| {
            let control_url = target.control_url?;
            Some(DiscoveredTv {
                id: target.id,
                friendly_name: target.friendly_name,
                control_url,
                location_url: format!("http://{}", target.address),
                model_name: target.model.unwrap_or_default(),
            })
        })
        .collect()
}

impl Default for RendererCache {
    fn default() -> Self {
        Self::new()
    }
}

fn renderer_ip(url: &str) -> Option<String> {
    let authority = url.split("://").nth(1)?.split('/').next()?;
    if let Some(ipv6) = authority.strip_prefix('[') {
        return Some(ipv6.split(']').next()?.to_string());
    }
    Some(authority.split(':').next()?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_key(epoch: u64) -> SoapCacheKey {
        SoapCacheKey {
            object_id: "audio".to_string(),
            starting_index: 0,
            requested_count: 25,
            is_metadata: false,
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
}

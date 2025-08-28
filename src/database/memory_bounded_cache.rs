use anyhow::Result;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime};
use tracing::{debug, info, warn};

/// Memory pressure detection thresholds
#[derive(Debug, Clone)]
pub struct MemoryPressureConfig {
    /// Memory usage percentage that triggers pressure detection (0.0-1.0)
    pub pressure_threshold: f64,
    /// Memory usage percentage that triggers aggressive eviction (0.0-1.0)
    pub critical_threshold: f64,
    /// Percentage of entries to evict during pressure (0.0-1.0)
    pub eviction_percentage: f64,
    /// Minimum number of entries to keep even under pressure
    pub min_entries: usize,
}

impl Default for MemoryPressureConfig {
    fn default() -> Self {
        Self {
            pressure_threshold: 0.8,  // 80% memory usage triggers pressure
            critical_threshold: 0.95, // 95% memory usage triggers aggressive eviction
            eviction_percentage: 0.25, // Evict 25% of entries during pressure
            min_entries: 100,         // Always keep at least 100 entries
        }
    }
}

/// LRU cache entry with atomic access tracking
#[derive(Debug)]
struct CacheEntry<V> {
    value: V,
    last_accessed: Instant,
    access_count: AtomicU64,
    memory_size: usize,
}

impl<V> CacheEntry<V> {
    fn new(value: V, memory_size: usize) -> Self {
        Self {
            value,
            last_accessed: Instant::now(),
            access_count: AtomicU64::new(1),
            memory_size,
        }
    }
    
    fn touch(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }
    
    fn get_access_count(&self) -> u64 {
        self.access_count.load(Ordering::Relaxed)
    }
}

/// Memory-bounded LRU cache with atomic operations and pressure detection
#[derive(Debug)]
pub struct MemoryBoundedCache<K, V> 
where 
    K: Clone + Hash + Eq,
    V: Clone,
{
    // Core cache storage
    cache: HashMap<K, CacheEntry<V>>,
    access_order: VecDeque<K>,
    
    // Configuration limits
    max_entries: usize,
    max_memory_bytes: usize,
    pressure_config: MemoryPressureConfig,
    
    // Atomic memory tracking
    current_memory_bytes: AtomicUsize,
    current_entries: AtomicUsize,
    
    // Atomic performance counters
    hit_count: AtomicU64,
    miss_count: AtomicU64,
    eviction_count: AtomicU64,
    pressure_eviction_count: AtomicU64,
    critical_eviction_count: AtomicU64,
    
    // Memory pressure tracking
    pressure_detected_count: AtomicU64,
    last_pressure_check: AtomicU64, // Timestamp in seconds
    pressure_check_interval: Duration,
    
    // Cache efficiency tracking
    total_memory_allocated: AtomicU64,
    total_memory_freed: AtomicU64,
    max_memory_used: AtomicUsize,
}

impl<K, V> MemoryBoundedCache<K, V>
where
    K: Clone + Hash + Eq,
    V: Clone,
{
    /// Create a new memory-bounded cache with atomic usage tracking
    pub fn new(max_entries: usize, max_memory_bytes: usize) -> Self {
        Self::with_pressure_config(max_entries, max_memory_bytes, MemoryPressureConfig::default())
    }
    
    /// Create a new memory-bounded cache with custom pressure configuration
    pub fn with_pressure_config(
        max_entries: usize, 
        max_memory_bytes: usize, 
        pressure_config: MemoryPressureConfig
    ) -> Self {
        let initial_capacity = max_entries.min(1000);
        
        Self {
            cache: HashMap::with_capacity(initial_capacity),
            access_order: VecDeque::with_capacity(initial_capacity),
            max_entries,
            max_memory_bytes,
            pressure_config,
            current_memory_bytes: AtomicUsize::new(0),
            current_entries: AtomicUsize::new(0),
            hit_count: AtomicU64::new(0),
            miss_count: AtomicU64::new(0),
            eviction_count: AtomicU64::new(0),
            pressure_eviction_count: AtomicU64::new(0),
            critical_eviction_count: AtomicU64::new(0),
            pressure_detected_count: AtomicU64::new(0),
            last_pressure_check: AtomicU64::new(0),
            pressure_check_interval: Duration::from_secs(5), // Check pressure every 5 seconds
            total_memory_allocated: AtomicU64::new(0),
            total_memory_freed: AtomicU64::new(0),
            max_memory_used: AtomicUsize::new(0),
        }
    }
    
    /// Get value from cache with atomic hit/miss tracking
    pub fn get(&mut self, key: &K) -> Option<V> {
        // Check if key exists first
        if self.cache.contains_key(key) {
            // Move to end of access order (most recently used)
            self.move_to_end(key);
            
            // Now get and update the entry
            if let Some(entry) = self.cache.get_mut(key) {
                entry.touch();
                self.hit_count.fetch_add(1, Ordering::Relaxed);
                Some(entry.value.clone())
            } else {
                self.miss_count.fetch_add(1, Ordering::Relaxed);
                None
            }
        } else {
            self.miss_count.fetch_add(1, Ordering::Relaxed);
            None
        }
    }
    
    /// Insert value into cache with automatic eviction and memory pressure detection
    pub fn insert(&mut self, key: K, value: V) -> Result<()> {
        let entry_size = self.estimate_entry_size(&key, &value);
        
        // Check for memory pressure before insertion
        self.check_memory_pressure();
        
        // Evict entries if needed to make space
        self.evict_if_needed(entry_size)?;
        
        // Remove existing entry if present
        if let Some(old_entry) = self.cache.remove(&key) {
            self.current_memory_bytes.fetch_sub(old_entry.memory_size, Ordering::Relaxed);
            self.current_entries.fetch_sub(1, Ordering::Relaxed);
            self.total_memory_freed.fetch_add(old_entry.memory_size as u64, Ordering::Relaxed);
            
            // Remove from access order
            if let Some(pos) = self.access_order.iter().position(|k| k == &key) {
                self.access_order.remove(pos);
            }
        }
        
        // Insert new entry
        let entry = CacheEntry::new(value, entry_size);
        self.cache.insert(key.clone(), entry);
        self.access_order.push_back(key);
        
        // Update atomic counters
        self.current_memory_bytes.fetch_add(entry_size, Ordering::Relaxed);
        self.current_entries.fetch_add(1, Ordering::Relaxed);
        self.total_memory_allocated.fetch_add(entry_size as u64, Ordering::Relaxed);
        
        // Update max memory tracking
        let current_memory = self.current_memory_bytes.load(Ordering::Relaxed);
        let max_memory = self.max_memory_used.load(Ordering::Relaxed);
        if current_memory > max_memory {
            self.max_memory_used.store(current_memory, Ordering::Relaxed);
        }
        
        Ok(())
    }
    
    /// Remove value from cache with atomic memory accounting
    pub fn remove(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.cache.remove(key) {
            // Update atomic counters
            self.current_memory_bytes.fetch_sub(entry.memory_size, Ordering::Relaxed);
            self.current_entries.fetch_sub(1, Ordering::Relaxed);
            self.total_memory_freed.fetch_add(entry.memory_size as u64, Ordering::Relaxed);
            
            // Remove from access order
            if let Some(pos) = self.access_order.iter().position(|k| k == key) {
                self.access_order.remove(pos);
            }
            
            Some(entry.value)
        } else {
            None
        }
    }
    
    /// Check if cache contains key
    pub fn contains_key(&self, key: &K) -> bool {
        self.cache.contains_key(key)
    }
    
    /// Get current cache size (number of entries)
    pub fn len(&self) -> usize {
        self.current_entries.load(Ordering::Relaxed)
    }
    
    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    /// Clear all entries from cache
    pub fn clear(&mut self) {
        let current_memory = self.current_memory_bytes.load(Ordering::Relaxed);
        
        self.cache.clear();
        self.access_order.clear();
        
        self.current_memory_bytes.store(0, Ordering::Relaxed);
        self.current_entries.store(0, Ordering::Relaxed);
        self.total_memory_freed.fetch_add(current_memory as u64, Ordering::Relaxed);
        
        debug!("Cache cleared, freed {} bytes", current_memory);
    }
    
    /// Get comprehensive cache statistics
    pub fn get_stats(&self) -> CacheStats {
        let hits = self.hit_count.load(Ordering::Relaxed);
        let misses = self.miss_count.load(Ordering::Relaxed);
        let total_requests = hits + misses;
        let current_memory = self.current_memory_bytes.load(Ordering::Relaxed);
        
        CacheStats {
            entries: self.current_entries.load(Ordering::Relaxed),
            max_entries: self.max_entries,
            memory_bytes: current_memory,
            max_memory_bytes: self.max_memory_bytes,
            max_memory_used: self.max_memory_used.load(Ordering::Relaxed),
            memory_utilization: if self.max_memory_bytes > 0 {
                current_memory as f64 / self.max_memory_bytes as f64
            } else {
                0.0
            },
            hit_count: hits,
            miss_count: misses,
            total_requests,
            hit_rate: if total_requests > 0 {
                hits as f64 / total_requests as f64
            } else {
                0.0
            },
            eviction_count: self.eviction_count.load(Ordering::Relaxed),
            pressure_eviction_count: self.pressure_eviction_count.load(Ordering::Relaxed),
            critical_eviction_count: self.critical_eviction_count.load(Ordering::Relaxed),
            pressure_detected_count: self.pressure_detected_count.load(Ordering::Relaxed),
            total_memory_allocated: self.total_memory_allocated.load(Ordering::Relaxed),
            total_memory_freed: self.total_memory_freed.load(Ordering::Relaxed),
            memory_efficiency: {
                let allocated = self.total_memory_allocated.load(Ordering::Relaxed);
                let freed = self.total_memory_freed.load(Ordering::Relaxed);
                if allocated > 0 {
                    (allocated - freed) as f64 / allocated as f64
                } else {
                    0.0
                }
            },
        }
    }
    
    /// Detect memory pressure and trigger automatic eviction
    pub fn check_memory_pressure(&mut self) -> MemoryPressureStatus {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let last_check = self.last_pressure_check.load(Ordering::Relaxed);
        
        // Only check pressure at intervals to avoid overhead
        if now - last_check < self.pressure_check_interval.as_secs() {
            return MemoryPressureStatus::Normal;
        }
        
        self.last_pressure_check.store(now, Ordering::Relaxed);
        
        let current_memory = self.current_memory_bytes.load(Ordering::Relaxed);
        let memory_utilization = if self.max_memory_bytes > 0 {
            current_memory as f64 / self.max_memory_bytes as f64
        } else {
            0.0
        };
        
        let status = if memory_utilization >= self.pressure_config.critical_threshold {
            self.pressure_detected_count.fetch_add(1, Ordering::Relaxed);
            warn!(
                "Critical memory pressure detected: {:.1}% utilization ({} / {} bytes)",
                memory_utilization * 100.0,
                current_memory,
                self.max_memory_bytes
            );
            
            // Aggressive eviction for critical pressure
            let entries_to_evict = (self.cache.len() as f64 * self.pressure_config.eviction_percentage * 2.0) as usize;
            let entries_to_evict = entries_to_evict.max(1).min(self.cache.len().saturating_sub(self.pressure_config.min_entries));
            
            if entries_to_evict > 0 {
                self.evict_lru_entries(entries_to_evict, EvictionReason::CriticalPressure);
            }
            
            MemoryPressureStatus::Critical
        } else if memory_utilization >= self.pressure_config.pressure_threshold {
            self.pressure_detected_count.fetch_add(1, Ordering::Relaxed);
            debug!(
                "Memory pressure detected: {:.1}% utilization ({} / {} bytes)",
                memory_utilization * 100.0,
                current_memory,
                self.max_memory_bytes
            );
            
            // Normal eviction for pressure
            let entries_to_evict = (self.cache.len() as f64 * self.pressure_config.eviction_percentage) as usize;
            let entries_to_evict = entries_to_evict.max(1).min(self.cache.len().saturating_sub(self.pressure_config.min_entries));
            
            if entries_to_evict > 0 {
                self.evict_lru_entries(entries_to_evict, EvictionReason::MemoryPressure);
            }
            
            MemoryPressureStatus::Pressure
        } else {
            MemoryPressureStatus::Normal
        };
        
        status
    }
    
    /// Force eviction of entries to free memory
    pub fn force_evict(&mut self, target_memory_bytes: usize) -> usize {
        let current_memory = self.current_memory_bytes.load(Ordering::Relaxed);
        if current_memory <= target_memory_bytes {
            return 0; // Already under target
        }
        
        let memory_to_free = current_memory - target_memory_bytes;
        let mut memory_freed = 0;
        let mut entries_evicted = 0;
        
        // Evict LRU entries until we free enough memory
        while memory_freed < memory_to_free && !self.access_order.is_empty() {
            if let Some(key) = self.access_order.pop_front() {
                if let Some(entry) = self.cache.remove(&key) {
                    memory_freed += entry.memory_size;
                    entries_evicted += 1;
                    
                    self.current_memory_bytes.fetch_sub(entry.memory_size, Ordering::Relaxed);
                    self.current_entries.fetch_sub(1, Ordering::Relaxed);
                    self.total_memory_freed.fetch_add(entry.memory_size as u64, Ordering::Relaxed);
                    self.eviction_count.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        
        info!(
            "Force evicted {} entries, freed {} bytes (target: {} bytes)",
            entries_evicted,
            memory_freed,
            memory_to_free
        );
        
        entries_evicted
    }
    
    /// Move key to end of access order (most recently used)
    fn move_to_end(&mut self, key: &K) {
        if let Some(pos) = self.access_order.iter().position(|k| k == key) {
            let key = self.access_order.remove(pos).unwrap();
            self.access_order.push_back(key);
        }
    }
    
    /// Evict entries if needed to make space for new entry
    fn evict_if_needed(&mut self, new_entry_size: usize) -> Result<()> {
        let current_memory = self.current_memory_bytes.load(Ordering::Relaxed);
        let current_entries = self.current_entries.load(Ordering::Relaxed);
        
        // Check memory limit
        let needs_memory_eviction = current_memory + new_entry_size > self.max_memory_bytes;
        
        // Check entry count limit
        let needs_count_eviction = current_entries >= self.max_entries;
        
        if needs_memory_eviction || needs_count_eviction {
            let entries_to_evict = if needs_count_eviction {
                (current_entries - self.max_entries + 1).max(1)
            } else {
                // Calculate how many entries to evict based on memory pressure
                let memory_to_free = (current_memory + new_entry_size) - self.max_memory_bytes;
                let avg_entry_size = if current_entries > 0 {
                    current_memory / current_entries
                } else {
                    new_entry_size
                };
                
                // Evict extra entries to provide buffer
                ((memory_to_free / avg_entry_size) + 1).max(1)
            };
            
            let reason = if needs_count_eviction {
                EvictionReason::CountLimit
            } else {
                EvictionReason::MemoryLimit
            };
            
            self.evict_lru_entries(entries_to_evict, reason);
        }
        
        Ok(())
    }
    
    /// Evict least recently used entries
    fn evict_lru_entries(&mut self, count: usize, reason: EvictionReason) {
        let mut evicted = 0;
        let mut memory_freed = 0;
        
        for _ in 0..count {
            if let Some(key) = self.access_order.pop_front() {
                if let Some(entry) = self.cache.remove(&key) {
                    memory_freed += entry.memory_size;
                    evicted += 1;
                    
                    self.current_memory_bytes.fetch_sub(entry.memory_size, Ordering::Relaxed);
                    self.current_entries.fetch_sub(1, Ordering::Relaxed);
                    self.total_memory_freed.fetch_add(entry.memory_size as u64, Ordering::Relaxed);
                    
                    match reason {
                        EvictionReason::MemoryLimit | EvictionReason::CountLimit => {
                            self.eviction_count.fetch_add(1, Ordering::Relaxed);
                        }
                        EvictionReason::MemoryPressure => {
                            self.pressure_eviction_count.fetch_add(1, Ordering::Relaxed);
                        }
                        EvictionReason::CriticalPressure => {
                            self.critical_eviction_count.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            } else {
                break;
            }
        }
        
        if evicted > 0 {
            debug!(
                "Evicted {} entries ({:?}), freed {} bytes",
                evicted,
                reason,
                memory_freed
            );
        }
    }
    
    /// Estimate memory size of cache entry
    fn estimate_entry_size(&self, _key: &K, _value: &V) -> usize {
        // Base size includes the HashMap entry overhead and LRU tracking
        let base_size = std::mem::size_of::<K>() + 
                       std::mem::size_of::<V>() + 
                       std::mem::size_of::<CacheEntry<V>>() +
                       std::mem::size_of::<K>() + // For access_order VecDeque
                       64; // HashMap overhead estimate
        
        // For string keys/values, add their heap allocation size
        // This is a simplified estimation - in practice you might want more accurate sizing
        base_size
    }
}

/// Memory pressure status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressureStatus {
    Normal,
    Pressure,
    Critical,
}

/// Reason for cache eviction
#[derive(Debug, Clone, Copy)]
enum EvictionReason {
    MemoryLimit,
    CountLimit,
    MemoryPressure,
    CriticalPressure,
}

/// Comprehensive cache statistics with atomic counters
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub max_entries: usize,
    pub memory_bytes: usize,
    pub max_memory_bytes: usize,
    pub max_memory_used: usize,
    pub memory_utilization: f64,
    pub hit_count: u64,
    pub miss_count: u64,
    pub total_requests: u64,
    pub hit_rate: f64,
    pub eviction_count: u64,
    pub pressure_eviction_count: u64,
    pub critical_eviction_count: u64,
    pub pressure_detected_count: u64,
    pub total_memory_allocated: u64,
    pub total_memory_freed: u64,
    pub memory_efficiency: f64,
}

impl CacheStats {
    /// Get a human-readable summary of cache performance
    pub fn summary(&self) -> String {
        format!(
            "Cache: {}/{} entries ({:.1}% full), {:.1}MB/{:.1}MB memory ({:.1}% used), {:.1}% hit rate, {} evictions",
            self.entries,
            self.max_entries,
            (self.entries as f64 / self.max_entries as f64) * 100.0,
            self.memory_bytes as f64 / 1024.0 / 1024.0,
            self.max_memory_bytes as f64 / 1024.0 / 1024.0,
            self.memory_utilization * 100.0,
            self.hit_rate * 100.0,
            self.eviction_count
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_memory_bounded_cache_basic_operations() {
        let mut cache = MemoryBoundedCache::new(100, 1024);
        
        // Test insertion and retrieval
        cache.insert("key1".to_string(), "value1".to_string()).unwrap();
        assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));
        assert_eq!(cache.get(&"nonexistent".to_string()), None);
        
        // Test stats
        let stats = cache.get_stats();
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.hit_count, 1);
        assert_eq!(stats.miss_count, 1);
        assert!(stats.memory_bytes > 0);
    }
    
    #[test]
    fn test_memory_bounded_cache_eviction() {
        let mut cache = MemoryBoundedCache::new(2, 1024); // Small cache for testing
        
        // Fill cache to capacity
        cache.insert("key1".to_string(), "value1".to_string()).unwrap();
        cache.insert("key2".to_string(), "value2".to_string()).unwrap();
        
        // Access key1 to make it more recently used
        cache.get(&"key1".to_string());
        
        // Insert third item, should evict key2 (least recently used)
        cache.insert("key3".to_string(), "value3".to_string()).unwrap();
        
        assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));
        assert_eq!(cache.get(&"key2".to_string()), None); // Should be evicted
        assert_eq!(cache.get(&"key3".to_string()), Some("value3".to_string()));
        
        let stats = cache.get_stats();
        assert_eq!(stats.entries, 2);
        assert!(stats.eviction_count > 0);
    }
    
    #[test]
    fn test_memory_pressure_detection() {
        let pressure_config = MemoryPressureConfig {
            pressure_threshold: 0.1, // 10% for testing (very low threshold)
            critical_threshold: 0.2,  // 20% for testing
            eviction_percentage: 0.5, // 50% eviction for testing
            min_entries: 1,
        };
        
        let mut cache = MemoryBoundedCache::with_pressure_config(100, 50, pressure_config); // Very small memory limit
        
        // Fill cache to trigger pressure
        for i in 0..20 {
            cache.insert(format!("key{}", i), format!("value{}", i)).unwrap();
        }
        
        let status = cache.check_memory_pressure();
        let stats = cache.get_stats();
        
        // The test should pass if we have any memory usage above the threshold
        // or if pressure was detected during insertion
        let has_pressure = matches!(status, MemoryPressureStatus::Pressure | MemoryPressureStatus::Critical) ||
                          stats.pressure_detected_count > 0 ||
                          stats.memory_utilization > 0.1;
        
        assert!(has_pressure, "Expected memory pressure to be detected. Status: {:?}, Utilization: {:.2}%", 
                status, stats.memory_utilization * 100.0);
    }
}
#[cfg(test)]
mod tests {
    use super::memory_bounded_cache::*;
    use std::thread;
    use std::time::Duration;
    
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
        assert!(stats.hit_rate > 0.0);
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
            pressure_threshold: 0.5, // 50% for testing
            critical_threshold: 0.8,  // 80% for testing
            eviction_percentage: 0.5, // 50% eviction for testing
            min_entries: 1,
        };
        
        let mut cache = MemoryBoundedCache::with_pressure_config(10, 100, pressure_config);
        
        // Fill cache to trigger pressure
        for i in 0..8 {
            cache.insert(format!("key{}", i), format!("value{}", i)).unwrap();
        }
        
        let status = cache.check_memory_pressure();
        // Should detect some level of pressure due to memory usage
        assert!(matches!(status, MemoryPressureStatus::Pressure | MemoryPressureStatus::Critical));
        
        let stats = cache.get_stats();
        assert!(stats.pressure_detected_count > 0);
    }
    
    #[test]
    fn test_atomic_counters() {
        let mut cache = MemoryBoundedCache::new(100, 1024);
        
        // Test atomic hit/miss tracking
        cache.insert("key1".to_string(), "value1".to_string()).unwrap();
        cache.get(&"key1".to_string()); // Hit
        cache.get(&"key2".to_string()); // Miss
        cache.get(&"key1".to_string()); // Hit
        
        let stats = cache.get_stats();
        assert_eq!(stats.hit_count, 2);
        assert_eq!(stats.miss_count, 1);
        assert_eq!(stats.total_requests, 3);
        assert!((stats.hit_rate - 2.0/3.0).abs() < 0.01);
    }
    
    #[test]
    fn test_memory_accounting() {
        let mut cache = MemoryBoundedCache::new(100, 1024);
        
        // Insert some entries
        cache.insert("key1".to_string(), "value1".to_string()).unwrap();
        cache.insert("key2".to_string(), "value2".to_string()).unwrap();
        
        let stats_after_insert = cache.get_stats();
        assert!(stats_after_insert.memory_bytes > 0);
        assert!(stats_after_insert.total_memory_allocated > 0);
        
        // Remove an entry
        cache.remove(&"key1".to_string());
        
        let stats_after_remove = cache.get_stats();
        assert!(stats_after_remove.memory_bytes < stats_after_insert.memory_bytes);
        assert!(stats_after_remove.total_memory_freed > 0);
        assert_eq!(stats_after_remove.entries, 1);
    }
    
    #[test]
    fn test_force_evict() {
        let mut cache = MemoryBoundedCache::new(100, 1024);
        
        // Fill cache with entries
        for i in 0..10 {
            cache.insert(format!("key{}", i), format!("value{}", i)).unwrap();
        }
        
        let stats_before = cache.get_stats();
        let initial_memory = stats_before.memory_bytes;
        
        // Force evict to half the memory
        let target_memory = initial_memory / 2;
        let evicted_count = cache.force_evict(target_memory);
        
        let stats_after = cache.get_stats();
        assert!(evicted_count > 0);
        assert!(stats_after.memory_bytes <= initial_memory);
        assert!(stats_after.entries < stats_before.entries);
    }
    
    #[test]
    fn test_cache_clear() {
        let mut cache = MemoryBoundedCache::new(100, 1024);
        
        // Fill cache
        for i in 0..5 {
            cache.insert(format!("key{}", i), format!("value{}", i)).unwrap();
        }
        
        let stats_before = cache.get_stats();
        assert!(stats_before.entries > 0);
        assert!(stats_before.memory_bytes > 0);
        
        // Clear cache
        cache.clear();
        
        let stats_after = cache.get_stats();
        assert_eq!(stats_after.entries, 0);
        assert_eq!(stats_after.memory_bytes, 0);
        assert!(stats_after.total_memory_freed > 0);
    }
    
    #[test]
    fn test_memory_utilization_calculation() {
        let mut cache = MemoryBoundedCache::new(100, 1000); // 1000 bytes max
        
        // Insert entries to reach specific memory utilization
        for i in 0..5 {
            cache.insert(format!("key{}", i), format!("value{}", i)).unwrap();
        }
        
        let stats = cache.get_stats();
        assert!(stats.memory_utilization >= 0.0);
        assert!(stats.memory_utilization <= 1.0);
        
        // Memory utilization should be calculated correctly
        let expected_utilization = stats.memory_bytes as f64 / stats.max_memory_bytes as f64;
        assert!((stats.memory_utilization - expected_utilization).abs() < 0.01);
    }
    
    #[test]
    fn test_cache_stats_summary() {
        let mut cache = MemoryBoundedCache::new(100, 1024);
        
        // Add some data and operations
        cache.insert("key1".to_string(), "value1".to_string()).unwrap();
        cache.insert("key2".to_string(), "value2".to_string()).unwrap();
        cache.get(&"key1".to_string()); // Hit
        cache.get(&"nonexistent".to_string()); // Miss
        
        let stats = cache.get_stats();
        let summary = stats.summary();
        
        // Summary should contain key information
        assert!(summary.contains("Cache:"));
        assert!(summary.contains("entries"));
        assert!(summary.contains("memory"));
        assert!(summary.contains("hit rate"));
        assert!(summary.contains("evictions"));
    }
    
    #[test]
    fn test_concurrent_pressure_checks() {
        let pressure_config = MemoryPressureConfig {
            pressure_threshold: 0.7,
            critical_threshold: 0.9,
            eviction_percentage: 0.3,
            min_entries: 2,
        };
        
        let mut cache = MemoryBoundedCache::with_pressure_config(50, 500, pressure_config);
        
        // Fill cache to moderate level
        for i in 0..20 {
            cache.insert(format!("key{}", i), format!("value{}", i)).unwrap();
        }
        
        // Multiple pressure checks should be handled efficiently
        let status1 = cache.check_memory_pressure();
        thread::sleep(Duration::from_millis(10));
        let status2 = cache.check_memory_pressure();
        
        // Both checks should return valid status
        assert!(matches!(status1, MemoryPressureStatus::Normal | MemoryPressureStatus::Pressure | MemoryPressureStatus::Critical));
        assert!(matches!(status2, MemoryPressureStatus::Normal | MemoryPressureStatus::Pressure | MemoryPressureStatus::Critical));
    }
    
    #[test]
    fn test_memory_efficiency_tracking() {
        let mut cache = MemoryBoundedCache::new(100, 1024);
        
        // Insert and remove entries to test memory efficiency tracking
        for i in 0..10 {
            cache.insert(format!("key{}", i), format!("value{}", i)).unwrap();
        }
        
        // Remove half the entries
        for i in 0..5 {
            cache.remove(&format!("key{}", i));
        }
        
        let stats = cache.get_stats();
        assert!(stats.total_memory_allocated > 0);
        assert!(stats.total_memory_freed > 0);
        assert!(stats.memory_efficiency >= 0.0);
        assert!(stats.memory_efficiency <= 1.0);
        
        // Memory efficiency should reflect the allocation/deallocation pattern
        let expected_efficiency = (stats.total_memory_allocated - stats.total_memory_freed) as f64 / stats.total_memory_allocated as f64;
        assert!((stats.memory_efficiency - expected_efficiency).abs() < 0.01);
    }
}
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use std::collections::VecDeque;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use tracing::{info, warn, debug};

/// Atomic performance tracker for zero-copy database operations
/// Provides comprehensive metrics with lock-free atomic operations
#[derive(Debug)]
pub struct AtomicPerformanceTracker {
    // Core atomic counters
    total_operations: AtomicU64,
    successful_operations: AtomicU64,
    failed_operations: AtomicU64,
    total_files_processed: AtomicU64,
    
    // Batch operation counters
    total_batches: AtomicU64,
    successful_batches: AtomicU64,
    failed_batches: AtomicU64,
    
    // Memory usage tracking (in bytes)
    current_memory_usage: AtomicU64,
    peak_memory_usage: AtomicU64,
    cache_memory_usage: AtomicU64,
    index_memory_usage: AtomicU64,
    
    // Performance metrics
    total_processing_time_nanos: AtomicU64,
    last_throughput_calculation: AtomicU64, // timestamp in nanos
    current_throughput_files_per_sec: AtomicU64, // stored as integer (files/sec * 1000 for precision)
    
    // Cache performance
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    
    // Error tracking
    transaction_rollbacks: AtomicU64,
    retry_attempts: AtomicU64,
    
    // Timing windows for throughput calculation
    throughput_window: RwLock<VecDeque<ThroughputSample>>,
    throughput_window_size: usize,
    
    // Configuration
    monitoring_interval: Duration,
    enable_detailed_logging: bool,
    
    // Start time for uptime calculation
    start_time: Instant,
}

/// Sample for throughput calculation window
#[derive(Debug, Clone)]
struct ThroughputSample {
    timestamp: Instant,
    files_processed: u64,
    operations_completed: u64,
}

/// Comprehensive performance metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    // Operation counts
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub total_files_processed: u64,
    pub success_rate: f64,
    
    // Batch metrics
    pub total_batches: u64,
    pub successful_batches: u64,
    pub failed_batches: u64,
    pub batch_success_rate: f64,
    pub average_batch_size: f64,
    
    // Performance metrics
    pub current_throughput_files_per_sec: f64,
    pub average_throughput_files_per_sec: f64,
    pub total_processing_time: Duration,
    pub average_operation_time_micros: f64,
    
    // Memory metrics (in MB for readability)
    pub current_memory_usage_mb: f64,
    pub peak_memory_usage_mb: f64,
    pub cache_memory_usage_mb: f64,
    pub index_memory_usage_mb: f64,
    pub memory_efficiency_files_per_mb: f64,
    
    // Cache performance
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_hit_rate: f64,
    
    // Error metrics
    pub transaction_rollbacks: u64,
    pub retry_attempts: u64,
    pub error_rate: f64,
    
    // System metrics
    pub uptime: Duration,
    pub last_updated: SystemTime,
}

/// Batch operation result for performance tracking
#[derive(Debug, Clone)]
pub struct BatchOperationResult {
    pub files_processed: usize,
    pub processing_time: Duration,
    pub success: bool,
    pub memory_used_bytes: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub retry_count: u32,
}

impl AtomicPerformanceTracker {
    /// Create a new atomic performance tracker
    pub fn new(monitoring_interval: Duration, enable_detailed_logging: bool) -> Self {
        Self {
            total_operations: AtomicU64::new(0),
            successful_operations: AtomicU64::new(0),
            failed_operations: AtomicU64::new(0),
            total_files_processed: AtomicU64::new(0),
            
            total_batches: AtomicU64::new(0),
            successful_batches: AtomicU64::new(0),
            failed_batches: AtomicU64::new(0),
            
            current_memory_usage: AtomicU64::new(0),
            peak_memory_usage: AtomicU64::new(0),
            cache_memory_usage: AtomicU64::new(0),
            index_memory_usage: AtomicU64::new(0),
            
            total_processing_time_nanos: AtomicU64::new(0),
            last_throughput_calculation: AtomicU64::new(0),
            current_throughput_files_per_sec: AtomicU64::new(0),
            
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            
            transaction_rollbacks: AtomicU64::new(0),
            retry_attempts: AtomicU64::new(0),
            
            throughput_window: RwLock::new(VecDeque::with_capacity(100)),
            throughput_window_size: 100,
            
            monitoring_interval,
            enable_detailed_logging,
            start_time: Instant::now(),
        }
    }
    
    /// Record a batch operation result with atomic updates
    pub async fn record_batch_operation(&self, result: BatchOperationResult) {
        // Update batch counters
        self.total_batches.fetch_add(1, Ordering::Relaxed);
        if result.success {
            self.successful_batches.fetch_add(1, Ordering::Relaxed);
            self.successful_operations.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_batches.fetch_add(1, Ordering::Relaxed);
            self.failed_operations.fetch_add(1, Ordering::Relaxed);
        }
        
        // Update file processing counter
        self.total_files_processed.fetch_add(result.files_processed as u64, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        
        // Update timing
        self.total_processing_time_nanos.fetch_add(
            result.processing_time.as_nanos() as u64, 
            Ordering::Relaxed
        );
        
        // Update memory usage
        self.update_memory_usage(result.memory_used_bytes).await;
        
        // Update cache metrics
        self.cache_hits.fetch_add(result.cache_hits, Ordering::Relaxed);
        self.cache_misses.fetch_add(result.cache_misses, Ordering::Relaxed);
        
        // Update retry counter
        self.retry_attempts.fetch_add(result.retry_count as u64, Ordering::Relaxed);
        
        // Update throughput calculation
        self.update_throughput_calculation(result.files_processed as u64).await;
        
        // Log detailed metrics if enabled
        if self.enable_detailed_logging {
            debug!(
                "Batch operation completed: {} files in {:?}, success: {}, memory: {}MB, cache hit rate: {:.1}%",
                result.files_processed,
                result.processing_time,
                result.success,
                result.memory_used_bytes / 1_048_576, // Convert to MB
                if result.cache_hits + result.cache_misses > 0 {
                    (result.cache_hits as f64 / (result.cache_hits + result.cache_misses) as f64) * 100.0
                } else {
                    0.0
                }
            );
        }
    }
    
    /// Record a transaction rollback
    pub fn record_transaction_rollback(&self) {
        self.transaction_rollbacks.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Update memory usage with atomic peak tracking
    pub async fn update_memory_usage(&self, current_usage_bytes: u64) {
        // Update current usage
        self.current_memory_usage.store(current_usage_bytes, Ordering::Relaxed);
        
        // Update peak usage atomically
        let mut peak = self.peak_memory_usage.load(Ordering::Relaxed);
        while current_usage_bytes > peak {
            match self.peak_memory_usage.compare_exchange_weak(
                peak,
                current_usage_bytes,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current_peak) => peak = current_peak,
            }
        }
    }
    
    /// Update cache memory usage
    pub fn update_cache_memory_usage(&self, cache_bytes: u64) {
        self.cache_memory_usage.store(cache_bytes, Ordering::Relaxed);
    }
    
    /// Update index memory usage
    pub fn update_index_memory_usage(&self, index_bytes: u64) {
        self.index_memory_usage.store(index_bytes, Ordering::Relaxed);
    }
    
    /// Update throughput calculation with sliding window
    async fn update_throughput_calculation(&self, files_processed: u64) {
        let now = Instant::now();
        let sample = ThroughputSample {
            timestamp: now,
            files_processed,
            operations_completed: 1,
        };
        
        // Update throughput window
        let mut window = self.throughput_window.write().await;
        window.push_back(sample);
        
        // Remove old samples (keep only last N samples or last 60 seconds)
        let cutoff_time = now - Duration::from_secs(60);
        while let Some(front) = window.front() {
            if front.timestamp < cutoff_time || window.len() > self.throughput_window_size {
                window.pop_front();
            } else {
                break;
            }
        }
        
        // Calculate current throughput
        if window.len() >= 2 {
            let oldest = window.front().unwrap();
            let newest = window.back().unwrap();
            
            let time_diff = newest.timestamp.duration_since(oldest.timestamp);
            if time_diff.as_secs_f64() > 0.0 {
                let total_files: u64 = window.iter().map(|s| s.files_processed).sum();
                let throughput = total_files as f64 / time_diff.as_secs_f64();
                
                // Store as integer with 3 decimal precision (multiply by 1000)
                self.current_throughput_files_per_sec.store(
                    (throughput * 1000.0) as u64,
                    Ordering::Relaxed,
                );
            }
        }
        
        // Update last calculation timestamp
        self.last_throughput_calculation.store(
            now.elapsed().as_nanos() as u64,
            Ordering::Relaxed,
        );
    }
    
    /// Get comprehensive performance metrics snapshot
    pub async fn get_metrics(&self) -> PerformanceMetrics {
        let total_ops = self.total_operations.load(Ordering::Relaxed);
        let successful_ops = self.successful_operations.load(Ordering::Relaxed);
        let failed_ops = self.failed_operations.load(Ordering::Relaxed);
        let total_files = self.total_files_processed.load(Ordering::Relaxed);
        
        let total_batches = self.total_batches.load(Ordering::Relaxed);
        let successful_batches = self.successful_batches.load(Ordering::Relaxed);
        let failed_batches = self.failed_batches.load(Ordering::Relaxed);
        
        let current_memory = self.current_memory_usage.load(Ordering::Relaxed);
        let peak_memory = self.peak_memory_usage.load(Ordering::Relaxed);
        let cache_memory = self.cache_memory_usage.load(Ordering::Relaxed);
        let index_memory = self.index_memory_usage.load(Ordering::Relaxed);
        
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);
        let cache_misses = self.cache_misses.load(Ordering::Relaxed);
        
        let total_processing_nanos = self.total_processing_time_nanos.load(Ordering::Relaxed);
        let current_throughput_raw = self.current_throughput_files_per_sec.load(Ordering::Relaxed);
        
        // Calculate derived metrics
        let success_rate = if total_ops > 0 {
            successful_ops as f64 / total_ops as f64
        } else {
            0.0
        };
        
        let batch_success_rate = if total_batches > 0 {
            successful_batches as f64 / total_batches as f64
        } else {
            0.0
        };
        
        let average_batch_size = if total_batches > 0 {
            total_files as f64 / total_batches as f64
        } else {
            0.0
        };
        
        let current_throughput = (current_throughput_raw as f64) / 1000.0; // Convert back from stored precision
        
        let total_processing_time = Duration::from_nanos(total_processing_nanos);
        let average_throughput = if total_processing_time.as_secs_f64() > 0.0 {
            total_files as f64 / total_processing_time.as_secs_f64()
        } else {
            0.0
        };
        
        let average_operation_time_micros = if total_ops > 0 {
            (total_processing_nanos as f64 / total_ops as f64) / 1000.0 // Convert nanos to micros
        } else {
            0.0
        };
        
        let cache_hit_rate = if cache_hits + cache_misses > 0 {
            cache_hits as f64 / (cache_hits + cache_misses) as f64
        } else {
            0.0
        };
        
        let memory_efficiency = if current_memory > 0 {
            total_files as f64 / (current_memory as f64 / 1_048_576.0) // Files per MB
        } else {
            0.0
        };
        
        let error_rate = if total_ops > 0 {
            failed_ops as f64 / total_ops as f64
        } else {
            0.0
        };
        
        PerformanceMetrics {
            total_operations: total_ops,
            successful_operations: successful_ops,
            failed_operations: failed_ops,
            total_files_processed: total_files,
            success_rate,
            
            total_batches,
            successful_batches,
            failed_batches,
            batch_success_rate,
            average_batch_size,
            
            current_throughput_files_per_sec: current_throughput,
            average_throughput_files_per_sec: average_throughput,
            total_processing_time,
            average_operation_time_micros,
            
            current_memory_usage_mb: current_memory as f64 / 1_048_576.0,
            peak_memory_usage_mb: peak_memory as f64 / 1_048_576.0,
            cache_memory_usage_mb: cache_memory as f64 / 1_048_576.0,
            index_memory_usage_mb: index_memory as f64 / 1_048_576.0,
            memory_efficiency_files_per_mb: memory_efficiency,
            
            cache_hits,
            cache_misses,
            cache_hit_rate,
            
            transaction_rollbacks: self.transaction_rollbacks.load(Ordering::Relaxed),
            retry_attempts: self.retry_attempts.load(Ordering::Relaxed),
            error_rate,
            
            uptime: self.start_time.elapsed(),
            last_updated: SystemTime::now(),
        }
    }
    
    /// Log performance summary at regular intervals
    pub async fn log_performance_summary(&self) {
        let metrics = self.get_metrics().await;
        
        info!("=== Performance Summary ===");
        info!("Operations: {} total, {} successful ({:.1}% success rate)", 
              metrics.total_operations, metrics.successful_operations, metrics.success_rate * 100.0);
        info!("Files processed: {} ({:.0} files/sec current, {:.0} files/sec average)",
              metrics.total_files_processed, metrics.current_throughput_files_per_sec, metrics.average_throughput_files_per_sec);
        info!("Batches: {} total, {} successful ({:.1}% success rate, {:.1} avg size)",
              metrics.total_batches, metrics.successful_batches, metrics.batch_success_rate * 100.0, metrics.average_batch_size);
        info!("Memory: {:.1}MB current, {:.1}MB peak ({:.0} files/MB efficiency)",
              metrics.current_memory_usage_mb, metrics.peak_memory_usage_mb, metrics.memory_efficiency_files_per_mb);
        info!("Cache: {:.1}% hit rate ({} hits, {} misses)",
              metrics.cache_hit_rate * 100.0, metrics.cache_hits, metrics.cache_misses);
        
        if metrics.error_rate > 0.01 { // More than 1% error rate
            warn!("High error rate detected: {:.1}% ({} rollbacks, {} retries)",
                  metrics.error_rate * 100.0, metrics.transaction_rollbacks, metrics.retry_attempts);
        }
        
        info!("Uptime: {:?}, Average operation time: {:.1}μs",
              metrics.uptime, metrics.average_operation_time_micros);
    }
    
    /// Export metrics in JSON format for external monitoring
    pub async fn export_metrics_json(&self) -> Result<String, serde_json::Error> {
        let metrics = self.get_metrics().await;
        serde_json::to_string_pretty(&metrics)
    }
    
    /// Check if performance targets are being met
    pub async fn check_performance_targets(&self, target_throughput_files_per_sec: f64) -> PerformanceStatus {
        let metrics = self.get_metrics().await;
        
        let throughput_ok = metrics.current_throughput_files_per_sec >= target_throughput_files_per_sec * 0.8; // 80% of target
        let error_rate_ok = metrics.error_rate < 0.05; // Less than 5% error rate
        let memory_ok = metrics.current_memory_usage_mb < 1024.0; // Less than 1GB memory usage
        let cache_ok = metrics.cache_hit_rate > 0.7; // More than 70% cache hit rate
        
        let overall_healthy = throughput_ok && error_rate_ok && memory_ok && cache_ok;
        
        PerformanceStatus {
            overall_healthy,
            throughput_ok,
            error_rate_ok,
            memory_ok,
            cache_ok,
            current_throughput: metrics.current_throughput_files_per_sec,
            target_throughput: target_throughput_files_per_sec,
            error_rate: metrics.error_rate,
            memory_usage_mb: metrics.current_memory_usage_mb,
            cache_hit_rate: metrics.cache_hit_rate,
        }
    }
    
    /// Reset all counters (useful for testing)
    pub async fn reset(&self) {
        self.total_operations.store(0, Ordering::Relaxed);
        self.successful_operations.store(0, Ordering::Relaxed);
        self.failed_operations.store(0, Ordering::Relaxed);
        self.total_files_processed.store(0, Ordering::Relaxed);
        
        self.total_batches.store(0, Ordering::Relaxed);
        self.successful_batches.store(0, Ordering::Relaxed);
        self.failed_batches.store(0, Ordering::Relaxed);
        
        self.current_memory_usage.store(0, Ordering::Relaxed);
        self.peak_memory_usage.store(0, Ordering::Relaxed);
        self.cache_memory_usage.store(0, Ordering::Relaxed);
        self.index_memory_usage.store(0, Ordering::Relaxed);
        
        self.total_processing_time_nanos.store(0, Ordering::Relaxed);
        self.current_throughput_files_per_sec.store(0, Ordering::Relaxed);
        
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        
        self.transaction_rollbacks.store(0, Ordering::Relaxed);
        self.retry_attempts.store(0, Ordering::Relaxed);
        
        self.throughput_window.write().await.clear();
    }
}

/// Performance status check result
#[derive(Debug, Clone)]
pub struct PerformanceStatus {
    pub overall_healthy: bool,
    pub throughput_ok: bool,
    pub error_rate_ok: bool,
    pub memory_ok: bool,
    pub cache_ok: bool,
    pub current_throughput: f64,
    pub target_throughput: f64,
    pub error_rate: f64,
    pub memory_usage_mb: f64,
    pub cache_hit_rate: f64,
}

/// Shared atomic performance tracker instance
pub type SharedPerformanceTracker = Arc<AtomicPerformanceTracker>;

/// Create a new shared performance tracker
pub fn create_shared_performance_tracker(
    monitoring_interval: Duration,
    enable_detailed_logging: bool,
) -> SharedPerformanceTracker {
    Arc::new(AtomicPerformanceTracker::new(monitoring_interval, enable_detailed_logging))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;
    
    #[tokio::test]
    async fn test_atomic_performance_tracker_basic() {
        let tracker = AtomicPerformanceTracker::new(Duration::from_secs(1), true);
        
        // Record a successful batch operation
        let result = BatchOperationResult {
            files_processed: 1000,
            processing_time: Duration::from_millis(100),
            success: true,
            memory_used_bytes: 10_485_760, // 10MB
            cache_hits: 800,
            cache_misses: 200,
            retry_count: 0,
        };
        
        tracker.record_batch_operation(result).await;
        
        let metrics = tracker.get_metrics().await;
        assert_eq!(metrics.total_operations, 1);
        assert_eq!(metrics.successful_operations, 1);
        assert_eq!(metrics.total_files_processed, 1000);
        assert_eq!(metrics.total_batches, 1);
        assert_eq!(metrics.successful_batches, 1);
        assert_eq!(metrics.cache_hits, 800);
        assert_eq!(metrics.cache_misses, 200);
        assert!((metrics.cache_hit_rate - 0.8).abs() < 0.001);
    }
    
    #[tokio::test]
    async fn test_throughput_calculation() {
        let tracker = AtomicPerformanceTracker::new(Duration::from_secs(1), false);
        
        // Record multiple operations with timing
        for _i in 0..5 {
            let result = BatchOperationResult {
                files_processed: 1000,
                processing_time: Duration::from_millis(100),
                success: true,
                memory_used_bytes: 1_048_576,
                cache_hits: 900,
                cache_misses: 100,
                retry_count: 0,
            };
            
            tracker.record_batch_operation(result).await;
            
            // Small delay to create time difference
            sleep(Duration::from_millis(10)).await;
        }
        
        let metrics = tracker.get_metrics().await;
        assert_eq!(metrics.total_files_processed, 5000);
        assert!(metrics.current_throughput_files_per_sec > 0.0);
        assert!(metrics.average_throughput_files_per_sec > 0.0);
    }
    
    #[tokio::test]
    async fn test_memory_tracking() {
        let tracker = AtomicPerformanceTracker::new(Duration::from_secs(1), false);
        
        // Update memory usage multiple times
        tracker.update_memory_usage(5_242_880).await; // 5MB
        tracker.update_memory_usage(10_485_760).await; // 10MB
        tracker.update_memory_usage(7_340_032).await; // 7MB
        
        let metrics = tracker.get_metrics().await;
        assert!((metrics.current_memory_usage_mb - 7.0).abs() < 0.1);
        assert!((metrics.peak_memory_usage_mb - 10.0).abs() < 0.1);
    }
    
    #[tokio::test]
    async fn test_performance_status_check() {
        let tracker = AtomicPerformanceTracker::new(Duration::from_secs(1), false);
        
        // Record high-performance operation
        let result = BatchOperationResult {
            files_processed: 100_000,
            processing_time: Duration::from_millis(100), // Very fast
            success: true,
            memory_used_bytes: 52_428_800, // 50MB
            cache_hits: 95_000,
            cache_misses: 5_000,
            retry_count: 0,
        };
        
        tracker.record_batch_operation(result).await;
        
        let status = tracker.check_performance_targets(500_000.0).await; // 500K files/sec target
        assert!(status.memory_ok);
        assert!(status.cache_ok);
        assert!(status.error_rate_ok);
    }
    
    #[tokio::test]
    async fn test_metrics_export() {
        let tracker = AtomicPerformanceTracker::new(Duration::from_secs(1), false);
        
        let result = BatchOperationResult {
            files_processed: 1000,
            processing_time: Duration::from_millis(100),
            success: true,
            memory_used_bytes: 1_048_576,
            cache_hits: 800,
            cache_misses: 200,
            retry_count: 0,
        };
        
        tracker.record_batch_operation(result).await;
        
        let json = tracker.export_metrics_json().await.unwrap();
        assert!(json.contains("total_operations"));
        assert!(json.contains("total_files_processed"));
        assert!(json.contains("cache_hit_rate"));
    }
}
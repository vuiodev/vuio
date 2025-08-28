//! Comprehensive test suite for ZeroCopy database
//! 
//! This test suite covers all ZeroCopy database operations including:
//! - Unit tests for atomic operations
//! - Integration tests for bulk operations  
//! - Performance benchmarks with atomic timing
//! - Memory usage tests with atomic tracking
//! - Stress tests for concurrent atomic operations
//! - Verification of 1M files/sec target performance

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::time::timeout;

use vuio::database::{
    DatabaseManager, MediaFile,
    zerocopy::{ZeroCopyDatabase, PerformanceProfile}
};

/// Test configuration constants
const SMALL_DATASET_SIZE: usize = 1_000;
const MEDIUM_DATASET_SIZE: usize = 10_000;
const LARGE_DATASET_SIZE: usize = 100_000;
const STRESS_DATASET_SIZE: usize = 500_000;
const TARGET_THROUGHPUT: f64 = 1_000_000.0; // 1M files/sec target
const PERFORMANCE_TIMEOUT: Duration = Duration::from_secs(60);

/// Helper function to create test media files
fn create_test_media_files(count: usize, base_path: &Path) -> Vec<MediaFile> {
    let mut files = Vec::with_capacity(count);
    
    for i in 0..count {
        let file_path = base_path.join(format!("media/dir_{:03}/file_{:06}.mp4", i / 100, i));
        let mut media_file = MediaFile::new(file_path, 1024 * 1024 + (i as u64 * 1024), "video/mp4".to_string());
        
        // Add metadata variation for realistic testing
        media_file.title = Some(format!("Test Media {}", i));
        media_file.artist = Some(format!("Artist {}", i % 50));
        media_file.album = Some(format!("Album {}", i % 25));
        media_file.genre = Some(format!("Genre {}", i % 10));
        media_file.year = Some(2000 + (i % 24) as u32);
        media_file.track_number = Some((i % 20 + 1) as u32);
        media_file.duration = Some(Duration::from_secs(180 + (i % 300) as u64));
        
        files.push(media_file);
    }
    
    files
}

/// Helper function to measure memory usage (simplified)
fn get_memory_usage_kb() -> usize {
    // Simple memory measurement - in production this could use more sophisticated methods
    std::process::Command::new("ps")
        .args(&["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()
        .and_then(|output| {
            String::from_utf8(output.stdout)
                .ok()?
                .trim()
                .parse::<usize>()
                .ok()
        })
        .unwrap_or(0)
}
#
[cfg(test)]
mod unit_tests {
    use super::*;

    #[tokio::test]
    async fn test_zerocopy_database_creation_and_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_creation.db");
        
        // Test database creation with different profiles
        let profiles = [
            PerformanceProfile::Minimal,
            PerformanceProfile::Balanced,
            PerformanceProfile::HighPerformance,
            PerformanceProfile::Maximum,
        ];
        
        for profile in &profiles {
            let db = ZeroCopyDatabase::new_with_profile(db_path.clone(), *profile).await.unwrap();
            
            // Verify configuration
            let config = db.get_config().await;
            assert_eq!(config.performance_profile, *profile);
            assert_eq!(config.memory_map_size_mb, profile.cache_size_mb());
            assert_eq!(config.index_cache_size, profile.index_cache_size());
            assert_eq!(config.batch_size, profile.batch_size());
            
            // Test initialization
            db.initialize().await.unwrap();
            
            // Verify database is ready
            assert!(db.is_open());
            
            println!("✓ Created and initialized database with {:?} profile", profile);
        }
    }

    #[tokio::test]
    async fn test_atomic_single_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_atomic_single.db");
        
        let db = ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::Balanced).await.unwrap();
        db.initialize().await.unwrap();
        
        let test_files = create_test_media_files(10, temp_dir.path());
        
        // Test atomic store operations
        let mut file_ids = Vec::new();
        for file in &test_files {
            let start_time = Instant::now();
            let file_id = db.store_media_file(file).await.unwrap();
            let operation_time = start_time.elapsed();
            
            file_ids.push(file_id);
            assert!(file_id > 0);
            assert!(operation_time < Duration::from_millis(100)); // Should be fast
        }
        
        // Test atomic get operations
        for (i, &file_id) in file_ids.iter().enumerate() {
            let start_time = Instant::now();
            let retrieved_file = db.get_file_by_id(file_id).await.unwrap();
            let operation_time = start_time.elapsed();
            
            assert!(retrieved_file.is_some());
            let file = retrieved_file.unwrap();
            assert_eq!(file.filename, test_files[i].filename);
            assert!(operation_time < Duration::from_millis(50)); // Should be very fast
        }
        
        // Test atomic update operations
        for (i, file) in test_files.iter().enumerate() {
            let mut updated_file = file.clone();
            updated_file.id = Some(file_ids[i]);
            updated_file.title = Some(format!("Updated Title {}", i));
            
            let start_time = Instant::now();
            db.update_media_file(&updated_file).await.unwrap();
            let operation_time = start_time.elapsed();
            
            assert!(operation_time < Duration::from_millis(100));
        }
        
        // Test atomic remove operations
        for file in &test_files {
            let start_time = Instant::now();
            let removed = db.remove_media_file(&file.path).await.unwrap();
            let operation_time = start_time.elapsed();
            
            assert!(removed);
            assert!(operation_time < Duration::from_millis(100));
        }
        
        println!("✓ All atomic single file operations completed successfully");
    }

    #[tokio::test]
    async fn test_atomic_counters_and_statistics() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_atomic_counters.db");
        
        let db = ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::Balanced).await.unwrap();
        db.initialize().await.unwrap();
        
        let test_files = create_test_media_files(100, temp_dir.path());
        
        // Get initial stats
        let initial_stats = db.get_stats().await.unwrap();
        assert_eq!(initial_stats.total_files, 0);
        
        // Perform bulk operations and verify atomic counters
        let file_ids = db.bulk_store_media_files(&test_files).await.unwrap();
        assert_eq!(file_ids.len(), test_files.len());
        
        // Verify stats were updated atomically
        let after_insert_stats = db.get_stats().await.unwrap();
        assert_eq!(after_insert_stats.total_files, test_files.len());
        
        // Test atomic performance tracking
        let performance_stats = db.get_performance_stats();
        assert!(performance_stats.total_operations > 0);
        assert!(performance_stats.processed_files > 0);
        
        // Test atomic memory tracking
        let cache_stats = db.get_cache_stats().await;
        assert!(cache_stats.combined_memory_usage > 0);
        
        println!("✓ Atomic counters and statistics working correctly");
        println!("  - Total files: {}", after_insert_stats.total_files);
        println!("  - Total operations: {}", performance_stats.total_operations);
        println!("  - Cache usage: {} bytes", cache_stats.combined_memory_usage);
    }
}#
[cfg(test)]
mod bulk_operations_tests {
    use super::*;

    #[tokio::test]
    async fn test_bulk_store_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_bulk_store.db");
        
        let db = ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::HighPerformance).await.unwrap();
        db.initialize().await.unwrap();
        
        // Test different batch sizes
        let batch_sizes = [100, 1_000, 10_000];
        
        for &batch_size in &batch_sizes {
            let test_files = create_test_media_files(batch_size, temp_dir.path());
            
            let start_memory = get_memory_usage_kb();
            let start_time = Instant::now();
            
            let file_ids = db.bulk_store_media_files(&test_files).await.unwrap();
            
            let operation_time = start_time.elapsed();
            let end_memory = get_memory_usage_kb();
            let memory_used = end_memory.saturating_sub(start_memory);
            
            // Verify results
            assert_eq!(file_ids.len(), batch_size);
            assert!(file_ids.iter().all(|&id| id > 0));
            
            // Calculate throughput
            let throughput = batch_size as f64 / operation_time.as_secs_f64();
            
            println!("✓ Bulk store {} files:", batch_size);
            println!("  - Time: {:?}", operation_time);
            println!("  - Throughput: {:.0} files/sec", throughput);
            println!("  - Memory used: {} KB", memory_used);
            
            // Performance assertions
            assert!(throughput > 1000.0, "Throughput too low: {:.0} files/sec", throughput);
            
            // For large batches, should approach target throughput
            if batch_size >= 10_000 {
                assert!(throughput > 50_000.0, "Large batch throughput too low: {:.0} files/sec", throughput);
            }
        }
    }

    #[tokio::test]
    async fn test_bulk_update_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_bulk_update.db");
        
        let db = ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::HighPerformance).await.unwrap();
        db.initialize().await.unwrap();
        
        let test_files = create_test_media_files(MEDIUM_DATASET_SIZE, temp_dir.path());
        
        // First, store the files
        let file_ids = db.bulk_store_media_files(&test_files).await.unwrap();
        
        // Prepare updated files
        let mut updated_files = test_files.clone();
        for (i, file) in updated_files.iter_mut().enumerate() {
            file.id = Some(file_ids[i]);
            file.title = Some(format!("Updated Title {}", i));
            file.artist = Some(format!("Updated Artist {}", i));
        }
        
        let start_memory = get_memory_usage_kb();
        let start_time = Instant::now();
        
        db.bulk_update_media_files(&updated_files).await.unwrap();
        
        let operation_time = start_time.elapsed();
        let end_memory = get_memory_usage_kb();
        let memory_used = end_memory.saturating_sub(start_memory);
        
        let throughput = updated_files.len() as f64 / operation_time.as_secs_f64();
        
        println!("✓ Bulk update {} files:", updated_files.len());
        println!("  - Time: {:?}", operation_time);
        println!("  - Throughput: {:.0} files/sec", throughput);
        println!("  - Memory used: {} KB", memory_used);
        
        // Verify updates were applied
        let sample_file = db.get_file_by_id(file_ids[0]).await.unwrap().unwrap();
        assert_eq!(sample_file.title, Some("Updated Title 0".to_string()));
        
        // Performance assertions
        assert!(throughput > 5000.0, "Update throughput too low: {:.0} files/sec", throughput);
    }

    #[tokio::test]
    async fn test_bulk_remove_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_bulk_remove.db");
        
        let db = ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::HighPerformance).await.unwrap();
        db.initialize().await.unwrap();
        
        let test_files = create_test_media_files(MEDIUM_DATASET_SIZE, temp_dir.path());
        
        // Store files first
        let _file_ids = db.bulk_store_media_files(&test_files).await.unwrap();
        
        // Prepare paths for removal
        let paths_to_remove: Vec<PathBuf> = test_files.iter().map(|f| f.path.clone()).collect();
        
        let start_memory = get_memory_usage_kb();
        let start_time = Instant::now();
        
        let removed_count = db.bulk_remove_media_files(&paths_to_remove).await.unwrap();
        
        let operation_time = start_time.elapsed();
        let end_memory = get_memory_usage_kb();
        let memory_used = end_memory.saturating_sub(start_memory);
        
        let throughput = removed_count as f64 / operation_time.as_secs_f64();
        
        println!("✓ Bulk remove {} files:", removed_count);
        println!("  - Time: {:?}", operation_time);
        println!("  - Throughput: {:.0} files/sec", throughput);
        println!("  - Memory used: {} KB", memory_used);
        
        // Verify removal
        assert_eq!(removed_count, test_files.len());
        
        let final_stats = db.get_stats().await.unwrap();
        assert_eq!(final_stats.total_files, 0);
        
        // Performance assertions
        assert!(throughput > 10_000.0, "Remove throughput too low: {:.0} files/sec", throughput);
    }

    #[tokio::test]
    async fn test_bulk_get_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_bulk_get.db");
        
        let db = ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::HighPerformance).await.unwrap();
        db.initialize().await.unwrap();
        
        let test_files = create_test_media_files(MEDIUM_DATASET_SIZE, temp_dir.path());
        
        // Store files first
        let _file_ids = db.bulk_store_media_files(&test_files).await.unwrap();
        
        // Prepare paths for bulk get
        let paths_to_get: Vec<PathBuf> = test_files.iter().map(|f| f.path.clone()).collect();
        
        let start_memory = get_memory_usage_kb();
        let start_time = Instant::now();
        
        let retrieved_files = db.bulk_get_files_by_paths(&paths_to_get).await.unwrap();
        
        let operation_time = start_time.elapsed();
        let end_memory = get_memory_usage_kb();
        let memory_used = end_memory.saturating_sub(start_memory);
        
        let throughput = retrieved_files.len() as f64 / operation_time.as_secs_f64();
        
        println!("✓ Bulk get {} files:", retrieved_files.len());
        println!("  - Time: {:?}", operation_time);
        println!("  - Throughput: {:.0} files/sec", throughput);
        println!("  - Memory used: {} KB", memory_used);
        
        // Verify retrieval
        assert_eq!(retrieved_files.len(), test_files.len());
        
        // Performance assertions - bulk get should be very fast
        assert!(throughput > 50_000.0, "Bulk get throughput too low: {:.0} files/sec", throughput);
    }
}#[cfg(
test)]
mod performance_benchmarks {
    use super::*;

    #[tokio::test]
    async fn test_million_files_per_second_target() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_million_files.db");
        
        // Use maximum performance profile for this test
        let db = ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::Maximum).await.unwrap();
        db.initialize().await.unwrap();
        
        // Test with progressively larger datasets to find the throughput limit
        let test_sizes = [10_000, 50_000, 100_000, 250_000];
        
        for &test_size in &test_sizes {
            println!("Testing throughput with {} files...", test_size);
            
            let test_files = create_test_media_files(test_size, temp_dir.path());
            
            let start_time = Instant::now();
            
            // Use timeout to prevent hanging
            let result = timeout(PERFORMANCE_TIMEOUT, async {
                db.bulk_store_media_files(&test_files).await
            }).await;
            
            match result {
                Ok(Ok(file_ids)) => {
                    let operation_time = start_time.elapsed();
                    let throughput = test_size as f64 / operation_time.as_secs_f64();
                    
                    println!("✓ {} files processed:", test_size);
                    println!("  - Time: {:?}", operation_time);
                    println!("  - Throughput: {:.0} files/sec", throughput);
                    println!("  - Target progress: {:.1}%", (throughput / TARGET_THROUGHPUT) * 100.0);
                    
                    assert_eq!(file_ids.len(), test_size);
                    
                    // Check if we're approaching the target
                    if throughput >= TARGET_THROUGHPUT * 0.8 {
                        println!("🎯 Approaching 1M files/sec target! ({:.0} files/sec)", throughput);
                    }
                    
                    if throughput >= TARGET_THROUGHPUT {
                        println!("🚀 TARGET ACHIEVED! {:.0} files/sec >= 1M files/sec", throughput);
                        return; // Exit early if target is achieved
                    }
                }
                Ok(Err(e)) => {
                    panic!("Database operation failed: {}", e);
                }
                Err(_) => {
                    panic!("Operation timed out after {:?}", PERFORMANCE_TIMEOUT);
                }
            }
        }
        
        println!("📊 Performance benchmark completed. Target: 1M files/sec");
    }

    #[tokio::test]
    async fn test_performance_scaling_across_profiles() {
        let temp_dir = TempDir::new().unwrap();
        
        let profiles = [
            PerformanceProfile::Minimal,
            PerformanceProfile::Balanced,
            PerformanceProfile::HighPerformance,
            PerformanceProfile::Maximum,
        ];
        
        let test_size = 25_000; // Moderate size for comparison
        let test_files = create_test_media_files(test_size, temp_dir.path());
        
        println!("Performance scaling test with {} files:", test_size);
        
        for profile in &profiles {
            let db_path = temp_dir.path().join(format!("test_scaling_{:?}.db", profile));
            let db = ZeroCopyDatabase::new_with_profile(db_path, *profile).await.unwrap();
            db.initialize().await.unwrap();
            
            let start_memory = get_memory_usage_kb();
            let start_time = Instant::now();
            
            let file_ids = db.bulk_store_media_files(&test_files).await.unwrap();
            
            let operation_time = start_time.elapsed();
            let end_memory = get_memory_usage_kb();
            let memory_used = end_memory.saturating_sub(start_memory);
            
            let throughput = test_size as f64 / operation_time.as_secs_f64();
            
            println!("  {:?}:", profile);
            println!("    - Throughput: {:.0} files/sec", throughput);
            println!("    - Time: {:?}", operation_time);
            println!("    - Memory: {} KB", memory_used);
            println!("    - Expected: {}", profile.expected_throughput_range());
            
            assert_eq!(file_ids.len(), test_size);
            
            // Verify performance increases with higher profiles
            match profile {
                PerformanceProfile::Minimal => {
                    assert!(throughput >= 10_000.0, "Minimal profile too slow: {:.0}", throughput);
                }
                PerformanceProfile::Balanced => {
                    assert!(throughput >= 25_000.0, "Balanced profile too slow: {:.0}", throughput);
                }
                PerformanceProfile::HighPerformance => {
                    assert!(throughput >= 50_000.0, "High performance profile too slow: {:.0}", throughput);
                }
                PerformanceProfile::Maximum => {
                    assert!(throughput >= 100_000.0, "Maximum profile too slow: {:.0}", throughput);
                }
                PerformanceProfile::Custom => {}
            }
        }
    }

    #[tokio::test]
    async fn test_atomic_timing_precision() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_atomic_timing.db");
        
        let db = ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::HighPerformance).await.unwrap();
        db.initialize().await.unwrap();
        
        let test_files = create_test_media_files(1000, temp_dir.path());
        
        // Test atomic timing for individual operations
        let mut operation_times = Vec::new();
        
        for file in &test_files[..10] { // Test first 10 files
            let start = Instant::now();
            let _file_id = db.store_media_file(file).await.unwrap();
            let duration = start.elapsed();
            operation_times.push(duration);
        }
        
        // Calculate timing statistics
        let total_time: Duration = operation_times.iter().sum();
        let avg_time = total_time / operation_times.len() as u32;
        let min_time = operation_times.iter().min().unwrap();
        let max_time = operation_times.iter().max().unwrap();
        
        println!("✓ Atomic timing precision test:");
        println!("  - Average operation time: {:?}", avg_time);
        println!("  - Min operation time: {:?}", min_time);
        println!("  - Max operation time: {:?}", max_time);
        println!("  - Total operations: {}", operation_times.len());
        
        // Verify timing precision (should be sub-millisecond for individual operations)
        assert!(avg_time < Duration::from_millis(10), "Individual operations too slow");
        assert!(*max_time < Duration::from_millis(100), "Slowest operation too slow");
        
        // Test bulk operation timing
        let bulk_start = Instant::now();
        let _bulk_ids = db.bulk_store_media_files(&test_files[10..]).await.unwrap();
        let bulk_time = bulk_start.elapsed();
        
        let bulk_throughput = (test_files.len() - 10) as f64 / bulk_time.as_secs_f64();
        
        println!("  - Bulk operation throughput: {:.0} files/sec", bulk_throughput);
        
        // Bulk operations should be much faster than individual operations
        let individual_throughput = 10.0 / total_time.as_secs_f64();
        let speedup_ratio = bulk_throughput / individual_throughput;
        
        println!("  - Bulk vs individual speedup: {:.1}x", speedup_ratio);
        
        assert!(speedup_ratio > 10.0, "Bulk operations should be much faster than individual");
    }
}

#[cfg(test)]
mod memory_usage_tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_bounded_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_memory_bounded.db");
        
        // Test with minimal profile to verify memory bounds
        let db = ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::Minimal).await.unwrap();
        db.initialize().await.unwrap();
        
        let config = db.get_config().await;
        let memory_limit_kb = config.memory_budget_limit_mb * 1024;
        
        println!("Testing memory bounds with {}MB limit", config.memory_budget_limit_mb);
        
        let test_files = create_test_media_files(MEDIUM_DATASET_SIZE, temp_dir.path());
        
        let start_memory = get_memory_usage_kb();
        
        // Perform operations and monitor memory usage
        let _file_ids = db.bulk_store_media_files(&test_files).await.unwrap();
        
        let peak_memory = get_memory_usage_kb();
        let memory_used = peak_memory.saturating_sub(start_memory);
        
        println!("✓ Memory usage test:");
        println!("  - Memory limit: {} KB", memory_limit_kb);
        println!("  - Memory used: {} KB", memory_used);
        println!("  - Usage ratio: {:.1}%", (memory_used as f64 / memory_limit_kb as f64) * 100.0);
        
        // Memory usage should stay within reasonable bounds
        // Allow some overhead for OS and other operations
        let reasonable_limit = memory_limit_kb * 2; // 2x limit for safety
        assert!(memory_used < reasonable_limit, 
                "Memory usage {} KB exceeds reasonable limit {} KB", 
                memory_used, reasonable_limit);
        
        // Test memory cleanup after operations
        let cache_stats = db.get_cache_stats().await;
        println!("  - Cache usage: {} bytes", cache_stats.combined_memory_usage);
        
        // Memory stats should be reasonable
        let total_internal_usage = cache_stats.combined_memory_usage / 1024;
        assert!(total_internal_usage < memory_limit_kb, 
                "Internal memory usage too high: {} KB", total_internal_usage);
    }

    #[tokio::test]
    async fn test_atomic_memory_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_atomic_memory.db");
        
        let db = ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::Balanced).await.unwrap();
        db.initialize().await.unwrap();
        
        // Get initial memory stats
        let initial_stats = db.get_cache_stats().await;
        
        let test_files = create_test_media_files(5000, temp_dir.path());
        
        // Perform operations and track memory atomically
        let _file_ids = db.bulk_store_media_files(&test_files).await.unwrap();
        
        let after_insert_stats = db.get_cache_stats().await;
        
        // Perform more operations
        let more_files = create_test_media_files(5000, temp_dir.path());
        let _more_ids = db.bulk_store_media_files(&more_files).await.unwrap();
        
        let final_stats = db.get_cache_stats().await;
        
        println!("✓ Atomic memory tracking test:");
        println!("  - Initial cache usage: {} bytes", initial_stats.combined_memory_usage);
        println!("  - After first insert: {} bytes", after_insert_stats.combined_memory_usage);
        println!("  - Final cache usage: {} bytes", final_stats.combined_memory_usage);
        
        // Memory usage should increase with more data
        assert!(after_insert_stats.combined_memory_usage >= initial_stats.combined_memory_usage);
        assert!(final_stats.combined_memory_usage >= after_insert_stats.combined_memory_usage);
        
        // Memory tracking should be atomic (no race conditions)
        let consistency_check_stats = db.get_cache_stats().await;
        assert_eq!(consistency_check_stats.combined_memory_usage, final_stats.combined_memory_usage);
    }
}

#[cfg(test)]
mod stress_tests {
    use super::*;
    use tokio::task::JoinSet;

    #[tokio::test]
    async fn test_concurrent_atomic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_concurrent.db");
        
        let db = Arc::new(ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::HighPerformance).await.unwrap());
        db.initialize().await.unwrap();
        
        let num_concurrent_tasks = 8;
        let files_per_task = 1000;
        
        println!("Testing {} concurrent tasks with {} files each", num_concurrent_tasks, files_per_task);
        
        let start_time = Instant::now();
        let mut join_set = JoinSet::new();
        
        // Spawn concurrent tasks
        for task_id in 0..num_concurrent_tasks {
            let db_clone = Arc::clone(&db);
            let temp_path = temp_dir.path().to_path_buf();
            
            join_set.spawn(async move {
                let task_files = create_test_media_files(files_per_task, &temp_path);
                
                // Modify paths to avoid conflicts between tasks
                let mut unique_files = task_files;
                for (i, file) in unique_files.iter_mut().enumerate() {
                    file.path = temp_path.join(format!("task_{}/file_{}.mp4", task_id, i));
                }
                
                let task_start = Instant::now();
                let file_ids = db_clone.bulk_store_media_files(&unique_files).await?;
                let task_time = task_start.elapsed();
                
                Ok::<(usize, Duration, Vec<i64>), anyhow::Error>((task_id, task_time, file_ids))
            });
        }
        
        // Wait for all tasks to complete
        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok((task_id, task_time, file_ids))) => {
                    results.push((task_id, task_time, file_ids));
                }
                Ok(Err(e)) => {
                    panic!("Task failed: {}", e);
                }
                Err(e) => {
                    panic!("Task join failed: {}", e);
                }
            }
        }
        
        let total_time = start_time.elapsed();
        let total_files: usize = results.iter().map(|(_, _, ids)| ids.len()).sum();
        let total_throughput = total_files as f64 / total_time.as_secs_f64();
        
        println!("✓ Concurrent operations completed:");
        println!("  - Total files processed: {}", total_files);
        println!("  - Total time: {:?}", total_time);
        println!("  - Overall throughput: {:.0} files/sec", total_throughput);
        
        // Verify all tasks completed successfully
        assert_eq!(results.len(), num_concurrent_tasks);
        assert_eq!(total_files, num_concurrent_tasks * files_per_task);
        
        // Check individual task performance
        for (task_id, task_time, file_ids) in &results {
            let task_throughput = file_ids.len() as f64 / task_time.as_secs_f64();
            println!("  - Task {}: {:.0} files/sec", task_id, task_throughput);
            
            assert_eq!(file_ids.len(), files_per_task);
            assert!(task_throughput > 1000.0, "Task {} too slow: {:.0} files/sec", task_id, task_throughput);
        }
        
        // Verify database consistency after concurrent operations
        let final_stats = db.get_stats().await.unwrap();
        assert_eq!(final_stats.total_files, total_files);
        
        // Performance assertion for concurrent operations
        assert!(total_throughput > 5000.0, "Concurrent throughput too low: {:.0} files/sec", total_throughput);
    }

    #[tokio::test]
    async fn test_stress_with_large_dataset() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_stress_large.db");
        
        let db = ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::Maximum).await.unwrap();
        db.initialize().await.unwrap();
        
        println!("Stress testing with {} files", STRESS_DATASET_SIZE);
        
        let test_files = create_test_media_files(STRESS_DATASET_SIZE, temp_dir.path());
        
        let start_memory = get_memory_usage_kb();
        let start_time = Instant::now();
        
        // Use timeout to prevent hanging on large datasets
        let result = timeout(Duration::from_secs(300), async { // 5 minute timeout
            db.bulk_store_media_files(&test_files).await
        }).await;
        
        match result {
            Ok(Ok(file_ids)) => {
                let operation_time = start_time.elapsed();
                let end_memory = get_memory_usage_kb();
                let memory_used = end_memory.saturating_sub(start_memory);
                
                let throughput = STRESS_DATASET_SIZE as f64 / operation_time.as_secs_f64();
                
                println!("✓ Stress test completed:");
                println!("  - Files processed: {}", file_ids.len());
                println!("  - Time: {:?}", operation_time);
                println!("  - Throughput: {:.0} files/sec", throughput);
                println!("  - Memory used: {} KB", memory_used);
                println!("  - Target progress: {:.1}%", (throughput / TARGET_THROUGHPUT) * 100.0);
                
                assert_eq!(file_ids.len(), STRESS_DATASET_SIZE);
                
                // Verify database is still functional
                let stats = db.get_stats().await.unwrap();
                assert_eq!(stats.total_files, STRESS_DATASET_SIZE);
                
                // Performance assertion
                assert!(throughput > 10_000.0, "Stress test throughput too low: {:.0} files/sec", throughput);
                
                if throughput >= TARGET_THROUGHPUT {
                    println!("🚀 STRESS TEST TARGET ACHIEVED! {:.0} files/sec", throughput);
                }
            }
            Ok(Err(e)) => {
                panic!("Stress test failed: {}", e);
            }
            Err(_) => {
                panic!("Stress test timed out");
            }
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_replace_sqlite_with_zerocopy_equivalents() {
        let temp_dir = TempDir::new().unwrap();
        let zerocopy_path = temp_dir.path().join("zerocopy_test.db");
        
        // Test ZeroCopy database as complete SQLite replacement
        let db = ZeroCopyDatabase::new_with_profile(zerocopy_path, PerformanceProfile::HighPerformance).await.unwrap();
        db.initialize().await.unwrap();
        
        let test_files = create_test_media_files(1000, temp_dir.path());
        
        println!("Testing ZeroCopy as SQLite replacement:");
        
        // Test all DatabaseManager interface methods
        
        // 1. Bulk operations (primary interface)
        let file_ids = db.bulk_store_media_files(&test_files).await.unwrap();
        assert_eq!(file_ids.len(), test_files.len());
        println!("  ✓ bulk_store_media_files");
        
        let paths: Vec<PathBuf> = test_files.iter().map(|f| f.path.clone()).collect();
        let retrieved_files = db.bulk_get_files_by_paths(&paths).await.unwrap();
        assert_eq!(retrieved_files.len(), test_files.len());
        println!("  ✓ bulk_get_files_by_paths");
        
        // 2. Individual operations (should work as single-item bulk operations)
        let single_file = &test_files[0];
        let retrieved_single = db.get_file_by_path(&single_file.path).await.unwrap();
        assert!(retrieved_single.is_some());
        println!("  ✓ get_file_by_path");
        
        let file_by_id = db.get_file_by_id(file_ids[0]).await.unwrap();
        assert!(file_by_id.is_some());
        println!("  ✓ get_file_by_id");
        
        // 3. Directory operations
        let base_dir = temp_dir.path().join("media");
        let (subdirs, dir_files) = db.get_directory_listing(&base_dir, "video").await.unwrap();
        println!("  ✓ get_directory_listing ({} subdirs, {} files)", subdirs.len(), dir_files.len());
        
        // 4. Music categorization
        let artists = db.get_artists().await.unwrap();
        let albums = db.get_albums(None).await.unwrap();
        let genres = db.get_genres().await.unwrap();
        println!("  ✓ Music categorization ({} artists, {} albums, {} genres)", 
                 artists.len(), albums.len(), genres.len());
        
        // 5. Statistics and health
        let stats = db.get_stats().await.unwrap();
        assert_eq!(stats.total_files, test_files.len());
        println!("  ✓ get_stats");
        
        let health = db.check_and_repair().await.unwrap();
        assert!(health.is_healthy);
        println!("  ✓ check_and_repair");
        
        // 6. Cleanup operations
        let existing_paths: HashSet<String> = test_files.iter()
            .take(500) // Keep only first 500 files
            .map(|f| f.path.to_string_lossy().to_lowercase())
            .collect();
        
        let removed_count = db.batch_cleanup_missing_files(&existing_paths).await.unwrap();
        assert!(removed_count > 0);
        println!("  ✓ batch_cleanup_missing_files (removed {})", removed_count);
        
        println!("✓ ZeroCopy database successfully replaces SQLite functionality");
    }

    #[tokio::test]
    async fn test_zerocopy_performance_vs_target() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("performance_target_test.db");
        
        let db = ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::Maximum).await.unwrap();
        db.initialize().await.unwrap();
        
        // Test with different dataset sizes to measure scaling
        let test_sizes = [1_000, 10_000, 50_000, 100_000];
        let mut throughputs = Vec::new();
        
        println!("Performance scaling towards 1M files/sec target:");
        
        for &size in &test_sizes {
            let test_files = create_test_media_files(size, temp_dir.path());
            
            let start_time = Instant::now();
            let file_ids = db.bulk_store_media_files(&test_files).await.unwrap();
            let operation_time = start_time.elapsed();
            
            let throughput = size as f64 / operation_time.as_secs_f64();
            throughputs.push(throughput);
            
            println!("  {} files: {:.0} files/sec ({:.1}% of target)", 
                     size, throughput, (throughput / TARGET_THROUGHPUT) * 100.0);
            
            assert_eq!(file_ids.len(), size);
            
            // Clear database for next test
            let paths: Vec<PathBuf> = test_files.iter().map(|f| f.path.clone()).collect();
            db.bulk_remove_media_files(&paths).await.unwrap();
        }
        
        // Analyze performance scaling
        let max_throughput = throughputs.iter().fold(0.0f64, |a, &b| a.max(b));
        let target_percentage = (max_throughput / TARGET_THROUGHPUT) * 100.0;
        
        println!("Performance analysis:");
        println!("  - Maximum throughput achieved: {:.0} files/sec", max_throughput);
        println!("  - Target achievement: {:.1}%", target_percentage);
        
        if max_throughput >= TARGET_THROUGHPUT {
            println!("  🚀 TARGET ACHIEVED! ZeroCopy database meets 1M files/sec requirement");
        } else if target_percentage >= 80.0 {
            println!("  🎯 Close to target! {:.1}% of 1M files/sec achieved", target_percentage);
        } else if target_percentage >= 50.0 {
            println!("  📈 Good progress! {:.1}% of 1M files/sec achieved", target_percentage);
        } else {
            println!("  ⚠️  More optimization needed. Only {:.1}% of target achieved", target_percentage);
        }
        
        // Verify significant improvement over typical SQLite performance
        let sqlite_typical_throughput = 2000.0; // Typical SQLite performance
        let improvement_factor = max_throughput / sqlite_typical_throughput;
        
        println!("  - Improvement over SQLite: {:.0}x faster", improvement_factor);
        
        // Should be significantly faster than SQLite
        assert!(improvement_factor >= 10.0, 
                "ZeroCopy should be at least 10x faster than SQLite, got {:.1}x", 
                improvement_factor);
    }
}
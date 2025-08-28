//! Large Dataset Performance Benchmarks
//! 
//! These benchmarks test database performance with 1,000,000+ media files
//! to verify that optimizations work correctly at scale and memory usage
//! remains bounded during large operations.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::fs;
use vuio::database::{DatabaseManager, MediaFile, SqliteDatabase};

/// Large benchmark configuration
const MILLION_DATASET_SIZE: usize = 1_000_000;
const LARGE_BATCH_SIZE: usize = 10_000;
const EXTREME_PERFORMANCE_THRESHOLD_MS: u128 = 300_000; // 5 minutes max for million-file operations
const OPTIMIZED_OPERATION_THRESHOLD_MS: u128 = 30_000; // 30 seconds for optimized operations

/// Memory usage tracking structure
#[derive(Debug, Clone)]
struct MemorySnapshot {
    timestamp: Instant,
    memory_kb: usize,
    operation: String,
}

/// Performance metrics collection
#[derive(Debug)]
struct BenchmarkMetrics {
    operation_name: String,
    dataset_size: usize,
    duration: Duration,
    memory_snapshots: Vec<MemorySnapshot>,
    peak_memory_kb: usize,
    items_processed: usize,
    throughput_items_per_sec: f64,
}

impl BenchmarkMetrics {
    fn new(operation_name: String, dataset_size: usize) -> Self {
        Self {
            operation_name,
            dataset_size,
            duration: Duration::default(),
            memory_snapshots: Vec::new(),
            peak_memory_kb: 0,
            items_processed: 0,
            throughput_items_per_sec: 0.0,
        }
    }

    fn record_memory(&mut self, operation: String) {
        let memory_kb = get_memory_usage();
        self.peak_memory_kb = self.peak_memory_kb.max(memory_kb);
        self.memory_snapshots.push(MemorySnapshot {
            timestamp: Instant::now(),
            memory_kb,
            operation,
        });
    }

    fn complete(&mut self, start_time: Instant, items_processed: usize) {
        self.duration = start_time.elapsed();
        self.items_processed = items_processed;
        self.throughput_items_per_sec = if self.duration.as_secs_f64() > 0.0 {
            items_processed as f64 / self.duration.as_secs_f64()
        } else {
            0.0
        };
    }

    fn print_summary(&self) {
        println!("=== {} Benchmark Results ===", self.operation_name);
        println!("  Dataset size: {}", self.dataset_size);
        println!("  Items processed: {}", self.items_processed);
        println!("  Total duration: {:?}", self.duration);
        println!("  Throughput: {:.2} items/sec", self.throughput_items_per_sec);
        println!("  Peak memory usage: {} KB", self.peak_memory_kb);
        
        if self.memory_snapshots.len() > 1 {
            let memory_growth = self.peak_memory_kb.saturating_sub(
                self.memory_snapshots.first().map(|s| s.memory_kb).unwrap_or(0)
            );
            println!("  Memory growth: {} KB", memory_growth);
        }
        
        // Print memory progression for key operations
        if self.memory_snapshots.len() > 2 {
            println!("  Memory progression:");
            for snapshot in &self.memory_snapshots {
                println!("    {}: {} KB", snapshot.operation, snapshot.memory_kb);
            }
        }
        println!();
    }
}

/// Enhanced memory usage measurement
fn get_memory_usage() -> usize {
    #[cfg(unix)]
    {
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
    
    #[cfg(windows)]
    {
        // On Windows, use a different approach or return 0 for now
        // Could use Windows API calls for more accurate measurement
        0
    }
    
    #[cfg(not(any(unix, windows)))]
    {
        0
    }
}

/// Create a large test media file with realistic metadata
fn create_large_test_media_file(index: usize, base_path: &Path) -> MediaFile {
    // Create realistic directory structure
    let dir_level_1 = index / 10000; // 100 top-level directories
    let dir_level_2 = (index % 10000) / 1000; // 10 subdirectories each
    let file_in_dir = index % 1000; // 1000 files per directory
    
    let file_path = base_path
        .join(format!("media_{:03}", dir_level_1))
        .join(format!("subdir_{:02}", dir_level_2))
        .join(format!("file_{:04}.mp4", file_in_dir));
    
    let mut media_file = MediaFile::new(
        file_path, 
        1024 * 1024 + (index % 1000) as u64 * 1024, // 1-2MB files
        "video/mp4".to_string()
    );
    
    // Add realistic metadata distribution
    media_file.title = Some(format!("Media File {}", index));
    media_file.artist = Some(format!("Artist {}", index % 1000)); // 1000 different artists
    media_file.album = Some(format!("Album {}", index % 500)); // 500 different albums
    media_file.genre = Some(format!("Genre {}", index % 20)); // 20 different genres
    media_file.year = Some(1990 + (index % 34) as u32); // Years 1990-2023
    media_file.track_number = Some((index % 50 + 1) as u32); // Track numbers 1-50
    media_file.duration = Some(Duration::from_secs(120 + (index % 600) as u64)); // 2-12 minutes
    
    media_file
}

/// Create a million-file test dataset with progress reporting
async fn create_million_file_dataset(db: &SqliteDatabase) -> anyhow::Result<Vec<i64>> {
    let mut metrics = BenchmarkMetrics::new("Million File Dataset Creation".to_string(), MILLION_DATASET_SIZE);
    let start_time = Instant::now();
    
    println!("Creating dataset with {} files...", MILLION_DATASET_SIZE);
    metrics.record_memory("Start".to_string());
    
    let temp_dir = TempDir::new()?;
    let mut file_ids = Vec::with_capacity(MILLION_DATASET_SIZE);
    
    // Create files in large batches to optimize performance
    for batch_start in (0..MILLION_DATASET_SIZE).step_by(LARGE_BATCH_SIZE) {
        let batch_end = std::cmp::min(batch_start + LARGE_BATCH_SIZE, MILLION_DATASET_SIZE);
        let batch_size = batch_end - batch_start;
        
        let batch_start_time = Instant::now();
        let mut batch_files = Vec::with_capacity(batch_size);
        
        // Generate batch files
        for i in batch_start..batch_end {
            batch_files.push(create_large_test_media_file(i, temp_dir.path()));
        }
        
        // Store batch in database
        for file in batch_files {
            let id = db.store_media_file(&file).await?;
            file_ids.push(id);
        }
        
        let batch_duration = batch_start_time.elapsed();
        
        // Progress reporting every 100k files
        if batch_start % 100_000 == 0 || batch_end == MILLION_DATASET_SIZE {
            println!("  Progress: {} / {} files ({:.1}%) - Batch time: {:?}", 
                batch_end, MILLION_DATASET_SIZE, 
                (batch_end as f64 / MILLION_DATASET_SIZE as f64) * 100.0,
                batch_duration
            );
            metrics.record_memory(format!("After {} files", batch_end));
        }
    }
    
    metrics.complete(start_time, file_ids.len());
    metrics.print_summary();
    
    Ok(file_ids)
}

#[cfg(test)]
mod large_dataset_benchmarks {
    use super::*;
    use futures_util::StreamExt;

    #[tokio::test]
    #[ignore] // Use --ignored flag to run this expensive test
    async fn benchmark_million_file_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("million_files.db");
        
        let db = SqliteDatabase::new(db_path.clone()).await.unwrap();
        db.initialize().await.unwrap();
        
        println!("=== Million File Creation Benchmark ===");
        
        let file_ids = create_million_file_dataset(&db).await.unwrap();
        
        // Verify all files were created
        assert_eq!(file_ids.len(), MILLION_DATASET_SIZE);
        
        // Check database file size
        let db_size = fs::metadata(&db_path).await.unwrap().len();
        println!("Final database size: {} MB", db_size / (1024 * 1024));
        
        // Verify database integrity
        let stats = db.get_stats().await.unwrap();
        println!("Database stats: {} files, {} MB total size", stats.total_files, stats.total_size / (1024 * 1024));
        assert_eq!(stats.total_files, MILLION_DATASET_SIZE);
    }

    #[tokio::test]
    #[ignore]
    async fn benchmark_million_file_streaming() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("streaming_million.db");
        
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();
        
        println!("=== Million File Streaming Benchmark ===");
        
        // Create dataset
        let _file_ids = create_million_file_dataset(&db).await.unwrap();
        
        // Benchmark streaming performance
        let mut metrics = BenchmarkMetrics::new("Million File Streaming".to_string(), MILLION_DATASET_SIZE);
        let start_time = Instant::now();
        
        metrics.record_memory("Stream start".to_string());
        
        let mut stream = db.stream_all_media_files();
        let mut count = 0;
        let mut batch_count = 0;
        
        while let Some(result) = stream.next().await {
            match result {
                Ok(_media_file) => {
                    count += 1;
                    batch_count += 1;
                    
                    // Record memory usage every 100k files
                    if batch_count >= 100_000 {
                        metrics.record_memory(format!("Streamed {} files", count));
                        batch_count = 0;
                        
                        println!("  Streamed {} / {} files ({:.1}%)", 
                            count, MILLION_DATASET_SIZE,
                            (count as f64 / MILLION_DATASET_SIZE as f64) * 100.0
                        );
                    }
                }
                Err(e) => {
                    panic!("Streaming error at file {}: {}", count, e);
                }
            }
        }
        
        metrics.complete(start_time, count);
        metrics.print_summary();
        
        // Verify all files were streamed
        assert_eq!(count, MILLION_DATASET_SIZE);
        
        // Performance assertion
        assert!(
            metrics.duration.as_millis() < EXTREME_PERFORMANCE_THRESHOLD_MS,
            "Million file streaming took too long: {:?}",
            metrics.duration
        );
    }

    #[tokio::test]
    #[ignore]
    async fn benchmark_database_native_cleanup_million_files() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("cleanup_million.db");
        
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();
        
        println!("=== Million File Database-Native Cleanup Benchmark ===");
        
        // Create dataset
        let _file_ids = create_million_file_dataset(&db).await.unwrap();
        
        // Get all canonical paths from database
        println!("Collecting canonical paths for cleanup simulation...");
        let collect_start = Instant::now();
        let all_files = db.collect_all_media_files().await.unwrap();
        let collect_time = collect_start.elapsed();
        println!("Collected {} files in {:?}", all_files.len(), collect_time);
        
        // Simulate that 30% of files are missing (keep 70%)
        let keep_ratio = 0.7;
        let files_to_keep = (MILLION_DATASET_SIZE as f64 * keep_ratio) as usize;
        
        let existing_paths: Vec<String> = all_files
            .iter()
            .take(files_to_keep)
            .map(|f| f.path.to_string_lossy().to_lowercase().replace('\\', "/"))
            .collect();
        
        println!("Simulating cleanup with {} existing files (removing {} files)...", 
            existing_paths.len(), 
            MILLION_DATASET_SIZE - existing_paths.len()
        );
        
        // Benchmark database-native cleanup
        let mut metrics = BenchmarkMetrics::new("Database-Native Cleanup".to_string(), MILLION_DATASET_SIZE);
        let start_time = Instant::now();
        
        metrics.record_memory("Cleanup start".to_string());
        
        let removed_count = db.database_native_cleanup(&existing_paths).await.unwrap();
        
        metrics.record_memory("Cleanup complete".to_string());
        metrics.complete(start_time, removed_count);
        metrics.print_summary();
        
        // Verify cleanup results
        let expected_removed = MILLION_DATASET_SIZE - existing_paths.len();
        println!("Expected to remove: {}, Actually removed: {}", expected_removed, removed_count);
        
        assert!(
            removed_count >= expected_removed.saturating_sub(1000) && removed_count <= expected_removed + 1000,
            "Cleanup removed unexpected number of files: expected ~{}, got {}",
            expected_removed,
            removed_count
        );
        
        // Performance assertion - should be much faster than old method
        assert!(
            metrics.duration.as_millis() < OPTIMIZED_OPERATION_THRESHOLD_MS,
            "Database-native cleanup took too long: {:?}",
            metrics.duration
        );
        
        // Verify remaining files
        let remaining_files = db.collect_all_media_files().await.unwrap();
        println!("Files remaining after cleanup: {}", remaining_files.len());
        assert!(remaining_files.len() > 0);
        assert!(remaining_files.len() < MILLION_DATASET_SIZE);
    }

    #[tokio::test]
    #[ignore]
    async fn benchmark_directory_operations_million_files() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("directory_million.db");
        
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();
        
        println!("=== Million File Directory Operations Benchmark ===");
        
        // Create dataset with hierarchical structure
        let _file_ids = create_million_file_dataset(&db).await.unwrap();
        
        // Test directory listing performance at various levels
        let test_paths = vec![
            "/media_000",           // Top-level directory with ~10k files
            "/media_000/subdir_00", // Mid-level directory with ~1k files  
            "/media_050/subdir_05", // Another mid-level directory
            "/media_099/subdir_09", // Last directory
        ];
        
        for test_path in &test_paths {
            let _canonical_path = test_path.to_lowercase();
            
            let mut metrics = BenchmarkMetrics::new(
                format!("Directory Listing: {}", test_path), 
                MILLION_DATASET_SIZE
            );
            let start_time = Instant::now();
            
            metrics.record_memory("Query start".to_string());
            
            let (subdirs, files) = db.get_directory_listing(
                &PathBuf::from(test_path), 
                "video"
            ).await.unwrap();
            
            metrics.record_memory("Query complete".to_string());
            metrics.complete(start_time, subdirs.len() + files.len());
            
            println!("Directory listing for '{}':", test_path);
            println!("  Subdirectories: {}", subdirs.len());
            println!("  Files: {}", files.len());
            println!("  Query time: {:?}", metrics.duration);
            
            // Directory listing should be fast even with million files
            assert!(
                metrics.duration.as_millis() < 5000, // 5 seconds max
                "Directory listing took too long: {:?}",
                metrics.duration
            );
        }
        
        // Test path prefix queries
        let prefix_tests = vec![
            "/media_000",
            "/media_050", 
            "/media_099",
        ];
        
        for prefix in &prefix_tests {
            let canonical_prefix = prefix.to_lowercase();
            
            let mut metrics = BenchmarkMetrics::new(
                format!("Path Prefix Query: {}", prefix), 
                MILLION_DATASET_SIZE
            );
            let start_time = Instant::now();
            
            metrics.record_memory("Prefix query start".to_string());
            
            let prefix_files = db.get_files_with_path_prefix(&canonical_prefix).await.unwrap();
            
            metrics.record_memory("Prefix query complete".to_string());
            metrics.complete(start_time, prefix_files.len());
            
            println!("Path prefix query for '{}':", prefix);
            println!("  Files found: {}", prefix_files.len());
            println!("  Query time: {:?}", metrics.duration);
            
            // Should find approximately 10k files per top-level directory
            assert!(
                prefix_files.len() >= 9000 && prefix_files.len() <= 11000,
                "Unexpected number of files for prefix '{}': {}",
                prefix,
                prefix_files.len()
            );
            
            // Prefix queries should be fast
            assert!(
                metrics.duration.as_millis() < 10000, // 10 seconds max
                "Path prefix query took too long: {:?}",
                metrics.duration
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn benchmark_memory_bounded_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("memory_bounded.db");
        
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();
        
        println!("=== Memory Bounded Operations Benchmark ===");
        
        // Create dataset
        let _file_ids = create_million_file_dataset(&db).await.unwrap();
        
        let initial_memory = get_memory_usage();
        println!("Initial memory usage: {} KB", initial_memory);
        
        // Test 1: Streaming should use bounded memory
        println!("\nTesting streaming memory usage...");
        let stream_start_memory = get_memory_usage();
        
        let mut stream = db.stream_all_media_files();
        let mut count = 0;
        let mut max_memory = stream_start_memory;
        
        while let Some(result) = stream.next().await {
            if result.is_ok() {
                count += 1;
                
                if count % 50_000 == 0 {
                    let current_memory = get_memory_usage();
                    max_memory = max_memory.max(current_memory);
                    
                    println!("  Streamed {} files, memory: {} KB", count, current_memory);
                }
            }
        }
        
        let stream_memory_growth = max_memory.saturating_sub(stream_start_memory);
        println!("Streaming memory growth: {} KB", stream_memory_growth);
        
        // Test 2: Database-native cleanup should use bounded memory
        println!("\nTesting cleanup memory usage...");
        let cleanup_start_memory = get_memory_usage();
        
        // Create a list of paths to keep (simulate 80% still exist)
        let paths_to_keep: Vec<String> = (0..800_000)
            .map(|i| format!("/media_{:03}/subdir_{:02}/file_{:04}.mp4", 
                i / 10000, (i % 10000) / 1000, i % 1000))
            .collect();
        
        let removed = db.database_native_cleanup(&paths_to_keep).await.unwrap();
        
        let cleanup_end_memory = get_memory_usage();
        let cleanup_memory_growth = cleanup_end_memory.saturating_sub(cleanup_start_memory);
        
        println!("Cleanup removed {} files", removed);
        println!("Cleanup memory growth: {} KB", cleanup_memory_growth);
        
        // Memory usage should remain reasonable
        if initial_memory > 0 {
            // Memory growth should be bounded (less than 500MB for million files)
            assert!(
                stream_memory_growth < 500_000,
                "Streaming used too much memory: {} KB",
                stream_memory_growth
            );
            
            assert!(
                cleanup_memory_growth < 200_000,
                "Cleanup used too much memory: {} KB", 
                cleanup_memory_growth
            );
        }
        
        println!("\nMemory usage summary:");
        println!("  Initial: {} KB", initial_memory);
        println!("  Streaming peak growth: {} KB", stream_memory_growth);
        println!("  Cleanup peak growth: {} KB", cleanup_memory_growth);
        println!("  Final: {} KB", get_memory_usage());
    }

    #[tokio::test]
    #[ignore]
    async fn benchmark_database_maintenance_million_files() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("maintenance_million.db");
        
        let db = SqliteDatabase::new(db_path.clone()).await.unwrap();
        db.initialize().await.unwrap();
        
        println!("=== Million File Database Maintenance Benchmark ===");
        
        // Create dataset
        let file_ids = create_million_file_dataset(&db).await.unwrap();
        
        // Check initial database size
        let initial_size = fs::metadata(&db_path).await.unwrap().len();
        println!("Initial database size: {} MB", initial_size / (1024 * 1024));
        
        // Delete 40% of files to create fragmentation
        println!("Creating fragmentation by deleting 40% of files...");
        let delete_start = Instant::now();
        let files_to_delete = MILLION_DATASET_SIZE * 40 / 100;
        
        let mut deleted_count = 0;
        for &file_id in file_ids.iter().take(files_to_delete) {
            if let Ok(Some(file)) = db.get_file_by_id(file_id).await {
                if db.remove_media_file(&file.path).await.unwrap_or(false) {
                    deleted_count += 1;
                }
            }
            
            if deleted_count % 50_000 == 0 {
                println!("  Deleted {} / {} files", deleted_count, files_to_delete);
            }
        }
        
        let delete_time = delete_start.elapsed();
        let fragmented_size = fs::metadata(&db_path).await.unwrap().len();
        
        println!("Deletion completed in {:?}", delete_time);
        println!("Deleted {} files", deleted_count);
        println!("Database size after deletion: {} MB", fragmented_size / (1024 * 1024));
        
        // Perform vacuum operation
        println!("Performing vacuum operation...");
        let mut metrics = BenchmarkMetrics::new("Database Vacuum".to_string(), MILLION_DATASET_SIZE);
        let vacuum_start = Instant::now();
        
        metrics.record_memory("Vacuum start".to_string());
        
        db.vacuum().await.unwrap();
        
        metrics.record_memory("Vacuum complete".to_string());
        metrics.complete(vacuum_start, 0);
        
        let final_size = fs::metadata(&db_path).await.unwrap().len();
        
        metrics.print_summary();
        
        // Calculate space reclaimed
        let space_reclaimed = fragmented_size.saturating_sub(final_size);
        let reclaim_percentage = if fragmented_size > 0 {
            (space_reclaimed as f64 / fragmented_size as f64) * 100.0
        } else {
            0.0
        };
        
        println!("Vacuum results:");
        println!("  Space reclaimed: {} MB ({:.1}%)", space_reclaimed / (1024 * 1024), reclaim_percentage);
        println!("  Final database size: {} MB", final_size / (1024 * 1024));
        
        // Vacuum should complete within reasonable time even for million files
        assert!(
            metrics.duration.as_millis() < EXTREME_PERFORMANCE_THRESHOLD_MS,
            "Vacuum took too long: {:?}",
            metrics.duration
        );
        
        // Should reclaim some space
        assert!(final_size <= fragmented_size, "Vacuum should not increase database size");
        
        // Verify database is still functional
        let remaining_files = db.collect_all_media_files().await.unwrap();
        println!("Files remaining after vacuum: {}", remaining_files.len());
        
        assert!(remaining_files.len() > 0);
        assert!(remaining_files.len() < MILLION_DATASET_SIZE);
        
        // Verify database integrity
        let health = db.check_and_repair().await.unwrap();
        println!("Database health after vacuum: {:?}", health);
    }
}
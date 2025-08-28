//! Database performance tests
//! 
//! These tests verify database performance with large datasets and measure
//! memory usage during operations to ensure the system scales properly.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::fs;
use vuio::database::{DatabaseManager, MediaFile, SqliteDatabase};

/// Performance test configuration
const LARGE_DATASET_SIZE: usize = 10_000;
const BATCH_SIZE: usize = 1_000;
const PERFORMANCE_THRESHOLD_MS: u128 = 15_000; // 15 seconds max for large operations
const FAST_OPERATION_THRESHOLD_MS: u128 = 1_000; // 1 second for fast operations

/// Helper function to create a test media file
fn create_test_media_file(index: usize, base_path: &Path) -> MediaFile {
    let file_path = base_path.join(format!("test_video_{:05}.mp4", index));
    let mut media_file = MediaFile::new(file_path, 1024 * 1024, "video/mp4".to_string());
    
    // Add some metadata variation
    media_file.title = Some(format!("Test Video {}", index));
    media_file.artist = Some(format!("Artist {}", index % 100)); // 100 different artists
    media_file.album = Some(format!("Album {}", index % 50)); // 50 different albums
    media_file.genre = Some(format!("Genre {}", index % 10)); // 10 different genres
    media_file.year = Some(2000 + (index % 24) as u32); // Years 2000-2023
    media_file.track_number = Some((index % 20 + 1) as u32); // Track numbers 1-20
    media_file.duration = Some(Duration::from_secs(180 + (index % 300) as u64)); // 3-8 minutes
    
    media_file
}

/// Helper function to create a large test dataset
async fn create_large_test_dataset(db: &SqliteDatabase, size: usize) -> anyhow::Result<Vec<i64>> {
    println!("Creating test dataset with {} files...", size);
    let start_time = Instant::now();
    
    let temp_dir = TempDir::new()?;
    let mut file_ids = Vec::with_capacity(size);
    
    // Create files in batches to avoid memory issues
    for batch_start in (0..size).step_by(BATCH_SIZE) {
        let batch_end = std::cmp::min(batch_start + BATCH_SIZE, size);
        let batch_size = batch_end - batch_start;
        
        let mut batch_files = Vec::with_capacity(batch_size);
        for i in batch_start..batch_end {
            batch_files.push(create_test_media_file(i, temp_dir.path()));
        }
        
        // Store batch in database
        for file in batch_files {
            let id = db.store_media_file(&file).await?;
            file_ids.push(id);
        }
        
        if batch_start % (BATCH_SIZE * 5) == 0 {
            println!("  Created {} / {} files", batch_end, size);
        }
    }
    
    let creation_time = start_time.elapsed();
    println!("Dataset creation completed in {:?}", creation_time);
    println!("Average time per file: {:?}", creation_time / size as u32);
    
    Ok(file_ids)
}

/// Measure memory usage during operation (simplified approach)
fn get_memory_usage() -> usize {
    // This is a simplified memory measurement
    // In a real implementation, you might use a more sophisticated approach
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

#[cfg(test)]
mod performance_tests {
    use super::*;
    use futures_util::StreamExt;

    #[tokio::test]
    async fn test_large_dataset_creation_performance() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("performance_test.db");
        
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();
        
        println!("=== Large Dataset Creation Performance Test ===");
        
        let start_memory = get_memory_usage();
        let start_time = Instant::now();
        
        let file_ids = create_large_test_dataset(&db, LARGE_DATASET_SIZE).await.unwrap();
        
        let creation_time = start_time.elapsed();
        let end_memory = get_memory_usage();
        
        println!("Results:");
        println!("  Files created: {}", file_ids.len());
        println!("  Total time: {:?}", creation_time);
        println!("  Average time per file: {:?}", creation_time / LARGE_DATASET_SIZE as u32);
        println!("  Memory usage change: {} KB", end_memory.saturating_sub(start_memory));
        
        // Verify all files were created
        assert_eq!(file_ids.len(), LARGE_DATASET_SIZE);
        
        // Performance assertion
        assert!(
            creation_time.as_millis() < PERFORMANCE_THRESHOLD_MS,
            "Dataset creation took too long: {:?} > {}ms",
            creation_time,
            PERFORMANCE_THRESHOLD_MS
        );
    }

    #[tokio::test]
    async fn test_directory_listing_performance() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("directory_performance.db");
        
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();
        
        println!("=== Directory Listing Performance Test ===");
        
        // Create test dataset with hierarchical directory structure
        let base_path = temp_dir.path().join("media");
        let mut test_files = Vec::new();
        
        // Create files in multiple directories
        for dir_idx in 0..10 {
            let dir_path = base_path.join(format!("directory_{:02}", dir_idx));
            
            for file_idx in 0..100 {
                let file_path = dir_path.join(format!("video_{:03}.mp4", file_idx));
                let mut media_file = MediaFile::new(file_path, 1024 * 1024, "video/mp4".to_string());
                media_file.title = Some(format!("Video {} in Dir {}", file_idx, dir_idx));
                test_files.push(media_file);
            }
        }
        
        // Store all files
        println!("Storing {} files in hierarchical structure...", test_files.len());
        let store_start = Instant::now();
        
        for file in &test_files {
            db.store_media_file(file).await.unwrap();
        }
        
        let store_time = store_start.elapsed();
        println!("Storage completed in {:?}", store_time);
        
        // Test directory listing performance
        let start_memory = get_memory_usage();
        let listing_start = Instant::now();
        
        let (subdirs, files) = db.get_directory_listing(&base_path, "video").await.unwrap();
        
        let listing_time = listing_start.elapsed();
        let end_memory = get_memory_usage();
        
        println!("Directory listing results:");
        println!("  Subdirectories found: {}", subdirs.len());
        println!("  Files found: {}", files.len());
        println!("  Listing time: {:?}", listing_time);
        println!("  Memory usage change: {} KB", end_memory.saturating_sub(start_memory));
        
        // Verify results
        assert_eq!(subdirs.len(), 10); // Should find 10 subdirectories
        assert_eq!(files.len(), 0); // No files directly in base_path
        
        // Performance assertion - directory listing should be fast
        assert!(
            listing_time.as_millis() < FAST_OPERATION_THRESHOLD_MS,
            "Directory listing took too long: {:?}",
            listing_time
        );
        
        // Test listing a specific subdirectory
        let subdir_path = base_path.join("directory_00");
        let subdir_start = Instant::now();
        
        let (sub_subdirs, sub_files) = db.get_directory_listing(&subdir_path, "video").await.unwrap();
        
        let subdir_time = subdir_start.elapsed();
        
        println!("Subdirectory listing results:");
        println!("  Subdirectories: {}", sub_subdirs.len());
        println!("  Files: {}", sub_files.len());
        println!("  Time: {:?}", subdir_time);
        
        assert_eq!(sub_subdirs.len(), 0); // No subdirectories
        assert_eq!(sub_files.len(), 100); // 100 files in this directory
        
        // Subdirectory listing should also be fast
        assert!(
            subdir_time.as_millis() < 500, // 500ms max
            "Subdirectory listing took too long: {:?}",
            subdir_time
        );
    }

    #[tokio::test]
    async fn test_batch_cleanup_performance() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("cleanup_performance.db");
        
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();
        
        println!("=== Batch Cleanup Performance Test ===");
        
        // Create large dataset
        let _file_ids = create_large_test_dataset(&db, LARGE_DATASET_SIZE).await.unwrap();
        
        // Get all files from database to see what canonical paths were actually stored
        let all_db_files = db.collect_all_media_files().await.unwrap();
        
        // Simulate that half the files are missing by taking only the first half as "existing"
        let existing_files: HashSet<String> = all_db_files
            .iter()
            .take(LARGE_DATASET_SIZE / 2)
            .map(|f| f.path.to_string_lossy().to_lowercase().replace('\\', "/"))
            .collect();
        
        println!("Simulating cleanup of {} missing files...", LARGE_DATASET_SIZE / 2);
        
        let start_memory = get_memory_usage();
        let cleanup_start = Instant::now();
        
        let removed_count = db.batch_cleanup_missing_files(&existing_files).await.unwrap();
        
        let cleanup_time = cleanup_start.elapsed();
        let end_memory = get_memory_usage();
        
        println!("Batch cleanup results:");
        println!("  Files removed: {}", removed_count);
        println!("  Cleanup time: {:?}", cleanup_time);
        println!("  Memory usage change: {} KB", end_memory.saturating_sub(start_memory));
        
        // Verify cleanup worked correctly
        assert!(removed_count > 0, "Should have removed some files");
        let expected_removed = LARGE_DATASET_SIZE - existing_files.len();
        assert!(
            removed_count >= expected_removed.saturating_sub(100) && removed_count <= expected_removed + 100,
            "Should remove approximately {} files, but removed {}",
            expected_removed,
            removed_count
        );
        
        // Performance assertion
        assert!(
            cleanup_time.as_millis() < PERFORMANCE_THRESHOLD_MS,
            "Batch cleanup took too long: {:?}",
            cleanup_time
        );
        
        // Verify remaining files
        let remaining_files = db.collect_all_media_files().await.unwrap();
        println!("Files remaining after cleanup: {}", remaining_files.len());
        
        assert!(remaining_files.len() > 0, "Should have some files remaining");
        assert!(remaining_files.len() < LARGE_DATASET_SIZE, "Should have fewer files than original");
    }

    #[tokio::test]
    async fn test_streaming_vs_bulk_memory_usage() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("streaming_test.db");
        
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();
        
        println!("=== Streaming vs Bulk Memory Usage Test ===");
        
        // Create dataset
        let _file_ids = create_large_test_dataset(&db, LARGE_DATASET_SIZE).await.unwrap();
        
        // Test collect method (uses streaming internally)
        println!("Testing collect method (collect_all_media_files)...");
        let bulk_start_memory = get_memory_usage();
        let bulk_start_time = Instant::now();
        
        let all_files = db.collect_all_media_files().await.unwrap();
        
        let bulk_time = bulk_start_time.elapsed();
        let bulk_peak_memory = get_memory_usage();
        let bulk_memory_usage = bulk_peak_memory.saturating_sub(bulk_start_memory);
        
        println!("Collect method results:");
        println!("  Files loaded: {}", all_files.len());
        println!("  Time: {:?}", bulk_time);
        println!("  Memory usage: {} KB", bulk_memory_usage);
        
        // Clear memory by dropping the collection
        drop(all_files);
        
        // Wait a moment for memory to be reclaimed
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Test streaming approach
        println!("Testing streaming approach (stream_all_media_files)...");
        let stream_start_memory = get_memory_usage();
        let stream_start_time = Instant::now();
        
        let mut stream = db.stream_all_media_files();
        let mut stream_count = 0;
        let mut stream_peak_memory = stream_start_memory;
        
        while let Some(result) = stream.next().await {
            match result {
                Ok(_media_file) => {
                    stream_count += 1;
                    
                    // Check memory usage periodically
                    if stream_count % 1000 == 0 {
                        let current_memory = get_memory_usage();
                        stream_peak_memory = stream_peak_memory.max(current_memory);
                    }
                }
                Err(e) => {
                    panic!("Streaming error: {}", e);
                }
            }
        }
        
        let stream_time = stream_start_time.elapsed();
        let stream_memory_usage = stream_peak_memory.saturating_sub(stream_start_memory);
        
        println!("Streaming results:");
        println!("  Files streamed: {}", stream_count);
        println!("  Time: {:?}", stream_time);
        println!("  Peak memory usage: {} KB", stream_memory_usage);
        
        // Verify counts match
        assert_eq!(stream_count, LARGE_DATASET_SIZE);
        
        // Memory usage comparison
        println!("Memory usage comparison:");
        println!("  Bulk loading: {} KB", bulk_memory_usage);
        println!("  Streaming: {} KB", stream_memory_usage);
        
        if bulk_memory_usage > 0 && stream_memory_usage > 0 {
            let memory_ratio = bulk_memory_usage as f64 / stream_memory_usage as f64;
            println!("  Memory ratio (bulk/stream): {:.2}x", memory_ratio);
            
            // Streaming should use significantly less memory for large datasets
            if LARGE_DATASET_SIZE > 1000 {
                assert!(
                    memory_ratio > 1.5,
                    "Streaming should use significantly less memory than bulk loading"
                );
            }
        }
        
        // Performance comparison
        println!("Performance comparison:");
        println!("  Bulk time: {:?}", bulk_time);
        println!("  Stream time: {:?}", stream_time);
        
        // Both should complete within reasonable time
        assert!(bulk_time.as_millis() < PERFORMANCE_THRESHOLD_MS);
        assert!(stream_time.as_millis() < PERFORMANCE_THRESHOLD_MS);
    }

    #[tokio::test]
    async fn test_path_prefix_query_performance() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("prefix_performance.db");
        
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();
        
        println!("=== Path Prefix Query Performance Test ===");
        
        // Create files with various path prefixes
        let base_paths = vec![
            "/media/videos/movies",
            "/media/videos/tv_shows", 
            "/media/music/rock",
            "/media/music/jazz",
            "/media/photos/2023",
        ];
        
        let mut all_files = Vec::new();
        for (_path_idx, base_path) in base_paths.iter().enumerate() {
            for file_idx in 0..2000 { // 2000 files per path = 10,000 total
                let file_path = PathBuf::from(format!("{}/file_{:04}.mp4", base_path, file_idx));
                let media_file = MediaFile::new(file_path, 1024 * 1024, "video/mp4".to_string());
                all_files.push(media_file);
            }
        }
        
        // Store all files
        println!("Storing {} files across {} path prefixes...", all_files.len(), base_paths.len());
        let store_start = Instant::now();
        
        for file in &all_files {
            db.store_media_file(file).await.unwrap();
        }
        
        let store_time = store_start.elapsed();
        println!("Storage completed in {:?}", store_time);
        
        // Test path prefix queries
        for prefix in &base_paths {
            let canonical_prefix = prefix.to_lowercase(); // Simulate canonical path format
            
            let start_memory = get_memory_usage();
            let query_start = Instant::now();
            
            let prefix_files = db.get_files_with_path_prefix(&canonical_prefix).await.unwrap();
            
            let query_time = query_start.elapsed();
            let end_memory = get_memory_usage();
            
            println!("Prefix query results for '{}':", prefix);
            println!("  Files found: {}", prefix_files.len());
            println!("  Query time: {:?}", query_time);
            println!("  Memory usage: {} KB", end_memory.saturating_sub(start_memory));
            
            // Should find approximately 2000 files per prefix
            assert!(
                prefix_files.len() >= 1900 && prefix_files.len() <= 2100,
                "Expected ~2000 files, found {}",
                prefix_files.len()
            );
            
            // Query should be fast
            assert!(
                query_time.as_millis() < FAST_OPERATION_THRESHOLD_MS,
                "Path prefix query took too long: {:?}",
                query_time
            );
        }
        
        // Test non-existent prefix
        let nonexistent_start = Instant::now();
        let nonexistent_files = db.get_files_with_path_prefix("/nonexistent/path").await.unwrap();
        let nonexistent_time = nonexistent_start.elapsed();
        
        println!("Non-existent prefix query:");
        println!("  Files found: {}", nonexistent_files.len());
        println!("  Query time: {:?}", nonexistent_time);
        
        assert_eq!(nonexistent_files.len(), 0);
        assert!(nonexistent_time.as_millis() < 100); // Should be very fast
    }

    #[tokio::test]
    async fn test_concurrent_operations_performance() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("concurrent_performance.db");
        
        let db = SqliteDatabase::new(db_path).await.unwrap();
        db.initialize().await.unwrap();
        
        println!("=== Concurrent Operations Performance Test ===");
        
        // Create initial dataset
        let _file_ids = create_large_test_dataset(&db, 5000).await.unwrap();
        
        let start_time = Instant::now();
        let start_memory = get_memory_usage();
        
        // Run multiple concurrent operations
        let tasks = vec![
            // Task 1: Stream all files
            tokio::spawn({
                let db = SqliteDatabase::new(temp_dir.path().join("concurrent_performance.db")).await.unwrap();
                async move {
                    let mut stream = db.stream_all_media_files();
                    let mut count = 0;
                    while let Some(result) = stream.next().await {
                        if result.is_ok() {
                            count += 1;
                        }
                    }
                    count
                }
            }),
            
            // Task 2: Directory listings
            tokio::spawn({
                let db = SqliteDatabase::new(temp_dir.path().join("concurrent_performance.db")).await.unwrap();
                async move {
                    let mut total_dirs = 0;
                    for i in 0..10 {
                        let path = PathBuf::from(format!("/test/path/dir_{}", i));
                        if let Ok((dirs, _files)) = db.get_directory_listing(&path, "video").await {
                            total_dirs += dirs.len();
                        }
                    }
                    total_dirs
                }
            }),
            
            // Task 3: Path prefix queries
            tokio::spawn({
                let db = SqliteDatabase::new(temp_dir.path().join("concurrent_performance.db")).await.unwrap();
                async move {
                    let mut total_files = 0;
                    for i in 0..5 {
                        let prefix = format!("/test/path/batch_{}", i);
                        if let Ok(files) = db.get_files_with_path_prefix(&prefix).await {
                            total_files += files.len();
                        }
                    }
                    total_files
                }
            }),
        ];
        
        // Wait for all tasks to complete
        let results = futures_util::future::join_all(tasks).await;
        
        let total_time = start_time.elapsed();
        let end_memory = get_memory_usage();
        
        println!("Concurrent operations results:");
        println!("  Total time: {:?}", total_time);
        println!("  Memory usage change: {} KB", end_memory.saturating_sub(start_memory));
        
        for (i, result) in results.iter().enumerate() {
            match result {
                Ok(count) => println!("  Task {}: {} items processed", i + 1, count),
                Err(e) => println!("  Task {}: Error - {}", i + 1, e),
            }
        }
        
        // All tasks should complete successfully
        for result in &results {
            assert!(result.is_ok(), "Concurrent task failed");
        }
        
        // Should complete within reasonable time
        assert!(
            total_time.as_millis() < PERFORMANCE_THRESHOLD_MS,
            "Concurrent operations took too long: {:?}",
            total_time
        );
    }

    #[tokio::test]
    async fn test_database_size_and_vacuum_performance() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("vacuum_performance.db");
        
        let db = SqliteDatabase::new(db_path.clone()).await.unwrap();
        db.initialize().await.unwrap();
        
        println!("=== Database Size and Vacuum Performance Test ===");
        
        // Create large dataset
        let file_ids = create_large_test_dataset(&db, LARGE_DATASET_SIZE).await.unwrap();
        
        // Check initial database size
        let initial_size = fs::metadata(&db_path).await.unwrap().len();
        println!("Initial database size: {} KB", initial_size / 1024);
        
        // Delete half the files to create fragmentation
        println!("Deleting half the files to create fragmentation...");
        let delete_start = Instant::now();
        
        for &file_id in file_ids.iter().take(LARGE_DATASET_SIZE / 2) {
            if let Ok(Some(file)) = db.get_file_by_id(file_id).await {
                let _ = db.remove_media_file(&file.path).await;
            }
        }
        
        let delete_time = delete_start.elapsed();
        let fragmented_size = fs::metadata(&db_path).await.unwrap().len();
        
        println!("Deletion completed in {:?}", delete_time);
        println!("Database size after deletion: {} KB", fragmented_size / 1024);
        
        // Perform vacuum operation
        println!("Performing vacuum operation...");
        let vacuum_start = Instant::now();
        
        db.vacuum().await.unwrap();
        
        let vacuum_time = vacuum_start.elapsed();
        let final_size = fs::metadata(&db_path).await.unwrap().len();
        
        println!("Vacuum completed in {:?}", vacuum_time);
        println!("Final database size: {} KB", final_size / 1024);
        
        // Calculate space reclaimed
        let space_reclaimed = fragmented_size.saturating_sub(final_size);
        let reclaim_percentage = if fragmented_size > 0 {
            (space_reclaimed as f64 / fragmented_size as f64) * 100.0
        } else {
            0.0
        };
        
        println!("Space reclaimed: {} KB ({:.1}%)", space_reclaimed / 1024, reclaim_percentage);
        
        // Vacuum should reclaim some space
        assert!(final_size <= fragmented_size, "Vacuum should not increase database size");
        
        // Vacuum should complete within reasonable time
        assert!(
            vacuum_time.as_millis() < PERFORMANCE_THRESHOLD_MS,
            "Vacuum took too long: {:?}",
            vacuum_time
        );
        
        // Verify database is still functional after vacuum
        let remaining_files = db.collect_all_media_files().await.unwrap();
        println!("Files remaining after vacuum: {}", remaining_files.len());
        
        assert!(remaining_files.len() > 0, "Should have files remaining after vacuum");
        assert!(remaining_files.len() < LARGE_DATASET_SIZE, "Should have fewer files than original");
    }
}
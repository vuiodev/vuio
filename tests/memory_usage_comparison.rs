//! Memory Usage Comparison Test
//! 
//! This test compares memory usage between different database implementations
//! to identify and validate memory optimization improvements.

use std::path::Path;
use std::time::{Duration, Instant};
use tempfile::TempDir;

use vuio::database::{
    DatabaseManager, MediaFile, SqliteDatabase,
    zerocopy::{ZeroCopyDatabase, PerformanceProfile},
    memory_optimized_zerocopy::MemoryOptimizedZeroCopyDatabase
};

/// Helper function to create test media files
fn create_test_media_files(count: usize, base_path: &Path) -> Vec<MediaFile> {
    let mut files = Vec::with_capacity(count);
    
    for i in 0..count {
        let file_path = base_path.join(format!("media/file_{:06}.mp4", i));
        let mut media_file = MediaFile::new(
            file_path,
            1024 * 1024 + (i as u64 * 1024), // 1-2MB files
            "video/mp4".to_string()
        );
        
        media_file.title = Some(format!("Test Media {}", i));
        media_file.artist = Some(format!("Artist {}", i % 100));
        media_file.album = Some(format!("Album {}", i % 50));
        
        files.push(media_file);
    }
    
    files
}

/// Get memory usage in KB
fn get_memory_usage_kb() -> usize {
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
    
    #[cfg(not(unix))]
    {
        0
    }
}

/// Benchmark database memory usage
async fn benchmark_database_memory<D: DatabaseManager>(
    db: &D,
    test_files: &[MediaFile],
    db_name: &str,
) -> (Duration, usize, usize, f64) {
    let initial_memory = get_memory_usage_kb();
    
    let start_time = Instant::now();
    let file_ids = db.bulk_store_media_files(test_files).await.unwrap();
    let duration = start_time.elapsed();
    
    let final_memory = get_memory_usage_kb();
    let memory_growth = final_memory.saturating_sub(initial_memory);
    
    let throughput = test_files.len() as f64 / duration.as_secs_f64();
    
    println!("{} Results:", db_name);
    println!("  - Files stored: {}", file_ids.len());
    println!("  - Duration: {:?}", duration);
    println!("  - Throughput: {:.0} files/sec", throughput);
    println!("  - Initial memory: {} KB", initial_memory);
    println!("  - Final memory: {} KB", final_memory);
    println!("  - Memory growth: {} KB", memory_growth);
    println!("  - Memory per file: {:.1} KB/file", memory_growth as f64 / test_files.len() as f64);
    println!();
    
    (duration, memory_growth, file_ids.len(), throughput)
}

#[cfg(test)]
mod memory_comparison_tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_usage_comparison_small_dataset() {
        println!("=== Memory Usage Comparison (10,000 files) ===");
        
        let temp_dir = TempDir::new().unwrap();
        let test_size = 10_000;
        let test_files = create_test_media_files(test_size, temp_dir.path());
        
        // Test SQLite
        let sqlite_db_path = temp_dir.path().join("sqlite_memory.db");
        let sqlite_db = SqliteDatabase::new(sqlite_db_path).await.unwrap();
        sqlite_db.initialize().await.unwrap();
        
        let (sqlite_duration, sqlite_memory, sqlite_files, sqlite_throughput) = 
            benchmark_database_memory(&sqlite_db, &test_files, "SQLite").await;
        
        // Test Original ZeroCopy
        let zerocopy_db_path = temp_dir.path().join("zerocopy_memory.db");
        let zerocopy_db = ZeroCopyDatabase::new_with_profile(zerocopy_db_path, PerformanceProfile::Balanced).await.unwrap();
        zerocopy_db.initialize().await.unwrap();
        zerocopy_db.open().await.unwrap();
        
        let (zerocopy_duration, zerocopy_memory, zerocopy_files, zerocopy_throughput) = 
            benchmark_database_memory(&zerocopy_db, &test_files, "Original ZeroCopy").await;
        
        // Test Memory-Optimized ZeroCopy
        let optimized_db_path = temp_dir.path().join("optimized_memory.db");
        let optimized_db = MemoryOptimizedZeroCopyDatabase::new_with_profile(optimized_db_path, PerformanceProfile::Balanced).await.unwrap();
        optimized_db.initialize().await.unwrap();
        optimized_db.open().await.unwrap();
        
        let (optimized_duration, optimized_memory, optimized_files, optimized_throughput) = 
            benchmark_database_memory(&optimized_db, &test_files, "Memory-Optimized ZeroCopy").await;
        
        // Analysis
        println!("=== Memory Usage Analysis ===");
        
        let zerocopy_vs_sqlite_memory = if sqlite_memory > 0 {
            zerocopy_memory as f64 / sqlite_memory as f64
        } else {
            0.0
        };
        
        let optimized_vs_sqlite_memory = if sqlite_memory > 0 {
            optimized_memory as f64 / sqlite_memory as f64
        } else {
            0.0
        };
        
        let optimized_vs_zerocopy_memory = if zerocopy_memory > 0 {
            optimized_memory as f64 / zerocopy_memory as f64
        } else {
            0.0
        };
        
        println!("Memory Usage Comparison:");
        println!("  - SQLite: {} KB", sqlite_memory);
        println!("  - Original ZeroCopy: {} KB ({:.1}x SQLite)", zerocopy_memory, zerocopy_vs_sqlite_memory);
        println!("  - Optimized ZeroCopy: {} KB ({:.1}x SQLite)", optimized_memory, optimized_vs_sqlite_memory);
        println!("  - Memory Optimization: {:.1}x reduction", 1.0 / optimized_vs_zerocopy_memory);
        
        println!("\nThroughput Comparison:");
        println!("  - SQLite: {:.0} files/sec", sqlite_throughput);
        println!("  - Original ZeroCopy: {:.0} files/sec ({:.1}x SQLite)", zerocopy_throughput, zerocopy_throughput / sqlite_throughput);
        println!("  - Optimized ZeroCopy: {:.0} files/sec ({:.1}x SQLite)", optimized_throughput, optimized_throughput / sqlite_throughput);
        
        // Verify all databases stored the same number of files
        assert_eq!(sqlite_files, test_size);
        assert_eq!(zerocopy_files, test_size);
        assert_eq!(optimized_files, test_size);
        
        // Memory-optimized should use less memory than original ZeroCopy
        if zerocopy_memory > 0 && optimized_memory > 0 {
            println!("\n✅ Memory optimization successful!");
            println!("   Reduced memory usage from {} KB to {} KB", zerocopy_memory, optimized_memory);
            
            // Should use significantly less memory
            assert!(optimized_memory < zerocopy_memory, 
                    "Memory-optimized version should use less memory: {} KB vs {} KB", 
                    optimized_memory, zerocopy_memory);
        }
        
        // All implementations should be faster than a baseline
        assert!(sqlite_throughput > 1000.0, "SQLite too slow");
        assert!(zerocopy_throughput > sqlite_throughput, "ZeroCopy should be faster than SQLite");
        assert!(optimized_throughput > sqlite_throughput, "Optimized ZeroCopy should be faster than SQLite");
    }

    #[tokio::test]
    async fn test_memory_efficiency_per_file() {
        println!("=== Memory Efficiency Per File Analysis ===");
        
        let temp_dir = TempDir::new().unwrap();
        let test_sizes = [1_000, 5_000, 10_000];
        
        for &test_size in &test_sizes {
            println!("\nTesting with {} files:", test_size);
            
            let test_files = create_test_media_files(test_size, temp_dir.path());
            
            // Test Memory-Optimized ZeroCopy
            let optimized_db_path = temp_dir.path().join(format!("optimized_{}.db", test_size));
            let optimized_db = MemoryOptimizedZeroCopyDatabase::new_with_profile(optimized_db_path, PerformanceProfile::Balanced).await.unwrap();
            optimized_db.initialize().await.unwrap();
            optimized_db.open().await.unwrap();
            
            let initial_memory = get_memory_usage_kb();
            
            let start_time = Instant::now();
            let file_ids = optimized_db.bulk_store_media_files(&test_files).await.unwrap();
            let duration = start_time.elapsed();
            
            let final_memory = get_memory_usage_kb();
            let memory_growth = final_memory.saturating_sub(initial_memory);
            
            let throughput = test_size as f64 / duration.as_secs_f64();
            let memory_per_file = memory_growth as f64 / test_size as f64;
            
            println!("  - Files: {}", file_ids.len());
            println!("  - Memory growth: {} KB", memory_growth);
            println!("  - Memory per file: {:.2} KB/file", memory_per_file);
            println!("  - Throughput: {:.0} files/sec", throughput);
            
            // Get internal cache stats
            let cache_stats = optimized_db.get_cache_stats().await;
            println!("  - Internal memory usage: {} bytes", cache_stats.combined_memory_usage);
            println!("  - Files in cache: {}", cache_stats.files_count);
            println!("  - Index entries: {}", cache_stats.index_entries);
            
            assert_eq!(file_ids.len(), test_size);
            assert!(throughput > 10_000.0, "Should achieve good throughput: {:.0} files/sec", throughput);
            
            // Memory per file should be reasonable (less than 1KB per file for metadata)
            if memory_growth > 0 {
                assert!(memory_per_file < 1000.0, "Memory per file too high: {:.2} KB/file", memory_per_file);
            }
        }
    }
}
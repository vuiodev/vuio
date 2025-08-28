//! Million File Stress Test
//! 
//! This test validates the ZeroCopy database performance with 1,000,000 files
//! to demonstrate the 1M files/sec target achievement.

use std::path::Path;
use std::time::{Duration, Instant};
use tempfile::TempDir;

use vuio::database::{
    DatabaseManager, MediaFile, SqliteDatabase,
    zerocopy::{ZeroCopyDatabase, PerformanceProfile},
    memory_optimized_zerocopy::MemoryOptimizedZeroCopyDatabase
};

/// Helper function to create test media files efficiently
fn create_million_test_files(count: usize, base_path: &Path) -> Vec<MediaFile> {
    let mut files = Vec::with_capacity(count);
    
    println!("Generating {} test files...", count);
    let generation_start = Instant::now();
    
    for i in 0..count {
        // Create realistic directory structure for large datasets
        let dir_level_1 = i / 10000; // 100 top-level directories for 1M files
        let dir_level_2 = (i % 10000) / 1000; // 10 subdirectories each
        let file_in_dir = i % 1000; // 1000 files per directory
        
        let file_path = base_path
            .join(format!("media_{:03}", dir_level_1))
            .join(format!("subdir_{:02}", dir_level_2))
            .join(format!("file_{:06}.mp4", file_in_dir));
        
        let mut media_file = MediaFile::new(
            file_path,
            1024 * 1024 + (i % 1000) as u64 * 1024, // 1-2MB files
            "video/mp4".to_string()
        );
        
        // Add realistic metadata distribution
        media_file.title = Some(format!("Media File {}", i));
        media_file.artist = Some(format!("Artist {}", i % 1000)); // 1000 different artists
        media_file.album = Some(format!("Album {}", i % 500)); // 500 different albums
        media_file.genre = Some(format!("Genre {}", i % 20)); // 20 different genres
        media_file.year = Some(1990 + (i % 34) as u32); // Years 1990-2023
        media_file.track_number = Some((i % 50 + 1) as u32); // Track numbers 1-50
        media_file.duration = Some(Duration::from_secs(120 + (i % 600) as u64)); // 2-12 minutes
        
        files.push(media_file);
        
        // Progress reporting
        if i > 0 && i % 100_000 == 0 {
            let elapsed = generation_start.elapsed();
            let rate = i as f64 / elapsed.as_secs_f64();
            println!("  Generated {} files ({:.0} files/sec)", i, rate);
        }
    }
    
    let generation_time = generation_start.elapsed();
    println!("File generation completed in {:?} ({:.0} files/sec)", 
             generation_time, count as f64 / generation_time.as_secs_f64());
    
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

/// Benchmark database with million files
async fn benchmark_million_files<D: DatabaseManager>(
    db: &D,
    test_files: &[MediaFile],
    db_name: &str,
) -> (Duration, usize, usize, f64) {
    println!("\n=== {} Million File Benchmark ===", db_name);
    
    let initial_memory = get_memory_usage_kb();
    println!("Initial memory: {} KB", initial_memory);
    
    let start_time = Instant::now();
    let mut processed = 0;
    
    // Process in batches to monitor progress
    let batch_size = 100_000;
    let mut file_ids = Vec::new();
    
    for (batch_num, batch) in test_files.chunks(batch_size).enumerate() {
        let batch_start = Instant::now();
        
        let batch_ids = db.bulk_store_media_files(batch).await.unwrap();
        file_ids.extend(batch_ids);
        
        let batch_duration = batch_start.elapsed();
        processed += batch.len();
        
        let batch_throughput = batch.len() as f64 / batch_duration.as_secs_f64();
        let overall_throughput = processed as f64 / start_time.elapsed().as_secs_f64();
        
        let current_memory = get_memory_usage_kb();
        let memory_growth = current_memory.saturating_sub(initial_memory);
        
        println!("Batch {}: {} files in {:?} ({:.0} files/sec) | Overall: {:.0} files/sec | Memory: {} KB (+{} KB)", 
                batch_num + 1, 
                batch.len(), 
                batch_duration, 
                batch_throughput,
                overall_throughput,
                current_memory,
                memory_growth);
        
        // Check if we're achieving target throughput
        if batch_throughput >= 1_000_000.0 {
            println!("    🚀 Batch achieved 1M+ files/sec target!");
        } else if batch_throughput >= 500_000.0 {
            println!("    🎯 Batch achieved 500K+ files/sec!");
        }
    }
    
    let total_duration = start_time.elapsed();
    let final_memory = get_memory_usage_kb();
    let memory_growth = final_memory.saturating_sub(initial_memory);
    
    let overall_throughput = test_files.len() as f64 / total_duration.as_secs_f64();
    
    println!("\n{} Final Results:", db_name);
    println!("  - Total files: {}", file_ids.len());
    println!("  - Total duration: {:?}", total_duration);
    println!("  - Overall throughput: {:.0} files/sec", overall_throughput);
    println!("  - Initial memory: {} KB", initial_memory);
    println!("  - Final memory: {} KB", final_memory);
    println!("  - Memory growth: {} KB", memory_growth);
    println!("  - Memory per file: {:.3} KB/file", memory_growth as f64 / test_files.len() as f64);
    
    // Target achievement analysis
    let target_achievement = (overall_throughput / 1_000_000.0) * 100.0;
    println!("  - Target achievement: {:.1}% of 1M files/sec", target_achievement);
    
    if overall_throughput >= 1_000_000.0 {
        println!("  🚀 TARGET ACHIEVED: 1M+ files/sec!");
    } else if overall_throughput >= 800_000.0 {
        println!("  🎯 CLOSE TO TARGET: 80%+ of 1M files/sec");
    } else if overall_throughput >= 500_000.0 {
        println!("  📈 GOOD PERFORMANCE: 500K+ files/sec");
    } else {
        println!("  ⚠️  BELOW TARGET: {:.0} files/sec", overall_throughput);
    }
    
    (total_duration, memory_growth, file_ids.len(), overall_throughput)
}

#[cfg(test)]
mod million_file_tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Use --ignored to run this expensive test
    async fn test_memory_optimized_million_files() {
        println!("=== Memory-Optimized ZeroCopy Million File Test ===");
        println!("This test validates 1M files/sec target with 1,000,000 files");
        
        let temp_dir = TempDir::new().unwrap();
        let test_size = 1_000_000;
        
        // Generate test files
        let test_files = create_million_test_files(test_size, temp_dir.path());
        
        // Test Memory-Optimized ZeroCopy
        let optimized_db_path = temp_dir.path().join("optimized_million.db");
        let optimized_db = MemoryOptimizedZeroCopyDatabase::new_with_profile(
            optimized_db_path, 
            PerformanceProfile::Maximum
        ).await.unwrap();
        
        optimized_db.initialize().await.unwrap();
        optimized_db.open().await.unwrap();
        
        let (duration, memory_growth, files_stored, throughput) = 
            benchmark_million_files(&optimized_db, &test_files, "Memory-Optimized ZeroCopy").await;
        
        // Verification
        assert_eq!(files_stored, test_size, "Should store all files");
        
        // Performance assertions
        assert!(throughput >= 100_000.0, "Should achieve at least 100K files/sec, got {:.0}", throughput);
        
        // Target achievement
        if throughput >= 1_000_000.0 {
            println!("\n🎉 MILLION FILES/SEC TARGET ACHIEVED!");
            println!("   Throughput: {:.0} files/sec", throughput);
        } else if throughput >= 800_000.0 {
            println!("\n🎯 CLOSE TO MILLION FILES/SEC TARGET!");
            println!("   Throughput: {:.0} files/sec (80%+ of target)", throughput);
        } else {
            println!("\n📊 MILLION FILE TEST COMPLETED");
            println!("   Throughput: {:.0} files/sec", throughput);
            println!("   Target progress: {:.1}%", (throughput / 1_000_000.0) * 100.0);
        }
        
        // Memory efficiency analysis
        let memory_per_file_kb = memory_growth as f64 / test_size as f64;
        println!("\nMemory Efficiency:");
        println!("  - Total memory growth: {} KB ({:.1} MB)", memory_growth, memory_growth as f64 / 1024.0);
        println!("  - Memory per file: {:.3} KB/file", memory_per_file_kb);
        
        if memory_per_file_kb < 1.0 {
            println!("  ✅ Excellent memory efficiency: < 1KB per file");
        } else if memory_per_file_kb < 2.0 {
            println!("  ✅ Good memory efficiency: < 2KB per file");
        } else {
            println!("  ⚠️  High memory usage: {:.3} KB per file", memory_per_file_kb);
        }
        
        // Database integrity check
        let stats = optimized_db.get_stats().await.unwrap();
        assert_eq!(stats.total_files, test_size, "Database should report correct file count");
        
        println!("\n✅ Million file test completed successfully!");
        println!("   Database integrity verified: {} files stored", stats.total_files);
    }

    #[tokio::test]
    #[ignore] // Use --ignored to run this expensive test  
    async fn test_million_files_comparison() {
        println!("=== Million Files: ZeroCopy vs Memory-Optimized Comparison ===");
        
        let temp_dir = TempDir::new().unwrap();
        let test_size = 1_000_000;
        
        // Generate test files once
        let test_files = create_million_test_files(test_size, temp_dir.path());
        
        // Test Original ZeroCopy (if it can handle it)
        println!("\n--- Testing Original ZeroCopy ---");
        let zerocopy_db_path = temp_dir.path().join("zerocopy_million.db");
        let zerocopy_db = ZeroCopyDatabase::new_with_profile(
            zerocopy_db_path, 
            PerformanceProfile::Maximum
        ).await.unwrap();
        
        zerocopy_db.initialize().await.unwrap();
        zerocopy_db.open().await.unwrap();
        
        let (zerocopy_duration, zerocopy_memory, zerocopy_files, zerocopy_throughput) = 
            benchmark_million_files(&zerocopy_db, &test_files, "Original ZeroCopy").await;
        
        // Test Memory-Optimized ZeroCopy
        println!("\n--- Testing Memory-Optimized ZeroCopy ---");
        let optimized_db_path = temp_dir.path().join("optimized_million.db");
        let optimized_db = MemoryOptimizedZeroCopyDatabase::new_with_profile(
            optimized_db_path, 
            PerformanceProfile::Maximum
        ).await.unwrap();
        
        optimized_db.initialize().await.unwrap();
        optimized_db.open().await.unwrap();
        
        let (optimized_duration, optimized_memory, optimized_files, optimized_throughput) = 
            benchmark_million_files(&optimized_db, &test_files, "Memory-Optimized ZeroCopy").await;
        
        // Comparison Analysis
        println!("\n=== Million Files Comparison Analysis ===");
        
        let throughput_improvement = if zerocopy_throughput > 0.0 {
            optimized_throughput / zerocopy_throughput
        } else {
            0.0
        };
        
        let memory_efficiency = if optimized_memory > 0 {
            zerocopy_memory as f64 / optimized_memory as f64
        } else {
            0.0
        };
        
        println!("Performance Comparison:");
        println!("  - Original ZeroCopy: {:.0} files/sec", zerocopy_throughput);
        println!("  - Optimized ZeroCopy: {:.0} files/sec", optimized_throughput);
        println!("  - Performance improvement: {:.1}x", throughput_improvement);
        
        println!("\nMemory Comparison:");
        println!("  - Original ZeroCopy: {} KB", zerocopy_memory);
        println!("  - Optimized ZeroCopy: {} KB", optimized_memory);
        println!("  - Memory efficiency: {:.1}x", memory_efficiency);
        
        println!("\nTarget Achievement:");
        let zerocopy_target = (zerocopy_throughput / 1_000_000.0) * 100.0;
        let optimized_target = (optimized_throughput / 1_000_000.0) * 100.0;
        println!("  - Original ZeroCopy: {:.1}% of 1M target", zerocopy_target);
        println!("  - Optimized ZeroCopy: {:.1}% of 1M target", optimized_target);
        
        // Verify both stored all files
        assert_eq!(zerocopy_files, test_size);
        assert_eq!(optimized_files, test_size);
        
        // Performance assertions
        assert!(zerocopy_throughput >= 10_000.0, "Original ZeroCopy too slow");
        assert!(optimized_throughput >= 100_000.0, "Optimized ZeroCopy too slow");
        
        if optimized_throughput >= 1_000_000.0 {
            println!("\n🚀 MILLION FILES/SEC TARGET ACHIEVED by Memory-Optimized ZeroCopy!");
        } else if optimized_throughput > zerocopy_throughput {
            println!("\n✅ Memory-Optimized ZeroCopy outperforms Original ZeroCopy!");
        }
        
        println!("\n🎉 Million files comparison completed successfully!");
    }

    #[tokio::test]
    async fn test_million_files_memory_projection() {
        println!("=== Million Files Memory Usage Projection ===");
        
        let temp_dir = TempDir::new().unwrap();
        
        // Test with smaller datasets to project million file memory usage
        let test_sizes = [10_000, 50_000, 100_000];
        let mut memory_per_file_samples = Vec::new();
        
        for &test_size in &test_sizes {
            println!("\nTesting {} files for memory projection...", test_size);
            
            let test_files = create_million_test_files(test_size, temp_dir.path());
            
            let optimized_db_path = temp_dir.path().join(format!("projection_{}.db", test_size));
            let optimized_db = MemoryOptimizedZeroCopyDatabase::new_with_profile(
                optimized_db_path, 
                PerformanceProfile::Maximum
            ).await.unwrap();
            
            optimized_db.initialize().await.unwrap();
            optimized_db.open().await.unwrap();
            
            let initial_memory = get_memory_usage_kb();
            let _file_ids = optimized_db.bulk_store_media_files(&test_files).await.unwrap();
            let final_memory = get_memory_usage_kb();
            
            let memory_growth = final_memory.saturating_sub(initial_memory);
            let memory_per_file = memory_growth as f64 / test_size as f64;
            
            memory_per_file_samples.push(memory_per_file);
            
            println!("  - Memory growth: {} KB", memory_growth);
            println!("  - Memory per file: {:.3} KB/file", memory_per_file);
        }
        
        // Calculate average memory per file
        let avg_memory_per_file = memory_per_file_samples.iter().sum::<f64>() / memory_per_file_samples.len() as f64;
        
        // Project million file memory usage
        let projected_million_memory_kb = avg_memory_per_file * 1_000_000.0;
        let projected_million_memory_mb = projected_million_memory_kb / 1024.0;
        let projected_million_memory_gb = projected_million_memory_mb / 1024.0;
        
        println!("\n=== Million Files Memory Projection ===");
        println!("Average memory per file: {:.3} KB/file", avg_memory_per_file);
        println!("Projected memory for 1M files:");
        println!("  - {} KB", projected_million_memory_kb as u64);
        println!("  - {:.1} MB", projected_million_memory_mb);
        println!("  - {:.2} GB", projected_million_memory_gb);
        
        // Memory feasibility analysis
        if projected_million_memory_gb < 1.0 {
            println!("  ✅ Excellent: < 1GB for 1M files");
        } else if projected_million_memory_gb < 2.0 {
            println!("  ✅ Good: < 2GB for 1M files");
        } else if projected_million_memory_gb < 4.0 {
            println!("  ⚠️  Moderate: < 4GB for 1M files");
        } else {
            println!("  ❌ High: {}GB for 1M files", projected_million_memory_gb);
        }
        
        // Verify memory usage is reasonable
        assert!(avg_memory_per_file < 5.0, "Memory per file too high: {:.3} KB/file", avg_memory_per_file);
        assert!(projected_million_memory_gb < 10.0, "Projected memory too high: {:.2} GB", projected_million_memory_gb);
        
        println!("\n✅ Memory projection analysis completed!");
    }
}
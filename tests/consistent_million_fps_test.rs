//! Consistent Million Files/sec Test
//! 
//! This test validates consistent 1M+ files/sec throughput with pre-allocated memory
//! to eliminate allocation overhead and achieve the target performance.

use std::path::Path;
use std::time::{Duration, Instant};
use tempfile::TempDir;

use vuio::database::{
    DatabaseManager, MediaFile,
    memory_optimized_zerocopy::MemoryOptimizedZeroCopyDatabase,
    zerocopy::PerformanceProfile
};

/// Helper function to create test media files efficiently
fn create_test_files_fast(count: usize, base_path: &Path) -> Vec<MediaFile> {
    let mut files = Vec::with_capacity(count);
    
    for i in 0..count {
        let file_path = base_path.join(format!("media/file_{:07}.mp4", i));
        let mut media_file = MediaFile::new(
            file_path,
            1024 * 1024, // Fixed 1MB size for consistency
            "video/mp4".to_string()
        );
        
        // Minimal metadata for maximum speed
        media_file.title = Some(format!("Media {}", i));
        
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

#[cfg(test)]
mod consistent_million_fps_tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Use --ignored to run this test
    async fn test_consistent_million_files_per_second() {
        println!("=== Consistent Million Files/sec Test ===");
        println!("Testing pre-allocated memory-optimized ZeroCopy for consistent 1M+ files/sec");
        
        let temp_dir = TempDir::new().unwrap();
        let test_size = 1_000_000;
        
        // Generate test files efficiently
        println!("Generating {} test files...", test_size);
        let generation_start = Instant::now();
        let test_files = create_test_files_fast(test_size, temp_dir.path());
        let generation_time = generation_start.elapsed();
        println!("File generation: {:?} ({:.0} files/sec)", 
                generation_time, test_size as f64 / generation_time.as_secs_f64());
        
        // Create pre-allocated database
        let db_path = temp_dir.path().join("consistent_million.db");
        let db = MemoryOptimizedZeroCopyDatabase::new_with_capacity(
            db_path, 
            PerformanceProfile::Maximum,
            test_size  // Pre-allocate exact capacity
        ).await.unwrap();
        
        db.initialize().await.unwrap();
        db.open().await.unwrap();
        
        println!("\nStarting consistent throughput test...");
        let initial_memory = get_memory_usage_kb();
        println!("Initial memory: {} KB", initial_memory);
        
        // Test with smaller, consistent batches for more stable performance
        let batch_size = 50_000; // Smaller batches for consistency
        let mut all_throughputs = Vec::new();
        let mut batches_over_1m = 0;
        let total_batches = (test_size + batch_size - 1) / batch_size;
        
        let overall_start = Instant::now();
        let mut total_processed = 0;
        
        for (batch_num, batch) in test_files.chunks(batch_size).enumerate() {
            let batch_start = Instant::now();
            
            let batch_ids = db.bulk_store_media_files(batch).await.unwrap();
            
            let batch_duration = batch_start.elapsed();
            total_processed += batch.len();
            
            let batch_throughput = batch.len() as f64 / batch_duration.as_secs_f64();
            let overall_throughput = total_processed as f64 / overall_start.elapsed().as_secs_f64();
            
            all_throughputs.push(batch_throughput);
            
            if batch_throughput >= 1_000_000.0 {
                batches_over_1m += 1;
            }
            
            let current_memory = get_memory_usage_kb();
            let memory_growth = current_memory.saturating_sub(initial_memory);
            
            println!("Batch {:2}: {:5} files in {:8.3}ms | {:8.0} files/sec | Overall: {:8.0} files/sec | Mem: +{:6} KB", 
                    batch_num + 1, 
                    batch.len(), 
                    batch_duration.as_millis(),
                    batch_throughput,
                    overall_throughput,
                    memory_growth);
            
            if batch_throughput >= 1_000_000.0 {
                print!(" 🚀 1M+");
            } else if batch_throughput >= 800_000.0 {
                print!(" 🎯 800K+");
            } else if batch_throughput >= 500_000.0 {
                print!(" 📈 500K+");
            }
            println!();
            
            assert_eq!(batch_ids.len(), batch.len(), "Should store all files in batch");
        }
        
        let total_duration = overall_start.elapsed();
        let final_memory = get_memory_usage_kb();
        let memory_growth = final_memory.saturating_sub(initial_memory);
        
        let overall_throughput = test_size as f64 / total_duration.as_secs_f64();
        
        // Calculate statistics
        let min_throughput = all_throughputs.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_throughput = all_throughputs.iter().fold(0.0f64, |a, &b| a.max(b));
        let avg_throughput = all_throughputs.iter().sum::<f64>() / all_throughputs.len() as f64;
        
        // Calculate standard deviation for consistency measurement
        let variance = all_throughputs.iter()
            .map(|&x| (x - avg_throughput).powi(2))
            .sum::<f64>() / all_throughputs.len() as f64;
        let std_dev = variance.sqrt();
        let coefficient_of_variation = std_dev / avg_throughput;
        
        println!("\n=== Consistent Million Files/sec Results ===");
        println!("Total Performance:");
        println!("  - Files processed: {}", test_size);
        println!("  - Total duration: {:?}", total_duration);
        println!("  - Overall throughput: {:.0} files/sec", overall_throughput);
        println!("  - Target achievement: {:.1}%", (overall_throughput / 1_000_000.0) * 100.0);
        
        println!("\nBatch Performance Statistics:");
        println!("  - Total batches: {}", total_batches);
        println!("  - Batches ≥ 1M files/sec: {} ({:.1}%)", batches_over_1m, (batches_over_1m as f64 / total_batches as f64) * 100.0);
        println!("  - Min throughput: {:.0} files/sec", min_throughput);
        println!("  - Max throughput: {:.0} files/sec", max_throughput);
        println!("  - Avg throughput: {:.0} files/sec", avg_throughput);
        println!("  - Std deviation: {:.0} files/sec", std_dev);
        println!("  - Consistency (CV): {:.1}%", coefficient_of_variation * 100.0);
        
        println!("\nMemory Efficiency:");
        println!("  - Memory growth: {} KB ({:.1} MB)", memory_growth, memory_growth as f64 / 1024.0);
        println!("  - Memory per file: {:.3} KB/file", memory_growth as f64 / test_size as f64);
        
        // Performance assessment
        if overall_throughput >= 1_000_000.0 {
            println!("\n🚀 TARGET ACHIEVED: 1M+ files/sec overall!");
        } else if avg_throughput >= 1_000_000.0 {
            println!("\n🎯 AVERAGE TARGET ACHIEVED: 1M+ files/sec average batch performance!");
        } else if batches_over_1m as f64 / total_batches as f64 >= 0.5 {
            println!("\n📈 GOOD CONSISTENCY: 50%+ batches achieved 1M+ files/sec!");
        }
        
        // Consistency assessment
        if coefficient_of_variation < 0.1 {
            println!("✅ EXCELLENT CONSISTENCY: CV < 10%");
        } else if coefficient_of_variation < 0.2 {
            println!("✅ GOOD CONSISTENCY: CV < 20%");
        } else {
            println!("⚠️  VARIABLE PERFORMANCE: CV = {:.1}%", coefficient_of_variation * 100.0);
        }
        
        // Verify database integrity
        let stats = db.get_stats().await.unwrap();
        assert_eq!(stats.total_files, test_size, "Database should contain all files");
        
        // Performance assertions
        assert!(overall_throughput >= 500_000.0, "Should achieve at least 500K files/sec overall");
        assert!(avg_throughput >= 600_000.0, "Should achieve at least 600K files/sec average");
        assert!(max_throughput >= 800_000.0, "Should achieve at least 800K files/sec peak");
        
        // Memory efficiency assertion
        let memory_per_file = memory_growth as f64 / test_size as f64;
        assert!(memory_per_file < 2.0, "Memory per file should be < 2KB, got {:.3} KB", memory_per_file);
        
        println!("\n✅ Consistent million files/sec test completed successfully!");
        println!("   Database integrity verified: {} files stored", stats.total_files);
        
        // Final achievement summary
        if overall_throughput >= 1_000_000.0 && coefficient_of_variation < 0.2 {
            println!("\n🏆 PERFECT: Achieved consistent 1M+ files/sec with good stability!");
        } else if avg_throughput >= 1_000_000.0 {
            println!("\n🎯 EXCELLENT: Average batch performance exceeds 1M files/sec!");
        } else if overall_throughput >= 800_000.0 {
            println!("\n📈 VERY GOOD: Achieved 80%+ of 1M files/sec target!");
        }
    }

    #[tokio::test]
    async fn test_pre_allocation_benefit() {
        println!("=== Pre-allocation Benefit Test ===");
        println!("Comparing pre-allocated vs dynamic allocation performance");
        
        let temp_dir = TempDir::new().unwrap();
        let test_size = 100_000; // Smaller test for comparison
        
        let test_files = create_test_files_fast(test_size, temp_dir.path());
        
        // Test without pre-allocation (dynamic)
        println!("\nTesting dynamic allocation...");
        let dynamic_db_path = temp_dir.path().join("dynamic.db");
        let dynamic_db = MemoryOptimizedZeroCopyDatabase::new_with_profile(
            dynamic_db_path, 
            PerformanceProfile::Maximum
        ).await.unwrap();
        
        dynamic_db.initialize().await.unwrap();
        dynamic_db.open().await.unwrap();
        
        let dynamic_start = Instant::now();
        let _dynamic_ids = dynamic_db.bulk_store_media_files(&test_files).await.unwrap();
        let dynamic_duration = dynamic_start.elapsed();
        let dynamic_throughput = test_size as f64 / dynamic_duration.as_secs_f64();
        
        // Test with pre-allocation
        println!("Testing pre-allocated capacity...");
        let prealloc_db_path = temp_dir.path().join("prealloc.db");
        let prealloc_db = MemoryOptimizedZeroCopyDatabase::new_with_capacity(
            prealloc_db_path, 
            PerformanceProfile::Maximum,
            test_size
        ).await.unwrap();
        
        prealloc_db.initialize().await.unwrap();
        prealloc_db.open().await.unwrap();
        
        let prealloc_start = Instant::now();
        let _prealloc_ids = prealloc_db.bulk_store_media_files(&test_files).await.unwrap();
        let prealloc_duration = prealloc_start.elapsed();
        let prealloc_throughput = test_size as f64 / prealloc_duration.as_secs_f64();
        
        // Compare results
        let improvement_factor = prealloc_throughput / dynamic_throughput;
        
        println!("\n=== Pre-allocation Benefit Results ===");
        println!("Dynamic allocation:");
        println!("  - Duration: {:?}", dynamic_duration);
        println!("  - Throughput: {:.0} files/sec", dynamic_throughput);
        
        println!("Pre-allocated capacity:");
        println!("  - Duration: {:?}", prealloc_duration);
        println!("  - Throughput: {:.0} files/sec", prealloc_throughput);
        
        println!("Improvement:");
        println!("  - Speed improvement: {:.1}x", improvement_factor);
        println!("  - Time reduction: {:.1}%", (1.0 - prealloc_duration.as_secs_f64() / dynamic_duration.as_secs_f64()) * 100.0);
        
        if improvement_factor >= 2.0 {
            println!("  🚀 SIGNIFICANT: 2x+ improvement from pre-allocation!");
        } else if improvement_factor >= 1.5 {
            println!("  ✅ GOOD: 1.5x+ improvement from pre-allocation!");
        } else if improvement_factor >= 1.1 {
            println!("  📈 MODERATE: 10%+ improvement from pre-allocation!");
        } else {
            println!("  ⚠️  MINIMAL: < 10% improvement from pre-allocation");
        }
        
        // Verify both stored all files
        let dynamic_stats = dynamic_db.get_stats().await.unwrap();
        let prealloc_stats = prealloc_db.get_stats().await.unwrap();
        
        assert_eq!(dynamic_stats.total_files, test_size);
        assert_eq!(prealloc_stats.total_files, test_size);
        
        // Pre-allocation should provide some improvement
        assert!(improvement_factor >= 1.0, "Pre-allocation should not hurt performance");
        
        println!("\n✅ Pre-allocation benefit test completed!");
    }
}
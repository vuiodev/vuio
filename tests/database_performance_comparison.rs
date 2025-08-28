use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tempfile::TempDir;

use vuio::database::{
    DatabaseManager, MediaFile, SqliteDatabase,
    zerocopy::{ZeroCopyDatabase, ZeroCopyConfig, PerformanceProfile},
};

/// Performance metrics for database operations
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub operation: String,
    pub duration: Duration,
    pub throughput_ops_per_sec: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub files_processed: usize,
}

/// Comprehensive performance test suite comparing SQL vs ZeroCopy databases
pub struct DatabasePerformanceComparison {
    temp_dir: TempDir,
    sql_db: Arc<SqliteDatabase>,
    zerocopy_db: Arc<ZeroCopyDatabase>,
    test_files: Vec<MediaFile>,
}

impl DatabasePerformanceComparison {
    /// Initialize the performance comparison test suite
    pub async fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        
        // Initialize SQL database
        let sql_db_path = temp_dir.path().join("test_sql.db");
        let sql_db = Arc::new(SqliteDatabase::new(sql_db_path).await?);
        sql_db.initialize().await?;
        
        // Initialize ZeroCopy database with balanced profile
        let zerocopy_db_path = temp_dir.path().join("test_zerocopy.db");
        let zerocopy_config = ZeroCopyConfig::with_performance_profile(PerformanceProfile::Balanced);
        let zerocopy_db = Arc::new(ZeroCopyDatabase::new(zerocopy_db_path, Some(zerocopy_config)).await?);
        zerocopy_db.initialize().await?;
        
        // Generate test data
        let test_files = Self::generate_test_media_files(10000);
        
        Ok(Self {
            temp_dir,
            sql_db,
            zerocopy_db,
            test_files,
        })
    }
    
    /// Generate synthetic media files for testing
    fn generate_test_media_files(count: usize) -> Vec<MediaFile> {
        let mut files = Vec::with_capacity(count);
        let base_path = PathBuf::from("/test/media");
        
        for i in 0..count {
            let file_path = base_path.join(format!("track_{:06}.mp3", i));
            let mut file = MediaFile::new(
                file_path,
                fastrand::u64(1_000_000..100_000_000), // 1MB to 100MB
                "audio/mpeg".to_string(),
            );
            
            // Add metadata
            file.title = Some(format!("Track {}", i));
            file.artist = Some(format!("Artist {}", i % 100)); // 100 different artists
            file.album = Some(format!("Album {}", i % 50));     // 50 different albums
            file.genre = Some(Self::random_genre());
            file.track_number = Some((i % 20) as u32 + 1);     // 1-20 tracks per album
            file.year = Some(2000 + (i % 24) as u32);          // Years 2000-2023
            file.album_artist = file.artist.clone();
            file.duration = Some(Duration::from_secs(fastrand::u64(120..600))); // 2-10 minutes
            
            files.push(file);
        }
        
        files
    }
    
    /// Get a random music genre for test data
    fn random_genre() -> String {
        let genres = [
            "Rock", "Pop", "Jazz", "Classical", "Electronic", "Hip-Hop",
            "Country", "Blues", "Folk", "Reggae", "Metal", "Punk",
            "Alternative", "Indie", "R&B", "Soul", "Funk", "Disco",
        ];
        genres[fastrand::usize(0..genres.len())].to_string()
    }
    
    /// Measure memory usage (simplified estimation)
    fn measure_memory_usage() -> f64 {
        // In a real implementation, you'd use system APIs to get actual memory usage
        // For this test, we'll return a placeholder value
        0.0
    }
    
    /// Measure CPU usage (simplified estimation)
    fn measure_cpu_usage() -> f64 {
        // In a real implementation, you'd measure actual CPU usage
        // For this test, we'll return a placeholder value
        0.0
    }
    
    /// Run bulk insert performance test
    pub async fn test_bulk_insert(&self) -> Result<(PerformanceMetrics, PerformanceMetrics)> {
        println!("🚀 Running bulk insert performance test...");
        
        // Test SQL database
        let start_time = Instant::now();
        let start_memory = Self::measure_memory_usage();
        let start_cpu = Self::measure_cpu_usage();
        
        let sql_ids = self.sql_db.bulk_store_media_files(&self.test_files).await?;
        
        let sql_duration = start_time.elapsed();
        let sql_throughput = self.test_files.len() as f64 / sql_duration.as_secs_f64();
        let sql_metrics = PerformanceMetrics {
            operation: "SQL Bulk Insert".to_string(),
            duration: sql_duration,
            throughput_ops_per_sec: sql_throughput,
            memory_usage_mb: Self::measure_memory_usage() - start_memory,
            cpu_usage_percent: Self::measure_cpu_usage() - start_cpu,
            files_processed: sql_ids.len(),
        };
        
        // Test ZeroCopy database
        let start_time = Instant::now();
        let start_memory = Self::measure_memory_usage();
        let start_cpu = Self::measure_cpu_usage();
        
        let zerocopy_ids = self.zerocopy_db.bulk_store_media_files(&self.test_files).await?;
        
        let zerocopy_duration = start_time.elapsed();
        let zerocopy_throughput = self.test_files.len() as f64 / zerocopy_duration.as_secs_f64();
        let zerocopy_metrics = PerformanceMetrics {
            operation: "ZeroCopy Bulk Insert".to_string(),
            duration: zerocopy_duration,
            throughput_ops_per_sec: zerocopy_throughput,
            memory_usage_mb: Self::measure_memory_usage() - start_memory,
            cpu_usage_percent: Self::measure_cpu_usage() - start_cpu,
            files_processed: zerocopy_ids.len(),
        };
        
        println!("✅ Bulk insert test completed");
        Ok((sql_metrics, zerocopy_metrics))
    }
    
    /// Run query performance test
    pub async fn test_query_performance(&self) -> Result<(PerformanceMetrics, PerformanceMetrics)> {
        println!("🔍 Running query performance test...");
        
        // First, ensure data is inserted
        let _ = self.sql_db.bulk_store_media_files(&self.test_files).await?;
        let _ = self.zerocopy_db.bulk_store_media_files(&self.test_files).await?;
        
        let query_count = 1000;
        
        // Test SQL database queries
        let start_time = Instant::now();
        let start_memory = Self::measure_memory_usage();
        let start_cpu = Self::measure_cpu_usage();
        
        for i in 0..query_count {
            let artist = format!("Artist {}", i % 100);
            let _ = self.sql_db.get_music_by_artist(&artist).await?;
        }
        
        let sql_duration = start_time.elapsed();
        let sql_throughput = query_count as f64 / sql_duration.as_secs_f64();
        let sql_metrics = PerformanceMetrics {
            operation: "SQL Queries".to_string(),
            duration: sql_duration,
            throughput_ops_per_sec: sql_throughput,
            memory_usage_mb: Self::measure_memory_usage() - start_memory,
            cpu_usage_percent: Self::measure_cpu_usage() - start_cpu,
            files_processed: query_count,
        };
        
        // Test ZeroCopy database queries
        let start_time = Instant::now();
        let start_memory = Self::measure_memory_usage();
        let start_cpu = Self::measure_cpu_usage();
        
        for i in 0..query_count {
            let artist = format!("Artist {}", i % 100);
            let _ = self.zerocopy_db.get_music_by_artist(&artist).await?;
        }
        
        let zerocopy_duration = start_time.elapsed();
        let zerocopy_throughput = query_count as f64 / zerocopy_duration.as_secs_f64();
        let zerocopy_metrics = PerformanceMetrics {
            operation: "ZeroCopy Queries".to_string(),
            duration: zerocopy_duration,
            throughput_ops_per_sec: zerocopy_throughput,
            memory_usage_mb: Self::measure_memory_usage() - start_memory,
            cpu_usage_percent: Self::measure_cpu_usage() - start_cpu,
            files_processed: query_count,
        };
        
        println!("✅ Query performance test completed");
        Ok((sql_metrics, zerocopy_metrics))
    }
    
    /// Run update performance test
    pub async fn test_update_performance(&self) -> Result<(PerformanceMetrics, PerformanceMetrics)> {
        println!("📝 Running update performance test...");
        
        // First, ensure data is inserted
        let _ = self.sql_db.bulk_store_media_files(&self.test_files).await?;
        let _ = self.zerocopy_db.bulk_store_media_files(&self.test_files).await?;
        
        // Create modified versions of test files
        let mut modified_files = self.test_files.clone();
        for file in &mut modified_files {
            file.title = Some(format!("Updated {}", file.title.as_ref().unwrap_or(&"Unknown".to_string())));
            file.updated_at = SystemTime::now();
        }
        
        // Test SQL database updates
        let start_time = Instant::now();
        let start_memory = Self::measure_memory_usage();
        let start_cpu = Self::measure_cpu_usage();
        
        self.sql_db.bulk_update_media_files(&modified_files).await?;
        
        let sql_duration = start_time.elapsed();
        let sql_throughput = modified_files.len() as f64 / sql_duration.as_secs_f64();
        let sql_metrics = PerformanceMetrics {
            operation: "SQL Bulk Update".to_string(),
            duration: sql_duration,
            throughput_ops_per_sec: sql_throughput,
            memory_usage_mb: Self::measure_memory_usage() - start_memory,
            cpu_usage_percent: Self::measure_cpu_usage() - start_cpu,
            files_processed: modified_files.len(),
        };
        
        // Test ZeroCopy database updates
        let start_time = Instant::now();
        let start_memory = Self::measure_memory_usage();
        let start_cpu = Self::measure_cpu_usage();
        
        self.zerocopy_db.bulk_update_media_files(&modified_files).await?;
        
        let zerocopy_duration = start_time.elapsed();
        let zerocopy_throughput = modified_files.len() as f64 / zerocopy_duration.as_secs_f64();
        let zerocopy_metrics = PerformanceMetrics {
            operation: "ZeroCopy Bulk Update".to_string(),
            duration: zerocopy_duration,
            throughput_ops_per_sec: zerocopy_throughput,
            memory_usage_mb: Self::measure_memory_usage() - start_memory,
            cpu_usage_percent: Self::measure_cpu_usage() - start_cpu,
            files_processed: modified_files.len(),
        };
        
        println!("✅ Update performance test completed");
        Ok((sql_metrics, zerocopy_metrics))
    }
    
    /// Run streaming performance test
    pub async fn test_streaming_performance(&self) -> Result<(PerformanceMetrics, PerformanceMetrics)> {
        println!("🌊 Running streaming performance test...");
        
        // First, ensure data is inserted
        let _ = self.sql_db.bulk_store_media_files(&self.test_files).await?;
        let _ = self.zerocopy_db.bulk_store_media_files(&self.test_files).await?;
        
        // Test SQL database streaming
        let start_time = Instant::now();
        let start_memory = Self::measure_memory_usage();
        let start_cpu = Self::measure_cpu_usage();
        
        let sql_files = self.sql_db.collect_all_media_files().await?;
        
        let sql_duration = start_time.elapsed();
        let sql_throughput = sql_files.len() as f64 / sql_duration.as_secs_f64();
        let sql_metrics = PerformanceMetrics {
            operation: "SQL Streaming".to_string(),
            duration: sql_duration,
            throughput_ops_per_sec: sql_throughput,
            memory_usage_mb: Self::measure_memory_usage() - start_memory,
            cpu_usage_percent: Self::measure_cpu_usage() - start_cpu,
            files_processed: sql_files.len(),
        };
        
        // Test ZeroCopy database streaming
        let start_time = Instant::now();
        let start_memory = Self::measure_memory_usage();
        let start_cpu = Self::measure_cpu_usage();
        
        let zerocopy_files = self.zerocopy_db.collect_all_media_files().await?;
        
        let zerocopy_duration = start_time.elapsed();
        let zerocopy_throughput = zerocopy_files.len() as f64 / zerocopy_duration.as_secs_f64();
        let zerocopy_metrics = PerformanceMetrics {
            operation: "ZeroCopy Streaming".to_string(),
            duration: zerocopy_duration,
            throughput_ops_per_sec: zerocopy_throughput,
            memory_usage_mb: Self::measure_memory_usage() - start_memory,
            cpu_usage_percent: Self::measure_cpu_usage() - start_cpu,
            files_processed: zerocopy_files.len(),
        };
        
        println!("✅ Streaming performance test completed");
        Ok((sql_metrics, zerocopy_metrics))
    }
    
    /// Run concurrent access performance test
    pub async fn test_concurrent_performance(&self) -> Result<(PerformanceMetrics, PerformanceMetrics)> {
        println!("🔄 Running concurrent access performance test...");
        
        // First, ensure data is inserted
        let _ = self.sql_db.bulk_store_media_files(&self.test_files).await?;
        let _ = self.zerocopy_db.bulk_store_media_files(&self.test_files).await?;
        
        let concurrent_tasks = 10;
        let operations_per_task = 100;
        
        // Test SQL database concurrent access
        let start_time = Instant::now();
        let start_memory = Self::measure_memory_usage();
        let start_cpu = Self::measure_cpu_usage();
        
        let sql_db = Arc::clone(&self.sql_db);
        let mut sql_handles = Vec::new();
        
        for task_id in 0..concurrent_tasks {
            let db = Arc::clone(&sql_db);
            let handle = tokio::spawn(async move {
                for i in 0..operations_per_task {
                    let artist = format!("Artist {}", (task_id * operations_per_task + i) % 100);
                    let _ = db.get_music_by_artist(&artist).await;
                }
            });
            sql_handles.push(handle);
        }
        
        // Wait for all SQL tasks to complete
        for handle in sql_handles {
            handle.await.unwrap();
        }
        
        let sql_duration = start_time.elapsed();
        let total_sql_ops = concurrent_tasks * operations_per_task;
        let sql_throughput = total_sql_ops as f64 / sql_duration.as_secs_f64();
        let sql_metrics = PerformanceMetrics {
            operation: "SQL Concurrent Access".to_string(),
            duration: sql_duration,
            throughput_ops_per_sec: sql_throughput,
            memory_usage_mb: Self::measure_memory_usage() - start_memory,
            cpu_usage_percent: Self::measure_cpu_usage() - start_cpu,
            files_processed: total_sql_ops,
        };
        
        // Test ZeroCopy database concurrent access
        let start_time = Instant::now();
        let start_memory = Self::measure_memory_usage();
        let start_cpu = Self::measure_cpu_usage();
        
        let zerocopy_db = Arc::clone(&self.zerocopy_db);
        let mut zerocopy_handles = Vec::new();
        
        for task_id in 0..concurrent_tasks {
            let db = Arc::clone(&zerocopy_db);
            let handle = tokio::spawn(async move {
                for i in 0..operations_per_task {
                    let artist = format!("Artist {}", (task_id * operations_per_task + i) % 100);
                    let _ = db.get_music_by_artist(&artist).await;
                }
            });
            zerocopy_handles.push(handle);
        }
        
        // Wait for all ZeroCopy tasks to complete
        for handle in zerocopy_handles {
            handle.await.unwrap();
        }
        
        let zerocopy_duration = start_time.elapsed();
        let total_zerocopy_ops = concurrent_tasks * operations_per_task;
        let zerocopy_throughput = total_zerocopy_ops as f64 / zerocopy_duration.as_secs_f64();
        let zerocopy_metrics = PerformanceMetrics {
            operation: "ZeroCopy Concurrent Access".to_string(),
            duration: zerocopy_duration,
            throughput_ops_per_sec: zerocopy_throughput,
            memory_usage_mb: Self::measure_memory_usage() - start_memory,
            cpu_usage_percent: Self::measure_cpu_usage() - start_cpu,
            files_processed: total_zerocopy_ops,
        };
        
        println!("✅ Concurrent access test completed");
        Ok((sql_metrics, zerocopy_metrics))
    }
    
    /// Print performance comparison results
    pub fn print_comparison_results(&self, results: &[(PerformanceMetrics, PerformanceMetrics)]) {
        println!("\n📊 DATABASE PERFORMANCE COMPARISON RESULTS");
        println!("{}", "=".repeat(80));
        
        for (sql_metrics, zerocopy_metrics) in results {
            println!("\n🔬 Test: {}", sql_metrics.operation.replace("SQL ", ""));
            println!("{}", "-".repeat(60));
            
            // Duration comparison
            println!("⏱️  Duration:");
            println!("   SQL:      {:>10.3}ms", sql_metrics.duration.as_millis());
            println!("   ZeroCopy: {:>10.3}ms", zerocopy_metrics.duration.as_millis());
            let duration_improvement = if sql_metrics.duration > zerocopy_metrics.duration {
                let improvement = (sql_metrics.duration.as_secs_f64() / zerocopy_metrics.duration.as_secs_f64() - 1.0) * 100.0;
                format!("ZeroCopy is {:.1}% faster", improvement)
            } else {
                let improvement = (zerocopy_metrics.duration.as_secs_f64() / sql_metrics.duration.as_secs_f64() - 1.0) * 100.0;
                format!("SQL is {:.1}% faster", improvement)
            };
            println!("   Result:   {}", duration_improvement);
            
            // Throughput comparison
            println!("\n🚀 Throughput (ops/sec):");
            println!("   SQL:      {:>10.0}", sql_metrics.throughput_ops_per_sec);
            println!("   ZeroCopy: {:>10.0}", zerocopy_metrics.throughput_ops_per_sec);
            let throughput_improvement = if zerocopy_metrics.throughput_ops_per_sec > sql_metrics.throughput_ops_per_sec {
                let improvement = (zerocopy_metrics.throughput_ops_per_sec / sql_metrics.throughput_ops_per_sec - 1.0) * 100.0;
                format!("ZeroCopy is {:.1}% faster", improvement)
            } else {
                let improvement = (sql_metrics.throughput_ops_per_sec / zerocopy_metrics.throughput_ops_per_sec - 1.0) * 100.0;
                format!("SQL is {:.1}% faster", improvement)
            };
            println!("   Result:   {}", throughput_improvement);
            
            // Files processed
            println!("\n📁 Files Processed:");
            println!("   SQL:      {:>10}", sql_metrics.files_processed);
            println!("   ZeroCopy: {:>10}", zerocopy_metrics.files_processed);
        }
        
        // Overall summary
        println!("\n🏆 OVERALL SUMMARY");
        println!("{}", "=".repeat(80));
        
        let mut sql_wins = 0;
        let mut zerocopy_wins = 0;
        let mut total_sql_time = Duration::new(0, 0);
        let mut total_zerocopy_time = Duration::new(0, 0);
        
        for (sql_metrics, zerocopy_metrics) in results {
            total_sql_time += sql_metrics.duration;
            total_zerocopy_time += zerocopy_metrics.duration;
            
            if sql_metrics.throughput_ops_per_sec > zerocopy_metrics.throughput_ops_per_sec {
                sql_wins += 1;
            } else {
                zerocopy_wins += 1;
            }
        }
        
        println!("Test Results:");
        println!("   SQL wins:      {} tests", sql_wins);
        println!("   ZeroCopy wins: {} tests", zerocopy_wins);
        
        println!("\nTotal Execution Time:");
        println!("   SQL:      {:>10.3}ms", total_sql_time.as_millis());
        println!("   ZeroCopy: {:>10.3}ms", total_zerocopy_time.as_millis());
        
        let overall_improvement = if total_sql_time > total_zerocopy_time {
            let improvement = (total_sql_time.as_secs_f64() / total_zerocopy_time.as_secs_f64() - 1.0) * 100.0;
            format!("ZeroCopy is {:.1}% faster overall", improvement)
        } else {
            let improvement = (total_zerocopy_time.as_secs_f64() / total_sql_time.as_secs_f64() - 1.0) * 100.0;
            format!("SQL is {:.1}% faster overall", improvement)
        };
        println!("   Result:   {}", overall_improvement);
        
        println!("\n💡 RECOMMENDATIONS");
        println!("{}", "=".repeat(80));
        if zerocopy_wins > sql_wins {
            println!("✅ ZeroCopy database shows better performance for this workload");
            println!("   - Consider using ZeroCopy for high-throughput scenarios");
            println!("   - ZeroCopy excels in read-heavy workloads with memory mapping");
            println!("   - Better for scenarios requiring low-latency access");
        } else {
            println!("✅ SQL database shows better performance for this workload");
            println!("   - SQL provides more consistent performance across operations");
            println!("   - Better for complex queries and transactions");
            println!("   - More mature ecosystem and tooling");
        }
        
        println!("\n📈 PERFORMANCE CHARACTERISTICS");
        println!("{}", "=".repeat(80));
        println!("SQL Database:");
        println!("   + Mature, well-tested technology");
        println!("   + ACID compliance and transactions");
        println!("   + Complex query capabilities");
        println!("   + Excellent tooling and debugging");
        println!("   - Higher memory overhead");
        println!("   - Serialization/deserialization costs");
        
        println!("\nZeroCopy Database:");
        println!("   + Zero-copy memory access");
        println!("   + Lower memory overhead");
        println!("   + Excellent for read-heavy workloads");
        println!("   + Memory-mapped file performance");
        println!("   - More complex implementation");
        println!("   - Limited query flexibility");
        println!("   - Newer, less battle-tested");
    }
    
    /// Run all performance tests and generate comprehensive report
    pub async fn run_comprehensive_benchmark(&self) -> Result<()> {
        println!("🎯 Starting comprehensive database performance comparison");
        println!("Test dataset: {} media files", self.test_files.len());
        println!("{}", "=".repeat(80));
        
        let mut all_results = Vec::new();
        
        // Run all performance tests
        all_results.push(self.test_bulk_insert().await?);
        all_results.push(self.test_query_performance().await?);
        all_results.push(self.test_update_performance().await?);
        all_results.push(self.test_streaming_performance().await?);
        all_results.push(self.test_concurrent_performance().await?);
        
        // Print comprehensive results
        self.print_comparison_results(&all_results);
        
        Ok(())
    }
}

#[tokio::test]
async fn test_database_performance_comparison() -> Result<()> {
    // Initialize tracing for better debugging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init()
        .ok();
    
    println!("🔬 Initializing database performance comparison test suite...");
    
    let comparison = DatabasePerformanceComparison::new().await?;
    comparison.run_comprehensive_benchmark().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_bulk_insert_performance() -> Result<()> {
    let comparison = DatabasePerformanceComparison::new().await?;
    let (sql_metrics, zerocopy_metrics) = comparison.test_bulk_insert().await?;
    
    println!("SQL Bulk Insert: {:.2} ops/sec", sql_metrics.throughput_ops_per_sec);
    println!("ZeroCopy Bulk Insert: {:.2} ops/sec", zerocopy_metrics.throughput_ops_per_sec);
    
    // Assert that both operations completed successfully
    assert!(sql_metrics.files_processed > 0);
    assert!(zerocopy_metrics.files_processed > 0);
    
    Ok(())
}

#[tokio::test]
async fn test_query_performance() -> Result<()> {
    let comparison = DatabasePerformanceComparison::new().await?;
    let (sql_metrics, zerocopy_metrics) = comparison.test_query_performance().await?;
    
    println!("SQL Query: {:.2} ops/sec", sql_metrics.throughput_ops_per_sec);
    println!("ZeroCopy Query: {:.2} ops/sec", zerocopy_metrics.throughput_ops_per_sec);
    
    // Assert that both operations completed successfully
    assert!(sql_metrics.files_processed > 0);
    assert!(zerocopy_metrics.files_processed > 0);
    
    Ok(())
}

#[tokio::test]
async fn test_concurrent_access_performance() -> Result<()> {
    let comparison = DatabasePerformanceComparison::new().await?;
    let (sql_metrics, zerocopy_metrics) = comparison.test_concurrent_performance().await?;
    
    println!("SQL Concurrent: {:.2} ops/sec", sql_metrics.throughput_ops_per_sec);
    println!("ZeroCopy Concurrent: {:.2} ops/sec", zerocopy_metrics.throughput_ops_per_sec);
    
    // Assert that both operations completed successfully
    assert!(sql_metrics.files_processed > 0);
    assert!(zerocopy_metrics.files_processed > 0);
    
    Ok(())
}

/// Benchmark different ZeroCopy performance profiles
#[tokio::test]
async fn test_zerocopy_performance_profiles() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_files = DatabasePerformanceComparison::generate_test_media_files(5000);
    
    let profiles = [
        PerformanceProfile::Minimal,
        PerformanceProfile::Balanced,
        PerformanceProfile::HighPerformance,
        PerformanceProfile::Maximum,
    ];
    
    println!("🔬 Testing ZeroCopy performance profiles...");
    
    for profile in &profiles {
        let db_path = temp_dir.path().join(format!("zerocopy_{:?}.db", profile));
        let config = ZeroCopyConfig::with_performance_profile(*profile);
        let db = ZeroCopyDatabase::new(db_path, Some(config)).await?;
        db.initialize().await?;
        
        let start_time = Instant::now();
        let _ids = db.bulk_store_media_files(&test_files).await?;
        let duration = start_time.elapsed();
        let throughput = test_files.len() as f64 / duration.as_secs_f64();
        
        println!("Profile {:?}: {:.0} files/sec ({:.2}ms)", 
                 profile, throughput, duration.as_millis());
    }
    
    Ok(())
}

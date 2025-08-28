use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tempfile::TempDir;
use tracing::info;

use vuio::database::{
    DatabaseManager, MediaFile, SqliteDatabase,
    zerocopy::{ZeroCopyDatabase, ZeroCopyConfig, PerformanceProfile},
};

/// A struct to hold performance results for a single test run.
#[derive(Debug, Clone)]
struct PerfResult {
    operation: String,
    db_type: String,
    num_files: usize,
    duration: Duration,
    throughput: f64, // ops/sec
}

/// Generates a vector of synthetic media files for performance testing.
fn generate_test_media_files(count: usize, base_path: &Path) -> Vec<MediaFile> {
    let mut files = Vec::with_capacity(count);
    let now = SystemTime::now();

    for i in 0..count {
        let dir_index = i / 1000;
        let file_path = base_path.join(format!("dir_{}", dir_index)).join(format!("media_file_{:06}.mp4", i));
        
        let mut media_file = MediaFile::new(
            file_path,
            1_048_576 + (i as u64 * 1024), // 1MB + some variation
            "video/mp4".to_string(),
        );
        media_file.modified = now;
        media_file.title = Some(format!("Test Media {}", i));
        media_file.artist = Some(format!("Artist {}", i % 100));
        media_file.album = Some(format!("Album {}", i % 50));
        media_file.year = Some(2000 + (i % 24) as u32);
        media_file.duration = Some(Duration::from_secs(180 + (i % 120) as u64));

        files.push(media_file);
    }
    files
}

/// Sets up both SQL and ZeroCopy databases for a benchmark run.
async fn setup_databases(
    temp_dir: &TempDir,
    num_files: usize,
) -> Result<(Arc<SqliteDatabase>, Arc<ZeroCopyDatabase>)> {
    // Setup SQL database
    let sql_db_path = temp_dir.path().join(format!("sql_perf_{}.db", num_files));
    let sql_db = Arc::new(SqliteDatabase::new(sql_db_path).await?);
    sql_db.initialize().await?;

    // Setup ZeroCopy database with a high-performance profile for benchmarking
    let zerocopy_db_path = temp_dir.path().join(format!("zerocopy_perf_{}.db", num_files));
    let mut zerocopy_config = ZeroCopyConfig::with_performance_profile(PerformanceProfile::HighPerformance);
    // Disable auto-scaling to ensure a consistent profile throughout the test.
    zerocopy_config.enable_auto_scaling = false;

    let zerocopy_db = Arc::new(ZeroCopyDatabase::new(zerocopy_db_path, Some(zerocopy_config)).await?);
    zerocopy_db.initialize().await?;
    zerocopy_db.open().await?;

    Ok((sql_db, zerocopy_db))
}

/// Prints a formatted summary table of performance results.
fn print_summary(num_files: usize, results: &[(PerfResult, PerfResult)]) {
    info!("\n📊 --- Performance Summary for {} files --- 📊", num_files);
    info!("{:-<105}", "");
    info!(
        "| {:<25} | {:<25} | {:<25} | {:<20} |",
        "Operation", "SQL Database", "ZeroCopy Database", "Winner"
    );
    info!("{:-<105}", "");

    for (sql, zerocopy) in results {
        let winner = if sql.duration < zerocopy.duration { "SQL" } else { "ZeroCopy" };
        let sql_duration = format!("{:.2?}", sql.duration);
        let zerocopy_duration = format!("{:.2?}", zerocopy.duration);

        info!(
            "| {:<25} | {:<25} | {:<25} | {:<20} |",
            sql.operation, sql_duration, zerocopy_duration, winner
        );
    }
    info!("{:-<105}", "");
}

/// Main benchmark runner for a given number of files.
async fn run_benchmark(num_files: usize) -> Result<()> {
    info!("\n\n--- Running Benchmark for {} files ---", num_files);
    let temp_dir = TempDir::new()?;
    let (sql_db, zerocopy_db) = setup_databases(&temp_dir, num_files).await?;
    let test_files = generate_test_media_files(num_files, temp_dir.path());
    
    let mut all_results = Vec::new();

    // --- Test 1: Bulk Insert Performance ---
    info!("\n--- Test 1: Bulk Insert Performance ---");
    let start_sql = Instant::now();
    sql_db.bulk_store_media_files(&test_files).await?;
    let sql_insert_duration = start_sql.elapsed();
    
    let start_zerocopy = Instant::now();
    zerocopy_db.bulk_store_media_files(&test_files).await?;
    let zerocopy_insert_duration = start_zerocopy.elapsed();

    all_results.push((
        PerfResult {
            operation: "Bulk Insert".to_string(),
            db_type: "SQL".to_string(),
            num_files,
            duration: sql_insert_duration,
            throughput: num_files as f64 / sql_insert_duration.as_secs_f64(),
        },
        PerfResult {
            operation: "Bulk Insert".to_string(),
            db_type: "ZeroCopy".to_string(),
            num_files,
            duration: zerocopy_insert_duration,
            throughput: num_files as f64 / zerocopy_insert_duration.as_secs_f64(),
        },
    ));

    // --- Test 2: Diverse Query Performance ---
    info!("\n--- Test 2: Diverse Query Performance ---");
    let query_count = 100.min(num_files);
    let mut paths_to_query = Vec::new();
    let mut dirs_to_query = HashSet::new();
    for i in 0..query_count {
        let file = &test_files[i * (num_files / query_count)];
        paths_to_query.push(file.path.clone());
        if let Some(parent) = file.path.parent() {
            dirs_to_query.insert(parent.to_path_buf());
        }
    }

    let start_sql = Instant::now();
    for path in &paths_to_query {
        let _ = sql_db.get_file_by_path(path).await?;
    }
    for dir in &dirs_to_query {
        let _ = sql_db.get_directory_listing(dir, "").await?;
    }
    let sql_query_duration = start_sql.elapsed();

    let start_zerocopy = Instant::now();
    for path in &paths_to_query {
        let _ = zerocopy_db.get_file_by_path(path).await?;
    }
    for dir in &dirs_to_query {
        let _ = zerocopy_db.get_directory_listing(dir, "").await?;
    }
    let zerocopy_query_duration = start_zerocopy.elapsed();

    let total_queries = paths_to_query.len() + dirs_to_query.len();
    all_results.push((
        PerfResult {
            operation: "Diverse Queries".to_string(),
            db_type: "SQL".to_string(),
            num_files: total_queries,
            duration: sql_query_duration,
            throughput: total_queries as f64 / sql_query_duration.as_secs_f64(),
        },
        PerfResult {
            operation: "Diverse Queries".to_string(),
            db_type: "ZeroCopy".to_string(),
            num_files: total_queries,
            duration: zerocopy_query_duration,
            throughput: total_queries as f64 / zerocopy_query_duration.as_secs_f64(),
        },
    ));

    // --- Test 3: Cleanup Performance ---
    info!("\n--- Test 3: Cleanup Performance ---");
    let mut existing_paths_vec = Vec::new();
    let mut existing_paths_set = HashSet::new();
    for (i, file) in test_files.iter().enumerate() {
        if i % 2 == 0 { // Keep half the files
            existing_paths_vec.push(file.path.to_string_lossy().to_string());
            existing_paths_set.insert(file.path.to_string_lossy().to_string());
        }
    }

    let start_sql = Instant::now();
    let sql_deleted = sql_db.database_native_cleanup(&existing_paths_vec).await?;
    let sql_cleanup_duration = start_sql.elapsed();

    let start_zerocopy = Instant::now();
    let zerocopy_deleted = zerocopy_db.batch_cleanup_missing_files(&existing_paths_set).await?;
    let zerocopy_cleanup_duration = start_zerocopy.elapsed();
    
    info!("SQL cleanup deleted {} files.", sql_deleted);
    info!("ZeroCopy cleanup deleted {} files.", zerocopy_deleted);

    all_results.push((
        PerfResult {
            operation: "Cleanup".to_string(),
            db_type: "SQL".to_string(),
            num_files: sql_deleted,
            duration: sql_cleanup_duration,
            throughput: sql_deleted as f64 / sql_cleanup_duration.as_secs_f64(),
        },
        PerfResult {
            operation: "Cleanup".to_string(),
            db_type: "ZeroCopy".to_string(),
            num_files: zerocopy_deleted,
            duration: zerocopy_cleanup_duration,
            throughput: zerocopy_deleted as f64 / zerocopy_cleanup_duration.as_secs_f64(),
        },
    ));

    print_summary(num_files, &all_results);
    zerocopy_db.close().await?;
    Ok(())
}

/// The main entry point for the advanced performance benchmark suite.
/// This test is ignored by default to prevent it from running on every `cargo test`.
/// Run it explicitly with: `cargo test --release --test advanced_db_perf_test -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn advanced_performance_benchmark_suite() -> Result<()> {
    // Initialize logging to see the output.
    let _ = tracing_subscriber::fmt().with_env_filter("info,sqlx=warn").try_init();

    info!("Starting Advanced Database Performance Benchmark Suite...");
    info!("This test will compare SQLite and ZeroCopy database implementations.");
    
    for num_files in [10_000, 100_000].iter() {
        if let Err(e) = run_benchmark(*num_files).await {
            eprintln!("Benchmark for {} files failed: {}", num_files, e);
        }
    }
    
    info!("\nAdvanced Database Performance Benchmark Suite Finished.");
    Ok(())
}
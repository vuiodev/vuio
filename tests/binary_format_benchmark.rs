use anyhow::Result;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use vuio::database::{MediaFile, binary_format::BinaryMediaFileSerializer};

/// Benchmark comparing custom binary format vs FlatBuffers
#[tokio::test]
#[ignore] // Run with: cargo test binary_format_benchmark --test binary_format_benchmark -- --ignored
async fn binary_format_benchmark() -> Result<()> {
    println!("🚀 BINARY FORMAT PERFORMANCE BENCHMARK");
    println!("{}", "=".repeat(60));
    
    // Generate test data
    let test_files = generate_test_files(10_000);
    println!("📊 Testing with {} media files", test_files.len());
    
    // Test custom binary format
    println!("\n🔥 CUSTOM BINARY FORMAT");
    println!("{}", "-".repeat(40));
    
    let start = Instant::now();
    let binary_data = BinaryMediaFileSerializer::serialize_batch(&test_files)?;
    let binary_serialize_time = start.elapsed();
    
    let start = Instant::now();
    let binary_deserialized = BinaryMediaFileSerializer::deserialize_batch(&binary_data)?;
    let binary_deserialize_time = start.elapsed();
    
    println!("✅ Serialization:   {:>8.2}ms ({:>8.0} files/sec)", 
             binary_serialize_time.as_millis(),
             test_files.len() as f64 / binary_serialize_time.as_secs_f64());
    
    println!("✅ Deserialization: {:>8.2}ms ({:>8.0} files/sec)", 
             binary_deserialize_time.as_millis(),
             binary_deserialized.len() as f64 / binary_deserialize_time.as_secs_f64());
    
    println!("📦 Data size:       {:>8} bytes ({:.1} bytes/file)", 
             binary_data.len(),
             binary_data.len() as f64 / test_files.len() as f64);
    
    // Performance summary
    println!("\n🏆 PERFORMANCE SUMMARY");
    println!("{}", "=".repeat(60));
    
    let total_throughput = test_files.len() as f64 / (binary_serialize_time.as_secs_f64() + binary_deserialize_time.as_secs_f64());
    
    println!("⚡ Total throughput:        {:.0} files/sec", total_throughput);
    println!("📦 Data efficiency:        {:.1} bytes/file", binary_data.len() as f64 / test_files.len() as f64);
    println!("🎯 Round-trip time:        {:.2}ms", (binary_serialize_time + binary_deserialize_time).as_millis());
    
    // Memory efficiency
    println!("\n💾 MEMORY EFFICIENCY");
    println!("{}", "-".repeat(40));
    println!("Custom Binary: {} bytes total, {:.1} bytes/file", 
             binary_data.len(), binary_data.len() as f64 / test_files.len() as f64);
    
    // Verify correctness
    assert_eq!(test_files.len(), binary_deserialized.len());
    
    for i in 0..test_files.len().min(10) {
        assert_eq!(test_files[i].filename, binary_deserialized[i].filename);
        assert_eq!(test_files[i].title, binary_deserialized[i].title);
    }
    
    println!("\n✅ All correctness tests passed!");
    
    Ok(())
}

fn generate_test_files(count: usize) -> Vec<MediaFile> {
    let mut files = Vec::with_capacity(count);
    let base_path = PathBuf::from("/test/media");
    
    for i in 0..count {
        let file_path = base_path.join(format!("track_{:06}.mp3", i));
        let mut file = MediaFile::new(
            file_path,
            fastrand::u64(1_000_000..100_000_000), // 1MB to 100MB
            "audio/mpeg".to_string(),
        );
        
        // Add metadata (realistic distribution)
        file.title = Some(format!("Track {}", i));
        file.artist = Some(format!("Artist {}", i % 100)); // 100 different artists
        file.album = Some(format!("Album {}", i % 50));     // 50 different albums
        file.genre = Some(random_genre());
        file.track_number = Some((i % 20) as u32 + 1);     // 1-20 tracks per album
        file.year = Some(2000 + (i % 24) as u32);          // Years 2000-2023
        file.album_artist = file.artist.clone();
        file.duration = Some(Duration::from_secs(fastrand::u64(120..600))); // 2-10 minutes
        
        files.push(file);
    }
    
    files
}

fn random_genre() -> String {
    let genres = [
        "Rock", "Pop", "Jazz", "Classical", "Electronic", "Hip-Hop",
        "Country", "Blues", "Folk", "Reggae", "Metal", "Punk",
    ];
    genres[fastrand::usize(0..genres.len())].to_string()
}

// Removed FlatBuffer serialization functions (using binary format only)

/// Micro-benchmark for individual file operations
#[tokio::test]
#[ignore]
async fn micro_benchmark() -> Result<()> {
    println!("🔬 MICRO-BENCHMARK: Single File Operations");
    println!("{}", "=".repeat(50));
    
    let test_file = generate_test_files(1)[0].clone();
    let iterations = 100_000;
    
    // Binary format micro-benchmark
    println!("\n🔥 Custom Binary Format ({} iterations)", iterations);
    
    let start = Instant::now();
    for _ in 0..iterations {
        let data = BinaryMediaFileSerializer::serialize(&test_file)?;
        let _ = BinaryMediaFileSerializer::deserialize(&data)?;
    }
    let binary_time = start.elapsed();
    
    println!("⚡ Time per operation: {:.2}μs", binary_time.as_micros() as f64 / iterations as f64);
    println!("⚡ Operations per sec: {:.0}", iterations as f64 / binary_time.as_secs_f64());
    
    println!("\n🏆 Custom Binary format performance: {:.0} operations/sec!", iterations as f64 / binary_time.as_secs_f64());
    
    Ok(())
}
//! Test to verify that the ZeroCopy database can now read and write data correctly

use anyhow::Result;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;
use tokio::runtime::Runtime;

use vuio::database::{
    DatabaseManager, MediaFile,
    zerocopy::{ZeroCopyDatabase, PerformanceProfile}
};

/// Create a test media file
fn create_test_file(id: usize, base_path: &std::path::Path) -> MediaFile {
    MediaFile {
        id: None,
        path: base_path.join(format!("test_{}.mp3", id)),
        filename: format!("test_{}.mp3", id),
        size: 1024 * 1024 * 3, // 3MB
        modified: SystemTime::now(),
        mime_type: "audio/mpeg".to_string(),
        duration: Some(Duration::from_secs(180)),
        title: Some(format!("Test Song {}", id)),
        artist: Some("Test Artist".to_string()),
        album: Some("Test Album".to_string()),
        genre: Some("Test Genre".to_string()),
        track_number: Some(id as u32),
        year: Some(2023),
        album_artist: Some("Test Artist".to_string()),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
    }
}

#[tokio::test]
async fn test_zerocopy_write_and_read_cycle() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test_complete.db");
    
    // Create ZeroCopy database
    let db = ZeroCopyDatabase::new_with_auto_detection(db_path.clone()).await?;
    db.initialize().await?;
    
    println!("✅ ZeroCopy database initialized");
    
    // Create test files
    let test_files: Vec<MediaFile> = (1..6)
        .map(|i| create_test_file(i, temp_dir.path()))
        .collect();
    
    println!("📁 Created {} test files", test_files.len());
    
    // Store files using bulk operation
    let file_ids = db.bulk_store_media_files(&test_files).await?;
    println!("💾 Stored {} files with IDs: {:?}", file_ids.len(), file_ids);
    
    // Verify we can read files back by ID
    for (i, &file_id) in file_ids.iter().enumerate() {
        match db.get_file_by_id(file_id).await? {
            Some(retrieved_file) => {
                println!("✅ Retrieved file {}: {}", file_id, retrieved_file.filename);
                
                // Verify the data matches what we stored
                assert_eq!(retrieved_file.filename, test_files[i].filename);
                assert_eq!(retrieved_file.size, test_files[i].size);
                assert_eq!(retrieved_file.mime_type, test_files[i].mime_type);
                assert_eq!(retrieved_file.title, test_files[i].title);
                assert_eq!(retrieved_file.artist, test_files[i].artist);
            }
            None => {
                panic!("❌ Failed to retrieve file with ID {}", file_id);
            }
        }
    }
    
    // Verify we can read files back by path
    for test_file in &test_files {
        match db.get_file_by_path(&test_file.path).await? {
            Some(retrieved_file) => {
                println!("✅ Retrieved by path: {}", retrieved_file.filename);
                assert_eq!(retrieved_file.filename, test_file.filename);
            }
            None => {
                panic!("❌ Failed to retrieve file by path: {}", test_file.path.display());
            }
        }
    }
    
    // Test database persistence (close and reopen)
    drop(db);
    println!("🔄 Database closed, reopening...");
    
    let db2 = ZeroCopyDatabase::new_with_auto_detection(db_path).await?;
    db2.initialize().await?;
    
    // Verify data persists after restart
    let stats = db2.get_stats().await?;
    println!("📊 After restart - Total files: {}", stats.total_files);
    
    // Try to read one file to verify persistence
    if let Some(retrieved_file) = db2.get_file_by_id(file_ids[0]).await? {
        println!("✅ Data persisted correctly: {}", retrieved_file.filename);
        assert_eq!(retrieved_file.filename, test_files[0].filename);
    } else {
        panic!("❌ Data did not persist after database restart");
    }
    
    println!("🎉 All tests passed! ZeroCopy database is fully functional.");
    Ok(())
}

#[test]
fn run_complete_functionality_test() {
    let rt = Runtime::new().unwrap();
    
    println!("🔬 ZeroCopy Database Complete Functionality Test");
    println!("Testing: Write → Read → Persistence → Deserialization");
    
    rt.block_on(async {
        test_zerocopy_write_and_read_cycle().await.unwrap();
    });
    
    println!("✅ ZeroCopy database implementation is complete and working!");
}
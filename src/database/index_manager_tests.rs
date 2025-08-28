#[cfg(test)]
mod tests {
    use crate::database::index_manager::{IndexManager, IndexType, MemoryBoundedCache};
    use crate::database::MediaFile;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};
    use tempfile::tempdir;
    use tokio;

    fn create_test_media_file(id: i64, path: &str, artist: Option<&str>, album: Option<&str>, genre: Option<&str>, year: Option<u32>) -> MediaFile {
        MediaFile {
            id: Some(id),
            path: PathBuf::from(path),
            filename: PathBuf::from(path).file_name().unwrap().to_string_lossy().to_string(),
            size: 1024,
            modified: SystemTime::now(),
            mime_type: "audio/mp3".to_string(),
            duration: Some(Duration::from_secs(180)),
            title: Some("Test Song".to_string()),
            artist: artist.map(|s| s.to_string()),
            album: album.map(|s| s.to_string()),
            genre: genre.map(|s| s.to_string()),
            track_number: Some(1),
            year,
            album_artist: None,
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        }
    }

    #[test]
    fn test_memory_bounded_cache_basic_operations() {
        let mut cache = MemoryBoundedCache::<String, u64>::new(100, 1024);
        
        // Test insertion and retrieval
        cache.insert("key1".to_string(), 42);
        assert_eq!(cache.get(&"key1".to_string()), Some(42));
        assert_eq!(cache.get(&"nonexistent".to_string()), None);
        
        // Test cache statistics
        let stats = cache.get_stats();
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.hit_count, 1);
        assert_eq!(stats.miss_count, 1);
        assert!(stats.hit_rate > 0.0);
    }

    #[test]
    fn test_memory_bounded_cache_eviction() {
        let mut cache = MemoryBoundedCache::<String, u64>::new(2, 1024); // Max 2 entries
        
        // Fill cache to capacity
        cache.insert("key1".to_string(), 1);
        cache.insert("key2".to_string(), 2);
        
        // Access key1 to make it more recently used
        cache.get(&"key1".to_string());
        
        // Insert key3, should evict key2 (least recently used)
        cache.insert("key3".to_string(), 3);
        
        assert_eq!(cache.get(&"key1".to_string()), Some(1)); // Should still exist
        assert_eq!(cache.get(&"key2".to_string()), None);    // Should be evicted
        assert_eq!(cache.get(&"key3".to_string()), Some(3)); // Should exist
        
        let stats = cache.get_stats();
        assert_eq!(stats.entries, 2);
        assert_eq!(stats.eviction_count, 1);
    }

    #[test]
    fn test_index_manager_basic_operations() {
        let mut index_manager = IndexManager::new(1000, 1024 * 1024); // 1MB memory limit
        
        let file = create_test_media_file(1, "/music/artist/album/song.mp3", Some("Artist"), Some("Album"), Some("Rock"), Some(2023));
        
        // Test insertion
        index_manager.insert_file_index(&file, 1000);
        
        // Test path lookup
        assert_eq!(index_manager.find_by_path("/music/artist/album/song.mp3"), Some(1000));
        assert_eq!(index_manager.find_by_path("/nonexistent.mp3"), None);
        
        // Test ID lookup
        assert_eq!(index_manager.find_by_id(1), Some(1000));
        assert_eq!(index_manager.find_by_id(999), None);
        
        // Test directory lookup
        let files_in_dir = index_manager.find_files_in_directory("/music/artist/album");
        assert_eq!(files_in_dir, vec![1000]);
        
        // Test music categorization
        let artist_files = index_manager.find_files_by_artist("Artist");
        assert_eq!(artist_files, vec![1000]);
        
        let album_files = index_manager.find_files_by_album("Album");
        assert_eq!(album_files, vec![1000]);
        
        let genre_files = index_manager.find_files_by_genre("Rock");
        assert_eq!(genre_files, vec![1000]);
        
        let year_files = index_manager.find_files_by_year(2023);
        assert_eq!(year_files, vec![1000]);
        
        // Test statistics
        let stats = index_manager.get_stats();
        assert_eq!(stats.insert_count, 1);
        assert!(stats.lookup_count >= 8); // At least 8 lookups, may be more due to internal operations
        assert!(stats.is_dirty);
        assert_eq!(stats.generation, 2); // Started at 1, incremented on insert
    }

    #[test]
    fn test_index_manager_removal() {
        let mut index_manager = IndexManager::new(1000, 1024 * 1024);
        
        let file = create_test_media_file(1, "/music/test.mp3", Some("Artist"), None, None, None);
        
        // Insert and then remove
        index_manager.insert_file_index(&file, 1000);
        assert_eq!(index_manager.find_by_path("/music/test.mp3"), Some(1000));
        
        let removed_offset = index_manager.remove_file_index("/music/test.mp3");
        assert_eq!(removed_offset, Some(1000));
        assert_eq!(index_manager.find_by_path("/music/test.mp3"), None);
        
        let stats = index_manager.get_stats();
        assert_eq!(stats.insert_count, 1);
        assert_eq!(stats.remove_count, 1);
    }

    #[test]
    fn test_index_manager_subdirectories() {
        let mut index_manager = IndexManager::new(1000, 1024 * 1024);
        
        // Add files in different directories
        let file1 = create_test_media_file(1, "/music/rock/band1/song1.mp3", None, None, None, None);
        let file2 = create_test_media_file(2, "/music/rock/band2/song2.mp3", None, None, None, None);
        let file3 = create_test_media_file(3, "/music/jazz/artist1/song3.mp3", None, None, None, None);
        
        index_manager.insert_file_index(&file1, 1000);
        index_manager.insert_file_index(&file2, 2000);
        index_manager.insert_file_index(&file3, 3000);
        
        // Test subdirectory discovery
        let subdirs = index_manager.find_subdirectories("/music");
        assert!(subdirs.contains(&"/music/jazz".to_string()));
        assert!(subdirs.contains(&"/music/rock".to_string()));
        
        let rock_subdirs = index_manager.find_subdirectories("/music/rock");
        assert!(rock_subdirs.contains(&"/music/rock/band1".to_string()));
        assert!(rock_subdirs.contains(&"/music/rock/band2".to_string()));
    }

    #[test]
    fn test_index_manager_music_categorization() {
        let mut index_manager = IndexManager::new(1000, 1024 * 1024);
        
        // Add files with different metadata
        let file1 = create_test_media_file(1, "/music/song1.mp3", Some("Artist A"), Some("Album X"), Some("Rock"), Some(2020));
        let file2 = create_test_media_file(2, "/music/song2.mp3", Some("Artist A"), Some("Album Y"), Some("Pop"), Some(2021));
        let file3 = create_test_media_file(3, "/music/song3.mp3", Some("Artist B"), Some("Album X"), Some("Rock"), Some(2020));
        
        index_manager.insert_file_index(&file1, 1000);
        index_manager.insert_file_index(&file2, 2000);
        index_manager.insert_file_index(&file3, 3000);
        
        // Test artist categorization
        let artists = index_manager.get_all_artists();
        let artist_names: Vec<String> = artists.iter().map(|(name, _)| name.clone()).collect();
        assert!(artist_names.contains(&"Artist A".to_string()));
        assert!(artist_names.contains(&"Artist B".to_string()));
        
        let artist_a_files = index_manager.find_files_by_artist("Artist A");
        assert_eq!(artist_a_files.len(), 2);
        assert!(artist_a_files.contains(&1000));
        assert!(artist_a_files.contains(&2000));
        
        // Test album categorization
        let albums = index_manager.get_all_albums();
        let album_names: Vec<String> = albums.iter().map(|(name, _)| name.clone()).collect();
        assert!(album_names.contains(&"Album X".to_string()));
        assert!(album_names.contains(&"Album Y".to_string()));
        
        let album_x_files = index_manager.find_files_by_album("Album X");
        assert_eq!(album_x_files.len(), 2);
        assert!(album_x_files.contains(&1000));
        assert!(album_x_files.contains(&3000));
        
        // Test genre categorization
        let genres = index_manager.get_all_genres();
        let genre_names: Vec<String> = genres.iter().map(|(name, _)| name.clone()).collect();
        assert!(genre_names.contains(&"Rock".to_string()));
        assert!(genre_names.contains(&"Pop".to_string()));
        
        let rock_files = index_manager.find_files_by_genre("Rock");
        assert_eq!(rock_files.len(), 2);
        assert!(rock_files.contains(&1000));
        assert!(rock_files.contains(&3000));
        
        // Test year categorization
        let years = index_manager.get_all_years();
        let year_values: Vec<u32> = years.iter().map(|(year, _)| *year).collect();
        assert!(year_values.contains(&2020));
        assert!(year_values.contains(&2021));
        
        let year_2020_files = index_manager.find_files_by_year(2020);
        assert_eq!(year_2020_files.len(), 2);
        assert!(year_2020_files.contains(&1000));
        assert!(year_2020_files.contains(&3000));
    }

    #[test]
    fn test_index_manager_dirty_tracking() {
        let mut index_manager = IndexManager::new(1000, 1024 * 1024);
        
        // Initially clean
        assert!(!index_manager.is_dirty());
        assert!(!index_manager.is_index_dirty(IndexType::PathToOffset));
        
        // Insert file should mark indexes as dirty
        let file = create_test_media_file(1, "/music/test.mp3", Some("Artist"), None, None, None);
        index_manager.insert_file_index(&file, 1000);
        
        assert!(index_manager.is_dirty());
        assert!(index_manager.is_index_dirty(IndexType::PathToOffset));
        assert!(index_manager.is_index_dirty(IndexType::DirectoryIndex));
        assert!(index_manager.is_index_dirty(IndexType::ArtistIndex));
        
        // Mark clean
        index_manager.mark_clean();
        assert!(!index_manager.is_dirty());
        assert!(!index_manager.is_index_dirty(IndexType::PathToOffset));
    }

    #[tokio::test]
    async fn test_index_persistence() {
        let temp_dir = tempdir().unwrap();
        let index_file = temp_dir.path().join("test_index.idx");
        
        // Create index manager and add some data
        let mut index_manager = IndexManager::new(1000, 1024 * 1024);
        
        let file1 = create_test_media_file(1, "/music/artist1/song1.mp3", Some("Artist 1"), Some("Album 1"), None, None);
        let file2 = create_test_media_file(2, "/music/artist2/song2.mp3", Some("Artist 2"), Some("Album 2"), None, None);
        
        index_manager.insert_file_index(&file1, 1000);
        index_manager.insert_file_index(&file2, 2000);
        
        // Persist indexes
        index_manager.persist_indexes(&index_file).await.unwrap();
        assert!(!index_manager.is_dirty()); // Should be clean after persistence
        
        // Create new index manager and load
        let mut new_index_manager = IndexManager::new(1000, 1024 * 1024);
        new_index_manager.load_indexes(&index_file).await.unwrap();
        
        // Verify data was loaded correctly
        let files_in_artist1_dir = new_index_manager.find_files_in_directory("/music/artist1");
        assert_eq!(files_in_artist1_dir, vec![1000]);
        
        let files_in_artist2_dir = new_index_manager.find_files_in_directory("/music/artist2");
        assert_eq!(files_in_artist2_dir, vec![2000]);
        
        let artist1_files = new_index_manager.find_files_by_artist("Artist 1");
        assert_eq!(artist1_files, vec![1000]);
        
        let artist2_files = new_index_manager.find_files_by_artist("Artist 2");
        assert_eq!(artist2_files, vec![2000]);
    }

    #[test]
    fn test_index_stats_comprehensive() {
        let mut index_manager = IndexManager::new(1000, 1024 * 1024);
        
        // Add multiple files
        for i in 1..=10 {
            let file = create_test_media_file(
                i,
                &format!("/music/artist{}/song{}.mp3", i % 3, i),
                Some(&format!("Artist {}", i % 3)),
                Some(&format!("Album {}", i % 2)),
                Some(&format!("Genre {}", i % 4)),
                Some(2020 + (i as u32 % 5)),
            );
            index_manager.insert_file_index(&file, i as u64 * 1000);
        }
        
        // Perform some lookups
        for i in 1..=5 {
            index_manager.find_by_id(i);
            index_manager.find_by_path(&format!("/music/artist{}/song{}.mp3", i % 3, i));
        }
        
        let stats = index_manager.get_stats();
        
        // Verify comprehensive statistics
        assert_eq!(stats.insert_count, 10);
        assert!(stats.lookup_count >= 10); // At least 10 lookups (5 ID + 5 path), may be more
        assert!(stats.directory_entries > 0);
        assert!(stats.artist_entries > 0);
        assert!(stats.album_entries > 0);
        assert!(stats.genre_entries > 0);
        assert!(stats.year_entries > 0);
        assert!(stats.total_operations() >= 20); // At least 10 inserts + 10 lookups
        assert!(stats.memory_utilization() > 0.0);
        assert!(stats.overall_cache_hit_rate() >= 0.0);
        
        println!("Index Stats: {:#?}", stats);
    }

    #[test]
    fn test_cache_memory_pressure() {
        // Create cache with very small memory limit
        let mut cache = MemoryBoundedCache::<String, Vec<u8>>::new(1000, 128); // 128 bytes limit
        
        // Insert data that exceeds memory limit
        let large_data = vec![0u8; 64]; // 64 bytes per entry
        
        cache.insert("key1".to_string(), large_data.clone());
        cache.insert("key2".to_string(), large_data.clone());
        cache.insert("key3".to_string(), large_data.clone()); // Should trigger eviction
        
        let stats = cache.get_stats();
        
        // Should have evicted some entries due to memory pressure
        assert!(stats.eviction_count > 0);
        assert!(stats.memory_bytes <= 128);
        
        println!("Cache stats after memory pressure: {:#?}", stats);
    }

    #[test]
    fn test_atomic_operations_thread_safety() {
        use std::sync::Arc;
        use std::thread;
        
        let index_manager: Arc<std::sync::Mutex<IndexManager>> = Arc::new(std::sync::Mutex::new(IndexManager::new(10000, 1024 * 1024)));
        let mut handles = vec![];
        
        // Spawn multiple threads to test atomic operations
        for thread_id in 0..4 {
            let index_manager_clone: Arc<std::sync::Mutex<IndexManager>> = Arc::clone(&index_manager);
            let handle = thread::spawn(move || {
                for i in 0..100 {
                    let file = create_test_media_file(
                        (thread_id * 100 + i) as i64,
                        &format!("/music/thread{}/song{}.mp3", thread_id, i),
                        Some(&format!("Artist {}", thread_id)),
                        None,
                        None,
                        None,
                    );
                    
                    let mut manager = index_manager_clone.lock().unwrap();
                    manager.insert_file_index(&file, (thread_id * 100 + i) as u64 * 1000);
                }
            });
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Verify all operations completed successfully
        let manager = index_manager.lock().unwrap();
        let stats = manager.get_stats();
        
        assert_eq!(stats.insert_count, 400); // 4 threads * 100 inserts each
        assert!(stats.artist_entries > 0);
        
        println!("Multi-threaded stats: {:#?}", stats);
    }
}
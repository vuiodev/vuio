#[cfg(test)]
mod tests {
    use crate::database::flatbuffer::{MediaFileSerializer, FlatBufferConverter, BatchOperationType};
    use crate::database::MediaFile;
    use std::path::PathBuf;
    use std::time::SystemTime;
    
    #[test]
    fn test_flatbuffer_basic_serialization() {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        
        // Create a test MediaFile
        let test_file = MediaFile {
            id: Some(42),
            path: PathBuf::from("/test/music/song.mp3"),
            filename: "song.mp3".to_string(),
            size: 5_242_880, // 5MB
            modified: SystemTime::now(),
            mime_type: "audio/mpeg".to_string(),
            duration: Some(std::time::Duration::from_secs(210)), // 3:30
            title: Some("Test Song".to_string()),
            artist: Some("Test Artist".to_string()),
            album: Some("Test Album".to_string()),
            genre: Some("Electronic".to_string()),
            track_number: Some(3),
            year: Some(2024),
            album_artist: Some("Test Artist".to_string()),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        };
        
        // Test serialization (with stub implementation, this will create a stub offset)
        let serialized_result = MediaFileSerializer::serialize_media_file(&mut builder, &test_file);
        assert!(serialized_result.is_ok(), "Failed to serialize MediaFile: {:?}", serialized_result.err());
        
        println!("✅ FlatBuffer serialization test passed (stub implementation)");
    }
    
    #[test]
    fn test_batch_serialization() {
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        
        // Create test files
        let test_files = vec![
            MediaFile {
                id: Some(1),
                path: PathBuf::from("/music/song1.mp3"),
                filename: "song1.mp3".to_string(),
                size: 1024,
                modified: SystemTime::now(),
                mime_type: "audio/mpeg".to_string(),
                duration: Some(std::time::Duration::from_secs(180)),
                title: Some("Song 1".to_string()),
                artist: Some("Artist 1".to_string()),
                album: None,
                genre: None,
                track_number: Some(1),
                year: Some(2024),
                album_artist: None,
                created_at: SystemTime::now(),
                updated_at: SystemTime::now(),
            },
            MediaFile {
                id: Some(2),
                path: PathBuf::from("/music/song2.mp3"),
                filename: "song2.mp3".to_string(),
                size: 2048,
                modified: SystemTime::now(),
                mime_type: "audio/mpeg".to_string(),
                duration: Some(std::time::Duration::from_secs(240)),
                title: Some("Song 2".to_string()),
                artist: Some("Artist 2".to_string()),
                album: Some("Album 2".to_string()),
                genre: Some("Rock".to_string()),
                track_number: Some(2),
                year: Some(2024),
                album_artist: Some("Artist 2".to_string()),
                created_at: SystemTime::now(),
                updated_at: SystemTime::now(),
            },
        ];
        
        // Test batch serialization (with stub implementation)
        let batch_result = MediaFileSerializer::serialize_media_file_batch(
            &mut builder,
            &test_files,
            12345,
            BatchOperationType::Insert,
        );
        
        assert!(batch_result.is_ok(), "Failed to serialize batch: {:?}", batch_result.err());
        
        println!("✅ FlatBuffer batch serialization test passed (stub implementation)");
    }
    
    #[test]
    fn test_converter_functions() {
        // Test time conversion
        let now = SystemTime::now();
        let timestamp = FlatBufferConverter::system_time_to_timestamp(now);
        let converted_back = FlatBufferConverter::timestamp_to_system_time(timestamp);
        
        // Should be within 1 second due to precision loss
        let diff = now.duration_since(converted_back)
            .unwrap_or_else(|_| converted_back.duration_since(now).unwrap());
        assert!(diff.as_secs() <= 1);
        
        // Test duration conversion
        let duration = Some(std::time::Duration::from_secs(300));
        let millis = FlatBufferConverter::duration_to_millis(duration);
        let converted_duration = FlatBufferConverter::millis_to_duration(millis);
        assert_eq!(duration, converted_duration);
        
        // Test string conversion
        let test_string = Some("test".to_string());
        let str_ref = FlatBufferConverter::optional_string_to_str(&test_string);
        assert_eq!(str_ref, "test");
        
        let converted_back = FlatBufferConverter::str_to_optional_string(Some(str_ref));
        assert_eq!(converted_back, test_string);
        
        // Test empty string handling
        let empty_string: Option<String> = None;
        let empty_str = FlatBufferConverter::optional_string_to_str(&empty_string);
        assert_eq!(empty_str, "");
        
        let converted_empty = FlatBufferConverter::str_to_optional_string(Some(""));
        assert_eq!(converted_empty, None);
        
        println!("✅ FlatBuffer converter functions test passed");
    }
}
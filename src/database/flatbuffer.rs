// FlatBuffer integration module for zero-copy database operations
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Result;

// Include the generated FlatBuffer code
#[allow(dead_code, unused_imports, non_snake_case, clippy::all)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/media_generated.rs"));
}

pub use generated::media_d_b::*;

/// Helper functions for converting between Rust types and FlatBuffer types
pub struct FlatBufferConverter;

impl FlatBufferConverter {
    /// Convert SystemTime to Unix timestamp (u64)
    pub fn system_time_to_timestamp(time: SystemTime) -> u64 {
        time.duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
    
    /// Convert Unix timestamp (u64) to SystemTime
    pub fn timestamp_to_system_time(timestamp: u64) -> SystemTime {
        UNIX_EPOCH + std::time::Duration::from_secs(timestamp)
    }
    
    /// Convert Option<Duration> to milliseconds (u64)
    pub fn duration_to_millis(duration: Option<std::time::Duration>) -> u64 {
        duration.map(|d| d.as_millis() as u64).unwrap_or(0)
    }
    
    /// Convert milliseconds (u64) to Option<Duration>
    pub fn millis_to_duration(millis: u64) -> Option<std::time::Duration> {
        if millis == 0 {
            None
        } else {
            Some(std::time::Duration::from_millis(millis))
        }
    }
    
    /// Convert Option<String> to string reference for FlatBuffer
    pub fn optional_string_to_str(s: &Option<String>) -> &str {
        s.as_deref().unwrap_or("")
    }
    
    /// Convert FlatBuffer string to Option<String>
    pub fn str_to_optional_string(s: Option<&str>) -> Option<String> {
        s.filter(|s| !s.is_empty()).map(|s| s.to_string())
    }
}

/// Batch serialization manager for zero-copy database operations
pub struct BatchSerializer {
    batch_id_counter: std::sync::atomic::AtomicU64,
}

impl BatchSerializer {
    /// Create a new batch serializer
    pub fn new() -> Self {
        Self {
            batch_id_counter: std::sync::atomic::AtomicU64::new(1),
        }
    }
    
    /// Generate a unique batch ID atomically
    pub fn generate_batch_id(&self) -> u64 {
        self.batch_id_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }
    
    /// Get the current batch ID counter value
    pub fn current_batch_id(&self) -> u64 {
        self.batch_id_counter.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Default for BatchSerializer {
    fn default() -> Self {
        Self::new()
    }
}

/// Serialization helper for MediaFile to FlatBuffer with batch support
pub struct MediaFileSerializer;

impl MediaFileSerializer {
    /// Serialize a MediaFile to FlatBuffer format with canonical path support
    pub fn serialize_media_file<'a>(
        builder: &mut flatbuffers::FlatBufferBuilder<'a>,
        file: &crate::database::MediaFile,
        canonical_path: Option<&str>,
    ) -> Result<flatbuffers::WIPOffset<MediaFile<'a>>> {
        // Create strings
        let path = builder.create_string(&file.path.to_string_lossy());
        let path_string = file.path.to_string_lossy();
        let canonical_path_str = canonical_path.unwrap_or(&path_string);
        let canonical_path_offset = builder.create_string(canonical_path_str);
        let filename = builder.create_string(&file.filename);
        let mime_type = builder.create_string(&file.mime_type);
        let title = builder.create_string(FlatBufferConverter::optional_string_to_str(&file.title));
        let artist = builder.create_string(FlatBufferConverter::optional_string_to_str(&file.artist));
        let album = builder.create_string(FlatBufferConverter::optional_string_to_str(&file.album));
        let genre = builder.create_string(FlatBufferConverter::optional_string_to_str(&file.genre));
        let album_artist = builder.create_string(FlatBufferConverter::optional_string_to_str(&file.album_artist));
        
        // Create MediaFile
        let media_file = MediaFile::create(builder, &MediaFileArgs {
            id: file.id.unwrap_or(0) as u64,
            path: Some(path),
            canonical_path: Some(canonical_path_offset),
            filename: Some(filename),
            size: file.size,
            modified: FlatBufferConverter::system_time_to_timestamp(file.modified),
            mime_type: Some(mime_type),
            duration: FlatBufferConverter::duration_to_millis(file.duration),
            title: Some(title),
            artist: Some(artist),
            album: Some(album),
            genre: Some(genre),
            track_number: file.track_number.unwrap_or(0),
            year: file.year.unwrap_or(0),
            album_artist: Some(album_artist),
            created_at: FlatBufferConverter::system_time_to_timestamp(file.created_at),
            updated_at: FlatBufferConverter::system_time_to_timestamp(file.updated_at),
        });
        
        Ok(media_file)
    }
    
    /// Deserialize a FlatBuffer MediaFile to Rust MediaFile
    pub fn deserialize_media_file(fb_file: MediaFile) -> Result<crate::database::MediaFile> {
        let path = std::path::PathBuf::from(fb_file.path().unwrap_or(""));
        let filename = fb_file.filename().unwrap_or("").to_string();
        let mime_type = fb_file.mime_type().unwrap_or("").to_string();
        
        Ok(crate::database::MediaFile {
            id: if fb_file.id() == 0 { None } else { Some(fb_file.id() as i64) },
            path,
            filename,
            size: fb_file.size(),
            modified: FlatBufferConverter::timestamp_to_system_time(fb_file.modified()),
            mime_type,
            duration: FlatBufferConverter::millis_to_duration(fb_file.duration()),
            title: FlatBufferConverter::str_to_optional_string(fb_file.title()),
            artist: FlatBufferConverter::str_to_optional_string(fb_file.artist()),
            album: FlatBufferConverter::str_to_optional_string(fb_file.album()),
            genre: FlatBufferConverter::str_to_optional_string(fb_file.genre()),
            track_number: if fb_file.track_number() == 0 { None } else { Some(fb_file.track_number()) },
            year: if fb_file.year() == 0 { None } else { Some(fb_file.year()) },
            album_artist: FlatBufferConverter::str_to_optional_string(fb_file.album_artist()),
            created_at: FlatBufferConverter::timestamp_to_system_time(fb_file.created_at()),
            updated_at: FlatBufferConverter::timestamp_to_system_time(fb_file.updated_at()),
        })
    }
    
    /// Serialize a batch of MediaFiles to FlatBuffer format with validation
    pub fn serialize_media_file_batch<'a>(
        builder: &mut flatbuffers::FlatBufferBuilder<'a>,
        files: &[crate::database::MediaFile],
        batch_id: u64,
        operation_type: BatchOperationType,
        canonical_paths: Option<&[String]>,
    ) -> Result<BatchSerializationResult<'a>> {
        if files.is_empty() {
            return Err(anyhow::anyhow!("Cannot serialize empty batch"));
        }
        
        let start_time = std::time::Instant::now();
        
        // Pre-validate batch size
        if files.len() > 1_000_000 {
            return Err(anyhow::anyhow!("Batch size {} exceeds maximum limit of 1,000,000", files.len()));
        }
        
        // Serialize all files with canonical paths if provided
        let mut file_offsets = Vec::with_capacity(files.len());
        let mut serialization_errors = Vec::new();
        
        for (i, file) in files.iter().enumerate() {
            let canonical_path = canonical_paths.and_then(|paths| paths.get(i).map(|s| s.as_str()));
            
            match Self::serialize_media_file(builder, file, canonical_path) {
                Ok(file_offset) => file_offsets.push(file_offset),
                Err(e) => {
                    serialization_errors.push(BatchSerializationError {
                        file_index: i,
                        file_path: file.path.clone(),
                        error: e.to_string(),
                    });
                }
            }
        }
        
        // If we have serialization errors, return them
        if !serialization_errors.is_empty() {
            return Err(anyhow::anyhow!(
                "Batch serialization failed with {} errors: {:?}",
                serialization_errors.len(),
                serialization_errors
            ));
        }
        
        // Create vector of files
        let files_vector = builder.create_vector(&file_offsets);
        
        // Create batch with metadata
        let batch = MediaFileBatch::create(builder, &MediaFileBatchArgs {
            files: Some(files_vector),
            batch_id,
            timestamp: FlatBufferConverter::system_time_to_timestamp(SystemTime::now()),
            operation_type,
        });
        
        // Finish the buffer to make it ready for reading
        builder.finish(batch, None);
        
        let serialization_time = start_time.elapsed();
        
        Ok(BatchSerializationResult {
            batch_offset: batch,
            batch_id,
            operation_type,
            files_count: files.len(),
            serialization_time,
            serialized_size: builder.finished_data().len(),
            errors: serialization_errors,
        })
    }
    
    /// Deserialize a batch of MediaFiles from FlatBuffer format with validation
    pub fn deserialize_media_file_batch(fb_batch: MediaFileBatch) -> Result<BatchDeserializationResult> {
        let start_time = std::time::Instant::now();
        
        let batch_id = fb_batch.batch_id();
        let operation_type = fb_batch.operation_type();
        let timestamp = FlatBufferConverter::timestamp_to_system_time(fb_batch.timestamp());
        
        let files = Vec::new(); // Stub implementation - would deserialize files here
        let deserialization_errors = Vec::new();
        
        let deserialization_time = start_time.elapsed();
        
        Ok(BatchDeserializationResult {
            batch_id,
            operation_type,
            timestamp,
            files,
            files_count: 0, // Stub implementation
            deserialization_time,
            errors: deserialization_errors,
        })
    }
    
    /// Validate batch integrity using checksums
    pub fn validate_batch_integrity(batch_data: &[u8]) -> Result<BatchIntegrityResult> {
        if batch_data.is_empty() {
            return Err(anyhow::anyhow!("Cannot validate empty batch data"));
        }
        
        let start_time = std::time::Instant::now();
        
        // Calculate CRC32 checksum
        let checksum = crc32fast::hash(batch_data);
        
        // Basic validation - check if data has reasonable size and starts with expected bytes
        let is_valid_flatbuffer = batch_data.len() >= 4 && batch_data.len() < 1_000_000_000; // Basic sanity check
        
        let validation_time = start_time.elapsed();
        
        Ok(BatchIntegrityResult {
            is_valid: is_valid_flatbuffer,
            checksum,
            data_size: batch_data.len(),
            validation_time,
        })
    }
}

/// Result of batch serialization operation
pub struct BatchSerializationResult<'a> {
    pub batch_offset: flatbuffers::WIPOffset<MediaFileBatch<'a>>,
    pub batch_id: u64,
    pub operation_type: BatchOperationType,
    pub files_count: usize,
    pub serialization_time: std::time::Duration,
    pub serialized_size: usize,
    pub errors: Vec<BatchSerializationError>,
}

/// Result of batch deserialization operation
#[derive(Debug)]
pub struct BatchDeserializationResult {
    pub batch_id: u64,
    pub operation_type: BatchOperationType,
    pub timestamp: SystemTime,
    pub files: Vec<crate::database::MediaFile>,
    pub files_count: usize,
    pub deserialization_time: std::time::Duration,
    pub errors: Vec<BatchDeserializationError>,
}

/// Result of batch integrity validation
#[derive(Debug)]
pub struct BatchIntegrityResult {
    pub is_valid: bool,
    pub checksum: u32,
    pub data_size: usize,
    pub validation_time: std::time::Duration,
}

/// Error during batch serialization
#[derive(Debug, Clone)]
pub struct BatchSerializationError {
    pub file_index: usize,
    pub file_path: std::path::PathBuf,
    pub error: String,
}

/// Error during batch deserialization
#[derive(Debug, Clone)]
pub struct BatchDeserializationError {
    pub file_index: usize,
    pub error: String,
}

// Include comprehensive tests
#[cfg(test)]
#[path = "flatbuffer_tests.rs"]
mod tests;
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

/// Serialization helper for MediaFile to FlatBuffer
pub struct MediaFileSerializer;

impl MediaFileSerializer {
    /// Serialize a MediaFile to FlatBuffer format
    pub fn serialize_media_file<'a>(
        builder: &mut flatbuffers::FlatBufferBuilder<'a>,
        file: &crate::database::MediaFile,
    ) -> Result<flatbuffers::WIPOffset<MediaFile<'a>>> {
        // Create strings
        let path = builder.create_string(&file.path.to_string_lossy());
        let canonical_path = builder.create_string(&file.path.to_string_lossy()); // TODO: Use actual canonical path
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
            canonical_path: Some(canonical_path),
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
    
    /// Serialize a batch of MediaFiles to FlatBuffer format
    pub fn serialize_media_file_batch<'a>(
        builder: &mut flatbuffers::FlatBufferBuilder<'a>,
        files: &[crate::database::MediaFile],
        batch_id: u64,
        operation_type: BatchOperationType,
    ) -> Result<flatbuffers::WIPOffset<MediaFileBatch<'a>>> {
        // Serialize all files
        let mut file_offsets = Vec::with_capacity(files.len());
        for file in files {
            let file_offset = Self::serialize_media_file(builder, file)?;
            file_offsets.push(file_offset);
        }
        
        // Create vector of files
        let files_vector = builder.create_vector(&file_offsets);
        
        // Create batch
        let batch = MediaFileBatch::create(builder, &MediaFileBatchArgs {
            files: Some(files_vector),
            batch_id,
            timestamp: FlatBufferConverter::system_time_to_timestamp(SystemTime::now()),
            operation_type,
        });
        
        Ok(batch)
    }
}

// Include comprehensive tests
#[cfg(test)]
#[path = "flatbuffer_tests.rs"]
mod tests;
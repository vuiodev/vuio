// Ultra-fast custom binary serialization for MediaFile
// Optimized for local file indexing with zero-copy reads

use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};
use crate::database::MediaFile;

/// Magic bytes to identify our binary format
const MAGIC: &[u8; 4] = b"VUIO";
const VERSION: u8 = 1;

/// Fixed-size header for each MediaFile entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct MediaFileHeader {
    magic: [u8; 4],           // "VUIO"
    version: u8,              // Format version
    flags: u8,                // Bit flags for optional fields
    id: u64,                  // File ID
    size: u64,                // File size in bytes
    modified: u64,            // Modified timestamp (Unix epoch)
    duration_ms: u64,         // Duration in milliseconds
    track_number: u32,        // Track number (0 = None)
    year: u32,                // Year (0 = None)
    
    // String lengths (for variable data)
    path_len: u16,
    filename_len: u16,
    mime_type_len: u16,
    title_len: u16,
    artist_len: u16,
    album_len: u16,
    genre_len: u16,
    album_artist_len: u16,
    
    created_at: u64,          // Created timestamp
    updated_at: u64,          // Updated timestamp
}

/// Bit flags for optional fields
mod flags {
    pub const HAS_TITLE: u8 = 1 << 0;
    pub const HAS_ARTIST: u8 = 1 << 1;
    pub const HAS_ALBUM: u8 = 1 << 2;
    pub const HAS_GENRE: u8 = 1 << 3;
    pub const HAS_ALBUM_ARTIST: u8 = 1 << 4;
    pub const HAS_TRACK_NUMBER: u8 = 1 << 5;
    pub const HAS_YEAR: u8 = 1 << 6;
}

impl MediaFileHeader {
    const SIZE: usize = std::mem::size_of::<Self>();
    
    fn new() -> Self {
        Self {
            magic: *MAGIC,
            version: VERSION,
            flags: 0,
            id: 0,
            size: 0,
            modified: 0,
            duration_ms: 0,
            track_number: 0,
            year: 0,
            path_len: 0,
            filename_len: 0,
            mime_type_len: 0,
            title_len: 0,
            artist_len: 0,
            album_len: 0,
            genre_len: 0,
            album_artist_len: 0,
            created_at: 0,
            updated_at: 0,
        }
    }
    
    fn validate(&self) -> Result<()> {
        if &self.magic != MAGIC {
            return Err(anyhow!("Invalid magic bytes"));
        }
        if self.version != VERSION {
            return Err(anyhow!("Unsupported version: {}", self.version));
        }
        Ok(())
    }
}

/// Ultra-fast binary serializer for MediaFile
pub struct BinaryMediaFileSerializer;

impl BinaryMediaFileSerializer {
    /// Serialize a single MediaFile to binary format
    /// Returns the serialized bytes
    pub fn serialize(file: &MediaFile) -> Result<Vec<u8>> {
        let mut header = MediaFileHeader::new();
        
        // Set basic fields
        header.id = file.id.unwrap_or(0) as u64;
        header.size = file.size;
        header.modified = file.modified.duration_since(UNIX_EPOCH)?.as_secs();
        header.duration_ms = file.duration.map(|d| d.as_millis() as u64).unwrap_or(0);
        header.created_at = file.created_at.duration_since(UNIX_EPOCH)?.as_secs();
        header.updated_at = file.updated_at.duration_since(UNIX_EPOCH)?.as_secs();
        
        // Convert path to string
        let path_str = file.path.to_string_lossy();
        let path_bytes = path_str.as_bytes();
        header.path_len = path_bytes.len().min(u16::MAX as usize) as u16;
        
        // Set string lengths and flags
        header.filename_len = file.filename.len().min(u16::MAX as usize) as u16;
        header.mime_type_len = file.mime_type.len().min(u16::MAX as usize) as u16;
        
        // Optional fields with flags
        if let Some(ref title) = file.title {
            header.flags |= flags::HAS_TITLE;
            header.title_len = title.len().min(u16::MAX as usize) as u16;
        }
        
        if let Some(ref artist) = file.artist {
            header.flags |= flags::HAS_ARTIST;
            header.artist_len = artist.len().min(u16::MAX as usize) as u16;
        }
        
        if let Some(ref album) = file.album {
            header.flags |= flags::HAS_ALBUM;
            header.album_len = album.len().min(u16::MAX as usize) as u16;
        }
        
        if let Some(ref genre) = file.genre {
            header.flags |= flags::HAS_GENRE;
            header.genre_len = genre.len().min(u16::MAX as usize) as u16;
        }
        
        if let Some(ref album_artist) = file.album_artist {
            header.flags |= flags::HAS_ALBUM_ARTIST;
            header.album_artist_len = album_artist.len().min(u16::MAX as usize) as u16;
        }
        
        if let Some(track_number) = file.track_number {
            header.flags |= flags::HAS_TRACK_NUMBER;
            header.track_number = track_number;
        }
        
        if let Some(year) = file.year {
            header.flags |= flags::HAS_YEAR;
            header.year = year;
        }
        
        // Calculate total size
        let total_size = MediaFileHeader::SIZE 
            + header.path_len as usize
            + header.filename_len as usize
            + header.mime_type_len as usize
            + header.title_len as usize
            + header.artist_len as usize
            + header.album_len as usize
            + header.genre_len as usize
            + header.album_artist_len as usize;
        
        // Allocate buffer
        let mut buffer = Vec::with_capacity(total_size);
        
        // Write header (unsafe but fast)
        unsafe {
            let header_bytes = std::slice::from_raw_parts(
                &header as *const _ as *const u8,
                MediaFileHeader::SIZE
            );
            buffer.extend_from_slice(header_bytes);
        }
        
        // Write variable data in order
        buffer.extend_from_slice(path_bytes);
        buffer.extend_from_slice(file.filename.as_bytes());
        buffer.extend_from_slice(file.mime_type.as_bytes());
        
        // Write optional strings
        if header.flags & flags::HAS_TITLE != 0 {
            buffer.extend_from_slice(file.title.as_ref().unwrap().as_bytes());
        }
        if header.flags & flags::HAS_ARTIST != 0 {
            buffer.extend_from_slice(file.artist.as_ref().unwrap().as_bytes());
        }
        if header.flags & flags::HAS_ALBUM != 0 {
            buffer.extend_from_slice(file.album.as_ref().unwrap().as_bytes());
        }
        if header.flags & flags::HAS_GENRE != 0 {
            buffer.extend_from_slice(file.genre.as_ref().unwrap().as_bytes());
        }
        if header.flags & flags::HAS_ALBUM_ARTIST != 0 {
            buffer.extend_from_slice(file.album_artist.as_ref().unwrap().as_bytes());
        }
        
        Ok(buffer)
    }
    
    /// Deserialize a MediaFile from binary format
    /// Zero-copy where possible
    pub fn deserialize(data: &[u8]) -> Result<MediaFile> {
        if data.len() < MediaFileHeader::SIZE {
            return Err(anyhow!("Data too short for header"));
        }
        
        // Read header (unsafe but fast)
        let header = unsafe {
            std::ptr::read_unaligned(data.as_ptr() as *const MediaFileHeader)
        };
        
        header.validate()?;
        
        // Calculate expected size
        let expected_size = MediaFileHeader::SIZE
            + header.path_len as usize
            + header.filename_len as usize
            + header.mime_type_len as usize
            + header.title_len as usize
            + header.artist_len as usize
            + header.album_len as usize
            + header.genre_len as usize
            + header.album_artist_len as usize;
        
        if data.len() != expected_size {
            return Err(anyhow!("Data size mismatch: expected {}, got {}", expected_size, data.len()));
        }
        
        // Read variable data
        let mut offset = MediaFileHeader::SIZE;
        
        // Path
        let path_bytes = &data[offset..offset + header.path_len as usize];
        let path = PathBuf::from(std::str::from_utf8(path_bytes)?);
        offset += header.path_len as usize;
        
        // Filename
        let filename_bytes = &data[offset..offset + header.filename_len as usize];
        let filename = std::str::from_utf8(filename_bytes)?.to_string();
        offset += header.filename_len as usize;
        
        // MIME type
        let mime_type_bytes = &data[offset..offset + header.mime_type_len as usize];
        let mime_type = std::str::from_utf8(mime_type_bytes)?.to_string();
        offset += header.mime_type_len as usize;
        
        // Optional fields
        let title = if header.flags & flags::HAS_TITLE != 0 {
            let title_bytes = &data[offset..offset + header.title_len as usize];
            offset += header.title_len as usize;
            Some(std::str::from_utf8(title_bytes)?.to_string())
        } else {
            None
        };
        
        let artist = if header.flags & flags::HAS_ARTIST != 0 {
            let artist_bytes = &data[offset..offset + header.artist_len as usize];
            offset += header.artist_len as usize;
            Some(std::str::from_utf8(artist_bytes)?.to_string())
        } else {
            None
        };
        
        let album = if header.flags & flags::HAS_ALBUM != 0 {
            let album_bytes = &data[offset..offset + header.album_len as usize];
            offset += header.album_len as usize;
            Some(std::str::from_utf8(album_bytes)?.to_string())
        } else {
            None
        };
        
        let genre = if header.flags & flags::HAS_GENRE != 0 {
            let genre_bytes = &data[offset..offset + header.genre_len as usize];
            offset += header.genre_len as usize;
            Some(std::str::from_utf8(genre_bytes)?.to_string())
        } else {
            None
        };
        
        let album_artist = if header.flags & flags::HAS_ALBUM_ARTIST != 0 {
            let album_artist_bytes = &data[offset..offset + header.album_artist_len as usize];
            Some(std::str::from_utf8(album_artist_bytes)?.to_string())
        } else {
            None
        };
        
        // Convert timestamps back
        let modified = UNIX_EPOCH + Duration::from_secs(header.modified);
        let created_at = UNIX_EPOCH + Duration::from_secs(header.created_at);
        let updated_at = UNIX_EPOCH + Duration::from_secs(header.updated_at);
        
        let duration = if header.duration_ms > 0 {
            Some(Duration::from_millis(header.duration_ms))
        } else {
            None
        };
        
        let track_number = if header.flags & flags::HAS_TRACK_NUMBER != 0 {
            Some(header.track_number)
        } else {
            None
        };
        
        let year = if header.flags & flags::HAS_YEAR != 0 {
            Some(header.year)
        } else {
            None
        };
        
        Ok(MediaFile {
            id: if header.id == 0 { None } else { Some(header.id as i64) },
            path,
            filename,
            size: header.size,
            modified,
            mime_type,
            duration,
            title,
            artist,
            album,
            genre,
            track_number,
            year,
            album_artist,
            created_at,
            updated_at,
        })
    }
    
    /// Serialize multiple MediaFiles into a batch format
    /// Much more efficient than individual serialization
    pub fn serialize_batch(files: &[MediaFile]) -> Result<Vec<u8>> {
        if files.is_empty() {
            return Ok(Vec::new());
        }
        
        // Pre-calculate total size to avoid reallocations
        let mut total_size = 8; // 8 bytes for count
        for file in files {
            let path_len = file.path.to_string_lossy().len();
            let title_len = file.title.as_ref().map(|s| s.len()).unwrap_or(0);
            let artist_len = file.artist.as_ref().map(|s| s.len()).unwrap_or(0);
            let album_len = file.album.as_ref().map(|s| s.len()).unwrap_or(0);
            let genre_len = file.genre.as_ref().map(|s| s.len()).unwrap_or(0);
            let album_artist_len = file.album_artist.as_ref().map(|s| s.len()).unwrap_or(0);
            
            total_size += MediaFileHeader::SIZE 
                + path_len
                + file.filename.len()
                + file.mime_type.len()
                + title_len
                + artist_len
                + album_len
                + genre_len
                + album_artist_len;
        }
        
        let mut buffer = Vec::with_capacity(total_size);
        
        // Write count
        buffer.extend_from_slice(&(files.len() as u64).to_le_bytes());
        
        // Serialize each file
        for file in files {
            let file_data = Self::serialize(file)?;
            buffer.extend_from_slice(&file_data);
        }
        
        Ok(buffer)
    }
    
    /// Deserialize multiple MediaFiles from batch format
    pub fn deserialize_batch(data: &[u8]) -> Result<Vec<MediaFile>> {
        if data.len() < 8 {
            return Err(anyhow!("Data too short for batch header"));
        }
        
        // Read count
        let count = u64::from_le_bytes([
            data[0], data[1], data[2], data[3],
            data[4], data[5], data[6], data[7],
        ]) as usize;
        
        let mut files = Vec::with_capacity(count);
        let mut offset = 8;
        
        for _ in 0..count {
            if offset >= data.len() {
                return Err(anyhow!("Unexpected end of data"));
            }
            
            // Read header to get size
            if offset + MediaFileHeader::SIZE > data.len() {
                return Err(anyhow!("Not enough data for header"));
            }
            
            let header = unsafe {
                std::ptr::read_unaligned((data.as_ptr().add(offset)) as *const MediaFileHeader)
            };
            
            let file_size = MediaFileHeader::SIZE
                + header.path_len as usize
                + header.filename_len as usize
                + header.mime_type_len as usize
                + header.title_len as usize
                + header.artist_len as usize
                + header.album_len as usize
                + header.genre_len as usize
                + header.album_artist_len as usize;
            
            if offset + file_size > data.len() {
                return Err(anyhow!("Not enough data for file"));
            }
            
            let file_data = &data[offset..offset + file_size];
            let file = Self::deserialize(file_data)?;
            files.push(file);
            
            offset += file_size;
        }
        
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    
    fn create_test_file() -> MediaFile {
        MediaFile {
            id: Some(123),
            path: PathBuf::from("/test/path/song.mp3"),
            filename: "song.mp3".to_string(),
            size: 1024000,
            modified: SystemTime::now(),
            mime_type: "audio/mpeg".to_string(),
            duration: Some(Duration::from_secs(180)),
            title: Some("Test Song".to_string()),
            artist: Some("Test Artist".to_string()),
            album: Some("Test Album".to_string()),
            genre: Some("Rock".to_string()),
            track_number: Some(5),
            year: Some(2023),
            album_artist: Some("Test Artist".to_string()),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        }
    }
    
    #[test]
    fn test_serialize_deserialize() {
        let original = create_test_file();
        
        let serialized = BinaryMediaFileSerializer::serialize(&original).unwrap();
        let deserialized = BinaryMediaFileSerializer::deserialize(&serialized).unwrap();
        
        assert_eq!(original.id, deserialized.id);
        assert_eq!(original.path, deserialized.path);
        assert_eq!(original.filename, deserialized.filename);
        assert_eq!(original.size, deserialized.size);
        assert_eq!(original.mime_type, deserialized.mime_type);
        assert_eq!(original.title, deserialized.title);
        assert_eq!(original.artist, deserialized.artist);
        assert_eq!(original.album, deserialized.album);
        assert_eq!(original.genre, deserialized.genre);
        assert_eq!(original.track_number, deserialized.track_number);
        assert_eq!(original.year, deserialized.year);
        assert_eq!(original.album_artist, deserialized.album_artist);
    }
    
    #[test]
    fn test_batch_serialize_deserialize() {
        let files = vec![
            create_test_file(),
            create_test_file(),
            create_test_file(),
        ];
        
        let serialized = BinaryMediaFileSerializer::serialize_batch(&files).unwrap();
        let deserialized = BinaryMediaFileSerializer::deserialize_batch(&serialized).unwrap();
        
        assert_eq!(files.len(), deserialized.len());
        for (original, deserialized) in files.iter().zip(deserialized.iter()) {
            assert_eq!(original.filename, deserialized.filename);
            assert_eq!(original.title, deserialized.title);
        }
    }
    
    #[test]
    fn test_minimal_file() {
        let mut file = create_test_file();
        file.title = None;
        file.artist = None;
        file.album = None;
        file.genre = None;
        file.album_artist = None;
        file.track_number = None;
        file.year = None;
        
        let serialized = BinaryMediaFileSerializer::serialize(&file).unwrap();
        let deserialized = BinaryMediaFileSerializer::deserialize(&serialized).unwrap();
        
        assert_eq!(file.title, deserialized.title);
        assert_eq!(file.artist, deserialized.artist);
        assert_eq!(file.album, deserialized.album);
    }
}
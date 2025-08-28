use anyhow::{anyhow, Result};
use memmap2::{MmapMut, MmapOptions};
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Memory-mapped file manager with atomic operations for zero-copy database operations
pub struct MemoryMappedFile {
    file: File,
    mmap: MmapMut,
    current_size: AtomicUsize,
    max_size: usize,
    current_offset: AtomicU64,
    file_path: std::path::PathBuf,
}

impl MemoryMappedFile {
    /// Create a new memory-mapped file with the specified initial size
    pub fn new(path: &Path, initial_size: usize) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Create or open the file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        // Set initial file size
        file.set_len(initial_size as u64)?;

        // Create memory mapping
        let mmap = unsafe {
            MmapOptions::new()
                .len(initial_size)
                .map_mut(&file)?
        };

        info!(
            "Created memory-mapped file at {} with initial size {} bytes",
            path.display(),
            initial_size
        );

        Ok(Self {
            file,
            mmap,
            current_size: AtomicUsize::new(initial_size),
            max_size: initial_size * 1000, // Allow growth up to 1000x initial size
            current_offset: AtomicU64::new(0),
            file_path: path.to_path_buf(),
        })
    }

    /// Create a new memory-mapped file with custom maximum size
    pub fn with_max_size(path: &Path, initial_size: usize, max_size: usize) -> Result<Self> {
        let mut instance = Self::new(path, initial_size)?;
        instance.max_size = max_size;
        Ok(instance)
    }

    /// Append data to the memory-mapped file and return the offset where data was written
    pub fn append_data(&mut self, data: &[u8]) -> Result<u64> {
        let data_len = data.len();
        if data_len == 0 {
            return Ok(self.current_offset.load(Ordering::Relaxed));
        }

        // Get current offset atomically
        let offset = self.current_offset.fetch_add(data_len as u64, Ordering::SeqCst);
        let end_offset = offset + data_len as u64;

        // Check if we need to resize
        let current_size = self.current_size.load(Ordering::Relaxed);
        if end_offset as usize > current_size {
            self.resize_if_needed(data_len)?;
        }

        // Perform bounds checking
        let current_size_after_resize = self.current_size.load(Ordering::Relaxed);
        if end_offset as usize > current_size_after_resize {
            return Err(anyhow!(
                "Data write would exceed file bounds: offset {} + size {} > current_size {}",
                offset,
                data_len,
                current_size_after_resize
            ));
        }

        // Write data to memory-mapped region
        let start_idx = offset as usize;
        let end_idx = end_offset as usize;
        
        // Safety: We've verified bounds above
        self.mmap[start_idx..end_idx].copy_from_slice(data);

        debug!(
            "Appended {} bytes at offset {} to memory-mapped file",
            data_len, offset
        );

        Ok(offset)
    }

    /// Read data from the memory-mapped file at the specified offset
    pub fn read_at_offset(&self, offset: u64, length: usize) -> Result<&[u8]> {
        let start_idx = offset as usize;
        let end_idx = start_idx + length;
        let current_size = self.current_size.load(Ordering::Relaxed);

        // Bounds checking
        if end_idx > current_size {
            return Err(anyhow!(
                "Read would exceed file bounds: offset {} + length {} > current_size {}",
                offset,
                length,
                current_size
            ));
        }

        // Return slice of memory-mapped data (zero-copy)
        Ok(&self.mmap[start_idx..end_idx])
    }

    /// Resize the memory-mapped file if additional space is needed
    pub fn resize_if_needed(&mut self, additional_size: usize) -> Result<()> {
        let current_size = self.current_size.load(Ordering::Relaxed);
        let current_offset = self.current_offset.load(Ordering::Relaxed) as usize;
        let required_size = current_offset + additional_size;

        if required_size <= current_size {
            return Ok(()); // No resize needed
        }

        // Calculate new size (double current size or required size, whichever is larger)
        let new_size = std::cmp::max(current_size * 2, required_size);
        
        // Check against maximum size limit
        if new_size > self.max_size {
            return Err(anyhow!(
                "Cannot resize file beyond maximum size: {} > {}",
                new_size,
                self.max_size
            ));
        }

        info!(
            "Resizing memory-mapped file from {} to {} bytes",
            current_size, new_size
        );

        // Resize the underlying file
        self.file.set_len(new_size as u64)?;

        // Create new memory mapping
        let new_mmap = unsafe {
            MmapOptions::new()
                .len(new_size)
                .map_mut(&self.file)?
        };

        // Replace the old mapping
        self.mmap = new_mmap;
        self.current_size.store(new_size, Ordering::Relaxed);

        debug!("Memory-mapped file resized successfully to {} bytes", new_size);

        Ok(())
    }

    /// Force synchronization of memory-mapped data to disk
    pub fn sync_to_disk(&self) -> Result<()> {
        self.mmap.flush()?;
        self.file.sync_all()?;
        debug!("Synchronized memory-mapped file to disk");
        Ok(())
    }

    /// Get the current size of the memory-mapped file
    pub fn current_size(&self) -> usize {
        self.current_size.load(Ordering::Relaxed)
    }

    /// Get the current write offset
    pub fn current_offset(&self) -> u64 {
        self.current_offset.load(Ordering::Relaxed)
    }

    /// Get the maximum allowed size
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Get the file path
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// Reset the write offset (use with caution - can overwrite existing data)
    pub fn reset_offset(&self) {
        self.current_offset.store(0, Ordering::SeqCst);
        warn!("Memory-mapped file write offset has been reset to 0");
    }

    /// Get usage statistics
    pub fn get_stats(&self) -> MemoryMappedFileStats {
        let current_size = self.current_size.load(Ordering::Relaxed);
        let current_offset = self.current_offset.load(Ordering::Relaxed);
        
        MemoryMappedFileStats {
            file_path: self.file_path.clone(),
            current_size,
            max_size: self.max_size,
            current_offset,
            utilization_percent: if current_size > 0 {
                (current_offset as f64 / current_size as f64) * 100.0
            } else {
                0.0
            },
        }
    }
}

/// Statistics for memory-mapped file usage
#[derive(Debug, Clone)]
pub struct MemoryMappedFileStats {
    pub file_path: std::path::PathBuf,
    pub current_size: usize,
    pub max_size: usize,
    pub current_offset: u64,
    pub utilization_percent: f64,
}

/// Thread-safe wrapper for MemoryMappedFile with atomic operations
pub struct AtomicMemoryMappedFile {
    inner: Arc<std::sync::Mutex<MemoryMappedFile>>,
    append_counter: AtomicU64,
    read_counter: AtomicU64,
    sync_counter: AtomicU64,
}

impl AtomicMemoryMappedFile {
    /// Create a new atomic memory-mapped file
    pub fn new(path: &Path, initial_size: usize) -> Result<Self> {
        let mmap_file = MemoryMappedFile::new(path, initial_size)?;
        
        Ok(Self {
            inner: Arc::new(std::sync::Mutex::new(mmap_file)),
            append_counter: AtomicU64::new(0),
            read_counter: AtomicU64::new(0),
            sync_counter: AtomicU64::new(0),
        })
    }

    /// Thread-safe append operation
    pub fn append_data(&self, data: &[u8]) -> Result<u64> {
        let mut inner = self.inner.lock().map_err(|_| anyhow!("Failed to acquire lock"))?;
        let offset = inner.append_data(data)?;
        self.append_counter.fetch_add(1, Ordering::Relaxed);
        Ok(offset)
    }

    /// Thread-safe read operation
    pub fn read_at_offset(&self, offset: u64, length: usize) -> Result<Vec<u8>> {
        let inner = self.inner.lock().map_err(|_| anyhow!("Failed to acquire lock"))?;
        let data = inner.read_at_offset(offset, length)?;
        self.read_counter.fetch_add(1, Ordering::Relaxed);
        // Return owned data to avoid lifetime issues with the lock
        Ok(data.to_vec())
    }

    /// Thread-safe sync operation
    pub fn sync_to_disk(&self) -> Result<()> {
        let inner = self.inner.lock().map_err(|_| anyhow!("Failed to acquire lock"))?;
        inner.sync_to_disk()?;
        self.sync_counter.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Get atomic operation counters
    pub fn get_operation_stats(&self) -> AtomicOperationStats {
        AtomicOperationStats {
            append_operations: self.append_counter.load(Ordering::Relaxed),
            read_operations: self.read_counter.load(Ordering::Relaxed),
            sync_operations: self.sync_counter.load(Ordering::Relaxed),
        }
    }

    /// Get file statistics (thread-safe)
    pub fn get_stats(&self) -> Result<MemoryMappedFileStats> {
        let inner = self.inner.lock().map_err(|_| anyhow!("Failed to acquire lock"))?;
        Ok(inner.get_stats())
    }
}

/// Statistics for atomic operations
#[derive(Debug, Clone)]
pub struct AtomicOperationStats {
    pub append_operations: u64,
    pub read_operations: u64,
    pub sync_operations: u64,
}

// Implement Clone for AtomicMemoryMappedFile to allow sharing across threads
impl Clone for AtomicMemoryMappedFile {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            append_counter: AtomicU64::new(self.append_counter.load(Ordering::Relaxed)),
            read_counter: AtomicU64::new(self.read_counter.load(Ordering::Relaxed)),
            sync_counter: AtomicU64::new(self.sync_counter.load(Ordering::Relaxed)),
        }
    }
}

// Safety: MemoryMappedFile operations are thread-safe when properly synchronized
unsafe impl Send for AtomicMemoryMappedFile {}
unsafe impl Sync for AtomicMemoryMappedFile {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_memory_mapped_file_creation() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.db");
        
        let mmap_file = MemoryMappedFile::new(&file_path, 1024).unwrap();
        
        assert_eq!(mmap_file.current_size(), 1024);
        assert_eq!(mmap_file.current_offset(), 0);
        assert!(file_path.exists());
    }

    #[test]
    fn test_append_and_read_data() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.db");
        
        let mut mmap_file = MemoryMappedFile::new(&file_path, 1024).unwrap();
        
        let test_data = b"Hello, World!";
        let offset = mmap_file.append_data(test_data).unwrap();
        
        assert_eq!(offset, 0);
        assert_eq!(mmap_file.current_offset(), test_data.len() as u64);
        
        let read_data = mmap_file.read_at_offset(offset, test_data.len()).unwrap();
        assert_eq!(read_data, test_data);
    }

    #[test]
    fn test_multiple_appends() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.db");
        
        let mut mmap_file = MemoryMappedFile::new(&file_path, 1024).unwrap();
        
        let data1 = b"First";
        let data2 = b"Second";
        
        let offset1 = mmap_file.append_data(data1).unwrap();
        let offset2 = mmap_file.append_data(data2).unwrap();
        
        assert_eq!(offset1, 0);
        assert_eq!(offset2, data1.len() as u64);
        
        let read1 = mmap_file.read_at_offset(offset1, data1.len()).unwrap();
        let read2 = mmap_file.read_at_offset(offset2, data2.len()).unwrap();
        
        assert_eq!(read1, data1);
        assert_eq!(read2, data2);
    }

    #[test]
    fn test_file_resize() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.db");
        
        let mut mmap_file = MemoryMappedFile::new(&file_path, 64).unwrap();
        
        // Fill up the initial space
        let large_data = vec![0u8; 100]; // Larger than initial 64 bytes
        let offset = mmap_file.append_data(&large_data).unwrap();
        
        assert_eq!(offset, 0);
        assert!(mmap_file.current_size() >= 100);
        
        let read_data = mmap_file.read_at_offset(offset, large_data.len()).unwrap();
        assert_eq!(read_data, &large_data);
    }

    #[test]
    fn test_bounds_checking() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.db");
        
        let mmap_file = MemoryMappedFile::new(&file_path, 64).unwrap();
        
        // Try to read beyond bounds
        let result = mmap_file.read_at_offset(100, 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_to_disk() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.db");
        
        let mut mmap_file = MemoryMappedFile::new(&file_path, 1024).unwrap();
        
        let test_data = b"Sync test data";
        mmap_file.append_data(test_data).unwrap();
        
        // Should not panic
        mmap_file.sync_to_disk().unwrap();
    }

    #[test]
    fn test_atomic_memory_mapped_file() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.db");
        
        let atomic_mmap = AtomicMemoryMappedFile::new(&file_path, 1024).unwrap();
        
        let test_data = b"Atomic test";
        let offset = atomic_mmap.append_data(test_data).unwrap();
        
        let read_data = atomic_mmap.read_at_offset(offset, test_data.len()).unwrap();
        assert_eq!(read_data, test_data);
        
        let stats = atomic_mmap.get_operation_stats();
        assert_eq!(stats.append_operations, 1);
        assert_eq!(stats.read_operations, 1);
    }

    #[test]
    fn test_max_size_limit() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.db");
        
        let mut mmap_file = MemoryMappedFile::with_max_size(&file_path, 64, 256).unwrap();
        
        // This should work (within max size)
        let data1 = vec![0u8; 60];
        mmap_file.append_data(&data1).unwrap();
        
        // This should trigger resize but still work
        let data2 = vec![0u8; 60];
        mmap_file.append_data(&data2).unwrap();
        
        // This should fail (exceeds max size)
        let data3 = vec![0u8; 200];
        let result = mmap_file.append_data(&data3);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_stats() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.db");
        
        let mut mmap_file = MemoryMappedFile::new(&file_path, 1024).unwrap();
        
        let test_data = b"Stats test data";
        mmap_file.append_data(test_data).unwrap();
        
        let stats = mmap_file.get_stats();
        assert_eq!(stats.current_size, 1024);
        assert_eq!(stats.current_offset, test_data.len() as u64);
        assert!(stats.utilization_percent > 0.0);
        assert_eq!(stats.file_path, file_path);
    }
}
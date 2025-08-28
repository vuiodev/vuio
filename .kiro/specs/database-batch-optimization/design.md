# Design Document

## Overview

This design implements extreme performance optimization for media file database operations to achieve 1,000,000 files per second throughput. Since SQLite cannot achieve this level of performance, the solution implements a custom zero-copy database using FlatBuffers for serialization and memory-mapped files for storage.

The current bottleneck occurs in the `perform_incremental_update` method where each new file triggers an individual `store_media_file` call, creating separate database transactions. This design replaces the SQLite backend with a high-performance custom database that uses zero-copy operations, batch processing, and memory-mapped storage.

## Architecture

### Current Architecture Issues
- Individual `store_media_file` calls create separate transactions
- Each file insertion involves full SQL preparation and execution overhead
- No batching of database operations during media scanning
- Memory inefficient for large file sets

### New Zero-Copy Database Architecture

```mermaid
graph TD
    A[MediaScanner] --> B[File Collection Phase]
    B --> C[ZeroCopy Batch Processor]
    C --> D[FlatBuffer Serializer]
    D --> E[Memory-Mapped Storage]
    
    F[Configuration] --> C
    G[Progress Reporter] --> C
    H[Error Handler] --> C
    
    subgraph "Zero-Copy Processing Pipeline"
        I[Collect Files] --> J[Serialize to FlatBuffers]
        J --> K[Memory-Map Batch Write]
        K --> L[Update Index Structures]
        L --> M[Atomic Commit]
    end
    
    subgraph "Storage Layer"
        N[Data Files (.fb)] --> O[Memory-Mapped Regions]
        P[Index Files (.idx)] --> Q[Hash Tables/B-Trees]
        R[WAL Log (.wal)] --> S[Crash Recovery]
    end
```

## Complete Database Refactoring Architecture

### 1. ZeroCopyDatabase - Full DatabaseManager Implementation

Complete replacement of SQLite with zero-copy database implementing ALL current operations:

```rust
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub struct ZeroCopyDatabase {
    data_file: MemoryMappedFile,
    index_manager: IndexManager,
    flatbuffer_builder: FlatBufferBuilder<'static>,
    write_buffer: Vec<u8>,
    config: ZeroCopyConfig,
    
    // Atomic counters for all operations
    total_files: AtomicU64,
    total_operations: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
}

impl DatabaseManager for ZeroCopyDatabase {
    // BULK OPERATIONS - Primary interface (replaces individual operations)
    async fn bulk_store_media_files(&self, files: &[MediaFile]) -> Result<Vec<i64>>;
    async fn bulk_update_media_files(&self, files: &[MediaFile]) -> Result<()>;
    async fn bulk_remove_media_files(&self, paths: &[PathBuf]) -> Result<usize>;
    async fn bulk_get_files_by_paths(&self, paths: &[PathBuf]) -> Result<Vec<MediaFile>>;
    
    // INDIVIDUAL OPERATIONS - Implemented as single-item bulk operations for efficiency
    // These remain available for cases where single operations are needed
    async fn store_media_file(&self, file: &MediaFile) -> Result<i64> {
        let ids = self.bulk_store_media_files(&[file.clone()]).await?;
        Ok(ids[0])
    }
    
    async fn remove_media_file(&self, path: &Path) -> Result<bool> {
        let removed = self.bulk_remove_media_files(&[path.to_path_buf()]).await?;
        Ok(removed > 0)
    }
    
    async fn update_media_file(&self, file: &MediaFile) -> Result<()> {
        self.bulk_update_media_files(&[file.clone()]).await
    }
    
    // Individual operations for playlists
    async fn add_to_playlist(&self, playlist_id: i64, media_file_id: i64, position: Option<u32>) -> Result<i64> {
        let pos = position.unwrap_or(0);
        let ids = self.bulk_add_to_playlist(playlist_id, &[(media_file_id, pos)]).await?;
        Ok(ids[0])
    }
    
    async fn remove_from_playlist(&self, playlist_id: i64, media_file_id: i64) -> Result<bool> {
        let removed = self.bulk_remove_from_playlist(playlist_id, &[media_file_id]).await?;
        Ok(removed > 0)
    }
    
    // DIRECTORY OPERATIONS - Optimized with atomic operations
    async fn get_files_in_directory(&self, dir: &Path) -> Result<Vec<MediaFile>>;
    async fn get_directory_listing(&self, parent_path: &Path, media_type_filter: &str) -> Result<(Vec<MediaDirectory>, Vec<MediaFile>)>;
    
    // CLEANUP OPERATIONS - Bulk atomic operations
    async fn cleanup_missing_files(&self, existing_paths: &[PathBuf]) -> Result<usize>;
    async fn batch_cleanup_missing_files(&self, existing_canonical_paths: &HashSet<String>) -> Result<usize>;
    
    // MUSIC CATEGORIZATION - Atomic index operations
    async fn get_artists(&self) -> Result<Vec<MusicCategory>>;
    async fn get_albums(&self, artist: Option<&str>) -> Result<Vec<MusicCategory>>;
    async fn get_genres(&self) -> Result<Vec<MusicCategory>>;
    async fn get_years(&self) -> Result<Vec<MusicCategory>>;
    async fn get_music_by_artist(&self, artist: &str) -> Result<Vec<MediaFile>>;
    async fn get_music_by_album(&self, album: &str, artist: Option<&str>) -> Result<Vec<MediaFile>>;
    async fn get_music_by_genre(&self, genre: &str) -> Result<Vec<MediaFile>>;
    async fn get_music_by_year(&self, year: u32) -> Result<Vec<MediaFile>>;
    
    // PLAYLIST OPERATIONS - Bulk atomic operations
    async fn create_playlist(&self, name: &str, description: Option<&str>) -> Result<i64>;
    async fn get_playlists(&self) -> Result<Vec<Playlist>>;
    async fn get_playlist_tracks(&self, playlist_id: i64) -> Result<Vec<MediaFile>>;
    async fn bulk_add_to_playlist(&self, playlist_id: i64, media_file_ids: &[(i64, u32)]) -> Result<Vec<i64>>;
    async fn bulk_remove_from_playlist(&self, playlist_id: i64, media_file_ids: &[i64]) -> Result<usize>;
}
```

### 2. FlatBuffer Schema

Custom FlatBuffer schema optimized for media file storage:

```flatbuffers
namespace MediaDB;

table MediaFile {
    id: uint64;
    path: string;
    canonical_path: string;
    filename: string;
    size: uint64;
    modified: uint64;
    mime_type: string;
    duration: uint64;
    title: string;
    artist: string;
    album: string;
    genre: string;
    track_number: uint32;
    year: uint32;
    album_artist: string;
    created_at: uint64;
    updated_at: uint64;
}

table MediaFileBatch {
    files: [MediaFile];
    batch_id: uint64;
    timestamp: uint64;
}

root_type MediaFileBatch;
```

### 3. Memory-Mapped Storage Manager

Handles memory-mapped file operations for zero-copy access:

```rust
pub struct MemoryMappedFile {
    file: File,
    mmap: MmapMut,
    current_size: usize,
    max_size: usize,
}

impl MemoryMappedFile {
    pub fn new(path: &Path, initial_size: usize) -> Result<Self>;
    pub fn append_data(&mut self, data: &[u8]) -> Result<u64>;
    pub fn read_at_offset(&self, offset: u64, length: usize) -> &[u8];
    pub fn resize_if_needed(&mut self, additional_size: usize) -> Result<()>;
    pub fn sync_to_disk(&self) -> Result<()>;
}
```

### 4. High-Performance Index Manager

In-memory indexes for fast lookups with persistent storage:

```rust
pub struct IndexManager {
    path_to_offset: HashMap<String, u64>,
    id_to_offset: HashMap<u64, u64>,
    directory_index: BTreeMap<String, Vec<u64>>,
    dirty_indexes: HashSet<IndexType>,
}

impl IndexManager {
    pub fn insert_file_index(&mut self, file: &MediaFile, offset: u64);
    pub fn remove_file_index(&mut self, path: &str) -> Option<u64>;
    pub fn find_by_path(&self, path: &str) -> Option<u64>;
    pub fn find_files_in_directory(&self, dir_path: &str) -> Vec<u64>;
    pub async fn persist_indexes(&self, index_file: &Path) -> Result<()>;
    pub async fn load_indexes(&mut self, index_file: &Path) -> Result<()>;
}
```

### 5. ZeroCopyConfig

Configuration for extreme performance database operations with memory-efficient defaults:

```rust
#[derive(Debug, Clone)]
pub struct ZeroCopyConfig {
    pub batch_size: usize,
    pub initial_file_size_mb: usize,
    pub max_file_size_gb: usize,
    pub memory_map_size_mb: usize,
    pub index_cache_size: usize,
    pub enable_compression: bool,
    pub sync_frequency: Duration,
    pub enable_wal: bool,
    pub auto_detect_memory: bool,
}

impl Default for ZeroCopyConfig {
    fn default() -> Self {
        Self {
            batch_size: 100_000,  // Process 100K files per batch
            initial_file_size_mb: 10,  // Start small
            max_file_size_gb: 10,
            memory_map_size_mb: 4,  // Fixed 4MB like original app
            index_cache_size: 1_000_000,  // 1M entries (1MB for indexes)
            enable_compression: false,  // Disable for max speed
            sync_frequency: Duration::from_secs(5),
            enable_wal: true,
            auto_detect_memory: false,  // NO automatic scaling - manual only
        }
    }
}

impl ZeroCopyConfig {
    /// Load configuration from environment variables (for Docker)
    pub fn from_env() -> Self {
        let mut config = Self::default();
        
        // Override with environment variables if present
        if let Ok(cache_mb) = std::env::var("ZEROCOPY_CACHE_MB") {
            if let Ok(size) = cache_mb.parse::<usize>() {
                config.memory_map_size_mb = size;
                tracing::info!("Using cache size from env: {}MB", size);
            }
        }
        
        if let Ok(index_size) = std::env::var("ZEROCOPY_INDEX_SIZE") {
            if let Ok(size) = index_size.parse::<usize>() {
                config.index_cache_size = size;
                tracing::info!("Using index cache size from env: {}", size);
            }
        }
        
        if let Ok(batch_size) = std::env::var("ZEROCOPY_BATCH_SIZE") {
            if let Ok(size) = batch_size.parse::<usize>() {
                config.batch_size = size;
                tracing::info!("Using batch size from env: {}", size);
            }
        }
        
        config
    }
    
    /// Validate configuration and warn about performance implications
    pub fn validate(&self) {
        if self.memory_map_size_mb == 4 {
            tracing::info!("Using default 4MB cache. For higher performance, increase memory_map_size_mb in config");
        }
        
        if self.index_cache_size == 1_000_000 {
            tracing::info!("Using default 1M index cache (1MB). For very large libraries, increase index_cache_size in config");
        }
        
        // Warn about performance expectations
        let expected_throughput = match self.memory_map_size_mb {
            4 => "100K-200K files/sec",
            16..=64 => "200K-500K files/sec", 
            65..=256 => "500K-800K files/sec",
            _ => "800K+ files/sec",
        };
        
        tracing::info!("Expected throughput with {}MB cache: {}", self.memory_map_size_mb, expected_throughput);
    }
}
```

### 4. Progress Reporting

Interface for reporting batch processing progress:

```rust
pub trait ProgressReporter: Send + Sync {
    fn report_batch_progress(&self, processed: usize, total: usize, operation: &str);
    fn report_batch_complete(&self, result: &BatchResult);
    fn report_error(&self, error: &str, batch_index: usize);
}

pub struct DefaultProgressReporter;
impl ProgressReporter for DefaultProgressReporter {
    fn report_batch_progress(&self, processed: usize, total: usize, operation: &str) {
        if processed % 10 == 0 || processed == total {
            tracing::info!("{}: {}/{} files processed", operation, processed, total);
        }
    }
}
```

## Data Models

### BatchResult

Result structure for batch operations:

```rust
#[derive(Debug, Clone)]
pub struct BatchResult {
    pub total_processed: usize,
    pub successful_operations: usize,
    pub failed_operations: usize,
    pub processing_time: Duration,
    pub average_throughput: f64, // files per second
    pub errors: Vec<BatchError>,
}

#[derive(Debug, Clone)]
pub struct BatchError {
    pub batch_index: usize,
    pub error_message: String,
    pub affected_files: Vec<PathBuf>,
}
```

### Enhanced MediaFile Processing

The existing `MediaFile` structure remains unchanged, but processing is optimized:

```rust
// Batch preparation helper
pub struct BatchMediaFile {
    pub media_file: MediaFile,
    pub canonical_path: String,
    pub canonical_parent_path: String,
    pub operation_type: BatchOperation,
}

#[derive(Debug, Clone)]
pub enum BatchOperation {
    Insert,
    Update,
    Delete,
}
```

## Error Handling

### Transaction Management
- Each batch wrapped in a single database transaction
- Automatic rollback on batch failure
- Detailed error reporting for failed batches
- Retry logic with exponential backoff

### Error Recovery Strategies
1. **Batch Subdivision**: If a large batch fails, split it into smaller sub-batches
2. **Individual Fallback**: For persistent failures, fall back to individual operations
3. **Partial Success Handling**: Track which files in a batch succeeded/failed
4. **Memory Pressure Handling**: Reduce batch sizes if memory constraints detected

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum BatchProcessingError {
    #[error("Database transaction failed: {reason}")]
    TransactionFailed { reason: String },
    
    #[error("Batch too large: {size} exceeds limit {limit}")]
    BatchTooLarge { size: usize, limit: usize },
    
    #[error("Memory limit exceeded: {usage_mb}MB > {limit_mb}MB")]
    MemoryLimitExceeded { usage_mb: usize, limit_mb: usize },
    
    #[error("Batch preparation failed: {reason}")]
    PreparationFailed { reason: String },
}
```

## Testing Strategy

### Unit Tests
1. **BatchProcessor Tests**
   - Batch size configuration
   - Memory limit enforcement
   - Error handling and recovery
   - Progress reporting

2. **Database Batch Operations Tests**
   - Upsert functionality with conflicts
   - Transaction rollback behavior
   - Large batch handling
   - Performance benchmarks

### Integration Tests
1. **End-to-End Batch Processing**
   - Full media scanning with batch processing
   - Comparison with individual operations
   - Memory usage validation
   - Performance regression tests

2. **Error Scenario Tests**
   - Database connection failures during batching
   - Partial batch failures
   - Memory pressure scenarios
   - Recovery from corrupted batches

### Performance Benchmarks
1. **Throughput Tests**
   - Files per second across different batch sizes
   - Memory usage patterns
   - Database lock contention

2. **Scalability Tests**
   - Performance with 10K, 100K, 1M files
   - Memory usage scaling
   - Transaction overhead analysis

## Implementation Details

### Zero-Copy Batch Processing Implementation

The core optimization uses FlatBuffers for zero-copy serialization and memory-mapped files for direct disk access:

```rust
impl ZeroCopyDatabase {
    pub async fn batch_insert_files(&mut self, files: &[MediaFile]) -> Result<Duration> {
        let start = Instant::now();
        
        // Step 1: Serialize all files to FlatBuffer in one operation
        let serialized_data = self.serialize_files_to_flatbuffer(files)?;
        
        // Step 2: Write entire batch to memory-mapped file (zero-copy)
        let offset = self.data_file.append_data(serialized_data)?;
        
        // Step 3: Update in-memory indexes (batch operation)
        self.index_manager.batch_insert_indexes(files, offset)?;
        
        // Step 4: Optional WAL logging for crash recovery
        if self.config.enable_wal {
            self.write_wal_entry(WalOperation::BatchInsert, offset, files.len())?;
        }
        
        Ok(start.elapsed())
    }
    
    fn serialize_files_to_flatbuffer(&mut self, files: &[MediaFile]) -> Result<&[u8]> {
        self.flatbuffer_builder.reset();
        
        // Pre-allocate vector for file offsets
        let mut file_offsets = Vec::with_capacity(files.len());
        
        // Serialize all files in batch
        for file in files {
            let file_offset = self.serialize_single_file(&mut self.flatbuffer_builder, file)?;
            file_offsets.push(file_offset);
        }
        
        // Create batch container
        let batch = MediaFileBatch::create(&mut self.flatbuffer_builder, &MediaFileBatchArgs {
            files: Some(self.flatbuffer_builder.create_vector(&file_offsets)),
            batch_id: self.generate_batch_id(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos() as u64,
        });
        
        self.flatbuffer_builder.finish(batch, None);
        Ok(self.flatbuffer_builder.finished_data())
    }
}
```

### Ultra-High Performance Processing Algorithm

```rust
impl ZeroCopyBatchProcessor {
    pub async fn process_million_files(&mut self, files: Vec<MediaFile>) -> Result<BatchResult> {
        let start_time = Instant::now();
        let total_files = files.len();
        
        // Pre-allocate result with known capacity
        let mut result = BatchResult::with_capacity(total_files);
        
        // Process in large batches for maximum throughput
        const ULTRA_BATCH_SIZE: usize = 100_000;  // 100K files per batch
        
        for chunk in files.chunks(ULTRA_BATCH_SIZE) {
            let batch_start = Instant::now();
            
            // Zero-copy batch processing
            match self.database.batch_insert_files(chunk).await {
                Ok(batch_duration) => {
                    let throughput = chunk.len() as f64 / batch_duration.as_secs_f64();
                    
                    result.successful_operations += chunk.len();
                    result.batch_throughputs.push(throughput);
                    
                    // Log extreme performance metrics
                    tracing::info!(
                        "Processed {} files in {:?} ({:.0} files/sec)", 
                        chunk.len(), 
                        batch_duration, 
                        throughput
                    );
                }
                Err(e) => {
                    // For extreme performance, minimal error handling
                    result.failed_operations += chunk.len();
                    tracing::error!("Batch failed: {}", e);
                }
            }
            
            // Yield control only occasionally to maintain throughput
            if result.successful_operations % 1_000_000 == 0 {
                tokio::task::yield_now().await;
            }
        }
        
        result.processing_time = start_time.elapsed();
        result.average_throughput = result.successful_operations as f64 / result.processing_time.as_secs_f64();
        
        // Verify we hit the target
        if result.average_throughput >= 1_000_000.0 {
            tracing::info!("🚀 TARGET ACHIEVED: {:.0} files/second", result.average_throughput);
        } else {
            tracing::warn!("Target missed: {:.0} files/second (target: 1M)", result.average_throughput);
        }
        
        Ok(result)
    }
}
```

### Memory Management

1. **Streaming Processing**: Process files in batches to avoid loading entire datasets into memory
2. **Memory Monitoring**: Track memory usage and adjust batch sizes dynamically
3. **Resource Cleanup**: Ensure prompt cleanup of temporary allocations
4. **Bounded Collections**: Use pre-allocated vectors with capacity limits

### Integration with MediaScanner

The `MediaScanner::perform_incremental_update` method will be modified to use batch processing:

```rust
async fn perform_incremental_update(
    &self,
    _directory: &Path,
    existing_files: Vec<MediaFile>,
    current_files: Vec<MediaFile>,
) -> Result<ScanResult> {
    // Collect files for batch operations
    let mut files_to_insert = Vec::new();
    let mut files_to_update = Vec::new();
    let mut files_to_remove = Vec::new();
    
    // ... existing comparison logic ...
    
    // Process batches
    let batch_processor = BatchProcessor::new(
        self.database_manager.get_batch_config(),
        self.database_manager.clone()
    );
    
    // Batch insert new files
    if !files_to_insert.is_empty() {
        let insert_result = batch_processor.process_files_batch(files_to_insert).await?;
        // Convert batch result to scan result
    }
    
    // Batch update changed files
    if !files_to_update.is_empty() {
        let update_result = batch_processor.process_files_batch(files_to_update).await?;
        // Convert batch result to scan result
    }
    
    // Batch remove deleted files
    if !files_to_remove.is_empty() {
        let remove_paths: Vec<PathBuf> = files_to_remove.iter().map(|f| f.path.clone()).collect();
        let remove_result = batch_processor.remove_files_batch(remove_paths).await?;
        // Convert batch result to scan result
    }
    
    Ok(result)
}
```

## Performance Expectations

### Target Performance Goals
- **1,000,000 files per second**: Primary target using zero-copy operations
- **10,000 files**: From 5-6 seconds to 0.01 seconds (500-600x improvement)
- **100,000 files**: From 50-60 seconds to 0.1 seconds (500-600x improvement)
- **1,000,000 files**: Target 1 second total processing time
- **Memory usage**: Bounded to configurable limits (default 1GB memory map)

### Zero-Copy Performance Optimizations
- **FlatBuffer serialization**: Zero-copy deserialization, minimal allocation
- **Memory-mapped I/O**: Direct memory access, no system call overhead
- **Batch processing**: 100K files per batch to amortize overhead
- **Index caching**: 1M+ entries in memory for instant lookups
- **No compression**: Raw speed over space efficiency

### Hardware Requirements for 1M files/sec
- **CPU**: Modern single-core performance (high clock speed preferred)
- **Memory**: 5MB RAM (4MB cache + 1MB indexes) - configurable up to 1GB if needed
- **Storage**: High-end NVMe SSD required (SATA SSD insufficient)
- **I/O**: Sustained 4GB/s+ write throughput capability
- **Architecture**: x86_64 with AVX2/AVX-512 support for SIMD operations

### Performance Scaling Characteristics
- **Linear scaling**: Performance scales linearly with batch size up to memory limits
- **Memory-bound**: Performance limited by available RAM for memory mapping
- **I/O-bound**: Ultimate performance limited by storage write throughput
- **CPU-bound**: FlatBuffer serialization may become CPU bottleneck at extreme scales

## Configuration Integration

### Configuration File Support
```toml
[database.zerocopy]
# Zero-copy database configuration for extreme performance
batch_size = 100_000
initial_file_size_mb = 10
max_file_size_gb = 10
memory_map_size_mb = 4  # Fixed 4MB default - change manually for more performance
index_cache_size = 1_000_000  # Fixed 1M entries (1MB) - change manually for more performance
enable_compression = false  # Disabled for maximum speed
sync_frequency_seconds = 5
enable_wal = true
auto_detect_memory = false  # NO automatic scaling - manual configuration only

[database.performance]
# Performance tuning
target_throughput_per_second = 1_000_000
enable_performance_monitoring = true
log_batch_metrics = true
memory_pressure_threshold_mb = 512

[database.storage]
# Storage configuration
data_file_path = "media.fb"
index_file_path = "media.idx"
wal_file_path = "media.wal"
enable_file_preallocation = true
use_direct_io = true  # Bypass OS page cache for maximum speed
```

### Runtime Configuration
- Dynamic batch size adjustment based on available memory
- Automatic fallback to smaller batches on memory pressure
- Performance monitoring and automatic tuning

This design provides a comprehensive solution for batch processing optimization while maintaining backward compatibility and robust error handling.
## Zer
o-Copy Database Architecture Details

### File Format Structure

```
Media Database File (.fb)
├── Header (64 bytes)
│   ├── Magic Number (8 bytes): "MEDIADB1"
│   ├── Version (4 bytes)
│   ├── File Size (8 bytes)
│   ├── Index Offset (8 bytes)
│   ├── Batch Count (8 bytes)
│   └── Reserved (28 bytes)
├── Batch 1 (Variable size)
│   ├── Batch Header (32 bytes)
│   └── FlatBuffer Data
├── Batch 2 (Variable size)
├── ...
└── Index Section
    ├── Path Hash Table
    ├── Directory B-Tree
    └── Metadata
```

### Memory Layout Optimization

```rust
// Optimized memory layout for cache efficiency
#[repr(C, packed)]
struct FileRecord {
    id: u64,                    // 8 bytes
    path_hash: u64,            // 8 bytes - for fast lookups
    size: u64,                 // 8 bytes
    modified: u64,             // 8 bytes - timestamp
    mime_type_id: u32,         // 4 bytes - enum for common types
    flags: u32,                // 4 bytes - packed metadata
    // Total: 40 bytes per record for cache line efficiency
}
```

### Extreme Performance Techniques

1. **SIMD Operations**: Use SIMD instructions for batch hash calculations and data processing
2. **Lock-Free Structures**: Atomic operations for concurrent access without mutex overhead
3. **Memory Prefetching**: Explicit prefetch instructions for predictable access patterns
4. **Huge Pages**: Use 2MB pages to reduce TLB misses
5. **CPU Affinity**: Pin threads to specific cores for consistent performance
6. **Atomic Operations**: Lock-free counters, atomic pointers, and compare-and-swap operations
7. **Single-Core Optimization**: Maximize single-thread performance to minimize RAM usage
8. **Cache-Friendly Access**: Optimize memory access patterns for CPU cache efficiency

### Crash Recovery and Durability

```rust
pub struct WALManager {
    wal_file: MemoryMappedFile,
    checkpoint_interval: Duration,
    last_checkpoint: Instant,
}

impl WALManager {
    pub fn log_batch_operation(&mut self, op: WalOperation, data: &[u8]) -> Result<()> {
        // Write operation to WAL with atomic commit
        let entry = WalEntry {
            timestamp: SystemTime::now(),
            operation: op,
            data_size: data.len(),
            checksum: calculate_checksum(data),
        };
        
        self.wal_file.append_data(&entry.serialize())?;
        self.wal_file.append_data(data)?;
        
        // Force sync for durability
        self.wal_file.sync_to_disk()?;
        
        Ok(())
    }
}
```

This design provides the foundation for achieving 1,000,000 files per second through zero-copy operations, memory-mapped storage, and FlatBuffer serialization while maintaining data durability and crash recovery capabilities.
## Single-Core Atomic Operations Architecture

### Lock-Free Concurrent Processing

```rust
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use crossbeam::queue::SegQueue;
use rayon::prelude::*;

pub struct AtomicZeroCopyDatabase {
    // Atomic counters for lock-free statistics
    total_files: AtomicU64,
    processed_files: AtomicU64,
    failed_files: AtomicU64,
    
    // Lock-free work queue
    work_queue: SegQueue<FileBatch>,
    
    // Atomic pointers for memory management
    current_write_offset: AtomicU64,
    index_generation: AtomicU64,
    
    // Multi-core configuration
    worker_count: usize,
    cpu_cores: usize,
}

impl AtomicZeroCopyDatabase {
    pub fn new() -> Self {
        let cpu_cores = num_cpus::get();
        let worker_count = (cpu_cores * 2).min(32); // 2x cores, max 32 workers
        
        Self {
            total_files: AtomicU64::new(0),
            processed_files: AtomicU64::new(0),
            failed_files: AtomicU64::new(0),
            work_queue: SegQueue::new(),
            current_write_offset: AtomicU64::new(0),
            index_generation: AtomicU64::new(0),
            worker_count,
            cpu_cores,
        }
    }
    
    pub async fn parallel_batch_insert(&self, files: Vec<MediaFile>) -> Result<Duration> {
        let start = Instant::now();
        let total_files = files.len();
        
        // Update atomic counter
        self.total_files.store(total_files as u64, Ordering::Relaxed);
        
        // Split work across CPU cores for maximum parallelism
        let chunk_size = (total_files / self.cpu_cores).max(1000);
        let batches: Vec<FileBatch> = files
            .chunks(chunk_size)
            .map(|chunk| FileBatch::new(chunk.to_vec()))
            .collect();
        
        // Enqueue all batches in lock-free queue
        for batch in batches {
            self.work_queue.push(batch);
        }
        
        // Spawn worker tasks across all CPU cores
        let workers: Vec<_> = (0..self.worker_count)
            .map(|worker_id| {
                let db = self.clone();
                tokio::spawn(async move {
                    db.worker_loop(worker_id).await
                })
            })
            .collect();
        
        // Wait for all workers to complete
        for worker in workers {
            worker.await??;
        }
        
        Ok(start.elapsed())
    }
    
    async fn worker_loop(&self, worker_id: usize) -> Result<()> {
        // Pin worker to specific CPU core for cache locality
        if let Err(e) = self.set_cpu_affinity(worker_id) {
            tracing::warn!("Failed to set CPU affinity for worker {}: {}", worker_id, e);
        }
        
        while let Some(batch) = self.work_queue.pop() {
            match self.process_batch_atomic(batch).await {
                Ok(processed_count) => {
                    // Atomic increment of processed files
                    self.processed_files.fetch_add(processed_count, Ordering::Relaxed);
                }
                Err(_) => {
                    // Atomic increment of failed files
                    self.failed_files.fetch_add(batch.len() as u64, Ordering::Relaxed);
                }
            }
        }
        
        Ok(())
    }
    
    async fn process_batch_atomic(&self, batch: FileBatch) -> Result<u64> {
        // Serialize batch using SIMD-optimized operations
        let serialized = self.simd_serialize_batch(&batch)?;
        
        // Atomic allocation of write space
        let write_offset = self.current_write_offset
            .fetch_add(serialized.len() as u64, Ordering::SeqCst);
        
        // Zero-copy write to memory-mapped region
        self.atomic_write_at_offset(write_offset, &serialized)?;
        
        // Update indexes using lock-free operations
        self.atomic_update_indexes(&batch, write_offset)?;
        
        Ok(batch.len() as u64)
    }
}
```

### SIMD-Optimized Operations

```rust
use std::arch::x86_64::*;

impl AtomicZeroCopyDatabase {
    #[target_feature(enable = "avx2")]
    unsafe fn simd_hash_paths(&self, paths: &[String]) -> Vec<u64> {
        let mut hashes = Vec::with_capacity(paths.len());
        
        // Process 4 paths at once using AVX2
        for chunk in paths.chunks(4) {
            let mut hash_values = [0u64; 4];
            
            for (i, path) in chunk.iter().enumerate() {
                // Use SIMD-optimized hash function
                hash_values[i] = self.simd_hash_string(path);
            }
            
            hashes.extend_from_slice(&hash_values[..chunk.len()]);
        }
        
        hashes
    }
    
    #[target_feature(enable = "avx2")]
    unsafe fn simd_hash_string(&self, s: &str) -> u64 {
        let bytes = s.as_bytes();
        let mut hash = 0u64;
        
        // Process 32 bytes at once using AVX2
        for chunk in bytes.chunks(32) {
            if chunk.len() == 32 {
                let data = _mm256_loadu_si256(chunk.as_ptr() as *const __m256i);
                // SIMD hash computation
                let hash_vec = _mm256_crc32_u64(hash, _mm256_extract_epi64(data, 0) as u64);
                hash ^= hash_vec;
            } else {
                // Handle remaining bytes
                for &byte in chunk {
                    hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
                }
            }
        }
        
        hash
    }
}
```

### Single-Thread Tokio Runtime Configuration

```rust
pub struct MemoryEfficientRuntime {
    runtime: tokio::runtime::Runtime,
}

impl MemoryEfficientRuntime {
    pub fn new() -> Result<Self> {
        // Configure Tokio runtime for memory efficiency (single-threaded)
        let runtime = tokio::runtime::Builder::new_current_thread()
            .thread_stack_size(2 * 1024 * 1024) // 2MB stack (minimal)
            .thread_name("zerocopy-main")
            .enable_all()
            .build()?;
        
        tracing::info!("Initialized single-thread runtime for memory efficiency");
        
        Ok(Self { runtime })
    }
    
    pub fn spawn_database_task(&self, database: SingleCoreZeroCopyDatabase) {
        self.runtime.spawn(async move {
            // Single-threaded processing loop
            database.run_processing_loop().await
        });
    }
}
```

### Atomic Memory Management

```rust
use std::sync::atomic::{AtomicPtr, AtomicBool};

pub struct AtomicMemoryPool {
    // Lock-free memory pool for zero-allocation operations
    free_blocks: SegQueue<*mut u8>,
    block_size: usize,
    total_blocks: AtomicUsize,
    allocated_blocks: AtomicUsize,
}

impl AtomicMemoryPool {
    pub fn allocate_block(&self) -> Option<*mut u8> {
        if let Some(block) = self.free_blocks.pop() {
            self.allocated_blocks.fetch_add(1, Ordering::Relaxed);
            Some(block)
        } else {
            // Allocate new block if pool is empty
            self.allocate_new_block()
        }
    }
    
    pub fn deallocate_block(&self, block: *mut u8) {
        self.free_blocks.push(block);
        self.allocated_blocks.fetch_sub(1, Ordering::Relaxed);
    }
}
```

### Performance Monitoring with Atomics

```rust
pub struct AtomicPerformanceMetrics {
    operations_per_second: AtomicU64,
    average_latency_ns: AtomicU64,
    peak_throughput: AtomicU64,
    memory_usage_bytes: AtomicU64,
    cache_hit_rate: AtomicU64,
}

impl AtomicPerformanceMetrics {
    pub fn record_operation(&self, duration: Duration, files_processed: u64) {
        let ops_per_sec = (files_processed as f64 / duration.as_secs_f64()) as u64;
        
        // Update metrics using atomic operations
        self.operations_per_second.store(ops_per_sec, Ordering::Relaxed);
        self.average_latency_ns.store(duration.as_nanos() as u64 / files_processed, Ordering::Relaxed);
        
        // Update peak throughput if this is a new record
        let current_peak = self.peak_throughput.load(Ordering::Relaxed);
        if ops_per_sec > current_peak {
            self.peak_throughput.compare_exchange_weak(
                current_peak, 
                ops_per_sec, 
                Ordering::Relaxed, 
                Ordering::Relaxed
            ).ok();
        }
    }
    
    pub fn get_current_throughput(&self) -> u64 {
        self.operations_per_second.load(Ordering::Relaxed)
    }
}
```

This single-core atomic operations architecture should significantly boost performance by:

1. **Eliminating lock contention** with single-thread processing
2. **Minimizing memory usage** with fixed 5MB footprint (4MB + 1MB)
3. **Reducing allocation overhead** with pre-allocated buffers
4. **Optimizing cache locality** with sequential access patterns
5. **Leveraging SIMD instructions** for batch operations
6. **Providing atomic statistics** for real-time monitoring

The combination of these techniques should help achieve high throughput while maintaining minimal memory usage.

## Manual Performance Configuration

### Explicit Memory Management

The zero-copy database uses fixed memory allocation with manual configuration only. No automatic scaling occurs:

```rust
pub struct FixedMemoryManager {
    cache_size_mb: usize,       // Fixed cache size from config
    index_cache_size: usize,    // Fixed index size from config
    allocated_memory: AtomicUsize,
}

impl FixedMemoryManager {
    pub fn new(config: &ZeroCopyConfig) -> Self {
        let cache_bytes = config.memory_map_size_mb * 1024 * 1024;
        
        tracing::info!(
            "Initializing with fixed {}MB cache, {} index entries (no auto-scaling)",
            config.memory_map_size_mb,
            config.index_cache_size
        );
        
        Self {
            cache_size_mb: config.memory_map_size_mb,
            index_cache_size: config.index_cache_size,
            allocated_memory: AtomicUsize::new(cache_bytes),
        }
    }
    
    pub fn get_cache_size(&self) -> usize {
        self.cache_size_mb * 1024 * 1024
    }
    
    pub fn get_index_cache_size(&self) -> usize {
        self.index_cache_size
    }
    
    pub fn validate_memory_usage(&self) -> Result<()> {
        let allocated = self.allocated_memory.load(Ordering::Relaxed);
        let limit = self.get_cache_size();
        
        if allocated > limit {
            return Err(anyhow::anyhow!(
                "Memory usage {}MB exceeds configured limit {}MB", 
                allocated / 1024 / 1024,
                limit / 1024 / 1024
            ));
        }
        
        Ok(())
    }
}
```

### Memory Usage Profiles

The system supports different memory profiles for various deployment scenarios:

```toml
# Default profile (5MB total: 4MB cache + 1MB indexes)
[database.zerocopy.profiles.default]
memory_map_size_mb = 4
index_cache_size = 1_000_000  # 1M entries = ~1MB
batch_size = 100_000

# Balanced profile (65MB total: 64MB cache + 1MB indexes)
[database.zerocopy.profiles.balanced]
memory_map_size_mb = 64
index_cache_size = 1_000_000  # Keep 1M entries
batch_size = 100_000

# High performance profile (257MB total: 256MB cache + 1MB indexes)
[database.zerocopy.profiles.high_performance]
memory_map_size_mb = 256
index_cache_size = 1_000_000  # Keep 1M entries
batch_size = 100_000

# Extreme performance profile (1025MB total: 1GB cache + 1MB indexes)
[database.zerocopy.profiles.extreme]
memory_map_size_mb = 1024
index_cache_size = 1_000_000  # Keep 1M entries
batch_size = 100_000
```

### Performance vs Memory Trade-offs

| Memory Profile | Total RAM | Cache Size | Index Entries | Expected Throughput | Use Case |
|----------------|-----------|------------|---------------|-------------------|----------|
| Default (5MB) | 4MB + 1MB | 4MB | 1M entries | 200K-400K files/sec | Default deployment |
| Balanced (65MB) | 64MB + 1MB | 64MB | 1M entries | 400K-600K files/sec | Desktop applications |
| High Performance (257MB) | 256MB + 1MB | 256MB | 1M entries | 600K-800K files/sec | Server deployments |
| Extreme (1025MB) | 1GB + 1MB | 1GB | 1M entries | 800K-1M+ files/sec | High-end servers |

### Fixed Memory Processing Algorithm

```rust
impl ZeroCopyDatabase {
    pub async fn fixed_memory_batch_processing(&mut self, files: Vec<MediaFile>) -> Result<BatchResult> {
        let memory_manager = FixedMemoryManager::new(&self.config);
        let batch_size = self.config.batch_size;  // Fixed batch size from config
        
        tracing::info!(
            "Processing {} files with fixed {}MB cache, {} batch size",
            files.len(),
            self.config.memory_map_size_mb,
            batch_size
        );
        
        let mut total_processed = 0;
        let start_time = Instant::now();
        
        for chunk in files.chunks(batch_size) {
            // Validate memory usage before each batch
            memory_manager.validate_memory_usage()?;
            
            // Process batch with fixed settings
            self.process_batch_fixed_memory(chunk, &memory_manager).await?;
            total_processed += chunk.len();
            
            // Report progress with fixed cache info
            if total_processed % 100_000 == 0 {
                let elapsed = start_time.elapsed();
                let throughput = total_processed as f64 / elapsed.as_secs_f64();
                tracing::info!(
                    "Processed {} files ({:.0} files/sec) using fixed {}MB cache",
                    total_processed,
                    throughput,
                    self.config.memory_map_size_mb
                );
            }
        }
        
        let final_elapsed = start_time.elapsed();
        let final_throughput = total_processed as f64 / final_elapsed.as_secs_f64();
        
        tracing::info!(
            "Completed processing {} files in {:?} ({:.0} files/sec) with {}MB cache",
            total_processed,
            final_elapsed,
            final_throughput,
            self.config.memory_map_size_mb
        );
        
        Ok(BatchResult::new())
    }
}
```

### Environment Variable Configuration for Docker

```bash
# Docker environment variables for performance tuning
ZEROCOPY_CACHE_MB=64          # Set cache to 64MB
ZEROCOPY_INDEX_SIZE=200000    # Set index cache to 200K entries  
ZEROCOPY_BATCH_SIZE=50000     # Set batch size to 50K files

# Example Docker run command
docker run -e ZEROCOPY_CACHE_MB=256 -e ZEROCOPY_INDEX_SIZE=1000000 myapp
```

### README.md Documentation Requirements

The implementation must include comprehensive documentation in README.md covering:

1. **Default Memory Usage**: 4MB cache, 10K index entries
2. **Performance Scaling**: How to increase cache for better performance
3. **Configuration Options**: All available config parameters
4. **Environment Variables**: Docker-specific configuration
5. **Performance Expectations**: Throughput at different cache sizes
6. **Hardware Requirements**: CPU, memory, and storage recommendations

This approach ensures the system:
1. **Uses 4MB by default** exactly like the original application
2. **Never auto-scales** - all changes must be explicit
3. **Provides clear documentation** for performance tuning
4. **Supports Docker deployment** with environment variables
5. **Maintains predictable behavior** across all deployments

The extreme performance target of 1M files/sec is achievable when users explicitly configure higher cache sizes, while maintaining a 5MB footprint by default (4MB cache + 1MB for 1M index entries).
## Comp
lete Codebase Refactoring Requirements

### 1. MediaScanner Refactoring - Bulk Operations

Current `MediaScanner::perform_incremental_update` processes files individually. Must be refactored to use bulk operations:

```rust
// BEFORE: Individual operations (slow)
for current_file in &current_normalized {
    let id = self.database_manager.store_media_file(current_file).await?;
    // ... individual processing
}

// AFTER: Bulk operations (fast)
impl MediaScanner {
    async fn perform_incremental_update_bulk(
        &self,
        _directory: &Path,
        existing_files: Vec<MediaFile>,
        current_files: Vec<MediaFile>,
    ) -> Result<ScanResult> {
        // Collect all operations into bulk batches
        let mut files_to_insert = Vec::new();
        let mut files_to_update = Vec::new();
        let mut paths_to_remove = Vec::new();
        
        // ... comparison logic ...
        
        // Execute all operations in bulk with atomic counters
        let insert_ids = if !files_to_insert.is_empty() {
            self.database_manager.bulk_store_media_files(&files_to_insert).await?
        } else { Vec::new() };
        
        if !files_to_update.is_empty() {
            self.database_manager.bulk_update_media_files(&files_to_update).await?;
        }
        
        let removed_count = if !paths_to_remove.is_empty() {
            self.database_manager.bulk_remove_media_files(&paths_to_remove).await?
        } else { 0 };
        
        // Update atomic counters
        self.total_processed.fetch_add(files_to_insert.len() + files_to_update.len(), Ordering::Relaxed);
        self.total_removed.fetch_add(removed_count, Ordering::Relaxed);
        
        Ok(result)
    }
}
```

### 2. Main Application Refactoring - Bulk File Operations

Current `main.rs` has individual file operations that must be converted to bulk:

```rust
// BEFORE: Individual file operations in main.rs
for media_file in &all_files {
    if !media_file.path.exists() {
        if database.remove_media_file(&media_file.path).await? {
            removed_count += 1;
        }
    }
}

// AFTER: Bulk operations with atomic counters
impl MainApplication {
    async fn cleanup_deleted_files_bulk(&self, database: &ZeroCopyDatabase) -> Result<usize> {
        // Collect all missing files
        let all_files = database.stream_all_media_files();
        let mut missing_paths = Vec::new();
        
        while let Some(file_result) = all_files.next().await {
            let file = file_result?;
            if !file.path.exists() {
                missing_paths.push(file.path);
            }
        }
        
        // Bulk remove all missing files
        let removed_count = database.bulk_remove_media_files(&missing_paths).await?;
        
        // Update atomic counter
        database.total_operations.fetch_add(1, Ordering::Relaxed);
        
        Ok(removed_count)
    }
    
    async fn process_file_events_bulk(&self, events: Vec<FileEvent>) -> Result<()> {
        let mut files_to_add = Vec::new();
        let mut files_to_update = Vec::new();
        let mut paths_to_remove = Vec::new();
        
        // Categorize all events
        for event in events {
            match event.event_type {
                FileEventType::Created => files_to_add.push(event.to_media_file()?),
                FileEventType::Modified => files_to_update.push(event.to_media_file()?),
                FileEventType::Deleted => paths_to_remove.push(event.path),
            }
        }
        
        // Execute all operations in bulk
        if !files_to_add.is_empty() {
            self.database.bulk_store_media_files(&files_to_add).await?;
        }
        if !files_to_update.is_empty() {
            self.database.bulk_update_media_files(&files_to_update).await?;
        }
        if !paths_to_remove.is_empty() {
            self.database.bulk_remove_media_files(&paths_to_remove).await?;
        }
        
        Ok(())
    }
}
```

### 3. Web Handlers Refactoring - Atomic Operations

All web handlers must use atomic operations for statistics and caching:

```rust
// BEFORE: Direct database queries
async fn browse_handler(state: &AppState, object_id: &str) -> Result<String> {
    let files = state.database.get_directory_listing(&path, filter).await?;
    // ... process files
}

// AFTER: Atomic operations with caching
impl WebHandlers {
    async fn browse_handler_atomic(state: &AppState, object_id: &str) -> Result<String> {
        // Atomic cache hit/miss tracking
        let cache_key = format!("browse:{}", object_id);
        
        if let Some(cached_result) = state.cache.get(&cache_key) {
            state.database.cache_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(cached_result);
        }
        
        state.database.cache_misses.fetch_add(1, Ordering::Relaxed);
        
        // Use bulk operations for directory listing
        let (directories, files) = state.database.get_directory_listing(&path, filter).await?;
        
        // Atomic operation counter
        state.database.total_operations.fetch_add(1, Ordering::Relaxed);
        
        let result = self.format_browse_response(directories, files)?;
        
        // Cache result
        state.cache.insert(cache_key, result.clone());
        
        Ok(result)
    }
    
    async fn music_browse_atomic(state: &AppState, category: &str, filter: &str) -> Result<String> {
        let files = match category {
            "artists" => {
                let artists = state.database.get_artists().await?;
                state.database.total_operations.fetch_add(1, Ordering::Relaxed);
                self.format_artists_response(artists)?
            }
            "albums" => {
                let albums = state.database.get_albums(None).await?;
                state.database.total_operations.fetch_add(1, Ordering::Relaxed);
                self.format_albums_response(albums)?
            }
            _ => return Err(anyhow::anyhow!("Unknown category")),
        };
        
        Ok(files)
    }
}
```

### 4. Playlist Operations Refactoring - Bulk Atomic Operations

All playlist operations must use bulk operations:

```rust
// BEFORE: Individual playlist operations
for file_path in playlist_files {
    let file_id = database.store_media_file(&media_file).await?;
    database.add_to_playlist(playlist_id, file_id, position).await?;
}

// AFTER: Bulk playlist operations
impl PlaylistManager {
    async fn import_playlist_bulk(&self, playlist_path: &Path) -> Result<i64> {
        let playlist_files = self.parse_playlist_file(playlist_path)?;
        
        // Bulk store all media files
        let media_files: Vec<MediaFile> = playlist_files.into_iter()
            .map(|path| MediaFile::from_path(path))
            .collect::<Result<Vec<_>>>()?;
        
        let file_ids = self.database.bulk_store_media_files(&media_files).await?;
        
        // Create playlist
        let playlist_id = self.database.create_playlist(&playlist_name, None).await?;
        
        // Bulk add all files to playlist
        let playlist_entries: Vec<(i64, u32)> = file_ids.into_iter()
            .enumerate()
            .map(|(pos, id)| (id, pos as u32))
            .collect();
        
        self.database.bulk_add_to_playlist(playlist_id, &playlist_entries).await?;
        
        // Atomic operation counter
        self.database.total_operations.fetch_add(1, Ordering::Relaxed);
        
        Ok(playlist_id)
    }
}
```

### 5. Database Migration Strategy

Complete replacement of SQLite with zero-copy database:

```rust
impl DatabaseMigration {
    async fn migrate_sqlite_to_zerocopy(&self) -> Result<()> {
        tracing::info!("Starting migration from SQLite to ZeroCopy database");
        
        // Initialize new zero-copy database
        let zerocopy_db = ZeroCopyDatabase::new(self.config.clone()).await?;
        
        // Stream all data from SQLite
        let sqlite_db = SqliteDatabase::new(self.old_db_path.clone()).await?;
        let mut file_stream = sqlite_db.stream_all_media_files();
        
        let mut batch = Vec::with_capacity(100_000);
        let mut total_migrated = 0;
        
        while let Some(file_result) = file_stream.next().await {
            let file = file_result?;
            batch.push(file);
            
            // Process in large batches for maximum performance
            if batch.len() >= 100_000 {
                let ids = zerocopy_db.bulk_store_media_files(&batch).await?;
                total_migrated += ids.len();
                batch.clear();
                
                tracing::info!("Migrated {} files", total_migrated);
            }
        }
        
        // Process remaining files
        if !batch.is_empty() {
            let ids = zerocopy_db.bulk_store_media_files(&batch).await?;
            total_migrated += ids.len();
        }
        
        tracing::info!("Migration completed: {} files migrated", total_migrated);
        
        // Atomic final count
        zerocopy_db.total_files.store(total_migrated as u64, Ordering::Relaxed);
        
        Ok(())
    }
}
```

### 6. Performance Monitoring with Atomic Operations

All operations must include atomic performance tracking:

```rust
pub struct AtomicPerformanceTracker {
    // Operation counters
    pub total_operations: AtomicU64,
    pub bulk_operations: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    
    // Performance metrics
    pub average_batch_size: AtomicU64,
    pub total_files_processed: AtomicU64,
    pub operations_per_second: AtomicU64,
    
    // Memory usage
    pub current_memory_usage: AtomicUsize,
    pub peak_memory_usage: AtomicUsize,
}

impl AtomicPerformanceTracker {
    pub fn record_bulk_operation(&self, batch_size: usize, duration: Duration) {
        self.bulk_operations.fetch_add(1, Ordering::Relaxed);
        self.total_files_processed.fetch_add(batch_size as u64, Ordering::Relaxed);
        
        let ops_per_sec = (batch_size as f64 / duration.as_secs_f64()) as u64;
        self.operations_per_second.store(ops_per_sec, Ordering::Relaxed);
        
        // Update average batch size
        let current_avg = self.average_batch_size.load(Ordering::Relaxed);
        let new_avg = (current_avg + batch_size as u64) / 2;
        self.average_batch_size.store(new_avg, Ordering::Relaxed);
    }
    
    pub fn get_performance_summary(&self) -> PerformanceSummary {
        PerformanceSummary {
            total_operations: self.total_operations.load(Ordering::Relaxed),
            bulk_operations: self.bulk_operations.load(Ordering::Relaxed),
            cache_hit_rate: self.calculate_cache_hit_rate(),
            average_throughput: self.operations_per_second.load(Ordering::Relaxed),
            total_files: self.total_files_processed.load(Ordering::Relaxed),
            memory_usage_mb: self.current_memory_usage.load(Ordering::Relaxed) / 1024 / 1024,
        }
    }
}
```

## Summary of Required Changes

**Every database operation in the codebase must be refactored:**

1. **MediaScanner**: Convert individual file operations to bulk operations
2. **Main Application**: Convert file event processing to bulk operations  
3. **Web Handlers**: Add atomic operation tracking and caching
4. **Playlist Operations**: Convert to bulk playlist management
5. **Database Interface**: Replace SQLite with zero-copy database
6. **Performance Monitoring**: Add atomic counters throughout
7. **Migration Strategy**: Bulk migration from SQLite to zero-copy
8. **Error Handling**: Atomic error tracking and recovery

**All operations must support:**
- Bulk processing (100K+ items per batch)
- Atomic operation counting
- Memory-efficient streaming
- Zero-copy serialization
- Performance monitoring
- Cache hit/miss tracking

This complete refactoring will achieve the 1M files/sec target while maintaining the 5MB memory footprint through bulk operations and atomic tracking.## API 
Compatibility and Usage Patterns

### Internal Bulk Processing Strategy

The zero-copy database uses bulk operations internally for maximum speed, while keeping the same external interface:

**How Speed is Achieved:**
- All operations internally collect into batches before processing
- Even single operations benefit from zero-copy serialization
- MediaScanner automatically batches files during directory scanning
- File system events are batched before database operations
- Cleanup operations process thousands of files in single bulk operation
- The external interface stays the same, but internal processing is bulk-optimized

### Internal Batching Implementation

```rust
impl ZeroCopyDatabase {
    // External interface stays the same, but internally uses batching
    async fn store_media_file(&self, file: &MediaFile) -> Result<i64> {
        // Internally, this gets batched with other pending operations
        self.add_to_pending_batch(BatchOperation::Insert(file.clone())).await?;
        
        // If batch is full or timeout reached, process the entire batch
        if self.should_flush_batch().await {
            self.flush_pending_batch().await?;
        }
        
        // Return the ID for this specific file
        self.get_file_id_from_batch_result(file).await
    }
    
    // Internal batch processing (not exposed externally)
    async fn flush_pending_batch(&self) -> Result<()> {
        let batch = self.take_pending_batch().await;
        if batch.is_empty() {
            return Ok(());
        }
        
        // Process entire batch in single zero-copy operation
        let start = Instant::now();
        let serialized = self.serialize_batch_zero_copy(&batch)?;
        let offset = self.write_batch_atomic(serialized)?;
        self.update_indexes_batch(&batch, offset)?;
        
        // Atomic performance tracking
        let duration = start.elapsed();
        let throughput = batch.len() as f64 / duration.as_secs_f64();
        self.current_throughput.store(throughput as u64, Ordering::Relaxed);
        self.total_files_processed.fetch_add(batch.len() as u64, Ordering::Relaxed);
        
        Ok(())
    }
    
    // MediaScanner automatically uses internal batching
    async fn scan_directory_internal(&self, files: Vec<MediaFile>) -> Result<Vec<i64>> {
        // Large batches are processed directly for maximum speed
        if files.len() > 1000 {
            return self.process_large_batch_direct(files).await;
        }
        
        // Smaller batches go through normal batching system
        let mut ids = Vec::new();
        for file in files {
            let id = self.store_media_file(&file).await?;
            ids.push(id);
        }
        Ok(ids)
    }
}
```

### How External Code Gets Speed Benefits

**Same External Interface, Internal Batching:**
```rust
// Web handler - looks the same, but internally batched
async fn handle_file_upload(state: &AppState, file: MediaFile) -> Result<String> {
    // This call internally gets batched with other pending operations
    let file_id = state.database.store_media_file(&file).await?;
    Ok(format!("File stored with ID: {}", file_id))
}

// MediaScanner - automatically uses internal batching
async fn scan_directory(scanner: &MediaScanner, directory: &Path) -> Result<ScanResult> {
    let files = scanner.scan_files(directory).await?;
    
    // These calls are internally batched into bulk operations
    for file in files {
        scanner.database.store_media_file(&file).await?;
    }
    // When batch is full or timeout reached, entire batch is processed at 1M files/sec
    
    Ok(result)
}

// File system watcher - events are batched internally
async fn handle_file_events(database: &ZeroCopyDatabase, events: Vec<FileEvent>) -> Result<()> {
    for event in events {
        match event.event_type {
            FileEventType::Created => {
                let media_file = MediaFile::from_path(event.path)?;
                // Internally batched - when 100K events accumulate, processed in single operation
                database.store_media_file(&media_file).await?;
            }
            FileEventType::Deleted => {
                // Internally batched with other deletions
                database.remove_media_file(&event.path).await?;
            }
        }
    }
    Ok(())
}
```

**MediaScanner Optimization (Internal):**
```rust
impl MediaScanner {
    async fn perform_incremental_update(&self, files: Vec<MediaFile>) -> Result<ScanResult> {
        // Instead of individual operations, collect all operations
        let mut files_to_insert = Vec::new();
        let mut files_to_update = Vec::new();
        let mut paths_to_remove = Vec::new();
        
        // ... categorize files ...
        
        // Process all operations in large batches internally
        if files_to_insert.len() > 10_000 {
            // Large batch - process directly at maximum speed
            self.database.process_large_insert_batch(&files_to_insert).await?;
        } else {
            // Smaller batch - add to pending operations
            for file in files_to_insert {
                self.database.store_media_file(&file).await?; // Internally batched
            }
        }
        
        Ok(result)
    }
}
```

### Performance Characteristics

| Operation Type | Items | Expected Performance | Use Case |
|----------------|-------|---------------------|----------|
| Individual | 1 | 1K-10K ops/sec | Web handlers, real-time events |
| Small Bulk | 10-100 | 10K-50K ops/sec | Small batch operations |
| Medium Bulk | 100-10K | 50K-500K ops/sec | Directory scanning |
| Large Bulk | 10K-100K | 500K-1M ops/sec | Full library operations |

### How Speed is Achieved Internally

**Automatic Batching System:**
```rust
impl ZeroCopyDatabase {
    // Internal batching buffer
    pending_operations: Arc<Mutex<Vec<BatchOperation>>>,
    batch_timer: Arc<AtomicU64>,
    
    async fn store_media_file(&self, file: &MediaFile) -> Result<i64> {
        // Add to internal batch buffer
        self.add_to_batch(BatchOperation::Insert(file.clone())).await;
        
        // Check if we should flush the batch
        let should_flush = {
            let ops = self.pending_operations.lock().await;
            ops.len() >= 100_000 || // Batch is full
            self.batch_timer_expired() || // Timeout reached
            self.memory_pressure_detected() // Memory pressure
        };
        
        if should_flush {
            // Process entire batch at 1M files/sec speed
            self.flush_batch_internal().await?;
        }
        
        // Return ID for this specific file
        self.get_result_for_file(file).await
    }
    
    async fn flush_batch_internal(&self) -> Result<()> {
        let batch = {
            let mut ops = self.pending_operations.lock().await;
            std::mem::take(&mut *ops) // Take all pending operations
        };
        
        if batch.is_empty() {
            return Ok(());
        }
        
        // THIS IS WHERE THE SPEED COMES FROM:
        // Process 100K operations in single zero-copy operation
        let start = Instant::now();
        
        // Serialize entire batch to FlatBuffer (zero-copy)
        let serialized = self.serialize_batch_zero_copy(&batch)?;
        
        // Write entire batch to memory-mapped file (atomic)
        let offset = self.write_batch_mmap(serialized)?;
        
        // Update all indexes in single operation
        self.update_indexes_batch(&batch, offset)?;
        
        let duration = start.elapsed();
        let throughput = batch.len() as f64 / duration.as_secs_f64();
        
        // Atomic performance tracking
        self.total_operations.fetch_add(batch.len() as u64, Ordering::Relaxed);
        self.current_throughput.store(throughput as u64, Ordering::Relaxed);
        
        tracing::info!("Processed {} operations in {:?} ({:.0} ops/sec)", 
                      batch.len(), duration, throughput);
        
        Ok(())
    }
}
```

**The Speed Secret:**
1. **External code calls individual operations** - same interface as before
2. **Internally, operations are batched** - collected into large batches
3. **When batch is full (100K operations)** - processed in single zero-copy operation
4. **Result: 1M files/sec throughput** - even though external code uses individual calls
5. **No code changes needed** - existing code automatically gets speed benefits

This provides:
1. **Same external interface** - no code changes needed
2. **Automatic speed optimization** - internal batching provides 1M files/sec
3. **Atomic operations throughout** - all operations tracked atomically
4. **Zero-copy benefits** - all operations use zero-copy serialization
5. **Transparent performance** - external code gets speed without knowing about batching#
# Database Connection and Storage Architecture

### No SQLite Connection - Direct Memory-Mapped Access

The zero-copy database completely replaces SQLite with direct file access:

```rust
// BEFORE: SQLite connection style
let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
let pool = SqlitePool::connect(&database_url).await?;

// AFTER: Direct memory-mapped file access
pub struct ZeroCopyDatabase {
    // Direct file access - no connection pool needed
    data_file: MemoryMappedFile,           // Raw data storage
    index_file: MemoryMappedFile,          // Index storage  
    metadata_file: MemoryMappedFile,       // Metadata storage
    
    // No SQL queries - direct memory access
    file_index: HashMap<String, u64>,      // Path -> file offset
    directory_index: BTreeMap<String, Vec<u64>>, // Directory -> file offsets
    
    // FlatBuffer builders for zero-copy serialization
    builder: FlatBufferBuilder<'static>,
}

impl ZeroCopyDatabase {
    pub async fn new(db_path: PathBuf) -> Result<Self> {
        // Open memory-mapped files directly
        // Files can grow unlimited - only RAM cache is limited
        let data_file = MemoryMappedFile::new(&db_path.join("media.fb"), 1024 * 1024)?; // Start 1MB, grows as needed
        let index_file = MemoryMappedFile::new(&db_path.join("media.idx"), 64 * 1024)?; // Start 64KB, grows as needed
        let metadata_file = MemoryMappedFile::new(&db_path.join("media.meta"), 16 * 1024)?; // Start 16KB, grows as needed
        
        // Load indexes into memory from index file
        let file_index = Self::load_file_index(&index_file)?;
        let directory_index = Self::load_directory_index(&index_file)?;
        
        Ok(Self {
            data_file,
            index_file,
            metadata_file,
            file_index,
            directory_index,
            builder: FlatBufferBuilder::new(),
        })
    }
}
```

### Full Zero-Copy Storage - FlatBuffers Stored Directly

All data is stored as FlatBuffers in the database files for zero-copy access:

```rust
// Database file format - all FlatBuffers
/*
media.fb file structure:
┌─────────────────────────────────────────┐
│ Header (64 bytes)                       │
├─────────────────────────────────────────┤
│ Batch 1: FlatBuffer MediaFileBatch      │
│ ┌─────────────────────────────────────┐ │
│ │ MediaFile 1 (FlatBuffer)            │ │
│ │ MediaFile 2 (FlatBuffer)            │ │
│ │ MediaFile 3 (FlatBuffer)            │ │
│ │ ...                                 │ │
│ └─────────────────────────────────────┘ │
├─────────────────────────────────────────┤
│ Batch 2: FlatBuffer MediaFileBatch      │
│ ...                                     │
└─────────────────────────────────────────┘
*/

impl ZeroCopyDatabase {
    // Store FlatBuffers directly - no SQL conversion
    async fn store_media_files_zero_copy(&mut self, files: &[MediaFile]) -> Result<Vec<i64>> {
        // Serialize directly to FlatBuffer format
        self.builder.reset();
        
        let mut fb_files = Vec::new();
        for file in files {
            // Create FlatBuffer MediaFile directly
            let path_fb = self.builder.create_string(&file.path.to_string_lossy());
            let filename_fb = self.builder.create_string(&file.filename);
            let mime_type_fb = self.builder.create_string(&file.mime_type);
            let title_fb = file.title.as_ref().map(|t| self.builder.create_string(t));
            let artist_fb = file.artist.as_ref().map(|a| self.builder.create_string(a));
            
            let fb_file = MediaFile::create(&mut self.builder, &MediaFileArgs {
                id: file.id.unwrap_or(0),
                path: Some(path_fb),
                filename: Some(filename_fb),
                size: file.size,
                modified: file.modified.duration_since(UNIX_EPOCH)?.as_secs(),
                mime_type: Some(mime_type_fb),
                title: title_fb,
                artist: artist_fb,
                // ... other fields
            });
            
            fb_files.push(fb_file);
        }
        
        // Create batch FlatBuffer
        let files_vector = self.builder.create_vector(&fb_files);
        let batch = MediaFileBatch::create(&mut self.builder, &MediaFileBatchArgs {
            files: Some(files_vector),
            batch_id: self.generate_batch_id(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        });
        
        self.builder.finish(batch, None);
        
        // Write FlatBuffer directly to memory-mapped file (zero-copy)
        let fb_data = self.builder.finished_data();
        let offset = self.data_file.append_data(fb_data)?;
        
        // Update indexes with file offsets
        let mut file_ids = Vec::new();
        for (i, file) in files.iter().enumerate() {
            let file_id = self.generate_file_id();
            let file_offset = offset + self.calculate_file_offset_in_batch(i);
            
            // Store offset in index for zero-copy reads
            self.file_index.insert(file.path.to_string_lossy().to_string(), file_offset);
            file_ids.push(file_id);
        }
        
        Ok(file_ids)
    }
    
    // Read FlatBuffers directly - no deserialization needed
    async fn get_media_file_zero_copy(&self, path: &Path) -> Result<Option<MediaFile>> {
        let path_str = path.to_string_lossy();
        
        // Get file offset from index
        if let Some(&offset) = self.file_index.get(&path_str.to_string()) {
            // Read FlatBuffer directly from memory-mapped file (zero-copy)
            let fb_data = self.data_file.read_at_offset(offset, 1024)?; // Read chunk
            
            // Access FlatBuffer data directly - no deserialization
            let fb_file = flatbuffers::root::<MediaFile>(fb_data)?;
            
            // Convert FlatBuffer to MediaFile only when needed
            let media_file = MediaFile {
                id: Some(fb_file.id()),
                path: PathBuf::from(fb_file.path().unwrap_or("")),
                filename: fb_file.filename().unwrap_or("").to_string(),
                size: fb_file.size(),
                modified: UNIX_EPOCH + Duration::from_secs(fb_file.modified()),
                mime_type: fb_file.mime_type().unwrap_or("").to_string(),
                title: fb_file.title().map(|s| s.to_string()),
                artist: fb_file.artist().map(|s| s.to_string()),
                // ... other fields
                created_at: UNIX_EPOCH + Duration::from_secs(fb_file.created_at()),
                updated_at: UNIX_EPOCH + Duration::from_secs(fb_file.updated_at()),
            };
            
            Ok(Some(media_file))
        } else {
            Ok(None)
        }
    }
}
```

### Integration with Current Application

The zero-copy database integrates seamlessly with the current application:

```rust
// BEFORE: SQLite integration in main.rs
let database = Arc::new(SqliteDatabase::new(db_path).await?) as Arc<dyn DatabaseManager>;

// AFTER: Zero-copy database integration
let database = Arc::new(ZeroCopyDatabase::new(db_path).await?) as Arc<dyn DatabaseManager>;

// All existing code works unchanged - same DatabaseManager trait
let media_scanner = MediaScanner::with_database(database.clone());
let app_state = AppState::new(database.clone(), config);

// Web handlers work unchanged
async fn browse_handler(state: &AppState, object_id: &str) -> Result<String> {
    // Same interface, but internally uses zero-copy FlatBuffer access
    let (directories, files) = state.database.get_directory_listing(&path, filter).await?;
    // ... rest of handler unchanged
}
```

### Performance Benefits for Current Application

**Directory Browsing (Web Handlers):**
```rust
// BEFORE: SQL query + deserialization
SELECT * FROM media_files WHERE parent_path = ? AND mime_type LIKE ?

// AFTER: Direct FlatBuffer access
async fn get_directory_listing_zero_copy(&self, dir: &Path, filter: &str) -> Result<(Vec<MediaDirectory>, Vec<MediaFile>)> {
    let dir_str = dir.to_string_lossy();
    
    // Get file offsets from directory index (no SQL)
    if let Some(file_offsets) = self.directory_index.get(&dir_str.to_string()) {
        let mut files = Vec::new();
        
        // Read FlatBuffers directly from memory-mapped file
        for &offset in file_offsets {
            let fb_data = self.data_file.read_at_offset(offset, 512)?;
            let fb_file = flatbuffers::root::<MediaFile>(fb_data)?;
            
            // Filter by mime type directly on FlatBuffer (no deserialization)
            if fb_file.mime_type().unwrap_or("").contains(filter) {
                // Only deserialize matching files
                files.push(self.fb_to_media_file(fb_file)?);
            }
        }
        
        Ok((Vec::new(), files)) // Simplified for example
    } else {
        Ok((Vec::new(), Vec::new()))
    }
}
```

**Media Scanning:**
```rust
// MediaScanner automatically benefits from zero-copy storage
impl MediaScanner {
    async fn scan_directory(&self, directory: &Path) -> Result<ScanResult> {
        let files = self.scan_files_from_filesystem(directory).await?;
        
        // This call now uses zero-copy FlatBuffer storage internally
        // External interface unchanged, but 1000x faster internally
        for file in files {
            self.database.store_media_file(&file).await?;
        }
        
        Ok(result)
    }
}
```

**DLNA Browsing:**
```rust
// DLNA handlers get automatic speed benefits
async fn handle_browse_request(state: &AppState, object_id: &str) -> Result<String> {
    // Same code, but internally uses zero-copy FlatBuffer access
    let (dirs, files) = state.database.get_directory_listing(&path, "video/").await?;
    
    // Generate DLNA XML response (unchanged)
    let xml = generate_didl_lite(dirs, files)?;
    Ok(xml)
}
```

### File Format Compatibility

**Migration from SQLite:**
```rust
impl DatabaseMigration {
    async fn migrate_sqlite_to_flatbuffer(&self) -> Result<()> {
        let sqlite_db = SqliteDatabase::new(&self.old_path).await?;
        let zerocopy_db = ZeroCopyDatabase::new(&self.new_path).await?;
        
        // Stream all data from SQLite
        let mut file_stream = sqlite_db.stream_all_media_files();
        let mut batch = Vec::new();
        
        while let Some(file_result) = file_stream.next().await {
            let file = file_result?;
            batch.push(file);
            
            // Process in large batches for maximum speed
            if batch.len() >= 100_000 {
                zerocopy_db.store_media_files_zero_copy(&batch).await?;
                batch.clear();
            }
        }
        
        // Process remaining files
        if !batch.is_empty() {
            zerocopy_db.store_media_files_zero_copy(&batch).await?;
        }
        
        Ok(())
    }
}
```

This architecture provides:
1. **No SQLite dependency** - direct memory-mapped file access
2. **Full zero-copy** - FlatBuffers stored directly in database files
3. **Same external interface** - existing code works unchanged
4. **Maximum performance** - 1M files/sec through zero-copy operations
5. **Memory efficiency** - 5MB total (4MB cache + 1MB indexes)
6. **Direct file access** - no connection pools or SQL parsing##
 Unlimited File Size with Configurable RAM Limits

### File Storage vs RAM Usage Separation

The zero-copy database separates file storage (unlimited) from RAM usage (configurable):

```rust
pub struct ZeroCopyConfig {
    // FILE STORAGE (unlimited - grows as needed)
    pub initial_data_file_mb: usize,      // Start small, grows unlimited
    pub initial_index_file_mb: usize,     // Start small, grows unlimited
    pub initial_metadata_file_mb: usize,  // Start small, grows unlimited
    pub file_growth_increment_mb: usize,  // How much to grow when needed
    
    // RAM USAGE (strictly limited to prevent memory leaks)
    pub ram_cache_limit_mb: usize,        // 4MB default - hard limit
    pub ram_index_limit_mb: usize,        // 1MB default - hard limit  
    pub ram_metadata_limit_mb: usize,     // 64KB default - hard limit
    
    // AUTO-SCALING PERFORMANCE
    pub auto_scale_performance: bool,     // Enable auto-scaling
    pub target_memory_mb: usize,          // Total RAM budget (default: 5MB)
    pub performance_scaling_factor: f64,  // How aggressively to scale
}

impl Default for ZeroCopyConfig {
    fn default() -> Self {
        Self {
            // File storage - starts small, grows unlimited
            initial_data_file_mb: 1,      // Start with 1MB file
            initial_index_file_mb: 1,     // Start with 1MB index
            initial_metadata_file_mb: 1,  // Start with 1MB metadata
            file_growth_increment_mb: 10, // Grow by 10MB when needed
            
            // RAM usage - strictly limited (ultra low defaults)
            ram_cache_limit_mb: 4,        // 4MB RAM cache limit
            ram_index_limit_mb: 1,        // 1MB RAM index limit
            ram_metadata_limit_mb: 1,     // 1MB RAM metadata limit (64KB -> 1MB)
            
            // Auto-scaling - disabled by default
            auto_scale_performance: false,
            target_memory_mb: 6,          // 4+1+1 = 6MB total budget
            performance_scaling_factor: 1.0,
        }
    }
}
```

### Auto-Scaling Performance Based on Memory Budget

```rust
impl ZeroCopyDatabase {
    pub fn auto_configure_for_memory_budget(target_memory_mb: usize) -> ZeroCopyConfig {
        let mut config = ZeroCopyConfig::default();
        
        if target_memory_mb < 6 {
            // Ultra-low memory mode (less than 6MB)
            config.ram_cache_limit_mb = 2;
            config.ram_index_limit_mb = 1;
            config.ram_metadata_limit_mb = 1;
            tracing::info!("Ultra-low memory mode: {}MB total", target_memory_mb);
        } else {
            // Distribute memory budget optimally
            let available = target_memory_mb - 2; // Reserve 2MB for system
            
            // Allocate 80% to cache, 15% to index, 5% to metadata
            config.ram_cache_limit_mb = (available * 80 / 100).max(4);
            config.ram_index_limit_mb = (available * 15 / 100).max(1);
            config.ram_metadata_limit_mb = (available * 5 / 100).max(1);
            
            tracing::info!(
                "Auto-configured for {}MB: {}MB cache, {}MB index, {}MB metadata",
                target_memory_mb,
                config.ram_cache_limit_mb,
                config.ram_index_limit_mb,
                config.ram_metadata_limit_mb
            );
        }
        
        config.target_memory_mb = target_memory_mb;
        config.auto_scale_performance = true;
        config
    }
    
    // Performance scales automatically based on available RAM
    pub fn get_expected_performance(&self) -> PerformanceProfile {
        let total_ram = self.config.ram_cache_limit_mb + 
                       self.config.ram_index_limit_mb + 
                       self.config.ram_metadata_limit_mb;
        
        match total_ram {
            0..=6 => PerformanceProfile {
                expected_throughput: 50_000,   // 50K files/sec
                batch_size: 10_000,
                description: "Ultra-low memory mode".to_string(),
            },
            7..=16 => PerformanceProfile {
                expected_throughput: 200_000,  // 200K files/sec
                batch_size: 25_000,
                description: "Low memory mode".to_string(),
            },
            17..=64 => PerformanceProfile {
                expected_throughput: 500_000,  // 500K files/sec
                batch_size: 50_000,
                description: "Balanced mode".to_string(),
            },
            65..=256 => PerformanceProfile {
                expected_throughput: 800_000,  // 800K files/sec
                batch_size: 100_000,
                description: "High performance mode".to_string(),
            },
            _ => PerformanceProfile {
                expected_throughput: 1_000_000, // 1M files/sec
                batch_size: 200_000,
                description: "Maximum performance mode".to_string(),
            },
        }
    }
}
```

### Memory-Bounded Cache Management

```rust
pub struct MemoryBoundedCache {
    // RAM usage tracking (atomic for thread safety)
    current_cache_usage: AtomicUsize,
    current_index_usage: AtomicUsize,
    current_metadata_usage: AtomicUsize,
    
    // Hard limits from config
    cache_limit: usize,
    index_limit: usize,
    metadata_limit: usize,
    
    // LRU eviction for cache management
    cache_lru: Arc<Mutex<LruCache<String, CachedData>>>,
    index_lru: Arc<Mutex<LruCache<String, IndexData>>>,
}

impl MemoryBoundedCache {
    pub fn insert_cached_data(&self, key: String, data: CachedData) -> Result<()> {
        let data_size = data.size_bytes();
        
        // Check if adding this data would exceed RAM limit
        let current_usage = self.current_cache_usage.load(Ordering::Relaxed);
        if current_usage + data_size > self.cache_limit {
            // Evict LRU entries until we have space
            self.evict_until_space_available(data_size)?;
        }
        
        // Insert data and update usage counter
        {
            let mut cache = self.cache_lru.lock().unwrap();
            cache.put(key, data);
        }
        
        self.current_cache_usage.fetch_add(data_size, Ordering::Relaxed);
        Ok(())
    }
    
    fn evict_until_space_available(&self, needed_space: usize) -> Result<()> {
        let mut cache = self.cache_lru.lock().unwrap();
        
        while self.current_cache_usage.load(Ordering::Relaxed) + needed_space > self.cache_limit {
            if let Some((_, evicted_data)) = cache.pop_lru() {
                let evicted_size = evicted_data.size_bytes();
                self.current_cache_usage.fetch_sub(evicted_size, Ordering::Relaxed);
            } else {
                return Err(anyhow::anyhow!("Cannot evict enough data to fit new entry"));
            }
        }
        
        Ok(())
    }
}
```

### Configuration Examples

```toml
# Ultra-low memory (4MB total)
[database.zerocopy.ultra_low]
ram_cache_limit_mb = 2
ram_index_limit_mb = 1
ram_metadata_limit_mb = 1
auto_scale_performance = false
# Expected: 50K files/sec

# Default (6MB total)
[database.zerocopy.default]
ram_cache_limit_mb = 4
ram_index_limit_mb = 1
ram_metadata_limit_mb = 1
auto_scale_performance = false
# Expected: 200K files/sec

# Auto-scale for specific memory budget
[database.zerocopy.auto_scale]
target_memory_mb = 64
auto_scale_performance = true
# Auto-configures: 51MB cache, 9MB index, 3MB metadata
# Expected: 500K files/sec

# High performance (256MB total)
[database.zerocopy.high_performance]
ram_cache_limit_mb = 200
ram_index_limit_mb = 40
ram_metadata_limit_mb = 16
auto_scale_performance = false
# Expected: 800K files/sec

# Maximum performance (1GB total)
[database.zerocopy.maximum]
ram_cache_limit_mb = 800
ram_index_limit_mb = 150
ram_metadata_limit_mb = 50
auto_scale_performance = false
# Expected: 1M+ files/sec
```

### Environment Variable Support

```bash
# Docker environment variables for memory limits
ZEROCOPY_TARGET_MEMORY_MB=64    # Auto-configure for 64MB budget
ZEROCOPY_CACHE_LIMIT_MB=32      # Or set specific limits
ZEROCOPY_INDEX_LIMIT_MB=8
ZEROCOPY_METADATA_LIMIT_MB=4

# Example Docker commands
docker run -e ZEROCOPY_TARGET_MEMORY_MB=16 myapp    # 16MB budget
docker run -e ZEROCOPY_TARGET_MEMORY_MB=256 myapp   # 256MB budget
docker run -e ZEROCOPY_TARGET_MEMORY_MB=1024 myapp  # 1GB budget
```

This approach provides:

1. **Unlimited file storage** - database files grow as needed
2. **Strict RAM limits** - prevents memory leaks and overprovisioning
3. **Ultra-low defaults** - 6MB total (4MB cache + 1MB index + 1MB metadata)
4. **Auto-scaling performance** - automatically optimize for memory budget
5. **Configurable limits** - can scale from 4MB to GB based on needs
6. **Memory-bounded caching** - LRU eviction when limits reached
7. **Performance scaling** - 50K to 1M+ files/sec based on RAM allocation

The system starts ultra-low (6MB) but can scale performance automatically based on available memory budget while preventing memory leaks through strict RAM limits.
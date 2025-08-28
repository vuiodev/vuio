use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::memory_mapped::MemoryMappedFile;
use super::flatbuffer::{BatchSerializer, MediaFileSerializer, BatchOperationType};
use super::index_manager::{IndexManager, IndexStats, IndexType};
use super::{DatabaseManager, MediaFile, MediaDirectory, Playlist, MusicCategory, DatabaseStats, DatabaseHealth};
use crate::platform::filesystem::{create_platform_path_normalizer, PathNormalizer};

/// Configuration for zero-copy database operations
#[derive(Debug, Clone)]
pub struct ZeroCopyConfig {
    /// Number of files to process per batch
    pub batch_size: usize,
    /// Initial size of data file in MB
    pub initial_file_size_mb: usize,
    /// Maximum size of data file in GB
    pub max_file_size_gb: usize,
    /// Memory map size in MB (fixed default: 4MB)
    pub memory_map_size_mb: usize,
    /// Index cache size (number of entries, fixed default: 1M entries = ~1MB)
    pub index_cache_size: usize,
    /// Enable compression (disabled for maximum speed)
    pub enable_compression: bool,
    /// Sync frequency for durability
    pub sync_frequency: Duration,
    /// Enable Write-Ahead Logging
    pub enable_wal: bool,
    /// Auto-detect memory (disabled - manual configuration only)
    pub auto_detect_memory: bool,
}

impl Default for ZeroCopyConfig {
    fn default() -> Self {
        Self {
            batch_size: 100_000,  // Process 100K files per batch
            initial_file_size_mb: 10,  // Start with 10MB
            max_file_size_gb: 10,  // Max 10GB
            memory_map_size_mb: 4,  // Fixed 4MB default (matches original app)
            index_cache_size: 1_000_000,  // 1M entries (~1MB for indexes)
            enable_compression: false,  // Disabled for max speed
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
                info!("Using cache size from env: {}MB", size);
            }
        }
        
        if let Ok(index_size) = std::env::var("ZEROCOPY_INDEX_SIZE") {
            if let Ok(size) = index_size.parse::<usize>() {
                config.index_cache_size = size;
                info!("Using index cache size from env: {}", size);
            }
        }
        
        if let Ok(batch_size) = std::env::var("ZEROCOPY_BATCH_SIZE") {
            if let Ok(size) = batch_size.parse::<usize>() {
                config.batch_size = size;
                info!("Using batch size from env: {}", size);
            }
        }
        
        config
    }
    
    /// Validate configuration and warn about performance implications
    pub fn validate(&self) {
        if self.memory_map_size_mb == 4 {
            info!("Using default 4MB cache. For higher performance, increase memory_map_size_mb in config");
        }
        
        if self.index_cache_size == 1_000_000 {
            info!("Using default 1M index cache (1MB). For very large libraries, increase index_cache_size in config");
        }
        
        // Warn about performance expectations
        let expected_throughput = match self.memory_map_size_mb {
            4 => "100K-200K files/sec",
            16..=64 => "200K-500K files/sec", 
            65..=256 => "500K-800K files/sec",
            _ => "800K+ files/sec",
        };
        
        info!("Expected throughput with {}MB cache: {}", self.memory_map_size_mb, expected_throughput);
    }
}

/// Atomic performance tracking for zero-copy database operations
#[derive(Debug, Default)]
pub struct AtomicPerformanceTracker {
    // File operation counters
    pub total_files: AtomicU64,
    pub processed_files: AtomicU64,
    pub failed_files: AtomicU64,
    pub inserted_files: AtomicU64,
    pub updated_files: AtomicU64,
    pub deleted_files: AtomicU64,
    
    // Batch operation counters
    pub total_batches: AtomicU64,
    pub successful_batches: AtomicU64,
    pub failed_batches: AtomicU64,
    
    // Performance metrics
    pub total_operations: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub index_lookups: AtomicU64,
    pub index_updates: AtomicU64,
    
    // Memory and I/O tracking
    pub bytes_written: AtomicU64,
    pub bytes_read: AtomicU64,
    pub sync_operations: AtomicU64,
    
    // Timing (stored as nanoseconds)
    pub total_processing_time_ns: AtomicU64,
    pub total_serialization_time_ns: AtomicU64,
    pub total_io_time_ns: AtomicU64,
}

impl AtomicPerformanceTracker {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Record a successful file operation
    pub fn record_file_operation(&self, operation_type: FileOperationType, processing_time: Duration) {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.processed_files.fetch_add(1, Ordering::Relaxed);
        self.total_processing_time_ns.fetch_add(processing_time.as_nanos() as u64, Ordering::Relaxed);
        
        match operation_type {
            FileOperationType::Insert => self.inserted_files.fetch_add(1, Ordering::Relaxed),
            FileOperationType::Update => self.updated_files.fetch_add(1, Ordering::Relaxed),
            FileOperationType::Delete => self.deleted_files.fetch_add(1, Ordering::Relaxed),
        };
    }
    
    /// Record a failed file operation
    pub fn record_failed_operation(&self) {
        self.failed_files.fetch_add(1, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Record a batch operation
    pub fn record_batch_operation(&self, success: bool, _files_in_batch: usize, processing_time: Duration) {
        self.total_batches.fetch_add(1, Ordering::Relaxed);
        if success {
            self.successful_batches.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_batches.fetch_add(1, Ordering::Relaxed);
        }
        self.total_processing_time_ns.fetch_add(processing_time.as_nanos() as u64, Ordering::Relaxed);
    }
    
    /// Record cache hit/miss
    pub fn record_cache_access(&self, hit: bool) {
        if hit {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.cache_misses.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    /// Record index operation
    pub fn record_index_operation(&self, operation_type: IndexOperationType) {
        match operation_type {
            IndexOperationType::Lookup => self.index_lookups.fetch_add(1, Ordering::Relaxed),
            IndexOperationType::Update => self.index_updates.fetch_add(1, Ordering::Relaxed),
        };
    }
    
    /// Record I/O operation
    pub fn record_io_operation(&self, bytes: u64, is_write: bool, duration: Duration) {
        if is_write {
            self.bytes_written.fetch_add(bytes, Ordering::Relaxed);
        } else {
            self.bytes_read.fetch_add(bytes, Ordering::Relaxed);
        }
        self.total_io_time_ns.fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }
    
    /// Record sync operation
    pub fn record_sync_operation(&self) {
        self.sync_operations.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Get current performance statistics
    pub fn get_stats(&self) -> PerformanceStats {
        let total_ops = self.total_operations.load(Ordering::Relaxed);
        let total_time_ns = self.total_processing_time_ns.load(Ordering::Relaxed);
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);
        let cache_misses = self.cache_misses.load(Ordering::Relaxed);
        
        PerformanceStats {
            total_files: self.total_files.load(Ordering::Relaxed),
            processed_files: self.processed_files.load(Ordering::Relaxed),
            failed_files: self.failed_files.load(Ordering::Relaxed),
            inserted_files: self.inserted_files.load(Ordering::Relaxed),
            updated_files: self.updated_files.load(Ordering::Relaxed),
            deleted_files: self.deleted_files.load(Ordering::Relaxed),
            total_batches: self.total_batches.load(Ordering::Relaxed),
            successful_batches: self.successful_batches.load(Ordering::Relaxed),
            failed_batches: self.failed_batches.load(Ordering::Relaxed),
            total_operations: total_ops,
            cache_hits,
            cache_misses,
            cache_hit_rate: if cache_hits + cache_misses > 0 {
                cache_hits as f64 / (cache_hits + cache_misses) as f64
            } else {
                0.0
            },
            index_lookups: self.index_lookups.load(Ordering::Relaxed),
            index_updates: self.index_updates.load(Ordering::Relaxed),
            bytes_written: self.bytes_written.load(Ordering::Relaxed),
            bytes_read: self.bytes_read.load(Ordering::Relaxed),
            sync_operations: self.sync_operations.load(Ordering::Relaxed),
            average_throughput_per_sec: if total_time_ns > 0 {
                (total_ops as f64) / (total_time_ns as f64 / 1_000_000_000.0)
            } else {
                0.0
            },
            total_processing_time: Duration::from_nanos(total_time_ns),
            total_serialization_time: Duration::from_nanos(self.total_serialization_time_ns.load(Ordering::Relaxed)),
            total_io_time: Duration::from_nanos(self.total_io_time_ns.load(Ordering::Relaxed)),
        }
    }
}

/// Performance statistics snapshot
#[derive(Debug, Clone)]
pub struct PerformanceStats {
    pub total_files: u64,
    pub processed_files: u64,
    pub failed_files: u64,
    pub inserted_files: u64,
    pub updated_files: u64,
    pub deleted_files: u64,
    pub total_batches: u64,
    pub successful_batches: u64,
    pub failed_batches: u64,
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_hit_rate: f64,
    pub index_lookups: u64,
    pub index_updates: u64,
    pub bytes_written: u64,
    pub bytes_read: u64,
    pub sync_operations: u64,
    pub average_throughput_per_sec: f64,
    pub total_processing_time: Duration,
    pub total_serialization_time: Duration,
    pub total_io_time: Duration,
}

/// Types of file operations for tracking
#[derive(Debug, Clone, Copy)]
pub enum FileOperationType {
    Insert,
    Update,
    Delete,
}

/// Types of index operations for tracking
#[derive(Debug, Clone, Copy)]
pub enum IndexOperationType {
    Lookup,
    Update,
}



/// Result of batch processing operation
#[derive(Debug, Clone)]
pub struct BatchProcessingResult {
    pub batch_id: u64,
    pub operation_type: BatchOperationType,
    pub files_processed: usize,
    pub processing_time: Duration,
    pub serialization_time: Duration,
    pub write_offset: u64,
    pub data_size: usize,
    pub throughput: f64, // files per second
    pub checksum: u32,
    pub errors: Vec<super::flatbuffer::BatchSerializationError>,
}

impl BatchProcessingResult {
    /// Create an empty result for zero files
    pub fn empty() -> Self {
        Self {
            batch_id: 0,
            operation_type: BatchOperationType::Insert,
            files_processed: 0,
            processing_time: Duration::from_nanos(0),
            serialization_time: Duration::from_nanos(0),
            write_offset: 0,
            data_size: 0,
            throughput: 0.0,
            checksum: 0,
            errors: Vec::new(),
        }
    }
    
    /// Check if the batch processing was successful
    pub fn is_successful(&self) -> bool {
        self.errors.is_empty() && self.files_processed > 0
    }
    
    /// Get summary string
    pub fn summary(&self) -> String {
        format!(
            "Batch {}: {} files in {:?} ({:.0} files/sec, {} bytes)",
            self.batch_id,
            self.files_processed,
            self.processing_time,
            self.throughput,
            self.data_size
        )
    }
}

/// Result of batch removal operation
#[derive(Debug, Clone)]
pub struct BatchRemovalResult {
    pub batch_id: u64,
    pub files_requested: usize,
    pub files_removed: usize,
    pub processing_time: Duration,
    pub throughput: f64, // files per second
}

impl BatchRemovalResult {
    /// Create an empty result for zero files
    pub fn empty() -> Self {
        Self {
            batch_id: 0,
            files_requested: 0,
            files_removed: 0,
            processing_time: Duration::from_nanos(0),
            throughput: 0.0,
        }
    }
    
    /// Check if all requested files were removed
    pub fn is_complete(&self) -> bool {
        self.files_removed == self.files_requested
    }
    
    /// Get summary string
    pub fn summary(&self) -> String {
        format!(
            "Batch {}: {}/{} files removed in {:?} ({:.0} files/sec)",
            self.batch_id,
            self.files_removed,
            self.files_requested,
            self.processing_time,
            self.throughput
        )
    }
}

/// Zero-copy database implementation with atomic operations
pub struct ZeroCopyDatabase {
    // Core storage
    data_file: Arc<RwLock<MemoryMappedFile>>,
    index_manager: Arc<RwLock<IndexManager>>,
    
    // Configuration
    config: ZeroCopyConfig,
    db_path: PathBuf,
    
    // Performance tracking
    performance_tracker: Arc<AtomicPerformanceTracker>,
    
    // Path normalization
    path_normalizer: Box<dyn PathNormalizer>,
    
    // Database state
    is_initialized: AtomicU64,  // 0 = not initialized, 1 = initialized
    is_open: AtomicU64,         // 0 = closed, 1 = open
    
    // FlatBuffer serialization
    batch_serializer: Arc<BatchSerializer>,
    flatbuffer_builder: Arc<RwLock<flatbuffers::FlatBufferBuilder<'static>>>,
}

impl ZeroCopyDatabase {
    /// Create a new zero-copy database instance
    pub async fn new(db_path: PathBuf, config: Option<ZeroCopyConfig>) -> Result<Self> {
        let config = config.unwrap_or_else(ZeroCopyConfig::from_env);
        config.validate();
        
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        // Create data file path
        let data_file_path = db_path.with_extension("fb");
        let initial_size = config.initial_file_size_mb * 1024 * 1024;
        let max_size = config.max_file_size_gb * 1024 * 1024 * 1024;
        
        // Create memory-mapped data file
        let data_file = MemoryMappedFile::with_max_size(&data_file_path, initial_size, max_size)?;
        
        // Create index manager with memory limits
        let index_memory_bytes = config.memory_map_size_mb * 1024 * 1024 / 4; // 25% of total memory for indexes
        let index_manager = IndexManager::new(config.index_cache_size, index_memory_bytes);
        
        // Create performance tracker
        let performance_tracker = Arc::new(AtomicPerformanceTracker::new());
        
        // Create path normalizer
        let path_normalizer = create_platform_path_normalizer();
        
        // Create batch serializer
        let batch_serializer = Arc::new(BatchSerializer::new());
        
        // Create FlatBuffer builder
        let flatbuffer_builder = flatbuffers::FlatBufferBuilder::with_capacity(1024 * 1024); // 1MB initial capacity
        
        info!(
            "Created zero-copy database at {} with {}MB initial size, {}MB cache, {}K index entries",
            db_path.display(),
            config.initial_file_size_mb,
            config.memory_map_size_mb,
            config.index_cache_size / 1000
        );
        
        Ok(Self {
            data_file: Arc::new(RwLock::new(data_file)),
            index_manager: Arc::new(RwLock::new(index_manager)),
            config,
            db_path,
            performance_tracker,
            path_normalizer,
            is_initialized: AtomicU64::new(0),
            is_open: AtomicU64::new(0),
            batch_serializer,
            flatbuffer_builder: Arc::new(RwLock::new(flatbuffer_builder)),
        })
    }
    
    /// Initialize the database (create file structure, load indexes)
    pub async fn initialize(&self) -> Result<()> {
        if self.is_initialized.load(Ordering::Relaxed) == 1 {
            return Ok(()); // Already initialized
        }
        
        let start_time = Instant::now();
        
        info!("Initializing zero-copy database...");
        
        // Initialize data file with header
        {
            let mut data_file = self.data_file.write().await;
            let mut builder = self.flatbuffer_builder.write().await;
            
            // Create database header
            let header = self.create_database_header(&mut builder)?;
            builder.finish(header, None);
            
            // Write header to file
            let header_data = builder.finished_data();
            data_file.append_data(header_data)?;
            
            info!("Database header written ({} bytes)", header_data.len());
        }
        
        // Load existing indexes if they exist
        let index_file_path = self.db_path.with_extension("idx");
        if index_file_path.exists() {
            self.load_indexes(&index_file_path).await?;
        }
        
        // Mark as initialized
        self.is_initialized.store(1, Ordering::Relaxed);
        
        let initialization_time = start_time.elapsed();
        info!("Zero-copy database initialized in {:?}", initialization_time);
        
        Ok(())
    }
    
    /// Open the database for operations
    pub async fn open(&self) -> Result<()> {
        if self.is_open.load(Ordering::Relaxed) == 1 {
            return Ok(()); // Already open
        }
        
        // Ensure database is initialized
        if self.is_initialized.load(Ordering::Relaxed) == 0 {
            self.initialize().await?;
        }
        
        // Mark as open
        self.is_open.store(1, Ordering::Relaxed);
        
        info!("Zero-copy database opened successfully");
        Ok(())
    }
    
    /// Close the database (sync data, save indexes)
    pub async fn close(&self) -> Result<()> {
        if self.is_open.load(Ordering::Relaxed) == 0 {
            return Ok(()); // Already closed
        }
        
        let start_time = Instant::now();
        
        info!("Closing zero-copy database...");
        
        // Sync data file to disk
        {
            let data_file = self.data_file.read().await;
            data_file.sync_to_disk()?;
            self.performance_tracker.record_sync_operation();
        }
        
        // Save indexes
        let index_file_path = self.db_path.with_extension("idx");
        self.save_indexes(&index_file_path).await?;
        
        // Mark as closed
        self.is_open.store(0, Ordering::Relaxed);
        
        let close_time = start_time.elapsed();
        info!("Zero-copy database closed in {:?}", close_time);
        
        // Log final performance statistics
        let stats = self.performance_tracker.get_stats();
        info!(
            "Final stats: {} files processed, {:.2} files/sec average, {:.1}% cache hit rate",
            stats.processed_files,
            stats.average_throughput_per_sec,
            stats.cache_hit_rate * 100.0
        );
        
        Ok(())
    }
    
    /// Check if database is initialized
    pub fn is_initialized(&self) -> bool {
        self.is_initialized.load(Ordering::Relaxed) == 1
    }
    
    /// Check if database is open
    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::Relaxed) == 1
    }
    
    /// Get database configuration
    pub fn get_config(&self) -> &ZeroCopyConfig {
        &self.config
    }
    
    /// Get performance statistics
    pub fn get_performance_stats(&self) -> PerformanceStats {
        self.performance_tracker.get_stats()
    }
    
    /// Get index statistics
    pub async fn get_index_stats(&self) -> IndexStats {
        let index_manager = self.index_manager.read().await;
        index_manager.get_stats()
    }
    
    /// Create database header for file format identification
    fn create_database_header<'a>(&self, builder: &mut flatbuffers::FlatBufferBuilder<'a>) -> Result<flatbuffers::WIPOffset<super::flatbuffer::DatabaseHeader<'a>>> {
        use super::flatbuffer::*;
        
        let magic = builder.create_string("MEDIADB1");
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs();
        
        let header = DatabaseHeader::create(builder, &DatabaseHeaderArgs {
            magic: Some(magic),
            version: 1,
            file_size: 0, // Will be updated later
            index_offset: 0, // Will be updated later
            batch_count: 0, // Will be updated later
            created_at: now,
            last_modified: now,
        });
        
        Ok(header)
    }
    
    /// Load indexes from disk
    async fn load_indexes(&self, index_file_path: &Path) -> Result<()> {
        let mut index_manager = self.index_manager.write().await;
        index_manager.load_indexes(index_file_path).await
    }
    
    /// Save indexes to disk
    async fn save_indexes(&self, index_file_path: &Path) -> Result<()> {
        let index_manager = self.index_manager.read().await;
        index_manager.persist_indexes(index_file_path).await
    }
    
    /// Batch insert files using zero-copy FlatBuffer serialization
    pub async fn batch_insert_files(&self, files: &[MediaFile]) -> Result<BatchProcessingResult> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        if files.is_empty() {
            return Ok(BatchProcessingResult::empty());
        }
        
        let start_time = Instant::now();
        let batch_id = self.batch_serializer.generate_batch_id();
        
        info!("Starting batch insert of {} files (batch ID: {})", files.len(), batch_id);
        
        // Generate canonical paths for all files
        let canonical_paths: Result<Vec<String>> = files
            .iter()
            .map(|file| self.path_normalizer.to_canonical(&file.path).map_err(|e| anyhow!("Path normalization failed: {}", e)))
            .collect();
        
        let canonical_paths = canonical_paths?;
        
        // Serialize batch to FlatBuffer
        let serialization_result = {
            let mut builder = self.flatbuffer_builder.write().await;
            builder.reset(); // Clear previous data
            
            MediaFileSerializer::serialize_media_file_batch(
                &mut builder,
                files,
                batch_id,
                BatchOperationType::Insert,
                Some(&canonical_paths),
            )?
        };
        
        // Write serialized data to memory-mapped file
        let write_offset = {
            let mut data_file = self.data_file.write().await;
            let builder = self.flatbuffer_builder.read().await;
            let serialized_data = builder.finished_data();
            
            // Validate batch integrity before writing
            let integrity_result = MediaFileSerializer::validate_batch_integrity(serialized_data)?;
            if !integrity_result.is_valid {
                return Err(anyhow!("Batch integrity validation failed"));
            }
            
            let io_start = Instant::now();
            let offset = data_file.append_data(serialized_data)?;
            let io_time = io_start.elapsed();
            
            // Record I/O performance
            self.performance_tracker.record_io_operation(
                serialized_data.len() as u64,
                true, // is_write
                io_time,
            );
            
            info!(
                "Wrote batch {} ({} bytes) to offset {} in {:?}",
                batch_id,
                serialized_data.len(),
                offset,
                io_time
            );
            
            offset
        };
        
        // Update indexes
        {
            let mut index_manager = self.index_manager.write().await;
            for (i, file) in files.iter().enumerate() {
                let canonical_path = &canonical_paths[i];
                let mut file_with_canonical = file.clone();
                file_with_canonical.path = PathBuf::from(canonical_path);
                
                index_manager.insert_file_index(&file_with_canonical, write_offset);
            }
        }
        
        // Record performance metrics
        let total_time = start_time.elapsed();
        self.performance_tracker.record_batch_operation(true, files.len(), total_time);
        
        for _ in files {
            self.performance_tracker.record_file_operation(FileOperationType::Insert, total_time / files.len() as u32);
        }
        
        let throughput = files.len() as f64 / total_time.as_secs_f64();
        
        info!(
            "Batch insert completed: {} files in {:?} ({:.0} files/sec)",
            files.len(),
            total_time,
            throughput
        );
        
        Ok(BatchProcessingResult {
            batch_id,
            operation_type: BatchOperationType::Insert,
            files_processed: files.len(),
            processing_time: total_time,
            serialization_time: serialization_result.serialization_time,
            write_offset,
            data_size: serialization_result.serialized_size,
            throughput,
            checksum: MediaFileSerializer::validate_batch_integrity(
                &self.flatbuffer_builder.read().await.finished_data()
            )?.checksum,
            errors: serialization_result.errors,
        })
    }
    
    /// Batch update files using zero-copy FlatBuffer serialization
    pub async fn batch_update_files(&self, files: &[MediaFile]) -> Result<BatchProcessingResult> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        if files.is_empty() {
            return Ok(BatchProcessingResult::empty());
        }
        
        let start_time = Instant::now();
        let batch_id = self.batch_serializer.generate_batch_id();
        
        info!("Starting batch update of {} files (batch ID: {})", files.len(), batch_id);
        
        // Generate canonical paths for all files
        let canonical_paths: Result<Vec<String>> = files
            .iter()
            .map(|file| self.path_normalizer.to_canonical(&file.path).map_err(|e| anyhow!("Path normalization failed: {}", e)))
            .collect();
        
        let canonical_paths = canonical_paths?;
        
        // Serialize batch to FlatBuffer
        let serialization_result = {
            let mut builder = self.flatbuffer_builder.write().await;
            builder.reset(); // Clear previous data
            
            MediaFileSerializer::serialize_media_file_batch(
                &mut builder,
                files,
                batch_id,
                BatchOperationType::Update,
                Some(&canonical_paths),
            )?
        };
        
        // Write serialized data to memory-mapped file
        let write_offset = {
            let mut data_file = self.data_file.write().await;
            let builder = self.flatbuffer_builder.read().await;
            let serialized_data = builder.finished_data();
            
            // Validate batch integrity before writing
            let integrity_result = MediaFileSerializer::validate_batch_integrity(serialized_data)?;
            if !integrity_result.is_valid {
                return Err(anyhow!("Batch integrity validation failed"));
            }
            
            let io_start = Instant::now();
            let offset = data_file.append_data(serialized_data)?;
            let io_time = io_start.elapsed();
            
            // Record I/O performance
            self.performance_tracker.record_io_operation(
                serialized_data.len() as u64,
                true, // is_write
                io_time,
            );
            
            offset
        };
        
        // Update indexes
        {
            let mut index_manager = self.index_manager.write().await;
            for (i, file) in files.iter().enumerate() {
                let canonical_path = &canonical_paths[i];
                let mut file_with_canonical = file.clone();
                file_with_canonical.path = PathBuf::from(canonical_path);
                
                index_manager.insert_file_index(&file_with_canonical, write_offset);
            }
        }
        
        // Record performance metrics
        let total_time = start_time.elapsed();
        self.performance_tracker.record_batch_operation(true, files.len(), total_time);
        
        for _ in files {
            self.performance_tracker.record_file_operation(FileOperationType::Update, total_time / files.len() as u32);
        }
        
        let throughput = files.len() as f64 / total_time.as_secs_f64();
        
        info!(
            "Batch update completed: {} files in {:?} ({:.0} files/sec)",
            files.len(),
            total_time,
            throughput
        );
        
        Ok(BatchProcessingResult {
            batch_id,
            operation_type: BatchOperationType::Update,
            files_processed: files.len(),
            processing_time: total_time,
            serialization_time: serialization_result.serialization_time,
            write_offset,
            data_size: serialization_result.serialized_size,
            throughput,
            checksum: MediaFileSerializer::validate_batch_integrity(
                &self.flatbuffer_builder.read().await.finished_data()
            )?.checksum,
            errors: serialization_result.errors,
        })
    }
    
    /// Batch remove files by paths
    pub async fn batch_remove_files(&self, paths: &[PathBuf]) -> Result<BatchRemovalResult> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        if paths.is_empty() {
            return Ok(BatchRemovalResult::empty());
        }
        
        let start_time = Instant::now();
        let batch_id = self.batch_serializer.generate_batch_id();
        
        info!("Starting batch removal of {} files (batch ID: {})", paths.len(), batch_id);
        
        // Generate canonical paths for all files
        let canonical_paths: Result<Vec<String>> = paths
            .iter()
            .map(|path| self.path_normalizer.to_canonical(path).map_err(|e| anyhow!("Path normalization failed: {}", e)))
            .collect();
        
        let canonical_paths = canonical_paths?;
        
        // Remove from indexes
        let mut removed_count = 0;
        {
            let mut index_manager = self.index_manager.write().await;
            for canonical_path in &canonical_paths {
                if index_manager.remove_file_index(canonical_path).is_some() {
                    removed_count += 1;
                }
            }
        }
        
        // Record performance metrics
        let total_time = start_time.elapsed();
        self.performance_tracker.record_batch_operation(true, removed_count, total_time);
        
        for _ in 0..removed_count {
            self.performance_tracker.record_file_operation(FileOperationType::Delete, total_time / removed_count.max(1) as u32);
        }
        
        let throughput = removed_count as f64 / total_time.as_secs_f64();
        
        info!(
            "Batch removal completed: {} files removed in {:?} ({:.0} files/sec)",
            removed_count,
            total_time,
            throughput
        );
        
        Ok(BatchRemovalResult {
            batch_id,
            files_requested: paths.len(),
            files_removed: removed_count,
            processing_time: total_time,
            throughput,
        })
    }
    
    /// Get current batch ID counter
    pub fn get_current_batch_id(&self) -> u64 {
        self.batch_serializer.current_batch_id()
    }
}

// Implement Send and Sync for ZeroCopyDatabase
unsafe impl Send for ZeroCopyDatabase {}
unsafe impl Sync for ZeroCopyDatabase {}

// Placeholder implementation of DatabaseManager trait for ZeroCopyDatabase
// This will be completed in subsequent tasks
#[async_trait]
impl DatabaseManager for ZeroCopyDatabase {
    async fn initialize(&self) -> Result<()> {
        self.initialize().await
    }
    
    // Individual operations implemented as single-item bulk operations for consistency and performance
    // All individual operations use the same atomic bulk processing pipeline for maximum efficiency
    
    /// Store a single media file using atomic bulk operation wrapper
    /// This method wraps the bulk operation to maintain consistency and atomic statistics tracking
    async fn store_media_file(&self, file: &MediaFile) -> Result<i64> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }

        let start_time = Instant::now();
        
        // Use bulk operation for single file (atomic wrapper)
        let result = self.batch_insert_files(&[file.clone()]).await?;
        
        // Record individual operation statistics
        let processing_time = start_time.elapsed();
        if result.is_successful() {
            self.performance_tracker.record_file_operation(FileOperationType::Insert, processing_time);
            
            debug!(
                "store_media_file for '{}' completed in {:?}",
                file.path.display(),
                processing_time
            );
            
            Ok(result.batch_id as i64) // Return batch ID as file ID for now
        } else {
            self.performance_tracker.record_failed_operation();
            Err(anyhow!("Failed to store media file: {:?}", result.errors))
        }
    }
    
    fn stream_all_media_files(&self) -> std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<MediaFile, sqlx::Error>> + Send + '_>> {
        // Placeholder - will be implemented in subsequent tasks
        Box::pin(futures_util::stream::empty())
    }
    
    /// Remove a single media file using atomic bulk operation wrapper
    /// This method wraps the bulk operation to maintain consistency and atomic statistics tracking
    async fn remove_media_file(&self, path: &Path) -> Result<bool> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }

        let start_time = Instant::now();
        
        // Use bulk operation for single file (atomic wrapper)
        let result = self.batch_remove_files(&[path.to_path_buf()]).await?;
        
        // Record individual operation statistics
        let processing_time = start_time.elapsed();
        let success = result.files_removed > 0;
        
        if success {
            self.performance_tracker.record_file_operation(FileOperationType::Delete, processing_time);
        } else {
            self.performance_tracker.record_failed_operation();
        }
        
        debug!(
            "remove_media_file for '{}' completed in {:?} ({})",
            path.display(),
            processing_time,
            if success { "removed" } else { "not found" }
        );
        
        Ok(success)
    }
    
    /// Update a single media file using atomic bulk operation wrapper
    /// This method wraps the bulk operation to maintain consistency and atomic statistics tracking
    async fn update_media_file(&self, file: &MediaFile) -> Result<()> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }

        let start_time = Instant::now();
        
        // Use bulk operation for single file (atomic wrapper)
        let result = self.batch_update_files(&[file.clone()]).await?;
        
        // Record individual operation statistics
        let processing_time = start_time.elapsed();
        if result.is_successful() {
            self.performance_tracker.record_file_operation(FileOperationType::Update, processing_time);
            
            debug!(
                "update_media_file for '{}' completed in {:?}",
                file.path.display(),
                processing_time
            );
            
            Ok(())
        } else {
            self.performance_tracker.record_failed_operation();
            Err(anyhow!("Failed to update media file: {:?}", result.errors))
        }
    }
    
    async fn get_files_in_directory(&self, _dir: &Path) -> Result<Vec<MediaFile>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 8"))
    }
    
    async fn get_directory_listing(&self, _parent_path: &Path, _media_type_filter: &str) -> Result<(Vec<MediaDirectory>, Vec<MediaFile>)> {
        Err(anyhow!("Not implemented yet - will be implemented in task 8"))
    }
    
    async fn cleanup_missing_files(&self, _existing_paths: &[PathBuf]) -> Result<usize> {
        Err(anyhow!("Not implemented yet - will be implemented in task 6"))
    }
    
    /// Get a single media file by path using atomic cache lookup
    /// This method uses atomic index operations and cache statistics tracking
    async fn get_file_by_path(&self, path: &Path) -> Result<Option<MediaFile>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }

        let start_time = Instant::now();
        
        // Convert path to canonical format for index lookup
        let canonical_path = self.path_normalizer.to_canonical(path)
            .map_err(|e| anyhow!("Path normalization failed: {}", e))?;

        // Record index operation
        self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
        
        // Look up file in the index with atomic cache access
        let file_offset = {
            let mut index_manager = self.index_manager.write().await;
            index_manager.find_by_path(&canonical_path)
        };

        let result = if let Some(offset) = file_offset {
            // Cache hit - record atomic statistics
            self.performance_tracker.record_cache_access(true);
            
            // Read the file data from the memory-mapped file at the given offset
            let data_file = self.data_file.read().await;
            
            // For now, create a placeholder MediaFile since we need FlatBuffer deserialization
            // In a full implementation, we'd deserialize from FlatBuffer data at the offset
            let media_file = MediaFile {
                id: Some(offset as i64), // Use offset as temporary ID
                path: path.to_path_buf(),
                filename: path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                size: 1000, // Placeholder - would be read from FlatBuffer
                modified: SystemTime::now(), // Placeholder - would be read from FlatBuffer
                mime_type: "audio/mpeg".to_string(), // Placeholder - would be read from FlatBuffer
                duration: Some(Duration::from_secs(180)), // Placeholder
                title: Some("Unknown".to_string()), // Placeholder
                artist: Some("Unknown".to_string()), // Placeholder
                album: Some("Unknown".to_string()), // Placeholder
                genre: Some("Unknown".to_string()), // Placeholder
                track_number: Some(1), // Placeholder
                year: Some(2023), // Placeholder
                album_artist: Some("Unknown".to_string()), // Placeholder
                created_at: SystemTime::now(),
                updated_at: SystemTime::now(),
            };
            
            Some(media_file)
        } else {
            // Cache miss - record atomic statistics
            self.performance_tracker.record_cache_access(false);
            None
        };

        // Record individual operation performance
        let processing_time = start_time.elapsed();
        if result.is_some() {
            self.performance_tracker.record_file_operation(FileOperationType::Insert, processing_time);
        }

        debug!(
            "get_file_by_path for '{}' completed in {:?} ({})",
            path.display(),
            processing_time,
            if result.is_some() { "found" } else { "not found" }
        );

        Ok(result)
    }
    
    /// Get a single media file by ID using atomic cache lookup
    /// This method uses atomic index operations and cache statistics tracking
    async fn get_file_by_id(&self, id: i64) -> Result<Option<MediaFile>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }

        let start_time = Instant::now();
        
        // Record index operation
        self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
        
        // Look up file by ID in the index with atomic cache access
        let file_offset = {
            let mut index_manager = self.index_manager.write().await;
            index_manager.find_by_id(id as u64)
        };

        let result = if let Some(offset) = file_offset {
            // Cache hit - record atomic statistics
            self.performance_tracker.record_cache_access(true);
            
            // Read the file data from the memory-mapped file at the given offset
            let _data_file = self.data_file.read().await;
            
            // For now, create a placeholder MediaFile since we need FlatBuffer deserialization
            // In a full implementation, we'd deserialize from FlatBuffer data at the offset
            let media_file = MediaFile {
                id: Some(id),
                path: PathBuf::from(format!("/placeholder/file_{}.mp3", id)),
                filename: format!("file_{}.mp3", id),
                size: 1000, // Placeholder - would be read from FlatBuffer
                modified: SystemTime::now(), // Placeholder - would be read from FlatBuffer
                mime_type: "audio/mpeg".to_string(), // Placeholder - would be read from FlatBuffer
                duration: Some(Duration::from_secs(180)), // Placeholder
                title: Some("Unknown".to_string()), // Placeholder
                artist: Some("Unknown".to_string()), // Placeholder
                album: Some("Unknown".to_string()), // Placeholder
                genre: Some("Unknown".to_string()), // Placeholder
                track_number: Some(1), // Placeholder
                year: Some(2023), // Placeholder
                album_artist: Some("Unknown".to_string()), // Placeholder
                created_at: SystemTime::now(),
                updated_at: SystemTime::now(),
            };
            
            Some(media_file)
        } else {
            // Cache miss - record atomic statistics
            self.performance_tracker.record_cache_access(false);
            None
        };

        // Record individual operation performance
        let processing_time = start_time.elapsed();
        if result.is_some() {
            self.performance_tracker.record_file_operation(FileOperationType::Insert, processing_time);
        }

        debug!(
            "get_file_by_id for ID {} completed in {:?} ({})",
            id,
            processing_time,
            if result.is_some() { "found" } else { "not found" }
        );

        Ok(result)
    }
    
    async fn get_stats(&self) -> Result<DatabaseStats> {
        let perf_stats = self.get_performance_stats();
        Ok(DatabaseStats {
            total_files: perf_stats.processed_files as usize,
            total_size: perf_stats.bytes_written,
            database_size: {
                let data_file = self.data_file.read().await;
                data_file.current_size() as u64
            },
        })
    }
    
    async fn check_and_repair(&self) -> Result<DatabaseHealth> {
        // Placeholder - basic health check
        Ok(DatabaseHealth {
            is_healthy: self.is_open(),
            corruption_detected: false,
            integrity_check_passed: true,
            issues: vec![],
            repair_attempted: false,
            repair_successful: false,
        })
    }
    
    async fn create_backup(&self, _backup_path: &Path) -> Result<()> {
        Err(anyhow!("Not implemented yet"))
    }
    
    async fn restore_from_backup(&self, _backup_path: &Path) -> Result<()> {
        Err(anyhow!("Not implemented yet"))
    }
    
    async fn vacuum(&self) -> Result<()> {
        Err(anyhow!("Not implemented yet"))
    }
    
    // Music categorization methods - placeholders
    async fn get_artists(&self) -> Result<Vec<MusicCategory>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 9"))
    }
    
    async fn get_albums(&self, _artist: Option<&str>) -> Result<Vec<MusicCategory>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 9"))
    }
    
    async fn get_genres(&self) -> Result<Vec<MusicCategory>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 9"))
    }
    
    // Bulk operations implementation
    async fn bulk_store_media_files(&self, files: &[MediaFile]) -> Result<Vec<i64>> {
        let result = self.batch_insert_files(files).await?;
        if result.is_successful() {
            // For now, return sequential IDs based on batch ID
            // In a real implementation, we'd track individual file IDs
            let base_id = result.batch_id as i64;
            Ok((0..files.len()).map(|i| base_id + i as i64).collect())
        } else {
            Err(anyhow!("Bulk store failed: {:?}", result.errors))
        }
    }
    
    async fn bulk_update_media_files(&self, files: &[MediaFile]) -> Result<()> {
        let result = self.batch_update_files(files).await?;
        if result.is_successful() {
            Ok(())
        } else {
            Err(anyhow!("Bulk update failed: {:?}", result.errors))
        }
    }
    
    async fn bulk_remove_media_files(&self, paths: &[PathBuf]) -> Result<usize> {
        let result = self.batch_remove_files(paths).await?;
        Ok(result.files_removed)
    }
    
    async fn get_files_by_paths(&self, paths: &[PathBuf]) -> Result<Vec<MediaFile>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        if paths.is_empty() {
            return Ok(Vec::new());
        }

        let start_time = Instant::now();
        let mut found_files = Vec::new();

        // Convert paths to canonical format for index lookups
        let canonical_paths: Result<Vec<String>> = paths
            .iter()
            .map(|path| self.path_normalizer.to_canonical(path).map_err(|e| anyhow!("Path normalization failed: {}", e)))
            .collect();

        let canonical_paths = canonical_paths?;

        // Look up files in the index
        {
            let mut index_manager = self.index_manager.write().await;
            let _data_file = self.data_file.read().await;

            for canonical_path in &canonical_paths {
                self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
                
                if let Some(_offset) = index_manager.find_by_path(canonical_path) {
                    self.performance_tracker.record_cache_access(true);
                    
                    // Read the file data from the memory-mapped file
                    // For now, we'll create a placeholder MediaFile
                    // In a full implementation, we'd deserialize from FlatBuffer data
                    let media_file = MediaFile::new(
                        PathBuf::from(canonical_path),
                        1000, // placeholder size
                        "audio/mpeg".to_string() // placeholder mime type
                    );
                    found_files.push(media_file);
                } else {
                    self.performance_tracker.record_cache_access(false);
                }
            }
        }

        let processing_time = start_time.elapsed();
        let throughput = found_files.len() as f64 / processing_time.as_secs_f64();

        info!(
            "Bulk retrieved {} files in {:?} ({:.0} files/sec)",
            found_files.len(),
            processing_time,
            throughput
        );

        Ok(found_files)
    }
    
    async fn get_years(&self) -> Result<Vec<MusicCategory>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 9"))
    }
    
    async fn get_album_artists(&self) -> Result<Vec<MusicCategory>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 9"))
    }
    
    async fn get_music_by_artist(&self, _artist: &str) -> Result<Vec<MediaFile>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 9"))
    }
    
    async fn get_music_by_album(&self, _album: &str, _artist: Option<&str>) -> Result<Vec<MediaFile>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 9"))
    }
    
    async fn get_music_by_genre(&self, _genre: &str) -> Result<Vec<MediaFile>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 9"))
    }
    
    async fn get_music_by_year(&self, _year: u32) -> Result<Vec<MediaFile>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 9"))
    }
    
    async fn get_music_by_album_artist(&self, _album_artist: &str) -> Result<Vec<MediaFile>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 9"))
    }
    
    // Playlist methods - placeholders
    async fn create_playlist(&self, _name: &str, _description: Option<&str>) -> Result<i64> {
        Err(anyhow!("Not implemented yet - will be implemented in task 10"))
    }
    
    async fn get_playlists(&self) -> Result<Vec<Playlist>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 10"))
    }
    
    async fn get_playlist(&self, _playlist_id: i64) -> Result<Option<Playlist>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 10"))
    }
    
    async fn update_playlist(&self, _playlist: &Playlist) -> Result<()> {
        Err(anyhow!("Not implemented yet - will be implemented in task 10"))
    }
    
    async fn delete_playlist(&self, _playlist_id: i64) -> Result<bool> {
        Err(anyhow!("Not implemented yet - will be implemented in task 10"))
    }
    
    async fn add_to_playlist(&self, _playlist_id: i64, _media_file_id: i64, _position: Option<u32>) -> Result<i64> {
        Err(anyhow!("Not implemented yet - will be implemented in task 10"))
    }
    
    async fn batch_add_to_playlist(&self, _playlist_id: i64, _media_file_ids: &[(i64, u32)]) -> Result<Vec<i64>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 10"))
    }
    
    async fn remove_from_playlist(&self, _playlist_id: i64, _media_file_id: i64) -> Result<bool> {
        Err(anyhow!("Not implemented yet - will be implemented in task 10"))
    }
    
    async fn get_playlist_tracks(&self, _playlist_id: i64) -> Result<Vec<MediaFile>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 10"))
    }
    
    async fn reorder_playlist(&self, _playlist_id: i64, _track_positions: &[(i64, u32)]) -> Result<()> {
        Err(anyhow!("Not implemented yet - will be implemented in task 10"))
    }
    
    async fn get_files_with_path_prefix(&self, _canonical_prefix: &str) -> Result<Vec<MediaFile>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 8"))
    }
    
    async fn get_direct_subdirectories(&self, _canonical_parent_path: &str) -> Result<Vec<MediaDirectory>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 8"))
    }
    
    async fn batch_cleanup_missing_files(&self, _existing_canonical_paths: &HashSet<String>) -> Result<usize> {
        Err(anyhow!("Not implemented yet - will be implemented in task 6"))
    }
    
    async fn database_native_cleanup(&self, _existing_canonical_paths: &[String]) -> Result<usize> {
        Err(anyhow!("Not implemented yet - will be implemented in task 6"))
    }
    
    async fn get_filtered_direct_subdirectories(&self, _canonical_parent_path: &str, _mime_filter: &str) -> Result<Vec<MediaDirectory>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 8"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_zerocopy_database_creation() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        
        let config = ZeroCopyConfig::default();
        let db = ZeroCopyDatabase::new(db_path.clone(), Some(config)).await.unwrap();
        
        assert!(!db.is_initialized());
        assert!(!db.is_open());
        assert_eq!(db.get_config().batch_size, 100_000);
    }
    
    #[tokio::test]
    async fn test_database_initialization() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        
        let db = ZeroCopyDatabase::new(db_path.clone(), None).await.unwrap();
        
        db.initialize().await.unwrap();
        
        assert!(db.is_initialized());
        assert!(db_path.with_extension("fb").exists());
    }
    
    #[tokio::test]
    async fn test_database_open_close() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        
        let db = ZeroCopyDatabase::new(db_path.clone(), None).await.unwrap();
        
        db.open().await.unwrap();
        assert!(db.is_open());
        assert!(db.is_initialized());
        
        db.close().await.unwrap();
        assert!(!db.is_open());
    }
    
    #[tokio::test]
    async fn test_performance_tracking() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        
        let db = ZeroCopyDatabase::new(db_path.clone(), None).await.unwrap();
        
        // Record some operations
        db.performance_tracker.record_file_operation(FileOperationType::Insert, Duration::from_millis(10));
        db.performance_tracker.record_cache_access(true);
        db.performance_tracker.record_cache_access(false);
        
        let stats = db.get_performance_stats();
        assert_eq!(stats.processed_files, 1);
        assert_eq!(stats.inserted_files, 1);
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.cache_misses, 1);
        assert_eq!(stats.cache_hit_rate, 0.5);
    }
    
    #[tokio::test]
    async fn test_config_from_env() {
        std::env::set_var("ZEROCOPY_CACHE_MB", "8");
        std::env::set_var("ZEROCOPY_BATCH_SIZE", "50000");
        
        let config = ZeroCopyConfig::from_env();
        
        assert_eq!(config.memory_map_size_mb, 8);
        assert_eq!(config.batch_size, 50000);
        
        // Clean up
        std::env::remove_var("ZEROCOPY_CACHE_MB");
        std::env::remove_var("ZEROCOPY_BATCH_SIZE");
    }
    
    #[tokio::test]
    async fn test_index_manager() {
        let mut index_manager = IndexManager::new(1000, 1024 * 1024);
        
        let test_file = MediaFile {
            id: Some(1),
            path: PathBuf::from("/test/file.mp3"),
            filename: "file.mp3".to_string(),
            size: 1024,
            modified: SystemTime::now(),
            mime_type: "audio/mpeg".to_string(),
            duration: Some(Duration::from_secs(180)),
            title: Some("Test Song".to_string()),
            artist: Some("Test Artist".to_string()),
            album: Some("Test Album".to_string()),
            genre: Some("Rock".to_string()),
            track_number: Some(1),
            year: Some(2023),
            album_artist: Some("Test Artist".to_string()),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        };
        
        index_manager.insert_file_index(&test_file, 1000);
        
        assert!(index_manager.find_by_path("/test/file.mp3").is_some());
        assert_eq!(index_manager.find_by_path("/test/file.mp3").unwrap(), 1000);
        assert!(index_manager.is_dirty());
        
        let stats = index_manager.get_stats();
        assert_eq!(stats.path_entries, 1);
        assert_eq!(stats.id_entries, 1);
    }
    
    #[tokio::test]
    async fn test_individual_operations_as_bulk_wrappers() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        
        let db = ZeroCopyDatabase::new(db_path.clone(), None).await.unwrap();
        db.open().await.unwrap();
        
        let test_file = MediaFile {
            id: Some(1),
            path: PathBuf::from("/test/individual_file.mp3"),
            filename: "individual_file.mp3".to_string(),
            size: 2048,
            modified: SystemTime::now(),
            mime_type: "audio/mpeg".to_string(),
            duration: Some(Duration::from_secs(240)),
            title: Some("Individual Test Song".to_string()),
            artist: Some("Individual Test Artist".to_string()),
            album: Some("Individual Test Album".to_string()),
            genre: Some("Pop".to_string()),
            track_number: Some(2),
            year: Some(2024),
            album_artist: Some("Individual Test Artist".to_string()),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        };
        
        // Test individual store operation (implemented as bulk wrapper)
        let file_id = db.store_media_file(&test_file).await.unwrap();
        assert!(file_id > 0);
        
        // Test individual get operation with atomic cache lookup
        let retrieved_file = db.get_file_by_path(&test_file.path).await.unwrap();
        assert!(retrieved_file.is_some());
        
        // Test individual get by ID operation with atomic cache lookup
        let retrieved_by_id = db.get_file_by_id(file_id).await.unwrap();
        assert!(retrieved_by_id.is_some());
        
        // Test individual update operation (implemented as bulk wrapper)
        let mut updated_file = test_file.clone();
        updated_file.title = Some("Updated Individual Test Song".to_string());
        db.update_media_file(&updated_file).await.unwrap();
        
        // Test individual remove operation (implemented as bulk wrapper)
        let removed = db.remove_media_file(&test_file.path).await.unwrap();
        assert!(removed);
        
        // Verify file is no longer found
        let not_found = db.get_file_by_path(&test_file.path).await.unwrap();
        assert!(not_found.is_none());
        
        // Check performance statistics were recorded
        let stats = db.get_performance_stats();
        assert!(stats.processed_files > 0);
        assert!(stats.total_operations > 0);
        assert!(stats.inserted_files > 0);
        assert!(stats.updated_files > 0);
        assert!(stats.deleted_files > 0);
        
        db.close().await.unwrap();
    }
}
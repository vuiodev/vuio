use anyhow::{anyhow, Result};
use std::collections::{HashMap, BTreeMap};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info, warn};

use super::MediaFile;
use super::memory_bounded_cache::{MemoryBoundedCache, CacheStats as MemoryCacheStats, MemoryPressureConfig, MemoryPressureStatus};

/// Types of indexes that can be marked as dirty
#[derive(Debug, Clone, Copy)]
pub enum IndexType {
    PathToOffset = 0,
    IdToOffset = 1,
    DirectoryIndex = 2,
    ArtistIndex = 3,
    AlbumIndex = 4,
    GenreIndex = 5,
    YearIndex = 6,
}

/// Memory-bounded cache manager with atomic operations and pressure detection
#[derive(Debug)]
pub struct CacheManager {
    path_cache: MemoryBoundedCache<String, u64>,
    id_cache: MemoryBoundedCache<u64, u64>,
    directory_cache: MemoryBoundedCache<String, Vec<u64>>,
    
    // Cache configuration
    pressure_config: MemoryPressureConfig,
    
    // Atomic performance tracking
    total_cache_operations: AtomicU64,
    cache_pressure_events: AtomicU64,
    last_pressure_check: AtomicU64,
}

impl CacheManager {
    pub fn new(max_entries_per_cache: usize, max_memory_per_cache: usize) -> Self {
        let pressure_config = MemoryPressureConfig {
            pressure_threshold: 0.8,  // 80% memory usage triggers pressure
            critical_threshold: 0.95, // 95% memory usage triggers aggressive eviction
            eviction_percentage: 0.25, // Evict 25% of entries during pressure
            min_entries: 100,         // Always keep at least 100 entries
        };
        
        Self {
            path_cache: MemoryBoundedCache::with_pressure_config(
                max_entries_per_cache, 
                max_memory_per_cache, 
                pressure_config.clone()
            ),
            id_cache: MemoryBoundedCache::with_pressure_config(
                max_entries_per_cache, 
                max_memory_per_cache, 
                pressure_config.clone()
            ),
            directory_cache: MemoryBoundedCache::with_pressure_config(
                max_entries_per_cache / 4, // Directories are less frequent
                max_memory_per_cache / 2,  // But can be larger
                pressure_config.clone()
            ),
            pressure_config,
            total_cache_operations: AtomicU64::new(0),
            cache_pressure_events: AtomicU64::new(0),
            last_pressure_check: AtomicU64::new(0),
        }
    }
    
    /// Get value from path cache with atomic tracking
    pub fn get_path(&mut self, path: &str) -> Option<u64> {
        self.total_cache_operations.fetch_add(1, Ordering::Relaxed);
        self.path_cache.get(&path.to_string())
    }
    
    /// Insert value into path cache with automatic pressure management
    pub fn insert_path(&mut self, path: String, offset: u64) -> Result<()> {
        self.total_cache_operations.fetch_add(1, Ordering::Relaxed);
        self.check_and_handle_pressure();
        self.path_cache.insert(path, offset)
    }
    
    /// Get value from ID cache with atomic tracking
    pub fn get_id(&mut self, id: u64) -> Option<u64> {
        self.total_cache_operations.fetch_add(1, Ordering::Relaxed);
        self.id_cache.get(&id)
    }
    
    /// Insert value into ID cache with automatic pressure management
    pub fn insert_id(&mut self, id: u64, offset: u64) -> Result<()> {
        self.total_cache_operations.fetch_add(1, Ordering::Relaxed);
        self.check_and_handle_pressure();
        self.id_cache.insert(id, offset)
    }
    
    /// Get directory files from cache
    pub fn get_directory(&mut self, dir_path: &str) -> Option<Vec<u64>> {
        self.total_cache_operations.fetch_add(1, Ordering::Relaxed);
        self.directory_cache.get(&dir_path.to_string())
    }
    
    /// Insert directory files into cache
    pub fn insert_directory(&mut self, dir_path: String, files: Vec<u64>) -> Result<()> {
        self.total_cache_operations.fetch_add(1, Ordering::Relaxed);
        self.check_and_handle_pressure();
        self.directory_cache.insert(dir_path, files)
    }
    
    /// Remove entries from all caches
    pub fn remove_path(&mut self, path: &str) -> Option<u64> {
        self.total_cache_operations.fetch_add(1, Ordering::Relaxed);
        self.path_cache.remove(&path.to_string())
    }
    
    /// Check memory pressure across all caches and handle if needed
    pub fn check_and_handle_pressure(&mut self) -> MemoryPressureStatus {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let last_check = self.last_pressure_check.load(Ordering::Relaxed);
        
        // Check pressure every 5 seconds to avoid overhead
        if now - last_check < 5 {
            return MemoryPressureStatus::Normal;
        }
        
        self.last_pressure_check.store(now, Ordering::Relaxed);
        
        // Check pressure on all caches
        let path_pressure = self.path_cache.check_memory_pressure();
        let id_pressure = self.id_cache.check_memory_pressure();
        let dir_pressure = self.directory_cache.check_memory_pressure();
        
        // Determine overall pressure status
        let overall_pressure = match (path_pressure, id_pressure, dir_pressure) {
            (MemoryPressureStatus::Critical, _, _) |
            (_, MemoryPressureStatus::Critical, _) |
            (_, _, MemoryPressureStatus::Critical) => MemoryPressureStatus::Critical,
            
            (MemoryPressureStatus::Pressure, _, _) |
            (_, MemoryPressureStatus::Pressure, _) |
            (_, _, MemoryPressureStatus::Pressure) => MemoryPressureStatus::Pressure,
            
            _ => MemoryPressureStatus::Normal,
        };
        
        if overall_pressure != MemoryPressureStatus::Normal {
            self.cache_pressure_events.fetch_add(1, Ordering::Relaxed);
            
            match overall_pressure {
                MemoryPressureStatus::Critical => {
                    warn!("Critical memory pressure detected across caches, forcing aggressive cleanup");
                    self.force_cleanup_all_caches(0.5); // Free 50% of memory
                }
                MemoryPressureStatus::Pressure => {
                    info!("Memory pressure detected across caches, performing cleanup");
                    self.force_cleanup_all_caches(0.25); // Free 25% of memory
                }
                _ => {}
            }
        }
        
        overall_pressure
    }
    
    /// Force cleanup of all caches to free memory
    pub fn force_cleanup_all_caches(&mut self, memory_reduction_factor: f64) {
        let path_stats = self.path_cache.get_stats();
        let id_stats = self.id_cache.get_stats();
        let dir_stats = self.directory_cache.get_stats();
        
        // Calculate target memory for each cache
        let path_target = (path_stats.memory_bytes as f64 * (1.0 - memory_reduction_factor)) as usize;
        let id_target = (id_stats.memory_bytes as f64 * (1.0 - memory_reduction_factor)) as usize;
        let dir_target = (dir_stats.memory_bytes as f64 * (1.0 - memory_reduction_factor)) as usize;
        
        // Force eviction on all caches
        let path_evicted = self.path_cache.force_evict(path_target);
        let id_evicted = self.id_cache.force_evict(id_target);
        let dir_evicted = self.directory_cache.force_evict(dir_target);
        
        info!(
            "Cache cleanup completed: evicted {} path entries, {} ID entries, {} directory entries",
            path_evicted, id_evicted, dir_evicted
        );
    }
    
    /// Get comprehensive cache statistics
    pub fn get_cache_stats(&self) -> CombinedCacheStats {
        let path_stats = self.path_cache.get_stats();
        let id_stats = self.id_cache.get_stats();
        let dir_stats = self.directory_cache.get_stats();
        
        CombinedCacheStats {
            path_cache: path_stats.clone(),
            id_cache: id_stats.clone(),
            directory_cache: dir_stats.clone(),
            total_cache_operations: self.total_cache_operations.load(Ordering::Relaxed),
            cache_pressure_events: self.cache_pressure_events.load(Ordering::Relaxed),
            combined_hit_rate: {
                let total_hits = path_stats.hit_count + id_stats.hit_count + dir_stats.hit_count;
                let total_requests = path_stats.total_requests + id_stats.total_requests + dir_stats.total_requests;
                if total_requests > 0 {
                    total_hits as f64 / total_requests as f64
                } else {
                    0.0
                }
            },
            combined_memory_usage: path_stats.memory_bytes + id_stats.memory_bytes + dir_stats.memory_bytes,
            combined_max_memory: path_stats.max_memory_bytes + id_stats.max_memory_bytes + dir_stats.max_memory_bytes,
        }
    }
    
    /// Clear all caches
    pub fn clear_all(&mut self) {
        self.path_cache.clear();
        self.id_cache.clear();
        self.directory_cache.clear();
        info!("All caches cleared");
    }
    
    /// Get all path cache entries for persistence
    pub fn get_all_path_entries(&self) -> Vec<(String, u64)> {
        self.path_cache.get_all_entries()
    }
    
    /// Cleanup expired entries from all caches
    pub fn cleanup_expired_entries(&mut self) -> CleanupResult {
        let total_removed = 0;
        let total_memory_freed = 0;
        
        // For now, just trigger pressure handling which will clean up entries
        let _pressure_status = self.check_and_handle_pressure();
        
        // Return basic cleanup result
        CleanupResult {
            entries_removed: total_removed,
            memory_freed: total_memory_freed,
        }
    }
}

/// Result of cleanup operation
#[derive(Debug)]
pub struct CleanupResult {
    pub entries_removed: usize,
    pub memory_freed: usize,
}

/// Combined statistics for all caches
#[derive(Debug, Clone)]
pub struct CombinedCacheStats {
    pub path_cache: MemoryCacheStats,
    pub id_cache: MemoryCacheStats,
    pub directory_cache: MemoryCacheStats,
    pub total_cache_operations: u64,
    pub cache_pressure_events: u64,
    pub combined_hit_rate: f64,
    pub combined_memory_usage: usize,
    pub combined_max_memory: usize,
}

/// Legacy cache statistics for backward compatibility
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub max_entries: usize,
    pub memory_bytes: usize,
    pub max_memory_bytes: usize,
    pub hit_count: u64,
    pub miss_count: u64,
    pub eviction_count: u64,
    pub hit_rate: f64,
}

/// Enhanced IndexManager with atomic operations and memory-bounded caching
#[derive(Debug)]
pub struct IndexManager {
    // Memory-bounded cache manager with pressure detection
    pub cache_manager: CacheManager,
    
    // Directory-based B-tree index with atomic operations
    directory_index: BTreeMap<String, Vec<u64>>,
    
    // Music categorization indexes
    artist_index: HashMap<String, Vec<u64>>,
    album_index: HashMap<String, Vec<u64>>,
    genre_index: HashMap<String, Vec<u64>>,
    year_index: HashMap<u32, Vec<u64>>,
    album_artist_index: HashMap<String, Vec<u64>>,
    
    // Atomic counters for operations
    lookup_count: AtomicU64,
    update_count: AtomicU64,
    insert_count: AtomicU64,
    remove_count: AtomicU64,
    
    // Index metadata with atomic operations
    generation: AtomicU64,
    dirty_flags: AtomicU64,  // Bitmask of dirty index types
    last_persistence: AtomicU64, // Timestamp of last persistence operation
    
    // Configuration
    max_entries: usize,
    max_memory_bytes: usize,
    persistence_interval: Duration,
}

impl IndexManager {
    /// Create a new IndexManager with specified memory limits
    pub fn new(max_entries: usize, max_memory_bytes: usize) -> Self {
        let cache_memory_per_index = max_memory_bytes / 3; // Split between path, id, and directory caches
        let cache_entries_per_index = max_entries / 3;
        
        Self {
            cache_manager: CacheManager::new(cache_entries_per_index, cache_memory_per_index),
            directory_index: BTreeMap::new(),
            artist_index: HashMap::new(),
            album_index: HashMap::new(),
            genre_index: HashMap::new(),
            year_index: HashMap::new(),
            album_artist_index: HashMap::new(),
            lookup_count: AtomicU64::new(0),
            update_count: AtomicU64::new(0),
            insert_count: AtomicU64::new(0),
            remove_count: AtomicU64::new(0),
            generation: AtomicU64::new(1),
            dirty_flags: AtomicU64::new(0),
            last_persistence: AtomicU64::new(0),
            max_entries,
            max_memory_bytes,
            persistence_interval: Duration::from_secs(300), // 5 minutes
        }
    }
    
    /// Insert file index entry with atomic operations
    pub fn insert_file_index(&mut self, file: &MediaFile, offset: u64) {
        let start_time = Instant::now();
        
        let path_key = file.path.to_string_lossy().to_string();
        
        // Update path-to-offset index with atomic cache operations and pressure management
        if let Err(e) = self.cache_manager.insert_path(path_key.clone(), offset) {
            warn!("Failed to insert path cache entry: {}", e);
        }
        
        // Update id-to-offset index if ID is available
        if let Some(id) = file.id {
            if let Err(e) = self.cache_manager.insert_id(id as u64, offset) {
                warn!("Failed to insert ID cache entry: {}", e);
            }
        }
        
        // Update directory index with atomic operations
        if let Some(parent) = file.path.parent() {
            let parent_key = parent.to_string_lossy().to_string();
            
            // Update in-memory directory index
            self.directory_index.entry(parent_key.clone()).or_insert_with(Vec::new).push(offset);
            
            // Update directory cache with current files
            let dir_files = self.directory_index.get(&parent_key).cloned().unwrap_or_default();
            if let Err(e) = self.cache_manager.insert_directory(parent_key, dir_files) {
                warn!("Failed to insert directory cache entry: {}", e);
            }
            
            self.mark_index_dirty(IndexType::DirectoryIndex);
        }
        
        // Update music categorization indexes
        if let Some(artist) = &file.artist {
            self.artist_index.entry(artist.clone()).or_insert_with(Vec::new).push(offset);
            self.mark_index_dirty(IndexType::ArtistIndex);
        }
        
        if let Some(album) = &file.album {
            self.album_index.entry(album.clone()).or_insert_with(Vec::new).push(offset);
            self.mark_index_dirty(IndexType::AlbumIndex);
        }
        
        if let Some(genre) = &file.genre {
            self.genre_index.entry(genre.clone()).or_insert_with(Vec::new).push(offset);
            self.mark_index_dirty(IndexType::GenreIndex);
        }
        
        if let Some(year) = file.year {
            self.year_index.entry(year).or_insert_with(Vec::new).push(offset);
            self.mark_index_dirty(IndexType::YearIndex);
        }
        
        if let Some(album_artist) = &file.album_artist {
            self.album_artist_index.entry(album_artist.clone()).or_insert_with(Vec::new).push(offset);
            // Note: We don't have AlbumArtistIndex in IndexType enum, so we'll use ArtistIndex for now
            self.mark_index_dirty(IndexType::ArtistIndex);
        }
        
        // Update atomic counters
        self.insert_count.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);
        
        // Mark path and ID indexes as dirty
        self.mark_index_dirty(IndexType::PathToOffset);
        self.mark_index_dirty(IndexType::IdToOffset);
        
        debug!("Inserted index for {} at offset {} in {:?}", path_key, offset, start_time.elapsed());
    }
    
    /// Remove file index entry with atomic operations
    pub fn remove_file_index(&mut self, path: &str) -> Option<u64> {
        let start_time = Instant::now();
        
        if let Some(offset) = self.cache_manager.remove_path(path) {
            // Remove from directory index (simplified - would need file data for complete removal)
            // In practice, we'd need to track which directory this file belonged to
            
            // Update atomic counters
            self.remove_count.fetch_add(1, Ordering::Relaxed);
            self.generation.fetch_add(1, Ordering::Relaxed);
            
            // Mark indexes as dirty
            self.mark_index_dirty(IndexType::PathToOffset);
            self.mark_index_dirty(IndexType::DirectoryIndex);
            
            debug!("Removed index for {} (offset {}) in {:?}", path, offset, start_time.elapsed());
            Some(offset)
        } else {
            None
        }
    }
    
    /// Find file by path with atomic cache lookup
    pub fn find_by_path(&mut self, path: &str) -> Option<u64> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.cache_manager.get_path(path)
    }
    
    /// Find file by ID with atomic cache lookup
    pub fn find_by_id(&mut self, id: u64) -> Option<u64> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.cache_manager.get_id(id)
    }
    
    /// Find files in directory with atomic B-tree operations and cache lookup
    pub fn find_files_in_directory(&mut self, dir_path: &str) -> Vec<u64> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        
        // Try cache first
        if let Some(cached_files) = self.cache_manager.get_directory(dir_path) {
            return cached_files;
        }
        
        // Fall back to in-memory index
        let files = self.directory_index.get(dir_path).cloned().unwrap_or_default();
        
        // Cache the result for future lookups
        if !files.is_empty() {
            if let Err(e) = self.cache_manager.insert_directory(dir_path.to_string(), files.clone()) {
                warn!("Failed to cache directory lookup result: {}", e);
            }
        }
        
        files
    }
    
    /// Get all directories that are subdirectories of the given path
    pub fn find_subdirectories(&self, parent_path: &str) -> Vec<String> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        
        let mut subdirs = Vec::new();
        let search_prefix = if parent_path.is_empty() || parent_path == "/" {
            String::new()
        } else {
            format!("{}/", parent_path)
        };
        
        for dir_path in self.directory_index.keys() {
            if dir_path.starts_with(&search_prefix) && dir_path != parent_path {
                // Extract immediate subdirectory
                let relative_path = &dir_path[search_prefix.len()..];
                if let Some(first_component) = relative_path.split('/').next() {
                    let subdir = if search_prefix.is_empty() {
                        first_component.to_string()
                    } else {
                        format!("{}{}", search_prefix, first_component)
                    };
                    
                    if !subdirs.contains(&subdir) {
                        subdirs.push(subdir);
                    }
                }
            }
        }
        
        subdirs.sort();
        subdirs
    }
    
    /// Find files by artist with atomic index operations
    pub fn find_files_by_artist(&self, artist: &str) -> Vec<u64> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.artist_index.get(artist).cloned().unwrap_or_default()
    }
    
    /// Find files by album with atomic index operations
    pub fn find_files_by_album(&self, album: &str) -> Vec<u64> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.album_index.get(album).cloned().unwrap_or_default()
    }
    
    /// Find files by genre with atomic index operations
    pub fn find_files_by_genre(&self, genre: &str) -> Vec<u64> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.genre_index.get(genre).cloned().unwrap_or_default()
    }
    
    /// Find files by year with atomic index operations
    pub fn find_files_by_year(&self, year: u32) -> Vec<u64> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.year_index.get(&year).cloned().unwrap_or_default()
    }
    
    /// Get all unique artists with file counts for atomic scanning
    pub fn get_all_artists(&self) -> Vec<(String, Vec<u64>)> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.artist_index.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
    
    /// Get all unique albums with file counts for atomic scanning
    pub fn get_all_albums(&self) -> Vec<(String, Vec<u64>)> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.album_index.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
    
    /// Get albums by specific artist with atomic filtering
    pub fn get_albums_by_artist(&self, artist: &str) -> Vec<(String, Vec<u64>)> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        
        // Get all files by this artist first
        let artist_files = self.artist_index.get(artist).cloned().unwrap_or_default();
        if artist_files.is_empty() {
            return Vec::new();
        }
        
        // Filter albums that contain files by this artist
        let mut artist_albums = Vec::new();
        for (album, album_files) in &self.album_index {
            // Check if any files in this album are by the specified artist
            let has_artist_files = album_files.iter().any(|offset| artist_files.contains(offset));
            if has_artist_files {
                // Only include files that are by this artist
                let filtered_files: Vec<u64> = album_files.iter()
                    .filter(|offset| artist_files.contains(offset))
                    .cloned()
                    .collect();
                if !filtered_files.is_empty() {
                    artist_albums.push((album.clone(), filtered_files));
                }
            }
        }
        
        artist_albums
    }
    
    /// Get all unique genres with file counts for atomic categorization
    pub fn get_all_genres(&self) -> Vec<(String, Vec<u64>)> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.genre_index.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
    
    /// Get all unique years with file counts for atomic year extraction
    pub fn get_all_years(&self) -> Vec<(u32, Vec<u64>)> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.year_index.iter().map(|(k, v)| (*k, v.clone())).collect()
    }
    
    /// Get all unique album artists with file counts for atomic scanning
    pub fn get_all_album_artists(&self) -> Vec<(String, Vec<u64>)> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.album_artist_index.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
    
    /// Find files by album and artist with atomic filtering
    pub fn find_files_by_album_and_artist(&self, album: &str, artist: &str) -> Vec<u64> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        
        let album_files = self.album_index.get(album).cloned().unwrap_or_default();
        let artist_files = self.artist_index.get(artist).cloned().unwrap_or_default();
        
        // Return intersection of album and artist files
        album_files.into_iter()
            .filter(|offset| artist_files.contains(offset))
            .collect()
    }
    
    /// Find files by album artist with atomic lookups
    pub fn find_files_by_album_artist(&self, album_artist: &str) -> Vec<u64> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.album_artist_index.get(album_artist).cloned().unwrap_or_default()
    }
    
    /// Mark specific index type as dirty with atomic operations
    fn mark_index_dirty(&self, index_type: IndexType) {
        let flag = 1u64 << (index_type as u8);
        self.dirty_flags.fetch_or(flag, Ordering::Relaxed);
        self.update_count.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Check if specific index type is dirty
    pub fn is_index_dirty(&self, index_type: IndexType) -> bool {
        let flag = 1u64 << (index_type as u8);
        (self.dirty_flags.load(Ordering::Relaxed) & flag) != 0
    }
    
    /// Check if any indexes are dirty
    pub fn is_dirty(&self) -> bool {
        self.dirty_flags.load(Ordering::Relaxed) != 0
    }
    
    /// Mark all indexes as clean
    pub fn mark_clean(&self) {
        self.dirty_flags.store(0, Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_persistence.store(now, Ordering::Relaxed);
    }
    
    /// Check if persistence is needed based on time interval
    pub fn needs_persistence(&self) -> bool {
        if !self.is_dirty() {
            return false;
        }
        
        let last_persistence = self.last_persistence.load(Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        now - last_persistence > self.persistence_interval.as_secs()
    }
    
    /// Get current generation (for consistency checks)
    pub fn get_generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
    
    /// Get comprehensive index statistics
    pub fn get_stats(&self) -> IndexStats {
        let combined_cache_stats = self.cache_manager.get_cache_stats();
        
        IndexStats {
            path_entries: combined_cache_stats.path_cache.entries,
            id_entries: combined_cache_stats.id_cache.entries,
            directory_entries: self.directory_index.len(),
            artist_entries: self.artist_index.len(),
            album_entries: self.album_index.len(),
            genre_entries: self.genre_index.len(),
            year_entries: self.year_index.len(),
            album_artist_entries: self.album_artist_index.len(),
            generation: self.generation.load(Ordering::Relaxed),
            is_dirty: self.is_dirty(),
            max_entries: self.max_entries,
            max_memory_bytes: self.max_memory_bytes,
            current_memory_bytes: combined_cache_stats.combined_memory_usage,
            lookup_count: self.lookup_count.load(Ordering::Relaxed),
            update_count: self.update_count.load(Ordering::Relaxed),
            insert_count: self.insert_count.load(Ordering::Relaxed),
            remove_count: self.remove_count.load(Ordering::Relaxed),
            path_cache_hit_rate: combined_cache_stats.path_cache.hit_rate,
            id_cache_hit_rate: combined_cache_stats.id_cache.hit_rate,
            path_cache_evictions: combined_cache_stats.path_cache.eviction_count,
            id_cache_evictions: combined_cache_stats.id_cache.eviction_count,
        }
    }
    
    /// Atomic index persistence to disk
    pub async fn persist_indexes(&self, index_file_path: &Path) -> Result<()> {
        if !self.is_dirty() {
            debug!("Indexes are clean, skipping persistence");
            return Ok(());
        }
        
        let start_time = Instant::now();
        info!("Persisting indexes to {}", index_file_path.display());
        
        // Create a binary format for efficient storage
        let mut buffer = Vec::new();
        
        // Write header
        buffer.extend_from_slice(b"MEDIAIDX"); // Magic number
        buffer.extend_from_slice(&1u32.to_le_bytes()); // Version
        buffer.extend_from_slice(&self.generation.load(Ordering::Relaxed).to_le_bytes());
        
        // Write directory index
        let dir_count = self.directory_index.len() as u32;
        buffer.extend_from_slice(&dir_count.to_le_bytes());
        
        for (path, offsets) in &self.directory_index {
            let path_bytes = path.as_bytes();
            buffer.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
            buffer.extend_from_slice(path_bytes);
            
            buffer.extend_from_slice(&(offsets.len() as u32).to_le_bytes());
            for offset in offsets {
                buffer.extend_from_slice(&offset.to_le_bytes());
            }
        }
        
        // Write music indexes (simplified - in practice would serialize all indexes)
        let artist_count = self.artist_index.len() as u32;
        buffer.extend_from_slice(&artist_count.to_le_bytes());
        
        for (artist, offsets) in &self.artist_index {
            let artist_bytes = artist.as_bytes();
            buffer.extend_from_slice(&(artist_bytes.len() as u32).to_le_bytes());
            buffer.extend_from_slice(artist_bytes);
            
            buffer.extend_from_slice(&(offsets.len() as u32).to_le_bytes());
            for offset in offsets {
                buffer.extend_from_slice(&offset.to_le_bytes());
            }
        }
        
        // Write to file atomically
        let temp_path = index_file_path.with_extension("tmp");
        let mut file = fs::File::create(&temp_path).await?;
        file.write_all(&buffer).await?;
        file.sync_all().await?;
        drop(file);
        
        // Atomic rename
        fs::rename(&temp_path, index_file_path).await?;
        
        // Mark as clean
        self.mark_clean();
        
        let persistence_time = start_time.elapsed();
        info!(
            "Persisted {} bytes of index data in {:?}",
            buffer.len(),
            persistence_time
        );
        
        Ok(())
    }
    
    /// Atomic index loading from disk
    pub async fn load_indexes(&mut self, index_file_path: &Path) -> Result<()> {
        if !index_file_path.exists() {
            info!("Index file does not exist, starting with empty indexes");
            return Ok(());
        }
        
        let start_time = Instant::now();
        info!("Loading indexes from {}", index_file_path.display());
        
        let mut file = fs::File::open(index_file_path).await?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await?;
        
        if buffer.len() < 16 {
            return Err(anyhow!("Index file too small"));
        }
        
        let mut offset = 0;
        
        // Read header
        let magic = &buffer[offset..offset + 8];
        if magic != b"MEDIAIDX" {
            return Err(anyhow!("Invalid index file magic number"));
        }
        offset += 8;
        
        let version = u32::from_le_bytes([buffer[offset], buffer[offset + 1], buffer[offset + 2], buffer[offset + 3]]);
        offset += 4;
        
        if version != 1 {
            return Err(anyhow!("Unsupported index file version: {}", version));
        }
        
        let generation = u64::from_le_bytes([
            buffer[offset], buffer[offset + 1], buffer[offset + 2], buffer[offset + 3],
            buffer[offset + 4], buffer[offset + 5], buffer[offset + 6], buffer[offset + 7],
        ]);
        offset += 8;
        
        // Read directory index
        let dir_count = u32::from_le_bytes([buffer[offset], buffer[offset + 1], buffer[offset + 2], buffer[offset + 3]]);
        offset += 4;
        
        self.directory_index.clear();
        for _ in 0..dir_count {
            let path_len = u32::from_le_bytes([buffer[offset], buffer[offset + 1], buffer[offset + 2], buffer[offset + 3]]) as usize;
            offset += 4;
            
            let path = String::from_utf8(buffer[offset..offset + path_len].to_vec())?;
            offset += path_len;
            
            let offset_count = u32::from_le_bytes([buffer[offset], buffer[offset + 1], buffer[offset + 2], buffer[offset + 3]]) as usize;
            offset += 4;
            
            let mut offsets = Vec::with_capacity(offset_count);
            for _ in 0..offset_count {
                let file_offset = u64::from_le_bytes([
                    buffer[offset], buffer[offset + 1], buffer[offset + 2], buffer[offset + 3],
                    buffer[offset + 4], buffer[offset + 5], buffer[offset + 6], buffer[offset + 7],
                ]);
                offset += 8;
                offsets.push(file_offset);
            }
            
            self.directory_index.insert(path, offsets);
        }
        
        // Read artist index (simplified)
        if offset < buffer.len() {
            let artist_count = u32::from_le_bytes([buffer[offset], buffer[offset + 1], buffer[offset + 2], buffer[offset + 3]]);
            offset += 4;
            
            self.artist_index.clear();
            for _ in 0..artist_count {
                if offset + 4 > buffer.len() { break; }
                
                let artist_len = u32::from_le_bytes([buffer[offset], buffer[offset + 1], buffer[offset + 2], buffer[offset + 3]]) as usize;
                offset += 4;
                
                if offset + artist_len > buffer.len() { break; }
                
                let artist = String::from_utf8(buffer[offset..offset + artist_len].to_vec())?;
                offset += artist_len;
                
                if offset + 4 > buffer.len() { break; }
                
                let offset_count = u32::from_le_bytes([buffer[offset], buffer[offset + 1], buffer[offset + 2], buffer[offset + 3]]) as usize;
                offset += 4;
                
                let mut offsets = Vec::with_capacity(offset_count);
                for _ in 0..offset_count {
                    if offset + 8 > buffer.len() { break; }
                    
                    let file_offset = u64::from_le_bytes([
                        buffer[offset], buffer[offset + 1], buffer[offset + 2], buffer[offset + 3],
                        buffer[offset + 4], buffer[offset + 5], buffer[offset + 6], buffer[offset + 7],
                    ]);
                    offset += 8;
                    offsets.push(file_offset);
                }
                
                self.artist_index.insert(artist, offsets);
            }
        }
        
        // Update generation and mark as clean
        self.generation.store(generation, Ordering::Relaxed);
        self.mark_clean();
        
        let load_time = start_time.elapsed();
        info!(
            "Loaded {} directory entries and {} artist entries in {:?}",
            self.directory_index.len(),
            self.artist_index.len(),
            load_time
        );
        
        Ok(())
    }
    
    /// Clear all indexes and reset counters
    pub fn clear(&mut self) {
        self.cache_manager.clear_all();
        self.directory_index.clear();
        self.artist_index.clear();
        self.album_index.clear();
        self.genre_index.clear();
        self.year_index.clear();
        self.album_artist_index.clear();
        
        // Reset atomic counters
        self.lookup_count.store(0, Ordering::Relaxed);
        self.update_count.store(0, Ordering::Relaxed);
        self.insert_count.store(0, Ordering::Relaxed);
        self.remove_count.store(0, Ordering::Relaxed);
        self.generation.store(1, Ordering::Relaxed);
        self.dirty_flags.store(0, Ordering::Relaxed);
        self.last_persistence.store(0, Ordering::Relaxed);
    }
    
    /// Load indexes from file with atomic operations
    pub async fn load_from_file(&mut self, file_path: &std::path::Path) -> anyhow::Result<usize> {
        use tokio::fs;
        // use std::io::Read;
        
        if !file_path.exists() {
            return Ok(0); // No file to load
        }
        
        let data = fs::read(file_path).await?;
        if data.is_empty() {
            return Ok(0);
        }
        
        // Simple binary format: [entry_count][entries...]
        // Each entry: [key_len][key][offset]
        let mut cursor = std::io::Cursor::new(data);
        let mut buffer = [0u8; 8];
        
        // Read entry count
        std::io::Read::read_exact(&mut cursor, &mut buffer)?;
        let entry_count = u64::from_le_bytes(buffer) as usize;
        
        let mut loaded_entries = 0;
        
        // Load path-to-offset entries
        for _ in 0..entry_count {
            // Read key length
            let mut key_len_buf = [0u8; 4];
            if std::io::Read::read_exact(&mut cursor, &mut key_len_buf).is_err() {
                break;
            }
            let key_len = u32::from_le_bytes(key_len_buf) as usize;
            
            // Read key
            let mut key_buf = vec![0u8; key_len];
            if std::io::Read::read_exact(&mut cursor, &mut key_buf).is_err() {
                break;
            }
            let key = String::from_utf8_lossy(&key_buf).to_string();
            
            // Read offset
            let mut offset_buf = [0u8; 8];
            if std::io::Read::read_exact(&mut cursor, &mut offset_buf).is_err() {
                break;
            }
            let offset = u64::from_le_bytes(offset_buf);
            
            // Insert into cache
            if self.cache_manager.insert_path(key, offset).is_ok() {
                loaded_entries += 1;
            }
        }
        
        // Update generation counter
        self.generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        Ok(loaded_entries)
    }
    
    /// Save indexes to file with atomic operations
    pub async fn save_to_file(&self, file_path: &std::path::Path) -> anyhow::Result<usize> {
        use tokio::fs;
        // use std::io::Write;
        
        // Create parent directory if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        let mut buffer = Vec::new();
        let mut saved_entries = 0;
        
        // Get all path cache entries
        let path_entries = self.cache_manager.get_all_path_entries();
        
        // Write entry count
        std::io::Write::write_all(&mut buffer, &(path_entries.len() as u64).to_le_bytes())?;
        
        // Write entries
        for (key, offset) in path_entries {
            // Write key length
            std::io::Write::write_all(&mut buffer, &(key.len() as u32).to_le_bytes())?;
            
            // Write key
            std::io::Write::write_all(&mut buffer, key.as_bytes())?;
            
            // Write offset
            std::io::Write::write_all(&mut buffer, &offset.to_le_bytes())?;
            
            saved_entries += 1;
        }
        
        // Write to file atomically
        let temp_path = file_path.with_extension("tmp");
        fs::write(&temp_path, &buffer).await?;
        fs::rename(&temp_path, file_path).await?;
        
        Ok(saved_entries)
    }
    
    /// Optimize indexes for better performance
    pub fn optimize_indexes(&mut self) -> IndexOptimizationResult {
        let start_time = std::time::Instant::now();
        let mut operations_performed = 0;
        
        // Trigger cache cleanup and optimization
        let cleanup_result = self.cache_manager.cleanup_expired_entries();
        operations_performed += cleanup_result.entries_removed;
        
        // Compact directory indexes (remove empty entries)
        let original_dir_count = self.directory_index.len();
        self.directory_index.retain(|_, files| !files.is_empty());
        let removed_dirs = original_dir_count - self.directory_index.len();
        operations_performed += removed_dirs;
        
        // Compact music indexes
        let original_artist_count = self.artist_index.len();
        self.artist_index.retain(|_, files| !files.is_empty());
        operations_performed += original_artist_count - self.artist_index.len();
        
        let original_album_count = self.album_index.len();
        self.album_index.retain(|_, files| !files.is_empty());
        operations_performed += original_album_count - self.album_index.len();
        
        let original_genre_count = self.genre_index.len();
        self.genre_index.retain(|_, files| !files.is_empty());
        operations_performed += original_genre_count - self.genre_index.len();
        
        let original_year_count = self.year_index.len();
        self.year_index.retain(|_, files| !files.is_empty());
        operations_performed += original_year_count - self.year_index.len();
        
        // Update generation counter
        self.generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        IndexOptimizationResult {
            operations_performed,
            total_time_ms: start_time.elapsed().as_millis() as f64,
            memory_freed: cleanup_result.memory_freed,
        }
    }
    
    /// Clear all indexes for repair operations
    pub fn clear_all_indexes(&mut self) {
        // Clear all in-memory indexes
        self.directory_index.clear();
        self.artist_index.clear();
        self.album_index.clear();
        self.genre_index.clear();
        self.year_index.clear();
        self.album_artist_index.clear();
        
        // Clear all caches
        self.cache_manager.clear_all();
        
        // Reset atomic counters
        self.lookup_count.store(0, Ordering::Relaxed);
        self.update_count.store(0, Ordering::Relaxed);
        self.insert_count.store(0, Ordering::Relaxed);
        self.remove_count.store(0, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.dirty_flags.store(0, Ordering::Relaxed);
        
        info!("All indexes cleared for repair");
    }
    
    /// Get all file offsets for streaming all files
    pub fn get_all_file_offsets(&mut self) -> Vec<u64> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        
        // Collect all unique offsets from all indexes
        let mut all_offsets = std::collections::HashSet::new();
        
        // Add offsets from directory index
        for offsets in self.directory_index.values() {
            all_offsets.extend(offsets);
        }
        
        // Add offsets from artist index
        for offsets in self.artist_index.values() {
            all_offsets.extend(offsets);
        }
        
        // Add offsets from album index
        for offsets in self.album_index.values() {
            all_offsets.extend(offsets);
        }
        
        // Add offsets from genre index
        for offsets in self.genre_index.values() {
            all_offsets.extend(offsets);
        }
        
        // Add offsets from year index
        for offsets in self.year_index.values() {
            all_offsets.extend(offsets);
        }
        
        // Convert to sorted vector for consistent ordering
        let mut result: Vec<u64> = all_offsets.into_iter().collect();
        result.sort();
        result
    }
}

/// Result of index optimization operation
#[derive(Debug)]
pub struct IndexOptimizationResult {
    pub operations_performed: usize,
    pub total_time_ms: f64,
    pub memory_freed: usize,
}

/// Comprehensive index statistics
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub path_entries: usize,
    pub id_entries: usize,
    pub directory_entries: usize,
    pub artist_entries: usize,
    pub album_entries: usize,
    pub genre_entries: usize,
    pub year_entries: usize,
    pub album_artist_entries: usize,
    pub generation: u64,
    pub is_dirty: bool,
    pub max_entries: usize,
    pub max_memory_bytes: usize,
    pub current_memory_bytes: usize,
    pub lookup_count: u64,
    pub update_count: u64,
    pub insert_count: u64,
    pub remove_count: u64,
    pub path_cache_hit_rate: f64,
    pub id_cache_hit_rate: f64,
    pub path_cache_evictions: u64,
    pub id_cache_evictions: u64,
}

impl IndexStats {
    /// Get overall cache hit rate
    pub fn overall_cache_hit_rate(&self) -> f64 {
        (self.path_cache_hit_rate + self.id_cache_hit_rate) / 2.0
    }
    
    /// Get memory utilization percentage
    pub fn memory_utilization(&self) -> f64 {
        if self.max_memory_bytes > 0 {
            (self.current_memory_bytes as f64 / self.max_memory_bytes as f64) * 100.0
        } else {
            0.0
        }
    }
    
    /// Get total operations count
    pub fn total_operations(&self) -> u64 {
        self.lookup_count + self.update_count + self.insert_count + self.remove_count
    }
}

// Include tests
#[path = "index_manager_tests.rs"]
mod tests;
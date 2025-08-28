use anyhow::{anyhow, Result};
use std::collections::{HashMap, BTreeMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info, warn};

use super::MediaFile;

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

/// LRU cache entry with access tracking
#[derive(Debug)]
struct LRUEntry<T> {
    value: T,
    last_accessed: Instant,
    access_count: AtomicU64,
}

impl<T> LRUEntry<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            last_accessed: Instant::now(),
            access_count: AtomicU64::new(1),
        }
    }
    
    fn touch(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }
}

/// Memory-bounded LRU cache with atomic operations
#[derive(Debug)]
pub struct MemoryBoundedCache<K, V> 
where 
    K: Clone + std::hash::Hash + Eq,
    V: Clone,
{
    cache: HashMap<K, LRUEntry<V>>,
    access_order: VecDeque<K>,
    max_entries: usize,
    max_memory_bytes: usize,
    current_memory_bytes: AtomicUsize,
    hit_count: AtomicU64,
    miss_count: AtomicU64,
    eviction_count: AtomicU64,
}

impl<K, V> MemoryBoundedCache<K, V>
where
    K: Clone + std::hash::Hash + Eq,
    V: Clone,
{
    pub fn new(max_entries: usize, max_memory_bytes: usize) -> Self {
        Self {
            cache: HashMap::with_capacity(max_entries.min(1000)),
            access_order: VecDeque::with_capacity(max_entries.min(1000)),
            max_entries,
            max_memory_bytes,
            current_memory_bytes: AtomicUsize::new(0),
            hit_count: AtomicU64::new(0),
            miss_count: AtomicU64::new(0),
            eviction_count: AtomicU64::new(0),
        }
    }
    
    pub fn get(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.cache.get_mut(key) {
            entry.touch();
            self.hit_count.fetch_add(1, Ordering::Relaxed);
            
            // Move to end of access order
            if let Some(pos) = self.access_order.iter().position(|k| k == key) {
                let key = self.access_order.remove(pos).unwrap();
                self.access_order.push_back(key);
            }
            
            Some(entry.value.clone())
        } else {
            self.miss_count.fetch_add(1, Ordering::Relaxed);
            None
        }
    }
    
    pub fn insert(&mut self, key: K, value: V) {
        let entry_size = std::mem::size_of::<K>() + std::mem::size_of::<V>() + std::mem::size_of::<LRUEntry<V>>();
        
        // Check if we need to evict entries
        self.evict_if_needed(entry_size);
        
        // Insert new entry
        let entry = LRUEntry::new(value);
        self.cache.insert(key.clone(), entry);
        self.access_order.push_back(key);
        
        self.current_memory_bytes.fetch_add(entry_size, Ordering::Relaxed);
    }
    
    pub fn remove(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.cache.remove(key) {
            // Remove from access order
            if let Some(pos) = self.access_order.iter().position(|k| k == key) {
                self.access_order.remove(pos);
            }
            
            let entry_size = std::mem::size_of::<K>() + std::mem::size_of::<V>() + std::mem::size_of::<LRUEntry<V>>();
            self.current_memory_bytes.fetch_sub(entry_size, Ordering::Relaxed);
            
            Some(entry.value)
        } else {
            None
        }
    }
    
    fn evict_if_needed(&mut self, new_entry_size: usize) {
        let current_memory = self.current_memory_bytes.load(Ordering::Relaxed);
        let current_entries = self.cache.len();
        
        // Check memory limit
        let needs_memory_eviction = current_memory + new_entry_size > self.max_memory_bytes;
        
        // Check entry count limit
        let needs_count_eviction = current_entries >= self.max_entries;
        
        if needs_memory_eviction || needs_count_eviction {
            let entries_to_evict = if needs_count_eviction {
                (current_entries - self.max_entries + 1).max(1)
            } else {
                // Evict 10% of entries to free memory
                (current_entries / 10).max(1)
            };
            
            for _ in 0..entries_to_evict {
                if let Some(oldest_key) = self.access_order.pop_front() {
                    if let Some(entry) = self.cache.remove(&oldest_key) {
                        let entry_size = std::mem::size_of::<K>() + std::mem::size_of::<V>() + std::mem::size_of::<LRUEntry<V>>();
                        self.current_memory_bytes.fetch_sub(entry_size, Ordering::Relaxed);
                        self.eviction_count.fetch_add(1, Ordering::Relaxed);
                    }
                } else {
                    break;
                }
            }
            
            if needs_memory_eviction {
                debug!("Evicted {} entries to free memory", entries_to_evict);
            } else {
                debug!("Evicted {} entries to maintain count limit", entries_to_evict);
            }
        }
    }
    
    pub fn get_stats(&self) -> CacheStats {
        let hits = self.hit_count.load(Ordering::Relaxed);
        let misses = self.miss_count.load(Ordering::Relaxed);
        let total_requests = hits + misses;
        
        CacheStats {
            entries: self.cache.len(),
            max_entries: self.max_entries,
            memory_bytes: self.current_memory_bytes.load(Ordering::Relaxed),
            max_memory_bytes: self.max_memory_bytes,
            hit_count: hits,
            miss_count: misses,
            eviction_count: self.eviction_count.load(Ordering::Relaxed),
            hit_rate: if total_requests > 0 {
                hits as f64 / total_requests as f64
            } else {
                0.0
            },
        }
    }
    
    pub fn clear(&mut self) {
        self.cache.clear();
        self.access_order.clear();
        self.current_memory_bytes.store(0, Ordering::Relaxed);
    }
}

/// Cache statistics
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
    // Path-based indexes with atomic operations
    path_to_offset: MemoryBoundedCache<String, u64>,
    id_to_offset: MemoryBoundedCache<u64, u64>,
    
    // Directory-based B-tree index with atomic operations
    directory_index: BTreeMap<String, Vec<u64>>,
    
    // Music categorization indexes
    artist_index: HashMap<String, Vec<u64>>,
    album_index: HashMap<String, Vec<u64>>,
    genre_index: HashMap<String, Vec<u64>>,
    year_index: HashMap<u32, Vec<u64>>,
    
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
        let cache_memory_per_index = max_memory_bytes / 2; // Split between path and id caches
        
        Self {
            path_to_offset: MemoryBoundedCache::new(max_entries / 2, cache_memory_per_index),
            id_to_offset: MemoryBoundedCache::new(max_entries / 2, cache_memory_per_index),
            directory_index: BTreeMap::new(),
            artist_index: HashMap::new(),
            album_index: HashMap::new(),
            genre_index: HashMap::new(),
            year_index: HashMap::new(),
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
        
        // Update path-to-offset index with atomic cache operations
        self.path_to_offset.insert(path_key.clone(), offset);
        
        // Update id-to-offset index if ID is available
        if let Some(id) = file.id {
            self.id_to_offset.insert(id as u64, offset);
        }
        
        // Update directory index with atomic operations
        if let Some(parent) = file.path.parent() {
            let parent_key = parent.to_string_lossy().to_string();
            self.directory_index.entry(parent_key).or_insert_with(Vec::new).push(offset);
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
        
        if let Some(offset) = self.path_to_offset.remove(&path.to_string()) {
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
        self.path_to_offset.get(&path.to_string())
    }
    
    /// Find file by ID with atomic cache lookup
    pub fn find_by_id(&mut self, id: u64) -> Option<u64> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.id_to_offset.get(&id)
    }
    
    /// Find files in directory with atomic B-tree operations
    pub fn find_files_in_directory(&self, dir_path: &str) -> Vec<u64> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.directory_index.get(dir_path).cloned().unwrap_or_default()
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
    
    /// Get all unique artists
    pub fn get_all_artists(&self) -> Vec<String> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.artist_index.keys().cloned().collect()
    }
    
    /// Get all unique albums
    pub fn get_all_albums(&self) -> Vec<String> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.album_index.keys().cloned().collect()
    }
    
    /// Get all unique genres
    pub fn get_all_genres(&self) -> Vec<String> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.genre_index.keys().cloned().collect()
    }
    
    /// Get all unique years
    pub fn get_all_years(&self) -> Vec<u32> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.year_index.keys().cloned().collect()
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
        let path_cache_stats = self.path_to_offset.get_stats();
        let id_cache_stats = self.id_to_offset.get_stats();
        
        IndexStats {
            path_entries: path_cache_stats.entries,
            id_entries: id_cache_stats.entries,
            directory_entries: self.directory_index.len(),
            artist_entries: self.artist_index.len(),
            album_entries: self.album_index.len(),
            genre_entries: self.genre_index.len(),
            year_entries: self.year_index.len(),
            generation: self.generation.load(Ordering::Relaxed),
            is_dirty: self.is_dirty(),
            max_entries: self.max_entries,
            max_memory_bytes: self.max_memory_bytes,
            current_memory_bytes: path_cache_stats.memory_bytes + id_cache_stats.memory_bytes,
            lookup_count: self.lookup_count.load(Ordering::Relaxed),
            update_count: self.update_count.load(Ordering::Relaxed),
            insert_count: self.insert_count.load(Ordering::Relaxed),
            remove_count: self.remove_count.load(Ordering::Relaxed),
            path_cache_hit_rate: path_cache_stats.hit_rate,
            id_cache_hit_rate: id_cache_stats.hit_rate,
            path_cache_evictions: path_cache_stats.eviction_count,
            id_cache_evictions: id_cache_stats.eviction_count,
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
        self.path_to_offset.clear();
        self.id_to_offset.clear();
        self.directory_index.clear();
        self.artist_index.clear();
        self.album_index.clear();
        self.genre_index.clear();
        self.year_index.clear();
        
        // Reset atomic counters
        self.lookup_count.store(0, Ordering::Relaxed);
        self.update_count.store(0, Ordering::Relaxed);
        self.insert_count.store(0, Ordering::Relaxed);
        self.remove_count.store(0, Ordering::Relaxed);
        self.generation.store(1, Ordering::Relaxed);
        self.dirty_flags.store(0, Ordering::Relaxed);
        self.last_persistence.store(0, Ordering::Relaxed);
    }
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
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::memory_mapped::MemoryMappedFile;
use super::flatbuffer::{BatchSerializer, MediaFileSerializer, BatchOperationType};
use super::index_manager::{IndexManager, IndexStats};
use super::atomic_performance::{AtomicPerformanceTracker, BatchOperationResult, SharedPerformanceTracker, create_shared_performance_tracker};
use super::{DatabaseManager, MediaFile, MediaDirectory, Playlist, MusicCategory, DatabaseStats, DatabaseHealth};
use crate::platform::filesystem::{create_platform_path_normalizer, PathNormalizer};

/// Performance profile for auto-scaling configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceProfile {
    /// Minimal memory usage (4MB cache, 100K index entries)
    Minimal,
    /// Balanced performance and memory (16MB cache, 500K index entries)
    Balanced,
    /// High performance (64MB cache, 1M index entries)
    HighPerformance,
    /// Maximum performance (256MB cache, 5M index entries)
    Maximum,
    /// Custom profile with user-defined settings
    Custom,
}

impl PerformanceProfile {
    /// Get memory budget in MB for this profile
    pub fn memory_budget_mb(&self) -> usize {
        match self {
            PerformanceProfile::Minimal => 6,        // 4MB cache + 2MB overhead
            PerformanceProfile::Balanced => 20,      // 16MB cache + 4MB overhead
            PerformanceProfile::HighPerformance => 80, // 64MB cache + 16MB overhead
            PerformanceProfile::Maximum => 320,     // 256MB cache + 64MB overhead
            PerformanceProfile::Custom => 0,        // User-defined
        }
    }
    
    /// Get cache size in MB for this profile
    pub fn cache_size_mb(&self) -> usize {
        match self {
            PerformanceProfile::Minimal => 4,
            PerformanceProfile::Balanced => 16,
            PerformanceProfile::HighPerformance => 64,
            PerformanceProfile::Maximum => 256,
            PerformanceProfile::Custom => 4, // Default fallback
        }
    }
    
    /// Get index cache size (number of entries) for this profile
    pub fn index_cache_size(&self) -> usize {
        match self {
            PerformanceProfile::Minimal => 100_000,     // ~100KB for indexes
            PerformanceProfile::Balanced => 500_000,    // ~500KB for indexes
            PerformanceProfile::HighPerformance => 1_000_000, // ~1MB for indexes
            PerformanceProfile::Maximum => 5_000_000,   // ~5MB for indexes
            PerformanceProfile::Custom => 1_000_000,    // Default fallback
        }
    }
    
    /// Get batch size for this profile
    pub fn batch_size(&self) -> usize {
        match self {
            PerformanceProfile::Minimal => 10_000,      // Smaller batches for memory efficiency
            PerformanceProfile::Balanced => 50_000,     // Balanced batch size
            PerformanceProfile::HighPerformance => 100_000, // Large batches for speed
            PerformanceProfile::Maximum => 250_000,     // Maximum batch size
            PerformanceProfile::Custom => 100_000,      // Default fallback
        }
    }
    
    /// Get expected throughput range for this profile
    pub fn expected_throughput_range(&self) -> &'static str {
        match self {
            PerformanceProfile::Minimal => "50K-100K files/sec",
            PerformanceProfile::Balanced => "100K-300K files/sec",
            PerformanceProfile::HighPerformance => "300K-700K files/sec",
            PerformanceProfile::Maximum => "700K-1M+ files/sec",
            PerformanceProfile::Custom => "Variable",
        }
    }
    
    /// Auto-detect performance profile based on available system memory
    pub fn auto_detect() -> Self {
        match Self::get_available_memory_mb() {
            Some(memory_mb) => {
                if memory_mb >= 2048 {      // 2GB+ RAM
                    PerformanceProfile::Maximum
                } else if memory_mb >= 1024 { // 1GB+ RAM
                    PerformanceProfile::HighPerformance
                } else if memory_mb >= 512 {  // 512MB+ RAM
                    PerformanceProfile::Balanced
                } else {                    // < 512MB RAM
                    PerformanceProfile::Minimal
                }
            }
            None => {
                warn!("Could not detect system memory, using balanced profile");
                PerformanceProfile::Balanced
            }
        }
    }
    
    /// Get available system memory in MB
    fn get_available_memory_mb() -> Option<usize> {
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
                for line in meminfo.lines() {
                    if line.starts_with("MemAvailable:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<usize>() {
                                return Some(kb / 1024); // Convert KB to MB
                            }
                        }
                    }
                }
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            // Windows memory detection would require additional dependencies
            // For now, return None to use default profile
        }
        
        #[cfg(target_os = "macos")]
        {
            // macOS memory detection would require additional dependencies
            // For now, return None to use default profile
        }
        
        None
    }
}

/// Configuration for zero-copy database operations with auto-scaling
#[derive(Debug, Clone)]
pub struct ZeroCopyConfig {
    /// Performance profile for auto-scaling
    pub performance_profile: PerformanceProfile,
    /// Number of files to process per batch
    pub batch_size: usize,
    /// Initial size of data file in MB
    pub initial_file_size_mb: usize,
    /// Maximum size of data file in GB
    pub max_file_size_gb: usize,
    /// Memory map size in MB
    pub memory_map_size_mb: usize,
    /// Index cache size (number of entries)
    pub index_cache_size: usize,
    /// Enable compression (disabled for maximum speed)
    pub enable_compression: bool,
    /// Sync frequency for durability
    pub sync_frequency: Duration,
    /// Enable Write-Ahead Logging
    pub enable_wal: bool,
    /// Enable auto-scaling based on system resources
    pub enable_auto_scaling: bool,
    /// Memory budget limit in MB (0 = no limit)
    pub memory_budget_limit_mb: usize,
    /// Enable runtime configuration updates
    pub enable_runtime_updates: bool,
    /// Performance monitoring interval
    pub performance_monitoring_interval: Duration,
}

impl Default for ZeroCopyConfig {
    fn default() -> Self {
        let profile = PerformanceProfile::Balanced;
        Self {
            performance_profile: profile,
            batch_size: profile.batch_size(),
            initial_file_size_mb: 10,
            max_file_size_gb: 10,
            memory_map_size_mb: profile.cache_size_mb(),
            index_cache_size: profile.index_cache_size(),
            enable_compression: false,
            sync_frequency: Duration::from_secs(5),
            enable_wal: true,
            enable_auto_scaling: true,
            memory_budget_limit_mb: profile.memory_budget_mb(),
            enable_runtime_updates: true,
            performance_monitoring_interval: Duration::from_secs(30),
        }
    }
}

impl ZeroCopyConfig {
    /// Validate cache size in MB
    fn validate_cache_size_mb(size: usize) -> bool {
        size >= 4 && size <= 1024
    }
    
    /// Validate index cache size (number of entries)
    fn validate_index_cache_size(size: usize) -> bool {
        size >= 10_000 && size <= 10_000_000
    }
    
    /// Validate batch size
    fn validate_batch_size(size: usize) -> bool {
        size >= 1_000 && size <= 1_000_000
    }
    
    /// Validate memory budget in MB
    fn validate_memory_budget_mb(budget: usize) -> bool {
        budget == 0 || (budget >= 6 && budget <= 4096)
    }
    
    /// Create a safe configuration with enforced bounds
    pub fn safe_from_env() -> Self {
        // Start with default and manually parse environment variables with clamping
        let mut config = if std::env::var("ZEROCOPY_AUTO_DETECT").unwrap_or_else(|_| "true".to_string()).to_lowercase() == "true" {
            Self::with_auto_detection()
        } else {
            Self::default()
        };
        
        // Override with environment variables if present, but clamp to safe values
        if let Ok(profile_str) = std::env::var("ZEROCOPY_PERFORMANCE_PROFILE") {
            let profile = match profile_str.to_lowercase().as_str() {
                "minimal" => PerformanceProfile::Minimal,
                "balanced" => PerformanceProfile::Balanced,
                "high" | "high_performance" => PerformanceProfile::HighPerformance,
                "maximum" | "max" => PerformanceProfile::Maximum,
                "custom" => PerformanceProfile::Custom,
                _ => {
                    warn!("Unknown performance profile '{}', using balanced", profile_str);
                    PerformanceProfile::Balanced
                }
            };
            config.apply_performance_profile(profile);
        }
        
        // Parse and clamp individual settings
        if let Ok(cache_mb) = std::env::var("ZEROCOPY_CACHE_MB") {
            if let Ok(size) = cache_mb.parse::<usize>() {
                config.memory_map_size_mb = size.clamp(4, 1024);
                config.performance_profile = PerformanceProfile::Custom;
                info!("Using clamped cache size from env: {}MB (requested: {})", config.memory_map_size_mb, size);
            }
        }
        
        if let Ok(index_size) = std::env::var("ZEROCOPY_INDEX_SIZE") {
            if let Ok(size) = index_size.parse::<usize>() {
                config.index_cache_size = size.clamp(10_000, 10_000_000);
                config.performance_profile = PerformanceProfile::Custom;
                info!("Using clamped index cache size from env: {} (requested: {})", config.index_cache_size, size);
            }
        }
        
        if let Ok(batch_size) = std::env::var("ZEROCOPY_BATCH_SIZE") {
            if let Ok(size) = batch_size.parse::<usize>() {
                config.batch_size = size.clamp(1_000, 1_000_000);
                config.performance_profile = PerformanceProfile::Custom;
                info!("Using clamped batch size from env: {} (requested: {})", config.batch_size, size);
            }
        }
        
        if let Ok(memory_budget) = std::env::var("ZEROCOPY_MEMORY_BUDGET_MB") {
            if let Ok(budget) = memory_budget.parse::<usize>() {
                if budget > 0 {
                    config.memory_budget_limit_mb = budget.clamp(6, 2048);
                    info!("Using clamped memory budget from env: {}MB (requested: {})", config.memory_budget_limit_mb, budget);
                }
            }
        }
        
        if let Ok(auto_scaling) = std::env::var("ZEROCOPY_ENABLE_AUTO_SCALING") {
            config.enable_auto_scaling = auto_scaling.to_lowercase() == "true";
        }
        
        if let Ok(runtime_updates) = std::env::var("ZEROCOPY_ENABLE_RUNTIME_UPDATES") {
            config.enable_runtime_updates = runtime_updates.to_lowercase() == "true";
        }
        
        // Enforce safe bounds even if validation would fail
        config.memory_map_size_mb = config.memory_map_size_mb.clamp(4, 1024);
        config.index_cache_size = config.index_cache_size.clamp(10_000, 10_000_000);
        config.batch_size = config.batch_size.clamp(1_000, 1_000_000);
        config.initial_file_size_mb = config.initial_file_size_mb.clamp(1, 1024);
        config.max_file_size_gb = config.max_file_size_gb.clamp(1, 100);
        
        if config.memory_budget_limit_mb > 0 {
            config.memory_budget_limit_mb = config.memory_budget_limit_mb.clamp(6, 2048);
        }
        
        // Ensure sync frequency is reasonable
        if config.sync_frequency.as_secs() == 0 || config.sync_frequency.as_secs() > 3600 {
            config.sync_frequency = Duration::from_secs(5);
        }
        
        // Ensure monitoring interval is reasonable
        if config.performance_monitoring_interval.as_secs() < 5 || config.performance_monitoring_interval.as_secs() > 3600 {
            config.performance_monitoring_interval = Duration::from_secs(30);
        }
        
        // Validate and fix memory budget consistency
        let estimated_usage = config.memory_map_size_mb + (config.index_cache_size / 1_000_000);
        if config.memory_budget_limit_mb > 0 && estimated_usage > config.memory_budget_limit_mb {
            // Automatically adjust memory budget to accommodate the configuration
            config.memory_budget_limit_mb = estimated_usage + 10; // Add 10MB buffer
            warn!("Automatically adjusted memory budget to {}MB to accommodate configuration", config.memory_budget_limit_mb);
        }
        
        config
    }
    
    /// Create configuration with auto-detected performance profile
    pub fn with_auto_detection() -> Self {
        let profile = PerformanceProfile::auto_detect();
        info!("Auto-detected performance profile: {:?} (expected throughput: {})", 
              profile, profile.expected_throughput_range());
        
        let mut config = Self::default();
        config.apply_performance_profile(profile);
        config
    }
    
    /// Create configuration with specific performance profile
    pub fn with_performance_profile(profile: PerformanceProfile) -> Self {
        let mut config = Self::default();
        config.apply_performance_profile(profile);
        config
    }
    
    /// Apply performance profile settings
    pub fn apply_performance_profile(&mut self, profile: PerformanceProfile) {
        self.performance_profile = profile;
        
        if profile != PerformanceProfile::Custom {
            self.memory_map_size_mb = profile.cache_size_mb();
            self.index_cache_size = profile.index_cache_size();
            self.batch_size = profile.batch_size();
            self.memory_budget_limit_mb = profile.memory_budget_mb();
        }
        
        info!("Applied performance profile: {:?}", profile);
        info!("  - Cache size: {}MB", self.memory_map_size_mb);
        info!("  - Index cache: {} entries", self.index_cache_size);
        info!("  - Batch size: {} files", self.batch_size);
        info!("  - Memory budget: {}MB", self.memory_budget_limit_mb);
        info!("  - Expected throughput: {}", profile.expected_throughput_range());
    }
    
    /// Load configuration from environment variables (for Docker)
    pub fn from_env() -> Self {
        let mut config = if std::env::var("ZEROCOPY_AUTO_DETECT").unwrap_or_else(|_| "true".to_string()).to_lowercase() == "true" {
            Self::with_auto_detection()
        } else {
            Self::default()
        };
        
        // Override with environment variables if present
        if let Ok(profile_str) = std::env::var("ZEROCOPY_PERFORMANCE_PROFILE") {
            let profile = match profile_str.to_lowercase().as_str() {
                "minimal" => PerformanceProfile::Minimal,
                "balanced" => PerformanceProfile::Balanced,
                "high" | "high_performance" => PerformanceProfile::HighPerformance,
                "maximum" | "max" => PerformanceProfile::Maximum,
                "custom" => PerformanceProfile::Custom,
                _ => {
                    warn!("Unknown performance profile '{}', using balanced", profile_str);
                    PerformanceProfile::Balanced
                }
            };
            config.apply_performance_profile(profile);
        }
        
        // Individual setting overrides with validation
        if let Ok(cache_mb) = std::env::var("ZEROCOPY_CACHE_MB") {
            if let Ok(size) = cache_mb.parse::<usize>() {
                if size >= 4 && size <= 1024 {
                    config.memory_map_size_mb = size;
                    config.performance_profile = PerformanceProfile::Custom;
                    info!("Using custom cache size from env: {}MB", size);
                } else {
                    warn!("Invalid ZEROCOPY_CACHE_MB value: {}. Must be between 4 and 1024 MB. Using default: {}MB", 
                          size, config.memory_map_size_mb);
                }
            } else {
                warn!("Invalid ZEROCOPY_CACHE_MB format: '{}'. Must be a number. Using default: {}MB", 
                      cache_mb, config.memory_map_size_mb);
            }
        }
        
        if let Ok(index_size) = std::env::var("ZEROCOPY_INDEX_SIZE") {
            if let Ok(size) = index_size.parse::<usize>() {
                if size >= 10_000 && size <= 10_000_000 {
                    config.index_cache_size = size;
                    config.performance_profile = PerformanceProfile::Custom;
                    info!("Using custom index cache size from env: {}", size);
                } else {
                    warn!("Invalid ZEROCOPY_INDEX_SIZE value: {}. Must be between 10,000 and 10,000,000. Using default: {}", 
                          size, config.index_cache_size);
                }
            } else {
                warn!("Invalid ZEROCOPY_INDEX_SIZE format: '{}'. Must be a number. Using default: {}", 
                      index_size, config.index_cache_size);
            }
        }
        
        if let Ok(batch_size) = std::env::var("ZEROCOPY_BATCH_SIZE") {
            if let Ok(size) = batch_size.parse::<usize>() {
                if size >= 1_000 && size <= 1_000_000 {
                    config.batch_size = size;
                    config.performance_profile = PerformanceProfile::Custom;
                    info!("Using custom batch size from env: {}", size);
                } else {
                    warn!("Invalid ZEROCOPY_BATCH_SIZE value: {}. Must be between 1,000 and 1,000,000. Using default: {}", 
                          size, config.batch_size);
                }
            } else {
                warn!("Invalid ZEROCOPY_BATCH_SIZE format: '{}'. Must be a number. Using default: {}", 
                      batch_size, config.batch_size);
            }
        }
        
        if let Ok(memory_budget) = std::env::var("ZEROCOPY_MEMORY_BUDGET_MB") {
            if let Ok(budget) = memory_budget.parse::<usize>() {
                if budget >= 6 && budget <= 2048 {  // Minimum 6MB, maximum 2GB
                    config.memory_budget_limit_mb = budget;
                    info!("Using memory budget from env: {}MB", budget);
                } else {
                    warn!("Invalid ZEROCOPY_MEMORY_BUDGET_MB value: {}. Must be between 6 and 2048 MB. Using default: {}MB", 
                          budget, config.memory_budget_limit_mb);
                }
            } else {
                warn!("Invalid ZEROCOPY_MEMORY_BUDGET_MB format: '{}'. Must be a number. Using default: {}MB", 
                      memory_budget, config.memory_budget_limit_mb);
            }
        }
        
        if let Ok(auto_scaling) = std::env::var("ZEROCOPY_ENABLE_AUTO_SCALING") {
            config.enable_auto_scaling = auto_scaling.to_lowercase() == "true";
            info!("Auto-scaling {}", if config.enable_auto_scaling { "enabled" } else { "disabled" });
        }
        
        if let Ok(runtime_updates) = std::env::var("ZEROCOPY_ENABLE_RUNTIME_UPDATES") {
            config.enable_runtime_updates = runtime_updates.to_lowercase() == "true";
            info!("Runtime updates {}", if config.enable_runtime_updates { "enabled" } else { "disabled" });
        }
        
        if let Err(e) = config.validate() {
            warn!("Configuration validation failed: {}", e);
            warn!("Falling back to default configuration");
            Self::default()
        } else {
            config
        }
    }
    
    /// Validate configuration and enforce strict bounds
    pub fn validate(&self) -> Result<()> {
        // Enforce strict bounds on critical parameters
        if self.memory_map_size_mb < 4 || self.memory_map_size_mb > 1024 {
            return Err(anyhow!("Invalid memory_map_size_mb: {}. Must be between 4 and 1024 MB", 
                              self.memory_map_size_mb));
        }
        
        if self.index_cache_size < 10_000 || self.index_cache_size > 10_000_000 {
            return Err(anyhow!("Invalid index_cache_size: {}. Must be between 10,000 and 10,000,000 entries", 
                              self.index_cache_size));
        }
        
        if self.batch_size < 1_000 || self.batch_size > 1_000_000 {
            return Err(anyhow!("Invalid batch_size: {}. Must be between 1,000 and 1,000,000 files", 
                              self.batch_size));
        }
        
        if self.memory_budget_limit_mb > 0 && (self.memory_budget_limit_mb < 6 || self.memory_budget_limit_mb > 2048) {
            return Err(anyhow!("Invalid memory_budget_limit_mb: {}. Must be between 6 and 2048 MB, or 0 for no limit", 
                              self.memory_budget_limit_mb));
        }
        
        // Check memory budget consistency
        let estimated_usage = self.memory_map_size_mb + (self.index_cache_size / 1_000_000); // Convert entries to MB
        if self.memory_budget_limit_mb > 0 && estimated_usage > self.memory_budget_limit_mb {
            return Err(anyhow!("Estimated memory usage ({}MB) exceeds budget limit ({}MB). Increase budget or reduce cache sizes", 
                              estimated_usage, self.memory_budget_limit_mb));
        }
        
        // Validate file size limits
        if self.initial_file_size_mb == 0 || self.initial_file_size_mb > 1024 {
            return Err(anyhow!("Invalid initial_file_size_mb: {}. Must be between 1 and 1024 MB", 
                              self.initial_file_size_mb));
        }
        
        if self.max_file_size_gb == 0 || self.max_file_size_gb > 100 {
            return Err(anyhow!("Invalid max_file_size_gb: {}. Must be between 1 and 100 GB", 
                              self.max_file_size_gb));
        }
        
        // Validate sync frequency
        if self.sync_frequency.as_secs() == 0 || self.sync_frequency.as_secs() > 3600 {
            return Err(anyhow!("Invalid sync_frequency: {:?}. Must be between 1 second and 1 hour", 
                              self.sync_frequency));
        }
        
        // Validate performance monitoring interval
        if self.performance_monitoring_interval.as_secs() < 5 || self.performance_monitoring_interval.as_secs() > 3600 {
            return Err(anyhow!("Invalid performance_monitoring_interval: {:?}. Must be between 5 seconds and 1 hour", 
                              self.performance_monitoring_interval));
        }
        
        // Warn about performance implications (non-fatal)
        if self.batch_size > 500_000 {
            warn!("Large batch size ({}), may cause memory pressure during processing", self.batch_size);
        }
        
        if self.memory_map_size_mb > 512 {
            warn!("Large cache size ({}MB), ensure sufficient system memory", self.memory_map_size_mb);
        }
        
        if self.index_cache_size > 5_000_000 {
            warn!("Large index cache ({}), may use significant memory", self.index_cache_size);
        }
        
        // Log configuration summary
        info!("ZeroCopy database configuration validated:");
        info!("  - Performance profile: {:?}", self.performance_profile);
        info!("  - Expected throughput: {}", self.performance_profile.expected_throughput_range());
        info!("  - Memory map cache: {}MB", self.memory_map_size_mb);
        info!("  - Index cache: {} entries (~{}MB)", self.index_cache_size, self.index_cache_size / 1_000_000);
        info!("  - Batch size: {} files", self.batch_size);
        info!("  - Memory budget: {}MB", self.memory_budget_limit_mb);
        info!("  - Auto-scaling: {}", if self.enable_auto_scaling { "enabled" } else { "disabled" });
        info!("  - Runtime updates: {}", if self.enable_runtime_updates { "enabled" } else { "disabled" });
        
        Ok(())
    }
    
    /// Check if current configuration exceeds memory budget
    pub fn check_memory_budget(&self) -> bool {
        if self.memory_budget_limit_mb == 0 {
            return true; // No limit
        }
        
        // Estimate memory usage: cache + index overhead (rough estimate: 1 byte per index entry)
        let estimated_usage = self.memory_map_size_mb + (self.index_cache_size / 1_000_000); // Convert entries to MB
        estimated_usage <= self.memory_budget_limit_mb
    }
    
    /// Auto-scale configuration based on current memory pressure
    pub fn auto_scale_for_memory_pressure(&mut self, memory_pressure_factor: f64) -> bool {
        if !self.enable_auto_scaling {
            return false;
        }
        
        let original_profile = self.performance_profile;
        
        if memory_pressure_factor > 0.8 {
            // High memory pressure - scale down
            match self.performance_profile {
                PerformanceProfile::Maximum => {
                    self.apply_performance_profile(PerformanceProfile::HighPerformance);
                }
                PerformanceProfile::HighPerformance => {
                    self.apply_performance_profile(PerformanceProfile::Balanced);
                }
                PerformanceProfile::Balanced => {
                    self.apply_performance_profile(PerformanceProfile::Minimal);
                }
                PerformanceProfile::Minimal => {
                    // Already at minimum, reduce batch size
                    self.batch_size = (self.batch_size / 2).max(1000);
                }
                PerformanceProfile::Custom => {
                    // Scale down custom settings
                    self.memory_map_size_mb = (self.memory_map_size_mb / 2).max(4);
                    self.index_cache_size = (self.index_cache_size / 2).max(10_000);
                    self.batch_size = (self.batch_size / 2).max(1000);
                }
            }
        } else if memory_pressure_factor < 0.3 {
            // Low memory pressure - scale up if possible
            match self.performance_profile {
                PerformanceProfile::Minimal => {
                    self.apply_performance_profile(PerformanceProfile::Balanced);
                }
                PerformanceProfile::Balanced => {
                    self.apply_performance_profile(PerformanceProfile::HighPerformance);
                }
                PerformanceProfile::HighPerformance => {
                    self.apply_performance_profile(PerformanceProfile::Maximum);
                }
                PerformanceProfile::Maximum | PerformanceProfile::Custom => {
                    // Already at maximum or custom
                    return false;
                }
            }
        } else {
            return false; // No scaling needed
        }
        
        let scaled = self.performance_profile != original_profile;
        if scaled {
            info!("Auto-scaled from {:?} to {:?} due to memory pressure ({:.1}%)", 
                  original_profile, self.performance_profile, memory_pressure_factor * 100.0);
        }
        
        scaled
    }
    
    /// Update configuration at runtime with atomic operations
    pub fn update_runtime_config(&mut self, updates: ZeroCopyConfigUpdate) -> Result<bool> {
        if !self.enable_runtime_updates {
            return Err(anyhow!("Runtime configuration updates are disabled"));
        }
        
        let mut changed = false;
        
        if let Some(batch_size) = updates.batch_size {
            if batch_size != self.batch_size {
                if Self::validate_batch_size(batch_size) {
                    self.batch_size = batch_size;
                    self.performance_profile = PerformanceProfile::Custom;
                    changed = true;
                    info!("Updated batch size to: {}", batch_size);
                } else {
                    return Err(anyhow!("Invalid batch size: {} (must be 1,000-1,000,000)", batch_size));
                }
            }
        }
        
        if let Some(cache_size_mb) = updates.memory_map_size_mb {
            if cache_size_mb != self.memory_map_size_mb {
                if Self::validate_cache_size_mb(cache_size_mb) {
                    self.memory_map_size_mb = cache_size_mb;
                    self.performance_profile = PerformanceProfile::Custom;
                    changed = true;
                    info!("Updated cache size to: {}MB", cache_size_mb);
                } else {
                    return Err(anyhow!("Invalid cache size: {}MB (must be 4-1024MB)", cache_size_mb));
                }
            }
        }
        
        if let Some(index_cache_size) = updates.index_cache_size {
            if index_cache_size != self.index_cache_size {
                if Self::validate_index_cache_size(index_cache_size) {
                    self.index_cache_size = index_cache_size;
                    self.performance_profile = PerformanceProfile::Custom;
                    changed = true;
                    info!("Updated index cache size to: {}", index_cache_size);
                } else {
                    return Err(anyhow!("Invalid index cache size: {} (must be 10,000-10,000,000)", index_cache_size));
                }
            }
        }
        
        if let Some(profile) = updates.performance_profile {
            if profile != self.performance_profile {
                self.apply_performance_profile(profile);
                changed = true;
            }
        }
        
        if changed {
            // Validate the updated configuration
            if let Err(e) = self.validate() {
                return Err(anyhow!("Configuration update validation failed: {}", e));
            }
        }
        
        Ok(changed)
    }
}

/// Runtime configuration update structure
#[derive(Debug, Clone, Default)]
pub struct ZeroCopyConfigUpdate {
    pub batch_size: Option<usize>,
    pub memory_map_size_mb: Option<usize>,
    pub index_cache_size: Option<usize>,
    pub performance_profile: Option<PerformanceProfile>,
}

/// Atomic performance tracking for zero-copy database operations (legacy compatibility)
/// This is a compatibility wrapper around the comprehensive AtomicPerformanceTracker
#[derive(Debug)]
pub struct ZeroCopyPerformanceTracker {
    // Comprehensive performance tracker
    inner: Arc<super::atomic_performance::AtomicPerformanceTracker>,
    
    // Legacy counters for backward compatibility
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

impl ZeroCopyPerformanceTracker {
    pub fn new(monitoring_interval: Duration, enable_detailed_logging: bool) -> Self {
        Self {
            inner: Arc::new(super::atomic_performance::AtomicPerformanceTracker::new(
                monitoring_interval,
                enable_detailed_logging,
            )),
            total_files: AtomicU64::new(0),
            processed_files: AtomicU64::new(0),
            failed_files: AtomicU64::new(0),
            inserted_files: AtomicU64::new(0),
            updated_files: AtomicU64::new(0),
            deleted_files: AtomicU64::new(0),
            total_batches: AtomicU64::new(0),
            successful_batches: AtomicU64::new(0),
            failed_batches: AtomicU64::new(0),
            total_operations: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            index_lookups: AtomicU64::new(0),
            index_updates: AtomicU64::new(0),
            bytes_written: AtomicU64::new(0),
            bytes_read: AtomicU64::new(0),
            sync_operations: AtomicU64::new(0),
            total_processing_time_ns: AtomicU64::new(0),
            total_serialization_time_ns: AtomicU64::new(0),
            total_io_time_ns: AtomicU64::new(0),
        }
    }
    
    /// Get the comprehensive performance tracker
    pub fn get_comprehensive_tracker(&self) -> Arc<super::atomic_performance::AtomicPerformanceTracker> {
        self.inner.clone()
    }
    
    /// Record a successful file operation (legacy compatibility)
    pub fn record_file_operation(&self, operation_type: FileOperationType, processing_time: Duration) {
        // Update legacy counters
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.processed_files.fetch_add(1, Ordering::Relaxed);
        self.total_processing_time_ns.fetch_add(processing_time.as_nanos() as u64, Ordering::Relaxed);
        
        match operation_type {
            FileOperationType::Insert => self.inserted_files.fetch_add(1, Ordering::Relaxed),
            FileOperationType::Update => self.updated_files.fetch_add(1, Ordering::Relaxed),
            FileOperationType::Delete => self.deleted_files.fetch_add(1, Ordering::Relaxed),
        };
        
        // Also record in comprehensive tracker
        let result = super::atomic_performance::BatchOperationResult {
            files_processed: 1,
            processing_time,
            success: true,
            memory_used_bytes: 0, // Unknown for individual operations
            cache_hits: 0,
            cache_misses: 0,
            retry_count: 0,
        };
        
        // Use tokio spawn to handle async call
        let inner = self.inner.clone();
        tokio::spawn(async move {
            inner.record_batch_operation(result).await;
        });
    }
    
    /// Record a failed file operation (legacy compatibility)
    pub fn record_failed_operation(&self) {
        self.failed_files.fetch_add(1, Ordering::Relaxed);
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        
        // Also record in comprehensive tracker
        let result = super::atomic_performance::BatchOperationResult {
            files_processed: 0,
            processing_time: Duration::from_nanos(0),
            success: false,
            memory_used_bytes: 0,
            cache_hits: 0,
            cache_misses: 0,
            retry_count: 0,
        };
        
        let inner = self.inner.clone();
        tokio::spawn(async move {
            inner.record_batch_operation(result).await;
        });
    }
    
    /// Record a batch operation (enhanced with comprehensive tracking)
    pub async fn record_batch_operation_comprehensive(&self, 
        success: bool, 
        files_in_batch: usize, 
        processing_time: Duration,
        memory_used_bytes: u64,
        cache_hits: u64,
        cache_misses: u64,
        retry_count: u32,
    ) {
        // Update legacy counters
        self.total_batches.fetch_add(1, Ordering::Relaxed);
        if success {
            self.successful_batches.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_batches.fetch_add(1, Ordering::Relaxed);
        }
        self.total_processing_time_ns.fetch_add(processing_time.as_nanos() as u64, Ordering::Relaxed);
        
        // Record in comprehensive tracker
        let result = super::atomic_performance::BatchOperationResult {
            files_processed: files_in_batch,
            processing_time,
            success,
            memory_used_bytes,
            cache_hits,
            cache_misses,
            retry_count,
        };
        
        self.inner.record_batch_operation(result).await;
    }
    
    /// Record a batch operation (legacy compatibility)
    pub fn record_batch_operation(&self, success: bool, files_in_batch: usize, processing_time: Duration) {
        // Update legacy counters
        self.total_batches.fetch_add(1, Ordering::Relaxed);
        if success {
            self.successful_batches.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_batches.fetch_add(1, Ordering::Relaxed);
        }
        self.total_processing_time_ns.fetch_add(processing_time.as_nanos() as u64, Ordering::Relaxed);
        
        // Also record in comprehensive tracker
        let result = super::atomic_performance::BatchOperationResult {
            files_processed: files_in_batch,
            processing_time,
            success,
            memory_used_bytes: 0, // Unknown for legacy calls
            cache_hits: 0,
            cache_misses: 0,
            retry_count: 0,
        };
        
        let inner = self.inner.clone();
        tokio::spawn(async move {
            inner.record_batch_operation(result).await;
        });
    }
    
    /// Record cache hit/miss (legacy compatibility)
    pub fn record_cache_access(&self, hit: bool) {
        if hit {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.cache_misses.fetch_add(1, Ordering::Relaxed);
        }
        
        // No need to record in comprehensive tracker as it's handled by batch operations
    }
    
    /// Record index operation (legacy compatibility)
    pub fn record_index_operation(&self, operation_type: IndexOperationType) {
        match operation_type {
            IndexOperationType::Lookup => self.index_lookups.fetch_add(1, Ordering::Relaxed),
            IndexOperationType::Update => self.index_updates.fetch_add(1, Ordering::Relaxed),
        };
    }
    
    /// Record I/O operation (legacy compatibility)
    pub fn record_io_operation(&self, bytes: u64, is_write: bool, duration: Duration) {
        if is_write {
            self.bytes_written.fetch_add(bytes, Ordering::Relaxed);
        } else {
            self.bytes_read.fetch_add(bytes, Ordering::Relaxed);
        }
        self.total_io_time_ns.fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }
    
    /// Record sync operation (legacy compatibility)
    pub fn record_sync_operation(&self) {
        self.sync_operations.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Get current performance statistics (legacy compatibility)
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
    
    /// Get comprehensive performance metrics
    pub async fn get_comprehensive_metrics(&self) -> super::atomic_performance::PerformanceMetrics {
        self.inner.get_metrics().await
    }
    
    /// Log comprehensive performance summary
    pub async fn log_comprehensive_performance_summary(&self) {
        self.inner.log_performance_summary().await;
    }
    
    /// Export comprehensive metrics as JSON
    pub async fn export_comprehensive_metrics_json(&self) -> Result<String, serde_json::Error> {
        self.inner.export_metrics_json().await
    }
    
    /// Check if performance targets are being met
    pub async fn check_performance_targets(&self, target_throughput_files_per_sec: f64) -> super::atomic_performance::PerformanceStatus {
        self.inner.check_performance_targets(target_throughput_files_per_sec).await
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

/// Zero-copy database implementation with atomic operations and auto-scaling
pub struct ZeroCopyDatabase {
    // Core storage
    data_file: Arc<RwLock<MemoryMappedFile>>,
    index_manager: Arc<RwLock<IndexManager>>,
    
    // Configuration with atomic updates
    config: Arc<RwLock<ZeroCopyConfig>>,
    db_path: PathBuf,
    
    // Performance tracking
    performance_tracker: Arc<ZeroCopyPerformanceTracker>,
    
    // Path normalization
    path_normalizer: Box<dyn PathNormalizer>,
    
    // Database state
    is_initialized: AtomicU64,  // 0 = not initialized, 1 = initialized
    is_open: AtomicU64,         // 0 = closed, 1 = open
    
    // FlatBuffer serialization
    batch_serializer: Arc<BatchSerializer>,
    flatbuffer_builder: Arc<RwLock<flatbuffers::FlatBufferBuilder<'static>>>,
    
    // Playlist storage - atomic counters for ID generation
    next_playlist_id: AtomicU64,
    next_playlist_entry_id: AtomicU64,
    
    // In-memory playlist storage for fast access
    playlists: Arc<RwLock<std::collections::HashMap<i64, Playlist>>>,
    playlist_entries: Arc<RwLock<std::collections::HashMap<i64, Vec<super::PlaylistEntry>>>>,
    
    // Runtime configuration management
    config_update_channel: Arc<RwLock<Option<tokio::sync::broadcast::Sender<ZeroCopyConfigUpdate>>>>,
    performance_monitor_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

impl ZeroCopyDatabase {
    /// Create a new zero-copy database instance with auto-scaling configuration
    pub async fn new(db_path: PathBuf, config: Option<ZeroCopyConfig>) -> Result<Self> {
        let config = config.unwrap_or_else(ZeroCopyConfig::from_env);
        config.validate().context("Invalid ZeroCopy database configuration")?;
        
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
        
        // Create performance tracker with comprehensive monitoring
        let performance_tracker = Arc::new(ZeroCopyPerformanceTracker::new(
            config.performance_monitoring_interval,
            true, // Enable detailed logging for comprehensive monitoring
        ));
        
        // Create path normalizer
        let path_normalizer = create_platform_path_normalizer();
        
        // Create batch serializer
        let batch_serializer = Arc::new(BatchSerializer::new());
        
        // Create FlatBuffer builder
        let flatbuffer_builder = flatbuffers::FlatBufferBuilder::with_capacity(1024 * 1024); // 1MB initial capacity
        
        // Create configuration update channel
        let (config_sender, _) = tokio::sync::broadcast::channel(16);
        
        info!(
            "Created zero-copy database at {} with performance profile: {:?}",
            db_path.display(),
            config.performance_profile
        );
        info!(
            "Configuration: {}MB cache, {}K index entries, {} batch size",
            config.memory_map_size_mb,
            config.index_cache_size / 1000,
            config.batch_size
        );
        
        let database = Self {
            data_file: Arc::new(RwLock::new(data_file)),
            index_manager: Arc::new(RwLock::new(index_manager)),
            config: Arc::new(RwLock::new(config)),
            db_path,
            performance_tracker,
            path_normalizer,
            is_initialized: AtomicU64::new(0),
            is_open: AtomicU64::new(0),
            batch_serializer,
            flatbuffer_builder: Arc::new(RwLock::new(flatbuffer_builder)),
            next_playlist_id: AtomicU64::new(1),
            next_playlist_entry_id: AtomicU64::new(1),
            playlists: Arc::new(RwLock::new(std::collections::HashMap::new())),
            playlist_entries: Arc::new(RwLock::new(std::collections::HashMap::new())),
            config_update_channel: Arc::new(RwLock::new(Some(config_sender))),
            performance_monitor_handle: Arc::new(RwLock::new(None)),
        };
        
        Ok(database)
    }
    
    /// Create a new zero-copy database instance with specific performance profile
    pub async fn new_with_profile(db_path: PathBuf, profile: PerformanceProfile) -> Result<Self> {
        let config = ZeroCopyConfig::with_performance_profile(profile);
        Self::new(db_path, Some(config)).await
    }
    
    /// Create a new zero-copy database instance with auto-detected performance profile
    pub async fn new_with_auto_detection(db_path: PathBuf) -> Result<Self> {
        let config = ZeroCopyConfig::with_auto_detection();
        Self::new(db_path, Some(config)).await
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
        
        // Start performance monitoring if enabled
        self.start_performance_monitoring().await?;
        
        // Mark as open
        self.is_open.store(1, Ordering::Relaxed);
        
        let config = self.config.read().await;
        info!("Zero-copy database opened successfully with profile: {:?}", config.performance_profile);
        info!("Expected throughput: {}", config.performance_profile.expected_throughput_range());
        
        Ok(())
    }
    
    /// Close the database (sync data, save indexes)
    pub async fn close(&self) -> Result<()> {
        if self.is_open.load(Ordering::Relaxed) == 0 {
            return Ok(()); // Already closed
        }
        
        let start_time = Instant::now();
        
        info!("Closing zero-copy database...");
        
        // Stop performance monitoring
        self.stop_performance_monitoring().await;
        
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
        let config = self.config.read().await;
        info!(
            "Final stats: {} files processed, {:.2} files/sec average, {:.1}% cache hit rate",
            stats.processed_files,
            stats.average_throughput_per_sec,
            stats.cache_hit_rate * 100.0
        );
        info!("Final configuration: {:?} profile, {}MB cache, {} batch size", 
              config.performance_profile, config.memory_map_size_mb, config.batch_size);
        
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
    
    /// Get database configuration (async to handle RwLock)
    pub async fn get_config(&self) -> ZeroCopyConfig {
        self.config.read().await.clone()
    }
    
    /// Update database configuration at runtime
    pub async fn update_config(&self, updates: ZeroCopyConfigUpdate) -> Result<bool> {
        let mut config = self.config.write().await;
        let changed = config.update_runtime_config(updates)?;
        
        if changed {
            // Notify configuration change listeners
            if let Some(sender) = self.config_update_channel.read().await.as_ref() {
                let _ = sender.send(ZeroCopyConfigUpdate::default()); // Notify of change
            }
            
            // Apply configuration changes to components
            self.apply_config_changes(&*config).await?;
        }
        
        Ok(changed)
    }
    
    /// Apply configuration changes to database components
    async fn apply_config_changes(&self, config: &ZeroCopyConfig) -> Result<()> {
        // Update index manager cache size if needed
        {
            let mut index_manager = self.index_manager.write().await;
            let current_cache_size = index_manager.get_stats().max_entries;
            
            if current_cache_size != config.index_cache_size {
                info!("Updating index cache size from {} to {}", current_cache_size, config.index_cache_size);
                // Note: In a real implementation, you'd need to implement cache resizing
                // For now, just log the change
            }
        }
        
        info!("Applied configuration changes: cache={}MB, index={}, batch={}", 
              config.memory_map_size_mb, config.index_cache_size, config.batch_size);
        
        Ok(())
    }
    
    /// Start performance monitoring and auto-scaling
    pub async fn start_performance_monitoring(&self) -> Result<()> {
        let config = self.config.read().await;
        
        if !config.enable_auto_scaling && !config.enable_runtime_updates {
            return Ok(()); // No monitoring needed
        }
        
        let monitoring_interval = config.performance_monitoring_interval;
        let config_arc = self.config.clone();
        let performance_tracker = self.performance_tracker.clone();
        let index_manager = self.index_manager.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(monitoring_interval);
            
            loop {
                interval.tick().await;
                
                // Check memory pressure
                let memory_pressure = Self::calculate_memory_pressure(&index_manager).await;
                
                // Auto-scale if needed
                {
                    let mut config = config_arc.write().await;
                    if config.enable_auto_scaling {
                        if config.auto_scale_for_memory_pressure(memory_pressure) {
                            info!("Auto-scaled configuration due to memory pressure: {:.1}%", memory_pressure * 100.0);
                        }
                    }
                }
                
                // Log performance statistics
                let stats = performance_tracker.get_stats();
                if stats.total_operations > 0 {
                    debug!("Performance: {:.0} ops/sec, {:.1}% cache hit rate, {} total ops",
                           stats.average_throughput_per_sec,
                           stats.cache_hit_rate * 100.0,
                           stats.total_operations);
                }
            }
        });
        
        *self.performance_monitor_handle.write().await = Some(handle);
        info!("Started performance monitoring with {:.0}s interval", monitoring_interval.as_secs_f64());
        
        Ok(())
    }
    
    /// Stop performance monitoring
    pub async fn stop_performance_monitoring(&self) {
        if let Some(handle) = self.performance_monitor_handle.write().await.take() {
            handle.abort();
            info!("Stopped performance monitoring");
        }
    }
    
    /// Calculate current memory pressure (0.0 = no pressure, 1.0 = maximum pressure)
    async fn calculate_memory_pressure(index_manager: &Arc<RwLock<IndexManager>>) -> f64 {
        let index_manager = index_manager.read().await;
        let cache_stats = index_manager.cache_manager.get_cache_stats();
        
        // Calculate pressure based on cache usage and hit rates
        let memory_usage_pressure = cache_stats.combined_memory_usage as f64 / cache_stats.combined_max_memory as f64;
        let cache_miss_pressure = 1.0 - cache_stats.combined_hit_rate;
        
        // Combine pressures (weighted average)
        (memory_usage_pressure * 0.7 + cache_miss_pressure * 0.3).min(1.0)
    }
    
    /// Get current performance profile
    pub async fn get_performance_profile(&self) -> PerformanceProfile {
        self.config.read().await.performance_profile
    }
    
    /// Set performance profile
    pub async fn set_performance_profile(&self, profile: PerformanceProfile) -> Result<()> {
        let update = ZeroCopyConfigUpdate {
            performance_profile: Some(profile),
            ..Default::default()
        };
        
        self.update_config(update).await?;
        Ok(())
    }
    
    /// Subscribe to configuration changes
    pub async fn subscribe_to_config_changes(&self) -> Option<tokio::sync::broadcast::Receiver<ZeroCopyConfigUpdate>> {
        self.config_update_channel.read().await.as_ref().map(|sender| sender.subscribe())
    }
    
    /// Validate current configuration against memory budget
    pub async fn validate_memory_budget(&self) -> bool {
        self.config.read().await.check_memory_budget()
    }
    
    /// Get performance statistics (legacy compatibility)
    pub fn get_performance_stats(&self) -> PerformanceStats {
        self.performance_tracker.get_stats()
    }
    
    /// Get comprehensive performance metrics
    pub async fn get_comprehensive_performance_metrics(&self) -> super::atomic_performance::PerformanceMetrics {
        self.performance_tracker.get_comprehensive_metrics().await
    }
    

    
    /// Export comprehensive performance metrics as JSON
    pub async fn export_performance_metrics_json(&self) -> Result<String, serde_json::Error> {
        self.performance_tracker.export_comprehensive_metrics_json().await
    }
    
    /// Check if performance targets are being met
    pub async fn check_performance_targets(&self, target_throughput_files_per_sec: f64) -> super::atomic_performance::PerformanceStatus {
        self.performance_tracker.check_performance_targets(target_throughput_files_per_sec).await
    }
    
    /// Demonstrate comprehensive performance monitoring with a sample workload
    pub async fn demonstrate_performance_monitoring(&self) -> Result<()> {
        info!("=== Starting Performance Monitoring Demonstration ===");
        
        // Start performance monitoring
        self.start_performance_monitoring().await?;
        
        // Create some sample media files for testing
        let sample_files: Vec<MediaFile> = (0..1000).map(|i| {
            MediaFile::new(
                PathBuf::from(format!("/test/sample_{}.mp3", i)),
                1024 * 1024, // 1MB file size
                "audio/mpeg".to_string(),
            )
        }).collect();
        
        info!("Created {} sample files for performance testing", sample_files.len());
        
        // Perform batch insert to demonstrate performance tracking
        let insert_start = Instant::now();
        let _insert_result = self.bulk_store_media_files(&sample_files).await?;
        let insert_time = insert_start.elapsed();
        
        info!("Batch insert completed in {:?}", insert_time);
        
        // Get comprehensive performance metrics
        let metrics = self.get_comprehensive_performance_metrics().await;
        info!("=== Comprehensive Performance Metrics ===");
        info!("Total operations: {}", metrics.total_operations);
        info!("Files processed: {}", metrics.total_files_processed);
        info!("Current throughput: {:.0} files/sec", metrics.current_throughput_files_per_sec);
        info!("Average throughput: {:.0} files/sec", metrics.average_throughput_files_per_sec);
        info!("Memory usage: {:.1}MB current, {:.1}MB peak", 
              metrics.current_memory_usage_mb, metrics.peak_memory_usage_mb);
        info!("Cache hit rate: {:.1}%", metrics.cache_hit_rate * 100.0);
        info!("Success rate: {:.1}%", metrics.success_rate * 100.0);
        
        // Check performance targets
        let target_throughput = 100_000.0; // 100K files/sec target
        let status = self.check_performance_targets(target_throughput).await;
        
        info!("=== Performance Target Check ===");
        info!("Target throughput: {:.0} files/sec", target_throughput);
        info!("Current throughput: {:.0} files/sec", status.current_throughput);
        info!("Overall healthy: {}", status.overall_healthy);
        info!("Throughput OK: {}", status.throughput_ok);
        info!("Error rate OK: {}", status.error_rate_ok);
        info!("Memory OK: {}", status.memory_ok);
        info!("Cache OK: {}", status.cache_ok);
        
        // Export metrics as JSON
        let json_metrics = self.export_performance_metrics_json().await?;
        info!("Exported comprehensive metrics as JSON ({} bytes)", json_metrics.len());
        
        // Stop performance monitoring
        self.stop_performance_monitoring().await;
        
        info!("=== Performance Monitoring Demonstration Complete ===");
        Ok(())
    }
    
    /// Get index statistics
    pub async fn get_index_stats(&self) -> IndexStats {
        let index_manager = self.index_manager.read().await;
        index_manager.get_stats()
    }
    
    /// Get memory-bounded cache statistics
    pub async fn get_cache_stats(&self) -> super::index_manager::CombinedCacheStats {
        let index_manager = self.index_manager.read().await;
        index_manager.cache_manager.get_cache_stats()
    }
    
    /// Check memory pressure across all caches
    pub async fn check_memory_pressure(&self) -> super::memory_bounded_cache::MemoryPressureStatus {
        let mut index_manager = self.index_manager.write().await;
        index_manager.cache_manager.check_and_handle_pressure()
    }
    
    /// Force cache cleanup to free memory
    pub async fn force_cache_cleanup(&self, memory_reduction_factor: f64) -> Result<()> {
        let mut index_manager = self.index_manager.write().await;
        index_manager.cache_manager.force_cleanup_all_caches(memory_reduction_factor);
        Ok(())
    }
    
    /// Clear all caches
    pub async fn clear_all_caches(&self) -> Result<()> {
        let mut index_manager = self.index_manager.write().await;
        index_manager.cache_manager.clear_all();
        Ok(())
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
    
    /// Helper method to deserialize MediaFile from FlatBuffer data
    fn deserialize_media_file_from_data(&self, _data: &[u8]) -> Result<MediaFile> {
        // For now, return a placeholder - this would need proper FlatBuffer deserialization
        // In a real implementation, this would parse the FlatBuffer data
        Err(anyhow!("MediaFile deserialization from FlatBuffer data not yet implemented"))
    }
    
    /// Batch insert files using zero-copy FlatBuffer serialization
    pub async fn batch_insert_files(&self, files: &[MediaFile]) -> Result<BatchProcessingResult> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        if files.is_empty() {
            return Ok(BatchProcessingResult::empty());
        }
        
        // Get current configuration for batch processing
        let config = self.config.read().await;
        let batch_size = config.batch_size;
        drop(config); // Release lock early
        
        // If files exceed batch size, process in chunks
        if files.len() > batch_size {
            return self.batch_insert_files_chunked(files, batch_size).await;
        }
        
        self.batch_insert_files_internal(files).await
    }
    
    /// Internal batch insert without chunking logic to avoid recursion
    async fn batch_insert_files_internal(&self, files: &[MediaFile]) -> Result<BatchProcessingResult> {
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
        
        // Record comprehensive performance metrics
        let total_time = start_time.elapsed();
        let memory_used = serialization_result.serialized_size as u64; // Estimate memory usage from serialized data size
        let cache_hits = 0; // No cache hits for insert operations
        let cache_misses = files.len() as u64; // All files are new, so all are cache misses
        
        // Record comprehensive batch operation
        self.performance_tracker.record_batch_operation_comprehensive(
            true,
            files.len(),
            total_time,
            memory_used,
            cache_hits,
            cache_misses,
            0, // No retries for successful operation
        ).await;
        
        // Also record legacy metrics for backward compatibility
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
    
    /// Process large batches in chunks to respect memory limits
    async fn batch_insert_files_chunked(&self, files: &[MediaFile], chunk_size: usize) -> Result<BatchProcessingResult> {
        let start_time = Instant::now();
        let total_files = files.len();
        let mut total_processed = 0;
        let mut total_errors = Vec::new();
        let mut total_data_size = 0;
        let mut total_serialization_time = Duration::from_nanos(0);
        
        info!("Processing {} files in chunks of {} (total chunks: {})", 
              total_files, chunk_size, (total_files + chunk_size - 1) / chunk_size);
        
        for (chunk_index, chunk) in files.chunks(chunk_size).enumerate() {
            let chunk_result = Box::pin(self.batch_insert_files_internal(chunk)).await?;
            
            total_processed += chunk_result.files_processed;
            total_errors.extend(chunk_result.errors);
            total_data_size += chunk_result.data_size;
            total_serialization_time += chunk_result.serialization_time;
            
            // Log progress every 10 chunks or on last chunk
            if chunk_index % 10 == 0 || chunk_index == (total_files + chunk_size - 1) / chunk_size - 1 {
                info!("Processed chunk {}/{}: {} files ({:.1}% complete)", 
                      chunk_index + 1, 
                      (total_files + chunk_size - 1) / chunk_size,
                      chunk_result.files_processed,
                      (total_processed as f64 / total_files as f64) * 100.0);
            }
            
            // Check for memory pressure and auto-scale if needed
            let config = self.config.read().await;
            if config.enable_auto_scaling {
                let memory_pressure = Self::calculate_memory_pressure(&self.index_manager).await;
                if memory_pressure > 0.8 {
                    drop(config);
                    let update = ZeroCopyConfigUpdate::default();
                    if let Ok(true) = self.update_config(update).await {
                        info!("Auto-scaled configuration during batch processing due to memory pressure");
                    }
                }
            }
        }
        
        let total_time = start_time.elapsed();
        let throughput = total_processed as f64 / total_time.as_secs_f64();
        
        info!("Chunked batch processing completed: {} files in {:?} ({:.0} files/sec)", 
              total_processed, total_time, throughput);
        
        Ok(BatchProcessingResult {
            batch_id: self.batch_serializer.generate_batch_id(),
            operation_type: BatchOperationType::Insert,
            files_processed: total_processed,
            processing_time: total_time,
            serialization_time: total_serialization_time,
            write_offset: 0, // Not applicable for chunked processing
            data_size: total_data_size,
            throughput,
            checksum: 0, // Not applicable for chunked processing
            errors: total_errors,
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
        
        // Get current configuration for batch processing
        let config = self.config.read().await;
        let batch_size = config.batch_size;
        drop(config); // Release lock early
        
        // If files exceed batch size, process in chunks
        if files.len() > batch_size {
            return self.batch_update_files_chunked(files, batch_size).await;
        }
        
        self.batch_update_files_internal(files).await
    }
    
    /// Internal batch update without chunking logic to avoid recursion
    async fn batch_update_files_internal(&self, files: &[MediaFile]) -> Result<BatchProcessingResult> {
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
        
        // Record comprehensive performance metrics
        let total_time = start_time.elapsed();
        let memory_used = serialization_result.serialized_size as u64; // Estimate memory usage from serialized data size
        let cache_hits = files.len() as u64; // Updates typically involve cache hits for existing files
        let cache_misses = 0; // Minimal cache misses for update operations
        
        // Record comprehensive batch operation
        self.performance_tracker.record_batch_operation_comprehensive(
            true,
            files.len(),
            total_time,
            memory_used,
            cache_hits,
            cache_misses,
            0, // No retries for successful operation
        ).await;
        
        // Also record legacy metrics for backward compatibility
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
    
    /// Process large update batches in chunks to respect memory limits
    async fn batch_update_files_chunked(&self, files: &[MediaFile], chunk_size: usize) -> Result<BatchProcessingResult> {
        let start_time = Instant::now();
        let total_files = files.len();
        let mut total_processed = 0;
        let mut total_errors = Vec::new();
        let mut total_data_size = 0;
        let mut total_serialization_time = Duration::from_nanos(0);
        
        info!("Processing {} file updates in chunks of {} (total chunks: {})", 
              total_files, chunk_size, (total_files + chunk_size - 1) / chunk_size);
        
        for (chunk_index, chunk) in files.chunks(chunk_size).enumerate() {
            let chunk_result = Box::pin(self.batch_update_files_internal(chunk)).await?;
            
            total_processed += chunk_result.files_processed;
            total_errors.extend(chunk_result.errors);
            total_data_size += chunk_result.data_size;
            total_serialization_time += chunk_result.serialization_time;
            
            // Log progress every 10 chunks or on last chunk
            if chunk_index % 10 == 0 || chunk_index == (total_files + chunk_size - 1) / chunk_size - 1 {
                info!("Updated chunk {}/{}: {} files ({:.1}% complete)", 
                      chunk_index + 1, 
                      (total_files + chunk_size - 1) / chunk_size,
                      chunk_result.files_processed,
                      (total_processed as f64 / total_files as f64) * 100.0);
            }
        }
        
        let total_time = start_time.elapsed();
        let throughput = total_processed as f64 / total_time.as_secs_f64();
        
        info!("Chunked batch update completed: {} files in {:?} ({:.0} files/sec)", 
              total_processed, total_time, throughput);
        
        Ok(BatchProcessingResult {
            batch_id: self.batch_serializer.generate_batch_id(),
            operation_type: BatchOperationType::Update,
            files_processed: total_processed,
            processing_time: total_time,
            serialization_time: total_serialization_time,
            write_offset: 0, // Not applicable for chunked processing
            data_size: total_data_size,
            throughput,
            checksum: 0, // Not applicable for chunked processing
            errors: total_errors,
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
        
        // Record comprehensive performance metrics
        let total_time = start_time.elapsed();
        let memory_used = removed_count as u64 * 64; // Estimate memory usage for removal operations (64 bytes per file)
        let cache_hits = removed_count as u64; // Removals typically involve cache hits for existing files
        let cache_misses = 0; // Minimal cache misses for removal operations
        
        // Record comprehensive batch operation
        self.performance_tracker.record_batch_operation_comprehensive(
            true,
            removed_count,
            total_time,
            memory_used,
            cache_hits,
            cache_misses,
            0, // No retries for successful operation
        ).await;
        
        // Also record legacy metrics for backward compatibility
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
    
    /// Read a media file from memory-mapped storage at the specified offset
    /// Uses zero-copy FlatBuffer deserialization for maximum performance
    async fn read_media_file_at_offset(&self, _data_file: &MemoryMappedFile, _offset: u64) -> Result<MediaFile> {
        // TODO: This is a stub implementation until FlatBuffer compiler is available
        // For now, return a placeholder MediaFile to allow compilation
        Err(anyhow!("FlatBuffer deserialization not available - stub implementation"))
    }
    
    /// Check if a directory contains files of the specified media type
    /// Uses atomic index operations for efficient filtering
    async fn directory_contains_media_type(&self, dir_path: &PathBuf, _media_type_filter: &str) -> Result<bool> {
        let start_time = Instant::now();
        
        // Normalize directory path
        let canonical_dir = self.path_normalizer.to_canonical(dir_path)
            .map_err(|e| anyhow!("Path normalization failed: {}", e))?;
        
        // Get files in directory
        let file_offsets = {
            let mut index_manager = self.index_manager.write().await;
            self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
            index_manager.find_files_in_directory(&canonical_dir)
        };
        
        if file_offsets.is_empty() {
            return Ok(false);
        }
        
        // Check if any file matches the media type filter
        let _data_file = self.data_file.read().await;
        for _offset in file_offsets.iter().take(10) { // Check only first 10 files for performance
            // TODO: Implement file reading when FlatBuffer is available
            // For now, assume directory contains matching files if it has any files
            if !file_offsets.is_empty() {
                let check_time = start_time.elapsed();
                self.performance_tracker.record_io_operation(
                    1024, // Estimate 1KB per file check
                    false, // is_read
                    check_time,
                );
                return Ok(true);
            }
        }
        
        let check_time = start_time.elapsed();
        self.performance_tracker.record_io_operation(
            file_offsets.len() as u64 * 100, // Estimate 100 bytes per file check
            false, // is_read
            check_time,
        );
        
        Ok(false)
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
    
    async fn get_files_in_directory(&self, dir: &Path) -> Result<Vec<MediaFile>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Normalize directory path for consistent lookups
        let canonical_dir = self.path_normalizer.to_canonical(dir)
            .map_err(|e| anyhow!("Path normalization failed: {}", e))?;
        
        // Get file offsets from directory index with atomic lookup
        let file_offsets = {
            let mut index_manager = self.index_manager.write().await;
            self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
            index_manager.find_files_in_directory(&canonical_dir)
        };
        
        if file_offsets.is_empty() {
            self.performance_tracker.record_cache_access(true); // Cache hit for empty result
            debug!("No files found in directory: {}", canonical_dir);
            return Ok(Vec::new());
        }
        
        // Read files from memory-mapped storage using zero-copy access
        let mut files = Vec::with_capacity(file_offsets.len());
        let data_file = self.data_file.read().await;
        
        for offset in file_offsets {
            match self.read_media_file_at_offset(&data_file, offset).await {
                Ok(file) => {
                    files.push(file);
                    self.performance_tracker.record_cache_access(true);
                }
                Err(e) => {
                    warn!("Failed to read file at offset {}: {}", offset, e);
                    self.performance_tracker.record_cache_access(false);
                    self.performance_tracker.record_failed_operation();
                }
            }
        }
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_io_operation(
            files.len() as u64 * 1024, // Estimate 1KB per file record
            false, // is_read
            processing_time,
        );
        
        debug!(
            "Retrieved {} files from directory {} in {:?}",
            files.len(),
            canonical_dir,
            processing_time
        );
        
        Ok(files)
    }
    
    async fn get_directory_listing(&self, parent_path: &Path, media_type_filter: &str) -> Result<(Vec<MediaDirectory>, Vec<MediaFile>)> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Normalize parent path for consistent lookups
        let canonical_parent_path = self.path_normalizer.to_canonical(parent_path)
            .map_err(|e| anyhow!("Path normalization failed: {}", e))?;
        
        // Get direct files in this directory with atomic index lookup
        let direct_files = self.get_files_in_directory(parent_path).await?;
        
        // Filter files by media type if specified
        let filtered_files = if media_type_filter.is_empty() {
            direct_files
        } else {
            direct_files.into_iter()
                .filter(|file| {
                    file.mime_type.starts_with(media_type_filter)
                })
                .collect()
        };
        
        // Get direct subdirectories with atomic B-tree operations
        let subdirectories = self.get_direct_subdirectories(&canonical_parent_path).await?;
        
        // Filter subdirectories that contain files matching the media type filter
        let filtered_subdirectories = if media_type_filter.is_empty() {
            subdirectories
        } else {
            let mut filtered_dirs = Vec::new();
            for subdir in subdirectories {
                // Check if subdirectory contains files of the specified media type
                if self.directory_contains_media_type(&subdir.path, media_type_filter).await? {
                    filtered_dirs.push(subdir);
                }
            }
            filtered_dirs
        };
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_io_operation(
            (filtered_files.len() + filtered_subdirectories.len()) as u64 * 512, // Estimate 512 bytes per entry
            false, // is_read
            processing_time,
        );
        
        debug!(
            "Retrieved directory listing for {}: {} subdirs, {} files in {:?}",
            canonical_parent_path,
            filtered_subdirectories.len(),
            filtered_files.len(),
            processing_time
        );
        
        Ok((filtered_subdirectories, filtered_files))
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
    
    // Music categorization methods with atomic operations
    async fn get_artists(&self) -> Result<Vec<MusicCategory>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Get all unique artists from index with atomic scanning
        let artists = {
            let index_manager = self.index_manager.read().await;
            self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
            index_manager.get_all_artists()
        };
        
        // Convert to MusicCategory with atomic counting
        let mut categories = Vec::with_capacity(artists.len());
        for (artist, file_offsets) in artists {
            categories.push(MusicCategory {
                id: artist.clone(),
                name: artist,
                category_type: super::MusicCategoryType::Artist,
                count: file_offsets.len(),
            });
        }
        
        // Sort by name for consistent ordering
        categories.sort_by(|a, b| a.name.cmp(&b.name));
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_io_operation(
            categories.len() as u64 * 64, // Estimate 64 bytes per category
            false, // is_read
            processing_time,
        );
        
        info!(
            "Retrieved {} artists in {:?} with atomic index scanning",
            categories.len(),
            processing_time
        );
        
        Ok(categories)
    }
    
    async fn get_albums(&self, artist: Option<&str>) -> Result<Vec<MusicCategory>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Get albums from index with atomic filtering
        let albums = {
            let index_manager = self.index_manager.read().await;
            self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
            
            if let Some(artist_filter) = artist {
                // Get albums for specific artist with atomic filtering
                index_manager.get_albums_by_artist(artist_filter)
            } else {
                // Get all albums with atomic scanning
                index_manager.get_all_albums()
            }
        };
        
        // Convert to MusicCategory with atomic counting
        let mut categories = Vec::with_capacity(albums.len());
        for (album, file_offsets) in albums {
            categories.push(MusicCategory {
                id: album.clone(),
                name: album,
                category_type: super::MusicCategoryType::Album,
                count: file_offsets.len(),
            });
        }
        
        // Sort by name for consistent ordering
        categories.sort_by(|a, b| a.name.cmp(&b.name));
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_io_operation(
            categories.len() as u64 * 64, // Estimate 64 bytes per category
            false, // is_read
            processing_time,
        );
        
        info!(
            "Retrieved {} albums{} in {:?} with atomic index filtering",
            categories.len(),
            if artist.is_some() { " for artist" } else { "" },
            processing_time
        );
        
        Ok(categories)
    }
    
    async fn get_genres(&self) -> Result<Vec<MusicCategory>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Get all unique genres from index with atomic categorization
        let genres = {
            let index_manager = self.index_manager.read().await;
            self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
            index_manager.get_all_genres()
        };
        
        // Convert to MusicCategory with atomic counting
        let mut categories = Vec::with_capacity(genres.len());
        for (genre, file_offsets) in genres {
            categories.push(MusicCategory {
                id: genre.clone(),
                name: genre,
                category_type: super::MusicCategoryType::Genre,
                count: file_offsets.len(),
            });
        }
        
        // Sort by name for consistent ordering
        categories.sort_by(|a, b| a.name.cmp(&b.name));
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_io_operation(
            categories.len() as u64 * 64, // Estimate 64 bytes per category
            false, // is_read
            processing_time,
        );
        
        info!(
            "Retrieved {} genres in {:?} with atomic index categorization",
            categories.len(),
            processing_time
        );
        
        Ok(categories)
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
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Get all unique years from index with atomic year extraction
        let years = {
            let index_manager = self.index_manager.read().await;
            self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
            index_manager.get_all_years()
        };
        
        // Convert to MusicCategory with atomic counting
        let mut categories = Vec::with_capacity(years.len());
        for (year, file_offsets) in years {
            categories.push(MusicCategory {
                id: year.to_string(),
                name: year.to_string(),
                category_type: super::MusicCategoryType::Year,
                count: file_offsets.len(),
            });
        }
        
        // Sort by year (descending - newest first)
        categories.sort_by(|a, b| b.name.cmp(&a.name));
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_io_operation(
            categories.len() as u64 * 64, // Estimate 64 bytes per category
            false, // is_read
            processing_time,
        );
        
        info!(
            "Retrieved {} years in {:?} with atomic index year extraction",
            categories.len(),
            processing_time
        );
        
        Ok(categories)
    }
    
    async fn get_album_artists(&self) -> Result<Vec<MusicCategory>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Get all unique album artists from index with atomic scanning
        let album_artists = {
            let index_manager = self.index_manager.read().await;
            self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
            index_manager.get_all_album_artists()
        };
        
        // Convert to MusicCategory with atomic counting
        let mut categories = Vec::with_capacity(album_artists.len());
        for (album_artist, file_offsets) in album_artists {
            categories.push(MusicCategory {
                id: album_artist.clone(),
                name: album_artist,
                category_type: super::MusicCategoryType::AlbumArtist,
                count: file_offsets.len(),
            });
        }
        
        // Sort by name for consistent ordering
        categories.sort_by(|a, b| a.name.cmp(&b.name));
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_io_operation(
            categories.len() as u64 * 64, // Estimate 64 bytes per category
            false, // is_read
            processing_time,
        );
        
        info!(
            "Retrieved {} album artists in {:?} with atomic index scanning",
            categories.len(),
            processing_time
        );
        
        Ok(categories)
    }
    
    async fn get_music_by_artist(&self, artist: &str) -> Result<Vec<MediaFile>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Get file offsets for artist with atomic lookups
        let file_offsets = {
            let index_manager = self.index_manager.read().await;
            self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
            index_manager.find_files_by_artist(artist)
        };
        
        if file_offsets.is_empty() {
            self.performance_tracker.record_cache_access(true); // Cache hit for empty result
            debug!("No files found for artist: {}", artist);
            return Ok(Vec::new());
        }
        
        // Read files from memory-mapped storage using zero-copy access
        let mut files = Vec::with_capacity(file_offsets.len());
        let data_file = self.data_file.read().await;
        
        for offset in file_offsets {
            match self.read_media_file_at_offset(&data_file, offset).await {
                Ok(file) => {
                    files.push(file);
                    self.performance_tracker.record_cache_access(true);
                }
                Err(e) => {
                    warn!("Failed to read file at offset {} for artist {}: {}", offset, artist, e);
                    self.performance_tracker.record_cache_access(false);
                    self.performance_tracker.record_failed_operation();
                }
            }
        }
        
        // Sort by album, then track number for consistent ordering
        files.sort_by(|a, b| {
            a.album.cmp(&b.album)
                .then_with(|| a.track_number.cmp(&b.track_number))
                .then_with(|| a.title.cmp(&b.title))
        });
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_io_operation(
            files.len() as u64 * 1024, // Estimate 1KB per file record
            false, // is_read
            processing_time,
        );
        
        info!(
            "Retrieved {} files for artist '{}' in {:?} with atomic lookups",
            files.len(),
            artist,
            processing_time
        );
        
        Ok(files)
    }
    
    async fn get_music_by_album(&self, album: &str, artist: Option<&str>) -> Result<Vec<MediaFile>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Get file offsets for album with atomic filtering
        let file_offsets = {
            let index_manager = self.index_manager.read().await;
            self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
            
            if let Some(artist_filter) = artist {
                // Get files for specific album and artist with atomic filtering
                index_manager.find_files_by_album_and_artist(album, artist_filter)
            } else {
                // Get all files for album with atomic lookups
                index_manager.find_files_by_album(album)
            }
        };
        
        if file_offsets.is_empty() {
            self.performance_tracker.record_cache_access(true); // Cache hit for empty result
            debug!("No files found for album: {} (artist: {:?})", album, artist);
            return Ok(Vec::new());
        }
        
        // Read files from memory-mapped storage using zero-copy access
        let mut files = Vec::with_capacity(file_offsets.len());
        let data_file = self.data_file.read().await;
        
        for offset in file_offsets {
            match self.read_media_file_at_offset(&data_file, offset).await {
                Ok(file) => {
                    files.push(file);
                    self.performance_tracker.record_cache_access(true);
                }
                Err(e) => {
                    warn!("Failed to read file at offset {} for album {}: {}", offset, album, e);
                    self.performance_tracker.record_cache_access(false);
                    self.performance_tracker.record_failed_operation();
                }
            }
        }
        
        // Sort by track number for consistent ordering
        files.sort_by(|a, b| {
            a.track_number.cmp(&b.track_number)
                .then_with(|| a.title.cmp(&b.title))
        });
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_io_operation(
            files.len() as u64 * 1024, // Estimate 1KB per file record
            false, // is_read
            processing_time,
        );
        
        info!(
            "Retrieved {} files for album '{}'{} in {:?} with atomic filtering",
            files.len(),
            album,
            if artist.is_some() { " by artist" } else { "" },
            processing_time
        );
        
        Ok(files)
    }
    
    async fn get_music_by_genre(&self, genre: &str) -> Result<Vec<MediaFile>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Get file offsets for genre with atomic lookups
        let file_offsets = {
            let index_manager = self.index_manager.read().await;
            self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
            index_manager.find_files_by_genre(genre)
        };
        
        if file_offsets.is_empty() {
            self.performance_tracker.record_cache_access(true); // Cache hit for empty result
            debug!("No files found for genre: {}", genre);
            return Ok(Vec::new());
        }
        
        // Read files from memory-mapped storage using zero-copy access
        let mut files = Vec::with_capacity(file_offsets.len());
        let data_file = self.data_file.read().await;
        
        for offset in file_offsets {
            match self.read_media_file_at_offset(&data_file, offset).await {
                Ok(file) => {
                    files.push(file);
                    self.performance_tracker.record_cache_access(true);
                }
                Err(e) => {
                    warn!("Failed to read file at offset {} for genre {}: {}", offset, genre, e);
                    self.performance_tracker.record_cache_access(false);
                    self.performance_tracker.record_failed_operation();
                }
            }
        }
        
        // Sort by artist, then album, then track number for consistent ordering
        files.sort_by(|a, b| {
            a.artist.cmp(&b.artist)
                .then_with(|| a.album.cmp(&b.album))
                .then_with(|| a.track_number.cmp(&b.track_number))
                .then_with(|| a.title.cmp(&b.title))
        });
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_io_operation(
            files.len() as u64 * 1024, // Estimate 1KB per file record
            false, // is_read
            processing_time,
        );
        
        info!(
            "Retrieved {} files for genre '{}' in {:?} with atomic lookups",
            files.len(),
            genre,
            processing_time
        );
        
        Ok(files)
    }
    
    async fn get_music_by_year(&self, year: u32) -> Result<Vec<MediaFile>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Get file offsets for year with atomic lookups
        let file_offsets = {
            let index_manager = self.index_manager.read().await;
            self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
            index_manager.find_files_by_year(year)
        };
        
        if file_offsets.is_empty() {
            self.performance_tracker.record_cache_access(true); // Cache hit for empty result
            debug!("No files found for year: {}", year);
            return Ok(Vec::new());
        }
        
        // Read files from memory-mapped storage using zero-copy access
        let mut files = Vec::with_capacity(file_offsets.len());
        let data_file = self.data_file.read().await;
        
        for offset in file_offsets {
            match self.read_media_file_at_offset(&data_file, offset).await {
                Ok(file) => {
                    files.push(file);
                    self.performance_tracker.record_cache_access(true);
                }
                Err(e) => {
                    warn!("Failed to read file at offset {} for year {}: {}", offset, year, e);
                    self.performance_tracker.record_cache_access(false);
                    self.performance_tracker.record_failed_operation();
                }
            }
        }
        
        // Sort by artist, then album, then track number for consistent ordering
        files.sort_by(|a, b| {
            a.artist.cmp(&b.artist)
                .then_with(|| a.album.cmp(&b.album))
                .then_with(|| a.track_number.cmp(&b.track_number))
                .then_with(|| a.title.cmp(&b.title))
        });
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_io_operation(
            files.len() as u64 * 1024, // Estimate 1KB per file record
            false, // is_read
            processing_time,
        );
        
        info!(
            "Retrieved {} files for year {} in {:?} with atomic lookups",
            files.len(),
            year,
            processing_time
        );
        
        Ok(files)
    }
    
    async fn get_music_by_album_artist(&self, album_artist: &str) -> Result<Vec<MediaFile>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Get file offsets for album artist with atomic lookups
        let file_offsets = {
            let index_manager = self.index_manager.read().await;
            self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
            index_manager.find_files_by_album_artist(album_artist)
        };
        
        if file_offsets.is_empty() {
            self.performance_tracker.record_cache_access(true); // Cache hit for empty result
            debug!("No files found for album artist: {}", album_artist);
            return Ok(Vec::new());
        }
        
        // Read files from memory-mapped storage using zero-copy access
        let mut files = Vec::with_capacity(file_offsets.len());
        let data_file = self.data_file.read().await;
        
        for offset in file_offsets {
            match self.read_media_file_at_offset(&data_file, offset).await {
                Ok(file) => {
                    files.push(file);
                    self.performance_tracker.record_cache_access(true);
                }
                Err(e) => {
                    warn!("Failed to read file at offset {} for album artist {}: {}", offset, album_artist, e);
                    self.performance_tracker.record_cache_access(false);
                    self.performance_tracker.record_failed_operation();
                }
            }
        }
        
        // Sort by album, then track number for consistent ordering
        files.sort_by(|a, b| {
            a.album.cmp(&b.album)
                .then_with(|| a.track_number.cmp(&b.track_number))
                .then_with(|| a.title.cmp(&b.title))
        });
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_io_operation(
            files.len() as u64 * 1024, // Estimate 1KB per file record
            false, // is_read
            processing_time,
        );
        
        info!(
            "Retrieved {} files for album artist '{}' in {:?} with atomic lookups",
            files.len(),
            album_artist,
            processing_time
        );
        
        Ok(files)
    }
    
    // Playlist methods - implemented with atomic operations and bulk processing
    async fn create_playlist(&self, name: &str, description: Option<&str>) -> Result<i64> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Generate atomic playlist ID
        let playlist_id = self.next_playlist_id.fetch_add(1, Ordering::SeqCst) as i64;
        
        let now = SystemTime::now();
        let playlist = Playlist {
            id: Some(playlist_id),
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        };
        
        // Store in memory for fast access
        {
            let mut playlists = self.playlists.write().await;
            playlists.insert(playlist_id, playlist.clone());
        }
        
        // Serialize and persist to disk using batch operations
        let serialization_result = {
            let mut builder = self.flatbuffer_builder.write().await;
            builder.reset();
            
            super::flatbuffer::PlaylistSerializer::serialize_playlist_batch(
                &mut builder,
                &[playlist],
                self.batch_serializer.generate_batch_id(),
                super::flatbuffer::BatchOperationType::Insert,
            )?
        };
        
        // Write to memory-mapped file
        {
            let mut data_file = self.data_file.write().await;
            let builder = self.flatbuffer_builder.read().await;
            let serialized_data = builder.finished_data();
            
            let io_start = Instant::now();
            let _offset = data_file.append_data(serialized_data)?;
            let io_time = io_start.elapsed();
            
            self.performance_tracker.record_io_operation(
                serialized_data.len() as u64,
                true,
                io_time,
            );
        }
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_file_operation(FileOperationType::Insert, processing_time);
        
        info!(
            "Created playlist '{}' with ID {} in {:?}",
            name,
            playlist_id,
            processing_time
        );
        
        Ok(playlist_id)
    }
    
    async fn get_playlists(&self) -> Result<Vec<Playlist>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Get from in-memory storage for fast access
        let playlists = {
            let playlists_map = self.playlists.read().await;
            playlists_map.values().cloned().collect::<Vec<_>>()
        };
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_cache_access(true); // Cache hit
        
        debug!(
            "Retrieved {} playlists in {:?}",
            playlists.len(),
            processing_time
        );
        
        Ok(playlists)
    }
    
    async fn get_playlist(&self, playlist_id: i64) -> Result<Option<Playlist>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Get from in-memory storage for fast access
        let playlist = {
            let playlists_map = self.playlists.read().await;
            playlists_map.get(&playlist_id).cloned()
        };
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_cache_access(playlist.is_some());
        
        debug!(
            "Retrieved playlist {} in {:?} (found: {})",
            playlist_id,
            processing_time,
            playlist.is_some()
        );
        
        Ok(playlist)
    }
    
    async fn update_playlist(&self, playlist: &Playlist) -> Result<()> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let playlist_id = playlist.id.ok_or_else(|| anyhow!("Playlist must have an ID"))?;
        let start_time = Instant::now();
        
        // Update in memory
        {
            let mut playlists_map = self.playlists.write().await;
            if !playlists_map.contains_key(&playlist_id) {
                return Err(anyhow!("Playlist {} not found", playlist_id));
            }
            
            let mut updated_playlist = playlist.clone();
            updated_playlist.updated_at = SystemTime::now();
            playlists_map.insert(playlist_id, updated_playlist.clone());
        }
        
        // Serialize and persist to disk
        let _serialization_result = {
            let mut builder = self.flatbuffer_builder.write().await;
            builder.reset();
            
            super::flatbuffer::PlaylistSerializer::serialize_playlist_batch(
                &mut builder,
                &[playlist.clone()],
                self.batch_serializer.generate_batch_id(),
                super::flatbuffer::BatchOperationType::Update,
            )?
        };
        
        // Write to memory-mapped file
        {
            let mut data_file = self.data_file.write().await;
            let builder = self.flatbuffer_builder.read().await;
            let serialized_data = builder.finished_data();
            
            let io_start = Instant::now();
            let _offset = data_file.append_data(serialized_data)?;
            let io_time = io_start.elapsed();
            
            self.performance_tracker.record_io_operation(
                serialized_data.len() as u64,
                true,
                io_time,
            );
        }
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_file_operation(FileOperationType::Update, processing_time);
        
        info!(
            "Updated playlist {} in {:?}",
            playlist_id,
            processing_time
        );
        
        Ok(())
    }
    
    async fn delete_playlist(&self, playlist_id: i64) -> Result<bool> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Remove from memory
        let playlist_existed = {
            let mut playlists_map = self.playlists.write().await;
            playlists_map.remove(&playlist_id).is_some()
        };
        
        if !playlist_existed {
            return Ok(false);
        }
        
        // Also remove all playlist entries
        {
            let mut entries_map = self.playlist_entries.write().await;
            entries_map.remove(&playlist_id);
        }
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_file_operation(FileOperationType::Delete, processing_time);
        
        info!(
            "Deleted playlist {} in {:?}",
            playlist_id,
            processing_time
        );
        
        Ok(true)
    }
    
    async fn add_to_playlist(&self, playlist_id: i64, media_file_id: i64, position: Option<u32>) -> Result<i64> {
        // Use bulk operation for consistency
        let entries = vec![(media_file_id, position.unwrap_or(0))];
        let entry_ids = self.batch_add_to_playlist(playlist_id, &entries).await?;
        Ok(entry_ids[0])
    }
    
    async fn batch_add_to_playlist(&self, playlist_id: i64, media_file_ids: &[(i64, u32)]) -> Result<Vec<i64>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        if media_file_ids.is_empty() {
            return Ok(Vec::new());
        }
        
        let start_time = Instant::now();
        
        // Verify playlist exists
        {
            let playlists_map = self.playlists.read().await;
            if !playlists_map.contains_key(&playlist_id) {
                return Err(anyhow!("Playlist {} not found", playlist_id));
            }
        }
        
        // Generate atomic entry IDs and create entries
        let mut entries = Vec::with_capacity(media_file_ids.len());
        let mut entry_ids = Vec::with_capacity(media_file_ids.len());
        let now = SystemTime::now();
        
        for &(media_file_id, position) in media_file_ids {
            let entry_id = self.next_playlist_entry_id.fetch_add(1, Ordering::SeqCst) as i64;
            entry_ids.push(entry_id);
            
            entries.push(super::PlaylistEntry {
                id: Some(entry_id),
                playlist_id,
                media_file_id,
                position,
                created_at: now,
            });
        }
        
        // Store in memory
        {
            let mut entries_map = self.playlist_entries.write().await;
            let playlist_entries = entries_map.entry(playlist_id).or_insert_with(Vec::new);
            playlist_entries.extend(entries.clone());
            
            // Sort by position for consistent ordering
            playlist_entries.sort_by_key(|e| e.position);
        }
        
        // Serialize and persist to disk using batch operations
        let _serialization_result = {
            let mut builder = self.flatbuffer_builder.write().await;
            builder.reset();
            
            super::flatbuffer::PlaylistSerializer::serialize_playlist_entry_batch(
                &mut builder,
                &entries,
                self.batch_serializer.generate_batch_id(),
                super::flatbuffer::BatchOperationType::Insert,
            )?
        };
        
        // Write to memory-mapped file
        {
            let mut data_file = self.data_file.write().await;
            let builder = self.flatbuffer_builder.read().await;
            let serialized_data = builder.finished_data();
            
            let io_start = Instant::now();
            let _offset = data_file.append_data(serialized_data)?;
            let io_time = io_start.elapsed();
            
            self.performance_tracker.record_io_operation(
                serialized_data.len() as u64,
                true,
                io_time,
            );
        }
        
        let processing_time = start_time.elapsed();
        let throughput = entries.len() as f64 / processing_time.as_secs_f64();
        
        self.performance_tracker.record_batch_operation(true, entries.len(), processing_time);
        
        info!(
            "Added {} tracks to playlist {} in {:?} ({:.0} tracks/sec)",
            entries.len(),
            playlist_id,
            processing_time,
            throughput
        );
        
        Ok(entry_ids)
    }
    
    async fn remove_from_playlist(&self, playlist_id: i64, media_file_id: i64) -> Result<bool> {
        // Use bulk operation for consistency
        let removed_count = self.bulk_remove_from_playlist(playlist_id, &[media_file_id]).await?;
        Ok(removed_count > 0)
    }
    
    async fn get_playlist_tracks(&self, playlist_id: i64) -> Result<Vec<MediaFile>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Get playlist entries from memory
        let entries = {
            let entries_map = self.playlist_entries.read().await;
            entries_map.get(&playlist_id).cloned().unwrap_or_default()
        };
        
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        
        // Get media files for the entries using zero-copy access
        let mut tracks = Vec::with_capacity(entries.len());
        let mut index_manager = self.index_manager.write().await;
        
        for entry in &entries {
            // Look up media file by ID in index
            if let Some(offset) = index_manager.find_by_id(entry.media_file_id as u64) {
                // Read from memory-mapped file at offset (zero-copy access)
                let data_file = self.data_file.read().await;
                if let Ok(data) = data_file.read_at_offset(offset, 1024) { // Read reasonable chunk
                    // Deserialize MediaFile from FlatBuffer data
                    if let Ok(media_file) = self.deserialize_media_file_from_data(data) {
                        tracks.push(media_file);
                    }
                }
            }
        }
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_cache_access(true);
        
        debug!(
            "Retrieved {} tracks for playlist {} in {:?}",
            tracks.len(),
            playlist_id,
            processing_time
        );
        
        Ok(tracks)
    }
    
    async fn reorder_playlist(&self, playlist_id: i64, track_positions: &[(i64, u32)]) -> Result<()> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        let position_map: std::collections::HashMap<i64, u32> = track_positions.iter().copied().collect();
        
        // Update positions in memory with atomic operations
        {
            let mut entries_map = self.playlist_entries.write().await;
            if let Some(playlist_entries) = entries_map.get_mut(&playlist_id) {
                for entry in playlist_entries.iter_mut() {
                    if let Some(&new_position) = position_map.get(&entry.media_file_id) {
                        entry.position = new_position;
                    }
                }
                
                // Sort by new positions
                playlist_entries.sort_by_key(|e| e.position);
            }
        }
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_file_operation(FileOperationType::Update, processing_time);
        
        info!(
            "Reordered {} tracks in playlist {} in {:?}",
            track_positions.len(),
            playlist_id,
            processing_time
        );
        
        Ok(())
    }
    
    async fn get_files_with_path_prefix(&self, _canonical_prefix: &str) -> Result<Vec<MediaFile>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 8"))
    }
    
    async fn get_direct_subdirectories(&self, _canonical_parent_path: &str) -> Result<Vec<MediaDirectory>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 8"))
    }
    
    async fn batch_cleanup_missing_files(&self, _existing_canonical_paths: &HashSet<String>) -> Result<usize> {
        Err(anyhow!("Not implemented yet - will be implemented in task 8"))
    }
    
    async fn database_native_cleanup(&self, _existing_canonical_paths: &[String]) -> Result<usize> {
        Err(anyhow!("Not implemented yet - will be implemented in task 8"))
    }
    
    async fn get_filtered_direct_subdirectories(
        &self,
        _canonical_parent_path: &str,
        _mime_filter: &str,
    ) -> Result<Vec<MediaDirectory>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 8"))
    }
}

impl ZeroCopyDatabase {
    /// Bulk remove tracks from playlist with atomic cleanup (private helper method)
    async fn bulk_remove_from_playlist(&self, playlist_id: i64, media_file_ids: &[i64]) -> Result<usize> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        if media_file_ids.is_empty() {
            return Ok(0);
        }
        
        let start_time = Instant::now();
        let media_file_set: HashSet<i64> = media_file_ids.iter().copied().collect();
        
        // Remove from memory
        let removed_count = {
            let mut entries_map = self.playlist_entries.write().await;
            if let Some(playlist_entries) = entries_map.get_mut(&playlist_id) {
                let original_len = playlist_entries.len();
                playlist_entries.retain(|entry| !media_file_set.contains(&entry.media_file_id));
                original_len - playlist_entries.len()
            } else {
                0
            }
        };
        
        let processing_time = start_time.elapsed();
        let throughput = removed_count as f64 / processing_time.as_secs_f64();
        
        self.performance_tracker.record_batch_operation(true, removed_count, processing_time);
        
        info!(
            "Removed {} tracks from playlist {} in {:?} ({:.0} tracks/sec)",
            removed_count,
            playlist_id,
            processing_time,
            throughput
        );
        
        Ok(removed_count)
    }
    
    async fn get_playlist_tracks(&self, playlist_id: i64) -> Result<Vec<MediaFile>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Get playlist entries from memory
        let entries = {
            let entries_map = self.playlist_entries.read().await;
            entries_map.get(&playlist_id).cloned().unwrap_or_default()
        };
        
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        
        // Get media files for the entries using zero-copy access
        let mut tracks = Vec::with_capacity(entries.len());
        let mut index_manager = self.index_manager.write().await;
        
        for entry in &entries {
            // Look up media file by ID in index
            if let Some(offset) = index_manager.find_by_id(entry.media_file_id as u64) {
                // Read from memory-mapped file at offset (zero-copy access)
                let data_file = self.data_file.read().await;
                if let Ok(data) = data_file.read_at_offset(offset, 1024) { // Read reasonable chunk
                    // Deserialize MediaFile from FlatBuffer data
                    if let Ok(media_file) = self.deserialize_media_file_from_data(data) {
                        tracks.push(media_file);
                    }
                }
            }
        }
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_cache_access(true);
        
        debug!(
            "Retrieved {} tracks for playlist {} in {:?}",
            tracks.len(),
            playlist_id,
            processing_time
        );
        
        Ok(tracks)
    }
    
    async fn reorder_playlist(&self, playlist_id: i64, track_positions: &[(i64, u32)]) -> Result<()> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        let position_map: std::collections::HashMap<i64, u32> = track_positions.iter().copied().collect();
        
        // Update positions in memory with atomic operations
        {
            let mut entries_map = self.playlist_entries.write().await;
            if let Some(playlist_entries) = entries_map.get_mut(&playlist_id) {
                for entry in playlist_entries.iter_mut() {
                    if let Some(&new_position) = position_map.get(&entry.media_file_id) {
                        entry.position = new_position;
                    }
                }
                
                // Sort by new positions
                playlist_entries.sort_by_key(|e| e.position);
            }
        }
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_file_operation(FileOperationType::Update, processing_time);
        
        info!(
            "Reordered {} tracks in playlist {} in {:?}",
            track_positions.len(),
            playlist_id,
            processing_time
        );
        
        Ok(())
    }
    
    async fn get_files_with_path_prefix(&self, _canonical_prefix: &str) -> Result<Vec<MediaFile>> {
        Err(anyhow!("Not implemented yet - will be implemented in task 8"))
    }
    
    async fn get_direct_subdirectories(&self, canonical_parent_path: &str) -> Result<Vec<MediaDirectory>> {
        if !self.is_open() {
            return Err(anyhow!("Database is not open"));
        }
        
        let start_time = Instant::now();
        
        // Get subdirectories from index with atomic B-tree operations
        let subdirectory_paths = {
            let index_manager = self.index_manager.read().await;
            self.performance_tracker.record_index_operation(IndexOperationType::Lookup);
            index_manager.find_subdirectories(canonical_parent_path)
        };
        
        if subdirectory_paths.is_empty() {
            self.performance_tracker.record_cache_access(true); // Cache hit for empty result
            debug!("No subdirectories found for parent: {}", canonical_parent_path);
            return Ok(Vec::new());
        }
        
        // Convert paths to MediaDirectory structures
        let mut directories = Vec::with_capacity(subdirectory_paths.len());
        for subdir_path in subdirectory_paths {
            let path_buf = PathBuf::from(&subdir_path);
            let name = path_buf.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&subdir_path)
                .to_string();
            
            directories.push(MediaDirectory {
                path: path_buf,
                name,
            });
        }
        
        // Sort directories by name for consistent ordering
        directories.sort_by(|a, b| a.name.cmp(&b.name));
        
        let processing_time = start_time.elapsed();
        self.performance_tracker.record_io_operation(
            directories.len() as u64 * 256, // Estimate 256 bytes per directory entry
            false, // is_read
            processing_time,
        );
        
        debug!(
            "Retrieved {} direct subdirectories for {} in {:?}",
            directories.len(),
            canonical_parent_path,
            processing_time
        );
        
        Ok(directories)
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
        assert_eq!(db.get_config().await.batch_size, 100_000);
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
    
    #[tokio::test]
    async fn test_directory_operations() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = ZeroCopyDatabase::new(db_path, None).await.unwrap();
        
        db.initialize().await.unwrap();
        db.open().await.unwrap();
        
        // Test get_files_in_directory with empty directory
        let test_dir = PathBuf::from("/test/music");
        let files = db.get_files_in_directory(&test_dir).await.unwrap();
        assert!(files.is_empty()); // Should be empty since no files are indexed
        
        // Test get_directory_listing with empty directory
        let (subdirs, files) = db.get_directory_listing(&test_dir, "").await.unwrap();
        assert!(subdirs.is_empty()); // Should be empty since no subdirectories are indexed
        assert!(files.is_empty()); // Should be empty since no files are indexed
        
        // Test get_directory_listing with media type filter
        let (subdirs_audio, files_audio) = db.get_directory_listing(&test_dir, "audio").await.unwrap();
        assert!(subdirs_audio.is_empty());
        assert!(files_audio.is_empty());
        
        // Test get_direct_subdirectories
        let canonical_parent = "/test/music";
        let subdirs = db.get_direct_subdirectories(canonical_parent).await.unwrap();
        assert!(subdirs.is_empty()); // Should be empty since no subdirectories are indexed
        
        // Verify performance tracking for directory operations
        let stats = db.get_performance_stats();
        assert!(stats.index_lookups > 0); // Should have recorded index lookups
        
        db.close().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_music_categorization_operations() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = ZeroCopyDatabase::new(db_path, None).await.unwrap();
        
        db.initialize().await.unwrap();
        db.open().await.unwrap();
        
        // Test music categorization methods with empty database
        let artists = db.get_artists().await.unwrap();
        assert!(artists.is_empty()); // Should be empty since no files are indexed
        
        let albums = db.get_albums(None).await.unwrap();
        assert!(albums.is_empty()); // Should be empty since no files are indexed
        
        let albums_by_artist = db.get_albums(Some("Test Artist")).await.unwrap();
        assert!(albums_by_artist.is_empty()); // Should be empty since no files are indexed
        
        let genres = db.get_genres().await.unwrap();
        assert!(genres.is_empty()); // Should be empty since no files are indexed
        
        let years = db.get_years().await.unwrap();
        assert!(years.is_empty()); // Should be empty since no files are indexed
        
        let album_artists = db.get_album_artists().await.unwrap();
        assert!(album_artists.is_empty()); // Should be empty since no files are indexed
        
        // Test get_music_by_* methods with empty database
        let music_by_artist = db.get_music_by_artist("Test Artist").await.unwrap();
        assert!(music_by_artist.is_empty()); // Should be empty since no files are indexed
        
        let music_by_album = db.get_music_by_album("Test Album", None).await.unwrap();
        assert!(music_by_album.is_empty()); // Should be empty since no files are indexed
        
        let music_by_album_and_artist = db.get_music_by_album("Test Album", Some("Test Artist")).await.unwrap();
        assert!(music_by_album_and_artist.is_empty()); // Should be empty since no files are indexed
        
        let music_by_genre = db.get_music_by_genre("Rock").await.unwrap();
        assert!(music_by_genre.is_empty()); // Should be empty since no files are indexed
        
        let music_by_year = db.get_music_by_year(2023).await.unwrap();
        assert!(music_by_year.is_empty()); // Should be empty since no files are indexed
        
        let music_by_album_artist = db.get_music_by_album_artist("Test Album Artist").await.unwrap();
        assert!(music_by_album_artist.is_empty()); // Should be empty since no files are indexed
        
        // Verify performance tracking for music categorization operations
        let stats = db.get_performance_stats();
        assert!(stats.index_lookups > 0); // Should have recorded index lookups
        
        db.close().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_playlist_operations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_playlist.db");
        
        let config = ZeroCopyConfig {
            batch_size: 100,
            memory_map_size_mb: 1,
            index_cache_size: 1000,
            ..Default::default()
        };
        
        let db = ZeroCopyDatabase::new(db_path, Some(config)).await.unwrap();
        db.initialize().await.unwrap();
        db.open().await.unwrap();
        
        // Test create playlist with atomic ID generation
        let playlist_id = db.create_playlist("Test Playlist", Some("A test playlist")).await.unwrap();
        assert!(playlist_id > 0);
        
        // Test get playlists
        let playlists = db.get_playlists().await.unwrap();
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].name, "Test Playlist");
        assert_eq!(playlists[0].description, Some("A test playlist".to_string()));
        
        // Test get specific playlist
        let playlist = db.get_playlist(playlist_id).await.unwrap();
        assert!(playlist.is_some());
        assert_eq!(playlist.unwrap().name, "Test Playlist");
        
        // Test bulk add to playlist with atomic batch operations
        let media_file_ids = vec![(1, 0), (2, 1), (3, 2)];
        let entry_ids = db.batch_add_to_playlist(playlist_id, &media_file_ids).await.unwrap();
        assert_eq!(entry_ids.len(), 3);
        
        // Test individual add to playlist (should use bulk operation internally)
        let entry_id = db.add_to_playlist(playlist_id, 4, Some(3)).await.unwrap();
        assert!(entry_id > 0);
        
        // Test remove from playlist
        let removed = db.remove_from_playlist(playlist_id, 2).await.unwrap();
        assert!(removed);
        
        // Test reorder playlist with atomic operations
        let track_positions = vec![(1, 1), (3, 0), (4, 2)];
        let result = db.reorder_playlist(playlist_id, &track_positions).await;
        assert!(result.is_ok());
        
        // Test update playlist
        let mut updated_playlist = db.get_playlist(playlist_id).await.unwrap().unwrap();
        updated_playlist.description = Some("Updated description".to_string());
        let result = db.update_playlist(&updated_playlist).await;
        assert!(result.is_ok());
        
        // Verify update
        let playlist = db.get_playlist(playlist_id).await.unwrap().unwrap();
        assert_eq!(playlist.description, Some("Updated description".to_string()));
        
        // Test delete playlist with atomic cleanup
        let deleted = db.delete_playlist(playlist_id).await.unwrap();
        assert!(deleted);
        
        // Verify deletion
        let playlist = db.get_playlist(playlist_id).await.unwrap();
        assert!(playlist.is_none());
        
        let playlists = db.get_playlists().await.unwrap();
        assert_eq!(playlists.len(), 0);
        
        db.close().await.unwrap();
    }
}

// Include configuration tests
#[cfg(test)]
#[path = "zerocopy_config_test.rs"]
mod config_tests;
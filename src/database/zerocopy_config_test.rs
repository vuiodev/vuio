#[cfg(test)]
mod tests {
    use crate::database::zerocopy::{PerformanceProfile, ZeroCopyConfig, ZeroCopyConfigUpdate, ZeroCopyDatabase};
    use std::env;
    use tokio;

    #[test]
    fn test_performance_profile_auto_detection() {
        // Test auto-detection logic
        let profile = PerformanceProfile::auto_detect();
        
        // Should return a valid profile
        match profile {
            PerformanceProfile::Minimal | 
            PerformanceProfile::Balanced | 
            PerformanceProfile::HighPerformance | 
            PerformanceProfile::Maximum => {
                // Valid profile detected
                println!("Auto-detected profile: {:?}", profile);
            }
            PerformanceProfile::Custom => {
                panic!("Auto-detection should not return Custom profile");
            }
        }
    }

    #[test]
    fn test_performance_profile_properties() {
        let profiles = [
            PerformanceProfile::Minimal,
            PerformanceProfile::Balanced,
            PerformanceProfile::HighPerformance,
            PerformanceProfile::Maximum,
        ];

        for profile in &profiles {
            // Test that memory budget is reasonable
            assert!(profile.memory_budget_mb() >= 6);
            assert!(profile.memory_budget_mb() <= 320);
            
            // Test that cache size is reasonable
            assert!(profile.cache_size_mb() >= 4);
            assert!(profile.cache_size_mb() <= 256);
            
            // Test that index cache size is reasonable
            assert!(profile.index_cache_size() >= 100_000);
            assert!(profile.index_cache_size() <= 5_000_000);
            
            // Test that batch size is reasonable
            assert!(profile.batch_size() >= 10_000);
            assert!(profile.batch_size() <= 250_000);
            
            println!("Profile {:?}: {}MB memory, {}MB cache, {} index entries, {} batch size", 
                     profile, profile.memory_budget_mb(), profile.cache_size_mb(), 
                     profile.index_cache_size(), profile.batch_size());
        }
    }

    #[test]
    fn test_zerocopy_config_defaults() {
        let config = ZeroCopyConfig::default();
        
        // Test default values
        assert_eq!(config.performance_profile, PerformanceProfile::Balanced);
        assert!(config.enable_auto_scaling);
        assert!(config.enable_runtime_updates);
        assert_eq!(config.initial_file_size_mb, 10);
        assert_eq!(config.max_file_size_gb, 10);
        assert!(!config.enable_compression); // Disabled for speed
        assert!(config.enable_wal);
        
        // Test that configuration is valid
        config.validate().unwrap();
        assert!(config.check_memory_budget());
    }

    #[test]
    fn test_zerocopy_config_with_profile() {
        let profile = PerformanceProfile::HighPerformance;
        let config = ZeroCopyConfig::with_performance_profile(profile);
        
        assert_eq!(config.performance_profile, profile);
        assert_eq!(config.memory_map_size_mb, profile.cache_size_mb());
        assert_eq!(config.index_cache_size, profile.index_cache_size());
        assert_eq!(config.batch_size, profile.batch_size());
        assert_eq!(config.memory_budget_limit_mb, profile.memory_budget_mb());
        
        config.validate().unwrap();
    }

    #[test]
    fn test_zerocopy_config_from_env() {
        // Set environment variables
        env::set_var("ZEROCOPY_PERFORMANCE_PROFILE", "maximum");
        env::set_var("ZEROCOPY_CACHE_MB", "128");
        env::set_var("ZEROCOPY_INDEX_SIZE", "2000000");
        env::set_var("ZEROCOPY_BATCH_SIZE", "150000");
        env::set_var("ZEROCOPY_MEMORY_BUDGET_MB", "200");
        env::set_var("ZEROCOPY_ENABLE_AUTO_SCALING", "false");
        env::set_var("ZEROCOPY_ENABLE_RUNTIME_UPDATES", "false");
        
        let config = ZeroCopyConfig::from_env();
        
        // Test that environment variables were applied (128MB is valid)
        assert_eq!(config.memory_map_size_mb, 128);
        assert_eq!(config.index_cache_size, 2_000_000);
        assert_eq!(config.batch_size, 150_000);
        assert_eq!(config.memory_budget_limit_mb, 200);
        assert!(!config.enable_auto_scaling);
        assert!(!config.enable_runtime_updates);
        assert_eq!(config.performance_profile, PerformanceProfile::Custom);
        
        config.validate().unwrap();
        
        // Clean up environment variables
        env::remove_var("ZEROCOPY_PERFORMANCE_PROFILE");
        env::remove_var("ZEROCOPY_CACHE_MB");
        env::remove_var("ZEROCOPY_INDEX_SIZE");
        env::remove_var("ZEROCOPY_BATCH_SIZE");
        env::remove_var("ZEROCOPY_MEMORY_BUDGET_MB");
        env::remove_var("ZEROCOPY_ENABLE_AUTO_SCALING");
        env::remove_var("ZEROCOPY_ENABLE_RUNTIME_UPDATES");
    }

    #[test]
    fn test_auto_scaling_memory_pressure() {
        let mut config = ZeroCopyConfig::default();
        config.enable_auto_scaling = true;
        
        let original_profile = config.performance_profile;
        
        // Test high memory pressure (should scale down)
        let scaled_down = config.auto_scale_for_memory_pressure(0.9);
        if scaled_down {
            // Should have scaled to a lower profile
            assert_ne!(config.performance_profile, original_profile);
            println!("Scaled down from {:?} to {:?}", original_profile, config.performance_profile);
        }
        
        // Test low memory pressure (should scale up if possible)
        let scaled_up = config.auto_scale_for_memory_pressure(0.2);
        if scaled_up {
            println!("Scaled up to {:?}", config.performance_profile);
        }
    }

    #[test]
    fn test_runtime_config_updates() {
        let mut config = ZeroCopyConfig::default();
        config.enable_runtime_updates = true;
        
        let update = ZeroCopyConfigUpdate {
            batch_size: Some(75_000),
            memory_map_size_mb: Some(16), // Reduced to fit within default budget
            index_cache_size: Some(750_000),
            performance_profile: None,
        };
        
        let changed = config.update_runtime_config(update).unwrap();
        assert!(changed);
        
        assert_eq!(config.batch_size, 75_000);
        assert_eq!(config.memory_map_size_mb, 16);
        assert_eq!(config.index_cache_size, 750_000);
        assert_eq!(config.performance_profile, PerformanceProfile::Custom);
    }

    #[test]
    fn test_config_validation_bounds() {
        // Test invalid cache size (too small)
        let mut config = ZeroCopyConfig::default();
        config.memory_map_size_mb = 2; // Below minimum of 4
        assert!(config.validate().is_err());
        
        // Test invalid cache size (too large)
        config.memory_map_size_mb = 2048; // Above maximum of 1024
        assert!(config.validate().is_err());
        
        // Test invalid batch size (too small)
        config.memory_map_size_mb = 16; // Reset to valid
        config.batch_size = 500; // Below minimum of 1000
        assert!(config.validate().is_err());
        
        // Test invalid batch size (too large)
        config.batch_size = 2_000_000; // Above maximum of 1,000,000
        assert!(config.validate().is_err());
        
        // Test invalid index cache size
        config.batch_size = 50_000; // Reset to valid
        config.index_cache_size = 5_000; // Below minimum of 10,000
        assert!(config.validate().is_err());
        
        // Test memory budget exceeded
        config.index_cache_size = 500_000; // Reset to valid
        config.memory_map_size_mb = 512;
        config.memory_budget_limit_mb = 100; // Too small for the cache size
        assert!(config.validate().is_err());
        
        // Test valid configuration
        config.memory_map_size_mb = 64;
        config.memory_budget_limit_mb = 80;
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_safe_from_env_with_invalid_values() {
        use std::env;
        
        // Set invalid environment variables
        env::set_var("ZEROCOPY_AUTO_DETECT", "false"); // Disable auto-detection
        env::set_var("ZEROCOPY_CACHE_MB", "2000"); // Too large
        env::set_var("ZEROCOPY_INDEX_SIZE", "5000"); // Too small
        env::set_var("ZEROCOPY_BATCH_SIZE", "500"); // Too small
        env::set_var("ZEROCOPY_MEMORY_BUDGET_MB", "2"); // Too small
        
        // safe_from_env should clamp values to valid ranges
        let config = ZeroCopyConfig::safe_from_env();
        
        assert!(config.memory_map_size_mb <= 1024 && config.memory_map_size_mb >= 4); // Clamped to valid range
        assert!(config.index_cache_size >= 10_000 && config.index_cache_size <= 10_000_000); // Clamped to valid range
        assert!(config.batch_size >= 1_000 && config.batch_size <= 1_000_000); // Clamped to valid range
        assert!(config.memory_budget_limit_mb >= 6); // Clamped to min or auto-adjusted
        
        // Should validate successfully
        assert!(config.validate().is_ok());
        
        // Clean up
        env::remove_var("ZEROCOPY_AUTO_DETECT");
        env::remove_var("ZEROCOPY_CACHE_MB");
        env::remove_var("ZEROCOPY_INDEX_SIZE");
        env::remove_var("ZEROCOPY_BATCH_SIZE");
        env::remove_var("ZEROCOPY_MEMORY_BUDGET_MB");
    }
    
    #[test]
    fn test_runtime_config_updates_disabled() {
        let mut config = ZeroCopyConfig::default();
        config.enable_runtime_updates = false;
        
        let update = ZeroCopyConfigUpdate {
            batch_size: Some(75_000),
            ..Default::default()
        };
        
        let result = config.update_runtime_config(update);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Runtime configuration updates are disabled"));
    }

    #[tokio::test]
    async fn test_zerocopy_database_with_auto_detection() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_auto_detect.db");
        
        let database = ZeroCopyDatabase::new_with_auto_detection(db_path).await.unwrap();
        let config = database.get_config().await;
        
        // Should have auto-detected a valid profile
        match config.performance_profile {
            PerformanceProfile::Minimal | 
            PerformanceProfile::Balanced | 
            PerformanceProfile::HighPerformance | 
            PerformanceProfile::Maximum => {
                println!("Database created with auto-detected profile: {:?}", config.performance_profile);
            }
            PerformanceProfile::Custom => {
                panic!("Auto-detection should not result in Custom profile");
            }
        }
        
        assert!(config.enable_auto_scaling);
        assert!(config.enable_runtime_updates);
    }

    #[tokio::test]
    async fn test_zerocopy_database_config_updates() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_config_updates.db");
        
        let database = ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::Balanced).await.unwrap();
        
        // Test configuration update
        let update = ZeroCopyConfigUpdate {
            batch_size: Some(80_000),
            memory_map_size_mb: Some(16), // Keep within budget limit
            ..Default::default()
        };
        
        let changed = database.update_config(update).await.unwrap();
        assert!(changed);
        
        let updated_config = database.get_config().await;
        assert_eq!(updated_config.batch_size, 80_000);
        assert_eq!(updated_config.memory_map_size_mb, 16);
        assert_eq!(updated_config.performance_profile, PerformanceProfile::Custom);
    }

    #[tokio::test]
    async fn test_zerocopy_database_performance_profile_change() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_profile_change.db");
        
        let database = ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::Minimal).await.unwrap();
        
        // Change to high performance profile
        database.set_performance_profile(PerformanceProfile::HighPerformance).await.unwrap();
        
        let config = database.get_config().await;
        assert_eq!(config.performance_profile, PerformanceProfile::HighPerformance);
        assert_eq!(config.memory_map_size_mb, PerformanceProfile::HighPerformance.cache_size_mb());
        assert_eq!(config.index_cache_size, PerformanceProfile::HighPerformance.index_cache_size());
        assert_eq!(config.batch_size, PerformanceProfile::HighPerformance.batch_size());
    }
}
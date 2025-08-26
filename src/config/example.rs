use anyhow::Result;
use std::path::PathBuf;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

use super::{
    watcher::{ConfigChangeHandler, ConfigWatcherService},
    AppConfig, NetworkInterfaceConfig,
};

/// Example service that responds to configuration changes
pub struct ExampleService {
    name: String,
}

impl ExampleService {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait::async_trait]
impl ConfigChangeHandler for ExampleService {
    async fn handle_config_reload(&self, new_config: &AppConfig) -> Result<()> {
        info!(
            "[{}] Configuration reloaded - server port: {}, directories: {}",
            self.name,
            new_config.server.port,
            new_config.media.directories.len()
        );
        Ok(())
    }

    async fn handle_directory_changes(
        &self,
        added: Vec<PathBuf>,
        removed: Vec<PathBuf>,
        modified: Vec<PathBuf>,
    ) -> Result<()> {
        info!(
            "[{}] Directory changes - added: {}, removed: {}, modified: {}",
            self.name,
            added.len(),
            removed.len(),
            modified.len()
        );

        for dir in &added {
            info!("[{}] New directory to monitor: {}", self.name, dir.display());
        }

        for dir in &removed {
            info!("[{}] Directory removed from monitoring: {}", self.name, dir.display());
        }

        Ok(())
    }

    async fn handle_network_changes(
        &self,
        old_interface: NetworkInterfaceConfig,
        new_interface: NetworkInterfaceConfig,
        old_port: u16,
        new_port: u16,
    ) -> Result<()> {
        info!(
            "[{}] Network configuration changed - interface: {:?} -> {:?}, port: {} -> {}",
            self.name, old_interface, new_interface, old_port, new_port
        );

        if old_port != new_port {
            warn!(
                "[{}] Port change detected - service restart may be required",
                self.name
            );
        }

        Ok(())
    }
}

/// Demonstrate configuration hot-reloading functionality
pub async fn demonstrate_config_hot_reload() -> Result<()> {
    use tempfile::{NamedTempFile, TempDir};

    info!("Starting configuration hot-reload demonstration");

    // Create a temporary config file and directory
    let temp_file = NamedTempFile::new()?;
    let config_path = temp_file.path().to_path_buf();
    let temp_dir = TempDir::new()?;

    // Delete the temp file so we can create our own config
    std::fs::remove_file(&config_path).ok();

    // Create a config with a temporary directory that exists
    let mut config = super::AppConfig::default();
    config.media.directories = vec![
        super::MonitoredDirectoryConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            recursive: true,
            extensions: None,
            exclude_patterns: None,
        }
    ];
    config.save_to_file(&config_path)?;

    // Create the configuration watcher service
    let watcher_service = ConfigWatcherService::new(config_path.clone()).await?;
    let config_manager = watcher_service.get_config_manager();

    // Get initial configuration
    let initial_config = config_manager.get_config().await;
    info!("Initial configuration loaded:");
    info!("  Server port: {}", initial_config.server.port);
    info!("  Server name: {}", initial_config.server.name);
    info!("  Monitored directories: {}", initial_config.media.directories.len());

    // Demonstrate programmatic configuration update
    info!("Updating configuration programmatically...");
    let mut updated_config = initial_config.clone();
    updated_config.server.port = 9090;
    updated_config.server.name = "Updated VuIO Server".to_string();

    config_manager.update_config(updated_config).await?;

    // Wait a moment for the change to propagate
    sleep(Duration::from_millis(100)).await;

    let current_config = config_manager.get_config().await;
    info!("Configuration after programmatic update:");
    info!("  Server port: {}", current_config.server.port);
    info!("  Server name: {}", current_config.server.name);

    // Demonstrate file-based configuration update
    info!("Updating configuration via file modification...");
    let mut file_config = current_config.clone();
    file_config.server.port = 8888;
    // Use the same temp directory to avoid validation errors
    file_config.media.directories.push(super::MonitoredDirectoryConfig {
        path: temp_dir.path().join("subdir").to_string_lossy().to_string(),
        recursive: true,
        extensions: None,
        exclude_patterns: Some(vec!["*.log".to_string()]),
    });
    
    // Create the subdirectory so it exists for validation
    std::fs::create_dir_all(temp_dir.path().join("subdir"))?;

    // Save directly to file to simulate external modification
    file_config.save_to_file(&config_path)?;

    // Wait for file watcher to detect the change
    sleep(Duration::from_millis(600)).await;

    let final_config = config_manager.get_config().await;
    info!("Configuration after file modification:");
    info!("  Server port: {}", final_config.server.port);
    info!("  Monitored directories: {}", final_config.media.directories.len());

    info!("Configuration hot-reload demonstration completed successfully");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;


    #[tokio::test]
    async fn test_example_service() -> Result<()> {
        let service = ExampleService::new("TestService".to_string());

        // Test config reload handling
        let config = AppConfig::default_for_platform();
        service.handle_config_reload(&config).await?;

        // Test directory changes handling
        service
            .handle_directory_changes(
                vec![PathBuf::from("/new/dir")],
                vec![PathBuf::from("/old/dir")],
                vec![PathBuf::from("/modified/dir")],
            )
            .await?;

        // Test network changes handling
        service
            .handle_network_changes(
                NetworkInterfaceConfig::Auto,
                NetworkInterfaceConfig::Specific("eth0".to_string()),
                8080,
                9090,
            )
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_demonstrate_config_hot_reload() -> Result<()> {
        // This test just ensures the demonstration function doesn't panic
        // In a real scenario, you'd want to capture and verify the log output
        demonstrate_config_hot_reload().await?;
        Ok(())
    }
}
use anyhow::Result;
use std::{
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

use super::{AppConfig, ConfigChangeEvent, ConfigManager};

/// Configuration hot-reload service that manages runtime configuration changes
pub struct ConfigWatcherService {
    config_manager: Arc<ConfigManager>,
    change_receiver: broadcast::Receiver<ConfigChangeEvent>,
}

impl ConfigWatcherService {
    /// Create a new configuration watcher service
    pub async fn new(config_path: PathBuf) -> Result<Self> {
        let config_manager = Arc::new(ConfigManager::new_with_watching(config_path).await?);
        let change_receiver = config_manager.subscribe_to_changes();
        
        Ok(Self {
            config_manager,
            change_receiver,
        })
    }

    /// Get the configuration manager
    pub fn get_config_manager(&self) -> Arc<ConfigManager> {
        self.config_manager.clone()
    }

    /// Start the configuration watcher service
    pub async fn start(mut self) -> Result<()> {
        info!("Starting configuration watcher service");
        
        while let Ok(event) = self.change_receiver.recv().await {
            match event {
                ConfigChangeEvent::Reloaded(new_config) => {
                    info!("Configuration reloaded successfully");
                    self.handle_config_reload(&new_config).await;
                }
                ConfigChangeEvent::DirectoriesChanged { added, removed, modified } => {
                    info!("Monitored directories changed: +{} -{} ~{}", 
                          added.len(), removed.len(), modified.len());
                    self.handle_directory_changes(added, removed, modified).await;
                }
                ConfigChangeEvent::NetworkChanged { old_interface, new_interface, old_port, new_port } => {
                    info!("Network configuration changed: interface {:?} -> {:?}, port {} -> {}", 
                          old_interface, new_interface, old_port, new_port);
                    self.handle_network_changes(old_interface, new_interface, old_port, new_port).await;
                }
            }
        }
        
        Ok(())
    }

    /// Handle configuration reload events
    async fn handle_config_reload(&self, _new_config: &AppConfig) {
        // This is a general reload event - specific handlers will be called for individual changes
        info!("Configuration has been reloaded from file");
    }

    /// Handle changes to monitored directories
    async fn handle_directory_changes(
        &self,
        added: Vec<PathBuf>,
        removed: Vec<PathBuf>,
        modified: Vec<PathBuf>,
    ) {
        for dir in &added {
            info!("Adding directory to monitoring: {}", dir.display());
            // TODO: Integrate with file watcher service to add new directories
        }
        
        for dir in &removed {
            info!("Removing directory from monitoring: {}", dir.display());
            // TODO: Integrate with file watcher service to remove directories
        }
        
        for dir in &modified {
            info!("Directory configuration modified: {}", dir.display());
            // TODO: Update directory monitoring settings
        }
        
        if !added.is_empty() || !removed.is_empty() || !modified.is_empty() {
            info!("Directory monitoring configuration updated - file watcher will be notified");
        }
    }

    /// Handle network configuration changes
    async fn handle_network_changes(
        &self,
        _old_interface: super::NetworkInterfaceConfig,
        _new_interface: super::NetworkInterfaceConfig,
        old_port: u16,
        new_port: u16,
    ) {
        if old_port != new_port {
            warn!("Server port changed from {} to {} - restart required for this change to take effect", 
                  old_port, new_port);
        }
        
        info!("Network interface configuration updated - SSDP service will be notified");
        // TODO: Integrate with SSDP service to update network interface selection
    }
}

/// Configuration change handler trait for services that need to respond to config changes
#[async_trait::async_trait]
pub trait ConfigChangeHandler: Send + Sync {
    /// Handle configuration reload
    async fn handle_config_reload(&self, new_config: &AppConfig) -> Result<()>;
    
    /// Handle directory changes
    async fn handle_directory_changes(
        &self,
        added: Vec<PathBuf>,
        removed: Vec<PathBuf>,
        modified: Vec<PathBuf>,
    ) -> Result<()>;
    
    /// Handle network configuration changes
    async fn handle_network_changes(
        &self,
        old_interface: super::NetworkInterfaceConfig,
        new_interface: super::NetworkInterfaceConfig,
        old_port: u16,
        new_port: u16,
    ) -> Result<()>;
}

/// Registry for configuration change handlers
pub struct ConfigChangeRegistry {
    handlers: Vec<Arc<dyn ConfigChangeHandler>>,
}

impl ConfigChangeRegistry {
    /// Create a new configuration change registry
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Register a configuration change handler
    pub fn register_handler(&mut self, handler: Arc<dyn ConfigChangeHandler>) {
        self.handlers.push(handler);
    }

    /// Notify all handlers of a configuration change
    pub async fn notify_handlers(&self, event: &ConfigChangeEvent) -> Result<()> {
        match event {
            ConfigChangeEvent::Reloaded(new_config) => {
                for handler in &self.handlers {
                    if let Err(e) = handler.handle_config_reload(new_config).await {
                        warn!("Handler failed to process config reload: {}", e);
                    }
                }
            }
            ConfigChangeEvent::DirectoriesChanged { added, removed, modified } => {
                for handler in &self.handlers {
                    if let Err(e) = handler.handle_directory_changes(
                        added.clone(),
                        removed.clone(),
                        modified.clone(),
                    ).await {
                        warn!("Handler failed to process directory changes: {}", e);
                    }
                }
            }
            ConfigChangeEvent::NetworkChanged { old_interface, new_interface, old_port, new_port } => {
                for handler in &self.handlers {
                    if let Err(e) = handler.handle_network_changes(
                        old_interface.clone(),
                        new_interface.clone(),
                        *old_port,
                        *new_port,
                    ).await {
                        warn!("Handler failed to process network changes: {}", e);
                    }
                }
            }
        }
        
        Ok(())
    }
}

impl Default for ConfigChangeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_config_watcher_service_creation() -> Result<()> {
        let temp_file = NamedTempFile::new()?;
        let config_path = temp_file.path().to_path_buf();
        
        // Delete the temp file so we can test creation
        std::fs::remove_file(&config_path).ok();
        
        let service = ConfigWatcherService::new(config_path).await?;
        let config_manager = service.get_config_manager();
        
        let config = config_manager.get_config().await;
        assert_eq!(config.server.port, 8080);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_config_change_registry() -> Result<()> {
        struct TestHandler {
            reload_count: Arc<RwLock<usize>>,
        }

        #[async_trait::async_trait]
        impl ConfigChangeHandler for TestHandler {
            async fn handle_config_reload(&self, _new_config: &AppConfig) -> Result<()> {
                let mut count = self.reload_count.write().await;
                *count += 1;
                Ok(())
            }

            async fn handle_directory_changes(
                &self,
                _added: Vec<PathBuf>,
                _removed: Vec<PathBuf>,
                _modified: Vec<PathBuf>,
            ) -> Result<()> {
                Ok(())
            }

            async fn handle_network_changes(
                &self,
                _old_interface: super::super::NetworkInterfaceConfig,
                _new_interface: super::super::NetworkInterfaceConfig,
                _old_port: u16,
                _new_port: u16,
            ) -> Result<()> {
                Ok(())
            }
        }

        let reload_count = Arc::new(RwLock::new(0));
        let handler = Arc::new(TestHandler {
            reload_count: reload_count.clone(),
        });

        let mut registry = ConfigChangeRegistry::new();
        registry.register_handler(handler);

        let config = AppConfig::default_for_platform();
        let event = ConfigChangeEvent::Reloaded(config);
        
        registry.notify_handlers(&event).await?;
        
        let count = *reload_count.read().await;
        assert_eq!(count, 1);

        Ok(())
    }
}
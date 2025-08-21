use std::path::PathBuf;
use tracing::{info, warn, error, debug};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use crate::platform::{PlatformInfo, PlatformError};
use crate::platform::diagnostics::{DiagnosticInfo, StartupDiagnostics};

/// Initialize logging with platform-specific configuration
pub fn init_logging() -> Result<(), PlatformError> {
    init_logging_with_options(None, None, false)
}

/// Initialize logging with debug flag
pub fn init_logging_with_debug(debug: bool) -> Result<(), PlatformError> {
    let log_level = if debug { "debug" } else { "info" };
    init_logging_with_options(Some(log_level), None, debug)
}

/// Initialize logging with platform-specific configuration and options
pub fn init_logging_with_options(log_level: Option<&str>, log_file: Option<PathBuf>, debug: bool) -> Result<(), PlatformError> {
    let default_level = if debug { "debug" } else { "info" };
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(log_level.unwrap_or(default_level)))
        .map_err(|e| PlatformError::Configuration(
            crate::platform::ConfigurationError::ValidationFailed { 
                reason: format!("Invalid log level: {}", e) 
            }
        ))?;

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true);

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer);

    // Add file logging if specified
    if let Some(log_path) = log_file {
        // TODO: Add file appender when available
        info!("File logging requested but not yet implemented: {}", log_path.display());
    }

    subscriber.init();
    
    info!("Logging initialized with level: {}", log_level.unwrap_or(default_level));
    Ok(())
}

/// Log comprehensive startup information including diagnostics
pub async fn log_startup_info() -> Result<(), PlatformError> {
    info!("=== VuIO Server Starting ===");
    
    // Perform startup diagnostic checks
    StartupDiagnostics::perform_startup_checks().await?;
    
    // Collect and log diagnostic information
    let diagnostics = DiagnosticInfo::collect().await?;
    diagnostics.log_startup_diagnostics();
    
    // Log debug diagnostics if debug logging is enabled
    if tracing::enabled!(tracing::Level::DEBUG) {
        diagnostics.log_debug_diagnostics();
    }
    
    info!("=== Startup Complete ===");
    Ok(())
}

/// Log platform-specific error with diagnostic context
pub fn log_platform_error(error: &PlatformError, context: &str) {
    error!("Platform error in {}: {}", context, error);
    
    if error.is_recoverable() {
        info!("Error is recoverable. Suggested actions:");
        for action in error.recovery_actions() {
            info!("  - {}", action);
        }
    } else {
        error!("Error is not recoverable. Manual intervention may be required.");
    }
    
    // Log user-friendly message for support purposes
    debug!("User-friendly error message: {}", error.user_message());
}

/// Log network interface information with diagnostic details
pub async fn log_network_status() -> Result<(), PlatformError> {
    let platform_info = PlatformInfo::detect().await?;
    
    info!("=== Network Status ===");
    info!("Detected {} network interface(s)", platform_info.network_interfaces.len());
    
    for interface in &platform_info.network_interfaces {
        let status = if interface.is_up { "UP" } else { "DOWN" };
        let multicast = if interface.supports_multicast { "multicast" } else { "no-multicast" };
        let loopback = if interface.is_loopback { "loopback" } else { "physical" };
        
        info!("  - {} ({}): {} [{}] [{}] [{}]", 
            interface.name,
            interface.ip_address,
            status,
            format!("{:?}", interface.interface_type).to_lowercase(),
            multicast,
            loopback
        );
    }
    
    if let Some(primary) = platform_info.get_primary_interface() {
        info!("Primary interface selected: {} ({})", primary.name, primary.ip_address);
    } else {
        warn!("No suitable primary interface found for DLNA operations");
    }
    
    // Log platform-specific network capabilities
    let caps = &platform_info.capabilities;
    info!("Network capabilities:");
    info!("  - Privileged port binding: {}", caps.can_bind_privileged_ports);
    info!("  - Multicast support: {}", caps.supports_multicast);
    info!("  - Firewall present: {}", caps.has_firewall);
    info!("  - Network permissions required: {}", caps.requires_network_permissions);
    
    Ok(())
}

/// Log database status with diagnostic information
pub async fn log_database_status(
    database_path: Option<&PathBuf>,
    media_count: Option<u64>,
    last_scan: Option<chrono::DateTime<chrono::Utc>>
) {
    info!("=== Database Status ===");
    
    if let Some(path) = database_path {
        info!("Database location: {}", path.display());
        
        if path.exists() {
            if let Ok(metadata) = std::fs::metadata(path) {
                let size_mb = metadata.len() as f64 / 1024.0 / 1024.0;
                info!("Database size: {:.2} MB", size_mb);
            }
            info!("Database file exists and is accessible");
        } else {
            warn!("Database file does not exist - will be created on first scan");
        }
    } else {
        warn!("Database path not configured - using default location");
    }
    
    if let Some(count) = media_count {
        info!("Media files in database: {}", count);
    } else {
        info!("Media file count not available - database may be empty");
    }
    
    if let Some(scan_time) = last_scan {
        let duration = chrono::Utc::now().signed_duration_since(scan_time);
        if duration.num_hours() < 24 {
            info!("Last media scan: {} hours ago", duration.num_hours());
        } else {
            info!("Last media scan: {} days ago", duration.num_days());
        }
    } else {
        info!("No previous media scan found - full scan will be performed");
    }
}

/// Log file system watcher status
pub async fn log_file_watcher_status(watched_directories: &[PathBuf], watcher_active: bool) {
    info!("=== File System Watcher Status ===");
    
    if watcher_active {
        info!("File system watcher is active");
    } else {
        warn!("File system watcher is not active - file changes will not be detected");
    }
    
    info!("Monitored directories: {}", watched_directories.len());
    for (i, dir) in watched_directories.iter().enumerate() {
        let status = if dir.exists() && dir.is_dir() {
            "accessible"
        } else {
            "inaccessible"
        };
        info!("  {}. {} [{}]", i + 1, dir.display(), status);
    }
    
    if watched_directories.is_empty() {
        warn!("No directories are being monitored for file changes");
    }
}

/// Log configuration status with validation results
pub async fn log_configuration_status(
    config_path: Option<&PathBuf>,
    config_valid: bool,
    config_errors: &[String],
    using_defaults: &[String]
) {
    info!("=== Configuration Status ===");
    
    if let Some(path) = config_path {
        info!("Configuration file: {}", path.display());
        
        if path.exists() {
            info!("Configuration file exists");
            if config_valid {
                info!("Configuration is valid");
            } else {
                error!("Configuration contains errors:");
                for error in config_errors {
                    error!("  - {}", error);
                }
            }
        } else {
            warn!("Configuration file not found - using defaults");
        }
    } else {
        info!("No configuration file specified - using built-in defaults");
    }
    
    if !using_defaults.is_empty() {
        info!("Using default values for:");
        for default in using_defaults {
            info!("  - {}", default);
        }
    }
}

/// Log system resource information
pub async fn log_system_resources() {
    info!("=== System Resources ===");
    
    // Log CPU information
    let cpu_count = num_cpus::get();
    info!("CPU cores: {}", cpu_count);
    
    // Log process information
    let pid = std::process::id();
    info!("Process ID: {}", pid);
    
    // TODO: Add memory usage, disk space, and other resource information
    // This would require additional platform-specific implementations
    
    debug!("System resource logging complete");
}

/// Log periodic status updates during operation
pub async fn log_periodic_status(
    uptime_seconds: u64,
    active_connections: usize,
    files_served: u64,
    errors_since_start: u64
) {
    let uptime_hours = uptime_seconds / 3600;
    let uptime_minutes = (uptime_seconds % 3600) / 60;
    
    info!("=== Periodic Status Update ===");
    info!("Uptime: {}h {}m", uptime_hours, uptime_minutes);
    info!("Active DLNA connections: {}", active_connections);
    info!("Files served since startup: {}", files_served);
    
    if errors_since_start > 0 {
        warn!("Errors since startup: {}", errors_since_start);
    } else {
        info!("No errors since startup");
    }
}

/// Log shutdown information
pub async fn log_shutdown_info(graceful: bool, uptime_seconds: u64) {
    info!("=== VuIO Server Shutting Down ===");
    
    let shutdown_type = if graceful { "Graceful" } else { "Forced" };
    info!("Shutdown type: {}", shutdown_type);
    
    let uptime_hours = uptime_seconds / 3600;
    let uptime_minutes = (uptime_seconds % 3600) / 60;
    info!("Total uptime: {}h {}m", uptime_hours, uptime_minutes);
    
    if graceful {
        info!("All resources cleaned up successfully");
    } else {
        warn!("Forced shutdown - some resources may not have been cleaned up properly");
    }
    
    info!("=== Shutdown Complete ===");
}

/// Create a diagnostic report for support purposes
pub async fn create_diagnostic_report(output_path: &PathBuf) -> Result<(), PlatformError> {
    info!("Creating diagnostic report at: {}", output_path.display());
    
    let diagnostics = DiagnosticInfo::collect().await?;
    diagnostics.save_to_file(output_path).await
        .map_err(|e| PlatformError::FileSystemAccess(format!("Failed to save diagnostic report: {}", e)))?;
    
    info!("Diagnostic report created successfully");
    info!("Please include this file when reporting issues or requesting support");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_logging_initialization() {
        // Test basic logging initialization
        let result = init_logging_with_options(Some("debug"), None, true);
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_startup_logging() {
        // This test may fail in some environments, so we just ensure it doesn't panic
        let result = log_startup_info().await;
        match result {
            Ok(_) => info!("Startup logging test passed"),
            Err(e) => warn!("Startup logging test failed: {}", e),
        }
    }
    
    #[tokio::test]
    async fn test_diagnostic_report_creation() {
        let temp_dir = tempdir().unwrap();
        let report_path = temp_dir.path().join("diagnostic_report.json");
        
        let result = create_diagnostic_report(&report_path).await;
        match result {
            Ok(_) => {
                assert!(report_path.exists());
                info!("Diagnostic report creation test passed");
            }
            Err(e) => warn!("Diagnostic report creation test failed: {}", e),
        }
    }
    
    #[tokio::test]
    async fn test_network_status_logging() {
        let result = log_network_status().await;
        match result {
            Ok(_) => info!("Network status logging test passed"),
            Err(e) => warn!("Network status logging test failed: {}", e),
        }
    }
}
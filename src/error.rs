use axum::{
    http::{Error as HttpError, StatusCode},
    response::{IntoResponse, Response},
};
use thiserror::Error;
use crate::platform::{PlatformError, DatabaseError, ConfigurationError};

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Not Found")]
    NotFound,

    #[error("Internal Server Error")]
    Internal(#[from] anyhow::Error),

    #[error("I/O Error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid Range Header")]
    InvalidRange,

    #[error("HTTP error: {0}")]
    Http(#[from] HttpError),

    #[error("File watcher error: {0}")]
    Watcher(#[from] notify::Error),

    #[error("Platform error: {0}")]
    Platform(#[from] PlatformError),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Configuration error: {0}")]
    Configuration(#[from] ConfigurationError),

    #[error("Media scanning error: {0}")]
    MediaScan(String),

    #[error("Network discovery error: {0}")]
    NetworkDiscovery(String),

    #[error("File serving error: {0}")]
    FileServing(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::InvalidRange => (StatusCode::RANGE_NOT_SATISFIABLE, self.to_string()),
            AppError::Platform(platform_err) => {
                // Use platform-specific error messages with troubleshooting info
                (StatusCode::INTERNAL_SERVER_ERROR, platform_err.user_message())
            }
            AppError::Database(db_err) => {
                // Include database recovery information
                let recovery_info = db_err.recovery_strategy();
                (StatusCode::INTERNAL_SERVER_ERROR, format!("{}\n\nRecovery: {}", db_err, recovery_info))
            }
            AppError::Configuration(config_err) => {
                // Include configuration solution guidance
                let solution = config_err.solution_guide();
                (StatusCode::INTERNAL_SERVER_ERROR, format!("{}\n\nSolution: {}", config_err, solution))
            }
            AppError::MediaScan(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Media scanning failed: {}. Try rescanning or check directory permissions.", msg))
            }
            AppError::NetworkDiscovery(msg) => {
                (StatusCode::SERVICE_UNAVAILABLE, format!("Network discovery failed: {}. Check network configuration and firewall settings.", msg))
            }
            AppError::FileServing(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("File serving error: {}. Check file permissions and disk space.", msg))
            }
            AppError::Internal(_) | AppError::Io(_) | AppError::Http(_) | AppError::Watcher(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
        };

        (status, message).into_response()
    }
}

impl AppError {
    /// Check if the error is recoverable and the operation can be retried
    pub fn is_recoverable(&self) -> bool {
        match self {
            AppError::NotFound => false,
            AppError::InvalidRange => false,
            AppError::Internal(_) => false,
            AppError::Io(io_err) => {
                // Some I/O errors are recoverable (temporary network issues, etc.)
                match io_err.kind() {
                    std::io::ErrorKind::TimedOut => true,
                    std::io::ErrorKind::Interrupted => true,
                    std::io::ErrorKind::WouldBlock => true,
                    std::io::ErrorKind::ConnectionRefused => true,
                    std::io::ErrorKind::ConnectionAborted => true,
                    std::io::ErrorKind::NotConnected => true,
                    _ => false,
                }
            }
            AppError::Http(_) => false,
            AppError::Watcher(_) => true, // File watcher can often be restarted
            AppError::Platform(platform_err) => platform_err.is_recoverable(),
            AppError::Database(db_err) => db_err.is_recoverable(),
            AppError::Configuration(config_err) => config_err.is_recoverable(),
            AppError::MediaScan(_) => true, // Media scanning can usually be retried
            AppError::NetworkDiscovery(_) => true, // Network discovery can be retried
            AppError::FileServing(_) => true, // File serving issues might be temporary
        }
    }
    
    /// Get suggested recovery actions for the error
    pub fn recovery_actions(&self) -> Vec<String> {
        match self {
            AppError::Platform(platform_err) => platform_err.recovery_actions(),
            AppError::Database(db_err) => db_err.recovery_actions(),
            AppError::Configuration(config_err) => config_err.recovery_actions(),
            AppError::Watcher(_) => vec![
                "Restart file system watcher".to_string(),
                "Check directory permissions".to_string(),
                "Verify watched directories exist".to_string(),
            ],
            AppError::MediaScan(msg) => vec![
                "Retry media scanning".to_string(),
                "Check directory permissions".to_string(),
                "Verify media directories are accessible".to_string(),
                format!("Review error details: {}", msg),
            ],
            AppError::NetworkDiscovery(msg) => vec![
                "Check network connectivity".to_string(),
                "Verify firewall settings".to_string(),
                "Try alternative network interfaces".to_string(),
                "Restart network discovery service".to_string(),
                format!("Review error details: {}", msg),
            ],
            AppError::FileServing(msg) => vec![
                "Check file permissions".to_string(),
                "Verify disk space availability".to_string(),
                "Restart file serving component".to_string(),
                format!("Review error details: {}", msg),
            ],
            AppError::Io(io_err) => match io_err.kind() {
                std::io::ErrorKind::TimedOut => vec![
                    "Retry the operation".to_string(),
                    "Check network connectivity".to_string(),
                    "Increase timeout values".to_string(),
                ],
                std::io::ErrorKind::PermissionDenied => vec![
                    "Check file/directory permissions".to_string(),
                    "Run with appropriate privileges".to_string(),
                    "Verify user has access rights".to_string(),
                ],
                std::io::ErrorKind::NotFound => vec![
                    "Verify the file or directory exists".to_string(),
                    "Check the path is correct".to_string(),
                    "Create missing directories if needed".to_string(),
                ],
                _ => vec![
                    "Check system resources".to_string(),
                    "Verify file system integrity".to_string(),
                    "Restart the application".to_string(),
                ],
            },
            _ => vec![
                "Restart the application".to_string(),
                "Check system logs for details".to_string(),
                "Contact support if problem persists".to_string(),
            ],
        }
    }
    
    /// Get a user-friendly error message with context and guidance
    pub fn user_friendly_message(&self) -> String {
        match self {
            AppError::Platform(platform_err) => platform_err.user_message(),
            AppError::Database(db_err) => {
                format!("Database Error: {}\n\nWhat this means: The application's database encountered an issue.\nRecovery: {}", 
                    db_err, db_err.recovery_strategy())
            }
            AppError::Configuration(config_err) => {
                format!("Configuration Error: {}\n\nWhat this means: There's an issue with the application configuration.\nSolution: {}", 
                    config_err, config_err.solution_guide())
            }
            AppError::MediaScan(msg) => {
                format!("Media Scanning Error: {}\n\nWhat this means: The application couldn't scan your media directories.\nSuggestion: Check that the directories exist and are accessible.", msg)
            }
            AppError::NetworkDiscovery(msg) => {
                format!("Network Discovery Error: {}\n\nWhat this means: The application couldn't set up network discovery for DLNA clients.\nSuggestion: Check your network settings and firewall configuration.", msg)
            }
            AppError::FileServing(msg) => {
                format!("File Serving Error: {}\n\nWhat this means: The application couldn't serve a media file to a client.\nSuggestion: Check file permissions and available disk space.", msg)
            }
            AppError::Watcher(err) => {
                format!("File Monitoring Error: {}\n\nWhat this means: The application couldn't monitor directories for file changes.\nSuggestion: Check directory permissions and restart the application.", err)
            }
            AppError::Io(err) => {
                format!("System Error: {}\n\nWhat this means: A system-level operation failed.\nSuggestion: Check file permissions, disk space, and network connectivity.", err)
            }
            _ => format!("Application Error: {}\n\nSuggestion: Try restarting the application or check the logs for more details.", self),
        }
    }
    
    /// Log the error with appropriate level and context
    pub fn log_error(&self) {
        match self {
            AppError::Platform(platform_err) => {
                tracing::error!("Platform error: {}", platform_err);
                if platform_err.is_recoverable() {
                    tracing::info!("Recovery actions available: {:?}", platform_err.recovery_actions());
                }
            }
            AppError::Database(db_err) => {
                tracing::error!("Database error: {}", db_err);
                if db_err.is_recoverable() {
                    tracing::info!("Database recovery strategy: {}", db_err.recovery_strategy());
                }
            }
            AppError::Configuration(config_err) => {
                tracing::warn!("Configuration error: {}", config_err);
                tracing::info!("Configuration solution: {}", config_err.solution_guide());
            }
            AppError::MediaScan(msg) => {
                tracing::warn!("Media scan error: {}", msg);
                tracing::info!("Media scanning can be retried or directories can be reconfigured");
            }
            AppError::NetworkDiscovery(msg) => {
                tracing::error!("Network discovery error: {}", msg);
                tracing::info!("Check network configuration and firewall settings");
            }
            AppError::FileServing(msg) => {
                tracing::warn!("File serving error: {}", msg);
                tracing::info!("Check file permissions and disk space");
            }
            AppError::Watcher(err) => {
                tracing::warn!("File watcher error: {}", err);
                tracing::info!("File monitoring can be restarted");
            }
            AppError::NotFound => {
                tracing::debug!("Resource not found - this is normal for some requests");
            }
            AppError::InvalidRange => {
                tracing::debug!("Invalid range request - client issue");
            }
            _ => {
                tracing::error!("Application error: {}", self);
            }
        }
    }
}

/// Trait for implementing automatic error recovery
pub trait ErrorRecovery {
    type Output;
    type Error;
    
    /// Attempt to recover from an error and retry the operation
    async fn recover_and_retry(&self, error: &Self::Error, max_attempts: u32) -> std::result::Result<Self::Output, Self::Error>;
}

/// Helper function to retry operations with exponential backoff
pub async fn retry_with_backoff<F, T, E>(
    mut operation: F,
    max_attempts: u32,
    initial_delay_ms: u64,
) -> std::result::Result<T, E>
where
    F: FnMut() -> std::result::Result<T, E>,
    E: std::fmt::Debug,
{
    let mut delay_ms = initial_delay_ms;
    
    for attempt in 1..=max_attempts {
        match operation() {
            Ok(result) => return Ok(result),
            Err(error) => {
                if attempt == max_attempts {
                    tracing::error!("Operation failed after {} attempts: {:?}", max_attempts, error);
                    return Err(error);
                }
                
                tracing::warn!("Operation failed on attempt {}/{}: {:?}. Retrying in {}ms", 
                    attempt, max_attempts, error, delay_ms);
                
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                delay_ms = (delay_ms * 2).min(30000); // Cap at 30 seconds
            }
        }
    }
    
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{WindowsError, DatabaseError, ConfigurationError};
    use std::path::PathBuf;
    
    #[test]
    fn test_error_recoverability() {
        let recoverable_error = AppError::MediaScan("temporary failure".to_string());
        assert!(recoverable_error.is_recoverable());
        
        let non_recoverable_error = AppError::NotFound;
        assert!(!non_recoverable_error.is_recoverable());
    }
    
    #[test]
    fn test_platform_error_integration() {
        let windows_error = PlatformError::Windows(WindowsError::PrivilegedPortAccess { port: 1900 });
        let app_error = AppError::Platform(windows_error);
        
        assert!(app_error.is_recoverable());
        assert!(!app_error.recovery_actions().is_empty());
    }
    
    #[test]
    fn test_database_error_integration() {
        let db_error = DatabaseError::CorruptionDetected { 
            location: "media_table".to_string() 
        };
        let app_error = AppError::Database(db_error);
        
        assert!(app_error.is_recoverable());
        let message = app_error.user_friendly_message();
        assert!(message.contains("Database Error"));
        assert!(message.contains("Recovery"));
    }
    
    #[test]
    fn test_configuration_error_integration() {
        let config_error = ConfigurationError::FileNotFound { 
            path: PathBuf::from("/etc/vuio/config.toml") 
        };
        let app_error = AppError::Configuration(config_error);
        
        assert!(app_error.is_recoverable());
        let actions = app_error.recovery_actions();
        assert!(actions.iter().any(|action| action.contains("default configuration")));
    }
    
    #[tokio::test]
    async fn test_retry_with_backoff() {
        let mut attempt_count = 0;
        let result = retry_with_backoff(
            || {
                attempt_count += 1;
                if attempt_count < 3 {
                    Err("temporary failure")
                } else {
                    Ok("success")
                }
            },
            5,
            10, // 10ms initial delay for fast test
        ).await;
        
        assert_eq!(result, Ok("success"));
        assert_eq!(attempt_count, 3);
    }
}

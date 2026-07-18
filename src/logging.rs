use crate::platform::PlatformError;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initialize logging with platform-specific configuration.
pub fn init_logging() -> Result<(), PlatformError> {
    init_logging_with_options(None, None, false)
}

/// Initialize logging with debug output enabled.
pub fn init_logging_with_debug(debug: bool) -> Result<(), PlatformError> {
    let log_level = if debug { "debug" } else { "info" };
    init_logging_with_options(Some(log_level), None, debug)
}

/// Initialize console and rolling application-file logging.
pub fn init_logging_with_options(
    log_level: Option<&str>,
    log_file: Option<PathBuf>,
    debug: bool,
) -> Result<(), PlatformError> {
    let is_rust_log_set = std::env::var("RUST_LOG").is_ok();
    let in_docker = crate::config::AppConfig::is_running_in_docker();
    let console_should_be_verbose = debug || is_rust_log_set || log_level.is_some() || in_docker;
    let console_level = if console_should_be_verbose {
        log_level.unwrap_or(if debug { "debug" } else { "info" })
    } else {
        "warn"
    };

    let console_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(console_level))
        .map_err(|error| {
            PlatformError::Configuration(crate::platform::ConfigurationError::ValidationFailed {
                reason: format!("Invalid console log level: {error}"),
            })
        })?;

    use tracing_subscriber::Layer;
    let console_layer: Box<dyn Layer<tracing_subscriber::Registry> + Send + Sync> =
        if console_should_be_verbose {
            Box::new(
                fmt::layer()
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339())
                    .with_filter(console_filter),
            )
        } else {
            Box::new(
                fmt::layer()
                    .with_target(false)
                    .with_thread_ids(false)
                    .with_file(false)
                    .with_line_number(false)
                    .without_time()
                    .with_filter(console_filter),
            )
        };

    let resolved_log_file =
        log_file.unwrap_or_else(crate::config::AppConfig::get_platform_log_file_path);
    if let Some(parent) = resolved_log_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let file_layer = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&resolved_log_file)
    {
        Ok(file) => {
            let file_level = if debug { "debug" } else { "info" };
            let file_filter =
                EnvFilter::try_new(file_level).unwrap_or_else(|_| EnvFilter::new("info"));
            Some(
                fmt::layer()
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339())
                    .with_ansi(false)
                    .with_writer(std::sync::Mutex::new(file))
                    .with_filter(file_filter),
            )
        }
        Err(error) => {
            eprintln!(
                "Warning: Failed to open log file {}: {}",
                resolved_log_file.display(),
                error
            );
            None
        }
    };

    let subscriber = tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer);
    let _ = subscriber.try_init();

    info!(
        "Logging initialized. Console level: {}. File log: {}",
        console_level,
        resolved_log_file.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logging_initialization_accepts_a_valid_level() {
        assert!(init_logging_with_options(Some("debug"), None, true).is_ok());
    }
}

use clap::Parser;
use vuio_core::config::{AppConfig, MonitoredDirectoryConfig, ValidationMode};
use vuio_core::lifecycle::RuntimeOptions;

pub struct Command {
    pub runtime: RuntimeOptions,
    pub update: bool,
}

impl Command {
    pub fn parse_env() -> anyhow::Result<Self> {
        let args = Args::parse();
        let config_override = create_config_override(&args);
        Ok(Self {
            runtime: RuntimeOptions {
                debug: args.debug,
                config_path: args.config,
                log_file: args.log_file,
                log_level: args.log_level,
                config_override,
                restore_backup: args.restore_backup,
                auth: args.auth,
                ..RuntimeOptions::default()
            },
            update: args.update,
        })
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The directory containing media files to serve
    media_dir: Option<String>,
    /// Additional media directories to serve
    #[arg(short = 'm', long = "media-dir", action = clap::ArgAction::Append)]
    additional_media_dirs: Vec<String>,
    /// The network port to listen on
    #[arg(short, long)]
    port: Option<u16>,
    /// The friendly name for the DLNA server
    #[arg(short, long, default_value = "VuIO Server")]
    name: String,
    /// Enable debug logging
    #[arg(long)]
    debug: bool,
    /// Path to configuration file
    #[arg(short, long)]
    config: Option<String>,
    /// Path to log file
    #[arg(long = "log-file")]
    log_file: Option<String>,
    /// Set log level
    #[arg(long = "log-level")]
    log_level: Option<String>,
    /// Restore a validated Redb backup before opening the database
    #[arg(long = "restore-backup")]
    restore_backup: Option<String>,
    /// Update the binary to the latest version from GitHub
    #[arg(long)]
    update: bool,
    /// Enable management authentication
    #[arg(long)]
    auth: bool,
}

fn create_config_override(args: &Args) -> Option<AppConfig> {
    if args.media_dir.is_none() && args.additional_media_dirs.is_empty() {
        return None;
    }

    let mut config = AppConfig::default_for_platform();
    if let Some(port) = args.port {
        config.server.port = port;
    }
    if args.name != "VuIO Server" {
        config.server.name = args.name.clone();
    }
    config.media.directories = args
        .media_dir
        .iter()
        .chain(&args.additional_media_dirs)
        .map(|path| {
            let path = std::path::PathBuf::from(path);
            if !path.is_dir() {
                tracing::warn!("Media directory is not available: {}", path.display());
            }
            MonitoredDirectoryConfig {
                path: path.to_string_lossy().into_owned(),
                recursive: true,
                case_sensitive: None,
                extensions: None,
                exclude_patterns: None,
                validation_mode: ValidationMode::Warn,
            }
        })
        .collect();
    Some(config)
}

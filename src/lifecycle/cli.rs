/// Struct containing parsed command line arguments
pub struct LaunchOptions {
    pub debug: bool,
    pub config_path: Option<String>,
    pub log_file: Option<String>,
    pub log_level: Option<String>,
    pub config_override: Option<AppConfig>,
}

/// Parse command line arguments once and return configuration overrides
/// This consolidates argument parsing into a single operation
fn parse_args_once() -> anyhow::Result<LaunchOptions> {
    use clap::Parser;

    #[derive(Parser, Debug)]
    #[command(author, version, about, long_about = None)]
    struct Args {
        /// The directory containing media files to serve
        media_dir: Option<String>,

        /// Additional media directories to serve (can be used multiple times)
        #[arg(long = "media-dir", action = clap::ArgAction::Append)]
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

        /// Set log level (off, error, warn, info, debug, trace)
        #[arg(long = "log-level")]
        log_level: Option<String>,
    }

    let args = Args::parse();

    // If no media directories provided, return early args only
    if args.media_dir.is_none() && args.additional_media_dirs.is_empty() {
        return Ok(LaunchOptions {
            debug: args.debug,
            config_path: args.config,
            log_file: args.log_file,
            log_level: args.log_level,
            config_override: None,
        });
    }

    // Build configuration from command line arguments
    let mut config_override = AppConfig::default_for_platform();

    // Apply command line overrides
    if let Some(port) = args.port {
        config_override.server.port = port;
    }

    if args.name != "VuIO Server" {
        config_override.server.name = args.name;
    }

    // Build media directories from arguments
    let mut media_directories = vec![];

    // Add primary media directory if provided
    if let Some(media_dir_str) = &args.media_dir {
        let media_dir = std::path::PathBuf::from(media_dir_str);
        if !media_dir.exists() || !media_dir.is_dir() {
            tracing::warn!(
                "Media directory does not exist or is not a directory: {}",
                media_dir.display()
            );
        }
        media_directories.push(MonitoredDirectoryConfig {
            path: media_dir.to_string_lossy().to_string(),
            recursive: true,
            extensions: None,
            exclude_patterns: None,
            validation_mode: ValidationMode::Warn,
        });
    }

    // Add additional media directories
    for additional_dir_str in &args.additional_media_dirs {
        let additional_dir = std::path::PathBuf::from(additional_dir_str);
        if !additional_dir.exists() || !additional_dir.is_dir() {
            tracing::warn!(
                "Additional media directory does not exist or is not a directory: {}",
                additional_dir.display()
            );
        }
        media_directories.push(MonitoredDirectoryConfig {
            path: additional_dir.to_string_lossy().to_string(),
            recursive: true,
            extensions: None,
            exclude_patterns: None,
            validation_mode: ValidationMode::Warn,
        });
    }

    config_override.media.directories = media_directories;

    Ok(LaunchOptions {
        debug: args.debug,
        config_path: args.config,
        log_file: args.log_file,
        log_level: args.log_level,
        config_override: Some(config_override),
    })
}

/// Parses VuIO command-line arguments into reusable launch options.
pub struct CliService;

impl CliService {
    pub fn parse_env() -> anyhow::Result<LaunchOptions> {
        parse_args_once()
    }
}

use clap::Parser;
use vuio_core::RuntimeOptions;

pub struct Command {
    pub runtime: RuntimeOptions,
    pub update: bool,
}

impl Command {
    pub fn parse_env() -> anyhow::Result<Self> {
        let args = Args::parse();
        let mut runtime = RuntimeOptions::new()
            .debug(args.debug)
            .management_auth(args.auth);
        if let Some(config) = args.config {
            runtime = runtime.config_path(config);
        }
        if let Some(log_file) = args.log_file {
            runtime = runtime.log_file(log_file);
        }
        if let Some(log_level) = args.log_level {
            runtime = runtime.log_level(log_level);
        }
        if let Some(backup) = args.restore_backup {
            runtime = runtime.restore_backup(backup);
        }
        if let Some(port) = args.port {
            runtime = runtime.port(port);
        }
        // The default is only a placeholder; forwarding it would override the
        // name a user set in their config file.
        if args.name != DEFAULT_SERVER_NAME {
            runtime = runtime.server_name(args.name);
        }
        for directory in args.media_dir.iter().chain(&args.additional_media_dirs) {
            runtime = runtime.media_dir(directory);
        }

        Ok(Self {
            runtime,
            update: args.update,
        })
    }
}

const DEFAULT_SERVER_NAME: &str = "VuIO Server";

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
    #[arg(short, long, default_value = DEFAULT_SERVER_NAME)]
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

use clap::{Parser, Subcommand};
use vuio_core::RuntimeOptions;

pub enum Command {
    /// Run the media server, which is what the bare invocation does.
    Serve {
        runtime: RuntimeOptions,
        update: bool,
    },
    /// Speak MCP over stdio on behalf of a running server.
    Mcp(crate::mcp::Options),
}

impl Command {
    pub fn parse_env() -> anyhow::Result<Self> {
        let args = Args::parse();

        if let Some(Subcommands::Mcp {
            url,
            token,
            token_file,
        }) = args.command
        {
            return Ok(Self::Mcp(crate::mcp::Options {
                url,
                token,
                token_file,
            }));
        }

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

        Ok(Self::Serve {
            runtime,
            update: args.update,
        })
    }
}

const DEFAULT_SERVER_NAME: &str = "VuIO Server";

/// `vuio [MEDIA_DIR] [OPTIONS]` runs the server; `vuio mcp` is the one
/// subcommand.
///
/// `args_conflicts_with_subcommands` is what keeps the bare form working: the
/// positional media directory and a subcommand cannot both be present, so clap
/// stops trying to read `mcp` as a path.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(args_conflicts_with_subcommands = true)]
struct Args {
    #[command(subcommand)]
    command: Option<Subcommands>,
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
    /// Restore a validated database backup before opening the database
    #[arg(long = "restore-backup")]
    restore_backup: Option<String>,
    /// Update the binary to the latest version from GitHub
    #[arg(long)]
    update: bool,
    /// Enable management authentication
    #[arg(long)]
    auth: bool,
}

#[derive(Subcommand, Debug)]
enum Subcommands {
    /// Bridge an MCP client that speaks stdio to a running VuIO server.
    ///
    /// Reads JSON-RPC on stdin, forwards it to the server's /mcp endpoint, and
    /// writes the answers to stdout. It serves nothing itself — the server has
    /// the library open, and the database takes one writer.
    #[command(
        long_about = "Bridge an MCP client that speaks stdio to a running VuIO server.\n\n\
                      Use this for clients that will only launch a local process, such as \
                      Claude Desktop. Clients that can reach an HTTP endpoint should point \
                      at the server's /mcp directly instead.\n\n\
                      Example:\n  \
                      vuio mcp --url http://nas.local:8080 --token-file ~/.vuio/admin.token"
    )]
    Mcp {
        /// The VuIO server to talk to, with or without the /mcp path
        #[arg(long)]
        url: String,
        /// Management token, if the server requires one
        #[arg(long, conflicts_with = "token_file")]
        token: Option<String>,
        /// File holding the management token, usually admin.token beside the config
        #[arg(long = "token-file")]
        token_file: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_argument_parser_is_well_formed() {
        Args::command().debug_assert();
    }

    /// Adding a subcommand must not change what a bare invocation means: every
    /// existing install runs `vuio /media`, and clap will happily read a
    /// positional as a subcommand if told to.
    #[test]
    fn a_bare_invocation_still_runs_the_server() {
        let args = Args::try_parse_from(["vuio", "/media", "--port", "8081"]).unwrap();
        assert!(args.command.is_none());
        assert_eq!(args.media_dir.as_deref(), Some("/media"));
        assert_eq!(args.port, Some(8081));

        let args = Args::try_parse_from(["vuio"]).unwrap();
        assert!(args.command.is_none());
        assert!(args.media_dir.is_none());
    }

    #[test]
    fn the_mcp_subcommand_takes_a_url_and_a_credential() {
        let args = Args::try_parse_from([
            "vuio",
            "mcp",
            "--url",
            "http://nas.local:8080",
            "--token-file",
            "/etc/vuio/admin.token",
        ])
        .unwrap();
        let Some(Subcommands::Mcp {
            url,
            token,
            token_file,
        }) = args.command
        else {
            panic!("expected the mcp subcommand");
        };
        assert_eq!(url, "http://nas.local:8080");
        assert_eq!(token, None);
        assert_eq!(token_file.as_deref(), Some("/etc/vuio/admin.token"));

        // A URL is the one thing it cannot work without.
        assert!(Args::try_parse_from(["vuio", "mcp"]).is_err());
        // Two ways to give the same secret is a configuration error, not a
        // precedence puzzle.
        assert!(Args::try_parse_from([
            "vuio",
            "mcp",
            "--url",
            "http://x",
            "--token",
            "a",
            "--token-file",
            "b"
        ])
        .is_err());
    }
}

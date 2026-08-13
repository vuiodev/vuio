//! Provider API keys supplied to the process, rather than stored in it.
//!
//! Keys are never compiled into the binary and never written to the repository.
//! A provider that needs one reads it from the environment — the same route
//! `VUIO_ADMIN_TOKEN` already takes — with a `.env` file as a convenience,
//! because exporting a variable before every run is not one.
//!
//! The name is always `VUIO_<PROVIDER ID>_API_KEY`, whatever the provider calls
//! the credential itself. Generalised rather than special-cased so every keyed
//! provider is configured the same way, and so a container — where the admin API
//! refuses to write the config file at all — can still supply one.
//!
//! A key set from the dashboard wins over anything here; see
//! [`CredentialStore::get`](super::credentials::CredentialStore::get).

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::OnceLock,
};

/// Where `.env` was found, and what it held.
static DOTENV: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Extra directories to search, published by the runtime once it knows where its
/// configuration lives. Set before the first lookup; ignored afterwards.
static SEARCH_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Tell the loader where this server's configuration lives, so an installed
/// server reads `.env` from its config directory rather than from whatever
/// directory it happened to be started in.
///
/// Idempotent and lossy on purpose: the first caller wins, and a second call is
/// not an error, because tests and embedded hosts may start several runtimes in
/// one process.
pub fn set_config_dir(config_path: &Path) {
    if let Some(parent) = config_path.parent() {
        let _ = SEARCH_DIR.set(parent.to_path_buf());
    }
}

/// The environment variable a provider's key would arrive in.
pub fn env_var_name(provider_id: &str) -> String {
    format!("VUIO_{}_API_KEY", provider_id.to_ascii_uppercase())
}

/// A provider credential from the environment, or `None`.
///
/// A real environment variable beats the file, so a container or a systemd unit
/// can override a `.env` that happens to be lying beside the config.
pub fn env_credential(provider_id: &str) -> Option<String> {
    let name = env_var_name(provider_id);
    if let Ok(value) = std::env::var(&name) {
        let value = value.trim();
        if !value.is_empty() {
            return Some(value.to_owned());
        }
    }
    dotenv().get(&name).cloned()
}

fn dotenv() -> &'static HashMap<String, String> {
    DOTENV.get_or_init(|| {
        let mut candidates = Vec::new();
        if let Some(dir) = SEARCH_DIR.get() {
            candidates.push(dir.join(".env"));
        }
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join(".env"));
        }

        for path in candidates {
            // Not finding one is the normal case and says nothing.
            let Ok(contents) = std::fs::read_to_string(&path) else {
                continue;
            };
            let parsed = parse(&contents, &path.display().to_string());
            if !parsed.is_empty() {
                tracing::debug!(path = %path.display(), keys = parsed.len(), "Loaded .env");
            }
            return parsed;
        }
        HashMap::new()
    })
}

/// `KEY=value`, `#` comments, blank lines, and optional surrounding quotes.
///
/// Deliberately not written into `std::env`: that mutates process-global state
/// other threads may be reading at the same moment, and a map answers the only
/// question anyone asks of it just as well.
///
/// A line that makes no sense is dropped with a warning rather than failing the
/// load. A malformed `.env` should cost the operator one provider, not the
/// ability to start the server.
fn parse(contents: &str, source: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for (index, raw) in contents.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // `export FOO=bar` is what a shell-sourced file looks like, and copying
        // one in is a likely way to end up here.
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();

        let Some((key, value)) = line.split_once('=') else {
            tracing::warn!(%source, line = index + 1, "Ignoring a .env line with no '='");
            continue;
        };
        let key = key.trim();
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            tracing::warn!(%source, line = index + 1, "Ignoring a .env line with an unusable name");
            continue;
        }

        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .or_else(|| {
                value
                    .strip_prefix('\'')
                    .and_then(|rest| rest.strip_suffix('\''))
            })
            .unwrap_or(value);
        if value.is_empty() {
            // The template ships every name with an empty value. Treating that as
            // a credential would turn "not configured" into "configured with the
            // empty string", which providers answer with a 401.
            continue;
        }
        values.insert(key.to_owned(), value.to_owned());
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(contents: &str) -> HashMap<String, String> {
        parse(contents, "test")
    }

    #[test]
    fn the_variable_name_follows_the_provider_id() {
        assert_eq!(env_var_name("tmdb"), "VUIO_TMDB_API_KEY");
        assert_eq!(env_var_name("lastfm"), "VUIO_LASTFM_API_KEY");
    }

    #[test]
    fn comments_blank_lines_and_quotes_are_handled() {
        let values = parsed(
            "# a comment\n\
             \n\
             VUIO_TMDB_API_KEY=plain\n\
             VUIO_OMDB_API_KEY=\"quoted\"\n\
             VUIO_LASTFM_API_KEY='single'\n\
             export VUIO_GENIUS_API_KEY=exported\n\
             \tVUIO_DISCOGS_API_KEY = spaced ",
        );
        assert_eq!(values["VUIO_TMDB_API_KEY"], "plain");
        assert_eq!(values["VUIO_OMDB_API_KEY"], "quoted");
        assert_eq!(values["VUIO_LASTFM_API_KEY"], "single");
        assert_eq!(values["VUIO_GENIUS_API_KEY"], "exported");
        assert_eq!(values["VUIO_DISCOGS_API_KEY"], "spaced");
    }

    /// The shipped `.env.example` has every name with an empty value. Reading
    /// those as credentials would turn "not configured" into a guaranteed 401.
    #[test]
    fn an_empty_value_is_not_a_credential() {
        let values = parsed("VUIO_TMDB_API_KEY=\nVUIO_OMDB_API_KEY=\"\"");
        assert!(values.is_empty());
    }

    #[test]
    fn a_final_line_without_a_newline_is_still_read() {
        let values = parsed("VUIO_TMDB_API_KEY=last");
        assert_eq!(values["VUIO_TMDB_API_KEY"], "last");
    }

    /// One bad line must not cost the operator the rest of the file.
    #[test]
    fn a_malformed_line_is_skipped_and_the_rest_survives() {
        let values = parsed("this line has no equals\nVUIO_TMDB_API_KEY=fine\nbad key!=x");
        assert_eq!(values["VUIO_TMDB_API_KEY"], "fine");
        assert_eq!(values.len(), 1);
    }

    #[test]
    fn a_value_may_contain_an_equals_sign() {
        // Base64 and signed tokens routinely do.
        let values = parsed("VUIO_TMDB_API_KEY=abc==");
        assert_eq!(values["VUIO_TMDB_API_KEY"], "abc==");
    }
}

use crate::{config::ManagementConfig, database::DatabaseManager, state::AppState};
use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::{ConnectInfo, Json, Request, State},
    http::{header, HeaderMap, Method, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Response},
};
use ipnet::IpNet;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::Write,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

const MAX_SESSIONS: usize = 128;
const LOGIN_WINDOW: Duration = Duration::from_secs(60);
const MAX_LOGIN_ATTEMPTS: u8 = 5;
const MANAGEMENT_WINDOW: Duration = Duration::from_secs(60);
const MAX_MANAGEMENT_REQUESTS_PER_WINDOW: u16 = 120;
const MAX_MANAGEMENT_CONCURRENCY: usize = 32;

#[derive(Clone)]
struct Session {
    peer: IpAddr,
    expires_at: Instant,
    /// Which admin token this session was minted against. A session outlives a
    /// change of that token only if the two still agree, so rotating a leaked
    /// token also revokes the access it already bought.
    token_generation: u64,
}

/// The parts of `[management]` that can change while the server runs.
///
/// Held behind one lock rather than three so a reload swaps them together: a request
/// must never be checked against the new allowlist and the old token.
#[derive(Debug)]
struct ManagementSettings {
    admin_token: String,
    /// Incremented by [`AuthState::apply`] whenever `admin_token` changes.
    /// Read under the same lock as the token it describes, so a login that
    /// raced a rotation stamps its session with the generation it actually
    /// authenticated against rather than the one that replaced it.
    token_generation: u64,
    session_ttl: Duration,
    allowed_networks: Vec<IpNet>,
    token_path: PathBuf,
}

pub struct AuthState {
    /// `true` once anything has switched auth on. Whether that was the command line,
    /// the environment or the config file is remembered separately, because neither of
    /// the first two may be undone by a later config reload.
    enabled: std::sync::atomic::AtomicBool,
    /// Auth was demanded by `--auth` or `VUIO_AUTH`, so a config file saying
    /// `enabled = false` cannot switch it off.
    forced_on: bool,
    settings: std::sync::RwLock<ManagementSettings>,
    sessions: Mutex<HashMap<String, Session>>,
    login_attempts: Mutex<HashMap<IpAddr, (Instant, u8)>>,
    management_requests: Mutex<HashMap<IpAddr, (Instant, u16)>>,
    concurrency: Arc<tokio::sync::Semaphore>,
}

impl std::fmt::Debug for AuthState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthState")
            .field("enabled", &self.enabled())
            .field("settings", &self.settings_read())
            .finish_non_exhaustive()
    }
}

/// Where the admin token is read from: the configured file, else `admin.token` beside
/// the configuration.
fn resolve_token_path(config: &ManagementConfig, config_path: &Path) -> PathBuf {
    config
        .token_file
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            config_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("admin.token")
        })
}

/// Read the admin token, generating and persisting one if the file does not exist yet.
fn load_admin_token(token_path: &Path) -> Result<String> {
    if let Ok(token) = std::env::var("VUIO_ADMIN_TOKEN") {
        validate_token(token.trim())?;
        return Ok(token.trim().to_owned());
    }
    if token_path.exists() {
        verify_private_token(token_path)?;
        let token = std::fs::read_to_string(token_path)
            .with_context(|| format!("failed to read {}", token_path.display()))?;
        validate_token(token.trim())?;
        return Ok(token.trim().to_owned());
    }
    let token = random_token();
    if let Some(parent) = token_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_private_token(token_path, &token)?;
    tracing::warn!(
        "Generated management token at {}. Keep this file private.",
        token_path.display()
    );
    Ok(token)
}

fn parse_networks(config: &ManagementConfig) -> Result<Vec<IpNet>> {
    config
        .allowed_networks
        .iter()
        .map(|network| {
            network
                .parse::<IpNet>()
                .with_context(|| format!("invalid management network {network}"))
        })
        .collect()
}

/// Clamped at an hour: a zero TTL would expire every session the instant it was issued.
/// Validation rejects it too; this is the second line of defence.
fn session_ttl(config: &ManagementConfig) -> Duration {
    Duration::from_secs(config.session_ttl_hours.max(1).saturating_mul(3600))
}

impl AuthState {
    pub fn load(config: &ManagementConfig, config_path: &Path, cli_auth: bool) -> Result<Self> {
        let token_path = resolve_token_path(config, config_path);
        let admin_token = load_admin_token(&token_path)?;
        let allowed_networks = parse_networks(config)?;

        let env_auth = std::env::var("VUIO_AUTH")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        // `management.enabled` was written to every generated config and read by
        // nothing. Any of the three turns auth on; the host's two cannot be undone
        // by a later config reload, which is what `forced_on` records.
        let forced_on = cli_auth || env_auth;

        Ok(Self {
            enabled: std::sync::atomic::AtomicBool::new(forced_on || config.enabled),
            forced_on,
            settings: std::sync::RwLock::new(ManagementSettings {
                admin_token,
                token_generation: 0,
                session_ttl: session_ttl(config),
                allowed_networks,
                token_path,
            }),
            sessions: Mutex::new(HashMap::new()),
            login_attempts: Mutex::new(HashMap::new()),
            management_requests: Mutex::new(HashMap::new()),
            concurrency: Arc::new(tokio::sync::Semaphore::new(MAX_MANAGEMENT_CONCURRENCY)),
        })
    }

    /// Re-apply `[management]` to a running server.
    ///
    /// Everything here used to be frozen at startup. The values are cheap per-request
    /// reads with no derived state behind them, so the only real work is re-reading the
    /// token file.
    ///
    /// A failure leaves the previous settings in place: half-applying an allowlist while
    /// keeping the old token would be worse than not applying it at all.
    pub fn apply(&self, config: &ManagementConfig, config_path: &Path) -> Result<()> {
        let token_path = resolve_token_path(config, config_path);
        let allowed_networks = parse_networks(config)?;
        // Always re-read, including when the path has not changed. Skipping that read
        // meant the one rotation an operator actually has — writing a new token into
        // `admin.token` and reloading — did nothing at all: the old token kept working,
        // the new one was refused, and the sessions the old one had bought stayed valid,
        // all without a word to say so. A leaked credential was live until a restart.
        //
        // The property that shortcut existed for is kept below instead: a file that has
        // become unreadable falls back to the token already in use rather than locking
        // out a server that is running fine. That is only defensible when the file is
        // the same one that token came from; a new path that cannot be read is a
        // configuration error and fails the reload.
        let admin_token = match load_admin_token(&token_path) {
            Ok(token) => token,
            Err(error) if token_path == self.settings_read().token_path => {
                tracing::warn!(
                    "Keeping the management token already in use: {error:#}"
                );
                self.settings_read().admin_token.clone()
            }
            Err(error) => return Err(error),
        };

        let mut settings = self
            .settings
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let token_rotated = settings.admin_token != admin_token;
        settings.admin_token = admin_token;
        if token_rotated {
            settings.token_generation = settings.token_generation.wrapping_add(1);
        }
        settings.session_ttl = session_ttl(config);
        settings.allowed_networks = allowed_networks;
        settings.token_path = token_path;
        drop(settings);

        if token_rotated {
            // Cookies issued under the old token are already refused by the
            // generation check in `session_from_headers`; this only stops the
            // dead entries counting towards `MAX_SESSIONS`. A login that read
            // the old token before the write lock above may still insert after
            // this clear, which is why the check, not the clear, is what
            // enforces the revocation.
            self.sessions
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clear();
            tracing::warn!("Management token changed; existing sessions revoked");
        }

        // `forced_on` wins: a config file must not be able to switch off auth that
        // --auth or VUIO_AUTH asked for.
        let enabled = self.forced_on || config.enabled;
        let previous = self
            .enabled
            .swap(enabled, std::sync::atomic::Ordering::Relaxed);
        if previous != enabled {
            tracing::warn!(
                "Management authentication is now {}",
                if enabled { "required" } else { "not required" }
            );
        }
        Ok(())
    }

    fn settings_read(&self) -> std::sync::RwLockReadGuard<'_, ManagementSettings> {
        self.settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
    }

    pub fn testing() -> Self {
        Self {
            enabled: std::sync::atomic::AtomicBool::new(true),
            forced_on: true,
            settings: std::sync::RwLock::new(ManagementSettings {
                admin_token: "test-management-token-which-is-long-enough".to_owned(),
                token_generation: 0,
                session_ttl: Duration::from_secs(3600),
                allowed_networks: Vec::new(),
                token_path: PathBuf::from("admin.token"),
            }),
            sessions: Mutex::new(HashMap::new()),
            login_attempts: Mutex::new(HashMap::new()),
            management_requests: Mutex::new(HashMap::new()),
            concurrency: Arc::new(tokio::sync::Semaphore::new(MAX_MANAGEMENT_CONCURRENCY)),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn token_path(&self) -> PathBuf {
        self.settings_read().token_path.clone()
    }

    /// Whether `address` may reach the management surface at all.
    ///
    /// The allowlist is a restriction an operator wrote down, and it used to be
    /// consulted only on the path a token guards — so on a server left in the default
    /// open mode, where it is the only access control there is, setting it did nothing
    /// whatsoever. Every management endpoint stayed reachable from every address the
    /// listener answers on: the config writer, the restart button, the radio controls,
    /// the credential store. The dashboard offers the field beside the token switch and
    /// says nothing about depending on it.
    ///
    /// An empty list keeps exactly the meaning it had. With a token required that is
    /// loopback plus the private ranges, which is [`Self::network_allowed`]'s default;
    /// without one it is no restriction at all, because narrowing an open server to the
    /// private ranges would be a new refusal nobody asked for.
    fn management_peer_allowed(&self, address: IpAddr) -> bool {
        if !self.enabled() && self.settings_read().allowed_networks.is_empty() {
            return true;
        }
        self.network_allowed(address)
    }

    fn network_allowed(&self, address: IpAddr) -> bool {
        let settings = self.settings_read();
        if settings.allowed_networks.is_empty() {
            match address {
                IpAddr::V4(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
                IpAddr::V6(ip) => {
                    ip.is_loopback()
                        || (ip.segments()[0] & 0xfe00) == 0xfc00 // Unique Local Address fc00::/7
                        || (ip.segments()[0] & 0xffc0) == 0xfe80 // Link-Local Address fe80::/10
                }
            }
        } else {
            address.is_loopback()
                || settings
                    .allowed_networks
                    .iter()
                    .any(|network| network.contains(&address))
        }
    }

    /// Constant-time comparison against the current admin token, yielding the
    /// generation of the token that matched so a caller minting a session
    /// records the credential it really checked.
    fn token_matches(&self, candidate: &str) -> Option<u64> {
        let settings = self.settings_read();
        constant_time_eq(candidate.as_bytes(), settings.admin_token.as_bytes())
            .then_some(settings.token_generation)
    }

    /// Whether the request carries the admin token as a bearer credential.
    ///
    /// Visible beyond this module because `[mcp].require_auth` gates one
    /// endpoint on a token even when management auth as a whole is off, and it
    /// must ask the same question this middleware does.
    pub(crate) fn bearer_valid(&self, headers: &HeaderMap) -> bool {
        headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .is_some_and(|token| self.token_matches(token).is_some())
    }

    fn session_from_headers(&self, headers: &HeaderMap, peer: IpAddr) -> Option<String> {
        let token = cookie_value(headers, "vuio_session")?;
        let now = Instant::now();
        let generation = self.settings_read().token_generation;
        let mut sessions = self
            .sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        sessions.retain(|_, session| session.expires_at > now);
        sessions
            .get(&token)
            .filter(|session| {
                session.peer == peer
                    && session.expires_at > now
                    && session.token_generation == generation
            })
            .map(|_| token)
    }

    fn origin_valid(headers: &HeaderMap) -> bool {
        let Some(origin) = headers
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok())
        else {
            return false;
        };
        let Some(host) = headers
            .get(header::HOST)
            .and_then(|value| value.to_str().ok())
        else {
            return false;
        };
        origin == format!("http://{host}") || origin == format!("https://{host}")
    }

    fn rate_limit_login(&self, peer: IpAddr) -> bool {
        let now = Instant::now();
        let mut attempts = self
            .login_attempts
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = attempts.entry(peer).or_insert((now, 0));
        if now.duration_since(entry.0) >= LOGIN_WINDOW {
            *entry = (now, 0);
        }
        if entry.1 >= MAX_LOGIN_ATTEMPTS {
            return false;
        }
        entry.1 += 1;
        true
    }

    /// `token_generation` is the one [`AuthState::token_matches`] reported for
    /// the credential this login presented, not the one current at insert time
    /// — a rotation in between must leave the new session dead on arrival.
    fn create_session(&self, peer: IpAddr, token_generation: u64) -> Option<String> {
        let now = Instant::now();
        let session_ttl = self.settings_read().session_ttl;
        let mut sessions = self
            .sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        sessions.retain(|_, session| session.expires_at > now);
        if sessions.len() >= MAX_SESSIONS {
            return None;
        }
        let token = random_token();
        sessions.insert(
            token.clone(),
            Session {
                peer,
                expires_at: now + session_ttl,
                token_generation,
            },
        );
        Some(token)
    }

    fn rate_limit_management(&self, peer: IpAddr) -> bool {
        let now = Instant::now();
        let mut requests = self
            .management_requests
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        requests.retain(|_, (started, _)| now.duration_since(*started) < MANAGEMENT_WINDOW);
        let entry = requests.entry(peer).or_insert((now, 0));
        if now.duration_since(entry.0) >= MANAGEMENT_WINDOW {
            *entry = (now, 0);
        }
        if entry.1 >= MAX_MANAGEMENT_REQUESTS_PER_WINDOW {
            return false;
        }
        entry.1 += 1;
        true
    }

    fn remove_session(&self, headers: &HeaderMap) {
        if let Some(token) = cookie_value(headers, "vuio_session") {
            self.sessions
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .remove(&token);
        }
    }
}

fn random_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn validate_token(token: &str) -> Result<()> {
    anyhow::ensure!(
        token.len() >= 32,
        "management token must contain at least 32 bytes"
    );
    Ok(())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let length = left.len().max(right.len());
    for index in 0..length {
        difference |= left.get(index).copied().unwrap_or_default() as usize
            ^ right.get(index).copied().unwrap_or_default() as usize;
    }
    difference == 0
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|cookie| cookie.strip_prefix(&format!("{name}=")).map(str::to_owned))
}

#[cfg(unix)]
fn write_private_token(path: &Path, token: &str) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    writeln!(file, "{token}")?;
    file.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn verify_private_token(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path)?.permissions().mode() & 0o777;
    anyhow::ensure!(
        mode & 0o077 == 0,
        "management token {} must not be accessible by group or other users",
        path.display()
    );
    Ok(())
}

#[cfg(not(unix))]
fn verify_private_token(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(not(unix))]
fn write_private_token(path: &Path, token: &str) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    writeln!(file, "{token}")?;
    file.sync_all()?;
    Ok(())
}

#[derive(Deserialize)]
pub struct LoginRequest {
    token: String,
    save_device: Option<bool>,
}

pub async fn login_page<D: DatabaseManager>(State(state): State<AppState<D>>) -> Response {
    if !state.auth.enabled() {
        return axum::response::Redirect::to("/").into_response();
    }
    Html(
        r#"<!doctype html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Sign In - VuIO</title>
    <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@400;500;600;700&display=swap" rel="stylesheet">
    <style>
        :root {
            --bg-color: #0b0f19;
            --card-bg: #111827;
            --card-border: rgba(255, 255, 255, 0.05);
            --text-primary: #f3f4f6;
            --text-secondary: #9ca3af;
            --accent-color: #00f0ff;
            --accent-glow: rgba(0, 240, 255, 0.4);
            --error-color: #ef4444;
        }
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        body {
            font-family: 'Outfit', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            background: var(--bg-color);
            color: var(--text-primary);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            padding: 1.5rem;
            position: relative;
            overflow: hidden;
        }
        body::before {
            content: '';
            position: absolute;
            width: 400px;
            height: 400px;
            background: radial-gradient(circle, var(--accent-glow) 0%, rgba(0,0,0,0) 70%);
            top: -100px;
            right: -100px;
            z-index: 0;
            pointer-events: none;
            opacity: 0.5;
        }
        body::after {
            content: '';
            position: absolute;
            width: 500px;
            height: 500px;
            background: radial-gradient(circle, rgba(99, 102, 241, 0.15) 0%, rgba(0,0,0,0) 70%);
            bottom: -150px;
            left: -150px;
            z-index: 0;
            pointer-events: none;
            opacity: 0.5;
        }
        .login-card {
            background: var(--card-bg);
            border: 1px solid var(--card-border);
            border-radius: 20px;
            width: 100%;
            max-width: 400px;
            padding: 2.5rem;
            box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
            backdrop-filter: blur(10px);
            z-index: 10;
            display: flex;
            flex-direction: column;
            gap: 1.75rem;
            animation: fadeIn 0.6s cubic-bezier(0.16, 1, 0.3, 1);
        }
        @keyframes fadeIn {
            from { opacity: 0; transform: translateY(15px); }
            to { opacity: 1; transform: translateY(0); }
        }
        .header {
            display: flex;
            flex-direction: column;
            align-items: center;
            gap: 0.5rem;
            text-align: center;
        }
        .logo {
            width: 48px;
            height: 48px;
            border-radius: 12px;
            background: linear-gradient(135deg, var(--accent-color), #6366f1);
            display: flex;
            align-items: center;
            justify-content: center;
            box-shadow: 0 0 20px var(--accent-glow);
            margin-bottom: 0.5rem;
            color: #fff;
        }
        h2 {
            font-size: 1.6rem;
            font-weight: 700;
            letter-spacing: -0.025em;
        }
        .subtitle {
            font-size: 0.88rem;
            color: var(--text-secondary);
        }
        .input-group {
            display: flex;
            flex-direction: column;
            gap: 0.5rem;
        }
        .input-group label {
            font-size: 0.8rem;
            font-weight: 600;
            color: var(--text-secondary);
            text-transform: uppercase;
            letter-spacing: 0.05em;
        }
        .input-wrapper {
            position: relative;
        }
        input[type="password"] {
            width: 100%;
            background: rgba(255, 255, 255, 0.02);
            border: 1px solid var(--card-border);
            border-radius: 10px;
            padding: 0.85rem 1rem;
            color: #fff;
            font-family: inherit;
            font-size: 0.95rem;
            outline: none;
            transition: all 0.2s ease;
        }
        input[type="password"]:focus {
            border-color: var(--accent-color);
            background: rgba(255, 255, 255, 0.04);
            box-shadow: 0 0 10px rgba(0, 240, 255, 0.15);
        }
        .checkbox-group {
            display: flex;
            align-items: center;
            gap: 0.65rem;
            cursor: pointer;
            user-select: none;
        }
        .checkbox-group input[type="checkbox"] {
            accent-color: var(--accent-color);
            width: 16px;
            height: 16px;
            cursor: pointer;
        }
        .checkbox-group span {
            font-size: 0.88rem;
            color: var(--text-secondary);
            transition: color 0.2s;
        }
        .checkbox-group:hover span {
            color: var(--text-primary);
        }
        button {
            width: 100%;
            background: linear-gradient(135deg, var(--accent-color), #6366f1);
            border: none;
            border-radius: 10px;
            color: #fff;
            padding: 0.9rem;
            font-size: 0.95rem;
            font-weight: 600;
            cursor: pointer;
            box-shadow: 0 4px 15px rgba(99, 102, 241, 0.25);
            transition: all 0.2s ease;
            outline: none;
        }
        button:hover {
            box-shadow: 0 4px 20px var(--accent-glow);
            transform: translateY(-1px);
        }
        button:active {
            transform: translateY(0);
        }
        .error-message {
            font-size: 0.85rem;
            color: var(--error-color);
            background: rgba(239, 68, 68, 0.08);
            border: 1px solid rgba(239, 68, 68, 0.15);
            padding: 0.75rem;
            border-radius: 8px;
            display: none;
            align-items: center;
            gap: 0.5rem;
            text-align: left;
            line-height: 1.4;
        }
    </style>
</head>
<body>
    <div class="login-card">
        <div class="header">
            <div class="logo">
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>
            </div>
            <h2>Sign In</h2>
            <p class="subtitle">Management token is required</p>
        </div>
        <form id="login" style="display: flex; flex-direction: column; gap: 1.25rem;">
            <div class="input-group">
                <label for="token">Token</label>
                <div class="input-wrapper">
                    <input id="token" type="password" autocomplete="current-password" placeholder="Enter admin token" required autofocus>
                </div>
            </div>
            <label class="checkbox-group">
                <input id="save-device" type="checkbox">
                <span>Save this device</span>
            </label>
            <div id="error" class="error-message"></div>
            <button type="submit">Sign In</button>
        </form>
    </div>
    <script>
        document.getElementById('login').onsubmit = async (e) => {
            e.preventDefault();
            const token = document.getElementById('token').value;
            const saveDevice = document.getElementById('save-device').checked;
            const errEl = document.getElementById('error');
            
            errEl.style.display = 'none';

            try {
                const r = await fetch('/login', {
                    method: 'POST',
                    headers: { 'content-type': 'application/json' },
                    body: JSON.stringify({ token, save_device: saveDevice })
                });
                if (r.ok) {
                    location = '/';
                } else {
                    errEl.style.display = 'block';
                    errEl.textContent = 'Invalid administration token';
                }
            } catch (err) {
                errEl.style.display = 'block';
                errEl.textContent = 'Connection failed';
            }
        };
    </script>
</body>
</html>"#,
    ).into_response()
}

pub async fn login<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(request): Json<LoginRequest>,
) -> Response {
    if !state.auth.enabled() || !state.auth.network_allowed(peer.ip()) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if !state.auth.rate_limit_login(peer.ip()) {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    let Some(token_generation) = state.auth.token_matches(&request.token) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(session) = state.auth.create_session(peer.ip(), token_generation) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    let cookie_header = if request.save_device.unwrap_or(false) {
        format!(
            "vuio_session={session}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}",
            30 * 24 * 3600
        )
    } else {
        format!("vuio_session={session}; HttpOnly; SameSite=Strict; Path=/")
    };

    (
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, cookie_header)],
    )
        .into_response()
}

pub async fn logout<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    headers: HeaderMap,
) -> Response {
    state.auth.remove_session(&headers);
    (
        StatusCode::NO_CONTENT,
        [(
            header::SET_COOKIE,
            "vuio_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
        )],
    )
        .into_response()
}

pub async fn require_management<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Before the `enabled` check, not after it: the allowlist applies to a server that
    // requires no token just as much as to one that does. See
    // [`AuthState::management_peer_allowed`].
    if !state.auth.management_peer_allowed(peer.ip()) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if !state.auth.enabled() {
        return next.run(request).await;
    }
    if !state.auth.rate_limit_management(peer.ip()) {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    let Ok(_permit) = state.auth.concurrency.clone().try_acquire_owned() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let bearer = state.auth.bearer_valid(request.headers());
    let cookie = state
        .auth
        .session_from_headers(request.headers(), peer.ip())
        .is_some();
    if !bearer && !cookie {
        // A subresource must never be answered with the login page: a <script> or
        // <link> would parse 200 bytes of HTML as JavaScript or CSS. Only navigations
        // get redirected; everything a page fetches for itself gets a plain 401.
        //
        // `/_app` is where the browser app's bundles live, and it is here for the
        // same reason as `/assets`: every one of them is loaded by a <script> or a
        // dynamic import, so a 200 login page would be parsed as JavaScript.
        let path = request.uri().path();
        if request.method() == Method::GET
            && (path == "/"
                || path == "/logs"
                || (!path.starts_with("/api")
                    && !path.starts_with("/mcp")
                    && !path.starts_with("/assets")
                    && !path.starts_with("/_app")))
        {
            return axum::response::Redirect::to("/login").into_response();
        }
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if cookie
        && !bearer
        && !matches!(
            *request.method(),
            Method::GET | Method::HEAD | Method::OPTIONS
        )
        && !AuthState::origin_valid(request.headers())
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn load_with(enabled: bool, cli_auth: bool) -> AuthState {
        let temp_dir = TempDir::new().expect("temp dir");
        let config = ManagementConfig {
            enabled,
            ..ManagementConfig::default()
        };
        AuthState::load(&config, &temp_dir.path().join("config.toml"), cli_auth)
            .expect("auth state should load")
    }

    /// `management.enabled` sat in every generated config and was read by nothing:
    /// only --auth and VUIO_AUTH could turn auth on. Setting it in the file now works,
    /// which is what makes the setting meaningful from the admin UI.
    #[test]
    fn config_can_enable_management_auth() {
        assert!(!load_with(false, false).enabled());
        assert!(load_with(true, false).enabled());
    }

    /// The command line still wins on its own, and a config that says `false` must not
    /// be able to switch off auth the host explicitly asked for.
    #[test]
    fn the_command_line_still_enables_auth_alone() {
        assert!(load_with(false, true).enabled());
        assert!(load_with(true, true).enabled());
    }

    /// The allowlist is one plain read per management request, so it can be swapped
    /// without a restart. Sessions carry an absolute expiry, so changing the TTL applies
    /// to new logins and leaves signed-in browsers alone.
    #[test]
    fn management_settings_apply_without_a_restart() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config_path = temp_dir.path().join("config.toml");
        let auth = AuthState::load(&ManagementConfig::default(), &config_path, true)
            .expect("auth state");

        // TEST-NET-3, deliberately not private: an empty allowlist already permits
        // loopback and every private range, so a 10.x address would prove nothing.
        let peer: IpAddr = "203.0.113.5".parse().unwrap();
        assert!(!auth.network_allowed(peer), "a public address is not allowed by default");

        auth.apply(
            &ManagementConfig {
                allowed_networks: vec!["203.0.113.0/24".to_string()],
                session_ttl_hours: 48,
                ..ManagementConfig::default()
            },
            &config_path,
        )
        .expect("apply");
        assert!(auth.network_allowed(peer), "the new allowlist applies immediately");
        assert_eq!(
            auth.settings_read().session_ttl,
            Duration::from_secs(48 * 3600)
        );

        // And narrowing it again takes effect just as immediately.
        auth.apply(&ManagementConfig::default(), &config_path)
            .expect("apply");
        assert!(!auth.network_allowed(peer));
    }

    /// A rejected reload must leave the previous settings whole. Applying the allowlist
    /// while keeping the old token would be worse than applying nothing.
    #[test]
    fn a_rejected_reload_changes_nothing() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config_path = temp_dir.path().join("config.toml");
        let auth = AuthState::load(&ManagementConfig::default(), &config_path, true)
            .expect("auth state");
        let token_before = auth.settings_read().admin_token.clone();

        let error = auth
            .apply(
                &ManagementConfig {
                    allowed_networks: vec!["10.0.0.0/64".to_string()],
                    ..ManagementConfig::default()
                },
                &config_path,
            )
            .expect_err("an invalid CIDR must be refused");
        assert!(error.to_string().contains("invalid management network"));
        assert_eq!(auth.settings_read().admin_token, token_before);
        assert!(auth.settings_read().allowed_networks.is_empty());
    }

    /// The command line and the environment outrank the file. A config that says
    /// `enabled = false` must not be able to switch off auth the host demanded.
    #[test]
    fn a_config_reload_cannot_switch_off_forced_auth() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config_path = temp_dir.path().join("config.toml");

        let forced = AuthState::load(&ManagementConfig::default(), &config_path, true)
            .expect("auth state");
        assert!(forced.enabled());
        forced
            .apply(&ManagementConfig { enabled: false, ..ManagementConfig::default() }, &config_path)
            .expect("apply");
        assert!(forced.enabled(), "--auth must survive a config reload");

        // Without the flag, the file is in charge in both directions.
        let from_file = AuthState::load(&ManagementConfig::default(), &config_path, false)
            .expect("auth state");
        assert!(!from_file.enabled());
        from_file
            .apply(&ManagementConfig { enabled: true, ..ManagementConfig::default() }, &config_path)
            .expect("apply");
        assert!(from_file.enabled());
        from_file
            .apply(&ManagementConfig::default(), &config_path)
            .expect("apply");
        assert!(!from_file.enabled());
    }

    /// Both defaults have to stay off. Every config generated before this change says
    /// `enabled = true`, and the Docker env default said true as well; leaving either
    /// on would have started demanding a token from installs that never asked for one.
    #[test]
    fn management_auth_defaults_off() {
        assert!(!ManagementConfig::default().enabled);
        assert!(!crate::config::AppConfig::default_for_platform().management.enabled);
    }
    /// Rotating a leaked management token has to revoke the access it already
    /// bought: a cookie minted under the old token must stop working the moment
    /// the new one is loaded, while a login against the new token still works.
    #[test]
    fn rotating_the_token_revokes_existing_sessions() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config_path = temp_dir.path().join("config.toml");

        let token_a = temp_dir.path().join("a.token");
        let token_b = temp_dir.path().join("b.token");
        write_private_token(&token_a, &"a".repeat(40)).expect("write a");
        write_private_token(&token_b, &"b".repeat(40)).expect("write b");

        let config_with = |path: &Path| ManagementConfig {
            enabled: true,
            token_file: Some(path.display().to_string()),
            ..ManagementConfig::default()
        };

        let auth =
            AuthState::load(&config_with(&token_a), &config_path, true).expect("auth state");
        let peer: IpAddr = "127.0.0.1".parse().unwrap();

        let generation = auth.token_matches(&"a".repeat(40)).expect("token a matches");
        let session = auth.create_session(peer, generation).expect("session");
        let cookie = |session: &str| {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::COOKIE,
                format!("vuio_session={session}").parse().unwrap(),
            );
            headers
        };
        assert!(
            auth.session_from_headers(&cookie(&session), peer).is_some(),
            "the session works under the token it was issued with"
        );

        auth.apply(&config_with(&token_b), &config_path)
            .expect("rotate to token b");

        assert!(
            auth.session_from_headers(&cookie(&session), peer).is_none(),
            "a cookie issued under the old token must not survive rotation"
        );
        assert!(auth.token_matches(&"a".repeat(40)).is_none());

        let generation = auth.token_matches(&"b".repeat(40)).expect("token b matches");
        let session = auth.create_session(peer, generation).expect("session");
        assert!(
            auth.session_from_headers(&cookie(&session), peer).is_some(),
            "the new token still logs in"
        );
    }

    /// Writing a new token into the file the server is already using is the only
    /// rotation most operators will ever perform. It used to do nothing until a
    /// restart, because `apply` re-read the file only when its *path* changed: the
    /// leaked token kept working and the new one was refused.
    #[test]
    fn rewriting_the_token_file_in_place_rotates_the_token() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config_path = temp_dir.path().join("config.toml");
        let token_path = temp_dir.path().join("admin.token");
        write_private_token(&token_path, &"a".repeat(40)).expect("write a");

        let config = ManagementConfig {
            enabled: true,
            token_file: Some(token_path.display().to_string()),
            ..ManagementConfig::default()
        };
        let auth = AuthState::load(&config, &config_path, true).expect("auth state");
        let peer: IpAddr = "127.0.0.1".parse().unwrap();

        let generation = auth.token_matches(&"a".repeat(40)).expect("token a matches");
        let session = auth.create_session(peer, generation).expect("session");
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            format!("vuio_session={session}").parse().unwrap(),
        );
        assert!(auth.session_from_headers(&headers, peer).is_some());

        // The same path, rewritten in place, as `printf ... > admin.token` leaves it.
        std::fs::remove_file(&token_path).expect("remove");
        write_private_token(&token_path, &"b".repeat(40)).expect("write b");
        auth.apply(&config, &config_path).expect("reload");

        assert!(
            auth.token_matches(&"a".repeat(40)).is_none(),
            "the replaced token must stop being accepted"
        );
        assert!(
            auth.token_matches(&"b".repeat(40)).is_some(),
            "the token now in the file must be accepted"
        );
        assert!(
            auth.session_from_headers(&headers, peer).is_none(),
            "a cookie bought with the replaced token must not survive it"
        );
    }

    /// The fallback that shortcut existed for, kept: a token file that has become
    /// unreadable leaves the running server with the token it already had rather than
    /// failing the reload and locking everyone out.
    #[test]
    fn an_unreadable_token_file_keeps_the_token_in_use() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config_path = temp_dir.path().join("config.toml");
        let token_path = temp_dir.path().join("admin.token");
        write_private_token(&token_path, &"a".repeat(40)).expect("write a");

        let config = ManagementConfig {
            enabled: true,
            token_file: Some(token_path.display().to_string()),
            ..ManagementConfig::default()
        };
        let auth = AuthState::load(&config, &config_path, true).expect("auth state");

        // Too short to load, which is the failure `load_admin_token` can be made to
        // produce on every platform; a mode the owner cannot read is not one of them.
        std::fs::write(&token_path, "short").expect("truncate");
        auth.apply(&config, &config_path)
            .expect("an unreadable token file must not fail the reload");
        assert!(
            auth.token_matches(&"a".repeat(40)).is_some(),
            "the token already in use stays in use"
        );
    }

    /// `allowed_networks` used to be read only when a token was required, so an
    /// operator who restricted the dashboard to one subnet and left it open got no
    /// restriction at all — on exactly the server where the list is the only control
    /// there is.
    #[test]
    fn the_allowlist_applies_without_a_token() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config_path = temp_dir.path().join("config.toml");
        let auth = AuthState::load(
            &ManagementConfig {
                enabled: false,
                allowed_networks: vec!["192.168.10.0/24".to_owned()],
                ..ManagementConfig::default()
            },
            &config_path,
            false,
        )
        .expect("auth state");

        assert!(!auth.enabled(), "this server requires no token");
        assert!(auth.management_peer_allowed("192.168.10.5".parse().unwrap()));
        assert!(auth.management_peer_allowed("127.0.0.1".parse().unwrap()));
        assert!(
            !auth.management_peer_allowed("192.168.20.5".parse().unwrap()),
            "an address outside the configured list is refused"
        );
        assert!(!auth.management_peer_allowed("203.0.113.5".parse().unwrap()));
    }

    /// And an empty list still means what it did: an open server stays open, rather
    /// than acquiring a private-ranges-only rule nobody asked for.
    #[test]
    fn no_allowlist_leaves_an_open_server_open() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config_path = temp_dir.path().join("config.toml");
        let auth = AuthState::load(&ManagementConfig::default(), &config_path, false)
            .expect("auth state");

        assert!(!auth.enabled());
        assert!(auth.management_peer_allowed("203.0.113.5".parse().unwrap()));

        // With a token required, the same empty list is loopback and the private
        // ranges, which is the behaviour `network_allowed` has always had.
        let guarded = AuthState::load(&ManagementConfig::default(), &config_path, true)
            .expect("auth state");
        assert!(guarded.enabled());
        assert!(!guarded.management_peer_allowed("203.0.113.5".parse().unwrap()));
        assert!(guarded.management_peer_allowed("10.0.0.5".parse().unwrap()));
    }

    /// A login that validated the old token but inserted its session after the
    /// rotation had already swept the map must not end up authenticated — the
    /// generation it carries, not the sweep, is what revokes it.
    #[test]
    fn a_login_racing_a_rotation_does_not_retain_authority() {
        let temp_dir = TempDir::new().expect("temp dir");
        let config_path = temp_dir.path().join("config.toml");
        let token_a = temp_dir.path().join("a.token");
        let token_b = temp_dir.path().join("b.token");
        write_private_token(&token_a, &"a".repeat(40)).expect("write a");
        write_private_token(&token_b, &"b".repeat(40)).expect("write b");

        let config_with = |path: &Path| ManagementConfig {
            enabled: true,
            token_file: Some(path.display().to_string()),
            ..ManagementConfig::default()
        };
        let auth =
            AuthState::load(&config_with(&token_a), &config_path, true).expect("auth state");
        let peer: IpAddr = "127.0.0.1".parse().unwrap();

        // The order a racing login would produce: check the token, rotate, then
        // insert the session.
        let generation = auth.token_matches(&"a".repeat(40)).expect("token a matches");
        auth.apply(&config_with(&token_b), &config_path)
            .expect("rotate to token b");
        let session = auth.create_session(peer, generation).expect("session");

        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            format!("vuio_session={session}").parse().unwrap(),
        );
        assert!(
            auth.session_from_headers(&headers, peer).is_none(),
            "a session minted against the superseded token carries no authority"
        );
    }
}

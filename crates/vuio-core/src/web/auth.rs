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
}

/// The parts of `[management]` that can change while the server runs.
///
/// Held behind one lock rather than three so a reload swaps them together: a request
/// must never be checked against the new allowlist and the old token.
#[derive(Debug)]
struct ManagementSettings {
    admin_token: String,
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
    /// token file when its path changes.
    ///
    /// A failure leaves the previous settings in place: half-applying an allowlist while
    /// keeping the old token would be worse than not applying it at all.
    pub fn apply(&self, config: &ManagementConfig, config_path: &Path) -> Result<()> {
        let token_path = resolve_token_path(config, config_path);
        let allowed_networks = parse_networks(config)?;
        let admin_token = if token_path == self.settings_read().token_path {
            // Same file: keep the token in memory rather than re-reading, so a file that
            // has become unreadable cannot lock out a server that is running fine.
            self.settings_read().admin_token.clone()
        } else {
            load_admin_token(&token_path)?
        };

        let mut settings = self
            .settings
            .write()
            .unwrap_or_else(|error| error.into_inner());
        settings.admin_token = admin_token;
        settings.session_ttl = session_ttl(config);
        settings.allowed_networks = allowed_networks;
        settings.token_path = token_path;
        drop(settings);

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

    /// Constant-time comparison against the current admin token.
    fn token_matches(&self, candidate: &str) -> bool {
        constant_time_eq(
            candidate.as_bytes(),
            self.settings_read().admin_token.as_bytes(),
        )
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
            .is_some_and(|token| self.token_matches(token))
    }

    fn session_from_headers(&self, headers: &HeaderMap, peer: IpAddr) -> Option<String> {
        let token = cookie_value(headers, "vuio_session")?;
        let now = Instant::now();
        let mut sessions = self
            .sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        sessions.retain(|_, session| session.expires_at > now);
        sessions
            .get(&token)
            .filter(|session| session.peer == peer && session.expires_at > now)
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

    fn create_session(&self, peer: IpAddr) -> Option<String> {
        let now = Instant::now();
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
                expires_at: now + self.settings_read().session_ttl,
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
    if !state.auth.token_matches(&request.token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Some(session) = state.auth.create_session(peer.ip()) else {
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
    if !state.auth.enabled() {
        return next.run(request).await;
    }
    if !state.auth.network_allowed(peer.ip()) {
        return StatusCode::FORBIDDEN.into_response();
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
}

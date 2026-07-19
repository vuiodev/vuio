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

pub struct AuthState {
    enabled: bool,
    auth_enabled: bool,
    admin_token: String,
    sessions: Mutex<HashMap<String, Session>>,
    login_attempts: Mutex<HashMap<IpAddr, (Instant, u8)>>,
    management_requests: Mutex<HashMap<IpAddr, (Instant, u16)>>,
    concurrency: Arc<tokio::sync::Semaphore>,
    session_ttl: Duration,
    allowed_networks: Vec<IpNet>,
}

impl std::fmt::Debug for AuthState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthState")
            .field("enabled", &self.enabled)
            .field("session_ttl", &self.session_ttl)
            .field("allowed_networks", &self.allowed_networks)
            .finish_non_exhaustive()
    }
}

impl AuthState {
    pub fn load(config: &ManagementConfig, config_path: &Path) -> Result<Self> {
        let token_path = config
            .token_file
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                config_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join("admin.token")
            });
        let admin_token = if let Ok(token) = std::env::var("VUIO_ADMIN_TOKEN") {
            validate_token(token.trim())?;
            token.trim().to_owned()
        } else if token_path.exists() {
            verify_private_token(&token_path)?;
            let token = std::fs::read_to_string(&token_path)
                .with_context(|| format!("failed to read {}", token_path.display()))?;
            validate_token(token.trim())?;
            token.trim().to_owned()
        } else {
            let token = random_token();
            if let Some(parent) = token_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            write_private_token(&token_path, &token)?;
            tracing::warn!(
                "Generated management token at {}. Keep this file private.",
                token_path.display()
            );
            token
        };
        let allowed_networks = config
            .allowed_networks
            .iter()
            .map(|network| {
                network
                    .parse::<IpNet>()
                    .with_context(|| format!("invalid management network {network}"))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            enabled: config.enabled,
            auth_enabled: config.auth_enabled,
            admin_token,
            sessions: Mutex::new(HashMap::new()),
            login_attempts: Mutex::new(HashMap::new()),
            management_requests: Mutex::new(HashMap::new()),
            concurrency: Arc::new(tokio::sync::Semaphore::new(MAX_MANAGEMENT_CONCURRENCY)),
            session_ttl: Duration::from_secs(config.session_ttl_hours.max(1).saturating_mul(3600)),
            allowed_networks,
        })
    }

    pub fn testing() -> Self {
        Self {
            enabled: true,
            auth_enabled: true,
            admin_token: "test-management-token-which-is-long-enough".to_owned(),
            sessions: Mutex::new(HashMap::new()),
            login_attempts: Mutex::new(HashMap::new()),
            management_requests: Mutex::new(HashMap::new()),
            concurrency: Arc::new(tokio::sync::Semaphore::new(MAX_MANAGEMENT_CONCURRENCY)),
            session_ttl: Duration::from_secs(3600),
            allowed_networks: Vec::new(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn auth_enabled(&self) -> bool {
        self.auth_enabled
    }

    fn network_allowed(&self, address: IpAddr) -> bool {
        address.is_loopback()
            || self
                .allowed_networks
                .iter()
                .any(|network| network.contains(&address))
    }

    fn bearer_valid(&self, headers: &HeaderMap) -> bool {
        headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .is_some_and(|token| constant_time_eq(token.as_bytes(), self.admin_token.as_bytes()))
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
                expires_at: now + self.session_ttl,
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
}

pub async fn login_page() -> Html<&'static str> {
    Html(
        r#"<!doctype html><meta charset="utf-8"><title>VuIO login</title>
<form id="login"><input id="token" type="password" autocomplete="current-password" placeholder="Admin token"><button>Sign in</button></form>
<p id="error"></p><script>document.getElementById('login').onsubmit=async(e)=>{e.preventDefault();const r=await fetch('/login',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({token:document.getElementById('token').value})});if(r.ok)location='/';else document.getElementById('error').textContent='Login failed';};</script>"#,
    )
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
    if !constant_time_eq(request.token.as_bytes(), state.auth.admin_token.as_bytes()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Some(session) = state.auth.create_session(peer.ip()) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    (
        StatusCode::NO_CONTENT,
        [(
            header::SET_COOKIE,
            format!(
                "vuio_session={session}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}",
                state.auth.session_ttl.as_secs()
            ),
        )],
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
        return StatusCode::NOT_FOUND.into_response();
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
    if !state.auth.auth_enabled() {
        return next.run(request).await;
    }
    let bearer = state.auth.bearer_valid(request.headers());
    let cookie = state
        .auth
        .session_from_headers(request.headers(), peer.ip())
        .is_some();
    if !bearer && !cookie {
        if request.uri().path() == "/" && request.method() == Method::GET {
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

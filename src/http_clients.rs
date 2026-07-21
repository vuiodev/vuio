use anyhow::{Context, Result};
use reqwest::Client;
use std::sync::OnceLock;
use std::time::Duration;

static LOCAL: OnceLock<Client> = OnceLock::new();
static EVENT: OnceLock<Client> = OnceLock::new();
static UPDATER: OnceLock<Client> = OnceLock::new();

fn shared(slot: &'static OnceLock<Client>, builder: reqwest::ClientBuilder) -> Result<Client> {
    if let Some(client) = slot.get() {
        return Ok(client.clone());
    }
    let client = builder
        .build()
        .context("failed to build shared HTTP client")?;
    let _ = slot.set(client.clone());
    Ok(slot.get().cloned().unwrap_or(client))
}

/// Local-device client. Redirects are disabled so a renderer cannot pivot a
/// validated request to an unvalidated destination.
pub fn local() -> Result<Client> {
    shared(
        &LOCAL,
        Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none()),
    )
}

pub fn event() -> Result<Client> {
    shared(
        &EVENT,
        Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::none()),
    )
}

pub fn updater() -> Result<Client> {
    shared(
        &UPDATER,
        Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .https_only(true)
            .user_agent(format!("vuio-updater/{}", env!("CARGO_PKG_VERSION"))),
    )
}

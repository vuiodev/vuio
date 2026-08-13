//! The dashboard's MediaInfo endpoints.
//!
//! Four verbs: report what is configured and how a run is going, save or clear a
//! provider credential, start a run, and stop one. The run itself is a background
//! task — the providers' rate limits put a library well past any sensible request
//! timeout — so starting it returns immediately and the dashboard polls for the
//! rest.

use crate::{
    database::DatabaseManager,
    mediainfo::{
        provider::PROVIDERS, provider_info, run_library_fetch, CredentialStore, MEDIAINFO_VERSION,
    },
    state::AppState,
};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

/// How many uncertain matches the dashboard is given to show. The list is for
/// spotting a pattern, not for auditing the whole library.
const LOW_CONFIDENCE_LIMIT: usize = 50;

#[derive(Serialize)]
struct ProviderView {
    id: &'static str,
    label: &'static str,
    group: &'static str,
    provides: &'static str,
    /// The label of the credential it wants, when it wants one.
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_label: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signup_url: Option<&'static str>,
    needs_credential: bool,
    /// Whether a credential is on file. Never the credential itself.
    has_credential: bool,
    /// Whether this provider is in the configured list.
    enabled: bool,
}

#[derive(Serialize)]
struct JobView {
    running: bool,
    total: usize,
    processed: usize,
    matched: usize,
    low_confidence: usize,
    failed: usize,
    cancelled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    current: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finished_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
}

#[derive(Serialize)]
struct FlaggedView {
    media_file_id: i64,
    confidence: u8,
    provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    matched_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

fn epoch_seconds(time: Option<SystemTime>) -> Option<u64> {
    time.and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|elapsed| elapsed.as_secs())
}

/// `GET /api/admin/mediainfo`
pub async fn get_status<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
) -> impl IntoResponse {
    let config = state.current_config();
    let settings = &config.mediainfo;
    let threshold = settings.min_confidence.min(100);

    let credentials =
        match CredentialStore::load(state.database.clone() as std::sync::Arc<dyn crate::database::SecretStore>)
            .await
        {
            Ok(store) => store,
            Err(error) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": error.to_string() })),
                )
                    .into_response()
            }
        };
    let stored = credentials.stored_providers().await;

    let providers: Vec<ProviderView> = PROVIDERS
        .iter()
        .map(|provider| ProviderView {
            id: provider.id,
            label: provider.label,
            group: provider.kind.group(),
            provides: provider.provides,
            credential_label: provider.credential.map(|credential| credential.label),
            signup_url: provider.credential.map(|credential| credential.signup_url),
            needs_credential: provider.needs_credential(),
            has_credential: stored.iter().any(|id| id == provider.id),
            enabled: settings.providers.iter().any(|id| id == provider.id),
        })
        .collect();

    let job = {
        let job = state.mediainfo_job.lock().await;
        JobView {
            running: job.running,
            total: job.total,
            processed: job.processed,
            matched: job.matched,
            low_confidence: job.low_confidence,
            failed: job.failed,
            cancelled: job.cancelled,
            current: job.current.clone(),
            started_at: epoch_seconds(job.started_at),
            finished_at: epoch_seconds(job.finished_at),
            last_error: job.last_error.clone(),
        }
    };

    let stats = state
        .database
        .mediainfo_stats(threshold)
        .await
        .unwrap_or_default();

    let flagged = match state
        .database
        .list_low_confidence(threshold, LOW_CONFIDENCE_LIMIT)
        .await
    {
        Ok(records) => {
            let mut views = Vec::with_capacity(records.len());
            for record in records {
                // The filename is what makes a flagged row identifiable; without it
                // the operator is looking at a list of database ids.
                let filename = state
                    .database
                    .get_file_by_id(record.media_file_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|file| file.filename);
                views.push(FlaggedView {
                    media_file_id: record.media_file_id,
                    confidence: record.confidence,
                    provider: record.provider,
                    matched_title: record.title,
                    filename,
                });
            }
            views
        }
        Err(error) => {
            tracing::warn!(%error, "Could not list low-confidence media info");
            Vec::new()
        }
    };

    Json(json!({
        "enabled": settings.enabled,
        "min_confidence": threshold,
        "artwork_enabled": settings.artwork_enabled,
        "version": MEDIAINFO_VERSION,
        "providers": providers,
        "job": job,
        "stats": stats,
        "flagged": flagged,
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct CredentialRequest {
    provider: String,
    /// An empty or absent token clears the stored one — the dashboard's Clear
    /// button posts the same shape as Save.
    #[serde(default)]
    token: String,
}

/// `POST /api/admin/mediainfo/credentials`
pub async fn put_credential<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    Json(request): Json<CredentialRequest>,
) -> impl IntoResponse {
    let Some(provider) = provider_info(&request.provider) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Unknown provider: {}", request.provider) })),
        )
            .into_response();
    };
    if !provider.needs_credential() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("{} does not use a credential", provider.label) })),
        )
            .into_response();
    }

    let credentials =
        match CredentialStore::load(state.database.clone() as std::sync::Arc<dyn crate::database::SecretStore>)
            .await
        {
            Ok(store) => store,
            Err(error) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": error.to_string() })),
                )
                    .into_response()
            }
        };

    if let Err(error) = credentials.set(provider.id, &request.token).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response();
    }

    // Reports whether one is now stored, never what it is.
    Json(json!({ "saved": true, "has_credential": !request.token.trim().is_empty() }))
        .into_response()
}

/// `POST /api/admin/mediainfo/run`
pub async fn run<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
) -> impl IntoResponse {
    match run_library_fetch(state).await {
        Ok(total) => Json(json!({ "started": true, "total": total })).into_response(),
        // "Already running" and "turned off" are both the caller asking for
        // something the current state does not allow, which is a conflict rather
        // than a server fault.
        Err(error) => (
            StatusCode::CONFLICT,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

/// `POST /api/admin/mediainfo/cancel`
pub async fn cancel<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
) -> impl IntoResponse {
    let job = state.mediainfo_job.lock().await;
    match job.cancel.as_ref() {
        Some(token) => {
            token.cancel();
            Json(json!({ "cancelled": true })).into_response()
        }
        None => (
            StatusCode::CONFLICT,
            Json(json!({ "error": "No media info fetch is running" })),
        )
            .into_response(),
    }
}

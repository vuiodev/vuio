//! Web metrics, health/readiness probes, and log diagnostics.

use crate::{database::DatabaseManager, state::AppState};
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
};
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::error;

async fn collect_runtime_diagnostics<D: DatabaseManager>(
    state: &AppState<D>,
) -> (
    Option<crate::platform::diagnostics::RuntimeDiagnostics>,
    usize,
    usize,
) {
    let directories = state.media_directories.read().await.clone();
    let checks = directories
        .iter()
        .map(|directory| async move { tokio::fs::read_dir(&directory.path).await.is_ok() });
    let accessible_directories = futures_util::future::join_all(checks)
        .await
        .into_iter()
        .filter(|accessible| *accessible)
        .count();
    let diagnostics = match state.runtime_diagnostics.snapshot().await {
        Ok(snapshot) => Some(snapshot),
        Err(error) => {
            tracing::warn!(%error, "Failed to collect runtime diagnostics");
            None
        }
    };
    (diagnostics, directories.len(), accessible_directories)
}

/// Atomic performance tracking for web handlers
pub struct WebHandlerMetrics {
    pub browse_requests: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub directory_listings: AtomicU64,
    pub file_serves: AtomicU64,
    pub errors: AtomicU64,
    pub total_response_time_us: AtomicU64,
    pub bytes_transferred: AtomicU64,
}

impl WebHandlerMetrics {
    pub fn new() -> Self {
        Self {
            browse_requests: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            directory_listings: AtomicU64::new(0),
            file_serves: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            total_response_time_us: AtomicU64::new(0),
            bytes_transferred: AtomicU64::new(0),
        }
    }

    pub fn record_browse_request(&self, response_time_us: u64, cache_hit: bool) {
        self.browse_requests.fetch_add(1, Ordering::Relaxed);
        self.total_response_time_us
            .fetch_add(response_time_us, Ordering::Relaxed);
        if cache_hit {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.cache_misses.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_directory_listing(&self, response_time_us: u64) {
        self.directory_listings.fetch_add(1, Ordering::Relaxed);
        self.total_response_time_us
            .fetch_add(response_time_us, Ordering::Relaxed);
    }

    pub fn record_file_serve(&self, response_time_us: u64, is_actual_serve: bool) {
        if is_actual_serve {
            self.file_serves.fetch_add(1, Ordering::Relaxed);
        }
        self.total_response_time_us
            .fetch_add(response_time_us, Ordering::Relaxed);
    }

    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_stats(&self) -> WebHandlerStats {
        let browse_requests = self.browse_requests.load(Ordering::Relaxed);
        let total_time_us = self.total_response_time_us.load(Ordering::Relaxed);

        WebHandlerStats {
            browse_requests,
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            directory_listings: self.directory_listings.load(Ordering::Relaxed),
            file_serves: self.file_serves.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            average_response_time_ms: if browse_requests > 0 {
                (total_time_us as f64 / browse_requests as f64) / 1000.0
            } else {
                0.0
            },
            cache_hit_rate: if browse_requests > 0 {
                (self.cache_hits.load(Ordering::Relaxed) as f64 / browse_requests as f64) * 100.0
            } else {
                0.0
            },
            gigabytes_transferred: self.bytes_transferred.load(Ordering::Relaxed) as f64
                / 1_073_741_824.0,
        }
    }
}

impl Default for WebHandlerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct WebHandlerStats {
    pub browse_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub directory_listings: u64,
    pub file_serves: u64,
    pub errors: u64,
    pub average_response_time_ms: f64,
    pub cache_hit_rate: f64,
    pub gigabytes_transferred: f64,
}

/// Get web handler performance metrics for monitoring
pub async fn get_web_metrics<D: DatabaseManager>(
    State(state): State<AppState<D>>,
) -> impl IntoResponse {
    let stats = state.web_metrics.get_stats();

    let (database_result, root_availability, runtime) = tokio::join!(
        state.database.get_stats(),
        state.database.list_root_availability(),
        collect_runtime_diagnostics(&state)
    );
    let db_stats = match database_result {
        Ok(s) => s,
        Err(_) => crate::database::DatabaseStats {
            total_files: 0,
            total_size: 0,
            database_size: 0,
            video_files: 0,
            audio_files: 0,
            image_files: 0,
            playlists: 0,
        },
    };

    let active_casts = {
        let mut casts = state.active_casts.lock().await;
        casts.snapshot()
    };
    let (runtime_diagnostics, monitored_directory_count, accessible_directory_count) = runtime;
    let unavailable_roots = root_availability
        .unwrap_or_default()
        .into_iter()
        .filter(|root| root.unavailable_since_secs.is_some())
        .collect::<Vec<_>>();

    let metrics_json = serde_json::json!({
        "web_handler_metrics": {
            "browse_requests": stats.browse_requests,
            "cache_hits": stats.cache_hits,
            "cache_misses": stats.cache_misses,
            "cache_hit_rate_percent": stats.cache_hit_rate,
            "directory_listings": stats.directory_listings,
            "file_serves": stats.file_serves,
            "errors": stats.errors,
            "average_response_time_ms": stats.average_response_time_ms,
            "gigabytes_transferred": stats.gigabytes_transferred,
            "redb_database": "active"
        },
        "database_stats": {
            "total_files": db_stats.total_files,
            "total_size_bytes": db_stats.total_size,
            "database_size_bytes": db_stats.database_size,
            "video_files": db_stats.video_files,
            "audio_files": db_stats.audio_files,
            "image_files": db_stats.image_files,
            "playlists": db_stats.playlists,
        },
        "runtime_diagnostics": {
            "snapshot": runtime_diagnostics,
            "monitored_directory_count": monitored_directory_count,
            "accessible_directory_count": accessible_directory_count,
            "watch_for_changes": state.current_config().media.watch_for_changes,
            "scan_on_startup": state.current_config().media.scan_on_startup,
            "platform": state.platform_info.os_type.display_name(),
            "architecture": std::env::consts::ARCH,
            "unavailable_or_incomplete_roots": unavailable_roots,
        },
        "configuration": {
            "desired": {
                "server_port": state.desired_config.load().server.port,
                "server_interface": state.desired_config.load().server.interface,
                "watch_for_changes": state.desired_config.load().media.watch_for_changes,
                "backup_enabled": state.desired_config.load().database.backup_enabled,
                "chromecast_enabled": state.desired_config.load().cast.chromecast_enabled,
                "airplay_enabled": state.desired_config.load().cast.airplay_enabled,
            },
            "effective": {
                "server_port": state.current_config().server.port,
                "server_interface": state.current_config().server.interface,
                "watch_for_changes": state.current_config().media.watch_for_changes,
                "backup_enabled": state.current_config().database.backup_enabled,
                "chromecast_enabled": state.current_config().cast.chromecast_enabled,
                "airplay_enabled": state.current_config().cast.airplay_enabled,
            },
            "restart_required": !state.pending_restart_fields.read().unwrap_or_else(|e| e.into_inner()).is_empty(),
            "pending_restart_fields": state.pending_restart_fields.read().unwrap_or_else(|e| e.into_inner()).clone(),
            "reload_errors": state.config_reload_errors.read().unwrap_or_else(|e| e.into_inner()).clone(),
        },
        "active_casts": active_casts
    });

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        metrics_json.to_string(),
    )
}

/// Helper to read the last N lines of the log file using a ring buffer
async fn read_last_log_lines(
    path: &std::path::Path,
    limit: usize,
) -> Result<String, std::io::Error> {
    use tokio::fs::File;
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    const MAX_TAIL_BYTES: u64 = 1024 * 1024;
    let mut file = File::open(path).await?;
    let length = file.metadata().await?.len();
    let start = length.saturating_sub(MAX_TAIL_BYTES);
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let mut bytes = Vec::with_capacity((length - start) as usize);
    file.read_to_end(&mut bytes).await?;
    let text = String::from_utf8_lossy(&bytes);
    let lines = text.lines().rev().take(limit).collect::<Vec<_>>();
    let mut result = lines.into_iter().rev().collect::<Vec<_>>().join("\n");
    if !result.is_empty() {
        result.push('\n');
    }
    Ok(result)
}

/// Liveness probe to check if the server is running
pub async fn healthz_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        r#"{"status":"healthy"}"#,
    )
}

/// Readiness probe to check if the database is accessible
pub async fn readyz_handler<D: DatabaseManager>(
    State(state): State<AppState<D>>,
) -> impl IntoResponse {
    match state.database.get_stats().await {
        Ok(_) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            r#"{"status":"ready"}"#.to_string(),
        ),
        Err(e) => {
            error!("Readiness check failed: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                [(header::CONTENT_TYPE, "application/json")],
                r#"{"status":"unhealthy"}"#.to_string(),
            )
        }
    }
}

/// Serve metrics in standard Prometheus Exposition Format (plain text)
pub async fn get_prometheus_metrics<D: DatabaseManager>(
    State(state): State<AppState<D>>,
) -> impl IntoResponse {
    let stats = state.web_metrics.get_stats();

    let runtime = collect_runtime_diagnostics(&state).await;
    let (db_files, db_total_size, db_size, db_video, db_audio, db_image, db_playlists) =
        match state.database.get_stats().await {
            Ok(s) => (
                s.total_files,
                s.total_size,
                s.database_size,
                s.video_files,
                s.audio_files,
                s.image_files,
                s.playlists,
            ),
            Err(_) => (0, 0, 0, 0, 0, 0, 0),
        };

    let mut body = String::new();

    body.push_str("# HELP vuio_web_browse_requests_total Total number of media browse requests\n");
    body.push_str("# TYPE vuio_web_browse_requests_total counter\n");
    body.push_str(&format!(
        "vuio_web_browse_requests_total {}\n\n",
        stats.browse_requests
    ));

    body.push_str("# HELP vuio_web_cache_hits_total Total number of browse cache hits\n");
    body.push_str("# TYPE vuio_web_cache_hits_total counter\n");
    body.push_str(&format!(
        "vuio_web_cache_hits_total {}\n\n",
        stats.cache_hits
    ));

    body.push_str("# HELP vuio_web_cache_misses_total Total number of browse cache misses\n");
    body.push_str("# TYPE vuio_web_cache_misses_total counter\n");
    body.push_str(&format!(
        "vuio_web_cache_misses_total {}\n\n",
        stats.cache_misses
    ));

    body.push_str(
        "# HELP vuio_web_directory_listings_total Total number of directory listing requests\n",
    );
    body.push_str("# TYPE vuio_web_directory_listings_total counter\n");
    body.push_str(&format!(
        "vuio_web_directory_listings_total {}\n\n",
        stats.directory_listings
    ));

    body.push_str("# HELP vuio_web_file_serves_total Total number of files served\n");
    body.push_str("# TYPE vuio_web_file_serves_total counter\n");
    body.push_str(&format!(
        "vuio_web_file_serves_total {}\n\n",
        stats.file_serves
    ));

    body.push_str(
        "# HELP vuio_web_gigabytes_transferred_total Total gigabytes of media transferred\n",
    );
    body.push_str("# TYPE vuio_web_gigabytes_transferred_total counter\n");
    body.push_str(&format!(
        "vuio_web_gigabytes_transferred_total {}\n\n",
        stats.gigabytes_transferred
    ));

    body.push_str("# HELP vuio_web_errors_total Total number of web handler errors\n");
    body.push_str("# TYPE vuio_web_errors_total counter\n");
    body.push_str(&format!("vuio_web_errors_total {}\n\n", stats.errors));

    body.push_str(
        "# HELP vuio_web_average_response_time_ms Average response time in milliseconds\n",
    );
    body.push_str("# TYPE vuio_web_average_response_time_ms gauge\n");
    body.push_str(&format!(
        "vuio_web_average_response_time_ms {}\n\n",
        stats.average_response_time_ms
    ));

    body.push_str("# HELP vuio_database_files Total media files indexed in database\n");
    body.push_str("# TYPE vuio_database_files gauge\n");
    body.push_str(&format!("vuio_database_files {}\n\n", db_files));

    body.push_str(
        "# HELP vuio_database_total_size_bytes Cumulative size of all media files in bytes\n",
    );
    body.push_str("# TYPE vuio_database_total_size_bytes gauge\n");
    body.push_str(&format!(
        "vuio_database_total_size_bytes {}\n\n",
        db_total_size
    ));

    body.push_str("# HELP vuio_database_size_bytes Size of the database file on disk in bytes\n");
    body.push_str("# TYPE vuio_database_size_bytes gauge\n");
    body.push_str(&format!("vuio_database_size_bytes {}\n\n", db_size));

    body.push_str("# HELP vuio_database_video_files Total video files indexed in database\n");
    body.push_str("# TYPE vuio_database_video_files gauge\n");
    body.push_str(&format!("vuio_database_video_files {}\n\n", db_video));

    body.push_str("# HELP vuio_database_audio_files Total audio files indexed in database\n");
    body.push_str("# TYPE vuio_database_audio_files gauge\n");
    body.push_str(&format!("vuio_database_audio_files {}\n\n", db_audio));

    body.push_str(
        "# HELP vuio_database_image_files Total image/picture files indexed in database\n",
    );
    body.push_str("# TYPE vuio_database_image_files gauge\n");
    body.push_str(&format!("vuio_database_image_files {}\n\n", db_image));

    body.push_str("# HELP vuio_database_playlists Total playlists imported in database\n");
    body.push_str("# TYPE vuio_database_playlists gauge\n");
    body.push_str(&format!("vuio_database_playlists {}\n", db_playlists));

    let (runtime_diagnostics, monitored_directory_count, accessible_directory_count) = runtime;
    body.push_str("\n# HELP vuio_monitored_directories Configured monitored directories\n");
    body.push_str("# TYPE vuio_monitored_directories gauge\n");
    body.push_str(&format!(
        "vuio_monitored_directories {monitored_directory_count}\n"
    ));
    body.push_str("# HELP vuio_accessible_directories Accessible monitored directories\n");
    body.push_str("# TYPE vuio_accessible_directories gauge\n");
    body.push_str(&format!(
        "vuio_accessible_directories {accessible_directory_count}\n"
    ));

    if let Some(runtime) = runtime_diagnostics {
        body.push_str("# HELP vuio_system_uptime_seconds System uptime in seconds\n");
        body.push_str("# TYPE vuio_system_uptime_seconds gauge\n");
        body.push_str(&format!(
            "vuio_system_uptime_seconds {}\n",
            runtime.system.uptime_seconds
        ));
        body.push_str("# HELP vuio_system_memory_bytes Total system memory in bytes\n");
        body.push_str("# TYPE vuio_system_memory_bytes gauge\n");
        body.push_str(&format!(
            "vuio_system_memory_bytes {}\n",
            runtime.system.total_memory_bytes
        ));
        body.push_str(
            "# HELP vuio_system_available_memory_bytes Available system memory in bytes\n",
        );
        body.push_str("# TYPE vuio_system_available_memory_bytes gauge\n");
        body.push_str(&format!(
            "vuio_system_available_memory_bytes {}\n",
            runtime.system.available_memory_bytes
        ));
        body.push_str("# HELP vuio_process_memory_bytes VuIO resident memory in bytes\n");
        body.push_str("# TYPE vuio_process_memory_bytes gauge\n");
        if let Some(memory) = runtime.process.memory_bytes {
            body.push_str(&format!("vuio_process_memory_bytes {memory}\n"));
        }
        body.push_str("# HELP vuio_process_cpu_usage_percent VuIO process CPU usage percentage\n");
        body.push_str("# TYPE vuio_process_cpu_usage_percent gauge\n");
        if let Some(cpu) = runtime.process.cpu_usage_percent {
            body.push_str(&format!("vuio_process_cpu_usage_percent {cpu}\n"));
        }
        body.push_str("# HELP vuio_disk_total_bytes Aggregate filesystem capacity in bytes\n");
        body.push_str("# TYPE vuio_disk_total_bytes gauge\n");
        body.push_str(&format!(
            "vuio_disk_total_bytes {}\n",
            runtime.disks.total_bytes
        ));
        body.push_str(
            "# HELP vuio_disk_available_bytes Aggregate available filesystem capacity in bytes\n",
        );
        body.push_str("# TYPE vuio_disk_available_bytes gauge\n");
        body.push_str(&format!(
            "vuio_disk_available_bytes {}\n",
            runtime.disks.available_bytes
        ));
        body.push_str(
            "# HELP vuio_network_received_bytes Total bytes received by all interfaces\n",
        );
        body.push_str("# TYPE vuio_network_received_bytes counter\n");
        body.push_str(&format!(
            "vuio_network_received_bytes {}\n",
            runtime.network.total_received_bytes
        ));
        body.push_str(
            "# HELP vuio_network_transmitted_bytes Total bytes transmitted by all interfaces\n",
        );
        body.push_str("# TYPE vuio_network_transmitted_bytes counter\n");
        body.push_str(&format!(
            "vuio_network_transmitted_bytes {}\n",
            runtime.network.total_transmitted_bytes
        ));
    }

    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

#[derive(serde::Deserialize)]
pub struct LogsQuery {
    pub limit: Option<usize>,
}

/// Serve log file contents for Loki / Grafana scraping or debugging
pub async fn get_logs_handler<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    axum::extract::Query(query): axum::extract::Query<LogsQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).min(5000); // Caps limit at 5000 lines to prevent memory issues

    match read_last_log_lines(&state.log_file_path, limit).await {
        Ok(content) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            content,
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "No log entries recorded yet.".to_string(),
        ),
        Err(e) => {
            error!("Failed to read log file: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "Internal Server Error".to_string(),
            )
        }
    }
}

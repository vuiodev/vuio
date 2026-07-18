//! Browser dashboard handler and compile-time UI template rendering.

use crate::web::format::format_bytes;
use crate::{
    database::{DatabaseManager, DatabaseReadSession, MediaFileQuery, MediaFileView},
    error::AppError,
    state::AppState,
};
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::io::Write as _;

const DASHBOARD_TEMPLATE: &str = include_str!("ui/dashboard.html");

pub async fn root_handler() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        DASHBOARD_TEMPLATE,
    )
}

#[derive(serde::Serialize)]
pub struct ServerInfo {
    server_name: String,
    monitored_directories: Vec<String>,
}

pub async fn server_info_handler<D: DatabaseManager>(
    State(state): State<AppState<D>>,
) -> Json<ServerInfo> {
    let monitored_directories = state
        .media_directories
        .read()
        .await
        .iter()
        .map(|directory| directory.path.clone())
        .collect();
    Json(ServerInfo {
        server_name: state.current_config().server.name.clone(),
        monitored_directories,
    })
}

#[derive(serde::Deserialize)]
pub struct MediaPageQuery {
    cursor: Option<String>,
    limit: Option<usize>,
    category: Option<String>,
    query: Option<String>,
}

pub async fn media_page_handler<D: DatabaseManager + 'static>(
    State(state): State<AppState<D>>,
    Query(params): Query<MediaPageQuery>,
) -> Result<Response, AppError> {
    let after_id = params
        .cursor
        .as_deref()
        .map(str::parse::<i64>)
        .transpose()
        .map_err(|_| AppError::InvalidInput("Invalid media cursor".to_string()))?;
    let limit = params.limit.unwrap_or(250).clamp(1, 500);
    let mime_family = match params.category.as_deref().unwrap_or("all") {
        "all" | "" => None,
        "audio" => Some("audio/".to_string()),
        "video" => Some("video/".to_string()),
        "image" => Some("image/".to_string()),
        "radio" => Some("audio/radio".to_string()),
        _ => return Err(AppError::InvalidInput("Unknown media category".to_string())),
    };
    let text = params.query.filter(|value| !value.is_empty());
    let query = MediaFileQuery::Filtered {
        after_id,
        mime_family,
        text,
    };
    let fetch_limit = limit + 1;
    let response = state
        .database
        .clone()
        .read(move |session| {
            let mut output = Vec::with_capacity(limit.saturating_mul(320));
            output.extend_from_slice(b"{\"files\":[");
            let mut emitted = 0_usize;
            let mut last_id = None;
            let summary = session.visit_files(&query, 0, fetch_limit, |file| {
                if emitted >= limit {
                    return Ok(());
                }
                if emitted > 0 {
                    output.push(b',');
                }
                write_web_media_file(&mut output, &file)?;
                last_id = file.id();
                emitted += 1;
                Ok(())
            })?;
            output.extend_from_slice(b"],\"next_cursor\":");
            if summary.visited > limit {
                serde_json::to_writer(&mut output, &last_id.map(|id| id.to_string()))?;
            } else {
                output.extend_from_slice(b"null");
            }
            output.push(b'}');
            Ok(Bytes::from(output))
        })
        .await
        .map_err(AppError::Internal)?;
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        response,
    )
        .into_response())
}

fn write_web_media_file(output: &mut Vec<u8>, file: &impl MediaFileView) -> anyhow::Result<()> {
    let mime_type = file.mime_type();
    let category = if mime_type == "audio/radio" {
        "radio"
    } else {
        mime_type.split('/').next().unwrap_or("file")
    };
    let extension = std::path::Path::new(file.path())
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_lowercase();
    write!(
        output,
        "{{\"id\":{},\"path\":",
        file.id().unwrap_or_default()
    )?;
    serde_json::to_writer(&mut *output, file.path())?;
    output.extend_from_slice(b",\"name\":");
    serde_json::to_writer(&mut *output, file.filename())?;
    output.extend_from_slice(b",\"title\":");
    serde_json::to_writer(&mut *output, &file.title())?;
    output.extend_from_slice(b",\"artist\":");
    serde_json::to_writer(&mut *output, &file.artist())?;
    output.extend_from_slice(b",\"album\":");
    serde_json::to_writer(&mut *output, &file.album())?;
    output.extend_from_slice(b",\"size_str\":");
    serde_json::to_writer(&mut *output, &format_bytes(file.size()))?;
    output.extend_from_slice(b",\"ext\":");
    serde_json::to_writer(&mut *output, &extension)?;
    output.extend_from_slice(b",\"cat\":");
    serde_json::to_writer(&mut *output, category)?;
    output.push(b'}');
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_contains_no_runtime_data_markers() {
        assert!(!DASHBOARD_TEMPLATE.contains("__VUIO_"));
        assert!(DASHBOARD_TEMPLATE.contains("/api/server-info"));
    }
}

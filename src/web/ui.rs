//! Browser dashboard handler and compile-time UI template rendering.

use crate::web::format::format_bytes;
use crate::{
    database::{DatabaseReadSession, MediaFileQuery, MediaFileView, MediaRepository},
    error::AppError,
    state::AppState,
};
use axum::{
    extract::{Query, State},
    http::header,
    response::IntoResponse,
    Json,
};

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

pub async fn server_info_handler(State(state): State<AppState>) -> Json<ServerInfo> {
    let monitored_directories = state
        .media_directories
        .read()
        .await
        .iter()
        .map(|directory| directory.path.clone())
        .collect();
    Json(ServerInfo {
        server_name: state.config.server.name.clone(),
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

#[derive(serde::Serialize)]
struct WebMediaFile {
    id: i64,
    path: String,
    name: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    size_str: String,
    ext: String,
    cat: String,
}

#[derive(serde::Serialize)]
pub struct MediaPage {
    files: Vec<WebMediaFile>,
    next_cursor: Option<String>,
}

pub async fn media_page_handler(
    State(state): State<AppState>,
    Query(params): Query<MediaPageQuery>,
) -> Result<Json<MediaPage>, AppError> {
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
    let mut files = state
        .database
        .clone()
        .read(move |session| {
            let mut page = Vec::with_capacity(fetch_limit);
            session.visit_files(&query, 0, fetch_limit, |file| {
                page.push(web_media_file(&file));
                Ok(())
            })?;
            Ok(page)
        })
        .await
        .map_err(AppError::Internal)?;
    let has_more = files.len() > limit;
    if has_more {
        files.pop();
    }
    let next_cursor = has_more
        .then(|| files.last().map(|file| file.id.to_string()))
        .flatten();
    Ok(Json(MediaPage { files, next_cursor }))
}

fn web_media_file(file: &impl MediaFileView) -> WebMediaFile {
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
    WebMediaFile {
        id: file.id().unwrap_or_default(),
        path: file.path().to_owned(),
        name: file.filename().to_owned(),
        title: file.title().map(str::to_owned),
        artist: file.artist().map(str::to_owned),
        album: file.album().map(str::to_owned),
        size_str: format_bytes(file.size()),
        ext: extension,
        cat: category.to_owned(),
    }
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

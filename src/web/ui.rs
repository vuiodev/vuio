//! Browser dashboard handler and compile-time UI template rendering.

use crate::{
    database::{DatabaseReadSession, MediaFileQuery, MediaFileView, MediaRepository},
    error::AppError,
    state::AppState,
};
use crate::web::format::format_bytes;
use axum::{extract::{Query, State}, http::header, response::IntoResponse, Json};

const DASHBOARD_TEMPLATE: &str = include_str!("ui/dashboard.html");

fn render_dashboard(server_name: &str, files_json: &str, dirs_json: &str) -> String {
    DASHBOARD_TEMPLATE
        .replace("__VUIO_SERVER_NAME__", server_name)
        .replace("__VUIO_FILES_JSON__", files_json)
        .replace("__VUIO_DIRS_JSON__", dirs_json)
}

pub async fn root_handler(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let monitored_dirs: Vec<String> = state
        .media_directories
        .read()
        .await
        .iter()
        .map(|dir| dir.path.clone())
        .collect();
    let dirs_json = serde_json::to_string(&monitored_dirs).unwrap_or_else(|_| "[]".to_string());

    let server_name = html_escape(&state.config.server.name);

    let html_content = render_dashboard(&server_name, "[]", &dirs_json);

    Ok((
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html_content,
    ))
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

fn html_escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + s.len() / 4);
    for ch in s.chars() {
        match ch {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#39;"),
            c => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_template_replaces_all_dynamic_markers() {
        let html = render_dashboard(
            "VuIO &amp; Friends",
            r#"[{"id":1,"name":"Movie"}]"#,
            r#"["/media"]"#,
        );

        assert!(!html.contains("__VUIO_"));
        assert!(html.contains("<title>VuIO &amp; Friends</title>"));
        assert!(html.contains(r#"[{"id":1,"name":"Movie"}]"#));
        assert!(html.contains(r#"["/media"]"#));
        assert!(html.contains("${file.id}"));
    }

    #[test]
    fn dashboard_server_name_is_html_escaped() {
        assert_eq!(
            html_escape("<VuIO & \"TV\">"),
            "&lt;VuIO &amp; &quot;TV&quot;&gt;"
        );
    }
}

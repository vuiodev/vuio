//! Browser dashboard handler and compile-time UI template rendering.

use crate::{database::MediaRepository, error::AppError, state::AppState};
use axum::{extract::State, http::header, response::IntoResponse};

const DASHBOARD_TEMPLATE: &str = include_str!("ui/dashboard.html");

fn render_dashboard(server_name: &str, files_json: &str, dirs_json: &str) -> String {
    DASHBOARD_TEMPLATE
        .replace("__VUIO_SERVER_NAME__", server_name)
        .replace("__VUIO_FILES_JSON__", files_json)
        .replace("__VUIO_DIRS_JSON__", dirs_json)
}

pub async fn root_handler(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    use futures_util::StreamExt;

    let mut stream = state.database.stream_all_media_files();
    let mut files = Vec::new();
    while let Some(res) = stream.next().await {
        if let Ok(file) = res {
            files.push(file);
        }
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

    let mut web_files = Vec::new();
    for file in &files {
        let extension = file
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let category = if file.mime_type == "audio/radio" {
            "radio"
        } else if file.mime_type.starts_with("video/") {
            "video"
        } else if file.mime_type.starts_with("audio/") {
            "audio"
        } else if file.mime_type.starts_with("image/") {
            "image"
        } else {
            "file"
        };

        web_files.push(WebMediaFile {
            id: file.id.unwrap_or(0),
            path: file.path.to_string_lossy().to_string(),
            name: file.filename.clone(),
            title: file.title.clone(),
            artist: file.artist.clone(),
            album: file.album.clone(),
            size_str: format_size(file.size),
            ext: extension,
            cat: category.to_string(),
        });
    }

    let files_json = serde_json::to_string(&web_files).unwrap_or_else(|_| "[]".to_string());

    let monitored_dirs: Vec<String> = state
        .media_directories
        .read()
        .await
        .iter()
        .map(|dir| dir.path.clone())
        .collect();
    let dirs_json = serde_json::to_string(&monitored_dirs).unwrap_or_else(|_| "[]".to_string());

    let server_name = html_escape(&state.config.server.name);

    let html_content = render_dashboard(&server_name, &files_json, &dirs_json);

    Ok((
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html_content,
    ))
}

fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "0 Bytes".to_string();
    }
    let k = 1024.0;
    let sizes = ["Bytes", "KB", "MB", "GB", "TB"];
    let i = (bytes as f64).log(k).floor() as usize;
    let i = std::cmp::min(i, sizes.len() - 1);
    format!("{:.1} {}", bytes as f64 / k.powi(i as i32), sizes[i])
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

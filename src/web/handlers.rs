use crate::{
    database::MediaDirectory,
    error::AppError,
    state::AppState,
    web::xml::{generate_description_xml, generate_scpd_xml},
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;
use tracing::{debug, error, info, warn};

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
        self.total_response_time_us.fetch_add(response_time_us, Ordering::Relaxed);
        if cache_hit {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.cache_misses.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    pub fn record_directory_listing(&self, response_time_us: u64) {
        self.directory_listings.fetch_add(1, Ordering::Relaxed);
        self.total_response_time_us.fetch_add(response_time_us, Ordering::Relaxed);
    }
    
    pub fn record_file_serve(&self, response_time_us: u64, is_actual_serve: bool) {
        if is_actual_serve {
            self.file_serves.fetch_add(1, Ordering::Relaxed);
        }
        self.total_response_time_us.fetch_add(response_time_us, Ordering::Relaxed);
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
            average_response_time_ms: if browse_requests > 0 { (total_time_us as f64 / browse_requests as f64) / 1000.0 } else { 0.0 },
            cache_hit_rate: if browse_requests > 0 { 
                (self.cache_hits.load(Ordering::Relaxed) as f64 / browse_requests as f64) * 100.0 
            } else { 0.0 },
            gigabytes_transferred: self.bytes_transferred.load(Ordering::Relaxed) as f64 / 1_073_741_824.0,
        }
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

// Web metrics will be stored in AppState for atomic access

pub async fn root_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
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
        let extension = file.path.extension()
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
    
    let monitored_dirs: Vec<String> = state.config.media.directories
        .iter()
        .map(|dir| dir.path.clone())
        .collect();
    let dirs_json = serde_json::to_string(&monitored_dirs).unwrap_or_else(|_| "[]".to_string());

    let server_name = html_escape(&state.config.server.name);

    let html_content = format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{server_name}</title>
    <link rel="icon" type="image/svg+xml" href='data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect width="100" height="100" rx="25" fill="url(#g)"/><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="#00f0ff"/><stop offset="100%" stop-color="#7000ff"/></linearGradient><polygon points="40,30 70,50 40,70" fill="white"/></svg>'>
    <style>
        :root {{
            --bg-color: #0c0f12;
            --card-bg: rgba(22, 28, 36, 0.6);
            --card-border: rgba(255, 255, 255, 0.06);
            --text-primary: #f3f4f6;
            --text-secondary: #9ca3af;
            --accent-color: #00f0ff;
            --accent-gradient: linear-gradient(135deg, #00f0ff 0%, #7000ff 100%);
            --accent-hover: #00d8e6;
            --focus-ring: rgba(0, 240, 255, 0.2);
            --folder-color: #ffb300;
            --folder-bg: rgba(255, 179, 0, 0.06);
        }}
        
        * {{
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }}
        
        body {{
            background-color: var(--bg-color);
            color: var(--text-primary);
            font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            line-height: 1.5;
            padding: 1rem;
            min-height: 100vh;
            display: flex;
            flex-direction: column;
            align-items: center;
        }}

        @media (min-width: 640px) {{
            body {{
                padding: 2rem;
            }}
        }}

        .container {{
            width: 100%;
            max-width: 100%;
            display: flex;
            flex-direction: column;
            gap: 1.25rem;
        }}

        .main-nav {{
            display: flex;
            gap: 0.5rem;
            border-bottom: 1px solid var(--card-border);
            padding-bottom: 0.75rem;
            margin-bottom: 0.5rem;
            width: 100%;
        }}

        .nav-tab {{
            background: transparent;
            border: none;
            color: var(--text-secondary);
            padding: 0.5rem 1.25rem;
            cursor: pointer;
            font-size: 0.95rem;
            font-weight: 600;
            transition: all 0.2s ease;
            border-radius: 8px;
            outline: none;
        }}

        .nav-tab.active {{
            background: var(--accent-gradient);
            color: white;
            box-shadow: 0 0 12px rgba(0, 240, 255, 0.25);
        }}

        .nav-tab:hover:not(.active) {{
            color: var(--text-primary);
            background: rgba(255, 255, 255, 0.04);
        }}

        header {{
            background: var(--card-bg);
            border: 1px solid var(--card-border);
            backdrop-filter: blur(12px);
            -webkit-backdrop-filter: blur(12px);
            border-radius: 16px;
            padding: 1.25rem;
            display: flex;
            flex-wrap: wrap;
            justify-content: space-between;
            align-items: center;
            gap: 1rem;
        }}

        .brand-section {{
            display: flex;
            align-items: center;
            gap: 0.75rem;
        }}

        .brand-logo {{
            width: 40px;
            height: 40px;
            background: var(--accent-gradient);
            border-radius: 10px;
            display: flex;
            align-items: center;
            justify-content: center;
            color: white;
            font-weight: bold;
            font-size: 1.25rem;
            box-shadow: 0 0 15px rgba(0, 240, 255, 0.25);
        }}

        .brand-info h1 {{
            font-size: 1.15rem;
            font-weight: 700;
            background: var(--accent-gradient);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
        }}

        .brand-info p {{
            font-size: 0.75rem;
            color: var(--text-secondary);
        }}

        /* Breadcrumbs navigation */
        .breadcrumbs {{
            display: flex;
            align-items: center;
            flex-wrap: wrap;
            gap: 0.25rem;
            font-size: 0.85rem;
            color: var(--text-secondary);
            padding: 0 0.5rem;
        }}

        .breadcrumb-item {{
            cursor: pointer;
            transition: color 0.2s ease;
            font-weight: 500;
        }}

        .breadcrumb-item:hover {{
            color: var(--accent-color);
            text-decoration: underline;
        }}

        .breadcrumb-separator {{
            color: rgba(255, 255, 255, 0.2);
            user-select: none;
        }}

        /* Controls: Search & Tabs */
        .controls {{
            display: flex;
            flex-direction: column;
            gap: 1rem;
        }}

        @media (min-width: 640px) {{
            .controls {{
                flex-direction: row;
                align-items: center;
            }}
        }}

        .search-box {{
            position: relative;
            flex: 1;
        }}

        .search-box input {{
            width: 100%;
            background: var(--card-bg);
            border: 1px solid var(--card-border);
            border-radius: 12px;
            padding: 0.75rem 1rem 0.75rem 2.5rem;
            color: var(--text-primary);
            font-size: 0.9375rem;
            outline: none;
            transition: all 0.2s ease;
        }}

        .search-box input:focus {{
            border-color: var(--accent-color);
            box-shadow: 0 0 0 3px var(--focus-ring);
        }}

        .search-icon {{
            position: absolute;
            left: 0.875rem;
            top: 50%;
            transform: translateY(-50%);
            width: 16px;
            height: 16px;
            color: var(--text-secondary);
            pointer-events: none;
        }}

        .tabs {{
            display: flex;
            background: rgba(22, 28, 36, 0.4);
            border: 1px solid var(--card-border);
            border-radius: 12px;
            padding: 0.25rem;
            gap: 0.25rem;
            align-self: flex-start;
        }}

        @media (max-width: 639px) {{
            .tabs {{
                width: 100%;
                justify-content: stretch;
            }}
            .tab-btn {{
                flex: 1;
            }}
        }}

        .tab-btn {{
            background: transparent;
            border: none;
            color: var(--text-secondary);
            padding: 0.5rem 1rem;
            border-radius: 8px;
            cursor: pointer;
            font-size: 0.875rem;
            font-weight: 500;
            transition: all 0.2s ease;
            text-align: center;
        }}

        .tab-btn.active {{
            background: var(--card-border);
            color: var(--text-primary);
        }}

        .tab-btn:hover:not(.active) {{
            color: var(--text-primary);
            background: rgba(255, 255, 255, 0.02);
        }}

        /* File List Container */
        .file-list {{
            display: flex;
            flex-direction: column;
            gap: 0.65rem;
        }}

        .media-card {{
            background: var(--card-bg);
            border: 1px solid var(--card-border);
            border-radius: 12px;
            padding: 0.875rem;
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 1rem;
            transition: all 0.2s ease;
            text-decoration: none;
            color: inherit;
        }}

        .media-card:hover {{
            border-color: rgba(0, 240, 255, 0.25);
            background: rgba(22, 28, 36, 0.85);
            transform: translateY(-1px);
        }}

        .media-info {{
            display: flex;
            align-items: center;
            gap: 0.75rem;
            min-width: 0;
            flex: 1;
        }}

        .media-icon-wrapper {{
            width: 40px;
            height: 40px;
            border-radius: 8px;
            background: rgba(255, 255, 255, 0.03);
            display: flex;
            align-items: center;
            justify-content: center;
            flex-shrink: 0;
            color: var(--text-secondary);
        }}

        .media-card:hover .media-icon-wrapper {{
            color: var(--accent-color);
            background: rgba(0, 240, 255, 0.06);
        }}

        /* Folder Specific Styles */
        .folder-card .media-icon-wrapper {{
            color: var(--folder-color);
            background: var(--folder-bg);
        }}

        .folder-card:hover .media-icon-wrapper {{
            color: #ffe082;
            background: rgba(255, 179, 0, 0.12);
        }}

        .media-details {{
            min-width: 0;
            display: flex;
            flex-direction: column;
            gap: 0.125rem;
        }}

        .media-name {{
            font-size: 0.9rem;
            font-weight: 500;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }}

        .media-meta {{
            display: flex;
            align-items: center;
            gap: 0.5rem;
            font-size: 0.725rem;
            color: var(--text-secondary);
        }}

        .media-meta-dot {{
            width: 3px;
            height: 3px;
            border-radius: 50%;
            background-color: rgba(255, 255, 255, 0.15);
        }}

        .action-area {{
            display: flex;
            align-items: center;
            gap: 0.5rem;
            flex-shrink: 0;
        }}

        .btn-action {{
            width: 38px;
            height: 38px;
            border-radius: 8px;
            border: 1px solid var(--card-border);
            background: rgba(255, 255, 255, 0.01);
            color: var(--text-secondary);
            display: flex;
            align-items: center;
            justify-content: center;
            cursor: pointer;
            transition: all 0.2s ease;
        }}

        .media-card:hover .btn-action {{
            border-color: rgba(0, 240, 255, 0.15);
            background: rgba(0, 240, 255, 0.04);
            color: var(--accent-color);
        }}

        .btn-action:hover {{
            background: var(--accent-gradient) !important;
            color: white !important;
            border-color: transparent !important;
            box-shadow: 0 0 10px rgba(0, 240, 255, 0.3);
        }}

        /* Empty State */
        .empty-state {{
            text-align: center;
            padding: 3rem 1.5rem;
            background: var(--card-bg);
            border: 1px dashed var(--card-border);
            border-radius: 14px;
            display: flex;
            flex-direction: column;
            align-items: center;
            gap: 0.75rem;
        }}

        .empty-icon {{
            width: 40px;
            height: 40px;
            color: var(--text-secondary);
        }}

        .empty-state h3 {{
            font-size: 1.05rem;
            font-weight: 600;
        }}

        .empty-state p {{
            font-size: 0.85rem;
            color: var(--text-secondary);
        }}

        /* Photos Image Grid and Lightbox Styles */
        .image-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
            gap: 12px;
            padding: 0.5rem 0;
        }}

        .image-card {{
            position: relative;
            border-radius: 12px;
            overflow: hidden;
            aspect-ratio: 1 / 1;
            background: #181f2a;
            border: 1px solid var(--card-border);
            cursor: pointer;
            transition: all 0.2s ease-in-out;
        }}

        .image-card:hover {{
            transform: scale(1.02);
            border-color: var(--accent-color);
            box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
        }}

        .image-card img {{
            width: 100%;
            height: 100%;
            object-fit: cover;
            transition: transform 0.3s ease;
        }}

        .image-card:hover img {{
            transform: scale(1.05);
        }}

        .image-card-overlay {{
            position: absolute;
            bottom: 0;
            left: 0;
            right: 0;
            background: linear-gradient(to top, rgba(0,0,0,0.85) 0%, rgba(0,0,0,0) 100%);
            padding: 1.5rem 0.75rem 0.75rem;
            color: white;
            opacity: 0;
            transition: opacity 0.2s ease;
            display: flex;
            justify-content: space-between;
            align-items: flex-end;
        }}

        .image-card:hover .image-card-overlay {{
            opacity: 1;
        }}

        .image-card-name {{
            font-size: 0.75rem;
            font-weight: 500;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
            margin-right: 0.5rem;
            flex: 1;
        }}

        .image-card-download {{
            color: rgba(255,255,255,0.7);
            display: flex;
            align-items: center;
            justify-content: center;
            transition: color 0.2s;
        }}

        .image-card-download:hover {{
            color: var(--accent-color);
        }}

        .image-grid .folder-card {{
            aspect-ratio: 1 / 1;
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            text-align: center;
            gap: 0.75rem;
            background: rgba(255, 255, 255, 0.02);
            border: 1px dashed var(--card-border);
        }}

        .image-grid .folder-card .media-info {{
            flex-direction: column;
            gap: 0.5rem;
            align-items: center;
            width: 100%;
        }}

        .image-grid .folder-card .media-details {{
            display: flex;
            flex-direction: column;
            align-items: center;
            text-align: center;
            width: 100%;
        }}

        .image-grid .folder-card .action-area {{
            margin-top: 0.5rem;
        }}

        /* Lightbox modal styles */
        .lightbox-modal {{
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background: rgba(10, 14, 20, 0.95);
            z-index: 2000;
            display: flex;
            align-items: center;
            justify-content: center;
            backdrop-filter: blur(15px);
            -webkit-backdrop-filter: blur(15px);
            animation: fadeIn 0.25s ease-out;
        }}

        @keyframes fadeIn {{
            from {{ opacity: 0; }}
            to {{ opacity: 1; }}
        }}

        .lightbox-close {{
            position: absolute;
            top: 1.5rem;
            right: 2rem;
            color: var(--text-secondary);
            font-size: 2.5rem;
            font-weight: 300;
            cursor: pointer;
            transition: color 0.2s;
            line-height: 1;
            z-index: 2010;
        }}

        .lightbox-close:hover {{
            color: #ef4444;
        }}

        .lightbox-nav {{
            position: absolute;
            top: 50%;
            transform: translateY(-50%);
            background: rgba(255, 255, 255, 0.03);
            border: 1px solid var(--card-border);
            color: var(--text-secondary);
            width: 48px;
            height: 48px;
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            cursor: pointer;
            transition: all 0.2s;
            z-index: 2010;
        }}

        .lightbox-nav:hover {{
            background: var(--accent-gradient);
            color: white;
            border-color: transparent;
            box-shadow: 0 0 15px rgba(0, 240, 255, 0.4);
        }}

        .lightbox-prev {{ left: 2rem; }}
        .lightbox-next {{ right: 2rem; }}

        @media (max-width: 600px) {{
            .lightbox-prev {{ left: 0.5rem; }}
            .lightbox-next {{ right: 0.5rem; }}
            .lightbox-nav {{
                width: 38px;
                height: 38px;
            }}
        }}

        .lightbox-content-wrapper {{
            max-width: 85%;
            max-height: 85%;
            display: flex;
            flex-direction: column;
            align-items: center;
            position: relative;
        }}

        .lightbox-content-wrapper img {{
            max-width: 100%;
            max-height: 80vh;
            object-fit: contain;
            border-radius: 8px;
            box-shadow: 0 10px 40px rgba(0, 0, 0, 0.6);
            border: 1px solid rgba(255, 255, 255, 0.05);
        }}

        .lightbox-meta {{
            margin-top: 1rem;
            color: var(--text-primary);
            font-size: 0.95rem;
            font-weight: 500;
            display: flex;
            align-items: center;
            gap: 1rem;
            width: 100%;
            justify-content: center;
        }}

        .lightbox-download-btn {{
            color: var(--text-secondary);
            display: flex;
            align-items: center;
            justify-content: center;
            padding: 0.25rem;
            border-radius: 6px;
            transition: all 0.2s;
        }}

        .lightbox-download-btn:hover {{
            color: var(--accent-color);
            background: rgba(255, 255, 255, 0.05);
        }}

        /* Floating Audio Player Bar Styles */
        .audio-player-bar {{
            position: fixed;
            bottom: 0;
            left: 0;
            right: 0;
            height: 90px;
            background: rgba(18, 22, 28, 0.85);
            border-top: 1px solid var(--card-border);
            backdrop-filter: blur(20px);
            -webkit-backdrop-filter: blur(20px);
            z-index: 1000;
            display: flex;
            align-items: center;
            padding: 0 1.5rem;
            box-shadow: 0 -10px 30px rgba(0, 0, 0, 0.5);
            animation: slideUp 0.3s cubic-bezier(0.16, 1, 0.3, 1);
        }}

        @keyframes slideUp {{
            from {{ transform: translateY(100%); }}
            to {{ transform: translateY(0); }}
        }}

        .player-container {{
            width: 100%;
            max-width: 100%;
            margin: 0 auto;
            display: grid;
            grid-template-columns: 280px 1fr 220px;
            align-items: center;
            gap: 1.5rem;
        }}

        @media (max-width: 768px) {{
            .audio-player-bar {{
                height: 140px;
                padding: 0.75rem 1rem;
            }}
            .player-container {{
                grid-template-columns: 1fr;
                grid-template-rows: auto auto auto;
                gap: 0.5rem;
            }}
            .player-extra {{
                justify-content: flex-end;
            }}
        }}

        .player-track-info {{
            display: flex;
            align-items: center;
            gap: 0.75rem;
            min-width: 0;
        }}

        .player-icon-wrapper {{
            width: 42px;
            height: 42px;
            border-radius: 10px;
            background: rgba(255, 255, 255, 0.03);
            border: 1px solid var(--card-border);
            display: flex;
            align-items: center;
            justify-content: center;
            flex-shrink: 0;
        }}

        .player-meta {{
            min-width: 0;
        }}

        .player-title {{
            font-size: 0.9rem;
            font-weight: 600;
            color: var(--text-primary);
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }}

        .player-subtitle {{
            font-size: 0.75rem;
            color: var(--text-secondary);
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
            margin-top: 0.15rem;
        }}

        .player-controls-progress {{
            display: flex;
            flex-direction: column;
            align-items: center;
            gap: 0.4rem;
        }}

        .player-controls {{
            display: flex;
            align-items: center;
            gap: 1.25rem;
        }}

        .player-btn {{
            background: transparent;
            border: none;
            color: var(--text-secondary);
            cursor: pointer;
            padding: 0.35rem;
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            transition: all 0.2s ease;
        }}

        .player-btn:hover {{
            color: var(--text-primary);
            background: rgba(255, 255, 255, 0.04);
        }}

        .player-btn.play-main {{
            width: 38px;
            height: 38px;
            background: var(--accent-gradient);
            color: white;
            box-shadow: 0 0 12px rgba(0, 240, 255, 0.2);
        }}

        .player-btn.play-main:hover {{
            transform: scale(1.05);
            box-shadow: 0 0 16px rgba(0, 240, 255, 0.4);
        }}

        .player-progress-bar {{
            width: 100%;
            display: flex;
            align-items: center;
            gap: 0.75rem;
        }}

        .player-time {{
            font-size: 0.75rem;
            color: var(--text-secondary);
            font-variant-numeric: tabular-nums;
            width: 32px;
        }}

        #player-time-current {{
            text-align: right;
        }}

        .player-progress-bar input[type="range"] {{
            flex: 1;
            -webkit-appearance: none;
            appearance: none;
            height: 24px;
            background: transparent;
            margin: 0;
            padding: 0;
            outline: none;
            cursor: pointer;
        }}

        /* Webkit (Chrome, Safari, Edge) */
        .player-progress-bar input[type="range"]::-webkit-slider-runnable-track {{
            width: 100%;
            height: 6px;
            background: rgba(255, 255, 255, 0.12);
            border-radius: 3px;
            border: none;
        }}

        .player-progress-bar input[type="range"]::-webkit-slider-thumb {{
            -webkit-appearance: none;
            height: 14px;
            width: 14px;
            border-radius: 50%;
            background: #ffffff;
            margin-top: -4px;
            box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
            transition: transform 0.1s ease, background-color 0.1s ease;
        }}

        .player-progress-bar input[type="range"]:active::-webkit-slider-thumb {{
            transform: scale(1.3);
            background: var(--accent-color);
        }}

        /* Firefox */
        .player-progress-bar input[type="range"]::-moz-range-track {{
            width: 100%;
            height: 6px;
            background: rgba(255, 255, 255, 0.12);
            border-radius: 3px;
            border: none;
        }}

        .player-progress-bar input[type="range"]::-moz-range-thumb {{
            height: 14px;
            width: 14px;
            border: none;
            border-radius: 50%;
            background: #ffffff;
            box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
            transition: transform 0.1s ease, background-color 0.1s ease;
        }}

        .player-progress-bar input[type="range"]:active::-moz-range-thumb {{
            transform: scale(1.3);
            background: var(--accent-color);
        }}

        .player-extra {{
            display: flex;
            align-items: center;
            justify-content: flex-end;
            gap: 1.25rem;
        }}

        .player-volume {{
            display: flex;
            align-items: center;
            gap: 0.5rem;
            color: var(--text-secondary);
        }}

        .player-volume input[type="range"] {{
            width: 80px;
            accent-color: var(--accent-color);
            cursor: pointer;
        }}

        .player-btn-close {{
            background: transparent;
            border: none;
            color: var(--text-secondary);
            cursor: pointer;
            padding: 0.35rem;
            border-radius: 8px;
            display: flex;
            align-items: center;
            justify-content: center;
            transition: all 0.2s ease;
        }}

        .player-btn-close:hover {{
            color: #ef4444;
            background: rgba(239, 68, 68, 0.08);
        }}

        /* TV Selection Modal Styles */
        .tv-select-btn {{
            background: rgba(255, 255, 255, 0.03);
            border: 1px solid var(--card-border);
            color: var(--text-primary);
            padding: 0.85rem 1.15rem;
            border-radius: 12px;
            cursor: pointer;
            display: flex;
            align-items: center;
            justify-content: space-between;
            transition: all 0.2s ease;
            font-weight: 600;
            outline: none;
            width: 100%;
            font-size: 0.95rem;
        }}
        .tv-select-btn:hover {{
            border-color: var(--accent-color);
            background: rgba(0, 240, 255, 0.06);
            box-shadow: 0 0 12px rgba(0, 240, 255, 0.2);
            transform: translateY(-1px);
        }}
        .tv-icon-wrapper {{
            display: flex;
            align-items: center;
            gap: 0.85rem;
        }}
    </style>
</head>
<body>
    <div id="files-data" style="display: none;">{files_json}</div>
    <div id="dirs-data" style="display: none;">{dirs_json}</div>

    <div class="container">
        <header>
            <div class="brand-section">
                <div class="brand-logo">▶</div>
                <div class="brand-info">
                    <h1>{server_name}</h1>
                </div>
            </div>
        </header>

        <!-- Main Navigation Tab Bar -->
        <div class="main-nav">
            <button id="nav-browse" class="nav-tab active" onclick="switchNav('browse')">Browse Files</button>
            <button id="nav-stats" class="nav-tab" onclick="switchNav('stats')">System Stats</button>
        </div>

        <!-- Browse View -->
        <div id="view-browse">
            <div class="breadcrumbs" id="breadcrumbs"></div>

            <div class="controls">
                <div class="search-box">
                    <svg class="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
                    <input type="text" id="search-input" placeholder="Search files..." oninput="onSearch()">
                </div>
                <div class="tabs">
                    <button class="tab-btn" data-tab="all" onclick="setTab('all')">All</button>
                    <button class="tab-btn active" data-tab="video" onclick="setTab('video')">Videos</button>
                    <button class="tab-btn" data-tab="audio" onclick="setTab('audio')">Music</button>
                    <button class="tab-btn" data-tab="image" onclick="setTab('image')">Images</button>
                    <button class="tab-btn" data-tab="radio" onclick="setTab('radio')">Radio</button>
                </div>
            </div>

            <div class="file-list" id="file-list"></div>
        </div>

        <!-- System Stats View -->
        <div id="view-stats" style="display: none; flex-direction: column; gap: 1.25rem;">
            <!-- Stats Dashboard Grid -->
            <div class="stats-grid" style="display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 1.25rem;">
                
                <!-- Database stats card -->
                <div class="stats-card" style="background: var(--card-bg); border: 1px solid var(--card-border); border-radius: 14px; padding: 1.25rem; display: flex; flex-direction: column; gap: 0.5rem;">
                    <div style="display: flex; justify-content: space-between; align-items: center;">
                        <span style="font-size: 0.85rem; color: var(--text-secondary); font-weight: 500;">DATABASE INFO</span>
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--accent-color)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22c5.523 0 10-2.239 10-5V7c0-2.761-4.477-5-10-5S2 4.239 2 7v10c0 2.761 4.477 5 10 5z"></path><path d="M2 7c0 2.761 4.477 5 10 5s10-2.239 10-5"></path><path d="M2 12c0 2.761 4.477 5 10 5s10-2.239 10-5"></path></svg>
                    </div>
                    <div id="db-total-files" style="font-size: 1.75rem; font-weight: 700; background: var(--accent-gradient); -webkit-background-clip: text; -webkit-text-fill-color: transparent;">0</div>
                    <div style="font-size: 0.8rem; color: var(--text-secondary);">
                        Total media size: <span id="db-total-size" style="color: var(--text-primary); font-weight: 600;">0 B</span><br>
                        DB File Size: <span id="db-file-size" style="color: var(--text-primary); font-weight: 600;">0 B</span>
                    </div>
                </div>

                <!-- Media breakdown card -->
                <div class="stats-card" style="background: var(--card-bg); border: 1px solid var(--card-border); border-radius: 14px; padding: 1.25rem; display: flex; flex-direction: column; gap: 0.65rem;">
                    <div style="display: flex; justify-content: space-between; align-items: center;">
                        <span style="font-size: 0.85rem; color: var(--text-secondary); font-weight: 500;">MEDIA BREAKDOWN</span>
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#10b981" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20v-6M6 20V10M18 20V4M3 20h18"></path></svg>
                    </div>
                    <div style="display: flex; flex-direction: column; gap: 0.35rem; font-size: 0.85rem; color: var(--text-secondary);">
                        <div style="display: flex; justify-content: space-between;">
                            <span>Videos:</span> <span id="db-video-count" style="color: var(--text-primary); font-weight: 600;">0</span>
                        </div>
                        <div style="display: flex; justify-content: space-between;">
                            <span>Music:</span> <span id="db-audio-count" style="color: var(--text-primary); font-weight: 600;">0</span>
                        </div>
                        <div style="display: flex; justify-content: space-between;">
                            <span>Images:</span> <span id="db-image-count" style="color: var(--text-primary); font-weight: 600;">0</span>
                        </div>
                        <div style="display: flex; justify-content: space-between; border-top: 1px solid rgba(255,255,255,0.05); padding-top: 0.3rem;">
                            <span>Playlists:</span> <span id="db-playlist-count" style="color: var(--text-primary); font-weight: 600;">0</span>
                        </div>
                    </div>
                </div>

                <!-- Web Traffic stats card -->
                <div class="stats-card" style="background: var(--card-bg); border: 1px solid var(--card-border); border-radius: 14px; padding: 1.25rem; display: flex; flex-direction: column; gap: 0.5rem;">
                    <div style="display: flex; justify-content: space-between; align-items: center;">
                        <span style="font-size: 0.85rem; color: var(--text-secondary); font-weight: 500;">WEB TRAFFIC</span>
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#f59e0b" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21.2 15c.7-1.2 1-2.5.7-3.9-.3-2-1.5-3.8-3.2-4.5M2.8 9c-.7 1.2-1 2.5-.7 3.9.3 2 1.5 3.8 3.2 4.5M16 12a4 4 0 10-8 0 4 4 0 008 0z"></path></svg>
                    </div>
                    <div id="web-gigabytes" style="font-size: 1.75rem; font-weight: 700; color: #f59e0b;">0.00 GB</div>
                    <div style="font-size: 0.8rem; color: var(--text-secondary);">
                        File Serves: <span id="web-file-serves" style="color: var(--text-primary); font-weight: 600;">0</span><br>
                        Dir Listings: <span id="web-dir-listings" style="color: var(--text-primary); font-weight: 600;">0</span><br>
                        Current Speed: <span id="web-speed" style="color: var(--text-primary); font-weight: 600;">0 Mbps</span>
                    </div>
                </div>

                <!-- Cache metrics card -->
                <div class="stats-card" style="background: var(--card-bg); border: 1px solid var(--card-border); border-radius: 14px; padding: 1.25rem; display: flex; flex-direction: column; gap: 0.5rem;">
                    <div style="display: flex; justify-content: space-between; align-items: center;">
                        <span style="font-size: 0.85rem; color: var(--text-secondary); font-weight: 500;">CACHE HIT RATE</span>
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#3b82f6" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><path d="M12 6v6l4 2"></path></svg>
                    </div>
                    <div id="web-cache-rate" style="font-size: 1.75rem; font-weight: 700; color: #3b82f6;">0.0%</div>
                    <div style="font-size: 0.8rem; color: var(--text-secondary);">
                        Hits: <span id="web-cache-hits" style="color: var(--text-primary); font-weight: 600;">0</span><br>
                        Misses: <span id="web-cache-misses" style="color: var(--text-primary); font-weight: 600;">0</span>
                    </div>
                </div>

            </div>

            <!-- Server Performance Info -->
            <div style="background: var(--card-bg); border: 1px solid var(--card-border); border-radius: 14px; padding: 1.25rem; display: flex; flex-direction: column; gap: 0.75rem;">
                <h3 style="font-size: 0.95rem; font-weight: 600; letter-spacing: 0.05em; color: var(--text-secondary);">SERVER HEALTH & PERFORMANCE</h3>
                <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; font-size: 0.875rem;">
                    <div style="display: flex; flex-direction: column; gap: 0.25rem;">
                        <span style="color: var(--text-secondary);">Avg Response Time:</span>
                        <span id="web-response-time" style="font-size: 1.15rem; font-weight: 700; color: var(--accent-color);">0 ms</span>
                    </div>
                    <div style="display: flex; flex-direction: column; gap: 0.25rem;">
                        <span style="color: var(--text-secondary);">Errors Logged:</span>
                        <span id="web-errors" style="font-size: 1.15rem; font-weight: 700; color: #ef4444;">0</span>
                    </div>
                    <div style="display: flex; flex-direction: column; gap: 0.25rem;">
                        <span style="color: var(--text-secondary);">Status:</span>
                        <span id="server-status-badge" style="font-size: 1.15rem; font-weight: 700; color: #10b981; display: flex; align-items: center; gap: 0.35rem;">
                            <span id="server-status-dot" style="display: inline-block; width: 8px; height: 8px; background: #10b981; border-radius: 50%; box-shadow: 0 0 8px #10b981;"></span>
                            <span id="server-status-text">Online</span>
                        </span>
                    </div>
                </div>
            </div>

            <!-- Active TV streams / casts -->
            <div style="background: var(--card-bg); border: 1px solid var(--card-border); border-radius: 14px; padding: 1.25rem; display: flex; flex-direction: column; gap: 0.75rem;">
                <h3 style="font-size: 0.95rem; font-weight: 600; letter-spacing: 0.05em; color: var(--text-secondary); display: flex; align-items: center; gap: 0.5rem;">
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--accent-color)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect><line x1="8" y1="21" x2="16" y2="21"></line><line x1="12" y1="17" x2="12" y2="21"></line></svg>
                    ACTIVE SMART TV STREAMING QUEUES
                </h3>
                <div id="active-tv-casts-container" style="display: flex; flex-direction: column; gap: 0.75rem; font-size: 0.875rem;">
                    <div style="color: var(--text-secondary); font-style: italic; padding: 0.25rem 0;">No active TV streams.</div>
                </div>
            </div>
        </div>
    </div>

    <!-- Floating Audio Player Bar -->
    <div id="audio-player-bar" class="audio-player-bar" style="display: none;">
        <div class="player-container">
            <audio id="audio-element" style="display: none;"></audio>
            
            <div class="player-track-info">
                <div class="player-icon-wrapper">
                    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--accent-color)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l12-2v13"></path><circle cx="6" cy="18" r="3"></circle><circle cx="18" cy="16" r="3"></circle></svg>
                </div>
                <div class="player-meta">
                    <div style="display: flex; align-items: center; gap: 0.5rem; min-width: 0;">
                        <div id="player-title" class="player-title">Track Name</div>
                        <span id="player-track-count" style="font-size: 0.75rem; color: var(--accent-color); font-weight: 600; white-space: nowrap;"></span>
                    </div>
                    <div id="player-subtitle" class="player-subtitle">Artist — Album</div>
                </div>
            </div>

            <div class="player-controls-progress">
                <div class="player-controls">
                    <button class="player-btn" onclick="playPrev()" title="Previous Track">
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="19 20 9 12 19 4 19 20"></polygon><line x1="5" y1="19" x2="5" y2="5"></line></svg>
                    </button>
                    <button class="player-btn play-main" id="player-play-btn" onclick="togglePlayPause()" title="Play/Pause">
                        <svg id="play-icon" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>
                        <svg id="pause-icon" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="display: none;"><rect x="6" y="4" width="4" height="16"></rect><rect x="14" y="4" width="4" height="16"></rect></svg>
                    </button>
                    <button class="player-btn" onclick="playNext()" title="Next Track">
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 4 15 12 5 20 5 4"></polygon><line x1="19" y1="5" x2="19" y2="19"></line></svg>
                    </button>
                </div>
                <div class="player-progress-bar">
                    <span id="player-time-current" class="player-time">0:00</span>
                    <input type="range" id="player-progress-slider" min="0" max="100" value="0" oninput="onProgressSeek(this.value)">
                    <span id="player-time-duration" class="player-time">0:00</span>
                </div>
            </div>

            <div class="player-extra">
                <div class="player-volume">
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path></svg>
                    <input type="range" id="player-volume-slider" min="0" max="100" value="80" oninput="onVolumeChange(this.value)">
                </div>
                <button class="player-btn-close" onclick="closePlayer()" title="Close Player">
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
                </button>
            </div>
        </div>
    </div>

    <!-- Lightbox Modal for Images -->
    <div id="image-lightbox" class="lightbox-modal" style="display: none;" onclick="closeLightbox()">
        <span class="lightbox-close" onclick="closeLightbox()">&times;</span>
        <button class="lightbox-nav lightbox-prev" onclick="event.stopPropagation(); showPrevImage()" title="Previous Image">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"></polyline></svg>
        </button>
        <div class="lightbox-content-wrapper" onclick="event.stopPropagation()">
            <img id="lightbox-img" src="" alt="Full view">
            <div class="lightbox-meta">
                <span id="lightbox-title">Image Name</span>
                <a id="lightbox-download" href="" download="" class="lightbox-download-btn" title="Download High Res">
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>
                </a>
            </div>
        </div>
        <button class="lightbox-nav lightbox-next" onclick="event.stopPropagation(); showNextImage()" title="Next Image">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg>
        </button>
    </div>

    <!-- Video Player Modal -->
    <div id="video-modal" class="lightbox-modal" style="display: none;" onclick="closeVideoPlayer()">
        <span class="lightbox-close" onclick="closeVideoPlayer()">&times;</span>
        <div class="lightbox-content-wrapper" onclick="event.stopPropagation()" style="max-width: 90%; max-height: 90%; width: 800px;">
            <video id="video-player-element" controls autoplay style="width: 100%; border-radius: 8px; box-shadow: 0 10px 40px rgba(0, 0, 0, 0.6); border: 1px solid rgba(255, 255, 255, 0.05);">
                <track id="video-subtitle-track" kind="subtitles" srclang="en" label="English">
            </video>
            <div class="lightbox-meta" style="justify-content: space-between; padding: 0.5rem 0.25rem; display: flex; align-items: center; width: 100%;">
                <div style="display: flex; align-items: center; gap: 1rem; min-width: 0; flex: 1;">
                    <span id="video-player-title" style="font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">Video Name</span>
                    <select id="video-audio-tracks" style="display: none; background: rgba(255,255,255,0.06); border: 1px solid var(--card-border); color: var(--text-primary); border-radius: 6px; padding: 0.25rem 0.5rem; font-size: 0.8rem; cursor: pointer; outline: none;"></select>
                </div>
                <a id="video-player-download" href="" download="" class="lightbox-download-btn" title="Download Video">
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>
                </a>
            </div>
        </div>
    </div>

    <!-- TV Selection Modal -->
    <div id="tv-modal" class="lightbox-modal" style="display: none;" onclick="closeTvModal()">
        <span class="lightbox-close" onclick="closeTvModal()">&times;</span>
        <div class="lightbox-content-wrapper" onclick="event.stopPropagation()" style="max-width: 420px; background: #0f172a; border: 1px solid var(--card-border); border-radius: 16px; padding: 1.75rem; display: flex; flex-direction: column; gap: 1.25rem; box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);">
            <div style="display: flex; flex-direction: column; gap: 0.35rem;">
                <h3 style="font-size: 1.3rem; font-weight: 700; color: var(--text-primary); display: flex; align-items: center; gap: 0.5rem;">
                    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--accent-color)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect><line x1="8" y1="21" x2="16" y2="21"></line><line x1="12" y1="17" x2="12" y2="21"></line></svg>
                    Play on TV
                </h3>
                <p id="tv-modal-subtitle" style="font-size: 0.88rem; color: var(--text-secondary); line-height: 1.4;">Select the destination TV screen to play folder "<span id="tv-modal-folder-name" style="color: var(--text-primary); font-weight: 600;"></span>"</p>
            </div>
            <div id="tv-list-container" style="display: flex; flex-direction: column; gap: 0.75rem; margin: 0.25rem 0; max-height: 250px; overflow-y: auto; padding-right: 4px;">
                <!-- TV buttons will be dynamically generated here -->
            </div>
            <button onclick="closeTvModal()" style="background: rgba(255, 255, 255, 0.04); border: 1px solid var(--card-border); color: var(--text-secondary); border-radius: 10px; padding: 0.7rem; cursor: pointer; transition: all 0.2s; font-weight: 600;" onmouseover="this.style.background='rgba(255,255,255,0.08)'; this.style.color='var(--text-primary)';" onmouseout="this.style.background='rgba(255,255,255,0.04)'; this.style.color='var(--text-secondary)';">Cancel</button>
        </div>
    </div>

    <script>
        let activeNav = 'browse';
        let metricsTimer = null;
        let lastBytes = null;
        let lastTime = null;
        let playlist = [];
        let currentTrackIndex = -1;
        let imageList = [];
        let currentImageIndex = -1;

        function switchNav(nav) {{
            activeNav = nav;
            document.querySelectorAll('.nav-tab').forEach(btn => {{
                if (btn.id === 'nav-' + nav) {{
                    btn.classList.add('active');
                }} else {{
                    btn.classList.remove('active');
                }}
            }});

            if (nav === 'browse') {{
                document.getElementById('view-browse').style.display = 'block';
                document.getElementById('view-stats').style.display = 'none';
                if (metricsTimer) {{
                    clearInterval(metricsTimer);
                    metricsTimer = null;
                }}
            }} else {{
                document.getElementById('view-browse').style.display = 'none';
                document.getElementById('view-stats').style.display = 'flex';
                updateMetrics();
                metricsTimer = setInterval(updateMetrics, 5000);
            }}
        }}

        function playAudioFile(fileOrId) {{
            let targetFile = null;
            if (typeof fileOrId === 'string' || typeof fileOrId === 'number') {{
                targetFile = filesData.find(f => f.id.toString() === fileOrId.toString());
            }} else {{
                targetFile = fileOrId;
            }}

            if (!targetFile) return;

            // Generate a playlist of all audio files matching the current tab/filter
            let filteredAudio = filesData.filter(f => f.cat === currentTab);
            
            // If currently in a path/folder, filter playlist to the current path
            if (currentPath.length > 0 && searchQuery === '') {{
                filteredAudio = filteredAudio.filter(f => {{
                    const comps = getRelativeComponents(f.path);
                    if (comps.length <= currentPath.length) return false;
                    for (let i = 0; i < currentPath.length; i++) {{
                        if (comps[i] !== currentPath[i]) return false;
                    }}
                    return true;
                }});
            }} else if (searchQuery !== '') {{
                filteredAudio = filteredAudio.filter(f => f.name.toLowerCase().includes(searchQuery));
            }}

            // Sort playlist by name
            filteredAudio.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
            
            playlist = filteredAudio;
            currentTrackIndex = playlist.findIndex(f => f.id.toString() === targetFile.id.toString());
            if (currentTrackIndex === -1) {{
                playlist = [targetFile];
                currentTrackIndex = 0;
            }}

            loadAndPlayTrack();
        }}

        function playFolder(folderName) {{
            const targetPath = [...currentPath, folderName];
            
            // Filter all audio files that reside in targetPath or any subdirectory
            let folderAudio = filesData.filter(file => {{
                if (file.cat !== 'audio') return false;
                const comps = getRelativeComponents(file.path);
                if (comps.length <= targetPath.length) return false;
                for (let i = 0; i < targetPath.length; i++) {{
                    if (comps[i] !== targetPath[i]) return false;
                }}
                return true;
            }});

            if (folderAudio.length === 0) return;

            // Sort playlist alphabetically by name
            folderAudio.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));

            playlist = folderAudio;
            currentTrackIndex = 0;
            loadAndPlayTrack();
        }}

        function playVideoFolderOnTv(folderName) {{
            const targetPath = [...currentPath, folderName];
            
            // Filter all video files that reside in targetPath or any subdirectory
            let folderVideos = filesData.filter(file => {{
                if (file.cat !== 'video') return false;
                const comps = getRelativeComponents(file.path);
                if (comps.length <= targetPath.length) return false;
                for (let i = 0; i < targetPath.length; i++) {{
                    if (comps[i] !== targetPath[i]) return false;
                }}
                return true;
            }});

            if (folderVideos.length === 0) {{
                showToast("No video files found in this folder.", "error");
                return;
            }}

            // Sort playlist alphabetically by name
            folderVideos.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
            const fileIds = folderVideos.map(v => v.id);

            showToast("Discovering TVs on local network...", "info");
            
            fetch('/api/tvs')
                .then(res => res.json())
                .then(tvs => {{
                    if (!Array.isArray(tvs) || tvs.length === 0) {{
                        showToast("No TVs found on your local network.", "error");
                        return;
                    }}
                    
                    if (tvs.length === 1) {{
                        // Only one TV found, cast immediately!
                        castPlaylistToTv(tvs[0], folderName, fileIds);
                    }} else {{
                        // Multiple TVs found, show selection modal
                        showTvSelectionModal(tvs, folderName, fileIds);
                    }}
                }})
                .catch(err => {{
                    console.error("TV discovery failed", err);
                    showToast("Failed to discover TVs: " + err, "error");
                }});
        }}

        function showTvSelectionModal(tvs, folderName, fileIds) {{
            document.getElementById('tv-modal-folder-name').textContent = folderName;
            const container = document.getElementById('tv-list-container');
            container.innerHTML = '';
            
            tvs.forEach(tvName => {{
                const btn = document.createElement('button');
                btn.className = 'tv-select-btn';
                btn.onclick = () => {{
                    closeTvModal();
                    castPlaylistToTv(tvName, folderName, fileIds);
                }};
                btn.innerHTML = `
                    <div class="tv-icon-wrapper">
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect><line x1="8" y1="21" x2="16" y2="21"></line><line x1="12" y1="17" x2="12" y2="21"></line></svg>
                        <span>${{tvName}}</span>
                    </div>
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--accent-color)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg>
                `;
                container.appendChild(btn);
            }});
            
            document.getElementById('tv-modal').style.display = 'flex';
        }}

        function closeTvModal() {{
            document.getElementById('tv-modal').style.display = 'none';
        }}

        function castPlaylistToTv(tvName, folderName, fileIds) {{
            showToast("Casting playlist to " + tvName + "...", "info");
            
            fetch('/api/cast/playlist', {{
                method: 'POST',
                headers: {{ 'Content-Type': 'application/json' }},
                body: JSON.stringify({{
                    tv_name: tvName,
                    folder_name: folderName,
                    file_ids: fileIds
                }})
            }})
            .then(res => res.json())
            .then(data => {{
                if (data.error) {{
                    showToast("Cast error: " + data.error, "error");
                }} else {{
                    showToast("Successfully playing on " + tvName + "!", "success");
                }}
            }})
            .catch(err => {{
                console.error("Cast request failed", err);
                showToast("Failed to cast to TV: " + err, "error");
            }});
        }}

        function showToast(message, type = 'info') {{
            let container = document.getElementById('toast-container');
            if (!container) {{
                container = document.createElement('div');
                container.id = 'toast-container';
                container.style.position = 'fixed';
                container.style.bottom = '1.5rem';
                container.style.right = '1.5rem';
                container.style.display = 'flex';
                container.style.flexDirection = 'column';
                container.style.gap = '0.5rem';
                container.style.zIndex = '9999';
                document.body.appendChild(container);
            }}

            const toast = document.createElement('div');
            toast.style.background = '#1e293b';
            toast.style.border = '1px solid ' + (type === 'error' ? '#ef4444' : type === 'success' ? '#10b981' : 'var(--accent-color)');
            toast.style.color = '#fff';
            toast.style.padding = '0.8rem 1.4rem';
            toast.style.borderRadius = '12px';
            toast.style.fontSize = '0.9rem';
            toast.style.fontWeight = '600';
            toast.style.boxShadow = '0 15px 30px rgba(0,0,0,0.3)';
            toast.style.opacity = '0';
            toast.style.transform = 'translateY(15px)';
            toast.style.transition = 'all 0.3s cubic-bezier(0.16, 1, 0.3, 1)';
            
            toast.textContent = message;
            container.appendChild(toast);
            
            setTimeout(() => {{
                toast.style.opacity = '1';
                toast.style.transform = 'translateY(0)';
            }}, 15);
            
            setTimeout(() => {{
                toast.style.opacity = '0';
                toast.style.transform = 'translateY(-15px)';
                setTimeout(() => {{
                    toast.remove();
                }}, 300);
            }}, 4500);
        }}

        function loadAndPlayTrack() {{
            if (currentTrackIndex < 0 || currentTrackIndex >= playlist.length) return;
            const file = playlist[currentTrackIndex];

            const playerBar = document.getElementById('audio-player-bar');
            playerBar.style.display = 'flex';

            document.getElementById('player-title').textContent = file.title || file.name;
            let metaText = 'Unknown Artist';
            if (file.artist) {{
                metaText = file.artist;
                if (file.album) {{
                    metaText += ' — ' + file.album;
                }}
            }} else if (file.album) {{
                metaText = file.album;
            }}
            document.getElementById('player-subtitle').textContent = metaText;
            document.getElementById('player-track-count').textContent = (currentTrackIndex + 1) + '/' + playlist.length;

            const audioEl = document.getElementById('audio-element');
            audioEl.src = '/media/' + file.id;
            audioEl.play().then(() => {{
                updatePlayPauseUI(true);
            }}).catch(err => {{
                console.error("Audio playback error:", err);
            }});
        }}

        function togglePlayPause() {{
            const audioEl = document.getElementById('audio-element');
            if (audioEl.paused) {{
                audioEl.play();
                updatePlayPauseUI(true);
            }} else {{
                audioEl.pause();
                updatePlayPauseUI(false);
            }}
        }}

        function updatePlayPauseUI(isPlaying) {{
            const playIcon = document.getElementById('play-icon');
            const pauseIcon = document.getElementById('pause-icon');
            if (isPlaying) {{
                playIcon.style.display = 'none';
                pauseIcon.style.display = 'block';
            }} else {{
                playIcon.style.display = 'block';
                pauseIcon.style.display = 'none';
            }}
        }}

        function playNext() {{
            if (playlist.length === 0) return;
            currentTrackIndex = (currentTrackIndex + 1) % playlist.length;
            loadAndPlayTrack();
        }}

        function playPrev() {{
            if (playlist.length === 0) return;
            currentTrackIndex = (currentTrackIndex - 1 + playlist.length) % playlist.length;
            loadAndPlayTrack();
        }}

        function onProgressSeek(percent) {{
            const audioEl = document.getElementById('audio-element');
            if (audioEl.duration) {{
                audioEl.currentTime = (percent / 100) * audioEl.duration;
            }}
        }}

        // Helper function to format time (e.g. 125 -> 2:05)
        function formatPlayerTime(t) {{
            const m = Math.floor(t / 60);
            const s = Math.floor(t % 60).toString().padStart(2, '0');
            return m + ':' + s;
        }}

        function onVolumeChange(vol) {{
            const audioEl = document.getElementById('audio-element');
            audioEl.volume = vol / 100;
        }}

        function closePlayer() {{
            const audioEl = document.getElementById('audio-element');
            audioEl.pause();
            audioEl.src = '';
            document.getElementById('audio-player-bar').style.display = 'none';
        }}

        function createImageCard(file) {{
            const card = document.createElement('div');
            card.className = 'image-card';
            card.onclick = () => openLightbox(file.id);

            card.innerHTML = `
                <img src="/media/${{file.id}}" alt="${{file.name}}" loading="lazy">
                <div class="image-card-overlay">
                    <div class="image-card-name" title="${{file.name}}">${{file.name}}</div>
                    <a href="/media/${{file.id}}" download="${{file.name}}" onclick="event.stopPropagation()" class="image-card-download" title="Download">
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>
                    </a>
                </div>
            `;
            return card;
        }}

        function openLightbox(fileId) {{
            // Construct a list of images matching current browse state
            let filteredImages = filesData.filter(f => f.cat === 'image');
            if (currentPath.length > 0 && searchQuery === '') {{
                filteredImages = filteredImages.filter(f => {{
                    const comps = getRelativeComponents(f.path);
                    if (comps.length <= currentPath.length) return false;
                    for (let i = 0; i < currentPath.length; i++) {{
                        if (comps[i] !== currentPath[i]) return false;
                    }}
                    return true;
                }});
            }} else if (searchQuery !== '') {{
                filteredImages = filteredImages.filter(f => f.name.toLowerCase().includes(searchQuery));
            }}

            // Sort alphabetically
            filteredImages.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));

            imageList = filteredImages;
            currentImageIndex = imageList.findIndex(f => f.id.toString() === fileId.toString());
            if (currentImageIndex === -1) {{
                // fallback
                const file = filesData.find(f => f.id.toString() === fileId.toString());
                if (file) {{
                    imageList = [file];
                    currentImageIndex = 0;
                }} else {{
                    return;
                }}
            }}

            showLightboxImage();
        }}

        function showLightboxImage() {{
            if (currentImageIndex < 0 || currentImageIndex >= imageList.length) return;
            const file = imageList[currentImageIndex];

            const lightbox = document.getElementById('image-lightbox');
            const img = document.getElementById('lightbox-img');
            const title = document.getElementById('lightbox-title');
            const dl = document.getElementById('lightbox-download');

            img.src = '/media/' + file.id;
            title.textContent = file.name;
            dl.href = '/media/' + file.id;
            dl.download = file.name;

            lightbox.style.display = 'flex';

            // Add keyboard navigation event listener if not already added
            document.removeEventListener('keydown', handleLightboxKeydown);
            document.addEventListener('keydown', handleLightboxKeydown);
        }}

        function handleLightboxKeydown(e) {{
            if (e.key === 'ArrowRight') {{
                showNextImage();
            }} else if (e.key === 'ArrowLeft') {{
                showPrevImage();
            }} else if (e.key === 'Escape') {{
                closeLightbox();
            }}
        }}

        function showNextImage() {{
            if (imageList.length === 0) return;
            currentImageIndex = (currentImageIndex + 1) % imageList.length;
            showLightboxImage();
        }}

        function showPrevImage() {{
            if (imageList.length === 0) return;
            currentImageIndex = (currentImageIndex - 1 + imageList.length) % imageList.length;
            showLightboxImage();
        }}

        function closeLightbox() {{
            document.getElementById('image-lightbox').style.display = 'none';
            document.removeEventListener('keydown', handleLightboxKeydown);
        }}

        function formatBytes(bytes) {{
            if (bytes === 0) return '0 B';
            const k = 1024;
            const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
            const i = Math.floor(Math.log(bytes) / Math.log(k));
            return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
        }}

        function updateStatusBadge(isOnline) {{
            const badge = document.getElementById('server-status-badge');
            const dot = document.getElementById('server-status-dot');
            const text = document.getElementById('server-status-text');
            if (!badge || !dot || !text) return;
            if (isOnline) {{
                badge.style.color = '#10b981';
                dot.style.background = '#10b981';
                dot.style.boxShadow = '0 0 8px #10b981';
                text.textContent = 'Online';
            }} else {{
                badge.style.color = '#ef4444';
                dot.style.background = '#ef4444';
                dot.style.boxShadow = '0 0 8px #ef4444';
                text.textContent = 'Offline';
            }}
        }}

        async function checkServerStatus() {{
            try {{
                const res = await fetch('/healthz');
                updateStatusBadge(res.ok);
            }} catch (err) {{
                updateStatusBadge(false);
            }}
        }}

        // Run global heartbeat status check every 5 seconds
        setInterval(checkServerStatus, 5000);

        async function updateMetrics() {{
            try {{
                const res = await fetch('/metrics/json');
                if (!res.ok) {{
                    updateStatusBadge(false);
                    return;
                }}
                const data = await res.json();
                updateStatusBadge(true);
                
                const stats = data.web_handler_metrics;
                const db = data.database_stats;

                // Update Database
                document.getElementById('db-total-files').textContent = db.total_files.toLocaleString();
                document.getElementById('db-total-size').textContent = formatBytes(db.total_size_bytes);
                document.getElementById('db-file-size').textContent = formatBytes(db.database_size_bytes);
                
                document.getElementById('db-video-count').textContent = db.video_files.toLocaleString();
                document.getElementById('db-audio-count').textContent = db.audio_files.toLocaleString();
                document.getElementById('db-image-count').textContent = db.image_files.toLocaleString();
                document.getElementById('db-playlist-count').textContent = db.playlists.toLocaleString();

                // Update Web Traffic
                document.getElementById('web-gigabytes').textContent = stats.gigabytes_transferred.toFixed(3) + ' GB';
                document.getElementById('web-file-serves').textContent = stats.file_serves.toLocaleString();
                document.getElementById('web-dir-listings').textContent = stats.directory_listings.toLocaleString();

                // Calculate network usage speed in Mbps
                const currentBytes = stats.gigabytes_transferred * 1073741824;
                const currentTime = Date.now();
                if (lastBytes !== null && lastTime !== null) {{
                    const deltaBytes = currentBytes - lastBytes;
                    const deltaTimeSeconds = (currentTime - lastTime) / 1000.0;
                    if (deltaTimeSeconds > 0) {{
                        const speedBps = (deltaBytes * 8) / deltaTimeSeconds;
                        const speedMbps = speedBps / 1000000.0;
                        document.getElementById('web-speed').textContent = Math.round(speedMbps) + ' Mbps';
                    }}
                }} else {{
                    document.getElementById('web-speed').textContent = '0 Mbps';
                }}
                lastBytes = currentBytes;
                lastTime = currentTime;

                // Update Cache
                document.getElementById('web-cache-rate').textContent = stats.cache_hit_rate_percent.toFixed(1) + '%';
                document.getElementById('web-cache-hits').textContent = stats.cache_hits.toLocaleString();
                document.getElementById('web-cache-misses').textContent = stats.cache_misses.toLocaleString();

                // Update Server Health
                document.getElementById('web-response-time').textContent = stats.average_response_time_ms.toFixed(2) + ' ms';
                document.getElementById('web-errors').textContent = stats.errors.toLocaleString();

                // Update Active TV Casts
                const activeCastsContainer = document.getElementById('active-tv-casts-container');
                if (activeCastsContainer) {{
                    const casts = data.active_casts || {{}};
                    const castKeys = Object.keys(casts);
                    
                    if (castKeys.length === 0) {{
                        activeCastsContainer.innerHTML = '<div style="color: var(--text-secondary); font-style: italic; padding: 0.25rem 0;">No active TV streams.</div>';
                    }} else {{
                        let castsHtml = '';
                        castKeys.forEach(tv => {{
                            castsHtml += `
                                <div style="display: flex; align-items: center; justify-content: space-between; padding: 0.6rem 0.85rem; background: rgba(255,255,255,0.02); border: 1px solid var(--card-border); border-radius: 10px; margin-bottom: 0.25rem;">
                                    <div style="display: flex; align-items: center; gap: 0.65rem; font-weight: 600; color: var(--text-primary); min-width: 0; flex: 1;">
                                        <span style="display: inline-block; width: 6px; height: 6px; background: var(--accent-color); border-radius: 50%; box-shadow: 0 0 6px var(--accent-color); flex-shrink: 0;"></span>
                                        <span style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">${{tv}}</span>
                                    </div>
                                    <div style="color: var(--accent-color); font-weight: 600; text-align: right; max-width: 65%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-left: 0.75rem;" title="${{casts[tv]}}">
                                        ${{casts[tv]}}
                                    </div>
                                </div>
                            `;
                        }});
                        activeCastsContainer.innerHTML = castsHtml;
                    }}
                }}

            }} catch (err) {{
                console.error("Failed to fetch metrics:", err);
                updateStatusBadge(false);
            }}
        }}

        const filesData = JSON.parse(document.getElementById('files-data').textContent);
        const monitoredDirs = JSON.parse(document.getElementById('dirs-data').textContent);

        let currentTab = 'video'; // Videos active by default!
        let currentPath = [];
        let searchQuery = '';

        function setTab(tab) {{
            currentTab = tab;
            document.querySelectorAll('.tab-btn').forEach(btn => {{
                if (btn.dataset.tab === tab) {{
                    btn.classList.add('active');
                }} else {{
                    btn.classList.remove('active');
                }}
            }});
            currentPath = [];
            render();
        }}

        function onSearch() {{
            searchQuery = document.getElementById('search-input').value.toLowerCase();
            render();
        }}

        function playMedia(id) {{
            window.location.href = '/media/' + id;
        }}

        function getRelativeComponents(filePath) {{
            let path = filePath.replace(/\\/g, '/');
            for (const dir of monitoredDirs) {{
                let dirNorm = dir.replace(/\\/g, '/');
                if (!dirNorm.endsWith('/')) {{
                    dirNorm += '/';
                }}
                if (path.startsWith(dirNorm)) {{
                    return path.substring(dirNorm.length).split('/').filter(p => p.length > 0);
                }}
            }}
            const parts = path.split('/').filter(p => p.length > 0);
            return parts.slice(-1);
        }}

        function render() {{
            const fileListContainer = document.getElementById('file-list');
            fileListContainer.innerHTML = '';

            if (currentTab === 'image') {{
                fileListContainer.className = 'image-grid';
            }} else {{
                fileListContainer.className = 'file-list';
            }}

            // Filter files by tab and search
            let filteredFiles = filesData.filter(file => {{
                if (currentTab === 'radio') {{
                    return file.cat === 'radio' && (searchQuery === '' || file.name.toLowerCase().includes(searchQuery));
                }}
                if (file.cat === 'radio') return false;
                
                const matchesTab = currentTab === 'all' || file.cat === currentTab;
                const matchesSearch = searchQuery === '' || file.name.toLowerCase().includes(searchQuery);
                return matchesTab && matchesSearch;
            }});

            // For radio tab, render a flat list of radio stations directly
            if (currentTab === 'radio') {{
                document.getElementById('breadcrumbs').innerHTML = '<span>Internet Radio Stations</span>';
                if (filteredFiles.length === 0) {{
                    renderEmptyState("No radio stations configured.");
                    return;
                }}
                filteredFiles.forEach(file => {{
                    fileListContainer.appendChild(createFileCard(file));
                }});
                return;
            }}

            // If searching, show a flat search results view
            if (searchQuery !== '') {{
                document.getElementById('breadcrumbs').innerHTML = '<span>Search Results</span>';
                
                if (filteredFiles.length === 0) {{
                    renderEmptyState("No matching files found.");
                    return;
                }}
                
                filteredFiles.forEach(file => {{
                    if (currentTab === 'image' && file.cat === 'image') {{
                        fileListContainer.appendChild(createImageCard(file));
                    }} else {{
                        fileListContainer.appendChild(createFileCard(file));
                    }}
                }});
                return;
            }}

            // Build directory tree
            const tree = {{ folders: {{}}, files: [] }};
            
            filteredFiles.forEach(file => {{
                const components = getRelativeComponents(file.path);
                let curr = tree;
                
                for (let i = 0; i < components.length - 1; i++) {{
                    const folderName = components[i];
                    if (!curr.folders[folderName]) {{
                        curr.folders[folderName] = {{ folders: {{}}, files: [] }};
                    }}
                    curr = curr.folders[folderName];
                }}
                
                curr.files.push(file);
            }});

            // Navigate tree to currentPath
            let activeNode = tree;
            for (const folder of currentPath) {{
                if (activeNode.folders[folder]) {{
                    activeNode = activeNode.folders[folder];
                }} else {{
                    currentPath = [];
                    activeNode = tree;
                    break;
                }}
            }}

            renderBreadcrumbs();

            // Parent Folder row
            if (currentPath.length > 0) {{
                const parentCard = document.createElement('div');
                parentCard.className = 'media-card';
                parentCard.style.cursor = 'pointer';
                parentCard.onclick = goBack;
                parentCard.innerHTML = `
                    <div class="media-info">
                        <div class="media-icon-wrapper">
                            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"></polyline></svg>
                        </div>
                        <div class="media-details">
                            <div class="media-name">..</div>
                            <div class="media-meta">Parent Directory</div>
                        </div>
                    </div>
                `;
                fileListContainer.appendChild(parentCard);
            }}

            // Render Subfolders
            const sortedFolders = Object.keys(activeNode.folders).sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()));
            sortedFolders.forEach(folderName => {{
                const folderCard = document.createElement('div');
                folderCard.className = 'media-card folder-card';
                folderCard.style.cursor = 'pointer';
                folderCard.onclick = () => enterFolder(folderName);

                let actionAreaHtml = '';
                if (currentTab === 'audio' || currentTab === 'all') {{
                    actionAreaHtml = `
                        <div class="action-area" onclick="event.stopPropagation(); playFolder('${{folderName}}')">
                            <button class="btn-action" title="Play Folder Content">
                                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>
                            </button>
                        </div>
                    `;
                }}

                // Calculate video files in this folder to see if "Play on TV" is applicable
                const targetPath = [...currentPath, folderName];
                const folderVideos = filesData.filter(file => {{
                    if (file.cat !== 'video') return false;
                    const comps = getRelativeComponents(file.path);
                    if (comps.length <= targetPath.length) return false;
                    for (let i = 0; i < targetPath.length; i++) {{
                        if (comps[i] !== targetPath[i]) return false;
                    }}
                    return true;
                }});

                let tvActionHtml = '';
                if ((currentTab === 'video' || currentTab === 'all') && folderVideos.length > 0) {{
                    tvActionHtml = `
                        <div class="action-area" onclick="event.stopPropagation(); playVideoFolderOnTv('${{folderName}}')">
                            <button class="btn-action" title="Play on TV" style="color: var(--accent-color);">
                                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect><line x1="8" y1="21" x2="16" y2="21"></line><line x1="12" y1="17" x2="12" y2="21"></line></svg>
                            </button>
                        </div>
                    `;
                }}

                folderCard.innerHTML = `
                    <div class="media-info">
                        <div class="media-icon-wrapper">
                            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>
                        </div>
                        <div class="media-details">
                            <div class="media-name" title="${{folderName}}">${{folderName}}</div>
                            <div class="media-meta">Folder</div>
                        </div>
                    </div>
                    <div style="display: flex; gap: 0.5rem;">
                        ${{actionAreaHtml}}
                        ${{tvActionHtml}}
                    </div>
                `;
                fileListContainer.appendChild(folderCard);
            }});

            // Render Files
            const sortedFiles = activeNode.files.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
            sortedFiles.forEach(file => {{
                if (currentTab === 'image' && file.cat === 'image') {{
                    fileListContainer.appendChild(createImageCard(file));
                }} else {{
                    fileListContainer.appendChild(createFileCard(file));
                }}
            }});

            if (sortedFolders.length === 0 && sortedFiles.length === 0) {{
                renderEmptyState("This folder contains no items matching the active filter.");
            }}
        }}

        function createFileCard(file) {{
            const card = document.createElement('div');
            card.className = 'media-card';
            card.style.cursor = 'pointer';
            if (file.cat === 'audio' || file.cat === 'radio') {{
                card.onclick = () => playAudioFile(file);
            }} else if (file.cat === 'image') {{
                card.onclick = () => openLightbox(file.id);
            }} else {{
                card.onclick = () => playMedia(file.id);
            }}
            
            let iconSvg = '';
            if (file.cat === 'video') {{
                iconSvg = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="23 7 16 12 23 17 23 7"></polygon><rect x="1" y="5" width="15" height="14" rx="2" ry="2"></rect></svg>`;
            }} else if (file.cat === 'audio') {{
                iconSvg = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l12-2v13"></path><circle cx="6" cy="18" r="3"></circle><circle cx="18" cy="16" r="3"></circle></svg>`;
            }} else if (file.cat === 'radio') {{
                iconSvg = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--accent-color)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="2"></circle><path d="M16.24 7.76a6 6 0 0 1 0 8.49m-8.48-.01a6 6 0 0 1 0-8.49m11.31-2.82a10 10 0 0 1 0 14.14m-14.14 0a10 10 0 0 1 0-14.14"></path></svg>`;
            }} else if (file.cat === 'image') {{
                iconSvg = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`;
            }} else {{
                iconSvg = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline></svg>`;
            }}

            let detailsHtml = `<div class="media-name" title="${{file.name}}">${{file.title || file.name}}</div>`;
            if (file.artist || file.album) {{
                let metaParts = [];
                if (file.artist) {{ metaParts.push(file.artist); }}
                if (file.album) {{ metaParts.push(file.album); }}
                detailsHtml += `<div class="media-artist-album" style="font-size: 0.8rem; color: var(--text-secondary); margin-top: 0.1rem;">${{metaParts.join(' &mdash; ')}}</div>`;
            }}

            let playBtnHtml = '';
            if (file.cat === 'audio' || file.cat === 'radio') {{
                playBtnHtml = `
                    <button class="btn-action" onclick="event.stopPropagation(); playAudioFile('${{file.id}}')" title="Play File" style="margin-right: 0.35rem;">
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>
                    </button>
                `;
            }}

            card.innerHTML = `
                <div class="media-info">
                    <div class="media-icon-wrapper">
                        ${{iconSvg}}
                    </div>
                    <div class="media-details">
                        ${{detailsHtml}}
                        <div class="media-meta" style="margin-top: 0.25rem;">
                            <span>${{file.size_str}}</span>
                            <span class="media-meta-dot"></span>
                            <span style="text-transform: uppercase;">${{file.ext}}</span>
                        </div>
                    </div>
                </div>
                <div class="action-area">
                    ${{playBtnHtml}}
                    <a href="/media/${{file.id}}" download="${{file.name}}" onclick="event.stopPropagation()" class="btn-action" title="Download File">
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>
                    </a>
                </div>
            `;
            return card;
        }}

        function renderBreadcrumbs() {{
            const container = document.getElementById('breadcrumbs');
            container.innerHTML = '';
            
            const rootSpan = document.createElement('span');
            rootSpan.className = 'breadcrumb-item';
            rootSpan.onclick = () => jumpToBreadcrumb(-1);
            rootSpan.textContent = 'Home';
            container.appendChild(rootSpan);
            
            currentPath.forEach((folder, idx) => {{
                const separator = document.createElement('span');
                separator.className = 'breadcrumb-separator';
                separator.textContent = ' / ';
                container.appendChild(separator);
                
                const folderSpan = document.createElement('span');
                folderSpan.className = 'breadcrumb-item';
                folderSpan.onclick = () => jumpToBreadcrumb(idx);
                folderSpan.textContent = folder;
                container.appendChild(folderSpan);
            }});
        }}

        function jumpToBreadcrumb(idx) {{
            currentPath = currentPath.slice(0, idx + 1);
            render();
        }}

        function enterFolder(folderName) {{
            currentPath.push(folderName);
            render();
        }}

        function goBack() {{
            currentPath.pop();
            render();
        }}

        function renderEmptyState(message) {{
            const container = document.getElementById('file-list');
            container.innerHTML = `
                <div class="empty-state">
                    <svg class="empty-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="8" x2="12" y2="12"></line><line x1="12" y1="16" x2="12.01" y2="16"></line></svg>
                    <h3>No items found</h3>
                    <p>${{message}}</p>
                </div>
            `;
        }}

        // Setup Audio Element event listeners
        const audioEl = document.getElementById('audio-element');
        const slider = document.getElementById('player-progress-slider');
        const currentEl = document.getElementById('player-time-current');
        const durationEl = document.getElementById('player-time-duration');

        audioEl.addEventListener('timeupdate', () => {{
            if (audioEl.duration) {{
                const curTime = audioEl.currentTime;
                const durTime = audioEl.duration;
                slider.value = (curTime / durTime) * 100;
                currentEl.textContent = formatPlayerTime(curTime);
                durationEl.textContent = formatPlayerTime(durTime);
            }}
        }});

        audioEl.addEventListener('ended', () => {{
            playNext();
        }});

        // Initial render
        render();
    </script>
</body>
</html>"##,
    );

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

pub async fn description_handler(State(state): State<AppState>) -> impl IntoResponse {
    let xml = generate_description_xml(&state).await;
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        xml,
    )
}

pub async fn content_directory_scpd() -> impl IntoResponse {
    let xml = generate_scpd_xml();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        xml,
    )
}

/// Extracts browse parameters from SOAP request
#[derive(Debug, Clone)]
struct BrowseParams {
    object_id: String,
    starting_index: u32,
    requested_count: u32,
}

fn parse_browse_params(body: &str) -> BrowseParams {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);
    
    let mut object_id = "0".to_string();
    let mut starting_index = 0u32;
    let mut requested_count = 0u32;
    
    let mut buf = Vec::new();
    let mut current_element = String::new();
    
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                current_element = String::from_utf8_lossy(e.name().as_ref()).to_string();
            }
            Ok(Event::Text(ref e)) => {
                let text = reader.decoder().decode(e.as_ref()).unwrap_or_default();
                match current_element.as_str() {
                    "ObjectID" => {
                        object_id = text.trim().to_string();
                        if object_id.is_empty() {
                            object_id = "0".to_string();
                        }
                    }
                    "StartingIndex" => {
                        starting_index = text.trim().parse().unwrap_or_else(|e| {
                            warn!("Failed to parse StartingIndex '{}': {}", text, e);
                            0
                        });
                    }
                    "RequestedCount" => {
                        requested_count = text.trim().parse().unwrap_or_else(|e| {
                            warn!("Failed to parse RequestedCount '{}': {}", text, e);
                            0
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("Error parsing XML: {}, falling back to defaults", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }
    
    debug!("Parsed browse params - ObjectID: '{}', StartingIndex: {}, RequestedCount: {}", 
           object_id, starting_index, requested_count);
    
    BrowseParams {
        object_id,
        starting_index,
        requested_count,
    }
}

/// Content Directory Handler struct to encapsulate specialized browse handlers
pub struct ContentDirectoryHandler;

impl ContentDirectoryHandler {
    /// Handle video browse requests
    async fn handle_video_browse(
        params: &BrowseParams,
        state: &AppState,
        path_prefix_str: &str,
    ) -> Response {
        Self::handle_folder_browse(params, state, "video/", path_prefix_str).await
    }

    /// Handle music browse requests (folder-based, not categorized)
    async fn handle_music_browse(
        params: &BrowseParams,
        state: &AppState,
        path_prefix_str: &str,
    ) -> Response {
        Self::handle_folder_browse(params, state, "audio/", path_prefix_str).await
    }

    /// Handle image browse requests
    async fn handle_image_browse(
        params: &BrowseParams,
        state: &AppState,
        path_prefix_str: &str,
    ) -> Response {
        Self::handle_folder_browse(params, state, "image/", path_prefix_str).await
    }


    /// Handle generic folder-based browse requests with consistent path normalization
    /// Enhanced with atomic performance tracking and cache-friendly operations
    async fn handle_folder_browse(
        params: &BrowseParams,
        state: &AppState,
        media_type_filter: &str,
        path_prefix_str: &str,
    ) -> Response {
        use crate::web::xml::generate_browse_response;

        let start_time = Instant::now();
        
        let client = crate::web::client::CURRENT_CLIENT.try_with(|c| *c)
            .unwrap_or(crate::web::client::DlnaClientProfile::Standard);
        
        let current_update_id = state.content_update_id.load(Ordering::Relaxed);
        let cache_key = crate::state::SoapCacheKey {
            object_id: params.object_id.clone(),
            starting_index: params.starting_index,
            requested_count: params.requested_count,
            client_profile: client,
            content_update_id: current_update_id,
        };

        // Cache lookup
        {
            let mut cache = state.browse_cache.lock().await;
            let needs_clear = cache.keys().next().map(|k| k.content_update_id != current_update_id).unwrap_or(false);
            if needs_clear {
                cache.clear();
            }
            if let Some(cached_xml) = cache.get(&cache_key) {
                let response_time = start_time.elapsed().as_micros() as u64;
                state.web_metrics.record_browse_request(response_time, true);
                state.web_metrics.record_directory_listing(response_time);
                debug!("Browse Cache Hit for Folder ObjectID: {} ({}ms)", params.object_id, response_time);
                return (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                        (header::HeaderName::from_static("ext"), ""),
                    ],
                    cached_xml.clone(),
                ).into_response();
            }
        }

        let cache_hit = false;

        let monitored_dirs = &state.config.media.directories;
        
        // Parse directory index prefix (e.g. "d0/movies" -> index 0, relative path "movies")
        let (dir_index_opt, relative_path) = parse_dir_index_prefix(path_prefix_str);

        let browse_path = match dir_index_opt {
            Some(idx) if idx < monitored_dirs.len() => {
                let base_path = PathBuf::from(&monitored_dirs[idx].path);
                if relative_path.is_empty() {
                    base_path
                } else {
                    base_path.join(relative_path)
                }
            }
            _ => {
                let media_root = state.config.get_primary_media_dir();
                if path_prefix_str.is_empty() {
                    media_root
                } else {
                    media_root.join(path_prefix_str)
                }
            }
        };

        // If there are multiple monitored directories and we are at the root, return virtual folders
        let (subdirectories, files) = if path_prefix_str.is_empty() && monitored_dirs.len() > 1 {
            let mut subdirs = Vec::new();
            for (idx, dir) in monitored_dirs.iter().enumerate() {
                let path = PathBuf::from(&dir.path);
                let name = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| dir.path.clone());
                subdirs.push(MediaDirectory {
                    path: PathBuf::from(format!("d{}", idx)),
                    name,
                });
            }
            (subdirs, Vec::new())
        } else {
            // Apply canonical path normalization to match how paths are stored in the database
            let canonical_browse_path = match state.filesystem_manager.get_canonical_path(&browse_path) {
                Ok(canonical) => std::path::PathBuf::from(canonical),
                Err(e) => {
                    warn!("Failed to get canonical path for browse request '{}': {}, using basic normalization", browse_path.display(), e);
                    state.web_metrics.record_error();
                    state.filesystem_manager.normalize_path(&browse_path)
                }
            };
            
            // Query the ReDB database for the directory listing
            let query_future = state.database.get_directory_listing(&canonical_browse_path, media_type_filter);
            let timeout_duration = std::time::Duration::from_secs(30); // 30 second timeout
            
            match tokio::time::timeout(timeout_duration, query_future).await {
                Ok(Ok(res)) => res,
                Ok(Err(e)) => {
                    error!("ReDB database error getting directory listing for {}: {}", params.object_id, e);
                    state.web_metrics.record_error();
                    let response_time = start_time.elapsed().as_micros() as u64;
                    state.web_metrics.record_browse_request(response_time, false);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Error browsing content".to_string(),
                    ).into_response();
                }
                Err(_) => {
                    error!("Database query timed out for {}", params.object_id);
                    state.web_metrics.record_error();
                    let response_time = start_time.elapsed().as_micros() as u64;
                    state.web_metrics.record_browse_request(response_time, false);
                    return (
                        StatusCode::REQUEST_TIMEOUT,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Request timeout - directory too large".to_string(),
                    ).into_response();
                }
            }
        };


        
        debug!("ReDB browse request for '{}' (filter: '{}') returned {} subdirs, {} files", 
               browse_path.display(), media_type_filter, subdirectories.len(), files.len());
               
        // Apply pagination if requested
        let starting_index = params.starting_index as usize;
        let requested_count = if params.requested_count == 0 { 
            // If RequestedCount is 0, return all items (but limit to prevent hanging)
            2000 
        } else { 
            std::cmp::min(params.requested_count as usize, 2000) 
        };
        
        // Combine directories and files for pagination with atomic counting
        let mut all_items = Vec::new();
        for subdir in &subdirectories {
            all_items.push((subdir.clone(), None));
        }
        for file in &files {
            all_items.push((MediaDirectory { path: file.path.clone(), name: String::new() }, Some(file.clone())));
        }
        
        let total_matches = all_items.len();
        let end_index = std::cmp::min(starting_index + requested_count, total_matches);
        
        if starting_index >= total_matches {
            // Starting index is beyond available items - record metrics and return empty
            let response_time = start_time.elapsed().as_micros() as u64;
            state.web_metrics.record_browse_request(response_time, cache_hit);
            
            let server_ip = state.get_server_ip();
            let response = generate_browse_response(&params.object_id, &[], &[], state, &server_ip).await;
            return (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                    (header::HeaderName::from_static("ext"), ""),
                ],
                response,
            )
                .into_response();
        }
        
        // Extract paginated items with zero-copy operations
        let paginated_items = &all_items[starting_index..end_index];
        let mut paginated_subdirs = Vec::new();
        let mut paginated_files = Vec::new();
        
        for (item, file_opt) in paginated_items {
            if let Some(file) = file_opt {
                paginated_files.push(file.clone());
            } else {
                paginated_subdirs.push(item.clone());
            }
        }
        
        debug!("ReDB returning paginated results: {} subdirs, {} files (index {}-{} of {})",
               paginated_subdirs.len(), paginated_files.len(), 
               starting_index, end_index, total_matches);
        
        // Record atomic performance metrics
        let response_time = start_time.elapsed().as_micros() as u64;
        state.web_metrics.record_browse_request(response_time, cache_hit);
        state.web_metrics.record_directory_listing(response_time);
        
        let server_ip = state.get_server_ip();
        let response = generate_browse_response(&params.object_id, &paginated_subdirs, &paginated_files, state, &server_ip).await;
        
        // Cache insert
        {
            let mut cache = state.browse_cache.lock().await;
            let needs_clear = cache.keys().next().map(|k| k.content_update_id != current_update_id).unwrap_or(false);
            if needs_clear {
                cache.clear();
            }
            cache.insert(cache_key, response.clone());
        }

        (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                (header::HeaderName::from_static("ext"), ""),
            ],
            response,
        )
            .into_response()
    }

    /// Handle root browse request (ObjectID "0")
    async fn handle_root_browse(_params: &BrowseParams, state: &AppState) -> Response {
        use crate::web::xml::generate_browse_response;
        
        // For the root, we typically return the top-level containers (Video, Audio, Image).
        // The generate_browse_response function should be smart enough to create these
        // when given an object_id of "0" and empty lists of subdirectories and files.
        let server_ip = state.get_server_ip();
        let response = generate_browse_response("0", &[], &[], state, &server_ip).await;
        (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                (header::HeaderName::from_static("ext"), ""),
            ],
            response,
        )
            .into_response()
    }

    /// Handle radio browse request
    async fn handle_radio_browse(_params: &BrowseParams, state: &AppState) -> Response {
        use crate::web::xml::generate_browse_response;
        use futures_util::StreamExt;
        
        let mut stream = state.database.stream_all_media_files();
        let mut radio_files = Vec::new();
        while let Some(res) = stream.next().await {
            if let Ok(file) = res {
                if file.mime_type == "audio/radio" {
                    radio_files.push(file);
                }
            }
        }
        
        let server_ip = state.get_server_ip();
        let response = generate_browse_response("radio", &[], &radio_files, state, &server_ip).await;
        (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                (header::HeaderName::from_static("ext"), ""),
            ],
            response,
        )
            .into_response()
    }
}

pub async fn content_directory_control(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let client = crate::web::client::detect_client(&headers);
    crate::web::client::CURRENT_CLIENT.scope(client, async move {
        if body.contains("<u:Browse") {
            let params = parse_browse_params(&body);
            info!("Browse request - ObjectID: {}, StartingIndex: {}, RequestedCount: {}", 
                  params.object_id, params.starting_index, params.requested_count);

            // Handle root browse request (ObjectID "0")
            if params.object_id == "0" {
                return ContentDirectoryHandler::handle_root_browse(&params, &state).await;
            }

            // Determine media type and delegate to specialized handlers
            if params.object_id.starts_with("video") {
                let path_prefix_str = params.object_id.strip_prefix("video").unwrap_or("").trim_start_matches('/');
                return ContentDirectoryHandler::handle_video_browse(&params, &state, path_prefix_str).await;
            } else if params.object_id.starts_with("audio") {
                // Handle music categorization within audio section
                let audio_path = params.object_id.strip_prefix("audio").unwrap_or("").trim_start_matches('/');
                
                // Check for music categorization paths
                if audio_path.is_empty() {
                    // Root audio container - return categorization containers
                    return handle_audio_root_browse(&params, &state).await;
                } else if audio_path.starts_with("artists") {
                    return ContentDirectoryHandler::handle_artist_browse(&params, &state, audio_path).await;
                } else if audio_path.starts_with("albums") {
                    return ContentDirectoryHandler::handle_album_browse(&params, &state, audio_path).await;
                } else if audio_path.starts_with("genres") {
                    return handle_genres_browse(&params, &state, audio_path).await;
                } else if audio_path.starts_with("years") {
                    return handle_years_browse(&params, &state, audio_path).await;
                } else if audio_path.starts_with("playlists") {
                    return handle_playlists_browse(&params, &state, audio_path).await;
                } else if audio_path.starts_with("folders") {
                    let folder_path = audio_path.strip_prefix("folders").unwrap_or("").trim_start_matches('/');
                    return ContentDirectoryHandler::handle_music_browse(&params, &state, folder_path).await;
                } else {
                    // Traditional folder browsing within audio
                    return ContentDirectoryHandler::handle_music_browse(&params, &state, audio_path).await;
                }
            } else if params.object_id.starts_with("image") {
                let path_prefix_str = params.object_id.strip_prefix("image").unwrap_or("").trim_start_matches('/');
                return ContentDirectoryHandler::handle_image_browse(&params, &state, path_prefix_str).await;
            } else if params.object_id.starts_with("radio") {
                return ContentDirectoryHandler::handle_radio_browse(&params, &state).await;
            } else {
                // This case might happen for deeper browsing or custom object IDs.
                // Assume no specific type filter for the database query, and the object_id itself
                // represents the path relative to the media root.
                return ContentDirectoryHandler::handle_folder_browse(&params, &state, "", params.object_id.as_str()).await;
            }
        } else if body.contains("<u:GetSearchCapabilities") {
            let content = "<SearchCaps>dc:creator,dc:date,dc:title,upnp:album,upnp:actor,upnp:artist,upnp:class,upnp:genre,@refID</SearchCaps>";
            build_soap_response("GetSearchCapabilities", "urn:schemas-upnp-org:service:ContentDirectory:1", content)
        } else if body.contains("<u:GetSortCapabilities") {
            let content = "<SortCaps>dc:title,dc:date,upnp:class,upnp:album,upnp:originalTrackNumber</SortCaps>";
            build_soap_response("GetSortCapabilities", "urn:schemas-upnp-org:service:ContentDirectory:1", content)
        } else if body.contains("<u:GetSystemUpdateID") {
            let update_id = state.content_update_id.load(Ordering::Relaxed);
            let content = format!("<Id>{}</Id>", update_id);
            build_soap_response("GetSystemUpdateID", "urn:schemas-upnp-org:service:ContentDirectory:1", &content)
        } else if body.contains("<u:X_GetFeatureList") {
            let content = r#"<FeatureList>&lt;?xml version="1.0" encoding="utf-8"?&gt;&lt;Features xmlns="urn:schemas-upnp-org:av:avs" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="urn:schemas-upnp-org:av:avs http://www.upnp.org/schemas/av/avs.xsd"&gt;&lt;Feature name="samsung.com_BASICVIEW" version="1"&gt;&lt;container id="1" type="object.item.audioItem"/&gt;&lt;container id="2" type="object.item.videoItem"/&gt;&lt;container id="3" type="object.item.imageItem"/&gt;&lt;/Feature&gt;&lt;/Features&gt;</FeatureList>"#;
            build_soap_response("X_GetFeatureList", "urn:schemas-upnp-org:service:ContentDirectory:1", content)
        } else if body.contains("<u:X_SetBookmark") {
            let object_id = body.split("<ObjectID>").nth(1).and_then(|s| s.split("</ObjectID>").next()).unwrap_or("");
            let pos_second_str = body.split("<PosSecond>").nth(1).and_then(|s| s.split("</PosSecond>").next()).unwrap_or("");
            if let (Ok(file_id), Ok(pos)) = (object_id.parse::<i64>(), pos_second_str.parse::<u32>()) {
                let mut bookmarks_guard = state.bookmarks.lock().await;
                bookmarks_guard.insert(file_id, pos);
            }
            build_soap_response("X_SetBookmark", "urn:schemas-upnp-org:service:ContentDirectory:1", "")
        } else {
            (
                StatusCode::NOT_IMPLEMENTED,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "Not implemented".to_string(),
            )
                .into_response()
        }
    }).await
}

pub struct MetricsTrackingReader<R> {
    inner: R,
    metrics: std::sync::Arc<WebHandlerMetrics>,
}

impl<R: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for MetricsTrackingReader<R> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        match std::pin::Pin::new(&mut self.inner).poll_read(cx, buf) {
            std::task::Poll::Ready(Ok(())) => {
                let after = buf.filled().len();
                let bytes_read = after - before;
                if bytes_read > 0 {
                    self.metrics.bytes_transferred.fetch_add(bytes_read as u64, Ordering::Relaxed);
                }
                std::task::Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

pub async fn serve_media(
    State(state): State<AppState>,
    axum::extract::ConnectInfo(client_addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Path(id): Path<String>,
    method: Method,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let start_time = Instant::now();
    
    let file_id = id.parse::<i64>().map_err(|_| {
        state.web_metrics.record_error();
        AppError::NotFound
    })?;
    
    // Use ReDB database with atomic cache lookup
    let file_info = state.database
        .get_file_by_id(file_id)
        .await
        .map_err(|e| {
            error!("ReDB database error getting file by ID {}: {}", file_id, e);
            state.web_metrics.record_error();
            AppError::NotFound
        })?
        .ok_or_else(|| {
            debug!("ReDB database: file ID {} not found", file_id);
            state.web_metrics.record_error();
            AppError::NotFound
        })?;

    if file_info.mime_type == "audio/radio" {
        return Ok(axum::response::Redirect::temporary(&file_info.path.to_string_lossy().to_string()).into_response());
    }

    // Record dynamic client telemetry for GET requests (playing)
    if method == Method::GET {
        let client_ip = client_addr.ip().to_string();
        
        let device_name = {
            let cache = state.discovered_tvs.lock().await;
            if let Some(name) = cache.get(&client_ip) {
                name.clone()
            } else if let Some(ua) = headers.get(axum::http::header::USER_AGENT).and_then(|h| h.to_str().ok()) {
                let ua_lower = ua.to_lowercase();
                if ua_lower.contains("ipad") {
                    format!("iPad ({})", client_ip)
                } else if ua_lower.contains("iphone") {
                    format!("iPhone ({})", client_ip)
                } else if ua_lower.contains("android") {
                    format!("Android ({})", client_ip)
                } else if ua_lower.contains("macintosh") || ua_lower.contains("mac os x") {
                    format!("Mac ({})", client_ip)
                } else if ua_lower.contains("windows") {
                    format!("Windows PC ({})", client_ip)
                } else {
                    format!("Device ({})", client_ip)
                }
            } else {
                format!("Device ({})", client_ip)
            }
        };

        {
            let mut casts = state.active_casts.lock().await;
            casts.insert(device_name, (file_info.filename.clone(), std::time::Instant::now()));
        }
    }

    // Enforce read-only access to media files
    let mut file = tokio::fs::OpenOptions::new()
        .read(true)
        .write(false)
        .open(&file_info.path)
        .await
        .map_err(AppError::Io)?;

    // Use actual file size from disk to avoid stale DB values causing range mismatches
    let metadata = file.metadata().await.map_err(AppError::Io)?;
    let file_size = metadata.len();

    let client = crate::web::client::detect_client(&headers);

    let mime_override = match client {
        crate::web::client::DlnaClientProfile::SamsungTv | crate::web::client::DlnaClientProfile::SamsungTvQ if file_info.mime_type == "video/x-matroska" => {
            "video/x-mkv".to_string()
        }
        crate::web::client::DlnaClientProfile::SamsungTv | crate::web::client::DlnaClientProfile::SamsungTvQ if file_info.mime_type == "video/x-msvideo" => {
            "video/mpeg".to_string()
        }
        crate::web::client::DlnaClientProfile::SonyBdp if file_info.mime_type == "video/x-matroska" || file_info.mime_type == "video/mpeg" => {
            "video/divx".to_string()
        }
        crate::web::client::DlnaClientProfile::Xbox if file_info.mime_type == "video/x-msvideo" => {
            "video/avi".to_string()
        }
        _ => file_info.mime_type.clone(),
    };

    let encoded_filename = percent_encoding::utf8_percent_encode(&file_info.filename, percent_encoding::NON_ALPHANUMERIC).to_string();
    let content_disposition = format!("inline; filename=\"{}\"; filename*=UTF-8''{}", file_info.filename.replace('"', "\\\""), encoded_filename);

    let mut response_builder = Response::builder()
        .header(header::CONTENT_TYPE, &mime_override)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_DISPOSITION, &content_disposition)
        .header("transferMode.dlna.org", "Streaming")
        .header("contentFeatures.dlna.org", "DLNA.ORG_OP=11;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000");

    // CaptionInfo.sec injection for Samsung TVs when subtitles exist
    if let Some(caption_req) = headers.get("getcaptioninfo.sec").and_then(|h| h.to_str().ok()) {
        if caption_req == "1" {
            let srt_path = std::path::PathBuf::from(&file_info.path).with_extension("srt");
            if srt_path.exists() {
                let server_ip = headers.get(header::HOST)
                    .and_then(|h| h.to_str().ok())
                    .and_then(|h| h.split(':').next())
                    .unwrap_or("127.0.0.1");
                let srt_url = format!(
                    "http://{}:{}/media/{}/subtitle",
                    server_ip,
                    state.config.server.port,
                    file_id
                );
                debug!("Injecting Samsung subtitle header CaptionInfo.sec: {}", srt_url);
                response_builder = response_builder.header("CaptionInfo.sec", srt_url);
            }
        }
    }

    let (start, end, is_range_request) = if let Some(range_header) = headers.get(header::RANGE) {
        let range_str = range_header.to_str().map_err(|_| AppError::InvalidRange)?;
        debug!("Received range request: {}", range_str);
        
        if file_size == 0 {
            return Err(AppError::InvalidRange);
        }
        
        // Parse the range header manually to avoid enum variant issues
        let (start, end) = parse_range_header(range_str, file_size)?;
        (start, end, true)
    } else {
        // No range requested, serve the whole file
        (0, file_size.saturating_sub(1), false)
    };

    let len = if file_size == 0 {
        0
    } else {
        end - start + 1
    };

    let response_status = if is_range_request {
        response_builder = response_builder.header(
            header::CONTENT_RANGE,
            format!("bytes {}-{}/{}", start, end, file_size),
        );
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };

    response_builder = response_builder.header(header::CONTENT_LENGTH, len);

    // For HEAD requests, return headers only without streaming the body
    if method == Method::HEAD {
        debug!("HEAD request for media file ID {} (size: {})", file_id, file_size);
        let response_time = start_time.elapsed().as_micros() as u64;
        state.web_metrics.record_file_serve(response_time, false);
        return Ok(response_builder.status(response_status).body(Body::empty())?);
    }

    file.seek(std::io::SeekFrom::Start(start)).await?;
    let tracking_reader = MetricsTrackingReader {
        inner: file.take(len),
        metrics: state.web_metrics.clone(),
    };
    let stream = ReaderStream::with_capacity(tracking_reader, 64 * 1024);
    let body = Body::from_stream(stream);

    // Record atomic performance metrics for file serving
    let response_time = start_time.elapsed().as_micros() as u64;
    let is_actual_serve = method == Method::GET && start == 0 && (len > 2 || len == file_size);
    state.web_metrics.record_file_serve(response_time, is_actual_serve);
    
    debug!("Served media file ID {} ({} bytes from offset {}) in {}ms", file_id, len, start, response_time);

    Ok(response_builder.status(response_status).body(body)?)
}

// Helper function to parse range header manually
fn parse_range_header(range_str: &str, file_size: u64) -> Result<(u64, u64), AppError> {
    let range_str = range_str.trim();
    // Remove "bytes=" prefix
    let range_part = range_str.strip_prefix("bytes=").ok_or(AppError::InvalidRange)?.trim();
    
    // Split on comma to get individual ranges (we'll just handle the first one)
    let first_range = range_part.split(',').next().ok_or(AppError::InvalidRange)?.trim();
    
    // Parse the range
    if let Some((start_str, end_str)) = first_range.split_once('-') {
        let start_str = start_str.trim();
        let end_str = end_str.trim();
        
        let start = if start_str.is_empty() {
            // Suffix range like "-500" (last 500 bytes)
            let suffix_len: u64 = end_str.parse().map_err(|_| AppError::InvalidRange)?;
            file_size.saturating_sub(suffix_len)
        } else {
            start_str.parse().map_err(|_| AppError::InvalidRange)?
        };
        
        let end = if end_str.is_empty() {
            // Range like "500-" (from 500 to end)
            file_size - 1
        } else {
            let parsed_end: u64 = end_str.parse().map_err(|_| AppError::InvalidRange)?;
            parsed_end.min(file_size - 1)
        };
        
        // Validate range
        if start > end || start >= file_size {
            return Err(AppError::InvalidRange);
        }
        
        Ok((start, end))
    } else {
        Err(AppError::InvalidRange)
    }
}

/// Handle UPnP eventing subscription requests for ContentDirectory service
pub async fn content_directory_subscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
    method: Method,
) -> impl IntoResponse {
    // Handle SUBSCRIBE method (which might come as GET or a custom method)
    if method == Method::GET || headers.get("CALLBACK").is_some() {
        // Handle subscription request
        if let Some(callback) = headers.get("CALLBACK") {
            let callback_url = callback.to_str().unwrap_or("");
            info!("UPnP subscription request from: {}", callback_url);
            
            // Generate a subscription ID (in a real implementation, this should be stored)
            let subscription_id = format!("uuid:{}", uuid::Uuid::new_v4());
            let timeout = "Second-1800"; // 30 minutes
            
            // Get current update ID
            let update_id = state.content_update_id.load(std::sync::atomic::Ordering::Relaxed);
            
            // Send initial event notification
            tokio::spawn(send_initial_event_notification(callback_url.to_string(), update_id));
            
            (
                StatusCode::OK,
                [
                    (header::HeaderName::from_static("sid"), subscription_id.as_str()),
                    (header::HeaderName::from_static("timeout"), timeout),
                    (header::CONTENT_LENGTH, "0"),
                ],
                "",
            ).into_response()
        } else {
            warn!("UPnP subscription request missing CALLBACK header");
            (
                StatusCode::BAD_REQUEST,
                [
                    (header::CONTENT_TYPE, "text/plain"),
                    (header::CONTENT_LENGTH, "0"),
                ],
                "",
            ).into_response()
        }
    } else if headers.get("SID").is_some() {
        // Handle unsubscription request (UNSUBSCRIBE method)
        let subscription_id = headers.get("SID").unwrap().to_str().unwrap_or("");
        info!("UPnP unsubscription request for: {}", subscription_id);
        
        (
            StatusCode::OK,
            [(header::CONTENT_LENGTH, "0")],
            "",
        ).into_response()
    } else {
        (
            StatusCode::METHOD_NOT_ALLOWED,
            [
                (header::CONTENT_TYPE, "text/plain"),
                (header::CONTENT_LENGTH, "0"),
            ],
            "",
        ).into_response()
    }
}

/// Send initial event notification to a subscribed client
async fn send_initial_event_notification(callback_url: String, update_id: u32) {
    let event_body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<e:propertyset xmlns:e="urn:schemas-upnp-org:event-1-0">
    <e:property>
        <SystemUpdateID>{}</SystemUpdateID>
    </e:property>
    <e:property>
        <ContainerUpdateIDs></ContainerUpdateIDs>
    </e:property>
</e:propertyset>"#,
        update_id
    );
    
    // Extract the actual URL from the callback (remove angle brackets if present)
    let url = callback_url.trim_start_matches('<').trim_end_matches('>');
    
    let client = reqwest::Client::new();
    match client
        .request(reqwest::Method::from_bytes(b"NOTIFY").unwrap(), url)
        .header("HOST", "")
        .header("CONTENT-TYPE", "text/xml; charset=\"utf-8\"")
        .header("NT", "upnp:event")
        .header("NTS", "upnp:propchange")
        .header("SID", "uuid:dummy") // In real implementation, use actual subscription ID
        .header("SEQ", "0")
        .body(event_body)
        .send()
        .await
    {
        Ok(response) => {
            debug!("Event notification sent successfully, status: {}", response.status());
        }
        Err(e) => {
            warn!("Failed to send event notification to {}: {}", url, e);
        }
    }
}

// Music categorization handlers

/// Handle browsing the root audio container with music categorization
async fn handle_audio_root_browse(
    params: &BrowseParams,
    state: &AppState,
) -> Response {
    use crate::web::xml::generate_browse_response;
    
    // Create virtual categorization containers
    let virtual_containers = vec![
        ("audio/artists", "Artists"),
        ("audio/albums", "Albums"), 
        ("audio/genres", "Genres"),
        ("audio/years", "Years"),
        ("audio/playlists", "Playlists"),
        ("audio/folders", "Folders"),
    ];
    
    // Convert to MediaDirectory for XML generation
    let subdirectories: Vec<crate::database::MediaDirectory> = virtual_containers
        .into_iter()
        .map(|(id, name)| crate::database::MediaDirectory {
            path: std::path::PathBuf::from(id),
            name: name.to_string(),
        })
        .collect();
    
    let server_ip = state.get_server_ip();
    let response = generate_browse_response(&params.object_id, &subdirectories, &[], state, &server_ip).await;
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
            (header::HeaderName::from_static("ext"), ""),
        ],
        response,
    )
        .into_response()
}
    
impl ContentDirectoryHandler {
    /// Handle artist browse requests with atomic performance tracking and ReDB operations
    async fn handle_artist_browse(
        params: &BrowseParams,
        state: &AppState,
        audio_path: &str,
    ) -> Response {
        let db1 = state.database.clone();
        let db2 = state.database.clone();
        handle_generic_category_browse(
            params,
            state,
            audio_path,
            "audio/artists",
            "artists",
            move || async move { db1.get_artists().await },
            |artist| crate::database::MediaDirectory {
                path: std::path::PathBuf::from(format!("audio/artists/{}", artist.name)),
                name: format!("{} ({})", artist.name, artist.count),
            },
            move |artist_name| async move { db2.get_music_by_artist(&artist_name).await },
        ).await
    }

    /// Handle album browse requests with atomic performance tracking and ReDB operations
    async fn handle_album_browse(
        params: &BrowseParams,
        state: &AppState,
        audio_path: &str,
    ) -> Response {
        let db1 = state.database.clone();
        let db2 = state.database.clone();
        handle_generic_category_browse(
            params,
            state,
            audio_path,
            "audio/albums",
            "albums",
            move || async move { db1.get_albums(None).await },
            |album| crate::database::MediaDirectory {
                path: std::path::PathBuf::from(format!("audio/albums/{}", album.name)),
                name: format!("{} ({})", album.name, album.count),
            },
            move |album_name| async move { db2.get_music_by_album(&album_name, None).await },
        ).await
    }
}

/// Handle browsing genres with atomic performance tracking and ReDB operations
async fn handle_genres_browse(
    params: &BrowseParams,
    state: &AppState,
    audio_path: &str,
) -> Response {
    let db1 = state.database.clone();
    let db2 = state.database.clone();
    handle_generic_category_browse(
        params,
        state,
        audio_path,
        "audio/genres",
        "genres",
        move || async move { db1.get_genres().await },
        |genre| crate::database::MediaDirectory {
            path: std::path::PathBuf::from(format!("audio/genres/{}", genre.name)),
            name: format!("{} ({})", genre.name, genre.count),
        },
        move |genre_name| async move { db2.get_music_by_genre(&genre_name).await },
    ).await
}

/// Handle browsing years with atomic performance tracking and ReDB operations
async fn handle_years_browse(
    params: &BrowseParams,
    state: &AppState,
    audio_path: &str,
) -> Response {
    let db1 = state.database.clone();
    let db2 = state.database.clone();
    handle_generic_category_browse(
        params,
        state,
        audio_path,
        "audio/years",
        "years",
        move || async move { db1.get_years().await },
        |year| crate::database::MediaDirectory {
            path: std::path::PathBuf::from(format!("audio/years/{}", year.name)),
            name: format!("{} ({})", year.name, year.count),
        },
        move |year_str| async move {
            if let Ok(year) = year_str.parse::<u32>() {
                db2.get_music_by_year(year).await
            } else {
                Err(anyhow::anyhow!("Invalid year format"))
            }
        },
    ).await
}

/// Handle browsing playlists with atomic performance tracking and ReDB operations
async fn handle_playlists_browse(
    params: &BrowseParams,
    state: &AppState,
    audio_path: &str,
) -> Response {
    let db1 = state.database.clone();
    let db2 = state.database.clone();
    handle_generic_category_browse(
        params,
        state,
        audio_path,
        "audio/playlists",
        "playlists",
        move || async move { db1.get_playlists().await },
        |playlist| crate::database::MediaDirectory {
            path: std::path::PathBuf::from(format!("audio/playlists/{}", playlist.id.unwrap_or(0))),
            name: playlist.name,
        },
        move |playlist_id_str| async move {
            if let Ok(playlist_id) = playlist_id_str.parse::<i64>() {
                db2.get_playlist_tracks(playlist_id).await
            } else {
                Err(anyhow::anyhow!("Invalid playlist ID format"))
            }
        },
    ).await
}

/// Helper function to perform generic music category browsing
async fn handle_generic_category_browse<C, F, FFuture, G, GFuture>(
    params: &BrowseParams,
    state: &AppState,
    audio_path: &str,
    _category_prefix: &str,
    category_name: &str,
    list_categories_fn: F,
    map_category_fn: impl Fn(C) -> crate::database::MediaDirectory,
    list_items_fn: G,
) -> Response
where
    F: FnOnce() -> FFuture,
    FFuture: std::future::Future<Output = Result<Vec<C>, anyhow::Error>>,
    G: FnOnce(String) -> GFuture,
    GFuture: std::future::Future<Output = Result<Vec<crate::database::MediaFile>, anyhow::Error>>,
{
    use crate::web::xml::generate_browse_response;
    
    let start_time = Instant::now();
    
    let client = crate::web::client::CURRENT_CLIENT.try_with(|c| *c)
        .unwrap_or(crate::web::client::DlnaClientProfile::Standard);
    
    let current_update_id = state.content_update_id.load(Ordering::Relaxed);
    let cache_key = crate::state::SoapCacheKey {
        object_id: params.object_id.clone(),
        starting_index: params.starting_index,
        requested_count: params.requested_count,
        client_profile: client,
        content_update_id: current_update_id,
    };

    // Cache lookup
    {
        let mut cache = state.browse_cache.lock().await;
        let needs_clear = cache.keys().next().map(|k| k.content_update_id != current_update_id).unwrap_or(false);
        if needs_clear {
            cache.clear();
        }
        if let Some(cached_xml) = cache.get(&cache_key) {
            let response_time = start_time.elapsed().as_micros() as u64;
            state.web_metrics.record_browse_request(response_time, true);
            debug!("Browse Cache Hit for Category ObjectID: {} ({}ms)", params.object_id, response_time);
            return (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                    (header::HeaderName::from_static("ext"), ""),
                ],
                cached_xml.clone(),
            ).into_response();
        }
    }

    // Find if we are browsing a category list (e.g. "artists") or filtering by a category value (e.g. "artists/AC/DC")
    let (is_category_list, key_str_opt) = if let Some(slash_idx) = audio_path.find('/') {
        let key_raw = &audio_path[slash_idx + 1..];
        let key_str = percent_encoding::percent_decode_str(key_raw)
            .decode_utf8_lossy()
            .into_owned();
        (false, Some(key_str))
    } else {
        (true, None)
    };
    
    if is_category_list {
        match list_categories_fn().await {
            Ok(categories) => {
                let has_data = !categories.is_empty();
                let subdirectories: Vec<crate::database::MediaDirectory> = categories
                    .into_iter()
                    .map(map_category_fn)
                    .collect();
                
                let response_time = start_time.elapsed().as_micros() as u64;
                state.web_metrics.record_browse_request(response_time, has_data);
                
                debug!("ReDB retrieved {} {} in {}ms", subdirectories.len(), category_name, response_time);
                    
                let server_ip = state.get_server_ip();
                let response = generate_browse_response(&params.object_id, &subdirectories, &[], state, &server_ip).await;
                
                // Cache insert
                {
                    let mut cache = state.browse_cache.lock().await;
                    let needs_clear = cache.keys().next().map(|k| k.content_update_id != current_update_id).unwrap_or(false);
                    if needs_clear {
                        cache.clear();
                    }
                    cache.insert(cache_key.clone(), response.clone());
                }

                (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                        (header::HeaderName::from_static("ext"), ""),
                    ],
                    response,
                )
                    .into_response()
            }
            Err(e) => {
                error!("ReDB error getting {}: {}", category_name, e);
                
                let response_time = start_time.elapsed().as_micros() as u64;
                state.web_metrics.record_error();
                state.web_metrics.record_browse_request(response_time, false);
                
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    format!("Error browsing {}", category_name),
                )
                    .into_response()
            }
        }
    } else if let Some(key_str) = key_str_opt {
        match list_items_fn(key_str.clone()).await {
            Ok(files) => {
                let response_time = start_time.elapsed().as_micros() as u64;
                state.web_metrics.record_browse_request(response_time, !files.is_empty());
                
                debug!("ReDB retrieved {} tracks for {} '{}' in {}ms", files.len(), category_name, key_str, response_time);
                
                let server_ip = state.get_server_ip();
                let response = generate_browse_response(&params.object_id, &[], &files, state, &server_ip).await;
                
                // Cache insert
                {
                    let mut cache = state.browse_cache.lock().await;
                    let needs_clear = cache.keys().next().map(|k| k.content_update_id != current_update_id).unwrap_or(false);
                    if needs_clear {
                        cache.clear();
                    }
                    cache.insert(cache_key.clone(), response.clone());
                }

                (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                        (header::HeaderName::from_static("ext"), ""),
                    ],
                    response,
                )
                    .into_response()
            }
            Err(e) => {
                error!("ReDB error getting music by {} {}: {}", category_name, key_str, e);
                
                let response_time = start_time.elapsed().as_micros() as u64;
                state.web_metrics.record_error();
                state.web_metrics.record_browse_request(response_time, false);
                
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    format!("Error browsing {} tracks", category_name),
                )
                    .into_response()
            }
        }
    } else {
        let response_time = start_time.elapsed().as_micros() as u64;
        state.web_metrics.record_error();
        state.web_metrics.record_browse_request(response_time, false);
        
        (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            format!("Invalid {} path", category_name),
        )
            .into_response()
    }
}

fn parse_dir_index_prefix(path_prefix_str: &str) -> (Option<usize>, &str) {
    if path_prefix_str.starts_with('d') {
        let chars = path_prefix_str.chars().skip(1);
        let mut num_str = String::new();
        for c in chars {
            if c.is_ascii_digit() {
                num_str.push(c);
            } else {
                break;
            }
        }
        if !num_str.is_empty() {
            if let Ok(idx) = num_str.parse::<usize>() {
                let prefix_len = 1 + num_str.len();
                let rem = if path_prefix_str.len() > prefix_len {
                    path_prefix_str[prefix_len..].trim_start_matches('/')
                } else {
                    ""
                };
                (Some(idx), rem)
            } else {
                (None, path_prefix_str)
            }
        } else {
            (None, path_prefix_str)
        }
    } else {
        (None, path_prefix_str)
    }
}

fn build_soap_response(action: &str, service_type: &str, content: &str) -> Response {
    let mut xml = String::with_capacity(300 + action.len() * 2 + service_type.len() + content.len());
    xml.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    xml.push_str("<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\n");
    xml.push_str("    <s:Body>\n");
    xml.push_str("        <u:");
    xml.push_str(action);
    xml.push_str("Response xmlns:u=\"");
    xml.push_str(service_type);
    xml.push_str("\">\n");
    xml.push_str("            ");
    xml.push_str(content);
    xml.push_str("\n");
    xml.push_str("        </u:");
    xml.push_str(action);
    xml.push_str("Response>\n");
    xml.push_str("    </s:Body>\n");
    xml.push_str("</s:Envelope>");
    
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
            (header::HeaderName::from_static("ext"), ""),
        ],
        xml,
    )
        .into_response()
}

pub async fn connection_manager_scpd() -> impl IntoResponse {
    let xml = crate::web::xml::generate_connection_manager_scpd();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        xml,
    )
}

pub async fn media_receiver_registrar_scpd() -> impl IntoResponse {
    let xml = crate::web::xml::generate_registrar_scpd();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        xml,
    )
}

pub async fn connection_manager_control(
    State(_state): State<AppState>,
    body: String,
) -> Response {
    if body.contains("<u:GetProtocolInfo") {
        let content = r#"<Source>http-get:*:video/x-msvideo:*,http-get:*:video/mp4:*,http-get:*:video/x-matroska:*,http-get:*:video/x-mkv:*,http-get:*:video/mpeg:*,http-get:*:video/divx:*,http-get:*:audio/mpeg:*,http-get:*:audio/x-flac:*,http-get:*:audio/wav:*,http-get:*:audio/mp4:*,http-get:*:image/jpeg:*,http-get:*:image/png:*,http-get:*:image/gif:*</Source><Sink></Sink>"#;
        build_soap_response("GetProtocolInfo", "urn:schemas-upnp-org:service:ConnectionManager:1", content)
    } else if body.contains("<u:GetCurrentConnectionIDs") {
        let content = "<ConnectionIDs>0</ConnectionIDs>";
        build_soap_response("GetCurrentConnectionIDs", "urn:schemas-upnp-org:service:ConnectionManager:1", content)
    } else if body.contains("<u:GetCurrentConnectionInfo") {
        let content = r#"<RcsID>-1</RcsID><AVTransportID>-1</AVTransportID><ProtocolInfo></ProtocolInfo><PeerConnectionManager></PeerConnectionManager><PeerConnectionID>-1</PeerConnectionID><Direction>Output</Direction><Status>Unknown</Status>"#;
        build_soap_response("GetCurrentConnectionInfo", "urn:schemas-upnp-org:service:ConnectionManager:1", content)
    } else {
        (
            StatusCode::NOT_IMPLEMENTED,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Not implemented".to_string(),
        )
            .into_response()
    }
}

pub async fn media_receiver_registrar_control(
    State(_state): State<AppState>,
    body: String,
) -> Response {
    if body.contains("<u:IsAuthorized") {
        let content = "<Result>1</Result>";
        build_soap_response("IsAuthorized", "urn:microsoft.com:service:X_MS_MediaReceiverRegistrar:1", content)
    } else if body.contains("<u:RegisterDevice") {
        let content = "<RegistrationRespMsg></RegistrationRespMsg>";
        build_soap_response("RegisterDevice", "urn:microsoft.com:service:X_MS_MediaReceiverRegistrar:1", content)
    } else {
        (
            StatusCode::NOT_IMPLEMENTED,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Not implemented".to_string(),
        )
            .into_response()
    }
}

pub async fn serve_subtitle(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let file_id = id.parse::<i64>().map_err(|_| {
        state.web_metrics.record_error();
        AppError::NotFound
    })?;
    
    let file_info = state.database
        .get_file_by_id(file_id)
        .await
        .map_err(|e| {
            error!("Error getting file by ID for subtitle {}: {}", file_id, e);
            state.web_metrics.record_error();
            AppError::NotFound
        })?
        .ok_or_else(|| {
            state.web_metrics.record_error();
            AppError::NotFound
        })?;

    let srt_path = PathBuf::from(&file_info.path).with_extension("srt");
    if !srt_path.exists() {
        return Err(AppError::NotFound);
    }

    let file = tokio::fs::OpenOptions::new()
        .read(true)
        .write(false)
        .open(&srt_path)
        .await
        .map_err(AppError::Io)?;

    let stream = tokio_util::io::ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Response::builder()
        .header(header::CONTENT_TYPE, "text/srt")
        .body(body)
        .map_err(|_| AppError::NotFound)
}

pub async fn serve_cover(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let file_id = id.parse::<i64>().map_err(|_| {
        state.web_metrics.record_error();
        AppError::NotFound
    })?;
    
    let file_info = state.database
        .get_file_by_id(file_id)
        .await
        .map_err(|e| {
            error!("Error getting file by ID for cover {}: {}", file_id, e);
            state.web_metrics.record_error();
            AppError::NotFound
        })?
        .ok_or_else(|| {
            state.web_metrics.record_error();
            AppError::NotFound
        })?;

    if !file_info.mime_type.starts_with("audio/") {
        return Err(AppError::NotFound);
    }

    // 1. Primary: Search parent directory for cover images (fast)
    if let Some(parent) = file_info.path.parent() {
        let base_name = file_info.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        
        let cover_filenames = [
            "cover", "Cover", "COVER",
            "folder", "Folder", "FOLDER",
            "album", "Album", "ALBUM",
            "artwork", "Artwork", "ARTWORK",
            base_name,
        ];
        
        let extensions = ["jpg", "jpeg", "png", "webp", "heif", "heic", "avif"];
        
        for name in &cover_filenames {
            for ext in &extensions {
                let img_path = parent.join(format!("{}.{}", name, ext));
                if img_path.exists() && img_path.is_file() {
                    if let Ok(data) = tokio::fs::read(&img_path).await {
                        let content_type = crate::platform::filesystem::get_mime_type_for_extension(ext);
                        return Response::builder()
                            .header(header::CONTENT_TYPE, content_type)
                            .body(Body::from(data))
                            .map_err(|_| AppError::NotFound);
                    }
                }
            }
        }
    }

    // 2. Secondary: Extract embedded artwork from audio tags using audiotags (blocking task)
    let path = file_info.path.clone();
    let tag_result = tokio::task::spawn_blocking(move || {
        audiotags::Tag::new().read_from_path(&path)
    }).await;

    if let Ok(Ok(tag)) = tag_result {
        if let Some(cover) = tag.album_cover() {
            let content_type = match cover.mime_type {
                audiotags::MimeType::Jpeg => "image/jpeg",
                audiotags::MimeType::Png => "image/png",
                _ => "image/jpeg",
            };
            return Response::builder()
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(cover.data.to_vec()))
                .map_err(|_| AppError::NotFound);
        }
    }

    Err(AppError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_browse_params_valid_xml() {
        let xml_body = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
    <s:Body>
        <u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
            <ObjectID>video/movies</ObjectID>
            <BrowseFlag>BrowseDirectChildren</BrowseFlag>
            <Filter>*</Filter>
            <StartingIndex>10</StartingIndex>
            <RequestedCount>25</RequestedCount>
            <SortCriteria></SortCriteria>
        </u:Browse>
    </s:Body>
</s:Envelope>"#;

        let params = parse_browse_params(xml_body);
        assert_eq!(params.object_id, "video/movies");
        assert_eq!(params.starting_index, 10);
        assert_eq!(params.requested_count, 25);
    }

    #[test]
    fn test_parse_browse_params_minimal_xml() {
        let xml_body = r#"<ObjectID>0</ObjectID><StartingIndex>0</StartingIndex><RequestedCount>0</RequestedCount>"#;

        let params = parse_browse_params(xml_body);
        assert_eq!(params.object_id, "0");
        assert_eq!(params.starting_index, 0);
        assert_eq!(params.requested_count, 0);
    }

    #[test]
    fn test_parse_browse_params_missing_elements() {
        let xml_body = r#"<ObjectID>audio/artists</ObjectID>"#;

        let params = parse_browse_params(xml_body);
        assert_eq!(params.object_id, "audio/artists");
        assert_eq!(params.starting_index, 0); // Default value
        assert_eq!(params.requested_count, 0); // Default value
    }

    #[test]
    fn test_parse_browse_params_invalid_numbers() {
        let xml_body = r#"<ObjectID>test</ObjectID><StartingIndex>invalid</StartingIndex><RequestedCount>not_a_number</RequestedCount>"#;

        let params = parse_browse_params(xml_body);
        assert_eq!(params.object_id, "test");
        assert_eq!(params.starting_index, 0); // Falls back to default
        assert_eq!(params.requested_count, 0); // Falls back to default
    }

    #[test]
    fn test_parse_browse_params_empty_xml() {
        let xml_body = "";

        let params = parse_browse_params(xml_body);
        assert_eq!(params.object_id, "0"); // Default value
        assert_eq!(params.starting_index, 0); // Default value
        assert_eq!(params.requested_count, 0); // Default value
    }

    #[test]
    fn test_parse_browse_params_malformed_xml() {
        let xml_body = r#"<ObjectID>test</ObjectID><StartingIndex>5<RequestedCount>10</RequestedCount>"#;

        let params = parse_browse_params(xml_body);
        // Should handle malformed XML gracefully and extract what it can
        assert_eq!(params.object_id, "test");
        // The parser should still work despite the malformed StartingIndex tag
    }

    #[test]
    fn test_parse_browse_params_with_whitespace() {
        let xml_body = r#"
        <ObjectID>  video/series  </ObjectID>
        <StartingIndex>  5  </StartingIndex>
        <RequestedCount>  15  </RequestedCount>
        "#;

        let params = parse_browse_params(xml_body);
        assert_eq!(params.object_id, "video/series"); // Should be trimmed
        assert_eq!(params.starting_index, 5);
        assert_eq!(params.requested_count, 15);
    }

    #[test]
    fn test_parse_browse_params_performance_comparison() {
        // This test demonstrates that the new XML parser handles complex XML correctly
        // while the old string-based approach would be fragile
        let complex_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" 
            s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
            <ObjectID>video/movies/action</ObjectID>
            <BrowseFlag>BrowseDirectChildren</BrowseFlag>
            <Filter>dc:title,dc:date,upnp:class,res@duration,res@size</Filter>
            <StartingIndex>100</StartingIndex>
            <RequestedCount>50</RequestedCount>
            <SortCriteria>+dc:title</SortCriteria>
        </u:Browse>
    </s:Body>
</s:Envelope>"#;

        let params = parse_browse_params(complex_xml);
        assert_eq!(params.object_id, "video/movies/action");
        assert_eq!(params.starting_index, 100);
        assert_eq!(params.requested_count, 50);
    }

    #[test]
    fn test_parse_dir_index_prefix() {
        assert_eq!(parse_dir_index_prefix("d0"), (Some(0), ""));
        assert_eq!(parse_dir_index_prefix("d0/movies"), (Some(0), "movies"));
        assert_eq!(parse_dir_index_prefix("d12/movies/action"), (Some(12), "movies/action"));
        assert_eq!(parse_dir_index_prefix("d0/"), (Some(0), ""));
        assert_eq!(parse_dir_index_prefix("movies"), (None, "movies"));
        assert_eq!(parse_dir_index_prefix("d"), (None, "d"));
        assert_eq!(parse_dir_index_prefix("dx"), (None, "dx"));
        assert_eq!(parse_dir_index_prefix(""), (None, ""));
    }

}/// 
/// Get web handler performance metrics for monitoring
pub async fn get_web_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.web_metrics.get_stats();
    
    let db_stats = match state.database.get_stats().await {
        Ok(s) => s,
        Err(_) => crate::database::DatabaseStats {
            total_files: 0,
            total_size: 0,
            database_size: 0,
            video_files: 0,
            audio_files: 0,
            image_files: 0,
            playlists: 0,
        }
    };

    let active_casts = {
        let mut casts = state.active_casts.lock().await;
        // Retain only entries active in the last 3 minutes (180 seconds)
        casts.retain(|_, (_, last_seen)| last_seen.elapsed() < std::time::Duration::from_secs(180));
        
        let map: std::collections::HashMap<String, String> = casts
            .iter()
            .map(|(k, (v, _))| (k.clone(), v.clone()))
            .collect();
        map
    };
    
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
        "active_casts": active_casts
    });
    
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        metrics_json.to_string(),
    )
}

/// Helper to read the last N lines of the log file using a ring buffer
async fn read_last_log_lines(path: &std::path::Path, limit: usize) -> Result<String, std::io::Error> {
    use tokio::fs::File;
    use tokio::io::{BufReader, AsyncBufReadExt};
    
    let file = File::open(path).await?;
    let reader = BufReader::new(file);
    let mut queue = std::collections::VecDeque::with_capacity(limit + 1);
    let mut lines_stream = reader.lines();
    
    while let Some(line) = lines_stream.next_line().await? {
        queue.push_back(line);
        if queue.len() > limit {
            queue.pop_front();
        }
    }
    
    let mut result = String::new();
    for line in queue {
        result.push_str(&line);
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
pub async fn readyz_handler(State(state): State<AppState>) -> impl IntoResponse {
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
                format!(r#"{{"status":"unhealthy","error":{}}}"#, serde_json::to_string(&e.to_string()).unwrap_or_default()),
            )
        }
    }
}

/// Serve metrics in standard Prometheus Exposition Format (plain text)
pub async fn get_prometheus_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.web_metrics.get_stats();
    
    let (db_files, db_total_size, db_size, db_video, db_audio, db_image, db_playlists) = match state.database.get_stats().await {
        Ok(s) => (s.total_files, s.total_size, s.database_size, s.video_files, s.audio_files, s.image_files, s.playlists),
        Err(_) => (0, 0, 0, 0, 0, 0, 0),
    };

    let mut body = String::new();
    
    body.push_str("# HELP vuio_web_browse_requests_total Total number of media browse requests\n");
    body.push_str("# TYPE vuio_web_browse_requests_total counter\n");
    body.push_str(&format!("vuio_web_browse_requests_total {}\n\n", stats.browse_requests));
    
    body.push_str("# HELP vuio_web_cache_hits_total Total number of browse cache hits\n");
    body.push_str("# TYPE vuio_web_cache_hits_total counter\n");
    body.push_str(&format!("vuio_web_cache_hits_total {}\n\n", stats.cache_hits));
    
    body.push_str("# HELP vuio_web_cache_misses_total Total number of browse cache misses\n");
    body.push_str("# TYPE vuio_web_cache_misses_total counter\n");
    body.push_str(&format!("vuio_web_cache_misses_total {}\n\n", stats.cache_misses));
    
    body.push_str("# HELP vuio_web_directory_listings_total Total number of directory listing requests\n");
    body.push_str("# TYPE vuio_web_directory_listings_total counter\n");
    body.push_str(&format!("vuio_web_directory_listings_total {}\n\n", stats.directory_listings));
    
    body.push_str("# HELP vuio_web_file_serves_total Total number of files served\n");
    body.push_str("# TYPE vuio_web_file_serves_total counter\n");
    body.push_str(&format!("vuio_web_file_serves_total {}\n\n", stats.file_serves));

    body.push_str("# HELP vuio_web_gigabytes_transferred_total Total gigabytes of media transferred\n");
    body.push_str("# TYPE vuio_web_gigabytes_transferred_total counter\n");
    body.push_str(&format!("vuio_web_gigabytes_transferred_total {}\n\n", stats.gigabytes_transferred));
    
    body.push_str("# HELP vuio_web_errors_total Total number of web handler errors\n");
    body.push_str("# TYPE vuio_web_errors_total counter\n");
    body.push_str(&format!("vuio_web_errors_total {}\n\n", stats.errors));
    
    body.push_str("# HELP vuio_web_average_response_time_ms Average response time in milliseconds\n");
    body.push_str("# TYPE vuio_web_average_response_time_ms gauge\n");
    body.push_str(&format!("vuio_web_average_response_time_ms {}\n\n", stats.average_response_time_ms));

    body.push_str("# HELP vuio_database_files Total media files indexed in database\n");
    body.push_str("# TYPE vuio_database_files gauge\n");
    body.push_str(&format!("vuio_database_files {}\n\n", db_files));

    body.push_str("# HELP vuio_database_total_size_bytes Cumulative size of all media files in bytes\n");
    body.push_str("# TYPE vuio_database_total_size_bytes gauge\n");
    body.push_str(&format!("vuio_database_total_size_bytes {}\n\n", db_total_size));

    body.push_str("# HELP vuio_database_size_bytes Size of the database file on disk in bytes\n");
    body.push_str("# TYPE vuio_database_size_bytes gauge\n");
    body.push_str(&format!("vuio_database_size_bytes {}\n\n", db_size));

    body.push_str("# HELP vuio_database_video_files Total video files indexed in database\n");
    body.push_str("# TYPE vuio_database_video_files gauge\n");
    body.push_str(&format!("vuio_database_video_files {}\n\n", db_video));

    body.push_str("# HELP vuio_database_audio_files Total audio files indexed in database\n");
    body.push_str("# TYPE vuio_database_audio_files gauge\n");
    body.push_str(&format!("vuio_database_audio_files {}\n\n", db_audio));

    body.push_str("# HELP vuio_database_image_files Total image/picture files indexed in database\n");
    body.push_str("# TYPE vuio_database_image_files gauge\n");
    body.push_str(&format!("vuio_database_image_files {}\n\n", db_image));

    body.push_str("# HELP vuio_database_playlists Total playlists imported in database\n");
    body.push_str("# TYPE vuio_database_playlists gauge\n");
    body.push_str(&format!("vuio_database_playlists {}\n", db_playlists));

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}

#[derive(serde::Deserialize)]
pub struct LogsQuery {
    pub limit: Option<usize>,
}

/// Serve log file contents for Loki / Grafana scraping or debugging
pub async fn get_logs_handler(
    State(state): State<AppState>,
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
                format!("Failed to read log file: {}", e),
            )
        }
    }
}

#[derive(serde::Deserialize)]
pub struct ApiCastPlaylistRequest {
    pub tv_name: String,
    pub folder_name: String,
    pub file_ids: Vec<i64>,
}

/// Discover UPnP/DLNA TVs and return their friendly names in JSON format
pub async fn api_list_tvs() -> impl IntoResponse {
    match crate::tv_control::discover_tvs().await {
        Ok(tvs) => {
            let names: Vec<String> = tvs.into_iter().map(|tv| tv.friendly_name).collect();
            (StatusCode::OK, axum::Json(names))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(vec![format!("Discovery error: {}", e)]),
        ),
    }
}

/// Create a temporary playlist with the provided video files and cast it to the TV
pub async fn api_cast_playlist(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<ApiCastPlaylistRequest>,
) -> impl IntoResponse {
    // 1. List playlists to find and delete old "Web Cast - " playlists
    let playlists = match state.database.get_playlists().await {
        Ok(list) => list,
        Err(e) => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({ "error": format!("Database error: {}", e) })),
        ),
    };

    for pl in playlists {
        if pl.name.starts_with("Web Cast - ") {
            if let Some(id) = pl.id {
                let _ = state.database.delete_playlist(id).await;
            }
        }
    }

    // 2. Create the new playlist
    let playlist_name = format!("Web Cast - {}", payload.folder_name);
    let playlist_id = match state.database.create_playlist(&playlist_name, None).await {
        Ok(id) => id,
        Err(e) => return (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({ "error": format!("Failed to create playlist: {}", e) })),
        ),
    };

    // 3. Add file IDs to the playlist (batch add)
    let tracks_to_add: Vec<(i64, u32)> = payload
        .file_ids
        .iter()
        .enumerate()
        .map(|(idx, &id)| (id, idx as u32))
        .collect();

    if let Err(e) = state.database.batch_add_to_playlist(playlist_id, &tracks_to_add).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({ "error": format!("Failed to add tracks: {}", e) })),
        );
    }

    // 4. Cast the playlist starting at index 0 using our shared helper
    match crate::web::mcp::cast_playlist_helper(&state, playlist_id, &payload.tv_name, 0).await {
        Ok(result) => (StatusCode::OK, axum::Json(result)),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({ "error": format!("Cast error: {}", e) })),
        ),
    }
}
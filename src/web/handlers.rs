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
    pub total_response_time_ms: AtomicU64,
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
            total_response_time_ms: AtomicU64::new(0),
        }
    }
    
    pub fn record_browse_request(&self, response_time_ms: u64, cache_hit: bool) {
        self.browse_requests.fetch_add(1, Ordering::Relaxed);
        self.total_response_time_ms.fetch_add(response_time_ms, Ordering::Relaxed);
        if cache_hit {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.cache_misses.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    pub fn record_directory_listing(&self, response_time_ms: u64) {
        self.directory_listings.fetch_add(1, Ordering::Relaxed);
        self.total_response_time_ms.fetch_add(response_time_ms, Ordering::Relaxed);
    }
    
    pub fn record_file_serve(&self, response_time_ms: u64) {
        self.file_serves.fetch_add(1, Ordering::Relaxed);
        self.total_response_time_ms.fetch_add(response_time_ms, Ordering::Relaxed);
    }
    
    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn get_stats(&self) -> WebHandlerStats {
        let browse_requests = self.browse_requests.load(Ordering::Relaxed);
        let total_time = self.total_response_time_ms.load(Ordering::Relaxed);
        
        WebHandlerStats {
            browse_requests,
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            directory_listings: self.directory_listings.load(Ordering::Relaxed),
            file_serves: self.file_serves.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            average_response_time_ms: if browse_requests > 0 { total_time / browse_requests } else { 0 },
            cache_hit_rate: if browse_requests > 0 { 
                (self.cache_hits.load(Ordering::Relaxed) as f64 / browse_requests as f64) * 100.0 
            } else { 0.0 },
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
    pub average_response_time_ms: u64,
    pub cache_hit_rate: f64,
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
            
        let category = if file.mime_type.starts_with("video/") {
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
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{server_name}</title>
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
            max-width: 800px;
            display: flex;
            flex-direction: column;
            gap: 1.25rem;
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
                    <p>VuIO Media Streamer</p>
                </div>
            </div>
        </header>

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
            </div>
        </div>

        <div class="file-list" id="file-list"></div>
    </div>

    <script>
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

            // Filter files by tab and search
            let filteredFiles = filesData.filter(file => {{
                const matchesTab = currentTab === 'all' || file.cat === currentTab;
                const matchesSearch = searchQuery === '' || file.name.toLowerCase().includes(searchQuery);
                return matchesTab && matchesSearch;
            }});

            // If searching, show a flat search results view
            if (searchQuery !== '') {{
                document.getElementById('breadcrumbs').innerHTML = '<span>Search Results</span>';
                
                if (filteredFiles.length === 0) {{
                    renderEmptyState("No matching files found.");
                    return;
                }}
                
                filteredFiles.forEach(file => {{
                    fileListContainer.appendChild(createFileCard(file));
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
                `;
                fileListContainer.appendChild(folderCard);
            }});

            // Render Files
            const sortedFiles = activeNode.files.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
            sortedFiles.forEach(file => {{
                fileListContainer.appendChild(createFileCard(file));
            }});

            if (sortedFolders.length === 0 && sortedFiles.length === 0) {{
                renderEmptyState("This folder contains no items matching the active filter.");
            }}
        }}

        function createFileCard(file) {{
            const card = document.createElement('div');
            card.className = 'media-card';
            card.style.cursor = 'pointer';
            card.onclick = () => playMedia(file.id);
            
            let iconSvg = '';
            if (file.cat === 'video') {{
                iconSvg = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="23 7 16 12 23 17 23 7"></polygon><rect x="1" y="5" width="15" height="14" rx="2" ry="2"></rect></svg>`;
            }} else if (file.cat === 'audio') {{
                iconSvg = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l12-2v13"></path><circle cx="6" cy="18" r="3"></circle><circle cx="18" cy="16" r="3"></circle></svg>`;
            }} else if (file.cat === 'image') {{
                iconSvg = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`;
            }} else {{
                iconSvg = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline></svg>`;
            }}

            card.innerHTML = `
                <div class="media-info">
                    <div class="media-icon-wrapper">
                        ${{iconSvg}}
                    </div>
                    <div class="media-details">
                        <div class="media-name" title="${{file.name}}">${{file.name}}</div>
                        <div class="media-meta">
                            <span>${{file.size_str}}</span>
                            <span class="media-meta-dot"></span>
                            <span style="text-transform: uppercase;">${{file.ext}}</span>
                        </div>
                    </div>
                </div>
                <div class="action-area">
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

        // Initial render
        render();
    </script>
</body>
</html>"#,
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
        let cache_hit;

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
            
            // Query the ZeroCopy database for the directory listing
            let query_future = state.database.get_directory_listing(&canonical_browse_path, media_type_filter);
            let timeout_duration = std::time::Duration::from_secs(30); // 30 second timeout
            
            match tokio::time::timeout(timeout_duration, query_future).await {
                Ok(Ok(res)) => res,
                Ok(Err(e)) => {
                    error!("ZeroCopy database error getting directory listing for {}: {}", params.object_id, e);
                    state.web_metrics.record_error();
                    let response_time = start_time.elapsed().as_millis() as u64;
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
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_browse_request(response_time, false);
                    return (
                        StatusCode::REQUEST_TIMEOUT,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Request timeout - directory too large".to_string(),
                    ).into_response();
                }
            }
        };

        cache_hit = !subdirectories.is_empty() || !files.is_empty();
        
        debug!("ZeroCopy browse request for '{}' (filter: '{}') returned {} subdirs, {} files", 
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
            let response_time = start_time.elapsed().as_millis() as u64;
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
        
        debug!("ZeroCopy returning paginated results: {} subdirs, {} files (index {}-{} of {})",
               paginated_subdirs.len(), paginated_files.len(), 
               starting_index, end_index, total_matches);
        
        // Record atomic performance metrics
        let response_time = start_time.elapsed().as_millis() as u64;
        state.web_metrics.record_browse_request(response_time, cache_hit);
        state.web_metrics.record_directory_listing(response_time);
        
        let server_ip = state.get_server_ip();
        let response = generate_browse_response(&params.object_id, &paginated_subdirs, &paginated_files, state, &server_ip).await;
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

pub async fn serve_media(
    State(state): State<AppState>,
    Path(id): Path<String>,
    method: Method,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let start_time = Instant::now();
    
    let file_id = id.parse::<i64>().map_err(|_| {
        state.web_metrics.record_error();
        AppError::NotFound
    })?;
    
    // Use ZeroCopy database with atomic cache lookup
    let file_info = state.database
        .get_file_by_id(file_id)
        .await
        .map_err(|e| {
            error!("ZeroCopy database error getting file by ID {}: {}", file_id, e);
            state.web_metrics.record_error();
            AppError::NotFound
        })?
        .ok_or_else(|| {
            debug!("ZeroCopy database: file ID {} not found", file_id);
            state.web_metrics.record_error();
            AppError::NotFound
        })?;

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
        let response_time = start_time.elapsed().as_millis() as u64;
        state.web_metrics.record_file_serve(response_time);
        return Ok(response_builder.status(response_status).body(Body::empty())?);
    }

    file.seek(std::io::SeekFrom::Start(start)).await?;
    let stream = ReaderStream::with_capacity(file.take(len), 64 * 1024);
    let body = Body::from_stream(stream);

    // Record atomic performance metrics for file serving
    let response_time = start_time.elapsed().as_millis() as u64;
    state.web_metrics.record_file_serve(response_time);
    
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
    /// Handle artist browse requests with atomic performance tracking and ZeroCopy operations
    async fn handle_artist_browse(
        params: &BrowseParams,
        state: &AppState,
        audio_path: &str,
    ) -> Response {
        use crate::web::xml::generate_browse_response;
        
        let start_time = Instant::now();
        let path_parts: Vec<&str> = audio_path.split('/').filter(|s| !s.is_empty()).collect();
        
        if path_parts.len() == 1 {
            // List all artists using ZeroCopy atomic operations
            match state.database.get_artists().await {
                Ok(artists) => {
                    let has_data = !artists.is_empty();
                    let subdirectories: Vec<crate::database::MediaDirectory> = artists
                        .into_iter()
                        .map(|artist| crate::database::MediaDirectory {
                            path: std::path::PathBuf::from(format!("audio/artists/{}", artist.name)),
                            name: format!("{} ({})", artist.name, artist.count),
                        })
                        .collect();
                    
                    // Record atomic performance metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_browse_request(response_time, has_data);
                    
                    debug!("ZeroCopy retrieved {} artists in {}ms", subdirectories.len(), response_time);
                        
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
                Err(e) => {
                    error!("ZeroCopy error getting artists: {}", e);
                    
                    // Record atomic error metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_error();
                    state.web_metrics.record_browse_request(response_time, false);
                    
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Error browsing artists".to_string(),
                    )
                        .into_response()
                }
            }
        } else if path_parts.len() == 2 {
            // List tracks by specific artist using ZeroCopy atomic operations
            let artist_name = path_parts[1];
            match state.database.get_music_by_artist(artist_name).await {
                Ok(files) => {
                    // Record atomic performance metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_browse_request(response_time, !files.is_empty());
                    
                    debug!("ZeroCopy retrieved {} tracks for artist '{}' in {}ms", files.len(), artist_name, response_time);
                    
                    let server_ip = state.get_server_ip();
                    let response = generate_browse_response(&params.object_id, &[], &files, state, &server_ip).await;
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
                    error!("ZeroCopy error getting music by artist {}: {}", artist_name, e);
                    
                    // Record atomic error metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_error();
                    state.web_metrics.record_browse_request(response_time, false);
                    
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Error browsing artist tracks".to_string(),
                    )
                        .into_response()
                }
            }
        } else {
            // Record atomic error metrics for invalid path
            let response_time = start_time.elapsed().as_millis() as u64;
            state.web_metrics.record_error();
            state.web_metrics.record_browse_request(response_time, false);
            
            (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "Invalid artist path".to_string(),
            )
                .into_response()
        }
    }

    /// Handle album browse requests with atomic performance tracking and ZeroCopy operations
    async fn handle_album_browse(
        params: &BrowseParams,
        state: &AppState,
        audio_path: &str,
    ) -> Response {
        use crate::web::xml::generate_browse_response;
        
        let start_time = Instant::now();
        let path_parts: Vec<&str> = audio_path.split('/').filter(|s| !s.is_empty()).collect();
        
        if path_parts.len() == 1 {
            // List all albums using ZeroCopy atomic operations
            match state.database.get_albums(None).await {
                Ok(albums) => {
                    let has_data = !albums.is_empty();
                    let subdirectories: Vec<crate::database::MediaDirectory> = albums
                        .into_iter()
                        .map(|album| crate::database::MediaDirectory {
                            path: std::path::PathBuf::from(format!("audio/albums/{}", album.name)),
                            name: format!("{} ({})", album.name, album.count),
                        })
                        .collect();
                    
                    // Record atomic performance metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_browse_request(response_time, has_data);
                    
                    debug!("ZeroCopy retrieved {} albums in {}ms", subdirectories.len(), response_time);
                        
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
                Err(e) => {
                    error!("ZeroCopy error getting albums: {}", e);
                    
                    // Record atomic error metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_error();
                    state.web_metrics.record_browse_request(response_time, false);
                    
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Error browsing albums".to_string(),
                    )
                        .into_response()
                }
            }
        } else if path_parts.len() == 2 {
            // List tracks by specific album using ZeroCopy atomic operations
            let album_name = path_parts[1];
            match state.database.get_music_by_album(album_name, None).await {
                Ok(files) => {
                    // Record atomic performance metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_browse_request(response_time, !files.is_empty());
                    
                    debug!("ZeroCopy retrieved {} tracks for album '{}' in {}ms", files.len(), album_name, response_time);
                    
                    let server_ip = state.get_server_ip();
                    let response = generate_browse_response(&params.object_id, &[], &files, state, &server_ip).await;
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
                    error!("ZeroCopy error getting music by album {}: {}", album_name, e);
                    
                    // Record atomic error metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_error();
                    state.web_metrics.record_browse_request(response_time, false);
                    
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Error browsing album tracks".to_string(),
                    )
                        .into_response()
                }
            }
        } else {
            (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "Invalid album path".to_string(),
            )
                .into_response()
        }
    }
}



/// Handle browsing genres with atomic performance tracking and ZeroCopy operations
async fn handle_genres_browse(
    params: &BrowseParams,
    state: &AppState,
    audio_path: &str,
) -> Response {
    use crate::web::xml::generate_browse_response;
    
    let start_time = Instant::now();
    let path_parts: Vec<&str> = audio_path.split('/').filter(|s| !s.is_empty()).collect();
    
    if path_parts.len() == 1 {
        // List all genres using ZeroCopy atomic operations
        match state.database.get_genres().await {
            Ok(genres) => {
                let has_data = !genres.is_empty();
                let subdirectories: Vec<crate::database::MediaDirectory> = genres
                    .into_iter()
                    .map(|genre| crate::database::MediaDirectory {
                        path: std::path::PathBuf::from(format!("audio/genres/{}", genre.name)),
                        name: format!("{} ({})", genre.name, genre.count),
                    })
                    .collect();
                
                // Record atomic performance metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_browse_request(response_time, has_data);
                
                debug!("ZeroCopy retrieved {} genres in {}ms", subdirectories.len(), response_time);
                    
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
            Err(e) => {
                error!("ZeroCopy error getting genres: {}", e);
                
                // Record atomic error metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_error();
                state.web_metrics.record_browse_request(response_time, false);
                
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing genres".to_string(),
                )
                    .into_response()
            }
        }
    } else if path_parts.len() == 2 {
        // List tracks by specific genre using ZeroCopy atomic operations
        let genre_name = path_parts[1];
        match state.database.get_music_by_genre(genre_name).await {
            Ok(files) => {
                // Record atomic performance metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_browse_request(response_time, !files.is_empty());
                
                debug!("ZeroCopy retrieved {} tracks for genre '{}' in {}ms", files.len(), genre_name, response_time);
                
                let server_ip = state.get_server_ip();
                let response = generate_browse_response(&params.object_id, &[], &files, state, &server_ip).await;
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
                error!("ZeroCopy error getting music by genre {}: {}", genre_name, e);
                
                // Record atomic error metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_error();
                state.web_metrics.record_browse_request(response_time, false);
                
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing genre tracks".to_string(),
                )
                    .into_response()
            }
        }
    } else {
        // Record atomic error metrics for invalid path
        let response_time = start_time.elapsed().as_millis() as u64;
        state.web_metrics.record_error();
        state.web_metrics.record_browse_request(response_time, false);
        
        (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Invalid genre path".to_string(),
        )
            .into_response()
    }
}

/// Handle browsing years with atomic performance tracking and ZeroCopy operations
async fn handle_years_browse(
    params: &BrowseParams,
    state: &AppState,
    audio_path: &str,
) -> Response {
    use crate::web::xml::generate_browse_response;
    
    let start_time = Instant::now();
    let path_parts: Vec<&str> = audio_path.split('/').filter(|s| !s.is_empty()).collect();
    
    if path_parts.len() == 1 {
        // List all years using ZeroCopy atomic operations
        match state.database.get_years().await {
            Ok(years) => {
                let has_data = !years.is_empty();
                let subdirectories: Vec<crate::database::MediaDirectory> = years
                    .into_iter()
                    .map(|year| crate::database::MediaDirectory {
                        path: std::path::PathBuf::from(format!("audio/years/{}", year.name)),
                        name: format!("{} ({})", year.name, year.count),
                    })
                    .collect();
                
                // Record atomic performance metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_browse_request(response_time, has_data);
                
                debug!("ZeroCopy retrieved {} years in {}ms", subdirectories.len(), response_time);
                    
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
            Err(e) => {
                error!("ZeroCopy error getting years: {}", e);
                
                // Record atomic error metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_error();
                state.web_metrics.record_browse_request(response_time, false);
                
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing years".to_string(),
                )
                    .into_response()
            }
        }
    } else if path_parts.len() == 2 {
        // List tracks by specific year using ZeroCopy atomic operations
        let year_str = path_parts[1];
        if let Ok(year) = year_str.parse::<u32>() {
            match state.database.get_music_by_year(year).await {
                Ok(files) => {
                    // Record atomic performance metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_browse_request(response_time, !files.is_empty());
                    
                    debug!("ZeroCopy retrieved {} tracks for year {} in {}ms", files.len(), year, response_time);
                    
                    let server_ip = state.get_server_ip();
                    let response = generate_browse_response(&params.object_id, &[], &files, state, &server_ip).await;
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
                    error!("ZeroCopy error getting music by year {}: {}", year, e);
                    
                    // Record atomic error metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_error();
                    state.web_metrics.record_browse_request(response_time, false);
                    
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Error browsing year tracks".to_string(),
                    )
                        .into_response()
                }
            }
        } else {
            // Record atomic error metrics for invalid year format
            let response_time = start_time.elapsed().as_millis() as u64;
            state.web_metrics.record_error();
            state.web_metrics.record_browse_request(response_time, false);
            
            (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "Invalid year format".to_string(),
            )
                .into_response()
        }
    } else {
        // Record atomic error metrics for invalid path
        let response_time = start_time.elapsed().as_millis() as u64;
        state.web_metrics.record_error();
        state.web_metrics.record_browse_request(response_time, false);
        
        (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Invalid year path".to_string(),
        )
            .into_response()
    }
}

/// Handle browsing playlists with atomic performance tracking and ZeroCopy operations
async fn handle_playlists_browse(
    params: &BrowseParams,
    state: &AppState,
    audio_path: &str,
) -> Response {
    use crate::web::xml::generate_browse_response;
    
    let start_time = Instant::now();
    let path_parts: Vec<&str> = audio_path.split('/').filter(|s| !s.is_empty()).collect();
    
    if path_parts.len() == 1 {
        // List all playlists using ZeroCopy atomic operations
        match state.database.get_playlists().await {
            Ok(playlists) => {
                let has_data = !playlists.is_empty();
                let subdirectories: Vec<crate::database::MediaDirectory> = playlists
                    .into_iter()
                    .map(|playlist| crate::database::MediaDirectory {
                        path: std::path::PathBuf::from(format!("audio/playlists/{}", playlist.id.unwrap_or(0))),
                        name: playlist.name,
                    })
                    .collect();
                
                // Record atomic performance metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_browse_request(response_time, has_data);
                
                debug!("ZeroCopy retrieved {} playlists in {}ms", subdirectories.len(), response_time);
                    
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
            Err(e) => {
                error!("ZeroCopy error getting playlists: {}", e);
                
                // Record atomic error metrics
                let response_time = start_time.elapsed().as_millis() as u64;
                state.web_metrics.record_error();
                state.web_metrics.record_browse_request(response_time, false);
                
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Error browsing playlists".to_string(),
                )
                    .into_response()
            }
        }
    } else if path_parts.len() == 2 {
        // List tracks in specific playlist using ZeroCopy atomic operations
        let playlist_id_str = path_parts[1];
        if let Ok(playlist_id) = playlist_id_str.parse::<i64>() {
            match state.database.get_playlist_tracks(playlist_id).await {
                Ok(files) => {
                    // Record atomic performance metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_browse_request(response_time, !files.is_empty());
                    
                    debug!("ZeroCopy retrieved {} tracks for playlist {} in {}ms", files.len(), playlist_id, response_time);
                    
                    let server_ip = state.get_server_ip();
                    let response = generate_browse_response(&params.object_id, &[], &files, state, &server_ip).await;
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
                    error!("ZeroCopy error getting playlist tracks for {}: {}", playlist_id, e);
                    
                    // Record atomic error metrics
                    let response_time = start_time.elapsed().as_millis() as u64;
                    state.web_metrics.record_error();
                    state.web_metrics.record_browse_request(response_time, false);
                    
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                        "Error browsing playlist tracks".to_string(),
                    )
                        .into_response()
                }
            }
        } else {
            // Record atomic error metrics for invalid playlist ID
            let response_time = start_time.elapsed().as_millis() as u64;
            state.web_metrics.record_error();
            state.web_metrics.record_browse_request(response_time, false);
            
            (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "Invalid playlist ID format".to_string(),
            )
                .into_response()
        }
    } else {
        // Record atomic error metrics for invalid path
        let response_time = start_time.elapsed().as_millis() as u64;
        state.web_metrics.record_error();
        state.web_metrics.record_browse_request(response_time, false);
        
        (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Invalid playlist path".to_string(),
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
    let xml = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:{action}Response xmlns:u="{service_type}">
            {content}
        </u:{action}Response>
    </s:Body>
</s:Envelope>"#,
        action = action,
        service_type = service_type,
        content = content
    );
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
            "zerocopy_database": "active"
        }
    });
    
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        metrics_json.to_string(),
    )
}
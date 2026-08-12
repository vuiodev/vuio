let lastBytes = null;
let lastTime = null;

function formatBytes(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

function updateStatusBadge(isOnline) {
    const badge = document.getElementById('server-status-badge');
    const dot = document.getElementById('server-status-dot');
    const text = document.getElementById('server-status-text');
    if (!badge || !dot || !text) return;
    if (isOnline) {
        badge.style.color = '#10b981';
        dot.style.background = '#10b981';
        dot.style.boxShadow = '0 0 8px #10b981';
        text.textContent = 'Online';
    } else {
        badge.style.color = '#ef4444';
        dot.style.background = '#ef4444';
        dot.style.boxShadow = '0 0 8px #ef4444';
        text.textContent = 'Offline';
    }
}

async function checkServerStatus() {
    try {
        const res = await fetch('/healthz');
        updateStatusBadge(res.ok);
    } catch (err) {
        updateStatusBadge(false);
    }
}

// Run global heartbeat status check every 5 seconds
setInterval(checkServerStatus, 5000);

async function updateMetrics() {
    try {
        const res = await fetch('/metrics/json');
        if (!res.ok) {
            updateStatusBadge(false);
            return;
        }
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
        if (lastBytes !== null && lastTime !== null) {
            const deltaBytes = currentBytes - lastBytes;
            const deltaTimeSeconds = (currentTime - lastTime) / 1000.0;
            if (deltaTimeSeconds > 0) {
                const speedBps = (deltaBytes * 8) / deltaTimeSeconds;
                const speedMbps = speedBps / 1000000.0;
                document.getElementById('web-speed').textContent = Math.round(speedMbps) + ' Mbps';
            }
        } else {
            document.getElementById('web-speed').textContent = '0 Mbps';
        }
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
        if (activeCastsContainer) {
            const casts = data.active_casts || {};
            const castKeys = Object.keys(casts);
            activeCastsContainer.replaceChildren();
            
            if (castKeys.length === 0) {
                const empty = document.createElement('div');
                empty.style.cssText = 'color: var(--text-secondary); font-style: italic; padding: 0.25rem 0;';
                empty.textContent = 'No active device streams.';
                activeCastsContainer.appendChild(empty);
            } else {
                castKeys.forEach(tv => {
                    const row = document.createElement('div');
                    row.style.cssText = 'display: flex; align-items: center; justify-content: space-between; padding: 0.6rem 0.85rem; background: rgba(255,255,255,0.02); border: 1px solid var(--card-border); border-radius: 10px; margin-bottom: 0.25rem;';
                    const renderer = document.createElement('div');
                    renderer.style.cssText = 'display: flex; align-items: center; gap: 0.65rem; font-weight: 600; color: var(--text-primary); min-width: 0; flex: 1;';
                    const dot = document.createElement('span');
                    dot.style.cssText = 'display: inline-block; width: 6px; height: 6px; background: var(--accent-color); border-radius: 50%; box-shadow: 0 0 6px var(--accent-color); flex-shrink: 0;';
                    const rendererName = document.createElement('span');
                    rendererName.style.cssText = 'overflow: hidden; text-overflow: ellipsis; white-space: nowrap;';
                    rendererName.textContent = tv;
                    renderer.append(dot, rendererName);
                    const mediaName = document.createElement('div');
                    mediaName.style.cssText = 'color: var(--accent-color); font-weight: 600; text-align: right; max-width: 65%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-left: 0.75rem;';
                    mediaName.textContent = String(casts[tv] ?? '');
                    mediaName.title = String(casts[tv] ?? '');
                    row.append(renderer, mediaName);
                    activeCastsContainer.appendChild(row);
                });
            }
        }

    } catch (err) {
        console.error("Failed to fetch metrics:", err);
        updateStatusBadge(false);
    }
}

async function initializeDashboard() {
    try {
        const response = await fetch('/api/server-info');
        if (!response.ok) throw new Error('Server info request failed: ' + response.status);
        const info = await response.json();
        const serverName = typeof info.server_name === 'string' ? info.server_name : 'VuIO';
        monitoredDirs = Array.isArray(info.monitored_directories)
            ? info.monitored_directories.filter(path => typeof path === 'string')
            : [];
        document.title = serverName;
        document.getElementById('server-name').textContent = serverName;
    } catch (error) {
        console.error('Failed to load server information:', error);
    }
    await reloadMedia();
}

// Load runtime data through JSON APIs; no untrusted values are embedded
// in the HTML document.
initializeDashboard();

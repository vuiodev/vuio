let filesData = [];
let monitoredDirs = [];
let nextMediaCursor = null;
let mediaLoading = false;
let mediaLoadGeneration = 0;
let searchTimer = null;

// The library revision this view was built from. The server bumps its own
// whenever content changes — a directory added in Admin, a scan finishing, a
// file appearing on disk — so comparing the two tells us when what is on screen
// no longer matches what the server holds.
let libraryRevision = null;
let libraryWatchTimer = null;
const LIBRARY_POLL_MS = 5000;

let currentTab = 'video'; // Videos active by default!
let currentPath = [];
let searchQuery = '';

function setTab(tab) {
    currentTab = tab;
    document.querySelectorAll('.tab-btn').forEach(btn => {
        if (btn.dataset.tab === tab) {
            btn.classList.add('active');
        } else {
            btn.classList.remove('active');
        }
    });
    currentPath = [];
    reloadMedia();
}

function onSearch() {
    searchQuery = document.getElementById('search-input').value.toLowerCase();
    clearTimeout(searchTimer);
    searchTimer = setTimeout(reloadMedia, 250);
}

async function reloadMedia() {
    mediaLoadGeneration += 1;
    filesData = [];
    nextMediaCursor = null;
    await loadMoreMedia(true, mediaLoadGeneration);
}

// Fetch server state and reload the library if it has moved on.
//
// Returns the revision that is now on screen. `force` is for the initial load,
// where there is nothing to compare against yet.
async function syncLibrary(force = false) {
    let info;
    try {
        const response = await fetch('/api/server-info');
        if (!response.ok) throw new Error('Server info request failed: ' + response.status);
        info = await response.json();
    } catch (error) {
        console.error('Failed to load server information:', error);
        if (force) await reloadMedia();
        return libraryRevision;
    }

    const serverName = typeof info.server_name === 'string' ? info.server_name : 'VuIO';
    monitoredDirs = Array.isArray(info.monitored_directories)
        ? info.monitored_directories.filter(path => typeof path === 'string')
        : [];
    document.title = serverName;
    const nameElement = document.getElementById('server-name');
    if (nameElement) nameElement.textContent = serverName;

    const revision = typeof info.library_revision === 'number' ? info.library_revision : null;
    const changed = revision === null || revision !== libraryRevision;
    libraryRevision = revision;
    if (force || changed) {
        // Paging restarts from the top: the records a cursor pointed into may
        // not exist any more, so resuming from it could skip or repeat rows.
        await reloadMedia();
    }
    return libraryRevision;
}

// Keep the browse view current while it is the one being looked at.
//
// Indexing is asynchronous — adding a large folder returns long before its
// contents are in the database — so noticing the change once on the way in is
// not enough; the view has to keep watching until the scan lands.
function startLibraryWatch(checkNow = true) {
    stopLibraryWatch();
    if (checkNow) syncLibrary();
    libraryWatchTimer = setInterval(() => {
        // A dashboard left open on a background tab has nobody to show a stale
        // view to, and no reason to keep asking.
        if (!document.hidden) syncLibrary();
    }, LIBRARY_POLL_MS);
}

function stopLibraryWatch() {
    if (libraryWatchTimer) {
        clearInterval(libraryWatchTimer);
        libraryWatchTimer = null;
    }
}

async function loadMoreMedia(firstPage = false, generation = mediaLoadGeneration) {
    if (mediaLoading && !firstPage) return;
    mediaLoading = true;
    try {
        const params = new URLSearchParams({
            limit: '250',
            category: currentTab,
        });
        if (!firstPage && nextMediaCursor) params.set('cursor', nextMediaCursor);
        if (searchQuery) params.set('query', searchQuery);
        const response = await fetch('/api/media?' + params.toString());
        if (!response.ok) throw new Error('Media request failed: ' + response.status);
        const page = await response.json();
        if (generation !== mediaLoadGeneration) return;
        filesData = firstPage ? page.files : filesData.concat(page.files);
        nextMediaCursor = page.next_cursor;
        render();
    } catch (error) {
        console.error('Failed to load media:', error);
        if (filesData.length === 0) renderEmptyState('Could not load the media library.');
    } finally {
        mediaLoading = false;
    }
}

function appendLoadMore(container) {
    if (!nextMediaCursor) return;
    const button = document.createElement('button');
    button.className = 'btn-action';
    button.style.margin = '1rem auto';
    button.style.display = 'block';
    button.textContent = mediaLoading ? 'Loading…' : 'Load more';
    button.disabled = mediaLoading;
    button.onclick = () => loadMoreMedia();
    container.appendChild(button);
}

function playMedia(id) {
    window.location.href = '/media/' + id;
}

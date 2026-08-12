let filesData = [];
let monitoredDirs = [];
let nextMediaCursor = null;
let mediaLoading = false;
let mediaLoadGeneration = 0;
let searchTimer = null;

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

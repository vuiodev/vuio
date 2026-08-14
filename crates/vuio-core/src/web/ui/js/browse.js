// Speaker with waves: casting audio, as distinct from the TV glyph video uses.
const CAST_AUDIO_ICON = '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><path d="M15.54 8.46a5 5 0 0 1 0 7.07"></path><path d="M19.07 4.93a10 10 0 0 1 0 14.14"></path></svg>';

function getRelativeComponents(filePath) {
    let path = filePath.replace(/\\/g, '/');
    for (const dir of monitoredDirs) {
        let dirNorm = dir.replace(/\\/g, '/');
        if (!dirNorm.endsWith('/')) {
            dirNorm += '/';
        }
        if (path.startsWith(dirNorm)) {
            return path.substring(dirNorm.length).split('/').filter(p => p.length > 0);
        }
    }
    const parts = path.split('/').filter(p => p.length > 0);
    return parts.slice(-1);
}

function render() {
    const fileListContainer = document.getElementById('file-list');
    fileListContainer.replaceChildren();

    if (currentTab === 'image') {
        fileListContainer.className = 'image-grid';
    } else {
        fileListContainer.className = 'file-list';
    }

    // Filter files by tab and search
    let filteredFiles = filesData.filter(file => {
        const matchesSearch = searchQuery === ''
            || file.name.toLowerCase().includes(searchQuery)
            || (file.title || '').toLowerCase().includes(searchQuery)
            || (file.artist || '').toLowerCase().includes(searchQuery)
            || (file.album || '').toLowerCase().includes(searchQuery);
        if (currentTab === 'radio') {
            return file.cat === 'radio' && matchesSearch;
        }
        if (file.cat === 'radio') return false;
        
        const matchesTab = currentTab === 'all' || file.cat === currentTab;
        return matchesTab && matchesSearch;
    });

    // For radio tab, render a flat list of radio stations directly
    if (currentTab === 'radio') {
        document.getElementById('breadcrumbs').innerHTML = '<span>Internet Radio Stations</span>';
        if (filteredFiles.length === 0) {
            renderEmptyState("No radio stations configured.");
            appendLoadMore(fileListContainer);
            return;
        }
        filteredFiles.forEach(file => {
            fileListContainer.appendChild(createFileCard(file));
        });
        appendLoadMore(fileListContainer);
        return;
    }

    // If searching, show a flat search results view
    if (searchQuery !== '') {
        document.getElementById('breadcrumbs').innerHTML = '<span>Search Results</span>';
        
        if (filteredFiles.length === 0) {
            renderEmptyState("No matching files found.");
            appendLoadMore(fileListContainer);
            return;
        }
        
        filteredFiles.forEach(file => {
            if (currentTab === 'image' && file.cat === 'image') {
                fileListContainer.appendChild(createImageCard(file));
            } else {
                fileListContainer.appendChild(createFileCard(file));
            }
        });
        appendLoadMore(fileListContainer);
        return;
    }

    // Build directory tree
    const tree = { folders: {}, files: [] };
    
    filteredFiles.forEach(file => {
        const components = getRelativeComponents(file.path);
        let curr = tree;
        
        for (let i = 0; i < components.length - 1; i++) {
            const folderName = components[i];
            if (!curr.folders[folderName]) {
                curr.folders[folderName] = { folders: {}, files: [] };
            }
            curr = curr.folders[folderName];
        }
        
        curr.files.push(file);
    });

    // Navigate tree to currentPath
    let activeNode = tree;
    for (const folder of currentPath) {
        if (activeNode.folders[folder]) {
            activeNode = activeNode.folders[folder];
        } else {
            currentPath = [];
            activeNode = tree;
            break;
        }
    }

    renderBreadcrumbs();

    // Parent Folder row
    if (currentPath.length > 0) {
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
    }

    // Render Subfolders
    const sortedFolders = Object.keys(activeNode.folders).sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()));
    sortedFolders.forEach(folderName => {
        const folderCard = document.createElement('div');
        folderCard.className = 'media-card folder-card';
        folderCard.style.cursor = 'pointer';
        folderCard.onclick = () => enterFolder(folderName);

        // Calculate video files in this folder to see if "Play on TV" is applicable
        const targetPath = [...currentPath, folderName];
        const folderContains = category => filesData.filter(file => {
            if (file.cat !== category) return false;
            const comps = getRelativeComponents(file.path);
            if (comps.length <= targetPath.length) return false;
            for (let i = 0; i < targetPath.length; i++) {
                if (comps[i] !== targetPath[i]) return false;
            }
            return true;
        });
        const folderVideos = folderContains('video');
        const folderTracks = folderContains('audio');

        folderCard.innerHTML = `
            <div class="media-info">
                <div class="media-icon-wrapper">
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>
                </div>
                <div class="media-details">
                    <div class="media-name"></div>
                    <div class="media-meta">Folder</div>
                </div>
            </div>
            <div class="folder-actions" style="display: flex; gap: 0.5rem;"></div>
        `;
        const folderLabel = folderCard.querySelector('.media-name');
        folderLabel.textContent = folderName;
        folderLabel.title = folderName;
        const actions = folderCard.querySelector('.folder-actions');

        if (currentTab === 'audio' || currentTab === 'all') {
            const playButton = document.createElement('button');
            playButton.className = 'btn-action';
            playButton.title = 'Play Folder Content';
            playButton.innerHTML = '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>';
            playButton.addEventListener('click', event => {
                event.stopPropagation();
                playFolder(folderName);
            });
            actions.appendChild(playButton);
        }

        if ((currentTab === 'audio' || currentTab === 'all') && folderTracks.length > 0) {
            const castButton = document.createElement('button');
            castButton.className = 'btn-action';
            castButton.title = 'Cast Folder to Device';
            castButton.style.color = 'var(--accent-color)';
            castButton.innerHTML = CAST_AUDIO_ICON;
            castButton.addEventListener('click', event => {
                event.stopPropagation();
                playAudioFolderOnTv(folderName);
            });
            actions.appendChild(castButton);
        }

        if ((currentTab === 'video' || currentTab === 'all') && folderVideos.length > 0) {
            const tvButton = document.createElement('button');
            tvButton.className = 'btn-action';
            tvButton.title = 'Play on TV';
            tvButton.style.color = 'var(--accent-color)';
            tvButton.innerHTML = '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect><line x1="8" y1="21" x2="16" y2="21"></line><line x1="12" y1="17" x2="12" y2="21"></line></svg>';
            tvButton.addEventListener('click', event => {
                event.stopPropagation();
                playVideoFolderOnTv(folderName);
            });
            actions.appendChild(tvButton);
        }
        fileListContainer.appendChild(folderCard);
    });

    // Render Files
    const sortedFiles = activeNode.files.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
    sortedFiles.forEach(file => {
        if (currentTab === 'image' && file.cat === 'image') {
            fileListContainer.appendChild(createImageCard(file));
        } else {
            fileListContainer.appendChild(createFileCard(file));
        }
    });

    if (sortedFolders.length === 0 && sortedFiles.length === 0) {
        renderEmptyState("This folder contains no items matching the active filter.");
        appendLoadMore(fileListContainer);
    } else {
        appendLoadMore(fileListContainer);
    }
}

function createFileCard(file) {
    const card = document.createElement('div');
    card.className = 'media-card';
    card.style.cursor = 'pointer';
    if (file.cat === 'audio' || file.cat === 'radio') {
        card.onclick = () => playAudioFile(file);
    } else if (file.cat === 'image') {
        card.onclick = () => openLightbox(file.id);
    } else if (file.cat === 'video') {
        card.onclick = () => openVideoPlayer(file);
    } else {
        card.onclick = () => playMedia(file.id);
    }
    
    let iconSvg = '';
    if (file.cat === 'video') {
        iconSvg = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="23 7 16 12 23 17 23 7"></polygon><rect x="1" y="5" width="15" height="14" rx="2" ry="2"></rect></svg>`;
    } else if (file.cat === 'audio') {
        iconSvg = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l12-2v13"></path><circle cx="6" cy="18" r="3"></circle><circle cx="18" cy="16" r="3"></circle></svg>`;
    } else if (file.cat === 'radio') {
        iconSvg = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--accent-color)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="2"></circle><path d="M16.24 7.76a6 6 0 0 1 0 8.49m-8.48-.01a6 6 0 0 1 0-8.49m11.31-2.82a10 10 0 0 1 0 14.14m-14.14 0a10 10 0 0 1 0-14.14"></path></svg>`;
    } else if (file.cat === 'image') {
        iconSvg = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`;
    } else {
        iconSvg = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline></svg>`;
    }

    card.innerHTML = `
        <div class="media-info">
            <div class="media-icon-wrapper"></div>
            <div class="media-details">
                <div class="media-name"></div>
                <div class="media-meta" style="margin-top: 0.25rem;">
                    <span class="media-size"></span>
                    <span class="media-meta-dot"></span>
                    <span class="media-extension" style="text-transform: uppercase;"></span>
                </div>
            </div>
        </div>
        <div class="action-area">
            <a class="btn-action media-download" title="Download File">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>
            </a>
        </div>
    `;
    card.querySelector('.media-icon-wrapper').innerHTML = iconSvg;
    const name = card.querySelector('.media-name');
    name.textContent = file.title || file.name;
    name.title = file.name;
    const details = card.querySelector('.media-details');
    const metadataParts = [file.artist, file.album].filter(Boolean);
    if (metadataParts.length > 0) {
        const metadata = document.createElement('div');
        metadata.className = 'media-artist-album';
        metadata.style.cssText = 'font-size: 0.8rem; color: var(--text-secondary); margin-top: 0.1rem;';
        metadata.textContent = metadataParts.join(' — ');
        details.insertBefore(metadata, details.querySelector('.media-meta'));
    }
    card.querySelector('.media-size').textContent = file.size_str;
    card.querySelector('.media-extension').textContent = file.ext;

    const actionArea = card.querySelector('.action-area');
    if (file.cat === 'audio' || file.cat === 'radio') {
        const playButton = document.createElement('button');
        playButton.className = 'btn-action';
        playButton.title = 'Play File';
        playButton.style.marginRight = '0.35rem';
        playButton.innerHTML = '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>';
        playButton.addEventListener('click', event => {
            event.stopPropagation();
            playAudioFile(file.id);
        });
        actionArea.prepend(playButton);
    }
    if (file.cat === 'audio') {
        const castButton = document.createElement('button');
        castButton.className = 'btn-action';
        castButton.title = 'Cast to Device';
        castButton.style.cssText = 'margin-right: 0.35rem; color: var(--accent-color);';
        castButton.innerHTML = CAST_AUDIO_ICON;
        castButton.addEventListener('click', event => {
            event.stopPropagation();
            playAudioFileOnTv(file);
        });
        actionArea.prepend(castButton);
    }
    if (file.cat === 'video') {
        const tvButton = document.createElement('button');
        tvButton.className = 'btn-action';
        tvButton.title = 'Play on TV';
        tvButton.style.cssText = 'margin-right: 0.35rem; color: var(--accent-color);';
        tvButton.innerHTML = '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect><line x1="8" y1="21" x2="16" y2="21"></line><line x1="12" y1="17" x2="12" y2="21"></line></svg>';
        tvButton.addEventListener('click', event => {
            event.stopPropagation();
            playVideoFileOnTv(file);
        });
        actionArea.prepend(tvButton);
    }

    const download = card.querySelector('.media-download');
    download.href = '/media/' + encodeURIComponent(file.id);
    download.download = file.name;
    download.addEventListener('click', event => event.stopPropagation());
    return card;
}

function renderBreadcrumbs() {
    const container = document.getElementById('breadcrumbs');
    container.replaceChildren();
    
    const rootSpan = document.createElement('span');
    rootSpan.className = 'breadcrumb-item';
    rootSpan.onclick = () => jumpToBreadcrumb(-1);
    rootSpan.textContent = 'Home';
    container.appendChild(rootSpan);
    
    currentPath.forEach((folder, idx) => {
        const separator = document.createElement('span');
        separator.className = 'breadcrumb-separator';
        separator.textContent = ' / ';
        container.appendChild(separator);
        
        const folderSpan = document.createElement('span');
        folderSpan.className = 'breadcrumb-item';
        folderSpan.onclick = () => jumpToBreadcrumb(idx);
        folderSpan.textContent = folder;
        container.appendChild(folderSpan);
    });
}

function jumpToBreadcrumb(idx) {
    currentPath = currentPath.slice(0, idx + 1);
    render();
}

function enterFolder(folderName) {
    currentPath.push(folderName);
    render();
}

function goBack() {
    currentPath.pop();
    render();
}

function renderEmptyState(message) {
    const container = document.getElementById('file-list');
    container.replaceChildren();
    container.innerHTML = `
        <div class="empty-state">
            <svg class="empty-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="8" x2="12" y2="12"></line><line x1="12" y1="16" x2="12.01" y2="16"></line></svg>
            <h3>No items found</h3>
            <p></p>
        </div>
    `;
    container.querySelector('p').textContent = message;
}

let imageList = [];
let currentImageIndex = -1;

function createImageCard(file) {
    const card = document.createElement('div');
    card.className = 'image-card';
    card.onclick = () => openLightbox(file.id);

    card.innerHTML = `
        <img loading="lazy">
        <div class="image-card-overlay">
            <div class="image-card-name"></div>
            <a class="image-card-download" title="Download">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>
            </a>
        </div>
    `;
    const image = card.querySelector('img');
    image.src = '/media/' + encodeURIComponent(file.id);
    image.alt = file.name;
    const name = card.querySelector('.image-card-name');
    name.textContent = file.name;
    name.title = file.name;
    const download = card.querySelector('.image-card-download');
    download.href = image.src;
    download.download = file.name;
    download.addEventListener('click', event => event.stopPropagation());
    return card;
}

function openLightbox(fileId) {
    // Construct a list of images matching current browse state
    let filteredImages = filesData.filter(f => f.cat === 'image');
    if (currentPath.length > 0 && searchQuery === '') {
        filteredImages = filteredImages.filter(f => {
            const comps = getRelativeComponents(f.path);
            if (comps.length <= currentPath.length) return false;
            for (let i = 0; i < currentPath.length; i++) {
                if (comps[i] !== currentPath[i]) return false;
            }
            return true;
        });
    } else if (searchQuery !== '') {
        filteredImages = filteredImages.filter(f => f.name.toLowerCase().includes(searchQuery));
    }

    // Sort alphabetically
    filteredImages.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));

    imageList = filteredImages;
    currentImageIndex = imageList.findIndex(f => f.id.toString() === fileId.toString());
    if (currentImageIndex === -1) {
        // fallback
        const file = filesData.find(f => f.id.toString() === fileId.toString());
        if (file) {
            imageList = [file];
            currentImageIndex = 0;
        } else {
            return;
        }
    }

    showLightboxImage();
}

function showLightboxImage() {
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
}

function handleLightboxKeydown(e) {
    if (e.key === 'ArrowRight') {
        showNextImage();
    } else if (e.key === 'ArrowLeft') {
        showPrevImage();
    } else if (e.key === 'Escape') {
        closeLightbox();
    }
}

function showNextImage() {
    if (imageList.length === 0) return;
    currentImageIndex = (currentImageIndex + 1) % imageList.length;
    showLightboxImage();
}

function showPrevImage() {
    if (imageList.length === 0) return;
    currentImageIndex = (currentImageIndex - 1 + imageList.length) % imageList.length;
    showLightboxImage();
}

function closeLightbox() {
    document.getElementById('image-lightbox').style.display = 'none';
    document.removeEventListener('keydown', handleLightboxKeydown);
}

let playlist = [];
let currentTrackIndex = -1;

function playAudioFile(fileOrId) {
    let targetFile = null;
    if (typeof fileOrId === 'string' || typeof fileOrId === 'number') {
        targetFile = filesData.find(f => f.id.toString() === fileOrId.toString());
    } else {
        targetFile = fileOrId;
    }

    if (!targetFile) return;

    // Generate a playlist of all audio files matching the current tab/filter
    let filteredAudio = filesData.filter(f => f.cat === currentTab);
    
    // If currently in a path/folder, filter playlist to the current path
    if (currentPath.length > 0 && searchQuery === '') {
        filteredAudio = filteredAudio.filter(f => {
            const comps = getRelativeComponents(f.path);
            if (comps.length <= currentPath.length) return false;
            for (let i = 0; i < currentPath.length; i++) {
                if (comps[i] !== currentPath[i]) return false;
            }
            return true;
        });
    } else if (searchQuery !== '') {
        filteredAudio = filteredAudio.filter(f => f.name.toLowerCase().includes(searchQuery));
    }

    // Sort playlist by name
    filteredAudio.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
    
    playlist = filteredAudio;
    currentTrackIndex = playlist.findIndex(f => f.id.toString() === targetFile.id.toString());
    if (currentTrackIndex === -1) {
        playlist = [targetFile];
        currentTrackIndex = 0;
    }

    loadAndPlayTrack();
}

function playFolder(folderName) {
    const targetPath = [...currentPath, folderName];
    
    // Filter all audio files that reside in targetPath or any subdirectory
    let folderAudio = filesData.filter(file => {
        if (file.cat !== 'audio') return false;
        const comps = getRelativeComponents(file.path);
        if (comps.length <= targetPath.length) return false;
        for (let i = 0; i < targetPath.length; i++) {
            if (comps[i] !== targetPath[i]) return false;
        }
        return true;
    });

    if (folderAudio.length === 0) return;

    // Sort playlist alphabetically by name
    folderAudio.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));

    playlist = folderAudio;
    currentTrackIndex = 0;
    loadAndPlayTrack();
}

function loadAndPlayTrack() {
    if (currentTrackIndex < 0 || currentTrackIndex >= playlist.length) return;
    const file = playlist[currentTrackIndex];

    const playerBar = document.getElementById('audio-player-bar');
    playerBar.style.display = 'flex';

    document.getElementById('player-title').textContent = file.title || file.name;
    let metaText = 'Unknown Artist';
    if (file.artist) {
        metaText = file.artist;
        if (file.album) {
            metaText += ' — ' + file.album;
        }
    } else if (file.album) {
        metaText = file.album;
    }
    document.getElementById('player-subtitle').textContent = metaText;
    document.getElementById('player-track-count').textContent = (currentTrackIndex + 1) + '/' + playlist.length;

    const audioEl = document.getElementById('audio-element');
    resetAudioBufferProgress();
    audioEl.src = '/media/' + file.id;
    audioEl.play().then(() => {
        updatePlayPauseUI(true);
    }).catch(err => {
        console.error("Audio playback error:", err);
    });
}

function togglePlayPause() {
    const audioEl = document.getElementById('audio-element');
    if (audioEl.paused) {
        audioEl.play();
        updatePlayPauseUI(true);
    } else {
        audioEl.pause();
        updatePlayPauseUI(false);
    }
}

function updatePlayPauseUI(isPlaying) {
    const playIcon = document.getElementById('play-icon');
    const pauseIcon = document.getElementById('pause-icon');
    if (isPlaying) {
        playIcon.style.display = 'none';
        pauseIcon.style.display = 'block';
    } else {
        playIcon.style.display = 'block';
        pauseIcon.style.display = 'none';
    }
}

function playNext() {
    if (playlist.length === 0) return;
    currentTrackIndex = (currentTrackIndex + 1) % playlist.length;
    loadAndPlayTrack();
}

function playPrev() {
    if (playlist.length === 0) return;
    currentTrackIndex = (currentTrackIndex - 1 + playlist.length) % playlist.length;
    loadAndPlayTrack();
}

function onProgressSeek(percent) {
    const audioEl = document.getElementById('audio-element');
    if (audioEl.duration) {
        audioEl.currentTime = (percent / 100) * audioEl.duration;
    }
}

// Helper function to format time (e.g. 125 -> 2:05)
function formatPlayerTime(t) {
    const m = Math.floor(t / 60);
    const s = Math.floor(t % 60).toString().padStart(2, '0');
    return m + ':' + s;
}

function onVolumeChange(vol) {
    const audioEl = document.getElementById('audio-element');
    audioEl.volume = vol / 100;
}

function closePlayer() {
    const audioEl = document.getElementById('audio-element');
    audioEl.pause();
    audioEl.src = '';
    resetAudioBufferProgress();
    document.getElementById('audio-player-bar').style.display = 'none';
}

// Show the range that is useful at the current playhead. After a seek the old
// range remains at buffered[0], so using end(0) would leave the cache display
// stuck behind playback even while the browser fills a new range.
function syncAudioBufferProgress() {
    if (!audioEl.duration || !Number.isFinite(audioEl.duration)) {
        resetAudioBufferProgress();
        return;
    }

    const playhead = audioEl.currentTime;
    let bufferedEnd = playhead;
    const tolerance = 0.25;
    for (let index = 0; index < audioEl.buffered.length; index++) {
        const start = audioEl.buffered.start(index);
        const end = audioEl.buffered.end(index);
        if (playhead >= start - tolerance && playhead <= end + tolerance) {
            bufferedEnd = Math.max(playhead, end);
            break;
        }
    }

    const playedPercent = Math.min(100, Math.max(0, playhead / audioEl.duration * 100));
    const bufferedPercent = Math.min(100, Math.max(playedPercent, bufferedEnd / audioEl.duration * 100));
    slider.style.setProperty('--played-percent', playedPercent + '%');
    slider.style.setProperty('--buffered-percent', bufferedPercent + '%');
}

function resetAudioBufferProgress() {
    const progressSlider = document.getElementById('player-progress-slider');
    progressSlider.style.setProperty('--played-percent', '0%');
    progressSlider.style.setProperty('--buffered-percent', '0%');
}

// Setup Audio Element event listeners
const audioEl = document.getElementById('audio-element');
const slider = document.getElementById('player-progress-slider');
const currentEl = document.getElementById('player-time-current');
const durationEl = document.getElementById('player-time-duration');

audioEl.addEventListener('timeupdate', () => {
    if (audioEl.duration) {
        const curTime = audioEl.currentTime;
        const durTime = audioEl.duration;
        slider.value = (curTime / durTime) * 100;
        currentEl.textContent = formatPlayerTime(curTime);
        durationEl.textContent = formatPlayerTime(durTime);
    }
    syncAudioBufferProgress();
});

for (const event of ['durationchange', 'loadedmetadata', 'progress', 'seeking', 'seeked']) {
    audioEl.addEventListener(event, syncAudioBufferProgress);
}

audioEl.addEventListener('ended', () => {
    playNext();
});

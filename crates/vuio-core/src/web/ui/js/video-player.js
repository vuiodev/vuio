// ---------------------------------------------------------------------------
// Video player (Plyr, plus hls.js for .m3u8 sources)
// ---------------------------------------------------------------------------

// Containers no browser can demux, whatever codecs sit inside them — this is a
// container limitation, not a codec one, so an .mkv holding plain H.264 is just
// as unplayable as one holding HEVC. Listed explicitly so the fallback panel
// appears without first spending a request. Every other extension is attempted
// and falls back through the <video> error event.
const BROWSER_UNPLAYABLE_VIDEO = new Set(['avi', 'wmv', 'flv', 'mpg', 'mpeg']);

const PLYR_VERSION = '3.8.4';
const HLS_VERSION = '1.6.17';
const VIDEO_FORWARD_BUFFER_SECONDS = 60;

let videoPlyr = null;
let videoHls = null;
let videoModalFile = null;
let hlsScriptPromise = null;
let videoBufferProgressTimer = null;

// hls.js is half a megabyte and no local file needs it, so it is fetched on
// first use rather than linked from <head>.
function loadHls() {
    if (!hlsScriptPromise) {
        hlsScriptPromise = new Promise((resolve, reject) => {
            const script = document.createElement('script');
            script.src = '/assets/hls.min.js?v=' + HLS_VERSION;
            script.onload = () => resolve(window.Hls);
            script.onerror = () => reject(new Error('Failed to load hls.js'));
            document.head.appendChild(script);
        });
    }
    return hlsScriptPromise;
}

function openVideoPlayer(file) {
    closeVideoPlayer();
    videoModalFile = file;

    const url = '/media/' + encodeURIComponent(file.id);
    document.getElementById('video-player-title').textContent = file.name;
    const download = document.getElementById('video-player-download');
    download.href = url;
    download.download = file.name;

    document.getElementById('video-modal').style.display = 'flex';
    document.addEventListener('keydown', handleVideoKeydown);

    const ext = (file.ext || '').toLowerCase();
    if (BROWSER_UNPLAYABLE_VIDEO.has(ext)) {
        showVideoUnsupported(file);
        return;
    }

    const playUrl = ext === 'mkv' ? url + '/hls/master.m3u8' : url;
    mountVideoPlayer(file, playUrl);
}

async function mountVideoPlayer(file, url) {
    const stage = document.getElementById('video-stage');
    document.getElementById('video-unsupported').style.display = 'none';
    resetAudioTrackUi();
    stage.style.display = 'block';
    stage.style.removeProperty('--video-ar');

    const video = document.createElement('video');
    video.setAttribute('playsinline', '');
    video.setAttribute('controls', '');
    // Direct files are buffered by the browser rather than hls.js. `auto` is
    // the strongest standards-based request for read-ahead; the browser still
    // owns the exact native-media buffer size.
    video.preload = 'auto';
    video.addEventListener('error', () => showVideoUnsupported(file), { once: true });

    if (file.subs) {
        const track = document.createElement('track');
        track.kind = 'subtitles';
        track.srclang = 'en';
        track.label = 'English';
        // Not /subtitle — that endpoint serves raw SRT for TVs, and <track>
        // accepts WebVTT only.
        track.src = url + '/subtitle.vtt';
        video.appendChild(track);
    }
    stage.replaceChildren(video);

    if (/\.m3u8(\?|$)/i.test(url)) {
        if (!await attachHlsSource(video, url, file)) return;
    } else {
        video.src = url;
    }

    // The modal may have been closed while hls.js was loading.
    if (videoModalFile !== file) return;

    // A fresh instance per open, destroyed on close, rather than reusing one and
    // assigning `player.source`.
    videoPlyr = new Plyr(video, {
        iconUrl: '/assets/plyr.svg?v=' + PLYR_VERSION,
        // Both of these default to files on Plyr's CDN, which resolve to nothing
        // on an isolated LAN. blankVideo is what destroy() parks the element on
        // to drop the open connection.
        blankVideo: '/assets/blank.mp4',
        controls: ['play-large', 'play', 'progress', 'current-time', 'duration',
                   'mute', 'volume', 'captions', 'settings', 'pip', 'airplay', 'fullscreen'],
        settings: ['captions', 'speed'],
        speed: { selected: 1, options: [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2] },
        keyboard: { focused: true, global: false },
        captions: { active: Boolean(file.subs), update: true },
        storage: { enabled: true, key: 'vuio-plyr' },
        seekTime: 10,
    });
    setupVideoBufferProgress(video);

    videoPlyr.on('loadedmetadata', () => {
        if (video.videoWidth && video.videoHeight) {
            stage.style.setProperty('--video-ar', video.videoWidth / video.videoHeight);
        }
    });

    videoPlyr.once('ready', () => {
        // Autoplay with sound is blocked by default in most browsers; the
        // rejection is expected and leaves Plyr showing its play button.
        const started = videoPlyr.play();
        if (started && typeof started.catch === 'function') started.catch(() => {});
    });
}

async function attachHlsSource(video, url, file) {
    // Prefer hls.js/MSE whenever it's actually usable, rather than trusting
    // canPlayType('application/vnd.apple.mpegurl') to mean "this is Safari,
    // use native HLS": recent Chrome also answers non-empty there (some
    // versions report "maybe") but its native engine doesn't reliably play
    // this server's fMP4-based HLS — the <video> just hangs at readyState 0
    // with no error event. Native <video src> is now the true last resort,
    // for browsers where hls.js reports no MSE support at all (older Safari).
    try {
        const Hls = await loadHls();
        if (!Hls || !Hls.isSupported()) throw new Error('Media Source Extensions unavailable');
        videoHls = new Hls({
            enableWorker: true,
            maxBufferLength: VIDEO_FORWARD_BUFFER_SECONDS,
            maxMaxBufferLength: VIDEO_FORWARD_BUFFER_SECONDS,
        });
        videoHls.on(Hls.Events.ERROR, (event, data) => {
            if (data && data.fatal) showVideoUnsupported(file);
        });
        // MSE appends do not consistently produce a media `progress` event.
        // Keep the visible read-ahead in step with the real SourceBuffer.
        videoHls.on(Hls.Events.BUFFER_APPENDED, () => scheduleVideoBufferProgress(video));
        setupAudioTrackUi(videoHls, Hls);
        videoHls.loadSource(url);
        videoHls.attachMedia(video);
        return true;
    } catch (error) {
        if (video.canPlayType('application/vnd.apple.mpegurl')) {
            video.src = url;
            return true;
        }
        console.error('HLS playback unavailable:', error);
        showVideoUnsupported(file);
        return false;
    }
}

// Plyr derives its gray buffer bar from video.buffered.end(0). That works only
// until the first seek: browsers retain the old range at index 0 and append the
// new read-ahead as a later range, so playback is buffered correctly while the
// bar remains stuck at the old position. Follow the range containing the
// playhead instead. The played bar covers everything behind currentTime, making
// the remaining gray part an honest view of the useful forward buffer.
function setupVideoBufferProgress(video) {
    if (videoBufferProgressTimer) clearInterval(videoBufferProgressTimer);
    const update = () => scheduleVideoBufferProgress(video);
    for (const event of ['durationchange', 'loadedmetadata', 'progress',
                         'seeking', 'seeked', 'timeupdate']) {
        video.addEventListener(event, update);
    }
    // Native media can extend a range without emitting another `progress`
    // event, and Plyr may subsequently restore its end(0)-based value. Keep the
    // small display-only correction authoritative while this player is open.
    videoBufferProgressTimer = setInterval(() => syncVideoBufferProgress(video), 250);
    update();
}

function scheduleVideoBufferProgress(video) {
    requestAnimationFrame(() => syncVideoBufferProgress(video));
}

function syncVideoBufferProgress(video) {
    const progress = video.closest('.plyr')?.querySelector('.plyr__progress__buffer');
    if (!progress || !Number.isFinite(video.duration) || video.duration <= 0) return;

    const playhead = video.currentTime;
    let bufferedEnd = playhead;
    // Timestamp rounding at a segment boundary can put the playhead a few
    // milliseconds outside a TimeRange that is usable in practice.
    const tolerance = 0.25;
    for (let index = 0; index < video.buffered.length; index++) {
        const start = video.buffered.start(index);
        const end = video.buffered.end(index);
        if (playhead >= start - tolerance && playhead <= end + tolerance) {
            bufferedEnd = Math.max(playhead, end);
            break;
        }
    }

    progress.value = Math.min(100, Math.max(0, bufferedEnd / video.duration * 100));
}

// Populates the audio-track <select> once hls.js knows the master playlist's
function setupAudioTrackUi(hls, Hls) {
    const select = document.getElementById('video-audio-track-select');
    const note = document.getElementById('video-audio-unavailable-note');

    const refreshTracks = (tracks, currentTrack) => {
        if (videoHls !== hls) return;
        const list = tracks || hls.audioTracks || [];
        note.style.display = list.length === 0 ? 'block' : 'none';
        updateAudioTrackOptions(list, currentTrack !== undefined ? currentTrack : hls.audioTrack);
    };

    hls.on(Hls.Events.MANIFEST_PARSED, (event, data) => {
        refreshTracks(data.audioTracks, hls.audioTrack);
    });

    hls.on(Hls.Events.AUDIO_TRACKS_UPDATED, (event, data) => {
        refreshTracks(data.audioTracks, hls.audioTrack);
    });

    hls.on(Hls.Events.AUDIO_TRACK_SWITCHED, (event, data) => {
        if (videoHls !== hls) return;
        if (typeof data.id === 'number') {
            select.value = String(data.id);
        }
    });

    function updateAudioTrackOptions(tracks, activeIndex) {
        if (!tracks || tracks.length < 2) {
            select.style.display = 'none';
            select.replaceChildren();
            return;
        }
        select.replaceChildren(...tracks.map((track, idx) => {
            const option = document.createElement('option');
            option.value = String(idx);
            let label = track.name || track.lang || ('Audio Track ' + (idx + 1));
            option.textContent = label;
            option.selected = idx === (activeIndex >= 0 ? activeIndex : 0);
            return option;
        }));
        select.style.display = 'inline-block';
    }

    select.onchange = () => {
        if (videoHls === hls) {
            const targetIdx = Number(select.value);
            hls.audioTrack = targetIdx;
            const selectedText = select.options[select.selectedIndex]?.textContent || '';
            if (typeof showToast === 'function' && selectedText) {
                showToast(`Audio: ${selectedText}`);
            }
        }
    };
}

function resetAudioTrackUi() {
    const select = document.getElementById('video-audio-track-select');
    select.style.display = 'none';
    select.replaceChildren();
    select.onchange = null;
    document.getElementById('video-audio-unavailable-note').style.display = 'none';
}

function showVideoUnsupported(file) {
    if (!file) return;
    teardownVideoPlayback();

    const stage = document.getElementById('video-stage');
    stage.replaceChildren();
    stage.style.display = 'none';

    const extension = (file.ext || '').toLowerCase();
    document.getElementById('video-unsupported-detail').textContent = extension
        ? 'No browser can open the .' + extension + ' container. Download the file or send it to a device on your network.'
        : 'Your browser cannot play this file. Download it or send it to a device on your network.';

    const download = document.getElementById('video-unsupported-download');
    download.href = '/media/' + encodeURIComponent(file.id);
    download.download = file.name;

    document.getElementById('video-unsupported-cast').onclick = () => {
        closeVideoPlayer();
        playVideoFileOnTv(file);
    };

    document.getElementById('video-unsupported').style.display = 'flex';
}

// Tears down playback without touching modal visibility, so it can be reused by
// both the close path and the fall-back-to-unsupported path.
function teardownVideoPlayback() {
    resetAudioTrackUi();
    if (videoBufferProgressTimer) {
        clearInterval(videoBufferProgressTimer);
        videoBufferProgressTimer = null;
    }
    if (videoHls) {
        videoHls.destroy();
        videoHls = null;
    }
    if (videoPlyr) {
        try {
            videoPlyr.destroy();
        } catch (error) {
            console.warn('Plyr teardown failed:', error);
        }
        videoPlyr = null;
    }
    // destroy() restores the bare <video>; stop it holding the connection open.
    document.getElementById('video-stage').querySelectorAll('video').forEach(video => {
        video.pause();
        video.removeAttribute('src');
        video.load();
    });
}

function closeVideoPlayer() {
    document.removeEventListener('keydown', handleVideoKeydown);
    teardownVideoPlayback();
    document.getElementById('video-stage').replaceChildren();
    document.getElementById('video-unsupported').style.display = 'none';
    document.getElementById('video-modal').style.display = 'none';
    videoModalFile = null;
}

function handleVideoKeydown(e) {
    if (e.key !== 'Escape') return;
    // In fullscreen the browser spends Escape on leaving fullscreen; closing the
    // modal at the same time would drop the user two levels at once.
    if (videoPlyr && videoPlyr.fullscreen && videoPlyr.fullscreen.active) return;
    closeVideoPlayer();
}

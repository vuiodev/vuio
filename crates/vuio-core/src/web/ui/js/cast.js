function playVideoFolderOnTv(folderName) {
    const components = [...currentPath, folderName];
    discoverAndCast(
        { kind: 'folder', components, media: 'video' },
        folderName
    );
}

function playVideoFileOnTv(file) {
    discoverAndCast(
        { kind: 'file', file_id: file.id },
        file.title || file.name
    );
}

function playAudioFolderOnTv(folderName) {
    const components = [...currentPath, folderName];
    discoverAndCast(
        { kind: 'folder', components, media: 'audio' },
        folderName
    );
}

function playAudioFileOnTv(file) {
    discoverAndCast(
        { kind: 'file', file_id: file.id },
        file.title || file.name
    );
}

// A remembered device skips the picker. Session storage wins over local
// so "for this session" can temporarily override "always".
const PREFERRED_RENDERER_KEY = 'vuio.preferredRendererId';

function getPreferredRendererId() {
    try {
        return sessionStorage.getItem(PREFERRED_RENDERER_KEY)
            || localStorage.getItem(PREFERRED_RENDERER_KEY);
    } catch { return null; }
}

function setPreferredRendererId(id, scope) {
    try {
        if (scope === 'session') sessionStorage.setItem(PREFERRED_RENDERER_KEY, id);
        else if (scope === 'forever') localStorage.setItem(PREFERRED_RENDERER_KEY, id);
    } catch { /* private browsing: fall back to asking every time */ }
}

function clearPreferredRenderer() {
    try {
        sessionStorage.removeItem(PREFERRED_RENDERER_KEY);
        localStorage.removeItem(PREFERRED_RENDERER_KEY);
    } catch { /* nothing stored */ }
}

function selectedRememberScope() {
    const checked = document.querySelector('input[name="tv-remember"]:checked');
    return checked ? checked.value : 'none';
}

function resetRememberRadios() {
    const none = document.querySelector('input[name="tv-remember"][value="none"]');
    if (none) none.checked = true;
}

// The single path to playback: pairs first when the receiver needs it,
// so a remembered device cannot bypass the PIN step.
async function castWithPairing(renderer, source) {
    if (renderer.pairing === 'required') {
        const paired = await pairAirplayRenderer(renderer);
        if (!paired) return false;
        renderer.pairing = 'paired';
    }
    await castToRenderer(renderer, source);
    return true;
}

async function discoverAndCast(source, label, ignorePreferred = false) {
    showToast("Finding playback devices...", "info");
    try {
        const response = await fetch('/api/renderers');
        if (!response.ok) {
            throw new Error('Device discovery request failed: ' + response.status);
        }
        const renderers = await response.json();
        if (!Array.isArray(renderers) || renderers.length === 0) {
            showToast("No compatible playback devices found on your local network.", "error");
            return;
        }

        const preferredId = ignorePreferred ? null : getPreferredRendererId();
        const preferred = preferredId
            ? renderers.find(r => String(r.id) === String(preferredId))
            : null;
        if (preferred) {
            // Offer a way back to the picker, or the choice is a trap.
            showToast('Casting to ' + preferred.friendly_name + '. Click to pick a different device.', 'info', () => {
                clearPreferredRenderer();
                discoverAndCast(source, label, true);
            });
            await castWithPairing(preferred, source);
            return;
        }

        showRendererSelectionModal(renderers, label, source);
    } catch (error) {
        console.error("Playback device discovery failed", error);
        showToast("Failed to discover playback devices: " + error.message, "error");
    }
}

function showRendererSelectionModal(renderers, label, source) {
    document.getElementById('tv-modal-folder-name').textContent = label;
    resetRememberRadios();
    const container = document.getElementById('tv-list-container');
    container.replaceChildren();
    
    renderers.forEach(renderer => {
        const btn = document.createElement('button');
        btn.className = 'tv-select-btn';
        btn.onclick = async () => {
            container.querySelectorAll('button').forEach(button => button.disabled = true);
            const scope = selectedRememberScope();
            closeTvModal();
            const started = await castWithPairing(renderer, source);
            // Only remember a device that actually accepted the cast.
            if (started && scope !== 'none') {
                setPreferredRendererId(renderer.id, scope);
            }
        };
        btn.innerHTML = `
            <div class="tv-icon-wrapper">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect><line x1="8" y1="21" x2="16" y2="21"></line><line x1="12" y1="17" x2="12" y2="21"></line></svg>
                <span class="tv-friendly-name"></span>
                <span class="tv-protocol" style="font-size: 0.68rem; text-transform: uppercase; color: var(--text-secondary); border: 1px solid var(--card-border); border-radius: 999px; padding: 0.12rem 0.38rem;"></span>
            </div>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--accent-color)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg>
        `;
        btn.querySelector('.tv-friendly-name').textContent = renderer.friendly_name || 'Unknown renderer';
        const protocol = renderer.protocol || 'dlna';
        btn.querySelector('.tv-protocol').textContent = renderer.pairing === 'required'
            ? protocol + ' · PIN required'
            : protocol;
        container.appendChild(btn);
        if (renderer.protocol === 'airplay' && renderer.pairing === 'paired') {
            const forget = document.createElement('button');
            forget.type = 'button';
            forget.textContent = 'Forget saved AirPlay pairing';
            forget.style.cssText = 'background: transparent; border: 0; color: var(--text-secondary); cursor: pointer; font-size: 0.72rem; text-align: right; padding: 0 0.35rem;';
            forget.onclick = async () => {
                forget.disabled = true;
                try {
                    const response = await fetch('/api/renderers/pair/forget', {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ renderer_id: renderer.id })
                    });
                    const result = await response.json().catch(() => ({}));
                    if (!response.ok || result.error) {
                        throw new Error(result.error || 'Could not forget pairing');
                    }
                    renderer.pairing = 'required';
                    closeTvModal();
                    showToast('Saved AirPlay pairing removed.', 'success');
                } catch (error) {
                    forget.disabled = false;
                    showToast('Failed to remove pairing: ' + error.message, 'error');
                }
            };
            container.appendChild(forget);
        }
    });
    
    document.getElementById('tv-modal').style.display = 'flex';
}

function closeTvModal() {
    document.getElementById('tv-modal').style.display = 'none';
}

async function pairAirplayRenderer(renderer) {
    showToast("Starting secure AirPlay pairing with " + renderer.friendly_name + "...", "info");
    try {
        const startResponse = await fetch('/api/renderers/pair/start', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ renderer_id: renderer.id })
        });
        const challenge = await startResponse.json().catch(() => ({}));
        if (!startResponse.ok || challenge.error) {
            throw new Error(challenge.error || 'Pairing could not be started');
        }
        const pin = window.prompt(
            'Enter the AirPlay PIN displayed on "' + renderer.friendly_name + '":'
        );
        if (!pin) {
            showToast("AirPlay pairing cancelled.", "info");
            return false;
        }
        const finishResponse = await fetch('/api/renderers/pair/finish', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                renderer_id: renderer.id,
                challenge_id: challenge.id,
                pin: pin.trim()
            })
        });
        const result = await finishResponse.json().catch(() => ({}));
        if (!finishResponse.ok || result.error) {
            throw new Error(result.error || 'Pairing failed');
        }
        showToast("AirPlay pairing saved securely.", "success");
        return true;
    } catch (error) {
        console.error("AirPlay pairing failed", error);
        showToast("Failed to pair with device: " + error.message, "error");
        return false;
    }
}

// Remembered so playback can be stopped without re-picking the device.
let lastCastRenderer = null;

async function stopCurrentCast() {
    if (!lastCastRenderer) {
        showToast("Nothing is casting from this page.", "info");
        return;
    }
    const name = lastCastRenderer.friendly_name;
    const rendererId = lastCastRenderer.id;
    // Stopping is one-way: once asked, this page is no longer casting
    // regardless of what the receiver says, so clear the control first.
    document.getElementById('stop-cast-btn').style.display = 'none';
    lastCastRenderer = null;
    try {
        const response = await fetch('/api/cast/control', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ renderer_id: rendererId, action: 'stop' })
        });
        const data = await response.json().catch(() => ({}));
        if (!response.ok || data.error) {
            throw new Error(data.error || 'Stop request failed');
        }
        showToast("Stopped playback on " + name + ".", "success");
    } catch (error) {
        showToast("Stopped, but the receiver reported: " + error.message, "info");
    }
}

async function castToRenderer(renderer, source) {
    showToast("Casting to " + renderer.friendly_name + "...", "info");
    try {
        const response = await fetch('/api/cast', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ renderer_id: renderer.id, source })
        });
        const data = await response.json().catch(() => ({}));
        if (!response.ok || data.error) {
            throw new Error(data.error || 'Cast request failed: ' + response.status);
        }
        showToast("Successfully playing on " + renderer.friendly_name + "!", "success");
        lastCastRenderer = renderer;
        const stopButton = document.getElementById('stop-cast-btn');
        if (stopButton) stopButton.style.display = 'inline-flex';
    } catch (error) {
        console.error("Cast request failed", error);
        showToast("Failed to cast to device: " + error.message, "error");
    }
}

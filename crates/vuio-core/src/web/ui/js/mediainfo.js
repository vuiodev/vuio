// The MediaInfo panel: provider credentials, the library fetch, and its progress.
//
// This hangs off the Admin tab's schema-driven pane. The settings above it are
// ordinary config keys and render themselves from the spec the server sends; what
// is here cannot be, because credentials live in the secrets table rather than the
// config file, and a running job is not a setting at all.
//
// Same conventions as admin.js: build nodes with createElement and textContent,
// never innerHTML, and report failures through showToast.

let mediaInfoData = null;
let mediaInfoPollTimer = null;
let mediaInfoBusy = false;

// While a run is going the page polls for progress. One second matches the pace a
// person reads a counter at, and the endpoint is a few cheap queries.
const MEDIAINFO_POLL_MS = 1000;

async function loadMediaInfo() {
    const response = await fetch('/api/admin/mediainfo');
    if (!response.ok) throw new Error('Could not load media info status');
    mediaInfoData = await response.json();
    return mediaInfoData;
}

function stopMediaInfoPolling() {
    if (mediaInfoPollTimer) {
        clearInterval(mediaInfoPollTimer);
        mediaInfoPollTimer = null;
    }
}

function startMediaInfoPolling() {
    stopMediaInfoPolling();
    mediaInfoPollTimer = setInterval(async () => {
        // A hidden tab has nobody to show a counter to, and the run continues on
        // the server regardless.
        if (document.hidden) return;
        try {
            const data = await loadMediaInfo();
            renderAdmin();
            if (!data.job || !data.job.running) stopMediaInfoPolling();
        } catch (error) {
            // The server may be restarting or the session may have expired. Give
            // up quietly rather than filling the screen with toasts once a second.
            stopMediaInfoPolling();
        }
    }, MEDIAINFO_POLL_MS);
}

function mediaInfoPill(text, className) {
    const pill = document.createElement('span');
    pill.className = 'admin-pill ' + className;
    pill.textContent = text;
    return pill;
}

function renderProviderRow(provider) {
    const card = document.createElement('div');
    card.className = 'admin-library mediainfo-provider';

    const head = document.createElement('div');
    head.className = 'admin-library-head';

    const name = document.createElement('strong');
    name.textContent = provider.label;
    head.appendChild(name);

    if (!provider.needs_credential) {
        head.appendChild(mediaInfoPill('No account needed', 'admin-pill-next'));
    } else if (provider.has_credential) {
        head.appendChild(mediaInfoPill('Credential saved', 'admin-pill-next'));
    } else {
        head.appendChild(mediaInfoPill('Needs a credential', 'admin-pill-restart'));
    }
    if (!provider.enabled) {
        head.appendChild(mediaInfoPill('Not in use', 'admin-pill-unset'));
    }
    card.appendChild(head);

    const provides = document.createElement('p');
    provides.className = 'admin-section-blurb';
    provides.textContent = provider.provides;
    card.appendChild(provides);

    if (provider.needs_credential) {
        const row = document.createElement('div');
        row.className = 'mediainfo-credential';

        const input = document.createElement('input');
        input.type = 'password';
        input.className = 'admin-input';
        input.autocomplete = 'off';
        input.placeholder = provider.has_credential
            ? 'Saved — type a new one to replace it'
            : provider.credential_label;
        input.setAttribute('aria-label', provider.label + ' ' + provider.credential_label);

        const save = document.createElement('button');
        save.type = 'button';
        save.className = 'admin-btn admin-btn-primary';
        save.textContent = 'Save';
        save.onclick = () => saveMediaInfoCredential(provider.id, input.value, input);

        row.append(input, save);

        if (provider.has_credential) {
            const clear = document.createElement('button');
            clear.type = 'button';
            clear.className = 'admin-btn admin-btn-danger';
            clear.textContent = 'Clear';
            clear.onclick = () => saveMediaInfoCredential(provider.id, '', input);
            row.appendChild(clear);
        }
        card.appendChild(row);

        if (provider.signup_url) {
            const link = document.createElement('a');
            link.className = 'mediainfo-signup';
            link.href = provider.signup_url;
            link.target = '_blank';
            link.rel = 'noreferrer noopener';
            link.textContent = 'Get a free ' + provider.credential_label.toLowerCase();
            card.appendChild(link);
        }
    }

    return card;
}

async function saveMediaInfoCredential(providerId, token, input) {
    try {
        const response = await fetch('/api/admin/mediainfo/credentials', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ provider: providerId, token }),
        });
        const result = await response.json().catch(() => ({}));
        if (!response.ok || result.error) {
            throw new Error(result.error || 'Could not save the credential');
        }
        // Never keep the secret in the DOM after it has been sent.
        if (input) input.value = '';
        showToast(token.trim() ? 'Credential saved.' : 'Credential cleared.', 'success');
        await loadMediaInfo();
        renderAdmin();
    } catch (error) {
        showToast(error.message, 'error');
    }
}

function renderMediaInfoProgress(job) {
    const wrapper = document.createElement('div');
    wrapper.className = 'mediainfo-progress';

    const done = job.total > 0 ? Math.round((job.processed / job.total) * 100) : 0;

    const bar = document.createElement('div');
    bar.className = 'mediainfo-bar';
    const fill = document.createElement('div');
    fill.className = 'mediainfo-bar-fill';
    fill.style.width = done + '%';
    bar.appendChild(fill);
    wrapper.appendChild(bar);

    const line = document.createElement('div');
    line.className = 'mediainfo-counts';
    const counts = [
        job.processed + ' of ' + job.total + ' checked',
        job.matched + ' matched',
        job.low_confidence + ' uncertain',
        job.failed + ' failed',
    ];
    for (const text of counts) {
        const span = document.createElement('span');
        span.textContent = text;
        line.appendChild(span);
    }
    wrapper.appendChild(line);

    if (job.running && job.current) {
        const current = document.createElement('div');
        current.className = 'admin-section-blurb admin-mono';
        current.textContent = job.current;
        wrapper.appendChild(current);
    }

    if (!job.running && job.finished_at) {
        const summary = document.createElement('p');
        summary.className = 'admin-section-blurb';
        summary.textContent = job.cancelled
            ? 'Last run was cancelled.'
            : 'Last run finished.';
        wrapper.appendChild(summary);
    }

    if (job.last_error) {
        const error = document.createElement('p');
        error.className = 'admin-section-blurb mediainfo-error';
        error.textContent = 'Most recent error: ' + job.last_error;
        wrapper.appendChild(error);
    }

    return wrapper;
}

function renderMediaInfoFlagged(flagged) {
    const details = document.createElement('details');
    details.className = 'mediainfo-flagged';

    const summary = document.createElement('summary');
    summary.textContent = flagged.length + ' uncertain match' + (flagged.length === 1 ? '' : 'es');
    details.appendChild(summary);

    const list = document.createElement('dl');
    list.className = 'admin-runtime';
    for (const item of flagged) {
        const term = document.createElement('dt');
        term.textContent = item.filename || ('#' + item.media_file_id);
        const value = document.createElement('dd');
        value.textContent =
            (item.matched_title || 'no match') + ' — ' + item.confidence + '% via ' + item.provider;
        list.append(term, value);
    }
    details.appendChild(list);
    return details;
}

async function startMediaInfoFetch() {
    if (mediaInfoBusy) return;
    mediaInfoBusy = true;
    try {
        const response = await fetch('/api/admin/mediainfo/run', { method: 'POST' });
        const result = await response.json().catch(() => ({}));
        if (!response.ok || result.error) {
            throw new Error(result.error || 'Could not start the fetch');
        }
        if (result.total === 0) {
            showToast('Everything in the library already has media info.', 'info');
        } else {
            showToast('Fetching media info for ' + result.total + ' items.', 'info');
        }
        await loadMediaInfo();
        renderAdmin();
        startMediaInfoPolling();
    } catch (error) {
        showToast(error.message, 'error');
    } finally {
        mediaInfoBusy = false;
    }
}

async function cancelMediaInfoFetch() {
    try {
        const response = await fetch('/api/admin/mediainfo/cancel', { method: 'POST' });
        const result = await response.json().catch(() => ({}));
        if (!response.ok || result.error) {
            throw new Error(result.error || 'Could not cancel the fetch');
        }
        // The run stops between items, so the counters keep moving briefly.
        showToast('Stopping after the item in flight.', 'info');
        await loadMediaInfo();
        renderAdmin();
    } catch (error) {
        showToast(error.message, 'error');
    }
}

function renderMediaInfoPanel(body) {
    if (!mediaInfoData) {
        const loading = document.createElement('div');
        loading.className = 'admin-loading';
        loading.textContent = 'Loading providers…';
        body.appendChild(loading);
        return;
    }

    const job = mediaInfoData.job || {};
    const stats = mediaInfoData.stats || {};

    const heading = document.createElement('div');
    heading.className = 'admin-section-title';
    heading.textContent = 'Providers';
    body.appendChild(heading);

    // Grouped by what they cover, so the three domains read as three lists rather
    // than one alphabetical run of ten names.
    const groups = new Map();
    for (const provider of mediaInfoData.providers || []) {
        if (!groups.has(provider.group)) groups.set(provider.group, []);
        groups.get(provider.group).push(provider);
    }
    for (const [group, providers] of groups) {
        const label = document.createElement('div');
        label.className = 'mediainfo-group';
        label.textContent = group;
        body.appendChild(label);
        for (const provider of providers) {
            body.appendChild(renderProviderRow(provider));
        }
    }

    const actionsHeading = document.createElement('div');
    actionsHeading.className = 'admin-section-title';
    actionsHeading.textContent = 'Fetch';
    body.appendChild(actionsHeading);

    const summary = document.createElement('p');
    summary.className = 'admin-section-blurb';
    summary.textContent =
        (stats.total || 0) + ' items have media info, ' +
        (stats.low_confidence || 0) + ' of them uncertain, ' +
        (stats.with_artwork || 0) + ' with artwork.';
    body.appendChild(summary);

    const actions = document.createElement('div');
    actions.className = 'mediainfo-actions';

    const fetchButton = document.createElement('button');
    fetchButton.type = 'button';
    fetchButton.className = 'admin-btn admin-btn-primary';
    fetchButton.textContent = 'Fetch media info for entire library';
    fetchButton.disabled = job.running || !mediaInfoData.enabled;
    fetchButton.onclick = startMediaInfoFetch;
    actions.appendChild(fetchButton);

    if (job.running) {
        const cancelButton = document.createElement('button');
        cancelButton.type = 'button';
        cancelButton.className = 'admin-btn admin-btn-danger';
        cancelButton.textContent = 'Cancel';
        cancelButton.onclick = cancelMediaInfoFetch;
        actions.appendChild(cancelButton);
    }
    body.appendChild(actions);

    if (!mediaInfoData.enabled) {
        const off = document.createElement('p');
        off.className = 'admin-section-blurb mediainfo-error';
        off.textContent =
            'Turn on "Enable online lookups" above and save before fetching.';
        body.appendChild(off);
    }

    if (job.total > 0 || job.running) {
        body.appendChild(renderMediaInfoProgress(job));
    }

    const flagged = mediaInfoData.flagged || [];
    if (flagged.length > 0) {
        body.appendChild(renderMediaInfoFlagged(flagged));
    }

    // Survives a re-render: switching away from the tab and back while a run is
    // going should pick the polling back up.
    if (job.running && !mediaInfoPollTimer) startMediaInfoPolling();
}

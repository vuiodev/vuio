// Admin tab. Every control is built from the schema GET /api/admin/config returns,
// so a setting added on the server side appears here without touching this file.

let adminData = null;
// Dotted key -> value, or null meaning "remove the key from the file".
let adminEdits = {};
// Null until the libraries list is edited; then it replaces the whole array.
let adminDirectories = null;
let adminSection = 'server';

function adminReadOnly() {
    return !adminData || !adminData.runtime.writable;
}

function adminSectionById(id) {
    return adminData.sections.find(section => section.id === id) || adminData.sections[0];
}

/** Effective value for a key: the pending edit if there is one, else what the server sent. */
function adminValue(key) {
    if (Object.prototype.hasOwnProperty.call(adminEdits, key)) return adminEdits[key];
    return adminData.values[key];
}

/** Whether the key would be written to config.toml once the pending edits are saved. */
function adminIsSet(key) {
    if (Object.prototype.hasOwnProperty.call(adminEdits, key)) return adminEdits[key] !== null;
    return !!adminData.present[key];
}

function adminDirectoryList() {
    return adminDirectories || adminData.directories;
}

function adminDirty() {
    return Object.keys(adminEdits).length > 0 || adminDirectories !== null;
}

async function loadAdminConfig() {
    // Re-entering the tab should show the server's state, not a stale draft.
    if (adminDirty()) {
        renderAdmin();
        return;
    }
    try {
        const response = await fetch('/api/admin/config');
        if (!response.ok) throw new Error('Request failed: ' + response.status);
        adminData = await response.json();
        adminEdits = {};
        adminDirectories = null;
        renderAdmin();
        // Providers and job progress come from a second endpoint, and a failure
        // there must not take the settings screen down with it — the panel renders
        // its own loading state until this lands.
        loadMediaInfo()
            .then(renderAdmin)
            .catch(() => {});
    } catch (error) {
        const body = document.getElementById('admin-pane-body');
        body.replaceChildren();
        const message = document.createElement('div');
        message.className = 'admin-loading';
        message.textContent = 'Could not load settings: ' + error.message;
        body.appendChild(message);
    }
}

function renderAdmin() {
    if (!adminData) return;
    renderAdminBanner();
    renderAdminNav();
    renderAdminPane();
    renderAdminFooter();
}

function renderAdminBanner() {
    const banner = document.getElementById('admin-banner');
    banner.replaceChildren();
    // A bind the server could not take outlives the save that caused it, so it is a
    // standing banner driven by the runtime state rather than a toast.
    const runtime = adminData.runtime;
    if (runtime.bind_error && runtime.desired_addr) {
        banner.className = 'admin-banner admin-banner-warn';
        banner.style.display = 'flex';
        const text = document.createElement('div');
        text.className = 'admin-banner-text';
        text.textContent =
            'Configured for ' + runtime.desired_addr + ' but still serving on ' +
            (runtime.bound_addr || 'the previous address') + ': ' + runtime.bind_error;
        banner.appendChild(text);
        return;
    }

    const reason = runtime.read_only_reason;
    if (!reason) {
        banner.style.display = 'none';
        return;
    }
    banner.className = 'admin-banner admin-banner-warn';
    banner.style.display = 'flex';
    const text = document.createElement('div');
    text.className = 'admin-banner-text';
    text.textContent = reason;
    banner.appendChild(text);
}

/** Shown after a save that only takes hold on the next start. */
function showAdminRestartBanner() {
    const banner = document.getElementById('admin-banner');
    banner.replaceChildren();
    banner.className = 'admin-banner admin-banner-info';
    banner.style.display = 'flex';

    const text = document.createElement('div');
    text.className = 'admin-banner-text';
    text.textContent = 'Saved. Some of these settings only take effect after a restart.';
    banner.appendChild(text);

    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'admin-btn admin-btn-danger';
    button.textContent = 'Restart server';
    button.onclick = restartServer;
    banner.appendChild(button);
}


/** The server moved and this page is talking to an address that is about to close. */
function showServerMovedModal(moved) {
    // Built from this browser's own hostname, not from what the server thinks its
    // address is: the operator may have reached the dashboard over localhost, a LAN
    // address or a proxy, and only they know which one works.
    const target = window.location.protocol + '//' + window.location.hostname + ':' + moved.port;

    const overlay = document.createElement('div');
    overlay.className = 'admin-modal-overlay';
    const dialog = document.createElement('div');
    dialog.className = 'admin-modal';

    const heading = document.createElement('h2');
    heading.textContent = 'This server moved';
    const body = document.createElement('p');
    body.textContent =
        'It is now at ' + target + '. This page is still connected to the old address ' +
        'and will stop responding shortly; anything streaming from it will be interrupted.';
    const note = document.createElement('p');
    note.className = 'admin-modal-note';
    note.textContent = 'Taking you there in a moment…';

    const actions = document.createElement('div');
    actions.className = 'admin-footer-actions';
    const go = document.createElement('button');
    go.type = 'button';
    go.className = 'admin-btn admin-btn-primary';
    go.textContent = 'Go to the new address';
    go.onclick = () => window.location.replace(target + window.location.pathname);
    actions.appendChild(go);

    dialog.append(heading, body, note, actions);
    overlay.appendChild(dialog);
    document.body.appendChild(overlay);
    setTimeout(() => window.location.replace(target + window.location.pathname), 4000);
}

function renderAdminNav() {
    const nav = document.getElementById('admin-nav');
    nav.replaceChildren();
    const entries = adminData.sections.map(section => [section.id, section.title]);
    entries.push(['runtime', 'This server']);
    for (const [id, title] of entries) {
        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'admin-nav-item' + (id === adminSection ? ' active' : '');
        button.textContent = title;
        button.onclick = () => {
            adminSection = id;
            renderAdmin();
        };
        nav.appendChild(button);
    }
}

function renderAdminPane() {
    const body = document.getElementById('admin-pane-body');
    body.replaceChildren();

    if (adminSection === 'runtime') {
        body.appendChild(adminHeading('This server', 'Where this configuration came from.'));
        body.appendChild(renderAdminRuntime());
        return;
    }

    const section = adminSectionById(adminSection);
    body.appendChild(adminHeading(section.title, section.blurb));

    if (section.directories) {
        renderAdminLibraries(body);
        return;
    }
    for (const spec of section.fields) {
        body.appendChild(renderAdminRow(spec));
    }
    // Sections may carry actions that are not settings — see mediainfo.js.
    if (section.panel) {
        renderMediaInfoPanel(body);
    }
}

function adminHeading(title, blurb) {
    const wrapper = document.createElement('div');
    const heading = document.createElement('div');
    heading.className = 'admin-section-title';
    heading.textContent = title;
    const description = document.createElement('p');
    description.className = 'admin-section-blurb';
    description.textContent = blurb;
    wrapper.append(heading, description);
    return wrapper;
}

function renderAdminRuntime() {
    const runtime = adminData.runtime;
    const list = document.createElement('dl');
    list.className = 'admin-runtime';

    const rows = [
        ['Config file', runtime.config_path, true],
        ['Serving on', runtime.bound_addr || 'not bound', true],
        ['Editable here', runtime.writable ? 'Yes' : 'No', false],
        ['Authentication', runtime.auth_enabled ? 'Required' : 'Not required', false],
        ['Container', runtime.is_docker ? 'Yes' : 'No', false],
        ['Version', runtime.version, true],
    ];
    for (const [term, value, mono] of rows) {
        const dt = document.createElement('dt');
        dt.textContent = term;
        const dd = document.createElement('dd');
        dd.textContent = value;
        if (mono) dd.className = 'admin-mono';
        list.append(dt, dd);
    }

    const actions = document.createElement('div');
    actions.className = 'admin-footer-actions';
    actions.style.marginTop = '1.25rem';

    const restart = document.createElement('button');
    restart.type = 'button';
    restart.className = 'admin-btn admin-btn-danger';
    restart.textContent = 'Restart server';
    restart.onclick = restartServer;
    actions.appendChild(restart);

    if (runtime.auth_enabled) {
        const signOut = document.createElement('button');
        signOut.type = 'button';
        signOut.className = 'admin-btn';
        signOut.textContent = 'Sign out';
        signOut.onclick = signOutOfDashboard;
        actions.appendChild(signOut);
    }

    const wrapper = document.createElement('div');
    wrapper.append(list, actions);
    return wrapper;
}

function renderAdminRow(spec) {
    const row = document.createElement('div');
    row.className = 'admin-row';

    const isSet = adminIsSet(spec.key);
    const disabled = adminReadOnly() || !isSet;

    const label = document.createElement('div');
    label.className = 'admin-label';
    const name = document.createElement('span');
    name.textContent = spec.label;
    label.appendChild(name);
    // "restart" means the running server is still using the old value; "next start"
    // means the setting only describes what happens at startup and there is nothing
    // to apply now. Collapsing the two is what made the old labels untrustworthy.
    const IMPACT_PILLS = {
        restart: ['admin-pill-restart', 'restart', 'The running server keeps the old value until it is restarted.'],
        next_start: ['admin-pill-next', 'next start', 'Describes what happens at startup, so there is nothing to apply now.'],
    };
    const pillSpec = IMPACT_PILLS[spec.impact];
    if (pillSpec) {
        const [className, text, title] = pillSpec;
        const pill = document.createElement('span');
        pill.className = 'admin-pill ' + className;
        pill.textContent = text;
        pill.title = title;
        label.appendChild(pill);
    }
    if (!isSet) {
        const pill = document.createElement('span');
        pill.className = 'admin-pill admin-pill-unset';
        pill.textContent = 'not set';
        label.appendChild(pill);
    }
    row.appendChild(label);

    const help = document.createElement('div');
    help.className = 'admin-help';
    help.textContent = spec.help;
    row.appendChild(help);

    if (spec.note) {
        const note = document.createElement('div');
        note.className = 'admin-note';
        note.textContent = spec.note;
        row.appendChild(note);
    }

    // A command-line override wins for the rest of this run, so a saved value here is
    // correct but not yet in effect. Saying so beats letting it look like it failed.
    const forced = (adminData.overrides || {})[spec.key];
    if (forced !== undefined) {
        const note = document.createElement('div');
        note.className = 'admin-note';
        note.textContent =
            'Running with ' + forced + ', set on the command line. What you save here ' +
            'goes into the file and takes effect the next time the server starts ' +
            'without that option.';
        row.appendChild(note);
    }

    const control = document.createElement('div');
    control.className = 'admin-control';
    control.appendChild(adminControlFor(spec, disabled));

    // Only keys AppConfig has a default for can be removed; the rest must always
    // carry a value, so offering to unset them would produce a config that will
    // not load.
    if (spec.removable) {
        const toggle = document.createElement('label');
        toggle.className = 'admin-toggle';
        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.checked = isSet;
        checkbox.disabled = adminReadOnly();
        checkbox.onchange = () => {
            // Turning it on starts from whatever default is currently in force, so
            // the value the operator was already looking at is what gets written.
            adminEdits[spec.key] = checkbox.checked ? adminData.values[spec.key] : null;
            renderAdmin();
        };
        const caption = document.createElement('span');
        caption.textContent = 'set';
        toggle.append(checkbox, caption);
        control.appendChild(toggle);
    }
    row.appendChild(control);

    return row;
}

function adminControlFor(spec, disabled) {
    const value = adminValue(spec.key);
    const commit = next => {
        adminEdits[spec.key] = next;
        renderAdminFooter();
    };

    if (spec.type === 'bool') {
        const select = document.createElement('select');
        select.className = 'admin-select';
        select.disabled = disabled;
        for (const [text, boolean] of [['On', true], ['Off', false]]) {
            const option = document.createElement('option');
            option.value = String(boolean);
            option.textContent = text;
            option.selected = !!value === boolean;
            select.appendChild(option);
        }
        select.onchange = () => commit(select.value === 'true');
        return select;
    }

    if (spec.type === 'int') {
        const input = document.createElement('input');
        input.className = 'admin-input';
        input.type = 'number';
        input.min = spec.min;
        input.max = spec.max;
        input.disabled = disabled;
        input.value = value === null || value === undefined ? '' : value;
        input.oninput = () => {
            const parsed = parseInt(input.value, 10);
            commit(Number.isNaN(parsed) ? null : parsed);
        };
        return input;
    }

    if (spec.type === 'enum') {
        // Free-form enums (an interface name) need a text box as well as the list.
        if (spec.free_form) {
            const input = document.createElement('input');
            input.className = 'admin-input';
            input.type = 'text';
            input.disabled = disabled;
            input.value = value === null || value === undefined ? '' : value;
            input.setAttribute('list', 'admin-options-' + spec.key.replace(/\./g, '-'));
            input.oninput = () => commit(input.value);

            const list = document.createElement('datalist');
            list.id = input.getAttribute('list');
            for (const option of spec.options) {
                const item = document.createElement('option');
                item.value = option;
                list.appendChild(item);
            }
            const wrapper = document.createElement('div');
            wrapper.style.flex = '1';
            wrapper.style.minWidth = '0';
            wrapper.append(input, list);
            return wrapper;
        }
        const select = document.createElement('select');
        select.className = 'admin-select';
        select.disabled = disabled;
        for (const option of spec.options) {
            const item = document.createElement('option');
            item.value = option;
            item.textContent = option;
            item.selected = option === value;
            select.appendChild(item);
        }
        select.onchange = () => commit(select.value);
        return select;
    }

    if (spec.type === 'string_list') {
        const area = document.createElement('textarea');
        area.className = 'admin-input admin-mono';
        area.disabled = disabled;
        area.spellcheck = false;
        area.placeholder = 'One entry per line';
        area.value = Array.isArray(value) ? value.join('\n') : '';
        area.oninput = () => {
            commit(
                area.value
                    .split('\n')
                    .map(entry => entry.trim())
                    .filter(entry => entry.length > 0)
            );
        };
        return area;
    }

    const input = document.createElement('input');
    input.className = 'admin-input' + (spec.type === 'path' ? ' admin-mono' : '');
    input.type = 'text';
    input.disabled = disabled;
    input.value = value === null || value === undefined ? '' : value;
    input.oninput = () => commit(input.value);
    return input;
}

function renderAdminLibraries(body) {
    const directories = adminDirectoryList();

    const forced = (adminData.overrides || {})['media.directories'];
    if (forced !== undefined) {
        const note = document.createElement('div');
        note.className = 'admin-note';
        note.style.marginBottom = '0.75rem';
        note.textContent =
            'Scanning ' + forced + ' for this run, set on the command line. The folders ' +
            'below are what the file says and take effect the next time the server ' +
            'starts without that option.';
        body.appendChild(note);
    }

    directories.forEach((directory, index) => {
        const card = document.createElement('div');
        card.className = 'admin-library';

        const head = document.createElement('div');
        head.className = 'admin-library-head';
        const path = document.createElement('input');
        path.className = 'admin-input admin-mono';
        path.type = 'text';
        path.value = directory.path || '';
        path.placeholder = '/path/to/media';
        path.disabled = adminReadOnly();
        path.oninput = () => editLibrary(index, 'path', path.value);
        head.appendChild(path);

        const remove = document.createElement('button');
        remove.type = 'button';
        remove.className = 'admin-btn admin-btn-danger';
        remove.textContent = 'Remove';
        remove.disabled = adminReadOnly() || directories.length <= 1;
        remove.title =
            directories.length <= 1 ? 'At least one library folder is required' : '';
        remove.onclick = () => {
            const next = adminDirectoryList().slice();
            next.splice(index, 1);
            adminDirectories = next;
            renderAdmin();
        };
        head.appendChild(remove);
        card.appendChild(head);

        const grid = document.createElement('div');
        grid.className = 'admin-library-grid';
        grid.appendChild(
            libraryField('Recurse into subfolders', libraryBool(index, 'recursive', directory.recursive !== false))
        );
        grid.appendChild(
            libraryField(
                'Missing folder handling',
                libraryEnum(index, 'validation_mode', directory.validation_mode || 'Warn', [
                    'Warn',
                    'Strict',
                    'Skip',
                ])
            )
        );
        // A key the file omits shows the value actually in force as a placeholder, and
        // stays out of the file unless it is edited — so saving a library does not
        // freeze this version's platform defaults into the config.
        // A library that is not saved yet has no effective entry, so fall back to
        // what the server says it would apply — otherwise a new folder claims it
        // excludes nothing while the running config excludes the usual noise.
        const effective = (adminData.effective_directories || [])[index]
            || adminData.library_defaults
            || {};
        const defaultExcludes = (effective.exclude_patterns && effective.exclude_patterns.length > 0)
            ? effective.exclude_patterns
            : ((adminData.library_defaults && adminData.library_defaults.exclude_patterns)
                ? adminData.library_defaults.exclude_patterns
                : ['.*', '.DS_Store', '.AppleDouble', '.Trashes', '*.tmp', '.fseventsd']);
        const excludeVal = (directory.exclude_patterns !== undefined && directory.exclude_patterns !== null)
            ? directory.exclude_patterns
            : defaultExcludes;

        grid.appendChild(
            libraryField(
                'Extensions',
                libraryList(index, 'extensions', directory.extensions, 'Using the global list')
            )
        );
        grid.appendChild(
            libraryField(
                'Exclude patterns',
                libraryList(
                    index,
                    'exclude_patterns',
                    excludeVal,
                    'One entry per line'
                )
            )
        );
        card.appendChild(grid);
        body.appendChild(card);
    });

    const add = document.createElement('button');
    add.type = 'button';
    add.className = 'admin-btn';
    add.textContent = 'Add a library folder';
    add.disabled = adminReadOnly();
    add.onclick = () => {
        const defaultExcludes = (adminData.library_defaults && adminData.library_defaults.exclude_patterns)
            ? adminData.library_defaults.exclude_patterns.slice()
            : ['.*', '.DS_Store', '.AppleDouble', '.Trashes', '*.tmp', '.fseventsd'];
        adminDirectories = adminDirectoryList().concat([{
            path: '',
            recursive: true,
            exclude_patterns: defaultExcludes
        }]);
        renderAdmin();
    };
    body.appendChild(add);
}

function libraryField(labelText, control) {
    const wrapper = document.createElement('div');
    const label = document.createElement('label');
    label.className = 'admin-field-label';
    label.textContent = labelText;
    wrapper.append(label, control);
    return wrapper;
}

function editLibrary(index, key, value) {
    const next = adminDirectoryList().map(entry => Object.assign({}, entry));
    if (value === null) {
        delete next[index][key];
    } else {
        next[index][key] = value;
    }
    adminDirectories = next;
    renderAdminFooter();
}

function libraryBool(index, key, checked) {
    const select = document.createElement('select');
    select.className = 'admin-select';
    select.disabled = adminReadOnly();
    for (const [text, boolean] of [['Yes', true], ['No', false]]) {
        const option = document.createElement('option');
        option.value = String(boolean);
        option.textContent = text;
        option.selected = checked === boolean;
        select.appendChild(option);
    }
    select.onchange = () => editLibrary(index, key, select.value === 'true');
    return select;
}

function libraryEnum(index, key, value, options) {
    const select = document.createElement('select');
    select.className = 'admin-select';
    select.disabled = adminReadOnly();
    for (const option of options) {
        const item = document.createElement('option');
        item.value = option;
        item.textContent = option;
        item.selected = option === value;
        select.appendChild(item);
    }
    select.onchange = () => editLibrary(index, key, select.value);
    return select;
}

function libraryList(index, key, value, placeholder) {
    const area = document.createElement('textarea');
    area.className = 'admin-input admin-mono';
    area.disabled = adminReadOnly();
    area.spellcheck = false;
    area.placeholder = placeholder || 'One entry per line';
    area.value = Array.isArray(value) ? value.join('\n') : '';
    area.oninput = () => {
        const entries = area.value
            .split('\n')
            .map(entry => entry.trim())
            .filter(entry => entry.length > 0);
        // An empty box means "unset", which is not the same as an empty list.
        editLibrary(index, key, entries.length > 0 ? entries : null);
    };
    return area;
}

function renderAdminFooter() {
    const footer = document.getElementById('admin-footer');
    if (adminReadOnly()) {
        footer.style.display = 'none';
        return;
    }
    footer.style.display = 'flex';
    document.getElementById('admin-config-path').textContent = adminData.runtime.config_path;

    const dirty = adminDirty();
    const count = Object.keys(adminEdits).length + (adminDirectories === null ? 0 : 1);
    document.getElementById('admin-dirty').textContent = dirty
        ? count + (count === 1 ? ' unsaved change' : ' unsaved changes')
        : '';
    document.getElementById('admin-save').disabled = !dirty;
}

function revertAdminChanges() {
    adminEdits = {};
    adminDirectories = null;
    loadAdminConfig();
}

async function saveAdminConfig() {
    const button = document.getElementById('admin-save');
    button.disabled = true;
    const payload = { values: adminEdits };
    if (adminDirectories !== null) payload.directories = adminDirectories;

    try {
        const response = await fetch('/api/admin/config', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload),
        });
        const result = await response.json().catch(() => ({}));
        if (!response.ok || result.error) {
            throw new Error(result.error || 'Could not save settings');
        }

        adminEdits = {};
        adminDirectories = null;
        // Re-read rather than assume: the reload may have normalised something, and
        // presence has changed for every key just set or unset.
        await loadAdminConfig();

        // A move disconnects this page, so it gets a modal rather than a toast: a toast
        // on a page that is about to stop responding is the worst possible affordance.
        if (result.moved && result.moved.state === 'moved') {
            showServerMovedModal(result.moved);
            return;
        }
        if (result.moved && result.moved.state === 'failed') {
            showToast(
                'Saved, but the server could not move to ' + result.moved.desired + ': ' +
                result.moved.error + '. Still serving on ' + result.moved.serving + '.',
                'error'
            );
            return;
        }
        if (result.moved && result.moved.state === 'pending') {
            showToast('Saved. The server is still moving to the new address.', 'info');
            return;
        }
        if (result.impact === 'restart_required') {
            showAdminRestartBanner();
            showToast('Settings saved. A restart is needed for some of them.', 'info');
        } else if (result.impact === 'next_start') {
            showToast('Settings saved. They apply the next time the server starts.', 'info');
        } else if (result.impact === 'no_change') {
            showToast('No changes to save.', 'info');
        } else {
            showToast('Settings saved and applied.', 'success');
        }
    } catch (error) {
        showToast(error.message, 'error');
        renderAdminFooter();
    }
}

async function restartServer() {
    const supervised = adminData && adminData.runtime.is_docker;
    const warning = supervised
        ? 'Restart the server now? Streaming and playback will drop for a few seconds.'
        : 'Stop and restart the server now? It will only come back if it runs under ' +
          'Docker, systemd or launchd — otherwise you will need to start it yourself.';
    if (!window.confirm(warning)) return;

    try {
        const response = await fetch('/api/admin/restart', { method: 'POST' });
        const result = await response.json().catch(() => ({}));
        if (!response.ok || result.error) {
            throw new Error(result.error || 'Could not restart the server');
        }
        showToast('Restarting. Waiting for the server to come back…', 'info');
        waitForServerToReturn();
    } catch (error) {
        showToast(error.message, 'error');
    }
}

function waitForServerToReturn() {
    let attempts = 0;
    const poll = setInterval(async () => {
        attempts += 1;
        try {
            const response = await fetch('/healthz', { cache: 'no-store' });
            if (response.ok) {
                clearInterval(poll);
                window.location.reload();
                return;
            }
        } catch (error) {
            // Expected while the listener is down.
        }
        if (attempts > 30) {
            clearInterval(poll);
            showToast('The server has not come back. Start it again manually.', 'error');
        }
    }, 1000);
}

async function signOutOfDashboard() {
    try {
        await fetch('/logout', { method: 'POST' });
    } catch (error) {
        // Signing out locally still makes sense if the request failed.
    }
    window.location.href = '/login';
}

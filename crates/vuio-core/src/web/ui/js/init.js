async function initializeDashboard() {
    // Loads server information and the media library, then leaves the browse
    // view watching for changes — the dashboard opens on it.
    await syncLibrary(true);
    startLibraryWatch(false);
}

// Load runtime data through JSON APIs; no untrusted values are embedded
// in the HTML document.
initializeDashboard();

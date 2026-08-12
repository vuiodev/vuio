let activeNav = 'browse';
let metricsTimer = null;

// Each view names how it lays out and what it needs doing on the way in and out.
// The switcher used to hardcode two ids and an if/else, which a third tab broke.
const NAV_VIEWS = {
    // Browsing re-checks the library on the way in and keeps watching while it
    // is visible, so content added from the Admin view appears without a reload.
    browse: {
        display: 'block',
        enter: () => startLibraryWatch(),
        leave: () => stopLibraryWatch(),
    },
    stats: {
        display: 'flex',
        enter: () => {
            updateMetrics();
            metricsTimer = setInterval(updateMetrics, 5000);
        },
        leave: () => {
            if (metricsTimer) {
                clearInterval(metricsTimer);
                metricsTimer = null;
            }
        },
    },
    admin: { display: 'flex', enter: () => loadAdminConfig() },
};

function switchNav(nav) {
    if (!NAV_VIEWS[nav]) return;

    const previous = NAV_VIEWS[activeNav];
    if (previous && previous.leave) previous.leave();
    activeNav = nav;

    document.querySelectorAll('.nav-tab').forEach(btn => {
        btn.classList.toggle('active', btn.id === 'nav-' + nav);
    });
    for (const [name, view] of Object.entries(NAV_VIEWS)) {
        const element = document.getElementById('view-' + name);
        if (element) element.style.display = name === nav ? view.display : 'none';
    }

    const view = NAV_VIEWS[nav];
    if (view.enter) view.enter();
}

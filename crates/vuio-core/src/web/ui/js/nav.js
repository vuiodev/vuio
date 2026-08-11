let activeNav = 'browse';
let metricsTimer = null;

function switchNav(nav) {
    activeNav = nav;
    document.querySelectorAll('.nav-tab').forEach(btn => {
        if (btn.id === 'nav-' + nav) {
            btn.classList.add('active');
        } else {
            btn.classList.remove('active');
        }
    });

    if (nav === 'browse') {
        document.getElementById('view-browse').style.display = 'block';
        document.getElementById('view-stats').style.display = 'none';
        if (metricsTimer) {
            clearInterval(metricsTimer);
            metricsTimer = null;
        }
    } else {
        document.getElementById('view-browse').style.display = 'none';
        document.getElementById('view-stats').style.display = 'flex';
        updateMetrics();
        metricsTimer = setInterval(updateMetrics, 5000);
    }
}

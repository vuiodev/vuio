//! Keeps the listener and the discovery advertisement matching the configuration
//! while the server runs.
//!
//! Everything here exists because the alternative is a restart. The pieces that answer
//! the network — the TCP listener, the SSDP service, the mDNS registration — are built
//! from configuration once and then hold resources, so a config change used to reach
//! them only by way of the process exiting.
//!
//! The shape is one task per resource that never returns, owning a generation of that
//! resource keyed by a `CancellationToken` child. `runner`'s supervisor treats any
//! service completion as fatal, including `Ok(())`, so an in-place restart cannot work
//! by letting a service return; it has to happen inside a task that stays running.

use super::*;
use std::{net::SocketAddr, time::Duration};

/// How long a replaced listener may keep serving requests that were already in flight.
///
/// `axum`'s graceful shutdown waits indefinitely, and a single whole-file stream can
/// hold a connection for hours — long enough to make a same-port interface change
/// impossible, because the old listener never releases the address. Short requests and
/// HLS segment fetches finish well inside this; a movie is severed, which moving the
/// port does anyway.
const DRAIN_DEADLINE: Duration = Duration::from_secs(30);

/// How long a secondary listener waits before trying an address it could not
/// take.
///
/// Only the web UI's listener retries: the main one is bound before the
/// supervisor starts, so a port already in use is a startup failure the
/// operator sees immediately. The web UI must not have that power — a clash on
/// its port would otherwise stop the media server — so it reports the failure
/// and keeps trying, and comes up on its own when whatever held the port lets
/// go.
#[cfg(feature = "web-ui")]
const REBIND_RETRY_INTERVAL: Duration = Duration::from_secs(30);

/// The address the configuration is asking a listener to answer on.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DesiredBind {
    interface: String,
    port: u16,
}

impl DesiredBind {
    fn of(config: &AppConfig) -> Self {
        Self {
            interface: config.server.interface.clone(),
            port: config.server.port,
        }
    }

    /// What the configuration asks of the web UI's listener, or `None` when it
    /// asks for nothing. It follows the main listener's interface: the two
    /// surfaces are the same server, and an operator who has bound the media
    /// server to one interface did not mean to expose its administration on
    /// every other one.
    #[cfg(feature = "web-ui")]
    fn of_web_ui(config: &AppConfig) -> Option<Self> {
        config.web_ui.enabled.then(|| Self {
            interface: config.server.interface.clone(),
            port: config.web_ui.port,
        })
    }

    fn resolve(&self) -> anyhow::Result<SocketAddr> {
        let address = if self.interface == "0.0.0.0" || self.interface.is_empty() {
            std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)
        } else {
            self.interface
                .parse()
                .with_context(|| format!("Invalid server interface address: {}", self.interface))?
        };
        Ok(SocketAddr::new(address, self.port))
    }
}

impl std::fmt::Display for DesiredBind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}:{}", self.interface, self.port)
    }
}

/// The settings the discovery advertisement is built from. A change to any of these
/// means the advertisement is describing a server that no longer exists.
#[derive(Clone, Debug, PartialEq)]
struct AdvertisedIdentity {
    name: String,
    uuid: String,
    ip: Option<String>,
    interface_selection: crate::config::NetworkInterfaceConfig,
    mdns_enabled: bool,
    announce_interval_seconds: u64,
    multicast_ttl: u8,
    port: u16,
}

impl AdvertisedIdentity {
    fn of(config: &AppConfig, port: u16) -> Self {
        Self {
            name: config.server.name.clone(),
            uuid: config.server.uuid.clone(),
            ip: config.server.ip.clone(),
            interface_selection: config.network.interface_selection.clone(),
            mdns_enabled: config.network.mdns_enabled,
            announce_interval_seconds: config.network.announce_interval_seconds,
            multicast_ttl: config.network.multicast_ttl,
            port,
        }
    }
}

/// One running listener.
struct HttpGeneration {
    addr: SocketAddr,
    shutdown: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}

fn spawn_http<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    listener: tokio::net::TcpListener,
    global: &CancellationToken,
    surface: web::Surface,
) -> anyhow::Result<HttpGeneration> {
    let addr = listener.local_addr()?;
    // A child token: cancelling this generation must not cancel the world, while a
    // global shutdown still cascades into it.
    let shutdown = global.child_token();
    let app = web::create_router(state.clone(), surface);
    let token = shutdown.clone();
    let task = tokio::spawn(async move {
        if let Err(error) = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(token.cancelled_owned())
        .await
        {
            error!(%addr, %error, "HTTP listener stopped with an error");
        }
    });
    Ok(HttpGeneration {
        addr,
        shutdown,
        task,
    })
}

/// Say so, loudly, when the MCP endpoint is reachable off this machine with no
/// credential.
///
/// The tools are not all read-only: an assistant reaching this endpoint can
/// delete playlists and start playback on any device it finds on the network.
/// That is fine on loopback and fine behind a token; it is worth a warning when
/// it is neither, because nothing else in the startup output would say so.
#[cfg_attr(not(feature = "mcp"), allow(unused_variables))]
fn warn_if_mcp_is_open<D: DatabaseManager + 'static>(state: &AppState<D>, addr: SocketAddr) {
    #[cfg(feature = "mcp")]
    {
        let config = state.current_config();
        if !config.mcp.enabled || addr.ip().is_loopback() {
            return;
        }
        if state.auth.enabled() || config.mcp.require_auth {
            return;
        }
        warn!(
            "The MCP endpoint at http://{addr}/mcp accepts requests from anywhere on this \
             network with no credential. It can delete playlists and cast to your devices. \
             Set [mcp].require_auth = true (or [management].enabled = true) to require a token, \
             [mcp].read_only = true to offer only browsing, or [mcp].enabled = false to switch \
             it off."
        );
    }
}

/// Whether two addresses can be bound at the same time.
///
/// They cannot when they share a port and either is the wildcard, which is exactly the
/// interface-only change: `0.0.0.0:8080` and `192.168.1.5:8080` conflict on most
/// platforms, and tokio does not set SO_REUSEADDR on Windows. Forcing an overlap with
/// SO_REUSEPORT would be worse — two generations would load-balance connections on one
/// address while one of them is about to be torn down.
fn overlaps(a: SocketAddr, b: SocketAddr) -> bool {
    a.port() == b.port() && (a.ip() == b.ip() || a.ip().is_unspecified() || b.ip().is_unspecified())
}

/// What a rebind attempt ended up doing.
enum Rebound {
    /// Serving on the new address.
    Moved(HttpGeneration),
    /// The new address could not be taken and the old generation is untouched.
    Refused(anyhow::Error),
    /// The old listener had to stop accepting before the new address could be tried,
    /// the attempt failed, and this generation is the old address bound again.
    RolledBack(HttpGeneration, anyhow::Error),
    /// As above, but rebinding the old address failed too. Nothing is accepting.
    Lost(anyhow::Error),
}

async fn rebind<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    global: &CancellationToken,
    current: &HttpGeneration,
    target: SocketAddr,
    surface: web::Surface,
) -> Rebound {
    // Disjoint addresses: take the new one first, so there is no instant at which the
    // server is not accepting, and a failure costs nothing.
    if !overlaps(target, current.addr) {
        return match tokio::net::TcpListener::bind(target).await {
            Ok(listener) => match spawn_http(state, listener, global, surface) {
                Ok(generation) => Rebound::Moved(generation),
                Err(error) => Rebound::Refused(error),
            },
            Err(error) => Rebound::Refused(
                anyhow::Error::from(error).context(format!("Failed to bind to {target}")),
            ),
        };
    }

    // Overlapping: the addresses cannot coexist, so the old listener has to stop
    // accepting first. In-flight requests keep running — axum drops the listener at the
    // signal and only then waits on connections — but nothing new is accepted until one
    // of the binds below succeeds.
    debug!(%target, current = %current.addr, "Addresses overlap; stopping the old listener first");
    current.shutdown.cancel();

    let mut backoff = Duration::from_millis(25);
    let mut last_error = None;
    for _ in 0..6 {
        tokio::time::sleep(backoff).await;
        match tokio::net::TcpListener::bind(target).await {
            Ok(listener) => match spawn_http(state, listener, global, surface) {
                Ok(generation) => return Rebound::Moved(generation),
                Err(error) => last_error = Some(error),
            },
            Err(error) => last_error = Some(anyhow::Error::from(error)),
        }
        backoff *= 2;
    }
    let failure = last_error
        .unwrap_or_else(|| anyhow::anyhow!("could not bind {target}"))
        .context(format!("Failed to bind to {target}"));

    // Without this the server is simply offline: the old generation already stopped
    // accepting on its way to an address it could not take.
    match tokio::net::TcpListener::bind(current.addr).await {
        Ok(listener) => match spawn_http(state, listener, global, surface) {
            Ok(generation) => Rebound::RolledBack(generation, failure),
            Err(error) => Rebound::Lost(error),
        },
        Err(error) => Rebound::Lost(
            anyhow::Error::from(error)
                .context(format!("Failed to restore the listener on {}", current.addr)),
        ),
    }
}

/// Let a replaced listener finish what it was already doing, then drop it.
fn drain<D: DatabaseManager + 'static>(state: &AppState<D>, mut old: HttpGeneration) {
    let addr = old.addr;
    state.background_tasks.spawn(async move {
        old.shutdown.cancel();
        let started = std::time::Instant::now();
        if tokio::time::timeout(DRAIN_DEADLINE, &mut old.task)
            .await
            .is_err()
        {
            warn!(
                %addr,
                "Old listener still had open connections after {DRAIN_DEADLINE:?}; dropping them"
            );
            old.task.abort();
        } else {
            debug!(%addr, elapsed = ?started.elapsed(), "Old listener drained");
        }
    });
}

/// A bound but not yet serving listener.
///
/// Binding happens before the supervisor is spawned so that a port already in use is
/// still a clear startup failure, reported by the caller, rather than a service that
/// dies moments after everything else has started.
pub(super) struct PendingListener {
    listener: tokio::net::TcpListener,
}

pub(super) async fn bind_first_listener<D: DatabaseManager + 'static>(
    state: &AppState<D>,
) -> anyhow::Result<PendingListener> {
    let config = state.current_config();
    let target = DesiredBind::of(&config).resolve()?;
    info!("Server UUID: {}", config.server.uuid);
    info!("Server name: {}", config.server.name);
    info!("Listening on http://{}", target);
    let listener = tokio::net::TcpListener::bind(target)
        .await
        .with_context(|| format!("Failed to bind to address: {target}"))?;
    // Published before anything reads it, so discovery announces the real port even
    // when the OS picked it.
    state.http_binding.publish_serving(listener.local_addr()?);
    Ok(PendingListener { listener })
}

/// Owns the listener across configuration changes.
///
/// Never returns while the server is healthy: `runner` treats a service that finishes
/// as fatal, so returning between generations would end the process.
pub(super) async fn run_http_supervisor<D: DatabaseManager + 'static>(
    state: AppState<D>,
    global: CancellationToken,
    started: PendingListener,
) -> anyhow::Result<()> {
    let mut changes = state.live_config.subscribe();
    let mut requested = DesiredBind::of(&state.current_config());
    let mut current = spawn_http(&state, started.listener, &global, web::Surface::Primary)?;
    state.http_binding.publish_serving(current.addr);
    info!("HTTP server started successfully");
    warn_if_mcp_is_open(&state, current.addr);

    loop {
        tokio::select! {
            _ = global.cancelled() => {
                current.shutdown.cancel();
                let _ = tokio::time::timeout(DRAIN_DEADLINE, &mut current.task).await;
                return Ok(());
            }
            // The live generation ending on its own is the failure the outer supervisor
            // exists for. Drained generations are owned elsewhere and never land here.
            joined = &mut current.task => {
                joined.context("HTTP listener task panicked")?;
                anyhow::bail!("HTTP listener on {} stopped unexpectedly", current.addr);
            }
            changed = changes.changed() => {
                if changed.is_err() {
                    continue;
                }
                let want = DesiredBind::of(&changes.borrow_and_update().clone());
                // Compared request to request, never request to bound: those differ
                // whenever the OS picks the port, and comparing against the bound
                // address would rebind on every unrelated config change.
                if want == requested {
                    continue;
                }
                requested = want.clone();

                let target = match want.resolve() {
                    Ok(target) => target,
                    Err(error) => {
                        error!(desired = %want, serving = %current.addr, "{error:#}");
                        state.http_binding.publish_failure(want.to_string(), format!("{error:#}"));
                        continue;
                    }
                };
                if target == current.addr {
                    continue;
                }

                match rebind(&state, &global, &current, target, web::Surface::Primary).await {
                    Rebound::Moved(next) => {
                        let previous = std::mem::replace(&mut current, next);
                        state.http_binding.publish_serving(current.addr);
                        info!(from = %previous.addr, to = %current.addr, "HTTP listener moved");
                        drain(&state, previous);
                    }
                    Rebound::Refused(error) => {
                        error!(
                            desired = %target, serving = %current.addr,
                            "Could not move the HTTP listener, still serving on the old address: {error:#}"
                        );
                        state.http_binding.publish_failure(want.to_string(), format!("{error:#}"));
                    }
                    Rebound::RolledBack(restored, error) => {
                        // The old generation stopped accepting on the way here, so the
                        // replacement is the old address bound again rather than the
                        // original listener.
                        let previous = std::mem::replace(&mut current, restored);
                        state.http_binding.publish_serving(current.addr);
                        error!(
                            desired = %target, serving = %current.addr,
                            "Could not move the HTTP listener; restored the old address: {error:#}"
                        );
                        state.http_binding.publish_failure(want.to_string(), format!("{error:#}"));
                        drain(&state, previous);
                    }
                    Rebound::Lost(error) => {
                        anyhow::bail!(
                            "HTTP listener could not be rebound to {target} or restored on {}: {error:#}",
                            current.addr
                        );
                    }
                }
            }
        }
    }
}

/// Owns the web UI's listener across configuration changes.
///
/// Deliberately weaker than [`run_http_supervisor`] in two ways, both because
/// this is a second front end rather than a second server:
///
/// * it never publishes to `state.http_binding`, which is where the media URLs
///   handed to TVs and renderers come from. Those must keep pointing at the
///   main port, which is the one DLNA advertises and the one a renderer on the
///   network can be expected to reach.
/// * a listener it cannot bind is reported and retried rather than fatal. The
///   main port is bound before any of this starts, so a clash there is a
///   startup error the operator sees; a clash on the web UI's port must not be
///   able to take the media server down with it.
///
/// Like every other service here it never returns while the server is healthy:
/// `runner` treats a service that finishes as fatal.
#[cfg(feature = "web-ui")]
pub(super) async fn run_web_ui_supervisor<D: DatabaseManager + 'static>(
    state: AppState<D>,
    global: CancellationToken,
) -> anyhow::Result<()> {
    let mut changes = state.live_config.subscribe();
    let mut requested = DesiredBind::of_web_ui(&state.current_config());
    let mut current: Option<HttpGeneration> = None;
    // Ticks continuously; the arm below does nothing unless something is owed.
    // A timer created on demand would have to be re-armed from three places.
    let mut retry = tokio::time::interval(REBIND_RETRY_INTERVAL);
    retry.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    retry.tick().await;

    reconcile_web_ui(&state, &global, &mut current, requested.as_ref()).await;

    loop {
        // `select!` polls a `None` option's future forever, so an absent
        // generation simply means the join arm never fires.
        let serving = async {
            match current.as_mut() {
                Some(generation) => (&mut generation.task).await,
                None => std::future::pending().await,
            }
        };

        tokio::select! {
            _ = global.cancelled() => {
                if let Some(mut generation) = current.take() {
                    generation.shutdown.cancel();
                    let _ = tokio::time::timeout(DRAIN_DEADLINE, &mut generation.task).await;
                }
                return Ok(());
            }
            joined = serving => {
                // The listener ended on its own. Fatal for the main server;
                // here it costs the web UI until the next retry.
                let addr = current.take().map(|generation| generation.addr);
                match joined {
                    Ok(()) => error!(?addr, "Web UI listener stopped unexpectedly; will retry"),
                    Err(error) => error!(?addr, %error, "Web UI listener panicked; will retry"),
                }
            }
            _ = retry.tick() => {
                // Only does anything when the configuration wants a listener
                // and there is none, which is the state a failed bind leaves.
                if current.is_none() && requested.is_some() {
                    reconcile_web_ui(&state, &global, &mut current, requested.as_ref()).await;
                }
            }
            changed = changes.changed() => {
                if changed.is_err() {
                    continue;
                }
                let want = DesiredBind::of_web_ui(&changes.borrow_and_update().clone());
                // Compared request to request, never request to bound: an
                // unrelated config change must not cycle the listener.
                if want == requested {
                    continue;
                }
                requested = want;
                reconcile_web_ui(&state, &global, &mut current, requested.as_ref()).await;
            }
        }
    }
}

/// Bring the web UI's listener in line with what the configuration asks for.
#[cfg(feature = "web-ui")]
async fn reconcile_web_ui<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    global: &CancellationToken,
    current: &mut Option<HttpGeneration>,
    want: Option<&DesiredBind>,
) {
    let Some(want) = want else {
        if let Some(previous) = current.take() {
            info!(addr = %previous.addr, "Web UI disabled; stopping its listener");
            drain(state, previous);
        }
        return;
    };

    let target = match want.resolve() {
        Ok(target) => target,
        Err(error) => {
            error!(desired = %want, "Web UI address is unusable: {error:#}");
            return;
        }
    };

    if let Some(existing) = current.as_ref() {
        if existing.addr == target {
            return;
        }
        match rebind(state, global, existing, target, web::Surface::WebUi).await {
            Rebound::Moved(next) => {
                let previous = current.replace(next).expect("checked above");
                info!(from = %previous.addr, to = %target, "Web UI listener moved");
                drain(state, previous);
            }
            Rebound::Refused(error) => {
                error!(
                    desired = %target, serving = %existing.addr,
                    "Could not move the web UI listener, still serving on the old address: {error:#}"
                );
            }
            Rebound::RolledBack(restored, error) => {
                let previous = current.replace(restored).expect("checked above");
                error!(
                    desired = %target, serving = %previous.addr,
                    "Could not move the web UI listener; restored the old address: {error:#}"
                );
                drain(state, previous);
            }
            Rebound::Lost(error) => {
                // Not fatal, unlike the main listener: the retry timer picks
                // this up, and the media server is still serving throughout.
                *current = None;
                error!(desired = %target, "Web UI listener could not be rebound or restored: {error:#}");
            }
        }
        return;
    }

    match tokio::net::TcpListener::bind(target).await {
        Ok(listener) => match spawn_http(state, listener, global, web::Surface::WebUi) {
            Ok(generation) => {
                info!("Web interface listening on http://{}", generation.addr);
                *current = Some(generation);
            }
            Err(error) => error!(desired = %target, "Could not start the web UI listener: {error:#}"),
        },
        Err(error) => error!(
            desired = %target,
            "Could not bind the web UI listener; retrying in {}s: {error}",
            REBIND_RETRY_INTERVAL.as_secs()
        ),
    }
}

/// One running advertisement: an SSDP service and, optionally, an mDNS registration.
///
/// Both are keyed by one token. Withdrawing them is what makes a restart correct —
/// SSDP's teardown sends `ssdp:byebye` and dropping the mDNS advertiser sends its
/// goodbye — so a generation swap tells clients the old identity is gone before
/// announcing the new one.
struct AdvertisementGeneration {
    identity: AdvertisedIdentity,
    shutdown: CancellationToken,
    ssdp: tokio::task::JoinHandle<anyhow::Result<()>>,
}

impl AdvertisementGeneration {
    /// Stop announcing, waiting for the goodbyes to go out.
    ///
    /// Bounded by SSDP's own 2-second teardown budget. Waiting matters for more than
    /// politeness: the SSDP socket is on the fixed port 1900 with SO_REUSEADDR, so two
    /// generations would coexist happily and answer the same M-SEARCH with conflicting
    /// LOCATIONs for one USN.
    async fn stop(mut self) {
        self.shutdown.cancel();
        if tokio::time::timeout(Duration::from_secs(5), &mut self.ssdp)
            .await
            .is_err()
        {
            warn!("SSDP did not stop within its teardown budget; aborting it");
            self.ssdp.abort();
        }
    }
}

fn start_advertisement<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    global: &CancellationToken,
) -> AdvertisementGeneration {
    let identity = AdvertisedIdentity::of(&state.current_config(), state.http_binding.port());
    let shutdown = global.child_token();

    // `UnifiedSsdpService::new` re-reads the configuration and the bound address every
    // time it is constructed, so a fresh generation picks up the change with no
    // config-threading of its own.
    let ssdp_state = state.clone();
    let ssdp_token = shutdown.clone();
    let ssdp = tokio::spawn(async move {
        ssdp::run_ssdp_service_until_cancelled(ssdp_state, ssdp_token)
            .await
            .context("SSDP service failed")
    });

    super::network::start_mdns_advertisement(state, state.http_binding.port(), shutdown.clone());

    AdvertisementGeneration {
        identity,
        shutdown,
        ssdp,
    }
}

/// Owns the SSDP and mDNS advertisement across configuration changes.
///
/// Split from the listener supervisor because the two want opposite orderings. The
/// listener takes the new address before releasing the old one, since two listeners on
/// different ports are harmless and a bind failure must not take the server offline.
/// The advertisement must do the reverse: withdraw, then announce.
pub(super) async fn run_advertisement_supervisor<D: DatabaseManager + 'static>(
    state: AppState<D>,
    global: CancellationToken,
) -> anyhow::Result<()> {
    let mut changes = state.live_config.subscribe();
    // Also woken by the listener moving, so the re-announce happens after the new
    // address is real rather than racing the rebind for the same config change.
    let mut moved = state.http_binding.subscribe();
    let mut current = start_advertisement(&state, &global);

    loop {
        let restart_after_failure = tokio::time::sleep(Duration::from_secs(5));
        tokio::select! {
            _ = global.cancelled() => {
                current.stop().await;
                return Ok(());
            }
            bumped = moved.changed() => {
                if bumped.is_err() {
                    continue;
                }
                moved.borrow_and_update();
                info!("Listener moved; re-announcing discovery");
                current.stop().await;
                current = start_advertisement(&state, &global);
            }
            // Generation 0 failing is a startup failure and stays fatal, matching the
            // behaviour before discovery could be restarted. Later generations must not
            // be: an operator changing a setting should never exit the server, and the
            // HTTP side is unaffected, so the server keeps serving while discovery retries.
            joined = &mut current.ssdp => {
                let outcome = joined.context("SSDP task panicked")?;
                if let Err(error) = outcome {
                    error!("SSDP stopped: {error:#}; retrying");
                } else {
                    warn!("SSDP stopped unexpectedly; restarting");
                }
                restart_after_failure.await;
                current = start_advertisement(&state, &global);
            }
            changed = changes.changed() => {
                if changed.is_err() {
                    continue;
                }
                let config = changes.borrow_and_update().clone();
                let wanted = AdvertisedIdentity::of(&config, state.http_binding.port());
                if wanted == current.identity {
                    continue;
                }
                info!("Discovery settings changed; re-announcing");
                current.stop().await;
                current = start_advertisement(&state, &global);
            }
        }
    }
}

/// Starts and stops file-system monitoring as `media.watch_for_changes` changes.
///
/// The flag was read once, immediately before the monitoring task was spawned, and
/// never again — so toggling it did nothing in either direction until a restart, while
/// the settings screen claimed it applied live.
///
/// What toggles is the OS watch registration, not the task. The event channel and its
/// sole receiver are created once for the life of the `CrossPlatformWatcher` and cannot
/// be taken twice, so a supervisor that stopped and restarted the consuming task could
/// never start it again. Keeping the consumer and swapping the registration underneath
/// it is the pattern the watcher is already built for.
pub(super) async fn run_monitoring_supervisor<D: DatabaseManager + 'static>(
    watcher: Arc<CrossPlatformWatcher>,
    state: AppState<D>,
    global: CancellationToken,
) -> anyhow::Result<()> {
    let mut changes = state.live_config.subscribe();
    // Started unconditionally: it registers the watches and takes the receiver, and if
    // monitoring is off the registrations are released immediately below.
    let consumer = match start_file_monitoring(watcher.clone(), state.clone(), global.clone()).await
    {
        Ok(Some(handle)) => Some(handle),
        Ok(None) => None,
        Err(error) => {
            warn!("Could not start file monitoring: {error:#}");
            warn!("Continuing without real-time file monitoring");
            None
        }
    };
    let mut watching = consumer.is_some();

    async fn apply<D: DatabaseManager + 'static>(
        watcher: &Arc<CrossPlatformWatcher>,
        state: &AppState<D>,
        watching: &mut bool,
        wanted: bool,
    ) {
        if wanted == *watching {
            return;
        }
        *watching = wanted;
        if wanted {
            let directories = state
                .media_directories
                .read()
                .await
                .iter()
                .map(|root| PathBuf::from(&root.path))
                .filter(|path| path.is_dir())
                .collect::<Vec<_>>();
            match watcher.start_watching(&directories).await {
                Ok(()) => info!("File monitoring enabled by configuration"),
                Err(error) => warn!("Could not resume file monitoring: {error:#}"),
            }
        } else {
            match watcher.stop_watching().await {
                Ok(()) => info!("File monitoring disabled by configuration"),
                Err(error) => warn!("Could not stop file monitoring: {error:#}"),
            }
        }
    }

    apply(
        &watcher,
        &state,
        &mut watching,
        state.current_config().media.watch_for_changes,
    )
    .await;

    loop {
        tokio::select! {
            _ = global.cancelled() => {
                if let Some(consumer) = consumer {
                    let _ = tokio::time::timeout(Duration::from_secs(5), consumer).await;
                }
                return Ok(());
            }
            changed = changes.changed() => {
                if changed.is_err() {
                    continue;
                }
                let wanted = changes.borrow_and_update().media.watch_for_changes;
                apply(&watcher, &state, &mut watching, wanted).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(text: &str) -> SocketAddr {
        text.parse().expect("address")
    }

    /// The overlap rule decides which of the two rebind strategies is safe, and getting
    /// it wrong either way is a real failure: treating an overlap as disjoint means
    /// binding an address the old listener still holds, and treating a disjoint pair as
    /// overlapping gives up the bind-first guarantee for no reason.
    #[test]
    fn overlap_is_about_sharing_a_port_with_a_wildcard() {
        // A port change is always disjoint, whatever the interfaces.
        assert!(!overlaps(addr("0.0.0.0:8080"), addr("0.0.0.0:9090")));
        assert!(!overlaps(addr("127.0.0.1:8080"), addr("0.0.0.0:9090")));

        // Same port, and one side accepts on every interface: they conflict.
        assert!(overlaps(addr("0.0.0.0:8080"), addr("192.168.1.5:8080")));
        assert!(overlaps(addr("192.168.1.5:8080"), addr("0.0.0.0:8080")));
        assert!(overlaps(addr("0.0.0.0:8080"), addr("0.0.0.0:8080")));

        // Same port on two specific, different addresses coexists fine.
        assert!(!overlaps(addr("127.0.0.1:8080"), addr("192.168.1.5:8080")));
    }

    /// Which address the configuration is asking for, including the two spellings of
    /// "every interface" that the config file allows.
    #[test]
    fn a_desired_bind_resolves_the_way_the_config_file_reads() {
        let wildcard = DesiredBind {
            interface: "0.0.0.0".to_string(),
            port: 8080,
        };
        assert_eq!(wildcard.resolve().unwrap(), addr("0.0.0.0:8080"));

        // An empty interface has always meant the wildcard too.
        let empty = DesiredBind {
            interface: String::new(),
            port: 8080,
        };
        assert_eq!(empty.resolve().unwrap(), addr("0.0.0.0:8080"));

        let specific = DesiredBind {
            interface: "127.0.0.1".to_string(),
            port: 8080,
        };
        assert_eq!(specific.resolve().unwrap(), addr("127.0.0.1:8080"));

        // Caught before anything is torn down, rather than mid-rebind.
        let nonsense = DesiredBind {
            interface: "not-an-address".to_string(),
            port: 8080,
        };
        let error = nonsense.resolve().unwrap_err();
        assert!(error.to_string().contains("Invalid server interface address"));
    }

    /// The advertisement is rebuilt when what it says about the server changes, and not
    /// otherwise — an unrelated edit must not cost every client a byebye/alive cycle.
    #[test]
    fn the_advertised_identity_ignores_unrelated_settings() {
        let base = AppConfig::default_for_platform();
        let identity = AdvertisedIdentity::of(&base, 8080);

        let mut unrelated = base.clone();
        unrelated.media.autoplay_enabled = !unrelated.media.autoplay_enabled;
        unrelated.database.vacuum_on_startup = !unrelated.database.vacuum_on_startup;
        assert_eq!(AdvertisedIdentity::of(&unrelated, 8080), identity);

        let mut renamed = base.clone();
        renamed.server.name = "Something Else".to_string();
        assert_ne!(AdvertisedIdentity::of(&renamed, 8080), identity);

        // And the port it advertises is the bound one, not a config field.
        assert_ne!(AdvertisedIdentity::of(&base, 9090), identity);
    }
}

use super::*;

const TV_DISCOVERY_INTERVAL: tokio::time::Duration = tokio::time::Duration::from_secs(30);

/// Keep one shared DLNA, Google Cast, and AirPlay renderer snapshot fresh.
///
/// Independent of the listener: it used to be started inside the HTTP setup, which
/// meant a rebind would needlessly cycle renderer discovery too.
pub(super) fn start_tv_discovery<D: DatabaseManager + 'static>(
    app_state: AppState<D>,
    cancellation: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(TV_DISCOVERY_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = cancellation.cancelled() => break,
                _ = interval.tick() => {
                    #[cfg(feature = "casting")]
                    if let Err(error) = app_state.discovered_tvs.refresh().await {
                        tracing::warn!(%error, "Background multi-protocol renderer discovery failed");
                    }
                }
            }
        }
        #[cfg(not(feature = "casting"))]
        let _ = &app_state;
    })
}

/// Announce this server over mDNS alongside SSDP.
///
/// The advertisement is held by a background task rather than returned,
/// because withdrawing it is what matters at shutdown: the task drops the
/// advertiser when cancelled, which sends the goodbye packets. Without that a
/// stopped server lingers in client caches for the record's full TTL.
///
/// `port` is the port actually bound, not the configured one, so a server
/// started on port 0 still advertises where it can be reached.
pub(super) fn start_mdns_advertisement<D: DatabaseManager + 'static>(
    app_state: &AppState<D>,
    port: u16,
    cancellation: CancellationToken,
) {
    let config = app_state.current_config();
    if !config.network.mdns_enabled {
        debug!("mDNS advertisement disabled by configuration");
        return;
    }

    // The same address SSDP advertises: announcing a different one over mDNS
    // would send clients somewhere the DLNA path does not point.
    let advertised = app_state.get_server_ip();
    let Ok(ip) = advertised.parse::<std::net::IpAddr>() else {
        warn!(address = %advertised, "Skipping mDNS advertisement: not a usable address");
        return;
    };

    let server = crate::mdns::ServerAdvertisement {
        uuid: config.server.uuid.clone(),
        name: config.server.name.clone(),
        ip,
        port,
        requires_auth: app_state.auth.enabled(),
    };

    app_state.background_tasks.spawn(async move {
        let advertiser = match crate::mdns::MdnsAdvertiser::start(&server) {
            Ok(advertiser) => advertiser,
            Err(error) => {
                // A server that cannot advertise still serves; SSDP is
                // unaffected and the address still works if typed in.
                warn!(%error, "Could not advertise over mDNS");
                return;
            }
        };
        cancellation.cancelled().await;
        drop(advertiser);
    });
}


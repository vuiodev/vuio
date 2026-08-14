//! Live radio: stations VuIO broadcasts, and the ones its neighbours do.
//!
//! A station is a folder selection turned into a continuous stream. The server
//! plays it — reads the files, keeps the clock, advances the queue — so a
//! listener who connects joins whatever is playing at that moment, part-way
//! through a track, exactly as a radio works. Nothing about it depends on a
//! browser being open, and a station that was live when the process stopped is
//! live again when it starts.
//!
//! - [`frames`] cuts source files into the self-describing frames a stream is
//!   made of, and decides what can be broadcast at all.
//! - [`engine`] is the playout task: one per station, pacing frames out in real
//!   time to everyone listening.
//! - [`peers`] finds other VuIO servers on the network and asks them what they
//!   are broadcasting.
//!
//! The HTTP surface is in [`crate::web::radio`].

pub mod engine;
pub mod frames;
pub mod peers;

use crate::database::{DatabaseManager, RadioStation};
use anyhow::{Context, Result};
use engine::{Station, StationSnapshot};
use peers::{PeerCache, PeerServer};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// Every station that is on the air right now.
///
/// The database holds what stations *are*; this holds the ones that are
/// running. The two are kept in step by `enabled`: a station is started when
/// that flag is set and stopped when it is cleared, which is what makes a
/// restart resume exactly the stations an operator left broadcasting.
#[derive(Default)]
pub struct RadioManager {
    live: RwLock<HashMap<i64, Arc<Station>>>,
    peers: Mutex<PeerCache>,
}

impl RadioManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Put a station on the air, replacing any earlier run of it.
    ///
    /// Fails if the station's folders hold nothing that can be broadcast, which
    /// is the one start-time problem an operator can actually fix.
    pub async fn start<D: DatabaseManager + 'static>(
        &self,
        state: &crate::state::AppState<D>,
        station: &RadioStation,
    ) -> Result<Arc<Station>> {
        self.stop(station.id).await;
        let running = Station::start(state.clone(), station)
            .await
            .with_context(|| format!("starting the station '{}'", station.name))?;
        self.live.write().await.insert(station.id, running.clone());
        Ok(running)
    }

    /// Take a station off the air. Quiet if it was not on it.
    pub async fn stop(&self, id: i64) {
        let station = self.live.write().await.remove(&id);
        if let Some(station) = station {
            station.shutdown().await;
        }
    }

    /// Move a station on to its next track.
    pub async fn skip(&self, id: i64) -> bool {
        match self.live.read().await.get(&id) {
            Some(station) => {
                station.skip();
                true
            }
            None => false,
        }
    }

    pub async fn get(&self, id: i64) -> Option<Arc<Station>> {
        self.live.read().await.get(&id).cloned()
    }

    /// What every live station is doing, newest first by station id.
    pub async fn snapshots(&self) -> Vec<StationSnapshot> {
        let live = self.live.read().await;
        let mut snapshots: Vec<_> = live.values().map(|station| station.snapshot()).collect();
        snapshots.sort_by_key(|snapshot| snapshot.id);
        snapshots
    }

    pub async fn is_live(&self, id: i64) -> bool {
        self.live.read().await.contains_key(&id)
    }

    pub async fn live_count(&self) -> usize {
        self.live.read().await.len()
    }

    /// Other VuIO servers on the network and what they are broadcasting.
    pub async fn peers(&self, own_uuid: &str) -> Vec<PeerServer> {
        peers::cached(&self.peers, own_uuid).await
    }

    /// Start every station the operator left enabled.
    ///
    /// Called once at startup. A station whose files have moved or been deleted
    /// since is logged and left off the air rather than blocking the rest.
    pub async fn restore<D: DatabaseManager + 'static>(state: &crate::state::AppState<D>) {
        let stations = match state.database.list_radio_stations().await {
            Ok(stations) => stations,
            Err(error) => {
                tracing::error!("Could not read the radio stations to restore: {error:#}");
                return;
            }
        };

        for station in stations.iter().filter(|station| station.enabled) {
            match state.radio.start(state, station).await {
                Ok(_) => tracing::info!(
                    station = %station.name,
                    "Resumed radio broadcast that was live before the last shutdown"
                ),
                Err(error) => tracing::warn!(
                    station = %station.name,
                    "Could not resume radio broadcast: {error:#}"
                ),
            }
        }
    }

}

//! UPnP ContentDirectory subscription and change-notification handling.

use crate::{database::DatabaseManager, state::AppState};
use axum::{
    extract::{ConnectInfo, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::IntoResponse,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::Ordering;
use tracing::{info, warn};

const MAX_SUBSCRIPTIONS: usize = 256;
const MAX_SUBSCRIPTIONS_PER_PEER: usize = 16;
const MIN_NOTIFICATION_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

/// Publish one externally visible ContentDirectory mutation.
///
/// The revision, browse-response invalidation, and UPnP notification are kept
/// together so callers cannot update one without the others.
pub async fn publish_content_change<D: DatabaseManager + 'static>(state: &AppState<D>) {
    let old_id = state
        .content_update_id
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let new_id = old_id.wrapping_add(1);
    invalidate_browse_responses(state).await;
    info!(old_id, new_id, "ContentDirectory revision published");

    state.content_change_notify.notify_one();
}

/// One coalescing publisher owns all mutation notifications. Every burst is
/// reduced to its latest revision and still receives a trailing notification.
pub async fn run_content_change_publisher<D: DatabaseManager + 'static>(
    state: AppState<D>,
    cancellation: tokio_util::sync::CancellationToken,
) {
    loop {
        tokio::select! {
            _ = cancellation.cancelled() => break,
            _ = state.content_change_notify.notified() => {}
        }
        loop {
            let quiet = tokio::time::sleep(MIN_NOTIFICATION_INTERVAL);
            tokio::pin!(quiet);
            tokio::select! {
                _ = cancellation.cancelled() => return,
                _ = state.content_change_notify.notified() => continue,
                _ = &mut quiet => break,
            }
        }
        let latest = state.content_update_id.load(Ordering::SeqCst);
        notify_content_change(&state, latest).await;
    }
}

/// Clear cached SOAP browse responses without announcing a content mutation.
pub async fn invalidate_browse_responses<D: DatabaseManager>(state: &AppState<D>) {
    state.browse_cache.lock().await.clear();
}

/// Handle UPnP eventing subscription requests for ContentDirectory service
pub async fn content_directory_subscribe<D: DatabaseManager>(
    State(state): State<AppState<D>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    method: Method,
) -> impl IntoResponse {
    let timeout_seconds = headers
        .get("TIMEOUT")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Second-"))
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1800)
        .min(1800);
    let timeout_header = format!("Second-{timeout_seconds}");

    if method.as_str() == "UNSUBSCRIBE" {
        let Some(sid) = headers.get("SID").and_then(|value| value.to_str().ok()) else {
            return StatusCode::PRECONDITION_FAILED.into_response();
        };
        state.upnp_subscriptions.lock().await.remove(sid);
        return (StatusCode::OK, [(header::CONTENT_LENGTH, "0")], "").into_response();
    }

    if method.as_str() != "SUBSCRIBE" && method != Method::GET {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }

    if let Some(sid) = headers.get("SID").and_then(|value| value.to_str().ok()) {
        let mut subscriptions = state.upnp_subscriptions.lock().await;
        let Some(subscription) = subscriptions.get_mut(sid) else {
            return StatusCode::PRECONDITION_FAILED.into_response();
        };
        if normalize_ip(subscription.peer) != normalize_ip(peer.ip()) {
            return StatusCode::PRECONDITION_FAILED.into_response();
        }
        subscription.expires_at =
            std::time::Instant::now() + std::time::Duration::from_secs(timeout_seconds);
        return (
            StatusCode::OK,
            [
                (header::HeaderName::from_static("sid"), sid),
                (
                    header::HeaderName::from_static("timeout"),
                    timeout_header.as_str(),
                ),
                (header::CONTENT_LENGTH, "0"),
            ],
            "",
        )
            .into_response();
    }

    let Some(raw_callback) = headers
        .get("CALLBACK")
        .and_then(|value| value.to_str().ok())
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(callback_url) = validate_upnp_callback(
        raw_callback,
        peer.ip(),
        &state
            .current_config()
            .network
            .upnp_callback_allowed_networks,
    ) else {
        warn!("Rejected unsafe UPnP callback URL: {}", raw_callback);
        return StatusCode::BAD_REQUEST.into_response();
    };
    let sid = format!("uuid:{}", uuid::Uuid::new_v4());
    let mut subscriptions = state.upnp_subscriptions.lock().await;
    let now = std::time::Instant::now();
    subscriptions.retain(|_, subscription| subscription.expires_at > now);
    let peer_ip = normalize_ip(peer.ip());
    let peer_count = subscriptions
        .values()
        .filter(|subscription| normalize_ip(subscription.peer) == peer_ip)
        .count();
    if subscriptions.len() >= MAX_SUBSCRIPTIONS || peer_count >= MAX_SUBSCRIPTIONS_PER_PEER {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    subscriptions.insert(
        sid.clone(),
        crate::state::UpnpSubscription {
            callback_url: callback_url.clone(),
            peer: peer_ip,
            generation: uuid::Uuid::new_v4(),
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(timeout_seconds),
            next_sequence: 0,
            consecutive_failures: 0,
            last_notification_at: now - MIN_NOTIFICATION_INTERVAL,
        },
    );
    drop(subscriptions);
    state.content_change_notify.notify_one();
    (
        StatusCode::OK,
        [
            (header::HeaderName::from_static("sid"), sid.as_str()),
            (
                header::HeaderName::from_static("timeout"),
                timeout_header.as_str(),
            ),
            (header::CONTENT_LENGTH, "0"),
        ],
        "",
    )
        .into_response()
}

fn normalize_ip(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V6(ip) => ip
            .to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(ip)),
        address => address,
    }
}

fn is_forbidden_callback_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip == Ipv4Addr::BROADCAST
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_unicast_link_local()
        }
    }
}

fn validate_upnp_callback(
    raw: &str,
    peer_ip: IpAddr,
    allowed_networks: &[String],
) -> Option<String> {
    let candidate = raw
        .split('>')
        .next()
        .unwrap_or(raw)
        .trim()
        .trim_start_matches('<');
    let url = reqwest::Url::parse(candidate).ok()?;
    if url.scheme() != "http" {
        return None;
    }
    if !url.username().is_empty() || url.password().is_some() || url.port() == Some(0) {
        return None;
    }

    let address = normalize_ip(url.host_str()?.parse::<IpAddr>().ok()?);
    if is_forbidden_callback_address(address) {
        return None;
    }

    let peer_ip = normalize_ip(peer_ip);
    let explicitly_allowed = allowed_networks
        .iter()
        .filter_map(|network| network.parse::<ipnet::IpNet>().ok())
        .any(|network| network.contains(&address));

    (address == peer_ip || explicitly_allowed).then(|| url.to_string())
}

async fn send_event_notification(
    callback_url: &str,
    sid: &str,
    sequence: u32,
    update_id: u32,
) -> bool {
    let event_body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<e:propertyset xmlns:e="urn:schemas-upnp-org:event-1-0">
    <e:property>
        <SystemUpdateID>{}</SystemUpdateID>
    </e:property>
    <e:property>
        <ContainerUpdateIDs>video,{0},audio,{0},image,{0}</ContainerUpdateIDs>
    </e:property>
</e:propertyset>"#,
        update_id
    );

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(client) => client,
        Err(_) => return false,
    };
    match client
        .request(
            reqwest::Method::from_bytes(b"NOTIFY")
                .expect("NOTIFY is a valid constant HTTP extension method"),
            callback_url,
        )
        .header("CONTENT-TYPE", "text/xml; charset=\"utf-8\"")
        .header("NT", "upnp:event")
        .header("NTS", "upnp:propchange")
        .header("SID", sid)
        .header("SEQ", sequence.to_string())
        .body(event_body)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => true,
        Ok(response) => {
            warn!(
                "UPnP event callback {} returned {}",
                callback_url,
                response.status()
            );
            false
        }
        Err(e) => {
            warn!(
                "Failed to send event notification to {}: {}",
                callback_url, e
            );
            false
        }
    }
}

pub async fn notify_content_change<D: DatabaseManager>(
    state: &AppState<D>,
    _published_update_id: u32,
) {
    use futures_util::{stream, StreamExt};

    let now = std::time::Instant::now();
    // Keep notification batches serialized so subscribers observe monotonically
    // increasing SEQ values even when content changes are published concurrently.
    let update_id = state.content_update_id.load(Ordering::SeqCst);
    let notifications = {
        let mut subscriptions = state.upnp_subscriptions.lock().await;
        subscriptions.retain(|_, subscription| subscription.expires_at > now);
        subscriptions
            .iter_mut()
            .filter_map(|(sid, subscription)| {
                if now.duration_since(subscription.last_notification_at) < MIN_NOTIFICATION_INTERVAL
                {
                    return None;
                }
                let sequence = subscription.next_sequence;
                subscription.next_sequence = subscription.next_sequence.wrapping_add(1);
                subscription.last_notification_at = now;
                Some((
                    sid.clone(),
                    subscription.callback_url.clone(),
                    sequence,
                    subscription.generation,
                ))
            })
            .collect::<Vec<_>>()
    };

    let results = stream::iter(notifications.into_iter().map(
        |(sid, url, sequence, generation)| async move {
            let success = send_event_notification(&url, &sid, sequence, update_id).await;
            (sid, generation, success)
        },
    ))
    .buffer_unordered(8)
    .collect::<Vec<_>>()
    .await;

    let mut subscriptions = state.upnp_subscriptions.lock().await;
    for (sid, generation, success) in results {
        if let Some(subscription) = subscriptions
            .get_mut(&sid)
            .filter(|subscription| subscription.generation == generation)
        {
            if success {
                subscription.consecutive_failures = 0;
            } else {
                subscription.consecutive_failures =
                    subscription.consecutive_failures.saturating_add(1);
            }
        }
    }
    subscriptions.retain(|_, subscription| subscription.consecutive_failures < 3);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upnp_callback_policy_accepts_peer_and_explicit_networks() {
        let peer = "192.168.1.25".parse().unwrap();
        assert!(validate_upnp_callback("<http://192.168.1.25:1234/events>", peer, &[]).is_some());
        assert!(validate_upnp_callback("http://192.168.1.26/events", peer, &[]).is_none());
        assert!(validate_upnp_callback(
            "http://192.168.1.26/events",
            peer,
            &["192.168.1.0/24".to_string()]
        )
        .is_some());
    }

    #[test]
    fn upnp_callback_policy_rejects_ssrf_targets_and_hostnames() {
        let peer = "169.254.169.254".parse().unwrap();
        assert!(validate_upnp_callback("http://169.254.169.254/events", peer, &[]).is_none());
        assert!(validate_upnp_callback(
            "http://127.0.0.1/events",
            "127.0.0.1".parse().unwrap(),
            &[]
        )
        .is_none());
        assert!(validate_upnp_callback(
            "https://192.168.1.25/events",
            "192.168.1.25".parse().unwrap(),
            &[]
        )
        .is_none());
        assert!(validate_upnp_callback("http://example.com/events", peer, &[]).is_none());
        assert!(validate_upnp_callback(
            "http://224.0.0.1/events",
            "224.0.0.1".parse().unwrap(),
            &[]
        )
        .is_none());
        assert!(
            validate_upnp_callback("http://[fe80::1]/events", "fe80::1".parse().unwrap(), &[])
                .is_none()
        );
        assert!(validate_upnp_callback(
            "http://user:secret@192.168.1.25/events",
            "192.168.1.25".parse().unwrap(),
            &[]
        )
        .is_none());
    }
}

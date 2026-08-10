use crate::http_client::{HttpClient, HttpResponse};
use anyhow::{Context, Result};
use http::Uri;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::net::UdpSocket;
use tracing::{debug, warn};

const MAX_RENDERER_ERROR_BYTES: usize = 64 * 1024;
const MAX_RENDERER_SOAP_BYTES: usize = 256 * 1024;

/// POST a SOAP envelope to a renderer's AVTransport control endpoint.
///
/// Every AVTransport call VuIO makes has this shape, so the body cap, headers
/// and `SOAPAction` quoting live in one place rather than in five copies.
async fn soap_request(
    client: &HttpClient,
    control_url: &Uri,
    soap_action: &str,
    body: String,
) -> Result<HttpResponse> {
    let request = http::Request::post(control_url.clone())
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .header(
            "SOAPAction",
            format!("\"urn:schemas-upnp-org:service:AVTransport:1#{soap_action}\""),
        )
        .body(crate::http_client::body(body))
        .context("could not build the SOAP request")?;
    client.send(request, MAX_RENDERER_SOAP_BYTES).await
}

fn renderer_error(operation: &str, response: &HttpResponse) -> anyhow::Error {
    // The body was already read under the SOAP cap; an error message quotes
    // only the smaller error cap of it.
    let shown = MAX_RENDERER_ERROR_BYTES.min(response.body.len());
    let truncated = response.truncated || shown < response.body.len();
    let suffix = if truncated { " [truncated]" } else { "" };
    anyhow::anyhow!(
        "{} failed (HTTP {}): {}{}",
        operation,
        response.status,
        String::from_utf8_lossy(&response.body[..shown]),
        suffix
    )
}

fn renderer_soap_body(response: &HttpResponse, operation: &str) -> Result<String> {
    anyhow::ensure!(
        !response.truncated,
        "{} response exceeded {} bytes",
        operation,
        MAX_RENDERER_SOAP_BYTES
    );
    Ok(response.text())
}

/// A discovered UPnP MediaRenderer (TV/speaker/player) on the network
#[derive(Clone, Debug, serde::Serialize)]
pub struct DiscoveredTv {
    pub id: String,
    pub friendly_name: String,
    pub control_url: String,
    pub location_url: String,
    pub model_name: String,
}

/// Discover UPnP MediaRenderer devices on the local network via SSDP M-SEARCH
pub async fn discover_tvs() -> Result<Vec<DiscoveredTv>> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_broadcast(true)?;

    let search_request = "M-SEARCH * HTTP/1.1\r\n\
        HOST: 239.255.255.250:1900\r\n\
        MAN: \"ssdp:discover\"\r\n\
        MX: 3\r\n\
        ST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n\
        \r\n";

    let target: SocketAddr = "239.255.255.250:1900".parse()?;
    socket.send_to(search_request.as_bytes(), target).await?;

    let mut locations: Vec<(String, String, IpAddr)> = Vec::new();
    let mut buf = vec![0u8; 2048];

    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);

    loop {
        let recv_future = socket.recv_from(&mut buf);
        match tokio::time::timeout_at(deadline, recv_future).await {
            Ok(Ok((len, addr))) => {
                let response = String::from_utf8_lossy(&buf[..len]);
                let mut location = None;
                let mut usn = None;
                let mut search_target = None;
                for line in response.lines() {
                    if let Some((name, value)) = line.split_once(':') {
                        match name.trim().to_ascii_lowercase().as_str() {
                            "location" => location = Some(value.trim().to_string()),
                            "usn" => usn = Some(value.trim().to_string()),
                            "st" => search_target = Some(value.trim().to_string()),
                            _ => {}
                        }
                    }
                }
                let is_renderer = search_target.as_deref().is_some_and(|target| {
                    target
                        .to_ascii_lowercase()
                        .starts_with("urn:schemas-upnp-org:device:mediarenderer:")
                });
                if is_renderer && response.starts_with("HTTP/1.1 200") {
                    let Some(location) = location else {
                        continue;
                    };
                    if locations.len() < 32
                        && !locations
                            .iter()
                            .any(|(existing, _, _)| existing == &location)
                    {
                        locations.push((location, usn.unwrap_or_default(), addr.ip()));
                    }
                }
            }
            Ok(Err(e)) => {
                warn!("SSDP receive error during TV discovery: {}", e);
                break;
            }
            Err(_) => break, // Timeout reached
        }
    }

    debug!(
        "Discovered {} potential MediaRenderer location(s)",
        locations.len()
    );

    let mut tvs = Vec::new();
    let client = HttpClient::new(Duration::from_secs(5));

    for (location, usn, responder) in locations {
        match fetch_tv_info(&client, &location, &usn, responder).await {
            Ok(Some(tv)) => {
                debug!("Discovered TV: {} at {}", tv.friendly_name, tv.control_url);
                tvs.push(tv);
            }
            Ok(None) => {
                debug!(
                    "Location {} is not a MediaRenderer with AVTransport",
                    location
                );
            }
            Err(e) => {
                warn!("Failed to fetch TV info from {}: {}", location, e);
            }
        }
    }

    Ok(tvs)
}

/// Fetch and parse a UPnP device descriptor XML to extract TV info
async fn fetch_tv_info(
    client: &HttpClient,
    location: &str,
    discovery_usn: &str,
    responder: IpAddr,
) -> Result<Option<DiscoveredTv>> {
    const MAX_DESCRIPTOR_BYTES: usize = 256 * 1024;
    let location_url = validate_renderer_url(location, Some(responder))?;
    let response = client.get(&location_url, MAX_DESCRIPTOR_BYTES).await?;
    anyhow::ensure!(
        response.status.is_success(),
        "renderer descriptor returned {}",
        response.status
    );
    anyhow::ensure!(!response.truncated, "renderer descriptor is too large");
    let body = std::str::from_utf8(&response.body).context("renderer descriptor is not UTF-8")?;

    let mut reader = Reader::from_str(body);

    let mut friendly_name = String::new();
    let mut model_name = String::new();
    let mut control_url = String::new();
    let mut udn = String::new();
    let mut is_media_renderer = false;
    let mut in_av_transport_service = false;
    let mut current_element = String::new();
    let mut current_service_type = String::new();

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                current_element = String::from_utf8_lossy(e.name().as_ref()).to_string();
            }
            Ok(Event::Text(e)) => {
                let decoded = reader.decoder().decode(e.as_ref())?;
                let text = quick_xml::escape::unescape(&decoded)?.into_owned();
                match current_element.as_str() {
                    "friendlyName" if friendly_name.is_empty() => {
                        friendly_name = text;
                    }
                    "modelName" if model_name.is_empty() => {
                        model_name = text;
                    }
                    "UDN" if udn.is_empty() => {
                        udn = text;
                    }
                    "deviceType" if text.contains("MediaRenderer") => {
                        is_media_renderer = true;
                    }
                    "serviceType" => {
                        current_service_type = text.clone();
                        if text.contains("AVTransport") {
                            in_av_transport_service = true;
                        }
                    }
                    "controlURL" if in_av_transport_service && control_url.is_empty() => {
                        control_url = text;
                        in_av_transport_service = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "service" {
                    if !current_service_type.contains("AVTransport") {
                        in_av_transport_service = false;
                    }
                    current_service_type.clear();
                }
                current_element.clear();
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(error.into()),
            _ => {}
        }
        buf.clear();
    }

    if !is_media_renderer || friendly_name.is_empty() || control_url.is_empty() {
        return Ok(None);
    }

    let joined = crate::http_client::join_path(&location_url, &control_url)?;
    let full_control_url = validate_renderer_url(&joined.to_string(), Some(responder))?;

    Ok(Some(DiscoveredTv {
        id: if udn.is_empty() {
            discovery_usn
                .split("::")
                .next()
                .filter(|value| !value.is_empty())
                .unwrap_or(location)
                .to_string()
        } else {
            udn
        },
        friendly_name,
        control_url: full_control_url.to_string(),
        location_url: location_url.to_string(),
        model_name,
    }))
}

fn renderer_address_is_safe(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => {
            !ip.is_loopback()
                && !ip.is_unspecified()
                && !ip.is_link_local()
                && !ip.is_multicast()
                && ip != Ipv4Addr::BROADCAST
        }
        IpAddr::V6(ip) => {
            !ip.is_loopback()
                && !ip.is_unspecified()
                && !ip.is_unicast_link_local()
                && !ip.is_multicast()
        }
    }
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

fn validate_renderer_url(raw: &str, expected_peer: Option<IpAddr>) -> Result<Uri> {
    anyhow::ensure!(
        !crate::http_client::has_fragment(raw),
        "renderer URL fragments are forbidden"
    );
    let url: Uri = raw.parse().context("invalid renderer URL")?;
    anyhow::ensure!(
        url.scheme_str() == Some("http"),
        "renderer URL must use HTTP"
    );
    anyhow::ensure!(
        !crate::http_client::has_credentials(&url),
        "renderer URL credentials are forbidden"
    );
    let address = normalize_ip(
        url.host()
            .context("renderer URL has no host")?
            .parse::<IpAddr>()
            .context("renderer URL host must be a numeric address")?,
    );
    anyhow::ensure!(
        renderer_address_is_safe(address),
        "renderer URL uses an unsafe address"
    );
    if let Some(peer) = expected_peer {
        anyhow::ensure!(
            address == normalize_ip(peer),
            "renderer URL host does not match SSDP responder"
        );
    }
    Ok(url)
}

fn write_text_element(writer: &mut Writer<Vec<u8>>, name: &str, value: &str) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new(name)))?;
    writer.write_event(Event::Text(BytesText::new(value)))?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

fn media_class(mime_type: &str) -> &'static str {
    if mime_type.starts_with("audio/") {
        "object.item.audioItem.musicTrack"
    } else if mime_type.starts_with("video/") {
        "object.item.videoItem"
    } else if mime_type.starts_with("image/") {
        "object.item.imageItem.photo"
    } else {
        "object.item"
    }
}

fn build_transport_uri_soap(
    action: &str,
    media_url: &str,
    title: &str,
    mime_type: &str,
) -> Result<String> {
    build_transport_uri_soap_with_metadata(action, media_url, title, mime_type, true)
}

fn build_transport_uri_soap_with_metadata(
    action: &str,
    media_url: &str,
    title: &str,
    mime_type: &str,
    include_metadata: bool,
) -> Result<String> {
    anyhow::ensure!(
        matches!(action, "SetAVTransportURI" | "SetNextAVTransportURI"),
        "unsupported transport action"
    );
    let media: Uri = media_url.parse().context("invalid media URL")?;
    anyhow::ensure!(
        matches!(media.scheme_str(), Some("http") | Some("https")),
        "media URL must use HTTP(S)"
    );
    let media = media.to_string();
    anyhow::ensure!(
        !mime_type.is_empty()
            && mime_type.is_ascii()
            && !mime_type.bytes().any(|b| b.is_ascii_control()),
        "invalid MIME type"
    );

    let didl = if include_metadata {
        let mut didl = Writer::new(Vec::new());
        let mut root = BytesStart::new("DIDL-Lite");
        root.push_attribute(("xmlns", "urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/"));
        root.push_attribute(("xmlns:dc", "http://purl.org/dc/elements/1.1/"));
        root.push_attribute(("xmlns:upnp", "urn:schemas-upnp-org:metadata-1-0/upnp/"));
        didl.write_event(Event::Start(root))?;
        let mut item = BytesStart::new("item");
        item.push_attribute(("id", "0"));
        item.push_attribute(("parentID", "0"));
        item.push_attribute(("restricted", "1"));
        didl.write_event(Event::Start(item))?;
        write_text_element(&mut didl, "dc:title", title)?;
        write_text_element(&mut didl, "upnp:class", media_class(mime_type))?;
        let protocol_info = format!("http-get:*:{mime_type}:*");
        let mut res = BytesStart::new("res");
        res.push_attribute(("protocolInfo", protocol_info.as_str()));
        didl.write_event(Event::Start(res))?;
        didl.write_event(Event::Text(BytesText::new(media.as_str())))?;
        didl.write_event(Event::End(BytesEnd::new("res")))?;
        didl.write_event(Event::End(BytesEnd::new("item")))?;
        didl.write_event(Event::End(BytesEnd::new("DIDL-Lite")))?;
        String::from_utf8(didl.into_inner())?
    } else {
        String::new()
    };

    let mut soap = Writer::new(Vec::new());
    soap.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;
    let mut envelope = BytesStart::new("s:Envelope");
    envelope.push_attribute(("xmlns:s", "http://schemas.xmlsoap.org/soap/envelope/"));
    envelope.push_attribute((
        "s:encodingStyle",
        "http://schemas.xmlsoap.org/soap/encoding/",
    ));
    soap.write_event(Event::Start(envelope))?;
    soap.write_event(Event::Start(BytesStart::new("s:Body")))?;
    let action_name = format!("u:{action}");
    let mut action_tag = BytesStart::new(action_name.as_str());
    action_tag.push_attribute(("xmlns:u", "urn:schemas-upnp-org:service:AVTransport:1"));
    soap.write_event(Event::Start(action_tag))?;
    write_text_element(&mut soap, "InstanceID", "0")?;
    let (uri_name, metadata_name) = if action == "SetAVTransportURI" {
        ("CurrentURI", "CurrentURIMetaData")
    } else {
        ("NextURI", "NextURIMetaData")
    };
    write_text_element(&mut soap, uri_name, media.as_str())?;
    write_text_element(&mut soap, metadata_name, &didl)?;
    soap.write_event(Event::End(BytesEnd::new(action_name.as_str())))?;
    soap.write_event(Event::End(BytesEnd::new("s:Body")))?;
    soap.write_event(Event::End(BytesEnd::new("s:Envelope")))?;
    Ok(String::from_utf8(soap.into_inner())?)
}

async fn set_transport_uri(
    client: &HttpClient,
    control_url: &Uri,
    media_url: &str,
    title: &str,
    mime_type: &str,
    include_metadata: bool,
) -> Result<()> {
    let body = build_transport_uri_soap_with_metadata(
        "SetAVTransportURI",
        media_url,
        title,
        mime_type,
        include_metadata,
    )?;
    let response = soap_request(client, control_url, "SetAVTransportURI", body).await?;
    if !response.status.is_success() {
        return Err(renderer_error("SetAVTransportURI", &response));
    }
    Ok(())
}

async fn playback_started(control_url: &str, media_url: &str) -> Option<bool> {
    let mut observed = false;
    for _ in 0..5 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let current_uri = get_position_info(control_url).await.ok();
        let state = get_transport_state(control_url).await.ok();
        observed |= current_uri.is_some() || state.is_some();

        let uri_matches = current_uri.as_deref() == Some(media_url);
        let active = state.as_deref().is_some_and(|value| value == "PLAYING");
        if active && (uri_matches || current_uri.is_none()) {
            return Some(true);
        }
        if uri_matches
            && state
                .as_deref()
                .is_some_and(|value| matches!(value, "PAUSED_PLAYBACK" | "RECORDING"))
        {
            return Some(true);
        }
    }
    observed.then_some(false)
}

/// Cast a media file to a TV by sending SOAP SetAVTransportURI + Play
pub async fn cast_media(
    control_url: &str,
    media_url: &str,
    title: &str,
    mime_type: &str,
) -> Result<()> {
    let control_url = validate_renderer_url(control_url, None)?;
    let client = HttpClient::new(Duration::from_secs(10));
    debug!("Casting {} to {}", media_url, control_url);
    set_transport_uri(&client, &control_url, media_url, title, mime_type, true).await?;
    debug!("SetAVTransportURI succeeded on {}", control_url);

    control_playback(&control_url.to_string(), "Play").await?;
    if playback_started(&control_url.to_string(), media_url).await != Some(false) {
        return Ok(());
    }

    // Some renderers accept DIDL metadata but silently refuse to load it.
    // Retry with an empty metadata field, which is valid AVTransport behavior
    // and is required by several older TV implementations.
    debug!(
        "Renderer did not start {}; retrying without DIDL metadata",
        media_url
    );
    let _ = control_playback(&control_url.to_string(), "Stop").await;
    set_transport_uri(&client, &control_url, media_url, title, mime_type, false).await?;
    tokio::time::sleep(Duration::from_millis(150)).await;
    control_playback(&control_url.to_string(), "Play").await?;
    if playback_started(&control_url.to_string(), media_url).await == Some(false) {
        anyhow::bail!(
            "TV accepted the cast commands but did not load the media URL {}",
            media_url
        );
    }
    Ok(())
}

/// Send a playback control command (Play, Pause, Stop) to a TV
pub async fn control_playback(control_url: &str, action: &str) -> Result<()> {
    let control_url = validate_renderer_url(control_url, None)?;
    anyhow::ensure!(
        matches!(action, "Play" | "Pause" | "Stop"),
        "unsupported playback action"
    );
    let client = HttpClient::new(Duration::from_secs(10));

    let speed_element = if action == "Play" {
        "<Speed>1</Speed>"
    } else {
        ""
    };

    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
    s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:{action} xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
            <InstanceID>0</InstanceID>
            {speed_element}
        </u:{action}>
    </s:Body>
</s:Envelope>"#
    );

    let resp = soap_request(&client, &control_url, action, body).await?;

    if !resp.status.is_success() {
        return Err(renderer_error(&format!("{action} command"), &resp));
    }

    debug!("{} command succeeded on {}", action, control_url);
    Ok(())
}

/// Queue the next media file to a TV by sending SOAP SetNextAVTransportURI
pub async fn set_next_media(
    control_url: &str,
    media_url: &str,
    title: &str,
    mime_type: &str,
) -> Result<()> {
    let control_url = validate_renderer_url(control_url, None)?;
    let client = HttpClient::new(Duration::from_secs(10));
    let body = build_transport_uri_soap("SetNextAVTransportURI", media_url, title, mime_type)?;

    let resp = soap_request(&client, &control_url, "SetNextAVTransportURI", body).await?;

    if !resp.status.is_success() {
        return Err(renderer_error("SetNextAVTransportURI", &resp));
    }

    debug!("SetNextAVTransportURI succeeded on {}", control_url);
    Ok(())
}

/// Query what media URI is currently active/playing on the TV
pub async fn get_position_info(control_url: &str) -> Result<String> {
    let control_url = validate_renderer_url(control_url, None)?;
    let client = HttpClient::new(Duration::from_secs(5));

    let body = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
    s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:GetPositionInfo xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
            <InstanceID>0</InstanceID>
        </u:GetPositionInfo>
    </s:Body>
</s:Envelope>"#;

    let resp = soap_request(&client, &control_url, "GetPositionInfo", body.to_owned()).await?;

    if !resp.status.is_success() {
        return Err(renderer_error("GetPositionInfo", &resp));
    }

    let text = renderer_soap_body(&resp, "GetPositionInfo")?;

    // Extract TrackURI
    if let Some(uri_part) = text.split("<TrackURI>").nth(1) {
        if let Some(uri) = uri_part.split("</TrackURI>").next() {
            return Ok(uri.trim().to_string());
        }
    }

    anyhow::bail!("TrackURI not found in SOAP response");
}

/// Query the current transport state (PLAYING, STOPPED, PAUSED_PLAYBACK) of the TV
pub async fn get_transport_state(control_url: &str) -> Result<String> {
    let control_url = validate_renderer_url(control_url, None)?;
    let client = HttpClient::new(Duration::from_secs(5));

    let body = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
    s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:GetTransportInfo xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
            <InstanceID>0</InstanceID>
        </u:GetTransportInfo>
    </s:Body>
</s:Envelope>"#;

    let resp = soap_request(&client, &control_url, "GetTransportInfo", body.to_owned()).await?;

    if !resp.status.is_success() {
        return Err(renderer_error("GetTransportInfo", &resp));
    }

    let text = renderer_soap_body(&resp, "GetTransportInfo")?;
    if let Some(state_part) = text.split("<CurrentTransportState>").nth(1) {
        if let Some(state) = state_part.split("</CurrentTransportState>").next() {
            return Ok(state.trim().to_string());
        }
    }

    Ok("STOPPED".to_string())
}

#[cfg(test)]
mod tests;

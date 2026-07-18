use anyhow::{Context, Result};
use futures_util::StreamExt;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::net::UdpSocket;
use tracing::{debug, warn};

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
                for line in response.lines() {
                    let lower = line.to_lowercase();
                    if lower.starts_with("location:") {
                        location = Some(line[9..].trim().to_string());
                    } else if lower.starts_with("usn:") {
                        usn = Some(line[4..].trim().to_string());
                    }
                }
                if let Some(location) = location {
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
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

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
    client: &reqwest::Client,
    location: &str,
    discovery_usn: &str,
    responder: IpAddr,
) -> Result<Option<DiscoveredTv>> {
    const MAX_DESCRIPTOR_BYTES: usize = 256 * 1024;
    let location_url = validate_renderer_url(location, Some(responder))?;
    let response = client.get(location_url.clone()).send().await?;
    anyhow::ensure!(
        response.status().is_success(),
        "renderer descriptor returned {}",
        response.status()
    );
    if let Some(length) = response.content_length() {
        anyhow::ensure!(
            length <= MAX_DESCRIPTOR_BYTES as u64,
            "renderer descriptor is too large"
        );
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        anyhow::ensure!(
            bytes.len().saturating_add(chunk.len()) <= MAX_DESCRIPTOR_BYTES,
            "renderer descriptor is too large"
        );
        bytes.extend_from_slice(&chunk);
    }
    let body = std::str::from_utf8(&bytes).context("renderer descriptor is not UTF-8")?;

    let mut reader = Reader::from_str(body);

    let mut friendly_name = String::new();
    let mut model_name = String::new();
    let mut control_url = String::new();
    let mut udn = String::new();
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

    if friendly_name.is_empty() || control_url.is_empty() {
        return Ok(None);
    }

    let full_control_url =
        validate_renderer_url(location_url.join(&control_url)?.as_str(), Some(responder))?;

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

fn validate_renderer_url(raw: &str, expected_peer: Option<IpAddr>) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(raw).context("invalid renderer URL")?;
    anyhow::ensure!(url.scheme() == "http", "renderer URL must use HTTP");
    anyhow::ensure!(
        url.username().is_empty() && url.password().is_none(),
        "renderer URL credentials are forbidden"
    );
    anyhow::ensure!(
        url.fragment().is_none(),
        "renderer URL fragments are forbidden"
    );
    let address = normalize_ip(
        url.host_str()
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
    anyhow::ensure!(
        matches!(action, "SetAVTransportURI" | "SetNextAVTransportURI"),
        "unsupported transport action"
    );
    let media = reqwest::Url::parse(media_url).context("invalid media URL")?;
    anyhow::ensure!(
        matches!(media.scheme(), "http" | "https"),
        "media URL must use HTTP(S)"
    );
    anyhow::ensure!(
        !mime_type.is_empty()
            && mime_type.is_ascii()
            && !mime_type.bytes().any(|b| b.is_ascii_control()),
        "invalid MIME type"
    );

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
    let didl = String::from_utf8(didl.into_inner())?;

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

/// Cast a media file to a TV by sending SOAP SetAVTransportURI + Play
pub async fn cast_media(
    control_url: &str,
    media_url: &str,
    title: &str,
    mime_type: &str,
) -> Result<()> {
    let control_url = validate_renderer_url(control_url, None)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let set_uri_body = build_transport_uri_soap("SetAVTransportURI", media_url, title, mime_type)?;

    let resp = client
        .post(control_url.clone())
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .header(
            "SOAPAction",
            "\"urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI\"",
        )
        .body(set_uri_body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("SetAVTransportURI failed (HTTP {}): {}", status, body);
    }

    debug!("SetAVTransportURI succeeded on {}", control_url);

    // Step 2: Play
    control_playback(control_url.as_str(), "Play").await?;

    Ok(())
}

/// Send a playback control command (Play, Pause, Stop) to a TV
pub async fn control_playback(control_url: &str, action: &str) -> Result<()> {
    let control_url = validate_renderer_url(control_url, None)?;
    anyhow::ensure!(
        matches!(action, "Play" | "Pause" | "Stop"),
        "unsupported playback action"
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let soap_action = format!("urn:schemas-upnp-org:service:AVTransport:1#{}", action);

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

    let resp = client
        .post(control_url.clone())
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .header("SOAPAction", format!("\"{}\"", soap_action))
        .body(body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("{} command failed (HTTP {}): {}", action, status, err_body);
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
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let body = build_transport_uri_soap("SetNextAVTransportURI", media_url, title, mime_type)?;

    let resp = client
        .post(control_url.clone())
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .header(
            "SOAPAction",
            "\"urn:schemas-upnp-org:service:AVTransport:1#SetNextAVTransportURI\"",
        )
        .body(body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err_body = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "SetNextAVTransportURI failed (HTTP {}): {}",
            status,
            err_body
        );
    }

    debug!("SetNextAVTransportURI succeeded on {}", control_url);
    Ok(())
}

/// Query what media URI is currently active/playing on the TV
pub async fn get_position_info(control_url: &str) -> Result<String> {
    let control_url = validate_renderer_url(control_url, None)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let body = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
    s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:GetPositionInfo xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
            <InstanceID>0</InstanceID>
        </u:GetPositionInfo>
    </s:Body>
</s:Envelope>"#;

    let resp = client
        .post(control_url)
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .header(
            "SOAPAction",
            "\"urn:schemas-upnp-org:service:AVTransport:1#GetPositionInfo\"",
        )
        .body(body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("GetPositionInfo failed (HTTP {}): {}", status, err_body);
    }

    let text = resp.text().await?;

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
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let body = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
    s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:GetTransportInfo xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
            <InstanceID>0</InstanceID>
        </u:GetTransportInfo>
    </s:Body>
</s:Envelope>"#;

    let resp = client
        .post(control_url)
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .header(
            "SOAPAction",
            "\"urn:schemas-upnp-org:service:AVTransport:1#GetTransportInfo\"",
        )
        .body(body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("GetTransportInfo failed (HTTP {}): {}", status, err_body);
    }

    let text = resp.text().await?;
    if let Some(state_part) = text.split("<CurrentTransportState>").nth(1) {
        if let Some(state) = state_part.split("</CurrentTransportState>").next() {
            return Ok(state.trim().to_string());
        }
    }

    Ok("STOPPED".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_urls_are_peer_bound_and_numeric() {
        let peer: IpAddr = "192.168.1.100".parse().unwrap();
        assert!(validate_renderer_url("http://192.168.1.100:8080/desc.xml", Some(peer)).is_ok());
        assert!(validate_renderer_url("http://192.168.1.101/desc.xml", Some(peer)).is_err());
        assert!(validate_renderer_url("http://localhost/desc.xml", None).is_err());
        assert!(validate_renderer_url("http://169.254.169.254/", None).is_err());
        assert!(validate_renderer_url("https://192.168.1.100/", None).is_err());
    }

    #[test]
    fn transport_xml_escapes_nested_metadata_once() {
        let xml = build_transport_uri_soap(
            "SetAVTransportURI",
            "http://192.168.1.2/a?x=1&y=2",
            "A & <B>",
            "video/mp4",
        )
        .unwrap();
        assert!(xml.contains("A &amp;amp; &amp;lt;B&amp;gt;"));
        assert!(!xml.contains("<dc:title>A &"));
    }
}

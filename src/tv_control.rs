use anyhow::Result;
use quick_xml::Reader;
use quick_xml::events::Event;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tracing::{debug, warn};

/// A discovered UPnP MediaRenderer (TV/speaker/player) on the network
#[derive(Clone, Debug, serde::Serialize)]
pub struct DiscoveredTv {
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

    let mut locations = Vec::new();
    let mut buf = vec![0u8; 2048];

    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);

    loop {
        let recv_future = socket.recv_from(&mut buf);
        match tokio::time::timeout_at(deadline, recv_future).await {
            Ok(Ok((len, _addr))) => {
                let response = String::from_utf8_lossy(&buf[..len]);
                // Extract LOCATION header
                for line in response.lines() {
                    let lower = line.to_lowercase();
                    if lower.starts_with("location:") {
                        let url = line[9..].trim().to_string();
                        if !locations.contains(&url) {
                            locations.push(url);
                        }
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

    debug!("Discovered {} potential MediaRenderer location(s)", locations.len());

    let mut tvs = Vec::new();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    for location in locations {
        match fetch_tv_info(&client, &location).await {
            Ok(Some(tv)) => {
                debug!("Discovered TV: {} at {}", tv.friendly_name, tv.control_url);
                tvs.push(tv);
            }
            Ok(None) => {
                debug!("Location {} is not a MediaRenderer with AVTransport", location);
            }
            Err(e) => {
                warn!("Failed to fetch TV info from {}: {}", location, e);
            }
        }
    }

    Ok(tvs)
}

/// Fetch and parse a UPnP device descriptor XML to extract TV info
async fn fetch_tv_info(client: &reqwest::Client, location: &str) -> Result<Option<DiscoveredTv>> {
    let body = client.get(location).send().await?.text().await?;

    let mut reader = Reader::from_str(&body);

    let mut friendly_name = String::new();
    let mut model_name = String::new();
    let mut control_url = String::new();
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
                let text = reader.decoder().decode(e.as_ref()).unwrap_or_default().to_string();
                match current_element.as_str() {
                    "friendlyName" if friendly_name.is_empty() => {
                        friendly_name = text;
                    }
                    "modelName" if model_name.is_empty() => {
                        model_name = text;
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
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    if friendly_name.is_empty() || control_url.is_empty() {
        return Ok(None);
    }

    // Resolve relative controlURL against the location base URL
    let full_control_url = if control_url.starts_with("http") {
        control_url
    } else {
        // Extract base URL from location
        if let Some(base) = extract_base_url(location) {
            format!("{}{}", base, control_url)
        } else {
            control_url
        }
    };

    Ok(Some(DiscoveredTv {
        friendly_name,
        control_url: full_control_url,
        location_url: location.to_string(),
        model_name,
    }))
}

fn extract_base_url(url: &str) -> Option<String> {
    // Parse "http://192.168.1.100:8080/path/desc.xml" -> "http://192.168.1.100:8080"
    let without_scheme = url.strip_prefix("http://").or_else(|| url.strip_prefix("https://"))?;
    let scheme = if url.starts_with("https") { "https" } else { "http" };
    let host_port = without_scheme.split('/').next()?;
    Some(format!("{}://{}", scheme, host_port))
}

/// Cast a media file to a TV by sending SOAP SetAVTransportURI + Play
pub async fn cast_media(control_url: &str, media_url: &str, title: &str, mime_type: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let class = if mime_type.starts_with("audio/") {
        "object.item.audioItem.musicTrack"
    } else if mime_type.starts_with("video/") {
        "object.item.videoItem"
    } else if mime_type.starts_with("image/") {
        "object.item.imageItem.photo"
    } else {
        "object.item"
    };

    // Construct fully compliant DIDL-Lite metadata with standard XML escaping for SOAP
    let didl_metadata = format!(
        r#"&lt;DIDL-Lite xmlns=&quot;urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/&quot; xmlns:dc=&quot;http://purl.org/dc/elements/1.1/&quot; xmlns:upnp=&quot;urn:schemas-upnp-org:metadata-1-0/upnp/&quot;&gt;&lt;item id=&quot;0&quot; parentID=&quot;0&quot; restricted=&quot;1&quot;&gt;&lt;dc:title&gt;{}&lt;/dc:title&gt;&lt;upnp:class&gt;{}&lt;/upnp:class&gt;&lt;res protocolInfo=&quot;http-get:*:{}:*&quot;&gt;{}&lt;/res&gt;&lt;/item&gt;&lt;/DIDL-Lite&gt;"#,
        title, class, mime_type, media_url
    );

    // Step 1: SetAVTransportURI
    let set_uri_body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
    s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:SetAVTransportURI xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
            <InstanceID>0</InstanceID>
            <CurrentURI>{media_url}</CurrentURI>
            <CurrentURIMetaData>{didl_metadata}</CurrentURIMetaData>
        </u:SetAVTransportURI>
    </s:Body>
</s:Envelope>"#
    );

    let resp = client
        .post(control_url)
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .header("SOAPAction", "\"urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI\"")
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
    control_playback(control_url, "Play").await?;

    Ok(())
}

/// Send a playback control command (Play, Pause, Stop) to a TV
pub async fn control_playback(control_url: &str, action: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
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
        .post(control_url)
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
pub async fn set_next_media(control_url: &str, media_url: &str, title: &str, mime_type: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let class = if mime_type.starts_with("audio/") {
        "object.item.audioItem.musicTrack"
    } else if mime_type.starts_with("video/") {
        "object.item.videoItem"
    } else if mime_type.starts_with("image/") {
        "object.item.imageItem.photo"
    } else {
        "object.item"
    };

    // Construct fully compliant DIDL-Lite metadata with standard XML escaping for SOAP
    let didl_metadata = format!(
        r#"&lt;DIDL-Lite xmlns=&quot;urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/&quot; xmlns:dc=&quot;http://purl.org/dc/elements/1.1/&quot; xmlns:upnp=&quot;urn:schemas-upnp-org:metadata-1-0/upnp/&quot;&gt;&lt;item id=&quot;0&quot; parentID=&quot;0&quot; restricted=&quot;1&quot;&gt;&lt;dc:title&gt;{}&lt;/dc:title&gt;&lt;upnp:class&gt;{}&lt;/upnp:class&gt;&lt;res protocolInfo=&quot;http-get:*:{}:*&quot;&gt;{}&lt;/res&gt;&lt;/item&gt;&lt;/DIDL-Lite&gt;"#,
        title, class, mime_type, media_url
    );

    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
    s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:SetNextAVTransportURI xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
            <InstanceID>0</InstanceID>
            <NextURI>{media_url}</NextURI>
            <NextURIMetaData>{didl_metadata}</NextURIMetaData>
        </u:SetNextAVTransportURI>
    </s:Body>
</s:Envelope>"#
    );

    let resp = client
        .post(control_url)
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .header("SOAPAction", "\"urn:schemas-upnp-org:service:AVTransport:1#SetNextAVTransportURI\"")
        .body(body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("SetNextAVTransportURI failed (HTTP {}): {}", status, err_body);
    }

    debug!("SetNextAVTransportURI succeeded on {}", control_url);
    Ok(())
}

/// Query what media URI is currently active/playing on the TV
pub async fn get_position_info(control_url: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
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
        .header("SOAPAction", "\"urn:schemas-upnp-org:service:AVTransport:1#GetPositionInfo\"")
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
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
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
        .header("SOAPAction", "\"urn:schemas-upnp-org:service:AVTransport:1#GetTransportInfo\"")
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
    fn test_extract_base_url() {
        assert_eq!(
            extract_base_url("http://192.168.1.100:8080/path/desc.xml"),
            Some("http://192.168.1.100:8080".to_string())
        );
        assert_eq!(
            extract_base_url("http://192.168.1.5:49152/dmr/SamsungMRDesc.xml"),
            Some("http://192.168.1.5:49152".to_string())
        );
        assert_eq!(extract_base_url("not-a-url"), None);
    }

    #[test]
    fn test_extract_base_url_https() {
        assert_eq!(
            extract_base_url("https://10.0.0.1:443/device.xml"),
            Some("https://10.0.0.1:443".to_string())
        );
    }
}

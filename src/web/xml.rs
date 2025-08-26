// src\web\xml.rs
use crate::{
    database::{MediaDirectory, MediaFile},
    state::AppState,
};
use tracing::warn;

/// XML escape helper with enhanced Unicode support
fn xml_escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + s.len() / 4);
    
    for ch in s.chars() {
        match ch {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#39;"),
            // Handle control characters (except tab, newline, carriage return)
            c if (c as u32) < 32 && c != '\t' && c != '\n' && c != '\r' => {
                result.push_str(&format!("&#{};", c as u32));
            },
            // Handle other potentially problematic characters
            c => result.push(c),
        }
    }
    
    result
}

/// Get the server's IP address for use in URLs from the application state.
async fn get_server_ip(state: &AppState) -> String {
    // Use the SSDP interface from config if it's a specific IP address
    match &state.config.network.interface_selection {
        crate::config::NetworkInterfaceConfig::Specific(ip) => {
            return ip.clone();
        }
        _ => {
            // For Auto or All, fallback to server interface if it's not 0.0.0.0
            if state.config.server.interface != "0.0.0.0" && !state.config.server.interface.is_empty() {
                return state.config.server.interface.clone();
            }
        }
    }
    
    // Auto-detect the primary network interface IP
    if let Some(ip) = get_primary_interface_ip_async().await {
        return ip;
    }
    
    // Last resort
    warn!("Could not auto-detect IP, falling back to 127.0.0.1");
    "127.0.0.1".to_string()
}

/// Async version of primary interface IP detection
async fn get_primary_interface_ip_async() -> Option<String> {
    use tokio::process::Command;
    
    // Check if host IP is overridden via environment variable (for containers)
    if let Ok(host_ip) = std::env::var("VUIO_IP") {
        if !host_ip.is_empty() {
            return Some(host_ip);
        }
    }
    
    // Try to get the default route interface first (most reliable method)
    if let Ok(output) = Command::new("ip").args(&["route", "show", "default"]).output().await {
        let route_output = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = route_output.lines().next() {
            // Parse "default via X.X.X.X dev eth0" to get interface name
            if let Some(dev_pos) = line.find(" dev ") {
                let interface_part = &line[dev_pos + 5..];
                if let Some(interface_name) = interface_part.split_whitespace().next() {
                    // Get IP for this interface
                    if let Ok(ip_output) = Command::new("ip").args(&["addr", "show", interface_name]).output().await {
                        let ip_str = String::from_utf8_lossy(&ip_output.stdout);
                        for line in ip_str.lines() {
                            if line.trim().starts_with("inet ") && !line.contains("127.0.0.1") {
                                if let Some(ip_part) = line.trim().split_whitespace().nth(1) {
                                    if let Some(ip) = ip_part.split('/').next() {
                                        return Some(ip.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Fallback: try to find any non-loopback interface with an IP
    if let Ok(output) = Command::new("ip").args(&["addr", "show"]).output().await {
        let ip_str = String::from_utf8_lossy(&output.stdout);
        for line in ip_str.lines() {
            if line.trim().starts_with("inet ") && !line.contains("127.0.0.1") && !line.contains("169.254.") {
                if let Some(ip_part) = line.trim().split_whitespace().nth(1) {
                    if let Some(ip) = ip_part.split('/').next() {
                        // Prefer private network ranges for local discovery
                        if ip.starts_with("192.168.") || ip.starts_with("10.") || ip.starts_with("172.") {
                            return Some(ip.to_string());
                        }
                    }
                }
            }
        }
    }
    
    None
}

/// Get the appropriate UPnP class for a given MIME type.
fn get_upnp_class(mime_type: &str) -> &str {
    if mime_type.starts_with("video/") {
        "object.item.videoItem"
    } else if mime_type.starts_with("audio/") {
        "object.item.audioItem.musicTrack"
    } else if mime_type.starts_with("image/") {
        "object.item.imageItem.photo"
    } else {
        "object.item" // Generic item
    }
}

pub async fn generate_description_xml(state: &AppState) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
    <specVersion><major>1</major><minor>0</minor></specVersion>
    <device>
        <deviceType>urn:schemas-upnp-org:device:MediaServer:1</deviceType>
        <friendlyName>{}</friendlyName>
        <manufacturer>VuIO</manufacturer>
        <modelName>VuIO Server</modelName>
        <UDN>uuid:{}</UDN>
        <serviceList>
            <service>
                <serviceType>urn:schemas-upnp-org:service:ContentDirectory:1</serviceType>
                <serviceId>urn:upnp-org:serviceId:ContentDirectory</serviceId>
                <SCPDURL>/ContentDirectory.xml</SCPDURL>
                <controlURL>/control/ContentDirectory</controlURL>
                <eventSubURL>/event/ContentDirectory</eventSubURL>
            </service>
        </serviceList>
    </device>
</root>"#,
        xml_escape(&state.config.server.name),
        state.config.server.uuid
    )
}

pub fn generate_scpd_xml() -> String {
    // This XML is static and doesn't need formatting.
    r#"<?xml version="1.0" encoding="UTF-8"?>
<scpd xmlns="urn:schemas-upnp-org:service-1-0">
    <specVersion><major>1</major><minor>0</minor></specVersion>
    <actionList>
        <action>
            <name>Browse</name>
            <argumentList>
                <argument><name>ObjectID</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_ObjectID</relatedStateVariable></argument>
                <argument><name>BrowseFlag</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_BrowseFlag</relatedStateVariable></argument>
                <argument><name>Filter</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_Filter</relatedStateVariable></argument>
                <argument><name>StartingIndex</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_Index</relatedStateVariable></argument>
                <argument><name>RequestedCount</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_Count</relatedStateVariable></argument>
                <argument><name>SortCriteria</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_SortCriteria</relatedStateVariable></argument>
                <argument><name>Result</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_Result</relatedStateVariable></argument>
                <argument><name>NumberReturned</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_Count</relatedStateVariable></argument>
                <argument><name>TotalMatches</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_Count</relatedStateVariable></argument>
                <argument><name>UpdateID</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_UpdateID</relatedStateVariable></argument>
            </argumentList>
        </action>
    </actionList>
    <serviceStateTable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_ObjectID</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_BrowseFlag</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_Filter</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_Index</name><dataType>ui4</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_Count</name><dataType>ui4</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_SortCriteria</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_Result</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_UpdateID</name><dataType>ui4</dataType></stateVariable>
        <stateVariable sendEvents="yes"><name>SystemUpdateID</name><dataType>ui4</dataType></stateVariable>
        <stateVariable sendEvents="yes"><name>ContainerUpdateIDs</name><dataType>string</dataType></stateVariable>
    </serviceStateTable>
</scpd>"#.to_string()
}

pub async fn generate_browse_response(
    object_id: &str,
    subdirectories: &[MediaDirectory],
    files: &[MediaFile],
    state: &AppState,
) -> String {
    generate_browse_response_with_totals(object_id, subdirectories, files, state, None).await
}

pub async fn generate_browse_response_with_totals(
    object_id: &str,
    subdirectories: &[MediaDirectory],
    files: &[MediaFile],
    state: &AppState,
    total_matches: Option<usize>,
) -> String {
    use tracing::{debug, warn, error};
    
    debug!(
        "Generating browse response for object_id: '{}', {} subdirs, {} files",
        object_id,
        subdirectories.len(),
        files.len()
    );
    
    let server_ip = get_server_ip(state).await;
    let mut didl = String::from(r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/">"#);

    let number_returned = if object_id == "0" {
        // Root directory: show containers for media types
        didl.push_str(r#"<container id="video" parentID="0" restricted="1"><dc:title>Video</dc:title><upnp:class>object.container</upnp:class></container>"#);
        didl.push_str(r#"<container id="audio" parentID="0" restricted="1"><dc:title>Music</dc:title><upnp:class>object.container</upnp:class></container>"#);
        didl.push_str(r#"<container id="image" parentID="0" restricted="1"><dc:title>Pictures</dc:title><upnp:class>object.container</upnp:class></container>"#);
        3
    } else {
        // Add sub-containers to DIDL
        for (idx, container) in subdirectories.iter().enumerate() {
            if idx % 100 == 0 && idx > 0 {
                debug!("Processing subdirectory {}/{}: {}", idx, subdirectories.len(), container.name);
            }
            
            let container_id = format!("{}/{}", object_id.trim_end_matches('/'), container.name);
            let container_xml = format!(
                r#"<container id="{}" parentID="{}" restricted="1"><dc:title>{}</dc:title><upnp:class>object.container</upnp:class></container>"#,
                xml_escape(&container_id),
                xml_escape(object_id),
                xml_escape(&container.name)
            );
            didl.push_str(&container_xml);
        }

        // Add items to DIDL with enhanced processing and error handling
        for (idx, file) in files.iter().enumerate() {
            if idx % 100 == 0 && idx > 0 {
                debug!("Processing file {}/{}: '{}'", idx, files.len(), file.filename);
            }
            
            // Log files with potentially problematic characters
            if file.filename.chars().any(|c| c as u32 > 127) {
                debug!("Processing file with Unicode characters: '{}' ({})", file.filename, file.path.display());
            }
            
            let file_id = file.id.unwrap_or(0);
            let url = format!(
                "http://{}:{}/media/{}",
                server_ip,
                state.config.server.port,
                file_id
            );
            let upnp_class = get_upnp_class(&file.mime_type);
            
            // Enhanced metadata for audio items
            let metadata_xml = if file.mime_type.starts_with("audio/") {
                let mut metadata_parts = Vec::new();
                
                if let Some(ref artist) = file.artist {
                    metadata_parts.push(format!(
                        "<upnp:artist>{}</upnp:artist>",
                        xml_escape(artist)
                    ));
                }
                
                if let Some(ref album) = file.album {
                    metadata_parts.push(format!(
                        "<upnp:album>{}</upnp:album>",
                        xml_escape(album)
                    ));
                }
                
                if let Some(ref genre) = file.genre {
                    metadata_parts.push(format!(
                        "<upnp:genre>{}</upnp:genre>",
                        xml_escape(genre)
                    ));
                }
                
                if let Some(track_num) = file.track_number {
                    metadata_parts.push(format!(
                        "<upnp:originalTrackNumber>{}</upnp:originalTrackNumber>",
                        track_num
                    ));
                }
                
                if let Some(year) = file.year {
                    metadata_parts.push(format!(
                        "<dc:date>{}-01-01</dc:date>",
                        year
                    ));
                }
                
                if let Some(ref album_artist) = file.album_artist {
                    metadata_parts.push(format!(
                        "<upnp:albumArtist>{}</upnp:albumArtist>",
                        xml_escape(album_artist)
                    ));
                }
                
                metadata_parts.join("")
            } else {
                String::new()
            };
            
            // Create the XML for this item with proper error handling
            match std::panic::catch_unwind(|| {
                format!(
                    r#"<item id="{id}" parentID="{parent_id}" restricted="1">
                    <dc:title>{title}</dc:title>
                    {metadata}
                    <upnp:class>{upnp_class}</upnp:class>
                    <res protocolInfo="http-get:*:{mime}:*" size="{size}">{url}</res>
                </item>"#,
                    id = file_id,
                    parent_id = xml_escape(object_id),
                    title = xml_escape(&file.filename),
                    metadata = metadata_xml,
                    upnp_class = upnp_class,
                    mime = &file.mime_type,
                    size = file.size,
                    url = xml_escape(&url)
                )
            }) {
                Ok(item_xml) => {
                    didl.push_str(&item_xml);
                },
                Err(_) => {
                    error!("Failed to generate XML for file: '{}' ({})", file.filename, file.path.display());
                    // Create a simplified entry for problematic files
                    let safe_title = file.filename.chars()
                        .filter(|c| c.is_ascii_alphanumeric() || " .-_".contains(*c))
                        .collect::<String>();
                    let fallback_xml = format!(
                        r#"<item id="{id}" parentID="{parent_id}" restricted="1">
                        <dc:title>{title}</dc:title>
                        <upnp:class>{upnp_class}</upnp:class>
                        <res protocolInfo="http-get:*:{mime}:*" size="{size}">{url}</res>
                    </item>"#,
                        id = file_id,
                        parent_id = xml_escape(object_id),
                        title = xml_escape(&safe_title),
                        upnp_class = upnp_class,
                        mime = &file.mime_type,
                        size = file.size,
                        url = xml_escape(&url)
                    );
                    didl.push_str(&fallback_xml);
                }
            }
        }

        let total_items = subdirectories.len() + files.len();
        if total_items > 1000 {
            warn!("Large browse response: {} items for object_id: {}", total_items, object_id);
        }
        
        total_items
    };

    didl.push_str("</DIDL-Lite>");
    let final_total_matches = total_matches.unwrap_or(number_returned);

    let update_id = state.content_update_id.load(std::sync::atomic::Ordering::Relaxed);
    
    debug!("Browse response completed: {} items, DIDL size: {} bytes, total matches: {}", 
           number_returned, didl.len(), final_total_matches);
    
    let final_response = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:BrowseResponse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
            <Result>{}</Result>
            <NumberReturned>{}</NumberReturned>
            <TotalMatches>{}</TotalMatches>
            <UpdateID>{}</UpdateID>
        </u:BrowseResponse>
    </s:Body>
</s:Envelope>"#,
        xml_escape(&didl),
        number_returned,
        final_total_matches,
        update_id
    );
    
    debug!("Final XML response size: {} bytes", final_response.len());
    final_response
}
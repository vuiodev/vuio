// src\web\xml.rs
use crate::{database::MediaFile, state::AppState};
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
};
use tracing::warn;

/// XML escape helper
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Get the server's IP address for use in URLs from the application state.
fn get_server_ip(state: &AppState) -> String {
    // 1. Use the primary interface detected at startup.
    if let Some(iface) = state.platform_info.get_primary_interface() {
        return iface.ip_address.to_string();
    }

    // 2. Fallback to the configured server interface if it's not a wildcard.
    if state.config.server.interface != "0.0.0.0" && !state.config.server.interface.is_empty() {
        return state.config.server.interface.clone();
    }

    // 3. Fallback to trying to find any usable interface from the list.
    if let Some(iface) = state
        .platform_info
        .network_interfaces
        .iter()
        .find(|i| !i.is_loopback && i.is_up)
    {
        return iface.ip_address.to_string();
    }

    // 4. Final fallback.
    warn!("Could not determine a specific server IP for XML description; falling back to 127.0.0.1.");
    "127.0.0.1".to_string()
}

/// Get the appropriate UPnP class for a given MIME type.
fn get_upnp_class(mime_type: &str) -> &str {
    if mime_type.starts_with("video/") {
        "object.item.videoItem"
    } else if mime_type.starts_with("audio/") {
        "object.item.audioItem"
    } else if mime_type.starts_with("image/") {
        "object.item.imageItem"
    } else {
        "object.item" // Generic item
    }
}

pub fn generate_description_xml(state: &AppState) -> String {
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

pub fn generate_browse_response(
    object_id: &str,
    files: &[MediaFile],
    state: &AppState,
) -> String {
    let server_ip = get_server_ip(state);
    let mut didl = String::from(r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/">"#);
    let number_returned;

    if object_id == "0" {
        // Root directory: show containers for media types
        didl.push_str(r#"<container id="video" parentID="0" restricted="1"><dc:title>Video</dc:title><upnp:class>object.container</upnp:class></container>"#);
        didl.push_str(r#"<container id="audio" parentID="0" restricted="1"><dc:title>Music</dc:title><upnp:class>object.container</upnp:class></container>"#);
        didl.push_str(r#"<container id="image" parentID="0" restricted="1"><dc:title>Pictures</dc:title><upnp:class>object.container</upnp:class></container>"#);
        number_returned = 3;
    } else {
        let mut sub_containers = HashSet::new();
        let mut items = Vec::new();

        let (media_type_filter, path_prefix_str) = if object_id.starts_with("video") {
            ("video/", object_id.strip_prefix("video").unwrap_or("").trim_start_matches('/'))
        } else if object_id.starts_with("audio") {
            ("audio/", object_id.strip_prefix("audio").unwrap_or("").trim_start_matches('/'))
        } else if object_id.starts_with("image") {
            ("image/", object_id.strip_prefix("image").unwrap_or("").trim_start_matches('/'))
        } else {
            ("", "")
        };
        
        let media_root = state.config.get_primary_media_dir();
        // Create a Path from the ObjectID's path part for reliable comparison
        let browse_path = Path::new(path_prefix_str);

        tracing::info!("Browse request - media_root: {:?}, browse_path: {:?}, media_type_filter: {}", media_root, browse_path, media_type_filter);
        tracing::info!("Total files to filter: {}", files.len());

        for file in files.iter().filter(|f| f.mime_type.starts_with(media_type_filter)) {
            tracing::debug!("Processing file: {:?} with mime_type: {}", file.path, file.mime_type);
            
            // Try to get relative path from media_root, handling case sensitivity and path normalization
            let relative_path_result = if cfg!(windows) {
                // On Windows, do case-insensitive comparison
                let file_path_lower = file.path.to_string_lossy().to_lowercase();
                let media_root_lower = media_root.to_string_lossy().to_lowercase();
                
                if file_path_lower.starts_with(&media_root_lower) {
                    // Manually strip the prefix and create a relative path
                    let remaining = &file.path.to_string_lossy()[media_root.to_string_lossy().len()..];
                    let remaining = remaining.trim_start_matches(['/', '\\']);
                    if remaining.is_empty() {
                        Ok(PathBuf::new())
                    } else {
                        Ok(PathBuf::from(remaining))
                    }
                } else {
                    Err(())
                }
            } else {
                // On Unix systems, use standard strip_prefix
                file.path.strip_prefix(&media_root).map(|p| p.to_path_buf()).map_err(|_| ())
            };
            
            if let Ok(relative_path) = relative_path_result {
                tracing::debug!("Relative path: {:?}", relative_path);
                if let Some(parent_path) = relative_path.parent() {
                    tracing::debug!("Parent path: {:?}, browse_path: {:?}", parent_path, browse_path);
                    // Check if the file is a direct child of the directory we're browsing
                    if parent_path == browse_path {
                        tracing::debug!("Adding file as direct child: {:?}", file.filename);
                        items.push(file);
                    } 
                    // Check if the file is in an immediate subdirectory
                    else if parent_path.starts_with(browse_path) {
                        if let Ok(path_after_browse) = parent_path.strip_prefix(browse_path) {
                            if let Some(Component::Normal(name)) = path_after_browse.components().next() {
                                sub_containers.insert(name.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            } else {
                tracing::debug!("Failed to strip prefix from file path: {:?}", file.path);
            }
        }
        
        // Add containers to DIDL
        let mut sorted_containers: Vec<_> = sub_containers.into_iter().collect();
        sorted_containers.sort_by_key(|a| a.to_lowercase());
        for container_name in &sorted_containers {
            let container_id = format!("{}/{}", object_id.trim_end_matches('/'), container_name);
            didl.push_str(&format!(
                r#"<container id="{}" parentID="{}" restricted="1"><dc:title>{}</dc:title><upnp:class>object.container</upnp:class></container>"#,
                xml_escape(&container_id),
                xml_escape(object_id),
                xml_escape(container_name)
            ));
        }

        // Add items to DIDL
        items.sort_by_key(|f| f.filename.to_lowercase());
        for file in &items {
            let file_id = file.id.unwrap_or(0);
            let url = format!("http://{}:{}/media/{}", server_ip, state.config.server.port, file_id);
            let upnp_class = get_upnp_class(&file.mime_type);
            didl.push_str(&format!(
                r#"<item id="{id}" parentID="{parent_id}" restricted="1">
                    <dc:title>{title}</dc:title>
                    <upnp:class>{upnp_class}</upnp:class>
                    <res protocolInfo="http-get:*:{mime}:*" size="{size}">{url}</res>
                </item>"#,
                id = file_id,
                parent_id = xml_escape(object_id),
                title = xml_escape(&file.filename),
                upnp_class = upnp_class,
                mime = &file.mime_type,
                size = file.size,
                url = xml_escape(&url)
            ));
        }
        
        number_returned = sorted_containers.len() + items.len();
    }

    didl.push_str("</DIDL-Lite>");
    let total_matches = number_returned;

    let update_id = state.content_update_id.load(std::sync::atomic::Ordering::Relaxed);
    
    format!(
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
        total_matches,
        update_id
    )
}
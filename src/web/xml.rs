// src\web\xml.rs
use crate::{
    database::{MediaDirectory, MediaFile},
    state::AppState,
};

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

/// Format duration in seconds to HH:MM:SS format for DLNA
fn format_duration(duration_seconds: u64) -> String {
    let hours = duration_seconds / 3600;
    let minutes = (duration_seconds % 3600) / 60;
    let seconds = duration_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
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
            <service>
                <serviceType>urn:schemas-upnp-org:service:ConnectionManager:1</serviceType>
                <serviceId>urn:upnp-org:serviceId:ConnectionManager</serviceId>
                <SCPDURL>/ConnectionManager.xml</SCPDURL>
                <controlURL>/control/ConnectionManager</controlURL>
                <eventSubURL>/event/ConnectionManager</eventSubURL>
            </service>
            <service>
                <serviceType>urn:microsoft.com:service:X_MS_MediaReceiverRegistrar:1</serviceType>
                <serviceId>urn:microsoft.com:serviceId:X_MS_MediaReceiverRegistrar</serviceId>
                <SCPDURL>/X_MS_MediaReceiverRegistrar.xml</SCPDURL>
                <controlURL>/control/X_MS_MediaReceiverRegistrar</controlURL>
                <eventSubURL>/event/X_MS_MediaReceiverRegistrar</eventSubURL>
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

pub fn generate_connection_manager_scpd() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<scpd xmlns="urn:schemas-upnp-org:service-1-0">
    <specVersion><major>1</major><minor>0</minor></specVersion>
    <actionList>
        <action>
            <name>GetProtocolInfo</name>
            <argumentList>
                <argument><name>Source</name><direction>out</direction><relatedStateVariable>SourceProtocolInfo</relatedStateVariable></argument>
                <argument><name>Sink</name><direction>out</direction><relatedStateVariable>SinkProtocolInfo</relatedStateVariable></argument>
            </argumentList>
        </action>
        <action>
            <name>GetCurrentConnectionIDs</name>
            <argumentList>
                <argument><name>ConnectionIDs</name><direction>out</direction><relatedStateVariable>CurrentConnectionIDs</relatedStateVariable></argument>
            </argumentList>
        </action>
        <action>
            <name>GetCurrentConnectionInfo</name>
            <argumentList>
                <argument><name>ConnectionID</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_ConnectionID</relatedStateVariable></argument>
                <argument><name>RcsID</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_RcsID</relatedStateVariable></argument>
                <argument><name>AVTransportID</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_AVTransportID</relatedStateVariable></argument>
                <argument><name>ProtocolInfo</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_ProtocolInfo</relatedStateVariable></argument>
                <argument><name>PeerConnectionManager</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_ConnectionManager</relatedStateVariable></argument>
                <argument><name>PeerConnectionID</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_ConnectionID</relatedStateVariable></argument>
                <argument><name>Direction</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_Direction</relatedStateVariable></argument>
                <argument><name>Status</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_ConnectionStatus</relatedStateVariable></argument>
            </argumentList>
        </action>
    </actionList>
    <serviceStateTable>
        <stateVariable sendEvents="yes"><name>SourceProtocolInfo</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="yes"><name>SinkProtocolInfo</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="yes"><name>CurrentConnectionIDs</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_ConnectionID</name><dataType>i4</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_RcsID</name><dataType>i4</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_AVTransportID</name><dataType>i4</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_ProtocolInfo</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_ConnectionManager</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_Direction</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_ConnectionStatus</name><dataType>string</dataType></stateVariable>
    </serviceStateTable>
</scpd>"#.to_string()
}

pub fn generate_registrar_scpd() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<scpd xmlns="urn:schemas-upnp-org:service-1-0">
    <specVersion><major>1</major><minor>0</minor></specVersion>
    <actionList>
        <action>
            <name>IsAuthorized</name>
            <argumentList>
                <argument><name>DeviceID</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_DeviceID</relatedStateVariable></argument>
                <argument><name>Result</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_Result</relatedStateVariable></argument>
            </argumentList>
        </action>
        <action>
            <name>RegisterDevice</name>
            <argumentList>
                <argument><name>RegistrationReqMsg</name><direction>in</direction><relatedStateVariable>A_ARG_TYPE_RegistrationReqMsg</relatedStateVariable></argument>
                <argument><name>RegistrationRespMsg</name><direction>out</direction><relatedStateVariable>A_ARG_TYPE_RegistrationRespMsg</relatedStateVariable></argument>
            </argumentList>
        </action>
    </actionList>
    <serviceStateTable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_DeviceID</name><dataType>string</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_Result</name><dataType>ui4</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_RegistrationReqMsg</name><dataType>bin.base64</dataType></stateVariable>
        <stateVariable sendEvents="no"><name>A_ARG_TYPE_RegistrationRespMsg</name><dataType>bin.base64</dataType></stateVariable>
        <stateVariable sendEvents="yes"><name>AuthorizationDeniedUpdateID</name><dataType>ui4</dataType></stateVariable>
        <stateVariable sendEvents="yes"><name>ValidationSucceededUpdateID</name><dataType>ui4</dataType></stateVariable>
        <stateVariable sendEvents="yes"><name>ValidationDeniedUpdateID</name><dataType>ui4</dataType></stateVariable>
    </serviceStateTable>
</scpd>"#.to_string()
}

pub async fn generate_browse_response(
    object_id: &str,
    subdirectories: &[MediaDirectory],
    files: &[MediaFile],
    state: &AppState,
    server_ip: &str,
) -> String {
    generate_browse_response_with_totals(object_id, subdirectories, files, state, server_ip, None).await
}

pub async fn generate_browse_response_with_totals(
    object_id: &str,
    subdirectories: &[MediaDirectory],
    files: &[MediaFile],
    state: &AppState,
    server_ip: &str,
    total_matches: Option<usize>,
) -> String {
    use tracing::{debug, warn};
    
    let client = crate::web::client::CURRENT_CLIENT.try_with(|c| *c)
        .unwrap_or(crate::web::client::DlnaClientProfile::Standard);

    debug!(
        "Generating browse response for object_id: '{}', {} subdirs, {} files, client: {:?}",
        object_id,
        subdirectories.len(),
        files.len(),
        client
    );
    

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
            
            let path_str = container.path.to_string_lossy();
            let container_id = if path_str.starts_with('d') && path_str[1..].chars().all(|c| c.is_ascii_digit()) {
                format!("{}/{}", object_id.trim_end_matches('/'), path_str)
            } else {
                format!("{}/{}", object_id.trim_end_matches('/'), container.name)
            };

            let mut media_class_xml = String::new();
            if client == crate::web::client::DlnaClientProfile::SonyBdp || 
               client == crate::web::client::DlnaClientProfile::SonyBravia || 
               client == crate::web::client::DlnaClientProfile::PlayStation {
                let class_char = if container_id.contains("audio") || container_id.contains("music") {
                    "A"
                } else if container_id.contains("image") || container_id.contains("picture") {
                    "P"
                } else {
                    "V"
                };
                media_class_xml = format!(
                    r#"<av:mediaClass xmlns:av="urn:schemas-sony-com:av">{}</av:mediaClass>"#,
                    class_char
                );
            }

            let container_xml = format!(
                r#"<container id="{}" parentID="{}" restricted="1"><dc:title>{}</dc:title><upnp:class>object.container</upnp:class>{}</container>"#,
                xml_escape(&container_id),
                xml_escape(object_id),
                xml_escape(&container.name),
                media_class_xml
            );
            didl.push_str(&container_xml);
        }

        // Add items to DIDL with enhanced processing and error handling
        for (idx, file) in files.iter().enumerate() {
            if idx % 100 == 0 && idx > 0 {
                debug!("Processing file {}/{}: '{}'", idx, files.len(), file.filename);
            }
            
            // Skip files without valid IDs - they can't be served
            let file_id = match file.id {
                Some(id) if id > 0 => id,
                _ => {
                    debug!("Skipping file without valid ID: '{}' ({})", file.filename, file.path.display());
                    continue;
                }
            };
            
            // Log files with potentially problematic characters
            if file.filename.chars().any(|c| c as u32 > 127) {
                debug!("Processing file with Unicode characters: '{}' ({})", file.filename, file.path.display());
            }
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
            
            // Create the XML for this item with autoplay attributes
            // Add duration for media files if available
            let duration_attr = if file.mime_type.starts_with("video/") || file.mime_type.starts_with("audio/") {
                if let Some(duration) = file.duration {
                    format!(" duration=\"{}\"", format_duration(duration.as_secs()))
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            // Use enhanced DLNA flags that support autoplay and streaming
            let dlna_flags = if state.config.media.autoplay_enabled {
                "DLNA.ORG_OP=11;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000"
            } else {
                "DLNA.ORG_OP=11;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=00D00000000000000000000000000000"
            };

            let has_srt = file.path.with_extension("srt").exists();
            let mut title = file.filename.clone();
            if client == crate::web::client::DlnaClientProfile::LgTv && has_srt {
                title.push('.');
            }
            let title_escaped = xml_escape(&title);

            let mime_override = match client {
                crate::web::client::DlnaClientProfile::SamsungTv if file.mime_type == "video/x-matroska" => {
                    "video/x-mkv".to_string()
                }
                crate::web::client::DlnaClientProfile::SamsungTv if file.mime_type == "video/x-msvideo" => {
                    "video/mpeg".to_string()
                }
                crate::web::client::DlnaClientProfile::SonyBdp if file.mime_type == "video/x-matroska" || file.mime_type == "video/mpeg" => {
                    "video/divx".to_string()
                }
                crate::web::client::DlnaClientProfile::Xbox if file.mime_type == "video/x-msvideo" => {
                    "video/avi".to_string()
                }
                _ => file.mime_type.clone(),
            };

            let mut res_tags = Vec::new();
            res_tags.push(format!(
                r#"<res protocolInfo="http-get:*:{mime}:{dlna_flags}" size="{size}"{duration}>{url}</res>"#,
                mime = mime_override,
                dlna_flags = dlna_flags,
                size = file.size,
                duration = duration_attr,
                url = xml_escape(&url)
            ));

            if client == crate::web::client::DlnaClientProfile::LgTv && has_srt {
                let srt_url = format!("http://{}:{}/media/{}/subtitle", server_ip, state.config.server.port, file_id);
                res_tags.push(format!(
                    r#"<res protocolInfo="http-get:*:text/srt:*">{}</res>"#,
                    xml_escape(&srt_url)
                ));
            }

            let mut caption_ex_xml = String::new();
            if client == crate::web::client::DlnaClientProfile::SamsungTv && has_srt {
                let srt_url = format!("http://{}:{}/media/{}/subtitle", server_ip, state.config.server.port, file_id);
                caption_ex_xml = format!(
                    r#"<sec:CaptionInfoEx sec:type="srt">{}</sec:CaptionInfoEx>"#,
                    xml_escape(&srt_url)
                );
            }

            let mut dcm_info_xml = String::new();
            if client == crate::web::client::DlnaClientProfile::SamsungTv {
                let bookmarks_guard = state.bookmarks.lock().await;
                let bookmark_sec = bookmarks_guard.get(&file_id).cloned().unwrap_or(0);
                dcm_info_xml = format!(
                    r#"<sec:dcmInfo>CREATIONDATE=0,FOLDER={},BM={}</sec:dcmInfo>"#,
                    xml_escape(&file.filename),
                    bookmark_sec
                );
            }

            let item_xml = format!(
                r#"<item id="{id}" parentID="{parent_id}" restricted="1">
                <dc:title>{title}</dc:title>
                {metadata}
                <upnp:class>{upnp_class}</upnp:class>
                {res_elements}
                {caption_ex}
                {dcm_info}
            </item>"#,
                id = file_id,
                parent_id = xml_escape(object_id),
                title = title_escaped,
                metadata = metadata_xml,
                upnp_class = upnp_class,
                res_elements = res_tags.join("\n"),
                caption_ex = caption_ex_xml,
                dcm_info = dcm_info_xml
            );
            didl.push_str(&item_xml);
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
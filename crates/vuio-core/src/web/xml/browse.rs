use super::rendering::*;
use super::*;

pub async fn generate_browse_response(
    object_id: &str,
    subdirectories: &[MediaDirectory],
    files: &[MediaFile],
    state: &AppState<impl DatabaseManager>,
    server_ip: &str,
    total_matches: usize,
) -> String {
    use std::fmt::Write;
    use tracing::{debug, warn};

    let client = crate::web::client::CURRENT_CLIENT
        .try_with(|c| *c)
        .unwrap_or(crate::web::client::DlnaClientProfile::Standard);

    debug!(
        "Generating browse response for object_id: '{}', {} subdirs, {} files, client: {:?}",
        object_id,
        subdirectories.len(),
        files.len(),
        client
    );

    let estimated_capacity = 750 + subdirectories.len() * 250 + files.len() * 500;
    let mut final_response = String::with_capacity(estimated_capacity);
    final_response.push_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:BrowseResponse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
            <Result>"#,
    );
    let result_start = final_response.len();
    let mut didl = SoapResultWriter(&mut final_response);
    didl.push_str(r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" xmlns:pv="http://www.pv.com/pvplay/" xmlns:sec="http://www.sec.co.kr/">"#);

    let number_returned = {
        // Add sub-containers to DIDL
        for (idx, container) in subdirectories.iter().enumerate() {
            if idx % 100 == 0 && idx > 0 {
                debug!(
                    "Processing subdirectory {}/{}: {}",
                    idx,
                    subdirectories.len(),
                    container.name
                );
            }

            let path_str = container.path.to_string_lossy();
            let container_id = if path_str.starts_with("audio/")
                || path_str.starts_with("video/")
                || path_str.starts_with("image/")
                || path_str.starts_with("radio/")
                || path_str == "audio"
                || path_str == "video"
                || path_str == "image"
                || path_str == "radio"
            {
                path_str.into_owned()
            } else if path_str.starts_with('d') && path_str[1..].chars().all(|c| c.is_ascii_digit())
            {
                format!("{}/{}", object_id.trim_end_matches('/'), path_str)
            } else {
                format!("{}/{}", object_id.trim_end_matches('/'), container.name)
            };

            let _ = write!(
                &mut didl,
                r#"<container id="{}" parentID="{}" restricted="1" searchable="0" childCount="1"><dc:title>{}</dc:title><upnp:class>object.container.storageFolder</upnp:class>"#,
                xml_escape(&container_id),
                xml_escape(object_id),
                xml_escape(&container.name)
            );

            if client == crate::web::client::DlnaClientProfile::SonyBdp
                || client == crate::web::client::DlnaClientProfile::SonyBravia
                || client == crate::web::client::DlnaClientProfile::PlayStation
            {
                let class_char = if container_id.contains("audio") || container_id.contains("music")
                {
                    "A"
                } else if container_id.contains("image") || container_id.contains("picture") {
                    "P"
                } else {
                    "V"
                };
                let _ = write!(
                    &mut didl,
                    r#"<av:mediaClass xmlns:av="urn:schemas-sony-com:av">{}</av:mediaClass>"#,
                    class_char
                );
            }
            didl.push_str("</container>");
        }

        let mut bookmarks_guard = if client == crate::web::client::DlnaClientProfile::SamsungTv
            || client == crate::web::client::DlnaClientProfile::SamsungTvQ
        {
            Some(state.bookmarks.lock().await)
        } else {
            None
        };

        // Add items to DIDL with enhanced processing and error handling
        for (idx, file) in files.iter().enumerate() {
            if idx % 100 == 0 && idx > 0 {
                debug!(
                    "Processing file {}/{}: '{}'",
                    idx,
                    files.len(),
                    file.filename
                );
            }

            // Skip files without valid IDs - they can't be served
            let file_id = match file.id {
                Some(id) if id > 0 => id,
                _ => {
                    debug!(
                        "Skipping file without valid ID: '{}' ({})",
                        file.filename,
                        file.path.display()
                    );
                    continue;
                }
            };

            // Log files with potentially problematic characters
            if file.filename.chars().any(|c| c as u32 > 127) {
                debug!(
                    "Processing file with Unicode characters: '{}' ({})",
                    file.filename,
                    file.path.display()
                );
            }

            let upnp_class = get_upnp_class(&file.mime_type);

            let has_srt = file.subtitle_available;
            let mut title = didl_display_title(file.title.as_deref(), &file.filename, client);
            if client == crate::web::client::DlnaClientProfile::LgTv && has_srt {
                title.push('.');
            }
            let title_escaped = xml_escape(&title);

            let _ = write!(
                &mut didl,
                r#"<item id="{}" parentID="{}" restricted="1">
                <dc:title>{}</dc:title>
                "#,
                file_id,
                xml_escape(object_id),
                title_escaped
            );

            // Enhanced metadata for audio items
            if file.mime_type.starts_with("audio/") {
                if let Some(ref artist) = file.artist {
                    let _ = write!(
                        &mut didl,
                        "<upnp:artist>{}</upnp:artist>",
                        xml_escape(artist)
                    );
                }

                if let Some(ref album) = file.album {
                    let _ = write!(&mut didl, "<upnp:album>{}</upnp:album>", xml_escape(album));
                }

                if let Some(ref genre) = file.genre {
                    let _ = write!(&mut didl, "<upnp:genre>{}</upnp:genre>", xml_escape(genre));
                }

                if let Some(track_num) = file.track_number {
                    let _ = write!(
                        &mut didl,
                        "<upnp:originalTrackNumber>{}</upnp:originalTrackNumber>",
                        track_num
                    );
                }

                if let Some(year) = file.year {
                    let _ = write!(&mut didl, "<dc:date>{}-01-01</dc:date>", year);
                }

                if let Some(ref album_artist) = file.album_artist {
                    let _ = write!(
                        &mut didl,
                        "<upnp:albumArtist>{}</upnp:albumArtist>",
                        xml_escape(album_artist)
                    );
                }

                let _ = write!(
                    &mut didl,
                    "<upnp:albumArtURI>http://{}:{}/media/{}/cover</upnp:albumArtURI>",
                    server_ip,
                    state.current_config().server.port,
                    file_id
                );
            }

            let _ = write!(
                &mut didl,
                r#"<upnp:class>{}</upnp:class>
                "#,
                upnp_class
            );

            let is_radio = file.mime_type == "audio/radio";

            // Create the XML for this item with autoplay attributes
            // Add duration for media files if available
            let duration_secs = if (file.mime_type.starts_with("video/")
                || file.mime_type.starts_with("audio/"))
                && !is_radio
            {
                file.duration.map(|d| d.as_secs())
            } else {
                None
            };

            // Use enhanced DLNA flags that support autoplay and streaming
            let dlna_flags = if state.current_config().media.autoplay_enabled {
                "DLNA.ORG_OP=11;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000"
            } else {
                "DLNA.ORG_OP=11;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=00D00000000000000000000000000000"
            };

            let mime_override = if is_radio {
                "audio/mpeg"
            } else {
                match client {
                    crate::web::client::DlnaClientProfile::SamsungTv
                    | crate::web::client::DlnaClientProfile::SamsungTvQ
                        if file.mime_type == "video/x-matroska" =>
                    {
                        "video/x-mkv"
                    }
                    crate::web::client::DlnaClientProfile::SamsungTv
                    | crate::web::client::DlnaClientProfile::SamsungTvQ
                        if file.mime_type == "video/x-msvideo" =>
                    {
                        "video/mpeg"
                    }
                    crate::web::client::DlnaClientProfile::SonyBdp
                        if file.mime_type == "video/x-matroska"
                            || file.mime_type == "video/mpeg" =>
                    {
                        "video/divx"
                    }
                    crate::web::client::DlnaClientProfile::Xbox
                        if file.mime_type == "video/x-msvideo" =>
                    {
                        "video/avi"
                    }
                    _ => &file.mime_type,
                }
            };

            let size_val = if is_radio {
                "0".to_string()
            } else {
                file.size.to_string()
            };

            let _ = write!(
                &mut didl,
                r#"<res protocolInfo="http-get:*:{mime}:{dlna_flags}" size="{size}""#,
                mime = mime_override,
                dlna_flags = dlna_flags,
                size = size_val
            );

            if let Some(secs) = duration_secs {
                let _ = write!(&mut didl, r#" duration="{}""#, format_duration(secs));
            }

            if (client == crate::web::client::DlnaClientProfile::LgTv
                || client == crate::web::client::DlnaClientProfile::PanasonicTv)
                && has_srt
            {
                let _ = write!(
                    &mut didl,
                    r#" pv:subtitleFileUri="http://{}:{}/media/{}/subtitle" pv:subtitleFileType="SRT""#,
                    server_ip,
                    state.current_config().server.port,
                    file_id
                );
            }

            let _ = write!(
                &mut didl,
                r#">http://{}:{}/media/{}</res>"#,
                server_ip,
                state.current_config().server.port,
                file_id
            );

            if client == crate::web::client::DlnaClientProfile::LgTv && has_srt {
                let _ = write!(
                    &mut didl,
                    r#"
                <res protocolInfo="http-get:*:text/srt:*">http://{}:{}/media/{}/subtitle</res>"#,
                    server_ip,
                    state.current_config().server.port,
                    file_id
                );
            }

            if (client == crate::web::client::DlnaClientProfile::SamsungTv
                || client == crate::web::client::DlnaClientProfile::SamsungTvQ)
                && has_srt
            {
                let _ = write!(
                    &mut didl,
                    r#"
                <sec:CaptionInfoEx sec:type="srt">http://{}:{}/media/{}/subtitle</sec:CaptionInfoEx>"#,
                    server_ip,
                    state.current_config().server.port,
                    file_id
                );
            }

            if client == crate::web::client::DlnaClientProfile::SamsungTv
                || client == crate::web::client::DlnaClientProfile::SamsungTvQ
            {
                let bookmark_sec = bookmarks_guard
                    .as_mut()
                    .and_then(|g| g.get(&file_id).copied())
                    .unwrap_or(0);
                let bookmark_val = if client == crate::web::client::DlnaClientProfile::SamsungTvQ {
                    bookmark_sec * 1000
                } else {
                    bookmark_sec
                };
                let _ = write!(
                    &mut didl,
                    r#"
                <sec:dcmInfo>CREATIONDATE=0,FOLDER={},BM={}</sec:dcmInfo>"#,
                    xml_escape(&file.filename),
                    bookmark_val
                );
            }

            didl.push_str("\n            </item>");
        }

        let total_items = subdirectories.len() + files.len();
        if total_items > 1000 {
            warn!(
                "Large browse response: {} items for object_id: {}",
                total_items, object_id
            );
        }

        total_items
    };

    didl.push_str("</DIDL-Lite>");
    let update_id = state
        .content_update_id
        .load(std::sync::atomic::Ordering::SeqCst);

    debug!(
        "Browse response completed: {} items, DIDL size: {} bytes, total matches: {}",
        number_returned,
        final_response.len() - result_start,
        total_matches
    );

    let _ = write!(
        &mut final_response,
        r#"</Result>
            <NumberReturned>{}</NumberReturned>
            <TotalMatches>{}</TotalMatches>
            <UpdateID>{}</UpdateID>
        </u:BrowseResponse>
    </s:Body>
</s:Envelope>"#,
        number_returned, total_matches, update_id
    );

    debug!("Final XML response size: {} bytes", final_response.len());
    final_response
}

/// Single-container BrowseMetadata response. Samsung TVs probe folders this way and
/// use `childCount` to decide whether a folder is empty.
pub fn generate_container_metadata_response(
    object_id: &str,
    parent_id: &str,
    title: &str,
    child_count: usize,
    update_id: u32,
) -> String {
    use std::fmt::Write;

    let mut response = String::with_capacity(768);
    response.push_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:BrowseResponse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
            <Result>"#,
    );
    let mut didl = SoapResultWriter(&mut response);
    didl.push_str(r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" xmlns:sec="http://www.sec.co.kr/">"#);
    let _ = write!(
        &mut didl,
        r#"<container id="{}" parentID="{}" restricted="1" searchable="0" childCount="{child_count}"><dc:title>{}</dc:title><upnp:class>object.container.storageFolder</upnp:class></container>"#,
        xml_escape(object_id),
        xml_escape(parent_id),
        xml_escape(title)
    );
    didl.push_str("</DIDL-Lite>");
    let _ = write!(
        &mut response,
        r#"</Result>
            <NumberReturned>1</NumberReturned>
            <TotalMatches>1</TotalMatches>
            <UpdateID>{update_id}</UpdateID>
        </u:BrowseResponse>
    </s:Body>
</s:Envelope>"#
    );
    response
}

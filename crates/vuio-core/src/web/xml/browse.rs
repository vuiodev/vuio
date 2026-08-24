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
            let spec = ContainerSpec::folder(
                directory_container_id(object_id, &path_str, &container.name),
                &container.name,
            );
            let _ = write_container(
                &mut didl,
                &spec,
                object_id,
                client,
                server_ip,
                state.http_binding.port(),
            );
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
                    state.http_binding.port(),
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

            // The same second resource the indexed path offers. Kept in step
            // with `rendering.rs` deliberately: an item reached through this
            // fallback must not lose the alternative it would have been given
            // through the other.
            let transcoded = crate::web::transcode_advert(state)
                .filter(|_| !is_radio)
                .filter(|_| {
                    crate::web::item_needs_transcode(
                        file.stream.codec.as_deref(),
                        &file.mime_type,
                        &file.filename,
                    )
                })
                .filter(|advert| advert.resource_for(&file.mime_type).is_some())
                .filter(|_| {
                    !file.mime_type.starts_with("video/")
                        || crate::web::item_can_remux_video(file.stream.video_codec.as_deref())
                });
            if let Some(advert) = transcoded.filter(|a| a.first) {
                let _ = advert.write_didl(
                    &mut didl,
                    server_ip,
                    state.http_binding.port(),
                    file_id,
                    &file.mime_type,
                    duration_secs,
                );
            }

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
                    state.http_binding.port(),
                    file_id
                );
            }

            let _ = write!(
                &mut didl,
                r#">http://{}:{}/media/{}</res>"#,
                server_ip,
                state.http_binding.port(),
                file_id
            );

            if let Some(advert) = transcoded.filter(|a| !a.first) {
                let _ = advert.write_didl(
                    &mut didl,
                    server_ip,
                    state.http_binding.port(),
                    file_id,
                    &file.mime_type,
                    duration_secs,
                );
            }

            if client == crate::web::client::DlnaClientProfile::LgTv && has_srt {
                let _ = write!(
                    &mut didl,
                    r#"
                <res protocolInfo="http-get:*:text/srt:*">http://{}:{}/media/{}/subtitle</res>"#,
                    server_ip,
                    state.http_binding.port(),
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
                    state.http_binding.port(),
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

/// A DIDL document listing containers only.
///
/// Every level of the music tree above the tracks is built from this, which is
/// why it takes fully-formed [`ContainerSpec`]s rather than deriving ids and
/// classes from a path the way the folder browse does.
pub fn generate_container_list_response(
    object_id: &str,
    containers: &[ContainerSpec],
    total_matches: usize,
    context: &BrowseRenderContext,
) -> String {
    use std::fmt::Write;

    let mut response = String::with_capacity(750 + containers.len() * 300);
    response.push_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:BrowseResponse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
            <Result>"#,
    );
    let mut didl = SoapResultWriter(&mut response);
    didl.push_str(r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" xmlns:pv="http://www.pv.com/pvplay/" xmlns:sec="http://www.sec.co.kr/">"#);
    for container in containers {
        let _ = write_container(
            &mut didl,
            container,
            object_id,
            context.client,
            &context.server_ip,
            context.server_port,
        );
    }
    didl.push_str("</DIDL-Lite>");
    let _ = write!(
        &mut response,
        r#"</Result>
            <NumberReturned>{}</NumberReturned>
            <TotalMatches>{}</TotalMatches>
            <UpdateID>{}</UpdateID>
        </u:BrowseResponse>
    </s:Body>
</s:Envelope>"#,
        containers.len(),
        total_matches,
        context.update_id
    );
    response
}

/// Single-container BrowseMetadata response. Samsung TVs probe folders this way and
/// use `childCount` to decide whether a folder is empty.
pub fn generate_container_metadata_response(
    object_id: &str,
    parent_id: &str,
    title: &str,
    class: &str,
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
        r#"<container id="{}" parentID="{}" restricted="1" searchable="0" childCount="{child_count}"><dc:title>{}</dc:title><upnp:class>{}</upnp:class></container>"#,
        xml_escape(object_id),
        xml_escape(parent_id),
        xml_escape(title),
        class
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

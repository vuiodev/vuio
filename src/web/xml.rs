// src\web\xml.rs
use crate::{
    database::{
        DatabaseManager, DatabaseReadSession, DirectoryView, MediaDirectory, MediaFile,
        MediaFileQuery, MediaFileView,
    },
    state::AppState,
};
use anyhow::Result;
use axum::body::Bytes;
use std::collections::HashMap;
use std::fmt::Write as _;

fn write_xml_escaped<W: std::fmt::Write>(target: &mut W, value: &str) -> std::fmt::Result {
    let mut unescaped_start = 0;
    for (offset, character) in value.char_indices() {
        let replacement = match character {
            '&' => Some("&amp;"),
            '<' => Some("&lt;"),
            '>' => Some("&gt;"),
            '"' => Some("&quot;"),
            '\'' => Some("&apos;"),
            value if !is_valid_xml_character(value) => Some("\u{fffd}"),
            _ => None,
        };
        if let Some(replacement) = replacement {
            target.write_str(&value[unescaped_start..offset])?;
            target.write_str(replacement)?;
            unescaped_start = offset + character.len_utf8();
        }
    }
    target.write_str(&value[unescaped_start..])
}

fn is_valid_xml_character(character: char) -> bool {
    matches!(character, '\u{9}' | '\u{a}' | '\u{d}')
        || ('\u{20}'..='\u{d7ff}').contains(&character)
        || ('\u{e000}'..='\u{fffd}').contains(&character)
        || ('\u{10000}'..='\u{10ffff}').contains(&character)
}

struct XmlEscaped<'a>(&'a str);

impl std::fmt::Display for XmlEscaped<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_xml_escaped(formatter, self.0)
    }
}

fn xml_escape(value: &str) -> XmlEscaped<'_> {
    XmlEscaped(value)
}

/// Writes a complete DIDL document as the escaped text of SOAP's `<Result>`
/// directly into the final response buffer.
struct ByteBuffer(Vec<u8>);

impl ByteBuffer {
    fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    fn into_bytes(self) -> Bytes {
        Bytes::from(self.0)
    }
}

impl std::fmt::Write for ByteBuffer {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.0.extend_from_slice(value.as_bytes());
        Ok(())
    }
}

struct SoapResultWriter<'a, W: std::fmt::Write>(&'a mut W);

impl<W: std::fmt::Write> SoapResultWriter<'_, W> {
    fn push_str(&mut self, value: &str) {
        let _ = std::fmt::Write::write_str(self, value);
    }
}

impl<W: std::fmt::Write> std::fmt::Write for SoapResultWriter<'_, W> {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        write_xml_escaped(self.0, value)
    }
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

#[derive(Clone)]
pub struct BrowseRenderContext {
    pub client: crate::web::client::DlnaClientProfile,
    pub server_ip: String,
    pub server_port: u16,
    pub autoplay_enabled: bool,
    pub update_id: u32,
    pub bookmarks: HashMap<i64, u32>,
}

fn write_directory<W: std::fmt::Write, D: DirectoryView>(
    output: &mut W,
    object_id: &str,
    container: &D,
    client: crate::web::client::DlnaClientProfile,
) -> std::fmt::Result {
    let path = container.path();
    let container_id = if path.starts_with("audio/")
        || path.starts_with("video/")
        || path.starts_with("image/")
        || path.starts_with("radio/")
        || path == "audio"
        || path == "video"
        || path == "image"
        || path == "radio"
    {
        path.to_owned()
    } else if path.starts_with('d') && path[1..].chars().all(|c| c.is_ascii_digit()) {
        format!("{}/{}", object_id.trim_end_matches('/'), path)
    } else {
        format!("{}/{}", object_id.trim_end_matches('/'), container.name())
    };
    write!(
        output,
        r#"<container id="{}" parentID="{}" restricted="1"><dc:title>{}</dc:title><upnp:class>object.container</upnp:class>"#,
        xml_escape(&container_id),
        xml_escape(object_id),
        xml_escape(container.name())
    )?;
    if matches!(
        client,
        crate::web::client::DlnaClientProfile::SonyBdp
            | crate::web::client::DlnaClientProfile::SonyBravia
            | crate::web::client::DlnaClientProfile::PlayStation
    ) {
        let class = if container_id.contains("audio") || container_id.contains("music") {
            "A"
        } else if container_id.contains("image") || container_id.contains("picture") {
            "P"
        } else {
            "V"
        };
        write!(
            output,
            r#"<av:mediaClass xmlns:av="urn:schemas-sony-com:av">{class}</av:mediaClass>"#
        )?;
    }
    output.write_str("</container>")
}

fn write_media_view<W: std::fmt::Write, V: MediaFileView>(
    output: &mut W,
    object_id: &str,
    file: &V,
    context: &BrowseRenderContext,
) -> std::fmt::Result {
    let Some(file_id) = file.id().filter(|id| *id > 0) else {
        return Ok(());
    };
    let mime = file.mime_type();
    let is_radio = mime == "audio/radio";
    let has_srt = file.subtitle_available();
    let title = file.title().unwrap_or(file.filename());
    write!(
        output,
        r#"<item id="{}" parentID="{}" restricted="1"><dc:title>{}"#,
        file_id,
        xml_escape(object_id),
        xml_escape(title)
    )?;
    if context.client == crate::web::client::DlnaClientProfile::LgTv && has_srt {
        output.write_char('.')?;
    }
    output.write_str("</dc:title>")?;

    if mime.starts_with("audio/") {
        if let Some(value) = file.artist() {
            write!(output, "<upnp:artist>{}</upnp:artist>", xml_escape(value))?;
        }
        if let Some(value) = file.album() {
            write!(output, "<upnp:album>{}</upnp:album>", xml_escape(value))?;
        }
        if let Some(value) = file.genre() {
            write!(output, "<upnp:genre>{}</upnp:genre>", xml_escape(value))?;
        }
        if let Some(value) = file.track_number() {
            write!(
                output,
                "<upnp:originalTrackNumber>{value}</upnp:originalTrackNumber>"
            )?;
        }
        if let Some(value) = file.year() {
            write!(output, "<dc:date>{value}-01-01</dc:date>")?;
        }
        if let Some(value) = file.album_artist() {
            write!(
                output,
                "<upnp:albumArtist>{}</upnp:albumArtist>",
                xml_escape(value)
            )?;
        }
        write!(
            output,
            "<upnp:albumArtURI>http://{}:{}/media/{}/cover</upnp:albumArtURI>",
            context.server_ip, context.server_port, file_id
        )?;
    }
    write!(output, "<upnp:class>{}</upnp:class>", get_upnp_class(mime))?;

    let flags = if context.autoplay_enabled {
        "DLNA.ORG_OP=11;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000"
    } else {
        "DLNA.ORG_OP=11;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=00D00000000000000000000000000000"
    };
    let wire_mime = if is_radio {
        "audio/mpeg"
    } else {
        match context.client {
            crate::web::client::DlnaClientProfile::SamsungTv
            | crate::web::client::DlnaClientProfile::SamsungTvQ
                if mime == "video/x-matroska" =>
            {
                "video/x-mkv"
            }
            crate::web::client::DlnaClientProfile::SamsungTv
            | crate::web::client::DlnaClientProfile::SamsungTvQ
                if mime == "video/x-msvideo" =>
            {
                "video/mpeg"
            }
            crate::web::client::DlnaClientProfile::SonyBdp
                if mime == "video/x-matroska" || mime == "video/mpeg" =>
            {
                "video/divx"
            }
            crate::web::client::DlnaClientProfile::Xbox if mime == "video/x-msvideo" => "video/avi",
            _ => mime,
        }
    };
    write!(
        output,
        r#"<res protocolInfo="http-get:*:{wire_mime}:{flags}" size="{}""#,
        if is_radio { 0 } else { file.size() }
    )?;
    if !is_radio && (mime.starts_with("video/") || mime.starts_with("audio/")) {
        if let Some(seconds) = file.duration_secs().map(|value| value as u64) {
            write!(
                output,
                r#" duration="{:02}:{:02}:{:02}""#,
                seconds / 3600,
                (seconds % 3600) / 60,
                seconds % 60
            )?;
        }
    }
    if matches!(
        context.client,
        crate::web::client::DlnaClientProfile::LgTv
            | crate::web::client::DlnaClientProfile::PanasonicTv
    ) && has_srt
    {
        write!(
            output,
            r#" pv:subtitleFileUri="http://{}:{}/media/{}/subtitle" pv:subtitleFileType="SRT""#,
            context.server_ip, context.server_port, file_id
        )?;
    }
    write!(
        output,
        ">http://{}:{}/media/{}</res>",
        context.server_ip, context.server_port, file_id
    )?;
    if context.client == crate::web::client::DlnaClientProfile::LgTv && has_srt {
        write!(
            output,
            r#"<res protocolInfo="http-get:*:text/srt:*">http://{}:{}/media/{}/subtitle</res>"#,
            context.server_ip, context.server_port, file_id
        )?;
    }
    if matches!(
        context.client,
        crate::web::client::DlnaClientProfile::SamsungTv
            | crate::web::client::DlnaClientProfile::SamsungTvQ
    ) && has_srt
    {
        write!(
            output,
            r#"<sec:CaptionInfoEx sec:type="srt">http://{}:{}/media/{}/subtitle</sec:CaptionInfoEx>"#,
            context.server_ip, context.server_port, file_id
        )?;
    }
    if matches!(
        context.client,
        crate::web::client::DlnaClientProfile::SamsungTv
            | crate::web::client::DlnaClientProfile::SamsungTvQ
    ) {
        let mut bookmark = context.bookmarks.get(&file_id).copied().unwrap_or(0);
        if context.client == crate::web::client::DlnaClientProfile::SamsungTvQ {
            bookmark = bookmark.saturating_mul(1000);
        }
        write!(
            output,
            "<sec:dcmInfo>CREATIONDATE=0,FOLDER={},BM={bookmark}</sec:dcmInfo>",
            xml_escape(file.filename())
        )?;
    }
    output.write_str("</item>")
}

pub fn generate_indexed_browse_response<S: DatabaseReadSession>(
    session: &mut S,
    canonical_parent: &str,
    mime_family: &str,
    object_id: &str,
    starting_index: usize,
    requested_count: usize,
    context: BrowseRenderContext,
) -> Result<Bytes> {
    let directory_count = session
        .visit_direct_subdirectories(
            canonical_parent,
            (!mime_family.is_empty()).then_some(mime_family),
            0,
            0,
            |_| Ok(()),
        )?
        .matched;
    let directory_limit = requested_count.min(directory_count.saturating_sub(starting_index));
    let file_offset = starting_index.saturating_sub(directory_count);
    let file_limit = requested_count.saturating_sub(directory_limit);

    let mut response = ByteBuffer::with_capacity(750 + requested_count.saturating_mul(500));
    response.write_str(r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body><u:BrowseResponse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1"><Result>"#)?;
    let mut result = SoapResultWriter(&mut response);
    result.push_str(r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" xmlns:pv="http://www.pv.com/pvplay/" xmlns:sec="http://www.sec.co.kr/">"#);
    let directory_summary = session.visit_direct_subdirectories(
        canonical_parent,
        (!mime_family.is_empty()).then_some(mime_family),
        starting_index,
        directory_limit,
        |directory| {
            write_directory(&mut result, object_id, &directory, context.client)
                .map_err(|_| anyhow::anyhow!("failed to construct directory XML"))
        },
    )?;
    let query = MediaFileQuery::Directory {
        path: canonical_parent.to_owned(),
        mime_family: (!mime_family.is_empty()).then(|| mime_family.to_owned()),
    };
    let summary = session.visit_files(&query, file_offset, file_limit, |file| {
        write_media_view(&mut result, object_id, &file, &context)
            .map_err(|_| anyhow::anyhow!("failed to construct browse XML"))
    })?;
    result.push_str("</DIDL-Lite>");
    let returned = directory_summary.visited + summary.visited;
    let total = directory_count + summary.matched;
    write!(&mut response, "</Result><NumberReturned>{returned}</NumberReturned><TotalMatches>{total}</TotalMatches><UpdateID>{}</UpdateID></u:BrowseResponse></s:Body></s:Envelope>", context.update_id)?;
    Ok(response.into_bytes())
}

pub fn generate_indexed_items_response<S: DatabaseReadSession>(
    session: &mut S,
    query: MediaFileQuery,
    object_id: &str,
    starting_index: usize,
    requested_count: usize,
    context: BrowseRenderContext,
) -> Result<Bytes> {
    let mut response = ByteBuffer::with_capacity(750 + requested_count.saturating_mul(500));
    response.write_str(r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body><u:BrowseResponse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1"><Result>"#)?;
    let mut result = SoapResultWriter(&mut response);
    result.push_str(r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" xmlns:pv="http://www.pv.com/pvplay/" xmlns:sec="http://www.sec.co.kr/">"#);
    let summary = session.visit_files(&query, starting_index, requested_count, |file| {
        write_media_view(&mut result, object_id, &file, &context)
            .map_err(|_| anyhow::anyhow!("failed to construct browse XML"))
    })?;
    result.push_str("</DIDL-Lite>");
    write!(&mut response, "</Result><NumberReturned>{}</NumberReturned><TotalMatches>{}</TotalMatches><UpdateID>{}</UpdateID></u:BrowseResponse></s:Body></s:Envelope>", summary.visited, summary.matched, context.update_id)?;
    Ok(response.into_bytes())
}

pub async fn generate_description_xml<D: DatabaseManager>(state: &AppState<D>) -> String {
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
                r#"<container id="{}" parentID="{}" restricted="1"><dc:title>{}</dc:title><upnp:class>object.container</upnp:class>"#,
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
            let mut title = file.title.clone().unwrap_or_else(|| file.filename.clone());
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
                    server_ip, state.config.server.port, file_id
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
            let dlna_flags = if state.config.media.autoplay_enabled {
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
                    server_ip, state.config.server.port, file_id
                );
            }

            let _ = write!(
                &mut didl,
                r#">http://{}:{}/media/{}</res>"#,
                server_ip, state.config.server.port, file_id
            );

            if client == crate::web::client::DlnaClientProfile::LgTv && has_srt {
                let _ = write!(
                    &mut didl,
                    r#"
                <res protocolInfo="http-get:*:text/srt:*">http://{}:{}/media/{}/subtitle</res>"#,
                    server_ip, state.config.server.port, file_id
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
                    server_ip, state.config.server.port, file_id
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_escape_handles_markup_unicode_and_invalid_controls() {
        let value = "A&B <tag> \"quoted\" 'single' café\u{1}";
        let escaped = xml_escape(value).to_string();
        assert_eq!(
            escaped,
            "A&amp;B &lt;tag&gt; &quot;quoted&quot; &apos;single&apos; café�"
        );
    }

    #[test]
    fn soap_result_writer_applies_the_required_second_escape_layer() {
        let mut output = String::new();
        write!(&mut SoapResultWriter(&mut output), "{}", xml_escape("A&B"))
            .expect("write nested XML");
        assert_eq!(output, "A&amp;amp;B");
    }
}

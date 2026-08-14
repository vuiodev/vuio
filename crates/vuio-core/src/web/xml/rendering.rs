use super::*;
use std::fmt::Write as _;

pub(super) fn write_xml_escaped<W: std::fmt::Write>(
    target: &mut W,
    value: &str,
) -> std::fmt::Result {
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

pub(super) fn is_valid_xml_character(character: char) -> bool {
    matches!(character, '\u{9}' | '\u{a}' | '\u{d}')
        || ('\u{20}'..='\u{d7ff}').contains(&character)
        || ('\u{e000}'..='\u{fffd}').contains(&character)
        || ('\u{10000}'..='\u{10ffff}').contains(&character)
}

pub(super) struct XmlEscaped<'a>(&'a str);

impl std::fmt::Display for XmlEscaped<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_xml_escaped(formatter, self.0)
    }
}

pub(super) fn xml_escape(value: &str) -> XmlEscaped<'_> {
    XmlEscaped(value)
}

/// Writes a complete DIDL document as the escaped text of SOAP's `<Result>`
/// directly into the final response buffer.
pub(super) struct ByteBuffer(Vec<u8>);

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

pub(super) struct SoapResultWriter<'a, W: std::fmt::Write>(pub(super) &'a mut W);

impl<W: std::fmt::Write> SoapResultWriter<'_, W> {
    pub(super) fn push_str(&mut self, value: &str) {
        let _ = std::fmt::Write::write_str(self, value);
    }
}

impl<W: std::fmt::Write> std::fmt::Write for SoapResultWriter<'_, W> {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        write_xml_escaped(self.0, value)
    }
}

/// Get the appropriate UPnP class for a given MIME type.
pub(super) fn get_upnp_class(mime_type: &str) -> &str {
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
pub(super) fn format_duration(duration_seconds: u64) -> String {
    let hours = duration_seconds / 3600;
    let minutes = (duration_seconds % 3600) / 60;
    let seconds = duration_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

/// Samsung TVs append a MIME-derived extension to `dc:title`. If the title already
/// ends with the file's extension (common when falling back to `filename`), strip it.
pub(super) fn strip_matching_media_extension(base: &str, filename: &str) -> String {
    let Some(ext) = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty())
    else {
        return base.to_string();
    };
    let suffix_len = ext.len() + 1; // '.' + ext
    if base.len() > suffix_len {
        let maybe = &base[base.len() - suffix_len..];
        if maybe.as_bytes()[0] == b'.' && maybe[1..].eq_ignore_ascii_case(ext) {
            return base[..base.len() - suffix_len].to_string();
        }
    }
    base.to_string()
}

pub(super) fn didl_display_title(
    title: Option<&str>,
    filename: &str,
    client: crate::web::client::DlnaClientProfile,
) -> String {
    let base = title.unwrap_or(filename);
    match client {
        crate::web::client::DlnaClientProfile::SamsungTv
        | crate::web::client::DlnaClientProfile::SamsungTvQ => {
            strip_matching_media_extension(base, filename)
        }
        _ => base.to_string(),
    }
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

/// UPnP container classes.
///
/// Control points key display off these: a `musicAlbum` gets album art and
/// track listing, a `musicArtist` gets grouped under Artists, a
/// `playlistContainer` offers "play all". Announcing everything as a
/// `storageFolder` is what makes a categorized library look like a file tree.
pub mod container_class {
    pub const STORAGE_FOLDER: &str = "object.container.storageFolder";
    pub const MUSIC_ARTIST: &str = "object.container.person.musicArtist";
    pub const MUSIC_ALBUM: &str = "object.container.album.musicAlbum";
    pub const MUSIC_GENRE: &str = "object.container.genre.musicGenre";
    pub const PLAYLIST: &str = "object.container.playlistContainer";
}

/// One container as it will be rendered into a DIDL document.
#[derive(Clone, Debug)]
pub struct ContainerSpec {
    pub id: String,
    pub title: String,
    pub class: &'static str,
    pub child_count: usize,
    /// A record whose cover art represents this container, if it has one.
    pub album_art_id: Option<i64>,
}

impl ContainerSpec {
    pub fn folder(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            class: container_class::STORAGE_FOLDER,
            child_count: 1,
            album_art_id: None,
        }
    }

    pub fn with_class(mut self, class: &'static str) -> Self {
        self.class = class;
        self
    }

    pub fn with_child_count(mut self, child_count: usize) -> Self {
        self.child_count = child_count;
        self
    }

    pub fn with_album_art(mut self, album_art_id: Option<i64>) -> Self {
        self.album_art_id = album_art_id;
        self
    }
}

/// The one place a `<container>` element is written.
pub(super) fn write_container<W: std::fmt::Write>(
    output: &mut W,
    spec: &ContainerSpec,
    parent_id: &str,
    client: crate::web::client::DlnaClientProfile,
    server_ip: &str,
    server_port: u16,
) -> std::fmt::Result {
    write!(
        output,
        r#"<container id="{}" parentID="{}" restricted="1" searchable="0" childCount="{}"><dc:title>{}</dc:title><upnp:class>{}</upnp:class>"#,
        xml_escape(&spec.id),
        xml_escape(parent_id),
        spec.child_count,
        xml_escape(&spec.title),
        spec.class
    )?;
    if let Some(art_id) = spec.album_art_id {
        write!(
            output,
            "<upnp:albumArtURI>http://{server_ip}:{server_port}/media/{art_id}/cover</upnp:albumArtURI>"
        )?;
    }
    if matches!(
        client,
        crate::web::client::DlnaClientProfile::SonyBdp
            | crate::web::client::DlnaClientProfile::SonyBravia
            | crate::web::client::DlnaClientProfile::PlayStation
    ) {
        let class = if spec.id.contains("audio") || spec.id.contains("music") {
            "A"
        } else if spec.id.contains("image") || spec.id.contains("picture") {
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

/// Derive the object id a filesystem-backed subdirectory browses under.
pub(super) fn directory_container_id(object_id: &str, path: &str, name: &str) -> String {
    if path.starts_with("audio/")
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
        format!("{}/{}", object_id.trim_end_matches('/'), name)
    }
}

pub(super) fn write_directory<W: std::fmt::Write, D: DirectoryView>(
    output: &mut W,
    object_id: &str,
    container: &D,
    context: &BrowseRenderContext,
) -> std::fmt::Result {
    let spec = ContainerSpec::folder(
        directory_container_id(object_id, container.path(), container.name()),
        container.name(),
    );
    write_container(
        output,
        &spec,
        object_id,
        context.client,
        &context.server_ip,
        context.server_port,
    )
}

pub(super) fn write_media_view<W: std::fmt::Write, V: MediaFileView>(
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
    let title = didl_display_title(file.title(), file.filename(), context.client);
    write!(
        output,
        r#"<item id="{}" parentID="{}" restricted="1"><dc:title>{}"#,
        file_id,
        xml_escape(object_id),
        xml_escape(&title)
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
        if let Some(value) = file.composer() {
            write!(
                output,
                r#"<upnp:author role="Composer">{}</upnp:author>"#,
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
    if !is_radio {
        // Renderers use these to decide whether they can play a track before
        // fetching a byte of it. Note that DLNA's `res@bitrate` is *bytes* per
        // second, not bits, which is the usual thing to get wrong.
        if let Some(bits_per_second) = file.bit_rate().filter(|rate| *rate > 0) {
            write!(output, r#" bitrate="{}""#, bits_per_second / 8)?;
        }
        if let Some(sample_rate) = file.sample_rate().filter(|rate| *rate > 0) {
            write!(output, r#" sampleFrequency="{sample_rate}""#)?;
        }
        if let Some(channels) = file.channels().filter(|count| *count > 0) {
            write!(output, r#" nrAudioChannels="{channels}""#)?;
        }
        if let Some(bits) = file.bits_per_sample().filter(|bits| *bits > 0) {
            write!(output, r#" bitsPerSample="{bits}""#)?;
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
            write_directory(&mut result, object_id, &directory, &context)
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

use axum::{
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub(super) struct BrowseParams {
    pub(super) object_id: String,
    pub(super) starting_index: u32,
    pub(super) requested_count: u32,
}

const MAX_BROWSE_ITEMS_PER_RESPONSE: usize = 2_000;

pub(super) fn browse_page_limit(params: &BrowseParams) -> usize {
    if params.requested_count == 0 {
        MAX_BROWSE_ITEMS_PER_RESPONSE
    } else {
        (params.requested_count as usize).min(MAX_BROWSE_ITEMS_PER_RESPONSE)
    }
}

pub(super) fn browse_page_bounds(
    params: &BrowseParams,
    total_matches: usize,
) -> std::ops::Range<usize> {
    let start = (params.starting_index as usize).min(total_matches);
    let end = start
        .saturating_add(browse_page_limit(params))
        .min(total_matches);
    start..end
}

pub(super) fn soap_action(headers: &HeaderMap, body: &str) -> Result<String, Box<Response>> {
    let header_action = headers
        .get("soapaction")
        .and_then(|value| value.to_str().ok())
        .and_then(parse_soap_action_header);
    let body_action = match body_soap_action(body) {
        Ok(action) => action,
        Err(message) => return Err(Box::new(invalid_soap_request(message))),
    };

    if let Some(header_action) = header_action {
        if header_action != body_action {
            return Err(Box::new(invalid_soap_request(
                "SOAPAction header does not match the SOAP body",
            )));
        }
        Ok(header_action)
    } else {
        Ok(body_action)
    }
}

fn parse_soap_action_header(value: &str) -> Option<String> {
    let value = value.trim().trim_matches('"');
    let action = value
        .rsplit_once('#')
        .map(|(_, action)| action)
        .unwrap_or(value)
        .trim();
    (!action.is_empty()).then(|| action.to_string())
}

fn body_soap_action(body: &str) -> Result<String, &'static str> {
    use quick_xml::{events::Event, Reader};

    let mut reader = Reader::from_str(body);
    let mut buffer = Vec::new();
    let mut in_body = false;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) => {
                let qualified_name = element.name();
                let name = local_xml_name(qualified_name.as_ref());
                if in_body {
                    return Ok(name.to_string());
                }
                if name == "Body" {
                    in_body = true;
                }
            }
            Ok(Event::Empty(element)) if in_body => {
                return Ok(local_xml_name(element.name().as_ref()).to_string());
            }
            Ok(Event::End(element)) if local_xml_name(element.name().as_ref()) == "Body" => break,
            Ok(Event::Eof) => break,
            Err(_) => return Err("Malformed SOAP XML"),
            _ => {}
        }
        buffer.clear();
    }
    Err("SOAP body has no action element")
}

pub(super) fn xml_element_text(body: &str, expected_name: &str) -> Option<String> {
    use quick_xml::{events::Event, Reader};

    let mut reader = Reader::from_str(body);
    let mut buffer = Vec::new();
    let mut capture = false;
    loop {
        match reader.read_event_into(&mut buffer).ok()? {
            Event::Start(element) => {
                capture = local_xml_name(element.name().as_ref()) == expected_name;
            }
            Event::Text(text) if capture => {
                return reader
                    .decoder()
                    .decode(text.as_ref())
                    .ok()
                    .map(|value| value.into_owned());
            }
            Event::End(_) => capture = false,
            Event::Eof => return None,
            _ => {}
        }
        buffer.clear();
    }
}

fn local_xml_name(name: &[u8]) -> &str {
    let local = name
        .iter()
        .rposition(|byte| *byte == b':')
        .map(|position| &name[position + 1..])
        .unwrap_or(name);
    std::str::from_utf8(local).unwrap_or_default()
}

fn invalid_soap_request(message: &'static str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        message,
    )
        .into_response()
}

pub(super) fn parse_browse_params(body: &str) -> BrowseParams {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);
    let mut object_id = "0".to_string();
    let mut starting_index = 0_u32;
    let mut requested_count = 0_u32;
    let mut buffer = Vec::new();
    let mut current_element = String::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(ref element)) | Ok(Event::Empty(ref element)) => {
                current_element = String::from_utf8_lossy(element.name().as_ref()).to_string();
            }
            Ok(Event::Text(ref text)) => {
                let text = reader.decoder().decode(text.as_ref()).unwrap_or_default();
                match current_element.as_str() {
                    "ObjectID" => {
                        object_id = text.trim().to_string();
                        if object_id.is_empty() {
                            object_id = "0".to_string();
                        }
                    }
                    "StartingIndex" => {
                        starting_index = text.trim().parse().unwrap_or_else(|error| {
                            warn!("Failed to parse StartingIndex '{}': {}", text, error);
                            0
                        });
                    }
                    "RequestedCount" => {
                        requested_count = text.trim().parse().unwrap_or_else(|error| {
                            warn!("Failed to parse RequestedCount '{}': {}", text, error);
                            0
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                warn!("Error parsing XML: {}, falling back to defaults", error);
                break;
            }
            _ => {}
        }
        buffer.clear();
    }

    debug!(
        "Parsed browse params - ObjectID: '{}', StartingIndex: {}, RequestedCount: {}",
        object_id, starting_index, requested_count
    );
    BrowseParams {
        object_id,
        starting_index,
        requested_count,
    }
}

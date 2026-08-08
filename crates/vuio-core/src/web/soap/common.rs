use super::*;

pub(super) fn parse_dir_index_prefix(path_prefix_str: &str) -> (Option<usize>, &str) {
    if path_prefix_str.starts_with('d') {
        let chars = path_prefix_str.chars().skip(1);
        let mut num_str = String::new();
        for c in chars {
            if c.is_ascii_digit() {
                num_str.push(c);
            } else {
                break;
            }
        }
        if !num_str.is_empty() {
            if let Ok(idx) = num_str.parse::<usize>() {
                let prefix_len = 1 + num_str.len();
                let rem = if path_prefix_str.len() > prefix_len {
                    path_prefix_str[prefix_len..].trim_start_matches('/')
                } else {
                    ""
                };
                (Some(idx), rem)
            } else {
                (None, path_prefix_str)
            }
        } else {
            (None, path_prefix_str)
        }
    } else {
        (None, path_prefix_str)
    }
}

pub(super) fn build_soap_response(action: &str, service_type: &str, content: &str) -> Response {
    let mut xml =
        String::with_capacity(300 + action.len() * 2 + service_type.len() + content.len());
    xml.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    xml.push_str("<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\n");
    xml.push_str("    <s:Body>\n");
    xml.push_str("        <u:");
    xml.push_str(action);
    xml.push_str("Response xmlns:u=\"");
    xml.push_str(service_type);
    xml.push_str("\">\n");
    xml.push_str("            ");
    xml.push_str(content);
    xml.push('\n');
    xml.push_str("        </u:");
    xml.push_str(action);
    xml.push_str("Response>\n");
    xml.push_str("    </s:Body>\n");
    xml.push_str("</s:Envelope>");

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
            (header::HeaderName::from_static("ext"), ""),
        ],
        xml,
    )
        .into_response()
}

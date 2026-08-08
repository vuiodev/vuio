use super::*;

#[test]
fn soap_action_ignores_action_names_in_comments() {
    let headers = HeaderMap::new();
    let body = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><!-- <u:Browse/> --><u:GetSystemUpdateID xmlns:u="urn:test"/></s:Body></s:Envelope>"#;
    assert_eq!(soap_action(&headers, body).unwrap(), "GetSystemUpdateID");
}

#[test]
fn soap_action_rejects_header_body_mismatch() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "soapaction",
        "\"urn:schemas-upnp-org:service:ContentDirectory:1#Browse\""
            .parse()
            .unwrap(),
    );
    let body = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><u:GetSystemUpdateID xmlns:u="urn:test"/></s:Body></s:Envelope>"#;
    assert!(soap_action(&headers, body).is_err());
}

#[test]
fn test_parse_browse_params_valid_xml() {
    let xml_body = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
    <s:Body>
        <u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
            <ObjectID>video/movies</ObjectID>
            <BrowseFlag>BrowseDirectChildren</BrowseFlag>
            <Filter>*</Filter>
            <StartingIndex>10</StartingIndex>
            <RequestedCount>25</RequestedCount>
            <SortCriteria></SortCriteria>
        </u:Browse>
    </s:Body>
</s:Envelope>"#;

    let params = parse_browse_params(xml_body);
    assert_eq!(params.object_id, "video/movies");
    assert_eq!(params.starting_index, 10);
    assert_eq!(params.requested_count, 25);
}

#[test]
fn test_parse_browse_params_minimal_xml() {
    let xml_body = r#"<ObjectID>0</ObjectID><StartingIndex>0</StartingIndex><RequestedCount>0</RequestedCount>"#;

    let params = parse_browse_params(xml_body);
    assert_eq!(params.object_id, "0");
    assert_eq!(params.starting_index, 0);
    assert_eq!(params.requested_count, 0);
}

#[test]
fn test_parse_browse_params_missing_elements() {
    let xml_body = r#"<ObjectID>audio/artists</ObjectID>"#;

    let params = parse_browse_params(xml_body);
    assert_eq!(params.object_id, "audio/artists");
    assert_eq!(params.starting_index, 0); // Default value
    assert_eq!(params.requested_count, 0); // Default value
}

#[test]
fn test_parse_browse_params_invalid_numbers() {
    let xml_body = r#"<ObjectID>test</ObjectID><StartingIndex>invalid</StartingIndex><RequestedCount>not_a_number</RequestedCount>"#;

    let params = parse_browse_params(xml_body);
    assert_eq!(params.object_id, "test");
    assert_eq!(params.starting_index, 0); // Falls back to default
    assert_eq!(params.requested_count, 0); // Falls back to default
}

#[test]
fn test_parse_browse_params_empty_xml() {
    let xml_body = "";

    let params = parse_browse_params(xml_body);
    assert_eq!(params.object_id, "0"); // Default value
    assert_eq!(params.starting_index, 0); // Default value
    assert_eq!(params.requested_count, 0); // Default value
}

#[test]
fn test_parse_browse_params_malformed_xml() {
    let xml_body =
        r#"<ObjectID>test</ObjectID><StartingIndex>5<RequestedCount>10</RequestedCount>"#;

    let params = parse_browse_params(xml_body);
    // Should handle malformed XML gracefully and extract what it can
    assert_eq!(params.object_id, "test");
    // The parser should still work despite the malformed StartingIndex tag
}

#[test]
fn test_parse_browse_params_with_whitespace() {
    let xml_body = r#"
        <ObjectID>  video/series  </ObjectID>
        <StartingIndex>  5  </StartingIndex>
        <RequestedCount>  15  </RequestedCount>
        "#;

    let params = parse_browse_params(xml_body);
    assert_eq!(params.object_id, "video/series"); // Should be trimmed
    assert_eq!(params.starting_index, 5);
    assert_eq!(params.requested_count, 15);
}

#[test]
fn test_parse_browse_params_performance_comparison() {
    // This test demonstrates that the new XML parser handles complex XML correctly
    // while the old string-based approach would be fragile
    let complex_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
            s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
    <s:Body>
        <u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
            <ObjectID>video/movies/action</ObjectID>
            <BrowseFlag>BrowseDirectChildren</BrowseFlag>
            <Filter>dc:title,dc:date,upnp:class,res@duration,res@size</Filter>
            <StartingIndex>100</StartingIndex>
            <RequestedCount>50</RequestedCount>
            <SortCriteria>+dc:title</SortCriteria>
        </u:Browse>
    </s:Body>
</s:Envelope>"#;

    let params = parse_browse_params(complex_xml);
    assert_eq!(params.object_id, "video/movies/action");
    assert_eq!(params.starting_index, 100);
    assert_eq!(params.requested_count, 50);
}

#[test]
fn test_parse_dir_index_prefix() {
    assert_eq!(parse_dir_index_prefix("d0"), (Some(0), ""));
    assert_eq!(parse_dir_index_prefix("d0/movies"), (Some(0), "movies"));
    assert_eq!(
        parse_dir_index_prefix("d12/movies/action"),
        (Some(12), "movies/action")
    );
    assert_eq!(parse_dir_index_prefix("d0/"), (Some(0), ""));
    assert_eq!(parse_dir_index_prefix("movies"), (None, "movies"));
    assert_eq!(parse_dir_index_prefix("d"), (None, "d"));
    assert_eq!(parse_dir_index_prefix("dx"), (None, "dx"));
    assert_eq!(parse_dir_index_prefix(""), (None, ""));
}

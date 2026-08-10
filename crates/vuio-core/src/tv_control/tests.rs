use super::*;

#[test]
fn renderer_urls_are_peer_bound_and_numeric() {
    let peer: IpAddr = "192.168.1.100".parse().unwrap();
    assert!(validate_renderer_url("http://192.168.1.100:8080/desc.xml", Some(peer)).is_ok());
    assert!(validate_renderer_url("http://192.168.1.101/desc.xml", Some(peer)).is_err());
    assert!(validate_renderer_url("http://localhost/desc.xml", None).is_err());
    assert!(validate_renderer_url("http://169.254.169.254/", None).is_err());
    assert!(validate_renderer_url("https://192.168.1.100/", None).is_err());
}

#[test]
fn transport_xml_escapes_nested_metadata_once() {
    let xml = build_transport_uri_soap(
        "SetAVTransportURI",
        "http://192.168.1.2/a?x=1&y=2",
        "A & <B>",
        "video/mp4",
    )
    .unwrap();
    assert!(xml.contains("A &amp;amp; &amp;lt;B&amp;gt;"));
    assert!(!xml.contains("<dc:title>A &"));
}

#[test]
fn transport_xml_can_omit_metadata_for_strict_renderers() {
    let xml = build_transport_uri_soap_with_metadata(
        "SetAVTransportURI",
        "http://192.168.1.2/media/7",
        "Episode 7",
        "video/x-matroska",
        false,
    )
    .unwrap();
    assert!(xml.contains("<CurrentURI>http://192.168.1.2/media/7</CurrentURI>"));
    assert!(xml.contains("<CurrentURIMetaData></CurrentURIMetaData>"));
    assert!(!xml.contains("DIDL-Lite"));
}

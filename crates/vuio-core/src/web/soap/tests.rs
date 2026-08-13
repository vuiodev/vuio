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

/// An object id must survive the round trip through a control point.
fn round_trip(node: MusicNode) {
    let id = node.object_id();
    let audio_path = id.strip_prefix("audio").unwrap().trim_start_matches('/');
    assert_eq!(
        parse_music_path(audio_path),
        Some(node.clone()),
        "id {id} did not parse back to {node:?}"
    );
}

#[test]
fn music_ids_round_trip_through_every_node_shape() {
    round_trip(MusicNode::Root);
    round_trip(MusicNode::AllMusic);

    round_trip(MusicNode::ArtistList);
    round_trip(MusicNode::Artist("Metallica".into()));
    round_trip(MusicNode::ArtistAll("Metallica".into()));
    round_trip(MusicNode::ArtistAlbum(
        "Metallica".into(),
        "Ride the Lightning".into(),
    ));

    round_trip(MusicNode::AlbumArtistList);
    round_trip(MusicNode::AlbumArtist("Various Artists".into()));
    round_trip(MusicNode::AlbumArtistAll("Various Artists".into()));
    round_trip(MusicNode::AlbumArtistAlbum(
        "Various Artists".into(),
        "Now 42".into(),
    ));

    round_trip(MusicNode::AlbumList);
    round_trip(MusicNode::Album("Ride the Lightning".into()));

    round_trip(MusicNode::GenreList);
    round_trip(MusicNode::Genre("Metal".into()));
    round_trip(MusicNode::GenreAll("Metal".into()));
    round_trip(MusicNode::GenreArtist("Metal".into(), "Metallica".into()));
    round_trip(MusicNode::GenreArtistAll("Metal".into(), "Metallica".into()));
    round_trip(MusicNode::GenreArtistAlbum(
        "Metal".into(),
        "Metallica".into(),
        "Ride the Lightning".into(),
    ));

    round_trip(MusicNode::YearList);
    round_trip(MusicNode::Year(1984));

    round_trip(MusicNode::PlaylistList);
    round_trip(MusicNode::Playlist(7));

    round_trip(MusicNode::Folders(String::new()));
    round_trip(MusicNode::Folders("Rock/Live".into()));
}

/// A slash in a tag value is why the segments are encoded at all: without it
/// the artist "AC/DC" is indistinguishable from an artist "AC" holding an
/// album "DC".
#[test]
fn a_slash_in_a_tag_value_stays_inside_its_segment() {
    let node = MusicNode::Artist("AC/DC".into());
    assert_eq!(node.object_id(), "audio/artists/AC%2FDC");
    round_trip(node);

    round_trip(MusicNode::ArtistAlbum(
        "AC/DC".into(),
        "Back in Black".into(),
    ));
    round_trip(MusicNode::GenreArtistAlbum(
        "Rock/Metal".into(),
        "AC/DC".into(),
        "Who Made Who".into(),
    ));
}

/// Percent signs must not be read as the escapes the decoder introduces.
#[test]
fn a_percent_in_a_tag_value_is_not_read_as_an_escape() {
    let node = MusicNode::Album("50% More".into());
    assert_eq!(node.object_id(), "audio/albums/50%25 More");
    round_trip(node);

    // The classic failure: a literal "%2F" in a name decoding to a slash.
    round_trip(MusicNode::Artist("%2F".into()));
}

/// The reserved segment cannot be forged by a tag value, because the encoder
/// escapes the character that introduces it.
#[test]
fn an_album_named_like_the_reserved_segment_still_resolves() {
    let album = MusicNode::ArtistAlbum("Metallica".into(), "!all".into());
    assert_eq!(album.object_id(), "audio/artists/Metallica/%21all");
    round_trip(album);

    // And the reserved segment itself still means All Songs.
    assert_eq!(
        parse_music_path("artists/Metallica/!all"),
        Some(MusicNode::ArtistAll("Metallica".into()))
    );
}

#[test]
fn music_nodes_report_their_parent_and_class() {
    use crate::web::xml::container_class;

    let album = MusicNode::ArtistAlbum("Metallica".into(), "Ride the Lightning".into());
    assert_eq!(album.parent_id(), "audio/artists/Metallica");
    assert_eq!(album.class(), container_class::MUSIC_ALBUM);

    assert_eq!(
        MusicNode::Artist("Metallica".into()).class(),
        container_class::MUSIC_ARTIST
    );
    assert_eq!(
        MusicNode::Genre("Metal".into()).class(),
        container_class::MUSIC_GENRE
    );
    assert_eq!(
        MusicNode::Playlist(1).class(),
        container_class::PLAYLIST
    );
    // A genre's artist is still an artist container.
    assert_eq!(
        MusicNode::GenreArtist("Metal".into(), "Metallica".into()).class(),
        container_class::MUSIC_ARTIST
    );
    // The category lists themselves are plain folders.
    assert_eq!(
        MusicNode::ArtistList.class(),
        container_class::STORAGE_FOLDER
    );
    assert_eq!(MusicNode::ArtistList.parent_id(), "audio");
    assert_eq!(MusicNode::Root.parent_id(), "0");
}

/// Only the leaves list tracks; every other node lists containers.
#[test]
fn track_listings_are_the_leaves_of_the_tree() {
    assert!(MusicNode::AllMusic.track_query().is_some());
    assert!(MusicNode::ArtistAll("A".into()).track_query().is_some());
    assert!(MusicNode::ArtistAlbum("A".into(), "B".into())
        .track_query()
        .is_some());
    assert!(MusicNode::Album("B".into()).track_query().is_some());
    assert!(MusicNode::Year(1984).track_query().is_some());
    assert!(MusicNode::Playlist(1).track_query().is_some());

    assert!(MusicNode::Root.track_query().is_none());
    assert!(MusicNode::ArtistList.track_query().is_none());
    assert!(MusicNode::Artist("A".into()).track_query().is_none());
    assert!(MusicNode::Genre("G".into()).track_query().is_none());
    assert!(MusicNode::GenreArtist("G".into(), "A".into())
        .track_query()
        .is_none());
}

/// An album under an artist must be constrained by that artist, or two records
/// sharing a title merge into one listing.
#[test]
fn an_album_reached_through_an_artist_is_scoped_to_it() {
    use crate::database::MediaFileQuery;

    let query = MusicNode::ArtistAlbum("Metallica".into(), "Greatest Hits".into())
        .track_query()
        .unwrap();
    match query {
        MediaFileQuery::Music {
            artist,
            album,
            exclude_radio,
            ..
        } => {
            assert_eq!(artist.as_deref(), Some("Metallica"));
            assert_eq!(album.as_deref(), Some("Greatest Hits"));
            assert!(exclude_radio, "radio streams are not music library tracks");
        }
        other => panic!("unexpected query: {other:?}"),
    }

    let query = MusicNode::GenreArtistAlbum("Metal".into(), "Metallica".into(), "X".into())
        .track_query()
        .unwrap();
    match query {
        MediaFileQuery::Music {
            genre,
            artist,
            album,
            ..
        } => {
            assert_eq!(genre.as_deref(), Some("Metal"));
            assert_eq!(artist.as_deref(), Some("Metallica"));
            assert_eq!(album.as_deref(), Some("X"));
        }
        other => panic!("unexpected query: {other:?}"),
    }
}

/// An id that names no node is browsed as a folder, so object ids minted by
/// older versions keep working.
#[test]
fn an_unrecognized_music_path_is_not_a_node() {
    assert_eq!(parse_music_path("artists/A/B/C/D"), None);
    assert_eq!(parse_music_path("nonsense/deep/path"), None);
    assert_eq!(parse_music_path("years/not-a-year"), None);
    assert_eq!(parse_music_path("playlists/not-a-number"), None);
    // "!all" is a structural segment, never a tag value.
    assert_eq!(parse_music_path("artists/!all"), None);
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

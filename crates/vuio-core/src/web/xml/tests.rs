use super::rendering::*;
use crate::web::client::DlnaClientProfile;
use std::fmt::Write as _;

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
fn xml_escape_accepts_every_unicode_scalar_and_emits_valid_xml_characters() {
    let all_scalars: String = (0..=char::MAX as u32).filter_map(char::from_u32).collect();
    let escaped = xml_escape(&all_scalars).to_string();

    assert!(escaped.chars().all(is_valid_xml_character));
}

#[test]
fn soap_result_writer_applies_the_required_second_escape_layer() {
    let mut output = String::new();
    write!(&mut SoapResultWriter(&mut output), "{}", xml_escape("A&B")).expect("write nested XML");
    assert_eq!(output, "A&amp;amp;B");
}

#[test]
fn samsung_strips_matching_extension_from_filename_fallback() {
    assert_eq!(
        didl_display_title(None, "movie.mp4", DlnaClientProfile::SamsungTv),
        "movie"
    );
    assert_eq!(
        didl_display_title(None, "movie.MP4", DlnaClientProfile::SamsungTvQ),
        "movie"
    );
    assert_eq!(
        didl_display_title(Some("clip.mp4"), "clip.mp4", DlnaClientProfile::SamsungTv),
        "clip"
    );
}

#[test]
fn samsung_keeps_titles_without_matching_extension() {
    assert_eq!(
        didl_display_title(Some("My Film"), "movie.mp4", DlnaClientProfile::SamsungTv),
        "My Film"
    );
}

#[test]
fn samsung_handles_unicode_near_the_extension_boundary() {
    let title = "09 - Symphony No. 8 in E-Flat Major, Pt. 2 V. Wie Felsenabgrund mir zu Füßen";
    let filename = format!("{title}.flac");

    assert_eq!(
        didl_display_title(Some(title), &filename, DlnaClientProfile::SamsungTv),
        title
    );
    assert_eq!(
        didl_display_title(None, &filename, DlnaClientProfile::SamsungTv),
        title
    );
}

#[test]
fn samsung_extension_handling_accepts_every_unicode_scalar() {
    for code_point in 0..=char::MAX as u32 {
        let Some(character) = char::from_u32(code_point) else {
            continue;
        };

        // With a one-byte extension, the old byte-offset implementation tried to
        // start its suffix one byte before `x`, bisecting every multibyte scalar.
        let title = format!("title{character}x");
        assert_eq!(
            didl_display_title(Some(&title), "file.z", DlnaClientProfile::SamsungTv),
            title,
            "metadata title containing U+{code_point:04X}"
        );

        // Exercise the filename-fallback and matching-extension path as well.
        // Both separators are excluded so this assertion has the same meaning on
        // Unix and Windows; they remain covered in the metadata-title assertion.
        if !matches!(character, '/' | '\\') {
            let stem = format!("title{character}");
            let filename = format!("{stem}.Z");
            assert_eq!(
                didl_display_title(None, &filename, DlnaClientProfile::SamsungTvQ),
                stem,
                "filename containing U+{code_point:04X}"
            );
        }
    }
}

#[test]
fn samsung_extension_handling_covers_suffix_edge_cases() {
    for (base, filename, expected) in [
        ("", "file.flac", ""),
        (".flac", "file.flac", ".flac"),
        ("archive.part.FLAC", "file.flac", "archive.part"),
        ("track.mp3", "file.flac", "track.mp3"),
        ("track.flac", "file", "track.flac"),
        ("track.flac", "file.", "track.flac"),
        ("曲.音", "file.音", "曲"),
        ("曲.音", "file.樂", "曲.音"),
    ] {
        assert_eq!(strip_matching_media_extension(base, filename), expected);
    }
}

#[test]
fn non_samsung_keeps_filename_when_title_missing() {
    assert_eq!(
        didl_display_title(None, "movie.mp4", DlnaClientProfile::Standard),
        "movie.mp4"
    );
    assert_eq!(
        didl_display_title(None, "movie.mp4", DlnaClientProfile::LgTv),
        "movie.mp4"
    );
}

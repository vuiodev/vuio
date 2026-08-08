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

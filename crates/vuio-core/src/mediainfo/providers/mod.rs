//! One module per service.
//!
//! Every provider is split the same way: a `search` that does the I/O, and a pure
//! `parse` that turns the decoded JSON into [`Candidate`]s. The split is what makes
//! the response shapes testable — the parsers run against recorded fixtures, so the
//! test suite never touches the network and a provider changing its schema shows up
//! as a failing assertion rather than an empty library.

use super::client::MetadataProvider;
use super::provider::ProviderKind;

mod anilist;
mod discogs;
mod genius;
mod jikan;
mod kitsu;
mod lastfm;
mod musicbrainz;
mod omdb;
mod tmdb;
mod tvmaze;

/// Build the providers named by `ids`, skipping any that are not recognised.
///
/// Unknown ids are ignored rather than rejected: a config written by a newer
/// version, or one carrying a typo, should cost that one provider and not the
/// whole run.
pub fn build(ids: &[String]) -> Vec<Box<dyn MetadataProvider>> {
    let mut built: Vec<Box<dyn MetadataProvider>> = Vec::new();
    for id in ids {
        let provider: Box<dyn MetadataProvider> = match id.as_str() {
            "tvmaze" => Box::new(tvmaze::TvMaze),
            "tmdb" => Box::new(tmdb::Tmdb),
            "omdb" => Box::new(omdb::Omdb),
            "musicbrainz" => Box::new(musicbrainz::MusicBrainz),
            "discogs" => Box::new(discogs::Discogs),
            "lastfm" => Box::new(lastfm::LastFm),
            "genius" => Box::new(genius::Genius),
            "jikan" => Box::new(jikan::Jikan),
            "anilist" => Box::new(anilist::AniList),
            "kitsu" => Box::new(kitsu::Kitsu),
            unknown => {
                tracing::warn!(provider = unknown, "Ignoring unknown media info provider");
                continue;
            }
        };
        built.push(provider);
    }
    built
}

/// Whether a provider of this kind should be asked about this sort of file.
pub fn serves(kind: ProviderKind, query: super::client::MediaQueryKind) -> bool {
    use super::client::MediaQueryKind as Q;
    match query {
        Q::Movie | Q::Episode => kind.serves_screen(),
        Q::Music => kind == ProviderKind::Music,
        // Anime files go to the anime providers, and fall back to the general
        // screen providers when none are enabled.
        Q::Anime => kind == ProviderKind::Anime || kind.serves_screen(),
    }
}

// ── Shared JSON helpers ────────────────────────────────────────────────────

pub(super) fn text(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|found| found.as_str())
        .map(str::trim)
        .filter(|found| !found.is_empty() && *found != "N/A")
        .map(str::to_string)
}

pub(super) fn number(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|found| match found {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.parse().ok(),
        _ => None,
    })
}

/// The year from an ISO-ish date, or from a bare year.
pub(super) fn year_of(date: &str) -> Option<u32> {
    let digits: String = date.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() != 4 {
        return None;
    }
    let year = digits.parse().ok()?;
    (1800..=2200).contains(&year).then_some(year)
}

/// Collect `[{ "name": ... }]` style genre lists.
pub(super) fn named_list(value: &serde_json::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(|found| found.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| match item {
                    serde_json::Value::String(name) => Some(name.clone()),
                    other => text(other, "name"),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Strip HTML tags and decode the handful of entities these APIs emit.
///
/// TVmaze summaries and AniList descriptions are HTML fragments. Passing those
/// through would put `<p>` into a DIDL `<dc:description>` that a TV then renders
/// literally, so the markup comes out here rather than at every consumer.
pub(super) fn strip_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for character in input.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(character),
            _ => {}
        }
    }
    let out = out
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_html_removes_markup_and_entities() {
        assert_eq!(
            strip_html("<p>A <b>bold</b> claim &amp; a quiet one.</p>"),
            "A bold claim & a quiet one."
        );
    }

    #[test]
    fn strip_html_collapses_the_whitespace_left_behind() {
        assert_eq!(strip_html("<p>one</p>\n\n  <p>two</p>"), "one two");
    }

    #[test]
    fn year_of_reads_dates_and_bare_years() {
        assert_eq!(year_of("2016-11-11"), Some(2016));
        assert_eq!(year_of("1971"), Some(1971));
        assert_eq!(year_of(""), None);
        assert_eq!(year_of("not a date"), None);
    }

    #[test]
    fn text_treats_omdbs_n_a_as_absent() {
        // OMDb writes "N/A" rather than omitting a field, and storing that string
        // as a synopsis would put it on screen.
        let value = serde_json::json!({ "Plot": "N/A", "Title": "Arrival" });
        assert_eq!(text(&value, "Plot"), None);
        assert_eq!(text(&value, "Title").as_deref(), Some("Arrival"));
    }

    #[test]
    fn number_accepts_the_string_forms_these_apis_use() {
        let value = serde_json::json!({ "a": 7.5, "b": "8.1", "c": "N/A" });
        assert_eq!(number(&value, "a"), Some(7.5));
        assert_eq!(number(&value, "b"), Some(8.1));
        assert_eq!(number(&value, "c"), None);
    }
}

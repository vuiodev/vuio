//! Turning a filename into something worth searching for, and deciding whether
//! what came back is actually the same thing.
//!
//! Release names are not metadata, they are a naming convention with thirty years
//! of accreted habits: `Show.Name.S02E05.1080p.WEB-DL.x265-GRP.mkv` carries a
//! title, a season and an episode buried in noise that would sink any search that
//! passed it through verbatim. The parser's whole job is deciding where the title
//! stops.
//!
//! Scoring exists because a search always returns something. "Arrival" matches a
//! 2016 film and a 1996 one; asking for episode 5 and being handed the series is
//! not a match at all. A number that says how sure we are lets the caller store
//! the good ones and show the operator the rest, rather than silently relabelling
//! a library with plausible-looking wrong answers.

use super::client::{Candidate, MediaQuery, MediaQueryKind};

/// What a filename turned out to describe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedName {
    pub kind: MediaQueryKind,
    pub title: String,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

/// Tokens that are never part of a title: quality, source, codec, audio and
/// release-status markers. Anything from the first of these to the end of the
/// name is noise, which is what makes this list the parser's main lever.
const NOISE: &[&str] = &[
    "2160p", "1080p", "1080i", "720p", "480p", "576p", "4k", "uhd", "hdr", "hdr10", "sdr",
    "dolbyvision", "dv", "10bit", "8bit", "x264", "x265", "h264", "h265", "avc", "hevc", "xvid",
    "divx", "av1", "remux", "bluray", "bdrip", "brrip", "bdremux", "webrip", "webdl", "web",
    "hdtv", "hdrip", "dvdrip", "dvd", "vhsrip", "cam", "ts", "tc", "proper", "repack", "internal",
    "limited", "extended", "uncut", "unrated", "remastered", "complete", "aac", "aac2", "ac3",
    "eac3", "dts", "dtshd", "truehd", "atmos", "flac", "mp3", "opus", "dd", "ddp", "5", "7",
    "multi", "dual", "dubbed", "subbed", "subs", "hardsub", "raw", "bd", "ma",
];

/// Tokens that mark a file as anime even without an obvious fansub group.
const ANIME_MARKERS: &[&str] = &["anime", "subsplease", "erai", "horriblesubs", "ohys", "judas"];

fn is_noise(token: &str) -> bool {
    let token = token.trim_matches(|c: char| !c.is_alphanumeric());
    if token.is_empty() {
        return true;
    }
    let lowered = token.to_ascii_lowercase();
    if NOISE.contains(&lowered.as_str()) {
        return true;
    }
    // `DD5.1`, `DDP5.1`, `H.264` and friends survive separator splitting as
    // fragments that are not in the table but are plainly not title words.
    matches!(lowered.as_str(), "1" | "2" | "0")
        || lowered.starts_with("dd5")
        || lowered.starts_with("ddp")
        || lowered.ends_with("bit")
        || lowered.ends_with("fps")
}

/// A four-digit number that could plausibly be a release year.
fn as_year(token: &str) -> Option<u32> {
    let digits = token.trim_matches(|c: char| !c.is_ascii_digit());
    if digits.len() != 4 {
        return None;
    }
    let year: u32 = digits.parse().ok()?;
    (1900..=2099).contains(&year).then_some(year)
}

/// `S02E05`, `s2e5`, and the `2x05` form.
fn as_season_episode(token: &str) -> Option<(u32, u32)> {
    let lowered = token.to_ascii_lowercase();
    let bytes = lowered.as_bytes();

    if bytes.first() == Some(&b's') {
        let rest = &lowered[1..];
        if let Some(split) = rest.find('e') {
            let (season, episode) = rest.split_at(split);
            let episode = &episode[1..];
            if !season.is_empty()
                && !episode.is_empty()
                && season.bytes().all(|b| b.is_ascii_digit())
                && episode.bytes().all(|b| b.is_ascii_digit())
            {
                return Some((season.parse().ok()?, episode.parse().ok()?));
            }
        }
        return None;
    }

    let split = lowered.find('x')?;
    let (season, episode) = lowered.split_at(split);
    let episode = &episode[1..];
    if season.is_empty()
        || episode.is_empty()
        || !season.bytes().all(|b| b.is_ascii_digit())
        || !episode.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    Some((season.parse().ok()?, episode.parse().ok()?))
}

/// Drop `[SubsPlease]`-style bracketed groups, returning what they contained so a
/// fansub tag can still be used as an anime signal.
fn strip_bracketed(name: &str) -> (String, Vec<String>) {
    let mut out = String::with_capacity(name.len());
    let mut captured = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;

    for character in name.chars() {
        match character {
            '[' | '(' | '{' => {
                depth += 1;
                current.clear();
            }
            ']' | ')' | '}' => {
                if depth > 0 {
                    depth -= 1;
                    captured.push(std::mem::take(&mut current));
                    out.push(' ');
                } else {
                    out.push(character);
                }
            }
            _ if depth > 0 => current.push(character),
            _ => out.push(character),
        }
    }
    (out, captured)
}

/// Split a release name into words.
///
/// Dots are separators in scene naming but real punctuation in `S.W.A.T.`, so they
/// only become separators when the name has no spaces of its own to separate by.
fn tokenize(name: &str) -> Vec<String> {
    let separators_are_dots = !name.contains(' ') && name.contains('.');
    name.split(|c: char| {
        c.is_whitespace() || c == '_' || (separators_are_dots && c == '.') || c == '+'
    })
    .filter(|token| !token.is_empty())
    .map(str::to_string)
    .collect()
}

/// Read a release name.
///
/// `stem` is the filename without its extension.
pub fn parse_media_name(stem: &str) -> ParsedName {
    let (without_brackets, bracketed) = strip_bracketed(stem);
    let mut tokens = tokenize(&without_brackets);

    let looks_like_anime = bracketed
        .iter()
        .chain(std::iter::once(&without_brackets))
        .any(|text| {
            let lowered = text.to_ascii_lowercase();
            ANIME_MARKERS.iter().any(|marker| lowered.contains(marker))
        });

    // An anime release names its episode as a bare number after a dash:
    // `[Group] Title - 12 [1080p]`. This has to run before the noise scan, which
    // would stop at the dash and never see the number. Only fansub-marked names
    // get this reading, since a trailing number is otherwise usually part of the
    // title.
    let mut anime_episode = None;
    if looks_like_anime {
        if let Some(position) = tokens.iter().rposition(|token| token == "-") {
            if let Some(number) = tokens.get(position + 1) {
                if !number.is_empty()
                    && number.len() <= 4
                    && number.bytes().all(|byte| byte.is_ascii_digit())
                {
                    anime_episode = number.parse().ok();
                    tokens.truncate(position);
                }
            }
        }
    }

    let mut title_tokens: Vec<String> = Vec::new();
    let mut year = None;
    let mut season = None;
    let mut episode = None;

    for token in &tokens {
        // A season/episode marker ends the title outright — everything after it is
        // either noise or a redundant episode name.
        if season.is_none() {
            if let Some((found_season, found_episode)) = as_season_episode(token) {
                season = Some(found_season);
                episode = Some(found_episode);
                break;
            }
        }
        // A year only ends the title if something already came before it, so
        // `2012.1080p.mkv` keeps its title instead of parsing as a bare year.
        if year.is_none() && !title_tokens.is_empty() {
            if let Some(found) = as_year(token) {
                year = Some(found);
                break;
            }
        }
        if is_noise(token) {
            break;
        }
        title_tokens.push(token.clone());
    }

    // An explicit SxxEyy always wins over the bare-number reading.
    if episode.is_none() {
        episode = anime_episode;
    }

    let title = title_tokens
        .join(" ")
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_string();

    let kind = if looks_like_anime {
        MediaQueryKind::Anime
    } else if season.is_some() || episode.is_some() {
        MediaQueryKind::Episode
    } else {
        MediaQueryKind::Movie
    };

    ParsedName {
        kind,
        title,
        year,
        season,
        episode,
    }
}

/// Reduce a title to comparable words: lowercase, punctuation dropped, and the
/// leading article removed so "The Matrix" and "Matrix" agree.
fn normalize_tokens(title: &str) -> Vec<String> {
    let mut tokens: Vec<String> = title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect();
    if tokens.len() > 1 && matches!(tokens[0].as_str(), "the" | "a" | "an") {
        tokens.remove(0);
    }
    tokens
}

/// Dice coefficient over the two token sets, as a percentage.
fn token_similarity(left: &str, right: &str) -> u32 {
    let left: std::collections::BTreeSet<String> = normalize_tokens(left).into_iter().collect();
    let right: std::collections::BTreeSet<String> = normalize_tokens(right).into_iter().collect();
    if left.is_empty() || right.is_empty() {
        return 0;
    }
    if left == right {
        return 100;
    }
    let shared = left.intersection(&right).count();
    ((2 * shared * 100) / (left.len() + right.len())) as u32
}

/// How confident we are that `candidate` is what `query` was looking for, 0–100.
pub fn score_candidate(query: &MediaQuery, candidate: &Candidate) -> u8 {
    let against_title = token_similarity(&query.title, &candidate.title);
    let against_original = candidate
        .original_title
        .as_deref()
        .map(|original| token_similarity(&query.title, original))
        .unwrap_or(0);
    // For music the album is usually what the provider's record is named after.
    let against_album = query
        .album
        .as_deref()
        .map(|album| token_similarity(album, &candidate.title))
        .unwrap_or(0);

    let mut score = against_title.max(against_original).max(against_album) as i32;

    // Episode agreement is decisive: the right series and the wrong episode is
    // not a near miss, it is the wrong record.
    if let (Some(wanted_season), Some(wanted_episode)) = (query.season, query.episode) {
        match (candidate.season, candidate.episode) {
            (Some(season), Some(episode)) => {
                if season == wanted_season && episode == wanted_episode {
                    score += 10;
                } else {
                    return 0;
                }
            }
            // A series-level record when an episode was asked for is usable but
            // clearly less specific.
            _ => score -= 15,
        }
    }

    match (query.year, candidate.year) {
        (Some(wanted), Some(found)) => {
            let drift = wanted.abs_diff(found);
            if drift == 0 {
                score += 10;
            } else if drift == 1 {
                // Release year and air year disagree by one all the time.
                score -= 10;
            } else {
                // Enough to put an otherwise perfect title below any sane
                // threshold: same name, different decade means a different work.
                score -= 45;
            }
        }
        (Some(_), None) => score -= 5,
        _ => {}
    }

    score.clamp(0, 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(stem: &str) -> ParsedName {
        parse_media_name(stem)
    }

    #[test]
    fn reads_a_scene_episode_name() {
        let name = parsed("Show.Name.S02E05.1080p.WEB-DL.x265-GRP");
        assert_eq!(name.title, "Show Name");
        assert_eq!(name.season, Some(2));
        assert_eq!(name.episode, Some(5));
        assert_eq!(name.kind, MediaQueryKind::Episode);
    }

    #[test]
    fn reads_a_scene_movie_name() {
        let name = parsed("Some.Movie.Title.2019.1080p.BluRay.x264-GRP");
        assert_eq!(name.title, "Some Movie Title");
        assert_eq!(name.year, Some(2019));
        assert_eq!(name.season, None);
        assert_eq!(name.kind, MediaQueryKind::Movie);
    }

    #[test]
    fn reads_the_lowercase_and_x_forms() {
        assert_eq!(parsed("Show Name s2e5 720p").season, Some(2));
        let cross = parsed("Show Name 2x05 720p");
        assert_eq!(cross.season, Some(2));
        assert_eq!(cross.episode, Some(5));
    }

    #[test]
    fn keeps_dots_that_are_part_of_the_title() {
        // The name has spaces of its own, so dots are punctuation here.
        let name = parsed("S.W.A.T. 2017 1080p");
        assert!(
            name.title.starts_with("S.W.A.T"),
            "title was {:?}",
            name.title
        );
        assert_eq!(name.year, Some(2017));
    }

    #[test]
    fn a_leading_year_is_not_mistaken_for_a_release_year() {
        let name = parsed("2012.2009.1080p.BluRay.x264");
        assert_eq!(name.title, "2012");
        assert_eq!(name.year, Some(2009));
    }

    #[test]
    fn drops_bracketed_groups_and_reads_an_anime_episode() {
        let name = parsed("[SubsPlease] Some Show - 12 [1080p][ABCD1234]");
        assert_eq!(name.title, "Some Show");
        assert_eq!(name.episode, Some(12));
        assert_eq!(name.kind, MediaQueryKind::Anime);
    }

    #[test]
    fn a_plain_name_survives_untouched() {
        let name = parsed("Arrival");
        assert_eq!(name.title, "Arrival");
        assert_eq!(name.year, None);
        assert_eq!(name.kind, MediaQueryKind::Movie);
    }

    fn candidate(title: &str) -> Candidate {
        Candidate::new("tvmaze", "movie", "1".to_string(), title.to_string())
    }

    #[test]
    fn an_exact_title_scores_full_marks() {
        let query = MediaQuery {
            title: "Arrival".to_string(),
            ..MediaQuery::default()
        };
        assert_eq!(score_candidate(&query, &candidate("Arrival")), 100);
    }

    #[test]
    fn a_leading_article_does_not_cost_anything() {
        let query = MediaQuery {
            title: "The Matrix".to_string(),
            ..MediaQuery::default()
        };
        assert_eq!(score_candidate(&query, &candidate("Matrix")), 100);
    }

    #[test]
    fn the_wrong_episode_scores_zero() {
        let query = MediaQuery {
            title: "Show Name".to_string(),
            season: Some(2),
            episode: Some(5),
            ..MediaQuery::default()
        };
        let mut wrong = candidate("Show Name");
        wrong.season = Some(2);
        wrong.episode = Some(6);
        assert_eq!(score_candidate(&query, &wrong), 0);

        let mut right = candidate("Show Name");
        right.season = Some(2);
        right.episode = Some(5);
        assert_eq!(score_candidate(&query, &right), 100);
    }

    #[test]
    fn a_year_that_disagrees_sinks_an_otherwise_perfect_title() {
        let query = MediaQuery {
            title: "Arrival".to_string(),
            year: Some(2016),
            ..MediaQuery::default()
        };
        let mut wrong = candidate("Arrival");
        wrong.year = Some(1996);
        // Same title, twenty years apart: it is a different film, and the score has
        // to fall below the default threshold of 60 or it would be stored as good.
        assert!(score_candidate(&query, &wrong) < 60);

        let mut right = candidate("Arrival");
        right.year = Some(2016);
        assert_eq!(score_candidate(&query, &right), 100);
    }

    #[test]
    fn a_year_off_by_one_is_only_a_small_penalty() {
        let query = MediaQuery {
            title: "Some Film".to_string(),
            year: Some(2019),
            ..MediaQuery::default()
        };
        let mut near = candidate("Some Film");
        near.year = Some(2020);
        assert!(score_candidate(&query, &near) >= 60);
    }

    #[test]
    fn an_unrelated_title_scores_low() {
        let query = MediaQuery {
            title: "Arrival".to_string(),
            ..MediaQuery::default()
        };
        assert!(score_candidate(&query, &candidate("Departures")) < 60);
    }

    #[test]
    fn an_album_name_can_carry_the_match_for_music() {
        let query = MediaQuery {
            title: "Black Dog".to_string(),
            album: Some("Led Zeppelin IV".to_string()),
            ..MediaQuery::default()
        };
        assert_eq!(score_candidate(&query, &candidate("Led Zeppelin IV")), 100);
    }
}

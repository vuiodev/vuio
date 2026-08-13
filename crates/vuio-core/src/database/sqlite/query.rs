//! Translating [`MediaFileQuery`] into SQL.
//!
//! Both the owned queries and the borrowed read sessions go through here, so
//! there is exactly one definition of what each query means and how its
//! results are ordered.

use rusqlite::types::Value;

use super::{MediaFileQuery, SqliteDatabase};
use crate::database::sqlite::schema::MEDIA_COLUMNS;

/// Browse order: disc, then track number where present, then natural filename,
/// with untagged records last. `disc_sort` and `track_sort` materialize the
/// first two rules so an index can serve the whole clause.
const BROWSE_ORDER: &str =
    "media_files.disc_sort, media_files.track_sort, media_files.filename COLLATE natural_order";
/// Insertion order, which is also cursor order for paged scans.
const ID_ORDER: &str = "media_files.id";

/// How a MIME filter constrains a query.
pub(super) enum MimeFilter {
    /// No constraint: an empty filter, or the wildcard the directory counters use.
    Any,
    /// A whole family, such as `video/` or `video`.
    Family(String),
    /// A longer prefix, such as `audio/mpeg`, which no family column can answer.
    Prefix(String),
}

impl MimeFilter {
    pub(super) fn parse(filter: Option<&str>) -> Self {
        let Some(filter) = filter else {
            return Self::Any;
        };
        let filter = filter.trim();
        if filter.is_empty() || filter == "*" {
            return Self::Any;
        }
        match filter.split_once('/') {
            None => Self::Family(filter.to_owned()),
            Some((family, "")) => Self::Family(family.to_owned()),
            Some(_) => Self::Prefix(filter.to_owned()),
        }
    }

    /// The `'*'`-style key this filter uses in `directory_mime_counts`.
    pub(super) fn counter_family(&self) -> Option<&str> {
        match self {
            Self::Any => Some("*"),
            Self::Family(family) => Some(family),
            // A longer prefix has no counter; callers fall back to a subtree probe.
            Self::Prefix(_) => None,
        }
    }

    fn push_predicate(&self, clauses: &mut Vec<String>, params: &mut Vec<Value>) {
        match self {
            Self::Any => {}
            Self::Family(family) => {
                clauses.push("media_files.mime_family = ?".to_owned());
                params.push(Value::Text(family.clone()));
            }
            Self::Prefix(prefix) => {
                clauses.push(r"media_files.mime_type LIKE ? ESCAPE '\'".to_owned());
                params.push(Value::Text(format!("{}%", escape_like(prefix))));
            }
        }
    }
}

/// Escape the wildcards SQLite's `LIKE` would otherwise interpret.
pub(super) fn escape_like(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

/// A query reduced to the pieces needed for both counting and paging.
///
/// `source` and `order` are owned rather than static because full-text search
/// builds both: it joins a ranked subquery and orders by that rank.
pub(super) struct MediaQueryPlan {
    source: String,
    filter: String,
    order: String,
    pub(super) params: Vec<Value>,
}

impl MediaQueryPlan {
    /// Total number of matching records, ignoring paging.
    pub(super) fn count_sql(&self) -> String {
        format!("SELECT COUNT(*) FROM {} {}", self.source, self.filter)
    }

    /// One page of records, in the query's defined order.
    pub(super) fn page_sql(&self) -> String {
        format!(
            "SELECT {MEDIA_COLUMNS} FROM {} {} ORDER BY {} LIMIT ? OFFSET ?",
            self.source, self.filter, self.order
        )
    }

    /// Every matching record, in the query's defined order.
    pub(super) fn select_sql(&self) -> String {
        format!(
            "SELECT {MEDIA_COLUMNS} FROM {} {} ORDER BY {}",
            self.source, self.filter, self.order
        )
    }
}

const MEDIA_ONLY: &str = "media_files";
const MEDIA_WITH_ENTRIES: &str =
    "media_files JOIN playlist_entries ON playlist_entries.media_file_id = media_files.id";

/// Media records joined to their best full-text rank.
///
/// A file can match on its own tags and again on a fetched synopsis, which live
/// in different tables and therefore different indexes. `MIN(rank)` keeps
/// whichever match was stronger and collapses the pair to one row, so a film
/// whose title and synopsis both match is ranked well rather than listed twice.
const FTS_JOIN: &str = "media_files JOIN (\
     SELECT mid, MIN(rank) AS rank FROM (\
         SELECT rowid AS mid, bm25(media_fts) AS rank FROM media_fts WHERE media_fts MATCH ? \
         UNION ALL \
         SELECT rowid AS mid, bm25(mediainfo_fts) AS rank FROM mediainfo_fts \
             WHERE mediainfo_fts MATCH ?\
     ) GROUP BY mid\
 ) AS hits ON hits.mid = media_files.id";

/// Turn what a person typed into a valid FTS5 `MATCH` expression.
///
/// User text cannot be handed to FTS5 directly: `"`, `*`, `:`, `-`, `^`, `NEAR`
/// and `OR` are all operators there, so a search for `AC/DC` or `Rock & Roll` is
/// a syntax error rather than a query. Every token is therefore quoted, which
/// makes it a literal, and the tokens are joined with `AND` so more words mean
/// fewer results — what a person expects from a search box.
///
/// The final token also matches as a prefix, so results narrow while someone is
/// still typing the last word.
///
/// Returns `None` when nothing searchable is left, which the caller turns into
/// an empty result rather than an unfiltered one.
pub(super) fn fts5_query_from_user_text(text: &str) -> Option<String> {
    let tokens: Vec<String> = text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{token}\""))
        .collect();
    let (last, rest) = tokens.split_last()?;
    let mut expression = String::new();
    for token in rest {
        expression.push_str(token);
        expression.push_str(" AND ");
    }
    expression.push_str(last);
    expression.push('*');
    Some(expression)
}

pub(super) fn plan(query: &MediaFileQuery) -> MediaQueryPlan {
    let mut clauses: Vec<String> = Vec::new();
    let mut params: Vec<Value> = Vec::new();
    let mut source = MEDIA_ONLY.to_owned();
    let mut order = BROWSE_ORDER.to_owned();

    match query {
        MediaFileQuery::All => {
            order = ID_ORDER.to_owned();
        }
        MediaFileQuery::Id(id) => {
            clauses.push("media_files.id = ?".to_owned());
            params.push(Value::Integer(*id));
            order = ID_ORDER.to_owned();
        }
        MediaFileQuery::Path(path) => {
            clauses.push("media_files.path = ?".to_owned());
            params.push(Value::Text(path.clone()));
            order = ID_ORDER.to_owned();
        }
        MediaFileQuery::Directory { path, mime_family } => {
            clauses.push("media_files.parent_path = ?".to_owned());
            params.push(Value::Text(path.clone()));
            MimeFilter::parse(mime_family.as_deref()).push_predicate(&mut clauses, &mut params);
        }
        MediaFileQuery::Artist(artist) => {
            clauses.push("media_files.artist = ?".to_owned());
            params.push(Value::Text(artist.clone()));
        }
        MediaFileQuery::Album { album, artist } => {
            clauses.push("media_files.album = ?".to_owned());
            params.push(Value::Text(album.clone()));
            if let Some(artist) = artist {
                clauses.push("media_files.artist = ?".to_owned());
                params.push(Value::Text(artist.clone()));
            }
        }
        MediaFileQuery::Genre(genre) => {
            clauses.push("media_files.genre = ?".to_owned());
            params.push(Value::Text(genre.clone()));
        }
        MediaFileQuery::Year(year) => {
            clauses.push("media_files.year = ?".to_owned());
            params.push(Value::Integer(i64::from(*year)));
        }
        MediaFileQuery::AlbumArtist(album_artist) => {
            clauses.push("media_files.album_artist = ?".to_owned());
            params.push(Value::Text(album_artist.clone()));
        }
        MediaFileQuery::Music {
            artist,
            album_artist,
            album,
            genre,
            year,
            exclude_radio,
        } => {
            for (column, value) in [
                ("artist", artist),
                ("album_artist", album_artist),
                ("album", album),
                ("genre", genre),
            ] {
                if let Some(value) = value {
                    clauses.push(format!("media_files.{column} = ?"));
                    params.push(Value::Text(value.clone()));
                }
            }
            if let Some(year) = year {
                clauses.push("media_files.year = ?".to_owned());
                params.push(Value::Integer(i64::from(*year)));
            }
            if *exclude_radio {
                // Radio streams share the audio family but are not part of a
                // music library, and they have no tags to categorize by.
                clauses.push("media_files.mime_family = 'audio'".to_owned());
                clauses.push("media_files.mime_type <> 'audio/radio'".to_owned());
            }
        }
        MediaFileQuery::Playlist(playlist_id) => {
            source = MEDIA_WITH_ENTRIES.to_owned();
            clauses.push("playlist_entries.playlist_id = ?".to_owned());
            params.push(Value::Integer(*playlist_id));
            order = "playlist_entries.position".to_owned();
        }
        MediaFileQuery::Filtered {
            after_id,
            mime_family,
            text,
        } => {
            if let Some(after) = after_id {
                clauses.push("media_files.id > ?".to_owned());
                params.push(Value::Integer(*after));
            }
            MimeFilter::parse(mime_family.as_deref()).push_predicate(&mut clauses, &mut params);
            if let Some(text) = text.as_deref().filter(|text| !text.is_empty()) {
                // `LIKE` is case-insensitive for ASCII, matching the
                // case-insensitive substring search this replaces. The pattern
                // is bound once per column rather than numbered, so the plan's
                // parameters stay positional throughout.
                clauses.push(
                    r"(media_files.filename LIKE ? ESCAPE '\'
                       OR media_files.title LIKE ? ESCAPE '\'
                       OR media_files.artist LIKE ? ESCAPE '\'
                       OR media_files.album LIKE ? ESCAPE '\')"
                        .to_owned(),
                );
                let pattern = format!("%{}%", escape_like(text));
                for _ in 0..4 {
                    params.push(Value::Text(pattern.clone()));
                }
            }
            // Results are id-ordered so `after_id` can resume a scan.
            order = ID_ORDER.to_owned();
        }
        MediaFileQuery::Search { text, mime_family } => {
            match fts5_query_from_user_text(text) {
                Some(expression) => {
                    source = FTS_JOIN.to_owned();
                    // The same expression against both indexes. Their bm25
                    // scores are not strictly comparable — they are computed
                    // over different corpora — but both are negative and both
                    // get better as they fall, which is enough to order by.
                    params.push(Value::Text(expression.clone()));
                    params.push(Value::Text(expression));
                    // Ties broken by id so paging is stable: two documents of
                    // equal rank must not swap places between pages.
                    order = "hits.rank, media_files.id".to_owned();
                }
                // Nothing searchable survived. Match nothing rather than
                // everything: a query of pure punctuation asked for something
                // specific, and the whole library is not it.
                None => clauses.push("0".to_owned()),
            }
            MimeFilter::parse(mime_family.as_deref()).push_predicate(&mut clauses, &mut params);
        }
    }

    let filter = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    };

    MediaQueryPlan {
        source,
        filter,
        order,
        params,
    }
}

/// Records under a directory subtree, used by prefix queries and removals.
pub(super) fn subtree_params(directory: &str) -> (String, Vec<Value>) {
    let (start, end) = SqliteDatabase::subtree_range(directory);
    (
        "media_files.path >= ? AND media_files.path < ?".to_owned(),
        vec![Value::Text(start), Value::Text(end)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every token quoted, joined with AND, last one a prefix. Quoting is what
    /// makes a token a literal instead of an operator.
    #[test]
    fn user_text_becomes_a_conjunction_of_quoted_tokens() {
        assert_eq!(
            fts5_query_from_user_text("beethoven symphony"),
            Some(r#""beethoven" AND "symphony"*"#.to_string())
        );
        assert_eq!(
            fts5_query_from_user_text("moon"),
            Some(r#""moon"*"#.to_string())
        );
    }

    /// Each of these is an FTS5 operator. Reaching the engine unquoted, any of
    /// them would be a syntax error rather than a search.
    #[test]
    fn fts5_operators_are_neutralised_rather_than_passed_through() {
        for (input, expected) in [
            ("AC/DC", r#""AC" AND "DC"*"#),
            (r#""quoted""#, r#""quoted"*"#),
            ("rock & roll", r#""rock" AND "roll"*"#),
            ("NEAR", r#""NEAR"*"#),
            ("a OR b", r#""a" AND "OR" AND "b"*"#),
            ("-minus", r#""minus"*"#),
            ("column:value", r#""column" AND "value"*"#),
            ("star*", r#""star"*"#),
            ("^caret", r#""caret"*"#),
        ] {
            assert_eq!(
                fts5_query_from_user_text(input).as_deref(),
                Some(expected),
                "{input}"
            );
        }
    }

    /// Nothing searchable left means an empty result, which the caller turns
    /// into no rows. Returning every row instead would answer a specific
    /// question with the whole library.
    #[test]
    fn a_query_with_no_searchable_tokens_is_rejected() {
        assert_eq!(fts5_query_from_user_text(""), None);
        assert_eq!(fts5_query_from_user_text("   "), None);
        assert_eq!(fts5_query_from_user_text("!!!"), None);
        assert_eq!(fts5_query_from_user_text("- : \" *"), None);
    }

    /// Non-ASCII letters are alphanumeric and must survive tokenization; the
    /// index folds their accents at match time.
    #[test]
    fn non_ascii_words_survive_tokenization() {
        assert_eq!(
            fts5_query_from_user_text("Björk Vespertine"),
            Some(r#""Björk" AND "Vespertine"*"#.to_string())
        );
        assert_eq!(
            fts5_query_from_user_text("久石譲"),
            Some(r#""久石譲"*"#.to_string())
        );
    }
}

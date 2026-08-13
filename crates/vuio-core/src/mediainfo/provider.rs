//! The provider roster.
//!
//! This is static data — no network, no `reqwest` — so it compiles whether or not
//! the `mediainfo` feature is on. The config layer needs it to know what a valid
//! provider id is, and the admin schema needs it to describe the credential fields,
//! and neither of those should require the feature that does the fetching.

/// What a provider knows about. A file is only offered to providers whose kind
/// matches what the filename parsed as, so a music lookup never burns a TV API's
/// rate limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Tv,
    Movie,
    /// Movies and TV from one endpoint.
    Screen,
    Music,
    Anime,
}

impl ProviderKind {
    /// The heading this provider appears under in the dashboard.
    pub fn group(&self) -> &'static str {
        match self {
            Self::Tv | Self::Movie | Self::Screen => "Movies & TV",
            Self::Music => "Music",
            Self::Anime => "Anime & Manga",
        }
    }

    pub fn serves_screen(&self) -> bool {
        matches!(self, Self::Tv | Self::Movie | Self::Screen)
    }
}

/// The credential a provider needs before it will answer.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct CredentialSpec {
    /// What the provider calls it, so the field label matches their signup page.
    pub label: &'static str,
    /// Where to get one.
    pub signup_url: &'static str,
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct ProviderInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: ProviderKind,
    /// What this provider contributes, in the dashboard's words.
    pub provides: &'static str,
    /// `None` means it answers without an account.
    pub credential: Option<CredentialSpec>,
}

impl ProviderInfo {
    pub fn needs_credential(&self) -> bool {
        self.credential.is_some()
    }
}

const fn free(
    id: &'static str,
    label: &'static str,
    kind: ProviderKind,
    provides: &'static str,
) -> ProviderInfo {
    ProviderInfo {
        id,
        label,
        kind,
        provides,
        credential: None,
    }
}

const fn keyed(
    id: &'static str,
    label: &'static str,
    kind: ProviderKind,
    provides: &'static str,
    credential_label: &'static str,
    signup_url: &'static str,
) -> ProviderInfo {
    ProviderInfo {
        id,
        label,
        kind,
        provides,
        credential: Some(CredentialSpec {
            label: credential_label,
            signup_url,
        }),
    }
}

/// Every provider VuIO can consult.
///
/// Cover Art Archive is deliberately absent: it has no search of its own and is
/// only ever reached through a MusicBrainz release id, so it is part of the
/// MusicBrainz provider rather than something to switch on separately.
pub const PROVIDERS: &[ProviderInfo] = &[
    free(
        "tvmaze",
        "TVmaze",
        ProviderKind::Tv,
        "TV shows, episode guides, cast and artwork.",
    ),
    keyed(
        "tmdb",
        "TheMovieDB",
        ProviderKind::Screen,
        "Movies, TV, posters, trailers and ratings.",
        "API key",
        "https://developer.themoviedb.org",
    ),
    keyed(
        "omdb",
        "OMDb",
        ProviderKind::Screen,
        "IMDb ratings, posters and plot summaries.",
        "API key",
        "https://omdbapi.com",
    ),
    free(
        "musicbrainz",
        "MusicBrainz",
        ProviderKind::Music,
        "Artists, albums, tracklists and release dates, with Cover Art Archive artwork.",
    ),
    keyed(
        "discogs",
        "Discogs",
        ProviderKind::Music,
        "Vinyl, CD and master releases, and artist discographies.",
        "Personal access token",
        "https://www.discogs.com/settings/developers",
    ),
    keyed(
        "lastfm",
        "Last.fm",
        ProviderKind::Music,
        "Artist biographies, tags and album art.",
        "API key",
        "https://www.last.fm/api/account/create",
    ),
    keyed(
        "genius",
        "Genius",
        ProviderKind::Music,
        "Song, artist and album metadata.",
        "Access token",
        "https://genius.com/api-clients",
    ),
    free(
        "jikan",
        "Jikan",
        ProviderKind::Anime,
        "Anime and manga from MyAnimeList, with characters and ratings.",
    ),
    free(
        "anilist",
        "AniList",
        ProviderKind::Anime,
        "Anime and manga metadata and artwork.",
    ),
    free(
        "kitsu",
        "Kitsu",
        ProviderKind::Anime,
        "Anime, manga and drama info, categories and characters.",
    ),
];

/// The providers enabled out of the box: every one that answers without an
/// account. Anything needing a credential stays off until the operator supplies
/// one, so a default install cannot produce a run that 401s on half its lookups.
pub const DEFAULT_PROVIDER_IDS: &[&str] = &["tvmaze", "musicbrainz", "jikan", "anilist", "kitsu"];

pub fn provider_info(id: &str) -> Option<&'static ProviderInfo> {
    PROVIDERS.iter().find(|provider| provider.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_providers_all_exist_and_need_no_account() {
        for id in DEFAULT_PROVIDER_IDS {
            let provider = provider_info(id).unwrap_or_else(|| panic!("unknown provider {id}"));
            assert!(
                !provider.needs_credential(),
                "{id} is on by default but needs a credential"
            );
        }
    }

    #[test]
    fn every_free_provider_is_on_by_default() {
        // Otherwise a provider that costs nothing to use would sit unused because
        // someone forgot to add it to the list.
        for provider in PROVIDERS.iter().filter(|p| !p.needs_credential()) {
            assert!(
                DEFAULT_PROVIDER_IDS.contains(&provider.id),
                "{} needs no account but is not on by default",
                provider.id
            );
        }
    }

    #[test]
    fn no_provider_id_is_repeated() {
        let mut seen = std::collections::HashSet::new();
        for provider in PROVIDERS {
            assert!(seen.insert(provider.id), "duplicate provider {}", provider.id);
        }
    }
}

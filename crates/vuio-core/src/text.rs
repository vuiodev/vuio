//! Normalization for the text VuIO compares, indexes and displays.
//!
//! The same name can be spelled more than one way in Unicode. `Füßen` is either
//! `F U+00FC ß e n` (NFC, what Windows and most taggers write) or
//! `F u U+0308 ß e n` (NFD, what macOS wrote for years and what HFS+ still hands
//! back). The two are canonically equivalent and look identical, but they share
//! no bytes, so every byte-oriented comparison we make — a `LIKE` clause, an FTS
//! token, a `==` between a scraped title and a parsed one — silently says "no".
//!
//! Everything we store for comparison or display is therefore folded to NFC on
//! the way into the database, and every search term is folded on the way in, so
//! both sides of a comparison are spelled the same way whatever the source used.
//!
//! # Paths are deliberately excluded
//!
//! A path is not text, it is a filesystem key. Linux and Windows compare the
//! bytes exactly: on ext4 the NFC and NFD spellings of `Füßen.flac` are two
//! different files, and only one of them exists. `canonical_to_platform` hands
//! the stored path straight to `PathBuf`, so normalizing it would produce a name
//! the kernel cannot open. Paths keep whatever the filesystem reported, byte for
//! byte; only `filename`, the display copy, is folded.

use std::borrow::Cow;
use unicode_normalization::{is_nfc_quick, IsNormalized, UnicodeNormalization};

/// Fold `value` to NFC, borrowing when it is already in that form.
///
/// `is_nfc_quick` answers `Yes` or `No` from character properties alone for the
/// overwhelming majority of input — ASCII included — so the common path costs a
/// scan and no allocation. Only a `Maybe` pays for the full composition.
pub(crate) fn to_nfc(value: &str) -> Cow<'_, str> {
    match is_nfc_quick(value.chars()) {
        IsNormalized::Yes => Cow::Borrowed(value),
        _ => Cow::Owned(value.nfc().collect()),
    }
}

/// Fold an owned string in place, keeping the allocation when it is already NFC.
pub(crate) fn into_nfc(value: String) -> String {
    match to_nfc(&value) {
        Cow::Borrowed(_) => value,
        Cow::Owned(normalized) => normalized,
    }
}

/// Fold an optional field, as media metadata columns are.
pub(crate) fn normalize_field(field: &mut Option<String>) {
    if let Some(value) = field.take() {
        *field = Some(into_nfc(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NFC: &str = "Füßen";
    const NFD: &str = "Fu\u{308}\u{df}en";

    #[test]
    fn the_two_spellings_differ_until_they_are_folded() {
        // The premise of this module: equal to a reader, unequal to a comparison.
        assert_ne!(NFC, NFD);
        assert_eq!(to_nfc(NFC), to_nfc(NFD));
        assert_eq!(to_nfc(NFD), NFC);
    }

    #[test]
    fn text_already_in_nfc_is_not_copied() {
        assert!(matches!(to_nfc("plain ascii"), Cow::Borrowed(_)));
        assert!(matches!(to_nfc(NFC), Cow::Borrowed(_)));
        assert!(matches!(to_nfc(NFD), Cow::Owned(_)));
    }

    #[test]
    fn folding_is_idempotent_across_the_corpus() {
        for sample in crate::unicode_corpus::alignment_sweep() {
            let once = to_nfc(&sample).into_owned();
            let twice = to_nfc(&once).into_owned();
            assert_eq!(once, twice, "folding {sample:?} was not idempotent");
        }
    }

    #[test]
    fn an_empty_or_absent_field_stays_that_way() {
        let mut absent = None;
        normalize_field(&mut absent);
        assert_eq!(absent, None);

        let mut empty = Some(String::new());
        normalize_field(&mut empty);
        assert_eq!(empty.as_deref(), Some(""));
    }
}

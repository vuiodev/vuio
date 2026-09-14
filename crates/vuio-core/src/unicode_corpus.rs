//! Text that byte-oriented string handling has historically broken on, plus the
//! alignment sweep that turns offset bugs into deterministic test failures.
//!
//! Issue #51 was `&title[title.len() - 5..]` splitting a `ü` in half. A flat list
//! of awkward strings would not have caught it: the panic needs the slice point to
//! land *inside* a multi-byte character, which depends on the string's length
//! relative to that character's width. [`alignment_sweep`] therefore repeats every
//! sample at eight different paddings, so whatever index the code under test
//! computes eventually lands on every byte position of every sample — including
//! the continuation bytes that are never valid boundaries.

/// Strings that stress encoding width, combining marks, direction and escaping.
pub(crate) const SAMPLES: &[&str] = &[
    "",
    "plain ascii title",
    // Issue #51, and the umlauts that reported it.
    "Wie Felsenabgrund mir zu Füßen",
    "Grüße aus München",
    "Ärger mit Öl",
    // The same word pre-composed (NFC) and decomposed (NFD). macOS hands back
    // whichever form was written, so both reach us from the same directory.
    "Füßen",
    "Fu\u{308}\u{df}en",
    // Scripts with no ASCII at all, including right-to-left and Indic clusters.
    "Лунная соната",
    "Ἀχιλλεύς",
    "交響曲第八番 変ホ長調",
    "안녕하세요",
    "שלום עולם",
    "مرحبا بالعالم",
    "नमस्ते",
    "สวัสดีชาวโลก",
    // Four-byte scalars, and graphemes built from several of them.
    "🎵🎶",
    "👩‍👩‍👧‍👦",
    "👋🏽",
    "🇩🇪",
    "𝄞 Clair de Lune",
    "\u{10ffff}",
    // Marks and invisibles that make character count disagree with byte count.
    "e\u{301}\u{302}\u{303}",
    "a\u{200b}b\u{200d}c",
    "a\u{202e}reversed",
    "\u{feff}leading bom",
    // Case mapping that changes length, and the dotted/dotless Turkish pair.
    "ß vs SS",
    "ǅunglica",
    "ﬁligree",
    "İstanbul",
    "ırmak",
    // Characters an XML writer has to replace rather than pass through.
    "A&B <tag> \"quoted\" 'single'",
    "bell\u{7}vertical\u{b}form\u{c}feed",
    "\u{fffe} noncharacter",
    "…ellipsis…",
];

/// Every sample at eight paddings on each side, so a computed byte offset lands
/// on every position within every sample across the whole sweep.
pub(crate) fn alignment_sweep() -> impl Iterator<Item = String> {
    SAMPLES.iter().flat_map(|sample| {
        (0..8).flat_map(move |pad| {
            let filler = "x".repeat(pad);
            [format!("{filler}{sample}"), format!("{sample}{filler}")]
        })
    })
}

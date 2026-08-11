//! SubRip (`.srt`) to WebVTT conversion.
//!
//! Sidecar subtitles are stored as SRT, which every TV understands and no browser does:
//! the HTML `<track>` element accepts WebVTT only. The two formats are close enough that
//! a text transform covers it — the differences that matter are the `WEBVTT` signature
//! line and the decimal separator in cue timings (`00:00:01,000` vs `00:00:01.000`).

/// Convert SubRip text into WebVTT.
///
/// Everything that is not a cue-timing line is passed through untouched, including cue
/// numbers (valid WebVTT cue identifiers) and SRT's `<i>`/`<b>` markup (valid WebVTT
/// markup). Malformed timings are left as close to the original as possible rather than
/// dropped, on the theory that a slightly wrong cue beats a missing one.
pub fn srt_to_vtt(srt: &str) -> String {
    let trimmed = srt.strip_prefix('\u{feff}').unwrap_or(srt);

    // `str::lines` already handles CRLF; a lone CR (classic Mac line endings) needs a
    // pass of its own.
    let converted;
    let body = if trimmed.contains('\r') {
        converted = trimmed.replace("\r\n", "\n").replace('\r', "\n");
        converted.as_str()
    } else {
        trimmed
    };

    let mut out = String::with_capacity(body.len() + 16);
    out.push_str("WEBVTT\n\n");

    for line in body.lines() {
        if is_timing_line(line) {
            out.push_str(&convert_timing_line(line));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }

    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// A cue-timing line, as opposed to a caption that merely happens to contain an arrow.
/// Requires the arrow *and* a leading token that looks like a timestamp.
fn is_timing_line(line: &str) -> bool {
    if !line.contains("-->") {
        return false;
    }
    line.split_whitespace().next().is_some_and(|first| {
        first.contains(':') && first.starts_with(|c: char| c.is_ascii_digit())
    })
}

fn convert_timing_line(line: &str) -> String {
    let Some((start, rest)) = line.split_once("-->") else {
        return line.to_string();
    };

    // SRT sometimes carries display coordinates after the end timestamp
    // (`X1:040 X2:600 Y1:050 Y2:100`). WebVTT parses trailing tokens as cue settings and
    // these are not valid ones, so drop everything after the timestamp.
    let end = rest.split_whitespace().next().unwrap_or("");

    format!(
        "{} --> {}",
        normalize_timestamp(start.trim()),
        normalize_timestamp(end)
    )
}

/// Normalize a single timestamp to WebVTT's `HH:MM:SS.mmm`.
///
/// Handles the common SRT deviations: a comma separator, a one-digit hour, and a missing
/// hour or millisecond component. Anything it cannot parse gets the comma swap only.
fn normalize_timestamp(stamp: &str) -> String {
    let dotted = stamp.replace(',', ".");
    let (clock, millis) = match dotted.split_once('.') {
        Some((clock, millis)) => (clock, millis),
        None => (dotted.as_str(), "0"),
    };

    let mut parts = clock.split(':').collect::<Vec<_>>();
    if parts.len() == 2 {
        parts.insert(0, "0");
    }
    if parts.len() != 3 || !parts.iter().all(|p| digits_only(p)) || !digits_only(millis) {
        return dotted;
    }

    format!(
        "{:0>2}:{:0>2}:{:0>2}.{:0<3}",
        parts[0],
        parts[1],
        parts[2],
        &millis[..millis.len().min(3)]
    )
}

fn digits_only(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_a_basic_cue() {
        let vtt = srt_to_vtt("1\n00:00:01,000 --> 00:00:04,500\nHello\n");
        assert_eq!(vtt, "WEBVTT\n\n1\n00:00:01.000 --> 00:00:04.500\nHello\n");
    }

    #[test]
    fn strips_bom_and_normalizes_crlf() {
        let vtt = srt_to_vtt("\u{feff}1\r\n00:00:01,000 --> 00:00:02,000\r\nHi\r\n");
        assert!(vtt.starts_with("WEBVTT\n\n1\n"));
        assert!(!vtt.contains('\r'));
        assert!(vtt.contains("00:00:01.000 --> 00:00:02.000"));
    }

    #[test]
    fn pads_a_single_digit_hour() {
        let vtt = srt_to_vtt("1\n0:00:01,000 --> 0:00:04,000\nHello\n");
        assert!(vtt.contains("00:00:01.000 --> 00:00:04.000"));
    }

    #[test]
    fn supplies_a_missing_hour_and_millis() {
        let vtt = srt_to_vtt("1\n00:01 --> 00:02,5\nHello\n");
        assert!(vtt.contains("00:00:01.000 --> 00:00:02.500"));
    }

    #[test]
    fn drops_srt_display_coordinates() {
        let vtt = srt_to_vtt("1\n00:00:01,000 --> 00:00:04,000 X1:040 X2:600\nHello\n");
        assert!(vtt.contains("00:00:01.000 --> 00:00:04.000\n"));
        assert!(!vtt.contains("X1:040"));
    }

    #[test]
    fn leaves_caption_text_alone() {
        let vtt = srt_to_vtt("1\n00:00:01,000 --> 00:00:04,000\n<i>Ich w\u{fc}rde --> so</i>\n");
        assert!(vtt.contains("<i>Ich w\u{fc}rde --> so</i>"));
    }

    #[test]
    fn always_emits_a_signature_and_trailing_newline() {
        let vtt = srt_to_vtt("");
        assert_eq!(vtt, "WEBVTT\n\n");

        let unterminated = srt_to_vtt("1\n00:00:01,000 --> 00:00:02,000\nHi");
        assert!(unterminated.starts_with("WEBVTT\n\n"));
        assert!(unterminated.ends_with("Hi\n"));
    }

    #[test]
    fn passes_through_a_timing_line_it_cannot_parse() {
        let vtt = srt_to_vtt("1\n00:xx:01,000 --> 00:00:04,000\nHello\n");
        assert!(vtt.contains("00:xx:01.000 --> 00:00:04.000"));
    }
}

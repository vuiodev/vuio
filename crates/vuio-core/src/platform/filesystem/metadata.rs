//! Reading tags, stream properties and cover art with symphonia.
//!
//! One reader covers every container symphonia can demux, so a library of OGG,
//! Opus, FLAC, AIFF or MP4 files categorizes the same way an MP3 library does.
//! APEv1 and APEv2 tags come along for free: symphonia registers its APE reader
//! as a probeable metadata source and scans for trailing metadata before it
//! looks for a container, which is exactly where APE tags live.
//!
//! Three gaps are worth knowing about, all of them upstream:
//!
//! - `.wma` has no ASF reader at all.
//! - A bare Monkey's Audio `.ape` file has no demuxer. The APE *tag* reader
//!   cannot rescue a container that never probes.
//! - `.wav` reads no tags. symphonia 0.6.0's WAV reader parses the RIFF INFO
//!   list into a metadata log and then overwrites that log with the empty one
//!   from its options before returning, so the tags it collected are dropped.
//!   AIFF, which shares the same crate, does this correctly.
//!
//! All three fall back to parsing the filename.

use super::*;
use crate::database::{AudioTags, MediaFile, StreamInfo};
use std::time::Duration;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, MetadataRevision, RawValue, StandardTag};

/// Bumped whenever this module learns to extract something it used to miss.
///
/// Records carry the version that wrote them, so a scan re-reads anything
/// written by an older extractor even though the file itself has not changed.
/// The file is opened and parsed on every scan regardless, so a bump costs one
/// database write per record and no extra I/O.
pub(crate) const TAGS_VERSION: u32 = 2;

/// Longest tag value kept in `media_tags`.
///
/// Lyrics sheets and acoustic fingerprints run to kilobytes and are indexed
/// alongside everything else, so they are dropped rather than allowed to
/// dominate the table.
const MAX_TAG_VALUE_LEN: usize = 4096;

/// Tags whose values are large enough to be worth storing nowhere.
const OVERSIZED_TAGS: &[&str] = &["Lyrics", "AcoustIdFingerprint", "CdToc"];

/// Read only the stream properties of a file, leaving its titling alone.
///
/// For video. A film's title comes from its filename (and, where the operator
/// enabled it, from the metadata fetcher); running the tag reader's
/// artist/album/track-number logic over it would fill a library's video rows
/// with whatever a muxer happened to write. What is wanted here is one field:
/// which codec the audio track is in, so the browse path can decide whether to
/// offer a decoded alternative without opening the file.
///
/// This is a header probe. It reads the container's front matter and its track
/// declarations; it demuxes nothing and decodes nothing.
pub(crate) async fn extract_stream_info(
    media_file: &mut MediaFile,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path = media_file.path.clone();
    match tokio::task::spawn_blocking(move || probe_metadata(&path)).await {
        Ok(Ok(probed)) => {
            media_file.stream = probed.stream;
            if media_file.duration.is_none() {
                media_file.duration = probed.duration;
            }
        }
        Ok(Err(error)) => {
            tracing::debug!(
                path = %media_file.path.display(),
                %error,
                "Failed to probe video stream properties"
            );
        }
        Err(error) => {
            tracing::debug!(
                path = %media_file.path.display(),
                %error,
                "Failed to execute blocking stream probe"
            );
        }
    }
    Ok(())
}

pub(crate) async fn extract_audio_metadata(
    media_file: &mut MediaFile,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path = media_file.path.clone();

    // Probing is synchronous file I/O and parsing, so it stays off the async
    // runtime the same way the previous reader did.
    match tokio::task::spawn_blocking(move || probe_metadata(&path)).await {
        Ok(Ok(probed)) => probed.apply(media_file),
        Ok(Err(error)) => {
            tracing::debug!(
                path = %media_file.path.display(),
                %error,
                "Failed to extract audio metadata during format probe; falling back to filename metadata"
            );
        }
        Err(error) => {
            tracing::debug!(
                path = %media_file.path.display(),
                %error,
                "Failed to execute blocking metadata extraction"
            );
        }
    }

    // Always fall back to parsing from filename for missing fields
    fallback_parse_filename(media_file);

    Ok(())
}

/// Read the first embedded picture from a file, if it has one.
///
/// Used to serve cover art for tracks with no image file beside them.
pub(crate) fn extract_embedded_cover(path: &Path) -> Option<(String, Vec<u8>)> {
    let mut format = open_format(path).ok()?;
    let mut log = format.metadata();
    let mut cover = None;

    // The newest revision wins, so keep overwriting as the log is drained from
    // oldest to newest.
    let mut absorb = |revision: &MetadataRevision| {
        if let Some(visual) = revision.media.visuals.first() {
            cover = Some((
                visual
                    .media_type
                    .clone()
                    .unwrap_or_else(|| "image/jpeg".to_owned()),
                visual.data.to_vec(),
            ));
        }
    };
    while let Some(revision) = log.pop() {
        absorb(&revision);
    }
    if let Some(revision) = log.current() {
        absorb(revision);
    }
    cover
}

/// Everything one probe of a file yields.
#[derive(Default)]
struct ProbedMetadata {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,
    track_number: Option<u32>,
    year: Option<u32>,
    album_artist: Option<String>,
    duration: Option<Duration>,
    tags: AudioTags,
    stream: StreamInfo,
    extra_tags: Vec<(String, String)>,
}

impl ProbedMetadata {
    fn apply(self, media_file: &mut MediaFile) {
        // A probe that found nothing must not clear what a caller already set,
        // so every field is only written when the probe produced one.
        if self.title.is_some() {
            media_file.title = self.title;
        }
        if self.artist.is_some() {
            media_file.artist = self.artist;
        }
        if self.album.is_some() {
            media_file.album = self.album;
        }
        if self.genre.is_some() {
            media_file.genre = self.genre;
        }
        if self.track_number.is_some() {
            media_file.track_number = self.track_number;
        }
        if self.year.is_some() {
            media_file.year = self.year;
        }
        if self.album_artist.is_some() {
            media_file.album_artist = self.album_artist;
        }
        if self.duration.is_some() {
            media_file.duration = self.duration;
        }

        media_file.tags = self.tags;
        media_file.stream = self.stream;
        media_file.extra_tags = self.extra_tags;

        // Average bit rate over the whole file. No container reports this
        // directly and DLNA wants an average anyway, so derive it from the two
        // numbers that are always available.
        if media_file.stream.bit_rate.is_none() {
            if let Some(seconds) = media_file
                .duration
                .map(|duration| duration.as_secs_f64())
                .filter(|seconds| *seconds > 0.0)
            {
                let bits_per_second = (media_file.size as f64 * 8.0) / seconds;
                if bits_per_second.is_finite() && bits_per_second > 0.0 {
                    media_file.stream.bit_rate = Some(bits_per_second as u32);
                }
            }
        }

        media_file.tags_version = TAGS_VERSION;
    }
}

fn open_format(
    path: &Path,
) -> anyhow::Result<Box<dyn symphonia::core::formats::FormatReader + 'static>> {
    let file = std::fs::File::open(path)?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }
    let format = symphonia::default::get_probe().probe(
        &hint,
        stream,
        FormatOptions::default(),
        MetadataOptions::default(),
    )?;
    Ok(format)
}

fn probe_metadata(path: &Path) -> anyhow::Result<ProbedMetadata> {
    let mut format = open_format(path)?;
    let mut probed = ProbedMetadata::default();

    // The video track's codec, where there is one. Recorded so the browse path
    // can tell a film whose picture can be copied through from one whose cannot
    // without opening either.
    if let Some(track) = format.default_track(TrackType::Video) {
        if let Some(video) = track.codec_params.as_ref().and_then(|params| params.video()) {
            probed.stream.video_codec = video_codec_short_name(video.codec);
        }
    }

    // The container's own duration (e.g. Matroska's Segment > Info > Duration or MP4 mvhd)
    let media_info = format.media_info();
    if let (Some(time_base), Some(duration)) = (media_info.time_base, media_info.duration) {
        if let Some(t) = time_base.calc_time(symphonia::core::units::Timestamp::new(duration.get() as i64)) {
            let secs = t.as_secs_f64();
            if secs > 0.0 {
                probed.duration = Some(Duration::from_secs_f64(secs));
            }
        }
    }

    // Stream properties come off the default audio track. A container with no
    // audio track still has usable tags, so this is not an error.
    if let Some(track) = format.default_track(TrackType::Audio) {
        let num_frames = track.num_frames;
        if let Some(audio) = track.codec_params.as_ref().and_then(|params| params.audio()) {
            probed.stream.codec = audio_codec_short_name(audio.codec).map(|c| c.to_string());
            probed.stream.sample_rate = audio.sample_rate;
            probed.stream.channels = audio
                .channels
                .as_ref()
                .map(|channels| channels.count() as u16);
            probed.stream.bits_per_sample = audio
                .bits_per_sample
                .or(audio.bits_per_coded_sample)
                .map(|bits| bits as u16);

            if probed.duration.is_none() {
                if let (Some(frames), Some(rate)) = (num_frames, audio.sample_rate.filter(|r| *r > 0)) {
                    probed.duration = Some(Duration::from_secs_f64(frames as f64 / f64::from(rate)));
                }
            }
        }
    }

    // If any audio track in the container is DTS, record it as DTS so that DTS transcoding
    // can be properly applied for the film.
    let has_dts = format.tracks().iter().any(|t| {
        t.track_type() == Some(TrackType::Audio)
            && t.codec_params
                .as_ref()
                .and_then(|p| p.audio())
                .and_then(|a| audio_codec_short_name(a.codec))
                .map(|c| c == "dca" || c == "dts")
                .unwrap_or(false)
    });
    if has_dts {
        probed.stream.codec = Some("dts".to_string());
    }

    // A file can carry more than one revision — ID3v2 at the head and APEv2 at
    // the tail, say. Draining the log oldest-first and letting later writes win
    // keeps every tag while still preferring the newest revision.
    let mut log = format.metadata();
    while let Some(revision) = log.pop() {
        absorb_revision(&revision, &mut probed);
    }
    if let Some(revision) = log.current() {
        absorb_revision(revision, &mut probed);
    }

    Ok(probed)
}

/// The short name to record for an identified audio codec.
///
/// Symphonia's registry is asked first, so anything it can decode keeps the
/// exact spelling it has always been stored under. The fallback below covers
/// the codecs it *identifies but cannot decode* — which, before this existed,
/// stored a NULL codec and so were indistinguishable from a file whose track
/// nothing recognised. Those are precisely the codecs a television is most
/// likely to be missing a licence for, so they are the ones the browse path
/// most needs to know about.
///
/// TrueHD is named here and is deliberately *not* decodable: nothing vendored
/// decodes it, [`crate::media::transcode::TranscodeCodec::from_stored_codec`]
/// returns `None` for it, and recording it is worth doing anyway so a
/// diagnostic can say what the track is instead of shrugging.
fn audio_codec_short_name(
    codec: symphonia::core::codecs::audio::AudioCodecId,
) -> Option<String> {
    use symphonia::core::codecs::audio::well_known::*;

    if let Some(registered) = symphonia::default::get_codecs().get_audio_decoder(codec) {
        return Some(registered.codec.info.short_name.to_owned());
    }
    let name = match codec {
        CODEC_ID_AC3 => "ac3",
        CODEC_ID_EAC3 => "eac3",
        CODEC_ID_DCA => "dca",
        CODEC_ID_TRUEHD => "truehd",
        CODEC_ID_AC4 => "ac4",
        CODEC_ID_WMA => "wma",
        CODEC_ID_OPUS => "opus",
        _ => return None,
    };
    Some(name.to_owned())
}

/// The short name to record for an identified video codec.
///
/// Nothing here decodes video, so symphonia's decoder registry has no opinion
/// and this is a plain table. Only the names the remuxer acts on need to be
/// distinguishable; the rest are recorded so a diagnostic can say what a file
/// holds instead of shrugging.
fn video_codec_short_name(
    codec: symphonia::core::codecs::video::VideoCodecId,
) -> Option<String> {
    use symphonia::core::codecs::video::well_known::*;

    let name = match codec {
        CODEC_ID_H264 => "h264",
        CODEC_ID_HEVC => "hevc",
        CODEC_ID_VP8 => "vp8",
        CODEC_ID_VP9 => "vp9",
        CODEC_ID_AV1 => "av1",
        CODEC_ID_MPEG2 => "mpeg2video",
        CODEC_ID_MPEG4 => "mpeg4",
        _ => return None,
    };
    Some(name.to_owned())
}

fn absorb_revision(revision: &MetadataRevision, probed: &mut ProbedMetadata) {
    for tag in &revision.media.tags {
        let key = match &tag.std {
            Some(standard) => {
                // A tag with a column of its own is stored there, not repeated
                // in the side table.
                if apply_standard_tag(standard, probed) {
                    continue;
                }
                standard_tag_name(standard)
            }
            None => tag.raw.key.clone(),
        };

        if OVERSIZED_TAGS.contains(&key.as_str()) {
            continue;
        }
        let Some(value) = raw_value_to_string(&tag.raw.value) else {
            continue;
        };
        if value.is_empty() || value.len() > MAX_TAG_VALUE_LEN {
            continue;
        }
        probed.extra_tags.push((key, value));
    }
}

/// Fill the promoted fields from a tag symphonia recognised.
///
/// Returns whether the tag has a column of its own, in which case it does not
/// also belong in the side table.
fn apply_standard_tag(tag: &StandardTag, probed: &mut ProbedMetadata) -> bool {
    match tag {
        StandardTag::TrackTitle(value) => probed.title = Some(value.to_string()),
        StandardTag::Artist(value) => probed.artist = Some(value.to_string()),
        StandardTag::Album(value) => probed.album = Some(value.to_string()),
        StandardTag::Genre(value) => probed.genre = Some(value.to_string()),
        StandardTag::AlbumArtist(value) => probed.album_artist = Some(value.to_string()),
        // A value too large to be a real track or disc number is a malformed
        // tag. Leaving the field alone keeps whatever an earlier revision of
        // the metadata got right, rather than clearing it.
        StandardTag::TrackNumber(value) => set_number(&mut probed.track_number, *value),
        StandardTag::TrackTotal(value) => set_number(&mut probed.tags.track_total, *value),
        StandardTag::DiscNumber(value) => set_number(&mut probed.tags.disc_number, *value),
        StandardTag::DiscTotal(value) => set_number(&mut probed.tags.disc_total, *value),
        StandardTag::Composer(value) => probed.tags.composer = Some(value.to_string()),
        StandardTag::Comment(value) => probed.tags.comment = Some(value.to_string()),
        StandardTag::Bpm(value) => set_number(&mut probed.tags.bpm, *value),
        StandardTag::CompilationFlag(value) => probed.tags.compilation = Some(*value),
        StandardTag::SortTrackTitle(value) => probed.tags.sort_title = Some(value.to_string()),
        StandardTag::SortArtist(value) => probed.tags.sort_artist = Some(value.to_string()),
        StandardTag::SortAlbum(value) => probed.tags.sort_album = Some(value.to_string()),
        StandardTag::MusicBrainzTrackId(value) => {
            probed.tags.musicbrainz_track_id = Some(value.to_string())
        }
        StandardTag::MusicBrainzAlbumId(value) => {
            probed.tags.musicbrainz_album_id = Some(value.to_string())
        }
        StandardTag::MusicBrainzArtistId(value) => {
            probed.tags.musicbrainz_artist_id = Some(value.to_string())
        }
        // Release date first, then the recording and original dates as
        // fallbacks, so a reissue still reports the year the browse tree groups
        // it under. Every dialect spells this differently: Vorbis comments use
        // DATE, ID3v2.4 uses TDRC, ID3v2.3 uses TYER, and RIFF uses ICRD.
        StandardTag::ReleaseDate(value) => set_date(probed, value, true),
        StandardTag::RecordingDate(value) | StandardTag::OriginalReleaseDate(value) => {
            set_date(probed, value, false)
        }
        StandardTag::ReleaseYear(value) => probed.year = Some(u32::from(*value)),
        StandardTag::RecordingYear(value)
        | StandardTag::OriginalReleaseYear(value)
        | StandardTag::OriginalRecordingYear(value) => {
            probed.year.get_or_insert(u32::from(*value));
        }
        // Everything else keeps its place in the side table.
        _ => return false,
    }
    true
}

/// Store a count, ignoring one that cannot be a real one.
fn set_number(field: &mut Option<u32>, value: u64) {
    if let Ok(value) = u32::try_from(value) {
        *field = Some(value);
    }
}

/// Record a date string, taking its leading year for the Years category.
fn set_date(probed: &mut ProbedMetadata, value: &str, authoritative: bool) {
    if authoritative || probed.tags.release_date.is_none() {
        probed.tags.release_date = Some(value.to_owned());
    }
    let year = value
        .trim()
        .get(..4)
        .filter(|prefix| prefix.chars().all(|c| c.is_ascii_digit()))
        .and_then(|prefix| prefix.parse::<u32>().ok());
    if let Some(year) = year {
        if authoritative {
            probed.year = Some(year);
        } else {
            probed.year.get_or_insert(year);
        }
    }
}

/// The variant name of a standard tag, used as its normalized key.
///
/// `StandardTag` is `#[non_exhaustive]` with around two hundred variants and no
/// accessor for its own name, so the name is taken from the `Debug` rendering,
/// which is `Variant(payload)`. Deriving it this way means new symphonia
/// variants get a sensible key without a match arm each.
fn standard_tag_name(tag: &StandardTag) -> String {
    let rendered = format!("{tag:?}");
    match rendered.find('(') {
        Some(index) => rendered[..index].to_owned(),
        None => rendered,
    }
}

fn raw_value_to_string(value: &RawValue) -> Option<String> {
    match value {
        RawValue::String(text) => Some(text.as_str().trim().to_owned()),
        RawValue::StringList(items) => Some(items.join("; ")),
        RawValue::UnsignedInt(number) => Some(number.to_string()),
        RawValue::SignedInt(number) => Some(number.to_string()),
        RawValue::Float(number) => Some(number.to_string()),
        RawValue::Boolean(flag) => Some(flag.to_string()),
        RawValue::Flag => Some("1".to_owned()),
        // Binary payloads are pictures and fingerprints, which belong nowhere
        // near a text index. `RawValue` is non-exhaustive, so anything symphonia
        // adds later is skipped until it is handled explicitly.
        RawValue::Binary(_) => None,
        _ => None,
    }
}

/// Parse metadata fields from a file path when tags are missing
pub(crate) fn fallback_parse_filename(media_file: &mut MediaFile) {
    if media_file.title.is_some() {
        return;
    }

    let filename_sans_ext = media_file
        .path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| media_file.filename.clone());

    if filename_sans_ext.contains(" - ") {
        let parts: Vec<&str> = filename_sans_ext.split(" - ").collect();
        if parts.len() >= 3 {
            let track_part = parts[0].trim();
            let mut track_num = None;
            let clean_track: String = track_part.chars().filter(|c| c.is_ascii_digit()).collect();
            if !clean_track.is_empty() {
                if let Ok(num) = clean_track.parse::<u32>() {
                    track_num = Some(num);
                }
            }

            if track_num.is_some() {
                if media_file.track_number.is_none() {
                    media_file.track_number = track_num;
                }
                if media_file.artist.is_none() {
                    media_file.artist = Some(parts[1].trim().to_string());
                }
                media_file.title = Some(parts[2..].join(" - ").trim().to_string());
            } else {
                if media_file.artist.is_none() {
                    media_file.artist = Some(parts[0].trim().to_string());
                }
                media_file.title = Some(parts[1..].join(" - ").trim().to_string());
            }
        } else if parts.len() == 2 {
            let part0 = parts[0].trim();
            let part1 = parts[1].trim();

            let clean_track: String = part0.chars().filter(|c| c.is_ascii_digit()).collect();
            if !clean_track.is_empty() && clean_track == part0 {
                if let Ok(num) = clean_track.parse::<u32>() {
                    if media_file.track_number.is_none() {
                        media_file.track_number = Some(num);
                    }
                }
                media_file.title = Some(part1.to_string());
            } else {
                let mut artist_name = part0;
                let mut track_num = None;
                if let Some(first_space) = part0.find(' ') {
                    let maybe_num = &part0[..first_space].trim_end_matches('.');
                    let clean: String = maybe_num.chars().filter(|c| c.is_ascii_digit()).collect();
                    if !clean.is_empty() && clean == *maybe_num {
                        if let Ok(num) = clean.parse::<u32>() {
                            track_num = Some(num);
                            artist_name = &part0[first_space + 1..];
                        }
                    }
                }

                if media_file.artist.is_none() {
                    media_file.artist = Some(artist_name.trim().to_string());
                }
                if media_file.track_number.is_none() && track_num.is_some() {
                    media_file.track_number = track_num;
                }
                media_file.title = Some(part1.to_string());
            }
        }
    } else {
        let mut title_part = filename_sans_ext.as_str();

        if let Some(first_space) = filename_sans_ext.find(' ') {
            let maybe_num = &filename_sans_ext[..first_space].trim_end_matches('.');
            let clean: String = maybe_num.chars().filter(|c| c.is_ascii_digit()).collect();
            if !clean.is_empty() && clean == *maybe_num {
                if let Ok(num) = clean.parse::<u32>() {
                    if media_file.track_number.is_none() {
                        media_file.track_number = Some(num);
                    }
                    title_part = &filename_sans_ext[first_space + 1..];
                }
            }
        }

        media_file.title = Some(title_part.trim().to_string());
    }
}

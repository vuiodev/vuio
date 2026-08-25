//! A film with an audio track the television cannot decode.
//!
//! The shape of the problem phase 4 exists for: `Movie.mkv` with AC-3 inside it,
//! where the picture plays and nothing comes out of the speakers. Everything
//! here drives the real router over a real Matroska file built by
//! `common::build_mkv`, so what is asserted is what a renderer receives.

#![cfg(all(feature = "transcode-ac3", feature = "casting"))]

mod common;

use common::{build_mkv, video_sample, Track, TrackKind, AVCC};
use std::sync::Arc;
use tower::ServiceExt;
use vuio_core::database::MediaRepository;

/// The vendored AC-3 conformance fixture: 48 kHz stereo, 440 Hz, 768-byte
/// frames of 1536 samples each.
const AC3: &[u8] = include_bytes!("../../vendor/oxideav-ac3/tests/fixtures/sine440_stereo.ac3");
const AC3_FRAME_LEN: usize = 768;
const AC3_FRAME_SAMPLES: u64 = 1536;
const AC3_FRAME_MS: f64 = AC3_FRAME_SAMPLES as f64 / 48.0;

/// Build a film: `AC3` looped to fill `seconds`, beside a 25 fps video track
/// with a keyframe every second.
pub fn film(seconds: f64) -> Vec<u8> {
    let frames: Vec<&[u8]> = AC3.chunks_exact(AC3_FRAME_LEN).collect();
    let audio_count = (seconds * 1000.0 / AC3_FRAME_MS).round() as usize;
    let audio_samples: Vec<(u64, Vec<u8>)> = (0..audio_count)
        .map(|i| {
            (
                (i as f64 * AC3_FRAME_MS).round() as u64,
                frames[i % frames.len()].to_vec(),
            )
        })
        .collect();

    let video_count = (seconds * 25.0).round() as usize;
    let video_samples: Vec<(u64, Vec<u8>)> = (0..video_count)
        .map(|i| {
            let keyframe = i % 25 == 0;
            (
                (i as f64 * 40.0).round() as u64,
                video_sample(keyframe, 96, i as u8),
            )
        })
        .collect();

    build_mkv(
        &[
            Track {
                number: 1,
                codec_id: "V_MPEG4/ISO/AVC",
                codec_private: AVCC.to_vec(),
                kind: TrackKind::Video {
                    width: 640,
                    height: 360,
                },
                samples: video_samples,
                all_keyframes: false,
                is_default: true,
                language: None,
            },
            Track {
                number: 2,
                codec_id: "A_AC3",
                codec_private: Vec::new(),
                kind: TrackKind::Audio {
                    sample_rate: 48_000.0,
                    channels: 2,
                },
                samples: audio_samples,
                all_keyframes: true,
                is_default: true,
                language: Some("eng"),
            },
        ],
        seconds * 1000.0,
    )
}

#[test]
fn the_fixture_really_is_a_matroska_file_with_an_ac3_track() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Film.mkv");
    std::fs::write(&path, film(12.0)).unwrap();

    let info = vuio_core::media::remux::MkvDemuxer::inspect(&path).expect("probe the fixture");
    let duration = info
        .duration_secs
        .expect("the container declares a duration");
    assert!(
        (duration - 12.0).abs() < 0.1,
        "duration came back as {duration}"
    );

    use vuio_core::media::remux::{TrackCodec, TrackKind as K};
    let video = info
        .tracks
        .iter()
        .find(|t| t.track_kind == K::Video)
        .expect("a video track");
    assert_eq!(video.codec_kind, TrackCodec::Avc);
    assert_eq!(
        video.extra_data, AVCC,
        "the avcC must survive the container unchanged"
    );

    let audio = info
        .tracks
        .iter()
        .find(|t| t.track_kind == K::Audio)
        .expect("an audio track");
    assert_eq!(
        audio.codec_kind,
        TrackCodec::Ac3,
        "AC-3 must be named, not lumped in with Unsupported"
    );
    assert_eq!(audio.sample_rate, Some(48_000));
}

// ── Step 1: what the scanner records ──────────────────────────────────────

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{header, Method, Request, StatusCode},
};
use vuio_core::web::{create_router, Surface};

/// A scanned film, and the server over it.
async fn scanned_film(seconds: f64) -> (tempfile::TempDir, vuio_core::state::AppState, i64) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("media");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("Film.mkv"), film(seconds)).unwrap();

    let state = common::state_over(temp.path(), &root).await;
    let files = common::scan_into(&state).await;
    let film = files
        .iter()
        .find(|f| f.filename == "Film.mkv")
        .expect("the scanner indexed the film");
    let id = film.id.unwrap();
    (temp, state, id)
}

/// The database prerequisite everything else in phase 4 rests on: a film's audio
/// codec has to be a column, not something learned by opening the file, because
/// a folder of four hundred films would otherwise open four hundred files to
/// render one Browse response.
#[tokio::test]
async fn a_scanned_film_records_the_codec_of_its_audio_track() {
    let (_temp, state, id) = scanned_film(6.0).await;
    let file = state
        .database
        .get_file_by_id(id)
        .await
        .unwrap()
        .expect("the film is in the database");

    assert_eq!(
        file.stream.codec.as_deref(),
        Some("ac3"),
        "symphonia identifies AC-3 perfectly well; it just cannot decode it,          which used to leave this NULL"
    );
    assert_eq!(file.stream.sample_rate, Some(48_000));
    assert!(file.mime_type.starts_with("video/"), "{}", file.mime_type);
}

// ── Step 3: the film's soundtrack, decoded ────────────────────────────────

async fn get(
    state: &vuio_core::state::AppState,
    uri: &str,
    method: Method,
    range: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo::<std::net::SocketAddr>(
            "127.0.0.1:50000".parse().unwrap(),
        ));
    if let Some(range) = range {
        builder = builder.header(header::RANGE, range);
    }
    let response = create_router(state.clone(), Surface::Primary)
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), 256 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    (status, headers, body)
}

#[tokio::test]
async fn a_films_soundtrack_is_served_as_playable_wav() {
    let (_temp, state, id) = scanned_film(6.0).await;
    let (status, headers, body) = get(
        &state,
        &format!("/media/{id}/transcode/audio.wav"),
        Method::GET,
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "audio/vnd.wave; codec=1");
    assert_eq!(&body[0..4], b"RIFF");
    assert_eq!(&body[8..12], b"WAVE");
    assert_eq!(u16::from_le_bytes([body[22], body[23]]), 2, "channels");
    assert_eq!(
        u32::from_le_bytes([body[24], body[25], body[26], body[27]]),
        48_000,
        "sample rate"
    );
    assert_eq!(
        body.len() as u64,
        headers[header::CONTENT_LENGTH]
            .to_str()
            .unwrap()
            .parse::<u64>()
            .unwrap(),
        "the body must be exactly as long as the header promised"
    );

    // Six seconds of 48 kHz stereo, within a frame either way.
    let payload = (body.len() - 44) as f64;
    let seconds = payload / (48_000.0 * 4.0);
    assert!(
        (seconds - 6.0).abs() < 0.1,
        "decoded {seconds:.3}s of audio from a six-second film"
    );

    // The fixture is a 440 Hz sine looped, so silence means the container path
    // produced a correctly-shaped empty response instead of decoding anything.
    let mut sum = 0f64;
    for c in body[44..].as_chunks::<2>().0 {
        let v = i16::from_le_bytes([c[0], c[1]]) as f64;
        sum += v * v;
    }
    let rms = (sum / ((body.len() - 44) as f64 / 2.0)).sqrt();
    assert!(rms > 100.0, "the decoded soundtrack is silent (rms {rms})");
}

#[tokio::test]
async fn a_films_soundtrack_is_byte_range_seekable() {
    let (_temp, state, id) = scanned_film(6.0).await;
    let uri = format!("/media/{id}/transcode/audio.wav");
    let (_, headers, whole) = get(&state, &uri, Method::GET, None).await;
    assert_eq!(headers[header::ACCEPT_RANGES], "bytes");

    // Two seconds in, deliberately not on a frame boundary.
    let start = 44 + 48_000 * 4 * 2 + 1234;
    let end = start + 40_000;
    let (status, headers, part) = get(
        &state,
        &uri,
        Method::GET,
        Some(&format!("bytes={start}-{end}")),
    )
    .await;

    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        headers[header::CONTENT_RANGE].to_str().unwrap(),
        format!("bytes {start}-{end}/{}", whole.len())
    );
    assert_eq!(part.len(), end - start + 1);

    // A container seek lands by timestamp rather than by byte, so the samples
    // are the same audio rather than the same bytes: compare energy, which a
    // seek to the wrong place would not preserve.
    let energy = |pcm: &[u8]| -> f64 {
        let samples = pcm.as_chunks::<2>().0;
        let sum: f64 = samples
            .iter()
            .map(|c| {
                let v = i16::from_le_bytes([c[0], c[1]]) as f64;
                v * v
            })
            .sum();
        (sum / samples.len() as f64).sqrt()
    };
    let here = energy(&part);
    let there = energy(&whole[start..=end]);
    assert!(
        here > 100.0 && (here - there).abs() / there < 0.15,
        "range RMS {here:.1} vs whole-decode RMS {there:.1} — the seek landed          somewhere else, or produced silence"
    );
}

#[tokio::test]
async fn head_promises_the_length_a_film_get_delivers() {
    let (_temp, state, id) = scanned_film(6.0).await;
    let uri = format!("/media/{id}/transcode/audio.wav");
    let (status, head_headers, head_body) = get(&state, &uri, Method::HEAD, None).await;
    let (_, get_headers, get_body) = get(&state, &uri, Method::GET, None).await;

    assert_eq!(status, StatusCode::OK);
    assert!(head_body.is_empty(), "HEAD carries no body");
    assert_eq!(
        head_headers[header::CONTENT_LENGTH],
        get_headers[header::CONTENT_LENGTH]
    );
    assert_eq!(
        get_body.len().to_string(),
        head_headers[header::CONTENT_LENGTH].to_str().unwrap()
    );
}

#[cfg(feature = "transcode-aac")]
#[tokio::test]
async fn a_films_soundtrack_is_also_available_as_aac() {
    let (_temp, state, id) = scanned_film(4.0).await;
    let (status, headers, body) = get(
        &state,
        &format!("/media/{id}/transcode/audio.aac"),
        Method::GET,
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "audio/aac");
    assert!(!body.is_empty());
    assert_eq!(body[0], 0xFF, "ADTS syncword high byte");
    assert_eq!(body[1] & 0xF0, 0xF0, "ADTS syncword low nibble");
}

// ── Step 4: the browser path ──────────────────────────────────────────────

/// Walk an ISO-BMFF byte string, yielding each top-level box's type and body.
fn boxes(data: &[u8]) -> Vec<(String, &[u8])> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let size = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        let name = String::from_utf8_lossy(&data[pos + 4..pos + 8]).into_owned();
        if size < 8 || pos + size > data.len() {
            break;
        }
        out.push((name, &data[pos + 8..pos + size]));
        pos += size;
    }
    out
}

/// Find the first box named `name` anywhere in `data`, descending into the
/// container boxes on the way.
fn find_box<'a>(data: &'a [u8], name: &str) -> Option<&'a [u8]> {
    const CONTAINERS: &[&str] = &[
        "moov", "trak", "mdia", "minf", "stbl", "stsd", "mvex", "moof", "traf", "avc1", "hvc1",
        "mp4a", "ac-3", "ec-3",
    ];
    for (found, body) in boxes(data) {
        if found == name {
            return Some(body);
        }
        if CONTAINERS.contains(&found.as_str()) {
            // `stsd` and the sample entries carry fixed preambles before their
            // children; skipping into them is what makes `avcC` reachable.
            let inner = match found.as_str() {
                "stsd" => &body[8..],
                "avc1" | "hvc1" => &body[78..],
                // Every audio sample entry shares the same 28-byte preamble
                // before its own configuration box.
                "mp4a" | "ac-3" | "ec-3" => &body[28..],
                _ => body,
            };
            if let Some(hit) = find_box(inner, name) {
                return Some(hit);
            }
        }
    }
    None
}

#[tokio::test]
async fn the_master_playlist_offers_the_films_ac3_track_as_a_rendition() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let (status, _, body) = get(
        &state,
        &format!("/media/{id}/hls/master.m3u8"),
        Method::GET,
        None,
    )
    .await;
    let playlist = String::from_utf8(body).unwrap();

    assert_eq!(status, StatusCode::OK);
    assert!(
        playlist.contains("#EXT-X-MEDIA:TYPE=AUDIO"),
        "the AC-3 track must be offered now that it can be decoded:\n{playlist}"
    );
    assert!(playlist.contains("audio/0/index.m3u8"), "{playlist}");
    assert!(
        playlist.contains("mp4a.40.2"),
        "the rendition arrives as AAC-LC whatever the source was:\n{playlist}"
    );
}

#[tokio::test]
async fn the_audio_init_segment_describes_aac_not_the_source_codec() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let (status, _, init) = get(
        &state,
        &format!("/media/{id}/hls/audio/0/init.mp4"),
        Method::GET,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let kinds: Vec<String> = boxes(&init).into_iter().map(|(name, _)| name).collect();
    assert_eq!(
        kinds,
        vec!["ftyp", "moov"],
        "an init segment is ftyp + moov"
    );

    assert!(
        find_box(&init, "mp4a").is_some(),
        "a browser initialises an AAC decoder from an mp4a entry, not an ac-3 one"
    );
    let esds = find_box(&init, "esds").expect("an esds carrying the AudioSpecificConfig");
    // The ES_Descriptor tree ends in a DecoderSpecificInfo (tag 0x05) holding
    // the raw config; the two bytes there must be the ones the encoder's own
    // ADTS headers declare, or the browser decodes at the wrong rate.
    let asc_at = esds
        .windows(2)
        .position(|w| w[0] == 0x05 && w[1] == 2)
        .expect("a two-byte DecoderSpecificInfo");
    let asc = &esds[asc_at + 2..asc_at + 4];
    assert_eq!(asc[0] >> 3, 2, "audioObjectType 2 is AAC-LC");
    assert_eq!(
        ((asc[0] & 0x07) << 1) | (asc[1] >> 7),
        3,
        "samplingFrequencyIndex 3 is 48 kHz"
    );
    assert_eq!((asc[1] >> 3) & 0x0F, 2, "two channels");
}

#[tokio::test]
async fn an_audio_segment_carries_real_re_encoded_aac() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let (status, _, segment) = get(
        &state,
        &format!("/media/{id}/hls/audio/0/segment/1"),
        Method::GET,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let kinds: Vec<String> = boxes(&segment).into_iter().map(|(name, _)| name).collect();
    assert_eq!(
        kinds,
        vec!["moof", "mdat"],
        "a media segment is moof + mdat"
    );

    let trun = find_box(&segment, "trun").expect("a trun");
    let sample_count = u32::from_be_bytes(trun[4..8].try_into().unwrap());
    assert!(
        sample_count > 150,
        "four seconds at 1024 samples a frame is ~187 frames, got {sample_count}"
    );

    let mdat = boxes(&segment)
        .into_iter()
        .find(|(name, _)| name == "mdat")
        .unwrap()
        .1;
    assert!(!mdat.is_empty(), "the segment carries samples");
    // The ADTS headers must be gone: an MP4 sample that begins with a syncword
    // is a header the decoder will read as spectral data.
    assert!(
        !(mdat[0] == 0xFF && mdat[1] & 0xF0 == 0xF0),
        "ADTS framing leaked into an MP4 sample"
    );

    // Not at its nominal four seconds: an AAC frame is 1024 samples, four
    // seconds of 48 kHz is 187.5 of them, and a segment opens on the film-wide
    // frame grid rather than half way through a frame.
    let tfdt = find_box(&segment, "tfdt").expect("a tfdt");
    let base = u64::from_be_bytes(tfdt[4..12].try_into().unwrap());
    assert_eq!(base, 188 * 1024, "the run opens on the frame grid");
}

/// The two defects a browser shows as playback stopping a few seconds in.
///
/// A player builds its whole timeline out of the playlist's `EXTINF` durations
/// and then fetches segments expecting to find exactly that. If a segment
/// overruns the next one's start the source buffer resolves the collision by
/// throwing samples away; if it covers something else entirely — which is what
/// rounding a segment's start forward to the next keyframe does on a film whose
/// keyframes are ten seconds apart — the buffer never reaches the playhead and
/// playback stops. So this asserts the one property that rules both out: the
/// segments tile the timeline the playlist described, exactly, with nothing
/// between them and nothing on top of each other.
#[tokio::test]
async fn the_segments_tile_the_timeline_the_playlist_promised() {
    const FRAME: u64 = 1024;
    const RATE: f64 = 48_000.0;
    let (_temp, state, id) = scanned_film(24.0).await;

    let (status, _, body) = get(
        &state,
        &format!("/media/{id}/hls/audio/0/index.m3u8"),
        Method::GET,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let playlist = String::from_utf8(body).expect("a playlist is text");
    let promised: Vec<f64> = playlist
        .lines()
        .filter_map(|line| line.strip_prefix("#EXTINF:"))
        .filter_map(|value| value.trim_end_matches(',').parse().ok())
        .collect();
    assert!(!promised.is_empty(), "no segments offered:\n{playlist}");

    let mut opens_at: Option<u64> = None;
    let mut first_open = 0u64;
    for seq in 0..promised.len() {
        let (status, _, segment) = get(
            &state,
            &format!("/media/{id}/hls/audio/0/segment/{seq}"),
            Method::GET,
            None,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "segment {seq} of {}",
            promised.len()
        );

        let tfdt = find_box(&segment, "tfdt").expect("a tfdt");
        let base = u64::from_be_bytes(tfdt[4..12].try_into().unwrap());
        let trun = find_box(&segment, "trun").expect("a trun");
        let samples = u64::from(u32::from_be_bytes(trun[4..8].try_into().unwrap()));

        match opens_at {
            None => first_open = base,
            Some(expected) => assert_eq!(
                base,
                expected,
                "segment {seq} opens at {base}, but segment {} ended at {expected}",
                seq - 1
            ),
        }
        opens_at = Some(base + samples * FRAME);
    }

    // And the timeline they cover is the one the playlist described, to within
    // the frame the AAC grid rounds by.
    let covered = opens_at.unwrap() - first_open;
    let promised_samples = (promised.iter().sum::<f64>() * RATE).round() as u64;
    assert!(
        covered.abs_diff(promised_samples) < FRAME,
        "the segments carry {covered} samples where the playlist promised {promised_samples}"
    );
}

#[tokio::test]
async fn a_video_segment_still_passes_the_picture_through_untouched() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let (_, _, init) = get(
        &state,
        &format!("/media/{id}/hls/video/init.mp4"),
        Method::GET,
        None,
    )
    .await;
    let avcc = find_box(&init, "avcC").expect("an avcC in the video init segment");
    assert_eq!(
        avcc, AVCC,
        "the decoder configuration must be the source's, byte for byte"
    );

    let (status, _, segment) = get(
        &state,
        &format!("/media/{id}/hls/video/segment/0"),
        Method::GET,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let trun = find_box(&segment, "trun").expect("a trun");
    let sample_count = u32::from_be_bytes(trun[4..8].try_into().unwrap());
    assert!(
        (95..=105).contains(&sample_count),
        "four seconds at 25 fps is ~100 frames, got {sample_count}"
    );
}

/// A rebuilt segment costs a decode; the same segment asked for twice must not.
#[tokio::test]
async fn a_segment_asked_for_twice_is_only_built_once() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let uri = format!("/media/{id}/hls/audio/0/segment/0");
    let (_, _, first) = get(&state, &uri, Method::GET, None).await;
    let (_, _, second) = get(&state, &uri, Method::GET, None).await;
    assert_eq!(first, second, "a cached segment must be the same bytes");
    assert!(!first.is_empty());
}

// ── Step 5: the film itself, remuxed ──────────────────────────────────────

async fn video_mp4(
    state: &vuio_core::state::AppState,
    id: i64,
    method: Method,
    time_seek: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut builder = Request::builder()
        .method(method)
        .uri(format!("/media/{id}/transcode/video.mp4"))
        .extension(ConnectInfo::<std::net::SocketAddr>(
            "127.0.0.1:50000".parse().unwrap(),
        ));
    if let Some(npt) = time_seek {
        builder = builder.header("TimeSeekRange.dlna.org", npt);
    }
    let response = create_router(state.clone(), Surface::Primary)
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), 256 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    (status, headers, body)
}

#[tokio::test]
async fn the_remuxed_film_is_a_parseable_fmp4_with_both_tracks() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let (status, headers, body) = video_mp4(&state, id, Method::GET, None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "video/mp4");
    let features = headers["contentFeatures.dlna.org"].to_str().unwrap();
    assert!(features.contains("DLNA.ORG_CI=1"), "{features}");
    assert!(
        features.contains("DLNA.ORG_OP=10"),
        "time seek yes, byte seek no — a byte offset into a stream produced on          demand does not name a fixed place in it: {features}"
    );
    assert_eq!(
        headers[header::ACCEPT_RANGES],
        "none",
        "stated rather than merely omitted: a client that probes with a range          request and gets a positional-looking answer concludes ranges work,          then seeks while parsing and splices two generations of the stream"
    );
    // The length is a promise the body is then made to keep exactly.
    let promised: usize = headers[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        body.len(),
        promised,
        "the body must be exactly as long as the response promised"
    );

    let top: Vec<String> = boxes(&body).into_iter().map(|(name, _)| name).collect();
    assert_eq!(&top[..2], &["ftyp", "moov"], "an init segment comes first");
    // Then moof/mdat pairs, and then however many `free` boxes it takes to
    // reach the length the response promised. The padding is what lets this
    // resource state a length at all, and a renderer skips it — mostly without
    // fetching it, since it stops at the duration the `moov` declared.
    let padding = top
        .iter()
        .position(|name| name == "free")
        .unwrap_or(top.len());
    assert!(
        top[padding..].iter().all(|name| name == "free"),
        "nothing may follow the padding: {top:?}"
    );
    let fragments = top[2..padding].chunks(2).collect::<Vec<_>>();
    assert!(
        fragments.len() >= 2,
        "expected several moof/mdat pairs, got {top:?}"
    );
    for pair in &fragments {
        assert_eq!(pair, &["moof", "mdat"], "in {top:?}");
    }
    assert!(padding < top.len(), "this film should be padded: {top:?}");

    // Two tracks, and the picture must arrive as the picture: the source's own
    // decoder configuration record, byte for byte. That is what "passthrough"
    // means, and the whole reason the CPU cost is bounded.
    let moov = boxes(&body)
        .into_iter()
        .find(|(name, _)| name == "moov")
        .unwrap()
        .1;
    let traks = boxes(moov).iter().filter(|(n, _)| n == "trak").count();
    assert_eq!(traks, 2, "one video track and one audio track");
    assert_eq!(
        find_box(&body, "avcC"),
        Some(AVCC),
        "the video track is copied, not re-encoded"
    );
    // AC-3 is handed over as AC-3: a television is what Dolby Digital was
    // built for, and a stereo AAC downmix would throw away the 5.1 it plays.
    assert!(
        find_box(&body, "ac-3").is_some(),
        "the AC-3 track must be passed through, not re-encoded"
    );
    assert_eq!(
        find_box(&body, "dac3").map(<[u8]>::len),
        Some(3),
        "an ac-3 sample entry is nothing without the record describing it"
    );
    assert!(
        find_box(&body, "mp4a").is_none(),
        "nothing here needed re-encoding"
    );
    // `mehd` is what a fragmented file carries its total duration in, and what a
    // renderer with no Content-Length draws a scrub bar from.
    let mehd = find_box(&body, "mehd").expect("a mehd declaring the film's length");
    let duration_ms = u64::from_be_bytes(mehd[4..12].try_into().unwrap());
    assert!(
        (7_900..=8_100).contains(&duration_ms),
        "mehd says {duration_ms} ms for an eight-second film"
    );
}

#[tokio::test]
async fn a_head_describes_the_film_without_decoding_it() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let (status, headers, body) = video_mp4(&state, id, Method::HEAD, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_empty());
    assert_eq!(headers[header::CONTENT_TYPE], "video/mp4");
    // A renderer probes with HEAD before it plays, and a resource with no
    // length — or worse, a length of zero — is one it mostly declines to draw a
    // scrub bar for. The promise made here is the one a GET then keeps.
    let promised = headers[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .to_string();
    assert_ne!(promised, "0", "HEAD must not describe an empty resource");
    let (_, get_headers, get_body) = video_mp4(&state, id, Method::GET, None).await;
    assert_eq!(get_headers[header::CONTENT_LENGTH], promised);
    assert_eq!(get_body.len().to_string(), promised);
    assert_eq!(
        headers["X-Content-Duration"]
            .to_str()
            .unwrap()
            .parse::<f64>()
            .unwrap()
            .round(),
        8.0
    );
}

/// The requirement that separates this from a stream you can only watch from the
/// beginning: a television must be able to scrub a film whose audio had to be
/// re-encoded, exactly as it can one whose audio it could already play.
#[tokio::test]
async fn a_time_seek_starts_the_film_where_it_was_asked_to() {
    let (_temp, state, id) = scanned_film(12.0).await;
    let (status, headers, body) = video_mp4(&state, id, Method::GET, Some("npt=6.000-")).await;

    assert_eq!(
        status,
        StatusCode::PARTIAL_CONTENT,
        "a time-seek request is answered as partial content"
    );
    let seek = headers["TimeSeekRange.dlna.org"].to_str().unwrap();
    assert!(
        seek.starts_with("npt=6.000-") && seek.ends_with("/12.000"),
        "the response must state the range it is answering: {seek}"
    );

    // A seek is a fresh stream: init segment, then fragments — and the
    // fragments carry the real timeline, so the renderer's position is right.
    let top: Vec<String> = boxes(&body).into_iter().map(|(name, _)| name).collect();
    assert_eq!(&top[..2], &["ftyp", "moov"]);

    let tfdt = find_box(&body, "tfdt").expect("a tfdt in the first fragment");
    let base = u64::from_be_bytes(tfdt[4..12].try_into().unwrap());
    let seconds = base as f64 / 90_000.0;
    assert!(
        (5.0..=6.1).contains(&seconds),
        "the first fragment starts at {seconds:.3}s — a seek to 6s must land on          the keyframe at or before it, never after"
    );

    // And it must be shorter than the whole film, or the seek did nothing.
    let (_, _, whole) = video_mp4(&state, id, Method::GET, None).await;
    assert!(
        body.len() < whole.len(),
        "seeking to the middle produced {} bytes against {} for the whole film",
        body.len(),
        whole.len()
    );
}

/// What an Android television does before it plays anything: read sixteen bytes
/// off the end of the file, looking for the `moov` a progressive MP4 carries at
/// its tail.
///
/// Answering that by producing the film would take a transcode slot and, on a
/// thirty-gigabyte remux, an enormous amount of work to hand back sixteen bytes.
/// Answering it with the whole film from the beginning — which is what ignoring
/// the range does — is a reply the set cannot make sense of, and it gives up
/// before playing a frame. So it is answered from the padding, which is the one
/// part of this resource whose contents are known without muxing anything.
#[tokio::test]
async fn the_tail_a_renderer_probes_is_answered_without_producing_the_film() {
    let (_temp, state, id) = scanned_film(8.0).await;

    // The length the resource commits to, which is what the renderer counts
    // back from.
    let (_, headers, whole) = video_mp4(&state, id, Method::GET, None).await;
    let promised: u64 = headers[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();

    let first = promised - 16;
    let response = create_router(state.clone(), Surface::Primary)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/media/{id}/transcode/video.mp4"))
                .header(header::RANGE, format!("bytes={first}-"))
                .extension(ConnectInfo::<std::net::SocketAddr>(
                    "127.0.0.1:50000".parse().unwrap(),
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        response.headers()[header::CONTENT_RANGE].to_str().unwrap(),
        format!("bytes {first}-{}/{promised}", promised - 1)
    );
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    assert_eq!(body.len(), 16);
    // And it is the truth rather than a convenient reply: these are the bytes a
    // whole download really ends with.
    assert_eq!(
        &body[..],
        &whole[whole.len() - 16..],
        "the padding answer must match what the stream actually ends with"
    );
}

/// The defect that made three rounds of seek fixes pointless: a byte offset into
/// this resource does not name a fixed place in it, because the bytes are
/// produced on demand and offset X is whatever the muxer emitted that time.
///
/// Answering a `Range` positionally therefore hands the client a fresh
/// `ftyp`/`moov` where it expected the continuation of what it was already
/// reading, and it decodes the join as noise. Worse, a client told
/// `Accept-Ranges: bytes` seeks while merely *parsing*, so straight playback
/// comes apart too — the stream is corrupt before anyone touches a scrub bar.
///
/// So a range is answered from the beginning, which HTTP explicitly allows, and
/// the refusal is advertised rather than left to be discovered.
#[tokio::test]
async fn a_byte_range_is_answered_from_the_beginning_rather_than_positionally() {
    let (_temp, state, id) = scanned_film(12.0).await;

    let response = create_router(state.clone(), Surface::Primary)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/media/{id}/transcode/video.mp4"))
                .header(header::RANGE, "bytes=100000-")
                .extension(ConnectInfo::<std::net::SocketAddr>(
                    "127.0.0.1:50000".parse().unwrap(),
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "a range this resource cannot honour must not be answered as though it          had been"
    );
    assert!(
        !response.headers().contains_key(header::CONTENT_RANGE),
        "no positional claim may be made about a stream produced on demand"
    );

    let body = axum::body::to_bytes(response.into_body(), 256 * 1024 * 1024)
        .await
        .unwrap();
    assert_eq!(&body[4..8], b"ftyp", "a whole stream, from its start");
    let tfdt = find_box(&body, "tfdt").expect("a tfdt in the first fragment");
    let seconds = u64::from_be_bytes(tfdt[4..12].try_into().unwrap()) as f64 / 90_000.0;
    assert!(
        seconds < 0.5,
        "the range was honoured positionally after all: started at {seconds:.3}s"
    );
}

/// A `206` is an answer to a range request. `?t=` is not one — it is how the
/// browser player and these tests name a starting point — so it gets a plain
/// `200`, and no `TimeSeekRange.dlna.org` stating a range nobody asked about.
#[tokio::test]
async fn only_a_range_request_is_answered_as_partial_content() {
    let (_temp, state, id) = scanned_film(12.0).await;

    let (status, headers, _) = video_mp4(&state, id, Method::GET, Some("npt=6.000-")).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert!(headers.contains_key("TimeSeekRange.dlna.org"));

    let response = create_router(state.clone(), Surface::Primary)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/media/{id}/transcode/video.mp4?t=6.0"))
                .extension(ConnectInfo::<std::net::SocketAddr>(
                    "127.0.0.1:50000".parse().unwrap(),
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        !response.headers().contains_key("TimeSeekRange.dlna.org"),
        "the header is a reply to a request that named a range"
    );

    // And the seek still happened — the point is the status, not the position.
    let body = axum::body::to_bytes(response.into_body(), 256 * 1024 * 1024)
        .await
        .unwrap();
    let tfdt = find_box(&body, "tfdt").expect("a tfdt in the first fragment");
    let seconds = u64::from_be_bytes(tfdt[4..12].try_into().unwrap()) as f64 / 90_000.0;
    assert!((5.0..=6.1).contains(&seconds), "started at {seconds:.3}s");
}

#[tokio::test]
async fn the_same_seek_expressed_as_a_clock_time_lands_in_the_same_place() {
    let (_temp, state, id) = scanned_film(12.0).await;
    let (_, _, decimal) = video_mp4(&state, id, Method::GET, Some("npt=6.000-")).await;
    let (_, _, clock) = video_mp4(&state, id, Method::GET, Some("npt=0:00:06.000-")).await;
    assert_eq!(
        decimal, clock,
        "npt=6.000 and npt=0:00:06.000 are the same instant"
    );
}

#[tokio::test]
async fn a_seek_past_the_end_is_clamped_rather_than_refused() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let (status, _, body) = video_mp4(&state, id, Method::GET, Some("npt=600.0-")).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    // Still a well-formed file, even if it carries almost nothing.
    let top: Vec<String> = boxes(&body).into_iter().map(|(name, _)| name).collect();
    assert_eq!(&top[..2], &["ftyp", "moov"]);
}

/// The reason every soundtrack is carried rather than one: a television switches
/// audio track inside its own demuxer, on bytes it already holds. No second
/// request is made, and nothing about the switch reaches this server — so a
/// track it can be switched to has to be in the body before it asks.
#[tokio::test]
async fn a_multi_audio_film_carries_every_soundtrack() {
    let (_temp, state, id) = multi_audio_state(6.0, 2).await;
    let (status, _, body) = video_mp4(&state, id, Method::GET, None).await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        audio_track_ids(&body),
        vec![2, 3, 4],
        "all three soundtracks must be in the container, not the one we guessed at"
    );
    let moov = find_moov(&body);
    let traks = boxes(moov).iter().filter(|(n, _)| n == "trak").count();
    assert_eq!(traks, 4, "one video track and three audio tracks");
}

/// The default leads, because a renderer that takes the first audio track
/// without asking must still get the one the container nominated.
#[tokio::test]
async fn the_default_soundtrack_is_written_first() {
    for (default_track, expected) in [
        (2u64, vec![2, 3, 4]),
        (3, vec![3, 2, 4]),
        (4, vec![4, 2, 3]),
    ] {
        let (_temp, state, id) = multi_audio_state(6.0, default_track).await;
        let (_, _, body) = video_mp4(&state, id, Method::GET, None).await;
        assert_eq!(
            audio_track_ids(&body),
            expected,
            "with track {default_track} marked default"
        );
    }
}

/// Audio tracks all in one alternate group, and only the first of them enabled.
/// A player reading a file whose audio tracks are all in group zero is entitled
/// to render them all at once — three soundtracks over each other — and three
/// tracks all claiming to be the default is a choice it then makes for itself.
#[tokio::test]
async fn the_soundtracks_are_declared_alternatives_of_each_other() {
    let (_temp, state, id) = multi_audio_state(6.0, 2).await;
    let (_, _, body) = video_mp4(&state, id, Method::GET, None).await;

    let headers: Vec<(&str, u8, u16)> = traks_by_kind(&body)
        .into_iter()
        .map(|(kind, trak)| {
            let tkhd = find_box(trak, "tkhd").expect("a tkhd");
            // version(1) + flags(3) + creation(4) + modification(4) + track_id(4)
            // + reserved(4) + duration(4) + reserved(8) + layer(2).
            (
                kind,
                tkhd[3],
                u16::from_be_bytes(tkhd[34..36].try_into().unwrap()),
            )
        })
        .collect();

    assert_eq!(
        headers,
        vec![
            ("video", 0x7, 0),
            ("audio", 0x7, 1),
            ("audio", 0x6, 1),
            ("audio", 0x6, 1),
        ],
        "(kind, tkhd flags, alternate_group) per track"
    );
}

/// What a television prints beside each entry in its audio menu. Without these
/// three soundtracks are three identical lines.
#[tokio::test]
async fn each_soundtrack_carries_its_language_and_a_label() {
    let (_temp, state, id) = multi_audio_state(6.0, 2).await;
    let (_, _, body) = video_mp4(&state, id, Method::GET, None).await;

    let mut seen = Vec::new();
    for (kind, trak) in traks_by_kind(&body) {
        if kind != "audio" {
            continue;
        }
        let mdhd = find_box(trak, "mdhd").expect("an mdhd");
        // version(1) + flags(3) + creation(4) + modification(4) + timescale(4)
        // + duration(4), then the packed language.
        let packed = u16::from_be_bytes(mdhd[20..22].try_into().unwrap());
        let unpack = |shift: u16| (((packed >> shift) & 0x1f) as u8 + 0x60) as char;
        let language: String = [unpack(10), unpack(5), unpack(0)].into_iter().collect();

        let hdlr = find_box(trak, "hdlr").expect("an hdlr");
        // version+flags(4) + pre_defined(4) + handler_type(4) + reserved(12).
        let name = String::from_utf8_lossy(&hdlr[24..])
            .trim_end_matches('\0')
            .to_string();
        seen.push((language, name));
    }

    assert_eq!(
        seen,
        vec![
            ("eng".to_string(), "AC-3 Stereo".to_string()),
            ("fra".to_string(), "AC-3 Stereo".to_string()),
            ("deu".to_string(), "AC-3 Stereo".to_string()),
        ],
        "each track must state the language it is in and what it arrived as"
    );
}

/// The one path that still carries a single track, for the browser player and
/// for narrowing down a report of a bad soundtrack.
#[tokio::test]
async fn an_explicit_audio_track_index_carries_that_one_alone() {
    let (_temp, state, id) = multi_audio_state(6.0, 2).await;
    let response = create_router(state.clone(), Surface::Primary)
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/media/{id}/transcode/video.mp4?audio_track=1"))
                .extension(ConnectInfo::<std::net::SocketAddr>(
                    "127.0.0.1:50000".parse().unwrap(),
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), 256 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();

    assert_eq!(
        audio_track_ids(&body),
        vec![3],
        "index 1 of the playable tracks is the film's second soundtrack"
    );
}

/// Present in the `moov` is not the same as audible. Each soundtrack runs off
/// its own decoder and its own encoder, and a routing mistake would leave two of
/// them declared in the init segment and empty for the length of the film —
/// which a television shows as three entries, two of them silent.
#[tokio::test]
async fn every_soundtrack_carries_samples_through_the_whole_film() {
    let (_temp, state, id) = multi_audio_state(6.0, 2).await;
    let (_, _, body) = video_mp4(&state, id, Method::GET, None).await;

    let mut samples: std::collections::BTreeMap<u32, u32> = Default::default();
    let mut fragments = 0;
    for (name, moof) in boxes(&body) {
        if name != "moof" {
            continue;
        }
        fragments += 1;
        for (name, traf) in boxes(moof) {
            if name != "traf" {
                continue;
            }
            let mut track_id = None;
            for (name, contents) in boxes(traf) {
                // tfhd: version(1) + flags(3), then track_ID.
                if name == "tfhd" {
                    track_id = Some(u32::from_be_bytes(contents[4..8].try_into().unwrap()));
                }
                // trun: version(1) + flags(3), then sample_count.
                if name == "trun" {
                    let count = u32::from_be_bytes(contents[4..8].try_into().unwrap());
                    *samples
                        .entry(track_id.expect("a tfhd before the trun"))
                        .or_default() += count;
                }
            }
        }
    }

    assert!(
        fragments >= 2,
        "expected several fragments, got {fragments}"
    );
    // These are AC-3 syncframes now, passed through rather than re-encoded:
    // 1536 samples each, so six seconds of 48 kHz is 187 or 188 of them, and
    // each track should carry very nearly all of them — the first fragment or
    // two are lost to the wait for the video's first keyframe.
    for track_id in [2u32, 3, 4] {
        let count = samples.get(&track_id).copied().unwrap_or(0);
        assert!(
            (170..=190).contains(&count),
            "track {track_id} carried {count} frames across the film: {samples:?}"
        );
    }
    let video = samples.get(&1).copied().unwrap_or(0);
    assert!(
        (140..=155).contains(&video),
        "the picture must still come through: {video} frames of a 25 fps six-second film"
    );
}

/// A film with several soundtracks, scanned and served.
async fn multi_audio_state(
    seconds: f64,
    default_track: u64,
) -> (tempfile::TempDir, vuio_core::state::AppState, i64) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("media");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("Film.mkv"),
        multi_audio_film(seconds, default_track),
    )
    .unwrap();

    let state = common::state_over(temp.path(), &root).await;
    let id = common::scan_into(&state)
        .await
        .iter()
        .find(|f| f.filename == "Film.mkv")
        .unwrap()
        .id
        .unwrap();
    (temp, state, id)
}

fn find_moov(body: &[u8]) -> &[u8] {
    boxes(body)
        .into_iter()
        .find(|(name, _)| name == "moov")
        .expect("a moov")
        .1
}

/// Each `trak` in the init segment, tagged by what it carries, in `moov` order.
fn traks_by_kind(body: &[u8]) -> Vec<(&'static str, &[u8])> {
    boxes(find_moov(body))
        .into_iter()
        .filter(|(name, _)| name == "trak")
        .map(|(_, trak)| {
            // From the `hdlr` handler type, not the sample entry: which sample
            // entry an audio track carries is the thing under test — `mp4a` for
            // one re-encoded, `ac-3` or `ec-3` for one passed through.
            let hdlr = find_box(trak, "hdlr").expect("a hdlr");
            // version+flags(4) + pre_defined(4), then the handler type.
            let kind = if &hdlr[8..12] == b"soun" {
                "audio"
            } else {
                "video"
            };
            (kind, trak)
        })
        .collect()
}

/// The `tkhd` track ids of the audio tracks, in the order they are written.
///
/// The output track keeps the source's track number — one track in, one track
/// out — so this says which of the film's soundtracks went where.
fn audio_track_ids(body: &[u8]) -> Vec<u32> {
    traks_by_kind(body)
        .into_iter()
        .filter(|(kind, _)| *kind == "audio")
        .map(|(_, trak)| {
            let tkhd = find_box(trak, "tkhd").expect("a tkhd");
            // version(1) + flags(3) + creation(4) + modification(4), then track_id.
            u32::from_be_bytes(tkhd[12..16].try_into().unwrap())
        })
        .collect()
}

/// A film with three soundtracks, one of which the container marks default.
///
/// Three rather than two, and in three languages, so that "the default leads"
/// and "the rest keep container order" are distinguishable assertions rather
/// than the same one.
fn multi_audio_film(seconds: f64, default_track: u64) -> Vec<u8> {
    let frames: Vec<&[u8]> = AC3.chunks_exact(AC3_FRAME_LEN).collect();
    let audio_count = (seconds * 1000.0 / AC3_FRAME_MS).round() as usize;
    let audio_samples = |offset: usize| -> Vec<(u64, Vec<u8>)> {
        (0..audio_count)
            .map(|i| {
                (
                    (i as f64 * AC3_FRAME_MS).round() as u64,
                    frames[(i + offset) % frames.len()].to_vec(),
                )
            })
            .collect()
    };
    let video_count = (seconds * 25.0).round() as usize;
    let video_samples: Vec<(u64, Vec<u8>)> = (0..video_count)
        .map(|i| {
            (
                (i as f64 * 40.0).round() as u64,
                video_sample(i % 25 == 0, 96, i as u8),
            )
        })
        .collect();

    let audio = |number: u64, offset: usize, language: &'static str| Track {
        number,
        codec_id: "A_AC3",
        codec_private: Vec::new(),
        kind: TrackKind::Audio {
            sample_rate: 48_000.0,
            channels: 2,
        },
        samples: audio_samples(offset),
        all_keyframes: true,
        is_default: number == default_track,
        language: Some(language),
    };

    build_mkv(
        &[
            Track {
                number: 1,
                codec_id: "V_MPEG4/ISO/AVC",
                codec_private: AVCC.to_vec(),
                kind: TrackKind::Video {
                    width: 640,
                    height: 360,
                },
                samples: video_samples,
                all_keyframes: false,
                is_default: true,
                language: None,
            },
            audio(2, 0, "eng"),
            audio(3, 1, "fra"),
            audio(4, 2, "deu"),
        ],
        seconds * 1000.0,
    )
}

#[tokio::test]
async fn a_film_advertises_the_remuxed_film_and_not_its_soundtrack() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let didl = browse_didl(&state).await;

    assert!(
        didl.contains(&format!("/media/{id}/transcode/video.ts")),
        "a film's alternative is the film:\n{didl}"
    );
    assert!(
        !didl.contains("transcode/audio.wav"),
        "offering a film's soundtrack in place of the film would lose the picture:\n{didl}"
    );
    assert!(
        didl.contains("DLNA.ORG_OP=11;DLNA.ORG_CI=1"),
        "a transport stream resynchronises wherever it is joined, so both seek          modes are honest here:\n{didl}"
    );
    // The original stays, and stays first by default, so a television that can
    // decode AC-3 keeps its byte-seekable direct-play resource.
    let original = didl
        .find(&format!("/media/{id}</res>"))
        .unwrap_or_else(|| panic!("no direct-play resource in:\n{didl}"));
    let transcoded = didl.find("transcode/video.ts").unwrap();
    assert!(
        original < transcoded,
        "the original is listed first:\n{didl}"
    );
}

/// Needing an alternative and being able to produce one are different
/// questions, and this is the one the *video* track answers. A film whose
/// picture cannot be copied through gets no second resource: advertising one
/// and then answering 404 is worse than the silence it was meant to fix.
#[tokio::test]
async fn a_film_whose_picture_cannot_be_copied_is_not_offered_an_alternative() {
    let (_temp, state, id) = scanned_film(4.0).await;

    // Rewrite the recorded video codec to one the fMP4 writer cannot describe.
    // The audio is untouched, so this is a film that still *needs* a decoded
    // alternative and simply cannot be given one.
    let mut file = state.database.get_file_by_id(id).await.unwrap().unwrap();
    assert_eq!(file.stream.video_codec.as_deref(), Some("h264"));
    file.stream.video_codec = Some("vp9".into());
    state
        .database
        .bulk_update_media_files(&[file])
        .await
        .unwrap();

    let didl = browse_didl(&state).await;
    assert!(
        !didl.contains("transcode/"),
        "a VP9 picture cannot be passed through, so nothing may be advertised:\n{didl}"
    );
}

#[tokio::test]
async fn a_scanned_film_records_the_codec_of_its_video_track_too() {
    let (_temp, state, id) = scanned_film(4.0).await;
    let file = state.database.get_file_by_id(id).await.unwrap().unwrap();
    assert_eq!(file.stream.video_codec.as_deref(), Some("h264"));
    assert_eq!(
        file.stream.codec.as_deref(),
        Some("ac3"),
        "the audio codec is the one that decides an alternative is needed"
    );
}

#[tokio::test]
async fn with_the_feature_off_a_film_has_exactly_one_resource() {
    let (_temp, mut state, _) = scanned_film(4.0).await;
    let mut config = (*state.config).clone();
    config.transcode.enabled = false;
    let config = Arc::new(config);
    state.config = config.clone();
    state.live_config = Arc::new(vuio_core::state::LiveConfig::new(config));

    let didl = browse_didl(&state).await;
    assert!(!didl.contains("transcode/"), "{didl}");
}

/// `mode = "forced"` must still offer the decoded resource for a film whose
/// audio is not DTS.
///
/// The forcing rule is about whether to *hide* the original, and only DTS earns
/// that — a television that cannot license DTS certainly has no decoder for it,
/// where AC-3 is common enough that hiding the original would take away a
/// working choice. Gating the decoded resource itself on the same test left an
/// AC-3 film in forced mode with no decoded resource at all, which is the exact
/// case this whole path exists for.
#[tokio::test]
async fn a_forced_film_still_offers_the_transcode_when_its_audio_is_not_dts() {
    let (_temp, mut state, _) = scanned_film(4.0).await;
    let mut config = (*state.config).clone();
    config.transcode.mode = vuio_core::config::TranscodeMode::Forced;
    let config = Arc::new(config);
    state.config = config.clone();
    state.live_config = Arc::new(vuio_core::state::LiveConfig::new(config));

    let didl = browse_didl(&state).await;
    assert!(
        didl.contains("transcode/video.ts"),
        "an AC-3 film in forced mode must still be offered the remux:\n{didl}"
    );
    // And the original stays beside it, because AC-3 is not DTS.
    assert!(
        didl.contains("video/x-matroska") || didl.contains("video/x-mkv"),
        "the original must not be hidden for AC-3:\n{didl}"
    );
    // Forced means the decoded resource leads. `DLNA.ORG_CI` is what separates
    // them: 1 is converted, 0 is the file as it is stored.
    let converted = didl.find("DLNA.ORG_CI=1").expect("the decoded resource");
    let stored = didl.find("DLNA.ORG_CI=0").expect("the original");
    assert!(
        converted < stored,
        "the forced resource must be listed first:\n{didl}"
    );
}

/// Browse the root folder as a television would, and return the DIDL.
async fn browse_didl(state: &vuio_core::state::AppState) -> String {
    let body = r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
<s:Body><u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
<ObjectID>video</ObjectID><BrowseFlag>BrowseDirectChildren</BrowseFlag>
<Filter>*</Filter><StartingIndex>0</StartingIndex><RequestedCount>50</RequestedCount>
<SortCriteria></SortCriteria></u:Browse></s:Body></s:Envelope>"#;
    let request = Request::builder()
        .method(Method::POST)
        .uri("/control/ContentDirectory")
        .header(header::CONTENT_TYPE, "text/xml")
        .header(
            "SOAPAction",
            "\"urn:schemas-upnp-org:service:ContentDirectory:1#Browse\"",
        )
        .header(header::USER_AGENT, "SEC_HHP_Samsung TV")
        .extension(ConnectInfo::<std::net::SocketAddr>(
            "127.0.0.1:50000".parse().unwrap(),
        ))
        .body(Body::from(body))
        .unwrap();
    let response = create_router(state.clone(), Surface::Primary)
        .oneshot(request)
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 8 * 1024 * 1024)
        .await
        .unwrap();
    String::from_utf8_lossy(&bytes)
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
}

/// What the video probe adds to a scan.
///
/// Ignored by default: it is a measurement, not an assertion, and the number it
/// prints belongs in a commit message rather than in a pass/fail. Run with
/// `cargo test -- --ignored measure_probe_cost --nocapture`.
#[test]
#[ignore]
fn measure_probe_cost() {
    use std::time::Instant;

    let dir = tempfile::tempdir().unwrap();
    let short = dir.path().join("Short.mkv");
    std::fs::write(&short, film(12.0)).unwrap();
    let long = dir.path().join("Long.mkv");
    std::fs::write(&long, film(7200.0)).unwrap();

    for (name, path) in [("12s", &short), ("2h", &long)] {
        let size = std::fs::metadata(path).unwrap().len();
        // Warm the page cache first, so this measures parsing rather than the
        // first read of a cold file.
        let _ = vuio_core::media::remux::MkvDemuxer::inspect(path);
        let started = Instant::now();
        const RUNS: u32 = 100;
        for _ in 0..RUNS {
            let _ = vuio_core::media::remux::MkvDemuxer::inspect(path);
        }
        let each = started.elapsed() / RUNS;
        eprintln!("{name} film ({size} bytes): {each:?} per probe");
    }
}

/// Writes the fixture where it can be inspected by hand. Ignored by default —
/// it exists so `cargo test -- --ignored dump_fixture` produces a file to open
/// in ffprobe or mpv when this writer is being changed.
#[test]
#[ignore]
fn dump_fixture() {
    let out = std::env::var("VUIO_FIXTURE_OUT").unwrap_or_else(|_| "/tmp/vuio-film.mkv".into());
    std::fs::write(&out, film(12.0)).unwrap();
    eprintln!("wrote {out}");
}

/// Not a test: prints what this server would commit to for a film on disk, so a
/// real library can be checked without a television in the room.
/// `VUIO_FILM=/path/to/Film.mkv cargo test --all-features describe_a_real_film -- --ignored --nocapture`
#[test]
#[ignore]
fn describe_a_real_film() {
    use vuio_core::media::remux::{
        browser_video_track, television_audio_tracks, MkvDemuxer, TrackKind,
    };
    use vuio_core::media::transcode::{audio_disposition, measure_track_rates, promised_ts_length};

    let path = std::env::var("VUIO_FILM").expect("set VUIO_FILM to the film to describe");
    let path = std::path::PathBuf::from(path);
    let size = std::fs::metadata(&path).unwrap().len();

    let probed = std::time::Instant::now();
    let info = MkvDemuxer::inspect(&path).expect("inspect the film");
    let duration = info.duration_secs.unwrap_or(0.0);
    eprintln!(
        "{}\n  {size} bytes, {duration:.1}s, inspected in {:.2}s",
        path.display(),
        probed.elapsed().as_secs_f64()
    );

    let video = browser_video_track(&info.tracks);
    match video {
        Some(track) => eprintln!("  video: {} ({:?})", track.codec, track.codec_kind),
        None => eprintln!("  video: NONE this build can copy through — no .ts is offered"),
    }

    let measured = std::time::Instant::now();
    let rates = measure_track_rates(&path, &info.tracks).expect("measure the film");
    eprintln!("  measured in {:.2}s", measured.elapsed().as_secs_f64());

    for track in &info.tracks {
        let rate = rates.get(track.id);
        let shape = match rate {
            Some(rate) => format!(
                "{:.0} kbps, {:.2} fps",
                rate.bits_per_second as f64 / 1000.0,
                rate.frames_per_second
            ),
            None => "not measured".to_string(),
        };
        let role = if track.track_kind == TrackKind::Audio {
            format!("{:?}", audio_disposition(track))
        } else {
            format!("{:?}", track.track_kind)
        };
        eprintln!(
            "  track {} {:?} {}ch lang={} — {role}, {shape}",
            track.id,
            track.codec_kind,
            track.channels.unwrap_or(0),
            track.language.as_deref().unwrap_or("und")
        );
    }

    let carried: Vec<_> = television_audio_tracks(&info.tracks)
        .into_iter()
        .cloned()
        .collect();
    let promised = promised_ts_length(size, duration, &info.tracks, &carried, &rates);
    let old = size + (size / 16);
    eprintln!(
        "  promised {promised} bytes ({:.1}% of source, {:.2} Mbps)\n  \
         the source-sized promise would have been {old} ({:.1}%), so a byte offset \
         would have named a moment {:.2}x too far along",
        promised as f64 * 100.0 / size as f64,
        promised as f64 * 8.0 / duration / 1e6,
        old as f64 * 100.0 / size as f64,
        old as f64 / promised as f64,
    );
}

/// Not a test: writes a DTS film out so a real server can be pointed at it.
/// `cargo test --all-features dump_dts_fixture -- --ignored`
#[cfg(feature = "transcode-dts")]
#[test]
#[ignore]
fn dump_dts_fixture() {
    let out = std::env::var("VUIO_FIXTURE_OUT").unwrap_or_else(|_| "/tmp/vuio-dts.mkv".into());
    let seconds: f64 = std::env::var("VUIO_FIXTURE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60.0);
    std::fs::write(&out, dts_film(seconds)).unwrap();
    eprintln!("wrote {out}");
}

#[test]
#[ignore]
fn dump_long_fixture() {
    let out = std::env::var("VUIO_FIXTURE_OUT").unwrap_or_else(|_| "/tmp/vuio-long.mkv".into());
    std::fs::write(&out, film(7200.0)).unwrap();
    eprintln!("wrote {out}");
}

/// Not a test: writes the multi-audio remux out so an external demuxer can be
/// pointed at it. `cargo test --all-features dump_multi_audio -- --ignored`
#[tokio::test]
#[ignore]
async fn dump_multi_audio() {
    let (_temp, state, id) = multi_audio_state(6.0, 3).await;
    let (_, _, body) = video_mp4(&state, id, Method::GET, None).await;
    let out = std::env::var("VUIO_DUMP").unwrap_or_else(|_| "/tmp/multi_audio.mp4".into());
    std::fs::write(&out, &body).unwrap();
    eprintln!("wrote {} bytes to {out}", body.len());
}

// ── Step 6: the transport stream a television can seek ────────────────────

async fn video_ts(
    state: &vuio_core::state::AppState,
    id: i64,
    query: &str,
    range: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut builder = Request::builder()
        .method(Method::GET)
        .uri(format!("/media/{id}/transcode/video.ts{query}"))
        .extension(ConnectInfo::<std::net::SocketAddr>(
            "127.0.0.1:50000".parse().unwrap(),
        ));
    if let Some(range) = range {
        builder = builder.header(header::RANGE, range);
    }
    let response = create_router(state.clone(), Surface::Primary)
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), 256 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    (status, headers, body)
}

const TS_PACKET: usize = 188;

/// Every packet in a transport stream begins with a sync byte, and that is the
/// whole reason a decoder can join one part way through.
fn assert_well_formed_ts(body: &[u8]) {
    assert_eq!(body.len() % TS_PACKET, 0, "packets must tile the stream");
    for (index, packet) in body.chunks(TS_PACKET).enumerate() {
        assert_eq!(packet[0], 0x47, "packet {index} has no sync byte");
    }
}

/// PIDs that carry a payload, in order of first appearance.
fn pids(body: &[u8]) -> Vec<u16> {
    let mut seen = Vec::new();
    for packet in body.chunks(TS_PACKET) {
        let pid = (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]);
        if !seen.contains(&pid) {
            seen.push(pid);
        }
    }
    seen
}

#[tokio::test]
async fn a_film_is_served_as_a_well_formed_transport_stream() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let (status, headers, body) = video_ts(&state, id, "", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "video/mpeg");
    assert_eq!(headers[header::ACCEPT_RANGES], "bytes");
    let features = headers["contentFeatures.dlna.org"].to_str().unwrap();
    assert!(
        features.contains("DLNA.ORG_OP=11"),
        "a transport stream resynchronises wherever it is joined, so byte          seeking is an honest claim here: {features}"
    );
    assert_well_formed_ts(&body);

    // The tables come first, because a decoder that joined here has to be told
    // what the programme contains before anything else means anything.
    let seen = pids(&body);
    assert_eq!(seen[0], 0x0000, "the programme association table leads");
    assert_eq!(seen[1], 0x1000, "then the programme map");
    assert!(
        seen.contains(&0x0100) && seen.contains(&0x0101),
        "a video stream and a soundtrack: {seen:?}"
    );

    // And the body is exactly the length it promised.
    let promised: usize = headers[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(body.len(), promised);
}

/// The tables are repeated rather than written once, which is the difference
/// between a stream that can be joined and one that cannot.
#[tokio::test]
async fn the_programme_tables_repeat_throughout_the_stream() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let (_, _, body) = video_ts(&state, id, "", None).await;

    let pat_packets = body
        .chunks(TS_PACKET)
        .filter(|packet| (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]) == 0x0000)
        .count();
    assert!(
        pat_packets >= 4,
        "an eight-second film should carry the tables several times over, not          once at the front: found {pat_packets}"
    );
}

/// What the whole transport stream exists for: a byte offset is a usable seek.
#[tokio::test]
async fn a_byte_offset_seeks_to_that_fraction_of_the_film() {
    let (_temp, state, id) = scanned_film(12.0).await;
    let (_, headers, whole) = video_ts(&state, id, "", None).await;
    let promised: u64 = headers[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();

    let half = promised / 2;
    let (status, headers, body) = video_ts(&state, id, "", Some(&format!("bytes={half}-"))).await;

    // Whole packets, so a range whose first byte falls inside one ends a little
    // short of the promise rather than delivering a fragment of a packet. HTTP
    // lets a server answer an open-ended range with less of it than was asked
    // for, and a decoder handed 40 bytes of a packet can do nothing with them.
    let expected = ((promised - half) / TS_PACKET as u64) * TS_PACKET as u64;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        headers[header::CONTENT_RANGE].to_str().unwrap(),
        format!("bytes {half}-{}/{promised}", half + expected - 1)
    );
    assert_eq!(body.len() as u64, expected, "exactly what was promised");
    assert_well_formed_ts(&body);
    // The tables lead this response too — without them a decoder joining here
    // would have nothing to interpret the packets against.
    assert_eq!(pids(&body)[0], 0x0000);
    assert!(
        body.len() < whole.len(),
        "seeking to the middle produced as much as the whole film"
    );
}

#[tokio::test]
async fn a_head_describes_the_transport_stream_without_producing_it() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let response = create_router(state.clone(), Surface::Primary)
        .oneshot(
            Request::builder()
                .method(Method::HEAD)
                .uri(format!("/media/{id}/transcode/video.ts"))
                .extension(ConnectInfo::<std::net::SocketAddr>(
                    "127.0.0.1:50000".parse().unwrap(),
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "video/mpeg");
    let promised = response.headers()[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .to_string();
    assert_ne!(promised, "0");

    let (_, get_headers, get_body) = video_ts(&state, id, "", None).await;
    assert_eq!(get_headers[header::CONTENT_LENGTH], promised);
    assert_eq!(get_body.len().to_string(), promised);
}

/// A client sizing the file up reads its last handful of bytes — fewer than one
/// packet, so there is no whole packet left to produce. Refusing that as
/// unsatisfiable is reported as a transfer error rather than shrugged off, and
/// the film never starts.
#[tokio::test]
async fn a_range_inside_the_final_packet_is_answered_from_it() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let (_, headers, whole) = video_ts(&state, id, "", None).await;
    let promised: u64 = headers[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();

    // Forty bytes into the last packet, which is where a reader counting back
    // from the end lands.
    let first = promised - 148;
    let (status, headers, body) = video_ts(&state, id, "", Some(&format!("bytes={first}-"))).await;

    assert_eq!(status, StatusCode::PARTIAL_CONTENT, "not 416");
    assert_eq!(body.len(), 148);
    assert_eq!(
        headers[header::CONTENT_RANGE].to_str().unwrap(),
        format!("bytes {first}-{}/{promised}", promised - 1)
    );
    // And it is the truth: these are the bytes a complete read really ends with.
    assert_eq!(
        &body[..],
        &whole[whole.len() - 148..],
        "the tail answer must match what the stream actually ends with"
    );
}

/// The `video.mp4` next door stays, because the browser player fetches whole
/// responses and never needs a byte offset — it is only a television that does.
#[tokio::test]
async fn the_mp4_resource_remains_beside_the_transport_stream() {
    let (_temp, state, id) = scanned_film(4.0).await;
    let (status, headers, _) = video_mp4(&state, id, Method::GET, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "video/mp4");

    let (status, headers, _) = video_ts(&state, id, "", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "video/mpeg");
}

/// The failure that stopped a film playing at all, and the least obvious one.
///
/// A television opens three connections in under a second: one to play on, one
/// to read the end of the file, and one to play on again. Answering the middle
/// one by muxing means seeking into a thirty-gigabyte film and holding a
/// transcode slot for as long as that takes — and with two slots, the request
/// the set actually wanted to play on is refused outright.
///
/// A probe produces nothing, so it should cost nothing.
#[tokio::test]
async fn a_tail_probe_is_answered_even_with_every_transcode_slot_taken() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let (_, headers, whole) = video_ts(&state, id, "", None).await;
    let promised: u64 = headers[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();

    // Every slot busy, as it is when a set has connections already open.
    let mut held = Vec::new();
    while let Some(permit) = state.transcode.try_acquire() {
        held.push(permit);
    }
    assert!(!held.is_empty(), "there is at least one slot to exhaust");

    // Producing the film is refused, which is the point of the limit.
    let (status, _, _) = video_ts(&state, id, "", None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);

    // Reading its tail is not, because that produces nothing.
    let first = promised - 4096;
    let (status, headers, body) = video_ts(&state, id, "", Some(&format!("bytes={first}-"))).await;
    assert_eq!(
        status,
        StatusCode::PARTIAL_CONTENT,
        "a probe must not queue behind the films being played"
    );
    assert_eq!(body.len(), 4096);
    assert_eq!(
        headers[header::CONTENT_RANGE].to_str().unwrap(),
        format!("bytes {first}-{}/{promised}", promised - 1)
    );
    // And it is still the truth, aligned the way a full read would deliver it.
    assert_eq!(
        &body[..],
        &whole[whole.len() - 4096..],
        "the padding answer must match what the stream actually ends with"
    );
}

/// The elementary-stream PIDs the programme map declares.
///
/// Parsed rather than inferred from what appears in the body, because the point
/// of the assertions below is the difference between the two: a PID a renderer
/// is told to expect audio on and never receives any is a renderer that waits
/// forever.
fn declared_pids(body: &[u8]) -> Vec<u16> {
    let pmt = body
        .chunks(TS_PACKET)
        .find(|packet| (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]) == 0x1000)
        .expect("the stream carries a programme map");
    // header(4) pointer_field(1), then the section.
    let section = &pmt[5..];
    assert_eq!(section[0], 0x02, "table_id 2 is the programme map");
    let length = usize::from(u16::from_be_bytes([section[1] & 0x0F, section[2]]));
    // table_id_extension(2) version(1) section(2) pcr_pid(2) program_info(2)
    let info_length = usize::from(u16::from_be_bytes([section[10] & 0x0F, section[11]]));
    let mut at = 12 + info_length;
    // The section runs to `length` past the third byte, less its four-byte CRC.
    let end = 3 + length - 4;
    let mut pids = Vec::new();
    while at + 5 <= end {
        pids.push(u16::from_be_bytes([
            section[at + 1] & 0x1F,
            section[at + 2],
        ]));
        let descriptors = usize::from(u16::from_be_bytes([
            section[at + 3] & 0x0F,
            section[at + 4],
        ]));
        at += 5 + descriptors;
    }
    pids
}

/// Where one packet's payload begins, and the programme clock it carries.
fn payload_and_pcr(packet: &[u8]) -> (usize, Option<u64>) {
    let control = (packet[3] >> 4) & 0b11;
    if control & 0b10 == 0 {
        return (4, None);
    }
    let length = usize::from(packet[4]);
    let payload = 5 + length;
    if length == 0 {
        return (payload, None);
    }
    if packet[5] & 0x10 == 0 {
        return (payload, None);
    }
    let base = (u64::from(packet[6]) << 25)
        | (u64::from(packet[7]) << 17)
        | (u64::from(packet[8]) << 9)
        | (u64::from(packet[9]) << 1)
        | u64::from(packet[10] >> 7);
    (payload, Some(base))
}

/// The presentation timestamp in a PES header starting at `payload`.
fn pes_pts(payload: &[u8]) -> Option<u64> {
    if payload.get(..3)? != [0x00, 0x00, 0x01] {
        return None;
    }
    if payload[7] >> 6 == 0 {
        return None;
    }
    let stamp = payload.get(9..14)?;
    Some(
        (u64::from(stamp[0] & 0x0E) << 29)
            | (u64::from(stamp[1]) << 22)
            | (u64::from(stamp[2] & 0xFE) << 14)
            | (u64::from(stamp[3]) << 7)
            | (u64::from(stamp[4]) >> 1),
    )
}

/// A decoder starts its clock from the PCR and shows each picture when that
/// clock reaches the picture's PTS. Write the two equal and every frame is due
/// the instant it arrives, so there is no buffer at all and the first jitter
/// starves the renderer — which after a seek, where the decoder starts from
/// nothing, is exactly when it can least afford it.
#[tokio::test]
async fn the_presentation_clock_runs_ahead_of_the_programme_clock() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let (_, _, body) = video_ts(&state, id, "", None).await;

    let mut checked = 0;
    for packet in body.chunks(TS_PACKET) {
        let pid = (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]);
        if pid != 0x0100 || packet[1] & 0x40 == 0 {
            continue;
        }
        let (payload, Some(pcr)) = payload_and_pcr(packet) else {
            continue;
        };
        let Some(pts) = pes_pts(&packet[payload..]) else {
            continue;
        };
        assert!(
            pts > pcr,
            "a picture due at {pts} was clocked as arriving at {pcr}, so the renderer \
             has no buffer to fill"
        );
        // Half a second of ninety-kilohertz ticks, and not so much more that the
        // film takes noticeably longer to start.
        assert_eq!(pts - pcr, 45_000, "the headroom is a fixed half second");
        checked += 1;
    }
    assert!(checked > 20, "only {checked} pictures carried a clock");
}

/// A decoder that joins the stream anywhere can interpret nothing until it has
/// seen a PAT and a PMT, so the wait for the next pair is the floor on how long
/// a seek takes to produce a picture.
#[tokio::test]
async fn the_tables_repeat_often_enough_to_join_the_stream_quickly() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let (_, _, body) = video_ts(&state, id, "", None).await;

    let pmts = body
        .chunks(TS_PACKET)
        .filter(|packet| (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]) == 0x1000)
        .count();
    // Ten a second is what the broadcast profiles ask for; eight seconds of film
    // should therefore carry something near eighty, and certainly not eight.
    assert!(
        pmts >= 60,
        "{pmts} programme maps in eight seconds of film is a decoder waiting a \
         second to learn what it has joined"
    );
}

/// A transport stream marks access unit boundaries with a delimiter and nothing
/// else — a container's own frame boundaries do not survive the conversion — so
/// every muxer in the field writes one, and a decoder that relies on it shows
/// nothing at all without it.
#[tokio::test]
async fn every_picture_opens_with_an_access_unit_delimiter() {
    let (_temp, state, id) = scanned_film(4.0).await;
    let (_, _, body) = video_ts(&state, id, "", None).await;

    let mut checked = 0;
    for packet in body.chunks(TS_PACKET) {
        let pid = (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]);
        if pid != 0x0100 || packet[1] & 0x40 == 0 {
            continue;
        }
        let (payload, _) = payload_and_pcr(packet);
        let pes = &packet[payload..];
        // start codes(3) stream_id(1) length(2) flags(2) header_len(1), then the
        // timestamps the header declared.
        let unit = &pes[9 + usize::from(pes[8])..];
        assert_eq!(
            &unit[..5],
            &[0, 0, 0, 1, 0x09],
            "an access unit that does not open with a delimiter"
        );
        checked += 1;
    }
    assert!(checked > 10, "only {checked} access units were examined");
}

// ── The codec this whole path exists for ──────────────────────────────────
//
// AC-3 is passed through: a television decodes it for itself, so a film with an
// AC-3 soundtrack never reaches a decoder here and exercises none of the work
// below. DTS is the one no television will play and the one every stage of this
// has to get right — measured, decoded, re-encoded, and declared in the
// programme map only once its decoder has been proved to open.

/// The vendored DTS conformance fixture: 48 kHz, 1024-byte frames of 512
/// samples each, which is 768 kbps in eleven-millisecond frames.
const DTS: &[u8] = include_bytes!("../../vendor/oxideav-dts/tests/fixtures/dts_5_frames.bin");
const DTS_FRAME_LEN: usize = 1024;
const DTS_FRAME_MS: f64 = 512.0 / 48.0;

/// A film whose only soundtrack is DTS, which is the case a television shows
/// the picture of and plays nothing.
#[cfg(feature = "transcode-dts")]
fn dts_film(seconds: f64) -> Vec<u8> {
    let frames: Vec<&[u8]> = DTS.chunks_exact(DTS_FRAME_LEN).collect();
    let audio_count = (seconds * 1000.0 / DTS_FRAME_MS).round() as usize;
    let audio_samples: Vec<(u64, Vec<u8>)> = (0..audio_count)
        .map(|i| {
            (
                (i as f64 * DTS_FRAME_MS).round() as u64,
                frames[i % frames.len()].to_vec(),
            )
        })
        .collect();

    let video_count = (seconds * 25.0).round() as usize;
    let video_samples: Vec<(u64, Vec<u8>)> = (0..video_count)
        .map(|i| {
            (
                (i as f64 * 40.0).round() as u64,
                video_sample(i % 25 == 0, 96, i as u8),
            )
        })
        .collect();

    build_mkv(
        &[
            Track {
                number: 1,
                codec_id: "V_MPEG4/ISO/AVC",
                codec_private: AVCC.to_vec(),
                kind: TrackKind::Video {
                    width: 640,
                    height: 360,
                },
                samples: video_samples,
                all_keyframes: false,
                is_default: true,
                language: None,
            },
            Track {
                number: 2,
                codec_id: "A_DTS",
                codec_private: Vec::new(),
                kind: TrackKind::Audio {
                    sample_rate: 48_000.0,
                    channels: 6,
                },
                samples: audio_samples,
                all_keyframes: true,
                is_default: true,
                language: Some("eng"),
            },
        ],
        seconds * 1000.0,
    )
}

#[cfg(feature = "transcode-dts")]
async fn scanned_dts_film(seconds: f64) -> (tempfile::TempDir, vuio_core::state::AppState, i64) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("media");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("Film.mkv"), dts_film(seconds)).unwrap();
    let state = common::state_over(temp.path(), &root).await;
    let id = common::scan_into(&state)
        .await
        .iter()
        .find(|f| f.filename == "Film.mkv")
        .unwrap()
        .id
        .unwrap();
    (temp, state, id)
}

/// Whole ADTS frames on `pid`, reassembled across the packets they are split
/// over — which is how a television's demuxer will see them.
fn elementary_stream(body: &[u8], pid: u16) -> Vec<u8> {
    let mut out = Vec::new();
    for packet in body.chunks(TS_PACKET) {
        let this = (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]);
        if this != pid {
            continue;
        }
        let (payload, _) = payload_and_pcr(packet);
        let start = packet[1] & 0x40 != 0;
        if start {
            // Past the PES header and the timestamps it declared.
            let pes = &packet[payload..];
            out.extend_from_slice(&pes[9 + usize::from(pes[8])..]);
        } else {
            out.extend_from_slice(&packet[payload..]);
        }
    }
    out
}

/// The presentation timestamp of every access unit on `pid`, in stream order.
fn pes_timestamps(body: &[u8], pid: u16) -> Vec<u64> {
    body.chunks(TS_PACKET)
        .filter(|packet| {
            (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]) == pid
                && packet[1] & 0x40 != 0
        })
        .filter_map(|packet| {
            let (payload, _) = payload_and_pcr(packet);
            pes_pts(&packet[payload..])
        })
        .collect()
}

/// The deliverable: a DTS soundtrack no television can decode arrives as AAC it
/// can, in a transport stream it can seek.
#[cfg(feature = "transcode-dts")]
#[tokio::test]
async fn a_dts_film_reaches_the_television_as_aac() {
    let (_temp, state, id) = scanned_dts_film(8.0).await;
    let (status, headers, body) = video_ts(&state, id, "", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "video/mpeg");
    assert_well_formed_ts(&body);

    // The programme map declares the picture and one soundtrack, and the
    // soundtrack is AAC — not the DTS the container held, which is the point.
    let declared = declared_pids(&body);
    assert_eq!(declared, vec![0x0100, 0x0101], "{declared:?}");
    let pmt = body
        .chunks(TS_PACKET)
        .find(|packet| (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]) == 0x1000)
        .unwrap();
    let section = &pmt[5..];
    let info_length = usize::from(u16::from_be_bytes([section[10] & 0x0F, section[11]]));
    assert_eq!(
        section[12 + info_length],
        0x1B,
        "the picture is declared as H.264"
    );
    assert_eq!(
        section[12 + info_length + 5],
        0x0F,
        "and the soundtrack as AAC, whatever it arrived as"
    );

    // And what arrives on that PID really is AAC: ADTS frames, each one a
    // syncword and a length that walks to the next.
    let audio = elementary_stream(&body, 0x0101);
    assert!(!audio.is_empty(), "the soundtrack PID carried nothing");
    let mut at = 0usize;
    let mut frames = 0usize;
    while at + 7 <= audio.len() {
        assert_eq!(audio[at], 0xFF, "frame {frames} has no ADTS syncword");
        assert_eq!(audio[at + 1] & 0xF0, 0xF0);
        let len = ((usize::from(audio[at + 3]) & 0x03) << 11)
            | (usize::from(audio[at + 4]) << 3)
            | (usize::from(audio[at + 5]) >> 5);
        assert!(len >= 7, "frame {frames} declares {len} bytes");
        at += len;
        frames += 1;
    }
    // Eight seconds of 1024-sample frames at 48 kHz is around 375 of them.
    // Eight seconds of 1024-sample frames at 48 kHz is 375 of them, and every
    // one is accounted for: a decode that drops frames shortens the soundtrack
    // against the picture for the rest of the film.
    assert_eq!(frames, 375, "eight seconds of AAC is 375 frames");

    // And they are placed across the whole film rather than bunched at its
    // start, which is the failure mode of a re-encoded run anchored wrongly:
    // the sound plays, and plays in the wrong place.
    let stamps = pes_timestamps(&body, 0x0101);
    let (first, last) = (stamps[0], *stamps.last().unwrap());
    assert!(
        first < 45_000 + 90_000 / 2,
        "the soundtrack starts {first} ticks in, half a second past the headroom"
    );
    let span = last - first;
    // Eight seconds less the final frame, in ninety-kilohertz ticks.
    assert!(
        span.abs_diff(718_080) < 9_000,
        "the soundtrack spans {span} ticks of an eight-second film"
    );
}

/// The estimate on the codec it was written for. A DTS soundtrack leaves at a
/// quarter of the rate it arrived at, so a promise built from the source's own
/// size is mostly padding — and every byte offset then names a moment far later
/// than the viewer dragged to.
#[cfg(feature = "transcode-dts")]
#[tokio::test]
async fn the_promise_follows_a_dts_soundtrack_down_to_what_it_becomes() {
    let (_temp, state, id) = scanned_dts_film(12.0).await;
    let (_, headers, body) = video_ts(&state, id, "", None).await;
    let promised: u64 = headers[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(body.len() as u64, promised);

    let padding = body
        .chunks(TS_PACKET)
        .filter(|packet| (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]) == 0x1FFF)
        .count() as u64
        * TS_PACKET as u64;
    let film = promised - padding;
    assert!(
        padding * 4 < film,
        "{padding} bytes of the {promised} promised are filler against {film} of film"
    );

    // The source's own size is what the promise used to be, and on this film it
    // is far more than the stream weighs: 768 kbps of DTS leaves as 192 of AAC.
    let source = std::fs::metadata(_temp.path().join("media").join("Film.mkv"))
        .unwrap()
        .len();
    assert!(
        promised < source,
        "the soundtrack shrank fourfold and the promise did not: {promised} against \
         a {source}-byte source"
    );
}

/// Seeking is the thing being fixed, and it has to work on the codec that needs
/// the decoder — where the muxer opens mid-film and every soundtrack frame has
/// to be decoded and re-encoded from a standing start.
#[cfg(feature = "transcode-dts")]
#[tokio::test]
async fn a_dts_film_seeks_by_byte_and_still_carries_its_soundtrack() {
    let (_temp, state, id) = scanned_dts_film(12.0).await;
    let (_, headers, _) = video_ts(&state, id, "", None).await;
    let promised: u64 = headers[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();

    let half = promised / 2;
    let (status, _, body) = video_ts(&state, id, "", Some(&format!("bytes={half}-"))).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_well_formed_ts(&body);
    assert_eq!(pids(&body)[0], 0x0000, "the tables lead the response");

    let audio = elementary_stream(&body, 0x0101);
    assert!(
        audio.len() > 1024,
        "a seek into the middle of a DTS film produced {} bytes of soundtrack",
        audio.len()
    );
    assert_eq!(audio[0], 0xFF, "and it is still framed AAC");

    // The picture is there too, and starts on a random-access point — otherwise
    // the renderer has no reference frame to decode the first picture against.
    let opens_on_a_keyframe = body
        .chunks(TS_PACKET)
        .filter(|packet| (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]) == 0x0100)
        .find(|packet| packet[1] & 0x40 != 0)
        .map(|packet| (packet[3] >> 4) & 0b10 != 0 && packet[5] & 0x40 != 0)
        .unwrap_or(false);
    assert!(
        opens_on_a_keyframe,
        "the first picture is not marked random-access"
    );
}

/// A film with a soundtrack that cannot produce a single packet: one declared in
/// the container and never written, and one whose frames no decoder will open.
fn film_with_broken_soundtracks(seconds: f64) -> Vec<u8> {
    let frames: Vec<&[u8]> = AC3.chunks_exact(AC3_FRAME_LEN).collect();
    let audio_count = (seconds * 1000.0 / AC3_FRAME_MS).round() as usize;
    let stamp = |i: usize| (i as f64 * AC3_FRAME_MS).round() as u64;

    let video_count = (seconds * 25.0).round() as usize;
    let video_samples: Vec<(u64, Vec<u8>)> = (0..video_count)
        .map(|i| {
            (
                (i as f64 * 40.0).round() as u64,
                video_sample(i % 25 == 0, 96, i as u8),
            )
        })
        .collect();

    build_mkv(
        &[
            Track {
                number: 1,
                codec_id: "V_MPEG4/ISO/AVC",
                codec_private: AVCC.to_vec(),
                kind: TrackKind::Video {
                    width: 640,
                    height: 360,
                },
                samples: video_samples,
                all_keyframes: false,
                is_default: true,
                language: None,
            },
            // The one that works.
            Track {
                number: 2,
                codec_id: "A_AC3",
                codec_private: Vec::new(),
                kind: TrackKind::Audio {
                    sample_rate: 48_000.0,
                    channels: 2,
                },
                samples: (0..audio_count)
                    .map(|i| (stamp(i), frames[i % frames.len()].to_vec()))
                    .collect(),
                all_keyframes: true,
                is_default: true,
                language: Some("eng"),
            },
            // Declared, and never written a block.
            Track {
                number: 3,
                codec_id: "A_AC3",
                codec_private: Vec::new(),
                kind: TrackKind::Audio {
                    sample_rate: 48_000.0,
                    channels: 2,
                },
                samples: Vec::new(),
                all_keyframes: true,
                is_default: false,
                language: Some("fra"),
            },
            // Written, and nothing will decode it.
            Track {
                number: 4,
                codec_id: "A_DTS",
                codec_private: Vec::new(),
                kind: TrackKind::Audio {
                    sample_rate: 48_000.0,
                    channels: 6,
                },
                samples: (0..audio_count)
                    .map(|i| (stamp(i), vec![0xA5u8; 1024]))
                    .collect(),
                all_keyframes: true,
                is_default: false,
                language: Some("deu"),
            },
        ],
        seconds * 1000.0,
    )
}

/// A stream declared in the programme map and then silent forever is worse than
/// one that was never declared: the renderer reads the map, sees a PID it is
/// expecting audio on, and waits for it. So the map promises nothing that has
/// not been proved first.
#[tokio::test]
async fn a_soundtrack_that_cannot_produce_packets_is_never_declared() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("media");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("Film.mkv"), film_with_broken_soundtracks(8.0)).unwrap();
    let state = common::state_over(temp.path(), &root).await;
    let id = common::scan_into(&state)
        .await
        .iter()
        .find(|f| f.filename == "Film.mkv")
        .unwrap()
        .id
        .unwrap();

    let (status, _, body) = video_ts(&state, id, "", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_well_formed_ts(&body);

    let declared = declared_pids(&body);
    assert_eq!(
        declared,
        vec![0x0100, 0x0101],
        "the picture and the one soundtrack that works, and nothing else: {declared:?}"
    );

    // And what it declared really does arrive.
    let carried = pids(&body);
    for pid in &declared {
        assert!(
            carried.contains(pid),
            "PID {pid:#x} was promised and never sent"
        );
    }
}

/// The number the whole seek mechanism rests on.
///
/// A byte offset into this resource is read as a fraction of its promised
/// length, so the promise being an honest account of what the stream weighs is
/// what makes an offset mean the moment the viewer dragged to. Promising the
/// source file's own size overstated it by a factor of three on the films this
/// path exists for, and every byte offset then named a moment three times too
/// far along.
#[tokio::test]
async fn the_promised_length_is_an_honest_account_of_the_stream() {
    let (_temp, state, id) = scanned_film(12.0).await;
    let (_, headers, body) = video_ts(&state, id, "", None).await;
    let promised: u64 = headers[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(body.len() as u64, promised);

    let padding = body
        .chunks(TS_PACKET)
        .filter(|packet| (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]) == 0x1FFF)
        .count() as u64
        * TS_PACKET as u64;
    let film = promised - padding;
    assert!(
        padding * 4 < film,
        "{padding} bytes of the {promised} promised are filler, against {film} of \
         film — the promise is not describing what is actually produced"
    );
}

/// How a television learns how long a transport stream is, and the reason a film
/// can play perfectly and still have no scrub bar.
///
/// There is no header to ask — a transport stream does not have one. So a set
/// reads the last hundred kilobytes or so of the resource and takes the newest
/// timestamp it finds; `last - first` is the duration, and until it has one it
/// publishes no seek map at all. Answering that read with null packets teaches
/// it nothing: the picture plays, every soundtrack plays, and the film is
/// unseekable and of unknown length.
///
/// So the end of the resource has to carry the end of the film.
#[tokio::test]
async fn the_last_bytes_carry_the_films_final_timestamps() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let (_, headers, _) = video_ts(&state, id, "", None).await;
    let promised: u64 = headers[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();

    // The last few kilobytes, which is where a duration reader looks.
    let first = promised - 4096;
    let (status, _, body) = video_ts(&state, id, "", Some(&format!("bytes={first}-"))).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_well_formed_ts(&body);

    let filler = body
        .chunks(TS_PACKET)
        .filter(|packet| (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]) == 0x1FFF)
        .count();
    assert!(
        filler * 2 < body.len() / TS_PACKET,
        "the tail is mostly null packets, which name no instant at all"
    );

    let clocks: Vec<f64> = body
        .chunks(TS_PACKET)
        .filter_map(|packet| payload_and_pcr(packet).1)
        .map(|pcr| pcr as f64 / 90_000.0)
        .collect();
    assert!(
        !clocks.is_empty(),
        "nothing in the last {} bytes says what time it is",
        body.len()
    );
    let newest = clocks.iter().cloned().fold(f64::MIN, f64::max);
    assert!(
        newest > 6.0,
        "the last bytes of an eight-second film are stamped {newest:.2}s, so a set \
         reading them would think the film that long"
    );
}

/// With nothing left to produce it with, the tail falls back to padding rather
/// than being refused: a renderer sizing the resource up gets an answer, and the
/// streams already playing keep their slots.
#[tokio::test]
async fn a_tail_probe_still_answers_when_every_slot_is_taken() {
    let (_temp, state, id) = scanned_film(8.0).await;
    let (_, headers, _) = video_ts(&state, id, "", None).await;
    let promised: u64 = headers[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();

    let mut held = Vec::new();
    while let Some(permit) = state.transcode.try_acquire() {
        held.push(permit);
    }
    let first = promised - 4096;
    let (status, _, body) = video_ts(&state, id, "", Some(&format!("bytes={first}-"))).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT, "not 503");
    assert_eq!(body.len(), 4096);
}

/// The decode timeline has to be recovered without reference to what follows,
/// because a batch is written before the next one is read.
///
/// Batches are cut wherever the reordering is closed rather than only at
/// keyframes — a film with ten-second groups of pictures would otherwise hand a
/// renderer ten seconds of decoded soundtrack before its first byte. The cut is
/// only safe where every frame held is displayed before the next one read; cut
/// through a reorder group instead and the batch after it opens with a frame
/// due *earlier* than the one that closed the batch before, which a decoder
/// reads as time running backwards.
#[tokio::test]
async fn decode_times_never_run_backwards_across_a_batch_seam() {
    let (_temp, state, id) = scanned_film(12.0).await;
    let (_, _, body) = video_ts(&state, id, "", None).await;

    let mut previous: Option<u64> = None;
    let mut seen = 0usize;
    for packet in body.chunks(TS_PACKET) {
        let pid = (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]);
        if pid != 0x0100 || packet[1] & 0x40 == 0 {
            continue;
        }
        let (payload, Some(pcr)) = payload_and_pcr(packet) else {
            continue;
        };
        if let Some(previous) = previous {
            assert!(
                pcr >= previous,
                "the programme clock went from {previous} back to {pcr} at picture {seen}"
            );
        }
        previous = Some(pcr);
        // And the picture is never due before it is decoded.
        if let Some(pts) = pes_pts(&packet[payload..]) {
            assert!(
                pts >= pcr,
                "picture {seen} is due at {pts}, clocked at {pcr}"
            );
        }
        seen += 1;
    }
    assert!(seen > 100, "only {seen} pictures were examined");
}

/// A seek near the end is a seek, not a probe: it still produces film.
#[tokio::test]
async fn a_genuine_seek_is_not_mistaken_for_a_probe() {
    let (_temp, state, id) = scanned_film(12.0).await;
    let (_, headers, _) = video_ts(&state, id, "", None).await;
    let promised: u64 = headers[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();

    let half = promised / 2;
    let (_, _, body) = video_ts(&state, id, "", Some(&format!("bytes={half}-"))).await;
    // Padding is null packets on PID 0x1FFF; film is not.
    let carries_film = body.chunks(TS_PACKET).any(|packet| {
        let pid = (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]);
        pid != 0x1FFF
    });
    assert!(carries_film, "a seek to the middle produced only padding");
}

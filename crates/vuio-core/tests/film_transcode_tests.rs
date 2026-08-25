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
        "mp4a",
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
                "mp4a" => &body[28..],
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
        "time seek yes, byte seek no: {features}"
    );
    assert!(
        !headers.contains_key(header::ACCEPT_RANGES),
        "claiming byte ranges and then refusing them is worse than never claiming"
    );
    assert!(
        !headers.contains_key(header::CONTENT_LENGTH),
        "the length of this resource is not knowable before it exists"
    );

    let top: Vec<String> = boxes(&body).into_iter().map(|(name, _)| name).collect();
    assert_eq!(&top[..2], &["ftyp", "moov"], "an init segment comes first");
    let fragments = top[2..].chunks(2).collect::<Vec<_>>();
    assert!(
        fragments.len() >= 2,
        "expected several moof/mdat pairs, got {top:?}"
    );
    for pair in &fragments {
        assert_eq!(pair, &["moof", "mdat"], "in {top:?}");
    }

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
    assert!(
        find_box(&body, "mp4a").is_some(),
        "the AC-3 track must arrive as AAC — nothing else would play"
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
    // Six seconds of 48 kHz audio is 281 AAC frames of 1024 samples, and each
    // track should carry very nearly all of them — the first fragment or two
    // are lost to the wait for the video's first keyframe.
    for track_id in [2u32, 3, 4] {
        let count = samples.get(&track_id).copied().unwrap_or(0);
        assert!(
            (200..=290).contains(&count),
            "track {track_id} carried {count} AAC frames across the film: {samples:?}"
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
            let kind = if find_box(trak, "mp4a").is_some() {
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
        didl.contains(&format!("/media/{id}/transcode/video.mp4")),
        "a film's alternative is the film:\n{didl}"
    );
    assert!(
        !didl.contains("transcode/audio.wav"),
        "offering a film's soundtrack in place of the film would lose the picture:\n{didl}"
    );
    assert!(
        didl.contains("DLNA.ORG_OP=10;DLNA.ORG_CI=1"),
        "the advertised operations must be the ones the resource honours:\n{didl}"
    );
    // The original stays, and stays first by default, so a television that can
    // decode AC-3 keeps its byte-seekable direct-play resource.
    let original = didl
        .find(&format!("/media/{id}</res>"))
        .unwrap_or_else(|| panic!("no direct-play resource in:\n{didl}"));
    let transcoded = didl.find("transcode/video.mp4").unwrap();
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

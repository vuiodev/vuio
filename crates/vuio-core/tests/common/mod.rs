//! A Matroska file, built at test time, with real audio inside it.
//!
//! Phase 4 is about films, and `test-media/movie1.mkv` is a 26-byte stub. The
//! alternative to committing a binary film clip is this: an EBML writer small
//! enough to read, fed the vendored decoders' own conformance fixtures, so the
//! MKV a test scans and streams carries AC-3 or DTS frames that really decode.
//!
//! The video track is synthesised rather than encoded — nothing in this tree
//! decodes video, and nothing needs to. What matters about it is what the
//! passthrough path actually touches: an `avcC` that must arrive byte-identical
//! in the output's `stsd`, and length-prefixed NAL units whose type says which
//! samples are random-access points.

#![allow(dead_code)]

/// A plausible AVCDecoderConfigurationRecord: High profile, level 3.0, 4-byte
/// NAL length prefixes, one SPS and one PPS. The payload bytes are not a real
/// sequence parameter set and do not need to be — no test here decodes a
/// picture, and every stage that handles this record copies it verbatim.
pub const AVCC: &[u8] = &[
    0x01, 0x64, 0x00, 0x1E, // configurationVersion, profile, compat, level
    0xFF, // lengthSizeMinusOne = 3
    0xE1, // numOfSequenceParameterSets = 1
    0x00, 0x0A, // SPS length
    0x67, 0x64, 0x00, 0x1E, 0xAC, 0xD9, 0x40, 0xA0, 0x2F, 0xF9, // SPS
    0x01, // numOfPictureParameterSets
    0x00, 0x04, // PPS length
    0x68, 0xEB, 0xE3, 0xCB, // PPS
];

/// One video sample: a single length-prefixed NAL unit of `len` bytes.
///
/// `keyframe` picks the NAL type the keyframe detector looks for — 5 (IDR) or 1
/// (non-IDR slice) — which is what decides where an HLS segment may begin.
pub fn video_sample(keyframe: bool, len: usize, fill: u8) -> Vec<u8> {
    let nal_type: u8 = if keyframe { 0x65 } else { 0x41 };
    let mut nal = vec![nal_type];
    nal.extend(std::iter::repeat_n(fill, len.saturating_sub(1)));
    let mut sample = (nal.len() as u32).to_be_bytes().to_vec();
    sample.extend_from_slice(&nal);
    sample
}

/// One track to write into the file.
pub struct Track {
    pub number: u64,
    /// Matroska `CodecID`, e.g. `V_MPEG4/ISO/AVC` or `A_AC3`.
    pub codec_id: &'static str,
    pub codec_private: Vec<u8>,
    pub kind: TrackKind,
    /// The samples, in order, each with the millisecond it is presented at.
    pub samples: Vec<(u64, Vec<u8>)>,
    /// Whether every sample is a random-access point.
    pub all_keyframes: bool,
    /// Whether the container marks this the default track of its kind.
    pub is_default: bool,
    /// Matroska `Language`, an ISO-639-2 code. `None` writes no element, which
    /// is how a muxer says nothing rather than saying "und".
    pub language: Option<&'static str>,
}

pub enum TrackKind {
    Video { width: u64, height: u64 },
    Audio { sample_rate: f64, channels: u64 },
}

/// Serialize `tracks` into a Matroska file `duration_ms` long.
///
/// One cluster per second, each opening with a `Timestamp` and holding a
/// `SimpleBlock` per sample. A `SeekHead` at the front points at the `Cues` at
/// the back, which is what makes the file seekable: symphonia stops scanning
/// top-level elements at the first cluster, so a `Cues` element it was not told
/// about in advance is one it never reads.
pub fn build_mkv(tracks: &[Track], duration_ms: f64) -> Vec<u8> {
    build_mkv_inner(tracks, duration_ms, false)
}

/// The same film with an `EditionEntry` that omits its nominally mandatory
/// `EditionUID`, matching Matroska emitted by ffmpeg in the wild. Players ignore
/// the incomplete chapter metadata; this fixture makes sure browser remuxing
/// does too.
pub fn build_mkv_with_invalid_chapters(tracks: &[Track], duration_ms: f64) -> Vec<u8> {
    build_mkv_inner(tracks, duration_ms, true)
}

fn build_mkv_inner(tracks: &[Track], duration_ms: f64, invalid_chapters: bool) -> Vec<u8> {
    const ID_SEEK_HEAD: u32 = 0x114D9B74;
    const ID_INFO: u32 = 0x1549A966;
    const ID_TRACKS: u32 = 0x1654AE6B;
    const ID_CHAPTERS: u32 = 0x1043A770;
    const ID_CLUSTER: u32 = 0x1F43B675;
    const ID_CUES: u32 = 0x1C53BB6B;

    // --- Info ---
    let mut info = Vec::new();
    info.extend(uint_el(0x2AD7B1, 1_000_000)); // TimestampScale: 1 ms per tick
    info.extend(float_el(0x4489, duration_ms)); // Duration, in ticks
    info.extend(str_el(0x4D80, "vuio-test"));
    info.extend(str_el(0x5741, "vuio-test"));
    let info = master(ID_INFO, &info);

    // --- Tracks ---
    let mut entries = Vec::new();
    for track in tracks {
        let mut entry = Vec::new();
        entry.extend(uint_el(0xD7, track.number)); // TrackNumber
        entry.extend(uint_el(0x73C5, track.number)); // TrackUID
        entry.extend(uint_el(
            0x83,
            match track.kind {
                TrackKind::Video { .. } => 1,
                TrackKind::Audio { .. } => 2,
            },
        )); // TrackType
        entry.extend(uint_el(0x88, u64::from(track.is_default))); // FlagDefault
        if let Some(language) = track.language {
            entry.extend(str_el(0x22B59C, language)); // Language
        }
        entry.extend(str_el(0x86, track.codec_id)); // CodecID
        if !track.codec_private.is_empty() {
            entry.extend(bin_el(0x63A2, &track.codec_private)); // CodecPrivate
        }
        match track.kind {
            TrackKind::Video { width, height } => {
                let mut video = Vec::new();
                video.extend(uint_el(0xB0, width));
                video.extend(uint_el(0xBA, height));
                entry.extend(master(0xE0, &video));
            }
            TrackKind::Audio {
                sample_rate,
                channels,
            } => {
                let mut audio = Vec::new();
                audio.extend(float_el(0xB5, sample_rate));
                audio.extend(uint_el(0x9F, channels));
                entry.extend(master(0xE1, &audio));
            }
        }
        entries.extend(master(0xAE, &entry));
    }
    let tracks_el = master(ID_TRACKS, &entries);

    // ffmpeg has emitted EditionEntry without EditionUID. Symphonia treats
    // that optional metadata defect as fatal for the entire container, while
    // players simply ignore the chapters.
    let chapters = if invalid_chapters {
        let edition = master(0x45B9, &uint_el(0x45DB, 1)); // EditionFlagDefault only.
        master(ID_CHAPTERS, &edition)
    } else {
        Vec::new()
    };

    // --- Clusters, one per second of content ---
    // Every sample from every track, in presentation order, so the interleaving
    // is what a real muxer would produce and a demuxer walking forward sees both
    // tracks advance together.
    let mut all: Vec<(u64, u64, &Vec<u8>, bool)> = Vec::new();
    for track in tracks {
        for (ms, data) in &track.samples {
            all.push((*ms, track.number, data, track.all_keyframes));
        }
    }
    all.sort_by_key(|(ms, number, _, _)| (*ms, *number));

    let mut clusters = Vec::new();
    // (cluster timestamp in ms, byte offset from the start of the segment's data)
    let mut cue_points: Vec<(u64, u64)> = Vec::new();
    let seek_entries: Vec<(u32, u64)> = if invalid_chapters {
        vec![(ID_INFO, 0), (ID_TRACKS, 0), (ID_CHAPTERS, 0), (ID_CUES, 0)]
    } else {
        vec![(ID_INFO, 0), (ID_TRACKS, 0), (ID_CUES, 0)]
    };
    let seek_head_len = seek_head(&seek_entries).len() as u64;
    let mut cursor =
        seek_head_len + info.len() as u64 + tracks_el.len() as u64 + chapters.len() as u64;

    let mut index = 0usize;
    while index < all.len() {
        let cluster_ms = all[index].0 / 1000 * 1000;
        let mut body = Vec::new();
        body.extend(uint_el(0xE7, cluster_ms)); // Timestamp
        while index < all.len() && all[index].0 < cluster_ms + 1000 {
            let (ms, number, data, keyframe) = all[index];
            let mut block = vint(number); // track number, as a vint
            block.extend_from_slice(&((ms as i64 - cluster_ms as i64) as i16).to_be_bytes());
            // Bit 7 is the keyframe flag; a SimpleBlock carries no other state
            // this writer needs.
            block.push(if keyframe || is_keyframe_sample(data) {
                0x80
            } else {
                0x00
            });
            block.extend_from_slice(data);
            body.extend(bin_el(0xA3, &block)); // SimpleBlock
            index += 1;
        }
        let cluster = master(ID_CLUSTER, &body);
        cue_points.push((cluster_ms, cursor));
        cursor += cluster.len() as u64;
        clusters.extend(cluster);
    }

    // --- Cues, one point per cluster, for the first track ---
    let cues_pos = cursor;
    let cue_track = tracks.first().map(|t| t.number).unwrap_or(1);
    let mut cues_body = Vec::new();
    for (time, position) in &cue_points {
        let mut point = Vec::new();
        point.extend(uint_el(0xB3, *time)); // CueTime
        let mut positions = Vec::new();
        positions.extend(uint_el(0xF7, cue_track)); // CueTrack
        positions.extend(uint_el(0xF1, *position)); // CueClusterPosition
        point.extend(master(0xB7, &positions));
        cues_body.extend(master(0xBB, &point));
    }
    let cues = master(ID_CUES, &cues_body);

    // --- SeekHead, now that the positions it points at are known ---
    let mut seek_entries = vec![
        (ID_INFO, seek_head_len),
        (ID_TRACKS, seek_head_len + info.len() as u64),
    ];
    if invalid_chapters {
        seek_entries.push((
            ID_CHAPTERS,
            seek_head_len + info.len() as u64 + tracks_el.len() as u64,
        ));
    }
    seek_entries.push((ID_CUES, cues_pos));
    let seek_head = seek_head(&seek_entries);
    assert_eq!(
        seek_head.len() as u64,
        seek_head_len,
        "the SeekHead must be exactly as long as its placeholder, or every \
         position after it moves"
    );

    let mut segment_body = Vec::new();
    segment_body.extend(seek_head);
    segment_body.extend(info);
    segment_body.extend(tracks_el);
    segment_body.extend(chapters);
    segment_body.extend(clusters);
    segment_body.extend(cues);

    let mut out = ebml_header();
    out.extend(master(0x18538067, &segment_body)); // Segment
    out
}

fn is_keyframe_sample(data: &[u8]) -> bool {
    data.len() > 4 && (data[4] & 0x1F) == 5
}

fn ebml_header() -> Vec<u8> {
    let mut body = Vec::new();
    body.extend(uint_el(0x4286, 1)); // EBMLVersion
    body.extend(uint_el(0x42F7, 1)); // EBMLReadVersion
    body.extend(uint_el(0x42F2, 4)); // EBMLMaxIDLength
    body.extend(uint_el(0x42F3, 8)); // EBMLMaxSizeLength
    body.extend(str_el(0x4282, "matroska"));
    body.extend(uint_el(0x4287, 4)); // DocTypeVersion
    body.extend(uint_el(0x4285, 2)); // DocTypeReadVersion
    master(0x1A45DFA3, &body)
}

/// Every `SeekPosition` is written at a fixed eight bytes, so the element's
/// length does not depend on the values it ends up carrying — which is what
/// allows one layout pass instead of iterating to a fixed point.
fn seek_head(entries: &[(u32, u64)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (id, position) in entries {
        let mut seek = Vec::new();
        seek.extend(bin_el(0x53AB, &id_bytes(*id))); // SeekID
        seek.extend(uint_el_fixed(0x53AC, *position, 8)); // SeekPosition
        body.extend(master(0x4DBB, &seek));
    }
    master(0x114D9B74, &body)
}

/// The bytes of an EBML element ID, written verbatim as the class ID they are.
fn id_bytes(id: u32) -> Vec<u8> {
    let bytes = id.to_be_bytes();
    let first = bytes.iter().position(|b| *b != 0).unwrap_or(3);
    bytes[first..].to_vec()
}

/// An EBML unsigned length or value, in the smallest width that fits.
fn vint(value: u64) -> Vec<u8> {
    for width in 1..=8u32 {
        // The all-ones value of each width is reserved as "unknown length".
        let capacity = (1u64 << (7 * width)) - 1;
        if value < capacity {
            let marked = value | (1u64 << (7 * width));
            return marked.to_be_bytes()[8 - width as usize..].to_vec();
        }
    }
    unreachable!("no EBML length exceeds eight bytes")
}

fn element(id: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = id_bytes(id);
    out.extend(vint(payload.len() as u64));
    out.extend_from_slice(payload);
    out
}

fn master(id: u32, body: &[u8]) -> Vec<u8> {
    element(id, body)
}

fn bin_el(id: u32, data: &[u8]) -> Vec<u8> {
    element(id, data)
}

fn str_el(id: u32, value: &str) -> Vec<u8> {
    element(id, value.as_bytes())
}

fn uint_el(id: u32, value: u64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let first = bytes.iter().position(|b| *b != 0).unwrap_or(7);
    element(id, &bytes[first..])
}

fn uint_el_fixed(id: u32, value: u64, width: usize) -> Vec<u8> {
    element(id, &value.to_be_bytes()[8 - width..])
}

fn float_el(id: u32, value: f64) -> Vec<u8> {
    element(id, &value.to_be_bytes())
}

// ── A server over a temporary library ──────────────────────────────────────

use std::sync::Arc;
use vuio_core::config::{AppConfig, MonitoredDirectoryConfig, ValidationMode};
use vuio_core::database::sqlite::SqliteDatabase;
use vuio_core::database::{DatabaseManager, MediaFile, MediaRepository};
use vuio_core::state::AppState;

/// Bring up the real server state over `root`, with `files` already indexed.
///
/// The rows are written by the scanner's own path where that matters (see
/// [`scan_into`]); this is for tests that only need an item to exist.
pub async fn state_over(temp: &std::path::Path, root: &std::path::Path) -> AppState {
    let database = Arc::new(SqliteDatabase::new(temp.join("library.db")).await.unwrap());
    database.initialize().await.unwrap();

    let mut config = AppConfig::default();
    config.media.directories = vec![MonitoredDirectoryConfig {
        path: root.to_string_lossy().into_owned(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Skip,
    }];
    let config = Arc::new(config);

    AppState {
        media_directories: Arc::new(tokio::sync::RwLock::new(config.media.directories.clone())),
        unavailable_roots: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
        config: config.clone(),
        config_source: Arc::new(Default::default()),
        http_binding: Arc::new(vuio_core::state::HttpBinding::new(8080)),
        live_config: Arc::new(vuio_core::state::LiveConfig::new(config)),
        database,
        auth: Arc::new(vuio_core::web::auth::AuthState::testing()),
        platform_info: Arc::new(vuio_core::platform::PlatformInfo::detect().await.unwrap()),
        filesystem_manager: Arc::from(
            vuio_core::platform::filesystem::create_platform_filesystem_manager(),
        ),
        content_update_id: Arc::new(std::sync::atomic::AtomicU32::new(1)),
        web_metrics: Arc::new(vuio_core::web::diagnostics::WebHandlerMetrics::new()),
        runtime_diagnostics: Arc::new(
            vuio_core::platform::diagnostics::SystemDiagnosticsSampler::new(),
        ),
        lifecycle_stats: Arc::new(vuio_core::lifecycle::ApplicationStats::new()),
        bookmarks: Arc::new(tokio::sync::Mutex::new(
            vuio_core::runtime_state::BookmarkRegistry::new(
                vuio_core::runtime_state::BOOKMARK_MAX_ENTRIES,
            ),
        )),
        log_file_path: temp.join("vuio.log"),
        browse_cache: Arc::new(tokio::sync::Mutex::new(
            vuio_core::runtime_state::BrowseResponseCache::new(),
        )),
        active_monitors: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_casts: Arc::new(tokio::sync::Mutex::new(
            vuio_core::runtime_state::ActiveCastRegistry::new(),
        )),
        #[cfg(feature = "mediainfo")]
        mediainfo_job: Arc::new(tokio::sync::Mutex::new(Default::default())),
        #[cfg(feature = "casting")]
        discovered_tvs: Arc::new(vuio_core::runtime_state::RendererCache::new()),
        upnp_subscriptions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        radio: Arc::new(Default::default()),
        #[cfg(feature = "transcode")]
        transcode: Arc::new(Default::default()),
        cancellation: tokio_util::sync::CancellationToken::new(),
        background_tasks: tokio_util::task::TaskTracker::new(),
    }
}

/// Run the real scanner over the state's configured directories.
///
/// Tests that assert on what the *scanner* records — a film's audio codec, say —
/// must go through it rather than injecting rows, because injecting rows is
/// exactly how a scanner that indexes nothing goes unnoticed.
pub async fn scan_into(state: &AppState) -> Vec<MediaFile> {
    let directories = state.media_directories.read().await.clone();
    let scanner = vuio_core::media::MediaScanner::with_database(state.database.clone());
    for directory in &directories {
        scanner
            .scan_directory(std::path::Path::new(&directory.path))
            .await
            .unwrap();
    }
    state.database.collect_all_media_files().await.unwrap()
}

//! Temporary diagnostic harness. Not part of the suite; deleted before commit.
#![cfg(all(feature = "transcode-dts", feature = "casting"))]

mod common;

use axum::http::{Method, Request, StatusCode};
use axum::extract::ConnectInfo;
use std::sync::Arc;
use tower::ServiceExt;
use vuio_core::database::MediaRepository;

async fn get(
    state: &vuio_core::state::AppState,
    uri: &str,
) -> (StatusCode, Vec<u8>) {
    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .extension(ConnectInfo::<std::net::SocketAddr>(
            "127.0.0.1:50000".parse().unwrap(),
        ))
        .body(axum::body::Body::empty())
        .unwrap();
    let router = vuio_core::web::create_router(state.clone(), vuio_core::web::Surface::Primary);
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, body.to_vec())
}

fn boxes(data: &[u8]) -> Vec<(String, &[u8])> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let size = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        let name = String::from_utf8_lossy(&data[pos + 4..pos + 8]).into_owned();
        if size < 8 || pos + size > data.len() { break; }
        out.push((name, &data[pos + 8..pos + size]));
        pos += size;
    }
    out
}

fn find_box<'a>(data: &'a [u8], name: &str) -> Option<&'a [u8]> {
    const CONTAINERS: &[&str] = &["moov", "trak", "mdia", "minf", "stbl", "stsd", "mvex", "moof", "traf", "avc1", "hvc1", "mp4a"];
    for (found, body) in boxes(data) {
        if found == name { return Some(body); }
        if CONTAINERS.contains(&found.as_str()) {
            let inner = match found.as_str() {
                "stsd" => 8,
                "avc1" | "hvc1" => 78,
                "mp4a" => 28,
                _ => 0,
            };
            if body.len() > inner {
                if let Some(hit) = find_box(&body[inner..], name) { return Some(hit); }
            }
        }
    }
    None
}

/// trun sample durations, summed: what the segment really covers on the timeline.
fn trun_span(segment: &[u8]) -> (u64, u64, u32) {
    let tfdt = find_box(segment, "tfdt").expect("tfdt");
    let base = u64::from_be_bytes(tfdt[4..12].try_into().unwrap());
    let trun = find_box(segment, "trun").expect("trun");
    let flags = u32::from_be_bytes([0, trun[1], trun[2], trun[3]]);
    let count = u32::from_be_bytes(trun[4..8].try_into().unwrap());
    let mut at = 8usize;
    if flags & 0x000001 != 0 { at += 4; }   // data-offset
    if flags & 0x000004 != 0 { at += 4; }   // first-sample-flags
    let per = ((flags & 0x000100 != 0) as usize
        + (flags & 0x000200 != 0) as usize
        + (flags & 0x000400 != 0) as usize
        + (flags & 0x000800 != 0) as usize) * 4;
    let mut total = 0u64;
    for i in 0..count as usize {
        let off = at + i * per;
        if flags & 0x000100 != 0 && off + 4 <= trun.len() {
            total += u64::from(u32::from_be_bytes(trun[off..off + 4].try_into().unwrap()));
        }
    }
    (base, total, count)
}

#[tokio::test]
async fn diag_real_router() {
    let Some(film) = std::env::var_os("DIAG_IN").map(std::path::PathBuf::from) else { return };
    let segs: usize = std::env::var("DIAG_SEGS").ok().and_then(|v| v.parse().ok()).unwrap_or(12);

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("media");
    std::fs::create_dir_all(&root).unwrap();
    let link = root.join("Film.mkv");
    std::os::unix::fs::symlink(&film, &link).unwrap();

    let state = common::state_over(temp.path(), &root).await;
    let files = common::scan_into(&state).await;
    let entry = files.iter().find(|f| f.filename == "Film.mkv").expect("scanned");
    let id = entry.id.unwrap();
    eprintln!("scanned as id {id}");

    let (status, body) = get(&state, &format!("/media/{id}/hls/master.m3u8")).await;
    eprintln!("master {status}:\n{}", String::from_utf8_lossy(&body));

    for rendition in ["video", "audio/0", "audio/1"] {
        let (status, body) = get(&state, &format!("/media/{id}/hls/{rendition}/index.m3u8")).await;
        let text = String::from_utf8_lossy(&body).into_owned();
        let extinf: Vec<f64> = text.lines()
            .filter_map(|l| l.strip_prefix("#EXTINF:"))
            .filter_map(|v| v.trim_end_matches(',').parse().ok())
            .collect();
        let mx = extinf.iter().cloned().fold(0.0f64, f64::max);
        let mn = extinf.iter().cloned().fold(f64::MAX, f64::min);
        let target = text.lines().find(|l| l.starts_with("#EXT-X-TARGETDURATION")).unwrap_or("(none)");
        eprintln!("--- {rendition} playlist {status}: {} segments, total {:.3}s, min {mn:.3} max {mx:.3}, {target}",
            extinf.len(), extinf.iter().sum::<f64>());
        let long: Vec<(usize, f64)> = extinf.iter().cloned().enumerate().filter(|(_, d)| *d > 20.0).collect();
        eprintln!("    segments over 20s: {} {:?}", long.len(), long.iter().take(10).collect::<Vec<_>>());

        let (status, init) = get(&state, &format!("/media/{id}/hls/{rendition}/init.mp4")).await;
        eprintln!("    init {status}, {} bytes", init.len());

        let mut expect_base: Option<u64> = None;
        let timescale: u64 = if rendition == "video" { 90_000 } else { 48_000 };
        for seq in 0..segs.min(extinf.len()) {
            let (status, seg) = get(&state, &format!("/media/{id}/hls/{rendition}/segment/{seq}")).await;
            if status != StatusCode::OK {
                eprintln!("    seg {seq}: {status} !!");
                continue;
            }
            let (base, span, count) = trun_span(&seg);
            let promised = extinf[seq];
            let actual = span as f64 / timescale as f64;
            let gap = expect_base.map(|e| base as i64 - e as i64);
            eprintln!(
                "    seg {seq:2}: {} bytes, {count} samples, tfdt={base} ({:.3}s) covers {actual:.3}s vs EXTINF {promised:.3}s, joint gap {:?}",
                seg.len(), base as f64 / timescale as f64, gap
            );
            expect_base = Some(base + span);
        }
    }
    let _ = Arc::strong_count(&state.database);
}

//! Golden-bitstream checksum for the AC-3 encoder.
//!
//! Several of the encoder optimisations are meant to be *bit-exact*: a faster
//! CRC, and hoisting work out of the rate-control loop that never depended on
//! the loop variable. "Bit-exact" is a strong claim and deserves a strong
//! check, so this encodes a fixed deterministic signal and prints a checksum of
//! the resulting bitstream. Run it before and after such a change; the digests
//! must match. A change that is only *numerically* equivalent — the FFT MDCT —
//! will move these, which is why it is kept separate from the ones that must not.

use std::fmt::Write as _;

/// FNV-1a. Not cryptographic; this only has to notice that a bitstream moved.
fn digest(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

/// Deterministic multi-tone PCM. Distinct frequency per channel so no channel
/// is a copy of another, and the LFE (WAVE index 3) stays under the 120 Hz
/// limit A/52 §5.5.5 puts on it — above that the encoder correctly discards it
/// and the channel tells us nothing.
fn signal(sample_rate: u32, channels: u16, secs: f64) -> Vec<u8> {
    let freqs: [f64; 8] = [440.0, 554.37, 659.25, 55.0, 880.0, 1108.73, 220.0, 1318.51];
    let frames = (sample_rate as f64 * secs) as usize;
    let mut out = Vec::with_capacity(frames * channels as usize * 2);
    for i in 0..frames {
        let t = i as f64 / sample_rate as f64;
        for ch in 0..channels as usize {
            let v = (t * freqs[ch % freqs.len()] * 2.0 * std::f64::consts::PI).sin();
            // A slow amplitude sweep keeps the rate-control loop moving instead
            // of settling on one snroffset for the whole run.
            let env = 0.55 + 0.45 * (t * 0.7 * 2.0 * std::f64::consts::PI).sin();
            out.extend_from_slice(&(((v * env) * 24000.0) as i16).to_le_bytes());
        }
    }
    out
}

fn case(label: &str, channels: u16, sample_rate: u32) -> String {
    let pcm = signal(sample_rate, channels, 4.0);
    let mut enc = vuio_core::media::transcode::Ac3Encoder::new(sample_rate, channels)
        .expect("construct Ac3Encoder");
    let mut stream = Vec::new();
    let mut frames = 0usize;
    // Feed in ragged chunks, the way the TS muxer does — decoded packets do not
    // arrive on 1536-sample boundaries.
    let stride = channels as usize * 2;
    for chunk in pcm.chunks(517 * stride) {
        for pkt in enc.push(chunk).expect("push") {
            stream.extend_from_slice(&pkt);
            frames += 1;
        }
    }
    for pkt in enc.finish() {
        stream.extend_from_slice(&pkt);
        frames += 1;
    }
    let mut s = String::new();
    write!(
        s,
        "{label:<22} frames={frames:<5} bytes={:<8} digest={:016x}",
        stream.len(),
        digest(&stream)
    )
    .unwrap();
    s
}

fn main() {
    println!("AC-3 golden bitstream digests");
    println!("-----------------------------");
    println!("{}", case("5.1 @ 640 kbps", 6, 48_000));
    println!("{}", case("5.0 @ 448 kbps", 5, 48_000));
    println!("{}", case("stereo @ 192 kbps", 2, 48_000));
    println!("{}", case("mono @ 96 kbps", 1, 48_000));
    println!("{}", case("stereo @ 44.1 kHz", 2, 44_100));
}

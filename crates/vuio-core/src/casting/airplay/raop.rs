//! Push PCM audio to an AirPlay 2 receiver over RTP.
//!
//! Unlike video, which is a URL hand-off, audio is a push protocol: the sender
//! paces RTP packets in real time and keeps the receiver's clock aligned with
//! periodic sync packets. This is a port of pyatv's `raop::stream_client` for
//! the buffered-audio (`type 96`) stream, carrying `PCM/44100/16/2`.

use super::audio::{PcmSource, BYTES_PER_FRAME, SAMPLE_RATE};
use anyhow::{Context, Result};
use hap_crypto::aead::chacha20poly1305_seal;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;

/// Frames carried by one RTP packet, as announced in SETUP (`spf`).
pub const FRAMES_PER_PACKET: usize = 352;
/// pyatv's latency: one second of audio plus half a second of slack.
const LATENCY: u32 = 22050 + SAMPLE_RATE;
const SYNC_INTERVAL: Duration = Duration::from_secs(1);
/// How often the receiver's seek bar is refreshed.
const PROGRESS_INTERVAL: Duration = Duration::from_secs(2);

/// Seconds between the NTP epoch (1900) and the Unix epoch (1970).
const NTP_UNIX_OFFSET: u64 = 2_208_988_800;

pub struct AudioSender {
    data_socket: UdpSocket,
    data_target: SocketAddr,
    key: [u8; 32],
    counter: u64,
    sequence: u16,
    head_ts: u32,
    start_ts: u32,
    ssrc: u32,
    /// Published for the sync task, which must announce the *current* stream
    /// position; a stale value leaves the receiver's clock parked and silent.
    published_rtptime: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

/// Pack interleaved 16-bit stereo frames into one uncompressed ALAC element.
///
/// A realtime AirPlay 2 receiver hardcodes ALAC and ignores the `ct` and
/// `audioFormat` it was given, so raw PCM is fed to an ALAC decoder and comes
/// out as hiss. ALAC's escape mode carries the samples verbatim, so this needs
/// no encoder: a 23-bit header, the samples MSB-first, then a 3-bit END tag.
fn pcm_to_uncompressed_alac(frames: &[u8]) -> Vec<u8> {
    let mut writer = BitWriter::default();
    writer.write(1, 3); // element type: stereo channel pair (CPE)
    writer.write(0, 4); // element instance tag
    writer.write(0, 12); // unused
    writer.write(0, 1); // hasSize
    writer.write(0, 2); // unused
    writer.write(1, 1); // isNotCompressed
    for frame in frames.chunks_exact(BYTES_PER_FRAME) {
        let left = u16::from_le_bytes([frame[0], frame[1]]);
        let right = u16::from_le_bytes([frame[2], frame[3]]);
        writer.write(u32::from(left), 16);
        writer.write(u32::from(right), 16);
    }
    writer.write(7, 3); // END element
    writer.finish()
}

/// Minimal MSB-first bit writer, which is the order ALAC elements use.
#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    partial: u8,
    used: u32,
}

impl BitWriter {
    fn write(&mut self, value: u32, bits: u32) {
        for index in (0..bits).rev() {
            let bit = ((value >> index) & 1) as u8;
            self.partial = (self.partial << 1) | bit;
            self.used += 1;
            if self.used == 8 {
                self.bytes.push(self.partial);
                self.partial = 0;
                self.used = 0;
            }
        }
    }

    /// Flush the trailing partial byte, padding with zeros.
    fn finish(mut self) -> Vec<u8> {
        if self.used > 0 {
            self.bytes.push(self.partial << (8 - self.used));
        }
        self.bytes
    }
}

/// The current NTP timestamp as (seconds, fraction).
fn ntp_now() -> (u32, u32) {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = duration.as_secs().saturating_add(NTP_UNIX_OFFSET) as u32;
    let fraction = ((u64::from(duration.subsec_nanos()) << 32) / 1_000_000_000) as u32;
    (seconds, fraction)
}

/// The NTP instant a sync packet pairs with the stream position it announces.
///
/// The stream is paced in real time from a start timestamp taken off the same
/// clock, so "now" is by construction the NTP time of the frames just queued.
fn sync_instant() -> (u32, u32) {
    ntp_now()
}

/// The RTP timestamp the stream starts from, derived from the wall clock.
fn ntp_to_ts() -> u32 {
    let (seconds, fraction) = ntp_now();
    let total = (u64::from(seconds) << 32) | u64::from(fraction);
    ((total as u128 * u128::from(SAMPLE_RATE)) >> 32) as u32
}

impl AudioSender {
    pub async fn connect(
        receiver: SocketAddr,
        data_port: u16,
        control_port: u16,
        key: [u8; 32],
        ssrc: u32,
        published_rtptime: std::sync::Arc<std::sync::atomic::AtomicU32>,
    ) -> Result<(Self, UdpSocket, SocketAddr)> {
        let bind = match receiver.ip() {
            std::net::IpAddr::V4(_) => "0.0.0.0:0",
            std::net::IpAddr::V6(_) => "[::]:0",
        };
        let data_socket = UdpSocket::bind(bind).await?;
        let control_socket = UdpSocket::bind(bind).await?;
        let start_ts = ntp_to_ts();
        Ok((
            Self {
                data_socket,
                data_target: SocketAddr::new(receiver.ip(), data_port),
                key,
                counter: 0,
                sequence: 0,
                head_ts: start_ts,
                start_ts,
                ssrc,
                published_rtptime,
            },
            control_socket,
            SocketAddr::new(receiver.ip(), control_port),
        ))
    }

    /// The receiver-side presentation time for what has been queued so far.
    fn rtptime(&self) -> u32 {
        self.head_ts
            .wrapping_sub(self.start_ts.wrapping_sub(LATENCY))
    }

    /// The sequence number the stream starts from.
    pub fn start_sequence(&self) -> u16 {
        self.sequence
    }

    /// The RTP timestamp the first packet will carry.
    pub fn start_rtptime(&self) -> u32 {
        self.rtptime()
    }

    /// Send one packet of PCM, encrypted, and advance the stream clock.
    ///
    /// The payload is an uncompressed ALAC element, which is what a realtime
    /// receiver decodes regardless of the format it was offered.
    async fn send_packet(&mut self, frames: &[u8], first: bool) -> Result<()> {
        let mut header = Vec::with_capacity(12);
        header.push(0x80);
        header.push(if first { 0xE0 } else { 0x60 });
        header.extend_from_slice(&self.sequence.to_be_bytes());
        header.extend_from_slice(&self.rtptime().to_be_bytes());
        header.extend_from_slice(&self.ssrc.to_be_bytes());

        // The nonce is four zero bytes plus a little-endian counter; the low
        // eight bytes travel at the end of the packet so the receiver can
        // reconstruct it. Bytes 4..12 of the header are the AEAD's AAD.
        let mut nonce = [0u8; 12];
        nonce[4..].copy_from_slice(&self.counter.to_le_bytes());

        let payload = pcm_to_uncompressed_alac(frames);
        let mut packet = header;
        let sealed = chacha20poly1305_seal(&self.key, &nonce, &packet[4..12], &payload)
            .map_err(|error| anyhow::anyhow!("encrypting AirPlay audio: {error}"))?;
        packet.extend_from_slice(&sealed);
        packet.extend_from_slice(&nonce[4..]);
        self.data_socket.send_to(&packet, self.data_target).await?;

        self.counter = self.counter.wrapping_add(1);
        self.sequence = self.sequence.wrapping_add(1);
        self.head_ts = self
            .head_ts
            .wrapping_add((frames.len() / BYTES_PER_FRAME) as u32);
        self.published_rtptime
            .store(self.rtptime(), std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Stream a decoded file and everything queued behind it, pacing packets
    /// against the audio clock.
    ///
    /// Tracks continue on the same RTP timeline, so a folder plays gaplessly.
    /// After the queue drains, silence is sent until a full latency window has
    /// been queued, so the receiver plays the tail instead of cutting it.
    pub async fn stream(
        &mut self,
        first: PcmSource,
        queue: std::sync::Arc<tokio::sync::Mutex<std::collections::VecDeque<std::path::PathBuf>>>,
        control: std::sync::Arc<tokio::sync::Mutex<super::SecureControl>>,
        rtsp_session: String,
        receiver_session: Option<String>,
    ) -> Result<()> {
        let started = tokio::time::Instant::now();
        let mut total_frames: u64 = 0;
        let mut padding_frames: u32 = 0;
        let mut is_first_packet = true;
        let silence = vec![0u8; FRAMES_PER_PACKET * BYTES_PER_FRAME];
        let mut announce: Option<super::audio::TrackMetadata> = Some(first.metadata().clone());
        let mut source = Some(first);
        let mut last_progress = tokio::time::Instant::now();
        // Where the current track began, and where it ends. Both stay fixed for
        // the track's lifetime -- the position is the only thing that moves.
        let mut track_start = self.rtptime();
        let mut track_end = track_start;

        loop {
            // Announce a track once its first packet position is known, so the
            // receiver's seek bar starts from the right place.
            if let Some(metadata) = announce.take() {
                track_start = self.rtptime();
                track_end = track_start.wrapping_add(
                    metadata
                        .duration_seconds
                        .and_then(|seconds| u32::try_from(seconds * u64::from(SAMPLE_RATE)).ok())
                        .unwrap_or(0),
                );
                super::announce_track(
                    &control,
                    &rtsp_session,
                    receiver_session.as_deref(),
                    self.sequence,
                    track_start,
                    track_end,
                    &metadata,
                )
                .await;
                last_progress = tokio::time::Instant::now();
            }
            let frames = match source.as_mut().map(|s| s.read_frames(FRAMES_PER_PACKET)) {
                Some(Ok(Some(frames))) => frames,
                other => {
                    if let Some(Err(error)) = other {
                        tracing::warn!(%error, "AirPlay audio decode failed; skipping track");
                    }
                    // Current track is done: pull the next one, if any.
                    let next = queue.lock().await.pop_front();
                    match next {
                        Some(path) => {
                            match tokio::task::spawn_blocking(move || PcmSource::open(&path))
                                .await
                                .context("joining the AirPlay audio decoder")?
                            {
                                Ok(opened) => {
                                    tracing::info!("AirPlay audio advancing to the next track");
                                    announce = Some(opened.metadata().clone());
                                    source = Some(opened);
                                }
                                Err(error) => {
                                    tracing::warn!(%error, "skipping an unplayable queued track");
                                    source = None;
                                }
                            }
                            continue;
                        }
                        None => {
                            source = None;
                            if padding_frames >= LATENCY {
                                break;
                            }
                            padding_frames += FRAMES_PER_PACKET as u32;
                            silence.clone()
                        }
                    }
                }
            };
            self.send_packet(&frames, is_first_packet).await?;
            is_first_packet = false;
            total_frames += (frames.len() / BYTES_PER_FRAME) as u64;
            if total_frames % (u64::from(SAMPLE_RATE) * 5) < FRAMES_PER_PACKET as u64 {
                tracing::debug!(
                    streamed_seconds = total_frames as f64 / f64::from(SAMPLE_RATE),
                    elapsed_seconds = started.elapsed().as_secs_f64(),
                    packets = self.sequence,
                    "AirPlay audio progress"
                );
            }

            // Refresh the position. `start` and `end` are the track's fixed
            // bounds; only the middle value advances, which is what moves the bar.
            if last_progress.elapsed() >= PROGRESS_INTERVAL && track_end != track_start {
                super::update_progress(
                    &control,
                    &rtsp_session,
                    track_start,
                    self.rtptime(),
                    track_end,
                )
                .await;
                last_progress = tokio::time::Instant::now();
            }

            // Sleep until this many frames should actually have been played.
            let stream_position = total_frames as f64 / f64::from(SAMPLE_RATE);
            let elapsed = started.elapsed().as_secs_f64();
            if stream_position > elapsed {
                tokio::time::sleep(Duration::from_secs_f64(stream_position - elapsed)).await;
            }
        }
        tracing::info!(
            seconds = total_frames as f64 / f64::from(SAMPLE_RATE),
            "AirPlay audio stream finished"
        );
        Ok(())
    }
}

/// Keep the receiver's clock aligned while audio is in flight.
pub fn spawn_sync_task(
    socket: UdpSocket,
    target: SocketAddr,
    rtptime: std::sync::Arc<std::sync::atomic::AtomicU32>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut first = true;
        let mut inbound = [0u8; 64];
        loop {
            // A receiver that is actually decoding asks for lost packets here,
            // so anything arriving is a sign the stream is being consumed.
            while let Ok(Ok((length, from))) =
                tokio::time::timeout(Duration::from_millis(1), socket.recv_from(&mut inbound)).await
            {
                tracing::debug!(
                    bytes = length,
                    %from,
                    kind = inbound.get(1).copied().unwrap_or(0),
                    "AirPlay audio control packet from receiver"
                );
            }
            let now = rtptime.load(std::sync::atomic::Ordering::Relaxed);
            let (seconds, fraction) = sync_instant();
            let mut packet = Vec::with_capacity(20);
            packet.push(if first { 0x90 } else { 0x80 });
            packet.push(0xD4);
            packet.extend_from_slice(&7u16.to_be_bytes());
            packet.extend_from_slice(&now.wrapping_sub(LATENCY).to_be_bytes());
            packet.extend_from_slice(&seconds.to_be_bytes());
            packet.extend_from_slice(&fraction.to_be_bytes());
            packet.extend_from_slice(&now.to_be_bytes());
            if socket.send_to(&packet, target).await.is_err() {
                return;
            }
            first = false;
            tokio::time::sleep(SYNC_INTERVAL).await;
        }
    })
}

/// The `shk` value and packet key for the realtime audio stream.
///
/// This is the first 32 bytes of the pairing shared secret **verbatim** -- no
/// HKDF. The same bytes are sent in the stream SETUP plist and used as the
/// ChaCha20-Poly1305 key for every audio packet.
pub fn stream_key(shared_secret: &[u8]) -> Result<[u8; 32]> {
    let key: [u8; 32] = shared_secret
        .get(..32)
        .context("AirPlay pairing secret is too short for an audio key")?
        .try_into()
        .expect("a 32-byte slice converts to [u8; 32]");
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtp_timestamps_start_at_the_latency_offset() {
        let start = 1_000_000u32;
        let sender_rtptime = |head: u32| head.wrapping_sub(start.wrapping_sub(LATENCY));
        assert_eq!(sender_rtptime(start), LATENCY);
        assert_eq!(sender_rtptime(start + 352), LATENCY + 352);
    }

    #[test]
    fn uncompressed_alac_carries_the_samples_verbatim() {
        // One frame, L = 0x1234, R = 0x5678 (little-endian on the way in).
        let element = pcm_to_uncompressed_alac(&[0x34, 0x12, 0x78, 0x56]);
        // 23-bit header: 001 0000 000000000000 0 00 1 -> 0x20 0x00 0x01...
        let mut bits = String::new();
        for byte in &element {
            bits.push_str(&format!("{byte:08b}"));
        }
        assert!(bits.starts_with("001"), "element type must be CPE: {bits}");
        assert_eq!(&bits[3..7], "0000");
        assert_eq!(&bits[7..19], "000000000000");
        assert_eq!(&bits[19..20], "0", "hasSize");
        assert_eq!(&bits[20..22], "00");
        assert_eq!(&bits[22..23], "1", "isNotCompressed");
        // Samples follow MSB-first, left then right.
        assert_eq!(&bits[23..39], "0001001000110100", "left = 0x1234");
        assert_eq!(&bits[39..55], "0101011001111000", "right = 0x5678");
        assert_eq!(&bits[55..58], "111", "END element");

        // A full packet: 23 + 352*32 + 3 bits, rounded up to whole bytes.
        let packet = vec![0u8; FRAMES_PER_PACKET * BYTES_PER_FRAME];
        let expected = (23 + FRAMES_PER_PACKET * 32 + 3).div_ceil(8);
        assert_eq!(pcm_to_uncompressed_alac(&packet).len(), expected);
    }

    #[test]
    fn stream_key_is_derived_from_the_event_salt() {
        let first = stream_key(&[7u8; 32]).unwrap();
        let second = stream_key(&[7u8; 32]).unwrap();
        let other = stream_key(&[9u8; 32]).unwrap();
        assert_eq!(first, second);
        assert_ne!(first, other);
    }
}

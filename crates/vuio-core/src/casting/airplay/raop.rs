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

/// Whether to encrypt the RTP payload with the key announced as `shk`.
///
/// Encrypting is the reference behaviour and the default. A receiver that
/// ignores `shk` would render the ciphertext as full-scale white noise, so
/// `VUIO_AIRPLAY_AUDIO_PLAIN=1` sends the PCM in the clear to rule that out.
fn encrypt_audio() -> bool {
    !std::env::var("VUIO_AIRPLAY_AUDIO_PLAIN")
        .is_ok_and(|value| matches!(value.trim(), "1" | "true" | "yes"))
}

/// Sample byte order on the wire.
///
/// Network audio is often big-endian; byte-swapped 16-bit PCM sounds like harsh
/// noise that still carries the rhythm of the track. Set
/// `VUIO_AIRPLAY_AUDIO_BE=1` to send big-endian samples.
pub fn big_endian_samples() -> bool {
    std::env::var("VUIO_AIRPLAY_AUDIO_BE")
        .is_ok_and(|value| matches!(value.trim(), "1" | "true" | "yes"))
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

        let mut packet = header;
        if encrypt_audio() {
            let sealed = chacha20poly1305_seal(&self.key, &nonce, &packet[4..12], frames)
                .map_err(|error| anyhow::anyhow!("encrypting AirPlay audio: {error}"))?;
            packet.extend_from_slice(&sealed);
            packet.extend_from_slice(&nonce[4..]);
        } else {
            // A receiver that ignores `shk` renders the payload as-is, so
            // ciphertext comes out as full-scale white noise.
            packet.extend_from_slice(frames);
        }
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
    ) -> Result<()> {
        let started = tokio::time::Instant::now();
        let mut total_frames: u64 = 0;
        let mut padding_frames: u32 = 0;
        let mut is_first_packet = true;
        let silence = vec![0u8; FRAMES_PER_PACKET * BYTES_PER_FRAME];
        let mut source = Some(first);

        loop {
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

/// The `shk` value and packet key for a buffered-audio stream.
pub fn stream_key(shared_secret: &[u8]) -> Result<[u8; 32]> {
    super::pair_verify::derive_key_from(
        shared_secret,
        b"Events-Salt",
        b"Events-Write-Encryption-Key",
    )
    .context("deriving the AirPlay audio stream key")
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
    fn stream_key_is_derived_from_the_event_salt() {
        let first = stream_key(&[7u8; 32]).unwrap();
        let second = stream_key(&[7u8; 32]).unwrap();
        let other = stream_key(&[9u8; 32]).unwrap();
        assert_eq!(first, second);
        assert_ne!(first, other);
    }
}

//! The 44-byte canonical RIFF/WAVE header.
//!
//! Hand-written rather than muxed. The decoders emit interleaved little-endian
//! S16, which is exactly a WAV `data` chunk's payload, so the whole container is
//! this header followed by a copy — and we already hand-write fMP4 boxes in
//! [`crate::media::remux::fmp4_writer`] for the same reason.
//!
//! Fixed-layout on purpose: a renderer asking for a byte range needs the header
//! length to be a constant it can subtract, and PCM's constant bitrate is what
//! makes the whole resource seekable.

/// Length of the header [`wav_header`] writes. PCM data begins here.
pub const WAV_HEADER_LEN: u64 = 44;

/// Bytes per sample per channel. S16 throughout — it is what the decoders emit
/// and what every renderer that accepts LPCM accepts.
pub(crate) const BYTES_PER_SAMPLE: u64 = 2;

/// Total size of the WAV resource carrying `total_samples` frames of `channels`.
///
/// `total_samples` counts sample *frames* (one per channel-tuple), matching
/// `AudioFrame::samples`, so a stereo second at 48 kHz is 48 000 — not 96 000.
pub(crate) fn wav_size(total_samples: u64, channels: u16) -> u64 {
    WAV_HEADER_LEN + pcm_size(total_samples, channels)
}

/// Size of the PCM payload alone.
pub(crate) fn pcm_size(total_samples: u64, channels: u16) -> u64 {
    total_samples * channels as u64 * BYTES_PER_SAMPLE
}

/// Build the header for a stream of `total_samples` frames.
///
/// RIFF sizes are 32-bit, so a stream whose payload will not fit is described
/// with the largest size the format can express rather than a wrapped one. That
/// is ~6.2 hours of 48 kHz stereo; past it a player sees a truncated duration
/// instead of a corrupt one, which is the better of the two failures. RF64 would
/// lift the limit and is not worth a container dependency for a case no real
/// library hits.
pub fn wav_header(sample_rate: u32, channels: u16, total_samples: u64) -> [u8; 44] {
    let data_len = u32::try_from(pcm_size(total_samples, channels)).unwrap_or(u32::MAX - 36);
    let byte_rate = sample_rate as u64 * channels as u64 * BYTES_PER_SAMPLE;
    let block_align = channels * BYTES_PER_SAMPLE as u16;

    let mut h = [0u8; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(36 + data_len).to_le_bytes());
    h[8..12].copy_from_slice(b"WAVE");
    h[12..16].copy_from_slice(b"fmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk length
    h[20..22].copy_from_slice(&1u16.to_le_bytes()); // WAVE_FORMAT_PCM
    h[22..24].copy_from_slice(&channels.to_le_bytes());
    h[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    h[28..32].copy_from_slice(&(byte_rate.min(u32::MAX as u64) as u32).to_le_bytes());
    h[32..34].copy_from_slice(&block_align.to_le_bytes());
    h[34..36].copy_from_slice(&16u16.to_le_bytes()); // bits per sample
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data_len.to_le_bytes());
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le32(h: &[u8; 44], at: usize) -> u32 {
        u32::from_le_bytes([h[at], h[at + 1], h[at + 2], h[at + 3]])
    }
    fn le16(h: &[u8; 44], at: usize) -> u16 {
        u16::from_le_bytes([h[at], h[at + 1]])
    }

    #[test]
    fn describes_one_second_of_48k_stereo() {
        let h = wav_header(48_000, 2, 48_000);
        assert_eq!(&h[0..4], b"RIFF");
        assert_eq!(&h[8..12], b"WAVE");
        assert_eq!(&h[36..40], b"data");
        assert_eq!(le32(&h, 40), 48_000 * 2 * 2, "payload bytes");
        assert_eq!(le32(&h, 4), 36 + 48_000 * 2 * 2, "riff size excludes its own 8");
        assert_eq!(le16(&h, 20), 1, "WAVE_FORMAT_PCM");
        assert_eq!(le16(&h, 22), 2, "channels");
        assert_eq!(le32(&h, 24), 48_000, "sample rate");
        assert_eq!(le32(&h, 28), 48_000 * 2 * 2, "byte rate");
        assert_eq!(le16(&h, 32), 4, "block align");
        assert_eq!(le16(&h, 34), 16, "bits per sample");
    }

    #[test]
    fn header_length_and_total_size_agree_with_the_declared_payload() {
        let h = wav_header(44_100, 2, 44_100 * 3);
        assert_eq!(h.len() as u64, WAV_HEADER_LEN);
        assert_eq!(
            wav_size(44_100 * 3, 2),
            WAV_HEADER_LEN + le32(&h, 40) as u64
        );
    }

    #[test]
    fn a_payload_too_large_for_riff_saturates_rather_than_wrapping() {
        // ~6.2 h of 48 kHz stereo is where a 32-bit size runs out. The header
        // must not describe a wrapped, far-too-small payload.
        let huge = u64::MAX / 8;
        let h = wav_header(48_000, 2, huge);
        assert_eq!(le32(&h, 40), u32::MAX - 36);
        assert_eq!(le32(&h, 4), u32::MAX, "riff size stays consistent with data");
    }

    #[test]
    fn mono_and_stereo_sizes_differ_by_exactly_the_channel_count() {
        assert_eq!(pcm_size(1_000, 1) * 2, pcm_size(1_000, 2));
    }
}

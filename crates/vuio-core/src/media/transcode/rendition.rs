//! Restating a film's undecodable audio track as an AAC one.
//!
//! Both delivery paths need the same thing and must do it identically: the
//! browser's HLS renditions and the television's progressive `video.mp4` each
//! carry the film's picture untouched beside an audio track that had to be
//! decoded and re-encoded to exist at all. What differs between them is
//! framing and headers; what happens to the samples is here.
//!
//! The one number worth understanding is the encoder's delay. Its MDCT window
//! spans the previous hop and the current one, so the frame emitted for input
//! samples `[n, n+1024)` is only fully reconstructed once the *next* frame has
//! been overlap-added — a decoder's output therefore trails its input by
//! exactly one frame. Placing the run 1024 samples earlier on the decode
//! timeline cancels that, and is the difference between lip-sync and a
//! twenty-one millisecond lag.

use anyhow::Result;

use super::{AacEncoder, PcmDecoder, TranscodeCodec};
use crate::media::remux::MediaPacket;

/// Samples per AAC-LC frame, and therefore the encoder's delay.
pub const AAC_FRAME_SAMPLES: u64 = 1024;

/// Bytes per sample per channel in the decoder's output.
const BYTES_PER_SAMPLE: usize = 2;

/// Decode `packets` and re-encode them as AAC, as samples for an MP4 track.
///
/// `packets` must be one track's packets in order, with timestamps already in
/// the output timescale — which for an audio track is its sample rate, so a
/// timestamp *is* a sample index. `channels` is what the output track declares,
/// and what the samples are made to match: a mono source asked to be stereo is
/// widened here rather than being allowed to contradict the `esds` box that has
/// already gone out in the init segment.
///
/// The returned packets carry `pts == dts` and a duration of one AAC frame.
/// Audio has no reordering, so there is nothing for a composition offset to
/// express.
pub fn reencode_to_aac(
    codec: TranscodeCodec,
    packets: &[MediaPacket],
    sample_rate: u32,
    channels: u16,
    track_id: u32,
) -> Result<Vec<MediaPacket>> {
    let Some(first) = packets.first() else {
        return Ok(Vec::new());
    };

    let (mut decoder, primed) =
        PcmDecoder::open(codec, sample_rate, Some(channels), &first.data)?;
    let decoded_channels = decoder.channels();
    let mut encoder = AacEncoder::new(sample_rate, channels)?;

    let mut adts = encoder.push(&fit_channels(&primed, decoded_channels, channels))?;
    for packet in &packets[1..] {
        let expect = super::frames::frame_samples(codec, &packet.data);
        let pcm = decoder.decode_or_silence(&packet.data, expect);
        adts.extend_from_slice(&encoder.push(&fit_channels(&pcm, decoded_channels, channels))?);
    }
    adts.extend_from_slice(&encoder.finish());

    // One frame early, to cancel the encoder's delay. Clamped at zero for a run
    // that already starts at the beginning of the film, where there is no
    // earlier timeline to move onto — the residual lag there is one frame,
    // twenty-one milliseconds at 48 kHz.
    let mut dts = first.pts.saturating_sub(AAC_FRAME_SAMPLES);
    let mut out = Vec::new();
    for payload in super::adts_payloads(&adts) {
        out.push(MediaPacket {
            track_id,
            pts: dts,
            dts,
            duration: AAC_FRAME_SAMPLES,
            // Every AAC frame is independently decodable after the previous
            // frame's overlap, so every one is a random-access point.
            is_keyframe: true,
            data: payload.to_vec(),
        });
        dts += AAC_FRAME_SAMPLES;
    }
    Ok(out)
}

/// Fit interleaved S16 with `have` channels into `want` channels.
///
/// The decoder is asked for the channel count the output declares and normally
/// obliges — AC-3 carries the §7.8 downmix coefficients, so its own two-channel
/// output is the mix the encoder authored. A source already narrower than the
/// request (mono AC-3 does exist) comes back narrower, and is widened here by
/// duplication so that what reaches the encoder always matches what the `esds`
/// box promised.
pub fn fit_channels(pcm: &[u8], have: u16, want: u16) -> Vec<u8> {
    if have == want || have == 0 || want == 0 {
        return pcm.to_vec();
    }
    let have = have as usize;
    let want = want as usize;
    let frames = pcm.len() / (have * BYTES_PER_SAMPLE);
    let mut out = Vec::with_capacity(frames * want * BYTES_PER_SAMPLE);
    for frame in 0..frames {
        let base = frame * have * BYTES_PER_SAMPLE;
        for channel in 0..want {
            // Widening repeats the last channel there is; narrowing takes the
            // leading ones, which for interleaved audio is the front pair.
            let source = channel.min(have - 1);
            let at = base + source * BYTES_PER_SAMPLE;
            out.extend_from_slice(&pcm[at..at + BYTES_PER_SAMPLE]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_is_widened_by_duplication_and_the_frame_count_is_kept() {
        let mono: Vec<u8> = [1i16, 2, 3]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let stereo = fit_channels(&mono, 1, 2);
        assert_eq!(stereo.len(), mono.len() * 2);
        let values: Vec<i16> = stereo
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| i16::from_le_bytes(*c))
            .collect();
        assert_eq!(values, vec![1, 1, 2, 2, 3, 3]);
    }

    #[test]
    fn a_matching_channel_count_is_passed_through_untouched() {
        let pcm = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(fit_channels(&pcm, 2, 2), pcm);
    }

    #[test]
    fn narrowing_keeps_the_leading_channels() {
        let five_one: Vec<u8> = (1i16..=6).flat_map(|v| v.to_le_bytes()).collect();
        let stereo = fit_channels(&five_one, 6, 2);
        let values: Vec<i16> = stereo
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| i16::from_le_bytes(*c))
            .collect();
        assert_eq!(values, vec![1, 2]);
    }
}

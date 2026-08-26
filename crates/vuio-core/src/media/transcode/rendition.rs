//! Restating a film's undecodable audio track as an AAC one.
//!
//! Both delivery paths need the same thing and must do it identically: the
//! browser's HLS renditions and the television's progressive `video.mp4` each
//! carry the film's picture untouched beside an audio track that had to be
//! decoded and re-encoded to exist at all. What differs between them is
//! framing and headers; what happens to the samples is here.
//!
//! Two numbers govern all of it.
//!
//! The first is [`ENCODER_DELAY`]. The encoder's MDCT window spans the previous
//! hop and the current one, so what a decoder reconstructs trails what the
//! encoder was fed — measured end to end at 1600 samples, constant across every
//! rate and channel count the encoder accepts. Placing the run that many
//! samples earlier on the decode timeline is the difference between lip-sync
//! and a thirty-three millisecond lag.
//!
//! The second is that an AAC frame is 1024 samples and a segment is four
//! seconds, and 192000 is not a multiple of 1024. A segment that simply encodes
//! its own four seconds and starts the run at its nominal decode time therefore
//! runs 512 samples past where the next segment begins, and the two collide in
//! the player's source buffer — once every four seconds, for the length of the
//! film. So a segment does not own a duration here; it owns a stretch of the
//! film-wide frame grid ([`AacWindow`]), and consecutive windows meet exactly
//! because each starts where the arithmetic says the previous one stopped.

use anyhow::Result;

use super::{AacEncoder, PcmDecoder, TranscodeCodec};
use crate::media::remux::MediaPacket;

/// Samples per AAC-LC frame.
pub const AAC_FRAME_SAMPLES: u64 = 1024;

/// Samples by which a decoder's output trails what the encoder was fed.
///
/// Measured, not assumed: an impulse train through this encoder and back out of
/// a reference decoder comes back 1600 samples late, at 32, 44.1 and 48 kHz and
/// in mono and stereo alike. It is not a multiple of the frame length, which is
/// why cancelling it is done by moving the *input* window rather than by
/// shifting whole frames.
pub const ENCODER_DELAY: u64 = 1600;

/// Frames fed to the encoder before the window's first, and then discarded.
///
/// Enough to cover [`ENCODER_DELAY`] and leave the first kept frame's MDCT
/// window filled with real audio rather than with the silence a cold encoder
/// starts from — without which every segment boundary would carry the encoder's
/// warm-up transient.
const PREROLL_FRAMES: u64 = 4;

/// Bytes per sample per channel in the decoder's output.
const BYTES_PER_SAMPLE: usize = 2;

/// The stretch of the film-wide AAC frame grid one segment owns.
///
/// Frames sit at multiples of [`AAC_FRAME_SAMPLES`] measured from the start of
/// the film, never from the start of the segment. That is the whole trick: two
/// segments computed independently, in different requests, on different
/// threads, cannot overlap or leave a gap, because neither of them is choosing
/// where its frames go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AacWindow {
    first_frame: u64,
    frames: u64,
}

impl AacWindow {
    /// The window covering `[start_sample, end_sample)` of the film.
    ///
    /// Rounding both ends up to a frame boundary is what makes the windows
    /// tile: this window's end is the next one's start, by construction.
    pub fn covering(start_sample: u64, end_sample: u64) -> Self {
        let first_frame = start_sample.div_ceil(AAC_FRAME_SAMPLES);
        Self {
            first_frame,
            frames: end_sample
                .div_ceil(AAC_FRAME_SAMPLES)
                .saturating_sub(first_frame),
        }
    }

    /// Decode time of the window's first sample.
    pub fn start_sample(&self) -> u64 {
        self.first_frame * AAC_FRAME_SAMPLES
    }

    /// The source samples the encoder has to be fed to produce this window:
    /// where to start on the film's timeline, and how many.
    ///
    /// The start is signed because the film's first window asks for audio from
    /// before the film — [`reencode_to_aac`] answers that with silence, which is
    /// what the encoder would have warmed up on anyway.
    pub fn source_span(&self) -> (i64, u64) {
        let from = self.start_sample() as i64 + ENCODER_DELAY as i64
            - (PREROLL_FRAMES * AAC_FRAME_SAMPLES) as i64;
        (from, (PREROLL_FRAMES + self.frames) * AAC_FRAME_SAMPLES)
    }
}

/// Decode `packets` and re-encode `window` of them as AAC, as MP4 samples.
///
/// `packets` must be one track's packets in order, with timestamps already in
/// the output timescale — which for an audio track is its sample rate, so a
/// timestamp *is* a sample index, and between them they say where the decoded
/// run sits on the film's timeline (see [`run_anchor`]). They must cover
/// [`AacWindow::source_span`]; anything they do not reach, at either end, is
/// silence. `channels` is what
/// the output track declares, and what the samples are made to match: a mono
/// source asked to be stereo is widened here rather than being allowed to
/// contradict the `esds` box that has already gone out in the init segment.
///
/// Exactly the window's frames come back, at exactly the window's decode times,
/// whatever the source's own framing was: the encoder is fed a run positioned
/// by absolute sample index, and its warm-up frames are dropped rather than
/// shipped. The returned packets carry `pts == dts` and a duration of one AAC
/// frame — audio has no reordering, so there is nothing for a composition
/// offset to express.
pub fn reencode_to_aac(
    codec: TranscodeCodec,
    packets: &[MediaPacket],
    sample_rate: u32,
    channels: u16,
    track_id: u32,
    window: AacWindow,
) -> Result<Vec<MediaPacket>> {
    let Some(first) = packets.first() else {
        return Ok(Vec::new());
    };

    let (mut decoder, primed) = PcmDecoder::open(codec, sample_rate, Some(channels), &first.data)?;
    let decoded_channels = decoder.channels();
    let source_frame_bytes = decoded_channels as usize * BYTES_PER_SAMPLE;

    // One contiguous run of PCM, and every packet's own account of where that
    // run begins. Each frame contributes exactly the sample count its own
    // header promised, so a frame that fails to decode costs its own duration
    // and does not shift everything after it off the timeline.
    let mut pcm = pad_to(
        primed,
        super::frames::frame_samples(codec, &first.data),
        decoded_channels,
    );
    let mut anchors = vec![first.pts as i64];
    for packet in &packets[1..] {
        anchors.push(packet.pts as i64 - (pcm.len() / source_frame_bytes) as i64);
        let expect = super::frames::frame_samples(codec, &packet.data);
        pcm.extend_from_slice(&decoder.decode_or_silence(&packet.data, expect));
    }
    let pcm = fit_channels(&pcm, decoded_channels, channels);

    let (from, len) = window.source_span();
    let mut encoder = AacEncoder::new(sample_rate, channels)?;
    let mut adts = encoder.push(&sample_range(
        &pcm,
        run_anchor(&mut anchors),
        from,
        len,
        channels as usize * BYTES_PER_SAMPLE,
    ))?;
    adts.extend_from_slice(&encoder.finish());

    let mut dts = window.start_sample();
    let mut out = Vec::new();
    for payload in super::adts_payloads(&adts)
        .into_iter()
        .skip(PREROLL_FRAMES as usize)
        .take(window.frames as usize)
    {
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

/// Where the decoded run begins on the film's timeline.
///
/// Not simply the first packet's timestamp. Matroska stores block timestamps in
/// milliseconds, and a muxer writing 512-sample DTS frames — ten and two thirds
/// milliseconds each — has to round every one of them. On real files the result
/// wanders as much as seventy-five milliseconds either side of where the audio
/// actually is, while the frames themselves stay perfectly contiguous. A
/// segment anchored on one such timestamp is placed that far out, and both of
/// its joins are heard as a jump.
///
/// So every packet is asked the same question instead — "if this run is
/// contiguous, where does it start?" — and the answer taken from the upper part
/// of the spread, because the noise is one-sided: rounding a timestamp down and
/// a muxer running behind both push an estimate low, and nothing pushes it
/// high. The maximum would be the estimate the rounding alone implies, but one
/// spurious timestamp would then set the answer, so this stops short of it.
///
/// A track whose timestamps are exact — AC-3 at 1536 samples a frame divides
/// into milliseconds, and every packet then agrees — is unaffected: all the
/// estimates are the same number, and every quantile of them is that number.
pub fn run_anchor(estimates: &mut [i64]) -> i64 {
    estimates.sort_unstable();
    estimates[(estimates.len() - 1) * ANCHOR_QUANTILE.0 / ANCHOR_QUANTILE.1]
}

/// The quantile [`run_anchor`] reads the anchor off at, as a fraction.
///
/// Measured against a sequential read of a real DTS track: at three quarters
/// the estimate is out by thirteen samples on average and never by more than
/// twenty-two milliseconds, where the first timestamp alone averaged six
/// hundred and eighty and reached seventy-five.
const ANCHOR_QUANTILE: (usize, usize) = (3, 4);

/// Pad one decoded frame out to the sample count its header declared.
fn pad_to(mut pcm: Vec<u8>, samples: Option<u32>, channels: u16) -> Vec<u8> {
    if let Some(samples) = samples {
        pcm.resize(samples as usize * channels as usize * BYTES_PER_SAMPLE, 0);
    }
    pcm
}

/// `len` sample frames of `pcm` from absolute position `from`, where `pcm`
/// itself begins at absolute position `pcm_start`.
///
/// Silence stands in for anything outside what `pcm` holds. That is not a
/// failure case: the film's first window reaches back before the film to prime
/// the encoder, and its last reaches past the end for the same reason.
fn sample_range(pcm: &[u8], pcm_start: i64, from: i64, len: u64, frame_bytes: usize) -> Vec<u8> {
    let mut out = vec![0u8; len as usize * frame_bytes];
    let available = (pcm.len() / frame_bytes) as i64;
    let begin = from.max(pcm_start);
    let end = (from + len as i64).min(pcm_start + available);
    if end > begin {
        let src = (begin - pcm_start) as usize * frame_bytes;
        let at = (begin - from) as usize * frame_bytes;
        let take = (end - begin) as usize * frame_bytes;
        out[at..at + take].copy_from_slice(&pcm[src..src + take]);
    }
    out
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

    /// The arithmetic the whole segmented path rests on: run the windows a
    /// player would actually request and check that they lay end to end.
    #[test]
    fn consecutive_windows_meet_without_a_gap_or_an_overlap() {
        const RATE: u64 = 48_000;
        const SEGMENT: u64 = 4 * RATE;
        // 192000 is 187.5 frames, so every other boundary falls mid-frame —
        // which is the case a naive "encode my own four seconds" gets wrong.
        let mut expected = 0;
        for seq in 0..64u64 {
            let window = AacWindow::covering(seq * SEGMENT, (seq + 1) * SEGMENT);
            assert_eq!(window.start_sample(), expected, "window {seq} does not open where {} closed", seq.saturating_sub(1));
            expected = window.start_sample() + window.frames * AAC_FRAME_SAMPLES;
        }
        // And they keep time: sixty-four segments of four seconds, to within
        // the frame the grid rounds by.
        assert!(expected.abs_diff(64 * SEGMENT) < AAC_FRAME_SAMPLES);
    }

    /// The encoder is fed from before the window and past it, or its first kept
    /// frame opens on silence and its last is never finished.
    #[test]
    fn the_source_span_brackets_the_window_it_produces() {
        let window = AacWindow::covering(4 * 48_000, 8 * 48_000);
        let (from, len) = window.source_span();
        assert!(from < window.start_sample() as i64, "no pre-roll");
        let ends_at = from + len as i64;
        let window_ends = (window.start_sample() + window.frames * AAC_FRAME_SAMPLES) as i64;
        assert_eq!(
            ends_at - window_ends,
            ENCODER_DELAY as i64,
            "the run must reach exactly the encoder's delay past its last output sample"
        );
    }

    /// The film's first window reaches back before the film, and its last past
    /// the end. Neither is an error, and neither may shift what is there.
    #[test]
    fn a_span_reaching_outside_the_decoded_run_is_padded_with_silence() {
        // Four stereo sample frames, at absolute position 100.
        let pcm: Vec<u8> = (1i16..=8).flat_map(|v| v.to_le_bytes()).collect();
        let out = sample_range(&pcm, 100, 98, 8, 4);
        let values: Vec<i16> = out
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| i16::from_le_bytes(*c))
            .collect();
        assert_eq!(
            values,
            vec![0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0],
            "the run has to sit at its own offset inside the padding"
        );
    }

    /// A track whose timestamps are exact is left exactly where they put it.
    #[test]
    fn an_unjittered_run_anchors_on_its_own_timestamps() {
        let mut anchors = vec![7_680; 128];
        assert_eq!(run_anchor(&mut anchors), 7_680);
    }

    /// One timestamp seventy-five milliseconds out of place must not move the
    /// run, which is the defect a browser hears as a jump at both its joins.
    #[test]
    fn a_stray_timestamp_does_not_move_the_run() {
        let mut anchors = vec![7_680; 128];
        anchors[0] -= 3_616;
        assert_eq!(run_anchor(&mut anchors), 7_680);
    }

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

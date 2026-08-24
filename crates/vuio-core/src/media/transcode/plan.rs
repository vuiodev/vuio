//! What a transcoded resource will be, resolved before a byte of it is sent.
//!
//! A DLNA renderer asks for the size first and the bytes second, often from a
//! different connection, and it will not tolerate the two disagreeing. So
//! everything the response's shape depends on — total samples, sample rate,
//! channel count — is settled up front, here, and the streaming half only ever
//! fills in a length that was already promised.
//!
//! Building a plan costs one pass over an elementary file's headers, or one
//! container probe, plus one decoded frame. That is why plans are cached (see
//! [`super::session`]): a renderer's `HEAD`, `GET` and range requests for one
//! file should pay it once.

use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use super::source::PacketSource;
use super::wav::{pcm_size, wav_size, WAV_HEADER_LEN};
use super::{FrameIndex, PcmDecoder, TranscodeCodec};

/// The decoded shape of one file, and how to reach the frames that produce it.
pub struct AudioPlan {
    /// The file this plan describes.
    ///
    /// Carried because the plan outlives the request that built it — it is
    /// cached by file id — and the streaming half re-opens the file to read
    /// frames rather than holding a handle for the life of the cache entry.
    pub source_path: std::path::PathBuf,
    /// Codec of the source.
    pub codec: TranscodeCodec,
    /// Where the compressed frames come from, and what they add up to.
    pub source: PacketSource,
    /// Channels the decoder emits — measured, not predicted.
    pub channels: u16,
}

impl std::fmt::Debug for AudioPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioPlan")
            .field("source_path", &self.source_path)
            .field("codec", &self.codec)
            .field("sample_rate", &self.sample_rate())
            .field("channels", &self.channels)
            .field("total_samples", &self.total_samples())
            .finish()
    }
}

/// Channels to ask the decoder for.
///
/// Stereo, always. A renderer that cannot decode AC-3 is not a renderer with a
/// 5.1 speaker set waiting behind it, and asking the decoder for two channels
/// gets the §7.8 downmix the encoder authored rather than one we invent. A
/// source already at or below stereo is unaffected — the decoder reports what it
/// actually emitted and [`AudioPlan::channels`] records that.
pub(crate) const TARGET_CHANNELS: u16 = 2;

impl AudioPlan {
    /// Index a raw `.ac3`/`.eac3`/`.dts` file and probe its decoded shape.
    ///
    /// Blocking: it reads the whole file's headers and decodes one frame, so
    /// callers on an async task must wrap it in `spawn_blocking`.
    pub fn elementary(path: &Path, codec: TranscodeCodec) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("opening {} for transcoding", path.display()))?;
        let mut reader = BufReader::with_capacity(256 * 1024, file);
        let index = FrameIndex::build(codec, &mut reader)?;

        // Re-open at the first frame to probe the decoder's output shape.
        let first = index.frames[0];
        let mut file = reader.into_inner();
        file.seek(SeekFrom::Start(first.offset))
            .context("seeking to the first frame")?;
        let mut buf = vec![0u8; first.len as usize];
        file.read_exact(&mut buf)
            .context("reading the first frame")?;

        let (decoder, _) = PcmDecoder::open(codec, index.sample_rate, Some(TARGET_CHANNELS), &buf)?;

        Ok(Self {
            source_path: path.to_path_buf(),
            codec,
            channels: decoder.channels(),
            source: PacketSource::Elementary(index),
        })
    }

    /// Probe the audio track of a container — a film — and its decoded shape.
    ///
    /// Blocking, and much cheaper than the elementary path: the container's own
    /// track declarations answer everything the header walk had to be run to
    /// find out, so this reads the file's front matter and one packet.
    #[cfg(feature = "demux")]
    pub fn container(path: &Path) -> Result<Self> {
        let (mut format, audio, codec) = super::source::probe_container_audio(path)?;
        let (first, _) = super::source::next_track_packet(format.as_mut(), audio.track_id)?
            .ok_or_else(|| anyhow::anyhow!("{} has no audio packets", path.display()))?;
        let (decoder, _) =
            PcmDecoder::open(codec, audio.sample_rate, Some(TARGET_CHANNELS), &first)?;

        Ok(Self {
            source_path: path.to_path_buf(),
            codec,
            channels: decoder.channels(),
            source: PacketSource::Container(audio),
        })
    }

    /// Total decoded sample frames, when they can be known before decoding.
    pub fn total_samples(&self) -> Option<u64> {
        self.source.total_samples()
    }

    /// Total size of the WAV resource, header included.
    ///
    /// `None` when the source will not say how long it is. The resource then has
    /// no `Content-Length` and no seeking — an honest loss, where a guessed
    /// length would be a truncated download.
    pub fn wav_size(&self) -> Option<u64> {
        self.total_samples()
            .map(|samples| wav_size(samples, self.channels))
    }

    /// Sample rate of the decoded output, in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }

    /// Duration in seconds, when the length is known.
    pub fn duration_secs(&self) -> Option<f64> {
        let rate = self.sample_rate();
        self.total_samples()
            .filter(|_| rate > 0)
            .map(|samples| samples as f64 / f64::from(rate))
    }

    /// The WAV header describing this resource.
    ///
    /// A source of unknown length is described with the largest payload RIFF can
    /// express, which is what a player shows as "unknown" rather than as zero.
    pub fn wav_header(&self) -> [u8; 44] {
        super::wav_header(
            self.sample_rate(),
            self.channels,
            self.total_samples().unwrap_or(u64::MAX / 8),
        )
    }

    /// Bytes each decoded sample frame occupies.
    pub fn stride(&self) -> u64 {
        self.channels as u64 * 2
    }

    /// Turn a byte offset into the resource into where decoding starts.
    ///
    /// Offsets inside the 44-byte header resolve to the very start, because a
    /// range that begins mid-header still has to be served the rest of it.
    pub fn seek(&self, byte_offset: u64) -> Seeked {
        if byte_offset < WAV_HEADER_LEN {
            return Seeked {
                header_skip: byte_offset as usize,
                start_sample: 0,
                byte_skip: 0,
            };
        }
        let pcm_offset = byte_offset - WAV_HEADER_LEN;
        Seeked {
            header_skip: WAV_HEADER_LEN as usize,
            start_sample: pcm_offset / self.stride(),
            byte_skip: (pcm_offset % self.stride()) as usize,
        }
    }

    /// Decoded bytes `samples` sample frames occupy at this plan's channel count.
    pub fn pcm_bytes(&self, samples: u64) -> u64 {
        pcm_size(samples, self.channels)
    }
}

/// Where a byte offset lands in the resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seeked {
    /// Bytes of the WAV header already passed — 44 once past it entirely.
    pub header_skip: usize,
    /// The decoded sample frame output begins at.
    pub start_sample: u64,
    /// Bytes to drop from that sample frame, for a range beginning mid-sample.
    pub byte_skip: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "transcode-ac3")]
    const AC3_FIXTURE: &[u8] =
        include_bytes!("../../../../vendor/oxideav-ac3/tests/fixtures/sine440_stereo.ac3");

    #[cfg(feature = "transcode-ac3")]
    fn fixture_plan() -> (AudioPlan, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sine.ac3");
        std::fs::write(&path, AC3_FIXTURE).unwrap();
        (
            AudioPlan::elementary(&path, TranscodeCodec::Ac3).unwrap(),
            dir,
        )
    }

    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn a_plan_describes_the_whole_resource_before_anything_is_decoded() {
        let (plan, _dir) = fixture_plan();
        assert_eq!(plan.channels, 2);
        assert_eq!(plan.sample_rate(), 48_000);
        assert_eq!(
            plan.wav_size(),
            Some(WAV_HEADER_LEN + plan.total_samples().unwrap() * 2 * 2)
        );
    }

    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn offset_zero_starts_at_the_header_and_the_first_sample() {
        let (plan, _dir) = fixture_plan();
        assert_eq!(
            plan.seek(0),
            Seeked {
                header_skip: 0,
                start_sample: 0,
                byte_skip: 0
            }
        );
    }

    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn a_range_inside_the_header_still_gets_the_rest_of_it() {
        let (plan, _dir) = fixture_plan();
        let s = plan.seek(20);
        assert_eq!(s.header_skip, 20);
        assert_eq!(s.start_sample, 0);
        assert_eq!(s.byte_skip, 0);
    }

    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn a_range_past_the_header_lands_on_the_right_sample() {
        let (plan, _dir) = fixture_plan();
        // One whole AC-3 frame of decoded stereo is 1536 * 2 * 2 bytes.
        let one_frame = 1536 * 2 * 2;
        let s = plan.seek(WAV_HEADER_LEN + one_frame);
        assert_eq!(s.header_skip, WAV_HEADER_LEN as usize, "header is done");
        assert_eq!(s.start_sample, 1536, "exactly the second frame's first sample");
        assert_eq!(s.byte_skip, 0);

        // Half a frame in, and one byte past a sample boundary: the sample is
        // rounded down and the odd bytes are dropped from its front.
        let s = plan.seek(WAV_HEADER_LEN + one_frame + one_frame / 2 + 1);
        assert_eq!(s.start_sample, 1536 + 1536 / 2);
        assert_eq!(s.byte_skip, 1);
    }

    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn the_declared_size_is_the_payload_the_samples_imply() {
        let (plan, _dir) = fixture_plan();
        let samples = plan.total_samples().unwrap();
        assert_eq!(
            plan.wav_size().unwrap(),
            WAV_HEADER_LEN + plan.pcm_bytes(samples)
        );
    }
}

//! What a transcoded resource will be, resolved before a byte of it is sent.
//!
//! A DLNA renderer asks for the size first and the bytes second, often from a
//! different connection, and it will not tolerate the two disagreeing. So
//! everything the response's shape depends on — total samples, sample rate,
//! channel count — is settled up front, here, and the streaming half only ever
//! fills in a length that was already promised.
//!
//! Building a plan costs one pass over the file's headers plus one decoded
//! frame. That is why plans are cached (see [`super::session`]): a renderer's
//! `HEAD`, `GET` and range requests for one file should pay it once.

use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use super::wav::{pcm_size, wav_size, WAV_HEADER_LEN};
use super::{FrameIndex, PcmDecoder, TranscodeCodec};

/// The decoded shape of one file, and the frame table to produce it.
#[derive(Debug)]
pub struct AudioPlan {
    /// The file this plan describes.
    ///
    /// Carried because the plan outlives the request that built it — it is
    /// cached by file id — and the streaming half re-opens the file to read
    /// frames rather than holding a handle for the life of the cache entry.
    pub source_path: std::path::PathBuf,
    /// Codec of the source.
    pub codec: TranscodeCodec,
    /// Where every frame is and how long it decodes to.
    pub index: FrameIndex,
    /// Channels the decoder emits — measured, not predicted.
    pub channels: u16,
}

/// Channels to ask the decoder for.
///
/// Stereo, always. A renderer that cannot decode AC-3 is not a renderer with a
/// 5.1 speaker set waiting behind it, and asking the decoder for two channels
/// gets the §7.8 downmix the encoder authored rather than one we invent. A
/// source already at or below stereo is unaffected — the decoder reports what it
/// actually emitted and [`AudioPlan::channels`] records that.
const TARGET_CHANNELS: u16 = 2;

impl AudioPlan {
    /// Index `path` and probe its decoded shape.
    ///
    /// Blocking: it reads the whole file's headers and decodes one frame, so
    /// callers on an async task must wrap it in `spawn_blocking`.
    pub fn build(path: &Path, codec: TranscodeCodec) -> Result<Self> {
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

        let (decoder, _) = PcmDecoder::open(
            codec,
            index.sample_rate,
            Some(TARGET_CHANNELS),
            &buf,
        )?;

        Ok(Self {
            source_path: path.to_path_buf(),
            codec,
            channels: decoder.channels(),
            index,
        })
    }

    /// Total size of the WAV resource, header included.
    pub fn wav_size(&self) -> u64 {
        wav_size(self.index.total_samples, self.channels)
    }

    /// Sample rate of the decoded output, in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.index.sample_rate
    }

    /// The WAV header describing this resource.
    pub fn wav_header(&self) -> [u8; 44] {
        super::wav_header(self.sample_rate(), self.channels, self.index.total_samples)
    }

    /// Bytes each decoded sample frame occupies.
    fn stride(&self) -> u64 {
        self.channels as u64 * 2
    }

    /// Turn a byte offset into the resource into the frame to start decoding at,
    /// and how many decoded bytes to drop from that frame's output.
    ///
    /// Offsets inside the 44-byte header resolve to the very start, because a
    /// range that begins mid-header still has to be served the rest of it.
    pub fn seek(&self, byte_offset: u64) -> Seeked {
        if byte_offset < WAV_HEADER_LEN {
            return Seeked {
                header_skip: byte_offset as usize,
                frame: 0,
                pcm_skip: 0,
            };
        }
        let pcm_offset = byte_offset - WAV_HEADER_LEN;
        let sample = pcm_offset / self.stride();
        let within = (pcm_offset % self.stride()) as usize;
        let (frame, samples_into_frame) = self.index.locate(sample);
        Seeked {
            header_skip: WAV_HEADER_LEN as usize,
            frame,
            pcm_skip: samples_into_frame as usize * self.stride() as usize + within,
        }
    }

    /// Decoded bytes produced by frame `i`.
    pub fn frame_bytes(&self, i: usize) -> usize {
        pcm_size(u64::from(self.index.frames[i].samples), self.channels) as usize
    }
}

/// Where a byte offset lands in the resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seeked {
    /// Bytes of the WAV header already passed — 44 once past it entirely.
    pub header_skip: usize,
    /// Index of the frame to begin decoding at.
    pub frame: usize,
    /// Decoded bytes to discard from that frame's output.
    pub pcm_skip: usize,
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
        (AudioPlan::build(&path, TranscodeCodec::Ac3).unwrap(), dir)
    }

    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn a_plan_describes_the_whole_resource_before_anything_is_decoded() {
        let (plan, _dir) = fixture_plan();
        assert_eq!(plan.channels, 2);
        assert_eq!(plan.sample_rate(), 48_000);
        assert_eq!(
            plan.wav_size(),
            WAV_HEADER_LEN + plan.index.total_samples * 2 * 2
        );
    }

    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn offset_zero_starts_at_the_header_and_the_first_frame() {
        let (plan, _dir) = fixture_plan();
        assert_eq!(
            plan.seek(0),
            Seeked {
                header_skip: 0,
                frame: 0,
                pcm_skip: 0
            }
        );
    }

    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn a_range_inside_the_header_still_gets_the_rest_of_it() {
        let (plan, _dir) = fixture_plan();
        let s = plan.seek(20);
        assert_eq!(s.header_skip, 20);
        assert_eq!(s.frame, 0);
        assert_eq!(s.pcm_skip, 0);
    }

    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn a_range_past_the_header_lands_on_the_right_frame_and_sample() {
        let (plan, _dir) = fixture_plan();
        // One whole AC-3 frame of decoded stereo is 1536 * 2 * 2 bytes.
        let one_frame = 1536 * 2 * 2;
        let s = plan.seek(WAV_HEADER_LEN + one_frame);
        assert_eq!(s.header_skip, WAV_HEADER_LEN as usize, "header is done");
        assert_eq!(s.frame, 1, "exactly the second frame");
        assert_eq!(s.pcm_skip, 0);

        // Half a frame in: same frame, half its output discarded.
        let s = plan.seek(WAV_HEADER_LEN + one_frame + one_frame / 2);
        assert_eq!(s.frame, 1);
        assert_eq!(s.pcm_skip, (one_frame / 2) as usize);
    }

    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn every_frame_reports_the_byte_count_its_sample_count_implies() {
        let (plan, _dir) = fixture_plan();
        let total: usize = (0..plan.index.frames.len()).map(|i| plan.frame_bytes(i)).sum();
        assert_eq!(total as u64 + WAV_HEADER_LEN, plan.wav_size());
    }
}

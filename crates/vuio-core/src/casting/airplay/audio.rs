//! Decode a media file into the PCM stream AirPlay carries.
//!
//! AirPlay never transports MP3. Its `audioFormat` bitmask covers PCM, ALAC,
//! AAC-LC, AAC-ELD and OPUS only, so a sender always decodes the source first;
//! an iPhone streaming an MP3 decodes it locally too. VuIO targets
//! `PCM/44100/16/2` (bit 11, `0x800`) because it is the one format that needs a
//! decoder and no encoder, and a Sony XR-75X90L accepts it.

use anyhow::{Context, Result};
use std::path::Path;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

/// The sample rate, channel count and sample size AirPlay's `0x800` format names.
pub const SAMPLE_RATE: u32 = 44100;
pub const CHANNELS: usize = 2;
pub const BYTES_PER_FRAME: usize = CHANNELS * 2;

/// What the receiver shows on its now-playing screen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// Track length in seconds, which is what gives the seek bar its extent.
    pub duration_seconds: Option<u64>,
    /// Embedded cover art, with its media type.
    pub artwork: Option<(String, Vec<u8>)>,
}

/// A decoded, resampled PCM stream ready to be packetised.
pub struct PcmSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::audio::AudioDecoder>,
    track_id: u32,
    source_rate: u32,
    source_channels: usize,
    /// Decoded frames at the source rate, interleaved, awaiting resampling.
    decoded: Vec<f32>,
    /// Fractional read position into `decoded`, in source frames.
    position: f64,
    /// Output frames at 44100 Hz, interleaved 16-bit.
    ready: Vec<u8>,
    exhausted: bool,
    metadata: TrackMetadata,
}

impl PcmSource {
    pub fn open(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("opening {} for AirPlay audio", path.display()))?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
            hint.with_extension(extension);
        }
        let mut format = symphonia::default::get_probe()
            .probe(
                &hint,
                stream,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .with_context(|| format!("{} is not a decodable audio container", path.display()))?;
        let track = format
            .default_track(TrackType::Audio)
            .context("the file has no audio track")?;
        let track_id = track.id;
        let duration_seconds = track
            .num_frames
            .zip(
                track
                    .codec_params
                    .as_ref()
                    .and_then(|params| params.audio()),
            )
            .and_then(|(frames, audio)| {
                audio
                    .sample_rate
                    .filter(|rate| *rate > 0)
                    .map(|rate| frames / u64::from(rate))
            });
        let parameters = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .context("the audio track has no codec parameters")?
            .clone();
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&parameters, &AudioDecoderOptions::default())
            .with_context(|| {
                format!(
                    "no decoder is available for the codec in {}",
                    path.display()
                )
            })?;
        let mut metadata = TrackMetadata {
            duration_seconds,
            ..TrackMetadata::default()
        };
        {
            let mut tags = format.metadata();
            if let Some(revision) = tags.skip_to_latest() {
                if let Some(visual) = revision.media.visuals.first() {
                    metadata.artwork = Some((
                        visual
                            .media_type
                            .clone()
                            .unwrap_or_else(|| "image/jpeg".into()),
                        visual.data.to_vec(),
                    ));
                }
                for tag in &revision.media.tags {
                    match &tag.std {
                        Some(symphonia::core::meta::StandardTag::TrackTitle(value)) => {
                            metadata.title = Some(value.to_string())
                        }
                        Some(symphonia::core::meta::StandardTag::Artist(value)) => {
                            metadata.artist = Some(value.to_string())
                        }
                        Some(symphonia::core::meta::StandardTag::Album(value)) => {
                            metadata.album = Some(value.to_string())
                        }
                        _ => {}
                    }
                }
            }
        }
        // Fall back to the filename so the receiver always shows something.
        if metadata.title.is_none() {
            metadata.title = path
                .file_stem()
                .and_then(|value| value.to_str())
                .map(str::to_string);
        }

        let source_rate = parameters.sample_rate.unwrap_or(SAMPLE_RATE);
        let source_channels = parameters
            .channels
            .as_ref()
            .map_or(CHANNELS, |channels| channels.count());
        Ok(Self {
            format,
            decoder,
            track_id,
            source_rate,
            source_channels: source_channels.max(1),
            decoded: Vec::new(),
            position: 0.0,
            ready: Vec::new(),
            exhausted: false,
            metadata,
        })
    }

    pub fn metadata(&self) -> &TrackMetadata {
        &self.metadata
    }

    /// Read the next packet from the container and decode it into `decoded`.
    ///
    /// Returns `false` once the stream is finished. Undecodable packets are
    /// skipped rather than aborting playback, which matches how players treat a
    /// damaged frame in the middle of a track.
    fn decode_more(&mut self) -> Result<bool> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => return Ok(false),
                Err(symphonia::core::errors::Error::IoError(error))
                    if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(false)
                }
                Err(error) => return Err(error).context("reading the next audio packet"),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(buffer) => {
                    // Copy out before touching `self` again: the buffer borrows
                    // the decoder.
                    let rate = buffer.spec().rate();
                    let channels = buffer.spec().channels().count().max(1);
                    let mut samples: Vec<f32> = Vec::new();
                    buffer.copy_to_vec_interleaved(&mut samples);
                    self.source_rate = rate;
                    self.source_channels = channels;
                    self.decoded.extend_from_slice(&samples);
                    return Ok(true);
                }
                Err(symphonia::core::errors::Error::DecodeError(error)) => {
                    tracing::debug!(%error, "skipping an undecodable audio packet");
                    continue;
                }
                Err(error) => return Err(error).context("decoding audio"),
            }
        }
    }

    /// Resample what is buffered into 44100 Hz stereo 16-bit frames.
    ///
    /// Linear interpolation is enough here: the common case is a 44100 Hz source
    /// where `step` is exactly 1.0 and samples pass through untouched.
    fn resample(&mut self) {
        let channels = self.source_channels;
        let available = self.decoded.len() / channels;
        if available == 0 {
            return;
        }
        let step = f64::from(self.source_rate) / f64::from(SAMPLE_RATE);
        // Interpolation needs the frame after `position`, so stop one short
        // unless the stream is finished and there is nothing more coming.
        let limit = if self.exhausted {
            available.saturating_sub(1)
        } else {
            available.saturating_sub(2)
        };
        while (self.position as usize) < limit {
            let index = self.position as usize;
            let fraction = (self.position - index as f64) as f32;
            for channel in 0..CHANNELS {
                // Mono sources feed both output channels.
                let source_channel = channel.min(channels - 1);
                let first = self.decoded[index * channels + source_channel];
                let second = self.decoded[(index + 1) * channels + source_channel];
                let value = first + (second - first) * fraction;
                let scaled = (value.clamp(-1.0, 1.0) * 32767.0) as i16;
                self.ready.extend_from_slice(&scaled.to_le_bytes());
            }
            self.position += step;
        }
        // Drop consumed frames and rebase the cursor.
        let consumed = self.position as usize;
        if consumed > 0 {
            self.decoded.drain(..consumed * channels);
            self.position -= consumed as f64;
        }
    }

    /// Return exactly `frames` frames of PCM, zero-padded at end of stream.
    ///
    /// Returns `None` once the source is fully drained, which tells the sender
    /// to stop reading and start its latency run-out.
    pub fn read_frames(&mut self, frames: usize) -> Result<Option<Vec<u8>>> {
        let wanted = frames * BYTES_PER_FRAME;
        while self.ready.len() < wanted && !self.exhausted {
            if !self.decode_more()? {
                self.exhausted = true;
            }
            self.resample();
        }
        if self.ready.is_empty() {
            return Ok(None);
        }
        let mut chunk: Vec<u8> = self.ready.drain(..wanted.min(self.ready.len())).collect();
        chunk.resize(wanted, 0);
        Ok(Some(chunk))
    }
}

/// Whether VuIO can decode this file for AirPlay audio.
pub fn is_streamable_audio(mime: &str, filename: &str) -> bool {
    let mime = mime
        .split(';')
        .next()
        .unwrap_or(mime)
        .trim()
        .to_ascii_lowercase();
    if mime.starts_with("audio/") && mime != "audio/radio" {
        return true;
    }
    filename.rsplit_once('.').is_some_and(|(_, extension)| {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            "mp3" | "m4a" | "aac" | "flac" | "wav" | "ogg" | "oga" | "alac"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamable_audio_covers_the_library_formats() {
        assert!(is_streamable_audio("audio/mpeg", "song.mp3"));
        assert!(is_streamable_audio("audio/mp4", "song.m4a"));
        assert!(is_streamable_audio("audio/flac", "song.flac"));
        assert!(is_streamable_audio("application/octet-stream", "song.wav"));
        assert!(!is_streamable_audio("audio/radio", "stream"));
        assert!(!is_streamable_audio("video/mp4", "movie.mp4"));
    }

    /// A synthesised WAV exercises decode, resample and framing without needing
    /// a fixture file.
    #[test]
    fn wav_decodes_into_44100_stereo_frames() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("tone.wav");
        write_test_wav(&path, 22050, 4410);

        let mut source = PcmSource::open(&path).unwrap();
        let mut total = 0usize;
        while let Some(chunk) = source.read_frames(352).unwrap() {
            assert_eq!(chunk.len(), 352 * BYTES_PER_FRAME);
            total += chunk.len();
        }
        // 4410 frames at 22050 Hz is 0.2s, so 8820 frames once resampled to
        // 44100 Hz. The final packet is zero-padded to 352 frames, so the total
        // lands on the next packet boundary: 26 * 352 = 9152.
        let produced = total / BYTES_PER_FRAME;
        assert_eq!(produced % 352, 0, "packets must be whole: {produced}");
        assert!(
            (8820..=8820 + 352).contains(&produced),
            "expected 8820 frames rounded up to a packet boundary, got {produced}"
        );
    }

    fn write_test_wav(path: &std::path::Path, rate: u32, frames: u32) {
        let data_len = frames * 4; // stereo, 16-bit
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&2u16.to_le_bytes()); // stereo
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * 4).to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for frame in 0..frames {
            let value = ((frame as f32 / 20.0).sin() * 8000.0) as i16;
            wav.extend_from_slice(&value.to_le_bytes());
            wav.extend_from_slice(&value.to_le_bytes());
        }
        std::fs::write(path, wav).unwrap();
    }
}

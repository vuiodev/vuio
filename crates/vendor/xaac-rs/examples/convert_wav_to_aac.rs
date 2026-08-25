use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use xaac_rs::{Encoder, EncoderConfig, OutputFormat, Profile};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let input = args.next().ok_or_else(usage)?;
    let output = args.next().ok_or_else(usage)?;
    let bitrate = match args.next() {
        Some(value) => value
            .parse::<u32>()
            .map_err(|_| "bitrate must be an integer number of bits per second")?,
        None => 128_000,
    };

    let wav_bytes = fs::read(&input)?;
    let wav = WavFile::parse(&wav_bytes)?;

    let mut encoder = Encoder::new(EncoderConfig {
        profile: Profile::AacLc,
        sample_rate: wav.sample_rate,
        native_sample_rate: Some(wav.sample_rate),
        channels: wav.channels,
        channel_mask: wav.channel_mask.unwrap_or(0),
        bitrate,
        pcm_word_size: wav.bits_per_sample,
        output_format: OutputFormat::Adts,
        ..EncoderConfig::default()
    })?;

    let mut out = fs::File::create(Path::new(&output))?;
    let frame_bytes = encoder.input_frame_bytes();
    let mut offset = 0usize;
    while offset + frame_bytes <= wav.pcm_data.len() {
        let packet = encoder.encode_pcm_bytes(&wav.pcm_data[offset..offset + frame_bytes])?;
        out.write_all(&packet.data)?;
        offset += frame_bytes;
    }

    if offset < wav.pcm_data.len() {
        let frame = encoder.encode_pcm_bytes_with_padding(&wav.pcm_data[offset..])?;
        out.write_all(&frame.packet.data)?;
    }

    eprintln!(
        "encoded {} Hz, {} channels, {}-bit PCM to ADTS AAC at {} bps",
        wav.sample_rate, wav.channels, wav.bits_per_sample, bitrate
    );

    Ok(())
}

fn usage() -> Box<dyn std::error::Error> {
    invalid_input(
        "usage: cargo run --example convert_wav_to_aac -- <input.wav> <output.aac> [bitrate]",
    )
}

#[derive(Debug)]
struct WavFile<'a> {
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    channel_mask: Option<u32>,
    pcm_data: &'a [u8],
}

impl<'a> WavFile<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, Box<dyn std::error::Error>> {
        if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return Err(invalid_input("input is not a RIFF/WAVE file"));
        }

        let mut offset = 12usize;
        let mut channels = None;
        let mut sample_rate = None;
        let mut bits_per_sample = None;
        let mut channel_mask = None;
        let mut pcm_data = None;

        while offset + 8 <= bytes.len() {
            let chunk_id = &bytes[offset..offset + 4];
            let chunk_size = le_u32(&bytes[offset + 4..offset + 8]) as usize;
            offset += 8;

            if offset + chunk_size > bytes.len() {
                return Err(invalid_input("WAV chunk extends past end of file"));
            }

            let chunk = &bytes[offset..offset + chunk_size];
            match chunk_id {
                b"fmt " => {
                    if chunk.len() < 16 {
                        return Err(invalid_input("WAV fmt chunk is too short"));
                    }
                    let audio_format = le_u16(&chunk[0..2]);
                    channels = Some(le_u16(&chunk[2..4]));
                    sample_rate = Some(le_u32(&chunk[4..8]));
                    bits_per_sample = Some(le_u16(&chunk[14..16]));

                    match audio_format {
                        1 => {}
                        0xfffe => {
                            if chunk.len() < 40 {
                                return Err(invalid_input("WAV extensible fmt chunk is too short"));
                            }
                            channel_mask = Some(le_u32(&chunk[20..24]));
                            let valid_bits = le_u16(&chunk[18..20]);
                            if valid_bits != 0 {
                                bits_per_sample = Some(valid_bits);
                            }
                            let subformat = &chunk[24..40];
                            let pcm_guid: [u8; 16] = [
                                0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00,
                                0xaa, 0x00, 0x38, 0x9b, 0x71,
                            ];
                            if subformat != pcm_guid {
                                return Err(invalid_input(
                                    "only PCM WAV extensible files are supported",
                                ));
                            }
                        }
                        _ => {
                            return Err(invalid_input(
                                "only uncompressed PCM WAV files are supported",
                            ));
                        }
                    }
                }
                b"data" => {
                    pcm_data = Some(chunk);
                }
                _ => {}
            }

            offset += chunk_size;
            if chunk_size % 2 == 1 {
                offset += 1;
            }
        }

        let channels =
            channels.ok_or_else(|| invalid_input("WAV fmt chunk missing channel count"))?;
        let sample_rate =
            sample_rate.ok_or_else(|| invalid_input("WAV fmt chunk missing sample rate"))?;
        let bits_per_sample = bits_per_sample
            .ok_or_else(|| invalid_input("WAV fmt chunk missing bits per sample"))?;
        let pcm_data = pcm_data.ok_or_else(|| invalid_input("WAV data chunk not found"))?;

        if !matches!(bits_per_sample, 16 | 24 | 32) {
            return Err(invalid_input(
                "only 16-bit, 24-bit, and 32-bit PCM WAV files are supported",
            ));
        }

        let bytes_per_sample = usize::from(bits_per_sample / 8);
        let frame_size = usize::from(channels) * bytes_per_sample;
        if frame_size == 0 || pcm_data.len() % frame_size != 0 {
            return Err(invalid_input(
                "WAV data size is not aligned to full PCM frames",
            ));
        }

        Ok(Self {
            sample_rate,
            channels,
            bits_per_sample,
            channel_mask,
            pcm_data,
        })
    }
}

fn invalid_input(message: &'static str) -> Box<dyn std::error::Error> {
    Box::new(io::Error::new(io::ErrorKind::InvalidInput, message))
}

fn le_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn le_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

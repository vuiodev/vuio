use std::env;
use std::fs;
use std::io;
use std::path::Path;

use xaac_rs::{Decoder, DecoderConfig, SbrMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args().nth(1).ok_or_else(usage)?;
    let input_path = Path::new(&input);
    let data = fs::read(input_path)?;

    let mut decoder = Decoder::new(DecoderConfig::default())?;
    let version = decoder.version().clone();

    println!("file: {}", input_path.display());
    println!("size: {} bytes", data.len());
    println!("decoder: {} {}", version.name, version.version);
    println!("decoder input capacity: {} bytes", decoder.input_capacity());

    if data.is_empty() {
        println!("stream info: file is empty");
        return Ok(());
    }

    let probe_len = data.len();
    let probe = &data;

    match decoder.probe_stream_info(probe) {
        Ok(info) => {
            println!("probe bytes: {}", probe_len);
            println!("sample rate: {} Hz", info.sample_rate);
            println!("channels: {}", info.channels);
            println!("channel mask: 0x{:x}", info.channel_mask);
            println!("channel mode: {:?}", info.channel_mode);
            println!("pcm word size: {} bits", info.pcm_word_size);
            println!("audio object type: {}", info.audio_object_type);
            println!("sbr mode: {:?}", info.sbr_mode);
            println!("drc active: {}", info.drc_active);
            println!(
                "drc target loudness: {}",
                info.drc_target_loudness
                    .map_or_else(|| "unavailable".to_string(), |value| value.to_string())
            );
            println!(
                "drc loudness norm: {}",
                info.drc_loudness_norm
                    .map_or_else(|| "unavailable".to_string(), |value| value.to_string())
            );
            println!(
                "loudness leveling: {}",
                info.loudness_leveling
                    .map_or_else(|| "unavailable".to_string(), |value| value.to_string())
            );
            println!(
                "preroll frames: {}",
                info.preroll_frames
                    .map_or_else(|| "unavailable".to_string(), |value| value.to_string())
            );
            println!(
                "gain payload bytes: {}",
                info.gain_payload_len
                    .map_or_else(|| "unavailable".to_string(), |value| value.to_string())
            );
            match estimate_adts_bitrate(&data, info.sample_rate, info.sbr_mode) {
                Some(bitrate) => println!("bit rate: {} bps", bitrate),
                None => println!("bit rate: unavailable"),
            }
        }
        Err(err) => {
            println!("stream info: unavailable");
            println!("decoder error: {err}");
            println!("probe bytes: {}", probe_len);
        }
    }

    Ok(())
}

fn usage() -> Box<dyn std::error::Error> {
    Box::new(io::Error::new(
        io::ErrorKind::InvalidInput,
        "usage: cargo run --example file_info -- <input.aac>",
    ))
}

fn estimate_adts_bitrate(data: &[u8], sample_rate: u32, sbr_mode: SbrMode) -> Option<u32> {
    if sample_rate == 0 {
        return None;
    }

    let mut offset = 0usize;
    let mut frames = 0u64;
    let mut payload_bytes = 0u64;

    while offset + 7 <= data.len() {
        if data[offset] != 0xff || (data[offset + 1] & 0xf0) != 0xf0 {
            return None;
        }

        let frame_length = (((data[offset + 3] & 0x03) as usize) << 11)
            | ((data[offset + 4] as usize) << 3)
            | (((data[offset + 5] & 0xe0) as usize) >> 5);

        if frame_length < 7 || offset + frame_length > data.len() {
            return None;
        }

        frames += 1;
        payload_bytes += frame_length as u64;
        offset += frame_length;
    }

    if frames == 0 || offset != data.len() {
        return None;
    }

    let samples_per_frame = match sbr_mode {
        SbrMode::Enabled => 2048u64,
        SbrMode::Esbr => 4096u64,
        _ => 1024u64,
    };
    let total_samples = frames.checked_mul(samples_per_frame)?;
    let bits = payload_bytes.checked_mul(8)?;
    let bitrate = bits
        .checked_mul(sample_rate as u64)?
        .checked_div(total_samples)?;
    u32::try_from(bitrate).ok()
}

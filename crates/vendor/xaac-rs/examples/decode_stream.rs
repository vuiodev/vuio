use std::env;
use std::fs;
use std::io;
use std::path::Path;

use xaac_rs::{DecodeStatus, Decoder, DecoderConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args().nth(1).ok_or_else(usage)?;
    let path = Path::new(&input);
    let data = fs::read(path)?;

    let mut decoder = Decoder::new(DecoderConfig::default())?;
    let mut frame_count = 0usize;
    let mut pcm_bytes = 0usize;
    let mut offset = 0usize;

    while offset < data.len() {
        let end = (offset + 256).min(data.len());
        match decoder.decode_stream_chunk(&data[offset..end])? {
            DecodeStatus::Frame(frame) => {
                frame_count += 1;
                pcm_bytes += frame.pcm.len();
                println!(
                    "frame {}: {} pcm bytes, {} Hz, {} ch",
                    frame_count,
                    frame.pcm.len(),
                    frame.stream_info.sample_rate,
                    frame.stream_info.channels
                );
            }
            DecodeStatus::NeedMoreInput(progress) => {
                if let Some(info) = progress.stream_info {
                    println!(
                        "need more input: initialized={}, {} Hz, {} ch",
                        progress.initialized, info.sample_rate, info.channels
                    );
                } else {
                    println!("need more input: initialized={}", progress.initialized);
                }
            }
            DecodeStatus::EndOfStream => break,
        }
        offset = end;
    }

    loop {
        match decoder.finish()? {
            DecodeStatus::Frame(frame) => {
                frame_count += 1;
                pcm_bytes += frame.pcm.len();
                println!(
                    "flush frame {}: {} pcm bytes, {} Hz, {} ch",
                    frame_count,
                    frame.pcm.len(),
                    frame.stream_info.sample_rate,
                    frame.stream_info.channels
                );
            }
            DecodeStatus::NeedMoreInput(_) => continue,
            DecodeStatus::EndOfStream => break,
        }
    }

    println!("decoded frames: {}", frame_count);
    println!("decoded pcm bytes: {}", pcm_bytes);
    Ok(())
}

fn usage() -> Box<dyn std::error::Error> {
    Box::new(io::Error::new(
        io::ErrorKind::InvalidInput,
        "usage: cargo run --example decode_stream -- <input.aac>",
    ))
}

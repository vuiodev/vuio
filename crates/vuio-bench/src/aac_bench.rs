//! Benchmark for audio decoders and encoders in VuIO:
//! - AC-3 Decoder (vuio-codec-ac3)
//! - E-AC-3 Decoder (vuio-codec-ac3)
//! - DTS Decoder (oxideav-dts)
//! - AAC Encoder (xaac-rs vs Apple Native vs oxideav-aac)

use std::time::{Duration, Instant};
use vuio_core::media::transcode::{PcmDecoder, TranscodeCodec};

const AC3_FIXTURE: &[u8] = include_bytes!("../../vuio-codec-ac3/tests/fixtures/sine440_stereo.ac3");
const DTS_FIXTURE: &[u8] =
    include_bytes!("../../vendor/oxideav-dts/tests/fixtures/dts_5_frames.bin");

fn generate_sine_wave(sample_rate: u32, channels: u16, duration_secs: f64) -> Vec<i16> {
    let total_samples = (sample_rate as f64 * duration_secs) as usize;
    let mut pcm = Vec::with_capacity(total_samples * channels as usize);
    let freq = 440.0;
    for i in 0..total_samples {
        let t = i as f64 / sample_rate as f64;
        let val = (t * freq * 2.0 * std::f64::consts::PI).sin();
        let sample = (val * 30000.0) as i16;
        for _ in 0..channels {
            pcm.push(sample);
        }
    }
    pcm
}

fn pcm_i16_to_u8_le(pcm: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(pcm.len() * 2);
    for &s in pcm {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    bytes
}

// =========================================================================
// DECODE BENCHMARKS: AC-3, E-AC-3, DTS
// =========================================================================

/// Benchmark AC-3 Decoding
fn bench_ac3_decode(iterations: usize) -> (Duration, f64, usize) {
    let frame_len = 768; // 48kHz 192kbps stereo AC-3 frame
    let num_frames_in_fixture = AC3_FIXTURE.len() / frame_len;
    let first_frame = &AC3_FIXTURE[..frame_len];

    let start = Instant::now();
    let (mut decoder, first_pcm) =
        PcmDecoder::open(TranscodeCodec::Ac3, 48000, Some(2), first_frame)
            .expect("open AC-3 decoder");

    let mut total_pcm_bytes = first_pcm.len();
    let mut total_samples = 1536; // first frame samples

    for _ in 0..iterations {
        for f in 0..num_frames_in_fixture {
            let frame = &AC3_FIXTURE[f * frame_len..(f + 1) * frame_len];
            let pcm = decoder.decode_or_silence(frame, Some(1536));
            total_pcm_bytes += pcm.len();
            total_samples += 1536;
        }
    }

    let elapsed = start.elapsed();
    let audio_duration_secs = total_samples as f64 / 48000.0;
    (elapsed, audio_duration_secs, total_pcm_bytes)
}

/// Benchmark E-AC-3 (Dolby Digital Plus) Decoding
fn bench_eac3_decode(iterations: usize) -> (Duration, f64, usize) {
    // Generate a compliant E-AC-3 bitstream via vuio-codec-ac3's E-AC-3 encoder
    use oxideav_core::{AudioFrame, CodecId, CodecParameters, Frame, SampleFormat};

    let sample_rate = 48000;
    let channels = 2;
    let mut params = CodecParameters::audio(CodecId::new("eac3"));
    params.sample_rate = Some(sample_rate);
    params.channels = Some(channels);
    params.sample_format = Some(SampleFormat::S16);

    let mut enc = vuio_codec_ac3::eac3::make_encoder(&params).expect("init Eac3Encoder");
    let test_pcm = generate_sine_wave(sample_rate, channels, 1.0);
    let test_bytes = pcm_i16_to_u8_le(&test_pcm);

    let frame = Frame::Audio(AudioFrame {
        samples: 1536,
        pts: None,
        data: vec![test_bytes[..1536 * 4].to_vec()],
    });
    enc.send_frame(&frame).expect("send frame to Eac3Encoder");
    enc.flush().expect("flush Eac3Encoder");

    let mut eac3_packets = Vec::new();
    while let Ok(pkt) = enc.receive_packet() {
        eac3_packets.push(pkt.data);
    }
    assert!(
        !eac3_packets.is_empty(),
        "E-AC-3 encoder produced no packets"
    );

    let first_frame = &eac3_packets[0];
    let start = Instant::now();
    let (mut decoder, first_pcm) = PcmDecoder::open(
        TranscodeCodec::Eac3,
        sample_rate,
        Some(channels),
        first_frame,
    )
    .expect("open E-AC-3 decoder");

    let mut total_pcm_bytes = first_pcm.len();
    let mut total_samples = 1536;

    for _ in 0..iterations {
        for pkt in &eac3_packets {
            let pcm = decoder.decode_or_silence(pkt, Some(1536));
            total_pcm_bytes += pcm.len();
            total_samples += 1536;
        }
    }

    let elapsed = start.elapsed();
    let audio_duration_secs = total_samples as f64 / sample_rate as f64;
    (elapsed, audio_duration_secs, total_pcm_bytes)
}

/// Benchmark DTS Decoding
fn bench_dts_decode(iterations: usize) -> (Duration, f64, usize) {
    // dts_5_frames.bin carries 5 real DTS 5.1/stereo frames
    // Read frame lengths from DTS frame headers (syncword 0x7FFE8001)
    let mut frames = Vec::new();
    let mut offset = 0;
    while offset + 10 <= DTS_FIXTURE.len() {
        if DTS_FIXTURE[offset..offset + 4] == [0x7F, 0xFE, 0x80, 0x01] {
            let fsize = (((DTS_FIXTURE[offset + 5] as usize & 0x03) << 12)
                | ((DTS_FIXTURE[offset + 6] as usize) << 4)
                | ((DTS_FIXTURE[offset + 7] as usize & 0xF0) >> 4))
                + 1;
            if offset + fsize <= DTS_FIXTURE.len() {
                frames.push(&DTS_FIXTURE[offset..offset + fsize]);
                offset += fsize;
                continue;
            }
        }
        offset += 1;
    }
    assert!(!frames.is_empty(), "No DTS frames found in fixture");

    let first_frame = frames[0];
    let start = Instant::now();
    let (mut decoder, first_pcm) =
        PcmDecoder::open(TranscodeCodec::Dts, 48000, Some(2), first_frame)
            .expect("open DTS decoder");

    let mut total_pcm_bytes = first_pcm.len();
    let mut total_samples = 512;

    for _ in 0..iterations {
        for frame in &frames {
            let pcm = decoder.decode_or_silence(frame, Some(512));
            total_pcm_bytes += pcm.len();
            total_samples += 512;
        }
    }

    let elapsed = start.elapsed();
    let audio_duration_secs = total_samples as f64 / 48000.0;
    (elapsed, audio_duration_secs, total_pcm_bytes)
}

// =========================================================================
// ENCODE BENCHMARKS
// =========================================================================

fn bench_vuio_aac(pcm_bytes: &[u8], sample_rate: u32, channels: u16) -> (Duration, usize) {
    let start = Instant::now();
    let mut encoder = vuio_core::media::transcode::AacEncoder::new(sample_rate, channels)
        .expect("failed to init VuIO AAC encoder");

    let chunk_size = 1024 * channels as usize * 2;
    let mut out_len = 0;
    for chunk in pcm_bytes.chunks(chunk_size) {
        let adts = encoder.push(chunk).expect("VuIO AAC encode error");
        out_len += adts.len();
    }
    let tail = encoder.finish();
    out_len += tail.len();
    let elapsed = start.elapsed();
    (elapsed, out_len)
}

#[cfg(target_os = "macos")]
#[allow(non_snake_case, non_upper_case_globals)]
mod apple_native {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    struct AudioStreamBasicDescription {
        mSampleRate: f64,
        mFormatID: u32,
        mFormatFlags: u32,
        mBytesPerPacket: u32,
        mFramesPerPacket: u32,
        mBytesPerFrame: u32,
        mChannelsPerFrame: u32,
        mBitsPerChannel: u32,
        mReserved: u32,
    }

    const kAudioFormatLinearPCM: u32 = 0x6c70636d;
    const kAudioFormatMPEG4AAC: u32 = 0x61616320;
    const kAudioFormatFlagIsSignedInteger: u32 = 1 << 2;
    const kAudioFormatFlagIsPacked: u32 = 1 << 3;

    #[repr(C)]
    struct AudioBuffer {
        mNumberChannels: u32,
        mDataByteSize: u32,
        mData: *mut c_void,
    }

    #[repr(C)]
    struct AudioBufferList {
        mNumberBuffers: u32,
        mBuffers: [AudioBuffer; 1],
    }

    #[repr(C)]
    #[derive(Debug, Default, Clone, Copy)]
    struct AudioStreamPacketDescription {
        mStartOffset: i64,
        mVariableFramesInPacket: u32,
        mDataByteSize: u32,
    }

    type AudioConverterRef = *mut c_void;
    type OSStatus = i32;

    type AudioConverterComplexInputDataProc = unsafe extern "C" fn(
        inAudioConverter: AudioConverterRef,
        ioNumberDataPackets: *mut u32,
        ioData: *mut AudioBufferList,
        outDataPacketDescription: *mut *mut AudioStreamPacketDescription,
        inUserData: *mut c_void,
    ) -> OSStatus;

    #[link(name = "AudioToolbox", kind = "framework")]
    extern "C" {
        fn AudioConverterNew(
            inSourceFormat: *const AudioStreamBasicDescription,
            inDestinationFormat: *const AudioStreamBasicDescription,
            outAudioConverter: *mut AudioConverterRef,
        ) -> OSStatus;

        fn AudioConverterDispose(inAudioConverter: AudioConverterRef) -> OSStatus;

        fn AudioConverterFillComplexBuffer(
            inAudioConverter: AudioConverterRef,
            inInputDataProc: AudioConverterComplexInputDataProc,
            inInputDataProcUserData: *mut c_void,
            ioOutputDataPacketSize: *mut u32,
            outOutputData: *mut AudioBufferList,
            outPacketDescription: *mut AudioStreamPacketDescription,
        ) -> OSStatus;
    }

    struct InputContext<'a> {
        pcm_bytes: &'a [u8],
        pos: usize,
        bytes_per_packet: usize,
    }

    unsafe extern "C" fn input_data_proc(
        _in_converter: AudioConverterRef,
        io_num_packets: *mut u32,
        io_data: *mut AudioBufferList,
        _out_packet_desc: *mut *mut AudioStreamPacketDescription,
        in_user_data: *mut c_void,
    ) -> OSStatus {
        let ctx = &mut *(in_user_data as *mut InputContext);
        let requested_packets = *io_num_packets as usize;
        let available_bytes = ctx.pcm_bytes.len().saturating_sub(ctx.pos);
        let available_packets = available_bytes / ctx.bytes_per_packet;
        let packets_to_give = requested_packets.min(available_packets);

        if packets_to_give == 0 {
            *io_num_packets = 0;
            return 0;
        }

        let bytes_to_give = packets_to_give * ctx.bytes_per_packet;
        let ptr = ctx.pcm_bytes[ctx.pos..].as_ptr() as *mut c_void;
        ctx.pos += bytes_to_give;

        *io_num_packets = packets_to_give as u32;
        (*io_data).mNumberBuffers = 1;
        (*io_data).mBuffers[0].mNumberChannels = 2;
        (*io_data).mBuffers[0].mDataByteSize = bytes_to_give as u32;
        (*io_data).mBuffers[0].mData = ptr;

        0
    }

    pub fn bench_apple(
        pcm_bytes: &[u8],
        sample_rate: u32,
        channels: u16,
    ) -> (std::time::Duration, usize) {
        let start = std::time::Instant::now();

        let in_format = AudioStreamBasicDescription {
            mSampleRate: sample_rate as f64,
            mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagIsSignedInteger | kAudioFormatFlagIsPacked,
            mBytesPerPacket: (channels * 2) as u32,
            mFramesPerPacket: 1,
            mBytesPerFrame: (channels * 2) as u32,
            mChannelsPerFrame: channels as u32,
            mBitsPerChannel: 16,
            mReserved: 0,
        };

        let out_format = AudioStreamBasicDescription {
            mSampleRate: sample_rate as f64,
            mFormatID: kAudioFormatMPEG4AAC,
            mFormatFlags: 0,
            mBytesPerPacket: 0,
            mFramesPerPacket: 1024,
            mBytesPerFrame: 0,
            mChannelsPerFrame: channels as u32,
            mBitsPerChannel: 0,
            mReserved: 0,
        };

        let mut converter: AudioConverterRef = std::ptr::null_mut();
        let status = unsafe { AudioConverterNew(&in_format, &out_format, &mut converter) };
        assert_eq!(status, 0, "AudioConverterNew failed: {status}");

        let mut ctx = InputContext {
            pcm_bytes,
            pos: 0,
            bytes_per_packet: (channels * 2) as usize,
        };

        let mut out_len = 0;
        let mut out_buf = vec![0u8; 8192];
        let mut packet_descs = vec![AudioStreamPacketDescription::default(); 16];

        loop {
            let mut num_packets: u32 = 16;
            let mut buffer_list = AudioBufferList {
                mNumberBuffers: 1,
                mBuffers: [AudioBuffer {
                    mNumberChannels: channels as u32,
                    mDataByteSize: out_buf.len() as u32,
                    mData: out_buf.as_mut_ptr() as *mut c_void,
                }],
            };

            let res = unsafe {
                AudioConverterFillComplexBuffer(
                    converter,
                    input_data_proc,
                    &mut ctx as *mut _ as *mut c_void,
                    &mut num_packets,
                    &mut buffer_list,
                    packet_descs.as_mut_ptr(),
                )
            };

            if num_packets == 0 || res != 0 {
                break;
            }

            for desc in packet_descs.iter().take(num_packets as usize) {
                out_len += desc.mDataByteSize as usize;
            }
        }

        unsafe { AudioConverterDispose(converter) };
        let elapsed = start.elapsed();
        (elapsed, out_len)
    }
}

fn bench_xaac(
    pcm_bytes: &[u8],
    sample_rate: u32,
    channels: u16,
) -> Result<(Duration, usize), String> {
    let start = Instant::now();
    use xaac_rs::{Encoder, EncoderConfig, OutputFormat, Profile};

    let config = EncoderConfig {
        profile: Profile::AacLc,
        sample_rate,
        channels,
        bitrate: 128_000,
        output_format: OutputFormat::Adts,
        ..Default::default()
    };

    let mut encoder = Encoder::new(config).map_err(|e| format!("{e:?}"))?;
    let frame_bytes = encoder.input_frame_bytes();

    let mut out_len = 0;
    for chunk in pcm_bytes.chunks(frame_bytes) {
        if chunk.len() == frame_bytes {
            let encoded = encoder
                .encode_pcm_bytes(chunk)
                .map_err(|e| format!("{e:?}"))?;
            out_len += encoded.data.len();
        } else {
            let encoded = encoder
                .encode_pcm_bytes_with_padding(chunk)
                .map_err(|e| format!("{e:?}"))?;
            out_len += encoded.packet.data.len();
        }
    }

    let elapsed = start.elapsed();
    Ok((elapsed, out_len))
}

fn main() {
    println!("================================================================================");
    println!("AUDIO DECODER SPEED BENCHMARKS (RELEASE MODE)");
    println!("================================================================================");

    // 1. AC-3 Decode Benchmark
    {
        print!("Benchmarking AC-3 Decoder (vuio-codec-ac3)... ");
        let iterations = 1000; // ~512 seconds of audio
        let (dur, audio_secs, out_bytes) = bench_ac3_decode(iterations);
        let speed = audio_secs / dur.as_secs_f64();
        println!("DONE\n  Decoded Audio: {:.2} seconds ({:.2} MB PCM)\n  Time Taken:    {:.3} ms\n  Speedup:       {:.1}x real-time\n  Throughput:    {:.2} MB/s\n",
            audio_secs,
            out_bytes as f64 / (1024.0 * 1024.0),
            dur.as_secs_f64() * 1000.0,
            speed,
            (out_bytes as f64 / (1024.0 * 1024.0)) / dur.as_secs_f64()
        );
    }

    // 2. E-AC-3 Decode Benchmark
    {
        print!("Benchmarking E-AC-3 Decoder (vuio-codec-ac3)... ");
        let iterations = 5000; // ~160 seconds of audio
        let (dur, audio_secs, out_bytes) = bench_eac3_decode(iterations);
        let speed = audio_secs / dur.as_secs_f64();
        println!("DONE\n  Decoded Audio: {:.2} seconds ({:.2} MB PCM)\n  Time Taken:    {:.3} ms\n  Speedup:       {:.1}x real-time\n  Throughput:    {:.2} MB/s\n",
            audio_secs,
            out_bytes as f64 / (1024.0 * 1024.0),
            dur.as_secs_f64() * 1000.0,
            speed,
            (out_bytes as f64 / (1024.0 * 1024.0)) / dur.as_secs_f64()
        );
    }

    // 3. DTS Decode Benchmark
    {
        print!("Benchmarking DTS Decoder (oxideav-dts)... ");
        let iterations = 3000; // ~160 seconds of audio
        let (dur, audio_secs, out_bytes) = bench_dts_decode(iterations);
        let speed = audio_secs / dur.as_secs_f64();
        println!("DONE\n  Decoded Audio: {:.2} seconds ({:.2} MB PCM)\n  Time Taken:    {:.3} ms\n  Speedup:       {:.1}x real-time\n  Throughput:    {:.2} MB/s\n",
            audio_secs,
            out_bytes as f64 / (1024.0 * 1024.0),
            dur.as_secs_f64() * 1000.0,
            speed,
            (out_bytes as f64 / (1024.0 * 1024.0)) / dur.as_secs_f64()
        );
    }

    println!("================================================================================");
    println!("AAC ENCODER BENCHMARKS (RELEASE MODE)");
    println!("================================================================================");

    let sample_rate = 48000;
    let channels = 2;
    let duration_secs = 60.0;
    let pcm = generate_sine_wave(sample_rate, channels, duration_secs);
    let pcm_bytes = pcm_i16_to_u8_le(&pcm);

    #[cfg(target_os = "macos")]
    {
        print!("Benchmarking Apple Native (AudioToolbox)... ");
        let (dur, out_bytes) = apple_native::bench_apple(&pcm_bytes, sample_rate, channels);
        let speed = duration_secs / dur.as_secs_f64();
        println!("DONE\n  Time:      {:.3} ms\n  Speedup:   {:.1}x real-time\n  Output:    {} bytes ({:.1} kbps)\n",
            dur.as_secs_f64() * 1000.0,
            speed,
            out_bytes,
            (out_bytes as f64 * 8.0) / (duration_secs * 1000.0)
        );
    }

    {
        print!("Benchmarking xaac-rs (libxaac)... ");
        match bench_xaac(&pcm_bytes, sample_rate, channels) {
            Ok((dur, out_bytes)) => {
                let speed = duration_secs / dur.as_secs_f64();
                println!("DONE\n  Time:      {:.3} ms\n  Speedup:   {:.1}x real-time\n  Output:    {} bytes ({:.1} kbps)\n",
                    dur.as_secs_f64() * 1000.0,
                    speed,
                    out_bytes,
                    (out_bytes as f64 * 8.0) / (duration_secs * 1000.0)
                );
            }
            Err(e) => {
                println!("FAILED: {}\n", e);
            }
        }
    }

    {
        print!("Benchmarking VuIO AacEncoder (libxaac)... ");
        let (dur, out_bytes) = bench_vuio_aac(&pcm_bytes, sample_rate, channels);
        let speed = duration_secs / dur.as_secs_f64();
        println!("DONE\n  Time:      {:.3} ms\n  Speedup:   {:.1}x real-time\n  Output:    {} bytes ({:.1} kbps)\n",
            dur.as_secs_f64() * 1000.0,
            speed,
            out_bytes,
            (out_bytes as f64 * 8.0) / (duration_secs * 1000.0)
        );
    }
}

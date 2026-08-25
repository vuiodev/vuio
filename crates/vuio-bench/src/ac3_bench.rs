//! AC-3 Encoder Benchmark: oxideav-ac3 vs Apple Native (AudioToolbox)
//!
//! Measures encoding throughput, real-time speedup factor, and latency
//! for multi-channel AC-3 encoding (5 channels, 640 kbps, 48 kHz).

use std::time::{Duration, Instant};
use oxideav_core::{
    AudioFrame, CodecId, CodecParameters, Frame, SampleFormat,
};

/// Generates multi-channel sine wave audio for benchmarking.
/// Each channel gets a different distinct frequency so the encoder processes
/// genuine independent channel data (avoiding trivial cross-channel redundancy).
fn generate_multichannel_audio(sample_rate: u32, channels: u16, duration_secs: f64) -> Vec<i16> {
    let total_frames = (sample_rate as f64 * duration_secs) as usize;
    let mut pcm = Vec::with_capacity(total_frames * channels as usize);

    // Distinct frequencies for each channel
    let freqs: [f64; 8] = [440.0, 554.37, 659.25, 329.63, 880.0, 1108.73, 220.0, 1318.51];

    for i in 0..total_frames {
        let t = i as f64 / sample_rate as f64;
        for ch in 0..channels as usize {
            let freq = freqs[ch % freqs.len()];
            let val = (t * freq * 2.0 * std::f64::consts::PI).sin();
            let sample = (val * 24000.0) as i16;
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

#[derive(Debug, Clone)]
pub struct BenchResult {
    pub name: String,
    pub duration: Duration,
    pub audio_duration_secs: f64,
    pub pcm_bytes: usize,
    pub output_bytes: usize,
    pub frames_encoded: usize,
}

impl BenchResult {
    pub fn speedup(&self) -> f64 {
        self.audio_duration_secs / self.duration.as_secs_f64()
    }

    pub fn throughput_mb_s(&self) -> f64 {
        (self.pcm_bytes as f64 / (1024.0 * 1024.0)) / self.duration.as_secs_f64()
    }

    pub fn output_bitrate_kbps(&self) -> f64 {
        (self.output_bytes as f64 * 8.0) / (self.audio_duration_secs * 1000.0)
    }

    pub fn avg_frame_latency_us(&self) -> f64 {
        if self.frames_encoded == 0 {
            0.0
        } else {
            (self.duration.as_secs_f64() * 1_000_000.0) / self.frames_encoded as f64
        }
    }

    pub fn print_summary(&self) {
        println!("  Time Taken:           {:.3} ms", self.duration.as_secs_f64() * 1000.0);
        println!("  Speedup:              {:.1}x real-time", self.speedup());
        println!("  PCM Throughput:       {:.2} MB/s", self.throughput_mb_s());
        println!("  Encoded Frames:       {}", self.frames_encoded);
        println!("  Average Frame Time:   {:.2} µs (32.0 ms audio/frame)", self.avg_frame_latency_us());
        println!("  Output Size:          {} bytes ({:.1} kbps)", self.output_bytes, self.output_bitrate_kbps());
    }
}

/// Benchmark oxideav-ac3 encoder
pub fn bench_oxideav_ac3(
    pcm_bytes: &[u8],
    sample_rate: u32,
    channels: u16,
    bitrate_bps: u32,
) -> Result<BenchResult, String> {
    let mut params = CodecParameters::audio(CodecId::new("ac3"));
    params.sample_rate = Some(sample_rate);
    params.channels = Some(channels);
    params.sample_format = Some(SampleFormat::S16);
    params.bit_rate = Some(bitrate_bps as u64);

    let mut encoder = oxideav_ac3::encoder::make_encoder(&params)
        .map_err(|e| format!("Failed to create oxideav-ac3 encoder: {e}"))?;

    let stride = channels as usize * 2;
    let chunk_samples = 1536usize;
    let chunk_bytes = chunk_samples * stride;

    let start = Instant::now();
    let mut out_len = 0;
    let mut frames_encoded = 0;

    for chunk in pcm_bytes.chunks(chunk_bytes) {
        let samples = chunk.len() / stride;
        let frame = Frame::Audio(AudioFrame {
            samples: samples as u32,
            pts: None,
            data: vec![chunk.to_vec()],
        });

        encoder.send_frame(&frame)
            .map_err(|e| format!("send_frame error: {e}"))?;

        while let Ok(packet) = encoder.receive_packet() {
            out_len += packet.data.len();
            frames_encoded += 1;
        }
    }

    let _ = encoder.flush();
    while let Ok(packet) = encoder.receive_packet() {
        out_len += packet.data.len();
        frames_encoded += 1;
    }

    let duration = start.elapsed();
    let audio_duration_secs = (pcm_bytes.len() / stride) as f64 / sample_rate as f64;

    Ok(BenchResult {
        name: "oxideav-ac3".to_string(),
        duration,
        audio_duration_secs,
        pcm_bytes: pcm_bytes.len(),
        output_bytes: out_len,
        frames_encoded,
    })
}

#[cfg(target_os = "macos")]
#[allow(non_snake_case, non_upper_case_globals)]
pub mod apple_native {
    use std::ffi::c_void;
    use std::time::Instant;
    use super::BenchResult;

    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default)]
    pub struct AudioStreamBasicDescription {
        pub mSampleRate: f64,
        pub mFormatID: u32,
        pub mFormatFlags: u32,
        pub mBytesPerPacket: u32,
        pub mFramesPerPacket: u32,
        pub mBytesPerFrame: u32,
        pub mChannelsPerFrame: u32,
        pub mBitsPerChannel: u32,
        pub mReserved: u32,
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct AudioClassDescription {
        pub mType: u32,
        pub mSubType: u32,
        pub mManufacturer: u32,
    }

    pub const kAudioFormatLinearPCM: u32 = 0x6c70636d; // 'lpcm'
    pub const kAudioFormatMPEG4AAC: u32 = 0x61616320; // 'aac '
    pub const kAudioFormatAC3: u32 = 0x61632d33; // 'ac-3'
    pub const kAudioFormatEnhancedAC3: u32 = 0x65632d33; // 'ec-3'
    pub const kAudioFormat60958AC3: u32 = 0x63616333; // 'cac3'

    pub const kAudioFormatFlagIsSignedInteger: u32 = 1 << 2;
    pub const kAudioFormatFlagIsPacked: u32 = 1 << 3;

    pub const kAudioFormatProperty_Encoders: u32 = 0x66656e63; // 'fenc'
    pub const kAudioFormatProperty_Decoders: u32 = 0x66646563; // 'fdec'
    pub const kAudioFormatProperty_EncodeFormatIDs: u32 = 0x61636f66; // 'acof'
    pub const kAudioFormatProperty_DecodeFormatIDs: u32 = 0x61636966; // 'acif'

    pub const kAudioConverterEncodeBitRate: u32 = 0x62726174; // 'brat'

    #[repr(C)]
    pub struct AudioBuffer {
        pub mNumberChannels: u32,
        pub mDataByteSize: u32,
        pub mData: *mut c_void,
    }

    #[repr(C)]
    pub struct AudioBufferList {
        pub mNumberBuffers: u32,
        pub mBuffers: [AudioBuffer; 1],
    }

    #[repr(C)]
    #[derive(Debug, Default, Clone, Copy)]
    pub struct AudioStreamPacketDescription {
        pub mStartOffset: i64,
        pub mVariableFramesInPacket: u32,
        pub mDataByteSize: u32,
    }

    pub type AudioConverterRef = *mut c_void;
    pub type OSStatus = i32;

    pub type AudioConverterComplexInputDataProc = unsafe extern "C" fn(
        inAudioConverter: AudioConverterRef,
        ioNumberDataPackets: *mut u32,
        ioData: *mut AudioBufferList,
        outDataPacketDescription: *mut *mut AudioStreamPacketDescription,
        inUserData: *mut c_void,
    ) -> OSStatus;

    #[link(name = "AudioToolbox", kind = "framework")]
    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        pub fn AudioFormatGetPropertyInfo(
            inPropertyID: u32,
            inSpecifierSize: u32,
            inSpecifier: *const c_void,
            outPropertyDataSize: *mut u32,
        ) -> OSStatus;

        pub fn AudioFormatGetProperty(
            inPropertyID: u32,
            inSpecifierSize: u32,
            inSpecifier: *const c_void,
            ioPropertyDataSize: *mut u32,
            outPropertyData: *mut c_void,
        ) -> OSStatus;

        pub fn AudioConverterNew(
            inSourceFormat: *const AudioStreamBasicDescription,
            inDestinationFormat: *const AudioStreamBasicDescription,
            outAudioConverter: *mut AudioConverterRef,
        ) -> OSStatus;

        pub fn AudioConverterDispose(inAudioConverter: AudioConverterRef) -> OSStatus;

        pub fn AudioConverterSetProperty(
            inAudioConverter: AudioConverterRef,
            inPropertyID: u32,
            inPropertyDataSize: u32,
            inPropertyData: *const c_void,
        ) -> OSStatus;

        pub fn AudioConverterGetProperty(
            inAudioConverter: AudioConverterRef,
            inPropertyID: u32,
            ioPropertyDataSize: *mut u32,
            outPropertyData: *mut c_void,
        ) -> OSStatus;

        pub fn AudioConverterFillComplexBuffer(
            inAudioConverter: AudioConverterRef,
            inInputDataProc: AudioConverterComplexInputDataProc,
            inInputDataProcUserData: *mut c_void,
            ioOutputDataPacketSize: *mut u32,
            outOutputData: *mut AudioBufferList,
            outPacketDescription: *mut AudioStreamPacketDescription,
        ) -> OSStatus;
    }

    pub fn fourcc_to_string(code: u32) -> String {
        let bytes = code.to_be_bytes();
        if bytes.iter().all(|&b| (0x20..=0x7E).contains(&b)) {
            String::from_utf8_lossy(&bytes).to_string()
        } else {
            format!("0x{:08X}", code)
        }
    }

    /// Query and list all audio encoders registered in Apple's AudioToolbox.
    pub fn query_available_encoders() -> Vec<String> {
        let mut size: u32 = 0;
        let status = unsafe {
            AudioFormatGetPropertyInfo(
                kAudioFormatProperty_EncodeFormatIDs,
                0,
                std::ptr::null(),
                &mut size,
            )
        };
        if status != 0 {
            return Vec::new();
        }

        let count = size as usize / std::mem::size_of::<u32>();
        let mut format_ids = vec![0u32; count];
        let status = unsafe {
            AudioFormatGetProperty(
                kAudioFormatProperty_EncodeFormatIDs,
                0,
                std::ptr::null(),
                &mut size,
                format_ids.as_mut_ptr() as *mut c_void,
            )
        };
        if status != 0 {
            return Vec::new();
        }

        format_ids.iter().map(|&id| fourcc_to_string(id)).collect()
    }

    /// Query and list all audio decoders registered in Apple's AudioToolbox.
    pub fn query_available_decoders() -> Vec<String> {
        let mut size: u32 = 0;
        let status = unsafe {
            AudioFormatGetPropertyInfo(
                kAudioFormatProperty_DecodeFormatIDs,
                0,
                std::ptr::null(),
                &mut size,
            )
        };
        if status != 0 {
            return Vec::new();
        }

        let count = size as usize / std::mem::size_of::<u32>();
        let mut format_ids = vec![0u32; count];
        let status = unsafe {
            AudioFormatGetProperty(
                kAudioFormatProperty_DecodeFormatIDs,
                0,
                std::ptr::null(),
                &mut size,
                format_ids.as_mut_ptr() as *mut c_void,
            )
        };
        if status != 0 {
            return Vec::new();
        }

        format_ids.iter().map(|&id| fourcc_to_string(id)).collect()
    }

    struct InputContext<'a> {
        pcm_bytes: &'a [u8],
        pos: usize,
        bytes_per_packet: usize,
        channels: u32,
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
        (*io_data).mBuffers[0].mNumberChannels = ctx.channels;
        (*io_data).mBuffers[0].mDataByteSize = bytes_to_give as u32;
        (*io_data).mBuffers[0].mData = ptr;

        0
    }

    pub fn bench_apple_ac3(
        pcm_bytes: &[u8],
        sample_rate: u32,
        channels: u16,
        bitrate_bps: u32,
    ) -> Result<BenchResult, String> {
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
            mFormatID: kAudioFormatAC3,
            mFormatFlags: 0,
            mBytesPerPacket: 0,
            mFramesPerPacket: 1536,
            mBytesPerFrame: 0,
            mChannelsPerFrame: channels as u32,
            mBitsPerChannel: 0,
            mReserved: 0,
        };

        let mut converter: AudioConverterRef = std::ptr::null_mut();
        let status = unsafe { AudioConverterNew(&in_format, &out_format, &mut converter) };

        if status != 0 {
            let err_str = fourcc_to_string(status as u32);
            return Err(format!(
                "AudioConverterNew for AC-3 failed with OSStatus {status} ('{err_str}'). \
                Apple AudioToolbox on this macOS system does not provide a native AC-3 encoder (only decoders/passthrough)."
            ));
        }

        // Set requested bitrate if supported
        let mut br = bitrate_bps;
        let _ = unsafe {
            AudioConverterSetProperty(
                converter,
                kAudioConverterEncodeBitRate,
                std::mem::size_of::<u32>() as u32,
                &mut br as *mut u32 as *const c_void,
            )
        };

        let start = Instant::now();
        let mut ctx = InputContext {
            pcm_bytes,
            pos: 0,
            bytes_per_packet: (channels * 2) as usize,
            channels: channels as u32,
        };

        let mut out_len = 0;
        let mut frames_encoded = 0;
        let mut out_buf = vec![0u8; 16384];
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

            for i in 0..num_packets as usize {
                out_len += packet_descs[i].mDataByteSize as usize;
                frames_encoded += 1;
            }
        }

        unsafe { AudioConverterDispose(converter) };
        let duration = start.elapsed();
        let audio_duration_secs = (pcm_bytes.len() / (channels as usize * 2)) as f64 / sample_rate as f64;

        Ok(BenchResult {
            name: "Apple Native AudioToolbox (AC-3)".to_string(),
            duration,
            audio_duration_secs,
            pcm_bytes: pcm_bytes.len(),
            output_bytes: out_len,
            frames_encoded,
        })
    }
}

fn main() {
    println!("================================================================================");
    println!("AC-3 ENCODER PERFORMANCE BENCHMARK (RELEASE MODE)");
    println!("================================================================================");

    #[cfg(target_os = "macos")]
    {
        println!("[System Audio Codec Capabilities (macOS AudioToolbox)]");
        let encoders = apple_native::query_available_encoders();
        println!("  Registered Encoders in AudioToolbox: {:?}", encoders);
        let decoders = apple_native::query_available_decoders();
        println!("  Registered Decoders in AudioToolbox: {:?}", decoders);
        println!("--------------------------------------------------------------------------------");
    }

    let sample_rate = 48000;
    let duration_secs = 60.0;

    // -------------------------------------------------------------------------
    // Target Test Case: 5 Channels, 640 kbps (48 kHz, 60s of multi-channel audio)
    // -------------------------------------------------------------------------
    let channels_5 = 5;
    let bitrate_640k = 640_000;

    println!("\n>>> TEST CASE 1: 5 Channels, 640 kbps @ 48 kHz ({} s test signal)", duration_secs);
    println!("--------------------------------------------------------------------------------");

    let pcm_5ch = generate_multichannel_audio(sample_rate, channels_5, duration_secs);
    let pcm_bytes_5ch = pcm_i16_to_u8_le(&pcm_5ch);
    println!("  Generated PCM: {} samples ({:.2} MB)", pcm_5ch.len(), pcm_bytes_5ch.len() as f64 / (1024.0 * 1024.0));

    // Benchmark oxideav-ac3
    println!("\n[1] oxideav-ac3 Encoder (Pure Rust):");
    match bench_oxideav_ac3(&pcm_bytes_5ch, sample_rate, channels_5, bitrate_640k) {
        Ok(res) => res.print_summary(),
        Err(e) => println!("  ERROR: {e}"),
    }

    // Benchmark Apple Native
    #[cfg(target_os = "macos")]
    {
        println!("\n[2] Apple Native Codec (AudioToolbox / CoreAudio):");
        match apple_native::bench_apple_ac3(&pcm_bytes_5ch, sample_rate, channels_5, bitrate_640k) {
            Ok(res) => res.print_summary(),
            Err(e) => println!("  NOTE: {e}"),
        }
    }

    // -------------------------------------------------------------------------
    // Additional Test Case: 5.1 Channels (6 ch), 640 kbps
    // -------------------------------------------------------------------------
    let channels_6 = 6;
    println!("\n>>> TEST CASE 2: 5.1 Channels (6 ch), 640 kbps @ 48 kHz ({} s test signal)", duration_secs);
    println!("--------------------------------------------------------------------------------");

    let pcm_6ch = generate_multichannel_audio(sample_rate, channels_6, duration_secs);
    let pcm_bytes_6ch = pcm_i16_to_u8_le(&pcm_6ch);
    println!("  Generated PCM: {} samples ({:.2} MB)", pcm_6ch.len(), pcm_bytes_6ch.len() as f64 / (1024.0 * 1024.0));

    println!("\n[1] oxideav-ac3 Encoder (Pure Rust):");
    match bench_oxideav_ac3(&pcm_bytes_6ch, sample_rate, channels_6, bitrate_640k) {
        Ok(res) => res.print_summary(),
        Err(e) => println!("  ERROR: {e}"),
    }

    #[cfg(target_os = "macos")]
    {
        println!("\n[2] Apple Native Codec (AudioToolbox / CoreAudio):");
        match apple_native::bench_apple_ac3(&pcm_bytes_6ch, sample_rate, channels_6, bitrate_640k) {
            Ok(res) => res.print_summary(),
            Err(e) => println!("  NOTE: {e}"),
        }
    }

    // -------------------------------------------------------------------------
    // Additional Test Case: Stereo (2 ch), 192 kbps
    // -------------------------------------------------------------------------
    let channels_2 = 2;
    let bitrate_192k = 192_000;
    println!("\n>>> TEST CASE 3: Stereo (2 ch), 192 kbps @ 48 kHz ({} s test signal)", duration_secs);
    println!("--------------------------------------------------------------------------------");

    let pcm_2ch = generate_multichannel_audio(sample_rate, channels_2, duration_secs);
    let pcm_bytes_2ch = pcm_i16_to_u8_le(&pcm_2ch);
    println!("  Generated PCM: {} samples ({:.2} MB)", pcm_2ch.len(), pcm_bytes_2ch.len() as f64 / (1024.0 * 1024.0));

    println!("\n[1] oxideav-ac3 Encoder (Pure Rust):");
    match bench_oxideav_ac3(&pcm_bytes_2ch, sample_rate, channels_2, bitrate_192k) {
        Ok(res) => res.print_summary(),
        Err(e) => println!("  ERROR: {e}"),
    }

    #[cfg(target_os = "macos")]
    {
        println!("\n[2] Apple Native Codec (AudioToolbox / CoreAudio):");
        match apple_native::bench_apple_ac3(&pcm_bytes_2ch, sample_rate, channels_2, bitrate_192k) {
            Ok(res) => res.print_summary(),
            Err(e) => println!("  NOTE: {e}"),
        }
    }

    println!("\n================================================================================");
}

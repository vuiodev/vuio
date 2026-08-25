//! AC-3 Encoder Benchmark & Quality Suite: oxideav-ac3 vs FFmpeg & Apple Native
//!
//! Measures encoding throughput, real-time speedup factor, latency, and
//! reconstruction fidelity (SNR, PSNR, RMS error) for multi-channel AC-3 encoding.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Generates multi-channel sine wave audio for benchmarking.
/// Each channel gets a distinct frequency so the encoder processes
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

fn pcm_u8_le_to_i16(bytes: &[u8]) -> Vec<i16> {
    let mut pcm = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        pcm.push(i16::from_le_bytes([chunk[0], chunk[1]]));
    }
    pcm
}

#[derive(Debug, Clone)]
pub struct BenchResult {
    pub name: String,
    pub duration: Duration,
    pub audio_duration_secs: f64,
    pub pcm_bytes: usize,
    pub output_bytes: usize,
    pub frames_encoded: usize,
    pub encoded_data: Vec<u8>,
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
    _bitrate_bps: u32,
) -> Result<BenchResult, String> {
    let mut encoder = vuio_core::media::transcode::Ac3Encoder::new(sample_rate, channels)
        .map_err(|e| format!("Failed to create Ac3Encoder: {e}"))?;

    let stride = channels as usize * 2;
    let chunk_samples = 1536usize;
    let chunk_bytes = chunk_samples * stride;

    let start = Instant::now();
    let mut out_data = Vec::with_capacity(pcm_bytes.len() / 10);
    let mut frames_encoded = 0;

    for chunk in pcm_bytes.chunks(chunk_bytes) {
        let packets = encoder.push(chunk)
            .map_err(|e| format!("push error: {e}"))?;
        for pkt in packets {
            out_data.extend_from_slice(&pkt);
            frames_encoded += 1;
        }
    }

    let packets = encoder.finish();
    for pkt in packets {
        out_data.extend_from_slice(&pkt);
        frames_encoded += 1;
    }

    let duration = start.elapsed();
    let audio_duration_secs = (pcm_bytes.len() / stride) as f64 / sample_rate as f64;
    let output_bytes = out_data.len();

    Ok(BenchResult {
        name: "oxideav-ac3".to_string(),
        duration,
        audio_duration_secs,
        pcm_bytes: pcm_bytes.len(),
        output_bytes,
        frames_encoded,
        encoded_data: out_data,
    })
}

/// Benchmark FFmpeg CLI AC-3 encoder
pub fn bench_ffmpeg_ac3(
    pcm_bytes: &[u8],
    sample_rate: u32,
    channels: u16,
    bitrate_bps: u32,
) -> Result<BenchResult, String> {
    let bitrate_str = format!("{}k", bitrate_bps / 1000);
    let start = Instant::now();

    let mut child = Command::new("ffmpeg")
        .args([
            "-y",
            "-hide_banner",
            "-loglevel", "error",
            "-f", "s16le",
            "-ar", &sample_rate.to_string(),
            "-ac", &channels.to_string(),
            "-i", "-",
            "-c:a", "ac3",
            "-b:a", &bitrate_str,
            "-f", "ac3",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn ffmpeg: {e}"))?;

    let pcm_vec = pcm_bytes.to_vec();
    let mut stdin = child.stdin.take().expect("failed to open stdin");
    let writer_handle = std::thread::spawn(move || {
        let _ = stdin.write_all(&pcm_vec);
    });

    let output = child.wait_with_output()
        .map_err(|e| format!("Failed to wait on ffmpeg: {e}"))?;
    let _ = writer_handle.join();

    let duration = start.elapsed();
    let stride = channels as usize * 2;
    let audio_duration_secs = (pcm_bytes.len() / stride) as f64 / sample_rate as f64;
    let frames = output.stdout.len() / (bitrate_bps as usize * 1536 / (sample_rate as usize * 8));

    Ok(BenchResult {
        name: "ffmpeg (libavcodec ac3)".to_string(),
        duration,
        audio_duration_secs,
        pcm_bytes: pcm_bytes.len(),
        output_bytes: output.stdout.len(),
        frames_encoded: frames,
        encoded_data: output.stdout,
    })
}

/// Decodes AC-3 byte stream back to PCM S16LE using FFmpeg standard decoder.
pub fn decode_ac3_with_ffmpeg(
    ac3_bytes: &[u8],
    sample_rate: u32,
    channels: u16,
) -> Result<Vec<i16>, String> {
    let mut child = Command::new("ffmpeg")
        .args([
            "-y",
            "-hide_banner",
            "-loglevel", "error",
            "-f", "ac3",
            "-i", "-",
            "-f", "s16le",
            "-acodec", "pcm_s16le",
            "-ar", &sample_rate.to_string(),
            "-ac", &channels.to_string(),
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn ffmpeg decoder: {e}"))?;

    let data_vec = ac3_bytes.to_vec();
    let mut stdin = child.stdin.take().expect("failed to open stdin");
    let writer_handle = std::thread::spawn(move || {
        let _ = stdin.write_all(&data_vec);
    });

    let output = child.wait_with_output()
        .map_err(|e| format!("Failed to read decoded pcm from ffmpeg: {e}"))?;
    let _ = writer_handle.join();

    Ok(pcm_u8_le_to_i16(&output.stdout))
}

/// Compute per-channel SNR (dB), PSNR (dB), and RMS reconstruction error.
#[derive(Debug, Clone)]
pub struct QualityReport {
    pub per_channel_snr_db: Vec<f64>,
    pub per_channel_psnr_db: Vec<f64>,
    pub per_channel_rms: Vec<f64>,
    pub overall_snr_db: f64,
    pub overall_psnr_db: f64,
    pub overall_rms: f64,
}

pub fn evaluate_quality(
    orig_pcm: &[i16],
    decoded_pcm: &[i16],
    channels: u16,
) -> QualityReport {
    let ch = channels as usize;
    let total_orig_frames = orig_pcm.len() / ch;
    let total_dec_frames = decoded_pcm.len() / ch;
    let frames = total_orig_frames.min(total_dec_frames);

    // AC-3 codec pipeline introduces a 256 or 512-sample algorithmic MDCT delay.
    // We find the optimal cross-correlation frame alignment offset (0..1024).
    let mut best_delay = 0usize;
    let mut min_sse = f64::MAX;

    for test_delay in (0..=768).step_by(32) {
        if frames <= test_delay + 2048 {
            break;
        }
        let mut sse = 0.0f64;
        let test_len = 2048;
        for i in 0..test_len {
            for c in 0..ch {
                let orig = orig_pcm[(test_delay + i) * ch + c] as f64;
                let dec = decoded_pcm[i * ch + c] as f64;
                let diff = orig - dec;
                sse += diff * diff;
            }
        }
        if sse < min_sse {
            min_sse = sse;
            best_delay = test_delay;
        }
    }

    let align_frames = frames.saturating_sub(best_delay + 256);
    let mut ch_sig_energy = vec![0.0f64; ch];
    let mut ch_noise_energy = vec![0.0f64; ch];

    for i in 0..align_frames {
        for c in 0..ch {
            let orig = orig_pcm[(best_delay + i) * ch + c] as f64;
            let dec = decoded_pcm[i * ch + c] as f64;
            let diff = orig - dec;
            ch_sig_energy[c] += orig * orig;
            ch_noise_energy[c] += diff * diff;
        }
    }

    let mut per_ch_snr = Vec::with_capacity(ch);
    let mut per_ch_psnr = Vec::with_capacity(ch);
    let mut per_ch_rms = Vec::with_capacity(ch);

    let mut tot_sig = 0.0f64;
    let mut tot_noise = 0.0f64;

    for c in 0..ch {
        let sig = ch_sig_energy[c];
        let noise = ch_noise_energy[c].max(1e-12);
        let rms = (noise / align_frames as f64).sqrt();
        let snr = 10.0 * (sig / noise).log10();
        let psnr = 20.0 * (32767.0 / rms.max(1e-6)).log10();

        per_ch_snr.push(snr);
        per_ch_psnr.push(psnr);
        per_ch_rms.push(rms);

        tot_sig += sig;
        tot_noise += noise;
    }

    let overall_rms = (tot_noise / (align_frames * ch) as f64).sqrt();
    let overall_snr = 10.0 * (tot_sig / tot_noise.max(1e-12)).log10();
    let overall_psnr = 20.0 * (32767.0 / overall_rms.max(1e-6)).log10();

    QualityReport {
        per_channel_snr_db: per_ch_snr,
        per_channel_psnr_db: per_ch_psnr,
        per_channel_rms: per_ch_rms,
        overall_snr_db: overall_snr,
        overall_psnr_db: overall_psnr,
        overall_rms,
    }
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

    type OSStatus = i32;
    type AudioConverterRef = *mut c_void;

    pub const kAudioFormatLinearPCM: u32 = 0x6C70636D; // 'lpcm'
    pub const kAudioFormatAC3: u32 = 0x61632D33;       // 'ac-3'
    pub const kAudioFormatFlagIsSignedInteger: u32 = 1 << 2;
    pub const kAudioFormatFlagsNativeEndian: u32 = 0;

    #[link(name = "AudioToolbox", kind = "framework")]
    extern "C" {
        pub fn AudioConverterNew(
            inSourceFormat: *const AudioStreamBasicDescription,
            inDestinationFormat: *const AudioStreamBasicDescription,
            outAudioConverter: *mut AudioConverterRef,
        ) -> OSStatus;

        pub fn AudioConverterDispose(inAudioConverter: AudioConverterRef) -> OSStatus;

        pub fn AudioFormatGetProperty(
            inPropertyID: u32,
            inSpecifierSize: u32,
            inSpecifier: *const c_void,
            ioPropertyDataSize: *mut u32,
            outPropertyData: *mut c_void,
        ) -> OSStatus;
    }

    pub const kAudioFormatProperty_Encoders: u32 = 0x656E636F; // 'enco'
    pub const kAudioFormatProperty_Decoders: u32 = 0x6465636F; // 'deco'

    fn u32_to_fourcc(val: u32) -> String {
        let bytes = val.to_be_bytes();
        if bytes.iter().all(|&b| b.is_ascii_graphic() || b == b' ') {
            String::from_utf8_lossy(&bytes).to_string()
        } else {
            format!("0x{:08X}", val)
        }
    }

    pub fn query_available_encoders() -> Vec<String> {
        let mut size = 0u32;
        let mut prop_val = kAudioFormatAC3;
        let status = unsafe {
            AudioFormatGetProperty(
                kAudioFormatProperty_Encoders,
                std::mem::size_of::<u32>() as u32,
                &mut prop_val as *mut _ as *const c_void,
                &mut size,
                std::ptr::null_mut(),
            )
        };
        if status != 0 || size == 0 {
            let mut all_size = 0u32;
            let status_all = unsafe {
                AudioFormatGetProperty(
                    kAudioFormatProperty_Encoders,
                    0,
                    std::ptr::null(),
                    &mut all_size,
                    std::ptr::null_mut(),
                )
            };
            if status_all == 0 && all_size > 0 {
                let count = all_size as usize / std::mem::size_of::<AudioStreamBasicDescription>();
                let mut descs = vec![AudioStreamBasicDescription::default(); count];
                let _ = unsafe {
                    AudioFormatGetProperty(
                        kAudioFormatProperty_Encoders,
                        0,
                        std::ptr::null(),
                        &mut all_size,
                        descs.as_mut_ptr() as *mut c_void,
                    )
                };
                return descs.into_iter().map(|d| u32_to_fourcc(d.mFormatID)).collect();
            }
            return vec![];
        }
        vec![]
    }

    pub fn query_available_decoders() -> Vec<String> {
        let mut all_size = 0u32;
        let status = unsafe {
            AudioFormatGetProperty(
                kAudioFormatProperty_Decoders,
                0,
                std::ptr::null(),
                &mut all_size,
                std::ptr::null_mut(),
            )
        };
        if status == 0 && all_size > 0 {
            let count = all_size as usize / std::mem::size_of::<AudioStreamBasicDescription>();
            let mut descs = vec![AudioStreamBasicDescription::default(); count];
            let _ = unsafe {
                AudioFormatGetProperty(
                    kAudioFormatProperty_Decoders,
                    0,
                    std::ptr::null(),
                    &mut all_size,
                    descs.as_mut_ptr() as *mut c_void,
                )
            };
            return descs.into_iter().map(|d| u32_to_fourcc(d.mFormatID)).collect();
        }
        vec![]
    }

    pub fn bench_apple_ac3(
        pcm_bytes: &[u8],
        sample_rate: u32,
        channels: u16,
        _bitrate_bps: u32,
    ) -> Result<BenchResult, String> {
        let in_format = AudioStreamBasicDescription {
            mSampleRate: sample_rate as f64,
            mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagIsSignedInteger | kAudioFormatFlagsNativeEndian,
            mBytesPerPacket: channels as u32 * 2,
            mFramesPerPacket: 1,
            mBytesPerFrame: channels as u32 * 2,
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
        let status = unsafe {
            AudioConverterNew(&in_format, &out_format, &mut converter)
        };

        if status != 0 || converter.is_null() {
            return Err(format!(
                "AudioConverterNew for AC-3 failed with OSStatus {} ('{}'). Apple AudioToolbox on this macOS system does not provide a native AC-3 encoder (only decoders/passthrough).",
                status,
                u32_to_fourcc(status as u32)
            ));
        }

        unsafe { AudioConverterDispose(converter); }

        Ok(BenchResult {
            name: "Apple AudioToolbox Native".to_string(),
            duration: Instant::now().elapsed(),
            audio_duration_secs: 0.0,
            pcm_bytes: pcm_bytes.len(),
            output_bytes: 0,
            frames_encoded: 0,
            encoded_data: Vec::new(),
        })
    }
}

fn print_quality_table(report: &QualityReport, label: &str) {
    println!("  [{label}] Reconstruction Fidelity (vs Original Reference):");
    println!("    Overall SNR:    {:>6.2} dB", report.overall_snr_db);
    println!("    Overall PSNR:   {:>6.2} dB", report.overall_psnr_db);
    println!("    Overall RMS:    {:>6.2} (amplitude error on 16-bit PCM)", report.overall_rms);
    print!("    Per-Channel SNR (dB):  [");
    for (i, snr) in report.per_channel_snr_db.iter().enumerate() {
        if i > 0 { print!(", "); }
        print!("Ch{i}: {snr:.1}");
    }
    println!("]");
    print!("    Per-Channel PSNR (dB): [");
    for (i, psnr) in report.per_channel_psnr_db.iter().enumerate() {
        if i > 0 { print!(", "); }
        print!("Ch{i}: {psnr:.1}");
    }
    println!("]");
}

fn main() {
    println!("================================================================================");
    println!("AC-3 ENCODER PERFORMANCE & QUALITY BENCHMARK (RELEASE MODE)");
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

    // 1. oxideav-ac3
    println!("\n[1] oxideav-ac3 Encoder (Pure Rust SIMD-accelerated):");
    let ox_res_5 = bench_oxideav_ac3(&pcm_bytes_5ch, sample_rate, channels_5, bitrate_640k);
    match &ox_res_5 {
        Ok(res) => res.print_summary(),
        Err(e) => println!("  ERROR: {e}"),
    }

    // 2. FFmpeg ac3
    println!("\n[2] FFmpeg AC-3 Encoder (C reference libavcodec):");
    let ff_res_5 = bench_ffmpeg_ac3(&pcm_bytes_5ch, sample_rate, channels_5, bitrate_640k);
    match &ff_res_5 {
        Ok(res) => res.print_summary(),
        Err(e) => println!("  ERROR: {e}"),
    }

    // 3. Apple Native
    #[cfg(target_os = "macos")]
    {
        println!("\n[3] Apple Native Codec (AudioToolbox / CoreAudio):");
        match apple_native::bench_apple_ac3(&pcm_bytes_5ch, sample_rate, channels_5, bitrate_640k) {
            Ok(res) => res.print_summary(),
            Err(e) => println!("  NOTE: {e}"),
        }
    }

    // Quality Comparison
    println!("\n--- Quality & Reconstruction Accuracy (5 Channels, 640 kbps) ---");
    if let Ok(ox) = &ox_res_5 {
        if let Ok(dec_pcm) = decode_ac3_with_ffmpeg(&ox.encoded_data, sample_rate, channels_5) {
            let report = evaluate_quality(&pcm_5ch, &dec_pcm, channels_5);
            print_quality_table(&report, "oxideav-ac3");
        }
    }
    if let Ok(ff) = &ff_res_5 {
        if let Ok(dec_pcm) = decode_ac3_with_ffmpeg(&ff.encoded_data, sample_rate, channels_5) {
            let report = evaluate_quality(&pcm_5ch, &dec_pcm, channels_5);
            print_quality_table(&report, "FFmpeg AC-3");
        }
    }

    // -------------------------------------------------------------------------
    // Target Test Case 2: 5.1 Channels (6 ch), 640 kbps
    // -------------------------------------------------------------------------
    let channels_6 = 6;
    println!("\n>>> TEST CASE 2: 5.1 Channels (6 ch), 640 kbps @ 48 kHz ({} s test signal)", duration_secs);
    println!("--------------------------------------------------------------------------------");

    let pcm_6ch = generate_multichannel_audio(sample_rate, channels_6, duration_secs);
    let pcm_bytes_6ch = pcm_i16_to_u8_le(&pcm_6ch);
    println!("  Generated PCM: {} samples ({:.2} MB)", pcm_6ch.len(), pcm_bytes_6ch.len() as f64 / (1024.0 * 1024.0));

    println!("\n[1] oxideav-ac3 Encoder (Pure Rust SIMD-accelerated):");
    let ox_res_6 = bench_oxideav_ac3(&pcm_bytes_6ch, sample_rate, channels_6, bitrate_640k);
    match &ox_res_6 {
        Ok(res) => res.print_summary(),
        Err(e) => println!("  ERROR: {e}"),
    }

    println!("\n[2] FFmpeg AC-3 Encoder (C reference libavcodec):");
    let ff_res_6 = bench_ffmpeg_ac3(&pcm_bytes_6ch, sample_rate, channels_6, bitrate_640k);
    match &ff_res_6 {
        Ok(res) => res.print_summary(),
        Err(e) => println!("  ERROR: {e}"),
    }

    println!("\n--- Quality & Reconstruction Accuracy (5.1 Channels, 640 kbps) ---");
    if let Ok(ox) = &ox_res_6 {
        if let Ok(dec_pcm) = decode_ac3_with_ffmpeg(&ox.encoded_data, sample_rate, channels_6) {
            let report = evaluate_quality(&pcm_6ch, &dec_pcm, channels_6);
            print_quality_table(&report, "oxideav-ac3");
        }
    }
    if let Ok(ff) = &ff_res_6 {
        if let Ok(dec_pcm) = decode_ac3_with_ffmpeg(&ff.encoded_data, sample_rate, channels_6) {
            let report = evaluate_quality(&pcm_6ch, &dec_pcm, channels_6);
            print_quality_table(&report, "FFmpeg AC-3");
        }
    }

    // -------------------------------------------------------------------------
    // Target Test Case 3: Stereo (2 ch), 192 kbps
    // -------------------------------------------------------------------------
    let channels_2 = 2;
    let bitrate_192k = 192_000;
    println!("\n>>> TEST CASE 3: Stereo (2 ch), 192 kbps @ 48 kHz ({} s test signal)", duration_secs);
    println!("--------------------------------------------------------------------------------");

    let pcm_2ch = generate_multichannel_audio(sample_rate, channels_2, duration_secs);
    let pcm_bytes_2ch = pcm_i16_to_u8_le(&pcm_2ch);
    println!("  Generated PCM: {} samples ({:.2} MB)", pcm_2ch.len(), pcm_bytes_2ch.len() as f64 / (1024.0 * 1024.0));

    println!("\n[1] oxideav-ac3 Encoder (Pure Rust SIMD-accelerated):");
    let ox_res_2 = bench_oxideav_ac3(&pcm_bytes_2ch, sample_rate, channels_2, bitrate_192k);
    match &ox_res_2 {
        Ok(res) => res.print_summary(),
        Err(e) => println!("  ERROR: {e}"),
    }

    println!("\n[2] FFmpeg AC-3 Encoder (C reference libavcodec):");
    let ff_res_2 = bench_ffmpeg_ac3(&pcm_bytes_2ch, sample_rate, channels_2, bitrate_192k);
    match &ff_res_2 {
        Ok(res) => res.print_summary(),
        Err(e) => println!("  ERROR: {e}"),
    }

    println!("\n--- Quality & Reconstruction Accuracy (Stereo, 192 kbps) ---");
    if let Ok(ox) = &ox_res_2 {
        if let Ok(dec_pcm) = decode_ac3_with_ffmpeg(&ox.encoded_data, sample_rate, channels_2) {
            let report = evaluate_quality(&pcm_2ch, &dec_pcm, channels_2);
            print_quality_table(&report, "oxideav-ac3");
        }
    }
    if let Ok(ff) = &ff_res_2 {
        if let Ok(dec_pcm) = decode_ac3_with_ffmpeg(&ff.encoded_data, sample_rate, channels_2) {
            let report = evaluate_quality(&pcm_2ch, &dec_pcm, channels_2);
            print_quality_table(&report, "FFmpeg AC-3");
        }
    }

    println!("\n================================================================================");
}

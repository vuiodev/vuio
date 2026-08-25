use std::ffi::c_void;
use std::mem::zeroed;
use std::ptr;

use crate::error::{Error, Result};
use crate::ffi::{check_encoder, encoder_version};
use crate::util::{VersionInfo, encoder_alloc, encoder_free};

const USAC_ONLY_SWITCHED: i32 = 0;
const USAC_ONLY_FD: i32 = 1;
const USAC_ONLY_TD: i32 = 2;

const NO_SBR_CCFL_768: i32 = 0;
const NO_SBR_CCFL_1024: i32 = 1;
const SBR_8_3_CCFL_768: i32 = 2;
const SBR_2_1_CCFL_1024: i32 = 3;
const SBR_4_1_CCFL_1024: i32 = 4;

const DEFAULT_LC_BITRESERVOIR: i32 = 768;
const DEFAULT_LD_BITRESERVOIR: i32 = 384;
const METHOD_DEFINITION_PROGRAM_LOUDNESS: u32 = 1;
const MEASUREMENT_SYSTEM_BS_1770_3: u32 = 2;
const DEFAULT_RAP_INTERVAL_IN_MS: i32 = -1;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    AacLc,
    HeAacV1,
    AacLd,
    HeAacV2,
    AacEld,
    Usac,
}

impl Profile {
    fn aot(self) -> i32 {
        match self {
            Self::AacLc => libxaac_sys::AOT_AAC_LC as i32,
            Self::HeAacV1 => libxaac_sys::AOT_SBR as i32,
            Self::AacLd => libxaac_sys::AOT_AAC_LD as i32,
            Self::HeAacV2 => libxaac_sys::AOT_PS as i32,
            Self::AacEld => libxaac_sys::AOT_AAC_ELD as i32,
            Self::Usac => libxaac_sys::AOT_USAC as i32,
        }
    }

    fn default_frame_length(self) -> u16 {
        match self {
            Self::AacLc | Self::HeAacV1 | Self::HeAacV2 => 1024,
            Self::AacLd | Self::AacEld => 512,
            Self::Usac => 1024,
        }
    }

    fn validate_frame_length(self, frame_length: u16) -> bool {
        match self {
            Self::AacLc | Self::HeAacV1 | Self::HeAacV2 => matches!(frame_length, 960 | 1024),
            Self::AacLd | Self::AacEld => matches!(frame_length, 480 | 512),
            Self::Usac => matches!(frame_length, 768 | 1024),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Raw,
    Adts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InverseQuantizationMode {
    Off,
    On,
    High,
}

impl InverseQuantizationMode {
    fn raw(self) -> i32 {
        match self {
            Self::Off => 0,
            Self::On => 1,
            Self::High => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsacCodecMode {
    Switched,
    FrequencyDomain,
    TimeDomain,
}

impl UsacCodecMode {
    fn raw(self) -> i32 {
        match self {
            Self::Switched => USAC_ONLY_SWITCHED,
            Self::FrequencyDomain => USAC_ONLY_FD,
            Self::TimeDomain => USAC_ONLY_TD,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsacFrameLengthIndex {
    NoSbr768,
    NoSbr1024,
    Esbr8_3_768,
    Esbr2_1_1024,
    Esbr4_1_1024,
}

impl UsacFrameLengthIndex {
    fn raw(self) -> i32 {
        match self {
            Self::NoSbr768 => NO_SBR_CCFL_768,
            Self::NoSbr1024 => NO_SBR_CCFL_1024,
            Self::Esbr8_3_768 => SBR_8_3_CCFL_768,
            Self::Esbr2_1_1024 => SBR_2_1_CCFL_1024,
            Self::Esbr4_1_1024 => SBR_4_1_CCFL_1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderDrcConfig {
    pub use_drc_element: bool,
    pub frame_size: Option<i32>,
    pub config_payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct EncoderConfig {
    pub profile: Profile,
    pub sample_rate: u32,
    pub native_sample_rate: Option<u32>,
    pub channels: u16,
    pub channel_mask: u32,
    pub num_coupling_channels: u16,
    pub bitrate: u32,
    pub pcm_word_size: u16,
    pub frame_length: Option<u16>,
    pub output_format: OutputFormat,
    pub bandwidth: Option<u32>,
    pub dual_mono: bool,
    pub use_tns: bool,
    pub noise_filling: bool,
    pub full_bandwidth: bool,
    pub private_bit: bool,
    pub copyright_bit: bool,
    pub original_copy_bit: bool,
    pub no_stereo_preprocessing: bool,
    pub use_mps: bool,
    pub mps_tree_config: Option<i32>,
    pub complex_prediction: bool,
    pub bit_reservoir_size: Option<i32>,
    pub inverse_quantization: InverseQuantizationMode,
    pub enable_esbr: Option<bool>,
    pub high_quality_esbr: bool,
    pub pvc: bool,
    pub harmonic_sbr: bool,
    pub inter_tes: bool,
    pub usac_mode: UsacCodecMode,
    pub usac_frame_length_index: UsacFrameLengthIndex,
    pub random_access_interval_ms: i32,
    pub stream_id: u16,
    pub use_delay_adjustment: bool,
    pub write_program_config_element: bool,
    pub measured_loudness: Option<f64>,
    pub sample_peak_level: Option<f32>,
    pub drc: Option<EncoderDrcConfig>,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            profile: Profile::AacLc,
            sample_rate: 44_100,
            native_sample_rate: None,
            channels: 2,
            channel_mask: 0,
            num_coupling_channels: 0,
            bitrate: 48_000,
            pcm_word_size: 16,
            frame_length: None,
            output_format: OutputFormat::Raw,
            bandwidth: None,
            dual_mono: false,
            use_tns: true,
            noise_filling: false,
            full_bandwidth: false,
            private_bit: false,
            copyright_bit: false,
            original_copy_bit: false,
            no_stereo_preprocessing: false,
            use_mps: false,
            mps_tree_config: None,
            complex_prediction: false,
            bit_reservoir_size: None,
            inverse_quantization: InverseQuantizationMode::High,
            enable_esbr: None,
            high_quality_esbr: false,
            pvc: false,
            harmonic_sbr: false,
            inter_tes: false,
            usac_mode: UsacCodecMode::FrequencyDomain,
            usac_frame_length_index: UsacFrameLengthIndex::NoSbr1024,
            random_access_interval_ms: DEFAULT_RAP_INTERVAL_IN_MS,
            stream_id: 0,
            use_delay_adjustment: true,
            write_program_config_element: false,
            measured_loudness: None,
            sample_peak_level: None,
            drc: None,
        }
    }
}

impl EncoderConfig {
    fn validate(&self) -> Result<u16> {
        if self.channels == 0 {
            return Err(Error::InvalidConfig(
                "channel count must be greater than zero",
            ));
        }
        if self.bitrate == 0 {
            return Err(Error::InvalidConfig("bitrate must be greater than zero"));
        }
        if !matches!(self.pcm_word_size, 16 | 24 | 32) {
            return Err(Error::InvalidConfig("pcm_word_size must be 16, 24, or 32"));
        }
        let frame_length = self
            .frame_length
            .unwrap_or_else(|| self.profile.default_frame_length());
        if !self.profile.validate_frame_length(frame_length) {
            return Err(Error::InvalidConfig(
                "frame_length is not valid for the selected profile",
            ));
        }
        if self.profile != Profile::Usac && self.output_format == OutputFormat::Adts && self.use_mps
        {
            return Err(Error::InvalidConfig(
                "MPS is only usable with elementary streams",
            ));
        }
        if self.bandwidth == Some(0) {
            return Err(Error::InvalidConfig("bandwidth must be greater than zero"));
        }
        Ok(frame_length)
    }
}

#[derive(Debug, Clone)]
pub struct EncodedPacket {
    pub data: Vec<u8>,
    pub bytes_consumed: usize,
}

#[derive(Debug, Clone)]
pub struct EncodedFrame {
    pub packet: EncodedPacket,
    pub padded_input_bytes: usize,
}

#[derive(Debug)]
pub struct Encoder {
    config: EncoderConfig,
    ffi: Box<libxaac_sys::ixheaace_user_config_struct>,
    input_size: usize,
    audio_specific_config: Vec<u8>,
    version: VersionInfo,
    _drc_payload: Option<Box<[u8]>>,
}

impl Encoder {
    pub fn new(config: EncoderConfig) -> Result<Self> {
        let frame_length = config.validate()?;
        let mut ffi: Box<libxaac_sys::ixheaace_user_config_struct> = Box::new(unsafe { zeroed() });
        let version = encoder_version()?;
        let drc_payload = config
            .drc
            .as_ref()
            .filter(|drc| !drc.config_payload.is_empty())
            .map(|drc| drc.config_payload.clone().into_boxed_slice());

        ffi.output_config.malloc_xheaace = encoder_alloc();
        ffi.output_config.free_xheaace = encoder_free();

        ffi.input_config.ui_pcm_wd_sz = config.pcm_word_size as u32;
        ffi.input_config.i_bitrate = config.bitrate as i32;
        ffi.input_config.frame_length = frame_length as i32;
        ffi.input_config.frame_cmd_flag = i32::from(config.frame_length.is_some());
        ffi.input_config.aot = config.profile.aot();
        ffi.input_config.i_mps_tree_config = config.mps_tree_config.unwrap_or(-1);
        ffi.input_config.esbr_flag = i32::from(
            config
                .enable_esbr
                .unwrap_or(config.profile == Profile::Usac),
        );
        ffi.input_config.i_channels = i32::from(config.channels);
        ffi.input_config.i_samp_freq = config.sample_rate;
        ffi.input_config.i_native_samp_freq =
            config.native_sample_rate.unwrap_or(config.sample_rate) as i32;
        ffi.input_config.i_channels_mask = config.channel_mask as i32;
        ffi.input_config.i_num_coupling_chan = i32::from(config.num_coupling_channels);
        ffi.input_config.i_use_mps = i32::from(config.use_mps);
        ffi.input_config.i_use_adts = i32::from(config.output_format == OutputFormat::Adts);
        ffi.input_config.i_use_es = i32::from(config.output_format == OutputFormat::Raw);
        ffi.input_config.usac_en = i32::from(config.profile == Profile::Usac);
        ffi.input_config.codec_mode = config.usac_mode.raw();
        ffi.input_config.cplx_pred = i32::from(config.complex_prediction);
        ffi.input_config.ccfl_idx = config.usac_frame_length_index.raw();
        ffi.input_config.pvc_active = i32::from(config.pvc);
        ffi.input_config.harmonic_sbr = i32::from(config.harmonic_sbr);
        ffi.input_config.inter_tes_active = i32::from(config.inter_tes);
        ffi.input_config.pv_drc_cfg = drc_payload
            .as_ref()
            .map_or(ptr::null_mut(), |payload| payload.as_ptr() as *mut _);
        ffi.input_config.use_drc_element = i32::from(
            config
                .drc
                .as_ref()
                .map(|drc| drc.use_drc_element)
                .unwrap_or(false),
        );
        ffi.input_config.drc_frame_size = config
            .drc
            .as_ref()
            .and_then(|drc| drc.frame_size)
            .unwrap_or(0);
        ffi.input_config.hq_esbr = i32::from(config.high_quality_esbr);
        ffi.input_config.write_program_config_element =
            i32::from(config.write_program_config_element);
        ffi.input_config.random_access_interval = config.random_access_interval_ms;
        ffi.input_config.method_def = METHOD_DEFINITION_PROGRAM_LOUDNESS;
        ffi.input_config.measured_loudness = config.measured_loudness.unwrap_or(0.0);
        ffi.input_config.measurement_system = MEASUREMENT_SYSTEM_BS_1770_3;
        ffi.input_config.sample_peak_level = config.sample_peak_level.unwrap_or(0.0);
        ffi.input_config.stream_id = config.stream_id;
        ffi.input_config.use_delay_adjustment = i32::from(config.use_delay_adjustment);
        ffi.input_config.user_tns_flag = 1;
        ffi.input_config.user_esbr_flag = i32::from(config.enable_esbr.is_some());

        ffi.input_config.aac_config.bitrate = config.bitrate as i32;
        ffi.input_config.aac_config.sample_rate = config.sample_rate as i32;
        ffi.input_config.aac_config.num_channels_in = i32::from(config.channels);
        ffi.input_config.aac_config.num_channels_out = i32::from(config.channels);
        ffi.input_config.aac_config.bandwidth = config.bandwidth.unwrap_or_default() as i32;
        ffi.input_config.aac_config.dual_mono = i32::from(config.dual_mono);
        ffi.input_config.aac_config.use_tns = i32::from(config.use_tns);
        ffi.input_config.aac_config.noise_filling = i32::from(config.noise_filling);
        ffi.input_config.aac_config.private_bit = i32::from(config.private_bit);
        ffi.input_config.aac_config.copyright_bit = i32::from(config.copyright_bit);
        ffi.input_config.aac_config.original_copy_bit = i32::from(config.original_copy_bit);
        ffi.input_config.aac_config.f_no_stereo_preprocessing =
            i32::from(config.no_stereo_preprocessing);
        ffi.input_config.aac_config.full_bandwidth = i32::from(config.full_bandwidth);
        ffi.input_config.aac_config.inv_quant = config.inverse_quantization.raw();
        ffi.input_config.aac_config.bitreservoir_size =
            config.bit_reservoir_size.unwrap_or(match config.profile {
                Profile::AacLd | Profile::AacEld => DEFAULT_LD_BITRESERVOIR,
                _ => DEFAULT_LC_BITRESERVOIR,
            });
        ffi.input_config.aac_config.length = 0;
        ffi.input_config.aac_config.use_adts =
            i32::from(config.output_format == OutputFormat::Adts);

        let create_status = unsafe {
            libxaac_sys::ixheaace_create(
                (&mut ffi.input_config as *mut _) as *mut c_void,
                (&mut ffi.output_config as *mut _) as *mut c_void,
            )
        };
        check_encoder(create_status)?;

        let input_size = ffi.output_config.input_size as usize;
        let asc_len = ffi.output_config.i_out_bytes.max(0) as usize;
        let out_ptr = ffi.output_config.mem_info_table[libxaac_sys::IA_MEMTYPE_OUTPUT as usize]
            .mem_ptr as *const u8;
        let audio_specific_config = if asc_len == 0 || out_ptr.is_null() {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(out_ptr, asc_len) }.to_vec()
        };

        Ok(Self {
            config,
            ffi,
            input_size,
            audio_specific_config,
            version,
            _drc_payload: drc_payload,
        })
    }

    pub fn config(&self) -> &EncoderConfig {
        &self.config
    }

    pub fn version(&self) -> &VersionInfo {
        &self.version
    }

    pub fn input_frame_bytes(&self) -> usize {
        self.input_size
    }

    pub fn input_samples_per_channel(&self) -> usize {
        self.ffi.input_config.frame_length as usize
    }

    pub fn expected_frame_count(&self) -> usize {
        self.ffi.output_config.expected_frame_count.max(0) as usize
    }

    pub fn audio_specific_config(&self) -> &[u8] {
        &self.audio_specific_config
    }

    pub fn encode_pcm_bytes(&mut self, pcm: &[u8]) -> Result<EncodedPacket> {
        if pcm.len() != self.input_size {
            return Err(Error::InvalidInput {
                expected: self.input_size,
                actual: pcm.len(),
                context: "encoder frame",
            });
        }
        self.encode_pcm_bytes_inner(pcm)
    }

    pub fn encode_pcm_bytes_with_padding(&mut self, pcm: &[u8]) -> Result<EncodedFrame> {
        if pcm.len() > self.input_size {
            return Err(Error::InvalidInput {
                expected: self.input_size,
                actual: pcm.len(),
                context: "encoder partial frame",
            });
        }

        let mut padded = vec![0u8; self.input_size];
        padded[..pcm.len()].copy_from_slice(pcm);
        let packet = self.encode_pcm_bytes_inner(&padded)?;
        Ok(EncodedFrame {
            packet,
            padded_input_bytes: self.input_size - pcm.len(),
        })
    }

    pub fn encode_i16_interleaved(&mut self, pcm: &[i16]) -> Result<EncodedPacket> {
        if self.config.pcm_word_size != 16 {
            return Err(Error::InvalidConfig(
                "encode_i16_interleaved requires pcm_word_size = 16",
            ));
        }
        let expected_samples = self.input_size / std::mem::size_of::<i16>();
        if pcm.len() != expected_samples {
            return Err(Error::InvalidInput {
                expected: expected_samples,
                actual: pcm.len(),
                context: "encoder i16 frame",
            });
        }
        let bytes = unsafe {
            std::slice::from_raw_parts(pcm.as_ptr().cast::<u8>(), std::mem::size_of_val(pcm))
        };
        self.encode_pcm_bytes_inner(bytes)
    }

    pub fn encode_i24_interleaved(&mut self, pcm: &[[u8; 3]]) -> Result<EncodedPacket> {
        if self.config.pcm_word_size != 24 {
            return Err(Error::InvalidConfig(
                "encode_i24_interleaved requires pcm_word_size = 24",
            ));
        }
        let expected_samples = self.input_size / 3;
        if pcm.len() != expected_samples {
            return Err(Error::InvalidInput {
                expected: expected_samples,
                actual: pcm.len(),
                context: "encoder i24 frame",
            });
        }
        let bytes = unsafe { std::slice::from_raw_parts(pcm.as_ptr().cast::<u8>(), pcm.len() * 3) };
        self.encode_pcm_bytes_inner(bytes)
    }

    pub fn encode_i32_interleaved(&mut self, pcm: &[i32]) -> Result<EncodedPacket> {
        if self.config.pcm_word_size != 32 {
            return Err(Error::InvalidConfig(
                "encode_i32_interleaved requires pcm_word_size = 32",
            ));
        }
        let expected_samples = self.input_size / std::mem::size_of::<i32>();
        if pcm.len() != expected_samples {
            return Err(Error::InvalidInput {
                expected: expected_samples,
                actual: pcm.len(),
                context: "encoder i32 frame",
            });
        }
        let bytes = unsafe {
            std::slice::from_raw_parts(pcm.as_ptr().cast::<u8>(), std::mem::size_of_val(pcm))
        };
        self.encode_pcm_bytes_inner(bytes)
    }

    fn encode_pcm_bytes_inner(&mut self, pcm: &[u8]) -> Result<EncodedPacket> {
        let in_ptr = self.ffi.output_config.mem_info_table[libxaac_sys::IA_MEMTYPE_INPUT as usize]
            .mem_ptr as *mut u8;
        let out_ptr = self.ffi.output_config.mem_info_table[libxaac_sys::IA_MEMTYPE_OUTPUT as usize]
            .mem_ptr as *const u8;
        if in_ptr.is_null() || out_ptr.is_null() {
            return Err(Error::InvalidConfig("encoder buffers were not allocated"));
        }

        unsafe {
            ptr::copy_nonoverlapping(pcm.as_ptr(), in_ptr, pcm.len());
        }

        let status = unsafe {
            libxaac_sys::ixheaace_process(
                self.ffi.output_config.pv_ia_process_api_obj,
                (&mut self.ffi.input_config as *mut _) as *mut c_void,
                (&mut self.ffi.output_config as *mut _) as *mut c_void,
            )
        };
        check_encoder(status)?;

        let len = self.ffi.output_config.i_out_bytes.max(0) as usize;
        let data = unsafe { std::slice::from_raw_parts(out_ptr, len) }.to_vec();
        Ok(EncodedPacket {
            data,
            bytes_consumed: self.ffi.output_config.i_bytes_consumed.max(0) as usize,
        })
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        let _ = unsafe {
            libxaac_sys::ixheaace_delete((&mut self.ffi.output_config as *mut _) as *mut c_void)
        };
    }
}

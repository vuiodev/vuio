use std::ffi::c_void;
use std::ptr;

use crate::error::{Error, Result};
use crate::ffi::{check_decoder, decoder_version};
use crate::util::{AlignedBuffer, VersionInfo};

const IA_DRC_DEC_CONFIG_PARAM_PCM_WDSZ: i32 = 0x0000;
const IA_DRC_DEC_CONFIG_PARAM_SAMP_FREQ: i32 = 0x0001;
const IA_DRC_DEC_CONFIG_PARAM_NUM_CHANNELS: i32 = 0x0002;
const IA_DRC_DEC_CONFIG_PARAM_BITS_FORMAT: i32 = 0x0007;
const IA_DRC_DEC_CONFIG_PARAM_INT_PRESENT: i32 = 0x0008;
const IA_DRC_DEC_CONFIG_PARAM_FRAME_SIZE: i32 = 0x000E;
const IA_DRC_DEC_CONFIG_GAIN_STREAM_FLAG: i32 = 0x0010;
const IA_DRC_DEC_CONFIG_DRC_EFFECT_TYPE: i32 = 0x0011;
const IA_DRC_DEC_CONFIG_DRC_TARGET_LOUDNESS: i32 = 0x0012;
const IA_DRC_DEC_CONFIG_DRC_LOUD_NORM: i32 = 0x0013;
const IA_DRC_DEC_CONFIG_PARAM_APPLY_CROSSFADE: i32 = 0x0017;
const IA_DRC_DEC_CONFIG_PARAM_CONFIG_CHANGED: i32 = 0x0018;
const IA_DRC_DEC_CONFIG_DRC_LOUDNESS_LEVELING: i32 = 0x0019;
const IA_CMD_TYPE_INIT_CPY_BSF_BUFF: i32 = 0x0201;
const IA_CMD_TYPE_INIT_CPY_IC_BSF_BUFF: i32 = 0x0202;
const IA_CMD_TYPE_INIT_CPY_IL_BSF_BUFF: i32 = 0x0203;
const IA_CMD_TYPE_INIT_CPY_IN_BSF_BUFF: i32 = 0x0205;
const IA_XHEAAC_DEC_CONFIG_PARAM_DRC_LOUDNESS_LEVELING: i32 = 0x0029;
const DECODER_MIN_INIT_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoderTransport {
    Auto,
    Raw,
    Mp4Raw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrcEffectType {
    None,
    Night,
    Noisy,
    Limited,
    LowLevel,
    Dialog,
    GeneralCompression,
    Expanded,
    Articulated,
    Headphone,
    PortableSpeaker,
    StereoDownmix,
}

impl DrcEffectType {
    fn raw(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Night => 1,
            Self::Noisy => 2,
            Self::Limited => 3,
            Self::LowLevel => 4,
            Self::Dialog => 5,
            Self::GeneralCompression => 6,
            Self::Expanded => 7,
            Self::Articulated => 8,
            Self::Headphone => 9,
            Self::PortableSpeaker => 10,
            Self::StereoDownmix => 11,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbrMode {
    Unknown,
    Disabled,
    Enabled,
    Downsampled,
    Esbr,
}

impl SbrMode {
    fn frame_samples(self) -> u32 {
        match self {
            Self::Enabled => 2048,
            Self::Esbr => 4096,
            _ => 1024,
        }
    }
}

impl From<i32> for SbrMode {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Disabled,
            1 => Self::Enabled,
            2 => Self::Downsampled,
            3 => Self::Esbr,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelMode {
    Mono,
    Stereo,
    DualMono,
    ParametricStereo,
    Unknown(i32),
}

impl From<i32> for ChannelMode {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Mono,
            1 => Self::Stereo,
            2 => Self::DualMono,
            3 => Self::ParametricStereo,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawStreamConfig {
    pub sample_rate: u32,
    pub audio_object_type: ProfileHint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileHint {
    AacLc,
    HeAacV1,
    AacLd,
    HeAacV2,
    AacEld,
    Usac,
}

impl ProfileHint {
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecoderDrcConfig {
    pub enabled: bool,
    pub cut: f32,
    pub boost: f32,
    pub target_level: u8,
    pub heavy_compression: bool,
    pub effect_type: DrcEffectType,
    pub target_loudness: Option<i32>,
    pub loudness_leveling: Option<bool>,
}

impl Default for DecoderDrcConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cut: 1.0,
            boost: 1.0,
            target_level: 108,
            heavy_compression: false,
            effect_type: DrcEffectType::None,
            target_loudness: None,
            loudness_leveling: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecoderConfig {
    pub transport: DecoderTransport,
    pub pcm_word_size: u16,
    pub raw: Option<RawStreamConfig>,
    pub downmix_to_mono: bool,
    pub to_stereo: bool,
    pub downsample_sbr: bool,
    pub max_channels: u8,
    pub coupling_channels: u8,
    pub downmix_to_stereo: bool,
    pub disable_sync: bool,
    pub auto_sbr_upsample: bool,
    pub ld_frame_480: bool,
    pub hq_esbr: bool,
    pub ps_enable: bool,
    pub peak_limiter: bool,
    pub frame_length_960: bool,
    pub error_concealment: bool,
    pub enable_esbr: bool,
    pub drc: DecoderDrcConfig,
}

impl Default for DecoderConfig {
    fn default() -> Self {
        Self {
            transport: DecoderTransport::Auto,
            pcm_word_size: 16,
            raw: None,
            downmix_to_mono: false,
            to_stereo: false,
            downsample_sbr: false,
            max_channels: 2,
            coupling_channels: 0,
            downmix_to_stereo: false,
            disable_sync: false,
            auto_sbr_upsample: true,
            ld_frame_480: false,
            hq_esbr: false,
            ps_enable: false,
            peak_limiter: true,
            frame_length_960: false,
            error_concealment: true,
            enable_esbr: false,
            drc: DecoderDrcConfig::default(),
        }
    }
}

impl DecoderConfig {
    fn validate(&self) -> Result<()> {
        if !matches!(self.pcm_word_size, 16 | 24) {
            return Err(Error::InvalidConfig("pcm_word_size must be 16 or 24"));
        }
        if self.max_channels == 0 {
            return Err(Error::InvalidConfig(
                "max_channels must be greater than zero",
            ));
        }
        if matches!(
            self.transport,
            DecoderTransport::Raw | DecoderTransport::Mp4Raw
        ) && self.raw.is_none()
        {
            return Err(Error::InvalidConfig(
                "raw transport requires RawStreamConfig",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub channel_mask: u32,
    pub channel_mode: Option<ChannelMode>,
    pub pcm_word_size: u16,
    pub sbr_mode: SbrMode,
    pub audio_object_type: i32,
    pub drc_effect_type: Option<DrcEffectType>,
    pub drc_target_loudness: Option<i32>,
    pub drc_loudness_norm: Option<i32>,
    pub loudness_leveling: Option<bool>,
    pub preroll_frames: Option<u32>,
    pub drc_config_changed: Option<bool>,
    pub drc_apply_crossfade: Option<bool>,
    pub extension_elements: Option<u32>,
    pub config_extensions: Option<u32>,
    pub gain_payload_len: Option<u32>,
    pub drc_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeProgress {
    pub initialized: bool,
    pub stream_info: Option<StreamInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeStatus {
    Frame(DecodedFrame),
    NeedMoreInput(DecodeProgress),
    EndOfStream,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    pub pcm: Vec<u8>,
    pub bytes_consumed: usize,
    pub stream_info: StreamInfo,
}

impl DecodedFrame {
    pub fn pcm_i16(&self) -> Option<Vec<i16>> {
        if self.stream_info.pcm_word_size != 16 || self.pcm.len() % 2 != 0 {
            return None;
        }

        Some(
            self.pcm
                .chunks_exact(2)
                .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                .collect(),
        )
    }

    pub fn pcm_i24(&self) -> Option<Vec<i32>> {
        if self.stream_info.pcm_word_size != 24 || self.pcm.len() % 3 != 0 {
            return None;
        }

        Some(
            self.pcm
                .chunks_exact(3)
                .map(|chunk| {
                    let value = i32::from(chunk[0])
                        | (i32::from(chunk[1]) << 8)
                        | (i32::from(chunk[2]) << 16);
                    if (value & 0x0080_0000) != 0 {
                        value | !0x00ff_ffff
                    } else {
                        value
                    }
                })
                .collect(),
        )
    }
}

#[derive(Debug)]
struct DrcDecoder {
    api: AlignedBuffer,
    memtabs: AlignedBuffer,
    memblocks: Vec<AlignedBuffer>,
    input_index: usize,
    output_index: usize,
}

impl DrcDecoder {
    fn new(stream_info: &StreamInfo) -> Result<Self> {
        let mut api_size = 0u32;
        check_decoder(unsafe {
            libxaac_sys::ia_drc_dec_api(
                ptr::null_mut(),
                libxaac_sys::IA_API_CMD_GET_API_SIZE as i32,
                0,
                (&mut api_size as *mut _) as *mut c_void,
            )
        })?;

        let api = AlignedBuffer::new(api_size as usize, 8)?;
        check_decoder(unsafe {
            libxaac_sys::ia_drc_dec_api(
                api.as_ptr().cast::<c_void>(),
                libxaac_sys::IA_API_CMD_INIT as i32,
                libxaac_sys::IA_CMD_TYPE_INIT_API_PRE_CONFIG_PARAMS as i32,
                ptr::null_mut(),
            )
        })?;

        let mut drc = Self {
            api,
            memtabs: AlignedBuffer::new(4, 4)?,
            memblocks: Vec::new(),
            input_index: 2,
            output_index: 3,
        };
        drc.set_config(
            IA_DRC_DEC_CONFIG_PARAM_SAMP_FREQ,
            stream_info.sample_rate as i32,
        )?;
        drc.set_config(
            IA_DRC_DEC_CONFIG_PARAM_NUM_CHANNELS,
            i32::from(stream_info.channels),
        )?;
        drc.set_config(
            IA_DRC_DEC_CONFIG_PARAM_PCM_WDSZ,
            i32::from(stream_info.pcm_word_size),
        )?;
        drc.set_config(
            IA_DRC_DEC_CONFIG_PARAM_FRAME_SIZE,
            stream_info.sbr_mode.frame_samples() as i32,
        )?;
        drc.allocate_memory()?;
        Ok(drc)
    }

    fn allocate_memory(&mut self) -> Result<()> {
        let mut memtabs_size = 0u32;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_GET_MEMTABS_SIZE as i32,
            0,
            (&mut memtabs_size as *mut _) as *mut c_void,
        ))?;
        self.memtabs = AlignedBuffer::new(memtabs_size as usize, 8)?;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_SET_MEMTABS_PTR as i32,
            0,
            self.memtabs.as_ptr().cast::<c_void>(),
        ))?;
        check_decoder(self.call(
            libxaac_sys::IA_API_CMD_INIT as i32,
            libxaac_sys::IA_CMD_TYPE_INIT_API_POST_CONFIG_PARAMS as i32,
            ptr::null_mut(),
        ))?;

        let mut mem_count = 0i32;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_GET_N_MEMTABS as i32,
            0,
            (&mut mem_count as *mut _) as *mut c_void,
        ))?;

        for index in 0..mem_count {
            let mut size = 0u32;
            let mut alignment = 0u32;
            check_decoder(self.call_with_value(
                libxaac_sys::IA_API_CMD_GET_MEM_INFO_SIZE as i32,
                index,
                (&mut size as *mut _) as *mut c_void,
            ))?;
            check_decoder(self.call_with_value(
                libxaac_sys::IA_API_CMD_GET_MEM_INFO_ALIGNMENT as i32,
                index,
                (&mut alignment as *mut _) as *mut c_void,
            ))?;
            let block = AlignedBuffer::new(size as usize, alignment.max(1) as usize)?;
            check_decoder(self.call_with_value(
                libxaac_sys::IA_API_CMD_SET_MEM_PTR as i32,
                index,
                block.as_ptr().cast::<c_void>(),
            ))?;
            self.memblocks.push(block);
        }

        Ok(())
    }

    fn sync_stream_info(&mut self, info: &StreamInfo) -> Result<()> {
        self.set_config(
            IA_DRC_DEC_CONFIG_PARAM_FRAME_SIZE,
            info.sbr_mode.frame_samples() as i32,
        )?;
        self.set_config(IA_DRC_DEC_CONFIG_PARAM_SAMP_FREQ, info.sample_rate as i32)?;
        self.set_config(
            IA_DRC_DEC_CONFIG_PARAM_NUM_CHANNELS,
            i32::from(info.channels),
        )?;
        self.set_config(
            IA_DRC_DEC_CONFIG_PARAM_PCM_WDSZ,
            i32::from(info.pcm_word_size),
        )?;
        if let Some(loudness) = info.drc_target_loudness {
            self.set_config(IA_DRC_DEC_CONFIG_DRC_TARGET_LOUDNESS, loudness)?;
        }
        if let Some(loud_norm) = info.drc_loudness_norm {
            self.set_config(IA_DRC_DEC_CONFIG_DRC_LOUD_NORM, loud_norm)?;
        }
        if let Some(effect) = info.drc_effect_type {
            self.set_config(IA_DRC_DEC_CONFIG_DRC_EFFECT_TYPE, effect.raw())?;
        }
        if let Some(leveling) = info.loudness_leveling {
            self.set_config(IA_DRC_DEC_CONFIG_DRC_LOUDNESS_LEVELING, i32::from(leveling))?;
        }
        if let Some(changed) = info.drc_config_changed {
            self.set_config(IA_DRC_DEC_CONFIG_PARAM_CONFIG_CHANGED, i32::from(changed))?;
        }
        if let Some(crossfade) = info.drc_apply_crossfade {
            self.set_config(
                IA_DRC_DEC_CONFIG_PARAM_APPLY_CROSSFADE,
                i32::from(crossfade),
            )?;
        }
        Ok(())
    }

    fn copy_split_payload(
        &mut self,
        input_len_cmd: i32,
        init_cmd: i32,
        payload: &[u8],
        bits_format: i32,
    ) -> Result<()> {
        if payload.is_empty() {
            return Ok(());
        }
        self.set_config(IA_DRC_DEC_CONFIG_PARAM_BITS_FORMAT, bits_format)?;
        self.copy_to_input(payload)?;
        let mut size = payload.len() as i32;
        check_decoder(self.call_with_value(
            input_len_cmd,
            0,
            (&mut size as *mut _) as *mut c_void,
        ))?;
        check_decoder(self.call(
            libxaac_sys::IA_API_CMD_INIT as i32,
            init_cmd,
            ptr::null_mut(),
        ))
    }

    fn copy_to_input(&mut self, data: &[u8]) -> Result<()> {
        let input = self
            .memblocks
            .get(self.input_index)
            .ok_or(Error::InvalidConfig("drc input buffer missing"))?
            .as_ptr();
        let capacity = self.input_capacity();
        if data.len() > capacity {
            return Err(Error::InputTooLarge {
                capacity,
                actual: data.len(),
            });
        }
        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr(), input, data.len());
            if data.len() < capacity {
                ptr::write_bytes(input.add(data.len()), 0, capacity - data.len());
            }
        }
        Ok(())
    }

    fn input_capacity(&self) -> usize {
        self.memblocks
            .get(self.input_index)
            .map(|_| {
                self.query_mem_size(self.input_index as i32)
                    .unwrap_or_default() as usize
            })
            .unwrap_or_default()
    }

    fn output_capacity(&self) -> usize {
        self.memblocks
            .get(self.output_index)
            .map(|_| {
                self.query_mem_size(self.output_index as i32)
                    .unwrap_or_default() as usize
            })
            .unwrap_or_default()
    }

    fn process_pcm(
        &mut self,
        pcm: &[u8],
        info: &StreamInfo,
        gain_payload: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        self.sync_stream_info(info)?;
        self.copy_to_input(pcm)?;
        if let Some(gain_payload) = gain_payload.filter(|payload| !payload.is_empty()) {
            self.set_config(IA_DRC_DEC_CONFIG_PARAM_BITS_FORMAT, 1)?;
            let gain_stream_flag = 1i32;
            self.call_with_value(
                libxaac_sys::IA_API_CMD_SET_INPUT_BYTES as i32,
                0,
                (&mut (pcm.len() as i32) as *mut _) as *mut c_void,
            );
            self.copy_to_input(gain_payload)?;
            let mut gain_len = gain_payload.len() as i32;
            check_decoder(self.call_with_value(
                0x000B,
                0,
                (&mut gain_len as *mut _) as *mut c_void,
            ))?;
            self.set_config(IA_DRC_DEC_CONFIG_GAIN_STREAM_FLAG, gain_stream_flag)?;
            check_decoder(self.call(
                libxaac_sys::IA_API_CMD_INIT as i32,
                IA_CMD_TYPE_INIT_CPY_BSF_BUFF,
                ptr::null_mut(),
            ))?;
            self.copy_to_input(pcm)?;
        }

        let mut pcm_len = pcm.len() as i32;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_SET_INPUT_BYTES as i32,
            0,
            (&mut pcm_len as *mut _) as *mut c_void,
        ))?;
        check_decoder(self.call(
            libxaac_sys::IA_API_CMD_EXECUTE as i32,
            libxaac_sys::IA_CMD_TYPE_DO_EXECUTE as i32,
            ptr::null_mut(),
        ))?;
        let out = self
            .memblocks
            .get(self.output_index)
            .ok_or(Error::InvalidConfig("drc output buffer missing"))?
            .as_ptr();
        let len = pcm.len().min(self.output_capacity());
        Ok(unsafe { std::slice::from_raw_parts(out, len) }.to_vec())
    }

    fn query_mem_size(&self, index: i32) -> Result<u32> {
        let mut size = 0u32;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_GET_MEM_INFO_SIZE as i32,
            index,
            (&mut size as *mut _) as *mut c_void,
        ))?;
        Ok(size)
    }

    fn set_config(&mut self, index: i32, value: i32) -> Result<()> {
        let mut value = value;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_SET_CONFIG_PARAM as i32,
            index,
            (&mut value as *mut _) as *mut c_void,
        ))
    }

    fn call(&self, cmd: i32, idx: i32, value: *mut c_void) -> i32 {
        unsafe { libxaac_sys::ia_drc_dec_api(self.api.as_ptr().cast::<c_void>(), cmd, idx, value) }
    }

    fn call_with_value(&self, cmd: i32, idx: i32, value: *mut c_void) -> i32 {
        self.call(cmd, idx, value)
    }
}

#[derive(Debug)]
pub struct Decoder {
    config: DecoderConfig,
    version: VersionInfo,
    api: AlignedBuffer,
    memtabs: AlignedBuffer,
    memblocks: Vec<AlignedBuffer>,
    input_index: usize,
    output_index: usize,
    input_capacity: usize,
    initialized: bool,
    input_over_signaled: bool,
    finished: bool,
    pending_input: Vec<u8>,
    cached_info: Option<StreamInfo>,
    drc_decoder: Option<DrcDecoder>,
}

impl Decoder {
    pub fn new(config: DecoderConfig) -> Result<Self> {
        config.validate()?;

        let version = decoder_version();
        let mut api_size = 0u32;
        check_decoder(unsafe {
            libxaac_sys::ixheaacd_dec_api(
                ptr::null_mut(),
                libxaac_sys::IA_API_CMD_GET_API_SIZE as i32,
                0,
                (&mut api_size as *mut _) as *mut c_void,
            )
        })?;

        let api = AlignedBuffer::new(api_size as usize, 4)?;
        check_decoder(unsafe {
            libxaac_sys::ixheaacd_dec_api(
                api.as_ptr().cast::<c_void>(),
                libxaac_sys::IA_API_CMD_INIT as i32,
                libxaac_sys::IA_CMD_TYPE_INIT_API_PRE_CONFIG_PARAMS as i32,
                ptr::null_mut(),
            )
        })?;

        let mut decoder = Self {
            config,
            version,
            api,
            memtabs: AlignedBuffer::new(4, 4)?,
            memblocks: Vec::new(),
            input_index: 0,
            output_index: 0,
            input_capacity: 0,
            initialized: false,
            input_over_signaled: false,
            finished: false,
            pending_input: Vec::new(),
            cached_info: None,
            drc_decoder: None,
        };
        decoder.apply_config()?;
        decoder.allocate_memory()?;
        Ok(decoder)
    }

    pub fn config(&self) -> &DecoderConfig {
        &self.config
    }

    pub fn version(&self) -> &VersionInfo {
        &self.version
    }

    pub fn input_capacity(&self) -> usize {
        self.input_capacity
    }

    pub fn stream_info(&self) -> Option<&StreamInfo> {
        self.cached_info.as_ref()
    }

    pub fn probe_stream_info(&mut self, data: &[u8]) -> Result<StreamInfo> {
        if self.cached_info.is_some() {
            return Ok(self.cached_info.clone().expect("cached stream info"));
        }
        self.pending_input.extend_from_slice(data);
        loop {
            match self.drive_initialization(data.is_empty())? {
                InitState::Ready => return self.refresh_stream_info(),
                InitState::NeedMoreInput => {
                    self.ensure_input_over()?;
                }
                InitState::Continue => {}
                InitState::EndOfStream => {
                    return Err(Error::DecoderError(libxaac_sys::IA_FATAL_ERROR as i32));
                }
            }
        }
    }

    pub fn decode(&mut self, input: &[u8]) -> Result<DecodedFrame> {
        match self.decode_stream_chunk(input)? {
            DecodeStatus::Frame(frame) => Ok(frame),
            DecodeStatus::NeedMoreInput(_) => Err(Error::NeedMoreInput),
            DecodeStatus::EndOfStream => Err(Error::EndOfStream),
        }
    }

    pub fn decode_stream_chunk(&mut self, input: &[u8]) -> Result<DecodeStatus> {
        if self.finished {
            return Ok(DecodeStatus::EndOfStream);
        }
        self.pending_input.extend_from_slice(input);
        self.drive(false)
    }

    pub fn finish(&mut self) -> Result<DecodeStatus> {
        if self.finished {
            return Ok(DecodeStatus::EndOfStream);
        }
        self.drive(true)
    }

    pub fn signal_end_of_input(&mut self) -> Result<()> {
        self.ensure_input_over()
    }

    fn drive(&mut self, finalize: bool) -> Result<DecodeStatus> {
        loop {
            if !self.initialized {
                match self.drive_initialization(finalize)? {
                    InitState::Ready => {}
                    InitState::NeedMoreInput => {
                        return Ok(DecodeStatus::NeedMoreInput(self.progress()));
                    }
                    InitState::Continue => continue,
                    InitState::EndOfStream => {
                        self.finished = true;
                        return Ok(DecodeStatus::EndOfStream);
                    }
                }
            }

            match self.execute_once(finalize)? {
                ExecuteState::Frame(frame) => return Ok(DecodeStatus::Frame(frame)),
                ExecuteState::NeedMoreInput => {
                    return Ok(DecodeStatus::NeedMoreInput(self.progress()));
                }
                ExecuteState::Continue => continue,
                ExecuteState::EndOfStream => {
                    self.finished = true;
                    return Ok(DecodeStatus::EndOfStream);
                }
            }
        }
    }

    fn drive_initialization(&mut self, finalize: bool) -> Result<InitState> {
        if !finalize && self.pending_input.len() < DECODER_MIN_INIT_BYTES {
            return Ok(InitState::NeedMoreInput);
        }
        if self.pending_input.is_empty() && finalize {
            self.ensure_input_over()?;
        }
        if self.pending_input.is_empty() && self.input_over_signaled {
            return Ok(InitState::EndOfStream);
        }

        let bytes = self.load_input_window();
        if bytes == 0 {
            return Ok(InitState::NeedMoreInput);
        }

        let mut bytes_i32 = bytes as i32;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_SET_INPUT_BYTES as i32,
            0,
            (&mut bytes_i32 as *mut _) as *mut c_void,
        ))?;
        let status = self.call(
            libxaac_sys::IA_API_CMD_INIT as i32,
            libxaac_sys::IA_CMD_TYPE_INIT_PROCESS as i32,
            ptr::null_mut(),
        );
        if is_fatal(status) {
            return Err(Error::DecoderError(status));
        }

        let consumed = self.query_consumed()? as usize;
        self.consume_pending(consumed)?;

        let mut init_done = 0i32;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_INIT as i32,
            libxaac_sys::IA_CMD_TYPE_INIT_DONE_QUERY as i32,
            (&mut init_done as *mut _) as *mut c_void,
        ))?;

        if init_done != 0 {
            self.initialized = true;
            let info = self.refresh_stream_info()?;
            self.try_initialize_drc(&info)?;
            return Ok(InitState::Ready);
        }

        if !finalize && consumed == 0 && bytes < self.input_capacity {
            return Ok(InitState::NeedMoreInput);
        }

        if self.pending_input.is_empty() && finalize {
            self.ensure_input_over()?;
            if self.pending_input.is_empty() && self.input_over_signaled {
                return Ok(InitState::NeedMoreInput);
            }
        }

        if self.pending_input.is_empty() {
            Ok(InitState::NeedMoreInput)
        } else {
            Ok(InitState::Continue)
        }
    }

    fn execute_once(&mut self, finalize: bool) -> Result<ExecuteState> {
        if self.pending_input.is_empty() && finalize {
            self.ensure_input_over()?;
        }
        if self.pending_input.is_empty() && self.input_over_signaled {
            return Ok(ExecuteState::EndOfStream);
        }

        let bytes = self.load_input_window();
        let mut bytes_i32 = bytes as i32;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_SET_INPUT_BYTES as i32,
            0,
            (&mut bytes_i32 as *mut _) as *mut c_void,
        ))?;
        let status = self.call(
            libxaac_sys::IA_API_CMD_EXECUTE as i32,
            libxaac_sys::IA_CMD_TYPE_DO_EXECUTE as i32,
            ptr::null_mut(),
        );
        if is_fatal(status) {
            return Err(Error::DecoderError(status));
        }

        let consumed = self.query_consumed()? as usize;
        self.consume_pending(consumed)?;

        let mut done = 0i32;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_EXECUTE as i32,
            libxaac_sys::IA_CMD_TYPE_DONE_QUERY as i32,
            (&mut done as *mut _) as *mut c_void,
        ))?;

        let mut out_bytes = 0i32;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_GET_OUTPUT_BYTES as i32,
            0,
            (&mut out_bytes as *mut _) as *mut c_void,
        ))?;

        let info = self.refresh_stream_info()?;
        let mut pcm = self.read_output_bytes(out_bytes.max(0) as usize)?;
        if !pcm.is_empty() {
            let gain_payload = if info.drc_active {
                self.read_optional_buffer(
                    libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_GAIN_PAYLOAD_BUF as i32,
                    libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_GAIN_PAYLOAD_LEN as i32,
                )?
            } else {
                None
            };
            if let Some(drc) = self.drc_decoder.as_mut().filter(|_| info.drc_active) {
                pcm = drc.process_pcm(&pcm, &info, gain_payload.as_deref())?;
            }
            return Ok(ExecuteState::Frame(DecodedFrame {
                pcm,
                bytes_consumed: consumed,
                stream_info: info,
            }));
        }

        if !finalize && consumed == 0 && bytes < self.input_capacity && done == 0 {
            return Ok(ExecuteState::NeedMoreInput);
        }

        if done != 0 && self.pending_input.is_empty() && self.input_over_signaled {
            return Ok(ExecuteState::EndOfStream);
        }
        if self.pending_input.is_empty() && !finalize && !self.input_over_signaled {
            return Ok(ExecuteState::NeedMoreInput);
        }
        if self.pending_input.is_empty() && finalize && self.input_over_signaled {
            return Ok(ExecuteState::EndOfStream);
        }
        Ok(ExecuteState::Continue)
    }

    fn apply_config(&mut self) -> Result<()> {
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_PCM_WDSZ as i32,
            self.config.pcm_word_size as i32,
        )?;
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_DOWNMIX as i32,
            i32::from(self.config.downmix_to_mono),
        )?;
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_TOSTEREO as i32,
            i32::from(self.config.to_stereo),
        )?;
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_DSAMPLE as i32,
            i32::from(self.config.downsample_sbr),
        )?;
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_ISMP4 as i32,
            i32::from(matches!(self.config.transport, DecoderTransport::Mp4Raw)),
        )?;
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_MAX_CHANNEL as i32,
            i32::from(self.config.max_channels),
        )?;
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_COUP_CHANNEL as i32,
            i32::from(self.config.coupling_channels),
        )?;
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_DOWNMIX_STEREO as i32,
            i32::from(self.config.downmix_to_stereo),
        )?;
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_DISABLE_SYNC as i32,
            i32::from(
                self.config.disable_sync
                    || matches!(
                        self.config.transport,
                        DecoderTransport::Raw | DecoderTransport::Mp4Raw
                    ),
            ),
        )?;
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_AUTO_SBR_UPSAMPLE as i32,
            i32::from(self.config.auto_sbr_upsample),
        )?;
        self.set_config_f32(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_CUT as i32,
            self.config.drc.cut,
        )?;
        self.set_config_f32(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_BOOST as i32,
            self.config.drc.boost,
        )?;
        self.set_config(
            libxaac_sys::IA_XHEAAC_DEC_CONFIG_PARAM_DRC_TARGET_LEVEL as i32,
            i32::from(self.config.drc.target_level),
        )?;
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_HEAVY_COMP as i32,
            i32::from(self.config.drc.heavy_compression),
        )?;
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_EFFECT_TYPE as i32,
            self.config.drc.effect_type.raw(),
        )?;
        if let Some(target_loudness) = self.config.drc.target_loudness {
            self.set_config(
                libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_TARGET_LOUDNESS as i32,
                target_loudness,
            )?;
        }
        if let Some(leveling) = self.config.drc.loudness_leveling {
            let _ = self.set_config_optional(
                IA_XHEAAC_DEC_CONFIG_PARAM_DRC_LOUDNESS_LEVELING,
                i32::from(leveling),
            );
        }
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_FRAMESIZE as i32,
            i32::from(self.config.ld_frame_480),
        )?;
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_HQ_ESBR as i32,
            i32::from(self.config.hq_esbr),
        )?;
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_PS_ENABLE as i32,
            i32::from(self.config.ps_enable),
        )?;
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_PEAK_LIMITER as i32,
            i32::from(!self.config.peak_limiter),
        )?;
        self.set_config(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_FRAMELENGTH_FLAG as i32,
            i32::from(self.config.frame_length_960),
        )?;
        self.set_config(
            libxaac_sys::IA_XHEAAC_DEC_CONFIG_ERROR_CONCEALMENT as i32,
            i32::from(self.config.error_concealment),
        )?;
        self.set_config(
            libxaac_sys::IA_XHEAAC_DEC_CONFIG_PARAM_ESBR as i32,
            i32::from(self.config.enable_esbr),
        )?;

        if let Some(raw) = self.config.raw {
            self.set_config(
                libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_SAMP_FREQ as i32,
                raw.sample_rate as i32,
            )?;
            self.set_config(
                libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_AOT as i32,
                raw.audio_object_type.aot(),
            )?;
        }
        Ok(())
    }

    fn allocate_memory(&mut self) -> Result<()> {
        let mut memtabs_size = 0u32;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_GET_MEMTABS_SIZE as i32,
            0,
            (&mut memtabs_size as *mut _) as *mut c_void,
        ))?;
        self.memtabs = AlignedBuffer::new(memtabs_size as usize, 4)?;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_SET_MEMTABS_PTR as i32,
            0,
            self.memtabs.as_ptr().cast::<c_void>(),
        ))?;

        check_decoder(self.call(
            libxaac_sys::IA_API_CMD_INIT as i32,
            libxaac_sys::IA_CMD_TYPE_INIT_API_POST_CONFIG_PARAMS as i32,
            ptr::null_mut(),
        ))?;

        let mut mem_count = 0i32;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_GET_N_MEMTABS as i32,
            0,
            (&mut mem_count as *mut _) as *mut c_void,
        ))?;

        for index in 0..mem_count {
            let mut size = 0u32;
            let mut alignment = 0u32;
            let mut mem_type = 0u32;

            check_decoder(self.call_with_value(
                libxaac_sys::IA_API_CMD_GET_MEM_INFO_SIZE as i32,
                index,
                (&mut size as *mut _) as *mut c_void,
            ))?;
            check_decoder(self.call_with_value(
                libxaac_sys::IA_API_CMD_GET_MEM_INFO_ALIGNMENT as i32,
                index,
                (&mut alignment as *mut _) as *mut c_void,
            ))?;
            check_decoder(self.call_with_value(
                libxaac_sys::IA_API_CMD_GET_MEM_INFO_TYPE as i32,
                index,
                (&mut mem_type as *mut _) as *mut c_void,
            ))?;

            let block = AlignedBuffer::new(size as usize, alignment.max(1) as usize)?;
            let ptr = block.as_ptr().cast::<c_void>();
            check_decoder(self.call_with_value(
                libxaac_sys::IA_API_CMD_SET_MEM_PTR as i32,
                index,
                ptr,
            ))?;

            if mem_type == libxaac_sys::IA_MEMTYPE_INPUT {
                self.input_index = self.memblocks.len();
                self.input_capacity = size as usize;
            } else if mem_type == libxaac_sys::IA_MEMTYPE_OUTPUT {
                self.output_index = self.memblocks.len();
            }

            self.memblocks.push(block);
        }

        Ok(())
    }

    fn try_initialize_drc(&mut self, info: &StreamInfo) -> Result<()> {
        if !self.config.drc.enabled || self.drc_decoder.is_some() {
            return Ok(());
        }
        if info.extension_elements.unwrap_or(0) == 0
            && info.config_extensions.unwrap_or(0) == 0
            && info.gain_payload_len.unwrap_or(0) == 0
        {
            return Ok(());
        }

        let mut drc = DrcDecoder::new(info)?;
        if let Some(payloads) = self.read_extension_payloads()? {
            for payload in &payloads.loudness_payloads {
                drc.copy_split_payload(0x000D, IA_CMD_TYPE_INIT_CPY_IL_BSF_BUFF, payload, 1)?;
            }
            for payload in &payloads.config_payloads {
                drc.copy_split_payload(0x000C, IA_CMD_TYPE_INIT_CPY_IC_BSF_BUFF, payload, 1)?;
            }
            if !payloads.has_any() {
                self.drc_decoder = Some(drc);
                return Ok(());
            }
            let interface_present = 1i32;
            drc.set_config(IA_DRC_DEC_CONFIG_PARAM_INT_PRESENT, interface_present)?;
            check_decoder(drc.call(
                libxaac_sys::IA_API_CMD_INIT as i32,
                IA_CMD_TYPE_INIT_CPY_IN_BSF_BUFF,
                ptr::null_mut(),
            ))?;
            check_decoder(drc.call(
                libxaac_sys::IA_API_CMD_INIT as i32,
                libxaac_sys::IA_CMD_TYPE_INIT_PROCESS as i32,
                ptr::null_mut(),
            ))?;
        }
        self.drc_decoder = Some(drc);
        if let Some(cached) = self.cached_info.as_mut() {
            cached.drc_active = true;
        }
        Ok(())
    }

    fn refresh_stream_info(&mut self) -> Result<StreamInfo> {
        let sample_rate =
            self.get_config_required(libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_SAMP_FREQ as i32)?;
        let channels = self
            .get_config_required(libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_NUM_CHANNELS as i32)?;
        let channel_mask = self
            .get_config_required(libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_CHANNEL_MASK as i32)?;
        let sbr_mode =
            self.get_config_required(libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_SBR_MODE as i32)?;
        let aot =
            self.get_config_required(libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_AOT as i32)?;

        let info = StreamInfo {
            sample_rate: sample_rate.max(0) as u32,
            channels: channels.max(0) as u16,
            channel_mask: channel_mask.max(0) as u32,
            channel_mode: self
                .get_config_optional(
                    libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_CHANNEL_MODE as i32,
                )?
                .map(ChannelMode::from),
            pcm_word_size: self.config.pcm_word_size,
            sbr_mode: SbrMode::from(sbr_mode),
            audio_object_type: aot,
            drc_effect_type: self
                .get_config_optional(
                    libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_EFFECT_TYPE as i32,
                )?
                .map(DrcEffectType::from_raw),
            drc_target_loudness: self.get_config_optional(
                libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_TARGET_LOUDNESS as i32,
            )?,
            drc_loudness_norm: self.get_config_optional(
                libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_PARAM_DRC_LOUD_NORM as i32,
            )?,
            loudness_leveling: self
                .get_config_optional(IA_XHEAAC_DEC_CONFIG_PARAM_DRC_LOUDNESS_LEVELING)?
                .map(|value| value != 0),
            preroll_frames: self
                .get_config_optional(
                    libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_GET_NUM_PRE_ROLL_FRAMES as i32,
                )?
                .map(|value| value.max(0) as u32),
            drc_config_changed: self
                .get_config_optional(libxaac_sys::IA_ENHAACPLUS_DEC_DRC_IS_CONFIG_CHANGED as i32)?
                .map(|value| value != 0),
            drc_apply_crossfade: self
                .get_config_optional(libxaac_sys::IA_ENHAACPLUS_DEC_DRC_APPLY_CROSSFADE as i32)?
                .map(|value| value != 0),
            extension_elements: self
                .get_config_optional(libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_NUM_ELE as i32)?
                .map(|value| value.max(0) as u32),
            config_extensions: self
                .get_config_optional(libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_NUM_CONFIG_EXT as i32)?
                .map(|value| value.max(0) as u32),
            gain_payload_len: self
                .get_config_optional(libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_GAIN_PAYLOAD_LEN as i32)?
                .map(|value| value.max(0) as u32),
            drc_active: self.drc_decoder.is_some(),
        };
        self.cached_info = Some(info.clone());
        Ok(info)
    }

    fn read_extension_payloads(&self) -> Result<Option<ExtensionPayloads>> {
        let num_elements = self
            .get_config_optional(libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_NUM_ELE as i32)?
            .unwrap_or_default()
            .max(0) as usize;
        let num_config_ext = self
            .get_config_optional(libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_NUM_CONFIG_EXT as i32)?
            .unwrap_or_default()
            .max(0) as usize;
        if num_elements == 0 && num_config_ext == 0 {
            return Ok(None);
        }

        let mut sizes = [[0i32; 16]; 2];
        check_decoder(self.get_config_raw(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_EXT_ELE_BUF_SIZES as i32,
            sizes.as_mut_ptr().cast::<c_void>(),
        ))?;
        let mut ptrs = [[ptr::null_mut::<u8>(); 16]; 2];
        check_decoder(self.get_config_raw(
            libxaac_sys::IA_ENHAACPLUS_DEC_CONFIG_EXT_ELE_PTR as i32,
            ptrs.as_mut_ptr().cast::<c_void>(),
        ))?;

        let mut payloads = ExtensionPayloads::default();
        for index in 0..num_config_ext.min(16) {
            let size = sizes[0][index].max(0) as usize;
            if size == 0 || ptrs[0][index].is_null() {
                continue;
            }
            payloads.loudness_payloads.push(unsafe {
                std::slice::from_raw_parts(ptrs[0][index].cast::<u8>(), size).to_vec()
            });
        }
        for index in 0..num_elements.min(16) {
            let size = sizes[1][index].max(0) as usize;
            if size == 0 || ptrs[1][index].is_null() {
                continue;
            }
            payloads.config_payloads.push(unsafe {
                std::slice::from_raw_parts(ptrs[1][index].cast::<u8>(), size).to_vec()
            });
        }
        Ok(Some(payloads))
    }

    fn read_optional_buffer(&self, ptr_index: i32, len_index: i32) -> Result<Option<Vec<u8>>> {
        let len = match self.get_config_optional(len_index)? {
            Some(len) if len > 0 => len as usize,
            _ => return Ok(None),
        };
        let mut ptr_value: *mut u8 = ptr::null_mut();
        check_decoder(self.get_config_raw(ptr_index, (&mut ptr_value as *mut _) as *mut c_void))?;
        if ptr_value.is_null() {
            return Ok(None);
        }
        Ok(Some(unsafe {
            std::slice::from_raw_parts(ptr_value.cast::<u8>(), len).to_vec()
        }))
    }

    fn progress(&self) -> DecodeProgress {
        DecodeProgress {
            initialized: self.initialized,
            stream_info: self.cached_info.clone(),
        }
    }

    fn load_input_window(&mut self) -> usize {
        let len = self.pending_input.len().min(self.input_capacity);
        let input_ptr = self.memblocks[self.input_index].as_ptr();
        unsafe {
            if len != 0 {
                ptr::copy_nonoverlapping(self.pending_input.as_ptr(), input_ptr, len);
            }
            if len < self.input_capacity {
                ptr::write_bytes(input_ptr.add(len), 0, self.input_capacity - len);
            }
        }
        len
    }

    fn read_output_bytes(&self, len: usize) -> Result<Vec<u8>> {
        let out_ptr = self.memblocks[self.output_index].as_ptr();
        if out_ptr.is_null() {
            return Err(Error::InvalidConfig("decoder output buffer missing"));
        }
        Ok(unsafe { std::slice::from_raw_parts(out_ptr, len) }.to_vec())
    }

    fn consume_pending(&mut self, consumed: usize) -> Result<()> {
        if consumed > self.pending_input.len() {
            return Err(Error::DecoderError(libxaac_sys::IA_FATAL_ERROR as i32));
        }
        self.pending_input.drain(..consumed);
        Ok(())
    }

    fn ensure_input_over(&mut self) -> Result<()> {
        if self.input_over_signaled {
            return Ok(());
        }
        check_decoder(self.call(
            libxaac_sys::IA_API_CMD_INPUT_OVER as i32,
            0,
            ptr::null_mut(),
        ))?;
        self.input_over_signaled = true;
        Ok(())
    }

    fn query_consumed(&self) -> Result<i32> {
        let mut consumed = 0i32;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_GET_CURIDX_INPUT_BUF as i32,
            0,
            (&mut consumed as *mut _) as *mut c_void,
        ))?;
        Ok(consumed.max(0))
    }

    fn set_config(&mut self, index: i32, value: i32) -> Result<()> {
        let mut value = value;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_SET_CONFIG_PARAM as i32,
            index,
            (&mut value as *mut _) as *mut c_void,
        ))
    }

    fn set_config_optional(&mut self, index: i32, value: i32) -> Result<()> {
        let mut value = value;
        let status = self.call_with_value(
            libxaac_sys::IA_API_CMD_SET_CONFIG_PARAM as i32,
            index,
            (&mut value as *mut _) as *mut c_void,
        );
        if status == libxaac_sys::IA_NO_ERROR as i32 || !is_fatal(status) {
            Ok(())
        } else {
            Err(Error::DecoderError(status))
        }
    }

    fn set_config_f32(&mut self, index: i32, value: f32) -> Result<()> {
        let mut value = value;
        check_decoder(self.call_with_value(
            libxaac_sys::IA_API_CMD_SET_CONFIG_PARAM as i32,
            index,
            (&mut value as *mut _) as *mut c_void,
        ))
    }

    fn get_config_required(&self, index: i32) -> Result<i32> {
        self.get_config_optional(index)?
            .ok_or(Error::DecoderError(libxaac_sys::IA_FATAL_ERROR as i32))
    }

    fn get_config_optional(&self, index: i32) -> Result<Option<i32>> {
        let mut value = 0i32;
        let status = self.get_config(index, &mut value);
        if status == libxaac_sys::IA_NO_ERROR as i32 {
            Ok(Some(value))
        } else if is_fatal(status) {
            Err(Error::DecoderError(status))
        } else {
            Ok(None)
        }
    }

    fn get_config_raw(&self, index: i32, value: *mut c_void) -> i32 {
        unsafe {
            libxaac_sys::ixheaacd_dec_api(
                self.api.as_ptr().cast::<c_void>(),
                libxaac_sys::IA_API_CMD_GET_CONFIG_PARAM as i32,
                index,
                value,
            )
        }
    }

    fn get_config(&self, index: i32, value: &mut i32) -> i32 {
        self.get_config_raw(index, (value as *mut _) as *mut c_void)
    }

    fn call(&self, cmd: i32, idx: i32, value: *mut c_void) -> i32 {
        unsafe {
            libxaac_sys::ixheaacd_dec_api(self.api.as_ptr().cast::<c_void>(), cmd, idx, value)
        }
    }

    fn call_with_value(&self, cmd: i32, idx: i32, value: *mut c_void) -> i32 {
        self.call(cmd, idx, value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InitState {
    Ready,
    NeedMoreInput,
    Continue,
    EndOfStream,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExecuteState {
    Frame(DecodedFrame),
    NeedMoreInput,
    Continue,
    EndOfStream,
}

#[derive(Debug, Default)]
struct ExtensionPayloads {
    loudness_payloads: Vec<Vec<u8>>,
    config_payloads: Vec<Vec<u8>>,
}

impl ExtensionPayloads {
    fn has_any(&self) -> bool {
        !self.loudness_payloads.is_empty() || !self.config_payloads.is_empty()
    }
}

impl DrcEffectType {
    fn from_raw(value: i32) -> Self {
        match value {
            1 => Self::Night,
            2 => Self::Noisy,
            3 => Self::Limited,
            4 => Self::LowLevel,
            5 => Self::Dialog,
            6 => Self::GeneralCompression,
            7 => Self::Expanded,
            8 => Self::Articulated,
            9 => Self::Headphone,
            10 => Self::PortableSpeaker,
            11 => Self::StereoDownmix,
            _ => Self::None,
        }
    }
}

fn is_fatal(status: i32) -> bool {
    (status as u32 & libxaac_sys::IA_FATAL_ERROR) != 0
}

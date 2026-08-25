use std::cell::RefCell;

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyModule};

use crate::decoder::{
    ChannelMode as RustChannelMode, DecodeProgress as RustDecodeProgress,
    DecodeStatus as RustDecodeStatus, DecodedFrame as RustDecodedFrame, Decoder as RustDecoder,
    DecoderConfig as RustDecoderConfig, DecoderDrcConfig as RustDecoderDrcConfig,
    DecoderTransport as RustDecoderTransport, DrcEffectType as RustDrcEffectType,
    ProfileHint as RustProfileHint, RawStreamConfig as RustRawStreamConfig, SbrMode as RustSbrMode,
    StreamInfo as RustStreamInfo,
};
use crate::encoder::{
    EncodedFrame as RustEncodedFrame, EncodedPacket as RustEncodedPacket, Encoder as RustEncoder,
    EncoderConfig as RustEncoderConfig, EncoderDrcConfig as RustEncoderDrcConfig,
    InverseQuantizationMode as RustInverseQuantizationMode, OutputFormat as RustOutputFormat,
    Profile as RustProfile, UsacCodecMode as RustUsacCodecMode,
    UsacFrameLengthIndex as RustUsacFrameLengthIndex,
};
use crate::error::Error;
use crate::util::VersionInfo as RustVersionInfo;

create_exception!(xaac_rs, XaacError, PyException);

#[pyclass(module = "xaac_rs", eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    AacLc,
    HeAacV1,
    AacLd,
    HeAacV2,
    AacEld,
    Usac,
}

#[pyclass(module = "xaac_rs", eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Raw,
    Adts,
}

#[pyclass(module = "xaac_rs", eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InverseQuantizationMode {
    Off,
    On,
    High,
}

#[pyclass(module = "xaac_rs", eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsacCodecMode {
    Switched,
    FrequencyDomain,
    TimeDomain,
}

#[pyclass(module = "xaac_rs", eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsacFrameLengthIndex {
    NoSbr768,
    NoSbr1024,
    Esbr8_3_768,
    Esbr2_1_1024,
    Esbr4_1_1024,
}

#[pyclass(module = "xaac_rs", eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoderTransport {
    Auto,
    Raw,
    Mp4Raw,
}

#[pyclass(module = "xaac_rs", eq, eq_int)]
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

#[pyclass(module = "xaac_rs", eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbrMode {
    Unknown,
    Disabled,
    Enabled,
    Downsampled,
    Esbr,
}

#[pyclass(module = "xaac_rs", eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileHint {
    AacLc,
    HeAacV1,
    AacLd,
    HeAacV2,
    AacEld,
    Usac,
}

#[pyclass(module = "xaac_rs")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelMode {
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub raw_value: i32,
}

#[pymethods]
impl ChannelMode {
    fn __repr__(&self) -> String {
        format!(
            "ChannelMode(name='{}', raw_value={})",
            self.name, self.raw_value
        )
    }
}

#[pyclass(module = "xaac_rs", get_all, set_all)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    pub name: String,
    pub version: String,
}

#[pymethods]
impl VersionInfo {
    #[new]
    fn new() -> Self {
        Self {
            name: String::new(),
            version: String::new(),
        }
    }
}

#[pyclass(module = "xaac_rs", get_all, set_all)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderDrcConfig {
    pub use_drc_element: bool,
    pub frame_size: Option<i32>,
    pub config_payload: Vec<u8>,
}

#[pymethods]
impl EncoderDrcConfig {
    #[new]
    fn new() -> Self {
        Self {
            use_drc_element: false,
            frame_size: None,
            config_payload: Vec::new(),
        }
    }
}

#[pyclass(module = "xaac_rs", get_all, set_all)]
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

#[pymethods]
impl EncoderConfig {
    #[new]
    fn new() -> Self {
        Self::from(RustEncoderConfig::default())
    }
}

#[pyclass(module = "xaac_rs")]
#[derive(Debug, Clone)]
pub struct EncodedPacket {
    data: Vec<u8>,
    #[pyo3(get)]
    pub bytes_consumed: usize,
}

#[pymethods]
impl EncodedPacket {
    #[new]
    fn new() -> Self {
        Self {
            data: Vec::new(),
            bytes_consumed: 0,
        }
    }

    #[getter]
    fn data<'py>(&self, py: Python<'py>) -> Py<PyBytes> {
        PyBytes::new(py, &self.data).into()
    }
}

#[pyclass(module = "xaac_rs")]
#[derive(Debug, Clone)]
pub struct EncodedFrame {
    #[pyo3(get)]
    pub packet: EncodedPacket,
    #[pyo3(get)]
    pub padded_input_bytes: usize,
}

#[pymethods]
impl EncodedFrame {
    #[new]
    fn new() -> Self {
        Self {
            packet: EncodedPacket::new(),
            padded_input_bytes: 0,
        }
    }
}

#[pyclass(module = "xaac_rs", get_all, set_all)]
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

#[pymethods]
impl DecoderDrcConfig {
    #[new]
    fn new() -> Self {
        Self::from(RustDecoderDrcConfig::default())
    }
}

#[pyclass(module = "xaac_rs", get_all, set_all)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawStreamConfig {
    pub sample_rate: u32,
    pub audio_object_type: ProfileHint,
}

#[pymethods]
impl RawStreamConfig {
    #[new]
    fn new() -> Self {
        Self {
            sample_rate: 0,
            audio_object_type: ProfileHint::AacLc,
        }
    }
}

#[pyclass(module = "xaac_rs", get_all, set_all)]
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

#[pymethods]
impl DecoderConfig {
    #[new]
    fn new() -> Self {
        Self::from(RustDecoderConfig::default())
    }
}

#[pyclass(module = "xaac_rs", get_all, set_all)]
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

#[pymethods]
impl StreamInfo {
    #[new]
    fn new() -> Self {
        Self {
            sample_rate: 0,
            channels: 0,
            channel_mask: 0,
            channel_mode: None,
            pcm_word_size: 0,
            sbr_mode: SbrMode::Unknown,
            audio_object_type: 0,
            drc_effect_type: None,
            drc_target_loudness: None,
            drc_loudness_norm: None,
            loudness_leveling: None,
            preroll_frames: None,
            drc_config_changed: None,
            drc_apply_crossfade: None,
            extension_elements: None,
            config_extensions: None,
            gain_payload_len: None,
            drc_active: false,
        }
    }
}

#[pyclass(module = "xaac_rs", get_all, set_all)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeProgress {
    pub initialized: bool,
    pub stream_info: Option<StreamInfo>,
}

#[pymethods]
impl DecodeProgress {
    #[new]
    fn new() -> Self {
        Self {
            initialized: false,
            stream_info: None,
        }
    }
}

#[pyclass(module = "xaac_rs")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFrame {
    pcm: Vec<u8>,
    #[pyo3(get)]
    pub bytes_consumed: usize,
    #[pyo3(get)]
    pub stream_info: StreamInfo,
}

#[pymethods]
impl DecodedFrame {
    #[new]
    fn new() -> Self {
        Self {
            pcm: Vec::new(),
            bytes_consumed: 0,
            stream_info: StreamInfo::new(),
        }
    }

    #[getter]
    fn pcm<'py>(&self, py: Python<'py>) -> Py<PyBytes> {
        PyBytes::new(py, &self.pcm).into()
    }

    fn pcm_i16(&self) -> Option<Vec<i16>> {
        RustDecodedFrame {
            pcm: self.pcm.clone(),
            bytes_consumed: self.bytes_consumed,
            stream_info: self.stream_info.clone().into(),
        }
        .pcm_i16()
    }

    fn pcm_i24(&self) -> Option<Vec<i32>> {
        RustDecodedFrame {
            pcm: self.pcm.clone(),
            bytes_consumed: self.bytes_consumed,
            stream_info: self.stream_info.clone().into(),
        }
        .pcm_i24()
    }
}

#[pyclass(module = "xaac_rs", get_all, set_all)]
#[derive(Debug, Clone)]
pub struct DecodeStatus {
    pub kind: String,
    pub frame: Option<DecodedFrame>,
    pub progress: Option<DecodeProgress>,
}

#[pymethods]
impl DecodeStatus {
    #[new]
    fn new() -> Self {
        Self {
            kind: "end_of_stream".to_string(),
            frame: None,
            progress: None,
        }
    }

    #[getter]
    fn is_frame(&self) -> bool {
        self.kind == "frame"
    }

    #[getter]
    fn is_need_more_input(&self) -> bool {
        self.kind == "need_more_input"
    }

    #[getter]
    fn is_end_of_stream(&self) -> bool {
        self.kind == "end_of_stream"
    }
}

#[pyclass(module = "xaac_rs", unsendable)]
pub struct Encoder {
    inner: RefCell<RustEncoder>,
}

#[pymethods]
impl Encoder {
    #[new]
    #[pyo3(signature = (config=None))]
    fn new(config: Option<EncoderConfig>) -> PyResult<Self> {
        let config = config.unwrap_or_else(EncoderConfig::new);
        let inner = RustEncoder::new(config.into()).map_err(to_py_err)?;
        Ok(Self {
            inner: RefCell::new(inner),
        })
    }

    fn config(&self) -> EncoderConfig {
        self.inner.borrow().config().clone().into()
    }

    fn version(&self) -> VersionInfo {
        self.inner.borrow().version().clone().into()
    }

    fn input_frame_bytes(&self) -> usize {
        self.inner.borrow().input_frame_bytes()
    }

    fn input_samples_per_channel(&self) -> usize {
        self.inner.borrow().input_samples_per_channel()
    }

    fn expected_frame_count(&self) -> usize {
        self.inner.borrow().expected_frame_count()
    }

    fn audio_specific_config<'py>(&self, py: Python<'py>) -> Py<PyBytes> {
        PyBytes::new(py, self.inner.borrow().audio_specific_config()).into()
    }

    fn encode_pcm_bytes(&self, pcm: Vec<u8>) -> PyResult<EncodedPacket> {
        self.inner
            .borrow_mut()
            .encode_pcm_bytes(&pcm)
            .map(Into::into)
            .map_err(to_py_err)
    }

    fn encode_pcm_bytes_with_padding(&self, pcm: Vec<u8>) -> PyResult<EncodedFrame> {
        self.inner
            .borrow_mut()
            .encode_pcm_bytes_with_padding(&pcm)
            .map(Into::into)
            .map_err(to_py_err)
    }

    fn encode_i16_interleaved(&self, pcm: Vec<i16>) -> PyResult<EncodedPacket> {
        self.inner
            .borrow_mut()
            .encode_i16_interleaved(&pcm)
            .map(Into::into)
            .map_err(to_py_err)
    }

    fn encode_i24_interleaved(&self, pcm: Vec<u8>) -> PyResult<EncodedPacket> {
        if pcm.len() % 3 != 0 {
            return Err(PyValueError::new_err(
                "encode_i24_interleaved expects raw 24-bit PCM bytes in 3-byte little-endian samples",
            ));
        }
        let samples: Vec<[u8; 3]> = pcm
            .chunks_exact(3)
            .map(|chunk| [chunk[0], chunk[1], chunk[2]])
            .collect();
        self.inner
            .borrow_mut()
            .encode_i24_interleaved(&samples)
            .map(Into::into)
            .map_err(to_py_err)
    }

    fn encode_i32_interleaved(&self, pcm: Vec<i32>) -> PyResult<EncodedPacket> {
        self.inner
            .borrow_mut()
            .encode_i32_interleaved(&pcm)
            .map(Into::into)
            .map_err(to_py_err)
    }
}

#[pyclass(module = "xaac_rs", unsendable)]
pub struct Decoder {
    inner: RefCell<RustDecoder>,
}

#[pymethods]
impl Decoder {
    #[new]
    #[pyo3(signature = (config=None))]
    fn new(config: Option<DecoderConfig>) -> PyResult<Self> {
        let config = config.unwrap_or_else(DecoderConfig::new);
        let inner = RustDecoder::new(config.into()).map_err(to_py_err)?;
        Ok(Self {
            inner: RefCell::new(inner),
        })
    }

    fn config(&self) -> DecoderConfig {
        self.inner.borrow().config().clone().into()
    }

    fn version(&self) -> VersionInfo {
        self.inner.borrow().version().clone().into()
    }

    fn input_capacity(&self) -> usize {
        self.inner.borrow().input_capacity()
    }

    fn stream_info(&self) -> Option<StreamInfo> {
        self.inner.borrow().stream_info().cloned().map(Into::into)
    }

    fn probe_stream_info(&self, data: Vec<u8>) -> PyResult<StreamInfo> {
        self.inner
            .borrow_mut()
            .probe_stream_info(&data)
            .map(Into::into)
            .map_err(to_py_err)
    }

    fn decode(&self, input: Vec<u8>) -> PyResult<DecodedFrame> {
        self.inner
            .borrow_mut()
            .decode(&input)
            .map(Into::into)
            .map_err(to_py_err)
    }

    fn decode_stream_chunk(&self, input: Vec<u8>) -> PyResult<DecodeStatus> {
        self.inner
            .borrow_mut()
            .decode_stream_chunk(&input)
            .map(Into::into)
            .map_err(to_py_err)
    }

    fn finish(&self) -> PyResult<DecodeStatus> {
        self.inner
            .borrow_mut()
            .finish()
            .map(Into::into)
            .map_err(to_py_err)
    }

    fn signal_end_of_input(&self) -> PyResult<()> {
        self.inner
            .borrow_mut()
            .signal_end_of_input()
            .map_err(to_py_err)
    }
}

fn to_py_err(err: Error) -> PyErr {
    XaacError::new_err(err.to_string())
}

impl From<Profile> for RustProfile {
    fn from(value: Profile) -> Self {
        match value {
            Profile::AacLc => Self::AacLc,
            Profile::HeAacV1 => Self::HeAacV1,
            Profile::AacLd => Self::AacLd,
            Profile::HeAacV2 => Self::HeAacV2,
            Profile::AacEld => Self::AacEld,
            Profile::Usac => Self::Usac,
        }
    }
}

impl From<RustProfile> for Profile {
    fn from(value: RustProfile) -> Self {
        match value {
            RustProfile::AacLc => Self::AacLc,
            RustProfile::HeAacV1 => Self::HeAacV1,
            RustProfile::AacLd => Self::AacLd,
            RustProfile::HeAacV2 => Self::HeAacV2,
            RustProfile::AacEld => Self::AacEld,
            RustProfile::Usac => Self::Usac,
        }
    }
}

impl From<OutputFormat> for RustOutputFormat {
    fn from(value: OutputFormat) -> Self {
        match value {
            OutputFormat::Raw => Self::Raw,
            OutputFormat::Adts => Self::Adts,
        }
    }
}

impl From<RustOutputFormat> for OutputFormat {
    fn from(value: RustOutputFormat) -> Self {
        match value {
            RustOutputFormat::Raw => Self::Raw,
            RustOutputFormat::Adts => Self::Adts,
        }
    }
}

impl From<InverseQuantizationMode> for RustInverseQuantizationMode {
    fn from(value: InverseQuantizationMode) -> Self {
        match value {
            InverseQuantizationMode::Off => Self::Off,
            InverseQuantizationMode::On => Self::On,
            InverseQuantizationMode::High => Self::High,
        }
    }
}

impl From<RustInverseQuantizationMode> for InverseQuantizationMode {
    fn from(value: RustInverseQuantizationMode) -> Self {
        match value {
            RustInverseQuantizationMode::Off => Self::Off,
            RustInverseQuantizationMode::On => Self::On,
            RustInverseQuantizationMode::High => Self::High,
        }
    }
}

impl From<UsacCodecMode> for RustUsacCodecMode {
    fn from(value: UsacCodecMode) -> Self {
        match value {
            UsacCodecMode::Switched => Self::Switched,
            UsacCodecMode::FrequencyDomain => Self::FrequencyDomain,
            UsacCodecMode::TimeDomain => Self::TimeDomain,
        }
    }
}

impl From<RustUsacCodecMode> for UsacCodecMode {
    fn from(value: RustUsacCodecMode) -> Self {
        match value {
            RustUsacCodecMode::Switched => Self::Switched,
            RustUsacCodecMode::FrequencyDomain => Self::FrequencyDomain,
            RustUsacCodecMode::TimeDomain => Self::TimeDomain,
        }
    }
}

impl From<UsacFrameLengthIndex> for RustUsacFrameLengthIndex {
    fn from(value: UsacFrameLengthIndex) -> Self {
        match value {
            UsacFrameLengthIndex::NoSbr768 => Self::NoSbr768,
            UsacFrameLengthIndex::NoSbr1024 => Self::NoSbr1024,
            UsacFrameLengthIndex::Esbr8_3_768 => Self::Esbr8_3_768,
            UsacFrameLengthIndex::Esbr2_1_1024 => Self::Esbr2_1_1024,
            UsacFrameLengthIndex::Esbr4_1_1024 => Self::Esbr4_1_1024,
        }
    }
}

impl From<RustUsacFrameLengthIndex> for UsacFrameLengthIndex {
    fn from(value: RustUsacFrameLengthIndex) -> Self {
        match value {
            RustUsacFrameLengthIndex::NoSbr768 => Self::NoSbr768,
            RustUsacFrameLengthIndex::NoSbr1024 => Self::NoSbr1024,
            RustUsacFrameLengthIndex::Esbr8_3_768 => Self::Esbr8_3_768,
            RustUsacFrameLengthIndex::Esbr2_1_1024 => Self::Esbr2_1_1024,
            RustUsacFrameLengthIndex::Esbr4_1_1024 => Self::Esbr4_1_1024,
        }
    }
}

impl From<DecoderTransport> for RustDecoderTransport {
    fn from(value: DecoderTransport) -> Self {
        match value {
            DecoderTransport::Auto => Self::Auto,
            DecoderTransport::Raw => Self::Raw,
            DecoderTransport::Mp4Raw => Self::Mp4Raw,
        }
    }
}

impl From<RustDecoderTransport> for DecoderTransport {
    fn from(value: RustDecoderTransport) -> Self {
        match value {
            RustDecoderTransport::Auto => Self::Auto,
            RustDecoderTransport::Raw => Self::Raw,
            RustDecoderTransport::Mp4Raw => Self::Mp4Raw,
        }
    }
}

impl From<DrcEffectType> for RustDrcEffectType {
    fn from(value: DrcEffectType) -> Self {
        match value {
            DrcEffectType::None => Self::None,
            DrcEffectType::Night => Self::Night,
            DrcEffectType::Noisy => Self::Noisy,
            DrcEffectType::Limited => Self::Limited,
            DrcEffectType::LowLevel => Self::LowLevel,
            DrcEffectType::Dialog => Self::Dialog,
            DrcEffectType::GeneralCompression => Self::GeneralCompression,
            DrcEffectType::Expanded => Self::Expanded,
            DrcEffectType::Articulated => Self::Articulated,
            DrcEffectType::Headphone => Self::Headphone,
            DrcEffectType::PortableSpeaker => Self::PortableSpeaker,
            DrcEffectType::StereoDownmix => Self::StereoDownmix,
        }
    }
}

impl From<RustDrcEffectType> for DrcEffectType {
    fn from(value: RustDrcEffectType) -> Self {
        match value {
            RustDrcEffectType::None => Self::None,
            RustDrcEffectType::Night => Self::Night,
            RustDrcEffectType::Noisy => Self::Noisy,
            RustDrcEffectType::Limited => Self::Limited,
            RustDrcEffectType::LowLevel => Self::LowLevel,
            RustDrcEffectType::Dialog => Self::Dialog,
            RustDrcEffectType::GeneralCompression => Self::GeneralCompression,
            RustDrcEffectType::Expanded => Self::Expanded,
            RustDrcEffectType::Articulated => Self::Articulated,
            RustDrcEffectType::Headphone => Self::Headphone,
            RustDrcEffectType::PortableSpeaker => Self::PortableSpeaker,
            RustDrcEffectType::StereoDownmix => Self::StereoDownmix,
        }
    }
}

impl From<RustSbrMode> for SbrMode {
    fn from(value: RustSbrMode) -> Self {
        match value {
            RustSbrMode::Unknown => Self::Unknown,
            RustSbrMode::Disabled => Self::Disabled,
            RustSbrMode::Enabled => Self::Enabled,
            RustSbrMode::Downsampled => Self::Downsampled,
            RustSbrMode::Esbr => Self::Esbr,
        }
    }
}

impl From<ProfileHint> for RustProfileHint {
    fn from(value: ProfileHint) -> Self {
        match value {
            ProfileHint::AacLc => Self::AacLc,
            ProfileHint::HeAacV1 => Self::HeAacV1,
            ProfileHint::AacLd => Self::AacLd,
            ProfileHint::HeAacV2 => Self::HeAacV2,
            ProfileHint::AacEld => Self::AacEld,
            ProfileHint::Usac => Self::Usac,
        }
    }
}

impl From<RustProfileHint> for ProfileHint {
    fn from(value: RustProfileHint) -> Self {
        match value {
            RustProfileHint::AacLc => Self::AacLc,
            RustProfileHint::HeAacV1 => Self::HeAacV1,
            RustProfileHint::AacLd => Self::AacLd,
            RustProfileHint::HeAacV2 => Self::HeAacV2,
            RustProfileHint::AacEld => Self::AacEld,
            RustProfileHint::Usac => Self::Usac,
        }
    }
}

impl From<RustChannelMode> for ChannelMode {
    fn from(value: RustChannelMode) -> Self {
        match value {
            RustChannelMode::Mono => Self {
                name: "Mono".to_string(),
                raw_value: 0,
            },
            RustChannelMode::Stereo => Self {
                name: "Stereo".to_string(),
                raw_value: 1,
            },
            RustChannelMode::DualMono => Self {
                name: "DualMono".to_string(),
                raw_value: 2,
            },
            RustChannelMode::ParametricStereo => Self {
                name: "ParametricStereo".to_string(),
                raw_value: 3,
            },
            RustChannelMode::Unknown(value) => Self {
                name: "Unknown".to_string(),
                raw_value: value,
            },
        }
    }
}

impl From<EncoderDrcConfig> for RustEncoderDrcConfig {
    fn from(value: EncoderDrcConfig) -> Self {
        Self {
            use_drc_element: value.use_drc_element,
            frame_size: value.frame_size,
            config_payload: value.config_payload,
        }
    }
}

impl From<RustEncoderDrcConfig> for EncoderDrcConfig {
    fn from(value: RustEncoderDrcConfig) -> Self {
        Self {
            use_drc_element: value.use_drc_element,
            frame_size: value.frame_size,
            config_payload: value.config_payload,
        }
    }
}

impl From<EncoderConfig> for RustEncoderConfig {
    fn from(value: EncoderConfig) -> Self {
        Self {
            profile: value.profile.into(),
            sample_rate: value.sample_rate,
            native_sample_rate: value.native_sample_rate,
            channels: value.channels,
            channel_mask: value.channel_mask,
            num_coupling_channels: value.num_coupling_channels,
            bitrate: value.bitrate,
            pcm_word_size: value.pcm_word_size,
            frame_length: value.frame_length,
            output_format: value.output_format.into(),
            bandwidth: value.bandwidth,
            dual_mono: value.dual_mono,
            use_tns: value.use_tns,
            noise_filling: value.noise_filling,
            full_bandwidth: value.full_bandwidth,
            private_bit: value.private_bit,
            copyright_bit: value.copyright_bit,
            original_copy_bit: value.original_copy_bit,
            no_stereo_preprocessing: value.no_stereo_preprocessing,
            use_mps: value.use_mps,
            mps_tree_config: value.mps_tree_config,
            complex_prediction: value.complex_prediction,
            bit_reservoir_size: value.bit_reservoir_size,
            inverse_quantization: value.inverse_quantization.into(),
            enable_esbr: value.enable_esbr,
            high_quality_esbr: value.high_quality_esbr,
            pvc: value.pvc,
            harmonic_sbr: value.harmonic_sbr,
            inter_tes: value.inter_tes,
            usac_mode: value.usac_mode.into(),
            usac_frame_length_index: value.usac_frame_length_index.into(),
            random_access_interval_ms: value.random_access_interval_ms,
            stream_id: value.stream_id,
            use_delay_adjustment: value.use_delay_adjustment,
            write_program_config_element: value.write_program_config_element,
            measured_loudness: value.measured_loudness,
            sample_peak_level: value.sample_peak_level,
            drc: value.drc.map(Into::into),
        }
    }
}

impl From<RustEncoderConfig> for EncoderConfig {
    fn from(value: RustEncoderConfig) -> Self {
        Self {
            profile: value.profile.into(),
            sample_rate: value.sample_rate,
            native_sample_rate: value.native_sample_rate,
            channels: value.channels,
            channel_mask: value.channel_mask,
            num_coupling_channels: value.num_coupling_channels,
            bitrate: value.bitrate,
            pcm_word_size: value.pcm_word_size,
            frame_length: value.frame_length,
            output_format: value.output_format.into(),
            bandwidth: value.bandwidth,
            dual_mono: value.dual_mono,
            use_tns: value.use_tns,
            noise_filling: value.noise_filling,
            full_bandwidth: value.full_bandwidth,
            private_bit: value.private_bit,
            copyright_bit: value.copyright_bit,
            original_copy_bit: value.original_copy_bit,
            no_stereo_preprocessing: value.no_stereo_preprocessing,
            use_mps: value.use_mps,
            mps_tree_config: value.mps_tree_config,
            complex_prediction: value.complex_prediction,
            bit_reservoir_size: value.bit_reservoir_size,
            inverse_quantization: value.inverse_quantization.into(),
            enable_esbr: value.enable_esbr,
            high_quality_esbr: value.high_quality_esbr,
            pvc: value.pvc,
            harmonic_sbr: value.harmonic_sbr,
            inter_tes: value.inter_tes,
            usac_mode: value.usac_mode.into(),
            usac_frame_length_index: value.usac_frame_length_index.into(),
            random_access_interval_ms: value.random_access_interval_ms,
            stream_id: value.stream_id,
            use_delay_adjustment: value.use_delay_adjustment,
            write_program_config_element: value.write_program_config_element,
            measured_loudness: value.measured_loudness,
            sample_peak_level: value.sample_peak_level,
            drc: value.drc.map(Into::into),
        }
    }
}

impl From<RustEncodedPacket> for EncodedPacket {
    fn from(value: RustEncodedPacket) -> Self {
        Self {
            data: value.data,
            bytes_consumed: value.bytes_consumed,
        }
    }
}

impl From<RustEncodedFrame> for EncodedFrame {
    fn from(value: RustEncodedFrame) -> Self {
        Self {
            packet: value.packet.into(),
            padded_input_bytes: value.padded_input_bytes,
        }
    }
}

impl From<DecoderDrcConfig> for RustDecoderDrcConfig {
    fn from(value: DecoderDrcConfig) -> Self {
        Self {
            enabled: value.enabled,
            cut: value.cut,
            boost: value.boost,
            target_level: value.target_level,
            heavy_compression: value.heavy_compression,
            effect_type: value.effect_type.into(),
            target_loudness: value.target_loudness,
            loudness_leveling: value.loudness_leveling,
        }
    }
}

impl From<RustDecoderDrcConfig> for DecoderDrcConfig {
    fn from(value: RustDecoderDrcConfig) -> Self {
        Self {
            enabled: value.enabled,
            cut: value.cut,
            boost: value.boost,
            target_level: value.target_level,
            heavy_compression: value.heavy_compression,
            effect_type: value.effect_type.into(),
            target_loudness: value.target_loudness,
            loudness_leveling: value.loudness_leveling,
        }
    }
}

impl From<RawStreamConfig> for RustRawStreamConfig {
    fn from(value: RawStreamConfig) -> Self {
        Self {
            sample_rate: value.sample_rate,
            audio_object_type: value.audio_object_type.into(),
        }
    }
}

impl From<RustRawStreamConfig> for RawStreamConfig {
    fn from(value: RustRawStreamConfig) -> Self {
        Self {
            sample_rate: value.sample_rate,
            audio_object_type: value.audio_object_type.into(),
        }
    }
}

impl From<DecoderConfig> for RustDecoderConfig {
    fn from(value: DecoderConfig) -> Self {
        Self {
            transport: value.transport.into(),
            pcm_word_size: value.pcm_word_size,
            raw: value.raw.map(Into::into),
            downmix_to_mono: value.downmix_to_mono,
            to_stereo: value.to_stereo,
            downsample_sbr: value.downsample_sbr,
            max_channels: value.max_channels,
            coupling_channels: value.coupling_channels,
            downmix_to_stereo: value.downmix_to_stereo,
            disable_sync: value.disable_sync,
            auto_sbr_upsample: value.auto_sbr_upsample,
            ld_frame_480: value.ld_frame_480,
            hq_esbr: value.hq_esbr,
            ps_enable: value.ps_enable,
            peak_limiter: value.peak_limiter,
            frame_length_960: value.frame_length_960,
            error_concealment: value.error_concealment,
            enable_esbr: value.enable_esbr,
            drc: value.drc.into(),
        }
    }
}

impl From<RustDecoderConfig> for DecoderConfig {
    fn from(value: RustDecoderConfig) -> Self {
        Self {
            transport: value.transport.into(),
            pcm_word_size: value.pcm_word_size,
            raw: value.raw.map(Into::into),
            downmix_to_mono: value.downmix_to_mono,
            to_stereo: value.to_stereo,
            downsample_sbr: value.downsample_sbr,
            max_channels: value.max_channels,
            coupling_channels: value.coupling_channels,
            downmix_to_stereo: value.downmix_to_stereo,
            disable_sync: value.disable_sync,
            auto_sbr_upsample: value.auto_sbr_upsample,
            ld_frame_480: value.ld_frame_480,
            hq_esbr: value.hq_esbr,
            ps_enable: value.ps_enable,
            peak_limiter: value.peak_limiter,
            frame_length_960: value.frame_length_960,
            error_concealment: value.error_concealment,
            enable_esbr: value.enable_esbr,
            drc: value.drc.into(),
        }
    }
}

impl From<RustStreamInfo> for StreamInfo {
    fn from(value: RustStreamInfo) -> Self {
        Self {
            sample_rate: value.sample_rate,
            channels: value.channels,
            channel_mask: value.channel_mask,
            channel_mode: value.channel_mode.map(Into::into),
            pcm_word_size: value.pcm_word_size,
            sbr_mode: value.sbr_mode.into(),
            audio_object_type: value.audio_object_type,
            drc_effect_type: value.drc_effect_type.map(Into::into),
            drc_target_loudness: value.drc_target_loudness,
            drc_loudness_norm: value.drc_loudness_norm,
            loudness_leveling: value.loudness_leveling,
            preroll_frames: value.preroll_frames,
            drc_config_changed: value.drc_config_changed,
            drc_apply_crossfade: value.drc_apply_crossfade,
            extension_elements: value.extension_elements,
            config_extensions: value.config_extensions,
            gain_payload_len: value.gain_payload_len,
            drc_active: value.drc_active,
        }
    }
}

impl From<StreamInfo> for RustStreamInfo {
    fn from(value: StreamInfo) -> Self {
        Self {
            sample_rate: value.sample_rate,
            channels: value.channels,
            channel_mask: value.channel_mask,
            channel_mode: value.channel_mode.map(|mode| match mode.raw_value {
                0 => RustChannelMode::Mono,
                1 => RustChannelMode::Stereo,
                2 => RustChannelMode::DualMono,
                3 => RustChannelMode::ParametricStereo,
                other => RustChannelMode::Unknown(other),
            }),
            pcm_word_size: value.pcm_word_size,
            sbr_mode: match value.sbr_mode {
                SbrMode::Unknown => RustSbrMode::Unknown,
                SbrMode::Disabled => RustSbrMode::Disabled,
                SbrMode::Enabled => RustSbrMode::Enabled,
                SbrMode::Downsampled => RustSbrMode::Downsampled,
                SbrMode::Esbr => RustSbrMode::Esbr,
            },
            audio_object_type: value.audio_object_type,
            drc_effect_type: value.drc_effect_type.map(Into::into),
            drc_target_loudness: value.drc_target_loudness,
            drc_loudness_norm: value.drc_loudness_norm,
            loudness_leveling: value.loudness_leveling,
            preroll_frames: value.preroll_frames,
            drc_config_changed: value.drc_config_changed,
            drc_apply_crossfade: value.drc_apply_crossfade,
            extension_elements: value.extension_elements,
            config_extensions: value.config_extensions,
            gain_payload_len: value.gain_payload_len,
            drc_active: value.drc_active,
        }
    }
}

impl From<RustDecodeProgress> for DecodeProgress {
    fn from(value: RustDecodeProgress) -> Self {
        Self {
            initialized: value.initialized,
            stream_info: value.stream_info.map(Into::into),
        }
    }
}

impl From<RustDecodedFrame> for DecodedFrame {
    fn from(value: RustDecodedFrame) -> Self {
        Self {
            pcm: value.pcm,
            bytes_consumed: value.bytes_consumed,
            stream_info: value.stream_info.into(),
        }
    }
}

impl From<RustDecodeStatus> for DecodeStatus {
    fn from(value: RustDecodeStatus) -> Self {
        match value {
            RustDecodeStatus::Frame(frame) => Self {
                kind: "frame".to_string(),
                frame: Some(frame.into()),
                progress: None,
            },
            RustDecodeStatus::NeedMoreInput(progress) => Self {
                kind: "need_more_input".to_string(),
                frame: None,
                progress: Some(progress.into()),
            },
            RustDecodeStatus::EndOfStream => Self {
                kind: "end_of_stream".to_string(),
                frame: None,
                progress: None,
            },
        }
    }
}

impl From<RustVersionInfo> for VersionInfo {
    fn from(value: RustVersionInfo) -> Self {
        Self {
            name: value.name,
            version: value.version,
        }
    }
}

#[pymodule]
fn xaac_rs(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("XaacError", _py.get_type::<XaacError>())?;

    m.add_class::<Profile>()?;
    m.add_class::<OutputFormat>()?;
    m.add_class::<InverseQuantizationMode>()?;
    m.add_class::<UsacCodecMode>()?;
    m.add_class::<UsacFrameLengthIndex>()?;
    m.add_class::<DecoderTransport>()?;
    m.add_class::<DrcEffectType>()?;
    m.add_class::<SbrMode>()?;
    m.add_class::<ProfileHint>()?;
    m.add_class::<ChannelMode>()?;

    m.add_class::<VersionInfo>()?;
    m.add_class::<EncoderDrcConfig>()?;
    m.add_class::<EncoderConfig>()?;
    m.add_class::<EncodedPacket>()?;
    m.add_class::<EncodedFrame>()?;
    m.add_class::<DecoderDrcConfig>()?;
    m.add_class::<RawStreamConfig>()?;
    m.add_class::<DecoderConfig>()?;
    m.add_class::<StreamInfo>()?;
    m.add_class::<DecodeProgress>()?;
    m.add_class::<DecodedFrame>()?;
    m.add_class::<DecodeStatus>()?;
    m.add_class::<Encoder>()?;
    m.add_class::<Decoder>()?;
    Ok(())
}

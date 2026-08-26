#[cfg(feature = "transcode-aac")]
mod aac;
#[cfg(feature = "transcode-ac3")]
mod ac3;
#[cfg(feature = "transcode-dts")]
mod dts;
mod frames;
mod pcm;
mod plan;
#[cfg(all(feature = "transcode-aac", feature = "demux"))]
mod rendition;
#[cfg(all(feature = "transcode-aac", feature = "casting"))]
mod ts;
#[cfg(all(feature = "transcode-aac", feature = "casting"))]
mod video;
mod session;
mod source;
mod wav;

#[cfg(feature = "transcode-aac")]
#[allow(unused_imports)]
pub use aac::{adts_payloads, audio_specific_config, AacEncoder};
#[cfg(feature = "transcode-ac3")]
#[allow(unused_imports)]
pub use ac3::{Ac3Encoder, AC3_FRAME_SAMPLES};
#[allow(unused_imports)]
pub use frames::{FrameIndex, IndexedFrame};
pub use pcm::PcmDecoder;
#[allow(unused_imports)]
pub use plan::{AudioPlan, Seeked};
#[cfg(all(feature = "transcode-aac", feature = "demux"))]
#[allow(unused_imports)]
pub use rendition::{
    fit_channels, reencode_to_aac, run_anchor, AacWindow, AAC_FRAME_SAMPLES, ENCODER_DELAY,
};
#[allow(unused_imports)]
pub use session::{ChunkKey, IndexKey, SegmentKey, TranscodeState};
#[cfg(all(feature = "transcode-aac", feature = "casting"))]
#[allow(unused_imports)]
pub use ts::{
    audio_disposition, measure_track_rates, promised_ts_length, AudioDisposition, SoundtrackFormat,
    TrackRate,
    TrackRates, TsStream,
};
#[cfg(all(feature = "transcode-aac", feature = "casting"))]
pub use video::ProgressiveStream;
#[allow(unused_imports)]
pub use source::{PacketSource, PcmStream};
#[allow(unused_imports)]
pub use wav::{wav_header, WAV_HEADER_LEN};

#[cfg(feature = "casting")]
pub(crate) fn seek_target(requested_secs: f64) -> f64 {
    /// Shorter than a frame at any rate a film is shot at, and longer than the
    /// rounding a container's own timestamps carry.
    const BACK_OFF: f64 = 0.005;
    (requested_secs - BACK_OFF).max(0.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TranscodeCodec {
    /// AC-3, "Dolby Digital" (ATSC A/52).
    Ac3,
    /// E-AC-3, "Dolby Digital Plus" (A/52 Annex E).
    Eac3,
    /// DTS Coherent Acoustics, Core profile.
    Dts,
}

impl TranscodeCodec {
    pub fn from_stored_codec(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "ac3" | "ac-3" | "a_ac3" | "dolby digital" => Some(Self::Ac3),
            "eac3" | "e-ac-3" | "eac-3" | "a_eac3" | "ec-3" => Some(Self::Eac3),
            "dca" | "dts" | "a_dts" => Some(Self::Dts),
            _ => None,
        }
    }

    /// The short name to record in the database and report in diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ac3 => "ac3",
            Self::Eac3 => "eac3",
            Self::Dts => "dca",
        }
    }

    pub fn is_decodable(self) -> bool {
        match self {
            Self::Ac3 | Self::Eac3 => cfg!(feature = "transcode-ac3"),
            Self::Dts => cfg!(feature = "transcode-dts"),
        }
    }
}

#[cfg(feature = "transcode")]
#[cfg_attr(not(feature = "transcode-ac3"), allow(unused_variables))]
pub(crate) fn make_decoder(
    codec: TranscodeCodec,
    sample_rate: u32,
    want_channels: Option<u16>,
) -> anyhow::Result<Box<dyn oxideav_core::Decoder>> {
    #[cfg(feature = "transcode-ac3")]
    use oxideav_core::{CodecId, CodecParameters, SampleFormat};

    match codec {
        #[cfg(feature = "transcode-ac3")]
        TranscodeCodec::Ac3 | TranscodeCodec::Eac3 => {
            let mut params = CodecParameters::audio(CodecId::new(codec.as_str()));
            params.sample_rate = Some(sample_rate);
            params.channels = want_channels;
            params.sample_format = Some(SampleFormat::S16);
            // Both codecs run through the same decoder — E-AC-3 is Annex E of the
            // same specification and dispatch is on the per-packet bsid — but the
            // eac3 factory registers the eac3 codec id, which is what the frame's
            // own reported id has to match for the registry-facing accessors.
            match codec {
                TranscodeCodec::Eac3 => vuio_codec_ac3::decoder::make_eac3_decoder(&params),
                _ => vuio_codec_ac3::decoder::make_decoder(&params),
            }
            .map_err(|e| anyhow::anyhow!("AC-3 decoder: {e}"))
        }
        #[allow(unreachable_patterns)]
        other => anyhow::bail!(
            "this build of vuio-core was compiled without a decoder for {}",
            other.as_str()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_codec_names_map_to_the_three_codecs() {
        assert_eq!(TranscodeCodec::from_stored_codec("ac3"), Some(TranscodeCodec::Ac3));
        assert_eq!(TranscodeCodec::from_stored_codec("EAC3"), Some(TranscodeCodec::Eac3));
        assert_eq!(TranscodeCodec::from_stored_codec("A_EAC3"), Some(TranscodeCodec::Eac3));
        assert_eq!(TranscodeCodec::from_stored_codec("dca"), Some(TranscodeCodec::Dts));
        assert_eq!(TranscodeCodec::from_stored_codec(" dts "), Some(TranscodeCodec::Dts));
        // Codecs every renderer already plays must never route through here.
        assert_eq!(TranscodeCodec::from_stored_codec("aac"), None);
        assert_eq!(TranscodeCodec::from_stored_codec("flac"), None);
        // Vendored has no TrueHD decoder, so it must not claim one.
        assert_eq!(TranscodeCodec::from_stored_codec("truehd"), None);
    }

    #[test]
    fn decodability_tracks_the_compiled_features() {
        assert_eq!(
            TranscodeCodec::Ac3.is_decodable(),
            cfg!(feature = "transcode-ac3")
        );
        assert_eq!(
            TranscodeCodec::Dts.is_decodable(),
            cfg!(feature = "transcode-dts")
        );
    }
}

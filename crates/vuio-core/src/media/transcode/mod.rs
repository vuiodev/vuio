//! Decoding AC-3, E-AC-3 and DTS to linear PCM.
//!
//! These three codecs are licensed, and a TV sold without the licence plays the
//! picture and nothing else. Symphonia identifies all three and demuxes them
//! fine — it has no decoder for any of them, which is the entire gap this module
//! closes, using the decoders vendored under `crates/vendor`.
//!
//! The split here follows the one in [`crate::mediainfo`]: [`TranscodeCodec`] is
//! static identification and always compiles, so a build with no decoder still
//! knows what an AC-3 track *is* and can say so; only the decode path itself is
//! behind `transcode-ac3` / `transcode-dts`. That is what lets the DIDL writer
//! ask "can this build decode this?" without a pile of `#[cfg]` at the call
//! site — it asks [`TranscodeCodec::is_decodable`] and gets an honest answer in
//! every build.

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
pub use aac::{adts_payloads, audio_specific_config, AacEncoder};
#[cfg(feature = "transcode-ac3")]
#[allow(unused_imports)]
pub use ac3::{Ac3Encoder, AC3_FRAME_SAMPLES};
// These are the vocabulary of this module's public surface — the element type
// of `FrameIndex::frames`, the result of `AudioPlan::seek`, the header length a
// caller subtracts from an offset. Nothing inside the crate spells some of them
// (field access needs no import), which reads as unused in a build where these
// modules are only `pub(crate)` rather than `pub`.
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

/// The instant to ask the demuxer for, given the instant that was requested.
///
/// A hair earlier. A coarse seek lands at or just *past* the point it is given,
/// and the caller then has to wait for a random-access point — so asking for the
/// exact time of a keyframe arrives immediately after it and costs a whole group
/// of pictures, which on a film with five-second groups is five seconds later
/// than the viewer dragged to. Backing off by less than a frame lands on that
/// keyframe instead.
///
/// It does not make the seek exact. Between keyframes there is nothing to land
/// on, so a request that falls mid-group still opens at the next one; what this
/// removes is the whole group lost when the request was already on the mark.
#[cfg(feature = "casting")]
pub(crate) fn seek_target(requested_secs: f64) -> f64 {
    /// Shorter than a frame at any rate a film is shot at, and longer than the
    /// rounding a container's own timestamps carry.
    const BACK_OFF: f64 = 0.005;
    (requested_secs - BACK_OFF).max(0.0)
}

/// An audio codec VuIO can decode but many renderers cannot play.
///
/// Deliberately not "every codec symphonia knows": this is the set that is both
/// commonly present in a library and commonly missing from a TV. AAC, MP3, FLAC
/// and PCM all play everywhere and never need this path; TrueHD is not decoded
/// by anything we vendor, so claiming it here would advertise a resource that
/// cannot be produced.
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
    /// Identify from the codec name stored in `media_files.codec`.
    ///
    /// The stored value comes from Symphonia's registry short name, so the
    /// spellings accepted here are the ones the scanner actually writes; the
    /// container-flavoured aliases are accepted too, because MKV `CodecID`
    /// strings reach some of the same columns.
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

    /// Whether *this build* can decode it.
    ///
    /// The answer is a compile-time constant, but it is asked at runtime by the
    /// DIDL writer and the streaming handler so neither has to carry `#[cfg]`.
    /// A build without the matching feature answers `false` and simply never
    /// advertises the second resource.
    pub fn is_decodable(self) -> bool {
        match self {
            Self::Ac3 | Self::Eac3 => cfg!(feature = "transcode-ac3"),
            Self::Dts => cfg!(feature = "transcode-dts"),
        }
    }
}

/// Build a trait-object decoder for `codec`.
///
/// AC-3 and E-AC-3 only. `want_channels` is passed through to the decoder
/// rather than applied afterwards: AC-3 carries the §7.8 downmix coefficients
/// in the bitstream, so asking the decoder for two channels produces the mix
/// the encoder intended, which a naive channel-summing downmix outside the
/// decoder would not. DTS does not come through here at all — the vendored
/// decoder's trait impl scales its output by an `rScale` that saturates `i32`
/// on most real films, so [`dts`] drives the reconstruction underneath it
/// instead.
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

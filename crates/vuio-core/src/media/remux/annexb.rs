//! Turning length-prefixed video back into the byte stream a transport carries.
//!
//! The same picture is framed two ways. Inside MP4 and Matroska each NAL unit is
//! preceded by its length, and the parameter sets that describe the whole
//! sequence — SPS and PPS for H.264, plus VPS for HEVC — are held once in the
//! container's own decoder configuration record. A transport stream has no such
//! record and no lengths: NAL units are separated by start codes, and the
//! parameter sets travel in the stream itself.
//!
//! That difference is not a formality. A transport stream is meant to be joined
//! part way through, so a decoder arriving at a random-access point has to find
//! everything it needs *there* rather than in a header it never saw. Which is
//! why the parameter sets are written again before every keyframe here, and why
//! a stream that carried them only once would play from the beginning and show
//! nothing at all to a set that seeked.

use super::mkv_demuxer::TrackCodec;

/// The four-byte start code, used at the head of every access unit and before
/// each parameter set.
const START_CODE: [u8; 4] = [0, 0, 0, 1];

/// An H.264 access unit delimiter: `primary_pic_type` 7, which says nothing
/// about the slices that follow and is what every muxer writes.
const AVC_DELIMITER: [u8; 6] = [0, 0, 0, 1, 0x09, 0xF0];

/// The HEVC one. Same job, and two bytes of NAL header rather than one.
const HEVC_DELIMITER: [u8; 7] = [0, 0, 0, 1, 0x46, 0x01, 0x50];

/// The parameter sets a decoder needs before it can decode anything, already in
/// Annex B form and ready to be written before each keyframe.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParameterSets(Vec<u8>);

impl ParameterSets {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Read them out of a container's decoder configuration record.
    ///
    /// `None` for a record this does not understand, which the caller should
    /// treat as a track it cannot carry rather than one to carry without them.
    pub fn parse(codec: TrackCodec, extra_data: &[u8]) -> Option<Self> {
        match codec {
            TrackCodec::Avc => parse_avcc(extra_data),
            TrackCodec::Hevc => parse_hvcc(extra_data),
            _ => None,
        }
    }
}

/// The `AVCDecoderConfigurationRecord`'s SPS and PPS, in stream order.
fn parse_avcc(record: &[u8]) -> Option<ParameterSets> {
    // version(1) profile(1) compat(1) level(1) lengthSizeMinusOne(1) numSPS(1)
    if record.len() < 6 {
        return None;
    }
    let mut out = Vec::new();
    let mut at = 5;
    for _ in 0..2 {
        // SPS then PPS: the counts are stored the same way, five bits for the
        // first and a whole byte for the second, and the low five bits are the
        // count in both.
        let count = record.get(at)? & 0x1F;
        at += 1;
        for _ in 0..count {
            let len = usize::from(u16::from_be_bytes(record.get(at..at + 2)?.try_into().ok()?));
            at += 2;
            out.extend_from_slice(&START_CODE);
            out.extend_from_slice(record.get(at..at + len)?);
            at += len;
        }
    }
    (!out.is_empty()).then_some(ParameterSets(out))
}

/// The `HEVCDecoderConfigurationRecord`'s VPS, SPS and PPS.
///
/// Laid out as a count of arrays, each naming its NAL type and holding however
/// many units of it — so unlike AVC the parameter sets are not at a fixed
/// offset and the arrays have to be walked.
fn parse_hvcc(record: &[u8]) -> Option<ParameterSets> {
    const ARRAY_COUNT_AT: usize = 22;
    if record.len() <= ARRAY_COUNT_AT {
        return None;
    }
    let mut out = Vec::new();
    let mut at = ARRAY_COUNT_AT + 1;
    for _ in 0..record[ARRAY_COUNT_AT] {
        // array_completeness(1) reserved(1) NAL_unit_type(6), then the count.
        at += 1;
        let count = u16::from_be_bytes(record.get(at..at + 2)?.try_into().ok()?);
        at += 2;
        for _ in 0..count {
            let len = usize::from(u16::from_be_bytes(record.get(at..at + 2)?.try_into().ok()?));
            at += 2;
            out.extend_from_slice(&START_CODE);
            out.extend_from_slice(record.get(at..at + len)?);
            at += len;
        }
    }
    (!out.is_empty()).then_some(ParameterSets(out))
}

/// Rewrite one length-prefixed access unit as an Annex B one.
///
/// Three things go in front of the picture, in this order.
///
/// An access unit delimiter, first, unless the unit already opens with one. A
/// container has frame boundaries of its own — a Matroska block is one access
/// unit and says so — and a transport stream has none: the boundary is exactly
/// this NAL unit and nothing else. Every muxer in the field inserts one for that
/// reason, ffmpeg included, and a decoder that relies on it shows nothing at all
/// without it.
///
/// Then `parameter_sets`, when this unit is a random-access point, so that a
/// decoder joining the stream here has them. They are deliberately not written
/// otherwise: repeating them on every frame would cost a few per cent of the
/// bitrate to say something that has not changed.
///
/// A malformed unit stops the conversion where it went wrong rather than
/// failing: what has been recovered so far is a valid, if short, access unit,
/// and one damaged frame should cost one frame.
pub fn to_annexb(
    sample: &[u8],
    parameter_sets: &ParameterSets,
    keyframe: bool,
    codec: TrackCodec,
) -> Vec<u8> {
    let delimiter: &[u8] = match codec {
        TrackCodec::Hevc => &HEVC_DELIMITER,
        _ => &AVC_DELIMITER,
    };
    let mut out = Vec::with_capacity(sample.len() + parameter_sets.0.len() + 24);
    if !opens_with_a_delimiter(sample, codec) {
        out.extend_from_slice(delimiter);
    }
    if keyframe {
        out.extend_from_slice(&parameter_sets.0);
    }
    let mut at = 0;
    while at + 4 <= sample.len() {
        let len = u32::from_be_bytes(sample[at..at + 4].try_into().unwrap()) as usize;
        at += 4;
        if len == 0 || at + len > sample.len() {
            break;
        }
        out.extend_from_slice(&START_CODE);
        out.extend_from_slice(&sample[at..at + len]);
        at += len;
    }
    out
}

/// Whether the encoder already wrote a delimiter, in which case writing a second
/// one is not belt and braces but a malformed access unit.
fn opens_with_a_delimiter(sample: &[u8], codec: TrackCodec) -> bool {
    // The length prefix, then as much of the NAL header as the codec spends on
    // its type: one byte for AVC, two for HEVC.
    let Some(header) = sample.get(4..6) else {
        return false;
    };
    match codec {
        TrackCodec::Hevc => (header[0] >> 1) & 0x3F == 35,
        _ => header[0] & 0x1F == 9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal `avcC`: one SPS of three bytes, one PPS of two.
    fn avcc() -> Vec<u8> {
        vec![
            0x01, 0x64, 0x00, 0x28, 0xFF, // version, profile, compat, level, lengthSize
            0xE1, 0x00, 0x03, 0x67, 0x64, 0x28, // one SPS
            0x01, 0x00, 0x02, 0x68, 0xEE, // one PPS
        ]
    }

    #[test]
    fn the_parameter_sets_come_out_of_the_record_in_stream_order() {
        let sets = ParameterSets::parse(TrackCodec::Avc, &avcc()).expect("a valid avcC");
        assert_eq!(
            sets.as_bytes(),
            &[0, 0, 0, 1, 0x67, 0x64, 0x28, 0, 0, 0, 1, 0x68, 0xEE],
            "SPS then PPS, each behind a start code"
        );
    }

    #[test]
    fn a_record_that_does_not_parse_is_declined_rather_than_half_read() {
        assert!(ParameterSets::parse(TrackCodec::Avc, &[]).is_none());
        assert!(ParameterSets::parse(TrackCodec::Avc, &[0x01, 0x64]).is_none());
        // A count that runs off the end of the record.
        assert!(ParameterSets::parse(TrackCodec::Avc, &[0x01, 0, 0, 0, 0xFF, 0xE1, 0xFF, 0xFF]).is_none());
        assert!(ParameterSets::parse(TrackCodec::Aac, &avcc()).is_none());
    }

    #[test]
    fn lengths_become_start_codes() {
        let sets = ParameterSets::default();
        // Two NAL units, of three and two bytes.
        let sample = [0, 0, 0, 3, 0x41, 0x9A, 0x02, 0, 0, 0, 2, 0x41, 0x9B];
        assert_eq!(
            to_annexb(&sample, &sets, false, TrackCodec::Avc),
            vec![
                0, 0, 0, 1, 0x09, 0xF0, // the delimiter this unit lacked
                0, 0, 0, 1, 0x41, 0x9A, 0x02, //
                0, 0, 0, 1, 0x41, 0x9B
            ]
        );
    }

    /// A transport stream marks access unit boundaries with a delimiter and
    /// nothing else, so every unit has to carry one — and exactly one.
    #[test]
    fn each_access_unit_opens_with_a_delimiter_and_never_two() {
        let sets = ParameterSets::default();

        // NAL type 9 is the delimiter; an encoder that wrote its own keeps it.
        let already = [0, 0, 0, 2, 0x09, 0xF0, 0, 0, 0, 2, 0x41, 0x9B];
        let out = to_annexb(&already, &sets, false, TrackCodec::Avc);
        assert_eq!(
            out,
            vec![0, 0, 0, 1, 0x09, 0xF0, 0, 0, 0, 1, 0x41, 0x9B],
            "a second delimiter would be a malformed access unit"
        );

        // HEVC spends two bytes on the NAL header and numbers the delimiter 35.
        let hevc = [0, 0, 0, 2, 0x26, 0x01];
        let out = to_annexb(&hevc, &sets, false, TrackCodec::Hevc);
        assert_eq!(&out[..7], &HEVC_DELIMITER, "and gets the HEVC spelling of it");

        let hevc_already = [0, 0, 0, 3, 0x46, 0x01, 0x50];
        let out = to_annexb(&hevc_already, &sets, false, TrackCodec::Hevc);
        assert_eq!(out, vec![0, 0, 0, 1, 0x46, 0x01, 0x50]);
    }

    /// The whole reason a transport stream can be joined part way through: a
    /// decoder arriving at a keyframe finds the parameter sets there.
    #[test]
    fn a_keyframe_carries_the_parameter_sets_and_other_frames_do_not() {
        let sets = ParameterSets::parse(TrackCodec::Avc, &avcc()).unwrap();
        let sample = [0, 0, 0, 2, 0x65, 0x88];

        let key = to_annexb(&sample, &sets, true, TrackCodec::Avc);
        assert!(
            key.starts_with(&AVC_DELIMITER),
            "the delimiter comes before them, as every muxer writes it"
        );
        assert!(
            key[AVC_DELIMITER.len()..].starts_with(sets.as_bytes()),
            "a keyframe leads with them"
        );
        assert_eq!(key.len(), AVC_DELIMITER.len() + sets.as_bytes().len() + 6);

        let inter = to_annexb(&sample, &sets, false, TrackCodec::Avc);
        assert_eq!(
            inter,
            vec![0, 0, 0, 1, 0x09, 0xF0, 0, 0, 0, 1, 0x65, 0x88],
            "and nothing else repeats them"
        );
    }

    #[test]
    fn a_damaged_unit_costs_only_itself() {
        let sets = ParameterSets::default();
        // A good unit, then a length reaching past the end of the sample.
        let sample = [0, 0, 0, 2, 0x41, 0x9A, 0x00, 0x00, 0xFF, 0xFF, 0x41];
        assert_eq!(
            to_annexb(&sample, &sets, false, TrackCodec::Avc),
            vec![0, 0, 0, 1, 0x09, 0xF0, 0, 0, 0, 1, 0x41, 0x9A],
            "what was recovered is still a valid access unit"
        );
    }
}

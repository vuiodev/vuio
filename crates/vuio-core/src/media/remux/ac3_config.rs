//! The configuration records an MP4 needs to carry AC-3 or E-AC-3 untouched.
//!
//! A television that can decode Dolby Digital should be handed Dolby Digital,
//! not a stereo AAC downmix of it: passing the track through costs no CPU and
//! keeps the 5.1 that re-encoding would throw away. What stands in the way is
//! that an `mp4a` sample entry cannot describe AC-3 — ISO-BMFF carries these in
//! `ac-3` and `ec-3` entries instead, each holding a small record that restates
//! what the bitstream's own headers already say (ETSI TS 102 366 Annex F).
//!
//! Matroska stores no `CodecPrivate` for either codec, because the syncframe is
//! self-describing. So the record is built here, from the first frame of the
//! track, by reading the same header fields the decoder would.
//!
//! Deliberately free of the vendored decoders and of `transcode-ac3`. Passing a
//! track through requires no decoder, and a build compiled without one should
//! still be able to hand a television the audio it could already play.

/// Sample rates an `fscod` of 0, 1 or 2 selects (A/52 Table 5.6).
const RATES: [u32; 3] = [48_000, 44_100, 32_000];

/// Channels each `acmod` describes, before `lfeon` (A/52 Table 5.8).
const ACMOD_CHANNELS: [u8; 8] = [2, 1, 2, 3, 3, 4, 4, 5];

/// What one AC-3 or E-AC-3 track needs stated in its sample entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ac3Config {
    /// The `dac3` or `dec3` box body, ready to wrap.
    pub record: Vec<u8>,
    /// Sample rate the syncframe declares, in Hz.
    pub sample_rate: u32,
    /// Channels the syncframe declares, the LFE included.
    pub channels: u8,
}

/// Read an AC-3 syncframe and build the `dac3` record describing it.
///
/// `None` for anything that is not a base AC-3 syncframe, which is the caller's
/// signal to fall back to decoding the track rather than to write a sample entry
/// describing something it has not actually understood.
pub fn parse_ac3(frame: &[u8]) -> Option<Ac3Config> {
    // syncword(16) crc1(16) fscod(2) frmsizecod(6) bsid(5) bsmod(3) acmod(3)
    if frame.len() < 7 || frame[0] != 0x0B || frame[1] != 0x77 {
        return None;
    }
    let fscod = frame[4] >> 6;
    let frmsizecod = frame[4] & 0x3F;
    let bsid = frame[5] >> 3;
    let bsmod = frame[5] & 0x07;
    let acmod = frame[6] >> 5;
    if fscod > 2 || bsid > 8 {
        return None;
    }

    // The mix-level fields between `acmod` and `lfeon` are present or absent
    // depending on `acmod` itself, so the one bit we want moves (A/52 §5.3.2).
    let mut bit = 6 * 8 + 3;
    if acmod & 0x01 != 0 && acmod != 0x01 {
        bit += 2; // cmixlev
    }
    if acmod & 0x04 != 0 {
        bit += 2; // surmixlev
    }
    if acmod == 0x02 {
        bit += 2; // dsurmod
    }
    let lfeon = read_bit(frame, bit)?;

    // `bit_rate_code` is the upper five bits of `frmsizecod`; the sixth selects
    // between the two frame sizes a 44.1 kHz stream alternates between, which
    // says nothing about the rate itself.
    let bit_rate_code = frmsizecod >> 1;

    let mut record = Vec::with_capacity(3);
    let mut writer = BitWriter::default();
    writer.push(u32::from(fscod), 2);
    writer.push(u32::from(bsid), 5);
    writer.push(u32::from(bsmod), 3);
    writer.push(u32::from(acmod), 3);
    writer.push(u32::from(lfeon), 1);
    writer.push(u32::from(bit_rate_code), 5);
    writer.push(0, 5); // reserved
    record.extend_from_slice(&writer.finish());

    Some(Ac3Config {
        record,
        sample_rate: RATES[fscod as usize],
        channels: ACMOD_CHANNELS[acmod as usize] + lfeon,
    })
}

/// Read an E-AC-3 syncframe and build the `dec3` record describing it.
///
/// One independent substream is described, which is what all but a handful of
/// files carry. `bsmod` is reported as zero — "complete main" — because Annex E
/// buries it behind a run of variable-length fields that would have to be
/// walked to reach it, and every renderer treats the field as advisory.
pub fn parse_eac3(frame: &[u8]) -> Option<Ac3Config> {
    // syncword(16) strmtyp(2) substreamid(3) frmsiz(11)
    // fscod(2) numblkscod(2) acmod(3) lfeon(1) bsid(5) …
    if frame.len() < 6 || frame[0] != 0x0B || frame[1] != 0x77 {
        return None;
    }
    let bsid = frame[5] >> 3;
    if !(9..=16).contains(&bsid) {
        return None;
    }
    let strmtyp = frame[2] >> 6;
    // Type 1 is a dependent substream: it cannot open a track, because what it
    // carries are extra channels for an independent one somewhere before it.
    if strmtyp == 1 {
        return None;
    }
    let frmsiz = (u32::from(frame[2] & 0x07) << 8) | u32::from(frame[3]);
    let frame_bytes = (frmsiz + 1) * 2;

    let fscod = frame[4] >> 6;
    let numblkscod = (frame[4] >> 4) & 0x03;
    let acmod = (frame[4] >> 1) & 0x07;
    let lfeon = frame[4] & 0x01;

    // `fscod == 3` is a half-rate stream: `fscod2` replaces the block count,
    // which is then six by definition (§E.2.3.1.4).
    let (sample_rate, blocks) = if fscod == 3 {
        let fscod2 = numblkscod;
        if fscod2 > 2 {
            return None;
        }
        (RATES[fscod2 as usize] / 2, 6u32)
    } else {
        (RATES[fscod as usize], [1u32, 2, 3, 6][numblkscod as usize])
    };

    // `data_rate` is stated in kbit/s, and a syncframe says everything needed to
    // work it out: this many bytes covering this many blocks of 256 samples.
    let frame_secs = f64::from(blocks * 256) / f64::from(sample_rate);
    let data_rate = ((f64::from(frame_bytes) * 8.0 / frame_secs) / 1000.0).round() as u32;

    let mut writer = BitWriter::default();
    writer.push(data_rate.min(0x1FFF), 13);
    writer.push(0, 3); // num_ind_sub, as "one substream" less one
    writer.push(u32::from(fscod.min(3)), 2);
    writer.push(u32::from(bsid), 5);
    writer.push(0, 1); // reserved
    writer.push(0, 1); // asvc
    writer.push(0, 3); // bsmod
    writer.push(u32::from(acmod), 3);
    writer.push(u32::from(lfeon), 1);
    writer.push(0, 3); // reserved
    writer.push(0, 4); // num_dep_sub
    writer.push(0, 1); // reserved, in place of chan_loc

    Some(Ac3Config {
        record: writer.finish(),
        sample_rate,
        channels: ACMOD_CHANNELS[acmod as usize] + lfeon,
    })
}

/// One bit of `data`, counted from the first bit of the first byte.
fn read_bit(data: &[u8], bit: usize) -> Option<u8> {
    let byte = data.get(bit / 8)?;
    Some((byte >> (7 - bit % 8)) & 1)
}

/// Big-endian bit packer, for records whose fields do not fall on byte edges.
#[derive(Default)]
struct BitWriter {
    out: Vec<u8>,
    partial: u8,
    used: u32,
}

impl BitWriter {
    fn push(&mut self, value: u32, bits: u32) {
        for shift in (0..bits).rev() {
            let bit = ((value >> shift) & 1) as u8;
            self.partial = (self.partial << 1) | bit;
            self.used += 1;
            if self.used == 8 {
                self.out.push(self.partial);
                self.partial = 0;
                self.used = 0;
            }
        }
    }

    /// The packed bytes, the last one padded with zeroes to a byte edge.
    fn finish(mut self) -> Vec<u8> {
        if self.used > 0 {
            self.out.push(self.partial << (8 - self.used));
        }
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vendored conformance stream: 48 kHz stereo, no LFE.
    #[cfg(feature = "transcode-ac3")]
    const AC3: &[u8] =
        include_bytes!("../../../../vendor/oxideav-ac3/tests/fixtures/sine440_stereo.ac3");

    #[cfg(feature = "transcode-ac3")]
    #[test]
    fn a_real_ac3_frame_yields_a_three_byte_record() {
        let config = parse_ac3(AC3).expect("the fixture is a base AC-3 syncframe");
        assert_eq!(config.record.len(), 3, "dac3 is exactly 24 bits");
        assert_eq!(config.sample_rate, 48_000);
        assert_eq!(config.channels, 2, "stereo, and the fixture has no LFE");

        // The record is fscod(2) bsid(5) bsmod(3) acmod(3) lfeon(1)
        // bit_rate_code(5) reserved(5), so the first byte is fscod, bsid, and
        // the leading bit of bsmod.
        assert_eq!(config.record[0] >> 6, 0, "48 kHz is fscod 0");
        let bsid = (config.record[0] >> 1) & 0x1F;
        assert_eq!(bsid, 8, "the fixture is bsid 8, plain AC-3");
        let acmod = ((config.record[1] >> 3) & 0x07) as usize;
        assert_eq!(ACMOD_CHANNELS[acmod], 2, "acmod {acmod} is not stereo");
        assert_eq!((config.record[1] >> 2) & 1, 0, "no LFE in the fixture");
    }

    #[test]
    fn anything_that_is_not_a_syncframe_is_declined() {
        assert!(parse_ac3(&[]).is_none());
        assert!(parse_ac3(&[0x0B, 0x77]).is_none(), "too short to read");
        assert!(parse_ac3(&[0xFF; 32]).is_none(), "no syncword");
        assert!(parse_eac3(&[0xFF; 32]).is_none());
    }

    /// A dependent substream carries extra channels for an independent one, so
    /// it cannot open a track of its own.
    #[test]
    fn a_dependent_substream_does_not_describe_a_track() {
        let mut frame = [0u8; 16];
        frame[0] = 0x0B;
        frame[1] = 0x77;
        frame[2] = 0x40; // strmtyp = 1
        frame[5] = 16 << 3; // bsid = 16, in the E-AC-3 range
        assert!(parse_eac3(&frame).is_none());
    }

    /// Hand-built E-AC-3 header: 48 kHz, 6 blocks, 3/2 plus LFE.
    #[test]
    fn an_eac3_header_is_described_as_five_point_one() {
        let mut frame = [0u8; 16];
        frame[0] = 0x0B;
        frame[1] = 0x77;
        // strmtyp = 0, substreamid = 0, frmsiz = 0x2FF → 1536 bytes.
        frame[2] = 0x02;
        frame[3] = 0xFF;
        // fscod = 0 (48 kHz), numblkscod = 3 (6 blocks), acmod = 7, lfeon = 1.
        frame[4] = (3 << 4) | (7 << 1) | 1;
        frame[5] = 16 << 3; // bsid = 16

        let config = parse_eac3(&frame).expect("a valid independent substream");
        assert_eq!(config.sample_rate, 48_000);
        assert_eq!(config.channels, 6, "3/2 is five channels, plus LFE");
        assert_eq!(config.record.len(), 5, "dec3 for one substream is 5 bytes");
        // data_rate occupies the leading 13 bits: 1536 bytes over 32 ms.
        let data_rate = (u32::from(config.record[0]) << 5) | (u32::from(config.record[1]) >> 3);
        assert_eq!(data_rate, 384, "1536 bytes per 32 ms is 384 kbit/s");
    }

    #[test]
    fn the_bit_writer_packs_big_endian_and_pads_the_tail() {
        let mut writer = BitWriter::default();
        writer.push(0b101, 3);
        writer.push(0b11, 2);
        // 101 then 11, left-packed and zero-padded to a byte.
        assert_eq!(writer.finish(), vec![0b1011_1000]);
    }
}

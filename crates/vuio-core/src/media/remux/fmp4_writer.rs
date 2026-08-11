//! Pure Rust ISO BMFF / fMP4 (CMAF) box serializer.

use super::mkv_demuxer::{MediaPacket, TrackInfo, TrackKind};

pub struct SampleInfo {
    pub duration: u32,
    pub size: u32,
    pub is_keyframe: bool,
}

pub struct Fmp4Writer;

impl Fmp4Writer {
    /// Build `ftyp` (File Type) box for fMP4 CMAF compatibility.
    pub fn build_ftyp() -> Vec<u8> {
        let mut box_data = Vec::with_capacity(32);
        box_data.extend_from_slice(b"ftyp");
        box_data.extend_from_slice(b"iso8"); // major_brand
        box_data.extend_from_slice(&[0, 0, 0, 1]); // minor_version = 1
        box_data.extend_from_slice(b"iso8mp41dashmp42"); // compatible_brands

        Self::wrap_box(&box_data)
    }

    /// Build `moov` (Movie Header) init segment box for a track.
    pub fn build_moov(track: &TrackInfo) -> Vec<u8> {
        let mut moov_body = Vec::new();

        // 1. mvhd (Movie Header)
        let mut mvhd = Vec::new();
        mvhd.extend_from_slice(b"mvhd");
        mvhd.push(0); // version 0
        mvhd.extend_from_slice(&[0; 3]); // flags
        mvhd.extend_from_slice(&[0; 4]); // creation_time
        mvhd.extend_from_slice(&[0; 4]); // modification_time
        mvhd.extend_from_slice(&(1000u32).to_be_bytes()); // timescale = 1000 Hz
        mvhd.extend_from_slice(&(0u32).to_be_bytes()); // duration = 0 for fMP4 init
        mvhd.extend_from_slice(&(0x00010000u32).to_be_bytes()); // rate = 1.0
        mvhd.extend_from_slice(&(0x0100u16).to_be_bytes()); // volume = 1.0
        mvhd.extend_from_slice(&[0; 10]); // reserved
                                          // Matrix structure (identity matrix)
        mvhd.extend_from_slice(&(0x00010000u32).to_be_bytes());
        mvhd.extend_from_slice(&[0; 12]);
        mvhd.extend_from_slice(&(0x00010000u32).to_be_bytes());
        mvhd.extend_from_slice(&[0; 12]);
        mvhd.extend_from_slice(&(0x40000000u32).to_be_bytes());
        mvhd.extend_from_slice(&[0; 24]); // pre_defined
        mvhd.extend_from_slice(&(2u32).to_be_bytes()); // next_track_id
        moov_body.extend_from_slice(&Self::wrap_box(&mvhd));

        // 2. trak (Track Atom)
        let trak = Self::build_trak(track);
        moov_body.extend_from_slice(&trak);

        // 3. mvex (Movie Extends Atom)
        let mvex = Self::build_mvex(track.id);
        moov_body.extend_from_slice(&mvex);

        let mut moov = Vec::new();
        moov.extend_from_slice(b"moov");
        moov.extend_from_slice(&moov_body);
        Self::wrap_box(&moov)
    }

    /// Return the timescale appropriate for a given track kind.
    ///
    /// - Video: 90 000 Hz (industry standard for MPEG-TS / HLS / DASH)
    /// - Audio: the track's sample rate (e.g. 48 000, 44 100)
    pub fn timescale_for(track: &TrackInfo) -> u32 {
        match track.track_kind {
            TrackKind::Video => 90_000,
            TrackKind::Audio => track.sample_rate.unwrap_or(44_100),
            TrackKind::Other => 1_000,
        }
    }

    fn build_trak(track: &TrackInfo) -> Vec<u8> {
        let mut trak_body = Vec::new();

        // tkhd (Track Header)
        let mut tkhd = Vec::new();
        tkhd.extend_from_slice(b"tkhd");
        tkhd.push(0); // version
        tkhd.extend_from_slice(&[0, 0, 7]); // flags = enabled + in_movie + in_preview
        tkhd.extend_from_slice(&[0; 4]); // creation_time
        tkhd.extend_from_slice(&[0; 4]); // modification_time
        tkhd.extend_from_slice(&track.id.to_be_bytes()); // track_id
        tkhd.extend_from_slice(&[0; 4]); // reserved
        tkhd.extend_from_slice(&[0; 4]); // duration
        tkhd.extend_from_slice(&[0; 8]); // reserved
        tkhd.extend_from_slice(&[0; 2]); // layer
        tkhd.extend_from_slice(&[0; 2]); // alternate_group
        let vol = if track.track_kind == TrackKind::Audio {
            0x0100u16
        } else {
            0
        };
        tkhd.extend_from_slice(&vol.to_be_bytes()); // volume
        tkhd.extend_from_slice(&[0; 2]); // reserved
                                          // Matrix structure
        tkhd.extend_from_slice(&(0x00010000u32).to_be_bytes());
        tkhd.extend_from_slice(&[0; 12]);
        tkhd.extend_from_slice(&(0x00010000u32).to_be_bytes());
        tkhd.extend_from_slice(&[0; 12]);
        tkhd.extend_from_slice(&(0x40000000u32).to_be_bytes());

        let w = (track.width.unwrap_or(0) << 16).to_be_bytes();
        let h = (track.height.unwrap_or(0) << 16).to_be_bytes();
        tkhd.extend_from_slice(&w); // width
        tkhd.extend_from_slice(&h); // height
        trak_body.extend_from_slice(&Self::wrap_box(&tkhd));

        // mdia (Media Atom)
        let mdia = Self::build_mdia(track);
        trak_body.extend_from_slice(&mdia);

        let mut trak = Vec::new();
        trak.extend_from_slice(b"trak");
        trak.extend_from_slice(&trak_body);
        Self::wrap_box(&trak)
    }

    fn build_mdia(track: &TrackInfo) -> Vec<u8> {
        let mut mdia_body = Vec::new();

        // mdhd (Media Header)
        let timescale = Self::timescale_for(track);
        let mut mdhd = Vec::new();
        mdhd.extend_from_slice(b"mdhd");
        mdhd.push(0); // version
        mdhd.extend_from_slice(&[0; 3]); // flags
        mdhd.extend_from_slice(&[0; 4]); // creation_time
        mdhd.extend_from_slice(&[0; 4]); // modification_time
        mdhd.extend_from_slice(&timescale.to_be_bytes());
        mdhd.extend_from_slice(&[0; 4]); // duration
        mdhd.extend_from_slice(&[0x55, 0xc4]); // lang: "und"
        mdhd.extend_from_slice(&[0; 2]); // pre_defined
        mdia_body.extend_from_slice(&Self::wrap_box(&mdhd));

        // hdlr (Handler Reference Atom)
        let (handler_type, name) = match track.track_kind {
            TrackKind::Video => (b"vide", "VideoHandler"),
            TrackKind::Audio => (b"soun", "SoundHandler"),
            TrackKind::Other => (b"hint", "HintHandler"),
        };
        let mut hdlr = Vec::new();
        hdlr.extend_from_slice(b"hdlr");
        hdlr.extend_from_slice(&[0; 4]); // version + flags
        hdlr.extend_from_slice(&[0; 4]); // pre_defined
        hdlr.extend_from_slice(handler_type);
        hdlr.extend_from_slice(&[0; 12]); // reserved
        hdlr.extend_from_slice(name.as_bytes());
        hdlr.push(0); // null term
        mdia_body.extend_from_slice(&Self::wrap_box(&hdlr));

        // minf (Media Information Atom)
        let minf = Self::build_minf(track);
        mdia_body.extend_from_slice(&minf);

        let mut mdia = Vec::new();
        mdia.extend_from_slice(b"mdia");
        mdia.extend_from_slice(&mdia_body);
        Self::wrap_box(&mdia)
    }

    fn build_minf(track: &TrackInfo) -> Vec<u8> {
        let mut minf_body = Vec::new();

        if track.track_kind == TrackKind::Video {
            let mut vmhd = Vec::new();
            vmhd.extend_from_slice(b"vmhd");
            vmhd.extend_from_slice(&[0, 0, 0, 1]); // version + flags
            vmhd.extend_from_slice(&[0; 8]); // graphicsmode + opcolor
            minf_body.extend_from_slice(&Self::wrap_box(&vmhd));
        } else {
            let mut smhd = Vec::new();
            smhd.extend_from_slice(b"smhd");
            smhd.extend_from_slice(&[0; 4]); // version + flags
            smhd.extend_from_slice(&[0; 4]); // balance + reserved
            minf_body.extend_from_slice(&Self::wrap_box(&smhd));
        }

        // dinf -> dref -> url
        let mut url = Vec::new();
        url.extend_from_slice(b"url ");
        url.extend_from_slice(&[0, 0, 0, 1]); // version + flags (self-contained)
        let url_box = Self::wrap_box(&url);

        let mut dref = Vec::new();
        dref.extend_from_slice(b"dref");
        dref.extend_from_slice(&[0; 4]); // version + flags
        dref.extend_from_slice(&(1u32).to_be_bytes()); // entry count
        dref.extend_from_slice(&url_box);
        let dref_box = Self::wrap_box(&dref);

        let mut dinf = Vec::new();
        dinf.extend_from_slice(b"dinf");
        dinf.extend_from_slice(&dref_box);
        minf_body.extend_from_slice(&Self::wrap_box(&dinf));

        // stbl (Sample Table)
        let stbl = Self::build_stbl(track);
        minf_body.extend_from_slice(&stbl);

        let mut minf = Vec::new();
        minf.extend_from_slice(b"minf");
        minf.extend_from_slice(&minf_body);
        Self::wrap_box(&minf)
    }

    fn build_stbl(track: &TrackInfo) -> Vec<u8> {
        let mut stbl_body = Vec::new();

        // stsd (Sample Description Atom)
        let mut stsd = Vec::new();
        stsd.extend_from_slice(b"stsd");
        stsd.extend_from_slice(&[0; 4]); // version + flags
        stsd.extend_from_slice(&(1u32).to_be_bytes()); // entry count

        let entry_box = if track.track_kind == TrackKind::Video {
            let mut sample_entry = Vec::new();
            sample_entry.extend_from_slice(b"avc1");
            sample_entry.extend_from_slice(&[0; 6]); // reserved
            sample_entry.extend_from_slice(&(1u16).to_be_bytes()); // data_reference_index
            sample_entry.extend_from_slice(&[0; 16]); // pre_defined + reserved
            sample_entry.extend_from_slice(&(track.width.unwrap_or(1920) as u16).to_be_bytes());
            sample_entry.extend_from_slice(&(track.height.unwrap_or(1080) as u16).to_be_bytes());
            sample_entry.extend_from_slice(&(0x00480000u32).to_be_bytes()); // horiz resolution 72 dpi
            sample_entry.extend_from_slice(&(0x00480000u32).to_be_bytes()); // vert resolution 72 dpi
            sample_entry.extend_from_slice(&[0; 4]); // reserved
            sample_entry.extend_from_slice(&(1u16).to_be_bytes()); // frame_count = 1
            sample_entry.extend_from_slice(&[0; 32]); // compressorname
            sample_entry.extend_from_slice(&(0x0018u16).to_be_bytes()); // depth = 24
            sample_entry.extend_from_slice(&(-1i16).to_be_bytes()); // pre_defined = -1

            if !track.extra_data.is_empty() {
                let mut avcc = Vec::new();
                avcc.extend_from_slice(b"avcC");
                avcc.extend_from_slice(&track.extra_data);
                sample_entry.extend_from_slice(&Self::wrap_box(&avcc));
            }
            Self::wrap_box(&sample_entry)
        } else {
            let mut sample_entry = Vec::new();
            sample_entry.extend_from_slice(b"mp4a");
            sample_entry.extend_from_slice(&[0; 6]); // reserved
            sample_entry.extend_from_slice(&(1u16).to_be_bytes()); // data_reference_index
            sample_entry.extend_from_slice(&[0; 8]); // reserved
            sample_entry
                .extend_from_slice(&(track.channels.unwrap_or(2) as u16).to_be_bytes());
            sample_entry.extend_from_slice(&(16u16).to_be_bytes()); // sample_size = 16
            sample_entry.extend_from_slice(&[0; 4]); // pre_defined + reserved
            sample_entry.extend_from_slice(
                &(track.sample_rate.unwrap_or(44100) << 16).to_be_bytes(),
            );

            if !track.extra_data.is_empty() {
                let mut esds = Vec::new();
                esds.extend_from_slice(b"esds");
                esds.extend_from_slice(&[0; 4]); // version + flags
                esds.extend_from_slice(&track.extra_data);
                sample_entry.extend_from_slice(&Self::wrap_box(&esds));
            }
            Self::wrap_box(&sample_entry)
        };
        stsd.extend_from_slice(&entry_box);
        stbl_body.extend_from_slice(&Self::wrap_box(&stsd));

        // Empty stts, stsc, stsz, stco for fMP4
        for tag in &[b"stts", b"stsc", b"stsz", b"stco"] {
            let mut empty_box = Vec::new();
            empty_box.extend_from_slice(*tag);
            empty_box.extend_from_slice(&[0; 4]); // version + flags
            empty_box.extend_from_slice(&[0; 4]); // entry count = 0
            stbl_body.extend_from_slice(&Self::wrap_box(&empty_box));
        }

        let mut stbl = Vec::new();
        stbl.extend_from_slice(b"stbl");
        stbl.extend_from_slice(&stbl_body);
        Self::wrap_box(&stbl)
    }

    fn build_mvex(track_id: u32) -> Vec<u8> {
        let mut trex = Vec::new();
        trex.extend_from_slice(b"trex");
        trex.extend_from_slice(&[0; 4]); // version + flags
        trex.extend_from_slice(&track_id.to_be_bytes());
        trex.extend_from_slice(&(1u32).to_be_bytes()); // default_sample_description_index
        trex.extend_from_slice(&[0; 12]); // default duration, size, flags

        let mut mvex = Vec::new();
        mvex.extend_from_slice(b"mvex");
        mvex.extend_from_slice(&Self::wrap_box(&trex));
        Self::wrap_box(&mvex)
    }

    /// Build `moof` (Movie Fragment) box for a sequence of samples.
    pub fn build_moof(
        sequence_number: u32,
        track_id: u32,
        base_decode_time: u64,
        samples: &[SampleInfo],
        data_offset: u32,
    ) -> Vec<u8> {
        let mut moof_body = Vec::new();

        // mfhd (Movie Fragment Header)
        let mut mfhd = Vec::new();
        mfhd.extend_from_slice(b"mfhd");
        mfhd.extend_from_slice(&[0; 4]); // version + flags
        mfhd.extend_from_slice(&sequence_number.to_be_bytes());
        moof_body.extend_from_slice(&Self::wrap_box(&mfhd));

        // traf (Track Fragment)
        let mut traf_body = Vec::new();

        // tfhd (Track Fragment Header)
        let mut tfhd = Vec::new();
        tfhd.extend_from_slice(b"tfhd");
        tfhd.extend_from_slice(&[0, 0, 0, 0x20]); // flags = default-base-is-moof
        tfhd.extend_from_slice(&track_id.to_be_bytes());
        traf_body.extend_from_slice(&Self::wrap_box(&tfhd));

        // tfdt (Track Fragment Decode Time)
        let mut tfdt = Vec::new();
        tfdt.extend_from_slice(b"tfdt");
        tfdt.push(1); // version 1 (64-bit)
        tfdt.extend_from_slice(&[0; 3]); // flags
        tfdt.extend_from_slice(&base_decode_time.to_be_bytes());
        traf_body.extend_from_slice(&Self::wrap_box(&tfdt));

        // trun (Track Run)
        let mut trun = Vec::new();
        trun.extend_from_slice(b"trun");
        // flags: data-offset-present (0x001) | sample-duration-present (0x100)
        //      | sample-size-present (0x200) | sample-flags-present (0x400)
        // = 0x000701 in 24-bit flags field
        trun.extend_from_slice(&[0, 0, 0x07, 0x01]);
        trun.extend_from_slice(&(samples.len() as u32).to_be_bytes());
        trun.extend_from_slice(&data_offset.to_be_bytes()); // data_offset

        for s in samples {
            trun.extend_from_slice(&s.duration.to_be_bytes());
            trun.extend_from_slice(&s.size.to_be_bytes());
            let flags: u32 = if s.is_keyframe {
                0x02000000 // keyframe
            } else {
                0x01010000 // non-keyframe
            };
            trun.extend_from_slice(&flags.to_be_bytes());
        }
        traf_body.extend_from_slice(&Self::wrap_box(&trun));

        let mut traf = Vec::new();
        traf.extend_from_slice(b"traf");
        traf.extend_from_slice(&traf_body);
        moof_body.extend_from_slice(&Self::wrap_box(&traf));

        let mut moof = Vec::new();
        moof.extend_from_slice(b"moof");
        moof.extend_from_slice(&moof_body);
        Self::wrap_box(&moof)
    }

    /// Build `mdat` (Media Data) box wrapping packet byte payloads.
    pub fn build_mdat(packets: &[MediaPacket]) -> Vec<u8> {
        let total_size: usize = packets.iter().map(|p| p.data.len()).sum();
        let mut mdat = Vec::with_capacity(8 + total_size);
        mdat.extend_from_slice(b"mdat");
        for p in packets {
            mdat.extend_from_slice(&p.data);
        }
        Self::wrap_box(&mdat)
    }

    /// Build a complete fMP4 segment (moof + mdat) from a list of media packets.
    ///
    /// This is a convenience method that builds proper `SampleInfo` entries from
    /// the packets and calculates the correct `data_offset`.
    pub fn build_segment(
        sequence_number: u32,
        track: &TrackInfo,
        base_decode_time: u64,
        packets: &[MediaPacket],
    ) -> Vec<u8> {
        let timescale = Self::timescale_for(track);

        let samples: Vec<SampleInfo> = packets
            .iter()
            .map(|p| {
                // If duration is 0 (unknown), use a sensible default:
                // video: 1 frame at 24fps in track timescale
                // audio: 1024 samples (common AAC frame size)
                let duration = if p.duration > 0 {
                    p.duration as u32
                } else {
                    match track.track_kind {
                        TrackKind::Video => timescale / 24,
                        TrackKind::Audio => 1024,
                        TrackKind::Other => 1,
                    }
                };
                SampleInfo {
                    duration,
                    size: p.data.len() as u32,
                    is_keyframe: p.is_keyframe,
                }
            })
            .collect();

        // Build moof first with a placeholder data_offset of 0 to measure its size.
        let moof_placeholder = Self::build_moof(sequence_number, track.id, base_decode_time, &samples, 0);
        // data_offset = moof box size + 8 bytes for the mdat header
        let data_offset = moof_placeholder.len() as u32 + 8;

        // Rebuild with the correct data_offset
        let moof = Self::build_moof(sequence_number, track.id, base_decode_time, &samples, data_offset);
        let mdat = Self::build_mdat(packets);

        let mut segment = Vec::with_capacity(moof.len() + mdat.len());
        segment.extend_from_slice(&moof);
        segment.extend_from_slice(&mdat);
        segment
    }

    fn wrap_box(contents: &[u8]) -> Vec<u8> {
        let size = (contents.len() + 4) as u32;
        let mut res = Vec::with_capacity(size as usize);
        res.extend_from_slice(&size.to_be_bytes());
        res.extend_from_slice(contents);
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ftyp_box_magic() {
        let ftyp = Fmp4Writer::build_ftyp();
        assert_eq!(&ftyp[4..8], b"ftyp");
        assert_eq!(&ftyp[8..12], b"iso8");
    }

    #[test]
    fn test_moov_box_magic() {
        let track = TrackInfo {
            id: 1,
            track_kind: TrackKind::Video,
            codec: "H264".into(),
            language: Some("eng".into()),
            name: None,
            sample_rate: None,
            channels: None,
            width: Some(1920),
            height: Some(1080),
            extra_data: vec![],
        };
        let moov = Fmp4Writer::build_moov(&track);
        assert_eq!(&moov[4..8], b"moov");
    }

    #[test]
    fn test_mdat_box_magic() {
        let pkt = MediaPacket {
            track_id: 1,
            pts: 0,
            dts: 0,
            duration: 100,
            is_keyframe: true,
            data: vec![1, 2, 3, 4],
        };
        let mdat = Fmp4Writer::build_mdat(&[pkt]);
        assert_eq!(&mdat[4..8], b"mdat");
    }

    #[test]
    fn test_video_timescale_is_90000() {
        let track = TrackInfo {
            id: 1,
            track_kind: TrackKind::Video,
            codec: "H264".into(),
            language: None,
            name: None,
            sample_rate: None,
            channels: None,
            width: Some(1920),
            height: Some(1080),
            extra_data: vec![],
        };
        assert_eq!(Fmp4Writer::timescale_for(&track), 90_000);
    }

    #[test]
    fn test_audio_timescale_matches_sample_rate() {
        let track = TrackInfo {
            id: 2,
            track_kind: TrackKind::Audio,
            codec: "AAC".into(),
            language: None,
            name: None,
            sample_rate: Some(48000),
            channels: Some(2),
            width: None,
            height: None,
            extra_data: vec![],
        };
        assert_eq!(Fmp4Writer::timescale_for(&track), 48_000);
    }

    #[test]
    fn test_build_segment_correct_data_offset() {
        let track = TrackInfo {
            id: 1,
            track_kind: TrackKind::Video,
            codec: "H264".into(),
            language: None,
            name: None,
            sample_rate: None,
            channels: None,
            width: Some(1920),
            height: Some(1080),
            extra_data: vec![],
        };
        let packets = vec![MediaPacket {
            track_id: 1,
            pts: 0,
            dts: 0,
            duration: 3750, // 90000 / 24
            is_keyframe: true,
            data: vec![0xAA; 100],
        }];
        let segment = Fmp4Writer::build_segment(1, &track, 0, &packets);
        // Should contain both moof and mdat
        assert!(segment.len() > 108); // moof + mdat header + 100 bytes of data
        // Verify moof magic at offset 4
        assert_eq!(&segment[4..8], b"moof");
    }

    #[test]
    fn test_trun_flags_bits() {
        let samples = vec![SampleInfo {
            duration: 3000,
            size: 100,
            is_keyframe: true,
        }];
        let moof = Fmp4Writer::build_moof(1, 1, 0, &samples, 8);
        // The trun box should be inside the moof. Search for "trun" magic.
        let trun_pos = moof
            .windows(4)
            .position(|w| w == b"trun")
            .expect("trun box not found");
        // Flags are the 4 bytes after "trun": version (1 byte) + flags (3 bytes)
        let flags_bytes = &moof[trun_pos + 4..trun_pos + 8];
        assert_eq!(flags_bytes[0], 0x00); // version 0
        // flags = 0x000701
        assert_eq!(flags_bytes[1], 0x00);
        assert_eq!(flags_bytes[2], 0x07);
        assert_eq!(flags_bytes[3], 0x01);
    }
}

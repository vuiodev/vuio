//! Pure Rust ISO BMFF / fMP4 (CMAF) box serializer.

use super::mkv_demuxer::{MediaPacket, TrackCodec, TrackInfo, TrackKind};

pub struct SampleInfo {
    pub duration: u32,
    pub size: u32,
    pub is_keyframe: bool,
    /// Presentation time minus decode time, in the track's output timescale. Zero for
    /// every sample unless the source has B-frames (decode order != presentation
    /// order) — without this, a player has no choice but to display frames in decode
    /// order, since nothing else in a fragmented MP4 carries presentation timing.
    pub composition_time_offset: i32,
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

        let entry_box = match track.codec_kind {
            TrackCodec::Avc | TrackCodec::Hevc => {
                let fourcc: &[u8; 4] = if track.codec_kind == TrackCodec::Hevc {
                    b"hvc1"
                } else {
                    b"avc1"
                };
                let mut sample_entry = Vec::new();
                sample_entry.extend_from_slice(fourcc);
                sample_entry.extend_from_slice(&[0; 6]); // reserved
                sample_entry.extend_from_slice(&(1u16).to_be_bytes()); // data_reference_index
                sample_entry.extend_from_slice(&[0; 16]); // pre_defined + reserved
                sample_entry
                    .extend_from_slice(&(track.width.unwrap_or(1920) as u16).to_be_bytes());
                sample_entry
                    .extend_from_slice(&(track.height.unwrap_or(1080) as u16).to_be_bytes());
                sample_entry.extend_from_slice(&(0x00480000u32).to_be_bytes()); // horiz resolution 72 dpi
                sample_entry.extend_from_slice(&(0x00480000u32).to_be_bytes()); // vert resolution 72 dpi
                sample_entry.extend_from_slice(&[0; 4]); // reserved
                sample_entry.extend_from_slice(&(1u16).to_be_bytes()); // frame_count = 1
                sample_entry.extend_from_slice(&[0; 32]); // compressorname
                sample_entry.extend_from_slice(&(0x0018u16).to_be_bytes()); // depth = 24
                sample_entry.extend_from_slice(&(-1i16).to_be_bytes()); // pre_defined = -1

                if !track.extra_data.is_empty() {
                    let config_box_name: &[u8; 4] = if track.codec_kind == TrackCodec::Hevc {
                        b"hvcC"
                    } else {
                        b"avcC"
                    };
                    // Matroska's CodecPrivate for V_MPEG4/ISO/AVC and V_MPEGH/ISO/HEVC
                    // *is* the AVC/HEVCDecoderConfigurationRecord, so the raw bytes can
                    // be copied straight into the box body.
                    let mut config = Vec::new();
                    config.extend_from_slice(config_box_name);
                    config.extend_from_slice(&track.extra_data);
                    sample_entry.extend_from_slice(&Self::wrap_box(&config));
                }
                Self::wrap_box(&sample_entry)
            }
            TrackCodec::Aac | TrackCodec::Unsupported => {
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
                    sample_entry
                        .extend_from_slice(&Self::wrap_box(&Self::build_esds(&track.extra_data)));
                }
                Self::wrap_box(&sample_entry)
            }
        };
        stsd.extend_from_slice(&entry_box);
        stbl_body.extend_from_slice(&Self::wrap_box(&stsd));

        // Empty stts and stsc for fMP4 — both are version+flags(4) then a single
        // entry_count(4) field.
        for tag in &[b"stts", b"stsc"] {
            let mut empty_box = Vec::new();
            empty_box.extend_from_slice(*tag);
            empty_box.extend_from_slice(&[0; 4]); // version + flags
            empty_box.extend_from_slice(&[0; 4]); // entry count = 0
            stbl_body.extend_from_slice(&Self::wrap_box(&empty_box));
        }

        // stsz has a different layout from the boxes above: version+flags(4), then
        // sample_size(4) *and* sample_count(4) — two fields, not one. Treating it like
        // the others silently drops sample_count, so a strict parser reads 4 bytes past
        // the box's end into whatever follows it.
        let mut stsz = Vec::new();
        stsz.extend_from_slice(b"stsz");
        stsz.extend_from_slice(&[0; 4]); // version + flags
        stsz.extend_from_slice(&[0; 4]); // sample_size = 0 (entries are variable-size)
        stsz.extend_from_slice(&[0; 4]); // sample_count = 0
        stbl_body.extend_from_slice(&Self::wrap_box(&stsz));

        // stco last, matching the order ffmpeg and other muxers emit.
        let mut stco = Vec::new();
        stco.extend_from_slice(b"stco");
        stco.extend_from_slice(&[0; 4]); // version + flags
        stco.extend_from_slice(&[0; 4]); // entry count = 0
        stbl_body.extend_from_slice(&Self::wrap_box(&stco));

        let mut stbl = Vec::new();
        stbl.extend_from_slice(b"stbl");
        stbl.extend_from_slice(&stbl_body);
        Self::wrap_box(&stbl)
    }

    /// Build an MPEG-4 descriptor (ISO/IEC 14496-1 `Descriptor` syntax): a tag byte, a
    /// length, then the content. Every descriptor built by `build_esds` is well under
    /// 128 bytes, so a single, non-continued length byte always suffices.
    fn build_descriptor(tag: u8, content: &[u8]) -> Vec<u8> {
        debug_assert!(content.len() < 0x80, "descriptor too large for a 1-byte length");
        let mut out = Vec::with_capacity(2 + content.len());
        out.push(tag);
        out.push(content.len() as u8);
        out.extend_from_slice(content);
        out
    }

    /// Build a spec-correct `esds` box wrapping a raw AAC `AudioSpecificConfig` in the
    /// ES_Descriptor/DecoderConfigDescriptor/DecoderSpecificInfo structure MSE requires
    /// (ISO/IEC 14496-1 §7.2.6.6) — an `esds` box cannot simply contain the raw config.
    fn build_esds(audio_specific_config: &[u8]) -> Vec<u8> {
        let decoder_specific_info = Self::build_descriptor(0x05, audio_specific_config);

        let mut decoder_config_content = Vec::with_capacity(13 + decoder_specific_info.len());
        decoder_config_content.push(0x40); // objectTypeIndication: MPEG-4 Audio (AAC)
        decoder_config_content.push(0x15); // streamType=5 (audio) << 2 | upStream=0 | reserved=1
        decoder_config_content.extend_from_slice(&[0, 0, 0]); // bufferSizeDB
        decoder_config_content.extend_from_slice(&[0, 0, 0, 0]); // maxBitrate
        decoder_config_content.extend_from_slice(&[0, 0, 0, 0]); // avgBitrate
        decoder_config_content.extend_from_slice(&decoder_specific_info);
        let decoder_config_descriptor = Self::build_descriptor(0x04, &decoder_config_content);

        let sl_config_descriptor = Self::build_descriptor(0x06, &[0x02]);

        let mut es_descriptor_content = Vec::with_capacity(
            3 + decoder_config_descriptor.len() + sl_config_descriptor.len(),
        );
        es_descriptor_content.extend_from_slice(&[0, 0]); // ES_ID
        es_descriptor_content.push(0x00); // flags: no dependsOn/URL/OCRstream
        es_descriptor_content.extend_from_slice(&decoder_config_descriptor);
        es_descriptor_content.extend_from_slice(&sl_config_descriptor);
        let es_descriptor = Self::build_descriptor(0x03, &es_descriptor_content);

        let mut esds = Vec::new();
        esds.extend_from_slice(b"esds");
        esds.extend_from_slice(&[0; 4]); // version + flags
        esds.extend_from_slice(&es_descriptor);
        esds
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
        //
        // flags = default-base-is-moof (0x020000) — matches trun's data-offset, which
        // is computed relative to the start of this moof. The previous value here,
        // 0x000020, is a *different* flag (default-sample-flags-present) that
        // requires a default_sample_flags field this box never wrote, so every parser
        // strict enough to honor the flags it's told (ffmpeg, and apparently Chrome's
        // MSE demuxer) read 4 bytes past the end of the box into tfdt's header,
        // corrupting every fragment from here on — this was silently tolerated by
        // ffprobe's lenient metadata-only parse, which is why it wasn't caught by
        // that alone.
        let mut tfhd = Vec::new();
        tfhd.extend_from_slice(b"tfhd");
        tfhd.extend_from_slice(&[0, 0x02, 0, 0]);
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
        //
        // version 1 (signed sample_composition_time_offset) so B-frame content —
        // where decode order != presentation order — can be reordered correctly by
        // the player. Without this field, every sample's presentation time defaults
        // to its decode time, and frames referencing data from *earlier* in
        // presentation order than their decode position have nothing to correct that.
        let mut trun = Vec::new();
        trun.extend_from_slice(b"trun");
        // flags: data-offset-present (0x001) | sample-duration-present (0x100)
        //      | sample-size-present (0x200) | sample-flags-present (0x400)
        //      | sample-composition-time-offsets-present (0x800)
        // = 0x000F01 in 24-bit flags field
        trun.push(1); // version 1
        trun.extend_from_slice(&[0, 0x0F, 0x01]);
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
            trun.extend_from_slice(&s.composition_time_offset.to_be_bytes());
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
    ///
    /// `packets` must be in decode order with decode timestamps already resolved (see
    /// `mkv_demuxer::derive_decode_timestamps`): sample durations and the fragment's
    /// base decode time are taken from `dts`, so that the decode timeline stays
    /// monotonic, while `pts - dts` supplies each sample's composition offset.
    /// `fallback_base_decode_time` is used only when `packets` is empty.
    pub fn build_segment(
        sequence_number: u32,
        track: &TrackInfo,
        fallback_base_decode_time: u64,
        packets: &[MediaPacket],
    ) -> Vec<u8> {
        let timescale = Self::timescale_for(track);

        let samples: Vec<SampleInfo> = packets
            .iter()
            .enumerate()
            .map(|(i, p)| {
                // Space samples by the gap to the next decode timestamp, so the decode
                // timeline this fragment declares matches the one the timestamps
                // describe. The container's own per-frame duration is the fallback for
                // the final sample (which has no successor here) and for sources that
                // don't carry usable timestamps at all.
                let duration = packets
                    .get(i + 1)
                    .map(|next| next.dts.saturating_sub(p.dts))
                    .filter(|gap| *gap > 0)
                    .or(Some(p.duration).filter(|d| *d > 0))
                    .unwrap_or(match track.track_kind {
                        // 1 frame at 24fps in the track timescale
                        TrackKind::Video => u64::from(timescale / 24),
                        // 1024 samples, the common AAC frame size
                        TrackKind::Audio => 1024,
                        TrackKind::Other => 1,
                    }) as u32;
                SampleInfo {
                    duration,
                    size: p.data.len() as u32,
                    is_keyframe: p.is_keyframe,
                    composition_time_offset: (p.pts as i64 - p.dts as i64) as i32,
                }
            })
            .collect();

        let base_decode_time = packets
            .first()
            .map(|p| p.dts)
            .unwrap_or(fallback_base_decode_time);

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
            codec_kind: TrackCodec::Avc,
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
            codec_kind: TrackCodec::Avc,
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
            codec_kind: TrackCodec::Aac,
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
            codec_kind: TrackCodec::Avc,
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
            composition_time_offset: 0,
        }];
        let moof = Fmp4Writer::build_moof(1, 1, 0, &samples, 8);
        // The trun box should be inside the moof. Search for "trun" magic.
        let trun_pos = moof
            .windows(4)
            .position(|w| w == b"trun")
            .expect("trun box not found");
        // Flags are the 4 bytes after "trun": version (1 byte) + flags (3 bytes)
        let flags_bytes = &moof[trun_pos + 4..trun_pos + 8];
        // version 1 — required for *signed* sample_composition_time_offset, so
        // B-frame content (decode order != presentation order) reorders correctly.
        assert_eq!(flags_bytes[0], 0x01);
        // flags = 0x000F01 (adds composition-time-offsets-present, 0x800, to the
        // previous 0x000701)
        assert_eq!(flags_bytes[1], 0x00);
        assert_eq!(flags_bytes[2], 0x0F);
        assert_eq!(flags_bytes[3], 0x01);
    }

    #[test]
    fn test_tfhd_flags_is_default_base_is_moof_with_no_extra_fields() {
        // Regression test: this box previously declared flags = 0x000020
        // (default-sample-flags-present) while a comment claimed
        // default-base-is-moof (0x020000), and never wrote the default_sample_flags
        // field that 0x000020 promises. A strict parser (ffmpeg, Chrome's MSE
        // demuxer) reads 4 bytes past the box's declared end as a result, corrupting
        // every fragment — invisible to ffprobe's lenient metadata-only parse, which
        // is why this needs its own targeted check rather than relying on ffprobe.
        let samples = vec![SampleInfo {
            duration: 3000,
            size: 100,
            is_keyframe: true,
            composition_time_offset: 0,
        }];
        let moof = Fmp4Writer::build_moof(1, 7, 0, &samples, 8);
        let tfhd_pos = moof.windows(4).position(|w| w == b"tfhd").expect("tfhd box not found");
        let box_size = u32::from_be_bytes(moof[tfhd_pos - 4..tfhd_pos].try_into().unwrap());
        let flags_bytes = &moof[tfhd_pos + 4..tfhd_pos + 8];
        assert_eq!(flags_bytes, &[0x00, 0x02, 0x00, 0x00], "flags must be default-base-is-moof (0x020000)");
        // size(4) + "tfhd"(4) + version+flags(4) + track_id(4) = 16, with nothing else,
        // since none of the optional-field flag bits are set.
        assert_eq!(box_size, 16);
        let track_id_bytes = &moof[tfhd_pos + 8..tfhd_pos + 12];
        assert_eq!(track_id_bytes, &7u32.to_be_bytes());
    }

    #[test]
    fn test_stsz_has_sample_size_and_sample_count_fields() {
        // Regression test: stsz has a different layout from stts/stsc/stco (it has
        // *two* 4-byte fields after version+flags, not one) — treating all four
        // boxes identically silently dropped the sample_count field.
        let track = TrackInfo {
            id: 1,
            track_kind: TrackKind::Video,
            codec: "H264".into(),
            codec_kind: TrackCodec::Avc,
            language: None,
            name: None,
            sample_rate: None,
            channels: None,
            width: Some(1920),
            height: Some(1080),
            extra_data: vec![],
        };
        let moov = Fmp4Writer::build_moov(&track);
        let stsz_pos = moov.windows(4).position(|w| w == b"stsz").expect("stsz box not found");
        let box_size = u32::from_be_bytes(moov[stsz_pos - 4..stsz_pos].try_into().unwrap());
        // size(4) + "stsz"(4) + version+flags(4) + sample_size(4) + sample_count(4) = 20.
        assert_eq!(box_size, 20);
    }

    #[test]
    fn test_avc_stsd_writes_avc1_and_avcc() {
        let track = TrackInfo {
            id: 1,
            track_kind: TrackKind::Video,
            codec: "H.264".into(),
            codec_kind: TrackCodec::Avc,
            language: None,
            name: None,
            sample_rate: None,
            channels: None,
            width: Some(1920),
            height: Some(1080),
            extra_data: vec![0x01, 0x64, 0x00, 0x28, 0xAB, 0xCD], // fake AVCDecoderConfigurationRecord
        };
        let moov = Fmp4Writer::build_moov(&track);
        assert!(moov.windows(4).any(|w| w == b"avc1"));
        assert!(moov.windows(4).any(|w| w == b"avcC"));
        assert!(!moov.windows(4).any(|w| w == b"hvc1"));
        // The raw CodecPrivate bytes must be copied verbatim into avcC.
        assert!(moov.windows(track.extra_data.len()).any(|w| w == track.extra_data.as_slice()));
    }

    #[test]
    fn test_hevc_stsd_writes_hvc1_and_hvcc() {
        let track = TrackInfo {
            id: 1,
            track_kind: TrackKind::Video,
            codec: "HEVC".into(),
            codec_kind: TrackCodec::Hevc,
            language: None,
            name: None,
            sample_rate: None,
            channels: None,
            width: Some(3840),
            height: Some(2160),
            extra_data: vec![0x01, 0x02, 0x20, 0x00, 0x00, 0x00], // fake HEVCDecoderConfigurationRecord
        };
        let moov = Fmp4Writer::build_moov(&track);
        assert!(moov.windows(4).any(|w| w == b"hvc1"));
        assert!(moov.windows(4).any(|w| w == b"hvcC"));
        assert!(!moov.windows(4).any(|w| w == b"avc1"));
    }

    #[test]
    fn test_avc_without_extra_data_omits_avcc_box() {
        let track = TrackInfo {
            id: 1,
            track_kind: TrackKind::Video,
            codec: "H.264".into(),
            codec_kind: TrackCodec::Avc,
            language: None,
            name: None,
            sample_rate: None,
            channels: None,
            width: Some(1920),
            height: Some(1080),
            extra_data: vec![],
        };
        let moov = Fmp4Writer::build_moov(&track);
        assert!(moov.windows(4).any(|w| w == b"avc1"));
        assert!(!moov.windows(4).any(|w| w == b"avcC"));
    }

    #[test]
    fn test_aac_stsd_esds_wraps_audio_specific_config() {
        let audio_specific_config = vec![0x11, 0x90]; // AAC-LC, 48kHz, stereo
        let track = TrackInfo {
            id: 2,
            track_kind: TrackKind::Audio,
            codec: "AAC".into(),
            codec_kind: TrackCodec::Aac,
            language: None,
            name: None,
            sample_rate: Some(48_000),
            channels: Some(2),
            width: None,
            height: None,
            extra_data: audio_specific_config.clone(),
        };
        let moov = Fmp4Writer::build_moov(&track);
        assert!(moov.windows(4).any(|w| w == b"mp4a"));
        assert!(moov.windows(4).any(|w| w == b"esds"));
        // The esds body must be a proper ES_Descriptor tree (tag 0x03), not the raw
        // AudioSpecificConfig dumped directly into the box.
        let esds_pos = moov.windows(4).position(|w| w == b"esds").unwrap();
        assert_eq!(moov[esds_pos + 8], 0x03, "esds body must start with an ES_Descriptor tag");
        // The raw AudioSpecificConfig must still be present, nested inside DecoderSpecificInfo.
        assert!(moov
            .windows(audio_specific_config.len())
            .any(|w| w == audio_specific_config.as_slice()));
    }

    #[test]
    fn test_esds_descriptor_tags_and_lengths() {
        let esds = Fmp4Writer::build_esds(&[0x11, 0x90]);
        assert_eq!(&esds[0..4], b"esds");
        assert_eq!(esds[8], 0x03, "ES_Descriptor tag");
        let es_descriptor_len = esds[9] as usize;
        assert_eq!(esds.len(), 10 + es_descriptor_len);
        // DecoderConfigDescriptor (tag 0x04) follows the 3-byte ES_ID+flags header.
        assert_eq!(esds[13], 0x04, "DecoderConfigDescriptor tag");
        assert_eq!(esds[15], 0x40, "objectTypeIndication must be MPEG-4 Audio (AAC)");
    }
}

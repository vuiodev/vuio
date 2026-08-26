//! MPEG-2 transport stream muxing, for televisions that seek by byte.
//!
//! Fragmented MP4 is the right answer for a browser and the wrong one for a
//! television. A set that scrubs a file asks for a byte offset, and a byte
//! offset into a stream produced on demand names nothing stable — answer it
//! positionally and the renderer splices two generations of the stream together
//! and decodes the join as noise.
//!
//! Transport stream is the format that does not care. It is a flat run of
//! 188-byte packets, each opening with a sync byte, and the tables describing
//! the programme are repeated forever rather than written once at the front. A
//! decoder handed the middle of one finds the next sync byte, waits for the next
//! PAT and PMT, waits for the next random-access point, and plays. That is what
//! it was designed for — it is a broadcast format, and a viewer turning a
//! television on mid-programme is the ordinary case, not the exceptional one.
//!
//! Which is why every DLNA server transcodes to this and not to MP4, and why
//! seeking a transcoded film works on hardware where the same film in fMP4 will
//! not scrub at all.
//!
//! What this module owns is the packet layer: the tables, the PES wrapping, the
//! continuity counters and the clock. What goes *into* the packets — Annex B
//! conversion, which audio is passed through and which is re-encoded — belongs
//! to [`crate::media::transcode::TsStream`].

use super::mkv_demuxer::TrackCodec;

/// Bytes in a transport packet. Fixed by the standard, and the reason a decoder
/// can find its footing anywhere in the stream.
pub const TS_PACKET_LEN: usize = 188;

/// Packets carrying nothing, used as padding.
const NULL_PID: u16 = 0x1FFF;
/// The programme association table always lives here.
const PAT_PID: u16 = 0x0000;
/// Where this muxer puts the programme map table.
pub const PMT_PID: u16 = 0x1000;
/// PID of the first elementary stream; later ones follow it.
pub const FIRST_ES_PID: u16 = 0x0100;
/// The one programme this muxer describes.
const PROGRAM_NUMBER: u16 = 1;

/// Ticks per second of the PTS/DTS clock.
pub const TS_CLOCK_HZ: u64 = 90_000;

/// One elementary stream, as the tables and the PES headers need it described.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsStreamSpec {
    pub pid: u16,
    /// The PMT's `stream_type`.
    pub stream_type: u8,
    /// The PES `stream_id`: which range of the MPEG-1 stream map this belongs
    /// to. Video takes `0xE0`, MPEG audio `0xC0`, and everything carried as
    /// private data — which is where AC-3 lives — takes `0xBD`.
    pub stream_id: u8,
    /// Descriptors for this stream's PMT entry, already encoded.
    pub descriptors: Vec<u8>,
}

impl TsStreamSpec {
    /// How `codec` is described in a transport stream, or `None` for one that
    /// has no place in it.
    pub fn for_codec(codec: TrackCodec, pid: u16) -> Option<Self> {
        // A registration descriptor naming the format is what an ATSC decoder
        // looks for to confirm a privately-carried stream is really Dolby;
        // `stream_type` alone is enough for most, and both together is what
        // every muxer in the field writes.
        let registration = |tag: &[u8; 4]| {
            let mut out = vec![0x05, 4];
            out.extend_from_slice(tag);
            out
        };
        let (stream_type, stream_id, descriptors) = match codec {
            TrackCodec::Avc => (0x1B, 0xE0, Vec::new()),
            TrackCodec::Hevc => (0x24, 0xE0, Vec::new()),
            TrackCodec::Aac => (0x0F, 0xC0, Vec::new()),
            TrackCodec::Ac3 => (0x81, 0xBD, registration(b"AC-3")),
            TrackCodec::Eac3 => (0x87, 0xBD, registration(b"EAC3")),
            TrackCodec::Dts | TrackCodec::Unsupported => return None,
        };
        Some(Self {
            pid,
            stream_type,
            stream_id,
            descriptors,
        })
    }
}

/// When one access unit is decoded and shown, and what a decoder arriving at it
/// should be told.
///
/// Grouped because every field is a property of the same instant, and a PES
/// writer taking four bare integers is one transposed pair away from telling a
/// decoder to show a frame before it decodes it.
#[derive(Debug, Clone, Copy, Default)]
pub struct PesTiming {
    pub pts: u64,
    /// Only written when it differs from `pts`, which is the convention and
    /// saves five bytes on every audio frame and every unreordered video one.
    pub dts: Option<u64>,
    /// Whether a decoder may start here.
    pub random_access: bool,
    /// Puts the programme clock in this packet's adaptation field. A decoder
    /// needs it regularly to run its own clock against.
    pub pcr: Option<u64>,
}

/// Packet-level state: what each PID's continuity counter is up to.
///
/// A decoder uses that counter to notice dropped packets, so it has to advance
/// by exactly one per packet carrying payload, per PID, and wrap at sixteen.
#[derive(Default)]
pub struct TsMuxer {
    continuity: std::collections::HashMap<u16, u8>,
}

impl TsMuxer {
    pub fn new() -> Self {
        Self::default()
    }

    /// The programme association table: one programme, and where its map is.
    pub fn pat(&mut self) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&PROGRAM_NUMBER.to_be_bytes());
        body.extend_from_slice(&(0xE000 | PMT_PID).to_be_bytes());
        let section = section(0x00, PROGRAM_NUMBER, &body);
        self.table_packet(PAT_PID, &section)
    }

    /// The programme map table: every elementary stream, and which PID carries
    /// the clock.
    pub fn pmt(&mut self, pcr_pid: u16, streams: &[TsStreamSpec]) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&(0xE000 | pcr_pid).to_be_bytes());
        body.extend_from_slice(&(0xF000u16).to_be_bytes()); // program_info_length = 0
        for stream in streams {
            body.push(stream.stream_type);
            body.extend_from_slice(&(0xE000 | stream.pid).to_be_bytes());
            body.extend_from_slice(&(0xF000 | stream.descriptors.len() as u16).to_be_bytes());
            body.extend_from_slice(&stream.descriptors);
        }
        let section = section(0x02, PROGRAM_NUMBER, &body);
        self.table_packet(PMT_PID, &section)
    }

    /// Wrap one access unit as a PES packet and split it across TS packets.
    pub fn pes(
        &mut self,
        out: &mut Vec<u8>,
        spec: &TsStreamSpec,
        payload: &[u8],
        timing: &PesTiming,
    ) {
        let &PesTiming {
            pts,
            dts,
            random_access,
            pcr,
        } = timing;
        let dts = dts.filter(|dts| *dts != pts);
        let mut header = Vec::with_capacity(19);
        header.extend_from_slice(&[0x00, 0x00, 0x01, spec.stream_id]);

        let stamps = if dts.is_some() { 10 } else { 5 };
        // A video PES may be longer than the field can express, and zero is the
        // defined escape for "runs until the next one starts". Audio always
        // fits, and stating it lets a decoder validate the frame.
        let pes_len = 3 + stamps + payload.len();
        let declared = if spec.stream_id == 0xE0 || pes_len > 0xFFFF {
            0
        } else {
            pes_len as u16
        };
        header.extend_from_slice(&declared.to_be_bytes());
        // '10', not scrambled, not priority, data-aligned.
        header.push(0b1000_0100);
        header.push(if dts.is_some() { 0b1100_0000 } else { 0b1000_0000 });
        header.push(stamps as u8);
        if let Some(dts) = dts {
            header.extend_from_slice(&timestamp(0b0011, pts));
            header.extend_from_slice(&timestamp(0b0001, dts));
        } else {
            header.extend_from_slice(&timestamp(0b0010, pts));
        }

        let mut body = header;
        body.extend_from_slice(payload);
        self.packetize(out, spec.pid, &body, random_access, pcr);
    }

    /// A packet carrying nothing, which is how a transport stream is padded.
    pub fn null(out: &mut Vec<u8>) {
        let mut packet = [0xFFu8; TS_PACKET_LEN];
        packet[0] = 0x47;
        packet[1] = (NULL_PID >> 8) as u8;
        packet[2] = (NULL_PID & 0xFF) as u8;
        packet[3] = 0x10; // payload only, continuity is not counted for null PIDs
        out.extend_from_slice(&packet);
    }

    /// Split `body` across as many packets as it needs.
    ///
    /// The first carries the payload-unit-start flag, and the last is padded out
    /// with an adaptation field rather than being left short — a transport
    /// packet is 188 bytes whether or not there is that much to say.
    fn packetize(
        &mut self,
        out: &mut Vec<u8>,
        pid: u16,
        body: &[u8],
        random_access: bool,
        pcr: Option<u64>,
    ) {
        let mut offset = 0;
        let mut first = true;
        while offset < body.len() {
            let mut adaptation = Vec::new();
            if first && (random_access || pcr.is_some()) {
                let mut flags = 0u8;
                if random_access {
                    flags |= 0b0100_0000;
                }
                if pcr.is_some() {
                    flags |= 0b0001_0000;
                }
                adaptation.push(flags);
                if let Some(pcr) = pcr {
                    // 33 bits of 90 kHz base, six reserved bits, then a 9-bit
                    // 27 MHz extension this muxer leaves at zero.
                    let base = pcr;
                    adaptation.push((base >> 25) as u8);
                    adaptation.push((base >> 17) as u8);
                    adaptation.push((base >> 9) as u8);
                    adaptation.push((base >> 1) as u8);
                    adaptation.push((((base & 1) as u8) << 7) | 0x7E);
                    adaptation.push(0);
                }
            }

            // What is left for payload once the header and any adaptation field
            // are accounted for. An adaptation field costs its own length byte.
            let remaining = body.len() - offset;
            let overhead = 4 + if adaptation.is_empty() {
                0
            } else {
                1 + adaptation.len()
            };
            let mut payload = (TS_PACKET_LEN - overhead).min(remaining);
            // A short tail is padded by growing the adaptation field, which is
            // the only place a transport packet has room to waste.
            let mut stuffing = TS_PACKET_LEN - overhead - payload;
            if stuffing > 0 && adaptation.is_empty() {
                // An adaptation field has to exist before it can stuff, and its
                // own length byte eats one of the bytes being made up for.
                adaptation.push(0);
                stuffing = stuffing.saturating_sub(2);
                payload = (TS_PACKET_LEN - 4 - 1 - adaptation.len() - stuffing).min(remaining);
            }
            adaptation.extend(std::iter::repeat_n(0xFFu8, stuffing));

            let counter = self.continuity.entry(pid).or_insert(0);
            let mut packet = Vec::with_capacity(TS_PACKET_LEN);
            packet.push(0x47);
            packet.push(((u16::from(first) << 6) | (pid >> 8)) as u8);
            packet.push((pid & 0xFF) as u8);
            let control = if adaptation.is_empty() { 0b01 } else { 0b11 };
            packet.push((control << 4) | (*counter & 0x0F));
            *counter = counter.wrapping_add(1) & 0x0F;
            if !adaptation.is_empty() {
                packet.push(adaptation.len() as u8);
                packet.extend_from_slice(&adaptation);
            }
            packet.extend_from_slice(&body[offset..offset + payload]);
            debug_assert_eq!(packet.len(), TS_PACKET_LEN, "a transport packet is 188 bytes");
            out.extend_from_slice(&packet);

            offset += payload;
            first = false;
        }
    }

    /// One table section, in one packet. Every section this muxer writes is far
    /// short of a packet, so none of them ever has to be continued.
    fn table_packet(&mut self, pid: u16, section: &[u8]) -> Vec<u8> {
        let mut packet = Vec::with_capacity(TS_PACKET_LEN);
        packet.push(0x47);
        packet.push((0x40 | (pid >> 8)) as u8); // payload unit start
        packet.push((pid & 0xFF) as u8);
        let counter = self.continuity.entry(pid).or_insert(0);
        packet.push(0x10 | (*counter & 0x0F));
        *counter = counter.wrapping_add(1) & 0x0F;
        packet.push(0); // pointer_field: the section starts immediately
        packet.extend_from_slice(section);
        packet.resize(TS_PACKET_LEN, 0xFF);
        packet
    }
}

/// Wrap a table body in its section header and CRC.
fn section(table_id: u8, id_extension: u16, body: &[u8]) -> Vec<u8> {
    // table_id_extension(2) + version/current(1) + section/last(2) + body + CRC(4)
    let section_length = 5 + body.len() + 4;
    let mut out = Vec::with_capacity(3 + section_length);
    out.push(table_id);
    // syntax indicator set, '0', two reserved bits, then the length.
    out.extend_from_slice(&(0xB000 | section_length as u16).to_be_bytes());
    out.extend_from_slice(&id_extension.to_be_bytes());
    out.push(0xC1); // reserved, version 0, current
    out.push(0x00); // section_number
    out.push(0x00); // last_section_number
    out.extend_from_slice(body);
    let crc = mpeg_crc32(&out);
    out.extend_from_slice(&crc.to_be_bytes());
    out
}

/// A PTS or DTS, in the five bytes a PES header spends on one.
///
/// Thirty-three bits, broken into three runs and interleaved with marker bits
/// that are always one — so that a parser scanning for a start code can never
/// mistake a timestamp for the beginning of a packet.
fn timestamp(prefix: u8, value: u64) -> [u8; 5] {
    let value = value & 0x1_FFFF_FFFF;
    [
        (prefix << 4) | (((value >> 30) & 0x07) as u8) << 1 | 1,
        ((value >> 22) & 0xFF) as u8,
        ((((value >> 15) & 0x7F) as u8) << 1) | 1,
        ((value >> 7) & 0xFF) as u8,
        (((value & 0x7F) as u8) << 1) | 1,
    ]
}

/// The CRC every MPEG-2 section ends with: the standard 32-bit polynomial, most
/// significant bit first, starting from all ones and not inverted at the end.
fn mpeg_crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04C1_1DB7
            } else {
                crc << 1
            };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The value every reference implementation produces for this input, and the
    /// one a television checks each table against before believing it.
    #[test]
    fn the_section_crc_matches_the_mpeg_polynomial() {
        // `0x0376E6E7` over "123456789" is the published check value for
        // CRC-32/MPEG-2, which is what pins the polynomial, the all-ones start
        // and the absence of any reflection or final inversion.
        assert_eq!(mpeg_crc32(b"123456789"), 0x0376_E6E7);
        assert_eq!(mpeg_crc32(&[0x00]), 0x4E08_BFB4);
    }

    #[test]
    fn a_timestamp_carries_its_marker_bits_and_survives_a_round_trip() {
        let value = 0x1_2345_6789u64;
        let encoded = timestamp(0b0010, value);
        assert_eq!(encoded[0] >> 4, 0b0010, "the prefix names which stamp it is");
        for (index, byte) in encoded.iter().enumerate() {
            if index % 2 == 0 {
                assert_eq!(byte & 1, 1, "byte {index} must end in a marker bit");
            }
        }
        let decoded = (u64::from(encoded[0] & 0x0E) << 29)
            | (u64::from(encoded[1]) << 22)
            | (u64::from(encoded[2] & 0xFE) << 14)
            | (u64::from(encoded[3]) << 7)
            | (u64::from(encoded[4]) >> 1);
        assert_eq!(decoded, value);
    }

    #[test]
    fn every_packet_is_a_hundred_and_eighty_eight_bytes_starting_with_a_sync() {
        let mut muxer = TsMuxer::new();
        let spec = TsStreamSpec::for_codec(TrackCodec::Avc, FIRST_ES_PID).unwrap();
        let mut out = muxer.pat();
        out.extend_from_slice(&muxer.pmt(spec.pid, std::slice::from_ref(&spec)));
        // A payload deliberately not a multiple of anything, so the tail has to
        // be stuffed.
        muxer.pes(
            &mut out,
            &spec,
            &vec![0xABu8; 1000],
            &PesTiming {
                pts: 90_000,
                dts: None,
                random_access: true,
                pcr: Some(90_000),
            },
        );
        TsMuxer::null(&mut out);

        assert_eq!(out.len() % TS_PACKET_LEN, 0, "packets must tile the stream");
        for packet in out.chunks(TS_PACKET_LEN) {
            assert_eq!(packet[0], 0x47, "every packet opens with a sync byte");
        }
    }

    /// A decoder uses this to notice a dropped packet, so it has to advance by
    /// exactly one per packet on a PID and wrap at sixteen.
    #[test]
    fn continuity_counters_advance_once_per_packet_and_wrap() {
        let mut muxer = TsMuxer::new();
        let spec = TsStreamSpec::for_codec(TrackCodec::Aac, FIRST_ES_PID).unwrap();
        let mut out = Vec::new();
        for frame in 0..20 {
            muxer.pes(
                &mut out,
                &spec,
                &[0u8; 20],
                &PesTiming {
                    pts: 90_000 * frame,
                    random_access: true,
                    ..Default::default()
                },
            );
        }
        let counters: Vec<u8> = out
            .chunks(TS_PACKET_LEN)
            .map(|packet| packet[3] & 0x0F)
            .collect();
        assert_eq!(counters.len(), 20, "each frame fits in one packet");
        for (index, counter) in counters.iter().enumerate() {
            assert_eq!(*counter, (index % 16) as u8, "at packet {index}");
        }
    }

    #[test]
    fn dolby_is_carried_as_private_data_with_a_registration_descriptor() {
        let ac3 = TsStreamSpec::for_codec(TrackCodec::Ac3, FIRST_ES_PID).unwrap();
        assert_eq!(ac3.stream_type, 0x81);
        assert_eq!(ac3.stream_id, 0xBD, "AC-3 rides in private_stream_1");
        assert_eq!(ac3.descriptors, vec![0x05, 4, b'A', b'C', b'-', b'3']);
        let eac3 = TsStreamSpec::for_codec(TrackCodec::Eac3, FIRST_ES_PID).unwrap();
        assert_eq!(eac3.stream_type, 0x87);
        // DTS has no place here: it reaches this muxer re-encoded as AAC.
        assert!(TsStreamSpec::for_codec(TrackCodec::Dts, FIRST_ES_PID).is_none());
    }
}

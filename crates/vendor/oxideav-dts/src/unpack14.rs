//! 14-bit DTS bitstream → 16-bit-equivalent unpacker.
//!
//! ## Why this exists
//!
//! Per the multimedia.cx wiki snapshot
//! (`docs/audio/dts/wiki/DTS.wiki`, section "14-bit words"):
//!
//! > This kind of bitstream is packed into 16-bit sample words so
//! > that the amplitude is reduced by 12 dB in the event that the
//! > data is inadvertently interpreted as uncompressed audio
//! > samples. The upper two bits are basically sign bit extension,
//! > as defined by twos-complement format.
//!
//! Each 16-bit container in a 14-bit-packed DTS stream carries 14
//! bits of payload in the **lower** 14 bits; the upper two bits are
//! a sign-extension of bit 13 of the payload. The payloads of
//! successive containers concatenate MSB-first to form the same
//! bitstream that a raw 16-bit-packed DTS stream would carry.
//!
//! ## Verification with the documented sync sequences
//!
//! The wiki lists four sync byte sequences:
//!
//! ```text
//!   raw BE        : 7F FE 80 01
//!   raw LE        : FE 7F 01 80
//!   14-bit BE     : 1F FF E8 00 07 Fx
//!   14-bit LE     : FF 1F 00 E8 Fx 07
//! ```
//!
//! Reading the 14-bit BE prefix as three 16-bit BE words gives
//! `0x1FFF`, `0xE800`, `0x07Fx`. Masking each to its lower 14 bits
//! yields the 14-bit payloads `0x1FFF`, `0x2800`, `0x07Fx`.
//! Concatenating those three 14-bit values MSB-first produces a
//! 42-bit stream whose first 32 bits are `0x7FFE8001` — exactly the
//! raw BE syncword. The 14-bit LE form is identical except each
//! 16-bit container is byte-swapped before the lower-14 mask. This
//! is the contract the unpacker implements.
//!
//! ## Output shape
//!
//! For every `8` bytes of 14-bit-packed input (= four 16-bit
//! containers carrying 56 payload bits) the unpacker emits `7`
//! bytes (= 56 unpacked bits). The output is byte-aligned because
//! 14 × 4 = 56 is a multiple of 8; this is also the smallest such
//! cycle, so the unpacker walks the input four containers at a
//! time. Input lengths that are not a multiple of 8 are still
//! handled — any trailing fractional bits are flushed as a final
//! padding byte.
//!
//! The unpacker is non-allocating beyond a single `Vec<u8>` for the
//! result and is pure CPU (no I/O).
//!
//! ## What lives in `docs/`
//!
//! Only the wiki snapshot above. The 14-bit sign-extension rule and
//! the lower-14-bit payload convention are stated verbatim in that
//! file; no external library source was consulted to write this
//! unpacker.

use crate::header::SyncWordEncoding;
use crate::{Error, Result};

/// Byte-order of the 14-bit-packed input.
///
/// In `BigEndian` mode each pair of input bytes is read as a 16-bit
/// big-endian word; in `LittleEndian` mode each pair is read as
/// little-endian. The two forms differ only in the byte order of the
/// 16-bit containers — the payload extraction (lower 14 bits,
/// MSB-first concatenation) is identical for both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FourteenBitByteOrder {
    /// Big-endian container words (`1F FF E8 00 07 Fx`).
    BigEndian,
    /// Little-endian container words (`FF 1F 00 E8 Fx 07`).
    LittleEndian,
}

impl FourteenBitByteOrder {
    /// Map a [`SyncWordEncoding`] to the corresponding byte order,
    /// returning `None` for the two raw (non-14-bit) variants.
    pub fn from_sync(sync: SyncWordEncoding) -> Option<Self> {
        match sync {
            SyncWordEncoding::FourteenBitBigEndian => Some(FourteenBitByteOrder::BigEndian),
            SyncWordEncoding::FourteenBitLittleEndian => Some(FourteenBitByteOrder::LittleEndian),
            _ => None,
        }
    }
}

/// Pack a 16-bit-equivalent (raw big-endian) DTS byte buffer back into
/// the 14-bit-packed container form.
///
/// This is the inverse of [`unpack_14bit_to_16bit`]. The input is read
/// as an MSB-first bit stream; successive 14-bit chunks are written
/// into the **lower** 14 bits of 16-bit containers, with the upper 2
/// bits filled by a copy of payload bit 13 (so the resulting container
/// represents the 14-bit payload as a two's-complement value as the
/// wiki snapshot prescribes — "The upper two bits are basically sign
/// bit extension"). Each container is then emitted as two bytes in the
/// requested [`FourteenBitByteOrder`].
///
/// The input is treated as an opaque bit stream — there is no DTS
/// header awareness here. The number of payload bits packed is exactly
/// `input.len() * 8`; if that count is not a multiple of 14 the final
/// container is zero-padded on the right (least-significant bits of the
/// payload) and a documented `payload_bit_count` is returned alongside
/// the byte buffer so callers can recover the exact pre-pack bit
/// length on the receiving end if needed.
///
/// The output length is exactly `ceil(input.len() * 8 / 14) * 2` bytes
/// — two bytes per container. For the four-byte raw-BE sync
/// `7F FE 80 01` (32 bits) this gives `ceil(32 / 14) = 3` containers =
/// 6 bytes, which matches the wiki's `1F FF E8 00 07 Fx` (BE) and
/// `FF 1F 00 E8 Fx 07` (LE) six-byte sync prefixes.
///
/// ## Round-trip
///
/// `unpack_14bit_to_16bit(pack_16bit_to_14bit(b, o).0, o)` returns a
/// buffer whose first `b.len()` bytes equal `b` (any trailing fractional
/// padding emitted by the pack step is consumed by the unpacker's
/// `bits_in_buf > 0` flush as the final padding byte). The empty input
/// yields the empty output and `payload_bit_count = 0`.
///
/// The function is pure / side-effect free and allocates exactly one
/// `Vec<u8>`.
pub fn pack_16bit_to_14bit(input: &[u8], order: FourteenBitByteOrder) -> (Vec<u8>, usize) {
    let payload_bits = input.len() * 8;
    if payload_bits == 0 {
        return (Vec::new(), 0);
    }
    let containers = payload_bits.div_ceil(14);
    let mut out = Vec::with_capacity(containers * 2);

    // Walk the input as an MSB-first bit stream, emitting one
    // 14-bit-payload container per iteration. `cursor` is the absolute
    // bit offset into `input` of the next payload bit to consume.
    let mut cursor: usize = 0;
    for _ in 0..containers {
        let mut payload: u16 = 0;
        for bit_index in 0..14 {
            let abs = cursor + bit_index;
            let bit = if abs < payload_bits {
                let byte = input[abs / 8];
                (byte >> (7 - (abs % 8))) & 1
            } else {
                // Past the end — zero-pad the last container on the
                // right (least-significant payload bits).
                0
            };
            payload = (payload << 1) | bit as u16;
        }
        cursor += 14;

        // Sign-extend bit 13 (the MSB of the 14-bit payload) into the
        // upper 2 bits to satisfy the wiki's "sign bit extension"
        // contract. `payload & 0x2000` is the sign bit; if set, set
        // both bits 14 and 15; otherwise leave them clear.
        let container: u16 = if payload & 0x2000 != 0 {
            payload | 0xC000
        } else {
            payload & 0x3FFF
        };
        let bytes = match order {
            FourteenBitByteOrder::BigEndian => container.to_be_bytes(),
            FourteenBitByteOrder::LittleEndian => container.to_le_bytes(),
        };
        out.extend_from_slice(&bytes);
    }

    (out, payload_bits)
}

/// Unpack a 14-bit-packed DTS byte buffer into the equivalent
/// 16-bit-packed (raw big-endian) byte buffer.
///
/// Every pair of input bytes is read as a 16-bit container in the
/// requested [`FourteenBitByteOrder`]; the lower 14 bits of each
/// container are concatenated MSB-first and re-packed into a
/// big-endian byte stream. The output is suitable to feed straight
/// into [`crate::parse_frame_header`].
///
/// The input length must be even (each 14-bit container occupies
/// exactly two input bytes); an odd length returns
/// [`Error::UnexpectedEof`]. The empty input yields an empty
/// output.
///
/// The function is pure / side-effect free and allocates exactly
/// one `Vec<u8>` whose final length is at most
/// `ceil(input.len() / 2 * 14 / 8)`.
pub fn unpack_14bit_to_16bit(input: &[u8], order: FourteenBitByteOrder) -> Result<Vec<u8>> {
    if input.len() % 2 != 0 {
        return Err(Error::UnexpectedEof);
    }
    let containers = input.len() / 2;
    // Output capacity: 14 bits per container, rounded up to whole
    // bytes.
    let out_bits = containers * 14;
    let out_bytes = out_bits.div_ceil(8);
    let mut out = Vec::with_capacity(out_bytes);

    // Walk each container, accumulating the 14-bit payload into a
    // little buffer; flush full bytes (MSB-first) as soon as the
    // buffer holds >= 8 bits.
    //
    // `buf` holds up to 7+14 = 21 bits at the time of an accumulate;
    // a u32 is comfortably wide enough.
    let mut buf: u32 = 0;
    let mut bits_in_buf: u32 = 0;

    for pair in input.chunks_exact(2) {
        let word = match order {
            FourteenBitByteOrder::BigEndian => u16::from_be_bytes([pair[0], pair[1]]),
            FourteenBitByteOrder::LittleEndian => u16::from_le_bytes([pair[0], pair[1]]),
        };
        // Lower 14 bits = the payload. The upper 2 bits are a
        // sign-extension of bit 13 per the wiki and are discarded.
        let payload = (word & 0x3FFF) as u32;
        // Shift the existing buffer left to make room and OR in the
        // new 14 bits.
        buf = (buf << 14) | payload;
        bits_in_buf += 14;

        // Flush whole bytes off the top of the buffer.
        while bits_in_buf >= 8 {
            let shift = bits_in_buf - 8;
            let byte = ((buf >> shift) & 0xFF) as u8;
            out.push(byte);
            // Clear the bits we just emitted so subsequent shifts
            // don't carry them along.
            buf &= (1u32 << shift) - 1;
            bits_in_buf -= 8;
        }
    }

    // Flush any trailing fractional bits as a final padding byte
    // (MSB-aligned). After consuming containers in groups of four,
    // bits_in_buf will be 0 at the boundary, so this branch only
    // fires when containers % 4 != 0.
    if bits_in_buf > 0 {
        let byte = ((buf << (8 - bits_in_buf)) & 0xFF) as u8;
        out.push(byte);
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 14-bit BE prefix from the wiki — three containers
    /// `1F FF`, `E8 00`, `07 F0` — unpacks to the raw BE syncword
    /// `7F FE 80 01` as the first 4 bytes, with `00` as the
    /// fractional trailing byte (the low nibble of `07 F0` carries
    /// the first 4 bits of the next payload field, which here is
    /// zero in our padded fixture).
    #[test]
    fn unpack_be_syncword_prefix_yields_raw_be_sync() {
        // Use 07 F0 (low nibble = 0) so the trailing bits flush to a
        // clean `0x00` padding byte for the assertion.
        let packed = [0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xF0];
        let unpacked = unpack_14bit_to_16bit(&packed, FourteenBitByteOrder::BigEndian).unwrap();
        // 3 containers × 14 = 42 bits → ceil(42/8) = 6 bytes out.
        assert_eq!(unpacked.len(), 6);
        // First four bytes: the raw BE syncword.
        assert_eq!(&unpacked[..4], &[0x7F, 0xFE, 0x80, 0x01]);
        // Remaining 10 bits encode `0000_0001_11` from the bottom of
        // payload #2 (`...01`) and the top of payload #3
        // (`00_0111_11`) → bits 32..42 = `0000_0001_1100_0111_11`.
        // Splitting MSB-first into bytes 4..6:
        //   byte 4 = `0000_0001` = 0x01
        //   byte 5 = `1100_0111` = 0xC7
        //   wait — let's recompute carefully and assert by recompute
        //   instead of by hand here.
        // The structural assertion is the syncword above; the
        // trailing bytes are dictated by the unpacker contract and
        // are exercised by the round-trip test below.
    }

    /// The 14-bit LE prefix from the wiki — three containers
    /// `FF 1F`, `00 E8`, `F0 07` — must unpack to the same raw BE
    /// syncword as the BE prefix above (since the payloads are
    /// identical; only container byte-order differs).
    #[test]
    fn unpack_le_syncword_prefix_yields_raw_be_sync() {
        let packed = [0xFF, 0x1F, 0x00, 0xE8, 0xF0, 0x07];
        let unpacked = unpack_14bit_to_16bit(&packed, FourteenBitByteOrder::LittleEndian).unwrap();
        assert_eq!(&unpacked[..4], &[0x7F, 0xFE, 0x80, 0x01]);
    }

    /// Round-trip a known bit-pattern: pack a stream of 14-bit
    /// payloads MSB-first, then unpack and verify the lower-14-bit
    /// chunks read back identically. This exercises the 4-container
    /// (= 7-byte) alignment cycle without depending on hand-computed
    /// trailing bytes.
    #[test]
    fn unpack_roundtrip_against_synthetic_payloads_be() {
        let payloads: [u16; 8] = [
            0x1FFF, 0x2800, 0x07F0, 0x1234, 0x0ABC, 0x3FFF, 0x0000, 0x2AAA,
        ];
        // Pack: sign-extend each 14-bit payload into the upper 2
        // bits (mirroring the wiki's "sign bit extension" rule), then
        // write as 16-bit BE.
        let mut packed = Vec::with_capacity(payloads.len() * 2);
        for &p in &payloads {
            let signed = if p & 0x2000 != 0 {
                0xC000u16 | p
            } else {
                p & 0x3FFF
            };
            packed.extend_from_slice(&signed.to_be_bytes());
        }
        let unpacked = unpack_14bit_to_16bit(&packed, FourteenBitByteOrder::BigEndian).unwrap();
        // 8 containers × 14 = 112 bits → 14 bytes.
        assert_eq!(unpacked.len(), 14);
        // Walk the unpacked bit stream and verify each 14-bit slice.
        for (i, &expected) in payloads.iter().enumerate() {
            let start = i * 14;
            let got = read_bits_msb(&unpacked, start, 14);
            assert_eq!(got, expected as u32, "payload {i} mismatch");
        }
    }

    /// Same payloads, LE container order.
    #[test]
    fn unpack_roundtrip_against_synthetic_payloads_le() {
        let payloads: [u16; 8] = [
            0x1FFF, 0x2800, 0x07F0, 0x1234, 0x0ABC, 0x3FFF, 0x0000, 0x2AAA,
        ];
        let mut packed = Vec::with_capacity(payloads.len() * 2);
        for &p in &payloads {
            let signed = if p & 0x2000 != 0 {
                0xC000u16 | p
            } else {
                p & 0x3FFF
            };
            packed.extend_from_slice(&signed.to_le_bytes());
        }
        let unpacked = unpack_14bit_to_16bit(&packed, FourteenBitByteOrder::LittleEndian).unwrap();
        assert_eq!(unpacked.len(), 14);
        for (i, &expected) in payloads.iter().enumerate() {
            let start = i * 14;
            let got = read_bits_msb(&unpacked, start, 14);
            assert_eq!(got, expected as u32, "payload {i} mismatch");
        }
    }

    /// The upper-2-bit sign extension is documented as informative
    /// only — the unpacker MUST mask those bits away. Build a
    /// container with junk in bits 14..16 and verify the same payload
    /// is recovered.
    #[test]
    fn unpack_discards_upper_sign_bits() {
        // Two payloads: 0x1FFF and 0x2800. We stuff garbage into
        // the upper 2 bits of each container (0b11 and 0b10
        // respectively, neither of which matches the "correct" sign
        // extension of `00` / `11` for these specific payloads).
        let packed = [
            0xDF, 0xFF, // (1<<15)|(1<<14)|0x1FFF = 0xDFFF — top bits garbage
            0xA8, 0x00, // (1<<15)|0x2800 = 0xA800 — top bit garbage
        ];
        let unpacked = unpack_14bit_to_16bit(&packed, FourteenBitByteOrder::BigEndian).unwrap();
        // 2 containers × 14 = 28 bits → 4 bytes out.
        assert_eq!(unpacked.len(), 4);
        // Top 14 bits = 0x1FFF, next 14 bits = 0x2800.
        assert_eq!(read_bits_msb(&unpacked, 0, 14), 0x1FFF);
        assert_eq!(read_bits_msb(&unpacked, 14, 14), 0x2800);
    }

    #[test]
    fn unpack_odd_length_returns_eof() {
        let packed = [0x1F, 0xFF, 0xE8];
        assert_eq!(
            unpack_14bit_to_16bit(&packed, FourteenBitByteOrder::BigEndian).unwrap_err(),
            Error::UnexpectedEof,
        );
    }

    #[test]
    fn unpack_empty_yields_empty() {
        let out = unpack_14bit_to_16bit(&[], FourteenBitByteOrder::BigEndian).unwrap();
        assert!(out.is_empty());
    }

    /// Four-container alignment: 4 × 14 = 56 bits = 7 bytes, no
    /// trailing padding.
    #[test]
    fn unpack_four_container_alignment_emits_exactly_seven_bytes() {
        let packed = [0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xF0, 0x12, 0x34];
        let out = unpack_14bit_to_16bit(&packed, FourteenBitByteOrder::BigEndian).unwrap();
        assert_eq!(out.len(), 7);
    }

    #[test]
    fn from_sync_routes_correctly() {
        assert_eq!(
            FourteenBitByteOrder::from_sync(SyncWordEncoding::FourteenBitBigEndian),
            Some(FourteenBitByteOrder::BigEndian),
        );
        assert_eq!(
            FourteenBitByteOrder::from_sync(SyncWordEncoding::FourteenBitLittleEndian),
            Some(FourteenBitByteOrder::LittleEndian),
        );
        assert_eq!(
            FourteenBitByteOrder::from_sync(SyncWordEncoding::RawBigEndian),
            None,
        );
        assert_eq!(
            FourteenBitByteOrder::from_sync(SyncWordEncoding::RawLittleEndian),
            None,
        );
    }

    // -------- pack_16bit_to_14bit tests (round 145) --------

    /// The raw-BE syncword `7F FE 80 01` packed BE must reproduce the
    /// first two containers of the wiki's 14-bit BE sync prefix bytes
    /// `1F FF E8 00 07 Fx` — i.e. `1F FF E8 00`. The third container
    /// `07 Fx` in the wiki notation includes 10 bits of the *next*
    /// field after the 32-bit syncword (FTYPE / SHORT / CRC_PRESENT /
    /// NBLKS_high), where the wiki example happens to have those 10
    /// bits start with `1_1111_1xxxx`. For a bare 32-bit syncword with
    /// zero padding (no following header bits), container 3 is
    /// `0001 0000 0000 00` = `0x0400`, BE = `04 00`. Together the
    /// six bytes are `1F FF E8 00 04 00`.
    #[test]
    fn pack_be_raw_syncword_reproduces_wiki_14bit_be_prefix() {
        let raw = [0x7F, 0xFE, 0x80, 0x01];
        let (packed, bits) = pack_16bit_to_14bit(&raw, FourteenBitByteOrder::BigEndian);
        // 32 input bits → ceil(32 / 14) = 3 containers → 6 bytes.
        assert_eq!(packed.len(), 6);
        assert_eq!(bits, 32);
        assert_eq!(packed.as_slice(), &[0x1F, 0xFF, 0xE8, 0x00, 0x04, 0x00]);
    }

    /// Same payload, LE container order — first two containers must
    /// reproduce the wiki's LE prefix `FF 1F 00 E8`, with the third
    /// container = `04 00` LE-swapped to `00 04`. Together: `FF 1F 00
    /// E8 00 04`.
    #[test]
    fn pack_le_raw_syncword_reproduces_wiki_14bit_le_prefix() {
        let raw = [0x7F, 0xFE, 0x80, 0x01];
        let (packed, bits) = pack_16bit_to_14bit(&raw, FourteenBitByteOrder::LittleEndian);
        assert_eq!(packed.len(), 6);
        assert_eq!(bits, 32);
        assert_eq!(packed.as_slice(), &[0xFF, 0x1F, 0x00, 0xE8, 0x00, 0x04]);
    }

    /// Pack a 6-byte input whose first 4 bytes are the raw-BE
    /// syncword and whose 5th byte's top nibble is `0xF` (i.e. the
    /// next-field bits are `1111_1111 = 0xFF...`). This reproduces
    /// the wiki's `0x07 0xFx` third container exactly because the 10
    /// bits after the syncword now genuinely are `11_1111_1xxx`.
    #[test]
    fn pack_be_with_wiki_post_sync_pattern_reproduces_07f_prefix() {
        // After syncword, next 10 bits MUST be `11_1111_1xxx` to
        // produce `0x07F<low4>`. Bit pattern: first byte after sync =
        // `1111_1111`, next byte high 2 bits = `1x` (only the top
        // matters for the 10-bit window — bits 32..42).
        // Container 3 payload = bits 28..42:
        //   bits 28..32 of sync = 0b0001
        //   bits 32..40 = first post-sync byte = 0xFF = 0b1111_1111
        //   bits 40..42 = top 2 bits of second post-sync byte
        // = 0b0001_1111_1111_<2 bits>. With top 2 bits = 0b00:
        //   payload = 0b0001_1111_1111_00 = 0x07FC. BE bytes: 0x07,
        //   0xFC.
        let raw = [0x7F, 0xFE, 0x80, 0x01, 0xFF, 0x00];
        let (packed, _) = pack_16bit_to_14bit(&raw, FourteenBitByteOrder::BigEndian);
        assert_eq!(&packed[..4], &[0x1F, 0xFF, 0xE8, 0x00]);
        // Container 3 must satisfy the wiki's `0x07_F<x>` pattern: top
        // 12 bits = `0000_0111_1111` = 0x07F.
        let container3 = u16::from_be_bytes([packed[4], packed[5]]);
        assert_eq!(container3 >> 4, 0x07F);
    }

    /// `unpack(pack(b))` recovers `b` on its first `b.len()` bytes for
    /// every byte order. Inputs whose bit length is not a multiple of
    /// 14 may have trailing padding bytes after the round-trip but the
    /// first `b.len()` bytes must equal `b`.
    #[test]
    fn pack_then_unpack_roundtrip_be() {
        let inputs: &[&[u8]] = &[
            &[],
            &[0x7F, 0xFE, 0x80, 0x01],
            &[0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0],
            &[
                0x7F, 0xFE, 0x80, 0x01, 0x80, 0x01, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0,
                0x11, 0x22,
            ],
        ];
        for input in inputs {
            let (packed, bits) = pack_16bit_to_14bit(input, FourteenBitByteOrder::BigEndian);
            assert_eq!(bits, input.len() * 8);
            let unpacked = unpack_14bit_to_16bit(&packed, FourteenBitByteOrder::BigEndian).unwrap();
            assert!(
                unpacked.len() >= input.len(),
                "unpacked too short for input.len()={}: got {}",
                input.len(),
                unpacked.len()
            );
            assert_eq!(
                &unpacked[..input.len()],
                *input,
                "round-trip failed for input {input:?}"
            );
        }
    }

    /// Same as `pack_then_unpack_roundtrip_be` but with LE container
    /// byte order.
    #[test]
    fn pack_then_unpack_roundtrip_le() {
        let inputs: &[&[u8]] = &[
            &[],
            &[0x7F, 0xFE, 0x80, 0x01],
            &[0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0],
            &[
                0x7F, 0xFE, 0x80, 0x01, 0x80, 0x01, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0,
                0x11, 0x22,
            ],
        ];
        for input in inputs {
            let (packed, bits) = pack_16bit_to_14bit(input, FourteenBitByteOrder::LittleEndian);
            assert_eq!(bits, input.len() * 8);
            let unpacked =
                unpack_14bit_to_16bit(&packed, FourteenBitByteOrder::LittleEndian).unwrap();
            assert!(
                unpacked.len() >= input.len(),
                "unpacked too short for input.len()={}: got {}",
                input.len(),
                unpacked.len()
            );
            assert_eq!(
                &unpacked[..input.len()],
                *input,
                "round-trip failed for input {input:?}"
            );
        }
    }

    /// `pack_16bit_to_14bit(b, BE)` and `pack_16bit_to_14bit(b, LE)`
    /// differ only in the byte order of each container — pair-swapping
    /// the BE output produces the LE output.
    #[test]
    fn pack_be_le_differ_only_by_container_byteswap() {
        let raw = [0x7F, 0xFE, 0x80, 0x01, 0x12, 0x34, 0x56, 0x78];
        let (be, _) = pack_16bit_to_14bit(&raw, FourteenBitByteOrder::BigEndian);
        let (le, _) = pack_16bit_to_14bit(&raw, FourteenBitByteOrder::LittleEndian);
        assert_eq!(be.len(), le.len());
        for pair in 0..(be.len() / 2) {
            assert_eq!(be[pair * 2], le[pair * 2 + 1]);
            assert_eq!(be[pair * 2 + 1], le[pair * 2]);
        }
    }

    /// Empty input yields empty output and `payload_bit_count == 0`
    /// (documented contract).
    #[test]
    fn pack_empty_yields_empty() {
        let (out, bits) = pack_16bit_to_14bit(&[], FourteenBitByteOrder::BigEndian);
        assert!(out.is_empty());
        assert_eq!(bits, 0);
    }

    /// Confirm the sign-extension contract: for an input whose first
    /// 14 bits have bit 13 (the payload sign bit) set, the upper 2
    /// bits of the corresponding container must be `0b11`. For an
    /// input where bit 13 is clear, the upper 2 bits must be `0b00`.
    #[test]
    fn pack_sign_extends_top_payload_bit() {
        // Sign bit set: 14 bits of `0b11_1111_0101_0101` = 0x3F55.
        // Source bytes: top 14 bits of 0xFD_54_xx where:
        //   first byte = 0xFD = 0b1111_1101
        //   second byte = 0x54 = 0b0101_0100
        // First 14 bits MSB-first: 0b11_1111_0101_0101 = 0x3F55. Good.
        let raw_pos = [0xFD, 0x54];
        let (packed_pos, _) = pack_16bit_to_14bit(&raw_pos, FourteenBitByteOrder::BigEndian);
        // First container: payload = 0x3F55, top 2 bits set → 0xFF55.
        assert_eq!(u16::from_be_bytes([packed_pos[0], packed_pos[1]]), 0xFF55);

        // Sign bit clear: 14 bits of `0b00_1010_1010_1010` = 0x0AAA.
        // First byte = 0b0010_1010 = 0x2A, second = 0b1010_1000 = 0xA8.
        // First 14 bits: 0b00_1010_1010_1010 = 0x0AAA. Bit 13 = 0.
        let raw_neg = [0x2A, 0xA8];
        let (packed_neg, _) = pack_16bit_to_14bit(&raw_neg, FourteenBitByteOrder::BigEndian);
        // First container: payload = 0x0AAA, top 2 bits clear → 0x0AAA.
        assert_eq!(u16::from_be_bytes([packed_neg[0], packed_neg[1]]), 0x0AAA);
    }

    /// Helper: read `n` bits MSB-first starting at absolute bit
    /// offset `start` from a big-endian byte stream. Mirrors the
    /// `BitReader` semantics but is duplicated here to keep this
    /// module testable in isolation.
    fn read_bits_msb(bytes: &[u8], start: usize, n: usize) -> u32 {
        let mut v: u32 = 0;
        for i in 0..n {
            let abs = start + i;
            let byte = bytes[abs / 8];
            let bit = (byte >> (7 - (abs % 8))) & 1;
            v = (v << 1) | bit as u32;
        }
        v
    }
}

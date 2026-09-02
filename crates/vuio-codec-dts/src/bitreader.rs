//! Minimal MSB-first bit reader for parsing the DTS frame sync
//! header.
//!
//! Round 1 only needs to walk roughly 100 bits of header from a
//! byte buffer, so this reader is deliberately small: no buffering,
//! no slicing, no skip-to-alignment helpers. The DTS bitstream is
//! defined MSB-first within each byte (per the wiki snapshot at
//! `docs/audio/dts/wiki/DTS.wiki`, which mirrors the ETSI spec's
//! convention).
//!
//! All reads return [`Result`]; the only failure mode is running
//! past the end of the buffer.

use crate::{Error, Result};

/// MSB-first bit reader over a borrowed byte slice.
///
/// Tracks the current bit position (`pos` is the index of the next
/// bit to read, counted from the MSB of `bytes[0]`).
#[derive(Debug)]
pub(crate) struct BitReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> BitReader<'a> {
    /// Construct a fresh reader positioned at bit 0. Only used by
    /// the in-module unit tests; the production header parser
    /// always starts at a byte-offset after the syncword via
    /// [`Self::from_byte_offset`].
    #[cfg(test)]
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        BitReader { bytes, pos: 0 }
    }

    /// Construct a reader that begins at an arbitrary byte offset.
    /// Used after the syncword is identified so the header read can
    /// resume from the byte immediately following the sync.
    pub(crate) fn from_byte_offset(bytes: &'a [u8], byte_offset: usize) -> Self {
        BitReader {
            bytes,
            pos: byte_offset * 8,
        }
    }

    /// Read `n` bits (1..=32) as an unsigned big-endian integer.
    ///
    /// Returns [`Error::UnexpectedEof`] if the read would walk past
    /// the end of the underlying buffer.
    pub(crate) fn read_bits(&mut self, n: u32) -> Result<u32> {
        debug_assert!((1..=32).contains(&n), "BitReader::read_bits expects 1..=32");
        let end = self.pos + n as usize;
        if end > self.bytes.len() * 8 {
            return Err(Error::UnexpectedEof);
        }
        let mut value: u32 = 0;
        let mut remaining = n;
        while remaining > 0 {
            let byte_idx = self.pos / 8;
            let bit_in_byte = self.pos % 8;
            // Bits available in this byte, MSB-first.
            let avail = (8 - bit_in_byte) as u32;
            let take = remaining.min(avail);
            // Shift the byte so the MSB of the bit window is at
            // bit position 7, then mask + shift down to bit 0 of
            // the take-bit field.
            let byte = self.bytes[byte_idx] as u32;
            let shifted = byte >> (avail - take);
            let mask = (1u32 << take) - 1;
            let chunk = shifted & mask;
            value = (value << take) | chunk;
            self.pos += take as usize;
            remaining -= take;
        }
        Ok(value)
    }

    /// Read a single bit and return it as a `bool`.
    pub(crate) fn read_bit(&mut self) -> Result<bool> {
        Ok(self.read_bits(1)? == 1)
    }

    /// Current absolute bit position.
    #[cfg(test)]
    pub(crate) fn position_bits(&self) -> usize {
        self.pos
    }

    /// Current absolute bit position, counted from the MSB of
    /// `bytes[0]`. Round-195 side-info decoders use this to report
    /// `bits_consumed` back to the caller (so the caller can advance
    /// its own bit cursor through the side-information block).
    pub(crate) fn absolute_bit_position(&self) -> usize {
        self.pos
    }

    /// Borrow the backing byte buffer. The round-340 §5.5 audio-array
    /// walk uses this to bridge a running reader into the byte-offset
    /// [`crate::audio_huff::decode_audio_huff_at`] entry point (which
    /// re-seeks over the same buffer) and then re-advance this reader.
    pub(crate) fn backing_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Advance the reader by `n` bits without materialising a value,
    /// returning [`Error::UnexpectedEof`] if the skip would walk past
    /// the end of the buffer. Used by the round-340 §5.5 audio-array
    /// walk after a Huffman index was decoded through the byte-offset
    /// entry point.
    pub(crate) fn skip_bits(&mut self, n: u32) -> Result<()> {
        let end = self.pos + n as usize;
        if end > self.bytes.len() * 8 {
            return Err(Error::UnexpectedEof);
        }
        self.pos = end;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_single_bits() {
        // 0b1011_0010 = 0xB2
        let mut br = BitReader::new(&[0xB2]);
        assert!(br.read_bit().unwrap());
        assert!(!br.read_bit().unwrap());
        assert!(br.read_bit().unwrap());
        assert!(br.read_bit().unwrap());
        assert!(!br.read_bit().unwrap());
        assert!(!br.read_bit().unwrap());
        assert!(br.read_bit().unwrap());
        assert!(!br.read_bit().unwrap());
        assert_eq!(br.position_bits(), 8);
    }

    #[test]
    fn read_multi_byte_field() {
        // 0x7F 0xFE 0x80 0x01 — the DTS BE syncword.
        let bytes = [0x7F, 0xFE, 0x80, 0x01];
        let mut br = BitReader::new(&bytes);
        assert_eq!(br.read_bits(32).unwrap(), 0x7FFE_8001);
        assert_eq!(br.position_bits(), 32);
    }

    #[test]
    fn read_crosses_byte_boundary() {
        // Bytes 0xFF 0xF0 → top 12 bits = 0xFFF.
        let bytes = [0xFF, 0xF0];
        let mut br = BitReader::new(&bytes);
        assert_eq!(br.read_bits(12).unwrap(), 0xFFF);
        // remaining 4 bits = 0.
        assert_eq!(br.read_bits(4).unwrap(), 0);
    }

    #[test]
    fn read_from_byte_offset_skips_sync() {
        // Skip the 4 sync bytes and read a 1-bit flag from byte 4.
        let bytes = [0x7F, 0xFE, 0x80, 0x01, 0b1000_0000];
        let mut br = BitReader::from_byte_offset(&bytes, 4);
        assert!(br.read_bit().unwrap());
        assert_eq!(br.position_bits(), 33);
    }

    #[test]
    fn read_past_end_returns_eof() {
        let mut br = BitReader::new(&[0xAA]);
        assert!(br.read_bits(8).is_ok());
        assert_eq!(br.read_bits(1).unwrap_err(), Error::UnexpectedEof);
    }

    #[test]
    fn read_field_at_arbitrary_bit_alignment() {
        // Construct a stream whose first 7 bits are 0b1010101 and
        // whose next 14 bits are 0b00110011001100 = 0x0CCC.
        // First byte: 0b1010101_0 | next 6 bits of 0b001100 -> 0b1010_1010
        //   0b1010101 (7 bits) + first bit of next field (0) = 0b1010_1010 = 0xAA
        //   next 8 bits (bits 8..16) = top 8 bits of remaining 13 field bits
        //     remaining 13 bits = 0b0110011001100
        //     wait: original 14 bits = 0b00_1100_1100_1100 = 0x0CCC
        //     first bit already consumed = 0, so 13 bits left = 0b0110011001100
        //   That's bits 8..16 = 0b01100110 and bits 16..21 = 0b01100
        // So encoded bytes = 0b1010_1010, 0b0110_0110, 0b0110_0xxx (pad).
        let bytes = [0xAA, 0x66, 0b0110_0000];
        let mut br = BitReader::new(&bytes);
        assert_eq!(br.read_bits(7).unwrap(), 0b1010101);
        assert_eq!(br.read_bits(14).unwrap(), 0x0CCC);
        assert_eq!(br.position_bits(), 21);
    }
}

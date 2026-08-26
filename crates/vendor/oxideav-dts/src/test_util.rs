//! Test-only helpers shared by the in-module unit-test suites:
//! an MSB-first bit writer for constructing synthetic bitstream
//! windows, and a minimal parseable raw-BE frame-header builder.

use crate::header::DtsFrameHeader;
use crate::parse_frame_header;

/// Minimal MSB-first bit writer for constructing synthetic bitstream
/// chunks in tests.
pub(crate) struct BitWriter {
    bytes: Vec<u8>,
    bit_len: usize,
}

impl BitWriter {
    pub(crate) fn new() -> Self {
        BitWriter {
            bytes: Vec::new(),
            bit_len: 0,
        }
    }

    /// Append the low `n` bits of `value`, MSB-first.
    pub(crate) fn push_bits(&mut self, value: u64, n: u32) {
        for i in (0..n).rev() {
            let bit = (value >> i) & 1;
            if self.bit_len % 8 == 0 {
                self.bytes.push(0);
            }
            let byte = self.bytes.last_mut().unwrap();
            *byte |= (bit as u8) << (7 - (self.bit_len % 8));
            self.bit_len += 1;
        }
    }

    /// Zero-pad until the running bit length is a multiple of `bits`.
    pub(crate) fn align(&mut self, bits: usize) {
        while self.bit_len % bits != 0 {
            self.push_bits(0, 1);
        }
    }

    /// Current bit length.
    pub(crate) fn bit_len(&self) -> usize {
        self.bit_len
    }

    /// Finish, zero-padding to a whole byte.
    pub(crate) fn into_bytes(mut self) -> Vec<u8> {
        self.align(8);
        self.bytes
    }
}

/// Build a parseable raw-BE frame header with the requested `AMODE`,
/// 13-bit flag window (`downmix .. predictor_history`; the 2-bit
/// `LFF` field sits one bit above `predictor_history`), and `NBLKS`
/// field (blocks per frame = `nblks + 1`).
pub(crate) fn synth_header_with_blocks(amode: u64, extra_13: u64, nblks: u64) -> DtsFrameHeader {
    let mut w = BitWriter::new();
    w.push_bits(0x7FFE_8001, 32); // raw-BE sync
    w.push_bits(1, 1); // FTYPE normal
    w.push_bits(31, 5); // SHORT deficit -> 32 samples/block
    w.push_bits(0, 1); // CPF
    w.push_bits(nblks, 7); // NBLKS
    w.push_bits(127, 14); // FSIZE -> 128 bytes
    w.push_bits(amode, 6); // AMODE
    w.push_bits(13, 4); // SFREQ 48 kHz
    w.push_bits(10, 5); // RATE
    w.push_bits(extra_13, 13); // downmix .. predictor_history
    w.push_bits(0, 16); // post-CRC trailing window
    let mut bytes = w.into_bytes();
    bytes.resize(16, 0); // parser reads a 16-byte window
    parse_frame_header(&bytes).expect("synthetic header parses")
}

/// [`synth_header_with_blocks`] with the default 16-block frame.
pub(crate) fn synth_header(amode: u64, extra_13: u64) -> DtsFrameHeader {
    synth_header_with_blocks(amode, extra_13, 15)
}
